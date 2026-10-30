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
    Block, Comment, DocumentFont, DocumentFonts, DocumentTree, EmbeddedObject, HeadersFooters,
    ImportReport, Limits, MacroProject, Media, Note, OfficeError, PageLayout, Section, Sheet,
    TrackedChanges,
    TreeOptions, detect_format, extract_text, read_document_tree, read_document_tree_named,
    read_presentation, read_workbook, sniff_image_type, write_docx, write_html, write_markdown,
    write_odt, write_pptx,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

mod presentation;
pub use presentation::{PptxWriter, PresentationHandle, pptx_from_json, presentation};

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

/// An embedded object of a document tree, its bytes kept aside like an image's.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ObjectOutput<'a> {
    index: usize,
    media: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    program: Option<&'a str>,
    name: &'a str,
    package: bool,
    byte_length: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentTreeOutput<'a> {
    blocks: &'a [Block],
    media: Vec<MediaOutput>,
    report: &'a ImportReport,
    notes: &'a [Note],
    headers_footers: &'a HeadersFooters,
    page: PageOutput,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    sections: Vec<SectionOutput<'a>>,
    #[serde(skip_serializing_if = "<[Comment]>::is_empty")]
    comments: &'a [Comment],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    objects: Vec<ObjectOutput<'a>>,
    #[serde(skip_serializing_if = "<[DocumentFont]>::is_empty")]
    fonts: &'a [DocumentFont],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    font_files: Vec<FontFileOutput>,
    #[serde(skip_serializing_if = "DefaultFont::is_empty")]
    default_font: DefaultFont,
}

/// An embedded font file of a document tree, its bytes kept aside like an image's.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FontFileOutput {
    index: usize,
    byte_length: usize,
}

/// The font and size of body text: `{ family?, sizeHalfPoints? }`.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct DefaultFont {
    #[serde(skip_serializing_if = "Option::is_none")]
    family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    size_half_points: Option<u32>,
}

impl DefaultFont {
    fn is_empty(&self) -> bool {
        self.family.is_none() && self.size_half_points.is_none()
    }
}

/// How a document is read for JavaScript: `{ trackedChanges: "accept" | "keep" }`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct TreeOptionsInput {
    tracked_changes: Option<String>,
}

/// The options of a read from their JSON; none means the defaults.
fn parse_tree_options(options_json: Option<&str>) -> Result<TreeOptions, Failure> {
    let Some(json) = options_json.map(str::trim).filter(|json| !json.is_empty()) else {
        return Ok(TreeOptions::default());
    };
    let input: TreeOptionsInput = serde_json::from_str(json).map_err(|error| Failure {
        code: "INVALID_OPTIONS",
        message: format!("invalid options: {error}"),
    })?;
    let tracked_changes = match input.tracked_changes.as_deref() {
        None | Some("accept") => TrackedChanges::Accept,
        Some("keep") => TrackedChanges::Keep,
        Some(other) => {
            return Err(Failure {
                code: "INVALID_OPTIONS",
                message: format!("trackedChanges is accept or keep, not {other}"),
            });
        }
    };
    Ok(TreeOptions { tracked_changes })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SectionOutput<'a> {
    end_block: usize,
    page: PageOutput,
    headers_footers: &'a HeadersFooters,
    continuous: bool,
}

