// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The charts of `OpenDocument`: an embedded chart object, `Object 1/content.xml`, whose series
//! point at the cells of the table the object carries.

use std::collections::HashMap;

use super::{Chart, DataLabels, Grouping, LegendPosition, Plot, PlotKind, Series, ValueAxis};
use crate::number_format::{NumberLocale, general_number};
use crate::sheet_layout::CellRange;
use crate::xml_tree::{Element, Namespace, parse_tree};

const CHART: Namespace = Namespace::OdfChart;
const STYLE: Namespace = Namespace::OdfStyle;
const DRAW: Namespace = Namespace::OdfDraw;
const FO: Namespace = Namespace::OdfFo;
const TABLE: Namespace = Namespace::OdfTable;
const TEXT: Namespace = Namespace::OdfText;
const OFFICE: Namespace = Namespace::OdfOffice;
const MAX_SERIES: usize = 64;
const MAX_POINTS: usize = 4_096;
const MAX_ROWS: usize = 4_096;
/// `LibreOffice`'s colours for series that name none.
const PALETTE: [&str; 12] = [
    "004586", "FF420E", "FFD320", "579D1C", "7E0021", "83CAFF", "314004", "AECF00", "4B1F6F",
    "FF950E", "C5000B", "0084D1",
];

/// A cell of the chart's own table.
#[derive(Debug, Clone, PartialEq)]
enum Cell {
    Number(f64),
    Text(String),
}

