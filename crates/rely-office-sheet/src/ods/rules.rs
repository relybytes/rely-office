// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! The conditional formats and validations of an ods, as `LibreOffice` writes and reads them:
//! `calcext:conditional-formats` at the end of each table, each condition applying a common
//! cell style, and `table:content-validations` before the tables, which cells name.

use std::fmt::Write as _;

use rely_office_core::{
    CellRange, ComparisonOperator, Condition, ConditionalRule, DataValidation, DifferentialLook,
    ErrorStyle, ScaleValue, TextOperator, ValidationKind,
};

use super::escape;
use super::formula::odf_formula_text;
use super::styles::{colour, conditional_style_xml};
use crate::address::{CellRef, column_name};
use crate::model::{Sheet, Workbook};
use crate::rules::anchor_of;

/// The looks the conditional rules of a workbook apply, each a common style once.
pub(super) struct ConditionalStyles {
    looks: Vec<DifferentialLook>,
}

impl ConditionalStyles {
    pub(super) fn new(workbook: &Workbook) -> Self {
        let mut looks: Vec<DifferentialLook> = Vec::new();
        for look in workbook
            .sheets
            .iter()
            .flat_map(|sheet| &sheet.conditional_formats)
            .flat_map(|format| format.rules.iter().filter_map(|rule| rule.look.as_ref()))
        {
            if !looks.contains(look) {
                looks.push(look.clone());
            }
        }
        Self { looks }
    }

    /// The style a rule applies: its look's, else `Default`, which changes nothing.
    fn name(&self, rule: &ConditionalRule) -> String {
        rule.look
            .as_ref()
            .and_then(|look| self.looks.iter().position(|known| known == look))
            .map_or_else(
                || "Default".to_owned(),
                |index| format!("Condizione{}", index + 1),
            )
    }

    /// The common styles, for `office:styles`.
    pub(super) fn xml(&self) -> String {
        self.looks
            .iter()
            .enumerate()
            .map(|(index, look)| conditional_style_xml(&format!("Condizione{}", index + 1), look))
            .collect()
    }
}

/// A sheet name as an ods address writes it, quoted when it is not a plain word.
pub(super) fn sheet_address(name: &str) -> String {
    if name
        .chars()
        .all(|character| character.is_alphanumeric() || character == '_')
    {
        name.to_owned()
    } else {
        format!("'{}'", name.replace('\'', "''"))
    }
}

fn cell_address(sheet: &str, cell: CellRef) -> String {
    format!("{sheet}.{}{}", column_name(cell.column), cell.row + 1)
}

fn range_address(sheet: &str, ranges: &[CellRange]) -> String {
    ranges
        .iter()
        .map(|range| {
            let start = cell_address(sheet, CellRef::new(range.first_row, range.first_column));
            if range.first_row == range.last_row && range.first_column == range.last_column {
                start
            } else {
                format!(
                    "{start}:{}",
                    cell_address(sheet, CellRef::new(range.last_row, range.last_column))
                )
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A string literal of `OpenFormula`.
fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\""))
}

fn comparison(operator: ComparisonOperator) -> &'static str {
    match operator {
        ComparisonOperator::Equal => "=",
        ComparisonOperator::NotEqual => "!=",
        ComparisonOperator::Greater => ">",
        ComparisonOperator::Less => "<",
        ComparisonOperator::GreaterOrEqual => ">=",
        // A range compares with its own syntax; this is what a lone operand means.
        ComparisonOperator::LessOrEqual
        | ComparisonOperator::Between
        | ComparisonOperator::NotBetween => "<=",
    }
}

/// The value of a `calcext:condition` for a rule that compares or tests, `None` for the rules
/// written as an element of their own.
fn condition_value(condition: &Condition, first_cell: &str) -> Option<String> {
    let formula = odf_formula_text;
    Some(match condition {
        Condition::CellIs { operator, formulas } => match operator {
            ComparisonOperator::Between | ComparisonOperator::NotBetween => format!(
                "{}({},{})",
                if *operator == ComparisonOperator::Between {
                    "between"
                } else {
                    "not-between"
                },
                formula(formulas.first()?),
                formula(formulas.get(1)?)
            ),
            other => format!("{}{}", comparison(*other), formula(formulas.first()?)),
        },
        Condition::Expression { formula: text } => format!("formula-is({})", formula(text)),
        Condition::Top {
            rank,
            percent,
            bottom,
        } => format!(
            "{}-{}({rank})",
            if *bottom { "bottom" } else { "top" },
            if *percent { "percent" } else { "elements" }
        ),
        Condition::Average {
            below, or_equal, ..
        } => format!(
            "{}{}-average",
            if *below { "below" } else { "above" },
            if *or_equal { "-equal" } else { "" }
        ),
        Condition::Text { operator, text } => format!(
            "{}({})",
            match operator {
                TextOperator::Contains => "contains-text",
                TextOperator::NotContains => "not-contains-text",
                TextOperator::BeginsWith => "begins-with",
                TextOperator::EndsWith => "ends-with",
            },
            quoted(text)
        ),
        Condition::Duplicates { unique } => if *unique { "unique" } else { "duplicate" }.to_owned(),
        Condition::Blanks { blank } => format!(
            "formula-is(LEN(TRIM([.{first_cell}])){}0)",
            if *blank { "=" } else { ">" }
        ),
        Condition::Errors { error } => if *error { "is-error" } else { "is-no-error" }.to_owned(),
        _ => return None,
    })
}

