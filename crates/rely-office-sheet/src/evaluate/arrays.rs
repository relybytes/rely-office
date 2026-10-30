// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! The functions that compute an array, which spills from the formula's cell: filtering,
//! sorting, unique values, sequences, and taking, dropping, stacking and reshaping rows and
//! columns, with matrix products and text split into cells.

#![allow(
    clippy::float_cmp,
    reason = "the flags, orders and values compared here are read from cells, exact by construction"
)]

use std::cmp::Ordering;

use super::Evaluator;
use crate::arrange::compare_values;
use crate::formula::Expr;
use crate::value::{ErrorKind, Value};

type Grid = Vec<Vec<Value>>;
type Answer = Option<Result<Grid, ErrorKind>>;

/// The most cells an array function builds, as the spill allows.
const MAX_ARRAY_CELLS: usize = super::MAX_SPILL_CELLS;

fn size(grid: &Grid) -> (usize, usize) {
    (grid.len(), grid.first().map_or(0, Vec::len))
}

fn transposed(grid: &Grid) -> Grid {
    let (rows, columns) = size(grid);
    (0..columns)
        .map(|column| (0..rows).map(|row| grid[row][column].clone()).collect())
        .collect()
}

/// A rectangle of `rows` by `columns` from a list read row by row, padded with `pad`.
fn rectangle(values: &[Value], columns: usize, pad: &Value) -> Grid {
    let mut grid: Grid = values
        .chunks(columns.max(1))
        .map(<[Value]>::to_vec)
        .collect();
    if let Some(last) = grid.last_mut() {
        last.resize(columns, pad.clone());
    }
    grid
}

/// Whether two values are the same for `UNIQUE`: text without regard to case.
fn same(first: &Value, second: &Value) -> bool {
    match (first, second) {
        (Value::Text(first), Value::Text(second)) => first.to_lowercase() == second.to_lowercase(),
        _ => first == second,
    }
}

/// The order of two values for a sort, blanks last whichever the direction.
fn sort_order(first: &Value, second: &Value, descending: bool) -> Ordering {
    match (first, second) {
        (Value::Empty, Value::Empty) => Ordering::Equal,
        (Value::Empty, _) => Ordering::Greater,
        (_, Value::Empty) => Ordering::Less,
        _ if descending => compare_values(second, first),
        _ => compare_values(first, second),
    }
}

/// A count from an argument, at least one, at most the array limit.
fn count(number: f64) -> Result<usize, ErrorKind> {
    let number = number.trunc();
    #[expect(
        clippy::cast_precision_loss,
        reason = "the array limit is far below 2^52"
    )]
    let limit = MAX_ARRAY_CELLS as f64;
    if !(1.0..=limit).contains(&number) {
        return Err(ErrorKind::Value);
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "checked range"
    )]
    Ok(number as usize)
}

/// Positions picked from a length by one-based indexes, negative ones counted from the end.
fn picked(indexes: &[f64], length: usize) -> Result<Vec<usize>, ErrorKind> {
    indexes
        .iter()
        .map(|index| {
            let index = index.trunc();
            #[expect(clippy::cast_precision_loss, reason = "a length stays small")]
            let length_number = length as f64;
            if index == 0.0 || index.abs() > length_number {
                return Err(ErrorKind::Value);
            }
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "checked range"
            )]
            Ok(if index > 0.0 {
                index as usize - 1
            } else {
                length - (-index) as usize
            })
        })
        .collect()
}

/// The first or last `take` of `length` places, negative counting from the end.
fn span(take: f64, length: usize) -> std::ops::Range<usize> {
    let take = take.trunc();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to the length"
    )]
    let amount = (take.abs() as usize).min(length);
    if take >= 0.0 {
        0..amount
    } else {
        length - amount..length
    }
}

fn check(grid: Grid) -> Result<Grid, ErrorKind> {
    let (rows, columns) = size(&grid);
    if rows * columns > MAX_ARRAY_CELLS {
        return Err(ErrorKind::Spill);
    }
    if rows == 0 || columns == 0 {
        return Err(ErrorKind::Calc);
    }
    Ok(grid)
}

