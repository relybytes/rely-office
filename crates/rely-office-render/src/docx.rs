// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! A docx document tree written out as Typst source, with the page, spacing, indents, fonts,
//! column widths and borders the document declares.
//!
//! Spacing follows the office suites rather than Typst: the space after a paragraph and the
//! space before the next add up instead of collapsing, so every block sits with no spacing of
//! its own and the gap between two blocks is written out as a weak `v`, which also vanishes at
//! the top of a page as an office suite drops it there; the space before the first block of the
//! document or of a cell stays. Word adds the extra height of spaced lines below every line, the
//! last one included, while Typst leads only between lines, so that extra goes after the block.

use std::collections::HashMap;
use std::fmt::Write;

use rely_office_core::{
    Alignment, Block, DocumentTree, Inline, LineSpacing, Mark, Media, ParagraphLook, RunLook,
    TableLook, TableRow,
};

use crate::fonts::{MONO, body_fonts};
use crate::markup::{font_list, points, string_literal};
use crate::metrics::FontLines;

const TWIPS_PER_POINT: f64 = 20.0;
const POINTS_PER_CSS_PIXEL: f64 = 0.75;
/// Word's body size when the document names none.
const DEFAULT_FONT_POINTS: f64 = 11.0;
/// The indent of each list level and the room of its marker, Word's 0.63 cm.
const LIST_INDENT_POINTS: f64 = 18.0;
/// Heading sizes of the default Word template, for a heading whose style names no size.
const HEADING_POINTS: [f64; 6] = [16.0, 13.0, 12.0, 11.0, 11.0, 11.0];
const HEADING_COLOUR: &str = "2F5496";
const LINK_COLOUR: &str = "0563C1";
/// Word's default cell margins in twips, top, left, bottom, right: 0.19 cm left and right.
const DEFAULT_CELL_MARGINS_TWIPS: [u32; 4] = [0, 108, 0, 108];
/// Word's own limit; a wider table is a crafted file.
const MAX_COLUMNS: usize = 63;

/// The source and the image files of a document. With `with_images` false every image is drawn
/// as an empty frame of its size, for a document whose images Typst cannot decode.
pub(crate) fn document_source(
    tree: &DocumentTree,
    with_images: bool,
    lines: &FontLines,
) -> (String, Vec<(String, Vec<u8>)>) {
    let layout = &tree.layout;
    let width = f64::from(layout.width_twips) / TWIPS_PER_POINT;
    let height = f64::from(layout.height_twips) / TWIPS_PER_POINT;
    let [top, right, bottom, left] = layout
        .margins_twips
        .map(|margin| f64::from(margin) / TWIPS_PER_POINT);
    let body_size = layout
        .font_size_half_points
        .map_or(DEFAULT_FONT_POINTS, |size| f64::from(size) / 2.0);
    let body_family = layout.font_family.clone();

    let mut writer = Writer {
        media: &tree.media,
        with_images,
        lines,
        body_family,
        body_size,
        text_width: (width - left - right).max(1.0),
        files: HashMap::new(),
        list_counters: HashMap::new(),
    };
    let mut source = String::new();
    let _ = writeln!(
        source,
        "#set page(width: {}, height: {}, margin: (top: {}, right: {}, bottom: {}, left: {}))",
        points(width),
        points(height),
        points(top),
        points(right),
        points(bottom),
        points(left)
    );
    let _ = writeln!(
        source,
        "#set text(font: {}, size: {}, lang: \"it\", hyphenate: false, top-edge: \"ascender\", bottom-edge: \"descender\")",
        font_list(&body_fonts(writer.body_family.as_deref())),
        points(body_size)
    );
    // Office suites fill each line greedily; Typst's optimized breaking would reflow paragraphs.
    source.push_str("#set par(spacing: 0pt, linebreaks: \"simple\")\n#set block(spacing: 0pt)\n");
    let _ = writeln!(
        source,
        "#show raw: set text(font: {})",
        string_literal(MONO)
    );
    let _ = writeln!(
        source,
        "#show link: it => underline(text(fill: rgb(\"#{LINK_COLOUR}\"), it))"
    );
    source.push_str("#show heading: it => it.body\n");
    source.push_str(&writer.blocks(&tree.blocks));

    let mut files: Vec<(String, Vec<u8>)> = writer
        .files
        .into_iter()
        .map(|(index, path)| (path, tree.media[index].bytes.clone()))
        .collect();
    files.sort();
    (source, files)
}

