// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! A chart drawn as SVG, in points, with the look of Office's default chart style: grey labels,
//! light gridlines, the series in their colours.

use std::fmt::Write;

use super::{Chart, Grouping, LegendPosition, Plot, PlotKind, Series, ValueAxis};
use crate::number_format::format_number;

const FONT: &str = "Calibri, Carlito, Liberation Sans, Arial, sans-serif";
const TEXT_COLOUR: &str = "#595959";
const GRID_COLOUR: &str = "#D9D9D9";
const AXIS_COLOUR: &str = "#BFBFBF";
const PAD: f64 = 6.0;
/// An average glyph width, as a fraction of the font size, to measure text before it is set.
const GLYPH_WIDTH: f64 = 0.52;
const LINE_WIDTH: f64 = 2.25;
const MARKER_RADIUS: f64 = 2.5;
const MAX_TICKS: f64 = 20.0;

/// A rectangle in points.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Area {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Area {
    fn right(self) -> f64 {
        self.x + self.width
    }

    fn bottom(self) -> f64 {
        self.y + self.height
    }
}

/// The range a value axis spans and the distance between its ticks.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Scale {
    minimum: f64,
    maximum: f64,
    step: f64,
}

impl Scale {
    /// Where a value falls between 0 (the minimum) and 1 (the maximum).
    fn fraction(self, value: f64) -> f64 {
        let span = self.maximum - self.minimum;
        if span <= 0.0 {
            0.0
        } else {
            (value - self.minimum) / span
        }
    }

    fn ticks(self) -> Vec<f64> {
        let mut ticks = Vec::new();
        let mut index = 0.0;
        loop {
            let tick = self.minimum + self.step * index;
            if tick > self.maximum + self.step * 1e-9 || index > MAX_TICKS + 1.0 {
                break;
            }
            ticks.push(if tick.abs() < self.step * 1e-9 {
                0.0
            } else {
                tick
            });
            index += 1.0;
        }
        ticks
    }

    fn baseline(self) -> f64 {
        0.0_f64.clamp(self.minimum, self.maximum)
    }
}

/// The SVG of a chart filling `width` by `height` points.
pub(crate) fn chart_svg(chart: &Chart, width: f64, height: f64) -> String {
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
    let pies = chart
        .plots
        .iter()
        .all(|plot| matches!(plot.kind, PlotKind::Pie | PlotKind::Doughnut { .. }));
    if let Some(position) = chart.legend {
        area = legend(&mut svg, chart, pies, position, area);
    }
    if area.width < 10.0 || area.height < 10.0 {
        svg.push_str("</svg>");
        return svg;
    }
    if pies {
        pie(&mut svg, chart, &chart.plots[0], area);
    } else if chart
        .plots
        .iter()
        .all(|plot| matches!(plot.kind, PlotKind::Radar { .. }))
    {
        radar(&mut svg, chart, area);
    } else if chart
        .plots
        .iter()
        .all(|plot| matches!(plot.kind, PlotKind::Scatter | PlotKind::Bubble))
    {
        scatter(&mut svg, chart, area);
    } else {
        categories(&mut svg, chart, area);
    }
    svg.push_str("</svg>");
    svg
}

/// The entries of the legend: a pie names its points, every other chart its series.
fn legend_entries(chart: &Chart, pies: bool) -> Vec<(String, String)> {
    if pies {
        let plot = &chart.plots[0];
        let Some(series) = plot.series.first() else {
            return Vec::new();
        };
        return plot
            .categories
            .iter()
            .enumerate()
            .take(series.values.len().max(1))
            .map(|(index, category)| (category.clone(), point_colour(series, index)))
            .collect();
    }
    chart
        .plots
        .iter()
        .flat_map(|plot| plot.series.iter())
        .map(|series| (series.name.clone(), series.colour.clone()))
        .collect()
}

