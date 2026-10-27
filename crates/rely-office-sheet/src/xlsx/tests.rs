// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;

fn cell(address: &str) -> CellRef {
    CellRef::parse(address).unwrap()
}

fn sample() -> Workbook {
    let mut book = Workbook::new();
    let inputs = [
        ("A1", "Voce"),
        ("B1", "Importo"),
        ("A2", "Canone <mensile> & \"altro\""),
        ("B2", "1850"),
        ("A3", "Ore"),
        ("B3", "65.5"),
        ("B4", "=SUM(B2:B3)"),
        ("C4", "=B4>1000"),
        ("D4", "=A2&\"!\""),
        ("E4", "=1/0"),
        ("F4", "=F4"),
        ("G4", "TRUE"),
        ("H4", "0.22"),
        ("I4", "46322"),
    ];
    let inputs: Vec<(CellRef, String)> = inputs
        .iter()
        .map(|(address, typed)| (cell(address), (*typed).to_owned()))
        .collect();
    book.set_inputs(0, &inputs, Locale::English).unwrap();
    book.set_format(0, cell("H4"), NumberFormat::Percent)
        .unwrap();
    book.set_format(0, cell("I4"), NumberFormat::Date).unwrap();
    book.set_format(0, cell("B3"), NumberFormat::Decimals(2))
        .unwrap();
    book.set_column_width(0, 0, Some(30.0)).unwrap();
    let second = book.add_sheet("Riepilogo 2026").unwrap();
    book.set_input(second, cell("A1"), "=Foglio1!B4*2", Locale::English)
        .unwrap();
    book
}

#[test]
fn reads_back_what_it_writes() {
    let original = sample();
    let bytes = write_xlsx(&original).unwrap();
    let read = read_spreadsheet(
        &bytes,
        Some("conti.xlsx"),
        &Limits::default(),
        Locale::English,
    )
    .unwrap();
    assert_eq!(read.sheets().len(), 2);
    assert_eq!(read.sheets()[1].name, "Riepilogo 2026");
    for (index, sheet) in original.sheets().iter().enumerate() {
        for (position, content) in sheet.cells() {
            let other = read.sheets()[index]
                .cell(position)
                .unwrap_or_else(|| panic!("{}", position.a1()));
            assert_eq!(
                other.input.text(),
                content.input.text(),
                "{}",
                position.a1()
            );
            if matches!(content.format, NumberFormat::Date) {
                assert_eq!(other.format, NumberFormat::Date);
            }
        }
    }
    assert_eq!(read.value(1, cell("A1")), Value::Number(3_831.0));
    assert_eq!(read.value(0, cell("F4")), Value::Error(ErrorKind::Circular));
    assert_eq!(read.sheets()[0].column_widths[&0], 30.0);
    assert_eq!(read.display(0, cell("I4"), Locale::Italian), "27/10/2026");
}

#[test]
fn caches_each_formula_value_for_viewers_that_do_not_recalculate() {
    let bytes = write_xlsx(&sample()).unwrap();
    let workbook =
        rely_office_core::read_workbook(&bytes, Some("conti.xlsx"), &Limits::default()).unwrap();
    let row = &workbook.sheets[0].rows[3];
    assert_eq!(row[1], "1915.5");
    assert_eq!(row[2], "true");
    assert!(row[3].ends_with("altro\"!"));
    assert_eq!(row[4], "#DIV/0!");
    assert_eq!(row[5], "#REF!");
    assert_eq!(workbook.sheets[1].rows[0][0], "3831");
}

#[test]
fn reads_an_xlsx_made_elsewhere() {
    let mut writer = rust_xlsxwriter::Workbook::new();
    let sheet = writer.add_worksheet().set_name("Vendite").unwrap();
    sheet.write(0, 0, "Mese").unwrap();
    sheet.write(1, 0, 10).unwrap();
    sheet.write(2, 0, 20.5).unwrap();
    sheet.write_formula(3, 0, "=SUM(A2:A3)").unwrap();
    sheet.write_boolean(4, 0, true).unwrap();
    let date = rust_xlsxwriter::ExcelDateTime::from_ymd(2026, 10, 27).unwrap();
    let format = rust_xlsxwriter::Format::new().set_num_format("dd/mm/yyyy");
    sheet
        .write_datetime_with_format(5, 0, &date, &format)
        .unwrap();
    sheet.set_column_width(0, 18).unwrap();
    let bytes = writer.save_to_buffer().unwrap();

    let read = read_spreadsheet(&bytes, None, &Limits::default(), Locale::Italian).unwrap();
    assert_eq!(read.sheets()[0].name, "Vendite");
    assert_eq!(read.value(0, cell("A4")), Value::Number(30.5));
    assert_eq!(
        read.sheets()[0].cell(cell("A4")).unwrap().input.text(),
        "=SUM(A2:A3)"
    );
    assert_eq!(read.value(0, cell("A5")), Value::Bool(true));
    assert_eq!(read.display(0, cell("A6"), Locale::Italian), "27/10/2026");
    assert!((read.sheets()[0].column_widths[&0] - 18.71).abs() < 0.01);
}

#[test]
fn reads_a_csv_in_the_locale() {
    let csv = "Voce;Importo\nCanone;1.850,50\nFormula;=1+1\n".as_bytes();
    let read =
        read_spreadsheet(csv, Some("conti.csv"), &Limits::default(), Locale::Italian).unwrap();
    assert_eq!(read.value(0, cell("B2")), Value::Number(1_850.5));
    assert_eq!(read.value(0, cell("B3")), Value::Text("=1+1".to_owned()));
    assert_eq!(read.sheets()[0].name, "Foglio1");
}

#[test]
fn refuses_what_is_not_a_spreadsheet_or_too_large() {
    assert!(matches!(
        read_spreadsheet(b"plain", None, &Limits::default(), Locale::Italian),
        Err(OfficeError::UnsupportedFormat(_))
    ));
    let limits = Limits {
        max_input_bytes: 3,
        ..Limits::default()
    };
    assert!(matches!(
        read_spreadsheet(b"a,b\n", Some("a.csv"), &limits, Locale::Italian),
        Err(OfficeError::TooLarge { .. })
    ));
    assert!(
        read_spreadsheet(
            b"PK\x03\x04broken",
            Some("a.xlsx"),
            &Limits::default(),
            Locale::Italian
        )
        .is_err()
    );
}

#[test]
fn escapes_text_for_xml() {
    assert_eq!(escape("a<b>&\"c\u{1}\td"), "a&lt;b&gt;&amp;&quot;c\td");
}
