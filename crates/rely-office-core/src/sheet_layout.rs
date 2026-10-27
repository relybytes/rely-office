// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! How the sheets of an xlsx are printed and how their cells look: orientation, margins, column
//! widths, row heights, header and footer, grid lines, the default font, the format of each cell
//! and the merged cells. The worksheet parts are read as a stream that skips the values, which
//! the spreadsheet reader has already read.

use std::collections::{BTreeMap, HashMap};

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::archive::{open_package, read_member_bytes, read_text_member};
use crate::relationships::{main_part_path, read_relationships};
use crate::sheet_drawings::{
    CarriedDrawing, CarriedPart, CarriedPivot, ContentTypes, ImageBudget, SheetDrawing,
    WorkbookCells, carried_drawing, carried_pivots, sheet_drawings,
};
use crate::sheet_rules::{
    ConditionalFormat, DataValidation, MAX_SHEET_RULES, read_conditional_formatting,
    read_data_validation,
};
use crate::sheet_styles::{
    CellStyle, ThemeColours, cell_styles, default_theme, differential_looks, theme_colours,
};
use crate::theme::Theme;
use crate::xml_text::attribute_value;
use crate::{Limits, OfficeError};

const WORKBOOK_PART: &str = "xl/workbook.xml";
/// The most columns given a width; Excel's own limit is 16,384.
const MAX_WIDTH_COLUMNS: u32 = 16_384;
/// Excel's grid.
const MAX_ROWS: u32 = 1_048_576;
/// The most cells of a sheet whose format is kept, and the most merges.
const MAX_STYLED_CELLS: usize = 1_000_000;
const MAX_MERGES: usize = 100_000;
/// Excel's tallest row, 409 points, in twentieths.
const MAX_ROW_HEIGHT: u32 = 8_180;

/// A rectangle of cells, rows and columns counted from 0, both ends included.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct CellRange {
    pub first_row: u32,
    pub first_column: u32,
    pub last_row: u32,
    pub last_column: u32,
}

impl CellRange {
    /// A range in A1 notation, `B2` or `A1:C3`; `None` for anything else.
    #[must_use]
    pub fn parse(reference: &str) -> Option<Self> {
        let (first, last) = reference.split_once(':').unwrap_or((reference, reference));
        let (first_row, first_column) = parse_cell(first)?;
        let (last_row, last_column) = parse_cell(last)?;
        Some(Self {
            first_row: first_row.min(last_row),
            first_column: first_column.min(last_column),
            last_row: first_row.max(last_row),
            last_column: first_column.max(last_column),
        })
    }

    /// Whether the range holds the cell.
    #[must_use]
    pub fn contains(&self, row: u32, column: u32) -> bool {
        (self.first_row..=self.last_row).contains(&row)
            && (self.first_column..=self.last_column).contains(&column)
    }
}

/// A cell in A1 notation as its row and column from 0, `$` signs allowed.
fn parse_cell(reference: &str) -> Option<(u32, u32)> {
    let reference = reference.trim().replace('$', "");
    let split = reference.find(|character: char| character.is_ascii_digit())?;
    let (letters, digits) = reference.split_at(split);
    if letters.is_empty() || letters.len() > 3 || !letters.chars().all(|c| c.is_ascii_alphabetic())
    {
        return None;
    }
    let column = letters.chars().fold(0u32, |column, letter| {
        column * 26 + u32::from(letter.to_ascii_uppercase() as u8 - b'A') + 1
    }) - 1;
    let row = digits.parse::<u32>().ok()?.checked_sub(1)?;
    (row < MAX_ROWS && column < MAX_WIDTH_COLUMNS).then_some((row, column))
}

