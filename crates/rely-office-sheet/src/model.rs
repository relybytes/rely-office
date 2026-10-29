// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The workbook an editor changes: sheets of cells, each with what was typed and how its value
//! is shown, and the values computed from them.

use std::collections::{BTreeMap, HashMap};

use rely_office_core::{
    CarriedDrawing, CarriedPart, CarriedPivot, CellRange, CellStyle, ChartValue, ConditionalFormat,
    DataValidation, Limits, SheetDrawing, SheetLayout, chart_formula_range, redraw_carried_drawing,
};

use crate::arrange::AutoFilter;

use crate::address::{CellRef, MAX_COLUMNS, MAX_ROWS};
use crate::evaluate;
use crate::formula::{Expr, FormulaError};
use crate::value::{Input, Locale, Value};

/// The most sheets a workbook may hold.
pub const MAX_SHEETS: usize = 256;
/// The most cells with content a workbook may hold, over all its sheets.
pub const MAX_CELLS: usize = 2_000_000;

/// How a number is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NumberFormat {
    #[default]
    General,
    /// A date: `27/10/2026`.
    Date,
    /// A date with its time: `27/10/2026 14:30`.
    DateTime,
    /// A share: `12,5%`.
    Percent,
    /// A fixed number of decimals, up to 10.
    Decimals(u8),
}

/// One cell with content.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub input: Input,
    pub format: NumberFormat,
}

/// A sheet of a workbook.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sheet {
    pub name: String,
    pub(crate) cells: BTreeMap<CellRef, Cell>,
    /// Column widths in characters, as files store them; a column not listed has the default.
    pub column_widths: BTreeMap<u32, f64>,
    /// Row heights in points, 0 for a hidden row; a row not listed fits its content.
    pub row_heights: BTreeMap<u32, f64>,
    /// The style of each cell that has one other than the default, an index into
    /// [`Workbook::styles`].
    pub(crate) styles: BTreeMap<CellRef, u32>,
    pub(crate) merges: Vec<CellRange>,
    /// The pictures and charts of the file the sheet was read from, written back as they were:
    /// they stay anchored to the cells they had, whatever rows or columns move.
    pub(crate) drawing: Option<CarriedDrawing>,
    pub(crate) conditional_formats: Vec<ConditionalFormat>,
    pub(crate) validations: Vec<DataValidation>,
    /// The pivot tables of the file the sheet was read from, written back as they were: the
    /// cells they show are the sheet's own, and a refresh in Excel rebuilds them.
    pub(crate) pivots: Vec<CarriedPivot>,
    /// The rows and columns frozen at the top and the left, 0 when none are.
    pub frozen_rows: u32,
    pub frozen_columns: u32,
    pub(crate) auto_filter: Option<AutoFilter>,
    /// The pictures and charts of the file, drawn as it was opened, for an editor to show.
    pub(crate) pictures: Vec<SheetDrawing>,
    pub(crate) comments: BTreeMap<CellRef, Comment>,
}

impl Sheet {
    /// The comments of the sheet's cells, by cell.
    #[must_use]
    pub fn comments(&self) -> &BTreeMap<CellRef, Comment> {
        &self.comments
    }

    /// The comment on a cell, if it has one.
    #[must_use]
    pub fn comment(&self, cell: CellRef) -> Option<&Comment> {
        self.comments.get(&cell)
    }

    /// The conditional formats of the sheet, in the order the file lists them.
    #[must_use]
    pub fn conditional_formats(&self) -> &[ConditionalFormat] {
        &self.conditional_formats
    }

    /// The data validations of the sheet.
    #[must_use]
    pub fn validations(&self) -> &[DataValidation] {
        &self.validations
    }

    /// The filter buttons of the sheet, if it has them.
    #[must_use]
    pub fn auto_filter(&self) -> Option<&AutoFilter> {
        self.auto_filter.as_ref()
    }

    /// The pictures and charts of the file the sheet was read from, as they were drawn then.
    #[must_use]
    pub fn pictures(&self) -> &[SheetDrawing] {
        &self.pictures
    }

    /// How many pivot tables the sheet carries.
    #[must_use]
    pub fn pivot_count(&self) -> usize {
        self.pivots.len()
    }