fn date_of(period: &str) -> &'static str {
    match period {
        "today" => "today",
        "yesterday" => "yesterday",
        "tomorrow" => "tomorrow",
        "last7Days" => "last-7-days",
        "thisWeek" => "this-week",
        "lastWeek" => "last-week",
        "nextWeek" => "next-week",
        "lastMonth" => "last-month",
        "nextMonth" => "next-month",
        _ => "this-month",
    }
}

fn entry_xml(element: &str, value: &ScaleValue, colour_attribute: Option<&str>) -> String {
    let (kind, value) = match value {
        ScaleValue::Min => ("minimum", "0".to_owned()),
        ScaleValue::Max => ("maximum", "0".to_owned()),
        ScaleValue::Number(text) => ("number", text.clone()),
        ScaleValue::Percent(text) => ("percent", text.clone()),
        ScaleValue::Percentile(text) => ("percentile", text.clone()),
        ScaleValue::Formula(text) => ("formula", odf_formula_text(text)),
    };
    format!(
        r#"<calcext:{element} calcext:value="{}" calcext:type="{kind}"{}/>"#,
        escape(&value),
        colour_attribute.map_or_else(String::new, |found| format!(
            r##" calcext:color="#{}""##,
            colour(found)
        ))
    )
}

/// The `calcext:conditional-formats` of a sheet, empty when it has none. Rules are written in
/// the order they apply, which is the only priority an ods keeps.
pub(super) fn conditional_formats_xml(
    sheet_name: &str,
    sheet: &Sheet,
    styles: &ConditionalStyles,
) -> String {
    if sheet.conditional_formats.is_empty() {
        return String::new();
    }
    let name = sheet_address(sheet_name);
    let mut xml = String::from("<calcext:conditional-formats>");
    for format in &sheet.conditional_formats {
        let anchor = anchor_of(&format.ranges);
        let first_cell = format!("{}{}", column_name(anchor.column), anchor.row + 1);
        let _ = write!(
            xml,
            r#"<calcext:conditional-format calcext:target-range-address="{}">"#,
            escape(&range_address(&name, &format.ranges))
        );
        let mut rules: Vec<&ConditionalRule> = format.rules.iter().collect();
        rules.sort_by_key(|rule| crate::looks::applying_order(rule.priority));
        for rule in rules {
            match &rule.condition {
                Condition::ColorScale { points } => {
                    xml.push_str("<calcext:color-scale>");
                    for point in points {
                        xml.push_str(&entry_xml(
                            "color-scale-entry",
                            &point.value,
                            Some(&point.colour),
                        ));
                    }
                    xml.push_str("</calcext:color-scale>");
                }
                Condition::DataBar {
                    min,
                    max,
                    colour: bar,
                    show_value,
                } => {
                    let _ = write!(
                        xml,
                        r##"<calcext:data-bar calcext:min-length="10" calcext:max-length="90" calcext:negative-color="#ff0000" calcext:axis-position="none" calcext:positive-color="#{}" calcext:axis-color="#000000"{}>{}{}</calcext:data-bar>"##,
                        colour(bar),
                        if *show_value {
                            ""
                        } else {
                            r#" calcext:show-value="false""#
                        },
                        entry_xml("formatting-entry", min, None),
                        entry_xml("formatting-entry", max, None)
                    );
                }
                Condition::IconSet {
                    icons,
                    thresholds,
                    show_value,
                    ..
                } => {
                    let _ = write!(
                        xml,
                        r#"<calcext:icon-set calcext:icon-set-type="{}"{}>"#,
                        escape(icons),
                        if *show_value {
                            ""
                        } else {
                            r#" calcext:show-value="false""#
                        }
                    );
                    for threshold in thresholds {
                        xml.push_str(&entry_xml("formatting-entry", threshold, None));
                    }
                    xml.push_str("</calcext:icon-set>");
                }
                Condition::TimePeriod { period } => {
                    let _ = write!(
                        xml,
                        r#"<calcext:date-is calcext:style="{}" calcext:date="{}"/>"#,
                        styles.name(rule),
                        date_of(period)
                    );
                }
                other => {
                    if let Some(value) = condition_value(other, &first_cell) {
                        let _ = write!(
                            xml,
                            r#"<calcext:condition calcext:apply-style-name="{}" calcext:value="{}" calcext:base-cell-address="{}"/>"#,
                            styles.name(rule),
                            escape(&value),
                            escape(&cell_address(&name, anchor))
                        );
                    }
                }
            }
        }
        xml.push_str("</calcext:conditional-format>");
    }
    xml.push_str("</calcext:conditional-formats>");
    xml
}

