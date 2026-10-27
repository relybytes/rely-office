// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! An odp read into the same [`Presentation`] a pptx gives: each page over its master page, its
//! frames, shapes, pictures and tables placed where they stand, the text styled through its
//! paragraph, text, presentation and graphic styles.

use std::collections::HashMap;

use super::styles::{FO, STYLE, Scope, Styles, TEXT, colour, points};
use crate::archive::{ZipPackage, open_package, read_member_bytes, read_text_member};
use crate::chart::{odf_chart_picture, odf_object_part};
use crate::document::{Alignment, LineSpacing, Media, ParagraphLook, RunLook, sniff_image_type};
use crate::slides::{
    Bullet, Frame, Presentation, ShapeContent, ShapeGeometry, ShapeLine, Slide, SlideParagraph,
    SlideRun, SlideShape, SlideTable, SlideTableCell, SlideTableRow, TextAnchor, TextBody,
};
use crate::xml_tree::{Element, Namespace, Node, parse_tree};
use crate::{Limits, OfficeError};

const OFFICE: Namespace = Namespace::OdfOffice;
const DRAW: Namespace = Namespace::OdfDraw;
const SVG: Namespace = Namespace::OdfSvg;
const TABLE: Namespace = Namespace::OdfTable;
const XLINK: Namespace = Namespace::XLink;
const PRESENTATION: Namespace = Namespace::OdfPresentation;
const EMU_PER_POINT: f64 = 12_700.0;
/// A 4:3 page, when the master page names no layout.
const DEFAULT_SIZE: (i64, i64) = (10_080_000, 7_560_000);
const MAX_SIDE: i64 = 51_206_400;
/// No length reaches past four of the largest pages.
const MAX_LENGTH: f64 = 204_825_600.0;
const MAX_SLIDES: usize = 2_000;
const MAX_SHAPES: usize = 5_000;
const MAX_GROUP_DEPTH: usize = 32;
const MAX_LEVEL: usize = 9;
/// The insets `LibreOffice` gives a text frame without its own: 0.25 cm beside, 0.125 cm above
/// and below.
const DEFAULT_INSETS: [i64; 4] = [90_000, 45_000, 90_000, 45_000];
const DEFAULT_SIZE_HUNDREDTHS: u32 = 1_800;

/// Read an odp as slides to draw.
pub(crate) fn read_odp(bytes: &[u8], limits: &Limits) -> Result<Presentation, OfficeError> {
    let mut package = open_package(bytes)?;
    let content = read_text_member(&mut package, "content.xml", limits)?.ok_or_else(|| {
        OfficeError::UnreadableDocument("the package has no content.xml".to_owned())
    })?;
    let content = parse_tree(&content, keeps_whitespace)?;
    let styles_root = read_text_member(&mut package, "styles.xml", limits)?
        .map(|xml| parse_tree(&xml, keeps_whitespace))
        .transpose()?;
    let styles = Styles::read(Some(&content), styles_root.as_ref());
    let pages = content
        .child(OFFICE, "body")
        .and_then(|body| body.child(OFFICE, "presentation"))
        .ok_or_else(|| {
            OfficeError::UnreadableDocument("the document has no presentation".to_owned())
        })?;

    let first_master = pages
        .elements()
        .find(|page| page.is(DRAW, "page"))
        .and_then(|page| page.attribute(DRAW, "master-page-name"));
    let size = master_page(&styles, first_master)
        .and_then(|master| master.attribute(STYLE, "page-layout-name"))
        .and_then(|name| styles.page_layouts.get(name))
        .and_then(|layout| layout.child(STYLE, "page-layout-properties"))
        .and_then(|properties| {
            let side = |name: &str| {
                properties
                    .attribute(FO, name)
                    .and_then(emu)
                    .filter(|side| (91_440..=MAX_SIDE).contains(side))
            };
            Some((side("page-width")?, side("page-height")?))
        })
        .unwrap_or(DEFAULT_SIZE);

    let mut reader = Reader {
        package: &mut package,
        limits,
        styles: &styles,
        media: Vec::new(),
        media_by_href: HashMap::new(),
        media_bytes: 0,
        remaining_chars: limits.max_text_chars,
        truncated: false,
    };
    let mut slides = Vec::new();
    for (index, page) in pages
        .elements()
        .filter(|page| page.is(DRAW, "page"))
        .enumerate()
    {
        if slides.len() >= MAX_SLIDES {
            reader.truncated = true;
            break;
        }
        slides.push(reader.slide(page, index + 1));
    }
    let truncated = reader.truncated;
    Ok(Presentation {
        width_emu: size.0,
        height_emu: size.1,
        slides,
        media: reader.media,
        truncated,
    })
}

