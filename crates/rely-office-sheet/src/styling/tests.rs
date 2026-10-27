// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use rely_office_core::BorderStyle;

use super::*;
use crate::value::Locale;

const IT: Locale = Locale::Italian;

fn at(row: u32, column: u32) -> CellRef {
    CellRef::new(row, column)
}

fn thin() -> Border {
    Border {
        style: BorderStyle::Single,
        width_eighths: 4,
        colour: Some("000000".to_owned()),
    }
}

#[test]
fn patches_the_look_of_a_rectangle_and_shares_its_styles() {
    let mut workbook = Workbook::new();
    let patch = StylePatch {
        bold: Some(true),
        italic: Some(true),
        underline: Some(true),
        strike: Some(true),
        font_family: Some(" Arial ".to_owned()),
        font_size: Some(14.0),
        font_colour: Some("#c00000".to_owned()),
        fill: Some("FFFF00".to_owned()),
        horizontal: Some("center".to_owned()),
        vertical: Some("top".to_owned()),
        wrap: Some(true),
        number_format: Some("#,##0.00 \"€\"".to_owned()),
    };
    workbook.set_style(0, (at(1, 1), at(0, 0)), &patch).unwrap();
    let style = workbook.style(0, at(1, 1)).clone();
    assert_eq!(style.font.family.as_deref(), Some("Arial"));
    assert_eq!(style.font.size_tenths, Some(140));
    assert_eq!(style.font.colour.as_deref(), Some("C00000"));
    assert!(style.font.bold && style.font.italic && style.font.underline && style.font.strike);
    assert_eq!(style.fill.as_deref(), Some("FFFF00"));
    assert_eq!(style.horizontal, Some(HorizontalAlignment::Center));
    assert_eq!(style.vertical, Some(VerticalAlignment::Top));
    assert!(style.wrap);
    assert_eq!(workbook.styles().len(), 2, "four cells share one style");
    assert_eq!(workbook.style_id(0, at(0, 0)), 1);
    assert_eq!(workbook.style_id(0, at(2, 2)), 0);
    assert_eq!(workbook.style(0, at(2, 2)), &CellStyle::default());
    assert_eq!(workbook.style_id(9, at(0, 0)), 0);

    workbook.set_input(0, at(0, 0), "1234,5", IT).unwrap();
    assert_eq!(
        workbook.display(0, at(0, 0), IT),
        "1.234,50 €",
        "the style's code shows"
    );

    let back = StylePatch {
        bold: Some(false),
        italic: Some(false),
        underline: Some(false),
        strike: Some(false),
        font_family: Some(String::new()),
        font_size: Some(0.0),
        font_colour: Some(String::new()),
        fill: Some(String::new()),
        horizontal: Some("general".to_owned()),
        vertical: Some("bottom".to_owned()),
        wrap: Some(false),
        number_format: Some("General".to_owned()),
    };
    workbook.set_style(0, (at(0, 0), at(1, 1)), &back).unwrap();
    assert_eq!(
        workbook.style_id(0, at(1, 1)),
        0,
        "a style back to the default is dropped"
    );
    assert_eq!(workbook.sheets()[0].styled_cells().count(), 0);
    assert_eq!(workbook.display(0, at(0, 0), IT), "1234,5");
}

#[test]
fn refuses_style_values_no_cell_can_show() {
    let mut workbook = Workbook::new();
    let range = (at(0, 0), at(0, 0));
    for patch in [
        StylePatch {
            font_size: Some(0.5),
            ..StylePatch::default()
        },
        StylePatch {
            font_size: Some(f64::NAN),
            ..StylePatch::default()
        },
        StylePatch {
            font_colour: Some("red".to_owned()),
            ..StylePatch::default()
        },
        StylePatch {
            fill: Some("#12345".to_owned()),
            ..StylePatch::default()
        },
        StylePatch {
            horizontal: Some("middle".to_owned()),
            ..StylePatch::default()
        },
        StylePatch {
            vertical: Some("left".to_owned()),
            ..StylePatch::default()
        },
        StylePatch {
            font_family: Some("x".repeat(32)),
            ..StylePatch::default()
        },
        StylePatch {
            number_format: Some("0".repeat(256)),
            ..StylePatch::default()
        },
    ] {
        assert_eq!(
            workbook.set_style(0, range, &patch),
            Err(EditError::InvalidStyle)
        );
    }
    assert_eq!(
        workbook.set_style(1, range, &StylePatch::default()),
        Err(EditError::NoSuchSheet(1))
    );
    assert_eq!(
        workbook.set_style(0, (at(0, 0), at(MAX_ROWS, 0)), &StylePatch::default()),
        Err(EditError::OutOfBounds)
    );
    assert_eq!(
        workbook.set_style(0, (at(0, 0), at(1_000_000, 1)), &StylePatch::default()),
        Err(EditError::OutOfBounds),
        "past the cells one edit may style"
    );
    assert_eq!(workbook.styles().len(), 1);
}

