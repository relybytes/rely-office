// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The enhanced geometry of `OpenDocument` (ODF 1.3, 19.145 and 19.171): `draw:equation`
//! formulas over the view box and the modifiers, and the commands of `draw:enhanced-path`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::f64::consts::{PI, TAU};

use super::drawingml::{arc_to, preset_geometry_with_values};
use super::{PathBuilder, ShapePath, flip_paths};
use crate::xml_tree::{Element, Namespace};

const DRAW: Namespace = Namespace::OdfDraw;
const SVG: Namespace = Namespace::OdfSvg;
/// A frame's logical size is in hundredths of a millimetre.
const EMU_PER_HUNDREDTH_MM: f64 = 360.0;
/// No formula nests deeper than this, nor references equations in a longer chain.
const MAX_DEPTH: usize = 64;
/// No path holds more parameters than this.
const MAX_TOKENS: usize = 16_384;
/// The magic number of a quarter ellipse drawn as one cubic curve.
const KAPPA: f64 = 0.552_284_749_8;

/// A parsed formula.
#[derive(Debug, Clone, PartialEq)]
enum Expression {
    Number(f64),
    Variable(String),
    Equation(usize),
    Modifier(usize),
    Negate(Box<Expression>),
    Binary(char, Box<Expression>, Box<Expression>),
    Call(String, Vec<Expression>),
}

/// Reads a formula: sums of products of numbers, names, `?fN`, `$N`, calls and brackets.
struct Parser<'a> {
    text: &'a [u8],
    position: usize,
    depth: usize,
}

impl Parser<'_> {
    fn skip_space(&mut self) {
        while self
            .text
            .get(self.position)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.position += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_space();
        self.text.get(self.position).copied()
    }

    fn expression(&mut self) -> Option<Expression> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return None;
        }
        let mut left = self.term()?;
        while let Some(operator @ (b'+' | b'-')) = self.peek() {
            self.position += 1;
            let right = self.term()?;
            left = Expression::Binary(char::from(operator), Box::new(left), Box::new(right));
        }
        self.depth -= 1;
        Some(left)
    }

    fn term(&mut self) -> Option<Expression> {
        let mut left = self.factor()?;
        while let Some(operator @ (b'*' | b'/')) = self.peek() {
            self.position += 1;
            let right = self.factor()?;
            left = Expression::Binary(char::from(operator), Box::new(left), Box::new(right));
        }
        Some(left)
    }

    fn factor(&mut self) -> Option<Expression> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return None;
        }
        let factor = match self.peek()? {
            b'-' => {
                self.position += 1;
                Expression::Negate(Box::new(self.factor()?))
            }
            b'+' => {
                self.position += 1;
                self.factor()?
            }
            b'(' => {
                self.position += 1;
                let inner = self.expression()?;
                (self.peek() == Some(b')')).then(|| self.position += 1)?;
                inner
            }
            b'?' => {
                self.position += 1;
                let name = self.word();
                Expression::Equation(name.strip_prefix('f')?.parse().ok()?)
            }
            b'$' => {
                self.position += 1;
                Expression::Modifier(self.word().parse().ok()?)
            }
            character if character.is_ascii_digit() || character == b'.' => {
                let start = self.position;
                while self.text.get(self.position).is_some_and(|character| {
                    character.is_ascii_digit() || matches!(character, b'.' | b'e' | b'E')
                }) {
                    self.position += 1;
                }
                let number = std::str::from_utf8(&self.text[start..self.position]).ok()?;
                Expression::Number(number.parse().ok()?)
            }
            character if character.is_ascii_alphabetic() => {
                let name = self.word();
                if self.peek() == Some(b'(') {
                    self.position += 1;
                    let mut arguments = Vec::new();
                    if self.peek() == Some(b')') {
                        self.position += 1;
                    } else {
                        loop {
                            arguments.push(self.expression()?);
                            match self.peek()? {
                                b',' => self.position += 1,
                                b')' => {
                                    self.position += 1;
                                    break;
                                }
                                _ => return None,
                            }
                        }
                    }
                    Expression::Call(name, arguments)
                } else {
                    Expression::Variable(name)
                }
            }
            _ => return None,
        };
        self.depth -= 1;
        Some(factor)
    }

    fn word(&mut self) -> String {
        let start = self.position;
        while self
            .text
            .get(self.position)
            .is_some_and(u8::is_ascii_alphanumeric)
        {
            self.position += 1;
        }
        String::from_utf8_lossy(&self.text[start..self.position]).into_owned()
    }
}

