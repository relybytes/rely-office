// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;
use crate::document::{
    CellLook, ImportReport, PageLayout, ParagraphLook, RunLook, TableCell, TableLook,
};
use crate::{Limits, read_document_tree};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01";

fn text(value: &str, marks: Vec<Mark>) -> Inline {
    Inline::Text {
        text: value.to_owned(),
        marks,
        look: RunLook::default(),
    }
}

fn paragraph(content: Vec<Inline>, align: Option<Alignment>) -> Block {
    Block::Paragraph {
        content,
        align,
        look: ParagraphLook::default(),
    }
}

fn item(ordered: bool, level: u8, list_id: u32, value: &str) -> Block {
    Block::ListItem {
        ordered,
        level,
        list_id,
        content: vec![text(value, Vec::new())],
        look: ParagraphLook::default(),
    }
}

fn cell(value: &str, col_span: u32, row_span: u32) -> TableCell {
    TableCell {
        col_span,
        row_span,
        blocks: vec![paragraph(vec![text(value, Vec::new())], None)],
        look: CellLook::default(),
    }
}

fn tree(blocks: Vec<Block>, media: Vec<Media>) -> DocumentTree {
    DocumentTree {
        blocks,
        media,
        report: ImportReport::default(),
        layout: PageLayout::default(),
        ..DocumentTree::default()
    }
}

/// The tree with its looks reset and its list ids renumbered in order of appearance, for
/// comparing structure across a write and a read.
fn normalized(blocks: &[Block], lists: &mut Vec<u32>) -> Vec<Block> {
    blocks
        .iter()
        .map(|block| match block.clone() {
            Block::Heading {
                level,
                content,
                align,
                ..
            } => Block::Heading {
                level,
                content: inlines(&content),
                align,
                look: ParagraphLook::default(),
            },
            Block::Paragraph { content, align, .. } => Block::Paragraph {
                content: inlines(&content),
                align,
                look: ParagraphLook::default(),
            },
            Block::ListItem {
                ordered,
                level,
                list_id,
                content,
                ..
            } => {
                let index = lists
                    .iter()
                    .position(|id| *id == list_id)
                    .unwrap_or_else(|| {
                        lists.push(list_id);
                        lists.len() - 1
                    });
                Block::ListItem {
                    ordered,
                    level,
                    list_id: u32::try_from(index).unwrap(),
                    content: inlines(&content),
                    look: ParagraphLook::default(),
                }
            }
            Block::Table { rows, .. } => Block::Table {
                rows: rows
                    .into_iter()
                    .map(|row| TableRow {
                        header: row.header,
                        cells: row
                            .cells
                            .into_iter()
                            .map(|cell| TableCell {
                                blocks: normalized(&cell.blocks, lists),
                                ..cell
                            })
                            .collect(),
                    })
                    .collect(),
                look: TableLook::default(),
            },
            Block::Blockquote { blocks } => Block::Blockquote {
                blocks: normalized(&blocks, lists),
            },
            other => other,
        })
        .collect()
}

fn inlines(content: &[Inline]) -> Vec<Inline> {
    content
        .iter()
        .map(|inline| match inline.clone() {
            Inline::Text { text, marks, .. } => Inline::Text {
                text,
                marks,
                look: RunLook::default(),
            },
            other => other,
        })
        .collect()
}

fn round_trip(original: &DocumentTree) -> DocumentTree {
    let bytes = write_docx(original).unwrap();
    read_document_tree(&bytes, &Limits::default()).unwrap()
}

#[test]
fn reads_back_the_tree_it_wrote() {
    let blocks = vec![
        Block::Heading {
            level: 1,
            content: vec![text("Contratto", Vec::new())],
            align: Some(Alignment::Center),
            look: ParagraphLook::default(),
        },
        Block::Heading {
            level: 9,
            content: vec![text("Livello", Vec::new())],
            align: None,
            look: ParagraphLook::default(),
        },
        paragraph(
            vec![
                text("grassetto", vec![Mark::Bold]),
                text(" corsivo ", vec![Mark::Italic, Mark::Underline]),
                text("barrato", vec![Mark::Strike]),
                text("x", vec![Mark::Code]),
                text("2", vec![Mark::Superscript]),
                text("i", vec![Mark::Subscript]),
                text(
                    "sito",
                    vec![Mark::Link {
                        href: "https://relybytes.com/?a=1&b=2".to_owned(),
                    }],
                ),
                Inline::HardBreak,
                text("a\tb <&> \"c\" 'd'", Vec::new()),
            ],
            Some(Alignment::Justify),
        ),
        item(false, 0, 7, "uno"),
        item(false, 1, 7, "due"),
        item(true, 0, 9, "primo"),
        item(true, 0, 9, "secondo"),
        Block::Blockquote {
            blocks: vec![
                paragraph(vec![text("citato", Vec::new())], None),
                paragraph(vec![text("ancora", Vec::new())], Some(Alignment::Right)),
            ],
        },
        Block::CodeBlock {
            text: "fn main() {\n    println!(\"ciao\");\n}".to_owned(),
        },
        Block::HorizontalRule,
        paragraph(Vec::new(), None),
    ];
    let original = tree(blocks, Vec::new());
    let read = round_trip(&original);
    assert_eq!(
        normalized(&read.blocks, &mut Vec::new()),
        normalized(
            &original
                .blocks
                .iter()
                .map(|block| match block {
                    Block::Heading { level: 9, .. } => Block::Heading {
                        level: 6,
                        content: vec![text("Livello", Vec::new())],
                        align: None,
                        look: ParagraphLook::default(),
                    },
                    other => other.clone(),
                })
                .collect::<Vec<_>>(),
            &mut Vec::new()
        )
    );
    assert_eq!(read.report, ImportReport::default());
}

