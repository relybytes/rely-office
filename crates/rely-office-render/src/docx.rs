// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! A docx document tree written out as Typst source, with the page, headers and footers, notes,
//! spacing, indents, tab stops, fonts, list labels, borders, shading and table looks the document
//! declares.
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
    Alignment, Block, Border, BorderStyle, Borders, CellLook, DocumentTree, HeadersFooters, Inline,
    LabelSuffix, LineSpacing, ListLabel, Mark, Media, Note, NoteKind, PageField, PageLayout,
    ParagraphLook, RunLook, Section, TabKind, TabLeader, TabStop, TableLook, TableRow,
    VerticalAlignment,
};

use crate::fonts::{MONO, body_fonts};
use crate::markup::{font_list, points, string_literal};
use crate::metrics::FontLines;
use floats::{Float, PagePlace, frame_of};

const TWIPS_PER_POINT: f64 = 20.0;
/// The colour of tracked changes, as Word marks those of its first reviewer.
const REVISION_COLOUR: &str = "#C00000";
/// The math font equations are set in, an OpenType font with a MATH table.
const MATH_FONT: &str = "DejaVu Math TeX Gyre";
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
/// Word draws a paragraph border a point above and below the text and four points beside it.
const PARAGRAPH_BORDER_SPACE: [f64; 4] = [1.0, 4.0, 1.0, 4.0];
/// The stroke of a table that draws borders but names none.
const DEFAULT_STROKE: &str = "0.5pt";

/// Lays out the text after each tab: at the next stop past the text before it, left, centre or
/// right aligned to the stop, or at the next default stop. Positions are from the left margin;
/// `origin` is where the line starts.
const TAB_FUNCTION: &str = r#"#let rely-tabbed(origin, step, stops, parts) = context {
  let x = origin
  let pieces = ()
  for (index, part) in parts.enumerate() {
    let width = measure(part).width
    if index > 0 {
      let found = stops.find(stop => stop.at(0) > x + 0.5pt)
      let (position, kind, leader) = if found == none {
        ((calc.floor((x + 0.5pt) / step) + 1) * step, "left", none)
      } else { found }
      let start = if kind == "right" { position - width } else if kind == "center" { position - width / 2 } else { position }
      if start < x { start = x }
      let gap = start - x
      pieces.push(if leader == none { h(gap) } else { box(width: gap, repeat(leader)) })
      x = start
    }
    pieces.push(part)
    x += width
  }
  pieces.join()
}
"#;

/// Footnotes as Word prints them: the number raised beside the note's first line, not above it.
const FOOTNOTE_RULES: &str = concat!(
    "#set footnote.entry(separator: line(length: 33%, stroke: 0.5pt), gap: 2pt)\n",
    "#show footnote.entry: it => grid(columns: (auto, 1fr), column-gutter: 2pt, ",
    "super(numbering(it.note.numbering, ..counter(footnote).at(it.note.location()))), it.note.body)\n",
);

