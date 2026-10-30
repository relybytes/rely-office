// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! The shapes of a docx drawn as pictures: a `wps:wsp` with no text box, and a `wpg:wgp` group
//! of shapes with their text, each with its preset or custom outline, its fill and line from
//! its properties or its style; and the shapes of a `SmartArt`, from the drawing Office keeps
//! beside its data model, the same shapes in the `dsp:` namespace with `DrawingML` text.

use super::{Converter, EMUS_PER_PIXEL};
use crate::document::{Inline, Media};
use crate::geometry::{
    DrawnShape, TextLine, custom_geometry, flip_paths, preset_geometry, rectangle_paths, shapes_svg,
};
use crate::theme::{Theme, colour_in};
use crate::xml_tree::{Element, Namespace, parse_tree};

const A: Namespace = Namespace::Drawing;
const DSP: Namespace = Namespace::DiagramDrawing;
const WPS: Namespace = Namespace::WordShape;
const WPG: Namespace = Namespace::WordGroup;
const W: Namespace = Namespace::Word;
/// A line no width names is three quarters of a point.
const DEFAULT_LINE_EMU: i64 = 9_525;
/// No group nests deeper than this.
const MAX_DEPTH: usize = 16;

/// How a group's child coordinates map onto the picture: `x' = (x - from) * scale + to`.
#[derive(Debug, Clone, Copy)]
struct Placement {
    from: (f64, f64),
    scale: (f64, f64),
    to: (f64, f64),
}

impl Placement {
    const IDENTITY: Self = Self {
        from: (0.0, 0.0),
        scale: (1.0, 1.0),
        to: (0.0, 0.0),
    };

    fn point(&self, x: i64, y: i64) -> (i64, i64) {
        #[expect(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            reason = "drawing coordinates stay within a few hundred million"
        )]
        let mapped = (
            ((x as f64 - self.from.0) * self.scale.0 + self.to.0).round() as i64,
            ((y as f64 - self.from.1) * self.scale.1 + self.to.1).round() as i64,
        );
        mapped
    }

    fn size(&self, width: i64, height: i64) -> (i64, i64) {
        #[expect(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            reason = "drawing sizes stay within a few hundred million"
        )]
        let size = (
            (width as f64 * self.scale.0).round().abs() as i64,
            (height as f64 * self.scale.1).round().abs() as i64,
        );
        size
    }
}

impl Converter<'_, '_> {
    /// A `SmartArt` of a drawing frame as a picture in `content`, drawn from the drawing part its
    /// data model names; whether it drew one.
    pub(super) fn append_smart_art(
        &mut self,
        frame: &Element,
        graphic_data: Option<&Element>,
        anchor: Option<crate::document::Anchor>,
        content: &mut Vec<Inline>,
    ) -> bool {
        let extent = frame.child(Namespace::WordDrawing, "extent");
        let emus = |name: &str| {
            extent
                .and_then(|extent| extent.attribute(Namespace::None, name))
                .and_then(|emus| emus.parse::<i64>().ok())
                .filter(|emus| *emus > 0)
        };
        let (Some(data), Some(width), Some(height)) = (graphic_data, emus("cx"), emus("cy")) else {
            return false;
        };
        let target = |id: &str| {
            self.relationships
                .get(id)
                .filter(|relationship| !relationship.external)
                .map(|relationship| relationship.target.clone())
        };
        let Some(data_part) = data
            .descendant(Namespace::Diagram, "relIds")
            .and_then(|ids| ids.attribute(Namespace::Relationships, "dm"))
            .and_then(target)
        else {
            return false;
        };
        let Some(model) = crate::archive::read_text_member(self.package, &data_part, self.limits)
            .ok()
            .flatten()
        else {
            return false;
        };
        let drawing = smart_art_drawing(&model).and_then(target).or_else(|| {
            let mut drawings = self.relationships.values().filter(|relationship| {
                !relationship.external && relationship.is_kind("diagramDrawing")
            });
            match (drawings.next(), drawings.next()) {
                (Some(only), None) => Some(only.target.clone()),
                _ => None,
            }
        });
        let Some(xml) = drawing.and_then(|drawing| {
            crate::archive::read_text_member(self.package, &drawing, self.limits)
                .ok()
                .flatten()
        }) else {
            return false;
        };
        let theme = self.loaded_theme();
        let Some(bytes) = smart_art_svg(&xml, &theme, width, height) else {
            return false;
        };
        let size = bytes.len() as u64;
        if self.media.len() >= self.limits.max_images
            || self.media_bytes.saturating_add(size) > self.limits.max_total_image_bytes
        {
            self.report.images_over_limit += 1;
            return false;
        }
        self.media_bytes += size;
        self.media.push(Media {
            content_type: "image/svg+xml",
            bytes,
        });
        let pixels = |emus: i64| {
            u32::try_from((emus.unsigned_abs() + EMUS_PER_PIXEL / 2) / EMUS_PER_PIXEL).ok()
        };
        content.push(Inline::Image {
            media: self.media.len() - 1,
            alt: None,
            width: pixels(width),
            height: pixels(height),
            anchor: anchor.map(Box::new),
        });
        true
    }

