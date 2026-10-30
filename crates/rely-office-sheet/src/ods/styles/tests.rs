// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use rely_office_core::{CellBorders, CellFont};

use super::*;

fn style(format: NumberFormat, code: Option<&str>) -> Option<String> {
    data_style("N1", format, code).map(|(_, markup)| markup)
}

#[test]
fn writes_a_data_style_for_each_number_format() {
    assert_eq!(style(NumberFormat::General, None), None);
    assert!(style(NumberFormat::Date, None).unwrap().starts_with(
        r#"<number:date-style style:name="N1"><number:day number:style="long"/><number:text>/</number:text>"#
    ));
    assert!(
        style(NumberFormat::DateTime, None)
            .unwrap()
            .contains("<number:minutes")
    );
    assert!(
        style(NumberFormat::Percent, None)
            .unwrap()
            .contains("percentage-style")
    );
    assert!(
        style(NumberFormat::Decimals(3), None)
            .unwrap()
            .contains(r#"number:decimal-places="3" number:min-decimal-places="3""#)
    );
    for code in ["General", "@", "[Red];;", ""] {
        assert_eq!(style(NumberFormat::General, Some(code)), None, "{code}");
    }
}

#[test]
fn turns_excel_codes_into_data_styles() {
    let markup = |code: &str| style(NumberFormat::General, Some(code)).unwrap();
    assert_eq!(
        markup("#,##0.00"),
        r#"<number:number-style style:name="N1"><number:number number:decimal-places="2" number:min-decimal-places="2" number:min-integer-digits="1" number:grouping="true"/></number:number-style>"#
    );
    assert_eq!(
        markup("\"€\" 0.0#;[Red]-0.0"),
        r#"<number:number-style style:name="N1"><number:text>€ </number:text><number:number number:decimal-places="2" number:min-decimal-places="1" number:min-integer-digits="1"/></number:number-style>"#
    );
    assert!(markup("0%").starts_with(
        r#"<number:percentage-style style:name="N1"><number:number number:decimal-places="0""#
    ));
    assert!(markup("0%").ends_with("<number:text>%</number:text></number:percentage-style>"));
    assert_eq!(
        markup("dd/mm/yyyy hh:mm:ss"),
        concat!(
            r#"<number:date-style style:name="N1"><number:day number:style="long"/><number:text>/</number:text>"#,
            r#"<number:month number:style="long"/><number:text>/</number:text><number:year number:style="long"/>"#,
            r#"<number:text> </number:text><number:hours number:style="long"/><number:text>:</number:text>"#,
            r#"<number:minutes number:style="long"/><number:text>:</number:text><number:seconds number:style="long"/></number:date-style>"#
        )
    );
    let textual = markup("dddd d mmmm yy");
    assert!(textual.contains(r#"<number:day-of-week number:style="long"/>"#));
    assert!(textual.contains(r#"<number:month number:style="long" number:textual="true"/>"#));
    assert!(textual.contains("<number:year/>"));
    let short = markup("ddd mmm h:m AM/PM");
    assert!(short.contains("<number:day-of-week/>"));
    assert!(short.contains(r#"<number:month number:textual="true"/>"#));
    assert!(short.contains(r#"<number:minutes number:style="short"/>"#));
    assert!(short.contains("<number:am-pm/>"));
    assert!(markup("m/d").contains(r#"<number:month number:style="short"/>"#));
    assert!(markup("\\x0_)*-").contains("<number:text>x</number:text>"));
}

#[test]
#[expect(clippy::too_many_lines, reason = "every part of a look")]
fn writes_the_look_of_a_cell_and_of_a_rule() {
    let mut xml = String::new();
    let look = CellStyle {
        font: CellFont {
            family: Some("Arial & co".to_owned()),
            size_tenths: Some(105),
            colour: Some("C00000".to_owned()),
            bold: true,
            italic: true,
            underline: true,
            strike: true,
        },
        fill: Some("fff2cc".to_owned()),
        borders: CellBorders {
            top: Some(Border {
                style: BorderStyle::Dashed,
                width_eighths: 4,
                colour: None,
            }),
            left: Some(Border {
                style: BorderStyle::None,
                width_eighths: 4,
                colour: None,
            }),
            right: Some(Border {
                style: BorderStyle::Dotted,
                width_eighths: 8,
                colour: Some("bad".to_owned()),
            }),
            bottom: Some(Border {
                style: BorderStyle::Single,
                width_eighths: 8,
                colour: Some("1F4E79".to_owned()),
            }),
        },
        horizontal: Some(HorizontalAlignment::Center),
        vertical: Some(VerticalAlignment::Top),
        wrap: true,
        indent: 2,
        number_format: None,
    };
    cell_style_xml(&mut xml, "ce1", &look, Some("N1"), "table-cell", false);
    for wanted in [
        r#"style:data-style-name="N1">"#,
        r##"fo:background-color="#fff2cc""##,
        r#"fo:border-top="0.5pt dashed #000000""#,
        r#"fo:border-left="none""#,
        r#"fo:border-right="1pt dotted #000000""#,
        r#"fo:border-bottom="1pt solid #1f4e79""#,
        r#"fo:wrap-option="wrap" style:vertical-align="top" style:text-align-source="fix""#,
        r#"<style:paragraph-properties fo:text-align="center" fo:margin-left="20pt"/>"#,
        r#"fo:font-family="Arial &amp; co" fo:font-size="10.5pt" fo:font-weight="bold" fo:font-style="italic""#,
        r##"style:text-line-through-style="solid" fo:color="#c00000""##,
    ] {
        assert!(xml.contains(wanted), "{wanted}\n{xml}");
    }
    for (horizontal, vertical, align, place) in [
        (
            HorizontalAlignment::Left,
            VerticalAlignment::Center,
            "start",
            "middle",
        ),
        (
            HorizontalAlignment::Right,
            VerticalAlignment::Bottom,
            "end",
            "bottom",
        ),
        (
            HorizontalAlignment::Justify,
            VerticalAlignment::Bottom,
            "justify",
            "bottom",
        ),
    ] {
        let mut xml = String::new();
        let style = CellStyle {
            horizontal: Some(horizontal),
            vertical: Some(vertical),
            ..CellStyle::default()
        };
        cell_style_xml(&mut xml, "ce2", &style, None, "table-cell", false);
        assert!(xml.contains(&format!(r#"fo:text-align="{align}""#)));
        assert!(xml.contains(&format!(r#"style:vertical-align="{place}""#)));
    }
    let rule = conditional_style_xml(
        "Condizione1",
        &DifferentialLook {
            fill: Some("FFC7CE".to_owned()),
            italic: Some(true),
            bold: Some(false),
            ..DifferentialLook::default()
        },
    );
    assert_eq!(
        rule,
        r##"<style:style style:name="Condizione1" style:family="table-cell" style:parent-style-name="Default"><style:table-cell-properties fo:background-color="#ffc7ce"/><style:text-properties fo:font-style="italic"/></style:style>"##
    );
    let mut empty = String::new();
    cell_style_xml(
        &mut empty,
        "ce3",
        &CellStyle::default(),
        None,
        "table-cell",
        false,
    );
    assert_eq!(
        empty,
        r#"<style:style style:name="ce3" style:family="table-cell" style:parent-style-name="Default"></style:style>"#
    );
    assert_eq!(colour("ABCDEF"), "abcdef");
}

#[test]
fn names_one_style_for_each_look_and_format_in_use() {
    let mut book = Workbook::new();
    book.set_input(
        0,
        crate::address::CellRef::new(0, 0),
        "1",
        crate::value::Locale::English,
    )
    .unwrap();
    book.set_format(0, crate::address::CellRef::new(0, 0), NumberFormat::Percent)
        .unwrap();
    book.set_input(
        0,
        crate::address::CellRef::new(1, 0),
        "2",
        crate::value::Locale::English,
    )
    .unwrap();
    let styles = OdsStyles::new(&book);
    assert_eq!(styles.name(0, NumberFormat::Percent), Some("ce1"));
    assert_eq!(
        styles.name(0, NumberFormat::General),
        None,
        "the default has no style"
    );
    assert!(
        styles
            .xml
            .contains(r#"<number:percentage-style style:name="N1">"#)
    );
    assert!(styles.xml.contains(r#"style:data-style-name="N1""#));
}
