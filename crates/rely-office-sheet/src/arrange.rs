// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! How a sheet is arranged for reading rather than what it holds: the rows and columns frozen
//! at the top and the left, the order of a range's rows, and the filter that hides the rows a
//! reader does not want to see.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use rely_office_core::{AutoFilterLayout, CellRange};

use crate::address::{CellRef, MAX_COLUMNS, MAX_ROWS};
use crate::model::{Cell, EditError, Sheet, Workbook};
use crate::value::{Input, Locale, Value};

/// The most keys one sort takes, as Excel's dialog offers.
pub const MAX_SORT_KEYS: usize = 64;
/// The most distinct values a filter column offers to choose from.
pub const MAX_FILTER_CHOICES: usize = 10_000;

/// A column a range is sorted by, counted from 0 on the sheet, and its direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortKey {
    pub column: u32,
    pub descending: bool,
}

/// The filter buttons of a sheet: the range whose first row heads it, the values each filtered
/// column shows, and the rows it hides with the height each had before.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AutoFilter {
    pub range: CellRange,
    /// The values shown, by column counted from the first of the range; an empty text stands
    /// for the blank cells. The values are cells as English shows them, which is what a file
    /// stores.
    pub(crate) columns: BTreeMap<u32, BTreeSet<String>>,
    pub(crate) hidden: BTreeMap<u32, Option<f64>>,
}

impl AutoFilter {
    /// The values a column shows, `None` when the column is not filtered.
    #[must_use]
    pub fn shown(&self, column: u32) -> Option<&BTreeSet<String>> {
        self.columns.get(&column)
    }

    /// The columns filtered, counted from the first of the range.
    pub fn filtered_columns(&self) -> impl Iterator<Item = u32> + '_ {
        self.columns.keys().copied()
    }

    /// The rows the filter hides.
    pub fn hidden_rows(&self) -> impl Iterator<Item = u32> + '_ {
        self.hidden.keys().copied()
    }

    /// The filter of a file: the rows already hidden below the heading of a filtered range are
    /// the filter's.
    pub(crate) fn from_layout(layout: &AutoFilterLayout, row_heights: &BTreeMap<u32, f64>) -> Self {
        let columns: BTreeMap<u32, BTreeSet<String>> = layout
            .columns
            .iter()
            .map(|(column, shown)| (*column, shown.iter().cloned().collect()))
            .collect();
        let hidden = if columns.is_empty() {
            BTreeMap::new()
        } else {
            row_heights
                .range(layout.range.first_row + 1..=layout.range.last_row)
                .filter(|(_, height)| **height == 0.0)
                .map(|(row, _)| (*row, None))
                .collect()
        };
        Self {
            range: layout.range,
            columns,
            hidden,
        }
    }
}

/// The order of two values as a sort puts them in ascending order: numbers, then text without
/// regard to case, then booleans, then errors.
pub(crate) fn compare_values(first: &Value, second: &Value) -> Ordering {
    let class = |value: &Value| match value {
        Value::Number(_) => 0,
        Value::Text(_) => 1,
        Value::Bool(_) => 2,
        Value::Error(_) => 3,
        Value::Empty => 4,
    };
    match (first, second) {
        (Value::Number(first), Value::Number(second)) => first.total_cmp(second),
        (Value::Text(first), Value::Text(second)) => first
            .to_lowercase()
            .cmp(&second.to_lowercase())
            .then_with(|| first.cmp(second)),
        (Value::Bool(first), Value::Bool(second)) => first.cmp(second),
        _ => class(first).cmp(&class(second)),
    }
}

