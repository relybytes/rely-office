// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The second wide set of functions: hyperbolic and combinatorial mathematics, the statistics of
//! samples, pairs and distributions, depreciation and the rates of loans, the conversions and bit
//! operations of engineering, and the text and information functions a workbook reaches for
//! after the common ones. Each answers `None` for a name that is not its own; none returns an
//! array.

#![allow(
    clippy::float_cmp,
    reason = "the flags and whole numbers compared here are read from cells, exact by construction"
)]

use super::{Evaluator, Operand};
use crate::address::column_name;
use crate::dates::date_from_serial;
use crate::formula::Expr;
use crate::value::{ErrorKind, Value};

type Answer = Option<Result<Value, ErrorKind>>;

/// A bit pattern of BITAND and its kin, checked by the caller to lie below 2^48.
fn bits_value(bits: u64) -> Value {
    #[expect(clippy::cast_precision_loss, reason = "below 2^48, exact in a double")]
    Value::Number(bits as f64)
}

const MAX_ITERATIONS: usize = 200;
const TOLERANCE: f64 = 1e-10;

/// The numbers of a value as the `A` functions read them: text counts 0, a boolean 1 or 0.
fn as_a_number(value: &Value) -> Option<Result<f64, ErrorKind>> {
    match value {
        Value::Empty => None,
        Value::Number(number) => Some(Ok(*number)),
        Value::Bool(value) => Some(Ok(if *value { 1.0 } else { 0.0 })),
        Value::Text(_) => Some(Ok(0.0)),
        Value::Error(kind) => Some(Err(*kind)),
    }
}

fn mean(values: &[f64]) -> f64 {
    #[expect(clippy::cast_precision_loss, reason = "counts stay far below 2^52")]
    let count = values.len() as f64;
    values.iter().sum::<f64>() / count
}

fn variance(values: &[f64], sample: bool) -> Result<f64, ErrorKind> {
    let minimum = if sample { 2 } else { 1 };
    if values.len() < minimum {
        return Err(ErrorKind::Div0);
    }
    let average = mean(values);
    let squares: f64 = values.iter().map(|value| (value - average).powi(2)).sum();
    #[expect(clippy::cast_precision_loss, reason = "counts stay far below 2^52")]
    let count = values.len() as f64;
    Ok(squares / if sample { count - 1.0 } else { count })
}

/// The value at a fraction of sorted values, between neighbours, as PERCENTILE.INC reads it.
fn percentile_inclusive(sorted: &[f64], fraction: f64) -> Result<f64, ErrorKind> {
    if sorted.is_empty() || !(0.0..=1.0).contains(&fraction) {
        return Err(ErrorKind::Num);
    }
    #[expect(clippy::cast_precision_loss, reason = "counts stay far below 2^52")]
    let position = fraction * (sorted.len() - 1) as f64;
    Ok(interpolate(sorted, position))
}

/// The value at a fraction of sorted values counted from outside the ends, as PERCENTILE.EXC.
fn percentile_exclusive(sorted: &[f64], fraction: f64) -> Result<f64, ErrorKind> {
    #[expect(clippy::cast_precision_loss, reason = "counts stay far below 2^52")]
    let position = fraction * (sorted.len() + 1) as f64 - 1.0;
    #[expect(clippy::cast_precision_loss, reason = "counts stay far below 2^52")]
    let last = (sorted.len() - 1) as f64;
    if sorted.is_empty() || position < 0.0 || position > last {
        return Err(ErrorKind::Num);
    }
    Ok(interpolate(sorted, position))
}

fn interpolate(sorted: &[f64], position: f64) -> f64 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the position lies within the slice"
    )]
    let low = position.floor() as usize;
    let high = (low + 1).min(sorted.len() - 1);
    let weight = position - position.floor();
    sorted[low] + (sorted[high] - sorted[low]) * weight
}

/// The error function, by Abramowitz and Stegun 7.1.26 refined to double precision with W. J.
/// Cody's rational approximations.
fn erf(x: f64) -> f64 {
    // erfc by the continued fraction of Numerical Recipes' erfccheb is overkill here: the
    // series below is accurate to 1e-15 for |x| < 6, past which erf is 1 within a double.
    if x.abs() >= 6.0 {
        return x.signum();
    }
    if x.abs() < 3.0 {
        // Maclaurin series of erf, converging fast below 3.
        let mut sum = x;
        let mut term = x;
        let square = x * x;
        let mut n = 0.0;
        loop {
            n += 1.0;
            term *= -square / n;
            let addition = term / (2.0 * n + 1.0);
            sum += addition;
            if addition.abs() < 1e-17 * sum.abs() || n > 200.0 {
                break;
            }
        }
        return 2.0 / std::f64::consts::PI.sqrt() * sum;
    }
    // A continued fraction of erfc for larger |x|.
    let z = x.abs();
    let mut fraction = 0.0;
    for k in (1..60).rev() {
        let k = f64::from(k);
        fraction = (k / 2.0) / (z + fraction);
    }
    let erfc = (-z * z).exp() / std::f64::consts::PI.sqrt() / (z + fraction);
    (1.0 - erfc) * x.signum()
}

fn normal_cdf(z: f64) -> f64 {
    f64::midpoint(1.0, erf(z / std::f64::consts::SQRT_2))
}

