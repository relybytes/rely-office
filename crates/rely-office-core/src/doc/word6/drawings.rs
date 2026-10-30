// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! The drawing layer of Word 6 and Word 95: each drawing object (`DO`) the main text anchors,
//! its primitives (lines, rectangles, ellipses, arcs, polylines, text boxes, callouts and the
//! groups that hold them) read from the `WordDocument` stream. The shapes of an object become
//! one picture over their bounding box; its text boxes keep their text, from the text box story.

use std::collections::HashMap;
use std::ops::Range;

use super::super::fkp::positions;
use super::super::pictures::ShapeAnchor;
use super::Word6Fib;
use crate::compound::{u16_at, u32_at};
use crate::geometry::{
    DrawnShape, FillShade, PathCommand, PathPoint, ShapePath, preset_geometry, rectangle_paths,
    shapes_svg,
};

/// The plex of the drawing objects of the main text, its anchor positions and their `FDOA`s.
const DRAWN_OBJECTS: usize = 38;
const TEXT_BOXES: usize = 56;
const FDOA_SIZE: usize = 6;
const DO_HEADER: usize = 10;
const PRIMITIVE_HEADER: usize = 12;
const LINE_TYPE: usize = 8;
const FILL: usize = 10;
const SHADOW: usize = 6;
const LINE_END: usize = 4;
/// No object holds more primitives, nor groups nest deeper, than this.
const MAX_PRIMITIVES: usize = 1_000;
const MAX_DEPTH: usize = 8;
const EMU_PER_TWIP: i64 = 635;
/// How far along the tangent the control points of a quarter ellipse stand.
const KAPPA: f64 = 0.552_284_75;
/// The shares of the foreground colour each fill pattern mixes into the background.
const PATTERN_SHARES: [u32; 32] = [
    0, 0, 5, 10, 20, 25, 30, 40, 50, 60, 70, 75, 80, 90, 50, 50, 50, 50, 50, 50, 60, 60, 60, 60,
    50, 50, 50, 50, 50, 50, 50, 50,
];

/// A drawing object of the main text, as the tree shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in super::super) struct Word6Drawing {
    /// Where the shapes' picture stands, its bounds in twips from the object's bases.
    pub(in super::super) anchor: ShapeAnchor,
    /// The shapes, an SVG picture over the anchor's bounds.
    pub(in super::super) picture: Option<Vec<u8>>,
    pub(in super::super) text_boxes: Vec<Word6TextBox>,
}

/// A text box of a drawing object: its bounds from the object's bases, its text, fill, line and
/// whether its corners are round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in super::super) struct Word6TextBox {
    pub(in super::super) bounds: [i32; 4],
    pub(in super::super) text: Range<usize>,
    pub(in super::super) fill: Option<String>,
    /// `RRGGBB` and the width in eighths of a point.
    pub(in super::super) line: Option<(String, u32)>,
    pub(in super::super) rounded: bool,
}

/// A shape read from a primitive, in twips from the object's bases.
struct Item {
    left: i64,
    top: i64,
    width: i64,
    height: i64,
    /// In the item's own frame, in English Metric Units.
    paths: Vec<ShapePath>,
    fill: Option<String>,
    line: Option<(String, i64)>,
}

/// How a primitive is lined and filled.
struct Look {
    line: Option<(String, i64)>,
    fill: Option<String>,
}

/// The reader of one object's primitives.
struct Reader<'a> {
    stream: &'a [u8],
    items: Vec<Item>,
    text_boxes: Vec<Word6TextBox>,
    /// The text box story's positions, and the next text box to take.
    box_positions: &'a [u32],
    box_story: usize,
    next_box: &'a mut usize,
    read: usize,
}

