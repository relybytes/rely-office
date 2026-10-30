// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use super::{determinant, r1c1_cells};
use crate::address::CellRef;
use crate::model::Workbook;
use crate::value::{ErrorKind, Locale, Value};

fn cell(address: &str) -> CellRef {
    CellRef::parse(address).unwrap()
}

/// A1:A4 numbers, B1 a formula, C1 text, D1:D3 with an error.
fn book() -> Workbook {
    let mut workbook = Workbook::new();
    let inputs: Vec<(CellRef, String)> = [
        ("A1", "10"),
        ("A2", "20"),
        ("A3", "30"),
        ("A4", "40"),
        ("B1", "=A1*2"),
        ("C1", "testo"),
        ("D1", "1"),
        ("D2", "=1/0"),
        ("D3", "5"),
    ]
    .iter()
    .map(|(address, typed)| (cell(address), (*typed).to_owned()))
    .collect();
    workbook.set_inputs(0, &inputs, Locale::English).unwrap();
    workbook.add_sheet("Altro").unwrap();
    workbook
        .set_input(1, cell("B2"), "7", Locale::English)
        .unwrap();
    workbook
}

fn value_in(workbook: &mut Workbook, formula: &str) -> Value {
    workbook
        .set_input(0, cell("Z99"), formula, Locale::English)
        .unwrap();
    workbook.value(0, cell("Z99"))
}

fn value(formula: &str) -> Value {
    value_in(&mut book(), formula)
}

fn number(formula: &str) -> f64 {
    match value(formula) {
        Value::Number(number) => number,
        other => panic!("{formula} gave {other:?}"),
    }
}

#[test]
fn offset_moves_and_sizes_a_reference() {
    assert_eq!(number("=SUM(OFFSET(A1,1,0,2,1))"), 50.0);
    assert_eq!(number("=OFFSET(A1,3,0)"), 40.0);
    assert_eq!(number("=OFFSET(A1,0,1)+1"), 21.0);
    assert_eq!(number("=SUM(OFFSET(A4,0,0,-3,1))"), 90.0);
    assert_eq!(number("=ROWS(OFFSET(A1:B2,0,0))"), 2.0);
    assert_eq!(value("=OFFSET(A1,-1,0)"), Value::Error(ErrorKind::Ref));
    assert_eq!(value("=OFFSET(A1,0,0,0,1)"), Value::Error(ErrorKind::Ref));
    assert_eq!(value("=OFFSET(5,0,0)"), Value::Error(ErrorKind::Value));
}

#[test]
fn indirect_reads_a_reference_written_as_text() {
    assert_eq!(number("=INDIRECT(\"A2\")"), 20.0);
    assert_eq!(number("=SUM(INDIRECT(\"A1:A3\"))"), 60.0);
    assert_eq!(number("=INDIRECT(\"Altro!B2\")"), 7.0);
    assert_eq!(number("=INDIRECT(\"'Altro'!B2\")"), 7.0);
    assert_eq!(number("=INDIRECT(\"R2C1\",FALSE)"), 20.0);
    assert_eq!(number("=SUM(INDIRECT(\"R1C1:R4C1\",FALSE))"), 100.0);
    assert_eq!(number("=INDIRECT(\"B\"&1)"), 20.0);
    assert_eq!(
        value("=INDIRECT(\"not a reference\")"),
        Value::Error(ErrorKind::Ref)
    );
    assert_eq!(
        value("=INDIRECT(\"Missing!A1\")"),
        Value::Error(ErrorKind::Ref)
    );
    assert_eq!(value("=INDIRECT(\"\")"), Value::Error(ErrorKind::Ref));
    let mut workbook = book();
    workbook
        .set_defined_name("Valori", None, "A1:A4", Locale::English)
        .unwrap();
    assert_eq!(
        value_in(&mut workbook, "=SUM(INDIRECT(\"Valori\"))"),
        Value::Number(100.0)
    );
}

#[test]
fn a_computed_reference_waits_for_the_formulas_it_reaches() {
    let mut workbook = Workbook::new();
    let inputs: Vec<(CellRef, String)> = [
        ("A1", "=INDIRECT(\"C5\")+1"),
        ("B1", "=SUM(OFFSET(C4,0,0,2,1))"),
        ("C4", "=B5*10"),
        ("C5", "=B5*2"),
        ("B5", "3"),
    ]
    .iter()
    .map(|(address, typed)| (cell(address), (*typed).to_owned()))
    .collect();
    workbook.set_inputs(0, &inputs, Locale::English).unwrap();
    assert_eq!(workbook.value(0, cell("A1")), Value::Number(7.0));
    assert_eq!(workbook.value(0, cell("B1")), Value::Number(36.0));
    workbook
        .set_input(0, cell("D1"), "=INDIRECT(\"D1\")", Locale::English)
        .unwrap();
    assert_eq!(
        workbook.value(0, cell("D1")),
        Value::Error(ErrorKind::Circular)
    );
    // OFFSET names its own cell only to move from it, which is no circular reference.
    workbook
        .set_input(0, cell("B4"), "=OFFSET(B4,1,0)", Locale::English)
        .unwrap();
    assert_eq!(workbook.value(0, cell("B4")), Value::Number(3.0));
}

