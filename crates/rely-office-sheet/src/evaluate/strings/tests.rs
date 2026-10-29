// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::{encode_url, full_width, half_width};
use crate::address::CellRef;
use crate::model::Workbook;
use crate::value::{ErrorKind, Locale, Value};

fn book() -> Workbook {
    let mut workbook = Workbook::new();
    workbook
        .set_input(
            0,
            CellRef::new(0, 0),
            "Città 2026, ordine 41 e 2027",
            Locale::English,
        )
        .unwrap();
    workbook
}

fn value(formula: &str) -> Value {
    let mut workbook = book();
    let target = CellRef::parse("Z99").unwrap();
    workbook
        .set_input(0, target, formula, Locale::English)
        .unwrap();
    workbook.value(0, target)
}

fn text(formula: &str) -> String {
    match value(formula) {
        Value::Text(text) => text,
        other => panic!("{formula} gave {other:?}"),
    }
}

/// The values a formula spills from H1, row by row.
fn spilled(formula: &str) -> Vec<Vec<Value>> {
    let mut workbook = book();
    let anchor = CellRef::parse("H1").unwrap();
    workbook
        .set_input(0, anchor, formula, Locale::English)
        .unwrap();
    match workbook
        .spills(0)
        .into_iter()
        .find(|(cell, _)| *cell == anchor)
    {
        Some((_, range)) => (range.first_row..=range.last_row)
            .map(|row| {
                (range.first_column..=range.last_column)
                    .map(|column| workbook.value(0, CellRef::new(row, column)))
                    .collect()
            })
            .collect(),
        None => vec![vec![workbook.value(0, anchor)]],
    }
}

fn texts(rows: &[&[&str]]) -> Vec<Vec<Value>> {
    rows.iter()
        .map(|row| {
            row.iter()
                .map(|text| Value::Text((*text).to_owned()))
                .collect()
        })
        .collect()
}

#[test]
fn tests_text_against_a_pattern() {
    assert_eq!(value("=REGEXTEST(\"Hello\",\"^h\")"), Value::Bool(false));
    assert_eq!(value("=REGEXTEST(\"Hello\",\"^h\",1)"), Value::Bool(true));
    assert_eq!(value("=REGEXTEST(A1,\"\\d{4}\")"), Value::Bool(true));
    // Word characters are Unicode ones: an accented letter is one.
    assert_eq!(value("=REGEXTEST(\"Città\",\"^\\w+$\")"), Value::Bool(true));
    assert_eq!(
        value("=REGEXTEST(\"x\",\"(\")"),
        Value::Error(ErrorKind::Value)
    );
    assert_eq!(
        value("=REGEXTEST(\"x\",\"x\",2)"),
        Value::Error(ErrorKind::Value)
    );
}

#[test]
fn extracts_the_first_match_every_match_or_the_groups() {
    assert_eq!(text("=REGEXEXTRACT(A1,\"\\d+\")"), "2026");
    assert_eq!(
        spilled("=REGEXEXTRACT(A1,\"\\d+\",1)"),
        texts(&[&["2026"], &["41"], &["2027"]])
    );
    assert_eq!(
        spilled("=REGEXEXTRACT(A1,\"(\\w+) (\\d+)\",2)"),
        texts(&[&["Città", "2026"]])
    );
    assert_eq!(spilled("=REGEXEXTRACT(A1,\"\\d+\",2)"), texts(&[&["2026"]]));
    assert_eq!(
        spilled("=REGEXEXTRACT(A1,\"z+\")"),
        vec![vec![Value::Error(ErrorKind::NotAvailable)]]
    );
    assert_eq!(
        spilled("=REGEXEXTRACT(A1,\"\\d+\",5)"),
        vec![vec![Value::Error(ErrorKind::Value)]]
    );
}

#[test]
fn replaces_every_match_or_one_of_them() {
    assert_eq!(text("=REGEXREPLACE(\"a1b2c3\",\"\\d\",\"#\")"), "a#b#c#");
    assert_eq!(text("=REGEXREPLACE(\"a1b2c3\",\"\\d\",\"#\",2)"), "a1b#c3");
    assert_eq!(text("=REGEXREPLACE(\"a1b2c3\",\"\\d\",\"#\",-1)"), "a1b2c#");
    assert_eq!(text("=REGEXREPLACE(\"a1b2c3\",\"\\d\",\"#\",9)"), "a1b2c3");
    assert_eq!(
        text("=REGEXREPLACE(\"Rossi, Mario\",\"(\\w+), (\\w+)\",\"$2 $1\")"),
        "Mario Rossi"
    );
    assert_eq!(text("=REGEXREPLACE(\"ABC\",\"b\",\"x\",0,1)"), "AxC");
    assert_eq!(
        value("=REGEXREPLACE(REPT(\"a\",30000),\"a\",\"bb\")"),
        Value::Error(ErrorKind::Value)
    );
}

#[test]
fn byte_functions_count_characters() {
    assert_eq!(value("=LENB(\"città\")"), Value::Number(5.0));
    assert_eq!(text("=LEFTB(\"città\",2)"), "ci");
    assert_eq!(text("=RIGHTB(\"città\",2)"), "tà");
    assert_eq!(text("=MIDB(\"città\",2,3)"), "itt");
    assert_eq!(value("=FINDB(\"t\",\"città\")"), Value::Number(3.0));
    assert_eq!(value("=SEARCHB(\"T\",\"città\")"), Value::Number(3.0));
    assert_eq!(text("=REPLACEB(\"città\",1,1,\"C\")"), "Città");
    assert_eq!(value("=LEFTB(\"a\",-1)"), Value::Error(ErrorKind::Value));
}

#[test]
fn converts_width_and_encodes_urls() {
    assert_eq!(half_width("ＡＢＣ\u{3000}１"), "ABC 1");
    assert_eq!(full_width("AB 1"), "ＡＢ\u{3000}１");
    assert_eq!(text("=ASC(\"ｒｅｌｙ\")"), "rely");
    assert_eq!(text("=DBCS(\"rely\")"), "ｒｅｌｙ");
    assert_eq!(encode_url("a b/c?d=è~"), "a%20b%2Fc%3Fd%3D%C3%A8~");
    assert_eq!(
        text("=ENCODEURL(\"http://x.it/a b\")"),
        "http%3A%2F%2Fx.it%2Fa%20b"
    );
}

#[test]
fn writes_values_and_arrays_as_text() {
    assert_eq!(text("=VALUETOTEXT(\"x\")"), "x");
    assert_eq!(text("=VALUETOTEXT(\"x\",1)"), "\"x\"");
    assert_eq!(text("=VALUETOTEXT(12.5)"), "12.5");
    assert_eq!(text("=VALUETOTEXT(TRUE,1)"), "TRUE");
    assert_eq!(text("=VALUETOTEXT(1/0)"), "#DIV/0!");
    assert_eq!(value("=VALUETOTEXT(1,2)"), Value::Error(ErrorKind::Value));
    assert_eq!(text("=ARRAYTOTEXT({1,\"a\";TRUE,2})"), "1, a, TRUE, 2");
    assert_eq!(text("=ARRAYTOTEXT({1,\"a\";TRUE,2},1)"), "{1,\"a\";TRUE,2}");
}