/// The print settings of a sheet; unset means the defaults of the suite that prints it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SheetLayout {
    pub landscape: bool,
    /// Page margins in thousandths of an inch: top, right, bottom, left, then the distance of the
    /// header from the top and of the footer from the bottom.
    pub margins_mils: Option<[u32; 6]>,
    /// The width of each column from A in 256ths of a character, 0 for a hidden column; a column
    /// past the end, or `None`, has the default width.
    pub column_widths: Vec<Option<u32>>,
    /// The default column width in 256ths of a character.
    pub default_column_width: Option<u32>,
    /// The header and footer as Excel codes them (`&C&A`, `&CPage &P`).
    pub header: Option<String>,
    pub footer: Option<String>,
    pub grid_lines: bool,
    /// The family and size, in tenths of a point, of the workbook's default font.
    pub font_family: Option<String>,
    pub font_size_tenths: Option<u32>,
    /// The cell formats of the workbook, which the cell, row and column formats index.
    pub styles: Vec<CellStyle>,
    /// The format of each cell that names one other than the first, by row and column from 0.
    pub cell_styles: HashMap<(u32, u32), u32>,
    /// The format of the rows that give one to their cells.
    pub row_styles: BTreeMap<u32, u32>,
    /// The format of columns that give one to their cells: first column, last column, format.
    pub column_styles: Vec<(u32, u32, u32)>,
    pub merges: Vec<CellRange>,
    /// The height of each row that sets one, in twentieths of a point; 0 for a hidden row.
    pub row_heights: BTreeMap<u32, u32>,
    /// The height of the other rows, in twentieths of a point.
    pub default_row_height: Option<u32>,
    /// The pictures and charts placed on the sheet.
    pub drawings: Vec<SheetDrawing>,
    pub conditional_formats: Vec<ConditionalFormat>,
    pub validations: Vec<DataValidation>,
}

impl SheetLayout {
    /// The look of a cell: its own format, else its row's, else its column's.
    #[must_use]
    pub fn style_of(&self, row: u32, column: u32) -> Option<&CellStyle> {
        let index = self
            .cell_styles
            .get(&(row, column))
            .or_else(|| self.row_styles.get(&row))
            .or_else(|| {
                self.column_styles
                    .iter()
                    .find(|(first, last, _)| (*first..=*last).contains(&column))
                    .map(|(_, _, style)| style)
            })?;
        self.styles.get(usize::try_from(*index).ok()?)
    }

    /// The merged range that starts at a cell, and whether a merge covers a cell without
    /// starting there.
    #[must_use]
    pub fn merge_at(&self, row: u32, column: u32) -> MergePlace {
        for merge in &self.merges {
            if merge.first_row == row && merge.first_column == column {
                return MergePlace::Start(*merge);
            }
            if merge.contains(row, column) {
                return MergePlace::Covered;
            }
        }
        MergePlace::None
    }
}

/// Where a cell stands among the merged ranges of its sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergePlace {
    None,
    /// The cell is the top-left one of the range and shows its content.
    Start(CellRange),
    /// The cell is inside a range another cell starts.
    Covered,
}

/// The layout of every sheet of an xlsx, by sheet name; a package it cannot read gives none, as
/// the sheets themselves were read by then and print with the defaults.
#[must_use]
pub fn read_xlsx_layouts(bytes: &[u8], limits: &Limits) -> HashMap<String, SheetLayout> {
    read_layouts(bytes, limits).unwrap_or_default()
}

