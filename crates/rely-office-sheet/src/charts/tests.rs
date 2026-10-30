// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use rely_office_core::{ChartSeriesSpec, ChartType, Limits};

use super::*;
use crate::address::CellRef;
use crate::ods::write_ods;
use crate::value::Locale;
use crate::xlsx::{read_spreadsheet, write_xlsx};

fn sales() -> Workbook {
    let mut workbook = Workbook::new();
    let inputs: Vec<(CellRef, String)> = [
        ("A1", "Mese"),
        ("B1", "Vendite"),
        ("A2", "Gen"),
        ("B2", "10"),
        ("A3", "Feb"),
        ("B3", "25"),
        ("A4", "Mar"),
        ("B4", "18"),
    ]
    .iter()
    .map(|(address, typed)| (CellRef::parse(address).unwrap(), (*typed).to_owned()))
    .collect();
    workbook.set_inputs(0, &inputs, Locale::English).unwrap();
    workbook
}

fn spec(chart_type: ChartType) -> ChartSpec {
    ChartSpec {
        chart_type,
        stacked: false,
        title: Some("Vendite del trimestre".to_owned()),
        legend: true,
        categories: Some("Foglio1!$A$2:$A$4".to_owned()),
        series: vec![ChartSeriesSpec {
            name: Some("Foglio1!$B$1".to_owned()),
            values: "Foglio1!$B$2:$B$4".to_owned(),
        }],
    }
}

const PLACEMENT: ChartPlacement = ChartPlacement {
    row: 1,
    column: 3,
    offset_emu: (12_700, 0),
    width_emu: 4_572_000,
    height_emu: 2_743_200,
};

fn reopened(bytes: &[u8], name: &str) -> Workbook {
    read_spreadsheet(bytes, Some(name), &Limits::default(), Locale::English).unwrap()
}

#[test]
fn adds_a_chart_that_is_drawn_and_listed() {
    let mut workbook = sales();
    assert_eq!(workbook.charts(0), Vec::new());
    assert_eq!(
        workbook.add_chart(0, &spec(ChartType::Column), PLACEMENT),
        Ok(0)
    );
    assert_eq!(
        workbook.charts(0),
        vec![SheetChart {
            spec: Some(spec(ChartType::Column)),
            placement: PLACEMENT,
        }]
    );
    let pictures = workbook.pictures(0);
    assert_eq!(pictures.len(), 1);
    assert_eq!(pictures[0].content_type, "image/svg+xml");
    assert_eq!((pictures[0].row, pictures[0].column), (1, 3));
    assert_eq!(
        pictures[0].description.as_deref(),
        Some("Vendite del trimestre")
    );
    // A second chart gets parts of its own.
    assert_eq!(
        workbook.add_chart(0, &spec(ChartType::Pie), PLACEMENT),
        Ok(1)
    );
    assert_eq!(workbook.charts(0).len(), 2);
}

#[test]
fn a_chart_follows_the_cells_it_reads() {
    let mut workbook = sales();
    workbook
        .add_chart(0, &spec(ChartType::Line), PLACEMENT)
        .unwrap();
    let before = workbook.pictures(0)[0].bytes.clone();
    let revision = workbook.revision();
    workbook
        .set_input(0, CellRef::parse("B3").unwrap(), "99", Locale::English)
        .unwrap();
    assert_ne!(workbook.revision(), revision);
    assert_ne!(workbook.pictures(0)[0].bytes, before);
}

