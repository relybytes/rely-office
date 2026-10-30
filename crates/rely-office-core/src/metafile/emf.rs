// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! The Enhanced Metafile ([MS-EMF]): a header with the picture's bounds and frame, then
//! records of a type and a size, each played on the device context. The EMF+ records some
//! files carry in comments are skipped: those files also hold the plain EMF drawing.

use encoding_rs::WINDOWS_1252;

use super::MAX_RECORDS;
use super::bitmap::{Decoded, decode_dib, i32_at, masked, u16_at, u32_at};
use super::context::{ArcKind, Brush, Context, Font, Object, Pen, Transform, colour_of};

/// The EMF signature, ` EMF`, at offset 40 of the header.
const SIGNATURE: u32 = 0x464D_4520;

/// An EMF as SVG; `None` when its header is not one, or it draws nothing.
pub(super) fn emf_svg(bytes: &[u8]) -> Option<String> {
    if u32_at(bytes, 0)? != 1 || u32_at(bytes, 40)? != SIGNATURE {
        return None;
    }
    let bounds = [
        i32_at(bytes, 8)?,
        i32_at(bytes, 12)?,
        i32_at(bytes, 16)?,
        i32_at(bytes, 20)?,
    ]
    .map(f64::from);
    let frame = [
        i32_at(bytes, 24)?,
        i32_at(bytes, 28)?,
        i32_at(bytes, 32)?,
        i32_at(bytes, 36)?,
    ]
    .map(f64::from);
    let device = (f64::from(i32_at(bytes, 72)?), f64::from(i32_at(bytes, 76)?));
    let millimetres = (f64::from(i32_at(bytes, 80)?), f64::from(i32_at(bytes, 84)?));
    let pixels_per_mm = (
        if millimetres.0 > 0.0 {
            device.0 / millimetres.0
        } else {
            96.0 / 25.4
        },
        if millimetres.1 > 0.0 {
            device.1 / millimetres.1
        } else {
            96.0 / 25.4
        },
    );
    let mut context = Context::new(pixels_per_mm);
    play(bytes, &mut context);
    // The frame is in hundredths of a millimetre; the bounds in device pixels, inclusive.
    let size = (
        (frame[2] - frame[0]) / 100.0 * 72.0 / 25.4,
        (frame[3] - frame[1]) / 100.0 * 72.0 / 25.4,
    );
    let view_box = if bounds[2] >= bounds[0] && bounds[3] >= bounds[1] {
        [
            bounds[0],
            bounds[1],
            bounds[2] - bounds[0] + 1.0,
            bounds[3] - bounds[1] + 1.0,
        ]
    } else {
        [
            frame[0] / 100.0 * pixels_per_mm.0,
            frame[1] / 100.0 * pixels_per_mm.1,
            (frame[2] - frame[0]) / 100.0 * pixels_per_mm.0,
            (frame[3] - frame[1]) / 100.0 * pixels_per_mm.1,
        ]
    };
    let size = if size.0 > 0.0 && size.1 > 0.0 {
        size
    } else {
        (view_box[2] * 0.75, view_box[3] * 0.75)
    };
    context.canvas.finish(view_box, size)
}

fn play(bytes: &[u8], context: &mut Context) {
    let mut at = 0;
    for _ in 0..MAX_RECORDS {
        let (Some(kind), Some(size)) = (u32_at(bytes, at), u32_at(bytes, at + 4)) else {
            return;
        };
        let Ok(size) = usize::try_from(size) else {
            return;
        };
        if size < 8 || size % 4 != 0 {
            return;
        }
        let Some(record) = bytes.get(at..at.saturating_add(size)) else {
            return;
        };
        if kind == 14 {
            return;
        }
        record_of(kind, record, context);
        at += size;
    }
}

