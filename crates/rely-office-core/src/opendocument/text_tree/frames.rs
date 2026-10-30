// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! Where an odt frame stands: its anchor type, the graphic style's position and wrapping, and
//! the fill and border of a text box.

use super::super::styles::{FO, STYLE, Scope, TEXT, border, colour, points};
use super::{Converter, DRAW, SVG, all_text};
use crate::document::{Anchor, AnchorAlign, AnchorBase, AnchorPosition, Media, TextFrame, Wrap};
use crate::geometry::{
    DrawnShape, TextLine, enhanced_geometry, flip_paths, preset_geometry, rectangle_paths,
    shapes_svg, svg_path_geometry,
};
use crate::xml_tree::Element;

const EMU_PER_POINT: f64 = 12_700.0;
/// The properties a group gives the shapes inside it over their own styles.
const GROUP_PLACED: [&str; 10] = [
    "horizontal-pos",
    "horizontal-rel",
    "vertical-pos",
    "vertical-rel",
    "wrap",
    "run-through",
    "margin-top",
    "margin-bottom",
    "margin-left",
    "margin-right",
];

fn emu(points: f64) -> i64 {
    #[expect(clippy::cast_possible_truncation, reason = "a frame is a page's size")]
    let emu = (points * EMU_PER_POINT).round().clamp(-1.0e9, 1.0e9) as i64;
    emu
}

impl Converter<'_, '_> {
    /// The attribute of a frame's graphic style, its own style first, then its parents. Inside
    /// a group the group's style places its shapes, so where and how text runs around them
    /// comes from it first.
    fn graphic_property(
        &self,
        frame: &Element,
        scope: Scope,
        namespace: crate::xml_tree::Namespace,
        name: &str,
    ) -> Option<String> {
        let from = |style: Option<&str>| {
            self.styles
                .properties(scope, "graphic", style, "graphic-properties")
                .into_iter()
                .find_map(|properties| properties.attribute(namespace, name).map(str::to_owned))
        };
        let own = frame.attribute(DRAW, "style-name");
        match self.group_style.as_deref() {
            Some(group) if GROUP_PLACED.contains(&name) => from(Some(group)).or_else(|| from(own)),
            Some(group) => from(own).or_else(|| from(Some(group))),
            None => from(own),
        }
    }

