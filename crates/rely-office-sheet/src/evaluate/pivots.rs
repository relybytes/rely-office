// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The functions that summarise like a pivot table: `GETPIVOTDATA` reads a value of a pivot
//! table of the workbook, `GROUPBY` and `PIVOTBY` summarise arrays by their rows and columns.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use super::lambda::Closure;
use super::{Evaluator, MAX_SPILL_CELLS, Operand};
use crate::formula::{Expr, FUNCTION_NAMES, canonical_function_name};
use crate::pivots::{Key, item_label};
use crate::value::{ErrorKind, Value};

type Grid = Vec<Vec<Value>>;

/// What summarises a group: a function of the grid by its name, `SUM`, or a `LAMBDA`.
enum Summary {
    Named(String),
    Lambda(Rc<Closure>),
}

/// The caption of the rows that total the groups.
const TOTAL: &str = "Total";

impl Evaluator<'_> {
    /// The pivot table functions; `None` for any other.
    pub(super) fn pivot_call(
        &self,
        sheet: usize,
        name: &str,
        arguments: &[Expr],
    ) -> Option<Operand> {
        let result = match name {
            "GETPIVOTDATA" => self.get_pivot_data(sheet, arguments).map(Operand::Value),
            "GROUPBY" => self.group_by(sheet, arguments).map(Operand::Array),
            "PIVOTBY" => self.pivot_by(sheet, arguments).map(Operand::Array),
            _ => return None,
        };
        Some(result.unwrap_or_else(|kind| Operand::Value(Value::Error(kind))))
    }

    /// A value of a pivot table: the data field named, over the records whose fields hold the
    /// items named, as the table was last computed.
    fn get_pivot_data(&self, sheet: usize, arguments: &[Expr]) -> Result<Value, ErrorKind> {
        if arguments.len() < 2 || !arguments.len().is_multiple_of(2) {
            return Err(ErrorKind::Value);
        }
        let field = self.scalar(sheet, &arguments[0]).as_text()?;
        let (target, cell) = match &arguments[1] {
            Expr::Cell(address) => (
                self.sheet_of(sheet, address.sheet.as_deref())
                    .ok_or(ErrorKind::Ref)?,
                address.cell,
            ),
            other => match self.evaluate(sheet, other) {
                Operand::Range { sheet, start, .. } => (sheet, start),
                _ => return Err(ErrorKind::Ref),
            },
        };
        let table = self.workbook.sheets[target]
            .pivot_tables
            .iter()
            .find(|table| {
                table.area.is_some_and(|area| {
                    (area.first_row..=area.last_row).contains(&cell.row)
                        && (area.first_column..=area.last_column).contains(&cell.column)
                })
            })
            .ok_or(ErrorKind::Ref)?;
        let (header, records) = table.snapshot.split_first().ok_or(ErrorKind::Ref)?;
        let named = |wanted: &str| {
            header
                .iter()
                .position(|name| item_label(name).eq_ignore_ascii_case(wanted.trim()))
        };
        let value = table
            .values
            .iter()
            .find(|value| {
                table
                    .value_caption(value, header)
                    .eq_ignore_ascii_case(field.trim())
                    || named(&field) == Some(value.field as usize)
            })
            .ok_or(ErrorKind::Ref)?;
        let mut conditions = Vec::new();
        for pair in arguments[2..].chunks(2) {
            let name = self.scalar(sheet, &pair[0]).as_text()?;
            let index = named(&name).ok_or(ErrorKind::Ref)?;
            if !table
                .rows
                .iter()
                .chain(&table.columns)
                .any(|placed| placed.field as usize == index)
            {
                return Err(ErrorKind::Ref);
            }
            let item = self.scalar(sheet, &pair[1]);
            conditions.push((index, item));
        }
        let matching: Vec<Value> = table
            .kept_records(records)
            .into_iter()
            .filter(|record| {
                conditions.iter().all(|(index, item)| {
                    let held = record.get(*index).unwrap_or(&Value::Empty);
                    Key(held.clone()) == Key(item.clone())
                        || item_label(held).eq_ignore_ascii_case(&item_label(item))
                })
            })
            .map(|record| {
                record
                    .get(value.field as usize)
                    .cloned()
                    .unwrap_or(Value::Empty)
            })
            .collect();
        if matching.is_empty() {
            return Err(ErrorKind::Ref);
        }
        Ok(value.function.summarise(&matching))
    }

    /// The function a `GROUPBY` or `PIVOTBY` summarises with.
    fn summary(&self, sheet: usize, argument: Option<&Expr>) -> Result<Summary, ErrorKind> {
        let argument = argument.ok_or(ErrorKind::Value)?;
        if let Expr::Name(name) = argument {
            let english = canonical_function_name(name);
            if FUNCTION_NAMES.binary_search(&english.as_str()).is_ok() {
                return Ok(Summary::Named(english));
            }
        }
        match self.evaluate(sheet, argument) {
            Operand::Lambda(closure) => Ok(Summary::Lambda(closure)),
            Operand::Value(Value::Error(kind)) => Err(kind),
            _ => Err(ErrorKind::Value),
        }
    }

    /// A group's values summarised into one value.
    fn summarise(&self, sheet: usize, summary: &Summary, values: &[Value]) -> Value {
        let operand = match summary {
            Summary::Named(name) => {
                let mut column: Vec<Vec<Expr>> = values
                    .iter()
                    .filter_map(|value| match value {
                        Value::Number(number) => Some(Expr::Number(*number)),
                        Value::Text(text) => Some(Expr::Text(text.clone())),
                        Value::Bool(flag) => Some(Expr::Bool(*flag)),
                        Value::Error(kind) => Some(Expr::Error(*kind)),
                        Value::Empty => None,
                    })
                    .map(|cell| vec![cell])
                    .collect();
                if column.is_empty() {
                    column.push(vec![Expr::Text(String::new())]);
                }
                self.evaluate(sheet, &Expr::Call(name.clone(), vec![Expr::Array(column)]))
            }
            Summary::Lambda(closure) => {
                let column: Grid = values.iter().map(|value| vec![value.clone()]).collect();
                self.invoke(sheet, closure, vec![Some(Operand::Array(column))])
            }
        };
        match operand {
            Operand::Value(value) => value,
            Operand::Array(grid) if grid.len() == 1 && grid[0].len() == 1 => grid[0][0].clone(),
            _ => Value::Error(ErrorKind::Calc),
        }
    }

    /// An optional whole number argument, `default` when left out or empty.
    fn whole(&self, sheet: usize, argument: Option<&Expr>, default: i64) -> Result<i64, ErrorKind> {
        match argument {
            None => Ok(default),
            Some(Expr::Text(text)) if text.is_empty() => Ok(default),
            Some(argument) => {
                let value = match self.evaluate(sheet, argument) {
                    Operand::Array(grid) => {
                        grid.into_iter().flatten().next().unwrap_or(Value::Empty)
                    }
                    _ => self.scalar(sheet, argument),
                };
                let number = value.as_number()?;
                if !number.is_finite() {
                    return Err(ErrorKind::Value);
                }
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "the number is checked finite"
                )]
                Ok(number.trunc() as i64)
            }
        }
    }

    /// The rows a filter argument keeps; all when it is left out.
    fn kept_rows(
        &self,
        sheet: usize,
        argument: Option<&Expr>,
        rows: usize,
    ) -> Result<Vec<bool>, ErrorKind> {
        match argument {
            None => Ok(vec![true; rows]),
            Some(Expr::Text(text)) if text.is_empty() => Ok(vec![true; rows]),
            Some(argument) => {
                let grid = self.grid(sheet, Some(argument))?;
                if grid.len() != rows {
                    return Err(ErrorKind::Value);
                }
                grid.iter()
                    .map(|row| match row.first() {
                        Some(Value::Bool(flag)) => Ok(*flag),
                        Some(Value::Number(number)) => Ok(*number != 0.0),
                        Some(Value::Error(kind)) => Err(*kind),
                        _ => Ok(false),
                    })
                    .collect()
            }
        }
    }

    /// `GROUPBY(row_fields, values, function, [field_headers], [total_depth], [sort_order],
    /// [filter_array])`: a row per distinct key of the row fields, each value column summarised.
    fn group_by(&self, sheet: usize, arguments: &[Expr]) -> Result<Grid, ErrorKind> {
        if !(3..=8).contains(&arguments.len()) {
            return Err(ErrorKind::Value);
        }
        let keys = self.grid(sheet, arguments.first())?;
        let values = self.grid(sheet, arguments.get(1))?;
        if keys.len() != values.len() || keys.is_empty() {
            return Err(ErrorKind::Value);
        }
        let summary = self.summary(sheet, arguments.get(2))?;
        let headers = self.whole(sheet, arguments.get(3), -1)?;
        let depth = self.whole(sheet, arguments.get(4), 1)?;
        let sort = self.whole(sheet, arguments.get(5), 1)?;
        let kept = self.kept_rows(sheet, arguments.get(6), keys.len())?;
        let (has_header, show_header) = header_mode(headers, &values)?;
        let start = usize::from(has_header);
        let key_width = keys[0].len();
        let value_width = values[0].len();
        let mut groups: BTreeMap<Vec<Key>, Vec<usize>> = BTreeMap::new();
        for row in start..keys.len() {
            if kept[row] {
                groups
                    .entry(keys[row].iter().cloned().map(Key).collect())
                    .or_default()
                    .push(row);
            }
        }
        let summarise_rows = |rows: &[usize]| -> Vec<Value> {
            (0..value_width)
                .map(|column| {
                    let cells: Vec<Value> = rows
                        .iter()
                        .map(|row| values[*row].get(column).cloned().unwrap_or(Value::Empty))
                        .collect();
                    self.summarise(sheet, &summary, &cells)
                })
                .collect()
        };
        let mut lines: Vec<(Vec<Key>, Vec<Value>, Vec<usize>)> = groups
            .into_iter()
            .map(|(key, rows)| {
                let summarised = summarise_rows(&rows);
                (key, summarised, rows)
            })
            .collect();
        sort_lines(&mut lines, sort, key_width)?;
        let mut grid: Grid = Vec::new();
        if show_header {
            let mut header: Vec<Value> = if has_header {
                keys[0].clone()
            } else {
                (1..=key_width)
                    .map(|index| Value::Text(format!("Row Field {index}")))
                    .collect()
            };
            if has_header {
                header.extend(values[0].iter().cloned());
            } else {
                header.extend((1..=value_width).map(|index| Value::Text(format!("Value {index}"))));
            }
            grid.push(header);
        }
        let all: Vec<usize> = lines
            .iter()
            .flat_map(|(_, _, rows)| rows.iter().copied())
            .collect();
        let total = |label: &str, rows: &[usize], prefix: Vec<Value>| {
            let mut line = prefix;
            if line.is_empty() {
                line.push(Value::Text(label.to_owned()));
            }
            line.resize(key_width, Value::Empty);
            line.extend(summarise_rows(rows));
            line
        };
        if depth < 0 {
            grid.push(total(TOTAL, &all, Vec::new()));
        }
        let subtotals = depth.abs() >= 2 && key_width >= 2 && sort.unsigned_abs() == 1;
        let mut index = 0;
        while index < lines.len() {
            let first = lines[index].0[0].clone();
            let mut block = Vec::new();
            while index < lines.len() && (!subtotals || lines[index].0[0] == first) {
                let (key, summarised, rows) = &lines[index];
                let mut line: Vec<Value> = key.iter().map(|key| key.0.clone()).collect();
                line.extend(summarised.iter().cloned());
                grid.push(line);
                block.extend(rows.iter().copied());
                index += 1;
            }
            if subtotals {
                grid.push(total(TOTAL, &block, vec![first.0.clone()]));
            }
        }
        if depth > 0 {
            grid.push(total(TOTAL, &all, Vec::new()));
        }
        checked(grid)
    }

    /// `PIVOTBY(row_fields, col_fields, values, function, [field_headers], [row_total_depth],
    /// [row_sort_order], [col_total_depth], [col_sort_order], [filter_array])`: the values
    /// summarised by the keys of the row fields down and of the column fields across.
    #[expect(
        clippy::too_many_lines,
        reason = "the header rows, the rows and the totals are laid out in one pass"
    )]
    fn pivot_by(&self, sheet: usize, arguments: &[Expr]) -> Result<Grid, ErrorKind> {
        if !(4..=11).contains(&arguments.len()) {
            return Err(ErrorKind::Value);
        }
        let row_keys = self.grid(sheet, arguments.first())?;
        let column_keys = self.grid(sheet, arguments.get(1))?;
        let values = self.grid(sheet, arguments.get(2))?;
        let rows = values.len();
        if row_keys.len() != rows || column_keys.len() != rows || rows == 0 {
            return Err(ErrorKind::Value);
        }
        let summary = self.summary(sheet, arguments.get(3))?;
        let headers = self.whole(sheet, arguments.get(4), -1)?;
        let row_depth = self.whole(sheet, arguments.get(5), 1)?;
        let row_sort = self.whole(sheet, arguments.get(6), 1)?;
        let column_depth = self.whole(sheet, arguments.get(7), 1)?;
        let column_sort = self.whole(sheet, arguments.get(8), 1)?;
        let kept = self.kept_rows(sheet, arguments.get(9), rows)?;
        let (has_header, show_header) = header_mode(headers, &values)?;
        let start = usize::from(has_header);
        let row_width = row_keys[0].len();
        let value_width = values[0].len();
        let records: Vec<usize> = (start..rows).filter(|row| kept[*row]).collect();
        let key =
            |grid: &Grid, row: usize| -> Vec<Key> { grid[row].iter().cloned().map(Key).collect() };
        let mut row_groups: Vec<Vec<Key>> = records
            .iter()
            .map(|row| key(&row_keys, *row))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut column_groups: Vec<Vec<Key>> = records
            .iter()
            .map(|row| key(&column_keys, *row))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if row_sort < 0 {
            row_groups.reverse();
        }
        if column_sort < 0 {
            column_groups.reverse();
        }
        if row_groups.len() * (column_groups.len() + 1) * value_width > MAX_SPILL_CELLS {
            return Err(ErrorKind::Num);
        }
        let cell = |rows: &[usize], column: usize| -> Value {
            let cells: Vec<Value> = rows
                .iter()
                .map(|row| values[*row].get(column).cloned().unwrap_or(Value::Empty))
                .collect();
            self.summarise(sheet, &summary, &cells)
        };
        let column_width = column_keys[0].len();
        let mut grid: Grid = Vec::new();
        for level in 0..column_width {
            let mut line = vec![Value::Empty; row_width];
            for group in &column_groups {
                line.extend(std::iter::repeat_n(group[level].0.clone(), value_width));
            }
            if column_depth != 0 {
                line.extend(std::iter::repeat_n(
                    if level == 0 {
                        Value::Text(TOTAL.to_owned())
                    } else {
                        Value::Empty
                    },
                    value_width,
                ));
            }
            grid.push(line);
        }
        if show_header || value_width > 1 {
            let mut line: Vec<Value> = if has_header {
                row_keys[0].clone()
            } else {
                vec![Value::Empty; row_width]
            };
            let captions: Vec<Value> = if has_header {
                values[0].clone()
            } else {
                (1..=value_width)
                    .map(|index| Value::Text(format!("Value {index}")))
                    .collect()
            };
            for _ in 0..column_groups.len() + usize::from(column_depth != 0) {
                line.extend(captions.iter().cloned());
            }
            grid.push(line);
        }
        let line_of = |label: Vec<Value>, rows: &[usize]| -> Vec<Value> {
            let mut line = label;
            for group in &column_groups {
                let inside: Vec<usize> = rows
                    .iter()
                    .copied()
                    .filter(|row| key(&column_keys, *row) == *group)
                    .collect();
                for column in 0..value_width {
                    line.push(if inside.is_empty() {
                        Value::Empty
                    } else {
                        cell(&inside, column)
                    });
                }
            }
            if column_depth != 0 {
                for column in 0..value_width {
                    line.push(cell(rows, column));
                }
            }
            line
        };
        let mut total_label = vec![Value::Text(TOTAL.to_owned())];
        total_label.resize(row_width, Value::Empty);
        if row_depth < 0 {
            grid.push(line_of(total_label.clone(), &records));
        }
        for group in &row_groups {
            let inside: Vec<usize> = records
                .iter()
                .copied()
                .filter(|row| key(&row_keys, *row) == *group)
                .collect();
            grid.push(line_of(
                group.iter().map(|key| key.0.clone()).collect(),
                &inside,
            ));
        }
        if row_depth > 0 {
            grid.push(line_of(total_label, &records));
        }
        checked(grid)
    }
}

