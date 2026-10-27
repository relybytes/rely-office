// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use std::io::Read;

use super::*;
use crate::document::Media;
use crate::slides::{SlideTableCell, SlideTableRow};
use crate::{Limits, read_presentation};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nimmagine";

fn frame(x: i64, y: i64) -> Frame {
    Frame {
        x,
        y,
        width: 1_270_000,
        height: 635_000,
    }
}

fn run(text: &str, line_break: bool) -> SlideRun {
    SlideRun {
        text: text.to_owned(),
        size: 2_400,
        bold: true,
        italic: false,
        underline: true,
        strike: true,
        colour: "C00000".to_owned(),
        family: "Arial".to_owned(),
        line_break,
    }
}

fn body(paragraphs: Vec<SlideParagraph>) -> TextBody {
    TextBody {
        paragraphs,
        anchor: TextAnchor::Middle,
        insets_emu: [91_440, 45_720, 91_440, 45_720],
        wrap: false,
    }
}

fn paragraph(runs: Vec<SlideRun>, bullet: Option<Bullet>, align: Alignment) -> SlideParagraph {
    SlideParagraph {
        level: 1,
        align,
        bullet,
        margin_left_emu: 342_900,
        indent_emu: -228_600,
        line_spacing_percent: 90,
        space_before: 600,
        space_after: 0,
        runs,
    }
}

fn shape(geometry: ShapeGeometry, content: ShapeContent, at: Frame) -> SlideShape {
    SlideShape {
        frame: at,
        rotation: 0.0,
        geometry,
        fill: Some("4472C4".to_owned()),
        line: Some(ShapeLine {
            colour: "1F3864".to_owned(),
            width_emu: 12_700,
        }),
        content,
    }
}

fn cell(text: &str, col_span: u32, covered: bool) -> SlideTableCell {
    SlideTableCell {
        body: body(vec![paragraph(
            vec![run(text, false)],
            None,
            Alignment::Left,
        )]),
        fill: Some("FFFF00".to_owned()),
        col_span,
        row_span: 1,
        covered,
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "one fixture that covers every kind of content"
)]
fn presentation() -> Presentation {
    let at = |x: i64, y: i64| PathPoint { x, y };
    let mut text_shape = shape(
        ShapeGeometry::Rectangle,
        ShapeContent::Text(body(vec![
            paragraph(
                vec![run("Primo", false), run("", true), run("a & <b>", false)],
                Some(Bullet::Character("\u{2022}".to_owned())),
                Alignment::Center,
            ),
            paragraph(
                vec![run("Secondo", false)],
                Some(Bullet::Number("1.".to_owned())),
                Alignment::Right,
            ),
            paragraph(Vec::new(), None, Alignment::Justify),
        ])),
        frame(0, 0),
    );
    text_shape.rotation = -90.0;
    let mut line = shape(
        ShapeGeometry::Line { flipped: true },
        ShapeContent::None,
        frame(0, 700_000),
    );
    line.fill = None;
    let mut bare_line = shape(
        ShapeGeometry::Line { flipped: false },
        ShapeContent::None,
        frame(0, 800_000),
    );
    bare_line.line = None;
    let path = ShapePath {
        commands: vec![
            PathCommand::MoveTo(at(0, 0)),
            PathCommand::LineTo(at(1_270_000, 0)),
            PathCommand::CubicTo(at(1, 2), at(3, 4), at(0, 635_000)),
            PathCommand::Close,
        ],
        filled: true,
        stroked: true,
        shade: FillShade::Darken,
    };
    let open = ShapePath {
        commands: vec![
            PathCommand::MoveTo(at(0, 0)),
            PathCommand::LineTo(at(10, 10)),
        ],
        filled: false,
        stroked: false,
        shade: FillShade::Normal,
    };
    let mut unfilled = shape(
        ShapeGeometry::Ellipse,
        ShapeContent::None,
        frame(2_000_000, 0),
    );
    unfilled.fill = None;
    unfilled.line = None;
    Presentation {
        width_emu: 12_192_000,
        height_emu: 6_858_000,
        slides: vec![
            Slide {
                background: Some("F0F0F0".to_owned()),
                shapes: vec![
                    text_shape,
                    line,
                    bare_line,
                    shape(
                        ShapeGeometry::Path(vec![path, open]),
                        ShapeContent::None,
                        frame(0, 1_000_000),
                    ),
                    shape(
                        ShapeGeometry::RoundedRectangle,
                        ShapeContent::None,
                        frame(1_500_000, 0),
                    ),
                    unfilled,
                    shape(
                        ShapeGeometry::Other,
                        ShapeContent::None,
                        frame(3_000_000, 0),
                    ),
                ],
            },
            Slide {
                background: None,
                shapes: vec![
                    shape(
                        ShapeGeometry::Rectangle,
                        ShapeContent::Picture { media: 0 },
                        frame(0, 0),
                    ),
                    shape(
                        ShapeGeometry::Rectangle,
                        ShapeContent::Picture { media: 0 },
                        frame(0, 700_000),
                    ),
                    shape(
                        ShapeGeometry::Rectangle,
                        ShapeContent::Picture { media: 1 },
                        frame(0, 1_400_000),
                    ),
                    shape(
                        ShapeGeometry::Rectangle,
                        ShapeContent::Table(SlideTable {
                            column_widths_emu: vec![635_000, 635_000],
                            rows: vec![
                                SlideTableRow {
                                    height_emu: 317_500,
                                    cells: vec![cell("Unita", 2, false), cell("", 1, true)],
                                },
                                SlideTableRow {
                                    height_emu: 317_500,
                                    cells: vec![
                                        SlideTableCell {
                                            row_span: 2,
                                            ..cell("A", 1, false)
                                        },
                                        cell("B", 1, false),
                                    ],
                                },
                            ],
                        }),
                        frame(2_000_000, 0),
                    ),
                ],
            },
        ],
        media: vec![
            Media {
                content_type: "image/png",
                bytes: PNG.to_vec(),
            },
            Media {
                content_type: "image/emf",
                bytes: vec![1, 2, 3],
            },
        ],
        truncated: false,
    }
}

