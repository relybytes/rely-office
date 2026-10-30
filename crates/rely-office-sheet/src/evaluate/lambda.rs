// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! Names in formulas and the functions formulas write: `LET` binds names, `LAMBDA` makes a
//! function of its parameters, and `MAP`, `REDUCE`, `SCAN`, `BYROW`, `BYCOL` and `MAKEARRAY`
//! call one over arrays. A name is looked up in the `LET`s and `LAMBDA`s around it, innermost
//! first, then among the names the workbook defines; a function sees the names around the
//! `LAMBDA` that made it, not those around its call.

use std::rc::Rc;

use super::{Evaluator, MAX_SPILL_CELLS, Operand};
use crate::formula::{EXTERNAL_FUNCTIONS, Expr, FUNCTION_NAMES};
use crate::value::{ErrorKind, Value};

/// How deep functions may call functions, a recursive name included, before `#NUM!`.
const MAX_CALL_DEPTH: u32 = 100;

/// A function a `LAMBDA` made: its parameters, its body, and the names it was made among.
#[derive(Debug, PartialEq)]
pub(super) struct Closure {
    parameters: Vec<String>,
    body: Expr,
    captured: Vec<Binding>,
}

impl Closure {
    /// How many parameters the function takes.
    pub(super) fn arity(&self) -> usize {
        self.parameters.len()
    }
}

/// A name a `LET` or a function binds; `None` for a parameter the call left out.
pub(super) type Binding = (String, Option<Operand>);

/// What a name is bound to in the scopes around it.
enum Bound {
    Value(Operand),
    /// A parameter the call left out.
    Omitted,
}

