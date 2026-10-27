// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;

const NAMESPACES: &str = concat!(
    r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
    r#"xmlns:chart="urn:oasis:names:tc:opendocument:xmlns:chart:1.0" "#,
    r#"xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" "#,
    r#"xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" "#,
    r#"xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" "#,
    r#"xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" "#,
    r#"xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" "#,
    r#"xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0""#
);

const TABLE_XML: &str = concat!(
    r#"<table:table table:name="local-table"><table:table-header-rows><table:table-row><table:table-cell/>"#,
    r#"<table:table-cell office:value-type="string"><text:p>Nord</text:p></table:table-cell>"#,
    r#"<table:table-cell office:value-type="string"><text:p>Sud</text:p></table:table-cell></table:table-row></table:table-header-rows>"#,
    r#"<table:table-rows><table:table-row><table:table-cell office:value-type="string"><text:p>Gen</text:p></table:table-cell>"#,
    r#"<table:table-cell office:value-type="float" office:value="10"/><table:table-cell office:value-type="float" office:value="4"/></table:table-row>"#,
    r#"<table:table-row><table:table-cell office:value-type="float" office:value="2026"/>"#,
    r#"<table:table-cell office:value-type="float" office:value="20" table:number-columns-repeated="2"/></table:table-row>"#,
    r#"<table:table-row><table:table-cell office:value-type="string"><text:p>Mar</text:p></table:table-cell>"#,
    r#"<table:table-cell office:value-type="string"><text:p>n/d</text:p></table:table-cell><table:table-cell/></table:table-row>"#,
    "</table:table-rows></table:table>"
);

fn chart(class: &str, styles: &str, plot: &str, extra: &str) -> String {
    format!(
        concat!(
            r#"<office:document-content {}><office:automatic-styles>{}</office:automatic-styles><office:body><office:chart>"#,
            r#"<chart:chart chart:class="{}">{}<chart:plot-area chart:style-name="plot">{}</chart:plot-area>{}</chart:chart>"#,
            "</office:chart></office:body></office:document-content>"
        ),
        NAMESPACES, styles, class, extra, plot, TABLE_XML
    )
}

const SERIES: &str = concat!(
    r#"<chart:axis chart:dimension="x" chart:style-name="x"><chart:categories table:cell-range-address="local-table.$A$2:.$A$4"/>"#,
    r#"<chart:title><text:p>Mese</text:p></chart:title></chart:axis>"#,
    r#"<chart:axis chart:dimension="y" chart:style-name="y"><chart:grid chart:class="major"/></chart:axis>"#,
    r#"<chart:series chart:style-name="s1" chart:values-cell-range-address="local-table.$B$2:.$B$4" chart:label-cell-address="local-table.$B$1">"#,
    r#"<chart:data-point chart:repeated="2"/><chart:data-point chart:style-name="p1"/></chart:series>"#,
    r#"<chart:series chart:class="chart:line" chart:values-cell-range-address="local-table.$C$2:.$C$4"/>"#
);

const STYLES: &str = concat!(
    r#"<style:style style:name="plot" style:family="chart"><style:chart-properties chart:vertical="true" chart:stacked="true" chart:gap-width="80"/></style:style>"#,
    r#"<style:style style:name="x" style:family="chart"><style:chart-properties chart:reverse-direction="true"/><style:text-properties fo:font-size="11pt"/></style:style>"#,
    r#"<style:style style:name="y" style:family="chart"><style:chart-properties chart:minimum="0" chart:maximum="50" chart:interval-major="10"/></style:style>"#,
    r##"<style:style style:name="s1" style:family="chart"><style:chart-properties chart:data-label-number="value-and-percentage"/><style:graphic-properties draw:fill-color="#123456"/></style:style>"##,
    r##"<style:style style:name="p1" style:family="chart"><style:graphic-properties draw:fill-color="#ABCDEF"/></style:style>"##,
    r#"<style:style style:name="t" style:family="chart"><style:text-properties fo:font-size="20pt"/></style:style>"#
);