fn keeps_whitespace(namespace: Namespace, name: &str) -> bool {
    namespace == TEXT && matches!(name, "p" | "h" | "span" | "a")
}

fn master_page<'s>(styles: &'s Styles, name: Option<&str>) -> Option<&'s Element> {
    styles
        .master_pages
        .iter()
        .find(|page| name.is_some() && page.attribute(STYLE, "name") == name)
        .or_else(|| styles.master_pages.first())
}

struct Reader<'a, 'p> {
    package: &'a mut ZipPackage<'p>,
    limits: &'a Limits,
    styles: &'a Styles,
    media: Vec<Media>,
    media_by_href: HashMap<String, Option<usize>>,
    media_bytes: u64,
    remaining_chars: usize,
    truncated: bool,
}

/// What a page shows of its master's placeholders.
#[derive(Debug, Clone, Copy, Default)]
struct Shown {
    page_number: bool,
    footer: bool,
    date_time: bool,
}

impl Reader<'_, '_> {
    fn slide(&mut self, page: &Element, number: usize) -> Slide {
        let master = master_page(self.styles, page.attribute(DRAW, "master-page-name"));
        let page_style = page.attribute(DRAW, "style-name");
        let drawing = |scope: Scope, name: Option<&str>, attribute: &str| {
            self.styles
                .properties(scope, "drawing-page", name, "drawing-page-properties")
                .into_iter()
                .find_map(|properties| properties.attribute(PRESENTATION, attribute))
                .map(|value| value == "true")
        };
        let shown = Shown {
            page_number: drawing(Scope::Body, page_style, "display-page-number").unwrap_or(false),
            footer: drawing(Scope::Body, page_style, "display-footer").unwrap_or(false),
            date_time: drawing(Scope::Body, page_style, "display-date-time").unwrap_or(false),
        };
        let background = match self.page_fill(Scope::Body, page_style) {
            Fill::Named(colour) => colour,
            Fill::Unset => master.and_then(|master| {
                self.page_fill(Scope::Master, master.attribute(DRAW, "style-name"))
                    .colour()
            }),
        };
        let background_objects =
            drawing(Scope::Body, page_style, "background-objects-visible").unwrap_or(true);
        let mut shapes = Vec::new();
        if background_objects && let Some(master) = master {
            self.group(master, Scope::Master, Some(shown), number, 0, &mut shapes);
        }
        self.group(page, Scope::Body, None, number, 0, &mut shapes);
        Slide { background, shapes }
    }

    /// The fill of a drawing-page style.
    fn page_fill(&self, scope: Scope, name: Option<&str>) -> Fill {
        if name.is_none() {
            return Fill::Unset;
        }
        let properties =
            self.styles
                .properties(scope, "drawing-page", name, "drawing-page-properties");
        graphic_fill(&properties, self.styles)
    }

