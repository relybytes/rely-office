// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! Markdown read into the document tree and written from it: `CommonMark` with the tables,
//! strikethrough, task lists, footnotes and math of GitHub's flavour.
//!
//! What Markdown cannot say is left out when a tree is written: fonts, sizes, colours,
//! alignment, merged cells, headers and footers, comments and tracked deletions. An image goes
//! into the text as a `data:` address, so the file stands alone; an image of the file that
//! points elsewhere is read as a link, since the reader fetches nothing.

use std::collections::HashMap;
use std::fmt::Write as _;

use pulldown_cmark::{
    Alignment as MarkdownAlignment, Event, HeadingLevel, Options, Parser, Tag, TagEnd,
};

use super::{
    Alignment, Block, DocumentTree, Inline, Mark, Media, Note, NoteKind, TableCell, TableRow,
    sniff_image_type,
};
use crate::{Limits, OfficeError};

/// Read Markdown text as a document tree.
///
/// # Errors
///
/// [`OfficeError::TooLarge`] past [`Limits::max_input_bytes`].
pub fn read_markdown_tree(text: &str, limits: &Limits) -> Result<DocumentTree, OfficeError> {
    if text.len() as u64 > limits.max_input_bytes {
        return Err(OfficeError::TooLarge {
            what: "input".to_owned(),
            limit: limits.max_input_bytes,
        });
    }
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_MATH
        | Options::ENABLE_SUPERSCRIPT
        | Options::ENABLE_SUBSCRIPT;
    let mut reader = Reader::default();
    for event in Parser::new_ext(text, options) {
        reader.event(event);
    }
    Ok(reader.finish())
}

/// The blocks a container being read collects.
enum Container {
    Body(Vec<Block>),
    Quote(Vec<Block>),
    Note(usize, Vec<Block>),
}

impl Container {
    fn blocks(&mut self) -> &mut Vec<Block> {
        match self {
            Self::Body(blocks) | Self::Quote(blocks) | Self::Note(_, blocks) => blocks,
        }
    }
}

struct OpenList {
    ordered: bool,
    id: u32,
    /// The item being read has text not yet written as a list item.
    item_pending: bool,
    /// The item being read has written its first paragraph already.
    item_started: bool,
}

struct OpenTable {
    alignments: Vec<MarkdownAlignment>,
    rows: Vec<TableRow>,
    header: bool,
}

struct Reader {
    containers: Vec<Container>,
    inlines: Vec<Inline>,
    marks: Vec<Mark>,
    /// For each open link, whether it took a mark (only safe addresses do).
    links: Vec<bool>,
    lists: Vec<OpenList>,
    next_list_id: u32,
    table: Option<OpenTable>,
    code: Option<String>,
    html: Option<String>,
    heading: Option<u8>,
    /// The address and alt text of an image being read.
    image: Option<(String, String)>,
    media: Vec<Media>,
    notes: Vec<Option<Vec<Block>>>,
    note_labels: HashMap<String, usize>,
}

impl Default for Reader {
    fn default() -> Self {
        Self {
            containers: vec![Container::Body(Vec::new())],
            inlines: Vec::new(),
            marks: Vec::new(),
            links: Vec::new(),
            lists: Vec::new(),
            next_list_id: 1,
            table: None,
            code: None,
            html: None,
            heading: None,
            image: None,
            media: Vec::new(),
            notes: Vec::new(),
            note_labels: HashMap::new(),
        }
    }
}