/// The source and the image files of a document. With `with_images` false every image is drawn
/// as an empty frame of its size, for a document whose images Typst cannot decode.
#[expect(
    clippy::too_many_lines,
    reason = "each section sets its page, then the shared text rules come before them all"
)]
pub(crate) fn document_source(
    tree: &DocumentTree,
    with_images: bool,
    lines: &FontLines,
) -> (String, Vec<(String, Vec<u8>)>) {
    let single = [Section {
        end_block: tree.blocks.len(),
        layout: tree.layout.clone(),
        headers_footers: tree.headers_footers.clone(),
        continuous: false,
    }];
    let sections: &[Section] = if tree.sections.is_empty() {
        &single
    } else {
        &tree.sections
    };
    let layout = &tree.layout;
    let body_size = layout
        .font_size_half_points
        .map_or(DEFAULT_FONT_POINTS, |size| f64::from(size) / 2.0);
    let body_family = layout.font_family.clone();

    let mut writer = Writer {
        media: &tree.media,
        notes: &tree.notes,
        with_images,
        lines,
        body_family,
        body_size,
        text_width: PageGeometry::of(&sections[0].layout).column_width,
        page: PagePlace::default(),
        open_frame: None,
        default_tab: (f64::from(layout.default_tab_twips) / TWIPS_PER_POINT).max(1.0),
        spacing_adds_up: layout.spacing_adds_up,
        files: HashMap::new(),
        list_counters: HashMap::new(),
        endnotes: Vec::new(),
        in_note: false,
    };
    let mut body = String::new();
    let mut start = 0;
    let mut previous: Option<&PageLayout> = None;
    for (index, section) in sections.iter().enumerate() {
        let end = if index + 1 == sections.len() {
            tree.blocks.len()
        } else {
            section.end_block.clamp(start, tree.blocks.len())
        };
        let geometry = PageGeometry::of(&section.layout);
        writer.text_width = geometry.column_width;
        writer.page = PagePlace {
            width: geometry.width,
            height: geometry.height,
            left: geometry.left,
            top: geometry.top,
            right: geometry.right,
            bottom: geometry.bottom,
        };
        // A continuous section on the same page goes on where the last one stopped.
        let goes_on = section.continuous
            && previous.is_some_and(|previous| PageGeometry::of(previous).same_page(&geometry));
        if !goes_on {
            let parts =
                writer.page_parts(&section.headers_footers, &section.layout, &geometry, index);
            let _ = writeln!(
                body,
                "#set page(width: {}, height: {}, margin: (top: {}, right: {}, bottom: {}, left: {}){parts})",
                points(geometry.width),
                points(geometry.height),
                points(geometry.top),
                points(geometry.right),
                points(geometry.bottom),
                points(geometry.left)
            );
        }
        let _ = writeln!(body, "#metadata(none) <rely-section-{index}>");
        let blocks = writer.blocks(&tree.blocks[start..end]);
        if geometry.columns > 1 {
            let _ = writeln!(
                body,
                "#columns({}, gutter: {})[\n{blocks}]",
                geometry.columns,
                points(geometry.column_gap)
            );
        } else {
            body.push_str(&blocks);
        }
        previous = Some(&section.layout);
        start = end;
    }
    let endnotes = writer.endnote_list();

    let mut source = String::from(TAB_FUNCTION);
    source.push_str(floats::FLOW_FUNCTION);
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
    // Equations take the math font the renderer holds, and the body's when it holds none.
    let _ = writeln!(
        source,
        "#show math.equation: set text(font: {})",
        font_list(&[MATH_FONT.to_owned(), "Liberation Serif".to_owned()])
    );
    let _ = writeln!(
        source,
        "#show link: it => underline(text(fill: rgb(\"#{LINK_COLOUR}\"), it))"
    );
    source.push_str("#show heading: it => it.body\n");
    source.push_str(FOOTNOTE_RULES);
    source.push_str(&body);
    source.push_str(&endnotes);

    let mut files: Vec<(String, Vec<u8>)> = writer
        .files
        .into_iter()
        .map(|(index, path)| (path, tree.media[index].bytes.clone()))
        .collect();
    files.sort();
    (source, files)
}

/// The page of a section in points: its size, margins, and the width of a column of text.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PageGeometry {
    width: f64,
    height: f64,
    top: f64,
    right: f64,
    bottom: f64,
    left: f64,
    columns: u32,
    column_gap: f64,
    column_width: f64,
}

impl PageGeometry {
    fn of(layout: &PageLayout) -> Self {
        let width = f64::from(layout.width_twips) / TWIPS_PER_POINT;
        let height = f64::from(layout.height_twips) / TWIPS_PER_POINT;
        let [top, right, bottom, left] = layout
            .margins_twips
            .map(|margin| f64::from(margin) / TWIPS_PER_POINT);
        let columns = layout.columns.max(1);
        let column_gap = f64::from(layout.column_gap_twips) / TWIPS_PER_POINT;
        let text_width = (width - left - right).max(1.0);
        let column_width = if columns > 1 {
            ((text_width - column_gap * f64::from(columns - 1)) / f64::from(columns)).max(1.0)
        } else {
            text_width
        };
        Self {
            width,
            height,
            top,
            right,
            bottom,
            left,
            columns,
            column_gap,
            column_width,
        }
    }

    /// The same sheet of paper with the same margins, whatever the columns on it.
    fn same_page(&self, other: &Self) -> bool {
        [
            self.width,
            self.height,
            self.top,
            self.right,
            self.bottom,
            self.left,
        ]
        .iter()
        .zip([
            other.width,
            other.height,
            other.top,
            other.right,
            other.bottom,
            other.left,
        ])
        .all(|(one, other)| (one - other).abs() < 0.01)
    }
}

/// A block written out, with the space it wants around it.
struct Placed {
    expression: String,
    before: f64,
    after: f64,
    /// The leading under its last line, which comes before any space after it.
    trailing: f64,
    page_break: bool,
}

impl Placed {
    fn tight(expression: String) -> Self {
        Self {
            expression,
            before: 0.0,
            after: 0.0,
            trailing: 0.0,
            page_break: false,
        }
    }
}

/// The text and paragraph settings of a paragraph, and the indents it sits in.
struct ParagraphParts {
    text_arguments: String,
    leading: f64,
    /// The distance from one line to the next.
    line_height: f64,
    inlines: String,
    before: f64,
    after: f64,
    left: f64,
    right: f64,
    first_line: f64,
    page_break: bool,
}

/// The tab stops a paragraph's text is laid out against.
struct TabContext {
    /// Where the first line starts, from the left margin.
    origin: f64,
    /// The paragraph's stops, from the left margin.
    stops: Vec<TabStop>,
}