#[test]
fn changes_moves_and_removes_a_chart() {
    let mut workbook = sales();
    workbook
        .add_chart(0, &spec(ChartType::Column), PLACEMENT)
        .unwrap();
    workbook
        .add_chart(0, &spec(ChartType::Area), PLACEMENT)
        .unwrap();
    let revision = workbook.revision();
    let changed = ChartSpec {
        chart_type: ChartType::Bar,
        stacked: true,
        title: None,
        ..spec(ChartType::Column)
    };
    workbook.set_chart(0, 0, Some(&changed), None).unwrap();
    assert_ne!(workbook.revision(), revision);
    let moved = ChartPlacement {
        row: 10,
        column: 0,
        offset_emu: (0, 0),
        width_emu: 1_000_000,
        height_emu: 800_000,
    };
    workbook.set_chart(0, 1, None, Some(moved)).unwrap();
    let charts = workbook.charts(0);
    assert_eq!(charts[0].spec.as_ref(), Some(&changed));
    assert_eq!(charts[0].placement, PLACEMENT);
    assert_eq!(charts[1].spec, Some(spec(ChartType::Area)));
    assert_eq!(charts[1].placement, moved);
    workbook.remove_chart(0, 0).unwrap();
    let charts = workbook.charts(0);
    assert_eq!(charts.len(), 1);
    assert_eq!(charts[0].placement, moved);
    let drawing = workbook.sheets[0].drawing.as_ref().unwrap();
    let charts_kept = drawing
        .parts
        .iter()
        .filter(|part| part.content_type == CHART_TYPE)
        .count();
    assert_eq!(charts_kept, 1);
    workbook.remove_chart(0, 0).unwrap();
    assert_eq!(workbook.charts(0), Vec::new());
    assert_eq!(workbook.pictures(0), Vec::new());
}

#[test]
fn refuses_a_chart_it_cannot_make() {
    let mut workbook = sales();
    let no_series = ChartSpec {
        series: Vec::new(),
        ..spec(ChartType::Column)
    };
    assert_eq!(
        workbook.add_chart(0, &no_series, PLACEMENT),
        Err(EditError::InvalidChart)
    );
    let elsewhere = ChartSpec {
        categories: Some("Missing!$A$1:$A$3".to_owned()),
        ..spec(ChartType::Column)
    };
    assert_eq!(
        workbook.add_chart(0, &elsewhere, PLACEMENT),
        Err(EditError::InvalidChart)
    );
    let not_a_range = ChartSpec {
        series: vec![ChartSeriesSpec {
            name: None,
            values: "SUM(A1:A2)".to_owned(),
        }],
        ..spec(ChartType::Column)
    };
    assert_eq!(
        workbook.add_chart(0, &not_a_range, PLACEMENT),
        Err(EditError::InvalidChart)
    );
    let long_title = ChartSpec {
        title: Some("x".repeat(256)),
        ..spec(ChartType::Column)
    };
    assert_eq!(
        workbook.add_chart(0, &long_title, PLACEMENT),
        Err(EditError::InvalidChart)
    );
    let empty = ChartPlacement {
        width_emu: 0,
        ..PLACEMENT
    };
    assert_eq!(
        workbook.add_chart(0, &spec(ChartType::Column), empty),
        Err(EditError::OutOfBounds)
    );
    assert_eq!(
        workbook.add_chart(3, &spec(ChartType::Column), PLACEMENT),
        Err(EditError::NoSuchSheet(3))
    );
    assert_eq!(
        workbook.set_chart(0, 0, None, None),
        Err(EditError::OutOfBounds)
    );
    assert_eq!(workbook.remove_chart(0, 0), Err(EditError::OutOfBounds));
}

#[test]
fn a_chart_survives_xlsx_and_ods_and_stays_editable() {
    let mut workbook = sales();
    workbook
        .add_chart(0, &spec(ChartType::Column), PLACEMENT)
        .unwrap();
    workbook
        .add_chart(
            0,
            &ChartSpec {
                chart_type: ChartType::Scatter,
                legend: false,
                ..spec(ChartType::Scatter)
            },
            ChartPlacement {
                row: 20,
                ..PLACEMENT
            },
        )
        .unwrap();
    let xlsx = reopened(&write_xlsx(&workbook).unwrap(), "a.xlsx");
    assert_eq!(xlsx.charts(0), workbook.charts(0));
    let ods = reopened(&write_ods(&workbook).unwrap(), "a.ods");
    let charts = ods.charts(0);
    assert_eq!(charts.len(), 2);
    assert_eq!(charts[0].spec, Some(spec(ChartType::Column)));
    assert_eq!(
        (charts[0].placement.row, charts[0].placement.column),
        (1, 3)
    );
    assert_eq!(charts[0].placement.width_emu, PLACEMENT.width_emu);
    assert_eq!(
        charts[1].spec.as_ref().map(|spec| spec.chart_type),
        Some(ChartType::Scatter)
    );
    // From the ods the chart goes on to an xlsx as a chart again.
    let again = reopened(&write_xlsx(&ods).unwrap(), "b.xlsx");
    assert_eq!(again.charts(0).len(), 2);
}

