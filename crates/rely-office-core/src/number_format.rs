// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! Excel number format codes: how a cell's number is shown. A code holds up to four sections
//! (positive; negative; zero; text) or sections with conditions (`[>=1000]`), and each section
//! is digit placeholders (`0 # ?`), separators, a percent, an exponent, a fraction, or date and
//! time parts, with literal text, colours and a locale around them. The separators of the code
//! are always `.` and `,`; the locale decides how they are shown.

use std::fmt::Write as _;

/// How numbers, months and days are written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NumberLocale {
    /// `1.234,5`, `27/10/2026`, `ottobre`.
    #[default]
    Italian,
    /// `1,234.5`, `10/27/2026`, `October`.
    English,
}

impl NumberLocale {
    fn decimal(self) -> char {
        match self {
            Self::Italian => ',',
            Self::English => '.',
        }
    }

    fn thousands(self) -> char {
        match self {
            Self::Italian => '.',
            Self::English => ',',
        }
    }
}

/// A number as a format code shows it, with the colour the code asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormattedNumber {
    pub text: String,
    /// A colour of the code, as `RRGGBB`: `[Red]` is `FF0000`.
    pub colour: Option<&'static str>,
}

/// The longest code read; a real one is a few dozen characters.
const MAX_CODE_CHARS: usize = 256;
/// The most decimals or exponent digits shown.
const MAX_DIGITS: usize = 30;
const MONTHS_IT: [&str; 12] = [
    "gennaio",
    "febbraio",
    "marzo",
    "aprile",
    "maggio",
    "giugno",
    "luglio",
    "agosto",
    "settembre",
    "ottobre",
    "novembre",
    "dicembre",
];
const MONTHS_EN: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const DAYS_IT: [&str; 7] = [
    "domenica",
    "lunedì",
    "martedì",
    "mercoledì",
    "giovedì",
    "venerdì",
    "sabato",
];
const DAYS_EN: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

/// The code of a built-in number format id, as Excel defines them; 14 and 22, the short date,
/// follow the locale when shown.
#[must_use]
pub fn builtin_format_code(id: u32) -> Option<&'static str> {
    Some(match id {
        0 => "General",
        1 => "0",
        2 => "0.00",
        3 => "#,##0",
        4 => "#,##0.00",
        5 => "#,##0 \"€\";-#,##0 \"€\"",
        6 => "#,##0 \"€\";[Red]-#,##0 \"€\"",
        7 => "#,##0.00 \"€\";-#,##0.00 \"€\"",
        8 => "#,##0.00 \"€\";[Red]-#,##0.00 \"€\"",
        9 => "0%",
        10 => "0.00%",
        11 => "0.00E+00",
        12 => "# ?/?",
        13 => "# ??/??",
        14 => "m/d/yyyy",
        15 => "d-mmm-yy",
        16 => "d-mmm",
        17 => "mmm-yy",
        18 => "h:mm AM/PM",
        19 => "h:mm:ss AM/PM",
        20 => "h:mm",
        21 => "h:mm:ss",
        22 => "m/d/yyyy h:mm",
        37 => "#,##0 ;(#,##0)",
        38 => "#,##0 ;[Red](#,##0)",
        39 => "#,##0.00;(#,##0.00)",
        40 => "#,##0.00;[Red](#,##0.00)",
        45 => "mm:ss",
        46 => "[h]:mm:ss",
        47 => "mm:ss.0",
        48 => "##0.0E+0",
        49 => "@",
        _ => return None,
    })
}

/// Whether a code shows a date or a time.
#[must_use]
pub fn is_date_code(code: &str) -> bool {
    let sections = split_sections(code);
    sections
        .first()
        .is_some_and(|section| parse_section(section).is_date())
}

/// A number shown with a format code.
#[must_use]
pub fn format_number(value: f64, code: &str, locale: NumberLocale) -> FormattedNumber {
    if !value.is_finite() {
        return FormattedNumber {
            text: "#NUM!".to_owned(),
            colour: None,
        };
    }
    let code: String = localized_code(code, locale)
        .chars()
        .take(MAX_CODE_CHARS)
        .collect();
    let sections: Vec<Section> = split_sections(&code)
        .iter()
        .map(|section| parse_section(section))
        .collect();
    let (section, value, sign) = choose_section(&sections, value);
    let locale = section.locale.unwrap_or(locale);
    let text = if section.is_general() {
        let shown = general_number(value.abs(), locale);
        format!(
            "{}{}",
            if sign { "-" } else { "" },
            section.literal_around(&shown)
        )
    } else if section.is_date() {
        if sign {
            "#".repeat(8)
        } else {
            format_date(value, &section, locale)
        }
    } else {
        let shown = format_numeric(value.abs(), &section, locale);
        if sign
            && shown
                .chars()
                .any(|character| character.is_ascii_digit() && character != '0')
        {
            format!("-{shown}")
        } else {
            shown
        }
    };
    FormattedNumber {
        text,
        colour: section.colour,
    }
}

