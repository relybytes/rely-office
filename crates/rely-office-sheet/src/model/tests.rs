// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;
use crate::value::ErrorKind;

fn cell(address: &str) -> CellRef {
    CellRef::parse(address).unwrap()
}

fn typed(book: &mut Workbook, sheet: usize, address: &str, text: &str) {
    book.set_input(sheet, cell(address), text, Locale::English)
        .unwrap();
}

fn formula(book: &Workbook, sheet: usize, address: &str) -> String {
    book.sheets[sheet].cells[&cell(address)].input.text()
}

#[test]
fn starts_with_one_sheet_and_edits_cells() {
    let mut book = Workbook::new();
    assert_eq!(book.sheets().len(), 1);
    assert_eq!(book.sheets()[0].name, "Foglio1");
    typed(&mut book, 0, "B2", "12.5");
    typed(&mut book, 0, "C3", "=B2*2");
    assert_eq!(book.sheets()[0].extent(), (3, 3));
    assert_eq!(book.display(0, cell("C3"), Locale::Italian), "25");
    typed(&mut book, 0, "B2", "");
    assert_eq!(book.value(0, cell("B2")), Value::Empty);
    assert_eq!(book.value(0, cell("C3")), Value::Number(0.0));
    assert!(book.formula(0, cell("C3")).is_some());
    assert!(book.formula(0, cell("B2")).is_none());
    assert!(matches!(
        book.set_input(0, cell("A1"), "=SUM(", Locale::English),
        Err(EditError::Formula(_))
    ));
    assert_eq!(
        book.set_input(4, cell("A1"), "1", Locale::English),
        Err(EditError::NoSuchSheet(4))
    );
    assert_eq!(
        book.set_input(0, CellRef::new(MAX_ROWS, 0), "1", Locale::English),
        Err(EditError::OutOfBounds)
    );
}

#[test]
fn keeps_a_paste_whole_or_not_at_all() {
    let mut book = Workbook::new();
    let refused = book.set_inputs(
        0,
        &[(cell("A1"), "1".to_owned()), (cell("A2"), "=(".to_owned())],
        Locale::English,
    );
    assert!(refused.is_err());
    assert_eq!(book.value(0, cell("A1")), Value::Empty);
}

#[test]
fn formats_numbers_and_sizes_columns() {
    let mut book = Workbook::new();
    typed(&mut book, 0, "A1", "0.25");
    book.set_format(0, cell("A1"), NumberFormat::Percent)
        .unwrap();
    book.set_format(0, cell("Z9"), NumberFormat::Date).unwrap();
    assert_eq!(book.display(0, cell("A1"), Locale::Italian), "25%");
    typed(&mut book, 0, "A1", "0.5");
    assert_eq!(
        book.display(0, cell("A1"), Locale::Italian),
        "50%",
        "the format stays"
    );
    book.set_format(0, cell("A1"), NumberFormat::Decimals(40))
        .unwrap();
    assert_eq!(
        book.sheets()[0].cell(cell("A1")).unwrap().format,
        NumberFormat::Decimals(10)
    );
    book.set_column_width(0, 2, Some(300.0)).unwrap();
    assert_eq!(book.sheets()[0].column_widths[&2], 255.0);
    book.set_column_width(0, 2, None).unwrap();
    assert!(book.sheets()[0].column_widths.is_empty());
    assert_eq!(
        book.set_column_width(0, MAX_COLUMNS, Some(1.0)),
        Err(EditError::OutOfBounds)
    );
}

#[test]
fn moves_formulas_with_inserted_and_deleted_rows() {
    let mut book = Workbook::new();
    typed(&mut book, 0, "A1", "1");
    typed(&mut book, 0, "A2", "2");
    typed(&mut book, 0, "A3", "3");
    typed(&mut book, 0, "B1", "=SUM(A1:A3)+A3+$A$2");
    book.insert_rows(0, 1, 2).unwrap();
    assert_eq!(formula(&book, 0, "B1"), "=SUM(A1:A5)+A5+$A$4");
    assert_eq!(book.value(0, cell("B1")), Value::Number(11.0));
    assert_eq!(book.value(0, cell("A5")), Value::Number(3.0));

    book.delete_rows(0, 4, 1).unwrap();
    assert_eq!(formula(&book, 0, "B1"), "=SUM(A1:A4)+#REF!+$A$4");
    assert_eq!(book.value(0, cell("B1")), Value::Error(ErrorKind::Ref));

    book.delete_rows(0, 0, 10).unwrap();
    assert!(book.sheets()[0].cells.is_empty());
}

