// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use std::fmt::Write as _;

use super::*;

const NS: &str = concat!(
    r#"xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" "#,
    r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main""#
);

fn chart_space(chart: &str, text: &str) -> String {
    format!(r"<c:chartSpace {NS}><c:chart>{chart}</c:chart>{text}</c:chartSpace>")
}

fn numbers_cache(values: &[&str]) -> String {
    let mut points = String::new();
    for (index, value) in values
        .iter()
        .enumerate()
        .filter(|(_, value)| !value.is_empty())
    {
        let _ = write!(points, r#"<c:pt idx="{index}"><c:v>{value}</c:v></c:pt>"#);
    }
    format!(
        r#"<c:numRef><c:f>Foglio1!$B$2:$B$4</c:f><c:numCache><c:formatCode>#,##0</c:formatCode><c:ptCount val="{}"/>{points}</c:numCache></c:numRef>"#,
        values.len()
    )
}

fn labels_cache(labels: &[&str]) -> String {
    let mut points = String::new();
    for (index, label) in labels.iter().enumerate() {
        let _ = write!(points, r#"<c:pt idx="{index}"><c:v>{label}</c:v></c:pt>"#);
    }
    format!(r"<c:strRef><c:f>Foglio1!$A$2:$A$4</c:f><c:strCache>{points}</c:strCache></c:strRef>")
}

fn series(index: usize, name: &str, values: &[&str], extra: &str) -> String {
    format!(
        concat!(
            r#"<c:ser><c:idx val="{0}"/><c:tx><c:strRef><c:strCache><c:pt idx="0"><c:v>{1}</c:v></c:pt></c:strCache></c:strRef></c:tx>"#,
            "{3}<c:cat>{4}</c:cat><c:val>{2}</c:val></c:ser>"
        ),
        index,
        name,
        numbers_cache(values),
        extra,
        labels_cache(&["Gen", "Feb", "Mar"])
    )
}

const AXES: &str = concat!(
    r#"<c:catAx><c:axId val="1"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/>"#,
    r#"<c:title><c:tx><c:rich><a:p><a:r><a:t>Mese</a:t></a:r></a:p></c:rich></c:tx></c:title></c:catAx>"#,
    r#"<c:valAx><c:axId val="2"/><c:scaling><c:orientation val="minMax"/><c:max val="500"/><c:min val="0"/></c:scaling>"#,
    r#"<c:delete val="0"/><c:majorGridlines/><c:numFmt formatCode="0.0" sourceLinked="0"/><c:majorUnit val="100"/>"#,
    r#"<c:crossBetween val="between"/></c:valAx>"#
);

fn read(xml: &str) -> Chart {
    read_chart(xml, &Theme::default(), NumberLocale::Italian, &no_cells).unwrap()
}

#[test]
fn reads_a_column_chart_with_its_series_axes_and_legend() {
    let xml = chart_space(
        &format!(
            concat!(
                r#"<c:title><c:tx><c:rich><a:p><a:r><a:rPr sz="1800"/><a:t>Ricavi</a:t></a:r><a:r><a:t> 2026</a:t></a:r></a:p></c:rich></c:tx></c:title>"#,
                r#"<c:autoTitleDeleted val="0"/><c:plotArea><c:barChart><c:barDir val="col"/><c:grouping val="clustered"/>"#,
                "{}{}",
                r#"<c:gapWidth val="219"/><c:overlap val="-27"/><c:axId val="1"/><c:axId val="2"/></c:barChart>{}</c:plotArea>"#,
                r#"<c:legend><c:legendPos val="b"/></c:legend>"#
            ),
            series(
                0,
                "Nord",
                &["120", "", "150"],
                r#"<c:spPr><a:solidFill><a:srgbClr val="112233"/></a:solidFill></c:spPr><c:dLbls><c:numFmt formatCode="0%" sourceLinked="0"/><c:showVal val="1"/></c:dLbls>"#
            ),
            series(1, "Sud", &["60", "72", "81"], ""),
            AXES
        ),
        r#"<c:txPr><a:bodyPr/><a:p><a:pPr><a:defRPr sz="1000"/></a:pPr></a:p></c:txPr>"#,
    );
    let chart = read(&xml);
    assert_eq!(chart.title.as_deref(), Some("Ricavi 2026"));
    assert!((chart.title_size - 18.0).abs() < f64::EPSILON);
    assert!((chart.label_size - 10.0).abs() < f64::EPSILON);
    assert_eq!(chart.legend, Some(LegendPosition::Bottom));
    assert_eq!(chart.category_title.as_deref(), Some("Mese"));
    let plot = &chart.plots[0];
    assert_eq!(
        (plot.kind, plot.grouping),
        (PlotKind::Column, Grouping::Standard)
    );
    assert_eq!((plot.gap_percent, plot.overlap_percent), (219, -27));
    assert_eq!(plot.categories, ["Gen", "Feb", "Mar"]);
    let north = &plot.series[0];
    assert_eq!(north.name, "Nord");
    assert_eq!(north.values, [Some(120.0), None, Some(150.0)]);
    assert_eq!(north.colour, "112233");
    assert_eq!(north.format_code.as_deref(), Some("#,##0"));
    assert!(north.labels.value);
    assert_eq!(north.label_format.as_deref(), Some("0%"));
    assert_eq!(plot.series[1].colour, "ED7D31", "the second accent");
    assert_eq!(
        (
            chart.primary.minimum,
            chart.primary.maximum,
            chart.primary.major_unit
        ),
        (Some(0.0), Some(500.0), Some(100.0))
    );
    assert_eq!(chart.primary.format_code.as_deref(), Some("0.0"));
    assert_eq!(
        (chart.primary.gridlines, chart.primary.between),
        (true, Some(true))
    );
    assert!(!chart.plots[0].secondary);
}

#[test]
fn reads_stacked_horizontal_bars_lines_and_a_secondary_axis() {
    let xml = chart_space(
        &format!(
            concat!(
                r#"<c:autoTitleDeleted val="1"/><c:title/><c:plotArea>"#,
                r#"<c:barChart><c:barDir val="bar"/><c:grouping val="percentStacked"/>{}<c:axId val="3"/><c:axId val="2"/></c:barChart>"#,
                r#"<c:lineChart><c:grouping val="stacked"/><c:marker val="1"/>{}{}<c:axId val="1"/><c:axId val="4"/></c:lineChart>"#,
                r#"{}<c:catAx><c:axId val="3"/><c:scaling><c:orientation val="maxMin"/></c:scaling><c:delete val="1"/></c:catAx>"#,
                r#"<c:valAx><c:axId val="4"/><c:scaling/><c:delete val="1"/><c:axPos val="r"/></c:valAx></c:plotArea>"#
            ),
            series(0, "A", &["1", "2", "3"], ""),
            series(
                1,
                "B",
                &["4", "5", "6"],
                r#"<c:marker><c:symbol val="none"/></c:marker>"#
            ),
            series(
                2,
                "C",
                &["7", "8", "9"],
                r"<c:spPr><a:ln><a:noFill/></a:ln></c:spPr>"
            ),
            AXES
        ),
        "",
    );
    let chart = read(&xml);
    assert_eq!(chart.title, None, "the title was deleted");
    assert_eq!(chart.legend, None);
    assert_eq!(chart.plots.len(), 2);
    assert_eq!(
        (chart.plots[0].kind, chart.plots[0].grouping),
        (PlotKind::Bar, Grouping::Percent)
    );
    assert_eq!(
        chart.plots[0].overlap_percent, 100,
        "stacked bars overlap whole"
    );
    let lines = &chart.plots[1];
    assert_eq!(
        (lines.kind, lines.grouping),
        (PlotKind::Line, Grouping::Stacked)
    );
    assert!(lines.secondary);
    assert!(!lines.series[0].markers, "its symbol is none");
    assert!(
        lines.series[1].markers && !lines.series[1].line,
        "markers only"
    );
    assert!(chart.secondary.hidden);
    assert!(chart.categories_reversed);
    assert!(chart.category_axis_hidden);
}

#[test]
fn reads_pies_doughnuts_and_scatter_charts() {
    let pie = chart_space(
        &format!(
            concat!(
                r#"<c:plotArea><c:doughnutChart><c:varyColors val="1"/>{}<c:firstSliceAng val="390"/><c:holeSize val="75"/></c:doughnutChart></c:plotArea>"#,
                r#"<c:legend/>"#
            ),
            series(
                0,
                "Quote",
                &["1", "2", "3"],
                r#"<c:dPt><c:idx val="1"/><c:spPr><a:solidFill><a:srgbClr val="00FF00"/></a:solidFill></c:spPr></c:dPt><c:dLbls><c:showPercent val="1"/></c:dLbls>"#
            )
        ),
        "",
    );
    let chart = read(&pie);
    assert_eq!(chart.title.as_deref(), None);
    assert_eq!(chart.legend, Some(LegendPosition::Right));
    let plot = &chart.plots[0];
    assert_eq!(plot.kind, PlotKind::Doughnut { hole_percent: 75 });
    assert_eq!(plot.first_slice_degrees, 30);
    let mut colours = plot.series[0].point_colours.clone();
    colours.sort();
    assert_eq!(
        colours,
        [
            (0, "4472C4".to_owned()),
            (1, "00FF00".to_owned()),
            (2, "A5A5A5".to_owned())
        ],
        "each point its accent unless it names its own"
    );
    assert!(plot.series[0].labels.percent);

    let scatter = chart_space(
        concat!(
            r#"<c:title><c:tx><c:strRef><c:strCache><c:pt idx="0"><c:v>Misure</c:v></c:pt></c:strCache></c:strRef></c:tx></c:title>"#,
            r#"<c:plotArea><c:scatterChart><c:scatterStyle val="marker"/><c:ser><c:idx val="0"/>"#,
            r#"<c:xVal><c:numLit><c:ptCount val="2"/><c:pt idx="0"><c:v>1</c:v></c:pt><c:pt idx="1"><c:v>2</c:v></c:pt></c:numLit></c:xVal>"#,
            r#"<c:yVal><c:numLit><c:ptCount val="2"/><c:pt idx="0"><c:v>5</c:v></c:pt><c:pt idx="1"><c:v>x</c:v></c:pt></c:numLit></c:yVal></c:ser>"#,
            r#"<c:axId val="1"/><c:axId val="2"/></c:scatterChart>"#,
            r#"<c:valAx><c:axId val="1"/><c:scaling><c:min val="-1"/></c:scaling><c:axPos val="b"/>"#,
            r#"<c:title><c:tx><c:rich><a:p><a:r><a:t>Tempo</a:t></a:r></a:p></c:rich></c:tx></c:title></c:valAx>"#,
            r#"<c:valAx><c:axId val="2"/><c:scaling/><c:axPos val="l"/></c:valAx></c:plotArea>"#
        ),
        "",
    );
    let chart = read(&scatter);
    assert_eq!(chart.title.as_deref(), Some("Misure"));
    let series = &chart.plots[0].series[0];
    assert_eq!(series.name, "Serie 1");
    assert_eq!(series.x_values, [Some(1.0), Some(2.0)]);
    assert_eq!(series.values, [Some(5.0), None]);
    assert!(
        series.markers && !series.line,
        "a marker-only scatter style"
    );
    assert_eq!(chart.x_axis.minimum, Some(-1.0));
    assert_eq!(chart.x_axis.title.as_deref(), Some("Tempo"));
    assert_eq!(chart.plots[0].categories, ["1", "2"]);
}

#[test]
fn takes_uncached_values_from_the_workbook() {
    let xml = chart_space(
        concat!(
            r#"<c:plotArea><c:lineChart><c:ser><c:idx val="4"/><c:tx><c:strRef><c:f>'Vendite'!$B$1</c:f></c:strRef></c:tx>"#,
            r#"<c:cat><c:numRef><c:f>'Vendite'!$A$2:$A$4</c:f></c:numRef></c:cat>"#,
            r#"<c:val><c:numRef><c:f>'Vendite'!$B$2:$B$4</c:f></c:numRef></c:val></c:ser></c:lineChart></c:plotArea>"#
        ),
        "",
    );
    let cells = |formula: &str| -> Option<Vec<ChartCell>> {
        match formula {
            "'Vendite'!$B$1" => Some(vec![ChartCell::Text("Nord".to_owned())]),
            "'Vendite'!$A$2:$A$4" => Some(vec![
                ChartCell::Number(2024.0),
                ChartCell::Text("2025".to_owned()),
                ChartCell::Empty,
            ]),
            "'Vendite'!$B$2:$B$4" => Some(vec![
                ChartCell::Number(1.5),
                ChartCell::Text("n/d".to_owned()),
                ChartCell::Number(3.0),
            ]),
            _ => None,
        }
    };
    let chart = read_chart(&xml, &Theme::default(), NumberLocale::Italian, &cells).unwrap();
    let plot = &chart.plots[0];
    assert_eq!(plot.series[0].name, "Nord");
    assert_eq!(plot.series[0].values, [Some(1.5), None, Some(3.0)]);
    assert_eq!(plot.categories, ["2024", "2025", ""]);
    assert_eq!(plot.series[0].colour, accent(&Theme::default(), 4));
    let named_by_number = |formula: &str| -> Option<Vec<ChartCell>> {
        (formula == "'Vendite'!$B$1").then(|| vec![ChartCell::Number(7.0)])
    };
    let chart = read_chart(
        &xml,
        &Theme::default(),
        NumberLocale::Italian,
        &named_by_number,
    )
    .unwrap();
    assert_eq!(chart.plots[0].series[0].name, "7");
    assert_eq!(chart.plots[0].series[0].values, Vec::<Option<f64>>::new());
}

#[test]
fn draws_nothing_for_what_it_cannot_read() {
    let read = |xml: &str| read_chart(xml, &Theme::default(), NumberLocale::Italian, &no_cells);
    assert!(read("<broken").is_none());
    assert!(read(&chart_space("", "")).is_none(), "no plot area");
    assert!(
        read(&chart_space("<c:plotArea><c:radarChart/></c:plotArea>", "")).is_none(),
        "a radar chart is not drawn"
    );
    assert!(
        read(&chart_space("<c:plotArea><c:barChart/></c:plotArea>", "")).is_none(),
        "a plot without series"
    );
    assert!(
        chart_picture(
            "<broken",
            &Theme::default(),
            100,
            100,
            &no_cells,
            NumberLocale::Italian
        )
        .is_none()
    );
    assert_eq!(no_cells("A1"), None);
}

#[test]
fn draws_a_chart_into_its_frame() {
    let xml = chart_space(
        &format!(
            "<c:plotArea><c:barChart>{}</c:barChart>{AXES}</c:plotArea>",
            series(0, "Nord", &["1", "2", "3"], "")
        ),
        "",
    );
    let (svg, title) = chart_picture(
        &xml,
        &Theme::default(),
        1_270_000,
        635_000,
        &no_cells,
        NumberLocale::Italian,
    )
    .unwrap();
    let svg = String::from_utf8(svg).unwrap();
    assert!(
        svg.starts_with(r#"<svg xmlns="http://www.w3.org/2000/svg" width="100pt" height="50pt""#),
        "{svg}"
    );
    assert_eq!(title, None, "a chart without a title element shows none");
}

#[test]
fn darkens_and_lightens_the_accents_past_the_sixth() {
    let theme = Theme::default();
    assert_eq!(accent(&theme, 0), "4472C4");
    assert_eq!(accent(&theme, 6), "294476", "darker");
    assert_eq!(accent(&theme, 12), "5FA0FF", "lighter");
}

#[test]
fn reads_radar_and_bubble_charts() {
    let filled = chart_space(
        &format!(
            r#"<c:plotArea><c:radarChart><c:radarStyle val="filled"/>{}</c:radarChart></c:plotArea>"#,
            series(0, "Q1", &["1", "2", "3"], "")
        ),
        "",
    );
    assert_eq!(
        read(&filled).plots[0].kind,
        PlotKind::Radar { filled: true }
    );
    let marked = chart_space(
        &format!(
            r#"<c:plotArea><c:radarChart><c:radarStyle val="marker"/>{}{}</c:radarChart></c:plotArea>"#,
            series(0, "A", &["1", "2", "3"], ""),
            series(
                1,
                "B",
                &["1", "2", "3"],
                r#"<c:marker><c:symbol val="none"/></c:marker>"#
            )
        ),
        "",
    );
    let chart = read(&marked);
    assert_eq!(chart.plots[0].kind, PlotKind::Radar { filled: false });
    assert!(chart.plots[0].series[0].markers, "the marker style");
    assert!(
        !chart.plots[0].series[1].markers,
        "a series that hides them"
    );
    let bubble = chart_space(
        concat!(
            r#"<c:plotArea><c:bubbleChart><c:ser><c:idx val="0"/>"#,
            r#"<c:xVal><c:numLit><c:ptCount val="2"/><c:pt idx="0"><c:v>1</c:v></c:pt><c:pt idx="1"><c:v>2</c:v></c:pt></c:numLit></c:xVal>"#,
            r#"<c:yVal><c:numLit><c:ptCount val="2"/><c:pt idx="0"><c:v>5</c:v></c:pt><c:pt idx="1"><c:v>6</c:v></c:pt></c:numLit></c:yVal>"#,
            r#"<c:bubbleSize><c:numLit><c:ptCount val="2"/><c:pt idx="0"><c:v>3</c:v></c:pt><c:pt idx="1"><c:v>12</c:v></c:pt></c:numLit></c:bubbleSize>"#,
            r#"</c:ser></c:bubbleChart></c:plotArea>"#
        ),
        "",
    );
    let chart = read(&bubble);
    assert_eq!(chart.plots[0].kind, PlotKind::Bubble);
    let series = &chart.plots[0].series[0];
    assert_eq!(series.x_values, [Some(1.0), Some(2.0)]);
    assert_eq!(series.values, [Some(5.0), Some(6.0)]);
    assert_eq!(series.sizes, [Some(3.0), Some(12.0)]);
    assert!(!series.line && !series.markers);
}

#[test]
fn reads_the_high_low_lines_and_up_down_bars_of_a_stock_chart() {
    let xml = chart_space(
        &format!(
            concat!(
                r#"<c:plotArea><c:stockChart>{}{}<c:hiLowLines/><c:upDownBars><c:upBars><c:spPr><a:solidFill><a:srgbClr val="00FF00"/></a:solidFill></c:spPr></c:upBars>"#,
                r#"<c:downBars/></c:upDownBars></c:stockChart></c:plotArea>"#
            ),
            series(0, "Apertura", &["1", "2", "3"], ""),
            series(1, "Chiusura", &["2", "1", "3"], "")
        ),
        "",
    );
    let plot = &read(&xml).plots[0];
    assert_eq!(plot.kind, PlotKind::Line);
    assert!(plot.high_low_lines);
    assert_eq!(
        plot.up_down_bars,
        Some(("00FF00".to_owned(), "000000".to_owned()))
    );
}

#[test]
fn reads_and_draws_a_surface_as_bands() {
    let chart = read(&chart_space(
        &format!(
            "<c:plotArea><c:surfaceChart>{}{}</c:surfaceChart>{AXES}</c:plotArea><c:legend><c:legendPos val=\"r\"/></c:legend>",
            series(0, "Basso", &["10", "20", "30"], ""),
            series(1, "Alto", &["40", "", "60"], "")
        ),
        "",
    ));
    assert_eq!(chart.plots[0].kind, PlotKind::Surface);
    let svg = svg::chart_svg(&chart, 400.0, 300.0);
    assert!(
        svg.contains(">0,0-100,0</text>"),
        "a band of the axis, in its format"
    );
    assert!(svg.contains(">Basso</text>") && svg.contains(">Gen</text>"));
    assert_eq!(
        svg.matches("<rect").count(),
        6,
        "the background and the five bands of the legend: each cell touches the blank"
    );
    let one_row = read(&chart_space(
        &format!(
            "<c:plotArea><c:surface3DChart>{}</c:surface3DChart>{AXES}</c:plotArea>",
            series(0, "Solo", &["1", "2", "3"], "")
        ),
        "",
    ));
    assert!(svg::chart_svg(&one_row, 300.0, 200.0).contains("<rect"));
}

#[test]
fn draws_a_chart_of_the_2016_kinds_from_its_part() {
    let xml = extended::tests::chart_xml(
        r#"<cx:data id="0"><cx:numDim type="val"><cx:lvl ptCount="2"><cx:pt idx="0">4</cx:pt><cx:pt idx="1">-1</cx:pt></cx:lvl></cx:numDim></cx:data>"#,
        r#"<cx:series layoutId="waterfall"><cx:dataId val="0"/></cx:series>"#,
        "",
    );
    let (svg, title) = chart_picture(
        &xml,
        &Theme::default(),
        1_270_000,
        635_000,
        &no_cells,
        NumberLocale::Italian,
    )
    .unwrap();
    assert_eq!(title.as_deref(), Some("Risultato"));
    assert!(String::from_utf8(svg).unwrap().starts_with("<svg"));
    let frame = crate::xml_tree::parse_tree(
        r#"<f xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex"><g><cx:chart/></g></f>"#,
        |_, _| false,
    )
    .unwrap();
    assert!(chart_reference(&frame).is_some());
    assert!(
        chart_picture(
            "<c:chartSpace",
            &Theme::default(),
            1,
            1,
            &no_cells,
            NumberLocale::Italian
        )
        .is_none()
    );
}