    /// The cell at a position, if it has content.
    #[must_use]
    pub fn cell(&self, cell: CellRef) -> Option<&Cell> {
        self.cells.get(&cell)
    }

    /// How many cells have content or a style of their own: the size a copy of the sheet takes.
    #[must_use]
    pub fn cell_count(&self) -> usize {
        self.cells.len() + self.styles.len()
    }

    /// Every cell with content, row by row.
    pub fn cells(&self) -> impl Iterator<Item = (CellRef, &Cell)> {
        self.cells.iter().map(|(cell, content)| (*cell, content))
    }

    /// Every cell with a style other than the default, row by row, with the style's index.
    pub fn styled_cells(&self) -> impl Iterator<Item = (CellRef, u32)> {
        self.styles.iter().map(|(cell, style)| (*cell, *style))
    }

    /// The last row and column holding content, plus one: the used area from A1.
    #[must_use]
    pub fn extent(&self) -> (u32, u32) {
        let rows = self.cells.keys().last().map_or(0, |cell| cell.row + 1);
        let columns = self
            .cells
            .keys()
            .map(|cell| cell.column + 1)
            .max()
            .unwrap_or(0);
        (rows, columns)
    }
}

/// Why an edit was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    #[error(transparent)]
    Formula(#[from] FormulaError),
    #[error("there is no sheet {0}")]
    NoSuchSheet(usize),
    #[error("a sheet named {0} already exists")]
    DuplicateSheetName(String),
    #[error("a sheet name must be 1 to 31 characters, without : \\ / ? * [ ]")]
    InvalidSheetName,
    #[error("a workbook keeps at least one sheet")]
    LastSheet,
    #[error("the edit goes past the grid or the workbook limits")]
    OutOfBounds,
    #[error("a style holds a value no cell can show")]
    InvalidStyle,
    #[error("a name starts with a letter, _ or \\, holds letters, digits, . and _, and is no cell")]
    InvalidName,
}

/// A name the workbook defines, `Aliquota` or `Vendite`: what it refers to, a range, a value
/// or a formula, a `LAMBDA` included, and the sheet it belongs to, `None` for the whole workbook.
#[derive(Debug, Clone, PartialEq)]
pub struct DefinedName {
    pub name: String,
    pub sheet: Option<usize>,
    pub expression: Expr,
    /// Whether the file hid the name from its list, as add-ins hide theirs.
    pub hidden: bool,
}

/// A comment on a cell: who wrote it and its text, lines separated by `\n`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Comment {
    pub author: String,
    pub text: String,
}

/// The longest comment a cell keeps, in characters, as Excel limits it.
pub const MAX_COMMENT_CHARS: usize = 32_767;

/// A workbook with its computed values; every edit recalculates.
#[derive(Debug, Clone, PartialEq)]
pub struct Workbook {
    pub(crate) sheets: Vec<Sheet>,
    /// The computed value of every formula cell and every spilled cell, per sheet.
    pub(crate) values: Vec<HashMap<CellRef, Value>>,
    /// The range each formula that computes an array fills, by the formula's cell, per sheet.
    pub(crate) spills: Vec<BTreeMap<CellRef, CellRange>>,
    /// Which computation the values are from, unique across every workbook of the process.
    pub(crate) revision: u64,
    /// Today's serial date for `TODAY` and `NOW`, which a host sets: WebAssembly has no clock.
    pub(crate) today: Option<f64>,
    /// Every style the cells use, each once; the first is the default look.
    pub(crate) styles: Vec<CellStyle>,
    pub(crate) style_index: HashMap<CellStyle, u32>,
    /// The theme of the file the workbook was read from, written back for its charts.
    pub(crate) theme: Option<CarriedPart>,
    /// The separator of the delimited file the workbook was read from.
    pub(crate) delimiter: Option<char>,
    pub(crate) names: Vec<DefinedName>,
}

impl Default for Workbook {
    fn default() -> Self {
        Self::new()
    }
}

impl Workbook {
    /// A workbook with one empty sheet, `Foglio1`.
    #[must_use]
    pub fn new() -> Self {
        Self::from_sheets(vec![Sheet {
            name: "Foglio1".to_owned(),
            ..Sheet::default()
        }])
    }

