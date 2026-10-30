// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! The device context a metafile is played on: its pen, brush and font, its colours and modes,
//! the mapping from logical to device coordinates, the objects the records create, and the
//! shapes, text and pictures it draws onto a [`Canvas`].

use std::f64::consts::{PI, TAU};
use std::fmt::Write as _;

use super::bitmap::Picture;
use super::svg::{Canvas, Paint, TextRun, number};

/// No metafile creates more objects than this.
const MAX_OBJECTS: usize = 65_536;
/// No `SaveDC` nests deeper than this.
const MAX_SAVED: usize = 1_024;
/// No clip intersects more regions than this.
const MAX_CLIPS: usize = 64;
/// No figure holds more points than this.
pub(super) const MAX_POINTS: usize = 1_000_000;

/// A pen: its style (`PS_*` in the low bits), its width in logical units, its colour.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Pen {
    pub style: u32,
    pub width: f64,
    /// A cosmetic pen is one device pixel wide, whatever the mapping.
    pub cosmetic: bool,
    pub colour: String,
}

/// A brush: solid, none, or hatched or patterned, drawn as its colour.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Brush {
    pub hollow: bool,
    pub colour: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Font {
    /// Logical units: negative is the character height, positive the cell height.
    pub height: f64,
    /// Tenths of a degree, counter-clockwise.
    pub escapement: f64,
    pub weight: i32,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub family: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Object {
    Pen(Pen),
    Brush(Brush),
    Font(Font),
    /// A palette, a region or anything else that takes a slot and draws nothing.
    Other,
}

/// An affine transform as `XFORM` writes it: `x' = x * m11 + y * m21 + dx`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Transform {
    pub m11: f64,
    pub m12: f64,
    pub m21: f64,
    pub m22: f64,
    pub dx: f64,
    pub dy: f64,
}

impl Transform {
    pub(super) const IDENTITY: Self = Self {
        m11: 1.0,
        m12: 0.0,
        m21: 0.0,
        m22: 1.0,
        dx: 0.0,
        dy: 0.0,
    };

    fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (
            x * self.m11 + y * self.m21 + self.dx,
            x * self.m12 + y * self.m22 + self.dy,
        )
    }

    /// This transform followed by `then`.
    pub(super) fn then(&self, then: &Self) -> Self {
        Self {
            m11: self.m11 * then.m11 + self.m12 * then.m21,
            m12: self.m11 * then.m12 + self.m12 * then.m22,
            m21: self.m21 * then.m11 + self.m22 * then.m21,
            m22: self.m21 * then.m12 + self.m22 * then.m22,
            dx: self.dx * then.m11 + self.dy * then.m21 + then.dx,
            dy: self.dx * then.m12 + self.dy * then.m22 + then.dy,
        }
    }
}

/// What `SaveDC` keeps.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct State {
    pub pen: Pen,
    pub brush: Brush,
    pub font: Font,
    pub text_colour: String,
    pub background: String,
    pub opaque: bool,
    pub even_odd: bool,
    pub text_align: u32,
    pub map_mode: u32,
    pub window_origin: (f64, f64),
    pub window_extent: (f64, f64),
    pub viewport_origin: (f64, f64),
    pub viewport_extent: (f64, f64),
    pub world: Transform,
    pub position: (f64, f64),
    pub clockwise: bool,
    /// The clip region: device paths, every one of which a drawing must fall in.
    pub clip: Vec<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            pen: Pen {
                style: 0,
                width: 0.0,
                cosmetic: true,
                colour: "000000".to_owned(),
            },
            brush: Brush {
                hollow: false,
                colour: "FFFFFF".to_owned(),
            },
            font: Font {
                height: -12.0,
                escapement: 0.0,
                weight: 400,
                italic: false,
                underline: false,
                strike: false,
                family: "Arial".to_owned(),
            },
            text_colour: "000000".to_owned(),
            background: "FFFFFF".to_owned(),
            opaque: true,
            even_odd: true,
            text_align: 0,
            map_mode: 1,
            window_origin: (0.0, 0.0),
            window_extent: (1.0, 1.0),
            viewport_origin: (0.0, 0.0),
            viewport_extent: (1.0, 1.0),
            world: Transform::IDENTITY,
            position: (0.0, 0.0),
            clockwise: false,
            clip: Vec::new(),
        }
    }
}

