// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The formula tree as Typst math. Nothing of a formula's own text can become Typst syntax:
//! letters and digits are written as they are, the operators Typst reads as syntax go in as
//! strings, delimiters by their symbol names, and every function is one of Typst's own.

use std::fmt::Write as _;

use super::{Accent, MAX_DEPTH, MathNode, parse_latex};

/// The operators Typst math takes as they are, with no meaning of their own between spaces.
const PLAIN_OPERATORS: &str = "+-=<>!|.?";

/// The functions Typst math sets upright by name.
const TYPST_FUNCTIONS: [&str; 34] = [
    "arccos", "arcsin", "arctan", "arg", "cos", "cosh", "cot", "coth", "csc", "deg", "det", "dim",
    "exp", "gcd", "hom", "inf", "ker", "lg", "lim", "liminf", "limsup", "ln", "log", "max", "min",
    "Pr", "sec", "sin", "sinh", "sup", "tan", "tanh", "tg", "ctg",
];

/// A LaTeX formula as the body of a Typst equation, the text between its dollars.
#[must_use]
pub fn latex_to_typst(latex: &str) -> String {
    let mut typst = String::new();
    write_node(&parse_latex(latex), &mut typst, 0);
    let typst = typst.trim().to_owned();
    if typst.is_empty() {
        "\"\"".to_owned()
    } else {
        typst
    }
}

/// A Typst string literal of `text`.
fn string(text: &str) -> String {
    let mut literal = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => literal.push_str("\\\""),
            '\\' => literal.push_str("\\\\"),
            control if control.is_control() => literal.push(' '),
            other => literal.push(other),
        }
    }
    literal.push('"');
    literal
}

fn piece(node: &MathNode, depth: usize) -> String {
    let mut written = String::new();
    write_node(node, &mut written, depth + 1);
    let written = written.trim().to_owned();
    if written.is_empty() {
        "\"\"".to_owned()
    } else {
        written
    }
}

fn is_bracket(text: &str) -> bool {
    matches!(text, "(" | ")" | "[" | "]")
}

/// Whether the parentheses and brackets of a row pair up, so Typst can take them as they are
/// without one of them closing a function's arguments.
fn brackets_balance(nodes: &[MathNode]) -> bool {
    let mut open: Vec<&str> = Vec::new();
    for node in nodes {
        let MathNode::Operator(text) = node else {
            continue;
        };
        match text.as_str() {
            "(" | "[" => open.push(text),
            ")" if open.pop() != Some("(") => return false,
            "]" if open.pop() != Some("[") => return false,
            _ => {}
        }
    }
    open.is_empty()
}

/// Whether a node is written as one atom a script can follow directly.
fn is_atom(node: &MathNode) -> bool {
    match node {
        MathNode::Identifier(text) => text.chars().count() == 1,
        MathNode::Number(_)
        | MathNode::Text(_)
        | MathNode::Operator(_)
        | MathNode::Fraction { .. }
        | MathNode::Root { .. }
        | MathNode::Fenced { .. }
        | MathNode::Matrix { .. }
        | MathNode::Accent { .. } => true,
        _ => false,
    }
}

/// The Typst symbol of a delimiter; `None` for none.
fn delimiter(delimiter: &str) -> Option<&'static str> {
    Some(match delimiter {
        "(" => "paren.l",
        ")" => "paren.r",
        "[" => "bracket.l",
        "]" => "bracket.r",
        "{" => "brace.l",
        "}" => "brace.r",
        "|" => "bar.v",
        "\u{2016}" => "bar.v.double",
        "\u{27E8}" | "<" => "angle.l",
        "\u{27E9}" | ">" => "angle.r",
        "\u{230A}" => "floor.l",
        "\u{230B}" => "floor.r",
        "\u{2308}" => "ceil.l",
        "\u{2309}" => "ceil.r",
        _ => return None,
    })
}

/// The Typst symbol of a big operator.
fn big_operator(operator: char) -> Option<&'static str> {
    Some(match operator {
        '\u{2211}' => "sum",
        '\u{220F}' => "product",
        '\u{2210}' => "product.co",
        '\u{222B}' => "integral",
        '\u{222C}' => "integral.double",
        '\u{222D}' => "integral.triple",
        '\u{222E}' => "integral.cont",
        '\u{22C3}' => "union.big",
        '\u{22C2}' => "sect.big",
        '\u{22C1}' => "or.big",
        '\u{22C0}' => "and.big",
        '\u{2A01}' => "plus.circle.big",
        _ => return None,
    })
}

fn push(typst: &mut String, piece: &str) {
    if !typst.is_empty() && !typst.ends_with(' ') {
        typst.push(' ');
    }
    typst.push_str(piece);
}

/// The scripts of a base, `_(…)^(…)` after an atom and through `attach` otherwise.
fn attached(base: &str, atom: bool, sub: Option<String>, sup: Option<String>) -> String {
    if atom {
        let mut written = base.to_owned();
        if let Some(sub) = sub {
            let _ = write!(written, "_({sub})");
        }
        if let Some(sup) = sup {
            let _ = write!(written, "^({sup})");
        }
        return written;
    }
    let mut arguments = vec![base.to_owned()];
    if let Some(sub) = sub {
        arguments.push(format!("b: {sub}"));
    }
    if let Some(sup) = sup {
        arguments.push(format!("t: {sup}"));
    }
    format!("attach({})", arguments.join(", "))
}

