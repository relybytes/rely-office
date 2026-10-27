// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! A value shown with its cell's number format.

use crate::dates::date_from_serial;
use crate::model::NumberFormat;
use crate::value::{Locale, Value};

pub(crate) fn display(value: &Value, format: NumberFormat, locale: Locale) -> String {
    let Value::Number(number) = value else {
        return locale.display(value);
    };
    let number = *number;
    match format {
        NumberFormat::General => locale.format_number(number),
        NumberFormat::Percent => format!("{}%", locale.format_number(round(number * 100.0, 10))),
        NumberFormat::Decimals(decimals) => {
            let decimals = usize::from(decimals.min(10));
            let text = format!("{number:.decimals$}");
            match locale {
                Locale::Italian => text.replace('.', ","),
                Locale::English => text,
            }
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
        assert_eq!(display(&number, NumberFormat::Date, italian), "27/10/2026");
        assert_eq!(
            display(&number, NumberFormat::DateTime, italian),
            "27/10/2026 12:00"
        );
        assert_eq!(
            display(&number, NumberFormat::Date, Locale::English),
            "10/27/2026"
        );
        assert_eq!(
            display(&Value::Number(0.125), NumberFormat::Percent, italian),
            "12,5%"
        );
        assert_eq!(
            display(&Value::Number(0.07), NumberFormat::Percent, italian),
            "7%"
        );
        assert_eq!(
            display(&Value::Number(2.0), NumberFormat::Decimals(2), italian),
            "2,00"
        );
        assert_eq!(
            display(
                &Value::Number(2.005),
                NumberFormat::Decimals(1),
                Locale::English
            ),
            "2.0"
        );
        assert_eq!(
            display(&Value::Number(3.0), NumberFormat::Date, italian),
            "3"
        );
        assert_eq!(
            display(&Value::Text("x".to_owned()), NumberFormat::Date, italian),
            "x"
        );
    }
}
