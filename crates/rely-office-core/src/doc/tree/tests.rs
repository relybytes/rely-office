// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;
use crate::doc::fib::pair;
use crate::doc::test_doc::{
    DELAY_OFFSET, DocBuilder, LevelSpec, definition, lists, overrides, plex as plex_of, png_blip,
    record, sheet, store_entry,
};
use crate::document::{BorderStyle, plain_text};
use crate::read_document_tree;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nimmagine";

/// The normal style in font 1 at eleven points with 200 twips after each paragraph, the first
/// heading, the second heading, a character style that turns bold on, and a list style with
/// contextual spacing.
fn styles() -> Vec<u8> {
    sheet(
        &[
            Some(definition(
                0,
                1,
                0x0FFF,
                "Normal",
                &[
                    &[0, 0, 0x14, 0xA4, 0xC8, 0x00],
                    &[0x4F, 0x4A, 0x01, 0x00, 0x43, 0x4A, 0x16, 0x00],
                ],
            )),
            Some(definition(
                1,
                1,
                0,
                "heading 1",
                &[&[0, 0], &[0x35, 0x08, 0x01, 0x43, 0x4A, 0x20, 0x00]],
            )),
            Some(definition(2, 1, 0, "heading 2", &[&[0, 0], &[]])),
            Some(definition(65, 2, 0x0FFF, "Forte", &[&[0x35, 0x08, 0x81]])),
            Some(definition(
                30,
                1,
                0,
                "Elenco",
                &[&[0, 0, 0x6D, 0x24, 0x01], &[]],
            )),
        ],
        0,
    )
}

fn styled(main: &str) -> DocBuilder {
    let mut builder = DocBuilder::new(main);
    builder.styles = styles();
    builder.fonts = vec!["Times New Roman", "Cambria", "Arial"];
    builder
}

fn read(builder: &DocBuilder) -> DocumentTree {
    read_doc_tree(&builder.build(), &Limits::default(), TreeOptions::default()).unwrap()
}

fn at(text: &str, needle: &str) -> usize {
    text.find(needle).unwrap()
}

fn texts(blocks: &[Block]) -> Vec<String> {
    blocks
        .iter()
        .map(|block| match block {
            Block::Paragraph { content, .. }
            | Block::Heading { content, .. }
            | Block::ListItem { content, .. } => plain_text(content),
            Block::Table { rows, .. } => format!("table of {}", rows.len()),
            other => format!("{other:?}"),
        })
        .collect()
}

fn content(block: &Block) -> &[Inline] {
    match block {
        Block::Paragraph { content, .. }
        | Block::Heading { content, .. }
        | Block::ListItem { content, .. } => content,
        other => panic!("not a paragraph: {other:?}"),
    }
}

fn paragraph_look(block: &Block) -> &ParagraphLook {
    match block {
        Block::Paragraph { look, .. }
        | Block::Heading { look, .. }
        | Block::ListItem { look, .. } => look,
        other => panic!("not a paragraph: {other:?}"),
    }
}

fn pieces(content: &[Inline]) -> Vec<String> {
    content
        .iter()
        .map(|inline| match inline {
            Inline::Text { text, .. } => text.clone(),
            Inline::HardBreak => "<br>".to_owned(),
            Inline::Image { .. } => "<image>".to_owned(),
            Inline::NoteReference { note } => format!("<note {note}>"),
            Inline::CommentAnchor { comment } => format!("<comment {comment}>"),
            Inline::Math { latex, .. } => format!("<math {latex}>"),
        })
        .collect()
}

fn marks(inline: &Inline) -> &[Mark] {
    match inline {
        Inline::Text { marks, .. } => marks,
        other => panic!("not text: {other:?}"),
    }
}

#[test]
fn reads_paragraphs_with_their_styles_and_runs() {
    let main = "Titolo\rUn testo forte e rosso.\r";
    let builder = styled(main)
        .paragraph(at(main, "\r"), 1, &[])
        .paragraph(
            main.len() - 1,
            0,
            &[
                0x61, 0x24, 0x03, 0x5E, 0x84, 0xD0, 0x02, 0x13, 0xA4, 0x78, 0x00, 0x12, 0x64, 0xF0,
                0x00, 0x00, 0x00, 0x07, 0x24, 0x01, 0x26, 0x64, 0x08, 0x01, 0x06, 0x00,
            ],
        )
        .character(
            at(main, "forte")..at(main, "forte") + 5,
            &[0x30, 0x4A, 0x03, 0x00],
        )
        .character(at(main, "e r")..at(main, "e r") + 1, &[0x36, 0x08, 0x01])
        .character(
            at(main, "rosso")..at(main, "rosso") + 5,
            &[
                0x42, 0x2A, 0x06, 0x4F, 0x4A, 0x02, 0x00, 0x43, 0x4A, 0x18, 0x00,
            ],
        );
    let tree = read(&builder);
    assert_eq!(
        texts(&tree.blocks),
        vec!["Titolo", "Un testo forte e rosso."]
    );
    let Block::Heading { level, look, .. } = &tree.blocks[0] else {
        panic!("a heading");
    };
    assert_eq!(*level, 1);
    assert_eq!(
        (look.font.bold, look.font.size_half_points),
        (Some(true), Some(32))
    );
    let Block::Paragraph {
        content,
        align,
        look,
    } = &tree.blocks[1]
    else {
        panic!("a paragraph");
    };
    assert_eq!(*align, Some(Alignment::Justify));
    assert_eq!(look.indent_left_twips, Some(720));
    assert_eq!(look.space_before_twips, Some(120));
    assert_eq!(look.space_after_twips, Some(200), "from the normal style");
    assert_eq!(look.line_spacing, Some(LineSpacing::AtLeast(240)));
    assert!(look.page_break_before);
    assert_eq!(
        look.borders.bottom.as_ref().map(|border| border.style),
        Some(BorderStyle::Single)
    );
    assert_eq!(
        (look.font.family.as_deref(), look.font.size_half_points),
        (Some("Cambria"), Some(22))
    );
    assert_eq!(
        pieces(content),
        vec!["Un testo ", "forte", " ", "e", " ", "rosso", "."]
    );
    assert_eq!(
        marks(&content[1]),
        &[Mark::Bold],
        "the character style turns bold on"
    );
    assert_eq!(marks(&content[3]), &[Mark::Italic]);
    assert_eq!(
        marks(&content[5]),
        &[Mark::TextStyle {
            font_family: Some("Arial".to_owned()),
            font_size: Some("12pt".to_owned()),
            color: Some("#FF0000".to_owned()),
        }]
    );
    assert_eq!(tree.layout.font_family.as_deref(), Some("Cambria"));
    assert_eq!(tree.layout.font_size_half_points, Some(22));
    assert_eq!(tree.sections, Vec::<Section>::new());
}

