// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use std::io::{Cursor, Read};

/// What a document is, decided from its bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocumentFormat {
    /// Word 2007+ (`.docx`, `.docm`).
    Docx,
    /// Excel 2007+ (`.xlsx`, `.xlsm`).
    Xlsx,
    /// `PowerPoint` 2007+ (`.pptx`, `.pptm`).
    Pptx,
    /// `OpenDocument` text (`.odt`).
    Odt,
    /// `OpenDocument` spreadsheet (`.ods`).
    Ods,
    /// `OpenDocument` presentation (`.odp`).
    Odp,
    /// Word 97-2003 (`.doc`), a compound file holding a `WordDocument` stream.
    Doc,
    /// Excel 97-2003 (`.xls`), a compound file holding a `Workbook` or `Book` stream.
    Xls,
    /// `PowerPoint` 97-2003 (`.ppt`).
    Ppt,
    /// Comma separated values.
    Csv,
    /// Tab separated values.
    Tsv,
    /// Markdown text (`.md`, `.markdown`).
    Markdown,
    /// An HTML page (`.html`, `.htm`).
    Html,
    /// Anything else.
    Unknown,
}

impl DocumentFormat {
    /// The lowercase name of the format, as its usual file extension spells it (`unknown` for
    /// anything else): the stable value a caller across a language boundary compares against.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Docx => "docx",
            Self::Xlsx => "xlsx",
            Self::Pptx => "pptx",
            Self::Odt => "odt",
            Self::Ods => "ods",
            Self::Odp => "odp",
            Self::Doc => "doc",
            Self::Xls => "xls",
            Self::Ppt => "ppt",
            Self::Csv => "csv",
            Self::Tsv => "tsv",
            Self::Markdown => "md",
            Self::Html => "html",
            Self::Unknown => "unknown",
        }
    }
}

const ZIP_SIGNATURE: &[u8] = b"PK\x03\x04";
const COMPOUND_FILE_SIGNATURE: &[u8] = &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// Tell what a document is from its bytes; the file name only names the text formats, which
/// carry no signature (CSV, TSV, Markdown, and HTML without its doctype), and never overrides
/// what the bytes say.
#[must_use]
pub fn detect_format(bytes: &[u8], file_name: Option<&str>) -> DocumentFormat {
    if bytes.starts_with(ZIP_SIGNATURE) {
        return detect_zip_format(bytes);
    }
    if bytes.starts_with(COMPOUND_FILE_SIGNATURE) {
        return detect_compound_file_format(bytes);
    }

    let extension = file_name
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, extension)| extension.to_ascii_lowercase());
    match extension.as_deref() {
        Some("csv") if looks_like_text(bytes) => DocumentFormat::Csv,
        Some("tsv" | "tab") if looks_like_text(bytes) => DocumentFormat::Tsv,
        Some("md" | "markdown" | "mdown" | "mkd") if looks_like_text(bytes) => {
            DocumentFormat::Markdown
        }
        Some("html" | "htm" | "xhtml") if looks_like_text(bytes) => DocumentFormat::Html,
        _ if looks_like_html(bytes) => DocumentFormat::Html,
        _ => DocumentFormat::Unknown,
    }
}

/// A page that says it is HTML at its start: a doctype or an `html` element.
fn looks_like_html(bytes: &[u8]) -> bool {
    let start = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let start: Vec<u8> = start
        .iter()
        .skip_while(|byte| byte.is_ascii_whitespace())
        .take(15)
        .map(u8::to_ascii_lowercase)
        .collect();
    start.starts_with(b"<!doctype html") || start.starts_with(b"<html")
}

/// An OOXML package names its main part in `[Content_Types].xml`; an `OpenDocument` one starts
/// with an uncompressed `mimetype` member.
fn detect_zip_format(bytes: &[u8]) -> DocumentFormat {
    let Ok(mut archive) = zip::ZipArchive::new(Cursor::new(bytes)) else {
        return DocumentFormat::Unknown;
    };

    if let Some(mimetype) = read_small_member(&mut archive, "mimetype") {
        return match mimetype.trim() {
            "application/vnd.oasis.opendocument.text" => DocumentFormat::Odt,
            "application/vnd.oasis.opendocument.spreadsheet" => DocumentFormat::Ods,
            "application/vnd.oasis.opendocument.presentation" => DocumentFormat::Odp,
            _ => DocumentFormat::Unknown,
        };
    }

    let Some(content_types) = read_small_member(&mut archive, "[Content_Types].xml") else {
        return DocumentFormat::Unknown;
    };
    if content_types.contains("wordprocessingml.document.main")
        || content_types.contains("ms-word.document.macroEnabled.main")
    {
        DocumentFormat::Docx
    } else if content_types.contains("spreadsheetml.sheet.main")
        || content_types.contains("ms-excel.sheet.macroEnabled.main")
    {
        DocumentFormat::Xlsx
    } else if content_types.contains("presentationml.presentation.main")
        || content_types.contains("ms-powerpoint.presentation.macroEnabled.main")
    {
        DocumentFormat::Pptx
    } else {
        DocumentFormat::Unknown
    }
}