#[test]
fn trimrange_drops_the_empty_edges_of_a_range() {
    assert_eq!(number("=ROWS(TRIMRANGE(A1:A50))"), 4.0);
    assert_eq!(number("=COLUMNS(TRIMRANGE(A1:H50))"), 4.0);
    assert_eq!(number("=ROWS(TRIMRANGE(A1:A50,0))"), 50.0);
    assert_eq!(number("=ROWS(TRIMRANGE(A1:A50,1))"), 50.0);
    assert_eq!(value("=TRIMRANGE(K1:K5)"), Value::Error(ErrorKind::Ref));
    assert_eq!(value("=TRIMRANGE(A1:A5,7)"), Value::Error(ErrorKind::Value));
}

#[test]
fn reads_r1c1_references() {
    let current = CellRef::new(4, 4);
    assert_eq!(
        r1c1_cells("R1C1", current),
        Some((CellRef::new(0, 0), CellRef::new(0, 0)))
    );
    assert_eq!(
        r1c1_cells("R[-1]C[2]", current),
        Some((CellRef::new(3, 6), CellRef::new(3, 6)))
    );
    assert_eq!(r1c1_cells("rc", current), Some((current, current)));
    assert_eq!(
        r1c1_cells("R1C1:R2C3", current),
        Some((CellRef::new(0, 0), CellRef::new(1, 2)))
    );
    assert_eq!(r1c1_cells("R0C1", current), None);
    assert_eq!(r1c1_cells("A1", current), None);
    assert_eq!(r1c1_cells("R[-9]C", current), None);
}

#[test]
fn describes_references() {
    assert_eq!(number("=AREAS(A1:B2)"), 1.0);
    assert_eq!(number("=SHEET()"), 1.0);
    assert_eq!(number("=SHEET(Altro!A1)"), 2.0);
    assert_eq!(number("=SHEET(\"Altro\")"), 2.0);
    assert_eq!(
        value("=SHEET(\"Nessuno\")"),
        Value::Error(ErrorKind::NotAvailable)
    );
    assert_eq!(number("=SHEETS()"), 2.0);
    assert_eq!(number("=SHEETS(A1)"), 1.0);
    assert_eq!(value("=FORMULATEXT(B1)"), Value::Text("=A1*2".to_owned()));
    assert_eq!(
        value("=FORMULATEXT(A1)"),
        Value::Error(ErrorKind::NotAvailable)
    );
    assert_eq!(value("=ISFORMULA(B1)"), Value::Bool(true));
    assert_eq!(value("=ISFORMULA(A1)"), Value::Bool(false));
    assert_eq!(
        value("=CELL(\"address\",B3)"),
        Value::Text("$B$3".to_owned())
    );
    assert_eq!(
        value("=CELL(\"address\",Altro!B2)"),
        Value::Text("'Altro'!$B$2".to_owned())
    );
    assert_eq!(number("=CELL(\"row\",B3)"), 3.0);
    assert_eq!(number("=CELL(\"col\",B3)"), 2.0);
    assert_eq!(value("=CELL(\"type\",C1)"), Value::Text("l".to_owned()));
    assert_eq!(value("=CELL(\"type\",A1)"), Value::Text("v".to_owned()));
    assert_eq!(value("=CELL(\"type\",K9)"), Value::Text("b".to_owned()));
    assert_eq!(number("=CELL(\"contents\",A2)"), 20.0);
    assert_eq!(value("=CELL(\"format\",A2)"), Value::Text("G".to_owned()));
    assert_eq!(number("=CELL(\"width\",A2)"), 8.0);
    assert_eq!(
        value("=CELL(\"nonsense\",A2)"),
        Value::Error(ErrorKind::Value)
    );
}

