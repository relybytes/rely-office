// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! The database functions, `DSUM` and its kin: a list with a heading row, the field to read and
//! a criteria range whose headings name fields. The conditions of one criteria row must all
//! hold, any row may; a text without an operator matches the values that begin with it, as in
//! Excel's advanced filter.

use super::more::{mean, variance};
use super::{Criterion, Evaluator};
use crate::formula::Expr;
use crate::value::{ErrorKind, Value};

type Answer = Option<Result<Value, ErrorKind>>;

/// The conditions of one criteria row: the database column and what it must match.
type Conditions = Vec<(usize, Criterion)>;

/// A criterion of a criteria cell: a plain text matches what begins with it.
fn database_criterion(value: &Value) -> Criterion {
    if let Value::Text(text) = value
        && !text.starts_with(['<', '>', '='])
        && text.parse::<f64>().is_err()
    {
        return Criterion::parse(&Value::Text(format!("{text}*")));
    }
    Criterion::parse(value)
}

fn heading(value: &Value) -> String {
    value.as_text().unwrap_or_default().trim().to_lowercase()
}

impl Evaluator<'_> {
    /// The values of the field of the records the criteria select.
    fn selected_values(&self, sheet: usize, arguments: &[Expr]) -> Result<Vec<Value>, ErrorKind> {
        let database = self.grid(sheet, arguments.first())?;
        let criteria = self.grid(sheet, arguments.get(2))?;
        let Some((headings, records)) = database.split_first() else {
            return Err(ErrorKind::Value);
        };
        let field = match arguments.get(1) {
            None => None,
            Some(Expr::Text(text)) if text.is_empty() => None,
            Some(expression) => Some(match self.scalar(sheet, expression) {
                Value::Number(number) => {
                    let index = number.trunc();
                    #[expect(
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss,
                        clippy::cast_precision_loss,
                        reason = "checked to lie within the database's columns"
                    )]
                    if index >= 1.0 && index <= headings.len() as f64 {
                        index as usize - 1
                    } else {
                        return Err(ErrorKind::Value);
                    }
                }
                Value::Error(kind) => return Err(kind),
                other => {
                    let wanted = heading(&other);
                    headings
                        .iter()
                        .position(|known| heading(known) == wanted)
                        .ok_or(ErrorKind::Value)?
                }
            }),
        };
        let Some((criteria_headings, criteria_rows)) = criteria.split_first() else {
            return Err(ErrorKind::Value);
        };
        let columns: Vec<Option<usize>> = criteria_headings
            .iter()
            .map(|name| {
                let wanted = heading(name);
                headings.iter().position(|known| heading(known) == wanted)
            })
            .collect();
        let rows: Vec<Conditions> = criteria_rows
            .iter()
            .map(|row| {
                row.iter()
                    .zip(&columns)
                    .filter(|(value, _)| !matches!(value, Value::Empty))
                    .filter_map(|(value, column)| {
                        column.map(|column| (column, database_criterion(value)))
                    })
                    .collect()
            })
            .collect();
        let selected = |record: &Vec<Value>| {
            rows.is_empty()
                || rows.iter().any(|conditions| {
                    conditions.iter().all(|(column, criterion)| {
                        criterion.matches(record.get(*column).unwrap_or(&Value::Empty))
                    })
                })
        };
        Ok(records
            .iter()
            .filter(|record| selected(record))
            .map(|record| match field {
                Some(column) => record.get(column).cloned().unwrap_or(Value::Empty),
                None => Value::Bool(true),
            })
            .collect())
    }

    /// A database function, or `None` when the name is none of them.
    pub(super) fn database(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        if !matches!(
            name,
            "DAVERAGE"
                | "DCOUNT"
                | "DCOUNTA"
                | "DGET"
                | "DMAX"
                | "DMIN"
                | "DPRODUCT"
                | "DSTDEV"
                | "DSTDEVP"
                | "DSUM"
                | "DVAR"
                | "DVARP"
        ) {
            return None;
        }
        Some((|| {
            if arguments.len() != 3 {
                return Err(ErrorKind::Value);
            }
            let values = self.selected_values(sheet, arguments)?;
            let numbers: Vec<f64> = values
                .iter()
                .filter_map(|value| match value {
                    Value::Number(number) => Some(*number),
                    _ => None,
                })
                .collect();
            #[expect(clippy::cast_precision_loss, reason = "record counts are small")]
            let count = |count: usize| Value::Number(count as f64);
            Ok(match name {
                "DCOUNT" => count(
                    if matches!(arguments.get(1), Some(Expr::Text(text)) if text.is_empty()) {
                        values.len()
                    } else {
                        numbers.len()
                    },
                ),
                "DCOUNTA" => count(
                    values
                        .iter()
                        .filter(|value| !matches!(value, Value::Empty))
                        .count(),
                ),
                "DGET" => match values.as_slice() {
                    [] => return Err(ErrorKind::Value),
                    [only] => only.clone(),
                    _ => return Err(ErrorKind::Num),
                },
                "DSUM" => Value::Number(numbers.iter().sum()),
                "DPRODUCT" => Value::Number(if numbers.is_empty() {
                    0.0
                } else {
                    numbers.iter().product()
                }),
                "DMAX" => Value::Number(numbers.iter().copied().reduce(f64::max).unwrap_or(0.0)),
                "DMIN" => Value::Number(numbers.iter().copied().reduce(f64::min).unwrap_or(0.0)),
                "DAVERAGE" => {
                    if numbers.is_empty() {
                        return Err(ErrorKind::Div0);
                    }
                    Value::Number(mean(&numbers))
                }
                "DSTDEV" => Value::Number(variance(&numbers, true)?.sqrt()),
                "DSTDEVP" => Value::Number(variance(&numbers, false)?.sqrt()),
                "DVAR" => Value::Number(variance(&numbers, true)?),
                _ => Value::Number(variance(&numbers, false)?),
            })
        })())
    }
}

#[cfg(test)]
mod tests;
