// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The pictures and charts a worksheet's drawing anchors to its cells, each with the cell it
//! starts at and the size its anchor gives it.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Cursor;

use calamine::{Data, Range, Reader, Sheets, open_workbook_auto_from_rs};

use crate::archive::{ZipPackage, read_member_bytes, read_text_member};
use crate::chart::{CellSource, ChartCell, chart_picture};
use crate::document::sniff_image_type;
use crate::relationships::{Relationship, read_relationships};
use crate::sheet_layout::{CellRange, SheetLayout};
use crate::theme::Theme;
use crate::xml_tree::{Element, Namespace, parse_tree};
use crate::{Limits, OfficeError};

const XDR: Namespace = Namespace::SpreadsheetDrawing;
const A: Namespace = Namespace::Drawing;
const EMU_PER_PIXEL: i64 = 9_525;
/// A twentieth of a point.
const EMU_PER_TWIP: i64 = 635;
/// Excel's default column, 8.43 characters, is 64 pixels.
const DEFAULT_COLUMN_PIXELS: i64 = 64;
/// Excel's default row, 15 points, in twentieths.
const DEFAULT_ROW_TWIPS: i64 = 300;
const MAX_DRAWINGS: usize = 200;
/// No anchor reaches further than this, about a hundred metres.
const MAX_EMU: i64 = 914_400 * 4_000;

/// A picture or a chart placed on a sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetDrawing {
    /// The cell its top-left corner is anchored in, from 0.
    pub row: u32,
    pub column: u32,
    /// How far into that cell the corner sits, across and down.
    pub offset_emu: (i64, i64),
    pub width_emu: i64,
    pub height_emu: i64,
    pub content_type: &'static str,
    pub bytes: Vec<u8>,
    /// The description a picture carries, or a chart's title.
    pub description: Option<String>,
}

/// A part of a package kept as the file had it, to be written back unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarriedPart {
    /// The member name, `xl/charts/chart1.xml`.
    pub name: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

/// The drawing of a sheet with every part it leads to: its pictures, its charts and what they
/// embed, and the relationships between them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarriedDrawing {
    /// The member name of the drawing part the sheet points at.
    pub drawing_part: String,
    pub parts: Vec<CarriedPart>,
}

/// The most parts one drawing carries.
const MAX_CARRIED_PARTS: usize = 512;
const RELATIONSHIPS_TYPE: &str = "application/vnd.openxmlformats-package.relationships+xml";

/// The content types of a package: by part name, else by extension.
#[derive(Debug, Default)]
pub(crate) struct ContentTypes {
    defaults: HashMap<String, String>,
    overrides: HashMap<String, String>,
}

impl ContentTypes {
    pub(crate) fn parse(xml: &str) -> Self {
        let mut types = Self::default();
        let Ok(root) = parse_tree(xml, |_, _| false) else {
            return types;
        };
        for entry in root.elements() {
            let attribute = |name: &str| entry.attribute(Namespace::None, name);
            match (entry.name.as_str(), attribute("ContentType")) {
                ("Default", Some(kind)) => {
                    if let Some(extension) = attribute("Extension") {
                        types
                            .defaults
                            .insert(extension.to_ascii_lowercase(), kind.to_owned());
                    }
                }
                ("Override", Some(kind)) => {
                    if let Some(part) = attribute("PartName") {
                        types.overrides.insert(
                            part.trim_start_matches('/').to_ascii_lowercase(),
                            kind.to_owned(),
                        );
                    }
                }
                _ => {}
            }
        }
        types
    }

    fn of(&self, name: &str) -> String {
        let lower = name.to_ascii_lowercase();
        self.overrides
            .get(&lower)
            .or_else(|| {
                lower
                    .rsplit_once('.')
                    .and_then(|(_, extension)| self.defaults.get(extension))
            })
            .cloned()
            .unwrap_or_else(|| "application/octet-stream".to_owned())
    }
}

/// The relationships part of a part, `xl/drawings/_rels/drawing1.xml.rels`.
fn relationships_part(name: &str) -> String {
    match name.rsplit_once('/') {
        Some((folder, file)) => format!("{folder}/_rels/{file}.rels"),
        None => format!("_rels/{name}.rels"),
    }
}

