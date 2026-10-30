// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! Formulas: parsed into an expression tree and printed back, in the syntax xlsx files store
//! (English function names, `,` between arguments, `.` in numbers).
//!
//! What a person types is accepted too: the Italian names of the common functions (`SOMMA`,
//! `SE`, `CERCA.VERT`) become their English ones, and a formula that separates arguments with
//! `;` reads `,` as the decimal separator, as Italian spreadsheets do. [`parse_formula_in`] reads
//! a formula typed in a locale and [`print_formula_in`] shows one in it, so what an Italian
//! editor shows (`=SOMMA(A1;1,5)`) is what it reads back.

use std::fmt::Write as _;

use crate::address::{CellRef, MAX_ROWS, column_index, column_name, parse_cell};
use crate::value::{ErrorKind, Locale};

/// The deepest nesting a formula may have; a deeper one is refused rather than overflowing.
const MAX_DEPTH: usize = 128;
/// The longest formula, as Excel limits it.
pub const MAX_FORMULA_CHARS: usize = 8_192;

/// A cell named by a formula, with its sheet when it names one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellAddress {
    pub sheet: Option<String>,
    pub cell: CellRef,
    pub absolute_row: bool,
    pub absolute_column: bool,
}

/// A rectangle named by a formula; `whole_columns` for `A:C`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeAddress {
    pub sheet: Option<String>,
    pub start: CellAddress,
    pub end: CellAddress,
    pub whole_columns: bool,
}

impl RangeAddress {
    /// The top-left and bottom-right corners, whichever way the range was written.
    #[must_use]
    pub fn bounds(&self) -> (CellRef, CellRef) {
        let (start, end) = (self.start.cell, self.end.cell);
        (
            CellRef::new(start.row.min(end.row), start.column.min(end.column)),
            CellRef::new(start.row.max(end.row), start.column.max(end.column)),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Plus,
    Minus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    Concatenate,
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

impl BinaryOperator {
    fn precedence(self) -> u8 {
        match self {
            Self::Equal
            | Self::NotEqual
            | Self::Less
            | Self::LessOrEqual
            | Self::Greater
            | Self::GreaterOrEqual => 1,
            Self::Concatenate => 2,
            Self::Add | Self::Subtract => 3,
            Self::Multiply | Self::Divide => 4,
            Self::Power => 5,
        }
    }

    fn symbol(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Subtract => "-",
            Self::Multiply => "*",
            Self::Divide => "/",
            Self::Power => "^",
            Self::Concatenate => "&",
            Self::Equal => "=",
            Self::NotEqual => "<>",
            Self::Less => "<",
            Self::LessOrEqual => "<=",
            Self::Greater => ">",
            Self::GreaterOrEqual => ">=",
        }
    }
}

/// A parsed formula.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    Text(String),
    Bool(bool),
    Error(ErrorKind),
    Cell(CellAddress),
    Range(RangeAddress),
    Unary(UnaryOperator, Box<Expr>),
    Binary(BinaryOperator, Box<Expr>, Box<Expr>),
    Percent(Box<Expr>),
    /// Parentheses as written, kept so a formula prints back as it was typed.
    Group(Box<Expr>),
    /// A function by its English name in capitals.
    Call(String, Vec<Expr>),
    /// An array constant, `{1,2;3,4}`: rows of constants, every row as long as the first.
    Array(Vec<Vec<Expr>>),
    /// A name: a name the workbook defines, or one a `LET` or a `LAMBDA` binds, as typed.
    Name(String),
    /// A function a formula computes called with arguments, `LAMBDA(x, x*2)(3)`.
    Invoke(Box<Expr>, Vec<Expr>),
}

/// Why a formula could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid formula at character {position}: {reason}")]
pub struct FormulaError {
    pub position: usize,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Text(String),
    Error(ErrorKind),
    /// Letters, digits, `.`, `_` and `$`: a function, a boolean, a cell or a column.
    Word(String),
    /// A sheet name before `!`, quoted or not.
    Sheet(String),
    Operator(&'static str),
    Open,
    Close,
    Separator,
    Colon,
    /// `{` and `}` around an array constant, and what separates its rows.
    BraceOpen,
    BraceClose,
    RowSeparator,
}

/// Parse a formula, with or without its leading `=`.
///
/// # Errors
///
/// [`FormulaError`] for text that is not a formula, a formula longer than
/// [`MAX_FORMULA_CHARS`] or nested deeper than 128 levels.
pub fn parse_formula(formula: &str) -> Result<Expr, FormulaError> {
    let comma = if has_semicolon_outside_text(formula) {
        Comma::Decimal
    } else {
        Comma::Separator
    };
    parse_with(formula, comma)
}

/// Parse a formula as a person types it in a locale. In Italian a `,` between two digits is the
/// decimal separator even with no `;` in the formula, so `=A1*1,5` reads as Italian Excel reads
/// it; a `.` there is read as a decimal point too, as a formula pasted from English would have it.
///
/// # Errors
///
/// [`FormulaError`] as [`parse_formula`].
pub fn parse_formula_in(formula: &str, locale: Locale) -> Result<Expr, FormulaError> {
    let comma = match locale {
        Locale::English => Comma::Separator,
        Locale::Italian if has_semicolon_outside_text(formula) => Comma::Decimal,
        Locale::Italian => Comma::DecimalBetweenDigits,
    };
    parse_with(formula, comma)
}

/// How a `,` reads in a formula.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Comma {
    /// Between arguments, as files and English write it.
    Separator,
    /// In numbers, with `;` between arguments, as Italian writes it.
    Decimal,
    /// In a number when it has a digit on both sides, between arguments otherwise.
    DecimalBetweenDigits,
}

fn parse_with(formula: &str, comma: Comma) -> Result<Expr, FormulaError> {
    let formula = formula.strip_prefix('=').unwrap_or(formula);
    if formula.chars().count() > MAX_FORMULA_CHARS {
        return Err(FormulaError {
            position: MAX_FORMULA_CHARS,
            reason: "the formula is too long".to_owned(),
        });
    }
    let tokens = tokenize(formula, comma)?;
    let mut parser = Parser {
        tokens,
        position: 0,
        depth: 0,
    };
    let expression = parser.comparison()?;
    if parser.position < parser.tokens.len() {
        return Err(parser.error("unexpected text after the formula"));
    }
    Ok(expression)
}

#[expect(
    clippy::too_many_lines,
    reason = "one pass over the characters reads every token of the grammar"
)]
fn tokenize(formula: &str, comma: Comma) -> Result<Vec<(Token, usize)>, FormulaError> {
    let characters: Vec<char> = formula.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0;
    let mut in_array = false;
    let error = |position: usize, reason: &str| FormulaError {
        position,
        reason: reason.to_owned(),
    };
    while index < characters.len() {
        let character = characters[index];
        let start = index;
        match character {
            ' ' | '\t' | '\n' | '\r' => index += 1,
            '{' if !in_array => {
                in_array = true;
                index += 1;
                tokens.push((Token::BraceOpen, start));
            }
            '}' if in_array => {
                in_array = false;
                index += 1;
                tokens.push((Token::BraceClose, start));
            }
            // In an array the rows are apart by `;`, or, with `;` between columns as Italian
            // writes it, by `|` or `\`.
            ';' if in_array && comma == Comma::Separator => {
                index += 1;
                tokens.push((Token::RowSeparator, start));
            }
            '|' | '\\' if in_array => {
                index += 1;
                tokens.push((Token::RowSeparator, start));
            }
            '"' => {
                let text = quoted(&characters, &mut index, '"')
                    .ok_or_else(|| error(start, "a text is not closed"))?;
                tokens.push((Token::Text(text), start));
            }
            '\'' => {
                let name = quoted(&characters, &mut index, '\'')
                    .ok_or_else(|| error(start, "a sheet name is not closed"))?;
                if characters.get(index) != Some(&'!') {
                    return Err(error(index, "a quoted sheet name must be followed by !"));
                }
                index += 1;
                tokens.push((Token::Sheet(name), start));
            }
            '#' => {
                let rest: String = characters[index..].iter().collect();
                let Some((kind, length)) = ErrorKind::ALL
                    .iter()
                    .find(|kind| rest.to_ascii_uppercase().starts_with(kind.code()))
                    .map(|kind| (*kind, kind.code().chars().count()))
                else {
                    return Err(error(start, "unknown error value"));
                };
                index += length;
                tokens.push((Token::Error(kind), start));
            }
            letter if letter.is_alphabetic() || letter == '$' || letter == '_' => {
                let mut word = String::new();
                while let Some(&next) = characters.get(index) {
                    if next.is_alphanumeric() || matches!(next, '.' | '_' | '$') {
                        word.push(next);
                        index += 1;
                    } else {
                        break;
                    }
                }
                if characters.get(index) == Some(&'!') {
                    index += 1;
                    tokens.push((Token::Sheet(word), start));
                } else {
                    tokens.push((Token::Word(word), start));
                }
            }
            digit
                if digit.is_ascii_digit()
                    || (digit == '.' && comma != Comma::Decimal)
                    || (digit == ',' && comma == Comma::Decimal) =>
            {
                let number = read_number(&characters, &mut index, comma);
                // A number glued to letters is a cell such as `1A`, which is not one.
                if characters
                    .get(index)
                    .is_some_and(|next| next.is_ascii_alphabetic() || *next == '$')
                {
                    return Err(error(start, "a number cannot be followed by letters"));
                }
                let value = number
                    .parse::<f64>()
                    .map_err(|_| error(start, "invalid number"))?;
                tokens.push((Token::Number(value), start));
            }
            symbol => {
                let next = characters.get(index + 1).copied();
                let Some((token, length)) = symbol_token(symbol, next, comma) else {
                    return Err(error(start, "unexpected character"));
                };
                index += length;
                tokens.push((token, start));
            }
        }
    }
    Ok(tokens)
}

