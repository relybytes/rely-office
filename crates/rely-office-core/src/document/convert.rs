// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! From the element tree of `word/document.xml` to the neutral document tree.

use std::collections::HashMap;

use super::numbering::Numbering;
use super::styles::{ParagraphRole, RunFormat, Styles, numbering_reference, on_off, outline_level};
use super::{Block, ImportReport, Inline, Mark, Media, TableCell, TableRow, sniff_image_type};
use crate::archive::{ZipPackage, read_member_bytes};
use crate::relationships::Relationship;
use crate::xml_tree::{Element, Namespace};
use crate::{Limits, OfficeError};

const W: Namespace = Namespace::Word;
const MC: Namespace = Namespace::MarkupCompatibility;
const MAX_HEADING_LEVEL: u8 = 6;
const EMUS_PER_PIXEL: u64 = 9_525;
/// The widest span a cell may claim; anything wider is a crafted file.
const MAX_SPAN: u32 = 1_000;
/// The longest field instruction kept; a real one is a few dozen characters.
const MAX_INSTRUCTION_CHARS: usize = 4_096;

/// A complex field (`w:fldChar` begin, separate, end) still open.
#[derive(Debug)]
struct OpenField {
    /// Whether the separator has passed, so the runs are the field's result.
    in_result: bool,
    instruction: String,
    /// The target of a `HYPERLINK` field, applied to its result.
    link: Option<String>,
}

/// What the runs of a paragraph said about the paragraph itself.
#[derive(Debug, Default)]
struct ParagraphScan {
    horizontal_rule: bool,
    page_break: bool,
}

pub(super) struct Converter<'a, 'p> {
    package: &'a mut ZipPackage<'p>,
    relationships: HashMap<&'a str, &'a Relationship>,
    styles: &'a Styles,
    numbering: &'a Numbering,
    limits: &'a Limits,
    report: &'a mut ImportReport,
    media: Vec<Media>,
    media_bytes: u64,
    media_by_target: HashMap<String, Option<usize>>,
    remaining_chars: usize,
    fields: Vec<OpenField>,
}

impl<'a, 'p> Converter<'a, 'p> {
    pub(super) fn new(
        package: &'a mut ZipPackage<'p>,
        relationships: &'a [Relationship],
        styles: &'a Styles,
        numbering: &'a Numbering,
        limits: &'a Limits,
        report: &'a mut ImportReport,
    ) -> Self {
        Self {
            package,
            relationships: relationships
                .iter()
                .map(|relationship| (relationship.id.as_str(), relationship))
                .collect(),
            styles,
            numbering,
            limits,
            report,
            media: Vec::new(),
            media_bytes: 0,
            media_by_target: HashMap::new(),
            remaining_chars: limits.max_text_chars,
            fields: Vec::new(),
        }
    }

    pub(super) fn into_media(self) -> Vec<Media> {
        self.media
    }

    /// The blocks of a container of block content: the body, a table cell, a content control.
    pub(super) fn convert_blocks(&mut self, container: &Element) -> Vec<Block> {
        let mut blocks = Vec::new();
        self.append_blocks(container, &mut blocks);
        blocks
    }

    fn append_blocks(&mut self, container: &Element, blocks: &mut Vec<Block>) {
        for element in container.elements() {
            if self.report.truncated {
                return;
            }
            match (element.namespace, element.name.as_str()) {
                (W, "p" | "tbl") if self.remaining_chars == 0 => {
                    if has_text(element) {
                        self.report.truncated = true;
                    }
                }
                (W, "p") => self.append_paragraph(element, blocks),
                (W, "tbl") => {
                    let table = self.convert_table(element);
                    push_block(blocks, table);
                }
                (W, "sdt") => {
                    if let Some(content) = element.child(W, "sdtContent") {
                        self.append_blocks(content, blocks);
                    }
                }
                (W, "ins" | "moveTo") => {
                    self.report.tracked_insertions += 1;
                    self.append_blocks(element, blocks);
                }
                (W, "del" | "moveFrom") => self.report.tracked_deletions += 1,
                (W, "customXml" | "smartTag") => self.append_blocks(element, blocks),
                (MC, "AlternateContent") => {
                    if let Some(choice) = alternate_choice(element) {
                        self.append_blocks(choice, blocks);
                    }
                }
                _ => {}
            }
        }
    }