/// The drawing of a worksheet part and every part it reaches, read as bytes; `None` for a sheet
/// without one, or one too large to carry whole.
pub(crate) fn carried_drawing(
    package: &mut ZipPackage<'_>,
    sheet_part: &str,
    limits: &Limits,
    types: &ContentTypes,
) -> Option<CarriedDrawing> {
    let drawing_part = read_relationships(package, sheet_part, limits)
        .ok()?
        .into_iter()
        .find(|relationship| !relationship.external && relationship.is_kind("drawing"))?
        .target;
    let mut parts = Vec::new();
    let mut seen = HashSet::from([drawing_part.clone()]);
    let mut queue = VecDeque::from([drawing_part.clone()]);
    let mut total: u64 = 0;
    while let Some(name) = queue.pop_front() {
        let bytes = read_member_bytes(package, &name, limits.max_member_bytes).ok()??;
        total += bytes.len() as u64;
        parts.push(CarriedPart {
            content_type: types.of(&name),
            name: name.clone(),
            bytes,
        });
        let relationships_name = relationships_part(&name);
        if let Some(bytes) =
            read_member_bytes(package, &relationships_name, limits.max_member_bytes)
                .ok()
                .flatten()
        {
            total += bytes.len() as u64;
            parts.push(CarriedPart {
                name: relationships_name,
                content_type: RELATIONSHIPS_TYPE.to_owned(),
                bytes,
            });
            for relationship in read_relationships(package, &name, limits).ok()? {
                if !relationship.external && seen.insert(relationship.target.clone()) {
                    queue.push_back(relationship.target);
                }
            }
        }
        if parts.len() > MAX_CARRIED_PARTS || total > limits.max_input_bytes {
            return None;
        }
    }
    Some(CarriedDrawing {
        drawing_part,
        parts,
    })
}

/// How much image data a workbook still may load.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ImageBudget {
    pub images: usize,
    pub bytes: u64,
}

impl ImageBudget {
    pub(crate) fn of(limits: &Limits) -> Self {
        Self {
            images: limits.max_images,
            bytes: limits.max_total_image_bytes,
        }
    }

    fn take(&mut self, size: u64) -> bool {
        if self.images == 0 || size > self.bytes {
            return false;
        }
        self.images -= 1;
        self.bytes -= size;
        true
    }
}

/// The most cells one chart formula reads.
const MAX_FORMULA_CELLS: u64 = 16_384;

/// The workbook behind the cells, opened on first use.
enum Opened<'b> {
    NotYet,
    Open(Box<Sheets<Cursor<&'b [u8]>>>),
    Refused,
}

/// The cells of a workbook, for the charts that cache none of the values they plot: the
/// workbook is opened on the first such chart, and each sheet read once.
pub(crate) struct WorkbookCells<'b> {
    bytes: &'b [u8],
    workbook: RefCell<Opened<'b>>,
    ranges: RefCell<HashMap<String, Option<Range<Data>>>>,
}

impl<'b> WorkbookCells<'b> {
    pub(crate) fn new(bytes: &'b [u8]) -> Self {
        Self {
            bytes,
            workbook: RefCell::new(Opened::NotYet),
            ranges: RefCell::new(HashMap::new()),
        }
    }

    /// The cells of a formula naming one range of one sheet, row by row.
    pub(crate) fn cells(&self, formula: &str) -> Option<Vec<ChartCell>> {
        let (sheet, range) = split_formula(formula)?;
        let range = CellRange::parse(&range)?;
        let area = u64::from(range.last_row - range.first_row + 1)
            * u64::from(range.last_column - range.first_column + 1);
        if area > MAX_FORMULA_CELLS {
            return None;
        }
        let mut ranges = self.ranges.borrow_mut();
        if !ranges.contains_key(&sheet) {
            let mut workbook = self.workbook.borrow_mut();
            if matches!(*workbook, Opened::NotYet) {
                *workbook = open_workbook_auto_from_rs(Cursor::new(self.bytes))
                    .map_or(Opened::Refused, |opened| Opened::Open(Box::new(opened)));
            }
            let read = match &mut *workbook {
                Opened::Open(opened) => opened.worksheet_range(&sheet).ok(),
                Opened::NotYet | Opened::Refused => None,
            };
            ranges.insert(sheet.clone(), read);
        }
        let values = ranges.get(&sheet)?.as_ref()?;
        let mut cells = Vec::new();
        for row in range.first_row..=range.last_row {
            for column in range.first_column..=range.last_column {
                let cell = values
                    .get_value((row, column))
                    .map_or(ChartCell::Empty, |value| match value {
                        Data::Float(number) => ChartCell::Number(*number),
                        #[expect(
                            clippy::cast_precision_loss,
                            reason = "a cell's integer is plotted as a float"
                        )]
                        Data::Int(number) => ChartCell::Number(*number as f64),
                        Data::Bool(flag) => ChartCell::Number(f64::from(u8::from(*flag))),
                        Data::DateTime(moment) => ChartCell::Number(moment.as_f64()),
                        Data::String(text) | Data::DateTimeIso(text) | Data::DurationIso(text) => {
                            ChartCell::Text(text.clone())
                        }
                        Data::Error(_) | Data::Empty => ChartCell::Empty,
                    });
                cells.push(cell);
            }
        }
        Some(cells)
    }
}

