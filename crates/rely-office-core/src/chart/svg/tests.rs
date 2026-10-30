// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use super::*;
use crate::chart::DataLabels;
use crate::number_format::NumberLocale;

fn series(name: &str, values: &[Option<f64>], colour: &str) -> Series {
    Series {
        name: name.to_owned(),
        values: values.to_vec(),
        x_values: Vec::new(),
        sizes: Vec::new(),
        colour: colour.to_owned(),
        point_colours: Vec::new(),
        line: true,
        markers: false,
        labels: DataLabels::default(),
        format_code: None,
        label_format: None,
    }
}

fn plot(kind: PlotKind, grouping: Grouping, series: Vec<Series>) -> Plot {
    let points = series
        .iter()
        .map(|series| series.values.len())
        .max()
        .unwrap_or(0);
    Plot {
        kind,
        grouping,
        series,
        categories: (1..=points).map(|number| format!("C{number}")).collect(),
        gap_percent: 100,
        overlap_percent: 0,
        secondary: false,
        vary_colours: false,
        first_slice_degrees: 0,
        high_low_lines: false,
        up_down_bars: None,
    }
}

fn chart(plots: Vec<Plot>) -> Chart {
    Chart {
        title: None,
        plots,
        legend: None,
        primary: ValueAxis {
            gridlines: true,
            ..ValueAxis::default()
        },
        secondary: ValueAxis::default(),
        x_axis: ValueAxis::default(),
        category_axis_hidden: false,
        category_title: None,
        categories_reversed: false,
        locale: NumberLocale::Italian,
        label_size: 9.0,
        title_size: 14.0,
    }
}

fn bars(svg: &str, colour: &str) -> Vec<(f64, f64, f64, f64)> {
    let marker = format!("fill=\"#{colour}\"/>");
    svg.split("<rect ")
        .filter(|rect| rect.contains(&marker))
        .map(|rect| {
            let attribute = |name: &str| -> f64 {
                let start = rect.find(&format!("{name}=\"")).unwrap() + name.len() + 2;
                let end = rect[start..].find('"').unwrap() + start;
                rect[start..end].parse().unwrap()
            };
            (
                attribute("x"),
                attribute("y"),
                attribute("width"),
                attribute("height"),
            )
        })
        .collect()
}