impl PageOutput {
    fn of(layout: &PageLayout) -> Self {
        Self {
            width_twips: layout.width_twips,
            height_twips: layout.height_twips,
            margins_twips: layout.margins_twips,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[expect(
    clippy::struct_field_names,
    reason = "the fields mirror the JSON keys, which name their unit"
)]
struct PageOutput {
    width_twips: u32,
    height_twips: u32,
    /// Top, right, bottom, left.
    margins_twips: [u32; 4],
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
    objects: Vec<EmbeddedObject>,
    macro_project: Option<Vec<u8>>,
    font_files: Vec<Vec<u8>>,
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

    /// The bytes of embedded object `index`, moved out of the handle: a second call returns
    /// none.
    #[wasm_bindgen(js_name = takeObjectBytes)]
    pub fn take_object_bytes(&mut self, index: usize) -> Option<Vec<u8>> {
        self.objects
            .get_mut(index)
            .map(|object| std::mem::take(&mut object.bytes))
    }

    #[wasm_bindgen(js_name = fontFileCount)]
    #[must_use]
    pub fn font_file_count(&self) -> usize {
        self.font_files.len()
    }

    /// The bytes of embedded font file `index`, moved out of the handle: a second call returns
    /// none.
    #[wasm_bindgen(js_name = takeFontFileBytes)]
    pub fn take_font_file_bytes(&mut self, index: usize) -> Option<Vec<u8>> {
        self.font_files.get_mut(index).map(std::mem::take)
    }

    /// The bytes of image `index`, moved out of the handle: a second call returns none.
    #[wasm_bindgen(js_name = takeMediaBytes)]
    pub fn take_media_bytes(&mut self, index: usize) -> Option<Vec<u8>> {
        self.media
            .get_mut(index)
            .map(|media| std::mem::take(&mut media.bytes))
    }

    /// The macros of a docm as one opaque value to give back to the writer, moved out of the
    /// handle: a second call, or a document without macros, returns none.
    #[wasm_bindgen(js_name = takeMacroProject)]
    pub fn take_macro_project(&mut self) -> Option<Vec<u8>> {
        self.macro_project.take()
    }
}

/// Read a docx or an odt as a [`DocumentTreeHandle`].
///
/// # Errors
///
/// The library's error for the document, or `INVALID_LIMITS` for limits it cannot read.
pub fn document_tree(
    bytes: &[u8],
    limits_json: Option<&str>,
    options_json: Option<&str>,
    file_name: Option<&str>,
) -> Result<DocumentTreeHandle, Failure> {
    let limits = parse_limits(limits_json)?;
    let options = parse_tree_options(options_json)?;
    let DocumentTree {
        blocks,
        media,
        report,
        layout,
        notes,
        headers_footers,
        sections,
        comments,
        objects,
        macro_project,
        fonts,
    } = read_document_tree_named(bytes, file_name, &limits, &options)?;
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
        notes: &notes,
        headers_footers: &headers_footers,
        page: PageOutput::of(&layout),
        sections: sections
            .iter()
            .map(|section| SectionOutput {
                end_block: section.end_block,
                page: PageOutput::of(&section.layout),
                headers_footers: &section.headers_footers,
                continuous: section.continuous,
            })
            .collect(),
        comments: &comments,
        objects: objects
            .iter()
            .enumerate()
            .map(|(index, object)| ObjectOutput {
                index,
                media: object.media,
                program: object.program.as_deref(),
                name: &object.name,
                package: object.is_package(),
                byte_length: object.bytes.len(),
            })
            .collect(),
        fonts: &fonts.families,
        font_files: fonts
            .files
            .iter()
            .enumerate()
            .map(|(index, file)| FontFileOutput {
                index,
                byte_length: file.len(),
            })
            .collect(),
        default_font: DefaultFont {
            family: layout.font_family.clone(),
            size_half_points: layout.font_size_half_points,
        },
    })?;
    let macro_project = macro_project
        .map(|project| project.to_bytes())
        .transpose()?;
    Ok(DocumentTreeHandle {
        json,
        media,
        objects,
        macro_project,
        font_files: fonts.files,
    })
}

/// The page of a document to write, in twentieths of a point; A4 with 2.5 cm margins when left
/// out.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[expect(
    clippy::struct_field_names,
    reason = "the fields mirror the JSON keys, which name their unit"
)]
struct PageInput {
    width_twips: u32,
    height_twips: u32,
    /// Top, right, bottom, left.
    margins_twips: [u32; 4],
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DocxInput {
    blocks: Vec<Block>,
    #[serde(default)]
    page: Option<PageInput>,
    #[serde(default)]
    notes: Vec<Note>,
    #[serde(default)]
    headers_footers: HeadersFooters,
    #[serde(default)]
    sections: Vec<SectionInput>,
    #[serde(default)]
    comments: Vec<Comment>,
    #[serde(default)]
    fonts: Vec<DocumentFont>,
    #[serde(default)]
    default_font: Option<DefaultFont>,
}

/// One section of a document to write: where it ends, its page and its headers and footers.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SectionInput {
    end_block: usize,
    page: PageInput,
    #[serde(default)]
    headers_footers: HeadersFooters,
    #[serde(default)]
    continuous: bool,
}

/// A page layout from its JSON, within the sizes Word accepts.
fn layout_of(page: &PageInput) -> PageLayout {
    PageLayout {
        width_twips: page.width_twips.clamp(1_440, 31_680),
        height_twips: page.height_twips.clamp(1_440, 31_680),
        margins_twips: page.margins_twips.map(|margin| margin.min(15_840)),
        ..PageLayout::default()
    }
}

/// Write a docx from the JSON of a document tree, `{ blocks, page?, notes?, headersFooters? }`,
/// with the images the tree names by index and the objects those images show.
///
/// # Errors
///
/// `INVALID_TREE` for JSON that is not a document tree, or the error of the writer.
pub fn docx_from_json(
    tree_json: &str,
    media: Vec<Media>,
    objects: Vec<EmbeddedObject>,
) -> Result<Vec<u8>, Failure> {
    Ok(write_docx(&tree_from_json(tree_json, media, objects)?)?)
}

