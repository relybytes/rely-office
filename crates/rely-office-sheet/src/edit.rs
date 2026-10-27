// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Edits that move cells, and the formulas that follow them: inserting and deleting rows and
//! columns, renaming and removing sheets, and copying with relative references.

use std::collections::BTreeMap;

use rely_office_core::CellRange;

use crate::address::{CellRef, MAX_COLUMNS, MAX_ROWS};
use crate::formula::{CellAddress, Expr, RangeAddress};
use crate::model::{EditError, Sheet};
use crate::rules::rewrite_rule_formulas;
use crate::value::{ErrorKind, Input};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shift {
    InsertRows { at: u32, count: u32 },
    DeleteRows { at: u32, count: u32 },
    InsertColumns { at: u32, count: u32 },
    DeleteColumns { at: u32, count: u32 },
}

impl Shift {
    fn rows(self) -> bool {
        matches!(self, Self::InsertRows { .. } | Self::DeleteRows { .. })
    }

    fn limit(self) -> u32 {
        if self.rows() { MAX_ROWS } else { MAX_COLUMNS }
    }

    /// Where an index lands, `None` when it is deleted or pushed off the grid.
    fn index(self, index: u32) -> Option<u32> {
        match self {
            Self::InsertRows { at, count } | Self::InsertColumns { at, count } => {
                if index < at {
                    Some(index)
                } else {
                    index
                        .checked_add(count)
                        .filter(|moved| *moved < self.limit())
                }
            }
            Self::DeleteRows { at, count } | Self::DeleteColumns { at, count } => {
                let end = at.saturating_add(count);
                if index < at {
                    Some(index)
                } else if index < end {
                    None
                } else {
                    Some(index - count)
                }
            }
        }
    }

    /// Where a span `first..=last` lands: an insertion inside it widens it, a deletion shrinks
    /// it, and `None` when all of it is deleted.
    fn span(self, first: u32, last: u32) -> Option<(u32, u32)> {
        match self {
            Self::InsertRows { .. } | Self::InsertColumns { .. } => {
                let first = self.index(first)?;
                let last = self.index(last).unwrap_or(self.limit() - 1);
                Some((first, last))
            }
            Self::DeleteRows { at, count } | Self::DeleteColumns { at, count } => {
                let end = at.saturating_add(count);
                let first = if first >= end {
                    first - count
                } else if first >= at {
                    at
                } else {
                    first
                };
                let last = if last >= end {
                    last - count
                } else if last >= at {
                    at.checked_sub(1)?
                } else {
                    last
                };
                (first <= last).then_some((first, last))
            }
        }
    }

    fn cell(self, cell: CellRef) -> Option<CellRef> {
        if self.rows() {
            Some(CellRef::new(self.index(cell.row)?, cell.column))
        } else {
            Some(CellRef::new(cell.row, self.index(cell.column)?))
        }
    }
}

/// Move what a sheet keeps beside its cells: styles go with their cells, merged ranges and
/// column widths or row heights with their rows and columns; a merge left a single cell ends.
fn move_looks(sheet: &mut Sheet, shift: Shift) {
    sheet.styles = std::mem::take(&mut sheet.styles)
        .into_iter()
        .filter_map(|(cell, style)| shift.cell(cell).map(|cell| (cell, style)))
        .collect();
    sheet.merges = std::mem::take(&mut sheet.merges)
        .into_iter()
        .filter_map(|mut merge| {
            if shift.rows() {
                (merge.first_row, merge.last_row) = shift.span(merge.first_row, merge.last_row)?;
            } else {
                (merge.first_column, merge.last_column) =
                    shift.span(merge.first_column, merge.last_column)?;
            }
            (merge.first_row != merge.last_row || merge.first_column != merge.last_column)
                .then_some(merge)
        })
        .collect();
    for format in &mut sheet.conditional_formats {
        format.ranges = shift_ranges(&format.ranges, shift);
    }
    sheet
        .conditional_formats
        .retain(|format| !format.ranges.is_empty());
    for validation in &mut sheet.validations {
        validation.ranges = shift_ranges(&validation.ranges, shift);
    }
    sheet
        .validations
        .retain(|validation| !validation.ranges.is_empty());
    if shift.rows() {
        sheet.row_heights = std::mem::take(&mut sheet.row_heights)
            .into_iter()
            .filter_map(|(row, height)| shift.index(row).map(|row| (row, height)))
            .collect();
    } else {
        sheet.column_widths = std::mem::take(&mut sheet.column_widths)
            .into_iter()
            .filter_map(|(column, width)| shift.index(column).map(|column| (column, width)))
            .collect();
    }
}

