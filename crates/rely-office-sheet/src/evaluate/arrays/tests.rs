// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use crate::address::CellRef;
use crate::model::Workbook;
use crate::value::{ErrorKind, Locale, Value};

fn cell(address: &str) -> CellRef {
    CellRef::parse(address).unwrap()
}

/// A1:C5 a small table: name, region, amount, with a heading.
fn book(extra: &[(&str, &str)]) -> Workbook {
    let mut workbook = Workbook::new();
    let rows = [
        ("A1", "Nome"),
        ("B1", "Zona"),
        ("C1", "Importo"),
        ("A2", "Verdi"),
        ("B2", "Nord"),
        ("C2", "30"),
        ("A3", "Bianchi"),
        ("B3", "Sud"),
        ("C3", "10"),
        ("A4", "Rossi"),
        ("B4", "Nord"),
        ("C4", "20"),
        ("A5", "Neri"),
        ("B5", "Sud"),
        ("C5", "40"),
    ];
    let inputs: Vec<(CellRef, String)> = rows
        .iter()
        .chain(extra)
        .map(|(address, typed)| (cell(address), (*typed).to_owned()))
        .collect();
    workbook.set_inputs(0, &inputs, Locale::English).unwrap();
    workbook
}

fn column(workbook: &Workbook, letter: char, from: u32, to: u32) -> Vec<String> {
    (from..=to)
        .map(|row| workbook.display(0, cell(&format!("{letter}{row}")), Locale::English))
        .collect()
}

#[test]
fn filters_rows_and_spills_them() {
    let workbook = book(&[("E1", "=FILTER(A2:C5,B2:B5=\"Nord\")")]);
    assert_eq!(column(&workbook, 'E', 1, 3), ["Verdi", "Rossi", ""]);
    assert_eq!(column(&workbook, 'G', 1, 2), ["30", "20"]);
    assert_eq!(
        workbook.spills(0),
        [(
            cell("E1"),
            rely_office_core::CellRange::parse("E1:G2").unwrap()
        )]
    );
    assert_eq!(workbook.spill_anchor(0, cell("F2")), Some(cell("E1")));
}

#[test]
fn filters_with_a_fallback_and_says_when_nothing_is_left() {
    let workbook = book(&[
        ("E1", "=FILTER(A2:A5,C2:C5>100,\"nessuno\")"),
        ("F1", "=FILTER(A2:A5,C2:C5>100)"),
        ("G1", "=FILTER(A2:A5,C2:C3>1)"),
    ]);
    assert_eq!(workbook.display(0, cell("E1"), Locale::English), "nessuno");
    assert_eq!(workbook.value(0, cell("F1")), Value::Error(ErrorKind::Calc));
    assert_eq!(
        workbook.value(0, cell("G1")),
        Value::Error(ErrorKind::Value)
    );
}

#[test]
fn sorts_by_a_column_and_by_another_array() {
    let workbook = book(&[
        ("E1", "=SORT(A2:C5,3,-1)"),
        ("I1", "=SORTBY(A2:A5,B2:B5,1,C2:C5,1)"),
        ("J1", "=SORT(C2:C5)"),
    ]);
    assert_eq!(
        column(&workbook, 'E', 1, 4),
        ["Neri", "Verdi", "Rossi", "Bianchi"]
    );
    assert_eq!(
        column(&workbook, 'I', 1, 4),
        ["Rossi", "Verdi", "Bianchi", "Neri"]
    );
    assert_eq!(column(&workbook, 'J', 1, 4), ["10", "20", "30", "40"]);
}

#[test]
fn keeps_unique_values_and_those_seen_once() {
    let workbook = book(&[
        ("E1", "=UNIQUE(B2:B5)"),
        ("F1", "=UNIQUE({1;2;2;3},FALSE,TRUE)"),
        ("G1", "=UNIQUE(B2:B5,FALSE,TRUE)"),
    ]);
    assert_eq!(column(&workbook, 'E', 1, 3), ["Nord", "Sud", ""]);
    assert_eq!(workbook.value(0, cell("G1")), Value::Error(ErrorKind::Calc));
    let _ = workbook.value(0, cell("F1"));
}