    fn append_paragraph(&mut self, paragraph: &Element, blocks: &mut Vec<Block>) {
        let properties = paragraph.child(W, "pPr");
        let style_id = properties
            .and_then(|properties| properties.child(W, "pStyle"))
            .and_then(|style| style.attribute(W, "val"));
        let style = self.styles.paragraph_style(style_id);
        let outline = properties.and_then(outline_level).or(style.outline_level);
        let numbering = properties.and_then(numbering_reference).or(style.numbering);

        let mut scan = ParagraphScan::default();
        let mut content = Vec::new();
        self.append_inlines(paragraph, None, &mut content, &mut scan);
        // A field instruction never spans paragraphs; one left open is damaged, not pending.
        self.fields.retain(|field| field.in_result);

        let blank = content.is_empty();
        let bottom_border = properties
            .and_then(|properties| properties.child(W, "pBdr"))
            .and_then(|borders| borders.child(W, "bottom"))
            .is_some();
        if blank && (scan.horizontal_rule || bottom_border) {
            blocks.push(Block::HorizontalRule);
            return;
        }
        if blank && scan.page_break {
            return;
        }

        let block = match style.role {
            ParagraphRole::Code => Block::CodeBlock {
                text: plain_text(&content),
            },
            ParagraphRole::Heading(level) => Block::Heading {
                level: level.min(MAX_HEADING_LEVEL),
                content,
            },
            _ if outline.is_some() => Block::Heading {
                level: outline
                    .unwrap_or(0)
                    .saturating_add(1)
                    .min(MAX_HEADING_LEVEL),
                content,
            },
            role => {
                let list = numbering.and_then(|reference| {
                    let level = reference.level.unwrap_or(0);
                    self.numbering
                        .is_ordered(reference.num_id, level)
                        .map(|ordered| (ordered, level, reference.num_id))
                });
                match (list, role) {
                    (Some((ordered, level, list_id)), _) => Block::ListItem {
                        ordered,
                        level,
                        list_id,
                        content,
                    },
                    (None, ParagraphRole::Quote) => Block::Blockquote {
                        blocks: vec![Block::Paragraph { content }],
                    },
                    (None, _) => Block::Paragraph { content },
                }
            }
        };
        push_block(blocks, block);
    }

    fn append_inlines(
        &mut self,
        container: &Element,
        link: Option<&str>,
        content: &mut Vec<Inline>,
        scan: &mut ParagraphScan,
    ) {
        for element in container.elements() {
            match (element.namespace, element.name.as_str()) {
                (W, "r") => self.append_run(element, link, content, scan),
                (W, "hyperlink") => {
                    let target = self.hyperlink_target(element);
                    self.append_inlines(element, target.as_deref().or(link), content, scan);
                }
                (W, "ins" | "moveTo") => {
                    self.report.tracked_insertions += 1;
                    self.append_inlines(element, link, content, scan);
                }
                (W, "del" | "moveFrom") => self.report.tracked_deletions += 1,
                (W, "smartTag" | "customXml" | "dir" | "bdo") => {
                    self.append_inlines(element, link, content, scan);
                }
                (W, "sdt") => {
                    if let Some(inner) = element.child(W, "sdtContent") {
                        self.append_inlines(inner, link, content, scan);
                    }
                }
                (W, "fldSimple") => {
                    self.report.fields_kept_as_text += 1;
                    let field_link =
                        self.field_link(element.attribute(W, "instr").unwrap_or_default());
                    self.append_inlines(element, field_link.as_deref().or(link), content, scan);
                }
                (Namespace::Math, "oMath" | "oMathPara") => self.report.equations += 1,
                (MC, "AlternateContent") => {
                    if let Some(choice) = alternate_choice(element) {
                        self.append_inlines(choice, link, content, scan);
                    }
                }
                _ => {}
            }
        }
    }

    fn append_run(
        &mut self,
        run: &Element,
        link: Option<&str>,
        content: &mut Vec<Inline>,
        scan: &mut ParagraphScan,
    ) {
        let format = self.run_format(run.child(W, "rPr"));
        if format.hidden == Some(true) {
            self.report.hidden_runs += 1;
            return;
        }
        self.append_run_content(run, format, link, content, scan);
    }

