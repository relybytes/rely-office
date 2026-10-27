// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! A document tree written out as a docx: the inverse of [`super::read_document_tree`], so an
//! editor's document can leave as a file Word opens and this crate reads back to the same tree.
//!
//! The package carries Word's built-in styles by their English names (`Heading 1`, `Quote`,
//! `List Paragraph`, `Hyperlink`, `Table Grid`), a bullet and a numbered list definition with one
//! instance per list of the tree, and the page of the tree's layout.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;

use super::{Alignment, Block, DocumentTree, Inline, Mark, Media, TableRow};
use crate::OfficeError;

const EMUS_PER_PIXEL: u64 = 9_525;
/// The deepest list level Word numbers.
const MAX_LIST_LEVEL: u8 = 8;
const MAX_HEADING_LEVEL: u8 = 6;
const RELATIONSHIP_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NAMESPACES: &str = concat!(
    r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" "#,
    r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#,
    r#"xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" "#,
    r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
    r#"xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture""#,
);

/// Write a document tree as a docx. An image whose type is not one Word shows (PNG, JPEG, GIF,
/// BMP, TIFF, WebP, SVG) is left out; its alternative text, if any, stays as text.
///
/// # Errors
///
/// [`OfficeError::DamagedContainer`] when the zip cannot be written, which only a failing
/// allocator causes.
pub fn write_docx(tree: &DocumentTree) -> Result<Vec<u8>, OfficeError> {
    let mut writer = DocxWriter {
        media: &tree.media,
        relationships: Vec::new(),
        image_parts: BTreeMap::new(),
        lists: HashMap::new(),
        next_drawing_id: 1,
        text_width_twips: text_width(tree),
    };
    let mut body = String::new();
    writer.blocks(&tree.blocks, None, &mut body);
    let _ = write!(body, "{}", section(tree));

    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {NAMESPACES}><w:body>{body}</w:body></w:document>"#
    );
    let mut lists: Vec<(u32, bool, u32)> = writer
        .lists
        .iter()
        .map(|(&(list_id, ordered), &num_id)| (num_id, ordered, list_id))
        .collect();
    lists.sort_unstable();

    let mut package = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default();
    let mut add = |name: &str, bytes: &[u8]| -> Result<(), OfficeError> {
        package
            .start_file(name, options)
            .map_err(|error| OfficeError::DamagedContainer(error.to_string()))?;
        package
            .write_all(bytes)
            .map_err(|error| OfficeError::DamagedContainer(error.to_string()))
    };
    add("[Content_Types].xml", content_types().as_bytes())?;
    add("_rels/.rels", package_relationships().as_bytes())?;
    add("word/document.xml", document.as_bytes())?;
    add("word/styles.xml", STYLES.as_bytes())?;
    add("word/numbering.xml", numbering(&lists).as_bytes())?;
    add(
        "word/_rels/document.xml.rels",
        document_relationships(&writer.relationships).as_bytes(),
    )?;
    for (index, (part, _)) in &writer.image_parts {
        add(&format!("word/{part}"), &tree.media[*index].bytes)?;
    }
    package
        .finish()
        .map(Cursor::into_inner)
        .map_err(|error| OfficeError::DamagedContainer(error.to_string()))
}

/// A relationship of the main part: id, type name, target, whether external.
struct Relationship {
    id: String,
    kind: &'static str,
    target: String,
    external: bool,
}

struct DocxWriter<'a> {
    media: &'a [Media],
    relationships: Vec<Relationship>,
    /// Media index to its part path and relationship id.
    image_parts: BTreeMap<usize, (String, String)>,
    /// `(list id, ordered)` of the tree to the `w:num` id written for it.
    lists: HashMap<(u32, bool), u32>,
    next_drawing_id: u32,
    text_width_twips: u32,
}

