// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! The pivot tables of a document for JavaScript: listed, made, changed, refreshed and removed
//! as JSON.

use rely_office_sheet::{
    CellRange, CellRef, EditError, PivotField, PivotFunction, PivotLayout, PivotSubtotals,
    PivotTable, PivotValue, PivotValuesAxis, Value, item_label,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use super::{Failure, SheetDocument, parse_json, throw, to_json};

/// A rectangle of cells, zero-based and inclusive.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct WindowJson {
    top: u32,
    left: u32,
    bottom: u32,
    right: u32,
}

impl From<CellRange> for WindowJson {
    fn from(range: CellRange) -> Self {
        Self {
            top: range.first_row,
            left: range.first_column,
            bottom: range.last_row,
            right: range.last_column,
        }
    }
}

impl From<WindowJson> for CellRange {
    fn from(window: WindowJson) -> Self {
        Self {
            first_row: window.top.min(window.bottom),
            first_column: window.left.min(window.right),
            last_row: window.top.max(window.bottom),
            last_column: window.left.max(window.right),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CellJson {
    row: u32,
    column: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FieldJson {
    field: u32,
    #[serde(default)]
    hidden: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ValueJson {
    field: u32,
    #[serde(default = "sum")]
    function: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

fn sum() -> String {
    PivotFunction::Sum.name().to_owned()
}

/// A pivot table to make or change.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PivotInput {
    #[serde(default)]
    name: Option<String>,
    source_sheet: usize,
    source: WindowJson,
    anchor: CellJson,
    #[serde(default)]
    rows: Vec<FieldJson>,
    #[serde(default)]
    columns: Vec<FieldJson>,
    #[serde(default)]
    values: Vec<ValueJson>,
    #[serde(default)]
    filters: Vec<FieldJson>,
    #[serde(default = "shown")]
    row_grand_totals: bool,
    #[serde(default = "shown")]
    column_grand_totals: bool,
    /// `compact`, `outline` or `tabular`; compact when left out.
    #[serde(default)]
    layout: Option<String>,
    /// `top`, `bottom` or `none`; top when left out.
    #[serde(default)]
    subtotals: Option<String>,
    #[serde(default)]
    repeat_labels: bool,
    /// `columns` or `rows`, the axis several values lay along; columns when left out.
    #[serde(default)]
    values_axis: Option<String>,
    /// Where `Σ Values` sits among the fields of its axis; after the last when left out.
    #[serde(default)]
    values_position: Option<usize>,
}

fn shown() -> bool {
    true
}

/// A pivot table of a sheet, with the names of its source's fields.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PivotSummary {
    index: usize,
    name: String,
    source_sheet: Option<usize>,
    source_sheet_name: String,
    source: WindowJson,
    anchor: CellJson,
    #[serde(skip_serializing_if = "Option::is_none")]
    area: Option<WindowJson>,
    rows: Vec<FieldJson>,
    columns: Vec<FieldJson>,
    values: Vec<ValueJson>,
    filters: Vec<FieldJson>,
    row_grand_totals: bool,
    column_grand_totals: bool,
    layout: &'static str,
    subtotals: &'static str,
    repeat_labels: bool,
    values_axis: &'static str,
    values_position: Option<usize>,
    fields: Vec<String>,
}

/// A field of a source: its name and the labels of its items.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceFieldSummary {
    name: String,
    items: Vec<String>,
}

fn fields_json(fields: &[PivotField]) -> Vec<FieldJson> {
    fields
        .iter()
        .map(|field| FieldJson {
            field: field.field,
            hidden: field.hidden.clone(),
        })
        .collect()
}

fn pivot_fields(fields: Vec<FieldJson>) -> Vec<PivotField> {
    fields
        .into_iter()
        .map(|field| PivotField {
            field: field.field,
            hidden: field.hidden,
        })
        .collect()
}

impl SheetDocument {
    /// The pivot tables of a sheet as JSON.
    ///
    /// # Errors
    ///
    /// `NO_SUCH_SHEET`, `INTERNAL` when the JSON cannot be written.
    pub fn pivot_tables_summary(&self, sheet: usize) -> Result<String, Failure> {
        let content = self
            .workbook
            .sheets()
            .get(sheet)
            .ok_or(EditError::NoSuchSheet(sheet))?;
        let summaries: Vec<PivotSummary> = content
            .pivot_tables()
            .iter()
            .enumerate()
            .map(|(index, table)| {
                let source_sheet = self.workbook.sheet_index(&table.source_sheet);
                let fields =
                    source_sheet
                        .map(|source| {
                            (table.source.first_column..=table.source.last_column)
                                .map(|column| {
                                    item_label(&self.workbook.value(
                                        source,
                                        CellRef::new(table.source.first_row, column),
                                    ))
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                PivotSummary {
                    index,
                    name: table.name.clone(),
                    source_sheet,
                    source_sheet_name: table.source_sheet.clone(),
                    source: table.source.into(),
                    anchor: CellJson {
                        row: table.anchor.row,
                        column: table.anchor.column,
                    },
                    area: table.area().map(WindowJson::from),
                    rows: fields_json(&table.rows),
                    columns: fields_json(&table.columns),
                    values: table
                        .values
                        .iter()
                        .map(|value| ValueJson {
                            field: value.field,
                            function: value.function.name().to_owned(),
                            name: value.name.clone(),
                        })
                        .collect(),
                    filters: fields_json(&table.filters),
                    row_grand_totals: table.row_grand_totals,
                    column_grand_totals: table.column_grand_totals,
                    layout: table.layout.name(),
                    subtotals: table.subtotals.name(),
                    repeat_labels: table.repeat_labels,
                    values_axis: table.values_axis.name(),
                    values_position: table.values_position,
                    fields,
                }
            })
            .collect();
        to_json(&summaries)
    }

    /// The fields of a range a pivot table could read, as JSON.
    ///
    /// # Errors
    ///
    /// `NO_SUCH_SHEET`, `INVALID_PIVOT` for a range no table can read.
    pub fn pivot_source_fields_summary(
        &self,
        sheet: usize,
        window: (u32, u32, u32, u32),
    ) -> Result<String, Failure> {
        let (top, left, bottom, right) = window;
        let fields = self.workbook.pivot_source_fields(
            sheet,
            WindowJson {
                top,
                left,
                bottom,
                right,
            }
            .into(),
        )?;
        let summaries: Vec<SourceFieldSummary> = fields
            .into_iter()
            .map(|field| SourceFieldSummary {
                name: field.name,
                items: field
                    .items
                    .iter()
                    .map(|item| match item {
                        Value::Empty => String::new(),
                        other => item_label(other),
                    })
                    .collect(),
            })
            .collect();
        to_json(&summaries)
    }

    /// The table a JSON input describes, its captions in the document's language.
    ///
    /// # Errors
    ///
    /// `INVALID_INPUT` for JSON that is no table or names an unknown function, `NO_SUCH_SHEET`
    /// for a source sheet the workbook lacks.
    pub fn pivot_from_json(&self, json: &str) -> Result<PivotTable, Failure> {
        let input: PivotInput = parse_json(json, "pivot table")?;
        let source_sheet = self
            .workbook
            .sheets()
            .get(input.source_sheet)
            .ok_or(EditError::NoSuchSheet(input.source_sheet))?
            .name
            .clone();
        let mut table = PivotTable::new(
            input.name.as_deref().unwrap_or(""),
            &source_sheet,
            input.source.into(),
            CellRef::new(input.anchor.row, input.anchor.column),
        );
        if table.name.is_empty() {
            table.name = format!(
                "{} {}",
                if self.locale == rely_office_sheet::Locale::Italian {
                    "Tabella pivot"
                } else {
                    "PivotTable"
                },
                self.workbook
                    .sheets()
                    .iter()
                    .map(|sheet| sheet.pivot_tables().len())
                    .sum::<usize>()
                    + 1
            );
        }
        table.locale = self.locale;
        table.rows = pivot_fields(input.rows);
        table.columns = pivot_fields(input.columns);
        table.filters = pivot_fields(input.filters);
        table.values = input
            .values
            .into_iter()
            .map(|value| {
                Ok(PivotValue {
                    field: value.field,
                    function: PivotFunction::parse(&value.function).ok_or_else(|| {
                        Failure::new(
                            "INVALID_INPUT",
                            format!("unknown pivot function {}", value.function),
                        )
                    })?,
                    name: value.name,
                })
            })
            .collect::<Result<_, Failure>>()?;
        table.row_grand_totals = input.row_grand_totals;
        table.column_grand_totals = input.column_grand_totals;
        if let Some(layout) = input.layout {
            table.layout = PivotLayout::parse(&layout).ok_or_else(|| {
                Failure::new("INVALID_INPUT", format!("unknown pivot layout {layout}"))
            })?;
        }
        if let Some(subtotals) = input.subtotals {
            table.subtotals = PivotSubtotals::parse(&subtotals).ok_or_else(|| {
                Failure::new(
                    "INVALID_INPUT",
                    format!("unknown pivot subtotals {subtotals}"),
                )
            })?;
        }
        table.repeat_labels = input.repeat_labels;
        if let Some(axis) = input.values_axis {
            table.values_axis = PivotValuesAxis::parse(&axis).ok_or_else(|| {
                Failure::new("INVALID_INPUT", format!("unknown pivot values axis {axis}"))
            })?;
        }
        table.values_position = input.values_position;
        Ok(table)
    }
}

#[wasm_bindgen]
impl SheetDocument {
    /// The pivot tables of a sheet, as JSON.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = pivotTablesJson)]
    pub fn pivot_tables_json(&self, sheet: usize) -> Result<String, JsError> {
        self.pivot_tables_summary(sheet)
            .map_err(|failure| throw(&failure))
    }

    /// The fields of a range a pivot table could read, as JSON.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = pivotSourceFieldsJson)]
    pub fn pivot_source_fields_json(
        &self,
        sheet: usize,
        top: u32,
        left: u32,
        bottom: u32,
        right: u32,
    ) -> Result<String, JsError> {
        self.pivot_source_fields_summary(sheet, (top, left, bottom, right))
            .map_err(|failure| throw(&failure))
    }

    /// Add a pivot table to a sheet and compute it; the index it gets.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = addPivotTable)]
    pub fn add_pivot_table(&mut self, sheet: usize, json: &str) -> Result<usize, JsError> {
        self.recorded(|document| {
            let table = document
                .pivot_from_json(json)
                .map_err(|failure| throw(&failure))?;
            document
                .workbook
                .add_pivot_table(sheet, table)
                .map_err(|error| throw(&error.into()))
        })
    }

    /// Change a pivot table and compute it again.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = setPivotTable)]
    pub fn set_pivot_table(
        &mut self,
        sheet: usize,
        index: usize,
        json: &str,
    ) -> Result<(), JsError> {
        self.recorded(|document| {
            let table = document
                .pivot_from_json(json)
                .map_err(|failure| throw(&failure))?;
            document
                .workbook
                .set_pivot_table(sheet, index, table)
                .map_err(|error| throw(&error.into()))
        })
    }

    /// Compute a pivot table again from its source.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = refreshPivotTable)]
    pub fn refresh_pivot_table(&mut self, sheet: usize, index: usize) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .workbook
                .refresh_pivot_table(sheet, index)
                .map_err(|error| throw(&error.into()))
        })
    }

    /// Compute every pivot table of the workbook again; how many were.
    #[wasm_bindgen(js_name = refreshPivotTables)]
    pub fn refresh_pivot_tables(&mut self) -> usize {
        self.history.record(&self.workbook);
        self.workbook.refresh_pivot_tables()
    }

    /// Remove a pivot table and its cells.
    ///
    /// # Errors
    ///
    /// An `Error` whose message starts with the failure code.
    #[wasm_bindgen(js_name = removePivotTable)]
    pub fn remove_pivot_table(&mut self, sheet: usize, index: usize) -> Result<(), JsError> {
        self.recorded(|document| {
            document
                .workbook
                .remove_pivot_table(sheet, index)
                .map_err(|error| throw(&error.into()))
        })
    }
}
