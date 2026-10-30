// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! An odt read into the same [`DocumentTree`] a docx gives: paragraphs, headings, lists, tables,
//! notes and images with the looks their styles resolve to, the headers and footers of the first
//! master page and its page layout.

use std::collections::HashMap;

use super::styles::{FO, STYLE, Scope, Styles, TEXT, TextFlags, border, colour, points, twips};
use crate::archive::{ZipPackage, open_package, read_member_bytes, read_text_member};
use crate::chart::{odf_chart_picture, odf_object_part};
use crate::document::objects::{self, EmbeddedObject};
use crate::document::{
    Block, Borders, CellLook, Comment, DocumentTree, HeadersFooters, ImportReport, Inline,
    LabelSuffix, ListLabel, Mark, Media, Note, NoteKind, PageField, PageLayout, ParagraphLook,
    RunLook, Section, TableCell, TableLook, TableRow, TextFrame, TrackedChanges, TreeOptions,
    bullet_label, number_label, plain_text, push_block, set_frame, sniff_image_type,
};
use crate::xml_tree::{Element, Namespace, Node, parse_tree};
use crate::{Limits, OfficeError};

const OFFICE: Namespace = Namespace::OdfOffice;
const TABLE: Namespace = Namespace::OdfTable;
const DRAW: Namespace = Namespace::OdfDraw;
const SVG: Namespace = Namespace::OdfSvg;
const XLINK: Namespace = Namespace::XLink;
const MAX_HEADING_LEVEL: u8 = 6;
const MAX_LIST_LEVEL: usize = 10;
/// A repeat count past this is a sheet-like table of empty rows; the rest is left out.
const MAX_REPEAT: usize = 256;
const MAX_TABLE_ROWS: usize = 10_000;
const PIXELS_PER_POINT: f64 = 96.0 / 72.0;
const DEFAULT_TAB_TWIPS: u32 = 709;

/// Read an odt as a document tree.
#[expect(
    clippy::too_many_lines,
    reason = "the parts, sections and master pages of an odt are read in order"
)]
pub(crate) fn read_odt_tree(
    bytes: &[u8],
    limits: &Limits,
    options: TreeOptions,
) -> Result<DocumentTree, OfficeError> {
    let mut package = open_package(bytes)?;
    let content = read_text_member(&mut package, "content.xml", limits)?.ok_or_else(|| {
        OfficeError::UnreadableDocument("the package has no content.xml".to_owned())
    })?;
    let content = parse_tree(&content, keeps_whitespace)?;
    let styles_root = read_text_member(&mut package, "styles.xml", limits)?
        .map(|xml| parse_tree(&xml, keeps_whitespace))
        .transpose()?;
    let styles = Styles::read(Some(&content), styles_root.as_ref());
    // `LibreOffice` adds the spaces of two paragraphs unless the document turns it off.
    let spacing_adds_up = read_text_member(&mut package, "settings.xml", limits)?
        .and_then(|xml| parse_tree(&xml, |_, _| false).ok())
        .and_then(|settings| config_item(&settings, "AddParaTableSpacing"))
        .is_none_or(|value| value == "true");
    let body = content
        .child(OFFICE, "body")
        .and_then(|body| body.child(OFFICE, "text"))
        .ok_or_else(|| {
            OfficeError::UnreadableDocument("the document has no text body".to_owned())
        })?;

    let mut converter = Converter {
        package: &mut package,
        limits,
        styles: &styles,
        report: ImportReport::default(),
        media: Vec::new(),
        media_by_href: HashMap::new(),
        media_bytes: 0,
        remaining_chars: limits.max_text_chars,
        notes: Vec::new(),
        reading_note: false,
        lists: HashMap::new(),
        next_list_id: 0,
        outline: [0; MAX_LIST_LEVEL],
        floating: Vec::new(),
        next_frame: 0,
        group_anchor: None,
        group_style: None,
        comments: Vec::new(),
        comment_names: Vec::new(),
        open_comments: Vec::new(),
        reading_comment: false,
        changes: if options.tracked_changes == TrackedChanges::Keep {
            body.child(TEXT, "tracked-changes")
                .map(read_changes)
                .unwrap_or_default()
        } else {
            HashMap::new()
        },
        open_changes: Vec::new(),
        objects: Vec::new(),
    };
    // A paragraph or table whose style names a master page other than the current one starts
    // a section on that page.
    let master_of = |element: &Element| {
        let style = match (element.namespace, element.name.as_str()) {
            (TEXT, "p" | "h") => element.attribute(TEXT, "style-name"),
            (TABLE, "table") => {
                return element
                    .attribute(TABLE, "style-name")
                    .and_then(|name| {
                        styles
                            .chain(Scope::Body, "table", Some(name))
                            .into_iter()
                            .find_map(|style| style.attribute(STYLE, "master-page-name"))
                    })
                    .filter(|page| !page.is_empty())
                    .map(str::to_owned);
            }
            _ => return None,
        };
        styles.master_page_of(Scope::Body, style)
    };
    let mut groups: Vec<(String, Vec<&Element>)> = Vec::new();
    for element in body.elements() {
        let named = master_of(element);
        match (groups.last_mut(), named) {
            (Some((current, _)), Some(named)) if *current != named => {
                groups.push((named, vec![element]));
            }
            (Some((_, elements)), _) => elements.push(element),
            (None, named) => groups.push((
                named.unwrap_or_else(|| "Standard".to_owned()),
                vec![element],
            )),
        }
    }
    if groups.is_empty() {
        groups.push(("Standard".to_owned(), Vec::new()));
    }
    let mut blocks = Vec::new();
    let mut sections = Vec::new();
    let mut by_master: HashMap<&str, HeadersFooters> = HashMap::new();
    for (name, elements) in &groups {
        converter.blocks_from(elements.iter().copied(), Scope::Body, &mut blocks);
        let master = styles
            .master_pages
            .iter()
            .find(|page| page.attribute(STYLE, "name") == Some(name.as_str()))
            .or_else(|| styles.master_pages.first());
        let headers_footers = if let Some(known) = by_master.get(name.as_str()) {
            known.clone()
        } else {
            let read = master
                .map(|master| converter.headers_footers(master))
                .unwrap_or_default();
            by_master.insert(name, read.clone());
            read
        };
        let mut layout = page_layout(&styles, master, &headers_footers);
        layout.spacing_adds_up = spacing_adds_up;
        sections.push(Section {
            end_block: blocks.len(),
            layout,
            headers_footers,
            continuous: false,
        });
    }
    let last = sections.last().cloned().unwrap_or_else(|| Section {
        end_block: blocks.len(),
        layout: PageLayout::default(),
        headers_footers: HeadersFooters::default(),
        continuous: false,
    });
    if sections.len() < 2 {
        sections.clear();
    }

    let comments = converter.threaded_comments();
    let mut report = converter.report;
    let roots: Vec<&Element> = std::iter::once(&content)
        .chain(styles_root.as_ref())
        .collect();
    let fonts = crate::document::read_odf_fonts(converter.package, &roots, limits, &mut report);
    Ok(DocumentTree {
        comments,
        blocks,
        media: converter.media,
        report,
        layout: last.layout,
        notes: converter.notes,
        headers_footers: last.headers_footers,
        sections,
        objects: converter.objects,
        macro_project: None,
        fonts,
    })
}

