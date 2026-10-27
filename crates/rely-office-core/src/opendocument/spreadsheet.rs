// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The layout of each sheet of an ods, as an xlsx gives it: column widths and row heights, the
//! cell styles with their number formats, merged cells, the page each sheet prints on with its
//! header and footer, and the pictures and charts placed on the sheet.

mod number_styles;

use std::collections::HashMap;

use self::number_styles::number_format_codes;
use super::styles::{FO, STYLE, Scope, Styles, TEXT, border, colour, points, vertical_alignment};
use crate::archive::{ZipPackage, open_package, read_member_bytes, read_text_member};
use crate::chart::{odf_chart_picture, odf_object_part};
use crate::document::{Border, sniff_image_type};
use crate::sheet_layout::{CellRange, SheetLayout};
use crate::sheet_styles::{CellBorders, CellFont, CellStyle, HorizontalAlignment};
use crate::xml_tree::{Element, Namespace, Node, parse_tree};
use crate::{Limits, OfficeError, SheetDrawing};

const OFFICE: Namespace = Namespace::OdfOffice;
const TABLE: Namespace = Namespace::OdfTable;
const DRAW: Namespace = Namespace::OdfDraw;
const SVG: Namespace = Namespace::OdfSvg;
const XLINK: Namespace = Namespace::XLink;
/// The grid a spreadsheet has; a repeat past it is cut there.
const MAX_ROWS: u32 = 1_048_576;
const MAX_COLUMNS: u32 = 16_384;
/// No sheet keeps more styled cells than this, whatever the repeats claim.
const MAX_STYLED_CELLS: usize = 2_000_000;
/// No sheet places more drawings than this.
const MAX_DRAWINGS: usize = 256;
/// Excel's widths: the pixels of its default font's digit, and its padding in 256ths.
const DIGIT_PIXELS: f64 = 7.0;
const WIDTH_PADDING: f64 = 18.0;
const POINTS_PER_PIXEL: f64 = 0.75;
const EMU_PER_POINT: f64 = 12_700.0;
/// The largest offset or size a drawing keeps, a few pages.
const MAX_EMU_LENGTH: f64 = 51_206_400.0 * 4.0;

/// The layout of each sheet of an ods by its name; a package it cannot read gives none, as the
/// cells were read by then and print with the defaults.
#[must_use]
pub(crate) fn read_ods_layouts(bytes: &[u8], limits: &Limits) -> HashMap<String, SheetLayout> {
    read_layouts(bytes, limits).unwrap_or_default()
}

fn read_layouts(
    bytes: &[u8],
    limits: &Limits,
) -> Result<HashMap<String, SheetLayout>, OfficeError> {
    let mut package = open_package(bytes)?;
    let Some(content) = read_text_member(&mut package, "content.xml", limits)? else {
        return Ok(HashMap::new());
    };
    let content = parse_tree(&content, keeps_whitespace)?;
    let styles = read_text_member(&mut package, "styles.xml", limits)?
        .map(|xml| parse_tree(&xml, keeps_whitespace))
        .transpose()?;
    let style_sheet = Styles::read(Some(&content), styles.as_ref());
    let formats = number_format_codes([Some(&content), styles.as_ref()].into_iter().flatten());
    let Some(spreadsheet) = content
        .child(OFFICE, "body")
        .and_then(|body| body.child(OFFICE, "spreadsheet"))
    else {
        return Ok(HashMap::new());
    };
    let mut reader = LayoutReader {
        styles: &style_sheet,
        formats: &formats,
        package: &mut package,
        limits,
        images: 0,
        image_bytes: 0,
    };
    let mut layouts = HashMap::new();
    for table in spreadsheet
        .elements()
        .filter(|table| table.is(TABLE, "table"))
    {
        if let Some(name) = table.attribute(TABLE, "name") {
            let layout = reader.table(table);
            layouts.insert(name.to_owned(), layout);
        }
    }
    Ok(layouts)
}

struct LayoutReader<'a, 'p, 'b> {
    styles: &'a Styles,
    /// The format code of each data style, by its name.
    formats: &'a HashMap<String, String>,
    package: &'p mut ZipPackage<'b>,
    limits: &'a Limits,
    images: usize,
    image_bytes: u64,
}