pub(super) struct Context {
    pub state: State,
    saved: Vec<State>,
    objects: Vec<Option<Object>>,
    pub canvas: Canvas,
    /// The figures drawn since `BeginPath`, kept until the path is filled or stroked.
    pub path: Option<String>,
    /// Device pixels per millimetre, for the metric mapping modes.
    pub pixels_per_mm: (f64, f64),
    /// The mask a `SRCPAINT` bitmap set, waiting for the `SRCAND` image it makes transparent:
    /// where it goes, then its width, height and pixels.
    pub pending_mask: Option<([f64; 4], usize, usize, Vec<u8>)>,
}

impl Context {
    pub(super) fn new(pixels_per_mm: (f64, f64)) -> Self {
        Self {
            state: State::default(),
            saved: Vec::new(),
            objects: Vec::new(),
            canvas: Canvas::default(),
            path: None,
            pixels_per_mm,
            pending_mask: None,
        }
    }

    /// A logical point on the device.
    pub(super) fn device(&self, x: f64, y: f64) -> (f64, f64) {
        let state = &self.state;
        let (x, y) = state.world.apply(x, y);
        let (x, y) = (x - state.window_origin.0, y - state.window_origin.1);
        let (scale_x, scale_y) = match state.map_mode {
            7 | 8 => {
                let ratio = |viewport: f64, window: f64| {
                    if window.abs() < f64::EPSILON {
                        1.0
                    } else {
                        viewport / window
                    }
                };
                let mut scale = (
                    ratio(state.viewport_extent.0, state.window_extent.0),
                    ratio(state.viewport_extent.1, state.window_extent.1),
                );
                // Isotropic keeps one unit the same length both ways, the smaller of the two.
                if state.map_mode == 7 {
                    let unit = scale.0.abs().min(scale.1.abs());
                    scale = (unit.copysign(scale.0), unit.copysign(scale.1));
                }
                scale
            }
            2..=6 => {
                let millimetres = match state.map_mode {
                    2 => 0.1,
                    3 => 0.01,
                    4 => 0.254,
                    5 => 0.0254,
                    _ => 25.4 / 1_440.0,
                };
                (
                    millimetres * self.pixels_per_mm.0,
                    -millimetres * self.pixels_per_mm.1,
                )
            }
            _ => (1.0, 1.0),
        };
        (
            x * scale_x + state.viewport_origin.0,
            y * scale_y + state.viewport_origin.1,
        )
    }

    /// How long a logical unit is on the device, across and down.
    pub(super) fn scale(&self) -> (f64, f64) {
        let origin = self.device(0.0, 0.0);
        let across = self.device(1.0, 0.0);
        let down = self.device(0.0, 1.0);
        (
            (across.0 - origin.0).hypot(across.1 - origin.1),
            (down.0 - origin.0).hypot(down.1 - origin.1),
        )
    }

    pub(super) fn save(&mut self) {
        if self.saved.len() < MAX_SAVED {
            self.saved.push(self.state.clone());
        }
    }

    /// Restore the state `relative` saves back (negative), or the save of that number.
    pub(super) fn restore(&mut self, relative: i32) {
        let target = if relative < 0 {
            self.saved
                .len()
                .checked_sub(usize::try_from(relative.unsigned_abs()).unwrap_or(usize::MAX))
        } else {
            usize::try_from(relative)
                .ok()
                .and_then(|number| number.checked_sub(1))
        };
        if let Some(target) = target.filter(|target| *target < self.saved.len()) {
            self.saved.truncate(target + 1);
            if let Some(state) = self.saved.pop() {
                self.state = state;
            }
        }
    }

