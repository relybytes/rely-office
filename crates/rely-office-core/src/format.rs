// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use std::io::{Cursor, Read};

/// What a document is, decided from its bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocumentFormat {
    /// Word 2007+ (`.docx`).
    Docx,
    /// Excel 2007+ (`.xlsx`, `.xlsm`).
    Xlsx,
    /// `PowerPoint` 2007+ (`.pptx`).
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
    /// Anything else.
    Unknown,
}

const ZIP_SIGNATURE: &[u8] = b"PK\x03\x04";
const COMPOUND_FILE_SIGNATURE: &[u8] = &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// Tell what a document is from its bytes; the file name only separates CSV from TSV, which carry
/// no signature, and never overrides what the bytes say.
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
        _ => DocumentFormat::Unknown,
    }
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
    if content_types.contains("wordprocessingml.document.main") {
        DocumentFormat::Docx
    } else if content_types.contains("spreadsheetml.sheet.main")
        || content_types.contains("ms-excel.sheet.macroEnabled.main")
    {
        DocumentFormat::Xlsx
    } else if content_types.contains("presentationml.presentation.main") {
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
    use crate::test_support::{build_docx, build_ods, build_xlsx};

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
