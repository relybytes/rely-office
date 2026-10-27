// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use rely_office_core::{FillShade, GradientStop, ShapeLine, SlideTableCell, SlideTableRow};

use super::*;

static LINES: std::sync::LazyLock<FontLines> = std::sync::LazyLock::new(FontLines::default);

fn frame(x: i64, y: i64, width: i64, height: i64) -> Frame {
    Frame {
        x,
        y,
        width,
        height,
    }
}

fn run(text: &str) -> SlideRun {
    SlideRun {
        text: text.to_owned(),
        size: 2_400,
        bold: false,
        italic: false,
        underline: false,
        strike: false,
        colour: "112233".to_owned(),
        family: "Calibri".to_owned(),
        line_break: false,
    }
}

fn paragraph(runs: Vec<SlideRun>) -> SlideParagraph {
    SlideParagraph {
        level: 0,
        align: Alignment::Left,
        bullet: None,
        margin_left_emu: 0,
        indent_emu: 0,
        line_spacing_percent: 100,
        space_before: 0,
        space_after: 0,
        runs,
    }
}

fn body(paragraphs: Vec<SlideParagraph>) -> TextBody {
    TextBody {
        paragraphs,
        anchor: TextAnchor::Top,
        insets_emu: [91_440, 45_720, 91_440, 45_720],
        wrap: true,
    }
}

fn shape(geometry: ShapeGeometry, content: ShapeContent) -> SlideShape {
    SlideShape {
        frame: frame(127_000, 254_000, 1_270_000, 635_000),
        rotation: 0.0,
        geometry,
        fill: None,
        line: None,
        content,
    }
}

fn presentation(slides: Vec<Slide>, media: Vec<Media>) -> Presentation {
    Presentation {
        width_emu: 12_192_000,
        height_emu: 6_858_000,
        slides,
        media,
        truncated: false,
    }
}

fn source(presentation: &Presentation, with_images: bool) -> (String, Vec<(String, Vec<u8>)>) {
    presentation_source(presentation, with_images, &LINES)
}

#[test]
fn sets_one_page_per_slide_at_the_slide_size() {
    let slides = vec![
        Slide {
            background: Some(Paint::Solid("1F3864".to_owned())),
            shapes: Vec::new(),
        },
        Slide {
            background: None,
            shapes: Vec::new(),
        },
    ];
    let (markup, files) = source(&presentation(slides, Vec::new()), true);
    assert!(markup.starts_with("#set page(width: 960pt, height: 540pt, margin: 0pt)"));
    assert!(markup.contains("#page(fill: rgb(\"#1F3864\"))["));
    assert!(markup.contains("#page(fill: white)["));
    assert_eq!(files, Vec::<(String, Vec<u8>)>::new());
    let (empty, _) = source(&presentation(Vec::new(), Vec::new()), true);
    assert!(
        empty.contains("#page[]"),
        "an empty presentation still has a page"
    );
}

#[test]
fn draws_shapes_with_their_fill_outline_geometry_and_rotation() {
    let mut rounded = shape(
        ShapeGeometry::RoundedRectangle,
        ShapeContent::Text(body(vec![paragraph(vec![run("Novità")])])),
    );
    rounded.fill = Some(Paint::Solid("ED7D31".to_owned()));
    rounded.line = Some(ShapeLine {
        colour: "FFFFFF".to_owned(),
        width_emu: 25_400,
    });
    rounded.rotation = 45.5;
    let ellipse = shape(ShapeGeometry::Ellipse, ShapeContent::None);
    let line = shape(ShapeGeometry::Line { flipped: true }, ShapeContent::None);
    let mut coloured_line = shape(ShapeGeometry::Line { flipped: false }, ShapeContent::None);
    coloured_line.line = Some(ShapeLine {
        colour: "ED7D31".to_owned(),
        width_emu: 0,
    });
    let slide = Slide {
        background: None,
        shapes: vec![rounded, ellipse, line, coloured_line],
    };
    let (markup, _) = source(&presentation(vec![slide], Vec::new()), true);
    assert!(markup.contains("#place(top + left, dx: 10pt, dy: 20pt, rotate(45.5deg, origin: center + horizon, block(width: 100pt, height: 50pt, fill: rgb(\"#ED7D31\"), stroke: 2pt + rgb(\"#FFFFFF\"), radius: 8.33pt"), "{markup}");
    assert!(markup.contains("\"Novità\""));
    assert!(
        markup.contains(
            "ellipse(width: 100pt, height: 50pt, fill: none, stroke: none, inset: 0pt, [])"
        )
    );
    assert!(markup.contains("line(start: (0pt, 50pt), end: (100pt, 0pt), stroke: 0.75pt + black)"));
    assert!(markup.contains(
        "line(start: (0pt, 0pt), end: (100pt, 50pt), stroke: 0.25pt + rgb(\"#ED7D31\"))"
    ));
}