/// The short date of built-in formats 14 and 22, which Excel writes in the order of the locale.
fn localized_code(code: &str, locale: NumberLocale) -> &str {
    match (code, locale) {
        ("m/d/yyyy", NumberLocale::Italian) => "dd/mm/yyyy",
        ("m/d/yyyy h:mm", NumberLocale::Italian) => "dd/mm/yyyy h:mm",
        _ => code,
    }
}

/// A text shown with a format code: its text section, or the only section when it holds `@`.
#[must_use]
pub fn format_text(text: &str, code: &str) -> String {
    let sections = split_sections(code);
    let section = match sections.len() {
        4.. => sections.get(3),
        _ => sections.first().filter(|section| section.contains('@')),
    };
    let Some(section) = section else {
        return text.to_owned();
    };
    let parsed = parse_section(section);
    let mut shown = String::new();
    for token in &parsed.tokens {
        match token {
            Token::Literal(literal) => shown.push_str(literal),
            Token::Text => shown.push_str(text),
            _ => {}
        }
    }
    shown
}

/// The plain number Excel's General format shows: up to eleven characters, an exponent past
/// them.
#[must_use]
pub fn general_number(value: f64, locale: NumberLocale) -> String {
    if value == 0.0 || !value.is_finite() {
        return "0".to_owned();
    }
    let magnitude = value.abs();
    let text = if (1e-9..1e11).contains(&magnitude) {
        // Ten significant digits, as the column of a default width holds.
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the magnitude lies between 1e-9 and 1e11"
        )]
        let digits_before = magnitude.log10().floor() as i32 + 1;
        let decimals = usize::try_from((10 - digits_before.max(0)).clamp(0, 10)).unwrap_or(0);
        trim_zeros(&format!("{value:.decimals$}"))
    } else {
        let formatted = format!("{value:.5E}");
        let (mantissa, exponent) = formatted.split_once('E').unwrap_or((&formatted, "0"));
        let mantissa = trim_zeros(mantissa);
        let exponent: i32 = exponent.parse().unwrap_or(0);
        let sign = if exponent < 0 { '-' } else { '+' };
        format!("{mantissa}E{sign}{:02}", exponent.abs())
    };
    text.replace('.', &locale.decimal().to_string())
}

fn trim_zeros(text: &str) -> String {
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text.to_owned()
    }
}

/// The sections of a code, split on `;` outside quotes, brackets and escapes.
fn split_sections(code: &str) -> Vec<String> {
    let mut sections = vec![String::new()];
    let mut characters = code.chars();
    let mut quoted = false;
    let mut bracket = false;
    while let Some(character) = characters.next() {
        let current = sections.last_mut().expect("one section at least");
        match character {
            '"' => {
                quoted = !quoted;
                current.push(character);
            }
            '\\' if !quoted => {
                current.push(character);
                if let Some(next) = characters.next() {
                    current.push(next);
                }
            }
            '[' if !quoted => {
                bracket = true;
                current.push(character);
            }
            ']' if !quoted => {
                bracket = false;
                current.push(character);
            }
            ';' if !quoted && !bracket => sections.push(String::new()),
            other => current.push(other),
        }
    }
    sections.truncate(4);
    sections
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Literal(String),
    /// A digit placeholder: `0`, `#` or `?`.
    Digit(char),
    DecimalPoint,
    Thousands,
    Percent,
    /// `E+` or `E-`.
    Exponent(bool),
    Fraction,
    Text,
    General,
    Year(usize),
    /// A month, or minutes when the token next to it is an hour or a second.
    Month(usize),
    Minute(usize),
    Day(usize),
    Hour(usize),
    Second(usize),
    /// Elapsed hours, minutes or seconds: `[h]`, `[mm]`, `[ss]`.
    Elapsed(char, usize),
    /// Tenths, hundredths, thousandths of a second after the seconds.
    SubSecond(usize),
    AmPm(bool),
}

#[derive(Debug, Clone, Default)]
struct Section {
    tokens: Vec<Token>,
    colour: Option<&'static str>,
    condition: Option<(String, f64)>,
    locale: Option<NumberLocale>,
}

impl Section {
    fn is_general(&self) -> bool {
        self.tokens.is_empty() || self.tokens.contains(&Token::General)
    }

    fn is_date(&self) -> bool {
        self.tokens.iter().any(|token| {
            matches!(
                token,
                Token::Year(_)
                    | Token::Month(_)
                    | Token::Minute(_)
                    | Token::Day(_)
                    | Token::Hour(_)
                    | Token::Second(_)
                    | Token::Elapsed(..)
                    | Token::AmPm(_)
            )
        })
    }

