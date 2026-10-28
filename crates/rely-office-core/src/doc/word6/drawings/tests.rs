// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;

pub(in super::super) const BLUE: u32 = 0x00FF_0000;
pub(in super::super) const YELLOW: u32 = 0x0000_FFFF;
pub(in super::super) const RED: u32 = 0x0000_00FF;
pub(in super::super) const GREEN: u32 = 0x0000_C000;

/// A primitive: its kind, its frame `xa`, `ya`, `dxa`, `dya` and the record of its kind.
pub(in super::super) fn primitive(kind: u16, frame: [i16; 4], body: &[u8]) -> Vec<u8> {
    let mut record = kind.to_le_bytes().to_vec();
    record.extend_from_slice(
        &u16::try_from(PRIMITIVE_HEADER + body.len())
            .unwrap()
            .to_le_bytes(),
    );
    for value in frame {
        record.extend_from_slice(&value.to_le_bytes());
    }
    record.extend_from_slice(body);
    record
}

/// A `DPLINETYPE`: colour, width in twips and style.
pub(in super::super) fn line_type(colour: u32, width: u16, style: u16) -> Vec<u8> {
    let mut bytes = colour.to_le_bytes().to_vec();
    bytes.extend_from_slice(&width.to_le_bytes());
    bytes.extend_from_slice(&style.to_le_bytes());
    bytes
}

/// A `DPFILL`: foreground, background and pattern.
pub(in super::super) fn fill(foreground: u32, background: u32, pattern: u16) -> Vec<u8> {
    let mut bytes = foreground.to_le_bytes().to_vec();
    bytes.extend_from_slice(&background.to_le_bytes());
    bytes.extend_from_slice(&pattern.to_le_bytes());
    bytes
}

/// The record of a rectangle, an ellipse or a text box: line, fill, shadow and its flags.
pub(in super::super) fn shape_body(
    line: u32,
    background: u32,
    flags: u16,
    margin: bool,
) -> Vec<u8> {
    let mut body = line_type(line, 20, 0);
    body.extend(fill(0, background, 1));
    body.extend_from_slice(&[0; SHADOW]);
    body.extend_from_slice(&flags.to_le_bytes());
    if margin {
        body.extend_from_slice(&0_u16.to_le_bytes());
    }
    body
}

pub(in super::super) fn line_body(start: (i16, i16), end: (i16, i16), colour: u32) -> Vec<u8> {
    let mut body = Vec::new();
    for value in [start.0, start.1, end.0, end.1] {
        body.extend_from_slice(&value.to_le_bytes());
    }
    body.extend(line_type(colour, 40, 0));
    body.extend_from_slice(&[0; LINE_END + SHADOW]);
    body
}

pub(in super::super) fn arc_body(colour: u32, facing_left: bool, facing_up: bool) -> Vec<u8> {
    let mut body = line_type(0, 20, 0);
    body.extend(fill(0, colour, 1));
    body.extend_from_slice(&[0; SHADOW]);
    body.push(u8::from(facing_left));
    body.push(u8::from(facing_up));
    body
}

pub(in super::super) fn polyline_body(points: &[(i16, i16)], closed: bool, colour: u32) -> Vec<u8> {
    let mut body = line_type(0, 20, 0);
    body.extend(fill(0, colour, 1));
    body.extend_from_slice(&[0; LINE_END + SHADOW]);
    let count = u16::try_from(points.len()).unwrap();
    body.extend_from_slice(&((count << 1) | u16::from(closed)).to_le_bytes());
    for (x, y) in points {
        body.extend_from_slice(&x.to_le_bytes());
        body.extend_from_slice(&y.to_le_bytes());
    }
    body
}

pub(in super::super) fn group(frame: [i16; 4], children: &[Vec<u8>]) -> Vec<u8> {
    let mut body = u16::try_from(children.len())
        .unwrap()
        .to_le_bytes()
        .to_vec();
    for child in children {
        body.extend_from_slice(child);
    }
    primitive(0, frame, &body)
}