    fn append_run_content(
        &mut self,
        container: &Element,
        format: RunFormat,
        link: Option<&str>,
        content: &mut Vec<Inline>,
        scan: &mut ParagraphScan,
    ) {
        for element in container.elements() {
            match (element.namespace, element.name.as_str()) {
                (W, "fldChar") => self.field_character(element),
                (W, "instrText") => {
                    if let Some(field) = self.fields.last_mut()
                        && !field.in_result
                        && field.instruction.len() < MAX_INSTRUCTION_CHARS
                    {
                        field.instruction.push_str(&element.text());
                    }
                }
                _ if self.in_field_code() => {}
                (W, "t") => self.push_text(content, &element.text(), format, link),
                (W, "tab" | "ptab") => self.push_text(content, "\t", format, link),
                (W, "noBreakHyphen") => self.push_text(content, "-", format, link),
                (W, "br") => match element.attribute(W, "type") {
                    Some("page" | "column") => scan.page_break = true,
                    _ => content.push(Inline::HardBreak),
                },
                (W, "cr") => content.push(Inline::HardBreak),
                (W, "footnoteReference") => self.report.footnotes += 1,
                (W, "endnoteReference") => self.report.endnotes += 1,
                (W, "commentReference") => self.report.comments += 1,
                (W, "drawing") => self.append_drawing(element, content),
                (W, "pict") => self.append_vml(element, content, scan),
                (W, "object") => self.report.embedded_objects += 1,
                (MC, "AlternateContent") => {
                    if let Some(choice) = alternate_choice(element) {
                        self.append_run_content(choice, format, link, content, scan);
                    }
                }
                _ => {}
            }
        }
    }

    /// A run's formatting: its character style, then its own properties over it.
    fn run_format(&self, properties: Option<&Element>) -> RunFormat {
        let Some(properties) = properties else {
            return RunFormat::default();
        };
        let style = properties
            .child(W, "rStyle")
            .and_then(|style| style.attribute(W, "val"))
            .map(|style_id| self.styles.character_format(style_id))
            .unwrap_or_default();
        style.overridden_by(RunFormat::parse(properties))
    }

    fn push_text(
        &mut self,
        content: &mut Vec<Inline>,
        text: &str,
        format: RunFormat,
        link: Option<&str>,
    ) {
        if text.is_empty() {
            return;
        }
        let total = text.chars().count();
        let taken: String = text.chars().take(self.remaining_chars).collect();
        let taken_count = total.min(self.remaining_chars);
        self.remaining_chars -= taken_count;
        if taken_count < total {
            self.report.truncated = true;
        }
        if taken.is_empty() {
            return;
        }

        let link = link.or_else(|| self.current_field_link());
        let marks = marks_of(format, link);
        if let Some(Inline::Text {
            text: previous,
            marks: previous_marks,
        }) = content.last_mut()
            && *previous_marks == marks
        {
            previous.push_str(&taken);
            return;
        }
        content.push(Inline::Text { text: taken, marks });
    }

    fn field_character(&mut self, element: &Element) {
        match element.attribute(W, "fldCharType") {
            Some("begin") => self.fields.push(OpenField {
                in_result: false,
                instruction: String::new(),
                link: None,
            }),
            Some("separate") => {
                let instruction = match self.fields.last() {
                    Some(field) if !field.in_result => field.instruction.clone(),
                    _ => return,
                };
                let link = self.field_link(&instruction);
                if let Some(field) = self.fields.last_mut() {
                    field.in_result = true;
                    field.link = link;
                }
            }
            Some("end") => {
                let closed_with_result = self.fields.pop().is_some_and(|field| field.in_result);
                self.report.fields_kept_as_text += usize::from(closed_with_result);
            }
            _ => {}
        }
    }

    /// Inside the instruction of a field, where runs are code, not text.
    fn in_field_code(&self) -> bool {
        self.fields.iter().any(|field| !field.in_result)
    }

    fn current_field_link(&self) -> Option<&str> {
        self.fields
            .iter()
            .rev()
            .find_map(|field| field.link.as_deref())
    }

