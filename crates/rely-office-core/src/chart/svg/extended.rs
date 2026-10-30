// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! The charts of Office 2016 drawn as SVG: a waterfall's floating bars and their totals, a
//! funnel, a treemap's nested rectangles, a sunburst's rings, a histogram's bins with its
//! Pareto line, and the boxes and whiskers of a distribution.

use std::fmt::Write;

use super::{
    Area, FONT, GRID_COLOUR, LINE_WIDTH, PAD, Scale, TEXT_COLOUR, count, fit, legend_layout, line,
    number, scale_of, text, text_coloured, text_width,
};
use crate::chart::ValueAxis;
use crate::chart::extended::{Binning, ExtendedChart, ExtendedKind, ExtendedSeries};
use crate::number_format::format_number;

/// The colour of a waterfall's totals, Office's grey, when the theme has fewer than three.
const TOTAL_COLOUR: &str = "A5A5A5";

/// The SVG of a chart of the 2016 kinds filling `width` by `height` points.
pub(crate) fn extended_svg(chart: &ExtendedChart, width: f64, height: f64) -> String {
    let width = width.clamp(20.0, 5_000.0);
    let height = height.clamp(20.0, 5_000.0);
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{0}pt" height="{1}pt" viewBox="0 0 {0} {1}" font-family="{FONT}">"#,
        number(width),
        number(height)
    );
    let _ = write!(
        svg,
        r##"<rect x="0" y="0" width="{}" height="{}" fill="#FFFFFF"/>"##,
        number(width),
        number(height)
    );
    let mut area = Area {
        x: PAD,
        y: PAD,
        width: width - 2.0 * PAD,
        height: height - 2.0 * PAD,
    };
    if let Some(title) = &chart.title {
        text(
            &mut svg,
            width / 2.0,
            area.y + chart.title_size * 0.9,
            chart.title_size,
            "middle",
            title,
        );
        area.y += chart.title_size * 1.6;
        area.height -= chart.title_size * 1.6;
    }
    let first = &chart.series[0];
    if let Some(position) = chart.legend {
        let entries = legend_entries(chart, first);
        area = legend_layout(&mut svg, &entries, chart.label_size, position, area);
    }
    if area.width >= 10.0 && area.height >= 10.0 {
        match first.kind {
            ExtendedKind::Waterfall => waterfall(&mut svg, chart, first, area),
            ExtendedKind::Funnel => funnel(&mut svg, chart, first, area),
            ExtendedKind::Treemap => treemap(&mut svg, chart, first, area),
            ExtendedKind::Sunburst => sunburst(&mut svg, chart, first, area),
            ExtendedKind::Histogram | ExtendedKind::ParetoLine => {
                histogram(&mut svg, chart, first, area);
            }
            ExtendedKind::BoxWhisker => box_whisker(&mut svg, chart, area),
        }
    }
    svg.push_str("</svg>");
    svg
}

fn palette(chart: &ExtendedChart, index: usize) -> String {
    chart
        .palette
        .get(index % chart.palette.len().max(1))
        .cloned()
        .unwrap_or_else(|| "4472C4".to_owned())
}

fn legend_entries(chart: &ExtendedChart, first: &ExtendedSeries) -> Vec<(String, String)> {
    match first.kind {
        ExtendedKind::Waterfall => vec![
            ("Aumento".to_owned(), palette(chart, 0)),
            ("Riduzione".to_owned(), palette(chart, 1)),
            (
                "Totale".to_owned(),
                chart
                    .palette
                    .get(2)
                    .cloned()
                    .unwrap_or_else(|| TOTAL_COLOUR.to_owned()),
            ),
        ],
        ExtendedKind::Treemap | ExtendedKind::Sunburst => groups(first)
            .iter()
            .enumerate()
            .map(|(index, (name, _))| (name.clone(), palette(chart, index)))
            .collect(),
        _ => chart
            .series
            .iter()
            .map(|series| (series.name.clone(), series.colour.clone()))
            .collect(),
    }
}

