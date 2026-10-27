// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The charts of `DrawingML`, read from the values the file caches beside each formula and
//! drawn as SVG: a chart in a docx, an xlsx or a pptx shows what the application last computed,
//! with no workbook to evaluate.

mod odf;
mod svg;

use crate::number_format::{NumberLocale, general_number};
use crate::theme::{Theme, colour_in};
use crate::xml_tree::{Element, Namespace, parse_tree};

use svg::chart_svg;

const C: Namespace = Namespace::Chart;
const A: Namespace = Namespace::Drawing;
/// The most series a chart draws, and the most points of one series.
const MAX_SERIES: usize = 64;
const MAX_POINTS: usize = 4_096;
const MAX_TEXT: usize = 200;

/// How a plot lays its values out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlotKind {
    /// Vertical bars.
    Column,
    /// Horizontal bars.
    Bar,
    Line,
    Area,
    Pie,
    Doughnut {
        hole_percent: u32,
    },
    Scatter,
    /// Points sized by a third value over two value axes.
    Bubble,
    /// A spoke per category around a centre, each series a closed outline.
    Radar {
        filled: bool,
    },
}

/// How the series of a plot share a category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Grouping {
    /// Side by side, or each on its own for lines and areas.
    Standard,
    Stacked,
    /// Stacked to a whole.
    Percent,
}

/// Where the legend sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LegendPosition {
    Right,
    Left,
    Top,
    Bottom,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Series {
    pub name: String,
    /// The values by point; a blank cell is `None`.
    pub values: Vec<Option<f64>>,
    /// The x of each point of a scatter or bubble series.
    pub x_values: Vec<Option<f64>>,
    /// The size of each point of a bubble series.
    pub sizes: Vec<Option<f64>>,
    pub colour: String,
    /// The colour of each point that has one of its own, by point index.
    pub point_colours: Vec<(usize, String)>,
    /// A series drawn with no line, only its markers.
    pub line: bool,
    pub markers: bool,
    pub labels: DataLabels,
    pub format_code: Option<String>,
    /// The format of the data labels, when it is not the values' own.
    pub label_format: Option<String>,
}

/// A cell a chart formula points at, read from its workbook when the chart caches no value.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ChartCell {
    Number(f64),
    Text(String),
    Empty,
}

/// What resolves a chart formula, `'Vendite'!$B$2:$B$7`, to its cells; `None` when nothing does.
pub(crate) type CellSource<'s> = &'s dyn Fn(&str) -> Option<Vec<ChartCell>>;

/// A source for charts outside a workbook, whose formulas point at an embedded one this reader
/// does not open.
pub(crate) fn no_cells(_: &str) -> Option<Vec<ChartCell>> {
    None
}

/// What a data label shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DataLabels {
    pub value: bool,
    pub percent: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Plot {
    pub kind: PlotKind,
    pub grouping: Grouping,
    pub series: Vec<Series>,
    /// The category labels, shared by the series of the plot.
    pub categories: Vec<String>,
    /// The space between groups of bars, as a percentage of a bar.
    pub gap_percent: u32,
    /// How much bars of one group overlap, in percent; negative for space between them.
    pub overlap_percent: i32,
    /// The plot is drawn on the secondary value axis.
    pub secondary: bool,
    /// Every point of a pie or a single series takes its own colour.
    pub vary_colours: bool,
    pub first_slice_degrees: u32,
}

/// A value axis as the file sets it.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ValueAxis {
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub major_unit: Option<f64>,
    pub format_code: Option<String>,
    pub hidden: bool,
    pub gridlines: bool,
    pub title: Option<String>,
    /// Between: categories sit in the middle of their band; otherwise on the band edges.
    pub between: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Chart {
    pub title: Option<String>,
    pub plots: Vec<Plot>,
    pub legend: Option<LegendPosition>,
    pub primary: ValueAxis,
    pub secondary: ValueAxis,
    /// The x axis of a scatter chart.
    pub x_axis: ValueAxis,
    pub category_axis_hidden: bool,
    pub category_title: Option<String>,
    /// Categories read from the last to the first.
    pub categories_reversed: bool,
    pub locale: NumberLocale,
    /// The size of labels and legend, and of the title, in points.
    pub label_size: f64,
    pub title_size: f64,
}

