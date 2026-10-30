// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The pivot tables of a workbook as an xlsx keeps them: a pivot cache definition naming the
//! source range and the items of each field, and a pivot table definition placing the fields.
//! The cache is written without its records and marked to refresh on load, so Excel rebuilds it
//! from the source the first time the file is opened.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use rely_office_core::{CarriedPart, CarriedPivot, CellRange, range_reference};

use super::escape;
use crate::address::CellRef;
use crate::formula::number_text;
use crate::model::Workbook;
use crate::pivots::{Key, PivotField, PivotFunction, PivotTable, PivotValue, item_label};
use crate::value::{Locale, Value};

/// A pivot table the writer makes, numbered across the workbook, with the id of its cache.
pub(super) struct PivotPart {
    pub(super) sheet: usize,
    pub(super) table: usize,
    pub(super) number: usize,
    pub(super) cache_id: u32,
}

impl PivotPart {
    pub(super) fn table_part(&self) -> String {
        format!("xl/pivotTables/relyPivotTable{}.xml", self.number)
    }

    pub(super) fn cache_part(&self) -> String {
        format!("xl/pivotCache/relyPivotCacheDefinition{}.xml", self.number)
    }

    pub(super) fn table_relationships_part(&self) -> String {
        format!(
            "xl/pivotTables/_rels/relyPivotTable{}.xml.rels",
            self.number
        )
    }
}

/// The pivot tables of the workbook's sheets, numbered in sheet order, their cache ids after
/// those of the pivot tables the file carried.
pub(super) fn pivot_parts(workbook: &Workbook) -> Vec<PivotPart> {
    let mut next_id = workbook
        .sheets
        .iter()
        .flat_map(|sheet| &sheet.pivots)
        .map(|pivot| pivot.cache_id + 1)
        .max()
        .unwrap_or(1);
    let mut parts = Vec::new();
    for (sheet, content) in workbook.sheets.iter().enumerate() {
        for table in 0..content.pivot_tables.len() {
            parts.push(PivotPart {
                sheet,
                table,
                number: parts.len() + 1,
                cache_id: next_id,
            });
            next_id += 1;
        }
    }
    parts
}

/// The relationship of a pivot table part to its cache.
pub(super) fn table_relationships_xml(part: &PivotPart) -> String {
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
            r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheDefinition" Target="../pivotCache/relyPivotCacheDefinition{}.xml"/>"#,
            "</Relationships>"
        ),
        part.number
    )
}

/// The source of a table as the writer describes it: as last computed, else as it is now.
fn source_of(workbook: &Workbook, table: &PivotTable) -> Vec<Vec<Value>> {
    if table.snapshot.is_empty() {
        table.read_source(workbook).unwrap_or_default()
    } else {
        table.snapshot.clone()
    }
}

/// The distinct items of a source column, as a pivot table orders them.
fn items_of(records: &[Vec<Value>], field: usize) -> Vec<Value> {
    records
        .iter()
        .map(|record| Key(record.get(field).cloned().unwrap_or(Value::Empty)))
        .collect::<BTreeSet<Key>>()
        .into_iter()
        .map(|key| key.0)
        .collect()
}

