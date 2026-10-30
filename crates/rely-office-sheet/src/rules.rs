// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! The conditional formats and data validations of a sheet as an editor keeps them: set as a
//! whole, their formulas checked, and a cell's value checked against its validation. Their
//! ranges move with rows and columns and their formulas follow the cells they read, in
//! [`crate::edit`].

use rely_office_core::{
    CellRange, ComparisonOperator, Condition, ConditionalFormat, DataValidation, ScaleValue,
    ValidationKind,
};

use crate::address::{CellRef, MAX_COLUMNS, MAX_ROWS};
use crate::edit::offset_relative;
use crate::evaluate::{FormulaContext, holds};
use crate::formula::{BinaryOperator, Expr, parse_formula, print_formula};
use crate::model::{EditError, Sheet, Workbook};
use crate::value::Value;

/// The most conditional formats, and the most validations, a sheet keeps.
pub const MAX_RULES: usize = 10_000;
/// The most ranges one format or validation covers, and the most rules a format holds.
const MAX_RANGES: usize = 1_000;
/// The most items a list validation offers.
const MAX_LIST_ITEMS: usize = 10_000;

/// Every formula of the sheet's rules, as text.
fn rule_formulas(sheet: &mut Sheet) -> Vec<&mut String> {
    let mut found = Vec::new();
    for format in &mut sheet.conditional_formats {
        for rule in &mut format.rules {
            match &mut rule.condition {
                Condition::CellIs { formulas, .. } => found.extend(formulas.iter_mut()),
                Condition::Expression { formula } => found.push(formula),
                Condition::ColorScale { points } => {
                    found.extend(
                        points
                            .iter_mut()
                            .filter_map(|point| scale_formula(&mut point.value)),
                    );
                }
                Condition::DataBar { min, max, .. } => {
                    found.extend(scale_formula(min));
                    found.extend(scale_formula(max));
                }
                Condition::IconSet { thresholds, .. } => {
                    found.extend(thresholds.iter_mut().filter_map(scale_formula));
                }
                _ => {}
            }
        }
    }
    for validation in &mut sheet.validations {
        found.extend(validation.formulas.iter_mut());
    }
    found
}

fn scale_formula(value: &mut ScaleValue) -> Option<&mut String> {
    match value {
        ScaleValue::Formula(formula) => Some(formula),
        _ => None,
    }
}

/// Rewrite every formula of the sheet's rules; a formula the rewrite leaves alone keeps its
/// text as the file had it, and one that does not parse is kept as it is.
pub(crate) fn rewrite_rule_formulas(sheet: &mut Sheet, rewrite: &mut dyn FnMut(&Expr) -> Expr) {
    for formula in rule_formulas(sheet) {
        if let Ok(expression) = parse_formula(formula) {
            let rewritten = rewrite(&expression);
            if rewritten != expression {
                *formula = print_formula(&rewritten);
            }
        }
    }
}

/// The first cell of the first range, which the relative references of a rule's formulas are
/// written for.
pub(crate) fn anchor_of(ranges: &[CellRange]) -> CellRef {
    ranges.first().map_or(CellRef::new(0, 0), |range| {
        CellRef::new(range.first_row, range.first_column)
    })
}

/// A rule's formula as it reads in `cell`: its relative references moved from the anchor.
pub(crate) fn formula_in(formula: &str, anchor: CellRef, cell: CellRef) -> Option<Expr> {
    let expression = parse_formula(formula).ok()?;
    Some(offset_relative(
        &expression,
        i64::from(cell.row) - i64::from(anchor.row),
        i64::from(cell.column) - i64::from(anchor.column),
    ))
}

fn check_ranges(ranges: &[CellRange]) -> Result<(), EditError> {
    if ranges.is_empty()
        || ranges.len() > MAX_RANGES
        || ranges.iter().any(|range| {
            range.first_row > range.last_row
                || range.first_column > range.last_column
                || range.last_row >= MAX_ROWS
                || range.last_column >= MAX_COLUMNS
        })
    {
        return Err(EditError::OutOfBounds);
    }
    Ok(())
}