/// Where the ranges of a rule land; a range all deleted is gone.
fn shift_ranges(ranges: &[CellRange], shift: Shift) -> Vec<CellRange> {
    ranges
        .iter()
        .filter_map(|range| {
            let mut range = *range;
            if shift.rows() {
                (range.first_row, range.last_row) = shift.span(range.first_row, range.last_row)?;
            } else {
                (range.first_column, range.last_column) =
                    shift.span(range.first_column, range.last_column)?;
            }
            Some(range)
        })
        .collect()
}

/// A formula of `local` sheet, or of another, after the cells of the sheet named `target` move.
fn shifted_expression(expression: &Expr, local: bool, target: &str, shift: Shift) -> Expr {
    let names_target = |sheet: &Option<String>| match sheet {
        None => local,
        Some(name) => name.to_lowercase() == target,
    };
    transform(expression, &mut |node| match node {
        Expr::Cell(address) if names_target(&address.sheet) => Some(
            shift
                .cell(address.cell)
                .map_or(Expr::Error(ErrorKind::Ref), |cell| {
                    Expr::Cell(CellAddress {
                        cell,
                        ..address.clone()
                    })
                }),
        ),
        Expr::Range(range) if names_target(&range.sheet) => Some(shift_range(range, shift)),
        _ => None,
    })
}

/// Move the cells of a sheet and make every formula of the workbook follow them.
pub(crate) fn shift(sheets: &mut [Sheet], target: usize, shift: Shift) -> Result<(), EditError> {
    if let Shift::InsertRows { count, .. } | Shift::InsertColumns { count, .. } = shift {
        if count == 0 {
            return Ok(());
        }
        // Content pushed off the grid would be lost: refuse instead.
        if sheets[target]
            .cells
            .keys()
            .any(|cell| shift.cell(*cell).is_none())
        {
            return Err(EditError::OutOfBounds);
        }
    }
    let moved: BTreeMap<CellRef, _> = std::mem::take(&mut sheets[target].cells)
        .into_iter()
        .filter_map(|(cell, content)| shift.cell(cell).map(|cell| (cell, content)))
        .collect();
    sheets[target].cells = moved;
    move_looks(&mut sheets[target], shift);

    let target_name = sheets[target].name.to_lowercase();
    for (index, sheet) in sheets.iter_mut().enumerate() {
        let local = index == target;
        let mut moved =
            |expression: &Expr| shifted_expression(expression, local, &target_name, shift);
        for content in sheet.cells.values_mut() {
            if let Input::Formula(expression) = &mut content.input {
                **expression = moved(expression);
            }
        }
        rewrite_rule_formulas(sheet, &mut moved);
    }
    Ok(())
}

fn shift_range(range: &RangeAddress, shift: Shift) -> Expr {
    let (top_left, bottom_right) = range.bounds();
    let ordered = |range: &RangeAddress| {
        let (start, end) = if range.start.cell == top_left {
            (range.start.clone(), range.end.clone())
        } else {
            (range.end.clone(), range.start.clone())
        };
        (start, end)
    };
    let (mut start, mut end) = ordered(range);
    start.cell = top_left;
    end.cell = bottom_right;
    if shift.rows() {
        if range.whole_columns {
            return Expr::Range(range.clone());
        }
        let Some((first, last)) = shift.span(top_left.row, bottom_right.row) else {
            return Expr::Error(ErrorKind::Ref);
        };
        start.cell.row = first;
        end.cell.row = last;
    } else {
        let Some((first, last)) = shift.span(top_left.column, bottom_right.column) else {
            return Expr::Error(ErrorKind::Ref);
        };
        start.cell.column = first;
        end.cell.column = last;
    }
    Expr::Range(RangeAddress {
        sheet: range.sheet.clone(),
        start,
        end,
        whole_columns: range.whole_columns,
    })
}

/// Formulas that name `old` name `new` instead.
pub(crate) fn rename_references(sheets: &mut [Sheet], old: &str, new: &str) {
    let old = old.to_lowercase();
    rewrite_named(sheets, &old, &mut |sheet| {
        sheet.as_ref().map(|_| new.to_owned())
    });
}

/// References to a removed sheet become `#REF!`.
pub(crate) fn break_references_to_sheet(sheets: &mut [Sheet], name: &str) {
    let name = name.to_lowercase();
    let named = |sheet: &Option<String>| {
        sheet
            .as_ref()
            .is_some_and(|sheet| sheet.to_lowercase() == name)
    };
    let mut broken = |expression: &Expr| {
        transform(expression, &mut |node| match node {
            Expr::Cell(address) if named(&address.sheet) => Some(Expr::Error(ErrorKind::Ref)),
            Expr::Range(range) if named(&range.sheet) => Some(Expr::Error(ErrorKind::Ref)),
            _ => None,
        })
    };
    for sheet in sheets.iter_mut() {
        for content in sheet.cells.values_mut() {
            if let Input::Formula(expression) = &mut content.input {
                **expression = broken(expression);
            }
        }
        rewrite_rule_formulas(sheet, &mut broken);
    }
}

