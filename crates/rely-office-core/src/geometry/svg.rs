// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! The outlines `OpenDocument` draws in SVG terms: the `svg:points` of a polygon or a
//! polyline and the `svg:d` of a path, in the coordinates of the shape's `svg:viewBox`.

use std::f64::consts::TAU;

use super::{PathBuilder, ShapePath};
use crate::xml_tree::{Element, Namespace};

const SVG: Namespace = Namespace::OdfSvg;
/// No outline holds more numbers than this.
const MAX_NUMBERS: usize = 65_536;

/// The numbers and command letters of path data or a point list.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Token {
    Command(char),
    Number(f64),
}

fn tokens(data: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let bytes = data.as_bytes();
    let mut index = 0;
    while index < bytes.len() && tokens.len() < MAX_NUMBERS {
        let byte = bytes[index];
        if byte.is_ascii_alphabetic() && !matches!(byte, b'e' | b'E') {
            tokens.push(Token::Command(char::from(byte)));
            index += 1;
        } else if byte.is_ascii_digit() || matches!(byte, b'-' | b'+' | b'.') {
            let start = index;
            index += 1;
            let mut seen_dot = byte == b'.';
            while index < bytes.len() {
                let next = bytes[index];
                if next.is_ascii_digit() {
                    index += 1;
                } else if next == b'.' && !seen_dot {
                    seen_dot = true;
                    index += 1;
                } else if matches!(next, b'e' | b'E') {
                    index += 1;
                    if bytes
                        .get(index)
                        .is_some_and(|sign| matches!(sign, b'-' | b'+'))
                    {
                        index += 1;
                    }
                } else {
                    break;
                }
            }
            if let Some(number) = data[start..index]
                .parse::<f64>()
                .ok()
                .filter(|number| number.is_finite())
            {
                tokens.push(Token::Number(number));
            }
        } else {
            index += 1;
        }
    }
    tokens
}

/// The view box a shape's coordinates are in, and the scale onto its frame.
fn mapping(element: &Element, width: f64, height: f64) -> ((f64, f64), (f64, f64)) {
    let view_box: Vec<f64> = element
        .attribute(SVG, "viewBox")
        .unwrap_or_default()
        .split_whitespace()
        .filter_map(|number| number.parse().ok())
        .collect();
    match view_box.as_slice() {
        [left, top, box_width, box_height] if *box_width > 0.0 && *box_height > 0.0 => {
            ((*left, *top), (width / box_width, height / box_height))
        }
        _ => ((0.0, 0.0), (1.0, 1.0)),
    }
}

/// The outline of a `draw:polygon`, `draw:polyline` or `draw:path` in a frame of this size.
pub(crate) fn svg_path_geometry(
    element: &Element,
    width: i64,
    height: i64,
) -> Option<Vec<ShapePath>> {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a frame is far below 2^52 units"
    )]
    let (origin, scale) = mapping(element, width.max(0) as f64, height.max(0) as f64);
    let at = |x: f64, y: f64| ((x - origin.0) * scale.0, (y - origin.1) * scale.1);
    match element.name.as_str() {
        "polygon" | "polyline" => {
            let numbers: Vec<f64> = tokens(element.attribute(Namespace::OdfDraw, "points")?)
                .into_iter()
                .filter_map(|token| match token {
                    Token::Number(number) => Some(number),
                    Token::Command(_) => None,
                })
                .collect();
            let mut builder = PathBuilder::default();
            for pair in numbers.as_chunks::<2>().0 {
                let (x, y) = at(pair[0], pair[1]);
                builder.line_to(x, y);
            }
            let polygon = element.name == "polygon";
            if polygon {
                builder.close();
            }
            Some(vec![builder.finish(polygon, true)?])
        }
        _ => path_data(element.attribute(SVG, "d")?, at).map(|path| vec![path]),
    }
}

