// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use serde_json::{Value, json};

use super::super::{
    Block, DocumentTree, Inline, LineSpacing, ParagraphLook, RunLook, TableCell, TableRow,
    read_document_tree, write_docx,
};
use super::*;
use crate::Limits;
use crate::test_support::{build_docx_from_body, build_zip, zip_member};

fn line(style: BorderStyle, width_eighths: u32, colour: Option<&str>) -> Border {
    Border {
        style,
        width_eighths,
        colour: colour.map(str::to_owned),
    }
}

fn text(value: &str) -> Inline {
    Inline::Text {
        text: value.to_owned(),
        marks: Vec::new(),
        look: RunLook::default(),
    }
}

fn cell(value: &str, look: CellLook) -> TableCell {
    TableCell {
        col_span: 1,
        row_span: 1,
        blocks: vec![Block::Paragraph {
            content: vec![text(value)],
            align: None,
            look: ParagraphLook::default(),
        }],
        look,
    }
}

/// The blocks of a tree as the JSON an editor receives.
fn json_of(blocks: &[Block]) -> Value {
    serde_json::to_value(blocks).unwrap()
}

/// The blocks of `json` written as a docx, as `writeDocx` writes what an editor sends back.
fn written_from_json(json: &Value) -> Vec<u8> {
    let blocks: Vec<Block> = serde_json::from_value(json.clone()).unwrap();
    write_docx(&DocumentTree {
        blocks,
        ..DocumentTree::default()
    })
    .unwrap()
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the whole JSON of a table is spelled out once, as the docs show it"
)]
fn a_table_its_cells_and_a_paragraph_cross_the_json_with_their_looks() {
    let red = line(BorderStyle::Single, 12, Some("C00000"));
    let blocks = vec![
        Block::Paragraph {
            content: vec![text("Premessa")],
            align: None,
            look: ParagraphLook {
                // Spacing a style gives does not cross; the borders and shading drawn do.
                space_after_twips: Some(160),
                shading: Some("FFF2CC".to_owned()),
                borders: Borders {
                    bottom: Some(line(BorderStyle::Double, 6, None)),
                    inside_horizontal: Some(line(BorderStyle::None, 4, None)),
                    ..Borders::default()
                },
                ..ParagraphLook::default()
            },
        },
        Block::Table {
            rows: vec![TableRow {
                header: true,
                cells: vec![
                    cell(
                        "Parte",
                        CellLook {
                            fill: Some("1F3864".to_owned()),
                            borders: Borders {
                                bottom: Some(line(BorderStyle::Dashed, 8, Some("00B050"))),
                                ..Borders::default()
                            },
                            vertical: Some(VerticalAlignment::Center),
                        },
                    ),
                    cell("Dati", CellLook::default()),
                ],
            }],
            look: TableLook {
                column_widths_twips: vec![3_000, 6_000],
                borders: true,
                cell_margins_twips: [Some(57), Some(113), None, Some(113)],
                edges: Borders {
                    top: Some(red.clone()),
                    bottom: Some(red),
                    inside_horizontal: Some(line(BorderStyle::Single, 4, Some("808080"))),
                    inside_vertical: Some(line(BorderStyle::None, 4, None)),
                    ..Borders::default()
                },
                frame: None,
            },
        },
    ];

    let json = json_of(&blocks);

    assert_eq!(
        json,
        json!([
            {
                "type": "paragraph",
                "content": [{ "type": "text", "text": "Premessa" }],
                "format": {
                    "borders": {
                        "bottom": { "style": "double", "widthEighths": 6 },
                        "between": { "style": "none" }
                    },
                    "shading": "#FFF2CC"
                }
            },
            {
                "type": "table",
                "rows": [{
                    "header": true,
                    "cells": [
                        {
                            "colSpan": 1, "rowSpan": 1,
                            "blocks": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Parte" }] }],
                            "fill": "#1F3864",
                            "borders": { "bottom": { "style": "dashed", "widthEighths": 8, "color": "#00B050" } },
                            "verticalAlign": "center"
                        },
                        {
                            "colSpan": 1, "rowSpan": 1,
                            "blocks": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Dati" }] }]
                        }
                    ]
                }],
                "columnWidthsTwips": [3000, 6000],
                "borders": {
                    "top": { "style": "single", "widthEighths": 12, "color": "#C00000" },
                    "bottom": { "style": "single", "widthEighths": 12, "color": "#C00000" },
                    "insideH": { "style": "single", "widthEighths": 4, "color": "#808080" },
                    "insideV": { "style": "none" }
                },
                "cellMarginsTwips": { "top": 57, "left": 113, "right": 113 }
            }
        ])
    );

    let back: Vec<Block> = serde_json::from_value(json.clone()).unwrap();
    let Block::Paragraph { look, .. } = &back[0] else {
        panic!("not a paragraph");
    };
    let Block::Paragraph { look: sent, .. } = &blocks[0] else {
        unreachable!();
    };
    assert_eq!(
        (&look.borders, &look.shading, look.space_after_twips),
        (&sent.borders, &sent.shading, None)
    );
    let (Block::Table { rows, look }, Block::Table { look: sent, .. }) = (&back[1], &blocks[1])
    else {
        panic!("not a table");
    };
    assert_eq!(
        *look,
        TableLook {
            borders: false,
            ..sent.clone()
        },
        "the edges are named, so nothing draws a grid of its own"
    );
    assert_eq!(rows[0].cells[0].look.fill.as_deref(), Some("1F3864"));
    assert_eq!(
        rows[0].cells[0].look.vertical,
        Some(VerticalAlignment::Center)
    );
    assert_eq!(
        json_of(&back),
        json,
        "the JSON reads back as it was written"
    );
}