/// A point of a record: two `i32`, or two `i16` for the `16` records.
fn points(record: &[u8], at: usize, count: usize, short: bool) -> Vec<(f64, f64)> {
    let step = if short { 4 } else { 8 };
    let available = record.len().saturating_sub(at) / step;
    (0..count.min(available))
        .filter_map(|index| {
            let offset = at + index * step;
            if short {
                Some((
                    f64::from(super::bitmap::i16_at(record, offset)?),
                    f64::from(super::bitmap::i16_at(record, offset + 2)?),
                ))
            } else {
                Some((
                    f64::from(i32_at(record, offset)?),
                    f64::from(i32_at(record, offset + 4)?),
                ))
            }
        })
        .collect()
}

fn count_at(record: &[u8], at: usize) -> usize {
    u32_at(record, at)
        .and_then(|count| usize::try_from(count).ok())
        .unwrap_or(0)
}

fn rect_at(record: &[u8], at: usize) -> Option<[f64; 4]> {
    Some([
        f64::from(i32_at(record, at)?),
        f64::from(i32_at(record, at + 4)?),
        f64::from(i32_at(record, at + 8)?),
        f64::from(i32_at(record, at + 12)?),
    ])
}

fn float_at(record: &[u8], at: usize) -> Option<f64> {
    let value = f64::from(f32::from_le_bytes(record.get(at..at + 4)?.try_into().ok()?));
    value.is_finite().then_some(value)
}

fn transform_at(record: &[u8], at: usize) -> Option<Transform> {
    Some(Transform {
        m11: float_at(record, at)?,
        m12: float_at(record, at + 4)?,
        m21: float_at(record, at + 8)?,
        m22: float_at(record, at + 12)?,
        dx: float_at(record, at + 16)?,
        dy: float_at(record, at + 20)?,
    })
}

