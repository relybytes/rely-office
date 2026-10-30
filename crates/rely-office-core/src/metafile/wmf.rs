// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! The Windows Metafile ([MS-WMF]): an optional placeable header with the picture's box and
//! resolution, a header, then records of a size in 16-bit words and a function, their
//! coordinates 16-bit and mostly given y first. The picture is mapped from its window onto a
//! device of the window's size.

use encoding_rs::WINDOWS_1252;

use super::MAX_RECORDS;
use super::bitmap::{i16_at, packed_dib_picture, u16_at, u32_at};
use super::context::{ArcKind, Brush, Context, Font, Object, Pen, colour_of};

/// The key of a placeable header.
const PLACEABLE_KEY: u32 = 0x9AC6_CDD7;
const PLACEABLE_SIZE: usize = 22;
/// A WMF of no placeable header is taken at 96 pixels to the inch, or in twips when its
/// window is larger than a screen would be.
const LARGE_WINDOW: f64 = 3_000.0;

/// Whether the bytes open with the header of a WMF that has no placeable header.
pub(super) fn is_unplaceable_wmf(bytes: &[u8]) -> bool {
    matches!(u16_at(bytes, 0), Some(1 | 2))
        && u16_at(bytes, 2) == Some(9)
        && matches!(u16_at(bytes, 4), Some(0x0100 | 0x0300))
}

/// A WMF as SVG; `None` when its header is not one, or it draws nothing.
pub(super) fn wmf_svg(bytes: &[u8]) -> Option<String> {
    let placeable = u32_at(bytes, 0) == Some(PLACEABLE_KEY);
    let start = if placeable { PLACEABLE_SIZE } else { 0 };
    if !is_unplaceable_wmf(bytes.get(start..)?) {
        return None;
    }
    let header_words = usize::from(u16_at(bytes, start + 2)?);
    let mut context = Context::new((96.0 / 25.4, 96.0 / 25.4));
    let mut size_points = None;
    if placeable {
        let side = |at: usize| i16_at(bytes, at).map_or(0.0, f64::from);
        let (left, top, right, bottom) = (side(6), side(8), side(10), side(12));
        let inch = f64::from(u16_at(bytes, 14)?).max(1.0);
        set_window(
            &mut context,
            Some((left, top)),
            Some((right - left, bottom - top)),
        );
        size_points = Some((
            (right - left).abs() / inch * 72.0,
            (bottom - top).abs() / inch * 72.0,
        ));
    }
    let mut player = Player { context };
    player.play(bytes, start + header_words * 2);
    let mut context = player.context;
    let (width, height) = (
        context.state.viewport_extent.0.abs(),
        context.state.viewport_extent.1.abs(),
    );
    let size = size_points.unwrap_or_else(|| {
        let per_unit = if width.max(height) > LARGE_WINDOW {
            72.0 / 1_440.0
        } else {
            0.75
        };
        (width * per_unit, height * per_unit)
    });
    let canvas = std::mem::take(&mut context.canvas);
    canvas.finish([0.0, 0.0, width, height], size)
}

/// Map the window, as far as it is given, onto a device of its own size.
fn set_window(context: &mut Context, origin: Option<(f64, f64)>, extent: Option<(f64, f64)>) {
    let state = &mut context.state;
    if let Some(origin) = origin {
        state.window_origin = origin;
    }
    if let Some(extent) = extent {
        let extent = (
            if extent.0 == 0.0 { 1.0 } else { extent.0 },
            if extent.1 == 0.0 { 1.0 } else { extent.1 },
        );
        state.window_extent = extent;
        state.viewport_extent = (extent.0.abs(), extent.1.abs());
    }
    state.viewport_origin = (0.0, 0.0);
    state.map_mode = 8;
}

struct Player {
    context: Context,
}

impl Player {
    fn play(&mut self, bytes: &[u8], mut at: usize) {
        for _ in 0..MAX_RECORDS {
            let (Some(words), Some(function)) = (u32_at(bytes, at), u16_at(bytes, at + 4)) else {
                return;
            };
            let Some(size) = usize::try_from(words)
                .ok()
                .and_then(|words| words.checked_mul(2))
            else {
                return;
            };
            if function == 0 || size < 6 {
                return;
            }
            let Some(record) = bytes.get(at..at.saturating_add(size)) else {
                return;
            };
            self.record(function, &record[6..]);
            at += size;
        }
    }