    /// The literal text of the section around a value shown in place of `General`.
    fn literal_around(&self, shown: &str) -> String {
        let mut text = String::new();
        let mut placed = false;
        for token in &self.tokens {
            match token {
                Token::Literal(literal) => text.push_str(literal),
                Token::General if !placed => {
                    text.push_str(shown);
                    placed = true;
                }
                _ => {}
            }
        }
        if !placed {
            text.push_str(shown);
        }
        text
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "one arm per character of the format code grammar"
)]
fn parse_section(section: &str) -> Section {
    let mut parsed = Section::default();
    let characters: Vec<char> = section.chars().collect();
    let mut index = 0;
    let mut literal = String::new();
    let flush = |literal: &mut String, tokens: &mut Vec<Token>| {
        if !literal.is_empty() {
            tokens.push(Token::Literal(std::mem::take(literal)));
        }
    };
    while index < characters.len() {
        let character = characters[index];
        let rest: String = characters[index..].iter().collect();
        let lower = rest.to_ascii_lowercase();
        let run = |wanted: char| {
            characters[index..]
                .iter()
                .take_while(|next| next.eq_ignore_ascii_case(&wanted))
                .count()
        };
        match character {
            '"' => {
                let end = characters[index + 1..]
                    .iter()
                    .position(|next| *next == '"')
                    .map_or(characters.len(), |end| index + 1 + end);
                literal.extend(&characters[index + 1..end]);
                index = end + 1;
            }
            '\\' => {
                if let Some(next) = characters.get(index + 1) {
                    literal.push(*next);
                }
                index += 2;
            }
            '_' => {
                literal.push(' ');
                index += 2;
            }
            '*' => index += 2,
            '[' => {
                let end = characters[index..]
                    .iter()
                    .position(|next| *next == ']')
                    .map_or(characters.len(), |end| index + end);
                let inside: String = characters[index + 1..end.min(characters.len())]
                    .iter()
                    .collect();
                index = end + 1;
                flush(&mut literal, &mut parsed.tokens);
                read_bracket(&inside, &mut parsed);
            }
            '0' | '#' | '?' => {
                flush(&mut literal, &mut parsed.tokens);
                parsed.tokens.push(Token::Digit(character));
                index += 1;
            }
            '.' | ',' | '%'
                if is_date_tokens(&parsed.tokens)
                    && !after_seconds(&parsed.tokens, character, &characters, index) =>
            {
                literal.push(character);
                index += 1;
            }
            '.' => {
                let seconds_before = matches!(
                    last_time_token(&parsed.tokens),
                    Some(Token::Second(_) | Token::Elapsed('s', _))
                );
                if seconds_before && characters.get(index + 1) == Some(&'0') {
                    let zeros = characters[index + 1..]
                        .iter()
                        .take_while(|next| **next == '0')
                        .count();
                    flush(&mut literal, &mut parsed.tokens);
                    parsed.tokens.push(Token::SubSecond(zeros.min(3)));
                    index += 1 + zeros;
                } else {
                    flush(&mut literal, &mut parsed.tokens);
                    parsed.tokens.push(Token::DecimalPoint);
                    index += 1;
                }
            }
            ',' => {
                flush(&mut literal, &mut parsed.tokens);
                parsed.tokens.push(Token::Thousands);
                index += 1;
            }
            '%' => {
                flush(&mut literal, &mut parsed.tokens);
                parsed.tokens.push(Token::Percent);
                index += 1;
            }
            '/' if has_digit_after(&characters[index + 1..]) && has_digits(&parsed.tokens) => {
                flush(&mut literal, &mut parsed.tokens);
                parsed.tokens.push(Token::Fraction);
                index += 1;
            }
            'E' | 'e' if matches!(characters.get(index + 1), Some('+' | '-')) => {
                flush(&mut literal, &mut parsed.tokens);
                parsed
                    .tokens
                    .push(Token::Exponent(characters[index + 1] == '+'));
                index += 2;
            }
            '@' => {
                flush(&mut literal, &mut parsed.tokens);
                parsed.tokens.push(Token::Text);
                index += 1;
            }
            _ if lower.starts_with("general") => {
                flush(&mut literal, &mut parsed.tokens);
                parsed.tokens.push(Token::General);
                index += "general".len();
            }
            _ if lower.starts_with("am/pm") => {
                flush(&mut literal, &mut parsed.tokens);
                parsed.tokens.push(Token::AmPm(true));
                index += 5;
            }
            _ if lower.starts_with("a/p") => {
                flush(&mut literal, &mut parsed.tokens);
                parsed.tokens.push(Token::AmPm(false));
                index += 3;
            }
            'y' | 'Y' => {
                let length = run(character);
                flush(&mut literal, &mut parsed.tokens);
                parsed.tokens.push(Token::Year(length));
                index += length;
            }
            'm' | 'M' => {
                let length = run('m');
                flush(&mut literal, &mut parsed.tokens);
                parsed.tokens.push(Token::Month(length));
                index += length;
            }
            'd' | 'D' => {
                let length = run(character);
                flush(&mut literal, &mut parsed.tokens);
                parsed.tokens.push(Token::Day(length));
                index += length;
            }
            'h' | 'H' => {
                let length = run('h');
                flush(&mut literal, &mut parsed.tokens);
                parsed.tokens.push(Token::Hour(length));
                index += length;
            }
            's' | 'S' => {
                let length = run('s');
                flush(&mut literal, &mut parsed.tokens);
                parsed.tokens.push(Token::Second(length));
                index += length;
            }
            other => {
                literal.push(other);
                index += 1;
            }
        }
    }
    flush(&mut literal, &mut parsed.tokens);
    resolve_minutes(&mut parsed.tokens);
    parsed
}

