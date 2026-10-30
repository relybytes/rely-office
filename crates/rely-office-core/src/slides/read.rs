// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Reading a pptx into [`Presentation`]: each slide over its layout and master, the shapes of
//! the layout and master drawn behind the slide's own, placeholders placed and styled as their
//! layout and master place and style them.

use crate::geometry::{custom_geometry, flip_paths, preset_geometry};
use std::collections::HashMap;
use std::rc::Rc;

use super::show::{read_animations, read_transition};
use super::text::{BulletStyle, LEVELS, LevelStyle, list_styles, number_label};
use super::{
    Bullet, Frame, Gradient, GradientKind, GradientStop, Paint, Presentation, ShapeContent,
    ShapeGeometry, ShapeLine, Slide, SlideParagraph, SlideRun, SlideShape, SlideTable,
    SlideTableCell, SlideTableRow, TextAnchor, TextBody,
};
use crate::archive::{ZipPackage, open_package, read_member_bytes, read_text_member};
use crate::chart::{chart_picture, chart_reference, no_cells};
use crate::document::{
    Alignment, Media, image_pixel_size, smart_art_drawing, smart_art_svg, sniff_image_type,
};
use crate::number_format::NumberLocale;
use crate::presentation::slide_parts;
use crate::relationships::{Relationship, main_part_path, read_relationships};
use crate::theme::{Theme, colour_in};
use crate::xml_tree::{Element, Namespace, parse_tree};
use crate::{DocumentFormat, Limits, OfficeError, detect_format};

const A: Namespace = Namespace::Drawing;
const P: Namespace = Namespace::Presentation;
const R: Namespace = Namespace::Relationships;
const MAIN_PART: &str = "ppt/presentation.xml";
/// A 4:3 slide, `PowerPoint`'s size when the presentation names none.
const DEFAULT_SIZE: (i64, i64) = (9_144_000, 6_858_000);
/// The largest slide side `PowerPoint` allows, 56 inches.
const MAX_SIDE: i64 = 51_206_400;
const MAX_SLIDES: usize = 2_000;
const MAX_SHAPES: usize = 5_000;
/// Group shapes nest at most this deep.
const MAX_GROUP_DEPTH: usize = 32;
/// The text insets `PowerPoint` gives a shape that names none: 0.1 inch beside, 0.05 above
/// and below.
const DEFAULT_INSETS: [i64; 4] = [91_440, 45_720, 91_440, 45_720];
/// Text of no style is 18 points.
const DEFAULT_SIZE_HUNDREDTHS: u32 = 1_800;
const DEFAULT_LINE_EMU: i64 = 9_525;
/// A pixel at 96 to the inch.
const PIXEL_EMU: f64 = 9_525.0;
/// No gradient blends more colours than this.
const MAX_GRADIENT_STOPS: usize = 64;

/// A layout or a master, with what its shapes need.
struct Template {
    root: Element,
    relationships: Vec<Relationship>,
}

/// What a placeholder is, as `p:ph` names it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Placeholder {
    kind: String,
    index: Option<String>,
}

impl Placeholder {
    fn of(shape: &Element) -> Option<Self> {
        let placeholder = shape
            .elements()
            .find(|child| child.name.starts_with("nv"))?
            .child(P, "nvPr")?
            .child(P, "ph")?;
        Some(Self {
            kind: placeholder
                .attribute(Namespace::None, "type")
                .unwrap_or("body")
                .to_owned(),
            index: placeholder
                .attribute(Namespace::None, "idx")
                .map(str::to_owned),
        })
    }

    /// The kind a master's placeholder answers to: a centred title is a title, a subtitle is
    /// body text.
    fn family(&self) -> &str {
        match self.kind.as_str() {
            "ctrTitle" | "title" => "title",
            "subTitle" | "obj" | "body" => "body",
            other => other,
        }
    }

    fn is_title(&self) -> bool {
        self.family() == "title"
    }
}

/// Read a pptx as slides to draw.
///
/// # Errors
///
/// [`OfficeError::UnsupportedFormat`] for anything but a pptx, [`OfficeError::TooLarge`] past the
/// input or member limits, [`OfficeError::DamagedContainer`] and [`OfficeError::MalformedXml`]
/// for a damaged package.
pub fn read_presentation(bytes: &[u8], limits: &Limits) -> Result<Presentation, OfficeError> {
    if bytes.len() as u64 > limits.max_input_bytes {
        return Err(OfficeError::TooLarge {
            what: "input".to_owned(),
            limit: limits.max_input_bytes,
        });
    }
    let format = detect_format(bytes, None);
    if format == DocumentFormat::Odp {
        return crate::opendocument::read_odp(bytes, limits);
    }
    if format == DocumentFormat::Ppt {
        let mut presentation = crate::ppt::read_ppt_presentation(bytes, limits)?;
        presentation.holds_macros = crate::holds_macros(bytes, limits);
        return Ok(presentation);
    }
    if format != DocumentFormat::Pptx {
        return Err(OfficeError::UnsupportedFormat(format));
    }
    let mut package = open_package(bytes)?;
    let main_part = main_part_path(&mut package, limits, MAIN_PART)?;
    let main_root = read_text_member(&mut package, &main_part, limits)?
        .and_then(|xml| parse_tree(&xml, |_, _| false).ok());
    let (width_emu, height_emu) = main_root
        .as_ref()
        .and_then(|root| {
            let size = root.child(P, "sldSz")?;
            let dimension = |name: &str| {
                size.attribute(Namespace::None, name)?
                    .parse::<i64>()
                    .ok()
                    .filter(|side| (914_400 / 10..=MAX_SIDE).contains(side))
            };
            Some((dimension("cx")?, dimension("cy")?))
        })
        .unwrap_or(DEFAULT_SIZE);

    let mut reader = Reader {
        package: &mut package,
        limits,
        templates: HashMap::new(),
        themes: HashMap::new(),
        media: Vec::new(),
        media_by_target: HashMap::new(),
        media_bytes: 0,
        remaining_chars: limits.max_text_chars,
        truncated: false,
        default_text_style: main_root.and_then(|root| root.child(P, "defaultTextStyle").cloned()),
    };
    let parts = slide_parts(reader.package, &main_part, limits)?;
    let mut slides = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        if slides.len() >= MAX_SLIDES {
            reader.truncated = true;
            break;
        }
        if let Some(slide) = reader.slide(part, index + 1)? {
            slides.push(slide);
        }
    }
    let truncated = reader.truncated;
    let macro_project = crate::read_macro_project(bytes, limits);
    Ok(Presentation {
        width_emu,
        height_emu,
        slides,
        media: reader.media,
        truncated,
        holds_macros: macro_project.is_some(),
        macro_project,
    })
}