#[test]
fn reads_headings_from_outline_levels_and_alignments() {
    let main = "Uno\rDue\rTre\rQuattro\r";
    let marks_at: Vec<usize> = main.match_indices('\r').map(|(index, _)| index).collect();
    let builder = styled(main)
        .paragraph(marks_at[0], 2, &[0x61, 0x24, 0x01])
        .paragraph(marks_at[1], 0, &[0x40, 0x26, 0x02, 0x61, 0x24, 0x02])
        .paragraph(marks_at[2], 0, &[0x61, 0x24, 0x00, 0x2D, 0x44, 0xC0, 0x00]);
    let tree = read(&builder);
    assert!(matches!(
        tree.blocks[0],
        Block::Heading {
            level: 2,
            align: Some(Alignment::Center),
            ..
        }
    ));
    assert!(matches!(
        tree.blocks[1],
        Block::Heading {
            level: 3,
            align: Some(Alignment::Right),
            ..
        }
    ));
    let Block::Paragraph { align, look, .. } = &tree.blocks[2] else {
        panic!("a paragraph");
    };
    assert_eq!(*align, None);
    assert_eq!(look.shading.as_deref(), Some("FF0000"));
    assert!(matches!(tree.blocks[3], Block::Paragraph { .. }));
}

#[test]
fn numbers_lists_and_spaces_their_items_together() {
    let main = "Primo\rSecondo\rDentro\rTerzo\rPunto\r";
    let marks_at: Vec<usize> = main.match_indices('\r').map(|(index, _)| index).collect();
    let numbered: Vec<LevelSpec> = (0..9)
        .map(|level| LevelSpec {
            start: 1,
            format: if level == 1 { 4 } else { 0 },
            template: if level == 1 {
                vec![0, u16::from(b'.'), 1, u16::from(b')')]
            } else {
                vec![u16::try_from(level).unwrap(), u16::from(b'.')]
            },
            follow: 0,
            paragraph: vec![0x5E, 0x84, 0xD0, 0x02, 0x60, 0x84, 0x98, 0xFE],
            character: vec![0x4F, 0x4A, 0x02, 0x00],
        })
        .collect();
    let bullet = vec![LevelSpec {
        start: 1,
        format: 23,
        template: vec![0xF0B7],
        follow: 1,
        paragraph: Vec::new(),
        character: Vec::new(),
    }];
    let builder = styled(main)
        .structure(
            pair::LISTS,
            lists(&[(10, false, numbered), (20, true, bullet)]),
        )
        .structure(
            pair::LIST_OVERRIDES,
            overrides(&[(10, vec![(0, Some(4), None)]), (20, Vec::new())]),
        )
        .paragraph(marks_at[0], 4, &[0x0B, 0x46, 0x01, 0x00])
        .paragraph(marks_at[1], 4, &[0x0B, 0x46, 0x01, 0x00])
        .paragraph(marks_at[2], 4, &[0x0B, 0x46, 0x01, 0x00, 0x0A, 0x26, 0x01])
        .paragraph(marks_at[3], 4, &[0x0B, 0x46, 0x01, 0x00])
        .paragraph(marks_at[4], 0, &[0x0B, 0x46, 0x02, 0x00]);
    let tree = read(&builder);
    let labels: Vec<Option<String>> = tree
        .blocks
        .iter()
        .map(|block| {
            paragraph_look(block)
                .list_label
                .as_ref()
                .map(|label| label.text.clone())
        })
        .collect();
    assert_eq!(
        texts(&tree.blocks),
        vec!["Primo", "Secondo", "Dentro", "Terzo", "Punto"]
    );
    assert_eq!(
        labels,
        vec![
            Some("4.".to_owned()),
            Some("5.".to_owned()),
            Some("5.a)".to_owned()),
            Some("6.".to_owned()),
            Some("\u{2022}".to_owned())
        ]
    );
    let Block::ListItem {
        ordered,
        level,
        list_id,
        look,
        ..
    } = &tree.blocks[2]
    else {
        panic!("a list item");
    };
    assert!(*ordered);
    assert_eq!((*level, *list_id), (1, 1));
    assert_eq!(
        (look.indent_left_twips, look.indent_first_line_twips),
        (Some(720), Some(-360))
    );
    let label = look.list_label.as_ref().unwrap();
    assert_eq!(
        (label.suffix, label.font.family.as_deref()),
        (LabelSuffix::Tab, Some("Arial"))
    );
    assert!(matches!(
        tree.blocks[4],
        Block::ListItem { ordered: false, .. }
    ));
    assert_eq!(
        paragraph_look(&tree.blocks[0]).space_after_twips,
        Some(0),
        "contextual spacing"
    );
    assert_eq!(paragraph_look(&tree.blocks[1]).space_before_twips, Some(0));
    assert_eq!(paragraph_look(&tree.blocks[4]).space_before_twips, None);
}