impl Reader {
    fn event(&mut self, event: Event<'_>) {
        if let Some((_, alt)) = &mut self.image {
            match event {
                Event::Text(text) | Event::Code(text) => alt.push_str(&text),
                Event::End(TagEnd::Image) => self.end_image(),
                _ => {}
            }
            return;
        }
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => {
                if let Some(code) = &mut self.code {
                    code.push_str(&text);
                } else {
                    self.push_text(&text, &[]);
                }
            }
            Event::Code(text) => self.push_text(&text, &[Mark::Code]),
            Event::InlineMath(latex) => self.inlines.push(Inline::Math {
                latex: latex.to_string(),
                display: false,
            }),
            Event::DisplayMath(latex) => self.inlines.push(Inline::Math {
                latex: latex.to_string(),
                display: true,
            }),
            Event::Html(html) => {
                if let Some(block) = &mut self.html {
                    block.push_str(&html);
                }
            }
            Event::InlineHtml(html) => self.inline_html(&html),
            Event::FootnoteReference(label) => {
                let note = self.note_index(&label);
                self.inlines.push(Inline::NoteReference { note });
            }
            Event::SoftBreak => self.push_text(" ", &[]),
            Event::HardBreak => self.inlines.push(Inline::HardBreak),
            Event::Rule => {
                self.flush_item();
                self.blocks().push(Block::HorizontalRule);
            }
            Event::TaskListMarker(done) => self.push_text(if done { "☑ " } else { "☐ " }, &[]),
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {
                if let Some(list) = self.lists.last_mut()
                    && list.item_started
                {
                    // A second paragraph of a list item goes on in the item, on a line of its own.
                    self.inlines.push(Inline::HardBreak);
                }
            }
            Tag::Heading { level, .. } => {
                self.flush_item();
                self.heading = Some(heading_level(level));
            }
            Tag::BlockQuote(_) => {
                self.flush_item();
                self.containers.push(Container::Quote(Vec::new()));
            }
            Tag::CodeBlock(_) => {
                self.flush_item();
                self.code = Some(String::new());
            }
            Tag::HtmlBlock => {
                self.flush_item();
                self.html = Some(String::new());
            }
            Tag::List(start) => {
                self.flush_item();
                let id = if let Some(parent) = self.lists.last() {
                    parent.id
                } else {
                    self.next_list_id += 1;
                    self.next_list_id - 1
                };
                self.lists.push(OpenList {
                    ordered: start.is_some(),
                    id,
                    item_pending: false,
                    item_started: false,
                });
            }
            Tag::Item => {
                if let Some(list) = self.lists.last_mut() {
                    list.item_pending = true;
                    list.item_started = false;
                }
            }
            Tag::FootnoteDefinition(label) => {
                let index = self.note_index(&label);
                self.containers.push(Container::Note(index, Vec::new()));
            }
            Tag::Table(alignments) => {
                self.table = Some(OpenTable {
                    alignments,
                    rows: Vec::new(),
                    header: false,
                });
            }
            Tag::TableHead => {
                if let Some(table) = &mut self.table {
                    table.header = true;
                    table.rows.push(TableRow {
                        header: true,
                        cells: Vec::new(),
                    });
                }
            }
            Tag::TableRow => {
                if let Some(table) = &mut self.table {
                    table.rows.push(TableRow {
                        header: false,
                        cells: Vec::new(),
                    });
                }
            }
            Tag::TableCell => self.inlines.clear(),
            Tag::Emphasis => self.marks.push(Mark::Italic),
            Tag::Strong => self.marks.push(Mark::Bold),
            Tag::Strikethrough => self.marks.push(Mark::Strike),
            Tag::Superscript => self.marks.push(Mark::Superscript),
            Tag::Subscript => self.marks.push(Mark::Subscript),
            Tag::Link { dest_url, .. } => {
                let safe = is_safe_link(&dest_url);
                if safe {
                    self.marks.push(Mark::Link {
                        href: dest_url.to_string(),
                    });
                }
                self.links.push(safe);
            }
            Tag::Image { dest_url, .. } => self.image = Some((dest_url.to_string(), String::new())),
            _ => {}
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one match over the ends of every Markdown element"
    )]
    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                if let Some(list) = self.lists.last_mut()
                    && list.item_pending
                {
                    list.item_started = true;
                } else {
                    let content = std::mem::take(&mut self.inlines);
                    if !content.is_empty() {
                        self.blocks().push(paragraph(content));
                    }
                }
            }
            TagEnd::Heading(_) => {
                let content = std::mem::take(&mut self.inlines);
                let level = self.heading.take().unwrap_or(1);
                self.blocks().push(Block::Heading {
                    level,
                    content,
                    align: None,
                    look: super::ParagraphLook::default(),
                });
            }
            TagEnd::BlockQuote(_) => {
                self.flush_item();
                if let Some(Container::Quote(blocks)) = self.containers.pop() {
                    self.blocks().push(Block::Blockquote { blocks });
                }
            }
            TagEnd::CodeBlock => {
                let mut text = self.code.take().unwrap_or_default();
                if text.ends_with('\n') {
                    text.pop();
                }
                self.blocks().push(Block::CodeBlock { text });
            }
            TagEnd::HtmlBlock => {
                let html = self.html.take().unwrap_or_default();
                let text = strip_tags(&html);
                if !text.trim().is_empty() {
                    let content = vec![text_inline(text.trim().to_owned(), Vec::new())];
                    self.blocks().push(paragraph(content));
                }
            }
            TagEnd::List(_) => {
                self.flush_item();
                self.lists.pop();
            }
            TagEnd::Item => self.flush_item(),
            TagEnd::FootnoteDefinition => {
                self.flush_item();
                if let Some(Container::Note(index, blocks)) = self.containers.pop()
                    && let Some(slot) = self.notes.get_mut(index)
                {
                    *slot = Some(blocks);
                }
            }
            TagEnd::TableCell => {
                let content = std::mem::take(&mut self.inlines);
                if let Some(table) = &mut self.table {
                    let column = table.rows.last().map_or(0, |row| row.cells.len());
                    let align = match table.alignments.get(column) {
                        Some(MarkdownAlignment::Center) => Some(Alignment::Center),
                        Some(MarkdownAlignment::Right) => Some(Alignment::Right),
                        Some(MarkdownAlignment::Left) => Some(Alignment::Left),
                        _ => None,
                    };
                    if let Some(row) = table.rows.last_mut() {
                        row.cells.push(TableCell {
                            col_span: 1,
                            row_span: 1,
                            blocks: vec![Block::Paragraph {
                                content,
                                align,
                                look: super::ParagraphLook::default(),
                            }],
                            look: super::CellLook::default(),
                        });
                    }
                }
            }
            TagEnd::Table => {
                if let Some(table) = self.table.take() {
                    self.blocks().push(Block::Table {
                        rows: table.rows,
                        look: super::TableLook::default(),
                    });
                }
            }
            TagEnd::Emphasis => self.pop_mark(&Mark::Italic),
            TagEnd::Strong => self.pop_mark(&Mark::Bold),
            TagEnd::Strikethrough => self.pop_mark(&Mark::Strike),
            TagEnd::Superscript => self.pop_mark(&Mark::Superscript),
            TagEnd::Subscript => self.pop_mark(&Mark::Subscript),
            TagEnd::Link => {
                if self.links.pop() == Some(true)
                    && let Some(at) = self
                        .marks
                        .iter()
                        .rposition(|mark| matches!(mark, Mark::Link { .. }))
                {
                    self.marks.remove(at);
                }
            }
            _ => {}
        }
    }

    fn end_image(&mut self) {
        let Some((address, alt)) = self.image.take() else {
            return;
        };
        if let Some((_, bytes)) = crate::base64::data_url(&address) {
            let content_type = sniff_image_type(&bytes);
            if content_type.starts_with("image/") {
                self.media.push(Media {
                    content_type,
                    bytes,
                });
                self.inlines.push(Inline::Image {
                    media: self.media.len() - 1,
                    alt: (!alt.is_empty()).then_some(alt),
                    width: None,
                    height: None,
                    anchor: None,
                });
                return;
            }
        }
        let shown = if alt.is_empty() { address.clone() } else { alt };
        if is_safe_link(&address) {
            self.push_text(&shown, &[Mark::Link { href: address }]);
        } else {
            self.push_text(&shown, &[]);
        }
    }

    /// The tags Markdown has no syntax for, written as HTML: a line break, underline,
    /// superscript, subscript and a highlight.
    fn inline_html(&mut self, html: &str) {
        let tag = html.trim().to_ascii_lowercase();
        let tag = tag
            .trim_start_matches('<')
            .trim_end_matches('>')
            .trim_end_matches('/')
            .trim();
        let (closing, name) = match tag.strip_prefix('/') {
            Some(name) => (true, name.trim()),
            None => (false, tag.split_whitespace().next().unwrap_or_default()),
        };
        let mark = match name {
            "br" => {
                self.inlines.push(Inline::HardBreak);
                return;
            }
            "u" | "ins" => Mark::Underline,
            "sup" => Mark::Superscript,
            "sub" => Mark::Subscript,
            "mark" => Mark::Highlight {
                color: HIGHLIGHT.to_owned(),
            },
            "s" | "del" | "strike" => Mark::Strike,
            _ => return,
        };
        if closing {
            self.pop_mark(&mark);
        } else {
            self.marks.push(mark);
        }
    }

    fn pop_mark(&mut self, mark: &Mark) {
        if let Some(at) = self.marks.iter().rposition(|open| open == mark) {
            self.marks.remove(at);
        }
    }

    fn push_text(&mut self, text: &str, extra: &[Mark]) {
        if text.is_empty() {
            return;
        }
        let mut marks = self.marks.clone();
        marks.extend_from_slice(extra);
        marks.sort_by_key(mark_order);
        marks.dedup();
        if let Some(Inline::Text {
            text: previous,
            marks: previous_marks,
            ..
        }) = self.inlines.last_mut()
            && *previous_marks == marks
        {
            previous.push_str(text);
            return;
        }
        self.inlines.push(text_inline(text.to_owned(), marks));
    }

    /// Write the text of the list item being read as a list item, once.
    fn flush_item(&mut self) {
        let level = self.lists.len().saturating_sub(1);
        let Some(list) = self.lists.last_mut() else {
            return;
        };
        if !list.item_pending {
            return;
        }
        list.item_pending = false;
        list.item_started = false;
        let (ordered, list_id) = (list.ordered, list.id);
        let mut content = std::mem::take(&mut self.inlines);
        while matches!(content.last(), Some(Inline::HardBreak)) {
            content.pop();
        }
        self.blocks().push(Block::ListItem {
            ordered,
            level: u8::try_from(level).unwrap_or(u8::MAX),
            list_id,
            content,
            look: super::ParagraphLook::default(),
        });
    }

    fn note_index(&mut self, label: &str) -> usize {
        if let Some(index) = self.note_labels.get(label) {
            return *index;
        }
        self.notes.push(None);
        let index = self.notes.len() - 1;
        self.note_labels.insert(label.to_owned(), index);
        index
    }

    fn blocks(&mut self) -> &mut Vec<Block> {
        if self.containers.is_empty() {
            self.containers.push(Container::Body(Vec::new()));
        }
        let last = self.containers.len() - 1;
        self.containers[last].blocks()
    }

    fn finish(mut self) -> DocumentTree {
        self.flush_item();
        let rest = std::mem::take(&mut self.inlines);
        if !rest.is_empty() {
            self.blocks().push(paragraph(rest));
        }
        let mut blocks = Vec::new();
        for container in self.containers {
            blocks.extend(match container {
                Container::Body(found) | Container::Quote(found) | Container::Note(_, found) => {
                    found
                }
            });
        }
        DocumentTree {
            blocks,
            media: self.media,
            notes: self
                .notes
                .into_iter()
                .map(|blocks| Note {
                    kind: NoteKind::Footnote,
                    blocks: blocks.unwrap_or_default(),
                })
                .collect(),
            ..DocumentTree::default()
        }
    }
}