fn read_layouts(
    bytes: &[u8],
    limits: &Limits,
) -> Result<HashMap<String, SheetLayout>, OfficeError> {
    let mut package = open_package(bytes)?;
    let workbook_part = main_part_path(&mut package, limits, WORKBOOK_PART)?;
    let Some(workbook) = read_text_member(&mut package, &workbook_part, limits)? else {
        return Ok(HashMap::new());
    };
    let relationships = read_relationships(&mut package, &workbook_part, limits)?;
    let part = |kind: &str| {
        relationships
            .iter()
            .find(|relationship| !relationship.external && relationship.is_kind(kind))
            .map(|relationship| relationship.target.clone())
    };
    let styles_xml = part("styles")
        .map(|target| read_text_member(&mut package, &target, limits))
        .transpose()?
        .flatten();
    let theme_xml = part("theme").and_then(|target| {
        read_text_member(&mut package, &target, limits)
            .ok()
            .flatten()
    });
    let theme = theme_xml
        .as_deref()
        .and_then(theme_colours)
        .unwrap_or_else(default_theme);
    let chart_theme = theme_xml.as_deref().map(Theme::parse).unwrap_or_default();
    let mut budget = ImageBudget::of(limits);
    let workbook_cells = WorkbookCells::new(bytes);
    let cells = |formula: &str| workbook_cells.cells(formula);
    let (font_family, font_size_tenths) =
        styles_xml.as_deref().map(default_font).unwrap_or_default();
    let styles = styles_xml
        .as_deref()
        .map(|xml| cell_styles(xml, &theme))
        .unwrap_or_default();
    let differentials = styles_xml
        .as_deref()
        .map(|xml| differential_looks(xml, &theme))
        .unwrap_or_default();

    let mut layouts = HashMap::new();
    for (name, relationship_id) in sheet_entries(&workbook)? {
        let Some(relationship) = relationships
            .iter()
            .find(|relationship| relationship.id == relationship_id && !relationship.external)
        else {
            continue;
        };
        let Some(sheet) = read_text_member(&mut package, &relationship.target, limits)? else {
            continue;
        };
        let (mut layout, looks) = sheet_layout(&sheet, &theme)?;
        drop(sheet);
        for (format, looks) in layout.conditional_formats.iter_mut().zip(looks) {
            for (rule, look) in format.rules.iter_mut().zip(looks) {
                rule.look = look
                    .and_then(|index| differentials.get(index))
                    .filter(|look| !look.is_empty())
                    .cloned();
            }
        }
        layout.drawings = sheet_drawings(
            &mut package,
            &relationship.target,
            &layout,
            &chart_theme,
            limits,
            &mut budget,
            &cells,
        )
        .unwrap_or_default();
        layout.font_family.clone_from(&font_family);
        layout.font_size_tenths = font_size_tenths;
        layout.styles.clone_from(&styles);
        layouts.insert(name, layout);
    }
    Ok(layouts)
}

/// The drawing of every sheet of an xlsx that has one, by sheet name, with each part it leads to,
/// for a writer to put back unchanged; a package it cannot read gives none.
#[must_use]
pub fn read_xlsx_drawing_parts(bytes: &[u8], limits: &Limits) -> HashMap<String, CarriedDrawing> {
    read_drawing_parts(bytes, limits).unwrap_or_default()
}

fn read_drawing_parts(
    bytes: &[u8],
    limits: &Limits,
) -> Result<HashMap<String, CarriedDrawing>, OfficeError> {
    let mut package = open_package(bytes)?;
    let types = read_text_member(&mut package, "[Content_Types].xml", limits)?
        .map(|xml| ContentTypes::parse(&xml))
        .unwrap_or_default();
    let workbook_part = main_part_path(&mut package, limits, WORKBOOK_PART)?;
    let Some(workbook) = read_text_member(&mut package, &workbook_part, limits)? else {
        return Ok(HashMap::new());
    };
    let relationships = read_relationships(&mut package, &workbook_part, limits)?;
    let mut drawings = HashMap::new();
    for (name, relationship_id) in sheet_entries(&workbook)? {
        let Some(relationship) = relationships
            .iter()
            .find(|relationship| relationship.id == relationship_id && !relationship.external)
        else {
            continue;
        };
        if let Some(drawing) = carried_drawing(&mut package, &relationship.target, limits, &types) {
            drawings.insert(name, drawing);
        }
    }
    Ok(drawings)
}

/// The pivot tables of every sheet of an xlsx that has some, by sheet name, with the caches they
/// read, for a writer to put back unchanged; a package it cannot read gives none.
#[must_use]
pub fn read_xlsx_pivot_parts(bytes: &[u8], limits: &Limits) -> HashMap<String, Vec<CarriedPivot>> {
    read_pivot_parts(bytes, limits).unwrap_or_default()
}

