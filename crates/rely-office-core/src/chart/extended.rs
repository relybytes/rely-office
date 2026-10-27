// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The charts Office 2016 added, in a `cx:chartSpace`: waterfall, funnel, treemap, sunburst,
//! histogram, Pareto and box and whisker. Their data sits apart from their series, in
//! `cx:chartData`, each dimension with the values the file last computed.

use std::collections::HashMap;

use super::{LegendPosition, accent, fill};
use crate::number_format::NumberLocale;
use crate::theme::Theme;
use crate::xml_tree::{Element, Namespace};

const CX: Namespace = Namespace::ChartEx;
const A: Namespace = Namespace::Drawing;
const MAX_SERIES: usize = 64;
const MAX_POINTS: usize = 10_000;
const MAX_LEVELS: usize = 8;
const MAX_TEXT: usize = 200;
const DEFAULT_LABEL_SIZE: f64 = 9.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExtendedKind {
    Waterfall,
    Funnel,
    Treemap,
    Sunburst,
    Histogram,
    /// The cumulative share of a histogram's bins, a line over it.
    ParetoLine,
    BoxWhisker,
}

/// How a histogram groups its values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Binning {
    /// As many bins as Scott's rule gives, which is what Excel does unless told.
    Automatic,
    Count(usize),
    Width(f64),
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "the switches a file stores for each series"
)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExtendedSeries {
    pub kind: ExtendedKind,
    pub name: String,
    /// The labels of the points by level, the leaves first: a treemap's or a sunburst's
    /// levels above them are their parents.
    pub categories: Vec<Vec<String>>,
    pub values: Vec<Option<f64>>,
    pub colour: String,
    pub point_colours: Vec<(usize, String)>,
    pub labels: bool,
    /// The points of a waterfall that are totals, drawn from zero.
    pub subtotals: Vec<usize>,
    pub binning: Binning,
    /// A box and whisker's quartiles computed leaving out the median, as `QUARTILE.EXC`.
    pub exclusive_quartiles: bool,
    pub mean_marker: bool,
    pub outliers: bool,
    pub format_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExtendedChart {
    pub title: Option<String>,
    pub series: Vec<ExtendedSeries>,
    pub legend: Option<LegendPosition>,
    pub locale: NumberLocale,
    pub label_size: f64,
    pub title_size: f64,
    /// The colours of a treemap's or a sunburst's top-level groups, and of the waterfall's
    /// rises, falls and totals.
    pub palette: Vec<String>,
}

/// A dimension of `cx:chartData`: its levels of labels or values, each by point.
#[derive(Debug, Default)]
struct Dimension {
    levels: Vec<Vec<Option<String>>>,
    format_code: Option<String>,
}

fn dimension(element: &Element) -> Dimension {
    let mut dimension = Dimension::default();
    for level in element
        .elements()
        .filter(|level| level.is(CX, "lvl"))
        .take(MAX_LEVELS)
    {
        let count = level
            .attribute(Namespace::None, "ptCount")
            .and_then(|count| count.parse::<usize>().ok())
            .unwrap_or(0)
            .min(MAX_POINTS);
        if dimension.format_code.is_none() {
            dimension.format_code = level
                .attribute(Namespace::None, "formatCode")
                .filter(|code| !code.is_empty())
                .map(str::to_owned);
        }
        let mut points = vec![None; count];
        for point in level.elements().filter(|point| point.is(CX, "pt")) {
            let Some(index) = point
                .attribute(Namespace::None, "idx")
                .and_then(|index| index.parse::<usize>().ok())
                .filter(|index| *index < MAX_POINTS)
            else {
                continue;
            };
            if index >= points.len() {
                points.resize(index + 1, None);
            }
            points[index] = Some(point.text());
        }
        dimension.levels.push(points);
    }
    dimension
}

/// The text of a title or a series name: its rich text, else its cached value.
fn text_of(element: &Element) -> Option<String> {
    let mut paragraphs = Vec::new();
    for paragraph in element
        .descendant(CX, "rich")
        .into_iter()
        .flat_map(|rich| rich.elements().filter(|paragraph| paragraph.is(A, "p")))
    {
        let mut text = String::new();
        collect_text(paragraph, &mut text);
        paragraphs.push(text);
    }
    let text = if paragraphs.is_empty() {
        element.descendant(CX, "v").map(Element::text)?
    } else {
        paragraphs.join(" ")
    };
    let text: String = text.trim().chars().take(MAX_TEXT).collect();
    (!text.is_empty()).then_some(text)
}

fn collect_text(element: &Element, text: &mut String) {
    for child in element.elements() {
        if child.is(A, "t") {
            text.push_str(&child.text());
        } else {
            collect_text(child, text);
        }
    }
}

fn kind_of(layout: &str) -> Option<ExtendedKind> {
    Some(match layout {
        "waterfall" => ExtendedKind::Waterfall,
        "funnel" => ExtendedKind::Funnel,
        "treemap" => ExtendedKind::Treemap,
        "sunburst" => ExtendedKind::Sunburst,
        "clusteredColumn" => ExtendedKind::Histogram,
        "paretoLine" => ExtendedKind::ParetoLine,
        "boxWhisker" => ExtendedKind::BoxWhisker,
        _ => return None,
    })
}

fn value_of<'e>(parent: &'e Element, name: &str) -> Option<&'e str> {
    parent
        .descendant(CX, name)?
        .attribute(Namespace::None, "val")
}