/// A block written out, with the space it wants around it.
struct Placed {
    expression: String,
    before: f64,
    after: f64,
    page_break: bool,
}

impl Placed {
    fn tight(expression: String) -> Self {
        Self {
            expression,
            before: 0.0,
            after: 0.0,
            page_break: false,
        }
    }
}

/// The text and paragraph settings of a paragraph, and the indents it sits in.
struct ParagraphParts {
    text_arguments: String,
    leading: f64,
    inlines: String,
    before: f64,
    after: f64,
    left: f64,
    right: f64,
    first_line: f64,
    page_break: bool,
}

struct Writer<'a> {
    media: &'a [Media],
    with_images: bool,
    lines: &'a FontLines,
    body_family: Option<String>,
    body_size: f64,
    text_width: f64,
    /// The media index of each image written out, with its path.
    files: HashMap<usize, String>,
    /// The counters of each numbered list, one per level.
    list_counters: HashMap<u32, Vec<u32>>,
}

impl Writer<'_> {
    /// Blocks as markup, each gap between two blocks the sum of their spacing.
    fn blocks(&mut self, blocks: &[Block]) -> String {
        let mut markup = String::new();
        let mut previous_after: Option<f64> = None;
        for block in blocks {
            let placed = self.place(block);
            if placed.page_break {
                markup.push_str("#pagebreak(weak: true)\n");
                previous_after = None;
            }
            let gap = previous_after.unwrap_or(0.0) + placed.before;
            if gap > 0.0 {
                let weak = if previous_after.is_some() || placed.page_break {
                    ", weak: true"
                } else {
                    ""
                };
                let _ = writeln!(markup, "#v({}{weak})", points(gap));
            }
            markup.push('#');
            markup.push_str(&placed.expression);
            markup.push('\n');
            previous_after = Some(placed.after);
        }
        markup
    }

    fn place(&mut self, block: &Block) -> Placed {
        match block {
            Block::Paragraph {
                content,
                align,
                look,
            } => self.paragraph(content, *align, look, None),
            Block::Heading {
                level,
                content,
                align,
                look,
            } => self.paragraph(content, *align, look, Some(*level)),
            Block::ListItem {
                ordered,
                level,
                list_id,
                content,
                look,
            } => self.list_item(*ordered, *level, *list_id, content, look),
            Block::Table { rows, look } => Placed::tight(self.table(rows, look)),
            Block::Blockquote { blocks } => Placed::tight(format!(
                "pad(left: {})[{}]",
                points(LIST_INDENT_POINTS * 2.0),
                self.blocks(blocks)
            )),
            Block::CodeBlock { text } => Placed::tight(format!(
                "block(width: 100%, fill: luma(246), inset: 6pt, raw(block: true, {}))",
                string_literal(text)
            )),
            Block::HorizontalRule => Placed {
                expression: "line(length: 100%, stroke: 0.5pt + luma(150))".to_owned(),
                before: 6.0,
                after: 6.0,
                page_break: false,
            },
        }
    }

    fn paragraph(
        &mut self,
        content: &[Inline],
        align: Option<Alignment>,
        look: &ParagraphLook,
        heading: Option<u8>,
    ) -> Placed {
        let parts = self.paragraph_parts(content, look, heading);
        let mut par_arguments = format!("leading: {}", points(parts.leading));
        if align == Some(Alignment::Justify) {
            par_arguments.push_str(", justify: true");
        }
        if parts.first_line > 0.0 {
            let _ = write!(
                par_arguments,
                ", first-line-indent: (amount: {}, all: true)",
                points(parts.first_line)
            );
        } else if parts.first_line < 0.0 {
            let _ = write!(
                par_arguments,
                ", hanging-indent: {}",
                points(-parts.first_line)
            );
        }
        // A hanging first line starts left of the paragraph's indent.
        let left = (parts.left + parts.first_line.min(0.0)).max(0.0);
        let mut expression = format!(
            "text({}, par({par_arguments})[{}])",
            parts.text_arguments, parts.inlines
        );
        if left > 0.0 || parts.right > 0.0 {
            expression = format!(
                "pad(left: {}, right: {}, {expression})",
                points(left),
                points(parts.right.max(0.0))
            );
        }
        expression = aligned(align, expression);
        if let Some(level) = heading {
            expression = format!("heading(level: {})[#{expression}]", level.clamp(1, 6));
        }
        Placed {
            expression,
            before: parts.before,
            after: parts.after + parts.leading,
            page_break: parts.page_break,
        }
    }

    fn paragraph_parts(
        &mut self,
        content: &[Inline],
        look: &ParagraphLook,
        heading: Option<u8>,
    ) -> ParagraphParts {
        let family = look
            .font
            .family
            .clone()
            .or_else(|| self.body_family.clone());
        let families = body_fonts(family.as_deref());
        let heading_fallback = heading.filter(|_| look.font.size_half_points.is_none());
        let size = match heading_fallback {
            Some(level) => HEADING_POINTS[usize::from(level.clamp(1, 6)) - 1],
            None => look
                .font
                .size_half_points
                .map_or(self.body_size, |size| f64::from(size) / 2.0),
        };
        let colour = look
            .font
            .colour
            .clone()
            .or_else(|| heading_fallback.map(|_| HEADING_COLOUR.to_owned()));
        let mut text_arguments = format!("font: {}, size: {}", font_list(&families), points(size));
        if look.font.bold == Some(true) {
            text_arguments.push_str(", weight: \"bold\"");
        }
        if look.font.italic == Some(true) {
            text_arguments.push_str(", style: \"italic\"");
        }
        if let Some(colour) = colour {
            let _ = write!(text_arguments, ", fill: rgb(\"#{colour}\")");
        }

        let metrics = self.lines.line(&families);
        let single = metrics.single_em * size;
        let line_height = match look.line_spacing {
            None => single,
            Some(LineSpacing::Auto(multiple)) => single * f64::from(multiple) / 240.0,
            Some(LineSpacing::Exact(height)) => f64::from(height) / TWIPS_PER_POINT,
            Some(LineSpacing::AtLeast(height)) => (f64::from(height) / TWIPS_PER_POINT).max(single),
        };
        let inlines = if content.is_empty() {
            "#hide[x]".to_owned()
        } else {
            self.inlines(content)
        };
        let twips =
            |value: Option<u32>| value.map_or(0.0, |value| f64::from(value) / TWIPS_PER_POINT);
        let signed =
            |value: Option<i32>| value.map_or(0.0, |value| f64::from(value) / TWIPS_PER_POINT);
        ParagraphParts {
            text_arguments,
            leading: (line_height - metrics.box_em * size).max(0.0),
            inlines,
            before: twips(look.space_before_twips),
            after: twips(look.space_after_twips),
            left: signed(look.indent_left_twips),
            right: signed(look.indent_right_twips),
            first_line: signed(look.indent_first_line_twips),
            page_break: look.page_break_before,
        }
    }

    fn list_item(
        &mut self,
        ordered: bool,
        level: u8,
        list_id: u32,
        content: &[Inline],
        look: &ParagraphLook,
    ) -> Placed {
        let marker = self.list_marker(ordered, level, list_id);
        let parts = self.paragraph_parts(content, look, None);
        let hanging = if parts.first_line < 0.0 {
            -parts.first_line
        } else {
            LIST_INDENT_POINTS
        };
        let left = if look.indent_left_twips.is_some() {
            parts.left
        } else {
            (f64::from(level) + 1.0) * LIST_INDENT_POINTS
        };
        let expression = format!(
            "pad(left: {}, text({}, grid(columns: ({}, 1fr), [{marker}], par(leading: {})[{}])))",
            points((left - hanging).max(0.0)),
            parts.text_arguments,
            points(hanging),
            points(parts.leading),
            parts.inlines
        );
        Placed {
            expression,
            before: parts.before,
            after: parts.after + parts.leading,
            page_break: parts.page_break,
        }
    }

    /// Inline content as markup, one expression after the other with nothing between them.
    fn inlines(&mut self, content: &[Inline]) -> String {
        let mut markup = String::new();
        for inline in content {
            markup.push('#');
            match inline {
                Inline::Text { text, marks, look } => markup.push_str(&marked(text, marks, look)),
                Inline::HardBreak => markup.push_str("linebreak()"),
                Inline::Image {
                    media,
                    width,
                    height,
                    ..
                } => markup.push_str(&self.image(*media, *width, *height)),
            }
        }
        markup
    }

    fn image(&mut self, media: usize, width: Option<u32>, height: Option<u32>) -> String {
        let (width, height) = self.image_size(width, height);
        let size = match (width, height) {
            (Some(width), Some(height)) => {
                format!("width: {}, height: {}", points(width), points(height))
            }
            (Some(width), None) => format!("width: {}", points(width)),
            _ => format!("width: {}", points(self.text_width.min(144.0))),
        };
        let extension = self
            .media
            .get(media)
            .and_then(|media| image_extension(media.content_type));
        match extension {
            Some(extension) if self.with_images => {
                let path = self
                    .files
                    .entry(media)
                    .or_insert_with(|| format!("/media/{media}.{extension}"))
                    .clone();
                format!(
                    "box(image({}, {size}, fit: \"stretch\"))",
                    string_literal(&path)
                )
            }
            _ => format!("box({size}, stroke: 0.5pt + luma(180))"),
        }
    }

    /// The size of an image in points, cut to the text width with its proportions kept.
    fn image_size(&self, width: Option<u32>, height: Option<u32>) -> (Option<f64>, Option<f64>) {
        let width = width.map(|width| f64::from(width) * POINTS_PER_CSS_PIXEL);
        let height = height.map(|height| f64::from(height) * POINTS_PER_CSS_PIXEL);
        match (width, height) {
            (Some(width), Some(height)) if width > self.text_width => (
                Some(self.text_width),
                Some(height * self.text_width / width),
            ),
            (Some(width), height) => (Some(width.min(self.text_width)), height),
            _ => (None, None),
        }
    }

    fn list_marker(&mut self, ordered: bool, level: u8, list_id: u32) -> String {
        let level = usize::from(level.min(8));
        let counters = self.list_counters.entry(list_id).or_default();
        counters.resize(level + 1, 0);
        counters[level] += 1;
        if !ordered {
            return format!(
                "#{}",
                string_literal(["\u{2022}", "\u{25e6}", "\u{25aa}"][level % 3])
            );
        }
        let number = counters[level];
        let label = match level % 3 {
            0 => number.to_string(),
            1 => alphabetic(number),
            _ => roman(number),
        };
        format!("#{}", string_literal(&format!("{label}.")))
    }

    fn table(&mut self, rows: &[TableRow], look: &TableLook) -> String {
        let columns = column_count(rows);
        if columns == 0 {
            return "[]".to_owned();
        }
        let row_count = rows.len();
        let header_rows = header_row_count(rows);
        let mut header = Vec::new();
        let mut body = Vec::new();
        for (row_index, row) in rows.iter().enumerate() {
            let in_header = row_index < header_rows;
            for cell in &row.cells {
                let col_span = cell
                    .col_span
                    .clamp(1, u32::try_from(columns).unwrap_or(u32::MAX));
                let remaining_rows = u32::try_from(row_count - row_index).unwrap_or(u32::MAX);
                let row_span = cell.row_span.clamp(1, remaining_rows);
                let content = self.blocks(&cell.blocks);
                let cell = if col_span == 1 && row_span == 1 {
                    format!("[{content}]")
                } else {
                    format!("table.cell(colspan: {col_span}, rowspan: {row_span})[{content}]")
                };
                if in_header {
                    header.push(cell);
                } else {
                    body.push(cell);
                }
            }
        }
        let stroke = if look.borders { "0.5pt" } else { "none" };
        let [top, left, bottom, right] = [0, 1, 2, 3].map(|side| {
            f64::from(look.cell_margins_twips[side].unwrap_or(DEFAULT_CELL_MARGINS_TWIPS[side]))
                / TWIPS_PER_POINT
        });
        let mut table = format!(
            "table(columns: ({}), stroke: {stroke}, inset: (top: {}, left: {}, bottom: {}, right: {}),",
            self.column_widths(&look.column_widths_twips, columns),
            points(top),
            points(left),
            points(bottom),
            points(right)
        );
        if !header.is_empty() && !body.is_empty() {
            let _ = write!(table, " table.header({}),", header.join(", "));
        } else {
            body.splice(0..0, header);
        }
        for cell in body {
            let _ = write!(table, " {cell},");
        }
        table.push(')');
        table
    }

    /// The widths of the grid when it has one column per table column, cut to the text width;
    /// equal shares otherwise.
    fn column_widths(&self, grid_twips: &[u32], columns: usize) -> String {
        if grid_twips.len() != columns || grid_twips.contains(&0) {
            return "1fr, ".repeat(columns);
        }
        let total: f64 = grid_twips
            .iter()
            .map(|width| f64::from(*width) / TWIPS_PER_POINT)
            .sum();
        let scale = if total > self.text_width {
            self.text_width / total
        } else {
            1.0
        };
        let mut widths = String::new();
        for width in grid_twips {
            let _ = write!(
                widths,
                "{}, ",
                points(f64::from(*width) / TWIPS_PER_POINT * scale)
            );
        }
        widths
    }
}

