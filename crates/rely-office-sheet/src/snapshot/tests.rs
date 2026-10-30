// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use rely_office_core::{
    CellRange, ChartSeriesSpec, ChartSpec, ChartType, ComparisonOperator, Condition,
    ConditionalFormat, ConditionalRule, DataValidation, ErrorStyle, Limits, ValidationKind,
};

use super::*;
use crate::address::CellRef;
use crate::charts::ChartPlacement;
use crate::pivots::{PivotField, PivotFunction, PivotLayout, PivotTable, PivotValue};
use crate::region::Region;
use crate::styling::StylePatch;
use crate::xlsx::read_spreadsheet;

fn cell(address: &str) -> CellRef {
    CellRef::parse(address).unwrap()
}

const PLACEMENT: ChartPlacement = ChartPlacement {
    row: 1,
    column: 6,
    offset_emu: (0, 0),
    width_emu: 3_000_000,
    height_emu: 2_000_000,
};

fn chart(sheet: &str) -> ChartSpec {
    ChartSpec {
        chart_type: ChartType::Column,
        stacked: false,
        title: Some("Vendite".to_owned()),
        legend: true,
        categories: Some(format!("'{sheet}'!$A$2:$A$4")),
        series: vec![ChartSeriesSpec {
            name: Some(format!("'{sheet}'!$B$1")),
            values: format!("'{sheet}'!$B$2:$B$4"),
        }],
    }
}

/// A workbook whose middle sheet holds a bit of everything a sheet can.
#[expect(clippy::too_many_lines, reason = "one of each thing a sheet holds")]
fn workbook() -> Workbook {
    let mut workbook = Workbook::new();
    workbook.add_sheet("Dati 2026").unwrap();
    workbook.add_sheet("Riepilogo").unwrap();
    let inputs: Vec<(CellRef, String)> = [
        ("A1", "Mese"),
        ("B1", "Importo"),
        ("A2", "Gen"),
        ("B2", "10"),
        ("A3", "Feb"),
        ("B3", "25"),
        ("A4", "Gen"),
        ("B4", "=B2*2+Foglio1!A1"),
        ("D1", "=SUM(B2:B4)+Totale"),
    ]
    .iter()
    .map(|(address, typed)| (cell(address), (*typed).to_owned()))
    .collect();
    workbook.set_inputs(1, &inputs, Locale::English).unwrap();
    workbook
        .set_inputs(0, &[(cell("A1"), "3".to_owned())], Locale::English)
        .unwrap();
    workbook
        .set_style(
            1,
            (cell("A1"), cell("B1")),
            &StylePatch {
                bold: Some(true),
                fill: Some("DBEAFE".to_owned()),
                ..StylePatch::default()
            },
        )
        .unwrap();
    workbook.merge_cells(1, (cell("C6"), cell("D7"))).unwrap();
    workbook
        .start_comment_thread(
            1,
            cell("B3"),
            "Anna",
            "Da controllare",
            Some("2026-10-28T09:00:00Z"),
        )
        .unwrap();
    workbook
        .reply_to_comment(
            1,
            cell("B3"),
            "Marco",
            "Fatto",
            Some("2026-10-28T10:00:00Z"),
        )
        .unwrap();
    workbook
        .set_conditional_formats(
            1,
            vec![ConditionalFormat {
                ranges: vec![CellRange {
                    first_row: 1,
                    first_column: 1,
                    last_row: 3,
                    last_column: 1,
                }],
                rules: vec![ConditionalRule {
                    condition: Condition::Expression {
                        formula: "B2>$B$3".to_owned(),
                    },
                    look: None,
                    priority: 0,
                    stop_if_true: false,
                }],
            }],
        )
        .unwrap();
    workbook
        .set_validations(
            1,
            vec![DataValidation {
                ranges: vec![CellRange {
                    first_row: 1,
                    first_column: 1,
                    last_row: 3,
                    last_column: 1,
                }],
                kind: ValidationKind::Decimal,
                formulas: vec!["0".to_owned()],
                operator: ComparisonOperator::GreaterOrEqual,
                allow_blank: true,
                dropdown: false,
                error_style: ErrorStyle::default(),
                show_error: true,
                error_title: None,
                error: None,
                show_prompt: false,
                prompt_title: None,
                prompt: None,
            }],
        )
        .unwrap();
    workbook
        .add_chart(1, &chart("Dati 2026"), PLACEMENT)
        .unwrap();
    let mut table = PivotTable::new(
        "Per mese",
        "Dati 2026",
        CellRange {
            first_row: 0,
            first_column: 0,
            last_row: 3,
            last_column: 1,
        },
        cell("J1"),
    );
    table.locale = Locale::Italian;
    table.layout = PivotLayout::Outline;
    table.rows = vec![PivotField {
        field: 0,
        hidden: Vec::new(),
    }];
    table.values = vec![PivotValue {
        field: 1,
        function: PivotFunction::Sum,
        name: None,
    }];
    workbook.add_pivot_table(1, table).unwrap();
    workbook
        .set_defined_name("Totale", Some(1), "=$B$2", Locale::English)
        .unwrap();
    workbook
        .set_defined_name("Tutto", None, "=Riepilogo!$A$1", Locale::English)
        .unwrap();
    workbook.set_column_width(1, 0, Some(18.5)).unwrap();
    workbook.set_row_height(1, 2, Some(30.0)).unwrap();
    workbook.set_frozen(1, 1, 1).unwrap();
    workbook
}

