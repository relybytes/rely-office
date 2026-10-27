// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! How cells look and which cells are merged: the styles of a workbook, kept once each and
//! shared by the cells that use them, and the edits that change them.

use rely_office_core::{Border, CellRange, CellStyle, HorizontalAlignment, VerticalAlignment};

use crate::address::{CellRef, MAX_COLUMNS, MAX_ROWS};
use crate::model::{EditError, NumberFormat, Workbook};

/// The most cells one style edit may cover, so a selection cannot fill memory.
pub const MAX_STYLED_AREA: u64 = 1_000_000;
/// The most cells with a style a workbook may hold, over all its sheets.
pub const MAX_STYLED_CELLS: usize = 2_000_000;
/// The most merged ranges of a sheet.
pub const MAX_MERGES: usize = 100_000;
/// Excel's tallest row, in points.
pub const MAX_ROW_HEIGHT: f64 = 409.0;

/// A change to the look of cells: what is set changes, what is left out stays. A colour is
/// `RRGGBB` or `#RRGGBB`, and an empty text, a size of 0 or an alignment of `general` goes back
/// to the default.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Deserialize),
    serde(rename_all = "camelCase", default, deny_unknown_fields)
)]
pub struct StylePatch {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    pub font_family: Option<String>,
    /// In points.
    pub font_size: Option<f64>,
    pub font_colour: Option<String>,
    pub fill: Option<String>,
    /// `general`, `left`, `center`, `right` or `justify`.
    pub horizontal: Option<String>,
    /// `bottom`, `center` or `top`.
    pub vertical: Option<String>,
    pub wrap: Option<bool>,
    /// An Excel number format code, `#,##0.00 "€"`; an empty one is General.
    pub number_format: Option<String>,
}

/// Which edges of a rectangle a border goes on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub enum BorderPlacement {
    /// Every edge of every cell.
    All,
    /// The outer edges of the rectangle.
    Outline,
    /// The edges between its cells.
    Inside,
    Top,
    Bottom,
    Left,
    Right,
}

/// Why a style patch was refused.
fn invalid() -> EditError {
    EditError::InvalidStyle
}

impl StylePatch {
    /// This patch applied to a style.
    fn apply(&self, style: &CellStyle) -> Result<CellStyle, EditError> {
        let mut changed = style.clone();
        let font = &mut changed.font;
        if let Some(bold) = self.bold {
            font.bold = bold;
        }
        if let Some(italic) = self.italic {
            font.italic = italic;
        }
        if let Some(underline) = self.underline {
            font.underline = underline;
        }
        if let Some(strike) = self.strike {
            font.strike = strike;
        }
        if let Some(family) = &self.font_family {
            let family = family.trim();
            if family.chars().count() > 31 || family.chars().any(char::is_control) {
                return Err(invalid());
            }
            font.family = (!family.is_empty()).then(|| family.to_owned());
        }
        if let Some(size) = self.font_size {
            if !size.is_finite() || !(0.0..=409.0).contains(&size) || (size > 0.0 && size < 1.0) {
                return Err(invalid());
            }
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "the size is checked to lie between 1 and 409 points"
            )]
            let tenths = (size * 10.0).round() as u32;
            font.size_tenths = (size > 0.0).then_some(tenths);
        }
        if let Some(colour) = &self.font_colour {
            font.colour = colour_patch(colour)?;
        }
        if let Some(fill) = &self.fill {
            changed.fill = colour_patch(fill)?;
        }
        if let Some(horizontal) = &self.horizontal {
            changed.horizontal = match horizontal.as_str() {
                "general" => None,
                "left" => Some(HorizontalAlignment::Left),
                "center" => Some(HorizontalAlignment::Center),
                "right" => Some(HorizontalAlignment::Right),
                "justify" => Some(HorizontalAlignment::Justify),
                _ => return Err(invalid()),
            };
        }
        if let Some(vertical) = &self.vertical {
            changed.vertical = match vertical.as_str() {
                "bottom" => None,
                "center" => Some(VerticalAlignment::Center),
                "top" => Some(VerticalAlignment::Top),
                _ => return Err(invalid()),
            };
        }
        if let Some(wrap) = self.wrap {
            changed.wrap = wrap;
        }
        if let Some(code) = &self.number_format {
            if code.chars().count() > 255 {
                return Err(invalid());
            }
            changed.number_format = (!code.trim().is_empty()
                && !code.eq_ignore_ascii_case("general"))
            .then(|| code.clone());
        }
        Ok(changed)
    }
}