/// A `sprmTDefTable` of cells with these edges.
fn table_definition(edges: &[i16], merged: &[usize]) -> Vec<u8> {
    let count = edges.len() - 1;
    let mut operand = vec![u8::try_from(count).unwrap()];
    for edge in edges {
        operand.extend_from_slice(&edge.to_le_bytes());
    }
    for index in 0..count {
        let mut cell = vec![0_u8; 20];
        if merged.contains(&index) {
            cell[0] = 0x03;
        }
        cell[4..8].copy_from_slice(&[4, 1, 0, 0]);
        operand.extend(cell);
    }
    let mut grpprl = vec![0x08, 0xD6];
    grpprl.extend_from_slice(&u16::try_from(operand.len() + 1).unwrap().to_le_bytes());
    grpprl.extend(operand);
    grpprl
}

#[test]
fn gathers_tables_from_their_cells_and_rows() {
    let main = "Prima\rA\u{7}B\u{7}\u{7}C\u{7}D\u{7}\u{7}Dopo\r";
    let in_table = [0x16, 0x24, 0x01];
    let mut builder = styled(main);
    for (index, character) in main.char_indices() {
        if character == '\u{7}' {
            let row_end =
                !main[index + 1..].starts_with('\u{7}') && main[..index].ends_with('\u{7}');
            let mut grpprl = in_table.to_vec();
            if row_end {
                grpprl.extend_from_slice(&[0x17, 0x24, 0x01]);
                grpprl.extend(table_definition(
                    &[0, 1_500, 3_000],
                    if index > 12 { &[1] } else { &[] },
                ));
            }
            builder = builder.paragraph(index, 0, &grpprl);
        }
    }
    let tree = read(&builder);
    assert_eq!(texts(&tree.blocks), vec!["Prima", "table of 2", "Dopo"]);
    let Block::Table { rows, look } = &tree.blocks[1] else {
        panic!("a table");
    };
    assert_eq!(look.column_widths_twips, vec![1_500, 1_500]);
    assert!(look.borders);
    assert_eq!(rows[0].cells.len(), 2);
    assert_eq!(texts(&rows[0].cells[1].blocks), vec!["B"]);
    assert_eq!(rows[1].cells.len(), 1, "the second row merges its cells");
    assert_eq!(rows[1].cells[0].col_span, 2);
    assert_eq!(texts(&rows[1].cells[0].blocks), vec!["C", "D"]);
}

#[test]
fn reads_a_table_nested_in_a_cell() {
    let main = "A\u{7}Su\rX\rY\r\rGiu\u{7}\u{7}Dopo\r";
    let in_table = [0x16, 0x24, 0x01];
    let nested = [in_table.as_slice(), &[0x49, 0x66, 2, 0, 0, 0]].concat();
    let inner_cell = [nested.as_slice(), &[0x4B, 0x24, 0x01]].concat();
    let mut inner_row = [nested.as_slice(), &[0x4C, 0x24, 0x01]].concat();
    inner_row.extend(table_definition(&[0, 700, 1_400], &[]));
    let mut outer_row = [in_table.as_slice(), &[0x17, 0x24, 0x01]].concat();
    outer_row.extend(table_definition(&[0, 1_500, 3_000], &[]));
    let marks: Vec<usize> = main
        .char_indices()
        .filter(|(_, character)| matches!(character, '\r' | '\u{7}'))
        .map(|(index, _)| index)
        .collect();
    let properties: [&[u8]; 8] = [
        &in_table,
        &in_table,
        &inner_cell,
        &inner_cell,
        &inner_row,
        &in_table,
        &outer_row,
        &[],
    ];
    let mut builder = styled(main);
    for (mark, grpprl) in marks.iter().zip(properties) {
        builder = builder.paragraph(*mark, 0, grpprl);
    }
    let tree = read(&builder);
    assert_eq!(texts(&tree.blocks), vec!["table of 1", "Dopo"]);
    let Block::Table { rows, .. } = &tree.blocks[0] else {
        panic!("a table");
    };
    assert_eq!(rows[0].cells.len(), 2);
    assert_eq!(texts(&rows[0].cells[0].blocks), vec!["A"]);
    let cell = &rows[0].cells[1].blocks;
    assert_eq!(texts(cell), vec!["Su", "table of 1", "Giu"]);
    let Block::Table { rows: inner, look } = &cell[1] else {
        panic!("the nested table");
    };
    assert_eq!(look.column_widths_twips, vec![700, 700]);
    assert_eq!(inner[0].cells.len(), 2);
    assert_eq!(texts(&inner[0].cells[1].blocks), vec!["Y"]);
}