/// The token of a symbol and how many characters it takes; `None` for no symbol of formulas.
fn symbol_token(symbol: char, next: Option<char>, comma: Comma) -> Option<(Token, usize)> {
    let operator = |text: &'static str| Some((Token::Operator(text), text.len()));
    match (symbol, next) {
        ('(', _) => Some((Token::Open, 1)),
        (')', _) => Some((Token::Close, 1)),
        (':', _) => Some((Token::Colon, 1)),
        (';', _) => Some((Token::Separator, 1)),
        (',', _) if comma != Comma::Decimal => Some((Token::Separator, 1)),
        ('<', Some('=')) => operator("<="),
        ('<', Some('>')) => operator("<>"),
        ('>', Some('=')) => operator(">="),
        ('<', _) => operator("<"),
        ('>', _) => operator(">"),
        ('+', _) => operator("+"),
        ('-', _) => operator("-"),
        ('*', _) => operator("*"),
        ('/', _) => operator("/"),
        ('^', _) => operator("^"),
        ('&', _) => operator("&"),
        ('=', _) => operator("="),
        ('%', _) => operator("%"),
        _ => None,
    }
}

/// The digits of a number at `index`, with the decimal separator `comma` says and an optional
/// exponent, written with a `.` for Rust to parse; `index` ends past the number.
fn read_number(characters: &[char], index: &mut usize, comma: Comma) -> String {
    let mut number = String::new();
    while let Some(&next) = characters.get(*index) {
        let digit_after = characters.get(*index + 1).is_some_and(char::is_ascii_digit);
        let decimal = match comma {
            Comma::Separator => next == '.',
            Comma::Decimal => next == ',',
            Comma::DecimalBetweenDigits => {
                next == '.' || (next == ',' && digit_after && !number.is_empty())
            }
        };
        if next.is_ascii_digit() {
            number.push(next);
        } else if decimal && !number.contains('.') {
            number.push('.');
        } else if matches!(next, 'e' | 'E')
            && characters
                .get(*index + 1)
                .is_some_and(|after| after.is_ascii_digit() || matches!(after, '+' | '-'))
        {
            number.push('e');
            if let Some(&sign) = characters
                .get(*index + 1)
                .filter(|sign| matches!(sign, '+' | '-'))
            {
                number.push(sign);
                *index += 1;
            }
        } else {
            break;
        }
        *index += 1;
    }
    number
}

/// The text between a quote at `index` and its closing quote, a doubled quote standing for
/// one; `index` ends past the closing quote. `None` when the text is not closed.
fn quoted(characters: &[char], index: &mut usize, quote: char) -> Option<String> {
    let mut text = String::new();
    *index += 1;
    loop {
        match characters.get(*index) {
            None => return None,
            Some(&character) if character == quote => {
                if characters.get(*index + 1) == Some(&quote) {
                    text.push(quote);
                    *index += 2;
                } else {
                    *index += 1;
                    return Some(text);
                }
            }
            Some(&other) => {
                text.push(other);
                *index += 1;
            }
        }
    }
}

fn has_semicolon_outside_text(formula: &str) -> bool {
    let mut in_text = false;
    let mut in_sheet = false;
    let mut in_array = false;
    for character in formula.chars() {
        match character {
            '"' if !in_sheet => in_text = !in_text,
            '\'' if !in_text => in_sheet = !in_sheet,
            // Inside an array constant a `;` separates its rows, not arguments.
            '{' if !in_text && !in_sheet => in_array = true,
            '}' if !in_text && !in_sheet => in_array = false,
            ';' if !in_text && !in_sheet && !in_array => return true,
            _ => {}
        }
    }
    false
}

