// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use std::collections::BTreeSet;

use rely_office_core::{CellStyle, Limits};

use super::{SortKey, compare_values};
use crate::address::CellRef;
use crate::model::{EditError, Workbook};
use crate::styling::StylePatch;
use crate::value::{ErrorKind, Locale, Value};
use crate::xlsx::{read_spreadsheet, write_xlsx};

fn cell(address: &str) -> CellRef {
    CellRef::parse(address).unwrap()
}

/// A heading on row 1, then names, regions and amounts, and a total formula beside each row.
fn people() -> Workbook {
    let mut workbook = Workbook::new();
    let rows = [
        ("A1", "Nome"),
        ("B1", "Zona"),
        ("C1", "Importo"),
        ("A2", "Verdi"),
        ("B2", "Nord"),
        ("C2", "30"),
        ("A3", "bianchi"),
        ("B3", "Sud"),
        ("C3", "10"),
        ("A4", "Rossi"),
        ("B4", "Nord"),
        ("C4", "20"),
        ("A5", "Neri"),
        ("B5", "Sud"),
        ("A6", "Gialli"),
        ("B6", "Centro"),
        ("C6", "20"),
    ];
    let inputs: Vec<(CellRef, String)> = rows
        .iter()
        .map(|(address, typed)| (cell(address), (*typed).to_owned()))
        .collect();
    workbook.set_inputs(0, &inputs, Locale::English).unwrap();
    for row in 2..=6 {
        workbook
            .set_input(
                0,
                cell(&format!("D{row}")),
                &format!("=C{row}*2"),
                Locale::English,
            )
            .unwrap();
    }
    workbook
}

fn column(workbook: &Workbook, letter: char) -> Vec<String> {
    (2..=6)
        .map(|row| workbook.display(0, cell(&format!("{letter}{row}")), Locale::English))
        .collect()
}

#[test]
fn orders_values_as_a_spreadsheet_does() {
    use std::cmp::Ordering;
    assert_eq!(
        compare_values(&Value::Number(9.0), &Value::Text("a".to_owned())),
        Ordering::Less
    );
    assert_eq!(
        compare_values(&Value::Text("b".to_owned()), &Value::Text("A".to_owned())),
        Ordering::Greater
    );
    assert_eq!(
        compare_values(&Value::Text("z".to_owned()), &Value::Bool(false)),
        Ordering::Less
    );
    assert_eq!(
        compare_values(&Value::Bool(true), &Value::Error(ErrorKind::Div0)),
        Ordering::Less
    );
}

#[test]
fn sorts_rows_by_a_column_with_their_formulas_and_looks() {
    let mut workbook = people();
    workbook
        .set_style(
            0,
            (cell("A2"), cell("A2")),
            &StylePatch {
                bold: Some(true),
                ..StylePatch::default()
            },
        )
        .unwrap();

    workbook
        .sort_range(
            0,
            (cell("A1"), cell("D6")),
            &[SortKey {
                column: 0,
                descending: false,
            }],
            true,
        )
        .unwrap();

    assert_eq!(
        column(&workbook, 'A'),
        ["bianchi", "Gialli", "Neri", "Rossi", "Verdi"]
    );
    assert_eq!(column(&workbook, 'D'), ["20", "40", "0", "40", "60"]);
    assert_eq!(
        workbook.sheets()[0]
            .cell(cell("D6"))
            .unwrap()
            .input
            .text_in(Locale::English),
        "=C6*2",
        "a moved formula points at its own row"
    );
    assert!(workbook.style(0, cell("A6")).font.bold);
    assert_eq!(workbook.style(0, cell("A2")), &CellStyle::default());
    assert_eq!(workbook.display(0, cell("A1"), Locale::English), "Nome");
}

#[test]
fn sorts_by_several_keys_and_keeps_blanks_last() {
    let mut workbook = people();
    workbook
        .sort_range(
            0,
            (cell("A2"), cell("D6")),
            &[
                SortKey {
                    column: 2,
                    descending: true,
                },
                SortKey {
                    column: 0,
                    descending: false,
                },
            ],
            false,
        )
        .unwrap();

    assert_eq!(
        column(&workbook, 'A'),
        ["Verdi", "Gialli", "Rossi", "bianchi", "Neri"]
    );
}

#[test]
fn refuses_a_sort_it_cannot_do() {
    let mut workbook = people();
    let outside = SortKey {
        column: 7,
        descending: false,
    };
    assert_eq!(
        workbook.sort_range(0, (cell("A1"), cell("D6")), &[outside], true),
        Err(EditError::OutOfBounds)
    );
    assert_eq!(
        workbook.sort_range(0, (cell("A1"), cell("D6")), &[], true),
        Err(EditError::OutOfBounds)
    );
    assert_eq!(
        workbook.sort_range(3, (cell("A1"), cell("D6")), &[outside], true),
        Err(EditError::NoSuchSheet(3))
    );
    let before = workbook.clone();
    workbook
        .sort_range(
            0,
            (cell("A1"), cell("D1")),
            &[SortKey {
                column: 0,
                descending: false,
            }],
            true,
        )
        .unwrap();
    assert_eq!(workbook, before, "a heading alone has nothing to sort");
}

