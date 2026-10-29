// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The functions past the core set: more mathematics and statistics, logic and information,
//! text, lookups, dates and times, and the financial functions of loans and investments. Each
//! family answers `None` for a name that is not its own.

use rely_office_core::{NumberLocale, format_number, format_text};

use super::{Criterion, Evaluator, Operand, compare, wildcard};
use crate::address::MAX_ROWS;
use crate::dates::{date_from_serial, serial_from_date};
use crate::formula::Expr;
use crate::value::{ErrorKind, Locale, Value};

/// The longest text a formula may build, as Excel's cells hold.
const MAX_TEXT_CHARS: usize = 32_767;
/// Newton's method gives up past this many steps.
const MAX_ITERATIONS: usize = 100;
const TOLERANCE: f64 = 1e-10;

type Answer = Option<Result<Value, ErrorKind>>;

/// A count or position from a number, which must not be negative.
fn whole(number: f64) -> Result<usize, ErrorKind> {
    if !number.is_finite() || number < 0.0 || number > 1e15 {
        return Err(ErrorKind::Value);
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the number is checked to lie between 0 and 1e15"
    )]
    Ok(number.trunc() as usize)
}

/// A whole number, signed, from a number.
fn integer(number: f64) -> Result<i64, ErrorKind> {
    if !number.is_finite() || number.abs() > 1e15 {
        return Err(ErrorKind::Num);
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the number is checked to lie within 1e15 of zero"
    )]
    Ok(number.trunc() as i64)
}

fn count_value(count: usize) -> Value {
    #[expect(clippy::cast_precision_loss, reason = "counts stay far below 2^52")]
    Value::Number(count as f64)
}

