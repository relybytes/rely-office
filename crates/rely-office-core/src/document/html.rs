// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! HTML read into the document tree and written from it, as a page that stands alone.
//!
//! The reader keeps the text and its structure: headings, paragraphs, nested lists, tables with
//! merged cells, quotes, preformatted code, rules, images in `data:` addresses and the marks
//! a run carries, font, size and colour of a `style` included. Scripts, styles, frames, forms
//! and every attribute it does not read are dropped, so what it gives is safe to show; an image
//! that points elsewhere is read as a link, since the reader fetches nothing. The writer gives
//! what the tree holds back as HTML, its notes as a numbered list at the end.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use html5gum::{DefaultEmitter, HtmlString, Token, Tokenizer};

use super::markdown::{is_safe_link, mark_order};
use super::{
    Alignment, Block, CellLook, DocumentTree, Inline, Mark, Media, Note, NoteKind, ParagraphLook,
    RunLook, TableCell, TableLook, TableRow, sniff_image_type,
};
use crate::{Limits, OfficeError};

/// Elements whose content is never text of the page.
const SKIPPED: [&str; 12] = [
    "script", "style", "head", "title", "template", "noscript", "iframe", "object", "embed", "svg",
    "math", "select",
];

/// Elements that end the paragraph being read without opening one of their own.
const SECTIONS: [&str; 16] = [
    "div", "section", "article", "header", "footer", "main", "nav", "aside", "figure", "body",
    "html", "address", "form", "fieldset", "details", "summary",
];

/// Read HTML as a document tree.
///
/// # Errors
///
/// [`OfficeError::TooLarge`] past [`Limits::max_input_bytes`].
pub fn read_html_tree(text: &str, limits: &Limits) -> Result<DocumentTree, OfficeError> {
    if text.len() as u64 > limits.max_input_bytes {
        return Err(OfficeError::TooLarge {
            what: "input".to_owned(),
            limit: limits.max_input_bytes,
        });
    }
    let mut emitter = DefaultEmitter::default();
    emitter.naively_switch_states(true);
    let mut reader = Reader::default();
    for token in Tokenizer::new_with_emitter(text, emitter).flatten() {
        reader.token(token);
    }
    Ok(reader.finish())
}

/// What the text being read becomes when its block ends.
#[derive(Clone, PartialEq)]
enum Pending {
    Paragraph(Option<Alignment>),
    Heading(u8, Option<Alignment>),
    Item {
        ordered: bool,
        level: u8,
        list_id: u32,
    },
}

struct OpenList {
    ordered: bool,
    id: u32,
    /// The list holds the notes the writer puts at the end of a page.
    notes: bool,
}

struct OpenTable {
    rows: Vec<TableRow>,
    /// The cell being read: its spans, whether it is a header cell, and the blocks read so far.
    cell: Option<(u32, u32, bool, Vec<Block>)>,
    in_head: bool,
}

/// An inline element open in the text, with a mark it gave; an element giving several marks
/// is several entries of one `element`.
struct OpenInline {
    name: String,
    element: usize,
    mark: Option<Mark>,
}

struct Reader {
    /// The blocks of the page, of each open quote and of each open table cell, innermost last.
    containers: Vec<Vec<Block>>,
    /// Which containers are quotes, so their end closes them.
    quotes: usize,
    inlines: Vec<Inline>,
    pending: Vec<Pending>,
    inline_stack: Vec<OpenInline>,
    next_element: usize,
    lists: Vec<OpenList>,
    next_list_id: u32,
    tables: Vec<OpenTable>,
    /// The depth of skipped elements the reader is inside.
    skipping: Vec<String>,
    preformatted: Option<String>,
    /// Inside a note reference link: its text is the number, already given by the reference.
    in_note_reference: bool,
    /// Inside the list of notes: the index of the note being read.
    note_item: Option<usize>,
    note_blocks: Vec<Block>,
    note_ids: BTreeMap<String, usize>,
    math: Option<(String, bool)>,
    media: Vec<Media>,
    notes: Vec<Note>,
}

impl Default for Reader {
    fn default() -> Self {
        Self {
            containers: vec![Vec::new()],
            quotes: 0,
            inlines: Vec::new(),
            pending: Vec::new(),
            inline_stack: Vec::new(),
            next_element: 0,
            lists: Vec::new(),
            next_list_id: 1,
            tables: Vec::new(),
            skipping: Vec::new(),
            preformatted: None,
            in_note_reference: false,
            note_item: None,
            note_blocks: Vec::new(),
            note_ids: BTreeMap::new(),
            math: None,
            media: Vec::new(),
            notes: Vec::new(),
        }
    }
}

fn name_of(name: &HtmlString) -> String {
    String::from_utf8_lossy(name).to_ascii_lowercase()
}

fn attribute(
    attributes: &BTreeMap<HtmlString, html5gum::Spanned<HtmlString, ()>>,
    name: &str,
) -> Option<String> {
    attributes
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name.as_bytes()))
        .map(|(_, value)| String::from_utf8_lossy(&value.value).into_owned())
}

