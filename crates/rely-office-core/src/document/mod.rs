// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! A structured reading of a docx for import into a rich-text editor: a neutral tree of blocks
//! and inline runs, the embedded images, and a report of what the tree could not carry.

mod anchor;
mod convert;
mod edges;
mod format;
mod html;
mod look;
mod markdown;
mod media;
mod notes;
mod numbering;
pub(crate) mod objects;
mod styles;
mod write;

use crate::archive::{open_package, read_text_member};
use crate::relationships::{main_part_path, read_relationships};
use crate::xml_tree::{Element, Namespace, parse_tree};
use crate::{DocumentFormat, Limits, OfficeError, detect_format};

pub(crate) use anchor::set_frame;
pub use anchor::{
    Anchor, AnchorAlign, AnchorBase, AnchorPosition, OUTLINE_BANDS, TextFrame, Wrap, WrapOutline,
};
pub use edges::{
    Border, BorderStyle, Borders, CellLook, TabKind, TabLeader, TabStop, VerticalAlignment,
};
pub use format::{LineSpacingFormat, NamedStyle, ParagraphFormat};
pub use html::{read_html_tree, write_html};
pub use look::{LabelSuffix, LineSpacing, ListLabel, PageField, ParagraphLook, RunLook, TableLook};
pub use markdown::{read_markdown_tree, write_markdown};
pub(crate) use media::image_pixel_size;
pub use media::sniff_image_type;
pub use objects::EmbeddedObject;
pub use write::write_docx;
pub(crate) use write::{escape as escape_xml, image_extension, zip_members};

pub(crate) use convert::{plain_text, push_block, smart_art_drawing, smart_art_svg};
pub(crate) use numbering::{bullet_character, bullet_label, format_number, number_label};

const MAIN_PART: &str = "word/document.xml";
/// A page side shorter than an inch is damaged; one longer than 22 inches is Word's own maximum.
const MIN_PAGE_TWIPS: u32 = 1_440;
const MAX_PAGE_TWIPS: u64 = 31_680;
/// Word's own limit of text columns.
const MAX_COLUMNS: u32 = 45;
/// Word's default distance of the header and the footer from the page edge, 1.25 cm.
const DEFAULT_HEADER_TWIPS: u32 = 708;
/// Word's default gap between text columns and between default tab stops, 1.25 cm.
const DEFAULT_GAP_TWIPS: u32 = 708;

/// A document read for an editor.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocumentTree {
    pub blocks: Vec<Block>,
    /// The embedded images, referenced from [`Inline::Image`] by their index in this list.
    pub media: Vec<Media>,
    pub report: ImportReport,
    /// The page and body font the document is laid out with, for rendering.
    pub layout: PageLayout,
    /// The footnotes and endnotes, referenced from [`Inline::NoteReference`] by their index in
    /// this list, in the order the text refers to them.
    pub notes: Vec<Note>,
    /// The headers and footers of the last section.
    pub headers_footers: HeadersFooters,
    /// Every section of a document of more than one, in order, the last one included; empty
    /// for a document of one section, whose page is `layout`.
    pub sections: Vec<Section>,
    /// The comments on the text, referenced from [`Mark::Comment`] and
    /// [`Inline::CommentAnchor`] by their index in this list, in the order the file keeps them.
    pub comments: Vec<Comment>,
    /// The objects embedded in the text, each shown by the image of its picture.
    pub objects: Vec<EmbeddedObject>,
}

/// A comment on a stretch of the document, or a reply to one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase", default)
)]
pub struct Comment {
    pub author: String,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub initials: Option<String>,
    /// When it was written, as the file stores it: ISO 8601, `2026-10-27T10:30:00Z`.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub date: Option<String>,
    pub blocks: Vec<Block>,
    /// The comment this one answers, by its index in [`DocumentTree::comments`].
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub parent: Option<usize>,
    /// The discussion is marked as done.
    pub resolved: bool,
}

/// What a reader does with the changes a document tracks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TrackedChanges {
    /// Insertions are kept and deletions dropped, as if every change were accepted.
    #[default]
    Accept,
    /// Both are kept, each run marked [`Mark::Insertion`] or [`Mark::Deletion`], so an editor
    /// can show them and a writer can write them back as changes.
    Keep,
}

/// How a document is read into its tree, beyond its [`Limits`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TreeOptions {
    pub tracked_changes: TrackedChanges,
}

