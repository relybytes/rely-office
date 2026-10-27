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
mod chart;
mod compound;
mod doc;
mod document;
mod error;
mod format;
mod geometry;
mod limits;
mod math;
mod metafile;
mod number_format;
mod office_art;
mod opendocument;
mod ppt;
mod presentation;
mod relationships;
mod sheet;
mod sheet_drawings;
mod sheet_layout;
mod sheet_styles;
mod slides;
mod text;
mod theme;
mod word;
mod xml_text;
mod xml_tree;

#[cfg(test)]
mod test_support;

pub use document::{
    Alignment, Anchor, AnchorAlign, AnchorBase, AnchorPosition, Block, Border, BorderStyle,
    Borders, CellLook, Comment, DocumentTree, HeadersFooters, ImportReport, Inline, LabelSuffix,
    LineSpacing, ListLabel, Mark, Media, Note, NoteKind, PageField, PageLayout, ParagraphLook,
    RunLook, Section, TabKind, TabLeader, TabStop, TableCell, TableLook, TableRow, TextFrame,
    TrackedChanges, TreeOptions, VerticalAlignment, Wrap, read_document_tree,
    read_document_tree_with, sniff_image_type, write_docx,
};
pub use error::OfficeError;
pub use format::{DocumentFormat, detect_format};
pub use geometry::{FillShade, PathCommand, PathPoint, ShapePath, preset_outline};
pub use limits::Limits;
pub use math::latex_to_typst;
pub use metafile::convert_metafile_to_svg;
pub use number_format::{
    FormattedNumber, NumberLocale, builtin_format_code, format_number, format_text, general_number,
    is_date_code,
};
pub use sheet::{Sheet, Workbook, read_workbook, read_workbook_in};
pub use sheet_drawings::{CarriedDrawing, CarriedPart, SheetDrawing};
pub use sheet_layout::{
    CellRange, MergePlace, SheetLayout, read_xlsx_drawing_parts, read_xlsx_layouts,
    read_xlsx_theme_part,
};
pub use sheet_styles::{CellBorders, CellFont, CellStyle, HorizontalAlignment};
pub use slides::{
    Animation, AnimationClass, AnimationTrigger, Bullet, Frame, Gradient, GradientKind,
    GradientStop, Paint, Presentation, ShapeContent, ShapeGeometry, ShapeLine, Slide,
    SlideParagraph, SlideRun, SlideShape, SlideTable, SlideTableCell, SlideTableRow,
    TRANSITION_EFFECTS, TextAnchor, TextBody, Transition, read_presentation, write_pptx,
};
pub use text::{ExtractedText, extract_text};