struct Writer<'a> {
    media: &'a [Media],
    notes: &'a [Note],
    with_images: bool,
    lines: &'a FontLines,
    body_family: Option<String>,
    body_size: f64,
    text_width: f64,
    default_tab: f64,
    /// The space after a paragraph and the space before the next add up, rather than the
    /// larger of the two being kept.
    spacing_adds_up: bool,
    /// The media index of each image written out, with its path.
    files: HashMap<usize, String>,
    /// The counters of each numbered list, one per level.
    list_counters: HashMap<u32, Vec<u32>>,
    /// The endnotes referred to so far, in order: their number is their position plus one.
    endnotes: Vec<usize>,
    /// Inside a note, where a reference to another note is not followed.
    in_note: bool,
    /// The page of the section being written, where floating objects are placed.
    page: PagePlace,
    /// The text box being written, whose blocks are not framed again.
    open_frame: Option<u32>,
}

impl Writer<'_> {
    /// The `header` and `footer` arguments of the page: the first page, even pages and the
    /// others each with their own when the document has them.
    /// The header and footer arguments of a section's page; a first-page variant shows on the
    /// page the section starts on.
    fn page_parts(
        &mut self,
        parts: &HeadersFooters,
        layout: &PageLayout,
        geometry: &PageGeometry,
        section: usize,
    ) -> String {
        if parts.is_empty() {
            // A page set later keeps the header of the one before unless it says otherwise.
            return if section == 0 {
                String::new()
            } else {
                ", header: none, footer: none".to_owned()
            };
        }
        let header_distance = (f64::from(layout.header_distance_twips) / TWIPS_PER_POINT)
            .clamp(0.0, geometry.top.max(0.0));
        let footer_distance = (f64::from(layout.footer_distance_twips) / TWIPS_PER_POINT)
            .clamp(0.0, geometry.bottom.max(0.0));
        let width = self.text_width;
        self.text_width = (geometry.width - geometry.left - geometry.right).max(1.0);
        let header = self.page_part(
            &parts.header,
            parts.first_header.as_deref(),
            parts.even_header.as_deref(),
            &format!("align(top, pad(top: {}, ", points(header_distance)),
            section,
        );
        let footer = self.page_part(
            &parts.footer,
            parts.first_footer.as_deref(),
            parts.even_footer.as_deref(),
            &format!("align(bottom, pad(bottom: {}, ", points(footer_distance)),
            section,
        );
        self.text_width = width;
        format!(", header: {header}, header-ascent: 0pt, footer: {footer}, footer-descent: 0pt")
    }

    fn page_part(
        &mut self,
        default: &[Block],
        first: Option<&[Block]>,
        even: Option<&[Block]>,
        frame: &str,
        section: usize,
    ) -> String {
        let wrap = |blocks: &[Block], writer: &mut Self| {
            format!("{frame}block(width: 100%)[{}]))", writer.blocks(blocks))
        };
        let default = wrap(default, self);
        let first = first.map(|blocks| wrap(blocks, self));
        let even = even.map(|blocks| wrap(blocks, self));
        let mut choice = String::from("context { let page-number = here().page(); ");
        if let Some(first) = &first {
            let _ = write!(
                choice,
                "if page-number == locate(<rely-section-{section}>).page() {{ {first} }} else "
            );
        }
        if let Some(even) = &even {
            let _ = write!(choice, "if calc.even(page-number) {{ {even} }} else ");
        }
        let _ = write!(choice, "{{ {default} }} }}");
        choice
    }

    /// The endnotes at the end of the document, numbered in roman numerals as Word numbers them.
    fn endnote_list(&mut self) -> String {
        let mut markup = String::new();
        if self.endnotes.is_empty() {
            return markup;
        }
        markup.push_str("#v(12pt, weak: true)\n#line(length: 33%, stroke: 0.5pt)\n");
        let mut index = 0;
        // Rendering an endnote may not add another: notes inside notes are not followed.
        while let Some(&note) = self.endnotes.get(index) {
            index += 1;
            let Some(content) = self.notes.get(note) else {
                continue;
            };
            self.in_note = true;
            let blocks = self.blocks(&content.blocks);
            self.in_note = false;
            let _ = writeln!(
                markup,
                "#grid(columns: (auto, 1fr), column-gutter: 3pt, super({}), [{blocks}])",
                string_literal(&roman(u32::try_from(index).unwrap_or(1)))
            );
        }
        markup
    }

    /// Blocks as markup, each gap between two blocks the sum of their spacing.
    fn blocks(&mut self, blocks: &[Block]) -> String {
        let mut placed_blocks: Vec<Placed> = Vec::new();
        let mut index = 0;
        while index < blocks.len() {
            let frame = frame_of(&blocks[index])
                .filter(|frame| self.open_frame != Some(frame.id))
                .cloned();
            let Some(frame) = frame else {
                placed_blocks.push(self.place(&blocks[index]));
                index += 1;
                continue;
            };
            let mut end = index + 1;
            while blocks
                .get(end)
                .and_then(frame_of)
                .is_some_and(|other| other.id == frame.id)
            {
                end += 1;
            }
            // A text box floats from the paragraph before it, the one that anchors it.
            let expression = self.frame_box(&blocks[index..end], &frame);
            let host = placed_blocks
                .pop()
                .unwrap_or_else(|| Placed::tight("[]".to_owned()));
            placed_blocks.push(self.with_floats(
                host,
                vec![Float {
                    expression,
                    anchor: frame.anchor,
                }],
            ));
            index = end;
        }
        let mut markup = String::new();
        let mut previous_after: Option<(f64, f64)> = None;
        for placed in placed_blocks {
            if placed.page_break {
                markup.push_str("#pagebreak(weak: true)\n");
                previous_after = None;
            }
            let gap = match previous_after {
                None => placed.before,
                Some((after, trailing)) if self.spacing_adds_up => trailing + after + placed.before,
                Some((after, trailing)) => trailing + after.max(placed.before),
            };
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
            previous_after = Some((placed.after, placed.trailing));
        }
        markup
    }

    fn place(&mut self, block: &Block) -> Placed {
        match block {
            Block::Paragraph {
                content,
                align,
                look,
            } => {
                let (content, mut floats) = self.split_floats(content);
                if let Some(flowed) = self.flowed(&content, *align, look, &mut floats) {
                    return self.with_floats(flowed, floats);
                }
                let placed = self.paragraph(&content, *align, look, None);
                self.with_floats(placed, floats)
            }
            Block::Heading {
                level,
                content,
                align,
                look,
            } => {
                let (content, floats) = self.split_floats(content);
                let placed = self.paragraph(&content, *align, look, Some(*level));
                self.with_floats(placed, floats)
            }
            Block::ListItem {
                ordered,
                level,
                list_id,
                content,
                look,
            } => {
                let (content, floats) = self.split_floats(content);
                let placed = self.list_item(*ordered, *level, *list_id, &content, look);
                self.with_floats(placed, floats)
            }
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
                trailing: 0.0,
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
        if let Some(label) = &look.list_label {
            return self.labelled(label, content, look, heading, align);
        }
        let parts = self.paragraph_parts(content, look, heading, 0.0);
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
        expression = aligned(align, expression);
        expression = framed(look, expression);
        if left > 0.0 || parts.right > 0.0 {
            expression = format!(
                "pad(left: {}, right: {}, {expression})",
                points(left),
                points(parts.right.max(0.0))
            );
        }
        if let Some(level) = heading {
            expression = format!("heading(level: {})[#{expression}]", level.clamp(1, 6));
        }
        Placed {
            expression,
            before: parts.before,
            after: parts.after,
            trailing: parts.leading,
            page_break: parts.page_break,
        }
    }

    fn paragraph_parts(
        &mut self,
        content: &[Inline],
        look: &ParagraphLook,
        heading: Option<u8>,
        label_width: f64,
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
        let metrics = self.lines.line(&families);
        // The line box is Word's single line, so a paragraph of one line is as tall as Word's.
        let mut text_arguments = format!(
            "font: {}, size: {}, {}",
            font_list(&families),
            points(size),
            metrics.edges()
        );
        if look.font.bold == Some(true) {
            text_arguments.push_str(", weight: \"bold\"");
        }
        if look.font.italic == Some(true) {
            text_arguments.push_str(", style: \"italic\"");
        }
        if let Some(colour) = colour {
            let _ = write!(text_arguments, ", fill: rgb(\"#{colour}\")");
        }

        let single = metrics.single_em * size;
        let line_height = match look.line_spacing {
            None => single,
            Some(LineSpacing::Auto(multiple)) => single * f64::from(multiple) / 240.0,
            Some(LineSpacing::Exact(height)) => f64::from(height) / TWIPS_PER_POINT,
            Some(LineSpacing::AtLeast(height)) => (f64::from(height) / TWIPS_PER_POINT).max(single),
        };
        let twips =
            |value: Option<u32>| value.map_or(0.0, |value| f64::from(value) / TWIPS_PER_POINT);
        let signed =
            |value: Option<i32>| value.map_or(0.0, |value| f64::from(value) / TWIPS_PER_POINT);
        let left = signed(look.indent_left_twips);
        let first_line = signed(look.indent_first_line_twips);
        let tabs = TabContext {
            origin: left + first_line + label_width,
            stops: tab_stops(look, left, first_line),
        };
        let inlines = if content.is_empty() {
            "#hide[x]".to_owned()
        } else {
            self.inlines(content, Some(&tabs), &look.font)
        };
        ParagraphParts {
            text_arguments,
            leading: (line_height - single).max(0.0),
            line_height: line_height.max(1.0),
            inlines,
            before: twips(look.space_before_twips),
            after: twips(look.space_after_twips),
            left,
            right: signed(look.indent_right_twips),
            first_line,
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
        let label = look.list_label.clone().unwrap_or_else(|| ListLabel {
            text: self.list_marker(ordered, level, list_id),
            suffix: LabelSuffix::Tab,
            font: RunLook::default(),
        });
        let look = if look.indent_left_twips.is_some() {
            look.clone()
        } else {
            let left = (f64::from(level) + 1.0) * LIST_INDENT_POINTS;
            ParagraphLook {
                indent_left_twips: Some(twips_of(left)),
                indent_first_line_twips: look
                    .indent_first_line_twips
                    .or(Some(-twips_of(LIST_INDENT_POINTS))),
                ..look.clone()
            }
        };
        self.labelled(&label, content, &look, None, None)
    }

    /// A paragraph that starts with a label: the label in the hanging indent, the text after it.
    fn labelled(
        &mut self,
        label: &ListLabel,
        content: &[Inline],
        look: &ParagraphLook,
        heading: Option<u8>,
        align: Option<Alignment>,
    ) -> Placed {
        let parts = self.paragraph_parts(content, look, heading, 0.0);
        let hanging = if parts.first_line < 0.0 {
            -parts.first_line
        } else {
            0.0
        };
        let label_markup = format!(
            "#{}",
            marked(&label.text, &[], &label.font, &RunLook::default())
        );
        let (columns, gutter) = match label.suffix {
            LabelSuffix::Tab if hanging > 0.0 => (
                format!(
                    "(calc.max({}, measure([{label_markup}]).width + 2pt), 1fr)",
                    points(hanging)
                ),
                "0pt".to_owned(),
            ),
            LabelSuffix::Tab => ("(auto, 1fr)".to_owned(), points(self.default_tab / 2.0)),
            LabelSuffix::Space => ("(auto, 1fr)".to_owned(), "0.25em".to_owned()),
            LabelSuffix::Nothing => ("(auto, 1fr)".to_owned(), "0pt".to_owned()),
        };
        let start = (parts.left - hanging).max(0.0);
        let mut text = format!("par(leading: {})[{}]", points(parts.leading), parts.inlines);
        text = aligned(align, text);
        let mut expression = format!(
            "text({}, context grid(columns: {columns}, column-gutter: {gutter}, [{label_markup}], {text}))",
            parts.text_arguments
        );
        expression = framed(look, expression);
        if start > 0.0 || parts.right > 0.0 {
            expression = format!(
                "pad(left: {}, right: {}, {expression})",
                points(start),
                points(parts.right.max(0.0))
            );
        }
        if let Some(level) = heading {
            expression = format!("heading(level: {})[#{expression}]", level.clamp(1, 6));
        }
        Placed {
            expression,
            before: parts.before,
            after: parts.after,
            trailing: parts.leading,
            page_break: parts.page_break,
        }
    }

    /// The words of a paragraph's content, each as markup and whether it is a tab: a text
    /// breaks where it has a space, a tab is a word of its own, and runs with no space between
    /// them stay one word; `None` for content with no word.
    fn words(
        &mut self,
        content: &[Inline],
        paragraph_font: &RunLook,
    ) -> Option<Vec<(String, bool)>> {
        let mut words = vec![(String::new(), false)];
        for inline in content {
            match inline {
                Inline::Text { text, marks, look } if look.field.is_none() => {
                    for (tab_index, stretch) in text.split('\t').enumerate() {
                        if tab_index > 0 {
                            words.push((String::new(), true));
                            words.push((String::new(), false));
                        }
                        for (index, piece) in stretch.split(' ').enumerate() {
                            if index > 0 {
                                words.push((String::new(), false));
                            }
                            if !piece.is_empty()
                                && let Some((word, _)) = words.last_mut()
                            {
                                word.push('#');
                                word.push_str(&marked(piece, marks, look, paragraph_font));
                            }
                        }
                    }
                }
                other => {
                    let markup = self.inlines(std::slice::from_ref(other), None, paragraph_font);
                    if let Some((word, _)) = words.last_mut() {
                        word.push_str(&markup);
                    }
                }
            }
        }
        words.retain(|(word, tab)| *tab || !word.is_empty());
        words.iter().any(|(_, tab)| !tab).then_some(words)
    }

    /// Inline content as markup, one expression after the other with nothing between them; text
    /// with tabs is laid out against the paragraph's stops.
    fn inlines(
        &mut self,
        content: &[Inline],
        tabs: Option<&TabContext>,
        paragraph_font: &RunLook,
    ) -> String {
        let has_tab = content
            .iter()
            .any(|inline| matches!(inline, Inline::Text { text, .. } if text.contains('\t')));
        let mut segments = vec![String::new()];
        for inline in content {
            match inline {
                Inline::Text { text, marks, look } => {
                    for (index, piece) in text.split('\t').enumerate() {
                        if index > 0 {
                            if has_tab && tabs.is_some() {
                                segments.push(String::new());
                            } else {
                                push_expression(&mut segments, "h(2em)");
                            }
                        }
                        if !piece.is_empty() || look.field.is_some() {
                            let expression = marked(piece, marks, look, paragraph_font);
                            push_expression(&mut segments, &expression);
                        }
                    }
                }
                Inline::HardBreak => push_expression(&mut segments, "linebreak()"),
                Inline::Image {
                    media,
                    width,
                    height,
                    ..
                } => {
                    let image = self.image(*media, *width, *height);
                    push_expression(&mut segments, &image);
                }
                Inline::NoteReference { note } => {
                    let reference = self.note_reference(*note);
                    push_expression(&mut segments, &reference);
                }
                Inline::CommentAnchor { .. } => {}
                Inline::Math { latex, display } => {
                    let math = rely_office_core::latex_to_typst(latex);
                    if *display {
                        // A paragraph holds inline content only: a display equation is set in
                        // display style between springs, on the line the paragraph gives it.
                        push_expression(&mut segments, "h(1fr)");
                        push_expression(&mut segments, &format!("$display({math})$"));
                        push_expression(&mut segments, "h(1fr)");
                    } else {
                        push_expression(&mut segments, &format!("${math}$"));
                    }
                }
            }
        }
        match tabs {
            Some(tabs) if segments.len() > 1 => {
                let mut stops = String::new();
                for stop in &tabs.stops {
                    let kind = match stop.kind {
                        TabKind::Center => "center",
                        TabKind::Right | TabKind::Decimal => "right",
                        TabKind::Left | TabKind::Clear => "left",
                    };
                    let leader = match stop.leader {
                        TabLeader::None => "none".to_owned(),
                        TabLeader::Dot => string_literal("."),
                        TabLeader::Hyphen => string_literal("-"),
                        TabLeader::Underscore => string_literal("_"),
                        TabLeader::MiddleDot => string_literal("\u{b7}"),
                    };
                    let _ = write!(
                        stops,
                        "({}, {}, {leader}), ",
                        points(f64::from(stop.position_twips) / TWIPS_PER_POINT),
                        string_literal(kind)
                    );
                }
                let mut parts = String::new();
                for segment in &segments {
                    let _ = write!(parts, "[{segment}], ");
                }
                format!(
                    "#rely-tabbed({}, {}, ({stops}), ({parts}))",
                    points(tabs.origin),
                    points(self.default_tab)
                )
            }
            _ => segments.concat(),
        }
    }

    /// A footnote where it is referred to, an endnote's number with the note kept for the end.
    fn note_reference(&mut self, note: usize) -> String {
        let Some(content) = self.notes.get(note).filter(|_| !self.in_note) else {
            return "[]".to_owned();
        };
        match content.kind {
            NoteKind::Footnote => {
                self.in_note = true;
                let blocks = self.blocks(&content.blocks);
                self.in_note = false;
                format!("footnote[{blocks}]")
            }
            NoteKind::Endnote => {
                let position =
                    if let Some(known) = self.endnotes.iter().position(|known| *known == note) {
                        known
                    } else {
                        self.endnotes.push(note);
                        self.endnotes.len() - 1
                    };
                format!(
                    "super({})",
                    string_literal(&roman(u32::try_from(position + 1).unwrap_or(1)))
                )
            }
        }
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

    /// The label of a list item whose document names none: bullets, numbers, letters and roman
    /// numerals by level.
    fn list_marker(&mut self, ordered: bool, level: u8, list_id: u32) -> String {
        let level = usize::from(level.min(8));
        let counters = self.list_counters.entry(list_id).or_default();
        counters.resize(level + 1, 0);
        counters[level] += 1;
        if !ordered {
            return ["\u{2022}", "\u{25e6}", "\u{25aa}"][level % 3].to_owned();
        }
        let number = counters[level];
        let label = match level % 3 {
            0 => number.to_string(),
            1 => alphabetic(number),
            _ => roman(number),
        };
        format!("{label}.")
    }

    fn table(&mut self, rows: &[TableRow], look: &TableLook) -> String {
        let placement = place_cells(rows);
        let columns = placement.columns;
        if columns == 0 {
            return "[]".to_owned();
        }
        let row_count = rows.len();
        let header_rows = header_row_count(rows);
        let draws_edges = look.edges.any_visible()
            || rows
                .iter()
                .any(|row| row.cells.iter().any(|cell| cell.look.borders.any_visible()));
        let mut header = Vec::new();
        let mut body = Vec::new();
        for (row_index, row) in rows.iter().enumerate() {
            let in_header = row_index < header_rows;
            for (cell_index, cell) in row.cells.iter().enumerate() {
                let (x, col_span, row_span) = placement.cells[row_index][cell_index];
                let content = self.blocks(&cell.blocks);
                let mut arguments = Vec::new();
                if col_span > 1 {
                    arguments.push(format!("colspan: {col_span}"));
                }
                if row_span > 1 {
                    arguments.push(format!("rowspan: {row_span}"));
                }
                if let Some(fill) = &cell.look.fill {
                    arguments.push(format!("fill: rgb(\"#{fill}\")"));
                }
                if let Some(vertical) = cell.look.vertical {
                    arguments.push(
                        match vertical {
                            VerticalAlignment::Top => "align: top",
                            VerticalAlignment::Center => "align: horizon",
                            VerticalAlignment::Bottom => "align: bottom",
                        }
                        .to_owned(),
                    );
                }
                if draws_edges {
                    let edges = CellEdges {
                        first_row: row_index == 0,
                        last_row: row_index + row_span >= row_count,
                        first_column: x == 0,
                        last_column: x + col_span >= columns,
                    };
                    arguments.push(format!(
                        "stroke: {}",
                        cell_stroke(&cell.look, &look.edges, edges, look.borders)
                    ));
                }
                let cell = if arguments.is_empty() {
                    format!("[{content}]")
                } else {
                    format!("table.cell({})[{content}]", arguments.join(", "))
                };
                if in_header {
                    header.push(cell);
                } else {
                    body.push(cell);
                }
            }
        }
        let stroke = if draws_edges {
            "none"
        } else if look.borders {
            DEFAULT_STROKE
        } else {
            "none"
        };
        let [top, left, bottom, right] = [0, 1, 2, 3].map(|side| {
            f64::from(look.cell_margins_twips[side].unwrap_or(DEFAULT_CELL_MARGINS_TWIPS[side]))
                / TWIPS_PER_POINT
        });
        // Each row is taller by the double rule under it, half above and half below its text.
        let rule = double_rule(rows, &look.edges) / 2.0;
        let (top, bottom) = (top + rule, bottom + rule);
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

/// Add an inline expression to the last segment of a paragraph.
fn push_expression(segments: &mut [String], expression: &str) {
    if let Some(segment) = segments.last_mut() {
        segment.push('#');
        segment.push_str(expression);
    }
}

/// The paragraph's stops a tab can reach, in points from the left margin: its own, and the
/// hanging indent, which Word treats as a stop.
fn tab_stops(look: &ParagraphLook, left: f64, first_line: f64) -> Vec<TabStop> {
    let mut stops: Vec<TabStop> = look
        .tabs
        .iter()
        .filter(|stop| stop.kind != TabKind::Clear)
        .copied()
        .collect();
    if first_line < 0.0 {
        let position = twips_of(left);
        if !stops.iter().any(|stop| stop.position_twips == position) {
            stops.push(TabStop {
                position_twips: position,
                kind: TabKind::Left,
                leader: TabLeader::None,
            });
            stops.sort_by_key(|stop| stop.position_twips);
        }
    }
    stops
}

fn twips_of(points: f64) -> i32 {
    let twips = (points * TWIPS_PER_POINT).round();
    if twips.is_finite() && twips.abs() < 1e9 {
        // The range is checked, so the conversion is exact.
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the value is finite, rounded and within the range of i32"
        )]
        let twips = twips as i32;
        twips
    } else {
        0
    }
}

/// Which outer edges of its table a cell touches.
#[derive(Debug, Clone, Copy)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "one flag per side of the cell, each decided on its own"
)]
struct CellEdges {
    first_row: bool,
    last_row: bool,
    first_column: bool,
    last_column: bool,
}