#[test]
fn draws_a_title_and_a_legend_on_each_side() {
    let mut drawn = chart(vec![plot(
        PlotKind::Column,
        Grouping::Standard,
        vec![
            series("Nord & Sud", &[Some(1.0)], "112233"),
            series("Centro", &[Some(2.0)], "445566"),
        ],
    )]);
    drawn.title = Some("Ricavi <2026>".to_owned());
    drawn.legend = Some(LegendPosition::Right);
    let svg = chart_svg(&drawn, 300.0, 200.0);
    assert!(svg.starts_with(r#"<svg xmlns="http://www.w3.org/2000/svg" width="300pt" height="200pt" viewBox="0 0 300 200""#));
    assert!(svg.ends_with("</svg>"));
    assert!(svg.contains(">Ricavi &lt;2026&gt;</text>"));
    assert!(svg.contains(">Nord &amp; Sud</text>"));
    for position in [
        LegendPosition::Left,
        LegendPosition::Top,
        LegendPosition::Bottom,
    ] {
        drawn.legend = Some(position);
        let svg = chart_svg(&drawn, 300.0, 200.0);
        assert!(svg.contains(">Centro</text>"), "{position:?}");
    }
    drawn.legend = Some(LegendPosition::Bottom);
    drawn.plots[0].series = (0..40)
        .map(|index| series(&format!("Serie lunga {index}"), &[Some(1.0)], "123456"))
        .collect();
    let svg = chart_svg(&drawn, 300.0, 200.0);
    assert!(
        svg.matches(">Serie lunga").count() < 40,
        "three lines of legend at most"
    );
    assert!(
        !chart_svg(&drawn, 5.0, 5.0).contains("<line"),
        "too small to draw axes in"
    );
}

#[test]
fn sets_clustered_columns_side_by_side_from_zero() {
    let mut north = series("Nord", &[Some(10.0), Some(-5.0)], "111111");
    north.labels.value = true;
    north.label_format = Some("0.0".to_owned());
    let drawn = chart(vec![plot(
        PlotKind::Column,
        Grouping::Standard,
        vec![north, series("Sud", &[Some(5.0), None], "222222")],
    )]);
    let svg = chart_svg(&drawn, 400.0, 300.0);
    let north = bars(&svg, "111111");
    let south = bars(&svg, "222222");
    assert_eq!(
        (north.len(), south.len()),
        (2, 1),
        "a blank value draws no bar"
    );
    assert!(south[0].0 > north[0].0, "side by side");
    assert!((north[0].2 - south[0].2).abs() < 0.01, "of one width");
    assert!(north[0].3 > south[0].3, "ten is taller than five");
    let zero = north[0].1 + north[0].3;
    assert!(
        (north[1].1 - zero).abs() < 0.01,
        "a negative bar hangs from zero"
    );
    assert!(
        svg.contains(">10,0</text>") && svg.contains(">-5,0</text>"),
        "labels in their own format"
    );
    assert!(svg.contains(">C1</text>"));
}

#[test]
fn stacks_bars_and_scales_a_percent_stack_to_the_whole() {
    let stacked = chart(vec![plot(
        PlotKind::Bar,
        Grouping::Stacked,
        vec![
            series("A", &[Some(1.0)], "111111"),
            series("B", &[Some(3.0)], "222222"),
        ],
    )]);
    let svg = chart_svg(&stacked, 400.0, 300.0);
    let (first, second) = (bars(&svg, "111111")[0], bars(&svg, "222222")[0]);
    assert!(
        (second.0 - (first.0 + first.2)).abs() < 0.01,
        "the second starts where the first ends"
    );
    assert!((second.2 - first.2 * 3.0).abs() < 0.05);
    assert!((first.1 - second.1).abs() < 0.01, "in one row");

    let percent = chart(vec![plot(
        PlotKind::Column,
        Grouping::Percent,
        vec![
            series("A", &[Some(1.0), Some(2.0)], "111111"),
            series("B", &[Some(3.0), Some(-2.0)], "222222"),
        ],
    )]);
    let svg = chart_svg(&percent, 400.0, 300.0);
    assert!(
        svg.contains(">100%</text>") && svg.contains(">0%</text>"),
        "{svg}"
    );
    assert_eq!(bars(&svg, "222222").len(), 2);
}

#[test]
fn draws_lines_with_gaps_markers_and_stacked_areas() {
    let mut line = series("L", &[Some(1.0), None, Some(3.0), Some(4.0)], "111111");
    line.markers = true;
    line.labels.value = true;
    let drawn = chart(vec![plot(PlotKind::Line, Grouping::Standard, vec![line])]);
    let svg = chart_svg(&drawn, 400.0, 300.0);
    let path = svg.split("<path d=\"").nth(1).unwrap();
    assert_eq!(
        path.split('"').next().unwrap().matches('M').count(),
        2,
        "a blank point breaks the line"
    );
    assert_eq!(svg.matches("<circle").count(), 3);
    assert!(svg.contains(">4</text>"));

    let mut hidden = series("H", &[Some(1.0), Some(2.0)], "333333");
    hidden.line = false;
    let drawn = chart(vec![plot(
        PlotKind::Line,
        Grouping::Stacked,
        vec![series("A", &[Some(1.0), Some(2.0)], "111111"), hidden],
    )]);
    let svg = chart_svg(&drawn, 400.0, 300.0);
    assert!(
        !svg.contains("stroke=\"#333333\""),
        "a series without a line draws none"
    );

    let areas = chart(vec![plot(
        PlotKind::Area,
        Grouping::Stacked,
        vec![
            series("A", &[Some(1.0), Some(2.0)], "111111"),
            series("B", &[Some(1.0), Some(1.0)], "222222"),
        ],
    )]);
    let svg = chart_svg(&areas, 400.0, 300.0);
    assert!(svg.contains("fill=\"#111111\"/>") && svg.contains("fill=\"#222222\"/>"));
    let percent = chart(vec![plot(
        PlotKind::Area,
        Grouping::Percent,
        vec![series("A", &[Some(1.0), Some(0.0)], "111111")],
    )]);
    assert!(chart_svg(&percent, 400.0, 300.0).contains(">100%</text>"));
    let standard = chart(vec![plot(
        PlotKind::Area,
        Grouping::Standard,
        vec![series("A", &[Some(2.0), None], "111111")],
    )]);
    assert!(chart_svg(&standard, 400.0, 300.0).contains("fill=\"#111111\"/>"));
}

#[test]
fn puts_a_plot_on_the_secondary_axis_and_honours_hidden_axes() {
    let mut second = plot(
        PlotKind::Line,
        Grouping::Standard,
        vec![series("Quota", &[Some(0.1), Some(0.3)], "222222")],
    );
    second.secondary = true;
    let mut drawn = chart(vec![
        plot(
            PlotKind::Column,
            Grouping::Standard,
            vec![series("Ricavi", &[Some(500.0), Some(800.0)], "111111")],
        ),
        second,
    ]);
    drawn.secondary.format_code = Some("0%".to_owned());
    let svg = chart_svg(&drawn, 400.0, 300.0);
    assert!(svg.contains(">800</text>"), "{svg}");
    assert!(
        svg.contains(">30%</text>"),
        "the secondary axis in its own format"
    );
    drawn.secondary.hidden = true;
    drawn.primary.hidden = true;
    drawn.category_axis_hidden = true;
    let svg = chart_svg(&drawn, 400.0, 300.0);
    assert!(
        !svg.contains(">30%</text>") && !svg.contains(">800</text>") && !svg.contains(">C1</text>")
    );

    let mut reversed = chart(vec![plot(
        PlotKind::Bar,
        Grouping::Standard,
        vec![series("A", &[Some(1.0), Some(2.0)], "111111")],
    )]);
    reversed.categories_reversed = true;
    reversed.primary.title = Some("Euro".to_owned());
    reversed.category_title = Some("Mese".to_owned());
    let svg = chart_svg(&reversed, 400.0, 300.0);
    let rows = bars(&svg, "111111");
    assert!(rows[0].1 < rows[1].1, "reversed bars read from the top");
    assert!(
        svg.contains("rotate(-90") && svg.contains(">Mese</text>") && svg.contains(">Euro</text>")
    );
}

#[test]
fn draws_pies_doughnuts_and_their_labels() {
    let mut quote = series("Quote", &[Some(1.0), Some(3.0), Some(-1.0), None], "111111");
    quote.point_colours = vec![(0, "AA0000".to_owned()), (1, "00AA00".to_owned())];
    quote.labels = DataLabels {
        value: true,
        percent: true,
    };
    let mut pie = plot(PlotKind::Pie, Grouping::Standard, vec![quote]);
    pie.vary_colours = true;
    let mut drawn = chart(vec![pie]);
    drawn.legend = Some(LegendPosition::Right);
    let svg = chart_svg(&drawn, 400.0, 300.0);
    assert_eq!(
        svg.matches("<path").count(),
        2,
        "only positive values make slices"
    );
    assert!(svg.contains("fill=\"#AA0000\"") && svg.contains("fill=\"#00AA00\""));
    assert!(
        svg.contains(">1; 25%</text>") && svg.contains(">3; 75%</text>"),
        "{svg}"
    );
    assert!(svg.contains(">C2</text>"), "the legend names the points");

    drawn.plots[0].kind = PlotKind::Doughnut { hole_percent: 50 };
    drawn.plots[0].series[0].values = vec![Some(5.0)];
    let svg = chart_svg(&drawn, 400.0, 300.0);
    assert_eq!(
        svg.matches("<circle").count(),
        2,
        "a whole ring is two circles"
    );
    drawn.plots[0].series[0].values = vec![Some(1.0), Some(1.0)];
    assert!(
        chart_svg(&drawn, 400.0, 300.0).contains(" 0 0 0 "),
        "the inner arc runs back"
    );
    drawn.plots[0].series[0].values = vec![Some(0.0)];
    assert!(
        !chart_svg(&drawn, 400.0, 300.0).contains("<path"),
        "nothing to share out"
    );
    drawn.plots[0].series.clear();
    assert!(!chart_svg(&drawn, 400.0, 300.0).contains("<path"));
}

#[test]
fn places_scatter_points_on_two_value_axes() {
    let mut points = series("P", &[Some(2.0), Some(4.0), None], "111111");
    points.x_values = vec![Some(10.0), Some(20.0), Some(30.0)];
    points.markers = true;
    let mut drawn = chart(vec![plot(
        PlotKind::Scatter,
        Grouping::Standard,
        vec![points, series("N", &[Some(1.0)], "222222")],
    )]);
    drawn.x_axis.gridlines = true;
    drawn.x_axis.title = Some("Tempo".to_owned());
    let svg = chart_svg(&drawn, 400.0, 300.0);
    assert!(
        svg.contains(">20</text>") && svg.contains(">Tempo</text>"),
        "{svg}"
    );
    assert_eq!(
        svg.matches("fill=\"#111111\"").count(),
        2,
        "markers on the points with both values"
    );
    drawn.plots[0].series[0].line = false;
    drawn.plots[0].series[0].markers = false;
    assert_eq!(
        chart_svg(&drawn, 400.0, 300.0)
            .matches("fill=\"#111111\"")
            .count(),
        2,
        "a series with no line shows its markers"
    );
    drawn.plots[0].series = vec![series("Vuota", &[None], "111111")];
    assert!(!chart_svg(&drawn, 400.0, 300.0).contains("<circle"));
}

#[test]
fn picks_round_scales() {
    let axis = ValueAxis::default();
    assert_eq!(
        scale_of(0.0, 28.6, &axis, true),
        Scale {
            minimum: 0.0,
            maximum: 30.0,
            step: 5.0
        }
    );
    assert_eq!(
        scale_of(100.0, 110.0, &axis, false),
        Scale {
            minimum: 100.0,
            maximum: 110.0,
            step: 2.0
        }
    );
    assert_eq!(scale_of(-8.0, -7.0, &axis, false).maximum, -7.0);
    assert_eq!(
        scale_of(0.0, 0.0, &axis, true),
        Scale {
            minimum: 0.0,
            maximum: 1.0,
            step: 0.2
        }
    );
    assert_eq!(scale_of(-3.0, -3.0, &axis, true).maximum, 0.0);
    let fixed = ValueAxis {
        minimum: Some(-10.0),
        maximum: Some(10.0),
        major_unit: Some(0.01),
        ..ValueAxis::default()
    };
    let scale = scale_of(0.0, 5.0, &fixed, true);
    assert_eq!((scale.minimum, scale.maximum), (-10.0, 10.0));
    assert!(
        scale.ticks().len() <= 22,
        "too many ticks fall back to fewer"
    );
    assert!((scale.baseline()).abs() < f64::EPSILON);
    assert_eq!(
        Scale {
            minimum: 1.0,
            maximum: 1.0,
            step: 1.0
        }
        .fraction(1.0),
        0.0
    );
    assert_eq!(nice(0.0), 1.0);
    assert_eq!(nice(f64::NAN), 1.0);
    assert_eq!(nice(0.034), 0.05);
    assert_eq!(nice(8.0), 10.0);
}

#[test]
fn writes_numbers_text_and_cut_labels() {
    assert_eq!(number(1.0), "1");
    assert_eq!(number(1.256), "1.26");
    assert_eq!(number(-0.001), "0");
    assert_eq!(escape("a\"b\u{7}<"), "a&quot;b&lt;");
    assert_eq!(fit("Fatturato", 100.0, 9.0), "Fatturato");
    assert_eq!(fit("Fatturato annuale", 30.0, 9.0), "Fattu\u{2026}");
    let mut svg = String::new();
    text(&mut svg, 0.0, 0.0, 9.0, "start", "");
    assert!(svg.is_empty(), "an empty label writes nothing");
}

/// The outline of the first path drawn in `colour`.
fn path_in(svg: &str, colour: &str) -> Option<String> {
    svg.split("<path d=\"").skip(1).find_map(|part| {
        let element = &part[..part.find("/>")?];
        let outline = &element[..element.find('"')?];
        element
            .contains(&format!("#{colour}\""))
            .then(|| outline.to_owned())
    })
}

#[test]
fn draws_radars_and_bubbles() {
    let mut radar = chart(vec![plot(
        PlotKind::Radar { filled: false },
        Grouping::Standard,
        vec![
            series("A", &[Some(1.0), Some(2.0), Some(3.0)], "111111"),
            series("B", &[Some(2.0), None, Some(1.0)], "222222"),
        ],
    )]);
    radar.primary.gridlines = true;
    let svg = chart_svg(&radar, 400.0, 300.0);
    assert!(
        svg.contains(">C1</text>") && svg.contains(">C3</text>"),
        "{svg}"
    );
    assert!(
        path_in(&svg, "111111").unwrap().ends_with('Z'),
        "a closed outline"
    );
    assert!(
        !path_in(&svg, "222222").unwrap().ends_with('Z'),
        "a gap leaves it open"
    );
    assert_eq!(svg.matches("<line").count(), 3, "a spoke per category");
    assert!(
        svg.matches(&format!("stroke=\"{GRID_COLOUR}\"")).count() >= 3,
        "the rings"
    );
    radar.plots[0].kind = PlotKind::Radar { filled: true };
    let svg = chart_svg(&radar, 400.0, 300.0);
    assert!(svg.contains("fill=\"#111111\" stroke=\"none\""), "{svg}");

    let mut bubbles = series("P", &[Some(5.0), Some(6.0), Some(7.0)], "333333");
    bubbles.x_values = vec![Some(1.0), Some(2.0), Some(3.0)];
    bubbles.sizes = vec![Some(1.0), Some(4.0), Some(-2.0)];
    bubbles.line = false;
    let svg = chart_svg(
        &chart(vec![plot(
            PlotKind::Bubble,
            Grouping::Standard,
            vec![bubbles],
        )]),
        400.0,
        300.0,
    );
    let radii: Vec<f64> = svg
        .split("<circle ")
        .skip(1)
        .filter(|circle| circle.contains("#333333"))
        .filter_map(|circle| {
            let start = circle.find(" r=\"")? + 4;
            circle[start..start + circle[start..].find('"')?]
                .parse()
                .ok()
        })
        .collect();
    assert_eq!(radii.len(), 2, "no bubble for a size below zero");
    assert!(
        (radii[1] / radii[0] - 2.0).abs() < 0.01,
        "sized by area: {radii:?}"
    );
}

#[test]
fn draws_high_low_lines_up_down_bars_and_the_close_tick() {
    let marks = |values: &[Option<f64>], colour: &str| {
        let mut series = series(colour, values, colour);
        series.line = false;
        series
    };
    let mut stock = plot(
        PlotKind::Line,
        Grouping::Standard,
        vec![
            marks(&[Some(5.0), Some(6.0)], "111111"),
            marks(&[Some(2.0), Some(3.0)], "222222"),
            marks(&[Some(4.0), None], "333333"),
        ],
    );
    stock.high_low_lines = true;
    let svg = chart_svg(&chart(vec![stock.clone()]), 400.0, 300.0);
    let stock_lines = svg.matches(&format!("stroke=\"{STOCK_COLOUR}\"")).count();
    assert_eq!(
        stock_lines, 3,
        "a line per category and a tick for the close: {svg}"
    );
    assert!(svg.contains("stroke-width=\"1.5\""));

    stock.series.truncate(2);
    stock.series[1].values = vec![Some(7.0), Some(1.0)];
    stock.up_down_bars = Some(("FFFFFF".to_owned(), "000000".to_owned()));
    let svg = chart_svg(&chart(vec![stock]), 400.0, 300.0);
    assert!(svg.contains("<rect") && svg.contains("fill=\"#FFFFFF\" stroke=\"#404040\""));
    assert!(
        svg.contains("fill=\"#000000\" stroke=\"#404040\""),
        "a fall: {svg}"
    );
    assert!(
        !svg.contains("stroke-width=\"1.5\""),
        "no tick beside a bar"
    );
}