/// A colour of a patch: `RRGGBB`, `#RRGGBB`, or empty for none.
fn colour_patch(colour: &str) -> Result<Option<String>, EditError> {
    let hex = colour.trim().trim_start_matches('#');
    if hex.is_empty() {
        return Ok(None);
    }
    if hex.len() == 6 && hex.chars().all(|character| character.is_ascii_hexdigit()) {
        Ok(Some(hex.to_ascii_uppercase()))
    } else {
        Err(invalid())
    }
}

/// The rectangle of two corners, checked against the grid and the size of one style edit.
fn rectangle(range: (CellRef, CellRef)) -> Result<CellRange, EditError> {
    let (start, end) = range;
    let rectangle = CellRange {
        first_row: start.row.min(end.row),
        first_column: start.column.min(end.column),
        last_row: start.row.max(end.row),
        last_column: start.column.max(end.column),
    };
    if rectangle.last_row >= MAX_ROWS || rectangle.last_column >= MAX_COLUMNS {
        return Err(EditError::OutOfBounds);
    }
    let area = u64::from(rectangle.last_row - rectangle.first_row + 1)
        * u64::from(rectangle.last_column - rectangle.first_column + 1);
    if area > MAX_STYLED_AREA {
        return Err(EditError::OutOfBounds);
    }
    Ok(rectangle)
}

fn cells_of(range: CellRange) -> impl Iterator<Item = CellRef> {
    (range.first_row..=range.last_row).flat_map(move |row| {
        (range.first_column..=range.last_column).map(move |column| CellRef::new(row, column))
    })
}

fn overlaps(first: &CellRange, second: &CellRange) -> bool {
    first.first_row <= second.last_row
        && second.first_row <= first.last_row
        && first.first_column <= second.last_column
        && second.first_column <= first.last_column
}

impl Workbook {
    /// The styles of the workbook; the first is the default look.
    #[must_use]
    pub fn styles(&self) -> &[CellStyle] {
        &self.styles
    }

    /// The index of a cell's style in [`Workbook::styles`]; 0 is the default.
    #[must_use]
    pub fn style_id(&self, sheet: usize, cell: CellRef) -> u32 {
        self.sheets
            .get(sheet)
            .and_then(|sheet| sheet.styles.get(&cell))
            .copied()
            .unwrap_or(0)
    }

    /// The look of a cell.
    #[must_use]
    pub fn style(&self, sheet: usize, cell: CellRef) -> &CellStyle {
        let id = usize::try_from(self.style_id(sheet, cell)).unwrap_or(0);
        self.styles.get(id).unwrap_or(&self.styles[0])
    }

    /// The index of a style, adding it the first time.
    pub(crate) fn intern_style(&mut self, style: CellStyle) -> u32 {
        if let Some(id) = self.style_index.get(&style) {
            return *id;
        }
        let id = u32::try_from(self.styles.len()).unwrap_or(u32::MAX);
        self.style_index.insert(style.clone(), id);
        self.styles.push(style);
        id
    }

    fn styled_count(&self) -> usize {
        self.sheets.iter().map(|sheet| sheet.styles.len()).sum()
    }

    /// Change the look of every cell of a rectangle.
    ///
    /// # Errors
    ///
    /// [`EditError::InvalidStyle`] for a patch with a value no style can hold,
    /// [`EditError::NoSuchSheet`], [`EditError::OutOfBounds`] past the grid, past
    /// [`MAX_STYLED_AREA`] cells at once or [`MAX_STYLED_CELLS`] styled cells.
    pub fn set_style(
        &mut self,
        sheet: usize,
        range: (CellRef, CellRef),
        patch: &StylePatch,
    ) -> Result<(), EditError> {
        self.restyle(sheet, range, |style, _| patch.apply(style))
    }