#[test]
fn builds_sequences_and_reshapes_arrays() {
    let workbook = book(&[
        ("E1", "=SEQUENCE(3,2,10,5)"),
        ("H1", "=TRANSPOSE(A2:A4)"),
        ("H3", "=TAKE(A2:C5,-1,2)"),
        ("H5", "=DROP(A2:A5,2)"),
        ("H8", "=CHOOSECOLS(A2:C3,3,1)"),
    ]);
    assert_eq!(column(&workbook, 'E', 1, 3), ["10", "20", "30"]);
    assert_eq!(column(&workbook, 'F', 1, 3), ["15", "25", "35"]);
    assert_eq!(
        ["H1", "I1", "J1"].map(|address| workbook.display(0, cell(address), Locale::English)),
        ["Verdi", "Bianchi", "Rossi"]
    );
    assert_eq!(
        ["H3", "I3"].map(|address| workbook.display(0, cell(address), Locale::English)),
        ["Neri", "Sud"]
    );
    assert_eq!(column(&workbook, 'H', 5, 6), ["Rossi", "Neri"]);
    assert_eq!(
        ["H8", "I8"].map(|address| workbook.display(0, cell(address), Locale::English)),
        ["30", "Verdi"]
    );
}

#[test]
fn stacks_wraps_and_expands() {
    let workbook = book(&[
        ("E1", "=VSTACK(A2:B2,C2)"),
        ("E4", "=HSTACK(A2,C2:C3)"),
        ("E7", "=WRAPROWS(C2:C5,3,0)"),
        ("E10", "=TOROW(A2:A3)"),
        ("E12", "=EXPAND(C2,2,2,\"-\")"),
    ]);
    assert_eq!(
        ["E1", "F1", "E2", "F2"].map(|address| workbook.display(0, cell(address), Locale::English)),
        ["Verdi", "Nord", "30", "#N/A"]
    );
    assert_eq!(
        ["E4", "F4", "E5", "F5"].map(|address| workbook.display(0, cell(address), Locale::English)),
        ["Verdi", "30", "#N/A", "10"]
    );
    assert_eq!(
        ["E7", "G7", "E8", "F8"].map(|address| workbook.display(0, cell(address), Locale::English)),
        ["30", "20", "40", "0"]
    );
    assert_eq!(
        ["E10", "F10"].map(|address| workbook.display(0, cell(address), Locale::English)),
        ["Verdi", "Bianchi"]
    );
    assert_eq!(
        ["E12", "F12", "F13"].map(|address| workbook.display(0, cell(address), Locale::English)),
        ["30", "-", "-"]
    );
}

#[test]
fn multiplies_matrices_counts_frequencies_and_splits_text() {
    let workbook = book(&[
        ("E1", "1"),
        ("F1", "2"),
        ("E2", "3"),
        ("F2", "4"),
        ("H1", "=MMULT(E1:F2,E1:F2)"),
        ("H4", "=MUNIT(2)"),
        ("J1", "=FREQUENCY(C2:C5,{15;35})"),
        ("K1", "=TEXTSPLIT(\"a,b;c,d\",\",\",\";\")"),
    ]);
    assert_eq!(
        ["H1", "I1", "H2", "I2"].map(|address| workbook.display(0, cell(address), Locale::English)),
        ["7", "10", "15", "22"]
    );
    assert_eq!(
        ["H4", "I4", "I5"].map(|address| workbook.display(0, cell(address), Locale::English)),
        ["1", "0", "1"]
    );
    assert_eq!(column(&workbook, 'J', 1, 3), ["1", "2", "1"]);
    assert_eq!(
        ["K1", "L1", "K2", "L2"].map(|address| workbook.display(0, cell(address), Locale::English)),
        ["a", "b", "c", "d"]
    );
}