/// How many leading rows repeat as the header: the rows marked so, unless a cell of theirs spans
/// into the body, which Typst does not accept in a header.
fn header_row_count(rows: &[TableRow]) -> usize {
    let marked = rows.iter().take_while(|row| row.header).count();
    let spans_out = rows[..marked].iter().enumerate().any(|(row_index, row)| {
        row.cells
            .iter()
            .any(|cell| row_index + usize::try_from(cell.row_span).unwrap_or(usize::MAX) > marked)
    });
    if spans_out { 0 } else { marked }
}

/// How many columns the rows fill once each cell takes the first free place, as Typst places
/// them.
fn column_count(rows: &[TableRow]) -> usize {
    let mut occupied: Vec<Vec<bool>> = Vec::new();
    let mut columns = 0;
    for (row_index, row) in rows.iter().enumerate() {
        let mut column = 0;
        for cell in &row.cells {
            while occupied
                .get(row_index)
                .and_then(|row| row.get(column))
                .copied()
                .unwrap_or(false)
            {
                column += 1;
            }
            let col_span = usize::try_from(cell.col_span.max(1))
                .unwrap_or(1)
                .min(MAX_COLUMNS.saturating_sub(column).max(1));
            let row_span = usize::try_from(cell.row_span.max(1))
                .unwrap_or(1)
                .min(rows.len() - row_index);
            while occupied.len() < row_index + row_span {
                occupied.push(Vec::new());
            }
            for covered_row in &mut occupied[row_index..row_index + row_span] {
                if covered_row.len() < column + col_span {
                    covered_row.resize(column + col_span, false);
                }
                for slot in &mut covered_row[column..column + col_span] {
                    *slot = true;
                }
            }
            column += col_span;
            columns = columns.max(column);
        }
    }
    columns
        .max(occupied.iter().map(Vec::len).max().unwrap_or(0))
        .min(MAX_COLUMNS)
}