/// The declarations of a `style` attribute, names lower-cased.
fn declarations(style: &str) -> Vec<(String, String)> {
    style
        .split(';')
        .filter_map(|declaration| {
            let (name, value) = declaration.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        })
        .collect()
}

fn alignment_of(
    attributes: &BTreeMap<HtmlString, html5gum::Spanned<HtmlString, ()>>,
) -> Option<Alignment> {
    let from_style = attribute(attributes, "style").and_then(|style| {
        declarations(&style)
            .into_iter()
            .find(|(name, _)| name == "text-align")
            .map(|(_, value)| value)
    });
    let value = from_style.or_else(|| attribute(attributes, "align"))?;
    match value.trim().to_ascii_lowercase().as_str() {
        "center" => Some(Alignment::Center),
        "right" | "end" => Some(Alignment::Right),
        "justify" => Some(Alignment::Justify),
        "left" | "start" => Some(Alignment::Left),
        _ => None,
    }
}

impl Reader {
    fn token(&mut self, token: Token) {
        match token {
            Token::StartTag(tag) => {
                let name = name_of(&tag.name);
                if !self.skipping.is_empty() {
                    if !tag.self_closing && SKIPPED.contains(&name.as_str()) {
                        self.skipping.push(name);
                    }
                    return;
                }
                if SKIPPED.contains(&name.as_str()) {
                    if !tag.self_closing {
                        self.skipping.push(name);
                    }
                    return;
                }
                self.start(&name, &tag.attributes);
            }
            Token::EndTag(tag) => {
                let name = name_of(&tag.name);
                if !self.skipping.is_empty() {
                    if self.skipping.last() == Some(&name) {
                        self.skipping.pop();
                    }
                    return;
                }
                self.end(&name);
            }
            Token::String(text) => {
                if self.skipping.is_empty() {
                    self.text(&String::from_utf8_lossy(&text.value));
                }
            }
            Token::Comment(_) | Token::Doctype(_) | Token::Error(_) => {}
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one match over the elements a page can open"
    )]
    fn start(
        &mut self,
        name: &str,
        attributes: &BTreeMap<HtmlString, html5gum::Spanned<HtmlString, ()>>,
    ) {
        if self.preformatted.is_some() {
            if name == "br" {
                self.text("\n");
            }
            return;
        }
        match name {
            "p" | "dt" | "dd" | "figcaption" | "caption" | "legend" => {
                self.flush();
                let inherited = match self.pending.last() {
                    Some(Pending::Item { .. }) => self.pending.last().cloned(),
                    _ => None,
                };
                self.pending
                    .push(inherited.unwrap_or(Pending::Paragraph(alignment_of(attributes))));
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.flush();
                let level = name[1..].parse().unwrap_or(1);
                self.pending
                    .push(Pending::Heading(level, alignment_of(attributes)));
            }
            _ if SECTIONS.contains(&name) => {
                self.flush();
                if let Some(align) = alignment_of(attributes) {
                    self.pending.push(Pending::Paragraph(Some(align)));
                } else {
                    self.pending.push(
                        self.pending
                            .last()
                            .cloned()
                            .unwrap_or(Pending::Paragraph(None)),
                    );
                }
            }
            "ul" | "ol" | "menu" => {
                self.flush();
                let notes = attribute(attributes, "class")
                    .is_some_and(|class| class.split_whitespace().any(|word| word == "notes"));
                let id = if let Some(parent) = self.lists.last() {
                    parent.id
                } else {
                    self.next_list_id += 1;
                    self.next_list_id - 1
                };
                self.lists.push(OpenList {
                    ordered: name == "ol",
                    id,
                    notes,
                });
            }
            "li" => {
                self.flush();
                if self.lists.last().is_some_and(|list| list.notes) {
                    self.finish_note();
                    let id = attribute(attributes, "id").unwrap_or_default();
                    let index = self.note_index(&id);
                    self.note_item = Some(index);
                    self.pending.push(Pending::Paragraph(None));
                    return;
                }
                let level = u8::try_from(self.lists.len().saturating_sub(1)).unwrap_or(u8::MAX);
                let (ordered, list_id) = self
                    .lists
                    .last()
                    .map_or((false, 0), |list| (list.ordered, list.id));
                let list_id = if self.lists.is_empty() {
                    self.next_list_id += 1;
                    self.next_list_id - 1
                } else {
                    list_id
                };
                self.pending.push(Pending::Item {
                    ordered,
                    level,
                    list_id,
                });
            }
            "blockquote" => {
                self.flush();
                self.containers.push(Vec::new());
                self.quotes += 1;
                self.pending.push(Pending::Paragraph(None));
            }
            "pre" => {
                self.flush();
                self.preformatted = Some(String::new());
            }
            "table" => {
                self.flush();
                self.tables.push(OpenTable {
                    rows: Vec::new(),
                    cell: None,
                    in_head: false,
                });
            }
            "thead" => {
                if let Some(table) = self.tables.last_mut() {
                    table.in_head = true;
                }
            }
            "tbody" | "tfoot" => {
                if let Some(table) = self.tables.last_mut() {
                    table.in_head = false;
                }
            }
            "tr" => {
                self.close_cell();
                if let Some(table) = self.tables.last_mut() {
                    let header = table.in_head;
                    table.rows.push(TableRow {
                        header,
                        cells: Vec::new(),
                    });
                }
            }
            "td" | "th" => {
                self.close_cell();
                let span = |name: &str| {
                    attribute(attributes, name)
                        .and_then(|value| value.trim().parse::<u32>().ok())
                        .unwrap_or(1)
                        .clamp(1, 1000)
                };
                let (col_span, row_span) = (span("colspan"), span("rowspan"));
                if let Some(table) = self.tables.last_mut() {
                    if table.rows.is_empty() {
                        table.rows.push(TableRow {
                            header: table.in_head,
                            cells: Vec::new(),
                        });
                    }
                    table.cell = Some((col_span, row_span, name == "th", Vec::new()));
                    self.containers.push(Vec::new());
                    self.pending
                        .push(Pending::Paragraph(alignment_of(attributes)));
                }
            }
            "hr" => {
                self.flush();
                self.blocks().push(Block::HorizontalRule);
            }
            "br" => self.inlines.push(Inline::HardBreak),
            "img" => self.image(attributes),
            "span" if attribute(attributes, "data-latex").is_some() => {
                let display = attribute(attributes, "class")
                    .is_some_and(|class| class.split_whitespace().any(|word| word == "display"));
                self.math = Some((
                    attribute(attributes, "data-latex").unwrap_or_default(),
                    display,
                ));
            }
            "a" if attribute(attributes, "data-note").is_some() => {
                let id = attribute(attributes, "href")
                    .map(|href| href.trim_start_matches('#').to_owned())
                    .unwrap_or_default();
                let note = self.note_index(&id);
                self.inlines.push(Inline::NoteReference { note });
                self.in_note_reference = true;
            }
            _ => {
                let mark = inline_mark(name, attributes);
                let mut marks = Vec::new();
                if let Some(mark) = mark {
                    marks.push(mark);
                }
                // A span's style can say several things at once; each is its own mark.
                if let Some(style) = attribute(attributes, "style") {
                    marks.extend(style_marks(&style));
                }
                let element = self.next_element;
                self.next_element += 1;
                if marks.is_empty() {
                    self.inline_stack.push(OpenInline {
                        name: name.to_owned(),
                        element,
                        mark: None,
                    });
                }
                for mark in marks {
                    self.inline_stack.push(OpenInline {
                        name: name.to_owned(),
                        element,
                        mark: Some(mark),
                    });
                }
            }
        }
    }

