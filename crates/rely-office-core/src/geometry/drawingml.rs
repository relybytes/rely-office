// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! `DrawingML` geometry: the guide formulas of ECMA-376 20.1.9, evaluated against a shape's
//! size into paths, for the preset shapes and for `a:custGeom`.

use std::collections::HashMap;
use std::f64::consts::{PI, TAU};
use std::sync::OnceLock;

use super::enhanced::enhanced_preset_geometry;
use super::presets::PRESETS;
use super::{FillShade, PathBuilder, ShapePath};
use crate::xml_tree::{Element, Namespace};

const A: Namespace = Namespace::Drawing;
/// Angles are in sixty-thousandths of a degree.
const ANGLE_UNITS: f64 = 60_000.0;
/// No geometry evaluates more guides than this.
const MAX_GUIDES: usize = 512;

/// A value a formula reads: a number, or a guide by name.
#[derive(Debug, Clone, PartialEq)]
enum Argument {
    Number(f64),
    Guide(String),
}

impl Argument {
    fn parse(token: &str) -> Self {
        token
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite())
            .map_or_else(|| Self::Guide(token.to_owned()), Self::Number)
    }
}

/// A guide formula: an operator and up to three arguments.
#[derive(Debug, Clone, PartialEq)]
struct Formula {
    operator: String,
    arguments: Vec<Argument>,
}

impl Formula {
    fn parse(text: &str) -> Option<Self> {
        let mut tokens = text.split_whitespace();
        let operator = tokens.next()?.to_owned();
        let arguments = tokens.take(3).map(Argument::parse).collect();
        Some(Self {
            operator,
            arguments,
        })
    }

    fn evaluate(&self, guides: &HashMap<String, f64>) -> f64 {
        let argument = |index: usize| match self.arguments.get(index) {
            Some(Argument::Number(number)) => *number,
            Some(Argument::Guide(name)) => guides.get(name).copied().unwrap_or(0.0),
            None => 0.0,
        };
        let (x, y, z) = (argument(0), argument(1), argument(2));
        let radians = |angle: f64| angle / ANGLE_UNITS * PI / 180.0;
        let value = match self.operator.as_str() {
            "*/" if z != 0.0 => x * y / z,
            "+-" => x + y - z,
            "+/" if z != 0.0 => (x + y) / z,
            "?:" => {
                if x > 0.0 {
                    y
                } else {
                    z
                }
            }
            "abs" => x.abs(),
            "at2" => y.atan2(x) * 180.0 / PI * ANGLE_UNITS,
            "cat2" => x * z.atan2(y).cos(),
            "sat2" => x * z.atan2(y).sin(),
            "cos" => x * radians(y).cos(),
            "sin" => x * radians(y).sin(),
            "tan" => x * radians(y).tan(),
            "max" => x.max(y),
            "min" => x.min(y),
            "mod" => (x * x + y * y + z * z).sqrt(),
            "pin" => {
                if y < x {
                    x
                } else if y > z {
                    z
                } else {
                    y
                }
            }
            "sqrt" => x.max(0.0).sqrt(),
            "val" => x,
            _ => 0.0,
        };
        if value.is_finite() { value } else { 0.0 }
    }
}

/// One step of a path definition.
#[derive(Debug, Clone, PartialEq)]
enum Step {
    Move(Argument, Argument),
    Line(Argument, Argument),
    Arc([Argument; 4]),
    Quad([Argument; 4]),
    Cubic([Argument; 6]),
    Close,
    /// A regular polygon of this many sides in the frame's ellipse, a vertex at the top.
    Regular(u32),
    /// A star of this many points, its inner radius the `adj` guide in fifty-thousandths.
    Star(u32),
}

#[derive(Debug, Clone, PartialEq)]
struct PathDefinition {
    /// The path's own coordinate space, scaled onto the frame.
    width: Option<f64>,
    height: Option<f64>,
    filled: bool,
    shade: FillShade,
    stroked: bool,
    steps: Vec<Step>,
}

