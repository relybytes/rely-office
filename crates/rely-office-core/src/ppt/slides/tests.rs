// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;
use crate::ppt::test_ppt::{
    PptBuilder, array, atom, container, exception, property_table, shape, text_chars,
};
use crate::read_presentation;
use crate::slides::{Gradient, GradientKind, GradientStop, Paint};
use crate::test_support::{build_compound_file, ppt_record};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nimmagine";
const MASTER_ID: u32 = 0x8000_0001;

fn font_entity(name: &str) -> Vec<u8> {
    let mut body = vec![0_u8; 68];
    for (index, unit) in name.encode_utf16().enumerate() {
        body[index * 2..index * 2 + 2].copy_from_slice(&unit.to_le_bytes());
    }
    atom(FONT_ENTITY, 0, &body)
}

fn slide_persist(persist_id: u32, slide_id: u32) -> Vec<u8> {
    let mut body = vec![0_u8; 20];
    body[0..4].copy_from_slice(&persist_id.to_le_bytes());
    body[12..16].copy_from_slice(&slide_id.to_le_bytes());
    atom(SLIDE_PERSIST, 0, &body)
}

fn text_header(kind: u32) -> Vec<u8> {
    atom(TEXT_HEADER, 0, &kind.to_le_bytes())
}

/// A master style of one level: a paragraph exception and a character exception of a size.
fn master_style(instance: u16, paragraph: Vec<u8>, size: u16) -> Vec<u8> {
    let mut body = 1_u16.to_le_bytes().to_vec();
    if instance >= 5 {
        body.extend_from_slice(&0_u16.to_le_bytes());
    }
    body.extend(paragraph);
    body.extend(exception(0x0002_0000, &size.to_le_bytes()));
    atom(MASTER_STYLE, instance, &body)
}

fn scheme(colours: [[u8; 3]; 8]) -> Vec<u8> {
    let body: Vec<u8> = colours
        .iter()
        .flat_map(|colour| [colour[0], colour[1], colour[2], 0])
        .collect();
    atom(COLOUR_SCHEME, 1, &body)
}

fn drawing(shapes: &[Vec<u8>], background: Option<u32>) -> Vec<u8> {
    let mut group = vec![shape(0, 0x5, None, &[])];
    group.extend_from_slice(shapes);
    let mut children = vec![container(SHAPE_GROUP_CONTAINER, 0, &group)];
    if let Some(colour) = background {
        children.push(shape(
            1,
            0x400,
            None,
            &[property_table(PROPERTIES, &[(0x0181, colour)], &[])],
        ));
    }
    container(DRAWING, 0, &[container(DRAWING_CONTAINER, 0, &children)])
}

/// A rectangle filled with the fill properties given, its fill colour red.
fn filled_shape(fill: &[(u16, u32)]) -> Vec<u8> {
    let mut properties = vec![(0x0181, 0x0000_00FF)];
    properties.extend_from_slice(fill);
    shape(
        1,
        0,
        Some([0, 0, 100, 100]),
        &[property_table(PROPERTIES, &properties, &[])],
    )
}

fn placeholder(text: &[Vec<u8>], anchor: [i16; 4]) -> Vec<u8> {
    shape(
        1,
        0,
        Some(anchor),
        &[
            container(CLIENT_DATA, 0, &[atom(PLACEHOLDER, 0, &[0; 8])]),
            container(CLIENT_TEXTBOX, 0, text),
        ],
    )
}

fn slide_atom(master: u32, flags: u16) -> Vec<u8> {
    let mut body = vec![0_u8; 24];
    body[12..16].copy_from_slice(&master.to_le_bytes());
    body[20..22].copy_from_slice(&flags.to_le_bytes());
    atom(SLIDE_ATOM, 0, &body)
}

