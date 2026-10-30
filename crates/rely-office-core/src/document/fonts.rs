// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! The fonts of a document: the families it names, what kind each is, and the faces it carries
//! inside it, read from a docx font table or the font face declarations of an odt and written
//! back the same way.

use std::fmt::Write as _;

use crate::archive::{ZipPackage, read_member_bytes, read_text_member};
use crate::relationships::read_relationships;
use crate::xml_tree::{Element, Namespace, parse_tree};
use crate::{Block, DocumentTree, HeadersFooters, ImportReport, Inline, Limits, Mark};

/// The most families a document lists.
pub const MAX_FONTS: usize = 256;
/// The most embedded font files a document keeps.
pub const MAX_FONT_FILES: usize = 32;
/// The largest embedded font file kept, in bytes; a larger one is reported, not kept.
pub const MAX_FONT_FILE_BYTES: u64 = 16 * 1024 * 1024;
/// The most bytes of all the embedded font files together.
pub const MAX_TOTAL_FONT_BYTES: u64 = 48 * 1024 * 1024;

/// The kind of a typeface, as Word's font table and `OpenDocument` name it, for a reader that
/// does not hold the face to pick the nearest it has.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub enum FontGeneric {
    #[default]
    Auto,
    /// Serif: Times New Roman, Cambria.
    Roman,
    /// Sans serif: Arial, Calibri.
    Swiss,
    /// Monospace: Courier New, Consolas.
    Modern,
    Script,
    Decorative,
}

impl FontGeneric {
    /// The name Word's font table and `OpenDocument`'s `style:font-family-generic` give it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Roman => "roman",
            Self::Swiss => "swiss",
            Self::Modern => "modern",
            Self::Script => "script",
            Self::Decorative => "decorative",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "roman" => Self::Roman,
            "swiss" => Self::Swiss,
            "modern" => Self::Modern,
            "script" => Self::Script,
            "decorative" => Self::Decorative,
            _ => Self::Auto,
        }
    }

    /// The CSS generic family a stack ends with.
    #[must_use]
    pub fn css(self) -> &'static str {
        match self {
            Self::Roman => "serif",
            Self::Modern => "monospace",
            Self::Script => "cursive",
            Self::Decorative => "fantasy",
            Self::Auto | Self::Swiss => "sans-serif",
        }
    }
}

/// Whether every character of a typeface is as wide as the others.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub enum FontPitch {
    #[default]
    Default,
    Fixed,
    Variable,
}

impl FontPitch {
    fn parse(value: &str) -> Self {
        match value {
            "fixed" => Self::Fixed,
            "variable" => Self::Variable,
            _ => Self::Default,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Fixed => "fixed",
            Self::Variable => "variable",
        }
    }
}

/// Which face of a family an embedded font file is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub enum FaceStyle {
    #[default]
    Regular,
    Bold,
    Italic,
    BoldItalic,
}

impl FaceStyle {
    /// Whether the face is bold.
    #[must_use]
    pub fn bold(self) -> bool {
        matches!(self, Self::Bold | Self::BoldItalic)
    }

    /// Whether the face is italic.
    #[must_use]
    pub fn italic(self) -> bool {
        matches!(self, Self::Italic | Self::BoldItalic)
    }

    fn of(bold: bool, italic: bool) -> Self {
        match (bold, italic) {
            (false, false) => Self::Regular,
            (true, false) => Self::Bold,
            (false, true) => Self::Italic,
            (true, true) => Self::BoldItalic,
        }
    }

    /// The suffix of Word's `w:embed*` element for this face.
    fn word_element(self) -> &'static str {
        match self {
            Self::Regular => "embedRegular",
            Self::Bold => "embedBold",
            Self::Italic => "embedItalic",
            Self::BoldItalic => "embedBoldItalic",
        }
    }
}

/// A face of a family the document carries, its bytes in [`DocumentFonts::files`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct FontFace {
    pub style: FaceStyle,
    /// The index of its bytes in [`DocumentFonts::files`].
    pub file: usize,
}

