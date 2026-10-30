// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The functions that summarise like a pivot table: `GETPIVOTDATA` reads a value of a pivot
//! table of the workbook, `GROUPBY` and `PIVOTBY` summarise arrays by their rows and columns.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::rc::Rc;

use super::lambda::Closure;
use super::{Evaluator, MAX_SPILL_CELLS, Operand};
use crate::formula::{Expr, FUNCTION_NAMES, canonical_function_name};
use crate::pivots::{Key, item_label};
use crate::value::{ErrorKind, Value};

type Grid = Vec<Vec<Value>>;

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

    /// The functions a `GROUPBY` or `PIVOTBY` summarises with: one, or a row or column of them
    /// stacked with `HSTACK` or `VSTACK` or written as an array.
    fn summaries(&self, sheet: usize, argument: Option<&Expr>) -> Result<Vec<Summary>, ErrorKind> {
        let argument = argument.ok_or(ErrorKind::Value)?;
        let parts: Vec<&Expr> = match argument {
            Expr::Call(name, parts)
                if matches!(canonical_function_name(name).as_str(), "HSTACK" | "VSTACK") =>
            {
                parts.iter().collect()
            }
            Expr::Array(rows) => rows.iter().flatten().collect(),
            single => vec![single],
        };
        if parts.is_empty() || parts.len() > MAX_SUMMARIES {
            return Err(ErrorKind::Value);
        }
        parts
            .into_iter()
            .map(|part| self.summary(sheet, part))
            .collect()
    }

    /// One function: a function of the grid by its name, `SUM`, or a `LAMBDA`.
    fn summary(&self, sheet: usize, argument: &Expr) -> Result<Summary, ErrorKind> {
        if let Expr::Name(name) = argument {
            let english = canonical_function_name(name);
            if FUNCTION_NAMES.binary_search(&english.as_str()).is_ok() {
                let pair = english == "PERCENTOF";
                return Ok(Summary {
                    caption: english.clone(),
                    kind: SummaryKind::Named(english),
                    pair,
                });
            }
        }
        match self.evaluate(sheet, argument) {
            Operand::Lambda(closure) => Ok(Summary {
                caption: "LAMBDA".to_owned(),
                pair: closure.arity() >= 2,
                kind: SummaryKind::Lambda(closure),
            }),
            Operand::Value(Value::Error(kind)) => Err(kind),
            _ => Err(ErrorKind::Value),
        }
    }

    /// A group's values summarised into one value; a function of two arguments, `PERCENTOF`,
    /// also gets the values the group is measured against.
    fn summarise(&self, sheet: usize, summary: &Summary, values: &[Value], all: &[Value]) -> Value {
        let column = |values: &[Value]| -> Vec<Vec<Expr>> {
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
            column
        };
        let operand = match &summary.kind {
            SummaryKind::Named(name) => {
                let mut arguments = vec![Expr::Array(column(values))];
                if summary.pair {
                    arguments.push(Expr::Array(column(all)));
                }
                self.evaluate(sheet, &Expr::Call(name.clone(), arguments))
            }
            SummaryKind::Lambda(closure) => {
                let grid = |values: &[Value]| -> Grid {
                    values.iter().map(|value| vec![value.clone()]).collect()
                };
                let mut arguments = vec![Some(Operand::Array(grid(values)))];
                if summary.pair {
                    arguments.push(Some(Operand::Array(grid(all))));
                }
                self.invoke(sheet, closure, arguments)
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
        Ok(self
            .wholes(sheet, argument)?
            .first()
            .copied()
            .unwrap_or(default))
    }

    /// An optional argument of whole numbers, one or an array of them; none when left out.
    fn wholes(&self, sheet: usize, argument: Option<&Expr>) -> Result<Vec<i64>, ErrorKind> {
        let Some(argument) = argument else {
            return Ok(Vec::new());
        };
        if matches!(argument, Expr::Text(text) if text.is_empty()) {
            return Ok(Vec::new());
        }
        let values: Vec<Value> = match self.evaluate(sheet, argument) {
            Operand::Array(grid) => grid.into_iter().flatten().collect(),
            _ => vec![self.scalar(sheet, argument)],
        };
        values
            .into_iter()
            .filter(|value| !matches!(value, Value::Empty))
            .map(|value| {
                let number = value.as_number()?;
                if !number.is_finite() || number.abs() > 1e9 {
                    return Err(ErrorKind::Value);
                }
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "the number is checked finite and small"
                )]
                Ok(number.trunc() as i64)
            })
            .collect()
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

    /// The summaries of a set of records, one per function and value column, function by
    /// function, each measured against `all`.
    fn summarised(
        &self,
        sheet: usize,
        data: &Data<'_>,
        records: &[usize],
        all: &[usize],
    ) -> Vec<Value> {
        let cells = |rows: &[usize], column: usize| -> Vec<Value> {
            rows.iter()
                .map(|row| {
                    data.values[*row]
                        .get(column)
                        .cloned()
                        .unwrap_or(Value::Empty)
                })
                .collect()
        };
        let mut line = Vec::with_capacity(data.summaries.len() * data.width);
        for summary in data.summaries {
            for column in 0..data.width {
                let all = if summary.pair {
                    cells(all, column)
                } else {
                    Vec::new()
                };
                line.push(self.summarise(sheet, summary, &cells(records, column), &all));
            }
        }
        line
    }

    /// `GROUPBY(row_fields, values, function, [field_headers], [total_depth], [sort_order],
    /// [filter_array], [field_relationship])`: a row per distinct key of the row fields, each
    /// value column summarised, with totals and subtotals above or below.
    fn group_by(&self, sheet: usize, arguments: &[Expr]) -> Result<Grid, ErrorKind> {
        if !(3..=8).contains(&arguments.len()) {
            return Err(ErrorKind::Value);
        }
        let keys = self.grid(sheet, arguments.first())?;
        let values = self.grid(sheet, arguments.get(1))?;
        if keys.len() != values.len() || keys.is_empty() {
            return Err(ErrorKind::Value);
        }
        let summaries = self.summaries(sheet, arguments.get(2))?;
        let headers = self.whole(sheet, arguments.get(3), -1)?;
        let depth = self.whole(sheet, arguments.get(4), 1)?;
        let sort = self.wholes(sheet, arguments.get(5))?;
        let kept = self.kept_rows(sheet, arguments.get(6), keys.len())?;
        let table = match self.whole(sheet, arguments.get(7), 0)? {
            0 => false,
            1 => true,
            _ => return Err(ErrorKind::Value),
        };
        let (has_header, show_header) = header_mode(headers, &values)?;
        let key_width = keys[0].len();
        let data = Data {
            values: &values,
            width: values[0].len(),
            summaries: &summaries,
        };
        let outputs = data.width * summaries.len();
        check_depth(depth, key_width, table)?;
        check_sort(&sort, key_width + outputs)?;
        let records: Vec<usize> = (usize::from(has_header)..keys.len())
            .filter(|row| kept[*row])
            .collect();
        let mut tree = Node::build(&keys, &records);
        tree.measure(&|node| self.summarised(sheet, &data, &node.records, &records));
        let mut leaf_of = vec![0; keys.len()];
        tree.prepare(table, &sort, key_width, &mut leaf_of);
        let mut grid: Grid = Vec::new();
        if show_header {
            let mut line: Vec<Value> = if has_header {
                keys[0].clone()
            } else {
                (1..=key_width)
                    .map(|index| Value::Text(format!("Row Field {index}")))
                    .collect()
            };
            line.extend(captions(&values, has_header, &summaries));
            grid.push(line);
        }
        let mut rows = Vec::new();
        tree.lines(depth, &mut rows);
        for (label, node) in rows {
            let mut line = label;
            line.resize(key_width, Value::Empty);
            line.extend(node.measures.iter().cloned());
            grid.push(line);
        }
        checked(grid)
    }

    /// `PIVOTBY(row_fields, col_fields, values, function, [field_headers], [row_total_depth],
    /// [row_sort_order], [col_total_depth], [col_sort_order], [filter_array], [relative_to])`:
    /// the values summarised by the keys of the row fields down and of the column fields across.
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
        let summaries = self.summaries(sheet, arguments.get(3))?;
        let headers = self.whole(sheet, arguments.get(4), -1)?;
        let row_depth = self.whole(sheet, arguments.get(5), 1)?;
        let row_sort = self.wholes(sheet, arguments.get(6))?;
        let column_depth = self.whole(sheet, arguments.get(7), 1)?;
        let column_sort = self.wholes(sheet, arguments.get(8))?;
        let kept = self.kept_rows(sheet, arguments.get(9), rows)?;
        let relative = self.whole(sheet, arguments.get(10), 0)?;
        if !(0..=4).contains(&relative) {
            return Err(ErrorKind::Value);
        }
        let (has_header, show_header) = header_mode(headers, &values)?;
        let row_width = row_keys[0].len();
        let column_width = column_keys[0].len();
        let data = Data {
            values: &values,
            width: values[0].len(),
            summaries: &summaries,
        };
        let outputs = data.width * summaries.len();
        check_depth(row_depth, row_width, false)?;
        check_depth(column_depth, column_width, false)?;
        check_sort(&row_sort, row_width + outputs)?;
        check_sort(&column_sort, column_width + outputs)?;
        let records: Vec<usize> = (usize::from(has_header)..rows)
            .filter(|row| kept[*row])
            .collect();
        let measure = |node: &Node| self.summarised(sheet, &data, &node.records, &records);
        let mut row_tree = Node::build(&row_keys, &records);
        row_tree.measure(&measure);
        let mut column_tree = Node::build(&column_keys, &records);
        column_tree.measure(&measure);
        let mut scratch = vec![0; rows];
        row_tree.prepare(false, &row_sort, row_width, &mut scratch);
        // Each record's column leaf, and each column line as a run of leaves.
        let mut leaf_of = vec![usize::MAX; rows];
        column_tree.prepare(false, &column_sort, column_width, &mut leaf_of);
        let leaves = column_tree.leaves.end.max(1);
        let mut row_lines = Vec::new();
        row_tree.lines(row_depth, &mut row_lines);
        let mut column_lines = Vec::new();
        column_tree.lines(column_depth, &mut column_lines);
        if row_lines.len() * (column_lines.len() * outputs + row_width) > MAX_SPILL_CELLS {
            return Err(ErrorKind::Num);
        }
        let mut grid: Grid = Vec::new();
        for level in 0..column_width {
            let mut line = vec![Value::Empty; row_width];
            for (label, _) in &column_lines {
                let shown = label.get(level).cloned().unwrap_or(Value::Empty);
                line.extend(std::iter::repeat_n(shown, outputs));
            }
            grid.push(line);
        }
        if show_header || outputs > 1 {
            let mut line: Vec<Value> = if has_header {
                row_keys[0].clone()
            } else {
                vec![Value::Empty; row_width]
            };
            let captions = captions(&values, has_header, &summaries);
            for _ in &column_lines {
                line.extend(captions.iter().cloned());
            }
            grid.push(line);
        }
        let buckets_of = |records: &[usize]| -> Vec<Vec<usize>> {
            let mut buckets = vec![Vec::new(); leaves];
            for row in records {
                if let Some(bucket) = buckets.get_mut(leaf_of[*row]) {
                    bucket.push(*row);
                }
            }
            buckets
        };
        let gather = |buckets: &[Vec<usize>], range: std::ops::Range<usize>| -> Vec<usize> {
            let mut rows: Vec<usize> = buckets[range].iter().flatten().copied().collect();
            rows.sort_unstable();
            rows
        };
        let all_buckets = buckets_of(&records);
        let parent_rows = relative == 4 && summaries.iter().any(|summary| summary.pair);
        let mut parent_cache: Option<(Vec<Key>, Vec<Vec<usize>>)> = None;
        for (label, row) in &row_lines {
            let mut line = label.clone();
            line.resize(row_width, Value::Empty);
            let buckets = buckets_of(&row.records);
            if parent_rows {
                let parent = row_tree.parent_of(row).unwrap_or(&row_tree);
                if parent_cache
                    .as_ref()
                    .is_none_or(|(cached, _)| *cached != parent.key)
                {
                    parent_cache = Some((parent.key.clone(), buckets_of(&parent.records)));
                }
            }
            for (_, column) in &column_lines {
                let range = column.leaves.clone();
                let inside = gather(&buckets, range.clone());
                if inside.is_empty() {
                    line.extend(std::iter::repeat_n(Value::Empty, outputs));
                    continue;
                }
                let against = match relative {
                    1 => row.records.clone(),
                    2 => records.clone(),
                    3 => {
                        let parent = column_tree.parent_of(column).unwrap_or(&column_tree);
                        gather(&buckets, parent.leaves.clone())
                    }
                    4 => parent_cache
                        .as_ref()
                        .map(|(_, buckets)| gather(buckets, range))
                        .unwrap_or_default(),
                    _ => gather(&all_buckets, range),
                };
                line.extend(self.summarised(sheet, &data, &inside, &against));
            }
            grid.push(line);
        }
        checked(grid)
    }
}