    pub(crate) fn from_sheets(sheets: Vec<Sheet>) -> Self {
        Self::from_parts(sheets, vec![CellStyle::default()])
    }

    /// A workbook of sheets whose cells index `styles`, the default look first.
    pub(crate) fn from_parts(sheets: Vec<Sheet>, mut styles: Vec<CellStyle>) -> Self {
        if styles.first() != Some(&CellStyle::default()) {
            styles.insert(0, CellStyle::default());
        }
        let style_index = styles
            .iter()
            .enumerate()
            .rev()
            .map(|(index, style)| (style.clone(), u32::try_from(index).unwrap_or(0)))
            .collect();
        let mut workbook = Self {
            values: vec![HashMap::new(); sheets.len()],
            spills: vec![BTreeMap::new(); sheets.len()],
            revision: 0,
            sheets,
            today: None,
            styles,
            style_index,
            theme: None,
            delimiter: None,
            names: Vec::new(),
        };
        workbook.recalculate();
        workbook
    }

    #[must_use]
    pub fn sheets(&self) -> &[Sheet] {
        &self.sheets
    }

    /// The index of a sheet by name, case-insensitive as formulas name sheets.
    #[must_use]
    pub fn sheet_index(&self, name: &str) -> Option<usize> {
        self.sheets
            .iter()
            .position(|sheet| sheet.name.to_lowercase() == name.to_lowercase())
    }

    /// Set today's date, as a serial, for `TODAY` and `NOW`.
    pub fn set_today(&mut self, serial: Option<f64>) {
        self.today = serial;
        self.recalculate();
    }

    /// What a cell shows: its computed value for a formula, its input otherwise, and for an
    /// empty cell an array spills into, the value spilled there.
    #[must_use]
    pub fn value(&self, sheet: usize, cell: CellRef) -> Value {
        let Some(content) = self
            .sheets
            .get(sheet)
            .and_then(|sheet| sheet.cells.get(&cell))
        else {
            return self
                .values
                .get(sheet)
                .and_then(|values| values.get(&cell))
                .cloned()
                .unwrap_or(Value::Empty);
        };
        match &content.input {
            Input::Number(number) => Value::Number(*number),
            Input::Text(text) => Value::Text(text.clone()),
            Input::Bool(value) => Value::Bool(*value),
            Input::Formula(_) => self.values[sheet]
                .get(&cell)
                .cloned()
                .unwrap_or(Value::Empty),
        }
    }

    /// A cell's value as the locale shows it, with the cell's number format.
    #[must_use]
    pub fn display(&self, sheet: usize, cell: CellRef, locale: Locale) -> String {
        let (format, code) = self.shown_format(sheet, cell);
        crate::display::display(&self.value(sheet, cell), format, code, locale)
    }

    /// Type into a cell, as an editor does: `=` starts a formula, a number in the locale's
    /// notation is a number, an empty text clears the cell. The format stays.
    ///
    /// # Errors
    ///
    /// [`EditError::Formula`] for a formula that cannot be parsed, [`EditError::NoSuchSheet`],
    /// [`EditError::OutOfBounds`] past the grid or [`MAX_CELLS`].
    pub fn set_input(
        &mut self,
        sheet: usize,
        cell: CellRef,
        typed: &str,
        locale: Locale,
    ) -> Result<(), EditError> {
        let input = Input::parse(typed, locale)?;
        self.put(sheet, cell, input)?;
        self.recalculate();
        Ok(())
    }

    /// Set several cells in one recalculation, as a paste does.
    ///
    /// # Errors
    ///
    /// As [`Workbook::set_input`]; nothing is changed when one of the inputs is refused.
    pub fn set_inputs(
        &mut self,
        sheet: usize,
        inputs: &[(CellRef, String)],
        locale: Locale,
    ) -> Result<(), EditError> {
        let parsed = inputs
            .iter()
            .map(|(cell, typed)| Input::parse(typed, locale).map(|input| (*cell, input)))
            .collect::<Result<Vec<_>, _>>()?;
        let backup = self
            .sheets
            .get(sheet)
            .cloned()
            .ok_or(EditError::NoSuchSheet(sheet))?;
        for (cell, input) in parsed {
            if let Err(error) = self.put(sheet, cell, input) {
                self.sheets[sheet] = backup;
                return Err(error);
            }
        }
        self.recalculate();
        Ok(())
    }