fn aligned(align: Option<Alignment>, expression: String) -> String {
    match align {
        Some(Alignment::Center) => format!("align(center, {expression})"),
        Some(Alignment::Right) => format!("align(right, {expression})"),
        _ => expression,
    }
}

/// A text run with its font and marks, innermost first in the order they are listed.
fn marked(text: &str, marks: &[Mark], look: &RunLook) -> String {
    let mut expression = string_literal(text);
    let mut font = Vec::new();
    if let Some(family) = &look.family {
        font.push(format!("font: {}", font_list(&body_fonts(Some(family)))));
    }
    if let Some(size) = look.size_half_points {
        font.push(format!("size: {}", points(f64::from(size) / 2.0)));
    }
    if let Some(colour) = &look.colour {
        font.push(format!("fill: rgb(\"#{colour}\")"));
    }
    if !font.is_empty() {
        expression = format!("text({}, {expression})", font.join(", "));
    }
    for mark in marks {
        expression = match mark {
            Mark::Bold => format!("text(weight: \"bold\", {expression})"),
            Mark::Italic => format!("text(style: \"italic\", {expression})"),
            Mark::Underline => format!("underline({expression})"),
            Mark::Strike => format!("strike({expression})"),
            Mark::Code => format!("text(font: {}, {expression})", string_literal(MONO)),
            Mark::Superscript => format!("super({expression})"),
            Mark::Subscript => format!("sub({expression})"),
            Mark::Link { href } => format!("link({}, {expression})", string_literal(href)),
        };
    }
    expression
}

fn image_extension(content_type: &str) -> Option<&'static str> {
    match content_type {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/svg+xml" => Some("svg"),
        _ => None,
    }
}

fn alphabetic(number: u32) -> String {
    let mut number = number.max(1);
    let mut label = Vec::new();
    while number > 0 {
        number -= 1;
        label.push(char::from(b'a' + u8::try_from(number % 26).unwrap_or(0)));
        number /= 26;
    }
    label.iter().rev().collect()
}

fn roman(number: u32) -> String {
    const NUMERALS: [(u32, &str); 13] = [
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let mut number = number.clamp(1, 3_999);
    let mut label = String::new();
    for (value, numeral) in NUMERALS {
        while number >= value {
            label.push_str(numeral);
            number -= value;
        }
    }
    label
}

#[cfg(test)]
mod tests;