    /// The shapes of a page, a master page or a group; a master's placeholders are templates,
    /// drawn only when they are the footer, the date or the page number the page shows.
    fn group(
        &mut self,
        group: &Element,
        scope: Scope,
        master: Option<Shown>,
        number: usize,
        depth: usize,
        shapes: &mut Vec<SlideShape>,
    ) {
        for element in group.elements().filter(|element| element.namespace == DRAW) {
            if shapes.len() >= MAX_SHAPES {
                self.truncated = true;
                return;
            }
            // On a master page every shape of a presentation class is a template for the
            // slides; only the footer, the date and the page number show, when the slide asks.
            if let Some(shown) = master
                && let Some(class) = element.attribute(PRESENTATION, "class")
            {
                let drawn = match class {
                    "page-number" => shown.page_number,
                    "footer" => shown.footer,
                    "date-time" => shown.date_time,
                    _ => false,
                };
                if !drawn {
                    continue;
                }
            }
            if master.is_none() && element.attribute(PRESENTATION, "placeholder") == Some("true") {
                continue;
            }
            match element.name.as_str() {
                "g" if depth < MAX_GROUP_DEPTH => {
                    self.group(element, scope, master, number, depth + 1, shapes);
                }
                _ => {
                    if let Some(shape) = self.shape(element, scope, number) {
                        shapes.push(shape);
                    }
                }
            }
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one match over every kind of drawing shape"
    )]
    fn shape(&mut self, element: &Element, scope: Scope, number: usize) -> Option<SlideShape> {
        let (frame, rotation, flipped) = if element.name == "line" || element.name == "connector" {
            line_frame(element)?
        } else {
            let (frame, rotation) = placed_frame(element)?;
            (frame, rotation, false)
        };
        let style_name = element
            .attribute(PRESENTATION, "style-name")
            .map(|name| ("presentation", name))
            .or_else(|| {
                element
                    .attribute(DRAW, "style-name")
                    .map(|name| ("graphic", name))
            });
        let (family, name) =
            style_name.map_or(("graphic", None), |(family, name)| (family, Some(name)));
        let graphic = self
            .styles
            .properties(scope, family, name, "graphic-properties");
        let attribute = |namespace: Namespace, key: &str| {
            graphic
                .iter()
                .find_map(|properties| properties.attribute(namespace, key))
        };
        let geometry = match element.name.as_str() {
            "rect" if element.attribute(DRAW, "corner-radius").is_some() => {
                ShapeGeometry::RoundedRectangle
            }
            "rect" | "frame" | "polygon" | "polyline" | "path" => ShapeGeometry::Rectangle,
            "ellipse" | "circle" => ShapeGeometry::Ellipse,
            "line" | "connector" => ShapeGeometry::Line { flipped },
            "custom-shape" => match element
                .child(DRAW, "enhanced-geometry")
                .and_then(|geometry| geometry.attribute(DRAW, "type"))
                .map(|kind| kind.trim_start_matches("ooxml-"))
            {
                Some("rectangle" | "rect") => ShapeGeometry::Rectangle,
                Some("round-rectangle" | "round-rect" | "roundRect" | "snipRoundRect") => {
                    ShapeGeometry::RoundedRectangle
                }
                Some("ellipse" | "circle") => ShapeGeometry::Ellipse,
                _ => ShapeGeometry::Other,
            },
            _ => return None,
        };
        let fill = if element.name == "line" || element.name == "connector" {
            None
        } else {
            graphic_fill(&graphic, self.styles).colour()
        };
        let is_line = matches!(element.name.as_str(), "line" | "connector");
        let line = match attribute(DRAW, "stroke") {
            Some("none") => None,
            Some(_) | None => attribute(SVG, "stroke-color")
                .and_then(colour)
                // A line is drawn black when nothing names its colour, as `OpenDocument` sets it.
                .or_else(|| is_line.then(|| "000000".to_owned()))
                .map(|stroke_colour| ShapeLine {
                    colour: stroke_colour,
                    width_emu: attribute(SVG, "stroke-width")
                        .and_then(emu)
                        .unwrap_or(0)
                        .clamp(0, 12_700 * 100),
                }),
        }
        .filter(|_| attribute(DRAW, "stroke").is_some() || element.name != "frame");

        // A table or an object comes with a replacement image, which only stands in for it.
        let chart = element
            .child(DRAW, "object")
            .and_then(|object| object.attribute(XLINK, "href"))
            .and_then(|href| self.chart(href, frame));
        let content = if let Some(media) = chart {
            ShapeContent::Picture { media }
        } else if let Some(table) = element.child(TABLE, "table") {
            ShapeContent::Table(self.table(table, scope, number))
        } else if let Some(image) = element
            .child(DRAW, "image")
            .filter(|_| element.name == "frame")
        {
            match image
                .attribute(XLINK, "href")
                .and_then(|href| self.image(href))
            {
                Some(media) => ShapeContent::Picture { media },
                None => ShapeContent::None,
            }
        } else {
            let text_container = element.child(DRAW, "text-box").unwrap_or(element);
            let has_text = text_container.elements().any(|child| {
                child.namespace == TEXT && matches!(child.name.as_str(), "p" | "h" | "list")
            });
            if has_text {
                ShapeContent::Text(self.text_body(
                    text_container,
                    scope,
                    family,
                    name,
                    &graphic,
                    number,
                ))
            } else {
                ShapeContent::None
            }
        };
        if content == ShapeContent::None && fill.is_none() && line.is_none() {
            return None;
        }
        Some(SlideShape {
            frame,
            rotation,
            geometry,
            fill,
            line,
            content,
        })
    }

