// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::math::to_latex;
use crate::xml_tree::parse_tree;

fn read(inner: &str) -> String {
    let root = parse_tree(
        &format!(r#"<math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mrow>{inner}</mrow><annotation encoding="StarMath 5.0">x</annotation></semantics></math>"#),
        |namespace, name| namespace == Namespace::MathMl && name == "mtext",
    )
    .unwrap();
    to_latex(&mathml_to_math(&root))
}

#[test]
fn reads_presentation_mathml() {
    assert_eq!(read("<mfrac><mi>a</mi><mn>2</mn></mfrac>"), r"\frac{a}{2}");
    assert_eq!(
        read("<msubsup><mi>x</mi><mi>i</mi><mn>2</mn></msubsup>"),
        "x_i^2"
    );
    assert_eq!(read("<mroot><mi>x</mi><mn>3</mn></mroot>"), r"\sqrt[3]{x}");
    assert_eq!(
        read("<msqrt><mi>x</mi><mo>+</mo><mn>1</mn></msqrt>"),
        r"\sqrt{x+1}"
    );
    assert_eq!(
        read(
            "<munderover><mo>∑</mo><mrow><mi>i</mi><mo>=</mo><mn>1</mn></mrow><mi>n</mi></munderover><mi>i</mi>"
        ),
        r"\sum_{i=1}^n{i}"
    );
    assert_eq!(read("<mover><mi>x</mi><mo>^</mo></mover>"), r"\hat{x}");
    assert_eq!(read("<mi>sin</mi><mi>x</mi>"), r"\sin{x}");
    assert_eq!(
        read(r#"<mfenced open="[" close="]"><mi>a</mi><mi>b</mi></mfenced>"#),
        r"\left[ a,b \right]"
    );
    assert_eq!(
        read("<mtable><mtr><mtd><mn>1</mn></mtd><mtd><mn>0</mn></mtd></mtr></mtable>"),
        r"\begin{matrix} 1 & 0 \end{matrix}"
    );
    assert_eq!(read("<mtext>area</mtext>"), r"\text{area}");
    assert_eq!(read("<mi>a</mi><mtext> </mtext><mi>b</mi>"), r"a\ b");
    assert_eq!(
        read(
            r#"<mrow><mo fence="true">(</mo><mtable><mtr><mtd><mn>1</mn></mtd></mtr></mtable><mo fence="true">)</mo></mrow>"#
        ),
        r"\begin{pmatrix} 1 \end{pmatrix}"
    );
    assert_eq!(
        read("<mrow><mo>[</mo><mi>a</mi><mo>+</mo><mi>b</mi><mo>]</mo></mrow>"),
        r"\left[ a+b \right]"
    );
    assert_eq!(read("<unknown/>"), "");
}
