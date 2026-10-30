// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! A shape drawn from its own geometry ([MS-ODRAW] 2.3.6): vertices in a coordinate space of
//! its own, the segments that join them (lines, curves, arcs and elliptical quadrants), and the
//! guide formulas a vertex can name instead of a number, as `PowerPoint` and `LibreOffice` save
//! the shapes the old shape types do not have, and as the old shape types themselves are drawn.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use crate::compound::{u16_at, u32_at};
use crate::geometry::{FillShade, PathBuilder, ShapePath};
use crate::office_art::array_elements;

/// No geometry has more guides than this, nor more segments.
const MAX_GUIDES: usize = 128;
const MAX_SEGMENTS: usize = 4_096;
/// Angles are in degrees as 16.16 fixed point.
const ANGLE_UNIT: f64 = 65_536.0;

/// A coordinate: a number, or the value of a guide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Coordinate {
    Number(i64),
    Guide(usize),
}

/// A guide formula (`MSOSG`): its operation with the flags saying which parameters name a
/// value, and its three parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Guide {
    pub(crate) operation: u16,
    pub(crate) parameters: [i32; 3],
}

/// A free-form geometry.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Freeform {
    /// The coordinate space: left, top, right, bottom.
    pub(crate) bounds: [i64; 4],
    pub(crate) adjust: [f64; 10],
    pub(crate) guides: Vec<Guide>,
    pub(crate) vertices: Vec<(Coordinate, Coordinate)>,
    pub(crate) segments: Vec<u16>,
    /// Whether the guides read angles in degrees, as `LibreOffice` writes the old shape types,
    /// rather than in the 16.16 degrees of a file.
    pub(crate) degrees: bool,
    /// Whether the space keeps its proportions across and down, its far edge moved out to the
    /// frame's: what keeps an action button's picture square in a wide or tall button.
    pub(crate) stretch: (bool, bool),
    /// The luminance of each filled path in turn: the count in the top four bits, then a signed
    /// tenth for each path, as the faces of a bevel.
    pub(crate) shades: u32,
}

impl Default for Freeform {
    fn default() -> Self {
        Self {
            bounds: [0, 0, 21_600, 21_600],
            adjust: [0.0; 10],
            guides: Vec::new(),
            vertices: Vec::new(),
            segments: Vec::new(),
            degrees: false,
            stretch: (false, false),
            shades: 0,
        }
    }
}

/// A coordinate of `pVertices`: a 32-bit value with its top bit set names a guide; a 16-bit
/// value is always a number.
pub(super) fn coordinate(value: i64, wide: bool) -> Coordinate {
    let raw = value & 0xFFFF_FFFF;
    if wide && raw & 0x8000_0000 != 0 && raw & 0x7FFF_0000 == 0 {
        Coordinate::Guide(usize::try_from(raw & 0xFFFF).unwrap_or(0))
    } else {
        Coordinate::Number(value)
    }
}

/// The shade of a filled path from its signed tenth of luminance.
fn shade_of(tenths: i32) -> FillShade {
    match tenths {
        0 => FillShade::Normal,
        3.. => FillShade::Lighten,
        1..=2 => FillShade::LightenLess,
        -2..=-1 => FillShade::DarkenLess,
        _ => FillShade::Darken,
    }
}

/// How a path being built is painted, as `noFill` and `noLine` escapes leave it.
#[derive(Debug, Clone, Copy)]
struct Paint {
    filled: bool,
    stroked: bool,
}

impl Default for Paint {
    fn default() -> Self {
        Self {
            filled: true,
            stroked: true,
        }
    }
}

/// The guides being computed: the value of each once known, and those under way.
struct Evaluation {
    values: Vec<Option<f64>>,
    visiting: Vec<bool>,
    /// How far a stretched space reaches past its size, across and down.
    ratios: (f64, f64),
}

/// The scale of a coordinate space onto a frame along one axis.
#[derive(Debug, Clone, Copy)]
struct Axis {
    origin: f64,
    /// The space's size, which a literal coordinate equal to it is moved out with.
    size: f64,
    /// How far a stretched space reaches past its size, 1 when it does not.
    ratio: f64,
    scale: f64,
}

impl Axis {
    fn new(origin: f64, size: f64, frame: f64, other_frame: f64, stretched: bool) -> Self {
        let ratio = if stretched && other_frame > 0.0 {
            (frame / other_frame).max(1.0)
        } else {
            1.0
        };
        Self {
            origin,
            size,
            ratio,
            scale: frame / (size * ratio),
        }
    }

