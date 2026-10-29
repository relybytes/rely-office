// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::year_fraction;
use crate::address::CellRef;
use crate::dates::serial_from_date;
use crate::model::Workbook;
use crate::value::{ErrorKind, Locale, Value};

fn value(formula: &str) -> Value {
    let mut workbook = Workbook::new();
    let target = CellRef::parse("Z99").unwrap();
    workbook
        .set_input(0, target, formula, Locale::English)
        .unwrap();
    workbook.value(0, target)
}

fn close(formula: &str, expected: f64, tolerance: f64) {
    match value(formula) {
        Value::Number(number) => assert!(
            (number - expected).abs() <= tolerance,
            "{formula} gave {number}, expected {expected}"
        ),
        other => panic!("{formula} gave {other:?}"),
    }
}

fn error(formula: &str) -> ErrorKind {
    match value(formula) {
        Value::Error(kind) => kind,
        other => panic!("{formula} gave {other:?}"),
    }
}

fn date(year: i64, month: i64, day: i64) -> f64 {
    serial_from_date(year, month, day).unwrap()
}

#[test]
fn counts_years_on_every_basis() {
    let fraction = |start: f64, end: f64, basis| year_fraction(start, end, basis).unwrap();
    let (start, end) = (date(2012, 1, 1), date(2012, 7, 30));
    assert!((fraction(start, end, 0) - 0.580_555_556).abs() < 1e-9);
    assert!((fraction(start, end, 1) - 0.576_502_732).abs() < 1e-9);
    assert!((fraction(start, end, 3) - 0.578_082_192).abs() < 1e-9);
    assert!((fraction(end, start, 2) - 211.0 / 360.0).abs() < 1e-12);
    // Two last days of February count as a whole month on the US basis.
    assert!((fraction(date(2011, 2, 28), date(2012, 2, 29), 0) - 1.0).abs() < 1e-12);
    // Within a year spanning two calendar years, a year is 366 days only with 29 February.
    assert!((fraction(date(2007, 11, 1), date(2008, 2, 1), 1) - 92.0 / 365.0).abs() < 1e-12);
    assert!((fraction(date(2007, 11, 1), date(2008, 3, 1), 1) - 121.0 / 366.0).abs() < 1e-12);
    assert_eq!(year_fraction(start, end, 5), Err(ErrorKind::Num));
}

#[test]
fn finds_the_coupons_of_a_bond() {
    let bond = "DATE(2011,1,25),DATE(2011,11,15),2,1";
    close(&format!("=COUPDAYBS({bond})"), 71.0, 0.0);
    close(&format!("=COUPDAYS({bond})"), 181.0, 0.0);
    close(&format!("=COUPDAYSNC({bond})"), 110.0, 0.0);
    close(&format!("=COUPNCD({bond})"), 40_678.0, 0.0);
    close(&format!("=COUPNUM({bond})"), 2.0, 0.0);
    close(&format!("=COUPPCD({bond})"), 40_497.0, 0.0);
    close(
        "=COUPDAYS(DATE(2011,1,25),DATE(2011,11,15),2,0)",
        180.0,
        0.0,
    );
    close(
        "=COUPDAYBS(DATE(2011,1,25),DATE(2011,11,15),2,0)",
        70.0,
        0.0,
    );
    close(
        "=COUPDAYSNC(DATE(2011,1,25),DATE(2011,11,15),2,0)",
        110.0,
        0.0,
    );
    // A maturity on the last day of a month keeps every coupon on the last day.
    close("=COUPPCD(DATE(2011,3,15),DATE(2011,8,31),2)", 40_602.0, 0.0);
    assert_eq!(
        error("=COUPNUM(DATE(2011,11,15),DATE(2011,1,25),2)"),
        ErrorKind::Num
    );
    assert_eq!(
        error("=COUPNUM(DATE(2011,1,25),DATE(2011,11,15),3)"),
        ErrorKind::Num
    );
    assert_eq!(
        error("=COUPNUM(DATE(2011,1,25),DATE(2011,11,15),2,5)"),
        ErrorKind::Num
    );
}

#[test]
fn prices_a_bond_and_finds_its_yield_and_duration() {
    close(
        "=PRICE(DATE(2008,2,15),DATE(2017,11,15),0.0575,0.065,100,2,0)",
        94.634_361_9,
        1e-6,
    );
    close(
        "=YIELD(DATE(2008,2,15),DATE(2016,11,15),0.0575,95.04287,100,2,0)",
        0.065,
        1e-7,
    );
    close(
        "=YIELD(DATE(2008,2,15),DATE(2008,11,15),0.0575,99.5,100,2,0)",
        0.064_277_518,
        1e-8,
    );
    close(
        "=YIELD(DATE(2008,8,15),DATE(2008,11,15),0.0575,99.5,100,2,0)",
        0.076_780_186,
        1e-8,
    );
    close(
        "=DURATION(DATE(2018,7,1),DATE(2048,1,1),0.08,0.09,2,1)",
        10.919_145_3,
        1e-6,
    );
    close(
        "=MDURATION(DATE(2008,1,1),DATE(2016,1,1),0.08,0.09,2,1)",
        5.735_669_814,
        1e-6,
    );
    assert_eq!(
        error("=PRICE(DATE(2008,2,15),DATE(2017,11,15),-0.1,0.065,100,2)"),
        ErrorKind::Num
    );
}