struct Reader<'a, 'p> {
    package: &'a mut ZipPackage<'p>,
    limits: &'a Limits,
    templates: HashMap<String, Rc<Template>>,
    themes: HashMap<String, Rc<Theme>>,
    media: Vec<Media>,
    media_by_target: HashMap<String, Option<usize>>,
    media_bytes: u64,
    remaining_chars: usize,
    truncated: bool,
    /// The presentation's `p:defaultTextStyle`, the style of text outside placeholders.
    default_text_style: Option<Element>,
}

/// The parts a slide is drawn over.
struct Context {
    layout: Option<Rc<Template>>,
    master: Option<Rc<Template>>,
    theme: Rc<Theme>,
    number: usize,
}

impl Reader<'_, '_> {
    fn template(&mut self, path: &str) -> Result<Option<Rc<Template>>, OfficeError> {
        if let Some(known) = self.templates.get(path) {
            return Ok(Some(Rc::clone(known)));
        }
        let Some(xml) = read_text_member(self.package, path, self.limits)? else {
            return Ok(None);
        };
        let template = Rc::new(Template {
            root: parse_tree(&xml, keeps_whitespace)?,
            relationships: read_relationships(self.package, path, self.limits)?,
        });
        self.templates.insert(path.to_owned(), Rc::clone(&template));
        Ok(Some(template))
    }

    fn theme(&mut self, master: Option<&Template>) -> Rc<Theme> {
        let Some(path) = master.and_then(|master| related(&master.relationships, "theme")) else {
            return Rc::new(Theme::default());
        };
        if let Some(known) = self.themes.get(&path) {
            return Rc::clone(known);
        }
        let theme = read_text_member(self.package, &path, self.limits)
            .ok()
            .flatten()
            .map(|xml| Theme::parse(&xml))
            .unwrap_or_default();
        let theme = Rc::new(theme);
        self.themes.insert(path, Rc::clone(&theme));
        theme
    }

    fn slide(&mut self, part: &str, number: usize) -> Result<Option<Slide>, OfficeError> {
        let Some(xml) = read_text_member(self.package, part, self.limits)? else {
            return Ok(None);
        };
        let root = parse_tree(&xml, keeps_whitespace)?;
        drop(xml);
        let relationships = read_relationships(self.package, part, self.limits)?;
        let layout = match related(&relationships, "slideLayout") {
            Some(path) => self.template(&path)?,
            None => None,
        };
        let master = match layout
            .as_ref()
            .and_then(|layout| related(&layout.relationships, "slideMaster"))
        {
            Some(path) => self.template(&path)?,
            None => None,
        };
        let theme = self.theme(master.as_deref());
        let context = Context {
            layout,
            master,
            theme,
            number,
        };

        let mut background = None;
        for (part, part_relationships) in [
            Some((&root, relationships.as_slice())),
            context
                .layout
                .as_ref()
                .map(|layout| (&layout.root, layout.relationships.as_slice())),
            context
                .master
                .as_ref()
                .map(|master| (&master.root, master.relationships.as_slice())),
        ]
        .into_iter()
        .flatten()
        {
            if let Some(paint) = self.background(part, part_relationships, &context.theme) {
                background = Some(paint);
                break;
            }
        }
        let mut slide = Slide {
            background,
            hidden: root.attribute(Namespace::None, "show") == Some("0"),
            transition: read_transition(&root),
            ..Slide::default()
        };
        let shows = |part: &Element| part.attribute(Namespace::None, "showMasterSp") != Some("0");
        let layout_shows = context
            .layout
            .as_ref()
            .is_none_or(|layout| shows(&layout.root));
        if shows(&root) {
            if layout_shows && let Some(master) = context.master.clone() {
                self.shapes_of(
                    &master.root,
                    &master.relationships,
                    &context,
                    false,
                    &mut slide.shapes,
                );
            }
            if let Some(layout) = context.layout.clone() {
                self.shapes_of(
                    &layout.root,
                    &layout.relationships,
                    &context,
                    false,
                    &mut slide.shapes,
                );
            }
        }
        self.shapes_of(&root, &relationships, &context, true, &mut slide.shapes);
        slide.animations = read_animations(&root, &slide.shapes);
        slide.notes = self.notes(&relationships, &context)?;
        Ok(Some(slide))
    }

    /// The speaker notes of a slide: the text of the body placeholder of its notes page.
    fn notes(
        &mut self,
        relationships: &[Relationship],
        context: &Context,
    ) -> Result<Option<TextBody>, OfficeError> {
        let Some(path) = related(relationships, "notesSlide") else {
            return Ok(None);
        };
        let Some(xml) = read_text_member(self.package, &path, self.limits)? else {
            return Ok(None);
        };
        let root = parse_tree(&xml, keeps_whitespace)?;
        let body = root
            .child(P, "cSld")
            .and_then(|slide| slide.child(P, "spTree"))
            .and_then(|tree| {
                tree.elements().find(|shape| {
                    shape.is(P, "sp")
                        && Placeholder::of(shape)
                            .is_some_and(|placeholder| placeholder.kind == "body")
                })
            })
            .and_then(|shape| shape.child(P, "txBody"));
        Ok(body
            .map(|body| self.text_body(body, None, &[], None, context))
            .filter(|body| {
                body.paragraphs
                    .iter()
                    .any(|paragraph| paragraph.runs.iter().any(|run| !run.text.trim().is_empty()))
            }))
    }