#[test]
fn sets_paragraphs_with_their_bullets_alignment_spacing_and_runs() {
    let mut bulleted = paragraph(vec![run("Uno")]);
    bulleted.bullet = Some(Bullet::Character("•".to_owned()));
    bulleted.margin_left_emu = 228_600;
    bulleted.indent_emu = -228_600;
    bulleted.space_after = 600;
    let mut numbered = paragraph(vec![
        SlideRun {
            bold: true,
            italic: true,
            underline: true,
            strike: true,
            ..run("Due")
        },
        SlideRun {
            line_break: true,
            ..run("")
        },
        run("dopo"),
    ]);
    numbered.bullet = Some(Bullet::Number("2.".to_owned()));
    numbered.align = Alignment::Justify;
    numbered.space_before = 1_200;
    let mut centred = paragraph(Vec::new());
    centred.align = Alignment::Center;
    let mut right = paragraph(vec![run("Fine")]);
    right.align = Alignment::Right;
    right.indent_emu = 127_000;
    let mut text = body(vec![bulleted, numbered, centred, right]);
    text.anchor = TextAnchor::Bottom;
    text.wrap = false;
    let slide = Slide {
        background: None,
        shapes: vec![shape(ShapeGeometry::Rectangle, ShapeContent::Text(text))],
    };
    let (markup, _) = source(&presentation(vec![slide], Vec::new()), true);
    assert!(
        markup.contains(
            "inset: (left: 7.2pt, top: 3.6pt, right: 7.2pt, bottom: 3.6pt), align(bottom, box["
        ),
        "{markup}"
    );
    assert!(
        markup.contains("pad(left: 0pt, grid(columns: (18pt, 1fr), text("),
        "{markup}"
    );
    assert!(markup.contains("\"•\""));
    assert!(markup.contains("#v(18pt)"), "6 points after and 12 before");
    assert!(markup.contains("\"2.\""));
    assert!(markup.contains("justify: true"));
    assert!(markup.contains("strike(underline(text("));
    assert!(markup.contains("weight: \"bold\", style: \"italic\""));
    assert!(markup.contains("#linebreak()"));
    assert!(
        markup.contains("#hide[x]"),
        "an empty paragraph keeps its line"
    );
    assert!(markup.contains("align(center, pad(left: 0pt"));
    assert!(markup.contains("align(right, pad(left: 10pt"));
}

#[test]
fn places_pictures_only_when_their_images_can_be_drawn() {
    let media = vec![
        Media {
            content_type: "image/png",
            bytes: vec![1, 2, 3],
        },
        Media {
            content_type: "image/x-emf",
            bytes: vec![4],
        },
    ];
    let mut line = shape(ShapeGeometry::Rectangle, ShapeContent::Picture { media: 0 });
    line.line = Some(ShapeLine {
        colour: "000000".to_owned(),
        width_emu: 12_700,
    });
    let slide = Slide {
        background: None,
        shapes: vec![
            line,
            shape(ShapeGeometry::Rectangle, ShapeContent::Picture { media: 0 }),
            shape(ShapeGeometry::Rectangle, ShapeContent::Picture { media: 1 }),
            shape(ShapeGeometry::Rectangle, ShapeContent::Picture { media: 7 }),
        ],
    };
    let deck = presentation(vec![slide], media);
    let (markup, files) = source(&deck, true);
    assert_eq!(
        files,
        vec![("/media/slides/0.png".to_owned(), vec![1, 2, 3])],
        "one file per image"
    );
    assert!(markup.contains("stroke: 1pt + rgb(\"#000000\"), image(\"/media/slides/0.png\", width: 100%, height: 100%, fit: \"stretch\")"), "{markup}");
    assert_eq!(
        markup.matches("stroke: 0.5pt + luma(180)").count(),
        2,
        "the image Typst cannot read and the missing one"
    );
    let (without, files) = source(&deck, false);
    assert_eq!(files, Vec::<(String, Vec<u8>)>::new());
    assert_eq!(without.matches("stroke: 0.5pt + luma(180)").count(), 4);
}