/// The order of two rows by the keys; blank cells go last whichever the direction.
fn compare_rows(first: &[Value], second: &[Value], keys: &[SortKey]) -> Ordering {
    for (index, key) in keys.iter().enumerate() {
        let (first, second) = (&first[index], &second[index]);
        let order = match (first, second) {
            (Value::Empty, Value::Empty) => Ordering::Equal,
            (Value::Empty, _) => Ordering::Greater,
            (_, Value::Empty) => Ordering::Less,
            _ if key.descending => compare_values(second, first),
            _ => compare_values(first, second),
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    Ordering::Equal
}

fn corners(from: (CellRef, CellRef)) -> CellRange {
    let (start, end) = from;
    CellRange {
        first_row: start.row.min(end.row),
        first_column: start.column.min(end.column),
        last_row: start.row.max(end.row),
        last_column: start.column.max(end.column),
    }
}

/// Put back the rows a filter hid, at the heights they had.
fn show_hidden_rows(sheet: &mut Sheet) {
    let Some(filter) = sheet.auto_filter.as_mut() else {
        return;
    };
    for (row, height) in std::mem::take(&mut filter.hidden) {
        match height {
            Some(height) => sheet.row_heights.insert(row, height),
            None => sheet.row_heights.remove(&row),
        };
    }
}

impl Workbook {
    /// Freeze the first `rows` rows and `columns` columns of a sheet, so they stay in view as
    /// the rest scrolls; zero and zero unfreezes.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::OutOfBounds`] past the grid.
    pub fn set_frozen(&mut self, sheet: usize, rows: u32, columns: u32) -> Result<(), EditError> {
        if rows >= MAX_ROWS || columns >= MAX_COLUMNS {
            return Err(EditError::OutOfBounds);
        }
        let target = self
            .sheets
            .get_mut(sheet)
            .ok_or(EditError::NoSuchSheet(sheet))?;
        target.frozen_rows = rows;
        target.frozen_columns = columns;
        Ok(())
    }

    /// Sort the rows of a rectangle by its columns, each key a column of the sheet within the
    /// rectangle. The cells move with their look; a formula that moves keeps pointing where its
    /// relative references point from its new place, as Excel moves it.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::OutOfBounds`] for a key outside the rectangle,
    /// no key or more than [`MAX_SORT_KEYS`].
    pub fn sort_range(
        &mut self,
        sheet: usize,
        from: (CellRef, CellRef),
        keys: &[SortKey],
        has_header: bool,
    ) -> Result<(), EditError> {
        let range = corners(from);
        if self.sheets.get(sheet).is_none() {
            return Err(EditError::NoSuchSheet(sheet));
        }
        if keys.is_empty()
            || keys.len() > MAX_SORT_KEYS
            || range.last_row >= MAX_ROWS
            || range.last_column >= MAX_COLUMNS
            || keys
                .iter()
                .any(|key| !(range.first_column..=range.last_column).contains(&key.column))
        {
            return Err(EditError::OutOfBounds);
        }
        let first = range.first_row + u32::from(has_header);
        if first >= range.last_row {
            return Ok(());
        }
        // Only the rows with something in them take part; the empty ones keep the bottom.
        let source = &self.sheets[sheet];
        let mut rows: BTreeSet<u32> = BTreeSet::new();
        for row in first..=range.last_row {
            let start = CellRef::new(row, range.first_column);
            let end = CellRef::new(row, range.last_column);
            if source.cells.range(start..=end).next().is_some()
                || source.styles.range(start..=end).next().is_some()
            {
                rows.insert(row);
            }
        }
        let mut order: Vec<(u32, Vec<Value>)> = rows
            .iter()
            .map(|row| {
                let values = keys
                    .iter()
                    .map(|key| self.value(sheet, CellRef::new(*row, key.column)))
                    .collect();
                (*row, values)
            })
            .collect();
        order.sort_by(|(_, first), (_, second)| compare_rows(first, second, keys));

        let target = &mut self.sheets[sheet];
        let mut moved_cells: Vec<(CellRef, Cell)> = Vec::new();
        let mut moved_styles: Vec<(CellRef, u32)> = Vec::new();
        for (place, (row, _)) in order.iter().enumerate() {
            let new_row = first + u32::try_from(place).map_err(|_| EditError::OutOfBounds)?;
            let offset = i64::from(new_row) - i64::from(*row);
            let start = CellRef::new(*row, range.first_column);
            let end = CellRef::new(*row, range.last_column);
            for (cell, content) in target.cells.range(start..=end) {
                let input = match &content.input {
                    Input::Formula(expression) if offset != 0 => Input::Formula(Box::new(
                        crate::edit::offset_relative(expression, offset, 0),
                    )),
                    other => other.clone(),
                };
                moved_cells.push((
                    CellRef::new(new_row, cell.column),
                    Cell {
                        input,
                        format: content.format,
                    },
                ));
            }
            for (cell, style) in target.styles.range(start..=end) {
                moved_styles.push((CellRef::new(new_row, cell.column), *style));
            }
        }
        let inside = |cell: &CellRef| {
            (first..=range.last_row).contains(&cell.row)
                && (range.first_column..=range.last_column).contains(&cell.column)
        };
        target.cells.retain(|cell, _| !inside(cell));
        target.styles.retain(|cell, _| !inside(cell));
        target.cells.extend(moved_cells);
        target.styles.extend(moved_styles);
        self.recalculate();
        Ok(())
    }

    /// Put filter buttons on the first row of a rectangle, or take them off a sheet with
    /// `None`; either way the rows an earlier filter hid come back.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::OutOfBounds`] past the grid.
    pub fn set_auto_filter(
        &mut self,
        sheet: usize,
        from: Option<(CellRef, CellRef)>,
    ) -> Result<(), EditError> {
        let range = from.map(corners);
        if range.is_some_and(|range| range.last_row >= MAX_ROWS || range.last_column >= MAX_COLUMNS)
        {
            return Err(EditError::OutOfBounds);
        }
        let target = self
            .sheets
            .get_mut(sheet)
            .ok_or(EditError::NoSuchSheet(sheet))?;
        show_hidden_rows(target);
        target.auto_filter = range.map(|range| AutoFilter {
            range,
            ..AutoFilter::default()
        });
        Ok(())
    }

    /// Choose the values a column of the filter shows, keys as [`Workbook::filter_choices`]
    /// gives them; `None` shows the whole column. The rows are filtered again.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::OutOfBounds`] for a sheet without a filter or a
    /// column outside it.
    pub fn set_filter_column(
        &mut self,
        sheet: usize,
        column: u32,
        shown: Option<Vec<String>>,
    ) -> Result<(), EditError> {
        let target = self
            .sheets
            .get_mut(sheet)
            .ok_or(EditError::NoSuchSheet(sheet))?;
        let Some(filter) = target.auto_filter.as_mut() else {
            return Err(EditError::OutOfBounds);
        };
        if column > filter.range.last_column - filter.range.first_column {
            return Err(EditError::OutOfBounds);
        }
        match shown {
            Some(shown) => {
                filter
                    .columns
                    .insert(column, shown.into_iter().take(MAX_FILTER_CHOICES).collect());
            }
            None => {
                filter.columns.remove(&column);
            }
        }
        self.apply_filter(sheet);
        Ok(())
    }

    /// The values a column of the filter can show, in sort order, each as a key and as the
    /// locale shows it: `("1234.5", "1.234,5")`. An empty key stands for the blank cells.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::OutOfBounds`] for a sheet without a filter or a
    /// column outside it.
    pub fn filter_choices(
        &self,
        sheet: usize,
        column: u32,
        locale: Locale,
    ) -> Result<Vec<(String, String)>, EditError> {
        let target = self
            .sheets
            .get(sheet)
            .ok_or(EditError::NoSuchSheet(sheet))?;
        let filter = target.auto_filter.as_ref().ok_or(EditError::OutOfBounds)?;
        if column > filter.range.last_column - filter.range.first_column {
            return Err(EditError::OutOfBounds);
        }
        let absolute = filter.range.first_column + column;
        let mut seen = BTreeSet::new();
        let mut choices: Vec<(Value, String, String)> = Vec::new();
        // A filter over whole columns stops at the sheet's last row with content.
        let last = filter
            .range
            .last_row
            .min(target.extent().0.saturating_sub(1));
        for row in filter.range.first_row + 1..=last {
            let cell = CellRef::new(row, absolute);
            let key = self.display(sheet, cell, Locale::English);
            if seen.insert(key.clone()) {
                choices.push((
                    self.value(sheet, cell),
                    key,
                    self.display(sheet, cell, locale),
                ));
                if choices.len() >= MAX_FILTER_CHOICES {
                    break;
                }
            }
        }
        choices.sort_by(|first, second| {
            compare_rows(
                std::slice::from_ref(&first.0),
                std::slice::from_ref(&second.0),
                &[SortKey {
                    column: 0,
                    descending: false,
                }],
            )
        });
        Ok(choices
            .into_iter()
            .map(|(_, key, label)| (key, label))
            .collect())
    }

    /// Hide the rows of the filter's range whose cells a filtered column does not show.
    pub(crate) fn apply_filter(&mut self, sheet: usize) {
        show_hidden_rows(&mut self.sheets[sheet]);
        let Some(filter) = self.sheets[sheet].auto_filter.clone() else {
            return;
        };
        if filter.columns.is_empty() {
            return;
        }
        let last = filter
            .range
            .last_row
            .min(self.sheets[sheet].extent().0.saturating_sub(1));
        let mut hidden = BTreeMap::new();
        for row in filter.range.first_row + 1..=last {
            let shows = filter.columns.iter().all(|(column, shown)| {
                let cell = CellRef::new(row, filter.range.first_column + column);
                shown.contains(&self.display(sheet, cell, Locale::English))
            });
            if !shows {
                hidden.insert(row, self.sheets[sheet].row_heights.get(&row).copied());
            }
        }
        let target = &mut self.sheets[sheet];
        for row in hidden.keys() {
            target.row_heights.insert(*row, 0.0);
        }
        if let Some(filter) = target.auto_filter.as_mut() {
            filter.hidden = hidden;
        }
    }
}

#[cfg(test)]
mod tests;