/// The document tree of its JSON, with the images it names by index and their objects.
///
/// # Errors
///
/// `INVALID_TREE` for JSON that is not a document tree.
pub fn tree_from_json(
    tree_json: &str,
    media: Vec<Media>,
    objects: Vec<EmbeddedObject>,
) -> Result<DocumentTree, Failure> {
    let input: DocxInput = serde_json::from_str(tree_json).map_err(|error| Failure {
        code: "INVALID_TREE",
        message: format!("invalid document tree: {error}"),
    })?;
    let mut layout = input
        .page
        .as_ref()
        .map_or_else(PageLayout::default, layout_of);
    if let Some(font) = input.default_font {
        layout.font_family = font
            .family
            .map(|family| family.trim().to_owned())
            .filter(|family| !family.is_empty());
        layout.font_size_half_points = font
            .size_half_points
            .filter(|size| (2..=3_276).contains(size));
    }
    let blocks = input.blocks.len();
    let sections = input
        .sections
        .into_iter()
        .map(|section| Section {
            end_block: section.end_block.min(blocks),
            layout: layout_of(&section.page),
            headers_footers: section.headers_footers,
            continuous: section.continuous,
        })
        .collect();
    Ok(DocumentTree {
        blocks: input.blocks,
        media,
        report: ImportReport::default(),
        layout,
        notes: input.notes,
        headers_footers: input.headers_footers,
        sections,
        comments: input.comments,
        objects,
        macro_project: None,
        fonts: DocumentFonts {
            families: input
                .fonts
                .into_iter()
                .take(rely_office_core::MAX_FONTS)
                .collect(),
            files: Vec::new(),
        },
    })
}

/// Writes a docx for JavaScript: images are added one by one, then the tree is written.
#[wasm_bindgen]
#[derive(Debug, Default)]
pub struct DocxWriter {
    media: Vec<Media>,
    objects: Vec<EmbeddedObject>,
    macro_project: Option<MacroProject>,
    font_files: Vec<Vec<u8>>,
}

#[wasm_bindgen]
impl DocxWriter {
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an image and return the index the tree names it by; its type is read from the bytes.
    #[wasm_bindgen(js_name = addMedia)]
    pub fn add_media(&mut self, bytes: Vec<u8>) -> usize {
        self.media.push(Media {
            content_type: sniff_image_type(&bytes),
            bytes,
        });
        self.media.len() - 1
    }

    /// Add a font file the tree's `fonts` embed and return the index their faces name it by;
    /// a file that is not a font, or one past the limits, is refused with `None`.
    #[wasm_bindgen(js_name = addFontFile)]
    pub fn add_font_file(&mut self, bytes: Vec<u8>) -> Option<usize> {
        let total: u64 = self.font_files.iter().map(|file| file.len() as u64).sum();
        if !rely_office_core::is_font_file(&bytes)
            || self.font_files.len() >= rely_office_core::MAX_FONT_FILES
            || bytes.len() as u64 > rely_office_core::MAX_FONT_FILE_BYTES
            || total + bytes.len() as u64 > rely_office_core::MAX_TOTAL_FONT_BYTES
        {
            return None;
        }
        self.font_files.push(bytes);
        Some(self.font_files.len() - 1)
    }

    /// The tree of its JSON with the images, objects and font files added so far.
    fn take_tree(&mut self, tree_json: &str, objects: bool) -> Result<DocumentTree, Failure> {
        let mut tree = tree_from_json(
            tree_json,
            std::mem::take(&mut self.media),
            if objects {
                std::mem::take(&mut self.objects)
            } else {
                Vec::new()
            },
        )?;
        tree.fonts.files = std::mem::take(&mut self.font_files);
        Ok(tree)
    }

    /// Add an object embedded in the document, shown by image `media`, and return its index;
    /// `program` is its `ProgID` when known.
    #[wasm_bindgen(js_name = addObject)]
    pub fn add_object(
        &mut self,
        media: usize,
        bytes: Vec<u8>,
        name: String,
        program: Option<String>,
    ) -> usize {
        self.objects.push(EmbeddedObject {
            media,
            program,
            name,
            bytes,
        });
        self.objects.len() - 1
    }

    /// Write the docx.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen]
    pub fn write(&mut self, tree_json: &str) -> Result<Vec<u8>, JsError> {
        let mut tree = self
            .take_tree(tree_json, true)
            .map_err(|failure| throw(&failure))?;
        tree.macro_project = self.macro_project.take();
        write_docx(&tree).map_err(|error| throw(&error.into()))
    }

    /// Give the docx the macros `takeMacroProject` gave, so it is written as a docm.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code, for bytes that are no macro
    /// project.
    #[wasm_bindgen(js_name = setMacroProject)]
    pub fn set_macro_project(&mut self, bytes: &[u8]) -> Result<(), JsError> {
        self.macro_project = Some(
            MacroProject::from_bytes(bytes, &Limits::default())
                .map_err(|error| throw(&error.into()))?,
        );
        Ok(())
    }

