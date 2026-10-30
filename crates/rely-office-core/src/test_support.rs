// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! Documents built in memory for the tests, so the repository carries no binary fixture.

use std::fmt::Write as _;
use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;

const WORD_NAMESPACE: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const RELATIONSHIP_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const DOCX_CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;

/// The namespaces a document part of the tests declares: every prefix Word itself writes.
const DOCX_NAMESPACES: &str = concat!(
    r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" "#,
    r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#,
    r#"xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" "#,
    r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
    r#"xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture" "#,
    r#"xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" "#,
    r#"xmlns:wpg="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup" "#,
    r#"xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" "#,
    r#"xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math" "#,
    r#"xmlns:v="urn:schemas-microsoft-com:vml" "#,
    r#"xmlns:o="urn:schemas-microsoft-com:office:office""#,
);

/// A zip holding exactly these members, deflated.
pub(crate) fn build_zip(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content) in members {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(content).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

/// A relationship part: `(id, type name, target, external)`.
pub(crate) fn rels_xml(relationships: &[(&str, &str, &str, bool)]) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for (id, kind, target, external) in relationships {
        let mode = if *external {
            r#" TargetMode="External""#
        } else {
            ""
        };
        let _ = write!(
            xml,
            r#"<Relationship Id="{id}" Type="{RELATIONSHIP_TYPE}/{kind}" Target="{target}"{mode}/>"#
        );
    }
    xml.push_str("</Relationships>");
    xml
}

/// A minimal docx: content types and a body with one paragraph per entry (already XML-escaped).
pub(crate) fn build_docx(paragraphs: &[&str]) -> Vec<u8> {
    build_docx_with_parts(paragraphs, &[])
}

/// A docx with extra parts, each holding one paragraph: headers, footers, notes.
pub(crate) fn build_docx_with_parts(paragraphs: &[&str], parts: &[(&str, &str)]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default();
    writer.start_file("[Content_Types].xml", options).unwrap();
    writer.write_all(DOCX_CONTENT_TYPES.as_bytes()).unwrap();
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

/// A docx whose body is `body` (raw `WordprocessingML`, every usual prefix declared), with extra
/// members such as `word/styles.xml`, `word/numbering.xml`, `word/_rels/document.xml.rels` or
/// images under `word/media/`.
pub(crate) fn build_docx_from_body(body: &str, extra: &[(&str, &[u8])]) -> Vec<u8> {
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document {DOCX_NAMESPACES}><w:body>{body}<w:sectPr/></w:body></w:document>"#
    );
    let mut members: Vec<(&str, &[u8])> = vec![
        ("[Content_Types].xml", DOCX_CONTENT_TYPES.as_bytes()),
        ("word/document.xml", document.as_bytes()),
    ];
    members.extend_from_slice(extra);
    build_zip(&members)
}

