// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! The pivot tables of a workbook as an ods keeps them: `table:data-pilot-tables`, each a source
//! range, a target range and a field per placed column, named by its header.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::io::{Cursor, Read};

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use rely_office_core::{CellRange, Limits};

use super::{escape, quoted_sheet};
use crate::address::{CellRef, column_index, column_name};
use crate::model::Workbook;
use crate::pivots::{
    PivotField, PivotFunction, PivotLayout, PivotSubtotals, PivotTable, PivotValue,
    PivotValuesAxis, item_label,
};
use crate::value::Locale;

/// The data pilot tables of a workbook; empty when it has no pivot table.
#[expect(
    clippy::too_many_lines,
    reason = "every part of a data pilot table is written in the order the schema sets"
)]
pub(super) fn data_pilot_tables_xml(workbook: &Workbook) -> String {
    let mut xml = String::new();
    for sheet in &workbook.sheets {
        for table in &sheet.pivot_tables {
            let source = table.read_source(workbook).unwrap_or_default();
            let header = source.first().map(Vec::as_slice).unwrap_or_default();
            let records = source.get(1..).unwrap_or_default();
            let area = table.area.unwrap_or(CellRange {
                first_row: table.anchor.row,
                first_column: table.anchor.column,
                last_row: table.anchor.row,
                last_column: table.anchor.column,
            });
            let grand_total = match (table.row_grand_totals, table.column_grand_totals) {
                (true, true) => "both",
                (true, false) => "row",
                (false, true) => "column",
                (false, false) => "none",
            };
            let _ = write!(
                xml,
                r#"<table:data-pilot-table table:name="{}" table:target-range-address="{}" table:grand-total="{grand_total}"><table:source-cell-range table:cell-range-address="{}"/>"#,
                escape(&table.name),
                escape(&range_address(&sheet.name, area)),
                escape(&range_address(&table.source_sheet, table.source))
            );
            let name_of = |field: u32| {
                header
                    .get(field as usize)
                    .map(item_label)
                    .unwrap_or_default()
            };
            let on_rows = table.values.len() > 1 && table.values_axis == PivotValuesAxis::Rows;
            let place = |fields: usize| {
                table
                    .values_position
                    .map_or(fields, |position| position.min(fields))
            };
            let data_row = on_rows.then(|| place(table.rows.len()));
            let data_column =
                (table.values.len() > 1 && !on_rows).then(|| place(table.columns.len()));
            for (fields, orientation) in [
                (&table.rows, "row"),
                (&table.columns, "column"),
                (&table.filters, "page"),
            ] {
                for (place, field) in fields.iter().enumerate() {
                    if orientation == "row" && data_row == Some(place) {
                        xml.push_str(DATA_LAYOUT_ROW);
                    }
                    if orientation == "column" && data_column == Some(place) {
                        xml.push_str(DATA_LAYOUT_COLUMN);
                    }
                    let mut attributes = String::new();
                    if orientation == "page" {
                        let shown: Vec<String> = records
                            .iter()
                            .map(|record| {
                                record
                                    .get(field.field as usize)
                                    .map(item_label)
                                    .unwrap_or_default()
                            })
                            .filter(|label| !field.hidden.contains(label))
                            .collect::<BTreeSet<String>>()
                            .into_iter()
                            .collect();
                        if let [only] = shown.as_slice()
                            && !field.hidden.is_empty()
                        {
                            let _ =
                                write!(attributes, r#" table:selected-page="{}""#, escape(only));
                        }
                    }
                    let axis = orientation != "page";
                    let repeat = if table.repeat_labels && orientation == "row" {
                        r#" calcext:repeat-item-labels="true""#
                    } else {
                        ""
                    };
                    let _ = write!(
                        xml,
                        r#"<table:data-pilot-field table:source-field-name="{}" table:orientation="{orientation}"{attributes}><table:data-pilot-level table:show-empty="false"{repeat}>"#,
                        escape(&name_of(field.field))
                    );
                    if axis && table.subtotals != PivotSubtotals::Off {
                        xml.push_str(r#"<table:data-pilot-subtotals><table:data-pilot-subtotal table:function="auto"/></table:data-pilot-subtotals>"#);
                    }
                    if !field.hidden.is_empty() {
                        xml.push_str("<table:data-pilot-members>");
                        for hidden in &field.hidden {
                            let _ = write!(
                                xml,
                                r#"<table:data-pilot-member table:name="{}" table:display="false" table:show-details="true"/>"#,
                                escape(hidden)
                            );
                        }
                        xml.push_str("</table:data-pilot-members>");
                    }
                    if orientation == "row" {
                        // An ods has no compact form: a compact table is kept in outline form.
                        let mode = match (table.layout, table.subtotals) {
                            (PivotLayout::Tabular, _) => "tabular-layout",
                            (_, PivotSubtotals::Bottom) => "outline-subtotals-bottom",
                            _ => "outline-subtotals-top",
                        };
                        let _ = write!(
                            xml,
                            r#"<table:data-pilot-layout-info table:layout-mode="{mode}" table:add-empty-lines="false"/>"#
                        );
                    }
                    xml.push_str("</table:data-pilot-level></table:data-pilot-field>");
                }
                if orientation == "row" && data_row == Some(fields.len()) {
                    xml.push_str(DATA_LAYOUT_ROW);
                }
                if orientation == "column" && data_column == Some(fields.len()) {
                    xml.push_str(DATA_LAYOUT_COLUMN);
                }
            }
            for value in &table.values {
                let _ = write!(
                    xml,
                    r#"<table:data-pilot-field table:source-field-name="{}" table:orientation="data" table:function="{}"/>"#,
                    escape(&name_of(value.field)),
                    value.function.odf()
                );
            }
            xml.push_str("</table:data-pilot-table>");
        }
    }
    if xml.is_empty() {
        xml
    } else {
        format!("<table:data-pilot-tables>{xml}</table:data-pilot-tables>")
    }
}

/// `Σ Values` as a row field of a data pilot table.
const DATA_LAYOUT_ROW: &str = r#"<table:data-pilot-field table:source-field-name="" table:is-data-layout-field="true" table:orientation="row"/>"#;

/// `Σ Values` as a column field of a data pilot table.
const DATA_LAYOUT_COLUMN: &str = r#"<table:data-pilot-field table:source-field-name="" table:is-data-layout-field="true" table:orientation="column"/>"#;

/// A range as `OpenDocument` writes it: `'Vendite'.A1:'Vendite'.C10`.
fn range_address(sheet: &str, range: CellRange) -> String {
    let sheet = quoted_sheet(sheet);
    format!(
        "{sheet}.{}{}:{sheet}.{}{}",
        column_name(range.first_column),
        range.first_row + 1,
        column_name(range.last_column),
        range.last_row + 1
    )
}

/// The sheet and range of an `OpenDocument` range address; the end's sheet may be left out.
fn parse_range_address(address: &str) -> Option<(String, CellRange)> {
    let (start, end) = address.split_once(':').unwrap_or((address, address));
    let (sheet, first) = parse_cell_address(start)?;
    let (_, last) = parse_cell_address(end)?;
    Some((
        sheet?,
        CellRange {
            first_row: first.row.min(last.row),
            first_column: first.column.min(last.column),
            last_row: first.row.max(last.row),
            last_column: first.column.max(last.column),
        },
    ))
}

/// The sheet, when named, and the cell of an address such as `$'Q1 2026'.$B$3`.
fn parse_cell_address(address: &str) -> Option<(Option<String>, CellRef)> {
    let address = address.trim().trim_start_matches('$');
    let (sheet, cell) = match address.rfind('.') {
        Some(dot) => (Some(&address[..dot]), &address[dot + 1..]),
        None => (None, address),
    };
    let sheet = sheet.filter(|sheet| !sheet.is_empty()).map(|sheet| {
        let sheet = sheet.trim_start_matches('$');
        sheet
            .strip_prefix('\'')
            .and_then(|quoted| quoted.strip_suffix('\''))
            .map_or_else(|| sheet.to_owned(), |quoted| quoted.replace("''", "'"))
    });
    let cell = cell.replace('$', "");
    let split = cell.find(|character: char| character.is_ascii_digit())?;
    let column = column_index(&cell[..split])?;
    let row = cell[split..].parse::<u32>().ok()?.checked_sub(1)?;
    Some((sheet, CellRef::new(row, column)))
}

/// A field of a data pilot table as the file names it, before the header gives its position.
#[derive(Debug, Default)]
pub(crate) struct PendingField {
    name: String,
    orientation: String,
    function: Option<PivotFunction>,
    selected_page: Option<String>,
    hidden: Vec<String>,
    /// The `table:layout-mode` of a row field.
    layout_mode: Option<String>,
    /// Whether the field shows subtotals.
    subtotals: bool,
    repeat: bool,
}

/// A data pilot table read, its fields still by name.
#[derive(Debug, Default)]
pub(crate) struct PendingPivot {
    name: String,
    target_sheet: String,
    target: CellRange,
    source_sheet: String,
    source: CellRange,
    grand_total: String,
    fields: Vec<PendingField>,
    /// The orientation of `Σ Values` and how many fields of that orientation come before it.
    data_layout: Option<(String, usize)>,
}

fn attribute(element: &BytesStart<'_>, name: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.local_name().as_ref() == name)
        .and_then(|attribute| {
            attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()
                .map(std::borrow::Cow::into_owned)
        })
}

/// The data pilot tables of an ods, read from its content; none from a file it cannot read.
#[expect(
    clippy::too_many_lines,
    reason = "one pass over the content reads every part of a data pilot table"
)]
pub(crate) fn read_data_pilots(bytes: &[u8], limits: &Limits) -> Vec<PendingPivot> {
    let Ok(mut archive) = zip::ZipArchive::new(Cursor::new(bytes)) else {
        return Vec::new();
    };
    let Ok(member) = archive.by_name("content.xml") else {
        return Vec::new();
    };
    let mut content = String::new();
    if member
        .take(limits.max_member_bytes)
        .read_to_string(&mut content)
        .is_err()
    {
        return Vec::new();
    }
    let mut reader = Reader::from_str(&content);
    let mut pivots = Vec::new();
    let mut current: Option<PendingPivot> = None;
    loop {
        let Ok(event) = reader.read_event() else {
            return Vec::new();
        };
        let (element, empty) = match &event {
            Event::Start(element) => (element, false),
            Event::Empty(element) => (element, true),
            Event::End(end) => {
                if end.local_name().as_ref() == "data-pilot-table"
                    && let Some(pivot) = current.take()
                {
                    pivots.push(pivot);
                }
                continue;
            }
            Event::Eof => return pivots,
            _ => continue,
        };
        match element.local_name().as_ref() {
            "data-pilot-table" => {
                let target = attribute(element, "target-range-address")
                    .and_then(|address| parse_range_address(&address));
                current = target.map(|(target_sheet, target)| PendingPivot {
                    name: attribute(element, "name").unwrap_or_default(),
                    target_sheet,
                    target,
                    grand_total: attribute(element, "grand-total")
                        .unwrap_or_else(|| "both".to_owned()),
                    ..PendingPivot::default()
                });
                if empty && let Some(pivot) = current.take() {
                    pivots.push(pivot);
                }
            }
            "source-cell-range" => {
                if let Some(pivot) = &mut current
                    && let Some((sheet, range)) = attribute(element, "cell-range-address")
                        .and_then(|address| parse_range_address(&address))
                {
                    pivot.source_sheet = sheet;
                    pivot.source = range;
                }
            }
            "data-pilot-field" => {
                if let Some(pivot) = &mut current {
                    if matches!(
                        attribute(element, "is-data-layout-field").as_deref(),
                        Some("true")
                    ) {
                        let orientation = attribute(element, "orientation").unwrap_or_default();
                        let before = pivot
                            .fields
                            .iter()
                            .filter(|field| field.orientation == orientation)
                            .count();
                        pivot.data_layout = Some((orientation, before));
                        continue;
                    }
                    pivot.fields.push(PendingField {
                        name: attribute(element, "source-field-name").unwrap_or_default(),
                        orientation: attribute(element, "orientation").unwrap_or_default(),
                        function: attribute(element, "function")
                            .and_then(|function| PivotFunction::from_odf(&function)),
                        selected_page: attribute(element, "selected-page"),
                        ..PendingField::default()
                    });
                }
            }
            "data-pilot-level" => {
                if let Some(field) = current.as_mut().and_then(|pivot| pivot.fields.last_mut()) {
                    field.repeat =
                        attribute(element, "repeat-item-labels").as_deref() == Some("true");
                }
            }
            "data-pilot-subtotal" => {
                if let Some(field) = current.as_mut().and_then(|pivot| pivot.fields.last_mut()) {
                    field.subtotals |= attribute(element, "function").as_deref() != Some("none");
                }
            }
            "data-pilot-layout-info" => {
                if let Some(field) = current.as_mut().and_then(|pivot| pivot.fields.last_mut()) {
                    field.layout_mode = attribute(element, "layout-mode");
                }
            }
            "data-pilot-member" => {
                if let Some(field) = current.as_mut().and_then(|pivot| pivot.fields.last_mut())
                    && matches!(attribute(element, "display").as_deref(), Some("false"))
                    && let Some(name) = attribute(element, "name")
                {
                    field.hidden.push(name);
                }
            }
            _ => {}
        }
    }
}