    /// Put an object in its slot, as an EMF numbers it.
    pub(super) fn set_object(&mut self, index: usize, object: Object) {
        if index >= MAX_OBJECTS {
            return;
        }
        if self.objects.len() <= index {
            self.objects.resize(index + 1, None);
        }
        self.objects[index] = Some(object);
    }

    /// Put an object in the first free slot, as a WMF numbers them.
    pub(super) fn add_object(&mut self, object: Object) {
        match self.objects.iter().position(Option::is_none) {
            Some(free) => self.objects[free] = Some(object),
            None if self.objects.len() < MAX_OBJECTS => self.objects.push(Some(object)),
            None => {}
        }
    }

    pub(super) fn delete_object(&mut self, index: usize) {
        if let Some(slot) = self.objects.get_mut(index) {
            *slot = None;
        }
    }

    pub(super) fn select_object(&mut self, index: usize) {
        match self.objects.get(index).cloned().flatten() {
            Some(Object::Pen(pen)) => self.state.pen = pen,
            Some(Object::Brush(brush)) => self.state.brush = brush,
            Some(Object::Font(font)) => self.state.font = font,
            Some(Object::Other) | None => {}
        }
    }

    /// One of the stock objects an EMF names by `0x8000_0000` plus its number.
    pub(super) fn select_stock_object(&mut self, number: u32) {
        let brush = |colour: &str| {
            Object::Brush(Brush {
                hollow: false,
                colour: colour.to_owned(),
            })
        };
        let pen = |colour: &str, style: u32| {
            Object::Pen(Pen {
                style,
                width: 0.0,
                cosmetic: true,
                colour: colour.to_owned(),
            })
        };
        let object = match number {
            0 => brush("FFFFFF"),
            1 => brush("C0C0C0"),
            2 => brush("808080"),
            3 => brush("404040"),
            4 => brush("000000"),
            5 => Object::Brush(Brush {
                hollow: true,
                colour: "000000".to_owned(),
            }),
            6 => pen("FFFFFF", 0),
            7 => pen("000000", 0),
            8 => pen("000000", 5),
            10..=17 => Object::Font(State::default().font),
            _ => return,
        };
        match object {
            Object::Pen(pen) => self.state.pen = pen,
            Object::Brush(brush) => self.state.brush = brush,
            Object::Font(font) => self.state.font = font,
            Object::Other => {}
        }
    }

    /// The paint of a closed shape: the brush fills, the pen outlines.
    fn shape_paint(&self, filled: bool, stroked: bool) -> Paint {
        let pen = &self.state.pen;
        let stroke = (stroked && pen.style & 0x0F != 5).then(|| pen.colour.clone());
        let width = if pen.cosmetic || pen.width <= 0.0 {
            1.0
        } else {
            let (scale_x, scale_y) = self.scale();
            pen.width * f64::midpoint(scale_x, scale_y)
        };
        let dashes = match pen.style & 0x0F {
            1 => vec![width * 3.0, width],
            2 => vec![width, width],
            3 => vec![width * 3.0, width, width, width],
            4 => vec![width * 3.0, width, width, width, width, width],
            _ => Vec::new(),
        };
        Paint {
            fill: (filled && !self.state.brush.hollow).then(|| self.state.brush.colour.clone()),
            even_odd: self.state.even_odd,
            stroke,
            stroke_width: width,
            dashes,
        }
    }

    /// Draw figures, or add them to the open path.
    pub(super) fn draw(&mut self, data: &str, filled: bool, stroked: bool) {
        if let Some(path) = &mut self.path {
            if path.len() < MAX_POINTS * 16 {
                path.push_str(data);
            }
            return;
        }
        let paint = self.shape_paint(filled, stroked);
        self.canvas.set_clip(&self.state.clip);
        self.canvas.path(data, &paint);
    }

