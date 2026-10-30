// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! Presentation `MathML`, the formula objects of an odt, read into the formula tree.

use super::{Accent, BIG_OPERATORS, FUNCTIONS, MAX_DEPTH, MathNode, atoms_of, row};
use crate::xml_tree::{Element, Namespace};

const MATHML: Namespace = Namespace::MathMl;

/// The formula of a `math` element.
pub(crate) fn mathml_to_math(math: &Element) -> MathNode {
    node(math, 0)
}

fn children(element: &Element, depth: usize) -> Vec<MathNode> {
    element
        .elements()
        .filter(|child| child.namespace == MATHML)
        .map(|child| node(child, depth + 1))
        .collect()
}

/// The `index`th child of an element, or nothing.
fn nth(element: &Element, index: usize, depth: usize) -> MathNode {
    element
        .elements()
        .filter(|child| child.namespace == MATHML)
        .nth(index)
        .map_or_else(|| MathNode::Row(Vec::new()), |child| node(child, depth + 1))
}

fn boxed(node: MathNode) -> Option<Box<MathNode>> {
    (node != MathNode::Row(Vec::new())).then(|| Box::new(node))
}

#[expect(
    clippy::too_many_lines,
    reason = "one match over the elements of presentation MathML"
)]
fn node(element: &Element, depth: usize) -> MathNode {
    if depth > MAX_DEPTH {
        return MathNode::Row(Vec::new());
    }
    let text = || element.text().trim().to_owned();
    match element.name.as_str() {
        "math" | "mrow" | "mstyle" | "mpadded" | "mphantom" | "menclose" | "merror" => {
            fenced_row(children(element, depth))
        }
        // The annotations of a semantics element say the same in other languages.
        "semantics" => nth(element, 0, depth),
        "mi" => {
            let name = text();
            if name.chars().count() > 1 && FUNCTIONS.contains(&name.as_str()) {
                MathNode::Function {
                    name,
                    argument: Box::new(MathNode::Row(Vec::new())),
                }
            } else if name.chars().count() > 1 {
                MathNode::Identifier(name)
            } else {
                row(atoms_of(&name))
            }
        }
        "mn" => MathNode::Number(text()),
        "mo" => {
            let operator = text();
            let mut characters = operator.chars();
            match (characters.next(), characters.next()) {
                (Some(single), None) if BIG_OPERATORS.iter().any(|(big, _)| *big == single) => {
                    MathNode::Nary {
                        operator: single,
                        lower: None,
                        upper: None,
                        body: Box::new(MathNode::Row(Vec::new())),
                    }
                }
                _ => MathNode::Operator(operator),
            }
        }
        "mtext" | "ms" => {
            let raw = element.text();
            if !raw.is_empty() && raw.trim().is_empty() {
                MathNode::Space(6)
            } else {
                MathNode::Text(text())
            }
        }
        "mspace" => MathNode::Space(6),
        "mfrac" => MathNode::Fraction {
            numerator: Box::new(nth(element, 0, depth)),
            denominator: Box::new(nth(element, 1, depth)),
            linear: element.attribute(Namespace::None, "bevelled") == Some("true"),
        },
        "msqrt" => MathNode::Root {
            index: None,
            radicand: Box::new(row(children(element, depth))),
        },
        "mroot" => MathNode::Root {
            index: boxed(nth(element, 1, depth)),
            radicand: Box::new(nth(element, 0, depth)),
        },
        "msub" | "msup" | "msubsup" => {
            let base = nth(element, 0, depth);
            let (sub, sup) = match element.name.as_str() {
                "msub" => (boxed(nth(element, 1, depth)), None),
                "msup" => (None, boxed(nth(element, 1, depth))),
                _ => (boxed(nth(element, 1, depth)), boxed(nth(element, 2, depth))),
            };
            with_scripts(base, sub, sup, false)
        }
        "munder" | "mover" | "munderover" => {
            let base = nth(element, 0, depth);
            let (under, over) = match element.name.as_str() {
                "munder" => (boxed(nth(element, 1, depth)), None),
                "mover" => (None, boxed(nth(element, 1, depth))),
                _ => (boxed(nth(element, 1, depth)), boxed(nth(element, 2, depth))),
            };
            if element.name == "mover"
                && let Some(MathNode::Operator(mark)) = over.as_deref()
                && let Some(accent) = mark.chars().next().and_then(Accent::from_character)
            {
                return MathNode::Accent {
                    accent,
                    base: Box::new(base),
                };
            }
            with_scripts(base, under, over, true)
        }
        "mfenced" => {
            let open = element.attribute(Namespace::None, "open").unwrap_or("(");
            let close = element.attribute(Namespace::None, "close").unwrap_or(")");
            let separator = element
                .attribute(Namespace::None, "separators")
                .unwrap_or(",")
                .chars()
                .next()
                .map(String::from);
            let mut body = Vec::new();
            for (index, part) in children(element, depth).into_iter().enumerate() {
                if index > 0
                    && let Some(separator) = &separator
                {
                    body.push(MathNode::Operator(separator.clone()));
                }
                body.push(part);
            }
            MathNode::Fenced {
                open: open.to_owned(),
                close: close.to_owned(),
                body: Box::new(row(body)),
            }
        }
        "mtable" => MathNode::Matrix {
            rows: element
                .elements()
                .filter(|table_row| {
                    table_row.is(MATHML, "mtr") || table_row.is(MATHML, "mlabeledtr")
                })
                .take(1_000)
                .map(|table_row| {
                    table_row
                        .elements()
                        .filter(|cell| cell.is(MATHML, "mtd"))
                        .take(1_000)
                        .map(|cell| row(children(cell, depth + 1)))
                        .collect()
                })
                .collect(),
            open: String::new(),
            close: String::new(),
        },
        _ => MathNode::Row(Vec::new()),
    }
}

