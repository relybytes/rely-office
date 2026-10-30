// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

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

    /// What a cell of the table shows for the values of its records: empty when every one is
    /// blank, as in Excel, whatever the function, else the function over them.
    pub(crate) fn summarise_cell(self, values: &[Value]) -> Value {
        if values.iter().all(|value| matches!(value, Value::Empty)) {
            Value::Empty
        } else {
            self.summarise(values)
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
            // Plus zero, so the sum of no numbers is 0 and not -0.
            Self::Sum => numbers.iter().sum::<f64>() + 0.0,
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

/// How a pivot table lays out its row labels, Excel's three forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PivotLayout {
    /// The row labels in one column, each field indented under the one before: Excel's default.
    #[default]
    Compact,
    /// A column per row field, each group on its own row above its items.
    Outline,
    /// A column per row field, a group's label beside its first item.
    Tabular,
}

impl PivotLayout {
    pub const ALL: [Self; 3] = [Self::Compact, Self::Outline, Self::Tabular];

    /// The stable name an API passes: `compact`, `outline`, `tabular`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Outline => "outline",
            Self::Tabular => "tabular",
        }
    }

    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|layout| layout.name() == name)
    }
}

/// Where a pivot table shows the total of each group of an outer field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PivotSubtotals {
    /// On the group's own row; in the tabular form, which has none, below the group.
    #[default]
    Top,
    /// On a row of its own below the group.
    Bottom,
    /// Not shown.
    Off,
}

impl PivotSubtotals {
    pub const ALL: [Self; 3] = [Self::Top, Self::Bottom, Self::Off];

    /// The stable name an API passes: `top`, `bottom`, `none`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Off => "none",
        }
    }

    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|subtotals| subtotals.name() == name)
    }
}

/// The axis a table with several value fields lays them along, Excel's `Σ Values`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PivotValuesAxis {
    /// A column per value under each column of labels: Excel's default.
    #[default]
    Columns,
    /// A row per value under each row of labels.
    Rows,
}

impl PivotValuesAxis {
    pub const ALL: [Self; 2] = [Self::Columns, Self::Rows];

    /// The stable name an API passes: `columns`, `rows`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Columns => "columns",
            Self::Rows => "rows",
        }
    }

    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|axis| axis.name() == name)
    }
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
    pub layout: PivotLayout,
    pub subtotals: PivotSubtotals,
    /// Every row shows the labels of the groups it belongs to, not only a group's first row; the
    /// compact form, with one label column, ignores it.
    pub repeat_labels: bool,
    /// The axis the values lay along when there are several.
    pub values_axis: PivotValuesAxis,
    /// Where `Σ Values` sits among the fields of its axis, from 0 for the outermost; after the
    /// last field when `None` or past them; on the column axis likewise among the column fields.
    pub values_position: Option<usize>,
    /// The language of the captions the table writes: `Grand Total`, `Totale complessivo`.
    pub locale: Locale,
    /// The cells the table filled when it was last computed.
    pub(crate) area: Option<CellRange>,
    /// The source as it was last computed, its header first, which `GETPIVOTDATA` reads.
    pub(crate) snapshot: Vec<Vec<Value>>,
}

impl PivotTable {
    /// A table of `source` on `source_sheet` starting at `anchor`, with nothing placed yet, both
    /// grand totals shown, in the compact form with subtotals at the top of their groups.
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
            layout: PivotLayout::default(),
            subtotals: PivotSubtotals::default(),
            repeat_labels: false,
            values_axis: PivotValuesAxis::default(),
            values_position: None,
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

    /// Whether the values lay along the rows: several of them, `Σ Values` on the row axis.
    pub(crate) fn values_in_rows(&self) -> bool {
        self.values.len() > 1 && self.values_axis == PivotValuesAxis::Rows
    }

    /// Where `Σ Values` sits among the row fields, when the values lay along the rows.
    fn values_row_level(&self) -> Option<usize> {
        self.values_in_rows().then(|| {
            self.values_position
                .map_or(self.rows.len(), |position| position.min(self.rows.len()))
        })
    }

