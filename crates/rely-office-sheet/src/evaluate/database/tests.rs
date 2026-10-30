// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use crate::address::CellRef;
use crate::model::Workbook;
use crate::value::{ErrorKind, Locale, Value};

/// Criteria in A1:F3 and the orchard of Excel's own example in A5:E11.
const DATA: &[(&str, &str)] = &[
    ("A1", "Tree"),
    ("B1", "Height"),
    ("C1", "Age"),
    ("D1", "Yield"),
    ("E1", "Profit"),
    ("F1", "Height"),
    ("A2", "=\"=Apple\""),
    ("B2", ">10"),
    ("F2", "<16"),
    ("A3", "=\"=Pear\""),
    ("A5", "Tree"),
    ("B5", "Height"),
    ("C5", "Age"),
    ("D5", "Yield"),
    ("E5", "Profit"),
    ("A6", "Apple"),
    ("B6", "18"),
    ("C6", "20"),
    ("D6", "14"),
    ("E6", "105"),
    ("A7", "Pear"),
    ("B7", "12"),
    ("C7", "12"),
    ("D7", "10"),
    ("E7", "96"),
    ("A8", "Cherry"),
    ("B8", "13"),
    ("C8", "14"),
    ("D8", "9"),
    ("E8", "105"),
    ("A9", "Apple"),
    ("B9", "14"),
    ("C9", "15"),
    ("D9", "10"),
    ("E9", "75"),
    ("A10", "Pear"),
    ("B10", "9"),
    ("C10", "8"),
    ("D10", "8"),
    ("E10", "76.8"),
    ("A11", "Apple"),
    ("B11", "8"),
    ("C11", "9"),
    ("D11", "6"),
    ("E11", "45"),
    ("H1", "Tree"),
    ("H2", "Ap"),
];

fn value(formula: &str) -> Value {
    let mut workbook = Workbook::new();
    let inputs: Vec<(CellRef, String)> = DATA
        .iter()
        .map(|(address, typed)| (CellRef::parse(address).unwrap(), (*typed).to_owned()))
        .collect();
    workbook.set_inputs(0, &inputs, Locale::English).unwrap();
    let target = CellRef::parse("Z99").unwrap();
    workbook
        .set_input(0, target, formula, Locale::English)
        .unwrap();
    workbook.value(0, target)
}

fn close(formula: &str, expected: f64) {
    match value(formula) {
        Value::Number(number) => assert!(
            (number - expected).abs() <= 1e-9,
            "{formula} gave {number}, expected {expected}"
        ),
        other => panic!("{formula} gave {other:?}"),
    }
}

#[test]
fn computes_excels_orchard_example() {
    close("=DCOUNT(A5:E11,\"Age\",A1:F2)", 1.0);
    close("=DCOUNTA(A5:E11,\"Profit\",A1:F2)", 1.0);
    close("=DMAX(A5:E11,\"Profit\",A1:A3)", 105.0);
    close("=DMIN(A5:E11,\"Profit\",A1:B2)", 75.0);
    close("=DSUM(A5:E11,\"Profit\",A1:A2)", 225.0);
    close("=DSUM(A5:E11,\"Profit\",A1:F2)", 75.0);
    close("=DPRODUCT(A5:E11,\"Yield\",A1:F2)", 10.0);
    close("=DAVERAGE(A5:E11,\"Yield\",A1:B2)", 12.0);
    close("=DAVERAGE(A5:E11,3,A5:E11)", 13.0);
    close("=DSTDEV(A5:E11,\"Yield\",A1:A3)", 2.966_479_394_838_265);
    close("=DSTDEVP(A5:E11,\"Yield\",A1:A3)", 2.653_299_832_284_32);
    close("=DVAR(A5:E11,\"Yield\",A1:A3)", 8.8);
    close("=DVARP(A5:E11,\"Yield\",A1:A3)", 7.04);
    assert_eq!(
        value("=DGET(A5:E11,\"Yield\",A1:A3)"),
        Value::Error(ErrorKind::Num)
    );
}

#[test]
fn a_plain_text_criterion_matches_what_begins_with_it() {
    close("=DSUM(A5:E11,\"Profit\",H1:H2)", 225.0);
    close("=DCOUNT(A5:E11,,H1:H2)", 3.0);
}

#[test]
fn dget_and_the_errors_of_a_wrong_field() {
    assert_eq!(value("=DGET(A5:E11,\"Age\",A1:F2)"), Value::Number(15.0));
    assert_eq!(
        value("=DGET(A5:E11,\"Age\",A1:B2)"),
        Value::Error(ErrorKind::Num)
    );
    assert_eq!(
        value("=DSUM(A5:E11,\"Colour\",A1:A2)"),
        Value::Error(ErrorKind::Value)
    );
    assert_eq!(
        value("=DSUM(A5:E11,9,A1:A2)"),
        Value::Error(ErrorKind::Value)
    );
    assert_eq!(
        value("=DSUM(A5:E11,\"Profit\")"),
        Value::Error(ErrorKind::Value)
    );
    close("=DPRODUCT(A5:E11,\"Yield\",H1:H1)", 604_800.0);
    assert_eq!(
        value("=DAVERAGE(A5:E11,\"Tree\",A1:A2)"),
        Value::Error(ErrorKind::Div0)
    );
}