/// A presentation of a master and two slides: one of placeholders whose text is in the slide
/// list, one of drawn shapes, a picture, a group and a free-form outline.
#[expect(clippy::too_many_lines, reason = "one presentation every test reads")]
fn presentation() -> PptBuilder {
    let mut document_atom = vec![0_u8; 40];
    document_atom[0..4].copy_from_slice(&5_760_i32.to_le_bytes());
    document_atom[4..8].copy_from_slice(&4_320_i32.to_le_bytes());
    let environment = container(
        ENVIRONMENT,
        0,
        &[
            container(
                FONT_COLLECTION,
                0,
                &[font_entity("Arial"), font_entity("Times New Roman")],
            ),
            atom(
                DEFAULT_CHARACTER,
                0,
                &exception(0x0001_0000, &0_u16.to_le_bytes()),
            ),
            master_style(4, exception(0, &[]), 18),
        ],
    );
    let mut blip_body = vec![0_u8; 17];
    blip_body.extend_from_slice(PNG);
    let blip = ppt_record(0, 0x6E0, 0xF01E, &blip_body);
    let mut entry = vec![0_u8; 36];
    entry[20..24].copy_from_slice(&u32::try_from(blip.len()).unwrap().to_le_bytes());
    entry[28..32].copy_from_slice(&0_u32.to_le_bytes());
    let drawing_group = container(
        DRAWING_GROUP,
        0,
        &[container(
            GROUP_CONTAINER,
            0,
            &[container(
                STORE_CONTAINER,
                1,
                &[ppt_record(2, 6, 0xF007, &entry)],
            )],
        )],
    );
    let mut style = Vec::new();
    style.extend_from_slice(&14_u32.to_le_bytes());
    style.extend_from_slice(&0_u16.to_le_bytes());
    style.extend(exception(0, &[]));
    style.extend_from_slice(&3_u32.to_le_bytes());
    style.extend(exception(0, &[]));
    style.extend_from_slice(&11_u32.to_le_bytes());
    style.extend(exception(0x0004_0001, &[1, 0, 0xC0, 0, 0, 0xFE]));
    let slide_list = container(
        SLIDE_LIST,
        0,
        &[
            slide_persist(2, 256),
            text_header(0),
            atom(TEXT_BYTES, 0, b"Titolo"),
            text_header(1),
            text_chars("Primo\rSecondo"),
            atom(STYLE_TEXT, 0, &style),
            slide_persist(3, 257),
        ],
    );
    let master_list = container(SLIDE_LIST, 1, &[slide_persist(4, MASTER_ID)]);
    let document = container(
        DOCUMENT,
        0,
        &[
            atom(DOCUMENT_ATOM, 0, &document_atom),
            environment,
            drawing_group,
            master_list,
            slide_list,
        ],
    );
    let bullet = exception(0x0000_008F, &[0x01, 0x00, 0x22, 0x20]);
    let master = container(
        MAIN_MASTER,
        0,
        &[
            master_style(0, exception(0x0800, &1_u16.to_le_bytes()), 44),
            master_style(1, bullet, 32),
            scheme([
                [255, 255, 255],
                [0, 0, 0],
                [128, 128, 128],
                [0, 0, 128],
                [200, 200, 255],
                [0, 128, 0],
                [0, 0, 255],
                [255, 0, 0],
            ]),
            drawing(
                &[
                    shape(20, 0, Some([4_000, 0, 5_760, 4_000]), &[]),
                    placeholder(&[text_chars("Fai clic")], [0, 0, 100, 100]),
                ],
                Some(0x0000_F0F0),
            ),
        ],
    );
    let first = container(
        SLIDE,
        0,
        &[
            slide_atom(MASTER_ID, 0x7),
            drawing(
                &[
                    placeholder(
                        &[atom(OUTLINE_TEXT_REFERENCE, 0, &0_u32.to_le_bytes())],
                        [100, 200, 5_560, 800],
                    ),
                    placeholder(
                        &[atom(OUTLINE_TEXT_REFERENCE, 0, &1_u32.to_le_bytes())],
                        [900, 200, 5_560, 3_800],
                    ),
                ],
                None,
            ),
        ],
    );
    let free_form = {
        let points: Vec<Vec<u8>> = [(0_i16, 0_i16), (21_600, 10_800), (0, 21_600)]
            .iter()
            .map(|(x, y)| [x.to_le_bytes(), y.to_le_bytes()].concat())
            .collect();
        let segments: Vec<Vec<u8>> = [0x4000_u16, 0x0002, 0x6001, 0x8000]
            .iter()
            .map(|value| value.to_le_bytes().to_vec())
            .collect();
        shape(
            0,
            0,
            Some([2_000, 3_000, 3_000, 4_000]),
            &[property_table(
                PROPERTIES,
                &[(0x0181, 0x0000_00FF)],
                &[(0x0145, array(4, &points)), (0x0146, array(2, &segments))],
            )],
        )
    };
    let mut child_anchor = Vec::new();
    for value in [0_i32, 0, 1_000, 1_000] {
        child_anchor.extend_from_slice(&value.to_le_bytes());
    }
    let mut group_shape = Vec::new();
    for value in [0_i32, 0, 2_000, 2_000] {
        group_shape.extend_from_slice(&value.to_le_bytes());
    }
    let group = container(
        SHAPE_GROUP_CONTAINER,
        0,
        &[
            shape(
                0,
                0x201,
                Some([3_000, 4_000, 5_000, 4_000]),
                &[atom(GROUP_SHAPE, 0, &group_shape)],
            ),
            shape(3, 0x202, None, &[atom(CHILD_ANCHOR, 0, &child_anchor)]),
        ],
    );
    let second = container(
        SLIDE,
        0,
        &[
            slide_atom(MASTER_ID, 0x0),
            scheme([
                [0, 0, 0],
                [255, 255, 255],
                [0, 0, 0],
                [255, 255, 0],
                [10, 20, 30],
                [0, 0, 0],
                [0, 0, 0],
                [0, 0, 0],
            ]),
            drawing(
                &[
                    shape(
                        13,
                        0x40,
                        Some([100, 100, 1_100, 2_100]),
                        &[
                            property_table(
                                PROPERTIES,
                                &[
                                    (0x0004, 90 << 16),
                                    (0x01C0, 0x0800_0003),
                                    (0x01CB, 25_400),
                                    (0x0087, 1),
                                ],
                                &[],
                            ),
                            container(
                                CLIENT_TEXTBOX,
                                0,
                                &[text_header(4), text_chars("Avanti\u{b}ora")],
                            ),
                        ],
                    ),
                    shape(
                        75,
                        0,
                        Some([100, 2_000, 1_100, 3_000]),
                        &[property_table(PROPERTIES, &[(0x0104, 1)], &[])],
                    ),
                    shape(
                        75,
                        0,
                        Some([100, 3_100, 1_100, 4_100]),
                        &[property_table(PROPERTIES, &[(0x0104, 7)], &[])],
                    ),
                    shape(202, 0x08, Some([0, 0, 10, 10]), &[]),
                    shape(202, 0, Some([0, 0, 10, 10]), &[]),
                    free_form,
                    group,
                    filled_shape(&[(0x0180, 3), (0x0186, 1)]),
                    filled_shape(&[(0x0180, 2), (0x0186, 1)]),
                    filled_shape(&[(0x0180, 3), (0x0186, 9)]),
                    filled_shape(&[
                        (0x0180, 4),
                        (0x0183, 0x0000_FF00),
                        (0x018B, 90 << 16),
                        (0x018C, 50),
                    ]),
                    filled_shape(&[(0x0180, 5), (0x018C, 100)]),
                    filled_shape(&[(0x0180, 7), (0x0183, 0x0800_0004)]),
                ],
                Some(0x0800_0004),
            ),
        ],
    );
    let mut pictures = Vec::new();
    pictures.extend_from_slice(&blip);
    PptBuilder {
        prefix: vec![0; 16],
        document: 1,
        pictures,
        ..PptBuilder::default()
    }
    .object(1, document)
    .object(2, first)
    .object(3, second)
    .object(4, master)
}