    /// What each level of the row axis holds, outermost first: a row field, or the values.
    fn row_entries(&self) -> Vec<RowEntry> {
        let mut entries: Vec<RowEntry> = (0..self.rows.len()).map(RowEntry::Field).collect();
        if let Some(level) = self.values_row_level() {
            entries.insert(level, RowEntry::Values);
        }
        entries
    }

    /// The columns the row labels take: one in the compact form, one per row field and one for
    /// the values when they lay along the rows otherwise, none without either.
    pub(crate) fn label_columns(&self) -> usize {
        let entries = self.rows.len() + usize::from(self.values_in_rows());
        if entries == 0 {
            0
        } else if self.layout == PivotLayout::Compact {
            1
        } else {
            entries
        }
    }

    /// The rows of the header above the first row of labels: the captions, the column labels of
    /// each column field, and the value captions when there are several across.
    pub(crate) fn header_rows(&self) -> usize {
        if self.columns.is_empty() || self.values.is_empty() {
            1
        } else {
            1 + self.columns.len() + usize::from(self.values.len() > 1 && !self.values_in_rows())
        }
    }

    /// The caption of `Σ Values` as a header shows it.
    fn values_caption(&self) -> &'static str {
        pick(self.italian(), "Valori", "Values")
    }

    fn total_of(&self, label: &str) -> String {
        if self.italian() {
            format!("Totale {label}")
        } else {
            format!("{label} Total")
        }
    }

    /// The table's cells from its top-left corner, Excel's way: the filters and a blank row, the
    /// header, a row per group and item of the row fields as the layout sets, the grand total.
    #[expect(
        clippy::too_many_lines,
        reason = "the filters, the header, the rows and their values are laid out in one pass"
    )]
    pub(crate) fn layout(&self, source: &[Vec<Value>]) -> Result<PivotGrid, EditError> {
        let Some((header, records)) = source.split_first() else {
            return Err(EditError::InvalidPivot);
        };
        let kept = self.kept_records(records);
        let all: Vec<usize> = (0..kept.len()).collect();
        let row_tree = Group::build(&kept, &self.rows, all.clone());
        let column_tree = Group::build(&kept, &self.columns, all.clone());
        if row_tree.leaves() > MAX_PIVOT_ROW_KEYS || column_tree.leaves() > MAX_PIVOT_COLUMN_KEYS {
            return Err(EditError::InvalidPivot);
        }
        let name_of = |field: u32| {
            header
                .get(field as usize)
                .map(item_label)
                .unwrap_or_default()
        };
        let mut grid = PivotGrid::default();
        for filter in &self.filters {
            grid.cells.push(vec![
                Value::Text(name_of(filter.field)),
                self.filter_state(filter, records),
            ]);
        }
        if !self.filters.is_empty() {
            grid.cells.push(Vec::new());
        }
        let labels = self.label_columns();
        let column_area = !self.columns.is_empty() && !self.values.is_empty();
        let entries = self.row_entries();
        let in_rows = self.values_in_rows();
        let row_headers: Vec<Value> = if entries.is_empty() {
            Vec::new()
        } else if self.layout == PivotLayout::Compact {
            vec![Value::Text(
                pick(self.italian(), "Etichette di riga", "Row Labels").to_owned(),
            )]
        } else {
            entries
                .iter()
                .map(|entry| match entry {
                    RowEntry::Field(index) => Value::Text(name_of(self.rows[*index].field)),
                    RowEntry::Values => Value::Text(self.values_caption().to_owned()),
                })
                .collect()
        };
        let captions: Vec<String> = self
            .values
            .iter()
            .map(|value| self.value_caption(value, header))
            .collect();
        // The leaf of the column tree each record falls in, and the columns of values across.
        let mut leaf_of = vec![0_usize; kept.len()];
        let several = captions.len() > 1 && !in_rows;
        let (columns, leaves) = if column_area {
            let mut next = 0;
            column_tree.number_leaves(&mut next, &mut leaf_of);
            let column_entries = self.column_entries(several);
            let mut walk = ColumnWalk {
                entries: &column_entries,
                captions: &captions,
                fresh: vec![false; column_entries.len()],
                path: Vec::new(),
                next_leaf: 0,
                columns: Vec::new(),
            };
            self.value_columns(&column_tree, None, &mut walk);
            let mut columns = walk.columns;
            if self.column_grand_totals {
                let all = 0..column_tree.leaves();
                let heading = |text: String| {
                    let mut heading = vec![Value::Empty; column_entries.len()];
                    heading[0] = Value::Text(text);
                    heading
                };
                if several {
                    columns.extend(captions.iter().enumerate().map(|(index, caption)| {
                        ValueColumn {
                            leaves: all.clone(),
                            value: Some(index),
                            heading: heading(format!(
                                "{} {caption}",
                                pick(self.italian(), "Totale", "Total")
                            )),
                        }
                    }));
                } else {
                    columns.push(ValueColumn {
                        leaves: all,
                        value: None,
                        heading: heading(self.grand_total().to_owned()),
                    });
                }
            }
            (columns, column_tree.leaves())
        } else if in_rows {
            // One column of values: each row says which value it shows.
            (
                vec![ValueColumn {
                    leaves: 0..1,
                    value: None,
                    heading: Vec::new(),
                }],
                1,
            )
        } else {
            let columns = (0..captions.len())
                .map(|index| ValueColumn {
                    leaves: 0..1,
                    value: Some(index),
                    heading: Vec::new(),
                })
                .collect();
            (columns, 1)
        };
        if column_area {
            let mut top = vec![Value::Empty; labels];
            if let (Some(first), [caption]) = (top.first_mut(), captions.as_slice()) {
                *first = Value::Text(caption.clone());
            }
            if self.layout == PivotLayout::Compact {
                top.push(Value::Text(
                    pick(self.italian(), "Etichette di colonna", "Column Labels").to_owned(),
                ));
            } else {
                top.extend(
                    self.columns
                        .iter()
                        .map(|field| Value::Text(name_of(field.field))),
                );
            }
            grid.cells.push(top);
            let levels = self.columns.len() + usize::from(several);
            for level in 0..levels {
                let mut line = if level + 1 == levels {
                    row_headers.clone()
                } else {
                    Vec::new()
                };
                line.resize(labels, Value::Empty);
                line.extend(
                    columns
                        .iter()
                        .map(|column| column.heading.get(level).cloned().unwrap_or(Value::Empty)),
                );
                grid.cells.push(line);
            }
        } else {
            let mut line = row_headers;
            line.resize(labels, Value::Empty);
            if in_rows {
                // One column of values under a blank caption: each row says which value it shows.
                line.push(Value::Empty);
            } else {
                line.extend(captions.iter().cloned().map(Value::Text));
            }
            grid.cells.push(line);
        }
        let mut lines = Vec::new();
        if entries.is_empty() {
            lines.push(Line {
                labels: Vec::new(),
                indent: 0,
                records: Some(all.clone()),
                value: None,
            });
        } else {
            let mut pending = vec![false; entries.len()];
            let root = RowNode {
                group: &row_tree,
                value: None,
                path: Vec::new(),
            };
            self.row_lines(&root, &entries, &captions, &mut pending, &mut lines);
            if self.row_grand_totals && !self.rows.is_empty() {
                let totals: Vec<(String, Option<usize>)> = if in_rows {
                    captions
                        .iter()
                        .enumerate()
                        .map(|(index, caption)| {
                            (
                                format!("{} {caption}", pick(self.italian(), "Totale", "Total")),
                                Some(index),
                            )
                        })
                        .collect()
                } else {
                    vec![(self.grand_total().to_owned(), None)]
                };
                for (text, value) in totals {
                    let mut total = vec![Value::Text(text)];
                    total.resize(labels, Value::Empty);
                    lines.push(Line {
                        labels: total,
                        indent: 0,
                        records: Some(all.clone()),
                        value,
                    });
                }
            }
        }
        for line in lines {
            let mut cells = line.labels;
            cells.resize(labels, Value::Empty);
            if let Some(rows) = &line.records {
                let mut buckets: Vec<Vec<&Vec<Value>>> = vec![Vec::new(); leaves];
                for row in rows {
                    buckets[if column_area { leaf_of[*row] } else { 0 }].push(kept[*row]);
                }
                for column in &columns {
                    let Some(value) = self
                        .values
                        .get(column.value.or(line.value).unwrap_or_default())
                    else {
                        continue;
                    };
                    let inside: Vec<&Vec<Value>> = buckets[column.leaves.clone()]
                        .iter()
                        .flatten()
                        .copied()
                        .collect();
                    cells.push(if inside.is_empty() {
                        Value::Empty
                    } else {
                        let figures: Vec<Value> = inside
                            .iter()
                            .map(|record| {
                                record
                                    .get(value.field as usize)
                                    .cloned()
                                    .unwrap_or(Value::Empty)
                            })
                            .collect();
                        value.function.summarise_cell(&figures)
                    });
                }
            }
            if line.indent > 0 {
                grid.indents.push((grid.cells.len(), line.indent));
            }
            grid.cells.push(cells);
        }
        let width = grid.cells.iter().map(Vec::len).max().unwrap_or(0);
        if u64::from(self.anchor.row) + grid.cells.len() as u64 > u64::from(MAX_ROWS)
            || u64::from(self.anchor.column) + width as u64 > u64::from(MAX_COLUMNS)
        {
            return Err(EditError::OutOfBounds);
        }
        Ok(grid)
    }

    /// What a filter shows: `(All)`, its one item, or `(Multiple Items)`.
    fn filter_state(&self, filter: &PivotField, records: &[Vec<Value>]) -> Value {
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
            .map(|record| item_label(record.get(filter.field as usize).unwrap_or(&Value::Empty)))
            .collect::<BTreeSet<String>>()
            .len();
        if items.len() == all_items {
            Value::Text(pick(self.italian(), "(Tutto)", "(All)").to_owned())
        } else if items.len() == 1 {
            items.first().map_or(Value::Empty, |key| self.shown(key))
        } else {
            Value::Text(pick(self.italian(), "(Più elementi)", "(Multiple Items)").to_owned())
        }
    }

    /// An item as a label shows it: a blank as `(blank)`.
    fn shown(&self, key: &Key) -> Value {
        match &key.0 {
            Value::Empty => Value::Text(pick(self.italian(), "(vuoto)", "(blank)").to_owned()),
            other => other.clone(),
        }
    }

    /// What each level of the column axis holds, outermost first: a column field, or the values
    /// when there are several across, at their place among the fields.
    fn column_entries(&self, several: bool) -> Vec<RowEntry> {
        let mut entries: Vec<RowEntry> = (0..self.columns.len()).map(RowEntry::Field).collect();
        if several {
            let at = self
                .values_position
                .map_or(entries.len(), |position| position.min(entries.len()));
            entries.insert(at, RowEntry::Values);
        }
        entries
    }

    /// The columns of values under a node of the column axis, as the header shows them: a column
    /// per item of the innermost level, each label over the first column of its group, then the
    /// node's subtotals when a column field lies below it. `value` is the value the node shows
    /// when `Σ Values` lies above it.
    fn value_columns(&self, group: &Group, value: Option<usize>, walk: &mut ColumnWalk<'_>) {
        let depth = walk.path.len();
        if depth == walk.entries.len() {
            let heading = walk
                .path
                .iter()
                .zip(&walk.fresh)
                .map(|(label, fresh)| if *fresh { label.clone() } else { Value::Empty })
                .collect();
            walk.fresh.fill(false);
            walk.columns.push(ValueColumn {
                leaves: walk.next_leaf..walk.next_leaf + 1,
                value,
                heading,
            });
            walk.next_leaf += 1;
            return;
        }
        let first_leaf = walk.next_leaf;
        match walk.entries[depth] {
            RowEntry::Values => {
                for (index, caption) in walk.captions.iter().enumerate() {
                    // The leaves of the group again, under each value.
                    walk.next_leaf = first_leaf;
                    walk.path.push(Value::Text(caption.clone()));
                    walk.fresh[depth] = true;
                    self.value_columns(group, Some(index), walk);
                    walk.path.pop();
                }
            }
            RowEntry::Field(_) => {
                for child in &group.children {
                    walk.path
                        .push(child.key.last().map_or(Value::Empty, |key| self.shown(key)));
                    walk.fresh[depth] = true;
                    self.value_columns(child, value, walk);
                    walk.path.pop();
                }
            }
        }
        let field_node = depth > 0 && matches!(walk.entries[depth - 1], RowEntry::Field(_));
        let deeper = walk.entries[depth..]
            .iter()
            .any(|entry| matches!(entry, RowEntry::Field(_)));
        if !field_node || !deeper || self.subtotals == PivotSubtotals::Off {
            return;
        }
        let Some(label) = walk.path.last().map(item_label) else {
            return;
        };
        let leaves = first_leaf..walk.next_leaf;
        let several = walk.entries.contains(&RowEntry::Values);
        let totals: Vec<(String, Option<usize>)> = if value.is_some() || !several {
            vec![(self.total_of(&label), value)]
        } else {
            // `Σ Values` lies below: a subtotal column per value, `Nord Sum of Amount`.
            walk.captions
                .iter()
                .enumerate()
                .map(|(index, caption)| (format!("{label} {caption}"), Some(index)))
                .collect()
        };
        for (text, value) in totals {
            let mut heading = vec![Value::Empty; walk.entries.len()];
            heading[depth - 1] = Value::Text(text);
            walk.columns.push(ValueColumn {
                leaves: leaves.clone(),
                value,
                heading,
            });
        }
    }

    /// The rows of a node of the row axis as the layout sets them: its own row, its children,
    /// its subtotals. A node is a group of the row fields, with the value it shows when
    /// `Σ Values` lies above it.
    #[expect(
        clippy::too_many_lines,
        reason = "the three forms place a node's own row, children and subtotals in one pass"
    )]
    fn row_lines(
        &self,
        node: &RowNode<'_>,
        entries: &[RowEntry],
        captions: &[String],
        pending: &mut [bool],
        lines: &mut Vec<Line>,
    ) {
        let path = &node.path;
        let depth = path.len();
        let compact = self.layout == PivotLayout::Compact;
        let in_rows = self.values_in_rows();
        let labelled = |column: usize, text: Value| -> Vec<Value> {
            if compact {
                return vec![text];
            }
            let mut labels = vec![Value::Empty; entries.len()];
            if self.repeat_labels {
                labels[..column].clone_from_slice(&path[..column]);
            }
            labels[column] = text;
            labels
        };
        #[expect(clippy::cast_possible_truncation, reason = "at most 65 levels deep")]
        let indent = if compact {
            depth.saturating_sub(1) as u8
        } else {
            0
        };
        let label = path.last().cloned();
        if depth == entries.len() {
            let Some(label) = label else {
                return;
            };
            let mut labels = labelled(depth - 1, label);
            if self.layout == PivotLayout::Tabular {
                for (level, shown) in path.iter().enumerate().take(depth - 1) {
                    if pending[level] || self.repeat_labels {
                        labels[level] = shown.clone();
                    }
                    pending[level] = false;
                }
            }
            lines.push(Line {
                labels,
                indent,
                records: Some(node.group.records.clone()),
                value: node.value,
            });
            return;
        }
        // The node is an item of `Σ Values` when the level above it holds the values.
        let values_node = depth > 0 && entries[depth - 1] == RowEntry::Values;
        // Whether one value is known here: the values lay along the columns, or above it.
        let known = !in_rows || node.value.is_some();
        // A group has totals of its own only over a field below it: when nothing but the
        // values lies below, their rows are its totals.
        let deeper = entries[depth..]
            .iter()
            .any(|entry| matches!(entry, RowEntry::Field(_)));
        if depth > 0 {
            if self.layout == PivotLayout::Tabular {
                pending[depth - 1] = true;
            } else if let Some(label) = label.clone() {
                lines.push(Line {
                    labels: labelled(depth - 1, label),
                    indent,
                    records: (self.subtotals == PivotSubtotals::Top
                        && known
                        && deeper
                        && !values_node)
                        .then(|| node.group.records.clone()),
                    value: node.value,
                });
            }
        }
        match entries[depth] {
            RowEntry::Values => {
                for (index, caption) in captions.iter().enumerate() {
                    let mut below = path.clone();
                    below.push(Value::Text(caption.clone()));
                    let child = RowNode {
                        group: node.group,
                        value: Some(index),
                        path: below,
                    };
                    self.row_lines(&child, entries, captions, pending, lines);
                }
            }
            RowEntry::Field(_) => {
                for group in &node.group.children {
                    let mut below = path.clone();
                    below.push(group.key.last().map_or(Value::Empty, |key| self.shown(key)));
                    let child = RowNode {
                        group,
                        value: node.value,
                        path: below,
                    };
                    self.row_lines(&child, entries, captions, pending, lines);
                }
            }
        }
        if depth == 0 || values_node || !deeper {
            return;
        }
        let below = match self.subtotals {
            PivotSubtotals::Off => false,
            PivotSubtotals::Top => self.layout == PivotLayout::Tabular || !known,
            PivotSubtotals::Bottom => true,
        };
        let Some(label) = label.filter(|_| below) else {
            return;
        };
        let text = item_label(&label);
        let totals: Vec<(String, Option<usize>)> = if known {
            vec![(self.total_of(&text), node.value)]
        } else {
            // `Σ Values` lies below: a subtotal row per value, `Nord Sum of Amount`.
            captions
                .iter()
                .enumerate()
                .map(|(index, caption)| (format!("{text} {caption}"), Some(index)))
                .collect()
        };
        for (text, value) in totals {
            lines.push(Line {
                labels: labelled(depth - 1, Value::Text(text)),
                indent,
                records: Some(node.group.records.clone()),
                value,
            });
        }
    }
}

