// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! A workbook read from a file and written to xlsx. Reading keeps what an editor needs: every
//! cell's formula or value, dates as dates, column widths, row heights, cell styles, merged
//! cells and sheet names. Writing stores each formula with its computed value, so a viewer that
//! does not recalculate still shows the results, and asks Excel to recalculate on opening.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::io::{Cursor, Write};

use calamine::{Data, Reader, open_workbook_auto_from_rs};
use rely_office_core::{
    Border, BorderStyle, CellBorders, CellFont, CellStyle, DocumentFormat, HorizontalAlignment,
    Limits, OfficeError, SheetLayout, VerticalAlignment, detect_format, read_workbook,
    read_xlsx_layouts,
};
use zip::write::SimpleFileOptions;

use crate::address::{CellRef, MAX_COLUMNS, MAX_ROWS, column_name};
use crate::formula::{Expr, number_text, parse_formula, print_formula};
use crate::model::{Cell, MAX_CELLS, MAX_SHEETS, NumberFormat, Sheet, Workbook};
use crate::value::{ErrorKind, Input, Locale, Value};

/// Read a spreadsheet (xlsx, xls, ods, csv, tsv) as an editable workbook. The text of a CSV is
/// read in `locale`, so `1.234,5` is a number in an Italian one.
///
/// # Errors
///
/// [`OfficeError::UnsupportedFormat`] for anything but a spreadsheet, [`OfficeError::TooLarge`]
/// past [`Limits::max_input_bytes`], [`MAX_SHEETS`] or [`MAX_CELLS`], and
/// [`OfficeError::UnreadableWorkbook`] when the reader refuses the file.
pub fn read_spreadsheet(
    bytes: &[u8],
    file_name: Option<&str>,
    limits: &Limits,
    locale: Locale,
) -> Result<Workbook, OfficeError> {
    if bytes.len() as u64 > limits.max_input_bytes {
        return Err(OfficeError::TooLarge {
            what: "input".to_owned(),
            limit: limits.max_input_bytes,
        });
    }
    let format = detect_format(bytes, file_name);
    let mut styles = StyleTable::default();
    let sheets = match format {
        DocumentFormat::Xlsx | DocumentFormat::Xls | DocumentFormat::Ods => {
            let mut sheets = read_binary(bytes)?;
            if format == DocumentFormat::Xlsx {
                let mut layouts = read_xlsx_layouts(bytes, limits);
                for sheet in &mut sheets {
                    if let Some(layout) = layouts.remove(&sheet.name) {
                        apply_layout(sheet, &layout, &mut styles);
                    }
                }
            }
            sheets
        }
        DocumentFormat::Csv | DocumentFormat::Tsv => {
            let limits = Limits {
                max_rows: usize::try_from(MAX_ROWS).unwrap_or(usize::MAX),
                max_columns: usize::try_from(MAX_COLUMNS).unwrap_or(usize::MAX),
                ..*limits
            };
            read_workbook(bytes, file_name, &limits)?
                .sheets
                .into_iter()
                .map(|table| delimited_sheet(&table.rows, locale))
                .collect::<Result<Vec<_>, _>>()?
        }
        other => return Err(OfficeError::UnsupportedFormat(other)),
    };
    let cells: usize = sheets.iter().map(|sheet| sheet.cells.len()).sum();
    if cells > MAX_CELLS {
        return Err(too_large("cells", MAX_CELLS));
    }
    Ok(Workbook::from_parts(sheets, styles.styles))
}

/// The styles of the cells of a file, each kept once.
#[derive(Debug)]
struct StyleTable {
    styles: Vec<CellStyle>,
    index: HashMap<CellStyle, u32>,
}

impl Default for StyleTable {
    fn default() -> Self {
        Self {
            styles: vec![CellStyle::default()],
            index: HashMap::from([(CellStyle::default(), 0)]),
        }
    }
}

