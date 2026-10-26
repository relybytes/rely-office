// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Pieces of Typst source. Document text only ever enters the source as a string literal, so
//! no input can become markup or code.

use std::fmt::Write;

/// A Typst string literal holding `text`; control characters other than tab and newline are
/// dropped.
pub(crate) fn string_literal(text: &str) -> String {
    let mut literal = String::with_capacity(text.len() + 2);
    literal.push('"');
    for character in text.chars() {
        match character {
            '\\' => literal.push_str("\\\\"),
            '"' => literal.push_str("\\\""),
            '\n' => literal.push_str("\\n"),
            '\t' => literal.push_str("\\t"),
            character if character.is_control() => {}
            character => literal.push(character),
        }
    }
    literal.push('"');
    literal
}

/// A length in points, with at most two decimals.
pub(crate) fn points(value: f64) -> String {
    let value = if value.is_finite() { value } else { 0.0 };
    let mut length = format!("{value:.2}");
    while length.ends_with('0') {
        length.pop();
    }
    if length.ends_with('.') {
        length.pop();
    }
    length.push_str("pt");
    length
}

/// A Typst array of font family names, the document's own first.
pub(crate) fn font_list(families: &[String]) -> String {
    let mut list = String::from("(");
    for family in families {
        let _ = write!(list, "{}, ", string_literal(family));
    }
    list.push(')');
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_what_would_end_or_bend_a_string() {
        assert_eq!(
            string_literal("a\"b\\c\nd\te\u{7}f #g [h]"),
            r#""a\"b\\c\nd\tef #g [h]""#
        );
    }

    #[test]
    fn writes_lengths_without_trailing_zeros() {
        assert_eq!(points(12.0), "12pt");
        assert_eq!(points(12.5), "12.5pt");
        assert_eq!(points(1.0 / 3.0), "0.33pt");
        assert_eq!(points(f64::NAN), "0pt");
    }

    #[test]
    fn writes_a_font_list() {
        assert_eq!(
            font_list(&["Carlito".to_owned(), "A \"b\"".to_owned()]),
            r#"("Carlito", "A \"b\"", )"#
        );
    }
}
