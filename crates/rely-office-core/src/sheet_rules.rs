// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! The rules a sheet puts on its cells: conditional formats, which change how a cell looks by
//! its value, and data validations, which limit what may be typed in it. Formulas are kept in
//! Excel's notation without the leading `=`, relative to the first cell of the first range.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::sheet_layout::CellRange;
use crate::sheet_styles::{ThemeColours, colour};
use crate::xml_text::attribute_value;

/// The most conditional formats and validations a sheet keeps, and the most ranges each covers.
pub(crate) const MAX_SHEET_RULES: usize = 10_000;
const MAX_RULE_RANGES: usize = 1_000;
const MAX_FORMULA_CHARS: usize = 8_192;
/// Excel's grid.
const LAST_ROW: u32 = 1_048_575;
const LAST_COLUMN: u32 = 16_383;

/// Rules that change the look of a set of ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct ConditionalFormat {
    pub ranges: Vec<CellRange>,
    pub rules: Vec<ConditionalRule>,
}

/// One rule of a conditional format.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct ConditionalRule {
    pub condition: Condition,
    /// The look a cell takes while the condition holds; a colour scale, a data bar and an icon
    /// set draw their own instead.
    #[cfg_attr(feature = "serde", serde(default))]
    pub look: Option<DifferentialLook>,
    /// Lower first: a rule outdoes, property by property, every rule after it.
    #[cfg_attr(feature = "serde", serde(default))]
    pub priority: u32,
    /// No rule after this one applies to a cell this one holds for.
    #[cfg_attr(feature = "serde", serde(default))]
    pub stop_if_true: bool,
}

/// When a rule holds, or how it shades its cells.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(
        tag = "kind",
        rename_all = "camelCase",
        rename_all_fields = "camelCase"
    )
)]
pub enum Condition {
    /// The value compared with one formula, or two for `between`.
    CellIs {
        operator: ComparisonOperator,
        formulas: Vec<String>,
    },
    /// A formula true for the cells it holds for, written for the first cell.
    Expression {
        formula: String,
    },
    /// A fill blended between two or three colours along the values of the ranges.
    ColorScale {
        points: Vec<ScalePoint>,
    },
    /// A bar as long as the value is within the ranges.
    DataBar {
        min: ScaleValue,
        max: ScaleValue,
        colour: String,
        show_value: bool,
    },
    /// One of the icons of a set (`3Arrows`, `3TrafficLights1`, `5Rating`), by threshold.
    IconSet {
        icons: String,
        thresholds: Vec<ScaleValue>,
        reverse: bool,
        show_value: bool,
    },
    /// The highest or lowest values, as a count or a share.
    Top {
        rank: u32,
        percent: bool,
        bottom: bool,
    },
    /// Above or below the mean of the ranges, by some standard deviations when set.
    Average {
        below: bool,
        or_equal: bool,
        deviations: Option<u32>,
    },
    Text {
        operator: TextOperator,
        text: String,
    },
    /// Values the ranges hold more than once, or only once.
    Duplicates {
        unique: bool,
    },
    Blanks {
        blank: bool,
    },
    Errors {
        error: bool,
    },
    /// A date in a period, `today`, `last7Days`, `thisMonth`, as Excel names them.
    TimePeriod {
        period: String,
    },
}

/// A stop of a colour scale: where it sits and its colour.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct ScalePoint {
    pub value: ScaleValue,
    /// `RRGGBB`.
    pub colour: String,
}

/// A point of a scale, a bar or an icon set, in the values of its ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(tag = "kind", content = "value", rename_all = "camelCase")
)]
pub enum ScaleValue {
    Min,
    Max,
    Number(String),
    Percent(String),
    Percentile(String),
    Formula(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub enum ComparisonOperator {
    #[default]
    Between,
    NotBetween,
    Equal,
    NotEqual,
    Greater,
    Less,
    GreaterOrEqual,
    LessOrEqual,
}

impl ComparisonOperator {
    /// The operator as a file names it, `greaterThan`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Between => "between",
            Self::NotBetween => "notBetween",
            Self::Equal => "equal",
            Self::NotEqual => "notEqual",
            Self::Greater => "greaterThan",
            Self::Less => "lessThan",
            Self::GreaterOrEqual => "greaterThanOrEqual",
            Self::LessOrEqual => "lessThanOrEqual",
        }
    }