    /// The text of a frame or shape: its paragraphs styled from their own styles out to the
    /// shape's presentation or graphic style.
    fn text_body(
        &mut self,
        container: &Element,
        scope: Scope,
        family: &str,
        style: Option<&str>,
        graphic: &[&Element],
        number: usize,
    ) -> TextBody {
        let attribute = |namespace: Namespace, key: &str| {
            graphic
                .iter()
                .find_map(|properties| properties.attribute(namespace, key))
        };
        let anchor = match attribute(DRAW, "textarea-vertical-align") {
            Some("middle") => TextAnchor::Middle,
            Some("bottom") => TextAnchor::Bottom,
            _ => TextAnchor::Top,
        };
        let inset = |name: &str, default: i64| {
            attribute(FO, name)
                .or_else(|| attribute(FO, "padding"))
                .and_then(emu)
                .unwrap_or(default)
                .max(0)
        };
        let insets = [
            inset("padding-left", DEFAULT_INSETS[0]),
            inset("padding-top", DEFAULT_INSETS[1]),
            inset("padding-right", DEFAULT_INSETS[2]),
            inset("padding-bottom", DEFAULT_INSETS[3]),
        ];
        let mut context = TextContext {
            scope,
            family: family.to_owned(),
            style: style.map(str::to_owned),
            number,
            paragraphs: Vec::new(),
        };
        self.paragraphs(container, &mut context, None, 0);
        TextBody {
            paragraphs: context.paragraphs,
            anchor,
            insets_emu: insets,
            wrap: attribute(FO, "wrap-option") != Some("no-wrap"),
        }
    }

    fn paragraphs(
        &mut self,
        container: &Element,
        context: &mut TextContext,
        list: Option<&str>,
        level: usize,
    ) {
        let mut counters = [0_u32; MAX_LEVEL];
        self.paragraphs_counting(container, context, list, level, &mut counters);
    }

    fn paragraphs_counting(
        &mut self,
        container: &Element,
        context: &mut TextContext,
        list: Option<&str>,
        level: usize,
        counters: &mut [u32; MAX_LEVEL],
    ) {
        for element in container
            .elements()
            .filter(|element| element.namespace == TEXT)
        {
            match element.name.as_str() {
                "p" | "h" => self.paragraph(element, context, None, level),
                "list" => self.list(element, context, list, level, counters),
                _ => {}
            }
        }
    }

    /// The items of a `text:list`, a nested list one level deeper.
    fn list(
        &mut self,
        list: &Element,
        context: &mut TextContext,
        inherited: Option<&str>,
        level: usize,
        counters: &mut [u32; MAX_LEVEL],
    ) {
        let style = list
            .attribute(TEXT, "style-name")
            .or(inherited)
            .map(str::to_owned);
        for item in list.elements() {
            let header = item.is(TEXT, "list-header");
            if !header && !item.is(TEXT, "list-item") {
                continue;
            }
            let mut first = !header;
            for child in item.elements().filter(|child| child.namespace == TEXT) {
                match child.name.as_str() {
                    "p" | "h" => {
                        let bullet = if first {
                            self.bullet(style.as_deref(), context.scope, level, counters)
                        } else {
                            None
                        };
                        first = false;
                        let place = ListLevel {
                            bullet,
                            indent: self.list_indent(style.as_deref(), context.scope, level),
                        };
                        self.paragraph(child, context, Some(place), level);
                    }
                    "list" => self.list(child, context, style.as_deref(), level + 1, counters),
                    _ => {}
                }
            }
        }
    }

    /// The bullet of a list level, counting a numbered one.
    fn bullet(
        &self,
        list: Option<&str>,
        scope: Scope,
        level: usize,
        counters: &mut [u32; MAX_LEVEL],
    ) -> Option<Bullet> {
        let level = level.min(MAX_LEVEL - 1);
        let style = list_level(self.styles, list, scope, level)?;
        counters[level] += 1;
        for deeper in &mut counters[level + 1..] {
            *deeper = 0;
        }
        if style.is(TEXT, "list-level-style-bullet") {
            let character = style.attribute(TEXT, "bullet-char").unwrap_or("\u{2022}");
            Some(Bullet::Character(crate::document::bullet_label(
                character, None,
            )))
        } else if style.is(TEXT, "list-level-style-number") {
            let format = style.attribute(STYLE, "num-format").unwrap_or("1");
            if format.is_empty() {
                return None;
            }
            let start = style
                .attribute(TEXT, "start-value")
                .and_then(|start| start.parse::<u32>().ok())
                .unwrap_or(1);
            Some(Bullet::Number(format!(
                "{}{}{}",
                style.attribute(STYLE, "num-prefix").unwrap_or(""),
                crate::document::number_label(counters[level] + start - 1, format),
                style.attribute(STYLE, "num-suffix").unwrap_or("")
            )))
        } else {
            None
        }
    }