#[test]
fn keeps_field_results_links_and_page_numbers() {
    let main = concat!(
        "Vai al \u{13} HYPERLINK \"https://rely.example/x\" \u{14}sito\u{15}, ",
        "\u{13}HYPERLINK \\l \"capitolo\"\u{14}capitolo\u{15}, ",
        "\u{13}HYPERLINK \"ftp://file\"\u{14}file\u{15}, ",
        "pagina \u{13} PAGE \u{14}3\u{15} di \u{13}NUMPAGES\u{14}9\u{15}, ",
        "\u{13}TOC \\o\u{14}indice\u{15}\u{13}DATE\u{15}.\r"
    );
    let tree = read(&styled(main));
    let content = content(&tree.blocks[0]);
    assert_eq!(
        plain_text(content),
        "Vai al sito, capitolo, file, pagina 3 di 9, indice."
    );
    let link = content
        .iter()
        .find(|inline| plain_text(std::slice::from_ref(inline)) == "sito")
        .unwrap();
    assert_eq!(
        marks(link),
        &[Mark::Link {
            href: "https://rely.example/x".to_owned()
        }]
    );
    let page = content
        .iter()
        .find(|inline| plain_text(std::slice::from_ref(inline)) == "3")
        .unwrap();
    let Inline::Text { look, .. } = page else {
        panic!("text");
    };
    assert_eq!(look.field, Some(PageField::Page));
    let pages = content
        .iter()
        .find(|inline| plain_text(std::slice::from_ref(inline)) == "9")
        .unwrap();
    let Inline::Text { look, .. } = pages else {
        panic!("text");
    };
    assert_eq!(look.field, Some(PageField::Pages));
    assert_eq!(tree.report.unsupported_links, 2);
    assert_eq!(
        tree.report.fields_kept_as_text, 1,
        "the field without a result shows nothing"
    );
}

#[test]
fn reads_special_characters_and_hidden_deleted_and_inserted_text() {
    let main = "a\tb\u{b}c\u{1e}d\u{1f}e\u{3}f nascosto via nuovo\u{c}\rDopo\r";
    let builder = styled(main)
        .character(
            at(main, " nascosto")..at(main, " nascosto") + 9,
            &[0x3C, 0x08, 0x01],
        )
        .character(at(main, " via")..at(main, " via") + 4, &[0x00, 0x08, 0x01])
        .character(
            at(main, " nuovo")..at(main, " nuovo") + 6,
            &[0x01, 0x08, 0x01],
        );
    let tree = read(&builder);
    assert_eq!(
        pieces(content(&tree.blocks[0])),
        vec!["a\tb", "<br>", "c-def nuovo"]
    );
    assert!(
        paragraph_look(&tree.blocks[1]).page_break_before,
        "the page break moves on"
    );
    assert!(!paragraph_look(&tree.blocks[0]).page_break_before);
    assert_eq!(tree.report.hidden_runs, 1);
    assert_eq!(tree.report.tracked_deletions, 1);
    assert_eq!(tree.report.tracked_insertions, 1);
}

#[test]
fn reads_footnotes_endnotes_and_counts_comments() {
    let main = "Testo\u{2} e altro\u{2} fine\u{5}.\r";
    let mut builder = styled(main);
    builder.footnotes = "\u{2} La nota.\r".to_owned();
    builder.endnotes = "\u{2} La chiusa.\r".to_owned();
    let footnote = u32::try_from(at(main, "\u{2} e")).unwrap();
    let endnote = u32::try_from(at(main, "\u{2} fine")).unwrap();
    let comment = u32::try_from(at(main, "\u{5}")).unwrap();
    let footnote_length = u32::try_from(builder.footnotes.len()).unwrap();
    let endnote_length = u32::try_from(builder.endnotes.len()).unwrap();
    let special = [0x55, 0x08, 0x01];
    builder = builder
        .character(footnote as usize..footnote as usize + 1, &special)
        .character(endnote as usize..endnote as usize + 1, &special)
        .character(comment as usize..comment as usize + 1, &special)
        .character(main.len()..main.len() + 1, &special)
        .structure(
            pair::FOOTNOTE_REFERENCES,
            plex_of(&[footnote, footnote + 1], &[&[1, 0]]),
        )
        .structure(
            pair::FOOTNOTE_TEXTS,
            plex_of(&[0, footnote_length, footnote_length + 1], &[]),
        )
        .structure(
            pair::ENDNOTE_REFERENCES,
            plex_of(&[endnote, endnote + 1], &[&[1, 0]]),
        )
        .structure(
            pair::ENDNOTE_TEXTS,
            plex_of(&[0, endnote_length, endnote_length + 1], &[]),
        )
        .structure(
            pair::COMMENT_REFERENCES,
            plex_of(&[comment, comment + 1], &[&[0; 30]]),
        );
    let tree = read(&builder);
    assert_eq!(
        pieces(content(&tree.blocks[0])),
        vec![
            "Testo",
            "<note 0>",
            " e altro",
            "<note 1>",
            " fine",
            "<comment 0>",
            "."
        ]
    );
    assert_eq!(tree.comments.len(), 1, "a comment on a point");
    assert_eq!(tree.notes.len(), 2);
    assert_eq!(tree.notes[0].kind, NoteKind::Footnote);
    assert_eq!(texts(&tree.notes[0].blocks), vec![" La nota."]);
    assert_eq!(tree.notes[1].kind, NoteKind::Endnote);
    assert_eq!(texts(&tree.notes[1].blocks), vec![" La chiusa."]);
    assert_eq!(
        (
            tree.report.footnotes,
            tree.report.endnotes,
            tree.report.comments
        ),
        (1, 1, 1)
    );
}