/// Whether the data has a header row, and whether the answer shows one: `field_headers` of 0
/// none, 1 one not shown, 2 none but shown, 3 one shown; left out, a first row of text over
/// numbers is taken for a header.
fn header_mode(headers: i64, values: &Grid) -> Result<(bool, bool), ErrorKind> {
    Ok(match headers {
        -1 => {
            let detected = values.len() > 1
                && values[0]
                    .iter()
                    .all(|value| matches!(value, Value::Text(_)))
                && values[1]
                    .iter()
                    .any(|value| matches!(value, Value::Number(_)));
            (detected, detected)
        }
        0 => (false, false),
        1 => (true, false),
        2 => (false, true),
        3 => (true, true),
        _ => return Err(ErrorKind::Value),
    })
}

/// Order the groups by a key column (1 to the number of keys) or a value column after them,
/// down for a negative number.
fn sort_lines(
    lines: &mut [(Vec<Key>, Vec<Value>, Vec<usize>)],
    sort: i64,
    key_width: usize,
) -> Result<(), ErrorKind> {
    let column = usize::try_from(sort.unsigned_abs()).map_err(|_| ErrorKind::Value)?;
    if column == 0 {
        return Err(ErrorKind::Value);
    }
    lines.sort_by(|left, right| {
        let pick = |line: &(Vec<Key>, Vec<Value>, Vec<usize>)| {
            if column <= key_width {
                line.0[column - 1].clone()
            } else {
                Key(line
                    .1
                    .get(column - key_width - 1)
                    .cloned()
                    .unwrap_or(Value::Empty))
            }
        };
        let order = pick(left).cmp(&pick(right));
        if sort < 0 { order.reverse() } else { order }
    });
    Ok(())
}

/// A grid made rectangular, or `#NUM!` past the spill limit.
fn checked(mut grid: Grid) -> Result<Grid, ErrorKind> {
    let width = grid.iter().map(Vec::len).max().unwrap_or(0);
    if grid.len() * width > MAX_SPILL_CELLS {
        return Err(ErrorKind::Num);
    }
    for line in &mut grid {
        line.resize(width, Value::Empty);
    }
    Ok(grid)
}
