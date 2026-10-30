// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use super::*;
use crate::document::Media;
use crate::geometry::{PathPoint, ShapePath};
use crate::slides::{Gradient, GradientStop, SlideRun, SlideTableCell, SlideTableRow, TextAnchor};
use crate::{DocumentFormat, Limits, detect_format, read_presentation};

const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0,
    0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

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
        italic: true,
        underline: true,
        strike: false,
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

fn paragraph(
    runs: Vec<SlideRun>,
    bullet: Option<Bullet>,
    level: u8,
    align: Alignment,
) -> SlideParagraph {
    SlideParagraph {
        level,
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
        id: None,
        inherited: false,
        frame: at,
        rotation: 0.0,
        geometry,
        fill: Some(Paint::Solid("4472C4".to_owned())),
        line: Some(ShapeLine {
            colour: "1F3864".to_owned(),
            width_emu: 12_700,
        }),
        content,
    }
}

#[expect(clippy::too_many_lines, reason = "one deck with every kind of content")]
fn deck() -> Presentation {
    let at = |x: i64, y: i64| PathPoint { x, y };
    let mut text = shape(
        ShapeGeometry::Rectangle,
        ShapeContent::Text(body(vec![
            paragraph(
                vec![run("Primo", false), run("", true), run("a & <b>", false)],
                Some(Bullet::Character("\u{2022}".to_owned())),
                0,
                Alignment::Center,
            ),
            paragraph(
                vec![run("Dentro", false)],
                Some(Bullet::Character("\u{2022}".to_owned())),
                1,
                Alignment::Left,
            ),
            paragraph(vec![run("Senza", false)], None, 0, Alignment::Right),
        ])),
        frame(100_000, 200_000),
    );
    text.rotation = 90.0;
    let numbered = shape(
        ShapeGeometry::Ellipse,
        ShapeContent::Text(body(vec![
            paragraph(
                vec![run("Uno", false)],
                Some(Bullet::Number("1.".to_owned())),
                0,
                Alignment::Left,
            ),
            paragraph(
                vec![run("Due", false)],
                Some(Bullet::Number("2.".to_owned())),
                0,
                Alignment::Left,
            ),
        ])),
        frame(2_000_000, 0),
    );
    let mut line = shape(
        ShapeGeometry::Line { flipped: true },
        ShapeContent::None,
        frame(0, 700_000),
    );
    line.fill = None;
    let path = shape(
        ShapeGeometry::Path(vec![ShapePath {
            commands: vec![
                PathCommand::MoveTo(at(0, 0)),
                PathCommand::LineTo(at(1_270_000, 0)),
                PathCommand::CubicTo(at(1, 2), at(3, 4), at(0, 635_000)),
                PathCommand::Close,
            ],
            filled: true,
            stroked: true,
            shade: FillShade::Normal,
        }]),
        ShapeContent::None,
        frame(0, 1_000_000),
    );
    let mut gradient = shape(
        ShapeGeometry::RoundedRectangle,
        ShapeContent::None,
        frame(3_000_000, 0),
    );
    gradient.fill = Some(Paint::Gradient(Gradient {
        stops: vec![
            GradientStop {
                offset: 0.0,
                colour: "FF0000".to_owned(),
            },
            GradientStop {
                offset: 0.0,
                colour: "FF0000".to_owned(),
            },
            GradientStop {
                offset: 1.0,
                colour: "0000FF".to_owned(),
            },
        ],
        kind: GradientKind::Linear { angle: 0.0 },
    }));
    let cell = |text: &str, col_span: u32, row_span: u32, covered: bool| SlideTableCell {
        body: body(vec![paragraph(
            vec![run(text, false)],
            None,
            0,
            Alignment::Left,
        )]),
        fill: Some("FFFF00".to_owned()),
        col_span,
        row_span,
        covered,
    };
    let table = shape(
        ShapeGeometry::Rectangle,
        ShapeContent::Table(SlideTable {
            column_widths_emu: vec![635_000, 635_000],
            rows: vec![
                SlideTableRow {
                    height_emu: 317_500,
                    cells: vec![cell("Unita", 2, 1, false), cell("", 1, 1, true)],
                },
                SlideTableRow {
                    height_emu: 317_500,
                    cells: vec![cell("A", 1, 1, false), cell("B", 1, 1, false)],
                },
            ],
        }),
        frame(2_000_000, 2_000_000),
    );
    let mut picture = shape(
        ShapeGeometry::Rectangle,
        ShapeContent::Picture { media: 0 },
        frame(0, 2_000_000),
    );
    picture.fill = None;
    picture.line = None;
    Presentation {
        width_emu: 12_192_000,
        height_emu: 6_858_000,
        slides: vec![
            Slide {
                background: Some(Paint::Solid("F0F0F0".to_owned())),
                shapes: vec![text, numbered, line, path, gradient],
                notes: Some(body(vec![paragraph(
                    vec![run("Ricordare", false)],
                    None,
                    0,
                    Alignment::Left,
                )])),
                hidden: false,
                transition: Some(Transition {
                    effect: "push".to_owned(),
                    direction: Some("u".to_owned()),
                    duration_ms: Some(700),
                    advance_on_click: true,
                    advance_after_ms: Some(3_000),
                }),
                animations: vec![
                    Animation {
                        shape: 0,
                        class: AnimationClass::Entrance,
                        preset: 10,
                        preset_subtype: 0,
                        trigger: AnimationTrigger::OnClick,
                        delay_ms: 0,
                        duration_ms: 500,
                        motion_path: None,
                    },
                    Animation {
                        shape: 1,
                        class: AnimationClass::Exit,
                        preset: 2,
                        preset_subtype: 8,
                        trigger: AnimationTrigger::AfterPrevious,
                        delay_ms: 250,
                        duration_ms: 1_000,
                        motion_path: None,
                    },
                ],
            },
            Slide {
                background: Some(Paint::Picture {
                    media: 0,
                    tile_emu: None,
                }),
                shapes: vec![table, picture],
                hidden: true,
                ..Slide::default()
            },
        ],
        media: vec![Media {
            content_type: "image/png",
            bytes: PNG.to_vec(),
        }],
        truncated: false,
        ..Presentation::default()
    }
}

