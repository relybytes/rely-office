// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! Drawn shapes as an SVG picture, for a document model that holds pictures and no shapes: a
//! word processor's arrows, boxes and flowchart symbols, alone or in a group, each with its
//! outline, fill, line and the text it carries.

use std::fmt::Write as _;

use super::{FillShade, PathCommand, ShapePath};

const EMU_PER_POINT: f64 = 12_700.0;
/// No picture holds more shapes than this.
const MAX_SHAPES: usize = 1_000;

/// One shape of a picture, in English Metric Units from the picture's top-left corner.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DrawnShape {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
    /// Clockwise, in degrees, about the shape's centre.
    pub rotation: f64,
    /// In the shape's own frame.
    pub paths: Vec<ShapePath>,
    /// `RRGGBB`.
    pub fill: Option<String>,
    /// `RRGGBB` and the width in English Metric Units.
    pub line: Option<(String, i64)>,
    pub text: Vec<TextLine>,
}

/// A line of the text a shape carries, centred in it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TextLine {
    pub text: String,
    pub size_points: f64,
    /// `RRGGBB`.
    pub colour: String,
    pub bold: bool,
    pub italic: bool,
}

/// The SVG of shapes on a picture of `width` by `height` English Metric Units; `None` when
/// nothing would show.
pub(crate) fn shapes_svg(shapes: &[DrawnShape], width: i64, height: i64) -> Option<Vec<u8>> {
    if width <= 0 || height <= 0 {
        return None;
    }
    let points = |emu: i64| {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a frame is far below 2^52 units"
        )]
        let emu = emu as f64;
        emu / EMU_PER_POINT
    };
    // A line on the edge of the picture keeps the half of its width outside the frame.
    let margin = shapes
        .iter()
        .filter_map(|shape| shape.line.as_ref())
        .map(|(_, width)| points(*width) / 2.0)
        .fold(0.0_f64, f64::max);
    let mut body = String::new();
    for shape in shapes.iter().take(MAX_SHAPES) {
        if let Some(group) = shape_group(shape) {
            body.push_str(&group);
        }
    }
    if body.is_empty() {
        return None;
    }
    let (width, height) = (points(width), points(height));
    Some(
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}pt" height="{}pt" viewBox="{} {} {} {}" overflow="visible">{body}</svg>"#,
            number(width + margin * 2.0),
            number(height + margin * 2.0),
            number(-margin),
            number(-margin),
            number(width + margin * 2.0),
            number(height + margin * 2.0)
        )
        .into_bytes(),
    )
}

/// One shape as a group placed and turned at its frame; `None` when it draws nothing.
fn shape_group(shape: &DrawnShape) -> Option<String> {
    let points = |emu: i64| {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a frame is far below 2^52 units"
        )]
        let emu = emu as f64;
        emu / EMU_PER_POINT
    };
    let (x, y) = (points(shape.x), points(shape.y));
    let (centre_x, centre_y) = (
        x + points(shape.width) / 2.0,
        y + points(shape.height) / 2.0,
    );
    let mut group = format!(r#"<g transform="translate({} {})"#, number(x), number(y));
    if shape.rotation.abs() > 0.01 {
        let _ = write!(
            group,
            " rotate({} {} {})",
            number(shape.rotation),
            number(centre_x - x),
            number(centre_y - y)
        );
    }
    group.push_str(r#"">"#);
    let mut drawn = false;
    for path in shape.paths.iter().filter(|path| path.filled) {
        if let Some(fill) = &shape.fill {
            let _ = write!(
                group,
                r##"<path d="{}" fill="#{}" fill-rule="evenodd"/>"##,
                path_data(path),
                path.shade.apply(fill)
            );
            drawn = true;
        }
    }
    if let Some((colour, line_width)) = &shape.line {
        for path in shape.paths.iter().filter(|path| path.stroked) {
            let _ = write!(
                group,
                r##"<path d="{}" fill="none" stroke="#{colour}" stroke-width="{}" stroke-linejoin="round"/>"##,
                path_data(path),
                number(points(*line_width).max(0.25))
            );
            drawn = true;
        }
    }
    let lines = &shape.text;
    if !lines.is_empty() {
        let total: f64 = lines.iter().map(|line| line.size_points * 1.2).sum();
        let mut baseline = (points(shape.height) - total) / 2.0;
        for line in lines {
            baseline += line.size_points * 1.2;
            if line.text.trim().is_empty() {
                continue;
            }
            let _ = write!(
                group,
                r##"<text x="{}" y="{}" font-family="Calibri, Carlito, sans-serif" font-size="{}" fill="#{}" text-anchor="middle"{}{} xml:space="preserve">{}</text>"##,
                number(points(shape.width) / 2.0),
                number(baseline - line.size_points * 0.25),
                number(line.size_points),
                line.colour,
                if line.bold {
                    r#" font-weight="bold""#
                } else {
                    ""
                },
                if line.italic {
                    r#" font-style="italic""#
                } else {
                    ""
                },
                escape(&line.text)
            );
            drawn = true;
        }
    }
    if drawn {
        group.push_str("</g>");
        return Some(group);
    }
    None
}

fn path_data(path: &ShapePath) -> String {
    let point = |emu: i64| {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a frame is far below 2^52 units"
        )]
        let emu = emu as f64;
        number(emu / EMU_PER_POINT)
    };
    let mut data = String::new();
    for command in &path.commands {
        let _ = match command {
            PathCommand::MoveTo(to) => write!(data, "M{} {}", point(to.x), point(to.y)),
            PathCommand::LineTo(to) => write!(data, "L{} {}", point(to.x), point(to.y)),
            PathCommand::CubicTo(first, second, to) => write!(
                data,
                "C{} {} {} {} {} {}",
                point(first.x),
                point(first.y),
                point(second.x),
                point(second.y),
                point(to.x),
                point(to.y)
            ),
            PathCommand::Close => write!(data, "Z"),
        };
    }
    data
}

fn number(value: f64) -> String {
    if !value.is_finite() {
        return "0".to_owned();
    }
    let mut text = format!("{:.2}", (value * 100.0).round() / 100.0);
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
    text.chars()
        .filter(|character| !character.is_control() || *character == '\t')
        .map(|character| match character {
            '&' => "&amp;".to_owned(),
            '<' => "&lt;".to_owned(),
            '>' => "&gt;".to_owned(),
            '\t' => " ".to_owned(),
            other => other.to_string(),
        })
        .collect()
}

/// The outline of a frame's rectangle, for a shape whose geometry gives none.
pub(crate) fn rectangle_paths(width: i64, height: i64) -> Vec<ShapePath> {
    use super::PathPoint;
    let at = |x: i64, y: i64| PathPoint { x, y };
    vec![ShapePath {
        commands: vec![
            PathCommand::MoveTo(at(0, 0)),
            PathCommand::LineTo(at(width, 0)),
            PathCommand::LineTo(at(width, height)),
            PathCommand::LineTo(at(0, height)),
            PathCommand::Close,
        ],
        filled: true,
        stroked: true,
        shade: FillShade::Normal,
    }]
}

#[cfg(test)]
mod tests;