const EMU_PER_POINT: f64 = 12_700.0;
/// Office's chart text without a size of its own: 9 points, the title 14.
const DEFAULT_LABEL_SIZE: f64 = 9.0;
const TITLE_SCALE: f64 = 14.0 / 9.0;

/// A chart drawn into a frame of `width_emu` by `height_emu`: the SVG and the chart's title, or
/// `None` when the part holds nothing this reader draws.
pub(crate) fn chart_picture(
    xml: &str,
    theme: &Theme,
    width_emu: i64,
    height_emu: i64,
    cells: CellSource<'_>,
) -> Option<(Vec<u8>, Option<String>)> {
    let chart = read_chart(xml, theme, NumberLocale::default(), cells)?;
    #[expect(clippy::cast_precision_loss, reason = "frame sizes fit in f64")]
    let (width, height) = (
        width_emu as f64 / EMU_PER_POINT,
        height_emu as f64 / EMU_PER_POINT,
    );
    let svg = chart_svg(&chart, width, height);
    Some((svg.into_bytes(), chart.title))
}

/// An `OpenDocument` chart object drawn into a frame: the SVG and the chart's title.
pub(crate) fn odf_chart_picture(
    xml: &str,
    width_emu: i64,
    height_emu: i64,
) -> Option<(Vec<u8>, Option<String>)> {
    let chart = odf::read_odf_chart(xml, NumberLocale::default())?;
    #[expect(clippy::cast_precision_loss, reason = "frame sizes fit in f64")]
    let (width, height) = (
        width_emu as f64 / EMU_PER_POINT,
        height_emu as f64 / EMU_PER_POINT,
    );
    Some((chart_svg(&chart, width, height).into_bytes(), chart.title))
}

/// The content part of an `OpenDocument` object, `./Object 1` becoming `Object 1/content.xml`.
pub(crate) fn odf_object_part(href: &str) -> Option<String> {
    let name = href.trim_start_matches("./").trim_end_matches('/');
    (!name.is_empty() && !name.contains("://") && !name.contains(".."))
        .then(|| format!("{name}/content.xml"))
}

