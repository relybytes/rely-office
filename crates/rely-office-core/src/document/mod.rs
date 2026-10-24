// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! A structured reading of a docx for import into a rich-text editor: a neutral tree of blocks
//! and inline runs, the embedded images, and a report of what the tree could not carry.

mod convert;
mod media;
mod numbering;
mod styles;

use crate::archive::{open_package, read_text_member};
use crate::relationships::{main_part_path, read_relationships};
use crate::xml_tree::{Namespace, parse_tree};
use crate::{DocumentFormat, Limits, OfficeError, detect_format};

pub use media::sniff_image_type;

const MAIN_PART: &str = "word/document.xml";

/// A document read for an editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentTree {
    pub blocks: Vec<Block>,
    /// The embedded images, referenced from [`Inline::Image`] by their index in this list.
    pub media: Vec<Media>,
    pub report: ImportReport,
}

/// A block of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize),
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
    },
    Paragraph {
        content: Vec<Inline>,
    },
    /// One item of a list. Consecutive items with the same `list_id` belong to one list; `level`
    /// is the nesting depth, 0 for the outermost.
    ListItem {
        ordered: bool,
        level: u8,
        list_id: u32,
        content: Vec<Inline>,
    },
    Table {
        rows: Vec<TableRow>,
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
    derive(serde::Serialize),
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
    derive(serde::Serialize),
    serde(rename_all = "camelCase")
)]
pub struct TableCell {
    pub col_span: u32,
    pub row_span: u32,
    pub blocks: Vec<Block>,
}

/// An inline piece of a block.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize),
    serde(
        tag = "type",
        rename_all = "camelCase",
        rename_all_fields = "camelCase"
    )
)]
pub enum Inline {
    Text {
        text: String,
        #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Vec::is_empty"))]
        marks: Vec<Mark>,
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
    },
}

/// A formatting mark of a text run, in this order when several apply.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize),
    serde(tag = "type", rename_all = "camelCase")
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
    /// A hyperlink; only `http`, `https` and `mailto` targets are kept.
    Link {
        href: String,
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
    derive(serde::Serialize),
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
    /// OLE objects (embedded spreadsheets and the like).
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
    if bytes.len() as u64 > limits.max_input_bytes {
        return Err(OfficeError::TooLarge {
            what: "input".to_owned(),
            limit: limits.max_input_bytes,
        });
    }
    let format = detect_format(bytes, None);
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
        limits,
        &mut report,
    );
    let blocks = converter.convert_blocks(body);
    let media = converter.into_media();

    Ok(DocumentTree {
        blocks,
        media,
        report,
    })
}

/// Text runs and field instructions keep their spaces; everywhere else whitespace is layout.
fn keeps_whitespace(namespace: Namespace, name: &str) -> bool {
    namespace == Namespace::Word && matches!(name, "t" | "instrText" | "delText")
}

#[cfg(test)]
mod tests;