impl Evaluator<'_> {
    /// A function past the core set, or `None` when the name is none of them.
    pub(super) fn extended(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        self.mathematics(sheet, name, arguments)
            .or_else(|| self.statistics(sheet, name, arguments))
            .or_else(|| self.logic(sheet, name, arguments))
            .or_else(|| self.texts(sheet, name, arguments))
            .or_else(|| self.lookups(sheet, name, arguments))
            .or_else(|| self.calendar(sheet, name, arguments))
            .or_else(|| self.finance(sheet, name, arguments))
            .or_else(|| self.more(sheet, name, arguments))
    }

    fn optional_number(
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

    #[expect(
        clippy::too_many_lines,
        reason = "one arm per mathematical function, each short"
    )]
    fn mathematics(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        let argument = |index: usize| arguments.get(index);
        let number = |index: usize| self.number(sheet, argument(index));
        let unary =
            |apply: fn(f64) -> Result<f64, ErrorKind>| number(0).and_then(apply).map(Value::Number);
        Some(match name {
            "SUMPRODUCT" => self.sum_product(sheet, arguments),
            "CEILING" | "FLOOR" => (|| {
                let value = number(0)?;
                let significance = self.optional_number(sheet, argument(1), 1.0)?;
                if significance == 0.0 {
                    return if name == "CEILING" {
                        Ok(Value::Number(0.0))
                    } else {
                        Err(ErrorKind::Div0)
                    };
                }
                if value > 0.0 && significance < 0.0 {
                    return Err(ErrorKind::Num);
                }
                let steps = value / significance;
                let steps = if name == "CEILING" {
                    (steps - 1e-12).ceil()
                } else {
                    (steps + 1e-12).floor()
                };
                Ok(Value::Number(steps * significance))
            })(),
            "CEILING.MATH" | "FLOOR.MATH" => (|| {
                let value = number(0)?;
                let significance = self.optional_number(sheet, argument(1), 1.0)?.abs();
                let away = self.optional_number(sheet, argument(2), 0.0)? != 0.0;
                if significance == 0.0 {
                    return Ok(Value::Number(0.0));
                }
                let up = name == "CEILING.MATH";
                let steps = value / significance;
                let rounded = if value < 0.0 && away {
                    // The mode turns a negative number away from zero, or toward it for FLOOR.
                    if up { steps.floor() } else { steps.ceil() }
                } else if up {
                    (steps - 1e-12).ceil()
                } else {
                    (steps + 1e-12).floor()
                };
                Ok(Value::Number(rounded * significance))
            })(),
            "MROUND" => (|| {
                let value = number(0)?;
                let multiple = number(1)?;
                if multiple == 0.0 {
                    return Ok(Value::Number(0.0));
                }
                if value * multiple < 0.0 {
                    return Err(ErrorKind::Num);
                }
                Ok(Value::Number(
                    (value / multiple + 1e-12_f64.copysign(value)).round() * multiple,
                ))
            })(),
            "TRUNC" => (|| {
                let value = number(0)?;
                let digits =
                    integer(self.optional_number(sheet, argument(1), 0.0)?)?.clamp(-15, 15);
                let scale = 10f64.powi(i32::try_from(digits.abs()).unwrap_or(0));
                Ok(Value::Number(if digits >= 0 {
                    (value * scale).trunc() / scale
                } else {
                    (value / scale).trunc() * scale
                }))
            })(),
            "EXP" => unary(|value| Ok(value.exp())),
            "LN" => unary(|value| {
                if value <= 0.0 {
                    Err(ErrorKind::Num)
                } else {
                    Ok(value.ln())
                }
            }),
            "LOG10" => unary(|value| {
                if value <= 0.0 {
                    Err(ErrorKind::Num)
                } else {
                    Ok(value.log10())
                }
            }),
            "LOG" => (|| {
                let value = number(0)?;
                let base = self.optional_number(sheet, argument(1), 10.0)?;
                if value <= 0.0 || base <= 0.0 {
                    return Err(ErrorKind::Num);
                }
                if (base - 1.0).abs() < f64::EPSILON {
                    return Err(ErrorKind::Div0);
                }
                Ok(Value::Number(value.log(base)))
            })(),
            "EVEN" | "ODD" => (|| {
                let value = number(0)?;
                let magnitude = value.abs().ceil();
                let even = name == "EVEN";
                let odd_magnitude = integer(magnitude)? % 2 == 1;
                let rounded = if odd_magnitude == even {
                    magnitude + 1.0
                } else {
                    magnitude
                };
                Ok(Value::Number(rounded.copysign(if value == 0.0 {
                    1.0
                } else {
                    value
                })))
            })(),
            "FACT" => (|| {
                let value = number(0)?;
                if value < 0.0 {
                    return Err(ErrorKind::Num);
                }
                let limit = whole(value)?;
                if limit > 170 {
                    return Err(ErrorKind::Num);
                }
                Ok(Value::Number((1..=limit).fold(1.0, |product, factor| {
                    #[expect(clippy::cast_precision_loss, reason = "factors stay below 171")]
                    let factor = factor as f64;
                    product * factor
                })))
            })(),
            "GCD" | "LCM" => (|| {
                let numbers = self.numbers(sheet, arguments)?;
                let mut result: Option<u64> = None;
                for value in numbers {
                    if value < 0.0 {
                        return Err(ErrorKind::Num);
                    }
                    let value = u64::try_from(whole(value)?).map_err(|_| ErrorKind::Num)?;
                    result = Some(match result {
                        None => value,
                        Some(current) if name == "GCD" => gcd(current, value),
                        Some(current) if current == 0 || value == 0 => 0,
                        Some(current) => current / gcd(current, value) * value,
                    });
                }
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "the result is shown as a number"
                )]
                Ok(Value::Number(result.unwrap_or(0) as f64))
            })(),
            "QUOTIENT" => (|| {
                let divisor = number(1)?;
                if divisor == 0.0 {
                    return Err(ErrorKind::Div0);
                }
                Ok(Value::Number((number(0)? / divisor).trunc()))
            })(),
            "SUMSQ" => self
                .numbers(sheet, arguments)
                .map(|numbers| Value::Number(numbers.iter().map(|value| value * value).sum())),
            "RADIANS" => unary(|value| Ok(value.to_radians())),
            "DEGREES" => unary(|value| Ok(value.to_degrees())),
            "SIN" => unary(|value| Ok(value.sin())),
            "COS" => unary(|value| Ok(value.cos())),
            "TAN" => unary(|value| Ok(value.tan())),
            "ASIN" | "ACOS" => (|| {
                let value = number(0)?;
                if !(-1.0..=1.0).contains(&value) {
                    return Err(ErrorKind::Num);
                }
                Ok(Value::Number(if name == "ASIN" {
                    value.asin()
                } else {
                    value.acos()
                }))
            })(),
            "ATAN" => unary(|value| Ok(value.atan())),
            _ => return None,
        })
    }

    fn sum_product(&self, sheet: usize, arguments: &[Expr]) -> Result<Value, ErrorKind> {
        if arguments.is_empty() {
            return Err(ErrorKind::Value);
        }
        let grids = arguments
            .iter()
            .map(|argument| self.grid(sheet, Some(argument)))
            .collect::<Result<Vec<_>, _>>()?;
        let shape = |grid: &Vec<Vec<Value>>| (grid.len(), grid.first().map_or(0, Vec::len));
        let expected = shape(&grids[0]);
        if grids.iter().any(|grid| shape(grid) != expected) {
            return Err(ErrorKind::Value);
        }
        let mut total = 0.0;
        for row in 0..expected.0 {
            for column in 0..expected.1 {
                let mut product = 1.0;
                for grid in &grids {
                    match &grid[row][column] {
                        Value::Number(value) => product *= value,
                        Value::Error(kind) => return Err(*kind),
                        _ => product = 0.0,
                    }
                }
                total += product;
            }
        }
        Ok(Value::Number(total))
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one arm per statistical function, each short"
    )]
    fn statistics(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        let argument = |index: usize| arguments.get(index);
        Some(match name {
            "MEDIAN" => (|| {
                let mut numbers = self.numbers(sheet, arguments)?;
                if numbers.is_empty() {
                    return Err(ErrorKind::Num);
                }
                numbers.sort_by(f64::total_cmp);
                let middle = numbers.len() / 2;
                Ok(Value::Number(if numbers.len() % 2 == 0 {
                    f64::midpoint(numbers[middle - 1], numbers[middle])
                } else {
                    numbers[middle]
                }))
            })(),
            "LARGE" | "SMALL" => (|| {
                let mut numbers = self.numbers(sheet, &arguments[..1.min(arguments.len())])?;
                let rank = whole(self.number(sheet, argument(1))?)?;
                if rank == 0 || rank > numbers.len() {
                    return Err(ErrorKind::Num);
                }
                numbers.sort_by(f64::total_cmp);
                Ok(Value::Number(if name == "SMALL" {
                    numbers[rank - 1]
                } else {
                    numbers[numbers.len() - rank]
                }))
            })(),
            "RANK" | "RANK.EQ" => (|| {
                let value = self.number(sheet, argument(0))?;
                let numbers = self.numbers(sheet, arguments.get(1..2).unwrap_or(&[]))?;
                let ascending = self.optional_number(sheet, argument(2), 0.0)? != 0.0;
                if !numbers.iter().any(|known| known.total_cmp(&value).is_eq()) {
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
                Ok(count_value(before + 1))
            })(),
            "STDEV" | "STDEV.S" | "STDEVP" | "STDEV.P" | "VAR" | "VAR.S" | "VARP" | "VAR.P" => {
                (|| {
                    let numbers = self.numbers(sheet, arguments)?;
                    let population = name.ends_with('P');
                    let minimum = if population { 1 } else { 2 };
                    if numbers.len() < minimum {
                        return Err(ErrorKind::Div0);
                    }
                    #[expect(clippy::cast_precision_loss, reason = "counts stay far below 2^52")]
                    let count = numbers.len() as f64;
                    let mean = numbers.iter().sum::<f64>() / count;
                    let squares: f64 = numbers.iter().map(|value| (value - mean).powi(2)).sum();
                    let variance = squares / if population { count } else { count - 1.0 };
                    Ok(Value::Number(if name.starts_with("STDEV") {
                        variance.sqrt()
                    } else {
                        variance
                    }))
                })()
            }
            "MODE" | "MODE.SNGL" => (|| {
                let numbers = self.numbers(sheet, arguments)?;
                let mut best: Option<(f64, usize)> = None;
                for (index, value) in numbers.iter().enumerate() {
                    let occurrences = numbers[index..]
                        .iter()
                        .filter(|other| other.total_cmp(value).is_eq())
                        .count();
                    if occurrences > 1 && best.is_none_or(|(_, count)| occurrences > count) {
                        best = Some((*value, occurrences));
                    }
                }
                best.map(|(value, _)| Value::Number(value))
                    .ok_or(ErrorKind::NotAvailable)
            })(),
            "MAXIFS" | "MINIFS" | "AVERAGEIFS" => (|| {
                let numbers = self.matching_numbers(sheet, arguments)?;
                match name {
                    "MAXIFS" => Ok(Value::Number(
                        numbers.into_iter().reduce(f64::max).unwrap_or(0.0),
                    )),
                    "MINIFS" => Ok(Value::Number(
                        numbers.into_iter().reduce(f64::min).unwrap_or(0.0),
                    )),
                    _ if numbers.is_empty() => Err(ErrorKind::Div0),
                    _ => {
                        #[expect(clippy::cast_precision_loss, reason = "counts stay small")]
                        let count = numbers.len() as f64;
                        Ok(Value::Number(numbers.iter().sum::<f64>() / count))
                    }
                }
            })(),
            _ => return None,
        })
    }

    /// The numbers of the first range whose cells meet every range and criterion pair after it.
    fn matching_numbers(&self, sheet: usize, arguments: &[Expr]) -> Result<Vec<f64>, ErrorKind> {
        let Some((target, pairs)) = arguments.split_first() else {
            return Err(ErrorKind::Value);
        };
        if pairs.is_empty() || pairs.len() % 2 != 0 {
            return Err(ErrorKind::Value);
        }
        let target = self.grid(sheet, Some(target))?;
        let mut conditions = Vec::new();
        for pair in pairs.chunks(2) {
            conditions.push((
                self.grid(sheet, Some(&pair[0]))?,
                Criterion::parse(&self.scalar(sheet, &pair[1])),
            ));
        }
        let mut numbers = Vec::new();
        for (row_index, row) in target.iter().enumerate() {
            for (column_index, value) in row.iter().enumerate() {
                let all = conditions.iter().all(|(grid, criterion)| {
                    grid.get(row_index)
                        .and_then(|row| row.get(column_index))
                        .is_some_and(|value| criterion.matches(value))
                });
                if all && let Value::Number(number) = value {
                    numbers.push(*number);
                }
            }
        }
        Ok(numbers)
    }

    fn logic(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        let argument = |index: usize| arguments.get(index);
        let first = || -> Result<Value, ErrorKind> {
            Ok(self.scalar(sheet, argument(0).ok_or(ErrorKind::Value)?))
        };
        Some(match name {
            "IFS" => (|| {
                if arguments.is_empty() || !arguments.len().is_multiple_of(2) {
                    return Err(ErrorKind::Value);
                }
                for pair in arguments.chunks(2) {
                    if self.scalar(sheet, &pair[0]).as_bool()? {
                        return Ok(self.scalar(sheet, &pair[1]));
                    }
                }
                Err(ErrorKind::NotAvailable)
            })(),
            "SWITCH" => (|| {
                let subject = first()?;
                if let Value::Error(kind) = subject {
                    return Err(kind);
                }
                let cases = &arguments[1.min(arguments.len())..];
                if cases.len() < 2 {
                    return Err(ErrorKind::Value);
                }
                for pair in cases.chunks(2) {
                    match pair {
                        [case, result] => {
                            let case = self.scalar(sheet, case);
                            if compare(&case, &subject) == std::cmp::Ordering::Equal
                                && std::mem::discriminant(&case) == std::mem::discriminant(&subject)
                            {
                                return Ok(self.scalar(sheet, result));
                            }
                        }
                        [default] => return Ok(self.scalar(sheet, default)),
                        _ => {}
                    }
                }
                Err(ErrorKind::NotAvailable)
            })(),
            "XOR" => {
                let mut count = 0_usize;
                let mut error = None;
                for expression in arguments {
                    self.values_of(sheet, expression, |value, from_range| match value {
                        Value::Error(kind) => error = error.or(Some(kind)),
                        Value::Empty | Value::Text(_) if from_range => {}
                        other => match other.as_bool() {
                            Ok(flag) => count += usize::from(flag),
                            Err(kind) => error = error.or(Some(kind)),
                        },
                    });
                }
                error.map_or(Ok(Value::Bool(count % 2 == 1)), Err)
            }
            "IFNA" => (|| {
                let value = first()?;
                if value == Value::Error(ErrorKind::NotAvailable) {
                    Ok(self.scalar(sheet, argument(1).ok_or(ErrorKind::Value)?))
                } else {
                    Ok(value)
                }
            })(),
            "NA" => Err(ErrorKind::NotAvailable),
            "ISNA" => {
                first().map(|value| Value::Bool(value == Value::Error(ErrorKind::NotAvailable)))
            }
            "ISERR" => first().map(|value| {
                Value::Bool(matches!(value, Value::Error(kind) if kind != ErrorKind::NotAvailable))
            }),
            "ISLOGICAL" => first().map(|value| Value::Bool(matches!(value, Value::Bool(_)))),
            "ISNONTEXT" => first().map(|value| Value::Bool(!matches!(value, Value::Text(_)))),
            "ISEVEN" | "ISODD" => (|| {
                let value = integer(first()?.as_number()?)?;
                Ok(Value::Bool((value % 2 == 0) == (name == "ISEVEN")))
            })(),
            _ => return None,
        })
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one arm per text function, each short"
    )]
    fn texts(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        let argument = |index: usize| arguments.get(index);
        let text = |index: usize| self.text(sheet, argument(index));
        Some(match name {
            "TEXT" => (|| {
                let value = self.scalar(sheet, argument(0).ok_or(ErrorKind::Value)?);
                let code = text(1)?;
                match value {
                    Value::Error(kind) => Err(kind),
                    Value::Text(shown) => Ok(Value::Text(format_text(&shown, &code))),
                    other => {
                        let number = other.as_number()?;
                        let (code, locale) = text_code(&code);
                        Ok(Value::Text(format_number(number, &code, locale).text))
                    }
                }
            })(),
            "VALUE" => (|| {
                let shown = text(0)?;
                let trimmed = shown.trim();
                let number = if trimmed.contains(',') && !trimmed.contains('.') {
                    Locale::Italian.parse_number(trimmed)
                } else {
                    Locale::English
                        .parse_number(trimmed)
                        .or_else(|| Locale::Italian.parse_number(trimmed))
                };
                number
                    .or_else(|| parse_date(trimmed))
                    .or_else(|| parse_time(trimmed))
                    .map(Value::Number)
                    .ok_or(ErrorKind::Value)
            })(),
            "SUBSTITUTE" => (|| {
                let source = text(0)?;
                let old = text(1)?;
                let new = text(2)?;
                if old.is_empty() {
                    return Ok(Value::Text(source));
                }
                let replaced = if argument(3).is_none() {
                    source.replace(&old, &new)
                } else {
                    {
                        let instance = whole(self.number(sheet, argument(3))?)?;
                        if instance == 0 {
                            return Err(ErrorKind::Value);
                        }
                        match source.match_indices(&old).nth(instance - 1) {
                            Some((position, _)) => format!(
                                "{}{new}{}",
                                &source[..position],
                                &source[position + old.len()..]
                            ),
                            None => source,
                        }
                    }
                };
                limited(replaced)
            })(),
            "REPLACE" => (|| {
                let source: Vec<char> = text(0)?.chars().collect();
                let start = whole(self.number(sheet, argument(1))?)?;
                let count = whole(self.number(sheet, argument(2))?)?;
                if start == 0 {
                    return Err(ErrorKind::Value);
                }
                let start = (start - 1).min(source.len());
                let end = start.saturating_add(count).min(source.len());
                let mut replaced: String = source[..start].iter().collect();
                replaced.push_str(&text(3)?);
                replaced.extend(&source[end..]);
                limited(replaced)
            })(),
            "FIND" | "SEARCH" => (|| {
                let wanted = text(0)?;
                let within = text(1)?;
                let start = whole(self.optional_number(sheet, argument(2), 1.0)?)?;
                let characters: Vec<char> = within.chars().collect();
                if start == 0 || start > characters.len() + 1 {
                    return Err(ErrorKind::Value);
                }
                let position = if name == "FIND" {
                    let wanted: Vec<char> = wanted.chars().collect();
                    (start - 1..=characters.len().saturating_sub(wanted.len()))
                        .find(|index| characters[*index..].starts_with(&wanted))
                } else {
                    let mut pattern: Vec<char> = wanted.to_lowercase().chars().collect();
                    pattern.push('*');
                    let lowered: Vec<char> = within.to_lowercase().chars().collect();
                    (start - 1..=lowered.len()).find(|index| wildcard(&pattern, &lowered[*index..]))
                };
                position
                    .map(|index| count_value(index + 1))
                    .ok_or(ErrorKind::Value)
            })(),
            "REPT" => (|| {
                let source = text(0)?;
                let times = whole(self.number(sheet, argument(1))?)?;
                if source.chars().count().saturating_mul(times) > MAX_TEXT_CHARS {
                    return Err(ErrorKind::Value);
                }
                Ok(Value::Text(source.repeat(times)))
            })(),
            "TEXTJOIN" => (|| {
                let delimiter = text(0)?;
                let skip_empty = self
                    .scalar(sheet, argument(1).ok_or(ErrorKind::Value)?)
                    .as_bool()?;
                let mut parts = Vec::new();
                let mut error = None;
                for expression in arguments.get(2..).unwrap_or(&[]) {
                    self.values_of(sheet, expression, |value, _| match value.as_text() {
                        Ok(part) if part.is_empty() && skip_empty => {}
                        Ok(part) => parts.push(part),
                        Err(kind) => error = error.or(Some(kind)),
                    });
                }
                if let Some(kind) = error {
                    return Err(kind);
                }
                limited(parts.join(&delimiter))
            })(),
            "EXACT" => (|| Ok(Value::Bool(text(0)? == text(1)?)))(),
            "PROPER" => text(0).map(|source| {
                let mut previous_letter = false;
                Value::Text(
                    source
                        .chars()
                        .map(|character| {
                            let shown = if previous_letter {
                                character.to_lowercase().collect::<String>()
                            } else {
                                character.to_uppercase().collect::<String>()
                            };
                            previous_letter = character.is_alphabetic();
                            shown
                        })
                        .collect(),
                )
            }),
            "CHAR" => (|| {
                let code = whole(self.number(sheet, argument(0))?)?;
                if !(1..=255).contains(&code) {
                    return Err(ErrorKind::Value);
                }
                let code = u8::try_from(code).map_err(|_| ErrorKind::Value)?;
                Ok(Value::Text(char::from(code).to_string()))
            })(),
            "CODE" => (|| {
                let first = text(0)?.chars().next().ok_or(ErrorKind::Value)?;
                Ok(Value::Number(f64::from(u32::from(first))))
            })(),
            "CLEAN" => text(0).map(|source| {
                Value::Text(
                    source
                        .chars()
                        .filter(|character| *character >= ' ')
                        .collect(),
                )
            }),
            "T" | "N" => (|| {
                let value = self.scalar(sheet, argument(0).ok_or(ErrorKind::Value)?);
                Ok(match (name, value) {
                    (_, Value::Error(kind)) => return Err(kind),
                    ("T", Value::Text(shown)) => Value::Text(shown),
                    ("T", _) => Value::Text(String::new()),
                    (_, Value::Number(number)) => Value::Number(number),
                    (_, Value::Bool(flag)) => Value::Number(f64::from(u8::from(flag))),
                    _ => Value::Number(0.0),
                })
            })(),
            _ => return None,
        })
    }

    fn lookups(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        let argument = |index: usize| arguments.get(index);
        Some(match name {
            "CHOOSE" => (|| {
                let index = whole(self.number(sheet, argument(0))?)?;
                if index == 0 || index >= arguments.len() {
                    return Err(ErrorKind::Value);
                }
                Ok(self.scalar(sheet, &arguments[index]))
            })(),
            "ROWS" | "COLUMNS" => {
                (|| match self.evaluate(sheet, argument(0).ok_or(ErrorKind::Value)?) {
                    Operand::Range { start, end, .. } => {
                        let rows = if end.row == MAX_ROWS - 1 {
                            MAX_ROWS - start.row
                        } else {
                            end.row - start.row + 1
                        };
                        Ok(Value::Number(f64::from(if name == "ROWS" {
                            rows
                        } else {
                            end.column - start.column + 1
                        })))
                    }
                    Operand::Array(grid) => {
                        let size = if name == "ROWS" {
                            grid.len()
                        } else {
                            grid.first().map_or(0, Vec::len)
                        };
                        Ok(Value::Number(f64::from(
                            u32::try_from(size).unwrap_or(u32::MAX),
                        )))
                    }
                    Operand::Value(Value::Error(kind)) => Err(kind),
                    Operand::Value(_) => Ok(Value::Number(1.0)),
                })()
            }
            "ROW" | "COLUMN" => (|| {
                let cell = match argument(0) {
                    None => self.current.get().ok_or(ErrorKind::Value)?,
                    Some(Expr::Cell(address)) => address.cell,
                    Some(Expr::Range(range)) => range.bounds().0,
                    Some(_) => return Err(ErrorKind::Value),
                };
                Ok(Value::Number(f64::from(if name == "ROW" {
                    cell.row + 1
                } else {
                    cell.column + 1
                })))
            })(),
            "XLOOKUP" => self.xlookup(sheet, arguments),
            "LOOKUP" => (|| {
                let lookup = self.scalar(sheet, argument(0).ok_or(ErrorKind::Value)?);
                let keys = line(&self.grid(sheet, argument(1))?);
                let results = match argument(2) {
                    Some(_) => line(&self.grid(sheet, argument(2))?),
                    None => keys.clone(),
                };
                let found = super::find(&keys, &lookup, 1).ok_or(ErrorKind::NotAvailable)?;
                results.get(found).cloned().ok_or(ErrorKind::NotAvailable)
            })(),
            _ => return None,
        })
    }

    /// `XLOOKUP(value, lookup, results, [not found], [match mode], [search mode])`.
    fn xlookup(&self, sheet: usize, arguments: &[Expr]) -> Result<Value, ErrorKind> {
        let argument = |index: usize| arguments.get(index);
        let lookup = self.scalar(sheet, argument(0).ok_or(ErrorKind::Value)?);
        let keys = line(&self.grid(sheet, argument(1))?);
        let results = self.grid(sheet, argument(2))?;
        let vertical = results.len() == keys.len() && results.len() > 1
            || results.first().is_some_and(|row| row.len() == 1);
        let mode = integer(self.optional_number(sheet, argument(4), 0.0)?)?;
        let reverse = integer(self.optional_number(sheet, argument(5), 1.0)?)? < 0;
        let order: Vec<usize> = if reverse {
            (0..keys.len()).rev().collect()
        } else {
            (0..keys.len()).collect()
        };
        let pattern: Option<Vec<char>> = (mode == 2).then(|| {
            lookup
                .as_text()
                .unwrap_or_default()
                .to_lowercase()
                .chars()
                .collect()
        });
        let mut best: Option<usize> = None;
        for index in order {
            let key = &keys[index];
            if matches!(key, Value::Empty) {
                continue;
            }
            if let Some(pattern) = &pattern {
                let text: Vec<char> = key
                    .as_text()
                    .unwrap_or_default()
                    .to_lowercase()
                    .chars()
                    .collect();
                if wildcard(pattern, &text) {
                    best = Some(index);
                    break;
                }
                continue;
            }
            let ordering = compare(key, &lookup);
            if ordering == std::cmp::Ordering::Equal {
                best = Some(index);
                break;
            }
            let closer = |current: Option<usize>| {
                current.is_none_or(|current| {
                    let held = &keys[current];
                    if mode < 0 {
                        compare(key, held) == std::cmp::Ordering::Greater
                    } else {
                        compare(key, held) == std::cmp::Ordering::Less
                    }
                })
            };
            if (mode < 0 && ordering == std::cmp::Ordering::Less
                || mode == 1 && ordering == std::cmp::Ordering::Greater)
                && closer(best)
            {
                best = Some(index);
            }
        }
        let Some(found) = best else {
            return match argument(3) {
                Some(missing) => Ok(self.scalar(sheet, missing)),
                None => Err(ErrorKind::NotAvailable),
            };
        };
        let value = if vertical {
            results.get(found).and_then(|row| row.first())
        } else {
            results.first().and_then(|row| row.get(found))
        };
        value.cloned().ok_or(ErrorKind::Value)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one arm per date function, each short"
    )]
    fn calendar(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        let argument = |index: usize| arguments.get(index);
        let number = |index: usize| self.number(sheet, argument(index));
        let date = |index: usize| -> Result<(i64, i64, i64), ErrorKind> {
            date_from_serial(number(index)?).ok_or(ErrorKind::Num)
        };
        Some(match name {
            "DATEDIF" => (|| {
                let start = number(0)?.floor();
                let end = number(1)?.floor();
                if start > end {
                    return Err(ErrorKind::Num);
                }
                let (start_year, start_month, start_day) = date(0)?;
                let (end_year, end_month, end_day) = date(1)?;
                let months = (end_year - start_year) * 12 + (end_month - start_month)
                    - i64::from(end_day < start_day);
                #[expect(clippy::cast_precision_loss, reason = "date parts are small")]
                let result = match self.text(sheet, argument(2))?.to_uppercase().as_str() {
                    "D" => end - start,
                    "M" => months as f64,
                    "Y" => (months / 12) as f64,
                    "YM" => (months % 12) as f64,
                    "MD" if end_day >= start_day => (end_day - start_day) as f64,
                    "MD" => {
                        let previous = date_from_serial(
                            serial_from_date(end_year, end_month, 0).ok_or(ErrorKind::Num)?,
                        )
                        .ok_or(ErrorKind::Num)?
                        .2;
                        ((previous - start_day).max(0) + end_day) as f64
                    }
                    "YD" => {
                        let year = if (end_month, end_day) >= (start_month, start_day) {
                            end_year
                        } else {
                            end_year - 1
                        };
                        end - serial_from_date(year, start_month, start_day)
                            .ok_or(ErrorKind::Num)?
                    }
                    _ => return Err(ErrorKind::Num),
                };
                Ok(Value::Number(result))
            })(),
            "EDATE" | "EOMONTH" => (|| {
                let (year, month, day) = date(0)?;
                let months = integer(number(1)?)?;
                let serial = if name == "EOMONTH" {
                    serial_from_date(year, month + months + 1, 0)
                } else {
                    let last =
                        serial_from_date(year, month + months + 1, 0).ok_or(ErrorKind::Num)?;
                    serial_from_date(year, month + months, day).map(|serial| serial.min(last))
                };
                serial.map(Value::Number).ok_or(ErrorKind::Num)
            })(),
            "WEEKDAY" => (|| {
                let serial = number(0)?;
                date_from_serial(serial).ok_or(ErrorKind::Num)?;
                let kind = integer(self.optional_number(sheet, argument(1), 1.0)?)?;
                // 0 for Sunday.
                let sunday_based = (integer(serial.floor())? + 6).rem_euclid(7);
                let shown = match kind {
                    1 | 17 => sunday_based + 1,
                    2 | 11 => (sunday_based + 6) % 7 + 1,
                    3 => (sunday_based + 6) % 7,
                    12..=16 => (sunday_based + 7 - (kind - 10)) % 7 + 1,
                    _ => return Err(ErrorKind::Num),
                };
                #[expect(clippy::cast_precision_loss, reason = "a weekday is below 8")]
                Ok(Value::Number(shown as f64))
            })(),
            "WEEKNUM" | "ISOWEEKNUM" => (|| {
                let serial = number(0)?.floor();
                let (year, _, _) = date(0)?;
                let kind = if name == "ISOWEEKNUM" {
                    21
                } else {
                    integer(self.optional_number(sheet, argument(1), 1.0)?)?
                };
                week_number(serial, year, kind).map(Value::Number)
            })(),
            "NETWORKDAYS" | "WORKDAY" => (|| {
                let start = number(0)?.floor();
                date_from_serial(start).ok_or(ErrorKind::Num)?;
                let holidays: Vec<f64> = match argument(2) {
                    Some(expression) => self
                        .numbers(sheet, std::slice::from_ref(expression))?
                        .into_iter()
                        .map(f64::floor)
                        .collect(),
                    None => Vec::new(),
                };
                let working = |serial: f64| {
                    let weekday = (integer(serial).unwrap_or(0) + 6).rem_euclid(7);
                    weekday != 0 && weekday != 6 && !holidays.contains(&serial)
                };
                if name == "NETWORKDAYS" {
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
                    let days = integer(number(1)?)?;
                    if days.abs() > 100_000 {
                        return Err(ErrorKind::Num);
                    }
                    let step = if days < 0 { -1.0 } else { 1.0 };
                    let mut remaining = days.abs();
                    let mut day = start;
                    while remaining > 0 {
                        day += step;
                        if working(day) {
                            remaining -= 1;
                        }
                    }
                    date_from_serial(day).ok_or(ErrorKind::Num)?;
                    Ok(Value::Number(day))
                }
            })(),
            "DAYS" => (|| Ok(Value::Number(number(0)?.floor() - number(1)?.floor())))(),
            "HOUR" | "MINUTE" | "SECOND" => (|| {
                let serial = number(0)?;
                if serial < 0.0 {
                    return Err(ErrorKind::Num);
                }
                let seconds = integer((serial.fract() * 86_400.0).round())?;
                let part = match name {
                    "HOUR" => seconds / 3_600 % 24,
                    "MINUTE" => seconds / 60 % 60,
                    _ => seconds % 60,
                };
                #[expect(clippy::cast_precision_loss, reason = "a time part is below 60")]
                Ok(Value::Number(part as f64))
            })(),
            "TIME" => (|| {
                let total =
                    number(0)?.trunc() * 3_600.0 + number(1)?.trunc() * 60.0 + number(2)?.trunc();
                if total < 0.0 {
                    return Err(ErrorKind::Num);
                }
                Ok(Value::Number((total / 86_400.0).fract()))
            })(),
            "DATEVALUE" => self
                .text(sheet, argument(0))
                .and_then(|shown| parse_date(shown.trim()).ok_or(ErrorKind::Value))
                .map(Value::Number),
            "TIMEVALUE" => self
                .text(sheet, argument(0))
                .and_then(|shown| parse_time(shown.trim()).ok_or(ErrorKind::Value))
                .map(Value::Number),
            "YEARFRAC" => (|| {
                let mut start = number(0)?.floor();
                let mut end = number(1)?.floor();
                if start > end {
                    std::mem::swap(&mut start, &mut end);
                }
                let basis = integer(self.optional_number(sheet, argument(2), 0.0)?)?;
                let (start_year, start_month, start_day) =
                    date_from_serial(start).ok_or(ErrorKind::Num)?;
                let (end_year, end_month, end_day) = date_from_serial(end).ok_or(ErrorKind::Num)?;
                #[expect(clippy::cast_precision_loss, reason = "date parts are small")]
                let thirty = |european: bool| {
                    let mut first = start_day.min(30);
                    let mut last = end_day;
                    if european {
                        last = last.min(30);
                    } else if first == 30 && last == 31 {
                        last = 30;
                    }
                    if !european && start_day == 31 {
                        first = 30;
                    }
                    ((end_year - start_year) * 360
                        + (end_month - start_month) * 30
                        + (last - first)) as f64
                        / 360.0
                };
                Ok(Value::Number(match basis {
                    0 => thirty(false),
                    1 => {
                        let years = (start_year..=end_year)
                            .map(|year| if leap(year) { 366.0 } else { 365.0 })
                            .collect::<Vec<f64>>();
                        #[expect(clippy::cast_precision_loss, reason = "few years")]
                        let average = years.iter().sum::<f64>() / years.len() as f64;
                        (end - start) / average
                    }
                    2 => (end - start) / 360.0,
                    3 => (end - start) / 365.0,
                    4 => thirty(true),
                    _ => return Err(ErrorKind::Num),
                }))
            })(),
            _ => return None,
        })
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one arm per financial function, each short"
    )]
    fn finance(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        let argument = |index: usize| arguments.get(index);
        let number = |index: usize| self.number(sheet, argument(index));
        let optional =
            |index: usize, default: f64| self.optional_number(sheet, argument(index), default);
        Some(match name {
            "PMT" => (|| {
                let kind = optional(4, 0.0)?;
                Ok(Value::Number(payment(
                    number(0)?,
                    number(1)?,
                    number(2)?,
                    optional(3, 0.0)?,
                    kind,
                )?))
            })(),
            "FV" => (|| {
                Ok(Value::Number(future_value(
                    number(0)?,
                    number(1)?,
                    number(2)?,
                    optional(3, 0.0)?,
                    optional(4, 0.0)?,
                )))
            })(),
            "PV" => (|| {
                let (rate, periods, pmt) = (number(0)?, number(1)?, number(2)?);
                let (future, kind) = (optional(3, 0.0)?, optional(4, 0.0)?);
                Ok(Value::Number(if rate == 0.0 {
                    -(future + pmt * periods)
                } else {
                    let growth = (1.0 + rate).powf(periods);
                    -(future + pmt * (1.0 + rate * kind) * (growth - 1.0) / rate) / growth
                }))
            })(),
            "NPER" => (|| {
                let (rate, pmt, present) = (number(0)?, number(1)?, number(2)?);
                let (future, kind) = (optional(3, 0.0)?, optional(4, 0.0)?);
                if rate == 0.0 {
                    if pmt == 0.0 {
                        return Err(ErrorKind::Num);
                    }
                    return Ok(Value::Number(-(present + future) / pmt));
                }
                let adjusted = pmt * (1.0 + rate * kind);
                let ratio = (adjusted - future * rate) / (adjusted + present * rate);
                if ratio <= 0.0 {
                    return Err(ErrorKind::Num);
                }
                Ok(Value::Number(ratio.ln() / (1.0 + rate).ln()))
            })(),
            "NPV" => (|| {
                let rate = number(0)?;
                let values = self.numbers(sheet, arguments.get(1..).unwrap_or(&[]))?;
                let mut total = 0.0;
                for (index, value) in values.iter().enumerate() {
                    let period = i32::try_from(index + 1).map_err(|_| ErrorKind::Num)?;
                    total += value / (1.0 + rate).powi(period);
                }
                Ok(Value::Number(total))
            })(),
            "IRR" => (|| {
                let values = self.numbers(sheet, arguments.get(..1).unwrap_or(&[]))?;
                let guess = optional(1, 0.1)?;
                internal_rate(&values, guess).map(Value::Number)
            })(),
            "RATE" => (|| {
                let (periods, pmt, present) = (number(0)?, number(1)?, number(2)?);
                let (future, kind, guess) =
                    (optional(3, 0.0)?, optional(4, 0.0)?, optional(5, 0.1)?);
                // The rate where the loan's flows leave the future value it names.
                let residual = |rate: f64| future - future_value(rate, periods, pmt, present, kind);
                let mut rate = guess;
                for _ in 0..MAX_ITERATIONS {
                    let value = residual(rate);
                    let step = 1e-7 * rate.abs().max(1e-3);
                    let slope = (residual(rate + step) - value) / step;
                    if slope == 0.0 || !slope.is_finite() {
                        return Err(ErrorKind::Num);
                    }
                    let next = rate - value / slope;
                    if (next - rate).abs() < TOLERANCE {
                        return Ok(Value::Number(next));
                    }
                    rate = next;
                }
                Err(ErrorKind::Num)
            })(),
            "IPMT" | "PPMT" => (|| {
                let (rate, period, periods, present) =
                    (number(0)?, number(1)?, number(2)?, number(3)?);
                let (future, kind) = (optional(4, 0.0)?, optional(5, 0.0)?);
                if period < 1.0 || period > periods {
                    return Err(ErrorKind::Num);
                }
                let pmt = payment(rate, periods, present, future, kind)?;
                let first_period = (period - 1.0).abs() < f64::EPSILON;
                let interest = if first_period {
                    if kind == 0.0 { -present * rate } else { 0.0 }
                } else if kind != 0.0 {
                    (future_value(rate, period - 2.0, pmt, present, 1.0) - pmt) * rate
                } else {
                    future_value(rate, period - 1.0, pmt, present, 0.0) * rate
                };
                Ok(Value::Number(if name == "IPMT" {
                    interest
                } else {
                    pmt - interest
                }))
            })(),
            _ => return None,
        })
    }
}

