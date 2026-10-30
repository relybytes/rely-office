// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! The macros of a macro-enabled Office file (docm, xlsm, pptm): the VBA project and the parts
//! it relates to, carried as bytes and written back into the saved file. Nothing here reads,
//! runs or changes a macro.

use std::collections::HashSet;
use std::io::{Cursor, Read, Write};

use zip::write::SimpleFileOptions;

use crate::archive::{member_names, open_package, read_member_bytes, read_text_member};
use crate::compound::{open_compound_file, read_stream};
use crate::office_art::{child, records};
use crate::relationships::{main_part_path, read_relationships};
use crate::sheet_drawings::{ContentTypes, carried_tree, relationships_part};
use crate::{CarriedPart, DocumentFormat, Limits, OfficeError, detect_format};

/// The relationship from a main part to its VBA project.
const VBA_PROJECT_RELATIONSHIP: &str =
    "http://schemas.microsoft.com/office/2006/relationships/vbaProject";
const RELATIONSHIPS_TYPE: &str = "application/vnd.openxmlformats-package.relationships+xml";
/// The most parts a macro project carries.
const MAX_PROJECT_PARTS: usize = 64;
/// The main part's content type of each format, and of the same format with macros.
const MACRO_ENABLED_TYPES: [(&str, &str); 3] = [
    (
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
        "application/vnd.ms-word.document.macroEnabled.main+xml",
    ),
    (
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
        "application/vnd.ms-excel.sheet.macroEnabled.main+xml",
    ),
    (
        "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml",
        "application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml",
    ),
];
/// The record of a ppt's document that holds its VBA project.
const PPT_DOCUMENT: u16 = 0x03E8;
const PPT_VBA_INFO: u16 = 0x03FF;

/// The VBA project of a docm, an xlsm or a pptm, with the parts it relates to (the data Word
/// keeps beside it, a signature) and their relationships, as the file had them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MacroProject {
    /// The member name of the project, `word/vbaProject.bin`.
    pub project_part: String,
    pub parts: Vec<CarriedPart>,
}

impl MacroProject {
    /// The project as one opaque value for a host to hold and give back: a small package of its
    /// parts, with their content types and the relationship naming the project.
    ///
    /// # Errors
    ///
    /// [`OfficeError::DamagedContainer`] when the zip cannot be written.
    pub fn to_bytes(&self) -> Result<Vec<u8>, OfficeError> {
        let mut types = String::from(concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">"#,
            r#"<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>"#,
        ));
        for part in self.parts.iter().filter(|part| !is_relationships(part)) {
            types.push_str(&override_xml(part));
        }
        types.push_str("</Types>");
        let relationships = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{VBA_PROJECT_RELATIONSHIP}" Target="/{}"/></Relationships>"#,
            escape(&self.project_part)
        );
        let mut members: Vec<(&str, &[u8])> = vec![
            ("[Content_Types].xml", types.as_bytes()),
            ("_rels/.rels", relationships.as_bytes()),
        ];
        members.extend(
            self.parts
                .iter()
                .map(|part| (part.name.as_str(), part.bytes.as_slice())),
        );
        write_zip(&members)
    }

    /// A project from the value [`MacroProject::to_bytes`] gave.
    ///
    /// # Errors
    ///
    /// [`OfficeError::DamagedContainer`] or [`OfficeError::UnreadableDocument`] for bytes that
    /// are not such a value, [`OfficeError::TooLarge`] past the limits.
    pub fn from_bytes(bytes: &[u8], limits: &Limits) -> Result<Self, OfficeError> {
        let unreadable =
            || OfficeError::UnreadableDocument("the bytes are not a macro project".to_owned());
        let mut package = open_package(bytes)?;
        let project_part = read_relationships(&mut package, "", limits)?
            .into_iter()
            .find(|relationship| relationship.kind == VBA_PROJECT_RELATIONSHIP)
            .map(|relationship| relationship.target)
            .ok_or_else(unreadable)?;
        let types = read_text_member(&mut package, "[Content_Types].xml", limits)?
            .map(|xml| ContentTypes::parse(&xml))
            .ok_or_else(unreadable)?;
        let names: Vec<String> = member_names(&package)
            .into_iter()
            .filter(|name| name != "[Content_Types].xml" && name != "_rels/.rels")
            .collect();
        if names.len() > MAX_PROJECT_PARTS {
            return Err(OfficeError::TooLarge {
                what: "macro project parts".to_owned(),
                limit: MAX_PROJECT_PARTS as u64,
            });
        }
        let mut parts = Vec::new();
        let mut total: u64 = 0;
        for name in names {
            let bytes = read_member_bytes(&mut package, &name, limits.max_member_bytes)?
                .unwrap_or_default();
            total += bytes.len() as u64;
            if total > limits.max_input_bytes {
                return Err(OfficeError::TooLarge {
                    what: "macro project".to_owned(),
                    limit: limits.max_input_bytes,
                });
            }
            let content_type = if name.to_ascii_lowercase().ends_with(".rels") {
                RELATIONSHIPS_TYPE.to_owned()
            } else {
                types.of(&name)
            };
            parts.push(CarriedPart {
                name,
                content_type,
                bytes,
            });
        }
        if !parts.iter().any(|part| part.name == project_part) {
            return Err(unreadable());
        }
        Ok(Self {
            project_part,
            parts,
        })
    }
}

