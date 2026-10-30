// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use super::*;
use crate::doc::properties::CellDefinition;
use crate::document::{Inline, ParagraphLook, RunLook};

fn paragraph(text: &str) -> Block {
    Block::Paragraph {
        content: if text.is_empty() {
            Vec::new()
        } else {
            vec![Inline::Text {
                text: text.to_owned(),
                marks: Vec::new(),
                look: RunLook::default(),
            }]
        },
        align: None,
        look: ParagraphLook::default(),
    }
}

fn cell(left: i16, right: i16) -> CellDefinition {
    CellDefinition {
        left,
        right,
        ..CellDefinition::default()
    }
}

fn texts(cell: &TableCell) -> Vec<String> {
    cell.blocks
        .iter()
        .map(|block| match block {
            Block::Paragraph { content, .. } => crate::document::plain_text(content),
            other => format!("{other:?}"),
        })
        .collect()
}

fn row(builder: &mut TableBuilder, cells: &[&str], definition: Row) {
    for text in cells {
        builder.push(paragraph(text));
        builder.end_cell();
    }
    builder.end_row(definition);
}

#[test]
fn builds_a_table_on_the_grid_of_its_cell_edges() {
    let mut builder = TableBuilder::default();
    row(
        &mut builder,
        &["Voce", "Q1", "Q2"],
        Row {
            cells: vec![cell(0, 2_000), cell(2_000, 3_000), cell(3_000, 4_000)],
            header: true,
            bordered: false,
        },
    );
    let mut merged = cell(2_000, 3_000);
    merged.merged = true;
    let mut filled = cell(3_000, 4_000);
    filled.fill = Some("FFFF00".to_owned());
    filled.bordered = true;
    row(
        &mut builder,
        &["Totale", "", "255"],
        Row {
            cells: vec![cell(0, 2_000), merged, filled],
            header: false,
            bordered: false,
        },
    );
    let Some(Block::Table { rows, look }) = builder.finish() else {
        panic!("a table");
    };
    assert_eq!(look.column_widths_twips, vec![2_000, 1_000, 1_000]);
    assert!(look.borders);
    assert!(rows[0].header && !rows[1].header);
    assert_eq!(rows[1].cells.len(), 2);
    assert_eq!(
        rows[1].cells[0].col_span, 2,
        "the merged cell folds into the one on its left"
    );
    assert_eq!(
        texts(&rows[1].cells[0]),
        vec!["Totale"],
        "an empty paragraph is not carried over"
    );
    assert_eq!(rows[1].cells[1].look.fill.as_deref(), Some("FFFF00"));
}

#[test]
fn spans_rows_for_the_cells_that_continue_the_one_above() {
    let mut builder = TableBuilder::default();
    let edges = || vec![cell(0, 1_000), cell(1_000, 2_000)];
    row(
        &mut builder,
        &["A", "B"],
        Row {
            cells: edges(),
            ..Row::default()
        },
    );
    let mut second = edges();
    second[0].continues_above = true;
    row(
        &mut builder,
        &["", "C"],
        Row {
            cells: second.clone(),
            ..Row::default()
        },
    );
    row(
        &mut builder,
        &["", "D"],
        Row {
            cells: second,
            ..Row::default()
        },
    );
    let Some(Block::Table { rows, .. }) = builder.finish() else {
        panic!("a table");
    };
    assert_eq!(rows[0].cells[0].row_span, 3);
    assert_eq!(rows[1].cells.len(), 1);
    assert_eq!(texts(&rows[2].cells[0]), vec!["D"]);
}

#[test]
fn spreads_a_table_without_cell_edges_evenly() {
    let mut builder = TableBuilder::default();
    row(&mut builder, &["a", "b"], Row::default());
    builder.push(paragraph("c"));
    let Some(Block::Table { rows, look }) = builder.finish() else {
        panic!("a table");
    };
    assert_eq!(look.column_widths_twips, vec![4_320, 4_320]);
    assert_eq!(rows.len(), 2, "a row left open is closed");
    assert_eq!(rows[1].cells.len(), 1);
    assert!(TableBuilder::default().finish().is_none());
}

#[test]
fn places_a_cell_past_the_edges_its_row_names() {
    let mut builder = TableBuilder::default();
    row(
        &mut builder,
        &["a", "b", "c"],
        Row {
            cells: vec![cell(0, 1_000), cell(1_000, 1_000)],
            ..Row::default()
        },
    );
    let mut continues = cell(0, 1_000);
    continues.continues_above = true;
    let mut merged = cell(0, 1_000);
    merged.merged = true;
    row(
        &mut builder,
        &["x", "y"],
        Row {
            cells: vec![merged, continues],
            ..Row::default()
        },
    );
    let Some(Block::Table { rows, .. }) = builder.finish() else {
        panic!("a table");
    };
    assert_eq!(
        rows[0]
            .cells
            .iter()
            .map(|cell| cell.col_span)
            .collect::<Vec<_>>(),
        vec![1, 1, 1]
    );
    assert_eq!(
        rows[1].cells.len(),
        1,
        "a merge with nothing on its left is a cell of its own"
    );
    assert_eq!(rows[0].cells[0].row_span, 2);
}