/// A styles part declaring paragraph styles `(id, name, extra XML inside w:style)`.
pub(crate) fn styles_xml(styles: &[(&str, &str, &str)]) -> String {
    let mut xml =
        format!(r#"<?xml version="1.0" encoding="UTF-8"?><w:styles xmlns:w="{WORD_NAMESPACE}">"#);
    for (id, name, properties) in styles {
        let _ = write!(
            xml,
            r#"<w:style w:type="paragraph" w:styleId="{id}"><w:name w:val="{name}"/>{properties}</w:style>"#
        );
    }
    xml.push_str("</w:styles>");
    xml
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
        let name = member.name().unwrap().into_owned();
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

/// A pptx with one slide per entry, each holding one paragraph per text, and speaker notes for
/// the slides named by index.
pub(crate) fn build_pptx(slides: &[&[&str]], notes: &[(usize, &str)]) -> Vec<u8> {
    let bodies: Vec<String> = slides
        .iter()
        .map(|paragraphs| drawing_paragraphs(paragraphs))
        .collect();
    let order: Vec<usize> = (1..=slides.len()).collect();
    pptx_package(&bodies, &order, notes)
}

/// A pptx whose `slideN.xml` holds `texts[N - 1]` and whose slide list follows `order`
/// (1-based slide numbers).
pub(crate) fn build_pptx_in_order(texts: &[&str], order: &[usize]) -> Vec<u8> {
    let bodies: Vec<String> = texts
        .iter()
        .map(|text| drawing_paragraphs(&[text]))
        .collect();
    pptx_package(&bodies, order, &[])
}

/// A pptx with one slide per raw `DrawingML` paragraph sequence.
pub(crate) fn build_pptx_from_paragraph_xml(slides: &[&str]) -> Vec<u8> {
    let bodies: Vec<String> = slides.iter().map(|xml| (*xml).to_owned()).collect();
    let order: Vec<usize> = (1..=slides.len()).collect();
    pptx_package(&bodies, &order, &[])
}

fn drawing_paragraphs(paragraphs: &[&str]) -> String {
    let mut xml = String::new();
    for text in paragraphs {
        let _ = write!(xml, "<a:p><a:r><a:t>{text}</a:t></a:r></a:p>");
    }
    xml
}

fn presentation_part(root: &str, body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><p:{root} xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">{body}</p:{root}>"#
    )
}

fn slide_part(root: &str, paragraphs: &str) -> String {
    presentation_part(
        root,
        &format!(
            "<p:cSld><p:spTree><p:sp><p:txBody><a:bodyPr/>{paragraphs}</p:txBody></p:sp></p:spTree></p:cSld>"
        ),
    )
}

fn pptx_package(bodies: &[String], order: &[usize], notes: &[(usize, &str)]) -> Vec<u8> {
    let mut owned: Vec<(String, Vec<u8>)> = vec![
        (
            "[Content_Types].xml".to_owned(),
            br#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/></Types>"#.to_vec(),
        ),
        (
            "_rels/.rels".to_owned(),
            rels_xml(&[("rId1", "officeDocument", "ppt/presentation.xml", false)]).into_bytes(),
        ),
    ];

    let mut slide_list = String::new();
    for (position, number) in order.iter().enumerate() {
        let _ = write!(
            slide_list,
            r#"<p:sldId id="{}" r:id="rId{number}"/>"#,
            256 + position
        );
    }
    owned.push((
        "ppt/presentation.xml".to_owned(),
        presentation_part(
            "presentation",
            &format!("<p:sldIdLst>{slide_list}</p:sldIdLst>"),
        )
        .into_bytes(),
    ));
    let targets: Vec<(String, String)> = (1..=bodies.len())
        .map(|number| (format!("rId{number}"), format!("slides/slide{number}.xml")))
        .collect();
    let target_refs: Vec<(&str, &str, &str, bool)> = targets
        .iter()
        .map(|(id, target)| (id.as_str(), "slide", target.as_str(), false))
        .collect();
    owned.push((
        "ppt/_rels/presentation.xml.rels".to_owned(),
        rels_xml(&target_refs).into_bytes(),
    ));

    for (index, body) in bodies.iter().enumerate() {
        let number = index + 1;
        owned.push((
            format!("ppt/slides/slide{number}.xml"),
            slide_part("sld", body).into_bytes(),
        ));
        if let Some((_, note)) = notes.iter().find(|(slide_index, _)| *slide_index == index) {
            let target = format!("../notesSlides/notesSlide{number}.xml");
            owned.push((
                format!("ppt/slides/_rels/slide{number}.xml.rels"),
                rels_xml(&[("rId1", "notesSlide", &target, false)]).into_bytes(),
            ));
            let notes_body = format!(
                r#"<a:p><a:fld id="n" type="slidenum"><a:t>{number}</a:t></a:fld></a:p><a:p><a:r><a:t>{note}</a:t></a:r></a:p>"#
            );
            owned.push((
                format!("ppt/notesSlides/notesSlide{number}.xml"),
                slide_part("notes", &notes_body).into_bytes(),
            ));
        }
    }

    let members: Vec<(&str, &[u8])> = owned
        .iter()
        .map(|(name, content)| (name.as_str(), content.as_slice()))
        .collect();
    build_zip(&members)
}

/// An `OpenDocument` package: a stored `mimetype` first, then `content.xml` wrapping `body` in
/// `office:body`, and `styles.xml` wrapping `master_styles` in `office:master-styles`.
pub(crate) fn build_opendocument(mimetype: &str, body: &str, master_styles: &str) -> Vec<u8> {
    const NAMESPACES: &str = r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:presentation="urn:oasis:names:tc:opendocument:xmlns:presentation:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0""#;
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    writer.start_file("mimetype", stored).unwrap();
    writer.write_all(mimetype.as_bytes()).unwrap();
    writer
        .start_file("content.xml", SimpleFileOptions::default())
        .unwrap();
    writer
        .write_all(
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content {NAMESPACES} office:version="1.2"><office:body>{body}</office:body></office:document-content>"#
            )
            .as_bytes(),
        )
        .unwrap();
    writer
        .start_file("styles.xml", SimpleFileOptions::default())
        .unwrap();
    writer
        .write_all(
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><office:document-styles {NAMESPACES} office:version="1.2"><office:master-styles>{master_styles}</office:master-styles></office:document-styles>"#
            )
            .as_bytes(),
        )
        .unwrap();
    writer.finish().unwrap().into_inner()
}