/// A chart part, `c:chartSpace`; `None` when it holds nothing this reader draws.
#[expect(
    clippy::too_many_lines,
    reason = "one pass reads the plots, their series and the axes they share"
)]
pub(crate) fn read_chart(
    xml: &str,
    theme: &Theme,
    locale: NumberLocale,
    cells: CellSource<'_>,
) -> Option<Chart> {
    let root = parse_tree(xml, |namespace, name| namespace == A && name == "t").ok()?;
    let chart = root.child(C, "chart")?;
    let plot_area = chart.child(C, "plotArea")?;

    let axes: Vec<&Element> = plot_area
        .elements()
        .filter(|axis| {
            axis.namespace == C
                && matches!(axis.name.as_str(), "valAx" | "catAx" | "dateAx" | "serAx")
        })
        .collect();
    let axis_by_id = |id: &str| {
        axes.iter().copied().find(|axis| {
            axis.child(C, "axId")
                .and_then(|axis_id| axis_id.attribute(Namespace::None, "val"))
                == Some(id)
        })
    };
    let value_axes: Vec<&Element> = axes
        .iter()
        .copied()
        .filter(|axis| axis.name == "valAx")
        .collect();
    let primary_value_id = value_axes.first().and_then(|axis| axis_id(axis));

    let mut plots = Vec::new();
    let mut series_index = 0;
    // The category axis of the first plot drawn is the one the chart shows.
    let mut category_axis: Option<&Element> = None;
    for plot in plot_area.elements().filter(|plot| plot.namespace == C) {
        let kind = match plot.name.as_str() {
            "barChart" | "bar3DChart" => {
                if value(plot, "barDir") == Some("bar") {
                    PlotKind::Bar
                } else {
                    PlotKind::Column
                }
            }
            "lineChart" | "line3DChart" | "stockChart" => PlotKind::Line,
            "areaChart" | "area3DChart" => PlotKind::Area,
            "pieChart" | "pie3DChart" | "ofPieChart" => PlotKind::Pie,
            "doughnutChart" => PlotKind::Doughnut {
                hole_percent: value(plot, "holeSize")
                    .and_then(|hole| hole.trim_end_matches('%').parse().ok())
                    .unwrap_or(50)
                    .clamp(10, 90),
            },
            "scatterChart" => PlotKind::Scatter,
            "bubbleChart" => PlotKind::Bubble,
            "radarChart" => PlotKind::Radar {
                filled: value(plot, "radarStyle") == Some("filled"),
            },
            _ => continue,
        };
        let grouping = match value(plot, "grouping") {
            Some("stacked") => Grouping::Stacked,
            Some("percentStacked") => Grouping::Percent,
            _ => Grouping::Standard,
        };
        let plot_ids: Vec<&str> = plot
            .elements()
            .filter(|child| child.is(C, "axId"))
            .filter_map(|child| child.attribute(Namespace::None, "val"))
            .collect();
        let secondary = plot_ids
            .iter()
            .filter_map(|id| axis_by_id(id))
            .find(|axis| axis.name == "valAx")
            .and_then(|axis| axis_id(axis))
            .is_some_and(|id| Some(id) != primary_value_id);
        let line_markers = value(plot, "marker").is_none_or(|marker| marker != "0");
        let scatter_lines = !matches!(value(plot, "scatterStyle"), Some("marker"));
        let plot_labels = labels_of(plot.child(C, "dLbls"));
        let mut categories = Vec::new();
        let mut series = Vec::new();
        for element in plot.elements().filter(|element| element.is(C, "ser")) {
            if series.len() >= MAX_SERIES {
                break;
            }
            let index = element
                .child(C, "idx")
                .and_then(|index| index.attribute(Namespace::None, "val"))
                .and_then(|index| index.parse::<usize>().ok())
                .unwrap_or(series_index);
            series_index += 1;
            let properties = element.child(C, "spPr");
            let colour = properties
                .and_then(|properties| fill(properties, theme))
                .or_else(|| {
                    properties
                        .and_then(|properties| properties.child(A, "ln"))
                        .and_then(|line| line.child(A, "solidFill"))
                        .and_then(|fill| colour_in(fill, theme))
                })
                .unwrap_or_else(|| accent(theme, index));
            let line = properties
                .and_then(|properties| properties.child(A, "ln"))
                .is_none_or(|line| line.child(A, "noFill").is_none());
            let symbol = element
                .child(C, "marker")
                .and_then(|marker| marker.child(C, "symbol"))
                .and_then(|symbol| symbol.attribute(Namespace::None, "val"));
            let markers = match kind {
                PlotKind::Line => symbol.map_or(line_markers, |symbol| symbol != "none"),
                PlotKind::Scatter => symbol != Some("none"),
                PlotKind::Radar { filled: false } => symbol
                    .map_or(value(plot, "radarStyle") == Some("marker"), |symbol| {
                        symbol != "none"
                    }),
                _ => false,
            };
            let point_colours = element
                .elements()
                .filter(|point| point.is(C, "dPt"))
                .filter_map(|point| {
                    let index = point
                        .child(C, "idx")?
                        .attribute(Namespace::None, "val")?
                        .parse::<usize>()
                        .ok()?;
                    let colour = fill(point.child(C, "spPr")?, theme)?;
                    Some((index, colour))
                })
                .collect();
            let (values, format_code) = numbers(
                element
                    .child(
                        C,
                        if matches!(kind, PlotKind::Scatter | PlotKind::Bubble) {
                            "yVal"
                        } else {
                            "val"
                        },
                    )
                    .or_else(|| element.child(C, "val")),
                cells,
            );
            let (x_values, _) = numbers(element.child(C, "xVal"), cells);
            let (sizes, _) = numbers(element.child(C, "bubbleSize"), cells);
            if categories.is_empty() {
                categories = element
                    .child(C, "cat")
                    .map(|category| category_labels(category, cells))
                    .unwrap_or_default();
            }
            let (labels, label_format) = element
                .child(C, "dLbls")
                .map_or_else(|| plot_labels.clone(), |labels| labels_of(Some(labels)));
            series.push(Series {
                name: series_name(element, cells).unwrap_or_else(|| format!("Serie {}", index + 1)),
                values,
                x_values,
                sizes,
                colour,
                point_colours,
                line: line
                    && kind != PlotKind::Bubble
                    && (kind != PlotKind::Scatter || scatter_lines),
                markers,
                labels,
                format_code,
                label_format,
            });
        }
        if series.is_empty() {
            continue;
        }
        let vary_colours = value(plot, "varyColors").map_or(
            matches!(kind, PlotKind::Pie | PlotKind::Doughnut { .. }),
            |vary| vary != "0",
        );
        if vary_colours && series.len() == 1 {
            let single = &mut series[0];
            for point in 0..single.values.len() {
                if !single
                    .point_colours
                    .iter()
                    .any(|(index, _)| *index == point)
                {
                    single.point_colours.push((point, accent(theme, point)));
                }
            }
        }
        let points = series
            .iter()
            .map(|series| series.values.len())
            .max()
            .unwrap_or(0);
        if categories.len() < points {
            for number in categories.len()..points {
                categories.push((number + 1).to_string());
            }
        }
        if plots.is_empty() {
            category_axis = plot_ids
                .iter()
                .filter_map(|id| axis_by_id(id))
                .find(|axis| axis.name != "valAx");
        }
        let bars = matches!(kind, PlotKind::Column | PlotKind::Bar);
        plots.push(Plot {
            kind,
            grouping,
            series,
            categories,
            gap_percent: value(plot, "gapWidth")
                .and_then(|gap| gap.trim_end_matches('%').parse().ok())
                .unwrap_or(150)
                .min(500),
            overlap_percent: value(plot, "overlap")
                .and_then(|overlap| overlap.trim_end_matches('%').parse().ok())
                .unwrap_or(if bars && grouping != Grouping::Standard {
                    100
                } else {
                    0
                })
                .clamp(-100, 100),
            secondary,
            vary_colours,
            first_slice_degrees: value(plot, "firstSliceAng")
                .and_then(|angle| angle.parse().ok())
                .unwrap_or(0)
                % 360,
        });
    }
    if plots.is_empty() {
        return None;
    }

    let scatter = plots
        .iter()
        .any(|plot| matches!(plot.kind, PlotKind::Scatter | PlotKind::Bubble));
    let nth_value_axis = |position: usize| {
        value_axes
            .iter()
            .copied()
            .filter(|axis| {
                // A scatter chart's x axis is a value axis too: it is the one at the bottom.
                !scatter || !matches!(value(axis, "axPos"), Some("b" | "t"))
            })
            .nth(position)
            .map(read_value_axis)
            .unwrap_or_default()
    };
    let x_axis = if scatter {
        value_axes
            .iter()
            .copied()
            .find(|axis| matches!(value(axis, "axPos"), Some("b" | "t")))
            .map(read_value_axis)
            .unwrap_or_default()
    } else {
        ValueAxis::default()
    };
    let category_axis =
        category_axis.or_else(|| axes.iter().copied().find(|axis| axis.name != "valAx"));
    let title = if value(chart, "autoTitleDeleted") == Some("1") {
        None
    } else {
        chart.child(C, "title").and_then(|title| {
            rich_text(title).or_else(|| {
                (plots.len() == 1 && plots[0].series.len() == 1)
                    .then(|| plots[0].series[0].name.clone())
            })
        })
    };
    let label_size = text_size(root.child(C, "txPr")).unwrap_or(DEFAULT_LABEL_SIZE);
    let title_size = text_size(chart.child(C, "title").and_then(|title| {
        title
            .descendant(C, "rich")
            .or_else(|| title.child(C, "txPr"))
    }))
    .unwrap_or(label_size * TITLE_SCALE);
    Some(Chart {
        title,
        legend: chart
            .child(C, "legend")
            .map(|legend| match value(legend, "legendPos") {
                Some("l") => LegendPosition::Left,
                Some("t") => LegendPosition::Top,
                Some("b") => LegendPosition::Bottom,
                _ => LegendPosition::Right,
            }),
        primary: nth_value_axis(0),
        secondary: nth_value_axis(1),
        x_axis,
        category_axis_hidden: category_axis.is_some_and(|axis| value(axis, "delete") == Some("1")),
        category_title: category_axis
            .and_then(|axis| axis.child(C, "title"))
            .and_then(rich_text),
        categories_reversed: category_axis.is_some_and(|axis| {
            axis.child(C, "scaling")
                .and_then(|scaling| scaling.child(C, "orientation"))
                .and_then(|orientation| orientation.attribute(Namespace::None, "val"))
                == Some("maxMin")
        }),
        plots,
        locale,
        label_size,
        title_size,
    })
}