impl Default for PathDefinition {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            filled: true,
            shade: FillShade::Normal,
            stroked: true,
            steps: Vec::new(),
        }
    }
}

/// A geometry: its adjust values, its guides and its paths.
#[derive(Debug, Clone, Default, PartialEq)]
struct Definition {
    adjust: Vec<(String, Formula)>,
    guides: Vec<(String, Formula)>,
    paths: Vec<PathDefinition>,
}

impl Definition {
    /// The paths at a size, the adjust values the shape names over the definition's.
    fn evaluate(&self, width: f64, height: f64, overrides: &[(String, Formula)]) -> Vec<ShapePath> {
        let mut guides = built_in_guides(width, height);
        for (name, formula) in self.adjust.iter().take(MAX_GUIDES) {
            let formula = overrides
                .iter()
                .find(|(other, _)| other == name)
                .map_or(formula, |(_, formula)| formula);
            let value = formula.evaluate(&guides);
            guides.insert(name.clone(), value);
        }
        for (name, formula) in self.guides.iter().take(MAX_GUIDES) {
            let value = formula.evaluate(&guides);
            guides.insert(name.clone(), value);
        }
        self.paths
            .iter()
            .filter_map(|path| draw_path(path, &guides, width, height))
            .collect()
    }
}

fn built_in_guides(width: f64, height: f64) -> HashMap<String, f64> {
    let short = width.min(height);
    let long = width.max(height);
    let mut guides = HashMap::new();
    let mut set = |name: &str, value: f64| {
        guides.insert(name.to_owned(), value);
    };
    set("w", width);
    set("h", height);
    set("l", 0.0);
    set("t", 0.0);
    set("r", width);
    set("b", height);
    set("hc", width / 2.0);
    set("vc", height / 2.0);
    set("ss", short);
    set("ls", long);
    for divisor in [2, 3, 4, 5, 6, 8, 10, 12, 16, 32] {
        let divided = f64::from(divisor);
        set(&format!("wd{divisor}"), width / divided);
        set(&format!("hd{divisor}"), height / divided);
        set(&format!("ssd{divisor}"), short / divided);
    }
    for (name, degrees) in [
        ("cd2", 180.0),
        ("cd4", 90.0),
        ("cd8", 45.0),
        ("3cd4", 270.0),
        ("3cd8", 135.0),
        ("5cd8", 225.0),
        ("7cd8", 315.0),
    ] {
        set(name, degrees * ANGLE_UNITS);
    }
    guides
}

fn draw_path(
    path: &PathDefinition,
    guides: &HashMap<String, f64>,
    width: f64,
    height: f64,
) -> Option<ShapePath> {
    let value = |argument: &Argument| match argument {
        Argument::Number(number) => *number,
        Argument::Guide(name) => guides.get(name).copied().unwrap_or(0.0),
    };
    let scale_x = path
        .width
        .filter(|space| *space > 0.0)
        .map_or(1.0, |space| width / space);
    let scale_y = path
        .height
        .filter(|space| *space > 0.0)
        .map_or(1.0, |space| height / space);
    let at = |x: &Argument, y: &Argument| (value(x) * scale_x, value(y) * scale_y);
    let mut builder = PathBuilder::default();
    for step in &path.steps {
        match step {
            Step::Move(x, y) => {
                let (x, y) = at(x, y);
                builder.move_to(x, y);
            }
            Step::Line(x, y) => {
                let (x, y) = at(x, y);
                builder.line_to(x, y);
            }
            Step::Arc([radius_x, radius_y, start, sweep]) => {
                let radii = (value(radius_x) * scale_x, value(radius_y) * scale_y);
                let start = value(start) / ANGLE_UNITS * PI / 180.0;
                let sweep = value(sweep) / ANGLE_UNITS * PI / 180.0;
                arc_to(&mut builder, radii, start, sweep);
            }
            Step::Quad([x1, y1, x, y]) => builder.quad_to(at(x1, y1), at(x, y)),
            Step::Cubic([x1, y1, x2, y2, x, y]) => {
                builder.cubic_to(at(x1, y1), at(x2, y2), at(x, y));
            }
            Step::Close => builder.close(),
            Step::Regular(sides) => polygon(&mut builder, width, height, *sides, None),
            Step::Star(points) => {
                let inner = guides
                    .get("adj")
                    .copied()
                    .unwrap_or(19_098.0)
                    .clamp(0.0, 50_000.0)
                    / 50_000.0;
                polygon(&mut builder, width, height, *points, Some(inner));
            }
        }
    }
    builder
        .finish(path.filled, path.stroked)
        .map(|finished| ShapePath {
            shade: path.shade,
            ..finished
        })
}

