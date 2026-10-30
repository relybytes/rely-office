// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use std::collections::HashMap;
use std::io::Cursor;

use calamine::{Data, Reader, open_workbook_auto_from_rs};

use crate::number_format::{NumberLocale, format_number, format_text, general_number};
use crate::opendocument::read_ods_layouts;
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
    /// The colour a cell's number format gives its value, `[Red]` for a negative amount, by
    /// row and column of `rows`, as `RRGGBB`.
    pub value_colours: HashMap<(usize, usize), &'static str>,
}

/// The sheets of a workbook, in the order the file declares them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workbook {
    pub format: DocumentFormat,
    pub sheets: Vec<Sheet>,
}

/// Read a spreadsheet (xlsx, xls, ods, csv, tsv) as tables of the values a reader sees, the
/// numbers of an xlsx shown with their cells' number formats in English notation.
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
    read_workbook_in(bytes, file_name, limits, NumberLocale::English)
}

/// [`read_workbook`] with the numbers, dates and month names of an xlsx shown in `locale`.
///
/// # Errors
///
/// As [`read_workbook`].
pub fn read_workbook_in(
    bytes: &[u8],
    file_name: Option<&str>,
    limits: &Limits,
    locale: NumberLocale,
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
            let mut layouts = read_xlsx_layouts(bytes, limits);
            read_binary_workbook(bytes, limits, &mut layouts, locale)?
        }
        DocumentFormat::Ods => {
            let mut layouts = read_ods_layouts(bytes, limits);
            read_binary_workbook(bytes, limits, &mut layouts, locale)?
        }
        DocumentFormat::Xls => read_binary_workbook(bytes, limits, &mut HashMap::new(), locale)?,
        DocumentFormat::Csv => vec![read_delimited(bytes, b',', limits)],
        DocumentFormat::Tsv => vec![read_delimited(bytes, b'\t', limits)],
        other => return Err(OfficeError::UnsupportedFormat(other)),
    };

    Ok(Workbook { format, sheets })
}

/// How each sheet of an ods is laid out, by sheet name, as an xlsx gives it: widths, heights,
/// cell styles, merges, conditional formats and validations; a package it cannot read gives
/// none.
#[must_use]
pub fn read_ods_sheet_layouts(bytes: &[u8], limits: &Limits) -> HashMap<String, SheetLayout> {
    read_ods_layouts(bytes, limits)
}

/// The sheets of a binary or zipped workbook; a sheet with a layout shows each value with its
/// cell's number format.
fn read_binary_workbook(
    bytes: &[u8],
    limits: &Limits,
    layouts: &mut HashMap<String, SheetLayout>,
    locale: NumberLocale,
) -> Result<Vec<Sheet>, OfficeError> {
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

        let layout = layouts.remove(&name).unwrap_or_default();
        let mut rows = vec![vec![String::new(); shown_columns]; shown_rows];
        let value_colours = std::cell::RefCell::new(HashMap::new());
        place_cells(&range, &mut rows, |row, column, cell| {
            let code = u32::try_from(row)
                .ok()
                .zip(u32::try_from(column).ok())
                .and_then(|(row, column)| layout.style_of(row, column))
                .and_then(|style| style.number_format.as_deref());
            let (text, colour) = display_value(cell, code, locale);
            if let Some(colour) = colour {
                value_colours.borrow_mut().insert((row, column), colour);
            }
            text
        });
        if let Some(formulas) = &formulas {
            place_cells(formulas, &mut rows, |_, _, formula| {
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
            layout,
            value_colours: value_colours.into_inner(),
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
    display: impl Fn(usize, usize, &T) -> String,
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
                *target = display(start_row + row_offset, start_column + column_offset, cell);
            }
        }
    }
}

/// The text a cell shows with its number format, numbers and dates as the code writes them and
/// text through the code's text section, and the colour the code gives it.
fn display_value(
    cell: &Data,
    code: Option<&str>,
    locale: NumberLocale,
) -> (String, Option<&'static str>) {
    let Some(code) = code else {
        let text = match cell {
            Data::Float(number) => general_number(*number, locale),
            #[allow(clippy::cast_precision_loss)]
            Data::Int(number) => general_number(*number as f64, locale),
            other => display_cell(other),
        };
        return (text, None);
    };
    let number = match cell {
        Data::Float(number) => *number,
        #[allow(clippy::cast_precision_loss)]
        Data::Int(number) => *number as f64,
        Data::DateTime(moment) => moment.as_f64(),
        Data::DateTimeIso(text) | Data::DurationIso(text) => match iso_serial(text) {
            Some(serial) => serial,
            None => return (text.clone(), None),
        },
        Data::String(text) => return (format_text(text, code), None),
        other => return (display_cell(other), None),
    };
    let formatted = format_number(number, code, locale);
    (formatted.text, formatted.colour)
}

