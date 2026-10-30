// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use crate::doc::extract_doc_text;
use crate::opendocument::extract_opendocument_text;
use crate::ppt::extract_ppt_text;
use crate::presentation::extract_pptx_text;
use crate::word::extract_docx_text;
use crate::{DocumentFormat, Limits, OfficeError, detect_format, read_workbook};

/// The text of a document, with the format it was read as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedText {
    pub format: DocumentFormat,
    pub text: String,
    /// Whether the document holds more text than [`Limits::max_text_chars`] let through.
    pub truncated: bool,
}

/// The text of a document for a search index, at most [`Limits::max_text_chars`] characters:
/// paragraphs for word processing files, slides then notes for presentations, one line per row
/// with tab-separated cells for a spreadsheet.
///
/// # Errors
///
/// [`OfficeError::UnsupportedFormat`] for a file that is not an office document,
/// [`OfficeError::TooLarge`] past the input or member limits, [`OfficeError::Encrypted`] for a
/// password-protected binary document, and the reading errors of the format.
pub fn extract_text(
    bytes: &[u8],
    file_name: Option<&str>,
    limits: &Limits,
) -> Result<ExtractedText, OfficeError> {
    if bytes.len() as u64 > limits.max_input_bytes {
        return Err(OfficeError::TooLarge {
            what: "input".to_owned(),
            limit: limits.max_input_bytes,
        });
    }

    let format = detect_format(bytes, file_name);
    let mut buffer = TextBuffer::new(limits.max_text_chars);
    match format {
        DocumentFormat::Docx => extract_docx_text(bytes, limits, &mut buffer)?,
        DocumentFormat::Pptx => extract_pptx_text(bytes, limits, &mut buffer)?,
        DocumentFormat::Odt | DocumentFormat::Odp => {
            extract_opendocument_text(bytes, format, limits, &mut buffer)?;
        }
        DocumentFormat::Doc => extract_doc_text(bytes, limits, &mut buffer)?,
        DocumentFormat::Ppt => extract_ppt_text(bytes, limits, &mut buffer)?,
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
            append_workbook_text(&workbook.sheets, &mut buffer);
        }
        DocumentFormat::Markdown | DocumentFormat::Html => {
            let tree =
                crate::document::read_text_tree(bytes, format, limits).map_err(
                    |error| match error {
                        OfficeError::UnsupportedFormat(_) => OfficeError::UnsupportedFormat(format),
                        other => other,
                    },
                )?;
            crate::document::append_tree_text(&tree.blocks, &mut buffer);
        }
        DocumentFormat::Unknown => return Err(OfficeError::UnsupportedFormat(format)),
    }

    let truncated = buffer.is_truncated();
    Ok(ExtractedText {
        format,
        text: buffer.into_text(),
        truncated,
    })
}

fn append_workbook_text(sheets: &[crate::Sheet], buffer: &mut TextBuffer) {
    for sheet in sheets {
        if !sheet.name.is_empty() {
            buffer.push_str(&sheet.name);
            buffer.push('\n');
        }
        for row in &sheet.rows {
            let line = row.join("\t");
            if line.trim().is_empty() {
                continue;
            }
            buffer.push_str(line.trim_end());
            buffer.push('\n');
            if buffer.is_truncated() {
                return;
            }
        }
    }
}

/// Text collected up to a character cap; anything pushed past the cap marks it truncated.
#[derive(Debug)]
pub(crate) struct TextBuffer {
    text: String,
    chars: usize,
    max_chars: usize,
    truncated: bool,
}

impl TextBuffer {
    pub(crate) fn new(max_chars: usize) -> Self {
        Self {
            text: String::new(),
            chars: 0,
            max_chars,
            truncated: false,
        }
    }

    pub(crate) fn push(&mut self, character: char) {
        if self.chars >= self.max_chars {
            self.truncated = true;
            return;
        }
        self.text.push(character);
        self.chars += 1;
    }