/// What a sheet holds, to compare one put back with the one taken.
fn contents(workbook: &Workbook, sheet: usize) -> (Region, String) {
    let region = workbook.region(sheet, (cell("A1"), cell("M12"))).unwrap();
    let held = &workbook.sheets()[sheet];
    let tables: Vec<_> = held
        .pivot_tables()
        .iter()
        .map(|table| {
            (
                &table.name,
                table.layout,
                table.locale,
                &table.rows,
                table.anchor,
            )
        })
        .collect();
    let names: Vec<_> = workbook
        .defined_names()
        .iter()
        .filter(|defined| defined.sheet == Some(sheet))
        .map(|defined| defined.name.clone())
        .collect();
    let rest = format!(
        "{:?}",
        (
            &held.name,
            &held.column_widths,
            &held.row_heights,
            (held.frozen_rows, held.frozen_columns),
            held.conditional_formats(),
            held.validations(),
            workbook
                .charts(sheet)
                .iter()
                .map(|chart| (&chart.spec, chart.placement))
                .collect::<Vec<_>>(),
            tables,
            names,
        )
    );
    (region, rest)
}

#[test]
fn a_removed_sheet_comes_back_where_it_was_with_all_it_held() {
    let mut workbook = workbook();
    let before = contents(&workbook, 1);
    let shown = workbook.display(1, cell("B4"), Locale::English);
    let snapshot = workbook.sheet_snapshot(1).unwrap();
    workbook.remove_sheet(1).unwrap();
    assert_eq!(workbook.sheets().len(), 2);
    assert_eq!(workbook.sheets()[1].name, "Riepilogo");

    workbook.restore_sheet(1, &snapshot).unwrap();
    let names: Vec<&str> = workbook
        .sheets()
        .iter()
        .map(|sheet| sheet.name.as_str())
        .collect();
    assert_eq!(names, ["Foglio1", "Dati 2026", "Riepilogo"]);
    assert_eq!(contents(&workbook, 1), before);
    assert_eq!(
        workbook.display(1, cell("B4"), Locale::English),
        shown,
        "its formulas compute again"
    );
    assert_eq!(
        workbook
            .defined_names()
            .iter()
            .find(|defined| defined.name == "Tutto")
            .and_then(|defined| defined.sheet),
        None,
        "a name of the workbook stays the workbook's"
    );
    assert_eq!(workbook.pictures(1).len(), 1, "its chart is drawn");

    let written = write_xlsx(&workbook).unwrap();
    let reopened = read_spreadsheet(
        &written,
        Some("back.xlsx"),
        &Limits::default(),
        Locale::English,
    )
    .unwrap();
    assert_eq!(reopened.charts(1).len(), 1);
    assert_eq!(reopened.sheets()[1].pivot_tables().len(), 1);
}

#[test]
fn a_restored_sheet_takes_new_names_for_parts_another_sheet_took_since() {
    let mut workbook = workbook();
    let snapshot = workbook.sheet_snapshot(1).unwrap();
    workbook.remove_sheet(1).unwrap();
    // The chart added now takes the names the removed sheet's parts had.
    workbook.add_chart(1, &chart("Foglio1"), PLACEMENT).unwrap();
    workbook.restore_sheet(1, &snapshot).unwrap();
    let mut names: Vec<String> = workbook
        .sheets()
        .iter()
        .flat_map(|sheet| {
            sheet
                .drawing
                .iter()
                .flat_map(|drawing| drawing.parts.iter())
        })
        .map(|part| part.name.clone())
        .collect();
    let count = names.len();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), count, "no two sheets share a part: {names:?}");
    let written = write_xlsx(&workbook).unwrap();
    let reopened = read_spreadsheet(
        &written,
        Some("both.xlsx"),
        &Limits::default(),
        Locale::English,
    )
    .unwrap();
    assert_eq!(reopened.charts(1).len(), 1, "the restored sheet's chart");
    assert_eq!(reopened.charts(2).len(), 1, "the chart added meanwhile");
    assert_eq!(
        reopened.charts(1)[0]
            .spec
            .as_ref()
            .map(|spec| spec.title.clone()),
        Some(Some("Vendite".to_owned()))
    );
}

#[test]
fn a_sheet_comes_back_at_the_end_or_not_at_all() {
    let mut workbook = workbook();
    let snapshot = workbook.sheet_snapshot(2).unwrap();
    workbook.remove_sheet(2).unwrap();
    assert_eq!(
        workbook.restore_sheet(5, &snapshot),
        Err(EditError::NoSuchSheet(5))
    );
    workbook.restore_sheet(2, &snapshot).unwrap();
    assert_eq!(workbook.sheets()[2].name, "Riepilogo");
    assert_eq!(
        workbook.restore_sheet(0, &snapshot),
        Err(EditError::DuplicateSheetName("Riepilogo".to_owned())),
        "a sheet of that name is there already"
    );
    let broken = SheetSnapshot {
        xlsx: b"not a workbook".to_vec(),
        pivot_locales: Vec::new(),
        styles: Vec::new(),
        cell_styles: Vec::new(),
        conditional_formats: Vec::new(),
        validations: Vec::new(),
    };
    assert_eq!(
        workbook.restore_sheet(0, &broken),
        Err(EditError::OutOfBounds)
    );
    assert_eq!(workbook.sheet_snapshot(9), Err(EditError::NoSuchSheet(9)));
}

#[test]
fn freshens_a_name_past_those_taken() {
    let taken: HashSet<String> = ["xl/charts/chart1.xml", "xl/charts/chart2.xml"]
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    assert_eq!(
        fresh_name("xl/charts/chart1.xml", &taken),
        "xl/charts/chart3.xml"
    );
    assert_eq!(
        fresh_name("xl/media/image.png", &taken),
        "xl/media/image1.png"
    );
    assert_eq!(
        relationships_of("xl/drawings/drawing1.xml"),
        "xl/drawings/_rels/drawing1.xml.rels"
    );
}
