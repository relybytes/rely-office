// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! From the element tree of `word/document.xml` to the neutral document tree.

use std::collections::HashMap;

use super::edges::CellLook;
use super::look::{PageField, ParagraphLook, RunLook, TableLook};
use super::notes::NoteParts;
use super::numbering::{ListCounters, Numbering};
use super::styles::{
    NumberingReference, ParagraphRole, RunFormat, Styles, TableFormat, TableRegion, alignment,
    numbering_reference, on_off, outline_level,
};
use super::{
    Block, ImportReport, Inline, Mark, Media, Note, NoteKind, TableCell, TableRow, sniff_image_type,
};
use crate::archive::{ZipPackage, read_member_bytes, read_text_member};
use crate::chart::{chart_picture, no_cells};
use crate::relationships::Relationship;
use crate::theme::Theme;
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
/// The share of the text limit a header, a footer or a note may take, so a part read after the
/// body never truncates the body.
const PART_TEXT_SHARE: usize = 10;

/// A complex field (`w:fldChar` begin, separate, end) still open.
#[derive(Debug)]
struct OpenField {
    /// Whether the separator has passed, so the runs are the field's result.
    in_result: bool,
    instruction: String,
    /// The target of a `HYPERLINK` field, applied to its result.
    link: Option<String>,
    /// The page number a `PAGE` or `NUMPAGES` field shows.
    page_field: Option<PageField>,
    /// Whether any of the result was read.
    shown: bool,
}

/// What a converter keeps while it reads one part, set aside while it reads another.
#[derive(Debug, Default)]
struct PartState {
    fields: Vec<OpenField>,
    hyperlink_depth: usize,
    pending_page_break: bool,
    floating: Vec<Block>,
    table_context: Option<ParagraphLook>,
}

/// Which conditional formats of a table style a table turns on, `w:tblLook`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is one switch of w:tblLook, which a table sets independently"
)]
struct TableFlags {
    first_row: bool,
    last_row: bool,
    first_column: bool,
    last_column: bool,
    banded_rows: bool,
    banded_columns: bool,
}

impl TableFlags {
    /// Read `w:tblLook`, by attribute or by the bits of its value; Word's own default without one.
    fn parse(properties: Option<&Element>) -> Self {
        let Some(table_look) = properties.and_then(|properties| properties.child(W, "tblLook"))
        else {
            return Self {
                first_row: true,
                last_row: false,
                first_column: true,
                last_column: false,
                banded_rows: true,
                banded_columns: false,
            };
        };
        let bits = table_look
            .attribute(W, "val")
            .and_then(|value| u32::from_str_radix(value, 16).ok())
            .unwrap_or(0);
        let flag = |name: &str, bit: u32| {
            table_look
                .attribute(W, name)
                .map_or(bits & bit != 0, |value| {
                    matches!(value, "1" | "true" | "on")
                })
        };
        Self {
            first_row: flag("firstRow", 0x0020),
            last_row: flag("lastRow", 0x0040),
            first_column: flag("firstColumn", 0x0080),
            last_column: flag("lastColumn", 0x0100),
            banded_rows: !flag("noHBand", 0x0200),
            banded_columns: !flag("noVBand", 0x0400),
        }
    }
}

/// What the runs of a paragraph said about the paragraph itself.
#[derive(Debug, Default)]
struct ParagraphScan {
    horizontal_rule: bool,
    /// A page break before the paragraph's first text.
    page_break_before: bool,
    /// A page break after some of the paragraph's text: the next paragraph starts the page.
    page_break_after: bool,
}

