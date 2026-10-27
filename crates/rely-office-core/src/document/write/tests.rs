// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;
use crate::document::{
    AnchorAlign, AnchorBase, AnchorPosition, CellLook, ImportReport, PageLayout, ParagraphLook,
    RunLook, TableCell, TableLook, Wrap,
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
                anchor: None,
            },
            Inline::Image {
                media: 0,
                alt: None,
                width: Some(10_000),
                height: Some(1_000),
                anchor: None,
            },
            Inline::Image {
                media: 1,
                alt: Some("grafico".to_owned()),
                width: None,
                height: None,
                anchor: None,
            },
            Inline::Image {
                media: 9,
                alt: None,
                width: None,
                height: None,
                anchor: None,
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
            anchor: None,
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

fn floating_anchor(wrap: Wrap, horizontal: AnchorPosition, vertical: AnchorPosition) -> Anchor {
    Anchor {
        horizontal,
        vertical,
        width_emu: 1_828_800,
        height_emu: 914_400,
        wrap,
        distance_emu: [0, 0, 114_300, 114_300],
    }
}

fn at(base: AnchorBase, offset_emu: i64, align: Option<AnchorAlign>) -> AnchorPosition {
    AnchorPosition {
        base,
        offset_emu,
        align,
    }
}

fn framed(value: &str, frame: &TextFrame) -> Block {
    Block::Paragraph {
        content: vec![text(value, Vec::new())],
        align: None,
        look: ParagraphLook {
            frame: Some(frame.clone()),
            ..ParagraphLook::default()
        },
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one document with every kind of floating object"
)]
fn writes_floating_pictures_and_text_boxes_where_they_float() {
    let picture_anchor = floating_anchor(
        Wrap::Around,
        at(AnchorBase::Margin, 0, Some(AnchorAlign::End)),
        at(AnchorBase::Paragraph, 127_000, None),
    );
    let boxed = TextFrame {
        id: 1,
        anchor: floating_anchor(
            Wrap::InFront,
            at(AnchorBase::Page, 914_400, None),
            at(AnchorBase::Page, 0, Some(AnchorAlign::Center)),
        ),
        fill: Some("FFF2CC".to_owned()),
        line: Some(("C00000".to_owned(), 12)),
    };
    let behind = TextFrame {
        id: 2,
        anchor: floating_anchor(
            Wrap::Behind,
            at(AnchorBase::Line, 0, Some(AnchorAlign::Start)),
            at(AnchorBase::Line, 0, Some(AnchorAlign::End)),
        ),
        fill: None,
        line: None,
    };
    let above_and_below = TextFrame {
        id: 3,
        anchor: floating_anchor(
            Wrap::TopAndBottom,
            at(AnchorBase::Column, 0, Some(AnchorAlign::Center)),
            at(AnchorBase::Margin, 0, Some(AnchorAlign::Start)),
        ),
        fill: None,
        line: None,
    };
    let blocks = vec![
        framed("prima del testo", &above_and_below),
        paragraph(
            vec![
                text("Ancora", Vec::new()),
                Inline::Image {
                    media: 0,
                    alt: Some("logo".to_owned()),
                    width: Some(192),
                    height: Some(96),
                    anchor: Some(Box::new(picture_anchor)),
                },
            ],
            None,
        ),
        framed("riga uno", &boxed),
        framed("riga due", &boxed),
        framed("dietro", &behind),
        Block::Table {
            rows: vec![TableRow {
                header: false,
                cells: vec![cell("cella", 1, 1)],
            }],
            look: TableLook::default(),
        },
        framed("dopo la tabella", &above_and_below),
    ];
    let media = vec![Media {
        content_type: "image/png",
        bytes: PNG.to_vec(),
    }];
    let bytes = write_docx(&tree(blocks, media)).unwrap();
    let read = read_document_tree(&bytes, &Limits::default()).unwrap();
    assert_eq!(read.report.text_boxes, 4);
    let frame_of = |block: &Block| match block {
        Block::Paragraph { look, .. } => look.frame.clone(),
        _ => None,
    };
    let texts: Vec<String> = read
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph { content, .. } => Some(
                content
                    .iter()
                    .filter_map(|inline| match inline {
                        Inline::Text { text, .. } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        })
        .collect();
    assert_eq!(
        texts,
        [
            "",
            "prima del testo",
            "Ancora",
            "riga uno",
            "riga due",
            "dietro",
            "",
            "dopo la tabella"
        ],
        "a box after a table, or first, gets a paragraph of its own"
    );
    let Block::Paragraph { content, .. } = &read.blocks[2] else {
        panic!("the anchoring paragraph");
    };
    let Inline::Image { anchor, .. } = &content[1] else {
        panic!("the picture");
    };
    assert_eq!(anchor.as_deref(), Some(&picture_anchor));
    let first = frame_of(&read.blocks[3]).unwrap();
    let second = frame_of(&read.blocks[4]).unwrap();
    assert_eq!(first.id, second.id, "two paragraphs of one box");
    assert_eq!(first.anchor, boxed.anchor);
    assert_eq!(first.fill.as_deref(), Some("FFF2CC"));
    assert_eq!(first.line, Some(("C00000".to_owned(), 12)));
    let dietro = frame_of(&read.blocks[5]).unwrap();
    assert_eq!(dietro.anchor, behind.anchor);
    assert_eq!((dietro.fill, dietro.line), (None, None));
    let column = frame_of(&read.blocks[1]).unwrap();
    assert_eq!(column.anchor, above_and_below.anchor);
}

#[test]
fn keeps_metafile_pictures_word_shows() {
    let media = vec![
        Media {
            content_type: "image/emf",
            bytes: vec![1, 0, 0, 0],
        },
        Media {
            content_type: "image/wmf",
            bytes: vec![0xD7, 0xCD, 0xC6, 0x9A],
        },
    ];
    let image = |media: usize| Inline::Image {
        media,
        alt: None,
        width: Some(10),
        height: Some(10),
        anchor: None,
    };
    let bytes = write_docx(&tree(
        vec![paragraph(vec![image(0), image(1)], None)],
        media,
    ))
    .unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    assert!(archive.by_name("word/media/image1.emf").is_ok());
    assert!(archive.by_name("word/media/image2.wmf").is_ok());
    let mut types = String::new();
    std::io::Read::read_to_string(
        &mut archive.by_name("[Content_Types].xml").unwrap(),
        &mut types,
    )
    .unwrap();
    assert!(types.contains(r#"<Default Extension="emf" ContentType="image/x-emf"/>"#));
}