fn texts(shape: &SlideShape) -> Vec<String> {
    match &shape.content {
        ShapeContent::Text(body) => body
            .paragraphs
            .iter()
            .map(|paragraph| {
                paragraph
                    .runs
                    .iter()
                    .map(|run| {
                        if run.line_break {
                            "|".to_owned()
                        } else {
                            run.text.clone()
                        }
                    })
                    .collect()
            })
            .collect(),
        other => vec![format!("{other:?}")],
    }
}

#[test]
fn reads_the_slides_over_their_master() {
    let presentation = read_ppt_presentation(&presentation().build(), &Limits::default()).unwrap();
    assert_eq!(
        (presentation.width_emu, presentation.height_emu),
        (9_144_000, 6_858_000)
    );
    assert_eq!(presentation.slides.len(), 2);
    let first = &presentation.slides[0];
    assert_eq!(
        first.background.as_ref().and_then(Paint::colour),
        Some("F0F000"),
        "the master's background"
    );
    assert_eq!(
        first.shapes.len(),
        3,
        "the master's line, then the two placeholders"
    );
    assert!(matches!(
        first.shapes[0].geometry,
        ShapeGeometry::Line { flipped: false }
    ));
    let title = &first.shapes[1];
    assert_eq!(texts(title), vec!["Titolo"]);
    let ShapeContent::Text(body) = &title.content else {
        panic!("text");
    };
    assert_eq!(body.anchor, TextAnchor::Middle);
    assert_eq!(body.paragraphs[0].align, Alignment::Center);
    assert_eq!(body.paragraphs[0].runs[0].size, 4_400);
    assert_eq!(body.paragraphs[0].runs[0].family, "Arial");
    assert_eq!(
        body.paragraphs[0].runs[0].colour, "000000",
        "the scheme's text colour"
    );
    assert_eq!(
        title.frame,
        Frame {
            x: 317_500,
            y: 158_750,
            width: 8_509_000,
            height: 1_111_250
        }
    );
    let body_shape = &first.shapes[2];
    assert_eq!(texts(body_shape), vec!["Primo", "Secondo"]);
    let ShapeContent::Text(body) = &body_shape.content else {
        panic!("text");
    };
    let first_paragraph = &body.paragraphs[0];
    assert_eq!(
        first_paragraph.bullet,
        Some(Bullet::Character("\u{2022}".to_owned()))
    );
    assert_eq!(first_paragraph.runs[0].size, 3_200);
    assert_eq!(first_paragraph.runs.len(), 2);
    assert!(first_paragraph.runs[1].bold, "the second run of the style");
    assert_eq!(first_paragraph.runs[1].colour, "C00000");
    assert_eq!(body.anchor, TextAnchor::Top);
}

