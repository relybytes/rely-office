// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use std::io::{Cursor, Write};

use serde_json::{Value, json};

use super::*;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

fn zip(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content) in members {
        writer
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(content).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn docx(body: &str, extra: &[(&str, &[u8])]) -> Vec<u8> {
    let content_types = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let document = format!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:body>{body}</w:body></w:document>"#
    );
    let mut members: Vec<(&str, &[u8])> = vec![
        ("[Content_Types].xml", content_types.as_bytes()),
        ("word/document.xml", document.as_bytes()),
    ];
    members.extend_from_slice(extra);
    zip(&members)
}

fn parse(json: &str) -> Value {
    serde_json::from_str(json).unwrap()
}

#[test]
fn detects_formats_by_name() {
    assert_eq!(detect(&docx("", &[]), Some("x.bin")), "docx");
    assert_eq!(detect(b"a;b\n", Some("x.csv")), "csv");
    assert_eq!(detect(b"plain", None), "unknown");
}

#[test]
fn extracts_text_as_json() {
    let bytes = docx("<w:p><w:r><w:t>Offerta</w:t></w:r></w:p>", &[]);
    assert_eq!(
        parse(&extract_text_json(&bytes, None, None).unwrap()),
        json!({ "format": "docx", "text": "Offerta\n", "truncated": false })
    );
    assert_eq!(
        parse(&extract_text_json(&bytes, None, Some(r#"{"maxTextChars":3}"#)).unwrap()),
        json!({ "format": "docx", "text": "Off", "truncated": true })
    );
}

#[test]
fn reads_sheets_as_json() {
    let json = read_sheets_json(b"a;b\n1;2\n", Some("x.csv"), Some("null")).unwrap();
    assert_eq!(
        parse(&json),
        json!({
            "format": "csv",
            "sheets": [{ "name": "", "rows": [["a", "b"], ["1", "2"]], "totalRows": 2, "totalColumns": 2 }]
        })
    );
}

#[test]
fn reads_a_document_tree_with_its_media_aside() {
    let rels = concat!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
        r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/logo.png"/>"#,
        r#"<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://relybytes.com" TargetMode="External"/>"#,
        "</Relationships>",
    );
    let body = concat!(
        r#"<w:p><w:pPr><w:outlineLvl w:val="0"/></w:pPr><w:r><w:t>Titolo</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Forte</w:t></w:r><w:hyperlink r:id="rId2"><w:r><w:t>sito</w:t></w:r></w:hyperlink>"#,
        r#"<w:r><w:drawing><wp:inline><wp:extent cx="952500" cy="952500"/><wp:docPr id="1" name="a" descr="Logo"/>"#,
        r#"<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:blipFill>"#,
        r#"<a:blip r:embed="rId1"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#,
    );
    let bytes = docx(
        body,
        &[
            ("word/_rels/document.xml.rels", rels.as_bytes()),
            ("word/media/logo.png", PNG),
        ],
    );
    let mut handle = document_tree(&bytes, None, None, None).unwrap();
    let tree = parse(&handle.json());
    assert_eq!(
        tree["blocks"],
        json!([
            { "type": "heading", "level": 1, "content": [{ "type": "text", "text": "Titolo" }] },
            { "type": "paragraph", "content": [
                { "type": "text", "text": "Forte", "marks": [{ "type": "bold" }] },
                { "type": "text", "text": "sito", "marks": [{ "type": "link", "href": "https://relybytes.com" }] },
                { "type": "image", "media": 0, "alt": "Logo", "width": 100, "height": 100 }
            ]}
        ])
    );
    assert_eq!(
        tree["media"],
        json!([{ "index": 0, "contentType": "image/png", "byteLength": PNG.len() }])
    );
    assert_eq!(tree["report"]["truncated"], json!(false));
    assert_eq!(tree["report"]["unsupportedLinks"], json!(0));
    assert_eq!(handle.media_count(), 1);
    assert_eq!(handle.take_media_bytes(0), Some(PNG.to_vec()));
    assert_eq!(handle.take_media_bytes(0), Some(Vec::new()));
    assert_eq!(handle.take_media_bytes(1), None);
}

#[test]
fn serializes_every_block_kind_in_camel_case() {
    let blocks = vec![
        Block::ListItem {
            ordered: true,
            level: 1,
            list_id: 4,
            content: vec![rely_office_core::Inline::HardBreak],
            look: rely_office_core::ParagraphLook::default(),
        },
        Block::Table {
            rows: vec![rely_office_core::TableRow {
                header: true,
                cells: vec![rely_office_core::TableCell {
                    col_span: 2,
                    row_span: 1,
                    blocks: vec![Block::HorizontalRule],
                    look: rely_office_core::CellLook::default(),
                }],
            }],
            look: rely_office_core::TableLook::default(),
        },
        Block::Blockquote { blocks: vec![] },
        Block::CodeBlock {
            text: "x".to_owned(),
        },
    ];
    assert_eq!(
        serde_json::to_value(&blocks).unwrap(),
        json!([
            { "type": "listItem", "ordered": true, "level": 1, "listId": 4, "content": [{ "type": "hardBreak" }] },
            { "type": "table", "rows": [{ "header": true, "cells": [{ "colSpan": 2, "rowSpan": 1, "blocks": [{ "type": "horizontalRule" }] }] }] },
            { "type": "blockquote", "blocks": [] },
            { "type": "codeBlock", "text": "x" }
        ])
    );
    let report = serde_json::to_value(ImportReport::default()).unwrap();
    for key in [
        "truncated",
        "textBoxes",
        "footnotes",
        "endnotes",
        "comments",
        "headersAndFooters",
        "fieldsKeptAsText",
        "charts",
        "smartArt",
        "equations",
        "shapes",
        "embeddedObjects",
        "unsupportedLinks",
        "oversizedImages",
        "imagesOverLimit",
        "missingImages",
        "trackedInsertions",
        "trackedDeletions",
        "hiddenRuns",
    ] {
        assert!(report.get(key).is_some(), "{key}");
    }
}

#[test]
fn reports_library_errors_with_their_code() {
    let failure = extract_text_json(b"plain", Some("a.txt"), None).unwrap_err();
    assert_eq!(failure.code, "UNSUPPORTED_FORMAT");
    assert!(
        failure
            .error_message()
            .starts_with("UNSUPPORTED_FORMAT: unsupported format")
    );
    let failure = document_tree(b"a,b", None, None, None).unwrap_err();
    assert_eq!(failure.code, "UNSUPPORTED_FORMAT");
    let failure =
        read_sheets_json(b"a,b", Some("a.csv"), Some(r#"{"maxInputBytes":1}"#)).unwrap_err();
    assert_eq!(failure.code, "TOO_LARGE");
}

#[test]
fn refuses_limits_it_cannot_read() {
    for limits in [
        r#"{"maxRows":-1}"#,
        r#"{"maxRows":1.5}"#,
        r#"{"maxRowz":1}"#,
        "[1]",
        "{",
    ] {
        let failure = extract_text_json(b"a", Some("a.csv"), Some(limits)).unwrap_err();
        assert_eq!(failure.code, "INVALID_LIMITS", "{limits}");
    }
}

#[test]
fn applies_only_the_limits_it_is_given() {
    let limits = parse_limits(Some(r#"{"maxImages":3,"maxTotalImageBytes":9}"#)).unwrap();
    assert_eq!(limits.max_images, 3);
    assert_eq!(limits.max_total_image_bytes, 9);
    assert_eq!(limits.max_text_chars, Limits::default().max_text_chars);
    assert_eq!(parse_limits(Some("  ")).unwrap(), Limits::default());
    assert_eq!(parse_limits(None).unwrap(), Limits::default());
}

#[test]
fn names_its_version() {
    assert_eq!(version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn writes_a_docx_from_the_json_of_a_tree() {
    let tree = json!({
        "blocks": [
            { "type": "heading", "level": 2, "content": [{ "type": "text", "text": "Titolo" }] },
            {
                "type": "paragraph",
                "align": "center",
                "content": [
                    { "type": "text", "text": "Ciao", "marks": [{ "type": "bold" }] },
                    { "type": "image", "media": 0, "alt": "logo", "width": 10, "height": 10 }
                ]
            }
        ],
        "page": { "widthTwips": 16838, "heightTwips": 11906, "marginsTwips": [720, 720, 720, 720] }
    });
    let mut writer = DocxWriter::new();
    assert_eq!(writer.add_media(PNG.to_vec()), 0);
    let bytes = docx_from_json(
        &tree.to_string(),
        std::mem::take(&mut writer.media),
        std::mem::take(&mut writer.objects),
    )
    .unwrap();
    let read = parse(&document_tree(&bytes, None, None, None).unwrap().json());
    assert_eq!(read["blocks"][0]["type"], "heading");
    assert_eq!(read["blocks"][1]["align"], "center");
    assert_eq!(
        read["blocks"][1]["content"][0]["marks"],
        json!([{ "type": "bold" }])
    );
    assert_eq!(read["media"][0]["contentType"], "image/png");

    let failure =
        docx_from_json(r#"{"blocks":[{"type":"nope"}]}"#, Vec::new(), Vec::new()).unwrap_err();
    assert_eq!(failure.code, "INVALID_TREE");
    assert_eq!(
        docx_from_json(r#"{"blocks":[],"extra":1}"#, Vec::new(), Vec::new())
            .unwrap_err()
            .code,
        "INVALID_TREE"
    );
}

#[test]
fn converts_a_document_to_a_docx() {
    let source = docx("<w:p><w:r><w:t>Da convertire</w:t></w:r></w:p>", &[]);
    let written = convert_to_docx(&source, None).unwrap();
    let tree = read_document_tree(&written, &Limits::default()).unwrap();
    let Block::Paragraph { content, .. } = &tree.blocks[0] else {
        panic!("a paragraph");
    };
    let text: String = content
        .iter()
        .filter_map(|inline| match inline {
            rely_office_core::Inline::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text, "Da convertire");
    assert_eq!(
        convert_to_docx(b"nothing", None).unwrap_err().code,
        "UNSUPPORTED_FORMAT"
    );
    assert_eq!(
        convert_to_docx(&source, Some("[")).unwrap_err().code,
        "INVALID_LIMITS"
    );
}

#[test]
fn converts_a_presentation_to_a_pptx() {
    let presentation = zip(&[
        (
            "[Content_Types].xml",
            br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/></Types>"#,
        ),
        (
            "ppt/presentation.xml",
            br#"<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:sldSz cx="9144000" cy="6858000"/></p:presentation>"#,
        ),
    ]);
    let written = convert_to_pptx(&presentation, None).unwrap();
    let read = rely_office_core::read_presentation(&written, &Limits::default()).unwrap();
    assert_eq!((read.width_emu, read.slides.len()), (9_144_000, 1));
    assert_eq!(
        convert_to_pptx(b"nothing", None).unwrap_err().code,
        "UNSUPPORTED_FORMAT"
    );
}

#[test]
fn an_embedded_object_goes_out_with_the_tree_and_back_into_a_docx() {
    let tree = json!({
        "blocks": [{
            "type": "paragraph",
            "content": [{ "type": "image", "media": 0, "width": 80, "height": 40 }]
        }]
    });
    let mut writer = DocxWriter::new();
    writer.add_media(PNG.to_vec());
    assert_eq!(
        writer.add_object(
            0,
            b"PK\x03\x04il foglio".to_vec(),
            "Foglio.xlsx".to_owned(),
            Some("Excel.Sheet.12".to_owned()),
        ),
        0
    );
    let bytes = docx_from_json(
        &tree.to_string(),
        std::mem::take(&mut writer.media),
        std::mem::take(&mut writer.objects),
    )
    .unwrap();
    let mut handle = document_tree(&bytes, None, None, None).unwrap();
    let read = parse(&handle.json());
    assert_eq!(
        read["objects"],
        json!([{
            "index": 0,
            "media": 0,
            "program": "Excel.Sheet.12",
            "name": "object1.xlsx",
            "package": true,
            "byteLength": 13
        }])
    );
    assert_eq!(
        handle.take_object_bytes(0).as_deref(),
        Some(&b"PK\x03\x04il foglio"[..])
    );
    assert_eq!(handle.take_object_bytes(0).as_deref(), Some(&[][..]));
    assert_eq!(handle.take_object_bytes(1), None);
    let plain = parse(
        &document_tree(&docx("<w:p/>", &[]), None, None, None)
            .unwrap()
            .json(),
    );
    assert_eq!(plain.get("objects"), None);
}

#[test]
fn reads_markdown_and_html_by_their_names_and_writes_them_back() {
    let handle = document_tree(b"# Titolo\n\nUn **testo**.", None, None, Some("nota.md")).unwrap();
    let tree = parse(&handle.json());
    assert_eq!(tree["blocks"][0]["type"], "heading");
    let page = document_tree(b"<!doctype html><p>Ciao <b>mondo</b></p>", None, None, None).unwrap();
    assert_eq!(parse(&page.json())["blocks"][0]["type"], "paragraph");
    assert!(document_tree(b"# Titolo", None, None, None).is_err());
    let json = r#"{"blocks":[{"type":"paragraph","content":[{"type":"text","text":"x","marks":[{"type":"bold"}]}]}]}"#;
    let tree = tree_from_json(json, Vec::new(), Vec::new()).unwrap();
    assert_eq!(rely_office_core::write_markdown(&tree), "**x**\n");
    assert!(rely_office_core::write_html(&tree, "T").contains("<p><strong>x</strong></p>"));
}

const CARLITO: &[u8] = include_bytes!("../../../fonts/Carlito-Regular.ttf");

#[test]
fn carries_the_fonts_of_a_document_through_javascript() {
    let mut writer = DocxWriter::new();
    assert_eq!(writer.add_font_file(b"not a font".to_vec()), None);
    let file = writer.add_font_file(CARLITO.to_vec()).unwrap();
    let json = format!(
        concat!(
            r#"{{"blocks":[{{"type":"paragraph","content":[{{"type":"text","text":"x","marks":"#,
            r#"[{{"type":"textStyle","fontFamily":"Carlito"}}]}}]}}],"#,
            r#""fonts":[{{"name":"Carlito","generic":"swiss","pitch":"variable","faces":[{{"style":"regular","file":{}}}]}}],"#,
            r#""defaultFont":{{"family":"Carlito","sizeHalfPoints":24}}}}"#
        ),
        file
    );
    let tree = writer.take_tree(&json, true).unwrap();
    assert_eq!(tree.layout.font_family.as_deref(), Some("Carlito"));
    assert_eq!(tree.layout.font_size_half_points, Some(24));
    assert_eq!(tree.fonts.files.len(), 1);
    let docx = write_docx(&tree).unwrap();

    let mut handle = document_tree(&docx, None, None, None).unwrap();
    let read = parse(&handle.json());
    assert_eq!(read["fonts"][0]["name"], "Carlito");
    assert_eq!(
        read["fonts"][0]["faces"][0],
        json!({ "style": "regular", "file": 0 })
    );
    assert_eq!(read["fontFiles"][0]["byteLength"], CARLITO.len());
    assert_eq!(
        read["defaultFont"],
        json!({ "family": "Carlito", "sizeHalfPoints": 24 })
    );
    assert_eq!(handle.font_file_count(), 1);
    assert_eq!(handle.take_font_file_bytes(0).unwrap(), CARLITO);
    assert_eq!(handle.take_font_file_bytes(0).unwrap(), Vec::<u8>::new());
    assert_eq!(handle.take_font_file_bytes(1), None);
}

#[test]
fn writes_font_stacks_for_javascript() {
    assert_eq!(
        font_stack_js("Calibri", None),
        "'Calibri', 'Carlito', sans-serif"
    );
    assert_eq!(
        font_stack_js("Mystery", Some("roman".to_owned())),
        "'Mystery', 'Liberation Serif', serif"
    );
    assert_eq!(font_substitute_js("Consolas", None), "Liberation Mono");
    assert_eq!(
        font_substitute_js("Mystery", Some("modern".to_owned())),
        "Liberation Mono"
    );
    assert_eq!(
        font_substitute_js("Mystery", Some("script".to_owned())),
        "Carlito"
    );
    assert_eq!(
        font_substitute_js("Mystery", Some("decorative".to_owned())),
        "Carlito"
    );
    assert_eq!(
        font_substitute_js("Mystery", Some("swiss".to_owned())),
        "Liberation Sans"
    );
}
