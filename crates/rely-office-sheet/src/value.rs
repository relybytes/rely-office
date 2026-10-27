// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! What a cell holds: the input a person typed, the value it computes to, and how a value is
//! shown.

use crate::formula::{Expr, FormulaError, number_text, parse_formula, print_formula};

/// The errors a formula computes to, as spreadsheets spell them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ErrorKind {
    Div0,
    Value,
    Ref,
    Name,
    NotAvailable,
    Num,
    Null,
    /// A formula that depends on itself, which Excel stops at with a warning.
    Circular,
}

impl ErrorKind {
    /// Every kind, the longer codes first so a reader matches `#N/A` before nothing shorter.
    pub const ALL: [Self; 8] = [
        Self::Div0,
        Self::Value,
        Self::Ref,
        Self::Name,
        Self::NotAvailable,
        Self::Num,
        Self::Null,
        Self::Circular,
    ];

    /// The code a cell shows and a file stores.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Div0 => "#DIV/0!",
            Self::Value => "#VALUE!",
            Self::Ref => "#REF!",
            Self::Name => "#NAME?",
            Self::NotAvailable => "#N/A",
            Self::Num => "#NUM!",
            Self::Null => "#NULL!",
            Self::Circular => "#CIRC!",
        }
    }
}

/// What a cell computes to.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize),
    serde(tag = "type", content = "value", rename_all = "camelCase")
)]
pub enum Value {
    Empty,
    Number(f64),
    Text(String),
    Bool(bool),
    Error(ErrorKind),
}

impl Value {
    /// The value as a number, as arithmetic sees it: empty is 0, a boolean 1 or 0, a text a
    /// number only when it reads as one.
    pub(crate) fn as_number(&self) -> Result<f64, ErrorKind> {
        match self {
            Self::Empty => Ok(0.0),
            Self::Number(value) => Ok(*value),
            Self::Bool(value) => Ok(if *value { 1.0 } else { 0.0 }),
            Self::Text(text) => text.trim().parse::<f64>().map_err(|_| ErrorKind::Value),
            Self::Error(kind) => Err(*kind),
        }
    }

    /// The value as text, as `&` sees it.
    pub(crate) fn as_text(&self) -> Result<String, ErrorKind> {
        match self {
            Self::Empty => Ok(String::new()),
            Self::Number(value) => Ok(number_text(*value)),
            Self::Text(text) => Ok(text.clone()),
            Self::Bool(value) => Ok(if *value { "TRUE" } else { "FALSE" }.to_owned()),
            Self::Error(kind) => Err(*kind),
        }
    }

    /// The value as a boolean, as `IF` sees it.
    pub(crate) fn as_bool(&self) -> Result<bool, ErrorKind> {
        match self {
            Self::Empty => Ok(false),
            Self::Number(value) => Ok(*value != 0.0),
            Self::Bool(value) => Ok(*value),
            Self::Text(text) => match text.to_ascii_uppercase().as_str() {
                "TRUE" | "VERO" => Ok(true),
                "FALSE" | "FALSO" => Ok(false),
                _ => Err(ErrorKind::Value),
            },
            Self::Error(kind) => Err(*kind),
        }
    }
}

/// What a person typed into a cell.
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Number(f64),
    Text(String),
    Bool(bool),
    /// A formula, parsed; its text without `=` is [`Input::text`].
    Formula(Box<Expr>),
}

