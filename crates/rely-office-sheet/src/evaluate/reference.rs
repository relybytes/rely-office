// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The functions about references rather than values: `OFFSET`, `INDIRECT` and `TRIMRANGE`
//! compute the range they read, so the cells they depend on are only known while computing,
//! and `AREAS`, `SHEET`, `SHEETS`, `FORMULATEXT`, `ISFORMULA` and `CELL` describe a reference.
//! With them, `XMATCH`, `AGGREGATE`, the random numbers, `SERIESSUM`, `PERCENTOF` and
//! `MDETERM`.

#![allow(
    clippy::float_cmp,
    reason = "codes, modes and whole numbers are compared exactly, as Excel compares them"
)]

use std::cmp::Ordering;

use super::more::{mean, percentile_exclusive, percentile_inclusive, variance};
use super::{Evaluator, Operand, State, compare, wildcard};
use crate::address::{CellRef, MAX_COLUMNS, MAX_ROWS, column_name};
use crate::formula::{Expr, parse_formula, print_formula};
use crate::model::NumberFormat;
use crate::value::{ErrorKind, Input, Value};

type Answer = Option<Result<Value, ErrorKind>>;

/// The functions whose first argument is a reference they describe, never a value they read:
/// a formula naming its own cell there is no circular reference.
pub(super) const REFERENCE_ARGUMENT_FUNCTIONS: &[&str] = &[
    "AREAS",
    "COLUMN",
    "COLUMNS",
    "FORMULATEXT",
    "ISFORMULA",
    "OFFSET",
    "ROW",
    "ROWS",
    "SHEET",
    "SHEETS",
];

/// A whole number of rows or columns from a cell argument, within the grid.
fn offset_of(number: f64) -> Result<i64, ErrorKind> {
    let number = number.trunc();
    if number.abs() > f64::from(MAX_ROWS) {
        return Err(ErrorKind::Ref);
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "checked to lie within the grid"
    )]
    let number = number as i64;
    Ok(number)
}

fn moved(position: u32, by: i64, limit: u32) -> Result<u32, ErrorKind> {
    u32::try_from(i64::from(position) + by)
        .ok()
        .filter(|moved| *moved < limit)
        .ok_or(ErrorKind::Ref)
}

/// A reference in R1C1 notation, `R2C3`, `R[-1]C`, `R1C1:R2C2`, the relative parts counted
/// from `current`.
fn r1c1_cells(text: &str, current: CellRef) -> Option<(CellRef, CellRef)> {
    let part = |text: &str| -> Option<CellRef> {
        let upper = text.to_ascii_uppercase();
        let rest = upper.strip_prefix('R')?;
        let (row, rest) = r1c1_number(rest, current.row)?;
        let rest = rest.strip_prefix('C')?;
        let (column, rest) = r1c1_number(rest, current.column)?;
        rest.is_empty().then(|| CellRef::new(row, column))
    };
    match text.split_once(':') {
        Some((first, last)) => Some((part(first)?, part(last)?)),
        None => part(text).map(|cell| (cell, cell)),
    }
}

/// The row or column of one R1C1 part and what follows it: `5` absolute, `[-1]` relative,
/// nothing for the current one.
fn r1c1_number(text: &str, current: u32) -> Option<(u32, &str)> {
    if let Some(rest) = text.strip_prefix('[') {
        let end = rest.find(']')?;
        let offset: i64 = rest[..end].parse().ok()?;
        let value = u32::try_from(i64::from(current) + offset).ok()?;
        return Some((value, &rest[end + 1..]));
    }
    let digits = text.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return Some((current, text));
    }
    let value: u32 = text[..digits].parse().ok()?;
    Some((value.checked_sub(1)?, &text[digits..]))
}

/// The determinant of a square matrix, by Gaussian elimination with partial pivoting.
pub(super) fn determinant(mut matrix: Vec<Vec<f64>>) -> f64 {
    let size = matrix.len();
    let mut result = 1.0;
    for column in 0..size {
        let pivot = (column..size)
            .max_by(|first, second| {
                matrix[*first][column]
                    .abs()
                    .partial_cmp(&matrix[*second][column].abs())
                    .unwrap_or(Ordering::Equal)
            })
            .unwrap_or(column);
        if matrix[pivot][column] == 0.0 {
            return 0.0;
        }
        if pivot != column {
            matrix.swap(pivot, column);
            result = -result;
        }
        result *= matrix[column][column];
        let (upper, lower) = matrix.split_at_mut(column + 1);
        let pivot_row = &upper[column];
        for row in lower {
            let factor = row[column] / pivot_row[column];
            for (value, pivot) in row[column..].iter_mut().zip(&pivot_row[column..]) {
                *value -= factor * pivot;
            }
        }
    }
    result
}