/// The `arcTo` of `DrawingML`: the arc of the ellipse with these radii that passes through the
/// pen at the visual angle `start`, through the visual angle `sweep`.
pub(super) fn arc_to(builder: &mut PathBuilder, radii: (f64, f64), start: f64, sweep: f64) {
    let (radius_x, radius_y) = radii;
    let parametric = |angle: f64| (radius_x * angle.sin()).atan2(radius_y * angle.cos());
    let first = parametric(start);
    let last = parametric(start + sweep);
    let turns = (sweep.abs() / TAU).floor();
    let rest = sweep.abs() % TAU;
    let parametric_sweep = if rest < 1e-9 {
        sweep
    } else if sweep > 0.0 {
        (last - first).rem_euclid(TAU) + turns * TAU
    } else {
        -((first - last).rem_euclid(TAU) + turns * TAU)
    };
    let (x, y) = builder.current();
    let centre = (x - radius_x * first.cos(), y - radius_y * first.sin());
    builder.ellipse_arc(centre, radii, first, parametric_sweep);
}

/// A regular polygon, or a star when the inner radius is given, in the frame's ellipse.
fn polygon(builder: &mut PathBuilder, width: f64, height: f64, corners: u32, inner: Option<f64>) {
    let corners = corners.clamp(3, 64);
    let vertices = if inner.is_some() {
        corners * 2
    } else {
        corners
    };
    let (radius_x, radius_y) = (width / 2.0, height / 2.0);
    for index in 0..vertices {
        let angle = -PI / 2.0 + TAU * f64::from(index) / f64::from(vertices);
        let scale = match inner {
            Some(inner) if index % 2 == 1 => inner,
            _ => 1.0,
        };
        let x = radius_x + radius_x * scale * angle.cos();
        let y = radius_y + radius_y * scale * angle.sin();
        if index == 0 {
            builder.move_to(x, y);
        } else {
            builder.line_to(x, y);
        }
    }
    builder.close();
}

/// The presets, parsed once from their table.
fn presets() -> &'static HashMap<String, Definition> {
    static PARSED: OnceLock<HashMap<String, Definition>> = OnceLock::new();
    PARSED.get_or_init(|| parse_presets(PRESETS))
}

