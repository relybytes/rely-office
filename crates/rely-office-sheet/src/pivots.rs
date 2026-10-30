// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Pivot tables: a range summarised by the values of some of its columns, computed into the
//! cells of a sheet. A pivot table is refreshed on demand, as in Excel: its cells keep what the
//! last refresh wrote until the next one.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use rely_office_core::CellRange;

use crate::address::{CellRef, MAX_COLUMNS, MAX_ROWS};
use crate::formula::{Expr, number_text};
use crate::model::{Cell, EditError, NumberFormat, Sheet, Workbook};
use crate::value::{ErrorKind, Input, Locale, Value};

/// The most pivot tables one sheet holds.
pub const MAX_PIVOT_TABLES: usize = 64;
/// The most distinct rows and columns of labels a pivot table shows.
pub const MAX_PIVOT_ROW_KEYS: usize = 10_000;
pub const MAX_PIVOT_COLUMN_KEYS: usize = 256;
/// The most fields one pivot table places, over its rows, columns, values and filters.
pub const MAX_PIVOT_FIELDS: usize = 64;
/// The most source cells a pivot table reads.
pub const MAX_PIVOT_SOURCE_CELLS: u64 = 2_000_000;
/// The longest name a pivot table and a value caption take.
const MAX_PIVOT_NAME_CHARS: usize = 255;

/// How a value field summarises the records of a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PivotFunction {
    #[default]
    Sum,
    /// Every value that is not empty.
    Count,
    Average,
    Max,
    Min,
    Product,
    /// The numbers only.
    CountNumbers,
    /// The sample standard deviation.
    StdDev,
    /// The population standard deviation.
    StdDevP,
    Var,
    VarP,
}

impl PivotFunction {
    pub const ALL: [Self; 11] = [
        Self::Sum,
        Self::Count,
        Self::Average,
        Self::Max,
        Self::Min,
        Self::Product,
        Self::CountNumbers,
        Self::StdDev,
        Self::StdDevP,
        Self::Var,
        Self::VarP,
    ];

    /// The stable name an API passes: `sum`, `countNumbers`, `stdDevP`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Sum => "sum",
            Self::Count => "count",
            Self::Average => "average",
            Self::Max => "max",
            Self::Min => "min",
            Self::Product => "product",
            Self::CountNumbers => "countNumbers",
            Self::StdDev => "stdDev",
            Self::StdDevP => "stdDevP",
            Self::Var => "var",
            Self::VarP => "varP",
        }
    }

    /// The function of its stable name.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|function| function.name() == name)
    }

    /// The `subtotal` an xlsx data field names it by.
    pub(crate) fn xlsx(self) -> &'static str {
        match self {
            Self::CountNumbers => "countNums",
            Self::StdDevP => "stdDevp",
            Self::VarP => "varp",
            other => other.name(),
        }
    }

    pub(crate) fn from_xlsx(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|function| function.xlsx().eq_ignore_ascii_case(name))
    }

    /// The `table:function` of an ods data pilot field.
    pub(crate) fn odf(self) -> &'static str {
        match self {
            Self::Sum => "sum",
            Self::Count => "count",
            Self::Average => "average",
            Self::Max => "max",
            Self::Min => "min",
            Self::Product => "product",
            Self::CountNumbers => "countnums",
            Self::StdDev => "stdev",
            Self::StdDevP => "stdevp",
            Self::Var => "var",
            Self::VarP => "varp",
        }
    }

    pub(crate) fn from_odf(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|function| function.odf().eq_ignore_ascii_case(name))
    }

    fn caption(self, locale: Locale) -> &'static str {
        let italian = locale == Locale::Italian;
        match self {
            Self::Sum => pick(italian, "Somma", "Sum"),
            Self::Count => pick(italian, "Conteggio", "Count"),
            Self::Average => pick(italian, "Media", "Average"),
            Self::Max => "Max",
            Self::Min => "Min",
            Self::Product => pick(italian, "Prodotto", "Product"),
            Self::CountNumbers => pick(italian, "Conta numeri", "Count Numbers"),
            Self::StdDev => pick(italian, "Dev. standard", "StdDev"),
            Self::StdDevP => pick(italian, "Dev. standard pop.", "StdDevp"),
            Self::Var => pick(italian, "Varianza", "Var"),
            Self::VarP => pick(italian, "Varianza pop.", "Varp"),
        }
    }

    /// The function over the values of one cell of the table.
    pub(crate) fn summarise(self, values: &[Value]) -> Value {
        let numbers: Vec<f64> = values
            .iter()
            .filter_map(|value| match value {
                Value::Number(number) => Some(*number),
                _ => None,
            })
            .collect();
        #[expect(clippy::cast_precision_loss, reason = "a count of records is small")]
        let count = numbers.len() as f64;
        let mean = numbers.iter().sum::<f64>() / count;
        let squares = || {
            numbers
                .iter()
                .map(|number| (number - mean).powi(2))
                .sum::<f64>()
        };
        let number = match self {
            Self::Sum => numbers.iter().sum(),
            #[expect(clippy::cast_precision_loss, reason = "a count of records is small")]
            Self::Count => values
                .iter()
                .filter(|value| !matches!(value, Value::Empty))
                .count() as f64,
            Self::CountNumbers => count,
            Self::Average if numbers.is_empty() => return Value::Error(ErrorKind::Div0),
            Self::Average => mean,
            Self::Max => numbers.iter().copied().reduce(f64::max).unwrap_or(0.0),
            Self::Min => numbers.iter().copied().reduce(f64::min).unwrap_or(0.0),
            Self::Product if numbers.is_empty() => 0.0,
            Self::Product => numbers.iter().product(),
            Self::StdDev | Self::Var if numbers.len() < 2 => return Value::Error(ErrorKind::Div0),
            Self::StdDevP | Self::VarP if numbers.is_empty() => {
                return Value::Error(ErrorKind::Div0);
            }
            Self::StdDev => (squares() / (count - 1.0)).sqrt(),
            Self::Var => squares() / (count - 1.0),
            Self::StdDevP => (squares() / count).sqrt(),
            Self::VarP => squares() / count,
        };
        Value::Number(number)
    }
}

