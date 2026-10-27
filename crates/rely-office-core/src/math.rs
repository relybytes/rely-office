// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Equations: one tree a formula is read into from Office Math (OMML) or `MathML`, kept in the
//! document tree as LaTeX, which is what an editor's formula field takes, and written back as
//! OMML or drawn as Typst math. LaTeX is read and written for the commands these formulas use:
//! fractions, scripts, roots, delimiters, matrices, accents, functions, big operators, limits
//! and the symbols of the [`SYMBOLS`] table.

mod latex;
mod mathml;
mod omml;
mod typst;

pub(crate) use latex::{parse_latex, to_latex};
pub(crate) use mathml::mathml_to_math;
pub(crate) use omml::{math_to_omml, omml_to_math};
pub use typst::latex_to_typst;

/// No formula nests deeper than this, so a crafted one cannot exhaust the stack.
const MAX_DEPTH: usize = 64;
/// No formula holds more nodes than this.
const MAX_NODES: usize = 20_000;

/// A piece of a formula.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MathNode {
    /// Pieces set one after the other.
    Row(Vec<MathNode>),
    /// A variable or a letter-like symbol, set in italic when it is one Latin letter.
    Identifier(String),
    Number(String),
    /// An operator, a relation or a delimiter used on its own.
    Operator(String),
    /// Words set upright, as text.
    Text(String),
    /// A space of so many eighteenths of an em.
    Space(u8),
    Fraction {
        numerator: Box<MathNode>,
        denominator: Box<MathNode>,
        /// Written on one line with a slash.
        linear: bool,
    },
    /// A base with a subscript, a superscript or both.
    Script {
        base: Box<MathNode>,
        sub: Option<Box<MathNode>>,
        sup: Option<Box<MathNode>>,
    },
    /// A base with something set under it, over it, or both: `lim` under its limit.
    Limits {
        base: Box<MathNode>,
        under: Option<Box<MathNode>>,
        over: Option<Box<MathNode>>,
    },
    Root {
        index: Option<Box<MathNode>>,
        radicand: Box<MathNode>,
    },
    /// A body between delimiters that grow with it; an empty delimiter is none.
    Fenced {
        open: String,
        close: String,
        body: Box<MathNode>,
    },
    /// Rows of cells, between the delimiters of the matrix kind.
    Matrix {
        rows: Vec<Vec<MathNode>>,
        open: String,
        close: String,
    },
    /// A mark over or under its base: a hat, a bar, a dot, a tilde, an arrow.
    Accent {
        accent: Accent,
        base: Box<MathNode>,
    },
    /// A function set upright, `sin`, applied to its argument.
    Function {
        name: String,
        argument: Box<MathNode>,
    },
    /// A big operator, a sum or an integral, with its limits and the expression it applies to.
    Nary {
        operator: char,
        lower: Option<Box<MathNode>>,
        upper: Option<Box<MathNode>>,
        body: Box<MathNode>,
    },
}

/// The marks a formula sets over or under a base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Accent {
    Hat,
    Bar,
    Dot,
    DoubleDot,
    Tilde,
    Vector,
    Overline,
    Underline,
}

impl Accent {
    /// The accent a combining or spacing character stands for.
    pub(crate) fn from_character(character: char) -> Option<Self> {
        Some(match character {
            '\u{0302}' | '^' | '\u{02C6}' => Self::Hat,
            '\u{0304}' | '\u{00AF}' => Self::Bar,
            '\u{0307}' | '\u{02D9}' => Self::Dot,
            '\u{0308}' | '\u{00A8}' => Self::DoubleDot,
            '\u{0303}' | '~' | '\u{02DC}' => Self::Tilde,
            '\u{20D7}' | '\u{2192}' => Self::Vector,
            '\u{0305}' => Self::Overline,
            _ => return None,
        })
    }

    /// The combining character Office Math writes for the accent.
    pub(crate) fn character(self) -> char {
        match self {
            Self::Hat => '\u{0302}',
            Self::Bar => '\u{0304}',
            Self::Dot => '\u{0307}',
            Self::DoubleDot => '\u{0308}',
            Self::Tilde => '\u{0303}',
            Self::Vector => '\u{20D7}',
            Self::Overline => '\u{0305}',
            Self::Underline => '\u{0332}',
        }
    }
}

/// The big operators, by their character and their LaTeX command.
pub(crate) const BIG_OPERATORS: [(char, &str); 12] = [
    ('\u{2211}', "sum"),
    ('\u{220F}', "prod"),
    ('\u{2210}', "coprod"),
    ('\u{222B}', "int"),
    ('\u{222C}', "iint"),
    ('\u{222D}', "iiint"),
    ('\u{222E}', "oint"),
    ('\u{22C3}', "bigcup"),
    ('\u{22C2}', "bigcap"),
    ('\u{22C1}', "bigvee"),
    ('\u{22C0}', "bigwedge"),
    ('\u{2A01}', "bigoplus"),
];