/// An ISO 8601 date, date and time, or duration as an `OpenDocument` sheet stores it, as the
/// serial day count of Excel's 1900 system: `2026-10-27`, `2026-10-27T08:30:00`, `PT36H30M`.
fn iso_serial(text: &str) -> Option<f64> {
    let text = text.trim();
    if let Some(duration) = text.strip_prefix("PT").or_else(|| text.strip_prefix("-PT")) {
        let mut seconds = 0.0;
        let mut number = String::new();
        for character in duration.chars() {
            match character {
                'H' | 'M' | 'S' => {
                    let value: f64 = number.parse().ok()?;
                    seconds += value
                        * match character {
                            'H' => 3_600.0,
                            'M' => 60.0,
                            _ => 1.0,
                        };
                    number.clear();
                }
                digit if digit.is_ascii_digit() || digit == '.' => number.push(digit),
                _ => return None,
            }
        }
        let days = seconds / 86_400.0;
        return Some(if text.starts_with('-') { -days } else { days });
    }
    let (date, time) = text.split_once('T').unwrap_or((text, ""));
    let mut parts = date.splitn(3, '-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: i64 = parts.next()?.parse().ok()?;
    let day: i64 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || !(1..=9_999).contains(&year) {
        return None;
    }
    // Days from the civil calendar, then from Excel's day zero, 30 December 1899.
    let shifted = if month <= 2 { year - 1 } else { year };
    let era = shifted.div_euclid(400);
    let year_of_era = shifted - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468 + 25_569;
    let mut fraction = 0.0;
    if !time.is_empty() {
        let mut clock = time.trim_end_matches('Z').split(':');
        let hours: f64 = clock.next()?.parse().ok()?;
        let minutes: f64 = clock.next().unwrap_or("0").parse().ok()?;
        let seconds: f64 = clock.next().unwrap_or("0").parse().ok()?;
        fraction = (hours * 3_600.0 + minutes * 60.0 + seconds) / 86_400.0;
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "a day count of the years 1 to 9999 fits in f64"
    )]
    let days = days as f64;
    Some(days + fraction)
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
    for line in delimited_records(&text) {
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
        value_colours: HashMap::new(),
    }
}

fn decode_text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => encoding_rs::WINDOWS_1252.decode(bytes).0.into_owned(),
    }
}

/// The separator of a csv or a tsv, as its reader splits it; `None` for any other format.
#[must_use]
pub fn delimited_separator(bytes: &[u8], format: DocumentFormat) -> Option<char> {
    match format {
        DocumentFormat::Csv => Some(guess_comma_or_semicolon(&decode_text(bytes))),
        DocumentFormat::Tsv => Some('\t'),
        _ => None,
    }
}

/// The records of delimited text: its lines, a line break inside double quotes kept in its cell.
fn delimited_records(text: &str) -> impl Iterator<Item = &str> {
    let mut rest = text;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let mut in_quotes = false;
        let mut end = rest.len();
        let mut next = rest.len();
        for (index, character) in rest.char_indices() {
            match character {
                '"' => in_quotes = !in_quotes,
                '\n' if !in_quotes => {
                    end = if rest[..index].ends_with('\r') {
                        index - 1
                    } else {
                        index
                    };
                    next = index + 1;
                    break;
                }
                _ => {}
            }
        }
        let record = &rest[..end];
        rest = &rest[next..];
        Some(record)
    })
}

