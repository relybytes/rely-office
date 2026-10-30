// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! A rectangle of a sheet as it is, to put back later: what a collaborative editor undoes with,
//! since the snapshot of the whole workbook the history keeps would also undo what the others
//! wrote since.

use rely_office_core::{CellRange, CellStyle, CommentReply};

use crate::address::{CellRef, MAX_COLUMNS, MAX_ROWS};
use crate::model::{
    Cell, Comment, EditError, MAX_CELLS, MAX_COMMENT_CHARS, MAX_COMMENT_REPLIES, NumberFormat,
    Workbook,
};
use crate::value::{Input, Locale};

/// The most cells a region holds, so an undo of a whole-sheet edit stays a bounded message.
pub const MAX_REGION_CELLS: usize = 200_000;

/// One cell of a region: its content typed in English, its format and its look.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct RegionCell {
    pub row: u32,
    pub column: u32,
    /// What was typed, as the English locale writes it; empty for a cell with only a look.
    pub input: String,
    pub format: NumberFormat,
    /// `None` for the default look.
    pub style: Option<CellStyle>,
}

/// A reply of a region's comment thread.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct RegionReply {
    pub author: String,
    pub text: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub date: Option<String>,
}

/// A comment of a region's cell, with its thread.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct RegionComment {
    pub row: u32,
    pub column: u32,
    pub author: String,
    pub text: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub date: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub threaded: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    pub resolved: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    pub replies: Vec<RegionReply>,
}

impl RegionComment {
    fn to_comment(&self) -> Result<Comment, EditError> {
        let too_long = |text: &str| text.chars().count() > MAX_COMMENT_CHARS;
        if too_long(&self.text)
            || too_long(&self.author)
            || self.replies.len() > MAX_COMMENT_REPLIES
            || self
                .replies
                .iter()
                .any(|reply| too_long(&reply.text) || too_long(&reply.author))
        {
            return Err(EditError::OutOfBounds);
        }
        Ok(Comment {
            author: self.author.clone(),
            text: self.text.clone(),
            date: self.date.clone(),
            threaded: self.threaded,
            resolved: self.resolved,
            replies: self
                .replies
                .iter()
                .map(|reply| CommentReply {
                    author: reply.author.clone(),
                    text: reply.text.clone(),
                    date: reply.date.clone(),
                })
                .collect(),
        })
    }
}

/// A rectangle of a sheet: its cells, the merges inside it and its comments.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct Region {
    pub area: CellRange,
    pub cells: Vec<RegionCell>,
    pub merges: Vec<CellRange>,
    pub comments: Vec<RegionComment>,
}

fn area_of(range: (CellRef, CellRef)) -> Result<CellRange, EditError> {
    let (start, end) = range;
    let area = CellRange {
        first_row: start.row.min(end.row),
        first_column: start.column.min(end.column),
        last_row: start.row.max(end.row),
        last_column: start.column.max(end.column),
    };
    if area.last_row >= MAX_ROWS || area.last_column >= MAX_COLUMNS {
        return Err(EditError::OutOfBounds);
    }
    Ok(area)
}

fn inside(area: &CellRange, cell: CellRef) -> bool {
    (area.first_row..=area.last_row).contains(&cell.row)
        && (area.first_column..=area.last_column).contains(&cell.column)
}

fn contains(area: &CellRange, range: &CellRange) -> bool {
    range.first_row >= area.first_row
        && range.last_row <= area.last_row
        && range.first_column >= area.first_column
        && range.last_column <= area.last_column
}

