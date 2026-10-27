// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;
use crate::chart::LegendPosition;
use crate::number_format::NumberLocale;

fn series(kind: ExtendedKind, categories: Vec<Vec<&str>>, values: &[f64]) -> ExtendedSeries {
    ExtendedSeries {
        kind,
        name: "Serie".to_owned(),
        categories: categories
            .into_iter()
            .map(|level| level.into_iter().map(str::to_owned).collect())
            .collect(),
        values: values.iter().map(|value| Some(*value)).collect(),
        colour: "4472C4".to_owned(),
        point_colours: Vec::new(),
        labels: true,
        subtotals: Vec::new(),
        binning: Binning::Automatic,
        exclusive_quartiles: true,
        mean_marker: true,
        outliers: true,
        format_code: None,
    }
}

fn chart(series: Vec<ExtendedSeries>) -> ExtendedChart {
    ExtendedChart {
        title: Some("Titolo".to_owned()),
        series,
        legend: Some(LegendPosition::Top),
        locale: NumberLocale::default(),
        label_size: 9.0,
        title_size: 14.0,
        palette: ["4472C4", "ED7D31", "A5A5A5", "FFC000", "5B9BD5", "70AD47"]
            .map(str::to_owned)
            .to_vec(),
    }
}

#[test]
fn draws_a_waterfall_with_its_totals_and_connectors() {
    let mut steps = series(
        ExtendedKind::Waterfall,
        vec![vec!["Inizio", "Su", "Giù", "Fine"]],
        &[100.0, 50.0, -30.0, 120.0],
    );
    steps.subtotals = vec![0, 3];
    steps.point_colours = vec![(1, "00FF00".to_owned())];
    let svg = extended_svg(&chart(vec![steps]), 400.0, 300.0);
    assert!(svg.contains(">Titolo</text>") && svg.contains(">Aumento</text>"));
    assert!(svg.contains("fill=\"#A5A5A5\""), "totals in grey");
    assert!(svg.contains("fill=\"#00FF00\""), "a point's own colour");
    assert!(svg.contains("fill=\"#ED7D31\""), "a fall");
    assert_eq!(svg.matches("stroke-dasharray").count(), 3);
    assert!(svg.contains(">-30</text>") && svg.contains(">Giù</text>"));
}

#[test]
fn draws_a_funnel_a_treemap_and_a_sunburst() {
    let funnel = extended_svg(
        &chart(vec![series(
            ExtendedKind::Funnel,
            vec![vec!["Uno", "Due"]],
            &[10.0, 5.0],
        )]),
        300.0,
        200.0,
    );
    assert!(funnel.contains(">Uno</text>") && funnel.contains(">10</text>"));
    let hierarchy = vec![vec!["a", "b", "c"], vec!["G", "G", "H"]];
    let treemap = extended_svg(
        &chart(vec![series(
            ExtendedKind::Treemap,
            hierarchy.clone(),
            &[6.0, 3.0, 1.0],
        )]),
        300.0,
        200.0,
    );
    assert_eq!(
        treemap.matches("stroke=\"#FFFFFF\"").count(),
        3,
        "a rectangle a leaf"
    );
    assert!(
        treemap.contains(">G</text>") && treemap.contains(">H</text>"),
        "the groups in the legend"
    );
    let sunburst = extended_svg(
        &chart(vec![series(
            ExtendedKind::Sunburst,
            hierarchy,
            &[6.0, 3.0, 1.0],
        )]),
        300.0,
        300.0,
    );
    assert_eq!(
        sunburst.matches("<path").count(),
        5,
        "two groups and three leaves"
    );
    let whole = extended_svg(
        &chart(vec![series(
            ExtendedKind::Sunburst,
            vec![vec!["solo"]],
            &[4.0],
        )]),
        300.0,
        300.0,
    );
    assert_eq!(
        whole.matches("<path").count(),
        2,
        "a whole ring in two halves"
    );
    let empty = extended_svg(
        &chart(vec![series(
            ExtendedKind::Sunburst,
            vec![vec!["x"]],
            &[0.0],
        )]),
        300.0,
        300.0,
    );
    assert!(!empty.contains("<path"));
}