fn format(chart: &ExtendedChart, series: &ExtendedSeries, value: f64) -> String {
    format_number(
        value,
        series.format_code.as_deref().unwrap_or("General"),
        chart.locale,
    )
    .text
}

/// Gridlines and labels of a value scale at the left of `area`, room below for the category
/// labels; the area left for the plot.
fn value_axis(
    svg: &mut String,
    chart: &ExtendedChart,
    series: &ExtendedSeries,
    scale: Scale,
    area: Area,
) -> Area {
    let size = chart.label_size;
    let ticks = scale.ticks();
    let labels: Vec<String> = ticks
        .iter()
        .map(|tick| format(chart, series, *tick))
        .collect();
    let widest = labels
        .iter()
        .map(|label| text_width(label, size))
        .fold(0.0, f64::max);
    let plot = Area {
        x: area.x + widest + 4.0,
        y: area.y + size * 0.5,
        width: (area.width - widest - 4.0).max(1.0),
        height: (area.height - size * 2.2).max(1.0),
    };
    for (tick, label) in ticks.iter().zip(&labels) {
        let y = plot.bottom() - scale.fraction(*tick) * plot.height;
        line(svg, plot.x, y, plot.right(), y, GRID_COLOUR, 0.75);
        text(svg, plot.x - 4.0, y + size * 0.35, size, "end", label);
    }
    plot
}

/// The labels of bands of equal width under a plot.
fn band_labels(svg: &mut String, chart: &ExtendedChart, plot: Area, labels: &[String]) {
    let band = plot.width / count(labels.len().max(1));
    for (index, label) in labels.iter().enumerate() {
        text(
            svg,
            plot.x + band * (count(index) + 0.5),
            plot.bottom() + chart.label_size * 1.3,
            chart.label_size,
            "middle",
            &fit(label, band, chart.label_size),
        );
    }
}

fn rect(svg: &mut String, x: f64, y: f64, width: f64, height: f64, colour: &str) {
    let _ = write!(
        svg,
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#{colour}\"/>",
        number(x),
        number(y),
        number(width.max(0.0)),
        number(height.max(0.0))
    );
}

fn point_colour(series: &ExtendedSeries, index: usize, default: String) -> String {
    series
        .point_colours
        .iter()
        .find(|(point, _)| *point == index)
        .map_or(default, |(_, colour)| colour.clone())
}

fn labels_of(series: &ExtendedSeries) -> Vec<String> {
    let level = series.categories.first();
    (0..series.values.len())
        .map(|index| {
            level
                .and_then(|level| level.get(index))
                .cloned()
                .unwrap_or_else(|| (index + 1).to_string())
        })
        .collect()
}

/// Bars that float from the running total, each total drawn from zero, with the lines that
/// join one bar to the next.
fn waterfall(svg: &mut String, chart: &ExtendedChart, series: &ExtendedSeries, area: Area) {
    let mut running = 0.0;
    let mut bars = Vec::with_capacity(series.values.len());
    for (index, value) in series.values.iter().enumerate() {
        let value = value.unwrap_or(0.0);
        if series.subtotals.contains(&index) {
            bars.push((0.0, value, true));
            running = value;
        } else {
            bars.push((running, running + value, false));
            running += value;
        }
    }
    if bars.is_empty() {
        return;
    }
    let low = bars
        .iter()
        .map(|(start, end, _)| start.min(*end))
        .fold(0.0, f64::min);
    let high = bars
        .iter()
        .map(|(start, end, _)| start.max(*end))
        .fold(0.0, f64::max);
    let scale = scale_of(low, high, &ValueAxis::default(), true);
    let plot = value_axis(svg, chart, series, scale, area);
    let band = plot.width / count(bars.len());
    let width = band / 1.5;
    let y_of = |value: f64| plot.bottom() - scale.fraction(value) * plot.height;
    let total = chart
        .palette
        .get(2)
        .cloned()
        .unwrap_or_else(|| TOTAL_COLOUR.to_owned());
    for (index, (start, end, subtotal)) in bars.iter().enumerate() {
        let x = plot.x + band * count(index) + (band - width) / 2.0;
        let (top, bottom) = (y_of(start.max(*end)), y_of(start.min(*end)));
        let default = if *subtotal {
            total.clone()
        } else if end >= start {
            palette(chart, 0)
        } else {
            palette(chart, 1)
        };
        rect(
            svg,
            x,
            top,
            width,
            bottom - top,
            &point_colour(series, index, default),
        );
        if let Some((next, _, _)) = bars.get(index + 1) {
            let y = y_of(if bars[index + 1].2 { *end } else { *next });
            let _ = write!(
                svg,
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#A6A6A6\" stroke-width=\"0.75\" stroke-dasharray=\"2 2\"/>",
                number(x + width),
                number(y),
                number(x + band),
                number(y)
            );
        }
        if series.labels {
            let value = series.values[index].unwrap_or(0.0);
            let y = if end >= start {
                top - 3.0
            } else {
                bottom + chart.label_size
            };
            text(
                svg,
                x + width / 2.0,
                y,
                chart.label_size,
                "middle",
                &format(chart, series, value),
            );
        }
    }
    band_labels(svg, chart, plot, &labels_of(series));
}

