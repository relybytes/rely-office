// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Reads office documents for previews and search, without an office suite.
//!
//! Four entry points, each bounded so an uploaded file can never exhaust memory:
//!
//! - [`detect_format`] tells what a file is from its bytes, never from its name alone;
//! - [`extract_text`] returns the text a search index needs;
//! - [`read_workbook`] returns the sheets of a spreadsheet as tables of displayed values;
//! - [`read_document_tree`] returns a docx as a neutral tree of blocks for a rich-text editor.
//!
//! Every input is treated as untrusted: archive members are read through a size cap, and no
//! function panics on malformed bytes.

mod archive;
mod compound;
mod doc;
mod document;
mod error;
mod format;
mod limits;
mod number_format;
mod opendocument;
mod ppt;
mod presentation;
mod relationships;
mod sheet;
mod sheet_layout;
mod sheet_styles;
mod slides;
mod text;
mod word;
mod xml_text;
mod xml_tree;

#[cfg(test)]
mod test_support;

pub use document::{
    Alignment, Block, Border, BorderStyle, Borders, CellLook, DocumentTree, HeadersFooters,
    ImportReport, Inline, LabelSuffix, LineSpacing, ListLabel, Mark, Media, Note, NoteKind,
    PageField, PageLayout, ParagraphLook, RunLook, TabKind, TabLeader, TabStop, TableCell,
    TableLook, TableRow, VerticalAlignment, read_document_tree, sniff_image_type, write_docx,
};
pub use error::OfficeError;
pub use format::{DocumentFormat, detect_format};
pub use limits::Limits;
pub use number_format::{
    FormattedNumber, NumberLocale, builtin_format_code, format_number, format_text, general_number,
    is_date_code,
};
pub use sheet::{Sheet, Workbook, read_workbook, read_workbook_in};
pub use sheet_layout::{CellRange, MergePlace, SheetLayout, read_xlsx_layouts};
pub use sheet_styles::{CellBorders, CellFont, CellStyle, HorizontalAlignment};
pub use slides::{
    Bullet, Frame, Presentation, ShapeContent, ShapeGeometry, ShapeLine, Slide, SlideParagraph,
    SlideRun, SlideShape, SlideTable, SlideTableCell, SlideTableRow, TextAnchor, TextBody,
    read_presentation,
};
pub use text::{ExtractedText, extract_text};
