// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;

#[test]
fn writes_typst_math() {
    assert_eq!(latex_to_typst(r"\frac{a+b}{2}"), "frac(a + b, 2)");
    assert_eq!(latex_to_typst("x_i^2"), "x_(i)^(2)");
    assert_eq!(latex_to_typst(r"\sqrt[3]{x}"), "root(3, x)");
    assert_eq!(
        latex_to_typst(r"\left( a \right]"),
        "lr(paren.l a bracket.r)"
    );
    assert_eq!(
        latex_to_typst(r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}"),
        "mat(delim: \"(\", a, b; c, d)"
    );
    assert_eq!(latex_to_typst(r"\sum_{i=1}^{n} i"), "sum_(i = 1)^(n) i");
    assert_eq!(
        latex_to_typst(r"\lim_{x\to 0} f"),
        "attach(limits(lim f), b: x → 0)"
    );
    assert_eq!(latex_to_typst(r"\hat{x}"), "hat(x)");
    assert_eq!(latex_to_typst(r"\operatorname{rank} A"), "op(\"rank\") A");
    assert_eq!(latex_to_typst(r"\text{area}"), "\"area\"");
    assert_eq!(latex_to_typst(""), "\"\"");
    assert_eq!(
        latex_to_typst(r"\frac{n(n+1)}{2}"),
        "frac(n ( n + 1 ), 2)",
        "balanced parentheses stay as they are"
    );
}

#[test]
fn lets_no_formula_text_become_typst_syntax() {
    let written = latex_to_typst(r#"\frac{a, b}{c: d} \text{" #x $} / _ ^ & * @"#);
    assert_eq!(
        written, r#"frac(a "," b, c ":" d) "\" #x $" "/"_(attach("", t: "&")) "*" "@""#,
        "commas, colons, quotes and dollars stay in strings"
    );
    for token in ["\"/\"", "\"*\"", "\"@\""] {
        assert!(written.contains(token), "{token} in {written}");
    }
    assert_eq!(
        latex_to_typst("(a"),
        "\"(\" a",
        "an unbalanced parenthesis is text"
    );
}