#[test]
fn moves_formulas_with_columns_across_sheets() {
    let mut book = Workbook::new();
    let other = book.add_sheet("Dati").unwrap();
    typed(&mut book, other, "B1", "5");
    typed(&mut book, other, "C1", "7");
    typed(
        &mut book,
        0,
        "A1",
        "=Dati!B1+SUM(Dati!B1:C1)+SUM(Dati!B:C)+B1",
    );
    book.set_column_width(other, 2, Some(20.0)).unwrap();
    book.insert_columns(other, 0, 1).unwrap();
    assert_eq!(
        formula(&book, 0, "A1"),
        "=Dati!C1+SUM(Dati!C1:D1)+SUM(Dati!C:D)+B1"
    );
    assert_eq!(book.value(0, cell("A1")), Value::Number(29.0));
    assert_eq!(
        book.sheets()[other]
            .column_widths
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        vec![3]
    );
    book.delete_columns(other, 2, 2).unwrap();
    assert_eq!(formula(&book, 0, "A1"), "=#REF!+SUM(#REF!)+SUM(#REF!)+B1");
}

#[test]
fn refuses_to_push_content_off_the_grid() {
    let mut book = Workbook::new();
    typed(&mut book, 0, "XFD1", "1");
    assert_eq!(book.insert_columns(0, 0, 1), Err(EditError::OutOfBounds));
    assert_eq!(
        book.value(0, CellRef::new(0, MAX_COLUMNS - 1)),
        Value::Number(1.0)
    );
    assert_eq!(book.insert_rows(0, 0, 0), Ok(()));
}

#[test]
fn renames_and_removes_sheets() {
    let mut book = Workbook::new();
    let other = book.add_sheet("Dati").unwrap();
    typed(&mut book, other, "A1", "3");
    typed(&mut book, 0, "A1", "=dati!A1*2");
    book.rename_sheet(other, "Dati 2026").unwrap();
    assert_eq!(formula(&book, 0, "A1"), "='Dati 2026'!A1*2");
    assert_eq!(book.value(0, cell("A1")), Value::Number(6.0));
    assert_eq!(
        book.add_sheet("dati 2026"),
        Err(EditError::DuplicateSheetName("dati 2026".to_owned()))
    );
    for wrong in ["", "a/b", "[x]", "'q", &"x".repeat(32)] {
        assert_eq!(
            book.add_sheet(wrong),
            Err(EditError::InvalidSheetName),
            "{wrong}"
        );
    }
    book.rename_sheet(other, "DATI 2026").unwrap();
    book.remove_sheet(other).unwrap();
    assert_eq!(formula(&book, 0, "A1"), "=#REF!*2");
    assert_eq!(book.remove_sheet(0), Err(EditError::LastSheet));
    assert_eq!(book.remove_sheet(5), Err(EditError::NoSuchSheet(5)));
    assert_eq!(book.rename_sheet(5, "x"), Err(EditError::NoSuchSheet(5)));
}

#[test]
fn copies_with_relative_references() {
    let mut book = Workbook::new();
    typed(&mut book, 0, "A1", "1");
    typed(&mut book, 0, "A2", "2");
    typed(&mut book, 0, "B1", "=A1*$A$1+A$1");
    book.copy_range(0, (cell("B1"), cell("B1")), 0, cell("B2"))
        .unwrap();
    assert_eq!(formula(&book, 0, "B2"), "=A2*$A$1+A$1");
    assert_eq!(book.value(0, cell("B2")), Value::Number(3.0));
    book.copy_range(0, (cell("B1"), cell("B1")), 0, cell("A3"))
        .unwrap();
    assert_eq!(formula(&book, 0, "A3"), "=#REF!*$A$1+#REF!");

    typed(&mut book, 0, "D1", "x");
    book.copy_range(0, (cell("C1"), cell("C2")), 0, cell("D1"))
        .unwrap();
    assert_eq!(
        book.value(0, cell("D1")),
        Value::Empty,
        "an empty source empties its target"
    );
    book.clear_range(0, (cell("A1"), cell("B9"))).unwrap();
    assert_eq!(book.sheets()[0].cells.len(), 0);
    typed(&mut book, 0, "A2", "1");
    assert_eq!(
        book.copy_range(
            0,
            (cell("A2"), cell("A2")),
            0,
            CellRef::new(MAX_ROWS - 1, 0)
        ),
        Ok(())
    );
    assert_eq!(
        book.copy_range(
            0,
            (cell("A1"), cell("A3")),
            0,
            CellRef::new(MAX_ROWS - 1, 0)
        ),
        Err(EditError::OutOfBounds)
    );
    assert_eq!(
        book.copy_range(0, (cell("A1"), cell("A1")), 3, cell("A1")),
        Err(EditError::NoSuchSheet(3))
    );
}