fn axis_id(axis: &Element) -> Option<&str> {
    axis.child(C, "axId")?.attribute(Namespace::None, "val")
}

/// The `val` of a child element of the chart namespace.
fn value<'e>(parent: &'e Element, name: &str) -> Option<&'e str> {
    parent.child(C, name)?.attribute(Namespace::None, "val")
}

fn read_value_axis(axis: &Element) -> ValueAxis {
    let scaling = axis.child(C, "scaling");
    let number = |name: &str| {
        scaling
            .and_then(|scaling| value(scaling, name))
            .and_then(|number| number.parse::<f64>().ok())
            .filter(|number| number.is_finite())
    };
    ValueAxis {
        minimum: number("min"),
        maximum: number("max"),
        major_unit: value(axis, "majorUnit")
            .and_then(|unit| unit.parse::<f64>().ok())
            .filter(|unit| unit.is_finite() && *unit > 0.0),
        format_code: axis
            .child(C, "numFmt")
            .filter(|format| format.attribute(Namespace::None, "sourceLinked") != Some("1"))
            .and_then(|format| format.attribute(Namespace::None, "formatCode"))
            .filter(|code| !code.is_empty())
            .map(str::to_owned),
        hidden: value(axis, "delete") == Some("1"),
        gridlines: axis.child(C, "majorGridlines").is_some(),
        title: axis.child(C, "title").and_then(rich_text),
        between: value(axis, "crossBetween").map(|between| between == "between"),
    }
}

