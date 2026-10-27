// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Character, paragraph and table-row properties ([MS-DOC] 2.6.1, 2.6.2 and 2.6.3), built by
//! applying modifiers over the ones a style gives.

use super::sprm::{byte, long, modifiers, signed_word, word};
use crate::document::{Border, BorderStyle};

/// A toggle's operand: on, off, the style's value, or its opposite.
const TOGGLE_AS_STYLE: u8 = 0x80;
const TOGGLE_OPPOSITE: u8 = 0x81;

/// The colours of `Ico`, the palette older properties name by index.
const PALETTE: [Option<&str>; 17] = [
    None,
    Some("000000"),
    Some("0000FF"),
    Some("00FFFF"),
    Some("00FF00"),
    Some("FF00FF"),
    Some("FF0000"),
    Some("FFFF00"),
    Some("FFFFFF"),
    Some("000080"),
    Some("008080"),
    Some("008000"),
    Some("800080"),
    Some("800000"),
    Some("808000"),
    Some("808080"),
    Some("C0C0C0"),
];

pub(super) fn palette_colour(index: u8) -> Option<String> {
    PALETTE
        .get(usize::from(index))
        .copied()
        .flatten()
        .map(str::to_owned)
}

/// A `COLORREF`: red, green, blue and a flag byte that marks the automatic colour.
pub(super) fn colour_reference(value: u32) -> Option<String> {
    let [red, green, blue, flag] = value.to_le_bytes();
    (flag != 0xFF).then(|| format!("{red:02X}{green:02X}{blue:02X}"))
}

/// Character properties.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is a property Word sets on its own"
)]
pub(super) struct Character {
    pub(super) bold: bool,
    pub(super) italic: bool,
    pub(super) strike: bool,
    pub(super) underline: bool,
    pub(super) caps: bool,
    pub(super) small_caps: bool,
    pub(super) hidden: bool,
    pub(super) superscript: bool,
    pub(super) subscript: bool,
    /// In half points.
    pub(super) size: Option<u16>,
    /// `RRGGBB`.
    pub(super) colour: Option<String>,
    pub(super) highlight: Option<String>,
    /// The font's index in the font table.
    pub(super) font: Option<u16>,
    /// The run's characters are special: a picture, a note reference, a drawing anchor.
    pub(super) special: bool,
    /// Where a picture's data lies in the Data stream.
    pub(super) picture: Option<u32>,
    /// The run is an embedded object, or holds form field data, not a picture.
    pub(super) object: bool,
    pub(super) deleted: bool,
    pub(super) inserted: bool,
    pub(super) style: Option<u16>,
}

impl Character {
    /// The properties after the modifiers, a toggle read against the style's value.
    pub(super) fn apply(&mut self, grpprl: &[u8], style: &Self) {
        let toggle = |operand: &[u8], style_value: bool| match byte(operand) {
            0 => false,
            TOGGLE_AS_STYLE => style_value,
            TOGGLE_OPPOSITE => !style_value,
            _ => true,
        };
        for (code, operand) in modifiers(grpprl) {
            match code {
                0x0835 => self.bold = toggle(operand, style.bold),
                0x0836 => self.italic = toggle(operand, style.italic),
                0x0837 | 0x2A53 => self.strike = toggle(operand, style.strike),
                0x083A => self.small_caps = toggle(operand, style.small_caps),
                0x083B => self.caps = toggle(operand, style.caps),
                0x083C => self.hidden = toggle(operand, style.hidden),
                0x2A3E => self.underline = byte(operand) != 0,
                0x2A48 => {
                    self.superscript = byte(operand) == 1;
                    self.subscript = byte(operand) == 2;
                }
                0x4A43 => self.size = Some(word(operand)).filter(|size| *size > 0),
                0x2A42 => self.colour = palette_colour(byte(operand)),
                0x6870 => self.colour = colour_reference(long(operand)),
                0x2A0C => self.highlight = palette_colour(byte(operand)),
                0x4A4F | 0x4A3D => self.font = Some(word(operand)),
                0x0855 => self.special = byte(operand) != 0,
                0x6A03 => self.picture = Some(long(operand)),
                0x0806 | 0x080A | 0x0856 => self.object |= byte(operand) != 0,
                0x0800 => self.deleted = byte(operand) != 0,
                0x0801 => self.inserted = byte(operand) != 0,
                0x4A30 => self.style = Some(word(operand)),
                _ => {}
            }
        }
    }
}

