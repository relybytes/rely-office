// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Money written out: `EUROCONVERT` between the euro and the currencies it replaced, at the
//! rates fixed when each joined, and `BAHTTEXT`, an amount in Thai words.

#![allow(
    clippy::float_cmp,
    reason = "an amount of whole satang and a precision are compared exactly"
)]

use super::Evaluator;
use crate::formula::Expr;
use crate::value::{ErrorKind, Value};

type Answer = Option<Result<Value, ErrorKind>>;

/// The currencies the euro replaced: code, units per euro and the decimals an amount is
/// rounded to.
const EURO_RATES: &[(&str, f64, i32)] = &[
    ("ATS", 13.7603, 2),
    ("BEF", 40.3399, 0),
    ("CYP", 0.585_274, 2),
    ("DEM", 1.955_83, 2),
    ("EEK", 15.6466, 2),
    ("ESP", 166.386, 0),
    ("EUR", 1.0, 2),
    ("FIM", 5.945_73, 2),
    ("FRF", 6.559_57, 2),
    ("GRD", 340.75, 2),
    ("IEP", 0.787_564, 2),
    ("ITL", 1_936.27, 0),
    ("LTL", 3.4528, 2),
    ("LUF", 40.3399, 0),
    ("LVL", 0.702_804, 2),
    ("MTL", 0.4293, 2),
    ("NLG", 2.203_71, 2),
    ("PTE", 200.482, 2),
    ("SIT", 239.64, 2),
    ("SKK", 30.126, 2),
];

fn euro_rate(code: &str) -> Option<(f64, i32)> {
    let code = code.trim().to_ascii_uppercase();
    EURO_RATES
        .iter()
        .find(|(name, _, _)| *name == code)
        .map(|(_, rate, decimals)| (*rate, *decimals))
}

fn round_to(value: f64, decimals: i32) -> f64 {
    let scale = 10_f64.powi(decimals);
    let scaled = value * scale;
    // Half away from zero, after the representation error of the product is taken off.
    let rounded = (scaled + scaled.signum() * 1e-9 * scaled.abs().max(1.0)).round();
    rounded / scale
}

/// An amount converted between two of the currencies, through the euro; `triangulation`
/// rounds the euro amount in between when neither currency is the euro.
pub(super) fn euro_convert(
    amount: f64,
    source: &str,
    target: &str,
    full_precision: bool,
    triangulation: Option<f64>,
) -> Result<f64, ErrorKind> {
    let (source_rate, _) = euro_rate(source).ok_or(ErrorKind::Value)?;
    let (target_rate, target_decimals) = euro_rate(target).ok_or(ErrorKind::Value)?;
    if source.trim().eq_ignore_ascii_case(target.trim()) {
        return Ok(amount);
    }
    let mut euros = amount / source_rate;
    if let Some(precision) = triangulation
        && source_rate != 1.0
        && target_rate != 1.0
    {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a precision checked to be small"
        )]
        let precision = precision as i32;
        euros = round_to(euros, precision);
    }
    let converted = euros * target_rate;
    Ok(if full_precision {
        converted
    } else {
        round_to(converted, target_decimals)
    })
}

const THAI_DIGITS: [&str; 10] = [
    "ศูนย์",
    "หนึ่ง",
    "สอง",
    "สาม",
    "สี่",
    "ห้า",
    "หก",
    "เจ็ด",
    "แปด",
    "เก้า",
];
const THAI_POWERS: [&str; 6] = ["", "สิบ", "ร้อย", "พัน", "หมื่น", "แสน"];
const THAI_MILLION: &str = "ล้าน";
const THAI_TWENTY: &str = "ยี่";
const THAI_ONE_AFTER_TENS: &str = "เอ็ด";
const THAI_BAHT: &str = "บาท";
const THAI_SATANG: &str = "สตางค์";
const THAI_EXACT: &str = "ถ้วน";
const THAI_MINUS: &str = "ลบ";

