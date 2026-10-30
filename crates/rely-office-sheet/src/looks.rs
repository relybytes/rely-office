// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! What the conditional formats of a sheet make each cell look like, computed from the current
//! values: the rules in priority order, a rule outdoing the later ones property by property,
//! colour scales blended between their stops, data bars and icons placed by their thresholds.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;

use rely_office_core::{
    CellRange, Condition, ConditionalFormat, ConditionalRule, DifferentialLook, ScalePoint,
    ScaleValue, TextOperator,
};

use crate::address::CellRef;
use crate::dates::date_from_serial;
use crate::evaluate::FormulaContext;
use crate::formula::number_text;
use crate::model::Workbook;
use crate::rules::{anchor_of, compares, formula_in, truthy};
use crate::value::Value;

/// The most cells one rule is checked for; the rest of a larger range keeps its own look.
const MAX_RULE_CELLS: usize = 200_000;

/// How a cell looks under the conditional formats of its sheet; unset keeps its own look.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize),
    serde(rename_all = "camelCase")
)]
pub struct CellLook {
    /// `RRGGBB`.
    pub fill: Option<String>,
    pub font_colour: Option<String>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    pub number_format: Option<String>,
    pub bar: Option<DataBarLook>,
    pub icon: Option<IconLook>,
}

/// A data bar in a cell.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize),
    serde(rename_all = "camelCase")
)]
pub struct DataBarLook {
    /// The share of the cell's width the bar fills, from 0.1 to 0.9 as Excel draws it.
    pub length: f64,
    pub colour: String,
    pub show_value: bool,
}

/// An icon of a set in a cell.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize),
    serde(rename_all = "camelCase")
)]
pub struct IconLook {
    /// The set, `3Arrows`.
    pub set: String,
    /// The icon of the set from the lowest, `0` the first.
    pub index: usize,
    pub show_value: bool,
}

impl CellLook {
    fn take(&mut self, look: &DifferentialLook) {
        fn first<T: Clone>(own: &mut Option<T>, other: Option<&T>) {
            if own.is_none() {
                *own = other.cloned();
            }
        }
        first(&mut self.fill, look.fill.as_ref());
        first(&mut self.font_colour, look.font_colour.as_ref());
        first(&mut self.bold, look.bold.as_ref());
        first(&mut self.italic, look.italic.as_ref());
        first(&mut self.underline, look.underline.as_ref());
        first(&mut self.strike, look.strike.as_ref());
        first(&mut self.number_format, look.number_format.as_ref());
    }
}

impl Workbook {
    /// The look the conditional formats give each cell of a sheet that one changes, by cell.
    #[must_use]
    pub fn conditional_looks(&self, sheet: usize) -> BTreeMap<CellRef, CellLook> {
        let mut looks: BTreeMap<CellRef, CellLook> = BTreeMap::new();
        let Some(target) = self.sheets.get(sheet) else {
            return looks;
        };
        let mut rules: Vec<(&ConditionalFormat, &ConditionalRule)> = target
            .conditional_formats
            .iter()
            .flat_map(|format| format.rules.iter().map(move |rule| (format, rule)))
            .collect();
        rules.sort_by_key(|(_, rule)| applying_order(rule.priority));
        let context = FormulaContext::new(self);
        let extent = target.extent();
        let mut stopped: HashSet<CellRef> = HashSet::new();
        for (format, rule) in rules {
            let cells = cells_of(&format.ranges, extent);
            let checker = RuleCheck::new(self, &context, sheet, format, rule, &cells);
            for cell in &cells {
                if stopped.contains(cell) {
                    continue;
                }
                let Some(effect) = checker.effect(*cell) else {
                    continue;
                };
                let look = looks.entry(*cell).or_default();
                match effect {
                    Effect::Look => {
                        if let Some(own) = &rule.look {
                            look.take(own);
                        }
                    }
                    Effect::Fill(colour) => {
                        look.fill.get_or_insert(colour);
                    }
                    Effect::Bar(bar) => {
                        look.bar.get_or_insert(bar);
                    }
                    Effect::Icon(icon) => {
                        look.icon.get_or_insert(icon);
                    }
                }
                if rule.stop_if_true {
                    stopped.insert(*cell);
                }
            }
        }
        looks.retain(|_, look| *look != CellLook::default());
        looks
    }
}