#[test]
fn filters_rows_and_shows_them_again() {
    let mut workbook = people();
    workbook.set_row_height(0, 3, Some(30.0)).unwrap();
    workbook
        .set_auto_filter(0, Some((cell("A1"), cell("D6"))))
        .unwrap();

    let choices = workbook.filter_choices(0, 1, Locale::Italian).unwrap();
    assert_eq!(
        choices,
        [
            ("Centro".to_owned(), "Centro".to_owned()),
            ("Nord".to_owned(), "Nord".to_owned()),
            ("Sud".to_owned(), "Sud".to_owned()),
        ]
    );

    workbook
        .set_filter_column(0, 1, Some(vec!["Nord".to_owned()]))
        .unwrap();
    let sheet = &workbook.sheets()[0];
    let hidden: Vec<u32> = sheet.auto_filter().unwrap().hidden_rows().collect();
    assert_eq!(hidden, [2, 4, 5]);
    assert_eq!(sheet.row_heights.get(&2), Some(&0.0));

    workbook
        .set_filter_column(0, 2, Some(vec!["20".to_owned()]))
        .unwrap();
    let hidden: Vec<u32> = workbook.sheets()[0]
        .auto_filter()
        .unwrap()
        .hidden_rows()
        .collect();
    assert_eq!(hidden, [1, 2, 4, 5]);

    workbook.set_auto_filter(0, None).unwrap();
    let sheet = &workbook.sheets()[0];
    assert!(sheet.auto_filter().is_none());
    assert_eq!(
        sheet.row_heights.get(&3),
        Some(&30.0),
        "a height of its own comes back"
    );
    assert!(!sheet.row_heights.contains_key(&2));
}

#[test]
fn offers_the_blank_cells_and_numbers_as_the_locale_shows_them() {
    let mut workbook = people();
    workbook
        .set_input(0, cell("C3"), "1234.5", Locale::English)
        .unwrap();
    workbook
        .set_auto_filter(0, Some((cell("A1"), cell("C6"))))
        .unwrap();

    let choices = workbook.filter_choices(0, 2, Locale::Italian).unwrap();
    assert_eq!(choices.first(), Some(&("20".to_owned(), "20".to_owned())));
    assert!(choices.contains(&("1234.5".to_owned(), "1234,5".to_owned())));
    assert_eq!(choices.last(), Some(&(String::new(), String::new())));

    workbook
        .set_filter_column(0, 2, Some(vec![String::new()]))
        .unwrap();
    let hidden: Vec<u32> = workbook.sheets()[0]
        .auto_filter()
        .unwrap()
        .hidden_rows()
        .collect();
    assert_eq!(hidden, [1, 2, 3, 5]);
}

#[test]
fn refuses_a_filter_column_outside_the_filter() {
    let mut workbook = people();
    assert_eq!(
        workbook.set_filter_column(0, 0, None),
        Err(EditError::OutOfBounds)
    );
    workbook
        .set_auto_filter(0, Some((cell("A1"), cell("B6"))))
        .unwrap();
    assert_eq!(
        workbook.set_filter_column(0, 2, None),
        Err(EditError::OutOfBounds)
    );
    assert_eq!(
        workbook.filter_choices(0, 2, Locale::English),
        Err(EditError::OutOfBounds)
    );
}

#[test]
fn moves_the_filter_with_its_rows_and_columns() {
    let mut workbook = people();
    workbook
        .set_auto_filter(0, Some((cell("A1"), cell("D6"))))
        .unwrap();
    workbook
        .set_filter_column(0, 1, Some(vec!["Sud".to_owned()]))
        .unwrap();

    workbook.insert_rows(0, 0, 1).unwrap();
    workbook.insert_columns(0, 0, 2).unwrap();

    let filter = workbook.sheets()[0].auto_filter().unwrap().clone();
    assert_eq!(filter.range.first_row, 1);
    assert_eq!(filter.range.first_column, 2);
    assert_eq!(filter.range.last_column, 5);
    assert!(filter.shown(1).is_some_and(|shown| shown.contains("Sud")));
    let hidden: Vec<u32> = filter.hidden_rows().collect();
    assert_eq!(hidden, [2, 4, 6]);

    workbook.delete_columns(0, 2, 4).unwrap();
    assert!(workbook.sheets()[0].auto_filter().is_none());
}

#[test]
fn freezes_rows_and_columns() {
    let mut workbook = people();
    workbook.set_frozen(0, 1, 2).unwrap();
    assert_eq!(workbook.sheets()[0].frozen_rows, 1);
    assert_eq!(workbook.sheets()[0].frozen_columns, 2);
    assert_eq!(
        workbook.set_frozen(0, crate::address::MAX_ROWS, 0),
        Err(EditError::OutOfBounds)
    );
    assert_eq!(workbook.set_frozen(1, 1, 0), Err(EditError::NoSuchSheet(1)));
}