#[test]
fn reads_a_chart_object_and_the_cells_of_its_table() {
    let xml = chart(
        "chart:bar",
        STYLES,
        SERIES,
        r#"<chart:title chart:style-name="t"><text:p>Ricavi</text:p><text:p>2026</text:p></chart:title><chart:legend chart:legend-position="top"/>"#,
    );
    let read = read_odf_chart(&xml, NumberLocale::Italian).unwrap();
    assert_eq!(read.title.as_deref(), Some("Ricavi 2026"));
    assert!((read.title_size - 20.0).abs() < f64::EPSILON);
    assert!((read.label_size - 11.0).abs() < f64::EPSILON);
    assert_eq!(read.legend, Some(LegendPosition::Top));
    assert_eq!(
        read.plots.len(),
        2,
        "a series of its own class makes a plot of its own"
    );
    let bars = &read.plots[0];
    assert_eq!(
        (
            bars.kind,
            bars.grouping,
            bars.gap_percent,
            bars.overlap_percent
        ),
        (PlotKind::Bar, Grouping::Stacked, 80, 100)
    );
    assert_eq!(bars.categories, ["Gen", "2026", "Mar"]);
    let north = &bars.series[0];
    assert_eq!(north.name, "Nord");
    assert_eq!(north.values, [Some(10.0), Some(20.0), None]);
    assert_eq!(north.colour, "123456");
    assert_eq!(north.point_colours, [(2, "ABCDEF".to_owned())]);
    assert!(north.labels.value && north.labels.percent);
    let line = &read.plots[1];
    assert_eq!(line.kind, PlotKind::Line);
    assert_eq!(line.series[0].name, "Serie 2");
    assert_eq!(line.series[0].colour, PALETTE[1]);
    assert_eq!(line.series[0].values, [Some(4.0), Some(20.0), None]);
    assert_eq!(
        (
            read.primary.minimum,
            read.primary.maximum,
            read.primary.major_unit
        ),
        (Some(0.0), Some(50.0), Some(10.0))
    );
    assert!(read.primary.gridlines);
    assert!(read.categories_reversed);
    assert_eq!(read.category_title.as_deref(), Some("Mese"));
}

#[test]
fn reads_pies_rings_and_scatter_charts() {
    let pie = chart(
        "chart:circle",
        "",
        r#"<chart:series chart:values-cell-range-address="local-table.$B$2:.$B$3"/>"#,
        "",
    );
    let read = read_odf_chart(&pie, NumberLocale::Italian).unwrap();
    assert_eq!(read.plots[0].kind, PlotKind::Pie);
    assert!(read.plots[0].vary_colours);
    assert_eq!(
        read.plots[0].series[0].point_colours,
        [(0, PALETTE[0].to_owned()), (1, PALETTE[1].to_owned())]
    );
    assert_eq!(read.legend, None);
    let ring = chart(
        "chart:ring",
        "",
        r#"<chart:series chart:values-cell-range-address="local-table.$B$2:.$B$3"/>"#,
        "",
    );
    assert_eq!(
        read_odf_chart(&ring, NumberLocale::Italian).unwrap().plots[0].kind,
        PlotKind::Doughnut { hole_percent: 50 }
    );
    let scatter = chart(
        "chart:scatter",
        r#"<style:style style:name="xs" style:family="chart"><style:chart-properties chart:minimum="5"/></style:style>"#,
        concat!(
            r#"<chart:axis chart:dimension="x" chart:style-name="xs"/><chart:series chart:values-cell-range-address="local-table.$C$2:.$C$3">"#,
            r#"<chart:domain table:cell-range-address="local-table.$B$2:.$B$3"/></chart:series>"#
        ),
        r#"<chart:legend chart:legend-position="start"/>"#,
    );
    let read = read_odf_chart(&scatter, NumberLocale::Italian).unwrap();
    let series = &read.plots[0].series[0];
    assert_eq!(series.x_values, [Some(10.0), Some(20.0)]);
    assert!(series.markers);
    assert_eq!(read.x_axis.minimum, Some(5.0));
    assert_eq!(read.legend, Some(LegendPosition::Left));
}

