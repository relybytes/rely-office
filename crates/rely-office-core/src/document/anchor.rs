// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Where a floating picture or text box stands: its position against the page, the margins,
//! the column or the paragraph, its size, and how the text goes around it.

use super::Block;
use crate::xml_tree::{Element, Namespace};

const WP: Namespace = Namespace::WordDrawing;
/// No offset reaches past four of the largest pages.
const MAX_EMU: i64 = 51_206_400 * 4;

/// What a position is measured from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub enum AnchorBase {
    Page,
    /// The page's margins, the text area.
    Margin,
    /// The column of text, the same as the margins on a page of one column.
    Column,
    /// The top of the paragraph that anchors the object.
    Paragraph,
    /// The line, or the character, the object is anchored at.
    Line,
}

/// A side or the middle of what a position is measured from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub enum AnchorAlign {
    /// Left, or top.
    Start,
    Center,
    /// Right, or bottom.
    End,
}

/// One axis of a position: an offset from a base, or an alignment within it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct AnchorPosition {
    pub base: AnchorBase,
    /// In English Metric Units, 914,400 to the inch.
    pub offset_emu: i64,
    /// An alignment, which wins over the offset.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub align: Option<AnchorAlign>,
}

/// How many bands down an object a [`WrapOutline`] measures.
pub const OUTLINE_BANDS: usize = 16;

/// The outline text keeps clear of when it wraps tight around an object: in each of
/// [`OUTLINE_BANDS`] bands from its top, the leftmost and the rightmost point of the outline,
/// in 255ths of the object's width. A band the outline leaves empty has its left past its right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WrapOutline {
    pub left: [u8; OUTLINE_BANDS],
    pub right: [u8; OUTLINE_BANDS],
}

impl WrapOutline {
    /// The outline of a polygon in Word's wrap space, 21,600 units each way; `None` for one of
    /// fewer than three points.
    #[must_use]
    pub fn from_polygon(points: &[(f64, f64)]) -> Option<Self> {
        if points.len() < 3 {
            return None;
        }
        let mut left = [u8::MAX; OUTLINE_BANDS];
        let mut right = [0_u8; OUTLINE_BANDS];
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a share of the width is clamped to 0..=255"
        )]
        let share = |x: f64| (x / 21_600.0 * 255.0).round().clamp(0.0, 255.0) as u8;
        let band_count = f64::from(u8::try_from(OUTLINE_BANDS).unwrap_or(16));
        for band in 0..OUTLINE_BANDS {
            let index = f64::from(u8::try_from(band).unwrap_or(0));
            let (top, bottom) = (
                index / band_count * 21_600.0,
                (index + 1.0) / band_count * 21_600.0,
            );
            let mut reach = |x: f64| {
                left[band] = left[band].min(share(x));
                right[band] = right[band].max(share(x));
            };
            for (start, end) in points.iter().zip(points.iter().cycle().skip(1)) {
                let (low, high) = (start.1.min(end.1), start.1.max(end.1));
                if high < top || low > bottom {
                    continue;
                }
                // The edge's part inside the band: its ends there, or where it crosses the
                // band's top and bottom.
                for y in [top, bottom, start.1, end.1] {
                    if y < top || y > bottom || y < low || y > high {
                        continue;
                    }
                    let x = if (end.1 - start.1).abs() < f64::EPSILON {
                        reach(start.0);
                        end.0
                    } else {
                        start.0 + (end.0 - start.0) * (y - start.1) / (end.1 - start.1)
                    };
                    reach(x);
                }
            }
        }
        Some(Self { left, right })
    }

    /// The outline of a polygon written back: down the right side, up the left, in Word's wrap
    /// space; the bands the outline leaves empty are skipped.
    #[must_use]
    pub fn polygon(&self) -> Vec<(i64, i64)> {
        let band_count = i64::try_from(OUTLINE_BANDS).unwrap_or(16);
        let x = |share: u8| i64::from(share) * 21_600 / 255;
        let filled: Vec<(i64, u8, u8)> = (0..OUTLINE_BANDS)
            .filter(|band| self.left[*band] <= self.right[*band])
            .map(|band| {
                (
                    i64::try_from(band).unwrap_or(0),
                    self.left[band],
                    self.right[band],
                )
            })
            .collect();
        let mut points = Vec::with_capacity(filled.len() * 4);
        for (band, _, right) in &filled {
            points.push((x(*right), band * 21_600 / band_count));
            points.push((x(*right), (band + 1) * 21_600 / band_count));
        }
        for (band, left, _) in filled.iter().rev() {
            points.push((x(*left), (band + 1) * 21_600 / band_count));
            points.push((x(*left), band * 21_600 / band_count));
        }
        points
    }
}

