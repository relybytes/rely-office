// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use super::*;
use crate::math::{parse_latex, to_latex};
use crate::xml_tree::parse_tree;

const NAMESPACE: &str = r#"xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math""#;

fn read(inner: &str) -> String {
    let root = parse_tree(
        &format!("<m:oMath {NAMESPACE}>{inner}</m:oMath>"),
        |_, _| false,
    )
    .unwrap();
    to_latex(&omml_to_math(&root))
}

fn r(text: &str) -> String {
    format!("<m:r><m:t>{text}</m:t></m:r>")
}

#[test]
fn reads_the_objects_of_office_math() {
    assert_eq!(
        read(&format!(
            "<m:f><m:num>{}</m:num><m:den>{}</m:den></m:f>",
            r("a+b"),
            r("2")
        )),
        r"\frac{a+b}{2}"
    );
    assert_eq!(
        read(&format!(
            "<m:sSubSup><m:e>{}</m:e><m:sub>{}</m:sub><m:sup>{}</m:sup></m:sSubSup>",
            r("x"),
            r("i"),
            r("2")
        )),
        "x_i^2"
    );
    assert_eq!(
        read(&format!(
            r#"<m:rad><m:radPr><m:degHide m:val="1"/></m:radPr><m:deg/><m:e>{}</m:e></m:rad>"#,
            r("x")
        )),
        r"\sqrt{x}"
    );
    assert_eq!(
        read(&format!(
            r#"<m:nary><m:naryPr><m:chr m:val="∑"/></m:naryPr><m:sub>{}</m:sub><m:sup>{}</m:sup><m:e>{}</m:e></m:nary>"#,
            r("i=1"),
            r("n"),
            r("i")
        )),
        r"\sum_{i=1}^n{i}"
    );
    assert_eq!(
        read(&format!(
            r#"<m:d><m:dPr><m:begChr m:val="["/><m:endChr m:val="]"/></m:dPr><m:e>{}</m:e><m:e>{}</m:e></m:d>"#,
            r("a"),
            r("b")
        )),
        r"\left[ a|b \right]"
    );
    assert_eq!(
        read(&format!(
            "<m:func><m:fName>{}</m:fName><m:e>{}</m:e></m:func>",
            r("sin"),
            r("x")
        )),
        r"\sin{x}"
    );
    assert_eq!(
        read(&format!(
            "<m:func><m:fName><m:limLow><m:e>{}</m:e><m:lim>{}</m:lim></m:limLow></m:fName><m:e>{}</m:e></m:func>",
            r("lim"),
            r("x→0"),
            r("f")
        )),
        r"\lim_{x\to 0}{f}"
    );
    assert_eq!(
        read(&format!(
            r#"<m:acc><m:accPr><m:chr m:val="̃"/></m:accPr><m:e>{}</m:e></m:acc>"#,
            r("x")
        )),
        r"\tilde{x}"
    );
    assert_eq!(
        read(&format!(
            "<m:m><m:mr><m:e>{}</m:e><m:e>{}</m:e></m:mr></m:m>",
            r("1"),
            r("0")
        )),
        r"\begin{matrix} 1 & 0 \end{matrix}"
    );
    assert_eq!(
        read("<m:r><m:rPr><m:nor/></m:rPr><m:t>area</m:t></m:r>"),
        r"\text{area}"
    );
}

#[test]
fn writes_office_math_that_reads_back() {
    for latex in [
        r"\frac{a+b}{2}",
        "x_i^2",
        r"\sqrt[3]{x}+\sqrt{2}",
        r"\sum_{i=1}^n{i^2}",
        r"\left( a+b \right)",
        r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}",
        r"\hat{x}+\overline{AB}",
        r"\sin{x}+\lim_{x\to 0}{f}",
        r"\text{area}=\pi r^2",
        r"\int_0^1{x}\,dx",
    ] {
        let omml = math_to_omml(&parse_latex(latex), false);
        let root = parse_tree(
            &omml.replacen("<m:oMath>", &format!("<m:oMath {NAMESPACE}>"), 1),
            |namespace, name| namespace == Namespace::Math && name == "t",
        )
        .unwrap_or_else(|error| panic!("{latex}: {error:?} in {omml}"));
        assert_eq!(to_latex(&omml_to_math(&root)), latex, "{omml}");
    }
    let display = math_to_omml(&parse_latex("x"), true);
    assert!(display.starts_with("<m:oMathPara><m:oMath>"));
    let escaped = math_to_omml(&parse_latex(r"a<b \text{&}"), false);
    assert!(
        escaped.contains("a</m:t></m:r><m:r><m:t xml:space=\"preserve\">&lt;"),
        "{escaped}"
    );
    assert!(escaped.contains("&amp;"));
}
