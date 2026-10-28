// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Objects embedded in a document: an OLE object, or an Office file inside another, kept with
//! the picture that stands in for it so a document written back still holds it.

use std::io::{Cursor, Write};
use std::path::Path;

use crate::compound::{CompoundFile, read_stream, u32_at};

/// The first bytes of a compound file and of a zip package.
const COMPOUND_SIGNATURE: &[u8] = &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
const ZIP_SIGNATURE: &[u8] = b"PK\x03\x04";
/// The streams and storages one object may copy; an object pool past it is not an object.
const MAX_OBJECT_ENTRIES: usize = 256;
/// The fixed header of a `CompObj` stream, before its strings.
const COMP_OBJ_HEADER: usize = 28;
/// A `ProgID` is at most 39 characters; anything much longer is not one.
const MAX_PROGRAM_LENGTH: usize = 128;

/// An object embedded in the document, which the document shows as a picture of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedObject {
    /// The picture that shows it, an index in [`super::DocumentTree::media`]; an
    /// [`super::Inline::Image`] of that picture is the object.
    pub media: usize,
    /// What opens it, as its `ProgID`: `Excel.Sheet.12`, `Word.Document.8`, `Package`.
    pub program: Option<String>,
    /// The name it was stored under, `Microsoft_Excel_Worksheet.xlsx` or `oleObject1.bin`.
    pub name: String,
    /// The object itself: an Office package, or a compound file holding an OLE object.
    pub bytes: Vec<u8>,
}

impl EmbeddedObject {
    /// Whether the object is an Office package (an xlsx, a docx, a pptx), rather than an OLE
    /// object in a compound file.
    #[must_use]
    pub fn is_package(&self) -> bool {
        self.bytes.starts_with(ZIP_SIGNATURE)
    }
}

/// The `ProgID` a compound file's `\x01CompObj` stream names, after its user type and its
/// clipboard format.
pub(crate) fn program_of(comp_obj: &[u8]) -> Option<String> {
    let mut position = COMP_OBJ_HEADER;
    let user_type = u32_at(comp_obj, position)? as usize;
    position = position.checked_add(4 + user_type)?;
    let marker = u32_at(comp_obj, position)?;
    position += 4;
    position += match marker {
        0 => 0,
        0xFFFF_FFFF | 0xFFFF_FFFE => 4,
        length => length as usize,
    };
    let length = u32_at(comp_obj, position)? as usize;
    if length == 0 || length > MAX_PROGRAM_LENGTH {
        return None;
    }
    let bytes = comp_obj.get(position + 4..position + 4 + length)?;
    let program: String = bytes
        .iter()
        .take_while(|byte| **byte != 0)
        .map(|byte| char::from(*byte))
        .collect();
    Some(program).filter(|program| {
        !program.is_empty()
            && program
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '.')
    })
}

/// The `ProgID` of an OLE object kept as a compound file, from its `CompObj` stream.
pub(crate) fn program_of_compound_file(bytes: &[u8]) -> Option<String> {
    if !bytes.starts_with(COMPOUND_SIGNATURE) {
        return None;
    }
    let mut compound_file = crate::compound::open_compound_file(bytes).ok()?;
    let comp_obj = read_stream(&mut compound_file, "/\u{1}CompObj", 64 * 1024).ok()??;
    program_of(&comp_obj)
}

/// A storage of a compound file and everything under it, as a compound file of its own whose
/// root is that storage: how an OLE object of a doc's object pool is stored in a docx.
pub(crate) fn storage_as_compound_file(
    compound_file: &mut CompoundFile<'_>,
    storage: &str,
    cap: u64,
) -> Option<Vec<u8>> {
    if !compound_file.is_storage(storage) {
        return None;
    }
    let entries: Vec<(String, bool, cfb::Entry)> = compound_file
        .walk_storage(storage)
        .ok()?
        .take(MAX_OBJECT_ENTRIES + 1)
        .map(|entry| {
            let path = entry.path().to_string_lossy().replace('\\', "/");
            (path, entry.is_stream(), entry)
        })
        .collect();
    if entries.len() > MAX_OBJECT_ENTRIES {
        return None;
    }
    let mut copy =
        cfb::CompoundFile::create_with_version(cfb::Version::V3, Cursor::new(Vec::new())).ok()?;
    let mut total = 0_u64;
    for (path, is_stream, entry) in &entries {
        let relative = path.strip_prefix(storage)?.trim_start_matches('/');
        let target = format!("/{relative}");
        if *is_stream {
            let bytes = read_stream(compound_file, path, cap).ok()??;
            total += bytes.len() as u64;
            if total > cap {
                return None;
            }
            copy.create_stream(Path::new(&target))
                .ok()?
                .write_all(&bytes)
                .ok()?;
        } else {
            if !relative.is_empty() {
                copy.create_storage(Path::new(&target)).ok()?;
            }
            copy.set_storage_clsid(Path::new(&target), *entry.clsid())
                .ok()?;
        }
    }
    copy.flush().ok()?;
    Some(copy.into_inner().into_inner())
}

/// Where the docx writer stores an object, its relationship and its content type: a package
/// under its own name, an OLE object as a `.bin`.
pub(crate) fn object_part(
    object: &EmbeddedObject,
    number: usize,
) -> (String, &'static str, String) {
    if object.is_package() {
        let extension = Path::new(&object.name)
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .filter(|extension| package_content_type(extension).is_some())
            .unwrap_or_else(|| "xlsx".to_owned());
        let content_type = package_content_type(&extension).unwrap_or_default();
        (
            format!("embeddings/object{number}.{extension}"),
            "package",
            content_type.to_owned(),
        )
    } else {
        (
            format!("embeddings/oleObject{number}.bin"),
            "oleObject",
            "application/vnd.openxmlformats-officedocument.oleObject".to_owned(),
        )
    }
}

fn package_content_type(extension: &str) -> Option<&'static str> {
    Some(match extension {
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "xlsm" => "application/vnd.ms-excel.sheet.macroEnabled.12",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "docm" => "application/vnd.ms-word.document.macroEnabled.12",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "pptm" => "application/vnd.ms-powerpoint.presentation.macroEnabled.12",
        "sldx" => "application/vnd.openxmlformats-officedocument.presentationml.slide",
        _ => return None,
    })
}

/// The `ProgID` Office gives a package of this name when the file does not say.
pub(crate) fn program_of_package(name: &str) -> Option<&'static str> {
    let extension = Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())?
        .to_ascii_lowercase();
    Some(match extension.as_str() {
        "xlsx" | "xlsm" => "Excel.Sheet.12",
        "docx" | "docm" => "Word.Document.12",
        "pptx" | "pptm" => "PowerPoint.Show.12",
        "sldx" => "PowerPoint.Slide.12",
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