struct Parser {
    tokens: Vec<(Token, usize)>,
    position: usize,
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position).map(|(token, _)| token)
    }

    fn error(&self, reason: &str) -> FormulaError {
        FormulaError {
            position: self.tokens.get(self.position).map_or_else(
                || self.tokens.last().map_or(0, |(_, position)| position + 1),
                |(_, position)| *position,
            ),
            reason: reason.to_owned(),
        }
    }

    fn operator(&self) -> Option<&'static str> {
        match self.peek() {
            Some(Token::Operator(operator)) => Some(operator),
            _ => None,
        }
    }

    fn enter(&mut self) -> Result<(), FormulaError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.error("the formula is nested too deeply"));
        }
        Ok(())
    }

    fn comparison(&mut self) -> Result<Expr, FormulaError> {
        self.enter()?;
        let mut left = self.concatenation()?;
        while let Some(operator) = self.operator() {
            let operator = match operator {
                "=" => BinaryOperator::Equal,
                "<>" => BinaryOperator::NotEqual,
                "<" => BinaryOperator::Less,
                "<=" => BinaryOperator::LessOrEqual,
                ">" => BinaryOperator::Greater,
                ">=" => BinaryOperator::GreaterOrEqual,
                _ => break,
            };
            self.position += 1;
            let right = self.concatenation()?;
            left = Expr::Binary(operator, Box::new(left), Box::new(right));
        }
        self.depth -= 1;
        Ok(left)
    }

    fn concatenation(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.additive()?;
        while self.operator() == Some("&") {
            self.position += 1;
            let right = self.additive()?;
            left = Expr::Binary(BinaryOperator::Concatenate, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn additive(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.multiplicative()?;
        while let Some(operator @ ("+" | "-")) = self.operator() {
            self.position += 1;
            let right = self.multiplicative()?;
            let operator = if operator == "+" {
                BinaryOperator::Add
            } else {
                BinaryOperator::Subtract
            };
            left = Expr::Binary(operator, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn multiplicative(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.power()?;
        while let Some(operator @ ("*" | "/")) = self.operator() {
            self.position += 1;
            let right = self.power()?;
            let operator = if operator == "*" {
                BinaryOperator::Multiply
            } else {
                BinaryOperator::Divide
            };
            left = Expr::Binary(operator, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn power(&mut self) -> Result<Expr, FormulaError> {
        let mut left = self.unary()?;
        while self.operator() == Some("^") {
            self.position += 1;
            let right = self.unary()?;
            left = Expr::Binary(BinaryOperator::Power, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    /// Unary signs bind tighter than `^`, as in Excel: `-2^2` is 4.
    fn unary(&mut self) -> Result<Expr, FormulaError> {
        match self.operator() {
            Some("-") => {
                self.position += 1;
                self.enter()?;
                let operand = self.unary()?;
                self.depth -= 1;
                Ok(Expr::Unary(UnaryOperator::Minus, Box::new(operand)))
            }
            Some("+") => {
                self.position += 1;
                self.enter()?;
                let operand = self.unary()?;
                self.depth -= 1;
                Ok(Expr::Unary(UnaryOperator::Plus, Box::new(operand)))
            }
            _ => self.percent(),
        }
    }

    fn percent(&mut self) -> Result<Expr, FormulaError> {
        let mut operand = self.primary()?;
        while self.operator() == Some("%") {
            self.position += 1;
            operand = Expr::Percent(Box::new(operand));
        }
        Ok(operand)
    }

    fn primary(&mut self) -> Result<Expr, FormulaError> {
        let Some((token, _)) = self.tokens.get(self.position).cloned() else {
            return Err(self.error("the formula ends too early"));
        };
        self.position += 1;
        match token {
            Token::Number(value) => Ok(Expr::Number(value)),
            Token::Text(text) => Ok(Expr::Text(text)),
            Token::Error(kind) => Ok(Expr::Error(kind)),
            Token::BraceOpen => self.array(),
            Token::Open => {
                let inner = self.comparison()?;
                if self.peek() != Some(&Token::Close) {
                    return Err(self.error("a parenthesis is not closed"));
                }
                self.position += 1;
                self.invocations(Expr::Group(Box::new(inner)))
            }
            Token::Sheet(sheet) => {
                let Some((Token::Word(word), _)) = self.tokens.get(self.position).cloned() else {
                    return Err(self.error("a sheet name must be followed by a cell"));
                };
                self.position += 1;
                self.reference(Some(sheet), &word)
            }
            Token::Word(word) => {
                if self.peek() == Some(&Token::Open) {
                    self.position += 1;
                    let call = self.call(&word)?;
                    return self.invocations(call);
                }
                match word.to_ascii_uppercase().as_str() {
                    "TRUE" | "VERO" => return Ok(Expr::Bool(true)),
                    "FALSE" | "FALSO" => return Ok(Expr::Bool(false)),
                    _ => {}
                }
                self.reference(None, &word)
            }
            _ => {
                self.position -= 1;
                Err(self.error("expected a value"))
            }
        }
    }

    /// The rows of an array constant after its `{`: constants, a sign before a number included.
    fn array(&mut self) -> Result<Expr, FormulaError> {
        let mut rows: Vec<Vec<Expr>> = vec![Vec::new()];
        loop {
            let element = self.unary()?;
            let constant = match &element {
                Expr::Number(_) | Expr::Text(_) | Expr::Bool(_) | Expr::Error(_) => true,
                Expr::Unary(_, operand) => matches!(**operand, Expr::Number(_)),
                _ => false,
            };
            if !constant {
                return Err(self.error("an array holds only constants"));
            }
            if let Some(row) = rows.last_mut() {
                row.push(element);
            }
            match self.peek() {
                Some(Token::Separator) => self.position += 1,
                Some(Token::RowSeparator) => {
                    self.position += 1;
                    rows.push(Vec::new());
                }
                Some(Token::BraceClose) => {
                    self.position += 1;
                    break;
                }
                _ => return Err(self.error("an array is not closed")),
            }
        }
        let width = rows[0].len();
        if rows.iter().any(|row| row.len() != width) {
            return Err(self.error("the rows of an array differ in length"));
        }
        Ok(Expr::Array(rows))
    }

    /// A function a formula computed, called: `LAMBDA(x, x*2)(3)`, as many times as written.
    fn invocations(&mut self, mut callee: Expr) -> Result<Expr, FormulaError> {
        while self.peek() == Some(&Token::Open) {
            self.position += 1;
            let Expr::Call(_, arguments) = self.call("")? else {
                return Err(self.error("expected arguments"));
            };
            callee = Expr::Invoke(Box::new(callee), arguments);
        }
        Ok(callee)
    }

    fn call(&mut self, name: &str) -> Result<Expr, FormulaError> {
        self.enter()?;
        let name = canonical_function_name(name);
        let mut arguments = Vec::new();
        if self.peek() == Some(&Token::Close) {
            self.position += 1;
            self.depth -= 1;
            return Ok(Expr::Call(name, arguments));
        }
        loop {
            // An empty argument, as in `IF(A1,,2)`, is an empty value.
            if matches!(self.peek(), Some(Token::Separator | Token::Close)) {
                arguments.push(Expr::Text(String::new()));
            } else {
                arguments.push(self.comparison()?);
            }
            match self.peek() {
                Some(Token::Separator) => self.position += 1,
                Some(Token::Close) => {
                    self.position += 1;
                    break;
                }
                _ => return Err(self.error("expected , or ) in the arguments")),
            }
        }
        self.depth -= 1;
        Ok(Expr::Call(name, arguments))
    }

    /// A cell, a range of cells or a range of columns, after its optional sheet.
    fn reference(&mut self, sheet: Option<String>, word: &str) -> Result<Expr, FormulaError> {
        let start = parse_address(sheet.as_ref(), word);
        if self.peek() == Some(&Token::Colon) {
            self.position += 1;
            let Some((Token::Word(end_word), _)) = self.tokens.get(self.position).cloned() else {
                return Err(self.error("a range must end with a cell"));
            };
            self.position += 1;
            if let (Some(start), Some(end)) =
                (start.clone(), parse_address(sheet.as_ref(), &end_word))
            {
                return Ok(Expr::Range(RangeAddress {
                    sheet,
                    start,
                    end,
                    whole_columns: false,
                }));
            }
            if let (Some(start), Some(end)) = (parse_column(word), parse_column(&end_word)) {
                let corner = |(column, absolute_column): (u32, bool), row: u32| CellAddress {
                    sheet: sheet.clone(),
                    cell: CellRef::new(row, column),
                    absolute_row: false,
                    absolute_column,
                };
                return Ok(Expr::Range(RangeAddress {
                    start: corner(start, 0),
                    end: corner(end, MAX_ROWS - 1),
                    sheet,
                    whole_columns: true,
                }));
            }
            return Err(self.error("not a range of cells"));
        }
        match start {
            Some(address) => Ok(Expr::Cell(address)),
            // A word that is no cell is a name: the workbook may define it, a `LET` or a
            // `LAMBDA` may bind it, and otherwise it computes Excel's #NAME?.
            None if sheet.is_none()
                && word.chars().all(|character| {
                    character.is_alphanumeric() || matches!(character, '.' | '_' | '\\')
                }) =>
            {
                Ok(Expr::Name(canonical_name(word)))
            }
            None => Err(self.error("not a cell")),
        }
    }
}

fn parse_address(sheet: Option<&String>, word: &str) -> Option<CellAddress> {
    let (cell, absolute_column, absolute_row, rest) = parse_cell(word.as_bytes())?;
    rest.is_empty().then(|| CellAddress {
        sheet: sheet.cloned(),
        cell,
        absolute_row,
        absolute_column,
    })
}

fn parse_column(word: &str) -> Option<(u32, bool)> {
    let (absolute, letters) = match word.strip_prefix('$') {
        Some(letters) => (true, letters),
        None => (false, word),
    };
    column_index(letters).map(|column| (column, absolute))
}

/// The Italian names of the functions and their English ones, as Excel names them in each language.
const ITALIAN_FUNCTION_NAMES: &[(&str, &str)] = &[
    ("SOMMA", "SUM"),
    ("MEDIA", "AVERAGE"),
    ("CONTA.NUMERI", "COUNT"),
    ("CONTA.VALORI", "COUNTA"),
    ("CONTA.VUOTE", "COUNTBLANK"),
    ("SE", "IF"),
    ("SE.ERRORE", "IFERROR"),
    ("E", "AND"),
    ("O", "OR"),
    ("NON", "NOT"),
    ("ARROTONDA", "ROUND"),
    ("ARROTONDA.PER.ECC", "ROUNDUP"),
    ("ARROTONDA.PER.DIF", "ROUNDDOWN"),
    ("ASS", "ABS"),
    ("INT", "INT"),
    ("RESTO", "MOD"),
    ("POTENZA", "POWER"),
    ("RADQ", "SQRT"),
    ("CONCATENA", "CONCATENATE"),
    ("LUNGHEZZA", "LEN"),
    ("SINISTRA", "LEFT"),
    ("DESTRA", "RIGHT"),
    ("STRINGA.ESTRAI", "MID"),
    ("MAIUSC", "UPPER"),
    ("MINUSC", "LOWER"),
    ("ANNULLA.SPAZI", "TRIM"),
    ("SOMMA.SE", "SUMIF"),
    ("CONTA.SE", "COUNTIF"),
    ("MEDIA.SE", "AVERAGEIF"),
    ("CERCA.VERT", "VLOOKUP"),
    ("CERCA.ORIZZ", "HLOOKUP"),
    ("INDICE", "INDEX"),
    ("CONFRONTA", "MATCH"),
    ("PRODOTTO", "PRODUCT"),
    ("VAL.VUOTO", "ISBLANK"),
    ("VAL.NUMERO", "ISNUMBER"),
    ("VAL.TESTO", "ISTEXT"),
    ("VAL.ERRORE", "ISERROR"),
    ("OGGI", "TODAY"),
    ("ADESSO", "NOW"),
    ("DATA", "DATE"),
    ("ANNO", "YEAR"),
    ("MESE", "MONTH"),
    ("GIORNO", "DAY"),
    ("VERO", "TRUE"),
    ("FALSO", "FALSE"),
    ("SOMMA.PIÙ.SE", "SUMIFS"),
    ("CONTA.PIÙ.SE", "COUNTIFS"),
    ("MATR.SOMMA.PRODOTTO", "SUMPRODUCT"),
    ("ARROTONDA.ECCESSO", "CEILING"),
    ("ARROTONDA.DIFETTO", "FLOOR"),
    ("ARROTONDA.ECCESSO.MAT", "CEILING.MATH"),
    ("ARROTONDA.DIFETTO.MAT", "FLOOR.MATH"),
    ("ARROTONDA.MULTIPLO", "MROUND"),
    ("TRONCA", "TRUNC"),
    ("PARI", "EVEN"),
    ("DISPARI", "ODD"),
    ("FATTORIALE", "FACT"),
    ("MCD", "GCD"),
    ("MCM", "LCM"),
    ("QUOZIENTE", "QUOTIENT"),
    ("SOMMA.Q", "SUMSQ"),
    ("RADIANTI", "RADIANS"),
    ("GRADI", "DEGREES"),
    ("SEN", "SIN"),
    ("ARCSEN", "ASIN"),
    ("ARCCOS", "ACOS"),
    ("ARCTAN", "ATAN"),
    ("MEDIANA", "MEDIAN"),
    ("GRANDE", "LARGE"),
    ("PICCOLO", "SMALL"),
    ("RANGO", "RANK"),
    ("RANGO.UG", "RANK.EQ"),
    ("DEV.ST", "STDEV"),
    ("DEV.ST.C", "STDEV.S"),
    ("DEV.ST.POP", "STDEVP"),
    ("DEV.ST.P", "STDEV.P"),
    ("VAR.C", "VAR.S"),
    ("VAR.POP", "VARP"),
    ("MODA", "MODE"),
    ("MODA.SNGL", "MODE.SNGL"),
    ("MAX.PIÙ.SE", "MAXIFS"),
    ("MIN.PIÙ.SE", "MINIFS"),
    ("MEDIA.PIÙ.SE", "AVERAGEIFS"),
    ("PIÙ.SE", "IFS"),
    ("SE.NON.DISP.", "IFNA"),
    ("NON.DISP", "NA"),
    ("VAL.NON.DISP", "ISNA"),
    ("VAL.ERR", "ISERR"),
    ("VAL.LOGICO", "ISLOGICAL"),
    ("VAL.NON.TESTO", "ISNONTEXT"),
    ("VAL.PARI", "ISEVEN"),
    ("VAL.DISPARI", "ISODD"),
    ("TESTO", "TEXT"),
    ("VALORE", "VALUE"),
    ("SOSTITUISCI", "SUBSTITUTE"),
    ("RIMPIAZZA", "REPLACE"),
    ("TROVA", "FIND"),
    ("RICERCA", "SEARCH"),
    ("RIPETI", "REPT"),
    ("TESTO.UNISCI", "TEXTJOIN"),
    ("IDENTICO", "EXACT"),
    ("MAIUSC.INIZ", "PROPER"),
    ("CODICE.CARATT", "CHAR"),
    ("CODICE", "CODE"),
    ("LIBERA", "CLEAN"),
    ("NUM", "N"),
    ("SCEGLI", "CHOOSE"),
    ("RIGHE", "ROWS"),
    ("COLONNE", "COLUMNS"),
    ("RIF.RIGA", "ROW"),
    ("RIF.COLONNA", "COLUMN"),
    ("CERCA.X", "XLOOKUP"),
    ("CERCA", "LOOKUP"),
    ("DATA.DIFF", "DATEDIF"),
    ("DATA.MESE", "EDATE"),
    ("FINE.MESE", "EOMONTH"),
    ("GIORNO.SETTIMANA", "WEEKDAY"),
    ("NUM.SETTIMANA", "WEEKNUM"),
    ("NUM.SETTIMANA.ISO.ANNO", "ISOWEEKNUM"),
    ("GIORNI.LAVORATIVI.TOT", "NETWORKDAYS"),
    ("GIORNO.LAVORATIVO", "WORKDAY"),
    ("GIORNI", "DAYS"),
    ("ORA", "HOUR"),
    ("MINUTO", "MINUTE"),
    ("SECONDO", "SECOND"),
    ("ORARIO", "TIME"),
    ("DATA.VALORE", "DATEVALUE"),
    ("ORARIO.VALORE", "TIMEVALUE"),
    ("FRAZIONE.ANNO", "YEARFRAC"),
    ("RATA", "PMT"),
    ("VAL.FUT", "FV"),
    ("VA", "PV"),
    ("NUM.RATE", "NPER"),
    ("VAN", "NPV"),
    ("TIR.COST", "IRR"),
    ("TASSO", "RATE"),
    ("INTERESSI", "IPMT"),
    ("P.RATA", "PPMT"),
    ("SENH", "SINH"),
    ("ARCSENH", "ASINH"),
    ("ARCCOSH", "ACOSH"),
    ("ARCTANH", "ATANH"),
    ("ARCCOT", "ACOT"),
    ("ARCCOTH", "ACOTH"),
    ("ARCTAN.2", "ATAN2"),
    ("RADQ.PI.GRECO", "SQRTPI"),
    ("FATT.DOPPIO", "FACTDOUBLE"),
    ("COMBINAZIONE", "COMBIN"),
    ("COMBINAZIONE.VALORI", "COMBINA"),
    ("PERMUTAZIONE", "PERMUT"),
    ("PERMUTAZIONE.VALORI", "PERMUTATIONA"),
    ("MULTINOMIALE", "MULTINOMIAL"),
    ("SOMMA.DIFF.Q", "SUMX2MY2"),
    ("SOMMA.SOMMA.Q", "SUMX2PY2"),
    ("SOMMA.Q.DIFF", "SUMXMY2"),
    ("ARROTONDA.ECCESSO.PRECISA", "CEILING.PRECISE"),
    ("ISO.ARROTONDA.ECCESSO", "ISO.CEILING"),
    ("ARROTONDA.DIFETTO.PRECISA", "FLOOR.PRECISE"),
    ("ROMANO", "ROMAN"),
    ("ARABO", "ARABIC"),
    ("DECIMALE", "DECIMAL"),
    ("SUBTOTALE", "SUBTOTAL"),
    ("SOGLIA", "GESTEP"),
    ("MEDIA.VALORI", "AVERAGEA"),
    ("MAX.VALORI", "MAXA"),
    ("MIN.VALORI", "MINA"),
    ("DEV.ST.VALORI", "STDEVA"),
    ("DEV.ST.POP.VALORI", "STDEVPA"),
    ("VAR.VALORI", "VARA"),
    ("VAR.POP.VALORI", "VARPA"),
    ("MEDIA.GEOMETRICA", "GEOMEAN"),
    ("MEDIA.ARMONICA", "HARMEAN"),
    ("MEDIA.DEV", "AVEDEV"),
    ("DEV.Q", "DEVSQ"),
    ("CURTOSI", "KURT"),
    ("ASIMMETRIA", "SKEW"),
    ("ESC.PERCENTILE", "PERCENTILE.EXC"),
    ("ESC.QUARTILE", "QUARTILE.EXC"),
    ("PERCENT.RANGO", "PERCENTRANK"),
    ("PERCENT.RANGO.INC", "PERCENTRANK.INC"),
    ("ESC.PERCENT.RANGO", "PERCENTRANK.EXC"),
    ("RANGO.MEDIA", "RANK.AVG"),
    ("MEDIA.TRONCATA", "TRIMMEAN"),
    ("NORMALIZZA", "STANDARDIZE"),
    ("CORRELAZIONE", "CORREL"),
    ("RQ", "RSQ"),
    ("PENDENZA", "SLOPE"),
    ("INTERCETTA", "INTERCEPT"),
    ("COVARIANZA", "COVAR"),
    ("COVARIANZA.P", "COVARIANCE.P"),
    ("COVARIANZA.C", "COVARIANCE.S"),
    ("ERR.STD.YX", "STEYX"),
    ("PREVISIONE", "FORECAST"),
    ("PREVISIONE.LINEARE", "FORECAST.LINEAR"),
    ("DISTRIB.NORM.N", "NORM.DIST"),
    ("DISTRIB.NORM", "NORMDIST"),
    ("DISTRIB.NORM.ST.N", "NORM.S.DIST"),
    ("DISTRIB.NORM.ST", "NORMSDIST"),
    ("INV.NORM.N", "NORM.INV"),
    ("INV.NORM", "NORMINV"),
    ("INV.NORM.S", "NORM.S.INV"),
    ("INV.NORM.ST", "NORMSINV"),
    ("CONFIDENZA", "CONFIDENCE"),
    ("CONFIDENZA.NORM", "CONFIDENCE.NORM"),
    ("DISTRIB.EXP.N", "EXPON.DIST"),
    ("DISTRIB.EXP", "EXPONDIST"),
    ("DISTRIB.POISSON", "POISSON.DIST"),
    ("DISTRIB.BINOM.N", "BINOM.DIST"),
    ("DISTRIB.BINOM", "BINOMDIST"),
    ("INV.FISHER", "FISHERINV"),
    ("LN.GAMMA", "GAMMALN"),
    ("LN.GAMMA.PRECISA", "GAMMALN.PRECISE"),
    ("AMMORT.COST", "SLN"),
    ("AMMORT.ANNUO", "SYD"),
    ("AMMORT", "DDB"),
    ("AMMORT.FISSO", "DB"),
    ("EFFETTIVO", "EFFECT"),
    ("NOMINALE", "NOMINAL"),
    ("INTERESSE.RATA", "ISPMT"),
    ("INT.CUMUL", "CUMIPMT"),
    ("CAP.CUM", "CUMPRINC"),
    ("TIR.VAR", "MIRR"),
    ("VAN.X", "XNPV"),
    ("TIR.X", "XIRR"),
    ("VAL.FUT.CAPITALE", "FVSCHEDULE"),
    ("DURATA.P", "PDURATION"),
    ("RIT.INVEST.EFFETT", "RRI"),
    ("VALUTA.DEC", "DOLLARDE"),
    ("VALUTA.FRAZ", "DOLLARFR"),
    ("GIORNO360", "DAYS360"),
    ("GIORNO.LAVORATIVO.INTL", "WORKDAY.INTL"),
    ("GIORNI.LAVORATIVI.TOT.INTL", "NETWORKDAYS.INTL"),
    ("BINARIO.DECIMALE", "BIN2DEC"),
    ("BINARIO.HEX", "BIN2HEX"),
    ("BINARIO.OCT", "BIN2OCT"),
    ("DECIMALE.BINARIO", "DEC2BIN"),
    ("DECIMALE.HEX", "DEC2HEX"),
    ("DECIMALE.OCT", "DEC2OCT"),
    ("HEX.BINARIO", "HEX2BIN"),
    ("HEX.DECIMALE", "HEX2DEC"),
    ("HEX.OCT", "HEX2OCT"),
    ("OCT.BINARIO", "OCT2BIN"),
    ("OCT.DECIMALE", "OCT2DEC"),
    ("OCT.HEX", "OCT2HEX"),
    ("BIT.SPOSTA.SX", "BITLSHIFT"),
    ("BIT.SPOSTA.DX", "BITRSHIFT"),
    ("CONVERTI", "CONVERT"),
    ("CARATT.UNI", "UNICHAR"),
    ("FISSO", "FIXED"),
    ("VALUTA", "DOLLAR"),
    ("NUMERO.VALORE", "NUMBERVALUE"),
    ("TESTO.PRIMA", "TEXTBEFORE"),
    ("TESTO.DOPO", "TEXTAFTER"),
    ("COLLEG.IPERTESTUALE", "HYPERLINK"),
    ("VAL.RIF", "ISREF"),
    ("TIPO", "TYPE"),
    ("ERRORE.TIPO", "ERROR.TYPE"),
    ("INDIRIZZO", "ADDRESS"),
    ("FILTRO", "FILTER"),
    ("DATI.ORDINA", "SORT"),
    ("DATI.ORDINA.PER", "SORTBY"),
    ("UNICI", "UNIQUE"),
    ("SEQUENZA", "SEQUENCE"),
    ("MATR.TRASPOSTA", "TRANSPOSE"),
    ("MATR.PRODOTTO", "MMULT"),
    ("MATR.UNIT", "MUNIT"),
    ("FREQUENZA", "FREQUENCY"),
    ("PRENDI", "TAKE"),
    ("ESCLUDI", "DROP"),
    ("SCEGLI.RIGA", "CHOOSEROWS"),
    ("SCEGLI.COL", "CHOOSECOLS"),
    ("STACK.VERT", "VSTACK"),
    ("STACK.ORIZ", "HSTACK"),
    ("A.COL", "TOCOL"),
    ("A.RIGA", "TOROW"),
    ("ESPANDI", "EXPAND"),
    ("DIVIDI.TESTO", "TEXTSPLIT"),
    ("INT.MATURATO.PER", "ACCRINT"),
    ("INT.MATURATO.SCAD", "ACCRINTM"),
    ("AGGREGA", "AGGREGATE"),
    ("AMMORT.DEGR", "AMORDEGRC"),
    ("AMMORT.PER", "AMORLINC"),
    ("AREE", "AREAS"),
    ("MATRICE.A.TESTO", "ARRAYTOTEXT"),
    ("BESSEL.I", "BESSELI"),
    ("BESSEL.J", "BESSELJ"),
    ("BESSEL.K", "BESSELK"),
    ("BESSEL.Y", "BESSELY"),
    ("DISTRIB.BETA", "BETADIST"),
    ("DISTRIB.BETA.N", "BETA.DIST"),
    ("INV.BETA", "BETAINV"),
    ("INV.BETA.N", "BETA.INV"),
    ("INTERVALLO.DISTRIB.BINOM.N", "BINOM.DIST.RANGE"),
    ("INV.BINOM", "BINOM.INV"),
    ("CELLA", "CELL"),
    ("DISTRIB.CHI", "CHIDIST"),
    ("INV.CHI", "CHIINV"),
    ("TEST.CHI", "CHITEST"),
    ("DISTRIB.CHI.QUAD", "CHISQ.DIST"),
    ("DISTRIB.CHI.QUAD.DS", "CHISQ.DIST.RT"),
    ("INV.CHI.QUAD", "CHISQ.INV"),
    ("INV.CHI.QUAD.DS", "CHISQ.INV.RT"),
    ("TEST.CHI.QUAD", "CHISQ.TEST"),
    ("COMPLESSO", "COMPLEX"),
    ("CONFIDENZA.T", "CONFIDENCE.T"),
    ("GIORNI.CED.INIZ.LIQ", "COUPDAYBS"),
    ("GIORNI.CED", "COUPDAYS"),
    ("GIORNI.CED.NUOVA", "COUPDAYSNC"),
    ("DATA.CED.SUCC", "COUPNCD"),
    ("NUM.CED", "COUPNUM"),
    ("DATA.CED.PREC", "COUPPCD"),
    ("CRIT.BINOM", "CRITBINOM"),
    ("DB.MEDIA", "DAVERAGE"),
    ("DB.CONTA.NUMERI", "DCOUNT"),
    ("DB.CONTA.VALORI", "DCOUNTA"),
    ("DB.VALORI", "DGET"),
    ("TASSO.SCONTO", "DISC"),
    ("DB.MAX", "DMAX"),
    ("DB.MIN", "DMIN"),
    ("DB.PRODOTTO", "DPRODUCT"),
    ("DB.DEV.ST", "DSTDEV"),
    ("DB.DEV.ST.POP", "DSTDEVP"),
    ("DB.SOMMA", "DSUM"),
    ("DURATA", "DURATION"),
    ("DB.VAR", "DVAR"),
    ("DB.VAR.POP", "DVARP"),
    ("CODIFICA.URL", "ENCODEURL"),
    ("FUNZ.ERRORE", "ERF"),
    ("FUNZ.ERRORE.PRECISA", "ERF.PRECISE"),
    ("FUNZ.ERRORE.COMP", "ERFC"),
    ("FUNZ.ERRORE.COMP.PRECISA", "ERFC.PRECISE"),
    ("DISTRIBF", "F.DIST"),
    ("DISTRIB.F.DS", "F.DIST.RT"),
    ("INVF", "F.INV"),
    ("INV.F.DS", "F.INV.RT"),
    ("TESTF", "F.TEST"),
    ("DISTRIB.F", "FDIST"),
    ("INV.F", "FINV"),
    ("TEST.F", "FTEST"),
    ("TESTO.FORMULA", "FORMULATEXT"),
    ("DISTRIB.GAMMA.N", "GAMMA.DIST"),
    ("DISTRIB.GAMMA", "GAMMADIST"),
    ("INV.GAMMA.N", "GAMMA.INV"),
    ("INV.GAMMA", "GAMMAINV"),
    ("CRESCITA", "GROWTH"),
    ("INFO.DATI.TAB.PIVOT", "GETPIVOTDATA"),
    ("RAGGRUPPA.PER", "GROUPBY"),
    ("PIVOT.PER", "PIVOTBY"),
    ("DISTRIB.IPERGEOM.N", "HYPGEOM.DIST"),
    ("DISTRIB.IPERGEOM", "HYPGEOMDIST"),
    ("COMP.MODULO", "IMABS"),
    ("COMP.IMMAGINARIO", "IMAGINARY"),
    ("COMP.ARGOMENTO", "IMARGUMENT"),
    ("COMP.CONIUGATO", "IMCONJUGATE"),
    ("COMP.COS", "IMCOS"),
    ("COMP.COSH", "IMCOSH"),
    ("COMP.COT", "IMCOT"),
    ("COMP.CSC", "IMCSC"),
    ("COMP.CSCH", "IMCSCH"),
    ("COMP.DIV", "IMDIV"),
    ("COMP.EXP", "IMEXP"),
    ("COMP.LN", "IMLN"),
    ("COMP.LOG10", "IMLOG10"),
    ("COMP.LOG2", "IMLOG2"),
    ("COMP.POTENZA", "IMPOWER"),
    ("COMP.PRODOTTO", "IMPRODUCT"),
    ("COMP.PARTE.REALE", "IMREAL"),
    ("COMP.SEC", "IMSEC"),
    ("COMP.SECH", "IMSECH"),
    ("COMP.SEN", "IMSIN"),
    ("COMP.SENH", "IMSINH"),
    ("COMP.RADQ", "IMSQRT"),
    ("COMP.DIFF", "IMSUB"),
    ("COMP.SOMMA", "IMSUM"),
    ("COMP.TAN", "IMTAN"),
    ("INDIRETTO", "INDIRECT"),
    ("TASSO.INT", "INTRATE"),
    ("VAL.FORMULA", "ISFORMULA"),
    ("REGR.LIN", "LINEST"),
    ("REGR.LOG", "LOGEST"),
    ("INV.LOGNORM", "LOGINV"),
    ("DISTRIB.LOGNORM.N", "LOGNORM.DIST"),
    ("INV.LOGNORM.N", "LOGNORM.INV"),
    ("DISTRIB.LOGNORM", "LOGNORMDIST"),
    ("MATR.DETERM", "MDETERM"),
    ("DURATA.M", "MDURATION"),
    ("MATR.INVERSA", "MINVERSE"),
    ("MODA.MULT", "MODE.MULT"),
    ("DISTRIB.BINOM.NEG.N", "NEGBINOM.DIST"),
    ("DISTRIB.BINOM.NEG", "NEGBINOMDIST"),
    ("SCARTO", "OFFSET"),
    ("PREZZO", "PRICE"),
    ("PREZZO.SCONT", "PRICEDISC"),
    ("PREZZO.SCAD", "PRICEMAT"),
    ("PROBABILITÀ", "PROB"),
    ("CASUALE", "RAND"),
    ("MATR.CASUALE", "RANDARRAY"),
    ("CASUALE.TRA", "RANDBETWEEN"),
    ("RICEV.SCAD", "RECEIVED"),
    ("SOMMA.SERIE", "SERIESSUM"),
    ("FOGLIO", "SHEET"),
    ("FOGLI", "SHEETS"),
    ("ASIMMETRIA.P", "SKEW.P"),
    ("DISTRIB.T.N", "T.DIST"),
    ("DISTRIB.T.2T", "T.DIST.2T"),
    ("DISTRIB.T.DS", "T.DIST.RT"),
    ("INVT", "T.INV"),
    ("INV.T.2T", "T.INV.2T"),
    ("TESTT", "T.TEST"),
    ("BOT.EQUIV", "TBILLEQ"),
    ("BOT.PREZZO", "TBILLPRICE"),
    ("BOT.REND", "TBILLYIELD"),
    ("DISTRIB.T", "TDIST"),
    ("INV.T", "TINV"),
    ("TENDENZA", "TREND"),
    ("TEST.T", "TTEST"),
    ("VALORE.A.TESTO", "VALUETOTEXT"),
    ("AMMORT.VAR", "VDB"),
    ("DISTRIB.WEIBULL", "WEIBULL.DIST"),
    ("CONFRONTA.X", "XMATCH"),
    ("REND", "YIELD"),
    ("REND.TITOLI.SCONT", "YIELDDISC"),
    ("REND.SCAD", "YIELDMAT"),
    ("TESTZ", "Z.TEST"),
    ("TEST.Z", "ZTEST"),
    ("BAHTTESTO", "BAHTTEXT"),
    ("FURIGANA", "PHONETIC"),
    ("FILTRO.XML", "FILTERXML"),
    ("PREVISIONE.ETS", "FORECAST.ETS"),
    ("PREVISIONE.ETS.INTCONF", "FORECAST.ETS.CONFINT"),
    ("PREVISIONE.ETS.STAGIONALITÀ", "FORECAST.ETS.SEASONALITY"),
    ("PREVISIONE.ETS.STAT", "FORECAST.ETS.STAT"),
    ("PREZZO.PRIMO.IRR", "ODDFPRICE"),
    ("REND.PRIMO.IRR", "ODDFYIELD"),
    ("PREZZO.ULTIMO.IRR", "ODDLPRICE"),
    ("REND.ULTIMO.IRR", "ODDLYIELD"),
    ("AMBIENTE.INFO", "INFO"),
    ("SERVIZIO.WEB", "WEBSERVICE"),
];

/// Whether a formula calls a function that reaches outside the workbook.
pub(crate) fn calls_external(expression: &Expr) -> bool {
    match expression {
        Expr::Call(name, arguments) => {
            EXTERNAL_FUNCTIONS.contains(&name.as_str()) || arguments.iter().any(calls_external)
        }
        Expr::Unary(_, inner) | Expr::Percent(inner) | Expr::Group(inner) => calls_external(inner),
        Expr::Binary(_, left, right) => calls_external(left) || calls_external(right),
        Expr::Array(rows) => rows.iter().flatten().any(calls_external),
        Expr::Invoke(callee, arguments) => {
            calls_external(callee) || arguments.iter().any(calls_external)
        }
        Expr::Number(_)
        | Expr::Text(_)
        | Expr::Bool(_)
        | Expr::Error(_)
        | Expr::Cell(_)
        | Expr::Range(_)
        | Expr::Name(_) => false,
    }
}

/// The English name of a function typed in Italian or English, in capitals.
/// A name as a formula keeps it: as typed, without the `_xlpm.` files put before the names a
/// `LET` or a `LAMBDA` binds.
fn canonical_name(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    match lower.strip_prefix("_xlpm.") {
        Some(_) => name["_xlpm.".len()..].to_owned(),
        None => name.to_owned(),
    }
}

pub(crate) fn canonical_function_name(name: &str) -> String {
    let upper = name.to_uppercase();
    // Files store the functions newer than Excel 2007 with a prefix.
    let upper = upper
        .strip_prefix("_XLFN._XLWS.")
        .or_else(|| upper.strip_prefix("_XLFN."))
        .or_else(|| upper.strip_prefix("_XLWS."))
        .map_or_else(|| upper.clone(), str::to_owned);
    let english = ITALIAN_FUNCTION_NAMES
        .iter()
        .find(|(italian, _)| *italian == upper)
        .map_or(upper.clone(), |(_, english)| (*english).to_owned());
    if FUNCTION_NAMES.binary_search(&english.as_str()).is_ok()
        || EXTERNAL_FUNCTIONS.contains(&english.as_str())
    {
        return english;
    }
    // A function the grid does not know is a name a formula defines: kept as typed.
    let prefix = name.len() - upper.len();
    name[prefix..].to_owned()
}

/// The Italian name of a function, for a formula shown in Italian; the English one when Excel
/// gives it no other.
fn italian_function_name(english: &str) -> &str {
    ITALIAN_FUNCTION_NAMES
        .iter()
        .find(|(_, name)| *name == english)
        .map_or(english, |(italian, _)| italian)
}

/// The functions an xlsx stores with the `_xlfn._xlws.` prefix, the dynamic arrays Excel ships
/// as worksheet functions.
pub(crate) const WORKSHEET_PREFIXED_FUNCTIONS: &[&str] = &["FILTER", "SORT"];

/// The functions an xlsx stores with the `_xlfn.` prefix, as Excel 2010 and later write them.
pub(crate) const PREFIXED_FUNCTIONS: &[&str] = &[
    "LET",
    "LAMBDA",
    "MAP",
    "REDUCE",
    "SCAN",
    "BYROW",
    "BYCOL",
    "MAKEARRAY",
    "ISOMITTED",
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
    "XOR",
    "IFNA",
    "TEXTJOIN",
    "CONCAT",
    "XLOOKUP",
    "DAYS",
    "ISOWEEKNUM",
    "CEILING.PRECISE",
    "FLOOR.PRECISE",
    "COT",
    "CSC",
    "SEC",
    "COTH",
    "CSCH",
    "SECH",
    "ACOT",
    "ACOTH",
    "COMBINA",
    "PERMUTATIONA",
    "ARABIC",
    "BASE",
    "DECIMAL",
    "PERCENTILE.INC",
    "PERCENTILE.EXC",
    "QUARTILE.INC",
    "QUARTILE.EXC",
    "PERCENTRANK.INC",
    "PERCENTRANK.EXC",
    "RANK.AVG",
    "COVARIANCE.P",
    "COVARIANCE.S",
    "FORECAST.LINEAR",
    "NORM.DIST",
    "NORM.S.DIST",
    "NORM.INV",
    "NORM.S.INV",
    "CONFIDENCE.NORM",
    "EXPON.DIST",
    "POISSON.DIST",
    "BINOM.DIST",
    "GAMMALN.PRECISE",
    "GAMMA",
    "PHI",
    "PDURATION",
    "RRI",
    "WORKDAY.INTL",
    "NETWORKDAYS.INTL",
    "BITAND",
    "BITOR",
    "BITXOR",
    "BITLSHIFT",
    "BITRSHIFT",
    "UNICHAR",
    "UNICODE",
    "NUMBERVALUE",
    "TEXTBEFORE",
    "TEXTAFTER",
    "SORTBY",
    "UNIQUE",
    "SEQUENCE",
    "TAKE",
    "DROP",
    "CHOOSEROWS",
    "CHOOSECOLS",
    "VSTACK",
    "HSTACK",
    "TOCOL",
    "TOROW",
    "WRAPROWS",
    "WRAPCOLS",
    "EXPAND",
    "TEXTSPLIT",
    "MUNIT",
    "AGGREGATE",
    "BETA.DIST",
    "BETA.INV",
    "BINOM.DIST.RANGE",
    "BINOM.INV",
    "CHISQ.DIST",
    "CHISQ.DIST.RT",
    "CHISQ.INV",
    "CHISQ.INV.RT",
    "CHISQ.TEST",
    "CONFIDENCE.T",
    "ERF.PRECISE",
    "ERFC.PRECISE",
    "F.DIST",
    "F.DIST.RT",
    "F.INV",
    "F.INV.RT",
    "F.TEST",
    "GAMMA.DIST",
    "GAMMA.INV",
    "GAUSS",
    "HYPGEOM.DIST",
    "LOGNORM.DIST",
    "LOGNORM.INV",
    "MODE.MULT",
    "NEGBINOM.DIST",
    "T.DIST",
    "T.DIST.2T",
    "T.DIST.RT",
    "T.INV",
    "T.INV.2T",
    "T.TEST",
    "WEIBULL.DIST",
    "Z.TEST",
    "SKEW.P",
    "FORMULATEXT",
    "ISFORMULA",
    "SHEET",
    "SHEETS",
    "IMCOSH",
    "IMCOT",
    "IMCSC",
    "IMCSCH",
    "IMSEC",
    "IMSECH",
    "IMSINH",
    "IMTAN",
    "ENCODEURL",
    "XMATCH",
    "RANDARRAY",
    "VALUETOTEXT",
    "ARRAYTOTEXT",
    "REGEXTEST",
    "REGEXEXTRACT",
    "REGEXREPLACE",
    "TRIMRANGE",
    "PERCENTOF",
    "FILTERXML",
    "WEBSERVICE",
    "STOCKHISTORY",
    "IMAGE",
    "TRANSLATE",
    "FORECAST.ETS",
    "FORECAST.ETS.CONFINT",
    "FORECAST.ETS.SEASONALITY",
    "FORECAST.ETS.STAT",
    "GROUPBY",
    "PIVOTBY",
];

/// The functions that reach outside the workbook: a web service, market data, a picture by
/// address, a translation service, a real-time server or code. They are kept in a formula and
/// written back as they were, never called.
pub const EXTERNAL_FUNCTIONS: &[&str] = &[
    "CALL",
    "IMAGE",
    "REGISTER.ID",
    "RTD",
    "STOCKHISTORY",
    "TRANSLATE",
    "WEBSERVICE",
];

/// Every function the evaluator computes, by its English name.
pub const FUNCTION_NAMES: &[&str] = &[
    "ABS",
    "ACCRINT",
    "ACCRINTM",
    "ACOS",
    "ACOSH",
    "ACOT",
    "ACOTH",
    "ADDRESS",
    "AGGREGATE",
    "AMORDEGRC",
    "AMORLINC",
    "AND",
    "ARABIC",
    "AREAS",
    "ARRAYTOTEXT",
    "ASC",
    "ASIN",
    "ASINH",
    "ATAN",
    "ATAN2",
    "ATANH",
    "AVEDEV",
    "AVERAGE",
    "AVERAGEA",
    "AVERAGEIF",
    "AVERAGEIFS",
    "BAHTTEXT",
    "BASE",
    "BESSELI",
    "BESSELJ",
    "BESSELK",
    "BESSELY",
    "BETA.DIST",
    "BETA.INV",
    "BETADIST",
    "BETAINV",
    "BIN2DEC",
    "BIN2HEX",
    "BIN2OCT",
    "BINOM.DIST",
    "BINOM.DIST.RANGE",
    "BINOM.INV",
    "BINOMDIST",
    "BITAND",
    "BITLSHIFT",
    "BITOR",
    "BITRSHIFT",
    "BITXOR",
    "BYCOL",
    "BYROW",
    "CEILING",
    "CEILING.MATH",
    "CEILING.PRECISE",
    "CELL",
    "CHAR",
    "CHIDIST",
    "CHIINV",
    "CHISQ.DIST",
    "CHISQ.DIST.RT",
    "CHISQ.INV",
    "CHISQ.INV.RT",
    "CHISQ.TEST",
    "CHITEST",
    "CHOOSE",
    "CHOOSECOLS",
    "CHOOSEROWS",
    "CLEAN",
    "CODE",
    "COLUMN",
    "COLUMNS",
    "COMBIN",
    "COMBINA",
    "COMPLEX",
    "CONCAT",
    "CONCATENATE",
    "CONFIDENCE",
    "CONFIDENCE.NORM",
    "CONFIDENCE.T",
    "CONVERT",
    "CORREL",
    "COS",
    "COSH",
    "COT",
    "COTH",
    "COUNT",
    "COUNTA",
    "COUNTBLANK",
    "COUNTIF",
    "COUNTIFS",
    "COUPDAYBS",
    "COUPDAYS",
    "COUPDAYSNC",
    "COUPNCD",
    "COUPNUM",
    "COUPPCD",
    "COVAR",
    "COVARIANCE.P",
    "COVARIANCE.S",
    "CRITBINOM",
    "CSC",
    "CSCH",
    "CUMIPMT",
    "CUMPRINC",
    "DATE",
    "DATEDIF",
    "DATEVALUE",
    "DAVERAGE",
    "DAY",
    "DAYS",
    "DAYS360",
    "DB",
    "DBCS",
    "DCOUNT",
    "DCOUNTA",
    "DDB",
    "DEC2BIN",
    "DEC2HEX",
    "DEC2OCT",
    "DECIMAL",
    "DEGREES",
    "DELTA",
    "DEVSQ",
    "DGET",
    "DISC",
    "DMAX",
    "DMIN",
    "DOLLAR",
    "DOLLARDE",
    "DOLLARFR",
    "DPRODUCT",
    "DROP",
    "DSTDEV",
    "DSTDEVP",
    "DSUM",
    "DURATION",
    "DVAR",
    "DVARP",
    "EDATE",
    "EFFECT",
    "ENCODEURL",
    "EOMONTH",
    "ERF",
    "ERF.PRECISE",
    "ERFC",
    "ERFC.PRECISE",
    "ERROR.TYPE",
    "EUROCONVERT",
    "EVEN",
    "EXACT",
    "EXP",
    "EXPAND",
    "EXPON.DIST",
    "EXPONDIST",
    "F.DIST",
    "F.DIST.RT",
    "F.INV",
    "F.INV.RT",
    "F.TEST",
    "FACT",
    "FACTDOUBLE",
    "FALSE",
    "FDIST",
    "FILTER",
    "FILTERXML",
    "FIND",
    "FINDB",
    "FINV",
    "FISHER",
    "FISHERINV",
    "FIXED",
    "FLOOR",
    "FLOOR.MATH",
    "FLOOR.PRECISE",
    "FORECAST",
    "FORECAST.ETS",
    "FORECAST.ETS.CONFINT",
    "FORECAST.ETS.SEASONALITY",
    "FORECAST.ETS.STAT",
    "FORECAST.LINEAR",
    "FORMULATEXT",
    "FREQUENCY",
    "FTEST",
    "FV",
    "FVSCHEDULE",
    "GAMMA",
    "GAMMA.DIST",
    "GAMMA.INV",
    "GAMMADIST",
    "GAMMAINV",
    "GAMMALN",
    "GAMMALN.PRECISE",
    "GAUSS",
    "GCD",
    "GEOMEAN",
    "GESTEP",
    "GETPIVOTDATA",
    "GROUPBY",
    "GROWTH",
    "HARMEAN",
    "HEX2BIN",
    "HEX2DEC",
    "HEX2OCT",
    "HLOOKUP",
    "HOUR",
    "HSTACK",
    "HYPERLINK",
    "HYPGEOM.DIST",
    "HYPGEOMDIST",
    "IF",
    "IFERROR",
    "IFNA",
    "IFS",
    "IMABS",
    "IMAGINARY",
    "IMARGUMENT",
    "IMCONJUGATE",
    "IMCOS",
    "IMCOSH",
    "IMCOT",
    "IMCSC",
    "IMCSCH",
    "IMDIV",
    "IMEXP",
    "IMLN",
    "IMLOG10",
    "IMLOG2",
    "IMPOWER",
    "IMPRODUCT",
    "IMREAL",
    "IMSEC",
    "IMSECH",
    "IMSIN",
    "IMSINH",
    "IMSQRT",
    "IMSUB",
    "IMSUM",
    "IMTAN",
    "INDEX",
    "INDIRECT",
    "INFO",
    "INT",
    "INTERCEPT",
    "INTRATE",
    "IPMT",
    "IRR",
    "ISBLANK",
    "ISERR",
    "ISERROR",
    "ISEVEN",
    "ISFORMULA",
    "ISLOGICAL",
    "ISNA",
    "ISNONTEXT",
    "ISNUMBER",
    "ISO.CEILING",
    "ISODD",
    "ISOMITTED",
    "ISOWEEKNUM",
    "ISPMT",
    "ISREF",
    "ISTEXT",
    "KURT",
    "LAMBDA",
    "LARGE",
    "LCM",
    "LEFT",
    "LEFTB",
    "LEN",
    "LENB",
    "LET",
    "LINEST",
    "LN",
    "LOG",
    "LOG10",
    "LOGEST",
    "LOGINV",
    "LOGNORM.DIST",
    "LOGNORM.INV",
    "LOGNORMDIST",
    "LOOKUP",
    "LOWER",
    "MAKEARRAY",
    "MAP",
    "MATCH",
    "MAX",
    "MAXA",
    "MAXIFS",
    "MDETERM",
    "MDURATION",
    "MEDIAN",
    "MID",
    "MIDB",
    "MIN",
    "MINA",
    "MINIFS",
    "MINUTE",
    "MINVERSE",
    "MIRR",
    "MMULT",
    "MOD",
    "MODE",
    "MODE.MULT",
    "MODE.SNGL",
    "MONTH",
    "MROUND",
    "MULTINOMIAL",
    "MUNIT",
    "N",
    "NA",
    "NEGBINOM.DIST",
    "NEGBINOMDIST",
    "NETWORKDAYS",
    "NETWORKDAYS.INTL",
    "NOMINAL",
    "NORM.DIST",
    "NORM.INV",
    "NORM.S.DIST",
    "NORM.S.INV",
    "NORMDIST",
    "NORMINV",
    "NORMSDIST",
    "NORMSINV",
    "NOT",
    "NOW",
    "NPER",
    "NPV",
    "NUMBERVALUE",
    "OCT2BIN",
    "OCT2DEC",
    "OCT2HEX",
    "ODD",
    "ODDFPRICE",
    "ODDFYIELD",
    "ODDLPRICE",
    "ODDLYIELD",
    "OFFSET",
    "OR",
    "PDURATION",
    "PEARSON",
    "PERCENTILE",
    "PERCENTILE.EXC",
    "PERCENTILE.INC",
    "PERCENTOF",
    "PERCENTRANK",
    "PERCENTRANK.EXC",
    "PERCENTRANK.INC",
    "PERMUT",
    "PERMUTATIONA",
    "PHI",
    "PHONETIC",
    "PI",
    "PIVOTBY",
    "PMT",
    "POISSON",
    "POISSON.DIST",
    "POWER",
    "PPMT",
    "PRICE",
    "PRICEDISC",
    "PRICEMAT",
    "PROB",
    "PRODUCT",
    "PROPER",
    "PV",
    "QUARTILE",
    "QUARTILE.EXC",
    "QUARTILE.INC",
    "QUOTIENT",
    "RADIANS",
    "RAND",
    "RANDARRAY",
    "RANDBETWEEN",
    "RANK",
    "RANK.AVG",
    "RANK.EQ",
    "RATE",
    "RECEIVED",
    "REDUCE",
    "REGEXEXTRACT",
    "REGEXREPLACE",
    "REGEXTEST",
    "REPLACE",
    "REPLACEB",
    "REPT",
    "RIGHT",
    "RIGHTB",
    "ROMAN",
    "ROUND",
    "ROUNDDOWN",
    "ROUNDUP",
    "ROW",
    "ROWS",
    "RRI",
    "RSQ",
    "SCAN",
    "SEARCH",
    "SEARCHB",
    "SEC",
    "SECH",
    "SECOND",
    "SEQUENCE",
    "SERIESSUM",
    "SHEET",
    "SHEETS",
    "SIGN",
    "SIN",
    "SINH",
    "SKEW",
    "SKEW.P",
    "SLN",
    "SLOPE",
    "SMALL",
    "SORT",
    "SORTBY",
    "SQRT",
    "SQRTPI",
    "STANDARDIZE",
    "STDEV",
    "STDEV.P",
    "STDEV.S",
    "STDEVA",
    "STDEVP",
    "STDEVPA",
    "STEYX",
    "SUBSTITUTE",
    "SUBTOTAL",
    "SUM",
    "SUMIF",
    "SUMIFS",
    "SUMPRODUCT",
    "SUMSQ",
    "SUMX2MY2",
    "SUMX2PY2",
    "SUMXMY2",
    "SWITCH",
    "SYD",
    "T",
    "T.DIST",
    "T.DIST.2T",
    "T.DIST.RT",
    "T.INV",
    "T.INV.2T",
    "T.TEST",
    "TAKE",
    "TAN",
    "TANH",
    "TBILLEQ",
    "TBILLPRICE",
    "TBILLYIELD",
    "TDIST",
    "TEXT",
    "TEXTAFTER",
    "TEXTBEFORE",
    "TEXTJOIN",
    "TEXTSPLIT",
    "TIME",
    "TIMEVALUE",
    "TINV",
    "TOCOL",
    "TODAY",
    "TOROW",
    "TRANSPOSE",
    "TREND",
    "TRIM",
    "TRIMMEAN",
    "TRIMRANGE",
    "TRUE",
    "TRUNC",
    "TTEST",
    "TYPE",
    "UNICHAR",
    "UNICODE",
    "UNIQUE",
    "UPPER",
    "VALUE",
    "VALUETOTEXT",
    "VAR",
    "VAR.P",
    "VAR.S",
    "VARA",
    "VARP",
    "VARPA",
    "VDB",
    "VLOOKUP",
    "VSTACK",
    "WEEKDAY",
    "WEEKNUM",
    "WEIBULL",
    "WEIBULL.DIST",
    "WORKDAY",
    "WORKDAY.INTL",
    "WRAPCOLS",
    "WRAPROWS",
    "XIRR",
    "XLOOKUP",
    "XMATCH",
    "XNPV",
    "XOR",
    "YEAR",
    "YEARFRAC",
    "YIELD",
    "YIELDDISC",
    "YIELDMAT",
    "Z.TEST",
    "ZTEST",
];

/// The names of the functions a formula can call, as the locale writes them, in alphabetical
/// order: what an editor offers while a formula is typed.
#[must_use]
pub fn function_names(locale: Locale) -> Vec<&'static str> {
    let mut names: Vec<&'static str> = FUNCTION_NAMES
        .iter()
        .map(|name| match locale {
            Locale::Italian => italian_function_name(name),
            Locale::English => name,
        })
        .collect();
    names.sort_unstable();
    names
}

/// A formula printed back in file syntax, without the leading `=`.
#[must_use]
pub fn print_formula(expression: &Expr) -> String {
    let mut text = String::new();
    print(expression, &mut text, 0, Locale::English);
    text
}

/// A formula as a person reads it in a locale, without the leading `=`: in Italian the Italian
/// names of the functions, `;` between arguments and `,` in numbers. [`parse_formula_in`] reads
/// it back to the same formula.
#[must_use]
pub fn print_formula_in(expression: &Expr, locale: Locale) -> String {
    let mut text = String::new();
    print(expression, &mut text, 0, locale);
    text
}

fn print(expression: &Expr, text: &mut String, parent_precedence: u8, locale: Locale) {
    let italian = locale == Locale::Italian;
    match expression {
        Expr::Number(value) if italian => text.push_str(&number_text(*value).replace('.', ",")),
        Expr::Number(value) => text.push_str(&number_text(*value)),
        Expr::Text(value) => {
            let _ = write!(text, "\"{}\"", value.replace('"', "\"\""));
        }
        Expr::Bool(value) => text.push_str(match (italian, *value) {
            (true, true) => "VERO",
            (true, false) => "FALSO",
            (false, true) => "TRUE",
            (false, false) => "FALSE",
        }),
        Expr::Error(kind) => text.push_str(kind.code()),
        Expr::Cell(address) => {
            print_sheet(address.sheet.as_deref(), text);
            print_cell(address, text);
        }
        Expr::Range(range) => {
            print_sheet(range.sheet.as_deref(), text);
            if range.whole_columns {
                let column = |address: &CellAddress, text: &mut String| {
                    if address.absolute_column {
                        text.push('$');
                    }
                    text.push_str(&column_name(address.cell.column));
                };
                column(&range.start, text);
                text.push(':');
                column(&range.end, text);
            } else {
                print_cell(&range.start, text);
                text.push(':');
                print_cell(&range.end, text);
            }
        }
        Expr::Unary(operator, operand) => {
            text.push(match operator {
                UnaryOperator::Plus => '+',
                UnaryOperator::Minus => '-',
            });
            print(operand, text, 6, locale);
        }
        Expr::Percent(operand) => {
            print(operand, text, 7, locale);
            text.push('%');
        }
        Expr::Group(inner) => {
            text.push('(');
            print(inner, text, 0, locale);
            text.push(')');
        }
        Expr::Array(rows) => {
            let (column, row) = if italian { (';', '|') } else { (',', ';') };
            text.push('{');
            for (index, line) in rows.iter().enumerate() {
                if index > 0 {
                    text.push(row);
                }
                for (position, element) in line.iter().enumerate() {
                    if position > 0 {
                        text.push(column);
                    }
                    print(element, text, 0, locale);
                }
            }
            text.push('}');
        }
        Expr::Binary(operator, left, right) => {
            let precedence = operator.precedence();
            let parenthesize = precedence < parent_precedence;
            if parenthesize {
                text.push('(');
            }
            print(left, text, precedence, locale);
            text.push_str(operator.symbol());
            // Operators are left-associative: a right operand of the same level needs brackets.
            print(right, text, precedence + 1, locale);
            if parenthesize {
                text.push(')');
            }
        }
        Expr::Call(name, arguments) => {
            text.push_str(if italian {
                italian_function_name(name)
            } else {
                name
            });
            print_arguments(arguments, text, locale);
        }
        Expr::Name(name) => text.push_str(name),
        Expr::Invoke(callee, arguments) => {
            print(callee, text, u8::MAX, locale);
            print_arguments(arguments, text, locale);
        }
    }
}

fn print_arguments(arguments: &[Expr], text: &mut String, locale: Locale) {
    let italian = locale == Locale::Italian;
    text.push('(');
    for (index, argument) in arguments.iter().enumerate() {
        if index > 0 {
            text.push(if italian { ';' } else { ',' });
        }
        if !matches!(argument, Expr::Text(value) if value.is_empty()) {
            print(argument, text, 0, locale);
        }
    }
    text.push(')');
}

fn print_sheet(sheet: Option<&str>, text: &mut String) {
    let Some(sheet) = sheet else {
        return;
    };
    let plain = sheet
        .chars()
        .all(|character| character.is_alphanumeric() || character == '_')
        && sheet.chars().next().is_some_and(char::is_alphabetic)
        && parse_cell(sheet.as_bytes()).is_none();
    if plain {
        text.push_str(sheet);
    } else {
        let _ = write!(text, "'{}'", sheet.replace('\'', "''"));
    }
    text.push('!');
}

fn print_cell(address: &CellAddress, text: &mut String) {
    if address.absolute_column {
        text.push('$');
    }
    text.push_str(&column_name(address.cell.column));
    if address.absolute_row {
        text.push('$');
    }
    let _ = write!(text, "{}", address.cell.row + 1);
}

/// A number as a formula writes it: the shortest text that reads back to the same value.
#[must_use]
pub fn number_text(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 1e15 {
        #[allow(clippy::cast_possible_truncation)]
        let integer = value as i64;
        return integer.to_string();
    }
    let text = format!("{value}");
    if text.contains('e') || text.len() < 22 {
        text
    } else {
        format!("{value:e}")
    }
}

#[cfg(test)]
mod tests;