/// A number from 1 to 999 999 in Thai words.
fn thai_block(value: u64, text: &mut String) {
    for power in (2..=5).rev() {
        let digit = (value / 10_u64.pow(power)) % 10;
        if digit > 0 {
            text.push_str(THAI_DIGITS[usize::try_from(digit).unwrap_or(0)]);
            text.push_str(THAI_POWERS[power as usize]);
        }
    }
    let tens = (value / 10) % 10;
    match tens {
        0 => {}
        1 => text.push_str(THAI_POWERS[1]),
        2 => {
            text.push_str(THAI_TWENTY);
            text.push_str(THAI_POWERS[1]);
        }
        _ => {
            text.push_str(THAI_DIGITS[usize::try_from(tens).unwrap_or(0)]);
            text.push_str(THAI_POWERS[1]);
        }
    }
    let units = value % 10;
    if units == 1 && value >= 10 {
        text.push_str(THAI_ONE_AFTER_TENS);
    } else if units > 0 {
        text.push_str(THAI_DIGITS[usize::try_from(units).unwrap_or(0)]);
    }
}

/// An amount in Thai words: baht and satang, the satang rounded to two decimals.
pub(super) fn baht_text(amount: f64) -> Result<String, ErrorKind> {
    let negative = amount < 0.0;
    let satang_total = (amount.abs() * 100.0 + 0.5).floor();
    if !satang_total.is_finite() || satang_total >= 1e17 {
        return Err(ErrorKind::Value);
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a non-negative whole amount checked against the cap"
    )]
    let satang_total = satang_total as u64;
    let (mut baht, satang) = (satang_total / 100, satang_total % 100);
    let mut text = String::new();
    if baht == 0 {
        if satang == 0 {
            text.push_str(THAI_DIGITS[0]);
        }
    } else {
        while baht > 0 {
            let block = baht % 1_000_000;
            baht /= 1_000_000;
            let mut words = String::new();
            if baht > 0 {
                words.push_str(THAI_MILLION);
            }
            if block > 0 {
                thai_block(block, &mut words);
            }
            text.insert_str(0, &words);
        }
    }
    if !text.is_empty() {
        text.push_str(THAI_BAHT);
    }
    if satang == 0 {
        text.push_str(THAI_EXACT);
    } else {
        thai_block(satang, &mut text);
        text.push_str(THAI_SATANG);
    }
    if negative {
        text.insert_str(0, THAI_MINUS);
    }
    Ok(text)
}

impl Evaluator<'_> {
    /// A currency function, or `None` when the name is none of them.
    pub(super) fn currency(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        let argument = |index: usize| arguments.get(index);
        Some(match name {
            "EUROCONVERT" => (|| {
                if !(3..=5).contains(&arguments.len()) {
                    return Err(ErrorKind::Value);
                }
                let amount = self.number(sheet, argument(0))?;
                let (source, target) = (
                    self.text(sheet, argument(1))?,
                    self.text(sheet, argument(2))?,
                );
                let full_precision = match argument(3) {
                    None => false,
                    Some(Expr::Text(text)) if text.is_empty() => false,
                    Some(expression) => self.scalar(sheet, expression).as_bool()?,
                };
                let triangulation = match argument(4) {
                    None => None,
                    Some(Expr::Text(text)) if text.is_empty() => None,
                    Some(expression) => {
                        let precision = self.scalar(sheet, expression).as_number()?.trunc();
                        if !(3.0..=15.0).contains(&precision) {
                            return Err(ErrorKind::Value);
                        }
                        Some(precision)
                    }
                };
                euro_convert(amount, &source, &target, full_precision, triangulation)
                    .map(Value::Number)
            })(),
            "BAHTTEXT" => (|| {
                if arguments.len() != 1 {
                    return Err(ErrorKind::Value);
                }
                baht_text(self.number(sheet, argument(0))?).map(Value::Text)
            })(),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests;