impl Workbook {
    /// Replace the conditional formats of a sheet.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::OutOfBounds`] for a format with no range, a
    /// range past the grid, no rule or more than [`MAX_RULES`] formats, and
    /// [`EditError::Formula`] for a formula that does not parse.
    pub fn set_conditional_formats(
        &mut self,
        sheet: usize,
        formats: Vec<ConditionalFormat>,
    ) -> Result<(), EditError> {
        if sheet >= self.sheets.len() {
            return Err(EditError::NoSuchSheet(sheet));
        }
        if formats.len() > MAX_RULES {
            return Err(EditError::OutOfBounds);
        }
        for format in &formats {
            check_ranges(&format.ranges)?;
            if format.rules.is_empty() || format.rules.len() > MAX_RANGES {
                return Err(EditError::OutOfBounds);
            }
        }
        let mut candidate = Sheet {
            conditional_formats: formats,
            ..Sheet::default()
        };
        for formula in rule_formulas(&mut candidate) {
            parse_formula(formula)?;
        }
        self.sheets[sheet].conditional_formats = candidate.conditional_formats;
        Ok(())
    }

    /// Replace the data validations of a sheet.
    ///
    /// # Errors
    ///
    /// As [`Workbook::set_conditional_formats`], and [`EditError::OutOfBounds`] for a
    /// validation with more formulas than its operator compares with.
    pub fn set_validations(
        &mut self,
        sheet: usize,
        validations: Vec<DataValidation>,
    ) -> Result<(), EditError> {
        if sheet >= self.sheets.len() {
            return Err(EditError::NoSuchSheet(sheet));
        }
        if validations.len() > MAX_RULES {
            return Err(EditError::OutOfBounds);
        }
        for validation in &validations {
            check_ranges(&validation.ranges)?;
            if validation.formulas.len() > 2 {
                return Err(EditError::OutOfBounds);
            }
            for formula in &validation.formulas {
                parse_formula(formula)?;
            }
        }
        self.sheets[sheet].validations = validations;
        Ok(())
    }

    /// The validation of a cell: the last one listed whose ranges hold it.
    #[must_use]
    pub fn validation_at(&self, sheet: usize, cell: CellRef) -> Option<&DataValidation> {
        self.sheets
            .get(sheet)?
            .validations
            .iter()
            .rev()
            .find(|validation| {
                validation
                    .ranges
                    .iter()
                    .any(|range| range.contains(cell.row, cell.column))
            })
    }

    /// Whether a cell's value is one its validation accepts; a cell with none, an empty cell
    /// and a validation of any value always are. An editor sets the input, asks, and takes it
    /// back when the validation's [`rely_office_core::ErrorStyle`] is `Stop`.
    #[must_use]
    pub fn is_valid(&self, sheet: usize, cell: CellRef) -> bool {
        let Some(validation) = self.validation_at(sheet, cell) else {
            return true;
        };
        let context = FormulaContext::new(self);
        accepts(self, &context, sheet, cell, validation)
    }

    /// The cells with content whose value their validation refuses, row by row.
    #[must_use]
    pub fn invalid_cells(&self, sheet: usize) -> Vec<CellRef> {
        let Some(target) = self.sheets.get(sheet) else {
            return Vec::new();
        };
        if target.validations.is_empty() {
            return Vec::new();
        }
        let context = FormulaContext::new(self);
        target
            .cells
            .keys()
            .copied()
            .filter(|cell| {
                self.validation_at(sheet, *cell)
                    .is_some_and(|validation| !accepts(self, &context, sheet, *cell, validation))
            })
            .collect()
    }

    /// The items a list validation offers a cell, as text: the quoted list's, or the cells of
    /// the range it names; none for a cell without a list.
    #[must_use]
    pub fn validation_items(&self, sheet: usize, cell: CellRef) -> Vec<String> {
        let Some(validation) = self
            .validation_at(sheet, cell)
            .filter(|validation| validation.kind == ValidationKind::List)
        else {
            return Vec::new();
        };
        let context = FormulaContext::new(self);
        list_items(&context, sheet, cell, validation)
    }
}