/// A sheet while its rows are walked.
struct SheetBuild {
    layout: SheetLayout,
    /// The index in `layout.styles` of each cell style already met.
    style_indexes: HashMap<String, Option<u32>>,
    /// The row the walk is at, and the anchor rows of the drawings met.
    row: u32,
}

impl LayoutReader<'_, '_, '_> {
    fn table(&mut self, table: &Element) -> SheetLayout {
        let mut build = SheetBuild {
            layout: SheetLayout::default(),
            style_indexes: HashMap::new(),
            row: 0,
        };
        self.page(table, &mut build.layout);
        let mut column = 0;
        self.columns(table, &mut build, &mut column);
        self.rows(table, &mut build);
        if let Some(shapes) = table.child(TABLE, "shapes") {
            for frame in shapes.elements() {
                self.drawing(frame, (0, 0), &mut build.layout, true);
            }
        }
        build.layout
    }

    /// The page a sheet prints on: the master page its table style names, else the default.
    fn page(&self, table: &Element, layout: &mut SheetLayout) {
        if let Some(properties) = self
            .styles
            .properties(
                Scope::Body,
                "table-cell",
                Some("Default"),
                "text-properties",
            )
            .first()
        {
            let (font, _) = self.styles.text_properties(properties);
            layout.font_family = font.family;
            layout.font_size_tenths = font.size_half_points.map(|half| half * 5);
        }
        let master_name = self
            .styles
            .chain(Scope::Body, "table", table.attribute(TABLE, "style-name"))
            .into_iter()
            .find_map(|style| style.attribute(STYLE, "master-page-name"))
            .unwrap_or("Default");
        let master = self
            .styles
            .master_pages
            .iter()
            .find(|page| page.attribute(STYLE, "name") == Some(master_name))
            .or_else(|| self.styles.master_pages.first());
        let Some(master) = master else {
            return;
        };
        layout.header = master.child(STYLE, "header").and_then(header_codes);
        layout.footer = master.child(STYLE, "footer").and_then(header_codes);
        let Some(properties) = master
            .attribute(STYLE, "page-layout-name")
            .and_then(|name| self.styles.page_layouts.get(name))
            .and_then(|page| page.child(STYLE, "page-layout-properties"))
        else {
            return;
        };
        let length = |name: &str| properties.attribute(FO, name).and_then(points);
        layout.landscape = properties.attribute(STYLE, "print-orientation") == Some("landscape")
            || matches!(
                (length("page-width"), length("page-height")),
                (Some(width), Some(height)) if width > height
            );
        let mils = |points: Option<f64>, default: f64| {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a margin is clamped to a page"
            )]
            let mils = (points.unwrap_or(default) / 72.0 * 1_000.0)
                .round()
                .clamp(0.0, 20_000.0) as u32;
            mils
        };
        let [top, bottom, left, right] =
            ["margin-top", "margin-bottom", "margin-left", "margin-right"]
                .map(|name| mils(length(name), 56.7));
        layout.margins_mils = Some([top, right, bottom, left, 300, 300]);
        layout.grid_lines = properties
            .attribute(STYLE, "print")
            .is_some_and(|print| print.split_whitespace().any(|item| item == "grid"));
    }

    /// The columns of a table and of the groups inside it, each repeat a column of its own.
    fn columns(&self, parent: &Element, build: &mut SheetBuild, column: &mut u32) {
        for element in parent.elements() {
            match (element.namespace, element.name.as_str()) {
                (TABLE, "table-column-group" | "table-columns" | "table-header-columns") => {
                    self.columns(element, build, column);
                }
                (TABLE, "table-column") => {
                    let repeated = repeat(element, "number-columns-repeated")
                        .min(MAX_COLUMNS.saturating_sub(*column));
                    if repeated == 0 {
                        return;
                    }
                    let hidden = matches!(
                        element.attribute(TABLE, "visibility"),
                        Some("collapse" | "filter")
                    );
                    let width = if hidden {
                        Some(0)
                    } else {
                        self.styles
                            .properties(
                                Scope::Body,
                                "table-column",
                                element.attribute(TABLE, "style-name"),
                                "table-column-properties",
                            )
                            .into_iter()
                            .find_map(|properties| properties.attribute(STYLE, "column-width"))
                            .and_then(points)
                            .map(width_of_points)
                    };
                    let shown_limit = u32::try_from(self.limits.max_columns).unwrap_or(u32::MAX);
                    let end = (*column + repeated).min(shown_limit.max(*column));
                    for index in *column..end {
                        let slot = usize::try_from(index).unwrap_or(usize::MAX);
                        if build.layout.column_widths.len() <= slot {
                            build.layout.column_widths.resize(slot + 1, None);
                        }
                        build.layout.column_widths[slot] = width;
                    }
                    if let Some(style) = element.attribute(TABLE, "default-cell-style-name")
                        && let Some(index) = self.cell_style(build, style)
                    {
                        build
                            .layout
                            .column_styles
                            .push((*column, *column + repeated - 1, index));
                    }
                    *column += repeated;
                }
                _ => {}
            }
        }
    }

    /// The rows of a table and of the groups inside it: their heights, their cells' styles and
    /// merges, and the drawings anchored in their cells.
    fn rows(&mut self, parent: &Element, build: &mut SheetBuild) {
        for element in parent.elements() {
            if build.row >= MAX_ROWS {
                return;
            }
            match (element.namespace, element.name.as_str()) {
                (TABLE, "table-row-group" | "table-rows" | "table-header-rows") => {
                    self.rows(element, build);
                }
                (TABLE, "table-row") => self.row(element, build),
                _ => {}
            }
        }
    }

    fn row(&mut self, row: &Element, build: &mut SheetBuild) {
        let repeated = repeat(row, "number-rows-repeated").min(MAX_ROWS - build.row);
        let hidden = matches!(
            row.attribute(TABLE, "visibility"),
            Some("collapse" | "filter")
        );
        let height = if hidden {
            Some(0)
        } else {
            self.styles
                .properties(
                    Scope::Body,
                    "table-row",
                    row.attribute(TABLE, "style-name"),
                    "table-row-properties",
                )
                .into_iter()
                .find_map(|properties| properties.attribute(STYLE, "row-height"))
                .and_then(points)
                .map(|height| {
                    #[expect(
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss,
                        reason = "a row height is clamped to the largest a sheet allows"
                    )]
                    let twips = (height * 20.0).round().clamp(0.0, 8_190.0) as u32;
                    twips
                })
        };
        let row_style = row
            .attribute(TABLE, "default-cell-style-name")
            .and_then(|style| self.cell_style(build, style));
        let shown_rows = u32::try_from(self.limits.max_rows).unwrap_or(u32::MAX);
        let start = build.row;
        // Every repeat of a row is the same row: its cells are walked once, for the rows shown.
        for index in start..(start + repeated).min(shown_rows.max(start)) {
            if let Some(height) = height {
                build.layout.row_heights.insert(index, height);
            }
            if let Some(style) = row_style {
                build.layout.row_styles.insert(index, style);
            }
            self.cells(row, index, build);
        }
        build.row = start + repeated;
    }

    fn cells(&mut self, row: &Element, index: u32, build: &mut SheetBuild) {
        let shown_columns = u32::try_from(self.limits.max_columns).unwrap_or(u32::MAX);
        let mut column = 0_u32;
        for cell in row.elements() {
            if column >= MAX_COLUMNS {
                break;
            }
            let covered = cell.is(TABLE, "covered-table-cell");
            if !covered && !cell.is(TABLE, "table-cell") {
                continue;
            }
            let repeated = repeat(cell, "number-columns-repeated").min(MAX_COLUMNS - column);
            if let Some(style) = cell
                .attribute(TABLE, "style-name")
                .and_then(|style| self.cell_style(build, style))
            {
                for offset in 0..repeated.min(shown_columns.saturating_sub(column)) {
                    if build.layout.cell_styles.len() >= MAX_STYLED_CELLS {
                        break;
                    }
                    build
                        .layout
                        .cell_styles
                        .insert((index, column + offset), style);
                }
            }
            let span = |name: &str| repeat(cell, name);
            let (columns_spanned, rows_spanned) =
                (span("number-columns-spanned"), span("number-rows-spanned"));
            if !covered && (columns_spanned > 1 || rows_spanned > 1) {
                build.layout.merges.push(CellRange {
                    first_row: index,
                    first_column: column,
                    last_row: (index + rows_spanned - 1).min(MAX_ROWS - 1),
                    last_column: (column + columns_spanned - 1).min(MAX_COLUMNS - 1),
                });
            }
            for frame in cell.elements() {
                self.drawing(frame, (index, column), &mut build.layout, false);
            }
            column += repeated;
        }
    }

    /// The index of a cell style in the sheet's list, read the first time it is met.
    fn cell_style(&self, build: &mut SheetBuild, name: &str) -> Option<u32> {
        if let Some(known) = build.style_indexes.get(name) {
            return *known;
        }
        let style = self.read_cell_style(name);
        let index = style.map(|style| {
            build.layout.styles.push(style);
            u32::try_from(build.layout.styles.len() - 1).unwrap_or(u32::MAX)
        });
        build.style_indexes.insert(name.to_owned(), index);
        index
    }

    /// A `table-cell` style and its parents as a cell format; `None` for one that sets nothing.
    fn read_cell_style(&self, name: &str) -> Option<CellStyle> {
        let properties = |kind: &str| {
            self.styles
                .properties(Scope::Body, "table-cell", Some(name), kind)
        };
        let cell = properties("table-cell-properties");
        let cell_attribute = |namespace: Namespace, key: &str| {
            cell.iter()
                .find_map(|properties| properties.attribute(namespace, key))
        };
        let side = |key: &str| {
            cell_attribute(FO, key)
                .or_else(|| cell_attribute(FO, "border"))
                .map(border)
                .filter(Border::is_visible)
        };
        let (font, flags) = self.styles.text_look(Scope::Body, "table-cell", Some(name));
        let horizontal = properties("paragraph-properties")
            .into_iter()
            .find_map(|properties| properties.attribute(FO, "text-align"))
            .and_then(|align| match align {
                "start" | "left" => Some(HorizontalAlignment::Left),
                "center" => Some(HorizontalAlignment::Center),
                "end" | "right" => Some(HorizontalAlignment::Right),
                "justify" => Some(HorizontalAlignment::Justify),
                _ => None,
            });
        let number_format = self
            .styles
            .chain(Scope::Body, "table-cell", Some(name))
            .into_iter()
            .find_map(|style| style.attribute(STYLE, "data-style-name"))
            .and_then(|data| self.formats.get(data))
            .cloned();
        let style = CellStyle {
            font: CellFont {
                family: font.family,
                size_tenths: font.size_half_points.map(|half| half * 5),
                colour: font.colour,
                bold: font.bold.unwrap_or(false),
                italic: font.italic.unwrap_or(false),
                underline: flags.underline.unwrap_or(false),
                strike: flags.strike.unwrap_or(false),
            },
            fill: cell_attribute(FO, "background-color").and_then(colour),
            borders: CellBorders {
                top: side("border-top"),
                right: side("border-right"),
                bottom: side("border-bottom"),
                left: side("border-left"),
            },
            horizontal,
            vertical: cell_attribute(STYLE, "vertical-align").and_then(vertical_alignment),
            wrap: cell_attribute(FO, "wrap-option") == Some("wrap"),
            number_format,
        };
        (style != CellStyle::default()).then_some(style)
    }

    /// A `draw:frame` with a picture or a chart, placed at its anchor cell, or on the sheet when
    /// it stands in `table:shapes`.
    fn drawing(
        &mut self,
        frame: &Element,
        (row, column): (u32, u32),
        layout: &mut SheetLayout,
        on_sheet: bool,
    ) {
        if !frame.is(DRAW, "frame") || layout.drawings.len() >= MAX_DRAWINGS {
            return;
        }
        let length = |name: &str| {
            frame
                .attribute(SVG, name)
                .and_then(points)
                .map_or(0, |points| {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "the length is clamped to a few pages"
                    )]
                    let emu = (points * EMU_PER_POINT)
                        .round()
                        .clamp(-MAX_EMU_LENGTH, MAX_EMU_LENGTH)
                        as i64;
                    emu
                })
        };
        let (width_emu, height_emu) = (length("width"), length("height"));
        if width_emu <= 0 || height_emu <= 0 {
            return;
        }
        // A frame in a cell keeps its offset from the sheet's corner; only one on the sheet
        // places itself by it.
        let offset_emu = if on_sheet {
            (length("x").max(0), length("y").max(0))
        } else {
            (0, 0)
        };
        let drawn = if let Some(href) = frame
            .child(DRAW, "object")
            .and_then(|object| object.attribute(XLINK, "href"))
        {
            odf_object_part(href)
                .and_then(|part| {
                    read_text_member(self.package, &part, self.limits)
                        .ok()
                        .flatten()
                })
                .and_then(|xml| odf_chart_picture(&xml, width_emu, height_emu))
                .map(|(bytes, title)| ("image/svg+xml", bytes, title))
        } else {
            frame
                .child(DRAW, "image")
                .and_then(|image| image.attribute(XLINK, "href"))
                .and_then(|href| self.image(href))
                .map(|bytes| (sniff_image_type(&bytes), bytes, None))
        };
        let Some((content_type, bytes, title)) = drawn else {
            return;
        };
        let description = title.or_else(|| {
            frame
                .child(SVG, "title")
                .or_else(|| frame.child(SVG, "desc"))
                .map(Element::text)
                .filter(|text| !text.trim().is_empty())
        });
        layout.drawings.push(SheetDrawing {
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

    /// The bytes of a picture of the package, within the image limits.
    fn image(&mut self, href: &str) -> Option<Vec<u8>> {
        let name = href.trim_start_matches("./");
        if name.contains("://") || self.images >= self.limits.max_images {
            return None;
        }
        let cap = self
            .limits
            .max_image_bytes
            .min(self.limits.max_member_bytes);
        let bytes = read_member_bytes(self.package, name, cap).ok()??;
        let size = bytes.len() as u64;
        if self.image_bytes.saturating_add(size) > self.limits.max_total_image_bytes {
            return None;
        }
        self.images += 1;
        self.image_bytes += size;
        Some(bytes)
    }
}

/// The literal text of a number format and of a header keeps its spaces.
fn keeps_whitespace(namespace: Namespace, name: &str) -> bool {
    matches!(
        (namespace, name),
        (Namespace::OdfNumber, "text") | (Namespace::OdfText, "p" | "span")
    )
}

/// How many times an element repeats, or how many cells it spans; at least once.
fn repeat(element: &Element, name: &str) -> u32 {
    element
        .attribute(TABLE, name)
        .and_then(|count| count.parse::<u32>().ok())
        .unwrap_or(1)
        .max(1)
}

/// A column width in points as Excel's 256ths of a character, the inverse of how it prints one.
fn width_of_points(width: f64) -> u32 {
    if width <= 0.0 {
        return 0;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the width is clamped to Excel's widest column"
    )]
    let width = (width / POINTS_PER_PIXEL / DIGIT_PIXELS * 256.0 - WIDTH_PADDING)
        .round()
        .clamp(1.0, 65_280.0) as u32;
    width
}