fn pick(italian: bool, it: &'static str, en: &'static str) -> &'static str {
    if italian { it } else { en }
}

/// A column of the source placed on an axis or as a filter, by its position in the source, with
/// the items it leaves out, by label.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PivotField {
    pub field: u32,
    pub hidden: Vec<String>,
}

/// A column of the source summarised in the cells of the table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PivotValue {
    pub field: u32,
    pub function: PivotFunction,
    /// The caption over its column; `Sum of Amount` when none is given.
    pub name: Option<String>,
}

/// A pivot table: the range it reads, a header row then records, where its cells start, and
/// the source columns it shows as rows, columns, values and filters.
#[derive(Debug, Clone, PartialEq)]
pub struct PivotTable {
    pub name: String,
    /// The sheet of the source, by name, so renaming it and moving sheets keep the table whole.
    pub source_sheet: String,
    pub source: CellRange,
    /// The top-left cell of the table on its own sheet; the filters come first, then a blank row.
    pub anchor: CellRef,
    pub rows: Vec<PivotField>,
    pub columns: Vec<PivotField>,
    pub values: Vec<PivotValue>,
    pub filters: Vec<PivotField>,
    pub row_grand_totals: bool,
    pub column_grand_totals: bool,
    /// The language of the captions the table writes: `Grand Total`, `Totale complessivo`.
    pub locale: Locale,
    /// The cells the table filled when it was last computed.
    pub(crate) area: Option<CellRange>,
    /// The source as it was last computed, its header first, which `GETPIVOTDATA` reads.
    pub(crate) snapshot: Vec<Vec<Value>>,
}

impl PivotTable {
    /// A table of `source` on `source_sheet` starting at `anchor`, with nothing placed yet and
    /// both grand totals shown.
    #[must_use]
    pub fn new(name: &str, source_sheet: &str, source: CellRange, anchor: CellRef) -> Self {
        Self {
            name: name.to_owned(),
            source_sheet: source_sheet.to_owned(),
            source,
            anchor,
            rows: Vec::new(),
            columns: Vec::new(),
            values: Vec::new(),
            filters: Vec::new(),
            row_grand_totals: true,
            column_grand_totals: true,
            locale: Locale::default(),
            area: None,
            snapshot: Vec::new(),
        }
    }

