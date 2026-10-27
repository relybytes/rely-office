// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Headers, footers, notes, text boxes, table styles and the looks a renderer draws.

use super::*;

const WORD: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#;

fn part(root: &str, inner: &str) -> String {
    format!(r#"<?xml version="1.0" encoding="UTF-8"?><w:{root} {WORD}>{inner}</w:{root}>"#)
}

fn run(value: &str) -> String {
    format!(r#"<w:r><w:t xml:space="preserve">{value}</w:t></w:r>"#)
}

fn plain(blocks: &[Block]) -> Vec<String> {
    blocks
        .iter()
        .map(|block| match block {
            Block::Paragraph { content, .. } | Block::Heading { content, .. } => {
                plain_text(content)
            }
            other => format!("{other:?}"),
        })
        .collect()
}

fn plain_text(content: &[Inline]) -> String {
    content
        .iter()
        .map(|inline| match inline {
            Inline::Text { text, .. } => text.clone(),
            Inline::NoteReference { note } => format!("[{note}]"),
            _ => String::new(),
        })
        .collect()
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one document exercises every case together"
)]
fn reads_the_headers_and_footers_of_the_last_section() {
    let rels = rels_xml(&[
        ("rId1", "header", "header1.xml", false),
        ("rId2", "header", "header2.xml", false),
        ("rId3", "footer", "footer1.xml", false),
        ("rId4", "footer", "footer2.xml", false),
        ("rId5", "settings", "settings.xml", false),
        ("rId6", "header", "missing.xml", false),
        ("rId7", "hyperlink", "https://example.com", true),
    ]);
    let page = concat!(
        r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> PAGE </w:instrText></w:r>"#,
        r#"<w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>4</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>"#
    );
    let header = part(
        "hdr",
        &format!(
            r#"<w:p>{}<w:hyperlink r:id="rId1"><w:r><w:t>sito</w:t></w:r></w:hyperlink></w:p>"#,
            run("Intestazione ")
        ),
    );
    let header_rels = rels_xml(&[("rId1", "hyperlink", "https://relybytes.com", true)]);
    let first = part("hdr", &format!("<w:p>{}</w:p>", run("Copertina")));
    let footer = part(
        "ftr",
        &format!(
            r#"<w:p>{}{page}{}<w:fldSimple w:instr="NUMPAGES"/></w:p>"#,
            run("Pagina "),
            run(" di ")
        ),
    );
    let even = part("ftr", &format!("<w:p>{}</w:p>", run("Pari")));
    let settings = part(
        "settings",
        r#"<w:evenAndOddHeaders/><w:defaultTabStop w:val="720"/>"#,
    );
    let body = concat!(
        "<w:p><w:r><w:t>Corpo</w:t></w:r></w:p>",
        r#"<w:sectPr><w:headerReference w:type="default" r:id="rId1"/><w:headerReference w:type="first" r:id="rId2"/>"#,
        r#"<w:headerReference w:type="even" r:id="rId6"/><w:headerReference w:type="default" r:id="rId2"/>"#,
        r#"<w:footerReference w:type="default" r:id="rId3"/><w:footerReference w:type="even" r:id="rId4"/>"#,
        r#"<w:footerReference r:id="rId7"/><w:footerReference w:type="first"/>"#,
        r#"<w:pgMar w:top="1417" w:right="1134" w:bottom="1134" w:left="1134" w:header="500" w:footer="400"/>"#,
        r#"<w:cols w:num="2" w:space="360"/><w:titlePg/></w:sectPr>"#
    );
    let read = tree_with(
        body,
        &[
            (RELS_PART, rels.as_bytes()),
            ("word/header1.xml", header.as_bytes()),
            ("word/_rels/header1.xml.rels", header_rels.as_bytes()),
            ("word/header2.xml", first.as_bytes()),
            ("word/footer1.xml", footer.as_bytes()),
            ("word/footer2.xml", even.as_bytes()),
            ("word/settings.xml", settings.as_bytes()),
        ],
    );
    let parts = &read.headers_footers;
    assert_eq!(plain(&read.blocks), vec!["Corpo"]);
    assert_eq!(plain(&parts.header), vec!["Intestazione sito"]);
    let Block::Paragraph { content, .. } = &parts.header[0] else {
        panic!("not a paragraph");
    };
    assert!(matches!(&content[1], Inline::Text { marks, .. }
        if marks == &vec![Mark::Link { href: "https://relybytes.com".to_owned() }]));
    assert_eq!(
        parts.first_header.as_deref().map(plain),
        Some(vec!["Copertina".to_owned()])
    );
    assert_eq!(
        parts.even_header,
        Some(Vec::new()),
        "a missing part leaves the slot empty"
    );
    assert_eq!(plain(&parts.footer), vec!["Pagina 4 di 1"]);
    let Block::Paragraph { content, .. } = &parts.footer[0] else {
        panic!("not a paragraph");
    };
    let fields: Vec<Option<PageField>> = content
        .iter()
        .map(|inline| match inline {
            Inline::Text { look, .. } => look.field,
            _ => None,
        })
        .collect();
    assert_eq!(
        fields,
        vec![None, Some(PageField::Page), None, Some(PageField::Pages)]
    );
    assert_eq!(
        parts.even_footer.as_deref().map(plain),
        Some(vec!["Pari".to_owned()])
    );
    assert_eq!(parts.first_footer, Some(Vec::new()));
    assert!(!parts.is_empty());
    assert_eq!(
        (
            read.layout.header_distance_twips,
            read.layout.footer_distance_twips,
            read.layout.columns,
            read.layout.column_gap_twips,
            read.layout.default_tab_twips
        ),
        (500, 400, 2, 360, 720)
    );
    assert_eq!(read.report.headers_and_footers, 5);
}

#[test]
fn has_no_header_variants_the_document_does_not_turn_on() {
    let rels = rels_xml(&[("rId1", "header", "header1.xml", false)]);
    let header = part("hdr", "<w:p><w:r><w:t>Testa</w:t></w:r></w:p>");
    let read = tree_with(
        r#"<w:p/><w:sectPr><w:headerReference w:type="first" r:id="rId1"/></w:sectPr>"#,
        &[
            (RELS_PART, rels.as_bytes()),
            ("word/header1.xml", header.as_bytes()),
        ],
    );
    assert_eq!(read.headers_footers, HeadersFooters::default());
    assert!(read.headers_footers.is_empty());
    assert_eq!(tree("<w:p/>").headers_footers, HeadersFooters::default());
}

#[test]
fn reads_footnotes_and_endnotes_where_the_text_refers_to_them() {
    let rels = rels_xml(&[
        ("rId1", "footnotes", "footnotes.xml", false),
        ("rId2", "endnotes", "endnotes.xml", false),
    ]);
    let footnotes = part(
        "footnotes",
        concat!(
            r#"<w:footnote w:type="separator" w:id="-1"><w:p><w:r><w:separator/></w:r></w:p></w:footnote>"#,
            r#"<w:footnote w:id="2"><w:p><w:r><w:footnoteRef/></w:r><w:r><w:t xml:space="preserve"> Seconda</w:t></w:r>"#,
            r#"<w:r><w:footnoteReference w:id="1"/></w:r></w:p></w:footnote>"#,
            r#"<w:footnote w:id="1"><w:p><w:hyperlink r:id="rId1"><w:r><w:t>Prima</w:t></w:r></w:hyperlink></w:p></w:footnote>"#
        ),
    );
    let footnote_rels = rels_xml(&[("rId1", "hyperlink", "https://example.com", true)]);
    let endnotes = part(
        "endnotes",
        r#"<w:endnote w:id="1"><w:p><w:r><w:t>Finale</w:t></w:r></w:p></w:endnote>"#,
    );
    let body = concat!(
        r#"<w:p><w:r><w:t>Uno</w:t></w:r><w:r><w:footnoteReference w:id="2"/></w:r>"#,
        r#"<w:r><w:t xml:space="preserve"> due</w:t></w:r><w:r><w:footnoteReference w:id="1"/></w:r>"#,
        r#"<w:r><w:footnoteReference w:id="2"/></w:r><w:r><w:endnoteReference w:id="1"/></w:r>"#,
        r#"<w:r><w:footnoteReference w:id="-1"/></w:r><w:r><w:footnoteReference w:id="9"/></w:r>"#,
        r#"<w:r><w:footnoteReference/></w:r></w:p>"#
    );
    let read = tree_with(
        body,
        &[
            (RELS_PART, rels.as_bytes()),
            ("word/footnotes.xml", footnotes.as_bytes()),
            ("word/_rels/footnotes.xml.rels", footnote_rels.as_bytes()),
            ("word/endnotes.xml", endnotes.as_bytes()),
        ],
    );
    assert_eq!(plain(&read.blocks), vec!["Uno[0] due[1][0][2]"]);
    assert_eq!(read.notes.len(), 3);
    assert_eq!(
        (read.notes[0].kind, plain(&read.notes[0].blocks)),
        (NoteKind::Footnote, vec![" Seconda".to_owned()]),
        "a reference inside a note is not followed"
    );
    assert_eq!(plain(&read.notes[1].blocks), vec!["Prima"]);
    let Block::Paragraph { content, .. } = &read.notes[1].blocks[0] else {
        panic!("not a paragraph");
    };
    assert!(matches!(&content[0], Inline::Text { marks, .. }
        if marks == &vec![Mark::Link { href: "https://example.com".to_owned() }]));
    assert_eq!(
        (read.notes[2].kind, plain(&read.notes[2].blocks)),
        (NoteKind::Endnote, vec!["Finale".to_owned()])
    );
    assert_eq!(read.report.footnotes, 7);
    assert_eq!(read.report.endnotes, 1);

    let damaged = tree_with(
        r#"<w:p><w:r><w:footnoteReference w:id="1"/></w:r></w:p>"#,
        &[
            (RELS_PART, rels.as_bytes()),
            ("word/footnotes.xml", b"<w:footnotes"),
        ],
    );
    assert_eq!(damaged.notes, []);
    assert_eq!(plain(&damaged.blocks), vec![""]);
}

#[test]
fn places_the_content_of_text_boxes_after_their_paragraph() {
    let text_box = |value: &str| {
        format!(
            concat!(
                r#"<w:r><w:pict><v:shape><v:textbox><w:txbxContent>"#,
                "<w:p><w:r><w:t>{}</w:t></w:r></w:p>",
                "</w:txbxContent></v:textbox></v:shape></w:pict></w:r>"
            ),
            value
        )
    };
    let body = format!(
        "<w:p>{}{}{}</w:p><w:p>{}</w:p>",
        run("Prima"),
        text_box("Casella A"),
        text_box("Casella B"),
        run("Dopo")
    );
    let read = tree(&body);
    assert_eq!(
        plain(&read.blocks),
        vec!["Prima", "Casella A", "Casella B", "Dopo"]
    );
    assert_eq!(read.report.text_boxes, 2);
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one document exercises every case together"
)]
fn gives_table_cells_the_formats_of_their_table_style() {
    let styles = styles_xml(&[("Normale", "Normal", "")])
        .replace(
            r#"w:styleId="Normale""#,
            r#"w:styleId="Normale" w:default="1""#,
        )
        .replace(
            "</w:styles>",
            concat!(
                r#"<w:docDefaults><w:pPrDefault><w:pPr><w:spacing w:after="160"/></w:pPr></w:pPrDefault></w:docDefaults>"#,
                r#"<w:style w:type="table" w:styleId="Base"><w:name w:val="Base"/><w:tblPr>"#,
                r#"<w:tblBorders><w:insideH w:val="single" w:sz="4" w:color="8EAADB"/></w:tblBorders>"#,
                r#"<w:tblCellMar><w:left w:w="50"/></w:tblCellMar></w:tblPr>"#,
                r#"<w:tblStylePr w:type="band1Horz"><w:tcPr><w:shd w:val="clear" w:fill="D9E2F3"/></w:tcPr></w:tblStylePr></w:style>"#,
                r#"<w:style w:type="table" w:styleId="Griglia"><w:name w:val="Grid Table 4"/><w:basedOn w:val="Base"/>"#,
                r#"<w:pPr><w:spacing w:after="0"/></w:pPr>"#,
                r#"<w:tblStylePr w:type="firstRow"><w:rPr><w:b/><w:color w:val="FFFFFF"/></w:rPr>"#,
                r#"<w:tcPr><w:shd w:val="clear" w:fill="4472C4"/></w:tcPr></w:tblStylePr>"#,
                r#"<w:tblStylePr w:type="lastRow"><w:tcPr><w:shd w:val="clear" w:fill="EEEEEE"/></w:tcPr></w:tblStylePr>"#,
                r#"<w:tblStylePr w:type="lastCol"><w:rPr><w:i/></w:rPr></w:tblStylePr>"#,
                r#"<w:tblStylePr w:type="band1Vert"><w:tcPr><w:vAlign w:val="bottom"/></w:tcPr></w:tblStylePr>"#,
                r#"<w:tblStylePr w:type="nwCell"/></w:style></w:styles>"#
            ),
        );
    let cell = |value: &str, properties: &str| {
        format!(
            "<w:tc><w:tcPr>{properties}</w:tcPr><w:p>{}</w:p></w:tc>",
            run(value)
        )
    };
    let row = |cells: &[String]| format!("<w:tr>{}</w:tr>", cells.concat());
    let body = format!(
        concat!(
            r#"<w:tbl><w:tblPr><w:tblStyle w:val="Griglia"/>"#,
            r#"<w:tblLook w:firstRow="1" w:lastRow="1" w:firstColumn="0" w:lastColumn="1" w:noHBand="0" w:noVBand="0"/></w:tblPr>"#,
            r#"<w:tblGrid><w:gridCol w:w="1000"/><w:gridCol w:w="1000"/></w:tblGrid>{}{}{}{}</w:tbl>"#
        ),
        row(&[cell("A", ""), cell("B", "")]),
        row(&[
            cell("C", r#"<w:shd w:val="clear" w:fill="FF0000"/>"#),
            cell("D", ""),
        ]),
        row(&[cell("E", ""), cell("F", "")]),
        row(&[cell("G", ""), cell("H", "")]),
    );
    let read = tree_with(&body, &[("word/styles.xml", styles.as_bytes())]);
    let Block::Table { rows, look } = &read.blocks[0] else {
        panic!("not a table: {:?}", read.blocks[0]);
    };
    assert!(look.borders);
    assert!(look.edges.inside_horizontal.is_some());
    assert_eq!(look.cell_margins_twips[1], Some(50));
    let fills: Vec<Vec<Option<&str>>> = rows
        .iter()
        .map(|row| {
            row.cells
                .iter()
                .map(|cell| cell.look.fill.as_deref())
                .collect()
        })
        .collect();
    assert_eq!(
        fills,
        vec![
            vec![Some("4472C4"), Some("4472C4")],
            vec![Some("FF0000"), Some("D9E2F3")],
            vec![None, None],
            vec![Some("EEEEEE"), Some("EEEEEE")],
        ]
    );
    assert_eq!(
        rows[1].cells[0].look.vertical,
        Some(VerticalAlignment::Bottom)
    );
    assert_eq!(rows[1].cells[1].look.vertical, None);
    let font = |row: usize, column: usize| match &rows[row].cells[column].blocks[0] {
        Block::Paragraph { look, .. } => look.clone(),
        other => panic!("not a paragraph: {other:?}"),
    };
    assert_eq!(font(0, 0).font.bold, Some(true));
    assert_eq!(font(0, 0).font.colour.as_deref(), Some("FFFFFF"));
    assert_eq!(font(1, 0).font.bold, None);
    assert_eq!(font(1, 1).font.italic, Some(true), "the last column");
    assert_eq!(
        font(2, 0).space_after_twips,
        Some(0),
        "the table style's spacing sits over the document defaults"
    );

    let default_table = styles.replace(r#"w:styleId="Base""#, r#"w:styleId="Base" w:default="1""#);
    let read = tree_with(
        "<w:tbl><w:tblGrid><w:gridCol/></w:tblGrid><w:tr><w:tc><w:p/></w:tc></w:tr><w:tr><w:tc><w:p/></w:tc></w:tr></w:tbl>",
        &[("word/styles.xml", default_table.as_bytes())],
    );
    let Block::Table { rows, look } = &read.blocks[0] else {
        panic!("not a table");
    };
    assert!(
        look.edges.inside_horizontal.is_some(),
        "the default table style"
    );
    assert_eq!(
        rows[1].cells[0].look.fill.as_deref(),
        Some("D9E2F3"),
        "without a look of its own a table bands its rows, as in Word"
    );
}

#[test]
fn reads_the_borders_shading_tabs_and_highlights_of_paragraphs_and_runs() {
    let body = concat!(
        r#"<w:p><w:pPr><w:pBdr><w:top w:val="single" w:sz="8"/></w:pBdr><w:shd w:val="clear" w:fill="DEEAF6"/>"#,
        r#"<w:tabs><w:tab w:val="right" w:leader="dot" w:pos="9000"/></w:tabs></w:pPr>"#,
        r#"<w:r><w:rPr><w:highlight w:val="yellow"/><w:caps/></w:rPr><w:t>giallo</w:t></w:r>"#,
        r#"<w:r><w:rPr><w:shd w:val="clear" w:fill="00FF00"/><w:smallCaps/><w:rFonts w:ascii="Arial"/></w:rPr><w:t>verde</w:t></w:r></w:p>"#
    );
    let read = tree(body);
    let Block::Paragraph { look, content, .. } = &read.blocks[0] else {
        panic!("not a paragraph");
    };
    assert_eq!(look.shading.as_deref(), Some("DEEAF6"));
    assert!(look.borders.top.as_ref().is_some_and(Border::is_visible));
    assert_eq!(
        look.tabs,
        vec![TabStop {
            position_twips: 9_000,
            kind: TabKind::Right,
            leader: TabLeader::Dot
        }]
    );
    let Inline::Text { look, marks, .. } = &content[0] else {
        panic!("not text");
    };
    assert_eq!(
        (look.highlight.as_deref(), look.caps),
        (Some("FFFF00"), Some(true))
    );
    assert_eq!(
        marks,
        &vec![Mark::Highlight {
            color: "#FFFF00".to_owned()
        }]
    );
    let Inline::Text { look, marks, .. } = &content[1] else {
        panic!("not text");
    };
    assert_eq!(
        (look.highlight.as_deref(), look.small_caps),
        (Some("00FF00"), Some(true))
    );
    assert_eq!(
        marks,
        &vec![
            Mark::TextStyle {
                font_family: Some("Arial".to_owned()),
                font_size: None,
                color: None
            },
            Mark::Highlight {
                color: "#00FF00".to_owned()
            }
        ]
    );
}

#[test]
fn labels_list_paragraphs_and_numbered_headings_as_the_numbering_part_says() {
    let numbering = concat!(
        r#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
        r#"<w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:start w:val="3"/><w:numFmt w:val="decimal"/>"#,
        r#"<w:lvlText w:val="Art. %1."/><w:suff w:val="space"/><w:rPr><w:b/></w:rPr></w:lvl>"#,
        r#"<w:lvl w:ilvl="1"><w:numFmt w:val="lowerLetter"/><w:lvlText w:val="%1.%2)"/><w:suff w:val="nothing"/></w:lvl></w:abstractNum>"#,
        r#"<w:abstractNum w:abstractNumId="1"><w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/>"#,
        r#"<w:lvlText w:val="&#xF0B7;"/><w:rPr><w:rFonts w:ascii="Symbol"/></w:rPr></w:lvl></w:abstractNum>"#,
        r#"<w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>"#,
        r#"<w:num w:numId="2"><w:abstractNumId w:val="0"/><w:lvlOverride w:ilvl="0"><w:startOverride w:val="1"/></w:lvlOverride></w:num>"#,
        r#"<w:num w:numId="3"><w:abstractNumId w:val="1"/></w:num></w:numbering>"#
    );
    let styles = styles_xml(&[("Titolo1", "heading 1", "")]);
    let item = |num_id: u32, level: u8| {
        format!(
            r#"<w:p><w:pPr><w:pStyle w:val="Titolo1"/><w:numPr><w:ilvl w:val="{level}"/><w:numId w:val="{num_id}"/></w:numPr></w:pPr>{}</w:p>"#,
            run("x")
        )
    };
    let bullet = concat!(
        r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="3"/></w:numPr></w:pPr>"#,
        r#"<w:r><w:t>punto</w:t></w:r></w:p>"#
    );
    let body = [
        item(1, 0),
        item(1, 1),
        item(1, 1),
        item(1, 0),
        item(2, 0),
        bullet.to_owned(),
    ]
    .concat();
    let read = tree_with(
        &body,
        &[
            ("word/numbering.xml", numbering.as_bytes()),
            ("word/styles.xml", styles.as_bytes()),
        ],
    );
    let labels: Vec<ListLabel> = read
        .blocks
        .iter()
        .map(|block| match block {
            Block::Heading { look, .. } | Block::ListItem { look, .. } => {
                look.list_label.clone().expect("a label")
            }
            other => panic!("not a numbered block: {other:?}"),
        })
        .collect();
    let texts: Vec<&str> = labels.iter().map(|label| label.text.as_str()).collect();
    assert_eq!(
        texts,
        vec!["Art. 3.", "3.a)", "3.b)", "Art. 4.", "Art. 1.", "\u{2022}"]
    );
    assert_eq!(labels[0].suffix, LabelSuffix::Space);
    assert_eq!(labels[0].font.bold, Some(true));
    assert_eq!(labels[1].suffix, LabelSuffix::Nothing);
    assert_eq!(labels[5].suffix, LabelSuffix::Tab);
    assert!(matches!(read.blocks[0], Block::Heading { level: 1, .. }));
}