    /// A shape or group of a drawing frame as a picture in `content`, at the frame's size and
    /// anchor; whether it drew one.
    pub(super) fn append_shape(
        &mut self,
        frame: &Element,
        graphic_data: Option<&Element>,
        anchor: Option<crate::document::Anchor>,
        content: &mut Vec<Inline>,
    ) -> bool {
        let extent = frame.child(Namespace::WordDrawing, "extent");
        let emus = |name: &str| {
            extent
                .and_then(|extent| extent.attribute(Namespace::None, name))
                .and_then(|emus| emus.parse::<i64>().ok())
                .filter(|emus| *emus > 0)
        };
        let (Some(data), Some(width), Some(height)) = (graphic_data, emus("cx"), emus("cy")) else {
            return false;
        };
        let Some(media) = self.shape_picture(data, width, height) else {
            return false;
        };
        let pixels = |emus: i64| {
            u32::try_from((emus.unsigned_abs() + EMUS_PER_PIXEL / 2) / EMUS_PER_PIXEL).ok()
        };
        content.push(Inline::Image {
            media,
            alt: None,
            width: pixels(width),
            height: pixels(height),
            anchor: anchor.map(Box::new),
        });
        true
    }

    /// A shape or group of a `w:drawing` as a picture of its extent; `None` for one that draws
    /// nothing, or past the image limits.
    pub(super) fn shape_picture(
        &mut self,
        graphic_data: &Element,
        width: i64,
        height: i64,
    ) -> Option<usize> {
        let theme = self.loaded_theme();
        let mut shapes = Vec::new();
        if let Some(group) = graphic_data.child(WPG, "wgp") {
            let placement = group_placement(
                group.child(WPG, "grpSpPr"),
                Placement::IDENTITY,
                (width, height),
            );
            collect(group, &theme, placement, &mut shapes, 0);
        } else if let Some(shape) = graphic_data.child(WPS, "wsp") {
            shapes.extend(drawn(
                shape,
                WPS,
                &theme,
                Placement::IDENTITY,
                Some((width, height)),
            ));
        }
        let bytes = shapes_svg(&shapes, width, height)?;
        if self.media.len() >= self.limits.max_images {
            self.report.images_over_limit += 1;
            return None;
        }
        let size = bytes.len() as u64;
        if self.media_bytes.saturating_add(size) > self.limits.max_total_image_bytes {
            self.report.images_over_limit += 1;
            return None;
        }
        self.media_bytes += size;
        self.media.push(Media {
            content_type: "image/svg+xml",
            bytes,
        });
        Some(self.media.len() - 1)
    }

    /// The theme of the document, read the first time it is needed.
    pub(super) fn loaded_theme(&mut self) -> Theme {
        if self.theme.is_none() {
            let theme = self
                .theme_target
                .clone()
                .and_then(|target| {
                    crate::archive::read_text_member(self.package, &target, self.limits)
                        .ok()
                        .flatten()
                })
                .map(|xml| Theme::parse(&xml))
                .unwrap_or_default();
            self.theme = Some(theme);
        }
        self.theme.clone().unwrap_or_default()
    }
}

