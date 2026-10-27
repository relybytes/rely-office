// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use crate::address::CellRef;
use crate::model::Workbook;
use crate::value::{ErrorKind, Locale, Value};

fn cell(address: &str) -> CellRef {
    CellRef::parse(address).unwrap()
}

fn workbook(inputs: &[(&str, &str)]) -> Workbook {
    let mut workbook = Workbook::new();
    let inputs: Vec<(CellRef, String)> = inputs
        .iter()
        .map(|(address, typed)| (cell(address), (*typed).to_owned()))
        .collect();
    workbook.set_inputs(0, &inputs, Locale::English).unwrap();
    workbook
}

fn value(inputs: &[(&str, &str)], formula: &str) -> Value {
    let mut book = workbook(inputs);
    book.set_input(0, cell("Z99"), formula, Locale::English)
        .unwrap();
    book.value(0, cell("Z99"))
}

fn number(inputs: &[(&str, &str)], formula: &str) -> f64 {
    match value(inputs, formula) {
        Value::Number(number) => number,
        other => panic!("{formula} gave {other:?}"),
    }
}

const DATA: &[(&str, &str)] = &[
    ("A1", "Mese"),
    ("B1", "Importo"),
    ("A2", "Gennaio"),
    ("B2", "10"),
    ("A3", "Febbraio"),
    ("B3", "20"),
    ("A4", "Marzo"),
    ("B4", "30"),
    ("A5", "aprile"),
    ("B5", "x"),
];

#[test]
fn computes_arithmetic_and_text() {
    assert!((number(&[], "=1+2*3-4/2") - 5.0).abs() < 1e-12);
    assert!((number(&[], "=-2^2") - 4.0).abs() < 1e-12);
    assert!((number(&[], "=2^3^2") - 64.0).abs() < 1e-12);
    assert!((number(&[], "=50%") - 0.5).abs() < 1e-12);
    assert!((number(&[("A1", "2"), ("A2", "'3")], "=A1*A2") - 6.0).abs() < 1e-12);
    assert_eq!(
        value(&[("A1", "Mario")], "=\"Ciao \"&A1&\"!\""),
        Value::Text("Ciao Mario!".to_owned())
    );
    assert_eq!(value(&[], "=1/0"), Value::Error(ErrorKind::Div0));
    assert_eq!(
        value(&[("A1", "x")], "=A1+1"),
        Value::Error(ErrorKind::Value)
    );
    assert_eq!(value(&[], "=A1:A2"), Value::Error(ErrorKind::Value));
    assert_eq!(value(&[], "=Altro!A1"), Value::Error(ErrorKind::Ref));
    assert_eq!(value(&[], "=MISTERO(1)"), Value::Error(ErrorKind::Name));
    assert_eq!(value(&[], "=10^400"), Value::Error(ErrorKind::Num));
    assert_eq!(value(&[], "=\"a\"=\"A\""), Value::Bool(true));
    assert_eq!(value(&[], "=\"5\">5"), Value::Bool(true));
    assert_eq!(value(&[], "=A1=0"), Value::Bool(true));
}

#[test]
fn aggregates_ranges_as_spreadsheets_do() {
    assert!((number(DATA, "=SUM(B1:B5)") - 60.0).abs() < 1e-12);
    assert!((number(DATA, "=SUM(B:B)") - 60.0).abs() < 1e-12);
    assert!((number(DATA, "=AVERAGE(B2:B5)") - 20.0).abs() < 1e-12);
    assert!((number(DATA, "=MIN(B2:B4,5)") - 5.0).abs() < 1e-12);
    assert!((number(DATA, "=MAX(B2:B4)") - 30.0).abs() < 1e-12);
    assert!((number(DATA, "=PRODUCT(B2:B3)") - 200.0).abs() < 1e-12);
    assert!((number(DATA, "=COUNT(B1:B5)") - 3.0).abs() < 1e-12);
    assert!((number(DATA, "=COUNTA(A1:B5)") - 10.0).abs() < 1e-12);
    assert!((number(DATA, "=COUNTBLANK(C1:C4)") - 4.0).abs() < 1e-12);
    assert!((number(&[], "=SUM(1,\"2\",TRUE)") - 4.0).abs() < 1e-12);
    assert_eq!(value(&[], "=SUM(1,\"x\")"), Value::Error(ErrorKind::Value));
    assert_eq!(value(&[], "=AVERAGE(C1:C3)"), Value::Error(ErrorKind::Div0));
    assert_eq!(
        value(&[("A1", "=1/0")], "=SUM(A1:A2)"),
        Value::Error(ErrorKind::Div0)
    );
}

