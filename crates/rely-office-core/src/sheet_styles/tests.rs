// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;

const STYLES: &str = concat!(
    r#"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#,
    r##"<numFmts count="1"><numFmt numFmtId="164" formatCode="#,##0.00 &quot;€&quot;"/></numFmts>"##,
    r#"<fonts count="3"><font><sz val="11"/><color theme="1"/><name val="Calibri"/></font>"#,
    r#"<font><b/><i val="0"/><u/><strike/><sz val="14"/><color rgb="FFC00000"/><name val="Arial"/></font>"#,
    r#"<font><u val="none"/><sz val="999"/><color theme="4" tint="0.39997558519241921"/></font></fonts>"#,
    r#"<fills count="4"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill>"#,
    r#"<fill><patternFill patternType="solid"><fgColor indexed="13"/><bgColor indexed="64"/></patternFill></fill>"#,
    r#"<fill><gradientFill><stop position="0"><color theme="5" tint="-0.5"/></stop></gradientFill></fill></fills>"#,
    r#"<borders count="2"><border><left/><right/><top/><bottom/><diagonal/></border>"#,
    r#"<border><left style="thin"><color auto="1"/></left><right style="medium"><color rgb="FF0000FF"/></right>"#,
    r#"<top style="double"/><bottom style="dashed"><color indexed="10"/></bottom><diagonal style="thick"/></border></borders>"#,
    r#"<cellStyleXfs count="1"><xf numFmtId="0" fontId="1" fillId="2" borderId="1"/></cellStyleXfs>"#,
    r#"<cellXfs count="4"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/>"#,
    r#"<xf numFmtId="164" fontId="1" fillId="2" borderId="1"><alignment horizontal="center" vertical="top" wrapText="1"/></xf>"#,
    r#"<xf numFmtId="14" fontId="2" fillId="3" borderId="9"><alignment horizontal="justify" vertical="center"/></xf>"#,
    r#"<xf numFmtId="999" fontId="99"><alignment horizontal="general" vertical="bottom"/></xf></cellXfs>"#,
    r#"<dxfs count="1"><dxf><font><b/></font><fill><patternFill><bgColor rgb="FF00FF00"/></patternFill></fill></dxf></dxfs>"#,
    "</styleSheet>",
);

#[test]
fn resolves_each_cell_format_into_a_style() {
    let styles = cell_styles(STYLES, &default_theme());
    assert_eq!(styles.len(), 4);
    assert_eq!(
        styles[0],
        CellStyle {
            font: CellFont {
                family: Some("Calibri".to_owned()),
                size_tenths: Some(110),
                ..CellFont::default()
            },
            ..CellStyle::default()
        },
        "the dark theme colour of text is automatic"
    );
    let formatted = &styles[1];
    assert_eq!(
        formatted.font,
        CellFont {
            family: Some("Arial".to_owned()),
            size_tenths: Some(140),
            colour: Some("C00000".to_owned()),
            bold: true,
            italic: false,
            underline: true,
            strike: true,
        }
    );
    assert_eq!(formatted.fill.as_deref(), Some("FFFF00"));
    assert_eq!(
        formatted.borders.left,
        Some(Border {
            style: BorderStyle::Single,
            width_eighths: 4,
            colour: None
        })
    );
    assert_eq!(
        formatted.borders.right,
        Some(Border {
            style: BorderStyle::Single,
            width_eighths: 8,
            colour: Some("0000FF".to_owned())
        })
    );
    assert_eq!(
        formatted.borders.top.as_ref().map(|edge| edge.style),
        Some(BorderStyle::Double)
    );
    assert_eq!(
        formatted
            .borders
            .bottom
            .as_ref()
            .map(|edge| (edge.style, edge.colour.clone())),
        Some((BorderStyle::Dashed, Some("FF0000".to_owned())))
    );
    assert!(!formatted.borders.is_empty());
    assert_eq!(formatted.horizontal, Some(HorizontalAlignment::Center));
    assert_eq!(formatted.vertical, Some(VerticalAlignment::Top));
    assert!(formatted.wrap);
    assert_eq!(formatted.number_format.as_deref(), Some("#,##0.00 \"€\""));

    let tinted_style = &styles[2];
    assert_eq!(tinted_style.font.colour.as_deref(), Some("8FAADC"));
    assert_eq!(tinted_style.font.size_tenths, None);
    assert!(!tinted_style.font.underline);
    assert_eq!(tinted_style.fill.as_deref(), Some("843C0B"));
    assert_eq!(tinted_style.borders, CellBorders::default());
    assert!(tinted_style.borders.is_empty());
    assert_eq!(tinted_style.horizontal, Some(HorizontalAlignment::Justify));
    assert_eq!(tinted_style.vertical, Some(VerticalAlignment::Center));
    assert_eq!(tinted_style.number_format.as_deref(), Some("m/d/yyyy"));

    let unknown = &styles[3];
    assert_eq!(unknown.font, CellFont::default());
    assert_eq!(unknown.number_format, None);
    assert_eq!(unknown.horizontal, None);
    assert_eq!(unknown.vertical, Some(VerticalAlignment::Bottom));
}