/// The payment of a loan or an annuity per period.
fn payment(
    rate: f64,
    periods: f64,
    present: f64,
    future: f64,
    kind: f64,
) -> Result<f64, ErrorKind> {
    if periods == 0.0 {
        return Err(ErrorKind::Num);
    }
    if rate == 0.0 {
        return Ok(-(present + future) / periods);
    }
    let growth = (1.0 + rate).powf(periods);
    Ok(-(rate * (present * growth + future)) / ((1.0 + rate * kind) * (growth - 1.0)))
}

fn future_value(rate: f64, periods: f64, pmt: f64, present: f64, kind: f64) -> f64 {
    if rate == 0.0 {
        return -(present + pmt * periods);
    }
    let growth = (1.0 + rate).powf(periods);
    -(present * growth + pmt * (1.0 + rate * kind) * (growth - 1.0) / rate)
}

/// The rate at which the values' present value is zero, by Newton's method.
fn internal_rate(values: &[f64], guess: f64) -> Result<f64, ErrorKind> {
    if !values.iter().any(|value| *value > 0.0) || !values.iter().any(|value| *value < 0.0) {
        return Err(ErrorKind::Num);
    }
    let mut rate = guess;
    for _ in 0..MAX_ITERATIONS {
        let mut present = 0.0;
        let mut slope = 0.0;
        for (index, value) in values.iter().enumerate() {
            let period = f64::from(u32::try_from(index).map_err(|_| ErrorKind::Num)?);
            present += value / (1.0 + rate).powf(period);
            slope -= period * value / (1.0 + rate).powf(period + 1.0);
        }
        if slope == 0.0 || !slope.is_finite() {
            return Err(ErrorKind::Num);
        }
        let next = rate - present / slope;
        if (next - rate).abs() < TOLERANCE {
            return Ok(next);
        }
        rate = next;
    }
    Err(ErrorKind::Num)
}

