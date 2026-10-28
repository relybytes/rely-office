// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Recalculation: every formula of the workbook computed once, each after the formulas it
//! reads. The walk keeps its own stack rather than recursing, so a chain of a hundred thousand
//! formulas cannot exhaust the WebAssembly stack, and a formula that reaches itself computes to
//! [`ErrorKind::Circular`].

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};

use crate::address::{CellRef, MAX_ROWS};
use crate::dates::{date_from_serial, serial_from_date};
use crate::formula::{BinaryOperator, CellAddress, Expr, RangeAddress, UnaryOperator};
use crate::model::Workbook;
use crate::value::{ErrorKind, Input, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Visiting,
    Done,
}

/// A value, or a rectangle of cells a function reads one by one.
#[derive(Debug, Clone, PartialEq)]
enum Operand {
    Value(Value),
    Range {
        sheet: usize,
        start: CellRef,
        end: CellRef,
    },
}

pub(crate) fn evaluate_workbook(workbook: &Workbook) -> Vec<HashMap<CellRef, Value>> {
    let formula_cells: Vec<BTreeSet<CellRef>> = workbook
        .sheets
        .iter()
        .map(|sheet| {
            sheet
                .cells
                .iter()
                .filter(|(_, cell)| matches!(cell.input, Input::Formula(_)))
                .map(|(position, _)| *position)
                .collect()
        })
        .collect();
    let mut evaluator = Evaluator {
        workbook,
        values: vec![HashMap::new(); workbook.sheets.len()],
        states: vec![HashMap::new(); workbook.sheets.len()],
        formula_cells,
        current: std::cell::Cell::new(None),
    };
    for sheet in 0..workbook.sheets.len() {
        let cells: Vec<CellRef> = evaluator.formula_cells[sheet].iter().copied().collect();
        for cell in cells {
            evaluator.resolve(sheet, cell);
        }
    }
    evaluator.values
}

/// The workbook as computed, for formulas that belong to no cell: the conditions of conditional
/// formats and validations, each computed as if it were in the cell it is checked for.
pub(crate) struct FormulaContext<'a> {
    evaluator: Evaluator<'a>,
}

impl<'a> FormulaContext<'a> {
    pub(crate) fn new(workbook: &'a Workbook) -> Self {
        let sheets = workbook.sheets.len();
        Self {
            evaluator: Evaluator {
                workbook,
                values: workbook.values.clone(),
                states: vec![HashMap::new(); sheets],
                formula_cells: vec![BTreeSet::new(); sheets],
                current: std::cell::Cell::new(None),
            },
        }
    }

    /// The single value of a formula computed in `cell` of `sheet`; a range is `#VALUE!`.
    pub(crate) fn value(&self, sheet: usize, cell: CellRef, expression: &Expr) -> Value {
        self.evaluator.current.set(Some(cell));
        finite(self.evaluator.scalar(sheet, expression))
    }

    /// Every value a formula computed in `cell` gives: a range's cells row by row, or its value.
    pub(crate) fn values(&self, sheet: usize, cell: CellRef, expression: &Expr) -> Vec<Value> {
        self.evaluator.current.set(Some(cell));
        let mut found = Vec::new();
        self.evaluator
            .values_of(sheet, expression, |value, _| found.push(value));
        found
    }
}

/// Whether two values compare as an operator says, as a formula compares them.
pub(crate) fn holds(operator: BinaryOperator, left: &Value, right: &Value) -> bool {
    binary(operator, left, right) == Value::Bool(true)
}

struct Evaluator<'a> {
    workbook: &'a Workbook,
    values: Vec<HashMap<CellRef, Value>>,
    states: Vec<HashMap<CellRef, State>>,
    formula_cells: Vec<BTreeSet<CellRef>>,
    /// The cell whose formula is being computed, for `ROW()` and `COLUMN()`.
    current: std::cell::Cell<Option<CellRef>>,
}