/// The drawing objects of the main text, by the position of the character that anchors each.
pub(in super::super) fn drawings(
    word_document: &[u8],
    fib: &Word6Fib,
) -> HashMap<usize, Word6Drawing> {
    let plex = fib.structure(word_document, DRAWN_OBJECTS);
    let count = plex.len().saturating_sub(4) / (4 + FDOA_SIZE);
    let box_positions = positions(fib.structure(word_document, TEXT_BOXES));
    let mut next_box = 0;
    let mut found = HashMap::new();
    for index in 0..count {
        let (Some(position), Some(offset)) = (
            u32_at(plex, index * 4),
            u32_at(plex, (count + 1) * 4 + index * FDOA_SIZE),
        ) else {
            break;
        };
        let mut reader = Reader {
            stream: word_document,
            items: Vec::new(),
            text_boxes: Vec::new(),
            box_positions: &box_positions,
            box_story: fib.stories.text_boxes_start(),
            next_box: &mut next_box,
            read: 0,
        };
        let Some(header) = word_document.get(offset as usize..) else {
            continue;
        };
        let (Some(size), Some(&horizontal), Some(&vertical)) =
            (u16_at(header, 2), header.get(4), header.get(5))
        else {
            continue;
        };
        let end = (offset as usize).saturating_add(usize::from(size));
        reader.primitives(offset as usize + DO_HEADER, end, (0, 0), 0, usize::MAX);
        let Reader {
            items, text_boxes, ..
        } = reader;
        let drawing = drawing_of(items, text_boxes, horizontal, vertical);
        found.insert(position as usize, drawing);
    }
    found
}

/// The picture of an object's shapes over their bounding box, and its text boxes.
fn drawing_of(
    items: Vec<Item>,
    text_boxes: Vec<Word6TextBox>,
    horizontal: u8,
    vertical: u8,
) -> Word6Drawing {
    let bounds = items.iter().fold(None, |bounds: Option<[i64; 4]>, item| {
        let edges = [
            item.left,
            item.top,
            item.left + item.width,
            item.top + item.height,
        ];
        Some(bounds.map_or(edges, |known| {
            [
                known[0].min(edges[0]),
                known[1].min(edges[1]),
                known[2].max(edges[2]),
                known[3].max(edges[3]),
            ]
        }))
    });
    let (picture, bounds) = match bounds {
        Some([left, top, right, bottom]) => {
            let (width, height) = ((right - left).max(1), (bottom - top).max(1));
            let shapes: Vec<DrawnShape> = items
                .into_iter()
                .map(|item| DrawnShape {
                    x: (item.left - left) * EMU_PER_TWIP,
                    y: (item.top - top) * EMU_PER_TWIP,
                    width: item.width.max(1) * EMU_PER_TWIP,
                    height: item.height.max(1) * EMU_PER_TWIP,
                    rotation: 0.0,
                    paths: item.paths,
                    fill: item.fill,
                    line: item.line,
                    text: Vec::new(),
                })
                .collect();
            (
                shapes_svg(&shapes, width * EMU_PER_TWIP, height * EMU_PER_TWIP),
                [left, top, left + width, top + height],
            )
        }
        None => (None, [0; 4]),
    };
    Word6Drawing {
        anchor: ShapeAnchor {
            shape: 0,
            bounds: bounds.map(clamp),
            horizontal_base: horizontal.min(2),
            vertical_base: vertical.min(2),
            wrap: 3,
            behind_text: false,
        },
        picture,
        text_boxes,
    }
}

fn clamp(value: i64) -> i32 {
    i32::try_from(value.clamp(i64::from(i32::MIN), i64::from(i32::MAX))).unwrap_or(0)
}

fn signed(bytes: &[u8], offset: usize) -> i64 {
    i64::from(u16_at(bytes, offset).map_or(0, |value| i16::from_le_bytes(value.to_le_bytes())))
}

/// A Word 6 colour: red, green and blue in its three low bytes, or a grey when its top byte's
/// low bit is set, the red byte then the share of black in half percents.
fn colour(value: u32) -> String {
    let [red, green, blue, flags] = value.to_le_bytes();
    if flags & 1 != 0 {
        let grey = (200_u32.saturating_sub(u32::from(red)) * 256 / 200).min(255);
        return format!("{grey:02X}{grey:02X}{grey:02X}");
    }
    format!("{red:02X}{green:02X}{blue:02X}")
}

