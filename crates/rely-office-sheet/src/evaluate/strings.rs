// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! The text functions of this set: regular expressions, the byte-counting variants of the
//! text functions, full and half width, URL encoding, and values and arrays written as text.
//! Regular expressions are those of the `regex` crate, which accepts the common syntax of
//! Excel's PCRE2 without look-around or back-references and always finishes in linear time.

use std::fmt::Write as _;

use regex::{Regex, RegexBuilder};

use super::Evaluator;
use crate::formula::Expr;
use crate::value::{ErrorKind, Value};

type Answer = Option<Result<Value, ErrorKind>>;
type Grid = Vec<Vec<Value>>;

/// The most memory a compiled pattern may take, so a pattern cannot exhaust the page.
const MAX_PATTERN_BYTES: usize = 1 << 20;
/// The longest text a cell holds, as Excel limits it.
const MAX_TEXT_CHARS: usize = 32_767;

fn limited(text: String) -> Result<Value, ErrorKind> {
    if text.chars().count() > MAX_TEXT_CHARS {
        Err(ErrorKind::Value)
    } else {
        Ok(Value::Text(text))
    }
}

fn pattern(text: &str, insensitive: bool) -> Result<Regex, ErrorKind> {
    RegexBuilder::new(text)
        .case_insensitive(insensitive)
        .size_limit(MAX_PATTERN_BYTES)
        .dfa_size_limit(MAX_PATTERN_BYTES)
        .build()
        .map_err(|_| ErrorKind::Value)
}

/// Full-width forms of the ASCII characters to their own, as `ASC` converts them.
fn half_width(text: &str) -> String {
    text.chars()
        .map(|character| match character {
            '\u{3000}' => ' ',
            '\u{ff01}'..='\u{ff5e}' => {
                char::from_u32(u32::from(character) - 0xfee0).unwrap_or(character)
            }
            other => other,
        })
        .collect()
}

/// The ASCII characters to their full-width forms, as `DBCS` converts them.
fn full_width(text: &str) -> String {
    text.chars()
        .map(|character| match character {
            ' ' => '\u{3000}',
            '!'..='~' => char::from_u32(u32::from(character) + 0xfee0).unwrap_or(character),
            other => other,
        })
        .collect()
}