#[test]
fn writes_tables_with_spans_and_merges() {
    let rows = vec![
        TableRow {
            header: true,
            cells: vec![cell("Voce", 2, 1), cell("Totale", 1, 3)],
        },
        TableRow {
            header: false,
            cells: vec![cell("a", 1, 2), cell("b", 1, 1)],
        },
        TableRow {
            header: false,
            cells: vec![cell("c", 1, 1)],
        },
    ];
    let original = tree(
        vec![Block::Table {
            rows,
            look: TableLook::default(),
        }],
        Vec::new(),
    );
    let read = round_trip(&original);
    assert_eq!(
        normalized(&read.blocks, &mut Vec::new()),
        normalized(&original.blocks, &mut Vec::new())
    );
    let Block::Table { look, .. } = &read.blocks[0] else {
        panic!("not a table");
    };
    assert!(look.borders);
    assert_eq!(look.column_widths_twips.len(), 3);
}

#[test]
fn places_merged_cells_on_the_grid() {
    let rows = vec![
        TableRow {
            header: false,
            cells: vec![cell("a", 1, 3), cell("b", 2, 1)],
        },
        TableRow {
            header: false,
            cells: vec![cell("c", 1, 1)],
        },
        TableRow {
            header: false,
            cells: vec![],
        },
    ];
    let placed = place_cells(&rows);
    assert_eq!(placed.columns, 3);
    let kinds: Vec<Vec<SlotKind>> = placed
        .rows
        .iter()
        .map(|row| row.iter().map(|slot| slot.kind).collect())
        .collect();
    assert_eq!(
        kinds,
        vec![
            vec![
                SlotKind::Cell {
                    cell: 0,
                    merge_start: true
                },
                SlotKind::Cell {
                    cell: 1,
                    merge_start: false
                }
            ],
            vec![
                SlotKind::Continuation,
                SlotKind::Cell {
                    cell: 0,
                    merge_start: false
                }
            ],
            vec![SlotKind::Continuation],
        ]
    );
}

#[test]
fn writes_images_and_drops_those_word_cannot_show() {
    let media = vec![
        Media {
            content_type: "image/png",
            bytes: PNG.to_vec(),
        },
        Media {
            content_type: "application/octet-stream",
            bytes: vec![1, 2, 3],
        },
    ];
    let blocks = vec![paragraph(
        vec![
            Inline::Image {
                media: 0,
                alt: Some("Il \"logo\"".to_owned()),
                width: Some(100),
                height: Some(50),
            },
            Inline::Image {
                media: 0,
                alt: None,
                width: Some(10_000),
                height: Some(1_000),
            },
            Inline::Image {
                media: 1,
                alt: Some("grafico".to_owned()),
                width: None,
                height: None,
            },
            Inline::Image {
                media: 9,
                alt: None,
                width: None,
                height: None,
            },
        ],
        None,
    )];
    let read = round_trip(&tree(blocks, media));
    assert_eq!(read.media.len(), 1, "the same image is stored once");
    assert_eq!(read.media[0].bytes, PNG);
    let Block::Paragraph { content, .. } = &read.blocks[0] else {
        panic!("not a paragraph");
    };
    assert_eq!(
        content[0],
        Inline::Image {
            media: 0,
            alt: Some("Il \"logo\"".to_owned()),
            width: Some(100),
            height: Some(50),
        }
    );
    let Inline::Image { width, height, .. } = &content[1] else {
        panic!("not an image");
    };
    assert_eq!(
        (*width, *height),
        (Some(604), Some(60)),
        "cut to the text width"
    );
    assert_eq!(content[2], text("grafico", Vec::new()));
    assert_eq!(content.len(), 3);
}

#[test]
fn keeps_the_page_of_the_tree() {
    let mut landscape = tree(vec![paragraph(Vec::new(), None)], Vec::new());
    landscape.layout.width_twips = 16_838;
    landscape.layout.height_twips = 11_906;
    landscape.layout.margins_twips = [720, 1_080, 720, 1_080];
    let read = round_trip(&landscape);
    assert_eq!(read.layout.width_twips, 16_838);
    assert_eq!(read.layout.height_twips, 11_906);
    assert_eq!(read.layout.margins_twips, [720, 1_080, 720, 1_080]);
    assert_eq!(read.layout.font_family.as_deref(), Some("Calibri"));
}

#[test]
fn escapes_text_and_drops_what_xml_cannot_hold() {
    assert_eq!(
        escape("a<b>&\"c'\u{1}\u{fffe}\td"),
        "a&lt;b&gt;&amp;&quot;c&apos;\td"
    );
}
