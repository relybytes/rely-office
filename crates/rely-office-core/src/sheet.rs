// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use std::io::Cursor;

use calamine::{Data, Reader, open_workbook_auto_from_rs};

use crate::sheet_layout::{SheetLayout, read_xlsx_layouts};
use crate::{DocumentFormat, Limits, OfficeError, detect_format};

/// One sheet as a table of displayed values, cut to the limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sheet {
    pub name: String,
    /// The cells, row by row, from A1; every row has the same number of columns.
    pub rows: Vec<Vec<String>>,
    /// How many rows the sheet holds, before the cut.
    pub total_rows: usize,
    /// How many columns the sheet holds, before the cut.
    pub total_columns: usize,
    /// How the sheet is printed, read from an xlsx; the defaults for every other format.
    pub layout: SheetLayout,
}

/// The sheets of a workbook, in the order the file declares them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workbook {
    pub format: DocumentFormat,
    pub sheets: Vec<Sheet>,
}

/// Read a spreadsheet (xlsx, xls, ods, csv, tsv) as tables of the values a reader sees.
///
/// # Errors
///
/// [`OfficeError::UnsupportedFormat`] for a file that is not a spreadsheet,
/// [`OfficeError::TooLarge`] past [`Limits::max_input_bytes`], and
/// [`OfficeError::UnreadableWorkbook`] when the spreadsheet reader refuses the bytes.
pub fn read_workbook(
    bytes: &[u8],
    file_name: Option<&str>,
    limits: &Limits,
) -> Result<Workbook, OfficeError> {
    if bytes.len() as u64 > limits.max_input_bytes {
        return Err(OfficeError::TooLarge {
            what: "input".to_owned(),
            limit: limits.max_input_bytes,
        });
    }

    let format = detect_format(bytes, file_name);
    let sheets = match format {
        DocumentFormat::Xlsx => {
            let mut sheets = read_binary_workbook(bytes, limits)?;
            let mut layouts = read_xlsx_layouts(bytes, limits);
            for sheet in &mut sheets {
                sheet.layout = layouts.remove(&sheet.name).unwrap_or_default();
            }
            sheets
        }
        DocumentFormat::Xls | DocumentFormat::Ods => read_binary_workbook(bytes, limits)?,
        DocumentFormat::Csv => vec![read_delimited(bytes, b',', limits)],
        DocumentFormat::Tsv => vec![read_delimited(bytes, b'\t', limits)],
        other => return Err(OfficeError::UnsupportedFormat(other)),
    };

    Ok(Workbook { format, sheets })
}

fn read_binary_workbook(bytes: &[u8], limits: &Limits) -> Result<Vec<Sheet>, OfficeError> {
    let mut workbook = open_workbook_auto_from_rs(Cursor::new(bytes))
        .map_err(|error| OfficeError::UnreadableWorkbook(error.to_string()))?;

    let mut sheets = Vec::new();
    for name in workbook.sheet_names() {
        let range = workbook
            .worksheet_range(&name)
            .map_err(|error| OfficeError::UnreadableWorkbook(error.to_string()))?;
        // A formula with no computed value is not in the value range: the formula range widens it.
        let formulas = workbook.worksheet_formula(&name).ok();
        let (total_rows, total_columns) = range_extent(&range);
        let (formula_rows, formula_columns) = formulas.as_ref().map_or((0, 0), range_extent);
        let total_rows = total_rows.max(formula_rows);
        let total_columns = total_columns.max(formula_columns);
        let shown_rows = total_rows.min(limits.max_rows);
        let shown_columns = total_columns.min(limits.max_columns);

        let mut rows = vec![vec![String::new(); shown_columns]; shown_rows];
        place_cells(&range, &mut rows, display_cell);
        if let Some(formulas) = &formulas {
            place_cells(formulas, &mut rows, |formula| {
                if formula.is_empty() {
                    String::new()
                } else {
                    format!("={formula}")
                }
            });
        }

        sheets.push(Sheet {
            name,
            rows,
            total_rows,
            total_columns,
            layout: SheetLayout::default(),
        });
    }

    Ok(sheets)
}