#[test]
fn puts_borders_on_the_edges_a_placement_names() {
    let mut workbook = Workbook::new();
    let range = (at(0, 0), at(2, 2));
    let edges = |workbook: &Workbook, row, column| {
        let borders = &workbook.style(0, at(row, column)).borders;
        [&borders.top, &borders.right, &borders.bottom, &borders.left].map(Option::is_some)
    };
    workbook
        .set_borders(0, range, BorderPlacement::Outline, Some(&thin()))
        .unwrap();
    assert_eq!(edges(&workbook, 0, 0), [true, false, false, true]);
    assert_eq!(edges(&workbook, 1, 1), [false; 4]);
    assert_eq!(edges(&workbook, 2, 2), [false, true, true, false]);
    workbook
        .set_borders(0, range, BorderPlacement::Inside, Some(&thin()))
        .unwrap();
    assert_eq!(edges(&workbook, 1, 1), [true; 4]);
    assert_eq!(edges(&workbook, 0, 0), [true, true, true, true]);
    workbook
        .set_borders(0, range, BorderPlacement::All, None)
        .unwrap();
    assert_eq!(edges(&workbook, 1, 1), [false; 4]);
    for (placement, expected) in [
        (BorderPlacement::Top, [true, false, false, false]),
        (BorderPlacement::Bottom, [false, false, true, false]),
        (BorderPlacement::Left, [false, false, false, true]),
        (BorderPlacement::Right, [false, true, false, false]),
    ] {
        workbook
            .set_borders(0, (at(5, 5), at(5, 5)), placement, Some(&thin()))
            .unwrap();
        assert_eq!(edges(&workbook, 5, 5), expected, "{placement:?}");
        workbook
            .set_borders(0, (at(5, 5), at(5, 5)), BorderPlacement::All, None)
            .unwrap();
    }
    workbook
        .set_borders(0, (at(0, 0), at(0, 0)), BorderPlacement::All, Some(&thin()))
        .unwrap();
    workbook.clear_styles(0, (at(0, 0), at(9, 9))).unwrap();
    assert_eq!(workbook.sheets()[0].styled_cells().count(), 0);
    assert_eq!(
        workbook.clear_styles(3, (at(0, 0), at(0, 0))),
        Err(EditError::NoSuchSheet(3))
    );
}

#[test]
fn merges_cells_keeping_the_first_one() {
    let mut workbook = Workbook::new();
    workbook.set_input(0, at(0, 0), "titolo", IT).unwrap();
    workbook.set_input(0, at(0, 1), "perso", IT).unwrap();
    workbook.set_input(0, at(3, 0), "=A1", IT).unwrap();
    workbook.merge_cells(0, (at(0, 2), at(0, 0))).unwrap();
    assert_eq!(
        workbook.merges(0),
        [CellRange {
            first_row: 0,
            first_column: 0,
            last_row: 0,
            last_column: 2
        }]
    );
    assert_eq!(workbook.display(0, at(0, 0), IT), "titolo");
    assert_eq!(workbook.display(0, at(0, 1), IT), "");
    workbook.merge_cells(0, (at(0, 1), at(1, 3))).unwrap();
    assert_eq!(
        workbook.merges(0).len(),
        1,
        "an overlapped merge is replaced"
    );
    assert_eq!(workbook.merges(0)[0].first_column, 1);
    workbook.merge_cells(0, (at(5, 5), at(5, 5))).unwrap();
    assert_eq!(workbook.merges(0).len(), 1, "a single cell is not a merge");
    workbook.unmerge_cells(0, (at(1, 2), at(1, 2))).unwrap();
    assert_eq!(workbook.merges(0), []);
    assert_eq!(workbook.merges(7), []);
    assert_eq!(
        workbook.merge_cells(0, (at(0, 0), at(MAX_ROWS, 0))),
        Err(EditError::OutOfBounds)
    );
    assert_eq!(
        workbook.merge_cells(2, (at(0, 0), at(1, 1))),
        Err(EditError::NoSuchSheet(2))
    );
    assert_eq!(
        workbook.unmerge_cells(2, (at(0, 0), at(1, 1))),
        Err(EditError::NoSuchSheet(2))
    );
}

