// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use std::io::Read as _;

use rely_office_core::Limits;

use super::*;
use crate::pivots::tests::{by_region, sales};
use crate::xlsx::{read_spreadsheet, write_xlsx};

fn member(bytes: &[u8], name: &str) -> String {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut text = String::new();
    archive
        .by_name(name)
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    text
}

fn full_table() -> PivotTable {
    let mut table = by_region("F1");
    table.columns = vec![PivotField {
        field: 2,
        hidden: Vec::new(),
    }];
    table.filters = vec![PivotField {
        field: 1,
        hidden: vec!["Pere".to_owned()],
    }];
    table.rows[0].hidden = vec!["Centro".to_owned()];
    table.values.push(PivotValue {
        field: 3,
        function: PivotFunction::Average,
        name: Some("Media".to_owned()),
    });
    table.column_grand_totals = false;
    table
}

#[test]
fn writes_the_cache_and_the_table_excel_refreshes() {
    let mut workbook = sales();
    workbook.add_pivot_table(0, full_table()).unwrap();
    let bytes = write_xlsx(&workbook).unwrap();
    let cache = member(&bytes, "xl/pivotCache/relyPivotCacheDefinition1.xml");
    assert!(cache.contains(r#"refreshOnLoad="1""#));
    assert!(cache.contains(r#"<worksheetSource ref="A1:D7" sheet="Foglio1"/>"#));
    assert!(!cache.contains(
        r#"<cacheField name="Regione" numFmtId="0"><sharedItems containsSemiMixedTypes="0"#
    ));
    assert!(cache.contains(r#"<s v="Centro"/><s v="Nord"/><s v="Sud"/>"#));
    assert!(cache.contains(
        r#"containsNumber="1" containsInteger="1" minValue="3" maxValue="20" containsBlank="1"/>"#
    ));
    let table = member(&bytes, "xl/pivotTables/relyPivotTable1.xml");
    assert!(table.contains(r#"name="Vendite" cacheId="1""#));
    assert!(table.contains(r#"colGrandTotals="0""#));
    assert!(table.contains(r#"<location ref="F3:"#));
    assert!(table.contains(r#"rowPageCount="1" colPageCount="1""#));
    assert!(table.contains(
        r#"<pivotField axis="axisRow" compact="0" outline="0" showAll="0"><items count="4"><item h="1" x="0"/>"#
    ));
    assert!(table.contains(r#"<field x="2"/><field x="-2"/>"#));
    assert!(table.contains(r#"<pageField fld="1" item="0" hier="-1"/>"#));
    assert!(
        table.contains(r#"<dataField name="Sum of Importo" fld="3" baseField="0" baseItem="0"/>"#)
    );
    assert!(table.contains(r#"<dataField name="Media" fld="3" subtotal="average""#));
    assert!(
        member(&bytes, "xl/workbook.xml")
            .contains(r#"<pivotCache cacheId="1" r:id="rIdPivotCache1"/>"#)
    );
    assert!(
        member(&bytes, "xl/worksheets/_rels/sheet1.xml.rels")
            .contains("../pivotTables/relyPivotTable1.xml")
    );
    assert!(
        member(&bytes, "xl/pivotTables/_rels/relyPivotTable1.xml.rels")
            .contains("relyPivotCacheDefinition1.xml")
    );
    let types = member(&bytes, "[Content_Types].xml");
    assert!(types.contains("spreadsheetml.pivotTable+xml"));
    assert!(types.contains("spreadsheetml.pivotCacheDefinition+xml"));
}

#[test]
fn a_table_written_reads_back_the_same() {
    let mut workbook = sales();
    workbook.add_pivot_table(0, full_table()).unwrap();
    let written = workbook.sheets()[0].pivot_tables()[0].clone();
    let bytes = write_xlsx(&workbook).unwrap();
    let read =
        read_spreadsheet(&bytes, Some("a.xlsx"), &Limits::default(), Locale::English).unwrap();
    let table = &read.sheets()[0].pivot_tables()[0];
    assert_eq!(table.name, written.name);
    assert_eq!(table.source_sheet, written.source_sheet);
    assert_eq!(table.source, written.source);
    assert_eq!(table.anchor, written.anchor);
    assert_eq!(table.area(), written.area());
    assert_eq!(table.rows, written.rows);
    assert_eq!(table.columns, written.columns);
    assert_eq!(table.filters, written.filters);
    assert_eq!(table.values[1], written.values[1]);
    assert_eq!(table.values[0].name.as_deref(), Some("Sum of Importo"));
    assert_eq!(
        (table.row_grand_totals, table.column_grand_totals),
        (true, false)
    );
    assert_eq!(read.sheets()[0].pivot_count(), 0);
    // The cells stay as written until a refresh, which writes the same.
    let mut read = read;
    let before = read.value(0, CellRef::new(3, 6));
    read.refresh_pivot_table(0, 0).unwrap();
    assert_eq!(read.value(0, CellRef::new(3, 6)), before);
}

const CACHE: &str = r#"<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><cacheSource type="worksheet"><worksheetSource ref="A1:C4" sheet="Dati"/></cacheSource><cacheFields count="3"><cacheField name="Anno"><sharedItems containsNumber="1" count="2"><n v="2024"/><n v="2025"/></sharedItems></cacheField><cacheField name="Attivo"><sharedItems count="2"><b v="0"/><b v="1"/></sharedItems></cacheField><cacheField name="Valore"><sharedItems containsNumber="1"/></cacheField></cacheFields></pivotCacheDefinition>"#;

#[test]
fn reads_a_table_as_excel_writes_it() {
    let table_xml = r#"<pivotTableDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" name="Tabella pivot1" cacheId="5" rowGrandTotals="0"><location ref="E5:F8" firstHeaderRow="1" firstDataRow="1" firstDataCol="1" rowPageCount="1" colPageCount="1"/><pivotFields count="3"><pivotField axis="axisRow" showAll="0"><items count="3"><item x="0"/><item h="1" x="1"/><item t="default"/></items></pivotField><pivotField axis="axisPage" showAll="0"><items count="3"><item x="0"/><item x="1"/><item t="default"/></items></pivotField><pivotField dataField="1" showAll="0"/></pivotFields><rowFields count="1"><field x="0"/></rowFields><pageFields count="1"><pageField fld="1" item="1" hier="-1"/></pageFields><dataFields count="1"><dataField name="Max di Valore" fld="2" subtotal="max" baseField="0" baseItem="0"/></dataFields></pivotTableDefinition>"#;
    let cache = read_cache(CACHE).unwrap();
    let table = read_table(table_xml, &cache, "Riepilogo", Locale::Italian).unwrap();
    assert_eq!(table.name, "Tabella pivot1");
    assert_eq!(table.source_sheet, "Dati");
    assert_eq!(table.anchor, CellRef::new(2, 4));
    assert_eq!(table.area(), CellRange::parse("E3:F8"));
    assert_eq!(
        table.rows,
        vec![PivotField {
            field: 0,
            hidden: vec!["2025".to_owned()]
        }]
    );
    assert_eq!(
        table.filters,
        vec![PivotField {
            field: 1,
            hidden: vec!["FALSE".to_owned()]
        }]
    );
    assert_eq!(
        table.values,
        vec![PivotValue {
            field: 2,
            function: PivotFunction::Max,
            name: Some("Max di Valore".to_owned())
        }]
    );
    assert!(!table.row_grand_totals && table.column_grand_totals);
    assert_eq!(table.locale, Locale::Italian);
}

#[test]
fn a_table_the_model_cannot_hold_stays_carried() {
    let named = r#"<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><cacheSource type="worksheet"><worksheetSource name="Tabella1"/></cacheSource></pivotCacheDefinition>"#;
    assert!(read_cache(named).is_none());
    assert!(read_cache("<broken").is_none());
    let carried = CarriedPivot {
        table_part: "xl/pivotTables/pivotTable1.xml".to_owned(),
        cache_part: "xl/pivotCache/pivotCacheDefinition1.xml".to_owned(),
        cache_id: 3,
        parts: vec![CarriedPart {
            name: "xl/pivotCache/pivotCacheDefinition1.xml".to_owned(),
            content_type: String::new(),
            bytes: named.as_bytes().to_vec(),
        }],
    };
    let (tables, kept) = model_pivots(vec![carried], "Foglio1", Locale::English);
    assert_eq!(tables.len(), 0);
    assert_eq!(kept.len(), 1);
    let table_without_location = r#"<pivotTableDefinition xmlns="x" name="t"/>"#;
    assert!(
        read_table(
            table_without_location,
            &read_cache(CACHE).unwrap(),
            "Dati",
            Locale::English
        )
        .is_none()
    );
}

#[test]
fn carried_and_model_tables_take_distinct_caches() {
    let mut workbook = sales();
    workbook.sheets[0].pivots.push(CarriedPivot {
        table_part: "xl/pivotTables/pivotTable1.xml".to_owned(),
        cache_part: "xl/pivotCache/pivotCacheDefinition1.xml".to_owned(),
        cache_id: 7,
        parts: Vec::new(),
    });
    workbook.add_pivot_table(0, by_region("F1")).unwrap();
    let parts = pivot_parts(&workbook);
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].cache_id, 8);
    let written = written_pivots(&workbook);
    assert_eq!(written[0].len(), 2);
}

#[test]
fn every_form_and_subtotal_placement_reads_back() {
    for layout in PivotLayout::ALL {
        for subtotals in PivotSubtotals::ALL {
            for repeat_labels in [false, true] {
                let mut workbook = sales();
                let mut table = by_region("F1");
                table.rows.push(PivotField {
                    field: 1,
                    hidden: Vec::new(),
                });
                table.layout = layout;
                table.subtotals = subtotals;
                table.repeat_labels = repeat_labels;
                workbook.add_pivot_table(0, table).unwrap();
                let written = workbook.sheets()[0].pivot_tables()[0].clone();
                let bytes = write_xlsx(&workbook).unwrap();
                let definition = member(&bytes, "xl/pivotTables/relyPivotTable1.xml");
                assert!(definition.contains(&format!(
                    r#"firstHeaderRow="1" firstDataRow="1" firstDataCol="{}""#,
                    written.label_columns()
                )));
                let read =
                    read_spreadsheet(&bytes, Some("a.xlsx"), &Limits::default(), Locale::English)
                        .unwrap();
                let table = &read.sheets()[0].pivot_tables()[0];
                assert_eq!(
                    (table.layout, table.subtotals, table.repeat_labels),
                    (layout, subtotals, repeat_labels),
                    "{definition}"
                );
                assert_eq!(table.area(), written.area());
            }
        }
    }
}

#[test]
fn reads_the_forms_excel_writes() {
    let field = |attributes: &str, extension: &str| {
        format!(
            r#"<pivotTableDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main" name="T" cacheId="1" compact="0" compactData="0"><location ref="E5:F8" firstHeaderRow="1" firstDataRow="1" firstDataCol="1"/><pivotFields count="3"><pivotField axis="axisRow" showAll="0"{attributes}><items count="1"><item x="0"/></items>{extension}</pivotField><pivotField showAll="0"/><pivotField dataField="1" showAll="0"/></pivotFields><rowFields count="1"><field x="0"/></rowFields><dataFields count="1"><dataField name="Somma di Valore" fld="2"/></dataFields></pivotTableDefinition>"#
        )
    };
    let cache = read_cache(CACHE).unwrap();
    let read = |xml: &str| read_table(xml, &cache, "Riepilogo", Locale::Italian).unwrap();
    let tabular = read(&field(
        r#" compact="0" outline="0" defaultSubtotal="0""#,
        r#"<extLst><ext uri="{2946ED86-A175-432a-8AC1-64E0C546D7DE}"><x14:pivotField fillDownLabels="1"/></ext></extLst>"#,
    ));
    assert_eq!(tabular.layout, PivotLayout::Tabular);
    assert_eq!(tabular.subtotals, PivotSubtotals::Off);
    assert!(tabular.repeat_labels);
    assert_eq!(tabular.values.len(), 1);
    let outline = read(&field(r#" compact="0" subtotalTop="0""#, ""));
    assert_eq!(
        (outline.layout, outline.subtotals, outline.repeat_labels),
        (PivotLayout::Outline, PivotSubtotals::Bottom, false)
    );
    // A table marked not compact shows its fields in outline form even when they say compact.
    assert_eq!(read(&field("", "")).layout, PivotLayout::Outline);
    let compact = field("", "").replace(r#" compact="0" compactData="0""#, "");
    assert_eq!(read(&compact).layout, PivotLayout::Compact);
}

#[test]
fn the_values_along_the_rows_survive_an_xlsx() {
    for position in [None, Some(0)] {
        let mut workbook = sales();
        let mut table = by_region("F1");
        table.rows.push(PivotField {
            field: 1,
            hidden: Vec::new(),
        });
        table.values.push(PivotValue {
            field: 3,
            function: PivotFunction::Count,
            name: None,
        });
        table.values_axis = PivotValuesAxis::Rows;
        table.values_position = position;
        workbook.add_pivot_table(0, table).unwrap();
        let written = workbook.sheets()[0].pivot_tables()[0].clone();
        let bytes = write_xlsx(&workbook).unwrap();
        let xml = member(&bytes, "xl/pivotTables/relyPivotTable1.xml");
        assert!(xml.contains(r#"dataOnRows="1""#));
        assert!(xml.contains(match position {
            None => r#"<rowFields count="3"><field x="0"/><field x="1"/><field x="-2"/>"#,
            Some(_) => r#"<rowFields count="3"><field x="-2"/><field x="0"/><field x="1"/>"#,
        }));
        assert!(!xml.contains("<colFields"));
        let mut read =
            read_spreadsheet(&bytes, Some("a.xlsx"), &Limits::default(), Locale::English).unwrap();
        let table = &read.sheets()[0].pivot_tables()[0];
        assert_eq!(table.values_axis, PivotValuesAxis::Rows);
        assert_eq!(table.values_position, position);
        assert_eq!(table.rows, written.rows);
        assert_eq!(table.area(), written.area());
        let before: Vec<Value> = (0..30)
            .map(|row| read.value(0, CellRef::new(row, 7)))
            .collect();
        read.refresh_pivot_table(0, 0).unwrap();
        let after: Vec<Value> = (0..30)
            .map(|row| read.value(0, CellRef::new(row, 7)))
            .collect();
        assert_eq!(after, before);
    }
}

#[test]
fn reads_the_values_of_a_file_from_their_field_or_data_position() {
    let definition = |fields: &str, extra: &str| {
        format!(
            concat!(
                r#"<pivotTableDefinition name="T" cacheId="1"{}>"#,
                r#"<location ref="F1:H9" firstHeaderRow="1" firstDataRow="1" firstDataCol="1"/>"#,
                r#"<pivotFields count="4"><pivotField axis="axisRow" showAll="0"/>"#,
                r#"<pivotField axis="axisRow" showAll="0"/><pivotField showAll="0"/>"#,
                r#"<pivotField dataField="1" showAll="0"/></pivotFields>{}"#,
                r#"<dataFields count="2"><dataField fld="3"/><dataField fld="3" subtotal="count"/></dataFields>"#,
                r#"</pivotTableDefinition>"#
            ),
            extra, fields
        )
    };
    let cache = CacheSource {
        sheet: Some("Foglio1".to_owned()),
        range: CellRange::parse("A1:D7"),
        fields: Vec::new(),
    };
    let read = |xml: &str| read_table(xml, &cache, "Foglio1", Locale::English).unwrap();
    let between = read(&definition(
        r#"<rowFields count="3"><field x="0"/><field x="-2"/><field x="1"/></rowFields>"#,
        "",
    ));
    assert_eq!(between.values_axis, PivotValuesAxis::Rows);
    assert_eq!(between.values_position, Some(1));
    let flagged = read(&definition(
        r#"<rowFields count="2"><field x="0"/><field x="1"/></rowFields>"#,
        r#" dataOnRows="1" dataPosition="0""#,
    ));
    assert_eq!(flagged.values_axis, PivotValuesAxis::Rows);
    assert_eq!(flagged.values_position, Some(0));
    let across = read(&definition(
        r#"<rowFields count="2"><field x="0"/><field x="1"/></rowFields><colFields count="1"><field x="-2"/></colFields>"#,
        "",
    ));
    assert_eq!(across.values_axis, PivotValuesAxis::Columns);
    assert_eq!(across.values_position, None);
}