/// A colour blended `share` percent of the way from `background` to `foreground`.
fn blend(foreground: u32, background: u32, share: u32) -> String {
    let parse = |value: &str| u32::from_str_radix(value, 16).unwrap_or(0);
    let (front, back) = (colour(foreground), colour(background));
    let channel = |index: usize| {
        let front = parse(&front[index * 2..index * 2 + 2]);
        let back = parse(&back[index * 2..index * 2 + 2]);
        (front * share + back * (100 - share)) / 100
    };
    format!("{:02X}{:02X}{:02X}", channel(0), channel(1), channel(2))
}

/// The line of a `DPLINETYPE`: its colour, width in twips and style, 5 being none.
fn line_of(bytes: &[u8]) -> Option<(String, i64)> {
    let style = u16_at(bytes, 6).unwrap_or(0);
    if style == 5 {
        return None;
    }
    let width = i64::from(u16_at(bytes, 4).unwrap_or(0));
    let width = if width == 0 {
        9_525
    } else {
        width * EMU_PER_TWIP
    };
    Some((colour(u32_at(bytes, 0).unwrap_or(0)), width))
}

/// The fill of a `DPFILL`: none for the hollow pattern, the background for a solid one, and the
/// two colours mixed as the pattern shades for the rest.
fn fill_of(bytes: &[u8]) -> Option<String> {
    let (foreground, background) = (u32_at(bytes, 0).unwrap_or(0), u32_at(bytes, 4).unwrap_or(0));
    match usize::from(u16_at(bytes, 8).unwrap_or(0)) {
        0 => None,
        pattern if pattern == 1 || pattern >= PATTERN_SHARES.len() => Some(colour(background)),
        pattern => Some(blend(foreground, background, PATTERN_SHARES[pattern])),
    }
}

fn point(x: i64, y: i64) -> PathPoint {
    PathPoint {
        x: x * EMU_PER_TWIP,
        y: y * EMU_PER_TWIP,
    }
}

fn path(commands: Vec<PathCommand>, filled: bool, stroked: bool) -> ShapePath {
    ShapePath {
        commands,
        filled,
        stroked,
        shade: FillShade::Normal,
    }
}

