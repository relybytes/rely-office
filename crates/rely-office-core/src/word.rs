// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use crate::archive::{member_names, open_package, read_text_member};
use crate::text::TextBuffer;
use crate::xml_text::{TextRules, append_xml_text};
use crate::{Limits, OfficeError};

/// The main part of a docx package; headers, footers and notes are separate parts read after it.
const MAIN_PART: &str = "word/document.xml";

/// `WordprocessingML`: text runs (`w:t`), tabs and breaks inside a paragraph (`w:p`); the
/// fallback of `mc:AlternateContent` repeats its choice, so only the choice is read.
const WORD_RULES: TextRules<'static> = TextRules {
    text_containers: &["t"],
    newline_after: &["p"],
    tab_after: &["tc"],
    tab_elements: &["tab"],
    break_elements: &["br", "cr"],
    skipped: &["Fallback"],
    space_element: None,
};

/// The paragraphs of a docx in reading order: the body, then headers, footers, footnotes and
/// endnotes, each paragraph ending with a newline and a table cell with a tab.
pub(crate) fn extract_docx_text(
    bytes: &[u8],
    limits: &Limits,
    buffer: &mut TextBuffer,
) -> Result<(), OfficeError> {
    let mut package = open_package(bytes)?;

    let mut part_names: Vec<String> = member_names(&package)
        .into_iter()
        .filter(|name| is_secondary_text_part(name))
        .collect();
    part_names.sort();
    part_names.insert(0, MAIN_PART.to_owned());

    for part_name in part_names {
        let Some(xml) = read_text_member(&mut package, &part_name, limits)? else {
            continue;
        };
        append_xml_text(&xml, &WORD_RULES, buffer)?;
        if buffer.is_truncated() {
            break;
        }
    }

    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{build_docx, build_docx_with_parts};

    fn extract_docx_text(bytes: &[u8], limits: &Limits) -> Result<String, OfficeError> {
        let mut buffer = TextBuffer::new(limits.max_text_chars);
        super::extract_docx_text(bytes, limits, &mut buffer)?;
        Ok(buffer.into_text())
    }

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
    fn reads_the_choice_of_alternate_content_once() {
        let bytes = build_docx(&[concat!(
            "Prima</w:t></w:r><w:r><mc:AlternateContent><mc:Choice><w:t>casella</w:t></mc:Choice>",
            "<mc:Fallback><w:t>casella</w:t></mc:Fallback></mc:AlternateContent></w:r><w:r><w:t>"
        )]);
        assert_eq!(
            extract_docx_text(&bytes, &Limits::default()).unwrap(),
            "Primacasella\n"
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
