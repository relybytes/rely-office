// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use super::{baht_text, euro_convert};
use crate::address::CellRef;
use crate::model::Workbook;
use crate::value::{ErrorKind, Locale, Value};

fn value(formula: &str) -> Value {
    let mut workbook = Workbook::new();
    let target = CellRef::parse("A1").unwrap();
    workbook
        .set_input(0, target, formula, Locale::English)
        .unwrap();
    workbook.value(0, target)
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "{actual} against {expected}"
    );
}

#[test]
fn converts_between_the_currencies_the_euro_replaced() {
    // Every value as LibreOffice computes it.
    let convert = |amount, source, target| euro_convert(amount, source, target, false, None);
    close(convert(1.0, "FRF", "DEM").unwrap(), 0.3);
    close(convert(1.0, "FRF", "EUR").unwrap(), 0.15);
    close(convert(1000.0, "ITL", "EUR").unwrap(), 0.52);
    close(convert(1000.0, "EUR", "ITL").unwrap(), 1_936_270.0);
    close(convert(1000.0, "EUR", "BEF").unwrap(), 40_340.0);
    close(convert(1.0, "DEM", "BEF").unwrap(), 21.0);
    close(convert(1.0, "EUR", "PTE").unwrap(), 200.48);
    close(convert(1.0, "EUR", "GRD").unwrap(), 340.75);
    close(convert(1.0, "EUR", "FIM").unwrap(), 5.95);
    close(convert(1.0, "EUR", "CYP").unwrap(), 0.59);
    close(convert(1.0, "eur", "lvl").unwrap(), 0.7);
    close(
        euro_convert(1.0, "FRF", "DEM", true, Some(3.0)).unwrap(),
        0.297_286_16,
    );
    close(
        euro_convert(1.0, "FRF", "EUR", true, None).unwrap(),
        0.152_449_017_237_41,
    );
    close(
        euro_convert(100.0, "ITL", "DEM", true, Some(3.0)).unwrap(),
        0.101_703_16,
    );
    close(
        euro_convert(100.0, "ITL", "DEM", true, None).unwrap(),
        0.101_010_189_694_619,
    );
    close(convert(5.0, "DEM", "dem").unwrap(), 5.0);
    assert_eq!(convert(1.0, "XXX", "EUR"), Err(ErrorKind::Value));
}

#[test]
fn reads_its_arguments_in_a_formula() {
    assert_eq!(value("=EUROCONVERT(1,\"FRF\",\"DEM\")"), Value::Number(0.3));
    assert_eq!(
        value("=EUROCONVERT(1,\"FRF\",\"DEM\",FALSE,2)"),
        Value::Error(ErrorKind::Value)
    );
    assert_eq!(
        value("=EUROCONVERT(1,\"FRF\")"),
        Value::Error(ErrorKind::Value)
    );
    assert_eq!(
        value("=BAHTTEXT(1234)"),
        Value::Text("หนึ่งพันสองร้อยสามสิบสี่บาทถ้วน".to_owned())
    );
    assert_eq!(value("=BAHTTEXT(\"x\")"), Value::Error(ErrorKind::Value));
}

#[test]
fn writes_an_amount_in_thai_words() {
    // As LibreOffice writes them, but for 101: Thai reads a last one as "et" after a hundred
    // too, which LibreOffice does only after a ten.
    for (amount, expected) in [
        (1234.0, "หนึ่งพันสองร้อยสามสิบสี่บาทถ้วน"),
        (1234.56, "หนึ่งพันสองร้อยสามสิบสี่บาทห้าสิบหกสตางค์"),
        (0.0, "ศูนย์บาทถ้วน"),
        (0.5, "ห้าสิบสตางค์"),
        (-21.0, "ลบยี่สิบเอ็ดบาทถ้วน"),
        (1_000_001.0, "หนึ่งล้านหนึ่งบาทถ้วน"),
        (101.0, "หนึ่งร้อยเอ็ดบาทถ้วน"),
        (11.0, "สิบเอ็ดบาทถ้วน"),
        (2_000_000.0, "สองล้านบาทถ้วน"),
        (
            12_345_678.9,
            "สิบสองล้านสามแสนสี่หมื่นห้าพันหกร้อยเจ็ดสิบแปดบาทเก้าสิบสตางค์",
        ),
        (0.01, "หนึ่งสตางค์"),
        (21.21, "ยี่สิบเอ็ดบาทยี่สิบเอ็ดสตางค์"),
    ] {
        assert_eq!(baht_text(amount).unwrap(), expected, "{amount}");
    }
    assert_eq!(baht_text(1e20), Err(ErrorKind::Value));
}
