// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Documents built in memory for the tests, so the repository carries no binary fixture.

use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;

const WORD_NAMESPACE: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// A minimal docx: content types and a body with one paragraph per entry (already XML-escaped).
pub(crate) fn build_docx(paragraphs: &[&str]) -> Vec<u8> {
    build_docx_with_parts(paragraphs, &[])
}

/// A docx with extra parts, each holding one paragraph: headers, footers, notes.
pub(crate) fn build_docx_with_parts(paragraphs: &[&str], parts: &[(&str, &str)]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default();
    writer.start_file("[Content_Types].xml", options).unwrap();
    writer
        .write_all(
            br#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        )
        .unwrap();
    writer.start_file("word/document.xml", options).unwrap();
    writer
        .write_all(word_xml("document", "body", paragraphs).as_bytes())
        .unwrap();
    for (name, paragraph) in parts {
        writer.start_file(*name, options).unwrap();
        writer
            .write_all(word_xml("hdr", "", &[paragraph]).as_bytes())
            .unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn word_xml(root: &str, container: &str, paragraphs: &[&str]) -> String {
    let mut body = String::new();
    for paragraph in paragraphs {
        body.push_str("<w:p><w:r><w:t>");
        body.push_str(paragraph);
        body.push_str("</w:t></w:r></w:p>");
    }
    let body = if container.is_empty() {
        body
    } else {
        format!("<w:{container}>{body}</w:{container}>")
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:{root} xmlns:w="{WORD_NAMESPACE}">{body}</w:{root}>"#
    )
}

/// An xlsx with two sheets: "Vendite" (header and two rows) and "Clienti".
pub(crate) fn build_xlsx() -> Vec<u8> {
    let mut workbook = rust_xlsxwriter::Workbook::new();
    let sales = workbook.add_worksheet().set_name("Vendite").unwrap();
    sales.write(0, 0, "Mese").unwrap();
    sales.write(0, 1, "Ricavi").unwrap();
    sales.write(1, 0, "Gennaio").unwrap();
    sales.write(1, 1, 1200.5).unwrap();
    sales.write(2, 0, "Febbraio").unwrap();
    sales.write(2, 1, 1500).unwrap();
    let customers = workbook.add_worksheet().set_name("Clienti").unwrap();
    customers.write(0, 0, "Cliente").unwrap();
    customers.write(0, 1, "Città").unwrap();
    customers.write(1, 0, "Acme").unwrap();
    customers.write(1, 1, "Milano").unwrap();
    workbook.save_to_buffer().unwrap()
}

/// A minimal ods with one sheet whose first cell reads "Cliente".
pub(crate) fn build_ods() -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    writer.start_file("mimetype", stored).unwrap();
    writer
        .write_all(b"application/vnd.oasis.opendocument.spreadsheet")
        .unwrap();
    writer.start_file("META-INF/manifest.xml", stored).unwrap();
    writer
        .write_all(
            br#"<?xml version="1.0" encoding="UTF-8"?><manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.2"><manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/><manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/></manifest:manifest>"#,
        )
        .unwrap();
    writer.start_file("content.xml", stored).unwrap();
    writer
        .write_all(
            br#"<?xml version="1.0" encoding="UTF-8"?><office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" office:version="1.2"><office:body><office:spreadsheet><table:table table:name="Foglio1"><table:table-row><table:table-cell office:value-type="string"><text:p>Cliente</text:p></table:table-cell><table:table-cell office:value-type="string"><text:p>Arona</text:p></table:table-cell></table:table-row></table:table></office:spreadsheet></office:body></office:document-content>"#,
        )
        .unwrap();
    writer.finish().unwrap().into_inner()
}

/// An xlsx whose C1 holds `=A1+B1` with no cached value, as libraries such as openpyxl write it.
pub(crate) fn build_xlsx_with_uncached_formula() -> Vec<u8> {
    let mut workbook = rust_xlsxwriter::Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet.write(0, 0, 2).unwrap();
    sheet.write(0, 1, 3).unwrap();
    sheet.write_formula(0, 2, "=A1+B1").unwrap();
    let original = workbook.save_to_buffer().unwrap();

    let mut source = zip::ZipArchive::new(Cursor::new(original)).unwrap();
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..source.len() {
        let mut member = source.by_index(index).unwrap();
        let name = member.name().to_owned();
        let mut content = Vec::new();
        std::io::Read::read_to_end(&mut member, &mut content).unwrap();
        if name == "xl/worksheets/sheet1.xml" {
            content = String::from_utf8(content)
                .unwrap()
                .replace("<v>0</v>", "")
                .into_bytes();
        }
        writer
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&content).unwrap();
    }
    writer.finish().unwrap().into_inner()
}