    /// The operator a file names, `None` for one it does not know.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "between" => Self::Between,
            "notBetween" => Self::NotBetween,
            "equal" => Self::Equal,
            "notEqual" => Self::NotEqual,
            "greaterThan" => Self::Greater,
            "lessThan" => Self::Less,
            "greaterThanOrEqual" => Self::GreaterOrEqual,
            "lessThanOrEqual" => Self::LessOrEqual,
            _ => return None,
        })
    }

    /// How many formulas the operator compares with.
    #[must_use]
    pub const fn operands(self) -> usize {
        match self {
            Self::Between | Self::NotBetween => 2,
            _ => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub enum TextOperator {
    Contains,
    NotContains,
    BeginsWith,
    EndsWith,
}

/// What a rule changes in the look of a cell; unset leaves the cell's own.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase", default)
)]
pub struct DifferentialLook {
    /// `RRGGBB`.
    pub fill: Option<String>,
    pub font_colour: Option<String>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    pub number_format: Option<String>,
}

impl DifferentialLook {
    /// Whether the look changes nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// What may be typed in a set of ranges.
#[expect(
    clippy::struct_excessive_bools,
    reason = "the switches a file stores for each validation"
)]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct DataValidation {
    pub ranges: Vec<CellRange>,
    pub kind: ValidationKind,
    #[cfg_attr(feature = "serde", serde(default))]
    pub operator: ComparisonOperator,
    /// One formula, or two for `between`; a list's items as `"a,b,c"` or a range, `$A$1:$A$5`.
    #[cfg_attr(feature = "serde", serde(default))]
    pub formulas: Vec<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub allow_blank: bool,
    /// Whether a list offers its items in the cell; Excel stores the opposite.
    #[cfg_attr(feature = "serde", serde(default))]
    pub dropdown: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    pub error_style: ErrorStyle,
    #[cfg_attr(feature = "serde", serde(default))]
    pub show_error: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    pub error_title: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub error: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub show_prompt: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    pub prompt_title: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub enum ValidationKind {
    Any,
    Whole,
    Decimal,
    List,
    Date,
    Time,
    TextLength,
    Custom,
}

impl ValidationKind {
    /// The kind as a file names it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Any => "none",
            Self::Whole => "whole",
            Self::Decimal => "decimal",
            Self::List => "list",
            Self::Date => "date",
            Self::Time => "time",
            Self::TextLength => "textLength",
            Self::Custom => "custom",
        }
    }

    fn from_name(name: &str) -> Self {
        match name {
            "whole" => Self::Whole,
            "decimal" => Self::Decimal,
            "list" => Self::List,
            "date" => Self::Date,
            "time" => Self::Time,
            "textLength" => Self::TextLength,
            "custom" => Self::Custom,
            _ => Self::Any,
        }
    }
}

/// What a refused value does: `Stop` refuses it, the others only warn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub enum ErrorStyle {
    #[default]
    Stop,
    Warning,
    Information,
}

/// The ranges of a `sqref`, `A1:B2 D4 C:C`; whole columns and rows reach the edge of the grid.
#[must_use]
pub fn parse_ranges(sqref: &str) -> Vec<CellRange> {
    sqref
        .split_whitespace()
        .filter_map(parse_range)
        .take(MAX_RULE_RANGES)
        .collect()
}

fn parse_range(reference: &str) -> Option<CellRange> {
    let reference = reference.rsplit('!').next().unwrap_or(reference);
    if let Some(range) = CellRange::parse(reference) {
        return Some(range);
    }
    let (first, last) = reference.split_once(':')?;
    let (first, last) = (first.replace('$', ""), last.replace('$', ""));
    if let (Ok(first), Ok(last)) = (first.parse::<u32>(), last.parse::<u32>()) {
        let (first, last) = (first.min(last).checked_sub(1)?, first.max(last) - 1);
        return (last <= LAST_ROW).then_some(CellRange {
            first_row: first,
            first_column: 0,
            last_row: last,
            last_column: LAST_COLUMN,
        });
    }
    let start = CellRange::parse(&format!("{first}1"))?;
    let end = CellRange::parse(&format!("{last}1"))?;
    Some(CellRange {
        first_row: 0,
        first_column: start.first_column.min(end.first_column),
        last_row: LAST_ROW,
        last_column: start.first_column.max(end.first_column),
    })
}

