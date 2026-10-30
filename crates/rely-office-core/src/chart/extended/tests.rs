// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::xml_tree::parse_tree;

pub(crate) fn chart_xml(data: &str, series: &str, extra: &str) -> String {
    format!(
        concat!(
            r#"<cx:chartSpace xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
            r#"xmlns:cx="http://schemas.microsoft.com/office/drawing/2014/chartex">"#,
            "<cx:chartData>{}</cx:chartData><cx:chart>",
            "<cx:title><cx:tx><cx:rich><a:p><a:r><a:t>Risultato</a:t></a:r></a:p></cx:rich></cx:tx></cx:title>",
            "<cx:plotArea><cx:plotAreaRegion>{}</cx:plotAreaRegion></cx:plotArea>{}</cx:chart></cx:chartSpace>"
        ),
        data, series, extra
    )
}

fn read(xml: &str) -> Option<ExtendedChart> {
    let root = parse_tree(xml, |_, _| false).unwrap();
    read_extended_chart(&root, &Theme::default(), NumberLocale::default())
}

#[test]
fn reads_the_data_and_the_settings_of_each_series() {
    let chart = read(&chart_xml(
        concat!(
            r#"<cx:data id="0"><cx:strDim type="cat"><cx:lvl ptCount="3"><cx:pt idx="0">A</cx:pt><cx:pt idx="2">C</cx:pt></cx:lvl>"#,
            r#"<cx:lvl ptCount="3"><cx:pt idx="0">G</cx:pt><cx:pt idx="1">G</cx:pt><cx:pt idx="2">H</cx:pt></cx:lvl></cx:strDim>"#,
            r#"<cx:numDim type="val"><cx:lvl ptCount="3" formatCode="0.0"><cx:pt idx="0">1.5</cx:pt><cx:pt idx="1">x</cx:pt><cx:pt idx="2">-2</cx:pt><cx:pt idx="99999">9</cx:pt></cx:lvl></cx:numDim></cx:data>"#
        ),
        concat!(
            r#"<cx:series layoutId="waterfall"><cx:tx><cx:txData><cx:v>Conti</cx:v></cx:txData></cx:tx>"#,
            r#"<cx:spPr><a:solidFill><a:srgbClr val="112233"/></a:solidFill></cx:spPr>"#,
            r#"<cx:dataPt idx="1"><cx:spPr><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></cx:spPr></cx:dataPt>"#,
            r#"<cx:dataLabels><cx:visibility value="1"/></cx:dataLabels><cx:dataId val="0"/>"#,
            r#"<cx:layoutPr><cx:subtotals><cx:idx val="2"/></cx:subtotals><cx:binning><cx:binSize val="2.5"/></cx:binning>"#,
            r#"<cx:visibility meanMarker="0" outliers="false"/><cx:statistics quartileMethod="inclusive"/></cx:layoutPr></cx:series>"#,
            r#"<cx:series layoutId="funnel"><cx:dataId val="0"/><cx:layoutPr><cx:binning><cx:binCount val="4"/></cx:binning></cx:layoutPr></cx:series>"#,
            r#"<cx:series layoutId="regionMap"><cx:dataId val="0"/></cx:series>"#,
            r#"<cx:series layoutId="treemap" hidden="1"><cx:dataId val="0"/></cx:series>"#,
            r#"<cx:series layoutId="sunburst"><cx:dataId val="7"/></cx:series>"#,
            r#"<cx:series layoutId="paretoLine"/>"#
        ),
        r#"<cx:legend pos="b"/>"#,
    ))
    .unwrap();
    assert_eq!(chart.title.as_deref(), Some("Risultato"));
    assert_eq!(chart.legend, Some(LegendPosition::Bottom));
    assert_eq!(
        chart.series.len(),
        3,
        "a map, a hidden series and missing data are left out"
    );
    let waterfall = &chart.series[0];
    assert_eq!(waterfall.kind, ExtendedKind::Waterfall);
    assert_eq!(waterfall.name, "Conti");
    assert_eq!(waterfall.values, vec![Some(1.5), None, Some(-2.0)]);
    assert_eq!(
        waterfall.categories,
        vec![
            vec!["A".to_owned(), String::new(), "C".to_owned()],
            vec!["G".to_owned(), "G".to_owned(), "H".to_owned()]
        ]
    );
    assert_eq!(waterfall.colour, "112233");
    assert_eq!(waterfall.point_colours, vec![(1, "FF0000".to_owned())]);
    assert!(waterfall.labels);
    assert_eq!(waterfall.subtotals, vec![2]);
    assert_eq!(waterfall.binning, Binning::Width(2.5));
    assert!(!waterfall.exclusive_quartiles && !waterfall.mean_marker && !waterfall.outliers);
    assert_eq!(waterfall.format_code.as_deref(), Some("0.0"));
    let funnel = &chart.series[1];
    assert_eq!(
        (funnel.kind, funnel.binning),
        (ExtendedKind::Funnel, Binning::Count(4))
    );
    assert!(!funnel.labels && funnel.exclusive_quartiles && funnel.mean_marker);
    assert_eq!(funnel.name, "Serie 2");
    assert_eq!(chart.series[2].kind, ExtendedKind::ParetoLine);
    assert_eq!(chart.series[2].values, Vec::new());
    assert_eq!(chart.palette.len(), 6);
}

#[test]
fn reads_nothing_from_a_chart_with_no_series_it_draws() {
    assert!(read(&chart_xml("", r#"<cx:series layoutId="regionMap"/>"#, "")).is_none());
    assert!(
        read(
            "<cx:chartSpace xmlns:cx=\"http://schemas.microsoft.com/office/drawing/2014/chartex\"/>"
        )
        .is_none()
    );
    for (layout, kind) in [
        ("treemap", ExtendedKind::Treemap),
        ("sunburst", ExtendedKind::Sunburst),
        ("clusteredColumn", ExtendedKind::Histogram),
        ("boxWhisker", ExtendedKind::BoxWhisker),
    ] {
        assert_eq!(kind_of(layout), Some(kind));
    }
    let chart = read(&chart_xml(
        r#"<cx:data id="0"><cx:numDim type="val"><cx:lvl ptCount="1"><cx:pt idx="0">3</cx:pt></cx:lvl></cx:numDim></cx:data>"#,
        r#"<cx:series layoutId="boxWhisker"><cx:dataId val="0"/><cx:layoutPr><cx:binning><cx:binCount val="0"/></cx:binning></cx:layoutPr></cx:series>"#,
        "<cx:legend/>",
    ))
    .unwrap();
    assert_eq!(chart.legend, Some(LegendPosition::Top));
    assert_eq!(chart.series[0].binning, Binning::Automatic);
    assert_eq!(chart.series[0].categories, Vec::<Vec<String>>::new());
}