#[test]
fn accrues_interest() {
    close(
        "=ACCRINT(DATE(2008,3,1),DATE(2008,8,31),DATE(2008,5,1),0.1,1000,2,0)",
        16.666_666_667,
        1e-6,
    );
    close(
        "=ACCRINT(DATE(2008,3,5),DATE(2008,8,31),DATE(2008,5,1),0.1,1000,2,0,FALSE)",
        15.555_555_556,
        1e-6,
    );
    close(
        "=ACCRINT(DATE(2008,4,5),DATE(2008,8,31),DATE(2008,5,1),0.1,1000,2,0,TRUE)",
        7.222_222_222,
        1e-6,
    );
    close(
        "=ACCRINTM(DATE(2008,4,1),DATE(2008,6,15),0.1,1000,3)",
        20.547_945_21,
        1e-6,
    );
    assert_eq!(
        error("=ACCRINTM(DATE(2008,6,15),DATE(2008,4,1),0.1,1000,3)"),
        ErrorKind::Num
    );
}

#[test]
fn computes_discounted_securities_and_treasury_bills() {
    close(
        "=DISC(DATE(2018,7,1),DATE(2048,1,1),97.975,100,1)",
        0.000_686_384,
        1e-9,
    );
    close(
        "=INTRATE(DATE(2008,2,15),DATE(2008,5,15),1000000,1014420,2)",
        0.057_680_000,
        1e-6,
    );
    close(
        "=RECEIVED(DATE(2008,2,15),DATE(2008,5,15),1000000,0.0575,2)",
        1_014_584.654,
        1e-3,
    );
    close(
        "=PRICEDISC(DATE(2008,2,16),DATE(2008,3,1),0.0525,100,2)",
        99.795_833_33,
        1e-6,
    );
    close(
        "=YIELDDISC(DATE(2008,2,16),DATE(2008,3,1),99.795,100,2)",
        0.052_822_571,
        1e-8,
    );
    close(
        "=PRICEMAT(DATE(2008,2,15),DATE(2008,4,13),DATE(2007,11,11),0.061,0.061,0)",
        99.984_498_88,
        1e-6,
    );
    close(
        "=YIELDMAT(DATE(2008,3,15),DATE(2008,11,3),DATE(2007,11,8),0.0625,100.0123,0)",
        0.060_954_334,
        1e-7,
    );
    close(
        "=TBILLEQ(DATE(2008,3,31),DATE(2008,6,1),0.0914)",
        0.094_151_494,
        1e-8,
    );
    close(
        "=TBILLPRICE(DATE(2008,3,31),DATE(2008,6,1),0.09)",
        98.45,
        1e-9,
    );
    close(
        "=TBILLYIELD(DATE(2008,3,31),DATE(2008,6,1),98.45)",
        0.091_416_963,
        1e-8,
    );
    assert_eq!(
        error("=TBILLPRICE(DATE(2008,3,31),DATE(2010,6,1),0.09)"),
        ErrorKind::Num
    );
    assert_eq!(
        error("=DISC(DATE(2018,7,1),DATE(2048,1,1),0,100,1)"),
        ErrorKind::Num
    );
}

#[test]
fn depreciates_the_french_way_and_by_variable_balance() {
    close(
        "=AMORDEGRC(2400,DATE(2008,8,19),DATE(2008,12,31),300,1,0.15,1)",
        776.0,
        0.0,
    );
    close(
        "=AMORLINC(2400,DATE(2008,8,19),DATE(2008,12,31),300,1,0.15,1)",
        360.0,
        1e-9,
    );
    close(
        "=AMORLINC(2400,DATE(2008,8,19),DATE(2008,12,31),300,0,0.15,1)",
        131.803_278_7,
        1e-6,
    );
    close("=VDB(2400,300,10*365,0,1)", 1.315_068_493, 1e-8);
    close("=VDB(2400,300,10*12,0,1)", 40.0, 1e-9);
    close("=VDB(2400,300,10,0,1)", 480.0, 1e-9);
    close("=VDB(2400,300,10*12,6,18)", 396.306_053_7, 1e-6);
    close("=VDB(2400,300,10*12,6,18,1.5)", 311.808_936_7, 1e-6);
    close("=VDB(2400,300,10,0,0.875,1.5)", 315.0, 1e-9);
    close("=VDB(2400,300,10,1,3,2,TRUE)", 691.2, 1e-9);
    assert_eq!(error("=VDB(2400,300,10,5,3)"), ErrorKind::Num);
    assert_eq!(
        error("=AMORLINC(2400,DATE(2008,8,19),DATE(2008,12,31),300,1,0.15,2)"),
        ErrorKind::Num
    );
}