/// SVG path data, absolute and relative, arcs included.
#[expect(
    clippy::too_many_lines,
    reason = "one arm per command of the path language"
)]
fn path_data(data: &str, at: impl Fn(f64, f64) -> (f64, f64)) -> Option<ShapePath> {
    let tokens = tokens(data);
    let mut builder = PathBuilder::default();
    let mut index = 0;
    let mut command = 'M';
    let (mut x, mut y) = (0.0_f64, 0.0_f64);
    let (mut start_x, mut start_y) = (0.0_f64, 0.0_f64);
    let mut last_control: Option<(f64, f64)> = None;
    let mut last_quad: Option<(f64, f64)> = None;
    let mut closed_any = false;
    while index < tokens.len() {
        if let Token::Command(letter) = tokens[index] {
            command = letter;
            index += 1;
            if matches!(command, 'Z' | 'z') {
                builder.close();
                closed_any = true;
                (x, y) = (start_x, start_y);
                last_control = None;
                last_quad = None;
            }
            continue;
        }
        let count = match command.to_ascii_uppercase() {
            'M' | 'L' | 'T' => 2,
            'H' | 'V' => 1,
            'S' | 'Q' => 4,
            'C' => 6,
            'A' => 7,
            _ => {
                index += 1;
                continue;
            }
        };
        let numbers: Vec<f64> = tokens[index..]
            .iter()
            .map_while(|token| match token {
                Token::Number(number) => Some(*number),
                Token::Command(_) => None,
            })
            .take(count)
            .collect();
        index += numbers.len();
        if numbers.len() < count {
            continue;
        }
        let relative = command.is_ascii_lowercase();
        let absolute = |dx: f64, dy: f64| if relative { (x + dx, y + dy) } else { (dx, dy) };
        let upper = command.to_ascii_uppercase();
        if !matches!(upper, 'C' | 'S') {
            last_control = None;
        }
        if !matches!(upper, 'Q' | 'T') {
            last_quad = None;
        }
        match upper {
            'M' => {
                (x, y) = absolute(numbers[0], numbers[1]);
                (start_x, start_y) = (x, y);
                let (page_x, page_y) = at(x, y);
                builder.move_to(page_x, page_y);
                command = if relative { 'l' } else { 'L' };
            }
            'L' => {
                (x, y) = absolute(numbers[0], numbers[1]);
                let (page_x, page_y) = at(x, y);
                builder.line_to(page_x, page_y);
            }
            'H' => {
                x = if relative { x + numbers[0] } else { numbers[0] };
                let (page_x, page_y) = at(x, y);
                builder.line_to(page_x, page_y);
            }
            'V' => {
                y = if relative { y + numbers[0] } else { numbers[0] };
                let (page_x, page_y) = at(x, y);
                builder.line_to(page_x, page_y);
            }
            'C' | 'S' => {
                let (first, second, end) = if upper == 'C' {
                    (
                        absolute(numbers[0], numbers[1]),
                        absolute(numbers[2], numbers[3]),
                        absolute(numbers[4], numbers[5]),
                    )
                } else {
                    let first =
                        last_control.map_or((x, y), |(cx, cy)| (2.0 * x - cx, 2.0 * y - cy));
                    (
                        first,
                        absolute(numbers[0], numbers[1]),
                        absolute(numbers[2], numbers[3]),
                    )
                };
                builder.cubic_to(
                    at(first.0, first.1),
                    at(second.0, second.1),
                    at(end.0, end.1),
                );
                last_control = Some(second);
                (x, y) = end;
            }
            'Q' | 'T' => {
                let (control, end) = if upper == 'Q' {
                    (
                        absolute(numbers[0], numbers[1]),
                        absolute(numbers[2], numbers[3]),
                    )
                } else {
                    let control = last_quad.map_or((x, y), |(cx, cy)| (2.0 * x - cx, 2.0 * y - cy));
                    (control, absolute(numbers[0], numbers[1]))
                };
                builder.quad_to(at(control.0, control.1), at(end.0, end.1));
                last_quad = Some(control);
                (x, y) = end;
            }
            'A' => {
                let end = absolute(numbers[5], numbers[6]);
                arc(&mut builder, (x, y), end, &numbers, &at);
                (x, y) = end;
            }
            _ => {}
        }
    }
    builder.finish(closed_any, true)
}

/// An SVG elliptical arc to `end`, from its endpoint form to its centre form (SVG 1.1, F.6.5).
fn arc(
    builder: &mut PathBuilder,
    start: (f64, f64),
    end: (f64, f64),
    numbers: &[f64],
    at: &impl Fn(f64, f64) -> (f64, f64),
) {
    let (mut radius_x, mut radius_y) = (numbers[0].abs(), numbers[1].abs());
    let rotation = numbers[2].to_radians();
    let (large, sweep_flag) = (numbers[3] != 0.0, numbers[4] != 0.0);
    if radius_x < 1e-9 || radius_y < 1e-9 || start == end {
        let (page_x, page_y) = at(end.0, end.1);
        builder.line_to(page_x, page_y);
        return;
    }
    let (sin, cos) = rotation.sin_cos();
    let half_x = (start.0 - end.0) / 2.0;
    let half_y = (start.1 - end.1) / 2.0;
    let x1 = cos * half_x + sin * half_y;
    let y1 = -sin * half_x + cos * half_y;
    let lambda = (x1 * x1) / (radius_x * radius_x) + (y1 * y1) / (radius_y * radius_y);
    if lambda > 1.0 {
        radius_x *= lambda.sqrt();
        radius_y *= lambda.sqrt();
    }
    let numerator = radius_x * radius_x * radius_y * radius_y
        - radius_x * radius_x * y1 * y1
        - radius_y * radius_y * x1 * x1;
    let denominator = radius_x * radius_x * y1 * y1 + radius_y * radius_y * x1 * x1;
    let mut factor = (numerator / denominator).max(0.0).sqrt();
    if large == sweep_flag {
        factor = -factor;
    }
    let prime = (
        factor * radius_x * y1 / radius_y,
        -factor * radius_y * x1 / radius_x,
    );
    let start_angle = ((y1 - prime.1) / radius_y).atan2((x1 - prime.0) / radius_x);
    let end_angle = ((-y1 - prime.1) / radius_y).atan2((-x1 - prime.0) / radius_x);
    let mut sweep = end_angle - start_angle;
    if sweep_flag && sweep < 0.0 {
        sweep += TAU;
    } else if !sweep_flag && sweep > 0.0 {
        sweep -= TAU;
    }
    let centre = (
        cos * prime.0 - sin * prime.1 + f64::midpoint(start.0, end.0),
        sin * prime.0 + cos * prime.1 + f64::midpoint(start.1, end.1),
    );
    // The arc is drawn in the shape's coordinates, then each point mapped onto the frame.
    let pieces = 16;
    for piece in 1..=pieces {
        let angle = start_angle + sweep * f64::from(piece) / f64::from(pieces);
        let (point_sin, point_cos) = angle.sin_cos();
        let local_x = radius_x * point_cos;
        let local_y = radius_y * point_sin;
        let point_x = cos * local_x - sin * local_y + centre.0;
        let point_y = sin * local_x + cos * local_y + centre.1;
        let (page_x, page_y) = at(point_x, point_y);
        builder.line_to(page_x, page_y);
    }
}

#[cfg(test)]
mod tests;