/// A run of blocks laid out on one page setup: a landscape table among portrait pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// The index after the section's last block in [`DocumentTree::blocks`].
    pub end_block: usize,
    pub layout: PageLayout,
    pub headers_footers: HeadersFooters,
    /// The section goes on where the one before ends instead of on a new page.
    pub continuous: bool,
}

/// A footnote or an endnote.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct Note {
    pub kind: NoteKind,
    pub blocks: Vec<Block>,
}

/// Where a note is printed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub enum NoteKind {
    /// At the foot of the page that refers to it.
    Footnote,
    /// At the end of the document.
    Endnote,
}

/// The headers and footers of a section. The first-page ones are set only when the section has
/// a different first page, the even-page ones only when the document has different odd and even
/// pages; every other page uses the default ones.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase", default)
)]
pub struct HeadersFooters {
    pub header: Vec<Block>,
    pub footer: Vec<Block>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub first_header: Option<Vec<Block>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub first_footer: Option<Vec<Block>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub even_header: Option<Vec<Block>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub even_footer: Option<Vec<Block>>,
}

impl HeadersFooters {
    /// Whether the section has no header or footer at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.header.is_empty()
            && self.footer.is_empty()
            && [
                &self.first_header,
                &self.first_footer,
                &self.even_header,
                &self.even_footer,
            ]
            .into_iter()
            .all(|part| part.as_ref().is_none_or(Vec::is_empty))
    }
}

/// The page geometry of the last section and the font of body text, in the units Word stores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageLayout {
    /// Page width and height in twentieths of a point.
    pub width_twips: u32,
    pub height_twips: u32,
    /// Margins in twentieths of a point: top, right, bottom, left.
    pub margins_twips: [u32; 4],
    /// The distance of the header from the top edge and of the footer from the bottom edge.
    pub header_distance_twips: u32,
    pub footer_distance_twips: u32,
    /// Text columns and the gap between them.
    pub columns: u32,
    pub column_gap_twips: u32,
    /// The distance between default tab stops.
    pub default_tab_twips: u32,
    /// The typeface of body text, as the document names it.
    pub font_family: Option<String>,
    /// The size of body text in half points.
    pub font_size_half_points: Option<u32>,
    /// The space after a paragraph and the space before the next add up, as Word 2003 and
    /// `LibreOffice` lay them out; otherwise the larger of the two is kept, as Word since 2007.
    pub spacing_adds_up: bool,
}

impl Default for PageLayout {
    /// A4 with Word's default margins of 2.5 cm.
    fn default() -> Self {
        Self {
            width_twips: 11_906,
            height_twips: 16_838,
            margins_twips: [1_417; 4],
            header_distance_twips: DEFAULT_HEADER_TWIPS,
            footer_distance_twips: DEFAULT_HEADER_TWIPS,
            columns: 1,
            column_gap_twips: DEFAULT_GAP_TWIPS,
            default_tab_twips: DEFAULT_GAP_TWIPS,
            font_family: None,
            font_size_half_points: None,
            spacing_adds_up: false,
        }
    }
}

/// How a paragraph's lines sit between the margins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub enum Alignment {
    Left,
    Center,
    Right,
    Justify,
}

/// A block of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(
        tag = "type",
        rename_all = "camelCase",
        rename_all_fields = "camelCase"
    )
)]
pub enum Block {
    /// A heading, `level` 1 to 6, from the paragraph style or its outline level.
    Heading {
        level: u8,
        content: Vec<Inline>,
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        align: Option<Alignment>,
        #[cfg_attr(
            feature = "serde",
            serde(
                rename = "format",
                default,
                skip_serializing_if = "format::look_as_format::is_empty",
                with = "format::look_as_format"
            )
        )]
        look: ParagraphLook,
    },
    Paragraph {
        content: Vec<Inline>,
        /// The alignment the paragraph or its style sets; `None` is the start of the line.
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        align: Option<Alignment>,
        #[cfg_attr(
            feature = "serde",
            serde(
                rename = "format",
                default,
                skip_serializing_if = "format::look_as_format::is_empty",
                with = "format::look_as_format"
            )
        )]
        look: ParagraphLook,
    },
    /// One item of a list. Consecutive items with the same `list_id` belong to one list; `level`
    /// is the nesting depth, 0 for the outermost.
    ListItem {
        ordered: bool,
        level: u8,
        list_id: u32,
        content: Vec<Inline>,
        #[cfg_attr(
            feature = "serde",
            serde(
                rename = "format",
                default,
                skip_serializing_if = "format::look_as_format::is_empty",
                with = "format::look_as_format"
            )
        )]
        look: ParagraphLook,
    },
    Table {
        rows: Vec<TableRow>,
        #[cfg_attr(
            feature = "serde",
            serde(
                rename = "columnWidthsTwips",
                default,
                skip_serializing_if = "format::table_widths::is_empty",
                with = "format::table_widths"
            )
        )]
        look: TableLook,
    },
    /// Consecutive quote paragraphs, together.
    Blockquote {
        blocks: Vec<Block>,
    },
    /// Consecutive code paragraphs, one line each.
    CodeBlock {
        text: String,
    },
    HorizontalRule,
}