    /// The cells the table filled when it was last computed.
    #[must_use]
    pub fn area(&self) -> Option<CellRange> {
        self.area
    }

    /// The caption of a value field: its own name, else `Sum of Amount`.
    #[must_use]
    pub fn value_caption(&self, value: &PivotValue, header: &[Value]) -> String {
        if let Some(name) = value.name.as_ref().filter(|name| !name.is_empty()) {
            return name.clone();
        }
        let field = header
            .get(value.field as usize)
            .map(item_label)
            .unwrap_or_default();
        let joiner = pick(self.locale == Locale::Italian, "di", "of");
        format!("{} {joiner} {field}", value.function.caption(self.locale))
    }

    fn italian(&self) -> bool {
        self.locale == Locale::Italian
    }

    fn grand_total(&self) -> &'static str {
        pick(self.italian(), "Totale complessivo", "Grand Total")
    }

    fn width(&self) -> u32 {
        self.source.last_column - self.source.first_column + 1
    }

    /// Whether the table places its fields within its source and each field once on an axis.
    fn is_valid(&self) -> bool {
        let width = self.width();
        let placed: Vec<u32> = self
            .rows
            .iter()
            .chain(&self.columns)
            .chain(&self.filters)
            .map(|field| field.field)
            .collect();
        let distinct: BTreeSet<u32> = placed.iter().copied().collect();
        self.name.chars().count() <= MAX_PIVOT_NAME_CHARS
            && self.source.first_row <= self.source.last_row
            && self.source.first_column <= self.source.last_column
            && self.source.last_row < MAX_ROWS
            && self.source.last_column < MAX_COLUMNS
            && u64::from(self.source.last_row - self.source.first_row + 1) * u64::from(width)
                <= MAX_PIVOT_SOURCE_CELLS
            && distinct.len() == placed.len()
            && placed.len() + self.values.len() <= MAX_PIVOT_FIELDS
            && placed.iter().all(|field| *field < width)
            && self.values.iter().all(|value| {
                value.field < width
                    && value
                        .name
                        .as_ref()
                        .is_none_or(|name| name.chars().count() <= MAX_PIVOT_NAME_CHARS)
            })
            && self.anchor.row < MAX_ROWS
            && self.anchor.column < MAX_COLUMNS
    }

    /// The source's header and records as they are now.
    pub(crate) fn read_source(&self, workbook: &Workbook) -> Result<Vec<Vec<Value>>, EditError> {
        let sheet = workbook
            .sheet_index(&self.source_sheet)
            .ok_or(EditError::InvalidPivot)?;
        Ok((self.source.first_row..=self.source.last_row)
            .map(|row| {
                (self.source.first_column..=self.source.last_column)
                    .map(|column| workbook.value(sheet, CellRef::new(row, column)))
                    .collect()
            })
            .collect())
    }

    /// The records a field's hidden items and the filters leave in.
    pub(crate) fn kept_records<'r>(&self, records: &'r [Vec<Value>]) -> Vec<&'r Vec<Value>> {
        let placed: Vec<&PivotField> = self
            .rows
            .iter()
            .chain(&self.columns)
            .chain(&self.filters)
            .filter(|field| !field.hidden.is_empty())
            .collect();
        records
            .iter()
            .filter(|record| {
                placed.iter().all(|field| {
                    let label = record
                        .get(field.field as usize)
                        .map(item_label)
                        .unwrap_or_default();
                    !field.hidden.contains(&label)
                })
            })
            .collect()
    }

    /// The table's cells from its top-left corner: the filters, a blank row, the header, a row
    /// per distinct row label and the grand total.
    #[expect(
        clippy::too_many_lines,
        reason = "the filters, the header, the rows and the totals are laid out in one pass"
    )]
    pub(crate) fn layout(&self, source: &[Vec<Value>]) -> Result<Vec<Vec<Value>>, EditError> {
        let Some((header, records)) = source.split_first() else {
            return Err(EditError::InvalidPivot);
        };
        let kept = self.kept_records(records);
        let key_of = |record: &Vec<Value>, fields: &[PivotField]| -> Vec<Key> {
            fields
                .iter()
                .map(|field| {
                    Key(record
                        .get(field.field as usize)
                        .cloned()
                        .unwrap_or(Value::Empty))
                })
                .collect()
        };
        let mut groups: BTreeMap<(Labels, Labels), Vec<&Vec<Value>>> = BTreeMap::new();
        let mut row_keys: BTreeSet<Vec<Key>> = BTreeSet::new();
        let mut column_keys: BTreeSet<Vec<Key>> = BTreeSet::new();
        for record in &kept {
            let row = key_of(record, &self.rows);
            let column = key_of(record, &self.columns);
            row_keys.insert(row.clone());
            column_keys.insert(column.clone());
            groups.entry((row, column)).or_default().push(record);
        }
        if row_keys.is_empty() {
            row_keys.insert(Vec::new());
        }
        if column_keys.is_empty() {
            column_keys.insert(Vec::new());
        }
        if row_keys.len() > MAX_PIVOT_ROW_KEYS || column_keys.len() > MAX_PIVOT_COLUMN_KEYS {
            return Err(EditError::InvalidPivot);
        }
        let summarise = |records: &[&&Vec<Value>], value: &PivotValue| {
            let values: Vec<Value> = records
                .iter()
                .map(|record| {
                    record
                        .get(value.field as usize)
                        .cloned()
                        .unwrap_or(Value::Empty)
                })
                .collect();
            value.function.summarise(&values)
        };
        let blank = pick(self.italian(), "(vuoto)", "(blank)");
        let shown = |key: &Key| match &key.0 {
            Value::Empty => Value::Text(blank.to_owned()),
            other => other.clone(),
        };
        let mut grid: Vec<Vec<Value>> = Vec::new();
        for filter in &self.filters {
            let items: BTreeSet<Key> = records
                .iter()
                .map(|record| {
                    Key(record
                        .get(filter.field as usize)
                        .cloned()
                        .unwrap_or(Value::Empty))
                })
                .filter(|key| !filter.hidden.contains(&item_label(&key.0)))
                .collect();
            let all_items = records
                .iter()
                .map(|record| {
                    item_label(record.get(filter.field as usize).unwrap_or(&Value::Empty))
                })
                .collect::<BTreeSet<String>>()
                .len();
            let state = if items.len() == all_items {
                Value::Text(pick(self.italian(), "(Tutto)", "(All)").to_owned())
            } else if items.len() == 1 {
                items.first().map_or(Value::Empty, shown)
            } else {
                Value::Text(pick(self.italian(), "(Più elementi)", "(Multiple Items)").to_owned())
            };
            grid.push(vec![
                Value::Text(
                    header
                        .get(filter.field as usize)
                        .map(item_label)
                        .unwrap_or_default(),
                ),
                state,
            ]);
        }
        if !self.filters.is_empty() {
            grid.push(Vec::new());
        }
        let several_values = self.values.len() > 1 || self.columns.is_empty();
        let mut heading: Vec<Value> = self
            .rows
            .iter()
            .map(|field| {
                Value::Text(
                    header
                        .get(field.field as usize)
                        .map(item_label)
                        .unwrap_or_default(),
                )
            })
            .collect();
        for column in &column_keys {
            for value in &self.values {
                let mut parts: Vec<String> =
                    column.iter().map(|key| text_of(&shown(key))).collect();
                if several_values {
                    parts.push(self.value_caption(value, header));
                }
                heading.push(Value::Text(parts.join(" - ")));
            }
        }
        let column_totals = self.column_grand_totals && !self.columns.is_empty();
        if column_totals {
            for value in &self.values {
                let mut caption = self.grand_total().to_owned();
                if self.values.len() > 1 {
                    caption = format!("{caption} - {}", self.value_caption(value, header));
                }
                heading.push(Value::Text(caption));
            }
        }
        grid.push(heading);
        for row in &row_keys {
            let mut line: Vec<Value> = row.iter().map(shown).collect();
            for column in &column_keys {
                let records: Vec<&&Vec<Value>> = groups
                    .get(&(row.clone(), column.clone()))
                    .map(|records| records.iter().collect())
                    .unwrap_or_default();
                for value in &self.values {
                    line.push(if records.is_empty() {
                        Value::Empty
                    } else {
                        summarise(&records, value)
                    });
                }
            }
            if column_totals {
                let records: Vec<&&Vec<Value>> = groups
                    .iter()
                    .filter(|((key, _), _)| key == row)
                    .flat_map(|(_, records)| records.iter())
                    .collect();
                for value in &self.values {
                    line.push(summarise(&records, value));
                }
            }
            grid.push(line);
        }
        if self.row_grand_totals && !self.rows.is_empty() {
            let mut line = vec![Value::Text(self.grand_total().to_owned())];
            line.resize(self.rows.len(), Value::Empty);
            for column in &column_keys {
                let records: Vec<&&Vec<Value>> = groups
                    .iter()
                    .filter(|((_, key), _)| key == column)
                    .flat_map(|(_, records)| records.iter())
                    .collect();
                for value in &self.values {
                    line.push(summarise(&records, value));
                }
            }
            if column_totals {
                let records: Vec<&&Vec<Value>> = kept.iter().collect();
                for value in &self.values {
                    line.push(summarise(&records, value));
                }
            }
            grid.push(line);
        }
        let width = grid.iter().map(Vec::len).max().unwrap_or(0);
        if u64::from(self.anchor.row) + grid.len() as u64 > u64::from(MAX_ROWS)
            || u64::from(self.anchor.column) + width as u64 > u64::from(MAX_COLUMNS)
        {
            return Err(EditError::OutOfBounds);
        }
        Ok(grid)
    }
}

