// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! The formulas, conditional formats and validations of an ods in the notation and the model
//! an xlsx gives them: `of:=SUM([.A1:.B2];[$Foglio2.C3])` read as `SUM(A1:B2,Foglio2!C3)`, the
//! conditions of `calcext:conditional-formats` and the `table:content-validations` a cell
//! names.

use crate::sheet_layout::CellRange;
use crate::sheet_rules::{
    ComparisonOperator, Condition, ConditionalFormat, ConditionalRule, DataValidation,
    DifferentialLook, ErrorStyle, ScalePoint, ScaleValue, TextOperator, ValidationKind,
};
use crate::xml_tree::{Element, Namespace};

const CALCEXT: Namespace = Namespace::OdfCalcExt;
const TABLE: Namespace = Namespace::OdfTable;
const TEXT: Namespace = Namespace::OdfText;
/// The most rules a sheet keeps, as an xlsx sheet keeps them.
const MAX_RULES: usize = 10_000;
const MAX_RANGES: usize = 1_000;

/// A formula of an `OpenDocument` file, `of:=SUM([.A1:.B2];1)`, in Excel's notation without the
/// leading `=`, `SUM(A1:B2,1)`: references lose their brackets, a sheet is named before `!`,
/// arguments are separated by commas and Microsoft's functions lose their prefix.
#[must_use]
pub fn excel_formula_from_odf(formula: &str) -> String {
    let formula = formula.trim();
    let formula = formula
        .strip_prefix("of:")
        .or_else(|| formula.strip_prefix("oooc:"))
        .unwrap_or(formula);
    let formula = formula.strip_prefix('=').unwrap_or(formula);
    let characters: Vec<char> = formula.chars().collect();
    let mut excel = String::with_capacity(formula.len());
    let mut index = 0;
    while index < characters.len() {
        let character = characters[index];
        match character {
            '"' => {
                let end = string_end(&characters, index);
                excel.extend(&characters[index..end]);
                index = end;
            }
            '[' => {
                let end = bracket_end(&characters, index);
                let inner: String = characters[index + 1..end.saturating_sub(1).max(index + 1)]
                    .iter()
                    .collect();
                excel.push_str(&excel_reference(&inner));
                index = end;
            }
            ';' => {
                excel.push(',');
                index += 1;
            }
            letter if letter.is_ascii_alphabetic() || letter == '_' => {
                let start = index;
                while index < characters.len()
                    && (characters[index].is_ascii_alphanumeric()
                        || matches!(characters[index], '.' | '_'))
                {
                    index += 1;
                }
                let word: String = characters[start..index].iter().collect();
                let word = word
                    .strip_prefix("COM.MICROSOFT.")
                    .or_else(|| word.strip_prefix("com.microsoft."))
                    .unwrap_or(&word);
                excel.push_str(word);
            }
            other => {
                excel.push(other);
                index += 1;
            }
        }
    }
    excel
}

/// Where a string literal that opens at `start` ends, past its closing quote; a doubled quote
/// is a quote inside it.
fn string_end(characters: &[char], start: usize) -> usize {
    let mut index = start + 1;
    while index < characters.len() {
        if characters[index] == '"' {
            if characters.get(index + 1) == Some(&'"') {
                index += 2;
                continue;
            }
            return index + 1;
        }
        index += 1;
    }
    characters.len()
}

/// Where a reference that opens at `start` ends, past its `]`; a quoted sheet name may hold one.
fn bracket_end(characters: &[char], start: usize) -> usize {
    let mut index = start + 1;
    let mut quoted = false;
    while index < characters.len() {
        match characters[index] {
            '\'' => quoted = !quoted,
            ']' if !quoted => return index + 1,
            _ => {}
        }
        index += 1;
    }
    characters.len()
}

/// The parts of a text split at a character outside quotes.
fn split_outside_quotes(text: &str, separator: char) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut quoted = false;
    for character in text.chars() {
        match character {
            '\'' => {
                quoted = !quoted;
                if let Some(part) = parts.last_mut() {
                    part.push(character);
                }
            }
            found if found == separator && !quoted => parts.push(String::new()),
            other => {
                if let Some(part) = parts.last_mut() {
                    part.push(other);
                }
            }
        }
    }
    parts
}