    /// The shapes of a part's tree; a layout's or a master's placeholders are templates for the
    /// slide's, not shapes of their own.
    fn shapes_of(
        &mut self,
        part: &Element,
        relationships: &[Relationship],
        context: &Context,
        own: bool,
        shapes: &mut Vec<SlideShape>,
    ) {
        let Some(tree) = part
            .child(P, "cSld")
            .and_then(|slide| slide.child(P, "spTree"))
        else {
            return;
        };
        let transform = Transform::identity();
        self.group(tree, relationships, context, own, &transform, 0, shapes);
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the walk carries the part, its context and the transform of the groups above"
    )]
    fn group(
        &mut self,
        group: &Element,
        relationships: &[Relationship],
        context: &Context,
        own: bool,
        transform: &Transform,
        depth: usize,
        shapes: &mut Vec<SlideShape>,
    ) {
        for element in group.elements() {
            if shapes.len() >= MAX_SHAPES {
                self.truncated = true;
                return;
            }
            if !own && Placeholder::of(element).is_some() {
                continue;
            }
            match (element.namespace, element.name.as_str()) {
                (P, "sp" | "cxnSp") => {
                    if let Some(shape) = self.shape(element, relationships, context, transform) {
                        shapes.push(placed(shape, element, own));
                    }
                }
                (P, "pic") => {
                    if let Some(shape) = self.picture(element, relationships, context, transform) {
                        shapes.push(placed(shape, element, own));
                    }
                }
                (P, "graphicFrame") => {
                    if let Some(shape) =
                        self.graphic_frame(element, relationships, context, transform)
                    {
                        shapes.push(placed(shape, element, own));
                    }
                }
                (P, "grpSp") if depth < MAX_GROUP_DEPTH => {
                    let inner = element
                        .child(P, "grpSpPr")
                        .and_then(|properties| properties.child(A, "xfrm"))
                        .map_or_else(|| transform.clone(), |xfrm| transform.then_group(xfrm));
                    self.group(
                        element,
                        relationships,
                        context,
                        own,
                        &inner,
                        depth + 1,
                        shapes,
                    );
                }
                (Namespace::MarkupCompatibility, "AlternateContent") => {
                    if let Some(choice) = element
                        .child(Namespace::MarkupCompatibility, "Choice")
                        .or_else(|| element.child(Namespace::MarkupCompatibility, "Fallback"))
                    {
                        self.group(
                            choice,
                            relationships,
                            context,
                            own,
                            transform,
                            depth + 1,
                            shapes,
                        );
                    }
                }
                _ => {}
            }
        }
    }
}

/// The placeholders of the layout and the master a slide shape stands in for, nearest first.
fn templates_of<'c>(placeholder: Option<&Placeholder>, context: &'c Context) -> Vec<&'c Element> {
    let Some(placeholder) = placeholder else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for (template, by_index) in [(&context.layout, true), (&context.master, false)] {
        let Some(template) = template else { continue };
        let Some(tree) = template
            .root
            .child(P, "cSld")
            .and_then(|slide| slide.child(P, "spTree"))
        else {
            continue;
        };
        let candidates: Vec<(&Element, Placeholder)> = tree
            .elements()
            .filter_map(|shape| Placeholder::of(shape).map(|found| (shape, found)))
            .collect();
        let matched = candidates
            .iter()
            .find(|(_, candidate)| {
                by_index && placeholder.index.is_some() && candidate.index == placeholder.index
            })
            .or_else(|| {
                candidates
                    .iter()
                    .find(|(_, candidate)| candidate.kind == placeholder.kind)
            })
            .or_else(|| {
                candidates
                    .iter()
                    .find(|(_, candidate)| candidate.family() == placeholder.family())
            });
        if let Some((shape, _)) = matched {
            found.push(*shape);
        }
    }
    found
}

impl Reader<'_, '_> {
    fn shape(
        &mut self,
        shape: &Element,
        relationships: &[Relationship],
        context: &Context,
        transform: &Transform,
    ) -> Option<SlideShape> {
        let placeholder = Placeholder::of(shape);
        let templates = templates_of(placeholder.as_ref(), context);
        let properties = shape.child(P, "spPr");
        let (frame, rotation, flipped) = [properties]
            .into_iter()
            .chain(templates.iter().map(|template| template.child(P, "spPr")))
            .flatten()
            .find_map(|properties| properties.child(A, "xfrm").and_then(read_frame))?;
        let frame = transform.apply(frame);
        let geometry = shape_geometry(properties, frame, flipped);
        let style = shape.child(P, "style");
        let theme = &context.theme;
        let fill = match properties.map_or(Fill::Unset, |properties| {
            self.paint_of(properties, relationships, theme)
        }) {
            Fill::Unset => style_colour(style, "fillRef", theme).map(Paint::Solid),
            Fill::Named(paint) => paint,
        };
        let line = properties
            .and_then(|properties| properties.child(A, "ln"))
            .map_or_else(
                || {
                    style_colour(style, "lnRef", theme).map(|colour| ShapeLine {
                        colour,
                        width_emu: DEFAULT_LINE_EMU,
                    })
                },
                |line| line_of(line, theme),
            );
        let shape_content = match shape.child(P, "txBody") {
            Some(body) => {
                let text = self.text_body(body, placeholder.as_ref(), &templates, style, context);
                if text
                    .paragraphs
                    .iter()
                    .all(|paragraph| paragraph.runs.is_empty())
                    && placeholder.is_some()
                {
                    ShapeContent::None
                } else {
                    ShapeContent::Text(text)
                }
            }
            None => ShapeContent::None,
        };
        if shape_content == ShapeContent::None && fill.is_none() && line.is_none() {
            return None;
        }
        Some(SlideShape {
            id: None,
            inherited: false,
            frame,
            rotation,
            geometry,
            fill,
            line,
            content: shape_content,
        })
    }