/// The VBA project of an OOXML package and the parts it reaches; `None` for a package without
/// one, or one too large to carry whole.
#[must_use]
pub fn read_macro_project(bytes: &[u8], limits: &Limits) -> Option<MacroProject> {
    let mut package = open_package(bytes).ok()?;
    let main = main_part_path(&mut package, limits, default_main_part(bytes)).ok()?;
    let project_part = read_relationships(&mut package, &main, limits)
        .ok()?
        .into_iter()
        .find(|relationship| !relationship.external && relationship.is_kind("vbaProject"))?
        .target;
    let types = read_text_member(&mut package, "[Content_Types].xml", limits)
        .ok()
        .flatten()
        .map(|xml| ContentTypes::parse(&xml))?;
    let parts = carried_tree(&mut package, &project_part, limits, &types)?;
    (parts.len() <= MAX_PROJECT_PARTS).then_some(MacroProject {
        project_part,
        parts,
    })
}

/// Whether a file holds macros: a VBA project in an OOXML package, the `Macros` storage of a
/// doc, the `_VBA_PROJECT_CUR` storage of an xls or the VBA record of a ppt's document.
#[must_use]
pub fn holds_macros(bytes: &[u8], limits: &Limits) -> bool {
    match detect_format(bytes, None) {
        DocumentFormat::Docx | DocumentFormat::Xlsx | DocumentFormat::Pptx => {
            let Ok(mut package) = open_package(bytes) else {
                return false;
            };
            main_part_path(&mut package, limits, default_main_part(bytes))
                .and_then(|main| read_relationships(&mut package, &main, limits))
                .is_ok_and(|relationships| {
                    relationships
                        .iter()
                        .any(|relationship| relationship.is_kind("vbaProject"))
                })
        }
        DocumentFormat::Doc => {
            open_compound_file(bytes).is_ok_and(|compound_file| compound_file.is_storage("/Macros"))
        }
        DocumentFormat::Xls => open_compound_file(bytes)
            .is_ok_and(|compound_file| compound_file.is_storage("/_VBA_PROJECT_CUR")),
        DocumentFormat::Ppt => open_compound_file(bytes)
            .ok()
            .and_then(|mut compound_file| {
                read_stream(
                    &mut compound_file,
                    "/PowerPoint Document",
                    limits.max_member_bytes,
                )
                .ok()
                .flatten()
            })
            .is_some_and(|stream| {
                records(&stream)
                    .filter(|record| record.kind == PPT_DOCUMENT)
                    .any(|document| child(document.body, PPT_VBA_INFO).is_some())
            }),
        _ => false,
    }
}

