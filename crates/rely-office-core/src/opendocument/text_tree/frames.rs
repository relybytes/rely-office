// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Where an odt frame stands: its anchor type, the graphic style's position and wrapping, and
//! the fill and border of a text box.

use super::super::styles::{FO, STYLE, Scope, TEXT, border, colour, points};
use super::{Converter, DRAW, SVG};
use crate::document::{Anchor, AnchorAlign, AnchorBase, AnchorPosition, TextFrame, Wrap};
use crate::xml_tree::Element;

const EMU_PER_POINT: f64 = 12_700.0;

fn emu(points: f64) -> i64 {
    #[expect(clippy::cast_possible_truncation, reason = "a frame is a page's size")]
    let emu = (points * EMU_PER_POINT).round().clamp(-1.0e9, 1.0e9) as i64;
    emu
}

impl Converter<'_, '_> {
    /// The attribute of a frame's graphic style, its own style first, then its parents.
    fn graphic_property(
        &self,
        frame: &Element,
        scope: Scope,
        namespace: crate::xml_tree::Namespace,
        name: &str,
    ) -> Option<String> {
        self.styles
            .properties(
                scope,
                "graphic",
                frame.attribute(DRAW, "style-name"),
                "graphic-properties",
            )
            .into_iter()
            .find_map(|properties| properties.attribute(namespace, name).map(str::to_owned))
    }

    /// The anchor of a frame; `None` for one set as a character in the line.
    pub(super) fn frame_anchor(&self, frame: &Element, scope: Scope) -> Option<Anchor> {
        let anchor_type = frame.attribute(TEXT, "anchor-type")?;
        if !matches!(anchor_type, "paragraph" | "char" | "page" | "frame") {
            return None;
        }
        let property = |namespace, name: &str| self.graphic_property(frame, scope, namespace, name);
        let length = |name: &str| frame.attribute(SVG, name).and_then(points).unwrap_or(0.0);
        let on_page = anchor_type == "page";
        let horizontal_base = match property(STYLE, "horizontal-rel").as_deref() {
            Some("page-content") => AnchorBase::Margin,
            Some(relation) if relation.starts_with("page") => AnchorBase::Page,
            None if on_page => AnchorBase::Page,
            Some(_) | None => AnchorBase::Column,
        };
        let vertical_base = match property(STYLE, "vertical-rel").as_deref() {
            Some("page-content") => AnchorBase::Margin,
            Some(relation) if relation.starts_with("page") => AnchorBase::Page,
            Some("char" | "line" | "baseline" | "text") => AnchorBase::Line,
            None if on_page => AnchorBase::Page,
            Some(_) | None => AnchorBase::Paragraph,
        };
        let align = |value: Option<String>| match value.as_deref() {
            Some("left" | "top" | "inside") => Some(AnchorAlign::Start),
            Some("center" | "middle") => Some(AnchorAlign::Center),
            Some("right" | "bottom" | "outside") => Some(AnchorAlign::End),
            _ => None,
        };
        let wrap = match property(STYLE, "wrap").as_deref() {
            Some("left" | "right" | "parallel" | "dynamic" | "biggest") => Wrap::Around,
            Some("run-through") => {
                if property(STYLE, "run-through").as_deref() == Some("background") {
                    Wrap::Behind
                } else {
                    Wrap::InFront
                }
            }
            Some(_) | None => Wrap::TopAndBottom,
        };
        let height = frame
            .attribute(SVG, "height")
            .or_else(|| frame.attribute(FO, "min-height"))
            .and_then(points)
            .unwrap_or(0.0);
        let margin = |name: &str| {
            property(FO, name)
                .as_deref()
                .and_then(points)
                .map_or(0, emu)
                .max(0)
        };
        Some(Anchor {
            horizontal: AnchorPosition {
                base: horizontal_base,
                offset_emu: emu(length("x")),
                align: align(property(STYLE, "horizontal-pos")),
            },
            vertical: AnchorPosition {
                base: vertical_base,
                offset_emu: emu(length("y")),
                align: align(property(STYLE, "vertical-pos")),
            },
            width_emu: emu(length("width")).max(0),
            height_emu: emu(height).max(0),
            wrap,
            distance_emu: [
                margin("margin-top"),
                margin("margin-bottom"),
                margin("margin-left"),
                margin("margin-right"),
            ],
        })
    }

    /// The frame of a text box: its anchor, and the fill and border of its graphic style.
    pub(super) fn text_frame(&mut self, frame: &Element, scope: Scope) -> Option<TextFrame> {
        let anchor = self.frame_anchor(frame, scope)?;
        let property = |namespace, name: &str| self.graphic_property(frame, scope, namespace, name);
        let fill = match property(DRAW, "fill").as_deref() {
            Some("none") => None,
            _ => property(DRAW, "fill-color")
                .or_else(|| property(FO, "background-color"))
                .as_deref()
                .and_then(colour),
        };
        // The nearest style that draws an outline decides it, as a stroke or as a border.
        let outline = self
            .styles
            .properties(
                scope,
                "graphic",
                frame.attribute(DRAW, "style-name"),
                "graphic-properties",
            )
            .into_iter()
            .find_map(|properties| {
                properties
                    .attribute(DRAW, "stroke")
                    .map(|_| None)
                    .or_else(|| {
                        properties
                            .attribute(FO, "border")
                            .map(|edge| Some(edge.to_owned()))
                    })
            })
            .flatten();
        let line = if let Some(edge) = outline {
            let edge = border(&edge);
            (edge.width_eighths > 0).then(|| {
                (
                    edge.colour.unwrap_or_else(|| "000000".to_owned()),
                    edge.width_eighths,
                )
            })
        } else if property(DRAW, "stroke")
            .as_deref()
            .is_some_and(|stroke| stroke != "none")
        {
            let width = property(SVG, "stroke-width")
                .as_deref()
                .and_then(points)
                .map_or(6, |points| {
                    #[expect(
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss,
                        reason = "a stroke is a few points wide"
                    )]
                    let eighths = (points * 8.0).round().clamp(1.0, 400.0) as u32;
                    eighths
                });
            Some((
                property(SVG, "stroke-color")
                    .as_deref()
                    .and_then(colour)
                    .unwrap_or_else(|| "000000".to_owned()),
                width,
            ))
        } else {
            None
        };
        self.next_frame += 1;
        Some(TextFrame {
            id: self.next_frame,
            anchor,
            fill,
            line,
        })
    }
}
