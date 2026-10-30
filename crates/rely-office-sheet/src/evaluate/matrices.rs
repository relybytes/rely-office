// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! The functions that answer an array computed from numbers: the inverse of a matrix, the
//! least-squares fits of `LINEST` and `LOGEST` with their statistics and the values `TREND` and
//! `GROWTH` predict from them, the modes of `MODE.MULT`, the random numbers of `RANDARRAY` and
//! the matches of `REGEXEXTRACT`.

#![allow(
    clippy::float_cmp,
    reason = "flags and whole numbers are compared exactly, as Excel compares them"
)]

use std::cmp::Ordering;

use super::{Evaluator, MAX_SPILL_CELLS};
use crate::formula::Expr;
use crate::value::{ErrorKind, Value};

type Grid = Vec<Vec<Value>>;
type Answer = Option<Result<Grid, ErrorKind>>;

/// The inverse of a square matrix by Gauss-Jordan elimination; `None` when it is singular.
pub(super) fn inverse(matrix: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let size = matrix.len();
    let mut work: Vec<Vec<f64>> = matrix
        .iter()
        .enumerate()
        .map(|(row, values)| {
            let mut line = values.clone();
            line.extend((0..size).map(|column| if column == row { 1.0 } else { 0.0 }));
            line
        })
        .collect();
    for column in 0..size {
        let pivot = (column..size).max_by(|first, second| {
            work[*first][column]
                .abs()
                .partial_cmp(&work[*second][column].abs())
                .unwrap_or(Ordering::Equal)
        })?;
        if work[pivot][column].abs() < 1e-300 {
            return None;
        }
        work.swap(pivot, column);
        let divisor = work[column][column];
        for value in &mut work[column] {
            *value /= divisor;
        }
        let pivot_row = work[column].clone();
        for (index, row) in work.iter_mut().enumerate() {
            let factor = row[column];
            if index != column && factor != 0.0 {
                for (value, pivot) in row.iter_mut().zip(&pivot_row) {
                    *value -= factor * pivot;
                }
            }
        }
    }
    Some(work.into_iter().map(|row| row[size..].to_vec()).collect())
}

/// A least-squares fit: the coefficients of the variables, the intercept, and the inverse of
/// the normal matrix with the residual statistics.
struct Fit {
    slopes: Vec<f64>,
    intercept: f64,
    with_intercept: bool,
    normal_inverse: Vec<Vec<f64>>,
    observations: usize,
    residual_squares: f64,
    total_squares: f64,
}

fn fit(xs: &[Vec<f64>], ys: &[f64], with_intercept: bool) -> Result<Fit, ErrorKind> {
    let variables = xs.first().map_or(0, Vec::len);
    let columns = variables + usize::from(with_intercept);
    let observations = ys.len();
    if observations == 0 || columns == 0 || observations < columns {
        return Err(ErrorKind::NotAvailable);
    }
    let design: Vec<Vec<f64>> = xs
        .iter()
        .map(|row| {
            let mut line = row.clone();
            if with_intercept {
                line.push(1.0);
            }
            line
        })
        .collect();
    let mut normal = vec![vec![0.0; columns]; columns];
    let mut moment = vec![0.0; columns];
    for (row, y) in design.iter().zip(ys) {
        for first in 0..columns {
            moment[first] += row[first] * y;
            for second in 0..columns {
                normal[first][second] += row[first] * row[second];
            }
        }
    }
    let normal_inverse = inverse(&normal).ok_or(ErrorKind::Num)?;
    let coefficients: Vec<f64> = (0..columns)
        .map(|row| {
            (0..columns)
                .map(|column| normal_inverse[row][column] * moment[column])
                .sum()
        })
        .collect();
    let residual_squares: f64 = design
        .iter()
        .zip(ys)
        .map(|(row, y)| {
            let predicted: f64 = row.iter().zip(&coefficients).map(|(x, b)| x * b).sum();
            (y - predicted).powi(2)
        })
        .sum();
    #[expect(clippy::cast_precision_loss, reason = "observation counts are small")]
    let average = ys.iter().sum::<f64>() / observations as f64;
    let total_squares: f64 = if with_intercept {
        ys.iter().map(|y| (y - average).powi(2)).sum()
    } else {
        ys.iter().map(|y| y * y).sum()
    };
    Ok(Fit {
        slopes: coefficients[..variables].to_vec(),
        intercept: if with_intercept {
            coefficients[variables]
        } else {
            0.0
        },
        with_intercept,
        normal_inverse,
        observations,
        residual_squares,
        total_squares,
    })
}

