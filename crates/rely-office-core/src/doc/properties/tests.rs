// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use super::*;

fn applied(grpprl: &[u8], style: &Character) -> Character {
    let mut character = style.clone();
    character.apply(grpprl, style);
    character
}

#[test]
fn reads_colours_from_the_palette_and_colour_references() {
    assert_eq!(palette_colour(0), None);
    assert_eq!(palette_colour(6).as_deref(), Some("FF0000"));
    assert_eq!(palette_colour(16).as_deref(), Some("C0C0C0"));
    assert_eq!(palette_colour(17), None);
    assert_eq!(colour_reference(0x0000_80FF).as_deref(), Some("FF8000"));
    assert_eq!(colour_reference(0xFF00_0000), None);
}

#[test]
fn applies_toggles_against_the_style() {
    let bold_style = Character {
        bold: true,
        ..Character::default()
    };
    assert!(applied(&[0x35, 0x08, 0x01], &Character::default()).bold);
    assert!(!applied(&[0x35, 0x08, 0x00], &bold_style).bold);
    assert!(applied(&[0x35, 0x08, 0x80], &bold_style).bold);
    assert!(!applied(&[0x35, 0x08, 0x81], &bold_style).bold);
    assert!(applied(&[0x35, 0x08, 0x81], &Character::default()).bold);
}

#[test]
fn applies_each_character_modifier() {
    let grpprl = [
        0x36, 0x08, 0x01, // italic
        0x37, 0x08, 0x01, // strike
        0x3A, 0x08, 0x01, // small capitals
        0x3B, 0x08, 0x01, // capitals
        0x3C, 0x08, 0x01, // hidden
        0x3E, 0x2A, 0x01, // underline
        0x48, 0x2A, 0x01, // superscript
        0x43, 0x4A, 0x18, 0x00, // twelve points
        0x42, 0x2A, 0x02, // blue
        0x0C, 0x2A, 0x07, // yellow highlight
        0x4F, 0x4A, 0x03, 0x00, // font 3
        0x55, 0x08, 0x01, // special
        0x03, 0x6A, 0x10, 0x00, 0x00, 0x00, // picture at 16
        0x0A, 0x08, 0x01, // an embedded object
        0x00, 0x08, 0x01, // deleted
        0x01, 0x08, 0x01, // inserted
        0x30, 0x4A, 0x0C, 0x00, // character style 12
        0x04, 0x48, 0x02, 0x00, // inserted by reviewer 2
        0x05, 0x68, 0x01, 0x02, 0x03, 0x04, // on this date
        0x63, 0x48, 0x01, 0x00, // deleted by reviewer 1
        0x64, 0x68, 0x05, 0x06, 0x07, 0x08, // on this date
    ];
    let character = applied(&grpprl, &Character::default());
    assert_eq!(
        character,
        Character {
            bold: false,
            italic: true,
            strike: true,
            underline: true,
            caps: true,
            small_caps: true,
            hidden: true,
            superscript: true,
            subscript: false,
            size: Some(24),
            colour: Some("0000FF".to_owned()),
            highlight: Some("FFFF00".to_owned()),
            font: Some(3),
            special: true,
            picture: Some(16),
            object: true,
            deleted: true,
            inserted: true,
            insertion: (2, 0x0403_0201),
            deletion: (1, 0x0807_0605),
            style: Some(12),
        }
    );
    let later = applied(
        &[
            0x48, 0x2A, 0x02, 0x70, 0x68, 0x11, 0x22, 0x33, 0x00, 0x43, 0x4A, 0, 0,
        ],
        &character,
    );
    assert!(later.subscript && !later.superscript);
    assert_eq!(later.colour.as_deref(), Some("112233"));
    assert_eq!(later.size, None, "a size of nothing is left unset");
    assert!(applied(&[0x53, 0x2A, 0x01], &Character::default()).strike);
}

