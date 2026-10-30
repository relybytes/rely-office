// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! LaTeX to the formula tree and back. The parser reads what an editor's formula field writes
//! and what [`to_latex`] writes, so a formula read from a file survives an edit: an unknown
//! command is kept as text rather than refused, and nothing fails on a malformed formula.

use std::fmt::Write as _;

use super::{
    Accent, BIG_OPERATORS, FUNCTIONS, MAX_DEPTH, MAX_NODES, MathNode, atom_of, command_of, row,
    symbol_of,
};

/// The functions whose limits are set under them in a display formula.
const LIMIT_FUNCTIONS: [&str; 8] = ["lim", "liminf", "limsup", "max", "min", "sup", "inf", "det"];

/// Read a formula written in LaTeX.
pub(crate) fn parse_latex(latex: &str) -> MathNode {
    let mut parser = Parser {
        characters: latex.chars().collect(),
        position: 0,
        nodes: 0,
    };
    row(parser.expression(Stop::End, 0))
}

/// Where an expression stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    End,
    /// The `}` that closes a group.
    Group,
    /// The `]` that closes the index of a root.
    Bracket,
    /// `\right`.
    Right,
    /// A cell of a matrix, ended by `&`, `\\` or `\end`.
    Cell,
}

struct Parser {
    characters: Vec<char>,
    position: usize,
    nodes: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.characters.get(self.position).copied()
    }

    fn skip_spaces(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.position += 1;
        }
    }

    /// The name of the command at the parser, its backslash not included, without moving.
    fn command_ahead(&self) -> Option<String> {
        if self.peek() != Some('\\') {
            return None;
        }
        let mut name = String::new();
        let mut index = self.position + 1;
        while let Some(character) = self.characters.get(index) {
            if character.is_ascii_alphabetic() {
                name.push(*character);
                index += 1;
            } else {
                if name.is_empty() {
                    name.push(*character);
                }
                break;
            }
        }
        Some(name)
    }

    /// Read a command's name past its backslash; a command of letters swallows the spaces
    /// after it.
    fn command(&mut self) -> String {
        self.position += 1;
        let mut name = String::new();
        while let Some(character) = self.peek() {
            if character.is_ascii_alphabetic() {
                name.push(character);
                self.position += 1;
            } else {
                if name.is_empty() {
                    name.push(character);
                    self.position += 1;
                    return name;
                }
                break;
            }
        }
        self.skip_spaces();
        name
    }

    fn expression(&mut self, stop: Stop, depth: usize) -> Vec<MathNode> {
        let mut nodes = Vec::new();
        loop {
            self.skip_spaces();
            let Some(character) = self.peek() else {
                return nodes;
            };
            if self.nodes >= MAX_NODES {
                self.position = self.characters.len();
                return nodes;
            }
            match (character, stop) {
                ('}', Stop::Group) | (']', Stop::Bracket) => {
                    self.position += 1;
                    return nodes;
                }
                ('}', _) => {
                    self.position += 1;
                    continue;
                }
                ('&', Stop::Cell) => return nodes,
                _ => {}
            }
            if character == '\\' {
                match self.command_ahead().as_deref() {
                    Some("right") if stop == Stop::Right => return nodes,
                    Some("\\" | "end") if stop == Stop::Cell => return nodes,
                    Some("right" | "end") => {
                        self.command();
                        continue;
                    }
                    _ => {}
                }
            }
            let Some(atom) = self.atom(depth) else {
                continue;
            };
            let atom = self.scripts(atom, depth);
            let atom = self.operand(atom, depth);
            self.nodes += 1;
            nodes.push(atom);
        }
    }

    /// One atom: a group, a number, a letter, a command and its arguments, or an operator.
    fn atom(&mut self, depth: usize) -> Option<MathNode> {
        if depth > MAX_DEPTH {
            self.position = self.characters.len();
            return None;
        }
        let character = self.peek()?;
        match character {
            '{' => {
                self.position += 1;
                Some(row(self.expression(Stop::Group, depth + 1)))
            }
            '0'..='9' | '.' => {
                let mut number = String::new();
                while let Some(digit) = self
                    .peek()
                    .filter(|digit| digit.is_ascii_digit() || *digit == '.')
                {
                    number.push(digit);
                    self.position += 1;
                }
                Some(MathNode::Number(number))
            }
            '\\' => {
                let name = self.command();
                self.command_node(&name, depth)
            }
            '~' => {
                self.position += 1;
                Some(MathNode::Space(6))
            }
            '_' | '^' => {
                // A script with no base attaches to nothing.
                self.position += 1;
                let script = self.argument(depth);
                Some(MathNode::Script {
                    base: Box::new(MathNode::Row(Vec::new())),
                    sub: (character == '_').then(|| Box::new(script.clone())),
                    sup: (character == '^').then(|| Box::new(script)),
                })
            }
            _ => {
                self.position += 1;
                Some(atom_of(character))
            }
        }
    }

    /// A function or a big operator takes the atom after it as what it applies to, unless that
    /// atom is an operator or the expression ends.
    fn operand(&mut self, node: MathNode, depth: usize) -> MathNode {
        let empty = MathNode::Row(Vec::new());
        let open = match &node {
            MathNode::Nary { body, .. } => **body == empty,
            MathNode::Function { argument, .. } => **argument == empty,
            MathNode::Limits { base, .. } | MathNode::Script { base, .. } => {
                matches!(base.as_ref(), MathNode::Function { argument, .. } if **argument == empty)
            }
            _ => false,
        };
        if !open {
            return node;
        }
        self.skip_spaces();
        if matches!(self.peek(), None | Some('}' | ']' | '&'))
            || matches!(
                self.command_ahead().as_deref(),
                Some("right" | "end" | "\\")
            )
        {
            return node;
        }
        let start = self.position;
        let Some(next) = self.atom(depth + 1) else {
            self.position = start;
            return node;
        };
        if matches!(
            next,
            MathNode::Operator(_) | MathNode::Space(_) | MathNode::Text(_)
        ) {
            self.position = start;
            return node;
        }
        let next = Box::new(self.scripts(next, depth));
        match node {
            MathNode::Nary {
                operator,
                lower,
                upper,
                ..
            } => MathNode::Nary {
                operator,
                lower,
                upper,
                body: next,
            },
            MathNode::Function { name, .. } => MathNode::Function {
                name,
                argument: next,
            },
            MathNode::Limits { base, under, over } => match *base {
                MathNode::Function { name, .. } => MathNode::Limits {
                    base: Box::new(MathNode::Function {
                        name,
                        argument: next,
                    }),
                    under,
                    over,
                },
                other => MathNode::Limits {
                    base: Box::new(other),
                    under,
                    over,
                },
            },
            MathNode::Script { base, sub, sup } => match *base {
                MathNode::Function { name, .. } => MathNode::Script {
                    base: Box::new(MathNode::Function {
                        name,
                        argument: next,
                    }),
                    sub,
                    sup,
                },
                other => MathNode::Script {
                    base: Box::new(other),
                    sub,
                    sup,
                },
            },
            other => other,
        }
    }

    /// The argument of a command or a script: a group, or the next atom, of which a digit is
    /// one character, as `\frac12` is a half.
    fn argument(&mut self, depth: usize) -> MathNode {
        self.skip_spaces();
        if let Some(digit) = self.peek().filter(char::is_ascii_digit) {
            self.position += 1;
            return MathNode::Number(digit.to_string());
        }
        self.atom(depth + 1)
            .unwrap_or_else(|| MathNode::Row(Vec::new()))
    }

    /// The scripts after an atom, and the primes that are superscripts.
    fn scripts(&mut self, base: MathNode, depth: usize) -> MathNode {
        let mut sub = None;
        let mut sup: Option<MathNode> = None;
        loop {
            self.skip_spaces();
            match self.peek() {
                Some('_') if sub.is_none() => {
                    self.position += 1;
                    sub = Some(self.argument(depth));
                }
                Some('^') if sup.is_none() => {
                    self.position += 1;
                    sup = Some(self.argument(depth));
                }
                Some('\'') => {
                    self.position += 1;
                    let prime = MathNode::Operator('\u{2032}'.to_string());
                    sup = Some(match sup {
                        Some(MathNode::Row(mut items)) => {
                            items.push(prime);
                            MathNode::Row(items)
                        }
                        Some(other) => MathNode::Row(vec![other, prime]),
                        None => prime,
                    });
                }
                _ => break,
            }
        }
        if sub.is_none() && sup.is_none() {
            return base;
        }
        match base {
            MathNode::Nary {
                operator,
                lower: None,
                upper: None,
                body,
            } => MathNode::Nary {
                operator,
                lower: sub.map(Box::new),
                upper: sup.map(Box::new),
                body,
            },
            MathNode::Function { name, argument }
                if LIMIT_FUNCTIONS.contains(&name.as_str()) && sup.is_none() =>
            {
                MathNode::Limits {
                    base: Box::new(MathNode::Function { name, argument }),
                    under: sub.map(Box::new),
                    over: None,
                }
            }
            base => MathNode::Script {
                base: Box::new(base),
                sub: sub.map(Box::new),
                sup: sup.map(Box::new),
            },
        }
    }

    /// The text of a group read as it is, for `\text`: braces nest, a backslash escapes.
    fn raw_group(&mut self) -> String {
        self.skip_spaces();
        if self.peek() != Some('{') {
            return self
                .peek()
                .map(|character| {
                    self.position += 1;
                    character.to_string()
                })
                .unwrap_or_default();
        }
        self.position += 1;
        let mut depth = 1;
        let mut text = String::new();
        while let Some(character) = self.peek() {
            self.position += 1;
            match character {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                '\\' => {
                    if let Some(next) = self.peek() {
                        self.position += 1;
                        text.push(next);
                    }
                    continue;
                }
                _ => {}
            }
            text.push(character);
        }
        text
    }

    /// A delimiter after `\left` or `\right`: a character, a command naming one, or `.` for
    /// none.
    fn delimiter(&mut self) -> String {
        self.skip_spaces();
        match self.peek() {
            Some('\\') => {
                let name = self.command();
                symbol_of(&name)
                    .map(|symbol| symbol.to_string())
                    .unwrap_or_default()
            }
            Some('.') => {
                self.position += 1;
                String::new()
            }
            Some(character) => {
                self.position += 1;
                character.to_string()
            }
            None => String::new(),
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one match over the commands a formula is written with"
    )]
    fn command_node(&mut self, name: &str, depth: usize) -> Option<MathNode> {
        let accent = |accent| Some(accent);
        let accent = match name {
            "hat" | "widehat" => accent(Accent::Hat),
            "bar" => accent(Accent::Bar),
            "overline" => accent(Accent::Overline),
            "underline" => accent(Accent::Underline),
            "dot" => accent(Accent::Dot),
            "ddot" => accent(Accent::DoubleDot),
            "tilde" | "widetilde" => accent(Accent::Tilde),
            "vec" | "overrightarrow" => accent(Accent::Vector),
            _ => None,
        };
        if let Some(accent) = accent {
            return Some(MathNode::Accent {
                accent,
                base: Box::new(self.argument(depth)),
            });
        }
        if let Some((operator, _)) = BIG_OPERATORS.iter().find(|(_, command)| *command == name) {
            return Some(MathNode::Nary {
                operator: *operator,
                lower: None,
                upper: None,
                body: Box::new(MathNode::Row(Vec::new())),
            });
        }
        if FUNCTIONS.contains(&name) {
            return Some(MathNode::Function {
                name: name.to_owned(),
                argument: Box::new(MathNode::Row(Vec::new())),
            });
        }
        Some(match name {
            "frac" | "dfrac" | "tfrac" | "cfrac" => {
                let numerator = self.argument(depth);
                let denominator = self.argument(depth);
                MathNode::Fraction {
                    numerator: Box::new(numerator),
                    denominator: Box::new(denominator),
                    linear: false,
                }
            }
            "sqrt" => {
                self.skip_spaces();
                let index = (self.peek() == Some('[')).then(|| {
                    self.position += 1;
                    row(self.expression(Stop::Bracket, depth + 1))
                });
                MathNode::Root {
                    index: index.map(Box::new),
                    radicand: Box::new(self.argument(depth)),
                }
            }
            "left" => {
                let open = self.delimiter();
                let body = row(self.expression(Stop::Right, depth + 1));
                let close = if self.command_ahead().as_deref() == Some("right") {
                    self.command();
                    self.delimiter()
                } else {
                    String::new()
                };
                MathNode::Fenced {
                    open,
                    close,
                    body: Box::new(body),
                }
            }
            "begin" => self.environment(depth),
            "text" | "textrm" | "mbox" | "textit" | "textbf" | "mathrm" => {
                MathNode::Text(self.raw_group())
            }
            "operatorname" => MathNode::Function {
                name: self.raw_group(),
                argument: Box::new(MathNode::Row(Vec::new())),
            },
            "mathbb" => {
                let letters = self.raw_group();
                MathNode::Identifier(letters.chars().map(double_struck).collect())
            }
            "mathbf" | "mathit" | "boldsymbol" | "mathcal" | "mathsf" | "mathtt"
            | "displaystyle" | "textstyle" => {
                return self.atom(depth + 1).or(Some(MathNode::Row(Vec::new())));
            }
            "overset" | "stackrel" | "underset" => {
                let mark = self.argument(depth);
                let base = self.argument(depth);
                let (under, over) = if name == "underset" {
                    (Some(Box::new(mark)), None)
                } else {
                    (None, Some(Box::new(mark)))
                };
                MathNode::Limits {
                    base: Box::new(base),
                    under,
                    over,
                }
            }
            "," => MathNode::Space(3),
            ":" | ">" => MathNode::Space(4),
            ";" => MathNode::Space(5),
            " " => MathNode::Space(6),
            "quad" => MathNode::Space(18),
            "qquad" => MathNode::Space(36),
            "!" => return None,
            "\\" => MathNode::Operator(String::new()),
            other => match symbol_of(other) {
                Some(symbol) => atom_of(symbol),
                None => MathNode::Text(format!("\\{other}")),
            },
        })
    }

    /// `\begin{...}`: a matrix of the environment's delimiters, its cells parted by `&` and its
    /// rows by `\\`, up to `\end`.
    fn environment(&mut self, depth: usize) -> MathNode {
        let kind = self.raw_group();
        let (open, close) = match kind.trim_end_matches('*') {
            "pmatrix" => ("(", ")"),
            "bmatrix" => ("[", "]"),
            "Bmatrix" => ("{", "}"),
            "vmatrix" => ("|", "|"),
            "Vmatrix" => ("\u{2016}", "\u{2016}"),
            "cases" => ("{", ""),
            _ => ("", ""),
        };
        let mut rows: Vec<Vec<MathNode>> = vec![Vec::new()];
        loop {
            let cell = row(self.expression(Stop::Cell, depth + 1));
            if let Some(last) = rows.last_mut() {
                last.push(cell);
            }
            self.skip_spaces();
            match self.peek() {
                Some('&') => self.position += 1,
                Some('\\') => {
                    let name = self.command();
                    if name == "end" {
                        self.raw_group();
                        break;
                    }
                    rows.push(Vec::new());
                }
                _ => break,
            }
            if rows.len() > 1_000 {
                break;
            }
        }
        if rows
            .last()
            .is_some_and(|last| last.iter().all(|cell| *cell == MathNode::Row(Vec::new())))
            && rows.len() > 1
        {
            rows.pop();
        }
        MathNode::Matrix {
            rows,
            open: open.to_owned(),
            close: close.to_owned(),
        }
    }
}