#[expect(clippy::too_many_lines, reason = "one arm per record of the format")]
fn record_of(kind: u32, record: &[u8], context: &mut Context) {
    let int = |at: usize| i32_at(record, at).map_or(0.0, f64::from);
    match kind {
        // Polygons, polylines and Beziers, with 32-bit and 16-bit points.
        2 | 85 | 3 | 86 | 4 | 87 | 5 | 88 | 6 | 89 => {
            let short = kind >= 85;
            let points = points(record, 28, count_at(record, 24), short);
            match kind {
                2 | 85 => {
                    let data = context.curves(&points, false);
                    context.draw(&data, false, true);
                }
                3 | 86 => {
                    let data = context.figure(&points, true, false);
                    context.draw(&data, true, true);
                }
                4 | 87 => {
                    let data = context.figure(&points, false, false);
                    context.draw(&data, false, true);
                }
                5 | 88 => {
                    let data = context.curves(&points, true);
                    context.draw(&data, false, true);
                }
                _ => {
                    let data = context.figure(&points, false, true);
                    context.draw(&data, false, true);
                }
            }
            if let Some(last) = points.last()
                && matches!(kind, 5 | 88 | 6 | 89)
            {
                context.state.position = *last;
            }
        }
        // PolyPolyline and PolyPolygon.
        7 | 90 | 8 | 91 => {
            let short = kind >= 90;
            let polygons = count_at(record, 24).min(super::context::MAX_POINTS);
            let total = count_at(record, 28);
            let counts: Vec<usize> = (0..polygons)
                .map(|index| count_at(record, 32 + index * 4))
                .collect();
            let all = points(record, 32 + polygons * 4, total, short);
            let closed = matches!(kind, 8 | 91);
            let mut data = String::new();
            let mut start = 0;
            for count in counts {
                let end = (start + count).min(all.len());
                data.push_str(&context.figure(&all[start..end], closed, false));
                start = end;
            }
            context.draw(&data, closed, true);
        }
        9 => context.state.window_extent = (int(8), int(12)),
        10 => context.state.window_origin = (int(8), int(12)),
        11 => context.state.viewport_extent = (int(8), int(12)),
        12 => context.state.viewport_origin = (int(8), int(12)),
        31 | 32 => {
            let ratio = |numerator: f64, denominator: f64| {
                if denominator == 0.0 {
                    1.0
                } else {
                    numerator / denominator
                }
            };
            let (x, y) = (ratio(int(8), int(12)), ratio(int(16), int(20)));
            let extent = if kind == 31 {
                &mut context.state.viewport_extent
            } else {
                &mut context.state.window_extent
            };
            *extent = (extent.0 * x, extent.1 * y);
        }
        17 => context.state.map_mode = u32_at(record, 8).unwrap_or(1),
        18 => context.state.opaque = u32_at(record, 8) == Some(2),
        19 => context.state.even_odd = u32_at(record, 8) != Some(2),
        22 => context.state.text_align = u32_at(record, 8).unwrap_or(0),
        24 => context.state.text_colour = colour_of(u32_at(record, 8).unwrap_or(0)),
        25 => context.state.background = colour_of(u32_at(record, 8).unwrap_or(0xFF_FFFF)),
        27 => context.state.position = (int(8), int(12)),
        33 => context.save(),
        34 => context.restore(i32_at(record, 8).unwrap_or(-1)),
        35 => {
            if let Some(transform) = transform_at(record, 8) {
                context.state.world = transform;
            }
        }
        36 => {
            if let Some(transform) = transform_at(record, 8) {
                let world = context.state.world;
                context.state.world = match u32_at(record, 32) {
                    Some(1) => Transform::IDENTITY,
                    Some(2) => transform.then(&world),
                    Some(3) => world.then(&transform),
                    Some(4) => transform,
                    _ => world,
                };
            }
        }
        37 => {
            let index = u32_at(record, 8).unwrap_or(0);
            if index & 0x8000_0000 != 0 {
                context.select_stock_object(index & 0x7FFF_FFFF);
            } else if let Ok(index) = usize::try_from(index) {
                context.select_object(index);
            }
        }
        38 => {
            let style = u32_at(record, 12).unwrap_or(0);
            let width = int(16);
            let pen = Pen {
                style,
                width,
                cosmetic: width <= 1.0,
                colour: colour_of(u32_at(record, 24).unwrap_or(0)),
            };
            set(context, record, Object::Pen(pen));
        }
        95 => {
            let style = u32_at(record, 28).unwrap_or(0);
            let width = u32_at(record, 32).map_or(0.0, f64::from);
            let pen = Pen {
                style,
                width,
                cosmetic: style & 0x0001_0000 == 0,
                colour: colour_of(u32_at(record, 40).unwrap_or(0)),
            };
            set(context, record, Object::Pen(pen));
        }
        39 => {
            let style = u32_at(record, 12).unwrap_or(0);
            let brush = Brush {
                hollow: style == 1,
                colour: colour_of(u32_at(record, 16).unwrap_or(0)),
            };
            set(context, record, Object::Brush(brush));
        }
        93 | 94 => {
            let brush = Brush {
                hollow: false,
                colour: "C0C0C0".to_owned(),
            };
            set(context, record, Object::Brush(brush));
        }
        49 => set(context, record, Object::Other),
        40 => {
            if let Some(index) = u32_at(record, 8).and_then(|index| usize::try_from(index).ok()) {
                context.delete_object(index);
            }
        }
        82 => {
            let text = |at: usize, length: usize| {
                let units: Vec<u16> = (0..length)
                    .map_while(|index| u16_at(record, at + index * 2))
                    .take_while(|unit| *unit != 0)
                    .collect();
                String::from_utf16_lossy(&units)
            };
            let font = Font {
                height: int(12),
                escapement: int(20),
                weight: i32_at(record, 28).unwrap_or(400),
                italic: record.get(32).is_some_and(|value| *value != 0),
                underline: record.get(33).is_some_and(|value| *value != 0),
                strike: record.get(34).is_some_and(|value| *value != 0),
                family: text(40, 32),
            };
            set(context, record, Object::Font(font));
        }
        42 => {
            if let Some(bounds) = rect_at(record, 8) {
                let data = context.ellipse(bounds);
                context.draw(&data, true, true);
            }
        }
        43 => {
            if let Some(bounds) = rect_at(record, 8) {
                let data = context.rectangle(bounds, None);
                context.draw(&data, true, true);
            }
        }
        44 => {
            if let Some(bounds) = rect_at(record, 8) {
                let data = context.rectangle(bounds, Some((int(24), int(28))));
                context.draw(&data, true, true);
            }
        }
        45 | 46 | 47 | 55 => {
            if let Some(bounds) = rect_at(record, 8) {
                let start = (int(24), int(28));
                let end = (int(32), int(36));
                let arc_kind = match kind {
                    46 => ArcKind::Chord,
                    47 => ArcKind::Pie,
                    _ => ArcKind::Open,
                };
                let data = context.arc(bounds, start, end, arc_kind);
                context.draw(&data, arc_kind != ArcKind::Open, true);
            }
        }
        54 => {
            let to = (int(8), int(12));
            let data = context.figure(&[to], false, true);
            context.draw(&data, false, true);
            context.state.position = to;
        }
        57 => context.state.clockwise = u32_at(record, 8) == Some(2),
        59 => context.path = Some(String::new()),
        61 => {
            if let Some(path) = &mut context.path {
                path.push('Z');
            }
        }
        62 => context.draw_path(true, false),
        63 => context.draw_path(true, true),
        64 => context.draw_path(false, true),
        30 => {
            if let Some(bounds) = rect_at(record, 8) {
                context.intersect_clip(bounds);
            }
        }
        67 => context.clip_to_path(u32_at(record, 8) == Some(5)),
        68 => context.path = None,
        75 => {
            let mode = u32_at(record, 12).unwrap_or(5);
            let size = count_at(record, 8);
            let rectangles: Vec<[f64; 4]> = if size >= 32 {
                let count = count_at(record, 24).min(size.saturating_sub(32) / 16);
                (0..count)
                    .filter_map(|index| rect_at(record, 48 + index * 16))
                    .collect()
            } else {
                Vec::new()
            };
            match mode {
                1 => context.clip_to_region(&rectangles, false),
                5 => context.clip_to_region(&rectangles, true),
                _ => {}
            }
        }
        76 | 77 | 114 => blit(kind, record, context),
        81 => {
            let destination = [int(24), int(28), int(72), int(76)];
            let operation = u32_at(record, 68).unwrap_or(0);
            draw_dib(record, (48, 52, 56, 60), destination, operation, context);
        }
        80 => {
            let destination = [int(24), int(28), int(40), int(44)];
            draw_dib(record, (48, 52, 56, 60), destination, 0x00CC_0020, context);
        }
        83 | 84 => text_out(kind == 84, record, context),
        _ => {}
    }
}