/// A bracketed part of a section: a colour, a condition, a locale with a currency, or elapsed
/// time.
fn read_bracket(inside: &str, parsed: &mut Section) {
    let lower = inside.to_ascii_lowercase();
    if let Some(colour) = named_colour(&lower) {
        parsed.colour = Some(colour);
    } else if lower.starts_with("color") || lower.starts_with("colore") {
    } else if let Some(rest) = inside.strip_prefix('$') {
        let (symbol, tag) = rest.split_once('-').unwrap_or((rest, ""));
        if !symbol.is_empty() {
            parsed.tokens.push(Token::Literal(symbol.to_owned()));
        }
        let language = tag.get(tag.len().saturating_sub(3)..).unwrap_or_default();
        parsed.locale = match language.to_ascii_lowercase().as_str() {
            "410" | "810" => Some(NumberLocale::Italian),
            "409" | "809" => Some(NumberLocale::English),
            _ => parsed.locale,
        };
    } else if let Some(operator) = ["<=", ">=", "<>", "<", ">", "="]
        .into_iter()
        .find(|operator| inside.starts_with(operator))
    {
        if let Ok(threshold) = inside[operator.len()..].trim().parse::<f64>() {
            parsed.condition = Some((operator.to_owned(), threshold));
        }
    } else if !lower.is_empty() && lower.chars().all(|character| character == 'h') {
        parsed.tokens.push(Token::Elapsed('h', lower.len()));
    } else if !lower.is_empty() && lower.chars().all(|character| character == 'm') {
        parsed.tokens.push(Token::Elapsed('m', lower.len()));
    } else if !lower.is_empty() && lower.chars().all(|character| character == 's') {
        parsed.tokens.push(Token::Elapsed('s', lower.len()));
    }
}

fn named_colour(name: &str) -> Option<&'static str> {
    Some(match name {
        "black" | "nero" => "000000",
        "blue" | "blu" => "0000FF",
        "cyan" | "ciano" => "00FFFF",
        "green" | "verde" => "008000",
        "magenta" => "FF00FF",
        "red" | "rosso" => "FF0000",
        "white" | "bianco" => "FFFFFF",
        "yellow" | "giallo" => "FFFF00",
        _ => return None,
    })
}

fn is_date_tokens(tokens: &[Token]) -> bool {
    !has_digits(tokens)
        && tokens.iter().any(|token| {
            matches!(
                token,
                Token::Year(_)
                    | Token::Month(_)
                    | Token::Day(_)
                    | Token::Hour(_)
                    | Token::Second(_)
                    | Token::Elapsed(..)
            )
        })
}

/// Whether a `.` starts the fraction of the seconds just read: `ss.00`.
fn after_seconds(tokens: &[Token], character: char, characters: &[char], index: usize) -> bool {
    character == '.'
        && characters.get(index + 1) == Some(&'0')
        && matches!(
            last_time_token(tokens),
            Some(Token::Second(_) | Token::Elapsed('s', _))
        )
}

fn has_digit_after(characters: &[char]) -> bool {
    characters
        .iter()
        .take_while(|character| !character.is_whitespace())
        .any(|character| matches!(character, '0'..='9' | '#' | '?'))
}

fn has_digits(tokens: &[Token]) -> bool {
    tokens.iter().any(|token| matches!(token, Token::Digit(_)))
}

fn last_time_token(tokens: &[Token]) -> Option<&Token> {
    tokens
        .iter()
        .rev()
        .find(|token| !matches!(token, Token::Literal(_)))
}