/// A row whose first and last pieces are an opening and a closing delimiter around the rest:
/// the delimiters of a matrix it holds, or a fenced body.
fn fenced_row(mut nodes: Vec<MathNode>) -> MathNode {
    if !encloses(&nodes) {
        return row(nodes);
    }
    let (Some(MathNode::Operator(close)), MathNode::Operator(open)) =
        (nodes.pop(), nodes.remove(0))
    else {
        return row(nodes);
    };
    match row(nodes) {
        MathNode::Matrix {
            rows,
            open: inner_open,
            close: inner_close,
        } if inner_open.is_empty() && inner_close.is_empty() => {
            MathNode::Matrix { rows, open, close }
        }
        body => MathNode::Fenced {
            open,
            close,
            body: Box::new(body),
        },
    }
}

/// Whether a row is one pair of delimiters around the rest: the first piece opens, the last
/// closes the same kind, and nothing between closes the first one early.
fn encloses(nodes: &[MathNode]) -> bool {
    const PAIRS: [(&str, &str); 6] = [
        ("(", ")"),
        ("[", "]"),
        ("{", "}"),
        ("\u{27E8}", "\u{27E9}"),
        ("|", "|"),
        ("\u{2016}", "\u{2016}"),
    ];
    fn text(node: Option<&MathNode>) -> Option<&str> {
        match node {
            Some(MathNode::Operator(text)) => Some(text.as_str()),
            _ => None,
        }
    }
    let (Some(open), Some(close)) = (text(nodes.first()), text(nodes.last())) else {
        return false;
    };
    if nodes.len() < 3 || !PAIRS.contains(&(open, close)) {
        return false;
    }
    let mut depth = 0_usize;
    for (index, node) in nodes.iter().enumerate() {
        let Some(piece) = text(Some(node)) else {
            continue;
        };
        if open == close {
            if piece == open && index != 0 && index != nodes.len() - 1 {
                return false;
            }
            continue;
        }
        if piece == open {
            depth += 1;
        } else if piece == close {
            depth = depth.saturating_sub(1);
            if depth == 0 && index != nodes.len() - 1 {
                return false;
            }
        }
    }
    true
}

/// A base with scripts or limits; a big operator takes them as its own.
fn with_scripts(
    base: MathNode,
    lower: Option<Box<MathNode>>,
    upper: Option<Box<MathNode>>,
    under: bool,
) -> MathNode {
    match base {
        MathNode::Nary { operator, body, .. } => MathNode::Nary {
            operator,
            lower,
            upper,
            body,
        },
        base if under => MathNode::Limits {
            base: Box::new(base),
            under: lower,
            over: upper,
        },
        base => MathNode::Script {
            base: Box::new(base),
            sub: lower,
            sup: upper,
        },
    }
}

#[cfg(test)]
mod tests;