/// The relationship id of the drawing a `SmartArt`'s data model names, `dsp:dataModelExt`.
pub(crate) fn smart_art_drawing(model: &str) -> Option<&str> {
    let start = model.find("dataModelExt")?;
    let rest = &model[start..];
    let at = rest.find("relId=\"")? + 7;
    let end = rest[at..].find('"')?;
    Some(&rest[at..at + end]).filter(|id| {
        id.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
    })
}

/// The SVG of a `SmartArt` drawing part in a frame of `width` by `height` EMU: its shapes, its
/// groups, their outlines, fills, lines and text.
pub(crate) fn smart_art_svg(xml: &str, theme: &Theme, width: i64, height: i64) -> Option<Vec<u8>> {
    let root = parse_tree(xml, |namespace, name| namespace == A && name == "t").ok()?;
    let tree = root.child(DSP, "spTree")?;
    let mut shapes = Vec::new();
    collect(tree, theme, Placement::IDENTITY, &mut shapes, 0);
    shapes_svg(&shapes, width, height)
}

/// The placement of a group's children: its child extent stretched over its own frame.
fn group_placement(
    properties: Option<&Element>,
    parent: Placement,
    frame: (i64, i64),
) -> Placement {
    let transform = properties.and_then(|properties| properties.child(A, "xfrm"));
    let pair = |name: &str, first: &str, second: &str| {
        let element = transform?.child(A, name)?;
        Some((
            element
                .attribute(Namespace::None, first)?
                .parse::<i64>()
                .ok()?,
            element
                .attribute(Namespace::None, second)?
                .parse::<i64>()
                .ok()?,
        ))
    };
    let offset = pair("off", "x", "y").unwrap_or((0, 0));
    let extent = pair("ext", "cx", "cy").unwrap_or(frame);
    let child_offset = pair("chOff", "x", "y").unwrap_or(offset);
    let child_extent = pair("chExt", "cx", "cy").unwrap_or(extent);
    let (to_x, to_y) = parent.point(offset.0, offset.1);
    let (width, height) = parent.size(extent.0, extent.1);
    #[expect(clippy::cast_precision_loss, reason = "drawing sizes fit in f64")]
    let ratio = |size: i64, child: i64| {
        if child == 0 {
            1.0
        } else {
            size as f64 / child as f64
        }
    };
    #[expect(clippy::cast_precision_loss, reason = "drawing coordinates fit in f64")]
    Placement {
        from: (child_offset.0 as f64, child_offset.1 as f64),
        scale: (ratio(width, child_extent.0), ratio(height, child_extent.1)),
        to: (to_x as f64, to_y as f64),
    }
}

/// The shapes of a group and of the groups inside it.
fn collect(
    group: &Element,
    theme: &Theme,
    placement: Placement,
    shapes: &mut Vec<DrawnShape>,
    depth: usize,
) {
    if depth > MAX_DEPTH {
        return;
    }
    for child in group.elements() {
        match (child.namespace, child.name.as_str()) {
            (WPS, "wsp") => shapes.extend(drawn(child, WPS, theme, placement, None)),
            (DSP, "sp") => shapes.extend(drawn(child, DSP, theme, placement, None)),
            (WPG, "grpSp") => {
                let inner = group_placement(child.child(WPG, "grpSpPr"), placement, (0, 0));
                collect(child, theme, inner, shapes, depth + 1);
            }
            (DSP, "grpSp") => {
                let inner = group_placement(child.child(DSP, "grpSpPr"), placement, (0, 0));
                collect(child, theme, inner, shapes, depth + 1);
            }
            _ => {}
        }
    }
}