/// Draw the legend at its side of `area` and give back what remains for the plot.
fn legend(
    svg: &mut String,
    chart: &Chart,
    pies: bool,
    position: LegendPosition,
    area: Area,
) -> Area {
    let entries = legend_entries(chart, pies);
    if entries.is_empty() {
        return area;
    }
    let row = chart.label_size * 1.7;
    let swatch = chart.label_size * 0.8;
    let entry_width = |label: &str| swatch + 4.0 + text_width(label, chart.label_size);
    match position {
        LegendPosition::Right | LegendPosition::Left => {
            let widest = entries
                .iter()
                .map(|(label, _)| entry_width(label))
                .fold(0.0, f64::max)
                .min(area.width * 0.4);
            let shown = entries.len().min(usize_of(area.height / row).max(1));
            let total = row * count(shown);
            let x = if position == LegendPosition::Right {
                area.right() - widest
            } else {
                area.x
            };
            let mut y = area.y + (area.height - total).max(0.0) / 2.0;
            for (label, colour) in entries.iter().take(shown) {
                legend_entry(
                    svg,
                    x,
                    y + row / 2.0,
                    swatch,
                    chart.label_size,
                    label,
                    colour,
                    widest,
                );
                y += row;
            }
            let used = widest + PAD * 2.0;
            Area {
                x: if position == LegendPosition::Left {
                    area.x + used
                } else {
                    area.x
                },
                width: area.width - used,
                ..area
            }
        }
        LegendPosition::Top | LegendPosition::Bottom => {
            let mut lines: Vec<Vec<(&str, &str, f64)>> = vec![Vec::new()];
            let mut line_width = 0.0;
            for (label, colour) in &entries {
                let width = entry_width(label).min(area.width) + PAD * 2.0;
                if line_width + width > area.width && !lines.last().is_none_or(Vec::is_empty) {
                    lines.push(Vec::new());
                    line_width = 0.0;
                }
                line_width += width;
                if let Some(line) = lines.last_mut() {
                    line.push((label, colour, width));
                }
            }
            lines.truncate(3);
            let height = row * count(lines.len());
            let top = if position == LegendPosition::Top {
                area.y
            } else {
                area.bottom() - height
            };
            for (index, line) in lines.iter().enumerate() {
                let total: f64 = line.iter().map(|(_, _, width)| width).sum();
                let mut x = area.x + (area.width - total).max(0.0) / 2.0 + PAD;
                let y = top + row * count(index) + row / 2.0;
                for (label, colour, width) in line {
                    legend_entry(
                        svg,
                        x,
                        y,
                        swatch,
                        chart.label_size,
                        label,
                        colour,
                        width - PAD * 2.0,
                    );
                    x += width;
                }
            }
            Area {
                y: if position == LegendPosition::Top {
                    area.y + height + PAD / 2.0
                } else {
                    area.y
                },
                height: area.height - height - PAD / 2.0,
                ..area
            }
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "an entry is placed, sized and labelled"
)]
fn legend_entry(
    svg: &mut String,
    x: f64,
    middle: f64,
    swatch: f64,
    size: f64,
    label: &str,
    colour: &str,
    width: f64,
) {
    let _ = write!(
        svg,
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#{colour}\"/>",
        number(x),
        number(middle - swatch / 2.0),
        number(swatch),
        number(swatch)
    );
    let room = (width - swatch - 4.0).max(size);
    text(
        svg,
        x + swatch + 4.0,
        middle + size * 0.35,
        size,
        "start",
        &fit(label, room, size),
    );
}

/// The colour of one point: its own, else its series'.
fn point_colour(series: &Series, index: usize) -> String {
    if let Some((_, colour)) = series
        .point_colours
        .iter()
        .find(|(point, _)| *point == index)
    {
        return colour.clone();
    }
    series.colour.clone()
}

#[expect(
    clippy::too_many_lines,
    reason = "each slice is drawn, labelled and placed in one walk around the pie"
)]
fn pie(svg: &mut String, chart: &Chart, plot: &Plot, area: Area) {
    let Some(series) = plot.series.first() else {
        return;
    };
    let total: f64 = series
        .values
        .iter()
        .flatten()
        .filter(|value| **value > 0.0)
        .sum();
    if total <= 0.0 {
        return;
    }
    let radius = (area.width.min(area.height) / 2.0 - PAD).max(4.0);
    let centre = (area.x + area.width / 2.0, area.y + area.height / 2.0);
    let inner = match plot.kind {
        PlotKind::Doughnut { hole_percent } => radius * f64::from(hole_percent) / 100.0,
        _ => 0.0,
    };
    let point = |angle: f64, distance: f64| {
        let radians = (angle - 90.0).to_radians();
        (
            centre.0 + distance * radians.cos(),
            centre.1 + distance * radians.sin(),
        )
    };
    let mut angle = f64::from(plot.first_slice_degrees);
    let mut labels = Vec::new();
    for (index, value) in series.values.iter().enumerate() {
        let Some(value) = value.filter(|value| *value > 0.0) else {
            continue;
        };
        let sweep = value / total * 360.0;
        let colour = point_colour(series, index);
        if sweep >= 359.99 {
            let _ = write!(
                svg,
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"#{colour}\"/>",
                number(centre.0),
                number(centre.1),
                number(radius)
            );
            if inner > 0.0 {
                let _ = write!(
                    svg,
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"#FFFFFF\"/>",
                    number(centre.0),
                    number(centre.1),
                    number(inner)
                );
            }
        } else {
            let large = u8::from(sweep > 180.0);
            let start = point(angle, radius);
            let end = point(angle + sweep, radius);
            let mut path = format!(
                "M{} {}A{} {} 0 {large} 1 {} {}",
                number(start.0),
                number(start.1),
                number(radius),
                number(radius),
                number(end.0),
                number(end.1)
            );
            if inner > 0.0 {
                let inner_end = point(angle + sweep, inner);
                let inner_start = point(angle, inner);
                let _ = write!(
                    path,
                    "L{} {}A{} {} 0 {large} 0 {} {}Z",
                    number(inner_end.0),
                    number(inner_end.1),
                    number(inner),
                    number(inner),
                    number(inner_start.0),
                    number(inner_start.1)
                );
            } else {
                let _ = write!(path, "L{} {}Z", number(centre.0), number(centre.1));
            }
            let _ = write!(
                svg,
                "<path d=\"{path}\" fill=\"#{colour}\" stroke=\"#FFFFFF\" stroke-width=\"0.75\"/>"
            );
        }
        if series.labels.value || series.labels.percent {
            let mut label = Vec::new();
            if series.labels.value {
                let code = series
                    .label_format
                    .as_deref()
                    .or(series.format_code.as_deref());
                label.push(format_value(chart, code, value));
            }
            if series.labels.percent {
                label.push(format_number(value / total, "0%", chart.locale).text);
            }
            let distance = if inner > 0.0 {
                f64::midpoint(radius, inner)
            } else {
                radius * 0.65
            };
            labels.push((point(angle + sweep / 2.0, distance), label.join("; ")));
        }
        angle += sweep;
    }
    for ((x, y), label) in labels {
        text_coloured(
            svg,
            x,
            y + chart.label_size * 0.35,
            chart.label_size,
            "middle",
            &label,
            "#FFFFFF",
        );
    }
}

