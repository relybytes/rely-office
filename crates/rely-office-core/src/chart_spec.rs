// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! A chart as an editor describes it: its kind, its title, its legend and the ranges its
//! series read, `'Vendite'!$B$2:$B$7`. It is read from and written as a `DrawingML` chart part
//! and as an `OpenDocument` chart object, so a chart made or changed in an editor opens in
//! Excel and in `LibreOffice`, which compute it from the same cells.

use std::fmt::Write as _;

use crate::sheet_drawings::chart_formula_range;
use crate::sheet_layout::CellRange;
use crate::xml_tree::{Element, Namespace, parse_tree};

const C: Namespace = Namespace::Chart;
const A: Namespace = Namespace::Drawing;
const CHART: Namespace = Namespace::OdfChart;
const TABLE: Namespace = Namespace::OdfTable;
const TEXT: Namespace = Namespace::OdfText;
const OFFICE: Namespace = Namespace::OdfOffice;
const STYLE: Namespace = Namespace::OdfStyle;
/// The most series a chart part written or read here keeps.
const MAX_SERIES: usize = 64;
/// The longest title kept, in characters.
const MAX_TITLE: usize = 255;
/// The colours of Office's default theme, one per series or per slice, written out so a chart
/// looks the same in a workbook that carries no theme.
const PALETTE: [&str; 6] = ["4472C4", "ED7D31", "A5A5A5", "FFC000", "5B9BD5", "70AD47"];
/// The most slices a pie is given colours for.
const MAX_SLICES: u32 = 256;

fn colour(index: usize) -> &'static str {
    PALETTE[index % PALETTE.len()]
}

/// How many values a formula's range holds, for the slices of a pie.
fn point_count(formula: &str) -> u32 {
    chart_formula_range(formula).map_or(0, |(_, range)| {
        let rows = range.last_row - range.first_row + 1;
        let columns = range.last_column - range.first_column + 1;
        rows.saturating_mul(columns).min(MAX_SLICES)
    })
}

/// The kinds of chart an editor makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum ChartType {
    /// Vertical bars.
    Column,
    /// Horizontal bars.
    Bar,
    Line,
    Area,
    Pie,
    Doughnut,
    /// Points of an x and a y value.
    Scatter,
}

/// One series of a chart: the formula of its name, a cell or nothing, and of its values.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct ChartSeriesSpec {
    pub name: Option<String>,
    pub values: String,
}

/// A chart an editor can change, its ranges as formulas, `'Vendite'!$B$2:$B$7`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct ChartSpec {
    pub chart_type: ChartType,
    /// Bars or areas stacked on one another.
    pub stacked: bool,
    pub title: Option<String>,
    pub legend: bool,
    /// The labels of the categories, or the x values of a scatter chart.
    pub categories: Option<String>,
    pub series: Vec<ChartSeriesSpec>,
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// A sheet name as a formula writes it: quoted when it needs to be.
fn sheet_reference(name: &str) -> String {
    let plain = name
        .chars()
        .all(|character| character.is_alphanumeric() || character == '_')
        && !name
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_digit());
    if plain {
        name.to_owned()
    } else {
        format!("'{}'", name.replace('\'', "''"))
    }
}

fn column_letters(mut column: u32) -> String {
    let mut letters = Vec::new();
    loop {
        letters.push(char::from(b'A' + u8::try_from(column % 26).unwrap_or(0)));
        if column < 26 {
            break;
        }
        column = column / 26 - 1;
    }
    letters.iter().rev().collect()
}

/// A range of a sheet as a chart formula, absolute: `'Vendite'!$B$2:$B$7`.
#[must_use]
pub fn chart_range_formula(sheet: &str, range: CellRange) -> String {
    let corner = |row: u32, column: u32| format!("${}${}", column_letters(column), row + 1);
    let start = corner(range.first_row, range.first_column);
    let reference = if range.first_row == range.last_row && range.first_column == range.last_column
    {
        start
    } else {
        format!("{start}:{}", corner(range.last_row, range.last_column))
    };
    format!("{}!{reference}", sheet_reference(sheet))
}

