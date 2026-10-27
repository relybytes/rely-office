// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

mod presentation;
mod spreadsheet;
mod styles;
mod text_tree;

pub(crate) use presentation::read_odp;
pub(crate) use spreadsheet::read_ods_layouts;
pub(crate) use text_tree::read_odt_tree;

use crate::archive::{open_package, read_text_member};
use crate::text::TextBuffer;
use crate::xml_text::{TextRules, append_xml_text};
use crate::{DocumentFormat, Limits, OfficeError};

/// `OpenDocument` text: paragraphs (`text:p`) and headings (`text:h`); a tracked deletion
/// (`text:tracked-changes` keeps the removed text there) is not content any more.
const CONTENT_RULES: TextRules<'static> = TextRules {
    text_containers: &["p", "h"],
    newline_after: &["p", "h"],
    tab_after: &["table-cell"],
    tab_elements: &["tab"],
    break_elements: &["line-break"],
    skipped: &["tracked-changes"],
    space_element: Some("s"),
};

/// The text of an odt (body, then the headers and footers of its page styles) or an odp (each
/// page in order, its speaker notes after its frames, as `content.xml` stores them).
pub(crate) fn extract_opendocument_text(
    bytes: &[u8],
    format: DocumentFormat,
    limits: &Limits,
    buffer: &mut TextBuffer,
) -> Result<(), OfficeError> {
    let mut package = open_package(bytes)?;
    let Some(content) = read_text_member(&mut package, "content.xml", limits)? else {
        return Err(OfficeError::UnreadableDocument(
            "the package has no content.xml".to_owned(),
        ));
    };
    append_xml_text(&content, &CONTENT_RULES, buffer)?;

    if format == DocumentFormat::Odt
        && !buffer.is_truncated()
        && let Some(styles) = read_text_member(&mut package, "styles.xml", limits)?
    {
        // Only the master pages hold paragraphs in styles.xml: their headers and footers.
        append_xml_text(&styles, &CONTENT_RULES, buffer)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{build_odp, build_odt, build_opendocument};

    fn extract(
        bytes: &[u8],
        format: DocumentFormat,
        limits: &Limits,
    ) -> Result<String, OfficeError> {
        let mut buffer = TextBuffer::new(limits.max_text_chars);
        extract_opendocument_text(bytes, format, limits, &mut buffer)?;
        Ok(buffer.into_text())
    }

    #[test]
    fn reads_headings_paragraphs_spans_and_lists() {
        let bytes = build_odt(concat!(
            "<text:h text:outline-level=\"1\">Titolo</text:h>",
            "<text:p>Uno <text:span>due</text:span><text:s text:c=\"2\"/>tre<text:tab/>quattro",
            "<text:line-break/>cinque</text:p>",
            "<text:list><text:list-item><text:p>voce</text:p></text:list-item></text:list>",
        ));
        assert_eq!(
            extract(&bytes, DocumentFormat::Odt, &Limits::default()).unwrap(),
            "Titolo\nUno due  tre\tquattro\ncinque\nvoce\n"
        );
    }

    #[test]
    fn leaves_out_tracked_deletions_and_reads_tables() {
        let bytes = build_odt(concat!(
            "<text:tracked-changes><text:changed-region><text:deletion><text:p>tolto</text:p>",
            "</text:deletion></text:changed-region></text:tracked-changes>",
            "<table:table><table:table-row><table:table-cell><text:p>A</text:p></table:table-cell>",
            "<table:table-cell><text:p>B</text:p></table:table-cell></table:table-row></table:table>",
        ));
        assert_eq!(
            extract(&bytes, DocumentFormat::Odt, &Limits::default()).unwrap(),
            "A\n\tB\n\t"
        );
    }

    #[test]
    fn appends_master_page_headers_and_footers_of_an_odt() {
        let bytes = build_opendocument(
            "application/vnd.oasis.opendocument.text",
            "<office:text><text:p>Corpo</text:p></office:text>",
            concat!(
                "<style:master-page style:name=\"Standard\"><style:header><text:p>Intestazione",
                "</text:p></style:header><style:footer><text:p>Pie</text:p></style:footer>",
                "</style:master-page>",
            ),
        );
        assert_eq!(
            extract(&bytes, DocumentFormat::Odt, &Limits::default()).unwrap(),
            "Corpo\nIntestazione\nPie\n"
        );
    }

    #[test]
    fn reads_the_pages_of_an_odp_with_their_notes() {
        let bytes = build_opendocument(
            "application/vnd.oasis.opendocument.presentation",
            concat!(
                "<office:presentation><draw:page><draw:frame><draw:text-box><text:p>Uno</text:p>",
                "</draw:text-box></draw:frame><presentation:notes><draw:frame><draw:text-box>",
                "<text:p>Nota</text:p></draw:text-box></draw:frame></presentation:notes></draw:page>",
                "<draw:page><draw:frame><draw:text-box><text:p>Due</text:p></draw:text-box>",
                "</draw:frame></draw:page></office:presentation>",
            ),
            "",
        );
        assert_eq!(
            extract(&bytes, DocumentFormat::Odp, &Limits::default()).unwrap(),
            "Uno\nNota\nDue\n"
        );
        assert_eq!(
            extract(
                &build_odp(&["Solo"]),
                DocumentFormat::Odp,
                &Limits::default()
            )
            .unwrap(),
            "Solo\n"
        );
    }

    #[test]
    fn stops_at_the_character_limit() {
        let limits = Limits {
            max_text_chars: 2,
            ..Limits::default()
        };
        assert_eq!(
            extract(
                &build_odt("<text:p>Testo</text:p>"),
                DocumentFormat::Odt,
                &limits
            )
            .unwrap(),
            "Te"
        );
    }

    #[test]
    fn refuses_a_package_without_content() {
        let bytes = crate::test_support::build_zip(&[("mimetype", b"x".as_slice())]);
        assert!(matches!(
            extract(&bytes, DocumentFormat::Odt, &Limits::default()).unwrap_err(),
            OfficeError::UnreadableDocument(_)
        ));
    }

    #[test]
    fn refuses_content_above_the_member_cap() {
        let limits = Limits {
            max_member_bytes: 32,
            ..Limits::default()
        };
        assert!(matches!(
            extract(
                &build_odt("<text:p>Testo</text:p>"),
                DocumentFormat::Odt,
                &limits
            )
            .unwrap_err(),
            OfficeError::TooLarge { .. }
        ));
    }
}