    /// Fill, stroke or both the path `BeginPath` opened.
    pub(super) fn draw_path(&mut self, filled: bool, stroked: bool) {
        if let Some(data) = self.path.take() {
            let paint = self.shape_paint(filled, stroked);
            self.canvas.set_clip(&self.state.clip);
            self.canvas.path(&data, &paint);
        }
    }

    /// Narrow the clip to a logical rectangle.
    pub(super) fn intersect_clip(&mut self, bounds: [f64; 4]) {
        let data = self.rectangle(bounds, None);
        self.add_clip(data);
    }

    /// Narrow the clip to the path `BeginPath` opened, or replace it with that path.
    pub(super) fn clip_to_path(&mut self, replace: bool) {
        if let Some(data) = self.path.take() {
            if replace {
                self.state.clip.clear();
            }
            self.add_clip(data);
        }
    }

    /// Narrow the clip to device rectangles, or replace it with them; none and a replace is
    /// no clip at all.
    pub(super) fn clip_to_region(&mut self, rectangles: &[[f64; 4]], replace: bool) {
        if replace {
            self.state.clip.clear();
        }
        if rectangles.is_empty() {
            return;
        }
        let mut data = String::new();
        for [left, top, right, bottom] in rectangles.iter().take(MAX_POINTS / 4) {
            let _ = write!(
                data,
                "M{} {}H{}V{}H{}Z",
                number(*left),
                number(*top),
                number(*right),
                number(*bottom),
                number(*left)
            );
        }
        self.add_clip(data);
    }

    fn add_clip(&mut self, data: String) {
        if !data.is_empty() && self.state.clip.len() < MAX_CLIPS {
            self.state.clip.push(data);
        }
    }

    /// The path data of points as one figure, from the current position when `continued`.
    pub(super) fn figure(&self, points: &[(f64, f64)], closed: bool, continued: bool) -> String {
        let mut data = String::new();
        if continued {
            let (x, y) = self.device(self.state.position.0, self.state.position.1);
            let _ = write!(data, "M{} {}", number(x), number(y));
        }
        for (index, (x, y)) in points.iter().take(MAX_POINTS).enumerate() {
            let (x, y) = self.device(*x, *y);
            let command = if index == 0 && !continued { 'M' } else { 'L' };
            let _ = write!(data, "{command}{} {}", number(x), number(y));
        }
        if closed && !data.is_empty() {
            data.push('Z');
        }
        data
    }

    /// The path data of Bezier curves: a start point, then three points per curve, or the
    /// curves alone from the current position when `continued`.
    pub(super) fn curves(&self, points: &[(f64, f64)], continued: bool) -> String {
        let mut data = String::new();
        let (start, rest) = if continued {
            (Some(self.state.position), points)
        } else {
            match points.split_first() {
                Some((first, rest)) => (Some(*first), rest),
                None => (None, points),
            }
        };
        let Some(start) = start else {
            return data;
        };
        let (x, y) = self.device(start.0, start.1);
        let _ = write!(data, "M{} {}", number(x), number(y));
        for curve in rest.as_chunks::<3>().0.iter().take(MAX_POINTS / 3) {
            data.push('C');
            for (index, (x, y)) in curve.iter().enumerate() {
                let (x, y) = self.device(*x, *y);
                if index > 0 {
                    data.push(' ');
                }
                let _ = write!(data, "{} {}", number(x), number(y));
            }
        }
        data
    }