/// One row of a table.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct TableRow {
    /// Whether the row repeats as the table header.
    pub header: bool,
    pub cells: Vec<TableCell>,
}

/// One cell; a cell covered by a merge above it is folded into that cell's `row_span`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct TableCell {
    pub col_span: u32,
    pub row_span: u32,
    pub blocks: Vec<Block>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub look: CellLook,
}

/// An inline piece of a block.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(
        tag = "type",
        rename_all = "camelCase",
        rename_all_fields = "camelCase"
    )
)]
pub enum Inline {
    Text {
        text: String,
        #[cfg_attr(
            feature = "serde",
            serde(default, skip_serializing_if = "Vec::is_empty")
        )]
        marks: Vec<Mark>,
        /// In JSON the look gives only `field`: the page number or the page count a run of a
        /// header shows, which an editor fills in on each page.
        #[cfg_attr(
            feature = "serde",
            serde(
                rename = "field",
                default,
                skip_serializing_if = "format::look_field::is_empty",
                with = "format::look_field"
            )
        )]
        look: RunLook,
    },
    /// A line break inside the block.
    HardBreak,
    /// An embedded image: `media` is its index in [`DocumentTree::media`]; the size is in CSS
    /// pixels, as the document lays it out.
    Image {
        media: usize,
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        alt: Option<String>,
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        width: Option<u32>,
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
        height: Option<u32>,
        /// Where a floating image stands; `None` for one set in the line. An editor gets the
        /// image in the line.
        #[cfg_attr(feature = "serde", serde(skip))]
        anchor: Option<Box<Anchor>>,
    },
    /// The reference to a footnote or an endnote: `note` is its index in [`DocumentTree::notes`].
    /// The number it shows is its position among the notes of its kind.
    NoteReference { note: usize },
    /// The place of a comment that covers no text: `comment` is its index in
    /// [`DocumentTree::comments`]. A comment on text marks the text instead.
    CommentAnchor { comment: usize },
    /// An equation in LaTeX, `\frac{a}{b}`: set in the line, or on a line of its own when
    /// `display`.
    Math { latex: String, display: bool },
}

/// A formatting mark of a text run, in this order when several apply.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(
        tag = "type",
        rename_all = "camelCase",
        rename_all_fields = "camelCase"
    )
)]
pub enum Mark {
    Bold,
    Italic,
    Underline,
    Strike,
    /// A run set in a monospace font or a code character style.
    Code,
    Superscript,
    Subscript,
    /// The font, size and colour a run sets itself, as CSS values: `"Arial"`, `"14pt"`,
    /// `"#1F3864"`. The paragraph's own font is not repeated here.
    TextStyle {
        #[cfg_attr(
            feature = "serde",
            serde(default, skip_serializing_if = "Option::is_none")
        )]
        font_family: Option<String>,
        #[cfg_attr(
            feature = "serde",
            serde(default, skip_serializing_if = "Option::is_none")
        )]
        font_size: Option<String>,
        #[cfg_attr(
            feature = "serde",
            serde(default, skip_serializing_if = "Option::is_none")
        )]
        color: Option<String>,
    },
    /// A highlight behind the text, as a CSS colour: `"#FFFF00"`.
    Highlight {
        color: String,
    },
    /// A hyperlink; only `http`, `https` and `mailto` targets are kept.
    Link {
        href: String,
    },
    /// Text a comment is on: `comment` is its index in [`DocumentTree::comments`].
    Comment {
        comment: usize,
    },
    /// Text inserted while changes were tracked, read only with [`TrackedChanges::Keep`].
    Insertion {
        author: String,
        #[cfg_attr(
            feature = "serde",
            serde(default, skip_serializing_if = "Option::is_none")
        )]
        date: Option<String>,
    },
    /// Text deleted while changes were tracked, still in the document until the change is
    /// accepted; read only with [`TrackedChanges::Keep`].
    Deletion {
        author: String,
        #[cfg_attr(
            feature = "serde",
            serde(default, skip_serializing_if = "Option::is_none")
        )]
        date: Option<String>,
    },
    /// Text whose formatting changed while changes were tracked: `previous` holds the marks it
    /// had before, which rejecting the change puts back; read only with [`TrackedChanges::Keep`].
    FormatChange {
        author: String,
        #[cfg_attr(
            feature = "serde",
            serde(default, skip_serializing_if = "Option::is_none")
        )]
        date: Option<String>,
        #[cfg_attr(feature = "serde", serde(default))]
        previous: Vec<Mark>,
    },
}