    /// The left margin and first-line indent of a list level, in English Metric Units.
    fn list_indent(&self, list: Option<&str>, scope: Scope, level: usize) -> Option<(i64, i64)> {
        let style = list_level(self.styles, list, scope, level.min(MAX_LEVEL - 1))?;
        let properties = style.child(STYLE, "list-level-properties")?;
        if let Some(alignment) = properties.child(STYLE, "list-level-label-alignment") {
            let left = alignment.attribute(FO, "margin-left").and_then(emu)?;
            let first = alignment
                .attribute(FO, "text-indent")
                .and_then(emu)
                .unwrap_or(0);
            return Some((left, first));
        }
        let before = properties
            .attribute(TEXT, "space-before")
            .and_then(emu)
            .unwrap_or(0);
        let width = properties
            .attribute(TEXT, "min-label-width")
            .and_then(emu)
            .unwrap_or(0);
        Some((before + width, -width))
    }

    fn paragraph(
        &mut self,
        paragraph: &Element,
        context: &mut TextContext,
        list: Option<ListLevel>,
        level: usize,
    ) {
        let scope = context.scope;
        let paragraph_style = paragraph.attribute(TEXT, "style-name");
        // An outline frame takes the presentation style of each level: `Default-outline2` ...
        let shape_style = level_style(
            self.styles,
            scope,
            &context.family,
            context.style.as_deref(),
            level,
        );
        let (own, align) = paragraph_only(self.styles, scope, paragraph_style);
        let mut look = own;
        let mut alignment = align;
        for properties in self.styles.properties(
            scope,
            &context.family,
            shape_style.as_deref(),
            "paragraph-properties",
        ) {
            let (shape_look, shape_align) = super::styles::paragraph_look_of(properties);
            look = look.or(shape_look);
            alignment = alignment.or(shape_align);
        }
        let paragraph_font = self.styles.text_look(scope, "paragraph", paragraph_style).0;
        let shape_font = self
            .styles
            .text_look(scope, &context.family, shape_style.as_deref())
            .0;
        let base = paragraph_font.or(shape_font);
        let mut runs = Vec::new();
        let mut space = true;
        self.runs(
            paragraph,
            scope,
            &base,
            false,
            false,
            context.number,
            &mut space,
            &mut runs,
        );
        while runs
            .last()
            .is_some_and(|run: &SlideRun| !run.line_break && run.text.trim().is_empty())
        {
            runs.pop();
        }
        if let Some(last) = runs.last_mut() {
            let trimmed = last.text.trim_end().len();
            last.text.truncate(trimmed);
        }
        let (margin_left, indent) = list.as_ref().and_then(|list| list.indent).unwrap_or((
            i64::from(look.indent_left_twips.unwrap_or(0)) * 635,
            i64::from(look.indent_first_line_twips.unwrap_or(0)) * 635,
        ));
        let line_spacing_percent = match look.line_spacing {
            Some(LineSpacing::Auto(multiple)) => (multiple * 100 / 240).clamp(10, 1_000),
            _ => 100,
        };
        let hundredths = |twips: Option<u32>| twips.unwrap_or(0).saturating_mul(5);
        context.paragraphs.push(SlideParagraph {
            level: u8::try_from(level).unwrap_or(0),
            align: alignment.unwrap_or(Alignment::Left),
            bullet: list
                .and_then(|list| list.bullet)
                .filter(|_| !runs.is_empty()),
            margin_left_emu: margin_left,
            indent_emu: indent,
            line_spacing_percent,
            space_before: hundredths(look.space_before_twips),
            space_after: hundredths(look.space_after_twips),
            runs,
        });
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "a run carries its look and the space before it down the tree"
    )]
    fn runs(
        &mut self,
        container: &Element,
        scope: Scope,
        look: &RunLook,
        underline: bool,
        strike: bool,
        number: usize,
        space: &mut bool,
        runs: &mut Vec<SlideRun>,
    ) {
        for node in &container.children {
            match node {
                Node::Text(raw) => {
                    let mut collapsed = String::new();
                    for character in raw.chars() {
                        if character.is_whitespace() {
                            if !*space {
                                collapsed.push(' ');
                                *space = true;
                            }
                        } else {
                            collapsed.push(character);
                            *space = false;
                        }
                    }
                    self.push_run(runs, &collapsed, look, underline, strike, false);
                }
                Node::Element(element) if element.namespace == TEXT => {
                    match element.name.as_str() {
                        "s" => {
                            let count = element
                                .attribute(TEXT, "c")
                                .and_then(|count| count.parse::<usize>().ok())
                                .unwrap_or(1)
                                .clamp(1, 1_000);
                            self.push_run(runs, &" ".repeat(count), look, underline, strike, false);
                        }
                        "tab" => {
                            self.push_run(runs, "\t", look, underline, strike, false);
                        }
                        "line-break" => {
                            self.push_run(runs, "", look, underline, strike, true);
                            *space = true;
                        }
                        "span" | "a" => {
                            let (own, flags) = self.styles.text_look(
                                scope,
                                "text",
                                element.attribute(TEXT, "style-name"),
                            );
                            let merged = own.or(look.clone());
                            self.runs(
                                element,
                                scope,
                                &merged,
                                flags.underline.unwrap_or(underline),
                                flags.strike.unwrap_or(strike),
                                number,
                                space,
                                runs,
                            );
                        }
                        "page-number" => {
                            let shown = if number > 0 {
                                number.to_string()
                            } else {
                                element.text()
                            };
                            self.push_run(runs, &shown, look, underline, strike, false);
                            *space = false;
                        }
                        _ => {
                            self.runs(element, scope, look, underline, strike, number, space, runs);
                        }
                    }
                }
                Node::Element(_) => {}
            }
        }
    }

    fn push_run(
        &mut self,
        runs: &mut Vec<SlideRun>,
        text: &str,
        look: &RunLook,
        underline: bool,
        strike: bool,
        line_break: bool,
    ) {
        if text.is_empty() && !line_break {
            return;
        }
        let taken: String = text.chars().take(self.remaining_chars).collect();
        let count = taken.chars().count();
        if count < text.chars().count() {
            self.truncated = true;
        }
        self.remaining_chars -= count;
        if taken.is_empty() && !line_break {
            return;
        }
        runs.push(SlideRun {
            text: taken,
            size: look
                .size_half_points
                .map_or(DEFAULT_SIZE_HUNDREDTHS, |size| size * 50)
                .max(100),
            bold: look.bold.unwrap_or(false),
            italic: look.italic.unwrap_or(false),
            underline,
            strike,
            colour: look.colour.clone().unwrap_or_else(|| "000000".to_owned()),
            family: look
                .family
                .clone()
                .unwrap_or_else(|| "Liberation Sans".to_owned()),
            line_break,
        });
    }

    fn table(&mut self, table: &Element, scope: Scope, number: usize) -> SlideTable {
        let mut column_widths_emu = Vec::new();
        let mut rows = Vec::new();
        for element in table
            .elements()
            .filter(|element| element.namespace == TABLE)
        {
            match element.name.as_str() {
                "table-column" => {
                    let width = self
                        .styles
                        .properties(
                            scope,
                            "table-column",
                            element.attribute(TABLE, "style-name"),
                            "table-column-properties",
                        )
                        .into_iter()
                        .find_map(|properties| properties.attribute(STYLE, "column-width"))
                        .and_then(emu)
                        .unwrap_or(0);
                    let repeat = element
                        .attribute(TABLE, "number-columns-repeated")
                        .and_then(|count| count.parse::<usize>().ok())
                        .unwrap_or(1)
                        .clamp(1, 64);
                    column_widths_emu.extend(std::iter::repeat_n(width, repeat));
                }
                "table-row" => {
                    let height = self
                        .styles
                        .properties(
                            scope,
                            "table-row",
                            element.attribute(TABLE, "style-name"),
                            "table-row-properties",
                        )
                        .into_iter()
                        .find_map(|properties| {
                            properties
                                .attribute(STYLE, "row-height")
                                .or_else(|| properties.attribute(STYLE, "min-row-height"))
                        })
                        .and_then(emu)
                        .unwrap_or(0);
                    let mut cells = Vec::new();
                    let row_style = element.attribute(TABLE, "default-cell-style-name");
                    for cell in element.elements().filter(|cell| cell.namespace == TABLE) {
                        let covered = cell.name == "covered-table-cell";
                        if !covered && cell.name != "table-cell" {
                            continue;
                        }
                        let style = cell.attribute(TABLE, "style-name").or(row_style);
                        let graphic = self.styles.properties(
                            scope,
                            "table-cell",
                            style,
                            "graphic-properties",
                        );
                        let cell_properties = self.styles.properties(
                            scope,
                            "table-cell",
                            style,
                            "table-cell-properties",
                        );
                        let fill = graphic_fill(&graphic, self.styles).colour().or_else(|| {
                            cell_properties
                                .iter()
                                .find_map(|properties| properties.attribute(FO, "background-color"))
                                .and_then(colour)
                        });
                        let body =
                            self.text_body(cell, scope, "table-cell", style, &graphic, number);
                        let span = |name: &str| {
                            cell.attribute(TABLE, name)
                                .and_then(|span| span.parse::<u32>().ok())
                                .unwrap_or(1)
                                .clamp(1, 1_000)
                        };
                        cells.push(SlideTableCell {
                            body,
                            fill,
                            col_span: span("number-columns-spanned"),
                            row_span: span("number-rows-spanned"),
                            covered,
                        });
                    }
                    rows.push(SlideTableRow {
                        height_emu: height,
                        cells,
                    });
                }
                _ => {}
            }
        }
        SlideTable {
            column_widths_emu,
            rows,
        }
    }

    /// A chart object drawn as an SVG image the size of its frame.
    fn chart(&mut self, href: &str, frame: Frame) -> Option<usize> {
        let part = odf_object_part(href)?;
        let xml = read_text_member(self.package, &part, self.limits)
            .ok()
            .flatten()?;
        let (bytes, _) = odf_chart_picture(&xml, frame.width, frame.height)?;
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

    fn image(&mut self, href: &str) -> Option<usize> {
        let href = href.trim_start_matches("./").to_owned();
        if href.contains("://") {
            return None;
        }
        if let Some(known) = self.media_by_href.get(&href) {
            return *known;
        }
        let loaded = self.load_image(&href);
        self.media_by_href.insert(href, loaded);
        loaded
    }

    fn load_image(&mut self, href: &str) -> Option<usize> {
        if self.media.len() >= self.limits.max_images {
            self.truncated = true;
            return None;
        }
        let cap = self
            .limits
            .max_image_bytes
            .min(self.limits.max_member_bytes);
        let bytes = read_member_bytes(self.package, href, cap).ok()??;
        let size = bytes.len() as u64;
        if self.media_bytes.saturating_add(size) > self.limits.max_total_image_bytes {
            self.truncated = true;
            return None;
        }
        let content_type = sniff_image_type(&bytes);
        if !content_type.starts_with("image/") {
            return None;
        }
        self.media_bytes += size;
        self.media.push(Media {
            content_type,
            bytes,
        });
        Some(self.media.len() - 1)
    }
}