#[test]
#[expect(clippy::too_many_lines, reason = "one check per kind of content")]
fn writes_an_odp_that_reads_back_as_it_was() {
    let original = deck();
    let written = write_odp(&original).unwrap();
    assert_eq!(detect_format(&written, None), DocumentFormat::Odp);
    let read = read_presentation(&written, &Limits::default()).unwrap();
    assert_eq!((read.width_emu, read.height_emu), (12_192_000, 6_858_000));
    assert_eq!(read.slides.len(), 2);

    let first = &read.slides[0];
    assert_eq!(
        first.background.as_ref().and_then(Paint::colour),
        Some("F0F0F0")
    );
    assert_eq!(first.shapes.len(), 5);
    let text = &first.shapes[0];
    assert!((text.rotation - 90.0).abs() < 1e-3, "{}", text.rotation);
    assert!(
        (text.frame.x - 100_000).abs() <= 2 && (text.frame.y - 200_000).abs() <= 2,
        "{:?}",
        text.frame
    );
    let ShapeContent::Text(body) = &text.content else {
        panic!("{:?}", text.content);
    };
    assert_eq!(body.anchor, TextAnchor::Middle);
    assert!(!body.wrap);
    assert_eq!(body.insets_emu, [91_440, 45_720, 91_440, 45_720]);
    let first_paragraph = &body.paragraphs[0];
    assert_eq!(first_paragraph.align, Alignment::Center);
    assert_eq!(
        first_paragraph.bullet,
        Some(Bullet::Character("\u{2022}".to_owned()))
    );
    assert_eq!(
        (
            first_paragraph.margin_left_emu,
            first_paragraph.indent_emu,
            first_paragraph.line_spacing_percent,
            first_paragraph.space_before
        ),
        (342_900, -228_600, 90, 600)
    );
    assert_eq!(first_paragraph.runs.len(), 3);
    assert!(first_paragraph.runs[1].line_break);
    let run = &first_paragraph.runs[0];
    assert_eq!(
        (
            run.text.as_str(),
            run.size,
            run.bold,
            run.italic,
            run.underline,
            run.colour.as_str(),
            run.family.as_str()
        ),
        ("Primo", 2_400, true, true, true, "C00000", "Arial")
    );
    assert_eq!(first_paragraph.runs[2].text, "a & <b>");
    assert_eq!(body.paragraphs[1].level, 1);
    assert_eq!(body.paragraphs[2].bullet, None);
    assert_eq!(body.paragraphs[2].align, Alignment::Right);

    let ShapeContent::Text(numbered) = &first.shapes[1].content else {
        panic!("{:?}", first.shapes[1].content);
    };
    assert_eq!(first.shapes[1].geometry, ShapeGeometry::Ellipse);
    assert_eq!(
        numbered.paragraphs[1].bullet,
        Some(Bullet::Number("2.".to_owned()))
    );
    assert_eq!(
        first.shapes[2].geometry,
        ShapeGeometry::Line { flipped: true }
    );
    assert_eq!(
        first.shapes[2].line,
        Some(ShapeLine {
            colour: "1F3864".to_owned(),
            width_emu: 12_700
        })
    );
    let ShapeGeometry::Path(paths) = &first.shapes[3].geometry else {
        panic!("{:?}", first.shapes[3].geometry);
    };
    assert_eq!(paths[0].commands.len(), 4);
    assert!(matches!(first.shapes[4].fill, Some(Paint::Gradient(_))));
    assert_eq!(
        first.shapes[4].fill.as_ref().and_then(Paint::colour),
        Some("FF0000")
    );

    let notes = first.notes.as_ref().unwrap();
    assert_eq!(notes.paragraphs[0].runs[0].text, "Ricordare");
    let transition = first.transition.as_ref().unwrap();
    assert_eq!(
        (
            transition.effect.as_str(),
            transition.direction.as_deref(),
            transition.duration_ms,
            transition.advance_on_click,
            transition.advance_after_ms
        ),
        ("push", Some("u"), Some(700), true, Some(3_000))
    );
    assert_eq!(first.animations.len(), 2);
    assert_eq!(
        (
            first.animations[0].shape,
            first.animations[0].class,
            first.animations[0].preset,
            first.animations[0].trigger
        ),
        (0, AnimationClass::Entrance, 10, AnimationTrigger::OnClick)
    );
    assert_eq!(
        (
            first.animations[1].shape,
            first.animations[1].preset,
            first.animations[1].preset_subtype,
            first.animations[1].delay_ms,
            first.animations[1].duration_ms
        ),
        (1, 2, 8, 250, 1_000)
    );

    let second = &read.slides[1];
    assert!(second.hidden);
    assert!(matches!(
        second.background,
        Some(Paint::Picture { media: 0, .. })
    ));
    let ShapeContent::Table(table) = &second.shapes[0].content else {
        panic!("{:?}", second.shapes[0].content);
    };
    assert_eq!(table.column_widths_emu, [635_000, 635_000]);
    assert_eq!(table.rows[0].cells[0].col_span, 2);
    assert!(table.rows[0].cells[1].covered);
    assert_eq!(table.rows[0].cells[0].fill.as_deref(), Some("FFFF00"));
    assert_eq!(table.rows[1].height_emu, 317_500);
    assert!(matches!(
        second.shapes[1].content,
        ShapeContent::Picture { media: 0 }
    ));
    assert_eq!(read.media[0].bytes, PNG);

    let again = read_presentation(&write_odp(&read).unwrap(), &Limits::default()).unwrap();
    assert_eq!(again.slides, read.slides);
}

#[test]
fn writes_lengths_and_labels_as_opendocument_spells_them() {
    assert_eq!(emu_length(12_700), "1pt");
    assert_eq!(emu_length(-6_350), "-0.5pt");
    assert_eq!(hex("#abcdef"), "ABCDEF");
    assert_eq!(hex("blu"), "000000");
    assert_eq!(
        bullet_kind(&Bullet::Number("(a)".to_owned())),
        "number|(|a|)"
    );
    assert_eq!(
        bullet_kind(&Bullet::Number("IV.".to_owned())),
        "number||I|."
    );
    assert_eq!(
        text_xml("a  b\tc"),
        r#"a <text:s text:c="1"/>b<text:tab/>c"#
    );
    assert!(placement(&frame(0, 0), 0.0).contains(r#"svg:x="0pt""#));
    assert!(placement(&frame(0, 0), 45.0).contains("draw:transform"));
}
