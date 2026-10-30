// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! A Word 97 table put together from its cell paragraphs and the cell layout each row ends with.

use super::super::properties::Row;
use crate::document::{Block, CellLook, TableCell, TableLook, TableRow};

/// The width a table without a cell layout spreads over, in twips: a Letter page's text.
const DEFAULT_TABLE_TWIPS: u32 = 8_640;

/// The rows read so far, and the cells of the row being read.
#[derive(Debug, Default)]
pub(super) struct TableBuilder {
    rows: Vec<(Vec<Vec<Block>>, Row)>,
    cells: Vec<Vec<Block>>,
    cell: Vec<Block>,
}

impl TableBuilder {
    pub(super) fn push(&mut self, block: Block) {
        self.cell.push(block);
    }

    pub(super) fn end_cell(&mut self) {
        let cell = std::mem::take(&mut self.cell);
        self.cells.push(cell);
    }

    pub(super) fn end_row(&mut self, row: Row) {
        if !self.cell.is_empty() {
            self.end_cell();
        }
        let cells = std::mem::take(&mut self.cells);
        self.rows.push((cells, row));
    }

    /// The table: a grid from every row's cell edges, each cell spanning the columns its edges
    /// cover, merged cells folded into the one they continue.
    pub(super) fn finish(mut self) -> Option<Block> {
        if !self.cell.is_empty() || !self.cells.is_empty() {
            self.end_row(Row::default());
        }
        if self.rows.is_empty() {
            return None;
        }
        let mut edges: Vec<i16> = self
            .rows
            .iter()
            .flat_map(|(_, row)| row.cells.iter().flat_map(|cell| [cell.left, cell.right]))
            .collect();
        edges.sort_unstable();
        edges.dedup();
        let widest = self
            .rows
            .iter()
            .map(|(cells, _)| cells.len())
            .max()
            .unwrap_or(1)
            .max(1);
        let column_widths_twips: Vec<u32> = if edges.len() >= 2 {
            edges
                .windows(2)
                .map(|pair| u32::from((pair[1] - pair[0]).unsigned_abs()))
                .collect()
        } else {
            let width = DEFAULT_TABLE_TWIPS / u32::try_from(widest).unwrap_or(1);
            vec![width; widest]
        };
        let column_of = |edge: i16| edges.iter().position(|known| *known == edge);
        let mut bordered = false;
        let mut rows: Vec<TableRow> = Vec::new();
        // The grid column each placed cell starts at, row by row, for the merges going down.
        let mut starts: Vec<Vec<usize>> = Vec::new();
        for (cells, row) in self.rows {
            bordered |= row.bordered;
            let mut placed: Vec<TableCell> = Vec::new();
            let mut placed_starts: Vec<usize> = Vec::new();
            let mut column = 0;
            for (index, blocks) in cells.into_iter().enumerate() {
                let definition = row.cells.get(index);
                bordered |= definition.is_some_and(|definition| definition.bordered);
                let (start, span) = match definition.and_then(|definition| {
                    Some((column_of(definition.left)?, column_of(definition.right)?))
                }) {
                    Some((left, right)) if right > left => (left, right - left),
                    _ => (column, 1),
                };
                column = start + span;
                let span = u32::try_from(span).unwrap_or(1);
                if definition.is_some_and(|definition| definition.merged)
                    && let Some(previous) = placed.last_mut()
                {
                    previous.col_span += span;
                    previous.blocks.extend(
                        blocks
                            .into_iter()
                            .filter(|block| !is_empty_paragraph(block)),
                    );
                    continue;
                }
                if definition.is_some_and(|definition| definition.continues_above)
                    && let Some(above) = cell_above(&mut rows, &starts, start)
                {
                    above.row_span += 1;
                    continue;
                }
                placed.push(TableCell {
                    col_span: span,
                    row_span: 1,
                    blocks,
                    look: CellLook {
                        fill: definition.and_then(|definition| definition.fill.clone()),
                        ..CellLook::default()
                    },
                });
                placed_starts.push(start);
            }
            rows.push(TableRow {
                header: row.header,
                cells: placed,
            });
            starts.push(placed_starts);
        }
        Some(Block::Table {
            rows,
            look: TableLook {
                column_widths_twips,
                borders: bordered,
                ..TableLook::default()
            },
        })
    }
}

/// The cell of an earlier row that starts at a grid column, the nearest row first.
fn cell_above<'r>(
    rows: &'r mut [TableRow],
    starts: &[Vec<usize>],
    column: usize,
) -> Option<&'r mut TableCell> {
    let (row, index) = starts.iter().enumerate().rev().find_map(|(row, columns)| {
        columns
            .iter()
            .position(|start| *start == column)
            .map(|index| (row, index))
    })?;
    rows.get_mut(row)?.cells.get_mut(index)
}

fn is_empty_paragraph(block: &Block) -> bool {
    matches!(block, Block::Paragraph { content, .. } if content.is_empty())
}

#[cfg(test)]
mod tests;