    /// Write the tree as an odt, `OpenDocument` text.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = writeOdt)]
    pub fn write_odt_js(&mut self, tree_json: &str) -> Result<Vec<u8>, JsError> {
        let tree = self
            .take_tree(tree_json, false)
            .map_err(|failure| throw(&failure))?;
        write_odt(&tree).map_err(|error| throw(&error.into()))
    }

    /// Write the tree as Markdown, its images in `data:` addresses.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = writeMarkdown)]
    pub fn write_markdown_js(&mut self, tree_json: &str) -> Result<String, JsError> {
        let tree = self
            .take_tree(tree_json, false)
            .map_err(|failure| throw(&failure))?;
        Ok(write_markdown(&tree))
    }

    /// Write the tree as a page of HTML that stands alone, titled `title`.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = writeHtml)]
    pub fn write_html_js(&mut self, tree_json: &str, title: &str) -> Result<String, JsError> {
        let tree = self
            .take_tree(tree_json, false)
            .map_err(|failure| throw(&failure))?;
        Ok(write_html(&tree, title))
    }
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

/// The kind a font family is, from the name JavaScript gives it; anything else is `auto`.
fn font_generic_of(generic: Option<&str>) -> rely_office_core::FontGeneric {
    use rely_office_core::FontGeneric;
    match generic {
        Some("roman") => FontGeneric::Roman,
        Some("swiss") => FontGeneric::Swiss,
        Some("modern") => FontGeneric::Modern,
        Some("script") => FontGeneric::Script,
        Some("decorative") => FontGeneric::Decorative,
        _ => FontGeneric::Auto,
    }
}

/// A CSS font stack for a family: the family, its bundled metric-compatible substitute, then
/// the generic family; `generic` is `roman`, `swiss`, `modern`, `script` or `decorative`.
#[wasm_bindgen(js_name = fontStack)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "wasm-bindgen hands an optional string over owned; it has no Option<&str>"
)]
#[must_use]
pub fn font_stack_js(family: &str, generic: Option<String>) -> String {
    rely_office_core::css_font_stack(family, font_generic_of(generic.as_deref()))
}

/// The bundled family of the same metrics as `family`, or the nearest of its kind.
#[wasm_bindgen(js_name = fontSubstitute)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "wasm-bindgen hands an optional string over owned; it has no Option<&str>"
)]
#[must_use]
pub fn font_substitute_js(family: &str, generic: Option<String>) -> String {
    rely_office_core::font_substitute(family, font_generic_of(generic.as_deref())).to_owned()
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
    options_json: Option<String>,
    file_name: Option<String>,
) -> Result<DocumentTreeHandle, JsError> {
    document_tree(
        bytes,
        limits_json.as_deref(),
        options_json.as_deref(),
        file_name.as_deref(),
    )
    .map_err(|failure| throw(&failure))
}

/// A document as a docx: a doc, an odt or a docx read into its tree and written back, so an
/// old file can be kept in the format an editor saves.
///
/// # Errors
///
/// The failure of reading the document, or of writing the docx.
pub fn convert_to_docx(bytes: &[u8], limits_json: Option<&str>) -> Result<Vec<u8>, Failure> {
    let limits = parse_limits(limits_json)?;
    Ok(write_docx(&read_document_tree(bytes, &limits)?)?)
}

/// A presentation as a pptx: a ppt, an odp or a pptx read into its slides and written back.
///
/// # Errors
///
/// The failure of reading the presentation, or of writing the pptx.
pub fn convert_to_pptx(bytes: &[u8], limits_json: Option<&str>) -> Result<Vec<u8>, Failure> {
    let limits = parse_limits(limits_json)?;
    Ok(write_pptx(&read_presentation(bytes, &limits)?)?)
}

/// # Errors
///
/// An `Error` whose message starts with the failure code.
#[wasm_bindgen(js_name = convertToDocx)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "wasm-bindgen hands an optional string over owned; it has no Option<&str>"
)]
pub fn convert_to_docx_js(bytes: &[u8], limits_json: Option<String>) -> Result<Vec<u8>, JsError> {
    convert_to_docx(bytes, limits_json.as_deref()).map_err(|failure| throw(&failure))
}

/// # Errors
///
/// An `Error` whose message starts with the failure code.
#[wasm_bindgen(js_name = convertToPptx)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "wasm-bindgen hands an optional string over owned; it has no Option<&str>"
)]
pub fn convert_to_pptx_js(bytes: &[u8], limits_json: Option<String>) -> Result<Vec<u8>, JsError> {
    convert_to_pptx(bytes, limits_json.as_deref()).map_err(|failure| throw(&failure))
}

#[cfg(test)]
mod tests;