/// The stroke of each side of a cell: its own border, else the table's outer edge or its inside
/// line, else the default line of a table that draws borders without naming them.
fn cell_stroke(cell: &CellLook, table: &Borders, edges: CellEdges, borders: bool) -> String {
    let pick =
        |own: Option<&Border>, outer: Option<&Border>, inner: Option<&Border>, is_outer: bool| {
            own.or(if is_outer { outer } else { inner }).cloned()
        };
    let sides = [
        (
            "top",
            pick(
                cell.borders.top.as_ref(),
                table.top.as_ref(),
                table.inside_horizontal.as_ref(),
                edges.first_row,
            ),
        ),
        (
            "bottom",
            pick(
                cell.borders.bottom.as_ref(),
                table.bottom.as_ref(),
                table.inside_horizontal.as_ref(),
                edges.last_row,
            ),
        ),
        (
            "left",
            pick(
                cell.borders.left.as_ref(),
                table.left.as_ref(),
                table.inside_vertical.as_ref(),
                edges.first_column,
            ),
        ),
        (
            "right",
            pick(
                cell.borders.right.as_ref(),
                table.right.as_ref(),
                table.inside_vertical.as_ref(),
                edges.last_column,
            ),
        ),
    ];
    let mut stroke = String::from("(");
    for (side, border) in sides {
        let value = match border {
            Some(border) => border_stroke(&border),
            None if borders && !table.any_visible() => DEFAULT_STROKE.to_owned(),
            None => "none".to_owned(),
        };
        let _ = write!(stroke, "{side}: {value}, ");
    }
    stroke.push(')');
    stroke
}