/// The double-struck capital of `\mathbb`: `R` as `ℝ`.
fn double_struck(letter: char) -> char {
    match letter {
        'C' => '\u{2102}',
        'H' => '\u{210D}',
        'N' => '\u{2115}',
        'P' => '\u{2119}',
        'Q' => '\u{211A}',
        'R' => '\u{211D}',
        'Z' => '\u{2124}',
        other => other,
    }
}

/// A formula written in LaTeX.
pub(crate) fn to_latex(node: &MathNode) -> String {
    let mut latex = String::new();
    write_node(node, &mut latex, 0);
    latex.trim().to_owned()
}

/// Append a piece, with a space after a command of letters a letter would otherwise extend.
fn append(latex: &mut String, piece: &str) {
    let ends_in_command = latex.rfind('\\').is_some_and(|start| {
        latex.len() > start + 1
            && latex[start + 1..]
                .chars()
                .all(|character| character.is_ascii_alphabetic())
    });
    if ends_in_command && piece.starts_with(|character: char| character.is_ascii_alphanumeric()) {
        latex.push(' ');
    }
    latex.push_str(piece);
}

/// A node as a LaTeX argument: braced, so it stays one argument.
fn group(node: &MathNode, depth: usize) -> String {
    let mut inner = String::new();
    write_node(node, &mut inner, depth + 1);
    format!("{{{inner}}}")
}

