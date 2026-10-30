// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! The outline of a drawn shape as paths of lines and curves, in English Metric Units from the
//! top-left corner of its frame. The presets and custom geometries of `DrawingML` and the
//! enhanced geometries of `OpenDocument` are both read into it.

mod drawingml;
mod enhanced;
mod enhanced_presets;
mod picture;
mod presets;
mod svg;

pub(crate) use drawingml::{custom_geometry, preset_geometry, preset_geometry_with_values};
pub(crate) use enhanced::enhanced_geometry;
pub(crate) use picture::{DrawnShape, TextLine, rectangle_paths, shapes_svg};
pub(crate) use svg::svg_path_geometry;

use std::f64::consts::{FRAC_PI_2, TAU};

/// A point of a path, in English Metric Units from the top-left corner of the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct PathPoint {
    pub x: i64,
    pub y: i64,
}

/// One step of a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(into = "PathCommandJson", from = "PathCommandJson")
)]
pub enum PathCommand {
    /// Starts a new figure.
    MoveTo(PathPoint),
    LineTo(PathPoint),
    /// A cubic Bezier curve through two control points; arcs are drawn this way too.
    CubicTo(PathPoint, PathPoint, PathPoint),
    /// Closes the figure back to where it started.
    Close,
}

/// The JSON of a path step, tagged by `type` like the rest of the slide model.
#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum PathCommandJson {
    MoveTo {
        to: PathPoint,
    },
    LineTo {
        to: PathPoint,
    },
    CubicTo {
        controls: [PathPoint; 2],
        to: PathPoint,
    },
    Close,
}

#[cfg(feature = "serde")]
impl From<PathCommand> for PathCommandJson {
    fn from(command: PathCommand) -> Self {
        match command {
            PathCommand::MoveTo(to) => Self::MoveTo { to },
            PathCommand::LineTo(to) => Self::LineTo { to },
            PathCommand::CubicTo(first, second, to) => Self::CubicTo {
                controls: [first, second],
                to,
            },
            PathCommand::Close => Self::Close,
        }
    }
}

#[cfg(feature = "serde")]
impl From<PathCommandJson> for PathCommand {
    fn from(command: PathCommandJson) -> Self {
        match command {
            PathCommandJson::MoveTo { to } => Self::MoveTo(to),
            PathCommandJson::LineTo { to } => Self::LineTo(to),
            PathCommandJson::CubicTo {
                controls: [first, second],
                to,
            } => Self::CubicTo(first, second, to),
            PathCommandJson::Close => Self::Close,
        }
    }
}

/// The outline of a `DrawingML` preset at a size in EMU, with its default adjust values;
/// `None` for a name no preset has.
#[must_use]
pub fn preset_outline(preset: &str, width: i64, height: i64) -> Option<Vec<ShapePath>> {
    preset_geometry(preset, None, width, height)
}

/// A path of a shape, with whether it is filled and whether its outline is drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct ShapePath {
    pub commands: Vec<PathCommand>,
    pub filled: bool,
    pub stroked: bool,
    /// How the fill is shaded against the shape's own colour, as a face of a cube.
    pub shade: FillShade,
}

/// A face drawn darker or lighter than the shape's fill, as `DrawingML` paths and the shading
/// commands of `OpenDocument` enhanced paths ask.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub enum FillShade {
    #[default]
    Normal,
    Darken,
    DarkenLess,
    Lighten,
    LightenLess,
}

impl FillShade {
    /// The shade a `DrawingML` path's `fill` names; `None` for no fill.
    pub(crate) fn from_drawingml(fill: Option<&str>) -> Option<Self> {
        Some(match fill {
            Some("none") => return None,
            Some("darken") => Self::Darken,
            Some("darkenLess") => Self::DarkenLess,
            Some("lighten") => Self::Lighten,
            Some("lightenLess") => Self::LightenLess,
            _ => Self::Normal,
        })
    }

    /// The `fill` attribute of a `DrawingML` path with this shade.
    #[must_use]
    pub fn drawingml_name(self) -> &'static str {
        match self {
            Self::Normal => "norm",
            Self::Darken => "darken",
            Self::DarkenLess => "darkenLess",
            Self::Lighten => "lighten",
            Self::LightenLess => "lightenLess",
        }
    }

    /// An `RRGGBB` colour shaded: darker faces scale the channels down, lighter ones mix them
    /// with white.
    #[must_use]
    pub fn apply(self, colour: &str) -> String {
        let factor = match self {
            Self::Normal => return colour.to_owned(),
            Self::Darken => -0.4,
            Self::DarkenLess => -0.2,
            Self::Lighten => 0.4,
            Self::LightenLess => 0.2,
        };
        let channel = |index: usize| {
            colour
                .get(index..index + 2)
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
        };
        let (Some(red), Some(green), Some(blue)) = (channel(0), channel(2), channel(4)) else {
            return colour.to_owned();
        };
        let shaded = |value: u8| {
            let value = f64::from(value);
            let result = if factor < 0.0 {
                value * (1.0 + factor)
            } else {
                value + (255.0 - value) * factor
            };
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "the value is clamped to a byte"
            )]
            let byte = result.round().clamp(0.0, 255.0) as u8;
            byte
        };
        format!(
            "{:02X}{:02X}{:02X}",
            shaded(red),
            shaded(green),
            shaded(blue)
        )
    }
}

/// No path runs more commands than this; a geometry past it is drawn as far as it goes.
const MAX_COMMANDS: usize = 4_096;
/// No coordinate reaches past this many units, a few times the largest slide.
const MAX_COORDINATE: f64 = 2.0e9;

