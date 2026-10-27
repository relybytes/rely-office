// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! A value shown with its cell's number format.

use rely_office_core::{format_number, format_text};

use crate::dates::date_from_serial;
use crate::model::NumberFormat;
use crate::value::{Locale, Value};

/// A value shown with a number format, or with an Excel format code when the cell's style
/// gives one.
pub(crate) fn display(
    value: &Value,
    format: NumberFormat,
    code: Option<&str>,
    locale: Locale,
) -> String {
    if let Some(code) = code {
        match value {
            Value::Number(number) => return format_number(*number, code, locale.into()).text,
            Value::Text(text) => return format_text(text, code),
            _ => {}
        }
    }
    let Value::Number(number) = value else {
        return locale.display(value);
    };
    let number = *number;
    match format {
        NumberFormat::General => locale.format_number(number),
        NumberFormat::Percent => format!("{}%", locale.format_number(round(number * 100.0, 10))),
        // Halves round away from zero, as Excel rounds them: 0,125 shows 0,13.
        NumberFormat::Decimals(0) => format_number(number, "0", locale.into()).text,
        NumberFormat::Decimals(decimals) => {
            let code = format!("0.{}", "0".repeat(usize::from(decimals.min(10))));
            format_number(number, &code, locale.into()).text
        }
        NumberFormat::Date | NumberFormat::DateTime => {
            let Some((year, month, day)) = date_from_serial(number) else {
                return locale.format_number(number);
            };
            let date = match locale {
                Locale::Italian => format!("{day:02}/{month:02}/{year}"),
                Locale::English => format!("{month:02}/{day:02}/{year}"),
            };
            if format == NumberFormat::Date {
                return date;
            }
            #[allow(clippy::cast_possible_truncation)]
            let minutes = ((number.fract()) * 1_440.0).round() as i64 % 1_440;
            format!("{date} {:02}:{:02}", minutes / 60, minutes % 60)
        }
    }
}

fn round(value: f64, decimals: i32) -> f64 {
    let scale = 10f64.powi(decimals);
    (value * scale).round() / scale
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_numbers_with_their_format() {
        let italian = Locale::Italian;
        let number = Value::Number(46_322.5);
        assert_eq!(
            display(&number, NumberFormat::Date, None, italian),
            "27/10/2026"
        );
        assert_eq!(
            display(&number, NumberFormat::DateTime, None, italian),
            "27/10/2026 12:00"
        );
        assert_eq!(
            display(&number, NumberFormat::Date, None, Locale::English),
            "10/27/2026"
        );
        assert_eq!(
            display(&Value::Number(0.125), NumberFormat::Percent, None, italian),
            "12,5%"
        );
        assert_eq!(
            display(&Value::Number(0.07), NumberFormat::Percent, None, italian),
            "7%"
        );
        assert_eq!(
            display(
                &Value::Number(2.0),
                NumberFormat::Decimals(2),
                None,
                italian
            ),
            "2,00"
        );
        assert_eq!(
            display(
                &Value::Number(2.005),
                NumberFormat::Decimals(1),
                None,
                Locale::English
            ),
            "2.0"
        );
        assert_eq!(
            display(
                &Value::Number(1_234.5),
                NumberFormat::General,
                Some("#,##0.00 \"€\""),
                italian
            ),
            "1.234,50 €"
        );
        assert_eq!(
            display(
                &Value::Text("A1".to_owned()),
                NumberFormat::General,
                Some("\"Rif. \"@"),
                italian
            ),
            "Rif. A1"
        );
        assert_eq!(
            display(
                &Value::Bool(true),
                NumberFormat::General,
                Some("0.00"),
                italian
            ),
            "VERO"
        );
        assert_eq!(
            display(&Value::Number(3.0), NumberFormat::Date, None, italian),
            "3"
        );
        assert_eq!(
            display(
                &Value::Text("x".to_owned()),
                NumberFormat::Date,
                None,
                italian
            ),
            "x"
        );
    }
}