/// Give the workbook the data pilot tables read, each field placed by the position of its name
/// in the source's header; a table whose source or fields cannot be found is left out.
#[expect(
    clippy::too_many_lines,
    reason = "the form, the fields and the cells of a table are placed in one pass"
)]
pub(crate) fn place_data_pilots(
    workbook: &mut Workbook,
    pending: Vec<PendingPivot>,
    locale: Locale,
) {
    for pivot in pending {
        let Some(sheet) = workbook.sheet_index(&pivot.target_sheet) else {
            continue;
        };
        let mut table = PivotTable::new(
            &pivot.name,
            &pivot.source_sheet,
            pivot.source,
            CellRef::new(pivot.target.first_row, pivot.target.first_column),
        );
        table.locale = locale;
        table.row_grand_totals = matches!(pivot.grand_total.as_str(), "both" | "row");
        table.column_grand_totals = matches!(pivot.grand_total.as_str(), "both" | "column");
        let Ok(source) = table.read_source(workbook) else {
            continue;
        };
        let Some((header, records)) = source.split_first() else {
            continue;
        };
        let position = |name: &str| {
            header
                .iter()
                .position(|cell| item_label(cell) == name)
                .or_else(|| {
                    header
                        .iter()
                        .position(|cell| item_label(cell).eq_ignore_ascii_case(name))
                })
                .and_then(|index| u32::try_from(index).ok())
        };
        // The first row field sets the form, else the first column field; a file that says
        // nothing is in LibreOffice's tabular form.
        if let Some(first) = pivot
            .fields
            .iter()
            .find(|field| field.orientation == "row")
            .or_else(|| {
                pivot
                    .fields
                    .iter()
                    .find(|field| field.orientation == "column")
            })
        {
            let mode = first.layout_mode.as_deref().unwrap_or("tabular-layout");
            table.layout = if mode == "tabular-layout" {
                PivotLayout::Tabular
            } else {
                PivotLayout::Outline
            };
            table.subtotals = if !first.subtotals {
                PivotSubtotals::Off
            } else if mode == "outline-subtotals-bottom" {
                PivotSubtotals::Bottom
            } else {
                PivotSubtotals::Top
            };
        }
        table.repeat_labels = pivot
            .fields
            .iter()
            .any(|field| field.orientation == "row" && field.repeat);
        let mut complete = true;
        for field in pivot.fields {
            let Some(index) = position(&field.name) else {
                complete = false;
                continue;
            };
            let mut hidden = field.hidden;
            if let Some(selected) = field.selected_page {
                hidden = records
                    .iter()
                    .map(|record| {
                        record
                            .get(index as usize)
                            .map(item_label)
                            .unwrap_or_default()
                    })
                    .filter(|label| *label != selected)
                    .collect::<BTreeSet<String>>()
                    .into_iter()
                    .collect();
            }
            let placed = PivotField {
                field: index,
                hidden,
            };
            match field.orientation.as_str() {
                "row" => table.rows.push(placed),
                "column" => table.columns.push(placed),
                "page" => table.filters.push(placed),
                "data" => table.values.push(PivotValue {
                    field: index,
                    function: field.function.unwrap_or_default(),
                    name: None,
                }),
                _ => {}
            }
        }
        if !complete {
            continue;
        }
        match pivot.data_layout {
            Some((orientation, before)) if orientation == "row" => {
                table.values_axis = PivotValuesAxis::Rows;
                table.values_position = (before < table.rows.len()).then_some(before);
            }
            Some((orientation, before)) if orientation == "column" => {
                table.values_position = (before < table.columns.len()).then_some(before);
            }
            _ => {}
        }
        table.area = Some(pivot.target);
        table.snapshot = source;
        if workbook.sheets[sheet].pivot_tables.len() < crate::pivots::MAX_PIVOT_TABLES {
            workbook.sheets[sheet].pivot_tables.push(table);
        }
    }
}

#[cfg(test)]
mod tests;