/// The values of each category a plot stacks or sets side by side, for the scale.
fn plot_extent(plot: &Plot) -> Option<(f64, f64)> {
    let mut extent: Option<(f64, f64)> = None;
    let mut widen = |value: f64| {
        extent = Some(extent.map_or((value, value), |(low, high)| {
            (low.min(value), high.max(value))
        }));
    };
    match plot.grouping {
        Grouping::Standard => {
            for value in plot
                .series
                .iter()
                .flat_map(|series| series.values.iter().flatten())
            {
                widen(*value);
            }
        }
        Grouping::Stacked | Grouping::Percent => {
            let points = plot
                .series
                .iter()
                .map(|series| series.values.len())
                .max()
                .unwrap_or(0);
            for index in 0..points {
                let (positive, negative) = stack_totals(plot, index);
                if plot.grouping == Grouping::Percent {
                    let whole = positive - negative;
                    if whole > 0.0 {
                        widen(positive / whole);
                        widen(negative / whole);
                    }
                } else {
                    widen(positive);
                    widen(negative);
                }
            }
        }
    }
    extent
}

/// The sums of the positive and the negative values of one category.
fn stack_totals(plot: &Plot, index: usize) -> (f64, f64) {
    plot.series
        .iter()
        .filter_map(|series| series.values.get(index).copied().flatten())
        .fold((0.0, 0.0), |(positive, negative), value| {
            if value >= 0.0 {
                (positive + value, negative)
            } else {
                (positive, negative + value)
            }
        })
}

/// A scale over `low..=high` with round ticks, the axis's own bounds winning.
fn scale_of(low: f64, high: f64, axis: &ValueAxis, include_zero: bool) -> Scale {
    let (mut low, mut high) = (low, high);
    if include_zero
        || (low >= 0.0 && low < high * 5.0 / 6.0)
        || (high <= 0.0 && high > low * 5.0 / 6.0)
    {
        low = low.min(0.0);
        high = high.max(0.0);
    }
    if let Some(minimum) = axis.minimum {
        low = minimum;
    }
    if let Some(maximum) = axis.maximum {
        high = maximum;
    }
    if high <= low {
        high = if low == 0.0 { 1.0 } else { low + low.abs() };
    }
    let mut step = axis.major_unit.unwrap_or_else(|| nice((high - low) / 5.0));
    if (high - low) / step > MAX_TICKS {
        step = nice((high - low) / 10.0);
    }
    Scale {
        minimum: axis.minimum.unwrap_or_else(|| (low / step).floor() * step),
        maximum: axis.maximum.unwrap_or_else(|| (high / step).ceil() * step),
        step,
    }
}

/// The round number nearest `value`: 1, 2 or 5 times a power of ten.
fn nice(value: f64) -> f64 {
    if value <= 0.0 || !value.is_finite() {
        return 1.0;
    }
    let power = 10_f64.powf(value.log10().floor());
    let fraction = value / power;
    let rounded = if fraction < 1.5 {
        1.0
    } else if fraction < 3.0 {
        2.0
    } else if fraction < 7.0 {
        5.0
    } else {
        10.0
    };
    rounded * power
}

fn format_value(chart: &Chart, code: Option<&str>, value: f64) -> String {
    format_number(value, code.unwrap_or("General"), chart.locale).text
}