fn guess_comma_or_semicolon(text: &str) -> char {
    let first_record = delimited_records(text).next().unwrap_or_default();
    let (mut semicolons, mut commas, mut in_quotes) = (0, 0, false);
    for character in first_record.chars() {
        match character {
            '"' => in_quotes = !in_quotes,
            ';' if !in_quotes => semicolons += 1,
            ',' if !in_quotes => commas += 1,
            _ => {}
        }
    }
    // A tie is an Italian line whose numbers carry a decimal comma: `Rossi;12,5`.
    if semicolons > 0 && semicolons >= commas {
        ';'
    } else {
        ','
    }
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
    fn reads_iso_dates_times_and_durations_as_serial_days() {
        let close = |text: &str, expected: f64| {
            let serial = iso_serial(text).unwrap();
            assert!((serial - expected).abs() < 1e-9, "{text}: {serial}");
        };
        close("1899-12-30", 0.0);
        close("1900-03-01", 61.0);
        close("2026-10-27", 46_322.0);
        close("2026-10-27T12:00:00", 46_322.5);
        close("2026-10-27T06:00:00.000Z", 46_322.25);
        close("PT36H", 1.5);
        close("PT1H30M36S", 0.062_916_666_666_666_7);
        close("-PT12H", -0.5);
        for broken in [
            "2026-13-01",
            "2026-10",
            "PT5X",
            "yesterday",
            "2026-10-27Tnoon",
        ] {
            assert_eq!(iso_serial(broken), None, "{broken}");
        }
        let (text, _) = display_value(
            &Data::DateTimeIso("2026-10-27".to_owned()),
            Some("dd/mm/yyyy"),
            NumberLocale::default(),
        );
        assert_eq!(text, "27/10/2026");
        let (text, _) = display_value(
            &Data::DateTimeIso("sometime".to_owned()),
            Some("dd/mm/yyyy"),
            NumberLocale::default(),
        );
        assert_eq!(text, "sometime");
    }

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

    #[test]
    fn shows_the_numbers_of_an_xlsx_with_their_formats_and_reads_their_looks() {
        use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder};
        let mut workbook = rust_xlsxwriter::Workbook::new();
        let sheet = workbook.add_worksheet().set_name("Conti").unwrap();
        let euro = Format::new()
            .set_num_format("#,##0.00 \"€\"")
            .set_bold()
            .set_background_color(Color::RGB(0x00FF_FF00))
            .set_border(FormatBorder::Thin)
            .set_align(FormatAlign::Center);
        let date = Format::new().set_num_format("dd/mm/yyyy");
        let month = Format::new().set_num_format("mmmm yyyy");
        let percent = Format::new().set_num_format("0.0%");
        let text = Format::new().set_num_format("\"Rif. \"@");
        sheet.write_with_format(0, 0, 1_200.5, &euro).unwrap();
        sheet.write_with_format(0, 1, 46_322, &date).unwrap();
        sheet.write_with_format(0, 2, 46_322, &month).unwrap();
        sheet.write_with_format(0, 3, 0.125, &percent).unwrap();
        sheet.write_with_format(0, 4, "A12", &text).unwrap();
        sheet.write(0, 5, 3.5).unwrap();
        let red = Format::new().set_num_format("0;[Red]-0");
        sheet.write_with_format(0, 6, -5, &red).unwrap();
        sheet
            .merge_range(2, 0, 3, 2, "unite", &Format::new())
            .unwrap();
        sheet.set_row_height(2, 30).unwrap();
        let bytes = workbook.save_to_buffer().unwrap();

        let italian =
            read_workbook_in(&bytes, None, &Limits::default(), NumberLocale::Italian).unwrap();
        let conti = &italian.sheets[0];
        assert_eq!(
            conti.rows[0],
            vec![
                "1.200,50 €",
                "27/10/2026",
                "ottobre 2026",
                "12,5%",
                "Rif. A12",
                "3,5",
                "-5"
            ]
        );
        assert_eq!(conti.value_colours.get(&(0, 6)), Some(&"FF0000"));
        assert_eq!(conti.value_colours.len(), 1);
        let english = read_workbook(&bytes, None, &Limits::default()).unwrap();
        assert_eq!(english.sheets[0].rows[0][0], "1,200.50 €");
        assert_eq!(english.sheets[0].rows[0][2], "October 2026");

        let style = conti.layout.style_of(0, 0).unwrap();
        assert!(style.font.bold);
        assert_eq!(style.fill.as_deref(), Some("FFFF00"));
        assert!(!style.borders.is_empty());
        assert_eq!(style.horizontal, Some(crate::HorizontalAlignment::Center));
        assert_eq!(
            conti.layout.merges,
            vec![crate::CellRange {
                first_row: 2,
                first_column: 0,
                last_row: 3,
                last_column: 2
            }]
        );
        assert_eq!(conti.layout.row_heights.get(&2), Some(&600));
    }
}
