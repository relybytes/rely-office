// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! A workbook read from a file and written to xlsx. Reading keeps what an editor needs: every
//! cell's formula or value, dates as dates, column widths, row heights, cell styles, merged
//! cells and sheet names. Writing stores each formula with its computed value, so a viewer that
//! does not recalculate still shows the results, and asks Excel to recalculate on opening.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::io::{Cursor, Write};

use calamine::{Data, Reader, open_workbook_auto_from_rs};
use rely_office_core::{
    Border, BorderStyle, CarriedPart, CarriedPivot, CellBorders, CellFont, CellRange, CellStyle,
    DocumentFormat, HorizontalAlignment, Limits, OfficeError, SheetLayout, VerticalAlignment,
    WorkbookName, delimited_separator, detect_format, excel_formula_from_odf, holds_macros,
    insert_macro_project, read_macro_project, read_ods_sheet_layouts, read_workbook,
    read_workbook_names, read_xlsx_code_names, read_xlsx_drawing_parts, read_xlsx_layouts,
    read_xlsx_pivot_parts, read_xlsx_theme_part,
};
use zip::write::SimpleFileOptions;

use crate::address::{CellRef, MAX_COLUMNS, MAX_ROWS, column_name};
use crate::arrange::AutoFilter;
use crate::charts::drawing_of_pictures;
use crate::dates::serial_from_date;
use crate::formula::{
    Expr, PREFIXED_FUNCTIONS, WORKSHEET_PREFIXED_FUNCTIONS, calls_external, number_text,
    parse_formula, print_formula,
};
use crate::model::{
    Cell, Comment, DefinedName, MAX_CELLS, MAX_SHEETS, NumberFormat, Sheet, Workbook, is_valid_name,
};
use crate::value::{ErrorKind, Input, Locale, Value};

mod pivots;
mod rules;

use rules::{Differentials, conditional_formatting_xml, data_validations_xml};

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
            let mut sheets = read_binary(bytes, format == DocumentFormat::Ods)?;
            if format == DocumentFormat::Ods {
                let mut layouts = read_ods_sheet_layouts(bytes, limits);
                let mut counter = 0;
                for sheet in &mut sheets {
                    if let Some(layout) = layouts.remove(&sheet.name) {
                        apply_layout(sheet, &layout, &mut styles);
                        apply_view(sheet, &layout);
                        sheet.drawing = drawing_of_pictures(&layout.drawings, &mut counter);
                        sheet.pictures = layout.drawings;
                    }
                }
            }
            if format == DocumentFormat::Xlsx {
                let mut layouts = read_xlsx_layouts(bytes, limits);
                // The layout's view and pictures are taken after its looks are applied.
                let mut layouts_for_view: HashMap<String, SheetLayout> = HashMap::new();
                let mut drawings = read_xlsx_drawing_parts(bytes, limits);
                let mut carried_pivots = read_xlsx_pivot_parts(bytes, limits);
                for sheet in &mut sheets {
                    if let Some(layout) = layouts.remove(&sheet.name) {
                        apply_layout(sheet, &layout, &mut styles);
                        layouts_for_view.insert(sheet.name.clone(), layout);
                    }
                    sheet.drawing = drawings.remove(&sheet.name);
                    if let Some(layout) = layouts_for_view.remove(&sheet.name) {
                        apply_view(sheet, &layout);
                        sheet.pictures = layout.drawings;
                    }
                    let (tables, kept) = pivots::model_pivots(
                        carried_pivots.remove(&sheet.name).unwrap_or_default(),
                        &sheet.name,
                        locale,
                    );
                    sheet.pivots = kept;
                    sheet.pivot_tables = tables;
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
    let count = sheets.len();
    let mut workbook = Workbook::from_parts(sheets, styles.styles);
    workbook.delimiter = delimited_separator(bytes, format);
    if matches!(format, DocumentFormat::Xlsx | DocumentFormat::Ods) {
        workbook.names = defined_names(read_workbook_names(bytes, limits), count);
        workbook.recalculate();
    }
    if format == DocumentFormat::Xlsx {
        workbook.theme = read_xlsx_theme_part(bytes, limits);
        workbook.macro_project = read_macro_project(bytes, limits);
        if workbook.macro_project.is_some() {
            let mut code_names = read_xlsx_code_names(bytes, limits);
            workbook.code_name = code_names.workbook;
            for sheet in &mut workbook.sheets {
                sheet.code_name = code_names.sheets.remove(&sheet.name);
            }
        }
    }
    if format == DocumentFormat::Ods {
        let pending = crate::ods::read_data_pilots(bytes, limits);
        crate::ods::place_data_pilots(&mut workbook, pending, locale);
    }
    workbook.snapshot_pivot_tables();
    workbook.holds_macros = match format {
        DocumentFormat::Xlsx => workbook.macro_project.is_some(),
        DocumentFormat::Xls => holds_macros(bytes, limits),
        _ => false,
    };
    Ok(workbook)
}

/// The names a file defines that this model can compute: one Excel would refuse, one for a sheet
/// the workbook lacks or one whose formula does not parse is left out.
fn defined_names(names: Vec<WorkbookName>, sheets: usize) -> Vec<DefinedName> {
    names
        .into_iter()
        .filter(|name| is_valid_name(&name.name) && name.sheet.is_none_or(|sheet| sheet < sheets))
        .filter_map(|name| {
            let formula = name.formula.trim();
            let expression = parse_formula(formula.strip_prefix('=').unwrap_or(formula)).ok()?;
            Some(DefinedName {
                name: name.name,
                sheet: name.sheet,
                expression,
                hidden: name.hidden,
            })
        })
        .collect()
}

/// What a file says of how a sheet is viewed: its frozen panes and its filter, and the arrays
/// its formulas spill.
fn apply_view(sheet: &mut Sheet, layout: &SheetLayout) {
    drop_spilled_values(sheet, &layout.array_formulas);
    sheet.comments = layout
        .comments
        .iter()
        .filter(|comment| comment.row < MAX_ROWS && comment.column < MAX_COLUMNS)
        .map(|comment| {
            (
                CellRef::new(comment.row, comment.column),
                Comment {
                    author: comment.author.clone(),
                    text: comment.text.clone(),
                    date: comment.date.clone(),
                    threaded: comment.threaded,
                    resolved: comment.resolved,
                    replies: comment.replies.clone(),
                },
            )
        })
        .collect();
    sheet.frozen_rows = layout.frozen_rows;
    sheet.frozen_columns = layout.frozen_columns;
    sheet.auto_filter = layout
        .auto_filter
        .as_ref()
        .map(|filter| AutoFilter::from_layout(filter, &sheet.row_heights));
}

/// Forget the values a file cached in the cells an array formula fills: the formula computes
/// them again, and kept as typed values they would block its spill.
fn drop_spilled_values(sheet: &mut Sheet, arrays: &[((u32, u32), CellRange)]) {
    for ((row, column), range) in arrays {
        let anchor = CellRef::new(*row, *column);
        let is_formula = sheet
            .cells
            .get(&anchor)
            .is_some_and(|cell| matches!(cell.input, Input::Formula(_)));
        if !is_formula {
            continue;
        }
        sheet
            .cells
            .retain(|cell, _| *cell == anchor || !range.contains(cell.row, cell.column));
    }
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
    sheet
        .conditional_formats
        .clone_from(&layout.conditional_formats);
    sheet.validations.clone_from(&layout.validations);
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
        // The second of each pair is how an ods the model wrote reads back.
        "m/d/yyyy" | "dd/mm/yyyy" => Some(NumberFormat::Date),
        "m/d/yyyy h:mm" | "dd/mm/yyyy hh:mm" => Some(NumberFormat::DateTime),
        "0.##%" => Some(NumberFormat::Percent),
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

/// The sheets of an xlsx, xls or ods; an ods's formulas, in `OpenDocument`'s notation, are read
/// in Excel's.
fn read_binary(bytes: &[u8], odf_formulas: bool) -> Result<Vec<Sheet>, OfficeError> {
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
        let mut stored: BTreeMap<CellRef, (Expr, Value)> = BTreeMap::new();
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
                let formula = if odf_formulas {
                    excel_formula_from_odf(formula)
                } else {
                    formula.clone()
                };
                let input = parse_formula(&formula).map_or_else(
                    |_| Input::Text(format!("={formula}")),
                    |expression| Input::Formula(Box::new(expression)),
                );
                if let Input::Formula(expression) = &input
                    && calls_external(expression)
                {
                    let value = cells.get(&position).map_or(Value::Empty, stored_cell_value);
                    stored.insert(position, ((**expression).clone(), value));
                }
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
            stored,
            ..Sheet::default()
        });
    }
    Ok(sheets)
}