/// The name a cell's validation has in the document: the last one of the sheet listed whose
/// ranges hold the cell, as the editor checks it.
pub(super) fn validation_name(sheet_index: usize, sheet: &Sheet, cell: CellRef) -> Option<String> {
    sheet
        .validations
        .iter()
        .rposition(|validation| {
            validation
                .ranges
                .iter()
                .any(|range| range.contains(cell.row, cell.column))
        })
        .map(|index| format!("val{}_{}", sheet_index + 1, index + 1))
}

/// The `table:content-validations` of a workbook, empty when no sheet has one.
pub(super) fn content_validations_xml(workbook: &Workbook) -> String {
    let mut xml = String::new();
    for (sheet_index, sheet) in workbook.sheets.iter().enumerate() {
        let name = sheet_address(&sheet.name);
        for (index, validation) in sheet.validations.iter().enumerate() {
            validation_xml(
                &mut xml,
                &format!("val{}_{}", sheet_index + 1, index + 1),
                &name,
                validation,
            );
        }
    }
    if xml.is_empty() {
        return xml;
    }
    format!("<table:content-validations>{xml}</table:content-validations>")
}

/// The condition of a validation in `OpenFormula`, `None` for one that accepts any value.
fn validation_condition(validation: &DataValidation) -> Option<String> {
    let formula = |index: usize| {
        validation
            .formulas
            .get(index)
            .map(|text| odf_formula_text(text))
    };
    let compared = |subject: &str| -> Option<String> {
        Some(match validation.operator {
            ComparisonOperator::Between => {
                format!("{subject}-is-between({},{})", formula(0)?, formula(1)?)
            }
            ComparisonOperator::NotBetween => {
                format!("{subject}-is-not-between({},{})", formula(0)?, formula(1)?)
            }
            other => format!("{subject}(){}{}", comparison(other), formula(0)?),
        })
    };
    let typed = |kind: &str| compared("cell-content").map(|rest| format!("{kind}() and {rest}"));
    let condition = match validation.kind {
        ValidationKind::Any => return None,
        ValidationKind::Whole => typed("cell-content-is-whole-number")?,
        ValidationKind::Decimal => typed("cell-content-is-decimal-number")?,
        ValidationKind::Date => typed("cell-content-is-date")?,
        ValidationKind::Time => typed("cell-content-is-time")?,
        ValidationKind::TextLength => compared("cell-content-text-length")?,
        ValidationKind::Custom => format!("is-true-formula({})", formula(0)?),
        ValidationKind::List => {
            let source = validation.formulas.first()?.trim();
            let items = match source
                .strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
            {
                Some(list) => list
                    .replace("\"\"", "\"")
                    .split(',')
                    .map(|item| quoted(item.trim()))
                    .collect::<Vec<_>>()
                    .join(";"),
                None => odf_formula_text(source),
            };
            format!("cell-content-is-in-list({items})")
        }
    };
    Some(format!("of:{condition}"))
}

fn validation_xml(xml: &mut String, name: &str, sheet: &str, validation: &DataValidation) {
    let _ = write!(xml, r#"<table:content-validation table:name="{name}""#);
    if let Some(condition) = validation_condition(validation) {
        let _ = write!(xml, r#" table:condition="{}""#, escape(&condition));
    }
    let _ = write!(
        xml,
        r#" table:allow-empty-cell="{}""#,
        validation.allow_blank
    );
    if validation.kind == ValidationKind::List {
        xml.push_str(if validation.dropdown {
            r#" table:display-list="unsorted""#
        } else {
            r#" table:display-list="none""#
        });
    }
    let _ = write!(
        xml,
        r#" table:base-cell-address="{}">"#,
        escape(&cell_address(sheet, anchor_of(&validation.ranges)))
    );
    let message = |element: &str,
                   extra: &str,
                   display: bool,
                   title: &Option<String>,
                   text: &Option<String>| {
        let mut message = format!("<table:{element}{extra}");
        if let Some(title) = title {
            let _ = write!(message, r#" table:title="{}""#, escape(title));
        }
        let _ = write!(message, r#" table:display="{display}">"#);
        for line in text.iter().flat_map(|text| text.lines()) {
            let _ = write!(message, "<text:p>{}</text:p>", escape(line));
        }
        let _ = write!(message, "</table:{element}>");
        message
    };
    if validation.show_prompt || validation.prompt.is_some() || validation.prompt_title.is_some() {
        xml.push_str(&message(
            "help-message",
            "",
            validation.show_prompt,
            &validation.prompt_title,
            &validation.prompt,
        ));
    }
    let kind = match validation.error_style {
        ErrorStyle::Stop => "stop",
        ErrorStyle::Warning => "warning",
        ErrorStyle::Information => "information",
    };
    xml.push_str(&message(
        "error-message",
        &format!(r#" table:message-type="{kind}""#),
        validation.show_error,
        &validation.error_title,
        &validation.error,
    ));
    xml.push_str("</table:content-validation>");
}

#[cfg(test)]
mod tests;