pub(super) struct Converter<'a, 'p> {
    package: &'a mut ZipPackage<'p>,
    /// The relationships of the part being read: the body, a header, a note.
    relationships: HashMap<&'a str, &'a Relationship>,
    styles: &'a Styles,
    numbering: &'a Numbering,
    note_parts: &'a NoteParts,
    limits: &'a Limits,
    report: &'a mut ImportReport,
    media: Vec<Media>,
    media_bytes: u64,
    media_by_target: HashMap<String, Option<usize>>,
    remaining_chars: usize,
    fields: Vec<OpenField>,
    /// How many `w:hyperlink` elements enclose the current run, a dropped link's included.
    hyperlink_depth: usize,
    /// A page break the next paragraph starts with.
    pending_page_break: bool,
    /// The content of the text boxes of the paragraph being read, placed after it.
    floating: Vec<Block>,
    /// The paragraph formatting a table style gives the cell being read.
    table_context: Option<ParagraphLook>,
    list_counters: ListCounters,
    notes: Vec<Note>,
    notes_by_id: HashMap<(NoteKind, String), usize>,
    reading_note: bool,
    /// The theme part the document's charts take their colours from, read on the first chart.
    theme_target: Option<String>,
    theme: Option<Theme>,
    /// The description of each chart image, by media index, taken as it is placed.
    media_titles: HashMap<usize, String>,
}

impl<'a, 'p> Converter<'a, 'p> {
    pub(super) fn new(
        package: &'a mut ZipPackage<'p>,
        relationships: &'a [Relationship],
        styles: &'a Styles,
        numbering: &'a Numbering,
        note_parts: &'a NoteParts,
        limits: &'a Limits,
        report: &'a mut ImportReport,
    ) -> Self {
        Self {
            package,
            relationships: relationship_map(relationships),
            styles,
            numbering,
            note_parts,
            limits,
            report,
            media: Vec::new(),
            media_bytes: 0,
            media_by_target: HashMap::new(),
            remaining_chars: limits.max_text_chars,
            fields: Vec::new(),
            hyperlink_depth: 0,
            pending_page_break: false,
            floating: Vec::new(),
            table_context: None,
            list_counters: ListCounters::default(),
            notes: Vec::new(),
            notes_by_id: HashMap::new(),
            reading_note: false,
            theme_target: relationships
                .iter()
                .find(|relationship| !relationship.external && relationship.is_kind("theme"))
                .map(|relationship| relationship.target.clone()),
            theme: None,
            media_titles: HashMap::new(),
        }
    }

    /// The images and the notes read, in the order the tree refers to them.
    pub(super) fn finish(self) -> (Vec<Media>, Vec<Note>) {
        (self.media, self.notes)
    }

    /// The blocks of another part, a header, a footer or a note, with its own relationships and
    /// a share of the text limit of its own.
    pub(super) fn convert_part(
        &mut self,
        container: &Element,
        relationships: &'a [Relationship],
    ) -> Vec<Block> {
        let outer_relationships =
            std::mem::replace(&mut self.relationships, relationship_map(relationships));
        let outer_state = self.take_state();
        let outer_chars = self.remaining_chars;
        let outer_truncated = self.report.truncated;
        self.remaining_chars = self.limits.max_text_chars / PART_TEXT_SHARE;
        self.report.truncated = false;

        let blocks = self.convert_blocks(container);

        self.report.truncated = outer_truncated;
        self.remaining_chars = outer_chars;
        self.restore_state(outer_state);
        self.relationships = outer_relationships;
        blocks
    }

    fn take_state(&mut self) -> PartState {
        PartState {
            fields: std::mem::take(&mut self.fields),
            hyperlink_depth: std::mem::take(&mut self.hyperlink_depth),
            pending_page_break: std::mem::take(&mut self.pending_page_break),
            floating: std::mem::take(&mut self.floating),
            table_context: self.table_context.take(),
        }
    }

    fn restore_state(&mut self, state: PartState) {
        self.fields = state.fields;
        self.hyperlink_depth = state.hyperlink_depth;
        self.pending_page_break = state.pending_page_break;
        self.floating = state.floating;
        self.table_context = state.table_context;
    }