    fn end(&mut self, name: &str) {
        if let Some(code) = &self.preformatted {
            if name == "pre" {
                let mut text = code.clone();
                self.preformatted = None;
                if text.starts_with('\n') {
                    text.remove(0);
                }
                while text.ends_with('\n') {
                    text.pop();
                }
                self.blocks().push(Block::CodeBlock { text });
            }
            return;
        }
        match name {
            "p" | "dt" | "dd" | "figcaption" | "caption" | "legend" | "h1" | "h2" | "h3" | "h4"
            | "h5" | "h6" | "li" => {
                self.flush();
                self.pending.pop();
            }
            _ if SECTIONS.contains(&name) => {
                self.flush();
                self.pending.pop();
            }
            "ul" | "ol" | "menu" => {
                self.flush();
                if self.lists.last().is_some_and(|list| list.notes) {
                    self.finish_note();
                }
                if let Some(list) = self.lists.pop() {
                    // An item left open by a missing `</li>` ends with its list.
                    while let Some(Pending::Item { list_id, level, .. }) = self.pending.last()
                        && *list_id == list.id
                        && usize::from(*level) >= self.lists.len()
                    {
                        self.pending.pop();
                    }
                }
            }
            "blockquote" => {
                self.flush();
                if self.quotes > 0 && self.containers.len() > 1 {
                    self.quotes -= 1;
                    self.pending.pop();
                    let blocks = self.containers.pop().unwrap_or_default();
                    self.blocks().push(Block::Blockquote { blocks });
                }
            }
            "td" | "th" | "tr" => self.close_cell(),
            "thead" => {
                if let Some(table) = self.tables.last_mut() {
                    table.in_head = false;
                }
            }
            "table" => {
                self.close_cell();
                if let Some(table) = self.tables.pop() {
                    let rows: Vec<TableRow> = table
                        .rows
                        .into_iter()
                        .filter(|row| !row.cells.is_empty())
                        .collect();
                    if !rows.is_empty() {
                        self.blocks().push(Block::Table {
                            rows,
                            look: TableLook::default(),
                        });
                    }
                }
            }
            "a" if self.in_note_reference => self.in_note_reference = false,
            "span" if self.math.is_some() => {
                if let Some((latex, display)) = self.math.take() {
                    self.inlines.push(Inline::Math { latex, display });
                }
            }
            _ => {
                if let Some(open) = self
                    .inline_stack
                    .iter()
                    .rev()
                    .find(|open| open.name == name)
                {
                    let element = open.element;
                    self.inline_stack.retain(|open| open.element != element);
                }
            }
        }
    }

