// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Presentations for JavaScript: a pptx, an odp or a ppt read into its slides as JSON with its
//! images beside it, and slides written back as a pptx, so an editor changes a presentation by
//! changing the JSON.

use rely_office_core::{
    Media, Presentation, Slide, read_presentation, sniff_image_type, write_pptx,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use super::{Failure, MediaOutput, parse_limits, throw, to_json};

/// The most slides a written presentation holds.
const MAX_SLIDES: usize = 2_000;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PresentationOutput<'a> {
    width_emu: i64,
    height_emu: i64,
    slides: &'a [Slide],
    media: Vec<MediaOutput>,
    truncated: bool,
}

/// A presentation read for JavaScript: its slides as JSON, and the bytes of its images, which
/// stay here until they are taken.
#[wasm_bindgen]
#[derive(Debug)]
pub struct PresentationHandle {
    json: String,
    media: Vec<Media>,
}

#[wasm_bindgen]
impl PresentationHandle {
    /// `{ widthEmu, heightEmu, slides, media: [{ index, contentType, byteLength }], truncated }`.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn json(&self) -> String {
        self.json.clone()
    }

    /// The bytes of image `index`, moved out of the handle: a second call returns none.
    #[wasm_bindgen(js_name = takeMediaBytes)]
    pub fn take_media_bytes(&mut self, index: usize) -> Option<Vec<u8>> {
        self.media
            .get_mut(index)
            .map(|media| std::mem::take(&mut media.bytes))
    }
}

/// Read a pptx, an odp or a ppt as a [`PresentationHandle`].
///
/// # Errors
///
/// The library's error for the file, or `INVALID_LIMITS` for limits it cannot read.
pub fn presentation(
    bytes: &[u8],
    limits_json: Option<&str>,
) -> Result<PresentationHandle, Failure> {
    let limits = parse_limits(limits_json)?;
    let Presentation {
        width_emu,
        height_emu,
        slides,
        media,
        truncated,
    } = read_presentation(bytes, &limits)?;
    let json = to_json(&PresentationOutput {
        width_emu,
        height_emu,
        slides: &slides,
        media: media
            .iter()
            .enumerate()
            .map(|(index, item)| MediaOutput {
                index,
                content_type: item.content_type,
                byte_length: item.bytes.len(),
            })
            .collect(),
        truncated,
    })?;
    Ok(PresentationHandle { json, media })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PresentationInput {
    width_emu: i64,
    height_emu: i64,
    slides: Vec<Slide>,
}

/// Write a pptx from the JSON of a presentation, `{ widthEmu, heightEmu, slides }`, with the
/// images its shapes and fills name by index.
///
/// # Errors
///
/// `INVALID_PRESENTATION` for JSON that is not a presentation or holds too many slides, or the
/// error of the writer.
pub fn pptx_from_json(presentation_json: &str, media: Vec<Media>) -> Result<Vec<u8>, Failure> {
    let input: PresentationInput =
        serde_json::from_str(presentation_json).map_err(|error| Failure {
            code: "INVALID_PRESENTATION",
            message: format!("invalid presentation: {error}"),
        })?;
    if input.slides.len() > MAX_SLIDES {
        return Err(Failure {
            code: "INVALID_PRESENTATION",
            message: format!("a presentation holds at most {MAX_SLIDES} slides"),
        });
    }
    Ok(write_pptx(&Presentation {
        width_emu: input.width_emu,
        height_emu: input.height_emu,
        slides: input.slides,
        media,
        truncated: false,
    })?)
}

/// # Errors
///
/// An `Error` whose message starts with the failure code.
#[wasm_bindgen(js_name = readPresentationHandle)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "wasm-bindgen hands an optional string over owned; it has no Option<&str>"
)]
pub fn read_presentation_js(
    bytes: &[u8],
    limits_json: Option<String>,
) -> Result<PresentationHandle, JsError> {
    presentation(bytes, limits_json.as_deref()).map_err(|failure| throw(&failure))
}

/// Writes a pptx for JavaScript: images are added one by one, then the slides are written.
#[wasm_bindgen]
#[derive(Debug, Default)]
pub struct PptxWriter {
    media: Vec<Media>,
}

#[wasm_bindgen]
impl PptxWriter {
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an image and return the index the slides name it by; its type is read from the
    /// bytes.
    #[wasm_bindgen(js_name = addMedia)]
    pub fn add_media(&mut self, bytes: Vec<u8>) -> usize {
        self.media.push(Media {
            content_type: sniff_image_type(&bytes),
            bytes,
        });
        self.media.len() - 1
    }

    /// Write the pptx.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen]
    pub fn write(&mut self, presentation_json: &str) -> Result<Vec<u8>, JsError> {
        pptx_from_json(presentation_json, std::mem::take(&mut self.media))
            .map_err(|failure| throw(&failure))
    }
}

#[cfg(test)]
mod tests;