    fn at(&self, value: f64, literal: bool) -> f64 {
        let value = if literal && (value - self.size).abs() < f64::EPSILON {
            value * self.ratio
        } else {
            value
        };
        (value - self.origin) * self.scale
    }
}

impl Freeform {
    pub(crate) fn read_vertices(&mut self, data: &[u8]) {
        self.vertices = array_elements(data)
            .into_iter()
            .filter_map(|element| match element.len() {
                4 => {
                    let read = |at| {
                        u16_at(element, at)
                            .map(|value| i64::from(i16::from_le_bytes(value.to_le_bytes())))
                    };
                    Some((coordinate(read(0)?, false), coordinate(read(2)?, false)))
                }
                8 => {
                    let read = |at| {
                        u32_at(element, at)
                            .map(|value| i64::from(i32::from_le_bytes(value.to_le_bytes())))
                    };
                    Some((coordinate(read(0)?, true), coordinate(read(4)?, true)))
                }
                _ => None,
            })
            .collect();
    }

    pub(crate) fn read_segments(&mut self, data: &[u8]) {
        self.segments = array_elements(data)
            .into_iter()
            .filter_map(|element| u16_at(element, 0))
            .take(MAX_SEGMENTS)
            .collect();
    }

    /// The guides of `pGuides`: a parameter that names a value keeps its number, any other is a
    /// signed 16-bit number.
    pub(crate) fn read_guides(&mut self, data: &[u8]) {
        self.guides = array_elements(data)
            .into_iter()
            .filter(|element| element.len() == 8)
            .filter_map(|element| {
                let operation = u16_at(element, 0)?;
                let mut parameters = [0; 3];
                for (index, parameter) in parameters.iter_mut().enumerate() {
                    let value = u16_at(element, 2 + index * 2)?;
                    *parameter = if operation & (0x2000 << index) != 0 {
                        i32::from(value)
                    } else {
                        i32::from(i16::from_le_bytes(value.to_le_bytes()))
                    };
                }
                Some(Guide {
                    operation,
                    parameters,
                })
            })
            .take(MAX_GUIDES)
            .collect();
    }

    /// The value a guide parameter names: the coordinate space, an adjust value, a guide, or
    /// the space's centre and size. `ratios` move out a stretched far edge.
    fn reference(&self, reference: i32, evaluation: &mut Evaluation) -> f64 {
        let [left, top, right, bottom] = self.bounds.map(|bound| {
            #[expect(clippy::cast_precision_loss, reason = "shape coordinates")]
            let bound = bound as f64;
            bound
        });
        let (right, bottom) = (right * evaluation.ratios.0, bottom * evaluation.ratios.1);
        match reference {
            0x0140 => left,
            0x0141 => top,
            0x0142 => right,
            0x0143 => bottom,
            0x0147..=0x0150 => self.adjust[usize::try_from(reference - 0x0147).unwrap_or(0)],
            0x0400..=0x047F => {
                self.guide_value(usize::try_from(reference - 0x0400).unwrap_or(0), evaluation)
            }
            0x04F8 => f64::midpoint(left, right),
            0x04F9 => f64::midpoint(top, bottom),
            0x04FA => right - left,
            0x04FB => bottom - top,
            _ => 0.0,
        }
    }

    /// The value of every guide. A formula may read a guide written after it, as `LibreOffice`
    /// does; one that reads itself, however far round, is 0.
    pub(crate) fn guide_values(&self, ratios: (f64, f64)) -> Vec<f64> {
        let mut evaluation = Evaluation {
            values: vec![None; self.guides.len()],
            visiting: vec![false; self.guides.len()],
            ratios,
        };
        (0..self.guides.len())
            .map(|index| self.guide_value(index, &mut evaluation))
            .collect()
    }

