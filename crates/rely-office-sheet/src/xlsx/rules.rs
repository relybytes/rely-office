// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! The conditional formats and data validations of a sheet as an xlsx writes them, and the
//! differential formats of the styles part their rules name.

use std::fmt::Write as _;

use rely_office_core::{
    Condition, ConditionalRule, DataValidation, DifferentialLook, ErrorStyle, ScaleValue,
    TextOperator, ValidationKind, range_reference,
};

use super::escape;
use crate::address::column_name;
use crate::looks::applying_order;
use crate::model::{Sheet, Workbook};
use crate::rules::anchor_of;

/// The looks the rules of a workbook give, each once, in the order the styles part lists them.
pub(super) struct Differentials {
    looks: Vec<DifferentialLook>,
}

impl Differentials {
    pub(super) fn new(workbook: &Workbook) -> Self {
        let mut looks: Vec<DifferentialLook> = Vec::new();
        for sheet in &workbook.sheets {
            for format in &sheet.conditional_formats {
                for look in format.rules.iter().filter_map(|rule| rule.look.as_ref()) {
                    if !looks.contains(look) {
                        looks.push(look.clone());
                    }
                }
            }
        }
        Self { looks }
    }

    fn index(&self, look: &DifferentialLook) -> Option<usize> {
        self.looks.iter().position(|known| known == look)
    }

