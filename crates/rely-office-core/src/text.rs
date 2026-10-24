// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use crate::word::extract_docx_text;
use crate::{DocumentFormat, Limits, OfficeError, detect_format, read_workbook};

/// The text of a document for a search index: paragraphs for a docx, one line per row with
/// tab-separated cells for a spreadsheet, at most [`Limits::max_text_chars`] characters.
///
/// # Errors
///
/// [`OfficeError::UnsupportedFormat`] for a format this version does not read yet (pptx, odt, doc,
/// ppt), [`OfficeError::TooLarge`] past the input or member limits, and the reading errors of the
/// format.
pub fn extract_text(
    bytes: &[u8],
    file_name: Option<&str>,
    limits: &Limits,
) -> Result<String, OfficeError> {
    if bytes.len() as u64 > limits.max_input_bytes {
        return Err(OfficeError::TooLarge {
            what: "input".to_owned(),
            limit: limits.max_input_bytes,
        });
    }

    match detect_format(bytes, file_name) {
        DocumentFormat::Docx => extract_docx_text(bytes, limits),
        DocumentFormat::Xlsx
        | DocumentFormat::Xls
        | DocumentFormat::Ods
        | DocumentFormat::Csv
        | DocumentFormat::Tsv => {
            // The search reads every row, so the table limits are lifted to the text limit.
            let text_limits = Limits {
                max_rows: limits.max_text_chars,
                max_columns: limits.max_text_chars,
                ..*limits
            };
            let workbook = read_workbook(bytes, file_name, &text_limits)?;
            Ok(workbook_text(&workbook.sheets, limits.max_text_chars))
        }
        other => Err(OfficeError::UnsupportedFormat(other)),
    }
}

fn workbook_text(sheets: &[crate::Sheet], max_chars: usize) -> String {
    let mut text = String::new();
    'sheets: for sheet in sheets {
        if !sheet.name.is_empty() {
            text.push_str(&sheet.name);
            text.push('\n');
        }
        for row in &sheet.rows {
            let line = row.join("\t");
            if line.trim().is_empty() {
                continue;
            }
            text.push_str(line.trim_end());
            text.push('\n');
            if text.chars().count() >= max_chars {
                break 'sheets;
            }
        }
    }

    match text.char_indices().nth(max_chars) {
        Some((byte_index, _)) => text[..byte_index].to_owned(),
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{build_docx, build_xlsx};

    #[test]
    fn extracts_the_text_of_a_docx() {
        let text = extract_text(&build_docx(&["Offerta"]), None, &Limits::default()).unwrap();
        assert_eq!(text, "Offerta\n");
    }

    #[test]
    fn extracts_a_spreadsheet_as_named_tab_separated_lines() {
        let text = extract_text(&build_xlsx(), None, &Limits::default()).unwrap();
        assert!(text.starts_with("Vendite\nMese\tRicavi\nGennaio\t1200.5\n"));
        assert!(text.contains("Clienti\nCliente\tCittà\nAcme\tMilano\n"));
    }

    #[test]
    fn skips_empty_rows_of_a_csv() {
        let text = extract_text(b"a,b\n,\nc,d\n", Some("x.csv"), &Limits::default()).unwrap();
        assert_eq!(text, "a\tb\nc\td\n");
    }

    #[test]
    fn stops_a_spreadsheet_at_the_character_limit() {
        let limits = Limits {
            max_text_chars: 5,
            ..Limits::default()
        };
        assert_eq!(
            extract_text(b"abcdef,g\n", Some("x.csv"), &limits).unwrap(),
            "abcde"
        );
    }

    #[test]
    fn refuses_a_format_not_read_yet() {
        let error = extract_text(b"plain", Some("a.txt"), &Limits::default()).unwrap_err();
        assert!(matches!(error, OfficeError::UnsupportedFormat(_)));
    }
}
