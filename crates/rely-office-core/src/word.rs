// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use std::io::{Cursor, Read};

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::{Limits, OfficeError};

/// The main part of a docx package; headers, footers and notes are separate parts read after it.
const MAIN_PART: &str = "word/document.xml";

/// The paragraphs of a docx in reading order: the body, then headers, footers, footnotes and
/// endnotes, each paragraph ending with a newline and a table cell with a tab.
pub(crate) fn extract_docx_text(bytes: &[u8], limits: &Limits) -> Result<String, OfficeError> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| OfficeError::DamagedContainer(error.to_string()))?;

    let mut part_names: Vec<String> = archive
        .file_names()
        .filter(|name| is_secondary_text_part(name))
        .map(str::to_owned)
        .collect();
    part_names.sort();
    part_names.insert(0, MAIN_PART.to_owned());

    let mut text = String::new();
    for part_name in part_names {
        let Some(xml) = read_member(&mut archive, &part_name, limits)? else {
            continue;
        };
        append_word_xml_text(&xml, &mut text, limits.max_text_chars)?;
        if text.chars().count() >= limits.max_text_chars {
            break;
        }
    }

    Ok(truncate_chars(text, limits.max_text_chars))
}

fn is_secondary_text_part(name: &str) -> bool {
    let Some(file) = name.strip_prefix("word/") else {
        return false;
    };
    let has_text_role = ["header", "footer", "footnotes", "endnotes"]
        .iter()
        .any(|role| file.starts_with(role));
    has_text_role
        && std::path::Path::new(file)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("xml"))
        && !file.contains('/')
}

/// A member read whole through the member cap; a missing member is `None`, not an error.
fn read_member(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    name: &str,
    limits: &Limits,
) -> Result<Option<String>, OfficeError> {
    let member = match archive.by_name(name) {
        Ok(member) => member,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(error) => return Err(OfficeError::DamagedContainer(error.to_string())),
    };

    let mut xml = String::new();
    // One byte past the cap tells a member at the limit from one above it.
    member
        .take(limits.max_member_bytes + 1)
        .read_to_string(&mut xml)
        .map_err(|error| OfficeError::DamagedContainer(error.to_string()))?;
    if xml.len() as u64 > limits.max_member_bytes {
        return Err(OfficeError::TooLarge {
            what: name.to_owned(),
            limit: limits.max_member_bytes,
        });
    }

    Ok(Some(xml))
}

/// Walk `WordprocessingML`: text runs (`w:t`), tabs and breaks inside a paragraph (`w:p`).
fn append_word_xml_text(xml: &str, text: &mut String, max_chars: usize) -> Result<(), OfficeError> {
    let mut reader = Reader::from_str(xml);
    let mut in_text_run = false;

    loop {
        let event = reader
            .read_event()
            .map_err(|error| OfficeError::MalformedXml(error.to_string()))?;
        match event {
            Event::Start(element) if element.local_name().as_ref() == b"t" => in_text_run = true,
            Event::End(element) => match element.local_name().as_ref() {
                b"t" => in_text_run = false,
                b"p" => text.push('\n'),
                b"tc" => text.push('\t'),
                _ => {}
            },
            Event::Empty(element) => match element.local_name().as_ref() {
                b"tab" => text.push('\t'),
                b"br" | b"cr" => text.push('\n'),
                _ => {}
            },
            Event::Text(run) if in_text_run => {
                let decoded = run
                    .xml10_content()
                    .map_err(|error| OfficeError::MalformedXml(error.to_string()))?;
                text.push_str(&decoded);
            }
            Event::GeneralRef(reference) if in_text_run => {
                if let Ok(Some(character)) = reference.resolve_char_ref() {
                    text.push(character);
                } else if let Ok(name) = reference.decode()
                    && let Some(resolved) = quick_xml::escape::resolve_xml_entity(&name)
                {
                    text.push_str(resolved);
                }
            }
            Event::Eof => break,
            _ => {}
        }
        if text.len() >= max_chars.saturating_mul(4) {
            break;
        }
    }

    Ok(())
}

fn truncate_chars(text: String, max_chars: usize) -> String {
    match text.char_indices().nth(max_chars) {
        Some((byte_index, _)) => text[..byte_index].to_owned(),
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{build_docx, build_docx_with_parts};

    #[test]
    fn reads_paragraphs_in_order() {
        let text = extract_docx_text(
            &build_docx(&["Contratto", "Firmato a Milano"]),
            &Limits::default(),
        );
        assert_eq!(text.unwrap(), "Contratto\nFirmato a Milano\n");
    }

    #[test]
    fn resolves_entities_and_character_references() {
        let text = extract_docx_text(
            &build_docx(&["Rossi &amp; Figli, citt&#224;"]),
            &Limits::default(),
        );
        assert_eq!(text.unwrap(), "Rossi & Figli, città\n");
    }

    #[test]
    fn appends_headers_and_footers_after_the_body() {
        let bytes = build_docx_with_parts(
            &["Corpo"],
            &[
                ("word/header1.xml", "Intestazione"),
                ("word/footer1.xml", "Pagina"),
            ],
        );
        assert_eq!(
            extract_docx_text(&bytes, &Limits::default()).unwrap(),
            "Corpo\nPagina\nIntestazione\n"
        );
    }

    #[test]
    fn stops_at_the_character_limit() {
        let limits = Limits {
            max_text_chars: 4,
            ..Limits::default()
        };
        assert_eq!(
            extract_docx_text(&build_docx(&["Fattura"]), &limits).unwrap(),
            "Fatt"
        );
    }

    #[test]
    fn refuses_a_member_above_the_member_cap() {
        let limits = Limits {
            max_member_bytes: 16,
            ..Limits::default()
        };
        let error = extract_docx_text(&build_docx(&["Testo lungo"]), &limits).unwrap_err();
        assert!(matches!(error, OfficeError::TooLarge { .. }));
    }

    #[test]
    fn refuses_a_damaged_archive() {
        let error = extract_docx_text(b"PK\x03\x04broken", &Limits::default()).unwrap_err();
        assert!(matches!(error, OfficeError::DamagedContainer(_)));
    }
}