/// A node as a base or a script: one character as it is, anything longer braced.
fn script_part(node: &MathNode, depth: usize) -> String {
    let mut inner = String::new();
    write_node(node, &mut inner, depth + 1);
    if inner.chars().count() == 1
        || (inner.starts_with('\\')
            && inner[1..]
                .chars()
                .all(|character| character.is_ascii_alphabetic()))
    {
        inner
    } else {
        format!("{{{inner}}}")
    }
}

fn delimiter(delimiter: &str) -> String {
    match delimiter {
        "" => ".".to_owned(),
        "{" => "\\{".to_owned(),
        "}" => "\\}".to_owned(),
        "\u{2016}" => "\\|".to_owned(),
        other => other
            .chars()
            .next()
            .and_then(|character| match character {
                '\u{27E8}' => Some("\\langle"),
                '\u{27E9}' => Some("\\rangle"),
                '\u{230A}' => Some("\\lfloor"),
                '\u{230B}' => Some("\\rfloor"),
                '\u{2308}' => Some("\\lceil"),
                '\u{2309}' => Some("\\rceil"),
                _ => None,
            })
            .map_or_else(|| other.to_owned(), str::to_owned),
    }
}

/// An operator or a letter: its command, or the character, escaped when LaTeX reserves it.
fn symbol(text: &str) -> String {
    let mut written = String::new();
    for character in text.chars() {
        if let Some(command) = command_of(character) {
            let _ = write!(written, "\\{command}");
            continue;
        }
        match character {
            '{' | '}' | '#' | '%' | '&' | '$' | '_' => {
                written.push('\\');
                written.push(character);
            }
            '\\' => written.push_str("\\backslash"),
            '^' => written.push_str("\\hat{}"),
            '~' => written.push_str("\\sim"),
            '\u{2212}' => written.push('-'),
            other => written.push(other),
        }
    }
    written
}