/// An embedded image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Media {
    /// The type sniffed from the bytes (`image/png`, ...); `application/octet-stream` when the
    /// bytes match no known image.
    pub content_type: &'static str,
    pub bytes: Vec<u8>,
}

/// What the tree leaves out or changes, counted by kind.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct ImportReport {
    /// The text reached [`Limits::max_text_chars`] and the rest of the document was left out.
    pub truncated: bool,
    pub text_boxes: usize,
    pub footnotes: usize,
    pub endnotes: usize,
    pub comments: usize,
    /// Header and footer parts, which an editor's single flow has no place for.
    pub headers_and_footers: usize,
    /// Fields (page numbers, cross references, tables of contents) kept as the text they last
    /// showed.
    pub fields_kept_as_text: usize,
    pub charts: usize,
    pub smart_art: usize,
    pub equations: usize,
    /// Drawing shapes and groups without a picture.
    pub shapes: usize,
    /// OLE objects (embedded spreadsheets and the like): shown as their picture, and kept in
    /// [`DocumentTree::objects`] when the object itself is found.
    pub embedded_objects: usize,
    /// Links whose target is not `http`, `https` or `mailto`, internal bookmarks included; the
    /// text stays, the link does not.
    pub unsupported_links: usize,
    /// Images larger than [`Limits::max_image_bytes`].
    pub oversized_images: usize,
    /// Images past [`Limits::max_images`] or [`Limits::max_total_image_bytes`].
    pub images_over_limit: usize,
    /// Images whose part is missing or linked from outside the package.
    pub missing_images: usize,
    /// Tracked insertions, accepted: their text is in the tree.
    pub tracked_insertions: usize,
    /// Tracked deletions, accepted: their text is not in the tree.
    pub tracked_deletions: usize,
    /// Runs formatted as hidden text, left out.
    pub hidden_runs: usize,
}

/// Read a docx as a tree of blocks for an editor, with its images and an import report.
///
/// # Errors
///
/// [`OfficeError::UnsupportedFormat`] for anything but a docx, [`OfficeError::TooLarge`] past the
/// input or member limits, [`OfficeError::DamagedContainer`] and [`OfficeError::MalformedXml`] for
/// a damaged package, [`OfficeError::UnreadableDocument`] when the main part has no body.
pub fn read_document_tree(bytes: &[u8], limits: &Limits) -> Result<DocumentTree, OfficeError> {
    read_document_tree_with(bytes, limits, &TreeOptions::default())
}

/// [`read_document_tree_with`] for a file whose name tells its format: Markdown and HTML carry
/// no signature, so they are read only when `file_name` names them (or, for HTML, when the page
/// starts with its doctype).
///
/// # Errors
///
/// As [`read_document_tree`].
pub fn read_document_tree_named(
    bytes: &[u8],
    file_name: Option<&str>,
    limits: &Limits,
    options: &TreeOptions,
) -> Result<DocumentTree, OfficeError> {
    let format = detect_format(bytes, file_name);
    if matches!(format, DocumentFormat::Markdown | DocumentFormat::Html) {
        return read_text_tree(bytes, format, limits);
    }
    read_document_tree_with(bytes, limits, options)
}