/// The format of a value axis's labels: its own, the percentage of a percent stack, else the
/// first series's.
fn axis_code<'c>(axis: &'c ValueAxis, plots: &[&'c Plot]) -> &'c str {
    if let Some(code) = &axis.format_code {
        return code;
    }
    if plots.iter().any(|plot| plot.grouping == Grouping::Percent) {
        return "0%";
    }
    plots
        .iter()
        .flat_map(|plot| plot.series.iter())
        .find_map(|series| series.format_code.as_deref())
        .unwrap_or("General")
}

/// A chart of categories: columns, bars, lines and areas on shared axes.
#[expect(
    clippy::too_many_lines,
    reason = "the axes, labels and plots are laid out together on one area"
)]
fn categories(svg: &mut String, chart: &Chart, area: Area) {
    let horizontal = chart.plots.iter().any(|plot| plot.kind == PlotKind::Bar);
    let plots: Vec<&Plot> = chart
        .plots
        .iter()
        .filter(|plot| {
            !matches!(
                plot.kind,
                PlotKind::Pie
                    | PlotKind::Doughnut { .. }
                    | PlotKind::Scatter
                    | PlotKind::Bubble
                    | PlotKind::Radar { .. }
            )
        })
        .collect();
    let points = plots
        .iter()
        .map(|plot| plot.categories.len())
        .max()
        .unwrap_or(0)
        .max(1);
    let labels: Vec<String> = plots
        .iter()
        .find(|plot| plot.categories.len() == points)
        .map(|plot| plot.categories.clone())
        .unwrap_or_default();

    let scale_for = |secondary: bool| {
        let chosen: Vec<&Plot> = plots
            .iter()
            .copied()
            .filter(|plot| plot.secondary == secondary)
            .collect();
        let extent = chosen.iter().filter_map(|plot| plot_extent(plot)).reduce(
            |(low, high), (other_low, other_high)| (low.min(other_low), high.max(other_high)),
        );
        let include_zero = chosen.iter().any(|plot| plot.kind != PlotKind::Line);
        let axis = if secondary {
            &chart.secondary
        } else {
            &chart.primary
        };
        extent.map(|(low, high)| (scale_of(low, high, axis, include_zero), axis, chosen))
    };
    let Some((primary, primary_axis, primary_plots)) = scale_for(false).or_else(|| scale_for(true))
    else {
        return;
    };
    let secondary = scale_for(true).filter(|_| plots.iter().any(|plot| !plot.secondary));
    let primary_code = axis_code(primary_axis, &primary_plots);
    let primary_labels: Vec<(f64, String)> = primary
        .ticks()
        .into_iter()
        .map(|tick| (tick, format_number(tick, primary_code, chart.locale).text))
        .collect();
    let secondary_labels: Vec<(f64, String)> = secondary
        .as_ref()
        .map(|(scale, axis, chosen)| {
            let code = axis_code(axis, chosen);
            scale
                .ticks()
                .into_iter()
                .map(|tick| (tick, format_number(tick, code, chart.locale).text))
                .collect()
        })
        .unwrap_or_default();
    let widest = |labels: &[(f64, String)]| {
        labels
            .iter()
            .map(|(_, label)| text_width(label, chart.label_size))
            .fold(0.0, f64::max)
    };

    let (left_title, bottom_title) = if horizontal {
        (
            chart.category_title.as_deref(),
            primary_axis.title.as_deref(),
        )
    } else {
        (
            primary_axis.title.as_deref(),
            chart.category_title.as_deref(),
        )
    };
    let area = axis_titles(svg, chart, area, left_title, bottom_title);

    // The plot rectangle, inside the labels of both axes.
    let mut plot_area = area;
    let value_hidden = primary_axis.hidden;
    if horizontal {
        let category_width = if chart.category_axis_hidden {
            0.0
        } else {
            labels
                .iter()
                .map(|label| text_width(label, chart.label_size))
                .fold(0.0, f64::max)
                .min(area.width * 0.35)
                + PAD
        };
        plot_area.x += category_width;
        plot_area.width -= category_width;
        if !value_hidden {
            plot_area.height -= chart.label_size * 1.8;
            // The last value label is centred on the right edge.
            plot_area.width -= widest(&primary_labels) / 2.0;
        }
    } else {
        if !value_hidden {
            let used = widest(&primary_labels) + PAD;
            plot_area.x += used;
            plot_area.width -= used;
        }
        if !secondary_labels.is_empty() {
            plot_area.width -= widest(&secondary_labels) + PAD;
        }
        if !chart.category_axis_hidden {
            plot_area.height -= chart.label_size * 1.8;
        }
    }
    if plot_area.width < 4.0 || plot_area.height < 4.0 {
        return;
    }

    // Where a value or a category falls.
    let value_position = |scale: Scale, value: f64| {
        let fraction = scale.fraction(value);
        if horizontal {
            plot_area.x + fraction * plot_area.width
        } else {
            plot_area.bottom() - fraction * plot_area.height
        }
    };
    let length = if horizontal {
        plot_area.height
    } else {
        plot_area.width
    };
    let band = length / count(points);
    let reversed = chart.categories_reversed != horizontal;
    let band_start = |index: usize| {
        let slot = if reversed { points - 1 - index } else { index };
        if horizontal {
            plot_area.y + band * count(slot)
        } else {
            plot_area.x + band * count(slot)
        }
    };
    let category_position = |index: usize, between: bool| {
        if between || points == 1 {
            band_start(index) + band / 2.0
        } else {
            let slot = if reversed { points - 1 - index } else { index };
            let start = if horizontal { plot_area.y } else { plot_area.x };
            start + length * count(slot) / count(points - 1)
        }
    };

    // Gridlines and value labels.
    for (tick, label) in &primary_labels {
        let position = value_position(primary, *tick);
        if horizontal {
            if primary_axis.gridlines {
                line(
                    svg,
                    position,
                    plot_area.y,
                    position,
                    plot_area.bottom(),
                    GRID_COLOUR,
                    0.75,
                );
            }
            if !value_hidden {
                text(
                    svg,
                    position,
                    plot_area.bottom() + chart.label_size * 1.3,
                    chart.label_size,
                    "middle",
                    label,
                );
            }
        } else {
            if primary_axis.gridlines {
                line(
                    svg,
                    plot_area.x,
                    position,
                    plot_area.right(),
                    position,
                    GRID_COLOUR,
                    0.75,
                );
            }
            if !value_hidden {
                text(
                    svg,
                    plot_area.x - PAD / 2.0,
                    position + chart.label_size * 0.35,
                    chart.label_size,
                    "end",
                    label,
                );
            }
        }
    }
    if let Some((scale, axis, _)) = &secondary
        && !axis.hidden
        && !horizontal
    {
        for (tick, label) in &secondary_labels {
            let position = value_position(*scale, *tick);
            text(
                svg,
                plot_area.right() + PAD / 2.0,
                position + chart.label_size * 0.35,
                chart.label_size,
                "start",
                label,
            );
        }
    }

    // Category labels, thinned out so they do not overlap.
    if !chart.category_axis_hidden {
        let between = primary_axis
            .between
            .unwrap_or_else(|| plots.iter().any(|plot| plot.kind != PlotKind::Area));
        let widest_label = labels
            .iter()
            .map(|label| text_width(label, chart.label_size))
            .fold(0.0, f64::max);
        let every = if horizontal {
            usize_of((chart.label_size * 1.2 / band).ceil()).max(1)
        } else {
            usize_of(((widest_label + PAD) / band).ceil()).max(1)
        };
        for (index, label) in labels.iter().enumerate().step_by(every) {
            let position = category_position(index, between);
            if horizontal {
                let room = plot_area.x - area.x - PAD;
                text(
                    svg,
                    plot_area.x - PAD / 2.0,
                    position + chart.label_size * 0.35,
                    chart.label_size,
                    "end",
                    &fit(label, room, chart.label_size),
                );
            } else {
                let room = band * count(every);
                text(
                    svg,
                    position,
                    plot_area.bottom() + chart.label_size * 1.3,
                    chart.label_size,
                    "middle",
                    &fit(label, room, chart.label_size),
                );
            }
        }
    }

    for plot in &plots {
        let scale = if plot.secondary {
            secondary.as_ref().map_or(primary, |(scale, _, _)| *scale)
        } else {
            primary
        };
        let position = |value: f64| value_position(scale, value);
        match plot.kind {
            PlotKind::Column | PlotKind::Bar => {
                let stacked = plot.grouping != Grouping::Standard;
                let slots = if stacked {
                    1.0
                } else {
                    count(plot.series.len())
                };
                let overlap = f64::from(plot.overlap_percent) / 100.0;
                let gap = f64::from(plot.gap_percent) / 100.0;
                let bar = band / (slots - (slots - 1.0) * overlap + gap).max(0.1);
                for index in 0..points {
                    let start = band_start(index) + gap * bar / 2.0;
                    let (positive_total, negative_total) = stack_totals(plot, index);
                    let whole = positive_total - negative_total;
                    let (mut positive, mut negative) = (0.0, 0.0);
                    for (slot, series) in plot.series.iter().enumerate() {
                        let Some(value) = series.values.get(index).copied().flatten() else {
                            continue;
                        };
                        let value = if plot.grouping == Grouping::Percent && whole > 0.0 {
                            value / whole
                        } else {
                            value
                        };
                        let (from, to) = if stacked {
                            let base = if value >= 0.0 {
                                &mut positive
                            } else {
                                &mut negative
                            };
                            let from = *base;
                            *base += value;
                            (from, *base)
                        } else {
                            (scale.baseline(), value)
                        };
                        let offset = if stacked {
                            0.0
                        } else {
                            count(slot) * bar * (1.0 - overlap)
                        };
                        let (low, high) = (
                            position(from.clamp(scale.minimum, scale.maximum)),
                            position(to.clamp(scale.minimum, scale.maximum)),
                        );
                        let colour = point_colour(series, index);
                        let along = start + offset;
                        let (x, y, width, height) = if horizontal {
                            (low.min(high), along, (high - low).abs(), bar)
                        } else {
                            (along, low.min(high), bar, (high - low).abs())
                        };
                        let _ = write!(
                            svg,
                            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#{colour}\"/>",
                            number(x),
                            number(y),
                            number(width),
                            number(height)
                        );
                        if series.labels.value {
                            let label = format_value(
                                chart,
                                series
                                    .label_format
                                    .as_deref()
                                    .or(series.format_code.as_deref()),
                                series.values[index].unwrap_or(value),
                            );
                            if horizontal {
                                text(
                                    svg,
                                    x + width + 2.0,
                                    y + height / 2.0 + chart.label_size * 0.35,
                                    chart.label_size,
                                    "start",
                                    &label,
                                );
                            } else {
                                text(
                                    svg,
                                    x + width / 2.0,
                                    y - 2.0,
                                    chart.label_size,
                                    "middle",
                                    &label,
                                );
                            }
                        }
                    }
                }
            }
            PlotKind::Line | PlotKind::Area => {
                let between = primary_axis.between.unwrap_or(plot.kind == PlotKind::Line);
                let mut floor = vec![0.0; points];
                for series in &plot.series {
                    let mut tops = Vec::with_capacity(points);
                    for (index, base) in floor.iter().enumerate() {
                        let value = series.values.get(index).copied().flatten();
                        let value = match (plot.grouping, value) {
                            (_, None) => None,
                            (Grouping::Standard, Some(value)) => Some(value),
                            (Grouping::Stacked, Some(value)) => Some(base + value),
                            (Grouping::Percent, Some(value)) => {
                                let (positive, negative) = stack_totals(plot, index);
                                let whole = positive - negative;
                                Some(base + if whole > 0.0 { value / whole } else { 0.0 })
                            }
                        };
                        tops.push(value.map(|value| value.clamp(scale.minimum, scale.maximum)));
                    }
                    let point = |index: usize, value: f64| {
                        let along = category_position(index, between);
                        if horizontal {
                            (position(value), along)
                        } else {
                            (along, position(value))
                        }
                    };
                    if plot.kind == PlotKind::Area {
                        let mut path = String::new();
                        for (index, top) in tops.iter().enumerate() {
                            let (x, y) = point(index, top.unwrap_or(floor[index]));
                            let _ = write!(
                                path,
                                "{}{} {}",
                                if index == 0 { "M" } else { "L" },
                                number(x),
                                number(y)
                            );
                        }
                        for index in (0..points).rev() {
                            let base = if plot.grouping == Grouping::Standard {
                                scale.baseline()
                            } else {
                                floor[index]
                            };
                            let (x, y) = point(index, base);
                            let _ = write!(path, "L{} {}", number(x), number(y));
                        }
                        let _ = write!(svg, "<path d=\"{path}Z\" fill=\"#{}\"/>", series.colour);
                    } else {
                        if series.line {
                            let mut path = String::new();
                            let mut pen_down = false;
                            for (index, top) in tops.iter().enumerate() {
                                match top {
                                    Some(value) => {
                                        let (x, y) = point(index, *value);
                                        let _ = write!(
                                            path,
                                            "{}{} {}",
                                            if pen_down { "L" } else { "M" },
                                            number(x),
                                            number(y)
                                        );
                                        pen_down = true;
                                    }
                                    None => pen_down = false,
                                }
                            }
                            let _ = write!(
                                svg,
                                "<path d=\"{path}\" fill=\"none\" stroke=\"#{}\" stroke-width=\"{LINE_WIDTH}\" stroke-linejoin=\"round\" stroke-linecap=\"round\"/>",
                                series.colour
                            );
                        }
                        if series.markers {
                            for (index, top) in tops.iter().enumerate() {
                                if let Some(value) = top {
                                    let (x, y) = point(index, *value);
                                    marker(svg, x, y, &series.colour);
                                }
                            }
                        }
                    }
                    if series.labels.value {
                        for (index, top) in tops.iter().enumerate() {
                            if let (Some(top), Some(original)) =
                                (top, series.values.get(index).copied().flatten())
                            {
                                let (x, y) = point(index, *top);
                                text(
                                    svg,
                                    x,
                                    y - MARKER_RADIUS - 2.0,
                                    chart.label_size,
                                    "middle",
                                    &format_value(
                                        chart,
                                        series
                                            .label_format
                                            .as_deref()
                                            .or(series.format_code.as_deref()),
                                        original,
                                    ),
                                );
                            }
                        }
                    }
                    if plot.grouping != Grouping::Standard {
                        for (index, top) in tops.iter().enumerate() {
                            if let Some(top) = top {
                                floor[index] = *top;
                            }
                        }
                    }
                }
            }
            PlotKind::Pie
            | PlotKind::Doughnut { .. }
            | PlotKind::Scatter
            | PlotKind::Bubble
            | PlotKind::Radar { .. } => {}
        }
    }

    // The category axis line sits on zero.
    let zero = value_position(primary, primary.baseline());
    if horizontal {
        line(
            svg,
            zero,
            plot_area.y,
            zero,
            plot_area.bottom(),
            AXIS_COLOUR,
            0.75,
        );
    } else {
        line(
            svg,
            plot_area.x,
            zero,
            plot_area.right(),
            zero,
            AXIS_COLOUR,
            0.75,
        );
    }
}