    /// The `dxfs` element, empty when no rule has a look.
    pub(super) fn xml(&self) -> String {
        if self.looks.is_empty() {
            return String::new();
        }
        let mut xml = format!(r#"<dxfs count="{}">"#, self.looks.len());
        for look in &self.looks {
            xml.push_str("<dxf>");
            let font: String = [
                (look.bold, "b"),
                (look.italic, "i"),
                (look.strike, "strike"),
            ]
            .into_iter()
            .filter_map(|(on, name)| {
                on.map(|on| {
                    if on {
                        format!("<{name}/>")
                    } else {
                        format!(r#"<{name} val="0"/>"#)
                    }
                })
            })
            .chain(look.underline.map(|on| {
                if on {
                    "<u/>".to_owned()
                } else {
                    r#"<u val="none"/>"#.to_owned()
                }
            }))
            .chain(
                look.font_colour
                    .as_ref()
                    .map(|colour| format!(r#"<color rgb="FF{}"/>"#, hex(colour))),
            )
            .collect();
            if !font.is_empty() {
                let _ = write!(xml, "<font>{font}</font>");
            }
            if let Some(code) = &look.number_format {
                let _ = write!(
                    xml,
                    r#"<numFmt numFmtId="{}" formatCode="{}"/>"#,
                    // Ids from 300 keep clear of the ones the cell formats use.
                    300 + self.index(look).unwrap_or(0),
                    escape(code)
                );
            }
            if let Some(fill) = &look.fill {
                let _ = write!(
                    xml,
                    r#"<fill><patternFill patternType="solid"><bgColor rgb="FF{}"/></patternFill></fill>"#,
                    hex(fill)
                );
            }
            xml.push_str("</dxf>");
        }
        xml.push_str("</dxfs>");
        xml
    }
}

/// A colour fit for an `rgb` attribute; anything but six hex digits is black.
fn hex(colour: &str) -> String {
    if colour.len() == 6
        && colour
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        colour.to_ascii_uppercase()
    } else {
        "000000".to_owned()
    }
}

fn sqref(ranges: &[rely_office_core::CellRange]) -> String {
    ranges
        .iter()
        .map(range_reference)
        .collect::<Vec<_>>()
        .join(" ")
}

/// The `conditionalFormatting` elements of a sheet. Priorities are written 1, 2, 3 in the
/// order the rules apply, as Excel wants them distinct.
pub(super) fn conditional_formatting_xml(sheet: &Sheet, differentials: &Differentials) -> String {
    let mut order: Vec<(u32, usize, usize)> = sheet
        .conditional_formats
        .iter()
        .enumerate()
        .flat_map(|(format, content)| {
            content
                .rules
                .iter()
                .enumerate()
                .map(move |(rule, own)| (applying_order(own.priority), format, rule))
        })
        .collect();
    order.sort_unstable();
    let priority = |format: usize, rule: usize| {
        order
            .iter()
            .position(|(_, at_format, at_rule)| *at_format == format && *at_rule == rule)
            .map_or(1, |position| position + 1)
    };
    let mut xml = String::new();
    for (format_index, format) in sheet.conditional_formats.iter().enumerate() {
        let anchor = anchor_of(&format.ranges);
        let first = format!("{}{}", column_name(anchor.column), anchor.row + 1);
        let _ = write!(
            xml,
            r#"<conditionalFormatting sqref="{}">"#,
            sqref(&format.ranges)
        );
        for (rule_index, rule) in format.rules.iter().enumerate() {
            rule_xml(
                &mut xml,
                rule,
                priority(format_index, rule_index),
                &first,
                differentials,
            );
        }
        xml.push_str("</conditionalFormatting>");
    }
    xml
}

#[expect(clippy::too_many_lines, reason = "one arm for each kind of rule")]
fn rule_xml(
    xml: &mut String,
    rule: &ConditionalRule,
    priority: usize,
    first: &str,
    differentials: &Differentials,
) {
    let differential = rule
        .look
        .as_ref()
        .and_then(|look| differentials.index(look))
        .filter(|_| {
            !matches!(
                rule.condition,
                Condition::ColorScale { .. }
                    | Condition::DataBar { .. }
                    | Condition::IconSet { .. }
            )
        })
        .map(|index| format!(r#" dxfId="{index}""#))
        .unwrap_or_default();
    let stop = if rule.stop_if_true {
        r#" stopIfTrue="1""#
    } else {
        ""
    };
    let open = |kind: &str, extra: &str| {
        format!(r#"<cfRule type="{kind}"{differential} priority="{priority}"{stop}{extra}>"#)
    };
    let formula = |text: &str| format!("<formula>{}</formula>", escape(text));
    match &rule.condition {
        Condition::CellIs { operator, formulas } => {
            xml.push_str(&open(
                "cellIs",
                &format!(r#" operator="{}""#, operator.name()),
            ));
            for text in formulas {
                xml.push_str(&formula(text));
            }
        }
        Condition::Expression { formula: text } => {
            xml.push_str(&open("expression", ""));
            xml.push_str(&formula(text));
        }
        Condition::ColorScale { points } => {
            xml.push_str(&open("colorScale", ""));
            xml.push_str("<colorScale>");
            for point in points {
                xml.push_str(&scale_value_xml(&point.value));
            }
            for point in points {
                let _ = write!(xml, r#"<color rgb="FF{}"/>"#, hex(&point.colour));
            }
            xml.push_str("</colorScale>");
        }
        Condition::DataBar {
            min,
            max,
            colour,
            show_value,
        } => {
            xml.push_str(&open("dataBar", ""));
            let _ = write!(
                xml,
                r#"<dataBar{}>{}{}<color rgb="FF{}"/></dataBar>"#,
                if *show_value { "" } else { r#" showValue="0""# },
                scale_value_xml(min),
                scale_value_xml(max),
                hex(colour)
            );
        }
        Condition::IconSet {
            icons,
            thresholds,
            reverse,
            show_value,
        } => {
            xml.push_str(&open("iconSet", ""));
            let _ = write!(
                xml,
                r#"<iconSet iconSet="{}"{}{}>"#,
                escape(icons),
                if *reverse { r#" reverse="1""# } else { "" },
                if *show_value { "" } else { r#" showValue="0""# }
            );
            for threshold in thresholds {
                xml.push_str(&scale_value_xml(threshold));
            }
            xml.push_str("</iconSet>");
        }
        Condition::Top {
            rank,
            percent,
            bottom,
        } => {
            xml.push_str(&open(
                "top10",
                &format!(
                    r#" rank="{rank}"{}{}"#,
                    if *percent { r#" percent="1""# } else { "" },
                    if *bottom { r#" bottom="1""# } else { "" }
                ),
            ));
        }
        Condition::Average {
            below,
            or_equal,
            deviations,
        } => {
            xml.push_str(&open(
                "aboveAverage",
                &format!(
                    "{}{}{}",
                    if *below { r#" aboveAverage="0""# } else { "" },
                    if *or_equal {
                        r#" equalAverage="1""#
                    } else {
                        ""
                    },
                    deviations.map_or_else(String::new, |count| format!(r#" stdDev="{count}""#))
                ),
            ));
        }
        Condition::Text { operator, text } => {
            let quoted = format!("\"{}\"", text.replace('"', "\"\""));
            let (kind, name, test) = match operator {
                TextOperator::Contains => (
                    "containsText",
                    "containsText",
                    format!("NOT(ISERROR(SEARCH({quoted},{first})))"),
                ),
                TextOperator::NotContains => (
                    "notContainsText",
                    "notContains",
                    format!("ISERROR(SEARCH({quoted},{first}))"),
                ),
                TextOperator::BeginsWith => (
                    "beginsWith",
                    "beginsWith",
                    format!("LEFT({first},LEN({quoted}))={quoted}"),
                ),
                TextOperator::EndsWith => (
                    "endsWith",
                    "endsWith",
                    format!("RIGHT({first},LEN({quoted}))={quoted}"),
                ),
            };
            xml.push_str(&open(
                kind,
                &format!(r#" operator="{name}" text="{}""#, escape(text)),
            ));
            xml.push_str(&formula(&test));
        }
        Condition::Duplicates { unique } => {
            xml.push_str(&open(
                if *unique {
                    "uniqueValues"
                } else {
                    "duplicateValues"
                },
                "",
            ));
        }
        Condition::Blanks { blank } => {
            let (kind, test) = if *blank {
                ("containsBlanks", format!("LEN(TRIM({first}))=0"))
            } else {
                ("notContainsBlanks", format!("LEN(TRIM({first}))>0"))
            };
            xml.push_str(&open(kind, ""));
            xml.push_str(&formula(&test));
        }
        Condition::Errors { error } => {
            let (kind, test) = if *error {
                ("containsErrors", format!("ISERROR({first})"))
            } else {
                ("notContainsErrors", format!("NOT(ISERROR({first}))"))
            };
            xml.push_str(&open(kind, ""));
            xml.push_str(&formula(&test));
        }
        Condition::TimePeriod { period } => {
            let day = format!("FLOOR({first},1)");
            let month = |offset: &str| {
                format!(
                    "AND(MONTH({first})=MONTH(EDATE(TODAY(),{offset})),YEAR({first})=YEAR(EDATE(TODAY(),{offset})))"
                )
            };
            let week = |offset: i32| {
                let start = format!("TODAY()-WEEKDAY(TODAY()){:+}", 1 + offset);
                format!("AND({day}>={start},{day}<={start}+6)")
            };
            let test = match period.as_str() {
                "today" => format!("{day}=TODAY()"),
                "yesterday" => format!("{day}=TODAY()-1"),
                "tomorrow" => format!("{day}=TODAY()+1"),
                "last7Days" => format!("AND(TODAY()-{day}<=6,{day}<=TODAY())"),
                "thisWeek" => week(0),
                "lastWeek" => week(-7),
                "nextWeek" => week(7),
                "lastMonth" => month("0-1"),
                "nextMonth" => month("0+1"),
                _ => month("0"),
            };
            xml.push_str(&open(
                "timePeriod",
                &format!(r#" timePeriod="{}""#, escape(period)),
            ));
            xml.push_str(&formula(&test));
        }
    }
    xml.push_str("</cfRule>");
}

fn scale_value_xml(value: &ScaleValue) -> String {
    let (kind, value) = match value {
        ScaleValue::Min => ("min", None),
        ScaleValue::Max => ("max", None),
        ScaleValue::Number(text) => ("num", Some(text)),
        ScaleValue::Percent(text) => ("percent", Some(text)),
        ScaleValue::Percentile(text) => ("percentile", Some(text)),
        ScaleValue::Formula(text) => ("formula", Some(text)),
    };
    match value {
        Some(value) => format!(r#"<cfvo type="{kind}" val="{}"/>"#, escape(value)),
        None => format!(r#"<cfvo type="{kind}"/>"#),
    }
}

/// The `dataValidations` element of a sheet, empty when it has none.
pub(super) fn data_validations_xml(sheet: &Sheet) -> String {
    if sheet.validations.is_empty() {
        return String::new();
    }
    let mut xml = format!(r#"<dataValidations count="{}">"#, sheet.validations.len());
    for validation in &sheet.validations {
        validation_xml(&mut xml, validation);
    }
    xml.push_str("</dataValidations>");
    xml
}

fn validation_xml(xml: &mut String, validation: &DataValidation) {
    xml.push_str("<dataValidation");
    if validation.kind != ValidationKind::Any {
        let _ = write!(xml, r#" type="{}""#, validation.kind.name());
    }
    match validation.error_style {
        ErrorStyle::Stop => {}
        ErrorStyle::Warning => xml.push_str(r#" errorStyle="warning""#),
        ErrorStyle::Information => xml.push_str(r#" errorStyle="information""#),
    }
    let compares = !matches!(
        validation.kind,
        ValidationKind::Any | ValidationKind::List | ValidationKind::Custom
    );
    if compares && validation.operator != rely_office_core::ComparisonOperator::Between {
        let _ = write!(xml, r#" operator="{}""#, validation.operator.name());
    }
    for (on, name) in [
        (validation.allow_blank, "allowBlank"),
        (!validation.dropdown, "showDropDown"),
        (validation.show_prompt, "showInputMessage"),
        (validation.show_error, "showErrorMessage"),
    ] {
        if on {
            let _ = write!(xml, r#" {name}="1""#);
        }
    }
    for (text, name) in [
        (&validation.error_title, "errorTitle"),
        (&validation.error, "error"),
        (&validation.prompt_title, "promptTitle"),
        (&validation.prompt, "prompt"),
    ] {
        if let Some(text) = text {
            let _ = write!(xml, r#" {name}="{}""#, escape(text));
        }
    }
    let _ = write!(xml, r#" sqref="{}">"#, sqref(&validation.ranges));
    let wanted = if compares {
        validation.operator.operands()
    } else {
        1
    };
    for (index, formula) in validation.formulas.iter().take(wanted).enumerate() {
        let _ = write!(
            xml,
            "<formula{0}>{1}</formula{0}>",
            index + 1,
            escape(formula)
        );
    }
    xml.push_str("</dataValidation>");
}

#[cfg(test)]
mod tests;
