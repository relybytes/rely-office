// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use rust_xlsxwriter::{Chart, ChartType, Image, Workbook};

use super::*;
use crate::read_xlsx_layouts;

const PNG: [u8; 70] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0x60, 0xf8, 0x5f,
    0x0f, 0x00, 0x02, 0x87, 0x01, 0x80, 0xeb, 0x47, 0xba, 0x92, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

fn workbook_with_drawings() -> Vec<u8> {
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet().set_name("Vendite").unwrap();
    for (row, value) in [10, 20, 30].into_iter().enumerate() {
        let row = u32::try_from(row).unwrap();
        sheet.write(row, 0, format!("M{row}")).unwrap();
        sheet.write(row, 1, value).unwrap();
    }
    let mut chart = Chart::new(ChartType::Column);
    chart.title().set_name("Ricavi");
    chart
        .add_series()
        .set_categories("Vendite!$A$1:$A$3")
        .set_values("Vendite!$B$1:$B$3");
    sheet.insert_chart(4, 2, &chart).unwrap();
    let image = Image::new_from_buffer(&PNG).unwrap().set_alt_text("Logo");
    sheet.insert_image(1, 5, &image).unwrap();
    workbook.save_to_buffer().unwrap()
}

#[test]
fn reads_the_charts_and_pictures_of_a_sheet() {
    let layouts = read_xlsx_layouts(&workbook_with_drawings(), &Limits::default());
    let mut drawings = layouts["Vendite"].drawings.clone();
    drawings.sort_by_key(|drawing| drawing.row);
    assert_eq!(drawings.len(), 2, "{drawings:?}");
    let picture = &drawings[0];
    assert_eq!((picture.row, picture.column), (1, 5));
    assert_eq!(picture.content_type, "image/png");
    assert_eq!(picture.bytes, PNG);
    assert_eq!(picture.description.as_deref(), Some("Logo"));
    let chart = &drawings[1];
    assert_eq!((chart.row, chart.column), (4, 2));
    assert_eq!(chart.content_type, "image/svg+xml");
    assert_eq!(chart.description.as_deref(), Some("Ricavi"));
    let svg = String::from_utf8(chart.bytes.clone()).unwrap();
    assert!(
        svg.contains(">M2</text>"),
        "the categories cached in the chart"
    );
    // Excel's default chart is 480 by 288 pixels.
    assert_eq!(
        (chart.width_emu, chart.height_emu),
        (480 * EMU_PER_PIXEL, 288 * EMU_PER_PIXEL)
    );

    let tight = Limits {
        max_images: 1,
        ..Limits::default()
    };
    assert_eq!(
        read_xlsx_layouts(&workbook_with_drawings(), &tight)["Vendite"]
            .drawings
            .len(),
        1
    );
}

#[test]
fn reads_the_cells_a_chart_formula_names() {
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet().set_name("Dati d'oggi").unwrap();
    sheet.write(0, 0, 1.5).unwrap();
    sheet.write(0, 1, "testo").unwrap();
    sheet.write(1, 0, true).unwrap();
    sheet.write(1, 1, 7).unwrap();
    let bytes = workbook.save_to_buffer().unwrap();
    let cells = WorkbookCells::new(&bytes);
    assert_eq!(
        cells.cells("'Dati d''oggi'!$A$1:$B$3"),
        Some(vec![
            ChartCell::Number(1.5),
            ChartCell::Text("testo".to_owned()),
            ChartCell::Number(1.0),
            ChartCell::Number(7.0),
            ChartCell::Empty,
            ChartCell::Empty,
        ])
    );
    assert_eq!(
        cells.cells("'Dati d''oggi'!B2"),
        Some(vec![ChartCell::Number(7.0)]),
        "read once, kept"
    );
    assert_eq!(cells.cells("Assente!A1"), None);
    assert_eq!(cells.cells("A1:B2"), None, "no sheet");
    assert_eq!(
        cells.cells("'Dati d''oggi'!A1:XFD1048576"),
        None,
        "too many cells"
    );
    assert_eq!(
        WorkbookCells::new(b"not a workbook").cells("Foglio!A1"),
        None
    );
}