/// A scatter chart: two value axes, each point at its x and y.
#[expect(
    clippy::too_many_lines,
    reason = "both axes, their labels and the points are laid out on one area"
)]
fn scatter(svg: &mut String, chart: &Chart, area: Area) {
    let series: Vec<&Series> = chart
        .plots
        .iter()
        .flat_map(|plot| plot.series.iter())
        .collect();
    let x_of = |series: &Series, index: usize| {
        if series.x_values.is_empty() {
            u32::try_from(index + 1).ok().map(f64::from)
        } else {
            series.x_values.get(index).copied().flatten()
        }
    };
    let mut x_extent: Option<(f64, f64)> = None;
    let mut y_extent: Option<(f64, f64)> = None;
    for series in &series {
        for (index, value) in series.values.iter().enumerate() {
            let (Some(y), Some(x)) = (*value, x_of(series, index)) else {
                continue;
            };
            x_extent = Some(x_extent.map_or((x, x), |(low, high)| (low.min(x), high.max(x))));
            y_extent = Some(y_extent.map_or((y, y), |(low, high)| (low.min(y), high.max(y))));
        }
    }
    let (Some((x_low, x_high)), Some((y_low, y_high))) = (x_extent, y_extent) else {
        return;
    };
    // Bubbles reach past their centres, so the axes leave room around the outermost.
    let room = |low: f64, high: f64| {
        if series.iter().any(|series| !series.sizes.is_empty()) {
            let room = (high - low) * 0.1;
            (low - room, high + room)
        } else {
            (low, high)
        }
    };
    let ((x_low, x_high), (y_low, y_high)) = (room(x_low, x_high), room(y_low, y_high));
    let x_scale = scale_of(x_low, x_high, &chart.x_axis, false);
    let y_scale = scale_of(y_low, y_high, &chart.primary, false);
    let first_code = series
        .iter()
        .find_map(|series| series.format_code.as_deref());
    let y_code = chart
        .primary
        .format_code
        .as_deref()
        .or(first_code)
        .unwrap_or("General");
    let x_code = chart.x_axis.format_code.as_deref().unwrap_or("General");
    let y_labels: Vec<(f64, String)> = y_scale
        .ticks()
        .into_iter()
        .map(|tick| (tick, format_number(tick, y_code, chart.locale).text))
        .collect();
    let label_width = y_labels
        .iter()
        .map(|(_, label)| text_width(label, chart.label_size))
        .fold(0.0, f64::max);
    let area = axis_titles(
        svg,
        chart,
        area,
        chart.primary.title.as_deref(),
        chart.x_axis.title.as_deref(),
    );
    let plot_area = Area {
        x: area.x + label_width + PAD,
        y: area.y,
        width: area.width - label_width - PAD,
        height: area.height - chart.label_size * 1.8,
    };
    if plot_area.width < 4.0 || plot_area.height < 4.0 {
        return;
    }
    let x_position = |value: f64| plot_area.x + x_scale.fraction(value) * plot_area.width;
    let y_position = |value: f64| plot_area.bottom() - y_scale.fraction(value) * plot_area.height;
    for (tick, label) in &y_labels {
        let y = y_position(*tick);
        if chart.primary.gridlines {
            line(svg, plot_area.x, y, plot_area.right(), y, GRID_COLOUR, 0.75);
        }
        text(
            svg,
            plot_area.x - PAD / 2.0,
            y + chart.label_size * 0.35,
            chart.label_size,
            "end",
            label,
        );
    }
    for tick in x_scale.ticks() {
        let x = x_position(tick);
        if chart.x_axis.gridlines {
            line(
                svg,
                x,
                plot_area.y,
                x,
                plot_area.bottom(),
                GRID_COLOUR,
                0.75,
            );
        }
        text(
            svg,
            x,
            plot_area.bottom() + chart.label_size * 1.3,
            chart.label_size,
            "middle",
            &format_number(tick, x_code, chart.locale).text,
        );
    }
    let zero = y_position(y_scale.baseline());
    line(
        svg,
        plot_area.x,
        zero,
        plot_area.right(),
        zero,
        AXIS_COLOUR,
        0.75,
    );
    // The largest bubble spans a quarter of the plot's shorter side, the others by their area.
    let largest = series
        .iter()
        .flat_map(|series| series.sizes.iter().flatten())
        .map(|size| size.abs())
        .fold(0.0, f64::max);
    let widest_bubble = plot_area.width.min(plot_area.height) / 8.0;
    for series in series {
        if !series.sizes.is_empty() {
            for (index, value) in series.values.iter().enumerate() {
                let (Some(y), Some(x), Some(size)) = (
                    *value,
                    x_of(series, index),
                    series.sizes.get(index).copied().flatten(),
                ) else {
                    continue;
                };
                if size <= 0.0 || largest <= 0.0 {
                    continue;
                }
                let x = x.clamp(x_scale.minimum, x_scale.maximum);
                let y = y.clamp(y_scale.minimum, y_scale.maximum);
                let _ = write!(
                    svg,
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"#{}\" stroke=\"#FFFFFF\" stroke-width=\"0.75\"/>",
                    number(x_position(x)),
                    number(y_position(y)),
                    number((size / largest).sqrt() * widest_bubble),
                    point_colour(series, index)
                );
            }
            continue;
        }
        let points: Vec<Option<(f64, f64)>> = series
            .values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let y = value.map(|y| y.clamp(y_scale.minimum, y_scale.maximum))?;
                let x = x_of(series, index)?.clamp(x_scale.minimum, x_scale.maximum);
                Some((x_position(x), y_position(y)))
            })
            .collect();
        if series.line {
            let mut path = String::new();
            let mut pen_down = false;
            for point in &points {
                match point {
                    Some((x, y)) => {
                        let _ = write!(
                            path,
                            "{}{} {}",
                            if pen_down { "L" } else { "M" },
                            number(*x),
                            number(*y)
                        );
                        pen_down = true;
                    }
                    None => pen_down = false,
                }
            }
            let _ = write!(
                svg,
                "<path d=\"{path}\" fill=\"none\" stroke=\"#{}\" stroke-width=\"{LINE_WIDTH}\" stroke-linejoin=\"round\"/>",
                series.colour
            );
        }
        if series.markers || !series.line {
            for (x, y) in points.iter().flatten() {
                marker(svg, *x, *y, &series.colour);
            }
        }
    }
}