fn parse_formula(text: &str) -> Option<Expression> {
    let mut parser = Parser {
        text: text.as_bytes(),
        position: 0,
        depth: 0,
    };
    let expression = parser.expression()?;
    (parser.peek().is_none()).then_some(expression)
}

/// The values a geometry's formulas read.
struct Context {
    variables: HashMap<&'static str, f64>,
    modifiers: Vec<f64>,
    equations: Vec<Option<Expression>>,
    /// The value of each equation once computed, `None` while it is being computed.
    values: RefCell<HashMap<usize, Option<f64>>>,
}

impl Context {
    fn evaluate(&self, expression: &Expression, depth: usize) -> f64 {
        if depth > MAX_DEPTH {
            return 0.0;
        }
        let value = |inner: &Expression| self.evaluate(inner, depth + 1);
        let result = match expression {
            Expression::Number(number) => *number,
            Expression::Variable(name) => self.variables.get(name.as_str()).copied().unwrap_or(0.0),
            Expression::Equation(index) => self.equation(*index, depth + 1),
            Expression::Modifier(index) => self.modifiers.get(*index).copied().unwrap_or(0.0),
            Expression::Negate(inner) => -value(inner),
            Expression::Binary(operator, left, right) => {
                let (left, right) = (value(left), value(right));
                match operator {
                    '+' => left + right,
                    '-' => left - right,
                    '*' => left * right,
                    _ if right == 0.0 => 0.0,
                    _ => left / right,
                }
            }
            Expression::Call(name, arguments) => {
                let argument = |index: usize| arguments.get(index).map_or(0.0, value);
                match name.as_str() {
                    "abs" => argument(0).abs(),
                    "sqrt" => argument(0).max(0.0).sqrt(),
                    "sin" => argument(0).sin(),
                    "cos" => argument(0).cos(),
                    "tan" => argument(0).tan(),
                    "atan" => argument(0).atan(),
                    "atan2" => argument(0).atan2(argument(1)),
                    "min" => argument(0).min(argument(1)),
                    "max" => argument(0).max(argument(1)),
                    "if" => {
                        if argument(0) > 0.0 {
                            argument(1)
                        } else {
                            argument(2)
                        }
                    }
                    _ => 0.0,
                }
            }
        };
        if result.is_finite() { result } else { 0.0 }
    }

    fn equation(&self, index: usize, depth: usize) -> f64 {
        if let Some(known) = self.values.borrow().get(&index) {
            return known.unwrap_or(0.0);
        }
        let Some(Some(expression)) = self.equations.get(index) else {
            return 0.0;
        };
        self.values.borrow_mut().insert(index, None);
        let value = self.evaluate(expression, depth);
        self.values.borrow_mut().insert(index, Some(value));
        value
    }

    /// A path parameter: a number, `?fN` or `$N`.
    fn parameter(&self, token: &str) -> f64 {
        if let Some(index) = token
            .strip_prefix("?f")
            .and_then(|index| index.parse().ok())
        {
            self.equation(index, 0)
        } else if let Some(index) = token
            .strip_prefix('$')
            .and_then(|index| index.parse::<usize>().ok())
        {
            self.modifiers.get(index).copied().unwrap_or(0.0)
        } else {
            token
                .parse::<f64>()
                .ok()
                .filter(|number| number.is_finite())
                .unwrap_or(0.0)
        }
    }
}

/// The path split into command letters and their parameters.
fn tokens(path: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let flush = |current: &mut String, tokens: &mut Vec<String>| {
        if !current.is_empty() && tokens.len() < MAX_TOKENS {
            tokens.push(std::mem::take(current));
        }
        current.clear();
    };
    for character in path.chars() {
        if character.is_whitespace() || character == ',' {
            flush(&mut current, &mut tokens);
        } else if character.is_ascii_alphabetic() && current.is_empty() {
            tokens.push(character.to_string());
        } else if character.is_ascii_alphabetic() && !current.starts_with('?') {
            flush(&mut current, &mut tokens);
            tokens.push(character.to_string());
        } else {
            current.push(character);
        }
    }
    flush(&mut current, &mut tokens);
    tokens
}

fn is_command(token: &str) -> bool {
    token.len() == 1
        && token
            .chars()
            .all(|character| character.is_ascii_alphabetic())
}