/// A text with every byte but the unreserved ones of RFC 3986 percent-encoded, as UTF-8.
fn encode_url(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

/// A value as `VALUETOTEXT` writes it: with `strict`, text in quotes.
fn value_text(value: &Value, strict: bool) -> String {
    match value {
        Value::Text(text) if strict => format!("\"{}\"", text.replace('"', "\"\"")),
        Value::Error(kind) => kind.code().to_owned(),
        other => other.as_text().unwrap_or_default(),
    }
}

impl Evaluator<'_> {
    fn case_insensitive(&self, sheet: usize, argument: Option<&Expr>) -> Result<bool, ErrorKind> {
        match argument {
            None => Ok(false),
            Some(Expr::Text(text)) if text.is_empty() => Ok(false),
            Some(expression) => match self.scalar(sheet, expression).as_number()?.trunc() {
                0.0 => Ok(false),
                1.0 => Ok(true),
                _ => Err(ErrorKind::Value),
            },
        }
    }

    /// The matches of a pattern: the first, every one down a column, or the groups of the
    /// first along a row.
    pub(super) fn regex_extract(
        &self,
        sheet: usize,
        arguments: &[Expr],
    ) -> Result<Grid, ErrorKind> {
        let text = self.text(sheet, arguments.first())?;
        let expression = self.text(sheet, arguments.get(1))?;
        let mode = match arguments.get(2) {
            None => 0.0,
            Some(Expr::Text(text)) if text.is_empty() => 0.0,
            Some(expression) => self.scalar(sheet, expression).as_number()?.trunc(),
        };
        let regex = pattern(&expression, self.case_insensitive(sheet, arguments.get(3))?)?;
        let found = |text: &str| Value::Text(text.to_owned());
        match mode {
            0.0 => regex
                .find(&text)
                .map(|matched| vec![vec![found(matched.as_str())]])
                .ok_or(ErrorKind::NotAvailable),
            1.0 => {
                let matches: Grid = regex
                    .find_iter(&text)
                    .map(|matched| vec![found(matched.as_str())])
                    .collect();
                if matches.is_empty() {
                    Err(ErrorKind::NotAvailable)
                } else {
                    Ok(matches)
                }
            }
            2.0 => {
                let captures = regex.captures(&text).ok_or(ErrorKind::NotAvailable)?;
                let groups: Vec<Value> = if captures.len() > 1 {
                    captures
                        .iter()
                        .skip(1)
                        .map(|group| found(group.map_or("", |group| group.as_str())))
                        .collect()
                } else {
                    vec![found(captures.get(0).map_or("", |group| group.as_str()))]
                };
                Ok(vec![groups])
            }
            _ => Err(ErrorKind::Value),
        }
    }

    /// A text function of this set, or `None` when the name is none of them.
    #[expect(clippy::too_many_lines, reason = "one arm per function, each short")]
    pub(super) fn strings(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        let argument = |index: usize| arguments.get(index);
        let text = |index: usize| self.text(sheet, argument(index));
        let strict = |index: usize| -> Result<bool, ErrorKind> {
            match argument(index) {
                None => Ok(false),
                Some(Expr::Text(text)) if text.is_empty() => Ok(false),
                Some(expression) => match self.scalar(sheet, expression).as_number()?.trunc() {
                    0.0 => Ok(false),
                    1.0 => Ok(true),
                    _ => Err(ErrorKind::Value),
                },
            }
        };
        Some(match name {
            // Excel counts bytes in these only with a double-byte language set for editing; in
            // every other one, the ones here, they count characters like their plain forms.
            "LEFTB" | "RIGHTB" | "MIDB" | "LENB" | "FINDB" | "SEARCHB" | "REPLACEB" => {
                match self.call(sheet, &name[..name.len() - 1], arguments) {
                    Value::Error(kind) => Err(kind),
                    value => Ok(value),
                }
            }
            "ASC" => text(0).map(|text| Value::Text(half_width(&text))),
            "DBCS" => text(0).map(|text| Value::Text(full_width(&text))),
            "ENCODEURL" => text(0).map(|text| Value::Text(encode_url(&text))),
            "REGEXTEST" => (|| {
                let (text, expression) = (text(0)?, text(1)?);
                let regex = pattern(&expression, self.case_insensitive(sheet, argument(2))?)?;
                Ok(Value::Bool(regex.is_match(&text)))
            })(),
            "REGEXREPLACE" => (|| {
                let (subject, expression, replacement) = (text(0)?, text(1)?, text(2)?);
                let occurrence = match argument(3) {
                    None => 0.0,
                    Some(Expr::Text(text)) if text.is_empty() => 0.0,
                    Some(expression) => self.scalar(sheet, expression).as_number()?.trunc(),
                };
                let regex = pattern(&expression, self.case_insensitive(sheet, argument(4))?)?;
                if occurrence == 0.0 {
                    return limited(
                        regex
                            .replace_all(&subject, replacement.as_str())
                            .into_owned(),
                    );
                }
                let matches: Vec<regex::Captures<'_>> = regex.captures_iter(&subject).collect();
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    clippy::cast_precision_loss,
                    reason = "checked to lie within the matches"
                )]
                let index = if occurrence > 0.0 {
                    if occurrence > matches.len() as f64 {
                        return Ok(Value::Text(subject));
                    }
                    occurrence as usize - 1
                } else {
                    if -occurrence > matches.len() as f64 {
                        return Ok(Value::Text(subject));
                    }
                    matches.len() - (-occurrence) as usize
                };
                let captures = &matches[index];
                let whole = captures.get(0).ok_or(ErrorKind::Value)?;
                let mut expanded = String::new();
                captures.expand(&replacement, &mut expanded);
                limited(format!(
                    "{}{expanded}{}",
                    &subject[..whole.start()],
                    &subject[whole.end()..]
                ))
            })(),
            "VALUETOTEXT" => (|| {
                let value =
                    argument(0).map_or(Value::Empty, |expression| self.scalar(sheet, expression));
                Ok(Value::Text(value_text(&value, strict(1)?)))
            })(),
            "ARRAYTOTEXT" => (|| {
                let strict = strict(1)?;
                let grid = match argument(0) {
                    Some(expression) => match self.grid(sheet, Some(expression)) {
                        Ok(grid) => grid,
                        Err(kind) => vec![vec![Value::Error(kind)]],
                    },
                    None => return Err(ErrorKind::Value),
                };
                let text = if strict {
                    let rows: Vec<String> = grid
                        .iter()
                        .map(|row| {
                            row.iter()
                                .map(|value| value_text(value, true))
                                .collect::<Vec<_>>()
                                .join(",")
                        })
                        .collect();
                    format!("{{{}}}", rows.join(";"))
                } else {
                    grid.iter()
                        .flatten()
                        .map(|value| value_text(value, false))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                Ok(Value::Text(text))
            })(),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests;
