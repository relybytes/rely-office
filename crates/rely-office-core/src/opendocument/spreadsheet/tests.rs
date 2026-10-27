// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;
use crate::document::{Border, BorderStyle, VerticalAlignment};
use crate::test_support::build_zip;

const NAMESPACES: &str = concat!(
    r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
    r#"xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" "#,
    r#"xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" "#,
    r#"xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" "#,
    r#"xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" "#,
    r#"xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" "#,
    r#"xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" "#,
    r#"xmlns:number="urn:oasis:names:tc:opendocument:xmlns:datastyle:1.0" "#,
    r#"xmlns:xlink="http://www.w3.org/1999/xlink""#
);
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x02\0\0\0\x01";

const STYLES: &str = concat!(
    "<office:styles>",
    r#"<style:default-style style:family="table-cell"><style:text-properties style:font-name="Liberation Sans" fo:font-size="10pt"/></style:default-style>"#,
    r#"<style:style style:name="Default" style:family="table-cell"/>"#,
    "</office:styles>",
    "<office:automatic-styles>",
    r#"<style:page-layout style:name="pm1"><style:page-layout-properties fo:page-width="29.7cm" fo:page-height="21cm" "#,
    r#"fo:margin-top="1in" fo:margin-bottom="0.5in" fo:margin-left="2.54cm" fo:margin-right="72pt" style:print="grid headers"/></style:page-layout>"#,
    "</office:automatic-styles>",
    r#"<office:master-styles><style:master-page style:name="Default" style:page-layout-name="pm1">"#,
    r#"<style:header><style:region-left><text:p>Offerta &amp; listino</text:p></style:region-left>"#,
    r#"<style:region-right><text:p>Foglio <text:sheet-name>?</text:sheet-name></text:p></style:region-right></style:header>"#,
    r#"<style:footer><text:p>Pagina <text:page-number>1</text:page-number> di <text:page-count>2</text:page-count></text:p></style:footer>"#,
    r#"<style:footer-left style:display="false"/>"#,
    "</style:master-page></office:master-styles>"
);

const AUTOMATIC: &str = concat!(
    "<office:automatic-styles>",
    r#"<number:currency-style style:name="N1"><number:number number:decimal-places="2" number:min-integer-digits="1" number:grouping="true"/>"#,
    r#"<number:text> </number:text><number:currency-symbol>€</number:currency-symbol></number:currency-style>"#,
    r#"<style:style style:name="co1" style:family="table-column"><style:table-column-properties style:column-width="64pt"/></style:style>"#,
    r#"<style:style style:name="ro1" style:family="table-row"><style:table-row-properties style:row-height="0.5in"/></style:style>"#,
    r#"<style:style style:name="ta1" style:family="table" style:master-page-name="Default"/>"#,
    r##"<style:style style:name="ce1" style:family="table-cell" style:parent-style-name="Default" style:data-style-name="N1"><style:table-cell-properties fo:background-color="#ffff00" "##,
    r##"fo:border="0.06pt solid #000000" fo:border-bottom="2.5pt double #ff0000" style:vertical-align="middle" fo:wrap-option="wrap"/>"##,
    r##"<style:paragraph-properties fo:text-align="center"/><style:text-properties fo:font-weight="bold" fo:font-style="italic" fo:color="#1f3864" "##,
    r#"style:text-underline-style="solid" style:text-line-through-style="solid" fo:font-size="14pt"/></style:style>"#,
    r#"<style:style style:name="ce2" style:family="table-cell" style:parent-style-name="Default"><style:paragraph-properties fo:text-align="end"/></style:style>"#,
    r#"<style:style style:name="ce3" style:family="table-cell" style:parent-style-name="Default"/>"#,
    "</office:automatic-styles>"
);

fn ods(table: &str) -> Vec<u8> {
    let content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content {NAMESPACES}>{AUTOMATIC}<office:body><office:spreadsheet>{table}</office:spreadsheet></office:body></office:document-content>"#
    );
    let styles = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-styles {NAMESPACES}>{STYLES}</office:document-styles>"#
    );
    build_zip(&[
        (
            "mimetype",
            b"application/vnd.oasis.opendocument.spreadsheet",
        ),
        ("content.xml", content.as_bytes()),
        ("styles.xml", styles.as_bytes()),
        ("Pictures/logo.png", PNG),
        ("Object 1/content.xml", CHART.as_bytes()),
    ])
}