impl DocxWriter<'_> {
    /// Blocks as body XML; `paragraph_style` names the style of plain paragraphs inside a quote.
    fn blocks(&mut self, blocks: &[Block], paragraph_style: Option<&str>, xml: &mut String) {
        for block in blocks {
            match block {
                Block::Heading {
                    level,
                    content,
                    align,
                    ..
                } => {
                    let style = format!("Heading{}", (*level).clamp(1, MAX_HEADING_LEVEL));
                    self.paragraph(xml, Some(&style), *align, None, content);
                }
                Block::Paragraph { content, align, .. } => {
                    self.paragraph(xml, paragraph_style, *align, None, content);
                }
                Block::ListItem {
                    ordered,
                    level,
                    list_id,
                    content,
                    ..
                } => {
                    let next = u32::try_from(self.lists.len()).unwrap_or(u32::MAX - 1) + 1;
                    let num_id = *self.lists.entry((*list_id, *ordered)).or_insert(next);
                    let level = (*level).min(MAX_LIST_LEVEL);
                    self.paragraph(
                        xml,
                        Some("ListParagraph"),
                        None,
                        Some((num_id, level)),
                        content,
                    );
                }
                Block::Table { rows, .. } => self.table(rows, xml),
                Block::Blockquote { blocks } => self.blocks(blocks, Some("Quote"), xml),
                Block::CodeBlock { text } => {
                    for line in text.split('\n') {
                        xml.push_str(r#"<w:p><w:pPr><w:pStyle w:val="Code"/></w:pPr>"#);
                        push_text_runs(xml, line, "");
                        xml.push_str("</w:p>");
                    }
                }
                Block::HorizontalRule => xml.push_str(concat!(
                    r#"<w:p><w:pPr><w:pBdr><w:bottom w:val="single" w:sz="6" w:space="1" w:color="auto"/>"#,
                    "</w:pBdr></w:pPr></w:p>"
                )),
            }
        }
    }

    fn paragraph(
        &mut self,
        xml: &mut String,
        style: Option<&str>,
        align: Option<Alignment>,
        list: Option<(u32, u8)>,
        content: &[Inline],
    ) {
        xml.push_str("<w:p>");
        let alignment = align.map(|align| match align {
            Alignment::Left => "left",
            Alignment::Center => "center",
            Alignment::Right => "right",
            Alignment::Justify => "both",
        });
        if style.is_some() || alignment.is_some() || list.is_some() {
            xml.push_str("<w:pPr>");
            if let Some(style) = style {
                let _ = write!(xml, r#"<w:pStyle w:val="{style}"/>"#);
            }
            if let Some((num_id, level)) = list {
                let _ = write!(
                    xml,
                    r#"<w:numPr><w:ilvl w:val="{level}"/><w:numId w:val="{num_id}"/></w:numPr>"#
                );
            }
            if let Some(alignment) = alignment {
                let _ = write!(xml, r#"<w:jc w:val="{alignment}"/>"#);
            }
            xml.push_str("</w:pPr>");
        }
        self.inlines(content, xml);
        xml.push_str("</w:p>");
    }

    fn inlines(&mut self, content: &[Inline], xml: &mut String) {
        for inline in content {
            match inline {
                Inline::Text { text, marks, .. } => self.text_run(text, marks, xml),
                Inline::HardBreak => xml.push_str("<w:r><w:br/></w:r>"),
                Inline::Image {
                    media,
                    alt,
                    width,
                    height,
                } => self.image(*media, alt.as_deref(), *width, *height, xml),
            }
        }
    }

    fn text_run(&mut self, text: &str, marks: &[Mark], xml: &mut String) {
        let mut properties = String::new();
        let mut link = None;
        for mark in marks {
            match mark {
                Mark::Link { href } => link = Some(href.as_str()),
                Mark::Code => properties.push_str(r#"<w:rStyle w:val="CodeChar"/>"#),
                _ => {}
            }
        }
        if link.is_some() && !properties.contains("rStyle") {
            properties.push_str(r#"<w:rStyle w:val="Hyperlink"/>"#);
        }
        // Word wants the run properties in schema order.
        for (mark, element) in [
            (Mark::Bold, "<w:b/>"),
            (Mark::Italic, "<w:i/>"),
            (Mark::Strike, "<w:strike/>"),
            (Mark::Underline, r#"<w:u w:val="single"/>"#),
            (Mark::Superscript, r#"<w:vertAlign w:val="superscript"/>"#),
            (Mark::Subscript, r#"<w:vertAlign w:val="subscript"/>"#),
        ] {
            if marks.contains(&mark) {
                properties.push_str(element);
            }
        }
        let run_properties = if properties.is_empty() {
            String::new()
        } else {
            format!("<w:rPr>{properties}</w:rPr>")
        };
        match link {
            Some(href) => {
                let id = self.relationship("hyperlink", href.to_owned(), true);
                let _ = write!(xml, r#"<w:hyperlink r:id="{id}" w:history="1">"#);
                push_text_runs(xml, text, &run_properties);
                xml.push_str("</w:hyperlink>");
            }
            None => push_text_runs(xml, text, &run_properties),
        }
    }

    fn image(
        &mut self,
        media: usize,
        alt: Option<&str>,
        width: Option<u32>,
        height: Option<u32>,
        xml: &mut String,
    ) {
        let Some(item) = self.media.get(media) else {
            return;
        };
        let Some(extension) = image_extension(item.content_type) else {
            if let Some(alt) = alt.filter(|alt| !alt.is_empty()) {
                push_text_runs(xml, alt, "");
            }
            return;
        };
        let relationship_id = if let Some((_, id)) = self.image_parts.get(&media) {
            id.clone()
        } else {
            let part = format!("media/image{}.{extension}", self.image_parts.len() + 1);
            let id = self.relationship("image", part.clone(), false);
            self.image_parts.insert(media, (part, id.clone()));
            id
        };
        // A width larger than the text is cut to it, the proportions kept.
        let max_pixels = u64::from(self.text_width_twips) * 96 / 1_440;
        let width = u64::from(width.unwrap_or(300).max(1));
        let height = u64::from(height.unwrap_or(200).max(1));
        let (width, height) = if width > max_pixels && max_pixels > 0 {
            (max_pixels, (height * max_pixels / width).max(1))
        } else {
            (width, height)
        };
        let (cx, cy) = (width * EMUS_PER_PIXEL, height * EMUS_PER_PIXEL);
        let drawing_id = self.next_drawing_id;
        self.next_drawing_id += 1;
        let description = escape(alt.unwrap_or_default());
        let _ = write!(
            xml,
            concat!(
                r#"<w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0">"#,
                r#"<wp:extent cx="{cx}" cy="{cy}"/><wp:docPr id="{id}" name="Picture {id}" descr="{descr}"/>"#,
                r#"<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">"#,
                r#"<pic:pic><pic:nvPicPr><pic:cNvPr id="{id}" name="Picture {id}" descr="{descr}"/><pic:cNvPicPr/></pic:nvPicPr>"#,
                r#"<pic:blipFill><a:blip r:embed="{rel}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>"#,
                r#"<pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>"#,
                r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic>"#,
                r#"</a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#
            ),
            cx = cx,
            cy = cy,
            id = drawing_id,
            descr = description,
            rel = relationship_id,
        );
    }

    fn table(&mut self, rows: &[TableRow], xml: &mut String) {
        let placed = place_cells(rows);
        let columns = placed.columns.max(1);
        let column_width = self.text_width_twips / u32::try_from(columns).unwrap_or(1).max(1);
        xml.push_str(concat!(
            r#"<w:tbl><w:tblPr><w:tblStyle w:val="TableGrid"/><w:tblW w:w="5000" w:type="pct"/>"#,
            r#"<w:tblLook w:val="04A0" w:firstRow="1" w:lastRow="0" w:firstColumn="1" w:lastColumn="0" w:noHBand="0" w:noVBand="1"/>"#,
            "</w:tblPr><w:tblGrid>"
        ));
        for _ in 0..columns {
            let _ = write!(xml, r#"<w:gridCol w:w="{column_width}"/>"#);
        }
        xml.push_str("</w:tblGrid>");
        for (row_index, row) in rows.iter().enumerate() {
            xml.push_str("<w:tr>");
            if row.header {
                xml.push_str("<w:trPr><w:tblHeader/></w:trPr>");
            }
            for slot in &placed.rows[row_index] {
                let span = slot.col_span;
                let width = column_width * span;
                xml.push_str("<w:tc><w:tcPr>");
                let _ = write!(xml, r#"<w:tcW w:w="{width}" w:type="dxa"/>"#);
                if span > 1 {
                    let _ = write!(xml, r#"<w:gridSpan w:val="{span}"/>"#);
                }
                match slot.kind {
                    SlotKind::Cell {
                        merge_start: true, ..
                    } => {
                        xml.push_str(r#"<w:vMerge w:val="restart"/>"#);
                    }
                    SlotKind::Continuation => xml.push_str("<w:vMerge/>"),
                    SlotKind::Cell { .. } => {}
                }
                xml.push_str("</w:tcPr>");
                let mut content = String::new();
                if let SlotKind::Cell { cell, .. } = slot.kind {
                    self.blocks(&row.cells[cell].blocks, None, &mut content);
                }
                // A cell must end with a paragraph.
                if !content.ends_with("</w:p>") {
                    content.push_str("<w:p/>");
                }
                xml.push_str(&content);
                xml.push_str("</w:tc>");
            }
            xml.push_str("</w:tr>");
        }
        xml.push_str("</w:tbl>");
    }

    fn relationship(&mut self, kind: &'static str, target: String, external: bool) -> String {
        if let Some(existing) = self
            .relationships
            .iter()
            .find(|relationship| relationship.kind == kind && relationship.target == target)
        {
            return existing.id.clone();
        }
        // rId1 and rId2 are the styles and the numbering.
        let id = format!("rId{}", self.relationships.len() + 3);
        self.relationships.push(Relationship {
            id: id.clone(),
            kind,
            target,
            external,
        });
        id
    }
}

/// What occupies one grid position of a row: a cell of the tree, or the continuation of a cell
/// merged down from a row above.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlotKind {
    Cell { cell: usize, merge_start: bool },
    Continuation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Slot {
    kind: SlotKind,
    col_span: u32,
}

struct PlacedTable {
    rows: Vec<Vec<Slot>>,
    columns: usize,
}

/// The slots of every row, with a continuation cell where a merge from above covers the grid, as
/// a docx spells a vertical merge.
fn place_cells(rows: &[TableRow]) -> PlacedTable {
    // Grid column to (rows still covered, column span) of a merge running down.
    let mut running: BTreeMap<usize, (u32, u32)> = BTreeMap::new();
    let mut placed = Vec::with_capacity(rows.len());
    let mut columns = 0;
    for row in rows {
        let mut slots = Vec::new();
        let mut column = 0;
        let mut cells = row.cells.iter().enumerate().peekable();
        loop {
            if let Some(&(remaining, span)) = running.get(&column) {
                slots.push(Slot {
                    kind: SlotKind::Continuation,
                    col_span: span,
                });
                if remaining <= 1 {
                    running.remove(&column);
                } else {
                    running.insert(column, (remaining - 1, span));
                }
                column += usize::try_from(span).unwrap_or(1);
                continue;
            }
            let Some((index, cell)) = cells.next() else {
                // Merges still running at the end of a short row are closed there.
                let tail: Vec<usize> = running.range(column..).map(|(start, _)| *start).collect();
                for start in tail {
                    let (remaining, span) = running[&start];
                    slots.push(Slot {
                        kind: SlotKind::Continuation,
                        col_span: span,
                    });
                    if remaining <= 1 {
                        running.remove(&start);
                    } else {
                        running.insert(start, (remaining - 1, span));
                    }
                    column = start + usize::try_from(span).unwrap_or(1);
                }
                break;
            };
            let span = cell.col_span.max(1);
            let merge_start = cell.row_span > 1;
            if merge_start {
                running.insert(column, (cell.row_span - 1, span));
            }
            slots.push(Slot {
                kind: SlotKind::Cell {
                    cell: index,
                    merge_start,
                },
                col_span: span,
            });
            column += usize::try_from(span).unwrap_or(1);
        }
        columns = columns.max(column);
        placed.push(slots);
    }
    PlacedTable {
        rows: placed,
        columns,
    }
}

/// Runs for text that may hold tabs and line breaks, each with the same properties.
fn push_text_runs(xml: &mut String, text: &str, run_properties: &str) {
    for (line_index, line) in text.split('\n').enumerate() {
        if line_index > 0 {
            let _ = write!(xml, "<w:r>{run_properties}<w:br/></w:r>");
        }
        for (piece_index, piece) in line.split('\t').enumerate() {
            if piece_index > 0 {
                let _ = write!(xml, "<w:r>{run_properties}<w:tab/></w:r>");
            }
            if !piece.is_empty() {
                let _ = write!(
                    xml,
                    r#"<w:r>{run_properties}<w:t xml:space="preserve">{}</w:t></w:r>"#,
                    escape(piece)
                );
            }
        }
    }
}

/// Text escaped for XML, with the characters XML 1.0 cannot hold dropped.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            '\t' | '\n' | '\r' => escaped.push(character),
            character if character.is_control() => {}
            '\u{FFFE}' | '\u{FFFF}' => {}
            character => escaped.push(character),
        }
    }
    escaped
}

fn image_extension(content_type: &str) -> Option<&'static str> {
    match content_type {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpeg"),
        "image/gif" => Some("gif"),
        "image/bmp" => Some("bmp"),
        "image/tiff" => Some("tiff"),
        "image/webp" => Some("webp"),
        "image/svg+xml" => Some("svg"),
        _ => None,
    }
}

fn text_width(tree: &DocumentTree) -> u32 {
    let [_, right, _, left] = tree.layout.margins_twips;
    tree.layout
        .width_twips
        .saturating_sub(left)
        .saturating_sub(right)
        .max(1_440)
}

fn section(tree: &DocumentTree) -> String {
    let layout = &tree.layout;
    let [top, right, bottom, left] = layout.margins_twips;
    let orientation = if layout.width_twips > layout.height_twips {
        r#" w:orient="landscape""#
    } else {
        ""
    };
    format!(
        concat!(
            r#"<w:sectPr><w:pgSz w:w="{}" w:h="{}"{}/>"#,
            r#"<w:pgMar w:top="{}" w:right="{}" w:bottom="{}" w:left="{}" w:header="708" w:footer="708" w:gutter="0"/>"#,
            "</w:sectPr>"
        ),
        layout.width_twips, layout.height_twips, orientation, top, right, bottom, left
    )
}

fn content_types() -> String {
    let mut xml = String::from(concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">"#,
        r#"<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>"#,
        r#"<Default Extension="xml" ContentType="application/xml"/>"#,
    ));
    for (extension, content_type) in [
        ("png", "image/png"),
        ("jpeg", "image/jpeg"),
        ("gif", "image/gif"),
        ("bmp", "image/bmp"),
        ("tiff", "image/tiff"),
        ("webp", "image/webp"),
        ("svg", "image/svg+xml"),
    ] {
        let _ = write!(
            xml,
            r#"<Default Extension="{extension}" ContentType="{content_type}"/>"#
        );
    }
    xml.push_str(concat!(
        r#"<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>"#,
        r#"<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>"#,
        r#"<Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/>"#,
        "</Types>"
    ));
    xml
}

fn package_relationships() -> String {
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
            r#"<Relationship Id="rId1" Type="{}/officeDocument" Target="word/document.xml"/>"#,
            "</Relationships>"
        ),
        RELATIONSHIP_TYPE
    )
}

fn document_relationships(relationships: &[Relationship]) -> String {
    let mut xml = format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
            r#"<Relationship Id="rId1" Type="{0}/styles" Target="styles.xml"/>"#,
            r#"<Relationship Id="rId2" Type="{0}/numbering" Target="numbering.xml"/>"#,
        ),
        RELATIONSHIP_TYPE
    );
    for relationship in relationships {
        let mode = if relationship.external {
            r#" TargetMode="External""#
        } else {
            ""
        };
        let _ = write!(
            xml,
            r#"<Relationship Id="{}" Type="{RELATIONSHIP_TYPE}/{}" Target="{}"{mode}/>"#,
            relationship.id,
            relationship.kind,
            escape(&relationship.target)
        );
    }
    xml.push_str("</Relationships>");
    xml
}

/// The numbering part: abstract list 0 bulleted, 1 numbered, and one instance per list of the
/// tree, so each list counts from one.
fn numbering(lists: &[(u32, bool, u32)]) -> String {
    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:numbering {NAMESPACES}>"#
    );
    for (abstract_id, ordered) in [(0, false), (1, true)] {
        let _ = write!(
            xml,
            r#"<w:abstractNum w:abstractNumId="{abstract_id}"><w:multiLevelType w:val="hybridMultilevel"/>"#
        );
        for level in 0..=MAX_LIST_LEVEL {
            let indent = 720 * (u32::from(level) + 1);
            let (format, text) = if ordered {
                (
                    ["decimal", "lowerLetter", "lowerRoman"][usize::from(level % 3)],
                    format!("%{}.", level + 1),
                )
            } else {
                (
                    "bullet",
                    ["\u{2022}", "o", "\u{25aa}"][usize::from(level % 3)].to_owned(),
                )
            };
            let _ = write!(
                xml,
                concat!(
                    r#"<w:lvl w:ilvl="{level}"><w:start w:val="1"/><w:numFmt w:val="{format}"/>"#,
                    r#"<w:lvlText w:val="{text}"/><w:lvlJc w:val="left"/>"#,
                    r#"<w:pPr><w:ind w:left="{indent}" w:hanging="360"/></w:pPr></w:lvl>"#
                ),
                level = level,
                format = format,
                text = escape(&text),
                indent = indent,
            );
        }
        xml.push_str("</w:abstractNum>");
    }
    for (num_id, ordered, _) in lists {
        let _ = write!(
            xml,
            r#"<w:num w:numId="{num_id}"><w:abstractNumId w:val="{}"/></w:num>"#,
            u8::from(*ordered)
        );
    }
    xml.push_str("</w:numbering>");
    xml
}

/// Word's built-in styles the writer uses, with the look of Word's default template.
const STYLES: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
    r#"<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:eastAsia="Calibri" w:cs="Calibri"/>"#,
    r#"<w:sz w:val="22"/><w:szCs w:val="22"/><w:lang w:val="it-IT"/></w:rPr></w:rPrDefault>"#,
    r#"<w:pPrDefault><w:pPr><w:spacing w:after="160" w:line="259" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults>"#,
    r#"<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/>"#,
    r#"<w:pPr><w:keepNext/><w:spacing w:before="240" w:after="0"/><w:outlineLvl w:val="0"/></w:pPr>"#,
    r#"<w:rPr><w:rFonts w:ascii="Calibri Light" w:hAnsi="Calibri Light"/><w:color w:val="2F5496"/><w:sz w:val="32"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/>"#,
    r#"<w:pPr><w:keepNext/><w:spacing w:before="40" w:after="0"/><w:outlineLvl w:val="1"/></w:pPr>"#,
    r#"<w:rPr><w:rFonts w:ascii="Calibri Light" w:hAnsi="Calibri Light"/><w:color w:val="2F5496"/><w:sz w:val="26"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Heading3"><w:name w:val="heading 3"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/>"#,
    r#"<w:pPr><w:keepNext/><w:spacing w:before="40" w:after="0"/><w:outlineLvl w:val="2"/></w:pPr>"#,
    r#"<w:rPr><w:rFonts w:ascii="Calibri Light" w:hAnsi="Calibri Light"/><w:color w:val="1F3763"/><w:sz w:val="24"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Heading4"><w:name w:val="heading 4"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/>"#,
    r#"<w:pPr><w:keepNext/><w:spacing w:before="40" w:after="0"/><w:outlineLvl w:val="3"/></w:pPr>"#,
    r#"<w:rPr><w:i/><w:color w:val="2F5496"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Heading5"><w:name w:val="heading 5"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/>"#,
    r#"<w:pPr><w:keepNext/><w:spacing w:before="40" w:after="0"/><w:outlineLvl w:val="4"/></w:pPr>"#,
    r#"<w:rPr><w:color w:val="2F5496"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Heading6"><w:name w:val="heading 6"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/>"#,
    r#"<w:pPr><w:keepNext/><w:spacing w:before="40" w:after="0"/><w:outlineLvl w:val="5"/></w:pPr>"#,
    r#"<w:rPr><w:color w:val="1F3763"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Quote"><w:name w:val="Quote"/><w:basedOn w:val="Normal"/><w:qFormat/>"#,
    r#"<w:pPr><w:spacing w:before="200"/><w:ind w:left="864" w:right="864"/></w:pPr>"#,
    r#"<w:rPr><w:i/><w:color w:val="404040"/></w:rPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="ListParagraph"><w:name w:val="List Paragraph"/><w:basedOn w:val="Normal"/><w:qFormat/>"#,
    r#"<w:pPr><w:ind w:left="720"/><w:contextualSpacing/></w:pPr></w:style>"#,
    r#"<w:style w:type="paragraph" w:styleId="Code"><w:name w:val="Code"/><w:basedOn w:val="Normal"/>"#,
    r#"<w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr>"#,
    r#"<w:rPr><w:rFonts w:ascii="Consolas" w:hAnsi="Consolas"/><w:sz w:val="20"/></w:rPr></w:style>"#,
    r#"<w:style w:type="character" w:default="1" w:styleId="DefaultParagraphFont"><w:name w:val="Default Paragraph Font"/><w:uiPriority w:val="1"/><w:semiHidden/></w:style>"#,
    r#"<w:style w:type="character" w:styleId="Hyperlink"><w:name w:val="Hyperlink"/><w:basedOn w:val="DefaultParagraphFont"/>"#,
    r#"<w:rPr><w:color w:val="0563C1"/><w:u w:val="single"/></w:rPr></w:style>"#,
    r#"<w:style w:type="character" w:styleId="CodeChar"><w:name w:val="Code Char"/><w:basedOn w:val="DefaultParagraphFont"/>"#,
    r#"<w:rPr><w:rFonts w:ascii="Consolas" w:hAnsi="Consolas"/><w:sz w:val="20"/></w:rPr></w:style>"#,
    r#"<w:style w:type="table" w:default="1" w:styleId="TableNormal"><w:name w:val="Normal Table"/><w:semiHidden/>"#,
    r#"<w:tblPr><w:tblInd w:w="0" w:type="dxa"/><w:tblCellMar><w:top w:w="0" w:type="dxa"/><w:left w:w="108" w:type="dxa"/>"#,
    r#"<w:bottom w:w="0" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style>"#,
    r#"<w:style w:type="table" w:styleId="TableGrid"><w:name w:val="Table Grid"/><w:basedOn w:val="TableNormal"/>"#,
    r#"<w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr>"#,
    r#"<w:tblPr><w:tblBorders><w:top w:val="single" w:sz="4" w:space="0" w:color="auto"/>"#,
    r#"<w:left w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:bottom w:val="single" w:sz="4" w:space="0" w:color="auto"/>"#,
    r#"<w:right w:val="single" w:sz="4" w:space="0" w:color="auto"/><w:insideH w:val="single" w:sz="4" w:space="0" w:color="auto"/>"#,
    r#"<w:insideV w:val="single" w:sz="4" w:space="0" w:color="auto"/></w:tblBorders></w:tblPr></w:style>"#,
    "</w:styles>"
);

#[cfg(test)]
mod tests;