/// The changed regions of `text:tracked-changes`, each with the mark its text takes: an
/// insertion or a deletion, its author and date; format changes are left out.
fn read_changes(tracked: &Element) -> HashMap<String, Change> {
    let mut changes = HashMap::new();
    for region in tracked
        .elements()
        .filter(|region| region.is(TEXT, "changed-region"))
        .take(10_000)
    {
        let Some(id) = region.attribute(TEXT, "id") else {
            continue;
        };
        let Some(change) = region
            .elements()
            .find(|change| change.is(TEXT, "insertion") || change.is(TEXT, "deletion"))
        else {
            continue;
        };
        let info = change.child(OFFICE, "change-info");
        let field = |name: &str| {
            info.and_then(|info| {
                info.elements()
                    .find(|child| child.namespace == Namespace::Other && child.name == name)
            })
            .map(Element::text)
            .filter(|value| !value.trim().is_empty())
        };
        let author = field("creator").unwrap_or_default();
        let date = field("date");
        let inserted = change.name == "insertion";
        changes.insert(
            id.to_owned(),
            Change {
                mark: if inserted {
                    Mark::Insertion { author, date }
                } else {
                    Mark::Deletion { author, date }
                },
                deleted: (!inserted).then(|| change.clone()),
            },
        );
    }
    changes
}

/// Text keeps its spaces inside paragraphs and spans; whitespace between blocks is layout.
fn keeps_whitespace(namespace: Namespace, name: &str) -> bool {
    namespace == TEXT
        && matches!(
            name,
            "p" | "h" | "span" | "a" | "meta" | "ruby-base" | "note-citation" | "sequence"
        )
}

/// The numbering of one list as it goes.
#[derive(Debug, Clone)]
struct ListState {
    id: u32,
    counters: [u32; MAX_LIST_LEVEL],
}

struct Converter<'a, 'p> {
    package: &'a mut ZipPackage<'p>,
    limits: &'a Limits,
    styles: &'a Styles,
    report: ImportReport,
    media: Vec<Media>,
    media_by_href: HashMap<String, Option<usize>>,
    media_bytes: u64,
    remaining_chars: usize,
    notes: Vec<Note>,
    reading_note: bool,
    /// The numbering of each list style, kept for a list that continues the one before.
    lists: HashMap<String, ListState>,
    next_list_id: u32,
    outline: [u32; MAX_LIST_LEVEL],
    /// The text boxes of the paragraph being read, placed after it.
    floating: Vec<Block>,
    /// The last id given to a text box frame.
    next_frame: u32,
    /// The anchor type and the graphic style of the group being read, which its shapes take
    /// where they name none of their own.
    group_anchor: Option<String>,
    group_style: Option<String>,
    comments: Vec<Comment>,
    /// The `office:name` of each comment, and of the comment it answers.
    comment_names: Vec<(Option<String>, Option<String>)>,
    /// The comments whose range the text being read is in.
    open_comments: Vec<usize>,
    reading_comment: bool,
    /// The changed regions of the document, by id, when changes are kept.
    changes: HashMap<String, Change>,
    /// The insertions the text being read is in, by change id.
    open_changes: Vec<(String, Mark)>,
    objects: Vec<EmbeddedObject>,
}

/// A changed region of `text:tracked-changes`: the mark of its text, and for a deletion the
/// text it took away.
struct Change {
    mark: Mark,
    deleted: Option<Element>,
}

/// Where a paragraph sits in a list: its level, its list, the label of an item's first
/// paragraph and the indent of its level.
struct ListPlace {
    level: usize,
    id: u32,
    label: Option<ListLabel>,
    ordered: bool,
    indent: Option<(i32, i32)>,
}

impl Converter<'_, '_> {
    /// The blocks of a container: the body, a section, a cell, a note, a header.
    fn blocks(&mut self, container: &Element, scope: Scope, blocks: &mut Vec<Block>) {
        self.blocks_from(container.elements(), scope, blocks);
    }