/// Bars centred on one axis, each as wide as its value, the categories at the left.
fn funnel(svg: &mut String, chart: &ExtendedChart, series: &ExtendedSeries, area: Area) {
    let labels = labels_of(series);
    let size = chart.label_size;
    let left = labels
        .iter()
        .map(|label| text_width(label, size))
        .fold(0.0, f64::max)
        .min(area.width * 0.3);
    let plot = Area {
        x: area.x + left + 6.0,
        width: (area.width - left - 6.0).max(1.0),
        ..area
    };
    let largest = series.values.iter().flatten().copied().fold(0.0, f64::max);
    if largest <= 0.0 {
        return;
    }
    let row = plot.height / count(series.values.len().max(1));
    for (index, value) in series.values.iter().enumerate() {
        let value = value.unwrap_or(0.0).max(0.0);
        let width = value / largest * plot.width;
        let y = plot.y + row * count(index);
        let middle = y + row / 2.0;
        rect(
            svg,
            plot.x + (plot.width - width) / 2.0,
            y + row * 0.06,
            width,
            row * 0.88,
            &point_colour(series, index, series.colour.clone()),
        );
        text(
            svg,
            area.x + left,
            middle + size * 0.35,
            size,
            "end",
            &fit(&labels[index], left, size),
        );
        if series.labels {
            text_coloured(
                svg,
                plot.x + plot.width / 2.0,
                middle + size * 0.35,
                size,
                "middle",
                &format(chart, series, value),
                if width > text_width(&format(chart, series, value), size) + 4.0 {
                    "#FFFFFF"
                } else {
                    TEXT_COLOUR
                },
            );
        }
    }
}

/// The top-level groups of a treemap or a sunburst, in the order they first appear, with the
/// points each holds.
fn groups(series: &ExtendedSeries) -> Vec<(String, Vec<usize>)> {
    let top = series.categories.last();
    let mut found: Vec<(String, Vec<usize>)> = Vec::new();
    for index in 0..series.values.len() {
        let name = top
            .and_then(|level| level.get(index))
            .filter(|name| !name.is_empty())
            .cloned()
            .unwrap_or_else(|| (index + 1).to_string());
        match found.iter_mut().find(|(known, _)| *known == name) {
            Some((_, points)) => points.push(index),
            None => found.push((name, vec![index])),
        }
    }
    found
}