    /// Put a border on edges of a rectangle, or take it off with `None`.
    ///
    /// # Errors
    ///
    /// As [`Workbook::set_style`].
    pub fn set_borders(
        &mut self,
        sheet: usize,
        range: (CellRef, CellRef),
        placement: BorderPlacement,
        border: Option<&Border>,
    ) -> Result<(), EditError> {
        let border = border.cloned();
        let area = rectangle(range)?;
        self.restyle(sheet, range, |style, cell| {
            let mut changed = style.clone();
            let top = cell.row == area.first_row;
            let bottom = cell.row == area.last_row;
            let left = cell.column == area.first_column;
            let right = cell.column == area.last_column;
            let edges = &mut changed.borders;
            let put = |edge: &mut Option<Border>, on: bool| {
                if on {
                    edge.clone_from(&border);
                }
            };
            match placement {
                BorderPlacement::All => {
                    put(&mut edges.top, true);
                    put(&mut edges.bottom, true);
                    put(&mut edges.left, true);
                    put(&mut edges.right, true);
                }
                BorderPlacement::Outline => {
                    put(&mut edges.top, top);
                    put(&mut edges.bottom, bottom);
                    put(&mut edges.left, left);
                    put(&mut edges.right, right);
                }
                BorderPlacement::Inside => {
                    put(&mut edges.top, !top);
                    put(&mut edges.bottom, !bottom);
                    put(&mut edges.left, !left);
                    put(&mut edges.right, !right);
                }
                BorderPlacement::Top => put(&mut edges.top, top),
                BorderPlacement::Bottom => put(&mut edges.bottom, bottom),
                BorderPlacement::Left => put(&mut edges.left, left),
                BorderPlacement::Right => put(&mut edges.right, right),
            }
            Ok(changed)
        })
    }

    /// Give every cell of a rectangle the default look; their content stays.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::OutOfBounds`].
    pub fn clear_styles(
        &mut self,
        sheet: usize,
        range: (CellRef, CellRef),
    ) -> Result<(), EditError> {
        let area = rectangle(range)?;
        let target = self
            .sheets
            .get_mut(sheet)
            .ok_or(EditError::NoSuchSheet(sheet))?;
        target.styles.retain(|cell, _| {
            !(area.first_row..=area.last_row).contains(&cell.row)
                || !(area.first_column..=area.last_column).contains(&cell.column)
        });
        Ok(())
    }

    fn restyle(
        &mut self,
        sheet: usize,
        range: (CellRef, CellRef),
        change: impl Fn(&CellStyle, CellRef) -> Result<CellStyle, EditError>,
    ) -> Result<(), EditError> {
        let area = rectangle(range)?;
        if sheet >= self.sheets.len() {
            return Err(EditError::NoSuchSheet(sheet));
        }
        let mut changes = Vec::new();
        for cell in cells_of(area) {
            let current = self.style(sheet, cell).clone();
            let updated = change(&current, cell)?;
            if updated != current {
                changes.push((cell, updated));
            }
        }
        let added = changes
            .iter()
            .filter(|(cell, _)| !self.sheets[sheet].styles.contains_key(cell))
            .count();
        if self.styled_count() + added > MAX_STYLED_CELLS {
            return Err(EditError::OutOfBounds);
        }
        for (cell, style) in changes {
            let id = self.intern_style(style);
            if id == 0 {
                self.sheets[sheet].styles.remove(&cell);
            } else {
                self.sheets[sheet].styles.insert(cell, id);
            }
        }
        Ok(())
    }

    /// The merged ranges of a sheet.
    #[must_use]
    pub fn merges(&self, sheet: usize) -> &[CellRange] {
        self.sheets
            .get(sheet)
            .map_or(&[], |sheet| sheet.merges.as_slice())
    }

