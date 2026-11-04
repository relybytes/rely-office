// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! The looks of tables, cells and paragraphs in the JSON tree: the lines, fills, margins and
//! alignment a docx draws them with, so a tree an editor sends back is written as it was read.
//!
//! Colours cross as CSS colours, `#C00000`, as the marks of a run do; border widths in eighths of
//! a point and margins in twentieths of a point, as Word stores them. What comes in from JSON is
//! checked here, since the writer puts it in the file as it is: a colour that is not six hex
//! digits is left out (automatic), a width or a margin is brought within Word's limits.

use serde::{Deserialize, Serialize};

use super::edges::{Border, BorderStyle, Borders, CellLook, VerticalAlignment};
use super::format::ParagraphFormat;
use super::look::{ParagraphLook, TableLook};

/// Word draws borders from a quarter of a point to six points, in eighths.
const MIN_BORDER_EIGHTHS: u32 = 2;
const MAX_BORDER_EIGHTHS: u32 = 96;
const DEFAULT_BORDER_EIGHTHS: u32 = 4;
/// Word's own page limit, 22 inches: a longer margin is a crafted tree.
const MAX_TWIPS: u32 = 31_680;

/// A colour as the JSON tree carries it: `RRGGBB` as `#RRGGBB`.
fn css_colour(hex: &str) -> String {
    format!("#{hex}")
}

/// A colour from JSON, `#RRGGBB` or `RRGGBB`, as `RRGGBB`; anything else is left out.
fn parse_colour(colour: &str) -> Option<String> {
    let hex = colour.trim();
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    (hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit())).then(|| hex.to_ascii_uppercase())
}

/// One edge of a border: `{ style, widthEighths?, color? }`; a `none` edge, which hides the line
/// a style would draw, has no width.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BorderLine {
    style: BorderStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    width_eighths: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    color: Option<String>,
}

fn word_width(width_eighths: u32) -> u32 {
    width_eighths.clamp(MIN_BORDER_EIGHTHS, MAX_BORDER_EIGHTHS)
}

impl From<&Border> for BorderLine {
    fn from(border: &Border) -> Self {
        let drawn = border.style != BorderStyle::None;
        Self {
            style: border.style,
            width_eighths: drawn.then(|| word_width(border.width_eighths)),
            color: border.colour.as_deref().filter(|_| drawn).map(css_colour),
        }
    }
}

impl From<BorderLine> for Border {
    fn from(line: BorderLine) -> Self {
        Self {
            style: line.style,
            width_eighths: word_width(line.width_eighths.unwrap_or(DEFAULT_BORDER_EIGHTHS)),
            colour: line.color.as_deref().and_then(parse_colour),
        }
    }
}

fn line(border: Option<&Border>) -> Option<BorderLine> {
    border.map(BorderLine::from)
}

fn border(line: Option<BorderLine>) -> Option<Border> {
    line.map(Border::from)
}

/// The edges of a paragraph; `between` is the line between it and the next paragraph with the
/// same borders.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct ParagraphBorders {
    #[serde(skip_serializing_if = "Option::is_none")]
    top: Option<BorderLine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    left: Option<BorderLine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bottom: Option<BorderLine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    right: Option<BorderLine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    between: Option<BorderLine>,
}

impl ParagraphBorders {
    fn of(borders: &Borders) -> Option<Self> {
        let named = [
            &borders.top,
            &borders.left,
            &borders.bottom,
            &borders.right,
            &borders.inside_horizontal,
        ]
        .into_iter()
        .any(Option::is_some);
        named.then(|| Self {
            top: line(borders.top.as_ref()),
            left: line(borders.left.as_ref()),
            bottom: line(borders.bottom.as_ref()),
            right: line(borders.right.as_ref()),
            between: line(borders.inside_horizontal.as_ref()),
        })
    }

    fn into_borders(self) -> Borders {
        Borders {
            top: border(self.top),
            left: border(self.left),
            bottom: border(self.bottom),
            right: border(self.right),
            inside_horizontal: border(self.between),
            inside_vertical: None,
        }
    }
}

/// The edges of a table: its outer edges, and the lines between its rows and its columns.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct TableBorders {
    #[serde(skip_serializing_if = "Option::is_none")]
    top: Option<BorderLine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    left: Option<BorderLine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bottom: Option<BorderLine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    right: Option<BorderLine>,
    #[serde(rename = "insideH", skip_serializing_if = "Option::is_none")]
    inside_horizontal: Option<BorderLine>,
    #[serde(rename = "insideV", skip_serializing_if = "Option::is_none")]
    inside_vertical: Option<BorderLine>,
}

impl TableBorders {
    fn of(borders: &Borders) -> Option<Self> {
        (*borders != Borders::default()).then(|| Self {
            top: line(borders.top.as_ref()),
            left: line(borders.left.as_ref()),
            bottom: line(borders.bottom.as_ref()),
            right: line(borders.right.as_ref()),
            inside_horizontal: line(borders.inside_horizontal.as_ref()),
            inside_vertical: line(borders.inside_vertical.as_ref()),
        })
    }

    fn into_borders(self) -> Borders {
        Borders {
            top: border(self.top),
            left: border(self.left),
            bottom: border(self.bottom),
            right: border(self.right),
            inside_horizontal: border(self.inside_horizontal),
            inside_vertical: border(self.inside_vertical),
        }
    }
}