    fn put(&mut self, sheet: usize, cell: CellRef, input: Option<Input>) -> Result<(), EditError> {
        if cell.row >= MAX_ROWS || cell.column >= MAX_COLUMNS {
            return Err(EditError::OutOfBounds);
        }
        let total = self.cell_count();
        let target = self
            .sheets
            .get_mut(sheet)
            .ok_or(EditError::NoSuchSheet(sheet))?;
        match input {
            Some(input) => {
                if !target.cells.contains_key(&cell) && total >= MAX_CELLS {
                    return Err(EditError::OutOfBounds);
                }
                let format = target
                    .cells
                    .get(&cell)
                    .map_or(NumberFormat::General, |content| content.format);
                target.cells.insert(cell, Cell { input, format });
            }
            None => {
                target.cells.remove(&cell);
            }
        }
        Ok(())
    }

    /// Set the number format of a cell with content; an empty cell has nothing to format.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`].
    pub fn set_format(
        &mut self,
        sheet: usize,
        cell: CellRef,
        format: NumberFormat,
    ) -> Result<(), EditError> {
        let target = self
            .sheets
            .get_mut(sheet)
            .ok_or(EditError::NoSuchSheet(sheet))?;
        if let Some(content) = target.cells.get_mut(&cell) {
            content.format = match format {
                NumberFormat::Decimals(decimals) => NumberFormat::Decimals(decimals.min(10)),
                other => other,
            };
            self.drop_format_code(sheet, cell);
        }
        Ok(())
    }

    /// Set the width of a column, in characters; `None` restores the default.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::OutOfBounds`] past the grid.
    pub fn set_column_width(
        &mut self,
        sheet: usize,
        column: u32,
        width: Option<f64>,
    ) -> Result<(), EditError> {
        if column >= MAX_COLUMNS {
            return Err(EditError::OutOfBounds);
        }
        let target = self
            .sheets
            .get_mut(sheet)
            .ok_or(EditError::NoSuchSheet(sheet))?;
        match width.filter(|width| width.is_finite()) {
            Some(width) => {
                target.column_widths.insert(column, width.clamp(0.0, 255.0));
            }
            None => {
                target.column_widths.remove(&column);
            }
        }
        Ok(())
    }

    /// Add an empty sheet at the end and return its index.
    ///
    /// # Errors
    ///
    /// [`EditError::InvalidSheetName`], [`EditError::DuplicateSheetName`], or
    /// [`EditError::OutOfBounds`] past [`MAX_SHEETS`].
    pub fn add_sheet(&mut self, name: &str) -> Result<usize, EditError> {
        self.check_new_name(name, None)?;
        if self.sheets.len() >= MAX_SHEETS {
            return Err(EditError::OutOfBounds);
        }
        self.sheets.push(Sheet {
            name: name.to_owned(),
            ..Sheet::default()
        });
        self.values.push(HashMap::new());
        self.recalculate();
        Ok(self.sheets.len() - 1)
    }

    /// Rename a sheet; formulas that name it follow.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::InvalidSheetName`],
    /// [`EditError::DuplicateSheetName`].
    pub fn rename_sheet(&mut self, sheet: usize, name: &str) -> Result<(), EditError> {
        if sheet >= self.sheets.len() {
            return Err(EditError::NoSuchSheet(sheet));
        }
        self.check_new_name(name, Some(sheet))?;
        let old = self.sheets[sheet].name.clone();
        crate::edit::rename_references(&mut self.sheets, &mut self.names, &old, name);
        name.clone_into(&mut self.sheets[sheet].name);
        self.recalculate();
        Ok(())
    }