/// Markdown or HTML bytes read as a tree: UTF-8, or Windows-1252 when they are not valid UTF-8.
pub(crate) fn read_text_tree(
    bytes: &[u8],
    format: DocumentFormat,
    limits: &Limits,
) -> Result<DocumentTree, OfficeError> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => std::borrow::Cow::Borrowed(text),
        Err(_) => encoding_rs::WINDOWS_1252.decode(bytes).0,
    };
    match format {
        DocumentFormat::Markdown => read_markdown_tree(&text, limits),
        DocumentFormat::Html => read_html_tree(&text, limits),
        other => Err(OfficeError::UnsupportedFormat(other)),
    }
}

/// The text of blocks for the search, a line per paragraph, a cell per tab.
pub(crate) fn append_tree_text(blocks: &[Block], buffer: &mut crate::text::TextBuffer) {
    for block in blocks {
        if buffer.is_truncated() {
            return;
        }
        match block {
            Block::Heading { content, .. }
            | Block::Paragraph { content, .. }
            | Block::ListItem { content, .. } => {
                for inline in content {
                    match inline {
                        Inline::Text { text, marks, .. }
                            if !marks
                                .iter()
                                .any(|mark| matches!(mark, Mark::Deletion { .. })) =>
                        {
                            buffer.push_str(text);
                        }
                        Inline::HardBreak => buffer.push('\n'),
                        Inline::Math { latex, .. } => buffer.push_str(latex),
                        _ => {}
                    }
                }
                buffer.push('\n');
            }
            Block::Table { rows, .. } => {
                for row in rows {
                    for cell in &row.cells {
                        append_tree_text(&cell.blocks, buffer);
                    }
                }
            }
            Block::Blockquote { blocks } => append_tree_text(blocks, buffer),
            Block::CodeBlock { text } => {
                buffer.push_str(text);
                buffer.push('\n');
            }
            Block::HorizontalRule => {}
        }
    }
}