    fn text(&mut self, text: &str) {
        if let Some(code) = &mut self.preformatted {
            code.push_str(text);
            return;
        }
        if self.in_note_reference || self.math.is_some() {
            return;
        }
        let mut collapsed = String::with_capacity(text.len());
        let mut space = false;
        for character in text.chars() {
            if character.is_whitespace() && character != '\u{a0}' {
                space = true;
            } else {
                if space {
                    collapsed.push(' ');
                }
                space = false;
                collapsed.push(character);
            }
        }
        if space {
            collapsed.push(' ');
        }
        let at_start = match self.inlines.last() {
            None | Some(Inline::HardBreak) => true,
            Some(Inline::Text { text, .. }) => text.ends_with(' '),
            Some(_) => false,
        };
        let collapsed = if at_start {
            collapsed.trim_start().to_owned()
        } else {
            collapsed
        };
        if collapsed.is_empty() {
            return;
        }
        let marks = self.current_marks();
        if let Some(Inline::Text {
            text: previous,
            marks: previous_marks,
            ..
        }) = self.inlines.last_mut()
            && *previous_marks == marks
        {
            previous.push_str(&collapsed);
            return;
        }
        self.inlines.push(Inline::Text {
            text: collapsed,
            marks,
            look: RunLook::default(),
        });
    }

    /// The marks of the open elements: an inner font, size, colour, highlight or link over an
    /// outer one.
    fn current_marks(&self) -> Vec<Mark> {
        let mut marks: Vec<Mark> = Vec::new();
        for mark in self
            .inline_stack
            .iter()
            .filter_map(|open| open.mark.as_ref())
        {
            let same = marks
                .iter()
                .position(|known| std::mem::discriminant(known) == std::mem::discriminant(mark));
            match (same, mark) {
                (
                    Some(at),
                    Mark::TextStyle {
                        font_family,
                        font_size,
                        color,
                    },
                ) => {
                    if let Mark::TextStyle {
                        font_family: known_family,
                        font_size: known_size,
                        color: known_color,
                    } = &mut marks[at]
                    {
                        if font_family.is_some() {
                            known_family.clone_from(font_family);
                        }
                        if font_size.is_some() {
                            known_size.clone_from(font_size);
                        }
                        if color.is_some() {
                            known_color.clone_from(color);
                        }
                    }
                }
                (Some(at), _) => marks[at] = mark.clone(),
                (None, _) => marks.push(mark.clone()),
            }
        }
        marks.sort_by_key(mark_order);
        marks
    }

    fn image(&mut self, attributes: &BTreeMap<HtmlString, html5gum::Spanned<HtmlString, ()>>) {
        let source = attribute(attributes, "src").unwrap_or_default();
        let alt = attribute(attributes, "alt").filter(|alt| !alt.trim().is_empty());
        let size = |name: &str| {
            attribute(attributes, name)
                .and_then(|value| value.trim().trim_end_matches("px").parse::<u32>().ok())
                .filter(|size| (1..=10_000).contains(size))
        };
        if let Some((_, bytes)) = crate::base64::data_url(&source) {
            let content_type = sniff_image_type(&bytes);
            if content_type.starts_with("image/") {
                self.media.push(Media {
                    content_type,
                    bytes,
                });
                self.inlines.push(Inline::Image {
                    media: self.media.len() - 1,
                    alt,
                    width: size("width"),
                    height: size("height"),
                    anchor: None,
                });
                return;
            }
        }
        let shown = alt.unwrap_or_else(|| source.clone());
        if shown.is_empty() {
            return;
        }
        let marks = if is_safe_link(&source) {
            vec![Mark::Link { href: source }]
        } else {
            Vec::new()
        };
        self.inlines.push(Inline::Text {
            text: shown,
            marks,
            look: RunLook::default(),
        });
    }

    /// Write the text read so far as the block its element makes.
    fn flush(&mut self) {
        let mut content = std::mem::take(&mut self.inlines);
        while matches!(content.last(), Some(Inline::HardBreak)) {
            content.pop();
        }
        if let Some(Inline::Text { text, .. }) = content.last_mut() {
            let trimmed = text.trim_end().len();
            text.truncate(trimmed);
        }
        content.retain(|inline| !matches!(inline, Inline::Text { text, .. } if text.is_empty()));
        if content.is_empty() {
            return;
        }
        let block = match self
            .pending
            .last()
            .cloned()
            .unwrap_or(Pending::Paragraph(None))
        {
            Pending::Paragraph(align) => Block::Paragraph {
                content,
                align,
                look: ParagraphLook::default(),
            },
            Pending::Heading(level, align) => Block::Heading {
                level: level.clamp(1, 6),
                content,
                align,
                look: ParagraphLook::default(),
            },
            Pending::Item {
                ordered,
                level,
                list_id,
            } => Block::ListItem {
                ordered,
                level,
                list_id,
                content,
                look: ParagraphLook::default(),
            },
        };
        if self.note_item.is_some() {
            self.note_blocks.push(block);
        } else {
            self.blocks().push(block);
        }
    }