/// Rectangles for weighted items laid out in rows along the shorter side, each row kept as
/// close to squares as it can be.
fn squarify(items: &[(usize, f64)], area: Area) -> Vec<(usize, Area)> {
    let mut sorted: Vec<(usize, f64)> = items
        .iter()
        .copied()
        .filter(|(_, weight)| *weight > 0.0)
        .collect();
    sorted.sort_by(|first, second| second.1.total_cmp(&first.1));
    let total: f64 = sorted.iter().map(|(_, weight)| weight).sum();
    if total <= 0.0 || area.width <= 0.0 || area.height <= 0.0 {
        return Vec::new();
    }
    let scale = area.width * area.height / total;
    let mut placed = Vec::new();
    let mut rest = area;
    let mut start = 0;
    while start < sorted.len() {
        let side = rest.width.min(rest.height);
        let worst = |row: &[(usize, f64)]| {
            let sum: f64 = row.iter().map(|(_, weight)| weight * scale).sum();
            row.iter()
                .map(|(_, weight)| {
                    let item = weight * scale;
                    let ratio = (side * side * item) / (sum * sum);
                    ratio.max(1.0 / ratio)
                })
                .fold(0.0, f64::max)
        };
        let mut end = start + 1;
        while end < sorted.len() && worst(&sorted[start..=end]) <= worst(&sorted[start..end]) {
            end += 1;
        }
        let row = &sorted[start..end];
        let sum: f64 = row.iter().map(|(_, weight)| weight * scale).sum();
        let thickness = sum / side;
        let mut offset = 0.0;
        for (index, weight) in row {
            let length = weight * scale / thickness;
            let rectangle = if rest.width >= rest.height {
                Area {
                    x: rest.x,
                    y: rest.y + offset,
                    width: thickness,
                    height: length,
                }
            } else {
                Area {
                    x: rest.x + offset,
                    y: rest.y,
                    width: length,
                    height: thickness,
                }
            };
            placed.push((*index, rectangle));
            offset += length;
        }
        rest = if rest.width >= rest.height {
            Area {
                x: rest.x + thickness,
                width: rest.width - thickness,
                ..rest
            }
        } else {
            Area {
                y: rest.y + thickness,
                height: rest.height - thickness,
                ..rest
            }
        };
        start = end;
    }
    placed
}

/// Nested rectangles: the groups by their totals, each group's points inside it, labelled
/// where they fit.
fn treemap(svg: &mut String, chart: &ExtendedChart, series: &ExtendedSeries, area: Area) {
    let groups = groups(series);
    let weights: Vec<(usize, f64)> = groups
        .iter()
        .enumerate()
        .map(|(index, (_, points))| {
            (
                index,
                points
                    .iter()
                    .map(|point| series.values[*point].unwrap_or(0.0).max(0.0))
                    .sum(),
            )
        })
        .collect();
    let size = chart.label_size;
    let leaves = series.categories.first();
    for (group, rectangle) in squarify(&weights, area) {
        let colour = palette(chart, group);
        let items: Vec<(usize, f64)> = groups[group]
            .1
            .iter()
            .map(|point| (*point, series.values[*point].unwrap_or(0.0).max(0.0)))
            .collect();
        for (point, leaf) in squarify(&items, rectangle) {
            let _ = write!(
                svg,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#{}\" stroke=\"#FFFFFF\" stroke-width=\"1\"/>",
                number(leaf.x),
                number(leaf.y),
                number(leaf.width),
                number(leaf.height),
                point_colour(series, point, colour.clone())
            );
            let label = leaves
                .and_then(|level| level.get(point))
                .cloned()
                .unwrap_or_default();
            if leaf.height > size * 1.4 && leaf.width > size * 2.0 {
                text_coloured(
                    svg,
                    leaf.x + 3.0,
                    leaf.y + size * 1.1,
                    size,
                    "start",
                    &fit(&label, leaf.width - 6.0, size),
                    "#FFFFFF",
                );
            }
        }
    }
}

/// A node of a sunburst's tree: its label, its total and the nodes under it.
#[derive(Debug, Default)]
struct Node {
    label: String,
    total: f64,
    children: Vec<Node>,
}

fn tree(series: &ExtendedSeries) -> Node {
    let mut root = Node::default();
    for (point, value) in series.values.iter().enumerate() {
        let value = value.unwrap_or(0.0).max(0.0);
        if value <= 0.0 {
            continue;
        }
        let path: Vec<String> = series
            .categories
            .iter()
            .rev()
            .filter_map(|level| level.get(point).filter(|label| !label.is_empty()).cloned())
            .collect();
        let mut node = &mut root;
        node.total += value;
        for label in path {
            let position = if let Some(position) =
                node.children.iter().position(|child| child.label == label)
            {
                position
            } else {
                node.children.push(Node {
                    label,
                    ..Node::default()
                });
                node.children.len() - 1
            };
            node = &mut node.children[position];
            node.total += value;
        }
    }
    root
}

