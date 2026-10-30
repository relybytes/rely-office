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
        r#"<pivotField axis="axisRow" showAll="0"><items count="4"><item h="1" x="0"/>"#
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
