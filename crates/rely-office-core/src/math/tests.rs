// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn splits_formula_text_into_atoms() {
    assert_eq!(
        atoms_of("x+12.5α∞ y"),
        vec![
            MathNode::Identifier("x".to_owned()),
            MathNode::Operator("+".to_owned()),
            MathNode::Number("12.5".to_owned()),
            MathNode::Identifier("α".to_owned()),
            MathNode::Identifier("∞".to_owned()),
            MathNode::Identifier("y".to_owned()),
        ]
    );
}

#[test]
fn names_symbols_and_accents_both_ways() {
    assert_eq!(command_of('\u{2264}'), Some("leq"));
    assert_eq!(symbol_of("le"), Some('\u{2264}'));
    assert_eq!(symbol_of("leq"), Some('\u{2264}'));
    assert_eq!(command_of('a'), None);
    assert_eq!(symbol_of("nothing"), None);
    for accent in [
        Accent::Hat,
        Accent::Bar,
        Accent::Dot,
        Accent::DoubleDot,
        Accent::Tilde,
        Accent::Vector,
        Accent::Overline,
    ] {
        assert_eq!(Accent::from_character(accent.character()), Some(accent));
    }
    assert_eq!(
        row(vec![MathNode::Number("1".to_owned())]),
        MathNode::Number("1".to_owned())
    );
    assert_eq!(row(Vec::new()), MathNode::Row(Vec::new()));
}