/// The next number of the random sequence a recalculation draws, uniform in [0, 1).
fn uniform(seed: u64) -> f64 {
    let mut z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^= z >> 31;
    #[expect(
        clippy::cast_precision_loss,
        reason = "53 random bits fit a double exactly"
    )]
    let fraction = (z >> 11) as f64 / (1_u64 << 53) as f64;
    fraction
}

impl Evaluator<'_> {
    /// A range a function computed: the formula cells inside it not yet computed are noted, so
    /// the formula is computed again after them.
    pub(super) fn dynamic_range(&self, sheet: usize, start: CellRef, end: CellRef) -> Operand {
        if let Some(cells) = self.formula_cells.get(sheet) {
            let mut discovered = self.discovered.borrow_mut();
            for cell in cells.range(start..=CellRef::new(end.row, end.column)) {
                if (start.column..=end.column).contains(&cell.column)
                    && self.states[sheet].get(cell) != Some(&State::Done)
                {
                    discovered.push((sheet, *cell));
                }
            }
            for (anchor, spill) in &self.prior_spills[sheet] {
                let overlaps = spill.first_row <= end.row
                    && spill.last_row >= start.row
                    && spill.first_column <= end.column
                    && spill.last_column >= start.column;
                if overlaps && self.states[sheet].get(anchor) != Some(&State::Done) {
                    discovered.push((sheet, *anchor));
                }
            }
        }
        Operand::Range { sheet, start, end }
    }

    /// A random number of this recalculation, different for each cell and each draw.
    pub(super) fn random(&self) -> f64 {
        let draw = self.draws.get();
        self.draws.set(draw + 1);
        let cell = self.current.get().map_or(0, |cell| {
            (u64::from(cell.row) << 16) ^ u64::from(cell.column)
        });
        uniform(
            self.workbook.revision.wrapping_mul(0x2545_f491_4f6c_dd1d)
                ^ cell.rotate_left(24)
                ^ draw.wrapping_mul(0x9e37_79b9),
        )
    }

    /// A function that answers a reference, or `None` when the name is none of them.
    pub(super) fn reference_call(
        &self,
        sheet: usize,
        name: &str,
        arguments: &[Expr],
    ) -> Option<Operand> {
        let result = match name {
            "OFFSET" => self.offset(sheet, arguments),
            "INDIRECT" => self.indirect(sheet, arguments),
            "TRIMRANGE" => self.trim_range(sheet, arguments),
            _ => return None,
        };
        Some(result.unwrap_or_else(|kind| Operand::Value(Value::Error(kind))))
    }

    fn reference_of(
        &self,
        sheet: usize,
        argument: Option<&Expr>,
    ) -> Result<(usize, CellRef, CellRef), ErrorKind> {
        match argument.map(|argument| self.evaluate(sheet, argument)) {
            Some(Operand::Range { sheet, start, end }) => Ok((sheet, start, end)),
            Some(Operand::Value(Value::Error(kind))) => Err(kind),
            _ => match argument {
                Some(Expr::Cell(address)) => {
                    let target = self
                        .sheet_of(sheet, address.sheet.as_deref())
                        .ok_or(ErrorKind::Ref)?;
                    Ok((target, address.cell, address.cell))
                }
                _ => Err(ErrorKind::Value),
            },
        }
    }

    fn offset(&self, sheet: usize, arguments: &[Expr]) -> Result<Operand, ErrorKind> {
        let (target, start, end) = self.reference_of(sheet, arguments.first())?;
        let number = |index: usize| self.number(sheet, arguments.get(index));
        let rows = offset_of(number(1)?)?;
        let columns = offset_of(number(2)?)?;
        let size = |index: usize, default: u32| -> Result<i64, ErrorKind> {
            match arguments.get(index) {
                None => Ok(i64::from(default)),
                Some(Expr::Text(text)) if text.is_empty() => Ok(i64::from(default)),
                Some(_) => offset_of(number(index)?),
            }
        };
        let height = size(3, end.row - start.row + 1)?;
        let width = size(4, end.column - start.column + 1)?;
        if height == 0 || width == 0 {
            return Err(ErrorKind::Ref);
        }
        let first_row = moved(start.row, rows, MAX_ROWS)?;
        let first_column = moved(start.column, columns, MAX_COLUMNS)?;
        // A negative size reaches up or to the left of the moved corner.
        let last_row = moved(first_row, height - height.signum(), MAX_ROWS)?;
        let last_column = moved(first_column, width - width.signum(), MAX_COLUMNS)?;
        let from = CellRef::new(first_row.min(last_row), first_column.min(last_column));
        let to = CellRef::new(first_row.max(last_row), first_column.max(last_column));
        Ok(self.dynamic_range(target, from, to))
    }

    fn indirect(&self, sheet: usize, arguments: &[Expr]) -> Result<Operand, ErrorKind> {
        let text = self.text(sheet, arguments.first())?;
        let a1 = match arguments.get(1) {
            None => true,
            Some(Expr::Text(text)) if text.is_empty() => true,
            Some(expression) => self.scalar(sheet, expression).as_bool()?,
        };
        let text = text.trim();
        if text.is_empty() {
            return Err(ErrorKind::Ref);
        }
        let (target, start, end) = if a1 {
            let expression = parse_formula(text).map_err(|_| ErrorKind::Ref)?;
            let operand = match &expression {
                Expr::Cell(_) | Expr::Range(_) | Expr::Name(_) => self.evaluate(sheet, &expression),
                _ => return Err(ErrorKind::Ref),
            };
            match operand {
                Operand::Range { sheet, start, end } => (sheet, start, end),
                _ => match &expression {
                    Expr::Cell(address) => (
                        self.sheet_of(sheet, address.sheet.as_deref())
                            .ok_or(ErrorKind::Ref)?,
                        address.cell,
                        address.cell,
                    ),
                    _ => return Err(ErrorKind::Ref),
                },
            }
        } else {
            let (sheet_name, reference) = match text.rsplit_once('!') {
                Some((name, reference)) => (Some(name.trim_matches('\'')), reference),
                None => (None, text),
            };
            let target = self.sheet_of(sheet, sheet_name).ok_or(ErrorKind::Ref)?;
            let current = self.current.get().unwrap_or(CellRef::new(0, 0));
            let (start, end) = r1c1_cells(reference, current).ok_or(ErrorKind::Ref)?;
            if start.row.max(end.row) >= MAX_ROWS || start.column.max(end.column) >= MAX_COLUMNS {
                return Err(ErrorKind::Ref);
            }
            (
                target,
                CellRef::new(start.row.min(end.row), start.column.min(end.column)),
                CellRef::new(start.row.max(end.row), start.column.max(end.column)),
            )
        };
        Ok(self.dynamic_range(target, start, end))
    }

    fn trim_range(&self, sheet: usize, arguments: &[Expr]) -> Result<Operand, ErrorKind> {
        let (target, start, end) = self.reference_of(sheet, arguments.first())?;
        let mode = |index: usize| -> Result<f64, ErrorKind> {
            match arguments.get(index) {
                None => Ok(3.0),
                Some(Expr::Text(text)) if text.is_empty() => Ok(3.0),
                Some(expression) => {
                    let mode = self.scalar(sheet, expression).as_number()?.trunc();
                    if (0.0..=3.0).contains(&mode) {
                        Ok(mode)
                    } else {
                        Err(ErrorKind::Value)
                    }
                }
            }
        };
        let (rows, columns) = (mode(1)?, mode(2)?);
        let end_row = end.row.min(self.last_row(target).max(start.row));
        let filled = |row: u32, column: u32| {
            !matches!(
                self.cell_value(target, CellRef::new(row, column)),
                Value::Empty
            )
        };
        let mut used = None::<(u32, u32, u32, u32)>;
        for row in start.row..=end_row {
            for column in start.column..=end.column {
                if filled(row, column) {
                    used = Some(match used {
                        None => (row, column, row, column),
                        Some((top, left, bottom, right)) => (
                            top.min(row),
                            left.min(column),
                            bottom.max(row),
                            right.max(column),
                        ),
                    });
                }
            }
        }
        let Some((top, left, bottom, right)) = used else {
            return Err(ErrorKind::Ref);
        };
        let first_row = if rows == 1.0 || rows == 3.0 {
            top
        } else {
            start.row
        };
        let last_row = if rows == 2.0 || rows == 3.0 {
            bottom
        } else {
            end.row
        };
        let first_column = if columns == 1.0 || columns == 3.0 {
            left
        } else {
            start.column
        };
        let last_column = if columns == 2.0 || columns == 3.0 {
            right
        } else {
            end.column
        };
        Ok(self.dynamic_range(
            target,
            CellRef::new(first_row, first_column),
            CellRef::new(last_row, last_column),
        ))
    }

    /// The cell a reference argument names, its own first cell for a range; the formula's own
    /// cell without one.
    fn referenced_cell(
        &self,
        sheet: usize,
        argument: Option<&Expr>,
    ) -> Result<(usize, CellRef), ErrorKind> {
        match argument {
            None => Ok((sheet, self.current.get().unwrap_or(CellRef::new(0, 0)))),
            Some(Expr::Cell(address)) => Ok((
                self.sheet_of(sheet, address.sheet.as_deref())
                    .ok_or(ErrorKind::Ref)?,
                address.cell,
            )),
            Some(Expr::Range(range)) => Ok((
                self.sheet_of(sheet, range.sheet.as_deref())
                    .ok_or(ErrorKind::Ref)?,
                range.bounds().0,
            )),
            Some(expression) => match self.evaluate(sheet, expression) {
                Operand::Range { sheet, start, .. } => Ok((sheet, start)),
                Operand::Value(Value::Error(kind)) => Err(kind),
                _ => Err(ErrorKind::Value),
            },
        }
    }

    /// A function about references, a random number or one of the rest of this set, or `None`
    /// when the name is none of them.
    #[expect(clippy::too_many_lines, reason = "one arm per function, each short")]
    pub(super) fn reference_value(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        let argument = |index: usize| arguments.get(index);
        let number = |index: usize| self.number(sheet, argument(index));
        let count = |count: usize| {
            #[expect(clippy::cast_precision_loss, reason = "counts are small")]
            let count = count as f64;
            Value::Number(count)
        };
        Some(match name {
            "AREAS" => (|| {
                self.referenced_cell(sheet, Some(argument(0).ok_or(ErrorKind::Value)?))?;
                Ok(Value::Number(1.0))
            })(),
            "SHEET" => (|| match argument(0) {
                None => Ok(count(sheet + 1)),
                Some(Expr::Cell(_) | Expr::Range(_)) => {
                    Ok(count(self.referenced_cell(sheet, argument(0))?.0 + 1))
                }
                Some(expression) => match self.scalar(sheet, expression) {
                    Value::Text(text) => self
                        .workbook
                        .sheet_index(&text)
                        .map(|index| count(index + 1))
                        .ok_or(ErrorKind::NotAvailable),
                    Value::Error(kind) => Err(kind),
                    _ => Err(ErrorKind::Value),
                },
            })(),
            "SHEETS" => (|| {
                if argument(0).is_none() {
                    return Ok(count(self.workbook.sheets.len()));
                }
                self.referenced_cell(sheet, argument(0))?;
                Ok(Value::Number(1.0))
            })(),
            "FORMULATEXT" | "ISFORMULA" => (|| {
                let (target, cell) =
                    self.referenced_cell(sheet, Some(argument(0).ok_or(ErrorKind::Value)?))?;
                let formula = self.workbook.sheets[target]
                    .cells
                    .get(&cell)
                    .and_then(|content| match &content.input {
                        Input::Formula(expression) => Some(expression),
                        _ => None,
                    });
                if name == "ISFORMULA" {
                    return Ok(Value::Bool(formula.is_some()));
                }
                formula
                    .map(|expression| Value::Text(format!("={}", print_formula(expression))))
                    .ok_or(ErrorKind::NotAvailable)
            })(),
            "CELL" => (|| {
                let info = self.text(sheet, argument(0))?.to_lowercase();
                let (target, cell) = self.referenced_cell(sheet, argument(1))?;
                let content = self.workbook.sheets[target].cells.get(&cell);
                let value = self.cell_value(target, cell);
                Ok(match info.as_str() {
                    "address" => {
                        let address = format!("${}${}", column_name(cell.column), cell.row + 1);
                        Value::Text(if target == sheet {
                            address
                        } else {
                            let name = &self.workbook.sheets[target].name;
                            format!("'{}'!{address}", name.replace('\'', "''"))
                        })
                    }
                    "col" => Value::Number(f64::from(cell.column + 1)),
                    "row" => Value::Number(f64::from(cell.row + 1)),
                    "contents" => value,
                    "type" => Value::Text(
                        match value {
                            Value::Empty => "b",
                            Value::Text(_) => "l",
                            _ => "v",
                        }
                        .to_owned(),
                    ),
                    "width" => Value::Number(
                        self.workbook.sheets[target]
                            .column_widths
                            .get(&cell.column)
                            .copied()
                            .unwrap_or(8.43)
                            .trunc(),
                    ),
                    "format" => Value::Text(match content.map(|content| content.format) {
                        None | Some(NumberFormat::General) => "G".to_owned(),
                        Some(NumberFormat::Decimals(decimals)) => format!("F{decimals}"),
                        Some(NumberFormat::Percent) => "P0".to_owned(),
                        Some(NumberFormat::Date) => "D1".to_owned(),
                        Some(NumberFormat::DateTime) => "D4".to_owned(),
                    }),
                    "filename" | "prefix" => Value::Text(String::new()),
                    "protect" => Value::Number(1.0),
                    "color" | "parentheses" => Value::Number(0.0),
                    _ => return Err(ErrorKind::Value),
                })
            })(),
            "INFO" => (|| {
                if arguments.len() != 1 {
                    return Err(ErrorKind::Value);
                }
                let key = self.text(sheet, argument(0))?.trim().to_lowercase();
                Ok(match key.as_str() {
                    "numfile" => count(self.workbook.sheets.len()),
                    "recalc" => Value::Text("Automatic".to_owned()),
                    "release" => Value::Text("16.0".to_owned()),
                    "system" => Value::Text("pcdos".to_owned()),
                    "osversion" => Value::Text("Windows (64-bit) NT 10.00".to_owned()),
                    // The others name the host's folders and memory, which a file must not read.
                    _ => return Err(ErrorKind::NotAvailable),
                })
            })(),
            "PHONETIC" => (|| {
                if arguments.len() != 1 {
                    return Err(ErrorKind::Value);
                }
                let (target, cell) = self.referenced_cell(sheet, argument(0))?;
                // No reading guide is stored with a cell, so the guide is the text itself.
                Ok(Value::Text(match self.cell_value(target, cell) {
                    Value::Text(text) => text,
                    Value::Error(kind) => return Err(kind),
                    _ => String::new(),
                }))
            })(),
            "XMATCH" => self.xmatch(sheet, arguments),
            "AGGREGATE" => self.aggregate(sheet, arguments),
            "RAND" => Ok(Value::Number(self.random())),
            "RANDBETWEEN" => (|| {
                let (bottom, top) = (number(0)?.ceil(), number(1)?.floor());
                if bottom > top {
                    return Err(ErrorKind::Num);
                }
                Ok(Value::Number(
                    (bottom + (self.random() * (top - bottom + 1.0)).floor()).min(top),
                ))
            })(),
            "SERIESSUM" => (|| {
                let (x, power, step) = (number(0)?, number(1)?, number(2)?);
                let coefficients = self.grid(sheet, argument(3))?;
                let mut total = 0.0;
                let mut exponent = power;
                for value in coefficients.into_iter().flatten() {
                    let coefficient = match value {
                        Value::Number(number) => number,
                        Value::Error(kind) => return Err(kind),
                        _ => return Err(ErrorKind::Value),
                    };
                    total += coefficient * x.powf(exponent);
                    exponent += step;
                }
                if total.is_finite() {
                    Ok(Value::Number(total))
                } else {
                    Err(ErrorKind::Num)
                }
            })(),
            "PERCENTOF" => (|| {
                let part: f64 = self
                    .numbers(sheet, &arguments[..1.min(arguments.len())])?
                    .iter()
                    .sum();
                let whole: f64 = self
                    .numbers(sheet, arguments.get(1..2).unwrap_or(&[]))?
                    .iter()
                    .sum();
                if whole == 0.0 {
                    return Err(ErrorKind::Div0);
                }
                Ok(Value::Number(part / whole))
            })(),
            "MDETERM" => (|| {
                let matrix = self.numeric_matrix(sheet, argument(0))?;
                if matrix.is_empty() || matrix.iter().any(|row| row.len() != matrix.len()) {
                    return Err(ErrorKind::Value);
                }
                Ok(Value::Number(determinant(matrix)))
            })(),
            _ => return None,
        })
    }

    /// A grid of numbers; any other value is `#VALUE!`.
    pub(super) fn numeric_matrix(
        &self,
        sheet: usize,
        argument: Option<&Expr>,
    ) -> Result<Vec<Vec<f64>>, ErrorKind> {
        self.grid(sheet, argument)?
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|value| match value {
                        Value::Number(number) => Ok(number),
                        Value::Error(kind) => Err(kind),
                        _ => Err(ErrorKind::Value),
                    })
                    .collect()
            })
            .collect()
    }

    fn xmatch(&self, sheet: usize, arguments: &[Expr]) -> Result<Value, ErrorKind> {
        let lookup = match arguments.first() {
            Some(expression) => self.scalar(sheet, expression),
            None => return Err(ErrorKind::Value),
        };
        if let Value::Error(kind) = lookup {
            return Err(kind);
        }
        let grid = self.grid(sheet, arguments.get(1))?;
        let keys: Vec<Value> = if grid.len() == 1 || grid.iter().all(|row| row.len() == 1) {
            grid.into_iter().flatten().collect()
        } else {
            return Err(ErrorKind::Value);
        };
        let mode = |index: usize, default: f64| -> Result<f64, ErrorKind> {
            match arguments.get(index) {
                None => Ok(default),
                Some(Expr::Text(text)) if text.is_empty() => Ok(default),
                Some(expression) => Ok(self.scalar(sheet, expression).as_number()?.trunc()),
            }
        };
        let (match_mode, search_mode) = (mode(2, 0.0)?, mode(3, 1.0)?);
        if ![0.0, -1.0, 1.0, 2.0].contains(&match_mode)
            || ![1.0, -1.0, 2.0, -2.0].contains(&search_mode)
        {
            return Err(ErrorKind::Value);
        }
        let order: Vec<usize> = if search_mode == -1.0 {
            (0..keys.len()).rev().collect()
        } else {
            (0..keys.len()).collect()
        };
        let pattern: Option<Vec<char>> = (match_mode == 2.0).then(|| {
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
                if let Value::Text(text) = key
                    && wildcard(pattern, &text.to_lowercase().chars().collect::<Vec<_>>())
                {
                    return Ok(Value::Number(f64::from(
                        u32::try_from(index + 1).unwrap_or(u32::MAX),
                    )));
                }
                continue;
            }
            let comparable = matches!(
                (key, &lookup),
                (Value::Number(_), Value::Number(_))
                    | (Value::Text(_), Value::Text(_))
                    | (Value::Bool(_), Value::Bool(_))
            );
            if !comparable {
                continue;
            }
            let ordering = compare(key, &lookup);
            if ordering == Ordering::Equal {
                return Ok(Value::Number(f64::from(
                    u32::try_from(index + 1).unwrap_or(u32::MAX),
                )));
            }
            let better = |candidate: &Value| match best {
                None => true,
                Some(current) => {
                    let against = compare(candidate, &keys[current]);
                    if match_mode == -1.0 {
                        against == Ordering::Greater
                    } else {
                        against == Ordering::Less
                    }
                }
            };
            if (match_mode == -1.0 && ordering == Ordering::Less
                || match_mode == 1.0 && ordering == Ordering::Greater)
                && better(key)
            {
                best = Some(index);
            }
        }
        best.map(|index| Value::Number(f64::from(u32::try_from(index + 1).unwrap_or(u32::MAX))))
            .ok_or(ErrorKind::NotAvailable)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one arm per function AGGREGATE stands for"
    )]
    fn aggregate(&self, sheet: usize, arguments: &[Expr]) -> Result<Value, ErrorKind> {
        let function = self.number(sheet, arguments.first())?.trunc();
        let options = match arguments.get(1) {
            None => 0.0,
            Some(Expr::Text(text)) if text.is_empty() => 0.0,
            Some(expression) => self.scalar(sheet, expression).as_number()?.trunc(),
        };
        if !(1.0..=19.0).contains(&function) || !(0.0..=7.0).contains(&options) {
            return Err(ErrorKind::Value);
        }
        let skip_hidden = [1.0, 3.0, 5.0, 7.0].contains(&options);
        let skip_errors = [2.0, 3.0, 6.0, 7.0].contains(&options);
        let skip_nested = options <= 3.0;
        let ranged = function >= 14.0;
        let references = if ranged {
            arguments.get(2..3).unwrap_or(&[])
        } else {
            arguments.get(2..).unwrap_or(&[])
        };
        if references.is_empty() {
            return Err(ErrorKind::Value);
        }
        let mut values = Vec::new();
        for reference in references {
            match self.evaluate(sheet, reference) {
                Operand::Range {
                    sheet: target,
                    start,
                    end,
                } => {
                    let end_row = end.row.min(self.last_row(target).max(start.row));
                    for row in start.row..=end_row {
                        if skip_hidden
                            && self.workbook.sheets[target].row_heights.get(&row) == Some(&0.0)
                        {
                            continue;
                        }
                        for column in start.column..=end.column {
                            let cell = CellRef::new(row, column);
                            let nested = skip_nested
                                && self.workbook.sheets[target].cells.get(&cell).is_some_and(|content| {
                                    matches!(&content.input, Input::Formula(expression)
                                        if matches!(&**expression, Expr::Call(called, _) if called == "SUBTOTAL" || called == "AGGREGATE"))
                                });
                            if !nested {
                                values.push(self.cell_value(target, cell));
                            }
                        }
                    }
                }
                Operand::Value(value) => values.push(value),
                Operand::Array(grid) => values.extend(grid.into_iter().flatten()),
                Operand::Lambda(_) => return Err(ErrorKind::Calc),
            }
        }
        let mut numbers = Vec::new();
        let mut counted = 0;
        for value in values {
            match value {
                Value::Error(kind) if !skip_errors => return Err(kind),
                Value::Error(_) | Value::Empty => {}
                Value::Number(number) => {
                    numbers.push(number);
                    counted += 1;
                }
                _ => counted += 1,
            }
        }
        let k = if ranged {
            Some(self.number(sheet, arguments.get(3))?)
        } else {
            None
        };
        let mut sorted = numbers.clone();
        sorted.sort_by(f64::total_cmp);
        #[expect(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "counts are small and the function number was checked to be 1 to 19"
        )]
        let result = match function as u8 {
            1 => {
                if numbers.is_empty() {
                    return Err(ErrorKind::Div0);
                }
                mean(&numbers)
            }
            2 => numbers.len() as f64,
            3 => f64::from(u32::try_from(counted).unwrap_or(u32::MAX)),
            4 => numbers.iter().copied().reduce(f64::max).unwrap_or(0.0),
            5 => numbers.iter().copied().reduce(f64::min).unwrap_or(0.0),
            6 => numbers.iter().product(),
            7 => variance(&numbers, true)?.sqrt(),
            8 => variance(&numbers, false)?.sqrt(),
            9 => numbers.iter().sum(),
            10 => variance(&numbers, true)?,
            11 => variance(&numbers, false)?,
            12 => {
                if sorted.is_empty() {
                    return Err(ErrorKind::Num);
                }
                percentile_inclusive(&sorted, 0.5)?
            }
            13 => {
                let mut best = None::<(f64, usize)>;
                for value in &numbers {
                    let times = numbers.iter().filter(|other| *other == value).count();
                    if times > 1 && best.is_none_or(|(_, most)| times > most) {
                        best = Some((*value, times));
                    }
                }
                best.ok_or(ErrorKind::NotAvailable)?.0
            }
            14 | 15 => {
                let k = k.unwrap_or(0.0).ceil();
                if k < 1.0 || k > sorted.len() as f64 {
                    return Err(ErrorKind::Num);
                }
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "checked to lie within the list"
                )]
                let index = k as usize - 1;
                if function == 14.0 {
                    sorted[sorted.len() - 1 - index]
                } else {
                    sorted[index]
                }
            }
            16 | 18 => {
                let fraction = k.unwrap_or(0.0);
                if sorted.is_empty() {
                    return Err(ErrorKind::Num);
                }
                if function == 16.0 {
                    percentile_inclusive(&sorted, fraction)?
                } else {
                    percentile_exclusive(&sorted, fraction)?
                }
            }
            _ => {
                let quarter = k.unwrap_or(0.0).trunc();
                if sorted.is_empty() || !(0.0..=4.0).contains(&quarter) {
                    return Err(ErrorKind::Num);
                }
                if function == 17.0 {
                    percentile_inclusive(&sorted, quarter / 4.0)?
                } else {
                    percentile_exclusive(&sorted, quarter / 4.0)?
                }
            }
        };
        if result.is_finite() {
            Ok(Value::Number(result))
        } else {
            Err(ErrorKind::Num)
        }
    }
}

#[cfg(test)]
mod tests;