/// A family the document names, with what the file says of it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase", default)
)]
pub struct DocumentFont {
    pub name: String,
    /// Another name the family goes by, which a reader may hold it under.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub alt_name: Option<String>,
    pub generic: FontGeneric,
    pub pitch: FontPitch,
    /// Word's character set, two hexadecimal digits.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub charset: Option<String>,
    /// The PANOSE classification, twenty hexadecimal digits.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub panose: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Vec::is_empty"))]
    pub faces: Vec<FontFace>,
}

/// The families a document names and the font files it carries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocumentFonts {
    pub families: Vec<DocumentFont>,
    /// The embedded font files, plain TrueType or OpenType, named by [`FontFace::file`].
    pub files: Vec<Vec<u8>>,
}

impl DocumentFonts {
    /// The family of this name, compared without case.
    #[must_use]
    pub fn family(&self, name: &str) -> Option<&DocumentFont> {
        let name = name.trim();
        self.families
            .iter()
            .find(|family| family.name.eq_ignore_ascii_case(name))
    }

    /// Whether the document carries the faces of no family.
    #[must_use]
    pub fn has_files(&self) -> bool {
        !self.files.is_empty()
    }

    /// Keep a font file if it fits the limits; its index, or `None` with the report told.
    pub(crate) fn keep_file(&mut self, bytes: Vec<u8>, report: &mut ImportReport) -> Option<usize> {
        let total: u64 = self.files.iter().map(|file| file.len() as u64).sum();
        if !is_font_file(&bytes)
            || self.files.len() >= MAX_FONT_FILES
            || total + bytes.len() as u64 > MAX_TOTAL_FONT_BYTES
        {
            report.fonts_left_out += 1;
            return None;
        }
        self.files.push(bytes);
        Some(self.files.len() - 1)
    }
}

/// Whether bytes start like a TrueType, OpenType or collection font file.
#[must_use]
pub fn is_font_file(bytes: &[u8]) -> bool {
    matches!(
        bytes.get(..4),
        Some([0, 1, 0, 0] | b"OTTO" | b"true" | b"ttcf")
    )
}

/// Whether a font file says its face is bold and italic, from the `macStyle` of its `head`
/// table; `None` when it has no readable one.
#[must_use]
pub fn face_style_of(bytes: &[u8]) -> Option<FaceStyle> {
    let tables = usize::from(u16::from_be_bytes(bytes.get(4..6)?.try_into().ok()?));
    for index in 0..tables.min(64) {
        let entry = bytes.get(12 + index * 16..28 + index * 16)?;
        if &entry[..4] == b"head" {
            let offset = usize::try_from(u32::from_be_bytes(entry[8..12].try_into().ok()?)).ok()?;
            let style = u16::from_be_bytes(bytes.get(offset + 44..offset + 46)?.try_into().ok()?);
            return Some(FaceStyle::of(style & 1 != 0, style & 2 != 0));
        }
    }
    None
}

/// The 16 bytes of a font key GUID in the order the obfuscation uses them: the hexadecimal
/// digits read from the last pair to the first, as ECMA-376 Part 2 describes.
fn font_key(guid: &str) -> Option<[u8; 16]> {
    let digits: String = guid.chars().filter(char::is_ascii_hexdigit).collect();
    if digits.len() != 32 {
        return None;
    }
    let mut key = [0u8; 16];
    for (index, slot) in key.iter_mut().enumerate() {
        let start = 30 - index * 2;
        *slot = u8::from_str_radix(&digits[start..start + 2], 16).ok()?;
    }
    Some(key)
}

/// Obfuscate or de-obfuscate a font file in place with its key: the first 32 bytes are combined
/// by exclusive or with the key, which is its own inverse. False when the file is too short.
///
/// The key is the GUID of `w:fontKey`, as ECMA-376 Part 2 obfuscates fonts.
fn xor_font(bytes: &mut [u8], key: &[u8; 16]) -> bool {
    if bytes.len() < 32 {
        return false;
    }
    for (index, byte) in bytes.iter_mut().take(32).enumerate() {
        *byte ^= key[index % 16];
    }
    true
}