    fn guide_value(&self, index: usize, evaluation: &mut Evaluation) -> f64 {
        let Some(guide) = self.guides.get(index) else {
            return 0.0;
        };
        if let Some(value) = evaluation.values[index] {
            return value;
        }
        if evaluation.visiting[index] {
            return 0.0;
        }
        evaluation.visiting[index] = true;
        let flags = guide.operation;
        let parameter = |position: usize, evaluation: &mut Evaluation| {
            let value = guide.parameters[position];
            if flags & (0x2000 << position) != 0 {
                self.reference(value, evaluation)
            } else {
                f64::from(value)
            }
        };
        let x = parameter(0, evaluation);
        let y = parameter(1, evaluation);
        let z = parameter(2, evaluation);
        let value = self.formula(flags & 0x1FFF, x, y, z);
        let value = if value.is_finite() { value } else { 0.0 };
        evaluation.visiting[index] = false;
        evaluation.values[index] = Some(value);
        value
    }

    /// A guide's formula over its three parameters ([MS-ODRAW] `MSOSGFORMULA`).
    fn formula(&self, operation: u16, x: f64, y: f64, z: f64) -> f64 {
        // A file's angles are 16.16 degrees; LibreOffice's tables write plain degrees.
        let unit = if self.degrees { 1.0 } else { ANGLE_UNIT };
        let radians = |angle: f64| (angle / unit).to_radians();
        match operation {
            0 => x + y - z,
            1 if z != 0.0 => x * y / z,
            1 if self.degrees => x * y,
            2 => f64::midpoint(x, y),
            3 => x.abs(),
            4 => x.min(y),
            5 => x.max(y),
            6 => {
                if x > 0.0 {
                    y
                } else {
                    z
                }
            }
            7 => (x * x + y * y + z * z).sqrt(),
            8 => y.atan2(x).to_degrees() * unit,
            9 => x * radians(y).sin(),
            10 => x * radians(y).cos(),
            11 => x * z.atan2(y).cos(),
            12 => x * z.atan2(y).sin(),
            13 => x.max(0.0).sqrt(),
            14 => x + (y - z) * unit,
            15 if y != 0.0 => z * (1.0 - (x / y) * (x / y)).max(0.0).sqrt(),
            // LibreOffice reads this tangent's angle in radians, unlike every other.
            16 if self.degrees => x * y.tan(),
            16 => x * radians(y).tan(),
            0x80 => (z * z - x * x).max(0.0).sqrt(),
            0x81 => {
                let angle = z.to_radians();
                angle.cos() * (x - 10_800.0) + angle.sin() * (y - 10_800.0) + 10_800.0
            }
            0x82 => {
                let angle = z.to_radians();
                -(angle.sin() * (x - 10_800.0) - angle.cos() * (y - 10_800.0)) + 10_800.0
            }
            _ => 0.0,
        }
    }