#[test]
fn xmatch_finds_exact_nearest_and_wildcard_matches() {
    assert_eq!(number("=XMATCH(4,{5,4,3,2,1})"), 2.0);
    assert_eq!(number("=XMATCH(4.5,{5,4,3,2,1},1)"), 1.0);
    assert_eq!(number("=XMATCH(4.5,{5,4,3,2,1},-1)"), 2.0);
    assert_eq!(number("=XMATCH(\"b*\",{\"apple\",\"banana\"},2)"), 2.0);
    assert_eq!(number("=XMATCH(1,{1,2,1},0,-1)"), 3.0);
    assert_eq!(number("=XMATCH(30,A1:A4)"), 3.0);
    assert_eq!(
        value("=XMATCH(9,{5,4,3,2,1})"),
        Value::Error(ErrorKind::NotAvailable)
    );
    assert_eq!(
        value("=XMATCH(9,{5,4,3,2,1},3)"),
        Value::Error(ErrorKind::Value)
    );
    assert_eq!(
        value("=XMATCH(1,{1,2;3,4})"),
        Value::Error(ErrorKind::Value)
    );
}

#[test]
fn aggregate_leaves_out_what_its_options_say() {
    assert_eq!(number("=AGGREGATE(9,6,D1:D3)"), 6.0);
    assert_eq!(
        value("=AGGREGATE(9,4,D1:D3)"),
        Value::Error(ErrorKind::Div0)
    );
    assert_eq!(number("=AGGREGATE(4,6,A1:A4,D1:D3)"), 40.0);
    assert_eq!(number("=AGGREGATE(14,6,A1:A4,2)"), 30.0);
    assert_eq!(number("=AGGREGATE(15,6,A1:A4,1)"), 10.0);
    assert_eq!(number("=AGGREGATE(16,6,A1:A4,0.5)"), 25.0);
    assert_eq!(number("=AGGREGATE(17,6,A1:A4,1)"), 17.5);
    assert_eq!(number("=AGGREGATE(1,0,A1:A4)"), 25.0);
    assert_eq!(number("=AGGREGATE(2,0,A1:C1)"), 2.0);
    assert_eq!(number("=AGGREGATE(3,0,A1:C1)"), 3.0);
    assert_eq!(
        value("=AGGREGATE(20,0,A1:A4)"),
        Value::Error(ErrorKind::Value)
    );
    assert_eq!(
        value("=AGGREGATE(14,6,A1:A4,9)"),
        Value::Error(ErrorKind::Num)
    );
    let mut workbook = book();
    workbook.set_row_height(0, 1, Some(0.0)).unwrap();
    assert_eq!(
        value_in(&mut workbook, "=AGGREGATE(9,5,A1:A4)"),
        Value::Number(80.0)
    );
    workbook
        .set_input(0, cell("A5"), "=AGGREGATE(9,0,A1:A4)", Locale::English)
        .unwrap();
    assert_eq!(
        value_in(&mut workbook, "=AGGREGATE(9,0,A1:A5)"),
        Value::Number(100.0)
    );
}

#[test]
fn draws_random_numbers_in_their_range() {
    let mut workbook = book();
    for _ in 0..20 {
        let Value::Number(draw) = value_in(&mut workbook, "=RAND()") else {
            panic!("RAND gave no number");
        };
        assert!((0.0..1.0).contains(&draw));
        let Value::Number(die) = value_in(&mut workbook, "=RANDBETWEEN(1,6)") else {
            panic!("RANDBETWEEN gave no number");
        };
        assert!((1.0..=6.0).contains(&die) && die.fract() == 0.0);
    }
    assert_ne!(
        value_in(&mut workbook, "=RAND()-RAND()"),
        Value::Number(0.0)
    );
    assert_eq!(value("=RANDBETWEEN(6,1)"), Value::Error(ErrorKind::Num));
}

#[test]
fn sums_series_divides_parts_and_takes_determinants() {
    let series = number("=SERIESSUM(PI()/4,0,2,{1,-0.5,0.041666667,-0.001388889})");
    assert!((series - 0.707_103_215).abs() < 1e-8);
    assert_eq!(number("=PERCENTOF(A1:A2,A1:A4)"), 0.3);
    assert_eq!(value("=PERCENTOF(A1,K1)"), Value::Error(ErrorKind::Div0));
    assert!((number("=MDETERM({1,3,8,5;1,3,6,1;1,1,1,0;7,3,10,2})") - 88.0).abs() < 1e-9);
    assert!((number("=MDETERM({3,6,1;1,1,0;3,10,2})") - 1.0).abs() < 1e-12);
    assert_eq!(
        value("=MDETERM({1,2,3;4,5,6})"),
        Value::Error(ErrorKind::Value)
    );
    assert_eq!(value("=MDETERM(C1)"), Value::Error(ErrorKind::Value));
    assert_eq!(determinant(vec![vec![0.0, 1.0], vec![1.0, 0.0]]), -1.0);
    assert_eq!(determinant(vec![vec![0.0, 0.0], vec![1.0, 0.0]]), 0.0);
}