/// The colour a `<mark>` highlight takes, Word's yellow.
const HIGHLIGHT: &str = "#FFFF00";

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn paragraph(content: Vec<Inline>) -> Block {
    Block::Paragraph {
        content,
        align: None,
        look: super::ParagraphLook::default(),
    }
}

fn text_inline(text: String, marks: Vec<Mark>) -> Inline {
    Inline::Text {
        text,
        marks,
        look: super::RunLook::default(),
    }
}

pub(crate) fn is_safe_link(address: &str) -> bool {
    let lower = address.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")
}

/// The text of HTML without its tags, its common entities decoded.
fn strip_tags(html: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for character in html.chars() {
        match character {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => text.push(character),
            _ => {}
        }
    }
    text.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// The order the tree lists marks in.
pub(crate) fn mark_order(mark: &Mark) -> u8 {
    match mark {
        Mark::Bold => 0,
        Mark::Italic => 1,
        Mark::Underline => 2,
        Mark::Strike => 3,
        Mark::Code => 4,
        Mark::Superscript => 5,
        Mark::Subscript => 6,
        Mark::TextStyle { .. } => 7,
        Mark::Highlight { .. } => 8,
        Mark::Link { .. } => 9,
        Mark::Comment { .. } => 10,
        Mark::Insertion { .. } => 11,
        Mark::Deletion { .. } => 12,
        Mark::FormatChange { .. } => 13,
    }
}

/// A document tree as Markdown: headings, paragraphs, nested lists, tables, quotes, code,
/// rules, images as `data:` addresses, footnotes and math. Tracked deletions are left out and
/// insertions kept, as if the changes were accepted.
#[must_use]
pub fn write_markdown(tree: &DocumentTree) -> String {
    let mut writer = Writer {
        tree,
        notes: Vec::new(),
    };
    let mut text = writer.blocks(&tree.blocks);
    if !writer.notes.is_empty() {
        let notes = std::mem::take(&mut writer.notes);
        for (number, note) in notes.iter().enumerate() {
            let body = writer.blocks(&tree.notes[*note].blocks);
            let mut lines = body.trim_end().lines();
            let _ = write!(
                text,
                "\n[^{}]: {}",
                number + 1,
                lines.next().unwrap_or_default()
            );
            for line in lines {
                text.push('\n');
                if !line.is_empty() {
                    text.push_str("    ");
                    text.push_str(line);
                }
            }
            text.push('\n');
        }
    }
    text
}

struct Writer<'t> {
    tree: &'t DocumentTree,
    /// The notes in the order the text refers to them, by their index in the tree.
    notes: Vec<usize>,
}