fn member(bytes: &[u8], name: &str) -> String {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut file = archive.by_name(name).unwrap();
    let mut text = String::new();
    file.read_to_string(&mut text).unwrap();
    text
}

#[test]
fn writes_slides_that_read_back_as_they_were() {
    let written = write_pptx(&presentation()).unwrap();
    let read = read_presentation(&written, &Limits::default()).unwrap();
    assert_eq!((read.width_emu, read.height_emu), (12_192_000, 6_858_000));
    assert_eq!(read.slides.len(), 2);
    let first = &read.slides[0];
    assert_eq!(first.background.as_deref(), Some("F0F0F0"));
    assert_eq!(
        first.shapes.len(),
        6,
        "the shape with neither fill nor line is left out"
    );
    let text = &first.shapes[0];
    assert!((text.rotation - 270.0).abs() < f64::EPSILON);
    let ShapeContent::Text(body) = &text.content else {
        panic!("text");
    };
    assert_eq!(body.anchor, TextAnchor::Middle);
    assert!(!body.wrap);
    let paragraph = &body.paragraphs[0];
    assert_eq!(paragraph.align, Alignment::Center);
    assert_eq!(
        paragraph.bullet,
        Some(Bullet::Character("\u{2022}".to_owned()))
    );
    assert_eq!(
        (
            paragraph.margin_left_emu,
            paragraph.indent_emu,
            paragraph.level
        ),
        (342_900, -228_600, 1)
    );
    assert_eq!(paragraph.runs.len(), 3);
    assert!(paragraph.runs[1].line_break);
    let first_run = &paragraph.runs[0];
    assert_eq!(
        (
            first_run.text.as_str(),
            first_run.size,
            first_run.bold,
            first_run.underline,
            first_run.strike
        ),
        ("Primo", 2_400, true, true, true)
    );
    assert_eq!(
        (first_run.colour.as_str(), first_run.family.as_str()),
        ("C00000", "Arial")
    );
    assert_eq!(paragraph.runs[2].text, "a & <b>");
    assert_eq!(body.paragraphs[1].align, Alignment::Right);
    assert!(matches!(
        first.shapes[1].geometry,
        ShapeGeometry::Line { flipped: true }
    ));
    let ShapeGeometry::Path(paths) = &first.shapes[3].geometry else {
        panic!("a path");
    };
    assert_eq!(paths.len(), 2);
    assert_eq!(
        paths[0].commands[1],
        PathCommand::LineTo(PathPoint { x: 1_270_000, y: 0 })
    );
    assert!(!paths[1].filled && !paths[1].stroked);
    assert_eq!(
        paths[0].shade,
        FillShade::Darken,
        "a shaded face keeps its shade"
    );
    assert!(
        matches!(first.shapes[4].geometry, ShapeGeometry::Path(_)),
        "a rounded rectangle's preset"
    );
    let second = &read.slides[1];
    assert!(matches!(
        second.shapes[0].content,
        ShapeContent::Picture { .. }
    ));
    assert_eq!(read.media.len(), 1, "one image, shown twice");
    assert_eq!(
        second.shapes[2].content,
        ShapeContent::None,
        "an image PowerPoint cannot show"
    );
    let ShapeContent::Table(table) = &second.shapes[3].content else {
        panic!("a table");
    };
    assert_eq!(table.column_widths_emu, vec![635_000, 635_000]);
    assert_eq!(table.rows[0].cells[0].col_span, 2);
    assert!(table.rows[0].cells[1].covered);
    assert_eq!(table.rows[1].cells[0].row_span, 2);
    assert_eq!(table.rows[1].cells[1].fill.as_deref(), Some("FFFF00"));
}