/// An odt whose `office:text` holds `text_body`.
pub(crate) fn build_odt(text_body: &str) -> Vec<u8> {
    build_opendocument(
        "application/vnd.oasis.opendocument.text",
        &format!("<office:text>{text_body}</office:text>"),
        "",
    )
}

/// An odp with one page per entry, each holding one text frame with that paragraph.
pub(crate) fn build_odp(pages: &[&str]) -> Vec<u8> {
    let mut body = String::from("<office:presentation>");
    for (index, text) in pages.iter().enumerate() {
        let _ = write!(
            body,
            r#"<draw:page draw:name="page{index}"><draw:frame><draw:text-box><text:p>{text}</text:p></draw:text-box></draw:frame></draw:page>"#
        );
    }
    body.push_str("</office:presentation>");
    build_opendocument("application/vnd.oasis.opendocument.presentation", &body, "")
}

/// Options of [`build_doc_with`].
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DocOptions {
    pub encrypted: bool,
    /// Put the piece table in `0Table` instead of `1Table`.
    pub zero_table: bool,
    /// Point the piece table past the end of the table stream.
    pub broken_clx: bool,
}

/// A Word 97-2003 file whose text is `pieces`: `(text, compressed)`, a compressed piece written as
/// Windows-1252 bytes, the others as UTF-16LE; paragraphs end with `\r`.
pub(crate) fn build_doc(pieces: &[(&str, bool)]) -> Vec<u8> {
    build_doc_with(pieces, DocOptions::default())
}