/// What data labels show, and the format they show values in.
fn labels_of(labels: Option<&Element>) -> (DataLabels, Option<String>) {
    let Some(labels) = labels else {
        return (DataLabels::default(), None);
    };
    if value(labels, "delete") == Some("1") {
        return (DataLabels::default(), None);
    }
    let format = labels
        .child(C, "numFmt")
        .filter(|format| format.attribute(Namespace::None, "sourceLinked") != Some("1"))
        .and_then(|format| format.attribute(Namespace::None, "formatCode"))
        .filter(|code| !code.is_empty() && *code != "General")
        .map(str::to_owned);
    (
        DataLabels {
            value: value(labels, "showVal") == Some("1"),
            percent: value(labels, "showPercent") == Some("1"),
        },
        format,
    )
}

/// The size of a text properties element's first run, in points.
fn text_size(parent: Option<&Element>) -> Option<f64> {
    let parent = parent?;
    parent
        .descendant(A, "defRPr")
        .or_else(|| parent.descendant(A, "rPr"))?
        .attribute(Namespace::None, "sz")?
        .parse::<u32>()
        .ok()
        .filter(|size| (100..=40_000).contains(size))
        .map(|size| f64::from(size) / 100.0)
}

/// The cells a formula of a reference points at, `c:f` under a `c:numRef` or `c:strRef`.
fn formula_cells(reference: &Element, cells: CellSource<'_>) -> Option<Vec<ChartCell>> {
    let formula = reference.descendant(C, "f")?.text();
    let mut found = cells(formula.trim())?;
    found.truncate(MAX_POINTS);
    Some(found)
}