/// The sheet and the range of a formula, `'Vendite'!$B$2:$B$7`; a formula of several ranges is
/// not one this reader follows.
fn split_formula(formula: &str) -> Option<(String, String)> {
    let formula = formula.trim().trim_start_matches('=');
    let formula = formula
        .strip_prefix('(')
        .and_then(|inner| inner.strip_suffix(')'))
        .unwrap_or(formula);
    if formula.matches('!').count() != 1 {
        return None;
    }
    let (sheet, range) = formula.split_once('!')?;
    let sheet = sheet
        .strip_prefix('\'')
        .and_then(|quoted| quoted.strip_suffix('\''))
        .map_or_else(|| sheet.to_owned(), |quoted| quoted.replace("''", "'"));
    Some((sheet, range.to_owned()))
}

/// The drawings of a worksheet part; a drawing that cannot be read is left out.
pub(crate) fn sheet_drawings(
    package: &mut ZipPackage<'_>,
    sheet_part: &str,
    layout: &SheetLayout,
    theme: &Theme,
    limits: &Limits,
    budget: &mut ImageBudget,
    cells: CellSource<'_>,
) -> Result<Vec<SheetDrawing>, OfficeError> {
    let sheet_relationships = read_relationships(package, sheet_part, limits)?;
    let Some(drawing_part) = sheet_relationships
        .iter()
        .find(|relationship| !relationship.external && relationship.is_kind("drawing"))
        .map(|relationship| relationship.target.clone())
    else {
        return Ok(Vec::new());
    };
    let Some(xml) = read_text_member(package, &drawing_part, limits)? else {
        return Ok(Vec::new());
    };
    let root = parse_tree(&xml, |_, _| false)?;
    drop(xml);
    let relationships = read_relationships(package, &drawing_part, limits)?;
    let mut drawings = Vec::new();
    for anchor in root.elements().filter(|anchor| anchor.namespace == XDR) {
        if drawings.len() >= MAX_DRAWINGS {
            break;
        }
        let Some(AnchorFrame {
            row,
            column,
            offset_emu,
            width_emu,
            height_emu,
        }) = anchor_frame(anchor, layout)
        else {
            continue;
        };
        if width_emu <= 0 || height_emu <= 0 {
            continue;
        }
        let drawn = if let Some(picture) = anchor.child(XDR, "pic") {
            picture_of(package, picture, &relationships, limits, budget)
        } else if let Some(chart) = anchor
            .child(XDR, "graphicFrame")
            .and_then(|frame| frame.descendant(Namespace::Chart, "chart"))
        {
            let frame = (width_emu, height_emu);
            chart_of(
                package,
                chart,
                &relationships,
                theme,
                limits,
                budget,
                frame,
                cells,
            )
        } else {
            None
        };
        if let Some((content_type, bytes, description)) = drawn {
            drawings.push(SheetDrawing {
                row,
                column,
                offset_emu,
                width_emu,
                height_emu,
                content_type,
                bytes,
                description,
            });
        }
    }
    Ok(drawings)
}

/// Where an anchor puts its drawing: the cell it starts in, how far into it, and the size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AnchorFrame {
    row: u32,
    column: u32,
    offset_emu: (i64, i64),
    width_emu: i64,
    height_emu: i64,
}

/// The cell an anchor starts in, how far into it, and the size it gives its drawing.
fn anchor_frame(anchor: &Element, layout: &SheetLayout) -> Option<AnchorFrame> {
    let marker = |name: &str| {
        let marker = anchor.child(XDR, name)?;
        let number = |name: &str| {
            marker
                .child(XDR, name)
                .map(Element::text)
                .and_then(|text| text.trim().parse::<i64>().ok())
        };
        Some((
            number("col")?.clamp(0, 16_383),
            number("colOff").unwrap_or(0).clamp(0, MAX_EMU),
            number("row")?.clamp(0, 1_048_575),
            number("rowOff").unwrap_or(0).clamp(0, MAX_EMU),
        ))
    };
    let extent = || {
        let extent = anchor.child(XDR, "ext")?;
        let side = |name: &str| {
            extent
                .attribute(Namespace::None, name)?
                .parse::<i64>()
                .ok()
                .map(|side| side.clamp(0, MAX_EMU))
        };
        Some((side("cx")?, side("cy")?))
    };
    let cell = |row: i64, column: i64| {
        (
            u32::try_from(row).unwrap_or(0),
            u32::try_from(column).unwrap_or(0),
        )
    };
    match anchor.name.as_str() {
        "twoCellAnchor" => {
            let (from_column, from_column_offset, from_row, from_row_offset) = marker("from")?;
            let (to_column, to_column_offset, to_row, to_row_offset) = marker("to")?;
            let width =
                columns_emu(layout, from_column, to_column) - from_column_offset + to_column_offset;
            let height = rows_emu(layout, from_row, to_row) - from_row_offset + to_row_offset;
            let (row, column) = cell(from_row, from_column);
            Some(AnchorFrame {
                row,
                column,
                offset_emu: (from_column_offset, from_row_offset),
                width_emu: width.min(MAX_EMU),
                height_emu: height.min(MAX_EMU),
            })
        }
        "oneCellAnchor" => {
            let (column, column_offset, row, row_offset) = marker("from")?;
            let (width, height) = extent()?;
            let (row, column) = cell(row, column);
            Some(AnchorFrame {
                row,
                column,
                offset_emu: (column_offset, row_offset),
                width_emu: width,
                height_emu: height,
            })
        }
        "absoluteAnchor" => {
            let (width, height) = extent()?;
            let position = anchor.child(XDR, "pos");
            let coordinate = |name: &str| {
                position
                    .and_then(|position| position.attribute(Namespace::None, name))
                    .and_then(|value| value.parse::<i64>().ok())
                    .map_or(0, |value| value.clamp(0, MAX_EMU))
            };
            Some(AnchorFrame {
                row: 0,
                column: 0,
                offset_emu: (coordinate("x"), coordinate("y")),
                width_emu: width,
                height_emu: height,
            })
        }
        _ => None,
    }
}

