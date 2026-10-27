// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Renders office documents to PDF and PNG through Typst, without an office suite.
//!
//! A document is read with `rely-office-core`, written out as Typst source and typeset with the
//! fonts the caller hands in: nothing is read from the file system or the network, so the same
//! input and fonts give the same pages everywhere, WebAssembly included.
//!
//! - docx and odt are laid out from their document tree, with the page size, margins and body
//!   font of the document ([`Fidelity::Layout`]);
//! - xlsx, xls, ods, csv and tsv are laid out as tables, one sheet after the other, the columns
//!   split across pages when they do not fit ([`Fidelity::Layout`]);
//! - pptx and odp are drawn slide by slide, one page each at the slide's size, its shapes placed
//!   where the slide, its layout and its master put them ([`Fidelity::Layout`]);
//! - doc and ppt are set as their text, on A4 ([`Fidelity::Text`]).

mod docx;
mod fonts;
mod markup;
mod metrics;
mod plain;
mod sheet;
mod slides;
mod world;

use rely_office_core::{
    DocumentFormat, Limits, NumberLocale, OfficeError, detect_format, extract_text,
    read_document_tree, read_presentation, read_workbook_in,
};
use typst::LibraryExt;
use typst::diag::SourceDiagnostic;
use typst::foundations::Bytes;
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, compile};
use typst_layout::PagedDocument;

use metrics::FontLines;

pub use fonts::bundled_font_families;

/// The widest thumbnail made, in pixels.
pub const MAX_THUMBNAIL_WIDTH: u32 = 2_000;
/// The highest resolution of a page image, in pixels per point (8 is 576 dpi).
pub const MAX_PIXELS_PER_POINT: f64 = 8.0;

/// How close the pages are to what an office suite would print.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fidelity {
    /// The document's own structure and page geometry, typeset by Typst.
    Layout,
    /// The document's text only, set on A4: the format has no layout reader yet.
    Text,
}

/// What to produce from one typesetting run.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderRequest {
    pub pdf: bool,
    /// A PNG of the first page this many pixels wide, at most [`MAX_THUMBNAIL_WIDTH`].
    pub thumbnail_width: Option<u32>,
    /// A PNG of each page, for comparisons and page-by-page viewers.
    pub page_images: Option<PageImages>,
    pub limits: Limits,
}

impl Default for RenderRequest {
    /// A PDF, with the reading limits lowered so one render stays within a request: 200,000
    /// characters of text, 1,000 rows and 60 columns per sheet.
    fn default() -> Self {
        Self {
            pdf: true,
            thumbnail_width: None,
            page_images: None,
            limits: render_limits(),
        }
    }
}

/// The reading limits a render uses by default.
#[must_use]
pub fn render_limits() -> Limits {
    Limits {
        max_text_chars: 200_000,
        ..Limits::default()
    }
}

/// Page images of the first `max_pages` pages at `pixels_per_point`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageImages {
    pub pixels_per_point: f64,
    pub max_pages: usize,
}

/// What a render produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub format: DocumentFormat,
    pub fidelity: Fidelity,
    pub page_count: usize,
    pub pdf: Option<Vec<u8>>,
    /// PNG bytes.
    pub thumbnail: Option<Vec<u8>>,
    /// PNG bytes, one per page.
    pub page_images: Vec<Vec<u8>>,
    /// Whether the document held more than the limits let through, so the pages show a part.
    pub truncated: bool,
}

/// Why a document could not be rendered.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error(transparent)]
    Office(#[from] OfficeError),
    #[error("{0:?} cannot be rendered")]
    UnsupportedFormat(DocumentFormat),
    /// Typst refused the generated source: a bug of this crate, never of the input.
    #[error("typesetting failed: {0}")]
    Typeset(String),
    #[error("export failed: {0}")]
    Export(String),
}

/// The Typst source of a document, with the files it refers to (the images of a docx).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypstSource {
    pub format: DocumentFormat,
    pub fidelity: Fidelity,
    pub source: String,
    /// `(path, bytes)`, paths absolute inside the project root, as the source names them.
    pub files: Vec<(String, Vec<u8>)>,
    pub truncated: bool,
}

/// Write a document out as Typst source, with line heights for fonts of unknown metrics;
/// [`Renderer::typst_source`] uses the metrics of the renderer's fonts.
///
/// # Errors
///
/// [`RenderError::Office`] when the document cannot be read, [`RenderError::UnsupportedFormat`]
/// for a format with no text to set.
pub fn typst_source(
    bytes: &[u8],
    file_name: Option<&str>,
    limits: &Limits,
) -> Result<TypstSource, RenderError> {
    source_with(bytes, file_name, limits, true, &FontLines::default())
}