#[test]
fn computes_with_arrays_value_by_value() {
    let workbook = book(&[
        ("E1", "=SUM(C2:C5*2)"),
        ("E2", "=SUMPRODUCT((B2:B5=\"Nord\")*C2:C5)"),
        ("F1", "=C2:C3*10"),
        ("G1", "=-C2:C3"),
        ("H1", "=ROWS(FILTER(A2:A5,C2:C5>15))"),
    ]);
    assert_eq!(workbook.value(0, cell("E1")), Value::Number(200.0));
    assert_eq!(workbook.value(0, cell("E2")), Value::Number(50.0));
    assert_eq!(column(&workbook, 'F', 1, 2), ["300", "100"]);
    assert_eq!(column(&workbook, 'G', 1, 2), ["-30", "-10"]);
    assert_eq!(workbook.value(0, cell("H1")), Value::Number(3.0));
}

#[test]
fn refuses_to_spill_over_a_cell_that_holds_something() {
    let mut workbook = book(&[("E1", "=SEQUENCE(3)"), ("E3", "ostacolo")]);
    assert_eq!(
        workbook.value(0, cell("E1")),
        Value::Error(ErrorKind::Spill)
    );
    assert_eq!(workbook.display(0, cell("E2"), Locale::English), "");
    workbook
        .set_input(0, cell("E3"), "", Locale::English)
        .unwrap();
    assert_eq!(column(&workbook, 'E', 1, 3), ["1", "2", "3"]);
}

#[test]
fn a_formula_reading_a_spilled_cell_sees_its_value() {
    let workbook = book(&[
        ("A10", "=E2+E3"),
        ("E1", "=SEQUENCE(3,1,5)"),
        ("B10", "=SUM(E1:E3)"),
    ]);
    assert_eq!(workbook.value(0, cell("A10")), Value::Number(13.0));
    assert_eq!(workbook.value(0, cell("B10")), Value::Number(18.0));
}

#[test]
fn a_whole_range_formula_spills_its_cells() {
    let workbook = book(&[("E1", "=A2:B3")]);
    assert_eq!(
        ["E1", "F1", "E2", "F2"].map(|address| workbook.display(0, cell(address), Locale::English)),
        ["Verdi", "Nord", "Bianchi", "Sud"]
    );
}

#[test]
fn a_spilling_formula_survives_xlsx_and_ods() {
    use rely_office_core::Limits;
    let workbook = book(&[
        ("E1", "=SORT(FILTER(A2:A5,C2:C5>15))"),
        ("G1", "=SUM({1,2;3,4})"),
    ]);
    let xlsx = crate::xlsx::write_xlsx(&workbook).unwrap();
    let ods = crate::ods::write_ods(&workbook).unwrap();
    for (bytes, name) in [(xlsx, "a.xlsx"), (ods, "a.ods")] {
        let read =
            crate::xlsx::read_spreadsheet(&bytes, Some(name), &Limits::default(), Locale::English)
                .unwrap();
        assert_eq!(
            column(&read, 'E', 1, 3),
            ["Neri", "Rossi", "Verdi"],
            "{name}"
        );
        assert_eq!(read.value(0, cell("G1")), Value::Number(10.0), "{name}");
        assert!(read.sheets()[0].cell(cell("E2")).is_none(), "{name}");
        assert_eq!(read.spills(0).len(), 1, "{name}");
    }
}

#[test]
fn writes_a_spilling_formula_as_a_dynamic_array() {
    let workbook = book(&[("E1", "=FILTER(A2:A5,C2:C5>15)")]);
    let xlsx = crate::xlsx::write_xlsx(&workbook).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(xlsx)).unwrap();
    let mut sheet = String::new();
    std::io::Read::read_to_string(
        &mut archive.by_name("xl/worksheets/sheet1.xml").unwrap(),
        &mut sheet,
    )
    .unwrap();
    assert!(
        sheet.contains(
            r#"cm="1"><f t="array" ref="E1:E3">_xlfn._xlws.FILTER(A2:A5,C2:C5&gt;15)</f>"#
        ),
        "{sheet}"
    );
    assert!(archive.by_name("xl/metadata.xml").is_ok());
}