impl Reader<'_> {
    /// The primitives between `start` and `end`, at most `count`, their positions moved by
    /// `origin`.
    fn primitives(
        &mut self,
        start: usize,
        end: usize,
        origin: (i64, i64),
        depth: usize,
        count: usize,
    ) {
        let mut position = start;
        let mut taken = 0;
        while taken < count && position + PRIMITIVE_HEADER <= end && self.read < MAX_PRIMITIVES {
            let Some(header) = self.stream.get(position..end) else {
                break;
            };
            let size = usize::from(u16_at(header, 2).unwrap_or(0));
            if size < PRIMITIVE_HEADER || size > header.len() {
                break;
            }
            self.read += 1;
            self.primitive(position, size, origin, depth);
            position += size;
            taken += 1;
        }
    }

    /// One primitive of `size` bytes at `at`: its kind in the low byte of `dpk`, its frame from
    /// `xa`, `ya`, `dxa` and `dya`, and the record of its kind after the header.
    fn primitive(&mut self, at: usize, size: usize, origin: (i64, i64), depth: usize) {
        let stream = self.stream;
        let record = stream.get(at..at + size).unwrap_or_default();
        let kind = u16_at(record, 0).unwrap_or(0) & 0xFF;
        let left = origin.0 + signed(record, 4);
        let top = origin.1 + signed(record, 6);
        let width = signed(record, 8);
        let height = signed(record, 10);
        let body = record.get(PRIMITIVE_HEADER..).unwrap_or_default();
        match kind {
            0 if depth < MAX_DEPTH => {
                let count = usize::from(u16_at(body, 0).unwrap_or(0));
                self.primitives(
                    at + PRIMITIVE_HEADER + 2,
                    at + size,
                    (left, top),
                    depth + 1,
                    count,
                );
            }
            1 => self.line(body, left, top),
            2 => self.text_box(body, [left, top, left + width, top + height]),
            3 | 4 => {
                let look = Look {
                    line: line_of(body),
                    fill: body.get(LINE_TYPE..).and_then(fill_of),
                };
                let rounded = u16_at(body, LINE_TYPE + FILL + SHADOW).unwrap_or(0) & 1 != 0;
                let (frame_width, frame_height) =
                    (width.max(1) * EMU_PER_TWIP, height.max(1) * EMU_PER_TWIP);
                let paths = match (kind, rounded) {
                    (4, _) => preset_geometry("ellipse", None, frame_width, frame_height),
                    (_, true) => preset_geometry("roundRect", None, frame_width, frame_height),
                    _ => Some(rectangle_paths(frame_width, frame_height)),
                };
                if let Some(paths) = paths {
                    self.push(left, top, width, height, paths, look);
                }
            }
            5 => self.arc(body, left, top, width, height),
            6 => self.polyline(body, left, top),
            7 => self.callout(body, left, top),
            _ => {}
        }
    }

    fn push(
        &mut self,
        left: i64,
        top: i64,
        width: i64,
        height: i64,
        paths: Vec<ShapePath>,
        look: Look,
    ) {
        self.items.push(Item {
            left,
            top,
            width: width.max(1),
            height: height.max(1),
            paths,
            fill: look.fill,
            line: look.line,
        });
    }

    /// A `DPLINE`: its two ends from the primitive's corner, and its line.
    fn line(&mut self, body: &[u8], left: i64, top: i64) {
        let ends = [0, 2, 4, 6].map(|offset| signed(body, offset));
        let (start, end) = (
            (left + ends[0], top + ends[1]),
            (left + ends[2], top + ends[3]),
        );
        let (x, y) = (start.0.min(end.0), start.1.min(end.1));
        let line = body.get(8..).and_then(line_of);
        let paths = vec![path(
            vec![
                PathCommand::MoveTo(point(start.0 - x, start.1 - y)),
                PathCommand::LineTo(point(end.0 - x, end.1 - y)),
            ],
            false,
            true,
        )];
        self.push(
            x,
            y,
            (start.0 - end.0).abs(),
            (start.1 - end.1).abs(),
            paths,
            Look { line, fill: None },
        );
    }

    /// A `DPARC`: a quarter of an ellipse filling the primitive's frame, the quarter `fLeft` and
    /// `fUp` name; the fill takes the wedge to the centre and the line only the curve.
    fn arc(&mut self, body: &[u8], left: i64, top: i64, width: i64, height: i64) {
        let at = LINE_TYPE + FILL + SHADOW;
        let (facing_left, facing_up) = (
            body.get(at).copied().unwrap_or(0) & 1 != 0,
            body.get(at + 1).copied().unwrap_or(0) & 1 != 0,
        );
        let (w, h) = (width.max(1), height.max(1));
        // The corner the ellipse's centre is at.
        let centre = match (facing_left, facing_up) {
            (true, true) => (0, h),
            (false, true) => (0, 0),
            (false, false) => (w, 0),
            (true, false) => (w, h),
        };
        let across = (if centre.0 == 0 { w } else { 0 }, centre.1);
        let down = (centre.0, if centre.1 == 0 { h } else { 0 });
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_precision_loss,
            reason = "a frame in twips"
        )]
        let lerp = |from: i64, to: i64| from + ((to - from) as f64 * KAPPA).round() as i64;
        let curve = PathCommand::CubicTo(
            point(across.0, lerp(across.1, down.1)),
            point(lerp(down.0, across.0), down.1),
            point(down.0, down.1),
        );
        let look = Look {
            line: line_of(body),
            fill: body.get(LINE_TYPE..).and_then(fill_of),
        };
        let mut paths = Vec::new();
        if look.fill.is_some() {
            paths.push(path(
                vec![
                    PathCommand::MoveTo(point(centre.0, centre.1)),
                    PathCommand::LineTo(point(across.0, across.1)),
                    curve,
                    PathCommand::Close,
                ],
                true,
                false,
            ));
        }
        paths.push(path(
            vec![PathCommand::MoveTo(point(across.0, across.1)), curve],
            false,
            true,
        ));
        self.push(left, top, w, h, paths, look);
    }

    /// A `DPPOLYLINE`: its points from the primitive's corner, closed and filled when it says so;
    /// the low bit of its flags closes it and the rest count the points.
    fn polyline(&mut self, body: &[u8], left: i64, top: i64) {
        let flags_at = LINE_TYPE + FILL + LINE_END + SHADOW;
        let flags = u16_at(body, flags_at).unwrap_or(0);
        let closed = flags & 1 != 0;
        let count = usize::from(flags >> 1).min(MAX_PRIMITIVES);
        let points: Vec<(i64, i64)> = (0..count)
            .map_while(|index| {
                let at = flags_at + 2 + index * 4;
                body.get(at..at + 4)?;
                Some((left + signed(body, at), top + signed(body, at + 2)))
            })
            .collect();
        if points.len() < 2 {
            return;
        }
        let x = points.iter().map(|point| point.0).min().unwrap_or(left);
        let y = points.iter().map(|point| point.1).min().unwrap_or(top);
        let right = points.iter().map(|point| point.0).max().unwrap_or(left);
        let bottom = points.iter().map(|point| point.1).max().unwrap_or(top);
        let mut commands: Vec<PathCommand> = points
            .iter()
            .enumerate()
            .map(|(index, (px, py))| {
                let at = point(px - x, py - y);
                if index == 0 {
                    PathCommand::MoveTo(at)
                } else {
                    PathCommand::LineTo(at)
                }
            })
            .collect();
        let fill = if closed {
            body.get(LINE_TYPE..).and_then(fill_of)
        } else {
            None
        };
        if closed {
            commands.push(PathCommand::Close);
        }
        let paths = vec![path(commands, fill.is_some(), true)];
        self.push(
            x,
            y,
            right - x,
            bottom - y,
            paths,
            Look {
                line: line_of(body),
                fill,
            },
        );
    }

    /// A `DPTXBX`: the next text of the text box story, in the primitive's frame.
    fn text_box(&mut self, body: &[u8], bounds: [i64; 4]) {
        let index = *self.next_box;
        *self.next_box += 1;
        let (Some(start), Some(end)) = (
            self.box_positions.get(index),
            self.box_positions.get(index + 1),
        ) else {
            return;
        };
        let line = line_of(body).map(|(colour, width)| {
            (
                colour,
                u32::try_from((width * 8 / 12_700).max(1)).unwrap_or(6),
            )
        });
        self.text_boxes.push(Word6TextBox {
            bounds: bounds.map(clamp),
            text: self.box_story + *start as usize
                ..self.box_story + (*end as usize).max(*start as usize),
            fill: body.get(LINE_TYPE..).and_then(fill_of),
            line,
            rounded: u16_at(body, LINE_TYPE + FILL + SHADOW).unwrap_or(0) & 1 != 0,
        });
    }

    /// A `DPCALLOUT`: a text box and the polyline that points from it, each a primitive whose
    /// frame is measured from the callout's.
    fn callout(&mut self, body: &[u8], left: i64, top: i64) {
        const PREFIX: usize = 8;
        let text_header = body.get(PREFIX..).unwrap_or_default();
        let text_left = left + signed(text_header, 4);
        let text_top = top + signed(text_header, 6);
        let bounds = [
            text_left,
            text_top,
            text_left + signed(text_header, 8),
            text_top + signed(text_header, 10),
        ];
        let text_body = text_header.get(PRIMITIVE_HEADER..).unwrap_or_default();
        self.text_box(text_body, bounds);
        let line_at = PREFIX + PRIMITIVE_HEADER + LINE_TYPE + FILL + SHADOW + 4;
        let line_header = body.get(line_at..).unwrap_or_default();
        let line_left = left + signed(line_header, 4);
        let line_top = top + signed(line_header, 6);
        self.polyline(
            line_header.get(PRIMITIVE_HEADER..).unwrap_or_default(),
            line_left,
            line_top,
        );
    }
}

#[cfg(test)]
pub(super) mod tests;
