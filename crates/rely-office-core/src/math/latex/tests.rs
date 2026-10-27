// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;

/// The LaTeX a formula is written as once read: a second read gives it back unchanged.
fn normalized(latex: &str) -> String {
    let once = to_latex(&parse_latex(latex));
    assert_eq!(to_latex(&parse_latex(&once)), once, "stable: {latex}");
    once
}

#[test]
fn reads_and_writes_fractions_scripts_and_roots() {
    assert_eq!(normalized(r"\frac{a+b}{2}"), r"\frac{a+b}{2}");
    assert_eq!(normalized("x^2 + y_i^{n+1}"), "x^2+y_i^{n+1}");
    assert_eq!(normalized(r"\sqrt[3]{x}+\sqrt{2}"), r"\sqrt[3]{x}+\sqrt{2}");
    assert_eq!(normalized("f'(x)"), r"f^\prime(x)");
    assert_eq!(normalized(r"\dfrac12"), r"\frac{1}{2}");
}

#[test]
fn reads_and_writes_delimiters_matrices_and_accents() {
    assert_eq!(normalized(r"\left( a \right]"), r"\left( a \right]");
    assert_eq!(normalized(r"\left\{ x \right."), r"\left\{ x \right.");
    assert_eq!(
        normalized(r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}"),
        r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}"
    );
    assert_eq!(
        normalized(r"f(x)=\begin{cases} 1 & x>0 \\ 0 & x\le 0 \end{cases}"),
        r"f(x)=\begin{cases} 1 & x>0 \\ 0 & x\leq 0 \end{cases}"
    );
    assert_eq!(
        normalized(r"\hat{x}+\vec v+\overline{AB}"),
        r"\hat{x}+\vec{v}+\overline{AB}"
    );
}

#[test]
fn reads_and_writes_functions_operators_and_symbols() {
    assert_eq!(normalized(r"\sin x+\cos^2 x"), r"\sin{x}+\cos^2{x}");
    assert_eq!(normalized(r"\lim_{x\to 0} f"), r"\lim_{x\to 0}{f}");
    assert_eq!(normalized(r"\sum_{i=1}^{n} i^2"), r"\sum_{i=1}^n{i^2}");
    assert_eq!(normalized(r"\int_0^1 x\,dx"), r"\int_0^1{x}\,dx");
    assert_eq!(
        normalized(r"\alpha\leq\beta\cdot\infty"),
        r"\alpha\leq\beta\cdot\infty"
    );
    assert_eq!(
        normalized(r"\operatorname{rank} A"),
        r"\operatorname{rank}{A}"
    );
    assert_eq!(normalized(r"x\in\mathbb{R}"), r"x\inℝ");
    assert_eq!(normalized(r"\text{area} = \pi r^2"), r"\text{area}=\pi r^2");
}

#[test]
fn keeps_what_it_cannot_read_and_never_fails() {
    assert_eq!(normalized(r"\foo{x}"), r"\text{\\foo}x");
    assert_eq!(normalized(r"\frac{a"), r"\frac{a}{}");
    assert_eq!(normalized(r"a}}b"), "ab");
    assert_eq!(normalized(r"\left( x"), r"\left( x \right.");
    assert_eq!(normalized("50\\% \\{1\\}"), r"50\%\{1\}");
    let deep = "{".repeat(500) + "x" + &"}".repeat(500);
    let read = to_latex(&parse_latex(&deep));
    assert!(read.len() < 2_000, "a nesting past the limit is cut short");
    let long = "x+".repeat(30_000);
    assert!(to_latex(&parse_latex(&long)).len() < long.len());
}
