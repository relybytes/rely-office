// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;
use crate::xml_tree::parse_tree;

const NAMESPACES: &str = concat!(
    r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
    r#"xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" "#,
    r#"xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" "#,
    r#"xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" "#,
    r#"xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0""#
);

fn root(inner: &str) -> Element {
    parse_tree(
        &format!(r"<office:document {NAMESPACES}>{inner}</office:document>"),
        |_, _| false,
    )
    .unwrap()
}

fn properties(xml: &str) -> Element {
    root(xml).elements().next().unwrap().clone()
}

#[test]
fn resolves_a_style_through_its_parents_to_the_default() {
    let styles = root(concat!(
        "<office:styles>",
        r##"<style:default-style style:family="paragraph"><style:text-properties fo:font-size="10pt" fo:color="#112233"/></style:default-style>"##,
        r#"<style:style style:name="Base" style:family="paragraph"><style:text-properties fo:font-size="12pt"/></style:style>"#,
        r#"<style:style style:name="Loop" style:family="paragraph" style:parent-style-name="Loop"/>"#,
        r#"<style:style style:name="Same" style:family="text"><style:text-properties fo:font-weight="bold"/></style:style>"#,
        r#"<style:style style:name="Same" style:family="paragraph" style:parent-style-name="Base" style:master-page-name="Landscape" style:list-style-name="L"/>"#,
        r#"<text:list-style style:name="L"/>"#,
        "</office:styles>",
        r#"<office:automatic-styles><style:style style:name="P" style:family="paragraph" style:parent-style-name="Same"/></office:automatic-styles>"#
    ));
    let content = root(concat!(
        r#"<office:automatic-styles><style:style style:name="P" style:family="paragraph" style:parent-style-name="Base">"#,
        r#"<style:text-properties fo:font-style="italic"/></style:style><text:list-style style:name="L"/></office:automatic-styles>"#
    ));
    let styles = Styles::read(Some(&content), Some(&styles));
    assert_eq!(
        styles.chain(Scope::Body, "paragraph", Some("P")).len(),
        3,
        "P, Base, the default"
    );
    assert_eq!(
        styles.chain(Scope::Master, "paragraph", Some("P")).len(),
        4,
        "the master's P is another style"
    );
    assert_eq!(
        styles.chain(Scope::Body, "paragraph", Some("Loop")).len(),
        2,
        "a style that is its own parent"
    );
    assert_eq!(
        styles
            .chain(Scope::Body, "paragraph", Some("Assente"))
            .len(),
        1
    );
    let (look, _) = styles.text_look(Scope::Body, "paragraph", Some("P"));
    assert_eq!(
        (look.size_half_points, look.italic, look.colour.as_deref()),
        (Some(24), Some(true), Some("112233"))
    );
    let (bold, _) = styles.text_look(Scope::Body, "text", Some("Same"));
    assert_eq!(
        bold.bold,
        Some(true),
        "a text and a paragraph style may share a name"
    );
    assert_eq!(
        styles.master_page_of(Scope::Master, Some("P")).as_deref(),
        Some("Landscape")
    );
    assert_eq!(
        styles.list_style_of(Scope::Master, Some("P")).as_deref(),
        Some("L")
    );
    assert!(styles.list_style(Scope::Body, "L").is_some());
    assert!(
        styles.list_style(Scope::Master, "L").is_some(),
        "the named one"
    );
    assert!(styles.list_style(Scope::Body, "X").is_none());
    assert_eq!(
        Styles::read(None, None)
            .chain(Scope::Body, "paragraph", None)
            .len(),
        0
    );
}

