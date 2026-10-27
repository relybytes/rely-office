// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use rely_office_core::{CellLook, ImportReport, PageLayout, TableCell};

use super::*;

static LINES: std::sync::LazyLock<FontLines> = std::sync::LazyLock::new(FontLines::default);

fn text(value: &str) -> Inline {
    Inline::Text {
        text: value.to_owned(),
        marks: Vec::new(),
        look: RunLook::default(),
    }
}

fn paragraph(value: &str) -> Block {
    Block::Paragraph {
        content: vec![text(value)],
        align: None,
        look: ParagraphLook::default(),
    }
}

fn cell(value: &str, col_span: u32, row_span: u32) -> TableCell {
    TableCell {
        col_span,
        row_span,
        blocks: vec![paragraph(value)],
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

fn writer(media: &[Media]) -> Writer<'_> {
    Writer {
        media,
        with_images: true,
        lines: &LINES,
        body_family: None,
        body_size: 11.0,
        text_width: 400.0,
        default_tab: 35.4,
        notes: &[],
        files: HashMap::new(),
        list_counters: HashMap::new(),
        endnotes: Vec::new(),
        in_note: false,
        page: super::PagePlace::default(),
        open_frame: None,
    }
}

#[test]
fn sets_the_page_and_the_body_font_of_the_document() {
    let mut document = tree(Vec::new(), Vec::new());
    document.layout.font_family = Some("Cambria".to_owned());
    document.layout.font_size_half_points = Some(24);
    let (source, files) = document_source(&document, true, &LINES);
    assert!(source.contains(
        "#set page(width: 595.3pt, height: 841.9pt, margin: (top: 70.85pt, right: 70.85pt, bottom: 70.85pt, left: 70.85pt))"
    ));
    assert!(
        source.contains(
            r#"font: ("Cambria", "Caladea", "Carlito", "Liberation Sans", ), size: 12pt"#
        )
    );
    assert_eq!(files.len(), 0);
}

#[test]
fn writes_text_only_as_string_literals() {
    let mut writer = writer(&[]);
    let paragraph = Block::Paragraph {
        content: vec![
            Inline::Text {
                text: "#set page(width: 1pt) ] [".to_owned(),
                marks: vec![
                    Mark::Bold,
                    Mark::Link {
                        href: "https://a.it/\"x".to_owned(),
                    },
                ],
                look: RunLook::default(),
            },
            Inline::HardBreak,
            text("fine"),
        ],
        align: Some(Alignment::Justify),
        look: ParagraphLook::default(),
    };
    let placed = writer.place(&paragraph);
    assert!(placed.expression.ends_with(
        r##"par(leading: 1.65pt, justify: true)[#link("https://a.it/\"x", text(weight: "bold", "#set page(width: 1pt) ] ["))#linebreak()#"fine"])"##
    ), "{}", placed.expression);
    let empty = writer.place(&Block::Paragraph {
        content: Vec::new(),
        align: None,
        look: ParagraphLook::default(),
    });
    assert!(empty.expression.ends_with("[#hide[x]])"));
}

#[test]
fn applies_the_look_of_a_paragraph() {
    let mut writer = writer(&[]);
    let look = ParagraphLook {
        space_before_twips: Some(240),
        space_after_twips: Some(120),
        line_spacing: Some(LineSpacing::Auto(480)),
        indent_left_twips: Some(720),
        indent_right_twips: Some(360),
        indent_first_line_twips: Some(-360),
        page_break_before: true,
        font: RunLook {
            family: Some("Arial".to_owned()),
            size_half_points: Some(28),
            colour: Some("FF0000".to_owned()),
            bold: Some(true),
            italic: Some(true),
            ..RunLook::default()
        },
        ..ParagraphLook::default()
    };
    let placed = writer.paragraph(&[text("a")], Some(Alignment::Right), &look, None);
    assert_eq!((placed.before, placed.page_break), (12.0, true));
    assert!(
        (placed.after - (6.0 + 18.2)).abs() < 1e-9,
        "{}",
        placed.after
    );
    assert_eq!(
        placed.expression,
        r##"pad(left: 18pt, right: 18pt, align(right, text(font: ("Arial", "Liberation Sans", "Carlito", ), size: 14pt, weight: "bold", style: "italic", fill: rgb("#FF0000"), par(leading: 18.2pt, hanging-indent: 18pt)[#"a"])))"##
    );

    let exact = ParagraphLook {
        line_spacing: Some(LineSpacing::Exact(300)),
        indent_first_line_twips: Some(240),
        ..ParagraphLook::default()
    };
    let placed = writer.paragraph(&[text("b")], None, &exact, Some(2));
    assert!(placed.expression.starts_with("heading(level: 2)[#text("));
    assert!(placed.expression.contains("size: 13pt"));
    assert!(placed.expression.contains(r##"fill: rgb("#2F5496")"##));
    assert!(
        placed
            .expression
            .contains("leading: 2pt, first-line-indent: (amount: 12pt, all: true)")
    );
}

#[test]
fn adds_the_spacing_of_two_blocks_and_breaks_pages() {
    let mut writer = writer(&[]);
    let spaced = |before: u32, after: u32, page_break: bool| Block::Paragraph {
        content: vec![text("x")],
        align: None,
        look: ParagraphLook {
            space_before_twips: Some(before),
            space_after_twips: Some(after),
            page_break_before: page_break,
            ..ParagraphLook::default()
        },
    };
    let markup = writer.blocks(&[
        spaced(60, 120, false),
        spaced(240, 0, false),
        spaced(100, 0, true),
    ]);
    let lines: Vec<&str> = markup
        .lines()
        .filter(|line| !line.starts_with("#text("))
        .collect();
    assert_eq!(
        lines,
        vec![
            "#v(3pt)",
            "#v(19.65pt, weak: true)",
            "#pagebreak(weak: true)",
            "#v(5pt, weak: true)"
        ]
    );
}

#[test]
fn numbers_lists_per_list_and_level_and_indents_them() {
    let mut writer = writer(&[]);
    let markers: Vec<String> = [(0, 1), (0, 1), (1, 1), (1, 1), (0, 1), (2, 1), (0, 2)]
        .iter()
        .map(|(level, list)| writer.list_marker(true, *level, *list))
        .collect();
    assert_eq!(markers, vec!["1.", "2.", "a.", "b.", "3.", "i.", "1."]);
    assert_eq!(writer.list_marker(false, 1, 3), "\u{25e6}");
    assert_eq!(alphabetic(27), "aa");
    assert_eq!(roman(1_994), "mcmxciv");

    let indented = writer.list_item(
        false,
        0,
        9,
        &[text("voce")],
        &ParagraphLook {
            indent_left_twips: Some(1_440),
            indent_first_line_twips: Some(-720),
            ..ParagraphLook::default()
        },
    );
    assert!(
        indented
            .expression
            .starts_with("pad(left: 36pt, right: 0pt, text(")
    );
    assert!(
        indented
            .expression
            .contains("grid(columns: (calc.max(36pt, measure("),
        "{}",
        indented.expression
    );
    let defaulted = writer.list_item(false, 1, 9, &[text("voce")], &ParagraphLook::default());
    assert!(
        defaulted
            .expression
            .starts_with("pad(left: 18pt, right: 0pt, text(")
    );
}

#[test]
fn counts_the_columns_a_table_fills_with_its_spans() {
    let rows = vec![
        TableRow {
            header: true,
            cells: vec![cell("a", 1, 2), cell("b", 2, 1)],
        },
        TableRow {
            header: false,
            cells: vec![cell("c", 1, 1), cell("d", 1, 1)],
        },
    ];
    assert_eq!(place_cells(&rows).columns, 3);
    assert_eq!(place_cells(&[]).columns, 0);
    assert_eq!(header_row_count(&rows), 0);

    let mut writer = writer(&[]);
    let table = writer.table(&rows, &TableLook::default());
    assert!(table.starts_with(
        "table(columns: (1fr, 1fr, 1fr, ), stroke: none, inset: (top: 0pt, left: 5.4pt, bottom: 0pt, right: 5.4pt), table.cell(rowspan: 2)"
    ));
    assert!(!table.contains("table.header"));

    let rows = vec![
        TableRow {
            header: true,
            cells: vec![cell("a", 1, 1), cell("b", 1, 1)],
        },
        TableRow {
            header: false,
            cells: vec![cell("c", 1, 5), cell("d", 1, 1)],
        },
    ];
    assert_eq!(header_row_count(&rows), 1);
    let bordered = TableLook {
        column_widths_twips: vec![4_000, 12_000],
        borders: true,
        cell_margins_twips: [Some(28), None, Some(40), Some(20)],
        edges: Borders::default(),
        frame: None,
    };
    let table = writer.table(&rows, &bordered);
    assert!(table.starts_with("table(columns: (100pt, 300pt, ), stroke: 0.5pt, inset: (top: 1.4pt, left: 5.4pt, bottom: 2pt, right: 1pt), table.header(["));
    assert!(!table.contains("rowspan: 5"));
    let narrow = TableLook {
        column_widths_twips: vec![2_000, 2_000],
        ..TableLook::default()
    };
    assert!(
        writer
            .table(&rows, &narrow)
            .starts_with("table(columns: (100pt, 100pt, ),")
    );

    let wide = vec![TableRow {
        header: false,
        cells: vec![cell("a", 1_000, 1)],
    }];
    assert_eq!(place_cells(&wide).columns, MAX_COLUMNS);
}

#[test]
fn sizes_images_within_the_text_width_and_frames_what_typst_cannot_read() {
    let media = vec![
        Media {
            content_type: "image/png",
            bytes: vec![1],
        },
        Media {
            content_type: "image/x-emf",
            bytes: vec![2],
        },
    ];
    let blocks = vec![Block::Paragraph {
        content: vec![
            Inline::Image {
                media: 0,
                alt: None,
                width: Some(1_000),
                height: Some(500),
                anchor: None,
            },
            Inline::Image {
                media: 1,
                alt: None,
                width: Some(100),
                height: None,
                anchor: None,
            },
            Inline::Image {
                media: 0,
                alt: None,
                width: None,
                height: None,
                anchor: None,
            },
        ],
        align: None,
        look: ParagraphLook::default(),
    }];
    let (source, files) = document_source(&tree(blocks.clone(), media.clone()), true, &LINES);
    assert!(source.contains(
        r#"box(image("/media/0.png", width: 453.6pt, height: 226.8pt, fit: "stretch"))"#
    ));
    assert!(source.contains("box(width: 75pt, stroke: 0.5pt + luma(180))"));
    assert_eq!(files, vec![("/media/0.png".to_owned(), vec![1])]);

    let (source, files) = document_source(&tree(blocks, media), false, &LINES);
    assert!(!source.contains("image("));
    assert_eq!(files.len(), 0);
}

#[test]
fn sets_the_font_of_a_run() {
    let look = RunLook {
        family: Some("Courier New".to_owned()),
        size_half_points: Some(18),
        colour: Some("00FF00".to_owned()),
        ..RunLook::default()
    };
    assert_eq!(
        marked("x", &[Mark::Italic], &look, &RunLook::default()),
        r##"text(style: "italic", text(font: ("Courier New", "Liberation Mono", "Carlito", "Liberation Sans", ), size: 9pt, fill: rgb("#00FF00"), "x"))"##
    );
    assert_eq!(
        marked("y", &[], &RunLook::default(), &RunLook::default()),
        r#""y""#
    );
}
