// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::compound::open_compound_file;
use crate::document::{Block, DocumentTree, Inline, write_docx};
use crate::test_support::{build_compound_file, build_docx_from_body, build_zip, rels_xml};
use crate::{Limits, read_document_tree};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nanteprima";

/// A `CompObj` stream: the header, a user type, a clipboard format and a `ProgID`.
fn comp_obj(clipboard: &[u8], program: &str) -> Vec<u8> {
    let mut stream = vec![0_u8; COMP_OBJ_HEADER];
    let user_type = b"Foglio di lavoro\0";
    stream.extend_from_slice(&u32::try_from(user_type.len()).unwrap().to_le_bytes());
    stream.extend_from_slice(user_type);
    stream.extend_from_slice(clipboard);
    let mut program = program.as_bytes().to_vec();
    program.push(0);
    stream.extend_from_slice(&u32::try_from(program.len()).unwrap().to_le_bytes());
    stream.extend(program);
    stream
}

fn ole_object(program: &str) -> Vec<u8> {
    build_compound_file(&[
        ("/\u{1}CompObj", &comp_obj(&[0, 0, 0, 0], program)),
        ("/Workbook", b"righe del foglio"),
    ])
}

fn object(bytes: Vec<u8>, name: &str) -> EmbeddedObject {
    EmbeddedObject {
        media: 0,
        program: None,
        name: name.to_owned(),
        bytes,
    }
}

#[test]
fn program_of_reads_past_each_clipboard_format() {
    assert_eq!(
        program_of(&comp_obj(&[0, 0, 0, 0], "Excel.Sheet.8")).as_deref(),
        Some("Excel.Sheet.8")
    );
    let mut standard = 0xFFFF_FFFF_u32.to_le_bytes().to_vec();
    standard.extend_from_slice(&3_u32.to_le_bytes());
    assert_eq!(
        program_of(&comp_obj(&standard, "Word.Document.8")).as_deref(),
        Some("Word.Document.8")
    );
    let mut named = 7_u32.to_le_bytes().to_vec();
    named.extend_from_slice(b"Biff8\0\0");
    assert_eq!(
        program_of(&comp_obj(&named, "Package")).as_deref(),
        Some("Package")
    );
}

#[test]
fn program_of_refuses_what_is_not_a_prog_id() {
    assert_eq!(program_of(&comp_obj(&[0, 0, 0, 0], "")), None);
    assert_eq!(program_of(&comp_obj(&[0, 0, 0, 0], "Excel Sheet")), None);
    assert_eq!(program_of(&comp_obj(&[0, 0, 0, 0], &"A".repeat(200))), None);
    assert_eq!(program_of(&[0; 10]), None);
    assert_eq!(program_of(&comp_obj(&[0xF0, 0xFF, 0xFF, 0x0F], "X")), None);
}

#[test]
fn program_of_compound_file_reads_its_comp_obj() {
    assert_eq!(
        program_of_compound_file(&ole_object("Excel.Sheet.8")).as_deref(),
        Some("Excel.Sheet.8")
    );
    assert_eq!(program_of_compound_file(b"PK\x03\x04"), None);
    assert_eq!(
        program_of_compound_file(&build_compound_file(&[("/Workbook", b"x")])),
        None
    );
}

#[test]
fn a_storage_is_copied_as_a_compound_file_of_its_own() {
    let source = build_compound_file(&[
        (
            "/ObjectPool/_1234/\u{1}CompObj",
            &comp_obj(&[0, 0, 0, 0], "Excel.Sheet.8"),
        ),
        ("/ObjectPool/_1234/Workbook", b"righe"),
        ("/ObjectPool/_1234/Nested/Dati", b"annidati"),
        ("/WordDocument", b"testo"),
    ]);
    let mut compound_file = open_compound_file(&source).unwrap();
    let copy = storage_as_compound_file(&mut compound_file, "/ObjectPool/_1234", 1_000).unwrap();
    assert_eq!(
        program_of_compound_file(&copy).as_deref(),
        Some("Excel.Sheet.8")
    );
    let mut copied = open_compound_file(&copy).unwrap();
    assert_eq!(
        read_stream(&mut copied, "/Workbook", 100).unwrap().unwrap(),
        b"righe"
    );
    assert_eq!(
        read_stream(&mut copied, "/Nested/Dati", 100)
            .unwrap()
            .unwrap(),
        b"annidati"
    );
    assert!(!copied.is_stream("/WordDocument"));
}