#[test]
fn lays_out_each_section_with_the_headers_it_inherits() {
    let main = "Verticale\u{c}Orizzontale\u{c}Ancora\r";
    let first_end = u32::try_from(at(main, "Orizzontale")).unwrap();
    let second_end = u32::try_from(at(main, "Ancora")).unwrap();
    let mut builder = styled(main);
    builder.headers = "Pari\rTesta\rPiede\rPrima\rNuova\r\r".to_owned();
    // Six separator parts, then six parts for each section: even and odd headers, even and
    // odd footers, first-page header and footer.
    let positions: Vec<u32> = vec![
        0, 0, 0, 0, 0, 0, //
        0, 5, 11, 11, 17, 23, //
        23, 23, 29, 29, 29, 29, //
        29, 29, 29, 29, 29, 29, //
        29, 30,
    ];
    builder.sections = vec![
        (first_end, vec![0x0A, 0x30, 0x01]),
        (
            second_end,
            vec![0x1F, 0xB0, 0xE0, 0x3D, 0x20, 0xB0, 0xD0, 0x2F],
        ),
        (u32::try_from(main.len()).unwrap(), vec![0x09, 0x30, 0x00]),
    ];
    let mut document_properties = vec![0_u8; 12];
    document_properties[0] = 1;
    document_properties[10..12].copy_from_slice(&360_u16.to_le_bytes());
    let builder = builder
        .structure(pair::HEADERS, plex_of(&positions, &[]))
        .structure(pair::DOCUMENT_PROPERTIES, document_properties);
    let tree = read(&builder);
    assert_eq!(
        texts(&tree.blocks),
        vec!["Verticale", "Orizzontale", "Ancora"]
    );
    assert_eq!(tree.sections.len(), 3);
    let first = &tree.sections[0];
    assert_eq!(first.end_block, 1);
    assert_eq!(texts(&first.headers_footers.header), vec!["Testa"]);
    assert_eq!(texts(&first.headers_footers.footer), vec!["Piede"]);
    assert_eq!(
        first.headers_footers.first_header.as_deref().map(texts),
        Some(vec!["Prima".to_owned()])
    );
    assert_eq!(first.headers_footers.first_footer, Some(Vec::new()));
    assert_eq!(
        first.headers_footers.even_header.as_deref().map(texts),
        Some(vec!["Pari".to_owned()])
    );
    assert_eq!(first.layout.default_tab_twips, 360);
    assert_eq!(first.layout.font_family.as_deref(), Some("Cambria"));
    let second = &tree.sections[1];
    assert_eq!(
        (
            second.end_block,
            second.layout.width_twips,
            second.layout.height_twips
        ),
        (2, 15_840, 12_240)
    );
    assert_eq!(texts(&second.headers_footers.header), vec!["Nuova"]);
    assert_eq!(
        texts(&second.headers_footers.footer),
        vec!["Piede"],
        "inherited from the first"
    );
    assert_eq!(second.headers_footers.first_header, None);
    assert!(!second.continuous);
    let third = &tree.sections[2];
    assert!(third.continuous);
    assert_eq!(texts(&third.headers_footers.header), vec!["Nuova"]);
    assert_eq!(tree.layout, third.layout);
    assert_eq!(tree.report.headers_and_footers, 4);
}

#[test]
fn reads_one_section_into_the_document_page() {
    let main = "Solo\r";
    let mut builder = styled(main);
    builder.sections = vec![(
        u32::try_from(main.len()).unwrap(),
        vec![0x21, 0xB0, 0xA0, 0x05],
    )];
    let tree = read(&builder);
    assert_eq!(tree.sections, Vec::<Section>::new());
    assert_eq!(tree.layout.margins_twips[3], 1_440);
    assert_eq!(
        tree.layout.width_twips, 12_240,
        "Letter, as Word 97 lays out a page"
    );
    let unsectioned = read(&styled(main));
    assert_eq!(
        unsectioned.layout.width_twips,
        PageLayout::default().width_twips
    );
}

/// A `PICF` of a picture a thousand twips by five hundred holding this drawing data.
fn picture(art: &[u8]) -> Vec<u8> {
    let mut header = vec![0_u8; 68];
    header[4..6].copy_from_slice(&68_u16.to_le_bytes());
    header[6..8].copy_from_slice(&0x0064_u16.to_le_bytes());
    header[28..30].copy_from_slice(&1_500_u16.to_le_bytes());
    header[30..32].copy_from_slice(&750_u16.to_le_bytes());
    header[32..34].copy_from_slice(&1_000_u16.to_le_bytes());
    header[34..36].copy_from_slice(&1_000_u16.to_le_bytes());
    header.extend_from_slice(art);
    let total = u32::try_from(header.len()).unwrap();
    header[0..4].copy_from_slice(&total.to_le_bytes());
    header
}