    fn close_cell(&mut self) {
        let open = self.tables.last().is_some_and(|table| table.cell.is_some());
        if !open {
            return;
        }
        self.flush();
        self.pending.pop();
        let blocks = if self.containers.len() > 1 {
            self.containers.pop().unwrap_or_default()
        } else {
            Vec::new()
        };
        if let Some(table) = self.tables.last_mut()
            && let Some((col_span, row_span, header_cell, _)) = table.cell.take()
            && let Some(row) = table.rows.last_mut()
        {
            // A row is a header when the table's head holds it or every one of its cells is a `th`.
            if row.cells.is_empty() {
                row.header = header_cell || table.in_head;
            } else if !header_cell {
                row.header = table.in_head;
            }
            row.cells.push(TableCell {
                col_span,
                row_span,
                blocks,
                look: CellLook::default(),
            });
        }
    }

    fn finish_note(&mut self) {
        self.flush();
        if let Some(index) = self.note_item.take() {
            let blocks = std::mem::take(&mut self.note_blocks);
            if let Some(note) = self.notes.get_mut(index) {
                note.blocks = blocks;
            }
        }
    }

    fn note_index(&mut self, id: &str) -> usize {
        if let Some(index) = self.note_ids.get(id) {
            return *index;
        }
        self.notes.push(Note {
            kind: NoteKind::Footnote,
            blocks: Vec::new(),
        });
        let index = self.notes.len() - 1;
        self.note_ids.insert(id.to_owned(), index);
        index
    }

    fn blocks(&mut self) -> &mut Vec<Block> {
        if self.containers.is_empty() {
            self.containers.push(Vec::new());
        }
        let last = self.containers.len() - 1;
        &mut self.containers[last]
    }

    fn finish(mut self) -> DocumentTree {
        self.flush();
        while self.tables.last().is_some() {
            self.end("table");
        }
        let mut blocks = Vec::new();
        for container in self.containers {
            blocks.extend(container);
        }
        DocumentTree {
            blocks,
            media: self.media,
            notes: self.notes,
            ..DocumentTree::default()
        }
    }
}

fn inline_mark(
    name: &str,
    attributes: &BTreeMap<HtmlString, html5gum::Spanned<HtmlString, ()>>,
) -> Option<Mark> {
    Some(match name {
        "b" | "strong" => Mark::Bold,
        "i" | "em" | "cite" | "dfn" | "var" => Mark::Italic,
        "u" | "ins" => Mark::Underline,
        "s" | "strike" | "del" => Mark::Strike,
        "code" | "kbd" | "samp" | "tt" => Mark::Code,
        "sup" => Mark::Superscript,
        "sub" => Mark::Subscript,
        "mark" => Mark::Highlight {
            color: attribute(attributes, "style")
                .and_then(|style| {
                    declarations(&style)
                        .into_iter()
                        .find(|(name, _)| name == "background-color" || name == "background")
                        .and_then(|(_, value)| css_colour(&value))
                })
                .unwrap_or_else(|| "#FFFF00".to_owned()),
        },
        "a" => {
            let href = attribute(attributes, "href")?;
            if !is_safe_link(&href) {
                return None;
            }
            Mark::Link {
                href: href.trim().to_owned(),
            }
        }
        "font" => {
            let color = attribute(attributes, "color").and_then(|value| css_colour(&value));
            let font_family = attribute(attributes, "face").map(|face| first_family(&face));
            if color.is_none() && font_family.is_none() {
                return None;
            }
            Mark::TextStyle {
                font_family,
                font_size: None,
                color,
            }
        }
        _ => return None,
    })
}

/// The marks a `style` attribute gives: weight, slant, decoration, font, size, colour and
/// background.
fn style_marks(style: &str) -> Vec<Mark> {
    let mut marks = Vec::new();
    let (mut font_family, mut font_size, mut color) = (None, None, None);
    for (name, value) in declarations(style) {
        let lower = value.to_ascii_lowercase();
        match name.as_str() {
            "font-weight"
                if lower == "bold" || lower.parse::<u32>().is_ok_and(|weight| weight >= 600) =>
            {
                marks.push(Mark::Bold);
            }
            "font-style" if lower == "italic" || lower == "oblique" => marks.push(Mark::Italic),
            "text-decoration" | "text-decoration-line" => {
                if lower.contains("underline") {
                    marks.push(Mark::Underline);
                }
                if lower.contains("line-through") {
                    marks.push(Mark::Strike);
                }
            }
            "vertical-align" if lower == "super" => marks.push(Mark::Superscript),
            "vertical-align" if lower == "sub" => marks.push(Mark::Subscript),
            "font-family" => font_family = Some(first_family(&value)),
            "font-size" if lower.ends_with("pt") || lower.ends_with("px") => {
                font_size = css_points(&lower);
            }
            "color" => color = css_colour(&value),
            "background-color" | "background" => {
                if let Some(colour) = css_colour(&value) {
                    marks.push(Mark::Highlight { color: colour });
                }
            }
            _ => {}
        }
    }
    if font_family.is_some() || font_size.is_some() || color.is_some() {
        marks.push(Mark::TextStyle {
            font_family,
            font_size,
            color,
        });
    }
    marks
}