#[test]
fn the_pictures_of_an_ods_are_kept_in_its_drawing() {
    let picture = SheetDrawing {
        row: 2,
        column: 1,
        offset_emu: (0, 0),
        width_emu: 952_500,
        height_emu: 952_500,
        content_type: "image/png",
        bytes: vec![0x89, b'P', b'N', b'G'],
        description: Some("Logo".to_owned()),
        chart: None,
    };
    let mut counter = 0;
    let drawing = drawing_of_pictures(std::slice::from_ref(&picture), &mut counter).unwrap();
    assert!(
        drawing
            .parts
            .iter()
            .any(|part| part.name.starts_with("xl/media/image") && part.bytes == picture.bytes)
    );
    let xml = part_text(&drawing, &drawing.drawing_part).unwrap();
    assert!(xml.contains(r#"descr="Logo""#));
    assert!(drawing_of_pictures(&[], &mut counter).is_none());
}

#[test]
fn resolves_and_writes_relationship_targets() {
    assert_eq!(
        resolve("xl/drawings/drawing1.xml", "../charts/chart1.xml"),
        "xl/charts/chart1.xml"
    );
    assert_eq!(
        resolve("xl/drawings/drawing1.xml", "/xl/media/a.png"),
        "xl/media/a.png"
    );
    assert_eq!(
        resolve("xl/drawings/drawing1.xml", "./x.xml"),
        "xl/drawings/x.xml"
    );
    assert_eq!(
        relative("xl/drawings/drawing1.xml", "xl/charts/chart1.xml"),
        "../charts/chart1.xml"
    );
    assert_eq!(
        relative("xl/drawings/drawing1.xml", "xl/drawings/x.xml"),
        "x.xml"
    );
    assert_eq!(
        relationships_name("xl/drawings/drawing1.xml"),
        "xl/drawings/_rels/drawing1.xml.rels"
    );
    assert_eq!(relationships_name("chart.xml"), "_rels/chart.xml.rels");
    let links = with_relationship(None, "rId1", CHART_RELATIONSHIP, "../charts/chart1.xml");
    let links = with_relationship(Some(&links), "rId2", IMAGE_RELATIONSHIP, "../media/a.png");
    assert_eq!(
        relationships(&links),
        vec![
            ("rId1".to_owned(), "../charts/chart1.xml".to_owned()),
            ("rId2".to_owned(), "../media/a.png".to_owned())
        ]
    );
    assert_eq!(
        relationships(&without_relationship(&links, "rId1")).len(),
        1
    );
    assert_eq!(free_relationship(&relationships(&links), "rId"), "rId3");
    assert_eq!(
        largest_shape_id(r#"<xdr:cNvPr id="7" name="a"/><xdr:cNvPr id="3"/>"#),
        7
    );
    assert_eq!(child_span("<a><b/><c><d/></c></a>", "c"), Some((7, 18)));
    assert_eq!(child_span("<a><b/></a>", "c"), None);
}

#[test]
fn makes_a_chart_from_a_selection() {
    use rely_office_core::CellRange;
    let mut workbook = sales();
    workbook
        .set_input(0, CellRef::parse("C1").unwrap(), "Resi", Locale::English)
        .unwrap();
    workbook
        .set_input(0, CellRef::parse("C2").unwrap(), "2", Locale::English)
        .unwrap();
    let table = CellRange {
        first_row: 0,
        first_column: 0,
        last_row: 3,
        last_column: 2,
    };
    let made = workbook
        .chart_for_range(0, table, ChartType::Column)
        .unwrap();
    assert_eq!(made.categories.as_deref(), Some("Foglio1!$A$2:$A$4"));
    assert_eq!(
        made.series,
        vec![
            ChartSeriesSpec {
                name: Some("Foglio1!$B$1".to_owned()),
                values: "Foglio1!$B$2:$B$4".to_owned(),
            },
            ChartSeriesSpec {
                name: Some("Foglio1!$C$1".to_owned()),
                values: "Foglio1!$C$2:$C$4".to_owned(),
            },
        ]
    );
    let pie = workbook.chart_for_range(0, table, ChartType::Pie).unwrap();
    assert_eq!(pie.series.len(), 1);
    // Numbers only: a series for each column, no names, no categories.
    let numbers = CellRange {
        first_row: 1,
        first_column: 1,
        last_row: 3,
        last_column: 1,
    };
    let plain = workbook
        .chart_for_range(0, numbers, ChartType::Line)
        .unwrap();
    assert_eq!(plain.categories, None);
    assert_eq!(plain.series[0].name, None);
    assert_eq!(plain.series[0].values, "Foglio1!$B$2:$B$4");
    // A wide selection makes a series of each row.
    let wide = CellRange {
        first_row: 1,
        first_column: 0,
        last_row: 1,
        last_column: 2,
    };
    let row = workbook.chart_for_range(0, wide, ChartType::Bar).unwrap();
    assert_eq!(row.categories, None);
    assert_eq!(row.series[0].values, "Foglio1!$B$2:$C$2");
    assert_eq!(row.series[0].name.as_deref(), Some("Foglio1!$A$2"));
    let text_only = CellRange {
        first_row: 1,
        first_column: 0,
        last_row: 3,
        last_column: 0,
    };
    assert_eq!(
        workbook.chart_for_range(0, text_only, ChartType::Column),
        None
    );
    assert_eq!(workbook.chart_for_range(5, table, ChartType::Column), None);
}

/// The formulas of the first chart of a sheet: categories, then each series' name and values.
fn chart_formulas(workbook: &Workbook, sheet: usize) -> Vec<Option<String>> {
    let spec = workbook.charts(sheet)[0].spec.clone().unwrap();
    let mut formulas = vec![spec.categories];
    for series in spec.series {
        formulas.push(series.name);
        formulas.push(Some(series.values));
    }
    formulas
}

fn formulas(texts: [&str; 3]) -> Vec<Option<String>> {
    texts.iter().map(|text| Some((*text).to_owned())).collect()
}

#[test]
fn chart_formulas_follow_inserted_and_deleted_rows_and_columns() {
    let mut workbook = sales();
    workbook
        .add_chart(0, &spec(ChartType::Column), PLACEMENT)
        .unwrap();
    workbook.insert_rows(0, 2, 2).unwrap();
    assert_eq!(
        chart_formulas(&workbook, 0),
        formulas(["Foglio1!$A$2:$A$6", "Foglio1!$B$1", "Foglio1!$B$2:$B$6"]),
        "rows inserted inside the range widen it"
    );
    workbook.insert_columns(0, 0, 1).unwrap();
    assert_eq!(
        chart_formulas(&workbook, 0),
        formulas(["Foglio1!$B$2:$B$6", "Foglio1!$C$1", "Foglio1!$C$2:$C$6"])
    );
    workbook.delete_rows(0, 1, 3).unwrap();
    assert_eq!(
        chart_formulas(&workbook, 0),
        formulas(["Foglio1!$B$2:$B$3", "Foglio1!$C$1", "Foglio1!$C$2:$C$3"]),
        "a deletion shrinks the range"
    );
    let drawn = String::from_utf8(workbook.pictures(0)[0].bytes.clone()).unwrap();
    assert!(drawn.contains("Feb") && drawn.contains("Mar") && !drawn.contains("Gen"));
    workbook.delete_columns(0, 2, 1).unwrap();
    let spec = workbook.charts(0)[0].spec.clone();
    assert!(
        spec.is_none_or(|spec| spec.series[0].values == "#REF!"),
        "a deleted column breaks the series"
    );
}

#[test]
fn chart_formulas_follow_a_renamed_or_removed_sheet() {
    let mut workbook = sales();
    let data = workbook.add_sheet("Dati").unwrap();
    workbook
        .set_inputs(
            data,
            &[
                (CellRef::new(0, 0), "1".to_owned()),
                (CellRef::new(1, 0), "2".to_owned()),
            ],
            Locale::English,
        )
        .unwrap();
    let mut chart = spec(ChartType::Line);
    chart.series.push(ChartSeriesSpec {
        name: None,
        values: "Dati!$A$1:$A$2".to_owned(),
    });
    workbook.add_chart(0, &chart, PLACEMENT).unwrap();
    workbook.rename_sheet(0, "Vendite d'oggi").unwrap();
    workbook.rename_sheet(data, "Altri dati").unwrap();
    let spec = workbook.charts(0)[0].spec.clone().unwrap();
    assert_eq!(
        spec.categories.as_deref(),
        Some("'Vendite d''oggi'!$A$2:$A$4")
    );
    assert_eq!(spec.series[1].values, "'Altri dati'!$A$1:$A$2");
    workbook.insert_rows(data, 0, 1).unwrap();
    assert_eq!(
        workbook.charts(0)[0].spec.clone().unwrap().series[1].values,
        "'Altri dati'!$A$2:$A$3",
        "a chart follows the rows of the sheet it plots, not only of its own"
    );
    workbook.remove_sheet(data).unwrap();
    let spec = workbook.charts(0)[0].spec.clone();
    assert!(spec.is_none_or(|spec| spec.series[1].values == "#REF!"));
    assert_eq!(workbook.pictures(0).len(), 1, "the chart is still drawn");
}

#[test]
fn rewrites_only_the_formulas_of_a_chart_part() {
    let xml = "<c:chartSpace><c:tx><c:f>Foglio1!$B$1</c:f></c:tx><c:v>f</c:v><cx:f dir=\"row\">'A &amp; B'!$A$1:$A$2</cx:f><c:f/></c:chartSpace>";
    let shift = crate::edit::Shift::InsertRows { at: 0, count: 1 };
    let moved = rewritten_chart_part(xml, &mut |expression| {
        crate::edit::shifted_expression(expression, false, "a & b", shift)
    })
    .unwrap();
    assert_eq!(
        moved,
        "<c:chartSpace><c:tx><c:f>Foglio1!$B$1</c:f></c:tx><c:v>f</c:v><cx:f dir=\"row\">'A &amp; B'!$A$2:$A$3</cx:f><c:f/></c:chartSpace>"
    );
    assert_eq!(
        rewritten_chart_part(xml, &mut |expression| expression.clone()),
        None
    );
    assert_eq!(unescape("a&#38;b&#x26;c&apos;"), Some("a&b&c'".to_owned()));
    assert_eq!(unescape("broken &amp"), None);
}

#[test]
fn draws_the_numbers_of_a_chart_in_the_locale_asked() {
    let mut workbook = sales();
    let small: Vec<(CellRef, String)> = [(1, "0.3"), (2, "0.7"), (3, "1.1")]
        .iter()
        .map(|(row, typed)| (CellRef::new(*row, 1), (*typed).to_owned()))
        .collect();
    workbook.set_inputs(0, &small, Locale::English).unwrap();
    workbook
        .add_chart(0, &spec(ChartType::Column), PLACEMENT)
        .unwrap();
    let drawn =
        |locale| String::from_utf8(workbook.pictures_in(0, locale)[0].bytes.clone()).unwrap();
    assert!(drawn(Locale::English).contains(">0.2<"));
    assert!(drawn(Locale::Italian).contains(">0,2<"));
    assert_eq!(
        workbook.pictures(0),
        workbook.pictures_in(0, Locale::Italian)
    );
}