#[test]
fn applies_each_paragraph_modifier() {
    let mut paragraph = Paragraph::default();
    paragraph.apply(&[
        0x61, 0x24, 0x01, // centred
        0x5E, 0x84, 0xD0, 0x02, // indent left 720
        0x5D, 0x84, 0x68, 0x01, // indent right 360
        0x60, 0x84, 0x98, 0xFE, // hang 360
        0x13, 0xA4, 0x78, 0x00, // 120 before
        0x14, 0xA4, 0xF0, 0x00, // 240 after
        0x12, 0x64, 0x14, 0x01, 0x01, 0x00, // 276 proportional
        0x07, 0x24, 0x01, // page break before
        0x16, 0x24, 0x01, // in a table
        0x17, 0x24, 0x01, // the row's end
        0x0A, 0x26, 0x02, // level 2
        0x0B, 0x46, 0x03, 0x00, // list override 3
        0x40, 0x26, 0x01, // outline level 1
        0x6D, 0x24, 0x01, // contextual spacing
        0x4D, 0xC6, 0x0A, 0, 0, 0, 0xFF, 0xFF, 0x00, 0xFF, 0x00, 0x00,
        0x00, // magenta shading
        0x26, 0x64, 0x08, 0x01, 0x06, 0x00, // a red bottom border of a point
        0x4E, 0xC6, 0x08, 0x00, 0x80, 0x00, 0x00, 0x04, 0x03, 0x00,
        0x00, // a green double top border
    ]);
    assert_eq!(paragraph.alignment, Some(1));
    assert_eq!(
        (
            paragraph.indent_left,
            paragraph.indent_right,
            paragraph.indent_first_line
        ),
        (Some(720), Some(360), Some(-360))
    );
    assert_eq!(
        (paragraph.space_before, paragraph.space_after),
        (Some(120), Some(240))
    );
    assert_eq!(
        paragraph.line_spacing,
        Some(LineSpacing {
            value: 276,
            proportional: true
        })
    );
    assert!(paragraph.page_break_before && paragraph.in_table && paragraph.row_end);
    assert_eq!(paragraph.table_depth, 1);
    assert_eq!((paragraph.list_level, paragraph.list_override), (2, 3));
    assert_eq!(paragraph.outline_level, Some(1));
    assert!(paragraph.contextual_spacing);
    assert_eq!(paragraph.shading.as_deref(), Some("FF00FF"));
    assert_eq!(
        paragraph.borders[2],
        Some(Border {
            style: BorderStyle::Single,
            width_eighths: 8,
            colour: Some("FF0000".to_owned()),
        })
    );
    assert_eq!(
        paragraph.borders[0],
        Some(Border {
            style: BorderStyle::Double,
            width_eighths: 4,
            colour: Some("008000".to_owned()),
        })
    );
    let mut nested = Paragraph::default();
    nested.apply(&[
        0x49, 0x66, 0x02, 0x00, 0x00, 0x00, 0x16, 0x24, 0x01, 0x40, 0x26, 0x09, 0x0A, 0x26, 0x0C,
    ]);
    assert_eq!(
        nested.table_depth, 2,
        "a depth set before the table flag is kept"
    );
    assert_eq!(nested.outline_level, None, "level 9 is body text");
    assert_eq!(nested.list_level, 8);
}

#[test]
fn reads_older_shading_and_border_forms() {
    assert_eq!(
        shading_80(6 << 5).as_deref(),
        Some("FF0000"),
        "the background when solid"
    );
    assert_eq!(
        shading_80(2 | (1 << 10)).as_deref(),
        Some("0000FF"),
        "the foreground of a pattern"
    );
    assert_eq!(shading_80(3 << 5 | 1 << 10).as_deref(), Some("00FFFF"));
    assert_eq!(shading(&[0, 0, 0, 0, 0, 0, 0, 0xFF, 0, 0]), None);
    assert_eq!(
        shading(&[0x10, 0x20, 0x30, 0x00, 0, 0, 0, 0xFF, 2, 0]).as_deref(),
        Some("102030"),
        "a pattern shows its foreground"
    );
    assert_eq!(border_80(&[4, 0, 1, 0]), None);
    assert_eq!(border_80(&[4, 0xFF, 1, 0]), None);
    assert_eq!(border(&[0, 0, 0, 0, 4, 0, 0, 0]), None);
    for (kind, style) in [
        (3, BorderStyle::Double),
        (6, BorderStyle::Dotted),
        (7, BorderStyle::Dashed),
        (22, BorderStyle::Dashed),
        (20, BorderStyle::Dotted),
        (1, BorderStyle::Single),
    ] {
        assert_eq!(border_style(kind), style);
    }
}

#[test]
fn reads_a_table_row() {
    let mut definition = vec![0x08, 0xD6];
    let mut operand = vec![3];
    for edge in [0_i16, 1_000, 2_000, 3_000] {
        operand.extend_from_slice(&edge.to_le_bytes());
    }
    let mut first = vec![0_u8; 20];
    first[4..8].copy_from_slice(&[4, 1, 1, 0]);
    let mut second = vec![0_u8; 20];
    second[0] = 0x03;
    let mut third = vec![0_u8; 20];
    third[0] = 0x20;
    operand.extend(first);
    operand.extend(second);
    operand.extend(third);
    definition.extend_from_slice(&u16::try_from(operand.len() + 1).unwrap().to_le_bytes());
    definition.extend(operand);
    definition.extend_from_slice(&[0x04, 0x34, 0x01]);
    definition.extend_from_slice(&[0x09, 0xD6, 0x06, 0xC0, 0x00, 0, 0, 0, 0]);
    let row = Row::read(&definition);
    assert!(row.header);
    assert!(!row.bordered);
    assert_eq!(row.cells.len(), 3);
    assert_eq!(
        row.cells[0],
        CellDefinition {
            left: 0,
            right: 1_000,
            merged: false,
            continues_above: false,
            bordered: true,
            fill: Some("FF0000".to_owned()),
        }
    );
    assert!(row.cells[1].merged && !row.cells[1].bordered);
    assert!(row.cells[2].continues_above);
    let with_borders = Row::read(&[
        0x05, 0xD6, 0x04, 0, 1, 0, 0, 0x13, 0xD6, 0x08, 0, 0, 0, 0, 4, 0, 0, 0,
    ]);
    assert!(with_borders.bordered);
    let shaded = Row::read(&[
        0x08, 0xD6, 0x06, 0x00, 1, 0, 0, 0x10, 0x27, 0x12, 0xD6, 0x0A, 0, 0, 0, 0, 0x11, 0x22,
        0x33, 0, 0, 0,
    ]);
    assert_eq!(shaded.cells.len(), 1);
    assert_eq!(shaded.cells[0].right, 10_000);
    assert_eq!(shaded.cells[0].fill.as_deref(), Some("112233"));
}