impl Workbook {
    /// A rectangle of a sheet as it is now.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::OutOfBounds`] past the grid or past
    /// [`MAX_REGION_CELLS`] cells with content or a look.
    pub fn region(&self, sheet: usize, range: (CellRef, CellRef)) -> Result<Region, EditError> {
        let area = area_of(range)?;
        let source = self
            .sheets
            .get(sheet)
            .ok_or(EditError::NoSuchSheet(sheet))?;
        let mut cells: Vec<RegionCell> = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for row in area.first_row..=area.last_row {
            let from = CellRef::new(row, area.first_column);
            let to = CellRef::new(row, area.last_column);
            for (cell, content) in source.cells.range(from..=to) {
                seen.insert(*cell);
                cells.push(RegionCell {
                    row: cell.row,
                    column: cell.column,
                    input: content.input.text_in(Locale::English),
                    format: content.format,
                    style: None,
                });
            }
            if cells.len() > MAX_REGION_CELLS {
                return Err(EditError::OutOfBounds);
            }
        }
        for cell in &mut cells {
            let id = source
                .styles
                .get(&CellRef::new(cell.row, cell.column))
                .copied();
            cell.style = id.and_then(|id| self.styles.get(usize::try_from(id).ok()?).cloned());
        }
        for (cell, id) in &source.styles {
            if inside(&area, *cell) && !seen.contains(cell) {
                cells.push(RegionCell {
                    row: cell.row,
                    column: cell.column,
                    input: String::new(),
                    format: NumberFormat::General,
                    style: self.styles.get(usize::try_from(*id).unwrap_or(0)).cloned(),
                });
            }
        }
        if cells.len() > MAX_REGION_CELLS {
            return Err(EditError::OutOfBounds);
        }
        cells.sort_by_key(|cell| (cell.row, cell.column));
        let merges = source
            .merges
            .iter()
            .filter(|merge| contains(&area, merge))
            .copied()
            .collect();
        let comments = source
            .comments
            .iter()
            .filter(|(cell, _)| inside(&area, **cell))
            .map(|(cell, comment)| RegionComment {
                row: cell.row,
                column: cell.column,
                author: comment.author.clone(),
                text: comment.text.clone(),
                date: comment.date.clone(),
                threaded: comment.threaded,
                resolved: comment.resolved,
                replies: comment
                    .replies
                    .iter()
                    .map(|reply| RegionReply {
                        author: reply.author.clone(),
                        text: reply.text.clone(),
                        date: reply.date.clone(),
                    })
                    .collect(),
            })
            .collect();
        Ok(Region {
            area,
            cells,
            merges,
            comments,
        })
    }