/// How the text goes around a floating object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(tag = "type", content = "outline", rename_all = "camelCase")
)]
pub enum Wrap {
    /// Beside it, on the side with room, as Word's square wrapping.
    Around,
    /// Beside its outline, as Word's tight and through wrapping.
    Contour(WrapOutline),
    /// Above and below it, nothing beside it.
    TopAndBottom,
    /// Over it: the object is in front of the text.
    InFront,
    /// Under it: the object is behind the text.
    Behind,
}

/// The place, size and wrapping of a floating object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct Anchor {
    pub horizontal: AnchorPosition,
    pub vertical: AnchorPosition,
    pub width_emu: i64,
    pub height_emu: i64,
    pub wrap: Wrap,
    /// The space kept between the object and the text: top, bottom, left, right.
    pub distance_emu: [i64; 4],
}

impl Anchor {
    /// A `wp:anchor`; `None` for an inline object.
    pub(crate) fn parse(anchor: &Element) -> Option<Self> {
        if !anchor.is(WP, "anchor") {
            return None;
        }
        let emu = |value: Option<&str>| {
            value
                .and_then(|value| value.parse::<i64>().ok())
                .map_or(0, |value| value.clamp(-MAX_EMU, MAX_EMU))
        };
        let extent = anchor.child(WP, "extent");
        let side = |name: &str| {
            emu(extent.and_then(|extent| extent.attribute(Namespace::None, name))).max(0)
        };
        let position = |name: &str, vertical: bool| {
            let element = anchor.child(WP, name);
            let base = match element
                .and_then(|element| element.attribute(Namespace::None, "relativeFrom"))
            {
                Some("page") => AnchorBase::Page,
                Some(
                    "margin" | "leftMargin" | "rightMargin" | "insideMargin" | "outsideMargin"
                    | "topMargin" | "bottomMargin",
                ) => AnchorBase::Margin,
                Some("paragraph") => AnchorBase::Paragraph,
                Some("line" | "character") => AnchorBase::Line,
                Some(_) | None if vertical => AnchorBase::Paragraph,
                Some(_) | None => AnchorBase::Column,
            };
            let align = element
                .and_then(|element| element.child(WP, "align"))
                .map(Element::text)
                .and_then(|align| match align.trim() {
                    "left" | "top" | "inside" => Some(AnchorAlign::Start),
                    "center" => Some(AnchorAlign::Center),
                    "right" | "bottom" | "outside" => Some(AnchorAlign::End),
                    _ => None,
                });
            let offset = element
                .and_then(|element| element.child(WP, "posOffset"))
                .map_or(0, |offset| emu(Some(offset.text().trim())));
            AnchorPosition {
                base,
                offset_emu: offset,
                align,
            }
        };
        let behind = anchor
            .attribute(Namespace::None, "behindDoc")
            .is_some_and(|value| matches!(value, "1" | "true"));
        let wrap = if anchor.child(WP, "wrapTopAndBottom").is_some() {
            Wrap::TopAndBottom
        } else if let Some(outline) = ["wrapTight", "wrapThrough"]
            .iter()
            .find_map(|name| anchor.child(WP, name))
            .and_then(|wrap| wrap.child(WP, "wrapPolygon"))
            .and_then(|polygon| {
                let points: Vec<(f64, f64)> = polygon
                    .elements()
                    .filter(|point| point.is(WP, "start") || point.is(WP, "lineTo"))
                    .filter_map(|point| {
                        let coordinate = |name: &str| {
                            point
                                .attribute(Namespace::None, name)?
                                .parse::<f64>()
                                .ok()
                                .filter(|value| value.is_finite())
                        };
                        Some((coordinate("x")?, coordinate("y")?))
                    })
                    .take(10_000)
                    .collect();
                WrapOutline::from_polygon(&points)
            })
        {
            Wrap::Contour(outline)
        } else if ["wrapSquare", "wrapTight", "wrapThrough"]
            .iter()
            .any(|name| anchor.child(WP, name).is_some())
        {
            Wrap::Around
        } else if behind {
            Wrap::Behind
        } else {
            Wrap::InFront
        };
        let distance = |name: &str| emu(anchor.attribute(Namespace::None, name)).max(0);
        Some(Self {
            horizontal: position("positionH", false),
            vertical: position("positionV", true),
            width_emu: side("cx"),
            height_emu: side("cy"),
            wrap,
            distance_emu: [
                distance("distT"),
                distance("distB"),
                distance("distL"),
                distance("distR"),
            ],
        })
    }