/// A month token is minutes when it follows an hour or comes before a second.
fn resolve_minutes(tokens: &mut [Token]) {
    for index in 0..tokens.len() {
        let Token::Month(length) = tokens[index] else {
            continue;
        };
        if length > 2 {
            continue;
        }
        let before = tokens[..index]
            .iter()
            .rev()
            .find(|token| !matches!(token, Token::Literal(_)));
        let after = tokens[index + 1..]
            .iter()
            .find(|token| !matches!(token, Token::Literal(_)));
        let follows_hour = matches!(before, Some(Token::Hour(_) | Token::Elapsed('h', _)));
        let before_second = matches!(after, Some(Token::Second(_) | Token::Elapsed('s', _)));
        if follows_hour || before_second {
            tokens[index] = Token::Minute(length);
        }
    }
}

/// The section a value is shown with, the value it shows and whether a minus sign goes before
/// it.
fn choose_section(sections: &[Section], value: f64) -> (Section, f64, bool) {
    let numeric: Vec<&Section> = sections
        .iter()
        .take(3)
        .filter(|section| !section.tokens.contains(&Token::Text))
        .collect();
    if numeric.iter().any(|section| section.condition.is_some()) {
        for section in &numeric {
            match &section.condition {
                Some((operator, threshold)) if compare(value, operator, *threshold) => {
                    return (
                        (*section).clone(),
                        value.abs(),
                        value < 0.0 && *operator != "<",
                    );
                }
                None => return ((*section).clone(), value.abs(), value < 0.0),
                Some(_) => {}
            }
        }
        let fallback = numeric
            .last()
            .map_or_else(Section::default, |section| (*section).clone());
        return (fallback, value.abs(), value < 0.0);
    }
    let zero = value.abs() < f64::MIN_POSITIVE;
    match numeric.len() {
        0 => (Section::default(), value.abs(), value < 0.0),
        1 => (numeric[0].clone(), value.abs(), value < 0.0),
        _ if value < 0.0 => (numeric[1].clone(), -value, false),
        3 if zero => (numeric[2].clone(), 0.0, false),
        _ => (numeric[0].clone(), value, false),
    }
}

fn compare(value: f64, operator: &str, threshold: f64) -> bool {
    match operator {
        "<" => value < threshold,
        "<=" => value <= threshold,
        ">" => value > threshold,
        ">=" => value >= threshold,
        "<>" => (value - threshold).abs() > f64::EPSILON,
        _ => (value - threshold).abs() <= f64::EPSILON,
    }
}

/// A positive value as a mantissa and an exponent; with several integer placeholders the
/// exponent is a multiple of their count, as engineering notation writes it.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the exponent of a finite f64 is within the range of i32"
)]
fn scientific(value: f64, integer_places: usize, decimals: usize) -> (f64, i32) {
    let step = i32::try_from(integer_places).unwrap_or(1);
    let magnitude = value.log10().floor() as i32;
    let mut exponent = if step > 1 {
        magnitude.div_euclid(step) * step
    } else {
        magnitude
    };
    let mut mantissa = value / 10f64.powi(exponent);
    // A rounding that reaches the next power, as 9.996 at two decimals, moves the exponent.
    if step == 1 && rounded_digits(mantissa, decimals).0.len() > 1 {
        mantissa /= 10.0;
        exponent += 1;
    }
    (mantissa, exponent)
}