fn first_family(value: &str) -> String {
    value
        .split(',')
        .next()
        .unwrap_or_default()
        .trim()
        .trim_matches(['"', '\''])
        .to_owned()
}

/// A CSS length in points, as the tree writes a size: `"12pt"`, from `16px` too.
fn css_points(value: &str) -> Option<String> {
    let (number, points_per_unit) = if let Some(number) = value.strip_suffix("pt") {
        (number, 1.0)
    } else {
        (value.strip_suffix("px")?, 0.75)
    };
    let points = number.trim().parse::<f64>().ok()? * points_per_unit;
    if !(1.0..=400.0).contains(&points) {
        return None;
    }
    let half_points = (points * 2.0).round() / 2.0;
    Some(if half_points.fract() == 0.0 {
        format!("{half_points:.0}pt")
    } else {
        format!("{half_points:.1}pt")
    })
}

/// A CSS colour as `#RRGGBB`: hex, `rgb()` and the named colours a document commonly uses.
fn css_colour(value: &str) -> Option<String> {
    let value = value.trim().to_ascii_lowercase();
    if let Some(hex) = value.strip_prefix('#') {
        let hex = match hex.len() {
            3 => hex
                .chars()
                .flat_map(|digit| [digit, digit])
                .collect::<String>(),
            6 => hex.to_owned(),
            _ => return None,
        };
        return hex
            .chars()
            .all(|digit| digit.is_ascii_hexdigit())
            .then(|| format!("#{}", hex.to_ascii_uppercase()));
    }
    if let Some(inner) = value
        .strip_prefix("rgb(")
        .or_else(|| value.strip_prefix("rgba("))
    {
        let channels: Vec<u8> = inner
            .trim_end_matches(')')
            .split(',')
            .take(3)
            .filter_map(|channel| channel.trim().parse::<f64>().ok())
            .map(|channel| {
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "clamped to 0..=255"
                )]
                let byte = channel.clamp(0.0, 255.0).round() as u8;
                byte
            })
            .collect();
        return (channels.len() == 3)
            .then(|| format!("#{:02X}{:02X}{:02X}", channels[0], channels[1], channels[2]));
    }
    let named = match value.as_str() {
        "black" => "000000",
        "white" => "FFFFFF",
        "red" => "FF0000",
        "green" => "008000",
        "blue" => "0000FF",
        "yellow" => "FFFF00",
        "orange" => "FFA500",
        "gray" | "grey" => "808080",
        "purple" => "800080",
        "navy" => "000080",
        "maroon" => "800000",
        "teal" => "008080",
        _ => return None,
    };
    Some(format!("#{named}"))
}

/// A document tree as a page of HTML that stands alone: its images in `data:` addresses, its
/// notes as a numbered list at the end. Tracked deletions are left out and insertions kept, as
/// The CSS a floating image keeps in a page: beside the text on its side, or on a line of its
/// own.
fn float_style(anchor: &super::Anchor) -> &'static str {
    use super::{AnchorAlign, Wrap};
    if matches!(anchor.wrap, Wrap::TopAndBottom)
        || anchor.horizontal.align == Some(AnchorAlign::Center)
    {
        return match anchor.horizontal.align {
            Some(AnchorAlign::Center) => "display:block;margin:0.5em auto",
            Some(AnchorAlign::End) => "display:block;margin:0.5em 0 0.5em auto",
            _ => "display:block;margin:0.5em 0",
        };
    }
    let right = anchor.horizontal.align == Some(AnchorAlign::End)
        || (anchor.horizontal.align.is_none() && anchor.horizontal.offset_emu > 2_743_200);
    if right {
        "float:right;margin:0.25em 0 0.5em 1em"
    } else {
        "float:left;margin:0.25em 1em 0.5em 0"
    }
}

/// if the changes were accepted.
#[must_use]
pub fn write_html(tree: &DocumentTree, title: &str) -> String {
    let mut writer = HtmlWriter {
        tree,
        notes: Vec::new(),
    };
    let body = writer.blocks(&tree.blocks);
    let mut page = String::from("<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n");
    let _ = writeln!(page, "<title>{}</title>", escape_text(title));
    page.push_str(
        "<style>body{font-family:sans-serif;max-width:48em;margin:2em auto;line-height:1.5}\
         table{border-collapse:collapse}td,th{border:1px solid #999;padding:4px 8px}\
         blockquote{border-left:3px solid #ccc;margin-left:0;padding-left:1em}</style>\n",
    );
    page.push_str("</head>\n<body>\n");
    page.push_str(&body);
    if !writer.notes.is_empty() {
        page.push_str("<ol class=\"notes\">\n");
        let notes = std::mem::take(&mut writer.notes);
        for (number, note) in notes.iter().enumerate() {
            let content = writer.blocks(&tree.notes[*note].blocks);
            let _ = writeln!(
                page,
                "<li id=\"note-{}\">{}</li>",
                number + 1,
                content.trim_end()
            );
        }
        page.push_str("</ol>\n");
    }
    page.push_str("</body>\n</html>\n");
    page
}