impl Writer<'_> {
    fn blocks(&mut self, blocks: &[Block]) -> String {
        let mut text = String::new();
        let mut previous_list: Option<u32> = None;
        // The width of the marker of the last item at each level, which a deeper item indents by.
        let mut marker_widths: Vec<usize> = Vec::new();
        let mut counters: Vec<usize> = Vec::new();
        let mut lists_seen = 0usize;
        for block in blocks {
            if let Block::ListItem {
                ordered,
                level,
                list_id,
                content,
                ..
            } = block
            {
                if previous_list != Some(*list_id) {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    lists_seen += 1;
                    marker_widths.clear();
                    counters.clear();
                }
                previous_list = Some(*list_id);
                let level = usize::from(*level);
                marker_widths.truncate(level + 1);
                counters.truncate(level + 1);
                while counters.len() <= level {
                    counters.push(0);
                    marker_widths.push(2);
                }
                counters[level] += 1;
                // Alternating markers keep two lists in a row from merging into one.
                let marker = if *ordered {
                    let close = if lists_seen % 2 == 1 { '.' } else { ')' };
                    format!("{}{close} ", counters[level])
                } else {
                    (if lists_seen % 2 == 1 { "- " } else { "* " }).to_owned()
                };
                marker_widths[level] = marker.len();
                let indent: usize = marker_widths[..level].iter().sum();
                let body = self.inlines(content);
                let mut lines = body.split('\n');
                let _ = writeln!(
                    text,
                    "{}{marker}{}",
                    " ".repeat(indent),
                    lines.next().unwrap_or_default()
                );
                for line in lines {
                    let _ = writeln!(text, "{}{line}", " ".repeat(indent + marker.len()));
                }
                continue;
            }
            previous_list = None;
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&self.block(block));
            text.push('\n');
        }
        text
    }

    fn block(&mut self, block: &Block) -> String {
        match block {
            Block::Heading { level, content, .. } => {
                format!(
                    "{} {}",
                    "#".repeat(usize::from((*level).clamp(1, 6))),
                    self.inlines(content).replace('\n', " ")
                )
            }
            Block::Paragraph { content, .. } => {
                let body = self.inlines(content);
                if body.is_empty() {
                    "&nbsp;".to_owned()
                } else {
                    body
                }
            }
            Block::ListItem { content, .. } => self.inlines(content),
            Block::Table { rows, .. } => self.table(rows),
            Block::Blockquote { blocks } => {
                let inner = self.blocks(blocks);
                inner
                    .trim_end()
                    .lines()
                    .map(|line| {
                        if line.is_empty() {
                            ">".to_owned()
                        } else {
                            format!("> {line}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            Block::CodeBlock { text } => {
                let fence = "`".repeat(longest_run(text, '`').max(2) + 1);
                format!("{fence}\n{text}\n{fence}")
            }
            Block::HorizontalRule => "---".to_owned(),
        }
    }

    fn table(&mut self, rows: &[TableRow]) -> String {
        let width = rows
            .iter()
            .map(|row| {
                row.cells
                    .iter()
                    .map(|cell| cell.col_span.max(1) as usize)
                    .sum::<usize>()
            })
            .max()
            .unwrap_or(0);
        if width == 0 {
            return String::new();
        }
        let mut lines = Vec::new();
        let mut alignments = vec![None; width];
        for (index, row) in rows.iter().enumerate() {
            let mut cells = Vec::with_capacity(width);
            for cell in &row.cells {
                let column = cells.len();
                if let Some(Block::Paragraph { align, .. }) = cell.blocks.first()
                    && column < width
                    && alignments[column].is_none()
                {
                    alignments[column] = *align;
                }
                let text = cell
                    .blocks
                    .iter()
                    .map(|block| self.cell_text(block))
                    .filter(|text| !text.is_empty())
                    .collect::<Vec<_>>()
                    .join("<br>");
                cells.push(text);
                for _ in 1..cell.col_span.max(1) {
                    cells.push(String::new());
                }
            }
            cells.resize(width, String::new());
            lines.push(format!("| {} |", cells.join(" | ")));
            if index == 0 {
                let rule = alignments
                    .iter()
                    .map(|align| match align {
                        Some(Alignment::Center) => ":---:",
                        Some(Alignment::Right) => "---:",
                        Some(Alignment::Left) => ":---",
                        _ => "---",
                    })
                    .collect::<Vec<_>>()
                    .join(" | ");
                lines.push(format!("| {rule} |"));
            }
        }
        lines.join("\n")
    }

    /// A block inside a table cell, on one line: the line breaks of a pipe table are `<br>`.
    fn cell_text(&mut self, block: &Block) -> String {
        let text = match block {
            Block::Heading { content, .. }
            | Block::Paragraph { content, .. }
            | Block::ListItem { content, .. } => self.inlines(content),
            Block::CodeBlock { text } => format!("`{}`", text.replace('\n', " ")),
            Block::Table { rows, .. } => rows
                .iter()
                .flat_map(|row| row.cells.iter())
                .flat_map(|cell| cell.blocks.iter())
                .map(|inner| self.cell_text(inner))
                .collect::<Vec<_>>()
                .join(" "),
            Block::Blockquote { blocks } => blocks
                .iter()
                .map(|inner| self.cell_text(inner))
                .collect::<Vec<_>>()
                .join("<br>"),
            Block::HorizontalRule => String::new(),
        };
        text.replace('\n', "<br>")
    }

    fn inlines(&mut self, content: &[Inline]) -> String {
        let mut out = InlineWriter::default();
        for inline in content {
            match inline {
                Inline::Text { text, marks, .. } => {
                    if marks
                        .iter()
                        .any(|mark| matches!(mark, Mark::Deletion { .. }))
                    {
                        continue;
                    }
                    out.text(text, marks);
                }
                Inline::HardBreak => out.raw("\\\n", &[]),
                Inline::Image { media, alt, .. } => {
                    let Some(item) = self.tree.media.get(*media) else {
                        continue;
                    };
                    let alt = alt
                        .as_deref()
                        .unwrap_or_default()
                        .replace(['[', ']', '\n'], " ");
                    out.raw(
                        &format!(
                            "![{alt}](data:{};base64,{})",
                            item.content_type,
                            crate::base64::encode(&item.bytes)
                        ),
                        &[],
                    );
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
                    out.raw(&format!("[^{number}]"), &[]);
                }
                Inline::Math { latex, display } => {
                    let fence = if *display { "$$" } else { "$" };
                    out.raw(&format!("{fence}{latex}{fence}"), &[]);
                }
                Inline::CommentAnchor { .. } => {}
            }
        }
        let text = out.finish();
        escape_line_start(&text)
    }
}

/// The text of a block, its marks opened and closed around the runs that carry them.
#[derive(Default)]
struct InlineWriter {
    text: String,
    open: Vec<Mark>,
    /// Whitespace at the end of the last run, written after the marks it should stay outside.
    pending: String,
}

impl InlineWriter {
    fn text(&mut self, text: &str, marks: &[Mark]) {
        let wanted: Vec<Mark> = marks.iter().filter(|mark| writes(mark)).cloned().collect();
        if wanted.contains(&Mark::Code) {
            let body = text.trim();
            if body.is_empty() {
                self.pending.push_str(text);
                return;
            }
            let leading = &text[..text.len() - text.trim_start().len()];
            let trailing = &text[text.trim_end().len()..];
            let others: Vec<Mark> = wanted
                .into_iter()
                .filter(|mark| *mark != Mark::Code)
                .collect();
            self.switch(&others, leading);
            let fence = "`".repeat(longest_run(body, '`') + 1);
            let padding = if body.starts_with('`') || body.ends_with('`') {
                " "
            } else {
                ""
            };
            let _ = write!(self.text, "{fence}{padding}{body}{padding}{fence}");
            self.pending.push_str(trailing);
            return;
        }
        let body = text.trim();
        if body.is_empty() {
            self.pending.push_str(text);
            return;
        }
        let leading = &text[..text.len() - text.trim_start().len()];
        let trailing = &text[text.trim_end().len()..];
        self.switch(&wanted, leading);
        self.text.push_str(&escape(body));
        self.pending.push_str(trailing);
    }

    fn raw(&mut self, markup: &str, marks: &[Mark]) {
        self.switch(marks, "");
        self.text.push_str(markup);
    }

    /// Close the marks the next run does not carry and open the ones it adds, the whitespace
    /// between the runs kept outside the delimiters.
    fn switch(&mut self, wanted: &[Mark], leading: &str) {
        let keep = self
            .open
            .iter()
            .zip(wanted)
            .take_while(|(open, want)| open == want)
            .count();
        let keep = if self.open.iter().all(|mark| wanted.contains(mark)) {
            self.open.len()
        } else {
            keep
        };
        while self.open.len() > keep {
            let mark = self.open.pop().unwrap_or(Mark::Bold);
            self.text.push_str(&close(&mark));
        }
        self.text.push_str(&std::mem::take(&mut self.pending));
        self.text.push_str(leading);
        for mark in wanted {
            if !self.open.contains(mark) {
                self.text.push_str(open(mark));
                self.open.push(mark.clone());
            }
        }
    }

    fn finish(mut self) -> String {
        self.switch(&[], "");
        self.text.trim_end_matches([' ', '\t']).to_owned()
    }
}

fn writes(mark: &Mark) -> bool {
    matches!(
        mark,
        Mark::Bold
            | Mark::Italic
            | Mark::Underline
            | Mark::Strike
            | Mark::Code
            | Mark::Superscript
            | Mark::Subscript
            | Mark::Highlight { .. }
            | Mark::Link { .. }
    )
}

fn open(mark: &Mark) -> &'static str {
    match mark {
        Mark::Bold => "**",
        Mark::Italic => "*",
        Mark::Strike => "~~",
        Mark::Underline => "<u>",
        Mark::Superscript => "<sup>",
        Mark::Subscript => "<sub>",
        Mark::Highlight { .. } => "<mark>",
        Mark::Link { .. } => "[",
        _ => "",
    }
}

fn close(mark: &Mark) -> String {
    match mark {
        Mark::Bold => "**".to_owned(),
        Mark::Italic => "*".to_owned(),
        Mark::Strike => "~~".to_owned(),
        Mark::Underline => "</u>".to_owned(),
        Mark::Superscript => "</sup>".to_owned(),
        Mark::Subscript => "</sub>".to_owned(),
        Mark::Highlight { .. } => "</mark>".to_owned(),
        Mark::Link { href } => format!("]({})", href.replace(' ', "%20").replace(')', "%29")),
        _ => String::new(),
    }
}

/// Text with the characters Markdown reads as syntax escaped.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        if matches!(
            character,
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '|' | '~' | '$'
        ) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

/// Escape what would open a heading, a list or a rule at the start of a line of text.
fn escape_line_start(text: &str) -> String {
    text.split('\n')
        .map(|line| {
            let trimmed = line.trim_start();
            let indent = &line[..line.len() - trimmed.len()];
            if trimmed.starts_with(['#', '+', '-', '=']) {
                return format!("{indent}\\{trimmed}");
            }
            let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
            if digits > 0 && trimmed[digits..].starts_with(['.', ')']) {
                return format!("{indent}{}\\{}", &trimmed[..digits], &trimmed[digits..]);
            }
            line.to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn longest_run(text: &str, character: char) -> usize {
    let mut longest = 0;
    let mut current = 0;
    for found in text.chars() {
        if found == character {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest
}

#[cfg(test)]
mod tests;