/// A drawing object: its header with its bases, then its primitives.
pub(in super::super) fn drawing_object(
    horizontal: u8,
    vertical: u8,
    primitives: &[Vec<u8>],
) -> Vec<u8> {
    let length: usize = primitives.iter().map(Vec::len).sum();
    let mut object = 0_u16.to_le_bytes().to_vec();
    object.extend_from_slice(&u16::try_from(DO_HEADER + length).unwrap().to_le_bytes());
    object.extend_from_slice(&[horizontal, vertical, 0, 0, 0, 0]);
    for primitive in primitives {
        object.extend_from_slice(primitive);
    }
    object
}

fn reader<'a>(stream: &'a [u8], positions: &'a [u32], next: &'a mut usize) -> Reader<'a> {
    Reader {
        stream,
        items: Vec::new(),
        text_boxes: Vec::new(),
        box_positions: positions,
        box_story: 100,
        next_box: next,
        read: 0,
    }
}

fn read(primitives: &[Vec<u8>], positions: &[u32]) -> (Vec<Item>, Vec<Word6TextBox>) {
    let stream: Vec<u8> = primitives.concat();
    let mut next = 0;
    let mut reader = reader(&stream, positions, &mut next);
    reader.primitives(0, stream.len(), (100, 200), 0, usize::MAX);
    (reader.items, reader.text_boxes)
}

#[test]
fn colours_are_rgb_or_a_share_of_grey() {
    assert_eq!(colour(BLUE), "0000FF");
    assert_eq!(colour(0x0012_3456), "563412");
    assert_eq!(colour(0x0100_0000), "FFFFFF");
    assert_eq!(colour(0x0100_00C8), "000000");
    assert_eq!(colour(0x0100_0064), "808080");
    assert_eq!(blend(RED, 0x00FF_FFFF, 50), "FF7F7F");
}

#[test]
fn fills_and_lines_read_their_patterns_and_styles() {
    assert_eq!(fill_of(&fill(RED, BLUE, 0)), None);
    assert_eq!(fill_of(&fill(RED, BLUE, 1)).as_deref(), Some("0000FF"));
    assert_eq!(fill_of(&fill(RED, BLUE, 40)).as_deref(), Some("0000FF"));
    assert_eq!(
        fill_of(&fill(RED, 0x00FF_FFFF, 4)).as_deref(),
        Some("FFCCCC")
    );
    assert_eq!(
        line_of(&line_type(RED, 20, 0)),
        Some(("FF0000".to_owned(), 12_700))
    );
    assert_eq!(
        line_of(&line_type(RED, 0, 1)),
        Some(("FF0000".to_owned(), 9_525))
    );
    assert_eq!(line_of(&line_type(RED, 20, 5)), None);
}

#[test]
fn reads_rectangles_ellipses_and_rounded_rectangles_in_their_frames() {
    let (items, boxes) = read(
        &[
            primitive(3, [10, 20, 300, 100], &shape_body(RED, BLUE, 0, false)),
            primitive(4, [0, 0, 200, 200], &shape_body(RED, YELLOW, 0, false)),
            primitive(3, [0, 0, 200, 100], &shape_body(RED, BLUE, 1, false)),
        ],
        &[],
    );
    assert_eq!(boxes, Vec::new());
    assert_eq!(items.len(), 3);
    assert_eq!(
        (items[0].left, items[0].top, items[0].width, items[0].height),
        (110, 220, 300, 100)
    );
    assert_eq!(items[0].fill.as_deref(), Some("0000FF"));
    assert_eq!(
        items[0].paths,
        rectangle_paths(300 * EMU_PER_TWIP, 100 * EMU_PER_TWIP)
    );
    assert_eq!(items[1].fill.as_deref(), Some("FFFF00"));
    assert_ne!(
        items[2].paths,
        rectangle_paths(200 * EMU_PER_TWIP, 100 * EMU_PER_TWIP)
    );
}

