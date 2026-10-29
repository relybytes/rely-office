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

/// A workbook with A1:A3 = 1, 2, 3 and B1:B3 = 10, 20, 30.
fn book() -> Workbook {
    let mut workbook = Workbook::new();
    let inputs: Vec<(CellRef, String)> = [
        ("A1", "1"),
        ("A2", "2"),
        ("A3", "3"),
        ("B1", "10"),
        ("B2", "20"),
        ("B3", "30"),
    ]
    .iter()
    .map(|(address, typed)| (cell(address), (*typed).to_owned()))
    .collect();
    workbook.set_inputs(0, &inputs, Locale::English).unwrap();
    workbook
}

fn value_in(workbook: &mut Workbook, formula: &str) -> Value {
    let target = cell("Z99");
    workbook
        .set_input(0, target, formula, Locale::English)
        .unwrap();
    workbook.value(0, target)
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

fn error(formula: &str) -> ErrorKind {
    match value(formula) {
        Value::Error(kind) => kind,
        other => panic!("{formula} gave {other:?}"),
    }
}

/// The values a formula spills from Z99, row by row.
fn spilled(workbook: &mut Workbook, formula: &str) -> Vec<Vec<Value>> {
    value_in(workbook, formula);
    let (_, range) = workbook
        .spills(0)
        .into_iter()
        .find(|(anchor, _)| *anchor == cell("Z99"))
        .unwrap_or_else(|| panic!("{formula} does not spill"));
    (range.first_row..=range.last_row)
        .map(|row| {
            (range.first_column..=range.last_column)
                .map(|column| workbook.value(0, CellRef::new(row, column)))
                .collect()
        })
        .collect()
}

fn numbers(rows: &[&[f64]]) -> Vec<Vec<Value>> {
    rows.iter()
        .map(|row| row.iter().map(|number| Value::Number(*number)).collect())
        .collect()
}

#[test]
fn let_binds_names_in_order() {
    assert_eq!(number("=LET(x, 2, y, x*3, x+y)"), 8.0);
    assert_eq!(number("=LET(x, A1:A3, SUM(x))"), 6.0);
    // An inner LET hides the outer name.
    assert_eq!(number("=LET(x, 1, LET(x, 5, x)+x)"), 6.0);
    // Names are read without case.
    assert_eq!(number("=LET(Rate, 0.5, rate*4)"), 2.0);
}

#[test]
fn let_refuses_a_malformed_list() {
    assert_eq!(error("=LET(x, 1)"), ErrorKind::Value);
    assert_eq!(error("=LET(x, 1, y, 2)"), ErrorKind::Value);
    assert_eq!(error("=LET(A1, 1, 2)"), ErrorKind::Value);
}

#[test]
fn an_unknown_name_is_name_error() {
    assert_eq!(error("=nothing+1"), ErrorKind::Name);
    assert_eq!(error("=NOTAFUNCTION(1)"), ErrorKind::Name);
}

#[test]
fn a_lambda_is_called_where_it_is_written() {
    assert_eq!(number("=LAMBDA(x, x*2)(21)"), 42.0);
    assert_eq!(number("=LAMBDA(x, y, x-y)(10, 4)"), 6.0);
    assert_eq!(number("=LET(double, LAMBDA(n, n*2), double(5))"), 10.0);
    // A function sees the names around the LAMBDA that made it.
    assert_eq!(
        number("=LET(k, 3, f, LAMBDA(n, n*k), LET(k, 100, f(2)))"),
        6.0
    );
    // A function called with an argument too many is #VALUE!.
    assert_eq!(error("=LAMBDA(x, x)(1, 2)"), ErrorKind::Value);
    // Its own value in a cell is #CALC!.
    assert_eq!(error("=LAMBDA(x, x)"), ErrorKind::Calc);
    // Calling what is no function is #VALUE!.
    assert_eq!(error("=(5)(1)"), ErrorKind::Value);
    assert_eq!(error("=LAMBDA(1, 2)"), ErrorKind::Value);
    assert_eq!(error("=LAMBDA()"), ErrorKind::Value);
}

#[test]
fn isomitted_tells_a_left_out_parameter() {
    assert_eq!(
        value("=LAMBDA(x, y, IF(ISOMITTED(y), x, x+y))(1)"),
        Value::Number(1.0)
    );
    assert_eq!(
        value("=LAMBDA(x, y, IF(ISOMITTED(y), x, x+y))(1, 2)"),
        Value::Number(3.0)
    );
    assert_eq!(value("=LAMBDA(x, y, ISOMITTED(y))(1,)"), Value::Bool(true));
    assert_eq!(value("=ISOMITTED(5)"), Value::Bool(false));
}

#[test]
fn map_calls_a_function_on_every_cell() {
    let mut workbook = book();
    assert_eq!(
        spilled(&mut workbook, "=MAP(A1:A3, LAMBDA(x, x*x))"),
        numbers(&[&[1.0], &[4.0], &[9.0]])
    );
    assert_eq!(
        spilled(&mut workbook, "=MAP(A1:A3, B1:B3, LAMBDA(a, b, a+b))"),
        numbers(&[&[11.0], &[22.0], &[33.0]])
    );
    // A function that gives an array for one cell is #CALC! there.
    assert_eq!(
        spilled(&mut workbook, "=MAP({1;2}, LAMBDA(x, {1,2}))"),
        vec![
            vec![Value::Error(ErrorKind::Calc)],
            vec![Value::Error(ErrorKind::Calc)]
        ]
    );
    assert_eq!(error("=MAP(A1:A3, 5)"), ErrorKind::Value);
    assert_eq!(error("=MAP(LAMBDA(x, x))"), ErrorKind::Value);
}

#[test]
fn reduce_and_scan_carry_an_accumulator() {
    assert_eq!(number("=REDUCE(0, A1:B3, LAMBDA(total, x, total+x))"), 66.0);
    assert_eq!(number("=REDUCE(, A1:A3, LAMBDA(total, x, total+x))"), 6.0);
    let mut workbook = book();
    assert_eq!(
        spilled(
            &mut workbook,
            "=SCAN(1, A1:A3, LAMBDA(product, x, product*x))"
        ),
        numbers(&[&[1.0], &[2.0], &[6.0]])
    );
    assert_eq!(error("=REDUCE(0, A1:A3)"), ErrorKind::Value);
}

#[test]
fn byrow_and_bycol_call_a_function_on_every_line() {
    let mut workbook = book();
    assert_eq!(
        spilled(&mut workbook, "=BYROW(A1:B3, LAMBDA(row, SUM(row)))"),
        numbers(&[&[11.0], &[22.0], &[33.0]])
    );
    assert_eq!(
        spilled(&mut workbook, "=BYCOL(A1:B3, LAMBDA(column, MAX(column)))"),
        numbers(&[&[3.0, 30.0]])
    );
    assert_eq!(error("=BYROW(A1:B3)"), ErrorKind::Value);
}

#[test]
fn makearray_builds_an_array_from_its_places() {
    let mut workbook = book();
    assert_eq!(
        spilled(&mut workbook, "=MAKEARRAY(2, 3, LAMBDA(r, c, r*10+c))"),
        numbers(&[&[11.0, 12.0, 13.0], &[21.0, 22.0, 23.0]])
    );
    assert_eq!(error("=MAKEARRAY(0, 3, LAMBDA(r, c, 1))"), ErrorKind::Value);
    assert_eq!(
        error("=MAKEARRAY(5000, 5000, LAMBDA(r, c, 1))"),
        ErrorKind::Spill
    );
    assert_eq!(error("=MAKEARRAY(2, 2, 1)"), ErrorKind::Value);
}

#[test]
fn a_defined_name_is_read_by_formulas() {
    let mut workbook = book();
    workbook
        .set_defined_name("Rate", None, "=0.5", Locale::English)
        .unwrap();
    workbook
        .set_defined_name("Prices", None, "A1:A3", Locale::English)
        .unwrap();
    assert_eq!(
        value_in(&mut workbook, "=SUM(Prices)*rate"),
        Value::Number(3.0)
    );
    // A name changes and the formulas that read it follow.
    workbook
        .set_defined_name("RATE", None, "2", Locale::English)
        .unwrap();
    assert_eq!(workbook.value(0, cell("Z99")), Value::Number(12.0));
    // A cell a name refers to changes and the formulas follow.
    workbook
        .set_input(0, cell("A1"), "11", Locale::English)
        .unwrap();
    assert_eq!(workbook.value(0, cell("Z99")), Value::Number(32.0));
    assert!(workbook.remove_defined_name("rate", None));
    assert!(!workbook.remove_defined_name("rate", None));
    assert_eq!(
        workbook.value(0, cell("Z99")),
        Value::Error(ErrorKind::Name)
    );
}

#[test]
fn a_named_function_is_called_by_its_name() {
    let mut workbook = book();
    workbook
        .set_defined_name(
            "Hypotenuse",
            None,
            "LAMBDA(a, b, SQRT(a^2+b^2))",
            Locale::English,
        )
        .unwrap();
    assert_eq!(
        value_in(&mut workbook, "=Hypotenuse(3, 4)"),
        Value::Number(5.0)
    );
    workbook
        .set_defined_name(
            "Factorial",
            None,
            "LAMBDA(n, IF(n<=1, 1, n*Factorial(n-1)))",
            Locale::English,
        )
        .unwrap();
    assert_eq!(
        value_in(&mut workbook, "=Factorial(5)"),
        Value::Number(120.0)
    );
    // A recursion that never ends stops at the depth limit.
    workbook
        .set_defined_name("Forever", None, "LAMBDA(n, Forever(n))", Locale::English)
        .unwrap();
    assert_eq!(
        value_in(&mut workbook, "=Forever(1)"),
        Value::Error(ErrorKind::Num)
    );
}

#[test]
fn a_name_of_a_sheet_wins_over_the_workbooks() {
    let mut workbook = book();
    workbook.add_sheet("Other").unwrap();
    workbook
        .set_defined_name("Size", None, "1", Locale::English)
        .unwrap();
    workbook
        .set_defined_name("Size", Some(0), "2", Locale::English)
        .unwrap();
    assert_eq!(value_in(&mut workbook, "=Size"), Value::Number(2.0));
    workbook
        .set_input(1, cell("A1"), "=Size", Locale::English)
        .unwrap();
    assert_eq!(workbook.value(1, cell("A1")), Value::Number(1.0));
    assert_eq!(workbook.defined_names().len(), 2);
}
