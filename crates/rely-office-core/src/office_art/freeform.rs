// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! A shape drawn from its own geometry ([MS-ODRAW] 2.3.6): vertices in a coordinate space of
//! its own, the segments that join them, and the guide formulas a vertex can name instead of a
//! number, as `PowerPoint` and `LibreOffice` save the shapes the old shape types do not have.

use std::f64::consts::PI;

use crate::compound::{u16_at, u32_at};
use crate::geometry::{PathBuilder, ShapePath};
use crate::office_art::array_elements;

/// No geometry has more guides than this, nor more segments.
const MAX_GUIDES: usize = 128;
const MAX_SEGMENTS: usize = 4_096;
/// Angles are in degrees as 16.16 fixed point.
const ANGLE_UNIT: f64 = 65_536.0;

/// A coordinate: a number, or the value of a guide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Coordinate {
    Number(i64),
    Guide(usize),
}

/// A free-form geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Freeform {
    /// The coordinate space: left, top, right, bottom.
    pub(super) bounds: [i64; 4],
    pub(super) adjust: [i64; 10],
    /// Each guide's formula and its three parameters (`MSOSG`).
    pub(super) guides: Vec<[u16; 4]>,
    pub(super) vertices: Vec<(Coordinate, Coordinate)>,
    pub(super) segments: Vec<u16>,
}

impl Default for Freeform {
    fn default() -> Self {
        Self {
            bounds: [0, 0, 21_600, 21_600],
            adjust: [0; 10],
            guides: Vec::new(),
            vertices: Vec::new(),
            segments: Vec::new(),
        }
    }
}

/// A coordinate of `pVertices`: a 32-bit value with its top bit set names a guide; a 16-bit
/// value is always a number.
fn coordinate(value: i64, wide: bool) -> Coordinate {
    let raw = value & 0xFFFF_FFFF;
    if wide && raw & 0x8000_0000 != 0 && raw & 0x7FFF_0000 == 0 {
        Coordinate::Guide(usize::try_from(raw & 0xFFFF).unwrap_or(0))
    } else {
        Coordinate::Number(value)
    }
}

impl Freeform {
    pub(super) fn read_vertices(&mut self, data: &[u8]) {
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

    pub(super) fn read_segments(&mut self, data: &[u8]) {
        self.segments = array_elements(data)
            .into_iter()
            .filter_map(|element| u16_at(element, 0))
            .take(MAX_SEGMENTS)
            .collect();
    }

    pub(super) fn read_guides(&mut self, data: &[u8]) {
        self.guides = array_elements(data)
            .into_iter()
            .filter(|element| element.len() == 8)
            .filter_map(|element| {
                Some([
                    u16_at(element, 0)?,
                    u16_at(element, 2)?,
                    u16_at(element, 4)?,
                    u16_at(element, 6)?,
                ])
            })
            .take(MAX_GUIDES)
            .collect();
    }

    /// The value a guide parameter names: the coordinate space, an adjust value, an earlier
    /// guide, or the space's centre and size.
    fn reference(&self, reference: u16, values: &[f64]) -> f64 {
        let [left, top, right, bottom] = self.bounds.map(|bound| {
            #[expect(clippy::cast_precision_loss, reason = "shape coordinates")]
            let bound = bound as f64;
            bound
        });
        match reference {
            0x0140 => left,
            0x0141 => top,
            0x0142 => right,
            0x0143 => bottom,
            0x0147..=0x0150 => {
                #[expect(clippy::cast_precision_loss, reason = "adjust values")]
                let adjust = self.adjust[usize::from(reference - 0x0147)] as f64;
                adjust
            }
            0x0400..=0x047F => values
                .get(usize::from(reference - 0x0400))
                .copied()
                .unwrap_or(0.0),
            0x04F8 => f64::midpoint(left, right),
            0x04F9 => f64::midpoint(top, bottom),
            0x04FA => right - left,
            0x04FB => bottom - top,
            _ => 0.0,
        }
    }

    /// The value of every guide, each formula reading the ones before it.
    fn guide_values(&self) -> Vec<f64> {
        let mut values = Vec::with_capacity(self.guides.len());
        for [flags, first, second, third] in &self.guides {
            let parameter = |value: u16, calculated: bool, values: &[f64]| {
                if calculated {
                    self.reference(value, values)
                } else {
                    f64::from(i16::from_le_bytes(value.to_le_bytes()))
                }
            };
            let x = parameter(*first, flags & 0x2000 != 0, &values);
            let y = parameter(*second, flags & 0x4000 != 0, &values);
            let z = parameter(*third, flags & 0x8000 != 0, &values);
            let degrees = |angle: f64| angle / ANGLE_UNIT * PI / 180.0;
            let value = match flags & 0x1FFF {
                0 => x + y - z,
                1 if z != 0.0 => x * y / z,
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
                8 => y.atan2(x) * 180.0 / PI * ANGLE_UNIT,
                9 => x * degrees(y).sin(),
                10 => x * degrees(y).cos(),
                11 => x * z.atan2(y).cos(),
                12 => x * z.atan2(y).sin(),
                13 => x.max(0.0).sqrt(),
                14 => x + y * ANGLE_UNIT - z * ANGLE_UNIT,
                15 if y != 0.0 => z * (1.0 - (x / y) * (x / y)).max(0.0).sqrt(),
                16 => x * degrees(y).tan(),
                _ => 0.0,
            };
            values.push(if value.is_finite() { value } else { 0.0 });
        }
        values
    }

    /// The paths in a frame of this size; `None` when nothing is drawn.
    pub(super) fn paths(&self, width: i64, height: i64) -> Option<Vec<ShapePath>> {
        if self.vertices.is_empty() {
            return None;
        }
        let values = self.guide_values();
        let value = |coordinate: Coordinate| match coordinate {
            #[expect(clippy::cast_precision_loss, reason = "shape coordinates")]
            Coordinate::Number(number) => number as f64,
            Coordinate::Guide(index) => values.get(index).copied().unwrap_or(0.0),
        };
        #[expect(clippy::cast_precision_loss, reason = "shape coordinates")]
        let (mut left, mut top, mut space_width, mut space_height, frame_width, frame_height) = {
            let [left, top, right, bottom] = self.bounds;
            (
                left as f64,
                top as f64,
                (right - left) as f64,
                (bottom - top) as f64,
                width as f64,
                height as f64,
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
        let at = |(x, y): (Coordinate, Coordinate)| {
            (
                (value(x) - left) * frame_width / space_width,
                (value(y) - top) * frame_height / space_height,
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
                4 => paths.extend(std::mem::take(&mut builder).finish(true, true)),
                _ => {
                    for _ in 0..usize::from(segment & 0x00FF) {
                        vertices.next();
                    }
                }
            }
        }
        paths.extend(builder.finish(true, true));
        (!paths.is_empty()).then_some(paths)
    }
}

#[cfg(test)]
mod tests;
