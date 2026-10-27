// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! A workbook read from a file and written to xlsx. Reading keeps what an editor needs: every
//! cell's formula or value, dates as dates, column widths and sheet names. Writing stores each
//! formula with its computed value, so a viewer that does not recalculate still shows the
//! results, and asks Excel to recalculate on opening.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::io::{Cursor, Write};

use calamine::{Data, Reader, open_workbook_auto_from_rs};
use rely_office_core::{
    DocumentFormat, Limits, OfficeError, detect_format, read_workbook, read_xlsx_layouts,
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
    let sheets = match format {
        DocumentFormat::Xlsx | DocumentFormat::Xls | DocumentFormat::Ods => {
            let mut sheets = read_binary(bytes)?;
            if format == DocumentFormat::Xlsx {
                let mut layouts = read_xlsx_layouts(bytes, limits);
                for sheet in &mut sheets {
                    if let Some(layout) = layouts.remove(&sheet.name) {
                        for (column, width) in layout.column_widths.iter().enumerate() {
                            if let (Some(width), Ok(column)) = (width, u32::try_from(column)) {
                                sheet
                                    .column_widths
                                    .insert(column, f64::from(*width) / 256.0);
                            }
                        }
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
    Ok(Workbook::from_sheets(sheets))
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
            column_widths: BTreeMap::new(),
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
        column_widths: BTreeMap::new(),
    })
}

/// Write a workbook as xlsx, each formula with its computed value.
///
/// # Errors
///
/// [`OfficeError::DamagedContainer`] when the zip cannot be written, which only a failing
/// allocator causes.
pub fn write_xlsx(workbook: &Workbook) -> Result<Vec<u8>, OfficeError> {
    let mut decimals: Vec<u8> = workbook
        .sheets
        .iter()
        .flat_map(|sheet| sheet.cells.values())
        .filter_map(|cell| match cell.format {
            NumberFormat::Decimals(count) => Some(count.min(10)),
            _ => None,
        })
        .collect();
    decimals.sort_unstable();
    decimals.dedup();
    let styles = Styles::new(&decimals);

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

/// The style index of each number format, and the styles part declaring them.
struct Styles {
    indexes: HashMap<NumberFormat, usize>,
    xml: String,
}

impl Styles {
    fn new(decimals: &[u8]) -> Self {
        // Built-in number formats: 14 date, 22 date and time, 10 percent with two decimals.
        let mut formats: Vec<(NumberFormat, u32)> = vec![
            (NumberFormat::General, 0),
            (NumberFormat::Date, 14),
            (NumberFormat::DateTime, 22),
            (NumberFormat::Percent, 10),
        ];
        let mut custom = String::new();
        for count in decimals {
            let id = 164 + u32::from(*count);
            let code = if *count == 0 {
                "0".to_owned()
            } else {
                format!("0.{}", "0".repeat(usize::from(*count)))
            };
            let _ = write!(custom, r#"<numFmt numFmtId="{id}" formatCode="{code}"/>"#);
            formats.push((NumberFormat::Decimals(*count), id));
        }
        let mut xml = String::from(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#,
        );
        if !decimals.is_empty() {
            let _ = write!(
                xml,
                r#"<numFmts count="{}">{custom}</numFmts>"#,
                decimals.len()
            );
        }
        xml.push_str(concat!(
            r#"<fonts count="1"><font><sz val="11"/><name val="Calibri"/><family val="2"/></font></fonts>"#,
            r#"<fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills>"#,
            r#"<borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders>"#,
            r#"<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>"#,
        ));
        let _ = write!(xml, r#"<cellXfs count="{}">"#, formats.len());
        let mut indexes = HashMap::new();
        for (index, (format, id)) in formats.iter().enumerate() {
            let apply = if *id == 0 {
                ""
            } else {
                r#" applyNumberFormat="1""#
            };
            let _ = write!(
                xml,
                r#"<xf numFmtId="{id}" fontId="0" fillId="0" borderId="0" xfId="0"{apply}/>"#
            );
            indexes.insert(*format, index);
        }
        xml.push_str(r#"</cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"#);
        Self { indexes, xml }
    }
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
    let mut current_row = None;
    for (cell, content) in &sheet.cells {
        if current_row != Some(cell.row) {
            if current_row.is_some() {
                xml.push_str("</row>");
            }
            let _ = write!(xml, r#"<row r="{}">"#, cell.row + 1);
            current_row = Some(cell.row);
        }
        let reference = format!("{}{}", column_name(cell.column), cell.row + 1);
        let style = styles.indexes.get(&content.format).copied().unwrap_or(0);
        let style = if style == 0 {
            String::new()
        } else {
            format!(r#" s="{style}""#)
        };
        match &content.input {
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
                let (kind, cached) = match values.get(cell).unwrap_or(&Value::Empty) {
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
    if current_row.is_some() {
        xml.push_str("</row>");
    }
    xml.push_str("</sheetData></worksheet>");
    xml
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