impl StyleTable {
    fn intern(&mut self, style: CellStyle) -> u32 {
        if let Some(id) = self.index.get(&style) {
            return *id;
        }
        let id = u32::try_from(self.styles.len()).unwrap_or(u32::MAX);
        self.index.insert(style.clone(), id);
        self.styles.push(style);
        id
    }
}

/// The column widths, row heights, merges and cell styles an xlsx gives a sheet. A number
/// format the model has a format of its own for becomes the cell's; every other code stays in
/// the cell's style.
fn apply_layout(sheet: &mut Sheet, layout: &SheetLayout, styles: &mut StyleTable) {
    for (column, width) in layout.column_widths.iter().enumerate() {
        if let (Some(width), Ok(column)) = (width, u32::try_from(column)) {
            sheet
                .column_widths
                .insert(column, f64::from(*width) / 256.0);
        }
    }
    for (row, height) in &layout.row_heights {
        sheet.row_heights.insert(*row, f64::from(*height) / 20.0);
    }
    sheet.merges.clone_from(&layout.merges);
    let default_font = layout
        .styles
        .first()
        .map(|style| style.font.clone())
        .unwrap_or_default();
    let mut positions: BTreeSet<CellRef> = layout
        .cell_styles
        .keys()
        .map(|(row, column)| CellRef::new(*row, *column))
        .filter(|cell| cell.row < MAX_ROWS && cell.column < MAX_COLUMNS)
        .collect();
    positions.extend(sheet.cells.keys().copied());
    for cell in positions {
        let Some(file_style) = layout.style_of(cell.row, cell.column) else {
            continue;
        };
        let mut style = file_style.clone();
        if style.font == default_font {
            style.font = CellFont::default();
        }
        let simple = style.number_format.as_deref().and_then(simple_format);
        if let Some(content) = sheet.cells.get_mut(&cell) {
            match simple {
                Some(format) => {
                    content.format = format;
                    style.number_format = None;
                }
                None if style.number_format.is_some() => content.format = NumberFormat::General,
                None => {}
            }
        }
        let id = styles.intern(style);
        if id != 0 {
            sheet.styles.insert(cell, id);
        }
    }
}

/// The model's own format for an Excel code that means the same.
fn simple_format(code: &str) -> Option<NumberFormat> {
    match code {
        "m/d/yyyy" => Some(NumberFormat::Date),
        "m/d/yyyy h:mm" => Some(NumberFormat::DateTime),
        "0" => Some(NumberFormat::Decimals(0)),
        _ => {
            let decimals = code.strip_prefix("0.")?;
            (!decimals.is_empty() && decimals.len() <= 10 && decimals.chars().all(|c| c == '0'))
                .then(|| NumberFormat::Decimals(u8::try_from(decimals.len()).unwrap_or(10)))
        }
    }
}

fn too_large(what: &str, limit: usize) -> OfficeError {
    OfficeError::TooLarge {
        what: what.to_owned(),
        limit: limit as u64,
    }
}

fn read_binary(bytes: &[u8]) -> Result<Vec<Sheet>, OfficeError> {
    let unreadable = |error: calamine::Error| OfficeError::UnreadableWorkbook(error.to_string());
    let mut workbook = open_workbook_auto_from_rs(Cursor::new(bytes)).map_err(unreadable)?;
    let names = workbook.sheet_names();
    if names.len() > MAX_SHEETS {
        return Err(too_large("sheets", MAX_SHEETS));
    }
    let mut sheets = Vec::with_capacity(names.len());
    let mut total = 0;
    for name in names {
        let values = workbook.worksheet_range(&name).map_err(unreadable)?;
        let formulas = workbook.worksheet_formula(&name).ok();
        let mut cells: BTreeMap<CellRef, Cell> = BTreeMap::new();
        let (row_offset, column_offset) = values.start().unwrap_or((0, 0));
        for (row, column, data) in values.used_cells() {
            let position = position(row_offset, column_offset, row, column)?;
            if let Some(cell) = cell_from_data(data) {
                cells.insert(position, cell);
            }
        }
        if let Some(formulas) = formulas {
            let (row_offset, column_offset) = formulas.start().unwrap_or((0, 0));
            for (row, column, formula) in formulas.used_cells() {
                if formula.is_empty() {
                    continue;
                }
                let position = position(row_offset, column_offset, row, column)?;
                let format = cells
                    .get(&position)
                    .map_or(NumberFormat::General, |cell| cell.format);
                let input = parse_formula(formula).map_or_else(
                    |_| Input::Text(format!("={formula}")),
                    |expression| Input::Formula(Box::new(expression)),
                );
                cells.insert(position, Cell { input, format });
            }
        }
        total += cells.len();
        if total > MAX_CELLS {
            return Err(too_large("cells", MAX_CELLS));
        }
        sheets.push(Sheet {
            name,
            cells,
            ..Sheet::default()
        });
    }
    Ok(sheets)
}