#[test]
fn a_storage_past_its_cap_or_missing_is_not_copied() {
    let source = build_compound_file(&[
        ("/ObjectPool/_1/Workbook", &[7_u8; 600]),
        ("/WordDocument", b"testo"),
    ]);
    let mut compound_file = open_compound_file(&source).unwrap();
    assert_eq!(
        storage_as_compound_file(&mut compound_file, "/ObjectPool/_1", 100),
        None
    );
    assert_eq!(
        storage_as_compound_file(&mut compound_file, "/ObjectPool/_2", 1_000),
        None
    );
    assert_eq!(
        storage_as_compound_file(&mut compound_file, "/WordDocument", 1_000),
        None
    );
}

#[test]
fn object_part_names_a_package_by_its_kind_and_an_ole_object_as_bin() {
    let package = object(b"PK\x03\x04rest".to_vec(), "Microsoft_Excel_Worksheet.XLSX");
    assert!(package.is_package());
    assert_eq!(
        object_part(&package, 2),
        (
            "embeddings/object2.xlsx".to_owned(),
            "package",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_owned()
        )
    );
    let unnamed = object(b"PK\x03\x04rest".to_vec(), "Object 1");
    assert_eq!(object_part(&unnamed, 1).0, "embeddings/object1.xlsx");
    let ole = object(ole_object("Excel.Sheet.8"), "_1234");
    assert!(!ole.is_package());
    assert_eq!(
        object_part(&ole, 3),
        (
            "embeddings/oleObject3.bin".to_owned(),
            "oleObject",
            "application/vnd.openxmlformats-officedocument.oleObject".to_owned()
        )
    );
}

#[test]
fn package_kinds_have_content_types_and_programs() {
    for (extension, program) in [
        ("xlsm", "Excel.Sheet.12"),
        ("docx", "Word.Document.12"),
        ("docm", "Word.Document.12"),
        ("pptx", "PowerPoint.Show.12"),
        ("pptm", "PowerPoint.Show.12"),
        ("sldx", "PowerPoint.Slide.12"),
    ] {
        assert!(package_content_type(extension).is_some());
        assert_eq!(
            program_of_package(&format!("oggetto.{extension}")),
            Some(program)
        );
    }
    assert_eq!(package_content_type("bin"), None);
    assert_eq!(program_of_package("oggetto.bin"), None);
    assert_eq!(program_of_package("oggetto"), None);
}

/// A docx holding one embedded workbook, shown by a PNG, and one linked object.
fn docx_with_objects() -> Vec<u8> {
    let body = concat!(
        r#"<w:p><w:r><w:t>Prima</w:t></w:r><w:r><w:object w:dxaOrig="3000" w:dyaOrig="1500">"#,
        r#"<v:shape id="_x0000_i1025" style="width:150pt;height:75pt"><v:imagedata r:id="rId10" o:title=""/></v:shape>"#,
        r#"<o:OLEObject Type="Embed" ProgID="Excel.Sheet.12" ShapeID="_x0000_i1025" DrawAspect="Content" ObjectID="_1" r:id="rId11"/>"#,
        r#"</w:object></w:r><w:r><w:object>"#,
        r#"<v:shape id="_x0000_i1026" style="width:30pt;height:30pt"><v:imagedata r:id="rId12" o:title=""/></v:shape>"#,
        r#"<o:OLEObject Type="Link" ProgID="Excel.Sheet.12" ShapeID="_x0000_i1026" r:id="rId13"/>"#,
        r#"</w:object></w:r><w:r><w:object><o:OLEObject Type="Embed" r:id="rId11"/></w:object></w:r></w:p>"#,
    );
    let relationships = rels_xml(&[
        ("rId10", "image", "media/image1.png", false),
        (
            "rId11",
            "package",
            "embeddings/Microsoft_Excel_Worksheet.xlsx",
            false,
        ),
        ("rId12", "image", "media/image2.png", false),
        ("rId13", "oleObject", "file:///C:/fogli/esterno.xlsx", true),
    ]);
    build_docx_from_body(
        body,
        &[
            ("word/_rels/document.xml.rels", relationships.as_bytes()),
            ("word/media/image1.png", PNG),
            ("word/media/image2.png", PNG),
            (
                "word/embeddings/Microsoft_Excel_Worksheet.xlsx",
                b"PK\x03\x04il foglio",
            ),
        ],
    )
}

fn images(tree: &DocumentTree) -> Vec<usize> {
    let Block::Paragraph { content, .. } = &tree.blocks[0] else {
        panic!("{:?}", tree.blocks[0]);
    };
    content
        .iter()
        .filter_map(|inline| match inline {
            Inline::Image { media, .. } => Some(*media),
            _ => None,
        })
        .collect()
}

