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

use rely_office_core::{Limits, OfficeError, SheetDrawing};
use rely_office_sheet::{
    Border, BorderPlacement, CellLook, CellRange, CellRef, ChartPlacement, ChartSpec, ChartType,
    ConditionalFormat, DataValidation, EditError, EditHistory, Input, Locale, MAX_COLUMNS,
    MAX_ROWS, NumberFormat, Region, SortKey, StylePatch, Value, Workbook, default_separator,
    function_names, print_formula_in, read_spreadsheet, write_csv, write_ods, write_xlsx,
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
            EditError::InvalidStyle => "INVALID_STYLE",
            EditError::InvalidName => "INVALID_NAME",
            EditError::InvalidChart => "INVALID_CHART",
            EditError::InvalidComment => "INVALID_COMMENT",
            EditError::InvalidPivot => "INVALID_PIVOT",
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
    /// Row index to height in points, 0 for a hidden row.
    row_heights: Vec<(u32, f64)>,
    /// How many pivot tables the sheet carries from its file, written back unchanged.
    pivot_tables: usize,
    frozen_rows: u32,
    frozen_columns: u32,
    auto_filter: Option<FilterSummary<'a>>,
    /// How many pictures and charts [`SheetDocument::pictures_summary`] lists.
    pictures: usize,
}

/// A name the workbook defines, its formula written as the document's locale writes it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DefinedNameSummary<'a> {
    name: &'a str,
    /// The sheet the name belongs to; absent for the whole workbook.
    sheet: Option<usize>,
    formula: String,
    hidden: bool,
}

/// Where a chart sits, in EMU: the cell of its corner, how far into it, and its size.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlacementJson {
    row: u32,
    column: u32,
    #[serde(default)]
    offset_x: i64,
    #[serde(default)]
    offset_y: i64,
    width: i64,
    height: i64,
}

impl From<PlacementJson> for ChartPlacement {
    fn from(placement: PlacementJson) -> Self {
        Self {
            row: placement.row,
            column: placement.column,
            offset_emu: (placement.offset_x, placement.offset_y),
            width_emu: placement.width,
            height_emu: placement.height,
        }
    }
}

/// A chart of a sheet: what it shows, absent for one the editor cannot change, and where.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChartSummary {
    index: usize,
    spec: Option<ChartSpec>,
    placement: PlacementJson,
}

fn parse_json<T: for<'de> Deserialize<'de>>(json: &str, what: &str) -> Result<T, Failure> {
    serde_json::from_str(json)
        .map_err(|error| Failure::new("INVALID_INPUT", format!("invalid {what}: {error}")))
}

/// The comment on a cell, with the replies of a thread.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommentSummary<'a> {
    row: u32,
    column: u32,
    author: &'a str,
    text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    date: Option<&'a str>,
    threaded: bool,
    resolved: bool,
    replies: Vec<ReplySummary<'a>>,
}

/// A reply of a comment thread.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReplySummary<'a> {
    author: &'a str,
    text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    date: Option<&'a str>,
}

/// The filter buttons of a sheet.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FilterSummary<'a> {
    top: u32,
    left: u32,
    bottom: u32,
    right: u32,
    /// The filtered columns, counted from `left`, and the keys of the values each shows.
    columns: Vec<FilterColumnSummary<'a>>,
    hidden_rows: Vec<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FilterColumnSummary<'a> {
    column: u32,
    shown: Vec<&'a str>,
}

#[derive(Serialize)]
struct FilterChoice {
    key: String,
    label: String,
}

/// A picture or a chart of the sheet: the cell its corner is anchored in and its size.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PictureSummary<'a> {
    row: u32,
    column: u32,
    offset_x_emu: i64,
    offset_y_emu: i64,
    width_emu: i64,
    height_emu: i64,
    content_type: &'a str,
    description: Option<&'a str>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SortKeyInput {
    column: u32,
    #[serde(default)]
    descending: bool,
}

fn invalid_input(error: impl std::fmt::Display) -> Failure {
    Failure::new("INVALID_INPUT", error.to_string())
}

/// The look the conditional formats give one cell.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LookOutput<'a> {
    row: u32,
    column: u32,
    #[serde(flatten)]
    look: &'a CellLook,
}

/// The validation of one cell, whether its value passes and the items a list offers.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidationOutput<'a> {
    validation: Option<&'a DataValidation>,
    valid: bool,
    items: Vec<String>,
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
    /// The index of the cell's look in `stylesJson`, 0 for the default.
    style: u32,
}

