// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! rely-office-sheet for JavaScript, through wasm-bindgen, built both for Node (opening and
//! saving on a server) and for the browser (the sheet editor).
//!
//! A [`SheetDocument`] is the workbook; an editor asks for the window of cells it shows with
//! [`SheetDocument::cells_json`] and sends edits as calls. Results answer as JSON, a saved file
//! as bytes, and a failure throws an `Error` whose message starts with its stable code.

use rely_office_core::{Limits, OfficeError};
use rely_office_sheet::{
    CellRef, EditError, Input, Locale, MAX_COLUMNS, MAX_ROWS, NumberFormat, Value, Workbook,
    read_spreadsheet, write_xlsx,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// The most cells one window may ask for, so a request cannot allocate the whole grid.
pub const MAX_WINDOW_CELLS: u64 = 250_000;

/// Why a call failed: the stable code and the message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub code: &'static str,
    pub message: String,
}

impl Failure {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// The message the thrown `Error` carries: the code first.
    #[must_use]
    pub fn error_message(&self) -> String {
        format!("{}: {}", self.code, self.message)
    }
}

impl From<EditError> for Failure {
    fn from(error: EditError) -> Self {
        let code = match &error {
            EditError::Formula(_) => "INVALID_FORMULA",
            EditError::NoSuchSheet(_) => "NO_SUCH_SHEET",
            EditError::DuplicateSheetName(_) => "DUPLICATE_SHEET_NAME",
            EditError::InvalidSheetName => "INVALID_SHEET_NAME",
            EditError::LastSheet => "LAST_SHEET",
            EditError::OutOfBounds => "OUT_OF_BOUNDS",
        };
        Self::new(code, error.to_string())
    }
}

impl From<OfficeError> for Failure {
    fn from(error: OfficeError) -> Self {
        Self::new(error.code(), error.to_string())
    }
}

fn throw(failure: &Failure) -> JsError {
    JsError::new(&failure.error_message())
}