fn gcd(mut first: u64, mut second: u64) -> u64 {
    while second != 0 {
        (first, second) = (second, first % second);
    }
    first
}

fn leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// The week number of a date: type 1 counts weeks from Sunday, 2 from Monday, 11 to 17 from the
/// day they name, 21 by ISO 8601.
fn week_number(serial: f64, year: i64, kind: i64) -> Result<f64, ErrorKind> {
    let weekday = |serial: f64| (integer(serial).unwrap_or(0) + 6).rem_euclid(7);
    if kind == 21 {
        // ISO: the week holding the year's first Thursday is week 1.
        let monday_based = (weekday(serial) + 6) % 7;
        #[expect(clippy::cast_precision_loss, reason = "a weekday is below 7")]
        let thursday = serial - monday_based as f64 + 3.0;
        let (thursday_year, _, _) = date_from_serial(thursday).ok_or(ErrorKind::Num)?;
        let first = serial_from_date(thursday_year, 1, 1).ok_or(ErrorKind::Num)?;
        return Ok(((thursday - first) / 7.0).floor() + 1.0);
    }
    let first_day = match kind {
        1 | 17 => 0,
        2 | 11 => 1,
        12..=16 => kind - 10,
        _ => return Err(ErrorKind::Num),
    };
    let january_first = serial_from_date(year, 1, 1).ok_or(ErrorKind::Num)?;
    let offset = (weekday(january_first) - first_day).rem_euclid(7);
    #[expect(clippy::cast_precision_loss, reason = "an offset is below 7")]
    Ok(((serial - january_first + offset as f64) / 7.0).floor() + 1.0)
}