#[test]
fn a_tree_without_looks_reads_and_writes_as_before() {
    let old = json!([
        { "type": "paragraph", "content": [], "format": { "lineSpacing": { "rule": "exact", "value": 300 } } },
        {
            "type": "table",
            "rows": [{ "header": false, "cells": [{ "colSpan": 1, "rowSpan": 1, "blocks": [] }] }],
            "columnWidthsTwips": [4000]
        },
        { "type": "table", "rows": [] }
    ]);

    let blocks: Vec<Block> = serde_json::from_value(old.clone()).unwrap();

    let Block::Paragraph { look, .. } = &blocks[0] else {
        panic!("not a paragraph");
    };
    assert_eq!(look.line_spacing, Some(LineSpacing::Exact(300)));
    assert_eq!((&look.borders, &look.shading), (&Borders::default(), &None));
    let Block::Table { rows, look } = &blocks[1] else {
        panic!("not a table");
    };
    assert_eq!(
        *look,
        TableLook {
            column_widths_twips: vec![4_000],
            ..TableLook::default()
        }
    );
    assert_eq!(rows[0].cells[0].look, CellLook::default());
    assert!(matches!(&blocks[2], Block::Table { look, .. } if *look == TableLook::default()));
    assert_eq!(json_of(&blocks), old);
}