    /// The anchor of a VML shape, from the CSS of its `style`: `position:absolute;
    /// margin-left:100pt; margin-top:20pt; width:200pt; height:50pt; mso-position-horizontal-
    /// relative:page`; `None` for a shape set in the line.
    pub(crate) fn from_vml_style(style: &str) -> Option<Self> {
        let property = |name: &str| {
            style.split(';').find_map(|declaration| {
                let (key, value) = declaration.split_once(':')?;
                (key.trim().eq_ignore_ascii_case(name)).then(|| value.trim().to_owned())
            })
        };
        if property("position").as_deref() != Some("absolute") {
            return None;
        }
        let length = |name: &str| {
            property(name)
                .and_then(|value| css_emu(&value))
                .unwrap_or(0)
        };
        let base = |name: &str, vertical: bool| match property(name).as_deref() {
            Some("page") => AnchorBase::Page,
            Some("margin") => AnchorBase::Margin,
            Some("line" | "char") => AnchorBase::Line,
            _ if vertical => AnchorBase::Paragraph,
            _ => AnchorBase::Column,
        };
        let align = |name: &str| match property(name).as_deref() {
            Some("left" | "top" | "inside") => Some(AnchorAlign::Start),
            Some("center") => Some(AnchorAlign::Center),
            Some("right" | "bottom" | "outside") => Some(AnchorAlign::End),
            _ => None,
        };
        let behind = property("z-index")
            .and_then(|index| index.parse::<i64>().ok())
            .is_some_and(|index| index < 0);
        Some(Self {
            horizontal: AnchorPosition {
                base: base("mso-position-horizontal-relative", false),
                offset_emu: length("margin-left") + length("left"),
                align: align("mso-position-horizontal"),
            },
            vertical: AnchorPosition {
                base: base("mso-position-vertical-relative", true),
                offset_emu: length("margin-top") + length("top"),
                align: align("mso-position-vertical"),
            },
            width_emu: length("width").max(0),
            height_emu: length("height").max(0),
            wrap: if behind { Wrap::Behind } else { Wrap::Around },
            distance_emu: [0; 4],
        })
    }
}

/// A CSS length in English Metric Units: `12pt`, `2cm`, `1in`, `10mm`, `96px`.
pub(crate) fn css_emu(value: &str) -> Option<i64> {
    let value = value.trim();
    let split = value
        .find(|character: char| character.is_ascii_alphabetic())
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(split);
    let number = number
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())?;
    let points = match unit {
        "pt" | "" => number,
        "in" => number * 72.0,
        "cm" => number * 72.0 / 2.54,
        "mm" => number * 72.0 / 25.4,
        "px" => number * 0.75,
        "pc" => number * 12.0,
        _ => return None,
    };
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        reason = "the length is clamped to a few pages"
    )]
    let emu = (points * 12_700.0)
        .round()
        .clamp(-(MAX_EMU as f64), MAX_EMU as f64) as i64;
    Some(emu)
}

/// The box a text box draws around its content: where it stands, and its fill and outline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextFrame {
    /// The blocks of one text box share an id; consecutive frames with different ids are
    /// different boxes.
    pub id: u32,
    pub anchor: Anchor,
    /// `RRGGBB`.
    pub fill: Option<String>,
    /// The outline's colour and width in eighths of a point.
    pub line: Option<(String, u32)>,
    /// The shape the box is drawn as, by its `DrawingML` preset name: `ellipse`, `roundRect`,
    /// `wedgeRoundRectCallout`; unset is a rectangle.
    pub shape: Option<String>,
}

/// Put the blocks of a text box in its frame.
pub(crate) fn set_frame(blocks: &mut [Block], frame: &TextFrame) {
    for block in blocks {
        match block {
            Block::Paragraph { look, .. }
            | Block::Heading { look, .. }
            | Block::ListItem { look, .. } => {
                look.frame = Some(frame.clone());
            }
            Block::Table { look, .. } => look.frame = Some(frame.clone()),
            Block::Blockquote { blocks } => set_frame(blocks, frame),
            Block::CodeBlock { .. } | Block::HorizontalRule => {}
        }
    }
}

#[cfg(test)]
mod tests;