fn parse_locale(locale: &str) -> Result<Locale, Failure> {
    match locale {
        "it" | "it-IT" => Ok(Locale::Italian),
        "en" | "en-US" | "en-GB" => Ok(Locale::English),
        other => Err(Failure::new(
            "INVALID_LOCALE",
            format!("unknown locale {other}"),
        )),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SheetSummary<'a> {
    name: &'a str,
    /// The used area from A1: one past the last row and column with content.
    rows: u32,
    columns: u32,
    /// Column index to width in characters.
    column_widths: Vec<(u32, f64)>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CellOutput {
    row: u32,
    column: u32,
    /// What was typed, as the formula bar shows it.
    input: String,
    /// What the cell shows.
    display: String,
    /// `number`, `text`, `bool`, `error` or `empty`: what the value is, for alignment.
    kind: &'static str,
    format: Format,
}

/// A number format as JavaScript names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
enum Format {
    General,
    Date,
    DateTime,
    Percent,
    Decimals { decimals: u8 },
}

impl From<NumberFormat> for Format {
    fn from(format: NumberFormat) -> Self {
        match format {
            NumberFormat::General => Self::General,
            NumberFormat::Date => Self::Date,
            NumberFormat::DateTime => Self::DateTime,
            NumberFormat::Percent => Self::Percent,
            NumberFormat::Decimals(decimals) => Self::Decimals { decimals },
        }
    }
}

impl From<Format> for NumberFormat {
    fn from(format: Format) -> Self {
        match format {
            Format::General => Self::General,
            Format::Date => Self::Date,
            Format::DateTime => Self::DateTime,
            Format::Percent => Self::Percent,
            Format::Decimals { decimals } => Self::Decimals(decimals),
        }
    }
}

/// An edited workbook.
#[wasm_bindgen]
#[derive(Debug)]
pub struct SheetDocument {
    workbook: Workbook,
    locale: Locale,
}

impl SheetDocument {
    /// A new workbook with one empty sheet.
    ///
    /// # Errors
    ///
    /// `INVALID_LOCALE` for a locale other than `it` or `en`.
    pub fn create(locale: &str) -> Result<Self, Failure> {
        Ok(Self {
            workbook: Workbook::new(),
            locale: parse_locale(locale)?,
        })
    }

    /// A workbook opened from a spreadsheet file.
    ///
    /// # Errors
    ///
    /// `INVALID_LOCALE`, or the reader's error for the file.
    pub fn open_file(bytes: &[u8], file_name: Option<&str>, locale: &str) -> Result<Self, Failure> {
        let locale = parse_locale(locale)?;
        Ok(Self {
            workbook: read_spreadsheet(bytes, file_name, &Limits::default(), locale)?,
            locale,
        })
    }

    /// The sheets: name, used area and column widths.
    ///
    /// # Errors
    ///
    /// `INTERNAL` when the JSON cannot be written.
    pub fn sheets_summary(&self) -> Result<String, Failure> {
        let sheets: Vec<SheetSummary<'_>> = self
            .workbook
            .sheets()
            .iter()
            .map(|sheet| {
                let (rows, columns) = sheet.extent();
                SheetSummary {
                    name: &sheet.name,
                    rows,
                    columns,
                    column_widths: sheet
                        .column_widths
                        .iter()
                        .map(|(column, width)| (*column, *width))
                        .collect(),
                }
            })
            .collect();
        to_json(&sheets)
    }

    /// The cells with content in a window, row by row.
    ///
    /// # Errors
    ///
    /// `NO_SUCH_SHEET`, `OUT_OF_BOUNDS` for a window past the grid or larger than
    /// [`MAX_WINDOW_CELLS`].
    pub fn window(
        &self,
        sheet: usize,
        top: u32,
        left: u32,
        bottom: u32,
        right: u32,
    ) -> Result<String, Failure> {
        let Some(content) = self.workbook.sheets().get(sheet) else {
            return Err(EditError::NoSuchSheet(sheet).into());
        };
        if top > bottom || left > right || bottom >= MAX_ROWS || right >= MAX_COLUMNS {
            return Err(EditError::OutOfBounds.into());
        }
        let area = u64::from(bottom - top + 1) * u64::from(right - left + 1);
        if area > MAX_WINDOW_CELLS {
            return Err(EditError::OutOfBounds.into());
        }
        let mut cells = Vec::new();
        for (position, cell) in content.cells() {
            if position.row < top
                || position.row > bottom
                || position.column < left
                || position.column > right
            {
                continue;
            }
            let value = self.workbook.value(sheet, position);
            cells.push(CellOutput {
                row: position.row,
                column: position.column,
                input: cell.input.text(),
                display: self.workbook.display(sheet, position, self.locale),
                kind: match value {
                    Value::Empty => "empty",
                    Value::Number(_) => "number",
                    Value::Text(_) => "text",
                    Value::Bool(_) => "bool",
                    Value::Error(_) => "error",
                },
                format: cell.format.into(),
            });
        }
        to_json(&cells)
    }

    /// Type several cells at once: `[[row, column, text], ...]`.
    ///
    /// # Errors
    ///
    /// `INVALID_INPUT` for JSON that is not such a list, or the error of the edit.
    pub fn set_inputs_json(&mut self, sheet: usize, inputs_json: &str) -> Result<(), Failure> {
        let inputs: Vec<(u32, u32, String)> = serde_json::from_str(inputs_json)
            .map_err(|error| Failure::new("INVALID_INPUT", format!("invalid inputs: {error}")))?;
        let inputs: Vec<(CellRef, String)> = inputs
            .into_iter()
            .map(|(row, column, text)| (CellRef::new(row, column), text))
            .collect();
        Ok(self.workbook.set_inputs(sheet, &inputs, self.locale)?)
    }

    /// Set the number format of a cell, as `{ "type": "percent" }` or
    /// `{ "type": "decimals", "decimals": 2 }`.
    ///
    /// # Errors
    ///
    /// `INVALID_INPUT` for an unknown format, or the error of the edit.
    pub fn set_format_json(
        &mut self,
        sheet: usize,
        row: u32,
        column: u32,
        format_json: &str,
    ) -> Result<(), Failure> {
        let format: Format = serde_json::from_str(format_json)
            .map_err(|error| Failure::new("INVALID_INPUT", format!("invalid format: {error}")))?;
        Ok(self
            .workbook
            .set_format(sheet, CellRef::new(row, column), format.into())?)
    }
}

fn to_json(value: &impl Serialize) -> Result<String, Failure> {
    serde_json::to_string(value).map_err(|error| Failure::new("INTERNAL", error.to_string()))
}

#[wasm_bindgen]
impl SheetDocument {
    /// A new workbook, `locale` `it` or `en` for typing and showing numbers.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(constructor)]
    pub fn new(locale: &str) -> Result<Self, JsError> {
        Self::create(locale).map_err(|failure| throw(&failure))
    }

    /// Open a spreadsheet file (xlsx, xls, ods, csv, tsv).
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen]
    #[expect(
        clippy::needless_pass_by_value,
        reason = "wasm-bindgen hands an optional string over owned; it has no Option<&str>"
    )]
    pub fn open(bytes: &[u8], file_name: Option<String>, locale: &str) -> Result<Self, JsError> {
        Self::open_file(bytes, file_name.as_deref(), locale).map_err(|failure| throw(&failure))
    }

    /// The workbook as xlsx.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen]
    pub fn save(&self) -> Result<Vec<u8>, JsError> {
        write_xlsx(&self.workbook).map_err(|error| throw(&error.into()))
    }

    /// `[{ name, rows, columns, columnWidths: [[column, width]] }]`.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = sheetsJson)]
    pub fn sheets_json(&self) -> Result<String, JsError> {
        self.sheets_summary().map_err(|failure| throw(&failure))
    }

    /// `[{ row, column, input, display, kind, format }]` for the cells with content in the
    /// window, zero-based and inclusive.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = cellsJson)]
    pub fn cells_json(
        &self,
        sheet: usize,
        top: u32,
        left: u32,
        bottom: u32,
        right: u32,
    ) -> Result<String, JsError> {
        self.window(sheet, top, left, bottom, right)
            .map_err(|failure| throw(&failure))
    }

    /// What was typed into a cell, `=` and all; empty for an empty cell.
    #[wasm_bindgen]
    #[must_use]
    pub fn input(&self, sheet: usize, row: u32, column: u32) -> String {
        self.workbook
            .sheets()
            .get(sheet)
            .and_then(|content| content.cell(CellRef::new(row, column)))
            .map(|cell| cell.input.text())
            .unwrap_or_default()
    }

    /// Type into a cell; an empty text clears it.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = setInput)]
    pub fn set_input(
        &mut self,
        sheet: usize,
        row: u32,
        column: u32,
        text: &str,
    ) -> Result<(), JsError> {
        self.workbook
            .set_input(sheet, CellRef::new(row, column), text, self.locale)
            .map_err(|error| throw(&error.into()))
    }

    /// Type several cells in one recalculation: `[[row, column, text], ...]`.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = setInputs)]
    pub fn set_inputs(&mut self, sheet: usize, inputs_json: &str) -> Result<(), JsError> {
        self.set_inputs_json(sheet, inputs_json)
            .map_err(|failure| throw(&failure))
    }

    /// Set a cell's number format, as JSON.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = setFormat)]
    pub fn set_format(
        &mut self,
        sheet: usize,
        row: u32,
        column: u32,
        format_json: &str,
    ) -> Result<(), JsError> {
        self.set_format_json(sheet, row, column, format_json)
            .map_err(|failure| throw(&failure))
    }

    /// Set a column's width in characters; a negative width restores the default.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = setColumnWidth)]
    pub fn set_column_width(
        &mut self,
        sheet: usize,
        column: u32,
        width: f64,
    ) -> Result<(), JsError> {
        self.workbook
            .set_column_width(sheet, column, (width >= 0.0).then_some(width))
            .map_err(|error| throw(&error.into()))
    }

    /// Empty a rectangle.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = clearRange)]
    pub fn clear_range(
        &mut self,
        sheet: usize,
        top: u32,
        left: u32,
        bottom: u32,
        right: u32,
    ) -> Result<(), JsError> {
        self.workbook
            .clear_range(
                sheet,
                (CellRef::new(top, left), CellRef::new(bottom, right)),
            )
            .map_err(|error| throw(&error.into()))
    }

    /// Copy a rectangle so its top-left corner lands on `(row, column)` of `target_sheet`.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = copyRange)]
    #[expect(
        clippy::too_many_arguments,
        reason = "a rectangle and a target cell, as flat numbers for JavaScript"
    )]
    pub fn copy_range(
        &mut self,
        sheet: usize,
        top: u32,
        left: u32,
        bottom: u32,
        right: u32,
        target_sheet: usize,
        row: u32,
        column: u32,
    ) -> Result<(), JsError> {
        self.workbook
            .copy_range(
                sheet,
                (CellRef::new(top, left), CellRef::new(bottom, right)),
                target_sheet,
                CellRef::new(row, column),
            )
            .map_err(|error| throw(&error.into()))
    }

    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = insertRows)]
    pub fn insert_rows(&mut self, sheet: usize, at: u32, count: u32) -> Result<(), JsError> {
        self.workbook
            .insert_rows(sheet, at, count)
            .map_err(|error| throw(&error.into()))
    }

    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = deleteRows)]
    pub fn delete_rows(&mut self, sheet: usize, at: u32, count: u32) -> Result<(), JsError> {
        self.workbook
            .delete_rows(sheet, at, count)
            .map_err(|error| throw(&error.into()))
    }

    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = insertColumns)]
    pub fn insert_columns(&mut self, sheet: usize, at: u32, count: u32) -> Result<(), JsError> {
        self.workbook
            .insert_columns(sheet, at, count)
            .map_err(|error| throw(&error.into()))
    }

    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = deleteColumns)]
    pub fn delete_columns(&mut self, sheet: usize, at: u32, count: u32) -> Result<(), JsError> {
        self.workbook
            .delete_columns(sheet, at, count)
            .map_err(|error| throw(&error.into()))
    }

    /// Add a sheet at the end and return its index.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = addSheet)]
    pub fn add_sheet(&mut self, name: &str) -> Result<usize, JsError> {
        self.workbook
            .add_sheet(name)
            .map_err(|error| throw(&error.into()))
    }

    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = renameSheet)]
    pub fn rename_sheet(&mut self, sheet: usize, name: &str) -> Result<(), JsError> {
        self.workbook
            .rename_sheet(sheet, name)
            .map_err(|error| throw(&error.into()))
    }

    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = removeSheet)]
    pub fn remove_sheet(&mut self, sheet: usize) -> Result<(), JsError> {
        self.workbook
            .remove_sheet(sheet)
            .map_err(|error| throw(&error.into()))
    }

    /// Set today's date, as a serial day count, for `TODAY` and `NOW`; a negative one unsets it.
    #[wasm_bindgen(js_name = setToday)]
    pub fn set_today(&mut self, serial: f64) {
        self.workbook.set_today((serial >= 0.0).then_some(serial));
    }

    /// Read a typed value as this document's locale reads it, without changing anything:
    /// `{ kind, value }`, for an editor's preview.
    #[wasm_bindgen(js_name = previewInput)]
    #[must_use]
    pub fn preview_input(&self, text: &str) -> String {
        let kind = match Input::parse(text, self.locale) {
            Ok(None) => "empty",
            Ok(Some(Input::Number(_))) => "number",
            Ok(Some(Input::Text(_))) => "text",
            Ok(Some(Input::Bool(_))) => "bool",
            Ok(Some(Input::Formula(_))) => "formula",
            Err(_) => "invalid",
        };
        kind.to_owned()
    }
}

#[cfg(test)]
mod tests;
