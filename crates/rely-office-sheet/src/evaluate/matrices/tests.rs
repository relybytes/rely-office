// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::inverse;
use crate::address::CellRef;
use crate::model::Workbook;
use crate::value::{ErrorKind, Locale, Value};

/// A1:A5 the x and B1:B5 the y of a fit, D1:D6 months and E1:E6 the units sold in them.
fn book() -> Workbook {
    let mut workbook = Workbook::new();
    let mut inputs: Vec<(CellRef, String)> = Vec::new();
    for (row, (x, y)) in [(1, 2), (2, 4), (3, 5), (4, 4), (5, 5)].iter().enumerate() {
        let row = u32::try_from(row).unwrap();
        inputs.push((CellRef::new(row, 0), x.to_string()));
        inputs.push((CellRef::new(row, 1), y.to_string()));
    }
    for (row, units) in [33_100, 47_300, 69_000, 102_000, 150_000, 220_000]
        .iter()
        .enumerate()
    {
        let row = u32::try_from(row).unwrap();
        inputs.push((CellRef::new(row, 3), (row + 11).to_string()));
        inputs.push((CellRef::new(row, 4), units.to_string()));
    }
    workbook.set_inputs(0, &inputs, Locale::English).unwrap();
    workbook
}

/// The values a formula spills from H1, row by row; one value for a formula that does not.
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

fn numbers(formula: &str) -> Vec<Vec<f64>> {
    spilled(formula)
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|value| match value {
                    Value::Number(number) => number,
                    other => panic!("{formula} gave {other:?}"),
                })
                .collect()
        })
        .collect()
}

fn assert_close(found: &[Vec<f64>], expected: &[&[f64]], tolerance: f64) {
    assert_eq!(found.len(), expected.len(), "{found:?}");
    for (row, wanted) in found.iter().zip(expected) {
        assert_eq!(row.len(), wanted.len(), "{found:?}");
        for (value, wanted) in row.iter().zip(wanted.iter()) {
            assert!(
                (value - wanted).abs() <= tolerance,
                "{found:?} expected {expected:?}"
            );
        }
    }
}

#[test]
fn inverts_a_matrix() {
    assert_close(
        &numbers("=MINVERSE({4,-1;2,0})"),
        &[&[0.0, 0.5], &[-1.0, 2.0]],
        1e-12,
    );
    assert_eq!(
        spilled("=MINVERSE({1,2;2,4})"),
        vec![vec![Value::Error(ErrorKind::Num)]]
    );
    assert_eq!(
        spilled("=MINVERSE({1,2,3})"),
        vec![vec![Value::Error(ErrorKind::Value)]]
    );
    assert!(inverse(&[vec![0.0]]).is_none());
}

#[test]
fn fits_a_line_with_its_statistics() {
    assert_close(
        &numbers("=LINEST({1,9,5,7},{0,4,2,3})"),
        &[&[2.0, 1.0]],
        1e-12,
    );
    assert_close(
        &numbers("=LINEST(B1:B5,A1:A5,FALSE)"),
        &[&[1.2, 0.0]],
        1e-12,
    );
    let statistics = spilled("=LINEST(B1:B5,A1:A5,TRUE,TRUE)");
    let number = |row: usize, column: usize| match &statistics[row][column] {
        Value::Number(number) => *number,
        other => panic!("{other:?} at {row}, {column}"),
    };
    assert_eq!(statistics.len(), 5);
    assert!((number(0, 0) - 0.6).abs() < 1e-12);
    assert!((number(0, 1) - 2.2).abs() < 1e-12);
    assert!((number(1, 0) - 0.282_842_712).abs() < 1e-8);
    assert!((number(1, 1) - 0.938_083_152).abs() < 1e-8);
    assert!((number(2, 0) - 0.6).abs() < 1e-12);
    assert!((number(2, 1) - 0.894_427_191).abs() < 1e-8);
    assert!((number(3, 0) - 4.5).abs() < 1e-9);
    assert!((number(3, 1) - 3.0).abs() < 1e-12);
    assert!((number(4, 0) - 3.6).abs() < 1e-9);
    assert!((number(4, 1) - 2.4).abs() < 1e-9);
    assert_eq!(
        spilled("=LINEST({1,2},{1,2,3})"),
        vec![vec![Value::Error(ErrorKind::Ref)]]
    );
    assert_eq!(
        spilled("=LINEST({1,\"a\"})"),
        vec![vec![Value::Error(ErrorKind::Value)]]
    );
}

#[test]
fn fits_two_variables_at_once() {
    assert_close(
        &numbers("=LINEST({3;5;7;10;12},{1,2;2,1;3,4;4,3;5,6})"),
        &[&[-0.083_333_333, 2.383_333_333, 0.516_666_667]],
        1e-8,
    );
}

#[test]
fn fits_an_exponential_curve() {
    assert_close(
        &numbers("=LOGEST(E1:E6,D1:D6)"),
        &[&[1.463_275_628, 495.304_770_2]],
        1e-6,
    );
    assert_eq!(
        spilled("=LOGEST({1,-2},{1,2})"),
        vec![vec![Value::Error(ErrorKind::Num)]]
    );
}

#[test]
fn predicts_along_the_fit() {
    assert_close(
        &numbers("=TREND({1,9,5,7},{0,4,2,3},{5,6})"),
        &[&[11.0, 13.0]],
        1e-9,
    );
    assert_close(
        &numbers("=TREND(B1:B5,A1:A5,{6;7})"),
        &[&[5.8], &[6.4]],
        1e-9,
    );
    assert_close(
        &numbers("=TREND(B1:B5)"),
        &[&[2.8], &[3.4], &[4.0], &[4.6], &[5.2]],
        1e-9,
    );
    assert_close(
        &numbers("=GROWTH(E1:E6,D1:D6,{17;18})"),
        &[&[320_196.718_4], &[468_536.054_2]],
        1e-3,
    );
}

#[test]
fn lists_every_mode() {
    assert_close(
        &numbers("=MODE.MULT({1,2,3,4,3,2,1,2,3})"),
        &[&[2.0], &[3.0]],
        0.0,
    );
    assert_eq!(
        spilled("=MODE.MULT({1,2,3})"),
        vec![vec![Value::Error(ErrorKind::NotAvailable)]]
    );
}

#[test]
fn draws_an_array_of_random_numbers() {
    let draws = numbers("=RANDARRAY(2,3)");
    assert_eq!(draws.len(), 2);
    assert!(draws.iter().all(|row| row.len() == 3));
    assert!(draws.iter().flatten().all(|draw| (0.0..1.0).contains(draw)));
    let dice = numbers("=RANDARRAY(4,4,1,6,TRUE)");
    assert!(
        dice.iter()
            .flatten()
            .all(|die| (1.0..=6.0).contains(die) && die.fract() == 0.0)
    );
    assert_eq!(
        spilled("=RANDARRAY(0,1)"),
        vec![vec![Value::Error(ErrorKind::Value)]]
    );
    assert_eq!(
        spilled("=RANDARRAY(1,1,5,1)"),
        vec![vec![Value::Error(ErrorKind::Value)]]
    );
    assert_eq!(
        spilled("=RANDARRAY(5000,5000)"),
        vec![vec![Value::Error(ErrorKind::Spill)]]
    );
}