/// Where a radar stands: its centre, its radius and how many spokes it has.
#[derive(Debug, Clone, Copy)]
struct RadarFrame {
    centre: (f64, f64),
    radius: f64,
    spokes: usize,
}

impl RadarFrame {
    /// The point `distance` out along spoke `index`, the first pointing up and the rest
    /// following clockwise.
    fn point(self, index: usize, distance: f64) -> (f64, f64) {
        let angle = (count(index) / count(self.spokes) * 360.0 - 90.0).to_radians();
        (
            self.centre.0 + distance * angle.cos(),
            self.centre.1 + distance * angle.sin(),
        )
    }

    /// The path through each spoke at its distance, closed when no spoke is missing.
    fn outline(self, distance: &dyn Fn(usize) -> Option<f64>) -> String {
        let mut path = String::new();
        let mut whole = true;
        for index in 0..self.spokes {
            let Some(distance) = distance(index) else {
                whole = false;
                continue;
            };
            let (x, y) = self.point(index, distance);
            let _ = write!(
                path,
                "{}{} {}",
                if path.is_empty() { "M" } else { "L" },
                number(x),
                number(y)
            );
        }
        if whole && !path.is_empty() {
            path.push('Z');
        }
        path
    }
}

/// A radar: a spoke per category from the centre, a ring at each value tick, and each series
/// a closed outline, filled when its plot is.
fn radar(svg: &mut String, chart: &Chart, area: Area) {
    let categories = chart
        .plots
        .iter()
        .map(|plot| &plot.categories)
        .max_by_key(|categories| categories.len())
        .map_or(&[][..], Vec::as_slice);
    let all_series = || chart.plots.iter().flat_map(|plot| plot.series.iter());
    let spokes = all_series()
        .map(|series| series.values.len())
        .max()
        .unwrap_or(0)
        .max(categories.len());
    let extent = all_series()
        .flat_map(|series| series.values.iter().flatten().copied())
        .fold(None, |extent: Option<(f64, f64)>, value| {
            Some(extent.map_or((value, value), |(low, high)| {
                (low.min(value), high.max(value))
            }))
        });
    let Some((low, high)) = extent.filter(|_| spokes > 0) else {
        return;
    };
    let scale = scale_of(low, high, &chart.primary, false);
    let size = chart.label_size;
    let widest = categories
        .iter()
        .map(|label| text_width(label, size))
        .fold(0.0, f64::max)
        .min(area.width * 0.25);
    let frame = RadarFrame {
        centre: (area.x + area.width / 2.0, area.y + area.height / 2.0),
        radius: ((area.width - 2.0 * (widest + PAD)).min(area.height - size * 3.0) / 2.0).max(4.0),
        spokes,
    };
    radar_axes(svg, chart, frame, scale, categories, widest);
    for plot in &chart.plots {
        let filled = matches!(plot.kind, PlotKind::Radar { filled: true });
        for series in &plot.series {
            let distance = |index: usize| {
                let value = series.values.get(index).copied().flatten()?;
                Some(scale.fraction(value.clamp(scale.minimum, scale.maximum)) * frame.radius)
            };
            let path = frame.outline(&distance);
            if path.is_empty() {
                continue;
            }
            if filled {
                let _ = write!(
                    svg,
                    "<path d=\"{path}\" fill=\"#{}\" stroke=\"none\"/>",
                    series.colour
                );
            } else if series.line {
                let _ = write!(
                    svg,
                    "<path d=\"{path}\" fill=\"none\" stroke=\"#{}\" stroke-width=\"{LINE_WIDTH}\" stroke-linejoin=\"round\"/>",
                    series.colour
                );
            }
            if series.markers {
                for index in 0..spokes {
                    if let Some(distance) = distance(index) {
                        let (x, y) = frame.point(index, distance);
                        marker(svg, x, y, &series.colour);
                    }
                }
            }
        }
    }
}