    fn picture(
        &mut self,
        picture: &Element,
        relationships: &[Relationship],
        context: &Context,
        transform: &Transform,
    ) -> Option<SlideShape> {
        let placeholder = Placeholder::of(picture);
        let templates = templates_of(placeholder.as_ref(), context);
        let (frame, rotation, _) = [picture.child(P, "spPr")]
            .into_iter()
            .chain(templates.iter().map(|template| template.child(P, "spPr")))
            .flatten()
            .find_map(|properties| properties.child(A, "xfrm").and_then(read_frame))?;
        let embed = picture
            .descendant(A, "blip")
            .and_then(|blip| blip.attribute(R, "embed"))?;
        let media = self.image(relationships, embed)?;
        Some(SlideShape {
            id: None,
            inherited: false,
            frame: transform.apply(frame),
            rotation,
            geometry: ShapeGeometry::Rectangle,
            fill: None,
            line: picture
                .child(P, "spPr")
                .and_then(|properties| properties.child(A, "ln"))
                .and_then(|line| line_of(line, &context.theme)),
            content: ShapeContent::Picture { media },
        })
    }

    fn graphic_frame(
        &mut self,
        frame: &Element,
        relationships: &[Relationship],
        context: &Context,
        transform: &Transform,
    ) -> Option<SlideShape> {
        let (bounds, _, _) = frame.child(P, "xfrm").and_then(read_frame)?;
        if let Some(chart) = chart_reference(frame) {
            let bounds = transform.apply(bounds);
            let media = self.chart(chart.attribute(R, "id")?, relationships, context, bounds)?;
            return Some(picture_shape(bounds, media));
        }
        if let Some(ids) = frame.descendant(Namespace::Diagram, "relIds") {
            let bounds = transform.apply(bounds);
            let media = self.smart_art(ids, relationships, context, bounds)?;
            return Some(picture_shape(bounds, media));
        }
        let table = frame.descendant(A, "tbl")?;
        let column_widths_emu: Vec<i64> = table
            .child(A, "tblGrid")
            .map(|grid| {
                grid.elements()
                    .filter(|column| column.is(A, "gridCol"))
                    .map(|column| emu(column.attribute(Namespace::None, "w")).unwrap_or(0))
                    .collect()
            })
            .unwrap_or_default();
        let mut rows = Vec::new();
        for row in table.elements().filter(|row| row.is(A, "tr")) {
            let mut cells = Vec::new();
            for cell in row.elements().filter(|cell| cell.is(A, "tc")) {
                let span = |name: &str| {
                    cell.attribute(Namespace::None, name)
                        .and_then(|value| value.parse::<u32>().ok())
                        .unwrap_or(1)
                        .clamp(1, 1_000)
                };
                let covered = ["hMerge", "vMerge"].iter().any(|name| {
                    cell.attribute(Namespace::None, name)
                        .is_some_and(|value| matches!(value, "1" | "true"))
                });
                let properties = cell.child(A, "tcPr");
                let body = cell.child(A, "txBody").map_or_else(
                    || TextBody {
                        paragraphs: Vec::new(),
                        anchor: TextAnchor::Top,
                        insets_emu: DEFAULT_INSETS,
                        wrap: true,
                    },
                    |body| self.text_body(body, None, &[], None, context),
                );
                let insets = properties.map_or(DEFAULT_INSETS, |properties| {
                    let inset = |name: &str, default: i64| {
                        emu(properties.attribute(Namespace::None, name)).unwrap_or(default)
                    };
                    [
                        inset("marL", DEFAULT_INSETS[0]),
                        inset("marT", DEFAULT_INSETS[1]),
                        inset("marR", DEFAULT_INSETS[2]),
                        inset("marB", DEFAULT_INSETS[3]),
                    ]
                });
                let anchor = properties
                    .and_then(|properties| properties.attribute(Namespace::None, "anchor"))
                    .map_or(TextAnchor::Top, anchor_of);
                cells.push(SlideTableCell {
                    body: TextBody {
                        insets_emu: insets,
                        anchor,
                        ..body
                    },
                    fill: properties.and_then(|properties| {
                        fill_of(properties, &context.theme)
                            .paint()
                            .and_then(|paint| paint.colour().map(str::to_owned))
                    }),
                    col_span: span("gridSpan"),
                    row_span: span("rowSpan"),
                    covered,
                });
            }
            rows.push(SlideTableRow {
                height_emu: emu(row.attribute(Namespace::None, "h")).unwrap_or(0),
                cells,
            });
        }
        Some(SlideShape {
            id: None,
            inherited: false,
            frame: transform.apply(bounds),
            rotation: 0.0,
            geometry: ShapeGeometry::Rectangle,
            fill: None,
            line: None,
            content: ShapeContent::Table(SlideTable {
                column_widths_emu,
                rows,
            }),
        })
    }