#[test]
fn draws_tables_with_their_grid_spans_and_fills() {
    let cell = |text: &str| SlideTableCell {
        body: body(vec![paragraph(vec![run(text)])]),
        fill: None,
        col_span: 1,
        row_span: 1,
        covered: false,
    };
    let table = SlideTable {
        column_widths_emu: vec![1_270_000, 635_000],
        rows: vec![
            SlideTableRow {
                height_emu: 381_000,
                cells: vec![
                    SlideTableCell {
                        fill: Some("2F5597".to_owned()),
                        col_span: 2,
                        ..cell("Testata")
                    },
                    SlideTableCell {
                        covered: true,
                        ..cell("")
                    },
                ],
            },
            SlideTableRow {
                height_emu: 0,
                cells: vec![
                    SlideTableCell {
                        row_span: 2,
                        ..cell("A")
                    },
                    cell("B"),
                ],
            },
        ],
    };
    let slide = Slide {
        background: None,
        shapes: vec![shape(ShapeGeometry::Rectangle, ShapeContent::Table(table))],
    };
    let (markup, _) = source(&presentation(vec![slide], Vec::new()), true);
    assert!(markup.contains("block(width: 100pt, table(columns: (100pt, 50pt, ), rows: (30pt, auto, ), stroke: 0.5pt + luma(160), inset: 0pt,"), "{markup}");
    assert!(markup.contains("colspan: 2, fill: rgb(\"#2F5597\")"));
    assert!(markup.contains("rowspan: 2"));
    assert_eq!(
        markup.matches("table.cell(").count(),
        3,
        "the covered cell is not drawn"
    );

    let ragged = SlideTable {
        column_widths_emu: Vec::new(),
        rows: vec![SlideTableRow {
            height_emu: 0,
            cells: vec![cell("x"), cell("y")],
        }],
    };
    let empty = SlideTable {
        column_widths_emu: Vec::new(),
        rows: Vec::new(),
    };
    let slide = Slide {
        background: None,
        shapes: vec![
            shape(ShapeGeometry::Rectangle, ShapeContent::Table(ragged)),
            shape(ShapeGeometry::Rectangle, ShapeContent::Table(empty)),
        ],
    };
    let (markup, _) = source(&presentation(vec![slide], Vec::new()), true);
    assert!(markup.contains("columns: (1fr, 1fr, )"));
    assert!(markup.contains("dy: 20pt, [])"));
}

#[test]
fn rounds_angles_and_names_image_types() {
    assert_eq!(round(90.0), "90");
    assert_eq!(round(12.345), "12.35");
    assert_eq!(round(-0.5), "-0.5");
    assert_eq!(image_extension("image/jpeg"), Some("jpg"));
    assert_eq!(image_extension("image/gif"), Some("gif"));
    assert_eq!(image_extension("image/webp"), Some("webp"));
    assert_eq!(image_extension("image/svg+xml"), Some("svg"));
    assert_eq!(image_extension("image/tiff"), None);
}

#[test]
fn draws_a_path_geometry_as_curves_under_its_text_its_faces_before_its_outlines() {
    let at = |x: i64, y: i64| PathPoint { x, y };
    let paths = vec![
        ShapePath {
            commands: vec![
                PathCommand::MoveTo(at(0, 0)),
                PathCommand::LineTo(at(127_000, 0)),
                PathCommand::CubicTo(at(127_000, 12_700), at(0, 12_700), at(0, 25_400)),
                PathCommand::Close,
            ],
            filled: true,
            stroked: true,
            shade: FillShade::Darken,
        },
        ShapePath {
            commands: vec![
                PathCommand::MoveTo(at(0, 0)),
                PathCommand::LineTo(at(12_700, 12_700)),
            ],
            filled: false,
            stroked: true,
            shade: FillShade::Normal,
        },
        ShapePath {
            commands: vec![PathCommand::MoveTo(at(0, 0))],
            filled: false,
            stroked: false,
            shade: FillShade::Normal,
        },
    ];
    let mut arrow = shape(
        ShapeGeometry::Path(paths.clone()),
        ShapeContent::Text(body(vec![paragraph(vec![run("Avanti")])])),
    );
    arrow.fill = Some(Paint::Solid("4472C4".to_owned()));
    arrow.line = Some(ShapeLine {
        colour: "1F3864".to_owned(),
        width_emu: 12_700,
    });
    let empty = shape(ShapeGeometry::Path(paths), ShapeContent::None);
    let slides = vec![Slide {
        background: None,
        shapes: vec![arrow, empty],
    }];
    let (markup, _) = source(&presentation(slides, Vec::new()), true);
    assert!(markup.contains(
        concat!(
            "block(width: 100pt, height: 50pt)[",
            "#place(top + left, curve(fill: rgb(\"#294476\"), fill-rule: \"even-odd\", stroke: none, curve.move((0pt, 0pt)), curve.line((10pt, 0pt)), curve.cubic((10pt, 1pt), (0pt, 1pt), (0pt, 2pt)), curve.close(mode: \"straight\")))",
            "#place(top + left, curve(fill: none, fill-rule: \"even-odd\", stroke: 1pt + rgb(\"#1F3864\"), curve.move((0pt, 0pt)), curve.line((10pt, 0pt)), curve.cubic((10pt, 1pt), (0pt, 1pt), (0pt, 2pt)), curve.close(mode: \"straight\")))",
            "#place(top + left, curve(fill: none, fill-rule: \"even-odd\", stroke: 1pt + rgb(\"#1F3864\"), curve.move((0pt, 0pt)), curve.line((1pt, 1pt))))",
            "#block(width: 100%, height: 100%, inset: "
        )
    ));
    assert!(markup.contains("Avanti"));
    assert!(markup.contains("#place(top + left, curve(fill: none, fill-rule: \"even-odd\", stroke: none, curve.move((0pt, 0pt)), curve.line((10pt, 0pt))"));
    assert!(markup.contains(", inset: 0pt, [])]"));
}