fn read_pivot_parts(
    bytes: &[u8],
    limits: &Limits,
) -> Result<HashMap<String, Vec<CarriedPivot>>, OfficeError> {
    let mut package = open_package(bytes)?;
    let types = read_text_member(&mut package, "[Content_Types].xml", limits)?
        .map(|xml| ContentTypes::parse(&xml))
        .unwrap_or_default();
    let workbook_part = main_part_path(&mut package, limits, WORKBOOK_PART)?;
    let Some(workbook) = read_text_member(&mut package, &workbook_part, limits)? else {
        return Ok(HashMap::new());
    };
    let relationships = read_relationships(&mut package, &workbook_part, limits)?;
    let caches: HashMap<String, u32> = pivot_cache_entries(&workbook)
        .into_iter()
        .filter_map(|(id, relationship_id)| {
            relationships
                .iter()
                .find(|relationship| relationship.id == relationship_id && !relationship.external)
                .map(|relationship| (relationship.target.clone(), id))
        })
        .collect();
    if caches.is_empty() {
        return Ok(HashMap::new());
    }
    let mut pivots = HashMap::new();
    for (name, relationship_id) in sheet_entries(&workbook)? {
        let Some(relationship) = relationships
            .iter()
            .find(|relationship| relationship.id == relationship_id && !relationship.external)
        else {
            continue;
        };
        let found = carried_pivots(&mut package, &relationship.target, &caches, limits, &types);
        if !found.is_empty() {
            pivots.insert(name, found);
        }
    }
    Ok(pivots)
}

/// The id and relationship id of each `pivotCache` of the workbook part.
fn pivot_cache_entries(workbook: &str) -> Vec<(u32, String)> {
    let mut reader = Reader::from_str(workbook);
    let mut entries = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(element) | Event::Empty(element))
                if element.local_name().as_ref() == "pivotCache" =>
            {
                if let (Some(id), Some(relationship)) = (
                    attribute(&element, "cacheId").and_then(|id| id.parse().ok()),
                    attribute_ending(&element, ":id"),
                ) {
                    entries.push((id, relationship));
                }
            }
            Ok(Event::Eof) | Err(_) => return entries,
            _ => {}
        }
    }
}

/// The theme part of an xlsx, which its charts take their colours from, for a writer to put back
/// unchanged.
#[must_use]
pub fn read_xlsx_theme_part(bytes: &[u8], limits: &Limits) -> Option<CarriedPart> {
    let mut package = open_package(bytes).ok()?;
    let workbook_part = main_part_path(&mut package, limits, WORKBOOK_PART).ok()?;
    let target = read_relationships(&mut package, &workbook_part, limits)
        .ok()?
        .into_iter()
        .find(|relationship| !relationship.external && relationship.is_kind("theme"))?
        .target;
    let bytes = read_member_bytes(&mut package, &target, limits.max_member_bytes).ok()??;
    Some(CarriedPart {
        name: target,
        content_type: "application/vnd.openxmlformats-officedocument.theme+xml".to_owned(),
        bytes,
    })
}

/// The name and relationship id of each `sheet` of the workbook part.
fn sheet_entries(workbook: &str) -> Result<Vec<(String, String)>, OfficeError> {
    let mut reader = Reader::from_str(workbook);
    let mut entries = Vec::new();
    loop {
        match reader.read_event().map_err(malformed)? {
            Event::Start(element) | Event::Empty(element)
                if element.local_name().as_ref() == "sheet" =>
            {
                if let (Some(name), Some(id)) = (
                    attribute(&element, "name"),
                    attribute_ending(&element, ":id"),
                ) {
                    entries.push((name, id));
                }
            }
            Event::Eof => return Ok(entries),
            _ => {}
        }
    }
}

/// The print settings of one worksheet part.
/// The layout of a worksheet part, with the differential format each rule of its conditional
/// formats names.
type ReadLayout = (SheetLayout, Vec<Vec<Option<usize>>>);