/// A chart object's content part, or `None` when it holds nothing this reader draws.
#[expect(
    clippy::too_many_lines,
    reason = "one pass reads the table, the series and the axes they share"
)]
pub(crate) fn read_odf_chart(xml: &str, locale: NumberLocale) -> Option<Chart> {
    let root = parse_tree(xml, |namespace, name| namespace == TEXT && name == "p").ok()?;
    let chart = root
        .child(OFFICE, "body")?
        .child(OFFICE, "chart")?
        .child(CHART, "chart")?;
    let styles: HashMap<&str, &Element> = root
        .child(OFFICE, "automatic-styles")
        .map(|automatic| {
            automatic
                .elements()
                .filter(|style| style.is(STYLE, "style"))
                .filter_map(|style| Some((style.attribute(STYLE, "name")?, style)))
                .collect()
        })
        .unwrap_or_default();
    let property =
        |name: Option<&str>, kind: &str, namespace: Namespace, key: &str| -> Option<String> {
            styles
                .get(name?)?
                .child(STYLE, kind)?
                .attribute(namespace, key)
                .map(str::to_owned)
        };
    let (table, copies) = chart
        .elements()
        .find(|element| element.is(TABLE, "table"))
        .map(local_table)
        .unwrap_or_default();
    let cells = |address: &str| -> Vec<Option<&Cell>> {
        let Some(range) = range_of(address) else {
            return Vec::new();
        };
        if let Some(&(row, column)) = copies.get(&address.replace('$', "")) {
            let down = range.first_column == range.last_column;
            let length = if down {
                range.last_row - range.first_row
            } else {
                range.last_column - range.first_column
            } as usize
                + 1;
            return (0..length.min(MAX_POINTS))
                .map(|offset| {
                    let (row, column) = if down {
                        (row + offset, column)
                    } else {
                        (row, column + offset)
                    };
                    table
                        .get(row)
                        .and_then(|cells| cells.get(column))
                        .and_then(Option::as_ref)
                })
                .collect();
        }
        let mut found = Vec::new();
        for row in range.first_row..=range.last_row.min(range.first_row + 4_096) {
            for column in range.first_column..=range.last_column.min(range.first_column + 64) {
                found.push(
                    table
                        .get(row as usize)
                        .and_then(|cells| cells.get(column as usize))
                        .and_then(Option::as_ref),
                );
                if found.len() >= MAX_POINTS {
                    return found;
                }
            }
        }
        found
    };
    let text_of = |cell: Option<&Cell>| match cell {
        Some(Cell::Text(text)) => text.clone(),
        Some(Cell::Number(number)) => general_number(*number, locale),
        None => String::new(),
    };

    let class = chart.attribute(CHART, "class").unwrap_or("chart:bar");
    let plot_area = chart.child(CHART, "plot-area")?;
    let plot_style = plot_area.attribute(CHART, "style-name");
    let flag = |name: &str| {
        property(plot_style, "chart-properties", CHART, name).as_deref() == Some("true")
    };
    let horizontal = flag("vertical");
    let grouping = if flag("percentage") {
        Grouping::Percent
    } else if flag("stacked") {
        Grouping::Stacked
    } else {
        Grouping::Standard
    };
    let kind_of = |class: &str| match class.trim_start_matches("chart:") {
        "bar" if horizontal => Some(PlotKind::Bar),
        "bar" => Some(PlotKind::Column),
        "line" | "stock" => Some(PlotKind::Line),
        "area" => Some(PlotKind::Area),
        "circle" => Some(PlotKind::Pie),
        "ring" => Some(PlotKind::Doughnut { hole_percent: 50 }),
        "scatter" => Some(PlotKind::Scatter),
        "bubble" => Some(PlotKind::Bubble),
        "radar" => Some(PlotKind::Radar { filled: false }),
        "filled-radar" => Some(PlotKind::Radar { filled: true }),
        _ => None,
    };
    let default_kind = kind_of(class)?;
    // A stock chart draws its series as marks between their values, never as lines.
    let stock = class.trim_start_matches("chart:") == "stock";
    let stock_fill = |name: &str, default: &str| {
        let style = plot_area
            .child(CHART, name)
            .and_then(|marker| marker.attribute(CHART, "style-name"));
        property(style, "graphic-properties", DRAW, "fill-color")
            .and_then(|value| hex(&value))
            .unwrap_or_else(|| default.to_owned())
    };
    let up_down_bars = (stock && flag("japanese-candle-stick")).then(|| {
        (
            stock_fill("stock-gain-marker", "FFFFFF"),
            stock_fill("stock-loss-marker", "000000"),
        )
    });

    let axes: Vec<&Element> = plot_area
        .elements()
        .filter(|axis| axis.is(CHART, "axis"))
        .collect();
    let axis_of = |dimension: &str| {
        axes.iter()
            .copied()
            .find(|axis| axis.attribute(CHART, "dimension") == Some(dimension))
    };
    let categories: Vec<String> = axis_of("x")
        .and_then(|axis| axis.child(CHART, "categories"))
        .and_then(|categories| categories.attribute(TABLE, "cell-range-address"))
        .map(|address| cells(address).into_iter().map(text_of).collect())
        .unwrap_or_default();

    let mut plots: Vec<Plot> = Vec::new();
    for (index, element) in plot_area
        .elements()
        .filter(|series| series.is(CHART, "series"))
        .take(MAX_SERIES)
        .enumerate()
    {
        let kind = element
            .attribute(CHART, "class")
            .and_then(kind_of)
            .unwrap_or(default_kind);
        let style = element.attribute(CHART, "style-name");
        let numbers_at = |address: Option<&str>| -> Vec<Option<f64>> {
            address
                .map(|address| {
                    cells(address)
                        .into_iter()
                        .map(|cell| match cell {
                            Some(Cell::Number(number)) => Some(*number),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        let domains: Vec<Option<&str>> = element
            .elements()
            .filter(|domain| domain.is(CHART, "domain"))
            .map(|domain| domain.attribute(TABLE, "cell-range-address"))
            .collect();
        let own_values = numbers_at(element.attribute(CHART, "values-cell-range-address"));
        // A bubble series names its sizes as its values, its y as the first domain and its x
        // as the second.
        let (values, x_values, sizes) = if kind == PlotKind::Bubble {
            (
                numbers_at(domains.first().copied().flatten()),
                numbers_at(domains.get(1).copied().flatten()),
                own_values,
            )
        } else {
            (
                own_values,
                numbers_at(domains.first().copied().flatten()),
                Vec::new(),
            )
        };
        let name = element
            .attribute(CHART, "label-cell-address")
            .map(|address| cells(address).into_iter().next().flatten())
            .map(text_of)
            .filter(|name| !name.is_empty())
            .or_else(|| {
                // A name typed in rather than taken from a cell, written as a quoted string.
                let typed = element.attribute(Namespace::LoExt, "label-string")?;
                let typed = typed.trim().trim_matches('"').replace("\"\"", "\"");
                (!typed.is_empty()).then_some(typed)
            })
            .unwrap_or_else(|| format!("Serie {}", index + 1));
        let colour = property(style, "graphic-properties", DRAW, "fill-color")
            .or_else(|| {
                property(
                    style,
                    "graphic-properties",
                    Namespace::OdfSvg,
                    "stroke-color",
                )
            })
            .and_then(|value| hex(&value))
            .unwrap_or_else(|| PALETTE[index % PALETTE.len()].to_owned());
        let mut point_colours = Vec::new();
        let mut point = 0;
        for data_point in element
            .elements()
            .filter(|point| point.is(CHART, "data-point"))
        {
            let repeated = data_point
                .attribute(CHART, "repeated")
                .and_then(|count| count.parse::<usize>().ok())
                .unwrap_or(1)
                .clamp(1, MAX_POINTS);
            if let Some(fill) = property(
                data_point.attribute(CHART, "style-name"),
                "graphic-properties",
                DRAW,
                "fill-color",
            )
            .and_then(|value| hex(&value))
            {
                for offset in 0..repeated {
                    point_colours.push((point + offset, fill.clone()));
                }
            }
            point += repeated;
        }
        let pies = matches!(kind, PlotKind::Pie | PlotKind::Doughnut { .. });
        if pies {
            for index in 0..values.len() {
                if !point_colours.iter().any(|(point, _)| *point == index) {
                    point_colours.push((index, PALETTE[index % PALETTE.len()].to_owned()));
                }
            }
        }
        let label = property(style, "chart-properties", CHART, "data-label-number");
        let symbol = property(style, "chart-properties", CHART, "symbol-type");
        let series = Series {
            name,
            values,
            x_values,
            sizes,
            colour,
            point_colours,
            line: !stock
                && kind != PlotKind::Bubble
                && (property(style, "graphic-properties", DRAW, "stroke").as_deref()
                    != Some("none")
                    || kind != PlotKind::Scatter),
            markers: !stock
                && (kind == PlotKind::Scatter
                    || symbol.as_deref().is_some_and(|symbol| symbol != "none")),
            labels: DataLabels {
                value: matches!(label.as_deref(), Some("value" | "value-and-percentage")),
                percent: matches!(
                    label.as_deref(),
                    Some("percentage" | "value-and-percentage")
                ),
            },
            format_code: None,
            label_format: None,
        };
        match plots.iter_mut().find(|plot| plot.kind == kind) {
            Some(plot) => plot.series.push(series),
            None => plots.push(Plot {
                kind,
                grouping,
                series: vec![series],
                categories: categories.clone(),
                gap_percent: property(plot_style, "chart-properties", CHART, "gap-width")
                    .and_then(|gap| gap.parse().ok())
                    .unwrap_or(100)
                    .min(500),
                overlap_percent: property(plot_style, "chart-properties", CHART, "overlap")
                    .and_then(|overlap| overlap.parse().ok())
                    .unwrap_or(if grouping == Grouping::Standard {
                        0
                    } else {
                        100
                    })
                    .clamp(-100, 100),
                secondary: false,
                vary_colours: pies,
                first_slice_degrees: 0,
                high_low_lines: stock,
                up_down_bars: up_down_bars.clone(),
            }),
        }
    }
    if plots.is_empty() {
        return None;
    }
    for plot in &mut plots {
        let points = plot
            .series
            .iter()
            .map(|series| series.values.len())
            .max()
            .unwrap_or(0);
        for number in plot.categories.len()..points {
            plot.categories.push((number + 1).to_string());
        }
    }
    let value_axis = |dimension: &str| {
        axis_of(dimension).map_or_else(ValueAxis::default, |axis| {
            let style = axis.attribute(CHART, "style-name");
            let number = |name: &str| {
                property(style, "chart-properties", CHART, name)
                    .and_then(|value| value.parse::<f64>().ok())
                    .filter(|value| value.is_finite())
            };
            ValueAxis {
                minimum: number("minimum"),
                maximum: number("maximum"),
                major_unit: number("interval-major").filter(|unit| *unit > 0.0),
                format_code: None,
                hidden: property(style, "chart-properties", CHART, "display-label").as_deref()
                    == Some("false"),
                gridlines: axis.elements().any(|grid| {
                    grid.is(CHART, "grid") && grid.attribute(CHART, "class") != Some("minor")
                }),
                between: None,
                title: axis
                    .child(CHART, "title")
                    .map(paragraph_text)
                    .filter(|title| !title.is_empty()),
            }
        })
    };
    let scatter = plots
        .iter()
        .all(|plot| matches!(plot.kind, PlotKind::Scatter | PlotKind::Bubble));
    let x_axis = axis_of("x");
    let size = |name: Option<&str>| {
        property(name, "text-properties", FO, "font-size")
            .and_then(|size| size.trim_end_matches("pt").parse::<f64>().ok())
            .filter(|size| (1.0..=400.0).contains(size))
    };
    let title = chart.child(CHART, "title");
    let label_size = size(x_axis.and_then(|axis| axis.attribute(CHART, "style-name")))
        .or_else(|| {
            size(
                chart
                    .child(CHART, "legend")
                    .and_then(|legend| legend.attribute(CHART, "style-name")),
            )
        })
        .unwrap_or(super::DEFAULT_LABEL_SIZE);
    Some(Chart {
        title: title.map(paragraph_text).filter(|title| !title.is_empty()),
        legend: chart.child(CHART, "legend").map(|legend| {
            match legend.attribute(CHART, "legend-position") {
                Some("start") => LegendPosition::Left,
                Some("top") => LegendPosition::Top,
                Some("bottom") => LegendPosition::Bottom,
                _ => LegendPosition::Right,
            }
        }),
        primary: value_axis("y"),
        secondary: ValueAxis::default(),
        x_axis: if scatter {
            value_axis("x")
        } else {
            ValueAxis::default()
        },
        category_axis_hidden: !scatter
            && x_axis.is_some_and(|axis| {
                property(
                    axis.attribute(CHART, "style-name"),
                    "chart-properties",
                    CHART,
                    "display-label",
                )
                .as_deref()
                    == Some("false")
            }),
        category_title: if scatter {
            None
        } else {
            x_axis
                .and_then(|axis| axis.child(CHART, "title"))
                .map(paragraph_text)
                .filter(|title| !title.is_empty())
        },
        categories_reversed: x_axis.is_some_and(|axis| {
            property(
                axis.attribute(CHART, "style-name"),
                "chart-properties",
                CHART,
                "reverse-direction",
            )
            .as_deref()
                == Some("true")
        }),
        plots,
        locale,
        label_size,
        title_size: size(title.and_then(|title| title.attribute(CHART, "style-name")))
            .unwrap_or(label_size * super::TITLE_SCALE),
    })
}

/// The cells of the chart's table, header rows included, row by row from A1.
/// The rows of a chart's own table, and where it keeps its copy of each range of the sheet a
/// series names: `LibreOffice` notes the range on the first cell of the copy.
fn local_table(table: &Element) -> LocalTable {
    let mut rows = Vec::new();
    let mut copies = HashMap::new();
    collect_rows(table, &mut rows, &mut copies);
    (rows, copies)
}

type LocalTable = (Vec<Vec<Option<Cell>>>, HashMap<String, (usize, usize)>);

fn collect_rows(
    part: &Element,
    rows: &mut Vec<Vec<Option<Cell>>>,
    copies: &mut HashMap<String, (usize, usize)>,
) {
    for element in part.elements().filter(|element| element.namespace == TABLE) {
        match element.name.as_str() {
            "table-header-rows" | "table-rows" | "table-row-group" => {
                collect_rows(element, rows, copies);
            }
            "table-row" if rows.len() < MAX_ROWS => {
                let mut cells = Vec::new();
                for cell in element
                    .elements()
                    .filter(|cell| cell.is(TABLE, "table-cell"))
                {
                    let value = match cell.attribute(OFFICE, "value-type") {
                        Some("float" | "percentage" | "currency") => cell
                            .attribute(OFFICE, "value")
                            .and_then(|value| value.parse::<f64>().ok())
                            .filter(|value| value.is_finite())
                            .map(Cell::Number),
                        Some(_) => Some(Cell::Text(paragraph_text(cell))),
                        None => None,
                    };
                    for range in cell
                        .elements()
                        .filter(|group| group.is(DRAW, "g"))
                        .filter_map(|group| group.child(Namespace::OdfSvg, "desc"))
                    {
                        for address in range.text().split_whitespace() {
                            copies
                                .entry(address.replace('$', ""))
                                .or_insert((rows.len(), cells.len()));
                        }
                    }
                    let repeated = cell
                        .attribute(TABLE, "number-columns-repeated")
                        .and_then(|count| count.parse::<usize>().ok())
                        .unwrap_or(1)
                        .clamp(1, 64);
                    for _ in 0..repeated {
                        cells.push(value.clone());
                    }
                }
                rows.push(cells);
            }
            _ => {}
        }
    }
}

/// The text of the paragraphs of an element, one after the other.
fn paragraph_text(element: &Element) -> String {
    let mut text = String::new();
    for paragraph in element
        .elements()
        .filter(|paragraph| paragraph.is(TEXT, "p"))
    {
        if !text.is_empty() {
            text.push(' ');
        }
        collect(paragraph, &mut text);
    }
    text.trim().chars().take(200).collect()
}

fn collect(element: &Element, text: &mut String) {
    for node in &element.children {
        match node {
            crate::xml_tree::Node::Text(content) => text.push_str(content),
            crate::xml_tree::Node::Element(child) => collect(child, text),
        }
    }
}

/// The range of a chart address, `local-table.$B$2:.$B$5`, the table names dropped.
fn range_of(address: &str) -> Option<CellRange> {
    let first = address.split_whitespace().next()?;
    let cell = |part: &str| part.rsplit('.').next().map(str::to_owned);
    let (from, to) = first.split_once(':').unwrap_or((first, first));
    CellRange::parse(&format!("{}:{}", cell(from)?, cell(to)?))
}

fn hex(value: &str) -> Option<String> {
    let hex = value.trim().strip_prefix('#')?;
    (hex.len() == 6 && hex.chars().all(|character| character.is_ascii_hexdigit()))
        .then(|| hex.to_ascii_uppercase())
}

#[cfg(test)]
mod tests;