/// A GUID for a font key, taken from the font's bytes so the same file writes the same package.
fn key_guid_for(bytes: &[u8]) -> String {
    let mut first: u64 = 0xcbf2_9ce4_8422_2325;
    let mut second: u64 = 0x8422_2325_cbf2_9ce4;
    for &byte in bytes {
        first = (first ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        second = (second ^ u64::from(byte.rotate_left(3))).wrapping_mul(0x0000_0100_0000_01b3);
    }
    let hex = format!("{first:016X}{second:016X}");
    format!(
        "{{{}-{}-{}-{}-{}}}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

const W: Namespace = Namespace::Word;

fn value(element: &Element, name: &str) -> Option<String> {
    element
        .child(W, name)
        .and_then(|child| child.attribute(W, "val"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// The fonts of a docx: its font table part, with the faces embedded in it de-obfuscated. A
/// face that cannot be read is counted in the report and left out; nothing here is fatal.
pub(crate) fn read_docx_fonts(
    package: &mut ZipPackage<'_>,
    font_table: Option<&str>,
    limits: &Limits,
    report: &mut ImportReport,
) -> DocumentFonts {
    let mut fonts = DocumentFonts::default();
    let Some(part) = font_table else {
        return fonts;
    };
    let Some(xml) = read_text_member(package, part, limits).ok().flatten() else {
        return fonts;
    };
    let Ok(root) = parse_tree(&xml, |_, _| false) else {
        return fonts;
    };
    let relationships = read_relationships(package, part, limits).unwrap_or_default();
    for font in root.elements().filter(|element| element.is(W, "font")) {
        if fonts.families.len() >= MAX_FONTS {
            break;
        }
        let Some(name) = font
            .attribute(W, "name")
            .map(str::trim)
            .filter(|name| !name.is_empty())
        else {
            continue;
        };
        let mut family = DocumentFont {
            name: name.to_owned(),
            alt_name: value(font, "altName"),
            generic: value(font, "family")
                .map_or(FontGeneric::Auto, |value| FontGeneric::parse(&value)),
            pitch: value(font, "pitch")
                .map_or(FontPitch::Default, |value| FontPitch::parse(&value)),
            charset: value(font, "charset"),
            panose: value(font, "panose1"),
            faces: Vec::new(),
        };
        for style in [
            FaceStyle::Regular,
            FaceStyle::Bold,
            FaceStyle::Italic,
            FaceStyle::BoldItalic,
        ] {
            let Some(embed) = font.child(W, style.word_element()) else {
                continue;
            };
            let target = embed
                .attribute(Namespace::Relationships, "id")
                .and_then(|id| {
                    relationships
                        .iter()
                        .find(|relationship| relationship.id == id)
                })
                .filter(|relationship| !relationship.external)
                .map(|relationship| relationship.target.clone());
            let key = embed.attribute(W, "fontKey").and_then(font_key);
            let bytes = target.and_then(|target| {
                read_member_bytes(
                    package,
                    &target,
                    MAX_FONT_FILE_BYTES.min(limits.max_member_bytes),
                )
                .ok()
                .flatten()
            });
            let plain = match (bytes, key) {
                (Some(mut bytes), Some(key)) => xor_font(&mut bytes, &key).then_some(bytes),
                (Some(bytes), None) => Some(bytes),
                _ => None,
            };
            let file = if let Some(bytes) = plain {
                fonts.keep_file(bytes, report)
            } else {
                report.fonts_left_out += 1;
                None
            };
            if let Some(file) = file {
                family.faces.push(FontFace { style, file });
            }
        }
        fonts.families.push(family);
    }
    fonts
}

/// What a writer knows of a family it has no record of: the kind and pitch of the common ones.
#[must_use]
pub fn known_font(name: &str) -> DocumentFont {
    let generic = font_generic(name);
    DocumentFont {
        name: name.to_owned(),
        generic,
        pitch: if generic == FontGeneric::Modern {
            FontPitch::Fixed
        } else {
            FontPitch::Variable
        },
        ..DocumentFont::default()
    }
}

/// The families a document uses: the body font, then every family a run sets, each once.
#[must_use]
pub fn used_families(tree: &DocumentTree) -> Vec<String> {
    let mut families: Vec<String> = Vec::new();
    let mut add = |family: &str| {
        let family = family.trim();
        if !family.is_empty()
            && families.len() < MAX_FONTS
            && !families
                .iter()
                .any(|known| known.eq_ignore_ascii_case(family))
        {
            families.push(family.to_owned());
        }
    };
    if let Some(family) = &tree.layout.font_family {
        add(family);
    }
    let mut visit = |blocks: &[Block]| {
        visit_marks(blocks, &mut |mark| {
            if let Mark::TextStyle {
                font_family: Some(family),
                ..
            } = mark
            {
                add(family);
            }
        });
    };
    visit(&tree.blocks);
    for note in &tree.notes {
        visit(&note.blocks);
    }
    for comment in &tree.comments {
        visit(&comment.blocks);
    }
    for parts in std::iter::once(&tree.headers_footers)
        .chain(tree.sections.iter().map(|section| &section.headers_footers))
    {
        for part in header_footer_parts(parts) {
            visit(part);
        }
    }
    families
}

fn header_footer_parts(parts: &HeadersFooters) -> impl Iterator<Item = &Vec<Block>> {
    [&parts.header, &parts.footer].into_iter().chain(
        [
            &parts.first_header,
            &parts.first_footer,
            &parts.even_header,
            &parts.even_footer,
        ]
        .into_iter()
        .flatten(),
    )
}

/// Every mark of the runs of some blocks, tables and quotes included.
fn visit_marks(blocks: &[Block], visit: &mut impl FnMut(&Mark)) {
    for block in blocks {
        match block {
            Block::Heading { content, .. }
            | Block::Paragraph { content, .. }
            | Block::ListItem { content, .. } => {
                for inline in content {
                    if let Inline::Text { marks, .. } = inline {
                        marks.iter().for_each(&mut *visit);
                    }
                }
            }
            Block::Table { rows, .. } => {
                for cell in rows.iter().flat_map(|row| &row.cells) {
                    visit_marks(&cell.blocks, visit);
                }
            }
            Block::Blockquote { blocks } => visit_marks(blocks, visit),
            Block::CodeBlock { .. } | Block::HorizontalRule => {}
        }
    }
}

/// A part of a package to write: its path and its bytes.
pub(crate) type Part = (String, Vec<u8>);

/// The font table part of a docx for the families a document uses, its embedded faces written
/// as obfuscated parts; returns the part, its relationships and the font parts with their bytes.
pub(crate) fn docx_font_table(tree: &DocumentTree) -> (String, Vec<(String, String)>, Vec<Part>) {
    let mut xml = String::from(concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<w:fonts xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" "#,
        r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">"#,
    ));
    let mut relationships = Vec::new();
    let mut parts = Vec::new();
    for name in used_families(tree) {
        let font = tree
            .fonts
            .family(&name)
            .cloned()
            .unwrap_or_else(|| known_font(&name));
        let _ = write!(xml, r#"<w:font w:name="{}">"#, escape(&name));
        if let Some(alt) = &font.alt_name {
            let _ = write!(xml, r#"<w:altName w:val="{}"/>"#, escape(alt));
        }
        if let Some(panose) = font.panose.as_deref().filter(|panose| is_hex(panose, 20)) {
            let _ = write!(xml, r#"<w:panose1 w:val="{panose}"/>"#);
        }
        if let Some(charset) = font.charset.as_deref().filter(|charset| is_hex(charset, 2)) {
            let _ = write!(xml, r#"<w:charset w:val="{charset}"/>"#);
        }
        let _ = write!(
            xml,
            r#"<w:family w:val="{}"/><w:pitch w:val="{}"/>"#,
            font.generic.name(),
            font.pitch.name()
        );
        for face in &font.faces {
            let Some(bytes) = tree.fonts.files.get(face.file) else {
                continue;
            };
            let guid = key_guid_for(bytes);
            let Some(key) = font_key(&guid) else {
                continue;
            };
            let mut obfuscated = bytes.clone();
            if !xor_font(&mut obfuscated, &key) {
                continue;
            }
            let id = format!("rId{}", relationships.len() + 1);
            let target = format!("fonts/font{}.odttf", parts.len() + 1);
            let _ = write!(
                xml,
                r#"<w:{} r:id="{id}" w:fontKey="{guid}"/>"#,
                face.style.word_element()
            );
            relationships.push((id, target.clone()));
            parts.push((target, obfuscated));
        }
        xml.push_str("</w:font>");
    }
    xml.push_str("</w:fonts>");
    (xml, relationships, parts)
}

fn is_hex(text: &str, length: usize) -> bool {
    text.len() == length && text.chars().all(|character| character.is_ascii_hexdigit())
}

fn escape(text: &str) -> String {
    super::write::escape(text)
}

/// The fonts an `OpenDocument` package declares in the font face declarations of one of its
/// parts, with the files it embeds under `Fonts/`.
pub(crate) fn read_odf_fonts(
    package: &mut ZipPackage<'_>,
    roots: &[&Element],
    limits: &Limits,
    report: &mut ImportReport,
) -> DocumentFonts {
    const OFFICE: Namespace = Namespace::OdfOffice;
    const STYLE: Namespace = Namespace::OdfStyle;
    const SVG: Namespace = Namespace::OdfSvg;
    let mut fonts = DocumentFonts::default();
    for root in roots {
        let Some(declarations) = root.child(OFFICE, "font-face-decls") else {
            continue;
        };
        for face in declarations
            .elements()
            .filter(|face| face.is(STYLE, "font-face"))
        {
            let Some(name) = face
                .attribute(SVG, "font-family")
                .map(unquote)
                .filter(|name| !name.is_empty())
                .or_else(|| face.attribute(STYLE, "name").map(str::to_owned))
            else {
                continue;
            };
            if fonts.families.len() >= MAX_FONTS {
                break;
            }
            let index = if let Some(index) = fonts
                .families
                .iter()
                .position(|known| known.name.eq_ignore_ascii_case(&name))
            {
                index
            } else {
                fonts.families.push(DocumentFont {
                    name: name.clone(),
                    generic: face
                        .attribute(STYLE, "font-family-generic")
                        .map_or(FontGeneric::Auto, FontGeneric::parse),
                    pitch: face
                        .attribute(STYLE, "font-pitch")
                        .map_or(FontPitch::Default, FontPitch::parse),
                    ..DocumentFont::default()
                });
                fonts.families.len() - 1
            };
            let mut found = Vec::new();
            collect(face, SVG, "font-face-uri", &mut found);
            let uris: Vec<String> = found
                .into_iter()
                .filter_map(|uri| uri.attribute(Namespace::XLink, "href"))
                .map(|href| href.trim_start_matches("./").to_owned())
                .filter(|href| !href.contains("://") && !href.starts_with('/'))
                .collect();
            for href in uris {
                let bytes = read_member_bytes(
                    package,
                    &href,
                    MAX_FONT_FILE_BYTES.min(limits.max_member_bytes),
                )
                .ok()
                .flatten();
                let Some(bytes) = bytes.filter(|bytes| is_font_file(bytes)) else {
                    report.fonts_left_out += 1;
                    continue;
                };
                let style = face_style_of(&bytes).unwrap_or_default();
                if fonts.families[index]
                    .faces
                    .iter()
                    .any(|known| known.style == style)
                {
                    continue;
                }
                if let Some(file) = fonts.keep_file(bytes, report) {
                    fonts.families[index].faces.push(FontFace { style, file });
                }
            }
        }
    }
    fonts
}

/// Every element of a name under one, depth first.
fn collect<'a>(
    element: &'a Element,
    namespace: Namespace,
    name: &str,
    found: &mut Vec<&'a Element>,
) {
    for child in element.elements() {
        if child.is(namespace, name) {
            found.push(child);
        }
        collect(child, namespace, name, found);
    }
}

fn unquote(value: &str) -> String {
    value.trim().trim_matches(['\'', '"']).trim().to_owned()
}

/// The font face declarations of an `OpenDocument` part for the families a document uses, and
/// the files its embedded faces become under `Fonts/`.
pub(crate) fn odf_font_faces(tree: &DocumentTree) -> (String, Vec<(String, Vec<u8>)>) {
    let mut xml = String::from("<office:font-face-decls>");
    let mut files = Vec::new();
    for name in used_families(tree) {
        let font = tree
            .fonts
            .family(&name)
            .cloned()
            .unwrap_or_else(|| known_font(&name));
        let quoted = escape(&name.replace('\'', ""));
        let _ = write!(
            xml,
            r#"<style:font-face style:name="{quoted}" svg:font-family="'{quoted}'""#
        );
        if font.generic != FontGeneric::Auto {
            let _ = write!(
                xml,
                r#" style:font-family-generic="{}""#,
                font.generic.name()
            );
        }
        if font.pitch != FontPitch::Default {
            let _ = write!(xml, r#" style:font-pitch="{}""#, font.pitch.name());
        }
        let faces: Vec<(FaceStyle, &Vec<u8>)> = font
            .faces
            .iter()
            .filter_map(|face| {
                tree.fonts
                    .files
                    .get(face.file)
                    .map(|bytes| (face.style, bytes))
            })
            .collect();
        if faces.is_empty() {
            xml.push_str("/>");
            continue;
        }
        xml.push_str("><svg:font-face-src>");
        for (style, bytes) in faces {
            let path = format!("Fonts/font{}-{}.ttf", files.len() + 1, style_suffix(style));
            let _ = write!(
                xml,
                r#"<svg:font-face-uri xlink:href="{path}" xlink:type="simple"><svg:font-face-format svg:string="truetype"/></svg:font-face-uri>"#
            );
            files.push((path, bytes.clone()));
        }
        xml.push_str("</svg:font-face-src></style:font-face>");
    }
    xml.push_str("</office:font-face-decls>");
    (xml, files)
}

fn style_suffix(style: FaceStyle) -> &'static str {
    match style {
        FaceStyle::Regular => "regular",
        FaceStyle::Bold => "bold",
        FaceStyle::Italic => "italic",
        FaceStyle::BoldItalic => "bolditalic",
    }
}

/// The bundled metric-compatible family that sets `family` when the reader does not hold it,
/// among the ones the packages ship under the SIL Open Font License: Carlito and Caladea match
/// Calibri and Cambria, the Liberation fonts match Arial, Times New Roman and Courier New.
#[must_use]
pub fn font_substitute(family: &str, generic: FontGeneric) -> &'static str {
    let lower = family.trim().to_ascii_lowercase();
    if let Some((_, substitute, _)) = SUBSTITUTES.iter().find(|(name, ..)| *name == lower) {
        return substitute;
    }
    match generic {
        FontGeneric::Roman => "Liberation Serif",
        FontGeneric::Modern => "Liberation Mono",
        FontGeneric::Swiss => "Liberation Sans",
        _ if lower.contains("mono") || lower.contains("code") || lower.contains("courier") => {
            "Liberation Mono"
        }
        _ if lower.contains("serif") && !lower.contains("sans") => "Liberation Serif",
        _ => "Carlito",
    }
}

/// The kind of a family, from the table of known ones or from its name.
#[must_use]
pub fn font_generic(family: &str) -> FontGeneric {
    let lower = family.trim().to_ascii_lowercase();
    if let Some((_, _, generic)) = SUBSTITUTES.iter().find(|(name, ..)| *name == lower) {
        return *generic;
    }
    if lower.contains("mono") || lower.contains("code") || lower.contains("courier") {
        FontGeneric::Modern
    } else if lower.contains("serif") && !lower.contains("sans") {
        FontGeneric::Roman
    } else {
        FontGeneric::Auto
    }
}

/// A CSS font stack for a family: the family, its bundled substitute, then the generic family.
#[must_use]
pub fn css_font_stack(family: &str, generic: FontGeneric) -> String {
    let family = family
        .trim()
        .replace(['\'', '"', ';', '{', '}', '<', '>'], "");
    let generic = if generic == FontGeneric::Auto {
        font_generic(&family)
    } else {
        generic
    };
    let substitute = font_substitute(&family, generic);
    if family.is_empty() {
        return format!("'{substitute}', {}", generic.css());
    }
    if family.eq_ignore_ascii_case(substitute) {
        return format!("'{family}', {}", generic.css());
    }
    format!("'{family}', '{substitute}', {}", generic.css())
}

/// Known families, lower case, with the bundled family of the same metrics or the nearest in
/// kind, and their kind. The React package keeps a copy of this table in `fonts.ts`; a test
/// keeps the two equal.
pub const SUBSTITUTES: &[(&str, &str, FontGeneric)] = &[
    ("calibri", "Carlito", FontGeneric::Swiss),
    ("calibri light", "Carlito", FontGeneric::Swiss),
    ("aptos", "Carlito", FontGeneric::Swiss),
    ("aptos display", "Carlito", FontGeneric::Swiss),
    ("aptos narrow", "Carlito", FontGeneric::Swiss),
    ("candara", "Carlito", FontGeneric::Swiss),
    ("corbel", "Carlito", FontGeneric::Swiss),
    ("century gothic", "Carlito", FontGeneric::Swiss),
    ("gill sans mt", "Carlito", FontGeneric::Swiss),
    ("carlito", "Carlito", FontGeneric::Swiss),
    ("cambria", "Caladea", FontGeneric::Roman),
    ("cambria math", "Caladea", FontGeneric::Roman),
    ("constantia", "Caladea", FontGeneric::Roman),
    ("caladea", "Caladea", FontGeneric::Roman),
    ("arial", "Liberation Sans", FontGeneric::Swiss),
    ("arial narrow", "Liberation Sans", FontGeneric::Swiss),
    ("helvetica", "Liberation Sans", FontGeneric::Swiss),
    ("helvetica neue", "Liberation Sans", FontGeneric::Swiss),
    ("verdana", "Liberation Sans", FontGeneric::Swiss),
    ("tahoma", "Liberation Sans", FontGeneric::Swiss),
    ("segoe ui", "Liberation Sans", FontGeneric::Swiss),
    ("trebuchet ms", "Liberation Sans", FontGeneric::Swiss),
    ("lucida sans unicode", "Liberation Sans", FontGeneric::Swiss),
    (
        "franklin gothic book",
        "Liberation Sans",
        FontGeneric::Swiss,
    ),
    ("liberation sans", "Liberation Sans", FontGeneric::Swiss),
    ("arimo", "Liberation Sans", FontGeneric::Swiss),
    ("dejavu sans", "Liberation Sans", FontGeneric::Swiss),
    ("times new roman", "Liberation Serif", FontGeneric::Roman),
    ("times", "Liberation Serif", FontGeneric::Roman),
    ("georgia", "Liberation Serif", FontGeneric::Roman),
    ("garamond", "Liberation Serif", FontGeneric::Roman),
    ("book antiqua", "Liberation Serif", FontGeneric::Roman),
    ("palatino", "Liberation Serif", FontGeneric::Roman),
    ("palatino linotype", "Liberation Serif", FontGeneric::Roman),
    ("bookman old style", "Liberation Serif", FontGeneric::Roman),
    ("century schoolbook", "Liberation Serif", FontGeneric::Roman),
    ("liberation serif", "Liberation Serif", FontGeneric::Roman),
    ("tinos", "Liberation Serif", FontGeneric::Roman),
    ("dejavu serif", "Liberation Serif", FontGeneric::Roman),
    ("courier new", "Liberation Mono", FontGeneric::Modern),
    ("courier", "Liberation Mono", FontGeneric::Modern),
    ("consolas", "Liberation Mono", FontGeneric::Modern),
    ("lucida console", "Liberation Mono", FontGeneric::Modern),
    ("cascadia code", "Liberation Mono", FontGeneric::Modern),
    ("cascadia mono", "Liberation Mono", FontGeneric::Modern),
    ("liberation mono", "Liberation Mono", FontGeneric::Modern),
    ("cousine", "Liberation Mono", FontGeneric::Modern),
    ("dejavu sans mono", "Liberation Mono", FontGeneric::Modern),
];

#[cfg(test)]
mod tests;