/// What a level of the row axis holds: a row field, by its place among them, or `Σ Values`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowEntry {
    Field(usize),
    Values,
}

/// A node of the row axis: a group of the row fields, the value it shows when `Σ Values` lies
/// above it, and the labels of the levels down to it.
struct RowNode<'g> {
    group: &'g Group,
    value: Option<usize>,
    path: Vec<Value>,
}

/// The cells a pivot table writes, and the rows whose label is indented, by how many steps.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct PivotGrid {
    pub(crate) cells: Vec<Vec<Value>>,
    pub(crate) indents: Vec<(usize, u8)>,
}

/// The records of the table grouped by the items of an axis's fields, a level per field.
struct Group {
    key: Vec<Key>,
    records: Vec<usize>,
    children: Vec<Group>,
}

impl Group {
    fn build(records: &[&Vec<Value>], fields: &[PivotField], within: Vec<usize>) -> Self {
        Self::grouped(records, fields, within, Vec::new())
    }

    fn grouped(
        records: &[&Vec<Value>],
        fields: &[PivotField],
        within: Vec<usize>,
        key: Vec<Key>,
    ) -> Self {
        let mut children = Vec::new();
        if let Some(field) = fields.get(key.len()) {
            let mut items: BTreeMap<Key, Vec<usize>> = BTreeMap::new();
            for row in &within {
                let item = records[*row]
                    .get(field.field as usize)
                    .cloned()
                    .unwrap_or(Value::Empty);
                items.entry(Key(item)).or_default().push(*row);
            }
            children = items
                .into_iter()
                .map(|(item, rows)| {
                    let mut child = key.clone();
                    child.push(item);
                    Self::grouped(records, fields, rows, child)
                })
                .collect();
        }
        Self {
            key,
            records: within,
            children,
        }
    }