fn position(
    row_offset: u32,
    column_offset: u32,
    row: usize,
    column: usize,
) -> Result<CellRef, OfficeError> {
    let row = u32::try_from(row)
        .ok()
        .and_then(|row| row.checked_add(row_offset));
    let column = u32::try_from(column)
        .ok()
        .and_then(|column| column.checked_add(column_offset));
    match (row, column) {
        (Some(row), Some(column)) if row < MAX_ROWS && column < MAX_COLUMNS => {
            Ok(CellRef::new(row, column))
        }
        _ => Err(OfficeError::UnreadableWorkbook(
            "a cell lies outside the grid".to_owned(),
        )),
    }
}

fn cell_from_data(data: &Data) -> Option<Cell> {
    let general = |input| {
        Some(Cell {
            input,
            format: NumberFormat::General,
        })
    };
    match data {
        Data::Empty => None,
        #[allow(clippy::cast_precision_loss)]
        Data::Int(number) => general(Input::Number(*number as f64)),
        Data::Float(number) => general(Input::Number(*number)),
        Data::String(text) if text.is_empty() => None,
        Data::String(text) | Data::DateTimeIso(text) | Data::DurationIso(text) => {
            general(Input::Text(text.clone()))
        }
        Data::Bool(value) => general(Input::Bool(*value)),
        Data::DateTime(moment) => Some(Cell {
            input: Input::Number(moment.as_f64()),
            format: if moment.as_f64().fract() == 0.0 {
                NumberFormat::Date
            } else {
                NumberFormat::DateTime
            },
        }),
        Data::Error(error) => {
            let code = error.to_string();
            let kind = ErrorKind::ALL
                .into_iter()
                .find(|kind| kind.code() == code)
                .unwrap_or(ErrorKind::Value);
            general(Input::Formula(Box::new(Expr::Error(kind))))
        }
    }
}

fn delimited_sheet(rows: &[Vec<String>], locale: Locale) -> Result<Sheet, OfficeError> {
    let mut cells = BTreeMap::new();
    for (row, values) in rows.iter().enumerate() {
        for (column, text) in values.iter().enumerate() {
            // A CSV holds values, not formulas: a leading = stays text.
            let input = if text.starts_with('=') {
                Some(Input::Text(text.clone()))
            } else {
                Input::parse(text, locale).unwrap_or_else(|_| Some(Input::Text(text.clone())))
            };
            if let Some(input) = input {
                cells.insert(
                    position(0, 0, row, column)?,
                    Cell {
                        input,
                        format: NumberFormat::General,
                    },
                );
            }
        }
    }
    Ok(Sheet {
        name: "Foglio1".to_owned(),
        cells,
        ..Sheet::default()
    })
}