/// The labels of a record on the fields of an axis, in their order.
type Labels = Vec<Key>;

/// The text an item of a field is matched and hidden by: a number as a formula writes it.
#[must_use]
pub fn item_label(value: &Value) -> String {
    match value {
        Value::Number(number) => number_text(*number),
        Value::Text(text) => text.clone(),
        Value::Bool(true) => "TRUE".to_owned(),
        Value::Bool(false) => "FALSE".to_owned(),
        Value::Error(kind) => kind.code().to_owned(),
        Value::Empty => String::new(),
    }
}

fn text_of(value: &Value) -> String {
    item_label(value)
}

/// A value as a pivot table orders its labels: numbers, then text ignoring case, then logical
/// values, errors and blanks last.
#[derive(Debug, Clone)]
pub(crate) struct Key(pub(crate) Value);

impl Key {
    fn rank(&self) -> u8 {
        match self.0 {
            Value::Number(_) => 0,
            Value::Text(_) => 1,
            Value::Bool(_) => 2,
            Value::Error(_) => 3,
            Value::Empty => 4,
        }
    }
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        self.rank()
            .cmp(&other.rank())
            .then_with(|| match (&self.0, &other.0) {
                (Value::Number(left), Value::Number(right)) => left.total_cmp(right),
                (Value::Text(left), Value::Text(right)) => left
                    .to_lowercase()
                    .cmp(&right.to_lowercase())
                    .then_with(|| left.cmp(right)),
                (Value::Bool(left), Value::Bool(right)) => left.cmp(right),
                (Value::Error(left), Value::Error(right)) => left.code().cmp(right.code()),
                _ => Ordering::Equal,
            })
    }
}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Key {}