impl Fit {
    fn predict(&self, row: &[f64]) -> f64 {
        row.iter()
            .zip(&self.slopes)
            .map(|(x, slope)| x * slope)
            .sum::<f64>()
            + self.intercept
    }

    /// The rows `LINEST` answers: the coefficients last variable first and the intercept, then
    /// with `statistics` their standard errors, r² and the standard error of y, F and the
    /// degrees of freedom, and the regression and residual sums of squares.
    fn rows(&self, statistics: bool, transform: fn(f64) -> f64) -> Grid {
        let width = self.slopes.len() + 1;
        let mut first: Vec<Value> = self
            .slopes
            .iter()
            .rev()
            .map(|slope| Value::Number(transform(*slope)))
            .collect();
        first.push(Value::Number(transform(self.intercept)));
        if !statistics {
            return vec![first];
        }
        let variables = self.slopes.len();
        let parameters = variables + usize::from(self.with_intercept);
        let freedom = self.observations.saturating_sub(parameters);
        #[expect(clippy::cast_precision_loss, reason = "observation counts are small")]
        let freedom_value = freedom as f64;
        let residual_variance = if freedom > 0 {
            self.residual_squares / freedom_value
        } else {
            0.0
        };
        let error = |index: usize| (self.normal_inverse[index][index] * residual_variance).sqrt();
        let mut errors: Vec<Value> = (0..variables)
            .rev()
            .map(|index| Value::Number(error(index)))
            .collect();
        errors.push(if self.with_intercept {
            Value::Number(error(variables))
        } else {
            Value::Error(ErrorKind::NotAvailable)
        });
        let regression_squares = self.total_squares - self.residual_squares;
        let determination = if self.total_squares == 0.0 {
            1.0
        } else {
            regression_squares / self.total_squares
        };
        #[expect(clippy::cast_precision_loss, reason = "variable counts are small")]
        let f = if self.residual_squares == 0.0 || freedom == 0 {
            Value::Error(ErrorKind::Num)
        } else {
            Value::Number((regression_squares / variables as f64) / residual_variance)
        };
        let pad = |mut row: Vec<Value>| {
            row.resize(width, Value::Error(ErrorKind::NotAvailable));
            row
        };
        vec![
            first,
            errors,
            pad(vec![
                Value::Number(determination),
                Value::Number(residual_variance.sqrt()),
            ]),
            pad(vec![f, Value::Number(freedom_value)]),
            pad(vec![
                Value::Number(regression_squares),
                Value::Number(self.residual_squares),
            ]),
        ]
    }
}

/// The observations of a fit: each row of variables and its y, read the way the known y's lie,
/// a column of observations or a row of them.
/// The rows of variables, the y of each, and whether the y's lie down a column.
type Observations = (Vec<Vec<f64>>, Vec<f64>, bool);

