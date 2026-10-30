// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use rely_office_core::Limits;

use super::*;
use crate::ods::write_ods;
use crate::pivots::tests::{by_region, sales};
use crate::pivots::{PivotLayout, PivotSubtotals, PivotValue, PivotValuesAxis};
use crate::xlsx::read_spreadsheet;

#[test]
fn a_table_survives_an_ods() {
    let mut workbook = sales();
    let mut table = by_region("F1");
    table.columns = vec![PivotField {
        field: 2,
        hidden: vec!["Feb".to_owned()],
    }];
    table.filters = vec![PivotField {
        field: 1,
        hidden: vec!["Pere".to_owned()],
    }];
    table.values.push(PivotValue {
        field: 3,
        function: PivotFunction::CountNumbers,
        name: None,
    });
    table.row_grand_totals = false;
    workbook.add_pivot_table(0, table).unwrap();
    let written = workbook.sheets()[0].pivot_tables()[0].clone();
    let bytes = write_ods(&workbook).unwrap();
    let read =
        read_spreadsheet(&bytes, Some("a.ods"), &Limits::default(), Locale::English).unwrap();
    let table = &read.sheets()[0].pivot_tables()[0];
    assert_eq!(table.source, written.source);
    assert_eq!(table.anchor, written.anchor);
    assert_eq!(table.area(), written.area());
    assert_eq!(table.rows, written.rows);
    assert_eq!(table.columns, written.columns);
    assert_eq!(table.filters, written.filters);
    assert_eq!(
        table
            .values
            .iter()
            .map(|value| (value.field, value.function))
            .collect::<Vec<_>>(),
        vec![(3, PivotFunction::Sum), (3, PivotFunction::CountNumbers)]
    );
    assert_eq!(
        (table.row_grand_totals, table.column_grand_totals),
        (false, true)
    );
}

#[test]
fn reads_the_addresses_libreoffice_writes() {
    assert_eq!(
        parse_range_address("$'Q1 ''26'.$B$3:.$D$9"),
        Some((
            "Q1 '26".to_owned(),
            CellRange {
                first_row: 2,
                first_column: 1,
                last_row: 8,
                last_column: 3
            }
        ))
    );
    assert_eq!(
        parse_range_address("Foglio1.A1"),
        Some((
            "Foglio1".to_owned(),
            CellRange {
                first_row: 0,
                first_column: 0,
                last_row: 0,
                last_column: 0
            }
        ))
    );
    assert_eq!(parse_range_address("A1:B2"), None);
    assert_eq!(parse_range_address("Foglio1.?"), None);
    assert_eq!(
        range_address("Dati", CellRange::parse("A1:C10").unwrap()),
        "'Dati'.A1:'Dati'.C10"
    );
}

#[test]
fn a_table_whose_fields_are_missing_is_left_out() {
    let mut workbook = sales();
    let pending = PendingPivot {
        name: "x".to_owned(),
        target_sheet: "Foglio1".to_owned(),
        target: CellRange::parse("F1:G4").unwrap(),
        source_sheet: "Foglio1".to_owned(),
        source: CellRange::parse("A1:D7").unwrap(),
        grand_total: "none".to_owned(),
        fields: vec![PendingField {
            name: "Assente".to_owned(),
            orientation: "row".to_owned(),
            ..PendingField::default()
        }],
        data_layout: None,
    };
    place_data_pilots(&mut workbook, vec![pending], Locale::English);
    assert_eq!(workbook.sheets()[0].pivot_tables().len(), 0);
    let selected = PendingPivot {
        name: "y".to_owned(),
        target_sheet: "Foglio1".to_owned(),
        target: CellRange::parse("F1:G4").unwrap(),
        source_sheet: "Foglio1".to_owned(),
        source: CellRange::parse("A1:D7").unwrap(),
        grand_total: "column".to_owned(),
        fields: vec![PendingField {
            name: "regione".to_owned(),
            orientation: "page".to_owned(),
            selected_page: Some("Nord".to_owned()),
            ..PendingField::default()
        }],
        data_layout: None,
    };
    place_data_pilots(&mut workbook, vec![selected], Locale::English);
    let table = &workbook.sheets()[0].pivot_tables()[0];
    assert_eq!(
        table.filters[0].hidden,
        vec!["Centro".to_owned(), "Sud".to_owned()]
    );
    assert!(!table.row_grand_totals && table.column_grand_totals);
}

#[test]
fn the_form_and_the_subtotals_survive_an_ods() {
    let cases = [
        (
            PivotLayout::Tabular,
            PivotSubtotals::Top,
            false,
            PivotLayout::Tabular,
        ),
        (
            PivotLayout::Tabular,
            PivotSubtotals::Off,
            true,
            PivotLayout::Tabular,
        ),
        (
            PivotLayout::Outline,
            PivotSubtotals::Bottom,
            true,
            PivotLayout::Outline,
        ),
        (
            PivotLayout::Outline,
            PivotSubtotals::Off,
            false,
            PivotLayout::Outline,
        ),
        // An ods has no compact form: the table comes back in outline form.
        (
            PivotLayout::Compact,
            PivotSubtotals::Top,
            false,
            PivotLayout::Outline,
        ),
    ];
    for (layout, subtotals, repeat_labels, read_as) in cases {
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
        let bytes = write_ods(&workbook).unwrap();
        let read =
            read_spreadsheet(&bytes, Some("a.ods"), &Limits::default(), Locale::English).unwrap();
        let table = &read.sheets()[0].pivot_tables()[0];
        assert_eq!(
            (table.layout, table.subtotals, table.repeat_labels),
            (read_as, subtotals, repeat_labels),
            "{layout:?} {subtotals:?}"
        );
    }
}

#[test]
fn the_values_along_the_rows_survive_an_ods() {
    for position in [None, Some(0), Some(1)] {
        let mut workbook = sales();
        let mut table = by_region("F1");
        table.layout = PivotLayout::Outline;
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
        let bytes = write_ods(&workbook).unwrap();
        let read =
            read_spreadsheet(&bytes, Some("a.ods"), &Limits::default(), Locale::English).unwrap();
        let table = &read.sheets()[0].pivot_tables()[0];
        assert_eq!(table.values_axis, PivotValuesAxis::Rows, "{position:?}");
        assert_eq!(table.values_position, position);
        assert_eq!(table.rows, written.rows);
        assert_eq!(table.area(), written.area());
    }
}

#[test]
fn several_values_across_stay_across_in_an_ods() {
    let mut workbook = sales();
    let mut table = by_region("F1");
    table.values.push(PivotValue {
        field: 3,
        function: PivotFunction::Count,
        name: None,
    });
    workbook.add_pivot_table(0, table).unwrap();
    let bytes = write_ods(&workbook).unwrap();
    let read =
        read_spreadsheet(&bytes, Some("a.ods"), &Limits::default(), Locale::English).unwrap();
    let table = &read.sheets()[0].pivot_tables()[0];
    assert_eq!(table.values_axis, PivotValuesAxis::Columns);
    assert_eq!(table.values_position, None);
}