/// The preset table: `[name]` or `[name=other]` heads a definition, then `av name value`,
/// `gd name formula`, `path` with `fill=none`, `stroke=false`, `w=` and `h=`, and the steps
/// `M x y`, `L x y`, `A wR hR stAng swAng`, `Q x1 y1 x y`, `C x1 y1 x2 y2 x y`, `Z`,
/// `regular n` and `star n`.
fn parse_presets(table: &str) -> HashMap<String, Definition> {
    let mut presets: HashMap<String, Definition> = HashMap::new();
    let mut aliases: Vec<(String, String)> = Vec::new();
    let mut current: Option<(String, Definition)> = None;
    for line in table.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some(head) = line
            .strip_prefix('[')
            .and_then(|head| head.strip_suffix(']'))
        {
            if let Some((name, definition)) = current.take() {
                presets.insert(name, definition);
            }
            match head.split_once('=') {
                Some((name, target)) => aliases.push((name.to_owned(), target.to_owned())),
                None => current = Some((head.to_owned(), Definition::default())),
            }
            continue;
        }
        let Some((_, definition)) = current.as_mut() else {
            continue;
        };
        let mut tokens = line.split_whitespace();
        let Some(keyword) = tokens.next() else {
            continue;
        };
        let rest: Vec<&str> = tokens.collect();
        match keyword {
            "av" => {
                if let [name, value] = rest.as_slice() {
                    definition.adjust.push((
                        (*name).to_owned(),
                        Formula {
                            operator: "val".to_owned(),
                            arguments: vec![Argument::parse(value)],
                        },
                    ));
                }
            }
            "gd" => {
                if let Some(formula) = rest
                    .get(1..)
                    .and_then(|formula| Formula::parse(&formula.join(" ")))
                {
                    definition.guides.push((rest[0].to_owned(), formula));
                }
            }
            "path" => {
                let mut path = PathDefinition::default();
                for option in rest {
                    match option.split_once('=') {
                        Some(("fill", fill)) => match FillShade::from_drawingml(Some(fill)) {
                            Some(shade) => path.shade = shade,
                            None => path.filled = false,
                        },
                        Some(("stroke", "false")) => path.stroked = false,
                        Some(("w", value)) => path.width = value.parse().ok(),
                        Some(("h", value)) => path.height = value.parse().ok(),
                        _ => {}
                    }
                }
                definition.paths.push(path);
            }
            _ => {
                if definition.paths.is_empty() {
                    definition.paths.push(PathDefinition::default());
                }
                if let (Some(path), Some(step)) =
                    (definition.paths.last_mut(), step_of(keyword, &rest))
                {
                    path.steps.push(step);
                }
            }
        }
    }
    if let Some((name, definition)) = current.take() {
        presets.insert(name, definition);
    }
    for (name, target) in aliases {
        if let Some(definition) = presets.get(&target).cloned() {
            presets.insert(name, definition);
        }
    }
    presets
}

fn step_of(keyword: &str, arguments: &[&str]) -> Option<Step> {
    let argument = |index: usize| arguments.get(index).map(|token| Argument::parse(token));
    let pair = || Some((argument(0)?, argument(1)?));
    let four = || Some([argument(0)?, argument(1)?, argument(2)?, argument(3)?]);
    Some(match keyword {
        "M" => {
            let (x, y) = pair()?;
            Step::Move(x, y)
        }
        "L" => {
            let (x, y) = pair()?;
            Step::Line(x, y)
        }
        "A" => Step::Arc(four()?),
        "Q" => Step::Quad(four()?),
        "C" => Step::Cubic([
            argument(0)?,
            argument(1)?,
            argument(2)?,
            argument(3)?,
            argument(4)?,
            argument(5)?,
        ]),
        "Z" => Step::Close,
        "regular" => Step::Regular(arguments.first()?.parse().ok()?),
        "star" => Step::Star(arguments.first()?.parse().ok()?),
        _ => return None,
    })
}

/// The adjust values an `a:avLst` names.
fn adjust_values(list: Option<&Element>) -> Vec<(String, Formula)> {
    list.into_iter()
        .flat_map(Element::elements)
        .filter(|guide| guide.is(A, "gd"))
        .filter_map(|guide| {
            let name = guide.attribute(Namespace::None, "name")?;
            let formula = Formula::parse(guide.attribute(Namespace::None, "fmla")?)?;
            Some((name.to_owned(), formula))
        })
        .take(MAX_GUIDES)
        .collect()
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a frame is far below 2^52 units"
)]
fn size(value: i64) -> f64 {
    value.max(0) as f64
}

/// The paths of a preset at a size, with the adjust values of its `a:avLst`; `None` for a
/// preset the table does not hold.
pub(crate) fn preset_geometry(
    preset: &str,
    adjust_list: Option<&Element>,
    width: i64,
    height: i64,
) -> Option<Vec<ShapePath>> {
    let Some(definition) = presets().get(preset) else {
        let named: Vec<(String, f64)> = adjust_values(adjust_list)
            .into_iter()
            .filter(|(_, formula)| formula.operator == "val")
            .filter_map(|(name, formula)| match formula.arguments.first() {
                Some(Argument::Number(value)) => Some((name, *value)),
                _ => None,
            })
            .collect();
        return enhanced_preset_geometry(preset, &[], &named, width, height);
    };
    let paths = definition.evaluate(size(width), size(height), &adjust_values(adjust_list));
    (!paths.is_empty()).then_some(paths)
}