#[test]
fn decides_with_logic() {
    assert_eq!(
        value(DATA, "=IF(B2>15,\"alto\",\"basso\")"),
        Value::Text("basso".to_owned())
    );
    assert_eq!(
        value(DATA, "=IF(B3>15,\"alto\")"),
        Value::Text("alto".to_owned())
    );
    assert_eq!(value(DATA, "=IF(B2>15,\"alto\")"), Value::Bool(false));
    assert_eq!(value(DATA, "=IF(TRUE,C9)"), Value::Number(0.0));
    assert_eq!(value(&[], "=IF(1/0,1,2)"), Value::Error(ErrorKind::Div0));
    assert_eq!(
        value(&[], "=IFERROR(1/0,\"n.d.\")"),
        Value::Text("n.d.".to_owned())
    );
    assert_eq!(value(&[], "=IFERROR(4,0)"), Value::Number(4.0));
    assert_eq!(value(DATA, "=AND(B2>5,B3>5)"), Value::Bool(true));
    assert_eq!(value(DATA, "=OR(B2>50,B3>50)"), Value::Bool(false));
    assert_eq!(value(&[], "=NOT(0)"), Value::Bool(true));
    assert_eq!(value(&[], "=AND()"), Value::Error(ErrorKind::Value));
    assert_eq!(value(&[], "=TRUE()"), Value::Bool(true));
    assert_eq!(value(&[], "=ISBLANK(Q1)"), Value::Bool(true));
    assert_eq!(value(DATA, "=ISNUMBER(B2)"), Value::Bool(true));
    assert_eq!(value(DATA, "=ISTEXT(A2)"), Value::Bool(true));
    assert_eq!(value(&[], "=ISERROR(1/0)"), Value::Bool(true));
}

#[test]
fn rounds_and_computes_math() {
    assert!((number(&[], "=ROUND(2.675,2)") - 2.68).abs() < 1e-12);
    assert!((number(&[], "=ROUND(1234.5,-2)") - 1_200.0).abs() < 1e-12);
    assert!((number(&[], "=ROUNDUP(2.01,0)") - 3.0).abs() < 1e-12);
    assert!((number(&[], "=ROUNDUP(2,0)") - 2.0).abs() < 1e-12);
    assert!((number(&[], "=ROUNDDOWN(-2.99,0)") + 2.0).abs() < 1e-12);
    assert!((number(&[], "=INT(-2.5)") + 3.0).abs() < 1e-12);
    assert!((number(&[], "=MOD(-7,3)") - 2.0).abs() < 1e-12);
    assert!((number(&[], "=ABS(-4)") - 4.0).abs() < 1e-12);
    assert!((number(&[], "=SIGN(-4)") + 1.0).abs() < 1e-12);
    assert!((number(&[], "=POWER(2,10)") - 1_024.0).abs() < 1e-12);
    assert!((number(&[], "=SQRT(16)") - 4.0).abs() < 1e-12);
    assert!((number(&[], "=PI()") - std::f64::consts::PI).abs() < 1e-12);
    assert_eq!(value(&[], "=SQRT(-1)"), Value::Error(ErrorKind::Num));
    assert_eq!(value(&[], "=MOD(1,0)"), Value::Error(ErrorKind::Div0));
    assert_eq!(value(&[], "=0^-1"), Value::Error(ErrorKind::Div0));
}

