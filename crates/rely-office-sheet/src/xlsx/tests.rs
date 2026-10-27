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

#[test]
fn reads_back_the_styles_merges_and_heights_it_writes() {
    use crate::{BorderPlacement, StylePatch};
    let locale = Locale::Italian;
    let mut workbook = Workbook::new();
    let header = StylePatch {
        bold: Some(true),
        font_size: Some(14.0),
        font_family: Some("Arial".to_owned()),
        font_colour: Some("FFFFFF".to_owned()),
        fill: Some("1F4E79".to_owned()),
        horizontal: Some("center".to_owned()),
        vertical: Some("center".to_owned()),
        ..StylePatch::default()
    };
    workbook.set_input(0, cell("A1"), "Titolo", locale).unwrap();
    workbook
        .set_style(0, (cell("A1"), cell("C1")), &header)
        .unwrap();
    workbook.merge_cells(0, (cell("A1"), cell("C1"))).unwrap();
    workbook.set_input(0, cell("A2"), "1234,5", locale).unwrap();
    workbook
        .set_style(
            0,
            (cell("A2"), cell("A2")),
            &StylePatch {
                number_format: Some("#,##0.00 \"€\";[Red]-#,##0.00 \"€\"".to_owned()),
                italic: Some(true),
                wrap: Some(true),
                ..StylePatch::default()
            },
        )
        .unwrap();
    workbook.set_input(0, cell("B2"), "0,5", locale).unwrap();
    workbook
        .set_format(0, cell("B2"), NumberFormat::Decimals(3))
        .unwrap();
    let border = Border {
        style: BorderStyle::Single,
        width_eighths: 8,
        colour: Some("FF0000".to_owned()),
    };
    workbook
        .set_borders(
            0,
            (cell("A4"), cell("B5")),
            BorderPlacement::Outline,
            Some(&border),
        )
        .unwrap();
    workbook.set_row_height(0, 0, Some(30.0)).unwrap();
    workbook.set_row_height(0, 9, Some(0.0)).unwrap();

    let read = read_spreadsheet(
        &write_xlsx(&workbook).unwrap(),
        None,
        &Limits::default(),
        locale,
    )
    .unwrap();
    assert_eq!(read.style(0, cell("A1")), workbook.style(0, cell("A1")));
    assert_eq!(
        read.style(0, cell("C1")),
        workbook.style(0, cell("C1")),
        "a styled empty cell"
    );
    assert_eq!(read.merges(0), workbook.merges(0));
    assert_eq!(read.display(0, cell("A2"), locale), "1.234,50 €");
    assert!(read.style(0, cell("A2")).font.italic);
    assert!(read.style(0, cell("A2")).wrap);
    assert_eq!(read.display(0, cell("B2"), locale), "0,500");
    assert_eq!(
        read.sheets()[0].cell(cell("B2")).unwrap().format,
        NumberFormat::Decimals(3)
    );
    let corner = &read.style(0, cell("A4")).borders;
    assert_eq!(corner.top, Some(border.clone()));
    assert_eq!(corner.left, Some(border.clone()));
    assert_eq!(corner.bottom, None);
    assert_eq!(read.style(0, cell("B5")).borders.right, Some(border));
    assert_eq!(read.sheets()[0].row_heights.get(&0), Some(&30.0));
    assert_eq!(read.sheets()[0].row_heights.get(&9), Some(&0.0));
}

#[test]
fn writes_back_the_pictures_and_charts_of_the_file() {
    const PNG: [u8; 70] = [
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64,
        0x60, 0xf8, 0x5f, 0x0f, 0x00, 0x02, 0x87, 0x01, 0x80, 0xeb, 0x47, 0xba, 0x92, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];
    let mut writer = rust_xlsxwriter::Workbook::new();
    let sheet = writer.add_worksheet().set_name("Vendite").unwrap();
    sheet.write(0, 0, 10).unwrap();
    sheet.write(1, 0, 20).unwrap();
    let mut chart = rust_xlsxwriter::Chart::new(rust_xlsxwriter::ChartType::Line);
    chart.add_series().set_values("Vendite!$A$1:$A$2");
    sheet.insert_chart(3, 1, &chart).unwrap();
    sheet
        .insert_image(
            0,
            4,
            &rust_xlsxwriter::Image::new_from_buffer(&PNG).unwrap(),
        )
        .unwrap();
    writer.add_worksheet().set_name("Vuoto").unwrap();
    let bytes = writer.save_to_buffer().unwrap();

    let mut read = read_spreadsheet(&bytes, None, &Limits::default(), Locale::Italian).unwrap();
    assert!(read.sheets()[0].drawing.is_some());
    assert!(read.sheets()[1].drawing.is_none());
    read.set_input(0, cell("A3"), "30", Locale::Italian)
        .unwrap();
    let written = write_xlsx(&read).unwrap();

    let layouts = rely_office_core::read_xlsx_layouts(&written, &Limits::default());
    let mut drawings = layouts["Vendite"].drawings.clone();
    drawings.sort_by_key(|drawing| drawing.column);
    assert_eq!(
        drawings.len(),
        2,
        "the chart and the picture are still there"
    );
    assert_eq!(drawings[0].content_type, "image/svg+xml");
    assert_eq!(drawings[1].bytes, PNG);
    assert_eq!(layouts["Vuoto"].drawings, Vec::new());
    let again = read_spreadsheet(&written, None, &Limits::default(), Locale::Italian).unwrap();
    assert_eq!(again.value(0, cell("A3")), Value::Number(30.0));
    assert_eq!(
        again.sheets()[0].drawing,
        read.sheets()[0].drawing,
        "carried unchanged"
    );
    assert!(read.theme.is_some());
    assert_eq!(
        again.theme, read.theme,
        "the theme its charts take their colours from"
    );
}

#[test]
fn reads_the_iso_dates_and_durations_of_an_ods() {
    assert_eq!(iso_date_serial("2026-10-27"), Some(46322.0));
    assert_eq!(iso_date_serial("2026-10-27T18:00:00"), Some(46322.75));
    assert_eq!(
        iso_date_serial("2026-10-27T06:30"),
        Some(46322.0 + 6.5 / 24.0)
    );
    assert_eq!(iso_date_serial("27/10/2026"), None);
    assert_eq!(iso_duration_days("PT36H00M00S"), Some(1.5));
    assert_eq!(iso_duration_days("-P1DT12H"), Some(-1.5));
    assert_eq!(iso_duration_days("PT0.5S"), Some(0.5 / 86_400.0));
    assert_eq!(iso_duration_days("12:00"), None);
    assert_eq!(iso_duration_days("PT1X"), None);
    let date = cell_from_data(&Data::DateTimeIso("2026-10-27".to_owned())).unwrap();
    assert_eq!(
        (date.input, date.format),
        (Input::Number(46322.0), NumberFormat::Date)
    );
    let moment = cell_from_data(&Data::DateTimeIso("2026-10-27T18:00:00".to_owned())).unwrap();
    assert_eq!(moment.format, NumberFormat::DateTime);
    assert_eq!(
        cell_from_data(&Data::DateTimeIso("domani".to_owned()))
            .unwrap()
            .input,
        Input::Text("domani".to_owned())
    );
    assert_eq!(
        cell_from_data(&Data::DurationIso("PT12H".to_owned()))
            .unwrap()
            .input,
        Input::Number(0.5)
    );
}