const CHART: &str = concat!(
    r#"<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
    r#"xmlns:chart="urn:oasis:names:tc:opendocument:xmlns:chart:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0">"#,
    r#"<office:body><office:chart><chart:chart chart:class="chart:bar"><chart:plot-area>"#,
    r#"<chart:series chart:values-cell-range-address="local-table.B2:B3"/></chart:plot-area>"#,
    r#"<table:table table:name="local-table"><table:table-rows><table:table-row><table:table-cell/><table:table-cell/></table:table-row>"#,
    r#"<table:table-row><table:table-cell/><table:table-cell office:value="3"/></table:table-row>"#,
    r#"<table:table-row><table:table-cell/><table:table-cell office:value="5"/></table:table-row></table:table-rows></table:table>"#,
    r#"</chart:chart></office:chart></office:body></office:document-content>"#
);

const TABLE: &str = concat!(
    r#"<table:table table:name="Listino" table:style-name="ta1">"#,
    r#"<table:shapes><draw:frame svg:x="2cm" svg:y="-1cm" svg:width="2cm" svg:height="1cm"><draw:image xlink:href="Pictures/logo.png"/></draw:frame></table:shapes>"#,
    r#"<table:table-column table:style-name="co1" table:number-columns-repeated="2" table:default-cell-style-name="ce2"/>"#,
    r#"<table:table-column-group><table:table-column table:visibility="collapse"/></table:table-column-group>"#,
    r#"<table:table-column table:number-columns-repeated="16384"/>"#,
    r#"<table:table-row table:style-name="ro1" table:default-cell-style-name="ce3">"#,
    r#"<table:table-cell table:style-name="ce1" table:number-columns-spanned="2" table:number-rows-spanned="2" office:value="12"/>"#,
    r#"<table:covered-table-cell/><table:table-cell table:number-columns-repeated="3"/>"#,
    r#"<table:table-cell><draw:frame svg:width="5cm" svg:height="3cm"><draw:object xlink:href="./Object 1"/></draw:frame></table:table-cell></table:table-row>"#,
    r#"<table:table-row-group><table:table-row table:number-rows-repeated="2"><table:covered-table-cell table:number-columns-repeated="2"/>"#,
    r#"<table:table-cell table:style-name="ce1" table:number-columns-repeated="2"/>"#,
    r#"<table:table-cell><draw:frame svg:width="1cm" svg:height="1cm"><draw:image xlink:href="Pictures/missing.png"/></draw:frame></table:table-cell>"#,
    r#"<table:table-cell><draw:frame svg:width="0cm" svg:height="1cm"><draw:image xlink:href="Pictures/logo.png"/></draw:frame></table:table-cell>"#,
    r#"</table:table-row></table:table-row-group>"#,
    r#"<table:table-row table:visibility="collapse"><table:table-cell/></table:table-row>"#,
    r#"<table:table-row table:number-rows-repeated="1048570"><table:table-cell table:number-columns-repeated="1024"/></table:table-row>"#,
    "</table:table>",
    r#"<table:table table:name="Vuoto"/>"#
);

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one sheet carries every part of the layout"
)]
fn reads_the_layout_of_each_sheet() {
    let layouts = read_ods_layouts(&ods(TABLE), &Limits::default());
    assert_eq!(layouts.len(), 2);
    let empty = &layouts["Vuoto"];
    assert!(empty.styles.is_empty() && empty.drawings.is_empty());
    let layout = &layouts["Listino"];

    assert!(layout.landscape);
    assert_eq!(
        layout.margins_mils,
        Some([1_000, 1_000, 500, 1_000, 300, 300])
    );
    assert!(layout.grid_lines);
    assert_eq!(
        layout.header.as_deref(),
        Some("&LOfferta && listino&RFoglio &A")
    );
    assert_eq!(layout.footer.as_deref(), Some("&CPagina &P di &N"));
    assert_eq!(layout.font_family.as_deref(), Some("Liberation Sans"));
    assert_eq!(layout.font_size_tenths, Some(100));

    let width = width_of_points(64.0);
    assert_eq!(
        layout.column_widths[..4],
        [Some(width), Some(width), Some(0), None]
    );
    assert_eq!(
        layout.column_widths.len(),
        Limits::default().max_columns,
        "the columns past the shown ones are left out"
    );
    assert_eq!(layout.row_heights.get(&0), Some(&720));
    assert_eq!(layout.row_heights.get(&3), Some(&0), "a hidden row");
    assert_eq!(layout.row_heights.get(&1), None);

    let bold = layout.style_of(0, 0).expect("the first cell's style");
    assert_eq!(
        bold,
        &CellStyle {
            font: CellFont {
                family: Some("Liberation Sans".to_owned()),
                size_tenths: Some(140),
                colour: Some("1F3864".to_owned()),
                bold: true,
                italic: true,
                underline: true,
                strike: true,
            },
            fill: Some("FFFF00".to_owned()),
            borders: CellBorders {
                top: Some(Border {
                    style: BorderStyle::Single,
                    width_eighths: 1,
                    colour: Some("000000".to_owned())
                }),
                right: Some(Border {
                    style: BorderStyle::Single,
                    width_eighths: 1,
                    colour: Some("000000".to_owned())
                }),
                bottom: Some(Border {
                    style: BorderStyle::Double,
                    width_eighths: 20,
                    colour: Some("FF0000".to_owned())
                }),
                left: Some(Border {
                    style: BorderStyle::Single,
                    width_eighths: 1,
                    colour: Some("000000".to_owned())
                }),
            },
            horizontal: Some(HorizontalAlignment::Center),
            vertical: Some(VerticalAlignment::Center),
            wrap: true,
            number_format: Some("#,##0.00 \"€\"".to_owned()),
        }
    );
    assert_eq!(
        layout.style_of(1, 3),
        Some(bold),
        "a repeated row repeats its styled cells"
    );
    assert_eq!(layout.style_of(2, 2), Some(bold));
    assert_eq!(
        layout
            .style_of(0, 2)
            .map(|style| (style.fill.as_deref(), style.font.size_tenths)),
        Some((None, Some(100))),
        "the row's default style, with the font of the default style"
    );
    assert_eq!(
        layout.style_of(9, 1).and_then(|style| style.horizontal),
        Some(HorizontalAlignment::Right),
        "the column's default style"
    );
    assert_eq!(
        layout.merges,
        vec![CellRange {
            first_row: 0,
            first_column: 0,
            last_row: 1,
            last_column: 1,
        }]
    );

    assert_eq!(
        layout.drawings.len(),
        2,
        "a missing and an empty picture left out"
    );
    let chart = &layout.drawings[0];
    assert_eq!(
        (chart.row, chart.column, chart.content_type),
        (0, 5, "image/svg+xml")
    );
    assert_eq!(chart.offset_emu, (0, 0));
    assert_eq!((chart.width_emu, chart.height_emu), (1_800_000, 1_080_000));
    let logo = &layout.drawings[1];
    assert_eq!((logo.row, logo.column), (0, 0));
    assert_eq!(
        logo.offset_emu,
        (720_000, 0),
        "on the sheet, from its corner"
    );
    assert_eq!(logo.content_type, "image/png");
}