#[test]
fn works_on_text() {
    assert_eq!(
        value(&[], "=LEFT(\"Arona\",3)"),
        Value::Text("Aro".to_owned())
    );
    assert_eq!(value(&[], "=LEFT(\"Arona\")"), Value::Text("A".to_owned()));
    assert_eq!(
        value(&[], "=RIGHT(\"Arona\",10)"),
        Value::Text("Arona".to_owned())
    );
    assert_eq!(
        value(&[], "=MID(\"Verbania\",4,3)"),
        Value::Text("ban".to_owned())
    );
    assert!((number(&[], "=LEN(\"città\")") - 5.0).abs() < 1e-12);
    assert_eq!(value(&[], "=UPPER(\"via\")"), Value::Text("VIA".to_owned()));
    assert_eq!(value(&[], "=LOWER(\"VIA\")"), Value::Text("via".to_owned()));
    assert_eq!(
        value(&[], "=TRIM(\"  a   b \")"),
        Value::Text("a b".to_owned())
    );
    assert_eq!(
        value(DATA, "=CONCATENATE(A2,\"-\",B2)"),
        Value::Text("Gennaio-10".to_owned())
    );
    assert_eq!(
        value(DATA, "=CONCAT(B2:B3)"),
        Value::Text("1020".to_owned())
    );
    assert_eq!(
        value(&[], "=LEFT(\"x\",-1)"),
        Value::Error(ErrorKind::Value)
    );
    assert_eq!(
        value(&[], "=MID(\"x\",0,1)"),
        Value::Error(ErrorKind::Value)
    );
}

#[test]
fn sums_and_counts_by_criteria() {
    assert!((number(DATA, "=SUMIF(B2:B5,\">15\")") - 50.0).abs() < 1e-12);
    assert!((number(DATA, "=SUMIF(A2:A5,\"*aio\",B2:B5)") - 30.0).abs() < 1e-12);
    assert!((number(DATA, "=COUNTIF(A2:A5,\"APRILE\")") - 1.0).abs() < 1e-12);
    assert!((number(DATA, "=COUNTIF(A2:A5,\"<>Marzo\")") - 3.0).abs() < 1e-12);
    assert!((number(DATA, "=COUNTIF(B2:B5,20)") - 1.0).abs() < 1e-12);
    assert!((number(DATA, "=COUNTIF(A2:A5,\"?????\")") - 1.0).abs() < 1e-12);
    assert!((number(DATA, "=AVERAGEIF(B2:B5,\">=10\")") - 20.0).abs() < 1e-12);
    assert_eq!(
        value(DATA, "=AVERAGEIF(B2:B5,\">99\")"),
        Value::Error(ErrorKind::Div0)
    );
    assert!((number(DATA, "=SUMIFS(B2:B5,A2:A5,\"*o\",B2:B5,\">15\")") - 50.0).abs() < 1e-12);
    assert!((number(DATA, "=COUNTIFS(A2:A5,\"*o\")") - 3.0).abs() < 1e-12);
    assert_eq!(
        value(DATA, "=COUNTIFS(A2:A5)"),
        Value::Error(ErrorKind::Value)
    );
}

#[test]
fn looks_values_up() {
    assert_eq!(
        value(DATA, "=VLOOKUP(\"marzo\",A2:B5,2,FALSE)"),
        Value::Number(30.0)
    );
    assert_eq!(
        value(DATA, "=VLOOKUP(\"Luglio\",A2:B5,2,FALSE)"),
        Value::Error(ErrorKind::NotAvailable)
    );
    assert_eq!(
        value(DATA, "=VLOOKUP(\"Marzo\",A2:B5,3,FALSE)"),
        Value::Error(ErrorKind::Ref)
    );
    let sorted = &[
        ("A1", "10"),
        ("B1", "basso"),
        ("A2", "20"),
        ("B2", "medio"),
        ("A3", "30"),
        ("B3", "alto"),
    ];
    assert_eq!(
        value(sorted, "=VLOOKUP(25,A1:B3,2)"),
        Value::Text("medio".to_owned())
    );
    assert_eq!(
        value(sorted, "=VLOOKUP(5,A1:B3,2)"),
        Value::Error(ErrorKind::NotAvailable)
    );
    assert_eq!(
        value(sorted, "=HLOOKUP(\"medio\",B1:B3,1,FALSE)"),
        Value::Error(ErrorKind::NotAvailable)
    );
    let row = &[("A1", "x"), ("B1", "y"), ("A2", "1"), ("B2", "2")];
    assert_eq!(
        value(row, "=HLOOKUP(\"y\",A1:B2,2,FALSE)"),
        Value::Number(2.0)
    );
    assert_eq!(value(sorted, "=MATCH(20,A1:A3,0)"), Value::Number(2.0));
    assert_eq!(value(sorted, "=MATCH(29,A1:A3)"), Value::Number(2.0));
    assert_eq!(value(row, "=MATCH(\"y\",A1:B1,0)"), Value::Number(2.0));
    assert_eq!(
        value(sorted, "=INDEX(A1:B3,3,2)"),
        Value::Text("alto".to_owned())
    );
    assert_eq!(value(row, "=INDEX(A1:B1,2)"), Value::Text("y".to_owned()));
    assert_eq!(
        value(sorted, "=INDEX(A1:B3,9,1)"),
        Value::Error(ErrorKind::Ref)
    );
}

