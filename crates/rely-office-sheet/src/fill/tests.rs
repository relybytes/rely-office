// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use super::{split_counter, trend};
use crate::address::CellRef;
use crate::model::{EditError, NumberFormat, Workbook};
use crate::styling::StylePatch;
use crate::value::{Locale, Value};

fn cell(address: &str) -> CellRef {
    CellRef::parse(address).unwrap()
}

fn book(inputs: &[(&str, &str)]) -> Workbook {
    let mut workbook = Workbook::new();
    let inputs: Vec<(CellRef, String)> = inputs
        .iter()
        .map(|(address, typed)| (cell(address), (*typed).to_owned()))
        .collect();
    workbook.set_inputs(0, &inputs, Locale::English).unwrap();
    workbook
}

fn fill(workbook: &mut Workbook, source: &str, target: &str) {
    let corners = |range: &str| {
        let (start, end) = range.split_once(':').unwrap();
        (cell(start), cell(end))
    };
    workbook
        .fill_range(0, corners(source), corners(target))
        .unwrap();
}

fn shown(workbook: &Workbook, addresses: &[&str]) -> Vec<String> {
    addresses
        .iter()
        .map(|address| workbook.display(0, cell(address), Locale::English))
        .collect()
}

#[test]
fn finds_the_trend_and_the_counter() {
    assert_eq!(trend(&[1.0, 3.0]), (1.0, 2.0));
    let (start, step) = trend(&[1.0, 2.0, 4.0]);
    assert!((start - 0.833_333_333_333_333_4).abs() < 1e-12);
    assert!((step - 1.5).abs() < 1e-12);
    assert_eq!(
        split_counter("Lotto 07 bis"),
        Some(("Lotto ".to_owned(), 7, " bis".to_owned(), 2))
    );
    assert_eq!(split_counter("senza"), None);
}

#[test]
fn continues_numbers_along_their_trend() {
    let mut workbook = book(&[("A1", "1"), ("A2", "3")]);
    fill(&mut workbook, "A1:A2", "A1:A5");
    assert_eq!(shown(&workbook, &["A3", "A4", "A5"]), ["5", "7", "9"]);

    let mut workbook = book(&[("A1", "10"), ("B1", "20")]);
    fill(&mut workbook, "A1:B1", "A1:E1");
    assert_eq!(shown(&workbook, &["C1", "D1", "E1"]), ["30", "40", "50"]);

    let mut workbook = book(&[("A4", "10"), ("A5", "20")]);
    fill(&mut workbook, "A4:A5", "A1:A5");
    assert_eq!(shown(&workbook, &["A1", "A2", "A3"]), ["-20", "-10", "0"]);
}

#[test]
fn repeats_a_single_number_and_counts_days_from_a_date() {
    let mut workbook = book(&[("A1", "7")]);
    fill(&mut workbook, "A1:A1", "A1:A3");
    assert_eq!(shown(&workbook, &["A2", "A3"]), ["7", "7"]);

    let mut workbook = book(&[("A1", "46000")]);
    workbook
        .set_format(0, cell("A1"), NumberFormat::Date)
        .unwrap();
    fill(&mut workbook, "A1:A1", "A1:A3");
    assert_eq!(workbook.value(0, cell("A3")), Value::Number(46_002.0));
    assert_eq!(
        workbook.sheets()[0].cell(cell("A3")).unwrap().format,
        NumberFormat::Date
    );
}

#[test]
fn counts_on_a_text_ending_in_a_number() {
    let mut workbook = book(&[("A1", "Lotto 08")]);
    fill(&mut workbook, "A1:A1", "A1:A3");
    assert_eq!(shown(&workbook, &["A2", "A3"]), ["Lotto 09", "Lotto 10"]);

    let mut workbook = book(&[("A1", "Q1"), ("A2", "Q3")]);
    fill(&mut workbook, "A1:A2", "A1:A4");
    assert_eq!(shown(&workbook, &["A3", "A4"]), ["Q5", "Q7"]);
}

#[test]
fn follows_months_and_weekdays_in_their_case() {
    let mut workbook = book(&[("A1", "Gennaio")]);
    fill(&mut workbook, "A1:A1", "A1:A3");
    assert_eq!(shown(&workbook, &["A2", "A3"]), ["Febbraio", "Marzo"]);

    let mut workbook = book(&[("A1", "NOV")]);
    fill(&mut workbook, "A1:A1", "A1:A4");
    assert_eq!(shown(&workbook, &["A2", "A3", "A4"]), ["DIC", "GEN", "FEB"]);

    let mut workbook = book(&[("A1", "monday"), ("B1", "wednesday")]);
    fill(&mut workbook, "A1:B1", "A1:D1");
    assert_eq!(shown(&workbook, &["C1", "D1"]), ["friday", "sunday"]);
}

#[test]
fn repeats_plain_text_and_moves_formulas() {
    let mut workbook = book(&[
        ("A1", "si"),
        ("A2", "no"),
        ("B1", "1"),
        ("B2", "2"),
        ("B3", "3"),
        ("C1", "=B1*10"),
    ]);
    fill(&mut workbook, "A1:A2", "A1:A5");
    assert_eq!(shown(&workbook, &["A3", "A4", "A5"]), ["si", "no", "si"]);
    fill(&mut workbook, "C1:C1", "C1:C3");
    assert_eq!(shown(&workbook, &["C2", "C3"]), ["20", "30"]);
    assert_eq!(
        workbook.sheets()[0]
            .cell(cell("C3"))
            .unwrap()
            .input
            .text_in(Locale::English),
        "=B3*10"
    );
}

#[test]
fn carries_the_looks_and_empties_what_the_source_leaves_empty() {
    let mut workbook = book(&[("A1", "x"), ("A3", "old"), ("A4", "old")]);
    workbook
        .set_style(
            0,
            (cell("A1"), cell("A1")),
            &StylePatch {
                bold: Some(true),
                ..StylePatch::default()
            },
        )
        .unwrap();
    fill(&mut workbook, "A1:A2", "A1:A4");
    assert_eq!(shown(&workbook, &["A3", "A4"]), ["x", ""]);
    assert!(workbook.style(0, cell("A3")).font.bold);
    assert!(!workbook.style(0, cell("A4")).font.bold);
}

#[test]
fn refuses_a_target_that_is_not_a_continuation() {
    let mut workbook = book(&[("B2", "1")]);
    let refused = |workbook: &mut Workbook, source: (&str, &str), target: (&str, &str)| {
        workbook.fill_range(
            0,
            (cell(source.0), cell(source.1)),
            (cell(target.0), cell(target.1)),
        )
    };
    assert_eq!(
        refused(&mut workbook, ("B2", "B2"), ("B3", "B5")),
        Err(EditError::OutOfBounds)
    );
    assert_eq!(
        refused(&mut workbook, ("B2", "B2"), ("A1", "C3")),
        Err(EditError::OutOfBounds)
    );
    assert_eq!(
        workbook.fill_range(4, (cell("B2"), cell("B2")), (cell("B2"), cell("B3"))),
        Err(EditError::NoSuchSheet(4))
    );
    let before = workbook.clone();
    refused(&mut workbook, ("B2", "B2"), ("B2", "B2")).unwrap();
    assert_eq!(workbook, before);
}