impl Evaluator<'_> {
    /// A function that computes an array, or `None` when the name is none of them.
    #[expect(clippy::too_many_lines, reason = "one arm per function, each short")]
    pub(super) fn array_call(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        if !ARRAY_FUNCTIONS.contains(&name) {
            return None;
        }
        let argument = |index: usize| arguments.get(index);
        let grid = |index: usize| self.grid(sheet, argument(index));
        let number = |index: usize, default: f64| -> Result<f64, ErrorKind> {
            match argument(index) {
                None => Ok(default),
                Some(Expr::Text(text)) if text.is_empty() => Ok(default),
                Some(expression) => self.scalar(sheet, expression).as_number(),
            }
        };
        let flag = |index: usize| -> Result<bool, ErrorKind> {
            argument(index).map_or(Ok(false), |expression| {
                self.scalar(sheet, expression).as_bool()
            })
        };
        let numbers_of = |index: usize| -> Result<Vec<f64>, ErrorKind> {
            grid(index)?
                .into_iter()
                .flatten()
                .map(|value| value.as_number())
                .collect()
        };
        Some((|| match name {
            "FILTER" => {
                let array = grid(0)?;
                let include = grid(1)?;
                let (rows, columns) = size(&array);
                let keep = |values: Vec<Value>| -> Result<Vec<bool>, ErrorKind> {
                    values
                        .iter()
                        .map(|value| {
                            value
                                .as_bool()
                                .or_else(|_| value.as_number().map(|number| number != 0.0))
                        })
                        .collect()
                };
                let filtered = match size(&include) {
                    (height, 1) if height == rows => {
                        let kept = keep(include.into_iter().flatten().collect())?;
                        array
                            .into_iter()
                            .zip(kept)
                            .filter(|(_, kept)| *kept)
                            .map(|(row, _)| row)
                            .collect::<Grid>()
                    }
                    (1, width) if width == columns => {
                        let kept = keep(include.into_iter().flatten().collect())?;
                        let chosen = transposed(&array)
                            .into_iter()
                            .zip(kept)
                            .filter(|(_, kept)| *kept)
                            .map(|(column, _)| column)
                            .collect::<Grid>();
                        if chosen.is_empty() {
                            Vec::new()
                        } else {
                            transposed(&chosen)
                        }
                    }
                    _ => return Err(ErrorKind::Value),
                };
                if filtered.is_empty() {
                    return match argument(2) {
                        Some(_) => grid(2),
                        None => Err(ErrorKind::Calc),
                    };
                }
                check(filtered)
            }
            "SORT" => {
                let mut array = grid(0)?;
                let by_column = flag(3)?;
                if by_column {
                    array = transposed(&array);
                }
                let keys = if argument(1).is_some() {
                    numbers_of(1)?
                } else {
                    vec![1.0]
                };
                let orders = if argument(2).is_some() {
                    numbers_of(2)?
                } else {
                    vec![1.0]
                };
                let width = size(&array).1;
                let keys = picked(&keys, width)?;
                if orders
                    .iter()
                    .any(|order| order.trunc() != 1.0 && order.trunc() != -1.0)
                {
                    return Err(ErrorKind::Value);
                }
                array.sort_by(|first, second| {
                    for (position, key) in keys.iter().enumerate() {
                        let descending = orders
                            .get(position)
                            .or(orders.last())
                            .copied()
                            .unwrap_or(1.0)
                            < 0.0;
                        let order = sort_order(&first[*key], &second[*key], descending);
                        if order != Ordering::Equal {
                            return order;
                        }
                    }
                    Ordering::Equal
                });
                check(if by_column { transposed(&array) } else { array })
            }
            "SORTBY" => {
                let array = grid(0)?;
                let (rows, columns) = size(&array);
                let mut keys: Vec<(Vec<Value>, bool)> = Vec::new();
                let mut by_column = None;
                let mut index = 1;
                while index < arguments.len() {
                    let key = grid(index)?;
                    let descending = number(index + 1, 1.0)?.trunc() < 0.0;
                    let column = match size(&key) {
                        (height, 1) if height == rows => false,
                        (1, width) if width == columns => true,
                        _ => return Err(ErrorKind::Value),
                    };
                    if by_column.is_some_and(|known| known != column) {
                        return Err(ErrorKind::Value);
                    }
                    by_column = Some(column);
                    keys.push((key.into_iter().flatten().collect(), descending));
                    index += 2;
                }
                if keys.is_empty() {
                    return Err(ErrorKind::Value);
                }
                let by_column = by_column.unwrap_or(false);
                let lines = if by_column { transposed(&array) } else { array };
                let mut order: Vec<usize> = (0..lines.len()).collect();
                order.sort_by(|first, second| {
                    for (key, descending) in &keys {
                        let ordering = sort_order(&key[*first], &key[*second], *descending);
                        if ordering != Ordering::Equal {
                            return ordering;
                        }
                    }
                    Ordering::Equal
                });
                let sorted: Grid = order
                    .into_iter()
                    .map(|position| lines[position].clone())
                    .collect();
                check(if by_column {
                    transposed(&sorted)
                } else {
                    sorted
                })
            }
            "UNIQUE" => {
                let by_column = flag(1)?;
                let once = flag(2)?;
                let array = grid(0)?;
                let lines = if by_column { transposed(&array) } else { array };
                let equal = |first: &[Value], second: &[Value]| {
                    first
                        .iter()
                        .zip(second)
                        .all(|(left, right)| same(left, right))
                };
                let mut distinct: Vec<(Vec<Value>, usize)> = Vec::new();
                for line in lines {
                    match distinct.iter_mut().find(|(known, _)| equal(known, &line)) {
                        Some((_, seen)) => *seen += 1,
                        None => distinct.push((line, 1)),
                    }
                }
                let unique: Grid = distinct
                    .into_iter()
                    .filter(|(_, seen)| !once || *seen == 1)
                    .map(|(line, _)| line)
                    .collect();
                if unique.is_empty() {
                    return Err(ErrorKind::Calc);
                }
                check(if by_column {
                    transposed(&unique)
                } else {
                    unique
                })
            }
            "SEQUENCE" => {
                let rows = count(number(0, 1.0)?)?;
                let columns = count(number(1, 1.0)?)?;
                let start = number(2, 1.0)?;
                let step = number(3, 1.0)?;
                if rows * columns > MAX_ARRAY_CELLS {
                    return Err(ErrorKind::Spill);
                }
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "positions stay below the array limit"
                )]
                let sequence = (0..rows)
                    .map(|row| {
                        (0..columns)
                            .map(|column| {
                                Value::Number(start + step * (row * columns + column) as f64)
                            })
                            .collect()
                    })
                    .collect();
                check(sequence)
            }
            "TRANSPOSE" => check(transposed(&grid(0)?)),
            "TAKE" | "DROP" => {
                let array = grid(0)?;
                let (rows, columns) = size(&array);
                let row_take = number(
                    1,
                    if name == "TAKE" {
                        f64::from(u32::MAX)
                    } else {
                        0.0
                    },
                )?;
                let column_take = number(
                    2,
                    if name == "TAKE" {
                        f64::from(u32::MAX)
                    } else {
                        0.0
                    },
                )?;
                let range = |take: f64, length: usize| {
                    if name == "TAKE" {
                        span(take, length)
                    } else {
                        let dropped = span(take, length);
                        if take >= 0.0 {
                            dropped.end..length
                        } else {
                            0..dropped.start
                        }
                    }
                };
                let row_range = range(row_take, rows);
                let column_range = range(column_take, columns);
                check(
                    array[row_range]
                        .iter()
                        .map(|row| row[column_range.clone()].to_vec())
                        .collect(),
                )
            }
            "CHOOSEROWS" | "CHOOSECOLS" => {
                let array = grid(0)?;
                let lines = if name == "CHOOSECOLS" {
                    transposed(&array)
                } else {
                    array
                };
                let mut indexes = Vec::new();
                for index in 1..arguments.len() {
                    indexes.extend(numbers_of(index)?);
                }
                if indexes.is_empty() {
                    return Err(ErrorKind::Value);
                }
                let chosen: Grid = picked(&indexes, lines.len())?
                    .into_iter()
                    .map(|index| lines[index].clone())
                    .collect();
                check(if name == "CHOOSECOLS" {
                    transposed(&chosen)
                } else {
                    chosen
                })
            }
            "VSTACK" | "HSTACK" => {
                let mut parts = Vec::new();
                for index in 0..arguments.len() {
                    let part = grid(index)?;
                    parts.push(if name == "HSTACK" {
                        transposed(&part)
                    } else {
                        part
                    });
                }
                let width = parts.iter().map(|part| size(part).1).max().unwrap_or(0);
                let mut stacked: Grid = Vec::new();
                for part in parts {
                    for mut line in part {
                        line.resize(width, Value::Error(ErrorKind::NotAvailable));
                        stacked.push(line);
                    }
                }
                check(if name == "HSTACK" {
                    transposed(&stacked)
                } else {
                    stacked
                })
            }
            "TOCOL" | "TOROW" => {
                let array = grid(0)?;
                let ignore = number(1, 0.0)?.trunc();
                let (skip_blanks, skip_errors) = (
                    ignore == 1.0 || ignore == 3.0,
                    ignore == 2.0 || ignore == 3.0,
                );
                let by_column = flag(2)?;
                let lines = if by_column { transposed(&array) } else { array };
                let values: Vec<Value> = lines
                    .into_iter()
                    .flatten()
                    .filter(|value| match value {
                        Value::Empty => !skip_blanks,
                        Value::Error(_) => !skip_errors,
                        _ => true,
                    })
                    .collect();
                if values.is_empty() {
                    return Err(ErrorKind::Calc);
                }
                check(if name == "TOCOL" {
                    values.into_iter().map(|value| vec![value]).collect()
                } else {
                    vec![values]
                })
            }
            "WRAPROWS" | "WRAPCOLS" => {
                let values: Vec<Value> = grid(0)?.into_iter().flatten().collect();
                let wrap = count(number(1, 0.0)?)?;
                let pad = match argument(2) {
                    Some(expression) => self.scalar(sheet, expression),
                    None => Value::Error(ErrorKind::NotAvailable),
                };
                let wrapped = rectangle(&values, wrap, &pad);
                check(if name == "WRAPCOLS" {
                    transposed(&wrapped)
                } else {
                    wrapped
                })
            }
            "EXPAND" => {
                let array = grid(0)?;
                let (rows, columns) = size(&array);
                #[expect(clippy::cast_precision_loss, reason = "a size stays small")]
                let (rows_number, columns_number) = (rows as f64, columns as f64);
                let target_rows = count(number(1, rows_number)?)?;
                let target_columns = count(number(2, columns_number)?)?;
                if target_rows < rows || target_columns < columns {
                    return Err(ErrorKind::Value);
                }
                let pad = match argument(3) {
                    Some(expression) => self.scalar(sheet, expression),
                    None => Value::Error(ErrorKind::NotAvailable),
                };
                let mut expanded = array;
                for line in &mut expanded {
                    line.resize(target_columns, pad.clone());
                }
                expanded.resize(target_rows, vec![pad; target_columns]);
                check(expanded)
            }
            "MMULT" => {
                let left = grid(0)?;
                let right = grid(1)?;
                let (rows, inner) = size(&left);
                let (inner_right, columns) = size(&right);
                if inner != inner_right {
                    return Err(ErrorKind::Value);
                }
                let mut product = Vec::with_capacity(rows);
                for left_row in &left {
                    let mut line = Vec::with_capacity(columns);
                    for column in 0..columns {
                        let mut total = 0.0;
                        for (step, right_row) in right.iter().enumerate().take(inner) {
                            let (Value::Number(first), Value::Number(second)) =
                                (&left_row[step], &right_row[column])
                            else {
                                return Err(ErrorKind::Value);
                            };
                            total += first * second;
                        }
                        line.push(Value::Number(total));
                    }
                    product.push(line);
                }
                check(product)
            }
            "MUNIT" => {
                let side = count(number(0, 0.0)?)?;
                if side * side > MAX_ARRAY_CELLS {
                    return Err(ErrorKind::Spill);
                }
                check(
                    (0..side)
                        .map(|row| {
                            (0..side)
                                .map(|column| Value::Number(f64::from(u8::from(row == column))))
                                .collect()
                        })
                        .collect(),
                )
            }
            "FREQUENCY" => {
                let data: Vec<f64> = grid(0)?
                    .into_iter()
                    .flatten()
                    .filter_map(|value| match value {
                        Value::Number(number) => Some(number),
                        _ => None,
                    })
                    .collect();
                let mut bins: Vec<f64> = grid(1)?
                    .into_iter()
                    .flatten()
                    .filter_map(|value| match value {
                        Value::Number(number) => Some(number),
                        _ => None,
                    })
                    .collect();
                bins.sort_by(f64::total_cmp);
                let mut counts = vec![0_u32; bins.len() + 1];
                for value in data {
                    let bin = bins
                        .iter()
                        .position(|edge| value <= *edge)
                        .unwrap_or(bins.len());
                    counts[bin] += 1;
                }
                check(
                    counts
                        .into_iter()
                        .map(|count| vec![Value::Number(f64::from(count))])
                        .collect(),
                )
            }
            "TEXTSPLIT" => {
                let text = self.text(sheet, argument(0))?;
                let column_delimiter = match argument(1) {
                    Some(expression) => self.scalar(sheet, expression).as_text()?,
                    None => String::new(),
                };
                let row_delimiter = match argument(2) {
                    Some(Expr::Text(text)) if text.is_empty() => String::new(),
                    Some(expression) => self.scalar(sheet, expression).as_text()?,
                    None => String::new(),
                };
                let ignore_empty = flag(3)?;
                let pad = match argument(5) {
                    Some(expression) => self.scalar(sheet, expression),
                    None => Value::Error(ErrorKind::NotAvailable),
                };
                let split = |part: &str, delimiter: &str| -> Vec<String> {
                    if delimiter.is_empty() {
                        vec![part.to_owned()]
                    } else {
                        part.split(delimiter)
                            .map(str::to_owned)
                            .filter(|piece| !ignore_empty || !piece.is_empty())
                            .collect()
                    }
                };
                let rows: Vec<Vec<Value>> = split(&text, &row_delimiter)
                    .into_iter()
                    .map(|line| {
                        split(&line, &column_delimiter)
                            .into_iter()
                            .map(Value::Text)
                            .collect()
                    })
                    .collect();
                let width = rows.iter().map(Vec::len).max().unwrap_or(0);
                check(
                    rows.into_iter()
                        .map(|mut line| {
                            line.resize(width, pad.clone());
                            line
                        })
                        .collect(),
                )
            }
            _ => Err(ErrorKind::Name),
        })())
    }
}

/// The functions this module computes.
pub(crate) const ARRAY_FUNCTIONS: &[&str] = &[
    "CHOOSECOLS",
    "CHOOSEROWS",
    "DROP",
    "EXPAND",
    "FILTER",
    "FREQUENCY",
    "HSTACK",
    "MMULT",
    "MUNIT",
    "SEQUENCE",
    "SORT",
    "SORTBY",
    "TAKE",
    "TEXTSPLIT",
    "TOCOL",
    "TOROW",
    "TRANSPOSE",
    "UNIQUE",
    "VSTACK",
    "WRAPCOLS",
    "WRAPROWS",
];

#[cfg(test)]
mod tests;
