// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The styles of an ods: one automatic cell style for each look and number format the cells
//! use, the data style each number format needs, and the common styles the rules of the
//! conditional formats apply.

use std::collections::HashMap;
use std::fmt::Write as _;

use rely_office_core::{
    Border, BorderStyle, CellStyle, DifferentialLook, HorizontalAlignment, VerticalAlignment,
};

use super::escape;
use crate::model::{NumberFormat, Workbook};

/// The cell styles of a workbook, by the look and number format they stand for.
pub(super) struct OdsStyles {
    names: HashMap<(u32, NumberFormat), String>,
    /// The data styles and cell styles, for `office:automatic-styles`.
    pub(super) xml: String,
}

impl OdsStyles {
    pub(super) fn new(workbook: &Workbook) -> Self {
        let mut used: Vec<(u32, NumberFormat)> = Vec::new();
        for sheet in &workbook.sheets {
            for (cell, content) in &sheet.cells {
                used.push((sheet.styles.get(cell).copied().unwrap_or(0), content.format));
            }
            for (cell, style) in &sheet.styles {
                if !sheet.cells.contains_key(cell) {
                    used.push((*style, NumberFormat::General));
                }
            }
        }
        let mut seen = std::collections::HashSet::new();
        used.retain(|pair| *pair != (0, NumberFormat::General) && seen.insert(*pair));
        let mut names = HashMap::new();
        let mut xml = String::new();
        for (index, (style_id, format)) in used.iter().enumerate() {
            let style = workbook
                .styles
                .get(usize::try_from(*style_id).unwrap_or(0))
                .cloned()
                .unwrap_or_default();
            let name = format!("ce{}", index + 1);
            let data = data_style(
                &format!("N{}", index + 1),
                *format,
                style.number_format.as_deref(),
            );
            let data_name = data.as_ref().map(|(name, _)| name.clone());
            if let Some((_, data)) = data {
                xml.push_str(&data);
            }
            cell_style_xml(
                &mut xml,
                &name,
                &style,
                data_name.as_deref(),
                "table-cell",
                false,
            );
            names.insert((*style_id, *format), name);
        }
        Self { names, xml }
    }

    /// The cell style of a look shown with a number format; `None` for the default.
    pub(super) fn name(&self, style: u32, format: NumberFormat) -> Option<&str> {
        self.names.get(&(style, format)).map(String::as_str)
    }
}