/// Rows and columns from A1 to the last used cell; calamine's range starts at the first used one.
fn range_extent<T: calamine::CellType>(range: &calamine::Range<T>) -> (usize, usize) {
    if range.is_empty() {
        return (0, 0);
    }
    let (start_row, start_column) = range
        .start()
        .map_or((0, 0), |(row, column)| (row as usize, column as usize));
    (start_row + range.height(), start_column + range.width())
}

/// Write the displayed text of each cell into the table at its A1 position; a cell the table
/// already holds text for keeps it, so a formula only fills a cell whose value is missing.
fn place_cells<T: calamine::CellType>(
    range: &calamine::Range<T>,
    rows: &mut [Vec<String>],
    display: impl Fn(&T) -> String,
) {
    let (start_row, start_column) = range
        .start()
        .map_or((0, 0), |(row, column)| (row as usize, column as usize));
    for (row_offset, source_row) in range.rows().enumerate() {
        let Some(row) = rows.get_mut(start_row + row_offset) else {
            break;
        };
        for (column_offset, cell) in source_row.iter().enumerate() {
            if let Some(target) = row.get_mut(start_column + column_offset)
                && target.is_empty()
            {
                *target = display(cell);
            }
        }
    }
}

/// The text a cell shows; an error shows its Excel code, an empty cell nothing.
fn display_cell(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::Error(error) => error.to_string(),
        other => other.to_string(),
    }
}

/// A delimited file: UTF-8 when valid, otherwise Windows-1252 as Excel writes it. A comma file
/// whose first line holds more semicolons than commas is read with semicolons, the Italian export.
fn read_delimited(bytes: &[u8], separator: u8, limits: &Limits) -> Sheet {
    let text = decode_text(bytes);
    let separator = if separator == b',' {
        guess_comma_or_semicolon(&text)
    } else {
        separator as char
    };

    let mut total_rows = 0;
    let mut total_columns = 0;
    let mut rows = Vec::new();
    for line in text.lines() {
        let cells: Vec<String> = split_delimited_line(line, separator);
        total_rows += 1;
        total_columns = total_columns.max(cells.len());
        if rows.len() < limits.max_rows {
            rows.push(
                cells
                    .into_iter()
                    .take(limits.max_columns)
                    .collect::<Vec<_>>(),
            );
        }
    }
    let shown_columns = total_columns.min(limits.max_columns);
    for row in &mut rows {
        row.resize(shown_columns, String::new());
    }

    Sheet {
        name: String::new(),
        rows,
        total_rows,
        total_columns,
        layout: SheetLayout::default(),
    }
}

fn decode_text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => encoding_rs::WINDOWS_1252.decode(bytes).0.into_owned(),
    }
}

fn guess_comma_or_semicolon(text: &str) -> char {
    let first_line = text.lines().next().unwrap_or_default();
    let semicolons = first_line.matches(';').count();
    let commas = first_line.matches(',').count();
    if semicolons > commas { ';' } else { ',' }
}