/// Where a priority puts a rule: 0 is unset, after every rule that sets one, in listing order.
pub(crate) const fn applying_order(priority: u32) -> u32 {
    if priority == 0 { u32::MAX } else { priority }
}

/// The cells of the ranges inside the used area, each once, row by row, at most
/// [`MAX_RULE_CELLS`].
fn cells_of(ranges: &[CellRange], (rows, columns): (u32, u32)) -> Vec<CellRef> {
    let mut seen = HashSet::new();
    let mut cells = Vec::new();
    for range in ranges {
        let last_row = range.last_row.min(rows.saturating_sub(1));
        let last_column = range.last_column.min(columns.saturating_sub(1));
        if rows == 0
            || columns == 0
            || range.first_row > last_row
            || range.first_column > last_column
        {
            continue;
        }
        for row in range.first_row..=last_row {
            for column in range.first_column..=last_column {
                if cells.len() >= MAX_RULE_CELLS {
                    return cells;
                }
                let cell = CellRef::new(row, column);
                if seen.insert(cell) {
                    cells.push(cell);
                }
            }
        }
    }
    cells
}

enum Effect {
    Look,
    Fill(String),
    Bar(DataBarLook),
    Icon(IconLook),
}

/// One rule over its cells, with what it needs to know of all of them: their numbers, sorted,
/// and how often each value occurs.
struct RuleCheck<'a> {
    workbook: &'a Workbook,
    context: &'a FormulaContext<'a>,
    sheet: usize,
    anchor: CellRef,
    rule: &'a ConditionalRule,
    numbers: Vec<f64>,
    counts: HashMap<String, usize>,
}

impl<'a> RuleCheck<'a> {
    fn new(
        workbook: &'a Workbook,
        context: &'a FormulaContext<'a>,
        sheet: usize,
        format: &ConditionalFormat,
        rule: &'a ConditionalRule,
        cells: &[CellRef],
    ) -> Self {
        let mut numbers = Vec::new();
        let mut counts = HashMap::new();
        let needs_numbers = matches!(
            rule.condition,
            Condition::ColorScale { .. }
                | Condition::DataBar { .. }
                | Condition::IconSet { .. }
                | Condition::Top { .. }
                | Condition::Average { .. }
        );
        let needs_counts = matches!(rule.condition, Condition::Duplicates { .. });
        if needs_numbers || needs_counts {
            for cell in cells {
                match workbook.value(sheet, *cell) {
                    Value::Number(number) if needs_numbers => numbers.push(number),
                    Value::Empty => {}
                    value if needs_counts => {
                        *counts.entry(count_key(&value)).or_default() += 1;
                    }
                    _ => {}
                }
            }
            numbers.sort_by(f64::total_cmp);
        }
        Self {
            workbook,
            context,
            sheet,
            anchor: anchor_of(&format.ranges),
            rule,
            numbers,
            counts,
        }
    }

    fn formula(&self, formula: &str, cell: CellRef) -> Option<Value> {
        let expression = formula_in(formula, self.anchor, cell)?;
        Some(self.context.value(self.sheet, cell, &expression))
    }

    /// A point of a scale as a number of the rule's values; `percent` is what a plain number
    /// means, a share of the span for an icon set's thresholds.
    fn point(&self, value: &ScaleValue, cell: CellRef) -> Option<f64> {
        let (low, high) = (*self.numbers.first()?, *self.numbers.last()?);
        let number = |text: &str| {
            text.trim()
                .parse::<f64>()
                .ok()
                .filter(|number| number.is_finite())
        };
        Some(match value {
            ScaleValue::Min => low,
            ScaleValue::Max => high,
            ScaleValue::Number(text) => number(text)?,
            ScaleValue::Percent(text) => low + (high - low) * number(text)? / 100.0,
            ScaleValue::Percentile(text) => percentile(&self.numbers, number(text)? / 100.0),
            ScaleValue::Formula(formula) => self.formula(formula, cell)?.as_number().ok()?,
        })
    }