    /// The accepted target of a `w:hyperlink`: an external relationship, plus its anchor.
    fn hyperlink_target(&mut self, hyperlink: &Element) -> Option<String> {
        let anchor = hyperlink.attribute(W, "anchor");
        let Some(id) = hyperlink.attribute(Namespace::Relationships, "id") else {
            if anchor.is_some() {
                self.report.unsupported_links += 1;
            }
            return None;
        };
        let target = self
            .relationships
            .get(id)
            .filter(|relationship| relationship.external)
            .map(|relationship| match anchor {
                Some(anchor) if !relationship.target.contains('#') => {
                    format!("{}#{anchor}", relationship.target)
                }
                _ => relationship.target.clone(),
            });
        self.accept_link(target.as_deref())
    }

    /// The target of a `HYPERLINK` field instruction, if it is one.
    fn field_link(&mut self, instruction: &str) -> Option<String> {
        let tokens = instruction_tokens(instruction);
        let (keyword, switches) = tokens.split_first()?;
        if !keyword.eq_ignore_ascii_case("HYPERLINK") {
            return None;
        }
        let mut target = None;
        let mut anchor = None;
        let mut position = 0;
        while let Some(token) = switches.get(position) {
            match token.as_str() {
                "\\l" => {
                    anchor = switches.get(position + 1);
                    position += 1;
                }
                "\\o" | "\\t" => position += 1,
                switch if switch.starts_with('\\') => {}
                url if target.is_none() => target = Some(url.to_owned()),
                _ => {}
            }
            position += 1;
        }
        let target = match (target, anchor) {
            (Some(target), Some(anchor)) => Some(format!("{target}#{anchor}")),
            (target, _) => target,
        };
        self.accept_link(target.as_deref())
    }

    /// Keep only `http`, `https` and `mailto` targets; anything else is counted and dropped.
    fn accept_link(&mut self, target: Option<&str>) -> Option<String> {
        let accepted = target.map(str::trim).filter(|target| {
            let lower = target.to_ascii_lowercase();
            lower.starts_with("http://")
                || lower.starts_with("https://")
                || lower.starts_with("mailto:")
        });
        if accepted.is_none() {
            self.report.unsupported_links += 1;
        }
        accepted.map(str::to_owned)
    }

    /// `w:drawing`: a picture becomes an image; charts, diagrams and shapes are counted.
    fn append_drawing(&mut self, drawing: &Element, content: &mut Vec<Inline>) {
        let Some(frame) = drawing.elements().find(|element| {
            element.namespace == Namespace::WordDrawing
                && matches!(element.name.as_str(), "inline" | "anchor")
        }) else {
            return;
        };
        let graphic_data = frame.descendant(Namespace::Drawing, "graphicData");
        let uri = graphic_data
            .and_then(|data| data.attribute(Namespace::None, "uri"))
            .unwrap_or_default();

        if uri.ends_with("/picture") {
            let embed = graphic_data
                .and_then(|data| data.descendant(Namespace::Drawing, "blip"))
                .and_then(|blip| blip.attribute(Namespace::Relationships, "embed"));
            let Some(index) = embed.and_then(|id| self.image(id)) else {
                if embed.is_none() {
                    self.report.missing_images += 1;
                }
                return;
            };
            let properties = frame.child(Namespace::WordDrawing, "docPr");
            let alt = properties.and_then(|properties| {
                [
                    properties.attribute(Namespace::None, "descr"),
                    properties.attribute(Namespace::None, "title"),
                ]
                .into_iter()
                .flatten()
                .map(str::trim)
                .find(|text| !text.is_empty())
                .map(str::to_owned)
            });
            let extent = frame.child(Namespace::WordDrawing, "extent");
            let pixels = |name: &str| {
                extent
                    .and_then(|extent| extent.attribute(Namespace::None, name))
                    .and_then(|emus| emus.parse::<u64>().ok())
                    .map(|emus| (emus + EMUS_PER_PIXEL / 2) / EMUS_PER_PIXEL)
                    .and_then(|pixels| u32::try_from(pixels).ok())
                    .filter(|pixels| *pixels > 0)
            };
            content.push(Inline::Image {
                media: index,
                alt,
                width: pixels("cx"),
                height: pixels("cy"),
            });
        } else if uri.contains("chart") {
            self.report.charts += 1;
        } else if uri.contains("/diagram") {
            self.report.smart_art += 1;
        } else if frame.descendant(W, "txbxContent").is_some() {
            self.report.text_boxes += 1;
        } else {
            self.report.shapes += 1;
        }
    }