#[test]
fn draws_nothing_it_cannot_read() {
    assert!(read_odf_chart("<broken", NumberLocale::Italian).is_none());
    assert!(read_odf_chart(&chart("chart:radar", "", "", ""), NumberLocale::Italian).is_none());
    assert!(
        read_odf_chart(&chart("chart:bar", "", "", ""), NumberLocale::Italian).is_none(),
        "no series"
    );
    let unaddressed = chart("chart:area", "", "<chart:series/>", "");
    let read = read_odf_chart(&unaddressed, NumberLocale::Italian).unwrap();
    assert_eq!(read.plots[0].series[0].values, Vec::<Option<f64>>::new());
}

#[test]
fn reads_chart_addresses_and_colours() {
    assert_eq!(
        range_of("local-table.$B$2:.$B$5"),
        CellRange::parse("B2:B5")
    );
    assert_eq!(range_of("Foglio1.A1"), CellRange::parse("A1"));
    assert_eq!(range_of(""), None);
    assert_eq!(hex("#abcdef").as_deref(), Some("ABCDEF"));
    assert_eq!(hex("abcdef"), None);
}

#[test]
fn reads_radars_and_bubbles_from_the_copy_of_the_sheet() {
    let radar = read_odf_chart(
        &chart("chart:filled-radar", STYLES, SERIES, ""),
        NumberLocale::Italian,
    )
    .unwrap();
    assert_eq!(radar.plots[0].kind, PlotKind::Radar { filled: true });
    let table = concat!(
        r#"<table:table table:name="local-table"><table:table-rows><table:table-row>"#,
        r#"<table:table-cell office:value-type="float" office:value="1"><draw:g><svg:desc>Dati.A11:Dati.A12</svg:desc></draw:g></table:table-cell>"#,
        r#"<table:table-cell office:value-type="float" office:value="10"><draw:g><svg:desc>Dati.$B$11:Dati.$B$12</svg:desc></draw:g></table:table-cell>"#,
        r#"<table:table-cell office:value-type="float" office:value="5"><draw:g><svg:desc>Dati.C11:Dati.C12</svg:desc></draw:g></table:table-cell></table:table-row>"#,
        r#"<table:table-row><table:table-cell office:value-type="float" office:value="2"/><table:table-cell office:value-type="float" office:value="30"/>"#,
        r#"<table:table-cell office:value-type="float" office:value="20"/></table:table-row></table:table-rows></table:table>"#
    );
    let series = concat!(
        r#"<chart:series chart:class="chart:bubble" chart:values-cell-range-address="Dati.C11:Dati.C12" loext:label-string="&quot;Progetti&quot;">"#,
        r#"<chart:domain table:cell-range-address="Dati.B11:Dati.B12"/><chart:domain table:cell-range-address="Dati.A11:Dati.A12"/></chart:series>"#
    );
    let xml = format!(
        concat!(
            r#"<office:document-content {} xmlns:loext="urn:org:documentfoundation:names:experimental:office:xmlns:loext:1.0"><office:body><office:chart>"#,
            r#"<chart:chart chart:class="chart:bubble"><chart:plot-area>{}</chart:plot-area>{}</chart:chart>"#,
            "</office:chart></office:body></office:document-content>"
        ),
        NAMESPACES, series, table
    );
    let bubble = read_odf_chart(&xml, NumberLocale::Italian).unwrap();
    assert_eq!(bubble.plots[0].kind, PlotKind::Bubble);
    let series = &bubble.plots[0].series[0];
    assert_eq!(series.name, "Progetti", "a name typed in");
    assert_eq!(
        series.values,
        [Some(10.0), Some(30.0)],
        "y, the first domain"
    );
    assert_eq!(series.x_values, [Some(1.0), Some(2.0)], "x, the second");
    assert_eq!(
        series.sizes,
        [Some(5.0), Some(20.0)],
        "the sizes, its values"
    );
    assert!(!series.line);
}