/// The paths of a preset at a size, its adjust values given in the order the preset declares
/// them, as `OpenDocument` keeps them in `draw:modifiers`.
pub(crate) fn preset_geometry_with_values(
    preset: &str,
    values: &[f64],
    width: i64,
    height: i64,
) -> Option<Vec<ShapePath>> {
    let Some(definition) = presets().get(preset) else {
        return enhanced_preset_geometry(preset, values, &[], width, height);
    };
    let overrides: Vec<(String, Formula)> = definition
        .adjust
        .iter()
        .zip(values)
        .map(|((name, _), value)| {
            (
                name.clone(),
                Formula {
                    operator: "val".to_owned(),
                    arguments: vec![Argument::Number(*value)],
                },
            )
        })
        .collect();
    let paths = definition.evaluate(size(width), size(height), &overrides);
    (!paths.is_empty()).then_some(paths)
}

/// The paths of an `a:custGeom` at a size; `None` when it draws nothing.
pub(crate) fn custom_geometry(
    geometry: &Element,
    width: i64,
    height: i64,
) -> Option<Vec<ShapePath>> {
    let mut definition = Definition {
        adjust: adjust_values(geometry.child(A, "avLst")),
        guides: adjust_values(geometry.child(A, "gdLst")),
        paths: Vec::new(),
    };
    let point = |element: &Element| {
        let point = element.child(A, "pt")?;
        Some((
            Argument::parse(point.attribute(Namespace::None, "x")?),
            Argument::parse(point.attribute(Namespace::None, "y")?),
        ))
    };
    let points = |element: &Element| {
        element
            .elements()
            .filter(|child| child.is(A, "pt"))
            .filter_map(|point| {
                Some([
                    Argument::parse(point.attribute(Namespace::None, "x")?),
                    Argument::parse(point.attribute(Namespace::None, "y")?),
                ])
            })
            .flatten()
            .collect::<Vec<_>>()
    };
    for path in geometry
        .child(A, "pathLst")
        .into_iter()
        .flat_map(Element::elements)
        .filter(|path| path.is(A, "path"))
    {
        let length = |name: &str| {
            path.attribute(Namespace::None, name)
                .and_then(|value| value.parse::<f64>().ok())
        };
        let mut definition_path = PathDefinition {
            width: length("w"),
            height: length("h"),
            filled: path.attribute(Namespace::None, "fill") != Some("none"),
            shade: FillShade::from_drawingml(path.attribute(Namespace::None, "fill"))
                .unwrap_or_default(),
            stroked: !matches!(
                path.attribute(Namespace::None, "stroke"),
                Some("0" | "false")
            ),
            steps: Vec::new(),
        };
        for command in path.elements().filter(|command| command.namespace == A) {
            let step = match command.name.as_str() {
                "moveTo" => point(command).map(|(x, y)| Step::Move(x, y)),
                "lnTo" => point(command).map(|(x, y)| Step::Line(x, y)),
                "arcTo" => {
                    let attribute = |name: &str| {
                        command
                            .attribute(Namespace::None, name)
                            .map(Argument::parse)
                    };
                    (|| {
                        Some(Step::Arc([
                            attribute("wR")?,
                            attribute("hR")?,
                            attribute("stAng")?,
                            attribute("swAng")?,
                        ]))
                    })()
                }
                "quadBezTo" => <[Argument; 4]>::try_from(points(command))
                    .ok()
                    .map(Step::Quad),
                "cubicBezTo" => <[Argument; 6]>::try_from(points(command))
                    .ok()
                    .map(Step::Cubic),
                "close" => Some(Step::Close),
                _ => None,
            };
            definition_path.steps.extend(step);
        }
        definition.paths.push(definition_path);
    }
    let paths = definition.evaluate(size(width), size(height), &[]);
    (!paths.is_empty()).then_some(paths)
}

#[cfg(test)]
mod tests;