#[test]
fn reads_text_properties() {
    let styles = Styles::read(
        None,
        Some(&root(
            r#"<office:font-face-decls><style:font-face style:name="F1" svg:font-family="'Times New Roman'"/></office:font-face-decls>"#,
        )),
    );
    let read = |xml: &str| styles.text_properties(&properties(xml));
    let (look, flags) = read(concat!(
        r#"<style:text-properties style:font-name="F1" fo:font-size="10.5pt" fo:font-weight="600" fo:font-style="oblique" "#,
        r##"fo:color="#aabbcc" fo:background-color="transparent" style:text-underline-style="none" "##,
        r#"style:text-line-through-style="solid" style:text-position="33% 58%" fo:text-transform="uppercase" fo:font-variant="small-caps"/>"#
    ));
    assert_eq!(look.family.as_deref(), Some("Times New Roman"));
    assert_eq!(look.size_half_points, Some(21));
    assert_eq!((look.bold, look.italic), (Some(true), Some(true)));
    assert_eq!(
        (look.colour.as_deref(), look.highlight.as_deref()),
        (Some("AABBCC"), None)
    );
    assert_eq!((look.caps, look.small_caps), (Some(true), Some(true)));
    assert_eq!(
        flags,
        TextFlags {
            underline: Some(false),
            strike: Some(true),
            raised: Some(true),
            code: None,
        }
    );
    let (look, flags) = read(
        r#"<style:text-properties style:font-name="Unknown" fo:font-weight="normal" fo:font-size="120%" style:text-position="0% 100%"/>"#,
    );
    assert_eq!(look.family.as_deref(), Some("Unknown"));
    assert_eq!(
        (look.bold, look.size_half_points, flags.raised),
        (Some(false), None, None)
    );
    let (look, flags) =
        read(r#"<style:text-properties fo:font-family="'Arial'" style:text-position="sub"/>"#);
    assert_eq!(
        (look.family.as_deref(), flags.raised),
        (Some("Arial"), Some(false))
    );
    assert_eq!(
        TextFlags {
            underline: None,
            strike: Some(false),
            raised: None,
            code: None,
        }
        .or(flags),
        TextFlags {
            underline: None,
            strike: Some(false),
            raised: Some(false),
            code: None,
        }
    );
}

#[test]
fn reads_paragraph_look_of() {
    let (look, align) = paragraph_look_of(&properties(concat!(
        r#"<style:paragraph-properties fo:text-align="center" fo:margin-top="-1cm" fo:line-height="0.5cm" "#,
        r#"fo:border-left="1pt dashed #0000ff" fo:border-bottom="2pt dotted"><style:tab-stops>"#,
        r#"<style:tab-stop style:position="3cm" style:type="char"/><style:tab-stop style:position="1cm" style:type="center" style:leader-style="dotted"/>"#,
        r#"<style:tab-stop style:position="2cm" style:leader-text="-"/><style:tab-stop/></style:tab-stops></style:paragraph-properties>"#
    )));
    assert_eq!(align, Some(Alignment::Center));
    assert_eq!(
        look.space_before_twips,
        Some(0),
        "a negative margin is no spacing"
    );
    assert_eq!(look.line_spacing, Some(LineSpacing::Exact(283)));
    assert_eq!(
        look.borders
            .left
            .as_ref()
            .map(|border| (border.style, border.colour.clone())),
        Some((BorderStyle::Dashed, Some("0000FF".to_owned())))
    );
    assert_eq!(
        look.borders
            .bottom
            .as_ref()
            .map(|border| (border.style, border.width_eighths)),
        Some((BorderStyle::Dotted, 16))
    );
    assert_eq!(look.borders.top, None);
    let kinds: Vec<(i32, TabKind, TabLeader)> = look
        .tabs
        .iter()
        .map(|tab| (tab.position_twips, tab.kind, tab.leader))
        .collect();
    assert_eq!(
        kinds,
        [
            (567, TabKind::Center, TabLeader::Dot),
            (1_134, TabKind::Left, TabLeader::Hyphen),
            (1_701, TabKind::Decimal, TabLeader::None)
        ]
    );
    let (look, align) = paragraph_look_of(&properties(
        r#"<style:paragraph-properties fo:text-align="left" style:line-height-at-least="0.4cm" fo:break-before="column"/>"#,
    ));
    assert_eq!(
        (align, look.line_spacing, look.page_break_before),
        (
            Some(Alignment::Left),
            Some(LineSpacing::AtLeast(227)),
            false
        )
    );
    for (value, expected) in [
        ("end", Some(Alignment::Right)),
        ("right", Some(Alignment::Right)),
        ("justify", Some(Alignment::Justify)),
        ("start", Some(Alignment::Left)),
        ("inherit", None),
    ] {
        let (_, align) = paragraph_look_of(&properties(&format!(
            r#"<style:paragraph-properties fo:text-align="{value}"/>"#
        )));
        assert_eq!(align, expected, "{value}");
    }
    let (look, _) = paragraph_look_of(&properties(
        r#"<style:paragraph-properties fo:line-height="normal"/>"#,
    ));
    assert_eq!(look.line_spacing, None);
}

#[test]
fn reads_lengths_colours_borders_and_alignments() {
    let close = |value: Option<f64>, expected: f64| {
        assert!((value.unwrap() - expected).abs() < 0.01, "{value:?}");
    };
    close(points("2.54cm"), 72.0);
    close(points("25.4mm"), 72.0);
    close(points("1in"), 72.0);
    close(points("1inch"), 72.0);
    close(points("1pc"), 12.0);
    close(points("96px"), 72.0);
    close(points(" 12pt "), 12.0);
    assert_eq!(points("12"), None);
    assert_eq!(points("12em"), None);
    assert_eq!(points("abc"), None);
    assert_eq!(twips("1in"), Some(1_440));
    assert_eq!(twips("-1in"), Some(-1_440));
    assert_eq!(twips("99999in"), Some(63_360), "clamped");
    assert_eq!(colour("#a1b2c3").as_deref(), Some("A1B2C3"));
    assert_eq!(colour("transparent"), None);
    assert_eq!(colour("#123"), None);
    assert_eq!(border("none").style, BorderStyle::None);
    assert_eq!(
        border("0.06pt solid #000000").width_eighths,
        1,
        "at least an eighth"
    );
    assert_eq!(
        border("solid").width_eighths,
        6,
        "three quarters of a point by default"
    );
    assert_eq!(border("1pt hidden").style, BorderStyle::None);
    assert_eq!(vertical_alignment("top"), Some(VerticalAlignment::Top));
    assert_eq!(
        vertical_alignment("bottom"),
        Some(VerticalAlignment::Bottom)
    );
    assert_eq!(vertical_alignment("automatic"), None);
    assert_eq!(unquote(" \"Liberation Serif\" "), "Liberation Serif");
}
