// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! rely-office for JavaScript, through wasm-bindgen.
//!
//! The boundary is JSON text: every export takes the document as bytes and options as a JSON
//! string, and answers JSON that the hand-written wrapper of the npm package parses. A failure
//! throws an `Error` whose message starts with the stable code of the error (`TOO_LARGE: ...`),
//! which the wrapper turns into the `code` of a `RelyOfficeError`. The images of a document tree
//! cross as their own byte arrays, never inside the JSON.

use rely_office_core::{
    Block, DocumentTree, ImportReport, Limits, Media, OfficeError, Sheet, detect_format,
    extract_text, read_document_tree, read_workbook,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Why a call failed: the stable code and the message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub code: &'static str,
    pub message: String,
}

impl Failure {
    fn invalid_limits(reason: &str) -> Self {
        Self {
            code: "INVALID_LIMITS",
            message: format!("invalid limits: {reason}"),
        }
    }

    /// The message the thrown `Error` carries: the code first, so the wrapper can read it back.
    #[must_use]
    pub fn error_message(&self) -> String {
        format!("{}: {}", self.code, self.message)
    }
}

impl From<OfficeError> for Failure {
    fn from(error: OfficeError) -> Self {
        Self {
            code: error.code(),
            message: error.to_string(),
        }
    }
}

/// The limits a caller may set; a field left out keeps the library default.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[expect(
    clippy::struct_field_names,
    reason = "the fields mirror Limits and the JSON keys a caller writes"
)]
struct LimitsInput {
    max_input_bytes: Option<u64>,
    max_member_bytes: Option<u64>,
    max_text_chars: Option<usize>,
    max_rows: Option<usize>,
    max_columns: Option<usize>,
    max_image_bytes: Option<u64>,
    max_images: Option<usize>,
    max_total_image_bytes: Option<u64>,
}

/// Parse the limits JSON over the defaults; no JSON, or `null`, means the defaults.
fn parse_limits(limits_json: Option<&str>) -> Result<Limits, Failure> {
    parse_limits_over(limits_json, Limits::default())
}

/// Parse the limits JSON over `defaults`; no JSON, or `null`, means `defaults`.
///
/// # Errors
///
/// `INVALID_LIMITS` for JSON that is not an object of known limits.
pub fn parse_limits_over(limits_json: Option<&str>, defaults: Limits) -> Result<Limits, Failure> {
    let Some(json) = limits_json.map(str::trim).filter(|json| !json.is_empty()) else {
        return Ok(defaults);
    };
    let input: Option<LimitsInput> =
        serde_json::from_str(json).map_err(|error| Failure::invalid_limits(&error.to_string()))?;
    let input = input.unwrap_or_default();
    Ok(Limits {
        max_input_bytes: input.max_input_bytes.unwrap_or(defaults.max_input_bytes),
        max_member_bytes: input.max_member_bytes.unwrap_or(defaults.max_member_bytes),
        max_text_chars: input.max_text_chars.unwrap_or(defaults.max_text_chars),
        max_rows: input.max_rows.unwrap_or(defaults.max_rows),
        max_columns: input.max_columns.unwrap_or(defaults.max_columns),
        max_image_bytes: input.max_image_bytes.unwrap_or(defaults.max_image_bytes),
        max_images: input.max_images.unwrap_or(defaults.max_images),
        max_total_image_bytes: input
            .max_total_image_bytes
            .unwrap_or(defaults.max_total_image_bytes),
    })
}

fn to_json(value: &impl Serialize) -> Result<String, Failure> {
    serde_json::to_string(value).map_err(|error| Failure {
        code: "INTERNAL",
        message: error.to_string(),
    })
}

#[derive(Serialize)]
struct ExtractedTextOutput<'a> {
    format: &'static str,
    text: &'a str,
    truncated: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SheetOutput<'a> {
    name: &'a str,
    rows: &'a [Vec<String>],
    total_rows: usize,
    total_columns: usize,
}

#[derive(Serialize)]
struct WorkbookOutput<'a> {
    format: &'static str,
    sheets: Vec<SheetOutput<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MediaOutput {
    index: usize,
    content_type: &'static str,
    byte_length: usize,
}

#[derive(Serialize)]
struct DocumentTreeOutput<'a> {
    blocks: &'a [Block],
    media: Vec<MediaOutput>,
    report: &'a ImportReport,
}

/// The format of a document, read from its bytes: `docx`, `xlsx`, ... or `unknown`.
#[must_use]
pub fn detect(bytes: &[u8], file_name: Option<&str>) -> &'static str {
    detect_format(bytes, file_name).as_str()
}

/// `{ format, text, truncated }` as JSON.
///
/// # Errors
///
/// The library's error for the document, or `INVALID_LIMITS` for limits it cannot read.
pub fn extract_text_json(
    bytes: &[u8],
    file_name: Option<&str>,
    limits_json: Option<&str>,
) -> Result<String, Failure> {
    let limits = parse_limits(limits_json)?;
    let extracted = extract_text(bytes, file_name, &limits)?;
    to_json(&ExtractedTextOutput {
        format: extracted.format.as_str(),
        text: &extracted.text,
        truncated: extracted.truncated,
    })
}