#[test]
fn checks_what_comes_in_before_the_writer_puts_it_in_the_file() {
    let blocks: Vec<Block> = serde_json::from_value(json!([
        {
            "type": "paragraph", "content": [],
            "format": { "shading": "\"/><w:evil/>", "borders": { "top": { "style": "single", "color": "red" } } }
        },
        {
            "type": "table",
            "rows": [{ "header": false, "cells": [{
                "colSpan": 1, "rowSpan": 1, "blocks": [], "fill": "d9e2f3",
                "borders": { "left": { "style": "dotted", "widthEighths": 0, "color": "#abc" } }
            }] }],
            "borders": { "insideV": { "style": "double", "widthEighths": 500, "color": " #00b050 " } },
            "cellMarginsTwips": { "left": 99999 }
        }
    ]))
    .unwrap();

    let Block::Paragraph { look, .. } = &blocks[0] else {
        panic!("not a paragraph");
    };
    assert_eq!(
        look.shading, None,
        "a shading that is not a colour is left out"
    );
    assert_eq!(
        look.borders.top,
        Some(line(BorderStyle::Single, 4, None)),
        "a missing width is Word's half point, a colour that is not one automatic"
    );
    let Block::Table { rows, look } = &blocks[1] else {
        panic!("not a table");
    };
    assert_eq!(
        look.edges.inside_vertical,
        Some(line(BorderStyle::Double, 96, Some("00B050")))
    );
    assert_eq!(look.cell_margins_twips, [None, Some(31_680), None, None]);
    assert_eq!(rows[0].cells[0].look.fill.as_deref(), Some("D9E2F3"));
    assert_eq!(
        rows[0].cells[0].look.borders.left,
        Some(line(BorderStyle::Dotted, 2, None))
    );

    let document = zip_member(
        &write_docx(&DocumentTree {
            blocks,
            ..DocumentTree::default()
        })
        .unwrap(),
        "word/document.xml",
    );
    assert!(!document.contains("evil"), "{document}");
    assert!(
        serde_json::from_value::<Block>(json!({
            "type": "table", "rows": [], "borders": { "top": { "style": "wavy" } }
        }))
        .is_err(),
        "a line style the tree does not know is refused"
    );
}

const STYLES: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?><w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
    r#"<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/>"#,
    r#"<w:pPr><w:pBdr><w:bottom w:val="single" w:sz="4" w:space="1" w:color="2F5496"/></w:pBdr></w:pPr></w:style>"#,
    r#"<w:style w:type="table" w:styleId="Contratto"><w:name w:val="Contratto"/><w:tblPr><w:tblBorders>"#,
    r#"<w:top w:val="single" w:sz="12" w:space="0" w:color="C00000"/><w:left w:val="nil"/>"#,
    r#"<w:bottom w:val="single" w:sz="12" w:space="0" w:color="C00000"/><w:right w:val="nil"/>"#,
    r#"<w:insideH w:val="single" w:sz="4" w:space="0" w:color="808080"/><w:insideV w:val="nil"/>"#,
    r#"</w:tblBorders></w:tblPr></w:style></w:styles>"#
);

const BODY: &str = concat!(
    r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Contratto di fornitura</w:t></w:r></w:p>"#,
    r#"<w:p><w:pPr><w:pBdr><w:top w:val="single" w:sz="8" w:space="1" w:color="C00000"/>"#,
    r#"<w:bottom w:val="double" w:sz="6" w:space="1" w:color="auto"/></w:pBdr>"#,
    r#"<w:shd w:val="clear" w:color="auto" w:fill="FFF2CC"/></w:pPr><w:r><w:t>Premessa</w:t></w:r></w:p>"#,
    r#"<w:tbl><w:tblPr><w:tblStyle w:val="Contratto"/><w:tblW w:w="0" w:type="auto"/>"#,
    r#"<w:tblCellMar><w:top w:w="57" w:type="dxa"/><w:left w:w="113" w:type="dxa"/>"#,
    r#"<w:bottom w:w="57" w:type="dxa"/><w:right w:w="113" w:type="dxa"/></w:tblCellMar></w:tblPr>"#,
    r#"<w:tblGrid><w:gridCol w:w="3000"/><w:gridCol w:w="6000"/></w:tblGrid>"#,
    r#"<w:tr><w:trPr><w:tblHeader/></w:trPr>"#,
    r#"<w:tc><w:tcPr><w:tcW w:w="3000" w:type="dxa"/><w:shd w:val="clear" w:color="auto" w:fill="1F3864"/><w:vAlign w:val="center"/></w:tcPr>"#,
    r#"<w:p><w:r><w:t>Parte</w:t></w:r></w:p></w:tc>"#,
    r#"<w:tc><w:tcPr><w:tcW w:w="6000" w:type="dxa"/><w:shd w:val="clear" w:color="auto" w:fill="1F3864"/><w:vAlign w:val="center"/></w:tcPr>"#,
    r#"<w:p><w:r><w:t>Dati</w:t></w:r></w:p></w:tc></w:tr>"#,
    r#"<w:tr><w:tc><w:tcPr><w:tcW w:w="3000" w:type="dxa"/><w:tcBorders><w:bottom w:val="dashed" w:sz="12" w:space="0" w:color="00B050"/></w:tcBorders>"#,
    r#"<w:vAlign w:val="bottom"/></w:tcPr><w:p><w:r><w:t>Fornitore</w:t></w:r></w:p></w:tc>"#,
    r#"<w:tc><w:tcPr><w:tcW w:w="6000" w:type="dxa"/><w:shd w:val="clear" w:color="auto" w:fill="F2F2F2"/></w:tcPr>"#,
    r#"<w:p><w:r><w:t>{{controparte.nome}}</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
    r#"<w:p><w:r><w:t>Fine</w:t></w:r></w:p>"#
);