#[test]
fn splits_a_formula_into_its_sheet_and_range() {
    assert_eq!(
        split_formula("='Vendite'!$B$2:$B$7"),
        Some(("Vendite".to_owned(), "$B$2:$B$7".to_owned()))
    );
    assert_eq!(
        split_formula("(Foglio1!A1)"),
        Some(("Foglio1".to_owned(), "A1".to_owned()))
    );
    assert_eq!(split_formula("Foglio1!A1,Foglio1!A3"), None);
    assert_eq!(split_formula("A1"), None);
}

fn anchor(xml: &str) -> Element {
    parse_tree(
        &format!(
            r#"<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing">{xml}</xdr:wsDr>"#
        ),
        |_, _| false,
    )
    .unwrap()
    .elements()
    .next()
    .unwrap()
    .clone()
}

#[test]
fn sizes_a_drawing_by_its_anchor() {
    let mut layout = SheetLayout {
        column_widths: vec![None, Some(0), Some(20 * 256)],
        default_row_height: Some(400),
        ..SheetLayout::default()
    };
    layout.row_heights.insert(1, 600);
    let two_cell = anchor(concat!(
        "<xdr:twoCellAnchor><xdr:from><xdr:col>0</xdr:col><xdr:colOff>9525</xdr:colOff><xdr:row>0</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from>",
        "<xdr:to><xdr:col>3</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>2</xdr:row><xdr:rowOff>635</xdr:rowOff></xdr:to></xdr:twoCellAnchor>"
    ));
    // 64 pixels, a hidden column, then 20 characters of 145 pixels.
    let width = (64 + 145) * EMU_PER_PIXEL - 9_525;
    let height = (400 + 600) * EMU_PER_TWIP + 635;
    assert_eq!(
        anchor_frame(&two_cell, &layout),
        Some(AnchorFrame {
            row: 0,
            column: 0,
            offset_emu: (9_525, 0),
            width_emu: width,
            height_emu: height
        })
    );
    let one_cell = anchor(concat!(
        "<xdr:oneCellAnchor><xdr:from><xdr:col>4</xdr:col><xdr:row>7</xdr:row><xdr:rowOff>12700</xdr:rowOff></xdr:from>",
        r#"<xdr:ext cx="1000" cy="2000"/></xdr:oneCellAnchor>"#
    ));
    assert_eq!(
        anchor_frame(&one_cell, &layout),
        Some(AnchorFrame {
            row: 7,
            column: 4,
            offset_emu: (0, 12_700),
            width_emu: 1_000,
            height_emu: 2_000
        })
    );
    let absolute = anchor(
        r#"<xdr:absoluteAnchor><xdr:pos x="300" y="-5"/><xdr:ext cx="10" cy="20"/></xdr:absoluteAnchor>"#,
    );
    assert_eq!(
        anchor_frame(&absolute, &layout),
        Some(AnchorFrame {
            row: 0,
            column: 0,
            offset_emu: (300, 0),
            width_emu: 10,
            height_emu: 20
        })
    );
    assert_eq!(anchor_frame(&anchor("<xdr:clientData/>"), &layout), None);
    assert_eq!(
        anchor_frame(
            &anchor("<xdr:oneCellAnchor><xdr:from/></xdr:oneCellAnchor>"),
            &layout
        ),
        None
    );
    assert_eq!(pixels_of_width(0), 0);
    assert_eq!(pixels_of_width(2_340), 69);
    assert_eq!(
        columns_emu(&SheetLayout::default(), 0, 2),
        128 * EMU_PER_PIXEL
    );
    assert_eq!(
        rows_emu(&SheetLayout::default(), 3, 1),
        0,
        "a range running backwards is empty"
    );
}

#[test]
fn spends_an_image_budget() {
    let mut budget = ImageBudget {
        images: 2,
        bytes: 10,
    };
    assert!(budget.take(6));
    assert!(!budget.take(6), "over the bytes left");
    assert!(budget.take(4));
    assert!(!budget.take(0), "no image left");
}