/// A range as a `sqref` writes it, `A1:B2`, a single cell as `A1`.
#[must_use]
pub fn range_reference(range: &CellRange) -> String {
    let cell = |row: u32, column: u32| format!("{}{}", column_letters(column), row + 1);
    if range.first_row == range.last_row && range.first_column == range.last_column {
        cell(range.first_row, range.first_column)
    } else {
        format!(
            "{}:{}",
            cell(range.first_row, range.first_column),
            cell(range.last_row, range.last_column)
        )
    }
}

fn column_letters(column: u32) -> String {
    let mut letters = Vec::new();
    let mut rest = column + 1;
    while rest > 0 {
        let digit = (rest - 1) % 26;
        letters.push(char::from(b'A' + u8::try_from(digit).unwrap_or(0)));
        rest = (rest - 1) / 26;
    }
    letters.iter().rev().collect()
}

/// A conditional format being read: its rules with the differential format each names.
pub(crate) struct ReadFormat {
    pub format: ConditionalFormat,
    pub looks: Vec<Option<usize>>,
}

/// A rule as its attributes and children give it, before it becomes a [`Condition`].
#[expect(
    clippy::struct_excessive_bools,
    reason = "the switches a file stores for each rule"
)]
#[derive(Default)]
struct RawRule {
    kind: String,
    operator: Option<String>,
    differential: Option<usize>,
    priority: u32,
    stop_if_true: bool,
    text: Option<String>,
    rank: Option<u32>,
    percent: bool,
    bottom: bool,
    above_average: bool,
    equal_average: bool,
    deviations: Option<u32>,
    period: Option<String>,
    formulas: Vec<String>,
    points: Vec<ScaleValue>,
    colours: Vec<String>,
    icons: Option<String>,
    reverse: bool,
    show_value: bool,
}

impl RawRule {
    fn of(element: &BytesStart<'_>) -> Self {
        let flag = |name: &str| attribute(element, name).is_some_and(|value| is_true(&value));
        let number = |name: &str| attribute(element, name).and_then(|value| value.parse().ok());
        Self {
            kind: attribute(element, "type").unwrap_or_default(),
            operator: attribute(element, "operator"),
            differential: number("dxfId").and_then(|value: u32| usize::try_from(value).ok()),
            priority: number("priority").unwrap_or(0),
            stop_if_true: flag("stopIfTrue"),
            text: attribute(element, "text"),
            rank: number("rank"),
            percent: flag("percent"),
            bottom: flag("bottom"),
            above_average: attribute(element, "aboveAverage").is_none_or(|value| is_true(&value)),
            equal_average: flag("equalAverage"),
            deviations: number("stdDev"),
            period: attribute(element, "timePeriod"),
            show_value: true,
            ..Self::default()
        }
    }

    fn condition(self) -> Option<Condition> {
        let text_rule = |operator| Condition::Text {
            operator,
            text: self.text.clone().unwrap_or_default(),
        };
        Some(match self.kind.as_str() {
            "cellIs" => {
                let operator = self
                    .operator
                    .as_deref()
                    .and_then(ComparisonOperator::from_name)?;
                let mut formulas = self.formulas;
                formulas.truncate(operator.operands());
                if formulas.len() < operator.operands() {
                    return None;
                }
                Condition::CellIs { operator, formulas }
            }
            "expression" => Condition::Expression {
                formula: self.formulas.into_iter().next()?,
            },
            "colorScale" => {
                if self.points.len() < 2 || self.points.len() != self.colours.len() {
                    return None;
                }
                Condition::ColorScale {
                    points: self
                        .points
                        .into_iter()
                        .zip(self.colours)
                        .map(|(value, colour)| ScalePoint { value, colour })
                        .collect(),
                }
            }
            "dataBar" => {
                let mut points = self.points.into_iter();
                Condition::DataBar {
                    min: points.next().unwrap_or(ScaleValue::Min),
                    max: points.next().unwrap_or(ScaleValue::Max),
                    colour: self
                        .colours
                        .into_iter()
                        .next()
                        .unwrap_or_else(|| "638EC6".to_owned()),
                    show_value: self.show_value,
                }
            }
            "iconSet" => Condition::IconSet {
                icons: self.icons.unwrap_or_else(|| "3TrafficLights1".to_owned()),
                thresholds: self.points,
                reverse: self.reverse,
                show_value: self.show_value,
            },
            "top10" => Condition::Top {
                rank: self.rank.unwrap_or(10),
                percent: self.percent,
                bottom: self.bottom,
            },
            "aboveAverage" => Condition::Average {
                below: !self.above_average,
                or_equal: self.equal_average,
                deviations: self.deviations,
            },
            "containsText" => text_rule(TextOperator::Contains),
            "notContainsText" => text_rule(TextOperator::NotContains),
            "beginsWith" => text_rule(TextOperator::BeginsWith),
            "endsWith" => text_rule(TextOperator::EndsWith),
            "duplicateValues" => Condition::Duplicates { unique: false },
            "uniqueValues" => Condition::Duplicates { unique: true },
            "containsBlanks" => Condition::Blanks { blank: true },
            "notContainsBlanks" => Condition::Blanks { blank: false },
            "containsErrors" => Condition::Errors { error: true },
            "notContainsErrors" => Condition::Errors { error: false },
            "timePeriod" => Condition::TimePeriod {
                period: self.period?,
            },
            _ => return None,
        })
    }
}