    fn blocks_from<'e>(
        &mut self,
        elements: impl IntoIterator<Item = &'e Element>,
        scope: Scope,
        blocks: &mut Vec<Block>,
    ) {
        for element in elements {
            if self.remaining_chars == 0 {
                self.report.truncated = true;
                return;
            }
            match (element.namespace, element.name.as_str()) {
                (TEXT, "p") => self.paragraph(element, scope, None, blocks),
                (TEXT, "h") => self.heading(element, scope, blocks),
                (TEXT, "list") => self.list(element, scope, None, 0, blocks),
                (TABLE, "table") => {
                    if let Some(table) = self.table(element, scope) {
                        blocks.push(table);
                    }
                }
                (
                    TEXT,
                    "section" | "index-body" | "table-of-content" | "illustration-index"
                    | "table-index" | "object-index" | "user-index" | "alphabetical-index"
                    | "bibliography" | "index-title",
                ) => self.blocks(element, scope, blocks),
                (DRAW, _) => {
                    let mut content = Vec::new();
                    self.drawing(element, scope, &mut content);
                    if !content.is_empty() {
                        blocks.push(Block::Paragraph {
                            content,
                            align: None,
                            look: ParagraphLook::default(),
                        });
                    }
                    blocks.append(&mut self.floating);
                }
                _ => {}
            }
        }
    }

    fn paragraph(
        &mut self,
        paragraph: &Element,
        scope: Scope,
        list: Option<&ListPlace>,
        blocks: &mut Vec<Block>,
    ) {
        let style = paragraph.attribute(TEXT, "style-name");
        let (mut look, align, flags) = self.styles.paragraph_look(scope, style);
        let content = self.content(paragraph, scope, flags);
        relative_tabs(&mut look);
        let role = paragraph_role(style, self.styles, scope);
        let block = if let Some(place) = list {
            if let Some((left, first)) = place.indent {
                look.indent_left_twips = look.indent_left_twips.or(Some(left));
                look.indent_first_line_twips = look.indent_first_line_twips.or(Some(first));
            }
            look.list_label.clone_from(&place.label);
            Block::ListItem {
                ordered: place.ordered,
                level: u8::try_from(place.level).unwrap_or(0),
                list_id: place.id,
                content,
                look,
            }
        } else {
            match role {
                Role::Code => Block::CodeBlock {
                    text: plain_text(&content),
                },
                Role::Quote => Block::Blockquote {
                    blocks: vec![Block::Paragraph {
                        content,
                        align,
                        look,
                    }],
                },
                Role::Body => Block::Paragraph {
                    content,
                    align,
                    look,
                },
            }
        };
        push_block(blocks, block);
        blocks.append(&mut self.floating);
    }

    fn heading(&mut self, heading: &Element, scope: Scope, blocks: &mut Vec<Block>) {
        let style = heading.attribute(TEXT, "style-name");
        let level = heading
            .attribute(TEXT, "outline-level")
            .and_then(|level| level.parse::<u8>().ok())
            .unwrap_or(1)
            .clamp(1, MAX_HEADING_LEVEL);
        let (mut look, align, flags) = self.styles.paragraph_look(scope, style);
        let content = self.content(heading, scope, flags);
        relative_tabs(&mut look);
        look.list_label = self.outline_label(level);
        blocks.push(Block::Heading {
            level,
            content,
            align,
            look,
        });
        blocks.append(&mut self.floating);
    }

    /// The label the outline numbering gives a heading of `level`, counting it.
    fn outline_label(&mut self, level: u8) -> Option<ListLabel> {
        let index = usize::from(level - 1).min(MAX_LIST_LEVEL - 1);
        self.outline[index] += 1;
        for deeper in &mut self.outline[index + 1..] {
            *deeper = 0;
        }
        let outline = self.styles.outline.as_ref()?;
        let style = outline.elements().find(|style| {
            style
                .attribute(TEXT, "level")
                .and_then(|level| level.parse::<usize>().ok())
                == Some(index + 1)
        })?;
        let format = style.attribute(STYLE, "num-format").unwrap_or("");
        if format.is_empty() {
            return None;
        }
        let text = numbered_label(style, &level_formats(outline), &self.outline, index);
        Some(ListLabel {
            text,
            suffix: LabelSuffix::Space,
            font: RunLook::default(),
        })
    }

    /// A `text:list`: each item's paragraphs, nested lists one level deeper.
    fn list(
        &mut self,
        list: &Element,
        scope: Scope,
        inherited: Option<&str>,
        level: usize,
        blocks: &mut Vec<Block>,
    ) {
        // A list that names no style takes the one its first paragraph's style names.
        let from_paragraph = list
            .descendant(TEXT, "p")
            .and_then(|paragraph| paragraph.attribute(TEXT, "style-name"))
            .and_then(|style| self.styles.list_style_of(scope, Some(style)));
        let style_name = list
            .attribute(TEXT, "style-name")
            .or(inherited)
            .or(from_paragraph.as_deref());
        let key = style_name.unwrap_or("").to_owned();
        if level == 0 {
            let continues = list.attribute(TEXT, "continue-numbering") == Some("true")
                || list.attribute(TEXT, "continue-list").is_some();
            if !continues || !self.lists.contains_key(&key) {
                let id = self.next_list_id;
                self.next_list_id += 1;
                self.lists.insert(
                    key.clone(),
                    ListState {
                        id,
                        counters: [0; MAX_LIST_LEVEL],
                    },
                );
            }
        }
        let level = level.min(MAX_LIST_LEVEL - 1);
        let list_style = style_name.and_then(|name| self.styles.list_style(scope, name));
        let formats = list_style.map(level_formats).unwrap_or_default();
        let level_style = list_style
            .and_then(|style| {
                style.elements().find(|level_style| {
                    level_style
                        .attribute(TEXT, "level")
                        .and_then(|value| value.parse::<usize>().ok())
                        == Some(level + 1)
                })
            })
            .cloned();
        for item in list.elements() {
            let header = item.is(TEXT, "list-header");
            if !header && !item.is(TEXT, "list-item") {
                continue;
            }
            let mut first = !header;
            for child in item.elements() {
                match (child.namespace, child.name.as_str()) {
                    (TEXT, "p" | "h") => {
                        let place = self.list_place(
                            &key,
                            level,
                            level_style.as_ref(),
                            &formats,
                            item,
                            first,
                        );
                        first = false;
                        self.paragraph(child, scope, Some(&place), blocks);
                    }
                    (TEXT, "list") => self.list(child, scope, style_name, level + 1, blocks),
                    (TABLE, "table") => {
                        if let Some(table) = self.table(child, scope) {
                            blocks.push(table);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// The place in its list of an item's paragraph, numbering the first one.
    fn list_place(
        &mut self,
        key: &str,
        level: usize,
        level_style: Option<&Element>,
        formats: &[String],
        item: &Element,
        first: bool,
    ) -> ListPlace {
        let ordered = level_style.is_some_and(|style| style.is(TEXT, "list-level-style-number"));
        let state = self.lists.entry(key.to_owned()).or_insert(ListState {
            id: 0,
            counters: [0; MAX_LIST_LEVEL],
        });
        let id = state.id;
        let mut label = None;
        if first {
            let start = item
                .attribute(TEXT, "start-value")
                .or_else(|| level_style.and_then(|style| style.attribute(TEXT, "start-value")))
                .and_then(|start| start.parse::<u32>().ok());
            state.counters[level] = match start {
                Some(start) if item.attribute(TEXT, "start-value").is_some() => start,
                Some(start) if state.counters[level] == 0 => start,
                _ => state.counters[level] + 1,
            };
            for deeper in &mut state.counters[level + 1..] {
                *deeper = 0;
            }
            let counters = state.counters;
            label = level_style.and_then(|style| self.list_label(style, formats, &counters, level));
        }
        let indent = level_style.and_then(list_indent);
        ListPlace {
            level,
            id,
            label,
            ordered,
            indent,
        }
    }

    fn list_label(
        &self,
        style: &Element,
        formats: &[String],
        counters: &[u32; MAX_LIST_LEVEL],
        level: usize,
    ) -> Option<ListLabel> {
        let font = style
            .child(STYLE, "text-properties")
            .map(|properties| self.styles.text_properties(properties).0)
            .unwrap_or_default();
        let text = if style.is(TEXT, "list-level-style-bullet") {
            bullet_label(
                style.attribute(TEXT, "bullet-char").unwrap_or("\u{2022}"),
                font.family.as_deref(),
            )
        } else if style.is(TEXT, "list-level-style-number") {
            let label = numbered_label(style, formats, counters, level);
            if label.is_empty() {
                return None;
            }
            label
        } else {
            "\u{2022}".to_owned()
        };
        let suffix = style
            .child(STYLE, "list-level-properties")
            .and_then(|properties| properties.child(STYLE, "list-level-label-alignment"))
            .and_then(|alignment| alignment.attribute(TEXT, "label-followed-by"))
            .map_or(LabelSuffix::Tab, |followed| match followed {
                "space" => LabelSuffix::Space,
                "nothing" => LabelSuffix::Nothing,
                _ => LabelSuffix::Tab,
            });
        Some(ListLabel { text, suffix, font })
    }

    /// The inline content of a paragraph or heading, with the text boxes it anchors set aside.
    fn content(
        &mut self,
        paragraph: &Element,
        scope: Scope,
        paragraph_flags: TextFlags,
    ) -> Vec<Inline> {
        let mut content = Vec::new();
        let mut text = TextState { space_before: true };
        self.inlines(
            paragraph,
            scope,
            &RunLook::default(),
            paragraph_flags,
            None,
            &mut text,
            &mut content,
        );
        trim_end(&mut content);
        content
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "a run carries its style, its link and the space before it down the tree"
    )]
    #[expect(clippy::too_many_lines, reason = "one match over every inline element")]
    fn inlines(
        &mut self,
        container: &Element,
        scope: Scope,
        look: &RunLook,
        flags: TextFlags,
        link: Option<&str>,
        text: &mut TextState,
        content: &mut Vec<Inline>,
    ) {
        for node in &container.children {
            let element = match node {
                Node::Text(raw) => {
                    let collapsed = collapse(raw, text);
                    self.push_text(content, &collapsed, look, flags, link);
                    continue;
                }
                Node::Element(element) => element,
            };
            match (element.namespace, element.name.as_str()) {
                (TEXT, "s") => {
                    let count = element
                        .attribute(TEXT, "c")
                        .and_then(|count| count.parse::<usize>().ok())
                        .unwrap_or(1)
                        .clamp(1, 1_000);
                    self.push_text(content, &" ".repeat(count), look, flags, link);
                    text.space_before = true;
                }
                (TEXT, "tab") => {
                    self.push_text(content, "\t", look, flags, link);
                    text.space_before = true;
                }
                (TEXT, "line-break") => {
                    content.push(Inline::HardBreak);
                    text.space_before = true;
                }
                (TEXT, "span") => {
                    let (own, own_flags) =
                        self.styles
                            .text_look(scope, "text", element.attribute(TEXT, "style-name"));
                    let merged = own.or(look.clone());
                    self.inlines(
                        element,
                        scope,
                        &merged,
                        own_flags.or(flags),
                        link,
                        text,
                        content,
                    );
                }
                (TEXT, "a") => {
                    let href = element
                        .attribute(XLINK, "href")
                        .and_then(|href| self.accept_link(href));
                    let (own, own_flags) =
                        self.styles
                            .text_look(scope, "text", element.attribute(TEXT, "style-name"));
                    let merged = own.or(look.clone());
                    let link = href.as_deref().or(link);
                    self.inlines(
                        element,
                        scope,
                        &merged,
                        own_flags.or(flags),
                        link,
                        text,
                        content,
                    );
                }
                (TEXT, "note") => self.note(element, scope, content),
                (TEXT, "page-number" | "page-count") => {
                    let field = if element.name == "page-number" {
                        PageField::Page
                    } else {
                        PageField::Pages
                    };
                    let shown = element.text();
                    let shown = if shown.trim().is_empty() {
                        "1".to_owned()
                    } else {
                        shown
                    };
                    let field_look = RunLook {
                        field: Some(field),
                        ..look.clone()
                    };
                    self.push_text(content, shown.trim(), &field_look, flags, link);
                    text.space_before = false;
                }
                (
                    TEXT,
                    "date" | "time" | "author-name" | "author-initials" | "chapter" | "title"
                    | "subject" | "file-name" | "sequence" | "sequence-ref" | "bookmark-ref"
                    | "reference-ref" | "note-ref" | "user-defined" | "user-field-get"
                    | "variable-get" | "variable-set" | "expression" | "text-input"
                    | "creation-date" | "creation-time" | "creator" | "description" | "keywords"
                    | "initial-creator" | "print-date" | "modification-date" | "editing-cycles"
                    | "word-count" | "character-count" | "paragraph-count" | "sender-firstname"
                    | "sender-lastname" | "sender-company" | "placeholder" | "hidden-text"
                    | "conditional-text" | "drop-down" | "database-display",
                ) => {
                    if element.attribute(TEXT, "is-hidden") == Some("true") {
                        self.report.hidden_runs += 1;
                        continue;
                    }
                    self.report.fields_kept_as_text += 1;
                    self.inlines(element, scope, look, flags, link, text, content);
                }
                (TEXT, "meta" | "meta-field" | "ruby" | "ruby-base" | "number") => {
                    self.inlines(element, scope, look, flags, link, text, content);
                }
                (OFFICE, "annotation") => self.annotation(element, scope, content),
                (OFFICE, "annotation-end") => {
                    if let Some(name) = element.attribute(OFFICE, "name") {
                        let names = &self.comment_names;
                        self.open_comments
                            .retain(|open| names[*open].0.as_deref() != Some(name));
                    }
                }
                (TEXT, "change-start") => {
                    if let Some(id) = element.attribute(TEXT, "change-id")
                        && let Some(change) = self.changes.get(id)
                        && change.deleted.is_none()
                    {
                        self.open_changes.push((id.to_owned(), change.mark.clone()));
                    }
                }
                (TEXT, "change-end") => {
                    if let Some(id) = element.attribute(TEXT, "change-id") {
                        self.open_changes.retain(|(open, _)| open != id);
                    }
                }
                (TEXT, "change") => self.deletion(element, scope, content),
                (DRAW, "a") => {
                    for child in element.elements() {
                        self.drawing(child, scope, content);
                    }
                }
                (DRAW, _) => self.drawing(element, scope, content),
                _ => {}
            }
        }
    }

    fn push_text(
        &mut self,
        content: &mut Vec<Inline>,
        text: &str,
        look: &RunLook,
        flags: TextFlags,
        link: Option<&str>,
    ) {
        if text.is_empty() || self.remaining_chars == 0 {
            if !text.is_empty() {
                self.report.truncated = true;
            }
            return;
        }
        let taken: String = text.chars().take(self.remaining_chars).collect();
        let count = taken.chars().count();
        if count < text.chars().count() {
            self.report.truncated = true;
        }
        self.remaining_chars -= count;
        let mut marks = marks_of(look, flags, link);
        marks.extend(self.open_changes.iter().map(|(_, mark)| mark.clone()));
        marks.extend(
            self.open_comments
                .iter()
                .map(|comment| Mark::Comment { comment: *comment }),
        );
        let run_look = RunLook {
            bold: None,
            italic: None,
            ..look.clone()
        };
        if let Some(Inline::Text {
            text: previous,
            marks: previous_marks,
            look: previous_look,
        }) = content.last_mut()
            && *previous_marks == marks
            && *previous_look == run_look
        {
            previous.push_str(&taken);
            return;
        }
        content.push(Inline::Text {
            text: taken,
            marks,
            look: run_look,
        });
    }

    fn accept_link(&mut self, href: &str) -> Option<String> {
        let lower = href.trim().to_ascii_lowercase();
        if lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("mailto:")
        {
            Some(href.trim().to_owned())
        } else {
            self.report.unsupported_links += 1;
            None
        }
    }

    fn note(&mut self, note: &Element, scope: Scope, content: &mut Vec<Inline>) {
        let kind = if note.attribute(TEXT, "note-class") == Some("endnote") {
            self.report.endnotes += 1;
            NoteKind::Endnote
        } else {
            self.report.footnotes += 1;
            NoteKind::Footnote
        };
        if self.reading_note {
            return;
        }
        let Some(body) = note.child(TEXT, "note-body") else {
            return;
        };
        self.reading_note = true;
        let floating = std::mem::take(&mut self.floating);
        let mut blocks = Vec::new();
        self.blocks(body, scope, &mut blocks);
        self.floating = floating;
        self.reading_note = false;
        self.notes.push(Note { kind, blocks });
        content.push(Inline::NoteReference {
            note: self.notes.len() - 1,
        });
    }

    /// An `office:annotation`: a comment on the text up to the `office:annotation-end` of its
    /// name, or on the place it stands when it has none.
    fn annotation(&mut self, element: &Element, scope: Scope, content: &mut Vec<Inline>) {
        self.report.comments += 1;
        if self.reading_comment || self.reading_note {
            return;
        }
        self.reading_comment = true;
        let floating = std::mem::take(&mut self.floating);
        let open = std::mem::take(&mut self.open_comments);
        let changes = std::mem::take(&mut self.open_changes);
        let mut blocks = Vec::new();
        self.blocks(element, scope, &mut blocks);
        self.open_changes = changes;
        self.open_comments = open;
        self.floating = floating;
        self.reading_comment = false;
        let text = |name: &str| {
            element
                .elements()
                .find(|child| child.namespace == Namespace::Other && child.name == name)
                .map(Element::text)
                .filter(|value| !value.trim().is_empty())
        };
        self.comments.push(Comment {
            author: text("creator").unwrap_or_default(),
            initials: text("creator-initials"),
            date: text("date"),
            blocks,
            parent: None,
            resolved: element.attribute(Namespace::LoExt, "resolved") == Some("true"),
        });
        let name = element.attribute(OFFICE, "name").map(str::to_owned);
        self.comment_names.push((
            name.clone(),
            element
                .attribute(Namespace::LoExt, "parent-name")
                .map(str::to_owned),
        ));
        let index = self.comments.len() - 1;
        if name.is_some() {
            self.open_comments.push(index);
        } else {
            content.push(Inline::CommentAnchor { comment: index });
        }
    }

    /// A `text:change` where a deletion took text away: the text, marked as deleted, when
    /// changes are kept; its paragraphs are joined by line breaks.
    fn deletion(&mut self, element: &Element, scope: Scope, content: &mut Vec<Inline>) {
        let Some(id) = element.attribute(TEXT, "change-id") else {
            return;
        };
        let Some((mark, deleted)) = self
            .changes
            .get(id)
            .and_then(|change| Some((change.mark.clone(), change.deleted.clone()?)))
        else {
            return;
        };
        self.open_changes.push((id.to_owned(), mark));
        let mut first = true;
        for paragraph in deleted
            .elements()
            .filter(|paragraph| paragraph.is(TEXT, "p") || paragraph.is(TEXT, "h"))
        {
            if !first {
                content.push(Inline::HardBreak);
            }
            first = false;
            let mut text = TextState {
                space_before: false,
            };
            self.inlines(
                paragraph,
                scope,
                &RunLook::default(),
                TextFlags::default(),
                None,
                &mut text,
                content,
            );
        }
        self.open_changes.retain(|(open, _)| open != id);
    }

    /// The comments read, each reply pointing at the comment it answers.
    fn threaded_comments(&mut self) -> Vec<Comment> {
        let mut comments = std::mem::take(&mut self.comments);
        for (index, (_, parent)) in self.comment_names.iter().enumerate() {
            if let Some(parent) = parent {
                comments[index].parent = self
                    .comment_names
                    .iter()
                    .position(|(name, _)| name.as_ref() == Some(parent))
                    .filter(|position| *position != index);
            }
        }
        comments
    }

    /// A drawing inside a paragraph: a picture, a text box set after the paragraph, or a
    /// shape counted.
    fn drawing(&mut self, element: &Element, scope: Scope, content: &mut Vec<Inline>) {
        match element.name.as_str() {
            // The parts of a shape's outline, not shapes of their own.
            "enhanced-geometry" | "glue-point" | "equation" | "handle" => {}
            "frame" => {
                // An object comes with a picture of itself: the formula or chart it holds is
                // read first, and the picture only stands in for what cannot be.
                let object = element.child(DRAW, "object");
                if let Some(object) = object
                    && (self.formula(element, object, content)
                        || self.chart(element, object, content))
                {
                } else if let Some(image) = element.child(DRAW, "image") {
                    let ole = element.child(DRAW, "object-ole");
                    if object.is_some() || ole.is_some() {
                        self.report.embedded_objects += 1;
                    }
                    let before = content.len();
                    self.image(element, image, scope, content);
                    if let (Some(ole), Some(Inline::Image { media, .. })) =
                        (ole, content.get(before))
                    {
                        self.keep_object(ole, *media);
                    }
                } else if let Some(text_box) = element.child(DRAW, "text-box") {
                    self.report.text_boxes += 1;
                    let frame = self.text_frame(element, scope);
                    self.text_box(text_box, scope, frame.as_ref());
                } else if object.is_some() || element.child(DRAW, "object-ole").is_some() {
                    self.report.embedded_objects += 1;
                }
            }
            "g" => {
                let anchor = element
                    .attribute(TEXT, "anchor-type")
                    .map(str::to_owned)
                    .or_else(|| self.group_anchor.clone());
                let style = element
                    .attribute(DRAW, "style-name")
                    .map(str::to_owned)
                    .or_else(|| self.group_style.clone());
                let outer = std::mem::replace(&mut self.group_anchor, anchor);
                let outer_style = std::mem::replace(&mut self.group_style, style);
                for child in element.elements().filter(|child| child.namespace == DRAW) {
                    self.drawing(child, scope, content);
                }
                self.group_anchor = outer;
                self.group_style = outer_style;
            }
            _ => {
                // A shape whose outline is a preset keeps its text editable in a text box of that
                // shape; any other keeps its text in its picture.
                let is_text_box = (element.name != "custom-shape"
                    || frames::frame_shape(element).is_some())
                    && element
                        .descendant(TEXT, "p")
                        .is_some_and(|paragraph| !all_text(paragraph).trim().is_empty());
                if is_text_box {
                    self.report.text_boxes += 1;
                    let frame = self.text_frame(element, scope);
                    self.text_box(element, scope, frame.as_ref());
                } else if let Some((media, width, height, anchor)) =
                    self.shape_picture(element, scope)
                {
                    #[expect(
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss,
                        reason = "the size is a page's"
                    )]
                    let pixels = |points: f64| {
                        Some((points * PIXELS_PER_POINT).round().clamp(1.0, 20_000.0) as u32)
                    };
                    content.push(Inline::Image {
                        media,
                        alt: None,
                        width: pixels(width),
                        height: pixels(height),
                        anchor: anchor.map(Box::new),
                    });
                } else {
                    self.report.shapes += 1;
                }
            }
        }
    }

    /// A chart object drawn as an SVG image the size of its frame; `false` when the object is
    /// not a chart this reader draws.
    /// A formula object: its `MathML` as an equation, in the line when the frame is anchored as
    /// a character and on a line of its own otherwise.
    fn formula(&mut self, frame: &Element, object: &Element, content: &mut Vec<Inline>) -> bool {
        let Some(xml) = object
            .attribute(XLINK, "href")
            .and_then(odf_object_part)
            .and_then(|part| {
                read_text_member(self.package, &part, self.limits)
                    .ok()
                    .flatten()
            })
        else {
            return false;
        };
        let Ok(root) = parse_tree(&xml, |namespace, name| {
            namespace == Namespace::MathMl && matches!(name, "mi" | "mn" | "mo" | "mtext")
        }) else {
            return false;
        };
        if !root.is(Namespace::MathMl, "math") {
            return false;
        }
        self.report.equations += 1;
        let latex = crate::math::to_latex(&crate::math::mathml_to_math(&root));
        let length = latex.chars().count();
        if length > self.remaining_chars {
            self.report.truncated = true;
            return true;
        }
        self.remaining_chars -= length;
        if !latex.is_empty() {
            content.push(Inline::Math {
                latex,
                display: root.attribute(Namespace::None, "display") == Some("block")
                    || frame.attribute(TEXT, "anchor-type") != Some("as-char"),
            });
        }
        true
    }

    fn chart(&mut self, frame: &Element, object: &Element, content: &mut Vec<Inline>) -> bool {
        let size = |name: &str| {
            frame
                .attribute(SVG, name)
                .and_then(points)
                .filter(|points| *points > 0.0)
        };
        let (Some(width), Some(height)) = (size("width"), size("height")) else {
            return false;
        };
        let Some(xml) = object
            .attribute(XLINK, "href")
            .and_then(odf_object_part)
            .and_then(|part| {
                read_text_member(self.package, &part, self.limits)
                    .ok()
                    .flatten()
            })
        else {
            return false;
        };
        #[expect(clippy::cast_possible_truncation, reason = "frame sizes are a page's")]
        let emu = |points: f64| (points * 12_700.0).round() as i64;
        let Some((bytes, title)) = odf_chart_picture(&xml, emu(width), emu(height)) else {
            return false;
        };
        self.report.charts += 1;
        let size = bytes.len() as u64;
        if self.media.len() >= self.limits.max_images
            || self.media_bytes.saturating_add(size) > self.limits.max_total_image_bytes
        {
            self.report.images_over_limit += 1;
            return true;
        }
        self.media_bytes += size;
        self.media.push(Media {
            content_type: "image/svg+xml",
            bytes,
        });
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the size is a page's"
        )]
        let pixels =
            |points: f64| Some((points * PIXELS_PER_POINT).round().clamp(1.0, 20_000.0) as u32);
        content.push(Inline::Image {
            media: self.media.len() - 1,
            alt: title.or_else(|| Some("Grafico".to_owned())),
            width: pixels(width),
            height: pixels(height),
            anchor: None,
        });
        true
    }

    fn text_box(&mut self, container: &Element, scope: Scope, frame: Option<&TextFrame>) {
        let outer = std::mem::take(&mut self.floating);
        let mut blocks = Vec::new();
        self.blocks(container, scope, &mut blocks);
        if let Some(frame) = frame {
            set_frame(&mut blocks, frame);
        }
        self.floating = outer;
        self.floating.extend(blocks);
    }

    /// The OLE object of a `draw:object-ole`, a compound file of the package, kept with the
    /// picture that shows it.
    fn keep_object(&mut self, ole: &Element, media: usize) {
        if self.objects.iter().any(|known| known.media == media) {
            return;
        }
        let Some(href) = ole
            .attribute(XLINK, "href")
            .map(|href| href.trim_start_matches("./").to_owned())
            .filter(|href| !href.contains("://"))
        else {
            return;
        };
        let Ok(Some(bytes)) = read_member_bytes(self.package, &href, self.limits.max_member_bytes)
        else {
            return;
        };
        let name = href.rsplit('/').next().unwrap_or(&href).to_owned();
        self.objects.push(EmbeddedObject {
            media,
            program: objects::program_of_compound_file(&bytes),
            name,
            bytes,
        });
    }

    fn image(&mut self, frame: &Element, image: &Element, scope: Scope, content: &mut Vec<Inline>) {
        let Some(href) = image.attribute(XLINK, "href") else {
            self.report.missing_images += 1;
            return;
        };
        let href = href.trim_start_matches("./").to_owned();
        if href.contains("://") {
            self.report.missing_images += 1;
            return;
        }
        let index = if let Some(known) = self.media_by_href.get(&href) {
            *known
        } else {
            let loaded = self.load_image(&href);
            self.media_by_href.insert(href, loaded);
            loaded
        };
        let Some(media) = index else {
            return;
        };
        let alt = [frame.child(SVG, "title"), frame.child(SVG, "desc")]
            .into_iter()
            .flatten()
            .map(Element::text)
            .map(|text| text.trim().to_owned())
            .find(|text| !text.is_empty())
            .or_else(|| frame.attribute(DRAW, "name").map(str::to_owned));
        let pixels = |name: &str| {
            frame
                .attribute(SVG, name)
                .and_then(points)
                .filter(|points| *points > 0.0)
                .map(|points| {
                    #[expect(
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss,
                        reason = "the size is clamped to a page's"
                    )]
                    let pixels = (points * PIXELS_PER_POINT).round().clamp(1.0, 20_000.0) as u32;
                    pixels
                })
        };
        let anchor = self.frame_anchor(frame, scope).map(Box::new);
        content.push(Inline::Image {
            media,
            alt,
            width: pixels("width"),
            height: pixels("height"),
            anchor,
        });
    }

    fn load_image(&mut self, href: &str) -> Option<usize> {
        if self.media.len() >= self.limits.max_images {
            self.report.images_over_limit += 1;
            return None;
        }
        let cap = self
            .limits
            .max_image_bytes
            .min(self.limits.max_member_bytes);
        match read_member_bytes(self.package, href, cap) {
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

    /// A `table:table`, its columns, header rows and spans.
    fn table(&mut self, table: &Element, scope: Scope) -> Option<Block> {
        let mut column_widths_twips = Vec::new();
        let mut rows = Vec::new();
        let mut state = TableState::default();
        self.table_part(
            table,
            scope,
            false,
            &mut column_widths_twips,
            &mut rows,
            &mut state,
        );
        if rows.is_empty() {
            return None;
        }
        Some(Block::Table {
            rows,
            look: TableLook {
                column_widths_twips,
                borders: state.any_border,
                cell_margins_twips: state.padding.unwrap_or([None; 4]),
                edges: Borders::default(),
                frame: None,
            },
        })
    }

    fn table_part(
        &mut self,
        part: &Element,
        scope: Scope,
        header: bool,
        widths: &mut Vec<u32>,
        rows: &mut Vec<TableRow>,
        table: &mut TableState,
    ) {
        for element in part.elements().filter(|element| element.namespace == TABLE) {
            match element.name.as_str() {
                "table-column" => {
                    let width = self
                        .styles
                        .properties(
                            scope,
                            "table-column",
                            element.attribute(TABLE, "style-name"),
                            "table-column-properties",
                        )
                        .into_iter()
                        .find_map(|properties| properties.attribute(STYLE, "column-width"))
                        .and_then(twips)
                        .map_or(0, |width| u32::try_from(width.max(0)).unwrap_or(0));
                    for _ in 0..repeat(element, "number-columns-repeated").min(64) {
                        widths.push(width);
                    }
                }
                "table-header-rows" => self.table_part(element, scope, true, widths, rows, table),
                "table-columns" | "table-column-group" | "table-rows" | "table-row-group" => {
                    self.table_part(element, scope, header, widths, rows, table);
                }
                "table-row" => {
                    let row = self.table_row(element, scope, header, table);
                    let empty = row.cells.iter().all(|cell| cell.blocks.is_empty());
                    let times = repeat(element, "number-rows-repeated");
                    let times = if empty {
                        times.min(1)
                    } else {
                        times.min(MAX_REPEAT)
                    };
                    for _ in 0..times {
                        if rows.len() >= MAX_TABLE_ROWS {
                            return;
                        }
                        rows.push(row.clone());
                    }
                }
                _ => {}
            }
        }
    }

    fn table_row(
        &mut self,
        row: &Element,
        scope: Scope,
        header: bool,
        table: &mut TableState,
    ) -> TableRow {
        let mut cells = Vec::new();
        for cell in row.elements().filter(|cell| cell.is(TABLE, "table-cell")) {
            let style = cell.attribute(TABLE, "style-name");
            let properties =
                self.styles
                    .properties(scope, "table-cell", style, "table-cell-properties");
            let attribute = |namespace: Namespace, name: &str| {
                properties
                    .iter()
                    .find_map(|properties| properties.attribute(namespace, name))
            };
            let edge = |name: &str| attribute(FO, name).map(border);
            let all = edge("border");
            let borders = Borders {
                top: edge("border-top").or_else(|| all.clone()),
                left: edge("border-left").or_else(|| all.clone()),
                bottom: edge("border-bottom").or_else(|| all.clone()),
                right: edge("border-right").or_else(|| all.clone()),
                inside_horizontal: None,
                inside_vertical: None,
            };
            table.any_border |= borders.any_visible();
            if table.padding.is_none() {
                let side = |name: &str| {
                    attribute(FO, name)
                        .or_else(|| attribute(FO, "padding"))
                        .and_then(twips)
                        .map(|twips| u32::try_from(twips.max(0)).unwrap_or(0))
                };
                table.padding = Some([
                    side("padding-top"),
                    side("padding-left"),
                    side("padding-bottom"),
                    side("padding-right"),
                ]);
            }
            let look = CellLook {
                fill: attribute(FO, "background-color").and_then(colour),
                borders,
                vertical: attribute(STYLE, "vertical-align")
                    .and_then(super::styles::vertical_alignment),
            };
            let mut blocks = Vec::new();
            self.blocks(cell, scope, &mut blocks);
            let span = |name: &str| {
                cell.attribute(TABLE, name)
                    .and_then(|span| span.parse::<u32>().ok())
                    .unwrap_or(1)
                    .clamp(1, 1_000)
            };
            let table_cell = TableCell {
                col_span: span("number-columns-spanned"),
                row_span: span("number-rows-spanned"),
                blocks,
                look,
            };
            for _ in 0..repeat(cell, "number-columns-repeated").min(64) {
                cells.push(table_cell.clone());
            }
        }
        TableRow { header, cells }
    }

    /// The headers and footers of a master page, each read in the scope of `styles.xml`.
    fn headers_footers(&mut self, master: &Element) -> HeadersFooters {
        let mut part = |name: &str| -> Option<Vec<Block>> {
            let element = master.child(STYLE, name)?;
            if element.attribute(STYLE, "display") == Some("false") {
                return None;
            }
            self.report.headers_and_footers += 1;
            let mut blocks = Vec::new();
            self.blocks(element, Scope::Master, &mut blocks);
            Some(blocks)
        };
        let header = part("header").unwrap_or_default();
        let footer = part("footer").unwrap_or_default();
        let first_header = part("header-first");
        let first_footer = part("footer-first");
        let even_header = part("header-left");
        let even_footer = part("footer-left");
        HeadersFooters {
            header,
            footer,
            first_header,
            first_footer,
            even_header,
            even_footer,
        }
    }
}

/// What a table learns from its cells as it is read.
#[derive(Debug, Default)]
struct TableState {
    any_border: bool,
    /// The padding of the first cell, which the tree keeps as the table's cell margins.
    padding: Option<[Option<u32>; 4]>,
}

/// What a paragraph style makes of its paragraphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Body,
    Quote,
    Code,
}