fn format_numeric(value: f64, section: &Section, locale: NumberLocale) -> String {
    let tokens = &section.tokens;
    let percents = tokens
        .iter()
        .filter(|token| **token == Token::Percent)
        .count();
    let mut value = value * 100f64.powi(i32::try_from(percents).unwrap_or(0));
    if tokens.contains(&Token::Fraction) {
        return format_fraction(value, tokens);
    }
    let decimal_index = tokens
        .iter()
        .position(|token| *token == Token::DecimalPoint);
    let exponent_index = tokens
        .iter()
        .position(|token| matches!(token, Token::Exponent(_)));
    let integer_end = decimal_index.or(exponent_index).unwrap_or(tokens.len());

    // Commas right after the last digit placeholder of the number divide by a thousand each.
    let number_end = exponent_index.unwrap_or(tokens.len());
    let scaling = tokens[..number_end]
        .iter()
        .rposition(|token| matches!(token, Token::Digit(_)))
        .map_or(0, |last| {
            tokens[last + 1..number_end]
                .iter()
                .take_while(|token| **token == Token::Thousands)
                .count()
        });
    let last_digit = tokens[..integer_end]
        .iter()
        .rposition(|token| matches!(token, Token::Digit(_)));
    value /= 1000f64.powi(i32::try_from(scaling).unwrap_or(0));
    let first_digit = tokens
        .iter()
        .position(|token| matches!(token, Token::Digit(_)));
    let grouping = match (first_digit, last_digit) {
        (Some(first), Some(last)) => tokens[first..last].contains(&Token::Thousands),
        _ => false,
    };

    let decimal_places: Vec<char> = decimal_index.map_or_else(Vec::new, |start| {
        tokens[start + 1..exponent_index.unwrap_or(tokens.len())]
            .iter()
            .filter_map(|token| match token {
                Token::Digit(placeholder) => Some(*placeholder),
                _ => None,
            })
            .collect()
    });
    let decimals = decimal_places.len().min(MAX_DIGITS);

    let mut exponent = 0;
    if exponent_index.is_some() && value != 0.0 {
        let integer_places = tokens[..integer_end]
            .iter()
            .filter(|token| matches!(token, Token::Digit(_)))
            .count()
            .max(1);
        (value, exponent) = scientific(value, integer_places, decimals);
    }

    let (integer_digits, fraction_digits) = rounded_digits(value, decimals);
    let integer_pieces = integer_part(&integer_digits, tokens, integer_end, grouping, locale);
    let decimal_text = decimal_part(&fraction_digits, &decimal_places);

    let mut shown = String::new();
    let mut pieces = integer_pieces.iter();
    for (index, token) in tokens.iter().enumerate() {
        match token {
            Token::Literal(literal) => shown.push_str(literal),
            Token::Digit(_) if index < integer_end => {
                if let Some(piece) = pieces.next() {
                    shown.push_str(piece);
                }
            }
            Token::DecimalPoint => {
                shown.push(locale.decimal());
                shown.push_str(&decimal_text);
            }
            Token::Percent => shown.push('%'),
            Token::Exponent(plus) => {
                let digits = tokens[index + 1..]
                    .iter()
                    .take_while(|token| matches!(token, Token::Digit(_)))
                    .count()
                    .clamp(1, MAX_DIGITS);
                let sign = if exponent < 0 {
                    "-"
                } else if *plus {
                    "+"
                } else {
                    ""
                };
                shown.push('E');
                shown.push_str(sign);
                let _ = write!(shown, "{:0digits$}", exponent.abs());
            }
            Token::General => shown.push_str(&general_number(value, locale)),
            _ => {}
        }
    }
    shown
}

/// The digits of a non-negative value rounded to `decimals`, halves away from zero, read at the
/// fifteen significant digits Excel keeps: the integer digits without leading zeros, and the
/// decimals.
fn rounded_digits(value: f64, decimals: usize) -> (String, String) {
    let scientific = format!("{:.14e}", value.abs());
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let exponent: i64 = exponent.parse().unwrap_or(0);
    let mut digits: Vec<u8> = mantissa
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|digit| digit - b'0')
        .collect();
    // The value is 0.d1 d2 d3 ... times ten to `point`.
    let mut point = exponent + 1;
    let keep = point + i64::try_from(decimals).unwrap_or(0);
    if keep < 0 {
        return (String::new(), "0".repeat(decimals));
    }
    let keep = usize::try_from(keep).unwrap_or(0);
    if keep < digits.len() {
        let round_up = digits[keep] >= 5;
        digits.truncate(keep);
        if round_up {
            let mut index = digits.len();
            loop {
                if index == 0 {
                    digits.insert(0, 1);
                    point += 1;
                    break;
                }
                index -= 1;
                if digits[index] == 9 {
                    digits[index] = 0;
                } else {
                    digits[index] += 1;
                    break;
                }
            }
        }
    }
    let leading_zeros = usize::try_from(-point).unwrap_or(0);
    let point = usize::try_from(point).unwrap_or(0);
    let mut all: Vec<u8> = vec![0; leading_zeros];
    all.extend(digits);
    while all.len() < point + decimals {
        all.push(0);
    }
    let text =
        |slice: &[u8]| -> String { slice.iter().map(|digit| char::from(b'0' + digit)).collect() };
    let integer = text(&all[..point.min(all.len())]);
    let fraction = text(&all[point.min(all.len())..(point + decimals).min(all.len())]);
    (integer.trim_start_matches('0').to_owned(), fraction)
}

