// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use super::*;
use crate::doc::test_doc::{plex, png_blip, record, store_entry};
use crate::office_art::GROUP_CONTAINER;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nimmagine";

/// A `PICF` of a picture `width` by `height` twips at the given scale, and its drawing data.
fn picture(width: u16, height: u16, scale: u16, name: Option<&str>, art: &[u8]) -> Vec<u8> {
    let mut header = vec![0_u8; 68];
    header[4..6].copy_from_slice(&68_u16.to_le_bytes());
    header[6..8].copy_from_slice(&(if name.is_some() { SHAPE_FILE } else { 0x0064 }).to_le_bytes());
    header[28..30].copy_from_slice(&width.to_le_bytes());
    header[30..32].copy_from_slice(&height.to_le_bytes());
    header[32..34].copy_from_slice(&scale.to_le_bytes());
    header[34..36].copy_from_slice(&scale.to_le_bytes());
    if let Some(name) = name {
        header.push(u8::try_from(name.len()).unwrap());
        header.extend_from_slice(name.as_bytes());
    }
    header.extend_from_slice(art);
    let total = u32::try_from(header.len()).unwrap();
    header[0..4].copy_from_slice(&total.to_le_bytes());
    header
}

fn shape_container() -> Vec<u8> {
    record(0xF, 0, SHAPE_CONTAINER, &[])
}

fn properties(kind: u16, values: &[(u16, u32)]) -> Vec<u8> {
    let mut body = Vec::new();
    for (identifier, value) in values {
        body.extend_from_slice(&identifier.to_le_bytes());
        body.extend_from_slice(&value.to_le_bytes());
    }
    record(3, u16::try_from(values.len()).unwrap(), kind, &body)
}

fn shape(identifier: u32, values: &[(u16, u32)], tertiary: &[(u16, u32)]) -> Vec<u8> {
    let mut body = record(2, 75, SHAPE, &[identifier.to_le_bytes(), [0; 4]].concat());
    body.extend(properties(PROPERTIES, values));
    body.extend(properties(TERTIARY_PROPERTIES, tertiary));
    record(0xF, 0, SHAPE_CONTAINER, &body)
}

#[test]
fn reads_an_inline_picture_and_its_size() {
    let mut art = shape_container();
    art.extend(store_entry(Some(&png_blip(PNG)), 0, 0));
    let mut data = vec![0_u8; 10];
    data.extend(picture(1_440, 720, 500, None, &art));
    let found = inline_picture(&data, 10, &[]).unwrap();
    assert_eq!(
        found,
        InlinePicture {
            bytes: Some(PNG.to_vec()),
            width: 720,
            height: 360,
        }
    );
    let named = picture(100, 100, 1_000, Some("foto.png"), &art);
    assert_eq!(
        inline_picture(&named, 0, &[]).unwrap().bytes,
        Some(PNG.to_vec())
    );
    assert_eq!(
        inline_picture(&data, 3, &[]),
        None,
        "no picture starts there"
    );
}

#[test]
fn reads_a_picture_whose_image_is_in_the_delay_stream() {
    let mut delay = vec![0_u8; 20];
    let blip = png_blip(PNG);
    delay.extend_from_slice(&blip);
    let mut art = shape_container();
    art.extend(store_entry(None, u32::try_from(blip.len()).unwrap(), 20));
    let data = picture(1_000, 1_000, 1_000, None, &art);
    assert_eq!(
        inline_picture(&data, 0, &delay).unwrap().bytes,
        Some(PNG.to_vec())
    );
    assert_eq!(inline_picture(&data, 0, &delay[..30]).unwrap().bytes, None);
    let without_entry = picture(1_000, 1_000, 1_000, None, &shape_container());
    assert_eq!(
        inline_picture(&without_entry, 0, &delay).unwrap().bytes,
        None
    );
}