#[test]
fn draws_shapes_pictures_groups_and_outlines() {
    let presentation = read_ppt_presentation(&presentation().build(), &Limits::default()).unwrap();
    let second = &presentation.slides[1];
    assert_eq!(
        second.background.as_ref().and_then(Paint::colour),
        Some("0A141E"),
        "its own scheme's fill colour"
    );
    assert_eq!(
        second.shapes.len(),
        10,
        "no master shapes, no deleted shape, no empty text box, no missing picture"
    );
    let arrow = &second.shapes[0];
    assert!(matches!(arrow.geometry, ShapeGeometry::Path(_)));
    assert!((arrow.rotation - 90.0).abs() < f64::EPSILON);
    assert_eq!(
        arrow
            .line
            .as_ref()
            .map(|line| (line.colour.as_str(), line.width_emu)),
        Some(("FFFF00", 25_400))
    );
    assert_eq!(
        arrow.fill.as_ref().and_then(Paint::colour),
        Some("0A141E"),
        "a shape without a fill takes the scheme's"
    );
    assert_eq!(texts(arrow), vec!["Avanti|ora"]);
    let ShapeContent::Text(body) = &arrow.content else {
        panic!("text");
    };
    assert_eq!(body.anchor, TextAnchor::Middle);
    assert_eq!(
        body.paragraphs[0].runs[0].size, 1_800,
        "the environment's other-text style"
    );
    assert_eq!(body.paragraphs[0].runs[0].colour, "FFFFFF");
    assert!(matches!(
        second.shapes[1].content,
        ShapeContent::Picture { media: 0 }
    ));
    assert_eq!(presentation.media.len(), 1);
    assert_eq!(presentation.media[0].content_type, "image/png");
    let ShapeGeometry::Path(paths) = &second.shapes[2].geometry else {
        panic!("a free-form outline");
    };
    assert_eq!(paths[0].commands.len(), 4);
    assert_eq!(
        second.shapes[2].fill.as_ref().and_then(Paint::colour),
        Some("FF0000")
    );
    let child = &second.shapes[3];
    assert_eq!(child.geometry, ShapeGeometry::Ellipse);
    assert_eq!(
        child.frame,
        Frame {
            x: 6_350_000,
            y: 4_762_500,
            width: 793_750,
            height: 793_750
        }
    );
}