    /// The items of the last field under this group; one for a group with no field below.
    fn leaves(&self) -> usize {
        if self.children.is_empty() {
            1
        } else {
            self.children.iter().map(Self::leaves).sum()
        }
    }

    /// Number the leaves in order, giving each record the number of its leaf.
    fn number_leaves(&self, next: &mut usize, leaf_of: &mut [usize]) {
        if self.children.is_empty() {
            for row in &self.records {
                leaf_of[*row] = *next;
            }
            *next += 1;
        } else {
            for child in &self.children {
                child.number_leaves(next, leaf_of);
            }
        }
    }
}

/// A column of the value area: the range of the column tree's leaves it sums, the value it shows
/// (`None` for the one of its row), and what the header shows over it at each level.
struct ValueColumn {
    leaves: std::ops::Range<usize>,
    value: Option<usize>,
    heading: Vec<Value>,
}

/// The walk of the column axis that lays out its columns of values.
struct ColumnWalk<'w> {
    entries: &'w [RowEntry],
    captions: &'w [String],
    /// Whether the next column is the first under the node at each level, which shows its label.
    fresh: Vec<bool>,
    /// The labels from the outermost level down to the node walked.
    path: Vec<Value>,
    /// The column tree's leaf the next column sums.
    next_leaf: usize,
    columns: Vec<ValueColumn>,
}