    /// `w:pict`, the VML of older documents: an image, a horizontal rule, or a counted shape.
    fn append_vml(
        &mut self,
        picture: &Element,
        content: &mut Vec<Inline>,
        scan: &mut ParagraphScan,
    ) {
        if picture.descendant(W, "txbxContent").is_some() {
            self.report.text_boxes += 1;
            return;
        }
        let is_rule = picture
            .descendant(Namespace::Vml, "rect")
            .and_then(|rect| rect.attribute(Namespace::VmlOffice, "hr"))
            .is_some_and(|value| matches!(value, "t" | "true"));
        if is_rule {
            scan.horizontal_rule = true;
            return;
        }
        let Some(image_data) = picture.descendant(Namespace::Vml, "imagedata") else {
            self.report.shapes += 1;
            return;
        };
        let Some(id) = image_data.attribute(Namespace::Relationships, "id") else {
            self.report.missing_images += 1;
            return;
        };
        let Some(index) = self.image(id) else {
            return;
        };
        let style = picture
            .descendant(Namespace::Vml, "shape")
            .and_then(|shape| shape.attribute(Namespace::None, "style"))
            .unwrap_or_default();
        content.push(Inline::Image {
            media: index,
            alt: image_data
                .attribute(Namespace::VmlOffice, "title")
                .map(str::trim)
                .filter(|title| !title.is_empty())
                .map(str::to_owned),
            width: css_length_in_pixels(style, "width"),
            height: css_length_in_pixels(style, "height"),
        });
    }

    /// The media index of the image a relationship points at, loading it the first time.
    fn image(&mut self, relationship_id: &str) -> Option<usize> {
        let Some(target) = self
            .relationships
            .get(relationship_id)
            .filter(|relationship| !relationship.external)
            .map(|relationship| relationship.target.clone())
        else {
            self.report.missing_images += 1;
            return None;
        };
        if let Some(known) = self.media_by_target.get(&target) {
            return *known;
        }
        let loaded = self.load_image(&target);
        self.media_by_target.insert(target, loaded);
        loaded
    }

    fn load_image(&mut self, target: &str) -> Option<usize> {
        if self.media.len() >= self.limits.max_images {
            self.report.images_over_limit += 1;
            return None;
        }
        let cap = self
            .limits
            .max_image_bytes
            .min(self.limits.max_member_bytes);
        match read_member_bytes(self.package, target, cap) {
            Ok(Some(bytes)) => {
                let size = bytes.len() as u64;
                if self.media_bytes.saturating_add(size) > self.limits.max_total_image_bytes {
                    self.report.images_over_limit += 1;
                    return None;
                }
                self.media_bytes += size;
                self.media.push(Media {
                    content_type: sniff_image_type(&bytes),
                    bytes,
                });
                Some(self.media.len() - 1)
            }
            Err(OfficeError::TooLarge { .. }) => {
                self.report.oversized_images += 1;
                None
            }
            Ok(None) | Err(_) => {
                self.report.missing_images += 1;
                None
            }
        }
    }

    fn convert_table(&mut self, table: &Element) -> Block {
        let mut rows: Vec<TableRow> = Vec::new();
        // Grid column where a vertical merge started, to the row and cell that hold it.
        let mut merge_origins: HashMap<u32, (usize, usize)> = HashMap::new();

        for row in child_elements_through_wrappers(table, "tr") {
            if self.report.truncated {
                break;
            }
            let properties = row.child(W, "trPr");
            if properties
                .and_then(|properties| properties.child(W, "del"))
                .is_some()
            {
                self.report.tracked_deletions += 1;
                continue;
            }
            let header = properties
                .and_then(|properties| properties.child(W, "tblHeader"))
                .is_some_and(on_off);
            let mut grid_column = properties
                .and_then(|properties| properties.child(W, "gridBefore"))
                .and_then(|grid_before| grid_before.attribute(W, "val"))
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(0)
                .min(MAX_SPAN);
            let row_index = rows.len();
            let mut cells: Vec<TableCell> = Vec::new();

            for cell in child_elements_through_wrappers(row, "tc") {
                let cell_properties = cell.child(W, "tcPr");
                let col_span = cell_properties
                    .and_then(|properties| properties.child(W, "gridSpan"))
                    .and_then(|span| span.attribute(W, "val"))
                    .and_then(|value| value.parse::<u32>().ok())
                    .unwrap_or(1)
                    .clamp(1, MAX_SPAN);
                let vertical_merge = cell_properties
                    .and_then(|properties| properties.child(W, "vMerge"))
                    .map(|merge| merge.attribute(W, "val") == Some("restart"));

                if vertical_merge == Some(false)
                    && let Some(&(origin_row, origin_cell)) = merge_origins.get(&grid_column)
                    && let Some(origin) = rows
                        .get_mut(origin_row)
                        .and_then(|row| row.cells.get_mut(origin_cell))
                {
                    origin.row_span += 1;
                    grid_column = grid_column.saturating_add(col_span);
                    continue;
                }

                let blocks = self.convert_blocks(cell);
                let end_column = grid_column.saturating_add(col_span);
                merge_origins.retain(|column, _| *column < grid_column || *column >= end_column);
                if vertical_merge == Some(true) {
                    merge_origins.insert(grid_column, (row_index, cells.len()));
                }
                cells.push(TableCell {
                    col_span,
                    row_span: 1,
                    blocks,
                });
                grid_column = end_column;
            }
            rows.push(TableRow { header, cells });
        }

        Block::Table { rows }
    }
}