/// The paths a `draw:enhanced-geometry` draws in a frame of this size; `None` when it has no
/// path.
#[expect(
    clippy::too_many_lines,
    reason = "the view box, the variables and the equations are read in one place"
)]
pub(crate) fn enhanced_geometry(
    geometry: &Element,
    width: i64,
    height: i64,
) -> Option<Vec<ShapePath>> {
    let mirrored = |name: &str| geometry.attribute(DRAW, name) == Some("true");
    let modifiers: Vec<f64> = geometry
        .attribute(DRAW, "modifiers")
        .unwrap_or_default()
        .split_whitespace()
        .filter_map(|number| number.parse().ok())
        .collect();
    // A shape from a `DrawingML` file keeps its preset's name, and its saved path can miss the
    // arcs, so the preset is drawn again from its adjust values.
    if let Some(mut paths) = geometry
        .attribute(DRAW, "type")
        .and_then(|kind| kind.strip_prefix("ooxml-"))
        .and_then(|preset| preset_geometry_with_values(preset, &modifiers, width, height))
    {
        flip_paths(
            &mut paths,
            width,
            height,
            mirrored("mirror-horizontal"),
            mirrored("mirror-vertical"),
        );
        return Some(paths);
    }
    let path = geometry.attribute(DRAW, "enhanced-path")?;
    #[expect(
        clippy::cast_precision_loss,
        reason = "a frame is far below 2^52 units"
    )]
    let (frame_width, frame_height) = (width.max(0) as f64, height.max(0) as f64);
    let view_box: Vec<f64> = geometry
        .attribute(SVG, "viewBox")
        .unwrap_or("0 0 21600 21600")
        .split_whitespace()
        .filter_map(|number| number.parse().ok())
        .collect();
    let logical = (
        frame_width / EMU_PER_HUNDREDTH_MM,
        frame_height / EMU_PER_HUNDREDTH_MM,
    );
    let (left, top, mut space_width, mut space_height) = match view_box.as_slice() {
        [left, top, width, height] => (*left, *top, *width, *height),
        _ => (0.0, 0.0, 21_600.0, 21_600.0),
    };
    if space_width <= 0.0 || space_height <= 0.0 {
        (space_width, space_height) = logical;
    }
    let mut variables = HashMap::new();
    for (name, value) in [
        ("pi", PI),
        ("left", left),
        ("top", top),
        ("right", left + space_width),
        ("bottom", top + space_height),
        ("width", space_width),
        ("height", space_height),
        ("logwidth", logical.0),
        ("logheight", logical.1),
        ("hasstroke", 1.0),
        ("hasfill", 1.0),
        ("xstretch", 0.0),
        ("ystretch", 0.0),
    ] {
        variables.insert(name, value);
    }
    let equations_by_name: HashMap<usize, Option<Expression>> = geometry
        .elements()
        .filter(|element| element.is(DRAW, "equation"))
        .filter_map(|equation| {
            let index = equation
                .attribute(DRAW, "name")?
                .strip_prefix('f')?
                .parse()
                .ok()?;
            Some((
                index,
                equation.attribute(DRAW, "formula").and_then(parse_formula),
            ))
        })
        .filter(|(index, _)| *index < MAX_TOKENS)
        .collect();
    let count = equations_by_name.keys().max().map_or(0, |last| last + 1);
    let mut equations = vec![None; count];
    for (index, expression) in equations_by_name {
        equations[index] = expression;
    }
    let context = Context {
        variables,
        modifiers,
        equations,
        values: RefCell::new(HashMap::new()),
    };
    let scale = (
        if space_width > 0.0 {
            frame_width / space_width
        } else {
            1.0
        },
        if space_height > 0.0 {
            frame_height / space_height
        } else {
            1.0
        },
    );
    let mut paths = draw(&tokens(path), &context, (left, top), scale);
    if paths.is_empty() {
        return None;
    }
    flip_paths(
        &mut paths,
        width,
        height,
        mirrored("mirror-horizontal"),
        mirrored("mirror-vertical"),
    );
    Some(paths)
}

/// The subpath being drawn and what it fills and strokes.
struct Pen {
    builder: PathBuilder,
    filled: bool,
    stroked: bool,
}