/// A sheet and a cell of an `OpenDocument` reference, `$'Due fogli'.$A$1`: the sheet as Excel
/// names it, quoted when it has to be, and the cell.
fn sheet_and_cell(part: &str) -> (Option<String>, String) {
    let mut quoted = false;
    let mut split = None;
    for (position, character) in part.char_indices() {
        match character {
            '\'' => quoted = !quoted,
            '.' if !quoted => split = Some(position),
            _ => {}
        }
    }
    let Some(split) = split else {
        return (None, part.to_owned());
    };
    let sheet = part[..split].trim_start_matches('$');
    let cell = part[split + 1..].to_owned();
    if sheet.is_empty() {
        return (None, cell);
    }
    let plain = sheet
        .chars()
        .all(|character| character.is_alphanumeric() || character == '_');
    let sheet = if plain || sheet.starts_with('\'') {
        sheet.to_owned()
    } else {
        format!("'{}'", sheet.replace('\'', "''"))
    };
    (Some(sheet), cell)
}

fn excel_reference(inner: &str) -> String {
    let parts = split_outside_quotes(inner, ':');
    let mut sheet = None;
    let mut cells = Vec::new();
    for part in &parts {
        let (own, cell) = sheet_and_cell(part);
        if sheet.is_none() {
            sheet = own;
        }
        cells.push(cell);
    }
    let cells = cells.join(":");
    match sheet {
        Some(sheet) => format!("{sheet}!{cells}"),
        None => cells,
    }
}

/// The ranges of a `target-range-address`, `Foglio1.A1:Foglio1.B2 'Due fogli'.C3`, on whatever
/// sheet they name.
pub(super) fn odf_ranges(address: &str) -> Vec<CellRange> {
    split_outside_quotes(address.trim(), ' ')
        .iter()
        .filter(|part| !part.is_empty())
        .filter_map(|part| {
            let cells: Vec<String> = split_outside_quotes(part, ':')
                .iter()
                .map(|side| sheet_and_cell(side).1)
                .collect();
            CellRange::parse(&cells.join(":"))
        })
        .take(MAX_RANGES)
        .collect()
}

/// The arguments between the brackets of `name(...)` in a condition, split at top-level commas
/// or semicolons, each in Excel's notation; `None` when the text is not such a call.
fn call_arguments(value: &str, name: &str) -> Option<Vec<String>> {
    let inner = value
        .strip_prefix(name)?
        .strip_prefix('(')?
        .strip_suffix(')')?;
    let mut arguments = vec![String::new()];
    let mut depth = 0_i32;
    let mut quoted = false;
    for character in inner.chars() {
        match character {
            '"' => quoted = !quoted,
            '(' | '[' if !quoted => depth += 1,
            ')' | ']' if !quoted => depth -= 1,
            ',' | ';' if !quoted && depth == 0 => {
                arguments.push(String::new());
                continue;
            }
            _ => {}
        }
        if let Some(argument) = arguments.last_mut() {
            argument.push(character);
        }
    }
    Some(
        arguments
            .iter()
            .map(|argument| excel_formula_from_odf(argument))
            .collect(),
    )
}

/// The text of a quoted string literal, `"a ""b"""` as `a "b"`.
fn unquoted(literal: &str) -> String {
    let literal = literal.trim();
    literal
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(literal)
        .replace("\"\"", "\"")
}

