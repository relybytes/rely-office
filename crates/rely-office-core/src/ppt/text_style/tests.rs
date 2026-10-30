// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::ppt::test_ppt::exception;

/// A paragraph exception setting every field, in the order the format keeps them.
fn full_paragraph() -> Vec<u8> {
    let mut fields = Vec::new();
    fields.extend_from_slice(&0x0003_u16.to_le_bytes()); // bullet flags: a bullet with a font
    fields.extend_from_slice(&0x2022_u16.to_le_bytes()); // bullet character
    fields.extend_from_slice(&2_u16.to_le_bytes()); // bullet font
    fields.extend_from_slice(&90_i16.to_le_bytes()); // bullet size
    fields.extend_from_slice(&[0x11, 0x22, 0x33, 0xFE]); // bullet colour
    fields.extend_from_slice(&1_u16.to_le_bytes()); // centred
    fields.extend_from_slice(&120_i16.to_le_bytes()); // line spacing
    fields.extend_from_slice(&(-80_i16).to_le_bytes()); // space before
    fields.extend_from_slice(&40_i16.to_le_bytes()); // space after
    fields.extend_from_slice(&288_i16.to_le_bytes()); // left margin
    fields.extend_from_slice(&144_i16.to_le_bytes()); // indent
    fields.extend_from_slice(&576_u16.to_le_bytes()); // default tab
    fields.extend_from_slice(&1_u16.to_le_bytes()); // one tab stop
    fields.extend_from_slice(&[0; 4]);
    fields.extend_from_slice(&0_u16.to_le_bytes()); // font alignment
    fields.extend_from_slice(&0_u16.to_le_bytes()); // wrap flags
    fields.extend_from_slice(&0_u16.to_le_bytes()); // direction
    exception(0x003F_FDFF, &fields)
}

fn full_character() -> Vec<u8> {
    let mut fields = Vec::new();
    fields.extend_from_slice(&0x0005_u16.to_le_bytes()); // bold and underlined
    fields.extend_from_slice(&3_u16.to_le_bytes()); // font
    fields.extend_from_slice(&[0; 6]); // the other three fonts
    fields.extend_from_slice(&24_u16.to_le_bytes()); // size
    fields.extend_from_slice(&[0x03, 0, 0, 0x05]); // scheme colour 5
    fields.extend_from_slice(&30_i16.to_le_bytes()); // position
    fields.extend_from_slice(&[0; 4]); // pp10 extension
    fields.extend_from_slice(&[0; 4]); // two more fonts
    fields.extend_from_slice(&[0; 4]); // pp11 extension
    exception(0x07FF_0007, &fields)
}

#[test]
fn reads_every_field_of_a_paragraph_exception() {
    let bytes = full_paragraph();
    let (style, end) = paragraph_exception(&bytes, 0).unwrap();
    assert_eq!(end, bytes.len());
    assert_eq!(
        style,
        ParagraphStyle {
            bullet: Some(true),
            bullet_character: Some(0x2022),
            bullet_font: Some(2),
            bullet_colour: Some(Colour::Rgb(0x11, 0x22, 0x33)),
            align: Some(1),
            line_spacing: Some(120),
            space_before: Some(-80),
            space_after: Some(40),
            left_margin: Some(288),
            indent: Some(144),
        }
    );
    let (empty, end) = paragraph_exception(&exception(0, &[]), 0).unwrap();
    assert_eq!((empty, end), (ParagraphStyle::default(), 4));
    assert_eq!(paragraph_exception(&bytes[..10], 0), None);
    assert_eq!(paragraph_exception(&[1], 0), None);
}