/// The text of a shape as it is read: the shape's style family and name and its paragraphs.
struct TextContext {
    scope: Scope,
    family: String,
    style: Option<String>,
    number: usize,
    paragraphs: Vec<SlideParagraph>,
}

/// The bullet and indent a list gives the paragraph of an item.
struct ListLevel {
    bullet: Option<Bullet>,
    indent: Option<(i64, i64)>,
}

/// The style of one level of a list style.
fn list_level<'s>(
    styles: &'s Styles,
    list: Option<&str>,
    scope: Scope,
    level: usize,
) -> Option<&'s Element> {
    styles.list_style(scope, list?)?.elements().find(|style| {
        style
            .attribute(TEXT, "level")
            .and_then(|value| value.parse::<usize>().ok())
            == Some(level + 1)
    })
}

/// The presentation style of an outline level: an outline frame's `<master>-outline1` becomes
/// `<master>-outline2` one level down, if the master has one.
fn level_style(
    styles: &Styles,
    scope: Scope,
    family: &str,
    style: Option<&str>,
    level: usize,
) -> Option<String> {
    if level == 0 || family != "presentation" {
        return style.map(str::to_owned);
    }
    let outline = styles
        .chain(scope, family, style)
        .into_iter()
        .filter_map(|element| element.attribute(STYLE, "name"))
        .find_map(|name| name.strip_suffix("-outline1").map(str::to_owned));
    match outline {
        Some(prefix) => {
            let deeper = format!("{prefix}-outline{}", (level + 1).min(MAX_LEVEL));
            if styles.chain(scope, family, Some(&deeper)).len() > 1 {
                Some(deeper)
            } else {
                style.map(str::to_owned)
            }
        }
        None => style.map(str::to_owned),
    }
}