/// A date typed as text: `27/10/2026` day first, as written in Italy, or `2026-10-27`.
fn parse_date(text: &str) -> Option<f64> {
    let (date, time) = text.split_once(' ').unwrap_or((text, ""));
    let parts: Vec<i64> = date
        .split(['/', '-', '.'])
        .map(|part| part.trim().parse().ok())
        .collect::<Option<Vec<_>>>()?;
    let [first, second, third] = parts.as_slice() else {
        return None;
    };
    let (year, month, day) = if *first > 31 {
        (*first, *second, *third)
    } else {
        let year = if *third < 100 { third + 2000 } else { *third };
        (year, *second, *first)
    };
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let serial = serial_from_date(year, month, day)?;
    let time = if time.is_empty() {
        0.0
    } else {
        parse_time(time)?
    };
    Some(serial + time)
}

/// A time typed as text: `14:30` or `14:30:15`, as a fraction of a day.
fn parse_time(text: &str) -> Option<f64> {
    let parts: Vec<f64> = text
        .split(':')
        .map(|part| part.trim().parse().ok())
        .collect::<Option<Vec<_>>>()?;
    let (hours, minutes, seconds) = match parts.as_slice() {
        [hours, minutes] => (*hours, *minutes, 0.0),
        [hours, minutes, seconds] => (*hours, *minutes, *seconds),
        _ => return None,
    };
    if !(0.0..24.0).contains(&hours)
        || !(0.0..60.0).contains(&minutes)
        || !(0.0..60.0).contains(&seconds)
    {
        return None;
    }
    Some((hours * 3_600.0 + minutes * 60.0 + seconds) / 86_400.0)
}