/// The role of a paragraph by its style's name or display name, through its parents.
fn paragraph_role(style: Option<&str>, styles: &Styles, scope: Scope) -> Role {
    for element in styles.chain(scope, "paragraph", style) {
        let names = [
            element.attribute(STYLE, "name"),
            element.attribute(STYLE, "display-name"),
        ];
        for name in names.into_iter().flatten() {
            let name = name.to_ascii_lowercase().replace("_20_", " ");
            if name == "quotations" || name == "quote" || name == "citazione" {
                return Role::Quote;
            }
            if name.starts_with("preformatted")
                || name == "source text"
                || name == "testo preformattato"
            {
                return Role::Code;
            }
        }
    }
    Role::Body
}

/// How many times a row, column or cell repeats.
fn repeat(element: &Element, name: &str) -> usize {
    element
        .attribute(TABLE, name)
        .and_then(|count| count.parse::<usize>().ok())
        .unwrap_or(1)
        .max(1)
}

/// The number format of each level of a list or outline style, by level from 0.
fn level_formats(style: &Element) -> Vec<String> {
    let mut formats = vec![String::new(); MAX_LIST_LEVEL];
    for level in style.elements() {
        if let Some(index) = level
            .attribute(TEXT, "level")
            .and_then(|level| level.parse::<usize>().ok())
            .filter(|level| (1..=MAX_LIST_LEVEL).contains(level))
        {
            level
                .attribute(STYLE, "num-format")
                .unwrap_or(if level.is(TEXT, "list-level-style-bullet") {
                    ""
                } else {
                    "1"
                })
                .clone_into(&mut formats[index - 1]);
        }
    }
    formats
}