#[test]
fn reads_inline_pictures_once_and_counts_what_it_cannot_keep() {
    let main = "a\u{1}b\u{1}c\u{1}d\u{1}\r";
    let positions: Vec<usize> = main
        .match_indices('\u{1}')
        .map(|(index, _)| index)
        .collect();
    let mut art = record(0xF, 0, 0xF004, &[]);
    art.extend(store_entry(Some(&png_blip(PNG)), 0, 0));
    let mut data = picture(&art);
    let metafile_offset = data.len();
    let mut metafile_art = record(0xF, 0, 0xF004, &[]);
    metafile_art.extend(store_entry(Some(&record(0, 0x3D4, 0xF01A, &[0; 20])), 0, 0));
    data.extend(picture(&metafile_art));
    let location = |offset: usize| {
        let mut grpprl = vec![0x55, 0x08, 0x01, 0x03, 0x6A];
        grpprl.extend_from_slice(&u32::try_from(offset).unwrap().to_le_bytes());
        grpprl
    };
    let mut builder = styled(main)
        .character(positions[0]..positions[0] + 1, &location(0))
        .character(positions[1]..positions[1] + 1, &location(0))
        .character(positions[2]..positions[2] + 1, &location(metafile_offset))
        .character(
            positions[3]..positions[3] + 1,
            &[
                0x55, 0x08, 0x01, 0x0A, 0x08, 0x01, 0x03, 0x6A, 0xFF, 0xFF, 0, 0,
            ],
        );
    builder.data = data;
    let tree = read(&builder);
    assert_eq!(
        pieces(content(&tree.blocks[0])),
        vec!["a", "<image>", "b", "<image>", "c", "d"]
    );
    assert_eq!(tree.media.len(), 1);
    assert_eq!(tree.media[0].content_type, "image/png");
    let Inline::Image {
        width,
        height,
        anchor,
        ..
    } = &content(&tree.blocks[0])[1]
    else {
        panic!("an image");
    };
    assert_eq!(
        (*width, *height, anchor.is_none()),
        (Some(100), Some(50), true)
    );
    assert_eq!(tree.report.missing_images, 1);
    assert_eq!(tree.report.embedded_objects, 1);

    let limits = Limits {
        max_images: 0,
        ..Limits::default()
    };
    let limited = read_doc_tree(&builder.build(), &limits, TreeOptions::default()).unwrap();
    assert_eq!(limited.media, Vec::<Media>::new());
    assert_eq!(
        limited.report.images_over_limit, 1,
        "the second showing reuses the refusal"
    );
    let small = Limits {
        max_image_bytes: 4,
        ..Limits::default()
    };
    assert_eq!(
        read_doc_tree(&builder.build(), &small, TreeOptions::default())
            .unwrap()
            .report
            .oversized_images,
        1
    );
}

/// An `FSPA` anchoring shape `shape` over `bounds` in twips, with its bases and wrapping.
fn anchor(shape: u32, bounds: [i32; 4], flags: u16) -> Vec<u8> {
    let mut anchor = vec![0_u8; 26];
    anchor[0..4].copy_from_slice(&shape.to_le_bytes());
    for (index, value) in bounds.into_iter().enumerate() {
        anchor[4 + index * 4..8 + index * 4].copy_from_slice(&value.to_le_bytes());
    }
    anchor[20..22].copy_from_slice(&flags.to_le_bytes());
    anchor
}

fn drawn_shape(identifier: u32, properties: &[(u16, u32)]) -> Vec<u8> {
    drawn_shape_of(75, identifier, properties)
}

/// A shape of type `kind`, mirrored across.
fn drawn_shape_of(kind: u16, identifier: u32, properties: &[(u16, u32)]) -> Vec<u8> {
    let mut body = record(
        2,
        kind,
        0xF00A,
        &[identifier.to_le_bytes(), 0x40_u32.to_le_bytes()].concat(),
    );
    let mut values = Vec::new();
    for (property, value) in properties {
        values.extend_from_slice(&property.to_le_bytes());
        values.extend_from_slice(&value.to_le_bytes());
    }
    body.extend(record(
        3,
        u16::try_from(properties.len()).unwrap(),
        0xF00B,
        &values,
    ));
    record(0xF, 0, 0xF004, &body)
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one story anchors a picture, a text box and a shape"
)]
fn places_floating_pictures_and_text_boxes() {
    let main = "Con figura\u{8} e casella\u{8} e forma\u{8}.\r";
    let anchors: Vec<u32> = main
        .match_indices('\u{8}')
        .map(|(index, _)| u32::try_from(index).unwrap())
        .collect();
    let mut builder = styled(main);
    builder.text_boxes = "Nella casella\r\r".to_owned();
    let blip = png_blip(PNG);
    builder.delay = blip.clone();
    let store = record(
        0xF,
        1,
        0xF001,
        &store_entry(
            None,
            u32::try_from(blip.len()).unwrap(),
            u32::try_from(DELAY_OFFSET).unwrap(),
        ),
    );
    let mut shapes = drawn_shape(1_025, &[(0x0104, 1), (0x038F, 3), (0x0390, 1), (0x0392, 2)]);
    shapes.extend(drawn_shape(
        1_026,
        &[
            (0x0080, 0x0001_0000),
            (0x0181, 0x00CC_F2FF),
            (0x01C0, 0x0000_90BF),
            (0x01CB, 25_400),
        ],
    ));
    shapes.extend(drawn_shape(1_027, &[]));
    let mut content_bytes = record(0xF, 0, 0xF000, &store);
    content_bytes.push(0);
    content_bytes.extend(record(0xF, 0, 0xF002, &record(0xF, 0, 0xF003, &shapes)));
    let mut text_box = vec![0_u8; 22];
    text_box[14..18].copy_from_slice(&1_026_u32.to_le_bytes());
    let mut reused = vec![0_u8; 22];
    reused[8] = 1;
    let shapes_plex = plex_of(
        &[anchors[0], anchors[1], anchors[2], anchors[2] + 1],
        &[
            &anchor(1_025, [0, 100, 1_500, 850], (2 << 1) | (2 << 3) | (2 << 5)),
            &anchor(
                1_026,
                [200, -100, 3_200, 900],
                (1 << 1) | (1 << 3) | (3 << 5) | 0x4000,
            ),
            &anchor(1_027, [0, 0, 10, 10], 1 << 5),
        ],
    );
    let special = [0x55, 0x08, 0x01];
    for position in &anchors {
        builder = builder.character(*position as usize..*position as usize + 1, &special);
    }
    let builder = builder
        .structure(pair::DRAWINGS, content_bytes)
        .structure(pair::MAIN_SHAPES, shapes_plex)
        .structure(
            pair::TEXT_BOXES,
            plex_of(&[0, 14, 15], &[&text_box, &reused]),
        );
    let tree = read(&builder);
    assert_eq!(
        texts(&tree.blocks),
        vec!["Con figura e casella e forma.", "Nella casella"]
    );
    let Inline::Image {
        anchor: Some(placed),
        width,
        ..
    } = &content(&tree.blocks[0])[1]
    else {
        panic!("a floating image");
    };
    assert_eq!(*width, Some(100));
    assert_eq!(placed.horizontal.base, AnchorBase::Page);
    assert_eq!(placed.horizontal.align, Some(AnchorAlign::End));
    assert_eq!(placed.vertical.base, AnchorBase::Paragraph);
    assert_eq!(placed.vertical.offset_emu, 63_500);
    assert_eq!(placed.wrap, Wrap::Around);
    let frame = paragraph_look(&tree.blocks[1])
        .frame
        .clone()
        .expect("a text box");
    assert_eq!(frame.fill.as_deref(), Some("FFF2CC"));
    assert_eq!(frame.line, Some(("BF9000".to_owned(), 16)));
    assert_eq!(frame.anchor.horizontal.base, AnchorBase::Page);
    assert_eq!(frame.anchor.vertical.offset_emu, -63_500);
    assert_eq!(frame.anchor.width_emu, 1_905_000);
    assert_eq!(frame.anchor.wrap, Wrap::Behind);
    assert_eq!((tree.report.text_boxes, tree.report.shapes), (1, 0));
    assert_eq!(tree.media.len(), 2, "the frame shape drawn as a picture");
    assert_eq!(tree.media[1].content_type, "image/svg+xml");
    let Inline::Image {
        anchor: Some(_),
        width: Some(1),
        ..
    } = &content(&tree.blocks[0])[4]
    else {
        panic!(
            "the shape as a floating picture: {:?}",
            content(&tree.blocks[0])
        );
    };
}