/// One `wps:wsp` or `dsp:sp`, the namespace `own` says: its frame from its transform (or the
/// whole picture), its outline, fill, line and text.
fn drawn(
    shape: &Element,
    own: Namespace,
    theme: &Theme,
    placement: Placement,
    whole: Option<(i64, i64)>,
) -> Option<DrawnShape> {
    let properties = shape.child(own, "spPr");
    let transform = properties.and_then(|properties| properties.child(A, "xfrm"));
    let number = |element: Option<&Element>, name: &str| {
        element
            .and_then(|element| element.attribute(Namespace::None, name))
            .and_then(|value| value.parse::<i64>().ok())
    };
    let offset = transform.and_then(|transform| transform.child(A, "off"));
    let extent = transform.and_then(|transform| transform.child(A, "ext"));
    let (x, y, width, height) = if let Some((width, height)) = whole {
        (0, 0, width, height)
    } else {
        let (x, y) = placement.point(
            number(offset, "x").unwrap_or(0),
            number(offset, "y").unwrap_or(0),
        );
        let (width, height) = placement.size(
            number(extent, "cx").unwrap_or(0),
            number(extent, "cy").unwrap_or(0),
        );
        (x, y, width, height)
    };
    if width <= 0 && height <= 0 {
        return None;
    }
    let flipped = |name: &str| {
        transform
            .and_then(|transform| transform.attribute(Namespace::None, name))
            .is_some_and(|value| matches!(value, "1" | "true"))
    };
    #[expect(
        clippy::cast_precision_loss,
        reason = "an angle in 60,000ths of a degree fits in f64"
    )]
    let rotation = number(transform, "rot").map_or(0.0, |angle| angle as f64 / 60_000.0);
    let mut paths = properties
        .and_then(|properties| {
            if let Some(custom) = properties.child(A, "custGeom") {
                return custom_geometry(custom, width, height);
            }
            let preset = properties.child(A, "prstGeom")?;
            let name = preset.attribute(Namespace::None, "prst").unwrap_or("rect");
            preset_geometry(name, preset.child(A, "avLst"), width, height)
        })
        .unwrap_or_else(|| rectangle_paths(width, height));
    flip_paths(
        &mut paths,
        width,
        height,
        flipped("flipH"),
        flipped("flipV"),
    );
    let style = shape.child(own, "style");
    let (fill, line) = paint_of(properties, style, theme);
    let style_colour = |name: &str| {
        let reference = style?.child(A, name)?;
        (reference.attribute(Namespace::None, "idx") != Some("0"))
            .then(|| colour_in(reference, theme))
            .flatten()
    };
    let text_colour = style_colour("fontRef").unwrap_or_else(|| "000000".to_owned());
    let text = if own == DSP {
        shape
            .child(DSP, "txBody")
            .map(|body| drawing_text_lines(body, &text_colour, theme))
            .unwrap_or_default()
    } else {
        shape
            .child(WPS, "txbx")
            .and_then(|text_box| text_box.child(W, "txbxContent"))
            .map(|content| text_lines(content, &text_colour))
            .unwrap_or_default()
    };
    Some(DrawnShape {
        x,
        y,
        width,
        height,
        rotation,
        paths,
        fill,
        line,
        text,
    })
}

