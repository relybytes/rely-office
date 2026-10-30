// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use super::*;
use crate::chart::read_chart;
use crate::number_format::NumberLocale;
use crate::theme::Theme;

fn spec(chart_type: ChartType) -> ChartSpec {
    ChartSpec {
        chart_type,
        stacked: false,
        title: Some("Vendite <2026> & co".to_owned()),
        legend: true,
        categories: Some("Foglio1!$A$2:$A$4".to_owned()),
        series: vec![
            ChartSeriesSpec {
                name: Some("Foglio1!$B$1".to_owned()),
                values: "Foglio1!$B$2:$B$4".to_owned(),
            },
            ChartSeriesSpec {
                name: None,
                values: "'Il mio foglio'!$C$2:$C$4".to_owned(),
            },
        ],
    }
}

const EVERY_TYPE: [ChartType; 7] = [
    ChartType::Column,
    ChartType::Bar,
    ChartType::Line,
    ChartType::Area,
    ChartType::Pie,
    ChartType::Doughnut,
    ChartType::Scatter,
];

#[test]
fn writes_formulas_absolute_and_quoted_when_needed() {
    let range = CellRange {
        first_row: 1,
        first_column: 27,
        last_row: 9,
        last_column: 27,
    };
    assert_eq!(
        chart_range_formula("Vendite", range),
        "Vendite!$AB$2:$AB$10"
    );
    assert_eq!(
        chart_range_formula("Il mio foglio", range),
        "'Il mio foglio'!$AB$2:$AB$10"
    );
    assert_eq!(chart_range_formula("2026", range), "'2026'!$AB$2:$AB$10");
    assert_eq!(
        chart_range_formula("L'anno", CellRange::default()),
        "'L''anno'!$A$1"
    );
}

#[test]
fn reads_back_every_kind_it_writes_as_drawingml() {
    for chart_type in EVERY_TYPE {
        for stacked in [false, true] {
            let written = ChartSpec {
                stacked,
                ..spec(chart_type)
            };
            let xml = chart_part_xml(&written);
            let read = read_chart_spec(&xml).unwrap_or_else(|| panic!("{chart_type:?} unread"));
            let stackable = matches!(
                chart_type,
                ChartType::Column | ChartType::Bar | ChartType::Line | ChartType::Area
            );
            assert_eq!(
                read,
                ChartSpec {
                    stacked: stacked && stackable,
                    ..written.clone()
                },
                "{chart_type:?}"
            );
            // The chart reader draws what was written.
            let drawn = read_chart(&xml, &Theme::default(), NumberLocale::default(), &|_| None);
            assert!(drawn.is_some(), "{chart_type:?} undrawn");
        }
    }
}

#[test]
fn a_chart_without_title_or_legend_says_so() {
    let written = ChartSpec {
        title: None,
        legend: false,
        categories: None,
        ..spec(ChartType::Column)
    };
    let xml = chart_part_xml(&written);
    assert!(xml.contains(r#"<c:autoTitleDeleted val="1"/>"#));
    assert!(!xml.contains("<c:legend>"));
    assert_eq!(read_chart_spec(&xml), Some(written));
}

#[test]
fn leaves_alone_what_it_cannot_change() {
    assert_eq!(read_chart_spec("not xml"), None);
    assert_eq!(read_chart_spec("<root/>"), None);
    let radar = chart_part_xml(&spec(ChartType::Line)).replace("lineChart", "radarChart");
    assert_eq!(read_chart_spec(&radar), None);
    let two_plots = chart_part_xml(&spec(ChartType::Line)).replace(
        "</c:lineChart>",
        "</c:lineChart><c:barChart><c:ser><c:val><c:numRef><c:f>A!$A$1</c:f></c:numRef></c:val></c:ser></c:barChart>",
    );
    assert_eq!(read_chart_spec(&two_plots), None);
    let literal = chart_part_xml(&spec(ChartType::Column)).replace(
        "<c:val><c:numRef><c:f>Foglio1!$B$2:$B$4</c:f></c:numRef></c:val>",
        "<c:val><c:numLit><c:ptCount val=\"0\"/></c:numLit></c:val>",
    );
    assert_eq!(read_chart_spec(&literal), None);
}

#[test]
fn reads_back_every_kind_it_writes_as_opendocument() {
    for chart_type in EVERY_TYPE {
        let written = ChartSpec {
            stacked: chart_type == ChartType::Area,
            ..spec(chart_type)
        };
        let xml = odf_chart_object_xml(&written);
        let read = read_odf_chart_spec(&xml).unwrap_or_else(|| panic!("{chart_type:?} unread"));
        let expected = ChartSpec {
            series: written
                .series
                .iter()
                .map(|series| ChartSeriesSpec {
                    name: series.name.clone(),
                    values: series.values.clone(),
                })
                .collect(),
            ..written.clone()
        };
        assert_eq!(read, expected, "{chart_type:?}");
    }
}

#[test]
fn reads_the_addresses_libreoffice_writes() {
    assert_eq!(
        formula_of_address("Sheet1.B2:Sheet1.B5"),
        Some("Sheet1!$B$2:$B$5".to_owned())
    );
    assert_eq!(
        formula_of_address("$Sheet1.$B$2:.$B$5"),
        Some("Sheet1!$B$2:$B$5".to_owned())
    );
    assert_eq!(
        formula_of_address("'My Sheet'.A1"),
        Some("'My Sheet'!$A$1".to_owned())
    );
    assert_eq!(formula_of_address("A1:B2"), None);
    assert_eq!(formula_of_address("S.A1 S.B2"), None);
    assert_eq!(formula_of_address("S.A1:S.B2:S.C3"), None);
    assert_eq!(
        odf_address("Vendite!$B$2:$B$5"),
        Some("'Vendite'.$B$2:'Vendite'.$B$5".to_owned())
    );
    assert_eq!(odf_address("nonsense"), None);
}

#[test]
fn spans_the_whole_data_of_a_chart_on_one_sheet() {
    let one_sheet = ChartSpec {
        series: vec![ChartSeriesSpec {
            name: Some("Foglio1!$B$1".to_owned()),
            values: "Foglio1!$B$2:$B$4".to_owned(),
        }],
        ..spec(ChartType::Column)
    };
    assert_eq!(
        plot_range(&one_sheet),
        Some("'Foglio1'.$A$1:'Foglio1'.$B$4".to_owned())
    );
    assert_eq!(plot_range(&spec(ChartType::Column)), None);
}