/// The condition a `calcext:condition` value states.
fn condition_of(value: &str) -> Option<Condition> {
    let value = value.trim();
    let text_rule = |name: &str, operator| {
        call_arguments(value, name).map(|arguments| Condition::Text {
            operator,
            text: arguments
                .first()
                .map(|text| unquoted(text))
                .unwrap_or_default(),
        })
    };
    let ranked = |name: &str, percent: bool, bottom: bool| {
        call_arguments(value, name).map(|arguments| Condition::Top {
            rank: arguments
                .first()
                .and_then(|rank| rank.trim().parse().ok())
                .unwrap_or(10),
            percent,
            bottom,
        })
    };
    if let Some(arguments) = call_arguments(value, "formula-is") {
        return Some(Condition::Expression {
            formula: arguments.join(","),
        });
    }
    for (name, operator) in [
        ("not-between", ComparisonOperator::NotBetween),
        ("between", ComparisonOperator::Between),
    ] {
        if let Some(formulas) = call_arguments(value, name).filter(|found| found.len() == 2) {
            return Some(Condition::CellIs { operator, formulas });
        }
    }
    let average = |below: bool, or_equal: bool| Condition::Average {
        below,
        or_equal,
        deviations: None,
    };
    let found = match value {
        "above-average" => Some(average(false, false)),
        "below-average" => Some(average(true, false)),
        "above-equal-average" => Some(average(false, true)),
        "below-equal-average" => Some(average(true, true)),
        "duplicate" => Some(Condition::Duplicates { unique: false }),
        "unique" => Some(Condition::Duplicates { unique: true }),
        "is-error" => Some(Condition::Errors { error: true }),
        "is-no-error" => Some(Condition::Errors { error: false }),
        _ => None,
    };
    if found.is_some() {
        return found;
    }
    let named = text_rule("begins-with", TextOperator::BeginsWith)
        .or_else(|| text_rule("ends-with", TextOperator::EndsWith))
        .or_else(|| text_rule("not-contains-text", TextOperator::NotContains))
        .or_else(|| text_rule("contains-text", TextOperator::Contains))
        .or_else(|| ranked("top-elements", false, false))
        .or_else(|| ranked("bottom-elements", false, true))
        .or_else(|| ranked("top-percent", true, false))
        .or_else(|| ranked("bottom-percent", true, true));
    if named.is_some() {
        return named;
    }
    for (symbol, operator) in [
        ("!=", ComparisonOperator::NotEqual),
        (">=", ComparisonOperator::GreaterOrEqual),
        ("<=", ComparisonOperator::LessOrEqual),
        ("=", ComparisonOperator::Equal),
        (">", ComparisonOperator::Greater),
        ("<", ComparisonOperator::Less),
    ] {
        if let Some(rest) = value.strip_prefix(symbol) {
            return Some(Condition::CellIs {
                operator,
                formulas: vec![excel_formula_from_odf(rest)],
            });
        }
    }
    None
}

fn scale_value(entry: &Element) -> ScaleValue {
    let value = entry
        .attribute(CALCEXT, "value")
        .unwrap_or_default()
        .to_owned();
    match entry.attribute(CALCEXT, "type").unwrap_or_default() {
        "minimum" | "auto-minimum" => ScaleValue::Min,
        "maximum" | "auto-maximum" => ScaleValue::Max,
        "percentile" => ScaleValue::Percentile(value),
        "percent" => ScaleValue::Percent(value),
        "formula" => ScaleValue::Formula(excel_formula_from_odf(&value)),
        _ => ScaleValue::Number(value),
    }
}

fn hex_colour(value: Option<&str>) -> Option<String> {
    let hex = value?.trim().trim_start_matches('#');
    (hex.len() == 6 && hex.chars().all(|character| character.is_ascii_hexdigit()))
        .then(|| hex.to_ascii_uppercase())
}

fn shows_value(element: &Element) -> bool {
    element.attribute(CALCEXT, "show-value") != Some("false")
}

/// The period of a `calcext:date-is`, as Excel names it.
fn period_of(date: &str) -> Option<&'static str> {
    Some(match date {
        "today" => "today",
        "yesterday" => "yesterday",
        "tomorrow" => "tomorrow",
        "last-7-days" => "last7Days",
        "this-week" => "thisWeek",
        "last-week" => "lastWeek",
        "next-week" => "nextWeek",
        "this-month" => "thisMonth",
        "last-month" => "lastMonth",
        "next-month" => "nextMonth",
        _ => return None,
    })
}