impl Pen {
    fn new() -> Self {
        Self {
            builder: PathBuilder::default(),
            filled: true,
            stroked: true,
        }
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "one arm per command of the path language"
)]
fn draw(
    tokens: &[String],
    context: &Context,
    origin: (f64, f64),
    scale: (f64, f64),
) -> Vec<ShapePath> {
    let at = |x: f64, y: f64| ((x - origin.0) * scale.0, (y - origin.1) * scale.1);
    let mut paths = Vec::new();
    let mut pen = Pen::new();
    let mut index = 0;
    let mut command = 'M';
    let mut quadrant_horizontal = true;
    while index < tokens.len() {
        let token = &tokens[index];
        if is_command(token) {
            command = token.chars().next().unwrap_or('M');
            index += 1;
            match command {
                'Z' => pen.builder.close(),
                'N' => {
                    let finished = std::mem::replace(&mut pen, Pen::new());
                    paths.extend(finished.builder.finish(finished.filled, finished.stroked));
                }
                'F' => pen.filled = false,
                'S' => pen.stroked = false,
                'X' => quadrant_horizontal = true,
                'Y' => quadrant_horizontal = false,
                _ => {}
            }
            continue;
        }
        let count = match command {
            'M' | 'L' | 'X' | 'Y' => 2,
            'Q' | 'G' => 4,
            'C' | 'T' | 'U' => 6,
            'A' | 'B' | 'W' | 'V' => 8,
            _ => {
                index += 1;
                continue;
            }
        };
        let arguments: Vec<f64> = tokens[index..]
            .iter()
            .take_while(|token| !is_command(token))
            .take(count)
            .map(|token| context.parameter(token))
            .collect();
        index += arguments.len();
        if arguments.len() < count {
            continue;
        }
        match command {
            'M' => {
                let (x, y) = at(arguments[0], arguments[1]);
                pen.builder.move_to(x, y);
                command = 'L';
            }
            'L' => {
                let (x, y) = at(arguments[0], arguments[1]);
                pen.builder.line_to(x, y);
            }
            'C' => pen.builder.cubic_to(
                at(arguments[0], arguments[1]),
                at(arguments[2], arguments[3]),
                at(arguments[4], arguments[5]),
            ),
            'Q' => pen.builder.quad_to(
                at(arguments[0], arguments[1]),
                at(arguments[2], arguments[3]),
            ),
            'T' | 'U' => {
                let centre = at(arguments[0], arguments[1]);
                let radii = (arguments[2] * scale.0, arguments[3] * scale.1);
                let (start, mut end) = (arguments[4], arguments[5]);
                if end <= start {
                    end += 360.0;
                }
                if command == 'U' {
                    let first = -start.to_radians();
                    let x = centre.0 + radii.0 * first.cos();
                    let y = centre.1 + radii.1 * first.sin();
                    pen.builder.move_to(x, y);
                }
                pen.builder.ellipse_arc(
                    centre,
                    radii,
                    -start.to_radians(),
                    -(end - start).to_radians(),
                );
            }
            'A' | 'B' | 'W' | 'V' => {
                let (x1, y1) = at(arguments[0], arguments[1]);
                let (x2, y2) = at(arguments[2], arguments[3]);
                let centre = (f64::midpoint(x1, x2), f64::midpoint(y1, y2));
                let radii = ((x2 - x1).abs() / 2.0, (y2 - y1).abs() / 2.0);
                if radii.0 <= 0.0 || radii.1 <= 0.0 {
                    continue;
                }
                let angle = |x: f64, y: f64| {
                    let (x, y) = at(x, y);
                    ((y - centre.1) / radii.1).atan2((x - centre.0) / radii.0)
                };
                let start = angle(arguments[4], arguments[5]);
                let end = angle(arguments[6], arguments[7]);
                let clockwise = matches!(command, 'W' | 'V');
                let mut sweep = if clockwise {
                    (end - start).rem_euclid(TAU)
                } else {
                    -(start - end).rem_euclid(TAU)
                };
                if sweep.abs() < 1e-9 {
                    sweep = if clockwise { TAU } else { -TAU };
                }
                if matches!(command, 'B' | 'V') {
                    let x = centre.0 + radii.0 * start.cos();
                    let y = centre.1 + radii.1 * start.sin();
                    pen.builder.move_to(x, y);
                }
                pen.builder.ellipse_arc(centre, radii, start, sweep);
            }
            'X' | 'Y' => {
                let (x0, y0) = pen.builder.current();
                let (x1, y1) = at(arguments[0], arguments[1]);
                let (first, second) = if quadrant_horizontal {
                    ((x0 + KAPPA * (x1 - x0), y0), (x1, y1 + KAPPA * (y0 - y1)))
                } else {
                    ((x0, y0 + KAPPA * (y1 - y0)), (x1 + KAPPA * (x0 - x1), y1))
                };
                pen.builder.cubic_to(first, second, (x1, y1));
                quadrant_horizontal = !quadrant_horizontal;
            }
            'G' => {
                let radii = (arguments[0] * scale.0, arguments[1] * scale.1);
                arc_to(
                    &mut pen.builder,
                    radii,
                    arguments[2].to_radians(),
                    arguments[3].to_radians(),
                );
            }
            _ => {}
        }
    }
    paths.extend(pen.builder.finish(pen.filled, pen.stroked));
    paths
}

#[cfg(test)]
mod tests;