/// One line split on the separator, honouring double quotes and doubled quotes inside them.
fn split_delimited_line(line: &str, separator: char) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut in_quotes = false;
    let mut characters = line.chars().peekable();

    while let Some(character) = characters.next() {
        match character {
            '"' if in_quotes && characters.peek() == Some(&'"') => {
                cell.push('"');
                characters.next();
            }
            '"' => in_quotes = !in_quotes,
            current if current == separator && !in_quotes => cells.push(std::mem::take(&mut cell)),
            current => cell.push(current),
        }
    }
    cells.push(cell);

    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{build_ods, build_xlsx, build_xlsx_with_uncached_formula};

    #[test]
    fn reads_every_sheet_of_an_xlsx_from_a1() {
        let workbook =
            read_workbook(&build_xlsx(), Some("vendite.xlsx"), &Limits::default()).unwrap();
        assert_eq!(workbook.format, DocumentFormat::Xlsx);
        assert_eq!(workbook.sheets.len(), 2);
        assert_eq!(workbook.sheets[0].name, "Vendite");
        assert_eq!(workbook.sheets[0].rows[0], vec!["Mese", "Ricavi"]);
        assert_eq!(workbook.sheets[0].rows[1], vec!["Gennaio", "1200.5"]);
        assert_eq!(workbook.sheets[1].rows[1], vec!["Acme", "Milano"]);
    }

    #[test]
    fn cuts_rows_and_columns_but_counts_them_all() {
        let limits = Limits {
            max_rows: 1,
            max_columns: 1,
            ..Limits::default()
        };
        let workbook = read_workbook(&build_xlsx(), None, &limits).unwrap();
        let sheet = &workbook.sheets[0];
        assert_eq!(sheet.rows, vec![vec!["Mese".to_owned()]]);
        assert_eq!((sheet.total_rows, sheet.total_columns), (3, 2));
    }

    #[test]
    fn shows_a_formula_that_has_no_computed_value() {
        let bytes = build_xlsx_with_uncached_formula();
        let read = read_workbook(&bytes, None, &Limits::default()).unwrap();
        assert_eq!(read.sheets[0].rows[0], vec!["2", "3", "=A1+B1"]);
    }

    #[test]
    fn reads_how_each_sheet_of_an_xlsx_is_printed() {
        let mut workbook = rust_xlsxwriter::Workbook::new();
        let sheet = workbook.add_worksheet().set_name("Vendite").unwrap();
        sheet.write(0, 0, "Mese").unwrap();
        sheet.set_column_width(1, 20).unwrap();
        sheet.set_landscape();
        sheet.set_header("&CVendite");
        workbook.add_worksheet().set_name("Altro").unwrap();
        let bytes = workbook.save_to_buffer().unwrap();
        let read = read_workbook(&bytes, None, &Limits::default()).unwrap();
        let layout = &read.sheets[0].layout;
        assert!(layout.landscape);
        assert_eq!(layout.header.as_deref(), Some("&CVendite"));
        assert_eq!(
            layout.column_widths.get(1).copied().flatten(),
            Some(20 * 256 + 182)
        );
        assert_eq!(layout.font_family.as_deref(), Some("Calibri"));
        assert_eq!(layout.font_size_tenths, Some(110));
        assert!(!read.sheets[1].layout.landscape);
        assert_eq!(read_xlsx_layouts(b"not a zip", &Limits::default()).len(), 0);
    }

    #[test]
    fn reads_an_ods() {
        let workbook = read_workbook(&build_ods(), None, &Limits::default()).unwrap();
        assert_eq!(workbook.format, DocumentFormat::Ods);
        assert_eq!(workbook.sheets[0].rows[0][0], "Cliente");
    }

    #[test]
    fn reads_an_italian_csv_with_semicolons_and_quotes() {
        let csv = "Nome;Città;Note\nMario;Milano;\"dice \"\"ciao\"\"; poi va\"\n".as_bytes();
        let sheet = &read_workbook(csv, Some("clienti.csv"), &Limits::default())
            .unwrap()
            .sheets[0];
        assert_eq!(sheet.rows[0], vec!["Nome", "Città", "Note"]);
        assert_eq!(
            sheet.rows[1],
            vec!["Mario", "Milano", "dice \"ciao\"; poi va"]
        );
    }

    #[test]
    fn reads_a_csv_written_in_windows_1252() {
        let csv = b"Citt\xe0,Importo\nArona,10\n";
        let sheet = &read_workbook(csv, Some("a.csv"), &Limits::default())
            .unwrap()
            .sheets[0];
        assert_eq!(sheet.rows[0], vec!["Città", "Importo"]);
    }

    #[test]
    fn pads_short_rows_to_the_widest_one() {
        let sheet = &read_workbook(b"a\tb\tc\nd\n", Some("x.tsv"), &Limits::default())
            .unwrap()
            .sheets[0];
        assert_eq!(sheet.rows[1], vec!["d", "", ""]);
    }

    #[test]
    fn refuses_a_document_that_is_not_a_spreadsheet() {
        let error = read_workbook(b"plain text", Some("a.txt"), &Limits::default()).unwrap_err();
        assert!(matches!(
            error,
            OfficeError::UnsupportedFormat(DocumentFormat::Unknown)
        ));
    }

    #[test]
    fn refuses_an_input_above_the_limit() {
        let limits = Limits {
            max_input_bytes: 4,
            ..Limits::default()
        };
        let error = read_workbook(b"a,b,c", Some("a.csv"), &limits).unwrap_err();
        assert!(matches!(error, OfficeError::TooLarge { .. }));
    }
}
