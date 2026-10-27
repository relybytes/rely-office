// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Formulas in `OpenFormula`, the notation of an ods: references in brackets, `[.A1:.B2]` and
//! `[$Foglio2.C3]`, arguments separated by semicolons, and the functions newer than ODF 1.2
//! under the prefix `LibreOffice` reads them by.

use std::fmt::Write as _;

use crate::address::{MAX_ROWS, column_name};
use crate::formula::{
    BinaryOperator, CellAddress, Expr, UnaryOperator, number_text, parse_formula,
};

/// Excel's functions `OpenFormula` names under Microsoft's prefix.
const MICROSOFT_FUNCTIONS: [&str; 15] = [
    "CEILING.MATH",
    "FLOOR.MATH",
    "STDEV.S",
    "STDEV.P",
    "VAR.S",
    "VAR.P",
    "RANK.EQ",
    "MODE.SNGL",
    "MAXIFS",
    "MINIFS",
    "IFS",
    "SWITCH",
    "TEXTJOIN",
    "CONCAT",
    "XLOOKUP",
];

/// A parsed formula in `OpenFormula`, without the `of:=` a cell writes before it.
pub(crate) fn odf_formula(expression: &Expr) -> String {
    let mut text = String::new();
    print(expression, &mut text, 0);
    text
}

/// A formula kept as text, in Excel's notation, in `OpenFormula`; a text that does not parse
/// is written as it is.
pub(crate) fn odf_formula_text(formula: &str) -> String {
    parse_formula(formula).map_or_else(
        |_| formula.to_owned(),
        |expression| odf_formula(&expression),
    )
}

fn precedence(operator: BinaryOperator) -> u8 {
    match operator {
        BinaryOperator::Equal
        | BinaryOperator::NotEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual => 1,
        BinaryOperator::Concatenate => 2,
        BinaryOperator::Add | BinaryOperator::Subtract => 3,
        BinaryOperator::Multiply | BinaryOperator::Divide => 4,
        BinaryOperator::Power => 5,
    }
}

fn symbol(operator: BinaryOperator) -> &'static str {
    match operator {
        BinaryOperator::Add => "+",
        BinaryOperator::Subtract => "-",
        BinaryOperator::Multiply => "*",
        BinaryOperator::Divide => "/",
        BinaryOperator::Power => "^",
        BinaryOperator::Concatenate => "&",
        BinaryOperator::Equal => "=",
        BinaryOperator::NotEqual => "<>",
        BinaryOperator::Less => "<",
        BinaryOperator::LessOrEqual => "<=",
        BinaryOperator::Greater => ">",
        BinaryOperator::GreaterOrEqual => ">=",
    }
}

fn print(expression: &Expr, text: &mut String, parent_precedence: u8) {
    match expression {
        Expr::Number(value) => text.push_str(&number_text(*value)),
        Expr::Text(value) => {
            let _ = write!(text, "\"{}\"", value.replace('"', "\"\""));
        }
        Expr::Bool(value) => text.push_str(if *value { "TRUE()" } else { "FALSE()" }),
        Expr::Error(kind) => text.push_str(kind.code()),
        Expr::Cell(address) => {
            text.push('[');
            print_sheet(address.sheet.as_deref(), text);
            print_cell(address, false, text);
            text.push(']');
        }
        Expr::Range(range) => {
            text.push('[');
            print_sheet(range.sheet.as_deref(), text);
            print_cell(&range.start, range.whole_columns, text);
            text.push_str(":.");
            let mut end = range.end.clone();
            if range.whole_columns {
                end.cell.row = MAX_ROWS - 1;
            }
            print_cell(&end, false, text);
            text.push(']');
        }
        Expr::Unary(operator, operand) => {
            text.push(match operator {
                UnaryOperator::Plus => '+',
                UnaryOperator::Minus => '-',
            });
            print(operand, text, 6);
        }
        Expr::Percent(operand) => {
            print(operand, text, 7);
            text.push('%');
        }
        Expr::Group(inner) => {
            text.push('(');
            print(inner, text, 0);
            text.push(')');
        }
        Expr::Binary(operator, left, right) => {
            let own = precedence(*operator);
            let parenthesize = own < parent_precedence;
            if parenthesize {
                text.push('(');
            }
            print(left, text, own);
            text.push_str(symbol(*operator));
            print(right, text, own + 1);
            if parenthesize {
                text.push(')');
            }
        }
        Expr::Call(name, arguments) => {
            let name = name.strip_prefix("_xlfn.").unwrap_or(name);
            if MICROSOFT_FUNCTIONS.contains(&name) {
                text.push_str("COM.MICROSOFT.");
            }
            text.push_str(name);
            text.push('(');
            for (index, argument) in arguments.iter().enumerate() {
                if index > 0 {
                    text.push(';');
                }
                if !matches!(argument, Expr::Text(value) if value.is_empty()) {
                    print(argument, text, 0);
                }
            }
            text.push(')');
        }
    }
}

/// The sheet of a reference, `$Foglio2` or `$'Due fogli'`, then the dot every reference has.
fn print_sheet(sheet: Option<&str>, text: &mut String) {
    if let Some(sheet) = sheet {
        let plain = sheet
            .chars()
            .all(|character| character.is_alphanumeric() || character == '_');
        if plain {
            let _ = write!(text, "${sheet}");
        } else {
            let _ = write!(text, "$'{}'", sheet.replace('\'', "''"));
        }
    }
    text.push('.');
}

/// A cell of a reference, its row from the top of the column for a whole column.
fn print_cell(address: &CellAddress, whole_column_start: bool, text: &mut String) {
    if address.absolute_column {
        text.push('$');
    }
    text.push_str(&column_name(address.cell.column));
    if address.absolute_row {
        text.push('$');
    }
    let row = if whole_column_start {
        0
    } else {
        address.cell.row
    };
    let _ = write!(text, "{}", row + 1);
}

#[cfg(test)]
mod tests;
