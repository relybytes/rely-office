// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! A pptx read as slides to draw: the page size, and each slide's background and shapes with
//! their frames, fills, outlines, text, pictures and tables. What a slide leaves unsaid comes
//! from its layout, then its master, then the master's text styles and the theme, as
//! `PowerPoint` resolves it, so every value here is the one the slide shows.

mod read;
mod text;
mod write;

pub use read::read_presentation;
pub use write::write_pptx;

use crate::document::{Alignment, Media};
use crate::geometry::ShapePath;

/// A presentation's slides, in show order.
#[derive(Debug, Clone, PartialEq)]
pub struct Presentation {
    /// The slide size in English Metric Units, 914,400 to the inch.
    pub width_emu: i64,
    pub height_emu: i64,
    pub slides: Vec<Slide>,
    /// The images of the slides, referenced from [`ShapeContent::Picture`] by index.
    pub media: Vec<Media>,
    /// The presentation held more slides, shapes or images than the limits let through.
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Slide {
    /// The background; unset is white.
    pub background: Option<Paint>,
    /// The shapes in drawing order, the first one at the back.
    pub shapes: Vec<SlideShape>,
}

/// A rectangle on the slide, in English Metric Units from the top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Frame {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
}

/// The outline of a shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShapeGeometry {
    Rectangle,
    RoundedRectangle,
    Ellipse,
    /// A straight line from the top-left to the bottom-right of the frame, or the other
    /// diagonal when flipped.
    Line {
        flipped: bool,
    },
    /// The paths of a preset, a custom geometry or a drawn outline, in the frame.
    Path(Vec<ShapePath>),
    /// A preset this library does not know, drawn as its rectangle.
    Other,
}

/// A shape's border line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeLine {
    pub colour: String,
    /// The width in English Metric Units, 12,700 to the point.
    pub width_emu: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlideShape {
    pub frame: Frame,
    /// Clockwise, in degrees.
    pub rotation: f64,
    pub geometry: ShapeGeometry,
    /// Unset is no fill.
    pub fill: Option<Paint>,
    pub line: Option<ShapeLine>,
    pub content: ShapeContent,
}

/// What fills a shape or a slide.
#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
    /// A colour, `RRGGBB`.
    Solid(String),
    Gradient(Gradient),
    /// An image of [`Presentation::media`], stretched over the frame, or repeated at
    /// `tile_emu` from the top-left corner.
    Picture {
        media: usize,
        tile_emu: Option<(i64, i64)>,
    },
}

impl Paint {
    /// The colour that stands for the paint where only one can be drawn: the colour itself,
    /// or a gradient's first stop; `None` for a picture.
    #[must_use]
    pub fn colour(&self) -> Option<&str> {
        match self {
            Self::Solid(colour) => Some(colour),
            Self::Gradient(gradient) => gradient.stops.first().map(|stop| stop.colour.as_str()),
            Self::Picture { .. } => None,
        }
    }
}

/// A blend of colours across a frame.
#[derive(Debug, Clone, PartialEq)]
pub struct Gradient {
    /// In order of their offset, at least two.
    pub stops: Vec<GradientStop>,
    pub kind: GradientKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GradientKind {
    /// Along a line through the frame, clockwise in degrees from left to right.
    Linear { angle: f64 },
    /// Out from a centre, given as fractions of the frame's width and height, to its far
    /// corners.
    Radial { centre: (f64, f64) },
}

#[derive(Debug, Clone, PartialEq)]
pub struct GradientStop {
    /// From 0.0 at the start to 1.0 at the end.
    pub offset: f64,
    /// `RRGGBB`.
    pub colour: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShapeContent {
    None,
    Text(TextBody),
    /// `media` indexes [`Presentation::media`].
    Picture {
        media: usize,
    },
    Table(SlideTable),
}

/// Where the text of a shape sits in its height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAnchor {
    Top,
    Middle,
    Bottom,
}

/// The text of a shape or a table cell.
#[derive(Debug, Clone, PartialEq)]
pub struct TextBody {
    pub paragraphs: Vec<SlideParagraph>,
    pub anchor: TextAnchor,
    /// The space between the frame and the text: left, top, right, bottom, in English Metric
    /// Units.
    pub insets_emu: [i64; 4],
    pub wrap: bool,
}

/// What a paragraph starts with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bullet {
    /// A character, `•` or `–`.
    Character(String),
    /// A number the paragraph counts, as it shows: `1.`, `a)`, `iii.`.
    Number(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlideParagraph {
    /// The outline level, 0 for the first.
    pub level: u8,
    pub align: Alignment,
    pub bullet: Option<Bullet>,
    /// The left margin and the first-line indent, in English Metric Units; a negative indent
    /// hangs the bullet.
    pub margin_left_emu: i64,
    pub indent_emu: i64,
    /// Line spacing as a percentage of single spacing.
    pub line_spacing_percent: u32,
    /// The space before and after, in hundredths of a point.
    pub space_before: u32,
    pub space_after: u32,
    pub runs: Vec<SlideRun>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is an independent mark of the run, as the file stores them"
)]
pub struct SlideRun {
    pub text: String,
    /// The size in hundredths of a point, the shape's autofit scale applied.
    pub size: u32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    /// The colour as `RRGGBB`.
    pub colour: String,
    pub family: String,
    /// A line break inside the paragraph instead of text.
    pub line_break: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlideTable {
    pub column_widths_emu: Vec<i64>,
    pub rows: Vec<SlideTableRow>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlideTableRow {
    pub height_emu: i64,
    pub cells: Vec<SlideTableCell>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlideTableCell {
    pub body: TextBody,
    pub fill: Option<String>,
    pub col_span: u32,
    pub row_span: u32,
    /// Covered by a merge from a cell before it, and not drawn.
    pub covered: bool,
}