fn normal_pdf(z: f64) -> f64 {
    (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt()
}

/// The quantile of the standard normal distribution, Acklam's approximation with one step of
/// Halley's method, accurate to about 1e-15.
fn normal_inverse(probability: f64) -> Result<f64, ErrorKind> {
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];
    if probability <= 0.0 || probability >= 1.0 {
        return Err(ErrorKind::Num);
    }
    let low = 0.024_25;
    let x = if probability < low {
        let q = (-2.0 * probability.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if probability <= 1.0 - low {
        let q = probability - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - probability).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    };
    let error = normal_cdf(x) - probability;
    let u = error * (2.0 * std::f64::consts::PI).sqrt() * (x * x / 2.0).exp();
    Ok(x - u / (1.0 + x * u / 2.0))
}

/// The natural logarithm of the gamma function, by Lanczos' approximation.
fn gamma_ln(x: f64) -> Result<f64, ErrorKind> {
    const G: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x <= 0.0 {
        return Err(ErrorKind::Num);
    }
    if x < 0.5 {
        let sine = (std::f64::consts::PI * x).sin();
        return Ok((std::f64::consts::PI / sine).ln() - gamma_ln(1.0 - x)?);
    }
    let x = x - 1.0;
    let mut sum = G[0];
    for (index, coefficient) in G.iter().enumerate().skip(1) {
        #[expect(clippy::cast_precision_loss, reason = "index is small")]
        let index = index as f64;
        sum += coefficient / (x + index);
    }
    let t = x + 7.5;
    Ok(0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + sum.ln())
}

fn combinations(n: f64, k: f64) -> Result<f64, ErrorKind> {
    let (n, k) = (n.trunc(), k.trunc());
    if n < 0.0 || k < 0.0 || k > n {
        return Err(ErrorKind::Num);
    }
    let k = k.min(n - k);
    let mut result = 1.0;
    let mut step = 0.0;
    while step < k {
        result = result * (n - step) / (step + 1.0);
        step += 1.0;
    }
    Ok(result.round())
}

fn permutations(n: f64, k: f64) -> Result<f64, ErrorKind> {
    let (n, k) = (n.trunc(), k.trunc());
    if n < 0.0 || k < 0.0 || k > n {
        return Err(ErrorKind::Num);
    }
    let mut result = 1.0;
    let mut step = 0.0;
    while step < k {
        result *= n - step;
        step += 1.0;
    }
    Ok(result)
}

fn roman(mut number: u32) -> String {
    const NUMERALS: [(u32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut text = String::new();
    for (value, numeral) in NUMERALS {
        while number >= value {
            text.push_str(numeral);
            number -= value;
        }
    }
    text
}

fn arabic(text: &str) -> Result<f64, ErrorKind> {
    let text = text.trim().to_uppercase();
    let (negative, digits) = text
        .strip_prefix('-')
        .map_or((false, text.as_str()), |rest| (true, rest));
    let value = |numeral: char| match numeral {
        'I' => Ok(1),
        'V' => Ok(5),
        'X' => Ok(10),
        'L' => Ok(50),
        'C' => Ok(100),
        'D' => Ok(500),
        'M' => Ok(1000),
        _ => Err(ErrorKind::Value),
    };
    let numerals: Vec<i64> = digits.chars().map(value).collect::<Result<_, _>>()?;
    let mut total = 0_i64;
    for (index, current) in numerals.iter().enumerate() {
        if numerals.get(index + 1).is_some_and(|next| next > current) {
            total -= current;
        } else {
            total += current;
        }
    }
    #[expect(clippy::cast_precision_loss, reason = "a roman numeral stays small")]
    Ok(if negative {
        -(total as f64)
    } else {
        total as f64
    })
}

/// A whole number in a base from 2 to 36, padded to a length.
fn to_base(number: i64, base: u32, length: usize) -> String {
    let digits = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut value = number.unsigned_abs();
    let mut text = Vec::new();
    loop {
        #[expect(clippy::cast_possible_truncation, reason = "a digit is below 36")]
        text.push(digits[(value % u64::from(base)) as usize]);
        value /= u64::from(base);
        if value == 0 {
            break;
        }
    }
    while text.len() < length {
        text.push(b'0');
    }
    text.reverse();
    String::from_utf8(text).unwrap_or_default()
}

fn from_base(text: &str, base: u32) -> Result<i64, ErrorKind> {
    let text = text.trim();
    if text.is_empty() || text.len() > 64 {
        return Err(ErrorKind::Num);
    }
    i64::from_str_radix(text, base).map_err(|_| ErrorKind::Num)
}

/// An engineering number of up to ten digits in base 2, 8 or 16, its top digit the sign, as
/// Excel's BIN2DEC, OCT2DEC and HEX2DEC read it.
fn from_signed_base(text: &str, base: u32) -> Result<i64, ErrorKind> {
    let text = text.trim();
    if text.len() > 10 {
        return Err(ErrorKind::Num);
    }
    let value = from_base(text, base)?;
    let width = match base {
        2 => 10,
        8 => 30,
        _ => 40,
    };
    let limit = 1_i64 << width;
    Ok(if text.len() == 10 && value >= limit / 2 {
        value - limit
    } else {
        value
    })
}

/// A number written in base 2, 8 or 16 as Excel's DEC2BIN, DEC2OCT and DEC2HEX write it:
/// negatives in ten digits of two's complement.
fn to_signed_base(number: i64, base: u32, places: Option<usize>) -> Result<String, ErrorKind> {
    let (width, minimum, maximum) = match base {
        2 => (10, -512, 511),
        8 => (30, -536_870_912, 536_870_911),
        _ => (40, -549_755_813_888, 549_755_813_887),
    };
    if number < minimum || number > maximum {
        return Err(ErrorKind::Num);
    }
    if number < 0 {
        return Ok(to_base((1_i64 << width) + number, base, 10));
    }
    let text = to_base(number, base, 0);
    match places {
        Some(places) if places < text.len() || places > 10 => Err(ErrorKind::Num),
        Some(places) => Ok(to_base(number, base, places)),
        None => Ok(text),
    }
}

/// The factor a unit of CONVERT is of its family's base unit, and the family.
fn unit_factor(unit: &str) -> Option<(&'static str, f64)> {
    Some(match unit {
        "m" => ("length", 1.0),
        "km" => ("length", 1_000.0),
        "cm" => ("length", 0.01),
        "mm" => ("length", 0.001),
        "mi" => ("length", 1_609.344),
        "yd" => ("length", 0.914_4),
        "ft" => ("length", 0.304_8),
        "in" => ("length", 0.025_4),
        "Nmi" => ("length", 1_852.0),
        "g" => ("mass", 1.0),
        "kg" => ("mass", 1_000.0),
        "mg" => ("mass", 0.001),
        "lbm" => ("mass", 453.592_37),
        "ozm" => ("mass", 28.349_523_125),
        "ton" => ("mass", 907_184.74),
        "l" | "L" | "lt" => ("volume", 1.0),
        "ml" | "mL" => ("volume", 0.001),
        "gal" => ("volume", 3.785_411_784),
        "qt" => ("volume", 0.946_352_946),
        "pt" => ("volume", 0.473_176_473),
        "m3" => ("volume", 1_000.0),
        "sec" | "s" => ("time", 1.0),
        "mn" | "min" => ("time", 60.0),
        "hr" => ("time", 3_600.0),
        "day" | "d" => ("time", 86_400.0),
        "yr" => ("time", 31_557_600.0),
        "m/s" => ("speed", 1.0),
        "km/h" | "kph" => ("speed", 1_000.0 / 3_600.0),
        "mph" => ("speed", 0.447_04),
        "kn" => ("speed", 1_852.0 / 3_600.0),
        "m2" => ("area", 1.0),
        "km2" => ("area", 1_000_000.0),
        "ha" => ("area", 10_000.0),
        "ft2" => ("area", 0.092_903_04),
        "acre" | "uk_acre" => ("area", 4_046.856_422_4),
        "J" => ("energy", 1.0),
        "kJ" => ("energy", 1_000.0),
        "cal" => ("energy", 4.186_8),
        "kcal" => ("energy", 4_186.8),
        "Wh" => ("energy", 3_600.0),
        "kWh" => ("energy", 3_600_000.0),
        "W" => ("power", 1.0),
        "kW" => ("power", 1_000.0),
        "HP" | "h" => ("power", 745.699_871_582_270_2),
        "Pa" => ("pressure", 1.0),
        "kPa" => ("pressure", 1_000.0),
        "bar" => ("pressure", 100_000.0),
        "atm" => ("pressure", 101_325.0),
        "psi" => ("pressure", 6_894.757_293_168),
        "mmHg" => ("pressure", 133.322_368_421),
        "byte" => ("information", 8.0),
        "bit" => ("information", 1.0),
        _ => return None,
    })
}

fn temperature_to_kelvin(value: f64, unit: &str) -> Option<f64> {
    match unit {
        "C" | "cel" => Some(value + 273.15),
        "F" | "fah" => Some((value - 32.0) * 5.0 / 9.0 + 273.15),
        "K" | "kel" => Some(value),
        _ => None,
    }
}

fn kelvin_to(value: f64, unit: &str) -> Option<f64> {
    match unit {
        "C" | "cel" => Some(value - 273.15),
        "F" | "fah" => Some((value - 273.15) * 9.0 / 5.0 + 32.0),
        "K" | "kel" => Some(value),
        _ => None,
    }
}

/// The weekend of a WORKDAY.INTL or NETWORKDAYS.INTL code: a number from 1 to 17 or seven
/// characters of 0 and 1 from Monday. Answers which weekdays (0 Sunday) rest.
fn weekend(code: &Value) -> Result<[bool; 7], ErrorKind> {
    let mut rest = [false; 7];
    match code {
        Value::Text(mask) => {
            let mask: Vec<char> = mask.chars().collect();
            if mask.len() != 7
                || mask
                    .iter()
                    .any(|character| *character != '0' && *character != '1')
                || mask.iter().all(|character| *character == '1')
            {
                return Err(ErrorKind::Value);
            }
            for (index, character) in mask.iter().enumerate() {
                // The mask starts on Monday, weekday 1.
                rest[(index + 1) % 7] = *character == '1';
            }
        }
        other => {
            let number = other.as_number()?;
            #[expect(clippy::cast_possible_truncation, reason = "checked to lie in 1..=17")]
            let pairs: &[usize] = match number as i64 {
                1 => &[6, 0],
                2 => &[0, 1],
                3 => &[1, 2],
                4 => &[2, 3],
                5 => &[3, 4],
                6 => &[4, 5],
                7 => &[5, 6],
                11 => &[0],
                12 => &[1],
                13 => &[2],
                14 => &[3],
                15 => &[4],
                16 => &[5],
                17 => &[6],
                _ => return Err(ErrorKind::Num),
            };
            for day in pairs {
                rest[*day] = true;
            }
        }
    }
    Ok(rest)
}

impl Evaluator<'_> {
    /// A function of the second wide set, or `None` when the name is none of them.
    pub(super) fn more(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        self.more_mathematics(sheet, name, arguments)
            .or_else(|| self.more_statistics(sheet, name, arguments))
            .or_else(|| self.distributions(sheet, name, arguments))
            .or_else(|| self.more_finance(sheet, name, arguments))
            .or_else(|| self.engineering(sheet, name, arguments))
            .or_else(|| self.more_texts(sheet, name, arguments))
            .or_else(|| self.information(sheet, name, arguments))
    }

    fn optional(
        &self,
        sheet: usize,
        argument: Option<&Expr>,
        default: f64,
    ) -> Result<f64, ErrorKind> {
        match argument {
            Some(Expr::Text(text)) if text.is_empty() => Ok(default),
            Some(argument) => self.scalar(sheet, argument).as_number(),
            None => Ok(default),
        }
    }

    /// The values of the arguments as the `A` functions read them.
    fn a_numbers(&self, sheet: usize, arguments: &[Expr]) -> Result<Vec<f64>, ErrorKind> {
        let mut numbers = Vec::new();
        let mut error = None;
        for argument in arguments {
            self.values_of(sheet, argument, |value, from_range| {
                if error.is_some() {
                    return;
                }
                let read = if from_range {
                    as_a_number(&value)
                } else {
                    match value {
                        Value::Empty => None,
                        other => Some(other.as_number()),
                    }
                };
                match read {
                    Some(Ok(number)) => numbers.push(number),
                    Some(Err(kind)) => error = Some(kind),
                    None => {}
                }
            });
        }
        error.map_or(Ok(numbers), Err)
    }

    /// The pairs of numbers two same-sized arguments hold where both cells are numbers.
    fn pairs(
        &self,
        sheet: usize,
        first: Option<&Expr>,
        second: Option<&Expr>,
    ) -> Result<(Vec<f64>, Vec<f64>), ErrorKind> {
        let flatten = |grid: Vec<Vec<Value>>| grid.into_iter().flatten().collect::<Vec<_>>();
        let first = flatten(self.grid(sheet, first)?);
        let second = flatten(self.grid(sheet, second)?);
        if first.len() != second.len() {
            return Err(ErrorKind::NotAvailable);
        }
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        for (x, y) in first.iter().zip(&second) {
            if let (Value::Error(kind), _) | (_, Value::Error(kind)) = (x, y) {
                return Err(*kind);
            }
            if let (Value::Number(x), Value::Number(y)) = (x, y) {
                xs.push(*x);
                ys.push(*y);
            }
        }
        Ok((xs, ys))
    }

    #[expect(clippy::too_many_lines, reason = "one arm per function, each short")]
    fn more_mathematics(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        let argument = |index: usize| arguments.get(index);
        let number = |index: usize| self.number(sheet, argument(index));
        let unary = |apply: fn(f64) -> f64| -> Result<Value, ErrorKind> {
            let result = apply(number(0)?);
            if result.is_finite() {
                Ok(Value::Number(result))
            } else {
                Err(ErrorKind::Num)
            }
        };
        let reciprocal = |apply: fn(f64) -> f64| -> Result<Value, ErrorKind> {
            let value = apply(number(0)?);
            if value == 0.0 {
                Err(ErrorKind::Div0)
            } else {
                Ok(Value::Number(1.0 / value))
            }
        };
        Some(match name {
            "SINH" => unary(f64::sinh),
            "COSH" => unary(f64::cosh),
            "TANH" => unary(f64::tanh),
            "ASINH" => unary(f64::asinh),
            "ACOSH" => (|| {
                let value = number(0)?;
                if value < 1.0 {
                    Err(ErrorKind::Num)
                } else {
                    Ok(Value::Number(value.acosh()))
                }
            })(),
            "ATANH" => (|| {
                let value = number(0)?;
                if value.abs() >= 1.0 {
                    Err(ErrorKind::Num)
                } else {
                    Ok(Value::Number(value.atanh()))
                }
            })(),
            "COT" => reciprocal(f64::tan),
            "CSC" => reciprocal(f64::sin),
            "SEC" => reciprocal(f64::cos),
            "COTH" => reciprocal(f64::tanh),
            "CSCH" => reciprocal(f64::sinh),
            "SECH" => reciprocal(f64::cosh),
            "ACOT" => (|| {
                Ok(Value::Number(
                    std::f64::consts::FRAC_PI_2 - number(0)?.atan(),
                ))
            })(),
            "ACOTH" => (|| {
                let value = number(0)?;
                if value.abs() <= 1.0 {
                    Err(ErrorKind::Num)
                } else {
                    Ok(Value::Number(0.5 * ((value + 1.0) / (value - 1.0)).ln()))
                }
            })(),
            "ATAN2" => (|| {
                let (x, y) = (number(0)?, number(1)?);
                if x == 0.0 && y == 0.0 {
                    Err(ErrorKind::Div0)
                } else {
                    Ok(Value::Number(y.atan2(x)))
                }
            })(),
            "SQRTPI" => (|| {
                let value = number(0)?;
                if value < 0.0 {
                    Err(ErrorKind::Num)
                } else {
                    Ok(Value::Number((value * std::f64::consts::PI).sqrt()))
                }
            })(),
            "FACTDOUBLE" => (|| {
                let value = number(0)?.trunc();
                if value < 0.0 {
                    return Err(ErrorKind::Num);
                }
                let mut result = 1.0;
                let mut step = value;
                while step > 1.0 {
                    result *= step;
                    step -= 2.0;
                }
                if result.is_finite() {
                    Ok(Value::Number(result))
                } else {
                    Err(ErrorKind::Num)
                }
            })(),
            "COMBIN" => (|| combinations(number(0)?, number(1)?).map(Value::Number))(),
            "COMBINA" => (|| {
                let (n, k) = (number(0)?.trunc(), number(1)?.trunc());
                if n < 0.0 || k < 0.0 {
                    return Err(ErrorKind::Num);
                }
                if n == 0.0 && k == 0.0 {
                    return Ok(Value::Number(1.0));
                }
                combinations(n + k - 1.0, k).map(Value::Number)
            })(),
            "PERMUT" => (|| permutations(number(0)?, number(1)?).map(Value::Number))(),
            "PERMUTATIONA" => (|| {
                let (n, k) = (number(0)?.trunc(), number(1)?.trunc());
                if n < 0.0 || k < 0.0 {
                    Err(ErrorKind::Num)
                } else {
                    Ok(Value::Number(n.powf(k)))
                }
            })(),
            "MULTINOMIAL" => (|| {
                let numbers = self.numbers(sheet, arguments)?;
                let mut total = 0.0;
                let mut result = 1.0;
                for value in numbers {
                    let value = value.trunc();
                    if value < 0.0 {
                        return Err(ErrorKind::Num);
                    }
                    result *= combinations(total + value, value)?;
                    total += value;
                }
                Ok(Value::Number(result))
            })(),
            "SUMX2MY2" | "SUMX2PY2" | "SUMXMY2" => (|| {
                let (xs, ys) = self.pairs(sheet, argument(0), argument(1))?;
                let total: f64 = xs
                    .iter()
                    .zip(&ys)
                    .map(|(x, y)| match name {
                        "SUMX2MY2" => x * x - y * y,
                        "SUMX2PY2" => x * x + y * y,
                        _ => (x - y).powi(2),
                    })
                    .sum();
                Ok(Value::Number(total))
            })(),
            "CEILING.PRECISE" | "ISO.CEILING" | "FLOOR.PRECISE" => (|| {
                let value = number(0)?;
                let significance = self.optional(sheet, argument(1), 1.0)?.abs();
                if significance == 0.0 {
                    return Ok(Value::Number(0.0));
                }
                let steps = value / significance;
                let rounded = if name == "FLOOR.PRECISE" {
                    (steps + 1e-12).floor()
                } else {
                    (steps - 1e-12).ceil()
                };
                Ok(Value::Number(rounded * significance))
            })(),
            "ROMAN" => (|| {
                let value = number(0)?.trunc();
                if !(0.0..=3_999.0).contains(&value) {
                    return Err(ErrorKind::Value);
                }
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "checked to lie in 0..=3999"
                )]
                Ok(Value::Text(roman(value as u32)))
            })(),
            "ARABIC" => (|| arabic(&self.text(sheet, argument(0))?).map(Value::Number))(),
            "BASE" => (|| {
                let value = number(0)?.trunc();
                let base = number(1)?.trunc();
                let length = self.optional(sheet, argument(2), 0.0)?.trunc();
                if value < 0.0
                    || value > 2f64.powi(53)
                    || !(2.0..=36.0).contains(&base)
                    || !(0.0..=255.0).contains(&length)
                {
                    return Err(ErrorKind::Num);
                }
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "checked ranges"
                )]
                Ok(Value::Text(to_base(
                    value as i64,
                    base as u32,
                    length as usize,
                )))
            })(),
            "DECIMAL" => (|| {
                let base = number(1)?.trunc();
                if !(2.0..=36.0).contains(&base) {
                    return Err(ErrorKind::Num);
                }
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "checked range"
                )]
                let value = from_base(&self.text(sheet, argument(0))?, base as u32)?;
                #[expect(clippy::cast_precision_loss, reason = "the value stays below 2^53")]
                Ok(Value::Number(value as f64))
            })(),
            "SUBTOTAL" => (|| {
                let code = number(0)?.trunc();
                #[expect(clippy::cast_possible_truncation, reason = "a code is a small number")]
                let function = match code as i64 % 100 {
                    1 => "AVERAGE",
                    2 => "COUNT",
                    3 => "COUNTA",
                    4 => "MAX",
                    5 => "MIN",
                    6 => "PRODUCT",
                    7 => "STDEV",
                    8 => "STDEVP",
                    9 => "SUM",
                    10 => "VAR",
                    11 => "VARP",
                    _ => return Err(ErrorKind::Value),
                };
                if code > 111.0 || (code > 11.0 && code < 101.0) {
                    return Err(ErrorKind::Value);
                }
                match self.call(sheet, function, &arguments[1..]) {
                    Value::Error(kind) => Err(kind),
                    value => Ok(value),
                }
            })(),
            "DELTA" => (|| {
                Ok(Value::Number(f64::from(u8::from(
                    number(0)? == self.optional(sheet, argument(1), 0.0)?,
                ))))
            })(),
            "GESTEP" => (|| {
                Ok(Value::Number(f64::from(u8::from(
                    number(0)? >= self.optional(sheet, argument(1), 0.0)?,
                ))))
            })(),
            _ => return None,
        })
    }

    #[expect(clippy::too_many_lines, reason = "one arm per function, each short")]
    fn more_statistics(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        let argument = |index: usize| arguments.get(index);
        let first = || self.numbers(sheet, arguments.get(..1).unwrap_or(&[]));
        Some(match name {
            "AVERAGEA" => (|| {
                let numbers = self.a_numbers(sheet, arguments)?;
                if numbers.is_empty() {
                    Err(ErrorKind::Div0)
                } else {
                    Ok(Value::Number(mean(&numbers)))
                }
            })(),
            "MAXA" | "MINA" => (|| {
                let numbers = self.a_numbers(sheet, arguments)?;
                let reduce = if name == "MAXA" { f64::max } else { f64::min };
                Ok(Value::Number(
                    numbers.into_iter().reduce(reduce).unwrap_or(0.0),
                ))
            })(),
            "STDEVA" | "STDEVPA" | "VARA" | "VARPA" => (|| {
                let numbers = self.a_numbers(sheet, arguments)?;
                let value = variance(&numbers, !name.contains("PA"))?;
                Ok(Value::Number(if name.starts_with("STDEV") {
                    value.sqrt()
                } else {
                    value
                }))
            })(),
            "GEOMEAN" => (|| {
                let numbers = self.numbers(sheet, arguments)?;
                if numbers.is_empty() || numbers.iter().any(|value| *value <= 0.0) {
                    return Err(ErrorKind::Num);
                }
                #[expect(clippy::cast_precision_loss, reason = "counts stay small")]
                let count = numbers.len() as f64;
                Ok(Value::Number(
                    (numbers.iter().map(|value| value.ln()).sum::<f64>() / count).exp(),
                ))
            })(),
            "HARMEAN" => (|| {
                let numbers = self.numbers(sheet, arguments)?;
                if numbers.is_empty() || numbers.iter().any(|value| *value <= 0.0) {
                    return Err(ErrorKind::Num);
                }
                #[expect(clippy::cast_precision_loss, reason = "counts stay small")]
                Ok(Value::Number(
                    numbers.len() as f64 / numbers.iter().map(|value| 1.0 / value).sum::<f64>(),
                ))
            })(),
            "AVEDEV" | "DEVSQ" => (|| {
                let numbers = self.numbers(sheet, arguments)?;
                if numbers.is_empty() {
                    return Err(ErrorKind::Num);
                }
                let average = mean(&numbers);
                Ok(Value::Number(if name == "AVEDEV" {
                    mean(
                        &numbers
                            .iter()
                            .map(|value| (value - average).abs())
                            .collect::<Vec<_>>(),
                    )
                } else {
                    numbers.iter().map(|value| (value - average).powi(2)).sum()
                }))
            })(),
            "KURT" => (|| {
                let numbers = self.numbers(sheet, arguments)?;
                if numbers.len() < 4 {
                    return Err(ErrorKind::Div0);
                }
                let deviation = variance(&numbers, true)?.sqrt();
                if deviation == 0.0 {
                    return Err(ErrorKind::Div0);
                }
                let average = mean(&numbers);
                #[expect(clippy::cast_precision_loss, reason = "counts stay small")]
                let n = numbers.len() as f64;
                let fourth: f64 = numbers
                    .iter()
                    .map(|value| ((value - average) / deviation).powi(4))
                    .sum();
                Ok(Value::Number(
                    n * (n + 1.0) / ((n - 1.0) * (n - 2.0) * (n - 3.0)) * fourth
                        - 3.0 * (n - 1.0).powi(2) / ((n - 2.0) * (n - 3.0)),
                ))
            })(),
            "SKEW" => (|| {
                let numbers = self.numbers(sheet, arguments)?;
                if numbers.len() < 3 {
                    return Err(ErrorKind::Div0);
                }
                let deviation = variance(&numbers, true)?.sqrt();
                if deviation == 0.0 {
                    return Err(ErrorKind::Div0);
                }
                let average = mean(&numbers);
                #[expect(clippy::cast_precision_loss, reason = "counts stay small")]
                let n = numbers.len() as f64;
                let third: f64 = numbers
                    .iter()
                    .map(|value| ((value - average) / deviation).powi(3))
                    .sum();
                Ok(Value::Number(n / ((n - 1.0) * (n - 2.0)) * third))
            })(),
            "PERCENTILE" | "PERCENTILE.INC" | "PERCENTILE.EXC" | "QUARTILE" | "QUARTILE.INC"
            | "QUARTILE.EXC" => (|| {
                let mut numbers = first()?;
                numbers.sort_by(f64::total_cmp);
                let position = self.number(sheet, argument(1))?;
                let fraction = if name.starts_with("QUARTILE") {
                    let quart = position.trunc();
                    if !(0.0..=4.0).contains(&quart) {
                        return Err(ErrorKind::Num);
                    }
                    quart / 4.0
                } else {
                    position
                };
                if name.ends_with("EXC") {
                    if fraction <= 0.0 || fraction >= 1.0 {
                        return Err(ErrorKind::Num);
                    }
                    percentile_exclusive(&numbers, fraction).map(Value::Number)
                } else {
                    percentile_inclusive(&numbers, fraction).map(Value::Number)
                }
            })(),
            "PERCENTRANK" | "PERCENTRANK.INC" | "PERCENTRANK.EXC" => (|| {
                let mut numbers = first()?;
                numbers.sort_by(f64::total_cmp);
                let value = self.number(sheet, argument(1))?;
                let significance = self.optional(sheet, argument(2), 3.0)?.trunc();
                if numbers.is_empty()
                    || significance < 1.0
                    || value < numbers[0]
                    || value > numbers[numbers.len() - 1]
                {
                    return Err(if numbers.is_empty() {
                        ErrorKind::Num
                    } else {
                        ErrorKind::NotAvailable
                    });
                }
                let below = numbers.iter().filter(|known| **known < value).count();
                #[expect(clippy::cast_precision_loss, reason = "counts stay small")]
                let (below, n) = (below as f64, numbers.len() as f64);
                let exact = numbers
                    .iter()
                    .position(|known| *known >= value)
                    .unwrap_or(0);
                let rank = if numbers[exact] == value || exact == 0 {
                    below
                } else {
                    // Between two values: interpolate between the ranks of the two.
                    let low = numbers[exact - 1];
                    let high = numbers[exact];
                    let below_low = numbers.iter().filter(|known| **known < low).count();
                    #[expect(clippy::cast_precision_loss, reason = "counts stay small")]
                    let below_low = below_low as f64;
                    below_low + (value - low) / (high - low) * (below - below_low)
                };
                let fraction = if name.ends_with("EXC") {
                    (rank + 1.0) / (n + 1.0)
                } else {
                    rank / (n - 1.0).max(1.0)
                };
                let scale = 10f64.powf(significance);
                Ok(Value::Number((fraction * scale).floor() / scale))
            })(),
            "RANK.AVG" => (|| {
                let value = self.number(sheet, argument(0))?;
                let numbers = self.numbers(sheet, arguments.get(1..2).unwrap_or(&[]))?;
                let ascending = self.optional(sheet, argument(2), 0.0)? != 0.0;
                let equal = numbers.iter().filter(|known| **known == value).count();
                if equal == 0 {
                    return Err(ErrorKind::NotAvailable);
                }
                let before = numbers
                    .iter()
                    .filter(|known| {
                        if ascending {
                            **known < value
                        } else {
                            **known > value
                        }
                    })
                    .count();
                #[expect(clippy::cast_precision_loss, reason = "counts stay small")]
                Ok(Value::Number(
                    before as f64 + f64::midpoint(equal as f64, 1.0),
                ))
            })(),
            "TRIMMEAN" => (|| {
                let mut numbers = first()?;
                let fraction = self.number(sheet, argument(1))?;
                if numbers.is_empty() || !(0.0..1.0).contains(&fraction) {
                    return Err(ErrorKind::Num);
                }
                numbers.sort_by(f64::total_cmp);
                #[expect(
                    clippy::cast_precision_loss,
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "a share of a small count"
                )]
                let cut = ((numbers.len() as f64 * fraction / 2.0).floor()) as usize;
                Ok(Value::Number(mean(&numbers[cut..numbers.len() - cut])))
            })(),
            "STANDARDIZE" => (|| {
                let (value, average, deviation) = (
                    self.number(sheet, argument(0))?,
                    self.number(sheet, argument(1))?,
                    self.number(sheet, argument(2))?,
                );
                if deviation <= 0.0 {
                    Err(ErrorKind::Num)
                } else {
                    Ok(Value::Number((value - average) / deviation))
                }
            })(),
            "CORREL" | "PEARSON" | "RSQ" | "SLOPE" | "INTERCEPT" | "COVAR" | "COVARIANCE.P"
            | "COVARIANCE.S" | "STEYX" | "FORECAST" | "FORECAST.LINEAR" => (|| {
                let forecast = name.starts_with("FORECAST");
                // SLOPE, INTERCEPT, STEYX and FORECAST name the known y before the known x.
                let (ys_at, xs_at) = if forecast {
                    (1, 2)
                } else if matches!(name, "SLOPE" | "INTERCEPT" | "STEYX" | "RSQ") {
                    (0, 1)
                } else {
                    (1, 0)
                };
                let (xs, ys) = self.pairs(sheet, argument(xs_at), argument(ys_at))?;
                let n = xs.len();
                if n == 0 {
                    return Err(ErrorKind::Div0);
                }
                let (mx, my) = (mean(&xs), mean(&ys));
                let sxy: f64 = xs.iter().zip(&ys).map(|(x, y)| (x - mx) * (y - my)).sum();
                let sxx: f64 = xs.iter().map(|x| (x - mx).powi(2)).sum();
                let syy: f64 = ys.iter().map(|y| (y - my).powi(2)).sum();
                #[expect(clippy::cast_precision_loss, reason = "counts stay small")]
                let count = n as f64;
                match name {
                    "COVAR" | "COVARIANCE.P" => Ok(Value::Number(sxy / count)),
                    "COVARIANCE.S" => {
                        if n < 2 {
                            Err(ErrorKind::Div0)
                        } else {
                            Ok(Value::Number(sxy / (count - 1.0)))
                        }
                    }
                    "CORREL" | "PEARSON" | "RSQ" => {
                        if sxx == 0.0 || syy == 0.0 {
                            return Err(ErrorKind::Div0);
                        }
                        let r = sxy / (sxx * syy).sqrt();
                        Ok(Value::Number(if name == "RSQ" { r * r } else { r }))
                    }
                    _ => {
                        if sxx == 0.0 {
                            return Err(ErrorKind::Div0);
                        }
                        let slope = sxy / sxx;
                        let intercept = my - slope * mx;
                        match name {
                            "SLOPE" => Ok(Value::Number(slope)),
                            "INTERCEPT" => Ok(Value::Number(intercept)),
                            "STEYX" => {
                                if n < 3 {
                                    Err(ErrorKind::Div0)
                                } else {
                                    Ok(Value::Number(
                                        ((syy - sxy * sxy / sxx) / (count - 2.0)).sqrt(),
                                    ))
                                }
                            }
                            _ => Ok(Value::Number(
                                intercept + slope * self.number(sheet, argument(0))?,
                            )),
                        }
                    }
                }
            })(),
            _ => return None,
        })
    }

    #[expect(clippy::too_many_lines, reason = "one arm per function, each short")]
    fn distributions(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        let argument = |index: usize| arguments.get(index);
        let number = |index: usize| self.number(sheet, argument(index));
        let flag = |index: usize| -> Result<bool, ErrorKind> {
            argument(index).map_or(Ok(true), |expression| {
                self.scalar(sheet, expression).as_bool()
            })
        };
        Some(match name {
            "NORM.DIST" | "NORMDIST" => (|| {
                let (x, average, deviation) = (number(0)?, number(1)?, number(2)?);
                if deviation <= 0.0 {
                    return Err(ErrorKind::Num);
                }
                let z = (x - average) / deviation;
                Ok(Value::Number(if flag(3)? {
                    normal_cdf(z)
                } else {
                    normal_pdf(z) / deviation
                }))
            })(),
            "NORM.S.DIST" => (|| {
                let z = number(0)?;
                Ok(Value::Number(if flag(1)? {
                    normal_cdf(z)
                } else {
                    normal_pdf(z)
                }))
            })(),
            "NORMSDIST" => (|| Ok(Value::Number(normal_cdf(number(0)?))))(),
            "PHI" => (|| Ok(Value::Number(normal_pdf(number(0)?))))(),
            "NORM.INV" | "NORMINV" => (|| {
                let (probability, average, deviation) = (number(0)?, number(1)?, number(2)?);
                if deviation <= 0.0 {
                    return Err(ErrorKind::Num);
                }
                Ok(Value::Number(
                    average + deviation * normal_inverse(probability)?,
                ))
            })(),
            "NORM.S.INV" | "NORMSINV" => (|| normal_inverse(number(0)?).map(Value::Number))(),
            "CONFIDENCE" | "CONFIDENCE.NORM" => (|| {
                let (alpha, deviation, size) = (number(0)?, number(1)?, number(2)?.trunc());
                if alpha <= 0.0 || alpha >= 1.0 || deviation <= 0.0 || size < 1.0 {
                    return Err(ErrorKind::Num);
                }
                Ok(Value::Number(
                    normal_inverse(1.0 - alpha / 2.0)? * deviation / size.sqrt(),
                ))
            })(),
            "EXPON.DIST" | "EXPONDIST" => (|| {
                let (x, lambda) = (number(0)?, number(1)?);
                if x < 0.0 || lambda <= 0.0 {
                    return Err(ErrorKind::Num);
                }
                Ok(Value::Number(if flag(2)? {
                    1.0 - (-lambda * x).exp()
                } else {
                    lambda * (-lambda * x).exp()
                }))
            })(),
            "POISSON.DIST" | "POISSON" => (|| {
                let (x, average) = (number(0)?.trunc(), number(1)?);
                if x < 0.0 || average < 0.0 {
                    return Err(ErrorKind::Num);
                }
                let probability =
                    |k: f64| (k * average.ln() - average - gamma_ln(k + 1.0).unwrap_or(0.0)).exp();
                if flag(2)? {
                    let mut total = 0.0;
                    let mut k = 0.0;
                    while k <= x {
                        total += probability(k);
                        k += 1.0;
                    }
                    Ok(Value::Number(total.min(1.0)))
                } else {
                    Ok(Value::Number(if average == 0.0 {
                        f64::from(u8::from(x == 0.0))
                    } else {
                        probability(x)
                    }))
                }
            })(),
            "BINOM.DIST" | "BINOMDIST" => (|| {
                let (successes, trials, probability) =
                    (number(0)?.trunc(), number(1)?.trunc(), number(2)?);
                if successes < 0.0 || successes > trials || !(0.0..=1.0).contains(&probability) {
                    return Err(ErrorKind::Num);
                }
                let mass = |k: f64| {
                    combinations(trials, k).unwrap_or(0.0)
                        * probability.powf(k)
                        * (1.0 - probability).powf(trials - k)
                };
                if flag(3)? {
                    let mut total = 0.0;
                    let mut k = 0.0;
                    while k <= successes {
                        total += mass(k);
                        k += 1.0;
                    }
                    Ok(Value::Number(total.min(1.0)))
                } else {
                    Ok(Value::Number(mass(successes)))
                }
            })(),
            "FISHER" => (|| {
                let x = number(0)?;
                if x.abs() >= 1.0 {
                    Err(ErrorKind::Num)
                } else {
                    Ok(Value::Number(x.atanh()))
                }
            })(),
            "FISHERINV" => (|| Ok(Value::Number(number(0)?.tanh())))(),
            "GAMMALN" | "GAMMALN.PRECISE" => (|| gamma_ln(number(0)?).map(Value::Number))(),
            "GAMMA" => (|| {
                let x = number(0)?;
                if x <= 0.0 && x.fract() == 0.0 {
                    return Err(ErrorKind::Num);
                }
                let sign = if x < 0.0 && x.floor().rem_euclid(2.0) != 0.0 {
                    -1.0
                } else {
                    1.0
                };
                let value = if x > 0.0 {
                    gamma_ln(x)?.exp()
                } else {
                    std::f64::consts::PI
                        / ((std::f64::consts::PI * x).sin().abs() * gamma_ln(1.0 - x)?.exp())
                        * sign
                };
                if value.is_finite() {
                    Ok(Value::Number(value))
                } else {
                    Err(ErrorKind::Num)
                }
            })(),
            _ => return None,
        })
    }

    #[expect(clippy::too_many_lines, reason = "one arm per function, each short")]
    fn more_finance(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        let argument = |index: usize| arguments.get(index);
        let number = |index: usize| self.number(sheet, argument(index));
        Some(match name {
            "SLN" => (|| {
                let (cost, salvage, life) = (number(0)?, number(1)?, number(2)?);
                if life == 0.0 {
                    Err(ErrorKind::Div0)
                } else {
                    Ok(Value::Number((cost - salvage) / life))
                }
            })(),
            "SYD" => (|| {
                let (cost, salvage, life, period) =
                    (number(0)?, number(1)?, number(2)?, number(3)?);
                if life <= 0.0 || period <= 0.0 || period > life {
                    return Err(ErrorKind::Num);
                }
                Ok(Value::Number(
                    (cost - salvage) * (life - period + 1.0) * 2.0 / (life * (life + 1.0)),
                ))
            })(),
            "DDB" => (|| {
                let (cost, salvage, life, period) =
                    (number(0)?, number(1)?, number(2)?, number(3)?);
                let factor = self.optional(sheet, argument(4), 2.0)?;
                if cost < 0.0
                    || salvage < 0.0
                    || life <= 0.0
                    || period <= 0.0
                    || period > life
                    || factor <= 0.0
                {
                    return Err(ErrorKind::Num);
                }
                let rate = (factor / life).min(1.0);
                let mut value = cost;
                let mut depreciation = 0.0;
                let mut step = 1.0;
                while step <= period {
                    depreciation = (value * rate).min((value - salvage).max(0.0));
                    value -= depreciation;
                    step += 1.0;
                }
                Ok(Value::Number(depreciation))
            })(),
            "DB" => (|| {
                let (cost, salvage, life, period) =
                    (number(0)?, number(1)?, number(2)?, number(3)?);
                let months = self.optional(sheet, argument(4), 12.0)?;
                if cost < 0.0
                    || salvage < 0.0
                    || life <= 0.0
                    || period <= 0.0
                    || period > life + 1.0
                    || !(1.0..=12.0).contains(&months)
                {
                    return Err(ErrorKind::Num);
                }
                let rate = if cost == 0.0 {
                    0.0
                } else {
                    ((1.0 - (salvage / cost).powf(1.0 / life)) * 1_000.0).round() / 1_000.0
                };
                let mut total = cost * rate * months / 12.0;
                if period == 1.0 {
                    return Ok(Value::Number(total));
                }
                let mut step = 2.0;
                let mut depreciation = total;
                while step <= period {
                    depreciation = if step > life {
                        (cost - total) * rate * (12.0 - months) / 12.0
                    } else {
                        (cost - total) * rate
                    };
                    total += depreciation;
                    step += 1.0;
                }
                Ok(Value::Number(depreciation))
            })(),
            "EFFECT" => (|| {
                let (nominal, periods) = (number(0)?, number(1)?.trunc());
                if nominal <= 0.0 || periods < 1.0 {
                    return Err(ErrorKind::Num);
                }
                Ok(Value::Number((1.0 + nominal / periods).powf(periods) - 1.0))
            })(),
            "NOMINAL" => (|| {
                let (effect, periods) = (number(0)?, number(1)?.trunc());
                if effect <= 0.0 || periods < 1.0 {
                    return Err(ErrorKind::Num);
                }
                Ok(Value::Number(
                    ((1.0 + effect).powf(1.0 / periods) - 1.0) * periods,
                ))
            })(),
            "ISPMT" => (|| {
                let (rate, period, periods, present) =
                    (number(0)?, number(1)?, number(2)?, number(3)?);
                if periods == 0.0 {
                    return Err(ErrorKind::Div0);
                }
                Ok(Value::Number(present * rate * (period / periods - 1.0)))
            })(),
            "CUMIPMT" | "CUMPRINC" => (|| {
                let (rate, periods, present, start, end, kind) = (
                    number(0)?,
                    number(1)?,
                    number(2)?,
                    number(3)?.trunc(),
                    number(4)?.trunc(),
                    number(5)?.trunc(),
                );
                if rate <= 0.0
                    || periods <= 0.0
                    || present <= 0.0
                    || start < 1.0
                    || end < start
                    || end > periods
                    || !(kind == 0.0 || kind == 1.0)
                {
                    return Err(ErrorKind::Num);
                }
                let payment = -present * rate
                    / (1.0 - (1.0 + rate).powf(-periods))
                    / if kind == 1.0 { 1.0 + rate } else { 1.0 };
                let mut balance = present;
                let (mut interest_total, mut principal_total) = (0.0, 0.0);
                let mut period = 1.0;
                while period <= end {
                    let interest = if kind == 1.0 && period == 1.0 {
                        0.0
                    } else {
                        -balance * rate
                    };
                    let principal = payment - interest;
                    if period >= start {
                        interest_total += interest;
                        principal_total += principal;
                    }
                    balance += principal;
                    period += 1.0;
                }
                Ok(Value::Number(if name == "CUMIPMT" {
                    interest_total
                } else {
                    principal_total
                }))
            })(),
            "MIRR" => (|| {
                let flows: Vec<f64> = self.numbers(sheet, arguments.get(..1).unwrap_or(&[]))?;
                let (finance, reinvest) = (number(1)?, number(2)?);
                #[expect(clippy::cast_precision_loss, reason = "counts stay small")]
                let n = flows.len() as f64;
                let mut positive = 0.0;
                let mut negative = 0.0;
                for (index, flow) in flows.iter().enumerate() {
                    #[expect(clippy::cast_precision_loss, reason = "counts stay small")]
                    let index = index as f64;
                    if *flow > 0.0 {
                        positive += flow * (1.0 + reinvest).powf(n - 1.0 - index);
                    } else {
                        negative += flow / (1.0 + finance).powf(index);
                    }
                }
                if positive == 0.0 || negative == 0.0 || n < 2.0 {
                    return Err(ErrorKind::Div0);
                }
                Ok(Value::Number(
                    (-positive / negative).powf(1.0 / (n - 1.0)) - 1.0,
                ))
            })(),
            "XNPV" | "XIRR" => (|| {
                let (rate_at, flows_at, dates_at) = if name == "XNPV" {
                    (Some(0), 1, 2)
                } else {
                    (None, 0, 1)
                };
                let flows = self.numbers(
                    sheet,
                    std::slice::from_ref(argument(flows_at).ok_or(ErrorKind::Value)?),
                )?;
                let dates = self.numbers(
                    sheet,
                    std::slice::from_ref(argument(dates_at).ok_or(ErrorKind::Value)?),
                )?;
                if flows.len() != dates.len() || flows.is_empty() {
                    return Err(ErrorKind::Num);
                }
                let first = dates[0];
                let value = |rate: f64| -> f64 {
                    flows
                        .iter()
                        .zip(&dates)
                        .map(|(flow, date)| flow / (1.0 + rate).powf((date - first) / 365.0))
                        .sum()
                };
                if let Some(index) = rate_at {
                    return Ok(Value::Number(value(self.number(sheet, argument(index))?)));
                }
                let derivative = |rate: f64| -> f64 {
                    flows
                        .iter()
                        .zip(&dates)
                        .map(|(flow, date)| {
                            let years = (date - first) / 365.0;
                            -years * flow / (1.0 + rate).powf(years + 1.0)
                        })
                        .sum()
                };
                let mut rate = self.optional(sheet, argument(2), 0.1)?;
                for _ in 0..MAX_ITERATIONS {
                    let slope = derivative(rate);
                    if slope == 0.0 {
                        break;
                    }
                    let next = rate - value(rate) / slope;
                    if (next - rate).abs() < TOLERANCE {
                        return Ok(Value::Number(next));
                    }
                    rate = next;
                    if rate <= -1.0 {
                        return Err(ErrorKind::Num);
                    }
                }
                Err(ErrorKind::Num)
            })(),
            "FVSCHEDULE" => (|| {
                let principal = number(0)?;
                let rates = self.numbers(sheet, arguments.get(1..2).unwrap_or(&[]))?;
                Ok(Value::Number(
                    rates
                        .iter()
                        .fold(principal, |total, rate| total * (1.0 + rate)),
                ))
            })(),
            "PDURATION" => (|| {
                let (rate, present, future) = (number(0)?, number(1)?, number(2)?);
                if rate <= 0.0 || present <= 0.0 || future <= 0.0 {
                    return Err(ErrorKind::Num);
                }
                Ok(Value::Number(
                    (future.ln() - present.ln()) / (1.0 + rate).ln(),
                ))
            })(),
            "RRI" => (|| {
                let (periods, present, future) = (number(0)?, number(1)?, number(2)?);
                if periods <= 0.0 || present == 0.0 {
                    return Err(ErrorKind::Num);
                }
                Ok(Value::Number((future / present).powf(1.0 / periods) - 1.0))
            })(),
            "DOLLARDE" | "DOLLARFR" => (|| {
                let (value, fraction) = (number(0)?, number(1)?.trunc());
                if fraction < 0.0 {
                    return Err(ErrorKind::Num);
                }
                if fraction == 0.0 {
                    return Err(ErrorKind::Div0);
                }
                let whole = value.trunc();
                let digits = 10f64.powf(fraction.log10().ceil());
                let part = value - whole;
                Ok(Value::Number(if name == "DOLLARDE" {
                    whole + part * digits / fraction
                } else {
                    whole + part * fraction / digits
                }))
            })(),
            "DAYS360" => (|| {
                let (start, end) = (
                    date_from_serial(number(0)?.floor()).ok_or(ErrorKind::Num)?,
                    date_from_serial(number(1)?.floor()).ok_or(ErrorKind::Num)?,
                );
                let european = argument(2).map_or(Ok(false), |expression| {
                    self.scalar(sheet, expression).as_bool()
                })?;
                let (y1, m1, mut d1) = start;
                let (y2, m2, mut d2) = end;
                if european {
                    d1 = d1.min(30);
                    d2 = d2.min(30);
                } else {
                    let last_of_february = |year: i64, month: i64, day: i64| {
                        month == 2
                            && day >= 28
                            && (day == 29 || year % 4 != 0 || (year % 100 == 0 && year % 400 != 0))
                    };
                    if last_of_february(y1, m1, d1) {
                        d1 = 30;
                    }
                    if d1 == 31 {
                        d1 = 30;
                    }
                    if d2 == 31 && d1 >= 30 {
                        d2 = 30;
                    }
                }
                #[expect(clippy::cast_precision_loss, reason = "a day count stays small")]
                Ok(Value::Number(
                    ((y2 - y1) * 360 + (m2 - m1) * 30 + (d2 - d1)) as f64,
                ))
            })(),
            "WORKDAY.INTL" | "NETWORKDAYS.INTL" => (|| {
                let start = number(0)?.floor();
                date_from_serial(start).ok_or(ErrorKind::Num)?;
                let rest = match argument(2) {
                    Some(expression) => weekend(&self.scalar(sheet, expression))?,
                    None => weekend(&Value::Number(1.0))?,
                };
                let holidays: Vec<f64> = match argument(3) {
                    Some(expression) => self
                        .numbers(sheet, std::slice::from_ref(expression))?
                        .into_iter()
                        .map(f64::floor)
                        .collect(),
                    None => Vec::new(),
                };
                let working = |serial: f64| {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "a serial date stays small"
                    )]
                    let weekday = ((serial as i64) + 6).rem_euclid(7);
                    #[expect(
                        clippy::cast_sign_loss,
                        clippy::cast_possible_truncation,
                        reason = "a weekday is 0 to 6"
                    )]
                    let rests = rest[weekday as usize];
                    !rests && !holidays.contains(&serial)
                };
                if name == "NETWORKDAYS.INTL" {
                    let end = number(1)?.floor();
                    date_from_serial(end).ok_or(ErrorKind::Num)?;
                    let (low, high, sign) = if start <= end {
                        (start, end, 1.0)
                    } else {
                        (end, start, -1.0)
                    };
                    if high - low > 400_000.0 {
                        return Err(ErrorKind::Num);
                    }
                    let mut count = 0.0;
                    let mut day = low;
                    while day <= high {
                        if working(day) {
                            count += 1.0;
                        }
                        day += 1.0;
                    }
                    Ok(Value::Number(count * sign))
                } else {
                    let days = number(1)?.trunc();
                    if days.abs() > 100_000.0 {
                        return Err(ErrorKind::Num);
                    }
                    let step = if days < 0.0 { -1.0 } else { 1.0 };
                    let mut remaining = days.abs();
                    let mut day = start;
                    while remaining > 0.0 {
                        day += step;
                        if working(day) {
                            remaining -= 1.0;
                        }
                    }
                    date_from_serial(day).ok_or(ErrorKind::Num)?;
                    Ok(Value::Number(day))
                }
            })(),
            _ => return None,
        })
    }

    #[expect(clippy::too_many_lines, reason = "one arm per function, each short")]
    fn engineering(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        let argument = |index: usize| arguments.get(index);
        let number = |index: usize| self.number(sheet, argument(index));
        let places = |index: usize| -> Result<Option<usize>, ErrorKind> {
            if argument(index).is_none() {
                return Ok(None);
            }
            let value = number(index)?.trunc();
            if !(0.0..=10.0).contains(&value) {
                return Err(ErrorKind::Num);
            }
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "checked to lie in 0..=10"
            )]
            Ok(Some(value as usize))
        };
        let base_of = |letter: &str| match letter {
            "BIN" => 2,
            "OCT" => 8,
            _ => 16,
        };
        let bits = |index: usize| -> Result<u64, ErrorKind> {
            let value = number(index)?;
            if value < 0.0 || value.fract() != 0.0 || value >= 2f64.powi(48) {
                return Err(ErrorKind::Num);
            }
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "checked to lie in 0..2^48"
            )]
            Ok(value as u64)
        };
        if let Some((from, to)) = name.split_once('2')
            && matches!(from, "BIN" | "OCT" | "HEX" | "DEC")
            && matches!(to, "BIN" | "OCT" | "HEX" | "DEC")
            && from != to
        {
            return Some((|| {
                let value = if from == "DEC" {
                    integer_of(number(0)?)?
                } else {
                    let text = self.scalar(sheet, argument(0).ok_or(ErrorKind::Value)?);
                    let text = match text {
                        Value::Number(value) => format!("{value}"),
                        other => other.as_text()?,
                    };
                    from_signed_base(&text, base_of(from))?
                };
                if to == "DEC" {
                    #[expect(
                        clippy::cast_precision_loss,
                        reason = "a ten digit number stays below 2^53"
                    )]
                    return Ok(Value::Number(value as f64));
                }
                to_signed_base(value, base_of(to), places(1)?).map(Value::Text)
            })());
        }
        Some(match name {
            "BITAND" => (|| Ok(bits_value(bits(0)? & bits(1)?)))(),
            "BITOR" => (|| Ok(bits_value(bits(0)? | bits(1)?)))(),
            "BITXOR" => (|| Ok(bits_value(bits(0)? ^ bits(1)?)))(),
            "BITLSHIFT" | "BITRSHIFT" => (|| {
                let value = bits(0)?;
                let shift = number(1)?.trunc();
                if shift.abs() > 53.0 {
                    return Err(ErrorKind::Num);
                }
                #[expect(clippy::cast_possible_truncation, reason = "checked to lie within 53")]
                let shift = shift as i32;
                let left = if name == "BITLSHIFT" { shift } else { -shift };
                let result = if left >= 0 {
                    value.checked_shl(left.unsigned_abs()).unwrap_or(u64::MAX)
                } else {
                    value >> left.unsigned_abs()
                };
                if result >= 1 << 48 {
                    return Err(ErrorKind::Num);
                }
                #[expect(clippy::cast_precision_loss, reason = "checked to lie below 2^48")]
                Ok(Value::Number(result as f64))
            })(),
            "CONVERT" => (|| {
                let value = number(0)?;
                let from = self.text(sheet, argument(1))?;
                let to = self.text(sheet, argument(2))?;
                if let (Some(kelvin), Some(_)) =
                    (temperature_to_kelvin(value, &from), kelvin_to(0.0, &to))
                {
                    return kelvin_to(kelvin, &to)
                        .map(Value::Number)
                        .ok_or(ErrorKind::NotAvailable);
                }
                match (unit_factor(&from), unit_factor(&to)) {
                    (Some((family, factor)), Some((other, target))) if family == other => {
                        Ok(Value::Number(value * factor / target))
                    }
                    _ => Err(ErrorKind::NotAvailable),
                }
            })(),
            _ => return None,
        })
    }

    #[expect(clippy::too_many_lines, reason = "one arm per function, each short")]
    fn more_texts(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        let argument = |index: usize| arguments.get(index);
        let number = |index: usize| self.number(sheet, argument(index));
        let text = |index: usize| self.text(sheet, argument(index));
        Some(match name {
            "UNICHAR" => (|| {
                let code = number(0)?.trunc();
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "checked by from_u32"
                )]
                char::from_u32(code as u32)
                    .filter(|_| code >= 1.0)
                    .map(|character| Value::Text(character.to_string()))
                    .ok_or(ErrorKind::Value)
            })(),
            "UNICODE" => (|| {
                text(0)?
                    .chars()
                    .next()
                    .map(|character| Value::Number(f64::from(u32::from(character))))
                    .ok_or(ErrorKind::Value)
            })(),
            "FIXED" | "DOLLAR" => (|| {
                let value = number(0)?;
                let decimals = self
                    .optional(sheet, argument(1), 2.0)?
                    .trunc()
                    .clamp(-15.0, 15.0);
                let no_separators = name == "FIXED"
                    && argument(2).map_or(Ok(false), |expression| {
                        self.scalar(sheet, expression).as_bool()
                    })?;
                let scale = 10f64.powf(decimals);
                let rounded = (value * scale).round() / scale;
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "clamped to 0..=15"
                )]
                let shown = decimals.max(0.0) as usize;
                let plain = format!("{:.shown$}", rounded.abs());
                let (whole, fraction) = plain
                    .split_once('.')
                    .map_or((plain.as_str(), ""), |(whole, fraction)| (whole, fraction));
                let (thousands, decimal) = match crate::value::Locale::default() {
                    crate::value::Locale::Italian => ('.', ','),
                    crate::value::Locale::English => (',', '.'),
                };
                let mut grouped = String::new();
                for (index, digit) in whole.chars().enumerate() {
                    if !no_separators && index > 0 && (whole.len() - index) % 3 == 0 {
                        grouped.push(thousands);
                    }
                    grouped.push(digit);
                }
                let mut result = String::new();
                if rounded < 0.0 {
                    result.push('-');
                }
                result.push_str(&grouped);
                if !fraction.is_empty() {
                    result.push(decimal);
                    result.push_str(fraction);
                }
                if name == "DOLLAR" {
                    result.push_str(" \u{20ac}");
                }
                Ok(Value::Text(result))
            })(),
            "NUMBERVALUE" => (|| {
                let value = text(0)?;
                let decimal = argument(1).map_or(Ok(".".to_owned()), |_| text(1))?;
                let group = argument(2).map_or(Ok(",".to_owned()), |_| text(2))?;
                let decimal = decimal.chars().next().unwrap_or('.');
                let group = group.chars().next().unwrap_or(',');
                let mut cleaned: String = value
                    .chars()
                    .filter(|character| !character.is_whitespace() && *character != group)
                    .map(|character| if character == decimal { '.' } else { character })
                    .collect();
                let percent = cleaned
                    .chars()
                    .rev()
                    .take_while(|character| *character == '%')
                    .count();
                cleaned.truncate(cleaned.len() - percent);
                let parsed: f64 = if cleaned.is_empty() {
                    0.0
                } else {
                    cleaned.parse().map_err(|_| ErrorKind::Value)?
                };
                #[expect(
                    clippy::cast_possible_wrap,
                    clippy::cast_possible_truncation,
                    reason = "a few percent signs"
                )]
                Ok(Value::Number(parsed / 100f64.powi(percent as i32)))
            })(),
            "TEXTBEFORE" | "TEXTAFTER" => (|| {
                let value = text(0)?;
                let delimiter = text(1)?;
                let instance = self.optional(sheet, argument(2), 1.0)?.trunc();
                if delimiter.is_empty() || instance == 0.0 {
                    return Err(ErrorKind::Value);
                }
                let positions: Vec<usize> = value
                    .match_indices(&delimiter)
                    .map(|(index, _)| index)
                    .collect();
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "an instance count stays small"
                )]
                let chosen = if instance > 0.0 {
                    positions.get(instance as usize - 1)
                } else {
                    positions
                        .len()
                        .checked_sub((-instance) as usize)
                        .and_then(|index| positions.get(index))
                };
                let Some(&at) = chosen else {
                    return Err(ErrorKind::NotAvailable);
                };
                Ok(Value::Text(if name == "TEXTBEFORE" {
                    value[..at].to_owned()
                } else {
                    value[at + delimiter.len()..].to_owned()
                }))
            })(),
            "HYPERLINK" => (|| {
                Ok(Value::Text(if argument(1).is_some() {
                    text(1)?
                } else {
                    text(0)?
                }))
            })(),
            _ => return None,
        })
    }

    fn information(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        let argument = |index: usize| arguments.get(index);
        Some(match name {
            "ISREF" => Ok(Value::Bool(matches!(
                argument(0),
                Some(Expr::Cell(_) | Expr::Range(_))
            ))),
            "TYPE" => (|| {
                let expression = argument(0).ok_or(ErrorKind::Value)?;
                if let Operand::Range { start, end, .. } = self.evaluate(sheet, expression)
                    && start != end
                {
                    return Ok(Value::Number(64.0));
                }
                Ok(Value::Number(match self.scalar(sheet, expression) {
                    Value::Number(_) | Value::Empty => 1.0,
                    Value::Text(_) => 2.0,
                    Value::Bool(_) => 4.0,
                    Value::Error(_) => 16.0,
                }))
            })(),
            "ERROR.TYPE" => (|| {
                let Value::Error(kind) = self.scalar(sheet, argument(0).ok_or(ErrorKind::Value)?)
                else {
                    return Err(ErrorKind::NotAvailable);
                };
                Ok(Value::Number(match kind {
                    ErrorKind::Null => 1.0,
                    ErrorKind::Div0 => 2.0,
                    ErrorKind::Value => 3.0,
                    ErrorKind::Ref => 4.0,
                    ErrorKind::Name => 5.0,
                    ErrorKind::Num => 6.0,
                    ErrorKind::NotAvailable => 7.0,
                    ErrorKind::Circular => 8.0,
                }))
            })(),
            "ADDRESS" => (|| {
                let row = self.number(sheet, argument(0))?.trunc();
                let column = self.number(sheet, argument(1))?.trunc();
                let kind = self.optional(sheet, argument(2), 1.0)?.trunc();
                if row < 1.0 || column < 1.0 || !(1.0..=4.0).contains(&kind) {
                    return Err(ErrorKind::Value);
                }
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "checked positive"
                )]
                let (row, column) = (row as u32, column as u32);
                let absolute_row = kind == 1.0 || kind == 2.0;
                let absolute_column = kind == 1.0 || kind == 3.0;
                let mut text = String::new();
                if let Some(sheet_name) = argument(4) {
                    let sheet_name = self.scalar(sheet, sheet_name).as_text()?;
                    text.push_str(&sheet_name);
                    text.push('!');
                }
                if absolute_column {
                    text.push('$');
                }
                text.push_str(&column_name(column - 1));
                if absolute_row {
                    text.push('$');
                }
                text.push_str(&row.to_string());
                Ok(Value::Text(text))
            })(),
            _ => return None,
        })
    }
}

fn integer_of(number: f64) -> Result<i64, ErrorKind> {
    if !number.is_finite() || number.abs() > 1e15 {
        return Err(ErrorKind::Num);
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "checked to lie within 1e15"
    )]
    Ok(number.trunc() as i64)
}

#[cfg(test)]
mod tests;