/// A package with a macro project put in: its parts added, the main part relating to it and
/// its content type the macro-enabled one, so a docx becomes a docm, an xlsx an xlsm and a pptx
/// a pptm. A part the package already has is left as the package has it.
///
/// # Errors
///
/// [`OfficeError::DamagedContainer`] when the package cannot be read or written, and
/// [`OfficeError::UnreadableDocument`] when it has no main part.
pub fn insert_macro_project(
    package_bytes: &[u8],
    project: &MacroProject,
) -> Result<Vec<u8>, OfficeError> {
    let limits = Limits {
        max_member_bytes: u64::MAX,
        ..Limits::default()
    };
    let mut package = open_package(package_bytes)?;
    let main = main_part_path(&mut package, &limits, default_main_part(package_bytes))?;
    if main.is_empty() {
        return Err(OfficeError::UnreadableDocument(
            "the package names no main part".to_owned(),
        ));
    }
    let main_relationships = relationships_part(&main);
    let names = member_names(&package);
    let existing: HashSet<String> = names.iter().map(|name| name.to_ascii_lowercase()).collect();
    let added: Vec<&CarriedPart> = project
        .parts
        .iter()
        .filter(|part| !existing.contains(&part.name.to_ascii_lowercase()))
        .collect();
    let mut members: Vec<(String, Vec<u8>)> = Vec::new();
    for name in &names {
        let mut bytes = Vec::new();
        package
            .by_name(name)
            .map_err(|error| OfficeError::DamagedContainer(error.to_string()))?
            .read_to_end(&mut bytes)
            .map_err(|error| OfficeError::DamagedContainer(error.to_string()))?;
        if name == "[Content_Types].xml" {
            bytes = with_macro_types(&String::from_utf8_lossy(&bytes), &added).into_bytes();
        } else if *name == main_relationships {
            bytes = with_project_relationship(&String::from_utf8_lossy(&bytes), &main, project)
                .into_bytes();
        }
        members.push((name.clone(), bytes));
    }
    if !names.contains(&main_relationships) {
        let empty = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#;
        members.push((
            main_relationships,
            with_project_relationship(empty, &main, project).into_bytes(),
        ));
    }
    members.extend(
        added
            .iter()
            .map(|part| (part.name.clone(), part.bytes.clone())),
    );
    let members: Vec<(&str, &[u8])> = members
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .collect();
    write_zip(&members)
}

/// The content types of a package with the main part made macro-enabled and the project's
/// parts declared.
fn with_macro_types(xml: &str, added: &[&CarriedPart]) -> String {
    let mut xml = xml.to_owned();
    for (plain, enabled) in MACRO_ENABLED_TYPES {
        xml = xml.replace(
            &format!(r#"ContentType="{plain}""#),
            &format!(r#"ContentType="{enabled}""#),
        );
    }
    let overrides: String = added
        .iter()
        .filter(|part| !is_relationships(part))
        .map(|part| override_xml(part))
        .collect();
    match xml.rfind("</Types>") {
        Some(end) => xml.insert_str(end, &overrides),
        None => xml.push_str(&overrides),
    }
    xml
}

/// The relationships of the main part with one more, to the project: by its name when the two
/// share a folder, from the package root otherwise.
fn with_project_relationship(xml: &str, main: &str, project: &MacroProject) -> String {
    let folder = |name: &str| {
        name.rsplit_once('/')
            .map_or("", |(folder, _)| folder)
            .to_owned()
    };
    let target = if folder(main) == folder(&project.project_part) {
        project
            .project_part
            .rsplit_once('/')
            .map_or(project.project_part.as_str(), |(_, file)| file)
            .to_owned()
    } else {
        format!("/{}", project.project_part)
    };
    let relationship = format!(
        r#"<Relationship Id="rIdVbaProject" Type="{VBA_PROJECT_RELATIONSHIP}" Target="{}"/>"#,
        escape(&target)
    );
    let mut xml = xml.to_owned();
    match xml.rfind("</Relationships>") {
        Some(end) => xml.insert_str(end, &relationship),
        None => xml.push_str(&relationship),
    }
    xml
}

/// Where a package of each format keeps its main part when `_rels/.rels` does not say.
fn default_main_part(bytes: &[u8]) -> &'static str {
    match detect_format(bytes, None) {
        DocumentFormat::Docx => "word/document.xml",
        DocumentFormat::Xlsx => "xl/workbook.xml",
        DocumentFormat::Pptx => "ppt/presentation.xml",
        _ => "",
    }
}

fn is_relationships(part: &CarriedPart) -> bool {
    part.content_type == RELATIONSHIPS_TYPE || part.name.to_ascii_lowercase().ends_with(".rels")
}

fn override_xml(part: &CarriedPart) -> String {
    format!(
        r#"<Override PartName="/{}" ContentType="{}"/>"#,
        escape(&part.name),
        escape(&part.content_type)
    )
}

fn write_zip(members: &[(&str, &[u8])]) -> Result<Vec<u8>, OfficeError> {
    let damaged = |error: &dyn std::fmt::Display| OfficeError::DamagedContainer(error.to_string());
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in members {
        zip.start_file(*name, SimpleFileOptions::default())
            .map_err(|error| damaged(&error))?;
        zip.write_all(bytes).map_err(|error| damaged(&error))?;
    }
    zip.finish()
        .map(Cursor::into_inner)
        .map_err(|error| damaged(&error))
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests;