/// The cache definition of a table: its source and its fields, the items listed of every field
/// an axis or a filter places.
pub(super) fn cache_definition_xml(workbook: &Workbook, table: &PivotTable) -> String {
    let source = source_of(workbook, table);
    let (header, records) = source
        .split_first()
        .map_or((&[][..], &[][..]), |(header, records)| {
            (header.as_slice(), records)
        });
    let placed: BTreeSet<u32> = table
        .rows
        .iter()
        .chain(&table.columns)
        .chain(&table.filters)
        .map(|field| field.field)
        .collect();
    let mut xml = format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            r#"<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" "#,
            r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#,
            r#"saveData="0" refreshOnLoad="1" createdVersion="6" refreshedVersion="6" "#,
            r#"minRefreshableVersion="3" recordCount="{}">"#,
            r#"<cacheSource type="worksheet"><worksheetSource ref="{}" sheet="{}"/></cacheSource>"#,
            r#"<cacheFields count="{}">"#
        ),
        records.len(),
        range_reference(&table.source),
        escape(&table.source_sheet),
        table.source.last_column - table.source.first_column + 1
    );
    let width = (table.source.last_column - table.source.first_column + 1) as usize;
    let mut used_names: Vec<String> = Vec::new();
    for field in 0..width {
        let mut name = header.get(field).map(item_label).unwrap_or_default();
        if name.is_empty() {
            name = format!("Column{}", field + 1);
        }
        // Excel refuses two cache fields of one name.
        let base = name.clone();
        let mut suffix = 2;
        while used_names
            .iter()
            .any(|used| used.eq_ignore_ascii_case(&name))
        {
            name = format!("{base}{suffix}");
            suffix += 1;
        }
        used_names.push(name.clone());
        let items = items_of(records, field);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a source is at most 16,384 wide"
        )]
        let listed = placed.contains(&(field as u32));
        let _ = write!(
            xml,
            r#"<cacheField name="{}" numFmtId="0">{}</cacheField>"#,
            escape(&name),
            shared_items_xml(&items, listed)
        );
    }
    xml.push_str("</cacheFields></pivotCacheDefinition>");
    xml
}