fn depth(node: &Node) -> usize {
    node.children
        .iter()
        .map(depth)
        .max()
        .map_or(0, |deepest| deepest + 1)
}

/// A colour mixed towards white by `share`.
fn lighter(colour: &str, share: f64) -> String {
    let channel = |start: usize| {
        f64::from(
            u8::from_str_radix(colour.get(start..start + 2).unwrap_or("80"), 16).unwrap_or(128),
        )
    };
    let mut mixed = String::new();
    for start in [0, 2, 4] {
        let value = channel(start) + (255.0 - channel(start)) * share;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a channel is clamped to 0..=255"
        )]
        let value = value.round().clamp(0.0, 255.0) as u8;
        let _ = write!(mixed, "{value:02X}");
    }
    mixed
}

/// Rings, the top level innermost, each node an arc as wide as its share of its parent.
fn sunburst(svg: &mut String, chart: &ExtendedChart, series: &ExtendedSeries, area: Area) {
    let root = tree(series);
    let levels = depth(&root);
    if levels == 0 || root.total <= 0.0 {
        return;
    }
    let radius = area.width.min(area.height) / 2.0;
    let centre = (area.x + area.width / 2.0, area.y + area.height / 2.0);
    let ring = radius / (count(levels) + 0.6);
    let hole = ring * 0.6;
    let mut stack: Vec<(&Node, f64, usize, String)> = Vec::new();
    let mut start = -std::f64::consts::FRAC_PI_2;
    for (index, child) in root.children.iter().enumerate() {
        stack.push((child, start, 0, palette(chart, index)));
        start += child.total / root.total * std::f64::consts::TAU;
    }
    while let Some((node, from, level, colour)) = stack.pop() {
        let sweep = node.total / root.total * std::f64::consts::TAU;
        let inner = hole + ring * count(level);
        let outer = inner + ring;
        arc(
            svg,
            centre,
            (inner, outer),
            (from, from + sweep),
            &lighter(&colour, 0.18 * count(level)),
        );
        let middle = f64::midpoint(inner, outer);
        let size = chart.label_size;
        let angle = from + sweep / 2.0;
        // A level label crosses the ring as much as its angle turns it across.
        let space = ((ring - 4.0) / angle.cos().abs().max(1e-6))
            .min((sweep * middle - 4.0) / angle.sin().abs().max(1e-6));
        let label = fit(&node.label, space, size);
        if space > size && ring > size * 1.2 {
            text_coloured(
                svg,
                centre.0 + middle * angle.cos(),
                centre.1 + middle * angle.sin() + size * 0.35,
                size,
                "middle",
                &label,
                "#FFFFFF",
            );
        }
        let mut start = from;
        for child in &node.children {
            stack.push((child, start, level + 1, colour.clone()));
            start += child.total / root.total * std::f64::consts::TAU;
        }
    }
}

fn arc(
    svg: &mut String,
    centre: (f64, f64),
    (inner, outer): (f64, f64),
    (from, to): (f64, f64),
    colour: &str,
) {
    let point = |radius: f64, angle: f64| {
        (
            centre.0 + radius * angle.cos(),
            centre.1 + radius * angle.sin(),
        )
    };
    let large = u8::from(to - from > std::f64::consts::PI);
    if to - from >= std::f64::consts::TAU - 1e-9 {
        // A whole ring: two halves, as one arc cannot close on itself.
        let middle = from + std::f64::consts::PI;
        arc(svg, centre, (inner, outer), (from, middle), colour);
        arc(svg, centre, (inner, outer), (middle, to), colour);
        return;
    }
    let (outer_start, outer_end) = (point(outer, from), point(outer, to));
    let (inner_end, inner_start) = (point(inner, to), point(inner, from));
    let _ = write!(
        svg,
        "<path d=\"M{} {} A{} {} 0 {large} 1 {} {} L{} {} A{} {} 0 {large} 0 {} {} Z\" fill=\"#{colour}\" stroke=\"#FFFFFF\" stroke-width=\"1\"/>",
        number(outer_start.0),
        number(outer_start.1),
        number(outer),
        number(outer),
        number(outer_end.0),
        number(outer_end.1),
        number(inner_end.0),
        number(inner_end.1),
        number(inner),
        number(inner),
        number(inner_start.0),
        number(inner_start.1)
    );
}