/// A number format as JavaScript names it; `code` is an Excel format code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
enum Format {
    General,
    Date,
    DateTime,
    Percent,
    Decimals { decimals: u8 },
    Code { code: String },
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

/// An edited workbook.
#[wasm_bindgen]
#[derive(Debug)]
pub struct SheetDocument {
    workbook: Workbook,
    locale: Locale,
    history: EditHistory,
    /// False in a collaborative editor, which undoes with regions instead of snapshots.
    history_enabled: bool,
    /// The workbook as a collaborative editor last agreed it with the room.
    checkpoint: Option<Workbook>,
    /// The pictures last drawn, by sheet and revision: a chart is drawn once per change.
    pictures: std::cell::RefCell<Option<(usize, u64, Vec<SheetDrawing>)>>,
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
            history: EditHistory::new(),
            history_enabled: true,
            checkpoint: None,
            pictures: std::cell::RefCell::new(None),
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
            history: EditHistory::new(),
            history_enabled: true,
            checkpoint: None,
            pictures: std::cell::RefCell::new(None),
        })
    }

    /// Run an edit as one step of the history; a refused edit leaves no step.
    fn recorded<T>(
        &mut self,
        edit: impl FnOnce(&mut Self) -> Result<T, JsError>,
    ) -> Result<T, JsError> {
        if !self.history_enabled {
            return edit(self);
        }
        self.history.record(&self.workbook);
        let result = edit(self);
        if result.is_err() {
            self.history.discard_last();
        }
        result
    }

    /// A rectangle of a sheet as JSON, to give back to `restore_region_json`.
    ///
    /// # Errors
    ///
    /// The workbook's error for the sheet or the rectangle.
    pub fn region_summary(
        &self,
        sheet: usize,
        top: u32,
        left: u32,
        bottom: u32,
        right: u32,
    ) -> Result<String, Failure> {
        let region = self.workbook.region(
            sheet,
            (CellRef::new(top, left), CellRef::new(bottom, right)),
        )?;
        serde_json::to_string(&region).map_err(|error| Failure::new("INTERNAL", error.to_string()))
    }

    /// Put back a rectangle `region_summary` gave.
    ///
    /// # Errors
    ///
    /// `INVALID_INPUT` for JSON that is not a region, else the workbook's error.
    pub fn restore_region_json(&mut self, sheet: usize, region_json: &str) -> Result<(), Failure> {
        let region: Region = serde_json::from_str(region_json)
            .map_err(|error| Failure::new("INVALID_INPUT", error.to_string()))?;
        self.workbook.restore_region(sheet, &region)?;
        Ok(())
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
                    row_heights: sheet
                        .row_heights
                        .iter()
                        .map(|(row, height)| (*row, *height))
                        .collect(),
                    pivot_tables: sheet.pivot_count(),
                    frozen_rows: sheet.frozen_rows,
                    frozen_columns: sheet.frozen_columns,
                    auto_filter: sheet.auto_filter().map(|filter| FilterSummary {
                        top: filter.range.first_row,
                        left: filter.range.first_column,
                        bottom: filter.range.last_row,
                        right: filter.range.last_column,
                        columns: filter
                            .filtered_columns()
                            .map(|column| FilterColumnSummary {
                                column,
                                shown: filter
                                    .shown(column)
                                    .map(|shown| shown.iter().map(String::as_str).collect())
                                    .unwrap_or_default(),
                            })
                            .collect(),
                        hidden_rows: filter.hidden_rows().collect(),
                    }),
                    pictures: sheet.pictures().len(),
                }
            })
            .collect();
        to_json(&sheets)
    }

    /// Sort the rows of a rectangle by `[{ column, descending }]`, columns of the sheet.
    ///
    /// # Errors
    ///
    /// `INVALID_INPUT` for JSON that is not a list of keys, or the error of the edit.
    #[expect(
        clippy::too_many_arguments,
        reason = "a rectangle, the keys and the heading flag, as flat values for JavaScript"
    )]
    pub fn sort_json(
        &mut self,
        sheet: usize,
        top: u32,
        left: u32,
        bottom: u32,
        right: u32,
        keys_json: &str,
        has_header: bool,
    ) -> Result<(), Failure> {
        let keys: Vec<SortKeyInput> = serde_json::from_str(keys_json).map_err(invalid_input)?;
        let keys: Vec<SortKey> = keys
            .into_iter()
            .map(|key| SortKey {
                column: key.column,
                descending: key.descending,
            })
            .collect();
        self.workbook
            .sort_range(
                sheet,
                (CellRef::new(top, left), CellRef::new(bottom, right)),
                &keys,
                has_header,
            )
            .map_err(Into::into)
    }

    /// Choose the values a filter column shows, a JSON list of keys or `null` for all.
    ///
    /// # Errors
    ///
    /// `INVALID_INPUT` for JSON that is neither, or the error of the edit.
    pub fn filter_column_json(
        &mut self,
        sheet: usize,
        column: u32,
        shown_json: &str,
    ) -> Result<(), Failure> {
        let shown: Option<Vec<String>> = serde_json::from_str(shown_json).map_err(invalid_input)?;
        self.workbook
            .set_filter_column(sheet, column, shown)
            .map_err(Into::into)
    }

    /// `[{ key, label }]`, the values a filter column can show.
    ///
    /// # Errors
    ///
    /// The error of the workbook for a sheet without a filter or a column outside it.
    pub fn filter_choices_summary(&self, sheet: usize, column: u32) -> Result<String, Failure> {
        let choices: Vec<FilterChoice> = self
            .workbook
            .filter_choices(sheet, column, self.locale)?
            .into_iter()
            .map(|(key, label)| FilterChoice { key, label })
            .collect();
        to_json(&choices)
    }

    /// The pictures and charts of a sheet, the charts drawn from the current values, kept until
    /// the workbook changes.
    fn current_pictures(&self, sheet: usize) -> Vec<SheetDrawing> {
        let revision = self.workbook.revision();
        let mut cached = self.pictures.borrow_mut();
        if let Some((known_sheet, known_revision, pictures)) = cached.as_ref()
            && *known_sheet == sheet
            && *known_revision == revision
        {
            return pictures.clone();
        }
        let pictures = self.workbook.pictures(sheet);
        *cached = Some((sheet, revision, pictures.clone()));
        pictures
    }

    /// The pictures and charts of a sheet, as JSON, in the order `picture_bytes` takes them.
    ///
    /// # Errors
    ///
    /// `NO_SUCH_SHEET`.
    pub fn pictures_summary(&self, sheet: usize) -> Result<String, Failure> {
        if sheet >= self.workbook.sheets().len() {
            return Err(EditError::NoSuchSheet(sheet).into());
        }
        let drawn = self.current_pictures(sheet);
        let pictures: Vec<PictureSummary<'_>> = drawn
            .iter()
            .map(|picture| PictureSummary {
                row: picture.row,
                column: picture.column,
                offset_x_emu: picture.offset_emu.0,
                offset_y_emu: picture.offset_emu.1,
                width_emu: picture.width_emu,
                height_emu: picture.height_emu,
                content_type: picture.content_type,
                description: picture.description.as_deref(),
            })
            .collect();
        to_json(&pictures)
    }

    /// The cells with content or a look of their own in a window, row by row.
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
        let inside = |position: CellRef| {
            (top..=bottom).contains(&position.row) && (left..=right).contains(&position.column)
        };
        let mut positions: std::collections::BTreeSet<CellRef> = content
            .cells()
            .map(|(position, _)| position)
            .filter(|position| inside(*position))
            .collect();
        positions.extend(
            content
                .styled_cells()
                .map(|(position, _)| position)
                .filter(|position| inside(*position)),
        );
        // The cells an array spills into show its values though nothing is typed there.
        for (_, spill) in self.workbook.spills(sheet) {
            for row in spill.first_row.max(top)..=spill.last_row.min(bottom) {
                for column in spill.first_column.max(left)..=spill.last_column.min(right) {
                    positions.insert(CellRef::new(row, column));
                }
            }
        }
        let mut cells = Vec::with_capacity(positions.len());
        for position in positions {
            let value = self.workbook.value(sheet, position);
            let style = self.workbook.style_id(sheet, position);
            let format = content
                .cell(position)
                .map_or(NumberFormat::General, |cell| cell.format);
            let format = match (
                format,
                self.workbook.style(sheet, position).number_format.as_ref(),
            ) {
                (NumberFormat::General, Some(code)) => Format::Code { code: code.clone() },
                (format, _) => format.into(),
            };
            cells.push(CellOutput {
                row: position.row,
                column: position.column,
                input: content
                    .cell(position)
                    .map(|cell| cell.input.text_in(self.locale))
                    .unwrap_or_default(),
                display: self.workbook.display(sheet, position, self.locale),
                kind: match value {
                    Value::Empty => "empty",
                    Value::Number(_) => "number",
                    Value::Text(_) => "text",
                    Value::Bool(_) => "bool",
                    Value::Error(_) => "error",
                },
                format,
                style,
            });
        }
        to_json(&cells)
    }

    /// Change the look of a rectangle with a patch: `{ bold, italic, underline, strike,
    /// fontFamily, fontSize, fontColour, fill, horizontal, vertical, wrap, numberFormat }`.
    ///
    /// # Errors
    ///
    /// `INVALID_INPUT` for JSON that is not a patch, or the error of the edit.
    pub fn set_style_json(
        &mut self,
        sheet: usize,
        range: (CellRef, CellRef),
        patch_json: &str,
    ) -> Result<(), Failure> {
        let patch: StylePatch = serde_json::from_str(patch_json)
            .map_err(|error| Failure::new("INVALID_INPUT", format!("invalid style: {error}")))?;
        Ok(self.workbook.set_style(sheet, range, &patch)?)
    }

    /// Put a border on edges of a rectangle: `placement` is `all`, `outline`, `inside`, `top`,
    /// `bottom`, `left` or `right`, `border_json` `{ style, widthEighths, colour }` or `null`.
    ///
    /// # Errors
    ///
    /// `INVALID_INPUT` for an unknown placement or border, or the error of the edit.
    pub fn set_borders_json(
        &mut self,
        sheet: usize,
        range: (CellRef, CellRef),
        placement: &str,
        border_json: &str,
    ) -> Result<(), Failure> {
        let placement: BorderPlacement = serde_json::from_value(serde_json::Value::String(
            placement.to_owned(),
        ))
        .map_err(|error| Failure::new("INVALID_INPUT", format!("invalid placement: {error}")))?;
        let border: Option<Border> = serde_json::from_str(border_json)
            .map_err(|error| Failure::new("INVALID_INPUT", format!("invalid border: {error}")))?;
        if let Some(border) = &border {
            let valid_colour = border.colour.as_deref().is_none_or(|colour| {
                colour.len() == 6
                    && colour
                        .chars()
                        .all(|character| character.is_ascii_hexdigit())
            });
            if !valid_colour || !(1..=96).contains(&border.width_eighths) {
                return Err(EditError::InvalidStyle.into());
            }
        }
        Ok(self
            .workbook
            .set_borders(sheet, range, placement, border.as_ref())?)
    }

    /// The looks of the workbook, which `cellsJson` names by index; the first is the default.
    ///
    /// # Errors
    ///
    /// `INTERNAL` when the JSON cannot be written.
    pub fn styles_summary(&self) -> Result<String, Failure> {
        to_json(&self.workbook.styles())
    }

    /// The merged ranges of a sheet.
    ///
    /// # Errors
    ///
    /// `NO_SUCH_SHEET`, `INTERNAL` when the JSON cannot be written.
    pub fn merges_summary(&self, sheet: usize) -> Result<String, Failure> {
        if sheet >= self.workbook.sheets().len() {
            return Err(EditError::NoSuchSheet(sheet).into());
        }
        to_json(&self.workbook.merges(sheet))
    }

    /// The names the workbook defines: `[{ name, sheet?, formula, hidden }]`, each formula with
    /// its `=` in the document's locale.
    ///
    /// # Errors
    ///
    /// `INTERNAL` when the JSON cannot be written.
    pub fn defined_names_summary(&self) -> Result<String, Failure> {
        let names: Vec<DefinedNameSummary<'_>> = self
            .workbook
            .defined_names()
            .iter()
            .map(|defined| DefinedNameSummary {
                name: &defined.name,
                sheet: defined.sheet,
                formula: format!("={}", print_formula_in(&defined.expression, self.locale)),
                hidden: defined.hidden,
            })
            .collect();
        to_json(&names)
    }

    /// The comments of a sheet's cells: `[{ row, column, author, text, date?, threaded,
    /// resolved, replies: [{ author, text, date? }] }]`, row by row.
    ///
    /// # Errors
    ///
    /// `NO_SUCH_SHEET`, `INTERNAL` when the JSON cannot be written.
    pub fn comments_summary(&self, sheet: usize) -> Result<String, Failure> {
        let content = self
            .workbook
            .sheets()
            .get(sheet)
            .ok_or(EditError::NoSuchSheet(sheet))?;
        let comments: Vec<CommentSummary<'_>> = content
            .comments()
            .iter()
            .map(|(cell, comment)| CommentSummary {
                row: cell.row,
                column: cell.column,
                author: &comment.author,
                text: &comment.text,
                date: comment.date.as_deref(),
                threaded: comment.threaded,
                resolved: comment.resolved,
                replies: comment
                    .replies
                    .iter()
                    .map(|reply| ReplySummary {
                        author: &reply.author,
                        text: &reply.text,
                        date: reply.date.as_deref(),
                    })
                    .collect(),
            })
            .collect();
        to_json(&comments)
    }

    /// The charts of a sheet: `[{ index, spec?, placement }]`.
    ///
    /// # Errors
    ///
    /// `NO_SUCH_SHEET`, `INTERNAL` when the JSON cannot be written.
    pub fn charts_summary(&self, sheet: usize) -> Result<String, Failure> {
        if sheet >= self.workbook.sheets().len() {
            return Err(EditError::NoSuchSheet(sheet).into());
        }
        let charts: Vec<ChartSummary> = self
            .workbook
            .charts(sheet)
            .into_iter()
            .enumerate()
            .map(|(index, chart)| ChartSummary {
                index,
                spec: chart.spec,
                placement: PlacementJson {
                    row: chart.placement.row,
                    column: chart.placement.column,
                    offset_x: chart.placement.offset_emu.0,
                    offset_y: chart.placement.offset_emu.1,
                    width: chart.placement.width_emu,
                    height: chart.placement.height_emu,
                },
            })
            .collect();
        to_json(&charts)
    }

    /// Add a chart from its JSON and its placement's; the index it gets.
    ///
    /// # Errors
    ///
    /// `INVALID_INPUT` for JSON that is no chart or placement, or the error of the edit.
    pub fn add_chart_json(
        &mut self,
        sheet: usize,
        spec_json: &str,
        placement_json: &str,
    ) -> Result<usize, Failure> {
        let spec: ChartSpec = parse_json(spec_json, "chart")?;
        let placement: PlacementJson = parse_json(placement_json, "placement")?;
        Ok(self.workbook.add_chart(sheet, &spec, placement.into())?)
    }

    /// Change a chart, what it shows or where it sits, each given as JSON or left as it is.
    ///
    /// # Errors
    ///
    /// `INVALID_INPUT` for JSON that is no chart or placement, or the error of the edit.
    pub fn set_chart_json(
        &mut self,
        sheet: usize,
        index: usize,
        spec_json: Option<&str>,
        placement_json: Option<&str>,
    ) -> Result<(), Failure> {
        let spec: Option<ChartSpec> = spec_json
            .map(|json| parse_json(json, "chart"))
            .transpose()?;
        let placement: Option<PlacementJson> = placement_json
            .map(|json| parse_json(json, "placement"))
            .transpose()?;
        Ok(self
            .workbook
            .set_chart(sheet, index, spec.as_ref(), placement.map(Into::into))?)
    }

    /// The chart a selection makes, as JSON, `null` when it holds nothing to plot.
    ///
    /// # Errors
    ///
    /// `INVALID_INPUT` for an unknown chart type, `INTERNAL` when the JSON cannot be written.
    pub fn chart_for_range_summary(
        &self,
        sheet: usize,
        (top, left, bottom, right): (u32, u32, u32, u32),
        chart_type: &str,
    ) -> Result<String, Failure> {
        let chart_type: ChartType = parse_json(&format!("\"{chart_type}\""), "chart type")?;
        let range = CellRange {
            first_row: top.min(bottom),
            first_column: left.min(right),
            last_row: top.max(bottom),
            last_column: left.max(right),
        };
        to_json(&self.workbook.chart_for_range(sheet, range, chart_type))
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
        let cell = CellRef::new(row, column);
        let simple = match format {
            Format::Code { code } => {
                let patch = StylePatch {
                    number_format: Some(code),
                    ..StylePatch::default()
                };
                self.workbook
                    .set_format(sheet, cell, NumberFormat::General)?;
                return Ok(self.workbook.set_style(sheet, (cell, cell), &patch)?);
            }
            Format::General => NumberFormat::General,
            Format::Date => NumberFormat::Date,
            Format::DateTime => NumberFormat::DateTime,
            Format::Percent => NumberFormat::Percent,
            Format::Decimals { decimals } => NumberFormat::Decimals(decimals),
        };
        Ok(self.workbook.set_format(sheet, cell, simple)?)
    }

    /// The conditional formats of a sheet.
    ///
    /// # Errors
    ///
    /// `NO_SUCH_SHEET`, `INTERNAL` when the JSON cannot be written.
    pub fn conditional_formats_summary(&self, sheet: usize) -> Result<String, Failure> {
        let content = self
            .workbook
            .sheets()
            .get(sheet)
            .ok_or(EditError::NoSuchSheet(sheet))?;
        to_json(&content.conditional_formats())
    }

    /// Replace the conditional formats of a sheet with a JSON list of them.
    ///
    /// # Errors
    ///
    /// `INVALID_INPUT` for JSON that is not such a list, or the error of the edit.
    pub fn set_conditional_formats_json(
        &mut self,
        sheet: usize,
        formats_json: &str,
    ) -> Result<(), Failure> {
        let formats: Vec<ConditionalFormat> =
            serde_json::from_str(formats_json).map_err(|error| {
                Failure::new(
                    "INVALID_INPUT",
                    format!("invalid conditional formats: {error}"),
                )
            })?;
        Ok(self.workbook.set_conditional_formats(sheet, formats)?)
    }

    /// The data validations of a sheet.
    ///
    /// # Errors
    ///
    /// `NO_SUCH_SHEET`, `INTERNAL` when the JSON cannot be written.
    pub fn validations_summary(&self, sheet: usize) -> Result<String, Failure> {
        let content = self
            .workbook
            .sheets()
            .get(sheet)
            .ok_or(EditError::NoSuchSheet(sheet))?;
        to_json(&content.validations())
    }

    /// Replace the data validations of a sheet with a JSON list of them.
    ///
    /// # Errors
    ///
    /// `INVALID_INPUT` for JSON that is not such a list, or the error of the edit.
    pub fn set_validations_json(
        &mut self,
        sheet: usize,
        validations_json: &str,
    ) -> Result<(), Failure> {
        let validations: Vec<DataValidation> =
            serde_json::from_str(validations_json).map_err(|error| {
                Failure::new("INVALID_INPUT", format!("invalid validations: {error}"))
            })?;
        Ok(self.workbook.set_validations(sheet, validations)?)
    }

    /// The look the conditional formats give each cell of a sheet they change.
    ///
    /// # Errors
    ///
    /// `NO_SUCH_SHEET`, `INTERNAL` when the JSON cannot be written.
    pub fn conditional_looks_summary(&self, sheet: usize) -> Result<String, Failure> {
        if sheet >= self.workbook.sheets().len() {
            return Err(EditError::NoSuchSheet(sheet).into());
        }
        let looks = self.workbook.conditional_looks(sheet);
        let output: Vec<LookOutput<'_>> = looks
            .iter()
            .map(|(cell, look)| LookOutput {
                row: cell.row,
                column: cell.column,
                look,
            })
            .collect();
        to_json(&output)
    }

    /// The validation of a cell: `{ validation, valid, items }`.
    ///
    /// # Errors
    ///
    /// `NO_SUCH_SHEET`, `INTERNAL` when the JSON cannot be written.
    pub fn validation_summary(&self, sheet: usize, cell: CellRef) -> Result<String, Failure> {
        if sheet >= self.workbook.sheets().len() {
            return Err(EditError::NoSuchSheet(sheet).into());
        }
        to_json(&ValidationOutput {
            validation: self.workbook.validation_at(sheet, cell),
            valid: self.workbook.is_valid(sheet, cell),
            items: self.workbook.validation_items(sheet, cell),
        })
    }

    /// The cells whose value their validation refuses, as `[[row, column], ...]`.
    ///
    /// # Errors
    ///
    /// `NO_SUCH_SHEET`, `INTERNAL` when the JSON cannot be written.
    pub fn invalid_cells_summary(&self, sheet: usize) -> Result<String, Failure> {
        if sheet >= self.workbook.sheets().len() {
            return Err(EditError::NoSuchSheet(sheet).into());
        }
        let cells: Vec<(u32, u32)> = self
            .workbook
            .invalid_cells(sheet)
            .into_iter()
            .map(|cell| (cell.row, cell.column))
            .collect();
        to_json(&cells)
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

    /// The workbook as ods: cells, styles, merges, sizes, conditional formats and validations;
    /// pictures, charts and pivot tables are kept by `save` only.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = saveOds)]
    pub fn save_ods(&self) -> Result<Vec<u8>, JsError> {
        write_ods(&self.workbook).map_err(|error| throw(&error.into()))
    }

    /// One sheet as delimited text, each cell as it shows in the document's locale. The
    /// separator is the one given, else the one of the file read, else the locale's (`;` in
    /// Italian, `,` in English).
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = saveCsv)]
    pub fn save_csv(&self, sheet: usize, separator: Option<String>) -> Result<Vec<u8>, JsError> {
        let separator = separator
            .and_then(|text| text.chars().next())
            .or_else(|| self.workbook.delimiter())
            .unwrap_or_else(|| default_separator(self.locale));
        write_csv(&self.workbook, sheet, self.locale, separator)
            .map_err(|error| throw(&error.into()))
    }

    /// The separator of the csv or tsv the document was read from; undefined for any other file.
    #[must_use]
    pub fn delimiter(&self) -> Option<String> {
        self.workbook.delimiter().map(String::from)
    }

    /// The macros of the file read, never run: `kept` for an xlsm, which `save` writes back as
    /// an xlsm, `dropped` for an xls, `none` otherwise.
    #[must_use]
    pub fn macros(&self) -> String {
        match (self.workbook.holds_macros(), self.workbook.macro_project()) {
            (_, Some(_)) => "kept",
            (true, None) => "dropped",
            (false, None) => "none",
        }
        .to_owned()
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

    /// `[{ row, column, input, display, kind, format, style }]` for the cells with content or a
    /// look of their own in the window, zero-based and inclusive.
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

    /// What was typed into a cell, `=` and all, as the document's locale writes it: in Italian
    /// `=SOMMA(A1;0,5)`; empty for an empty cell. Typed back, it gives the same cell.
    #[wasm_bindgen]
    #[must_use]
    pub fn input(&self, sheet: usize, row: u32, column: u32) -> String {
        self.workbook
            .sheets()
            .get(sheet)
            .and_then(|content| content.cell(CellRef::new(row, column)))
            .map(|cell| cell.input.text_in(self.locale))
            .unwrap_or_default()
    }

    /// Put back the workbook as it was before the last edit; false when there is none.
    #[wasm_bindgen]
    pub fn undo(&mut self) -> bool {
        self.history.undo(&mut self.workbook)
    }

    /// Put back the edit the last undo took away; false when there is none.
    #[wasm_bindgen]
    pub fn redo(&mut self) -> bool {
        self.history.redo(&mut self.workbook)
    }

    #[wasm_bindgen(js_name = canUndo)]
    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    #[wasm_bindgen(js_name = canRedo)]
    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
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
        self.recorded(|document| {
            document
                .workbook
                .set_input(sheet, CellRef::new(row, column), text, document.locale)
                .map_err(|error| throw(&error.into()))
        })
    }

    /// Type several cells in one recalculation: `[[row, column, text], ...]`.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = setInputs)]
    pub fn set_inputs(&mut self, sheet: usize, inputs_json: &str) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .set_inputs_json(sheet, inputs_json)
                .map_err(|failure| throw(&failure))
        })
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
        self.recorded(|document| {
            document
                .set_format_json(sheet, row, column, format_json)
                .map_err(|failure| throw(&failure))
        })
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
        self.recorded(|document| {
            document
                .workbook
                .set_column_width(sheet, column, (width >= 0.0).then_some(width))
                .map_err(|error| throw(&error.into()))
        })
    }

    /// Change the look of a rectangle with a JSON patch.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = setStyle)]
    pub fn set_style(
        &mut self,
        sheet: usize,
        top: u32,
        left: u32,
        bottom: u32,
        right: u32,
        patch_json: &str,
    ) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .set_style_json(
                    sheet,
                    (CellRef::new(top, left), CellRef::new(bottom, right)),
                    patch_json,
                )
                .map_err(|failure| throw(&failure))
        })
    }

    /// Put a border, as JSON or `null`, on the edges of a rectangle a placement names.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = setBorders)]
    #[expect(
        clippy::too_many_arguments,
        reason = "a rectangle as flat numbers for JavaScript, a placement and a border"
    )]
    pub fn set_borders(
        &mut self,
        sheet: usize,
        top: u32,
        left: u32,
        bottom: u32,
        right: u32,
        placement: &str,
        border_json: &str,
    ) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .set_borders_json(
                    sheet,
                    (CellRef::new(top, left), CellRef::new(bottom, right)),
                    placement,
                    border_json,
                )
                .map_err(|failure| throw(&failure))
        })
    }

    /// Give a rectangle the default look.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = clearStyles)]
    pub fn clear_styles(
        &mut self,
        sheet: usize,
        top: u32,
        left: u32,
        bottom: u32,
        right: u32,
    ) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .workbook
                .clear_styles(
                    sheet,
                    (CellRef::new(top, left), CellRef::new(bottom, right)),
                )
                .map_err(|error| throw(&error.into()))
        })
    }

    /// `[{ font, fill, borders, horizontal, vertical, wrap, numberFormat }]`: the looks
    /// `cellsJson` names by index.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = stylesJson)]
    pub fn styles_json(&self) -> Result<String, JsError> {
        self.styles_summary().map_err(|failure| throw(&failure))
    }

    /// `[{ firstRow, firstColumn, lastRow, lastColumn }]`: the merged ranges of a sheet.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = mergesJson)]
    pub fn merges_json(&self, sheet: usize) -> Result<String, JsError> {
        self.merges_summary(sheet)
            .map_err(|failure| throw(&failure))
    }

    /// Merge a rectangle into one cell, keeping the content of its top-left cell.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = mergeCells)]
    pub fn merge_cells(
        &mut self,
        sheet: usize,
        top: u32,
        left: u32,
        bottom: u32,
        right: u32,
    ) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .workbook
                .merge_cells(
                    sheet,
                    (CellRef::new(top, left), CellRef::new(bottom, right)),
                )
                .map_err(|error| throw(&error.into()))
        })
    }

    /// Split the merged ranges a rectangle overlaps.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = unmergeCells)]
    pub fn unmerge_cells(
        &mut self,
        sheet: usize,
        top: u32,
        left: u32,
        bottom: u32,
        right: u32,
    ) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .workbook
                .unmerge_cells(
                    sheet,
                    (CellRef::new(top, left), CellRef::new(bottom, right)),
                )
                .map_err(|error| throw(&error.into()))
        })
    }

    /// The names the workbook defines, as JSON.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = definedNamesJson)]
    pub fn defined_names_json(&self) -> Result<String, JsError> {
        self.defined_names_summary()
            .map_err(|failure| throw(&failure))
    }

    /// Define a name, or change what it refers to: `formula` as typed in a cell, in the
    /// document's locale; `sheet` keeps it to one sheet, absent for the whole workbook.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = setDefinedName)]
    pub fn set_defined_name(
        &mut self,
        name: &str,
        sheet: Option<u32>,
        formula: &str,
    ) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .workbook
                .set_defined_name(
                    name,
                    sheet.map(|sheet| sheet as usize),
                    formula,
                    document.locale,
                )
                .map_err(|error| throw(&error.into()))
        })
    }

    /// Remove a name; false when the workbook has none by that name there.
    #[wasm_bindgen(js_name = removeDefinedName)]
    pub fn remove_defined_name(&mut self, name: &str, sheet: Option<u32>) -> bool {
        if self.history_enabled {
            self.history.record(&self.workbook);
        }
        let removed = self
            .workbook
            .remove_defined_name(name, sheet.map(|sheet| sheet as usize));
        if !removed && self.history_enabled {
            self.history.discard_last();
        }
        removed
    }

    /// Keep the workbook as it is, for `rollback` to return to: a collaborative editor keeps
    /// what the room agreed while its own edits wait for their turn.
    #[wasm_bindgen]
    pub fn checkpoint(&mut self) {
        self.checkpoint = Some(self.workbook.clone());
    }

    /// Return to the last checkpoint, which is used up; false when there is none.
    #[wasm_bindgen]
    pub fn rollback(&mut self) -> bool {
        match self.checkpoint.take() {
            Some(workbook) => {
                self.workbook = workbook;
                true
            }
            None => false,
        }
    }

    /// Forget the last checkpoint.
    #[wasm_bindgen(js_name = discardCheckpoint)]
    pub fn discard_checkpoint(&mut self) {
        self.checkpoint = None;
    }

    /// Whether `checkpoint` holds a workbook.
    #[wasm_bindgen(js_name = hasCheckpoint)]
    #[must_use]
    pub fn has_checkpoint(&self) -> bool {
        self.checkpoint.is_some()
    }

    /// Turn the undo history on or off; off forgets it. A collaborative editor turns it off,
    /// since a snapshot would also undo what the others wrote.
    #[wasm_bindgen(js_name = setHistoryEnabled)]
    pub fn set_history_enabled(&mut self, enabled: bool) {
        self.history_enabled = enabled;
        if !enabled {
            self.history = EditHistory::new();
        }
    }

    /// The locale inputs are typed and shown in, `it` or `en`.
    #[wasm_bindgen]
    #[must_use]
    pub fn locale(&self) -> String {
        match self.locale {
            Locale::Italian => "it",
            Locale::English => "en",
        }
        .to_owned()
    }

    /// Type and show in another locale from now on: a collaborative editor applies another
    /// client's operation in the locale it was typed in.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with `INVALID_LOCALE`.
    #[wasm_bindgen(js_name = setLocale)]
    pub fn set_locale(&mut self, locale: &str) -> Result<(), JsError> {
        self.locale = parse_locale(locale).map_err(|failure| throw(&failure))?;
        Ok(())
    }

    /// A rectangle of a sheet as JSON: its cells typed in English with their looks, the merges
    /// inside it and its comments.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = regionJson)]
    pub fn region_json(
        &self,
        sheet: usize,
        top: u32,
        left: u32,
        bottom: u32,
        right: u32,
    ) -> Result<String, JsError> {
        self.region_summary(sheet, top, left, bottom, right)
            .map_err(|failure| throw(&failure))
    }

    /// Put back a rectangle `regionJson` gave: everything in it goes, then what it held returns.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = restoreRegion)]
    pub fn restore_region(&mut self, sheet: usize, region_json: &str) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .restore_region_json(sheet, region_json)
                .map_err(|failure| throw(&failure))
        })
    }

    /// The charts of a sheet, as JSON.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = chartsJson)]
    pub fn charts_json(&self, sheet: usize) -> Result<String, JsError> {
        self.charts_summary(sheet)
            .map_err(|failure| throw(&failure))
    }

    /// Add a chart; the index it gets among the sheet's charts.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = addChart)]
    pub fn add_chart(
        &mut self,
        sheet: usize,
        spec_json: &str,
        placement_json: &str,
    ) -> Result<usize, JsError> {
        self.recorded(|document| {
            document
                .add_chart_json(sheet, spec_json, placement_json)
                .map_err(|failure| throw(&failure))
        })
    }

    /// Change a chart: what it shows, where it sits, or both.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = setChart)]
    #[expect(
        clippy::needless_pass_by_value,
        reason = "wasm-bindgen hands an optional string over owned; it has no Option<&str>"
    )]
    pub fn set_chart(
        &mut self,
        sheet: usize,
        index: usize,
        spec_json: Option<String>,
        placement_json: Option<String>,
    ) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .set_chart_json(
                    sheet,
                    index,
                    spec_json.as_deref(),
                    placement_json.as_deref(),
                )
                .map_err(|failure| throw(&failure))
        })
    }

    /// Remove a chart.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = removeChart)]
    pub fn remove_chart(&mut self, sheet: usize, index: usize) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .workbook
                .remove_chart(sheet, index)
                .map_err(|error| throw(&error.into()))
        })
    }

    /// The chart a selection makes, as JSON, `null` when it holds nothing to plot.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = chartForRangeJson)]
    pub fn chart_for_range_json(
        &self,
        sheet: usize,
        top: u32,
        left: u32,
        bottom: u32,
        right: u32,
        chart_type: &str,
    ) -> Result<String, JsError> {
        self.chart_for_range_summary(sheet, (top, left, bottom, right), chart_type)
            .map_err(|failure| throw(&failure))
    }

    /// The comments of a sheet, as JSON.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = commentsJson)]
    pub fn comments_json(&self, sheet: usize) -> Result<String, JsError> {
        self.comments_summary(sheet)
            .map_err(|failure| throw(&failure))
    }

    /// Write the comment on a cell; an empty text removes it.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = setComment)]
    pub fn set_comment(
        &mut self,
        sheet: usize,
        row: u32,
        column: u32,
        author: &str,
        text: &str,
    ) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .workbook
                .set_comment(sheet, CellRef::new(row, column), author, text)
                .map_err(|error| throw(&error.into()))
        })
    }

    /// Open a thread on a cell in place of any comment it had; `date` is ISO 8601, or empty.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = startCommentThread)]
    pub fn start_comment_thread(
        &mut self,
        sheet: usize,
        row: u32,
        column: u32,
        author: &str,
        text: &str,
        date: &str,
    ) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .workbook
                .start_comment_thread(
                    sheet,
                    CellRef::new(row, column),
                    author,
                    text,
                    Some(date).filter(|date| !date.is_empty()),
                )
                .map_err(|error| throw(&error.into()))
        })
    }

    /// Answer the comment of a cell; `date` is ISO 8601, or empty.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = replyToComment)]
    pub fn reply_to_comment(
        &mut self,
        sheet: usize,
        row: u32,
        column: u32,
        author: &str,
        text: &str,
        date: &str,
    ) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .workbook
                .reply_to_comment(
                    sheet,
                    CellRef::new(row, column),
                    author,
                    text,
                    Some(date).filter(|date| !date.is_empty()),
                )
                .map_err(|error| throw(&error.into()))
        })
    }

    /// Mark the thread of a cell resolved, or open again.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = setCommentResolved)]
    pub fn set_comment_resolved(
        &mut self,
        sheet: usize,
        row: u32,
        column: u32,
        resolved: bool,
    ) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .workbook
                .set_comment_resolved(sheet, CellRef::new(row, column), resolved)
                .map_err(|error| throw(&error.into()))
        })
    }

    /// Remove a reply of the thread of a cell, by its position.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = removeCommentReply)]
    pub fn remove_comment_reply(
        &mut self,
        sheet: usize,
        row: u32,
        column: u32,
        reply: usize,
    ) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .workbook
                .remove_comment_reply(sheet, CellRef::new(row, column), reply)
                .map_err(|error| throw(&error.into()))
        })
    }

    /// Set a row's height in points, 0 hiding it; a negative height lets it fit its content.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = setRowHeight)]
    pub fn set_row_height(&mut self, sheet: usize, row: u32, height: f64) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .workbook
                .set_row_height(sheet, row, (height >= 0.0).then_some(height))
                .map_err(|error| throw(&error.into()))
        })
    }

    /// Empty a rectangle; its look stays.
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
        self.recorded(|document| {
            document
                .workbook
                .clear_range(
                    sheet,
                    (CellRef::new(top, left), CellRef::new(bottom, right)),
                )
                .map_err(|error| throw(&error.into()))
        })
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
        self.recorded(|document| {
            document
                .workbook
                .copy_range(
                    sheet,
                    (CellRef::new(top, left), CellRef::new(bottom, right)),
                    target_sheet,
                    CellRef::new(row, column),
                )
                .map_err(|error| throw(&error.into()))
        })
    }

    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = insertRows)]
    pub fn insert_rows(&mut self, sheet: usize, at: u32, count: u32) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .workbook
                .insert_rows(sheet, at, count)
                .map_err(|error| throw(&error.into()))
        })
    }

    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = deleteRows)]
    pub fn delete_rows(&mut self, sheet: usize, at: u32, count: u32) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .workbook
                .delete_rows(sheet, at, count)
                .map_err(|error| throw(&error.into()))
        })
    }

    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = insertColumns)]
    pub fn insert_columns(&mut self, sheet: usize, at: u32, count: u32) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .workbook
                .insert_columns(sheet, at, count)
                .map_err(|error| throw(&error.into()))
        })
    }

    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = deleteColumns)]
    pub fn delete_columns(&mut self, sheet: usize, at: u32, count: u32) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .workbook
                .delete_columns(sheet, at, count)
                .map_err(|error| throw(&error.into()))
        })
    }

    /// Add a sheet at the end and return its index.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = addSheet)]
    pub fn add_sheet(&mut self, name: &str) -> Result<usize, JsError> {
        self.recorded(|document| {
            document
                .workbook
                .add_sheet(name)
                .map_err(|error| throw(&error.into()))
        })
    }

    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = renameSheet)]
    pub fn rename_sheet(&mut self, sheet: usize, name: &str) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .workbook
                .rename_sheet(sheet, name)
                .map_err(|error| throw(&error.into()))
        })
    }

    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = removeSheet)]
    pub fn remove_sheet(&mut self, sheet: usize) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .workbook
                .remove_sheet(sheet)
                .map_err(|error| throw(&error.into()))
        })
    }

    /// The conditional formats of a sheet, as JSON.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = conditionalFormatsJson)]
    pub fn conditional_formats_json(&self, sheet: usize) -> Result<String, JsError> {
        self.conditional_formats_summary(sheet)
            .map_err(|failure| throw(&failure))
    }

    /// Replace the conditional formats of a sheet with a JSON list of them.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = setConditionalFormats)]
    pub fn set_conditional_formats(
        &mut self,
        sheet: usize,
        formats_json: &str,
    ) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .set_conditional_formats_json(sheet, formats_json)
                .map_err(|failure| throw(&failure))
        })
    }

    /// The data validations of a sheet, as JSON.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = validationsJson)]
    pub fn validations_json(&self, sheet: usize) -> Result<String, JsError> {
        self.validations_summary(sheet)
            .map_err(|failure| throw(&failure))
    }

    /// Replace the data validations of a sheet with a JSON list of them.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = setValidations)]
    pub fn set_validations(&mut self, sheet: usize, validations_json: &str) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .set_validations_json(sheet, validations_json)
                .map_err(|failure| throw(&failure))
        })
    }

    /// The look the conditional formats give each cell they change: `[{ row, column, fill,
    /// fontColour, bold, italic, underline, strike, numberFormat, bar, icon }]`.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = conditionalLooksJson)]
    pub fn conditional_looks_json(&self, sheet: usize) -> Result<String, JsError> {
        self.conditional_looks_summary(sheet)
            .map_err(|failure| throw(&failure))
    }

    /// The validation of a cell, whether its value passes and the items of its list.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = validationJson)]
    pub fn validation_json(&self, sheet: usize, row: u32, column: u32) -> Result<String, JsError> {
        self.validation_summary(sheet, CellRef::new(row, column))
            .map_err(|failure| throw(&failure))
    }

    /// The cells whose value their validation refuses.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = invalidCellsJson)]
    pub fn invalid_cells_json(&self, sheet: usize) -> Result<String, JsError> {
        self.invalid_cells_summary(sheet)
            .map_err(|failure| throw(&failure))
    }

    /// Continue the cells of a source rectangle over a target that holds it and reaches past it
    /// on one side, as the fill handle does: numbers follow their trend, dates count days, a
    /// text ending in a number counts on, month and weekday names follow the calendar.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = fillRange)]
    #[expect(
        clippy::too_many_arguments,
        reason = "two rectangles as flat numbers for JavaScript"
    )]
    pub fn fill_range(
        &mut self,
        sheet: usize,
        top: u32,
        left: u32,
        bottom: u32,
        right: u32,
        target_top: u32,
        target_left: u32,
        target_bottom: u32,
        target_right: u32,
    ) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .workbook
                .fill_range(
                    sheet,
                    (CellRef::new(top, left), CellRef::new(bottom, right)),
                    (
                        CellRef::new(target_top, target_left),
                        CellRef::new(target_bottom, target_right),
                    ),
                )
                .map_err(|error| throw(&error.into()))
        })
    }

    /// Freeze the first rows and columns of a sheet; 0 and 0 unfreezes.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = setFrozen)]
    pub fn set_frozen(&mut self, sheet: usize, rows: u32, columns: u32) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .workbook
                .set_frozen(sheet, rows, columns)
                .map_err(|error| throw(&error.into()))
        })
    }

    /// Sort the rows of a rectangle by `[{ column, descending }]`, columns of the sheet; with
    /// `hasHeader` the first row stays where it is.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = sortRange)]
    #[expect(
        clippy::too_many_arguments,
        reason = "a rectangle, the keys and the heading flag, as flat values for JavaScript"
    )]
    pub fn sort_range(
        &mut self,
        sheet: usize,
        top: u32,
        left: u32,
        bottom: u32,
        right: u32,
        keys_json: &str,
        has_header: bool,
    ) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .sort_json(sheet, top, left, bottom, right, keys_json, has_header)
                .map_err(|failure| throw(&failure))
        })
    }

    /// Put filter buttons on the first row of a rectangle; the rows an earlier filter hid come
    /// back.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = setAutoFilter)]
    pub fn set_auto_filter(
        &mut self,
        sheet: usize,
        top: u32,
        left: u32,
        bottom: u32,
        right: u32,
    ) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .workbook
                .set_auto_filter(
                    sheet,
                    Some((CellRef::new(top, left), CellRef::new(bottom, right))),
                )
                .map_err(|error| throw(&error.into()))
        })
    }

    /// Take the filter off a sheet, showing every row it hid.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = clearAutoFilter)]
    pub fn clear_auto_filter(&mut self, sheet: usize) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .workbook
                .set_auto_filter(sheet, None)
                .map_err(|error| throw(&error.into()))
        })
    }

    /// Choose the values a filter column, counted from the filter's first, shows: a JSON list
    /// of the keys `filterChoicesJson` gives, or `null` to show them all.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = setFilterColumn)]
    pub fn set_filter_column(
        &mut self,
        sheet: usize,
        column: u32,
        shown_json: &str,
    ) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .filter_column_json(sheet, column, shown_json)
                .map_err(|failure| throw(&failure))
        })
    }

    /// `[{ key, label }]`: the values a filter column can show, in sort order, as keys and as
    /// the document's locale shows them; an empty key stands for the blank cells.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = filterChoicesJson)]
    pub fn filter_choices_json(&self, sheet: usize, column: u32) -> Result<String, JsError> {
        self.filter_choices_summary(sheet, column)
            .map_err(|failure| throw(&failure))
    }

    /// `[{ row, column, offsetXEmu, offsetYEmu, widthEmu, heightEmu, contentType, description }]`:
    /// the pictures and charts of the file, charts drawn as SVG when it was opened.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = picturesJson)]
    pub fn pictures_json(&self, sheet: usize) -> Result<String, JsError> {
        self.pictures_summary(sheet)
            .map_err(|failure| throw(&failure))
    }

    /// The bytes of a picture or a chart, in the order `picturesJson` lists them; empty past
    /// the end.
    #[wasm_bindgen(js_name = pictureBytes)]
    #[must_use]
    pub fn picture_bytes(&self, sheet: usize, index: usize) -> Vec<u8> {
        if sheet >= self.workbook.sheets().len() {
            return Vec::new();
        }
        self.current_pictures(sheet)
            .get(index)
            .map(|picture| picture.bytes.clone())
            .unwrap_or_default()
    }

    /// The names of the functions a formula can call, as the document's locale writes them,
    /// in alphabetical order, as JSON.
    #[wasm_bindgen(js_name = functionNamesJson)]
    #[must_use]
    pub fn function_names_json(&self) -> String {
        serde_json::to_string(&function_names(self.locale)).unwrap_or_else(|_| "[]".to_owned())
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

mod pivots;

#[cfg(test)]
mod tests;