/// A chart part of the 2016 kinds, `cx:chartSpace`; `None` when it holds nothing this reader
/// draws, a map among them.
#[expect(
    clippy::too_many_lines,
    reason = "one pass reads the data, then each series with its settings"
)]
pub(crate) fn read_extended_chart(
    root: &Element,
    theme: &Theme,
    locale: NumberLocale,
) -> Option<ExtendedChart> {
    let chart = root.child(CX, "chart")?;
    let data: HashMap<String, (Dimension, Dimension)> = root
        .child(CX, "chartData")
        .into_iter()
        .flat_map(|data| data.elements().filter(|data| data.is(CX, "data")))
        .filter_map(|data| {
            let id = data.attribute(Namespace::None, "id")?.to_owned();
            let labels = data
                .elements()
                .find(|dimension| dimension.is(CX, "strDim"))
                .map(dimension)
                .unwrap_or_default();
            let values = data
                .elements()
                .find(|dimension| dimension.is(CX, "numDim"))
                .map(dimension)
                .unwrap_or_default();
            Some((id, (labels, values)))
        })
        .collect();
    let region = chart.descendant(CX, "plotAreaRegion")?;
    let mut series = Vec::new();
    for (index, element) in region
        .elements()
        .filter(|series| series.is(CX, "series"))
        .take(MAX_SERIES)
        .enumerate()
    {
        let Some(kind) = element
            .attribute(Namespace::None, "layoutId")
            .and_then(kind_of)
        else {
            continue;
        };
        if element.attribute(Namespace::None, "hidden") == Some("1") {
            continue;
        }
        let empty = (Dimension::default(), Dimension::default());
        // A Pareto line has no data of its own: it draws the histogram it belongs to.
        let Some((labels, values)) = value_of(element, "dataId")
            .and_then(|id| data.get(id))
            .or((kind == ExtendedKind::ParetoLine).then_some(&empty))
        else {
            continue;
        };
        let numbers: Vec<Option<f64>> = values
            .levels
            .first()
            .map(|level| {
                level
                    .iter()
                    .map(|value| {
                        value
                            .as_deref()
                            .and_then(|value| value.trim().parse::<f64>().ok())
                            .filter(|value| value.is_finite())
                    })
                    .collect()
            })
            .unwrap_or_default();
        let categories: Vec<Vec<String>> = labels
            .levels
            .iter()
            .map(|level| {
                level
                    .iter()
                    .map(|label| label.clone().unwrap_or_default())
                    .collect()
            })
            .collect();
        let layout = element.child(CX, "layoutPr");
        let binning = layout
            .and_then(|layout| layout.child(CX, "binning"))
            .map_or(Binning::Automatic, |binning| {
                if let Some(count) = value_of(binning, "binCount")
                    .and_then(|count| count.parse::<usize>().ok())
                    .filter(|count| (1..=1_000).contains(count))
                {
                    Binning::Count(count)
                } else if let Some(width) = value_of(binning, "binSize")
                    .and_then(|width| width.parse::<f64>().ok())
                    .filter(|width| *width > 0.0 && width.is_finite())
                {
                    Binning::Width(width)
                } else {
                    Binning::Automatic
                }
            });
        let visibility = layout.and_then(|layout| layout.child(CX, "visibility"));
        let shown = |name: &str, default: bool| {
            visibility
                .and_then(|visibility| visibility.attribute(Namespace::None, name))
                .map_or(default, |value| value == "1" || value == "true")
        };
        let subtotals = layout
            .and_then(|layout| layout.child(CX, "subtotals"))
            .map(|subtotals| {
                subtotals
                    .elements()
                    .filter(|index| index.is(CX, "idx"))
                    .filter_map(|index| index.attribute(Namespace::None, "val")?.parse().ok())
                    .collect()
            })
            .unwrap_or_default();
        let point_colours = element
            .elements()
            .filter(|point| point.is(CX, "dataPt"))
            .filter_map(|point| {
                let index = point.attribute(Namespace::None, "idx")?.parse().ok()?;
                Some((index, fill(point.child(CX, "spPr")?, theme)?))
            })
            .collect();
        series.push(ExtendedSeries {
            kind,
            name: element
                .child(CX, "tx")
                .and_then(text_of)
                .unwrap_or_else(|| format!("Serie {}", index + 1)),
            categories,
            values: numbers,
            colour: element
                .child(CX, "spPr")
                .and_then(|properties| fill(properties, theme))
                .unwrap_or_else(|| accent(theme, index)),
            point_colours,
            labels: element.child(CX, "dataLabels").is_some_and(|labels| {
                labels
                    .child(CX, "visibility")
                    .and_then(|visibility| visibility.attribute(Namespace::None, "value"))
                    .is_none_or(|value| value == "1")
            }),
            subtotals,
            binning,
            exclusive_quartiles: layout
                .and_then(|layout| layout.child(CX, "statistics"))
                .and_then(|statistics| statistics.attribute(Namespace::None, "quartileMethod"))
                != Some("inclusive"),
            mean_marker: shown("meanMarker", true),
            outliers: shown("outliers", true),
            format_code: values.format_code.clone(),
        });
    }
    if series.is_empty() {
        return None;
    }
    let title = chart.child(CX, "title").and_then(text_of);
    let label_size = root
        .child(CX, "txPr")
        .and_then(|properties| properties.descendant(A, "defRPr"))
        .and_then(|properties| properties.attribute(Namespace::None, "sz"))
        .and_then(|size| size.parse::<u32>().ok())
        .filter(|size| (100..=40_000).contains(size))
        .map_or(DEFAULT_LABEL_SIZE, |size| f64::from(size) / 100.0);
    Some(ExtendedChart {
        title,
        series,
        legend: chart.child(CX, "legend").map(|legend| {
            match legend.attribute(Namespace::None, "pos") {
                Some("l") => LegendPosition::Left,
                Some("r") => LegendPosition::Right,
                Some("b") => LegendPosition::Bottom,
                _ => LegendPosition::Top,
            }
        }),
        locale,
        label_size,
        title_size: label_size * 14.0 / 9.0,
        palette: (0..6).map(|index| accent(theme, index)).collect(),
    })
}

#[cfg(test)]
pub(crate) mod tests;