    #[expect(clippy::too_many_lines, reason = "one arm per record of the format")]
    fn record(&mut self, function: u16, parameters: &[u8]) {
        let word = |index: usize| i16_at(parameters, index * 2).map_or(0.0, f64::from);
        let unsigned = |index: usize| u16_at(parameters, index * 2).unwrap_or(0);
        let colour = |index: usize| colour_of(u32_at(parameters, index * 2).unwrap_or(0));
        let bounds = |first: usize| {
            [
                word(first + 3),
                word(first + 2),
                word(first + 1),
                word(first),
            ]
        };
        let context = &mut self.context;
        match function {
            0x001E => context.save(),
            0x0127 => context.restore(i32::from(i16_at(parameters, 0).unwrap_or(-1))),
            0x0102 => context.state.opaque = unsigned(0) == 2,
            0x0106 => context.state.even_odd = unsigned(0) != 2,
            0x012E => context.state.text_align = u32::from(unsigned(0)),
            0x0201 => context.state.background = colour(0),
            0x0209 => context.state.text_colour = colour(0),
            0x020B => set_window(context, Some((word(1), word(0))), None),
            0x020C => set_window(context, None, Some((word(1), word(0)))),
            0x020F => {
                let (x, y) = context.state.window_origin;
                set_window(context, Some((x + word(1), y + word(0))), None);
            }
            0x012D => context.select_object(usize::from(unsigned(0))),
            0x01F0 => context.delete_object(usize::from(unsigned(0))),
            0x02FA => {
                let style = u32::from(unsigned(0));
                let width = word(1);
                context.add_object(Object::Pen(Pen {
                    style,
                    width,
                    cosmetic: width <= 1.0,
                    colour: colour(3),
                }));
            }
            0x02FC => {
                context.add_object(Object::Brush(Brush {
                    hollow: unsigned(0) == 1,
                    colour: colour(1),
                }));
            }
            0x0142 | 0x01F9 => context.add_object(Object::Brush(Brush {
                hollow: false,
                colour: "C0C0C0".to_owned(),
            })),
            0x00F7 | 0x06FF => context.add_object(Object::Other),
            0x02FB => {
                let name = parameters.get(18..50).unwrap_or_default();
                let name = name.split(|byte| *byte == 0).next().unwrap_or_default();
                context.add_object(Object::Font(Font {
                    height: word(0),
                    escapement: word(2),
                    weight: i32::from(i16_at(parameters, 8).unwrap_or(400)),
                    italic: parameters.get(10).is_some_and(|value| *value != 0),
                    underline: parameters.get(11).is_some_and(|value| *value != 0),
                    strike: parameters.get(12).is_some_and(|value| *value != 0),
                    family: WINDOWS_1252.decode(name).0.into_owned(),
                }));
            }
            0x0416 => context.intersect_clip(bounds(0)),
            0x012C => context.clip_to_region(&[], true),
            0x0214 => context.state.position = (word(1), word(0)),
            0x0213 => {
                let to = (word(1), word(0));
                let data = context.figure(&[to], false, true);
                context.draw(&data, false, true);
                context.state.position = to;
            }
            0x0324 | 0x0325 => {
                let count = usize::try_from(i16_at(parameters, 0).unwrap_or(0)).unwrap_or(0);
                let points = points(parameters, 2, count);
                let closed = function == 0x0324;
                let data = context.figure(&points, closed, false);
                context.draw(&data, closed, true);
            }
            0x0538 => {
                let polygons = usize::from(unsigned(0));
                let counts: Vec<usize> = (0..polygons)
                    .map(|index| usize::from(unsigned(1 + index)))
                    .collect();
                let total: usize = counts.iter().sum();
                let all = points(parameters, 2 + polygons * 2, total);
                let mut data = String::new();
                let mut start = 0;
                for count in counts {
                    let end = (start + count).min(all.len());
                    data.push_str(&context.figure(&all[start..end], true, false));
                    start = end;
                }
                context.draw(&data, true, true);
            }
            0x041B => {
                let data = context.rectangle(bounds(0), None);
                context.draw(&data, true, true);
            }
            0x061C => {
                let data = context.rectangle(bounds(2), Some((word(1), word(0))));
                context.draw(&data, true, true);
            }
            0x0418 => {
                let data = context.ellipse(bounds(0));
                context.draw(&data, true, true);
            }
            0x0817 | 0x081A | 0x0830 => {
                let end = (word(1), word(0));
                let start = (word(3), word(2));
                let kind = match function {
                    0x081A => ArcKind::Pie,
                    0x0830 => ArcKind::Chord,
                    _ => ArcKind::Open,
                };
                let data = context.arc(bounds(4), start, end, kind);
                context.draw(&data, kind != ArcKind::Open, true);
            }
            0x061D => {
                let rectangle = [word(5), word(4), word(3), word(2)];
                let colour = match u32_at(parameters, 0).unwrap_or(0) {
                    0x00F0_0021 | 0x005A_0049 if !context.state.brush.hollow => {
                        context.state.brush.colour.clone()
                    }
                    0x0000_0042 => "000000".to_owned(),
                    0x00FF_0062 => "FFFFFF".to_owned(),
                    _ => return,
                };
                context.fill_rectangle(rectangle, &colour);
            }
            0x0521 => {
                let count = usize::try_from(i16_at(parameters, 0).unwrap_or(0)).unwrap_or(0);
                let padded = count + count % 2;
                let text = parameters.get(2..2 + count).unwrap_or_default();
                let y = i16_at(parameters, 2 + padded).map_or(0.0, f64::from);
                let x = i16_at(parameters, 4 + padded).map_or(0.0, f64::from);
                let text = WINDOWS_1252.decode(text).0.into_owned();
                context.text((x, y), &text, None, None);
            }
            0x0A32 => {
                let (y, x) = (word(0), word(1));
                let count = usize::try_from(i16_at(parameters, 4).unwrap_or(0)).unwrap_or(0);
                let options = unsigned(3);
                let mut at = 8;
                let rectangle = if options & 0x06 != 0 {
                    let rectangle = [word(4), word(5), word(6), word(7)];
                    at += 8;
                    Some(rectangle)
                } else {
                    None
                };
                let text = parameters.get(at..at + count).unwrap_or_default();
                let text = WINDOWS_1252.decode(text).0.into_owned();
                let dx_at = at + count + count % 2;
                let widths = (0..count)
                    .map_while(|index| i16_at(parameters, dx_at + index * 2))
                    .map(f64::from)
                    .sum::<f64>();
                if options & 0x02 != 0
                    && let Some([left, top, right, bottom]) = rectangle
                {
                    let background = context.state.background.clone();
                    context.fill_rectangle([left, top, right - left, bottom - top], &background);
                }
                context.text((x, y), &text, (widths > 0.0).then_some(widths), None);
            }
            0x0F43 => {
                let destination = [word(10), word(9), word(8), word(7)];
                if let Some(picture) = parameters.get(22..).and_then(packed_dib_picture) {
                    context.picture(&picture, destination);
                }
            }
            0x0B41 => {
                let destination = [word(9), word(8), word(7), word(6)];
                if let Some(picture) = parameters.get(20..).and_then(packed_dib_picture) {
                    context.picture(&picture, destination);
                }
            }
            0x0940 => {
                let destination = [word(7), word(6), word(5), word(4)];
                if let Some(picture) = parameters.get(16..).and_then(packed_dib_picture) {
                    context.picture(&picture, destination);
                }
            }
            _ => {}
        }
    }
}

/// Points as `(x, y)` pairs of `i16` from `at`.
fn points(parameters: &[u8], at: usize, count: usize) -> Vec<(f64, f64)> {
    let available = parameters.len().saturating_sub(at) / 4;
    (0..count.min(available))
        .filter_map(|index| {
            let offset = at + index * 4;
            Some((
                f64::from(i16_at(parameters, offset)?),
                f64::from(i16_at(parameters, offset + 2)?),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests;