    /// The text of a shape, each paragraph styled from its own properties out to the master's
    /// text styles and the theme.
    #[expect(
        clippy::too_many_lines,
        reason = "one walk resolves the body, its level styles and every paragraph in order"
    )]
    fn text_body(
        &mut self,
        body: &Element,
        placeholder: Option<&Placeholder>,
        templates: &[&Element],
        style: Option<&Element>,
        context: &Context,
    ) -> TextBody {
        let theme = &context.theme;
        let body_properties: Vec<&Element> = [body.child(A, "bodyPr")]
            .into_iter()
            .chain(templates.iter().map(|template| {
                template
                    .child(P, "txBody")
                    .and_then(|body| body.child(A, "bodyPr"))
            }))
            .flatten()
            .collect();
        let body_attribute = |name: &str| {
            body_properties
                .iter()
                .find_map(|properties| properties.attribute(Namespace::None, name))
        };
        let anchor = body_attribute("anchor").map_or(TextAnchor::Top, anchor_of);
        let insets = [("lIns", 0), ("tIns", 1), ("rIns", 2), ("bIns", 3)]
            .map(|(name, side)| emu(body_attribute(name)).unwrap_or(DEFAULT_INSETS[side]));
        let wrap = body_attribute("wrap") != Some("none");
        let autofit = body_properties
            .first()
            .and_then(|properties| properties.child(A, "normAutofit"));
        let scale = |name: &str| {
            autofit
                .and_then(|autofit| autofit.attribute(Namespace::None, name))
                .and_then(|value| value.parse::<f64>().ok())
                .map_or(1.0, |value| (value / 100_000.0).clamp(0.1, 1.0))
        };
        let font_scale = scale("fontScale");
        let line_reduction = autofit
            .and_then(|autofit| autofit.attribute(Namespace::None, "lnSpcReduction"))
            .and_then(|value| value.parse::<u32>().ok())
            .map_or(0, |value| value / 1_000);

        // The level styles from nearest to furthest.
        let mut layers: Vec<Vec<LevelStyle>> = vec![list_styles(body.child(A, "lstStyle"), theme)];
        for template in templates {
            layers.push(list_styles(
                template
                    .child(P, "txBody")
                    .and_then(|body| body.child(A, "lstStyle")),
                theme,
            ));
        }
        if let Some(colour) = style
            .and_then(|style| style.child(A, "fontRef"))
            .and_then(|reference| colour_in(reference, theme))
        {
            layers.push(vec![
                LevelStyle {
                    colour: Some(colour),
                    ..LevelStyle::default()
                };
                LEVELS
            ]);
        }
        let master_styles = context
            .master
            .as_ref()
            .and_then(|master| master.root.child(P, "txStyles"));
        let outer = match placeholder {
            Some(placeholder) if placeholder.is_title() => {
                master_styles.and_then(|styles| styles.child(P, "titleStyle"))
            }
            Some(placeholder) if placeholder.family() == "body" => {
                master_styles.and_then(|styles| styles.child(P, "bodyStyle"))
            }
            Some(_) => master_styles.and_then(|styles| styles.child(P, "otherStyle")),
            None => self.default_text_style.as_ref(),
        };
        layers.push(list_styles(outer, theme));
        let defaults = LevelStyle {
            size: Some(DEFAULT_SIZE_HUNDREDTHS),
            bold: Some(false),
            italic: Some(false),
            underline: Some(false),
            strike: Some(false),
            colour: theme.scheme("tx1"),
            family: Some(if placeholder.is_some_and(Placeholder::is_title) {
                theme.major_font.clone()
            } else {
                theme.minor_font.clone()
            }),
            align: Some(Alignment::Left),
            bullet: Some(BulletStyle::None),
            margin_left: Some(0),
            indent: Some(0),
            line_spacing: Some(100),
            space_before: Some(0),
            space_after: Some(0),
        };

        let mut paragraphs = Vec::new();
        let mut counters = [0_u32; LEVELS];
        for paragraph in body.elements().filter(|paragraph| paragraph.is(A, "p")) {
            let own = paragraph.child(A, "pPr");
            let level = own
                .and_then(|properties| properties.attribute(Namespace::None, "lvl"))
                .and_then(|level| level.parse::<usize>().ok())
                .unwrap_or(0)
                .min(LEVELS - 1);
            let mut resolved = own
                .map(|properties| LevelStyle::paragraph(properties, theme))
                .unwrap_or_default();
            for layer in &layers {
                resolved = resolved.or(&layer[level]);
            }
            resolved = resolved.or(&defaults);
            let runs = self.runs(paragraph, &resolved, theme, font_scale, context.number);
            let bullet = match resolved.bullet.clone() {
                _ if runs.is_empty() => None,
                Some(BulletStyle::Character(character)) => Some(Bullet::Character(character)),
                Some(BulletStyle::Number { scheme, start }) => {
                    counters[level] = if counters[level] == 0 {
                        start
                    } else {
                        counters[level] + 1
                    };
                    Some(Bullet::Number(number_label(&scheme, counters[level])))
                }
                Some(BulletStyle::None) | None => None,
            };
            if !matches!(bullet, Some(Bullet::Number(_))) && !runs.is_empty() {
                counters[level] = 0;
            }
            for deeper in &mut counters[level + 1..] {
                *deeper = 0;
            }
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "sizes are capped far below u32::MAX"
            )]
            let scaled = |value: u32| (f64::from(value) * font_scale).round() as u32;
            paragraphs.push(SlideParagraph {
                level: u8::try_from(level).unwrap_or(0),
                align: resolved.align.unwrap_or(Alignment::Left),
                bullet,
                margin_left_emu: resolved.margin_left.unwrap_or(0),
                indent_emu: resolved.indent.unwrap_or(0),
                line_spacing_percent: resolved
                    .line_spacing
                    .unwrap_or(100)
                    .saturating_sub(line_reduction)
                    .max(10),
                space_before: scaled(resolved.space_before.unwrap_or(0)),
                space_after: scaled(resolved.space_after.unwrap_or(0)),
                runs,
            });
        }
        TextBody {
            paragraphs,
            anchor,
            insets_emu: insets,
            wrap,
        }
    }

    fn runs(
        &mut self,
        paragraph: &Element,
        style: &LevelStyle,
        theme: &Theme,
        font_scale: f64,
        number: usize,
    ) -> Vec<SlideRun> {
        let mut runs = Vec::new();
        for element in paragraph
            .elements()
            .filter(|element| element.namespace == A)
        {
            let (text, line_break) = match element.name.as_str() {
                "r" => (
                    element.child(A, "t").map(Element::text).unwrap_or_default(),
                    false,
                ),
                "fld" => (
                    if element.attribute(Namespace::None, "type") == Some("slidenum") {
                        number.to_string()
                    } else {
                        element.child(A, "t").map(Element::text).unwrap_or_default()
                    },
                    false,
                ),
                "br" => (String::new(), true),
                _ => continue,
            };
            let taken: String = text.chars().take(self.remaining_chars).collect();
            let count = taken.chars().count();
            if count < text.chars().count() {
                self.truncated = true;
            }
            self.remaining_chars -= count;
            if taken.is_empty() && !line_break {
                continue;
            }
            let run = element
                .child(A, "rPr")
                .map(|properties| LevelStyle::run(properties, theme))
                .unwrap_or_default()
                .or(style);
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "sizes are capped far below u32::MAX"
            )]
            let size = (f64::from(run.size.unwrap_or(DEFAULT_SIZE_HUNDREDTHS)) * font_scale).round()
                as u32;
            runs.push(SlideRun {
                text: taken,
                size: size.max(100),
                bold: run.bold.unwrap_or(false),
                italic: run.italic.unwrap_or(false),
                underline: run.underline.unwrap_or(false),
                strike: run.strike.unwrap_or(false),
                colour: run.colour.unwrap_or_else(|| "000000".to_owned()),
                family: run.family.unwrap_or_else(|| theme.minor_font.clone()),
                line_break,
            });
        }
        runs
    }

    /// A chart of a slide drawn as an SVG image the size of its frame.
    fn chart(
        &mut self,
        id: &str,
        relationships: &[Relationship],
        context: &Context,
        frame: Frame,
    ) -> Option<usize> {
        let target = relationships
            .iter()
            .find(|relationship| relationship.id == id && !relationship.external)?
            .target
            .clone();
        let xml = read_text_member(self.package, &target, self.limits)
            .ok()
            .flatten()?;
        let (bytes, _) = chart_picture(
            &xml,
            &context.theme,
            frame.width,
            frame.height,
            &no_cells,
            NumberLocale::default(),
        )?;
        if self.media.len() >= self.limits.max_images {
            self.truncated = true;
            return None;
        }
        let size = bytes.len() as u64;
        if self.media_bytes.saturating_add(size) > self.limits.max_total_image_bytes {
            self.truncated = true;
            return None;
        }
        self.media_bytes += size;
        self.media.push(Media {
            content_type: "image/svg+xml",
            bytes,
        });
        Some(self.media.len() - 1)
    }

    /// A `SmartArt` as a picture, drawn from the drawing part its data model names.
    fn smart_art(
        &mut self,
        ids: &Element,
        relationships: &[Relationship],
        context: &Context,
        frame: Frame,
    ) -> Option<usize> {
        let target = |id: &str| {
            relationships
                .iter()
                .find(|relationship| relationship.id == id && !relationship.external)
                .map(|relationship| relationship.target.clone())
        };
        let data = target(ids.attribute(R, "dm")?)?;
        let model = read_text_member(self.package, &data, self.limits)
            .ok()
            .flatten()?;
        let drawing = smart_art_drawing(&model).and_then(target).or_else(|| {
            let mut drawings = relationships.iter().filter(|relationship| {
                !relationship.external && relationship.is_kind("diagramDrawing")
            });
            match (drawings.next(), drawings.next()) {
                (Some(only), None) => Some(only.target.clone()),
                _ => None,
            }
        })?;
        let xml = read_text_member(self.package, &drawing, self.limits)
            .ok()
            .flatten()?;
        let bytes = smart_art_svg(&xml, &context.theme, frame.width, frame.height)?;
        let size = bytes.len() as u64;
        if self.media.len() >= self.limits.max_images
            || self.media_bytes.saturating_add(size) > self.limits.max_total_image_bytes
        {
            self.truncated = true;
            return None;
        }
        self.media_bytes += size;
        self.media.push(Media {
            content_type: "image/svg+xml",
            bytes,
        });
        Some(self.media.len() - 1)
    }

    /// The media index of an image of a part, loading it the first time.
    /// The fill a shape's properties name, a picture fill included.
    fn paint_of(
        &mut self,
        properties: &Element,
        relationships: &[Relationship],
        theme: &Theme,
    ) -> Fill {
        let Some(picture) = properties.elements().find(|fill| {
            matches!(
                fill.name.as_str(),
                "noFill" | "solidFill" | "gradFill" | "pattFill" | "blipFill" | "grpFill"
            )
        }) else {
            return Fill::Unset;
        };
        if picture.name != "blipFill" {
            return fill_of(properties, theme);
        }
        let Some(media) = picture
            .child(A, "blip")
            .and_then(|blip| blip.attribute(Namespace::Relationships, "embed"))
            .and_then(|id| self.image(relationships, id))
        else {
            return Fill::Named(None);
        };
        let tile_emu = picture.child(A, "tile").and_then(|tile| {
            let (width, height) = image_pixel_size(&self.media.get(media)?.bytes)?;
            let scale = |name: &str| {
                tile.attribute(Namespace::None, name)
                    .and_then(|value| value.parse::<f64>().ok())
                    .filter(|value| value.is_finite() && *value > 0.0)
                    .map_or(1.0, |value| value / 100_000.0)
            };
            let side = |pixels: u32, scale: f64| {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "an image is far below the range of i64 in EMU"
                )]
                let emu = (f64::from(pixels) * PIXEL_EMU * scale).round() as i64;
                emu.max(1)
            };
            Some((side(width, scale("sx")), side(height, scale("sy"))))
        });
        Fill::Named(Some(Paint::Picture { media, tile_emu }))
    }

    /// The background of a slide, a layout or a master.
    fn background(
        &mut self,
        part: &Element,
        relationships: &[Relationship],
        theme: &Theme,
    ) -> Option<Paint> {
        let background = part.child(P, "cSld")?.child(P, "bg")?;
        if let Some(properties) = background.child(P, "bgPr") {
            return self.paint_of(properties, relationships, theme).paint();
        }
        background
            .child(P, "bgRef")
            .and_then(|reference| colour_in(reference, theme))
            .map(Paint::Solid)
    }

    fn image(&mut self, relationships: &[Relationship], id: &str) -> Option<usize> {
        let target = relationships
            .iter()
            .find(|relationship| relationship.id == id && !relationship.external)?
            .target
            .clone();
        if let Some(known) = self.media_by_target.get(&target) {
            return *known;
        }
        let loaded = self.load_image(&target);
        self.media_by_target.insert(target, loaded);
        loaded
    }

    fn load_image(&mut self, target: &str) -> Option<usize> {
        if self.media.len() >= self.limits.max_images {
            self.truncated = true;
            return None;
        }
        let cap = self
            .limits
            .max_image_bytes
            .min(self.limits.max_member_bytes);
        let bytes = read_member_bytes(self.package, target, cap).ok()??;
        let size = bytes.len() as u64;
        if self.media_bytes.saturating_add(size) > self.limits.max_total_image_bytes {
            self.truncated = true;
            return None;
        }
        self.media_bytes += size;
        self.media.push(Media {
            content_type: sniff_image_type(&bytes),
            bytes,
        });
        Some(self.media.len() - 1)
    }
}