    pub(crate) fn push_str(&mut self, text: &str) {
        for character in text.chars() {
            self.push(character);
            if self.truncated {
                return;
            }
        }
    }

    pub(crate) fn is_truncated(&self) -> bool {
        self.truncated
    }

    pub(crate) fn ends_with_newline(&self) -> bool {
        self.text.is_empty() || self.text.ends_with('\n')
    }

    pub(crate) fn into_text(self) -> String {
        self.text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{build_docx, build_xlsx};

    #[test]
    fn extracts_the_text_of_markdown_and_html() {
        let markdown = extract_text(
            b"# Titolo\n\n| a | b |\n| - | - |\n| uno | due |\n",
            Some("nota.md"),
            &Limits::default(),
        )
        .unwrap();
        assert_eq!(markdown.format, DocumentFormat::Markdown);
        assert_eq!(markdown.text, "Titolo\na\nb\nuno\ndue\n");
        let page = extract_text(
            b"<html><script>nascosto()</script><p>Ciao <b>mondo</b></p></html>",
            None,
            &Limits::default(),
        )
        .unwrap();
        assert_eq!(page.text, "Ciao mondo\n");
    }

    #[test]
    fn extracts_the_text_of_a_docx() {
        let extracted = extract_text(&build_docx(&["Offerta"]), None, &Limits::default()).unwrap();
        assert_eq!(extracted.format, DocumentFormat::Docx);
        assert_eq!(extracted.text, "Offerta\n");
        assert!(!extracted.truncated);
    }

    #[test]
    fn extracts_a_spreadsheet_as_named_tab_separated_lines() {
        let text = extract_text(&build_xlsx(), None, &Limits::default())
            .unwrap()
            .text;
        assert!(text.starts_with("Vendite\nMese\tRicavi\nGennaio\t1200.5\n"));
        assert!(text.contains("Clienti\nCliente\tCittà\nAcme\tMilano\n"));
    }

    #[test]
    fn skips_empty_rows_of_a_csv() {
        let text = extract_text(b"a,b\n,\nc,d\n", Some("x.csv"), &Limits::default()).unwrap();
        assert_eq!(text.text, "a\tb\nc\td\n");
    }

    #[test]
    fn stops_a_spreadsheet_at_the_character_limit() {
        let limits = Limits {
            max_text_chars: 5,
            ..Limits::default()
        };
        let extracted = extract_text(b"abcdef,g\n", Some("x.csv"), &limits).unwrap();
        assert_eq!(extracted.text, "abcde");
        assert!(extracted.truncated);
    }

    #[test]
    fn is_not_truncated_when_the_text_fills_the_limit_exactly() {
        let limits = Limits {
            max_text_chars: 4,
            ..Limits::default()
        };
        let extracted = extract_text(b"abc\n", Some("x.csv"), &limits).unwrap();
        assert_eq!(extracted.text, "abc\n");
        assert!(!extracted.truncated);
    }

    #[test]
    fn refuses_a_file_that_is_not_an_office_document() {
        let error = extract_text(b"plain", Some("a.txt"), &Limits::default()).unwrap_err();
        assert!(matches!(
            error,
            OfficeError::UnsupportedFormat(DocumentFormat::Unknown)
        ));
    }

    #[test]
    fn refuses_an_input_above_the_limit() {
        let limits = Limits {
            max_input_bytes: 3,
            ..Limits::default()
        };
        let error = extract_text(b"a,b,c", Some("a.csv"), &limits).unwrap_err();
        assert!(matches!(error, OfficeError::TooLarge { .. }));
    }

    #[test]
    fn counts_characters_not_bytes() {
        let mut buffer = TextBuffer::new(2);
        buffer.push_str("\u{e0}\u{e8}");
        assert!(!buffer.is_truncated());
        buffer.push('x');
        assert!(buffer.is_truncated());
        assert_eq!(buffer.into_text(), "\u{e0}\u{e8}");
    }
}