    /// The paths in a frame of this size; `None` when nothing is drawn.
    #[expect(
        clippy::too_many_lines,
        reason = "one arm per segment kind, each short"
    )]
    pub(crate) fn paths(&self, width: i64, height: i64) -> Option<Vec<ShapePath>> {
        if self.vertices.is_empty() {
            return None;
        }
        #[expect(clippy::cast_precision_loss, reason = "shape coordinates")]
        let (frame_width, frame_height) = (width as f64, height as f64);
        let stretch_x = Axis::new(0.0, 1.0, frame_width, frame_height, self.stretch.0).ratio;
        let stretch_y = Axis::new(0.0, 1.0, frame_height, frame_width, self.stretch.1).ratio;
        let values = self.guide_values((stretch_x, stretch_y));
        let value = |coordinate: Coordinate| match coordinate {
            #[expect(clippy::cast_precision_loss, reason = "shape coordinates")]
            Coordinate::Number(number) => number as f64,
            Coordinate::Guide(index) => values.get(index).copied().unwrap_or(0.0),
        };
        #[expect(clippy::cast_precision_loss, reason = "shape coordinates")]
        let (mut left, mut top, mut space_width, mut space_height) = {
            let [left, top, right, bottom] = self.bounds;
            (
                left as f64,
                top as f64,
                (right - left) as f64,
                (bottom - top) as f64,
            )
        };
        // A space of no size, as LibreOffice writes it, is taken from the vertices themselves.
        if space_width <= 0.0 || space_height <= 0.0 {
            let xs = self.vertices.iter().map(|(x, _)| value(*x));
            let ys = self.vertices.iter().map(|(_, y)| value(*y));
            let (min_x, max_x) = xs.fold((f64::MAX, f64::MIN), |(low, high), x| {
                (low.min(x), high.max(x))
            });
            let (min_y, max_y) = ys.fold((f64::MAX, f64::MIN), |(low, high), y| {
                (low.min(y), high.max(y))
            });
            (left, top) = (min_x.min(0.0), min_y.min(0.0));
            space_width = (max_x - left).max(1.0);
            space_height = (max_y - top).max(1.0);
        }
        let across = Axis::new(left, space_width, frame_width, frame_height, self.stretch.0);
        let down = Axis::new(top, space_height, frame_height, frame_width, self.stretch.1);
        let at = |(x, y): (Coordinate, Coordinate)| {
            (
                across.at(value(x), matches!(x, Coordinate::Number(_))),
                down.at(value(y), matches!(y, Coordinate::Number(_))),
            )
        };
        let mut vertices = self.vertices.iter().copied();
        let default_segments;
        let segments: &[u16] = if self.segments.is_empty() {
            default_segments = [
                0x4000,
                u16::try_from(self.vertices.len().saturating_sub(1)).unwrap_or(0),
            ];
            &default_segments
        } else {
            &self.segments
        };
        let mut paths = Vec::new();
        let mut builder = PathBuilder::default();
        let mut paint = Paint::default();
        let mut filled_paths = 0_u32;
        let mut finish = |builder: PathBuilder, paint: Paint, paths: &mut Vec<ShapePath>| {
            if let Some(mut path) = builder.finish(paint.filled, paint.stroked) {
                if paint.filled {
                    path.shade = self.shade(filled_paths);
                    filled_paths += 1;
                }
                paths.push(path);
            }
        };
        for segment in segments {
            let count = usize::from(segment & 0x1FFF).max(1);
            match segment >> 13 {
                0 => {
                    for point in vertices.by_ref().take(count) {
                        let (x, y) = at(point);
                        builder.line_to(x, y);
                    }
                }
                1 => {
                    for _ in 0..count {
                        let (Some(first), Some(second), Some(end)) =
                            (vertices.next(), vertices.next(), vertices.next())
                        else {
                            break;
                        };
                        builder.cubic_to(at(first), at(second), at(end));
                    }
                }
                2 => {
                    if let Some(point) = vertices.next() {
                        let (x, y) = at(point);
                        builder.move_to(x, y);
                    }
                }
                3 => builder.close(),
                4 => {
                    finish(std::mem::take(&mut builder), paint, &mut paths);
                    paint = Paint::default();
                }
                _ => {
                    let points = usize::from(segment & 0x00FF);
                    match (segment >> 8) & 0x1F {
                        0x00 => {
                            for point in vertices.by_ref().take(points.max(1)) {
                                let (x, y) = at(point);
                                builder.line_to(x, y);
                            }
                        }
                        kind @ (0x01 | 0x02) => {
                            for _ in 0..points / 3 {
                                let (Some(centre), Some(radii), Some(angles)) =
                                    (vertices.next(), vertices.next(), vertices.next())
                                else {
                                    break;
                                };
                                self.angle_ellipse(
                                    &mut builder,
                                    at(centre),
                                    (value(radii.0) * across.scale, value(radii.1) * down.scale),
                                    (value(angles.0), value(angles.1)),
                                    kind == 0x02,
                                );
                            }
                        }
                        kind @ 0x03..=0x06 => {
                            for _ in 0..points / 4 {
                                let (Some(first), Some(second), Some(start), Some(end)) = (
                                    vertices.next(),
                                    vertices.next(),
                                    vertices.next(),
                                    vertices.next(),
                                ) else {
                                    break;
                                };
                                arc(
                                    &mut builder,
                                    [at(first), at(second), at(start), at(end)],
                                    matches!(kind, 0x05 | 0x06),
                                    matches!(kind, 0x04 | 0x06),
                                );
                            }
                        }
                        kind @ (0x07 | 0x08) => {
                            let mut across_first = kind == 0x07;
                            for point in vertices.by_ref().take(points) {
                                quadrant(&mut builder, at(point), across_first);
                                across_first = !across_first;
                            }
                        }
                        0x09 => {
                            for _ in 0..points / 2 {
                                let (Some(control), Some(end)) = (vertices.next(), vertices.next())
                                else {
                                    break;
                                };
                                builder.quad_to(at(control), at(end));
                            }
                        }
                        0x0A => paint.filled = false,
                        0x0B => paint.stroked = false,
                        _ => {
                            for _ in 0..points {
                                vertices.next();
                            }
                        }
                    }
                }
            }
        }
        finish(builder, paint, &mut paths);
        (!paths.is_empty()).then_some(paths)
    }

    /// The shade of the filled path at `index`, the last shade going on for any past it.
    fn shade(&self, index: u32) -> FillShade {
        let count = self.shades >> 28;
        if count == 0 {
            return FillShade::Normal;
        }
        let index = index.min(count - 1);
        let nibble = (self.shades << ((index + 1) * 4)) >> 28;
        let tenths = i32::try_from(nibble).unwrap_or(0);
        shade_of(if tenths >= 8 { tenths - 16 } else { tenths })
    }

    /// An arc of an ellipse from its centre, radii, start angle and swing, both counter-clockwise
    /// on the page from the right; `moves` starts a new outline at its start.
    fn angle_ellipse(
        &self,
        builder: &mut PathBuilder,
        centre: (f64, f64),
        radii: (f64, f64),
        angles: (f64, f64),
        moves: bool,
    ) {
        let unit = if self.degrees { 1.0 } else { ANGLE_UNIT };
        let (start, swing) = (
            -(angles.0 / unit).to_radians(),
            -(angles.1 / unit).to_radians(),
        );
        if moves {
            builder.move_to(
                centre.0 + radii.0 * start.cos(),
                centre.1 + radii.1 * start.sin(),
            );
        }
        builder.ellipse_arc(centre, radii, start, swing);
    }
}