    /// A rectangle, its corners rounded by an ellipse of `corner` when given.
    pub(super) fn rectangle(
        &self,
        [left, top, right, bottom]: [f64; 4],
        corner: Option<(f64, f64)>,
    ) -> String {
        let (x1, y1) = self.device(left, top);
        let (x2, y2) = self.device(right, bottom);
        let (left, right) = (x1.min(x2), x1.max(x2));
        let (top, bottom) = (y1.min(y2), y1.max(y2));
        let (scale_x, scale_y) = self.scale();
        let (radius_x, radius_y) = corner.map_or((0.0, 0.0), |(width, height)| {
            (
                (width.abs() * scale_x / 2.0).min((right - left) / 2.0),
                (height.abs() * scale_y / 2.0).min((bottom - top) / 2.0),
            )
        });
        if radius_x <= 0.0 || radius_y <= 0.0 {
            return format!(
                "M{} {}H{}V{}H{}Z",
                number(left),
                number(top),
                number(right),
                number(bottom),
                number(left)
            );
        }
        let arc = |x: f64, y: f64| {
            format!(
                "A{} {} 0 0 1 {} {}",
                number(radius_x),
                number(radius_y),
                number(x),
                number(y)
            )
        };
        format!(
            "M{} {}H{}{}V{}{}H{}{}V{}{}Z",
            number(left + radius_x),
            number(top),
            number(right - radius_x),
            arc(right, top + radius_y),
            number(bottom - radius_y),
            arc(right - radius_x, bottom),
            number(left + radius_x),
            arc(left, bottom - radius_y),
            number(top + radius_y),
            arc(left + radius_x, top)
        )
    }

    /// An ellipse inscribed in a box.
    pub(super) fn ellipse(&self, [left, top, right, bottom]: [f64; 4]) -> String {
        let (x1, y1) = self.device(left, top);
        let (x2, y2) = self.device(right, bottom);
        let (centre_x, centre_y) = (f64::midpoint(x1, x2), f64::midpoint(y1, y2));
        let (radius_x, radius_y) = ((x2 - x1).abs() / 2.0, (y2 - y1).abs() / 2.0);
        if radius_x <= 0.0 || radius_y <= 0.0 {
            return String::new();
        }
        let (rx, ry) = (number(radius_x), number(radius_y));
        format!(
            "M{} {}A{rx} {ry} 0 1 0 {} {}A{rx} {ry} 0 1 0 {} {}Z",
            number(centre_x - radius_x),
            number(centre_y),
            number(centre_x + radius_x),
            number(centre_y),
            number(centre_x - radius_x),
            number(centre_y)
        )
    }

    /// An arc of the ellipse in a box, from the ray through `start` to the ray through `end`,
    /// counter-clockwise unless the arc direction is clockwise; a pie joins its ends to the
    /// centre and a chord to each other.
    pub(super) fn arc(
        &self,
        bounds: [f64; 4],
        start: (f64, f64),
        end: (f64, f64),
        kind: ArcKind,
    ) -> String {
        let [left, top, right, bottom] = bounds;
        let (x1, y1) = self.device(left, top);
        let (x2, y2) = self.device(right, bottom);
        let (centre_x, centre_y) = (f64::midpoint(x1, x2), f64::midpoint(y1, y2));
        let (radius_x, radius_y) = ((x2 - x1).abs() / 2.0, (y2 - y1).abs() / 2.0);
        if radius_x <= 0.0 || radius_y <= 0.0 {
            return String::new();
        }
        let angle = |point: (f64, f64)| {
            let (x, y) = self.device(point.0, point.1);
            (-(y - centre_y) / radius_y).atan2((x - centre_x) / radius_x)
        };
        let (from, to) = (angle(start), angle(end));
        let mut sweep = (to - from).rem_euclid(TAU);
        if sweep < 1e-9 {
            sweep = TAU;
        }
        let clockwise = self.state.clockwise;
        if clockwise {
            sweep = TAU - sweep;
            if sweep < 1e-9 {
                sweep = TAU;
            }
        }
        let point = |theta: f64| {
            (
                centre_x + radius_x * theta.cos(),
                centre_y - radius_y * theta.sin(),
            )
        };
        let (start_x, start_y) = point(from);
        let mut data = String::new();
        if kind == ArcKind::Pie {
            let _ = write!(data, "M{} {}L", number(centre_x), number(centre_y));
        } else {
            data.push('M');
        }
        let _ = write!(data, "{} {}", number(start_x), number(start_y));
        // A whole turn is two halves: SVG draws nothing for an arc that ends where it starts.
        let mut drawn = 0.0;
        while drawn < sweep - 1e-9 {
            let step = (sweep - drawn).min(PI);
            drawn += step;
            let theta = if clockwise {
                from - drawn
            } else {
                from + drawn
            };
            let (x, y) = point(theta);
            let _ = write!(
                data,
                "A{} {} 0 0 {} {} {}",
                number(radius_x),
                number(radius_y),
                u8::from(clockwise),
                number(x),
                number(y)
            );
        }
        if kind != ArcKind::Open {
            data.push('Z');
        }
        data
    }