impl Evaluator<'_> {
    /// The functions that bind names or call functions; `None` for any other.
    pub(super) fn lambda_call(
        &self,
        sheet: usize,
        name: &str,
        arguments: &[Expr],
    ) -> Option<Operand> {
        let result = match name {
            "LET" => self.let_names(sheet, arguments),
            "LAMBDA" => self.lambda(arguments),
            "ISOMITTED" => Ok(Operand::Value(Value::Bool(match arguments {
                [Expr::Name(name)] => matches!(self.binding(name), Some(Bound::Omitted)),
                _ => false,
            }))),
            "MAP" => self.map(sheet, arguments),
            "REDUCE" => self.reduce(sheet, arguments, false),
            "SCAN" => self.reduce(sheet, arguments, true),
            "BYROW" => self.by_line(sheet, arguments, true),
            "BYCOL" => self.by_line(sheet, arguments, false),
            "MAKEARRAY" => self.make_array(sheet, arguments),
            _ if FUNCTION_NAMES.binary_search(&name).is_err()
                && !EXTERNAL_FUNCTIONS.contains(&name) =>
            {
                // A name the grid does not define as a function: a function bound to the name.
                match self.name_operand(sheet, name) {
                    Some(Operand::Lambda(closure)) => {
                        let values = self.arguments(sheet, arguments);
                        Ok(self.invoke(sheet, &closure, values))
                    }
                    Some(Operand::Value(Value::Error(kind))) => Err(kind),
                    Some(_) => Err(ErrorKind::Value),
                    None => Err(ErrorKind::Name),
                }
            }
            _ => return None,
        };
        Some(result.unwrap_or_else(|kind| Operand::Value(Value::Error(kind))))
    }

    /// A function a formula computed, called with arguments.
    pub(super) fn invoke_expression(
        &self,
        sheet: usize,
        callee: &Expr,
        arguments: &[Expr],
    ) -> Operand {
        match self.evaluate(sheet, callee) {
            Operand::Lambda(closure) => {
                let values = self.arguments(sheet, arguments);
                self.invoke(sheet, &closure, values)
            }
            Operand::Value(Value::Error(kind)) => Operand::Value(Value::Error(kind)),
            _ => Operand::Value(Value::Error(ErrorKind::Value)),
        }
    }

    /// What a name stands for where it is read: what a `LET` or a function bound, then what
    /// the workbook defines; `#NAME?` for a name nothing gives.
    pub(super) fn name_value(&self, sheet: usize, name: &str) -> Operand {
        self.name_operand(sheet, name)
            .unwrap_or(Operand::Value(Value::Error(ErrorKind::Name)))
    }

    fn name_operand(&self, sheet: usize, name: &str) -> Option<Operand> {
        match self.binding(name) {
            Some(Bound::Value(operand)) => return Some(operand),
            Some(Bound::Omitted) => return Some(Operand::Value(Value::Empty)),
            None => {}
        }
        let expression = self.workbook.defined_name(sheet, name)?;
        if self.calls.get() >= MAX_CALL_DEPTH {
            return Some(Operand::Value(Value::Error(ErrorKind::Num)));
        }
        self.calls.set(self.calls.get() + 1);
        // A defined name sees no `LET` around the formula that reads it.
        let outer = self.scopes.replace(Vec::new());
        let operand = self.evaluate(sheet, expression);
        self.scopes.replace(outer);
        self.calls.set(self.calls.get() - 1);
        Some(operand)
    }

    /// The binding of a name in the scopes around, innermost first; `None` when none binds it.
    fn binding(&self, name: &str) -> Option<Bound> {
        self.scopes.borrow().iter().rev().find_map(|scope| {
            scope
                .iter()
                .rev()
                .find(|(bound, _)| bound.eq_ignore_ascii_case(name))
                .map(|(_, operand)| operand.clone().map_or(Bound::Omitted, Bound::Value))
        })
    }

    fn let_names(&self, sheet: usize, arguments: &[Expr]) -> Result<Operand, ErrorKind> {
        if arguments.len() < 3 || arguments.len().is_multiple_of(2) {
            return Err(ErrorKind::Value);
        }
        self.scopes.borrow_mut().push(Vec::new());
        let mut result = Ok(());
        for pair in arguments[..arguments.len() - 1].chunks(2) {
            let Expr::Name(name) = &pair[0] else {
                result = Err(ErrorKind::Value);
                break;
            };
            let value = self.evaluate(sheet, &pair[1]);
            if let Some(scope) = self.scopes.borrow_mut().last_mut() {
                scope.push((name.clone(), Some(value)));
            }
        }
        let operand = result.map(|()| self.evaluate(sheet, &arguments[arguments.len() - 1]));
        self.scopes.borrow_mut().pop();
        operand
    }

    fn lambda(&self, arguments: &[Expr]) -> Result<Operand, ErrorKind> {
        let Some((body, parameters)) = arguments.split_last() else {
            return Err(ErrorKind::Value);
        };
        let parameters = parameters
            .iter()
            .map(|parameter| match parameter {
                Expr::Name(name) => Ok(name.clone()),
                _ => Err(ErrorKind::Value),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if parameters.len() > 253 {
            return Err(ErrorKind::Value);
        }
        let captured = self.scopes.borrow().iter().flatten().cloned().collect();
        Ok(Operand::Lambda(Rc::new(Closure {
            parameters,
            body: body.clone(),
            captured,
        })))
    }

    /// The arguments of a call, each as it computes; one left empty is left out.
    fn arguments(&self, sheet: usize, arguments: &[Expr]) -> Vec<Option<Operand>> {
        arguments
            .iter()
            .map(|argument| match argument {
                Expr::Text(text) if text.is_empty() => None,
                argument => Some(self.evaluate(sheet, argument)),
            })
            .collect()
    }

    /// Call a function: its body computed among the names it was made with and its parameters.
    pub(super) fn invoke(
        &self,
        sheet: usize,
        closure: &Closure,
        mut values: Vec<Option<Operand>>,
    ) -> Operand {
        if values.len() > closure.parameters.len() {
            return Operand::Value(Value::Error(ErrorKind::Value));
        }
        if self.calls.get() >= MAX_CALL_DEPTH {
            return Operand::Value(Value::Error(ErrorKind::Num));
        }
        values.resize(closure.parameters.len(), None);
        let parameters: Vec<Binding> = closure.parameters.iter().cloned().zip(values).collect();
        self.calls.set(self.calls.get() + 1);
        let outer = self
            .scopes
            .replace(vec![closure.captured.clone(), parameters]);
        let operand = self.evaluate(sheet, &closure.body);
        self.scopes.replace(outer);
        self.calls.set(self.calls.get() - 1);
        operand
    }

    /// A function argument, the last of a call.
    fn closure(&self, sheet: usize, argument: Option<&Expr>) -> Result<Rc<Closure>, ErrorKind> {
        match argument.map(|argument| self.evaluate(sheet, argument)) {
            Some(Operand::Lambda(closure)) => Ok(closure),
            Some(Operand::Value(Value::Error(kind))) => Err(kind),
            _ => Err(ErrorKind::Value),
        }
    }

    /// One value a function gave for one cell of an array; an array there is `#CALC!`.
    fn single(&self, operand: Operand) -> Value {
        match operand {
            Operand::Value(value) => value,
            Operand::Array(grid) if grid.len() == 1 && grid[0].len() == 1 => grid[0][0].clone(),
            Operand::Range { sheet, start, end } if start == end => self.cell_value(sheet, start),
            _ => Value::Error(ErrorKind::Calc),
        }
    }

    fn map(&self, sheet: usize, arguments: &[Expr]) -> Result<Operand, ErrorKind> {
        let Some((function, arrays)) = arguments.split_last() else {
            return Err(ErrorKind::Value);
        };
        if arrays.is_empty() {
            return Err(ErrorKind::Value);
        }
        let closure = self.closure(sheet, Some(function))?;
        let grids = arrays
            .iter()
            .map(|array| self.grid(sheet, Some(array)))
            .collect::<Result<Vec<_>, _>>()?;
        let rows = grids.iter().map(Vec::len).max().unwrap_or(0);
        let columns = grids
            .iter()
            .map(|grid| grid.first().map_or(0, Vec::len))
            .max()
            .unwrap_or(0);
        if rows * columns > MAX_SPILL_CELLS {
            return Err(ErrorKind::Spill);
        }
        let mut result = Vec::with_capacity(rows);
        for row in 0..rows {
            let mut line = Vec::with_capacity(columns);
            for column in 0..columns {
                let values = grids
                    .iter()
                    .map(|grid| {
                        Some(Operand::Value(
                            grid.get(row)
                                .and_then(|line| line.get(column))
                                .cloned()
                                .unwrap_or(Value::Error(ErrorKind::NotAvailable)),
                        ))
                    })
                    .collect();
                line.push(self.single(self.invoke(sheet, &closure, values)));
            }
            result.push(line);
        }
        Ok(Operand::Array(result))
    }

    fn reduce(
        &self,
        sheet: usize,
        arguments: &[Expr],
        keep_steps: bool,
    ) -> Result<Operand, ErrorKind> {
        let [initial, array, function] = arguments else {
            return Err(ErrorKind::Value);
        };
        let closure = self.closure(sheet, Some(function))?;
        let grid = self.grid(sheet, Some(array))?;
        let mut accumulator = match initial {
            Expr::Text(text) if text.is_empty() => Operand::Value(Value::Empty),
            initial => self.evaluate(sheet, initial),
        };
        let mut steps = Vec::with_capacity(grid.len());
        for line in grid {
            let mut row = Vec::with_capacity(line.len());
            for value in line {
                accumulator = self.invoke(
                    sheet,
                    &closure,
                    vec![Some(accumulator), Some(Operand::Value(value))],
                );
                if keep_steps {
                    row.push(self.single(accumulator.clone()));
                }
            }
            steps.push(row);
        }
        Ok(if keep_steps {
            Operand::Array(steps)
        } else {
            accumulator
        })
    }

    fn by_line(&self, sheet: usize, arguments: &[Expr], rows: bool) -> Result<Operand, ErrorKind> {
        let [array, function] = arguments else {
            return Err(ErrorKind::Value);
        };
        let closure = self.closure(sheet, Some(function))?;
        let grid = self.grid(sheet, Some(array))?;
        let lines: Vec<Vec<Vec<Value>>> = if rows {
            grid.into_iter().map(|line| vec![line]).collect()
        } else {
            let columns = grid.first().map_or(0, Vec::len);
            (0..columns)
                .map(|column| {
                    grid.iter()
                        .map(|line| vec![line.get(column).cloned().unwrap_or(Value::Empty)])
                        .collect()
                })
                .collect()
        };
        let results: Vec<Value> = lines
            .into_iter()
            .map(|line| self.single(self.invoke(sheet, &closure, vec![Some(Operand::Array(line))])))
            .collect();
        Ok(Operand::Array(if rows {
            results.into_iter().map(|value| vec![value]).collect()
        } else {
            vec![results]
        }))
    }

    fn make_array(&self, sheet: usize, arguments: &[Expr]) -> Result<Operand, ErrorKind> {
        let [rows, columns, function] = arguments else {
            return Err(ErrorKind::Value);
        };
        let count = |argument: &Expr| -> Result<usize, ErrorKind> {
            let number = self.scalar(sheet, argument).as_number()?.trunc();
            #[expect(
                clippy::cast_precision_loss,
                reason = "the spill limit is far below 2^52"
            )]
            let limit = MAX_SPILL_CELLS as f64;
            if !(1.0..=limit).contains(&number) {
                return Err(ErrorKind::Value);
            }
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "checked to be a positive count below the spill limit"
            )]
            let count = number as usize;
            Ok(count)
        };
        let (rows, columns) = (count(rows)?, count(columns)?);
        if rows * columns > MAX_SPILL_CELLS {
            return Err(ErrorKind::Spill);
        }
        let closure = self.closure(sheet, Some(function))?;
        let mut grid = Vec::with_capacity(rows);
        for row in 0..rows {
            let mut line = Vec::with_capacity(columns);
            for column in 0..columns {
                #[expect(clippy::cast_precision_loss, reason = "counts below the spill limit")]
                let place = |index: usize| Some(Operand::Value(Value::Number((index + 1) as f64)));
                line.push(self.single(self.invoke(
                    sheet,
                    &closure,
                    vec![place(row), place(column)],
                )));
            }
            grid.push(line);
        }
        Ok(Operand::Array(grid))
    }
}

#[cfg(test)]
mod tests;