    #[expect(clippy::too_many_lines, reason = "one arm for each kind of rule")]
    fn effect(&self, cell: CellRef) -> Option<Effect> {
        let value = self.workbook.value(self.sheet, cell);
        let holds = |condition: bool| condition.then_some(Effect::Look);
        match &self.rule.condition {
            Condition::CellIs { operator, formulas } => {
                if matches!(value, Value::Error(_)) {
                    return None;
                }
                let operand = |index: usize| {
                    formulas
                        .get(index)
                        .and_then(|formula| self.formula(formula, cell))
                };
                holds(compares(*operator, &value, operand(0), operand(1)))
            }
            Condition::Expression { formula } => holds(
                self.formula(formula, cell)
                    .is_some_and(|value| truthy(&value)),
            ),
            Condition::ColorScale { points } => {
                let Value::Number(number) = value else {
                    return None;
                };
                self.scale_colour(points, number, cell).map(Effect::Fill)
            }
            Condition::DataBar {
                min,
                max,
                colour,
                show_value,
            } => {
                let Value::Number(number) = value else {
                    return None;
                };
                let (low, high) = (self.point(min, cell)?, self.point(max, cell)?);
                let share = if high > low {
                    ((number - low) / (high - low)).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                Some(Effect::Bar(DataBarLook {
                    length: 0.1 + 0.8 * share,
                    colour: colour.clone(),
                    show_value: *show_value,
                }))
            }
            Condition::IconSet {
                icons,
                thresholds,
                reverse,
                show_value,
            } => {
                let Value::Number(number) = value else {
                    return None;
                };
                let count = icons
                    .chars()
                    .next()
                    .and_then(|digit| digit.to_digit(10))
                    .and_then(|digit| usize::try_from(digit).ok())
                    .unwrap_or(thresholds.len())
                    .max(1);
                let mut index = 0;
                for (position, threshold) in thresholds.iter().enumerate().skip(1) {
                    if self
                        .point(threshold, cell)
                        .is_some_and(|threshold| number >= threshold)
                    {
                        index = position;
                    }
                }
                let index = index.min(count - 1);
                Some(Effect::Icon(IconLook {
                    set: icons.clone(),
                    index: if *reverse { count - 1 - index } else { index },
                    show_value: *show_value,
                }))
            }
            Condition::Top {
                rank,
                percent,
                bottom,
            } => {
                let Value::Number(number) = value else {
                    return None;
                };
                let total = self.numbers.len();
                let wanted = usize::try_from(*rank).unwrap_or(usize::MAX);
                let wanted = if *percent {
                    (total.saturating_mul(wanted) / 100).max(1)
                } else {
                    wanted
                }
                .min(total);
                if wanted == 0 {
                    return None;
                }
                holds(if *bottom {
                    number <= self.numbers[wanted - 1]
                } else {
                    number >= self.numbers[total - wanted]
                })
            }
            Condition::Average {
                below,
                or_equal,
                deviations,
            } => {
                let Value::Number(number) = value else {
                    return None;
                };
                #[expect(clippy::cast_precision_loss, reason = "a sheet holds far fewer values")]
                let count = self.numbers.len() as f64;
                if count == 0.0 {
                    return None;
                }
                let mean = self.numbers.iter().sum::<f64>() / count;
                let spread = deviations.map_or(0.0, |deviations| {
                    let variance = self
                        .numbers
                        .iter()
                        .map(|number| (number - mean).powi(2))
                        .sum::<f64>()
                        / count;
                    f64::from(deviations) * variance.sqrt()
                });
                holds(match (below, or_equal) {
                    (false, false) => number > mean + spread,
                    (false, true) => number >= mean + spread,
                    (true, false) => number < mean - spread,
                    (true, true) => number <= mean - spread,
                })
            }
            Condition::Text { operator, text } => {
                if matches!(value, Value::Error(_)) {
                    return None;
                }
                let shown = shown_text(&value).to_lowercase();
                let wanted = text.to_lowercase();
                holds(match operator {
                    TextOperator::Contains => shown.contains(&wanted),
                    TextOperator::NotContains => !shown.contains(&wanted),
                    TextOperator::BeginsWith => shown.starts_with(&wanted),
                    TextOperator::EndsWith => shown.ends_with(&wanted),
                })
            }
            Condition::Duplicates { unique } => {
                if value == Value::Empty {
                    return None;
                }
                let count = self.counts.get(&count_key(&value)).copied().unwrap_or(0);
                holds(if *unique { count == 1 } else { count > 1 })
            }
            Condition::Blanks { blank } => {
                let empty = match &value {
                    Value::Empty => true,
                    Value::Text(text) => text.trim().is_empty(),
                    _ => false,
                };
                holds(empty == *blank)
            }
            Condition::Errors { error } => holds(matches!(value, Value::Error(_)) == *error),
            Condition::TimePeriod { period } => {
                let (Value::Number(number), Some(today)) = (value, self.workbook.today) else {
                    return None;
                };
                holds(in_period(period, number.floor(), today.floor()))
            }
        }
    }

    fn scale_colour(&self, points: &[ScalePoint], number: f64, cell: CellRef) -> Option<String> {
        let mut stops: Vec<(f64, &str)> = points
            .iter()
            .filter_map(|point| Some((self.point(&point.value, cell)?, point.colour.as_str())))
            .collect();
        stops.sort_by(|first, second| first.0.total_cmp(&second.0));
        let (first, last) = (stops.first()?, stops.last()?);
        if number <= first.0 {
            return Some(first.1.to_owned());
        }
        if number >= last.0 {
            return Some(last.1.to_owned());
        }
        let segment = stops.windows(2).find(|pair| number <= pair[1].0)?;
        let (low, high) = (segment[0], segment[1]);
        let share = if high.0 > low.0 {
            (number - low.0) / (high.0 - low.0)
        } else {
            0.0
        };
        Some(blend(low.1, high.1, share))
    }
}

/// The value a sheet's ordering keys a value by, text without case.
fn count_key(value: &Value) -> String {
    match value {
        Value::Number(number) => format!("n{}", number_text(*number)),
        Value::Text(text) => format!("t{}", text.to_lowercase()),
        Value::Bool(value) => format!("b{value}"),
        Value::Error(kind) => format!("e{}", kind.code()),
        Value::Empty => String::new(),
    }
}

fn shown_text(value: &Value) -> String {
    match value {
        Value::Number(number) => number_text(*number),
        Value::Text(text) => text.clone(),
        Value::Bool(value) => if *value { "TRUE" } else { "FALSE" }.to_owned(),
        Value::Error(kind) => kind.code().to_owned(),
        Value::Empty => String::new(),
    }
}

/// The value at a share of sorted numbers, interpolated as `PERCENTILE.INC` does.
fn percentile(sorted: &[f64], share: f64) -> f64 {
    let share = share.clamp(0.0, 1.0);
    #[expect(clippy::cast_precision_loss, reason = "a sheet holds far fewer values")]
    let position = share * (sorted.len().saturating_sub(1)) as f64;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a position within the values"
    )]
    let below = position.floor() as usize;
    let above = (below + 1).min(sorted.len().saturating_sub(1));
    let fraction = position - position.floor();
    sorted.get(below).copied().unwrap_or(0.0) * (1.0 - fraction)
        + sorted.get(above).copied().unwrap_or(0.0) * fraction
}