#[test]
fn draws_the_shapes_of_a_type_and_counts_the_rest() {
    let main = "a\u{8}b\u{8}c\u{8}d\u{8}.\r";
    let anchors: Vec<u32> = main
        .match_indices('\u{8}')
        .map(|(index, _)| u32::try_from(index).unwrap())
        .collect();
    let mut builder = styled(main);
    let mut shapes = drawn_shape_of(13, 1, &[(0x0181, 0x0000_00FF), (0x01FF, 0x0008_0000)]);
    shapes.extend(drawn_shape_of(3, 2, &[(0x01BF, 0x0010_0000)]));
    shapes.extend(drawn_shape_of(20, 3, &[]));
    shapes.extend(drawn_shape_of(0, 4, &[]));
    let mut content_bytes = record(0xF, 0, 0xF000, &[]);
    content_bytes.push(0);
    content_bytes.extend(record(0xF, 0, 0xF002, &record(0xF, 0, 0xF003, &shapes)));
    let shapes_plex = plex_of(
        &[
            anchors[0],
            anchors[1],
            anchors[2],
            anchors[3],
            anchors[3] + 1,
        ],
        &[
            &anchor(1, [0, 0, 1_500, 750], 0),
            &anchor(2, [0, 0, 300, 300], 0),
            &anchor(3, [0, 0, 300, 0], 0),
            &anchor(4, [0, 0, 300, 300], 0),
        ],
    );
    let special = [0x55, 0x08, 0x01];
    for position in &anchors {
        builder = builder.character(*position as usize..*position as usize + 1, &special);
    }
    let builder = builder
        .structure(pair::DRAWINGS, content_bytes)
        .structure(pair::MAIN_SHAPES, shapes_plex);
    let tree = read(&builder);
    assert_eq!(tree.media.len(), 3, "an arrow, an ellipse and a line");
    assert_eq!(tree.report.shapes, 1, "a shape of no outline it can draw");
    let arrow = String::from_utf8(tree.media[0].bytes.clone()).unwrap();
    assert!(
        arrow.contains(r##"fill="#FF0000""##) && !arrow.contains("stroke="),
        "{arrow}"
    );
    let ellipse = String::from_utf8(tree.media[1].bytes.clone()).unwrap();
    assert!(!ellipse.contains("fill-rule"), "no fill: {ellipse}");
    assert!(ellipse.contains(r##"stroke="#000000""##));
    let images: Vec<(Option<u32>, Option<u32>)> = content(&tree.blocks[0])
        .iter()
        .filter_map(|inline| match inline {
            Inline::Image { width, height, .. } => Some((*width, *height)),
            _ => None,
        })
        .collect();
    assert_eq!(
        images,
        vec![
            (Some(100), Some(50)),
            (Some(20), Some(20)),
            (Some(20), Some(1))
        ]
    );
}

#[test]
fn stops_at_the_text_limit() {
    let main = "Uno\rDue\rTre\r";
    let limits = Limits {
        max_text_chars: 5,
        ..Limits::default()
    };
    let tree = read_doc_tree(&styled(main).build(), &limits, TreeOptions::default()).unwrap();
    assert!(tree.report.truncated);
    assert_eq!(texts(&tree.blocks), vec!["Uno", "Du"]);
}

#[test]
fn refuses_what_it_cannot_read() {
    let mut encrypted = styled("Segreto\r");
    encrypted.encrypted = true;
    assert!(matches!(
        read_doc_tree(
            &encrypted.build(),
            &Limits::default(),
            TreeOptions::default()
        ),
        Err(OfficeError::Encrypted)
    ));
    let limits = Limits {
        max_input_bytes: 10,
        ..Limits::default()
    };
    assert!(matches!(
        read_doc_tree(&styled("x\r").build(), &limits, TreeOptions::default()),
        Err(OfficeError::TooLarge { .. })
    ));
    let other = crate::test_support::build_compound_file(&[("/Altro", b"x".as_slice())]);
    assert!(matches!(
        read_doc_tree(&other, &Limits::default(), TreeOptions::default()),
        Err(OfficeError::UnreadableDocument(_))
    ));
}

#[test]
fn is_what_the_document_tree_reads_for_a_doc() {
    let tree =
        read_document_tree(&styled("Dal vecchio Word\r").build(), &Limits::default()).unwrap();
    assert_eq!(texts(&tree.blocks), vec!["Dal vecchio Word"]);
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one document carries a comment, its owner, its range and a change"
)]
fn reads_comments_with_their_range_and_keeps_changes() {
    use crate::document::{Comment, TrackedChanges};
    let main = "Uno due\u{5} tre quattro.\r";
    let mut builder = styled(main);
    builder.comments = "\u{5}Rivedere.\r".to_owned();
    let reference = u32::try_from(at(main, "\u{5}")).unwrap();
    let comment_length = u32::try_from(builder.comments.len()).unwrap();
    let mut record = vec![0_u8; 30];
    record[0..2].copy_from_slice(&2_u16.to_le_bytes());
    record[2..6].copy_from_slice(&[b'G', 0, b'D', 0]);
    record[20..22].copy_from_slice(&1_u16.to_le_bytes());
    record[26..30].copy_from_slice(&7_u32.to_le_bytes());
    let mut owners = Vec::new();
    for name in ["Anna", "Giosue"] {
        owners.extend_from_slice(&u16::try_from(name.len()).unwrap().to_le_bytes());
        owners.extend(name.encode_utf16().flat_map(u16::to_le_bytes));
    }
    let mut bookmarks = vec![0xFF, 0xFF, 1, 0, 10, 0, 0, 0];
    bookmarks.extend_from_slice(&[0, 0]);
    bookmarks.extend_from_slice(&7_u32.to_le_bytes());
    bookmarks.extend_from_slice(&[0; 4]);
    let mut reviewers = vec![0xFF, 0xFF, 1, 0, 0, 0];
    reviewers.extend_from_slice(&5_u16.to_le_bytes());
    reviewers.extend("Bruno".encode_utf16().flat_map(u16::to_le_bytes));
    let special = [0x55, 0x08, 0x01];
    let deleted_from = at(main, " quattro");
    builder = builder
        .character(reference as usize..reference as usize + 1, &special)
        .character(main.len()..main.len() + 1, &special)
        .character(
            deleted_from..deleted_from + 8,
            &[
                0x00, 0x08, 0x01, 0x63, 0x48, 0x00, 0x00, 0x64, 0x68, 0x5E, 0xDA, 0xEA, 0x47,
            ],
        )
        .structure(
            pair::COMMENT_REFERENCES,
            plex_of(&[reference, reference + 1], &[&record]),
        )
        .structure(
            pair::COMMENT_TEXTS,
            plex_of(&[0, comment_length, comment_length + 1], &[]),
        )
        .structure(pair::COMMENT_OWNERS, owners)
        .structure(pair::COMMENT_BOOKMARK_NAMES, bookmarks)
        .structure(
            pair::COMMENT_BOOKMARK_FIRSTS,
            plex_of(&[4, 100], &[&[0, 0, 0, 0]]),
        )
        .structure(
            pair::COMMENT_BOOKMARK_LASTS,
            [7_u32, 100]
                .iter()
                .flat_map(|position| position.to_le_bytes())
                .collect(),
        )
        .structure(pair::REVIEWERS, reviewers);
    let accepted = read(&builder);
    assert_eq!(
        accepted.comments,
        vec![Comment {
            author: "Giosue".to_owned(),
            initials: Some("GD".to_owned()),
            date: None,
            blocks: accepted.comments[0].blocks.clone(),
            parent: None,
            resolved: false,
        }]
    );
    assert_eq!(texts(&accepted.comments[0].blocks), vec!["Rivedere."]);
    let pieces_of = |tree: &DocumentTree| -> Vec<(String, Vec<Mark>)> {
        content(&tree.blocks[0])
            .iter()
            .filter_map(|inline| match inline {
                Inline::Text { text, marks, .. } => Some((text.clone(), marks.clone())),
                _ => None,
            })
            .collect()
    };
    let read_accepted = pieces_of(&accepted);
    assert_eq!(
        read_accepted[1],
        ("due".to_owned(), vec![Mark::Comment { comment: 0 }])
    );
    assert!(
        read_accepted
            .iter()
            .all(|(text, _)| !text.contains("quattro"))
    );
    let kept = read_doc_tree(
        &builder.build(),
        &Limits::default(),
        TreeOptions {
            tracked_changes: TrackedChanges::Keep,
        },
    )
    .unwrap();
    let deleted = pieces_of(&kept)
        .into_iter()
        .find(|(text, _)| text == " quattro")
        .expect("the deleted text kept");
    assert_eq!(
        deleted.1,
        vec![Mark::Deletion {
            author: "Bruno".to_owned(),
            date: Some("2026-10-27T09:30:00Z".to_owned()),
        }]
    );
}