/// How the coordinates of a group's children map onto the slide.
#[derive(Debug, Clone, PartialEq)]
struct Transform {
    offset: (f64, f64),
    scale: (f64, f64),
}

impl Transform {
    fn identity() -> Self {
        Self {
            offset: (0.0, 0.0),
            scale: (1.0, 1.0),
        }
    }

    /// The transform of a group inside this one: its children's frame `chOff`/`chExt` maps
    /// onto its own `off`/`ext`.
    fn then_group(&self, xfrm: &Element) -> Self {
        let point = |name: &str, x: &str, y: &str| {
            xfrm.child(A, name).map(|point| {
                (
                    emu(point.attribute(Namespace::None, x)).unwrap_or(0),
                    emu(point.attribute(Namespace::None, y)).unwrap_or(0),
                )
            })
        };
        let (Some(offset), Some(extent)) = (point("off", "x", "y"), point("ext", "cx", "cy"))
        else {
            return self.clone();
        };
        let child_offset = point("chOff", "x", "y").unwrap_or(offset);
        let child_extent = point("chExt", "cx", "cy").unwrap_or(extent);
        #[expect(clippy::cast_precision_loss, reason = "slide coordinates fit in f64")]
        let ratio = |own: i64, child: i64| {
            if child == 0 {
                1.0
            } else {
                own as f64 / child as f64
            }
        };
        let scale = (
            ratio(extent.0, child_extent.0),
            ratio(extent.1, child_extent.1),
        );
        #[expect(clippy::cast_precision_loss, reason = "slide coordinates fit in f64")]
        let local = (
            offset.0 as f64 - child_offset.0 as f64 * scale.0,
            offset.1 as f64 - child_offset.1 as f64 * scale.1,
        );
        Self {
            offset: (
                self.offset.0 + local.0 * self.scale.0,
                self.offset.1 + local.1 * self.scale.1,
            ),
            scale: (self.scale.0 * scale.0, self.scale.1 * scale.1),
        }
    }

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        reason = "slide coordinates stay within a few hundred million"
    )]
    fn apply(&self, frame: Frame) -> Frame {
        Frame {
            x: (self.offset.0 + frame.x as f64 * self.scale.0).round() as i64,
            y: (self.offset.1 + frame.y as f64 * self.scale.1).round() as i64,
            width: (frame.width as f64 * self.scale.0).round() as i64,
            height: (frame.height as f64 * self.scale.1).round() as i64,
        }
    }
}