/// A `style:style` of the table-cell family for a look; `common` writes a common style, which
/// keeps only what the look sets over `Default`.
fn cell_style_xml(
    xml: &mut String,
    name: &str,
    style: &CellStyle,
    data_style: Option<&str>,
    family: &str,
    common: bool,
) {
    let _ = write!(
        xml,
        r#"<style:style style:name="{name}" style:family="{family}" style:parent-style-name="Default""#
    );
    if let Some(data) = data_style {
        let _ = write!(xml, r#" style:data-style-name="{data}""#);
    }
    xml.push('>');
    let mut cell = String::new();
    if let Some(fill) = &style.fill {
        let _ = write!(cell, r##" fo:background-color="#{}""##, colour(fill));
    }
    for (side, border) in [
        ("top", &style.borders.top),
        ("right", &style.borders.right),
        ("bottom", &style.borders.bottom),
        ("left", &style.borders.left),
    ] {
        if let Some(border) = border {
            let _ = write!(cell, r#" fo:border-{side}="{}""#, border_value(border));
        }
    }
    if style.wrap {
        cell.push_str(r#" fo:wrap-option="wrap""#);
    }
    if let Some(vertical) = style.vertical {
        let _ = write!(
            cell,
            r#" style:vertical-align="{}""#,
            match vertical {
                VerticalAlignment::Top => "top",
                VerticalAlignment::Center => "middle",
                VerticalAlignment::Bottom => "bottom",
            }
        );
    }
    if style.horizontal.is_some() {
        cell.push_str(r#" style:text-align-source="fix" style:repeat-content="false""#);
    }
    if !cell.is_empty() {
        let _ = write!(xml, "<style:table-cell-properties{cell}/>");
    }
    if let Some(horizontal) = style.horizontal {
        let _ = write!(
            xml,
            r#"<style:paragraph-properties fo:text-align="{}"/>"#,
            match horizontal {
                HorizontalAlignment::Left => "start",
                HorizontalAlignment::Center => "center",
                HorizontalAlignment::Right => "end",
                HorizontalAlignment::Justify => "justify",
            }
        );
    }
    let font = &style.font;
    let mut text = String::new();
    if let Some(family) = &font.family {
        let _ = write!(text, r#" fo:font-family="{}""#, escape(family));
    }
    if let Some(size) = font.size_tenths {
        let _ = write!(text, r#" fo:font-size="{}pt""#, f64::from(size) / 10.0);
    }
    if font.bold {
        text.push_str(r#" fo:font-weight="bold""#);
    }
    if font.italic {
        text.push_str(r#" fo:font-style="italic""#);
    }
    if font.underline {
        text.push_str(
            r#" style:text-underline-style="solid" style:text-underline-width="auto" style:text-underline-color="font-color""#,
        );
    }
    if font.strike {
        text.push_str(r#" style:text-line-through-style="solid""#);
    }
    if let Some(font_colour) = &font.colour {
        let _ = write!(text, r##" fo:color="#{}""##, colour(font_colour));
    }
    if !text.is_empty() || common {
        let _ = write!(xml, "<style:text-properties{text}/>");
    }
    xml.push_str("</style:style>");
}

/// The common style a conditional rule's look applies, as `styles.xml` lists it.
pub(super) fn conditional_style_xml(name: &str, look: &DifferentialLook) -> String {
    let style = CellStyle {
        fill: look.fill.clone(),
        font: rely_office_core::CellFont {
            colour: look.font_colour.clone(),
            bold: look.bold == Some(true),
            italic: look.italic == Some(true),
            underline: look.underline == Some(true),
            strike: look.strike == Some(true),
            ..rely_office_core::CellFont::default()
        },
        ..CellStyle::default()
    };
    let mut xml = String::new();
    cell_style_xml(&mut xml, name, &style, None, "table-cell", true);
    xml
}

/// A colour fit for `fo:color`, lower case as `LibreOffice` writes it; anything but six hex
/// digits is black.
pub(super) fn colour(value: &str) -> String {
    if value.len() == 6 && value.chars().all(|character| character.is_ascii_hexdigit()) {
        value.to_ascii_lowercase()
    } else {
        "000000".to_owned()
    }
}

fn border_value(border: &Border) -> String {
    let style = match border.style {
        BorderStyle::None => return "none".to_owned(),
        BorderStyle::Single => "solid",
        BorderStyle::Double => "double",
        BorderStyle::Dotted => "dotted",
        BorderStyle::Dashed => "dashed",
    };
    format!(
        "{}pt {style} #{}",
        f64::from(border.width_eighths) / 8.0,
        colour(border.colour.as_deref().unwrap_or("000000"))
    )
}

/// The data style a number format needs, named `name`, with its markup; `None` for General.
pub(super) fn data_style(
    name: &str,
    format: NumberFormat,
    code: Option<&str>,
) -> Option<(String, String)> {
    let day_month_year = concat!(
        r#"<number:day number:style="long"/><number:text>/</number:text>"#,
        r#"<number:month number:style="long"/><number:text>/</number:text>"#,
        r#"<number:year number:style="long"/>"#
    );
    let markup = match format {
        NumberFormat::Date => {
            format!(
                r#"<number:date-style style:name="{name}">{day_month_year}</number:date-style>"#
            )
        }
        NumberFormat::DateTime => format!(
            concat!(
                r#"<number:date-style style:name="{name}">{date}<number:text> </number:text>"#,
                r#"<number:hours number:style="long"/><number:text>:</number:text>"#,
                r#"<number:minutes number:style="long"/></number:date-style>"#
            ),
            name = name,
            date = day_month_year
        ),
        NumberFormat::Percent => format!(
            r#"<number:percentage-style style:name="{name}"><number:number number:decimal-places="2" number:min-decimal-places="0" number:min-integer-digits="1"/><number:text>%</number:text></number:percentage-style>"#
        ),
        NumberFormat::Decimals(decimals) => format!(
            r#"<number:number-style style:name="{name}"><number:number number:decimal-places="{decimals}" number:min-decimal-places="{decimals}" number:min-integer-digits="1"/></number:number-style>"#
        ),
        NumberFormat::General => code_style(name, code?)?,
    };
    Some((name.to_owned(), markup))
}

/// The data style of an Excel format code's first section: a date or time, a percentage or a
/// number with its decimals, grouping and the text around it; `None` for General and text.
fn code_style(name: &str, code: &str) -> Option<String> {
    let section = first_section(code);
    let tokens = tokenize(&section);
    if tokens.is_empty() {
        return None;
    }
    let is_date = tokens.iter().any(|token| matches!(token, Token::Date(_)));
    if is_date {
        return Some(date_style(name, &tokens));
    }
    let digits: String = tokens
        .iter()
        .filter_map(|token| match token {
            Token::Digits(digits) => Some(digits.as_str()),
            _ => None,
        })
        .collect();
    if digits.is_empty() {
        return None;
    }
    let (integer, decimals) = digits.split_once('.').unwrap_or((&digits, ""));
    let decimal_places = decimals
        .chars()
        .filter(|character| *character == '0' || *character == '#')
        .count();
    let min_decimals = decimals
        .chars()
        .filter(|character| *character == '0')
        .count();
    let min_integer = integer
        .chars()
        .filter(|character| *character == '0')
        .count();
    let grouping = integer.contains(',');
    let percent = tokens
        .iter()
        .any(|token| matches!(token, Token::Literal(text) if text == "%"));
    let kind = if percent {
        "percentage-style"
    } else {
        "number-style"
    };
    let mut markup = format!(r#"<number:{kind} style:name="{name}">"#);
    let mut number_written = false;
    for token in &tokens {
        match token {
            Token::Digits(_) if !number_written => {
                let _ = write!(
                    markup,
                    r#"<number:number number:decimal-places="{decimal_places}" number:min-decimal-places="{min_decimals}" number:min-integer-digits="{min_integer}"{}/>"#,
                    if grouping {
                        r#" number:grouping="true""#
                    } else {
                        ""
                    }
                );
                number_written = true;
            }
            Token::Literal(text) => {
                let _ = write!(markup, "<number:text>{}</number:text>", escape(text));
            }
            _ => {}
        }
    }
    let _ = write!(markup, "</number:{kind}>");
    Some(markup)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    /// A run of `0`, `#`, `?`, `,` and `.`.
    Digits(String),
    /// A run of a date or time letter, `dd`, `mmmm`, `yyyy`, `hh`, `ss`, or `AM/PM`.
    Date(String),
    Literal(String),
}

/// The first section of a format code, the one positive numbers show with.
fn first_section(code: &str) -> String {
    let mut section = String::new();
    let mut quoted = false;
    for character in code.chars() {
        match character {
            '"' => {
                quoted = !quoted;
                section.push(character);
            }
            ';' if !quoted => break,
            other => section.push(other),
        }
    }
    section
}

fn tokenize(section: &str) -> Vec<Token> {
    if section.trim().eq_ignore_ascii_case("general") || section.contains('@') {
        return Vec::new();
    }
    let characters: Vec<char> = section.chars().collect();
    let mut tokens = Vec::new();
    let mut literal = String::new();
    let mut index = 0;
    let flush = |literal: &mut String, tokens: &mut Vec<Token>| {
        if !literal.is_empty() {
            tokens.push(Token::Literal(std::mem::take(literal)));
        }
    };
    while index < characters.len() {
        let character = characters[index];
        match character {
            '"' => {
                index += 1;
                while index < characters.len() && characters[index] != '"' {
                    literal.push(characters[index]);
                    index += 1;
                }
                index += 1;
            }
            '\\' => {
                if let Some(next) = characters.get(index + 1) {
                    literal.push(*next);
                }
                index += 2;
            }
            '[' => {
                // A colour, a condition or a locale: none of them shows.
                while index < characters.len() && characters[index] != ']' {
                    index += 1;
                }
                index += 1;
            }
            '_' | '*' => index += 2,
            '0' | '#' | '?' | ',' | '.' => {
                flush(&mut literal, &mut tokens);
                let start = index;
                while index < characters.len()
                    && matches!(characters[index], '0' | '#' | '?' | ',' | '.')
                {
                    index += 1;
                }
                tokens.push(Token::Digits(characters[start..index].iter().collect()));
            }
            letter if "dDmMyYhHsS".contains(letter) => {
                flush(&mut literal, &mut tokens);
                let start = index;
                let lower = letter.to_ascii_lowercase();
                while index < characters.len() && characters[index].to_ascii_lowercase() == lower {
                    index += 1;
                }
                tokens.push(Token::Date(
                    characters[start..index]
                        .iter()
                        .collect::<String>()
                        .to_ascii_lowercase(),
                ));
            }
            'A' | 'a'
                if characters[index..]
                    .iter()
                    .take(5)
                    .collect::<String>()
                    .eq_ignore_ascii_case("am/pm") =>
            {
                flush(&mut literal, &mut tokens);
                tokens.push(Token::Date("am/pm".to_owned()));
                index += 5;
            }
            other => {
                literal.push(other);
                index += 1;
            }
        }
    }
    flush(&mut literal, &mut tokens);
    tokens
}

/// A date style from the tokens of a code; `m` is the month, or the minutes right after an hour
/// or before a second.
fn date_style(name: &str, tokens: &[Token]) -> String {
    let mut markup = format!(r#"<number:date-style style:name="{name}">"#);
    for (position, token) in tokens.iter().enumerate() {
        match token {
            Token::Date(letters) => {
                let length = letters.len();
                let after_hour = tokens[..position]
                    .iter()
                    .rev()
                    .find_map(|token| match token {
                        Token::Date(letters) => Some(letters.starts_with('h')),
                        _ => None,
                    })
                    .unwrap_or(false);
                let before_second = tokens[position + 1..]
                    .iter()
                    .find_map(|token| match token {
                        Token::Date(letters) => Some(letters.starts_with('s')),
                        _ => None,
                    })
                    .unwrap_or(false);
                let style = if length >= 2 { "long" } else { "short" };
                let element = match letters.chars().next().unwrap_or('d') {
                    'd' if length >= 4 => r#"<number:day-of-week number:style="long"/>"#.to_owned(),
                    'd' if length == 3 => "<number:day-of-week/>".to_owned(),
                    'd' => format!(r#"<number:day number:style="{style}"/>"#),
                    'm' if after_hour || before_second => {
                        format!(r#"<number:minutes number:style="{style}"/>"#)
                    }
                    'm' if length >= 4 => {
                        r#"<number:month number:style="long" number:textual="true"/>"#.to_owned()
                    }
                    'm' if length == 3 => r#"<number:month number:textual="true"/>"#.to_owned(),
                    'm' => format!(r#"<number:month number:style="{style}"/>"#),
                    'y' if length >= 3 => r#"<number:year number:style="long"/>"#.to_owned(),
                    'y' => "<number:year/>".to_owned(),
                    'h' => format!(r#"<number:hours number:style="{style}"/>"#),
                    's' => format!(r#"<number:seconds number:style="{style}"/>"#),
                    _ => "<number:am-pm/>".to_owned(),
                };
                markup.push_str(&element);
            }
            Token::Literal(text) => {
                let _ = write!(markup, "<number:text>{}</number:text>", escape(text));
            }
            Token::Digits(digits) => {
                let _ = write!(markup, "<number:text>{}</number:text>", escape(digits));
            }
        }
    }
    markup.push_str("</number:date-style>");
    markup
}

#[cfg(test)]
mod tests;