/// Write a workbook as xlsx, each formula with its computed value, the cells with their
/// styles, the merged cells and the row heights.
///
/// # Errors
///
/// [`OfficeError::DamagedContainer`] when the zip cannot be written, which only a failing
/// allocator causes.
pub fn write_xlsx(workbook: &Workbook) -> Result<Vec<u8>, OfficeError> {
    let styles = Styles::new(workbook);

    let mut package = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default();
    let mut add = |name: &str, content: &[u8]| -> Result<(), OfficeError> {
        package
            .start_file(name, options)
            .map_err(|error| OfficeError::DamagedContainer(error.to_string()))?;
        package
            .write_all(content)
            .map_err(|error| OfficeError::DamagedContainer(error.to_string()))
    };
    let count = workbook.sheets.len();
    add("[Content_Types].xml", content_types(count).as_bytes())?;
    add("_rels/.rels", PACKAGE_RELATIONSHIPS.as_bytes())?;
    add("xl/workbook.xml", workbook_part(workbook).as_bytes())?;
    add(
        "xl/_rels/workbook.xml.rels",
        workbook_relationships(count).as_bytes(),
    )?;
    add("xl/styles.xml", styles.xml.as_bytes())?;
    for (index, sheet) in workbook.sheets.iter().enumerate() {
        let xml = sheet_part(sheet, &workbook.values[index], &styles);
        add(
            &format!("xl/worksheets/sheet{}.xml", index + 1),
            xml.as_bytes(),
        )?;
    }
    package
        .finish()
        .map(Cursor::into_inner)
        .map_err(|error| OfficeError::DamagedContainer(error.to_string()))
}

/// The cell format index of each style and number format a cell uses, and the styles part
/// declaring them.
struct Styles {
    formats: HashMap<(u32, NumberFormat), usize>,
    xml: String,
}

/// The lists of a styles part, each entry once, in the order cell formats refer to them.
#[derive(Default)]
struct StyleLists {
    fonts: Vec<CellFont>,
    fills: Vec<String>,
    borders: Vec<CellBorders>,
    codes: Vec<String>,
}

impl StyleLists {
    fn index<T: PartialEq + Clone>(list: &mut Vec<T>, item: &T) -> usize {
        list.iter()
            .position(|known| known == item)
            .unwrap_or_else(|| {
                list.push(item.clone());
                list.len() - 1
            })
    }