/// The value a file stored in a cell, as it was read before its formula replaced it.
fn stored_cell_value(cell: &Cell) -> Value {
    match &cell.input {
        Input::Number(number) => Value::Number(*number),
        Input::Text(text) => Value::Text(text.clone()),
        Input::Bool(value) => Value::Bool(*value),
        Input::Formula(expression) => match **expression {
            Expr::Error(kind) => Value::Error(kind),
            _ => Value::Empty,
        },
    }
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
        // An ods keeps dates and durations as ISO text: a date is a serial, as in an xlsx.
        Data::DateTimeIso(text) => match iso_date_serial(text) {
            Some(serial) => Some(Cell {
                input: Input::Number(serial),
                format: if serial.fract() == 0.0 {
                    NumberFormat::Date
                } else {
                    NumberFormat::DateTime
                },
            }),
            None => general(Input::Text(text.clone())),
        },
        Data::DurationIso(text) => general(
            iso_duration_days(text).map_or_else(|| Input::Text(text.clone()), Input::Number),
        ),
        Data::String(text) => general(Input::Text(text.clone())),
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

/// The serial of an ISO date, `2026-10-27` or `2026-10-27T18:30:00.5`.
fn iso_date_serial(text: &str) -> Option<f64> {
    let (date, time) = text.split_once('T').unwrap_or((text, ""));
    let mut parts = date.splitn(3, '-').map(|part| part.parse::<i64>().ok());
    let (year, month, day) = (parts.next()??, parts.next()??, parts.next()??);
    let serial = serial_from_date(year, month, day)?;
    if time.is_empty() {
        return Some(serial);
    }
    let mut parts = time.splitn(3, ':').map(|part| part.parse::<f64>().ok());
    let hours = parts.next()??;
    let minutes = parts.next().flatten().unwrap_or(0.0);
    let seconds = parts.next().flatten().unwrap_or(0.0);
    Some(serial + (hours * 3_600.0 + minutes * 60.0 + seconds) / 86_400.0)
}

/// The days of an ISO duration, `PT36H30M00S`, as a sheet keeps a time.
fn iso_duration_days(text: &str) -> Option<f64> {
    let (negative, rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let rest = rest.strip_prefix('P')?;
    let (days, time) = rest.split_once('T').unwrap_or((rest, ""));
    let mut total = if days.is_empty() {
        0.0
    } else {
        days.strip_suffix('D')?.parse::<f64>().ok()?
    };
    let mut number = String::new();
    for character in time.chars() {
        let unit = match character {
            'H' => 24.0,
            'M' => 1_440.0,
            'S' => 86_400.0,
            digit if digit.is_ascii_digit() || digit == '.' => {
                number.push(digit);
                continue;
            }
            _ => return None,
        };
        total += number.parse::<f64>().ok()? / unit;
        number.clear();
    }
    Some(if negative { -total } else { total })
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
    let package = write_package(workbook)?;
    match &workbook.macro_project {
        Some(project) => insert_macro_project(&package, project),
        None => Ok(package),
    }
}

/// The package of a workbook, its macros aside.
#[expect(
    clippy::too_many_lines,
    reason = "the parts of a package are written in the order the content types list them"
)]
fn write_package(workbook: &Workbook) -> Result<Vec<u8>, OfficeError> {
    let differentials = Differentials::new(workbook);
    let styles = Styles::new(workbook, &differentials);

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
    let pivots = pivots::written_pivots(workbook);
    let carried = carried_parts(workbook, &pivots);
    let spills = workbook.spills.iter().any(|spills| !spills.is_empty());
    let commented: Vec<usize> = (0..count)
        .filter(|index| !workbook.sheets[*index].comments.is_empty())
        .collect();
    let threaded: Vec<usize> = (0..count)
        .filter(|index| has_threads(&workbook.sheets[*index]))
        .collect();
    let persons = thread_persons(workbook);
    add(
        "[Content_Types].xml",
        content_types(
            count,
            &carried,
            workbook.theme.is_some(),
            spills,
            &commented,
            &threaded,
        )
        .as_bytes(),
    )?;
    add("_rels/.rels", PACKAGE_RELATIONSHIPS.as_bytes())?;
    add(
        "xl/workbook.xml",
        workbook_part(workbook, &pivots).as_bytes(),
    )?;
    add(
        "xl/_rels/workbook.xml.rels",
        workbook_relationships(
            count,
            workbook.theme.is_some(),
            &pivot_caches(&pivots),
            spills,
            !persons.is_empty(),
        )
        .as_bytes(),
    )?;
    if !persons.is_empty() {
        add(PERSONS_PART, persons_part(&persons).as_bytes())?;
    }
    if let Some(theme) = &workbook.theme {
        add(THEME_PART, &theme.bytes)?;
    }
    if spills {
        add(METADATA_PART, DYNAMIC_ARRAY_METADATA.as_bytes())?;
    }
    add("xl/styles.xml", styles.xml.as_bytes())?;
    for (index, sheet) in workbook.sheets.iter().enumerate() {
        let xml = sheet_part(
            sheet,
            &workbook.values[index],
            &workbook.spills[index],
            &styles,
            &differentials,
        );
        add(
            &format!("xl/worksheets/sheet{}.xml", index + 1),
            xml.as_bytes(),
        )?;
        if sheet.drawing.is_some() || !pivots[index].is_empty() || !sheet.comments.is_empty() {
            add(
                &format!("xl/worksheets/_rels/sheet{}.xml.rels", index + 1),
                sheet_relationships(sheet, index, &pivots[index]).as_bytes(),
            )?;
        }
        if !sheet.comments.is_empty() {
            add(
                &format!("xl/comments{}.xml", index + 1),
                comments_part(sheet, index).as_bytes(),
            )?;
            if has_threads(sheet) {
                add(
                    &format!("xl/threadedComments/threadedComment{}.xml", index + 1),
                    threaded_comments_part(sheet, index).as_bytes(),
                )?;
            }
            add(
                &format!("xl/drawings/vmlDrawing{}.vml", index + 1),
                comment_shapes_part(sheet, index).as_bytes(),
            )?;
        }
    }
    for part in &carried {
        add(&part.name, &part.bytes)?;
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
fn styles_part(lists: &StyleLists, cell_formats: &str, count: usize, dxfs: &str) -> String {
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
    xml.push_str(
        r#"<cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles>"#,
    );
    xml.push_str(dxfs);
    xml.push_str("</styleSheet>");
    xml
}

impl Styles {
    fn new(workbook: &Workbook, differentials: &Differentials) -> Self {
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
            xml: styles_part(&lists, &cell_formats, used.len(), &differentials.xml()),
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
    } else if style.indent > 0 {
        // Excel indents only text aligned to an edge.
        xml.push_str(r#" horizontal="left""#);
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
    if style.indent > 0 {
        let _ = write!(xml, r#" indent="{}""#, style.indent);
    }
    xml
}

fn sheet_part(
    sheet: &Sheet,
    values: &HashMap<CellRef, Value>,
    spills: &BTreeMap<CellRef, CellRange>,
    styles: &Styles,
    differentials: &Differentials,
) -> String {
    let mut xml = String::from(concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" "#,
        r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">"#
    ));
    if let Some(code_name) = &sheet.code_name {
        let _ = write!(xml, r#"<sheetPr codeName="{}"/>"#, escape(code_name));
    }
    xml.push_str(&sheet_views_xml(sheet));
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
    // The cells an array spills into hold its values, which Excel caches as plain values.
    positions.extend(values.keys().filter(|cell| !sheet.cells.contains_key(cell)));
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
                None => match values.get(&cell) {
                    Some(value) => write_spilled(&mut xml, &reference, &style, value),
                    None => {
                        let _ = write!(xml, r#"<c r="{reference}"{style}/>"#);
                    }
                },
                Some(input) => write_cell(
                    &mut xml,
                    &reference,
                    &style,
                    input,
                    values.get(&cell),
                    spills.get(&cell),
                ),
            }
        }
        xml.push_str("</row>");
    }
    xml.push_str("</sheetData>");
    xml.push_str(&auto_filter_xml(sheet));
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
    xml.push_str(&conditional_formatting_xml(sheet, differentials));
    xml.push_str(&data_validations_xml(sheet));
    if sheet.drawing.is_some() {
        let _ = write!(xml, r#"<drawing r:id="{DRAWING_RELATIONSHIP}"/>"#);
    }
    if !sheet.comments.is_empty() {
        let _ = write!(
            xml,
            r#"<legacyDrawing r:id="{COMMENT_SHAPES_RELATIONSHIP}"/>"#
        );
    }
    xml.push_str("</worksheet>");
    xml
}

/// The view of a sheet: its frozen rows and columns, when it has any.
fn sheet_views_xml(sheet: &Sheet) -> String {
    if sheet.frozen_rows == 0 && sheet.frozen_columns == 0 {
        return String::new();
    }
    let mut pane = String::from("<pane");
    if sheet.frozen_columns > 0 {
        let _ = write!(pane, r#" xSplit="{}""#, sheet.frozen_columns);
    }
    if sheet.frozen_rows > 0 {
        let _ = write!(pane, r#" ySplit="{}""#, sheet.frozen_rows);
    }
    let active = match (sheet.frozen_rows > 0, sheet.frozen_columns > 0) {
        (true, true) => "bottomRight",
        (true, false) => "bottomLeft",
        _ => "topRight",
    };
    let _ = write!(
        pane,
        r#" topLeftCell="{}{}" activePane="{active}" state="frozen"/>"#,
        column_name(sheet.frozen_columns),
        sheet.frozen_rows + 1
    );
    format!(
        r#"<sheetViews><sheetView workbookViewId="0">{pane}<selection pane="{active}"/></sheetView></sheetViews>"#
    )
}

/// The filter buttons of a sheet and the values its filtered columns show.
fn auto_filter_xml(sheet: &Sheet) -> String {
    let Some(filter) = sheet.auto_filter() else {
        return String::new();
    };
    let range = &filter.range;
    let mut xml = format!(
        r#"<autoFilter ref="{}{}:{}{}""#,
        column_name(range.first_column),
        range.first_row + 1,
        column_name(range.last_column),
        range.last_row + 1
    );
    let columns: Vec<u32> = filter.filtered_columns().collect();
    if columns.is_empty() {
        xml.push_str("/>");
        return xml;
    }
    xml.push('>');
    for column in columns {
        let shown = filter.shown(column).cloned().unwrap_or_default();
        let blank = if shown.contains("") {
            r#" blank="1""#
        } else {
            ""
        };
        let _ = write!(xml, r#"<filterColumn colId="{column}"><filters{blank}>"#);
        for value in shown.iter().filter(|value| !value.is_empty()) {
            let _ = write!(xml, r#"<filter val="{}"/>"#, escape(value));
        }
        xml.push_str("</filters></filterColumn>");
    }
    xml.push_str("</autoFilter>");
    xml
}

/// The names the workbook defines, and the hidden ones Excel keeps for the range of each sheet's
/// filter.
fn defined_names_xml(workbook: &Workbook) -> String {
    let mut names: Vec<String> = workbook
        .names
        .iter()
        .map(|defined| {
            let mut xml = format!(r#"<definedName name="{}""#, escape(&defined.name));
            if let Some(sheet) = defined.sheet {
                let _ = write!(xml, r#" localSheetId="{sheet}""#);
            }
            if defined.hidden {
                xml.push_str(r#" hidden="1""#);
            }
            let _ = write!(
                xml,
                ">{}</definedName>",
                escape(&print_formula(&file_names(&defined.expression)))
            );
            xml
        })
        .collect();
    let filters: Vec<String> = workbook
        .sheets
        .iter()
        .enumerate()
        .filter_map(|(index, sheet)| {
            let range = sheet.auto_filter()?.range;
            Some(format!(
                r#"<definedName name="_xlnm._FilterDatabase" localSheetId="{index}" hidden="1">'{}'!${}${}:${}${}</definedName>"#,
                escape(&sheet.name.replace('\'', "''")),
                column_name(range.first_column),
                range.first_row + 1,
                column_name(range.last_column),
                range.last_row + 1
            ))
        })
        .collect();
    names.extend(filters);
    if names.is_empty() {
        String::new()
    } else {
        format!("<definedNames>{}</definedNames>", names.concat())
    }
}

/// A formula with the functions newer than Excel 2007 named as a file names them, and the names
/// a `LET` or a `LAMBDA` binds with the `_xlpm.` Excel writes before them.
fn file_names(expression: &Expr) -> Expr {
    file_names_within(expression, &mut Vec::new())
}

fn file_names_within(expression: &Expr, bound: &mut Vec<String>) -> Expr {
    if let Expr::Name(name) = expression
        && bound.contains(&name.to_ascii_lowercase())
    {
        return Expr::Name(format!("_xlpm.{name}"));
    }
    let mut boxed = |inner: &Expr| Box::new(file_names_within(inner, bound));
    match expression {
        Expr::Call(name, arguments) => {
            let file_name = if WORKSHEET_PREFIXED_FUNCTIONS.contains(&name.as_str()) {
                format!("_xlfn._xlws.{name}")
            } else if PREFIXED_FUNCTIONS.contains(&name.as_str()) {
                format!("_xlfn.{name}")
            } else {
                name.clone()
            };
            let depth = bound.len();
            let last = arguments.len().saturating_sub(1);
            let mut written = Vec::with_capacity(arguments.len());
            for (index, argument) in arguments.iter().enumerate() {
                let binds = match name.as_str() {
                    "LET" => index % 2 == 0 && index < last,
                    "LAMBDA" => index < last,
                    _ => false,
                };
                match argument {
                    Expr::Name(parameter) if binds => {
                        bound.push(parameter.to_ascii_lowercase());
                        written.push(Expr::Name(format!("_xlpm.{parameter}")));
                    }
                    argument => written.push(file_names_within(argument, bound)),
                }
            }
            bound.truncate(depth);
            Expr::Call(file_name, written)
        }
        Expr::Invoke(callee, arguments) => {
            let callee = boxed(callee);
            Expr::Invoke(
                callee,
                arguments
                    .iter()
                    .map(|argument| file_names_within(argument, bound))
                    .collect(),
            )
        }
        Expr::Unary(operator, operand) => Expr::Unary(*operator, boxed(operand)),
        Expr::Percent(operand) => Expr::Percent(boxed(operand)),
        Expr::Group(operand) => Expr::Group(boxed(operand)),
        Expr::Binary(operator, left, right) => {
            let left = boxed(left);
            Expr::Binary(*operator, left, Box::new(file_names_within(right, bound)))
        }
        other => other.clone(),
    }
}

/// A cell an array spills into, its value cached as Excel caches it.
fn write_spilled(xml: &mut String, reference: &str, style: &str, value: &Value) {
    let _ = match value {
        Value::Empty => write!(xml, r#"<c r="{reference}"{style}/>"#),
        Value::Number(number) => write!(
            xml,
            r#"<c r="{reference}"{style}><v>{}</v></c>"#,
            number_text(*number)
        ),
        Value::Text(text) => write!(
            xml,
            r#"<c r="{reference}"{style} t="inlineStr"><is><t xml:space="preserve">{}</t></is></c>"#,
            escape(text)
        ),
        Value::Bool(value) => write!(
            xml,
            r#"<c r="{reference}"{style} t="b"><v>{}</v></c>"#,
            u8::from(*value)
        ),
        Value::Error(error) => write!(
            xml,
            r#"<c r="{reference}"{style} t="e"><v>{}</v></c>"#,
            error.code()
        ),
    };
}

fn write_cell(
    xml: &mut String,
    reference: &str,
    style: &str,
    input: &Input,
    value: Option<&Value>,
    spill: Option<&CellRange>,
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
            let formula = escape(&print_formula(&file_names(expression)));
            let (kind, cached) = match value.unwrap_or(&Value::Empty) {
                Value::Empty => ("", String::new()),
                Value::Number(number) => ("", format!("<v>{}</v>", number_text(*number))),
                Value::Text(text) => (r#" t="str""#, format!("<v>{}</v>", escape(text))),
                Value::Bool(value) => (r#" t="b""#, format!("<v>{}</v>", u8::from(*value))),
                // Excel has no circular error value; it shows the cell as a broken reference.
                Value::Error(ErrorKind::Circular) => (r#" t="e""#, "<v>#REF!</v>".to_owned()),
                Value::Error(error) => (r#" t="e""#, format!("<v>{}</v>", error.code())),
            };
            // A formula that spills is a dynamic array: Excel finds it by the cell's metadata.
            let _ = match spill {
                Some(range) => write!(
                    xml,
                    r#"<c r="{reference}"{style}{kind} cm="1"><f t="array" ref="{}{}:{}{}">{formula}</f>{cached}</c>"#,
                    column_name(range.first_column),
                    range.first_row + 1,
                    column_name(range.last_column),
                    range.last_row + 1
                ),
                None => write!(
                    xml,
                    r#"<c r="{reference}"{style}{kind}><f>{formula}</f>{cached}</c>"#
                ),
            };
        }
    }
}

fn workbook_part(workbook: &Workbook, pivots: &[Vec<CarriedPivot>]) -> String {
    let mut xml = String::from(concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" "#,
        r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">"#
    ));
    if let Some(code_name) = &workbook.code_name {
        let _ = write!(xml, r#"<workbookPr codeName="{}"/>"#, escape(code_name));
    }
    xml.push_str("<sheets>");
    for (index, sheet) in workbook.sheets.iter().enumerate() {
        let _ = write!(
            xml,
            r#"<sheet name="{}" sheetId="{}" r:id="rId{}"/>"#,
            escape(&sheet.name),
            index + 1,
            index + 1
        );
    }
    xml.push_str("</sheets>");
    xml.push_str(&defined_names_xml(workbook));
    xml.push_str(r#"<calcPr calcId="191029" fullCalcOnLoad="1"/>"#);
    let caches = pivot_caches(pivots);
    if !caches.is_empty() {
        xml.push_str("<pivotCaches>");
        for (index, (_, cache_id)) in caches.iter().enumerate() {
            let _ = write!(
                xml,
                r#"<pivotCache cacheId="{cache_id}" r:id="rIdPivotCache{}"/>"#,
                index + 1
            );
        }
        xml.push_str("</pivotCaches>");
    }
    xml.push_str("</workbook>");
    xml
}

/// The pivot caches the sheets' pivot tables read, each once with its id, in sheet order.
fn pivot_caches(pivots: &[Vec<CarriedPivot>]) -> Vec<(&str, u32)> {
    let mut caches: Vec<(&str, u32)> = Vec::new();
    for pivot in pivots.iter().flatten() {
        if !caches.iter().any(|(part, _)| *part == pivot.cache_part) {
            caches.push((&pivot.cache_part, pivot.cache_id));
        }
    }
    caches
}

/// A part's member name as a relationship of a part in `xl/` or below it names it.
fn target_from(depth: usize, part: &str) -> String {
    part.strip_prefix("xl/").map_or_else(
        || format!("/{part}"),
        |inside| format!("{}{inside}", "../".repeat(depth)),
    )
}

/// Where the carried theme is written, whatever its name in the file it came from.
const THEME_PART: &str = "xl/theme/theme1.xml";

fn workbook_relationships(
    sheets: usize,
    theme: bool,
    caches: &[(&str, u32)],
    metadata: bool,
    persons: bool,
) -> String {
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
        r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#,
        sheets + 1
    );
    if theme {
        let _ = write!(
            xml,
            r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>"#,
            sheets + 2
        );
    }
    for (index, (part, _)) in caches.iter().enumerate() {
        let _ = write!(
            xml,
            r#"<Relationship Id="rIdPivotCache{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheDefinition" Target="{}"/>"#,
            index + 1,
            escape(&target_from(0, part))
        );
    }
    if metadata {
        xml.push_str(r#"<Relationship Id="rIdMetadata" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sheetMetadata" Target="metadata.xml"/>"#);
    }
    if persons {
        xml.push_str(r#"<Relationship Id="rIdPersons" Type="http://schemas.microsoft.com/office/2017/10/relationships/person" Target="persons/person.xml"/>"#);
    }
    xml.push_str("</Relationships>");
    xml
}

/// The parts the drawings of the sheets carry, each once; none can take the place of a part the
/// writer makes itself.
fn carried_parts<'w>(
    workbook: &'w Workbook,
    pivots: &'w [Vec<CarriedPivot>],
) -> Vec<&'w CarriedPart> {
    let mut names = HashSet::new();
    workbook
        .sheets
        .iter()
        .flat_map(|sheet| {
            sheet
                .drawing
                .iter()
                .flat_map(|drawing| drawing.parts.iter())
        })
        .chain(pivots.iter().flatten().flat_map(|pivot| pivot.parts.iter()))
        .filter(|part| {
            let name = part.name.to_ascii_lowercase();
            !name.starts_with("xl/worksheets/")
                && !name.starts_with("xl/_rels/")
                && !name.starts_with("_rels/")
                && !name.starts_with('[')
                && name != "xl/workbook.xml"
                && name != "xl/styles.xml"
                && name != THEME_PART
                && names.insert(name)
        })
        .collect()
}

/// The relationships of a worksheet: its drawing, its pivot tables and its comments.
fn sheet_relationships(sheet: &Sheet, index: usize, pivots: &[CarriedPivot]) -> String {
    let mut xml = String::from(concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    ));
    if let Some(drawing) = &sheet.drawing {
        let _ = write!(
            xml,
            r#"<Relationship Id="{DRAWING_RELATIONSHIP}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing" Target="{}"/>"#,
            escape(&target_from(1, &drawing.drawing_part))
        );
    }
    for (index, pivot) in pivots.iter().enumerate() {
        let _ = write!(
            xml,
            r#"<Relationship Id="rIdPivot{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotTable" Target="{}"/>"#,
            index + 1,
            escape(&target_from(1, &pivot.table_part))
        );
    }
    if !sheet.comments.is_empty() {
        let _ = write!(
            xml,
            concat!(
                r#"<Relationship Id="rIdComments1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="../comments{0}.xml"/>"#,
                r#"<Relationship Id="{1}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/vmlDrawing" Target="../drawings/vmlDrawing{0}.vml"/>"#
            ),
            index + 1,
            COMMENT_SHAPES_RELATIONSHIP
        );
    }
    if has_threads(sheet) {
        let _ = write!(
            xml,
            r#"<Relationship Id="rIdThreadedComments1" Type="http://schemas.microsoft.com/office/2017/10/relationships/threadedComment" Target="../threadedComments/threadedComment{}.xml"/>"#,
            index + 1
        );
    }
    xml.push_str("</Relationships>");
    xml
}

/// The relationship a worksheet names its drawing by.
const DRAWING_RELATIONSHIP: &str = "rIdDrawing1";
/// The relationship a worksheet names the shapes of its comments by.
const COMMENT_SHAPES_RELATIONSHIP: &str = "rIdCommentShapes1";

/// The comments of a sheet, each author listed once. A thread is written as the note Excel puts
/// beside it for the versions that do not read threads: its author is `tc=` and the thread's id,
/// and its text the thread spelled out.
fn comments_part(sheet: &Sheet, index: usize) -> String {
    let author_of = |cell: &CellRef, comment: &Comment| {
        if comment.threaded {
            format!("tc={}", thread_id(index, *cell, None))
        } else {
            comment.author.clone()
        }
    };
    let mut authors: Vec<String> = Vec::new();
    for (cell, comment) in &sheet.comments {
        let author = author_of(cell, comment);
        if !authors.contains(&author) {
            authors.push(author);
        }
    }
    let mut xml = String::from(concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<comments xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" "#,
        r#"xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="xr" "#,
        r#"xmlns:xr="http://schemas.microsoft.com/office/spreadsheetml/2014/revision"><authors>"#
    ));
    for author in &authors {
        let _ = write!(xml, "<author>{}</author>", escape(author));
    }
    xml.push_str("</authors><commentList>");
    for (cell, comment) in &sheet.comments {
        let author = author_of(cell, comment);
        let author = authors
            .iter()
            .position(|known| *known == author)
            .unwrap_or(0);
        let (uid, text) = if comment.threaded {
            (
                format!(r#" xr:uid="{}""#, thread_id(index, *cell, None)),
                thread_note_text(comment),
            )
        } else {
            (String::new(), comment.text.clone())
        };
        let _ = write!(
            xml,
            r#"<comment ref="{}{}" authorId="{author}"{uid}><text><r><t xml:space="preserve">{}</t></r></text></comment>"#,
            column_name(cell.column),
            cell.row + 1,
            escape(&text)
        );
    }
    xml.push_str("</commentList></comments>");
    xml
}

/// Whether any comment of a sheet is a thread.
fn has_threads(sheet: &Sheet) -> bool {
    sheet.comments.values().any(|comment| comment.threaded)
}

/// A thread spelled out as a note's text: the comment and each reply under its author.
fn thread_note_text(comment: &Comment) -> String {
    let indented = |author: &str, text: &str| {
        let body = text.replace('\n', "\n    ");
        if author.is_empty() {
            format!("    {body}")
        } else {
            format!("    {author}: {body}")
        }
    };
    let mut text = format!(
        "[Threaded comment]\n\nComment:\n{}",
        indented(&comment.author, &comment.text)
    );
    for reply in &comment.replies {
        let _ = write!(text, "\nReply:\n{}", indented(&reply.author, &reply.text));
    }
    text
}

/// The id of a thread, or of one of its replies: a GUID made from where the thread is, so the
/// same workbook is always written the same way.
fn thread_id(sheet: usize, cell: CellRef, reply: Option<usize>) -> String {
    let seed = match reply {
        Some(reply) => format!("reply:{sheet}:{}:{}:{reply}", cell.row, cell.column),
        None => format!("thread:{sheet}:{}:{}", cell.row, cell.column),
    };
    guid(&seed)
}

/// A GUID in braces, as Office writes them, from a seed: two FNV-1a hashes with the version 4
/// and variant bits set.
fn guid(seed: &str) -> String {
    let hash = |offset: u64| {
        seed.bytes().fold(offset, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        })
    };
    let high = hash(0xcbf2_9ce4_8422_2325);
    let low = hash(0x8422_2325_cbf2_9ce4);
    let value = (u128::from(high) << 64 | u128::from(low)) & !(0xF_u128 << 76) & !(0x3_u128 << 62)
        | (0x4_u128 << 76)
        | (0x2_u128 << 62);
    let hex = format!("{value:032X}");
    format!(
        "{{{}-{}-{}-{}-{}}}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// The authors of the threads of a workbook, each once, in the order they first write.
fn thread_persons(workbook: &Workbook) -> Vec<&str> {
    let mut persons: Vec<&str> = Vec::new();
    for comment in workbook
        .sheets
        .iter()
        .flat_map(|sheet| sheet.comments.values())
        .filter(|comment| comment.threaded)
    {
        for author in std::iter::once(comment.author.as_str())
            .chain(comment.replies.iter().map(|reply| reply.author.as_str()))
        {
            if !persons.contains(&author) {
                persons.push(author);
            }
        }
    }
    persons
}

const PERSONS_PART: &str = "xl/persons/person.xml";
const THREADS_NAMESPACE: &str =
    "http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments";

/// The people who write the threads of a workbook, each with the id the threads name them by.
fn persons_part(persons: &[&str]) -> String {
    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><personList xmlns="{THREADS_NAMESPACE}" xmlns:x="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#
    );
    for person in persons {
        let shown = if person.is_empty() { "?" } else { person };
        let _ = write!(
            xml,
            r#"<person displayName="{}" id="{}" userId="{}" providerId="None"/>"#,
            escape(shown),
            guid(&format!("person:{person}")),
            escape(shown)
        );
    }
    xml.push_str("</personList>");
    xml
}

/// The threads of a sheet: each comment, then its replies naming it as their parent.
fn threaded_comments_part(sheet: &Sheet, index: usize) -> String {
    let mut xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><ThreadedComments xmlns="{THREADS_NAMESPACE}" xmlns:x="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#
    );
    let date = |date: &Option<String>| {
        date.as_ref()
            .map(|date| format!(r#" dT="{}""#, escape(date)))
            .unwrap_or_default()
    };
    for (cell, comment) in sheet
        .comments
        .iter()
        .filter(|(_, comment)| comment.threaded)
    {
        let reference = format!("{}{}", column_name(cell.column), cell.row + 1);
        let id = thread_id(index, *cell, None);
        let _ = write!(
            xml,
            r#"<threadedComment ref="{reference}"{} personId="{}" id="{id}"{}><text>{}</text></threadedComment>"#,
            date(&comment.date),
            guid(&format!("person:{}", comment.author)),
            if comment.resolved { r#" done="1""# } else { "" },
            escape(&comment.text)
        );
        for (number, reply) in comment.replies.iter().enumerate() {
            let _ = write!(
                xml,
                r#"<threadedComment ref="{reference}"{} personId="{}" id="{}" parentId="{id}"><text>{}</text></threadedComment>"#,
                date(&reply.date),
                guid(&format!("person:{}", reply.author)),
                thread_id(index, *cell, Some(number)),
                escape(&reply.text)
            );
        }
    }
    xml.push_str("</ThreadedComments>");
    xml
}

/// The note boxes Excel draws for the comments of a sheet, hidden until the cell is pointed at.
fn comment_shapes_part(sheet: &Sheet, index: usize) -> String {
    let mut xml = format!(
        concat!(
            r#"<xml xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:x="urn:schemas-microsoft-com:office:excel">"#,
            r#"<o:shapelayout v:ext="edit"><o:idmap v:ext="edit" data="{}"/></o:shapelayout>"#,
            r#"<v:shapetype id="_x0000_t202" coordsize="21600,21600" o:spt="202" path="m,l,21600r21600,l21600,xe">"#,
            r#"<v:stroke joinstyle="miter"/><v:path gradientshapeok="t" o:connecttype="rect"/></v:shapetype>"#
        ),
        index + 1
    );
    for (number, cell) in sheet.comments.keys().enumerate() {
        let _ = write!(
            xml,
            concat!(
                r##"<v:shape id="_x0000_s{}" type="#_x0000_t202" style="position:absolute;margin-left:59.25pt;margin-top:1.5pt;width:108pt;height:59.25pt;z-index:{};visibility:hidden" fillcolor="#ffffe1" o:insetmode="auto">"##,
                r##"<v:fill color2="#ffffe1"/><v:shadow on="t" color="black" obscured="t"/><v:path o:connecttype="none"/>"##,
                r##"<v:textbox style="mso-direction-alt:auto"><div style="text-align:left"></div></v:textbox>"##,
                r##"<x:ClientData ObjectType="Note"><x:MoveWithCells/><x:SizeWithCells/>"##,
                r##"<x:Anchor>{}, 15, {}, 2, {}, 15, {}, 16</x:Anchor><x:AutoFill>False</x:AutoFill>"##,
                r##"<x:Row>{}</x:Row><x:Column>{}</x:Column></x:ClientData></v:shape>"##
            ),
            (index + 1) * 1_024 + number + 1,
            number + 1,
            cell.column + 1,
            cell.row,
            cell.column + 3,
            cell.row + 4,
            cell.row,
            cell.column
        );
    }
    xml.push_str("</xml>");
    xml
}

fn content_types(
    sheets: usize,
    carried: &[&CarriedPart],
    theme: bool,
    metadata: bool,
    commented: &[usize],
    threaded: &[usize],
) -> String {
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
    if !commented.is_empty() {
        xml.push_str(r#"<Default Extension="vml" ContentType="application/vnd.openxmlformats-officedocument.vmlDrawing"/>"#);
    }
    for index in commented {
        let _ = write!(
            xml,
            r#"<Override PartName="/xl/comments{}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.comments+xml"/>"#,
            index + 1
        );
    }
    for index in threaded {
        let _ = write!(
            xml,
            r#"<Override PartName="/xl/threadedComments/threadedComment{}.xml" ContentType="application/vnd.ms-excel.threadedcomments+xml"/>"#,
            index + 1
        );
    }
    if !threaded.is_empty() {
        let _ = write!(
            xml,
            r#"<Override PartName="/{PERSONS_PART}" ContentType="application/vnd.ms-excel.person+xml"/>"#
        );
    }
    if theme {
        let _ = write!(
            xml,
            r#"<Override PartName="/{THEME_PART}" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>"#
        );
    }
    if metadata {
        let _ = write!(
            xml,
            r#"<Override PartName="/{METADATA_PART}" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheetMetadata+xml"/>"#
        );
    }
    for part in carried.iter().filter(|part| {
        part.content_type != "application/vnd.openxmlformats-package.relationships+xml"
    }) {
        let _ = write!(
            xml,
            r#"<Override PartName="/{}" ContentType="{}"/>"#,
            escape(&part.name),
            escape(&part.content_type)
        );
    }
    xml.push_str("</Types>");
    xml
}

const METADATA_PART: &str = "xl/metadata.xml";

/// The cell metadata that marks a formula as a dynamic array, as Excel 365 writes it; every
/// spilling formula names its one record with `cm="1"`.
const DYNAMIC_ARRAY_METADATA: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<metadata xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" "#,
    r#"xmlns:xda="http://schemas.microsoft.com/office/spreadsheetml/2017/dynamicarray">"#,
    r#"<metadataTypes count="1"><metadataType name="XLDAPR" minSupportedVersion="120000" copy="1" "#,
    r#"pasteAll="1" pasteValues="1" merge="1" splitFirst="1" rowColShift="1" clearFormats="1" "#,
    r#"clearComments="1" assign="1" coerce="1" cellMeta="1"/></metadataTypes>"#,
    r#"<futureMetadata name="XLDAPR" count="1"><bk><extLst><ext uri="{bdbb8cdc-fa1e-496e-a857-3c3f30c029c3}">"#,
    r#"<xda:dynamicArrayProperties fDynamic="1" fCollapsed="0"/></ext></extLst></bk></futureMetadata>"#,
    r#"<cellMetadata count="1"><bk><rc t="1" v="0"/></bk></cellMetadata></metadata>"#
);

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