/// The colour a shape's properties fill with.
fn fill(properties: &Element, theme: &Theme) -> Option<String> {
    properties
        .elements()
        .find_map(|fill| match fill.name.as_str() {
            "solidFill" => colour_in(fill, theme),
            "gradFill" => fill
                .descendant(A, "gs")
                .and_then(|stop| colour_in(stop, theme)),
            "pattFill" => fill
                .child(A, "fgClr")
                .and_then(|colour| colour_in(colour, theme)),
            _ => None,
        })
}

/// The colour Office gives the `index`th series: the six accents, then darker and lighter
/// rounds of them.
pub(crate) fn accent(theme: &Theme, index: usize) -> String {
    let base = theme
        .scheme(&format!("accent{}", index % 6 + 1))
        .unwrap_or_else(|| "4472C4".to_owned());
    let round = index / 6;
    if round == 0 {
        return base;
    }
    let factor = if round % 2 == 1 { 0.6 } else { 1.4 };
    let channel = |start: usize| {
        u8::from_str_radix(base.get(start..start + 2).unwrap_or("00"), 16).unwrap_or(0)
    };
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "channels are clamped to 0..=255"
    )]
    let scaled = |channel: u8| (f64::from(channel) * factor).round().clamp(0.0, 255.0) as u8;
    format!(
        "{:02X}{:02X}{:02X}",
        scaled(channel(0)),
        scaled(channel(2)),
        scaled(channel(4))
    )
}

/// The cached numbers of a `c:val`, `c:yVal` or `c:xVal`, with their format code.
fn numbers(
    reference: Option<&Element>,
    cells: CellSource<'_>,
) -> (Vec<Option<f64>>, Option<String>) {
    let Some(reference) = reference else {
        return (Vec::new(), None);
    };
    let cache = reference
        .descendant(C, "numCache")
        .or_else(|| reference.descendant(C, "numLit"))
        .filter(|cache| cache.child(C, "pt").is_some());
    let Some(cache) = cache else {
        if reference.descendant(C, "numRef").is_some()
            && let Some(found) = formula_cells(reference, cells)
        {
            let values = found
                .into_iter()
                .map(|cell| match cell {
                    ChartCell::Number(number) => Some(number),
                    ChartCell::Text(_) | ChartCell::Empty => None,
                })
                .collect();
            return (values, None);
        }
        // A scatter series may take its x from text: the points are then numbered.
        let labels = category_labels(reference, cells);
        let numbered = (1..=labels.len())
            .map(|number| u32::try_from(number).ok().map(f64::from))
            .collect();
        return (numbered, None);
    };
    let format_code = cache
        .child(C, "formatCode")
        .map(Element::text)
        .filter(|code| !code.is_empty() && code != "General");
    let count = cache
        .child(C, "ptCount")
        .and_then(|count| count.attribute(Namespace::None, "val"))
        .and_then(|count| count.parse::<usize>().ok())
        .unwrap_or(0);
    let mut values: Vec<Option<f64>> = Vec::new();
    for point in cache.elements().filter(|point| point.is(C, "pt")) {
        let Some(index) = point
            .attribute(Namespace::None, "idx")
            .and_then(|index| index.parse::<usize>().ok())
            .filter(|index| *index < MAX_POINTS)
        else {
            continue;
        };
        if values.len() <= index {
            values.resize(index + 1, None);
        }
        values[index] = point
            .child(C, "v")
            .and_then(|number| number.text().trim().parse::<f64>().ok())
            .filter(|number| number.is_finite());
    }
    if values.len() < count.min(MAX_POINTS) {
        values.resize(count.min(MAX_POINTS), None);
    }
    (values, format_code)
}

