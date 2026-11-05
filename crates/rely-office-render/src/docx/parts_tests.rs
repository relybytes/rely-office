// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! Headers, footers, notes, tabs, labels, borders and shading written as Typst source.

use rely_office_core::{CellLook, HeadersFooters, PageLayout, TableCell};

use super::*;

static LINES: std::sync::LazyLock<FontLines> = std::sync::LazyLock::new(FontLines::default);

fn text(value: &str) -> Inline {
    Inline::Text {
        text: value.to_owned(),
        marks: Vec::new(),
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

fn source(tree: &DocumentTree) -> String {
    document_source(tree, true, &LINES).0
}

fn border(style: BorderStyle, colour: Option<&str>) -> Border {
    Border {
        style,
        width_eighths: 8,
        colour: colour.map(str::to_owned),
    }
}

#[test]
fn draws_headers_and_footers_with_their_variants_and_page_numbers() {
    let page = Inline::Text {
        text: "1".to_owned(),
        marks: Vec::new(),
        look: RunLook {
            field: Some(PageField::Page),
            ..RunLook::default()
        },
    };
    let pages = Inline::Text {
        text: "9".to_owned(),
        marks: Vec::new(),
        look: RunLook {
            field: Some(PageField::Pages),
            ..RunLook::default()
        },
    };
    let tree = DocumentTree {
        blocks: vec![paragraph(vec![text("corpo")])],
        headers_footers: HeadersFooters {
            header: vec![paragraph(vec![text("Testa")])],
            footer: vec![paragraph(vec![page, text(" / "), pages])],
            first_header: Some(vec![paragraph(vec![text("Copertina")])]),
            even_footer: Some(Vec::new()),
            ..HeadersFooters::default()
        },
        layout: PageLayout {
            header_distance_twips: 400,
            footer_distance_twips: 99_999,
            columns: 2,
            column_gap_twips: 400,
            ..PageLayout::default()
        },
        ..DocumentTree::default()
    };
    let source = source(&tree);
    assert!(source.contains("left: 70.85pt), header: context { let page-number = here().page(); if page-number == locate(<rely-section-0>).page() { align(top, pad(top: 20pt, block(width: 100%)["), "{source}");
    assert!(source.contains("\"Copertina\""));
    assert!(source.contains("} else { align(top, pad(top: 20pt, block(width: 100%)[#text("));
    assert!(source.contains(", header-ascent: 0pt, footer: context { let page-number = here().page(); if calc.even(page-number) {"));
    assert!(
        source.contains("align(bottom, pad(bottom: 70.85pt,"),
        "the footer distance is cut to the margin"
    );
    assert!(source.contains("[#context counter(page).display()]"));
    assert!(source.contains("[#context str(counter(page).final().first())]"));
    assert!(source.contains(
        "footer-descent: 0pt)\n#metadata(none) <rely-section-0>\n#columns(2, gutter: 20pt)["
    ));
    assert!(
        !source.contains("\"1\""),
        "a page field shows the page, not its last result"
    );

    let bare = document_source(&DocumentTree::default(), true, &LINES).0;
    assert!(!bare.contains("header:"));
    assert!(!bare.contains("#columns("));
}

#[test]
fn puts_footnotes_on_their_page_and_endnotes_at_the_end() {
    let tree = DocumentTree {
        blocks: vec![paragraph(vec![
            text("a"),
            Inline::NoteReference { note: 0 },
            Inline::NoteReference { note: 1 },
            Inline::NoteReference { note: 2 },
            Inline::NoteReference { note: 1 },
            Inline::NoteReference { note: 9 },
        ])],
        notes: vec![
            Note {
                kind: NoteKind::Footnote,
                blocks: vec![paragraph(vec![
                    text("piede"),
                    Inline::NoteReference { note: 1 },
                ])],
            },
            Note {
                kind: NoteKind::Endnote,
                blocks: vec![paragraph(vec![text("fine")])],
            },
            Note {
                kind: NoteKind::Endnote,
                blocks: vec![paragraph(vec![text("ultima")])],
            },
        ],
        ..DocumentTree::default()
    };
    let source = source(&tree);
    assert!(source.contains("#footnote[#text("), "{source}");
    assert!(
        source.contains("\"piede\"#[]"),
        "a note does not follow a note"
    );
    assert!(source.contains("#super(\"i\")#super(\"ii\")#super(\"i\")#[]"));
    let end = source
        .find("#line(length: 33%, stroke: 0.5pt)")
        .expect("the endnotes");
    let tail = &source[end..];
    assert!(tail.find("\"fine\"").unwrap() < tail.find("\"ultima\"").unwrap());
    assert!(tail.contains("super(\"ii\")"));
    assert!(source.contains("#show footnote.entry: it => grid("));
    assert!(
        !document_source(&DocumentTree::default(), true, &LINES)
            .0
            .contains("#line(length: 33%")
    );
}

#[test]
fn lays_out_text_after_tabs_against_the_stops() {
    let look = ParagraphLook {
        indent_left_twips: Some(720),
        indent_first_line_twips: Some(-360),
        tabs: vec![
            TabStop {
                position_twips: 9_000,
                kind: TabKind::Right,
                leader: TabLeader::Dot,
            },
            TabStop {
                position_twips: 4_000,
                kind: TabKind::Clear,
                leader: TabLeader::None,
            },
            TabStop {
                position_twips: 5_000,
                kind: TabKind::Center,
                leader: TabLeader::MiddleDot,
            },
            TabStop {
                position_twips: 6_000,
                kind: TabKind::Decimal,
                leader: TabLeader::Hyphen,
            },
            TabStop {
                position_twips: 7_000,
                kind: TabKind::Left,
                leader: TabLeader::Underscore,
            },
        ],
        ..ParagraphLook::default()
    };
    let mut writer = Writer {
        media: &[],
        notes: &[],
        with_images: true,
        lines: &LINES,
        body_family: None,
        body_size: 11.0,
        text_width: 400.0,
        default_tab: 35.4,
        spacing_adds_up: false,
        files: HashMap::new(),
        list_counters: HashMap::new(),
        endnotes: Vec::new(),
        in_note: false,
        page: super::PagePlace::default(),
        open_frame: None,
    };
    let placed = writer.paragraph(&[text("Premesse\t1"), text("\tx")], None, &look, None);
    assert!(
        placed.expression.contains(concat!(
            r#"#rely-tabbed(18pt, 35.4pt, ((36pt, "left", none), (250pt, "center", "·"), "#,
            r#"(300pt, "right", "-"), (350pt, "left", "_"), (450pt, "right", "."), ), "#,
            r#"([#"Premesse"], [#"1"], [#"x"], ))"#
        )),
        "{}",
        placed.expression
    );
    let untabbed = writer.paragraph(&[text("senza")], None, &look, None);
    assert!(!untabbed.expression.contains("rely-tabbed"));
    assert!(TAB_FUNCTION.starts_with("#let rely-tabbed(origin, step, stops, parts) = context {"));
}

#[test]
fn draws_list_labels_and_numbered_headings_as_the_document_names_them() {
    let mut writer = Writer {
        media: &[],
        notes: &[],
        with_images: true,
        lines: &LINES,
        body_family: None,
        body_size: 11.0,
        text_width: 400.0,
        default_tab: 36.0,
        spacing_adds_up: false,
        files: HashMap::new(),
        list_counters: HashMap::new(),
        endnotes: Vec::new(),
        in_note: false,
        page: super::PagePlace::default(),
        open_frame: None,
    };
    let label = |suffix| ListLabel {
        text: "Art. 1.".to_owned(),
        suffix,
        font: RunLook {
            bold: Some(true),
            ..RunLook::default()
        },
    };
    let look = |suffix, hanging: Option<i32>| ParagraphLook {
        indent_left_twips: Some(720),
        indent_first_line_twips: hanging,
        list_label: Some(label(suffix)),
        ..ParagraphLook::default()
    };
    let heading = writer.paragraph(
        &[text("Oggetto")],
        Some(Alignment::Center),
        &look(LabelSuffix::Tab, Some(-720)),
        Some(1),
    );
    assert!(
        heading.expression.starts_with("heading(level: 1)[#text("),
        "{}",
        heading.expression
    );
    assert!(heading.expression.contains(r#"grid(columns: (calc.max(36pt, measure([#text(weight: "bold", "Art. 1.")]).width + 2pt), 1fr), column-gutter: 0pt"#));
    assert!(heading.expression.contains("align(center, par("));
    let spaced = writer.paragraph(&[text("x")], None, &look(LabelSuffix::Space, None), None);
    assert!(
        spaced
            .expression
            .contains("columns: (auto, 1fr), column-gutter: 0.25em")
    );
    let tight = writer.paragraph(&[text("x")], None, &look(LabelSuffix::Nothing, None), None);
    assert!(
        tight
            .expression
            .contains("columns: (auto, 1fr), column-gutter: 0pt")
    );
    let tabbed = writer.paragraph(&[text("x")], None, &look(LabelSuffix::Tab, None), None);
    assert!(
        tabbed
            .expression
            .contains("columns: (auto, 1fr), column-gutter: 18pt")
    );
    assert!(tabbed.expression.starts_with("pad(left: 36pt"));
}

#[test]
fn frames_shaded_and_bordered_paragraphs_and_highlights_runs() {
    let look = ParagraphLook {
        shading: Some("DEEAF6".to_owned()),
        borders: Borders {
            top: Some(border(BorderStyle::Single, Some("2F5496"))),
            bottom: Some(border(BorderStyle::None, None)),
            ..Borders::default()
        },
        ..ParagraphLook::default()
    };
    let framed_expression = framed(&look, "x".to_owned());
    assert_eq!(
        framed_expression,
        r##"block(width: 100%, fill: rgb("#DEEAF6"), stroke: (top: (paint: rgb("#2F5496"), thickness: 1pt, dash: "solid"), right: none, bottom: none, left: none), outset: (top: 1pt, right: 4pt, bottom: 1pt, left: 4pt), x)"##
    );
    let shaded_only = ParagraphLook {
        shading: Some("FFFF00".to_owned()),
        ..ParagraphLook::default()
    };
    assert!(!framed(&shaded_only, "x".to_owned()).contains("stroke"));
    assert_eq!(framed(&ParagraphLook::default(), "x".to_owned()), "x");

    let highlighted = RunLook {
        highlight: Some("FFFF00".to_owned()),
        caps: Some(true),
        ..RunLook::default()
    };
    assert_eq!(
        marked("a", &[Mark::Bold], &highlighted, &RunLook::default()),
        r##"highlight(fill: rgb("#FFFF00"), text(weight: "bold", upper("a")))"##
    );
    let small = RunLook {
        small_caps: Some(true),
        ..RunLook::default()
    };
    assert_eq!(
        marked("b", &[], &RunLook::default(), &small),
        r#"smallcaps("b")"#,
        "a paragraph's font applies to its runs"
    );
    assert_eq!(
        marked(
            "c",
            &[Mark::TextStyle {
                font_family: None,
                font_size: None,
                color: None
            }],
            &RunLook::default(),
            &RunLook::default()
        ),
        r#""c""#
    );
}

#[test]
fn strokes_each_cell_side_naming_its_dash_or_the_default_line() {
    let sides: CellSides = [
        Some(border(BorderStyle::Single, Some("FF0000"))),
        Some(border(BorderStyle::Dotted, None)),
        Some(border(BorderStyle::None, None)),
        None,
    ];
    assert_eq!(
        cell_stroke(&sides, false),
        r##"(top: (paint: rgb("#FF0000"), thickness: 1pt, dash: "solid"), bottom: (paint: black, thickness: 1pt, dash: "dotted"), left: none, right: none, )"##
    );
    assert_eq!(
        cell_stroke(&[None, None, None, None], true),
        "(top: 0.5pt, bottom: 0.5pt, left: 0.5pt, right: 0.5pt, )"
    );
    assert_eq!(
        cell_stroke(&[None, None, None, None], false),
        "(top: none, bottom: none, left: none, right: none, )"
    );
}

#[test]
fn draws_a_double_border_as_two_bands_three_widths_wide() {
    let double = border(BorderStyle::Double, Some("C00000"));
    assert_eq!(
        border_stroke(&double, LineDirection::Across),
        r##"(paint: tiling(size: (1pt, 3pt), relative: "self")[#place(dy: 0.5pt, rect(width: 1pt, height: 2pt, fill: rgb("#C00000"), stroke: none))], thickness: 3pt, dash: "solid", cap: "butt")"##
    );
    assert_eq!(
        border_stroke(&double, LineDirection::Down),
        r##"(paint: tiling(size: (3pt, 1pt), relative: "self")[#place(dx: 0.5pt, rect(width: 2pt, height: 1pt, fill: rgb("#C00000"), stroke: none))], thickness: 3pt, dash: "solid", cap: "butt")"##
    );
    assert_eq!(
        border_stroke(&border(BorderStyle::Dashed, None), LineDirection::Down),
        r#"(paint: black, thickness: 1pt, dash: "dashed")"#
    );
    assert_eq!(
        border_stroke(&border(BorderStyle::None, None), LineDirection::Across),
        "none"
    );
}

#[test]
fn decides_a_shared_edge_as_word_does() {
    let single = border(BorderStyle::Single, Some("000000"));
    let heavy_single = Border {
        width_eighths: 24,
        ..border(BorderStyle::Single, Some("111111"))
    };
    let double = border(BorderStyle::Double, Some("C00000"));
    let none = border(BorderStyle::None, None);
    let inside = border(BorderStyle::Dashed, Some("808080"));
    assert_eq!(
        shared_edge(None, &[], Some(&inside)),
        Some(inside.clone()),
        "nothing declared: the table's inside line"
    );
    assert_eq!(
        shared_edge(None, &[Some(&double)], Some(&inside)),
        Some(double.clone()),
        "a neighbour's own border beats the inside line"
    );
    assert_eq!(
        shared_edge(Some(&single), &[Some(&double)], Some(&inside)),
        Some(double.clone()),
        "the heavier of two own borders"
    );
    assert_eq!(
        shared_edge(Some(&heavy_single), &[Some(&double)], None),
        Some(heavy_single.clone()),
        "weight is width times the style's number"
    );
    assert_eq!(
        shared_edge(Some(&single), &[Some(&single.clone())], None),
        Some(single.clone()),
        "a tie keeps the cell's own"
    );
    assert_eq!(
        shared_edge(Some(&none), &[Some(&double)], Some(&inside)),
        Some(double.clone()),
        "none on one side does not hide the other's line"
    );
    assert_eq!(
        shared_edge(Some(&none), &[None], Some(&inside)),
        Some(none.clone()),
        "only none declared: no line, not the inside one"
    );
    assert_eq!(border_weight(&none), 0);
    assert_eq!(border_weight(&single), 8);
    assert_eq!(border_weight(&double), 24);
    assert_eq!(border_weight(&border(BorderStyle::Dotted, None)), 32);
    assert_eq!(border_weight(&border(BorderStyle::Dashed, None)), 40);
}

#[test]
fn gives_both_cells_of_an_edge_the_same_border() {
    let cell = |borders: Borders| TableCell {
        col_span: 1,
        row_span: 1,
        blocks: Vec::new(),
        look: CellLook {
            borders,
            ..CellLook::default()
        },
    };
    let double = border(BorderStyle::Double, Some("C00000"));
    let rows = vec![
        TableRow {
            header: false,
            cells: vec![cell(Borders::default()), cell(Borders::default())],
        },
        TableRow {
            header: false,
            cells: vec![
                cell(Borders::default()),
                cell(Borders {
                    top: Some(double.clone()),
                    bottom: Some(double.clone()),
                    ..Borders::default()
                }),
            ],
        },
        TableRow {
            header: false,
            cells: vec![TableCell {
                col_span: 2,
                ..cell(Borders::default())
            }],
        },
    ];
    let edge = border(BorderStyle::Single, Some("1F4E79"));
    let inside = border(BorderStyle::Dashed, Some("808080"));
    let table = Borders {
        top: Some(edge.clone()),
        left: Some(edge.clone()),
        bottom: Some(edge.clone()),
        right: Some(edge.clone()),
        inside_horizontal: Some(inside.clone()),
        inside_vertical: Some(inside.clone()),
    };
    let sides = cell_sides(&rows, &place_cells(&rows), &table);
    assert_eq!(
        sides[0][1][BOTTOM],
        Some(double.clone()),
        "the cell above takes the cell's top"
    );
    assert_eq!(sides[1][1][TOP], Some(double.clone()));
    assert_eq!(sides[1][1][BOTTOM], Some(double.clone()));
    assert_eq!(
        sides[2][0][TOP],
        Some(double.clone()),
        "a spanning cell takes the heaviest of the cells above it"
    );
    assert_eq!(sides[0][0][BOTTOM], Some(inside.clone()));
    assert_eq!(
        sides[0][0][TOP],
        Some(edge.clone()),
        "an outer side is the table's edge"
    );
    assert_eq!(sides[0][0][LEFT], Some(edge.clone()));
    assert_eq!(sides[0][0][RIGHT], Some(inside.clone()));
    assert_eq!(sides[0][1][RIGHT], Some(edge.clone()));
    assert_eq!(sides[2][0][BOTTOM], Some(edge.clone()));
}

#[test]
fn writes_the_fill_alignment_and_edges_of_table_cells() {
    let cell = |value: &str, look: CellLook| TableCell {
        col_span: 1,
        row_span: 1,
        blocks: vec![paragraph(vec![text(value)])],
        look,
    };
    let rows = vec![TableRow {
        header: false,
        cells: vec![
            cell(
                "a",
                CellLook {
                    fill: Some("4472C4".to_owned()),
                    vertical: Some(VerticalAlignment::Center),
                    ..CellLook::default()
                },
            ),
            cell(
                "b",
                CellLook {
                    vertical: Some(VerticalAlignment::Bottom),
                    ..CellLook::default()
                },
            ),
            cell(
                "c",
                CellLook {
                    vertical: Some(VerticalAlignment::Top),
                    ..CellLook::default()
                },
            ),
        ],
    }];
    let tree = DocumentTree {
        blocks: vec![Block::Table {
            rows: rows.clone(),
            look: TableLook {
                edges: Borders {
                    top: Some(border(BorderStyle::Single, None)),
                    ..Borders::default()
                },
                ..TableLook::default()
            },
        }],
        ..DocumentTree::default()
    };
    let source = source(&tree);
    assert!(source.contains(r##"table.cell(fill: rgb("#4472C4"), align: horizon, stroke: (top: (paint: black, thickness: 1pt, dash: "solid"), bottom: none, left: none, right: none, ))"##), "{source}");
    assert!(source.contains("table.cell(align: bottom, stroke:"));
    assert!(source.contains("table.cell(align: top, stroke:"));
    assert!(source.contains("stroke: none, inset:"));

    let plain_tree = DocumentTree {
        blocks: vec![Block::Table {
            rows,
            look: TableLook::default(),
        }],
        ..DocumentTree::default()
    };
    assert!(
        !super::document_source(&plain_tree, true, &LINES)
            .0
            .contains("stroke: (top")
    );
}

#[test]
fn sets_the_page_of_each_section() {
    let landscape = PageLayout {
        width_twips: 16_838,
        height_twips: 11_906,
        ..PageLayout::default()
    };
    let wide = PageLayout {
        columns: 2,
        column_gap_twips: 400,
        ..landscape.clone()
    };
    let header = vec![paragraph(vec![text("Orizzontale")])];
    let tree = DocumentTree {
        blocks: vec![
            paragraph(vec![text("Uno")]),
            paragraph(vec![text("Due")]),
            paragraph(vec![text("Tre")]),
        ],
        layout: wide.clone(),
        sections: vec![
            Section {
                end_block: 1,
                layout: PageLayout::default(),
                headers_footers: HeadersFooters::default(),
                continuous: false,
            },
            Section {
                end_block: 2,
                layout: landscape.clone(),
                headers_footers: HeadersFooters {
                    header,
                    first_header: Some(Vec::new()),
                    ..HeadersFooters::default()
                },
                continuous: false,
            },
            Section {
                end_block: 3,
                layout: wide,
                headers_footers: HeadersFooters::default(),
                continuous: true,
            },
        ],
        ..DocumentTree::default()
    };
    let written = source(&tree);
    assert_eq!(
        written.matches("#set page(").count(),
        2,
        "the continuous section goes on: {written}"
    );
    assert!(written.contains("#set page(width: 595.3pt, height: 841.9pt,"));
    assert!(written.contains("#set page(width: 841.9pt, height: 595.3pt,"));
    assert!(written.contains("if page-number == locate(<rely-section-1>).page() {"));
    for section in 0..3 {
        assert!(written.contains(&format!("#metadata(none) <rely-section-{section}>")));
    }
    assert!(
        written.contains("#columns(2, gutter: 20pt)["),
        "columns within the same page"
    );
    let first = written.find("\"Uno\"").unwrap();
    let landscape_page = written.find("height: 595.3pt").unwrap();
    assert!(first < landscape_page && landscape_page < written.find("\"Due\"").unwrap());

    let restarting = DocumentTree {
        sections: vec![
            Section {
                end_block: 1,
                layout: landscape.clone(),
                headers_footers: HeadersFooters {
                    header: vec![paragraph(vec![text("Testa")])],
                    ..HeadersFooters::default()
                },
                continuous: false,
            },
            Section {
                end_block: 3,
                layout: PageLayout::default(),
                headers_footers: HeadersFooters::default(),
                continuous: true,
            },
        ],
        ..tree
    };
    let written = source(&restarting);
    assert_eq!(
        written.matches("#set page(").count(),
        2,
        "a continuous section on another page starts one"
    );
    assert!(
        written.contains(", header: none, footer: none)"),
        "no header carried on"
    );
}

#[test]
fn a_double_rule_between_rows_takes_its_room() {
    let cell = |top: Option<Border>| TableCell {
        col_span: 1,
        row_span: 1,
        blocks: Vec::new(),
        look: CellLook {
            borders: Borders {
                top,
                ..Borders::default()
            },
            ..CellLook::default()
        },
    };
    let rows = vec![TableRow {
        header: false,
        cells: vec![
            cell(Some(border(BorderStyle::Single, None))),
            cell(Some(Border {
                width_eighths: 2,
                ..border(BorderStyle::Double, None)
            })),
        ],
    }];
    assert!((double_rule(&rows, &Borders::default()) - 0.75).abs() < 1e-9);
    let table = Borders {
        inside_horizontal: Some(border(BorderStyle::Double, None)),
        ..Borders::default()
    };
    assert!((double_rule(&rows, &table) - 3.0).abs() < 1e-9);
    assert!(double_rule(&[], &Borders::default()).abs() < 1e-9);
}