    /// The `xf` of a style shown with a number format, adding its font, fill, border and code
    /// to the lists.
    fn cell_format(&mut self, style: &CellStyle, format: NumberFormat) -> String {
        let font = if style.font == CellFont::default() {
            0
        } else {
            Self::index(&mut self.fonts, &style.font) + 1
        };
        let fill = style
            .fill
            .as_ref()
            .map_or(0, |fill| Self::index(&mut self.fills, fill) + 2);
        let border = if style.borders == CellBorders::default() {
            0
        } else {
            Self::index(&mut self.borders, &style.borders) + 1
        };
        let number_format = match format {
            NumberFormat::General => style
                .number_format
                .as_ref()
                .map_or(0, |code| 164 + Self::index(&mut self.codes, code)),
            NumberFormat::Date => 14,
            NumberFormat::DateTime => 22,
            NumberFormat::Percent => 10,
            NumberFormat::Decimals(0) => 1,
            NumberFormat::Decimals(count) => {
                let code = format!("0.{}", "0".repeat(usize::from(count.min(10))));
                164 + Self::index(&mut self.codes, &code)
            }
        };
        let mut xml = format!(
            r#"<xf numFmtId="{number_format}" fontId="{font}" fillId="{fill}" borderId="{border}" xfId="0""#
        );
        for (applies, name) in [
            (number_format != 0, "applyNumberFormat"),
            (font != 0, "applyFont"),
            (fill != 0, "applyFill"),
            (border != 0, "applyBorder"),
        ] {
            if applies {
                let _ = write!(xml, r#" {name}="1""#);
            }
        }
        let alignment = alignment_xml(style);
        if alignment.is_empty() {
            xml.push_str("/>");
        } else {
            let _ = write!(xml, r#" applyAlignment="1"><alignment{alignment}/></xf>"#);
        }
        xml
    }
}

/// The styles part: number formats, fonts, fills, borders and the cell formats using them.
fn styles_part(lists: &StyleLists, cell_formats: &str, count: usize) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#,
    );
    if !lists.codes.is_empty() {
        let _ = write!(xml, r#"<numFmts count="{}">"#, lists.codes.len());
        for (index, code) in lists.codes.iter().enumerate() {
            let _ = write!(
                xml,
                r#"<numFmt numFmtId="{}" formatCode="{}"/>"#,
                164 + index,
                escape(code)
            );
        }
        xml.push_str("</numFmts>");
    }
    let _ = write!(
        xml,
        r#"<fonts count="{}"><font><sz val="11"/><name val="Calibri"/><family val="2"/></font>"#,
        lists.fonts.len() + 1
    );
    for font in &lists.fonts {
        xml.push_str(&font_xml(font));
    }
    let _ = write!(
        xml,
        r#"</fonts><fills count="{}"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill>"#,
        lists.fills.len() + 2
    );
    for fill in &lists.fills {
        let _ = write!(
            xml,
            r#"<fill><patternFill patternType="solid"><fgColor rgb="FF{fill}"/><bgColor indexed="64"/></patternFill></fill>"#
        );
    }
    let _ = write!(
        xml,
        r#"</fills><borders count="{}"><border><left/><right/><top/><bottom/><diagonal/></border>"#,
        lists.borders.len() + 1
    );
    for borders in &lists.borders {
        xml.push_str(&borders_xml(borders));
    }
    xml.push_str(concat!(
        "</borders>",
        r#"<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>"#,
    ));
    let _ = write!(xml, r#"<cellXfs count="{count}">{cell_formats}</cellXfs>"#);
    xml.push_str(r#"<cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"#);
    xml
}

impl Styles {
    fn new(workbook: &Workbook) -> Self {
        let mut used: Vec<(u32, NumberFormat)> = vec![(0, NumberFormat::General)];
        for sheet in &workbook.sheets {
            for (cell, content) in &sheet.cells {
                used.push((sheet.styles.get(cell).copied().unwrap_or(0), content.format));
            }
            for (cell, style) in &sheet.styles {
                if !sheet.cells.contains_key(cell) {
                    used.push((*style, NumberFormat::General));
                }
            }
        }
        let mut seen = std::collections::HashSet::new();
        used.retain(|pair| seen.insert(*pair));

        let mut lists = StyleLists::default();
        let mut formats = HashMap::new();
        let mut cell_formats = String::new();
        for (index, (style_id, format)) in used.iter().enumerate() {
            let style = workbook
                .styles
                .get(usize::try_from(*style_id).unwrap_or(0))
                .cloned()
                .unwrap_or_default();
            cell_formats.push_str(&lists.cell_format(&style, *format));
            formats.insert((*style_id, *format), index);
        }
        Self {
            formats,
            xml: styles_part(&lists, &cell_formats, used.len()),
        }
    }

    fn index(&self, style: u32, format: NumberFormat) -> usize {
        self.formats.get(&(style, format)).copied().unwrap_or(0)
    }
}

fn font_xml(font: &CellFont) -> String {
    let mut xml = String::from("<font>");
    for (on, element) in [
        (font.bold, "<b/>"),
        (font.italic, "<i/>"),
        (font.strike, "<strike/>"),
        (font.underline, "<u/>"),
    ] {
        if on {
            xml.push_str(element);
        }
    }
    let size = font.size_tenths.map_or(110, |size| size.clamp(10, 4_090));
    let _ = write!(
        xml,
        r#"<sz val="{}"/>"#,
        number_text(f64::from(size) / 10.0)
    );
    if let Some(colour) = &font.colour {
        let _ = write!(xml, r#"<color rgb="FF{colour}"/>"#);
    }
    let _ = write!(
        xml,
        r#"<name val="{}"/></font>"#,
        escape(font.family.as_deref().unwrap_or("Calibri"))
    );
    xml
}

fn borders_xml(borders: &CellBorders) -> String {
    let mut xml = String::from("<border>");
    for (name, edge) in [
        ("left", &borders.left),
        ("right", &borders.right),
        ("top", &borders.top),
        ("bottom", &borders.bottom),
    ] {
        match edge.as_ref().filter(|edge| edge.is_visible()) {
            Some(edge) => {
                let colour = edge.colour.as_ref().map_or_else(
                    || r#"<color auto="1"/>"#.to_owned(),
                    |colour| format!(r#"<color rgb="FF{colour}"/>"#),
                );
                let _ = write!(
                    xml,
                    r#"<{name} style="{}">{colour}</{name}>"#,
                    border_style_name(edge)
                );
            }
            None => {
                let _ = write!(xml, "<{name}/>");
            }
        }
    }
    xml.push_str("<diagonal/></border>");
    xml
}

/// The Excel style of a border: its line, and thin, medium or thick by its width.
fn border_style_name(border: &Border) -> &'static str {
    match (border.style, border.width_eighths) {
        (BorderStyle::Double, _) => "double",
        (BorderStyle::Dotted, width) if width <= 2 => "hair",
        (BorderStyle::Dotted, _) => "dotted",
        (BorderStyle::Dashed, width) if width >= 8 => "mediumDashed",
        (BorderStyle::Dashed, _) => "dashed",
        (_, width) if width >= 12 => "thick",
        (_, width) if width >= 8 => "medium",
        _ => "thin",
    }
}

fn alignment_xml(style: &CellStyle) -> String {
    let mut xml = String::new();
    if let Some(horizontal) = style.horizontal {
        let value = match horizontal {
            HorizontalAlignment::Left => "left",
            HorizontalAlignment::Center => "center",
            HorizontalAlignment::Right => "right",
            HorizontalAlignment::Justify => "justify",
        };
        let _ = write!(xml, r#" horizontal="{value}""#);
    }
    if let Some(vertical) = style.vertical {
        let value = match vertical {
            VerticalAlignment::Top => "top",
            VerticalAlignment::Center => "center",
            VerticalAlignment::Bottom => "bottom",
        };
        let _ = write!(xml, r#" vertical="{value}""#);
    }
    if style.wrap {
        xml.push_str(r#" wrapText="1""#);
    }
    xml
}

fn sheet_part(sheet: &Sheet, values: &HashMap<CellRef, Value>, styles: &Styles) -> String {
    let mut xml = String::from(concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" "#,
        r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">"#
    ));
    if !sheet.column_widths.is_empty() {
        xml.push_str("<cols>");
        for (column, width) in &sheet.column_widths {
            let _ = write!(
                xml,
                r#"<col min="{0}" max="{0}" width="{1}" customWidth="1"{2}/>"#,
                column + 1,
                number_text(*width),
                if *width == 0.0 { r#" hidden="1""# } else { "" }
            );
        }
        xml.push_str("</cols>");
    }
    xml.push_str("<sheetData>");
    let mut positions: BTreeSet<CellRef> = sheet.cells.keys().copied().collect();
    positions.extend(sheet.styles.keys().copied());
    let mut rows: BTreeSet<u32> = positions.iter().map(|cell| cell.row).collect();
    rows.extend(sheet.row_heights.keys().copied());
    let mut cells = positions.into_iter().peekable();
    for row in rows {
        let height = match sheet.row_heights.get(&row) {
            Some(height) if *height == 0.0 => r#" hidden="1""#.to_owned(),
            Some(height) => format!(r#" ht="{}" customHeight="1""#, number_text(*height)),
            None => String::new(),
        };
        let _ = write!(xml, r#"<row r="{}"{height}>"#, row + 1);
        while let Some(cell) = cells.next_if(|cell| cell.row == row) {
            let style_id = sheet.styles.get(&cell).copied().unwrap_or(0);
            let format = sheet
                .cells
                .get(&cell)
                .map_or(NumberFormat::General, |content| content.format);
            let index = styles.index(style_id, format);
            let style = if index == 0 {
                String::new()
            } else {
                format!(r#" s="{index}""#)
            };
            let reference = format!("{}{}", column_name(cell.column), cell.row + 1);
            match sheet.cells.get(&cell).map(|content| &content.input) {
                None => {
                    let _ = write!(xml, r#"<c r="{reference}"{style}/>"#);
                }
                Some(input) => write_cell(&mut xml, &reference, &style, input, values.get(&cell)),
            }
        }
        xml.push_str("</row>");
    }
    xml.push_str("</sheetData>");
    if !sheet.merges.is_empty() {
        let _ = write!(xml, r#"<mergeCells count="{}">"#, sheet.merges.len());
        for merge in &sheet.merges {
            let _ = write!(
                xml,
                r#"<mergeCell ref="{}{}:{}{}"/>"#,
                column_name(merge.first_column),
                merge.first_row + 1,
                column_name(merge.last_column),
                merge.last_row + 1
            );
        }
        xml.push_str("</mergeCells>");
    }
    xml.push_str("</worksheet>");
    xml
}

fn write_cell(
    xml: &mut String,
    reference: &str,
    style: &str,
    input: &Input,
    value: Option<&Value>,
) {
    match input {
        Input::Number(number) => {
            let _ = write!(
                xml,
                r#"<c r="{reference}"{style}><v>{}</v></c>"#,
                number_text(*number)
            );
        }
        Input::Text(text) => {
            let _ = write!(
                xml,
                r#"<c r="{reference}"{style} t="inlineStr"><is><t xml:space="preserve">{}</t></is></c>"#,
                escape(text)
            );
        }
        Input::Bool(value) => {
            let _ = write!(
                xml,
                r#"<c r="{reference}"{style} t="b"><v>{}</v></c>"#,
                u8::from(*value)
            );
        }
        Input::Formula(expression) => {
            let formula = escape(&print_formula(expression));
            let (kind, cached) = match value.unwrap_or(&Value::Empty) {
                Value::Empty => ("", String::new()),
                Value::Number(number) => ("", format!("<v>{}</v>", number_text(*number))),
                Value::Text(text) => (r#" t="str""#, format!("<v>{}</v>", escape(text))),
                Value::Bool(value) => (r#" t="b""#, format!("<v>{}</v>", u8::from(*value))),
                // Excel has no circular error value; it shows the cell as a broken reference.
                Value::Error(ErrorKind::Circular) => (r#" t="e""#, "<v>#REF!</v>".to_owned()),
                Value::Error(error) => (r#" t="e""#, format!("<v>{}</v>", error.code())),
            };
            let _ = write!(
                xml,
                r#"<c r="{reference}"{style}{kind}><f>{formula}</f>{cached}</c>"#
            );
        }
    }
}

fn workbook_part(workbook: &Workbook) -> String {
    let mut xml = String::from(concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" "#,
        r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>"#
    ));
    for (index, sheet) in workbook.sheets.iter().enumerate() {
        let _ = write!(
            xml,
            r#"<sheet name="{}" sheetId="{}" r:id="rId{}"/>"#,
            escape(&sheet.name),
            index + 1,
            index + 1
        );
    }
    xml.push_str(r#"</sheets><calcPr calcId="191029" fullCalcOnLoad="1"/></workbook>"#);
    xml
}

fn workbook_relationships(sheets: usize) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for index in 1..=sheets {
        let _ = write!(
            xml,
            r#"<Relationship Id="rId{index}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{index}.xml"/>"#
        );
    }
    let _ = write!(
        xml,
        r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#,
        sheets + 1
    );
    xml
}

fn content_types(sheets: usize) -> String {
    let mut xml = String::from(concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">"#,
        r#"<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>"#,
        r#"<Default Extension="xml" ContentType="application/xml"/>"#,
        r#"<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>"#,
        r#"<Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>"#,
    ));
    for index in 1..=sheets {
        let _ = write!(
            xml,
            r#"<Override PartName="/xl/worksheets/sheet{index}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#
        );
    }
    xml.push_str("</Types>");
    xml
}

const PACKAGE_RELATIONSHIPS: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>"#,
    "</Relationships>"
);

/// Text escaped for XML, without the characters XML 1.0 cannot hold.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\t' | '\n' | '\r' => escaped.push(character),
            character if character.is_control() => {}
            '\u{FFFE}' | '\u{FFFF}' => {}
            character => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests;