/// The elements of a table or row with this name, also when content controls or custom XML
/// wrap them.
fn child_elements_through_wrappers<'e>(parent: &'e Element, name: &str) -> Vec<&'e Element> {
    let mut found = Vec::new();
    for element in parent.elements() {
        if element.is(W, name) {
            found.push(element);
        } else if element.is(W, "sdt") {
            if let Some(content) = element.child(W, "sdtContent") {
                found.extend(child_elements_through_wrappers(content, name));
            }
        } else if element.is(W, "customXml") {
            found.extend(child_elements_through_wrappers(element, name));
        }
    }
    found
}

/// The branch of `mc:AlternateContent` to read: the first choice, else the fallback.
fn alternate_choice(element: &Element) -> Option<&Element> {
    element
        .child(MC, "Choice")
        .or_else(|| element.child(MC, "Fallback"))
}

/// Whether a subtree holds any text a reader would see.
fn has_text(element: &Element) -> bool {
    (element.is(W, "t") && !element.text().is_empty()) || element.elements().any(has_text)
}

/// Merge a code block into the code block before it, a quote into the quote before it.
fn push_block(blocks: &mut Vec<Block>, block: Block) {
    match block {
        Block::CodeBlock { text } => {
            if let Some(Block::CodeBlock { text: previous }) = blocks.last_mut() {
                previous.push('\n');
                previous.push_str(&text);
            } else {
                blocks.push(Block::CodeBlock { text });
            }
        }
        Block::Blockquote { blocks: quoted } => {
            if let Some(Block::Blockquote { blocks: previous }) = blocks.last_mut() {
                previous.extend(quoted);
            } else {
                blocks.push(Block::Blockquote { blocks: quoted });
            }
        }
        other => blocks.push(other),
    }
}

/// The text of inline content, line breaks as newlines; images have no text.
fn plain_text(content: &[Inline]) -> String {
    let mut text = String::new();
    for inline in content {
        match inline {
            Inline::Text { text: run, .. } => text.push_str(run),
            Inline::HardBreak => text.push('\n'),
            Inline::Image { .. } => {}
        }
    }
    text
}

/// The marks of a run; a link carries its own underline, so the run's is not repeated.
fn marks_of(format: RunFormat, link: Option<&str>) -> Vec<Mark> {
    let flags = [
        (format.bold, Mark::Bold),
        (format.italic, Mark::Italic),
        (format.underline.filter(|_| link.is_none()), Mark::Underline),
        (format.strike, Mark::Strike),
        (format.code, Mark::Code),
        (format.superscript, Mark::Superscript),
        (format.subscript, Mark::Subscript),
    ];
    let mut marks: Vec<Mark> = flags
        .into_iter()
        .filter(|(flag, _)| *flag == Some(true))
        .map(|(_, mark)| mark)
        .collect();
    if let Some(href) = link {
        marks.push(Mark::Link {
            href: href.to_owned(),
        });
    }
    marks
}