/// The label of a numbered level: its prefix, its number and those of the levels above it it
/// shows, each in its own level's format, its suffix.
fn numbered_label(
    style: &Element,
    formats: &[String],
    counters: &[u32; MAX_LIST_LEVEL],
    level: usize,
) -> String {
    let format = style.attribute(STYLE, "num-format").unwrap_or("1");
    if format.is_empty() {
        return String::new();
    }
    let shown = style
        .attribute(TEXT, "display-levels")
        .and_then(|levels| levels.parse::<usize>().ok())
        .unwrap_or(1)
        .clamp(1, level + 1);
    let numbers: Vec<String> = (level + 1 - shown..=level)
        .map(|index| {
            let own = formats
                .get(index)
                .filter(|format| !format.is_empty() && index != level)
                .map_or(format, String::as_str);
            number_label(counters[index].max(1), own)
        })
        .collect();
    format!(
        "{}{}{}",
        style.attribute(STYLE, "num-prefix").unwrap_or(""),
        numbers.join("."),
        style.attribute(STYLE, "num-suffix").unwrap_or("")
    )
}

/// The indent of a list level: its left margin and its first line, from the label alignment
/// or the older label width.
fn list_indent(style: &Element) -> Option<(i32, i32)> {
    let properties = style.child(STYLE, "list-level-properties")?;
    if let Some(alignment) = properties.child(STYLE, "list-level-label-alignment") {
        let left = alignment.attribute(FO, "margin-left").and_then(twips)?;
        let first = alignment
            .attribute(FO, "text-indent")
            .and_then(twips)
            .unwrap_or(0);
        return Some((left, first));
    }
    let before = properties
        .attribute(TEXT, "space-before")
        .and_then(twips)
        .unwrap_or(0);
    let width = properties
        .attribute(TEXT, "min-label-width")
        .and_then(twips)
        .unwrap_or(0);
    (before != 0 || width != 0).then_some((before + width, -width))
}