/// Two `RRGGBB` colours mixed, `share` of the way from the first to the second.
fn blend(first: &str, second: &str, share: f64) -> String {
    let channel = |colour: &str, start: usize| {
        f64::from(u8::from_str_radix(colour.get(start..start + 2).unwrap_or("00"), 16).unwrap_or(0))
    };
    let mut mixed = String::with_capacity(6);
    for start in [0, 2, 4] {
        let value =
            channel(first, start) + (channel(second, start) - channel(first, start)) * share;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a channel is clamped to 0..=255"
        )]
        let value = value.round().clamp(0.0, 255.0) as u8;
        let _ = write!(mixed, "{value:02X}");
    }
    mixed
}

/// Whether a date serial falls in a period Excel names, from today's; a week starts on Sunday.
fn in_period(period: &str, date: f64, today: f64) -> bool {
    let month =
        |serial: f64| date_from_serial(serial).map(|(year, month, _)| year * 12 + month - 1);
    let (Some(month_of_date), Some(this_month)) = (month(date), month(today)) else {
        return false;
    };
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a date serial within the calendar, which date_from_serial checked"
    )]
    let (date, today) = (date.floor() as i64, today.floor() as i64);
    let week_start = |serial: i64| serial - (serial - 1).rem_euclid(7);
    match period {
        "today" => date == today,
        "yesterday" => date == today - 1,
        "tomorrow" => date == today + 1,
        "last7Days" => date <= today && date > today - 7,
        "thisWeek" => week_start(date) == week_start(today),
        "lastWeek" => week_start(date) == week_start(today) - 7,
        "nextWeek" => week_start(date) == week_start(today) + 7,
        "thisMonth" => month_of_date == this_month,
        "lastMonth" => month_of_date == this_month - 1,
        "nextMonth" => month_of_date == this_month + 1,
        _ => false,
    }
}

#[cfg(test)]
mod tests;