/// A field of a source range: its name in the header and its distinct items, ordered as a
/// pivot table shows them.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceField {
    pub name: String,
    pub items: Vec<Value>,
}

impl Sheet {
    /// The pivot tables the sheet shows.
    #[must_use]
    pub fn pivot_tables(&self) -> &[PivotTable] {
        &self.pivot_tables
    }
}

impl Workbook {
    /// The fields of a range a pivot table could read: the header's names and the distinct
    /// items of each column below it, at most [`MAX_PIVOT_ROW_KEYS`] each.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::InvalidPivot`] for a range a pivot table cannot
    /// read.
    pub fn pivot_source_fields(
        &self,
        sheet: usize,
        range: CellRange,
    ) -> Result<Vec<SourceField>, EditError> {
        let name = self
            .sheets
            .get(sheet)
            .ok_or(EditError::NoSuchSheet(sheet))?
            .name
            .clone();
        let probe = PivotTable::new("", &name, range, CellRef::new(0, 0));
        if !probe.is_valid() {
            return Err(EditError::InvalidPivot);
        }
        let source = probe.read_source(self)?;
        let Some((header, records)) = source.split_first() else {
            return Err(EditError::InvalidPivot);
        };
        Ok(header
            .iter()
            .enumerate()
            .map(|(index, name)| SourceField {
                name: item_label(name),
                items: records
                    .iter()
                    .map(|record| Key(record.get(index).cloned().unwrap_or(Value::Empty)))
                    .collect::<BTreeSet<Key>>()
                    .into_iter()
                    .take(MAX_PIVOT_ROW_KEYS)
                    .map(|key| key.0)
                    .collect(),
            })
            .collect())
    }