#[test]
fn reads_slides_in_stream_order_without_a_current_user() {
    let mut builder = presentation();
    builder.without_current_user = true;
    let presentation = read_ppt_presentation(&builder.build(), &Limits::default()).unwrap();
    assert_eq!(presentation.slides.len(), 2);
    assert_eq!(
        presentation.slides[1].shapes.len(),
        10,
        "the slide that needs neither master nor list"
    );
}

#[test]
fn keeps_within_the_limits() {
    let limits = Limits {
        max_text_chars: 3,
        max_images: 0,
        ..Limits::default()
    };
    let presentation = read_ppt_presentation(&presentation().build(), &limits).unwrap();
    assert!(presentation.truncated);
    assert_eq!(presentation.media, Vec::new());
    assert_eq!(texts(&presentation.slides[0].shapes[1]), vec!["Tit"]);
}

#[test]
fn is_what_a_presentation_reads_for_a_ppt() {
    let presentation = read_presentation(&presentation().build(), &Limits::default()).unwrap();
    assert_eq!(presentation.slides.len(), 2);
}

#[test]
fn refuses_what_it_cannot_read() {
    let mut encrypted = presentation();
    encrypted.encrypted_token = true;
    assert!(matches!(
        read_ppt_presentation(&encrypted.build(), &Limits::default()),
        Err(OfficeError::Encrypted)
    ));
    let summary = build_compound_file(&[
        ("/PowerPoint Document", b"x".as_slice()),
        ("/EncryptedSummary", b"x".as_slice()),
    ]);
    assert!(matches!(
        read_ppt_presentation(&summary, &Limits::default()),
        Err(OfficeError::Encrypted)
    ));
    let missing = build_compound_file(&[("/Altro", b"x".as_slice())]);
    assert!(matches!(
        read_ppt_presentation(&missing, &Limits::default()),
        Err(OfficeError::UnreadableDocument(_))
    ));
    let no_document = PptBuilder::default()
        .object(1, container(SLIDE, 0, &[]))
        .build();
    assert!(matches!(
        read_ppt_presentation(&no_document, &Limits::default()),
        Err(OfficeError::UnreadableDocument(_))
    ));
    let limits = Limits {
        max_input_bytes: 10,
        ..Limits::default()
    };
    assert!(matches!(
        read_ppt_presentation(&presentation().build(), &limits),
        Err(OfficeError::TooLarge { .. })
    ));
}