    /// The anchor of a frame; `None` for one set as a character in the line.
    pub(super) fn frame_anchor(&self, frame: &Element, scope: Scope) -> Option<Anchor> {
        let anchor_type = frame
            .attribute(TEXT, "anchor-type")
            .or(self.group_anchor.as_deref())?;
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
            shape: frame_shape(frame),
            id: self.next_frame,
            anchor,
            fill,
            line,
        })
    }

    /// The paragraphs of a shape as centred lines, each in the look of its paragraph style.
    fn shape_text(&self, element: &Element, scope: Scope) -> Vec<TextLine> {
        let paragraphs: Vec<&Element> = element
            .elements()
            .filter(|paragraph| paragraph.is(TEXT, "p"))
            .collect();
        if paragraphs
            .iter()
            .all(|paragraph| all_text(paragraph).trim().is_empty())
        {
            return Vec::new();
        }
        paragraphs
            .into_iter()
            .map(|paragraph| {
                let style = paragraph.attribute(TEXT, "style-name");
                let span = paragraph
                    .elements()
                    .find(|span| span.is(TEXT, "span"))
                    .and_then(|span| span.attribute(TEXT, "style-name"));
                let (paragraph_look, _) = self.styles.text_look(scope, "paragraph", style);
                let (look, _) = self.styles.text_look(scope, "text", span);
                let look = look.or(paragraph_look);
                TextLine {
                    text: all_text(paragraph),
                    size_points: look
                        .size_half_points
                        .map_or(11.0, |half| f64::from(half) / 2.0),
                    colour: look.colour.unwrap_or_else(|| "000000".to_owned()),
                    bold: look.bold.unwrap_or(false),
                    italic: look.italic.unwrap_or(false),
                }
            })
            .collect()
    }

    /// A drawn shape as an SVG picture of its frame: its outline, the fill and
    /// line of its graphic style. Its media, its size in points and its anchor.
    pub(super) fn shape_picture(
        &mut self,
        element: &Element,
        scope: Scope,
    ) -> Option<(usize, f64, f64, Option<Anchor>)> {
        let length = |name: &str| element.attribute(SVG, name).and_then(points);
        let is_line = matches!(element.name.as_str(), "line" | "connector");
        let (left, top, width, height) = if is_line {
            let (x1, y1, x2, y2) = (length("x1")?, length("y1")?, length("x2")?, length("y2")?);
            (x1.min(x2), y1.min(y2), (x2 - x1).abs(), (y2 - y1).abs())
        } else {
            (
                length("x").unwrap_or(0.0),
                length("y").unwrap_or(0.0),
                length("width")?,
                length("height")?,
            )
        };
        if width <= 0.0 && height <= 0.0 {
            return None;
        }
        let (width_emu, height_emu) = (emu(width).max(1), emu(height).max(1));
        let mut paths = match element.name.as_str() {
            "rect" => rectangle_paths(width_emu, height_emu),
            "ellipse" | "circle" => preset_geometry("ellipse", None, width_emu, height_emu)?,
            "polygon" | "polyline" | "path" => svg_path_geometry(element, width_emu, height_emu)?,
            "custom-shape" => element
                .child(DRAW, "enhanced-geometry")
                .and_then(|geometry| enhanced_geometry(geometry, width_emu, height_emu))
                .unwrap_or_else(|| rectangle_paths(width_emu, height_emu)),
            "line" | "connector" => preset_geometry("line", None, width_emu, height_emu)?,
            _ => return None,
        };
        if is_line {
            let falling = |first: &str, second: &str| length(first) < length(second);
            let flipped = falling("x1", "x2") != falling("y1", "y2");
            flip_paths(&mut paths, width_emu, height_emu, flipped, false);
        }
        let property =
            |namespace, name: &str| self.graphic_property(element, scope, namespace, name);
        let fill = (!is_line && property(DRAW, "fill").as_deref() != Some("none"))
            .then(|| property(DRAW, "fill-color").as_deref().and_then(colour))
            .flatten();
        let line = (property(DRAW, "stroke").as_deref() != Some("none")).then(|| {
            (
                property(SVG, "stroke-color")
                    .as_deref()
                    .and_then(colour)
                    .unwrap_or_else(|| "000000".to_owned()),
                property(SVG, "stroke-width")
                    .as_deref()
                    .and_then(points)
                    .map_or(0, emu),
            )
        });
        let drawn = DrawnShape {
            x: 0,
            y: 0,
            width: width_emu,
            height: height_emu,
            rotation: 0.0,
            paths,
            fill,
            line,
            text: self.shape_text(element, scope),
        };
        let bytes = shapes_svg(&[drawn], width_emu, height_emu)?;
        let size = bytes.len() as u64;
        if self.media.len() >= self.limits.max_images
            || self.media_bytes.saturating_add(size) > self.limits.max_total_image_bytes
        {
            self.report.images_over_limit += 1;
            return None;
        }
        self.media_bytes += size;
        self.media.push(Media {
            content_type: "image/svg+xml",
            bytes,
        });
        let anchor = self.frame_anchor(element, scope).map(|mut anchor| {
            if anchor.horizontal.align.is_none() {
                anchor.horizontal.offset_emu = emu(left);
            }
            if anchor.vertical.align.is_none() {
                anchor.vertical.offset_emu = emu(top);
            }
            anchor.width_emu = width_emu;
            anchor.height_emu = height_emu;
            anchor
        });
        Some((self.media.len() - 1, width, height, anchor))
    }
}

/// The `DrawingML` preset a drawn shape is, when its text can be kept as a text box in that
/// shape: an ellipse, and a custom shape whose enhanced geometry names a preset, its own or the
/// one `LibreOffice` maps from `DrawingML` (`ooxml-...`); `None` for a rectangle.
pub(super) fn frame_shape(element: &Element) -> Option<String> {
    match element.name.as_str() {
        "ellipse" | "circle" => Some("ellipse".to_owned()),
        "custom-shape" => {
            let kind = element
                .child(DRAW, "enhanced-geometry")?
                .attribute(DRAW, "type")?;
            if let Some(preset) = kind.strip_prefix("ooxml-") {
                return (preset != "rect"
                    && preset
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric()))
                .then(|| preset.to_owned());
            }
            Some(
                match kind {
                    "ellipse" => "ellipse",
                    "round-rectangle" => "roundRect",
                    "diamond" => "diamond",
                    "isosceles-triangle" => "triangle",
                    "right-triangle" => "rtTriangle",
                    "parallelogram" => "parallelogram",
                    "trapezoid" => "trapezoid",
                    "hexagon" => "hexagon",
                    "octagon" => "octagon",
                    "pentagon" => "pentagon",
                    "right-arrow" => "rightArrow",
                    "left-arrow" => "leftArrow",
                    "up-arrow" => "upArrow",
                    "down-arrow" => "downArrow",
                    "star5" => "star5",
                    "heart" => "heart",
                    "cloud" => "cloudCallout",
                    "smiley" => "smileyFace",
                    "flowchart-process" => "flowChartProcess",
                    "flowchart-decision" => "flowChartDecision",
                    "flowchart-terminator" => "flowChartTerminator",
                    "round-rectangular-callout" => "wedgeRoundRectCallout",
                    "rectangular-callout" => "wedgeRectCallout",
                    "round-callout" => "wedgeEllipseCallout",
                    _ => return None,
                }
                .to_owned(),
            )
        }
        _ => None,
    }
}