/// The integer digits laid over the integer placeholders, one piece per placeholder: `0` shows
/// a zero where there is no digit, `?` a space, `#` nothing, and the digits past the
/// placeholders go with the first. With grouping the whole number is the first piece, its
/// thousands separated.
fn integer_part(
    digits: &str,
    tokens: &[Token],
    end: usize,
    grouping: bool,
    locale: NumberLocale,
) -> Vec<String> {
    let placeholders: Vec<char> = tokens[..end]
        .iter()
        .filter_map(|token| match token {
            Token::Digit(placeholder) => Some(*placeholder),
            _ => None,
        })
        .collect();
    let digits: Vec<char> = digits.chars().collect();
    let extra = digits.len().saturating_sub(placeholders.len());
    let offset = placeholders.len().saturating_sub(digits.len());
    let mut pieces: Vec<String> = placeholders
        .iter()
        .enumerate()
        .map(|(index, placeholder)| {
            if index >= offset {
                digits[extra + index - offset].to_string()
            } else {
                match placeholder {
                    '0' => "0".to_owned(),
                    '?' => " ".to_owned(),
                    _ => String::new(),
                }
            }
        })
        .collect();
    if let Some(first) = pieces.first_mut() {
        let leading: String = digits[..extra].iter().collect();
        first.insert_str(0, &leading);
    }
    if !grouping {
        return pieces;
    }
    let whole: String = pieces.concat();
    let digit_count = whole.chars().filter(char::is_ascii_digit).count();
    let mut grouped = String::new();
    let mut seen = 0;
    for character in whole.chars() {
        if character.is_ascii_digit() {
            if seen > 0 && (digit_count - seen) % 3 == 0 {
                grouped.push(locale.thousands());
            }
            seen += 1;
        }
        grouped.push(character);
    }
    let mut grouped_pieces = vec![grouped];
    grouped_pieces.resize(pieces.len().max(1), String::new());
    grouped_pieces
}

/// The decimal digits laid over the decimal placeholders: trailing zeros a `#` stands for are
/// dropped, those of a `?` become spaces.
fn decimal_part(digits: &str, placeholders: &[char]) -> String {
    let mut shown: Vec<char> = digits.chars().collect();
    shown.resize(placeholders.len(), '0');
    for index in (0..placeholders.len()).rev() {
        if shown[index] != '0' {
            break;
        }
        match placeholders[index] {
            '#' => {
                shown.pop();
            }
            '?' => shown[index] = ' ',
            _ => break,
        }
    }
    shown.into_iter().collect()
}

/// A fraction: `# ?/?` shows the whole part and the nearest fraction with a denominator of as
/// many digits as the code gives it, or with the denominator the code writes out.
fn format_fraction(value: f64, tokens: &[Token]) -> String {
    let slash = tokens
        .iter()
        .position(|token| *token == Token::Fraction)
        .unwrap_or(0);
    let numerator_start = tokens[..slash]
        .iter()
        .rposition(|token| !matches!(token, Token::Digit(_)))
        .map_or(0, |position| position + 1);
    let has_whole = tokens[..numerator_start]
        .iter()
        .any(|token| matches!(token, Token::Digit(_)));
    let denominator_tokens: Vec<&Token> = tokens[slash + 1..]
        .iter()
        .take_while(|token| matches!(token, Token::Digit(_) | Token::Literal(_)))
        .collect();
    let fixed: String = denominator_tokens
        .iter()
        .filter_map(|token| match token {
            Token::Literal(literal) => Some(literal.trim().to_owned()),
            _ => None,
        })
        .collect();
    let digit_count = denominator_tokens
        .iter()
        .filter(|token| matches!(token, Token::Digit(_)))
        .count()
        .clamp(1, 4);
    let (whole, rest) = if has_whole {
        (value.trunc(), value.fract())
    } else {
        (0.0, value)
    };
    let (numerator, denominator) = match fixed.parse::<u32>() {
        Ok(denominator) if denominator > 0 => (
            (rest * f64::from(denominator)).round(),
            f64::from(denominator),
        ),
        _ => best_fraction(rest, 10u32.pow(u32::try_from(digit_count).unwrap_or(1)) - 1),
    };
    let (whole, numerator) = if numerator >= denominator && has_whole {
        (whole + 1.0, numerator - denominator)
    } else {
        (whole, numerator)
    };
    if has_whole {
        if numerator == 0.0 {
            return format!("{whole}");
        }
        if whole == 0.0 {
            return format!("{numerator}/{denominator}");
        }
        return format!("{whole} {numerator}/{denominator}");
    }
    format!("{numerator}/{denominator}")
}