/// The most functions one `GROUPBY` or `PIVOTBY` applies.
const MAX_SUMMARIES: usize = 64;

/// What summarises a group, the caption its columns take when there are several, and whether it
/// also takes the values the group is measured against.
struct Summary {
    kind: SummaryKind,
    caption: String,
    pair: bool,
}

enum SummaryKind {
    Named(String),
    Lambda(Rc<Closure>),
}

/// The values a `GROUPBY` or `PIVOTBY` summarises, their width and its functions.
struct Data<'a> {
    values: &'a Grid,
    width: usize,
    summaries: &'a [Summary],
}

/// The captions over the summarised columns: the header of each value column, or `Value 1`, and
/// with several functions each function's name too.
fn captions(values: &Grid, has_header: bool, summaries: &[Summary]) -> Vec<Value> {
    let width = values[0].len();
    let bases: Vec<String> = (0..width)
        .map(|column| {
            if has_header {
                item_label(&values[0][column])
            } else {
                format!("Value {}", column + 1)
            }
        })
        .collect();
    let mut captions = Vec::new();
    for summary in summaries {
        for base in &bases {
            captions.push(Value::Text(if summaries.len() == 1 {
                base.clone()
            } else if width == 1 && !has_header {
                summary.caption.clone()
            } else {
                format!("{base} {}", summary.caption)
            }));
        }
    }
    captions
}