#[test]
fn a_bordered_and_filled_docx_keeps_its_looks_through_the_json_tree() {
    let original = build_docx_from_body(BODY, &[("word/styles.xml", STYLES.as_bytes())]);
    let first = json_of(
        &read_document_tree(&original, &Limits::default())
            .unwrap()
            .blocks,
    );

    assert_eq!(
        first[0]["format"],
        json!({ "borders": { "bottom": { "style": "single", "widthEighths": 4, "color": "#2F5496" } } }),
        "a border the style draws crosses, since the writer writes no style of the file"
    );
    assert_eq!(
        first[1]["format"],
        json!({
            "borders": {
                "top": { "style": "single", "widthEighths": 8, "color": "#C00000" },
                "bottom": { "style": "double", "widthEighths": 6 }
            },
            "shading": "#FFF2CC"
        })
    );
    let table = &first[2];
    assert_eq!(table["columnWidthsTwips"], json!([3000, 6000]));
    assert_eq!(
        table["borders"],
        json!({
            "top": { "style": "single", "widthEighths": 12, "color": "#C00000" },
            "left": { "style": "none" },
            "bottom": { "style": "single", "widthEighths": 12, "color": "#C00000" },
            "right": { "style": "none" },
            "insideH": { "style": "single", "widthEighths": 4, "color": "#808080" },
            "insideV": { "style": "none" }
        }),
        "the table's lines come from its style"
    );
    assert_eq!(
        table["cellMarginsTwips"],
        json!({ "top": 57, "left": 113, "bottom": 57, "right": 113 })
    );
    let header = &table["rows"][0]["cells"][0];
    assert_eq!(
        (&header["fill"], &header["verticalAlign"]),
        (&json!("#1F3864"), &json!("center"))
    );
    let body = &table["rows"][1]["cells"];
    assert_eq!(
        body[0]["borders"],
        json!({ "bottom": { "style": "dashed", "widthEighths": 12, "color": "#00B050" } })
    );
    assert_eq!(body[0]["verticalAlign"], json!("bottom"));
    assert_eq!(body[1]["fill"], json!("#F2F2F2"));

    let written = written_from_json(&first);
    let document = zip_member(&written, "word/document.xml");
    for element in [
        r#"<w:tblBorders><w:top w:val="single" w:sz="12" w:space="0" w:color="C00000"/>"#,
        r#"<w:tblCellMar><w:top w:w="57" w:type="dxa"/>"#,
        r#"<w:tcBorders><w:bottom w:val="dashed" w:sz="12" w:space="0" w:color="00B050"/></w:tcBorders>"#,
        r#"<w:shd w:val="clear" w:color="auto" w:fill="1F3864"/><w:vAlign w:val="center"/>"#,
        r#"<w:pBdr><w:top w:val="single" w:sz="8" w:space="0" w:color="C00000"/>"#,
        r#"<w:shd w:val="clear" w:color="auto" w:fill="FFF2CC"/>"#,
    ] {
        assert!(document.contains(element), "{element} in {document}");
    }
    assert!(
        !document.contains("TableGrid"),
        "a table whose lines are named draws no grid of its own"
    );
    let again = json_of(
        &read_document_tree(&written, &Limits::default())
            .unwrap()
            .blocks,
    );
    assert_eq!(again, first, "read, sent as JSON, written and read again");
}