/// The rings and value labels of a radar, its spokes and the category at the end of each.
fn radar_axes(
    svg: &mut String,
    chart: &Chart,
    frame: RadarFrame,
    scale: Scale,
    categories: &[String],
    widest: f64,
) {
    let size = chart.label_size;
    let (centre, radius) = (frame.centre, frame.radius);
    let first_code = chart
        .plots
        .iter()
        .flat_map(|plot| plot.series.iter())
        .find_map(|series| series.format_code.as_deref());
    let code = chart
        .primary
        .format_code
        .as_deref()
        .or(first_code)
        .unwrap_or("General");
    for tick in scale.ticks() {
        let distance = scale.fraction(tick) * radius;
        if chart.primary.gridlines && distance > 0.0 {
            let _ = write!(
                svg,
                "<path d=\"{}\" fill=\"none\" stroke=\"{GRID_COLOUR}\" stroke-width=\"0.75\"/>",
                frame.outline(&|_| Some(distance))
            );
        }
        if !chart.primary.hidden {
            text(
                svg,
                centre.0 - PAD / 2.0,
                centre.1 - distance + size * 0.35,
                size * 0.9,
                "end",
                &format_number(tick, code, chart.locale).text,
            );
        }
    }
    for index in 0..frame.spokes {
        let (x, y) = frame.point(index, radius);
        line(svg, centre.0, centre.1, x, y, AXIS_COLOUR, 0.75);
        let Some(label) = categories.get(index) else {
            continue;
        };
        let (x, y) = frame.point(index, radius + size * 0.6);
        let across = x - centre.0;
        let anchor = if across > radius * 0.1 {
            "start"
        } else if across < -radius * 0.1 {
            "end"
        } else {
            "middle"
        };
        let below = if y > centre.1 + radius * 0.5 {
            size * 0.8
        } else if y < centre.1 - radius * 0.5 {
            0.0
        } else {
            size * 0.35
        };
        text(svg, x, y + below, size, anchor, &fit(label, widest, size));
    }
}