#[test]
fn reads_the_floating_shapes_a_story_anchors() {
    let mut anchor = vec![0_u8; ANCHOR_SIZE];
    anchor[0..4].copy_from_slice(&1_025_u32.to_le_bytes());
    for (index, value) in [100_i32, -200, 1_100, 800].into_iter().enumerate() {
        anchor[4 + index * 4..8 + index * 4].copy_from_slice(&value.to_le_bytes());
    }
    let flags: u16 = (1 << 1) | (2 << 3) | (3 << 5) | 0x4000;
    anchor[20..22].copy_from_slice(&flags.to_le_bytes());
    let anchors = shape_anchors(&plex(&[12, 99], &[&anchor]));
    assert_eq!(
        anchors[&12],
        ShapeAnchor {
            shape: 1_025,
            bounds: [100, -200, 1_100, 800],
            horizontal_base: 1,
            vertical_base: 2,
            wrap: 3,
            behind_text: true,
        }
    );
    assert!(shape_anchors(&[1, 2]).is_empty());
}

#[test]
fn reads_the_drawings_their_images_and_what_each_shape_shows() {
    let store = record(
        0xF,
        2,
        STORE_CONTAINER,
        &[
            store_entry(Some(&png_blip(PNG)), 0, 0),
            store_entry(None, 4, 999),
        ]
        .concat(),
    );
    let group = record(0xF, 0, GROUP_CONTAINER, &store);
    let mut shapes = shape(
        1_025,
        &[
            (PICTURE_PROPERTY | 0x4000, 1),
            (0x0181, 0x00CC_F2FF),
            (0x01BF, (1 << 20) | (1 << 4)),
        ],
        &[(0x038F, 3), (0x0390, 2), (0x0391, 1), (0x0392, 0)],
    );
    shapes.extend(shape(
        1_026,
        &[
            (TEXT_PROPERTY, 0x0001_0000),
            (0x01C0, 0x0000_90BF),
            (0x01CB, 25_400),
            (0x01FF, 1 << 19),
        ],
        &[],
    ));
    shapes.extend(record(
        0xF,
        0,
        SHAPE_CONTAINER,
        &record(3, 0, PROPERTIES, &[]),
    ));
    let drawing = record(0xF, 0, DRAWING_CONTAINER, &record(0xF, 0, 0xF003, &shapes));
    let mut content = group;
    content.push(0);
    content.extend(drawing);
    content.push(1);
    content.extend(record(0xF, 0, 0xF00D, &[]));
    let drawings = Drawings::read(&content, &[]);
    assert_eq!(drawings.image(1), Some(PNG));
    assert_eq!(drawings.image(2), None, "a delayed image past the stream");
    assert_eq!(drawings.image(0), None);
    assert_eq!(drawings.image(3), None);
    assert_eq!(drawings.shapes.len(), 2);
    let picture = drawings.shapes[&1_025];
    assert_eq!(picture.picture, Some(1));
    assert_eq!(picture.fill, Some(0x00CC_F2FF));
    assert!(picture.filled && picture.stroked);
    assert_eq!(
        (
            picture.horizontal_align,
            picture.horizontal_base,
            picture.vertical_align,
            picture.vertical_base
        ),
        (Some(3), Some(2), Some(1), Some(0))
    );
    let text_box = drawings.shapes[&1_026];
    assert_eq!(text_box.text, Some(0x0001_0000));
    assert_eq!(
        (text_box.line, text_box.line_width),
        (Some(0x90BF), Some(25_400))
    );
    assert!(!text_box.stroked && text_box.filled);
    assert!(Drawings::read(&[], &[]).shapes.is_empty());
}

#[test]
fn stops_at_drawings_nested_too_deep() {
    let mut nested = shape(7, &[], &[]);
    for _ in 0..(MAX_DEPTH + 4) {
        nested = record(0xF, 0, 0xF003, &nested);
    }
    let mut shapes = HashMap::new();
    collect_shapes(&nested, &mut shapes, 0);
    assert!(shapes.is_empty());
}