/// The bars of a histogram: a category's total when the points are labelled, else the count
/// of each bin of the values, as Excel bins them.
fn histogram_bars(chart: &ExtendedChart, series: &ExtendedSeries) -> Vec<(String, f64)> {
    let labelled = series
        .categories
        .first()
        .is_some_and(|level| level.iter().any(|label| !label.is_empty()));
    if labelled {
        let labels = labels_of(series);
        let mut bars: Vec<(String, f64)> = Vec::new();
        for (label, value) in labels.into_iter().zip(&series.values) {
            let value = value.unwrap_or(0.0);
            match bars.iter_mut().find(|(known, _)| *known == label) {
                Some((_, total)) => *total += value,
                None => bars.push((label, value)),
            }
        }
        return bars;
    }
    let values: Vec<f64> = series.values.iter().flatten().copied().collect();
    if values.is_empty() {
        return Vec::new();
    }
    let low = values.iter().copied().fold(f64::INFINITY, f64::min);
    let high = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let width = match series.binning {
        Binning::Width(width) => width,
        Binning::Count(bins) => (high - low) / count(bins),
        Binning::Automatic => {
            let mean = values.iter().sum::<f64>() / count(values.len());
            let deviation = (values
                .iter()
                .map(|value| (value - mean).powi(2))
                .sum::<f64>()
                / count(values.len()))
            .sqrt();
            3.49 * deviation / count(values.len()).cbrt()
        }
    };
    let width = if width > 0.0 && width.is_finite() {
        width
    } else {
        1.0
    };
    let bins = super::usize_of(((high - low) / width).ceil()).clamp(1, 1_000);
    let mut counts = vec![0.0; bins];
    for value in &values {
        let position = ((value - low) / width).ceil() - 1.0;
        let index = if position < 0.0 {
            0
        } else {
            super::usize_of(position)
        };
        counts[index.min(bins - 1)] += 1.0;
    }
    let label = |value: f64| format_number(value, "General", chart.locale).text;
    counts
        .into_iter()
        .enumerate()
        .map(|(index, total)| {
            let from = low + width * count(index);
            let opening = if index == 0 { '[' } else { '(' };
            (
                format!("{opening}{}, {}]", label(from), label(from + width)),
                total,
            )
        })
        .collect()
}