impl Input {
    /// Read what a person typed: `=` starts a formula, a number in the locale's notation is a
    /// number, `TRUE` and `FALSE` (or `VERO` and `FALSO`) are booleans, anything else is text.
    /// `None` for an empty cell. A leading `'` keeps the rest as text.
    ///
    /// # Errors
    ///
    /// [`FormulaError`] for a formula that cannot be parsed.
    pub fn parse(typed: &str, locale: Locale) -> Result<Option<Self>, FormulaError> {
        if typed.is_empty() {
            return Ok(None);
        }
        if let Some(text) = typed.strip_prefix('\'') {
            return Ok(Some(Self::Text(text.to_owned())));
        }
        if let Some(formula) = typed.strip_prefix('=')
            && !formula.trim().is_empty()
        {
            return parse_formula(formula)
                .map(|expression| Some(Self::Formula(Box::new(expression))));
        }
        match typed.trim().to_ascii_uppercase().as_str() {
            "TRUE" | "VERO" => return Ok(Some(Self::Bool(true))),
            "FALSE" | "FALSO" => return Ok(Some(Self::Bool(false))),
            _ => {}
        }
        Ok(Some(locale.parse_number(typed).map_or_else(
            || Self::Text(typed.to_owned()),
            Self::Number,
        )))
    }

    /// The input as it is edited and stored: a formula with its `=`, a number in file notation.
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Self::Number(value) => number_text(*value),
            Self::Text(text) => text.clone(),
            Self::Bool(value) => if *value { "TRUE" } else { "FALSE" }.to_owned(),
            Self::Formula(expression) => format!("={}", print_formula(expression)),
        }
    }
}

/// How numbers are typed and shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Locale {
    /// `1.234,5`
    #[default]
    Italian,
    /// `1,234.5`
    English,
}

impl Locale {
    fn separators(self) -> (char, char) {
        match self {
            Self::Italian => (',', '.'),
            Self::English => ('.', ','),
        }
    }

    /// A number typed in this locale, with optional grouping, sign, percent or currency:
    /// `1.234,5`, `-3`, `12%`, `€ 10`.
    #[must_use]
    pub fn parse_number(self, typed: &str) -> Option<f64> {
        let (decimal, group) = self.separators();
        let mut text = typed.trim();
        let mut scale = 1.0;
        if let Some(stripped) = text.strip_suffix('%') {
            text = stripped.trim_end();
            scale = 0.01;
        }
        text = text.trim_start_matches('€').trim_end_matches('€').trim();
        if text.is_empty() {
            return None;
        }
        let mut normalized = String::with_capacity(text.len());
        let mut seen_decimal = false;
        for (index, character) in text.chars().enumerate() {
            match character {
                digit if digit.is_ascii_digit() => normalized.push(digit),
                '-' | '+' if index == 0 => normalized.push(character),
                separator if separator == decimal && !seen_decimal => {
                    seen_decimal = true;
                    normalized.push('.');
                }
                separator if separator == group && !seen_decimal => {}
                'e' | 'E' => normalized.push('e'),
                _ => return None,
            }
        }
        if !normalized
            .chars()
            .any(|character| character.is_ascii_digit())
        {
            return None;
        }
        normalized.parse::<f64>().ok().map(|value| value * scale)
    }

    /// A number shown in this locale: integers whole, other values with up to ten significant
    /// decimals and no trailing zeros, very large and very small ones in exponent form.
    #[must_use]
    pub fn format_number(self, value: f64) -> String {
        if !value.is_finite() {
            return ErrorKind::Num.code().to_owned();
        }
        let (decimal, _) = self.separators();
        let magnitude = value.abs();
        let text = if magnitude != 0.0 && !(1e-9..1e15).contains(&magnitude) {
            let formatted = format!("{value:.5e}");
            let (mantissa, exponent) = formatted.split_once('e').unwrap_or((&formatted, "0"));
            let mantissa = mantissa.trim_end_matches('0').trim_end_matches('.');
            format!("{mantissa}e{exponent}")
        } else if value.fract() == 0.0 {
            format!("{value:.0}")
        } else {
            let decimals = 11_usize.saturating_sub(digits_before_point(magnitude));
            let mut text = format!("{value:.decimals$}");
            while text.ends_with('0') {
                text.pop();
            }
            if text.ends_with('.') {
                text.pop();
            }
            text
        };
        text.replace('.', &decimal.to_string())
    }

