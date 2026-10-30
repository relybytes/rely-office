// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use rely_office_core::Limits;

use super::*;
use crate::xlsx::read_spreadsheet;

fn cell(address: &str) -> CellRef {
    CellRef::parse(address).unwrap()
}

fn book(inputs: &[(&str, &str)], locale: Locale) -> Workbook {
    let mut workbook = Workbook::new();
    let inputs: Vec<(CellRef, String)> = inputs
        .iter()
        .map(|(address, typed)| (cell(address), (*typed).to_owned()))
        .collect();
    workbook.set_inputs(0, &inputs, locale).unwrap();
    workbook
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[test]
fn writes_what_the_cells_show_in_the_locale() {
    let workbook = book(
        &[
            ("A1", "Nome"),
            ("B1", "Importo"),
            ("A2", "Rossi; Bianchi"),
            ("B2", "1234,5"),
            ("A3", "Detto \"così\""),
            ("B3", "=B2*2"),
            ("C4", "fine"),
        ],
        Locale::Italian,
    );
    let written = text(&write_csv(&workbook, 0, Locale::Italian, ';').unwrap());
    assert_eq!(
        written,
        "\u{feff}Nome;Importo\r\n\"Rossi; Bianchi\";1234,5\r\n\"Detto \"\"così\"\"\";2469\r\n;;fine\r\n"
    );
    let english = text(&write_csv(&workbook, 0, Locale::English, ',').unwrap());
    assert!(english.contains("Rossi; Bianchi,1234.5\r\n"), "{english}");
}

#[test]
fn writes_the_cells_an_array_spills_into() {
    let workbook = book(&[("A1", "=SEQUENCE(3)")], Locale::English);
    assert_eq!(
        text(&write_csv(&workbook, 0, Locale::English, ',').unwrap()),
        "\u{feff}1\r\n2\r\n3\r\n"
    );
}

#[test]
fn writes_an_empty_sheet_and_refuses_one_it_does_not_have() {
    let workbook = Workbook::new();
    assert_eq!(
        write_csv(&workbook, 0, Locale::Italian, ';').unwrap(),
        "\u{feff}".as_bytes()
    );
    assert!(matches!(
        write_csv(&workbook, 3, Locale::Italian, ';'),
        Err(EditError::NoSuchSheet(3))
    ));
    assert_eq!(default_separator(Locale::Italian), ';');
    assert_eq!(default_separator(Locale::English), ',');
}

#[test]
fn reads_back_what_it_wrote_line_breaks_inside_a_cell_included() {
    let workbook = book(
        &[
            ("A1", "Riga uno\nriga due"),
            ("B1", "12,5"),
            ("A2", " spazio"),
        ],
        Locale::Italian,
    );
    let written = write_csv(&workbook, 0, Locale::Italian, ';').unwrap();
    let read =
        read_spreadsheet(&written, Some("a.csv"), &Limits::default(), Locale::Italian).unwrap();
    assert_eq!(
        read.display(0, cell("A1"), Locale::Italian),
        "Riga uno\nriga due"
    );
    assert_eq!(read.display(0, cell("B1"), Locale::Italian), "12,5");
    assert_eq!(read.display(0, cell("A2"), Locale::Italian), " spazio");
    assert_eq!(read.delimiter(), Some(';'));
}