    /// Merge a rectangle into one cell, as a spreadsheet does: the merges it overlaps are
    /// replaced, and the content of every cell but the top-left one is cleared.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::OutOfBounds`] past the grid or [`MAX_MERGES`];
    /// a single cell is not merged.
    pub fn merge_cells(
        &mut self,
        sheet: usize,
        range: (CellRef, CellRef),
    ) -> Result<(), EditError> {
        let (start, end) = range;
        let merged = CellRange {
            first_row: start.row.min(end.row),
            first_column: start.column.min(end.column),
            last_row: start.row.max(end.row),
            last_column: start.column.max(end.column),
        };
        if merged.last_row >= MAX_ROWS || merged.last_column >= MAX_COLUMNS {
            return Err(EditError::OutOfBounds);
        }
        let target = self
            .sheets
            .get_mut(sheet)
            .ok_or(EditError::NoSuchSheet(sheet))?;
        if merged.first_row == merged.last_row && merged.first_column == merged.last_column {
            return Ok(());
        }
        target.merges.retain(|merge| !overlaps(merge, &merged));
        if target.merges.len() >= MAX_MERGES {
            return Err(EditError::OutOfBounds);
        }
        target.merges.push(merged);
        let origin = CellRef::new(merged.first_row, merged.first_column);
        target
            .cells
            .retain(|cell, _| *cell == origin || !merged_contains(&merged, *cell));
        self.recalculate();
        Ok(())
    }

    /// Split the merged ranges a rectangle overlaps.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`].
    pub fn unmerge_cells(
        &mut self,
        sheet: usize,
        range: (CellRef, CellRef),
    ) -> Result<(), EditError> {
        let (start, end) = range;
        let area = CellRange {
            first_row: start.row.min(end.row),
            first_column: start.column.min(end.column),
            last_row: start.row.max(end.row),
            last_column: start.column.max(end.column),
        };
        let target = self
            .sheets
            .get_mut(sheet)
            .ok_or(EditError::NoSuchSheet(sheet))?;
        target.merges.retain(|merge| !overlaps(merge, &area));
        Ok(())
    }

    /// Set the height of a row, in points: 0 hides it, `None` lets it fit its content.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::OutOfBounds`] past the grid.
    pub fn set_row_height(
        &mut self,
        sheet: usize,
        row: u32,
        height: Option<f64>,
    ) -> Result<(), EditError> {
        if row >= MAX_ROWS {
            return Err(EditError::OutOfBounds);
        }
        let target = self
            .sheets
            .get_mut(sheet)
            .ok_or(EditError::NoSuchSheet(sheet))?;
        match height.filter(|height| height.is_finite()) {
            Some(height) => {
                target
                    .row_heights
                    .insert(row, height.clamp(0.0, MAX_ROW_HEIGHT));
            }
            None => {
                target.row_heights.remove(&row);
            }
        }
        Ok(())
    }

    /// The number format a cell is shown with: its own, else the code of its style.
    pub(crate) fn shown_format(&self, sheet: usize, cell: CellRef) -> (NumberFormat, Option<&str>) {
        let format = self
            .sheets
            .get(sheet)
            .and_then(|sheet| sheet.cells.get(&cell))
            .map_or(NumberFormat::General, |content| content.format);
        let code = (format == NumberFormat::General)
            .then(|| self.style(sheet, cell).number_format.as_deref())
            .flatten();
        (format, code)
    }

    /// Drop the number format code of a cell's style, so a format set on the cell shows.
    pub(crate) fn drop_format_code(&mut self, sheet: usize, cell: CellRef) {
        let style = self.style(sheet, cell);
        if style.number_format.is_none() {
            return;
        }
        let changed = CellStyle {
            number_format: None,
            ..style.clone()
        };
        let id = self.intern_style(changed);
        if id == 0 {
            self.sheets[sheet].styles.remove(&cell);
        } else {
            self.sheets[sheet].styles.insert(cell, id);
        }
    }
}

fn merged_contains(range: &CellRange, cell: CellRef) -> bool {
    range.contains(cell.row, cell.column)
}

#[cfg(test)]
mod tests;