/// A row of the row area: its labels, the indent of its label in the compact form, the records
/// its values summarise, `None` for a group's row that shows no values, and the one value it
/// shows when the values lay along the rows.
struct Line {
    labels: Vec<Value>,
    indent: u8,
    records: Option<Vec<usize>>,
    value: Option<usize>,
}

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
            self.set_indents(sheet, area, &[]);
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
    ) -> Result<(PivotTable, PivotGrid), EditError> {
        if sheet >= self.sheets.len() {
            return Err(EditError::NoSuchSheet(sheet));
        }
        if !table.is_valid() {
            return Err(EditError::InvalidPivot);
        }
        let source = table.read_source(self)?;
        let grid = table.layout(&source)?;
        let width = u32::try_from(grid.cells.iter().map(Vec::len).max().unwrap_or(1).max(1))
            .map_err(|_| EditError::OutOfBounds)?;
        let height = u32::try_from(grid.cells.len().max(1)).map_err(|_| EditError::OutOfBounds)?;
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
        (mut table, grid): (PivotTable, PivotGrid),
    ) {
        let previous = index.and_then(|index| self.sheets[sheet].pivot_tables[index].area);
        if let Some(previous) = previous {
            clear_area(&mut self.sheets[sheet], previous);
            self.set_indents(sheet, previous, &[]);
        }
        let area = write_grid(&mut self.sheets[sheet], table.anchor, &grid.cells);
        let indents: Vec<(CellRef, u8)> = grid
            .indents
            .iter()
            .map(|(row, indent)| {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "the layout is checked against the grid's limits"
                )]
                let row = table.anchor.row + *row as u32;
                (CellRef::new(row, table.anchor.column), *indent)
            })
            .collect();
        self.set_indents(sheet, area, &indents);
        table.area = Some(area);
        let target = &mut self.sheets[sheet];
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

impl Workbook {
    /// Indent the labels a pivot table indents and take the indent off the other cells of its
    /// area, their other looks kept.
    fn set_indents(&mut self, sheet: usize, area: CellRange, indents: &[(CellRef, u8)]) {
        let inside = |cell: &CellRef| {
            (area.first_row..=area.last_row).contains(&cell.row)
                && (area.first_column..=area.last_column).contains(&cell.column)
        };
        let mut changes: Vec<(CellRef, u8)> = self.sheets[sheet]
            .styles
            .iter()
            .filter(|(cell, id)| {
                inside(cell)
                    && self
                        .styles
                        .get(**id as usize)
                        .is_some_and(|style| style.indent > 0)
            })
            .map(|(cell, _)| (*cell, 0))
            .collect();
        changes.extend(indents.iter().copied());
        for (cell, indent) in changes {
            let mut style = self.style(sheet, cell).clone();
            style.indent = indent;
            let id = self.intern_style(style);
            if id == 0 {
                self.sheets[sheet].styles.remove(&cell);
            } else {
                self.sheets[sheet].styles.insert(cell, id);
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
