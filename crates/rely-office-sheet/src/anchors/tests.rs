// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use rely_office_core::{ChartSeriesSpec, ChartSpec, ChartType};

use super::*;
use crate::address::CellRef;
use crate::charts::ChartPlacement;
use crate::model::Workbook;
use crate::value::Locale;

const NAMESPACE: &str =
    r#"xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing""#;

fn marker(name: &str, (column, column_offset, row, row_offset): (u32, i64, u32, i64)) -> String {
    format!(
        "<xdr:{name}><xdr:col>{column}</xdr:col><xdr:colOff>{column_offset}</xdr:colOff>\
         <xdr:row>{row}</xdr:row><xdr:rowOff>{row_offset}</xdr:rowOff></xdr:{name}>"
    )
}

fn two_cell(edit_as: Option<&str>, from: (u32, i64, u32, i64), to: (u32, i64, u32, i64)) -> String {
    let edit_as = edit_as.map_or_else(String::new, |value| format!(r#" editAs="{value}""#));
    format!(
        "<xdr:twoCellAnchor{edit_as}>{}{}<xdr:pic/><xdr:clientData/></xdr:twoCellAnchor>",
        marker("from", from),
        marker("to", to)
    )
}

fn one_cell(from: (u32, i64, u32, i64)) -> String {
    format!(
        r#"<xdr:oneCellAnchor>{}<xdr:ext cx="100" cy="100"/><xdr:pic/><xdr:clientData/></xdr:oneCellAnchor>"#,
        marker("from", from)
    )
}

fn drawing(anchors: &[String]) -> String {
    format!("<xdr:wsDr {NAMESPACE}>{}</xdr:wsDr>", anchors.concat())
}

/// The corners of every anchor along both axes: (column, column offset, row, row offset).
fn corners(xml: &str) -> Vec<Vec<(i64, i64, i64, i64)>> {
    let rows = find_anchors(xml, true);
    let columns = find_anchors(xml, false);
    rows.iter()
        .zip(&columns)
        .map(|(row, column)| {
            [(row.from, column.from), (row.to, column.to)]
                .into_iter()
                .filter_map(|(row, column)| {
                    Some((column?.index, column?.offset, row?.index, row?.offset))
                })
                .collect()
        })
        .collect()
}

fn after(xml: &str, shift: Shift) -> Vec<Vec<(i64, i64, i64, i64)>> {
    corners(&moved_drawing(xml, shift).unwrap_or_else(|| xml.to_owned()))
}

#[test]
fn an_anchor_that_sizes_with_its_cells_moves_and_grows_with_inserted_rows() {
    let xml = drawing(&[two_cell(None, (1, 10, 4, 20), (3, 30, 8, 40))]);
    assert_eq!(
        after(&xml, Shift::InsertRows { at: 2, count: 3 }),
        vec![vec![(1, 10, 7, 20), (3, 30, 11, 40)]],
        "rows above it move it down"
    );
    assert_eq!(
        after(&xml, Shift::InsertRows { at: 6, count: 2 }),
        vec![vec![(1, 10, 4, 20), (3, 30, 10, 40)]],
        "rows inside it make it taller"
    );
    assert_eq!(
        after(&xml, Shift::InsertRows { at: 9, count: 2 }),
        vec![vec![(1, 10, 4, 20), (3, 30, 8, 40)]],
        "rows below it leave it"
    );
    let flush = drawing(&[two_cell(None, (1, 0, 4, 0), (3, 0, 8, 0))]);
    assert_eq!(
        after(&flush, Shift::InsertRows { at: 8, count: 2 }),
        vec![vec![(1, 0, 4, 0), (3, 0, 8, 0)]],
        "rows inserted where it ends, flush with the row's top, fall below it"
    );
}

#[test]
fn an_anchor_that_sizes_with_its_cells_shrinks_with_deleted_rows_and_columns() {
    let xml = drawing(&[two_cell(None, (1, 10, 4, 20), (5, 30, 8, 40))]);
    assert_eq!(
        after(&xml, Shift::DeleteRows { at: 0, count: 2 }),
        vec![vec![(1, 10, 2, 20), (5, 30, 6, 40)]],
        "rows above it move it up"
    );
    assert_eq!(
        after(&xml, Shift::DeleteRows { at: 5, count: 2 }),
        vec![vec![(1, 10, 4, 20), (5, 30, 6, 40)]],
        "rows inside it make it shorter"
    );
    assert_eq!(
        after(&xml, Shift::DeleteRows { at: 3, count: 3 }),
        vec![vec![(1, 10, 3, 0), (5, 30, 5, 40)]],
        "its first row deleted, it starts at the edge"
    );
    assert_eq!(
        after(&xml, Shift::DeleteColumns { at: 2, count: 2 }),
        vec![vec![(1, 10, 4, 20), (3, 30, 8, 40)]],
        "columns inside it make it narrower"
    );
}

#[test]
fn an_anchor_whose_cells_are_all_deleted_moves_to_the_edge_with_its_size() {
    let xml = drawing(&[two_cell(None, (1, 10, 4, 20), (3, 30, 6, 40))]);
    assert_eq!(
        after(&xml, Shift::DeleteRows { at: 3, count: 6 }),
        vec![vec![(1, 10, 3, 0), (3, 30, 5, 40)]]
    );
}

#[test]
fn an_anchor_that_keeps_its_size_moves_with_its_corner_only() {
    let xml = drawing(&[
        two_cell(Some("oneCell"), (1, 10, 4, 20), (3, 30, 8, 40)),
        one_cell((2, 5, 6, 7)),
    ]);
    assert_eq!(
        after(&xml, Shift::InsertRows { at: 5, count: 2 }),
        vec![vec![(1, 10, 4, 20), (3, 30, 8, 40)], vec![(2, 5, 8, 7)]],
        "rows inside it do not size it"
    );
    assert_eq!(
        after(&xml, Shift::InsertRows { at: 0, count: 2 }),
        vec![vec![(1, 10, 6, 20), (3, 30, 10, 40)], vec![(2, 5, 8, 7)]]
    );
    assert_eq!(
        after(&xml, Shift::DeleteRows { at: 6, count: 1 }),
        vec![vec![(1, 10, 4, 20), (3, 30, 8, 40)], vec![(2, 5, 6, 0)]],
        "a corner on a deleted row goes to the edge"
    );
    assert_eq!(
        after(&xml, Shift::DeleteColumns { at: 0, count: 1 }),
        vec![vec![(0, 10, 4, 20), (2, 30, 8, 40)], vec![(1, 5, 6, 7)]]
    );
}

#[test]
fn an_absolute_anchor_stays() {
    let xml = drawing(&[
        two_cell(Some("absolute"), (1, 10, 4, 20), (3, 30, 8, 40)),
        r#"<xdr:absoluteAnchor><xdr:pos x="0" y="0"/><xdr:ext cx="1" cy="1"/><xdr:pic/></xdr:absoluteAnchor>"#
            .to_owned(),
    ]);
    assert_eq!(
        moved_drawing(&xml, Shift::InsertRows { at: 0, count: 5 }),
        None
    );
}

#[test]
fn an_anchor_inside_alternate_content_moves_too() {
    let xml = format!(
        r#"<xdr:wsDr {NAMESPACE} xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:AlternateContent><mc:Choice Requires="a14">{}</mc:Choice></mc:AlternateContent></xdr:wsDr>"#,
        two_cell(None, (0, 0, 2, 0), (1, 0, 4, 0))
    );
    assert_eq!(
        after(&xml, Shift::InsertRows { at: 0, count: 1 }),
        vec![vec![(0, 0, 3, 0), (1, 0, 5, 0)]]
    );
}

#[test]
fn an_anchor_stays_on_the_grid() {
    let xml = drawing(&[one_cell((0, 0, MAX_ROWS - 2, 0))]);
    assert_eq!(
        after(&xml, Shift::InsertRows { at: 0, count: 10 }),
        vec![vec![(0, 0, i64::from(MAX_ROWS) - 1, 0)]]
    );
}

fn chart() -> ChartSpec {
    ChartSpec {
        chart_type: ChartType::Column,
        stacked: false,
        title: None,
        legend: false,
        categories: None,
        series: vec![ChartSeriesSpec {
            name: None,
            values: "Foglio1!$B$2:$B$4".to_owned(),
        }],
    }
}

const PLACEMENT: ChartPlacement = ChartPlacement {
    row: 4,
    column: 3,
    offset_emu: (12_700, 25_400),
    width_emu: 4_572_000,
    height_emu: 2_743_200,
};

#[test]
fn a_chart_of_the_workbook_follows_inserted_and_deleted_rows_and_columns() {
    let mut workbook = Workbook::new();
    workbook
        .set_inputs(
            0,
            &[
                (CellRef::new(1, 1), "1".to_owned()),
                (CellRef::new(3, 1), "3".to_owned()),
            ],
            Locale::English,
        )
        .unwrap();
    workbook.add_chart(0, &chart(), PLACEMENT).unwrap();
    let placement = |workbook: &Workbook| workbook.charts(0)[0].placement;

    workbook.insert_rows(0, 2, 3).unwrap();
    assert_eq!(
        placement(&workbook),
        ChartPlacement {
            row: 7,
            ..PLACEMENT
        }
    );
    workbook.insert_columns(0, 5, 2).unwrap();
    assert_eq!(
        placement(&workbook),
        ChartPlacement {
            row: 7,
            ..PLACEMENT
        },
        "columns right of its corner leave it"
    );
    workbook.insert_columns(0, 0, 1).unwrap();
    assert_eq!(
        placement(&workbook),
        ChartPlacement {
            row: 7,
            column: 4,
            ..PLACEMENT
        }
    );
    workbook.delete_rows(0, 6, 3).unwrap();
    assert_eq!(
        placement(&workbook),
        ChartPlacement {
            row: 6,
            column: 4,
            offset_emu: (12_700, 0),
            ..PLACEMENT
        },
        "its row deleted, it moves to the edge and keeps its size"
    );
    let pictures = workbook.pictures(0);
    assert_eq!((pictures[0].row, pictures[0].column), (6, 4));
}

#[test]
fn a_sheet_without_a_drawing_shifts_as_before() {
    let mut workbook = Workbook::new();
    workbook.insert_rows(0, 0, 2).unwrap();
    assert_eq!(workbook.charts(0), Vec::new());
}