/// The labels of a `c:cat`: text, numbers or the last level of a multi-level reference.
fn category_labels(category: &Element, cells: CellSource<'_>) -> Vec<String> {
    let cache = category
        .descendant(C, "strCache")
        .or_else(|| category.descendant(C, "strLit"))
        .or_else(|| category.descendant(C, "numCache"))
        .or_else(|| category.descendant(C, "numLit"))
        .or_else(|| category.descendant(C, "lvl"))
        .filter(|cache| cache.child(C, "pt").is_some());
    let Some(cache) = cache else {
        return formula_cells(category, cells)
            .unwrap_or_default()
            .into_iter()
            .map(|cell| match cell {
                ChartCell::Number(number) => general_number(number, NumberLocale::default()),
                ChartCell::Text(text) => short(&text),
                ChartCell::Empty => String::new(),
            })
            .collect();
    };
    let mut labels: Vec<String> = Vec::new();
    for point in cache.elements().filter(|point| point.is(C, "pt")) {
        let Some(index) = point
            .attribute(Namespace::None, "idx")
            .and_then(|index| index.parse::<usize>().ok())
            .filter(|index| *index < MAX_POINTS)
        else {
            continue;
        };
        if labels.len() <= index {
            labels.resize(index + 1, String::new());
        }
        labels[index] = point
            .child(C, "v")
            .map(Element::text)
            .map(|text| short(&text))
            .unwrap_or_default();
    }
    labels
}

fn series_name(series: &Element, cells: CellSource<'_>) -> Option<String> {
    let text = series.child(C, "tx")?;
    let name = text
        .descendant(C, "v")
        .map(Element::text)
        .or_else(|| rich_text(text))
        .or_else(|| {
            formula_cells(text, cells)?
                .into_iter()
                .find_map(|cell| match cell {
                    ChartCell::Text(text) => Some(text),
                    ChartCell::Number(number) => {
                        Some(general_number(number, NumberLocale::default()))
                    }
                    ChartCell::Empty => None,
                })
        })?;
    let name = short(name.trim());
    (!name.is_empty()).then_some(name)
}

/// The text of a title or series name written as rich text.
fn rich_text(parent: &Element) -> Option<String> {
    let rich = parent
        .descendant(C, "rich")
        .or_else(|| parent.descendant(C, "strCache"))?;
    let mut paragraphs = Vec::new();
    for paragraph in rich.elements().filter(|paragraph| paragraph.is(A, "p")) {
        let mut text = String::new();
        collect_runs(paragraph, &mut text);
        paragraphs.push(text);
    }
    if paragraphs.is_empty() {
        paragraphs.push(
            rich.descendant(C, "v")
                .map(Element::text)
                .unwrap_or_default(),
        );
    }
    let text = short(paragraphs.join(" ").trim());
    (!text.is_empty()).then_some(text)
}

fn collect_runs(element: &Element, text: &mut String) {
    for child in element.elements() {
        if child.is(A, "t") {
            text.push_str(&child.text());
        } else {
            collect_runs(child, text);
        }
    }
}

fn short(text: &str) -> String {
    text.chars().take(MAX_TEXT).collect()
}

#[cfg(test)]
mod tests;
