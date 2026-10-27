// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Office Math, the `m:` elements of a docx, read into the formula tree and written from it.

use std::fmt::Write as _;

use super::{Accent, MAX_DEPTH, MathNode, atoms_of, row};
use crate::document::escape_xml as escape;
use crate::xml_tree::{Element, Namespace};

const M: Namespace = Namespace::Math;

/// The formula of an `m:oMath`, or of the `m:oMath` elements of an `m:oMathPara` one after
/// the other.
pub(crate) fn omml_to_math(element: &Element) -> MathNode {
    if element.is(M, "oMathPara") {
        let parts: Vec<MathNode> = element
            .elements()
            .filter(|part| part.is(M, "oMath"))
            .map(|part| children(part, 0))
            .collect();
        return row(parts);
    }
    children(element, 0)
}

/// The value of a property of an object, `m:fPr/m:type/@m:val`.
fn property<'e>(element: &'e Element, properties: &str, name: &str) -> Option<&'e str> {
    element
        .child(M, properties)?
        .child(M, name)?
        .attribute(M, "val")
}

/// The formula of an argument, `m:e`, `m:num` and the rest, or nothing when it is missing.
fn argument(element: &Element, name: &str, depth: usize) -> MathNode {
    element.child(M, name).map_or_else(
        || MathNode::Row(Vec::new()),
        |child| children(child, depth + 1),
    )
}

fn optional(element: &Element, name: &str, depth: usize, hidden: bool) -> Option<Box<MathNode>> {
    if hidden {
        return None;
    }
    let node = argument(element, name, depth);
    (node != MathNode::Row(Vec::new())).then(|| Box::new(node))
}

/// The pieces of a container, each child an object or a run of text.
fn children(element: &Element, depth: usize) -> MathNode {
    if depth > MAX_DEPTH {
        return MathNode::Row(Vec::new());
    }
    let mut nodes = Vec::new();
    for child in element.elements() {
        match child.namespace {
            M => nodes.extend(object(child, depth)),
            Namespace::MarkupCompatibility if child.name == "AlternateContent" => {
                if let Some(choice) = child
                    .child(Namespace::MarkupCompatibility, "Choice")
                    .or_else(|| child.child(Namespace::MarkupCompatibility, "Fallback"))
                {
                    nodes.push(children(choice, depth + 1));
                }
            }
            _ => {}
        }
    }
    row(nodes)
}

fn on(value: Option<&str>) -> bool {
    matches!(value, Some("1" | "on" | "true"))
}