fn detect_compound_file_format(bytes: &[u8]) -> DocumentFormat {
    let Ok(compound_file) = cfb::CompoundFile::open(Cursor::new(bytes)) else {
        return DocumentFormat::Unknown;
    };
    if compound_file.exists("/WordDocument") {
        DocumentFormat::Doc
    } else if compound_file.exists("/Workbook") || compound_file.exists("/Book") {
        DocumentFormat::Xls
    } else if compound_file.exists("/PowerPoint Document") {
        DocumentFormat::Ppt
    } else {
        DocumentFormat::Unknown
    }
}

/// A member small enough to hold a type declaration; anything larger is not one.
fn read_small_member(archive: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Option<String> {
    const MAX_DECLARATION_BYTES: u64 = 256 * 1024;
    let member = archive.by_name(name).ok()?;
    let mut text = String::new();
    member
        .take(MAX_DECLARATION_BYTES)
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}

/// Text has no NUL byte in its first kilobytes; a binary file almost always does.
fn looks_like_text(bytes: &[u8]) -> bool {
    !bytes.iter().take(8 * 1024).any(|&byte| byte == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        build_doc, build_docx, build_odp, build_ods, build_odt, build_ppt, build_pptx, build_xlsx,
    };

    #[test]
    fn detects_a_docx_from_its_content_types() {
        assert_eq!(
            detect_format(&build_docx(&["Ciao"]), Some("a.bin")),
            DocumentFormat::Docx
        );
    }

    #[test]
    fn detects_an_xlsx_from_its_content_types() {
        assert_eq!(detect_format(&build_xlsx(), None), DocumentFormat::Xlsx);
    }

    #[test]
    fn detects_an_ods_from_its_mimetype_member() {
        assert_eq!(
            detect_format(&build_ods(), Some("x.zip")),
            DocumentFormat::Ods
        );
    }

    #[test]
    fn detects_a_pptx_from_its_content_types() {
        assert_eq!(
            detect_format(&build_pptx(&[&["Titolo"]], &[]), Some("a.docx")),
            DocumentFormat::Pptx
        );
    }

    #[test]
    fn detects_odt_and_odp_from_their_mimetype_member() {
        assert_eq!(
            detect_format(&build_odt("<text:p>Ciao</text:p>"), None),
            DocumentFormat::Odt
        );
        assert_eq!(
            detect_format(&build_odp(&["Ciao"]), None),
            DocumentFormat::Odp
        );
    }

    #[test]
    fn tells_compound_files_apart_by_their_streams() {
        assert_eq!(
            detect_format(&build_doc(&[("Ciao\r", true)]), Some("a.xls")),
            DocumentFormat::Doc
        );
        assert_eq!(
            detect_format(&build_ppt(&["Ciao"]), None),
            DocumentFormat::Ppt
        );
    }

    #[test]
    fn names_every_format_in_lowercase() {
        assert_eq!(DocumentFormat::Docx.as_str(), "docx");
        assert_eq!(DocumentFormat::Ppt.as_str(), "ppt");
        assert_eq!(DocumentFormat::Unknown.as_str(), "unknown");
    }

    #[test]
    fn tells_csv_from_tsv_by_name_only_for_text() {
        assert_eq!(
            detect_format(b"a;b\n1;2\n", Some("data.CSV")),
            DocumentFormat::Csv
        );
        assert_eq!(
            detect_format(b"a\tb\n", Some("data.tsv")),
            DocumentFormat::Tsv
        );
        assert_eq!(
            detect_format(b"a\0b", Some("data.csv")),
            DocumentFormat::Unknown
        );
    }

    #[test]
    fn names_markdown_and_html_by_their_extension_or_doctype() {
        assert_eq!(
            detect_format(b"# Titolo", Some("nota.MD")),
            DocumentFormat::Markdown
        );
        assert_eq!(
            detect_format(b"# Titolo", Some("nota.markdown")),
            DocumentFormat::Markdown
        );
        assert_eq!(detect_format(b"# Titolo", None), DocumentFormat::Unknown);
        assert_eq!(
            detect_format(b"<p>x</p>", Some("pagina.htm")),
            DocumentFormat::Html
        );
        assert_eq!(
            detect_format(b"\xEF\xBB\xBF  <!DOCTYPE html><p>x</p>", None),
            DocumentFormat::Html
        );
        assert_eq!(detect_format(b"<HTML><body>x", None), DocumentFormat::Html);
        assert_eq!(DocumentFormat::Markdown.as_str(), "md");
        assert_eq!(DocumentFormat::Html.as_str(), "html");
    }

    #[test]
    fn never_trusts_a_name_over_the_bytes() {
        assert_eq!(
            detect_format(&build_xlsx(), Some("report.docx")),
            DocumentFormat::Xlsx
        );
        assert_eq!(
            detect_format(b"plain", Some("report.docx")),
            DocumentFormat::Unknown
        );
    }

    #[test]
    fn answers_unknown_for_a_damaged_container() {
        assert_eq!(
            detect_format(b"PK\x03\x04garbage", None),
            DocumentFormat::Unknown
        );
        assert_eq!(
            detect_format(COMPOUND_FILE_SIGNATURE, None),
            DocumentFormat::Unknown
        );
    }
}