/// [`read_document_tree`] with options: whether the changes a document tracks are accepted or
/// kept as marks.
///
/// # Errors
///
/// As [`read_document_tree`].
#[expect(
    clippy::too_many_lines,
    reason = "the parts of a docx are read in the order the tree needs them"
)]
pub fn read_document_tree_with(
    bytes: &[u8],
    limits: &Limits,
    options: &TreeOptions,
) -> Result<DocumentTree, OfficeError> {
    if bytes.len() as u64 > limits.max_input_bytes {
        return Err(OfficeError::TooLarge {
            what: "input".to_owned(),
            limit: limits.max_input_bytes,
        });
    }
    let format = detect_format(bytes, None);
    if format == DocumentFormat::Odt {
        return crate::opendocument::read_odt_tree(bytes, limits, *options);
    }
    if format == DocumentFormat::Doc {
        return crate::doc::read_doc_tree(bytes, limits, *options);
    }
    if format != DocumentFormat::Docx {
        return Err(OfficeError::UnsupportedFormat(format));
    }

    let mut package = open_package(bytes)?;
    let main_part = main_part_path(&mut package, limits, MAIN_PART)?;
    let xml = read_text_member(&mut package, &main_part, limits)?.ok_or_else(|| {
        OfficeError::UnreadableDocument(format!("the package has no {main_part}"))
    })?;
    let document = parse_tree(&xml, keeps_whitespace)?;
    drop(xml);
    let body = document
        .child(Namespace::Word, "body")
        .ok_or_else(|| OfficeError::UnreadableDocument("the document has no body".to_owned()))?;

    let relationships = read_relationships(&mut package, &main_part, limits)?;
    let part_of = |kind: &str, fallback: &str| {
        relationships
            .iter()
            .find(|relationship| !relationship.external && relationship.is_kind(kind))
            .map_or_else(
                || fallback.to_owned(),
                |relationship| relationship.target.clone(),
            )
    };
    let styles = read_text_member(&mut package, &part_of("styles", "word/styles.xml"), limits)?;
    let numbering = read_text_member(
        &mut package,
        &part_of("numbering", "word/numbering.xml"),
        limits,
    )?;
    let styles = styles::Styles::parse(styles.as_deref())?;
    let numbering = numbering::Numbering::parse(numbering.as_deref())?;
    let settings = read_text_member(
        &mut package,
        &part_of("settings", "word/settings.xml"),
        limits,
    )
    .ok()
    .flatten();
    let settings = notes::Settings::parse(settings.as_deref());
    let side_part = |package: &mut crate::archive::ZipPackage<'_>, kind: &str| {
        relationships
            .iter()
            .find(|relationship| !relationship.external && relationship.is_kind(kind))
            .and_then(|relationship| notes::read_part(package, &relationship.target, limits))
    };
    let note_parts = notes::NoteParts {
        footnotes: side_part(&mut package, "footnotes"),
        endnotes: side_part(&mut package, "endnotes"),
    };
    let comment_parts = notes::CommentParts::read(
        side_part(&mut package, "comments"),
        side_part(&mut package, "commentsExtended"),
    );
    // Each paragraph whose properties hold a `w:sectPr` ends a section; the body's own is the last.
    let mut groups: Vec<(Vec<&Element>, Option<&Element>)> = Vec::new();
    let mut current = Vec::new();
    for element in body.elements() {
        current.push(element);
        if let Some(section) = element
            .child(Namespace::Word, "pPr")
            .and_then(|properties| properties.child(Namespace::Word, "sectPr"))
            .filter(|_| element.is(Namespace::Word, "p"))
        {
            groups.push((std::mem::take(&mut current), Some(section)));
        }
    }
    groups.push((current, body.child(Namespace::Word, "sectPr")));
    let mut section_parts = Vec::new();
    for (_, section) in &groups {
        section_parts.push(
            section
                .map(|section| notes::section_parts(section, &relationships, &mut package, limits))
                .unwrap_or_default(),
        );
    }

    let mut report = ImportReport {
        headers_and_footers: relationships
            .iter()
            .filter(|relationship| relationship.is_kind("header") || relationship.is_kind("footer"))
            .count(),
        ..ImportReport::default()
    };
    let mut converter = convert::Converter::new(
        &mut package,
        &relationships,
        &styles,
        &numbering,
        &note_parts,
        &comment_parts,
        options.tracked_changes,
        limits,
        &mut report,
    );
    let font = styles.body_font();
    let mut blocks = Vec::new();
    let mut sections: Vec<Section> = Vec::new();
    for ((elements, section), parts) in groups.iter().zip(&section_parts) {
        blocks.extend(converter.convert_elements(elements));
        let title_page = section
            .and_then(|section| section.child(Namespace::Word, "titlePg"))
            .is_some_and(styles::on_off);
        let mut own = headers_footers(
            &mut converter,
            parts,
            title_page,
            settings.even_and_odd_headers,
        );
        // A section that names no header or footer of a kind keeps the one before it.
        if let Some(previous) = sections.last() {
            inherit(&mut own, &previous.headers_footers, parts);
        }
        let mut layout = page_layout(*section);
        layout.font_family.clone_from(&font.family);
        layout.font_size_half_points = font.size_half_points;
        layout.default_tab_twips = settings.default_tab_twips;
        layout.spacing_adds_up = settings.spacing_adds_up;
        sections.push(Section {
            end_block: blocks.len(),
            layout,
            headers_footers: own,
            continuous: section
                .and_then(|section| section.child(Namespace::Word, "type"))
                .and_then(|kind| kind.attribute(Namespace::Word, "val"))
                == Some("continuous"),
        });
    }
    let (media, notes, comments, objects) = converter.finish();
    let last = sections.last().cloned().unwrap_or_else(|| Section {
        end_block: blocks.len(),
        layout: PageLayout::default(),
        headers_footers: HeadersFooters::default(),
        continuous: false,
    });
    if sections.len() < 2 {
        sections.clear();
    }

    Ok(DocumentTree {
        blocks,
        media,
        report,
        layout: last.layout,
        notes,
        headers_footers: last.headers_footers,
        sections,
        comments,
        objects,
    })
}

/// Take from the section before the headers and footers a section does not name itself.
fn inherit(
    own: &mut HeadersFooters,
    previous: &HeadersFooters,
    parts: &[(notes::Slot, notes::Part)],
) {
    let names = |slot: notes::Slot| parts.iter().any(|(named, _)| *named == slot);
    if !names(notes::Slot::Header) {
        own.header.clone_from(&previous.header);
    }
    if !names(notes::Slot::Footer) {
        own.footer.clone_from(&previous.footer);
    }
    if own.first_header.is_some() && !names(notes::Slot::FirstHeader) {
        own.first_header.clone_from(&previous.first_header);
        own.first_header.get_or_insert_with(Vec::new);
    }
    if own.first_footer.is_some() && !names(notes::Slot::FirstFooter) {
        own.first_footer.clone_from(&previous.first_footer);
        own.first_footer.get_or_insert_with(Vec::new);
    }
    if own.even_header.is_some() && !names(notes::Slot::EvenHeader) {
        own.even_header.clone_from(&previous.even_header);
        own.even_header.get_or_insert_with(Vec::new);
    }
    if own.even_footer.is_some() && !names(notes::Slot::EvenFooter) {
        own.even_footer.clone_from(&previous.even_footer);
        own.even_footer.get_or_insert_with(Vec::new);
    }
}