#[expect(
    clippy::too_many_lines,
    reason = "one pass over the worksheet for every setting it holds"
)]
fn sheet_layout(sheet: &str, theme: &ThemeColours) -> Result<ReadLayout, OfficeError> {
    let mut reader = Reader::from_str(sheet);
    let mut layout = SheetLayout::default();
    let mut looks = Vec::new();
    let mut reading: Option<bool> = None;
    // Inside the extensions only the validations Excel 2010 moved there are read: the rest
    // repeat, with additions, the formats already read.
    let mut extension_depth = 0_usize;
    loop {
        let event = reader.read_event().map_err(malformed)?;
        match &event {
            Event::Start(element) if element.local_name().as_ref() == "extLst" => {
                extension_depth += 1;
                continue;
            }
            Event::End(element) if element.local_name().as_ref() == "extLst" => {
                extension_depth = extension_depth.saturating_sub(1);
                continue;
            }
            Event::Start(element)
                if element.local_name().as_ref() == "conditionalFormatting"
                    && extension_depth == 0 =>
            {
                let element = element.clone().into_owned();
                if let Some(read) = read_conditional_formatting(&mut reader, &element, theme)
                    && layout.conditional_formats.len() < MAX_SHEET_RULES
                {
                    layout.conditional_formats.push(read.format);
                    looks.push(read.looks);
                }
                continue;
            }
            Event::Start(element) | Event::Empty(element)
                if element.local_name().as_ref() == "dataValidation" =>
            {
                let element = element.clone().into_owned();
                let empty = matches!(event, Event::Empty(_));
                if let Some(validation) = read_data_validation(&mut reader, &element, empty)
                    && layout.validations.len() < MAX_SHEET_RULES
                {
                    layout.validations.push(validation);
                }
                continue;
            }
            _ => {}
        }
        if extension_depth > 0 && !matches!(event, Event::Eof) {
            continue;
        }
        match event {
            Event::Start(element) | Event::Empty(element) => match element.local_name().as_ref() {
                "sheetFormatPr" => {
                    layout.default_column_width = attribute(&element, "defaultColWidth")
                        .and_then(|width| character_width(&width));
                    layout.default_row_height = attribute(&element, "defaultRowHeight")
                        .and_then(|height| row_height(&height));
                }
                "col" => apply_column(&mut layout, &element),
                "row" => apply_row(&mut layout, &element),
                "c" => apply_cell(&mut layout, &element),
                "mergeCell" if layout.merges.len() < MAX_MERGES => {
                    if let Some(range) = attribute(&element, "ref")
                        .and_then(|reference| CellRange::parse(&reference))
                    {
                        layout.merges.push(range);
                    }
                }
                "pageMargins" => layout.margins_mils = page_margins(&element),
                "pageSetup" => {
                    layout.landscape =
                        attribute(&element, "orientation").as_deref() == Some("landscape");
                }
                "printOptions" => {
                    layout.grid_lines = attribute(&element, "gridLines")
                        .is_some_and(|value| value == "1" || value == "true");
                }
                "oddHeader" => reading = Some(true),
                "oddFooter" => reading = Some(false),
                _ => {}
            },
            Event::Text(text) => {
                if let Some(header) = reading {
                    push_header_text(&mut layout, header, &text.xml10_content());
                }
            }
            Event::GeneralRef(reference) => {
                if let Some(header) = reading {
                    let resolved = match reference.resolve_char_ref() {
                        Ok(Some(character)) => Some(character.to_string()),
                        _ => quick_xml::escape::resolve_xml_entity(&reference).map(str::to_owned),
                    };
                    if let Some(resolved) = resolved {
                        push_header_text(&mut layout, header, &resolved);
                    }
                }
            }
            Event::End(element)
                if matches!(element.local_name().as_ref(), "oddHeader" | "oddFooter") =>
            {
                reading = None;
            }
            Event::Eof => return Ok((layout, looks)),
            _ => {}
        }
    }
}

fn push_header_text(layout: &mut SheetLayout, header: bool, text: &str) {
    let target = if header {
        &mut layout.header
    } else {
        &mut layout.footer
    };
    target.get_or_insert_with(String::new).push_str(text);
}

fn apply_column(layout: &mut SheetLayout, element: &BytesStart<'_>) {
    let number = |name: &str| {
        attribute(element, name)
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|value| (1..=MAX_WIDTH_COLUMNS).contains(value))
    };
    let (Some(min), Some(max)) = (number("min"), number("max")) else {
        return;
    };
    if max < min {
        return;
    }
    let hidden = attribute(element, "hidden").is_some_and(|value| value == "1" || value == "true");
    if let Some(style) = attribute(element, "style").and_then(|style| style.parse::<u32>().ok())
        && style != 0
    {
        layout.column_styles.push((min - 1, max - 1, style));
    }
    let width = if hidden {
        Some(0)
    } else {
        attribute(element, "width").and_then(|width| character_width(&width))
    };
    // Only the columns a sheet can show matter; a style for A:XFD would otherwise fill memory.
    let last = usize::try_from(max.min(min.saturating_add(255))).unwrap_or(0);
    if layout.column_widths.len() < last {
        layout.column_widths.resize(last, None);
    }
    for column in usize::try_from(min).unwrap_or(1)..=last {
        layout.column_widths[column - 1] = width;
    }
}