/// Split a field instruction into words, a quoted string being one word.
fn instruction_tokens(instruction: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for character in instruction.chars() {
        match character {
            '"' => {
                // A closing quote ends a word even when it is empty.
                if quoted || !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                quoted = !quoted;
            }
            whitespace if whitespace.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            other => current.push(other),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// A length of an inline CSS style (`width:120pt`) in CSS pixels; `pt` and `px` only.
fn css_length_in_pixels(style: &str, property: &str) -> Option<u32> {
    let value = style.split(';').find_map(|declaration| {
        let (name, value) = declaration.split_once(':')?;
        (name.trim() == property).then(|| value.trim())
    })?;
    // A point is four thirds of a CSS pixel.
    let (number, numerator, denominator) = value
        .strip_suffix("pt")
        .map(|points| (points, 4, 3))
        .or_else(|| value.strip_suffix("px").map(|pixels| (pixels, 1, 1)))?;
    let hundredths = decimal_hundredths(number.trim())?;
    let pixels = hundredths
        .checked_mul(numerator)?
        .checked_add(denominator * 50)?
        / (denominator * 100);
    u32::try_from(pixels).ok().filter(|pixels| *pixels > 0)
}

/// A non-negative decimal number in hundredths, the digits past the second decimal dropped.
fn decimal_hundredths(number: &str) -> Option<u64> {
    let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    if !whole
        .bytes()
        .chain(fraction.bytes())
        .all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let whole: u64 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let fraction = fraction
        .bytes()
        .chain(std::iter::repeat(b'0'))
        .take(2)
        .fold(0, |total, digit| total * 10 + u64::from(digit - b'0'));
    whole.checked_mul(100)?.checked_add(fraction)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_field_instructions_into_words() {
        assert_eq!(
            instruction_tokens(r#" HYPERLINK "https://a b" \o "Titolo" "#),
            vec!["HYPERLINK", "https://a b", "\\o", "Titolo"]
        );
        assert_eq!(instruction_tokens("PAGE"), vec!["PAGE"]);
        assert_eq!(instruction_tokens("   "), Vec::<String>::new());
    }

    #[test]
    fn reads_css_lengths_in_points_and_pixels() {
        assert_eq!(
            css_length_in_pixels("width:75pt;height:30px", "width"),
            Some(100)
        );
        assert_eq!(
            css_length_in_pixels("width:75pt;height:30px", "height"),
            Some(30)
        );
        assert_eq!(css_length_in_pixels("width:2in", "width"), None);
        assert_eq!(css_length_in_pixels("width:-5pt", "width"), None);
        assert_eq!(css_length_in_pixels("width:1e400pt", "width"), None);
        assert_eq!(css_length_in_pixels("", "width"), None);
        assert_eq!(css_length_in_pixels("width:.75pt", "width"), Some(1));
        assert_eq!(css_length_in_pixels("width:0px", "width"), None);
        assert_eq!(
            css_length_in_pixels("width:99999999999999999999pt", "width"),
            None
        );
        assert_eq!(decimal_hundredths("12.345"), Some(1234));
        assert_eq!(decimal_hundredths("."), None);
    }

    #[test]
    fn orders_marks_and_drops_the_underline_of_a_link() {
        let format = RunFormat {
            bold: Some(true),
            underline: Some(true),
            code: Some(false),
            ..RunFormat::default()
        };
        assert_eq!(marks_of(format, None), vec![Mark::Bold, Mark::Underline]);
        assert_eq!(
            marks_of(format, Some("https://x")),
            vec![
                Mark::Bold,
                Mark::Link {
                    href: "https://x".to_owned()
                }
            ]
        );
    }

    #[test]
    fn merges_consecutive_code_and_quote_blocks() {
        let mut blocks = Vec::new();
        push_block(
            &mut blocks,
            Block::CodeBlock {
                text: "a".to_owned(),
            },
        );
        push_block(
            &mut blocks,
            Block::CodeBlock {
                text: "b".to_owned(),
            },
        );
        push_block(&mut blocks, Block::HorizontalRule);
        push_block(
            &mut blocks,
            Block::Blockquote {
                blocks: vec![Block::HorizontalRule],
            },
        );
        push_block(
            &mut blocks,
            Block::Blockquote {
                blocks: vec![Block::HorizontalRule],
            },
        );
        assert_eq!(
            blocks,
            vec![
                Block::CodeBlock {
                    text: "a\nb".to_owned()
                },
                Block::HorizontalRule,
                Block::Blockquote {
                    blocks: vec![Block::HorizontalRule, Block::HorizontalRule]
                },
            ]
        );
    }
}