pub(crate) fn build_doc_with(pieces: &[(&str, bool)], options: DocOptions) -> Vec<u8> {
    const TEXT_START: usize = 1024;
    const FC_CLX: usize = 154 + 33 * 8;
    fn put_u16(buffer: &mut [u8], offset: usize, value: u16) {
        buffer[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }
    fn put_u32(buffer: &mut [u8], offset: usize, value: usize) {
        buffer[offset..offset + 4].copy_from_slice(&u32::try_from(value).unwrap().to_le_bytes());
    }

    let mut word_document = vec![0_u8; TEXT_START];
    put_u16(&mut word_document, 0, 0xA5EC);
    put_u16(&mut word_document, 2, 0x00C1);
    let mut flags = 0x0004_u16;
    if !options.zero_table {
        flags |= 0x0200;
    }
    if options.encrypted {
        flags |= 0x0100;
    }
    put_u16(&mut word_document, 10, flags);
    put_u16(&mut word_document, 32, 14);
    put_u16(&mut word_document, 62, 22);
    put_u16(&mut word_document, 152, 0x5D);

    let mut character_positions = vec![0_usize];
    let mut descriptors = Vec::new();
    for (text, compressed) in pieces {
        let offset = word_document.len();
        let length = if *compressed {
            let (bytes, _, _) = encoding_rs::WINDOWS_1252.encode(text);
            word_document.extend_from_slice(&bytes);
            descriptors.push((offset * 2) | 0x4000_0000);
            bytes.len()
        } else {
            let units: Vec<u16> = text.encode_utf16().collect();
            for unit in &units {
                word_document.extend_from_slice(&unit.to_le_bytes());
            }
            descriptors.push(offset);
            units.len()
        };
        let last = *character_positions.last().unwrap();
        character_positions.push(last + length);
    }
    put_u32(
        &mut word_document,
        64 + 12,
        *character_positions.last().unwrap(),
    );

    let mut table = vec![0_u8; 16];
    let clx_offset = table.len();
    table.extend_from_slice(&[0x01, 0x02, 0x00, 0xAA, 0xBB]);
    table.push(0x02);
    let plc_length = character_positions.len() * 4 + descriptors.len() * 8;
    table.extend_from_slice(&u32::try_from(plc_length).unwrap().to_le_bytes());
    for position in &character_positions {
        table.extend_from_slice(&u32::try_from(*position).unwrap().to_le_bytes());
    }
    for descriptor in &descriptors {
        table.extend_from_slice(&0_u16.to_le_bytes());
        table.extend_from_slice(&u32::try_from(*descriptor).unwrap().to_le_bytes());
        table.extend_from_slice(&0_u16.to_le_bytes());
    }
    let clx_length = table.len() - clx_offset;
    let clx_offset = if options.broken_clx {
        table.len() + 100
    } else {
        clx_offset
    };
    put_u32(&mut word_document, FC_CLX, clx_offset);
    put_u32(&mut word_document, FC_CLX + 4, clx_length);

    let table_name = if options.zero_table {
        "/0Table"
    } else {
        "/1Table"
    };
    build_compound_file(&[("/WordDocument", &word_document), (table_name, &table)])
}

/// A compound file holding these streams.
pub(crate) fn build_compound_file(streams: &[(&str, &[u8])]) -> Vec<u8> {
    // Version 3, 512-byte sectors, as Office writes them.
    let mut compound_file =
        cfb::CompoundFile::create_with_version(cfb::Version::V3, Cursor::new(Vec::new())).unwrap();
    for (name, content) in streams {
        let mut parent = std::path::PathBuf::from("/");
        let components: Vec<_> = std::path::Path::new(name).components().skip(1).collect();
        for component in components.iter().take(components.len().saturating_sub(1)) {
            parent.push(component);
            if !compound_file.is_storage(&parent) {
                compound_file.create_storage(&parent).unwrap();
            }
        }
        let mut stream = compound_file.create_stream(name).unwrap();
        stream.write_all(content).unwrap();
    }
    compound_file.flush().unwrap();
    compound_file.into_inner().into_inner()
}

/// The text of a member of a zip package.
pub(crate) fn zip_member(package: &[u8], name: &str) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(package)).unwrap();
    let mut text = String::new();
    std::io::Read::read_to_string(&mut archive.by_name(name).unwrap(), &mut text).unwrap();
    text
}

/// A `PowerPoint` record header followed by its body.
pub(crate) fn ppt_record(version: u16, instance: u16, kind: u16, body: &[u8]) -> Vec<u8> {
    let mut record = Vec::with_capacity(8 + body.len());
    record.extend_from_slice(&(version | (instance << 4)).to_le_bytes());
    record.extend_from_slice(&kind.to_le_bytes());
    record.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
    record.extend_from_slice(body);
    record
}

/// A `PowerPoint` 97-2003 file whose slide list holds `texts`, alternately as a `TextBytesAtom`
/// (even index, Latin-1) and a `TextCharsAtom` (odd index, UTF-16LE), plus a master slide whose
/// placeholder text must not be read.
pub(crate) fn build_ppt(texts: &[&str]) -> Vec<u8> {
    let mut slide_list = Vec::new();
    for (index, text) in texts.iter().enumerate() {
        slide_list.extend(ppt_record(0, 0, 0x0F9F, &1_u32.to_le_bytes()));
        if index % 2 == 0 {
            let bytes: Vec<u8> = text
                .chars()
                .map(|character| u8::try_from(u32::from(character)).unwrap())
                .collect();
            slide_list.extend(ppt_record(0, 0, 0x0FA8, &bytes));
        } else {
            let bytes: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
            slide_list.extend(ppt_record(0, 0, 0x0FA0, &bytes));
        }
    }
    let master_text: Vec<u8> = "Fare clic per modificare"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let master = ppt_record(0xF, 0, 0x03F8, &ppt_record(0, 0, 0x0FA0, &master_text));
    let mut document_body = ppt_record(0xF, 0, 0x0FF0, &slide_list);
    document_body.extend(master);
    let stream = ppt_record(0xF, 0, 0x03E8, &document_body);
    build_ppt_from_stream(&stream)
}

/// A `PowerPoint` 97-2003 file whose `PowerPoint Document` stream is exactly `stream`.
pub(crate) fn build_ppt_from_stream(stream: &[u8]) -> Vec<u8> {
    build_compound_file(&[("/PowerPoint Document", stream)])
}