#[test]
fn computes_dates_with_the_host_clock() {
    assert!((number(&[], "=DATE(2026,10,27)") - 46_322.0).abs() < 1e-12);
    assert!((number(&[], "=DATE(26,1,1)") - number(&[], "=DATE(1926,1,1)")).abs() < 1e-12);
    assert!((number(&[], "=YEAR(46322)") - 2_026.0).abs() < 1e-12);
    assert!((number(&[], "=MONTH(46322)") - 10.0).abs() < 1e-12);
    assert!((number(&[], "=DAY(46322)") - 27.0).abs() < 1e-12);
    assert_eq!(value(&[], "=DAY(1)"), Value::Error(ErrorKind::Num));
    assert_eq!(
        value(&[], "=TODAY()"),
        Value::Error(ErrorKind::NotAvailable)
    );
    let mut book = workbook(&[("A1", "=TODAY()"), ("A2", "=NOW()")]);
    book.set_today(Some(46_322.5));
    assert_eq!(book.value(0, cell("A1")), Value::Number(46_322.0));
    assert_eq!(book.value(0, cell("A2")), Value::Number(46_322.5));
}

#[test]
fn recalculates_chains_and_stops_at_cycles() {
    let mut book = workbook(&[
        ("A1", "1"),
        ("A2", "=A1*2"),
        ("A3", "=A2+A1"),
        ("B1", "=SUM(A1:A3)"),
    ]);
    assert_eq!(book.value(0, cell("B1")), Value::Number(6.0));
    book.set_input(0, cell("A1"), "10", Locale::English)
        .unwrap();
    assert_eq!(book.value(0, cell("A3")), Value::Number(30.0));
    assert_eq!(book.value(0, cell("B1")), Value::Number(60.0));

    let cycle = workbook(&[
        ("A1", "=B1+1"),
        ("B1", "=A1+1"),
        ("C1", "=C1"),
        ("D1", "=A1*0"),
    ]);
    for address in ["A1", "B1", "C1", "D1"] {
        assert_eq!(
            cycle.value(0, cell(address)),
            Value::Error(ErrorKind::Circular),
            "{address}"
        );
    }
}

#[test]
fn computes_a_long_chain_without_recursion() {
    let mut book = Workbook::new();
    let mut inputs = vec![(cell("A1"), "1".to_owned())];
    for row in 1..100_000_u32 {
        inputs.push((CellRef::new(row, 0), format!("=A{row}+1")));
    }
    book.set_inputs(0, &inputs, Locale::English).unwrap();
    assert_eq!(
        book.value(0, CellRef::new(99_999, 0)),
        Value::Number(100_000.0)
    );
}

#[test]
fn reads_other_sheets() {
    let mut book = workbook(&[("A1", "5")]);
    let second = book.add_sheet("Riepilogo 2026").unwrap();
    book.set_input(second, cell("A1"), "='Foglio1'!A1*2", Locale::English)
        .unwrap();
    book.set_input(0, cell("B1"), "='riepilogo 2026'!A1+1", Locale::English)
        .unwrap();
    assert_eq!(book.value(second, cell("A1")), Value::Number(10.0));
    assert_eq!(book.value(0, cell("B1")), Value::Number(11.0));
}