/// The fraction nearest to `value` with a denominator up to `largest`.
fn best_fraction(value: f64, largest: u32) -> (f64, f64) {
    let mut best = (value.round(), 1.0);
    let mut best_error = (value - best.0).abs();
    for denominator in 1..=largest.max(1) {
        let denominator = f64::from(denominator);
        let numerator = (value * denominator).round();
        let error = (value - numerator / denominator).abs();
        if error < best_error - 1e-12 {
            best = (numerator, denominator);
            best_error = error;
        }
    }
    best
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "serials are checked to lie between 0 and the year 9999"
)]
#[expect(
    clippy::too_many_lines,
    reason = "one arm per date and time part a code can hold"
)]
fn format_date(serial: f64, section: &Section, locale: NumberLocale) -> String {
    let Some(moment) = Moment::from_serial(serial) else {
        return "#".repeat(8);
    };
    let twelve_hour = section
        .tokens
        .iter()
        .any(|token| matches!(token, Token::AmPm(_)));
    let mut shown = String::new();
    for token in &section.tokens {
        match token {
            Token::Literal(literal) => shown.push_str(literal),
            Token::Year(length) => {
                if *length <= 2 {
                    let _ = write!(shown, "{:02}", moment.year.rem_euclid(100));
                } else {
                    let _ = write!(shown, "{:04}", moment.year);
                }
            }
            Token::Month(length) => {
                let index = usize::try_from(moment.month - 1).unwrap_or(0);
                let name = match locale {
                    NumberLocale::Italian => MONTHS_IT[index],
                    NumberLocale::English => MONTHS_EN[index],
                };
                match length {
                    1 => shown.push_str(&moment.month.to_string()),
                    2 => {
                        let _ = write!(shown, "{:02}", moment.month);
                    }
                    3 => shown.extend(name.chars().take(3)),
                    5 => shown.extend(name.chars().take(1)),
                    _ => shown.push_str(name),
                }
            }
            Token::Day(length) => {
                let name = match locale {
                    NumberLocale::Italian => DAYS_IT[moment.weekday],
                    NumberLocale::English => DAYS_EN[moment.weekday],
                };
                match length {
                    1 => shown.push_str(&moment.day.to_string()),
                    2 => {
                        let _ = write!(shown, "{:02}", moment.day);
                    }
                    3 => shown.extend(name.chars().take(3)),
                    _ => shown.push_str(name),
                }
            }
            Token::Hour(length) => {
                let hour = if twelve_hour {
                    match moment.hour % 12 {
                        0 => 12,
                        other => other,
                    }
                } else {
                    moment.hour
                };
                if *length >= 2 {
                    let _ = write!(shown, "{hour:02}");
                } else {
                    shown.push_str(&hour.to_string());
                }
            }
            Token::Minute(length) => {
                if *length >= 2 {
                    let _ = write!(shown, "{:02}", moment.minute);
                } else {
                    shown.push_str(&moment.minute.to_string());
                }
            }
            Token::Second(length) => {
                if *length >= 2 {
                    let _ = write!(shown, "{:02}", moment.second);
                } else {
                    shown.push_str(&moment.second.to_string());
                }
            }
            Token::SubSecond(digits) => {
                let fraction = (moment.milliseconds as f64 / 1000.0
                    * 10f64.powi(i32::try_from(*digits).unwrap_or(1)))
                .floor();
                shown.push(locale.decimal());
                let _ = write!(shown, "{:0width$}", fraction as u64, width = *digits);
            }
            Token::Elapsed(unit, length) => {
                let total_seconds = (serial * 86_400.0).round();
                let value = match unit {
                    'h' => (total_seconds / 3_600.0).floor(),
                    'm' => (total_seconds / 60.0).floor(),
                    _ => total_seconds,
                };
                let _ = write!(shown, "{:0width$}", value as i64, width = *length);
            }
            Token::AmPm(full) => {
                let morning = moment.hour < 12;
                shown.push_str(match (full, morning) {
                    (true, true) => "AM",
                    (true, false) => "PM",
                    (false, true) => "A",
                    (false, false) => "P",
                });
            }
            Token::Digit(_)
            | Token::DecimalPoint
            | Token::Thousands
            | Token::Percent
            | Token::Exponent(_)
            | Token::Fraction
            | Token::Text
            | Token::General => {}
        }
    }
    shown
}

/// A date and time from an Excel serial in the 1900 system, which counts a 29 February 1900
/// that never was.
struct Moment {
    year: i64,
    month: i64,
    day: i64,
    /// 0 for Sunday.
    weekday: usize,
    hour: i64,
    minute: i64,
    second: i64,
    milliseconds: i64,
}

impl Moment {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the serial is checked to lie between 0 and the year 9999"
    )]
    fn from_serial(serial: f64) -> Option<Self> {
        if !(0.0..2_958_466.0).contains(&serial) {
            return None;
        }
        let total_milliseconds = (serial * 86_400_000.0).round() as i64;
        let days = total_milliseconds.div_euclid(86_400_000);
        let in_day = total_milliseconds.rem_euclid(86_400_000);
        let (year, month, day) = match days {
            0 => (1900, 1, 0),
            60 => (1900, 2, 29),
            _ => {
                // Serial 1 is 1 January 1900; past the false leap day every serial is one ahead.
                let real = if days > 60 { days - 1 } else { days };
                civil_from_days(real - 1 + days_from_civil(1900, 1, 1))
            }
        };
        let weekday = usize::try_from((days + 6).rem_euclid(7)).unwrap_or(0);
        Some(Self {
            year,
            month,
            day,
            weekday,
            hour: in_day / 3_600_000,
            minute: in_day / 60_000 % 60,
            second: in_day / 1_000 % 60,
            milliseconds: in_day % 1_000,
        })
    }
}

/// Days since 1 January 1970 of a civil date, Howard Hinnant's algorithm.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests;