/// A paragraph style's own paragraph properties through its parents, without the family's
/// default, which the shape's style comes before.
fn paragraph_only(
    styles: &Styles,
    scope: Scope,
    name: Option<&str>,
) -> (ParagraphLook, Option<Alignment>) {
    let mut look = ParagraphLook::default();
    let mut align = None;
    if name.is_none() {
        return (look, align);
    }
    for properties in styles.properties(scope, "paragraph", name, "paragraph-properties") {
        let (own, own_align) = super::styles::paragraph_look_of(properties);
        look = look.or(own);
        align = align.or(own_align);
    }
    (look, align)
}
/// What graphic properties say about a fill.
enum Fill {
    /// They name none, so whatever stands behind decides.
    Unset,
    /// They name one, a colour or none at all.
    Named(Option<String>),
}

impl Fill {
    fn colour(self) -> Option<String> {
        match self {
            Self::Unset => None,
            Self::Named(colour) => colour,
        }
    }
}

/// The fill graphic properties name, nearest first; a gradient shows its first colour.
fn graphic_fill(properties: &[&Element], styles: &Styles) -> Fill {
    let Some(kind) = properties
        .iter()
        .find_map(|properties| properties.attribute(DRAW, "fill"))
    else {
        return Fill::Unset;
    };
    let attribute = |key: &str| {
        properties
            .iter()
            .find_map(|properties| properties.attribute(DRAW, key))
    };
    Fill::Named(match kind {
        "solid" | "hatch" | "bitmap" => attribute("fill-color").and_then(colour),
        "gradient" => attribute("fill-gradient-name")
            .and_then(|name| styles.gradient_start(name))
            .or_else(|| attribute("fill-color").and_then(colour)),
        _ => None,
    })
}