/// A total depth a `GROUPBY` or `PIVOTBY` takes: 0 none, 1 the grand total, 2 and deeper a
/// subtotal per level too, negative at the top; subtotals need the hierarchy.
fn check_depth(depth: i64, width: usize, table: bool) -> Result<(), ErrorKind> {
    let deepest = i64::try_from(width.max(2)).map_err(|_| ErrorKind::Value)?;
    if depth.abs() > deepest || (table && depth.abs() >= 2) {
        return Err(ErrorKind::Value);
    }
    Ok(())
}

/// A sort order names columns from 1, negative for descending.
fn check_sort(sort: &[i64], width: usize) -> Result<(), ErrorKind> {
    let width = i64::try_from(width).map_err(|_| ErrorKind::Value)?;
    if sort
        .iter()
        .any(|column| *column == 0 || column.abs() > width)
    {
        return Err(ErrorKind::Value);
    }
    Ok(())
}

/// The records grouped by the keys of the fields, a level per field, each group with its
/// summaries over every function and value column.
struct Node {
    key: Vec<Key>,
    records: Vec<usize>,
    children: Vec<Node>,
    measures: Vec<Value>,
    /// The leaves of the tree this group covers, once numbered.
    leaves: std::ops::Range<usize>,
}

impl Node {
    fn build(keys: &Grid, records: &[usize]) -> Self {
        let width = keys.first().map_or(0, Vec::len);
        Self::grouped(keys, width, records.to_vec(), Vec::new())
    }