    /// Add a pivot table to a sheet and compute it into its cells; its index among the sheet's
    /// pivot tables.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::InvalidPivot`] for a table whose source sheet is
    /// missing, whose fields fall outside its source or repeat on its axes, whose cells would
    /// cover its source, or past [`MAX_PIVOT_TABLES`], [`EditError::OutOfBounds`] past the grid.
    pub fn add_pivot_table(&mut self, sheet: usize, table: PivotTable) -> Result<usize, EditError> {
        let count = self
            .sheets
            .get(sheet)
            .ok_or(EditError::NoSuchSheet(sheet))?
            .pivot_tables
            .len();
        if count >= MAX_PIVOT_TABLES {
            return Err(EditError::InvalidPivot);
        }
        let computed = self.computed_pivot(sheet, table)?;
        self.place_pivot(sheet, None, computed);
        Ok(count)
    }

    /// Change a pivot table of a sheet and compute it again, its old cells cleared.
    ///
    /// # Errors
    ///
    /// As [`Workbook::add_pivot_table`], and [`EditError::InvalidPivot`] for an index the sheet
    /// has no table at.
    pub fn set_pivot_table(
        &mut self,
        sheet: usize,
        index: usize,
        table: PivotTable,
    ) -> Result<(), EditError> {
        self.pivot_at(sheet, index)?;
        let computed = self.computed_pivot(sheet, table)?;
        self.place_pivot(sheet, Some(index), computed);
        Ok(())
    }

    /// Compute a pivot table of a sheet again from its source.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::InvalidPivot`] for an index the sheet has no
    /// table at or a source that is gone.
    pub fn refresh_pivot_table(&mut self, sheet: usize, index: usize) -> Result<(), EditError> {
        let table = self.pivot_at(sheet, index)?.clone();
        let computed = self.computed_pivot(sheet, table)?;
        self.place_pivot(sheet, Some(index), computed);
        Ok(())
    }

    /// Compute every pivot table of the workbook again; a table whose source is gone keeps its
    /// cells. How many tables were refreshed.
    pub fn refresh_pivot_tables(&mut self) -> usize {
        let mut refreshed = 0;
        for sheet in 0..self.sheets.len() {
            for index in 0..self.sheets[sheet].pivot_tables.len() {
                refreshed += usize::from(self.refresh_pivot_table(sheet, index).is_ok());
            }
        }
        refreshed
    }

    /// Remove a pivot table and the cells it filled.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::InvalidPivot`] for an index the sheet has no
    /// table at.
    pub fn remove_pivot_table(&mut self, sheet: usize, index: usize) -> Result<(), EditError> {
        self.pivot_at(sheet, index)?;
        let table = self.sheets[sheet].pivot_tables.remove(index);
        if let Some(area) = table.area {
            clear_area(&mut self.sheets[sheet], area);
        }
        self.recalculate();
        Ok(())
    }

    fn pivot_at(&self, sheet: usize, index: usize) -> Result<&PivotTable, EditError> {
        self.sheets
            .get(sheet)
            .ok_or(EditError::NoSuchSheet(sheet))?
            .pivot_tables
            .get(index)
            .ok_or(EditError::InvalidPivot)
    }