#[expect(
    clippy::too_many_lines,
    reason = "one match over the kinds of node a formula holds"
)]
fn write_node(node: &MathNode, typst: &mut String, depth: usize) {
    if depth > MAX_DEPTH * 2 {
        return;
    }
    match node {
        MathNode::Row(nodes) => {
            let balanced = brackets_balance(nodes);
            for node in nodes {
                match node {
                    MathNode::Operator(text) if balanced && is_bracket(text) => push(typst, text),
                    node => {
                        let written = piece(node, depth);
                        push(typst, &written);
                    }
                }
            }
        }
        MathNode::Identifier(text) => {
            let mut characters = text.chars();
            match (characters.next(), characters.next()) {
                (Some(letter), None) if letter.is_alphabetic() => push(typst, &letter.to_string()),
                (Some(_), _) => push(typst, &format!("upright({})", string(text))),
                (None, _) => {}
            }
        }
        MathNode::Number(text) => {
            let digits: String = text
                .chars()
                .filter(|character| character.is_ascii_digit() || *character == '.')
                .collect();
            if !digits.is_empty() {
                push(typst, &digits);
            }
        }
        MathNode::Operator(text) => {
            for character in text.chars() {
                let written = if PLAIN_OPERATORS.contains(character)
                    || (!character.is_ascii() && !character.is_alphanumeric())
                {
                    character.to_string()
                } else {
                    string(&character.to_string())
                };
                push(typst, &written);
            }
        }
        MathNode::Text(text) => push(typst, &string(text)),
        MathNode::Space(eighteenths) => push(
            typst,
            match eighteenths {
                0..=3 => "thin",
                4 => "med",
                5 => "thick",
                6..=17 => "space",
                18..=35 => "quad",
                _ => "wide",
            },
        ),
        MathNode::Fraction {
            numerator,
            denominator,
            linear,
        } => {
            let (numerator, denominator) = (piece(numerator, depth), piece(denominator, depth));
            if *linear {
                push(typst, &format!("{numerator} \"/\" {denominator}"));
            } else {
                push(typst, &format!("frac({numerator}, {denominator})"));
            }
        }
        MathNode::Script { base, sub, sup } => {
            let written = attached(
                &piece(base, depth),
                is_atom(base),
                sub.as_ref().map(|sub| piece(sub, depth)),
                sup.as_ref().map(|sup| piece(sup, depth)),
            );
            push(typst, &written);
        }
        MathNode::Limits { base, under, over } => {
            let mut arguments = vec![format!("limits({})", piece(base, depth))];
            if let Some(under) = under {
                arguments.push(format!("b: {}", piece(under, depth)));
            }
            if let Some(over) = over {
                arguments.push(format!("t: {}", piece(over, depth)));
            }
            push(typst, &format!("attach({})", arguments.join(", ")));
        }
        MathNode::Root { index, radicand } => {
            let radicand = piece(radicand, depth);
            match index {
                Some(index) => push(typst, &format!("root({}, {radicand})", piece(index, depth))),
                None => push(typst, &format!("sqrt({radicand})")),
            }
        }
        MathNode::Fenced { open, close, body } => {
            let body = piece(body, depth);
            match (delimiter(open), delimiter(close)) {
                (None, None) => push(typst, &body),
                (open, close) => {
                    let mut parts = Vec::new();
                    parts.extend(open);
                    parts.push(body.as_str());
                    parts.extend(close);
                    push(typst, &format!("lr({})", parts.join(" ")));
                }
            }
        }
        MathNode::Matrix { rows, open, close } => {
            let body: Vec<String> = rows
                .iter()
                .map(|cells| {
                    cells
                        .iter()
                        .map(|cell| piece(cell, depth))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .collect();
            if open == "{" && close.is_empty() {
                let cases: Vec<String> = rows
                    .iter()
                    .map(|cells| {
                        cells
                            .iter()
                            .map(|cell| piece(cell, depth))
                            .collect::<Vec<_>>()
                            .join(" & ")
                    })
                    .collect();
                push(typst, &format!("cases({})", cases.join(", ")));
                return;
            }
            let delimiter = match (open.as_str(), close.as_str()) {
                ("(", ")") => "\"(\"",
                ("[", "]") => "\"[\"",
                ("{", "}") => "\"{\"",
                ("|", "|") => "\"|\"",
                ("\u{2016}", "\u{2016}") => "\"||\"",
                _ => "#none",
            };
            let delimiter = if delimiter == "#none" {
                "none"
            } else {
                delimiter
            };
            push(
                typst,
                &format!("mat(delim: {delimiter}, {})", body.join("; ")),
            );
        }
        MathNode::Accent { accent, base } => {
            let function = match accent {
                Accent::Hat => "hat",
                Accent::Bar => "macron",
                Accent::Dot => "dot",
                Accent::DoubleDot => "dot.double",
                Accent::Tilde => "tilde",
                Accent::Vector => "arrow",
                Accent::Overline => "overline",
                Accent::Underline => "underline",
            };
            push(typst, &format!("{function}({})", piece(base, depth)));
        }
        MathNode::Function { name, argument } => {
            let head = if TYPST_FUNCTIONS.contains(&name.as_str()) {
                name.clone()
            } else {
                format!("op({})", string(name))
            };
            push(typst, &head);
            if **argument != MathNode::Row(Vec::new()) {
                let argument = piece(argument, depth);
                push(typst, &argument);
            }
        }
        MathNode::Nary {
            operator,
            lower,
            upper,
            body,
        } => {
            let base = big_operator(*operator)
                .map_or_else(|| string(&operator.to_string()), str::to_owned);
            let written = attached(
                &base,
                true,
                lower.as_ref().map(|lower| piece(lower, depth)),
                upper.as_ref().map(|upper| piece(upper, depth)),
            );
            push(typst, &written);
            if **body != MathNode::Row(Vec::new()) {
                let body = piece(body, depth);
                push(typst, &body);
            }
        }
    }
}

#[cfg(test)]
mod tests;