/// How a paragraph's lines are spaced: in 240ths of a line when proportional, in twips
/// otherwise, negative for an exact height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct LineSpacing {
    pub(super) value: i16,
    pub(super) proportional: bool,
}

/// Paragraph properties.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is a property Word sets on its own"
)]
pub(super) struct Paragraph {
    /// The paragraph's style, by its index in the style sheet.
    pub(super) style: u16,
    /// 0 left, 1 centre, 2 right, 3 and above justified.
    pub(super) alignment: Option<u8>,
    pub(super) indent_left: Option<i16>,
    pub(super) indent_right: Option<i16>,
    pub(super) indent_first_line: Option<i16>,
    pub(super) space_before: Option<u16>,
    pub(super) space_after: Option<u16>,
    pub(super) line_spacing: Option<LineSpacing>,
    pub(super) page_break_before: bool,
    pub(super) in_table: bool,
    /// The paragraph ends a table row.
    pub(super) row_end: bool,
    /// How deep in tables the paragraph is, 1 for a table that is not nested.
    pub(super) table_depth: u32,
    pub(super) list_level: u8,
    /// The list override, 1-based; 0 is no list.
    pub(super) list_override: u16,
    pub(super) outline_level: Option<u8>,
    pub(super) shading: Option<String>,
    /// No space between this paragraph and a neighbour of the same style.
    pub(super) contextual_spacing: bool,
    /// The top, left, bottom and right borders.
    pub(super) borders: [Option<Border>; 4],
}

impl Paragraph {
    pub(super) fn apply(&mut self, grpprl: &[u8]) {
        for (code, operand) in modifiers(grpprl) {
            match code {
                0x2403 | 0x2461 => self.alignment = Some(byte(operand)),
                0x840F | 0x845E => self.indent_left = Some(signed_word(operand)),
                0x840E | 0x845D => self.indent_right = Some(signed_word(operand)),
                0x8411 | 0x8460 => self.indent_first_line = Some(signed_word(operand)),
                0xA413 => self.space_before = Some(word(operand)),
                0xA414 => self.space_after = Some(word(operand)),
                0x6412 => {
                    self.line_spacing = Some(LineSpacing {
                        value: signed_word(operand),
                        proportional: signed_word(operand.get(2..).unwrap_or_default()) != 0,
                    });
                }
                0x2407 => self.page_break_before = byte(operand) != 0,
                0x2416 => {
                    self.in_table = byte(operand) != 0;
                    if self.in_table && self.table_depth == 0 {
                        self.table_depth = 1;
                    }
                }
                0x2417 => self.row_end = byte(operand) != 0,
                0x6649 => self.table_depth = long(operand),
                0x260A => self.list_level = byte(operand).min(8),
                0x460B => self.list_override = word(operand),
                0x2640 => self.outline_level = Some(byte(operand)).filter(|level| *level < 9),
                0x442D => self.shading = shading_80(word(operand)),
                0xC64D => self.shading = shading(operand),
                0x246D => self.contextual_spacing = byte(operand) != 0,
                0x6424..=0x6427 => {
                    self.borders[usize::from(code - 0x6424)] = border_80(operand);
                }
                0xC64E..=0xC651 => {
                    self.borders[usize::from(code - 0xC64E)] = border(operand);
                }
                _ => {}
            }
        }
    }
}

/// A `SHD80`: foreground and background palette indexes and a pattern; the background fills.
pub(super) fn shading_80(value: u16) -> Option<String> {
    let background = u8::try_from((value >> 5) & 0x1F).unwrap_or(0);
    let pattern = value >> 10;
    if pattern == 0 {
        palette_colour(background)
    } else {
        palette_colour(u8::try_from(value & 0x1F).unwrap_or(0))
            .or_else(|| palette_colour(background))
    }
}