fn rewrite_named(
    sheets: &mut [Sheet],
    old: &str,
    rename: &mut dyn FnMut(&Option<String>) -> Option<String>,
) {
    let named = |sheet: &Option<String>| {
        sheet
            .as_ref()
            .is_some_and(|sheet| sheet.to_lowercase() == old)
    };
    let mut renamed = |expression: &Expr| {
        transform(expression, &mut |node| match node {
            Expr::Cell(address) if named(&address.sheet) => Some(Expr::Cell(CellAddress {
                sheet: rename(&address.sheet),
                ..address.clone()
            })),
            Expr::Range(range) if named(&range.sheet) => {
                let sheet = rename(&range.sheet);
                Some(Expr::Range(RangeAddress {
                    start: CellAddress {
                        sheet: sheet.clone(),
                        ..range.start.clone()
                    },
                    end: CellAddress {
                        sheet: sheet.clone(),
                        ..range.end.clone()
                    },
                    sheet,
                    whole_columns: range.whole_columns,
                }))
            }
            _ => None,
        })
    };
    for sheet in sheets.iter_mut() {
        for content in sheet.cells.values_mut() {
            if let Input::Formula(expression) = &mut content.input {
                **expression = renamed(expression);
            }
        }
        rewrite_rule_formulas(sheet, &mut renamed);
    }
}

/// A formula moved by an offset, as a paste moves it: relative rows and columns follow, absolute
/// ones stay, and a reference pushed off the grid becomes `#REF!`.
pub(crate) fn offset_relative(expression: &Expr, rows: i64, columns: i64) -> Expr {
    let moved = |address: &CellAddress, whole_columns: bool| -> Option<CellAddress> {
        let row = if address.absolute_row || whole_columns {
            i64::from(address.cell.row)
        } else {
            i64::from(address.cell.row) + rows
        };
        let column = if address.absolute_column {
            i64::from(address.cell.column)
        } else {
            i64::from(address.cell.column) + columns
        };
        let row = u32::try_from(row).ok().filter(|row| *row < MAX_ROWS)?;
        let column = u32::try_from(column)
            .ok()
            .filter(|column| *column < MAX_COLUMNS)?;
        Some(CellAddress {
            cell: CellRef::new(row, column),
            ..address.clone()
        })
    };
    transform(expression, &mut |node| match node {
        Expr::Cell(address) => {
            Some(moved(address, false).map_or(Expr::Error(ErrorKind::Ref), Expr::Cell))
        }
        Expr::Range(range) => Some(
            match (
                moved(&range.start, range.whole_columns),
                moved(&range.end, range.whole_columns),
            ) {
                (Some(start), Some(end)) => Expr::Range(RangeAddress {
                    start,
                    end,
                    ..range.clone()
                }),
                _ => Expr::Error(ErrorKind::Ref),
            },
        ),
        _ => None,
    })
}

/// A copy of an expression where `replace` may substitute any node; the others are copied with
/// their children transformed.
fn transform(expression: &Expr, replace: &mut dyn FnMut(&Expr) -> Option<Expr>) -> Expr {
    if let Some(replaced) = replace(expression) {
        return replaced;
    }
    match expression {
        Expr::Unary(operator, operand) => {
            Expr::Unary(*operator, Box::new(transform(operand, replace)))
        }
        Expr::Percent(operand) => Expr::Percent(Box::new(transform(operand, replace))),
        Expr::Group(operand) => Expr::Group(Box::new(transform(operand, replace))),
        Expr::Binary(operator, left, right) => Expr::Binary(
            *operator,
            Box::new(transform(left, replace)),
            Box::new(transform(right, replace)),
        ),
        Expr::Call(name, arguments) => Expr::Call(
            name.clone(),
            arguments
                .iter()
                .map(|argument| transform(argument, replace))
                .collect(),
        ),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_indexes_and_spans() {
        let insert = Shift::InsertRows { at: 2, count: 3 };
        assert_eq!(insert.index(1), Some(1));
        assert_eq!(insert.index(2), Some(5));
        assert_eq!(insert.index(MAX_ROWS - 1), None);
        assert_eq!(insert.span(0, 4), Some((0, 7)));
        assert_eq!(insert.span(2, 4), Some((5, 7)));

        let delete = Shift::DeleteColumns { at: 2, count: 3 };
        assert_eq!(delete.index(1), Some(1));
        assert_eq!(delete.index(3), None);
        assert_eq!(delete.index(5), Some(2));
        assert_eq!(delete.span(0, 9), Some((0, 6)));
        assert_eq!(delete.span(3, 9), Some((2, 6)));
        assert_eq!(delete.span(0, 3), Some((0, 1)));
        assert_eq!(delete.span(2, 4), None);
        assert_eq!(Shift::DeleteRows { at: 0, count: 2 }.span(0, 1), None);
    }
}