    /// The index of a note in the tree, read the first time the text refers to it; a note
    /// referred to from inside a note is not followed.
    fn note(&mut self, kind: NoteKind, id: &str) -> Option<usize> {
        if let Some(index) = self.notes_by_id.get(&(kind, id.to_owned())) {
            return Some(*index);
        }
        if self.reading_note {
            return None;
        }
        let (element, relationships) = self.note_parts.find(kind, id)?;
        self.reading_note = true;
        let blocks = self.convert_part(element, relationships);
        self.reading_note = false;
        self.notes.push(Note { kind, blocks });
        let index = self.notes.len() - 1;
        self.notes_by_id.insert((kind, id.to_owned()), index);
        Some(index)
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

    /// A paragraph, then the content of its text boxes.
    fn append_paragraph(&mut self, paragraph: &Element, blocks: &mut Vec<Block>) {
        self.append_paragraph_block(paragraph, blocks);
        let floating = std::mem::take(&mut self.floating);
        blocks.extend(floating);
    }

    fn append_paragraph_block(&mut self, paragraph: &Element, blocks: &mut Vec<Block>) {
        let properties = paragraph.child(W, "pPr");
        let style_id = properties
            .and_then(|properties| properties.child(W, "pStyle"))
            .and_then(|style| style.attribute(W, "val"));
        let style = self.styles.paragraph_style(style_id);
        let outline = properties.and_then(outline_level).or(style.outline_level);
        let numbering = properties.and_then(numbering_reference).or(style.numbering);
        let align = properties.and_then(alignment).or(style.alignment);
        let list_look = numbering
            .map(|reference| {
                self.numbering
                    .level_look(reference.num_id, reference.level.unwrap_or(0))
            })
            .unwrap_or_default();
        let style_look = match &self.table_context {
            Some(table) => self.styles.paragraph_look_in_table(style_id, table),
            None => self.styles.paragraph_look(style_id),
        };
        let mut look = properties
            .map(ParagraphLook::parse)
            .unwrap_or_default()
            .or(list_look)
            .or(style_look);

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
        let rule_style = style.role == ParagraphRole::HorizontalRule;
        if blank && (scan.horizontal_rule || bottom_border || rule_style) {
            blocks.push(Block::HorizontalRule);
            return;
        }
        if blank && (scan.page_break_before || scan.page_break_after) {
            self.pending_page_break = true;
            return;
        }
        look.page_break_before |=
            std::mem::take(&mut self.pending_page_break) || scan.page_break_before;
        self.pending_page_break = scan.page_break_after;
        if style.role != ParagraphRole::Code
            && let Some(reference) = numbering
        {
            look.list_label = self.numbering.next_label(
                &mut self.list_counters,
                reference.num_id,
                reference.level.unwrap_or(0),
            );
        }

        let block = match style.role {
            ParagraphRole::Code => Block::CodeBlock {
                text: plain_text(&content),
            },
            ParagraphRole::Heading(level) => Block::Heading {
                level: level.min(MAX_HEADING_LEVEL),
                content,
                align,
                look,
            },
            _ if outline.is_some() => Block::Heading {
                level: outline
                    .unwrap_or(0)
                    .saturating_add(1)
                    .min(MAX_HEADING_LEVEL),
                content,
                align,
                look,
            },
            role => self.body_block(role, numbering, content, align, look),
        };
        push_block(blocks, block);
    }

    /// A paragraph that is not a heading or code: a list item, a quote or a plain paragraph.
    fn body_block(
        &self,
        role: ParagraphRole,
        numbering: Option<NumberingReference>,
        content: Vec<Inline>,
        align: Option<super::Alignment>,
        look: ParagraphLook,
    ) -> Block {
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
                look,
            },
            (None, ParagraphRole::Quote) => Block::Blockquote {
                blocks: vec![Block::Paragraph {
                    content,
                    align,
                    look,
                }],
            },
            (None, _) => Block::Paragraph {
                content,
                align,
                look,
            },
        }
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
                    self.hyperlink_depth += 1;
                    self.append_inlines(element, target.as_deref().or(link), content, scan);
                    self.hyperlink_depth -= 1;
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
                    let instruction = element.attribute(W, "instr").unwrap_or_default();
                    let field_link = self.field_link(instruction);
                    self.fields.push(OpenField {
                        in_result: true,
                        instruction: String::new(),
                        link: None,
                        page_field: page_field(instruction),
                        shown: false,
                    });
                    self.append_inlines(element, field_link.as_deref().or(link), content, scan);
                    if let Some(field) = self.fields.pop()
                        && let Some(page_field) = field.page_field.filter(|_| !field.shown)
                    {
                        self.push_text(
                            content,
                            "1",
                            RunFormat::default(),
                            &with_field(&RunLook::default(), page_field),
                            link,
                        );
                    }
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
        let look = self.run_look(run.child(W, "rPr"));
        self.append_run_content(run, format, &look, link, content, scan);
    }

    fn append_run_content(
        &mut self,
        container: &Element,
        format: RunFormat,
        look: &RunLook,
        link: Option<&str>,
        content: &mut Vec<Inline>,
        scan: &mut ParagraphScan,
    ) {
        for element in container.elements() {
            match (element.namespace, element.name.as_str()) {
                (W, "fldChar") => {
                    if let Some(page_field) = self.field_character(element) {
                        self.push_text(content, "1", format, &with_field(look, page_field), link);
                    }
                }
                (W, "instrText") => {
                    if let Some(field) = self.fields.last_mut()
                        && !field.in_result
                        && field.instruction.len() < MAX_INSTRUCTION_CHARS
                    {
                        field.instruction.push_str(&element.text());
                    }
                }
                _ if self.in_field_code() => {}
                (W, "t") => self.push_text(content, &element.text(), format, look, link),
                (W, "tab" | "ptab") => self.push_text(content, "\t", format, look, link),
                (W, "noBreakHyphen") => self.push_text(content, "-", format, look, link),
                (W, "br") => match element.attribute(W, "type") {
                    Some("page" | "column") if content.is_empty() => {
                        scan.page_break_before = true;
                    }
                    Some("page" | "column") => scan.page_break_after = true,
                    _ => content.push(Inline::HardBreak),
                },
                (W, "cr") => content.push(Inline::HardBreak),
                (W, name @ ("footnoteReference" | "endnoteReference")) => {
                    let kind = if name == "footnoteReference" {
                        self.report.footnotes += 1;
                        NoteKind::Footnote
                    } else {
                        self.report.endnotes += 1;
                        NoteKind::Endnote
                    };
                    if let Some(note) = element
                        .attribute(W, "id")
                        .and_then(|id| self.note(kind, id))
                    {
                        content.push(Inline::NoteReference { note });
                    }
                }
                (W, "commentReference") => self.report.comments += 1,
                (W, "drawing") => self.append_drawing(element, content),
                (W, "pict") => self.append_vml(element, content, scan),
                (W, "object") => self.report.embedded_objects += 1,
                (MC, "AlternateContent") => {
                    if let Some(choice) = alternate_choice(element) {
                        self.append_run_content(choice, format, look, link, content, scan);
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

    /// A run's font: its own properties over its character style.
    fn run_look(&self, properties: Option<&Element>) -> RunLook {
        let Some(properties) = properties else {
            return RunLook::default();
        };
        let style = properties
            .child(W, "rStyle")
            .and_then(|style| style.attribute(W, "val"))
            .map(|style_id| self.styles.character_look(style_id))
            .unwrap_or_default();
        // Bold and italic are marks of the run already.
        RunLook {
            bold: None,
            italic: None,
            ..RunLook::parse(properties).or(style)
        }
    }

    fn push_text(
        &mut self,
        content: &mut Vec<Inline>,
        text: &str,
        format: RunFormat,
        look: &RunLook,
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

        let page_field = self.fields.iter_mut().rev().find_map(|field| {
            field.shown |= field.page_field.is_some();
            field.page_field
        });
        let field_look;
        let look = match page_field {
            Some(page_field) if look.field.is_none() => {
                field_look = with_field(look, page_field);
                &field_look
            }
            _ => look,
        };
        let link = link.or_else(|| self.current_field_link());
        let marks = marks_of(format, look, link, self.hyperlink_depth > 0);
        if let Some(Inline::Text {
            text: previous,
            marks: previous_marks,
            look: previous_look,
        }) = content.last_mut()
            && *previous_marks == marks
            && previous_look == look
        {
            previous.push_str(&taken);
            return;
        }
        content.push(Inline::Text {
            text: taken,
            marks,
            look: look.clone(),
        });
    }

    /// Follow a `w:fldChar`. A page number field that ends without a result of its own gives
    /// its kind, so a number can stand in for it.
    fn field_character(&mut self, element: &Element) -> Option<PageField> {
        match element.attribute(W, "fldCharType") {
            Some("begin") => self.fields.push(OpenField {
                in_result: false,
                instruction: String::new(),
                link: None,
                page_field: None,
                shown: false,
            }),
            Some("separate") => {
                let instruction = match self.fields.last() {
                    Some(field) if !field.in_result => field.instruction.clone(),
                    _ => return None,
                };
                let link = self.field_link(&instruction);
                if let Some(field) = self.fields.last_mut() {
                    field.in_result = true;
                    field.link = link;
                    field.page_field = page_field(&instruction);
                }
            }
            Some("end") => {
                let field = self.fields.pop()?;
                self.report.fields_kept_as_text += usize::from(field.in_result);
                let page_field = field
                    .page_field
                    .or_else(|| page_field(&field.instruction).filter(|_| !field.in_result));
                return page_field.filter(|_| !field.shown && !self.in_field_code());
            }
            _ => {}
        }
        None
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
            let extent = frame.child(Namespace::WordDrawing, "extent");
            let emus = |name: &str| {
                extent
                    .and_then(|extent| extent.attribute(Namespace::None, name))
                    .and_then(|emus| emus.parse::<i64>().ok())
                    .filter(|emus| *emus > 0)
            };
            let (Some(width), Some(height)) = (emus("cx"), emus("cy")) else {
                return;
            };
            let Some(index) = graphic_data
                .and_then(|data| data.descendant(Namespace::Chart, "chart"))
                .and_then(|chart| chart.attribute(Namespace::Relationships, "id"))
                .and_then(|id| self.chart(id, width, height))
            else {
                return;
            };
            let pixels = |emus: i64| {
                u32::try_from((emus.unsigned_abs() + EMUS_PER_PIXEL / 2) / EMUS_PER_PIXEL).ok()
            };
            let alt = self.media_titles.remove(&index);
            content.push(Inline::Image {
                media: index,
                alt,
                width: pixels(width),
                height: pixels(height),
            });
        } else if uri.contains("/diagram") {
            self.report.smart_art += 1;
        } else if let Some(text_box) = frame.descendant(W, "txbxContent") {
            self.append_text_box(text_box);
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
        if let Some(text_box) = picture.descendant(W, "txbxContent") {
            self.append_text_box(text_box);
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

    /// Read the content of a text box, to be placed after the paragraph that anchors it. Its
    /// fields and links are its own, and a text box inside it is read in turn.
    fn append_text_box(&mut self, text_box: &Element) {
        self.report.text_boxes += 1;
        let outer_floating = std::mem::take(&mut self.floating);
        let outer_fields = std::mem::take(&mut self.fields);
        let outer_depth = std::mem::take(&mut self.hyperlink_depth);
        let blocks = self.convert_blocks(text_box);
        self.hyperlink_depth = outer_depth;
        self.fields = outer_fields;
        self.floating = outer_floating;
        self.floating.extend(blocks);
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

    /// A chart part drawn as an SVG image, its title kept for the image's description.
    fn chart(&mut self, relationship_id: &str, width_emu: i64, height_emu: i64) -> Option<usize> {
        let target = self
            .relationships
            .get(relationship_id)
            .filter(|relationship| !relationship.external)
            .map(|relationship| relationship.target.clone())?;
        if self.theme.is_none() {
            let theme = self
                .theme_target
                .clone()
                .and_then(|target| {
                    read_text_member(self.package, &target, self.limits)
                        .ok()
                        .flatten()
                })
                .map(|xml| Theme::parse(&xml))
                .unwrap_or_default();
            self.theme = Some(theme);
        }
        let xml = read_text_member(self.package, &target, self.limits)
            .ok()
            .flatten()?;
        let theme = self.theme.as_ref()?;
        let (bytes, title) = chart_picture(&xml, theme, width_emu, height_emu, &no_cells)?;
        if self.media.len() >= self.limits.max_images {
            self.report.images_over_limit += 1;
            return None;
        }
        let size = bytes.len() as u64;
        if self.media_bytes.saturating_add(size) > self.limits.max_total_image_bytes {
            self.report.images_over_limit += 1;
            return None;
        }
        self.media_bytes += size;
        self.media.push(Media {
            content_type: "image/svg+xml",
            bytes,
        });
        let index = self.media.len() - 1;
        self.media_titles
            .insert(index, title.unwrap_or_else(|| "Grafico".to_owned()));
        Some(index)
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
        let mut look = TableLook::parse(table);
        let table_properties = table.child(W, "tblPr");
        let style_id = table_properties
            .and_then(|properties| properties.child(W, "tblStyle"))
            .and_then(|style| style.attribute(W, "val"));
        let style = self.styles.table_style(style_id).unwrap_or_default();
        look.edges = look.edges.or(style.edges.clone());
        look.borders |= style.edges.any_visible();
        for side in 0..4 {
            look.cell_margins_twips[side] =
                look.cell_margins_twips[side].or(style.cell_margins_twips[side]);
        }
        let flags = TableFlags::parse(table_properties);
        let kept_rows: Vec<&Element> = child_elements_through_wrappers(table, "tr")
            .into_iter()
            .filter(|row| {
                let deleted = row
                    .child(W, "trPr")
                    .and_then(|properties| properties.child(W, "del"))
                    .is_some();
                self.report.tracked_deletions += usize::from(deleted);
                !deleted
            })
            .collect();
        let row_count = kept_rows.len();
        let grid_columns = u32::try_from(look.column_widths_twips.len()).unwrap_or(0);

        let mut rows: Vec<TableRow> = Vec::new();
        // Grid column where a vertical merge started, to the row and cell that hold it.
        let mut merge_origins: HashMap<u32, (usize, usize)> = HashMap::new();
        let outer_context = self.table_context.take();

        for row in kept_rows {
            if self.report.truncated {
                break;
            }
            let properties = row.child(W, "trPr");
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

                let end_column = grid_column.saturating_add(col_span);
                let place = CellPlace {
                    row: row_index,
                    rows: row_count,
                    first_column: grid_column,
                    end_column,
                    columns: grid_columns,
                };
                let format = region_format(&style, flags, place);
                self.table_context = Some(format.paragraph);
                let blocks = self.convert_blocks(cell);
                merge_origins.retain(|column, _| *column < grid_column || *column >= end_column);
                if vertical_merge == Some(true) {
                    merge_origins.insert(grid_column, (row_index, cells.len()));
                }
                cells.push(TableCell {
                    col_span,
                    row_span: 1,
                    blocks,
                    look: cell_properties
                        .map(CellLook::parse)
                        .unwrap_or_default()
                        .or(format.cell),
                });
                grid_column = end_column;
            }
            rows.push(TableRow { header, cells });
        }
        self.table_context = outer_context;

        Block::Table { rows, look }
    }
}

/// Where a cell sits in its table, in rows and grid columns.
#[derive(Debug, Clone, Copy)]
struct CellPlace {
    row: usize,
    rows: usize,
    first_column: u32,
    /// The grid column after the cell's last.
    end_column: u32,
    /// The columns of the grid; 0 when the table declares no grid.
    columns: u32,
}

/// What a table style gives a cell: the conditional formats that cover it, the strongest first,
/// over the style's own formatting.
fn region_format(
    style: &super::styles::TableStyle,
    flags: TableFlags,
    place: CellPlace,
) -> TableFormat {
    let header_rows = usize::from(flags.first_row);
    let body_row = place.row.checked_sub(header_rows);
    let in_last_row = flags.last_row && place.row + 1 == place.rows && place.rows > 1;
    let row_band = body_row
        .filter(|_| flags.banded_rows && !in_last_row)
        .map(|row| {
            if row % 2 == 0 {
                TableRegion::OddRow
            } else {
                TableRegion::EvenRow
            }
        });
    let header_columns = u32::from(flags.first_column);
    let column_band = place
        .first_column
        .checked_sub(header_columns)
        .filter(|_| flags.banded_columns)
        .map(|column| {
            if column % 2 == 0 {
                TableRegion::OddColumn
            } else {
                TableRegion::EvenColumn
            }
        });
    let regions = [
        (flags.first_row && place.row == 0).then_some(TableRegion::FirstRow),
        in_last_row.then_some(TableRegion::LastRow),
        (flags.first_column && place.first_column == 0).then_some(TableRegion::FirstColumn),
        (flags.last_column && place.columns > 0 && place.end_column >= place.columns)
            .then_some(TableRegion::LastColumn),
        row_band,
        column_band,
    ];
    let mut format = TableFormat::default();
    for region in regions.into_iter().flatten() {
        if let Some((_, conditional)) = style.regions.iter().find(|(known, _)| *known == region) {
            format = format.or(conditional.clone());
        }
    }
    format.or(style.whole.clone())
}

fn relationship_map(relationships: &[Relationship]) -> HashMap<&str, &Relationship> {
    relationships
        .iter()
        .map(|relationship| (relationship.id.as_str(), relationship))
        .collect()
}

/// The page number a field instruction shows, if it is `PAGE`, `NUMPAGES` or `SECTIONPAGES`.
fn page_field(instruction: &str) -> Option<PageField> {
    let keyword = instruction.split_whitespace().next()?;
    if keyword.eq_ignore_ascii_case("PAGE") {
        Some(PageField::Page)
    } else if keyword.eq_ignore_ascii_case("NUMPAGES")
        || keyword.eq_ignore_ascii_case("SECTIONPAGES")
    {
        Some(PageField::Pages)
    } else {
        None
    }
}

fn with_field(look: &RunLook, field: PageField) -> RunLook {
    RunLook {
        field: Some(field),
        ..look.clone()
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
            Inline::Image { .. } | Inline::NoteReference { .. } => {}
        }
    }
    text
}

/// The marks of a run. A link carries its own underline and colour, so the run's are not
/// repeated; nor is the underline kept on a link that was dropped, where it only said it was a
/// link. A code run keeps no font of its own.
fn marks_of(
    format: RunFormat,
    look: &RunLook,
    link: Option<&str>,
    in_hyperlink: bool,
) -> Vec<Mark> {
    let flags = [
        (format.bold, Mark::Bold),
        (format.italic, Mark::Italic),
        (
            format.underline.filter(|_| link.is_none() && !in_hyperlink),
            Mark::Underline,
        ),
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
    let code = format.code == Some(true);
    let font_family = look.family.clone().filter(|_| !code);
    let font_size = look.size_half_points.map(|size| {
        if size % 2 == 0 {
            format!("{}pt", size / 2)
        } else {
            format!("{}.5pt", size / 2)
        }
    });
    let color = look
        .colour
        .as_ref()
        .filter(|_| link.is_none() && !in_hyperlink)
        .map(|colour| format!("#{colour}"));
    if !code && (font_family.is_some() || font_size.is_some() || color.is_some()) {
        marks.push(Mark::TextStyle {
            font_family,
            font_size,
            color,
        });
    }
    if let Some(highlight) = &look.highlight {
        marks.push(Mark::Highlight {
            color: format!("#{highlight}"),
        });
    }
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
        assert_eq!(
            marks_of(format, &RunLook::default(), None, false),
            vec![Mark::Bold, Mark::Underline]
        );
        assert_eq!(
            marks_of(format, &RunLook::default(), None, true),
            vec![Mark::Bold]
        );
        assert_eq!(
            marks_of(format, &RunLook::default(), Some("https://x"), false),
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