/// The fill and the line a shape's properties name, else those its style refers to.
fn paint_of(
    properties: Option<&Element>,
    style: Option<&Element>,
    theme: &Theme,
) -> (Option<String>, Option<(String, i64)>) {
    let number = |element: Option<&Element>, name: &str| {
        element
            .and_then(|element| element.attribute(Namespace::None, name))
            .and_then(|value| value.parse::<i64>().ok())
    };
    let style_colour = |name: &str| {
        let reference = style?.child(A, name)?;
        (reference.attribute(Namespace::None, "idx") != Some("0"))
            .then(|| colour_in(reference, theme))
            .flatten()
    };
    let fill = match properties.and_then(|properties| {
        properties.elements().find(|fill| {
            matches!(
                fill.name.as_str(),
                "noFill" | "solidFill" | "gradFill" | "pattFill" | "blipFill"
            )
        })
    }) {
        Some(fill) => match fill.name.as_str() {
            "solidFill" => colour_in(fill, theme),
            "gradFill" => fill
                .descendant(A, "gs")
                .and_then(|stop| colour_in(stop, theme)),
            "pattFill" => fill
                .child(A, "fgClr")
                .and_then(|colour| colour_in(colour, theme)),
            _ => None,
        },
        None => style_colour("fillRef"),
    };
    let line = match properties.and_then(|properties| properties.child(A, "ln")) {
        Some(line) if line.child(A, "noFill").is_some() => None,
        Some(line) => {
            let colour = line
                .child(A, "solidFill")
                .and_then(|fill| colour_in(fill, theme))
                .or_else(|| style_colour("lnRef"));
            let width = number(Some(line), "w").unwrap_or(DEFAULT_LINE_EMU);
            colour.map(|colour| (colour, width))
        }
        None => style_colour("lnRef").map(|colour| (colour, DEFAULT_LINE_EMU)),
    };
    (fill, line)
}

/// The paragraphs of a `DrawingML` text body as centred lines, each in the size, colour and
/// weight of its first run; an empty paragraph stays as an empty line.
fn drawing_text_lines(body: &Element, default_colour: &str, theme: &Theme) -> Vec<TextLine> {
    body.elements()
        .filter(|paragraph| paragraph.is(A, "p"))
        .map(|paragraph| {
            let runs: Vec<&Element> = paragraph.elements().filter(|run| run.is(A, "r")).collect();
            let text: String = runs
                .iter()
                .filter_map(|run| run.child(A, "t"))
                .map(Element::text)
                .collect();
            let properties = runs
                .first()
                .and_then(|run| run.child(A, "rPr"))
                .or_else(|| paragraph.child(A, "endParaRPr"));
            let attribute = |name: &str| {
                properties.and_then(|properties| properties.attribute(Namespace::None, name))
            };
            TextLine {
                text,
                size_points: attribute("sz")
                    .and_then(|size| size.parse::<f64>().ok())
                    .map_or(18.0, |hundredths| (hundredths / 100.0).clamp(1.0, 400.0)),
                colour: properties
                    .and_then(|properties| properties.child(A, "solidFill"))
                    .and_then(|fill| colour_in(fill, theme))
                    .unwrap_or_else(|| default_colour.to_owned()),
                bold: matches!(attribute("b"), Some("1" | "true")),
                italic: matches!(attribute("i"), Some("1" | "true")),
            }
        })
        .collect()
}

/// The paragraphs of a shape's text as centred lines, each in the size, colour and weight of
/// its first run.
fn text_lines(content: &Element, default_colour: &str) -> Vec<TextLine> {
    content
        .elements()
        .filter(|paragraph| paragraph.is(W, "p"))
        .map(|paragraph| {
            let runs: Vec<&Element> = paragraph.elements().filter(|run| run.is(W, "r")).collect();
            let text: String = runs
                .iter()
                .flat_map(|run| run.elements())
                .filter(|element| element.is(W, "t"))
                .map(Element::text)
                .collect();
            let properties = runs.first().and_then(|run| run.child(W, "rPr"));
            let value = |name: &str| {
                properties
                    .and_then(|properties| properties.child(W, name))
                    .and_then(|element| element.attribute(W, "val"))
            };
            let on = |name: &str| {
                properties
                    .and_then(|properties| properties.child(W, name))
                    .is_some_and(|element| {
                        !matches!(element.attribute(W, "val"), Some("0" | "false"))
                    })
            };
            TextLine {
                text,
                size_points: value("sz")
                    .and_then(|size| size.parse::<f64>().ok())
                    .map_or(11.0, |half_points| (half_points / 2.0).clamp(1.0, 400.0)),
                colour: value("color")
                    .filter(|colour| {
                        colour.len() == 6
                            && colour
                                .chars()
                                .all(|character| character.is_ascii_hexdigit())
                    })
                    .map_or_else(|| default_colour.to_owned(), str::to_ascii_uppercase),
                bold: on("b"),
                italic: on("i"),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