#[test]
fn sets_row_heights() {
    let mut workbook = Workbook::new();
    workbook.set_row_height(0, 3, Some(30.0)).unwrap();
    workbook.set_row_height(0, 4, Some(9_999.0)).unwrap();
    workbook.set_row_height(0, 5, Some(0.0)).unwrap();
    workbook.set_row_height(0, 6, Some(f64::NAN)).unwrap();
    let heights = &workbook.sheets()[0].row_heights;
    assert_eq!(heights.get(&3), Some(&30.0));
    assert_eq!(heights.get(&4), Some(&MAX_ROW_HEIGHT));
    assert_eq!(heights.get(&5), Some(&0.0));
    assert_eq!(heights.get(&6), None);
    workbook.set_row_height(0, 3, None).unwrap();
    assert_eq!(workbook.sheets()[0].row_heights.get(&3), None);
    assert_eq!(
        workbook.set_row_height(0, MAX_ROWS, Some(1.0)),
        Err(EditError::OutOfBounds)
    );
    assert_eq!(
        workbook.set_row_height(1, 0, Some(1.0)),
        Err(EditError::NoSuchSheet(1))
    );
}

#[test]
fn moves_styles_merges_and_heights_with_their_rows_and_columns() {
    let mut workbook = Workbook::new();
    let bold = StylePatch {
        bold: Some(true),
        ..StylePatch::default()
    };
    workbook.set_style(0, (at(2, 2), at(2, 2)), &bold).unwrap();
    workbook.merge_cells(0, (at(2, 0), at(4, 1))).unwrap();
    workbook.set_row_height(0, 3, Some(25.0)).unwrap();
    workbook.set_column_width(0, 2, Some(20.0)).unwrap();

    workbook.insert_rows(0, 0, 2).unwrap();
    assert_eq!(workbook.style_id(0, at(4, 2)), 1);
    assert_eq!(workbook.merges(0)[0].first_row, 4);
    assert_eq!(workbook.sheets()[0].row_heights.get(&5), Some(&25.0));

    workbook.insert_columns(0, 1, 1).unwrap();
    assert_eq!(workbook.style_id(0, at(4, 3)), 1);
    assert_eq!(
        (
            workbook.merges(0)[0].first_column,
            workbook.merges(0)[0].last_column
        ),
        (0, 2),
        "an insertion inside a merge widens it"
    );
    assert_eq!(workbook.sheets()[0].column_widths.get(&3), Some(&20.0));

    workbook.delete_rows(0, 5, 2).unwrap();
    assert_eq!(
        (
            workbook.merges(0)[0].first_row,
            workbook.merges(0)[0].last_row
        ),
        (4, 4)
    );
    workbook.delete_columns(0, 1, 2).unwrap();
    assert_eq!(workbook.merges(0), [], "a merge left one cell ends");
    assert_eq!(workbook.style_id(0, at(4, 1)), 1);

    let mut copied = Workbook::new();
    copied.set_style(0, (at(0, 0), at(0, 0)), &bold).unwrap();
    copied.set_style(0, (at(5, 6), at(5, 6)), &bold).unwrap();
    copied.set_input(0, at(0, 0), "x", IT).unwrap();
    copied
        .copy_range(0, (at(0, 0), at(0, 1)), 0, at(5, 5))
        .unwrap();
    assert_eq!(
        copied.style_id(0, at(5, 5)),
        1,
        "a paste brings the look along"
    );
    assert_eq!(
        copied.style_id(0, at(5, 6)),
        0,
        "and clears the look it covers"
    );
}

#[test]
fn shows_the_format_set_on_a_cell_over_the_style_code() {
    let mut workbook = Workbook::new();
    workbook
        .set_style(
            0,
            (at(0, 0), at(0, 0)),
            &StylePatch {
                number_format: Some("0.0%".to_owned()),
                fill: Some("FF0000".to_owned()),
                ..StylePatch::default()
            },
        )
        .unwrap();
    workbook.set_input(0, at(0, 0), "0,125", IT).unwrap();
    assert_eq!(workbook.display(0, at(0, 0), IT), "12,5%");
    workbook
        .set_format(0, at(0, 0), NumberFormat::Decimals(2))
        .unwrap();
    assert_eq!(workbook.display(0, at(0, 0), IT), "0,13");
    assert_eq!(workbook.style(0, at(0, 0)).number_format, None);
    assert_eq!(workbook.style(0, at(0, 0)).fill.as_deref(), Some("FF0000"));
}