/// A page's header or footer as Excel codes it: the left, centre and right regions, or the
/// whole as the centre; the sheet name, the page number and the page count as fields.
fn header_codes(part: &Element) -> Option<String> {
    if part.attribute(STYLE, "display") == Some("false") {
        return None;
    }
    let regions = [
        ("&L", part.child(STYLE, "region-left")),
        ("&C", part.child(STYLE, "region-center")),
        ("&R", part.child(STYLE, "region-right")),
    ];
    let mut codes = String::new();
    if regions.iter().any(|(_, region)| region.is_some()) {
        for (code, region) in regions {
            if let Some(region) = region {
                let text = region_codes(region);
                if !text.is_empty() {
                    codes.push_str(code);
                    codes.push_str(&text);
                }
            }
        }
    } else {
        let text = region_codes(part);
        if !text.is_empty() {
            codes.push_str("&C");
            codes.push_str(&text);
        }
    }
    (!codes.is_empty()).then_some(codes)
}

/// The text of a region in Excel's header codes, paragraphs joined by spaces.
fn region_codes(region: &Element) -> String {
    let mut text = String::new();
    for paragraph in region
        .elements()
        .filter(|paragraph| paragraph.is(TEXT, "p"))
    {
        if !text.is_empty() {
            text.push(' ');
        }
        append_codes(paragraph, &mut text);
    }
    text
}

fn append_codes(element: &Element, text: &mut String) {
    for node in &element.children {
        match node {
            Node::Text(run) => text.push_str(&run.replace('&', "&&")),
            Node::Element(child) => match (child.namespace, child.name.as_str()) {
                (TEXT, "sheet-name") => text.push_str("&A"),
                (TEXT, "page-number") => text.push_str("&P"),
                (TEXT, "page-count") => text.push_str("&N"),
                (TEXT, "s" | "tab") => text.push(' '),
                (TEXT, "title" | "date" | "time" | "file-name") => {}
                _ => append_codes(child, text),
            },
        }
    }
}

#[cfg(test)]
mod tests;