    /// A table checked and computed: the table with its source kept, and its cells.
    fn computed_pivot(
        &self,
        sheet: usize,
        mut table: PivotTable,
    ) -> Result<(PivotTable, Vec<Vec<Value>>), EditError> {
        if sheet >= self.sheets.len() {
            return Err(EditError::NoSuchSheet(sheet));
        }
        if !table.is_valid() {
            return Err(EditError::InvalidPivot);
        }
        let source = table.read_source(self)?;
        let grid = table.layout(&source)?;
        let width = u32::try_from(grid.iter().map(Vec::len).max().unwrap_or(1).max(1))
            .map_err(|_| EditError::OutOfBounds)?;
        let height = u32::try_from(grid.len().max(1)).map_err(|_| EditError::OutOfBounds)?;
        let area = CellRange {
            first_row: table.anchor.row,
            first_column: table.anchor.column,
            last_row: table.anchor.row + height - 1,
            last_column: table.anchor.column + width - 1,
        };
        let source_sheet = self.sheet_index(&table.source_sheet);
        if source_sheet == Some(sheet) && overlaps(area, table.source) {
            return Err(EditError::InvalidPivot);
        }
        table.snapshot = source;
        table.area = Some(area);
        Ok((table, grid))
    }

    /// Write a computed table into its sheet, in place of the table at `index` when given.
    fn place_pivot(
        &mut self,
        sheet: usize,
        index: Option<usize>,
        (mut table, grid): (PivotTable, Vec<Vec<Value>>),
    ) {
        let target = &mut self.sheets[sheet];
        let previous = index.and_then(|index| target.pivot_tables[index].area);
        if let Some(previous) = previous {
            clear_area(target, previous);
        }
        table.area = Some(write_grid(target, table.anchor, &grid));
        match index {
            Some(index) => target.pivot_tables[index] = table,
            None => target.pivot_tables.push(table),
        }
        self.recalculate();
    }

    /// Keep the source of every pivot table as it is now, for `GETPIVOTDATA`, without writing
    /// their cells: what a file read gives the tables it carries.
    pub(crate) fn snapshot_pivot_tables(&mut self) {
        for sheet in 0..self.sheets.len() {
            for index in 0..self.sheets[sheet].pivot_tables.len() {
                let table = &self.sheets[sheet].pivot_tables[index];
                if let Ok(source) = table.read_source(self) {
                    self.sheets[sheet].pivot_tables[index].snapshot = source;
                }
            }
        }
    }
}

fn overlaps(first: CellRange, second: CellRange) -> bool {
    first.first_row <= second.last_row
        && second.first_row <= first.last_row
        && first.first_column <= second.last_column
        && second.first_column <= first.last_column
}

/// Empty the cells of an area, their looks kept.
fn clear_area(sheet: &mut Sheet, area: CellRange) {
    sheet.cells.retain(|cell, _| {
        !(area.first_row..=area.last_row).contains(&cell.row)
            || !(area.first_column..=area.last_column).contains(&cell.column)
    });
}

/// Write a grid of values from `anchor`, each cell keeping its number format; the area it
/// covers.
fn write_grid(sheet: &mut Sheet, anchor: CellRef, grid: &[Vec<Value>]) -> CellRange {
    let width = grid.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let height = grid.len().max(1);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the layout is checked against the grid's limits"
    )]
    let area = CellRange {
        first_row: anchor.row,
        first_column: anchor.column,
        last_row: anchor.row + height as u32 - 1,
        last_column: anchor.column + width as u32 - 1,
    };
    clear_area(sheet, area);
    for (row, line) in grid.iter().enumerate() {
        for (column, value) in line.iter().enumerate() {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "the layout is checked against the grid's limits"
            )]
            let cell = CellRef::new(anchor.row + row as u32, anchor.column + column as u32);
            let input = match value {
                Value::Empty => continue,
                Value::Number(number) => Input::Number(*number),
                Value::Text(text) => Input::Text(text.clone()),
                Value::Bool(flag) => Input::Bool(*flag),
                Value::Error(kind) => Input::Formula(Box::new(Expr::Error(*kind))),
            };
            sheet.cells.insert(
                cell,
                Cell {
                    input,
                    format: NumberFormat::General,
                },
            );
        }
    }
    area
}

#[cfg(test)]
pub(crate) mod tests;