/// The titles of the axes, one turned up along the left side and one under the plot, and what
/// remains of `area` inside them.
fn axis_titles(
    svg: &mut String,
    chart: &Chart,
    area: Area,
    left: Option<&str>,
    bottom: Option<&str>,
) -> Area {
    let size = chart.label_size * 1.1;
    let mut inner = area;
    if let Some(title) = left.filter(|title| !title.is_empty()) {
        let (x, y) = (area.x + size * 0.85, area.y + area.height / 2.0);
        let _ = write!(
            svg,
            "<text x=\"{0}\" y=\"{1}\" font-size=\"{2}\" fill=\"{TEXT_COLOUR}\" text-anchor=\"middle\" transform=\"rotate(-90 {0} {1})\">{3}</text>",
            number(x),
            number(y),
            number(size),
            escape(&fit(title, area.height, size))
        );
        inner.x += size * 1.5;
        inner.width -= size * 1.5;
    }
    if let Some(title) = bottom.filter(|title| !title.is_empty()) {
        text(
            svg,
            inner.x + inner.width / 2.0,
            area.bottom() - size * 0.25,
            size,
            "middle",
            &fit(title, inner.width, size),
        );
        inner.height -= size * 1.6;
    }
    inner
}

fn marker(svg: &mut String, x: f64, y: f64, colour: &str) {
    let _ = write!(
        svg,
        "<circle cx=\"{}\" cy=\"{}\" r=\"{MARKER_RADIUS}\" fill=\"#{colour}\"/>",
        number(x),
        number(y)
    );
}

fn line(svg: &mut String, x1: f64, y1: f64, x2: f64, y2: f64, colour: &str, width: f64) {
    let _ = write!(
        svg,
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{colour}\" stroke-width=\"{}\"/>",
        number(x1),
        number(y1),
        number(x2),
        number(y2),
        number(width)
    );
}

fn text(svg: &mut String, x: f64, y: f64, size: f64, anchor: &str, content: &str) {
    text_coloured(svg, x, y, size, anchor, content, TEXT_COLOUR);
}

fn text_coloured(
    svg: &mut String,
    x: f64,
    y: f64,
    size: f64,
    anchor: &str,
    content: &str,
    colour: &str,
) {
    if content.is_empty() {
        return;
    }
    let _ = write!(
        svg,
        "<text x=\"{}\" y=\"{}\" font-size=\"{}\" fill=\"{colour}\" text-anchor=\"{anchor}\">{}</text>",
        number(x),
        number(y),
        number(size),
        escape(content)
    );
}

/// A label cut with an ellipsis to fit `room` points.
fn fit(label: &str, room: f64, size: f64) -> String {
    if text_width(label, size) <= room + 0.5 {
        return label.to_owned();
    }
    let keep = usize_of(room / (size * GLYPH_WIDTH)).saturating_sub(1);
    let mut fitted: String = label.chars().take(keep).collect();
    fitted.push('\u{2026}');
    fitted
}

fn text_width(text: &str, size: f64) -> f64 {
    count(text.chars().count()) * size * GLYPH_WIDTH
}

#[expect(
    clippy::cast_precision_loss,
    reason = "counts of labels and points are small"
)]
fn count(value: usize) -> f64 {
    value as f64
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is clamped to a small positive range first"
)]
fn usize_of(value: f64) -> usize {
    value.clamp(0.0, 1_000_000.0) as usize
}

/// A coordinate with at most two decimals.
fn number(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    let mut text = format!("{rounded:.2}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    if text == "-0" {
        "0".clone_into(&mut text);
    }
    text
}

fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            character if character.is_control() => {}
            character => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests;