#[test]
fn reads_a_line_between_its_ends_whichever_way_it_runs() {
    let (items, _) = read(
        &[primitive(
            1,
            [0, 0, 0, 0],
            &line_body((300, 0), (0, 100), RED),
        )],
        &[],
    );
    let item = &items[0];
    assert_eq!(
        (item.left, item.top, item.width, item.height),
        (100, 200, 300, 100)
    );
    assert_eq!(
        item.paths[0].commands,
        vec![
            PathCommand::MoveTo(point(300, 0)),
            PathCommand::LineTo(point(0, 100)),
        ]
    );
    assert_eq!(item.line, Some(("FF0000".to_owned(), 40 * EMU_PER_TWIP)));
    assert!(!item.paths[0].filled);
}

#[test]
fn reads_each_quarter_of_an_arc_about_its_corner() {
    for (left, up, centre) in [
        (true, true, (0, 100)),
        (false, true, (0, 0)),
        (false, false, (200, 0)),
        (true, false, (200, 100)),
    ] {
        let (items, _) = read(
            &[primitive(5, [0, 0, 200, 100], &arc_body(GREEN, left, up))],
            &[],
        );
        let paths = &items[0].paths;
        assert_eq!(paths.len(), 2, "the wedge filled, the curve lined");
        assert_eq!(
            paths[0].commands[0],
            PathCommand::MoveTo(point(centre.0, centre.1))
        );
        assert!(paths[0].filled && !paths[0].stroked);
        assert!(!paths[1].filled && paths[1].stroked);
    }
    let mut hollow = arc_body(GREEN, true, true);
    hollow[16..18].copy_from_slice(&0_u16.to_le_bytes());
    let (items, _) = read(&[primitive(5, [0, 0, 200, 100], &hollow)], &[]);
    assert_eq!(items[0].paths.len(), 1);
}

#[test]
fn reads_polylines_open_and_closed() {
    let triangle = [(0, 100), (50, 0), (100, 100)];
    let (items, _) = read(
        &[primitive(
            6,
            [10, 10, 100, 100],
            &polyline_body(&triangle, true, GREEN),
        )],
        &[],
    );
    let item = &items[0];
    assert_eq!(
        (item.left, item.top, item.width, item.height),
        (110, 210, 100, 100)
    );
    assert_eq!(item.paths[0].commands.last(), Some(&PathCommand::Close));
    assert!(item.paths[0].filled);
    let (items, _) = read(
        &[primitive(
            6,
            [0, 0, 100, 100],
            &polyline_body(&triangle, false, GREEN),
        )],
        &[],
    );
    assert!(!items[0].paths[0].filled);
    assert_eq!(items[0].fill, None);
    let (items, _) = read(
        &[primitive(
            6,
            [0, 0, 1, 1],
            &polyline_body(&[(0, 0)], false, GREEN),
        )],
        &[],
    );
    assert!(items.is_empty(), "one point draws nothing");
}

#[test]
fn reads_text_boxes_in_order_from_the_text_box_story() {
    let body = shape_body(RED, YELLOW, 1, true);
    let (items, boxes) = read(
        &[
            primitive(2, [0, 0, 500, 200], &body),
            primitive(2, [600, 0, 500, 200], &body),
            primitive(2, [0, 0, 1, 1], &body),
        ],
        &[0, 7, 15],
    );
    assert!(items.is_empty());
    assert_eq!(boxes.len(), 2, "the third has no text left");
    assert_eq!(boxes[0].bounds, [100, 200, 600, 400]);
    assert_eq!(boxes[0].text, 100..107);
    assert_eq!(boxes[1].text, 107..115);
    assert_eq!(boxes[0].fill.as_deref(), Some("FFFF00"));
    assert_eq!(boxes[0].line, Some(("FF0000".to_owned(), 8)));
    assert!(boxes[0].rounded);
}