/// `{ format, sheets: [{ name, rows, totalRows, totalColumns }] }` as JSON.
///
/// # Errors
///
/// The library's error for the document, or `INVALID_LIMITS` for limits it cannot read.
pub fn read_sheets_json(
    bytes: &[u8],
    file_name: Option<&str>,
    limits_json: Option<&str>,
) -> Result<String, Failure> {
    let limits = parse_limits(limits_json)?;
    let workbook = read_workbook(bytes, file_name, &limits)?;
    to_json(&WorkbookOutput {
        format: workbook.format.as_str(),
        sheets: workbook.sheets.iter().map(sheet_output).collect(),
    })
}

fn sheet_output(sheet: &Sheet) -> SheetOutput<'_> {
    SheetOutput {
        name: &sheet.name,
        rows: &sheet.rows,
        total_rows: sheet.total_rows,
        total_columns: sheet.total_columns,
    }
}

/// A document tree read for JavaScript: the JSON of its blocks, media list and report, and the
/// image bytes kept aside until the caller takes them.
#[wasm_bindgen]
#[derive(Debug)]
pub struct DocumentTreeHandle {
    json: String,
    media: Vec<Media>,
}

#[wasm_bindgen]
impl DocumentTreeHandle {
    /// `{ blocks, media: [{ index, contentType, byteLength }], report }`.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn json(&self) -> String {
        self.json.clone()
    }

    #[wasm_bindgen(js_name = mediaCount)]
    #[must_use]
    pub fn media_count(&self) -> usize {
        self.media.len()
    }

    /// The bytes of image `index`, moved out of the handle: a second call returns none.
    #[wasm_bindgen(js_name = takeMediaBytes)]
    pub fn take_media_bytes(&mut self, index: usize) -> Option<Vec<u8>> {
        self.media
            .get_mut(index)
            .map(|media| std::mem::take(&mut media.bytes))
    }
}

/// Read a docx as a [`DocumentTreeHandle`].
///
/// # Errors
///
/// The library's error for the document, or `INVALID_LIMITS` for limits it cannot read.
pub fn document_tree(
    bytes: &[u8],
    limits_json: Option<&str>,
) -> Result<DocumentTreeHandle, Failure> {
    let limits = parse_limits(limits_json)?;
    let DocumentTree {
        blocks,
        media,
        report,
        ..
    } = read_document_tree(bytes, &limits)?;
    let json = to_json(&DocumentTreeOutput {
        blocks: &blocks,
        media: media
            .iter()
            .enumerate()
            .map(|(index, item)| MediaOutput {
                index,
                content_type: item.content_type,
                byte_length: item.bytes.len(),
            })
            .collect(),
        report: &report,
    })?;
    Ok(DocumentTreeHandle { json, media })
}

fn throw(failure: &Failure) -> JsError {
    JsError::new(&failure.error_message())
}

/// The version of the library, as its crate declares it.
#[wasm_bindgen]
#[must_use]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

#[wasm_bindgen(js_name = detectFormat)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "wasm-bindgen hands an optional string over owned; it has no Option<&str>"
)]
#[must_use]
pub fn detect_format_js(bytes: &[u8], file_name: Option<String>) -> String {
    detect(bytes, file_name.as_deref()).to_owned()
}

/// # Errors
///
/// An `Error` whose message starts with the failure code.
#[wasm_bindgen(js_name = extractTextJson)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "wasm-bindgen hands an optional string over owned; it has no Option<&str>"
)]
pub fn extract_text_js(
    bytes: &[u8],
    file_name: Option<String>,
    limits_json: Option<String>,
) -> Result<String, JsError> {
    extract_text_json(bytes, file_name.as_deref(), limits_json.as_deref())
        .map_err(|failure| throw(&failure))
}

/// # Errors
///
/// An `Error` whose message starts with the failure code.
#[wasm_bindgen(js_name = readSheetsJson)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "wasm-bindgen hands an optional string over owned; it has no Option<&str>"
)]
pub fn read_sheets_js(
    bytes: &[u8],
    file_name: Option<String>,
    limits_json: Option<String>,
) -> Result<String, JsError> {
    read_sheets_json(bytes, file_name.as_deref(), limits_json.as_deref())
        .map_err(|failure| throw(&failure))
}

/// # Errors
///
/// An `Error` whose message starts with the failure code.
#[wasm_bindgen(js_name = readDocumentTreeHandle)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "wasm-bindgen hands an optional string over owned; it has no Option<&str>"
)]
pub fn read_document_tree_js(
    bytes: &[u8],
    limits_json: Option<String>,
) -> Result<DocumentTreeHandle, JsError> {
    document_tree(bytes, limits_json.as_deref()).map_err(|failure| throw(&failure))
}

#[cfg(test)]
mod tests;