/// The edges of a cell, over the table's.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct CellBorders {
    #[serde(skip_serializing_if = "Option::is_none")]
    top: Option<BorderLine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    left: Option<BorderLine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bottom: Option<BorderLine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    right: Option<BorderLine>,
}

impl CellBorders {
    fn of(borders: &Borders) -> Option<Self> {
        let named = [&borders.top, &borders.left, &borders.bottom, &borders.right]
            .into_iter()
            .any(Option::is_some);
        named.then(|| Self {
            top: line(borders.top.as_ref()),
            left: line(borders.left.as_ref()),
            bottom: line(borders.bottom.as_ref()),
            right: line(borders.right.as_ref()),
        })
    }

    fn into_borders(self) -> Borders {
        Borders {
            top: border(self.top),
            left: border(self.left),
            bottom: border(self.bottom),
            right: border(self.right),
            inside_horizontal: None,
            inside_vertical: None,
        }
    }
}

/// The space between a cell's edges and its text, in twentieths of a point.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct CellMargins {
    #[serde(skip_serializing_if = "Option::is_none")]
    top: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    left: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bottom: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    right: Option<u32>,
}

impl CellMargins {
    /// From the look's top, left, bottom and right.
    fn of(margins: [Option<u32>; 4]) -> Option<Self> {
        let [top, left, bottom, right] = margins;
        margins.iter().any(Option::is_some).then_some(Self {
            top,
            left,
            bottom,
            right,
        })
    }

    fn into_margins(self) -> [Option<u32>; 4] {
        [self.top, self.left, self.bottom, self.right]
            .map(|margin| margin.map(|m| m.min(MAX_TWIPS)))
    }
}

/// The `format` of a paragraph, a heading or a list item: what the paragraph sets itself, with
/// the borders and shading it is drawn with.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct FormatJson {
    #[serde(flatten)]
    format: ParagraphFormat,
    #[serde(skip_serializing_if = "Option::is_none")]
    borders: Option<ParagraphBorders>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shading: Option<String>,
}

/// The `format` of a block's look in the JSON tree: written from the format and the look's
/// borders and shading, read into a look.
pub(crate) mod look_as_format {
    use super::{FormatJson, ParagraphBorders, ParagraphLook, css_colour, parse_colour};

    pub fn serialize<S: serde::Serializer>(
        look: &ParagraphLook,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(
            &FormatJson {
                format: look.format,
                borders: ParagraphBorders::of(&look.borders),
                shading: look.shading.as_deref().map(css_colour),
            },
            serializer,
        )
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<ParagraphLook, D::Error> {
        let json: FormatJson = serde::Deserialize::deserialize(deserializer)?;
        Ok(ParagraphLook {
            borders: json
                .borders
                .map(ParagraphBorders::into_borders)
                .unwrap_or_default(),
            shading: json.shading.as_deref().and_then(parse_colour),
            ..json.format.to_look()
        })
    }

    #[must_use]
    pub fn is_empty(look: &ParagraphLook) -> bool {
        look.format.is_empty()
            && ParagraphBorders::of(&look.borders).is_none()
            && look.shading.is_none()
    }
}

/// The look of a table, flattened into its block: `columnWidthsTwips`, `borders` and
/// `cellMarginsTwips`.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct TableLookJson {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    column_widths_twips: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    borders: Option<TableBorders>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cell_margins_twips: Option<CellMargins>,
}

pub(crate) mod table_look {
    use super::{CellMargins, TableBorders, TableLook, TableLookJson};

    pub fn serialize<S: serde::Serializer>(
        look: &TableLook,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(
            &TableLookJson {
                column_widths_twips: look.column_widths_twips.clone(),
                borders: TableBorders::of(&look.edges),
                cell_margins_twips: CellMargins::of(look.cell_margins_twips),
            },
            serializer,
        )
    }

    /// The look with its borders as the edges the JSON names: a side it leaves out draws no line.
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<TableLook, D::Error> {
        let json: TableLookJson = serde::Deserialize::deserialize(deserializer)?;
        Ok(TableLook {
            column_widths_twips: json.column_widths_twips,
            borders: false,
            cell_margins_twips: json
                .cell_margins_twips
                .map(CellMargins::into_margins)
                .unwrap_or_default(),
            edges: json
                .borders
                .map(TableBorders::into_borders)
                .unwrap_or_default(),
            frame: None,
        })
    }
}

/// The look of a cell, flattened into it: `fill`, `borders` and `verticalAlign`.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct CellLookJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    fill: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    borders: Option<CellBorders>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vertical_align: Option<VerticalAlignment>,
}

pub(crate) mod cell_look {
    use super::{CellBorders, CellLook, CellLookJson, css_colour, parse_colour};

    pub fn serialize<S: serde::Serializer>(
        look: &CellLook,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(
            &CellLookJson {
                fill: look.fill.as_deref().map(css_colour),
                borders: CellBorders::of(&look.borders),
                vertical_align: look.vertical,
            },
            serializer,
        )
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<CellLook, D::Error> {
        let json: CellLookJson = serde::Deserialize::deserialize(deserializer)?;
        Ok(CellLook {
            fill: json.fill.as_deref().and_then(parse_colour),
            borders: json
                .borders
                .map(CellBorders::into_borders)
                .unwrap_or_default(),
            vertical: json.vertical_align,
        })
    }
}

#[cfg(test)]
mod tests;