#[test]
fn bins_a_histogram_and_draws_its_pareto_line() {
    let mut values = series(
        ExtendedKind::Histogram,
        Vec::new(),
        &[1.0, 2.0, 2.0, 3.0, 9.0],
    );
    values.binning = Binning::Count(4);
    let bars = histogram_bars(&chart(vec![values.clone()]), &values);
    assert_eq!(
        bars,
        vec![
            ("[1, 3]".to_owned(), 4.0),
            ("(3, 5]".to_owned(), 0.0),
            ("(5, 7]".to_owned(), 0.0),
            ("(7, 9]".to_owned(), 1.0)
        ]
    );
    values.binning = Binning::Width(5.0);
    assert_eq!(
        histogram_bars(&chart(vec![values.clone()]), &values).len(),
        2
    );
    values.binning = Binning::Automatic;
    assert_ne!(
        histogram_bars(&chart(vec![values.clone()]), &values),
        Vec::new()
    );
    let flat = series(ExtendedKind::Histogram, Vec::new(), &[4.0, 4.0]);
    assert_eq!(
        histogram_bars(&chart(vec![flat.clone()]), &flat),
        vec![("[4, 5]".to_owned(), 2.0)]
    );
    let causes = series(
        ExtendedKind::Histogram,
        vec![vec!["a", "b", "a"]],
        &[1.0, 5.0, 2.0],
    );
    let mut line = series(ExtendedKind::ParetoLine, Vec::new(), &[]);
    line.colour = "ED7D31".to_owned();
    let pareto = chart(vec![causes, line]);
    let svg = extended_svg(&pareto, 400.0, 300.0);
    assert!(svg.contains("<polyline") && svg.contains("stroke=\"#ED7D31\""));
    assert!(
        svg.find(">b</text>").unwrap() < svg.find(">a</text>").unwrap(),
        "the largest first"
    );
    assert!(svg.contains(">100%</text>"));
    let none = series(ExtendedKind::Histogram, Vec::new(), &[]);
    assert!(!extended_svg(&chart(vec![none]), 300.0, 200.0).contains("<rect x=\"6\""));
}

#[test]
fn draws_boxes_with_their_whiskers_and_outliers() {
    let values = [2.0, 4.0, 5.0, 6.0, 7.0, 30.0];
    let found = statistics(&values, true).unwrap();
    assert_eq!((found.low, found.high), (2.0, 7.0));
    assert_eq!(found.outliers, vec![30.0]);
    assert!((found.median - 5.5).abs() < 1e-9);
    assert!((found.mean - 9.0).abs() < 1e-9);
    assert!((quantile(&[1.0, 2.0, 3.0, 4.0], 0.25, false) - 1.75).abs() < 1e-9);
    assert!((quantile(&[1.0, 2.0, 3.0, 4.0], 0.25, true) - 1.25).abs() < 1e-9);
    assert_eq!(statistics(&[], true), None);
    let mut boxes = series(
        ExtendedKind::BoxWhisker,
        vec![vec!["A", "A", "A", "A", "A", "A", "B", "B"]],
        &[2.0, 4.0, 5.0, 6.0, 7.0, 30.0, 10.0, 12.0],
    );
    let svg = extended_svg(&chart(vec![boxes.clone()]), 400.0, 300.0);
    assert_eq!(svg.matches("<circle").count(), 1, "one outlier");
    assert!(svg.contains(">A</text>") && svg.contains(">B</text>"));
    boxes.outliers = false;
    boxes.mean_marker = false;
    boxes.categories.clear();
    let plain = extended_svg(&chart(vec![boxes]), 400.0, 300.0);
    assert!(!plain.contains("<circle"));
    assert!(!plain.contains(">A</text>"));
    assert_eq!(lighter("000000", 0.5), "808080");
}

#[test]
fn lays_out_squares_that_fill_the_area() {
    let area = Area {
        x: 0.0,
        y: 0.0,
        width: 60.0,
        height: 40.0,
    };
    let placed = squarify(
        &[
            (0, 6.0),
            (1, 6.0),
            (2, 4.0),
            (3, 3.0),
            (4, 2.0),
            (5, 2.0),
            (6, 1.0),
            (7, 0.0),
        ],
        area,
    );
    assert_eq!(placed.len(), 7, "a point of no weight gets no rectangle");
    let covered: f64 = placed
        .iter()
        .map(|(_, rectangle)| rectangle.width * rectangle.height)
        .sum();
    assert!((covered - 2_400.0).abs() < 1e-6);
    assert_eq!(squarify(&[], area), Vec::new());
}