fn list_items(
    context: &FormulaContext<'_>,
    sheet: usize,
    cell: CellRef,
    validation: &DataValidation,
) -> Vec<String> {
    let Some(formula) = validation.formulas.first() else {
        return Vec::new();
    };
    let trimmed = formula.trim();
    if let Some(quoted) = trimmed
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    {
        return quoted
            .replace("\"\"", "\"")
            .split(',')
            .map(|item| item.trim().to_owned())
            .filter(|item| !item.is_empty())
            .take(MAX_LIST_ITEMS)
            .collect();
    }
    let anchor = crate::rules::anchor_of(&validation.ranges);
    let Some(expression) = formula_in(formula, anchor, cell) else {
        return Vec::new();
    };
    context
        .values(sheet, cell, &expression)
        .into_iter()
        .filter_map(|value| value.as_text().ok().filter(|text| !text.is_empty()))
        .take(MAX_LIST_ITEMS)
        .collect()
}

fn accepts(
    workbook: &Workbook,
    context: &FormulaContext<'_>,
    sheet: usize,
    cell: CellRef,
    validation: &DataValidation,
) -> bool {
    let value = workbook.value(sheet, cell);
    if value == Value::Empty || validation.kind == ValidationKind::Any {
        return true;
    }
    let anchor = anchor_of(&validation.ranges);
    let operand = |index: usize| -> Option<Value> {
        let expression = formula_in(validation.formulas.get(index)?, anchor, cell)?;
        Some(context.value(sheet, cell, &expression))
    };
    match validation.kind {
        ValidationKind::Any => true,
        ValidationKind::Custom => operand(0).is_some_and(|value| truthy(&value)),
        ValidationKind::List => {
            let items = list_items(context, sheet, cell, validation);
            let shown = value.as_text().unwrap_or_default();
            items.iter().any(|item| {
                item.eq_ignore_ascii_case(&shown)
                    || matches!((item.parse::<f64>(), &value), (Ok(number), Value::Number(own)) if number.total_cmp(own).is_eq())
            })
        }
        ValidationKind::TextLength => {
            let length = value.as_text().unwrap_or_default().chars().count();
            #[expect(
                clippy::cast_precision_loss,
                reason = "a text holds far fewer characters"
            )]
            let length = Value::Number(length as f64);
            compares(validation.operator, &length, operand(0), operand(1))
        }
        ValidationKind::Whole
        | ValidationKind::Decimal
        | ValidationKind::Date
        | ValidationKind::Time => {
            let Value::Number(number) = value else {
                return false;
            };
            if validation.kind == ValidationKind::Whole && number.fract() != 0.0 {
                return false;
            }
            compares(validation.operator, &value, operand(0), operand(1))
        }
    }
}

/// Whether a value compares with one operand, or lies between two in either order.
pub(crate) fn compares(
    operator: ComparisonOperator,
    value: &Value,
    first: Option<Value>,
    second: Option<Value>,
) -> bool {
    let Some(first) = first else {
        return false;
    };
    let between = |first: &Value, second: &Value| {
        let (low, high) = if holds(BinaryOperator::LessOrEqual, first, second) {
            (first, second)
        } else {
            (second, first)
        };
        holds(BinaryOperator::GreaterOrEqual, value, low)
            && holds(BinaryOperator::LessOrEqual, value, high)
    };
    match operator {
        ComparisonOperator::Between => second.is_some_and(|second| between(&first, &second)),
        ComparisonOperator::NotBetween => second.is_some_and(|second| !between(&first, &second)),
        ComparisonOperator::Equal => holds(BinaryOperator::Equal, value, &first),
        ComparisonOperator::NotEqual => holds(BinaryOperator::NotEqual, value, &first),
        ComparisonOperator::Greater => holds(BinaryOperator::Greater, value, &first),
        ComparisonOperator::Less => holds(BinaryOperator::Less, value, &first),
        ComparisonOperator::GreaterOrEqual => holds(BinaryOperator::GreaterOrEqual, value, &first),
        ComparisonOperator::LessOrEqual => holds(BinaryOperator::LessOrEqual, value, &first),
    }
}

/// Whether a value counts as true where a condition is expected: true, or a number not 0.
pub(crate) fn truthy(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        Value::Number(number) => *number != 0.0,
        _ => false,
    }
}

#[cfg(test)]
mod tests;