/// How much room a double rule between rows takes, its two lines and the gap between them:
/// the widest of the table's and its cells' top and bottom double borders.
fn double_rule(rows: &[TableRow], table: &Borders) -> f64 {
    let cells = rows
        .iter()
        .flat_map(|row| &row.cells)
        .flat_map(|cell| [&cell.look.borders.top, &cell.look.borders.bottom]);
    [&table.top, &table.bottom, &table.inside_horizontal]
        .into_iter()
        .chain(cells)
        .flatten()
        .filter(|border| border.style == BorderStyle::Double)
        .map(|border| f64::from(border.width_eighths) / 8.0 * 3.0)
        .fold(0.0, f64::max)
}

/// A Typst stroke for a border edge.
fn border_stroke(border: &Border) -> String {
    if border.style == BorderStyle::None {
        return "none".to_owned();
    }
    let width = f64::from(border.width_eighths) / 8.0;
    let width = if border.style == BorderStyle::Double {
        width * 2.0
    } else {
        width
    };
    let paint = border.colour.as_ref().map_or_else(
        || "black".to_owned(),
        |colour| format!("rgb(\"#{colour}\")"),
    );
    let dash = match border.style {
        BorderStyle::Dotted => ", dash: \"dotted\"",
        BorderStyle::Dashed => ", dash: \"dashed\"",
        _ => "",
    };
    format!("(paint: {paint}, thickness: {}{dash})", points(width))
}