/// Tab stops of an odt count from the paragraph's indent; the tree counts them from the margin.
fn relative_tabs(look: &mut ParagraphLook) {
    let indent = look.indent_left_twips.unwrap_or(0);
    if indent != 0 {
        for tab in &mut look.tabs {
            tab.position_twips += indent;
        }
    }
}

/// Whether the text read so far ends in a space, for collapsing the next.
struct TextState {
    space_before: bool,
}

/// Text with its runs of whitespace collapsed to one space, as `OpenDocument` lays it out; a
/// space after a space, or at the start of the paragraph, is dropped.
fn collapse(raw: &str, state: &mut TextState) -> String {
    let mut collapsed = String::with_capacity(raw.len());
    for character in raw.chars() {
        if matches!(character, ' ' | '\t' | '\n' | '\r') {
            if !state.space_before {
                collapsed.push(' ');
                state.space_before = true;
            }
        } else {
            collapsed.push(character);
            state.space_before = false;
        }
    }
    collapsed
}

/// Drop the spaces a paragraph ends with.
fn trim_end(content: &mut Vec<Inline>) {
    while let Some(Inline::Text { text, .. }) = content.last_mut() {
        let trimmed = text.trim_end_matches(' ').len();
        text.truncate(trimmed);
        if text.is_empty() {
            content.pop();
        } else {
            break;
        }
    }
}

