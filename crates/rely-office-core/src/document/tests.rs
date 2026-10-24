// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;
use crate::test_support::{build_docx_from_body, build_zip, rels_xml, styles_xml};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
const RELS_PART: &str = "word/_rels/document.xml.rels";

fn tree(body: &str) -> DocumentTree {
    tree_with(body, &[])
}

fn tree_with(body: &str, extra: &[(&str, &[u8])]) -> DocumentTree {
    read_document_tree(&build_docx_from_body(body, extra), &Limits::default()).unwrap()
}

fn text(value: &str) -> Inline {
    Inline::Text {
        text: value.to_owned(),
        marks: Vec::new(),
    }
}

fn marked(value: &str, marks: Vec<Mark>) -> Inline {
    Inline::Text {
        text: value.to_owned(),
        marks,
    }
}

fn paragraph(value: &str) -> Block {
    Block::Paragraph {
        content: vec![text(value)],
    }
}

fn styled_paragraph(style: &str, value: &str) -> String {
    format!(r#"<w:p><w:pPr><w:pStyle w:val="{style}"/></w:pPr><w:r><w:t>{value}</w:t></w:r></w:p>"#)
}

fn drawing(relationship_id: &str) -> String {
    format!(
        concat!(
            r#"<w:r><w:drawing><wp:inline><wp:extent cx="952500" cy="476250"/>"#,
            r#"<wp:docPr id="1" name="Immagine 1" descr="Il logo"/><a:graphic>"#,
            r#"<a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">"#,
            r#"<pic:pic><pic:blipFill><a:blip r:embed="{}"/></pic:blipFill></pic:pic>"#,
            r#"</a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#
        ),
        relationship_id
    )
}

fn graphic(uri: &str, inner: &str) -> String {
    format!(
        r#"<w:r><w:drawing><wp:anchor><a:graphic><a:graphicData uri="{uri}">{inner}</a:graphicData></a:graphic></wp:anchor></w:drawing></w:r>"#
    )
}

#[test]
fn reads_plain_paragraphs_and_empty_ones() {
    let read = tree("<w:p><w:r><w:t>Ciao</w:t></w:r></w:p><w:p/>");
    assert_eq!(
        read.blocks,
        vec![paragraph("Ciao"), Block::Paragraph { content: vec![] }]
    );
    assert_eq!(read.report, ImportReport::default());
}

#[test]
fn reads_headings_from_styles_outline_levels_and_title() {
    let styles = styles_xml(&[
        ("Titolo1", "heading 1", ""),
        ("Titolo8", "heading 8", ""),
        ("Titolo", "Title", ""),
        (
            "Capitolo",
            "Capitolo",
            r#"<w:pPr><w:outlineLvl w:val="1"/></w:pPr>"#,
        ),
    ]);
    let body = [
        styled_paragraph("Titolo1", "Uno"),
        styled_paragraph("Titolo8", "Otto"),
        styled_paragraph("Titolo", "Documento"),
        styled_paragraph("Capitolo", "Capitolo"),
        r#"<w:p><w:pPr><w:outlineLvl w:val="2"/></w:pPr><w:r><w:t>Diretto</w:t></w:r></w:p>"#
            .to_owned(),
    ]
    .concat();
    let read = tree_with(&body, &[("word/styles.xml", styles.as_bytes())]);
    let levels: Vec<u8> = read
        .blocks
        .iter()
        .map(|block| match block {
            Block::Heading { level, .. } => *level,
            other => panic!("not a heading: {other:?}"),
        })
        .collect();
    assert_eq!(levels, vec![1, 6, 1, 2, 3]);
}

#[test]
fn reads_marks_and_merges_runs_that_share_them() {
    let body = concat!(
        "<w:p>",
        "<w:r><w:rPr><w:b/></w:rPr><w:t>gras</w:t></w:r>",
        "<w:r><w:rPr><w:b/></w:rPr><w:t>setto</w:t></w:r>",
        r#"<w:r><w:rPr><w:i/><w:u w:val="single"/></w:rPr><w:t xml:space="preserve"> corsivo </w:t></w:r>"#,
        "<w:r><w:rPr><w:strike/></w:rPr><w:t>barrato</w:t></w:r>",
        r#"<w:r><w:rPr><w:rFonts w:ascii="Consolas" w:hAnsi="Consolas"/></w:rPr><w:t>codice</w:t></w:r>"#,
        r#"<w:r><w:rPr><w:vertAlign w:val="superscript"/></w:rPr><w:t>2</w:t></w:r>"#,
        r#"<w:r><w:rPr><w:vertAlign w:val="subscript"/></w:rPr><w:t>i</w:t></w:r>"#,
        r#"<w:r><w:rPr><w:b w:val="0"/></w:rPr><w:t>normale</w:t></w:r>"#,
        "</w:p>",
    );
    let read = tree(body);
    assert_eq!(
        read.blocks,
        vec![Block::Paragraph {
            content: vec![
                marked("grassetto", vec![Mark::Bold]),
                marked(" corsivo ", vec![Mark::Italic, Mark::Underline]),
                marked("barrato", vec![Mark::Strike]),
                marked("codice", vec![Mark::Code]),
                marked("2", vec![Mark::Superscript]),
                marked("i", vec![Mark::Subscript]),
                text("normale"),
            ]
        }]
    );
}

#[test]
fn applies_character_styles_under_direct_formatting() {
    let styles = styles_xml(&[]).replace(
        "</w:styles>",
        r#"<w:style w:type="character" w:styleId="Forte"><w:name w:val="Strong"/></w:style></w:styles>"#,
    );
    let body = concat!(
        r#"<w:p><w:r><w:rPr><w:rStyle w:val="Forte"/></w:rPr><w:t>forte</w:t></w:r>"#,
        r#"<w:r><w:rPr><w:rStyle w:val="Forte"/><w:b w:val="false"/></w:rPr><w:t>no</w:t></w:r></w:p>"#,
    );
    let read = tree_with(body, &[("word/styles.xml", styles.as_bytes())]);
    assert_eq!(
        read.blocks,
        vec![Block::Paragraph {
            content: vec![marked("forte", vec![Mark::Bold]), text("no")]
        }]
    );
}

#[test]
fn ignores_the_old_formatting_of_a_tracked_format_change() {
    let body = concat!(
        "<w:p><w:r><w:rPr><w:i/><w:rPrChange w:id=\"1\"><w:rPr><w:b/></w:rPr></w:rPrChange>",
        "</w:rPr><w:t>nuovo</w:t></w:r></w:p>",
    );
    assert_eq!(
        tree(body).blocks,
        vec![Block::Paragraph {
            content: vec![marked("nuovo", vec![Mark::Italic])]
        }]
    );
}

#[test]
fn keeps_web_and_mail_links_and_reports_the_others() {
    let rels = rels_xml(&[
        ("rId1", "hyperlink", "https://relybytes.com/", true),
        ("rId2", "hyperlink", "mailto:info@example.com", true),
        ("rId3", "hyperlink", "javascript:alert(1)", true),
        ("rId4", "hyperlink", "file:///C:/segreto.txt", true),
    ]);
    let body = concat!(
        "<w:p>",
        r#"<w:hyperlink r:id="rId1"><w:r><w:rPr><w:u w:val="single"/></w:rPr><w:t>sito</w:t></w:r></w:hyperlink>"#,
        r#"<w:hyperlink r:id="rId2"><w:r><w:t>posta</w:t></w:r></w:hyperlink>"#,
        r#"<w:hyperlink r:id="rId3"><w:r><w:t>script</w:t></w:r></w:hyperlink>"#,
        r#"<w:hyperlink r:id="rId4"><w:r><w:t>file</w:t></w:r></w:hyperlink>"#,
        r#"<w:hyperlink w:anchor="_Toc1"><w:r><w:t>indice</w:t></w:r></w:hyperlink>"#,
        r#"<w:hyperlink r:id="rId9"><w:r><w:t>perso</w:t></w:r></w:hyperlink>"#,
        "</w:p>",
    );
    let read = tree_with(body, &[(RELS_PART, rels.as_bytes())]);
    assert_eq!(
        read.blocks,
        vec![Block::Paragraph {
            content: vec![
                marked(
                    "sito",
                    vec![Mark::Link {
                        href: "https://relybytes.com/".to_owned()
                    }]
                ),
                marked(
                    "posta",
                    vec![Mark::Link {
                        href: "mailto:info@example.com".to_owned()
                    }]
                ),
                text("scriptfileindiceperso"),
            ]
        }]
    );
    assert_eq!(read.report.unsupported_links, 4);
}

#[test]
fn adds_the_anchor_of_an_external_link() {
    let rels = rels_xml(&[("rId1", "hyperlink", "https://example.com/pagina", true)]);
    let body = r#"<w:p><w:hyperlink r:id="rId1" w:anchor="sezione"><w:r><w:t>a</w:t></w:r></w:hyperlink></w:p>"#;
    let read = tree_with(body, &[(RELS_PART, rels.as_bytes())]);
    assert_eq!(
        read.blocks,
        vec![Block::Paragraph {
            content: vec![marked(
                "a",
                vec![Mark::Link {
                    href: "https://example.com/pagina#sezione".to_owned()
                }]
            )]
        }]
    );
}

#[test]
fn keeps_field_results_and_links_hyperlink_fields() {
    let body = concat!(
        "<w:p>",
        r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r>"#,
        r#"<w:r><w:instrText xml:space="preserve"> HYPERLINK "https://example.com" \o "suggerimento" </w:instrText></w:r>"#,
        r#"<w:r><w:fldChar w:fldCharType="separate"/></w:r>"#,
        "<w:r><w:t>esempio</w:t></w:r>",
        r#"<w:r><w:fldChar w:fldCharType="end"/></w:r>"#,
        r#"<w:r><w:t xml:space="preserve"> pagina </w:t></w:r>"#,
        r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText>PAGE</w:instrText></w:r>"#,
        r#"<w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>3</w:t></w:r>"#,
        r#"<w:r><w:fldChar w:fldCharType="end"/></w:r>"#,
        r#"<w:fldSimple w:instr=" HYPERLINK &quot;ftp://x&quot; "><w:r><w:t> semplice</w:t></w:r></w:fldSimple>"#,
        "</w:p>",
    );
    let read = tree(body);
    assert_eq!(
        read.blocks,
        vec![Block::Paragraph {
            content: vec![
                marked(
                    "esempio",
                    vec![Mark::Link {
                        href: "https://example.com".to_owned()
                    }]
                ),
                text(" pagina 3 semplice"),
            ]
        }]
    );
    assert_eq!(read.report.fields_kept_as_text, 3);
    assert_eq!(read.report.unsupported_links, 1);
}

#[test]
fn closes_a_field_left_without_a_separator_at_the_paragraph_end() {
    let body = concat!(
        r#"<w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText>TOC</w:instrText></w:r></w:p>"#,
        "<w:p><w:r><w:t>dopo</w:t></w:r></w:p>",
    );
    assert_eq!(
        tree(body).blocks,
        vec![Block::Paragraph { content: vec![] }, paragraph("dopo")]
    );
}

#[test]
fn reads_bullet_and_numbered_lists_with_their_levels() {
    let numbering = concat!(
        r#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
        r#"<w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/></w:lvl>"#,
        r#"<w:lvl w:ilvl="1"><w:numFmt w:val="decimal"/></w:lvl></w:abstractNum>"#,
        r#"<w:abstractNum w:abstractNumId="1"><w:lvl w:ilvl="0"><w:numFmt w:val="upperLetter"/></w:lvl></w:abstractNum>"#,
        r#"<w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>"#,
        r#"<w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num></w:numbering>"#,
    );
    let item = |num_id: u32, level: u8, value: &str| {
        format!(
            r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="{level}"/><w:numId w:val="{num_id}"/></w:numPr></w:pPr><w:r><w:t>{value}</w:t></w:r></w:p>"#
        )
    };
    let body = [
        item(1, 0, "punto"),
        item(1, 1, "sotto"),
        item(2, 0, "lettera"),
        item(0, 0, "nessuno"),
    ]
    .concat();
    let read = tree_with(&body, &[("word/numbering.xml", numbering.as_bytes())]);
    assert_eq!(
        read.blocks,
        vec![
            Block::ListItem {
                ordered: false,
                level: 0,
                list_id: 1,
                content: vec![text("punto")]
            },
            Block::ListItem {
                ordered: true,
                level: 1,
                list_id: 1,
                content: vec![text("sotto")]
            },
            Block::ListItem {
                ordered: true,
                level: 0,
                list_id: 2,
                content: vec![text("lettera")]
            },
            paragraph("nessuno"),
        ]
    );
}

#[test]
fn takes_list_numbering_from_the_paragraph_style() {
    let styles = styles_xml(&[(
        "Elenco",
        "List Bullet",
        r#"<w:pPr><w:numPr><w:numId w:val="4"/></w:numPr></w:pPr>"#,
    )]);
    let numbering = concat!(
        r#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
        r#"<w:abstractNum w:abstractNumId="3"><w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/></w:lvl></w:abstractNum>"#,
        r#"<w:num w:numId="4"><w:abstractNumId w:val="3"/></w:num></w:numbering>"#,
    );
    let read = tree_with(
        &styled_paragraph("Elenco", "voce"),
        &[
            ("word/styles.xml", styles.as_bytes()),
            ("word/numbering.xml", numbering.as_bytes()),
        ],
    );
    assert_eq!(
        read.blocks,
        vec![Block::ListItem {
            ordered: false,
            level: 0,
            list_id: 4,
            content: vec![text("voce")]
        }]
    );
}

#[test]
fn reads_tables_with_header_rows_spans_merges_and_nesting() {
    let cell = |properties: &str, value: &str| {
        format!(
            r"<w:tc><w:tcPr>{properties}</w:tcPr><w:p><w:r><w:t>{value}</w:t></w:r></w:p></w:tc>"
        )
    };
    let nested =
        r"<w:tbl><w:tr><w:tc><w:p><w:r><w:t>interna</w:t></w:r></w:p></w:tc></w:tr></w:tbl>";
    let body = [
        "<w:tbl><w:tblPr/>",
        r"<w:tr><w:trPr><w:tblHeader/></w:trPr>",
        &cell(r#"<w:gridSpan w:val="2"/>"#, "Titolo"),
        &cell("", "C"),
        "</w:tr><w:tr>",
        &cell(r#"<w:vMerge w:val="restart"/>"#, "Unito"),
        &cell("", "B2"),
        &format!("<w:tc><w:p><w:r><w:t>C2</w:t></w:r></w:p>{nested}<w:p/></w:tc>"),
        "</w:tr><w:tr>",
        &cell("<w:vMerge/>", ""),
        &cell("", "B3"),
        &cell("", "C3"),
        "</w:tr></w:tbl>",
    ]
    .concat();
    let read = tree(&body);
    let Block::Table { rows } = &read.blocks[0] else {
        panic!("not a table: {:?}", read.blocks);
    };
    assert_eq!(rows.len(), 3);
    assert!(rows[0].header);
    assert!(!rows[1].header);
    assert_eq!(rows[0].cells[0].col_span, 2);
    assert_eq!(rows[0].cells[0].blocks, vec![paragraph("Titolo")]);
    assert_eq!(rows[1].cells[0].row_span, 2);
    assert_eq!(rows[2].cells.len(), 2);
    assert_eq!(rows[2].cells[0].blocks, vec![paragraph("B3")]);
    let Block::Table { rows: inner } = &rows[1].cells[2].blocks[1] else {
        panic!("no nested table: {:?}", rows[1].cells[2].blocks);
    };
    assert_eq!(inner[0].cells[0].blocks, vec![paragraph("interna")]);
}

#[test]
fn keeps_a_continued_merge_without_an_origin_as_a_cell() {
    let body = r"<w:tbl><w:tr><w:tc><w:tcPr><w:vMerge/></w:tcPr><w:p><w:r><w:t>solo</w:t></w:r></w:p></w:tc></w:tr></w:tbl>";
    let Block::Table { rows } = &tree(body).blocks[0] else {
        panic!("not a table");
    };
    assert_eq!(rows[0].cells[0].blocks, vec![paragraph("solo")]);
}

#[test]
fn skips_a_deleted_table_row() {
    let body = concat!(
        r#"<w:tbl><w:tr><w:trPr><w:del w:id="1"/></w:trPr><w:tc><w:p><w:r><w:t>via</w:t></w:r></w:p></w:tc></w:tr>"#,
        "<w:tr><w:tc><w:p><w:r><w:t>resta</w:t></w:r></w:p></w:tc></w:tr></w:tbl>",
    );
    let read = tree(body);
    let Block::Table { rows } = &read.blocks[0] else {
        panic!("not a table");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(read.report.tracked_deletions, 1);
}

#[test]
fn groups_quotes_and_code_lines() {
    let styles = styles_xml(&[
        ("Citazione", "Quote", ""),
        ("CitazioneIntensa", "Intense Quote", ""),
        ("Codice", "Code", ""),
    ]);
    let body = [
        styled_paragraph("Citazione", "Uno"),
        styled_paragraph("CitazioneIntensa", "Due"),
        styled_paragraph("Codice", "let a = 1;"),
        r#"<w:p><w:pPr><w:pStyle w:val="Codice"/></w:pPr><w:r><w:t>a</w:t></w:r><w:r><w:br/></w:r><w:r><w:tab/><w:t>b</w:t></w:r></w:p>"#.to_owned(),
        styled_paragraph("Citazione", "Tre"),
    ]
    .concat();
    let read = tree_with(&body, &[("word/styles.xml", styles.as_bytes())]);
    assert_eq!(
        read.blocks,
        vec![
            Block::Blockquote {
                blocks: vec![paragraph("Uno"), paragraph("Due")]
            },
            Block::CodeBlock {
                text: "let a = 1;\na\n\tb".to_owned()
            },
            Block::Blockquote {
                blocks: vec![paragraph("Tre")]
            },
        ]
    );
}

#[test]
fn reads_horizontal_rules_and_ignores_page_breaks() {
    let body = concat!(
        r#"<w:p><w:r><w:pict><v:rect o:hralign="center" o:hrstd="t" o:hr="t"/></w:pict></w:r></w:p>"#,
        r#"<w:p><w:pPr><w:pBdr><w:bottom w:val="single" w:sz="6"/></w:pBdr></w:pPr></w:p>"#,
        r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#,
        r#"<w:p><w:r><w:t>prima</w:t></w:r><w:r><w:br w:type="page"/><w:t>dopo</w:t></w:r><w:r><w:br/><w:t>riga</w:t></w:r><w:r><w:cr/></w:r></w:p>"#,
    );
    assert_eq!(
        tree(body).blocks,
        vec![
            Block::HorizontalRule,
            Block::HorizontalRule,
            Block::Paragraph {
                content: vec![
                    text("primadopo"),
                    Inline::HardBreak,
                    text("riga"),
                    Inline::HardBreak
                ]
            },
        ]
    );
}

#[test]
fn returns_images_once_with_their_alt_text_size_and_type() {
    let rels = rels_xml(&[
        ("rId5", "image", "media/image1.png", false),
        ("rId6", "image", "media/image1.png", false),
        ("rId7", "image", "https://example.com/a.png", true),
    ]);
    let body = format!(
        "<w:p>{}{}{}<w:r><w:pict><v:shape style=\"width:75pt;height:30pt\"><v:imagedata r:id=\"rId6\" o:title=\"vecchia\"/></v:shape></w:pict></w:r></w:p>",
        drawing("rId5"),
        drawing("rId6"),
        drawing("rId7"),
    );
    let read = tree_with(
        &body,
        &[(RELS_PART, rels.as_bytes()), ("word/media/image1.png", PNG)],
    );
    let image = Inline::Image {
        media: 0,
        alt: Some("Il logo".to_owned()),
        width: Some(100),
        height: Some(50),
    };
    assert_eq!(
        read.blocks,
        vec![Block::Paragraph {
            content: vec![
                image.clone(),
                image,
                Inline::Image {
                    media: 0,
                    alt: Some("vecchia".to_owned()),
                    width: Some(100),
                    height: Some(40),
                },
            ]
        }]
    );
    assert_eq!(read.media.len(), 1);
    assert_eq!(read.media[0].content_type, "image/png");
    assert_eq!(read.media[0].bytes, PNG);
    assert_eq!(read.report.missing_images, 1);
}

#[test]
fn reports_images_over_the_limits_instead_of_returning_them() {
    let rels = rels_xml(&[
        ("rId1", "image", "media/a.png", false),
        ("rId2", "image", "media/b.png", false),
        ("rId3", "image", "media/c.png", false),
        ("rId4", "image", "media/assente.png", false),
    ]);
    let large = [PNG, &[0_u8; 64]].concat();
    let body = format!(
        "<w:p>{}{}{}{}</w:p>",
        drawing("rId1"),
        drawing("rId2"),
        drawing("rId3"),
        drawing("rId4")
    );
    let bytes = build_docx_from_body(
        &body,
        &[
            (RELS_PART, rels.as_bytes()),
            ("word/media/a.png", PNG),
            ("word/media/b.png", &large),
            ("word/media/c.png", PNG),
        ],
    );
    let limits = Limits {
        max_image_bytes: 32,
        max_images: 2,
        ..Limits::default()
    };
    let read = read_document_tree(&bytes, &limits).unwrap();
    assert_eq!(read.media.len(), 2);
    assert_eq!(read.report.oversized_images, 1);
    assert_eq!(read.report.images_over_limit, 1);
    assert_eq!(read.report.missing_images, 0);

    let limits = Limits {
        max_total_image_bytes: 20,
        ..Limits::default()
    };
    let read = read_document_tree(&bytes, &limits).unwrap();
    assert_eq!(read.media.len(), 1);
    assert_eq!(read.report.images_over_limit, 2);
    assert_eq!(read.report.missing_images, 1);
}

#[test]
fn counts_what_an_editor_cannot_hold() {
    let rels = rels_xml(&[
        ("rId1", "header", "header1.xml", false),
        ("rId2", "footer", "footer1.xml", false),
    ]);
    let body = [
        "<w:p>",
        &graphic(
            "http://schemas.microsoft.com/office/word/2010/wordprocessingShape",
            "<wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>casella</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp>",
        ),
        &graphic("http://schemas.microsoft.com/office/word/2010/wordprocessingShape", "<wps:wsp/>"),
        &graphic("http://schemas.openxmlformats.org/drawingml/2006/chart", ""),
        &graphic("http://schemas.openxmlformats.org/drawingml/2006/diagram", ""),
        r"<w:r><w:pict><v:shape><v:textbox><w:txbxContent><w:p/></w:txbxContent></v:textbox></v:shape></w:pict></w:r>",
        r"<w:r><w:pict><v:line/></w:pict></w:r>",
        r#"<w:r><w:object><o:OLEObject ProgID="Excel.Sheet.12"/></w:object></w:r>"#,
        r#"<w:r><w:footnoteReference w:id="1"/></w:r>"#,
        r#"<w:r><w:endnoteReference w:id="1"/></w:r>"#,
        r#"<w:commentRangeStart w:id="0"/><w:r><w:t>commentato</w:t></w:r><w:commentRangeEnd w:id="0"/>"#,
        r#"<w:r><w:commentReference w:id="0"/></w:r>"#,
        "<m:oMath><m:r><m:t>x</m:t></m:r></m:oMath>",
        r"<w:r><w:rPr><w:vanish/></w:rPr><w:t>nascosto</w:t></w:r>",
        "</w:p>",
    ]
    .concat();
    let read = tree_with(&body, &[(RELS_PART, rels.as_bytes())]);
    assert_eq!(
        read.blocks,
        vec![paragraph("commentato")],
        "only the visible body text remains"
    );
    let report = read.report;
    assert_eq!(report.text_boxes, 2);
    assert_eq!(report.shapes, 2);
    assert_eq!(report.charts, 1);
    assert_eq!(report.smart_art, 1);
    assert_eq!(report.embedded_objects, 1);
    assert_eq!(report.footnotes, 1);
    assert_eq!(report.endnotes, 1);
    assert_eq!(report.comments, 1);
    assert_eq!(report.equations, 1);
    assert_eq!(report.hidden_runs, 1);
    assert_eq!(report.headers_and_footers, 2);
}

#[test]
fn accepts_tracked_insertions_and_deletions() {
    let body = concat!(
        "<w:p><w:r><w:t xml:space=\"preserve\">Il </w:t></w:r>",
        r#"<w:ins w:id="1" w:author="A"><w:r><w:t>nuovo</w:t></w:r></w:ins>"#,
        r#"<w:del w:id="2" w:author="A"><w:r><w:delText>vecchio</w:delText></w:r></w:del>"#,
        r#"<w:moveFrom w:id="3"><w:r><w:t>spostato</w:t></w:r></w:moveFrom>"#,
        r#"<w:moveTo w:id="4"><w:r><w:t xml:space="preserve"> testo</w:t></w:r></w:moveTo>"#,
        "</w:p>",
    );
    let read = tree(body);
    assert_eq!(read.blocks, vec![paragraph("Il nuovo testo")]);
    assert_eq!(read.report.tracked_insertions, 2);
    assert_eq!(read.report.tracked_deletions, 2);
}

#[test]
fn reads_content_controls_smart_tags_and_alternate_content() {
    let body = concat!(
        "<w:sdt><w:sdtPr/><w:sdtContent><w:p><w:r><w:t>controllo</w:t></w:r></w:p></w:sdtContent></w:sdt>",
        "<w:p><w:smartTag><w:r><w:t>etichetta</w:t></w:r></w:smartTag>",
        "<w:sdt><w:sdtContent><w:r><w:t>+in linea</w:t></w:r></w:sdtContent></w:sdt>",
        "<mc:AlternateContent><mc:Choice Requires=\"wps\"><w:r><w:t>+scelta</w:t></w:r></mc:Choice>",
        "<mc:Fallback><w:r><w:t>+ripiego</w:t></w:r></mc:Fallback></mc:AlternateContent></w:p>",
    );
    assert_eq!(
        tree(body).blocks,
        vec![
            paragraph("controllo"),
            paragraph("etichetta+in linea+scelta")
        ]
    );
}

#[test]
fn stops_at_the_character_limit_and_says_so() {
    let bytes = build_docx_from_body(
        "<w:p><w:r><w:t>abcdef</w:t></w:r></w:p><w:p><w:r><w:t>ghi</w:t></w:r></w:p>",
        &[],
    );
    let limits = Limits {
        max_text_chars: 4,
        ..Limits::default()
    };
    let read = read_document_tree(&bytes, &limits).unwrap();
    assert_eq!(read.blocks, vec![paragraph("abcd")]);
    assert!(read.report.truncated);

    let limits = Limits {
        max_text_chars: 6,
        ..Limits::default()
    };
    let read = read_document_tree(&bytes, &limits).unwrap();
    assert_eq!(read.blocks, vec![paragraph("abcdef")]);
    assert!(read.report.truncated);

    let limits = Limits {
        max_text_chars: 9,
        ..Limits::default()
    };
    assert!(
        !read_document_tree(&bytes, &limits)
            .unwrap()
            .report
            .truncated
    );
}

#[test]
fn follows_the_main_part_the_package_declares() {
    let document = format!(
        r#"<w:document xmlns:w="{}"><w:body><w:p><w:r><w:t>altrove</w:t></w:r></w:p></w:body></w:document>"#,
        "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
    );
    let root_rels = rels_xml(&[("rId1", "officeDocument", "word/main.xml", false)]);
    let bytes = build_docx_from_body(
        "",
        &[
            ("_rels/.rels", root_rels.as_bytes()),
            ("word/main.xml", document.as_bytes()),
        ],
    );
    let read = read_document_tree(&bytes, &Limits::default()).unwrap();
    assert_eq!(read.blocks, vec![paragraph("altrove")]);
}

#[test]
fn refuses_what_is_not_a_docx() {
    let error = read_document_tree(&crate::test_support::build_xlsx(), &Limits::default());
    assert!(matches!(
        error.unwrap_err(),
        OfficeError::UnsupportedFormat(DocumentFormat::Xlsx)
    ));
    assert!(matches!(
        read_document_tree(b"PK\x03\x04broken", &Limits::default()).unwrap_err(),
        OfficeError::UnsupportedFormat(DocumentFormat::Unknown)
    ));
}

#[test]
fn refuses_inputs_and_parts_above_the_limits() {
    let bytes = build_docx_from_body("<w:p/>", &[]);
    let limits = Limits {
        max_input_bytes: 10,
        ..Limits::default()
    };
    assert!(matches!(
        read_document_tree(&bytes, &limits).unwrap_err(),
        OfficeError::TooLarge { .. }
    ));
    let limits = Limits {
        max_member_bytes: 64,
        ..Limits::default()
    };
    assert!(matches!(
        read_document_tree(&bytes, &limits).unwrap_err(),
        OfficeError::TooLarge { .. }
    ));
}

#[test]
fn refuses_malformed_document_xml() {
    let content_types = br#"<Types><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
    let broken = build_zip(&[
        ("[Content_Types].xml", content_types.as_slice()),
        (
            "word/document.xml",
            b"<w:document><w:body></w:document>".as_slice(),
        ),
    ]);
    assert!(matches!(
        read_document_tree(&broken, &Limits::default()).unwrap_err(),
        OfficeError::MalformedXml(_)
    ));

    let no_body = build_zip(&[
        ("[Content_Types].xml", content_types.as_slice()),
        ("word/document.xml", b"<w:document/>".as_slice()),
    ]);
    assert!(matches!(
        read_document_tree(&no_body, &Limits::default()).unwrap_err(),
        OfficeError::UnreadableDocument(_)
    ));

    let no_part = build_zip(&[("[Content_Types].xml", content_types.as_slice())]);
    assert!(matches!(
        read_document_tree(&no_part, &Limits::default()).unwrap_err(),
        OfficeError::UnreadableDocument(_)
    ));

    let bad_styles =
        build_docx_from_body("<w:p/>", &[("word/styles.xml", b"<w:styles>".as_slice())]);
    assert!(matches!(
        read_document_tree(&bad_styles, &Limits::default()).unwrap_err(),
        OfficeError::MalformedXml(_)
    ));
}