#[expect(
    clippy::too_many_lines,
    reason = "one match over the objects of Office Math"
)]
fn object(element: &Element, depth: usize) -> Vec<MathNode> {
    let node = match element.name.as_str() {
        "r" => {
            let text: String = element
                .elements()
                .filter(|part| part.is(M, "t") || part.is(Namespace::Word, "t"))
                .map(Element::text)
                .collect();
            // A run in plain style is text, not a formula.
            let plain = element
                .child(M, "rPr")
                .and_then(|properties| properties.child(M, "nor"))
                .is_some_and(|plain| {
                    plain
                        .attribute(M, "val")
                        .is_none_or(|value| on(Some(value)))
                });
            if text.is_empty() {
                return Vec::new();
            }
            if plain {
                if text.chars().all(char::is_whitespace) {
                    return text.chars().map(space_of).collect();
                }
                return vec![MathNode::Text(text)];
            }
            let letters = text
                .chars()
                .filter(|character| character.is_alphabetic())
                .count();
            if letters > 1 && text.chars().all(char::is_alphabetic) && is_function_name(&text) {
                return vec![MathNode::Function {
                    name: text,
                    argument: Box::new(MathNode::Row(Vec::new())),
                }];
            }
            return atoms_of(&text);
        }
        "f" => MathNode::Fraction {
            numerator: Box::new(argument(element, "num", depth)),
            denominator: Box::new(argument(element, "den", depth)),
            linear: matches!(property(element, "fPr", "type"), Some("lin" | "skw")),
        },
        "sSub" | "sSup" | "sSubSup" => MathNode::Script {
            base: Box::new(argument(element, "e", depth)),
            sub: optional(element, "sub", depth, element.name == "sSup"),
            sup: optional(element, "sup", depth, element.name == "sSub"),
        },
        "sPre" => MathNode::Row(vec![
            MathNode::Script {
                base: Box::new(MathNode::Row(Vec::new())),
                sub: optional(element, "sub", depth, false),
                sup: optional(element, "sup", depth, false),
            },
            argument(element, "e", depth),
        ]),
        "rad" => MathNode::Root {
            index: optional(
                element,
                "deg",
                depth,
                on(property(element, "radPr", "degHide")),
            ),
            radicand: Box::new(argument(element, "e", depth)),
        },
        "nary" => {
            let operator = property(element, "naryPr", "chr")
                .and_then(|operator| operator.chars().next())
                .unwrap_or('\u{222B}');
            MathNode::Nary {
                operator,
                lower: optional(
                    element,
                    "sub",
                    depth,
                    on(property(element, "naryPr", "subHide")),
                ),
                upper: optional(
                    element,
                    "sup",
                    depth,
                    on(property(element, "naryPr", "supHide")),
                ),
                body: Box::new(argument(element, "e", depth)),
            }
        }
        "d" => {
            let properties = element.child(M, "dPr");
            let character = |name: &str, default: &str| {
                properties
                    .and_then(|properties| properties.child(M, name))
                    .and_then(|child| child.attribute(M, "val"))
                    .unwrap_or(default)
                    .to_owned()
            };
            let separator = character("sepChr", "|");
            let mut body = Vec::new();
            for (index, part) in element
                .elements()
                .filter(|part| part.is(M, "e"))
                .enumerate()
            {
                if index > 0 {
                    body.push(MathNode::Operator(separator.clone()));
                }
                body.push(children(part, depth + 1));
            }
            let (open, close) = (character("begChr", "("), character("endChr", ")"));
            match row(body) {
                // A matrix between delimiters is the matrix of those delimiters.
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
        "m" => MathNode::Matrix {
            rows: element
                .elements()
                .filter(|matrix_row| matrix_row.is(M, "mr"))
                .take(1_000)
                .map(|matrix_row| {
                    matrix_row
                        .elements()
                        .filter(|cell| cell.is(M, "e"))
                        .take(1_000)
                        .map(|cell| children(cell, depth + 1))
                        .collect()
                })
                .collect(),
            open: String::new(),
            close: String::new(),
        },
        "eqArr" => MathNode::Matrix {
            rows: element
                .elements()
                .filter(|line| line.is(M, "e"))
                .take(1_000)
                .map(|line| vec![children(line, depth + 1)])
                .collect(),
            open: String::new(),
            close: String::new(),
        },
        "acc" => {
            let accent = property(element, "accPr", "chr")
                .and_then(|accent| accent.chars().next())
                .map_or(Some(Accent::Hat), Accent::from_character);
            let base = argument(element, "e", depth);
            match accent {
                Some(accent) => MathNode::Accent {
                    accent,
                    base: Box::new(base),
                },
                None => base,
            }
        }
        "bar" => MathNode::Accent {
            accent: if property(element, "barPr", "pos") == Some("top") {
                Accent::Overline
            } else {
                Accent::Underline
            },
            base: Box::new(argument(element, "e", depth)),
        },
        "func" => {
            let name = argument(element, "fName", depth);
            let argument = Box::new(argument(element, "e", depth));
            match name {
                MathNode::Function { name, .. } => MathNode::Function { name, argument },
                MathNode::Limits { base, under, over } => match *base {
                    MathNode::Function { name, .. } => MathNode::Limits {
                        base: Box::new(MathNode::Function { name, argument }),
                        under,
                        over,
                    },
                    other => MathNode::Row(vec![
                        MathNode::Limits {
                            base: Box::new(other),
                            under,
                            over,
                        },
                        *argument,
                    ]),
                },
                other => MathNode::Function {
                    name: plain_name(&other),
                    argument,
                },
            }
        }
        "limLow" | "limUpp" => {
            let base = argument(element, "e", depth);
            let limit = optional(element, "lim", depth, false);
            let base = match base {
                MathNode::Identifier(name) if is_function_name(&name) => MathNode::Function {
                    name,
                    argument: Box::new(MathNode::Row(Vec::new())),
                },
                other => other,
            };
            let lower = element.name == "limLow";
            MathNode::Limits {
                base: Box::new(base),
                under: if lower { limit.clone() } else { None },
                over: if lower { None } else { limit },
            }
        }
        "groupChr" => {
            let base = argument(element, "e", depth);
            let character = property(element, "groupChrPr", "chr").unwrap_or("\u{23DF}");
            let mark = Box::new(MathNode::Operator(character.to_owned()));
            if property(element, "groupChrPr", "pos") == Some("top") {
                MathNode::Limits {
                    base: Box::new(base),
                    under: None,
                    over: Some(mark),
                }
            } else {
                MathNode::Limits {
                    base: Box::new(base),
                    under: Some(mark),
                    over: None,
                }
            }
        }
        "box" | "borderBox" | "phant" => argument(element, "e", depth),
        "oMath" => children(element, depth + 1),
        _ => return Vec::new(),
    };
    vec![node]
}

/// The width of a space character, in eighteenths of an em, as [`math_to_omml`] writes them.
fn space_of(space: char) -> MathNode {
    MathNode::Space(match space {
        '\u{2009}' => 3,
        '\u{205F}' => 4,
        '\u{2003}' => 18,
        _ => 6,
    })
}

/// Whether a word is a function set upright, as Office Math names its own.
fn is_function_name(name: &str) -> bool {
    super::FUNCTIONS.contains(&name)
}

/// The name a function's `m:fName` spells, its letters one after the other.
fn plain_name(node: &MathNode) -> String {
    match node {
        MathNode::Row(nodes) => nodes.iter().map(plain_name).collect(),
        MathNode::Identifier(text)
        | MathNode::Number(text)
        | MathNode::Operator(text)
        | MathNode::Text(text) => text.clone(),
        MathNode::Function { name, .. } => name.clone(),
        _ => String::new(),
    }
}

/// The OMML of a formula: an `m:oMath`, inside an `m:oMathPara` when it is set on a line of its
/// own.
pub(crate) fn math_to_omml(node: &MathNode, display: bool) -> String {
    let mut body = String::new();
    write_node(node, &mut body, 0);
    if display {
        format!("<m:oMathPara><m:oMath>{body}</m:oMath></m:oMathPara>")
    } else {
        format!("<m:oMath>{body}</m:oMath>")
    }
}

fn run(xml: &mut String, text: &str, plain: bool) {
    if text.is_empty() {
        return;
    }
    let properties = if plain { "<m:rPr><m:nor/></m:rPr>" } else { "" };
    let _ = write!(
        xml,
        r#"<m:r>{properties}<m:t xml:space="preserve">{}</m:t></m:r>"#,
        escape(text)
    );
}

fn element(xml: &mut String, name: &str, node: &MathNode, depth: usize) {
    let _ = write!(xml, "<m:{name}>");
    write_node(node, xml, depth + 1);
    let _ = write!(xml, "</m:{name}>");
}

fn character_property(xml: &mut String, name: &str, value: &str) {
    let _ = write!(xml, r#"<m:{name} m:val="{}"/>"#, escape(value));
}

#[expect(
    clippy::too_many_lines,
    reason = "one match over the kinds of node a formula holds"
)]
fn write_node(node: &MathNode, xml: &mut String, depth: usize) {
    if depth > MAX_DEPTH * 2 {
        return;
    }
    let empty = MathNode::Row(Vec::new());
    match node {
        MathNode::Row(nodes) => {
            for node in nodes {
                write_node(node, xml, depth + 1);
            }
        }
        MathNode::Identifier(text) | MathNode::Number(text) | MathNode::Operator(text) => {
            run(xml, text, false);
        }
        MathNode::Text(text) => run(xml, text, true),
        MathNode::Space(eighteenths) => {
            let space = match eighteenths {
                0..=3 => "\u{2009}",
                4..=5 => "\u{205F}",
                6..=17 => " ",
                _ => "\u{2003}",
            };
            run(xml, space, true);
        }
        MathNode::Fraction {
            numerator,
            denominator,
            linear,
        } => {
            xml.push_str("<m:f>");
            if *linear {
                xml.push_str(r#"<m:fPr><m:type m:val="lin"/></m:fPr>"#);
            }
            element(xml, "num", numerator, depth);
            element(xml, "den", denominator, depth);
            xml.push_str("</m:f>");
        }
        MathNode::Script { base, sub, sup } => {
            let name = match (sub, sup) {
                (Some(_), Some(_)) => "sSubSup",
                (Some(_), None) => "sSub",
                (None, Some(_)) => "sSup",
                (None, None) => {
                    write_node(base, xml, depth + 1);
                    return;
                }
            };
            let _ = write!(xml, "<m:{name}>");
            element(xml, "e", base, depth);
            if let Some(sub) = sub {
                element(xml, "sub", sub, depth);
            }
            if let Some(sup) = sup {
                element(xml, "sup", sup, depth);
            }
            let _ = write!(xml, "</m:{name}>");
        }
        MathNode::Limits { base, under, over } => {
            if let MathNode::Function { name, argument } = base.as_ref()
                && over.is_none()
            {
                xml.push_str("<m:func><m:fName><m:limLow><m:e>");
                run(xml, name, false);
                xml.push_str("</m:e>");
                element(xml, "lim", under.as_deref().unwrap_or(&empty), depth);
                xml.push_str("</m:limLow></m:fName>");
                element(xml, "e", argument, depth);
                xml.push_str("</m:func>");
                return;
            }
            let mut inner = String::new();
            write_node(base, &mut inner, depth + 1);
            if let Some(under) = under {
                let mut limit = String::new();
                write_node(under, &mut limit, depth + 1);
                inner = format!("<m:limLow><m:e>{inner}</m:e><m:lim>{limit}</m:lim></m:limLow>");
            }
            if let Some(over) = over {
                let mut limit = String::new();
                write_node(over, &mut limit, depth + 1);
                inner = format!("<m:limUpp><m:e>{inner}</m:e><m:lim>{limit}</m:lim></m:limUpp>");
            }
            xml.push_str(&inner);
        }
        MathNode::Root { index, radicand } => {
            xml.push_str("<m:rad>");
            match index {
                Some(index) => {
                    element(xml, "deg", index, depth);
                }
                None => xml.push_str(r#"<m:radPr><m:degHide m:val="1"/></m:radPr><m:deg/>"#),
            }
            element(xml, "e", radicand, depth);
            xml.push_str("</m:rad>");
        }
        MathNode::Fenced { open, close, body } => {
            xml.push_str("<m:d><m:dPr>");
            character_property(xml, "begChr", open);
            character_property(xml, "endChr", close);
            xml.push_str("</m:dPr>");
            element(xml, "e", body, depth);
            xml.push_str("</m:d>");
        }
        MathNode::Matrix { rows, open, close } => {
            let mut matrix = String::from("<m:m>");
            for cells in rows {
                matrix.push_str("<m:mr>");
                for cell in cells {
                    element(&mut matrix, "e", cell, depth);
                }
                matrix.push_str("</m:mr>");
            }
            matrix.push_str("</m:m>");
            if open.is_empty() && close.is_empty() {
                xml.push_str(&matrix);
            } else {
                xml.push_str("<m:d><m:dPr>");
                character_property(xml, "begChr", open);
                character_property(xml, "endChr", close);
                let _ = write!(xml, "</m:dPr><m:e>{matrix}</m:e></m:d>");
            }
        }
        MathNode::Accent { accent, base } => match accent {
            Accent::Overline | Accent::Underline => {
                let position = if *accent == Accent::Overline {
                    "top"
                } else {
                    "bot"
                };
                let _ = write!(
                    xml,
                    r#"<m:bar><m:barPr><m:pos m:val="{position}"/></m:barPr>"#
                );
                element(xml, "e", base, depth);
                xml.push_str("</m:bar>");
            }
            _ => {
                xml.push_str("<m:acc><m:accPr>");
                character_property(xml, "chr", &accent.character().to_string());
                xml.push_str("</m:accPr>");
                element(xml, "e", base, depth);
                xml.push_str("</m:acc>");
            }
        },
        MathNode::Function { name, argument } => {
            xml.push_str("<m:func><m:fName>");
            run(xml, name, false);
            xml.push_str("</m:fName>");
            element(xml, "e", argument, depth);
            xml.push_str("</m:func>");
        }
        MathNode::Nary {
            operator,
            lower,
            upper,
            body,
        } => {
            xml.push_str("<m:nary><m:naryPr>");
            character_property(xml, "chr", &operator.to_string());
            if lower.is_none() {
                character_property(xml, "subHide", "1");
            }
            if upper.is_none() {
                character_property(xml, "supHide", "1");
            }
            xml.push_str("</m:naryPr>");
            element(xml, "sub", lower.as_deref().unwrap_or(&empty), depth);
            element(xml, "sup", upper.as_deref().unwrap_or(&empty), depth);
            element(xml, "e", body, depth);
            xml.push_str("</m:nary>");
        }
    }
}

#[cfg(test)]
mod tests;