/// The parametric angle of a point on an ellipse, as the page measures it downwards.
fn parameter(centre: (f64, f64), radii: (f64, f64), point: (f64, f64)) -> f64 {
    let angle = (point.1 - centre.1).atan2(point.0 - centre.0);
    (radii.0 * angle.sin()).atan2(radii.1 * angle.cos())
}

/// An arc of the ellipse a rectangle bounds, from where the line from its centre to `start`
/// meets it to where the one to `end` does: counter-clockwise on the page, or clockwise.
/// `moves` starts a new outline at its start; otherwise a line leads to it.
fn arc(builder: &mut PathBuilder, points: [(f64, f64); 4], clockwise: bool, moves: bool) {
    let [first, second, start, end] = points;
    let (rx, ry) = (
        (second.0 - first.0).abs() / 2.0,
        (second.1 - first.1).abs() / 2.0,
    );
    if rx <= 0.0 || ry <= 0.0 {
        return;
    }
    let centre = (
        f64::midpoint(first.0, second.0),
        f64::midpoint(first.1, second.1),
    );
    // A rectangle written from its far corner on one axis turns the arc around.
    let (start, end) = if (first.0 > second.0) == (first.1 > second.1) {
        (start, end)
    } else {
        (end, start)
    };
    let (from, to) = (
        parameter(centre, (rx, ry), start),
        parameter(centre, (rx, ry), end),
    );
    let sweep = if clockwise {
        let turn = (to - from).rem_euclid(TAU);
        if turn == 0.0 { TAU } else { turn }
    } else {
        let turn = (from - to).rem_euclid(TAU);
        -(if turn == 0.0 { TAU } else { turn })
    };
    if moves {
        builder.move_to(centre.0 + rx * from.cos(), centre.1 + ry * from.sin());
    }
    builder.ellipse_arc(centre, (rx, ry), from, sweep);
}

/// A quarter of an ellipse from the pen to `end`, leaving it across first or down first.
fn quadrant(builder: &mut PathBuilder, end: (f64, f64), across_first: bool) {
    let start = builder.current();
    let radii = ((end.0 - start.0).abs(), (end.1 - start.1).abs());
    if radii.0 <= 0.0 || radii.1 <= 0.0 {
        builder.line_to(end.0, end.1);
        return;
    }
    let (centre, from, to) = if across_first {
        (
            (start.0, end.1),
            if start.1 > end.1 {
                FRAC_PI_2
            } else {
                -FRAC_PI_2
            },
            if end.0 > start.0 { 0.0 } else { PI },
        )
    } else {
        (
            (end.0, start.1),
            if start.0 > end.0 { 0.0 } else { PI },
            if end.1 > start.1 {
                FRAC_PI_2
            } else {
                -FRAC_PI_2
            },
        )
    };
    let sweep = (to - from + PI).rem_euclid(TAU) - PI;
    builder.ellipse_arc(centre, radii, from, sweep);
}

#[cfg(test)]
mod tests;
