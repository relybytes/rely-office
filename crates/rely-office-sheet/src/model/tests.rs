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

#[test]
fn names_follow_the_edits_of_rows_columns_and_sheets() {
    let mut workbook = Workbook::new();
    workbook
        .set_input(0, CellRef::parse("B2").unwrap(), "5", Locale::English)
        .unwrap();
    workbook
        .set_defined_name("Target", None, "B2", Locale::English)
        .unwrap();
    workbook
        .set_input(
            0,
            CellRef::parse("D1").unwrap(),
            "=Target*2",
            Locale::English,
        )
        .unwrap();
    let expression = |workbook: &Workbook| {
        crate::formula::print_formula(&workbook.defined_names()[0].expression)
    };
    assert_eq!(expression(&workbook), "Foglio1!B2");
    workbook.insert_rows(0, 0, 2).unwrap();
    assert_eq!(expression(&workbook), "Foglio1!B4");
    assert_eq!(
        workbook.value(0, CellRef::parse("D3").unwrap()),
        Value::Number(10.0)
    );
    workbook.insert_columns(0, 0, 1).unwrap();
    assert_eq!(expression(&workbook), "Foglio1!C4");
    workbook.rename_sheet(0, "Dati").unwrap();
    assert_eq!(expression(&workbook), "Dati!C4");
    workbook.delete_rows(0, 3, 1).unwrap();
    assert_eq!(expression(&workbook), "#REF!");
}

#[test]
fn removing_a_sheet_removes_its_names_and_breaks_the_references() {
    let mut workbook = Workbook::new();
    workbook.add_sheet("Altro").unwrap();
    workbook.add_sheet("Terzo").unwrap();
    workbook
        .set_defined_name("Local", Some(1), "1", Locale::English)
        .unwrap();
    workbook
        .set_defined_name("Later", Some(2), "2", Locale::English)
        .unwrap();
    workbook
        .set_defined_name("There", None, "Altro!A1", Locale::English)
        .unwrap();
    workbook.remove_sheet(1).unwrap();
    let names: Vec<(&str, Option<usize>, String)> = workbook
        .defined_names()
        .iter()
        .map(|defined| {
            (
                defined.name.as_str(),
                defined.sheet,
                crate::formula::print_formula(&defined.expression),
            )
        })
        .collect();
    assert_eq!(
        names,
        vec![
            ("Later", Some(1), "2".to_owned()),
            ("There", None, "#REF!".to_owned())
        ]
    );
}

#[test]
fn refuses_a_name_excel_refuses() {
    let mut workbook = Workbook::new();
    for name in [
        "",
        "A1",
        "r1c1",
        "R",
        "TRUE",
        "1abc",
        "a b",
        "x-y",
        &"n".repeat(256),
    ] {
        assert_eq!(
            workbook.set_defined_name(name, None, "1", Locale::English),
            Err(EditError::InvalidName),
            "{name}"
        );
    }
    for name in ["Rate", "_hidden", r"\path", "Tax.2026", "XFD1048577A"] {
        assert!(
            workbook
                .set_defined_name(name, None, "1", Locale::English)
                .is_ok(),
            "{name}"
        );
    }
    assert_eq!(
        workbook.set_defined_name("Rate", Some(4), "1", Locale::English),
        Err(EditError::NoSuchSheet(4))
    );
    assert!(matches!(
        workbook.set_defined_name("Rate", None, "=1+", Locale::English),
        Err(EditError::Formula(_))
    ));
    // An Italian formula is read in Italian.
    workbook
        .set_defined_name("Somma", None, "=SOMMA(1;2)", Locale::Italian)
        .unwrap();
    workbook
        .set_input(0, CellRef::parse("A1").unwrap(), "=Somma", Locale::English)
        .unwrap();
    assert_eq!(
        workbook.value(0, CellRef::parse("A1").unwrap()),
        Value::Number(3.0)
    );
}

#[test]
fn comments_are_written_moved_and_removed() {
    let mut workbook = Workbook::new();
    let b2 = CellRef::parse("B2").unwrap();
    workbook
        .set_comment(0, b2, " Anna ", "Da verificare\r\nentro venerdi")
        .unwrap();
    assert_eq!(
        workbook.sheets()[0].comment(b2),
        Some(&Comment {
            author: "Anna".to_owned(),
            text: "Da verificare\nentro venerdi".to_owned(),
            ..Comment::default()
        })
    );
    workbook.insert_rows(0, 0, 1).unwrap();
    let b3 = CellRef::parse("B3").unwrap();
    assert!(workbook.sheets()[0].comment(b2).is_none());
    assert!(workbook.sheets()[0].comment(b3).is_some());
    workbook.delete_rows(0, 2, 1).unwrap();
    assert!(workbook.sheets()[0].comments().is_empty());
    workbook.set_comment(0, b2, "", "x").unwrap();
    workbook.set_comment(0, b2, "", "  ").unwrap();
    assert!(workbook.sheets()[0].comments().is_empty());
    assert_eq!(
        workbook.set_comment(3, b2, "", "x"),
        Err(EditError::NoSuchSheet(3))
    );
    assert_eq!(
        workbook.set_comment(0, b2, "", &"x".repeat(MAX_COMMENT_CHARS + 1)),
        Err(EditError::OutOfBounds)
    );
    assert_eq!(
        workbook.set_comment(0, CellRef::new(MAX_ROWS, 0), "", "x"),
        Err(EditError::OutOfBounds)
    );
}