/// A histogram's bars side by side, and its Pareto line over them when the chart has one: the
/// bars then sorted from the largest, the line their running share on a scale of 0 to 100%.
fn histogram(svg: &mut String, chart: &ExtendedChart, series: &ExtendedSeries, area: Area) {
    let pareto = chart
        .series
        .iter()
        .find(|series| series.kind == ExtendedKind::ParetoLine);
    let mut bars = histogram_bars(chart, series);
    if bars.is_empty() {
        return;
    }
    if pareto.is_some() {
        bars.sort_by(|first, second| second.1.total_cmp(&first.1));
    }
    let high = bars.iter().map(|(_, total)| *total).fold(0.0, f64::max);
    let scale = scale_of(0.0, high, &ValueAxis::default(), true);
    let mut plot = value_axis(svg, chart, series, scale, area);
    if pareto.is_some() {
        plot.width -= text_width("100%", chart.label_size) + 6.0;
    }
    let band = plot.width / count(bars.len());
    for (index, (_, total)) in bars.iter().enumerate() {
        let top = plot.bottom() - scale.fraction(*total) * plot.height;
        let x = plot.x + band * count(index);
        let _ = write!(
            svg,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#{}\" stroke=\"#FFFFFF\" stroke-width=\"0.75\"/>",
            number(x),
            number(top),
            number(band),
            number(plot.bottom() - top),
            point_colour(series, index, series.colour.clone())
        );
        if series.labels {
            text(
                svg,
                x + band / 2.0,
                top - 3.0,
                chart.label_size,
                "middle",
                &format_number(*total, "General", chart.locale).text,
            );
        }
    }
    let labels: Vec<String> = bars.iter().map(|(label, _)| label.clone()).collect();
    band_labels(svg, chart, plot, &labels);
    if let Some(pareto) = pareto {
        let sum: f64 = bars.iter().map(|(_, total)| total).sum();
        if sum > 0.0 {
            let mut running = 0.0;
            let mut points = String::new();
            for (index, (_, total)) in bars.iter().enumerate() {
                running += total;
                let _ = write!(
                    points,
                    "{},{} ",
                    number(plot.x + band * (count(index) + 0.5)),
                    number(plot.bottom() - running / sum * plot.height)
                );
            }
            let _ = write!(
                svg,
                "<polyline points=\"{}\" fill=\"none\" stroke=\"#{}\" stroke-width=\"{LINE_WIDTH}\"/>",
                points.trim_end(),
                pareto.colour
            );
            for share in [0.0, 0.5, 1.0] {
                text(
                    svg,
                    plot.right() + 4.0,
                    plot.bottom() - share * plot.height + chart.label_size * 0.35,
                    chart.label_size,
                    "start",
                    &format!("{}%", number(share * 100.0)),
                );
            }
        }
    }
}

/// The statistics of a box: whiskers, quartiles, median, mean and the values outside the
/// whiskers.
#[derive(Debug, Clone, PartialEq)]
struct BoxStatistics {
    low: f64,
    first: f64,
    median: f64,
    third: f64,
    high: f64,
    mean: f64,
    outliers: Vec<f64>,
}

/// A quantile of sorted values, `exclusive` as `QUARTILE.EXC` computes it, else as
/// `QUARTILE.INC`.
fn quantile(sorted: &[f64], share: f64, exclusive: bool) -> f64 {
    let count_of = count(sorted.len());
    let position = if exclusive {
        share * (count_of + 1.0) - 1.0
    } else {
        share * (count_of - 1.0)
    };
    let position = position.clamp(0.0, count_of - 1.0);
    let below = super::usize_of(position.floor());
    let above = (below + 1).min(sorted.len() - 1);
    sorted[below] + (sorted[above] - sorted[below]) * (position - position.floor())
}

fn statistics(values: &[f64], exclusive: bool) -> Option<BoxStatistics> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let first = quantile(&sorted, 0.25, exclusive && sorted.len() > 2);
    let median = quantile(&sorted, 0.5, exclusive && sorted.len() > 2);
    let third = quantile(&sorted, 0.75, exclusive && sorted.len() > 2);
    let reach = (third - first) * 1.5;
    let inside: Vec<f64> = sorted
        .iter()
        .copied()
        .filter(|value| *value >= first - reach && *value <= third + reach)
        .collect();
    Some(BoxStatistics {
        low: inside.first().copied().unwrap_or(first),
        first,
        median,
        third,
        high: inside.last().copied().unwrap_or(third),
        mean: sorted.iter().sum::<f64>() / count(sorted.len()),
        outliers: sorted
            .iter()
            .copied()
            .filter(|value| *value < first - reach || *value > third + reach)
            .collect(),
    })
}