#[test]
fn reads_a_group_moved_by_its_corner_and_a_callout() {
    let children = [
        primitive(3, [0, 0, 100, 100], &shape_body(RED, BLUE, 0, false)),
        primitive(4, [200, 0, 100, 100], &shape_body(RED, BLUE, 0, false)),
    ];
    let (items, _) = read(&[group([1000, 500, 300, 100], &children)], &[]);
    assert_eq!(items.len(), 2);
    assert_eq!((items[0].left, items[0].top), (1100, 700));
    assert_eq!((items[1].left, items[1].top), (1300, 700));
    let mut nested = group([0, 0, 0, 0], &[group([0, 0, 0, 0], &children)]);
    let (items, _) = read(&[nested.clone()], &[]);
    assert_eq!(items.len(), 2);
    nested.truncate(nested.len() - 10);
    let stream = nested.clone();
    let mut next = 0;
    let mut cut = reader(&stream, &[], &mut next);
    cut.primitives(0, stream.len(), (0, 0), 0, usize::MAX);
    assert!(
        cut.items.len() <= 2,
        "a group cut short reads what it holds"
    );

    let mut callout = vec![0_u8; 8];
    callout
        .extend(primitive(2, [0, 0, 400, 200], &shape_body(RED, YELLOW, 0, true))[..12].to_vec());
    callout.extend(shape_body(RED, YELLOW, 0, true));
    callout.extend(primitive(6, [0, 0, 0, 0], &[])[..12].to_vec());
    callout.extend(polyline_body(&[(400, 200), (600, 400)], false, RED));
    let (items, boxes) = read(&[primitive(7, [1000, 0, 600, 400], &callout)], &[0, 4]);
    assert_eq!(boxes.len(), 1);
    assert_eq!(boxes[0].bounds, [1100, 200, 1500, 400]);
    assert_eq!(items.len(), 1);
    assert_eq!((items[0].left, items[0].top), (1500, 400));
}

#[test]
fn a_drawing_is_the_picture_of_its_shapes_over_their_box() {
    let object = drawing_object(
        2,
        2,
        &[
            primitive(3, [100, 100, 1440, 720], &shape_body(RED, BLUE, 0, false)),
            primitive(4, [1800, 100, 720, 720], &shape_body(RED, YELLOW, 0, false)),
        ],
    );
    let stream: Vec<u8> = [vec![0; 4], object].concat();
    let mut next = 0;
    let mut reader = reader(&stream, &[], &mut next);
    reader.primitives(4 + DO_HEADER, stream.len(), (0, 0), 0, usize::MAX);
    let drawing = drawing_of(reader.items, reader.text_boxes, 2, 2);
    assert_eq!(drawing.anchor.bounds, [100, 100, 2520, 820]);
    assert_eq!(
        (drawing.anchor.horizontal_base, drawing.anchor.vertical_base),
        (2, 2)
    );
    let svg = String::from_utf8(drawing.picture.unwrap()).unwrap();
    assert!(svg.contains("#0000FF") && svg.contains("#FFFF00"), "{svg}");
    let empty = drawing_of(Vec::new(), Vec::new(), 9, 9);
    assert_eq!(empty.picture, None);
    assert_eq!(
        (empty.anchor.horizontal_base, empty.anchor.vertical_base),
        (2, 2)
    );
}

#[test]
fn unknown_and_damaged_primitives_are_skipped() {
    let (items, _) = read(&[primitive(9, [0, 0, 10, 10], &[1, 2, 3])], &[]);
    assert!(items.is_empty());
    let mut broken = primitive(3, [0, 0, 10, 10], &shape_body(RED, BLUE, 0, false));
    broken[2..4].copy_from_slice(&4_u16.to_le_bytes());
    let (items, _) = read(&[broken], &[]);
    assert!(items.is_empty());
}