#[test]
fn an_odt_table_and_paragraph_keep_their_looks_through_the_json_tree() {
    let namespaces = concat!(
        r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
        r#"xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" "#,
        r#"xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" "#,
        r#"xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" "#,
        r#"xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0""#
    );
    let content = format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content {namespaces}><office:automatic-styles>"#,
            r#"<style:style style:name="P1" style:family="paragraph"><style:paragraph-properties "#,
            r##"fo:background-color="#fff2cc" fo:border-bottom="0.5pt solid #2f5496"/></style:style>"##,
            r#"<style:style style:name="Tab.A" style:family="table-column"><style:table-column-properties style:column-width="2in"/></style:style>"#,
            r#"<style:style style:name="Tab.A1" style:family="table-cell"><style:table-cell-properties "#,
            r##"fo:background-color="#d9e2f3" fo:padding="0.05in" fo:border="0.5pt solid #c00000" style:vertical-align="middle"/></style:style>"##,
            r#"<style:style style:name="Tab.B1" style:family="table-cell"><style:table-cell-properties fo:padding="0.05in" "#,
            r##"fo:border-top="none" fo:border-left="none" fo:border-right="none" fo:border-bottom="1.5pt dashed #00b050"/></style:style>"##,
            r#"</office:automatic-styles><office:body><office:text>"#,
            r#"<text:p text:style-name="P1">Premessa</text:p>"#,
            r#"<table:table table:name="Tab"><table:table-column table:style-name="Tab.A" table:number-columns-repeated="2"/>"#,
            r#"<table:table-row><table:table-cell table:style-name="Tab.A1"><text:p>Parte</text:p></table:table-cell>"#,
            r#"<table:table-cell table:style-name="Tab.B1"><text:p>Dati</text:p></table:table-cell></table:table-row></table:table>"#,
            r#"</office:text></office:body></office:document-content>"#
        ),
        namespaces = namespaces
    );
    let odt = build_zip(&[
        ("mimetype", b"application/vnd.oasis.opendocument.text"),
        ("content.xml", content.as_bytes()),
    ]);
    let first = json_of(&read_document_tree(&odt, &Limits::default()).unwrap().blocks);

    assert_eq!(
        first[0]["format"],
        json!({
            "borders": { "bottom": { "style": "single", "widthEighths": 4, "color": "#2F5496" } },
            "shading": "#FFF2CC"
        })
    );
    let table = &first[1];
    assert_eq!(table["columnWidthsTwips"], json!([2880, 2880]));
    assert_eq!(
        table["cellMarginsTwips"],
        json!({ "top": 72, "left": 72, "bottom": 72, "right": 72 })
    );
    assert_eq!(
        table.get("borders"),
        None,
        "an odt draws its lines on its cells"
    );
    let cells = &table["rows"][0]["cells"];
    let red = json!({ "style": "single", "widthEighths": 4, "color": "#C00000" });
    assert_eq!(
        cells[0]["borders"],
        json!({ "top": red, "left": red, "bottom": red, "right": red })
    );
    assert_eq!(
        (&cells[0]["fill"], &cells[0]["verticalAlign"]),
        (&json!("#D9E2F3"), &json!("center"))
    );
    assert_eq!(
        cells[1]["borders"]["bottom"],
        json!({ "style": "dashed", "widthEighths": 12, "color": "#00B050" })
    );

    let again = json_of(
        &read_document_tree(&written_from_json(&first), &Limits::default())
            .unwrap()
            .blocks,
    );
    assert_eq!(
        again, first,
        "the docx written from the odt's JSON keeps the looks"
    );
}