struct HtmlWriter<'t> {
    tree: &'t DocumentTree,
    notes: Vec<usize>,
}

impl HtmlWriter<'_> {
    fn blocks(&mut self, blocks: &[Block]) -> String {
        let mut html = String::new();
        // The open lists, innermost last: whether ordered, the list id, and whether an item of
        // it is open, a deeper list going inside that item.
        let mut open: Vec<(bool, u32, bool)> = Vec::new();
        for block in blocks {
            if let Block::ListItem {
                ordered,
                level,
                list_id,
                content,
                ..
            } = block
            {
                let depth = usize::from(*level) + 1;
                if open.first().is_some_and(|(_, id, _)| id != list_id) {
                    close_lists(&mut html, &mut open, 0);
                }
                close_lists(&mut html, &mut open, depth);
                if open.len() == depth && open.last().is_some_and(|(kind, _, _)| kind != ordered) {
                    close_lists(&mut html, &mut open, depth - 1);
                }
                let siblings = open.len() == depth;
                if let Some(last) = open.last_mut()
                    && siblings
                    && last.2
                {
                    html.push_str("</li>\n");
                    last.2 = false;
                }
                while open.len() < depth {
                    html.push_str(if *ordered { "<ol>\n" } else { "<ul>\n" });
                    open.push((*ordered, *list_id, false));
                }
                let _ = write!(html, "<li>{}", self.inlines(content));
                if let Some(last) = open.last_mut() {
                    last.2 = true;
                }
                continue;
            }
            close_lists(&mut html, &mut open, 0);
            html.push_str(&self.block(block));
            html.push('\n');
        }
        close_lists(&mut html, &mut open, 0);
        html
    }

    fn block(&mut self, block: &Block) -> String {
        match block {
            Block::Heading {
                level,
                content,
                align,
                ..
            } => {
                let level = (*level).clamp(1, 6);
                format!(
                    "<h{level}{}>{}</h{level}>",
                    align_attribute(*align),
                    self.inlines(content)
                )
            }
            Block::Paragraph { content, align, .. } => {
                let body = self.inlines(content);
                format!(
                    "<p{}>{}</p>",
                    align_attribute(*align),
                    if body.is_empty() {
                        "<br>".to_owned()
                    } else {
                        body
                    }
                )
            }
            Block::ListItem { content, .. } => format!("<p>{}</p>", self.inlines(content)),
            Block::Table { rows, .. } => {
                let mut html = String::from("<table>\n");
                for row in rows {
                    let tag = if row.header { "th" } else { "td" };
                    html.push_str("<tr>");
                    for cell in &row.cells {
                        let mut attributes = String::new();
                        if cell.col_span > 1 {
                            let _ = write!(attributes, " colspan=\"{}\"", cell.col_span);
                        }
                        if cell.row_span > 1 {
                            let _ = write!(attributes, " rowspan=\"{}\"", cell.row_span);
                        }
                        let content = self.cell(&cell.blocks);
                        let _ = write!(html, "<{tag}{attributes}>{content}</{tag}>");
                    }
                    html.push_str("</tr>\n");
                }
                html.push_str("</table>");
                html
            }
            Block::Blockquote { blocks } => {
                format!("<blockquote>\n{}</blockquote>", self.blocks(blocks))
            }
            Block::CodeBlock { text } => format!("<pre><code>{}</code></pre>", escape_text(text)),
            Block::HorizontalRule => "<hr>".to_owned(),
        }
    }

    /// The blocks of a cell: a lone paragraph is written as its text, as a cell usually holds it.
    fn cell(&mut self, blocks: &[Block]) -> String {
        if let [Block::Paragraph { content, align, .. }] = blocks {
            let body = self.inlines(content);
            return match align {
                Some(_) => format!("<p{}>{body}</p>", align_attribute(*align)),
                None => body,
            };
        }
        self.blocks(blocks).trim_end().to_owned()
    }

    fn inlines(&mut self, content: &[Inline]) -> String {
        let mut html = String::new();
        let mut open: Vec<Mark> = Vec::new();
        for inline in content {
            let marks: Vec<Mark> = match inline {
                Inline::Text { marks, .. } => {
                    if marks
                        .iter()
                        .any(|mark| matches!(mark, Mark::Deletion { .. }))
                    {
                        continue;
                    }
                    marks
                        .iter()
                        .filter(|mark| writes_html(mark))
                        .cloned()
                        .collect()
                }
                _ => Vec::new(),
            };
            let keep = open
                .iter()
                .zip(&marks)
                .take_while(|(open, want)| open == want)
                .count();
            while open.len() > keep {
                let mark = open.pop().unwrap_or(Mark::Bold);
                html.push_str(close_tag(&mark));
            }
            for mark in &marks[keep..] {
                html.push_str(&open_tag(mark));
                open.push(mark.clone());
            }
            match inline {
                Inline::Text { text, .. } => html.push_str(&escape_text(text)),
                Inline::HardBreak => html.push_str("<br>"),
                Inline::Image {
                    media,
                    alt,
                    width,
                    height,
                    anchor,
                } => {
                    let Some(item) = self.tree.media.get(*media) else {
                        continue;
                    };
                    let _ = write!(
                        html,
                        "<img src=\"data:{};base64,{}\" alt=\"{}\"",
                        item.content_type,
                        crate::base64::encode(&item.bytes),
                        escape_attribute(alt.as_deref().unwrap_or_default())
                    );
                    if let Some(width) = width {
                        let _ = write!(html, " width=\"{width}\"");
                    }
                    if let Some(height) = height {
                        let _ = write!(html, " height=\"{height}\"");
                    }
                    if let Some(style) = anchor.as_deref().map(float_style) {
                        let _ = write!(html, " style=\"{style}\"");
                    }
                    html.push('>');
                }
                Inline::NoteReference { note } => {
                    if *note >= self.tree.notes.len() {
                        continue;
                    }
                    let number = if let Some(at) = self.notes.iter().position(|known| known == note)
                    {
                        at + 1
                    } else {
                        self.notes.push(*note);
                        self.notes.len()
                    };
                    let _ = write!(
                        html,
                        "<sup><a href=\"#note-{number}\" data-note=\"{number}\">{number}</a></sup>"
                    );
                }
                Inline::Math { latex, display } => {
                    let class = if *display { "math display" } else { "math" };
                    let _ = write!(
                        html,
                        "<span class=\"{class}\" data-latex=\"{}\">{}</span>",
                        escape_attribute(latex),
                        escape_text(latex)
                    );
                }
                Inline::CommentAnchor { .. } => {}
            }
        }
        while let Some(mark) = open.pop() {
            html.push_str(close_tag(&mark));
        }
        html
    }
}