/// A `SHD`: foreground and background `COLORREF`s and a pattern.
pub(super) fn shading(operand: &[u8]) -> Option<String> {
    let background = colour_reference(long(operand.get(4..).unwrap_or_default()));
    let pattern = word(operand.get(8..).unwrap_or_default());
    if pattern == 0 || pattern == 0xFFFF {
        background
    } else {
        colour_reference(long(operand)).or(background)
    }
}

/// One cell of a table row as `sprmTDefTable` describes it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct CellDefinition {
    /// The left and right edges, in twips from the row's origin.
    pub(super) left: i16,
    pub(super) right: i16,
    /// The cell continues the one on its left.
    pub(super) merged: bool,
    /// The cell continues the one above it.
    pub(super) continues_above: bool,
    pub(super) bordered: bool,
    pub(super) fill: Option<String>,
}

/// A table row's properties.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Row {
    pub(super) cells: Vec<CellDefinition>,
    pub(super) header: bool,
    pub(super) bordered: bool,
}

impl Row {
    pub(super) fn read(grpprl: &[u8]) -> Self {
        let mut row = Self::default();
        let mut fills: Vec<Option<String>> = Vec::new();
        for (code, operand) in modifiers(grpprl) {
            match code {
                0xD608 => row.cells = cell_definitions(operand),
                0x3404 => row.header = byte(operand) != 0,
                0xD605 => row.bordered |= operand.chunks(4).any(|border| border_drawn(border, 1)),
                0xD613 => row.bordered |= operand.chunks(8).any(|border| border_drawn(border, 5)),
                0xD609 => {
                    fills = operand
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|pair| shading_80(u16::from_le_bytes(*pair)))
                        .collect();
                }
                0xD612 => fills = operand.chunks(10).map(shading).collect(),
                _ => {}
            }
        }
        for (cell, fill) in row.cells.iter_mut().zip(fills) {
            cell.fill = fill;
        }
        row
    }
}

/// The line a `BRC80` describes: its width in eighths of a point, its type and its palette
/// colour; `None` for no line.
fn border_80(operand: &[u8]) -> Option<Border> {
    border_drawn(operand, 1).then(|| Border {
        style: border_style(operand.get(1).copied().unwrap_or(1)),
        width_eighths: u32::from(byte(operand)).max(2),
        colour: palette_colour(operand.get(2).copied().unwrap_or(0)),
    })
}

/// The line a `BRC` describes: its `COLORREF`, width and type.
fn border(operand: &[u8]) -> Option<Border> {
    border_drawn(operand, 5).then(|| Border {
        style: border_style(operand.get(5).copied().unwrap_or(1)),
        width_eighths: u32::from(operand.get(4).copied().unwrap_or(4)).max(2),
        colour: colour_reference(long(operand)),
    })
}

fn border_style(kind: u8) -> BorderStyle {
    match kind {
        3 | 10..=12 => BorderStyle::Double,
        6 | 20 => BorderStyle::Dotted,
        7..=9 | 22 | 23 => BorderStyle::Dashed,
        _ => BorderStyle::Single,
    }
}

/// Whether a `BRC80` or a `BRC` draws a line: its type byte names one, and it is not the nil
/// border of all ones.
fn border_drawn(border: &[u8], type_offset: usize) -> bool {
    let kind = border.get(type_offset).copied().unwrap_or(0);
    kind != 0 && kind != 0xFF
}

/// The `TDefTableOperand`: the cell count, `count + 1` edges and a `TC80` of twenty bytes
/// for each cell.
fn cell_definitions(operand: &[u8]) -> Vec<CellDefinition> {
    let count = usize::from(byte(operand)).min(63);
    let edge = |index: usize| signed_word(operand.get(1 + index * 2..).unwrap_or_default());
    let cells_start = 1 + (count + 1) * 2;
    (0..count)
        .map(|index| {
            let cell = operand
                .get(cells_start + index * 20..cells_start + (index + 1) * 20)
                .unwrap_or_default();
            let flags = word(cell);
            CellDefinition {
                left: edge(index),
                right: edge(index + 1),
                merged: flags & 0x0002 != 0,
                continues_above: flags & 0x0020 != 0 && flags & 0x0040 == 0,
                bordered: cell
                    .get(4..20)
                    .is_some_and(|borders| borders.chunks(4).any(|border| border_drawn(border, 1))),
                fill: None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