/// Put an object in the slot the record's first field names.
fn set(context: &mut Context, record: &[u8], object: Object) {
    if let Some(index) = u32_at(record, 8).and_then(|index| usize::try_from(index).ok()) {
        context.set_object(index, object);
    }
}

/// `BitBlt`, `StretchBlt` and `AlphaBlend`: a bitmap when the record holds one, else the
/// rectangle painted by the raster operation.
fn blit(kind: u32, record: &[u8], context: &mut Context) {
    let int = |at: usize| i32_at(record, at).map_or(0.0, f64::from);
    let destination = [int(24), int(28), int(32), int(36)];
    if u32_at(record, 88).unwrap_or(0) > 0 {
        let operation = if kind == 114 {
            0x00CC_0020
        } else {
            u32_at(record, 40).unwrap_or(0)
        };
        draw_dib(record, (84, 88, 92, 96), destination, operation, context);
        return;
    }
    if kind == 114 {
        return;
    }
    let colour = match u32_at(record, 40).unwrap_or(0) {
        0x00F0_0021 | 0x005A_0049 => {
            if context.state.brush.hollow {
                return;
            }
            context.state.brush.colour.clone()
        }
        0x0000_0042 => "000000".to_owned(),
        0x00FF_0062 => "FFFFFF".to_owned(),
        _ => return,
    };
    context.fill_rectangle(destination, &colour);
}