#[test]
fn fills_shapes_and_pages_with_gradients_and_pictures() {
    let stop = |offset: f64, colour: &str| GradientStop {
        offset,
        colour: colour.to_owned(),
    };
    let linear = Paint::Gradient(Gradient {
        stops: vec![stop(1.0, "0000FF"), stop(0.0, "FF0000")],
        kind: GradientKind::Linear { angle: 45.0 },
    });
    let radial = Paint::Gradient(Gradient {
        stops: vec![stop(0.0, "FFFFFF"), stop(1.0, "000000")],
        kind: GradientKind::Radial {
            centre: (0.25, 0.5),
        },
    });
    let mut shapes = Vec::new();
    for paint in [
        radial,
        Paint::Picture {
            media: 0,
            tile_emu: None,
        },
        Paint::Picture {
            media: 0,
            tile_emu: Some((25_400, 12_700)),
        },
        Paint::Picture {
            media: 1,
            tile_emu: None,
        },
        Paint::Gradient(Gradient {
            stops: vec![stop(0.5, "123456")],
            kind: GradientKind::Linear { angle: 0.0 },
        }),
        Paint::Gradient(Gradient {
            stops: Vec::new(),
            kind: GradientKind::Linear { angle: 0.0 },
        }),
    ] {
        let mut filled = shape(ShapeGeometry::Rectangle, ShapeContent::None);
        filled.frame = frame(0, 0, 127_000, 63_500);
        filled.fill = Some(paint);
        shapes.push(filled);
    }
    let slides = vec![Slide {
        background: Some(linear),
        shapes,
    }];
    let media = vec![
        Media {
            content_type: "image/png",
            bytes: b"\x89PNG\r\n\x1a\nimmagine".to_vec(),
        },
        Media {
            content_type: "image/emf",
            bytes: vec![1, 2, 3],
        },
    ];
    let (markup, files) = source(&presentation(slides, media.clone()), true);
    assert!(markup.contains(
        "#page(fill: gradient.linear((rgb(\"#FF0000\"), 0%), (rgb(\"#0000FF\"), 100%), angle: 45deg))"
    ));
    assert!(markup.contains(
        "fill: gradient.radial((rgb(\"#FFFFFF\"), 0%), (rgb(\"#000000\"), 100%), center: (25%, 50%), radius: 90.14%)"
    ));
    assert!(markup.contains(
        "fill: tiling(size: (10pt, 5pt))[#image(\"/media/slides/0.png\", width: 10pt, height: 5pt, fit: \"stretch\")]"
    ));
    assert!(markup.contains(
        "fill: tiling(size: (2pt, 1pt))[#image(\"/media/slides/0.png\", width: 2pt, height: 1pt, fit: \"stretch\")]"
    ));
    assert!(
        markup.contains("fill: luma(230)"),
        "an image Typst cannot draw"
    );
    assert!(
        markup.contains("fill: rgb(\"#123456\")"),
        "one stop is a colour"
    );
    assert!(markup.contains("fill: none"), "no stop is no fill");
    assert_eq!(files.len(), 1);
    let (without, _) = source(
        &presentation(
            vec![Slide {
                background: Some(Paint::Picture {
                    media: 0,
                    tile_emu: None,
                }),
                shapes: Vec::new(),
            }],
            media,
        ),
        false,
    );
    assert!(
        without.contains("#page(fill: luma(230))"),
        "images left out"
    );
}