/// The width of the columns from `first` up to, not including, `last`.
fn columns_emu(layout: &SheetLayout, first: i64, last: i64) -> i64 {
    let default = layout
        .default_column_width
        .map_or(DEFAULT_COLUMN_PIXELS, pixels_of_width);
    (first..last.max(first))
        .take(16_384)
        .map(|column| {
            usize::try_from(column)
                .ok()
                .and_then(|column| layout.column_widths.get(column).copied().flatten())
                .map_or(default, pixels_of_width)
                * EMU_PER_PIXEL
        })
        .sum()
}

/// The height of the rows from `first` up to, not including, `last`.
fn rows_emu(layout: &SheetLayout, first: i64, last: i64) -> i64 {
    let default = layout
        .default_row_height
        .map_or(DEFAULT_ROW_TWIPS, i64::from);
    let span = last.max(first) - first;
    let set: i64 = layout
        .row_heights
        .range(u32::try_from(first).unwrap_or(0)..u32::try_from(last.max(first)).unwrap_or(0))
        .map(|(_, height)| i64::from(*height) - default)
        .sum();
    (span * default + set) * EMU_PER_TWIP
}

/// The pixels of a column `width` 256ths of a character wide, with the 5 pixels of padding Excel
/// adds to the digits of its default font.
fn pixels_of_width(width: u32) -> i64 {
    if width == 0 {
        return 0;
    }
    (i64::from(width) * 7 + 128) / 256 + 5
}

fn picture_of(
    package: &mut ZipPackage<'_>,
    picture: &Element,
    relationships: &[Relationship],
    limits: &Limits,
    budget: &mut ImageBudget,
) -> Option<(&'static str, Vec<u8>, Option<String>)> {
    let embed = picture
        .descendant(A, "blip")?
        .attribute(Namespace::Relationships, "embed")?;
    let target = &relationships
        .iter()
        .find(|relationship| relationship.id == embed && !relationship.external)?
        .target;
    let cap = limits.max_image_bytes.min(limits.max_member_bytes);
    let bytes = read_member_bytes(package, target, cap).ok()??;
    if !budget.take(bytes.len() as u64) {
        return None;
    }
    let description = picture
        .descendant(XDR, "cNvPr")
        .and_then(|properties| {
            properties
                .attribute(Namespace::None, "descr")
                .or_else(|| properties.attribute(Namespace::None, "title"))
        })
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned);
    Some((sniff_image_type(&bytes), bytes, description))
}

#[expect(
    clippy::too_many_arguments,
    reason = "a chart needs its part, the workbook's theme and limits and its own frame"
)]
fn chart_of(
    package: &mut ZipPackage<'_>,
    chart: &Element,
    relationships: &[Relationship],
    theme: &Theme,
    limits: &Limits,
    budget: &mut ImageBudget,
    (width_emu, height_emu): (i64, i64),
    cells: CellSource<'_>,
) -> Option<(&'static str, Vec<u8>, Option<String>)> {
    let id = chart.attribute(Namespace::Relationships, "id")?;
    let target = &relationships
        .iter()
        .find(|relationship| relationship.id == id && !relationship.external)?
        .target;
    let xml = read_text_member(package, target, limits).ok().flatten()?;
    let (bytes, title) = chart_picture(&xml, theme, width_emu, height_emu, cells)?;
    if !budget.take(bytes.len() as u64) {
        return None;
    }
    Some(("image/svg+xml", bytes, title))
}

#[cfg(test)]
mod tests;