#[test]
fn reads_colour_references_and_shape_types() {
    let scheme: Vec<String> = DEFAULT_SCHEME
        .iter()
        .map(|colour| (*colour).to_owned())
        .collect();
    assert_eq!(
        reference_colour(0x0033_2211, &scheme).as_deref(),
        Some("112233")
    );
    assert_eq!(
        reference_colour(0x0800_0005, &scheme).as_deref(),
        Some("333399")
    );
    assert_eq!(reference_colour(0x1000_0005, &scheme), None);
    assert_eq!(resolve(Colour::Scheme(9), &scheme), None);
    let frame = Frame {
        x: 0,
        y: 0,
        width: 100,
        height: 100,
    };
    let every_mapped_kind =
        (2..=200).filter(|kind| !matches!(kind, 3 | 20 | 24..=32 | 109 | 120 | 136..=175));
    for kind in every_mapped_kind {
        assert!(
            matches!(
                geometry(kind, &[None; 10], frame, false, false),
                ShapeGeometry::Path(_)
            ),
            "{kind}"
        );
    }
    assert_eq!(
        geometry(1, &[None; 10], frame, false, false),
        ShapeGeometry::Rectangle
    );
    assert_eq!(
        geometry(120, &[None; 10], frame, false, false),
        ShapeGeometry::Ellipse
    );
    assert_eq!(
        geometry(20, &[None; 10], frame, true, false),
        ShapeGeometry::Line { flipped: true }
    );
    assert_eq!(
        geometry(999, &[None; 10], frame, false, false),
        ShapeGeometry::Rectangle
    );
    let mut round = [None; 10];
    round[0] = Some(10_800);
    let (ShapeGeometry::Path(rounder), ShapeGeometry::Path(plain)) = (
        geometry(2, &round, frame, false, false),
        geometry(2, &[None; 10], frame, false, false),
    ) else {
        panic!("a rounded rectangle is a path");
    };
    assert_ne!(rounder, plain, "the shape's own corner radius");
    assert_eq!(master_style_of(6), 0);
    assert_eq!(master_style_of(8), 1);
    assert_eq!(master_style_of(2), 2);
    assert_eq!(master_style_of(4), 4);
}

#[test]
fn reads_pictures_textures_and_shaded_blends_as_fills() {
    let presentation = read_ppt_presentation(&presentation().build(), &Limits::default()).unwrap();
    let second = &presentation.slides[1];
    let fills: Vec<Option<Paint>> = second.shapes[4..]
        .iter()
        .map(|shape| shape.fill.clone())
        .collect();
    let stop = |offset: f64, colour: &str| GradientStop {
        offset,
        colour: colour.to_owned(),
    };
    assert_eq!(
        fills[..3],
        [
            Some(Paint::Picture {
                media: 0,
                tile_emu: None
            }),
            Some(Paint::Picture {
                media: 0,
                tile_emu: None
            }),
            Some(Paint::Solid("FF0000".to_owned())),
        ],
        "a picture, a texture whose size is unknown, and a picture the store lacks"
    );
    assert_eq!(
        fills[3],
        Some(Paint::Gradient(Gradient {
            stops: vec![
                stop(0.0, "FF0000"),
                stop(0.5, "00FF00"),
                stop(1.0, "FF0000")
            ],
            kind: GradientKind::Linear { angle: 0.0 },
        })),
        "a focus in the middle blends out to both sides"
    );
    assert_eq!(
        fills[4],
        Some(Paint::Gradient(Gradient {
            stops: vec![stop(0.0, "FF0000"), stop(1.0, "FFFFFF")],
            kind: GradientKind::Radial { centre: (0.5, 0.5) },
        }))
    );
    assert_eq!(
        fills[5],
        Some(Paint::Gradient(Gradient {
            stops: vec![stop(0.0, "0A141E"), stop(1.0, "FF0000")],
            kind: GradientKind::Linear { angle: 90.0 },
        })),
        "no focus starts from the back colour, here the scheme's fill"
    );
}

#[test]
fn reads_the_transition_of_a_slide_show_info() {
    let info = |time: u32, direction: u8, effect: u8, flags: u16, speed: u8| {
        let mut body = vec![0_u8; 16];
        body[0..4].copy_from_slice(&time.to_le_bytes());
        body[8] = direction;
        body[9] = effect;
        body[10..12].copy_from_slice(&flags.to_le_bytes());
        body[12] = speed;
        body
    };
    let push = transition(&info(0, 1, 20, 0x1, 0)).unwrap();
    assert_eq!(
        (
            push.effect.as_str(),
            push.direction.as_deref(),
            push.duration_ms
        ),
        ("push", Some("u"), Some(1_000))
    );
    assert!(push.advance_on_click && push.advance_after_ms.is_none());
    let automatic = transition(&info(4_000, 0, 2, 0x80, 2)).unwrap();
    assert_eq!(automatic.effect, "blinds");
    assert_eq!(automatic.direction.as_deref(), Some("vert"));
    assert!(!automatic.advance_on_click);
    assert_eq!(automatic.advance_after_ms, Some(4_000));
    assert_eq!(
        transition(&info(0, 0, 0, 0x1, 0)),
        None,
        "a cut on a click is no transition"
    );
    assert_eq!(transition(&info(0, 0, 99, 0x1, 1)).unwrap().effect, "fade");
}