/// The frame, rotation and vertical flip of an `a:xfrm` or `p:xfrm`.
/// A picture the slide draws in a frame: a chart, a `SmartArt`.
fn picture_shape(frame: Frame, media: usize) -> SlideShape {
    SlideShape {
        id: None,
        inherited: false,
        frame,
        rotation: 0.0,
        geometry: ShapeGeometry::Rectangle,
        fill: None,
        line: None,
        content: ShapeContent::Picture { media },
    }
}

/// A shape with the id its slide gives it, or marked as drawn from the layout or the master.
fn placed(mut shape: SlideShape, element: &Element, own: bool) -> SlideShape {
    shape.inherited = !own;
    shape.id = own
        .then(|| {
            element
                .elements()
                .find(|child| child.namespace == P && child.name.starts_with("nv"))?
                .child(P, "cNvPr")?
                .attribute(Namespace::None, "id")?
                .parse()
                .ok()
        })
        .flatten();
    shape
}

fn read_frame(xfrm: &Element) -> Option<(Frame, f64, bool)> {
    let offset = xfrm.child(A, "off")?;
    let extent = xfrm.child(A, "ext")?;
    let frame = Frame {
        x: emu(offset.attribute(Namespace::None, "x"))?,
        y: emu(offset.attribute(Namespace::None, "y"))?,
        width: emu(extent.attribute(Namespace::None, "cx"))?.max(0),
        height: emu(extent.attribute(Namespace::None, "cy"))?.max(0),
    };
    let rotation = xfrm
        .attribute(Namespace::None, "rot")
        .and_then(|value| value.parse::<f64>().ok())
        .map_or(0.0, |value| (value / 60_000.0) % 360.0);
    let flipped = xfrm
        .attribute(Namespace::None, "flipV")
        .is_some_and(|value| matches!(value, "1" | "true"))
        != xfrm
            .attribute(Namespace::None, "flipH")
            .is_some_and(|value| matches!(value, "1" | "true"));
    Some((frame, rotation, flipped))
}