/// Builds a path in floating point, the current point kept for the arcs.
#[derive(Debug, Default)]
pub(crate) struct PathBuilder {
    commands: Vec<PathCommand>,
    current: (f64, f64),
    start: (f64, f64),
}

fn point(x: f64, y: f64) -> PathPoint {
    let clamp = |value: f64| {
        let value = if value.is_finite() { value } else { 0.0 };
        #[expect(clippy::cast_possible_truncation, reason = "the value is clamped")]
        let value = value.round().clamp(-MAX_COORDINATE, MAX_COORDINATE) as i64;
        value
    };
    PathPoint {
        x: clamp(x),
        y: clamp(y),
    }
}

impl PathBuilder {
    fn push(&mut self, command: PathCommand) {
        if self.commands.len() < MAX_COMMANDS {
            self.commands.push(command);
        }
    }

    pub(crate) fn move_to(&mut self, x: f64, y: f64) {
        self.push(PathCommand::MoveTo(point(x, y)));
        self.current = (x, y);
        self.start = (x, y);
    }

    /// A line to the point, or the first point of the path when nothing is drawn yet.
    pub(crate) fn line_to(&mut self, x: f64, y: f64) {
        if self.commands.is_empty() {
            self.move_to(x, y);
            return;
        }
        self.push(PathCommand::LineTo(point(x, y)));
        self.current = (x, y);
    }

    pub(crate) fn cubic_to(&mut self, first: (f64, f64), second: (f64, f64), end: (f64, f64)) {
        if self.commands.is_empty() {
            self.move_to(self.current.0, self.current.1);
        }
        self.push(PathCommand::CubicTo(
            point(first.0, first.1),
            point(second.0, second.1),
            point(end.0, end.1),
        ));
        self.current = end;
    }

    /// A quadratic curve, drawn as the cubic curve that is the same line.
    pub(crate) fn quad_to(&mut self, control: (f64, f64), end: (f64, f64)) {
        let (x0, y0) = self.current;
        let first = (
            x0 + 2.0 / 3.0 * (control.0 - x0),
            y0 + 2.0 / 3.0 * (control.1 - y0),
        );
        let second = (
            end.0 + 2.0 / 3.0 * (control.0 - end.0),
            end.1 + 2.0 / 3.0 * (control.1 - end.1),
        );
        self.cubic_to(first, second, end);
    }

    pub(crate) fn close(&mut self) {
        if !self.commands.is_empty() {
            self.push(PathCommand::Close);
            self.current = self.start;
        }
    }

    pub(crate) fn current(&self) -> (f64, f64) {
        self.current
    }

    /// An arc of the ellipse centred on `centre` with radii `radii`, from the parametric angle
    /// `start` through `sweep`, both in radians, clockwise on the page when positive. A line
    /// leads to its start when the pen is elsewhere.
    pub(crate) fn ellipse_arc(
        &mut self,
        centre: (f64, f64),
        radii: (f64, f64),
        start: f64,
        sweep: f64,
    ) {
        let (cx, cy) = centre;
        let (rx, ry) = radii;
        let at = |angle: f64| (cx + rx * angle.cos(), cy + ry * angle.sin());
        let first = at(start);
        if self.commands.is_empty() {
            self.move_to(first.0, first.1);
        } else if (self.current.0 - first.0).abs() > 0.5 || (self.current.1 - first.1).abs() > 0.5 {
            self.line_to(first.0, first.1);
        }
        let sweep = sweep.clamp(-TAU * 4.0, TAU * 4.0);
        if !sweep.is_finite() || sweep.abs() < 1e-9 {
            return;
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "at most sixteen quarter turns"
        )]
        let pieces = (sweep.abs() / FRAC_PI_2).ceil().max(1.0) as usize;
        #[expect(clippy::cast_precision_loss, reason = "a few pieces")]
        let step = sweep / pieces as f64;
        let handle = 4.0 / 3.0 * (step / 4.0).tan();
        let mut angle = start;
        for _ in 0..pieces {
            let next = angle + step;
            let (x0, y0) = at(angle);
            let (x1, y1) = at(next);
            let first = (
                x0 - handle * rx * angle.sin(),
                y0 + handle * ry * angle.cos(),
            );
            let second = (x1 + handle * rx * next.sin(), y1 - handle * ry * next.cos());
            self.cubic_to(first, second, (x1, y1));
            angle = next;
        }
    }

    pub(crate) fn finish(self, filled: bool, stroked: bool) -> Option<ShapePath> {
        (!self.commands.is_empty()).then_some(ShapePath {
            commands: self.commands,
            filled,
            stroked,
            shade: FillShade::Normal,
        })
    }
}

/// The paths mirrored across the frame: left to right, top to bottom, or both.
pub(crate) fn flip_paths(
    paths: &mut [ShapePath],
    width: i64,
    height: i64,
    horizontal: bool,
    vertical: bool,
) {
    if !horizontal && !vertical {
        return;
    }
    let flip = |point: &mut PathPoint| {
        if horizontal {
            point.x = width - point.x;
        }
        if vertical {
            point.y = height - point.y;
        }
    };
    for path in paths {
        for command in &mut path.commands {
            match command {
                PathCommand::MoveTo(point) | PathCommand::LineTo(point) => flip(point),
                PathCommand::CubicTo(first, second, end) => {
                    flip(first);
                    flip(second);
                    flip(end);
                }
                PathCommand::Close => {}
            }
        }
    }
}

#[cfg(test)]
mod tests;