fn own_shape(id: u32) -> SlideShape {
    SlideShape {
        id: Some(id),
        inherited: false,
        frame: Frame::default(),
        rotation: 0.0,
        geometry: ShapeGeometry::Rectangle,
        fill: None,
        line: None,
        content: ShapeContent::None,
    }
}

#[test]
fn reads_the_builds_of_powerpoint_97() {
    let mut body = vec![0_u8; 28];
    body[4..6].copy_from_slice(&0x4_u16.to_le_bytes());
    body[12..16].copy_from_slice(&750_u32.to_le_bytes());
    body[16..18].copy_from_slice(&3_u16.to_le_bytes());
    body[21] = 0x0C;
    body[22] = 2;
    let data = container(
        CLIENT_DATA,
        0,
        &[container(
            ANIMATION_INFO,
            0,
            &[atom(ANIMATION_INFO_ATOM, 0, &body)],
        )],
    );
    let build = Build::read(&data).unwrap();
    assert_eq!(build.order, 3);
    let animation = build.animation(4);
    assert_eq!(
        (animation.shape, animation.preset, animation.preset_subtype),
        (4, 2, 2),
        "a fly in from the right"
    );
    assert_eq!(animation.trigger, AnimationTrigger::AfterPrevious);
    assert_eq!(animation.delay_ms, 750);
    assert_eq!(Build::read(&container(CLIENT_DATA, 0, &[])), None);
}

#[test]
fn reads_the_timeline_of_powerpoint_2002() {
    let variant = |id: u16, value: u32| {
        let mut body = vec![1_u8];
        body.extend_from_slice(&value.to_le_bytes());
        atom(TIME_VARIANT, id, &body)
    };
    let node_atom = |duration: i32| {
        let mut body = vec![0_u8; 32];
        body[24..28].copy_from_slice(&duration.to_le_bytes());
        atom(TIME_NODE_ATOM, 0, &body)
    };
    let mut condition = vec![0_u8; 16];
    condition[12..16].copy_from_slice(&250_u32.to_le_bytes());
    let mut visual = vec![0_u8; 20];
    visual[8..12].copy_from_slice(&42_u32.to_le_bytes());
    let effect = container(
        TIME_NODE,
        0,
        &[
            node_atom(-1),
            container(
                TIME_PROPERTIES,
                0,
                &[
                    variant(0x14, 2),
                    variant(0x09, 22),
                    variant(0x0A, 4),
                    variant(0x0B, 2),
                ],
            ),
            container(
                TIME_CONDITION,
                0,
                &[atom(TIME_CONDITION_ATOM, 0, &condition)],
            ),
            container(
                TIME_NODE,
                0,
                &[
                    node_atom(1_200),
                    container(0xF12D, 0, &[atom(VISUAL_SHAPE, 0, &visual)]),
                ],
            ),
        ],
    );
    let root = container(
        TIME_NODE,
        0,
        &[node_atom(-1), container(TIME_NODE, 0, &[effect])],
    );
    let mut blob = atom(0x2EEB, 0, &[0; 4]);
    blob.extend(root);
    let slide = container(
        PROGRAMMABLE_TAGS,
        0,
        &[container(BINARY_TAG, 0, &[atom(BINARY_TAG_DATA, 0, &blob)])],
    );
    let animations = timeline(&slide, &[own_shape(7), own_shape(42)]);
    assert_eq!(
        animations,
        vec![Animation {
            shape: 1,
            class: AnimationClass::Exit,
            preset: 22,
            preset_subtype: 4,
            trigger: AnimationTrigger::WithPrevious,
            delay_ms: 250,
            duration_ms: 1_200,
            motion_path: None,
        }]
    );
    assert!(
        timeline(&slide, &[own_shape(7)]).is_empty(),
        "no shape with that id"
    );
}