#[test]
fn reads_the_colours_of_a_theme() {
    let theme = theme_colours(concat!(
        r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:themeElements>"#,
        r#"<a:clrScheme name="Rely"><a:dk1><a:sysClr val="windowText" lastClr="111111"/></a:dk1>"#,
        r#"<a:lt1><a:sysClr val="window" lastClr="FEFEFE"/></a:lt1><a:dk2><a:srgbClr val="222222"/></a:dk2>"#,
        r#"<a:accent1><a:srgbClr val="ff6600"/></a:accent1></a:clrScheme></a:themeElements></a:theme>"#
    ))
    .unwrap();
    assert_eq!(theme[0], "FEFEFE");
    assert_eq!(theme[1], "111111");
    assert_eq!(theme[3], "222222");
    assert_eq!(theme[4], "FF6600");
    assert_eq!(theme[5], default_theme()[5]);
    assert_eq!(theme_colours("<a:theme/>"), None);
    assert_eq!(theme_colours("<broken"), None);
}

#[test]
fn tints_colours_on_their_luminance() {
    assert_eq!(tinted("4472C4", 0.0), "4472C4");
    assert_eq!(tinted("000000", 0.5), "808080");
    assert_eq!(tinted("FFFFFF", -0.5), "808080");
    assert_eq!(tinted("FF0000", 1.0), "FFFFFF");
    assert_eq!(tinted("808080", -1.0), "000000");
    assert_eq!(hex("FF123456"), Some("123456".to_owned()));
    assert_eq!(hex("abcdef"), Some("ABCDEF".to_owned()));
    assert_eq!(hex("xyz"), None);
}

#[test]
fn reads_damaged_styles_as_far_as_they_go() {
    assert_eq!(cell_styles("", &default_theme()), Vec::new());
    assert_eq!(
        cell_styles("<styleSheet><cellXfs><xf fontId=\"3\"", &default_theme()),
        Vec::new()
    );
    let empty_lists = cell_styles(
        "<styleSheet><fonts/><fills/><borders/><cellXfs><xf/></cellXfs></styleSheet>",
        &default_theme(),
    );
    assert_eq!(empty_lists, vec![CellStyle::default()]);
}

#[test]
fn reads_the_differential_formats_of_conditional_rules() {
    let looks = differential_looks(
        concat!(
            r#"<styleSheet><numFmts count="1"><numFmt numFmtId="164" formatCode="0.0"/></numFmts>"#,
            r#"<cellXfs count="1"><xf numFmtId="164"/></cellXfs><dxfs count="3">"#,
            r#"<dxf><font><b/><i val="0"/><u val="none"/><strike/><color rgb="FF9C0006"/></font><numFmt numFmtId="300" formatCode="0%"/><fill><patternFill><bgColor rgb="FFFFC7CE"/></patternFill></fill></dxf>"#,
            r#"<dxf><fill><patternFill patternType="solid"><fgColor theme="4"/></patternFill></fill><font><u/></font></dxf>"#,
            r#"<dxf/></dxfs></styleSheet>"#
        ),
        &default_theme(),
    );
    assert_eq!(looks.len(), 3);
    assert_eq!(
        looks[0],
        DifferentialLook {
            fill: Some("FFC7CE".to_owned()),
            font_colour: Some("9C0006".to_owned()),
            bold: Some(true),
            italic: Some(false),
            underline: Some(false),
            strike: Some(true),
            number_format: Some("0%".to_owned()),
        }
    );
    assert_eq!(looks[1].fill, Some(default_theme()[4].clone()));
    assert_eq!(looks[1].underline, Some(true));
    assert!(looks[2].is_empty());
    let styles = cell_styles(
        r#"<styleSheet><cellXfs count="1"><xf numFmtId="0"/></cellXfs><dxfs count="1"><dxf><numFmt numFmtId="0" formatCode="0.00"/></dxf></dxfs></styleSheet>"#,
        &default_theme(),
    );
    assert_eq!(
        styles[0].number_format, None,
        "a differential number format is no cell format's"
    );
}