/// The headers and footers of the section: a first-page pair when the section has a different
/// first page, an even-page pair when the document has different odd and even pages.
fn headers_footers<'a>(
    converter: &mut convert::Converter<'a, '_>,
    section_parts: &'a [(notes::Slot, notes::Part)],
    title_page: bool,
    even_and_odd: bool,
) -> HeadersFooters {
    let mut parts = HeadersFooters::default();
    if title_page {
        parts.first_header = Some(Vec::new());
        parts.first_footer = Some(Vec::new());
    }
    if even_and_odd {
        parts.even_header = Some(Vec::new());
        parts.even_footer = Some(Vec::new());
    }
    for (slot, part) in section_parts {
        let blocks = converter.convert_part(&part.root, &part.relationships);
        let target = match slot {
            notes::Slot::Header => Some(&mut parts.header),
            notes::Slot::Footer => Some(&mut parts.footer),
            notes::Slot::FirstHeader => parts.first_header.as_mut(),
            notes::Slot::FirstFooter => parts.first_footer.as_mut(),
            notes::Slot::EvenHeader => parts.even_header.as_mut(),
            notes::Slot::EvenFooter => parts.even_footer.as_mut(),
        };
        if let Some(target) = target {
            *target = blocks;
        }
    }
    parts
}

/// The page size and margins of a section's `w:sectPr`; A4 when it says nothing.
fn page_layout(section: Option<&Element>) -> PageLayout {
    let mut layout = PageLayout::default();
    let Some(section) = section else {
        return layout;
    };
    let twips = |element: &Element, name: &str| {
        element
            .attribute(Namespace::Word, name)
            .and_then(|value| value.parse::<i64>().ok())
            .map(|value| u32::try_from(value.unsigned_abs().min(MAX_PAGE_TWIPS)).unwrap_or(0))
    };
    if let Some(size) = section.child(Namespace::Word, "pgSz") {
        if let Some(width) = twips(size, "w").filter(|width| *width >= MIN_PAGE_TWIPS) {
            layout.width_twips = width;
        }
        if let Some(height) = twips(size, "h").filter(|height| *height >= MIN_PAGE_TWIPS) {
            layout.height_twips = height;
        }
    }
    if let Some(margins) = section.child(Namespace::Word, "pgMar") {
        for (index, name) in ["top", "right", "bottom", "left"].iter().enumerate() {
            if let Some(margin) = twips(margins, name) {
                layout.margins_twips[index] = margin;
            }
        }
        if let Some(header) = twips(margins, "header") {
            layout.header_distance_twips = header;
        }
        if let Some(footer) = twips(margins, "footer") {
            layout.footer_distance_twips = footer;
        }
    }
    if let Some(columns) = section.child(Namespace::Word, "cols") {
        if let Some(count) = twips(columns, "num") {
            layout.columns = count.clamp(1, MAX_COLUMNS);
        }
        if let Some(gap) = twips(columns, "space") {
            layout.column_gap_twips = gap;
        }
    }
    // Margins that leave no room for text are damaged, not a layout.
    let [top, right, bottom, left] = layout.margins_twips;
    if left + right + MIN_PAGE_TWIPS > layout.width_twips
        || top + bottom + MIN_PAGE_TWIPS > layout.height_twips
    {
        layout.margins_twips = PageLayout::default().margins_twips;
    }
    layout
}

/// Text runs and field instructions keep their spaces; everywhere else whitespace is layout.
fn keeps_whitespace(namespace: Namespace, name: &str) -> bool {
    (namespace == Namespace::Word && matches!(name, "t" | "instrText" | "delText"))
        || (namespace == Namespace::Math && name == "t")
}

#[cfg(test)]
mod tests;