/// The conditional formats of a table, each condition with the look of the cell style it
/// applies, which `look_of` reads by name; priorities follow the order the file lists them in.
pub(super) fn conditional_formats(
    table: &Element,
    look_of: &dyn Fn(&str) -> Option<DifferentialLook>,
) -> Vec<ConditionalFormat> {
    let Some(formats) = table.child(CALCEXT, "conditional-formats") else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let mut priority = 0;
    for format in formats
        .elements()
        .filter(|format| format.is(CALCEXT, "conditional-format"))
        .take(MAX_RULES)
    {
        let ranges = odf_ranges(
            format
                .attribute(CALCEXT, "target-range-address")
                .unwrap_or_default(),
        );
        let mut rules = Vec::new();
        for entry in format.elements() {
            let (condition, style) = match entry.name.as_str() {
                "condition" => (
                    entry.attribute(CALCEXT, "value").and_then(condition_of),
                    entry.attribute(CALCEXT, "apply-style-name"),
                ),
                "date-is" => (
                    entry
                        .attribute(CALCEXT, "date")
                        .and_then(period_of)
                        .map(|period| Condition::TimePeriod {
                            period: period.to_owned(),
                        }),
                    entry.attribute(CALCEXT, "style"),
                ),
                "color-scale" => {
                    let points: Vec<ScalePoint> = entry
                        .elements()
                        .filter(|point| point.is(CALCEXT, "color-scale-entry"))
                        .filter_map(|point| {
                            Some(ScalePoint {
                                value: scale_value(point),
                                colour: hex_colour(point.attribute(CALCEXT, "color"))?,
                            })
                        })
                        .collect();
                    (
                        (points.len() >= 2).then_some(Condition::ColorScale { points }),
                        None,
                    )
                }
                "data-bar" => {
                    let mut points = entry
                        .elements()
                        .filter(|point| point.is(CALCEXT, "formatting-entry"))
                        .map(scale_value);
                    (
                        Some(Condition::DataBar {
                            min: points.next().unwrap_or(ScaleValue::Min),
                            max: points.next().unwrap_or(ScaleValue::Max),
                            colour: hex_colour(entry.attribute(CALCEXT, "positive-color"))
                                .unwrap_or_else(|| "638EC6".to_owned()),
                            show_value: shows_value(entry),
                        }),
                        None,
                    )
                }
                "icon-set" => (
                    Some(Condition::IconSet {
                        icons: entry
                            .attribute(CALCEXT, "icon-set-type")
                            .unwrap_or("3TrafficLights1")
                            .to_owned(),
                        thresholds: entry
                            .elements()
                            .filter(|point| point.is(CALCEXT, "formatting-entry"))
                            .map(scale_value)
                            .collect(),
                        reverse: false,
                        show_value: shows_value(entry),
                    }),
                    None,
                ),
                _ => (None, None),
            };
            if let Some(condition) = condition {
                priority += 1;
                rules.push(ConditionalRule {
                    condition,
                    look: style.and_then(look_of),
                    priority,
                    stop_if_true: false,
                });
            }
        }
        if !ranges.is_empty() && !rules.is_empty() {
            found.push(ConditionalFormat { ranges, rules });
        }
    }
    found
}