    /// Remove a sheet; formulas that named it show `#REF!`.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::LastSheet`].
    pub fn remove_sheet(&mut self, sheet: usize) -> Result<(), EditError> {
        if sheet >= self.sheets.len() {
            return Err(EditError::NoSuchSheet(sheet));
        }
        if self.sheets.len() == 1 {
            return Err(EditError::LastSheet);
        }
        let removed = self.sheets.remove(sheet);
        self.values.remove(sheet);
        crate::edit::break_references_to_sheet(&mut self.sheets, &mut self.names, &removed.name);
        // The names of the removed sheet go with it; the later sheets move up by one.
        self.names.retain(|defined| defined.sheet != Some(sheet));
        for defined in &mut self.names {
            if let Some(index) = &mut defined.sheet
                && *index > sheet
            {
                *index -= 1;
            }
        }
        self.recalculate();
        Ok(())
    }

    /// Insert `count` empty rows before `at`; cells below move down and formulas follow them.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::OutOfBounds`] when content would leave the grid.
    pub fn insert_rows(&mut self, sheet: usize, at: u32, count: u32) -> Result<(), EditError> {
        self.shift(sheet, crate::edit::Shift::InsertRows { at, count })
    }

    /// Delete `count` rows from `at`; formulas that named a deleted cell show `#REF!`.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`].
    pub fn delete_rows(&mut self, sheet: usize, at: u32, count: u32) -> Result<(), EditError> {
        self.shift(sheet, crate::edit::Shift::DeleteRows { at, count })
    }

    /// Insert `count` empty columns before `at`.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::OutOfBounds`] when content would leave the grid.
    pub fn insert_columns(&mut self, sheet: usize, at: u32, count: u32) -> Result<(), EditError> {
        self.shift(sheet, crate::edit::Shift::InsertColumns { at, count })
    }

    /// Delete `count` columns from `at`.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`].
    pub fn delete_columns(&mut self, sheet: usize, at: u32, count: u32) -> Result<(), EditError> {
        self.shift(sheet, crate::edit::Shift::DeleteColumns { at, count })
    }

    fn shift(&mut self, sheet: usize, shift: crate::edit::Shift) -> Result<(), EditError> {
        if sheet >= self.sheets.len() {
            return Err(EditError::NoSuchSheet(sheet));
        }
        crate::edit::shift(&mut self.sheets, &mut self.names, sheet, shift)?;
        self.recalculate();
        Ok(())
    }