/// The functions set upright, which LaTeX names with a command of their own.
pub(crate) const FUNCTIONS: [&str; 32] = [
    "sin", "cos", "tan", "cot", "sec", "csc", "arcsin", "arccos", "arctan", "sinh", "cosh", "tanh",
    "coth", "log", "ln", "lg", "exp", "lim", "liminf", "limsup", "max", "min", "sup", "inf", "det",
    "dim", "ker", "deg", "gcd", "arg", "hom", "Pr",
];

/// The symbols LaTeX names with a command, by their character.
pub(crate) const SYMBOLS: [(char, &str); 96] = [
    ('\u{03B1}', "alpha"),
    ('\u{03B2}', "beta"),
    ('\u{03B3}', "gamma"),
    ('\u{03B4}', "delta"),
    ('\u{03B5}', "epsilon"),
    ('\u{03F5}', "epsilon"),
    ('\u{03B6}', "zeta"),
    ('\u{03B7}', "eta"),
    ('\u{03B8}', "theta"),
    ('\u{03B9}', "iota"),
    ('\u{03BA}', "kappa"),
    ('\u{03BB}', "lambda"),
    ('\u{03BC}', "mu"),
    ('\u{03BD}', "nu"),
    ('\u{03BE}', "xi"),
    ('\u{03C0}', "pi"),
    ('\u{03C1}', "rho"),
    ('\u{03C3}', "sigma"),
    ('\u{03C2}', "varsigma"),
    ('\u{03C4}', "tau"),
    ('\u{03C5}', "upsilon"),
    ('\u{03C6}', "phi"),
    ('\u{03D5}', "phi"),
    ('\u{03C7}', "chi"),
    ('\u{03C8}', "psi"),
    ('\u{03C9}', "omega"),
    ('\u{0393}', "Gamma"),
    ('\u{0394}', "Delta"),
    ('\u{0398}', "Theta"),
    ('\u{039B}', "Lambda"),
    ('\u{039E}', "Xi"),
    ('\u{03A0}', "Pi"),
    ('\u{03A3}', "Sigma"),
    ('\u{03A5}', "Upsilon"),
    ('\u{03A6}', "Phi"),
    ('\u{03A8}', "Psi"),
    ('\u{03A9}', "Omega"),
    ('\u{00B1}', "pm"),
    ('\u{2213}', "mp"),
    ('\u{00D7}', "times"),
    ('\u{00F7}', "div"),
    ('\u{22C5}', "cdot"),
    ('\u{00B7}', "cdot"),
    ('\u{2217}', "ast"),
    ('\u{2218}', "circ"),
    ('\u{2219}', "bullet"),
    ('\u{2264}', "leq"),
    ('\u{2265}', "geq"),
    ('\u{2260}', "neq"),
    ('\u{2248}', "approx"),
    ('\u{2261}', "equiv"),
    ('\u{221D}', "propto"),
    ('\u{223C}', "sim"),
    ('\u{2243}', "simeq"),
    ('\u{226A}', "ll"),
    ('\u{226B}', "gg"),
    ('\u{2208}', "in"),
    ('\u{2209}', "notin"),
    ('\u{220B}', "ni"),
    ('\u{2282}', "subset"),
    ('\u{2283}', "supset"),
    ('\u{2286}', "subseteq"),
    ('\u{2287}', "supseteq"),
    ('\u{222A}', "cup"),
    ('\u{2229}', "cap"),
    ('\u{2205}', "emptyset"),
    ('\u{2200}', "forall"),
    ('\u{2203}', "exists"),
    ('\u{00AC}', "neg"),
    ('\u{2227}', "wedge"),
    ('\u{2228}', "vee"),
    ('\u{2295}', "oplus"),
    ('\u{2297}', "otimes"),
    ('\u{221E}', "infty"),
    ('\u{2202}', "partial"),
    ('\u{2207}', "nabla"),
    ('\u{210F}', "hbar"),
    ('\u{2113}', "ell"),
    ('\u{2192}', "to"),
    ('\u{2190}', "leftarrow"),
    ('\u{2194}', "leftrightarrow"),
    ('\u{21D2}', "Rightarrow"),
    ('\u{21D0}', "Leftarrow"),
    ('\u{21D4}', "Leftrightarrow"),
    ('\u{21A6}', "mapsto"),
    ('\u{2191}', "uparrow"),
    ('\u{2193}', "downarrow"),
    ('\u{2026}', "ldots"),
    ('\u{22EF}', "cdots"),
    ('\u{22EE}', "vdots"),
    ('\u{22F1}', "ddots"),
    ('\u{2032}', "prime"),
    ('\u{2220}', "angle"),
    ('\u{22A5}', "perp"),
    ('\u{2225}', "parallel"),
    ('\u{00B0}', "circ"),
];