    fn grouped(keys: &Grid, width: usize, records: Vec<usize>, key: Vec<Key>) -> Self {
        let mut children = Vec::new();
        if key.len() < width {
            let mut items: BTreeMap<Key, Vec<usize>> = BTreeMap::new();
            for row in &records {
                let item = keys[*row].get(key.len()).cloned().unwrap_or(Value::Empty);
                items.entry(Key(item)).or_default().push(*row);
            }
            children = items
                .into_iter()
                .map(|(item, rows)| {
                    let mut child = key.clone();
                    child.push(item);
                    Self::grouped(keys, width, rows, child)
                })
                .collect();
        }
        Self {
            key,
            records,
            children,
            measures: Vec::new(),
            leaves: 0..0,
        }
    }

    fn measure(&mut self, summarise: &dyn Fn(&Self) -> Vec<Value>) {
        self.measures = summarise(self);
        for child in &mut self.children {
            child.measure(summarise);
        }
    }

    /// Number the leaves as the lines will show them, giving each record its leaf's number.
    fn number_leaves(&mut self, next: &mut usize, leaf_of: &mut [usize]) {
        let first = *next;
        if self.children.is_empty() {
            for row in &self.records {
                leaf_of[*row] = *next;
            }
            *next += 1;
        } else {
            for child in &mut self.children {
                child.number_leaves(next, leaf_of);
            }
        }
        self.leaves = first..*next;
    }