fn observations(ys: &Grid, xs: Option<&Grid>) -> Result<Observations, ErrorKind> {
    let numbers = |grid: &Grid| -> Result<Vec<Vec<f64>>, ErrorKind> {
        grid.iter()
            .map(|row| {
                row.iter()
                    .map(|value| match value {
                        Value::Number(number) => Ok(*number),
                        Value::Error(kind) => Err(*kind),
                        _ => Err(ErrorKind::Value),
                    })
                    .collect()
            })
            .collect()
    };
    let y_grid = numbers(ys)?;
    let rows = y_grid.len();
    let columns = y_grid.first().map_or(0, Vec::len);
    let by_column = columns == 1;
    if !by_column && rows != 1 {
        return Err(ErrorKind::Ref);
    }
    let y: Vec<f64> = y_grid.into_iter().flatten().collect();
    let count = y.len();
    let x: Vec<Vec<f64>> = match xs {
        None => (1..=count)
            .map(|index| vec![f64::from(u32::try_from(index).unwrap_or(u32::MAX))])
            .collect(),
        Some(grid) => {
            let x_grid = numbers(grid)?;
            let x_rows = x_grid.len();
            let x_columns = x_grid.first().map_or(0, Vec::len);
            if x_rows == count && (by_column || x_columns != count) {
                x_grid
            } else if x_columns == count {
                (0..count)
                    .map(|index| x_grid.iter().map(|row| row[index]).collect())
                    .collect()
            } else {
                return Err(ErrorKind::Ref);
            }
        }
    };
    Ok((x, y, by_column))
}

