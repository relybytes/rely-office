// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! rely-office-render for JavaScript, through wasm-bindgen.
//!
//! A module of its own, apart from the reading one, so a process that only extracts text never
//! loads the typesetter. The boundary follows the reading module: options cross as JSON, the
//! summary of a render answers as JSON, the PDF and the images cross as their own byte arrays,
//! and a failure throws an `Error` whose message starts with its stable code.

use rely_office_render::{
    Fidelity, PageImages, RenderError, RenderRequest, Rendered, Renderer, render_limits,
};
use rely_office_wasm::{Failure, parse_limits_over};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// The options of a render; a field left out keeps its default.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RenderOptions {
    pdf: Option<bool>,
    thumbnail_width: Option<u32>,
    page_images: Option<PageImagesOptions>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PageImagesOptions {
    pixels_per_point: f64,
    max_pages: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RenderedSummary {
    format: &'static str,
    fidelity: &'static str,
    page_count: usize,
    truncated: bool,
    has_pdf: bool,
    has_thumbnail: bool,
    page_image_count: usize,
}

/// The failure of a render, with the codes of the reading module for what the reader refused.
fn render_failure(error: RenderError) -> Failure {
    match error {
        RenderError::Office(error) => Failure::from(error),
        RenderError::UnsupportedFormat(format) => Failure {
            code: "UNSUPPORTED_FORMAT",
            message: format!("{} cannot be rendered", format.as_str()),
        },
        RenderError::Typeset(message) => Failure {
            code: "TYPESET_FAILED",
            message,
        },
        RenderError::Export(message) => Failure {
            code: "EXPORT_FAILED",
            message,
        },
    }
}

/// Build the request of a render from its options and limits JSON.
///
/// # Errors
///
/// `INVALID_OPTIONS` for options it cannot read, `INVALID_LIMITS` for limits it cannot read.
pub fn render_request(
    options_json: Option<&str>,
    limits_json: Option<&str>,
) -> Result<RenderRequest, Failure> {
    let options: RenderOptions = match options_json.map(str::trim).filter(|json| !json.is_empty()) {
        Some(json) => serde_json::from_str::<Option<RenderOptions>>(json)
            .map_err(|error| Failure {
                code: "INVALID_OPTIONS",
                message: format!("invalid render options: {error}"),
            })?
            .unwrap_or_default(),
        None => RenderOptions::default(),
    };
    Ok(RenderRequest {
        pdf: options.pdf.unwrap_or(true),
        thumbnail_width: options.thumbnail_width,
        page_images: options.page_images.map(|images| PageImages {
            pixels_per_point: images.pixels_per_point,
            max_pages: images.max_pages,
        }),
        limits: parse_limits_over(limits_json, render_limits())?,
    })
}

/// A renderer for JavaScript: fonts are added one file at a time, and the typesetter is set up
/// on the first render after the last font.
#[wasm_bindgen(js_name = Renderer)]
#[derive(Default)]
pub struct RendererHandle {
    font_files: Vec<Vec<u8>>,
    renderer: Option<Renderer>,
}

#[wasm_bindgen(js_class = Renderer)]
impl RendererHandle {
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a font file (TrueType or OpenType).
    #[wasm_bindgen(js_name = addFont)]
    pub fn add_font(&mut self, bytes: Vec<u8>) {
        self.font_files.push(bytes);
        self.renderer = None;
    }

    /// How many font faces the renderer holds once set up.
    #[wasm_bindgen(js_name = fontCount)]
    pub fn font_count(&mut self) -> usize {
        self.renderer().font_count()
    }

    /// Render a document.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen]
    #[expect(
        clippy::needless_pass_by_value,
        reason = "wasm-bindgen hands an optional string over owned; it has no Option<&str>"
    )]
    pub fn render(
        &mut self,
        bytes: &[u8],
        file_name: Option<String>,
        options_json: Option<String>,
        limits_json: Option<String>,
    ) -> Result<RenderedHandle, JsError> {
        self.render_document(
            bytes,
            file_name.as_deref(),
            options_json.as_deref(),
            limits_json.as_deref(),
        )
        .map_err(|failure| JsError::new(&failure.error_message()))
    }
}

impl RendererHandle {
    fn renderer(&mut self) -> &Renderer {
        self.renderer
            .get_or_insert_with(|| Renderer::new(self.font_files.iter().cloned()))
    }

    /// Render a document, with the failure as a value.
    ///
    /// # Errors
    ///
    /// The failure of the options, the limits or the render.
    pub fn render_document(
        &mut self,
        bytes: &[u8],
        file_name: Option<&str>,
        options_json: Option<&str>,
        limits_json: Option<&str>,
    ) -> Result<RenderedHandle, Failure> {
        let request = render_request(options_json, limits_json)?;
        let rendered = self
            .renderer()
            .render(bytes, file_name, &request)
            .map_err(render_failure)?;
        RenderedHandle::new(rendered)
    }
}

/// What a render produced, held until JavaScript takes the bytes.
#[wasm_bindgen]
pub struct RenderedHandle {
    json: String,
    pdf: Option<Vec<u8>>,
    thumbnail: Option<Vec<u8>>,
    page_images: Vec<Vec<u8>>,
}

impl RenderedHandle {
    fn new(rendered: Rendered) -> Result<Self, Failure> {
        let summary = RenderedSummary {
            format: rendered.format.as_str(),
            fidelity: match rendered.fidelity {
                Fidelity::Layout => "layout",
                Fidelity::Text => "text",
            },
            page_count: rendered.page_count,
            truncated: rendered.truncated,
            has_pdf: rendered.pdf.is_some(),
            has_thumbnail: rendered.thumbnail.is_some(),
            page_image_count: rendered.page_images.len(),
        };
        let json = serde_json::to_string(&summary).map_err(|error| Failure {
            code: "INTERNAL",
            message: error.to_string(),
        })?;
        Ok(Self {
            json,
            pdf: rendered.pdf,
            thumbnail: rendered.thumbnail,
            page_images: rendered.page_images,
        })
    }
}

#[wasm_bindgen]
impl RenderedHandle {
    /// `{ format, fidelity, pageCount, truncated, hasPdf, hasThumbnail, pageImageCount }`.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn json(&self) -> String {
        self.json.clone()
    }

    /// The PDF, moved out of the handle: a second call returns none.
    #[wasm_bindgen(js_name = takePdf)]
    pub fn take_pdf(&mut self) -> Option<Vec<u8>> {
        self.pdf.take()
    }

    /// The thumbnail PNG, moved out of the handle.
    #[wasm_bindgen(js_name = takeThumbnail)]
    pub fn take_thumbnail(&mut self) -> Option<Vec<u8>> {
        self.thumbnail.take()
    }

    /// The PNG of page `index`, moved out of the handle.
    #[wasm_bindgen(js_name = takePageImage)]
    pub fn take_page_image(&mut self, index: usize) -> Option<Vec<u8>> {
        self.page_images
            .get_mut(index)
            .map(std::mem::take)
            .filter(|image| !image.is_empty())
    }
}

#[cfg(test)]
mod tests;