/// The `conditionalFormatting` the reader stands in, read to its end; `None` when it has no
/// range or no rule this reader knows.
pub(crate) fn read_conditional_formatting(
    reader: &mut Reader<&[u8]>,
    start: &BytesStart<'_>,
    theme: &ThemeColours,
) -> Option<ReadFormat> {
    let ranges = parse_ranges(&attribute(start, "sqref").unwrap_or_default());
    let mut rules: Vec<(ConditionalRule, Option<usize>)> = Vec::new();
    let mut rule: Option<RawRule> = None;
    let mut formula: Option<String> = None;
    loop {
        match reader.read_event().ok()? {
            Event::Empty(element) if element.local_name().as_ref() == "cfRule" => {
                finish_rule(RawRule::of(&element), &mut rules);
            }
            Event::Start(element) | Event::Empty(element) => match element.local_name().as_ref() {
                "cfRule" => rule = Some(RawRule::of(&element)),
                "formula" => formula = Some(String::new()),
                "iconSet" => {
                    if let Some(rule) = &mut rule {
                        rule.icons = attribute(&element, "iconSet");
                        rule.reverse =
                            attribute(&element, "reverse").is_some_and(|value| is_true(&value));
                        rule.show_value =
                            attribute(&element, "showValue").is_none_or(|value| is_true(&value));
                    }
                }
                "dataBar" => {
                    if let Some(rule) = &mut rule {
                        rule.show_value =
                            attribute(&element, "showValue").is_none_or(|value| is_true(&value));
                    }
                }
                "cfvo" => {
                    if let Some(rule) = &mut rule {
                        rule.points.push(scale_value(&element));
                    }
                }
                "color" => {
                    if let (Some(rule), Some(colour)) = (&mut rule, colour(&element, theme)) {
                        rule.colours.push(colour);
                    }
                }
                _ => {}
            },
            Event::Text(text) => {
                if let Some(formula) = &mut formula {
                    push_limited(formula, &text.xml10_content());
                }
            }
            Event::GeneralRef(reference) => {
                if let Some(formula) = &mut formula {
                    let resolved = match reference.resolve_char_ref() {
                        Ok(Some(character)) => Some(character.to_string()),
                        _ => quick_xml::escape::resolve_xml_entity(&reference).map(str::to_owned),
                    };
                    push_limited(formula, &resolved.unwrap_or_default());
                }
            }
            Event::End(element) => match element.local_name().as_ref() {
                "formula" => {
                    if let (Some(rule), Some(text)) = (&mut rule, formula.take()) {
                        rule.formulas.push(text.trim().to_owned());
                    }
                }
                "cfRule" => {
                    if let Some(raw) = rule.take() {
                        finish_rule(raw, &mut rules);
                    }
                }
                "conditionalFormatting" => break,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    if ranges.is_empty() || rules.is_empty() {
        return None;
    }
    let (rules, looks) = rules.into_iter().unzip();
    Some(ReadFormat {
        format: ConditionalFormat { ranges, rules },
        looks,
    })
}

fn finish_rule(raw: RawRule, rules: &mut Vec<(ConditionalRule, Option<usize>)>) {
    let (priority, stop_if_true, differential) = (raw.priority, raw.stop_if_true, raw.differential);
    if let Some(condition) = raw.condition()
        && rules.len() < MAX_SHEET_RULES
    {
        rules.push((
            ConditionalRule {
                condition,
                look: None,
                priority,
                stop_if_true,
            },
            differential,
        ));
    }
}

fn scale_value(element: &BytesStart<'_>) -> ScaleValue {
    let value = attribute(element, "val").unwrap_or_default();
    match attribute(element, "type").as_deref() {
        Some("min") => ScaleValue::Min,
        Some("max") => ScaleValue::Max,
        Some("percent") => ScaleValue::Percent(value),
        Some("percentile") => ScaleValue::Percentile(value),
        Some("formula") => ScaleValue::Formula(value),
        _ => ScaleValue::Number(value),
    }
}

/// The `dataValidation` the reader stands in, read to its end. A validation of the Excel 2010
/// extension keeps its ranges in a `sqref` child and its formulas in `f` children.
pub(crate) fn read_data_validation(
    reader: &mut Reader<&[u8]>,
    start: &BytesStart<'_>,
    empty: bool,
) -> Option<DataValidation> {
    let flag = |name: &str| attribute(start, name).is_some_and(|value| is_true(&value));
    let mut validation = DataValidation {
        ranges: parse_ranges(&attribute(start, "sqref").unwrap_or_default()),
        kind: ValidationKind::from_name(&attribute(start, "type").unwrap_or_default()),
        operator: attribute(start, "operator")
            .as_deref()
            .and_then(ComparisonOperator::from_name)
            .unwrap_or_default(),
        formulas: Vec::new(),
        allow_blank: flag("allowBlank"),
        dropdown: !flag("showDropDown"),
        error_style: match attribute(start, "errorStyle").as_deref() {
            Some("warning") => ErrorStyle::Warning,
            Some("information") => ErrorStyle::Information,
            _ => ErrorStyle::Stop,
        },
        show_error: flag("showErrorMessage"),
        error_title: attribute(start, "errorTitle").filter(|text| !text.is_empty()),
        error: attribute(start, "error").filter(|text| !text.is_empty()),
        show_prompt: flag("showInputMessage"),
        prompt_title: attribute(start, "promptTitle").filter(|text| !text.is_empty()),
        prompt: attribute(start, "prompt").filter(|text| !text.is_empty()),
    };
    if !empty {
        let mut text: Option<String> = None;
        let mut slots: [Option<String>; 2] = [None, None];
        let mut slot = None;
        let mut sqref = None;
        loop {
            match reader.read_event().ok()? {
                Event::Start(element) => match element.local_name().as_ref() {
                    "formula1" => {
                        slot = Some(0);
                        text = Some(String::new());
                    }
                    "formula2" => {
                        slot = Some(1);
                        text = Some(String::new());
                    }
                    "f" | "sqref" => text = Some(String::new()),
                    _ => {}
                },
                Event::Text(content) => {
                    if let Some(text) = &mut text {
                        push_limited(text, &content.xml10_content());
                    }
                }
                Event::GeneralRef(reference) => {
                    if let Some(text) = &mut text {
                        let resolved = match reference.resolve_char_ref() {
                            Ok(Some(character)) => Some(character.to_string()),
                            _ => {
                                quick_xml::escape::resolve_xml_entity(&reference).map(str::to_owned)
                            }
                        };
                        push_limited(text, &resolved.unwrap_or_default());
                    }
                }
                Event::End(element) => match element.local_name().as_ref() {
                    "sqref" => sqref = text.take(),
                    "formula1" | "formula2" => {
                        if let (Some(index), Some(content)) = (slot.take(), text.take())
                            && slots[index].is_none()
                        {
                            slots[index] = Some(content.trim().to_owned());
                        }
                    }
                    "f" => {
                        if let (Some(index), Some(content)) = (slot, text.take()) {
                            slots[index] = Some(content.trim().to_owned());
                        }
                    }
                    "dataValidation" => break,
                    _ => {}
                },
                Event::Eof => break,
                _ => {}
            }
        }
        if let Some(sqref) = sqref {
            validation.ranges = parse_ranges(&sqref);
        }
        validation.formulas = slots
            .into_iter()
            .map_while(|formula| formula.filter(|formula| !formula.is_empty()))
            .collect();
    }
    (!validation.ranges.is_empty()).then_some(validation)
}

fn push_limited(target: &mut String, text: &str) {
    if target.len() + text.len() <= MAX_FORMULA_CHARS {
        target.push_str(text);
    }
}

fn is_true(value: &str) -> bool {
    value == "1" || value.eq_ignore_ascii_case("true")
}

fn attribute(element: &BytesStart<'_>, name: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.as_ref() == name)
        .and_then(|attribute| attribute_value(&attribute))
}

#[cfg(test)]
mod tests;