impl Evaluator<'_> {
    fn optional_flag(
        &self,
        sheet: usize,
        argument: Option<&Expr>,
        default: bool,
    ) -> Result<bool, ErrorKind> {
        match argument {
            None => Ok(default),
            Some(Expr::Text(text)) if text.is_empty() => Ok(default),
            Some(expression) => self.scalar(sheet, expression).as_bool(),
        }
    }

    fn optional_grid(
        &self,
        sheet: usize,
        argument: Option<&Expr>,
    ) -> Result<Option<Grid>, ErrorKind> {
        match argument {
            None => Ok(None),
            Some(Expr::Text(text)) if text.is_empty() => Ok(None),
            Some(expression) => self.grid(sheet, Some(expression)).map(Some),
        }
    }

    /// An array function of this set, or `None` when the name is none of them.
    #[expect(clippy::too_many_lines, reason = "one arm per function, each short")]
    pub(super) fn matrix_call(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        let argument = |index: usize| arguments.get(index);
        Some(match name {
            "MINVERSE" => (|| {
                let matrix = self.numeric_matrix(sheet, argument(0))?;
                if matrix.is_empty() || matrix.iter().any(|row| row.len() != matrix.len()) {
                    return Err(ErrorKind::Value);
                }
                let inverted = inverse(&matrix).ok_or(ErrorKind::Num)?;
                Ok(inverted
                    .into_iter()
                    .map(|row| row.into_iter().map(Value::Number).collect())
                    .collect())
            })(),
            "LINEST" | "LOGEST" => (|| {
                let logarithmic = name == "LOGEST";
                let ys = self.grid(sheet, argument(0))?;
                let xs = self.optional_grid(sheet, argument(1))?;
                let with_intercept = self.optional_flag(sheet, argument(2), true)?;
                let statistics = self.optional_flag(sheet, argument(3), false)?;
                let (x, mut y, _) = observations(&ys, xs.as_ref())?;
                if logarithmic {
                    if y.iter().any(|value| *value <= 0.0) {
                        return Err(ErrorKind::Num);
                    }
                    for value in &mut y {
                        *value = value.ln();
                    }
                }
                let fitted = fit(&x, &y, with_intercept)?;
                let transform: fn(f64) -> f64 = if logarithmic { f64::exp } else { |value| value };
                let mut rows = fitted.rows(statistics, transform);
                if logarithmic
                    && !with_intercept
                    && let Some(intercept) = rows[0].last_mut()
                {
                    *intercept = Value::Number(1.0);
                }
                Ok(rows)
            })(),
            "TREND" | "GROWTH" => (|| {
                let logarithmic = name == "GROWTH";
                let ys = self.grid(sheet, argument(0))?;
                let xs = self.optional_grid(sheet, argument(1))?;
                let new_xs = self.optional_grid(sheet, argument(2))?;
                let with_intercept = self.optional_flag(sheet, argument(3), true)?;
                let (x, mut y, by_column) = observations(&ys, xs.as_ref())?;
                if logarithmic {
                    if y.iter().any(|value| *value <= 0.0) {
                        return Err(ErrorKind::Num);
                    }
                    for value in &mut y {
                        *value = value.ln();
                    }
                }
                let fitted = fit(&x, &y, with_intercept)?;
                let variables = fitted.slopes.len();
                let finish =
                    |value: f64| Value::Number(if logarithmic { value.exp() } else { value });
                let Some(new_xs) = new_xs else {
                    let predicted: Vec<Value> =
                        x.iter().map(|row| finish(fitted.predict(row))).collect();
                    return Ok(if by_column {
                        predicted.into_iter().map(|value| vec![value]).collect()
                    } else {
                        vec![predicted]
                    });
                };
                let numbers: Vec<Vec<f64>> = new_xs
                    .iter()
                    .map(|row| {
                        row.iter()
                            .map(|value| match value {
                                Value::Number(number) => Ok(*number),
                                Value::Error(kind) => Err(*kind),
                                _ => Err(ErrorKind::Value),
                            })
                            .collect::<Result<Vec<f64>, ErrorKind>>()
                    })
                    .collect::<Result<_, _>>()?;
                if variables == 1 {
                    return Ok(numbers
                        .iter()
                        .map(|row| row.iter().map(|x| finish(fitted.predict(&[*x]))).collect())
                        .collect());
                }
                if by_column {
                    if numbers.iter().any(|row| row.len() != variables) {
                        return Err(ErrorKind::Ref);
                    }
                    Ok(numbers
                        .iter()
                        .map(|row| vec![finish(fitted.predict(row))])
                        .collect())
                } else {
                    if numbers.len() != variables {
                        return Err(ErrorKind::Ref);
                    }
                    let columns = numbers[0].len();
                    Ok(vec![
                        (0..columns)
                            .map(|column| {
                                let row: Vec<f64> =
                                    numbers.iter().map(|line| line[column]).collect();
                                finish(fitted.predict(&row))
                            })
                            .collect(),
                    ])
                }
            })(),
            "MODE.MULT" => (|| {
                let numbers = self.numbers(sheet, arguments)?;
                let mut counted: Vec<(f64, usize)> = Vec::new();
                for number in &numbers {
                    match counted.iter_mut().find(|(value, _)| value == number) {
                        Some((_, times)) => *times += 1,
                        None => counted.push((*number, 1)),
                    }
                }
                let most = counted.iter().map(|(_, times)| *times).max().unwrap_or(0);
                if most < 2 {
                    return Err(ErrorKind::NotAvailable);
                }
                Ok(counted
                    .into_iter()
                    .filter(|(_, times)| *times == most)
                    .map(|(value, _)| vec![Value::Number(value)])
                    .collect())
            })(),
            "RANDARRAY" => (|| {
                let number = |index: usize, default: f64| -> Result<f64, ErrorKind> {
                    match argument(index) {
                        None => Ok(default),
                        Some(Expr::Text(text)) if text.is_empty() => Ok(default),
                        Some(expression) => self.scalar(sheet, expression).as_number(),
                    }
                };
                let (rows, columns) = (number(0, 1.0)?.trunc(), number(1, 1.0)?.trunc());
                let (low, high) = (number(2, 0.0)?, number(3, 1.0)?);
                let whole = self.optional_flag(sheet, argument(4), false)?;
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "the spill limit is far below 2^52"
                )]
                let limit = MAX_SPILL_CELLS as f64;
                if rows < 1.0
                    || columns < 1.0
                    || low > high
                    || (whole && (low.fract() != 0.0 || high.fract() != 0.0))
                {
                    return Err(ErrorKind::Value);
                }
                if rows * columns > limit {
                    return Err(ErrorKind::Spill);
                }
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "checked counts below the spill limit"
                )]
                let (rows, columns) = (rows as usize, columns as usize);
                Ok((0..rows)
                    .map(|_| {
                        (0..columns)
                            .map(|_| {
                                let draw = self.random();
                                Value::Number(if whole {
                                    (low + (draw * (high - low + 1.0)).floor()).min(high)
                                } else {
                                    low + draw * (high - low)
                                })
                            })
                            .collect()
                    })
                    .collect())
            })(),
            "REGEXEXTRACT" => self.regex_extract(sheet, arguments),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests;
