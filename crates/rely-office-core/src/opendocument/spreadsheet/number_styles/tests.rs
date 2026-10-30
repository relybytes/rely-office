// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::xml_tree::parse_tree;

fn codes(styles: &str) -> HashMap<String, String> {
    let xml = format!(
        concat!(
            r#"<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
            r#"xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" "#,
            r#"xmlns:number="urn:oasis:names:tc:opendocument:xmlns:datastyle:1.0">"#,
            "<office:automatic-styles>{}</office:automatic-styles><office:styles>",
            r#"<number:text-style style:name="T"><number:text-content/></number:text-style></office:styles></office:document-content>"#
        ),
        styles
    );
    let root = parse_tree(&xml, |namespace, name| {
        namespace == Namespace::OdfNumber && name == "text"
    })
    .unwrap();
    number_format_codes(std::iter::once(&root))
}

#[test]
fn turns_number_percentage_and_currency_styles_into_codes() {
    let found = codes(concat!(
        r#"<number:number-style style:name="N0"><number:number number:min-integer-digits="1"/></number:number-style>"#,
        r#"<number:number-style style:name="N2"><number:number number:decimal-places="2" number:min-decimal-places="1" number:grouping="true"/></number:number-style>"#,
        r#"<number:number-style style:name="N3"><number:number number:decimal-places="1" number:min-integer-digits="0"/></number:number-style>"#,
        r#"<number:number-style style:name="N4"><number:number number:min-integer-digits="0" number:grouping="true"/></number:number-style>"#,
        r#"<number:percentage-style style:name="P"><number:number number:decimal-places="1"/><number:text>%</number:text></number:percentage-style>"#,
        r#"<number:currency-style style:name="C"><number:currency-symbol>€</number:currency-symbol><number:text> </number:text>"#,
        r#"<number:number number:decimal-places="2" number:grouping="true"/></number:currency-style>"#,
        r#"<number:number-style style:name="E"><number:scientific-number number:decimal-places="2" number:min-exponent-digits="2"/></number:number-style>"#,
        r#"<number:number-style style:name="F"><number:fraction number:min-denominator-digits="2"/></number:number-style>"#,
        r#"<number:number-style style:name="L"><number:text>"tot."</number:text><number:number/></number:number-style>"#,
        r#"<number:boolean-style style:name="B"><number:boolean/></number:boolean-style>"#,
        r#"<number:number-style style:name="Empty"/>"#,
        r#"<number:number-style><number:number/></number:number-style>"#,
    ));
    let code = |name: &str| found.get(name).map(String::as_str);
    assert_eq!(code("N0"), Some("0"));
    assert_eq!(code("N2"), Some("#,##0.0#"));
    assert_eq!(code("N3"), Some("#.0"));
    assert_eq!(code("N4"), Some("#,###"));
    assert_eq!(code("P"), Some("0.0%"));
    assert_eq!(code("C"), Some("\"€\" #,##0.00"));
    assert_eq!(code("E"), Some("0.00E+00"));
    assert_eq!(code("F"), Some("# ??/??"));
    assert_eq!(
        code("L"),
        Some("\"tot.\"0"),
        "quotes dropped, the text quoted"
    );
    assert_eq!(code("T"), Some("@"));
    assert_eq!(code("B"), None);
    assert_eq!(code("Empty"), None);
    assert_eq!(found.len(), 10);
}

#[test]
fn turns_date_and_time_styles_into_codes() {
    let found = codes(concat!(
        r#"<number:date-style style:name="D1"><number:day number:style="long"/><number:text>/</number:text>"#,
        r#"<number:month number:style="long"/><number:text>/</number:text><number:year number:style="long"/></number:date-style>"#,
        r#"<number:date-style style:name="D2"><number:day-of-week number:style="long"/><number:text> </number:text><number:day/>"#,
        r#"<number:text> </number:text><number:month number:textual="true" number:style="long"/><number:text> </number:text><number:year/></number:date-style>"#,
        r#"<number:date-style style:name="D3"><number:day-of-week/><number:month number:textual="true"/><number:month/></number:date-style>"#,
        r#"<number:time-style style:name="H1"><number:hours number:style="long"/><number:text>:</number:text><number:minutes number:style="long"/>"#,
        r#"<number:text>:</number:text><number:seconds number:style="long" number:decimal-places="2"/></number:time-style>"#,
        r#"<number:time-style style:name="H2"><number:hours/><number:text>.</number:text><number:minutes/><number:seconds/>"#,
        r#"<number:text> </number:text><number:am-pm/></number:time-style>"#,
    ));
    let code = |name: &str| found.get(name).map(String::as_str);
    assert_eq!(code("D1"), Some("dd/mm/yyyy"));
    assert_eq!(code("D2"), Some("dddd d mmmm yy"));
    assert_eq!(code("D3"), Some("dddmmmm"));
    assert_eq!(code("H1"), Some("hh:mm:ss.00"));
    assert_eq!(code("H2"), Some("h.ms AM/PM"));
}

#[test]
fn keeps_literal_text_bare_only_when_excel_shows_it_as_it_is() {
    assert_eq!(literal(""), "");
    assert_eq!(literal(" - "), " - ");
    assert_eq!(literal("\u{a0}%"), " %");
    assert_eq!(literal("kg"), "\"kg\"");
    assert_eq!(literal("\"x\""), "\"x\"");
}