/// The outline a shape's properties name: a preset or a custom geometry, mirrored as its
/// frame says.
fn shape_geometry(properties: Option<&Element>, frame: Frame, flipped: bool) -> ShapeGeometry {
    let flips = properties
        .and_then(|properties| properties.child(A, "xfrm"))
        .map_or((false, false), |xfrm| {
            let flip = |name: &str| {
                xfrm.attribute(Namespace::None, name)
                    .is_some_and(|value| matches!(value, "1" | "true"))
            };
            (flip("flipH"), flip("flipV"))
        });
    let paths = if let Some(custom) =
        properties.and_then(|properties| properties.child(A, "custGeom"))
    {
        custom_geometry(custom, frame.width, frame.height)
    } else {
        let Some(preset) = properties.and_then(|properties| properties.child(A, "prstGeom")) else {
            return ShapeGeometry::Rectangle;
        };
        match preset.attribute(Namespace::None, "prst") {
            None | Some("rect") => return ShapeGeometry::Rectangle,
            Some("ellipse") => return ShapeGeometry::Ellipse,
            Some("line" | "straightConnector1") => return ShapeGeometry::Line { flipped },
            Some(name) => {
                preset_geometry(name, preset.child(A, "avLst"), frame.width, frame.height)
            }
        }
    };
    match paths {
        Some(mut paths) => {
            flip_paths(&mut paths, frame.width, frame.height, flips.0, flips.1);
            ShapeGeometry::Path(paths)
        }
        None => ShapeGeometry::Other,
    }
}

fn emu(value: Option<&str>) -> Option<i64> {
    value?
        .parse::<i64>()
        .ok()
        .map(|value| value.clamp(-MAX_SIDE * 4, MAX_SIDE * 4))
}

fn anchor_of(value: &str) -> TextAnchor {
    match value {
        "ctr" => TextAnchor::Middle,
        "b" => TextAnchor::Bottom,
        _ => TextAnchor::Top,
    }
}

/// What a shape's properties say about its fill.
enum Fill {
    /// They name no fill, so the shape's style decides.
    Unset,
    /// They name one, a paint or none at all.
    Named(Option<Paint>),
}

impl Fill {
    fn paint(self) -> Option<Paint> {
        match self {
            Self::Unset => None,
            Self::Named(paint) => paint,
        }
    }
}

/// The fill a shape's properties name without a picture: a colour, a gradient, a pattern as
/// its foreground; a picture fill counts as unset here.
fn fill_of(properties: &Element, theme: &Theme) -> Fill {
    properties
        .elements()
        .find_map(|fill| match fill.name.as_str() {
            "noFill" => Some(None),
            "solidFill" => Some(colour_in(fill, theme).map(Paint::Solid)),
            "gradFill" => Some(gradient_of(fill, theme)),
            "pattFill" => Some(
                fill.child(A, "fgClr")
                    .and_then(|colour| colour_in(colour, theme))
                    .map(Paint::Solid),
            ),
            _ => None,
        })
        .map_or(Fill::Unset, Fill::Named)
}

/// A `a:gradFill`: its stops in order, along its `a:lin` angle or out from the centre its
/// `a:path` names; one stop is a colour.
fn gradient_of(fill: &Element, theme: &Theme) -> Option<Paint> {
    let mut stops: Vec<GradientStop> = fill
        .child(A, "gsLst")
        .into_iter()
        .flat_map(Element::elements)
        .filter(|stop| stop.is(A, "gs"))
        .filter_map(|stop| {
            let offset = stop
                .attribute(Namespace::None, "pos")
                .and_then(|position| position.parse::<f64>().ok())
                .filter(|position| position.is_finite())
                .unwrap_or(0.0);
            Some(GradientStop {
                offset: (offset / 100_000.0).clamp(0.0, 1.0),
                colour: colour_in(stop, theme)?,
            })
        })
        .take(MAX_GRADIENT_STOPS)
        .collect();
    stops.sort_by(|first, second| first.offset.total_cmp(&second.offset));
    match stops.len() {
        0 => return None,
        1 => return stops.pop().map(|stop| Paint::Solid(stop.colour)),
        _ => {}
    }
    let kind = match fill.child(A, "path") {
        Some(path) => {
            let inset = |name: &str| {
                path.child(A, "fillToRect")
                    .and_then(|rect| rect.attribute(Namespace::None, name))
                    .and_then(|value| value.parse::<f64>().ok())
                    .filter(|value| value.is_finite())
                    .map_or(0.5, |value| value / 100_000.0)
            };
            GradientKind::Radial {
                centre: (
                    f64::midpoint(inset("l"), 1.0 - inset("r")).clamp(0.0, 1.0),
                    f64::midpoint(inset("t"), 1.0 - inset("b")).clamp(0.0, 1.0),
                ),
            }
        }
        None => GradientKind::Linear {
            angle: fill
                .child(A, "lin")
                .and_then(|line| line.attribute(Namespace::None, "ang"))
                .and_then(|angle| angle.parse::<f64>().ok())
                .filter(|angle| angle.is_finite())
                .map_or(90.0, |angle| (angle / 60_000.0).rem_euclid(360.0)),
        },
    };
    Some(Paint::Gradient(Gradient { stops, kind }))
}

fn line_of(line: &Element, theme: &Theme) -> Option<ShapeLine> {
    if line.child(A, "noFill").is_some() {
        return None;
    }
    let colour = line
        .child(A, "solidFill")
        .and_then(|fill| colour_in(fill, theme))?;
    Some(ShapeLine {
        colour,
        width_emu: emu(line.attribute(Namespace::None, "w"))
            .unwrap_or(DEFAULT_LINE_EMU)
            .clamp(0, 12_700 * 100),
    })
}

/// The colour a shape's style gives its fill or line, when its reference names one.
fn style_colour(style: Option<&Element>, name: &str, theme: &Theme) -> Option<String> {
    let reference = style?.child(A, name)?;
    if reference.attribute(Namespace::None, "idx") == Some("0") {
        return None;
    }
    colour_in(reference, theme)
}

fn related(relationships: &[Relationship], kind: &str) -> Option<String> {
    relationships
        .iter()
        .find(|relationship| !relationship.external && relationship.is_kind(kind))
        .map(|relationship| relationship.target.clone())
}

/// Text keeps its spaces; everywhere else whitespace is layout.
fn keeps_whitespace(namespace: Namespace, name: &str) -> bool {
    namespace == A && name == "t"
}

#[cfg(test)]
mod tests;