fn close_lists(html: &mut String, open: &mut Vec<(bool, u32, bool)>, depth: usize) {
    while open.len() > depth {
        let (ordered, _, item_open) = open.pop().unwrap_or((false, 0, false));
        if item_open {
            html.push_str("</li>\n");
        }
        html.push_str(if ordered { "</ol>\n" } else { "</ul>\n" });
    }
}

fn align_attribute(align: Option<Alignment>) -> &'static str {
    match align {
        Some(Alignment::Center) => " style=\"text-align:center\"",
        Some(Alignment::Right) => " style=\"text-align:right\"",
        Some(Alignment::Justify) => " style=\"text-align:justify\"",
        Some(Alignment::Left) => " style=\"text-align:left\"",
        None => "",
    }
}

fn writes_html(mark: &Mark) -> bool {
    !matches!(
        mark,
        Mark::Comment { .. }
            | Mark::Insertion { .. }
            | Mark::Deletion { .. }
            | Mark::FormatChange { .. }
    )
}

fn open_tag(mark: &Mark) -> String {
    match mark {
        Mark::Bold => "<strong>".to_owned(),
        Mark::Italic => "<em>".to_owned(),
        Mark::Underline => "<u>".to_owned(),
        Mark::Strike => "<s>".to_owned(),
        Mark::Code => "<code>".to_owned(),
        Mark::Superscript => "<sup>".to_owned(),
        Mark::Subscript => "<sub>".to_owned(),
        Mark::TextStyle {
            font_family,
            font_size,
            color,
        } => {
            let mut style = String::new();
            if let Some(family) = font_family {
                let _ = write!(
                    style,
                    "font-family:'{}';",
                    family.replace(['\'', '"', ';'], "")
                );
            }
            if let Some(size) = font_size {
                let _ = write!(style, "font-size:{};", size.replace(['"', ';'], ""));
            }
            if let Some(color) = color {
                let _ = write!(style, "color:{};", color.replace(['"', ';'], ""));
            }
            format!("<span style=\"{}\">", escape_attribute(&style))
        }
        Mark::Highlight { color } => format!(
            "<mark style=\"background-color:{}\">",
            escape_attribute(&color.replace(['"', ';'], ""))
        ),
        Mark::Link { href } => format!("<a href=\"{}\">", escape_attribute(href)),
        _ => String::new(),
    }
}

fn close_tag(mark: &Mark) -> &'static str {
    match mark {
        Mark::Bold => "</strong>",
        Mark::Italic => "</em>",
        Mark::Underline => "</u>",
        Mark::Strike => "</s>",
        Mark::Code => "</code>",
        Mark::Superscript => "</sup>",
        Mark::Subscript => "</sub>",
        Mark::TextStyle { .. } => "</span>",
        Mark::Highlight { .. } => "</mark>",
        Mark::Link { .. } => "</a>",
        _ => "",
    }
}

fn escape_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attribute(text: &str) -> String {
    escape_text(text).replace('"', "&quot;")
}

#[cfg(test)]
mod tests;