    /// Put a region back as it was taken: everything in its rectangle goes, then its cells,
    /// merges and comments return. All or nothing.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::OutOfBounds`] for a cell or merge outside the
    /// region or past the limits, the parser's error for an input that does not read.
    pub fn restore_region(&mut self, sheet: usize, region: &Region) -> Result<(), EditError> {
        let area = area_of((
            CellRef::new(region.area.first_row, region.area.first_column),
            CellRef::new(region.area.last_row, region.area.last_column),
        ))?;
        if region.cells.len() > MAX_REGION_CELLS {
            return Err(EditError::OutOfBounds);
        }
        let mut parsed = Vec::with_capacity(region.cells.len());
        for cell in &region.cells {
            let at = CellRef::new(cell.row, cell.column);
            if !inside(&area, at) {
                return Err(EditError::OutOfBounds);
            }
            let input = if cell.input.is_empty() {
                None
            } else {
                Input::parse(&cell.input, Locale::English)?
            };
            parsed.push((at, input, cell.format, cell.style.clone()));
        }
        if region.merges.iter().any(|merge| !contains(&area, merge)) {
            return Err(EditError::OutOfBounds);
        }
        let mut comments = Vec::with_capacity(region.comments.len());
        for comment in &region.comments {
            let at = CellRef::new(comment.row, comment.column);
            if !inside(&area, at) {
                return Err(EditError::OutOfBounds);
            }
            comments.push((at, comment.to_comment()?));
        }
        let replaced = self
            .sheets
            .get(sheet)
            .ok_or(EditError::NoSuchSheet(sheet))?
            .cells
            .keys()
            .filter(|cell| inside(&area, **cell))
            .count();
        let outside = self.cell_count() - replaced;
        if outside
            + parsed
                .iter()
                .filter(|(_, input, ..)| input.is_some())
                .count()
            > MAX_CELLS
        {
            return Err(EditError::OutOfBounds);
        }
        let mut ids = Vec::with_capacity(parsed.len());
        for (_, _, _, style) in &parsed {
            ids.push(style.clone().map_or(0, |style| self.intern_style(style)));
        }
        let target = &mut self.sheets[sheet];
        target.cells.retain(|cell, _| !inside(&area, *cell));
        target.styles.retain(|cell, _| !inside(&area, *cell));
        target.comments.retain(|cell, _| !inside(&area, *cell));
        target.merges.retain(|merge| {
            merge.last_row < area.first_row
                || merge.first_row > area.last_row
                || merge.last_column < area.first_column
                || merge.first_column > area.last_column
        });
        for ((cell, input, format, _), id) in parsed.into_iter().zip(ids) {
            if let Some(input) = input {
                target.cells.insert(cell, Cell { input, format });
            }
            if id != 0 {
                target.styles.insert(cell, id);
            }
        }
        target.merges.extend(region.merges.iter().copied());
        target.comments.extend(comments);
        self.recalculate();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::styling::StylePatch;

    fn at(row: u32, column: u32) -> CellRef {
        CellRef::new(row, column)
    }

    #[test]
    fn a_region_comes_back_as_it_was_taken() {
        let mut workbook = Workbook::new();
        workbook
            .set_input(0, at(0, 0), "1,5", Locale::Italian)
            .unwrap();
        workbook
            .set_input(0, at(1, 0), "=SOMMA(A1;2)", Locale::Italian)
            .unwrap();
        workbook
            .set_style(
                0,
                (at(1, 1), at(1, 1)),
                &StylePatch {
                    bold: Some(true),
                    ..StylePatch::default()
                },
            )
            .unwrap();
        workbook.merge_cells(0, (at(2, 0), at(2, 1))).unwrap();
        workbook.set_comment(0, at(0, 0), "Ada", "Check").unwrap();
        let region = workbook.region(0, (at(0, 0), at(3, 1))).unwrap();
        assert_eq!(region.cells.len(), 3);
        assert_eq!(region.cells[0].input, "1.5");
        assert_eq!(region.cells[1].input, "=SUM(A1,2)");
        assert!(
            region.cells[2]
                .style
                .as_ref()
                .is_some_and(|style| style.font.bold)
        );
        assert_eq!(region.merges.len(), 1);
        assert_eq!(region.comments[0].text, "Check");

        workbook.clear_range(0, (at(0, 0), at(3, 1))).unwrap();
        workbook.clear_styles(0, (at(0, 0), at(3, 1))).unwrap();
        workbook.unmerge_cells(0, (at(0, 0), at(3, 1))).unwrap();
        workbook.set_comment(0, at(0, 0), "", "").unwrap();
        workbook
            .set_input(0, at(3, 1), "x", Locale::Italian)
            .unwrap();
        workbook.restore_region(0, &region).unwrap();

        assert_eq!(workbook.region(0, (at(0, 0), at(3, 1))).unwrap(), region);
        assert_eq!(workbook.value(0, at(1, 0)), crate::Value::Number(3.5));
    }

    #[test]
    fn a_region_is_refused_outside_its_rectangle() {
        let mut workbook = Workbook::new();
        let mut region = workbook.region(0, (at(0, 0), at(1, 1))).unwrap();
        region.cells.push(RegionCell {
            row: 5,
            column: 0,
            input: "1".to_owned(),
            format: NumberFormat::General,
            style: None,
        });
        assert_eq!(
            workbook.restore_region(0, &region),
            Err(EditError::OutOfBounds)
        );
        assert_eq!(
            workbook.restore_region(3, &workbook.region(0, (at(0, 0), at(0, 0))).unwrap()),
            Err(EditError::NoSuchSheet(3))
        );
        assert_eq!(
            workbook.region(0, (at(0, 0), at(MAX_ROWS, 0))),
            Err(EditError::OutOfBounds)
        );
    }

    #[test]
    fn a_region_keeps_what_is_outside_it() {
        let mut workbook = Workbook::new();
        workbook
            .set_input(0, at(0, 0), "a", Locale::English)
            .unwrap();
        workbook
            .set_input(0, at(5, 5), "b", Locale::English)
            .unwrap();
        let region = workbook.region(0, (at(0, 0), at(1, 1))).unwrap();
        workbook
            .set_input(0, at(0, 0), "c", Locale::English)
            .unwrap();
        workbook.restore_region(0, &region).unwrap();
        assert_eq!(
            workbook.value(0, at(0, 0)),
            crate::Value::Text("a".to_owned())
        );
        assert_eq!(
            workbook.value(0, at(5, 5)),
            crate::Value::Text("b".to_owned())
        );
    }
}