/// The `sharedItems` of a field: what kinds of values it holds and, for a field a table places,
/// each item.
fn shared_items_xml(items: &[Value], listed: bool) -> String {
    let has = |test: fn(&Value) -> bool| items.iter().any(test);
    let strings = has(|value| matches!(value, Value::Text(_)));
    let numbers: Vec<f64> = items
        .iter()
        .filter_map(|value| match value {
            Value::Number(number) => Some(*number),
            _ => None,
        })
        .collect();
    let booleans = has(|value| matches!(value, Value::Bool(_)));
    let errors = has(|value| matches!(value, Value::Error(_)));
    let blanks = has(|value| matches!(value, Value::Empty));
    let kinds = [strings, !numbers.is_empty(), booleans, errors]
        .into_iter()
        .filter(|kind| *kind)
        .count();
    let mut attributes = String::new();
    if !strings && !booleans && !errors && !blanks {
        attributes.push_str(r#" containsSemiMixedTypes="0""#);
    }
    if !strings {
        attributes.push_str(r#" containsString="0""#);
    }
    if kinds > 1 {
        attributes.push_str(r#" containsMixedTypes="1""#);
    }
    if !numbers.is_empty() {
        attributes.push_str(r#" containsNumber="1""#);
        if numbers.iter().all(|number| number.fract() == 0.0) {
            attributes.push_str(r#" containsInteger="1""#);
        }
        let minimum = numbers.iter().copied().fold(f64::INFINITY, f64::min);
        let maximum = numbers.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let _ = write!(
            attributes,
            r#" minValue="{}" maxValue="{}""#,
            number_text(minimum),
            number_text(maximum)
        );
    }
    if blanks {
        attributes.push_str(r#" containsBlank="1""#);
    }
    if !listed {
        return format!("<sharedItems{attributes}/>");
    }
    let mut xml = format!(r#"<sharedItems{attributes} count="{}">"#, items.len());
    for item in items {
        match item {
            Value::Text(text) => {
                let _ = write!(xml, r#"<s v="{}"/>"#, escape(text));
            }
            Value::Number(number) => {
                let _ = write!(xml, r#"<n v="{}"/>"#, number_text(*number));
            }
            Value::Bool(flag) => {
                let _ = write!(xml, r#"<b v="{}"/>"#, u8::from(*flag));
            }
            Value::Error(kind) => {
                let _ = write!(xml, r#"<e v="{}"/>"#, escape(kind.code()));
            }
            Value::Empty => xml.push_str("<m/>"),
        }
    }
    xml.push_str("</sharedItems>");
    xml
}

/// The definition of a table: where it sits, how each source column is placed, which items
/// are hidden, and what its values summarise.
#[expect(
    clippy::too_many_lines,
    reason = "the parts of a definition are written in the order the schema sets"
)]
pub(super) fn table_definition_xml(
    workbook: &Workbook,
    table: &PivotTable,
    cache_id: u32,
) -> String {
    let source = source_of(workbook, table);
    let (header, records) = source
        .split_first()
        .map_or((&[][..], &[][..]), |(header, records)| {
            (header.as_slice(), records)
        });
    let area = table.area.unwrap_or(CellRange {
        first_row: table.anchor.row,
        first_column: table.anchor.column,
        last_row: table.anchor.row,
        last_column: table.anchor.column,
    });
    let page_rows = if table.filters.is_empty() {
        0
    } else {
        u32::try_from(table.filters.len()).unwrap_or(0) + 1
    };
    let location = CellRange {
        first_row: (area.first_row + page_rows).min(area.last_row),
        ..area
    };
    let mut xml = format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            r#"<pivotTableDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" "#,
            r#"name="{}" cacheId="{}" applyNumberFormats="0" applyBorderFormats="0" applyFontFormats="0" "#,
            r#"applyPatternFormats="0" applyAlignmentFormats="0" applyWidthHeightFormats="1" "#,
            r#"dataCaption="{}" updatedVersion="6" minRefreshableVersion="3" useAutoFormatting="1" "#,
            r#"itemPrintTitles="1" createdVersion="6" indent="0" outline="1" outlineData="1" "#,
            r#"multipleFieldFilters="0"{}{}>"#,
            r#"<location ref="{}" firstHeaderRow="0" firstDataRow="1" firstDataCol="{}"{}/>"#
        ),
        escape(&table.name),
        cache_id,
        if table.locale == Locale::Italian {
            "Valori"
        } else {
            "Values"
        },
        if table.row_grand_totals {
            ""
        } else {
            r#" rowGrandTotals="0""#
        },
        if table.column_grand_totals {
            ""
        } else {
            r#" colGrandTotals="0""#
        },
        range_reference(&location),
        table.rows.len(),
        if table.filters.is_empty() {
            String::new()
        } else {
            format!(
                r#" rowPageCount="{}" colPageCount="1""#,
                table.filters.len()
            )
        }
    );
    let width = table.source.last_column - table.source.first_column + 1;
    let _ = write!(xml, r#"<pivotFields count="{width}">"#);
    for field in 0..width {
        let placement = [
            (&table.rows, "axisRow"),
            (&table.columns, "axisCol"),
            (&table.filters, "axisPage"),
        ]
        .into_iter()
        .find_map(|(fields, axis)| {
            fields
                .iter()
                .find(|placed| placed.field == field)
                .map(|placed| (placed, axis))
        });
        let data = if table.values.iter().any(|value| value.field == field) {
            r#" dataField="1""#
        } else {
            ""
        };
        match placement {
            Some((placed, axis)) => {
                let items = items_of(records, field as usize);
                let _ = write!(
                    xml,
                    r#"<pivotField axis="{axis}"{data} showAll="0"><items count="{}">"#,
                    items.len() + 1
                );
                for (index, item) in items.iter().enumerate() {
                    let hidden = if placed.hidden.contains(&item_label(item)) {
                        r#" h="1""#
                    } else {
                        ""
                    };
                    let _ = write!(xml, r#"<item{hidden} x="{index}"/>"#);
                }
                xml.push_str(r#"<item t="default"/></items></pivotField>"#);
            }
            None => {
                let _ = write!(xml, r#"<pivotField{data} showAll="0"/>"#);
            }
        }
    }
    xml.push_str("</pivotFields>");
    if !table.rows.is_empty() {
        let _ = write!(xml, r#"<rowFields count="{}">"#, table.rows.len());
        for field in &table.rows {
            let _ = write!(xml, r#"<field x="{}"/>"#, field.field);
        }
        xml.push_str("</rowFields>");
    }
    let several_values = table.values.len() > 1;
    let column_count = table.columns.len() + usize::from(several_values);
    if column_count > 0 {
        let _ = write!(xml, r#"<colFields count="{column_count}">"#);
        for field in &table.columns {
            let _ = write!(xml, r#"<field x="{}"/>"#, field.field);
        }
        if several_values {
            xml.push_str(r#"<field x="-2"/>"#);
        }
        xml.push_str("</colFields>");
    }
    if !table.filters.is_empty() {
        let _ = write!(xml, r#"<pageFields count="{}">"#, table.filters.len());
        for filter in &table.filters {
            let items = items_of(records, filter.field as usize);
            let shown: Vec<usize> = items
                .iter()
                .enumerate()
                .filter(|(_, item)| !filter.hidden.contains(&item_label(item)))
                .map(|(index, _)| index)
                .collect();
            let item = match shown.as_slice() {
                [only] if items.len() > 1 => format!(r#" item="{only}""#),
                _ => String::new(),
            };
            let _ = write!(
                xml,
                r#"<pageField fld="{}"{item} hier="-1"/>"#,
                filter.field
            );
        }
        xml.push_str("</pageFields>");
    }
    if !table.values.is_empty() {
        let _ = write!(xml, r#"<dataFields count="{}">"#, table.values.len());
        for value in &table.values {
            let subtotal = if value.function == PivotFunction::Sum {
                String::new()
            } else {
                format!(r#" subtotal="{}""#, value.function.xlsx())
            };
            let _ = write!(
                xml,
                r#"<dataField name="{}" fld="{}"{subtotal} baseField="0" baseItem="0"/>"#,
                escape(&table.value_caption(value, header)),
                value.field
            );
        }
        xml.push_str("</dataFields>");
    }
    xml.push_str(concat!(
        r#"<pivotTableStyleInfo name="PivotStyleLight16" showRowHeaders="1" showColHeaders="1" "#,
        r#"showRowStripes="0" showColStripes="0" showLastColumn="1"/></pivotTableDefinition>"#
    ));
    xml
}

/// The attributes of an element by local name.
fn attributes(element: &BytesStart<'_>) -> Vec<(String, String)> {
    element
        .attributes()
        .flatten()
        .filter_map(|attribute| {
            let key = attribute.key.local_name().as_ref().to_owned();
            let value = attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()?
                .into_owned();
            Some((key, value))
        })
        .collect()
}

fn attribute<'a>(attributes: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

/// What a cache definition says: the sheet and range of its source, and each field's name and
/// the labels of its items.
#[derive(Debug, Default)]
struct CacheSource {
    sheet: Option<String>,
    range: Option<CellRange>,
    fields: Vec<(String, Vec<String>)>,
}

fn read_cache(xml: &str) -> Option<CacheSource> {
    let mut reader = Reader::from_str(xml);
    let mut cache = CacheSource::default();
    let mut in_items = false;
    loop {
        let event = reader.read_event().ok()?;
        let (element, empty) = match &event {
            Event::Start(element) => (element, false),
            Event::Empty(element) => (element, true),
            Event::End(end) => {
                if end.local_name().as_ref() == "sharedItems" {
                    in_items = false;
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        let found = attributes(element);
        match element.local_name().as_ref() {
            "worksheetSource" => {
                if attribute(&found, "name").is_some() {
                    return None;
                }
                cache.sheet = attribute(&found, "sheet").map(str::to_owned);
                cache.range = attribute(&found, "ref").and_then(CellRange::parse);
            }
            "cacheField" => cache.fields.push((
                attribute(&found, "name").unwrap_or_default().to_owned(),
                Vec::new(),
            )),
            "sharedItems" => in_items = !empty,
            "s" | "n" | "b" | "e" | "d" | "m" if in_items => {
                let raw = attribute(&found, "v").unwrap_or_default();
                let label = match element.local_name().as_ref() {
                    "n" => raw
                        .parse::<f64>()
                        .map_or_else(|_| raw.to_owned(), number_text),
                    "b" => {
                        if matches!(raw, "1" | "true") {
                            "TRUE".to_owned()
                        } else {
                            "FALSE".to_owned()
                        }
                    }
                    "m" => String::new(),
                    _ => raw.to_owned(),
                };
                if let Some((_, items)) = cache.fields.last_mut() {
                    items.push(label);
                }
            }
            _ => {}
        }
    }
    cache.range?;
    Some(cache)
}

/// A pivot table of an xlsx as the model holds it, from its definition and its cache's; `None`
/// for one the model cannot hold (a source that is a name or another workbook), which then
/// stays carried as it was.
#[expect(
    clippy::too_many_lines,
    reason = "one pass over the definition reads every part of it"
)]
fn read_table(
    table_xml: &str,
    cache: &CacheSource,
    own_sheet: &str,
    locale: Locale,
) -> Option<PivotTable> {
    let mut reader = Reader::from_str(table_xml);
    let mut name = String::new();
    let mut location: Option<CellRange> = None;
    let mut row_totals = true;
    let mut column_totals = true;
    let mut page_count = 0_u32;
    // Each pivot field: its axis and the indexes of its hidden items.
    let mut fields: Vec<(Option<String>, Vec<usize>)> = Vec::new();
    let mut in_field = false;
    let mut section = "";
    let mut rows = Vec::new();
    let mut columns = Vec::new();
    let mut pages: Vec<(u32, Option<usize>)> = Vec::new();
    let mut values = Vec::new();
    loop {
        let event = reader.read_event().ok()?;
        let (element, empty) = match &event {
            Event::Start(element) => (element, false),
            Event::Empty(element) => (element, true),
            Event::End(end) => {
                match end.local_name().as_ref() {
                    "pivotField" => in_field = false,
                    "rowFields" | "colFields" => section = "",
                    _ => {}
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        let found = attributes(element);
        let number = |key: &str| attribute(&found, key).and_then(|value| value.parse::<i64>().ok());
        let flag =
            |key: &str| attribute(&found, key).is_none_or(|value| !matches!(value, "0" | "false"));
        match element.local_name().as_ref() {
            "pivotTableDefinition" => {
                attribute(&found, "name")
                    .unwrap_or_default()
                    .clone_into(&mut name);
                row_totals = flag("rowGrandTotals");
                column_totals = flag("colGrandTotals");
            }
            "location" => {
                location = attribute(&found, "ref").and_then(CellRange::parse);
                page_count = number("rowPageCount")
                    .and_then(|count| u32::try_from(count).ok())
                    .unwrap_or(0);
            }
            "pivotField" => {
                fields.push((attribute(&found, "axis").map(str::to_owned), Vec::new()));
                in_field = !empty;
            }
            "item" if in_field => {
                if matches!(attribute(&found, "h"), Some("1" | "true"))
                    && let Some(index) = number("x").and_then(|index| usize::try_from(index).ok())
                    && let Some((_, hidden)) = fields.last_mut()
                {
                    hidden.push(index);
                }
            }
            "rowFields" => section = "rows",
            "colFields" => section = "columns",
            "field" => {
                if let Some(index) = number("x").and_then(|index| u32::try_from(index).ok()) {
                    match section {
                        "rows" => rows.push(index),
                        "columns" => columns.push(index),
                        _ => {}
                    }
                }
            }
            "pageField" => {
                if let Some(field) = number("fld").and_then(|index| u32::try_from(index).ok()) {
                    pages.push((
                        field,
                        number("item").and_then(|item| usize::try_from(item).ok()),
                    ));
                }
            }
            "dataField" => {
                let field = number("fld").and_then(|index| u32::try_from(index).ok())?;
                values.push(PivotValue {
                    field,
                    function: attribute(&found, "subtotal")
                        .map_or(Some(PivotFunction::Sum), PivotFunction::from_xlsx)?,
                    name: attribute(&found, "name").map(str::to_owned),
                });
            }
            _ => {}
        }
    }
    let location = location?;
    let source = cache.range?;
    let width = source.last_column - source.first_column + 1;
    let items_of_field = |field: u32| {
        cache
            .fields
            .get(field as usize)
            .map(|(_, items)| items.as_slice())
            .unwrap_or_default()
    };
    let placed = |field: u32| -> PivotField {
        let items = items_of_field(field);
        let hidden = fields
            .get(field as usize)
            .map(|(_, hidden)| {
                hidden
                    .iter()
                    .filter_map(|index| items.get(*index).cloned())
                    .collect()
            })
            .unwrap_or_default();
        PivotField { field, hidden }
    };
    let anchor_row = if page_count > 0 {
        location.first_row.checked_sub(page_count + 1)?
    } else {
        location.first_row
    };
    let mut table = PivotTable::new(
        &name,
        cache.sheet.as_deref().unwrap_or(own_sheet),
        source,
        CellRef::new(anchor_row, location.first_column),
    );
    table.locale = locale;
    table.rows = rows
        .into_iter()
        .filter(|field| *field < width)
        .map(placed)
        .collect();
    table.columns = columns
        .into_iter()
        .filter(|field| *field < width)
        .map(placed)
        .collect();
    table.filters = pages
        .into_iter()
        .filter(|(field, _)| *field < width)
        .map(|(field, item)| {
            let mut filter = placed(field);
            if let Some(chosen) = item.and_then(|item| items_of_field(field).get(item)) {
                filter.hidden = items_of_field(field)
                    .iter()
                    .filter(|label| *label != chosen)
                    .cloned()
                    .collect();
            }
            filter
        })
        .collect();
    table.values = values
        .into_iter()
        .filter(|value| value.field < width)
        .collect();
    table.row_grand_totals = row_totals;
    table.column_grand_totals = column_totals;
    table.area = Some(CellRange {
        first_row: anchor_row,
        ..location
    });
    Some(table)
}

/// The pivot tables of a sheet the model can hold, and those it carries as the file had them.
pub(super) fn model_pivots(
    carried: Vec<CarriedPivot>,
    own_sheet: &str,
    locale: Locale,
) -> (Vec<PivotTable>, Vec<CarriedPivot>) {
    let mut tables = Vec::new();
    let mut kept = Vec::new();
    for pivot in carried {
        let text = |name: &str| {
            pivot
                .parts
                .iter()
                .find(|part| part.name == name)
                .and_then(|part| String::from_utf8(part.bytes.clone()).ok())
        };
        let table = text(&pivot.table_part)
            .zip(text(&pivot.cache_part).and_then(|xml| read_cache(&xml)))
            .and_then(|(table, cache)| read_table(&table, &cache, own_sheet, locale));
        match table {
            Some(table) => tables.push(table),
            None => kept.push(pivot),
        }
    }
    (tables, kept)
}

/// The pivot tables each sheet writes: those the file carried, then the model's, made into
/// parts the way a carried one holds them.
pub(super) fn written_pivots(workbook: &Workbook) -> Vec<Vec<CarriedPivot>> {
    let mut written: Vec<Vec<CarriedPivot>> = workbook
        .sheets
        .iter()
        .map(|sheet| sheet.pivots.clone())
        .collect();
    for part in pivot_parts(workbook) {
        let table = &workbook.sheets[part.sheet].pivot_tables[part.table];
        written[part.sheet].push(CarriedPivot {
            table_part: part.table_part(),
            cache_part: part.cache_part(),
            cache_id: part.cache_id,
            parts: vec![
                CarriedPart {
                    name: part.table_part(),
                    content_type: TABLE_TYPE.to_owned(),
                    bytes: table_definition_xml(workbook, table, part.cache_id).into_bytes(),
                },
                CarriedPart {
                    name: part.table_relationships_part(),
                    content_type: RELATIONSHIPS_TYPE.to_owned(),
                    bytes: table_relationships_xml(&part).into_bytes(),
                },
                CarriedPart {
                    name: part.cache_part(),
                    content_type: CACHE_TYPE.to_owned(),
                    bytes: cache_definition_xml(workbook, table).into_bytes(),
                },
            ],
        });
    }
    written
}

const TABLE_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml";
const CACHE_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml";
const RELATIONSHIPS_TYPE: &str = "application/vnd.openxmlformats-package.relationships+xml";

#[cfg(test)]
mod tests;