/// A box for each series in each category, its whiskers, median, mean and outliers.
#[expect(
    clippy::too_many_lines,
    reason = "the statistics of every box, then the scale and the boxes they give"
)]
fn box_whisker(svg: &mut String, chart: &ExtendedChart, area: Area) {
    let boxes: Vec<&ExtendedSeries> = chart
        .series
        .iter()
        .filter(|series| series.kind == ExtendedKind::BoxWhisker)
        .collect();
    let mut categories: Vec<String> = Vec::new();
    for series in &boxes {
        for label in labels_of_groups(series) {
            if !categories.contains(&label) {
                categories.push(label);
            }
        }
    }
    let mut statistics_by: Vec<Vec<Option<BoxStatistics>>> = Vec::new();
    let (mut low, mut high) = (f64::INFINITY, f64::NEG_INFINITY);
    for series in &boxes {
        let labels = labels_of_groups(series);
        let row: Vec<Option<BoxStatistics>> = categories
            .iter()
            .map(|category| {
                let values: Vec<f64> = series
                    .values
                    .iter()
                    .zip(&labels)
                    .filter(|(_, label)| *label == category)
                    .filter_map(|(value, _)| *value)
                    .collect();
                statistics(&values, series.exclusive_quartiles)
            })
            .collect();
        for found in row.iter().flatten() {
            low = low
                .min(found.low)
                .min(found.outliers.iter().copied().fold(f64::INFINITY, f64::min));
            high = high.max(found.high).max(
                found
                    .outliers
                    .iter()
                    .copied()
                    .fold(f64::NEG_INFINITY, f64::max),
            );
        }
        statistics_by.push(row);
    }
    if !low.is_finite() || !high.is_finite() {
        return;
    }
    let scale = scale_of(low, high, &ValueAxis::default(), false);
    let plot = value_axis(svg, chart, boxes[0], scale, area);
    let band = plot.width / count(categories.len().max(1));
    let slot = band / (count(boxes.len()) + 1.0);
    let width = slot * 0.8;
    let y_of = |value: f64| plot.bottom() - scale.fraction(value) * plot.height;
    for (series_index, series) in boxes.iter().enumerate() {
        for (category, found) in statistics_by[series_index].iter().enumerate() {
            let Some(found) = found else {
                continue;
            };
            let middle = plot.x + band * count(category) + slot * (count(series_index) + 1.0);
            let colour = format!("#{}", series.colour);
            line(
                svg,
                middle,
                y_of(found.low),
                middle,
                y_of(found.first),
                &colour,
                1.0,
            );
            line(
                svg,
                middle,
                y_of(found.third),
                middle,
                y_of(found.high),
                &colour,
                1.0,
            );
            for end in [found.low, found.high] {
                line(
                    svg,
                    middle - width / 4.0,
                    y_of(end),
                    middle + width / 4.0,
                    y_of(end),
                    &colour,
                    1.0,
                );
            }
            rect(
                svg,
                middle - width / 2.0,
                y_of(found.third),
                width,
                y_of(found.first) - y_of(found.third),
                &series.colour,
            );
            line(
                svg,
                middle - width / 2.0,
                y_of(found.median),
                middle + width / 2.0,
                y_of(found.median),
                "#FFFFFF",
                1.5,
            );
            if series.mean_marker {
                let (x, y, arm) = (middle, y_of(found.mean), width * 0.12);
                line(svg, x - arm, y - arm, x + arm, y + arm, "#FFFFFF", 1.25);
                line(svg, x - arm, y + arm, x + arm, y - arm, "#FFFFFF", 1.25);
            }
            if series.outliers {
                for outlier in &found.outliers {
                    let _ = write!(
                        svg,
                        "<circle cx=\"{}\" cy=\"{}\" r=\"2\" fill=\"none\" stroke=\"{colour}\" stroke-width=\"1\"/>",
                        number(middle),
                        number(y_of(*outlier))
                    );
                }
            }
        }
    }
    if categories.len() > 1 || categories.first().is_some_and(|label| !label.is_empty()) {
        band_labels(svg, chart, plot, &categories);
    }
}

/// The category of each value of a box and whisker series, empty when it names none.
fn labels_of_groups(series: &ExtendedSeries) -> Vec<String> {
    let level = series.categories.first();
    (0..series.values.len())
        .map(|index| {
            level
                .and_then(|level| level.get(index))
                .cloned()
                .unwrap_or_default()
        })
        .collect()
}

#[cfg(test)]
mod tests;