/// A paragraph's shading and borders drawn around it, outside its text as Word draws them.
fn framed(look: &ParagraphLook, expression: String) -> String {
    let borders = &look.borders;
    let visible = borders.any_visible();
    if look.shading.is_none() && !visible {
        return expression;
    }
    let mut arguments = String::from("width: 100%");
    if let Some(fill) = &look.shading {
        let _ = write!(arguments, ", fill: rgb(\"#{fill}\")");
    }
    if visible {
        let side = |border: &Option<Border>| {
            border
                .as_ref()
                .map_or_else(|| "none".to_owned(), border_stroke)
        };
        let _ = write!(
            arguments,
            ", stroke: (top: {}, right: {}, bottom: {}, left: {})",
            side(&borders.top),
            side(&borders.right),
            side(&borders.bottom),
            side(&borders.left)
        );
    }
    let [top, right, bottom, left] = PARAGRAPH_BORDER_SPACE;
    let _ = write!(
        arguments,
        ", outset: (top: {}, right: {}, bottom: {}, left: {})",
        points(top),
        points(right),
        points(bottom),
        points(left)
    );
    format!("block({arguments}, {expression})")
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

/// Where every cell lands once each takes the first free place, as Typst places them.
struct Placement {
    /// For each row, each cell's first column, column span and row span, cut to the table.
    cells: Vec<Vec<(usize, usize, usize)>>,
    columns: usize,
}

fn place_cells(rows: &[TableRow]) -> Placement {
    let mut occupied: Vec<Vec<bool>> = Vec::new();
    let mut columns = 0;
    let mut cells = Vec::with_capacity(rows.len());
    for (row_index, row) in rows.iter().enumerate() {
        let mut column = 0;
        let mut placed_row = Vec::with_capacity(row.cells.len());
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
            placed_row.push((column, col_span, row_span));
            column += col_span;
            columns = columns.max(column);
        }
        cells.push(placed_row);
    }
    let columns = columns
        .max(occupied.iter().map(Vec::len).max().unwrap_or(0))
        .min(MAX_COLUMNS);
    for row in &mut cells {
        for (column, col_span, _) in row.iter_mut() {
            *col_span = (*col_span).min(columns.saturating_sub(*column).max(1));
        }
    }
    Placement { cells, columns }
}