    /// How a cell's value is shown.
    #[must_use]
    pub fn display(self, value: &Value) -> String {
        match value {
            Value::Empty => String::new(),
            Value::Number(number) => self.format_number(*number),
            Value::Text(text) => text.clone(),
            Value::Bool(value) => match (self, value) {
                (Self::Italian, true) => "VERO".to_owned(),
                (Self::Italian, false) => "FALSO".to_owned(),
                (Self::English, true) => "TRUE".to_owned(),
                (Self::English, false) => "FALSE".to_owned(),
            },
            Value::Error(kind) => kind.code().to_owned(),
        }
    }
}

fn digits_before_point(magnitude: f64) -> usize {
    let mut digits = 1;
    let mut bound = 10.0;
    while magnitude >= bound && digits < 16 {
        digits += 1;
        bound *= 10.0;
    }
    digits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_a_person_types() {
        let italian = Locale::Italian;
        assert_eq!(Input::parse("", italian).unwrap(), None);
        assert_eq!(
            Input::parse("1.234,5", italian).unwrap(),
            Some(Input::Number(1_234.5))
        );
        assert_eq!(
            Input::parse("12%", italian).unwrap(),
            Some(Input::Number(0.12))
        );
        assert_eq!(
            Input::parse("€ 10", italian).unwrap(),
            Some(Input::Number(10.0))
        );
        assert_eq!(
            Input::parse("-3", italian).unwrap(),
            Some(Input::Number(-3.0))
        );
        assert_eq!(
            Input::parse("vero", italian).unwrap(),
            Some(Input::Bool(true))
        );
        assert_eq!(
            Input::parse("'12", italian).unwrap(),
            Some(Input::Text("12".to_owned()))
        );
        assert_eq!(
            Input::parse("12a", italian).unwrap(),
            Some(Input::Text("12a".to_owned()))
        );
        assert_eq!(
            Input::parse("=", italian).unwrap(),
            Some(Input::Text("=".to_owned()))
        );
        assert_eq!(
            Input::parse("1,234.5", Locale::English).unwrap(),
            Some(Input::Number(1_234.5))
        );
        assert!(Input::parse("=SUM(", italian).is_err());
        let formula = Input::parse("=somma(A1;1,5)", italian).unwrap().unwrap();
        assert_eq!(formula.text(), "=SUM(A1,1.5)");
    }

    #[test]
    fn shows_numbers_in_the_locale() {
        let italian = Locale::Italian;
        assert_eq!(italian.format_number(1_234.0), "1234");
        assert_eq!(italian.format_number(0.1 + 0.2), "0,3");
        assert_eq!(italian.format_number(-2.5), "-2,5");
        assert_eq!(italian.format_number(1.0 / 3.0), "0,3333333333");
        assert_eq!(italian.format_number(12_345.678_9), "12345,6789");
        assert_eq!(italian.format_number(1e20), "1e20");
        assert_eq!(italian.format_number(f64::NAN), "#NUM!");
        assert_eq!(Locale::English.format_number(2.5), "2.5");
        assert_eq!(italian.display(&Value::Bool(true)), "VERO");
        assert_eq!(italian.display(&Value::Error(ErrorKind::Div0)), "#DIV/0!");
        assert_eq!(italian.display(&Value::Empty), "");
    }

    #[test]
    fn converts_values_as_formulas_do() {
        assert_eq!(Value::Text(" 2 ".to_owned()).as_number(), Ok(2.0));
        assert_eq!(
            Value::Text("x".to_owned()).as_number(),
            Err(ErrorKind::Value)
        );
        assert_eq!(Value::Bool(true).as_number(), Ok(1.0));
        assert_eq!(Value::Number(2.0).as_text(), Ok("2".to_owned()));
        assert_eq!(Value::Number(0.0).as_bool(), Ok(false));
        assert_eq!(Value::Text("falso".to_owned()).as_bool(), Ok(false));
        assert_eq!(Value::Error(ErrorKind::Ref).as_text(), Err(ErrorKind::Ref));
    }
}