/// The frame and clockwise rotation of a shape placed by `svg:x`/`svg:y`, or by a
/// `draw:transform` that rotates it about its corner and moves it.
fn placed_frame(element: &Element) -> Option<(Frame, f64)> {
    let width = element
        .attribute(SVG, "width")
        .and_then(emu)
        .unwrap_or(0)
        .max(0);
    let height = element
        .attribute(SVG, "height")
        .and_then(emu)
        .unwrap_or(0)
        .max(0);
    if let Some(transform) = element.attribute(DRAW, "transform") {
        let (angle, translate) = parse_transform(transform);
        #[expect(clippy::cast_precision_loss, reason = "slide coordinates fit in f64")]
        let (half_width, half_height) = (width as f64 / 2.0, height as f64 / 2.0);
        // Counter-clockwise on a page whose y grows downwards.
        let (sin, cos) = angle.sin_cos();
        let centre_x = translate.0 + half_width * cos + half_height * sin;
        let centre_y = translate.1 - half_width * sin + half_height * cos;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "slide coordinates stay within a few hundred million"
        )]
        let frame = Frame {
            x: (centre_x - half_width).round() as i64,
            y: (centre_y - half_height).round() as i64,
            width,
            height,
        };
        let degrees = (-angle.to_degrees()).rem_euclid(360.0);
        return Some((frame, if degrees.abs() < 1e-6 { 0.0 } else { degrees }));
    }
    let x = element.attribute(SVG, "x").and_then(emu)?;
    let y = element.attribute(SVG, "y").and_then(emu)?;
    Some((
        Frame {
            x,
            y,
            width,
            height,
        },
        0.0,
    ))
}

/// The rotation in radians and the translation in English Metric Units of a `draw:transform`.
fn parse_transform(transform: &str) -> (f64, (f64, f64)) {
    let mut angle = 0.0;
    let mut translate = (0.0, 0.0);
    let mut rest = transform;
    while let Some(open) = rest.find('(') {
        let name = rest[..open].trim();
        let Some(close) = rest[open..].find(')') else {
            break;
        };
        let arguments: Vec<&str> = rest[open + 1..open + close].split_whitespace().collect();
        match name {
            "rotate" => {
                angle += arguments
                    .first()
                    .and_then(|value| value.parse::<f64>().ok())
                    .unwrap_or(0.0);
            }
            "translate" => {
                #[expect(clippy::cast_precision_loss, reason = "slide coordinates fit in f64")]
                let value = |index: usize| {
                    arguments
                        .get(index)
                        .and_then(|value| emu(value))
                        .unwrap_or(0) as f64
                };
                translate = (translate.0 + value(0), translate.1 + value(1));
            }
            _ => {}
        }
        rest = &rest[open + close + 1..];
    }
    (angle, translate)
}

/// The frame of a line from `svg:x1`/`svg:y1` to `svg:x2`/`svg:y2`, and whether it runs up.
fn line_frame(element: &Element) -> Option<(Frame, f64, bool)> {
    let coordinate = |name: &str| element.attribute(SVG, name).and_then(emu);
    let (x1, y1, x2, y2) = (
        coordinate("x1")?,
        coordinate("y1")?,
        coordinate("x2")?,
        coordinate("y2")?,
    );
    let frame = Frame {
        x: x1.min(x2),
        y: y1.min(y2),
        width: (x2 - x1).abs(),
        height: (y2 - y1).abs(),
    };
    Some((frame, 0.0, (x2 > x1) != (y2 > y1)))
}

/// A length in English Metric Units.
fn emu(length: &str) -> Option<i64> {
    let points = points(length)?;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the length is clamped to the largest page"
    )]
    let emu = (points * EMU_PER_POINT)
        .round()
        .clamp(-MAX_LENGTH, MAX_LENGTH) as i64;
    Some(emu)
}

#[cfg(test)]
mod tests;