#[test]
fn a_thread_is_opened_answered_resolved_and_trimmed() {
    let mut workbook = Workbook::new();
    let b2 = CellRef::parse("B2").unwrap();
    workbook
        .start_comment_thread(0, b2, " Anna ", "Prezzo?", Some("2026-10-30T09:00:00.000Z"))
        .unwrap();
    workbook
        .reply_to_comment(0, b2, "Bruno", "Confermato", Some("2026-10-30T09:05:00"))
        .unwrap();
    workbook
        .reply_to_comment(0, b2, "Carla", "Grazie", None)
        .unwrap();
    workbook.set_comment_resolved(0, b2, true).unwrap();
    workbook.remove_comment_reply(0, b2, 1).unwrap();
    let comment = workbook.sheets()[0].comment(b2).unwrap();
    assert_eq!(comment.author, "Anna");
    assert!(comment.threaded && comment.resolved);
    assert_eq!(comment.date.as_deref(), Some("2026-10-30T09:00:00.000Z"));
    assert_eq!(
        comment.replies,
        vec![CommentReply {
            author: "Bruno".to_owned(),
            text: "Confermato".to_owned(),
            date: Some("2026-10-30T09:05:00".to_owned()),
        }]
    );
    // Writing the text again keeps the thread and the author who opened it.
    workbook
        .set_comment(0, b2, "Dario", "Prezzo giusto?")
        .unwrap();
    let comment = workbook.sheets()[0].comment(b2).unwrap();
    assert_eq!(
        (comment.author.as_str(), comment.text.as_str()),
        ("Anna", "Prezzo giusto?")
    );
    assert_eq!(comment.replies.len(), 1);
    // A note answered becomes a thread.
    let c3 = CellRef::parse("C3").unwrap();
    workbook.set_comment(0, c3, "Eva", "Nota").unwrap();
    assert!(!workbook.sheets()[0].comment(c3).unwrap().threaded);
    workbook
        .reply_to_comment(0, c3, "Eva", "Seguito", None)
        .unwrap();
    assert!(workbook.sheets()[0].comment(c3).unwrap().threaded);
    workbook.set_comment_resolved(0, c3, false).unwrap();
    // A thread moves with its cell and is removed as a note is.
    workbook.insert_rows(0, 0, 1).unwrap();
    assert!(
        workbook.sheets()[0]
            .comment(CellRef::parse("B3").unwrap())
            .is_some()
    );
    workbook
        .set_comment(0, CellRef::parse("B3").unwrap(), "", "")
        .unwrap();
    assert!(
        workbook.sheets()[0]
            .comment(CellRef::parse("B3").unwrap())
            .is_none()
    );
}

#[test]
fn a_thread_refuses_what_it_cannot_keep() {
    let mut workbook = Workbook::new();
    let a1 = CellRef::new(0, 0);
    assert_eq!(
        workbook.reply_to_comment(0, a1, "Anna", "x", None),
        Err(EditError::InvalidComment)
    );
    assert_eq!(
        workbook.set_comment_resolved(0, a1, true),
        Err(EditError::InvalidComment)
    );
    assert_eq!(
        workbook.remove_comment_reply(0, a1, 0),
        Err(EditError::InvalidComment)
    );
    assert_eq!(
        workbook.start_comment_thread(0, a1, "Anna", " ", None),
        Err(EditError::InvalidComment)
    );
    for date in [
        "30/10/2026",
        "2026-10-30",
        "2026-10-30T09:00:00 ora",
        "2026-1x-30T09:00:00",
    ] {
        assert_eq!(
            workbook.start_comment_thread(0, a1, "Anna", "x", Some(date)),
            Err(EditError::InvalidComment),
            "{date}"
        );
    }
    assert_eq!(
        workbook.start_comment_thread(0, CellRef::new(MAX_ROWS, 0), "Anna", "x", None),
        Err(EditError::OutOfBounds)
    );
    assert_eq!(
        workbook.start_comment_thread(0, a1, "Anna", &"x".repeat(MAX_COMMENT_CHARS + 1), None),
        Err(EditError::OutOfBounds)
    );
    assert_eq!(
        workbook.start_comment_thread(2, a1, "Anna", "x", None),
        Err(EditError::NoSuchSheet(2))
    );
    assert_eq!(
        workbook.reply_to_comment(2, a1, "Anna", "x", None),
        Err(EditError::NoSuchSheet(2))
    );
    workbook
        .start_comment_thread(0, a1, "Anna", "x", None)
        .unwrap();
    assert_eq!(
        workbook.remove_comment_reply(0, a1, 0),
        Err(EditError::InvalidComment)
    );
    for _ in 0..MAX_COMMENT_REPLIES {
        workbook
            .reply_to_comment(0, a1, "Bruno", "y", None)
            .unwrap();
    }
    assert_eq!(
        workbook.reply_to_comment(0, a1, "Bruno", "y", None),
        Err(EditError::InvalidComment)
    );
}