fn apply_row(layout: &mut SheetLayout, element: &BytesStart<'_>) {
    let Some(row) = attribute(element, "r")
        .and_then(|row| row.parse::<u32>().ok())
        .and_then(|row| row.checked_sub(1))
        .filter(|row| *row < MAX_ROWS)
    else {
        return;
    };
    let is_on =
        |name: &str| attribute(element, name).is_some_and(|value| value == "1" || value == "true");
    if is_on("hidden") {
        layout.row_heights.insert(row, 0);
    } else if is_on("customHeight")
        && let Some(height) = attribute(element, "ht").and_then(|height| row_height(&height))
    {
        layout.row_heights.insert(row, height);
    }
    if is_on("customFormat")
        && let Some(style) = attribute(element, "s").and_then(|style| style.parse::<u32>().ok())
    {
        layout.row_styles.insert(row, style);
    }
}

fn apply_cell(layout: &mut SheetLayout, element: &BytesStart<'_>) {
    if layout.cell_styles.len() >= MAX_STYLED_CELLS {
        return;
    }
    let (Some(style), Some((row, column))) = (
        attribute(element, "s").and_then(|style| style.parse::<u32>().ok()),
        attribute(element, "r").and_then(|reference| parse_cell(&reference)),
    ) else {
        return;
    };
    if style != 0 || layout.row_styles.contains_key(&row) {
        layout.cell_styles.insert((row, column), style);
    }
}