    /// Copy the cells of a rectangle so its top-left corner lands on `to`, formulas adjusted as
    /// a paste adjusts them: relative references move with the cell, absolute ones stay.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::OutOfBounds`] past the grid or [`MAX_CELLS`].
    pub fn copy_range(
        &mut self,
        sheet: usize,
        from: (CellRef, CellRef),
        target_sheet: usize,
        to: CellRef,
    ) -> Result<(), EditError> {
        let source = self
            .sheets
            .get(sheet)
            .ok_or(EditError::NoSuchSheet(sheet))?;
        if target_sheet >= self.sheets.len() {
            return Err(EditError::NoSuchSheet(target_sheet));
        }
        let (start, end) = from;
        let (top, left) = (start.row.min(end.row), start.column.min(end.column));
        let (bottom, right) = (start.row.max(end.row), start.column.max(end.column));
        let row_offset = i64::from(to.row) - i64::from(top);
        let column_offset = i64::from(to.column) - i64::from(left);
        let mut copies = Vec::new();
        for row in top..=bottom {
            for (cell, content) in source
                .cells
                .range(CellRef::new(row, left)..=CellRef::new(row, right))
            {
                let target = CellRef::new(
                    u32::try_from(i64::from(cell.row) + row_offset)
                        .map_err(|_| EditError::OutOfBounds)?,
                    u32::try_from(i64::from(cell.column) + column_offset)
                        .map_err(|_| EditError::OutOfBounds)?,
                );
                if target.row >= MAX_ROWS || target.column >= MAX_COLUMNS {
                    return Err(EditError::OutOfBounds);
                }
                let input = match &content.input {
                    Input::Formula(expression) => Input::Formula(Box::new(
                        crate::edit::offset_relative(expression, row_offset, column_offset),
                    )),
                    other => other.clone(),
                };
                copies.push((
                    target,
                    Cell {
                        input,
                        format: content.format,
                    },
                ));
            }
        }
        if self.cell_count() + copies.len() > MAX_CELLS {
            return Err(EditError::OutOfBounds);
        }
        // A paste brings the look of its cells along.
        let style_copies: Vec<(CellRef, u32)> = (top..=bottom)
            .flat_map(|row| {
                source
                    .styles
                    .range(CellRef::new(row, left)..=CellRef::new(row, right))
                    .filter_map(|(cell, style)| {
                        let row = u32::try_from(i64::from(cell.row) + row_offset).ok()?;
                        let column = u32::try_from(i64::from(cell.column) + column_offset).ok()?;
                        Some((CellRef::new(row, column), *style))
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        // The target rectangle is cleared first, so empty source cells empty their targets.
        let height = bottom - top;
        let width = right - left;
        let target = &mut self.sheets[target_sheet];
        let clear: Vec<CellRef> = (to.row..=to.row.saturating_add(height))
            .flat_map(|row| {
                target
                    .cells
                    .range(
                        CellRef::new(row, to.column)
                            ..=CellRef::new(row, to.column.saturating_add(width)),
                    )
                    .map(|(cell, _)| *cell)
                    .collect::<Vec<_>>()
            })
            .collect();
        for cell in clear {
            target.cells.remove(&cell);
        }
        for (cell, content) in copies {
            target.cells.insert(cell, content);
        }
        let (target_bottom, target_right) = (
            to.row.saturating_add(height),
            to.column.saturating_add(width),
        );
        target.styles.retain(|cell, _| {
            !(to.row..=target_bottom).contains(&cell.row)
                || !(to.column..=target_right).contains(&cell.column)
        });
        for (cell, style) in style_copies {
            if cell.row < MAX_ROWS && cell.column < MAX_COLUMNS {
                target.styles.insert(cell, style);
            }
        }
        self.recalculate();
        Ok(())
    }

    /// Empty every cell of a rectangle.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`].
    pub fn clear_range(&mut self, sheet: usize, from: (CellRef, CellRef)) -> Result<(), EditError> {
        let target = self
            .sheets
            .get_mut(sheet)
            .ok_or(EditError::NoSuchSheet(sheet))?;
        let (start, end) = from;
        let (top, left) = (start.row.min(end.row), start.column.min(end.column));
        let (bottom, right) = (start.row.max(end.row), start.column.max(end.column));
        let cleared: Vec<CellRef> = (top..=bottom)
            .flat_map(|row| {
                target
                    .cells
                    .range(CellRef::new(row, left)..=CellRef::new(row, right))
                    .map(|(cell, _)| *cell)
                    .collect::<Vec<_>>()
            })
            .collect();
        for cell in cleared {
            target.cells.remove(&cell);
        }
        self.recalculate();
        Ok(())
    }

    pub(crate) fn cell_count(&self) -> usize {
        self.sheets.iter().map(|sheet| sheet.cells.len()).sum()
    }

    /// Recompute every formula.
    pub(crate) fn recalculate(&mut self) {
        static NEXT_REVISION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let computed = evaluate::evaluate_workbook(self);
        self.values = computed.values;
        self.spills = computed.spills;
        self.revision = NEXT_REVISION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn check_new_name(&self, name: &str, except: Option<usize>) -> Result<(), EditError> {
        let length = name.chars().count();
        if length == 0
            || length > 31
            || name
                .chars()
                .any(|character| matches!(character, ':' | '\\' | '/' | '?' | '*' | '[' | ']'))
            || name.starts_with('\'')
            || name.ends_with('\'')
        {
            return Err(EditError::InvalidSheetName);
        }
        if self
            .sheet_index(name)
            .is_some_and(|existing| Some(existing) != except)
        {
            return Err(EditError::DuplicateSheetName(name.to_owned()));
        }
        Ok(())
    }

    /// The names the workbook defines, the workbook's first, then each sheet's own.
    #[must_use]
    pub fn defined_names(&self) -> &[DefinedName] {
        &self.names
    }

    /// What a name a formula of `sheet` reads refers to: the sheet's own name before the
    /// workbook's. Names are compared without case, as Excel compares them.
    pub(crate) fn defined_name(&self, sheet: usize, name: &str) -> Option<&Expr> {
        let named = |defined: &&DefinedName| defined.name.eq_ignore_ascii_case(name);
        self.names
            .iter()
            .filter(named)
            .find(|defined| defined.sheet == Some(sheet))
            .or_else(|| {
                self.names
                    .iter()
                    .filter(named)
                    .find(|defined| defined.sheet.is_none())
            })
            .map(|defined| &defined.expression)
    }

    /// Define a name, or change what it refers to: `formula` is typed as in a cell, with or
    /// without its `=`, in `locale`; `sheet` keeps the name to one sheet.
    ///
    /// # Errors
    ///
    /// [`EditError::InvalidName`] for a name Excel refuses, [`EditError::NoSuchSheet`],
    /// [`EditError::Formula`] for a formula that cannot be parsed.
    pub fn set_defined_name(
        &mut self,
        name: &str,
        sheet: Option<usize>,
        formula: &str,
        locale: Locale,
    ) -> Result<(), EditError> {
        if !is_valid_name(name) {
            return Err(EditError::InvalidName);
        }
        if let Some(sheet) = sheet
            && sheet >= self.sheets.len()
        {
            return Err(EditError::NoSuchSheet(sheet));
        }
        let text = formula.trim();
        let text = text.strip_prefix('=').unwrap_or(text);
        let expression = crate::formula::parse_formula_in(text, locale)?;
        // A file keeps the sheet of every reference a name makes, as Excel's name manager does.
        let home = &self.sheets[sheet.unwrap_or(0)].name;
        let expression = crate::edit::qualify_references(&expression, home);
        let defined = DefinedName {
            name: name.to_owned(),
            sheet,
            expression,
            hidden: false,
        };
        match self
            .names
            .iter_mut()
            .find(|known| known.sheet == sheet && known.name.eq_ignore_ascii_case(name))
        {
            Some(known) => *known = defined,
            None => self.names.push(defined),
        }
        self.recalculate();
        Ok(())
    }

    /// Write a comment on a cell, or remove it with an empty text.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::OutOfBounds`] for a cell past the grid or a text
    /// longer than [`MAX_COMMENT_CHARS`].
    pub fn set_comment(
        &mut self,
        sheet: usize,
        cell: CellRef,
        author: &str,
        text: &str,
    ) -> Result<(), EditError> {
        let Some(target) = self.sheets.get_mut(sheet) else {
            return Err(EditError::NoSuchSheet(sheet));
        };
        if cell.row >= MAX_ROWS
            || cell.column >= MAX_COLUMNS
            || text.chars().count() > MAX_COMMENT_CHARS
            || author.chars().count() > MAX_COMMENT_CHARS
        {
            return Err(EditError::OutOfBounds);
        }
        let text = text.replace("\r\n", "\n");
        if text.trim().is_empty() {
            target.comments.remove(&cell);
        } else {
            target.comments.insert(
                cell,
                Comment {
                    author: author.trim().to_owned(),
                    text,
                },
            );
        }
        Ok(())
    }

    /// Remove a name; formulas that read it show `#NAME?`. False when the workbook has none.
    pub fn remove_defined_name(&mut self, name: &str, sheet: Option<usize>) -> bool {
        let before = self.names.len();
        self.names
            .retain(|known| !(known.sheet == sheet && known.name.eq_ignore_ascii_case(name)));
        let removed = self.names.len() < before;
        if removed {
            self.recalculate();
        }
        removed
    }

    /// The separator of the csv or tsv the workbook was read from, so a save writes the file
    /// back as it came; `None` for any other file.
    #[must_use]
    pub fn delimiter(&self) -> Option<char> {
        self.delimiter
    }

    /// The pictures and charts of a sheet, the charts drawn again from the values the cells hold
    /// now, so a chart follows an edit; a sheet whose drawing is not carried gives what it was
    /// read with.
    #[must_use]
    pub fn pictures(&self, sheet: usize) -> Vec<SheetDrawing> {
        let Some(content) = self.sheets.get(sheet) else {
            return Vec::new();
        };
        let Some(drawing) = &content.drawing else {
            return content.pictures.clone();
        };
        let last_column = content
            .column_widths
            .keys()
            .next_back()
            .map_or(0, |last| last + 1);
        let layout = SheetLayout {
            column_widths: (0..last_column)
                .map(|column| {
                    content.column_widths.get(&column).map(|width| {
                        #[expect(
                            clippy::cast_possible_truncation,
                            clippy::cast_sign_loss,
                            reason = "widths are clamped to 255"
                        )]
                        let units = (width * 256.0).round() as u32;
                        units
                    })
                })
                .collect(),
            row_heights: content
                .row_heights
                .iter()
                .map(|(row, height)| {
                    #[expect(
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss,
                        reason = "heights are clamped to 409 points"
                    )]
                    let twips = (height * 20.0).round() as u32;
                    (*row, twips)
                })
                .collect(),
            ..SheetLayout::default()
        };
        let values = |formula: &str| {
            let (name, range) = chart_formula_range(formula)?;
            let index = self.sheet_index(&name)?;
            let area = u64::from(range.last_row - range.first_row + 1)
                * u64::from(range.last_column - range.first_column + 1);
            if area > 100_000 {
                return None;
            }
            let mut found = Vec::new();
            for row in range.first_row..=range.last_row {
                for column in range.first_column..=range.last_column {
                    found.push(match self.value(index, CellRef::new(row, column)) {
                        Value::Number(number) => ChartValue::Number(number),
                        Value::Empty => ChartValue::Empty,
                        Value::Text(text) => ChartValue::Text(text),
                        Value::Bool(flag) => {
                            ChartValue::Text(if flag { "TRUE" } else { "FALSE" }.to_owned())
                        }
                        Value::Error(kind) => ChartValue::Text(kind.code().to_owned()),
                    });
                }
            }
            Some(found)
        };
        let drawn = redraw_carried_drawing(
            drawing,
            self.theme.as_ref(),
            &layout,
            &Limits::default(),
            &values,
        );
        if drawn.is_empty() {
            content.pictures.clone()
        } else {
            drawn
        }
    }

    /// Which computation the workbook's values are from: it changes with every edit, and a copy
    /// keeps its own, so a host can keep what it drew from the values until it changes.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The ranges the arrays of a sheet's formulas fill, each by the formula's cell.
    #[must_use]
    pub fn spills(&self, sheet: usize) -> Vec<(CellRef, CellRange)> {
        self.spills
            .get(sheet)
            .map(|spills| {
                spills
                    .iter()
                    .map(|(anchor, range)| (*anchor, *range))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The cell of the formula whose array reaches a cell, the formula's own cell included.
    #[must_use]
    pub fn spill_anchor(&self, sheet: usize, cell: CellRef) -> Option<CellRef> {
        self.spills
            .get(sheet)?
            .iter()
            .find_map(|(anchor, range)| range.contains(cell.row, cell.column).then_some(*anchor))
    }

    /// The formula of a cell, parsed, if it holds one.
    #[must_use]
    pub fn formula(&self, sheet: usize, cell: CellRef) -> Option<&Expr> {
        match &self.sheets.get(sheet)?.cells.get(&cell)?.input {
            Input::Formula(expression) => Some(expression),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;

/// Whether Excel accepts a name: a letter, `_` or `\\` first, then letters, digits, `.` and `_`,
/// at most 255 characters, neither a cell such as `A1` or `R1C1` nor `TRUE` or `FALSE`.
pub(crate) fn is_valid_name(name: &str) -> bool {
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    if !(first.is_alphabetic() || first == '_' || first == '\\') || name.chars().count() > 255 {
        return false;
    }
    if !characters
        .all(|character| character.is_alphanumeric() || matches!(character, '.' | '_' | '\\'))
    {
        return false;
    }
    let upper = name.to_ascii_uppercase();
    if matches!(
        upper.as_str(),
        "TRUE" | "FALSE" | "VERO" | "FALSO" | "R" | "C"
    ) {
        return false;
    }
    let looks_like_cell = CellRef::parse(&upper).is_some();
    let looks_like_r1c1 = upper.strip_prefix('R').is_some_and(|rest| {
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        rest[digits..]
            .strip_prefix('C')
            .is_some_and(|columns| columns.chars().all(|character| character.is_ascii_digit()))
    });
    !looks_like_cell && !looks_like_r1c1
}