    /// The group one level up from a group of this tree.
    fn parent_of(&self, node: &Self) -> Option<&Self> {
        if node.key.is_empty() {
            return None;
        }
        let mut parent = self;
        for depth in 0..node.key.len() - 1 {
            parent = parent
                .children
                .iter()
                .find(|child| child.key[depth] == node.key[depth])?;
        }
        Some(parent)
    }

    /// Order the groups by the sort order: each level by its own key column or by a summary,
    /// down for a negative column, ascending by key otherwise; with `table` the items as one
    /// list, each field sorted on its own.
    fn sort(&mut self, sort: &[i64], width: usize, table: bool) {
        let level = self.key.len();
        let order = |left: &Self, right: &Self| -> Ordering {
            for column in sort {
                let index = usize::try_from(column.unsigned_abs()).unwrap_or(usize::MAX) - 1;
                let order = if index < width {
                    if table || index == level {
                        left.key.get(index).cmp(&right.key.get(index))
                    } else {
                        Ordering::Equal
                    }
                } else {
                    let pick = |node: &Self| {
                        Key(node
                            .measures
                            .get(index - width)
                            .cloned()
                            .unwrap_or(Value::Empty))
                    };
                    pick(left).cmp(&pick(right))
                };
                let order = if *column < 0 { order.reverse() } else { order };
                if order != Ordering::Equal {
                    return order;
                }
            }
            left.key.cmp(&right.key)
        };
        self.children.sort_by(|left, right| order(left, right));
        if !table {
            for child in &mut self.children {
                child.sort(sort, width, table);
            }
        }
    }

    /// Sort the groups, as one list of items with `table`, and number the leaves in order.
    fn prepare(&mut self, table: bool, sort: &[i64], width: usize, leaf_of: &mut [usize]) {
        if table {
            let mut leaves = Vec::new();
            self.take_leaves(&mut leaves);
            self.children = leaves;
        }
        self.sort(sort, width, table);
        let mut next = 0;
        self.number_leaves(&mut next, leaf_of);
    }

    /// The lines a total depth shows, each its labels and its group, in order.
    fn lines<'n>(&'n self, depth: i64, lines: &mut Vec<(Vec<Value>, &'n Self)>) {
        let subtotals = usize::try_from(depth.unsigned_abs())
            .unwrap_or(0)
            .saturating_sub(1);
        self.walk(depth, subtotals, lines);
    }

    fn take_leaves(&mut self, leaves: &mut Vec<Self>) {
        if self.children.is_empty() {
            if !self.key.is_empty() {
                leaves.push(Self {
                    key: std::mem::take(&mut self.key),
                    records: std::mem::take(&mut self.records),
                    children: Vec::new(),
                    measures: std::mem::take(&mut self.measures),
                    leaves: 0..0,
                });
            }
        } else {
            for child in &mut self.children {
                child.take_leaves(leaves);
            }
        }
    }

    fn walk<'n>(&'n self, depth: i64, subtotals: usize, lines: &mut Vec<(Vec<Value>, &'n Self)>) {
        let level = self.key.len();
        if level > 0 && self.children.is_empty() {
            lines.push((self.key.iter().map(|key| key.0.clone()).collect(), self));
            return;
        }
        let total = if level == 0 {
            (depth != 0).then(|| vec![Value::Text(TOTAL.to_owned())])
        } else {
            (level <= subtotals).then(|| self.key.iter().map(|key| key.0.clone()).collect())
        };
        if depth < 0
            && let Some(label) = &total
        {
            lines.push((label.clone(), self));
        }
        for child in &self.children {
            child.walk(depth, subtotals, lines);
        }
        if depth > 0
            && let Some(label) = total
        {
            lines.push((label, self));
        }
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