fn aligned(align: Option<Alignment>, expression: String) -> String {
    match align {
        Some(Alignment::Center) => format!("align(center, {expression})"),
        Some(Alignment::Right) => format!("align(right, {expression})"),
        _ => expression,
    }
}

/// A text run with its font and marks, innermost first in the order they are listed; a page
/// number field shows the page it is drawn on.
fn marked(text: &str, marks: &[Mark], look: &RunLook, paragraph_font: &RunLook) -> String {
    let mut expression = match look.field {
        Some(PageField::Page) => "[#context counter(page).display()]".to_owned(),
        Some(PageField::Pages) => "[#context str(counter(page).final().first())]".to_owned(),
        None => string_literal(text),
    };
    if look.caps.or(paragraph_font.caps) == Some(true) {
        expression = format!("upper({expression})");
    } else if look.small_caps.or(paragraph_font.small_caps) == Some(true) {
        expression = format!("smallcaps({expression})");
    }
    let mut font = Vec::new();
    if look.bold == Some(true) {
        font.push("weight: \"bold\"".to_owned());
    }
    if look.italic == Some(true) {
        font.push("style: \"italic\"".to_owned());
    }
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
            // Changes show as Word shows them with markup on.
            Mark::Insertion { .. } => {
                format!("text(fill: rgb(\"{REVISION_COLOUR}\"), underline({expression}))")
            }
            Mark::Deletion { .. } => {
                format!("text(fill: rgb(\"{REVISION_COLOUR}\"), strike({expression}))")
            }
            // The look already draws the run's own font, size and colour; a comment is not
            // printed.
            Mark::TextStyle { .. } | Mark::Highlight { .. } | Mark::Comment { .. } => expression,
        };
    }
    if let Some(highlight) = look
        .highlight
        .as_ref()
        .or(paragraph_font.highlight.as_ref())
    {
        expression = format!("highlight(fill: rgb(\"#{highlight}\"), {expression})");
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

mod floats;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod parts_tests;