#[test]
fn reads_every_field_of_a_character_exception() {
    let bytes = full_character();
    let (style, end) = character_exception(&bytes, 0).unwrap();
    assert_eq!(end, bytes.len());
    assert_eq!(
        style,
        CharacterStyle {
            bold: Some(true),
            italic: Some(false),
            underline: Some(true),
            font: Some(3),
            size: Some(24),
            colour: Some(Colour::Scheme(5)),
        }
    );
    let (unset, _) = character_exception(&exception(0x0002_0000, &[18, 0]), 0).unwrap();
    assert_eq!((unset.bold, unset.size), (None, Some(18)));
    assert_eq!(character_exception(&bytes[..12], 0), None);
    let undefined = exception(0x0004_0000, &[1, 2, 3, 0xFF]);
    assert_eq!(character_exception(&undefined, 0).unwrap().0.colour, None);
}

#[test]
fn merges_a_style_over_its_fallback() {
    let own = ParagraphStyle {
        align: Some(2),
        ..ParagraphStyle::default()
    };
    let fallback = ParagraphStyle {
        align: Some(0),
        bullet: Some(true),
        ..ParagraphStyle::default()
    };
    assert_eq!(own.or(fallback).align, Some(2));
    assert_eq!(own.or(fallback).bullet, Some(true));
    let character = CharacterStyle {
        size: Some(12),
        ..CharacterStyle::default()
    };
    let base = CharacterStyle {
        size: Some(40),
        bold: Some(true),
        ..CharacterStyle::default()
    };
    assert_eq!(character.or(base).size, Some(12));
    assert_eq!(character.or(base).bold, Some(true));
}

#[test]
fn reads_the_runs_of_a_text() {
    let mut body = Vec::new();
    body.extend_from_slice(&6_u32.to_le_bytes());
    body.extend_from_slice(&0_u16.to_le_bytes());
    body.extend(exception(0x0800, &1_u16.to_le_bytes()));
    body.extend_from_slice(&5_u32.to_le_bytes());
    body.extend_from_slice(&1_u16.to_le_bytes());
    body.extend(exception(0, &[]));
    body.extend_from_slice(&3_u32.to_le_bytes());
    body.extend(exception(0x0001_0001, &[1, 0, 2, 0]));
    body.extend_from_slice(&8_u32.to_le_bytes());
    body.extend(exception(0, &[]));
    let (paragraphs, characters) = style_runs(&body, 10);
    assert_eq!(paragraphs.len(), 2);
    assert_eq!(paragraphs[0].0, 6);
    assert_eq!(paragraphs[0].2.align, Some(1));
    assert_eq!(paragraphs[1].1, 1, "the second paragraph's level");
    assert_eq!(characters.len(), 2);
    assert_eq!(characters[0].1.bold, Some(true));
    assert_eq!(characters[0].1.font, Some(2));
    let (none, nothing) = style_runs(&[], 3);
    assert!(none.is_empty() && nothing.is_empty());
}

#[test]
fn reads_the_levels_of_a_master_style() {
    let mut body = 2_u16.to_le_bytes().to_vec();
    body.extend(exception(0, &[]));
    body.extend(exception(0x0002_0000, &44_u16.to_le_bytes()));
    body.extend(exception(0x0800, &2_u16.to_le_bytes()));
    body.extend(exception(0x0002_0000, &28_u16.to_le_bytes()));
    let levels = master_levels(&body, 0);
    assert_eq!(levels.len(), 2);
    assert_eq!(levels[0].1.size, Some(44));
    assert_eq!(levels[1].0.align, Some(2));
    let mut with_numbers = 1_u16.to_le_bytes().to_vec();
    with_numbers.extend_from_slice(&0_u16.to_le_bytes());
    with_numbers.extend(exception(0, &[]));
    with_numbers.extend(exception(0x0002_0000, &20_u16.to_le_bytes()));
    assert_eq!(
        master_levels(&with_numbers, 5)[0].1.size,
        Some(20),
        "a level number comes first"
    );
    assert_eq!(master_levels(&[], 0), Vec::new());
    assert_eq!(master_levels(&body[..8], 0).len(), 0);
}