#[test]
fn a_docx_object_is_its_picture_and_keeps_its_part() {
    let tree = read_document_tree(&docx_with_objects(), &Limits::default()).unwrap();
    assert_eq!(tree.report.embedded_objects, 3);
    assert_eq!(images(&tree).len(), 2);
    assert_eq!(tree.objects.len(), 1);
    let kept = &tree.objects[0];
    assert_eq!(kept.media, images(&tree)[0]);
    assert_eq!(kept.program.as_deref(), Some("Excel.Sheet.12"));
    assert_eq!(kept.name, "Microsoft_Excel_Worksheet.xlsx");
    assert_eq!(kept.bytes, b"PK\x03\x04il foglio");
}

#[test]
fn a_docx_written_back_keeps_its_objects() {
    let tree = read_document_tree(&docx_with_objects(), &Limits::default()).unwrap();
    let written = write_docx(&tree).unwrap();
    let document = crate::test_support::zip_member(&written, "word/document.xml");
    assert!(document.contains(r#"<o:OLEObject Type="Embed" ProgID="Excel.Sheet.12""#));
    assert!(document.contains("<v:imagedata r:id="));
    let types = crate::test_support::zip_member(&written, "[Content_Types].xml");
    assert!(types.contains(
        r#"<Override PartName="/word/embeddings/object1.xlsx" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"/>"#
    ));
    let relationships = crate::test_support::zip_member(&written, "word/_rels/document.xml.rels");
    assert!(relationships.contains("relationships/package\" Target=\"embeddings/object1.xlsx\""));
    let again = read_document_tree(&written, &Limits::default()).unwrap();
    assert_eq!(again.objects.len(), 1);
    assert_eq!(again.objects[0].bytes, tree.objects[0].bytes);
    assert_eq!(again.objects[0].program.as_deref(), Some("Excel.Sheet.12"));
    assert_eq!(again.objects[0].media, images(&again)[0]);
}

#[test]
fn an_ole_object_without_a_prog_id_is_written_without_one() {
    let tree = DocumentTree {
        blocks: vec![Block::Paragraph {
            content: vec![Inline::Image {
                media: 0,
                alt: None,
                width: Some(40),
                height: None,
                anchor: None,
            }],
            align: None,
            look: crate::document::ParagraphLook::default(),
        }],
        media: vec![crate::document::Media {
            content_type: "image/png",
            bytes: PNG.to_vec(),
        }],
        objects: vec![object(
            build_compound_file(&[("/Contenuto", b"x")]),
            "Oggetto",
        )],
        ..DocumentTree::default()
    };
    let written = write_docx(&tree).unwrap();
    let document = crate::test_support::zip_member(&written, "word/document.xml");
    assert!(document.contains(r#"<o:OLEObject Type="Embed" ShapeID="#));
    assert!(document.contains(r#"w:dxaOrig="600" w:dyaOrig="3000""#));
    assert!(document.contains("width:30pt;height:150pt"));
    let types = crate::test_support::zip_member(&written, "[Content_Types].xml");
    assert!(types.contains("/word/embeddings/oleObject1.bin"));
}

#[test]
fn an_odt_object_ole_keeps_its_compound_file() {
    let content = concat!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content "#,
        r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
        r#"xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" "#,
        r#"xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" "#,
        r#"xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" "#,
        r#"xmlns:xlink="http://www.w3.org/1999/xlink"><office:body><office:text><text:p>"#,
        r#"<draw:frame svg:width="4cm" svg:height="2cm"><draw:object-ole xlink:href="./Object 1"/>"#,
        r#"<draw:image xlink:href="./ObjectReplacements/Object 1"/></draw:frame>"#,
        r#"</text:p></office:text></office:body></office:document-content>"#
    );
    let ole = ole_object("Excel.Sheet.8");
    let odt = build_zip(&[
        ("mimetype", b"application/vnd.oasis.opendocument.text"),
        ("content.xml", content.as_bytes()),
        ("Object 1", &ole),
        ("ObjectReplacements/Object 1", PNG),
    ]);
    let tree = read_document_tree(&odt, &Limits::default()).unwrap();
    assert_eq!(tree.report.embedded_objects, 1);
    assert_eq!(tree.objects.len(), 1);
    assert_eq!(tree.objects[0].program.as_deref(), Some("Excel.Sheet.8"));
    assert_eq!(tree.objects[0].name, "Object 1");
    assert_eq!(tree.objects[0].bytes, ole);
    assert_eq!(tree.objects[0].media, images(&tree)[0]);
}
