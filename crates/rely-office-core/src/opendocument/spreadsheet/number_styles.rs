// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! The data styles of `OpenDocument` (ODF 1.3, 16.29) as the number format codes of Excel,
//! which the sheet's values are shown through.

use std::collections::HashMap;

use crate::xml_tree::{Element, Namespace};

const NUMBER: Namespace = Namespace::OdfNumber;
const STYLE: Namespace = Namespace::OdfStyle;
/// No format code grows past this; a longer one is cut there.
const MAX_CODE_CHARS: usize = 255;
/// No number keeps more decimals than Excel shows.
const MAX_DECIMALS: usize = 30;

/// The Excel format code of each data style the roots declare, by name; a style with nothing
/// a code can say is left out.
pub(super) fn number_format_codes<'a>(
    roots: impl Iterator<Item = &'a Element>,
) -> HashMap<String, String> {
    let mut codes = HashMap::new();
    for root in roots {
        for container in root.elements().filter(|element| {
            element.is(Namespace::OdfOffice, "styles")
                || element.is(Namespace::OdfOffice, "automatic-styles")
        }) {
            for style in container
                .elements()
                .filter(|style| style.namespace == NUMBER)
            {
                if let (Some(name), Some(code)) =
                    (style.attribute(STYLE, "name"), format_code(style))
                {
                    codes.insert(name.to_owned(), code);
                }
            }
        }
    }
    codes
}

/// The format code of one data style.
fn format_code(style: &Element) -> Option<String> {
    if style.name == "text-style" {
        return Some("@".to_owned());
    }
    if !matches!(
        style.name.as_str(),
        "number-style" | "percentage-style" | "currency-style" | "date-style" | "time-style"
    ) {
        return None;
    }
    let mut code = String::new();
    for part in style.elements() {
        if part.namespace != NUMBER {
            continue;
        }
        let long = part.attribute(NUMBER, "style") == Some("long");
        let textual = part.attribute(NUMBER, "textual") == Some("true");
        match part.name.as_str() {
            "number" => code.push_str(&number_code(part)),
            "scientific-number" => {
                code.push_str(&number_code(part));
                let digits = count(part, "min-exponent-digits").clamp(1, 3);
                code.push_str("E+");
                code.push_str(&"0".repeat(digits));
            }
            "fraction" => {
                let digits = count(part, "min-denominator-digits").clamp(1, 3);
                code.push_str("# ");
                code.push_str(&"?".repeat(digits));
                code.push('/');
                code.push_str(&"?".repeat(digits));
            }
            "text" | "currency-symbol" => code.push_str(&literal(&part.text())),
            "day" => code.push_str(if long { "dd" } else { "d" }),
            "day-of-week" => code.push_str(if long { "dddd" } else { "ddd" }),
            "month" => code.push_str(match (textual, long) {
                (true, true) => "mmmm",
                (true, false) => "mmm",
                (false, true) => "mm",
                (false, false) => "m",
            }),
            "year" => code.push_str(if long { "yyyy" } else { "yy" }),
            "hours" => code.push_str(if long { "hh" } else { "h" }),
            "minutes" => code.push_str(if long { "mm" } else { "m" }),
            "seconds" => {
                code.push_str(if long { "ss" } else { "s" });
                let decimals = count(part, "decimal-places").min(3);
                if decimals > 0 {
                    code.push('.');
                    code.push_str(&"0".repeat(decimals));
                }
            }
            "am-pm" => code.push_str("AM/PM"),
            _ => {}
        }
    }
    let code: String = code.chars().take(MAX_CODE_CHARS).collect();
    (!code.trim().is_empty()).then_some(code)
}

/// A `number:number` as digits: the integer part with or without grouping, then the decimals,
/// the ones that show only when set as `#`.
fn number_code(number: &Element) -> String {
    let grouping = number.attribute(NUMBER, "grouping") == Some("true");
    let integers = number
        .attribute(NUMBER, "min-integer-digits")
        .and_then(|digits| digits.parse::<usize>().ok())
        .unwrap_or(1)
        .min(MAX_DECIMALS);
    let mut code = match (grouping, integers) {
        (true, 0) => "#,###".to_owned(),
        (true, digits) => format!("#,##{}", "0".repeat(digits.max(1))),
        (false, 0) => "#".to_owned(),
        (false, digits) => "0".repeat(digits),
    };
    let decimals = count(number, "decimal-places").min(MAX_DECIMALS);
    let minimum = number
        .attribute(NUMBER, "min-decimal-places")
        .and_then(|digits| digits.parse::<usize>().ok())
        .unwrap_or(decimals)
        .min(decimals);
    if decimals > 0 {
        code.push('.');
        code.push_str(&"0".repeat(minimum));
        code.push_str(&"#".repeat(decimals - minimum));
    }
    code
}

/// A count an attribute of the `number` namespace gives; 0 when it gives none.
fn count(element: &Element, name: &str) -> usize {
    element
        .attribute(NUMBER, name)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0)
}

/// Literal text in a format code: the characters Excel shows as they are stay bare, the rest
/// is quoted.
fn literal(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    if text.chars().all(|character| {
        matches!(
            character,
            ' ' | '-' | '/' | '.' | ',' | ':' | '(' | ')' | '%' | '\u{a0}'
        )
    }) {
        return text.replace('\u{a0}', " ");
    }
    format!("\"{}\"", text.replace('"', ""))
}

#[cfg(test)]
mod tests;