/// A row height in points as twentieths of a point.
fn row_height(height: &str) -> Option<u32> {
    let points = height.parse::<f64>().ok()?;
    if !(0.0..=409.5).contains(&points) {
        return None;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some(((points * 20.0).round() as u32).min(MAX_ROW_HEIGHT))
}

fn page_margins(element: &BytesStart<'_>) -> Option<[u32; 6]> {
    let mut margins = [0; 6];
    for (index, name) in ["top", "right", "bottom", "left", "header", "footer"]
        .into_iter()
        .enumerate()
    {
        let inches = attribute(element, name)?.parse::<f64>().ok()?;
        if !(0.0..=22.0).contains(&inches) {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let mils = (inches * 1000.0).round() as u32;
        margins[index] = mils;
    }
    Some(margins)
}

/// A width in characters as 256ths of a character, the unit Excel stores internally.
fn character_width(width: &str) -> Option<u32> {
    let width = width.parse::<f64>().ok()?;
    if !(0.0..=255.0).contains(&width) {
        return None;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some((width * 256.0).round() as u32)
}

/// The first font of the styles part, the one every cell without its own uses.
fn default_font(styles: &str) -> (Option<String>, Option<u32>) {
    let mut reader = Reader::from_str(styles);
    let mut in_fonts = false;
    let mut in_font = false;
    let (mut family, mut size) = (None, None);
    loop {
        match reader.read_event() {
            Ok(Event::Start(element) | Event::Empty(element)) => {
                match element.local_name().as_ref() {
                    "fonts" => in_fonts = true,
                    "font" if in_fonts => in_font = true,
                    "name" if in_font => family = attribute(&element, "val"),
                    "sz" if in_font => {
                        size = attribute(&element, "val")
                            .and_then(|value| value.parse::<f64>().ok())
                            .filter(|value| (1.0..=409.0).contains(value))
                            .map(|value| {
                                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                                let tenths = (value * 10.0).round() as u32;
                                tenths
                            });
                    }
                    _ => {}
                }
            }
            Ok(Event::End(element)) if element.local_name().as_ref() == "font" && in_font => {
                return (family, size);
            }
            Ok(Event::Eof) | Err(_) => return (family, size),
            _ => {}
        }
    }
}

fn attribute(element: &BytesStart<'_>, name: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.as_ref() == name)
        .and_then(|attribute| attribute_value(&attribute))
}

/// The value of the first attribute whose qualified name ends with `suffix` (`r:id` in any prefix).
fn attribute_ending(element: &BytesStart<'_>, suffix: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.as_ref().ends_with(suffix))
        .and_then(|attribute| attribute_value(&attribute))
}

fn malformed(error: impl std::fmt::Display) -> OfficeError {
    OfficeError::MalformedXml(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout_of(sheet: &str) -> Result<SheetLayout, OfficeError> {
        sheet_layout(sheet, &default_theme()).map(|(layout, _)| layout)
    }

    const SHEET: &str = concat!(
        r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#,
        r#"<sheetFormatPr defaultColWidth="11.5" defaultRowHeight="12.8"/>"#,
        r#"<cols><col min="1" max="1" width="9.92"/><col min="3" max="4" width="20"/>"#,
        r#"<col min="5" max="5" width="30" hidden="1"/><col min="0" max="2" width="3"/></cols>"#,
        r#"<sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>"#,
        r#"<printOptions gridLines="true"/>"#,
        r#"<pageMargins left="0.7875" right="0.7875" top="1.05" bottom="1" header="0.5" footer="0.4"/>"#,
        r#"<pageSetup paperSize="9" orientation="landscape"/>"#,
        r#"<headerFooter><oddHeader>&amp;C&amp;A</oddHeader><oddFooter>&amp;CPagina &amp;P</oddFooter></headerFooter>"#,
        "</worksheet>"
    );

    #[test]
    fn reads_the_print_settings_of_a_sheet() {
        let layout = layout_of(SHEET).unwrap();
        assert!(layout.landscape);
        assert!(layout.grid_lines);
        assert_eq!(
            layout.margins_mils,
            Some([1_050, 788, 1_000, 788, 500, 400])
        );
        assert_eq!(layout.default_column_width, Some(2_944));
        assert_eq!(
            layout.column_widths,
            vec![Some(2_540), None, Some(5_120), Some(5_120), Some(0)]
        );
        assert_eq!(layout.header.as_deref(), Some("&C&A"));
        assert_eq!(layout.footer.as_deref(), Some("&CPagina &P"));
    }

    #[test]
    fn keeps_the_defaults_for_what_a_sheet_does_not_say() {
        let layout =
            layout_of("<worksheet><pageMargins left=\"x\"/><col min=\"2\" max=\"1\"/></worksheet>")
                .unwrap();
        assert_eq!(layout, SheetLayout::default());
        assert!(layout_of("<worksheet><col").is_err());
    }

    #[test]
    fn caps_a_column_range_that_spans_the_whole_sheet() {
        let layout = layout_of(
            r#"<worksheet><cols><col min="1" max="16384" width="9"/></cols></worksheet>"#,
        )
        .unwrap();
        assert_eq!(layout.column_widths.len(), 256);
    }

    #[test]
    fn reads_the_default_font_and_the_sheet_entries() {
        let styles = r#"<styleSheet><fonts count="2"><font><sz val="10"/><name val="Arial"/></font><font><sz val="14"/><name val="Cambria"/></font></fonts></styleSheet>"#;
        assert_eq!(default_font(styles), (Some("Arial".to_owned()), Some(100)));
        assert_eq!(default_font("<styleSheet/>"), (None, None));
        let workbook = r#"<workbook xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Vendite" sheetId="1" r:id="rId3"/><sheet name="Senza id"/></sheets></workbook>"#;
        assert_eq!(
            sheet_entries(workbook).unwrap(),
            vec![("Vendite".to_owned(), "rId3".to_owned())]
        );
    }

    #[test]
    fn reads_row_heights_cell_formats_and_merges() {
        let layout = layout_of(concat!(
            r#"<worksheet><sheetFormatPr defaultRowHeight="15"/><cols><col min="2" max="3" width="9" style="7"/></cols>"#,
            r#"<sheetData><row r="1" ht="30" customHeight="1"><c r="A1" s="2"><v>1</v></c><c r="B1" s="0"/></row>"#,
            r#"<row r="2" hidden="1" s="5" customFormat="1"><c r="A2" s="0"/><c r="C2"/></row>"#,
            r#"<row r="3" ht="12" s="9"><c r="$D$3" s="4"/><c s="3"/><c r="ZZZZ1" s="3"/></row>"#,
            r#"<row r="x" ht="99" customHeight="1"/><row r="4" ht="9999" customHeight="1"/></sheetData>"#,
            r#"<mergeCells count="3"><mergeCell ref="A1:C1"/><mergeCell ref="E5:D4"/><mergeCell ref="bad"/></mergeCells>"#,
            "</worksheet>"
        ))
        .unwrap();
        assert_eq!(layout.default_row_height, Some(300));
        assert_eq!(
            layout
                .row_heights
                .iter()
                .map(|(row, height)| (*row, *height))
                .collect::<Vec<_>>(),
            vec![(0, 600), (1, 0)],
            "a row that is hidden is 0 tall, a height past Excel's is refused"
        );
        assert_eq!(layout.row_styles.get(&1), Some(&5));
        assert_eq!(
            layout.row_styles.get(&2),
            None,
            "a row style counts only with customFormat"
        );
        assert_eq!(layout.cell_styles.get(&(0, 0)), Some(&2));
        assert_eq!(layout.cell_styles.get(&(0, 1)), None);
        assert_eq!(
            layout.cell_styles.get(&(1, 0)),
            Some(&0),
            "a cell back to the default in a styled row"
        );
        assert_eq!(layout.cell_styles.get(&(2, 3)), Some(&4));
        assert_eq!(layout.cell_styles.len(), 3);
        assert_eq!(layout.column_styles, vec![(1, 2, 7)]);
        assert_eq!(
            layout.merges,
            vec![
                CellRange {
                    first_row: 0,
                    first_column: 0,
                    last_row: 0,
                    last_column: 2
                },
                CellRange {
                    first_row: 3,
                    first_column: 3,
                    last_row: 4,
                    last_column: 4
                }
            ]
        );
    }

    #[test]
    fn finds_the_style_and_the_merge_of_a_cell() {
        let mut layout = SheetLayout {
            styles: (0..10)
                .map(|index| CellStyle {
                    number_format: Some(index.to_string()),
                    ..CellStyle::default()
                })
                .collect(),
            ..SheetLayout::default()
        };
        layout.cell_styles.insert((0, 0), 2);
        layout.row_styles.insert(1, 5);
        layout.column_styles.push((1, 2, 7));
        layout.cell_styles.insert((5, 5), 99);
        let format = |row, column| {
            layout
                .style_of(row, column)
                .and_then(|style| style.number_format.clone())
        };
        assert_eq!(format(0, 0).as_deref(), Some("2"));
        assert_eq!(format(1, 4).as_deref(), Some("5"));
        assert_eq!(format(3, 2).as_deref(), Some("7"));
        assert_eq!(format(3, 3), None);
        assert_eq!(format(5, 5), None, "an index past the formats");

        layout.merges.push(CellRange::parse("B2:C3").unwrap());
        assert_eq!(
            layout.merge_at(1, 1),
            MergePlace::Start(CellRange {
                first_row: 1,
                first_column: 1,
                last_row: 2,
                last_column: 2
            })
        );
        assert_eq!(layout.merge_at(2, 2), MergePlace::Covered);
        assert_eq!(layout.merge_at(0, 0), MergePlace::None);
    }

    #[test]
    fn parses_cell_references() {
        assert_eq!(parse_cell("A1"), Some((0, 0)));
        assert_eq!(parse_cell("$AB$12"), Some((11, 27)));
        assert_eq!(parse_cell("XFD1048576"), Some((1_048_575, 16_383)));
        assert_eq!(parse_cell("XFE1"), None);
        assert_eq!(parse_cell("A0"), None);
        assert_eq!(parse_cell("1A"), None);
        assert_eq!(parse_cell("A"), None);
        assert_eq!(CellRange::parse("C3"), CellRange::parse("C3:C3"));
        assert_eq!(CellRange::parse("A1:"), None);
    }
}