/// The code TEXT reads, which a person types in the notation of their spreadsheet: a code with
/// a comma before its decimals and no point is Italian, and Italian date letters (`g` for
/// days, `a` for years) are read as Excel's.
fn text_code(code: &str) -> (String, NumberLocale) {
    let unquoted: String = code.split('"').step_by(2).collect();
    let (comma, point) = (unquoted.rfind(','), unquoted.rfind('.'));
    // A separator before exactly three placeholders, `#,##0` or `#.##0`, groups thousands.
    let groups_three = |separator: char| {
        unquoted.split_once(separator).is_some_and(|(_, after)| {
            after
                .chars()
                .take_while(|character| matches!(character, '#' | '0' | '?'))
                .count()
                == 3
        })
    };
    let italian = match (comma, point) {
        (Some(comma), Some(point)) => comma > point,
        (Some(_), None) => !groups_three(','),
        (None, Some(_)) => groups_three('.'),
        (None, None) => false,
    } || unquoted
        .chars()
        .any(|character| matches!(character, 'g' | 'G' | 'a' | 'A'))
        && !unquoted.to_ascii_lowercase().contains("am/pm");
    if !italian {
        return (code.to_owned(), NumberLocale::English);
    }
    let mut converted = String::with_capacity(code.len());
    let mut quoted = false;
    for character in code.chars() {
        if character == '"' {
            quoted = !quoted;
            converted.push(character);
            continue;
        }
        if quoted {
            converted.push(character);
            continue;
        }
        converted.push(match character {
            ',' => '.',
            '.' => ',',
            'g' | 'G' => 'd',
            'a' | 'A' => 'y',
            other => other,
        });
    }
    (converted, NumberLocale::Italian)
}

/// The cells of a row or a column as one list.
fn line(grid: &[Vec<Value>]) -> Vec<Value> {
    if grid.len() == 1 {
        grid[0].clone()
    } else {
        grid.iter()
            .map(|row| row.first().cloned().unwrap_or(Value::Empty))
            .collect()
    }
}

fn limited(text: String) -> Result<Value, ErrorKind> {
    if text.chars().count() > MAX_TEXT_CHARS {
        Err(ErrorKind::Value)
    } else {
        Ok(Value::Text(text))
    }
}

#[cfg(test)]
mod tests;