#[test]
fn writes_and_reads_back_frozen_panes_and_the_filter() {
    let mut workbook = people();
    workbook.set_frozen(0, 1, 1).unwrap();
    workbook
        .set_auto_filter(0, Some((cell("A1"), cell("D6"))))
        .unwrap();
    workbook
        .set_filter_column(0, 1, Some(vec!["Nord".to_owned(), String::new()]))
        .unwrap();

    let bytes = write_xlsx(&workbook).unwrap();
    let read =
        read_spreadsheet(&bytes, Some("a.xlsx"), &Limits::default(), Locale::English).unwrap();

    let sheet = &read.sheets()[0];
    assert_eq!((sheet.frozen_rows, sheet.frozen_columns), (1, 1));
    let filter = sheet.auto_filter().unwrap();
    assert_eq!(
        filter.range,
        workbook.sheets()[0].auto_filter().unwrap().range
    );
    assert_eq!(
        filter.shown(1),
        Some(&BTreeSet::from(["Nord".to_owned(), String::new()]))
    );
    let hidden: Vec<u32> = filter.hidden_rows().collect();
    assert_eq!(hidden, [2, 4, 5]);

    let mut read = read;
    read.set_auto_filter(0, None).unwrap();
    assert!(!read.sheets()[0].row_heights.contains_key(&2));
}

#[test]
fn writes_no_view_or_filter_a_sheet_does_not_have() {
    let bytes = write_xlsx(&people()).unwrap();
    let read =
        read_spreadsheet(&bytes, Some("a.xlsx"), &Limits::default(), Locale::English).unwrap();
    let sheet = &read.sheets()[0];
    assert_eq!((sheet.frozen_rows, sheet.frozen_columns), (0, 0));
    assert!(sheet.auto_filter().is_none());
    assert_eq!(sheet.pictures(), &[]);
}

#[test]
fn writes_and_reads_back_frozen_panes_and_the_filter_in_ods() {
    let mut workbook = people();
    workbook.set_frozen(0, 1, 2).unwrap();
    workbook
        .set_auto_filter(0, Some((cell("A1"), cell("D6"))))
        .unwrap();
    workbook
        .set_filter_column(0, 1, Some(vec!["Nord".to_owned(), String::new()]))
        .unwrap();

    let bytes = crate::ods::write_ods(&workbook).unwrap();
    let read =
        read_spreadsheet(&bytes, Some("a.ods"), &Limits::default(), Locale::English).unwrap();

    let sheet = &read.sheets()[0];
    assert_eq!((sheet.frozen_rows, sheet.frozen_columns), (1, 2));
    let filter = sheet.auto_filter().unwrap();
    assert_eq!(
        filter.range,
        workbook.sheets()[0].auto_filter().unwrap().range
    );
    assert_eq!(
        filter.shown(1),
        Some(&BTreeSet::from(["Nord".to_owned(), String::new()]))
    );
    let hidden: Vec<u32> = filter.hidden_rows().collect();
    assert_eq!(hidden, [2, 4, 5]);
}

#[test]
fn draws_a_chart_again_from_the_values_the_cells_hold_now() {
    use rust_xlsxwriter::{Chart, ChartType, Workbook as Writer};
    let mut writer = Writer::new();
    let sheet = writer.add_worksheet().set_name("Vendite").unwrap();
    for (row, (label, value)) in [("Gennaio", 10), ("Febbraio", 20)].into_iter().enumerate() {
        let row = u32::try_from(row).unwrap();
        sheet.write(row, 0, label).unwrap();
        sheet.write(row, 1, value).unwrap();
    }
    let mut chart = Chart::new(ChartType::Column);
    chart
        .add_series()
        .set_categories("Vendite!$A$1:$A$2")
        .set_values("Vendite!$B$1:$B$2");
    sheet.insert_chart(4, 2, &chart).unwrap();
    let bytes = writer.save_to_buffer().unwrap();

    let mut workbook =
        read_spreadsheet(&bytes, Some("a.xlsx"), &Limits::default(), Locale::English).unwrap();
    let svg = |workbook: &Workbook| {
        let pictures = workbook.pictures(0);
        assert_eq!(pictures.len(), 1);
        assert_eq!(pictures[0].content_type, "image/svg+xml");
        String::from_utf8(pictures[0].bytes.clone()).unwrap()
    };
    assert!(svg(&workbook).contains(">Gennaio</text>"));
    workbook
        .set_input(0, cell("A1"), "Marzo", Locale::English)
        .unwrap();
    let redrawn = svg(&workbook);
    assert!(redrawn.contains(">Marzo</text>"), "{redrawn}");
    assert!(!redrawn.contains(">Gennaio</text>"));
}