#[test]
fn caps_what_a_sheet_keeps_and_reads_no_layout_from_what_is_not_a_sheet() {
    let limits = Limits {
        max_rows: 1,
        max_columns: 1,
        max_images: 0,
        ..Limits::default()
    };
    let layouts = read_ods_layouts(&ods(TABLE), &limits);
    let layout = &layouts["Listino"];
    assert_eq!(layout.column_widths.len(), 1);
    assert_eq!(layout.row_heights.len(), 1);
    assert_eq!(layout.cell_styles.len(), 1);
    assert_eq!(
        layout.drawings.len(),
        1,
        "a chart is drawn, not an image read"
    );
    assert!(read_ods_layouts(b"not a zip", &Limits::default()).is_empty());
    let text = build_zip(&[(
        "content.xml",
        br#"<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"/>"#.as_slice(),
    )]);
    assert!(read_ods_layouts(&text, &Limits::default()).is_empty());
    assert!(
        read_ods_layouts(
            &build_zip(&[("mimetype", b"x".as_slice())]),
            &Limits::default()
        )
        .is_empty()
    );
}

#[test]
fn reads_repeats_widths_and_headers() {
    let element = |xml: &str| {
        parse_tree(&format!("<root {NAMESPACES}>{xml}</root>"), |_, _| false)
            .unwrap()
            .elements()
            .next()
            .unwrap()
            .clone()
    };
    let column = element(r#"<table:table-column table:number-columns-repeated="0"/>"#);
    assert_eq!(repeat(&column, "number-columns-repeated"), 1);
    assert_eq!(repeat(&column, "number-rows-repeated"), 1);
    assert_eq!(width_of_points(0.0), 0);
    assert_eq!(width_of_points(48.0), 2_323);
    assert_eq!(width_of_points(1.0e9), 65_280);
    assert_eq!(
        header_codes(&element(
            r#"<style:header style:display="false"><text:p>x</text:p></style:header>"#
        )),
        None
    );
    assert_eq!(
        header_codes(&element("<style:header><text:p/><text:p/></style:header>")),
        None
    );
    assert_eq!(
        header_codes(&element(concat!(
            r#"<style:header><style:region-center><text:p>a<text:s/>b<text:tab/>c<text:title>T</text:title>"#,
            r#"<text:span>d</text:span></text:p><text:p>e</text:p></style:region-center><style:region-left><text:p/></style:region-left></style:header>"#
        ))),
        Some("&Ca b cd e".to_owned())
    );
}