#[test]
fn writes_the_parts_a_pptx_needs() {
    let written = write_pptx(&presentation()).unwrap();
    let types = member(&written, "[Content_Types].xml");
    assert!(types.contains(r#"<Override PartName="/ppt/slides/slide2.xml""#));
    let presentation_xml = member(&written, "ppt/presentation.xml");
    assert!(presentation_xml.contains(r#"<p:sldSz cx="12192000" cy="6858000"/>"#));
    assert!(presentation_xml.contains(r#"<p:sldId id="257" r:id="rId4"/>"#));
    let slide = member(&written, "ppt/slides/slide1.xml");
    assert!(slide.contains("<a:buAutoNum"));
    assert!(slide.contains(r#"fill="darken""#));
    assert!(slide.contains("<a:buNone/>"));
    assert!(slide.contains(r#"algn="just""#));
    assert!(slide.contains("<a:p><a:pPr"));
    assert!(slide.contains("a &amp; &lt;b&gt;"));
    let relationships = member(&written, "ppt/slides/_rels/slide2.xml.rels");
    assert_eq!(relationships.matches("media/image1.png").count(), 2);
    for part in [
        "_rels/.rels",
        "ppt/_rels/presentation.xml.rels",
        "ppt/slideMasters/slideMaster1.xml",
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        "ppt/slideLayouts/slideLayout1.xml",
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        "ppt/theme/theme1.xml",
    ] {
        assert!(!member(&written, part).is_empty(), "{part}");
    }
}

#[test]
fn writes_one_blank_slide_for_an_empty_presentation_within_the_sizes_powerpoint_opens() {
    let empty = Presentation {
        width_emu: 10,
        height_emu: i64::MAX,
        slides: Vec::new(),
        media: Vec::new(),
        truncated: false,
    };
    let written = write_pptx(&empty).unwrap();
    let presentation_xml = member(&written, "ppt/presentation.xml");
    assert!(presentation_xml.contains(r#"<p:sldSz cx="914400" cy="51206400"/>"#));
    let read = read_presentation(&written, &Limits::default()).unwrap();
    assert_eq!(read.slides.len(), 1);
    let blank = member(&written, "ppt/slides/slide1.xml");
    assert!(!blank.contains("<p:bg>"));
    let mut xml = String::new();
    text_body(&body(Vec::new()), &mut xml);
    assert!(xml.ends_with("<a:p/>"));
}