/// The LaTeX command of a symbol, `None` for a character that needs none.
pub(crate) fn command_of(character: char) -> Option<&'static str> {
    if character == '\u{00B0}' {
        return None;
    }
    SYMBOLS
        .iter()
        .find(|(symbol, _)| *symbol == character)
        .map(|(_, command)| *command)
}

/// The character of a LaTeX symbol command.
pub(crate) fn symbol_of(command: &str) -> Option<char> {
    SYMBOLS
        .iter()
        .find(|(_, name)| *name == command)
        .map(|(symbol, _)| *symbol)
        .or(match command {
            "le" => Some('\u{2264}'),
            "ge" => Some('\u{2265}'),
            "ne" => Some('\u{2260}'),
            "varepsilon" => Some('\u{03B5}'),
            "varphi" => Some('\u{03C6}'),
            "rightarrow" => Some('\u{2192}'),
            "gets" => Some('\u{2190}'),
            "implies" => Some('\u{21D2}'),
            "iff" => Some('\u{21D4}'),
            "dots" => Some('\u{2026}'),
            "lbrace" | "{" => Some('{'),
            "rbrace" | "}" => Some('}'),
            "langle" => Some('\u{27E8}'),
            "rangle" => Some('\u{27E9}'),
            "lfloor" => Some('\u{230A}'),
            "rfloor" => Some('\u{230B}'),
            "lceil" => Some('\u{2308}'),
            "rceil" => Some('\u{2309}'),
            "vert" | "|" => Some('|'),
            "Vert" => Some('\u{2016}'),
            "%" => Some('%'),
            "$" => Some('$'),
            "&" => Some('&'),
            "#" => Some('#'),
            "_" => Some('_'),
            _ => None,
        })
}

/// The class a character of a formula's text falls in when the text is split into atoms.
pub(crate) fn atom_of(character: char) -> MathNode {
    if character.is_ascii_digit() || character == '.' {
        MathNode::Number(character.to_string())
    } else if character.is_alphabetic() || command_of(character).is_some_and(is_letter_like) {
        MathNode::Identifier(character.to_string())
    } else {
        MathNode::Operator(character.to_string())
    }
}

/// Whether a symbol command names something set like a letter rather than an operator.
fn is_letter_like(command: &str) -> bool {
    matches!(command, "infty" | "partial" | "nabla" | "emptyset")
}

/// The atoms of a run of formula text: digits joined into numbers, every other character an
/// atom of its own.
pub(crate) fn atoms_of(text: &str) -> Vec<MathNode> {
    let mut atoms: Vec<MathNode> = Vec::new();
    for character in text.chars() {
        let atom = atom_of(character);
        if let (MathNode::Number(next), Some(MathNode::Number(previous))) =
            (&atom, atoms.last_mut())
        {
            previous.push_str(next);
            continue;
        }
        if character.is_whitespace() {
            continue;
        }
        atoms.push(atom);
    }
    atoms
}

/// A row of nodes, or the one node when there is one. A function or a big operator with
/// nothing to apply to takes the node after it, as LaTeX reads `\sum_i x`.
pub(crate) fn row(nodes: Vec<MathNode>) -> MathNode {
    let mut joined: Vec<MathNode> = Vec::with_capacity(nodes.len());
    for node in nodes {
        if let Some(last) = joined.last_mut()
            && takes_operand(last)
            && !matches!(
                node,
                MathNode::Operator(_) | MathNode::Space(_) | MathNode::Text(_)
            )
        {
            give_operand(last, node);
            continue;
        }
        joined.push(node);
    }
    if joined.len() == 1 {
        joined.remove(0)
    } else {
        MathNode::Row(joined)
    }
}

fn is_empty(node: &MathNode) -> bool {
    *node == MathNode::Row(Vec::new())
}

/// Whether a node is a function or a big operator with nothing yet to apply to.
fn takes_operand(node: &MathNode) -> bool {
    match node {
        MathNode::Nary { body, .. } => is_empty(body),
        MathNode::Function { argument, .. } => is_empty(argument),
        MathNode::Limits { base, .. } | MathNode::Script { base, .. } => {
            matches!(base.as_ref(), MathNode::Function { argument, .. } if is_empty(argument))
        }
        _ => false,
    }
}

fn give_operand(node: &mut MathNode, operand: MathNode) {
    match node {
        MathNode::Nary { body, .. } => **body = operand,
        MathNode::Function { argument, .. } => **argument = operand,
        MathNode::Limits { base, .. } | MathNode::Script { base, .. } => {
            if let MathNode::Function { argument, .. } = base.as_mut() {
                **argument = operand;
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
