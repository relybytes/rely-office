// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Notes, headers, footers, run styles and looks written and read back.

use super::*;
use crate::document::{
    Border, BorderStyle, Borders, CellLook, HeadersFooters, LabelSuffix, ListLabel, PageLayout,
    RunLook, TabLeader, TabStop, TableCell, TableLook,
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

fn paragraph(content: Vec<Inline>) -> Block {
    Block::Paragraph {
        content,
        align: None,
        look: ParagraphLook::default(),
    }
}

fn round_trip(original: &DocumentTree) -> DocumentTree {
    let bytes = write_docx(original).unwrap();
    read_document_tree(&bytes, &Limits::default()).unwrap()
}

fn plain(blocks: &[Block]) -> Vec<String> {
    blocks
        .iter()
        .map(|block| match block {
            Block::Paragraph { content, .. } | Block::Heading { content, .. } => content
                .iter()
                .map(|inline| match inline {
                    Inline::Text { text, .. } => text.clone(),
                    Inline::NoteReference { note } => format!("[{note}]"),
                    Inline::Image { .. } => "[image]".to_owned(),
                    Inline::HardBreak => "\n".to_owned(),
                    Inline::CommentAnchor { comment } => format!("[comment {comment}]"),
                    Inline::Math { latex, .. } => format!("[math {latex}]"),
                })
                .collect(),
            other => format!("{other:?}"),
        })
        .collect()
}

#[test]
fn writes_notes_and_reads_them_back_in_order() {
    let original = DocumentTree {
        blocks: vec![paragraph(vec![
            text("Uno", Vec::new()),
            Inline::NoteReference { note: 1 },
            text(" due", Vec::new()),
            Inline::NoteReference { note: 0 },
            Inline::NoteReference { note: 1 },
            Inline::NoteReference { note: 7 },
        ])],
        notes: vec![
            Note {
                kind: NoteKind::Endnote,
                blocks: vec![paragraph(vec![text("Finale", Vec::new())])],
            },
            Note {
                kind: NoteKind::Footnote,
                blocks: vec![
                    paragraph(vec![text(
                        "Vedi",
                        vec![Mark::Link {
                            href: "https://example.com".to_owned(),
                        }],
                    )]),
                    paragraph(vec![Inline::Image {
                        media: 0,
                        alt: None,
                        width: Some(10),
                        height: Some(10),
                        anchor: None,
                    }]),
                ],
            },
        ],
        media: vec![Media {
            content_type: "image/png",
            bytes: PNG.to_vec(),
        }],
        ..DocumentTree::default()
    };
    let read = round_trip(&original);
    assert_eq!(plain(&read.blocks), vec!["Uno[0] due[1][0]"]);
    assert_eq!(read.notes.len(), 2);
    assert_eq!(read.notes[0].kind, NoteKind::Footnote);
    assert_eq!(plain(&read.notes[0].blocks), vec!["Vedi", "[image]"]);
    let Block::Paragraph { content, .. } = &read.notes[0].blocks[0] else {
        panic!("not a paragraph");
    };
    assert!(matches!(&content[0], Inline::Text { marks, .. }
        if marks.contains(&Mark::Link { href: "https://example.com".to_owned() })));
    assert_eq!(
        read.media.len(),
        1,
        "the image of a note is in its own part"
    );
    assert_eq!(read.notes[1].kind, NoteKind::Endnote);
    assert_eq!(plain(&read.notes[1].blocks), vec!["Finale"]);
}

#[test]
fn writes_headers_and_footers_and_their_variants() {
    let page_number = Inline::Text {
        text: "1".to_owned(),
        marks: Vec::new(),
        look: RunLook {
            field: Some(PageField::Page),
            ..RunLook::default()
        },
    };
    let pages = Inline::Text {
        text: "3".to_owned(),
        marks: Vec::new(),
        look: RunLook {
            field: Some(PageField::Pages),
            ..RunLook::default()
        },
    };
    let original = DocumentTree {
        blocks: vec![paragraph(vec![text("Corpo", Vec::new())])],
        headers_footers: HeadersFooters {
            header: vec![paragraph(vec![
                text(
                    "Logo",
                    vec![Mark::Link {
                        href: "https://relybytes.com".to_owned(),
                    }],
                ),
                Inline::Image {
                    media: 0,
                    alt: None,
                    width: None,
                    height: None,
                    anchor: None,
                },
            ])],
            footer: vec![paragraph(vec![
                text("Pagina ", Vec::new()),
                page_number,
                text(" di ", Vec::new()),
                pages,
            ])],
            first_header: Some(vec![paragraph(vec![text("Copertina", Vec::new())])]),
            first_footer: Some(Vec::new()),
            even_header: None,
            even_footer: Some(vec![paragraph(vec![text("Pari", Vec::new())])]),
        },
        media: vec![Media {
            content_type: "image/png",
            bytes: PNG.to_vec(),
        }],
        layout: PageLayout {
            header_distance_twips: 500,
            footer_distance_twips: 400,
            columns: 2,
            column_gap_twips: 360,
            default_tab_twips: 720,
            ..PageLayout::default()
        },
        ..DocumentTree::default()
    };
    let read = round_trip(&original);
    let parts = &read.headers_footers;
    assert_eq!(plain(&parts.header), vec!["Logo[image]"]);
    assert_eq!(plain(&parts.footer), vec!["Pagina 1 di 3"]);
    let Block::Paragraph { content, .. } = &parts.footer[0] else {
        panic!("not a paragraph");
    };
    assert!(
        matches!(&content[1], Inline::Text { look, .. } if look.field == Some(PageField::Page))
    );
    assert!(
        matches!(&content[3], Inline::Text { look, .. } if look.field == Some(PageField::Pages))
    );
    assert_eq!(
        parts.first_header.as_deref().map(plain),
        Some(vec!["Copertina".to_owned()])
    );
    assert_eq!(
        parts.first_footer.as_deref().map(plain),
        Some(vec![String::new()])
    );
    assert_eq!(parts.even_header, Some(Vec::new()));
    assert_eq!(
        parts.even_footer.as_deref().map(plain),
        Some(vec!["Pari".to_owned()])
    );
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

    let bare = round_trip(&DocumentTree {
        blocks: vec![paragraph(vec![text("x", Vec::new())])],
        ..DocumentTree::default()
    });
    assert_eq!(bare.headers_footers, HeadersFooters::default());
}

#[test]
fn writes_the_font_size_colour_and_highlight_of_runs() {
    let original = DocumentTree {
        blocks: vec![paragraph(vec![
            text(
                "rosso",
                vec![
                    Mark::Bold,
                    Mark::TextStyle {
                        font_family: Some("Arial".to_owned()),
                        font_size: Some("10.5pt".to_owned()),
                        color: Some("#c00000".to_owned()),
                    },
                    Mark::Highlight {
                        color: "#FFFF00".to_owned(),
                    },
                ],
            ),
            text(
                "strano",
                vec![
                    Mark::TextStyle {
                        font_family: None,
                        font_size: Some("enorme".to_owned()),
                        color: Some("red".to_owned()),
                    },
                    Mark::Highlight {
                        color: "giallo".to_owned(),
                    },
                ],
            ),
            text(
                "link",
                vec![
                    Mark::TextStyle {
                        font_family: None,
                        font_size: None,
                        color: Some("#00FF00".to_owned()),
                    },
                    Mark::Link {
                        href: "https://example.com".to_owned(),
                    },
                ],
            ),
        ])],
        ..DocumentTree::default()
    };
    let read = round_trip(&original);
    let Block::Paragraph { content, .. } = &read.blocks[0] else {
        panic!("not a paragraph");
    };
    let marks: Vec<&Vec<Mark>> = content
        .iter()
        .map(|inline| match inline {
            Inline::Text { marks, .. } => marks,
            other => panic!("not text: {other:?}"),
        })
        .collect();
    assert_eq!(
        marks[0],
        &vec![
            Mark::Bold,
            Mark::TextStyle {
                font_family: Some("Arial".to_owned()),
                font_size: Some("10.5pt".to_owned()),
                color: Some("#C00000".to_owned()),
            },
            Mark::Highlight {
                color: "#FFFF00".to_owned()
            },
        ]
    );
    assert!(marks[1].is_empty(), "values that are not CSS are dropped");
    assert_eq!(
        marks[2],
        &vec![Mark::Link {
            href: "https://example.com".to_owned()
        }],
        "a link draws in the link colour"
    );
    assert_eq!(half_points("12pt"), Some(24));
    assert_eq!(half_points("1pt"), Some(2));
    assert_eq!(half_points("0pt"), None);
    assert_eq!(half_points("99999pt"), None);
    assert_eq!(css_hex("#abc"), None);
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one document exercises every case together"
)]
fn writes_the_looks_a_tree_read_from_a_docx_carries() {
    let border = Border {
        style: BorderStyle::Single,
        width_eighths: 8,
        colour: Some("2F5496".to_owned()),
    };
    let look = ParagraphLook {
        space_before_twips: Some(120),
        space_after_twips: Some(0),
        line_spacing: Some(LineSpacing::Exact(300)),
        indent_left_twips: Some(720),
        indent_right_twips: Some(360),
        indent_first_line_twips: Some(-360),
        page_break_before: true,
        font: RunLook {
            family: Some("Times New Roman".to_owned()),
            size_half_points: Some(24),
            colour: Some("333333".to_owned()),
            italic: Some(true),
            caps: Some(true),
            ..RunLook::default()
        },
        tabs: vec![TabStop {
            position_twips: 9_000,
            kind: TabKind::Right,
            leader: TabLeader::Dot,
        }],
        shading: Some("DEEAF6".to_owned()),
        borders: Borders {
            top: Some(border.clone()),
            bottom: Some(Border {
                style: BorderStyle::Double,
                ..border.clone()
            }),
            left: Some(Border {
                style: BorderStyle::Dotted,
                colour: None,
                ..border.clone()
            }),
            right: Some(Border {
                style: BorderStyle::Dashed,
                ..border.clone()
            }),
            inside_horizontal: Some(Border {
                style: BorderStyle::None,
                width_eighths: 4,
                colour: None,
            }),
            inside_vertical: None,
        },
        list_label: None,
        frame: None,
        format: crate::document::ParagraphFormat::default(),
    };
    let heading_look = ParagraphLook {
        list_label: Some(ListLabel {
            text: "Art. 1.".to_owned(),
            suffix: LabelSuffix::Tab,
            font: RunLook::default(),
        }),
        ..ParagraphLook::default()
    };
    let cell = |value: &str, look: CellLook| TableCell {
        col_span: 1,
        row_span: 1,
        blocks: vec![paragraph(vec![text(value, Vec::new())])],
        look,
    };
    let original = DocumentTree {
        blocks: vec![
            Block::Paragraph {
                content: vec![text("corpo\tpagina", Vec::new())],
                align: Some(Alignment::Justify),
                look: look.clone(),
            },
            Block::Heading {
                level: 1,
                content: vec![text("Oggetto", Vec::new())],
                align: None,
                look: heading_look,
            },
            Block::Table {
                rows: vec![TableRow {
                    header: false,
                    cells: vec![
                        cell(
                            "a",
                            CellLook {
                                fill: Some("4472C4".to_owned()),
                                borders: Borders {
                                    bottom: Some(border.clone()),
                                    ..Borders::default()
                                },
                                vertical: Some(VerticalAlignment::Center),
                            },
                        ),
                        cell(
                            "b",
                            CellLook {
                                vertical: Some(VerticalAlignment::Bottom),
                                ..CellLook::default()
                            },
                        ),
                    ],
                }],
                look: TableLook {
                    column_widths_twips: vec![2_000, 3_000],
                    borders: false,
                    cell_margins_twips: [Some(40), Some(80), None, None],
                    edges: Borders {
                        inside_horizontal: Some(border.clone()),
                        ..Borders::default()
                    },
                    frame: None,
                },
            },
        ],
        ..DocumentTree::default()
    };
    let read = round_trip(&original);
    let Block::Paragraph {
        look: read_look,
        align,
        content,
    } = &read.blocks[0]
    else {
        panic!("not a paragraph: {:?}", read.blocks[0]);
    };
    assert_eq!(*align, Some(Alignment::Justify));
    assert_eq!(read_look.space_before_twips, Some(120));
    assert_eq!(read_look.space_after_twips, Some(0));
    assert_eq!(read_look.line_spacing, Some(LineSpacing::Exact(300)));
    assert_eq!(
        (
            read_look.indent_left_twips,
            read_look.indent_right_twips,
            read_look.indent_first_line_twips
        ),
        (Some(720), Some(360), Some(-360))
    );
    assert!(read_look.page_break_before);
    assert_eq!(read_look.tabs, look.tabs);
    assert_eq!(read_look.shading, look.shading);
    assert_eq!(read_look.borders, look.borders);
    let Inline::Text {
        look: run, marks, ..
    } = &content[0]
    else {
        panic!("not text");
    };
    assert_eq!(run.family.as_deref(), Some("Times New Roman"));
    assert_eq!(run.size_half_points, Some(24));
    assert_eq!(run.colour.as_deref(), Some("333333"));
    assert_eq!(run.caps, Some(true));
    assert!(marks.contains(&Mark::Italic));

    assert_eq!(plain(&read.blocks[1..2]), vec!["Art. 1.\tOggetto"]);

    let Block::Table { rows, look } = &read.blocks[2] else {
        panic!("not a table");
    };
    assert_eq!(look.column_widths_twips, vec![2_000, 3_000]);
    assert!(!look.borders || look.edges.inside_horizontal.is_some());
    assert_eq!(look.edges.inside_horizontal, Some(border.clone()));
    assert_eq!(
        look.cell_margins_twips[..2],
        [Some(40), Some(80)],
        "the margins a table sets over those of the default table style"
    );
    assert_eq!(rows[0].cells[0].look.fill.as_deref(), Some("4472C4"));
    assert_eq!(rows[0].cells[0].look.borders.bottom, Some(border));
    assert_eq!(
        rows[0].cells[0].look.vertical,
        Some(VerticalAlignment::Center)
    );
    assert_eq!(
        rows[0].cells[1].look.vertical,
        Some(VerticalAlignment::Bottom)
    );
}