impl<'a> Evaluator<'a> {
    fn expression(&self, sheet: usize, cell: CellRef) -> Option<&'a Expr> {
        let workbook: &'a Workbook = self.workbook;
        match &workbook.sheets[sheet].cells.get(&cell)?.input {
            Input::Formula(expression) => Some(expression),
            _ => None,
        }
    }

    /// Compute one formula cell after every formula cell it reads.
    fn resolve(&mut self, sheet: usize, cell: CellRef) {
        let mut stack = vec![(sheet, cell)];
        while let Some(&(current_sheet, current)) = stack.last() {
            if self.states[current_sheet].get(&current) == Some(&State::Done) {
                stack.pop();
                continue;
            }
            let Some(expression) = self.expression(current_sheet, current) else {
                stack.pop();
                continue;
            };
            self.states[current_sheet].insert(current, State::Visiting);
            let mut dependencies = Vec::new();
            self.dependencies(current_sheet, expression, &mut dependencies);
            let mut pending = Vec::new();
            let mut circular = false;
            for (dependency_sheet, dependency) in dependencies {
                match self.states[dependency_sheet].get(&dependency) {
                    Some(State::Done) => {}
                    Some(State::Visiting) => circular = true,
                    None => pending.push((dependency_sheet, dependency)),
                }
            }
            if circular {
                self.finish(current_sheet, current, Value::Error(ErrorKind::Circular));
                stack.pop();
            } else if pending.is_empty() {
                self.current.set(Some(current));
                let value = match self.evaluate(current_sheet, expression) {
                    Operand::Value(value) => finite(value),
                    // A formula that is a whole range, such as `=A1:B2`, shows no single value.
                    Operand::Range { .. } => Value::Error(ErrorKind::Value),
                };
                self.finish(current_sheet, current, value);
                stack.pop();
            } else {
                stack.extend(pending);
            }
        }
    }

    fn finish(&mut self, sheet: usize, cell: CellRef, value: Value) {
        self.values[sheet].insert(cell, value);
        self.states[sheet].insert(cell, State::Done);
    }

    /// The formula cells an expression reads, by sheet.
    fn dependencies(&self, sheet: usize, expression: &Expr, found: &mut Vec<(usize, CellRef)>) {
        match expression {
            Expr::Cell(address) => {
                if let Some(target) = self.sheet_of(sheet, address.sheet.as_deref())
                    && self.formula_cells[target].contains(&address.cell)
                {
                    found.push((target, address.cell));
                }
            }
            Expr::Range(range) => {
                if let Some(target) = self.sheet_of(sheet, range.sheet.as_deref()) {
                    let (start, end) = range.bounds();
                    for cell in
                        self.formula_cells[target].range(start..=CellRef::new(end.row, end.column))
                    {
                        if (start.column..=end.column).contains(&cell.column) {
                            found.push((target, *cell));
                        }
                    }
                }
            }
            Expr::Unary(_, operand) | Expr::Percent(operand) | Expr::Group(operand) => {
                self.dependencies(sheet, operand, found);
            }
            Expr::Binary(_, left, right) => {
                self.dependencies(sheet, left, found);
                self.dependencies(sheet, right, found);
            }
            Expr::Call(_, arguments) => {
                for argument in arguments {
                    self.dependencies(sheet, argument, found);
                }
            }
            Expr::Number(_) | Expr::Text(_) | Expr::Bool(_) | Expr::Error(_) => {}
        }
    }

    fn sheet_of(&self, current: usize, name: Option<&str>) -> Option<usize> {
        match name {
            None => Some(current),
            Some(name) => self.workbook.sheet_index(name),
        }
    }

    /// The value of a cell as a formula reads it: a computed value for a formula cell.
    fn cell_value(&self, sheet: usize, cell: CellRef) -> Value {
        match self.workbook.sheets[sheet]
            .cells
            .get(&cell)
            .map(|content| &content.input)
        {
            None => Value::Empty,
            Some(Input::Number(number)) => Value::Number(*number),
            Some(Input::Text(text)) => Value::Text(text.clone()),
            Some(Input::Bool(value)) => Value::Bool(*value),
            Some(Input::Formula(_)) => self.values[sheet]
                .get(&cell)
                .cloned()
                .unwrap_or(Value::Empty),
        }
    }

    fn evaluate(&self, sheet: usize, expression: &Expr) -> Operand {
        match expression {
            Expr::Number(number) => Operand::Value(Value::Number(*number)),
            Expr::Text(text) => Operand::Value(Value::Text(text.clone())),
            Expr::Bool(value) => Operand::Value(Value::Bool(*value)),
            Expr::Error(kind) => Operand::Value(Value::Error(*kind)),
            Expr::Cell(address) => Operand::Value(self.address_value(sheet, address)),
            Expr::Range(range) => self.range(sheet, range),
            Expr::Group(inner) => self.evaluate(sheet, inner),
            Expr::Unary(operator, operand) => {
                Operand::Value(match self.scalar(sheet, operand).as_number() {
                    Ok(number) => Value::Number(match operator {
                        UnaryOperator::Plus => number,
                        UnaryOperator::Minus => -number,
                    }),
                    Err(kind) => Value::Error(kind),
                })
            }
            Expr::Percent(operand) => {
                Operand::Value(match self.scalar(sheet, operand).as_number() {
                    Ok(number) => Value::Number(number / 100.0),
                    Err(kind) => Value::Error(kind),
                })
            }
            Expr::Binary(operator, left, right) => {
                let left = self.scalar(sheet, left);
                let right = self.scalar(sheet, right);
                Operand::Value(binary(*operator, &left, &right))
            }
            Expr::Call(name, arguments) => {
                Operand::Value(finite(self.call(sheet, name, arguments)))
            }
        }
    }

    fn address_value(&self, sheet: usize, address: &CellAddress) -> Value {
        match self.sheet_of(sheet, address.sheet.as_deref()) {
            Some(target) => self.cell_value(target, address.cell),
            None => Value::Error(ErrorKind::Ref),
        }
    }

    fn range(&self, sheet: usize, range: &RangeAddress) -> Operand {
        match self.sheet_of(sheet, range.sheet.as_deref()) {
            Some(target) => {
                let (start, end) = range.bounds();
                Operand::Range {
                    sheet: target,
                    start,
                    end,
                }
            }
            None => Operand::Value(Value::Error(ErrorKind::Ref)),
        }
    }

    /// An expression where one value is expected: a range there is `#VALUE!`.
    fn scalar(&self, sheet: usize, expression: &Expr) -> Value {
        match self.evaluate(sheet, expression) {
            Operand::Value(value) => value,
            Operand::Range { .. } => Value::Error(ErrorKind::Value),
        }
    }

    /// Every value an argument holds, a range cell by cell row by row, with whether it came from
    /// a range (where text and booleans are skipped by the numeric aggregates).
    fn values_of(&self, sheet: usize, expression: &Expr, mut visit: impl FnMut(Value, bool)) {
        match self.evaluate(sheet, expression) {
            Operand::Value(value) => visit(value, false),
            Operand::Range {
                sheet: target,
                start,
                end,
            } => {
                for row in start.row..=end.row {
                    for column in start.column..=end.column {
                        visit(self.cell_value(target, CellRef::new(row, column)), true);
                    }
                    // A whole-column range stops after the sheet's last row with content.
                    if end.row == MAX_ROWS - 1 && row > self.last_row(target) {
                        break;
                    }
                }
            }
        }
    }

    fn last_row(&self, sheet: usize) -> u32 {
        self.workbook.sheets[sheet]
            .cells
            .keys()
            .last()
            .map_or(0, |cell| cell.row)
    }

    /// The numbers of the arguments, as SUM reads them: numbers and numeric text typed as an
    /// argument count, a range counts only its numbers; the first error wins.
    fn numbers(&self, sheet: usize, arguments: &[Expr]) -> Result<Vec<f64>, ErrorKind> {
        let mut numbers = Vec::new();
        let mut error = None;
        for argument in arguments {
            self.values_of(sheet, argument, |value, from_range| {
                if error.is_some() {
                    return;
                }
                match (value, from_range) {
                    (Value::Number(number), _) => numbers.push(number),
                    (Value::Error(kind), _) => error = Some(kind),
                    (Value::Empty | Value::Text(_) | Value::Bool(_), true)
                    | (Value::Empty, false) => {}
                    (other, false) => match other.as_number() {
                        Ok(number) => numbers.push(number),
                        Err(kind) => error = Some(kind),
                    },
                }
            });
        }
        error.map_or(Ok(numbers), Err)
    }

    fn number(&self, sheet: usize, argument: Option<&Expr>) -> Result<f64, ErrorKind> {
        match argument {
            Some(argument) => self.scalar(sheet, argument).as_number(),
            None => Err(ErrorKind::Value),
        }
    }

    fn text(&self, sheet: usize, argument: Option<&Expr>) -> Result<String, ErrorKind> {
        match argument {
            Some(argument) => self.scalar(sheet, argument).as_text(),
            None => Err(ErrorKind::Value),
        }
    }

    /// The cells of a range argument as a grid of rows.
    fn grid(&self, sheet: usize, argument: Option<&Expr>) -> Result<Vec<Vec<Value>>, ErrorKind> {
        let Some(argument) = argument else {
            return Err(ErrorKind::Value);
        };
        match self.evaluate(sheet, argument) {
            Operand::Value(Value::Error(kind)) => Err(kind),
            Operand::Value(value) => Ok(vec![vec![value]]),
            Operand::Range {
                sheet: target,
                start,
                end,
            } => {
                let last = if end.row == MAX_ROWS - 1 {
                    self.last_row(target).max(start.row)
                } else {
                    end.row
                };
                Ok((start.row..=end.row.min(last))
                    .map(|row| {
                        (start.column..=end.column)
                            .map(|column| self.cell_value(target, CellRef::new(row, column)))
                            .collect()
                    })
                    .collect())
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn call(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Value {
        let result: Result<Value, ErrorKind> = (|| {
            let count = arguments.len();
            let argument = |index: usize| arguments.get(index);
            Ok(match name {
                "SUM" => Value::Number(self.numbers(sheet, arguments)?.iter().sum()),
                "PRODUCT" => Value::Number(self.numbers(sheet, arguments)?.iter().product()),
                "AVERAGE" => {
                    let numbers = self.numbers(sheet, arguments)?;
                    if numbers.is_empty() {
                        return Err(ErrorKind::Div0);
                    }
                    #[allow(clippy::cast_precision_loss)]
                    Value::Number(numbers.iter().sum::<f64>() / numbers.len() as f64)
                }
                "MIN" => Value::Number(
                    self.numbers(sheet, arguments)?
                        .into_iter()
                        .reduce(f64::min)
                        .unwrap_or(0.0),
                ),
                "MAX" => Value::Number(
                    self.numbers(sheet, arguments)?
                        .into_iter()
                        .reduce(f64::max)
                        .unwrap_or(0.0),
                ),
                "COUNT" | "COUNTA" | "COUNTBLANK" => {
                    let mut total = 0_u32;
                    for expression in arguments {
                        self.values_of(sheet, expression, |value, _| {
                            let counted = match name {
                                "COUNT" => matches!(value, Value::Number(_)),
                                "COUNTA" => !matches!(value, Value::Empty),
                                _ => {
                                    matches!(value, Value::Empty)
                                        || matches!(&value, Value::Text(text) if text.is_empty())
                                }
                            };
                            total += u32::from(counted);
                        });
                    }
                    Value::Number(f64::from(total))
                }
                "IF" => {
                    if !(2..=3).contains(&count) {
                        return Err(ErrorKind::Value);
                    }
                    let condition = self.scalar(sheet, &arguments[0]).as_bool()?;
                    match (condition, argument(1), argument(2)) {
                        (true, Some(branch), _) | (false, _, Some(branch)) => {
                            empty_as_zero(self.scalar(sheet, branch))
                        }
                        (false, _, None) => Value::Bool(false),
                        (true, None, _) => Value::Bool(true),
                    }
                }
                "IFERROR" => {
                    let value = self.scalar(sheet, argument(0).ok_or(ErrorKind::Value)?);
                    if matches!(value, Value::Error(_)) {
                        empty_as_zero(self.scalar(sheet, argument(1).ok_or(ErrorKind::Value)?))
                    } else {
                        value
                    }
                }
                "AND" | "OR" => {
                    if count == 0 {
                        return Err(ErrorKind::Value);
                    }
                    let mut values = Vec::new();
                    let mut error = None;
                    for expression in arguments {
                        self.values_of(sheet, expression, |value, from_range| match value {
                            Value::Error(kind) => error = error.or(Some(kind)),
                            Value::Empty | Value::Text(_) if from_range => {}
                            other => match other.as_bool() {
                                Ok(flag) => values.push(flag),
                                Err(kind) => error = error.or(Some(kind)),
                            },
                        });
                    }
                    if let Some(kind) = error {
                        return Err(kind);
                    }
                    if values.is_empty() {
                        return Err(ErrorKind::Value);
                    }
                    Value::Bool(if name == "AND" {
                        values.iter().all(|flag| *flag)
                    } else {
                        values.iter().any(|flag| *flag)
                    })
                }
                "NOT" => Value::Bool(
                    !self
                        .scalar(sheet, argument(0).ok_or(ErrorKind::Value)?)
                        .as_bool()?,
                ),
                "TRUE" => Value::Bool(true),
                "FALSE" => Value::Bool(false),
                "ROUND" | "ROUNDUP" | "ROUNDDOWN" => {
                    let number = self.number(sheet, argument(0))?;
                    #[allow(clippy::cast_possible_truncation)]
                    let digits = self.number(sheet, argument(1)).unwrap_or(0.0).trunc() as i32;
                    let digits = digits.clamp(-15, 15);
                    // Dividing by a power of ten is exact where multiplying by its inverse is not.
                    let scale = 10f64.powi(digits.abs());
                    let scaled = if digits >= 0 {
                        number * scale
                    } else {
                        number / scale
                    };
                    // A tiny nudge so 2.675 rounds to 2.68 as a person expects, not as binary does.
                    let nudged = scaled + scaled.signum() * scaled.abs() * f64::EPSILON * 4.0;
                    let rounded = match name {
                        "ROUND" => nudged.round(),
                        "ROUNDUP" => scaled.signum() * (scaled.abs() - 1e-9).ceil(),
                        _ => scaled.trunc(),
                    };
                    Value::Number(if digits >= 0 {
                        rounded / scale
                    } else {
                        rounded * scale
                    })
                }
                "ABS" => Value::Number(self.number(sheet, argument(0))?.abs()),
                "INT" => Value::Number(self.number(sheet, argument(0))?.floor()),
                "SIGN" => {
                    let number = self.number(sheet, argument(0))?;
                    Value::Number(if number == 0.0 { 0.0 } else { number.signum() })
                }
                "MOD" => {
                    let number = self.number(sheet, argument(0))?;
                    let divisor = self.number(sheet, argument(1))?;
                    if divisor == 0.0 {
                        return Err(ErrorKind::Div0);
                    }
                    Value::Number(number - divisor * (number / divisor).floor())
                }
                "POWER" => Value::Number(
                    self.number(sheet, argument(0))?
                        .powf(self.number(sheet, argument(1))?),
                ),
                "SQRT" => {
                    let number = self.number(sheet, argument(0))?;
                    if number < 0.0 {
                        return Err(ErrorKind::Num);
                    }
                    Value::Number(number.sqrt())
                }
                "PI" => Value::Number(std::f64::consts::PI),
                "CONCATENATE" | "CONCAT" => {
                    let mut text = String::new();
                    let mut error = None;
                    for expression in arguments {
                        self.values_of(sheet, expression, |value, _| match value.as_text() {
                            Ok(part) => text.push_str(&part),
                            Err(kind) => error = error.or(Some(kind)),
                        });
                    }
                    if let Some(kind) = error {
                        return Err(kind);
                    }
                    Value::Text(text)
                }
                "LEN" => {
                    #[allow(clippy::cast_precision_loss)]
                    let length = self.text(sheet, argument(0))?.chars().count() as f64;
                    Value::Number(length)
                }
                "LEFT" | "RIGHT" => {
                    let text = self.text(sheet, argument(0))?;
                    let wanted = if count > 1 {
                        self.number(sheet, argument(1))?
                    } else {
                        1.0
                    };
                    if wanted < 0.0 {
                        return Err(ErrorKind::Value);
                    }
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let wanted = wanted.trunc() as usize;
                    let characters: Vec<char> = text.chars().collect();
                    let wanted = wanted.min(characters.len());
                    Value::Text(if name == "LEFT" {
                        characters[..wanted].iter().collect()
                    } else {
                        characters[characters.len() - wanted..].iter().collect()
                    })
                }
                "MID" => {
                    let text = self.text(sheet, argument(0))?;
                    let start = self.number(sheet, argument(1))?;
                    let length = self.number(sheet, argument(2))?;
                    if start < 1.0 || length < 0.0 {
                        return Err(ErrorKind::Value);
                    }
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let (start, length) = (start.trunc() as usize - 1, length.trunc() as usize);
                    Value::Text(text.chars().skip(start).take(length).collect())
                }
                "UPPER" => Value::Text(self.text(sheet, argument(0))?.to_uppercase()),
                "LOWER" => Value::Text(self.text(sheet, argument(0))?.to_lowercase()),
                "TRIM" => Value::Text(
                    self.text(sheet, argument(0))?
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                ),
                "ISBLANK" => Value::Bool(matches!(
                    self.scalar(sheet, argument(0).ok_or(ErrorKind::Value)?),
                    Value::Empty
                )),
                "ISNUMBER" => Value::Bool(matches!(
                    self.scalar(sheet, argument(0).ok_or(ErrorKind::Value)?),
                    Value::Number(_)
                )),
                "ISTEXT" => Value::Bool(matches!(
                    self.scalar(sheet, argument(0).ok_or(ErrorKind::Value)?),
                    Value::Text(_)
                )),
                "ISERROR" => Value::Bool(matches!(
                    self.scalar(sheet, argument(0).ok_or(ErrorKind::Value)?),
                    Value::Error(_)
                )),
                "SUMIF" | "COUNTIF" | "AVERAGEIF" => {
                    let range = self.grid(sheet, argument(0))?;
                    let criterion =
                        Criterion::parse(&self.scalar(sheet, argument(1).ok_or(ErrorKind::Value)?));
                    let sum_range = if count > 2 && name != "COUNTIF" {
                        self.grid(sheet, argument(2))?
                    } else {
                        range.clone()
                    };
                    let mut matched = 0_u32;
                    let mut total = 0.0;
                    for (row_index, row) in range.iter().enumerate() {
                        for (column_index, value) in row.iter().enumerate() {
                            if criterion.matches(value) {
                                matched += 1;
                                if let Some(Value::Number(number)) = sum_range
                                    .get(row_index)
                                    .and_then(|row| row.get(column_index))
                                {
                                    total += number;
                                }
                            }
                        }
                    }
                    match name {
                        "COUNTIF" => Value::Number(f64::from(matched)),
                        "SUMIF" => Value::Number(total),
                        _ if matched == 0 => return Err(ErrorKind::Div0),
                        _ => Value::Number(total / f64::from(matched)),
                    }
                }
                "SUMIFS" | "COUNTIFS" => {
                    let (target, pairs) = if name == "SUMIFS" {
                        (
                            Some(self.grid(sheet, argument(0))?),
                            &arguments[1.min(count)..],
                        )
                    } else {
                        (None, arguments)
                    };
                    if pairs.is_empty() || pairs.len() % 2 != 0 {
                        return Err(ErrorKind::Value);
                    }
                    let mut conditions = Vec::new();
                    for pair in pairs.chunks(2) {
                        conditions.push((
                            self.grid(sheet, Some(&pair[0]))?,
                            Criterion::parse(&self.scalar(sheet, &pair[1])),
                        ));
                    }
                    let shape = &conditions[0].0;
                    let mut matched = 0_u32;
                    let mut total = 0.0;
                    for (row_index, row) in shape.iter().enumerate() {
                        for column_index in 0..row.len() {
                            let all = conditions.iter().all(|(grid, criterion)| {
                                grid.get(row_index)
                                    .and_then(|row| row.get(column_index))
                                    .is_some_and(|value| criterion.matches(value))
                            });
                            if all {
                                matched += 1;
                                if let Some(Value::Number(number)) = target
                                    .as_ref()
                                    .and_then(|grid| grid.get(row_index))
                                    .and_then(|row| row.get(column_index))
                                {
                                    total += number;
                                }
                            }
                        }
                    }
                    Value::Number(if name == "SUMIFS" {
                        total
                    } else {
                        f64::from(matched)
                    })
                }
                "VLOOKUP" | "HLOOKUP" => {
                    let lookup = self.scalar(sheet, argument(0).ok_or(ErrorKind::Value)?);
                    let mut table = self.grid(sheet, argument(1))?;
                    if name == "HLOOKUP" {
                        table = transpose(&table);
                    }
                    let index = self.number(sheet, argument(2))?;
                    let exact = count > 3 && !self.scalar(sheet, &arguments[3]).as_bool()?;
                    if index < 1.0 {
                        return Err(ErrorKind::Value);
                    }
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let index = index.trunc() as usize - 1;
                    let keys: Vec<Value> = table
                        .iter()
                        .map(|row| row.first().cloned().unwrap_or(Value::Empty))
                        .collect();
                    let found =
                        find(&keys, &lookup, i8::from(!exact)).ok_or(ErrorKind::NotAvailable)?;
                    table[found].get(index).cloned().ok_or(ErrorKind::Ref)?
                }
                "MATCH" => {
                    let lookup = self.scalar(sheet, argument(0).ok_or(ErrorKind::Value)?);
                    let grid = self.grid(sheet, argument(1))?;
                    let kind = if count > 2 {
                        self.number(sheet, argument(2))?
                    } else {
                        1.0
                    };
                    let keys: Vec<Value> = if grid.len() == 1 {
                        grid[0].clone()
                    } else {
                        grid.iter()
                            .map(|row| row.first().cloned().unwrap_or(Value::Empty))
                            .collect()
                    };
                    #[allow(clippy::cast_possible_truncation)]
                    let found =
                        find(&keys, &lookup, kind.signum() as i8).ok_or(ErrorKind::NotAvailable)?;
                    #[allow(clippy::cast_precision_loss)]
                    Value::Number((found + 1) as f64)
                }
                "INDEX" => {
                    let grid = self.grid(sheet, argument(0))?;
                    let row = self.number(sheet, argument(1))?;
                    let column = if count > 2 {
                        self.number(sheet, argument(2))?
                    } else {
                        1.0
                    };
                    if row < 0.0 || column < 0.0 {
                        return Err(ErrorKind::Value);
                    }
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let (row, column) = (row.trunc() as usize, column.trunc() as usize);
                    // A single row indexes its columns with one number.
                    let (row, column) = if grid.len() == 1 && count == 2 {
                        (1, row)
                    } else {
                        (row, column)
                    };
                    grid.get(row.max(1) - 1)
                        .and_then(|cells| cells.get(column.max(1) - 1))
                        .cloned()
                        .ok_or(ErrorKind::Ref)?
                }
                "DATE" => {
                    #[allow(clippy::cast_possible_truncation)]
                    let part = |index: usize| {
                        self.number(sheet, argument(index))
                            .map(|number| number.trunc() as i64)
                    };
                    let mut year = part(0)?;
                    if (0..1900).contains(&year) {
                        year += 1900;
                    }
                    Value::Number(serial_from_date(year, part(1)?, part(2)?).ok_or(ErrorKind::Num)?)
                }
                "YEAR" | "MONTH" | "DAY" => {
                    let (year, month, day) =
                        date_from_serial(self.number(sheet, argument(0))?).ok_or(ErrorKind::Num)?;
                    #[allow(clippy::cast_precision_loss)]
                    Value::Number(match name {
                        "YEAR" => year,
                        "MONTH" => month,
                        _ => day,
                    } as f64)
                }
                "TODAY" => {
                    Value::Number(self.workbook.today.ok_or(ErrorKind::NotAvailable)?.floor())
                }
                "NOW" => Value::Number(self.workbook.today.ok_or(ErrorKind::NotAvailable)?),
                _ => match self.extended(sheet, name, arguments) {
                    Some(result) => result?,
                    None => return Err(ErrorKind::Name),
                },
            })
        })();
        result.unwrap_or_else(Value::Error)
    }
}

/// An empty branch of IF shows 0, as in Excel.
fn empty_as_zero(value: Value) -> Value {
    if value == Value::Empty {
        Value::Number(0.0)
    } else {
        value
    }
}

/// A value Excel can show: an infinite or undefined number is `#NUM!`.
fn finite(value: Value) -> Value {
    match value {
        Value::Number(number) if !number.is_finite() => Value::Error(ErrorKind::Num),
        other => other,
    }
}

fn transpose(grid: &[Vec<Value>]) -> Vec<Vec<Value>> {
    let width = grid.iter().map(Vec::len).max().unwrap_or(0);
    (0..width)
        .map(|column| {
            grid.iter()
                .map(|row| row.get(column).cloned().unwrap_or(Value::Empty))
                .collect()
        })
        .collect()
}

/// The index of `lookup` in `keys`: `kind` 0 the first equal one, 1 the last one not greater
/// in an ascending list, -1 the last one not smaller in a descending list.
fn find(keys: &[Value], lookup: &Value, kind: i8) -> Option<usize> {
    if kind == 0 {
        return keys.iter().position(|key| {
            compare(key, lookup) == Ordering::Equal && !matches!(key, Value::Empty)
        });
    }
    let mut found = None;
    for (index, key) in keys.iter().enumerate() {
        if matches!(key, Value::Empty) {
            continue;
        }
        let ordering = compare(key, lookup);
        let acceptable = if kind > 0 {
            ordering != Ordering::Greater
        } else {
            ordering != Ordering::Less
        };
        if acceptable {
            found = Some(index);
        } else {
            break;
        }
    }
    found
}

/// How spreadsheets order values: numbers before text before booleans, text without case.
fn compare(left: &Value, right: &Value) -> Ordering {
    fn rank(value: &Value) -> u8 {
        match value {
            Value::Number(_) | Value::Empty => 0,
            Value::Text(_) => 1,
            Value::Bool(_) => 2,
            Value::Error(_) => 3,
        }
    }
    match (left, right) {
        (Value::Empty, Value::Text(text)) | (Value::Text(text), Value::Empty)
            if text.is_empty() =>
        {
            Ordering::Equal
        }
        (Value::Empty, Value::Bool(false)) | (Value::Bool(false), Value::Empty) => Ordering::Equal,
        _ => match (left.as_number(), right.as_number(), rank(left), rank(right)) {
            (Ok(a), Ok(b), 0, 0) => a.partial_cmp(&b).unwrap_or(Ordering::Equal),
            (_, _, 1, 1) => {
                let a = left.as_text().unwrap_or_default().to_lowercase();
                let b = right.as_text().unwrap_or_default().to_lowercase();
                a.cmp(&b)
            }
            (_, _, 2, 2) => left
                .as_bool()
                .unwrap_or(false)
                .cmp(&right.as_bool().unwrap_or(false)),
            (_, _, a, b) => a.cmp(&b),
        },
    }
}

fn binary(operator: BinaryOperator, left: &Value, right: &Value) -> Value {
    if let Value::Error(kind) = left {
        return Value::Error(*kind);
    }
    if let Value::Error(kind) = right {
        return Value::Error(*kind);
    }
    let arithmetic =
        |apply: fn(f64, f64) -> Result<f64, ErrorKind>| match (left.as_number(), right.as_number())
        {
            (Ok(a), Ok(b)) => apply(a, b).map_or_else(Value::Error, Value::Number),
            (Err(kind), _) | (_, Err(kind)) => Value::Error(kind),
        };
    match operator {
        BinaryOperator::Add => arithmetic(|a, b| Ok(a + b)),
        BinaryOperator::Subtract => arithmetic(|a, b| Ok(a - b)),
        BinaryOperator::Multiply => arithmetic(|a, b| Ok(a * b)),
        BinaryOperator::Divide => arithmetic(|a, b| {
            if b == 0.0 {
                Err(ErrorKind::Div0)
            } else {
                Ok(a / b)
            }
        }),
        BinaryOperator::Power => arithmetic(|a, b| {
            if a == 0.0 && b < 0.0 {
                Err(ErrorKind::Div0)
            } else {
                Ok(a.powf(b))
            }
        }),
        BinaryOperator::Concatenate => match (left.as_text(), right.as_text()) {
            (Ok(a), Ok(b)) => Value::Text(a + &b),
            (Err(kind), _) | (_, Err(kind)) => Value::Error(kind),
        },
        BinaryOperator::Equal => Value::Bool(compare(left, right) == Ordering::Equal),
        BinaryOperator::NotEqual => Value::Bool(compare(left, right) != Ordering::Equal),
        BinaryOperator::Less => Value::Bool(compare(left, right) == Ordering::Less),
        BinaryOperator::LessOrEqual => Value::Bool(compare(left, right) != Ordering::Greater),
        BinaryOperator::Greater => Value::Bool(compare(left, right) == Ordering::Greater),
        BinaryOperator::GreaterOrEqual => Value::Bool(compare(left, right) != Ordering::Less),
    }
}

/// A condition of SUMIF and COUNTIF: `>5`, `<>a`, `=x`, `a*`, or a plain value.
struct Criterion {
    operator: BinaryOperator,
    operand: Value,
    /// For an equality on text with `*` or `?`.
    pattern: Option<Vec<char>>,
}

impl Criterion {
    fn parse(criterion: &Value) -> Self {
        let Value::Text(text) = criterion else {
            return Self {
                operator: BinaryOperator::Equal,
                operand: criterion.clone(),
                pattern: None,
            };
        };
        let (operator, rest) = [
            ("<=", BinaryOperator::LessOrEqual),
            (">=", BinaryOperator::GreaterOrEqual),
            ("<>", BinaryOperator::NotEqual),
            ("<", BinaryOperator::Less),
            (">", BinaryOperator::Greater),
            ("=", BinaryOperator::Equal),
        ]
        .into_iter()
        .find_map(|(symbol, operator)| text.strip_prefix(symbol).map(|rest| (operator, rest)))
        .unwrap_or((BinaryOperator::Equal, text.as_str()));
        let operand = rest
            .trim()
            .parse::<f64>()
            .map_or_else(|_| Value::Text(rest.to_owned()), Value::Number);
        let pattern = match (&operand, operator) {
            (Value::Text(text), BinaryOperator::Equal | BinaryOperator::NotEqual)
                if text.contains(['*', '?']) =>
            {
                Some(text.to_lowercase().chars().collect())
            }
            _ => None,
        };
        Self {
            operator,
            operand,
            pattern,
        }
    }

    fn matches(&self, value: &Value) -> bool {
        if let Some(pattern) = &self.pattern {
            let text: Vec<char> = value
                .as_text()
                .unwrap_or_default()
                .to_lowercase()
                .chars()
                .collect();
            let matched = wildcard(pattern, &text);
            return if self.operator == BinaryOperator::NotEqual {
                !matched
            } else {
                matched
            };
        }
        // A number criterion only matches numbers, a text one only texts, as Excel counts.
        let comparable = matches!(
            (&self.operand, value),
            (Value::Number(_), Value::Number(_))
                | (Value::Text(_), Value::Text(_) | Value::Empty)
                | (Value::Bool(_), Value::Bool(_))
        );
        if !comparable {
            return self.operator == BinaryOperator::NotEqual;
        }
        if let (Value::Text(wanted), Value::Empty) = (&self.operand, value) {
            return match self.operator {
                BinaryOperator::Equal => wanted.is_empty(),
                BinaryOperator::NotEqual => !wanted.is_empty(),
                _ => false,
            };
        }
        matches!(
            binary(self.operator, value, &self.operand),
            Value::Bool(true)
        )
    }
}

/// Whether `text` matches a pattern of `*` (any run) and `?` (one character); `~` escapes.
fn wildcard(pattern: &[char], text: &[char]) -> bool {
    let (mut pattern_index, mut text_index) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while text_index < text.len() {
        match pattern.get(pattern_index) {
            Some('~') if pattern.get(pattern_index + 1) == Some(&text[text_index]) => {
                pattern_index += 2;
                text_index += 1;
            }
            Some('*') => {
                star = Some((pattern_index, text_index));
                pattern_index += 1;
            }
            Some('?') => {
                pattern_index += 1;
                text_index += 1;
            }
            Some(character) if *character == text[text_index] => {
                pattern_index += 1;
                text_index += 1;
            }
            _ => match star {
                Some((star_pattern, star_text)) => {
                    pattern_index = star_pattern + 1;
                    text_index = star_text + 1;
                    star = Some((star_pattern, star_text + 1));
                }
                None => return false,
            },
        }
    }
    pattern[pattern_index..]
        .iter()
        .all(|character| *character == '*')
}

mod functions;
mod more;

#[cfg(test)]
mod tests;