/// The command of a function: its own, or `\operatorname`.
fn function_head(name: &str) -> String {
    if FUNCTIONS.contains(&name) {
        format!("\\{name}")
    } else {
        format!("\\operatorname{{{}}}", escape_text(name))
    }
}

fn escape_text(text: &str) -> String {
    let mut escaped = String::new();
    for character in text.chars() {
        if matches!(character, '{' | '}' | '\\' | '#' | '%' | '&' | '$' | '_') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

#[expect(
    clippy::too_many_lines,
    reason = "one match over the kinds of node a formula holds"
)]
fn write_node(node: &MathNode, latex: &mut String, depth: usize) {
    if depth > MAX_DEPTH * 2 {
        return;
    }
    match node {
        MathNode::Row(nodes) => {
            for node in nodes {
                let mut piece = String::new();
                write_node(node, &mut piece, depth + 1);
                append(latex, &piece);
            }
        }
        MathNode::Identifier(text) => {
            let piece = if text.chars().count() > 1
                && command_of(text.chars().next().unwrap_or(' ')).is_none()
            {
                format!("\\mathrm{{{}}}", escape_text(text))
            } else {
                symbol(text)
            };
            append(latex, &piece);
        }
        MathNode::Number(text) => append(latex, text),
        MathNode::Operator(text) => append(latex, &symbol(text)),
        MathNode::Text(text) => append(latex, &format!("\\text{{{}}}", escape_text(text))),
        MathNode::Space(eighteenths) => append(
            latex,
            match eighteenths {
                0..=3 => "\\,",
                4 => "\\:",
                5 => "\\;",
                6..=17 => "\\ ",
                18..=35 => "\\quad ",
                _ => "\\qquad ",
            },
        ),
        MathNode::Fraction {
            numerator,
            denominator,
            linear,
        } => {
            if *linear {
                let piece = format!(
                    "{}/{}",
                    script_part(numerator, depth),
                    script_part(denominator, depth)
                );
                append(latex, &piece);
            } else {
                append(
                    latex,
                    &format!(
                        "\\frac{}{}",
                        group(numerator, depth),
                        group(denominator, depth)
                    ),
                );
            }
        }
        MathNode::Script { base, sub, sup }
            if matches!(base.as_ref(), MathNode::Function { .. }) =>
        {
            if let MathNode::Function { name, argument } = base.as_ref() {
                let mut piece = function_head(name);
                if let Some(sub) = sub {
                    let _ = write!(piece, "_{}", script_part(sub, depth));
                }
                if let Some(sup) = sup {
                    let _ = write!(piece, "^{}", script_part(sup, depth));
                }
                if **argument != MathNode::Row(Vec::new()) {
                    piece.push_str(&group(argument, depth));
                }
                append(latex, &piece);
            }
        }
        MathNode::Script { base, sub, sup } => {
            let mut piece = script_part(base, depth);
            if piece.is_empty() || piece == "{}" {
                "{}".clone_into(&mut piece);
            }
            if let Some(sub) = sub {
                let _ = write!(piece, "_{}", script_part(sub, depth));
            }
            if let Some(sup) = sup {
                let _ = write!(piece, "^{}", script_part(sup, depth));
            }
            append(latex, &piece);
        }
        MathNode::Limits { base, under, over } => {
            let limit_function = matches!(base.as_ref(), MathNode::Function { name, .. } if LIMIT_FUNCTIONS.contains(&name.as_str()));
            if limit_function
                && over.is_none()
                && let MathNode::Function { name, argument } = base.as_ref()
            {
                let mut piece = function_head(name);
                if let Some(under) = under {
                    let _ = write!(piece, "_{}", script_part(under, depth));
                }
                if **argument != MathNode::Row(Vec::new()) {
                    piece.push_str(&group(argument, depth));
                }
                append(latex, &piece);
                return;
            }
            let mut piece = group(base, depth);
            if let Some(under) = under {
                piece = format!("\\underset{}{piece}", group(under, depth));
            }
            if let Some(over) = over {
                piece = format!("\\overset{}{{{piece}}}", group(over, depth));
            }
            append(latex, &piece);
        }
        MathNode::Root { index, radicand } => {
            let piece = match index {
                Some(index) => {
                    let mut inner = String::new();
                    write_node(index, &mut inner, depth + 1);
                    format!("\\sqrt[{inner}]{}", group(radicand, depth))
                }
                None => format!("\\sqrt{}", group(radicand, depth)),
            };
            append(latex, &piece);
        }
        MathNode::Fenced { open, close, body } => {
            let mut inner = String::new();
            write_node(body, &mut inner, depth + 1);
            append(
                latex,
                &format!(
                    "\\left{} {inner} \\right{}",
                    delimiter(open),
                    delimiter(close)
                ),
            );
        }
        MathNode::Matrix { rows, open, close } => {
            let kind = match (open.as_str(), close.as_str()) {
                ("(", ")") => "pmatrix",
                ("[", "]") => "bmatrix",
                ("{", "}") => "Bmatrix",
                ("|", "|") => "vmatrix",
                ("\u{2016}", "\u{2016}") => "Vmatrix",
                ("{", "") => "cases",
                _ => "matrix",
            };
            let body: Vec<String> = rows
                .iter()
                .map(|cells| {
                    cells
                        .iter()
                        .map(|cell| {
                            let mut inner = String::new();
                            write_node(cell, &mut inner, depth + 1);
                            inner
                        })
                        .collect::<Vec<_>>()
                        .join(" & ")
                })
                .collect();
            let mut piece = format!("\\begin{{{kind}}} {} \\end{{{kind}}}", body.join(" \\\\ "));
            if kind == "matrix" && (!open.is_empty() || !close.is_empty()) {
                piece = format!(
                    "\\left{} {piece} \\right{}",
                    delimiter(open),
                    delimiter(close)
                );
            }
            append(latex, &piece);
        }
        MathNode::Accent { accent, base } => {
            let command = match accent {
                Accent::Hat => "hat",
                Accent::Bar => "bar",
                Accent::Dot => "dot",
                Accent::DoubleDot => "ddot",
                Accent::Tilde => "tilde",
                Accent::Vector => "vec",
                Accent::Overline => "overline",
                Accent::Underline => "underline",
            };
            append(latex, &format!("\\{command}{}", group(base, depth)));
        }
        MathNode::Function { name, argument } => {
            let mut piece = function_head(name);
            if **argument != MathNode::Row(Vec::new()) {
                piece.push_str(&group(argument, depth));
            }
            append(latex, &piece);
        }
        MathNode::Nary {
            operator,
            lower,
            upper,
            body,
        } => {
            let mut piece = BIG_OPERATORS
                .iter()
                .find(|(character, _)| character == operator)
                .map_or_else(
                    || symbol(&operator.to_string()),
                    |(_, command)| format!("\\{command}"),
                );
            if let Some(lower) = lower {
                let _ = write!(piece, "_{}", script_part(lower, depth));
            }
            if let Some(upper) = upper {
                let _ = write!(piece, "^{}", script_part(upper, depth));
            }
            if **body != MathNode::Row(Vec::new()) {
                piece.push_str(&group(body, depth));
            }
            append(latex, &piece);
        }
    }
}

#[cfg(test)]
mod tests;