/// A `table:content-validation` as a validation with no range yet: its ranges are the cells
/// that name it.
#[expect(
    clippy::too_many_lines,
    reason = "every condition a validation states, then its messages"
)]
pub(super) fn content_validation(validation: &Element) -> Option<(String, DataValidation)> {
    let name = validation.attribute(TABLE, "name")?.to_owned();
    let condition = validation.attribute(TABLE, "condition").unwrap_or_default();
    let condition = condition
        .strip_prefix("of:")
        .or_else(|| condition.strip_prefix("oooc:"))
        .unwrap_or(condition)
        .trim();
    let (kind, rest) = [
        ("cell-content-is-whole-number() and ", ValidationKind::Whole),
        (
            "cell-content-is-decimal-number() and ",
            ValidationKind::Decimal,
        ),
        ("cell-content-is-date() and ", ValidationKind::Date),
        ("cell-content-is-time() and ", ValidationKind::Time),
    ]
    .into_iter()
    .find_map(|(prefix, kind)| condition.strip_prefix(prefix).map(|rest| (kind, rest)))
    .unwrap_or((ValidationKind::Any, condition));
    let (kind, operator, formulas) =
        if let Some(items) = call_arguments(rest, "cell-content-is-in-list") {
            let quoted = items.iter().all(|item| item.trim().starts_with('"'));
            let formula = if quoted {
                format!(
                    "\"{}\"",
                    items
                        .iter()
                        .map(|item| unquoted(item).replace('"', "\"\""))
                        .collect::<Vec<_>>()
                        .join(",")
                )
            } else {
                items.join(",")
            };
            (
                ValidationKind::List,
                ComparisonOperator::Between,
                vec![formula],
            )
        } else if let Some(formula) = call_arguments(rest, "is-true-formula") {
            (
                ValidationKind::Custom,
                ComparisonOperator::Between,
                vec![formula.join(",")],
            )
        } else {
            let (kind, rest) = match rest.strip_prefix("cell-content-text-length") {
                Some(rest) => (ValidationKind::TextLength, rest),
                None => (kind, rest.strip_prefix("cell-content").unwrap_or(rest)),
            };
            if let Some(bounds) = call_arguments(rest, "-is-not-between") {
                (kind, ComparisonOperator::NotBetween, bounds)
            } else if let Some(bounds) = call_arguments(rest, "-is-between") {
                (kind, ComparisonOperator::Between, bounds)
            } else {
                let rest = rest.strip_prefix("()").unwrap_or(rest);
                let found = [
                    ("!=", ComparisonOperator::NotEqual),
                    (">=", ComparisonOperator::GreaterOrEqual),
                    ("<=", ComparisonOperator::LessOrEqual),
                    ("=", ComparisonOperator::Equal),
                    (">", ComparisonOperator::Greater),
                    ("<", ComparisonOperator::Less),
                ]
                .into_iter()
                .find_map(|(symbol, operator)| {
                    rest.strip_prefix(symbol)
                        .map(|operand| (operator, vec![excel_formula_from_odf(operand)]))
                });
                match found {
                    Some((operator, formulas)) => (kind, operator, formulas),
                    None => (kind, ComparisonOperator::Between, Vec::new()),
                }
            }
        };
    let message = |element: Option<&Element>| {
        element.map(|message| {
            let text: Vec<String> = message
                .elements()
                .filter(|paragraph| paragraph.is(TEXT, "p"))
                .map(Element::text)
                .collect();
            (
                message.attribute(TABLE, "display") == Some("true"),
                message
                    .attribute(TABLE, "title")
                    .filter(|title| !title.is_empty())
                    .map(str::to_owned),
                Some(text.join("\n")).filter(|text| !text.is_empty()),
            )
        })
    };
    let help = message(validation.child(TABLE, "help-message"));
    let error_element = validation.child(TABLE, "error-message");
    let error = message(error_element);
    Some((
        name,
        DataValidation {
            ranges: Vec::new(),
            kind,
            operator,
            formulas,
            allow_blank: validation.attribute(TABLE, "allow-empty-cell") != Some("false"),
            dropdown: validation.attribute(TABLE, "display-list") != Some("none"),
            error_style: match error_element
                .and_then(|element| element.attribute(TABLE, "message-type"))
            {
                Some("warning") => ErrorStyle::Warning,
                Some("information") => ErrorStyle::Information,
                _ => ErrorStyle::Stop,
            },
            show_error: error.as_ref().is_some_and(|error| error.0),
            error_title: error.as_ref().and_then(|error| error.1.clone()),
            error: error.and_then(|error| error.2),
            show_prompt: help.as_ref().is_some_and(|help| help.0),
            prompt_title: help.as_ref().and_then(|help| help.1.clone()),
            prompt: help.and_then(|help| help.2),
        },
    ))
}

/// Add a run of cells to the ranges of a validation, growing the last range down when the run
/// covers the same columns on the next row.
pub(super) fn add_validation_cells(ranges: &mut Vec<CellRange>, row: u32, first: u32, last: u32) {
    if let Some(previous) = ranges.last_mut()
        && previous.first_column == first
        && previous.last_column == last
        && previous.last_row + 1 == row
    {
        previous.last_row = row;
        return;
    }
    if ranges.len() < MAX_RANGES {
        ranges.push(CellRange {
            first_row: row,
            first_column: first,
            last_row: row,
            last_column: last,
        });
    }
}

#[cfg(test)]
mod tests;