fn source_with(
    bytes: &[u8],
    file_name: Option<&str>,
    limits: &Limits,
    with_images: bool,
    lines: &FontLines,
) -> Result<TypstSource, RenderError> {
    let format = detect_format(bytes, file_name);
    match format {
        DocumentFormat::Docx | DocumentFormat::Odt | DocumentFormat::Doc => {
            let tree = read_document_tree(bytes, limits)?;
            let truncated = tree.report.truncated;
            let (source, files) = docx::document_source(&tree, with_images, lines);
            Ok(TypstSource {
                format,
                fidelity: Fidelity::Layout,
                source,
                files,
                truncated,
            })
        }
        DocumentFormat::Xlsx
        | DocumentFormat::Xls
        | DocumentFormat::Ods
        | DocumentFormat::Csv
        | DocumentFormat::Tsv => {
            // Sheets show their numbers and dates in Italian, the language the pages are set in.
            let workbook = read_workbook_in(bytes, file_name, limits, NumberLocale::Italian)?;
            let (source, files) = sheet::workbook_source(&workbook, with_images);
            let truncated = workbook.sheets.iter().any(|sheet| {
                sheet.total_rows > sheet.rows.len()
                    || sheet.total_columns > sheet.rows.first().map_or(0, Vec::len)
            });
            Ok(TypstSource {
                format,
                fidelity: Fidelity::Layout,
                source,
                files,
                truncated,
            })
        }
        DocumentFormat::Pptx | DocumentFormat::Odp => {
            let presentation = read_presentation(bytes, limits)?;
            let truncated = presentation.truncated;
            let (source, files) = slides::presentation_source(&presentation, with_images, lines);
            Ok(TypstSource {
                format,
                fidelity: Fidelity::Layout,
                source,
                files,
                truncated,
            })
        }
        DocumentFormat::Ppt => {
            let extracted = extract_text(bytes, file_name, limits)?;
            Ok(TypstSource {
                format,
                fidelity: Fidelity::Text,
                source: plain::text_source(&extracted.text),
                files: Vec::new(),
                truncated: extracted.truncated,
            })
        }
        DocumentFormat::Unknown => Err(RenderError::UnsupportedFormat(format)),
    }
}

/// Typesets documents with a fixed set of fonts; build it once and reuse it, since reading the
/// fonts is the slow part of setting up.
pub struct Renderer {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    lines: FontLines,
}

impl Renderer {
    /// A renderer with these font files (TrueType or OpenType, collections included); files that
    /// are not fonts are skipped.
    pub fn new<I>(font_files: I) -> Self
    where
        I: IntoIterator<Item = Vec<u8>>,
    {
        let fonts: Vec<Font> = font_files
            .into_iter()
            .flat_map(|data| Font::iter(Bytes::new(data)))
            .collect();
        Self {
            library: LazyHash::new(Library::default()),
            book: LazyHash::new(FontBook::from_fonts(&fonts)),
            lines: FontLines::from_fonts(&fonts),
            fonts,
        }
    }

    /// How many font faces the renderer holds.
    #[must_use]
    pub fn font_count(&self) -> usize {
        self.fonts.len()
    }

    /// Render a document.
    ///
    /// # Errors
    ///
    /// Those of [`typst_source`], [`RenderError::Typeset`] when Typst refuses the source and
    /// [`RenderError::Export`] when the PDF cannot be written.
    pub fn render(
        &self,
        bytes: &[u8],
        file_name: Option<&str>,
        request: &RenderRequest,
    ) -> Result<Rendered, RenderError> {
        let source = self.typst_source(bytes, file_name, &request.limits)?;
        match self.render_source(&source, request) {
            // An image Typst cannot decode fails the whole document: draw the images as frames.
            Err(RenderError::Typeset(_)) if !source.files.is_empty() => {
                let framed = source_with(bytes, file_name, &request.limits, false, &self.lines)?;
                self.render_source(&framed, request)
            }
            rendered => rendered,
        }
    }

    /// Write a document out as Typst source, with the line heights of the renderer's fonts.
    ///
    /// # Errors
    ///
    /// Those of [`typst_source`].
    pub fn typst_source(
        &self,
        bytes: &[u8],
        file_name: Option<&str>,
        limits: &Limits,
    ) -> Result<TypstSource, RenderError> {
        source_with(bytes, file_name, limits, true, &self.lines)
    }

    /// Typeset Typst source written by [`typst_source`].
    ///
    /// # Errors
    ///
    /// [`RenderError::Typeset`] when Typst refuses the source, [`RenderError::Export`] when the
    /// PDF cannot be written.
    pub fn render_source(
        &self,
        source: &TypstSource,
        request: &RenderRequest,
    ) -> Result<Rendered, RenderError> {
        let world = world::OfficeWorld::new(
            &self.library,
            &self.book,
            &self.fonts,
            &source.source,
            &source.files,
        );
        let document = compile::<PagedDocument>(&world)
            .output
            .map_err(|errors| RenderError::Typeset(describe(&errors)));
        // Typst memoizes across runs; a long-lived process renders unrelated documents, so
        // nothing is worth keeping.
        comemo::evict(0);
        let document = document?;

        let pdf = if request.pdf {
            Some(
                typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default())
                    .map_err(|errors| RenderError::Export(describe(&errors)))?,
            )
        } else {
            None
        };
        let pages = document.pages();
        let thumbnail = match (request.thumbnail_width, pages.first()) {
            (Some(width), Some(page)) => {
                let width = width.clamp(1, MAX_THUMBNAIL_WIDTH);
                let pixels_per_point = f64::from(width) / page.frame.width().to_pt();
                Some(page_png(page, pixels_per_point)?)
            }
            _ => None,
        };
        let page_images = match request.page_images {
            Some(images) => pages
                .iter()
                .take(images.max_pages)
                .map(|page| {
                    page_png(
                        page,
                        images.pixels_per_point.clamp(0.1, MAX_PIXELS_PER_POINT),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };

        Ok(Rendered {
            format: source.format,
            fidelity: source.fidelity,
            page_count: pages.len(),
            pdf,
            thumbnail,
            page_images,
            truncated: source.truncated,
        })
    }
}

fn page_png(page: &typst_layout::Page, pixels_per_point: f64) -> Result<Vec<u8>, RenderError> {
    let pixmap = typst_render::render(
        page,
        &typst_render::RenderOptions {
            pixel_per_pt: pixels_per_point.into(),
            render_bleed: false,
        },
    );
    pixmap
        .encode_png()
        .map_err(|error| RenderError::Export(error.to_string()))
}

fn describe(diagnostics: &[SourceDiagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests;