    /// Text at a logical reference point, `widths` the advance of each character when the
    /// metafile gives them.
    pub(super) fn text(
        &mut self,
        at: (f64, f64),
        text: &str,
        widths: Option<f64>,
        opaque_box: Option<[f64; 4]>,
    ) {
        let state = &self.state;
        let (scale_x, scale_y) = self.scale();
        let font = &state.font;
        let height = if font.height < 0.0 {
            -font.height
        } else if font.height > 0.0 {
            font.height * 0.82
        } else {
            12.0
        };
        let size = height * scale_y;
        let (x, y) = self.device(at.0, at.1);
        let anchor = match state.text_align & 0x06 {
            6 => "middle",
            2 => "end",
            _ => "start",
        };
        let baseline_shift = match state.text_align & 0x18 {
            24 => 0.0,
            8 => -size * 0.2,
            _ => size * 0.8,
        };
        let background = opaque_box
            .filter(|_| state.opaque)
            .map(|[left, top, right, bottom]| {
                let (x1, y1) = self.device(left, top);
                let (x2, y2) = self.device(right, bottom);
                (
                    state.background.clone(),
                    [x1.min(x2), y1.min(y2), x1.max(x2), y1.max(y2)],
                )
            });
        let run = TextRun {
            x,
            y,
            text: text.to_owned(),
            family: font.family.clone(),
            size,
            bold: font.weight >= 600,
            italic: font.italic,
            underline: font.underline,
            strike: font.strike,
            colour: state.text_colour.clone(),
            anchor,
            baseline_shift,
            angle: font.escapement / 10.0,
            width: widths.map(|width| width * scale_x),
            background,
        };
        self.canvas.set_clip(&self.state.clip);
        self.canvas.text(&run);
    }

    /// A picture stretched over a logical rectangle.
    pub(super) fn picture(&mut self, picture: &Picture, [x, y, width, height]: [f64; 4]) {
        let (x1, y1) = self.device(x, y);
        let (x2, y2) = self.device(x + width, y + height);
        self.canvas.set_clip(&self.state.clip);
        self.canvas.image(
            picture.media_type,
            &picture.bytes,
            [x1, y1, x2 - x1, y2 - y1],
        );
    }

    /// A logical rectangle filled with a colour, as a raster operation paints it.
    pub(super) fn fill_rectangle(&mut self, [x, y, width, height]: [f64; 4], colour: &str) {
        let data = self.rectangle([x, y, x + width, y + height], None);
        let paint = Paint {
            fill: Some(colour.to_owned()),
            even_odd: false,
            stroke: None,
            stroke_width: 0.0,
            dashes: Vec::new(),
        };
        self.canvas.set_clip(&self.state.clip);
        self.canvas.path(&data, &paint);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ArcKind {
    Open,
    Chord,
    Pie,
}

/// A `COLORREF`, `0x00BBGGRR`, as `RRGGBB`.
pub(super) fn colour_of(value: u32) -> String {
    let [red, green, blue, _] = value.to_le_bytes();
    format!("{red:02X}{green:02X}{blue:02X}")
}

#[cfg(test)]
mod tests;