/// The marks of a run.
fn marks_of(look: &RunLook, flags: TextFlags, link: Option<&str>) -> Vec<Mark> {
    let mut marks = Vec::new();
    let code = flags.code == Some(true);
    if look.bold == Some(true) {
        marks.push(Mark::Bold);
    }
    if look.italic == Some(true) {
        marks.push(Mark::Italic);
    }
    if flags.underline == Some(true) && link.is_none() {
        marks.push(Mark::Underline);
    }
    if flags.strike == Some(true) {
        marks.push(Mark::Strike);
    }
    if code {
        marks.push(Mark::Code);
    }
    match flags.raised {
        Some(true) => marks.push(Mark::Superscript),
        Some(false) => marks.push(Mark::Subscript),
        None => {}
    }
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
        .filter(|_| link.is_none())
        .map(|colour| format!("#{colour}"));
    // A code run keeps no font of its own: the code style gives it.
    let font_family = look.family.clone().filter(|_| !code);
    if font_family.is_some() || font_size.is_some() || color.is_some() {
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

/// The page of a master page: its size and margins, the room its header and footer take, its
/// columns, and the document's default font and tab distance.
fn page_layout(
    styles: &Styles,
    master: Option<&Element>,
    headers_footers: &HeadersFooters,
) -> PageLayout {
    let mut layout = PageLayout {
        margins_twips: [1_134; 4],
        default_tab_twips: DEFAULT_TAB_TWIPS,
        ..PageLayout::default()
    };
    let page = master
        .and_then(|master| master.attribute(STYLE, "page-layout-name"))
        .and_then(|name| styles.page_layouts.get(name));
    if let Some(properties) = page.and_then(|page| page.child(STYLE, "page-layout-properties")) {
        let length = |name: &str| {
            properties
                .attribute(FO, name)
                .and_then(twips)
                .map(|twips| u32::try_from(twips.max(0)).unwrap_or(0))
        };
        if let (Some(width), Some(height)) = (length("page-width"), length("page-height"))
            && width >= 1_440
            && height >= 1_440
        {
            layout.width_twips = width;
            layout.height_twips = height;
        }
        let all = length("margin");
        let margin =
            |name: &str, index: usize| length(name).or(all).unwrap_or(layout.margins_twips[index]);
        layout.margins_twips = [
            margin("margin-top", 0),
            margin("margin-right", 1),
            margin("margin-bottom", 2),
            margin("margin-left", 3),
        ];
        if let Some(columns) = properties.child(STYLE, "columns") {
            layout.columns = columns
                .attribute(FO, "column-count")
                .and_then(|count| count.parse::<u32>().ok())
                .unwrap_or(1)
                .clamp(1, 45);
            if let Some(gap) = columns.attribute(FO, "column-gap").and_then(twips) {
                layout.column_gap_twips = u32::try_from(gap.max(0)).unwrap_or(0);
            }
        }
    }
    // A header sits inside the top margin's edge: the body starts below it and its spacing.
    let room = |style: &str, spacing: &str| {
        page.and_then(|page| page.child(STYLE, style))
            .and_then(|style| style.child(STYLE, "header-footer-properties"))
            .map_or(0, |properties| {
                let length = |name: &str| {
                    properties
                        .attribute(FO, name)
                        .or_else(|| properties.attribute(Namespace::OdfSvg, name))
                        .and_then(twips)
                        .map_or(0, |twips| u32::try_from(twips.max(0)).unwrap_or(0))
                };
                length("min-height").max(length("height")) + length(spacing)
            })
    };
    layout.header_distance_twips = layout.margins_twips[0];
    layout.footer_distance_twips = layout.margins_twips[2];
    if !headers_footers.header.is_empty() || headers_footers.first_header.is_some() {
        layout.margins_twips[0] += room("header-style", "margin-bottom");
    }
    if !headers_footers.footer.is_empty() || headers_footers.first_footer.is_some() {
        layout.margins_twips[2] += room("footer-style", "margin-top");
    }
    let defaults = styles.chain(Scope::Body, "paragraph", Some("Standard"));
    let (font, _) = styles.text_look(Scope::Body, "paragraph", Some("Standard"));
    layout.font_family = font.family;
    layout.font_size_half_points = font.size_half_points;
    if let Some(distance) = defaults
        .iter()
        .filter_map(|style| style.child(STYLE, "paragraph-properties"))
        .find_map(|properties| properties.attribute(STYLE, "tab-stop-distance"))
        .and_then(twips)
        .filter(|distance| *distance > 0)
    {
        layout.default_tab_twips = u32::try_from(distance).unwrap_or(DEFAULT_TAB_TWIPS);
    }
    layout
}

mod frames;

/// The text of an element and all it holds, `text:s` and `text:tab` as spaces.
pub(super) fn all_text(element: &Element) -> String {
    let mut text = String::new();
    for node in &element.children {
        match node {
            Node::Text(run) => text.push_str(run),
            Node::Element(child) if child.is(TEXT, "s") || child.is(TEXT, "tab") => text.push(' '),
            Node::Element(child) => text.push_str(&all_text(child)),
        }
    }
    text
}

#[cfg(test)]
mod tests;

/// The value of a `config:config-item` of a settings part, anywhere under `element`.
fn config_item(element: &Element, name: &str) -> Option<String> {
    for child in element.elements() {
        if child.name == "config-item"
            && child.attribute(Namespace::OdfConfig, "name") == Some(name)
        {
            return Some(child.text().trim().to_owned());
        }
        if let Some(found) = config_item(child, name) {
            return Some(found);
        }
    }
    None
}