/// The raster operations of a transparent bitmap: the mask `OR`ed in, then the image `AND`ed.
const SOURCE_PAINT: u32 = 0x00EE_0086;
const SOURCE_AND: u32 = 0x0088_00C6;

/// The bitmap a record carries, from the offsets of its `BITMAPINFO` and its bits; a mask and
/// the image it makes transparent are drawn as one picture.
fn draw_dib(
    record: &[u8],
    (info_offset, info_size, bits_offset, bits_size): (usize, usize, usize, usize),
    destination: [f64; 4],
    operation: u32,
    context: &mut Context,
) {
    let slice = |offset: usize, size: usize| {
        let start = usize::try_from(u32_at(record, offset)?).ok()?;
        let length = usize::try_from(u32_at(record, size)?).ok()?;
        record.get(start..start.checked_add(length)?)
    };
    let (Some(info), Some(bits)) = (slice(info_offset, info_size), slice(bits_offset, bits_size))
    else {
        return;
    };
    let Some(decoded) = decode_dib(info, bits) else {
        return;
    };
    let mask = context.pending_mask.take();
    let picture = match decoded {
        Decoded::Pixels {
            width,
            height,
            rgba,
        } if operation == SOURCE_PAINT => {
            context.pending_mask = Some((destination, width, height, rgba));
            return;
        }
        Decoded::Pixels {
            width,
            height,
            rgba,
        } if operation == SOURCE_AND => match mask {
            Some((at, mask_width, mask_height, mask)) if same_place(at, destination) => {
                masked((width, height, &rgba), (mask_width, mask_height, &mask))
            }
            _ => Decoded::Pixels {
                width,
                height,
                rgba,
            }
            .into_picture(),
        },
        other => other.into_picture(),
    };
    if let Some(picture) = picture {
        context.picture(&picture, destination);
    }
}

/// Whether two rectangles are the same, to a device unit.
fn same_place(first: [f64; 4], second: [f64; 4]) -> bool {
    first
        .iter()
        .zip(second)
        .all(|(first, second)| (first - second).abs() < 1.0)
}

/// `ExtTextOutW` and `ExtTextOutA`: the string at its reference point, its characters spaced
/// as the record's advances say, over its rectangle when the background is opaque.
fn text_out(wide: bool, record: &[u8], context: &mut Context) {
    let int = |at: usize| i32_at(record, at).map_or(0.0, f64::from);
    let reference = (int(36), int(40));
    let count = count_at(record, 44).min(8_192);
    let Some(offset) = u32_at(record, 48).and_then(|offset| usize::try_from(offset).ok()) else {
        return;
    };
    let options = u32_at(record, 52).unwrap_or(0);
    let has_rectangle = options & 0x100 == 0;
    let rectangle = if has_rectangle {
        rect_at(record, 56)
    } else {
        None
    };
    let dx_at = if has_rectangle { 72 } else { 56 };
    let text = if wide {
        let units: Vec<u16> = (0..count)
            .map_while(|index| u16_at(record, offset + index * 2))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        let bytes = record
            .get(offset..offset.saturating_add(count))
            .unwrap_or_default();
        WINDOWS_1252.decode(bytes).0.into_owned()
    };
    let widths = u32_at(record, dx_at)
        .and_then(|offset| usize::try_from(offset).ok())
        .filter(|offset| *offset > 0)
        .map(|offset| {
            (0..count)
                .map_while(|index| i32_at(record, offset + index * 4))
                .map(f64::from)
                .sum::<f64>()
        })
        .filter(|width| *width > 0.0);
    // ETO_OPAQUE paints the rectangle in the background colour.
    let opaque_box = rectangle.filter(|_| options & 0x02 != 0);
    if options & 0x02 != 0
        && let Some([left, top, right, bottom]) = opaque_box
    {
        let colour = context.state.background.clone();
        context.fill_rectangle([left, top, right - left, bottom - top], &colour);
    }
    context.text(reference, &text, widths, None);
}

#[cfg(test)]
mod tests;