#[test]
fn puts_the_reference_mark_at_the_start_of_a_note() {
    assert_eq!(
        with_leading_run("<w:p><w:pPr><w:jc/></w:pPr><w:r/></w:p>", "<M/>", "S"),
        "<w:p><w:pPr><w:jc/></w:pPr><M/><w:r/></w:p>"
    );
    assert_eq!(
        with_leading_run("<w:p><w:r/></w:p>", "<M/>", "S"),
        "<w:p><M/><w:r/></w:p>"
    );
    assert_eq!(
        with_leading_run("<w:tbl/>", "<M/>", "S"),
        r#"<w:p><w:pPr><w:pStyle w:val="S"/></w:pPr><M/></w:p><w:tbl/>"#
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "three sections with their pages and headers"
)]
fn writes_each_section_with_its_page_and_headers() {
    let header = |value: &str| vec![paragraph(vec![text(value, Vec::new())])];
    let portrait = PageLayout::default();
    let landscape = PageLayout {
        width_twips: portrait.height_twips,
        height_twips: portrait.width_twips,
        columns: 2,
        column_gap_twips: 500,
        ..PageLayout::default()
    };
    let sections = vec![
        Section {
            end_block: 1,
            layout: portrait.clone(),
            headers_footers: HeadersFooters {
                header: header("Verticale"),
                first_header: Some(header("Copertina")),
                ..HeadersFooters::default()
            },
            continuous: false,
        },
        Section {
            end_block: 3,
            layout: landscape.clone(),
            headers_footers: HeadersFooters {
                header: header("Orizzontale"),
                footer: header("Piede"),
                ..HeadersFooters::default()
            },
            continuous: false,
        },
        Section {
            end_block: 4,
            layout: landscape.clone(),
            headers_footers: HeadersFooters::default(),
            continuous: true,
        },
    ];
    let original = DocumentTree {
        blocks: vec![
            paragraph(vec![text("Uno", Vec::new())]),
            paragraph(vec![text("Due", Vec::new())]),
            Block::Table {
                rows: vec![TableRow {
                    header: false,
                    cells: vec![TableCell {
                        col_span: 1,
                        row_span: 1,
                        blocks: vec![paragraph(vec![text("Cella", Vec::new())])],
                        look: CellLook::default(),
                    }],
                }],
                look: TableLook::default(),
            },
            paragraph(vec![text("Tre", Vec::new())]),
        ],
        layout: landscape.clone(),
        sections,
        ..DocumentTree::default()
    };
    let read = round_trip(&original);
    assert_eq!(plain(&read.blocks)[..2], ["Uno", "Due"]);
    assert_eq!(
        read.blocks.len(),
        4,
        "the paragraphs that carry the sections read as none"
    );
    assert_eq!(read.sections.len(), 3);
    assert_eq!(
        read.sections
            .iter()
            .map(|section| section.end_block)
            .collect::<Vec<_>>(),
        [1, 3, 4]
    );
    assert_eq!(
        (
            read.sections[1].layout.width_twips,
            read.sections[1].layout.columns
        ),
        (16_838, 2)
    );
    assert!(read.sections[2].continuous);
    assert_eq!(
        plain(&read.sections[0].headers_footers.header),
        ["Verticale"]
    );
    assert_eq!(
        plain(
            read.sections[0]
                .headers_footers
                .first_header
                .as_ref()
                .unwrap()
        ),
        ["Copertina"]
    );
    assert_eq!(plain(&read.sections[1].headers_footers.footer), ["Piede"]);
    let last = &read.sections[2].headers_footers;
    assert_eq!(
        (plain(&last.header), plain(&last.footer)),
        (vec![String::new()], vec![String::new()]),
        "written as empty parts, so the section before is not carried on"
    );

    let one = round_trip(&DocumentTree {
        blocks: vec![paragraph(vec![text("Solo", Vec::new())])],
        ..DocumentTree::default()
    });
    assert_eq!(one.sections, Vec::new());
}