fn title_xml(title: &str) -> String {
    format!(
        concat!(
            "<c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>{}</a:t></a:r></a:p>",
            r#"</c:rich></c:tx><c:overlay val="0"/></c:title><c:autoTitleDeleted val="0"/>"#
        ),
        escape(title)
    )
}

/// A chart as a `DrawingML` chart part, `c:chartSpace`, with no cached values: Excel and
/// `LibreOffice` read the cells the formulas name when they open it.
#[must_use]
#[expect(
    clippy::too_many_lines,
    reason = "the elements of a chart part in their schema order"
)]
pub fn chart_part_xml(spec: &ChartSpec) -> String {
    let mut xml = String::from(concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" "#,
        r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
        r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">"#,
        r#"<c:roundedCorners val="0"/><c:chart>"#
    ));
    match spec.title.as_deref().filter(|title| !title.is_empty()) {
        Some(title) => xml.push_str(&title_xml(title)),
        None => xml.push_str(r#"<c:autoTitleDeleted val="1"/>"#),
    }
    xml.push_str("<c:plotArea><c:layout/>");
    let grouping = if spec.stacked { "stacked" } else { "standard" };
    let (element, opening) = match spec.chart_type {
        ChartType::Column | ChartType::Bar => (
            "barChart",
            format!(
                r#"<c:barDir val="{}"/><c:grouping val="{}"/><c:varyColors val="0"/>"#,
                if spec.chart_type == ChartType::Bar {
                    "bar"
                } else {
                    "col"
                },
                if spec.stacked { "stacked" } else { "clustered" }
            ),
        ),
        ChartType::Line => (
            "lineChart",
            format!(r#"<c:grouping val="{grouping}"/><c:varyColors val="0"/>"#),
        ),
        ChartType::Area => (
            "areaChart",
            format!(r#"<c:grouping val="{grouping}"/><c:varyColors val="0"/>"#),
        ),
        ChartType::Pie => ("pieChart", r#"<c:varyColors val="1"/>"#.to_owned()),
        ChartType::Doughnut => ("doughnutChart", r#"<c:varyColors val="1"/>"#.to_owned()),
        ChartType::Scatter => (
            "scatterChart",
            r#"<c:scatterStyle val="lineMarker"/><c:varyColors val="0"/>"#.to_owned(),
        ),
    };
    let _ = write!(xml, "<c:{element}>{opening}");
    for (index, series) in spec.series.iter().take(MAX_SERIES).enumerate() {
        let _ = write!(
            xml,
            r#"<c:ser><c:idx val="{index}"/><c:order val="{index}"/>"#
        );
        if let Some(name) = &series.name {
            let _ = write!(
                xml,
                "<c:tx><c:strRef><c:f>{}</c:f></c:strRef></c:tx>",
                escape(name)
            );
        }
        let fill =
            |colour: &str| format!(r#"<a:solidFill><a:srgbClr val="{colour}"/></a:solidFill>"#);
        match spec.chart_type {
            ChartType::Line => {
                let _ = write!(
                    xml,
                    r#"<c:spPr><a:ln w="28575" cap="rnd">{}<a:round/></a:ln></c:spPr><c:marker><c:symbol val="none"/></c:marker>"#,
                    fill(colour(index))
                );
            }
            ChartType::Scatter => {
                let _ = write!(
                    xml,
                    r#"<c:spPr><a:ln w="19050"><a:noFill/></a:ln></c:spPr><c:marker><c:symbol val="circle"/><c:size val="5"/><c:spPr>{}</c:spPr></c:marker>"#,
                    fill(colour(index))
                );
            }
            ChartType::Column | ChartType::Bar | ChartType::Area => {
                let _ = write!(xml, "<c:spPr>{}</c:spPr>", fill(colour(index)));
                if spec.chart_type != ChartType::Area {
                    xml.push_str(r#"<c:invertIfNegative val="0"/>"#);
                }
            }
            ChartType::Pie | ChartType::Doughnut => {
                for point in 0..point_count(&series.values) {
                    let _ = write!(
                        xml,
                        r#"<c:dPt><c:idx val="{point}"/><c:bubble3D val="0"/><c:spPr>{}<a:ln w="12700"><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill></a:ln></c:spPr></c:dPt>"#,
                        fill(colour(point as usize))
                    );
                }
            }
        }
        let values = escape(&series.values);
        if spec.chart_type == ChartType::Scatter {
            if let Some(categories) = &spec.categories {
                let _ = write!(
                    xml,
                    "<c:xVal><c:numRef><c:f>{}</c:f></c:numRef></c:xVal>",
                    escape(categories)
                );
            }
            let _ = write!(
                xml,
                "<c:yVal><c:numRef><c:f>{values}</c:f></c:numRef></c:yVal>"
            );
            xml.push_str(r#"<c:smooth val="0"/>"#);
        } else {
            if let Some(categories) = &spec.categories {
                let _ = write!(
                    xml,
                    "<c:cat><c:strRef><c:f>{}</c:f></c:strRef></c:cat>",
                    escape(categories)
                );
            }
            let _ = write!(
                xml,
                "<c:val><c:numRef><c:f>{values}</c:f></c:numRef></c:val>"
            );
            if spec.chart_type == ChartType::Line {
                xml.push_str(r#"<c:smooth val="0"/>"#);
            }
        }
        xml.push_str("</c:ser>");
    }
    match spec.chart_type {
        ChartType::Column | ChartType::Bar => {
            xml.push_str(r#"<c:gapWidth val="150"/>"#);
            if spec.stacked {
                xml.push_str(r#"<c:overlap val="100"/>"#);
            }
        }
        ChartType::Line => xml.push_str(r#"<c:marker val="1"/>"#),
        ChartType::Pie => xml.push_str(r#"<c:firstSliceAng val="0"/>"#),
        ChartType::Doughnut => xml.push_str(r#"<c:firstSliceAng val="0"/><c:holeSize val="50"/>"#),
        _ => {}
    }
    let round = matches!(spec.chart_type, ChartType::Pie | ChartType::Doughnut);
    if !round {
        xml.push_str(r#"<c:axId val="500000001"/><c:axId val="500000002"/>"#);
    }
    let _ = write!(xml, "</c:{element}>");
    if !round {
        let (category_position, value_position) = if spec.chart_type == ChartType::Bar {
            ("l", "b")
        } else {
            ("b", "l")
        };
        let scaling = r#"<c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/>"#;
        if spec.chart_type == ChartType::Scatter {
            let _ = write!(
                xml,
                concat!(
                    r#"<c:valAx><c:axId val="500000001"/>{0}<c:axPos val="b"/><c:numFmt formatCode="General" sourceLinked="1"/>"#,
                    r#"<c:majorTickMark val="out"/><c:minorTickMark val="none"/><c:tickLblPos val="nextTo"/>"#,
                    r#"<c:crossAx val="500000002"/><c:crosses val="autoZero"/><c:crossBetween val="midCat"/></c:valAx>"#
                ),
                scaling
            );
        } else {
            let _ = write!(
                xml,
                concat!(
                    r#"<c:catAx><c:axId val="500000001"/>{0}<c:axPos val="{1}"/><c:numFmt formatCode="General" sourceLinked="1"/>"#,
                    r#"<c:majorTickMark val="out"/><c:minorTickMark val="none"/><c:tickLblPos val="nextTo"/>"#,
                    r#"<c:crossAx val="500000002"/><c:crosses val="autoZero"/><c:auto val="1"/><c:lblAlgn val="ctr"/>"#,
                    r#"<c:lblOffset val="100"/><c:noMultiLvlLbl val="0"/></c:catAx>"#
                ),
                scaling, category_position
            );
        }
        let _ = write!(
            xml,
            concat!(
                r#"<c:valAx><c:axId val="500000002"/>{0}<c:axPos val="{1}"/><c:majorGridlines/>"#,
                r#"<c:numFmt formatCode="General" sourceLinked="1"/><c:majorTickMark val="out"/>"#,
                r#"<c:minorTickMark val="none"/><c:tickLblPos val="nextTo"/><c:crossAx val="500000001"/>"#,
                r#"<c:crosses val="autoZero"/><c:crossBetween val="{2}"/></c:valAx>"#
            ),
            scaling,
            value_position,
            if spec.chart_type == ChartType::Scatter {
                "midCat"
            } else {
                "between"
            }
        );
    }
    xml.push_str("</c:plotArea>");
    if spec.legend {
        xml.push_str(r#"<c:legend><c:legendPos val="r"/><c:overlay val="0"/></c:legend>"#);
    }
    xml.push_str(r#"<c:plotVisOnly val="1"/><c:dispBlanksAs val="gap"/></c:chart></c:chartSpace>"#);
    xml
}

fn value_of<'e>(element: &'e Element, name: &str) -> Option<&'e str> {
    element
        .child(C, name)
        .and_then(|child| child.attribute(Namespace::None, "val"))
}

/// The formula of a reference element, `c:strRef` or `c:numRef`, inside `parent`.
fn formula_in(parent: Option<&Element>) -> Option<String> {
    let parent = parent?;
    let reference = parent
        .child(C, "strRef")
        .or_else(|| parent.child(C, "numRef"))
        .or_else(|| parent.child(C, "multiLvlStrRef"))?;
    let formula = reference.child(C, "f")?.text();
    let formula = formula.trim();
    (!formula.is_empty()).then(|| formula.to_owned())
}

/// The text of every `a:t` in an element, in order.
fn rich_text(element: &Element, text: &mut String) {
    for child in element.elements() {
        if child.is(A, "t") {
            text.push_str(&child.text());
        } else {
            rich_text(child, text);
        }
    }
}

fn truncate(text: &str) -> String {
    text.chars().take(MAX_TITLE).collect()
}

/// The chart a `DrawingML` chart part describes, when an editor can change it: one plot of a
/// kind it makes, every series reading a range; `None` for anything else, which stays as the
/// file drew it.
#[must_use]
pub fn read_chart_spec(xml: &str) -> Option<ChartSpec> {
    let root = parse_tree(xml, |namespace, name| namespace == A && name == "t").ok()?;
    if !root.is(C, "chartSpace") {
        return None;
    }
    let chart = root.child(C, "chart")?;
    let plot_area = chart.child(C, "plotArea")?;
    let plots: Vec<&Element> = plot_area
        .elements()
        .filter(|plot| plot.namespace == C && plot.name.ends_with("Chart"))
        .collect();
    let [plot] = plots.as_slice() else {
        return None;
    };
    let chart_type = match plot.name.as_str() {
        "barChart" | "bar3DChart" if value_of(plot, "barDir") == Some("bar") => ChartType::Bar,
        "barChart" | "bar3DChart" => ChartType::Column,
        "lineChart" | "line3DChart" => ChartType::Line,
        "areaChart" | "area3DChart" => ChartType::Area,
        "pieChart" | "pie3DChart" => ChartType::Pie,
        "doughnutChart" => ChartType::Doughnut,
        "scatterChart" => ChartType::Scatter,
        _ => return None,
    };
    let stacked = matches!(
        value_of(plot, "grouping"),
        Some("stacked" | "percentStacked")
    );
    let title = chart.child(C, "title").and_then(|title| {
        let mut text = String::new();
        if let Some(rich) = title.child(C, "tx").and_then(|tx| tx.child(C, "rich")) {
            rich_text(rich, &mut text);
        }
        let text = truncate(text.trim());
        (!text.is_empty()).then_some(text)
    });
    let mut categories = None;
    let mut series = Vec::new();
    for element in plot
        .elements()
        .filter(|element| element.is(C, "ser"))
        .take(MAX_SERIES)
    {
        let (values, labels) = if chart_type == ChartType::Scatter {
            (element.child(C, "yVal"), element.child(C, "xVal"))
        } else {
            (element.child(C, "val"), element.child(C, "cat"))
        };
        let values = formula_in(values)?;
        if categories.is_none() {
            categories = formula_in(labels);
        }
        series.push(ChartSeriesSpec {
            name: formula_in(element.child(C, "tx")),
            values,
        });
    }
    if series.is_empty() {
        return None;
    }
    Some(ChartSpec {
        chart_type,
        stacked,
        title,
        legend: chart.child(C, "legend").is_some(),
        categories,
        series,
    })
}

/// A chart formula as an `OpenDocument` address, `'Vendite'.$B$2:'Vendite'.$B$7`.
fn odf_address(formula: &str) -> Option<String> {
    let (sheet, range) = chart_formula_range(formula)?;
    let quoted = format!("'{}'", sheet.replace('\'', "''"));
    let corner =
        |row: u32, column: u32| format!("{quoted}.${}${}", column_letters(column), row + 1);
    let start = corner(range.first_row, range.first_column);
    Some(
        if range.first_row == range.last_row && range.first_column == range.last_column {
            start
        } else {
            format!("{start}:{}", corner(range.last_row, range.last_column))
        },
    )
}

/// An `OpenDocument` address as a chart formula; the sheet a part leaves out is the one before.
fn formula_of_address(address: &str) -> Option<String> {
    let address = address.trim();
    let mut parts = Vec::new();
    let mut quoted = false;
    let mut start = 0;
    for (index, character) in address.char_indices() {
        match character {
            '\'' => quoted = !quoted,
            ':' if !quoted => {
                parts.push(&address[start..index]);
                start = index + 1;
            }
            ' ' if !quoted => return None,
            _ => {}
        }
    }
    parts.push(&address[start..]);
    if parts.len() > 2 {
        return None;
    }
    let mut sheet: Option<String> = None;
    let mut cells = Vec::new();
    for part in parts {
        let part = part.trim_start_matches('$');
        let (name, cell) = match part.rfind('.') {
            Some(dot) => (&part[..dot], &part[dot + 1..]),
            None => ("", part),
        };
        if !name.is_empty() {
            let name = name.trim_start_matches('$');
            sheet = Some(
                name.strip_prefix('\'')
                    .and_then(|inner| inner.strip_suffix('\''))
                    .map_or_else(|| name.to_owned(), |inner| inner.replace("''", "'")),
            );
        }
        cells.push(cell.replace('$', ""));
    }
    let range = CellRange::parse(&cells.join(":"))?;
    Some(chart_range_formula(&sheet?, range))
}

/// The bounding range of every range a chart reads, when they are all on one sheet.
fn plot_range(spec: &ChartSpec) -> Option<String> {
    let mut formulas: Vec<&str> = spec
        .series
        .iter()
        .map(|series| series.values.as_str())
        .collect();
    formulas.extend(spec.categories.as_deref());
    formulas.extend(
        spec.series
            .iter()
            .filter_map(|series| series.name.as_deref()),
    );
    let mut sheet = None;
    let mut bounds: Option<CellRange> = None;
    for formula in formulas {
        let (name, range) = chart_formula_range(formula)?;
        if sheet.get_or_insert_with(|| name.clone()) != &name {
            return None;
        }
        bounds = Some(match bounds {
            None => range,
            Some(known) => CellRange {
                first_row: known.first_row.min(range.first_row),
                first_column: known.first_column.min(range.first_column),
                last_row: known.last_row.max(range.last_row),
                last_column: known.last_column.max(range.last_column),
            },
        });
    }
    odf_address(&chart_range_formula(&sheet?, bounds?))
}

/// A chart as the content of an `OpenDocument` chart object, its series reading the cells of
/// the spreadsheet it is embedded in.
#[must_use]
#[expect(
    clippy::too_many_lines,
    reason = "the elements of a chart object in their schema order"
)]
pub fn odf_chart_object_xml(spec: &ChartSpec) -> String {
    let class = match spec.chart_type {
        ChartType::Column | ChartType::Bar => "chart:bar",
        ChartType::Line => "chart:line",
        ChartType::Area => "chart:area",
        ChartType::Pie => "chart:circle",
        ChartType::Doughnut => "chart:ring",
        ChartType::Scatter => "chart:scatter",
    };
    let mut xml = format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content "#,
            r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
            r#"xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" "#,
            r#"xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" "#,
            r#"xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" "#,
            r#"xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" "#,
            r#"xmlns:chart="urn:oasis:names:tc:opendocument:xmlns:chart:1.0" "#,
            r#"xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" "#,
            r#"xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" office:version="1.3">"#,
            r#"<office:automatic-styles><style:style style:name="plot" style:family="chart">"#,
            r#"<style:chart-properties chart:vertical="{}" chart:stacked="{}" chart:lines="{}" chart:symbol-type="{}"/>"#,
            r#"</style:style><style:style style:name="axis" style:family="chart">"#,
            r#"<style:chart-properties chart:display-label="true" chart:tick-marks-major-outer="true"/>"#,
            r#"</style:style><style:style style:name="wall" style:family="chart">"#,
            r#"<style:graphic-properties draw:fill="none" draw:stroke="none"/></style:style>"#
        ),
        spec.chart_type == ChartType::Bar,
        spec.stacked,
        spec.chart_type != ChartType::Scatter,
        if spec.chart_type == ChartType::Scatter {
            "automatic"
        } else {
            "none"
        },
    );
    let line = matches!(spec.chart_type, ChartType::Line);
    for index in 0..spec.series.len().min(MAX_SERIES) {
        let _ = write!(
            xml,
            concat!(
                r#"<style:style style:name="series{0}" style:family="chart"><style:graphic-properties "#,
                r##"draw:fill="solid" draw:fill-color="#{1}" svg:stroke-color="#{2}" svg:stroke-width="{3}"/></style:style>"##
            ),
            index,
            colour(index),
            if line { colour(index) } else { "FFFFFF" },
            if line { "0.08cm" } else { "0cm" }
        );
    }
    let _ = write!(
        xml,
        r#"</office:automatic-styles><office:body><office:chart><chart:chart chart:class="{class}">"#
    );
    if let Some(title) = spec.title.as_deref().filter(|title| !title.is_empty()) {
        let _ = write!(
            xml,
            "<chart:title><text:p>{}</text:p></chart:title>",
            escape(title)
        );
    }
    if spec.legend {
        xml.push_str(r#"<chart:legend chart:legend-position="end"/>"#);
    }
    xml.push_str(r#"<chart:plot-area chart:style-name="plot""#);
    if let Some(range) = plot_range(spec) {
        let _ = write!(
            xml,
            r#" table:cell-range-address="{}" chart:data-source-has-labels="none""#,
            escape(&range)
        );
    }
    xml.push('>');
    // A pie has no axes to draw, but its categories still belong to the x axis.
    xml.push_str(
        r#"<chart:axis chart:dimension="x" chart:name="primary-x" chart:style-name="axis">"#,
    );
    if spec.chart_type != ChartType::Scatter
        && let Some(categories) = spec.categories.as_deref().and_then(odf_address)
    {
        let _ = write!(
            xml,
            r#"<chart:categories table:cell-range-address="{}"/>"#,
            escape(&categories)
        );
    }
    xml.push_str("</chart:axis>");
    if !matches!(spec.chart_type, ChartType::Pie | ChartType::Doughnut) {
        xml.push_str(r#"<chart:axis chart:dimension="y" chart:name="primary-y" chart:style-name="axis"><chart:grid chart:class="major"/></chart:axis>"#);
    }
    xml.push_str(r#"<chart:wall chart:style-name="wall"/>"#);
    for (index, series) in spec.series.iter().take(MAX_SERIES).enumerate() {
        let Some(values) = odf_address(&series.values) else {
            continue;
        };
        // A pie keeps the automatic colours, one a slice; a style would give every slice one.
        let style = if matches!(spec.chart_type, ChartType::Pie | ChartType::Doughnut) {
            String::new()
        } else {
            format!(r#" chart:style-name="series{index}""#)
        };
        let _ = write!(
            xml,
            r#"<chart:series{style} chart:values-cell-range-address="{}""#,
            escape(&values)
        );
        if let Some(name) = series.name.as_deref().and_then(odf_address) {
            let _ = write!(xml, r#" chart:label-cell-address="{}""#, escape(&name));
        }
        let _ = write!(xml, r#" chart:class="{class}">"#);
        if spec.chart_type == ChartType::Scatter
            && let Some(x_values) = spec.categories.as_deref().and_then(odf_address)
        {
            let _ = write!(
                xml,
                r#"<chart:domain table:cell-range-address="{}"/>"#,
                escape(&x_values)
            );
        }
        xml.push_str("</chart:series>");
    }
    xml.push_str(
        "</chart:plot-area></chart:chart></office:chart></office:body></office:document-content>",
    );
    xml
}

/// The chart an `OpenDocument` chart object describes, when its series read cell ranges.
#[must_use]
pub fn read_odf_chart_spec(xml: &str) -> Option<ChartSpec> {
    let root = parse_tree(xml, |namespace, name| namespace == TEXT && name == "p").ok()?;
    let chart = root
        .child(OFFICE, "body")?
        .child(OFFICE, "chart")?
        .child(CHART, "chart")?;
    let plot_area = chart.child(CHART, "plot-area")?;
    let styles: Vec<&Element> = root
        .child(OFFICE, "automatic-styles")
        .map(|styles| styles.elements().collect())
        .unwrap_or_default();
    let property = |name: &str| {
        let style_name = plot_area.attribute(CHART, "style-name")?;
        styles
            .iter()
            .find(|style| style.attribute(STYLE, "name") == Some(style_name))?
            .child(STYLE, "chart-properties")?
            .attribute(CHART, name)
            .map(str::to_owned)
    };
    let vertical = property("vertical").as_deref() == Some("true");
    let stacked = property("stacked").as_deref() == Some("true")
        || property("percentage").as_deref() == Some("true");
    let chart_type = match chart.attribute(CHART, "class")? {
        "chart:bar" if vertical => ChartType::Bar,
        "chart:bar" => ChartType::Column,
        "chart:line" => ChartType::Line,
        "chart:area" => ChartType::Area,
        "chart:circle" => ChartType::Pie,
        "chart:ring" => ChartType::Doughnut,
        "chart:scatter" => ChartType::Scatter,
        _ => return None,
    };
    let title = chart.child(CHART, "title").and_then(|title| {
        let text: Vec<String> = title
            .elements()
            .filter(|paragraph| paragraph.is(TEXT, "p"))
            .map(Element::text)
            .collect();
        let text = truncate(text.join(" ").trim());
        (!text.is_empty()).then_some(text)
    });
    let mut categories = plot_area
        .elements()
        .filter(|axis| axis.is(CHART, "axis") && axis.attribute(CHART, "dimension") == Some("x"))
        .find_map(|axis| axis.child(CHART, "categories"))
        .and_then(|categories| categories.attribute(TABLE, "cell-range-address"))
        .and_then(formula_of_address);
    let mut series = Vec::new();
    for element in plot_area
        .elements()
        .filter(|element| element.is(CHART, "series"))
        .take(MAX_SERIES)
    {
        let values = formula_of_address(element.attribute(CHART, "values-cell-range-address")?)?;
        if chart_type == ChartType::Scatter && categories.is_none() {
            categories = element
                .child(CHART, "domain")
                .and_then(|domain| domain.attribute(TABLE, "cell-range-address"))
                .and_then(formula_of_address);
        }
        series.push(ChartSeriesSpec {
            name: element
                .attribute(CHART, "label-cell-address")
                .and_then(formula_of_address),
            values,
        });
    }
    if series.is_empty() {
        return None;
    }
    Some(ChartSpec {
        chart_type,
        stacked,
        title,
        legend: chart.child(CHART, "legend").is_some(),
        categories,
        series,
    })
}

#[cfg(test)]
mod tests;
