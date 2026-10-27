// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The styles of an `OpenDocument` package: the named styles of `styles.xml`, the automatic
//! styles of each part, the defaults of each family, the list styles and the font faces, with
//! each style resolved through its parents to the look the document tree carries.

use std::collections::HashMap;

use crate::document::{
    Alignment, Border, BorderStyle, Borders, LineSpacing, ParagraphLook, RunLook, TabKind,
    TabLeader, TabStop, VerticalAlignment,
};
use crate::xml_tree::{Element, Namespace};

pub(super) const STYLE: Namespace = Namespace::OdfStyle;
pub(super) const FO: Namespace = Namespace::OdfFo;
pub(super) const TEXT: Namespace = Namespace::OdfText;
const SVG: Namespace = Namespace::OdfSvg;
const OFFICE: Namespace = Namespace::OdfOffice;
const DRAW: Namespace = Namespace::OdfDraw;
const XLINK: Namespace = Namespace::XLink;
/// A style chain stops here, so a style that names itself as its parent cannot loop.
const MAX_CHAIN: usize = 16;
const TWIPS_PER_POINT: f64 = 20.0;
const MAX_TWIPS: f64 = 31_680.0 * 2.0;

/// Which automatic styles apply: those of `content.xml` to the body, those of `styles.xml` to
/// the master pages, their headers and footers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Scope {
    Body,
    Master,
}

/// The formatting a text style sets beyond the font of [`RunLook`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct TextFlags {
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    /// `true` raised, `false` lowered.
    pub raised: Option<bool>,
}

impl TextFlags {
    #[must_use]
    pub(super) fn or(self, fallback: Self) -> Self {
        Self {
            underline: self.underline.or(fallback.underline),
            strike: self.strike.or(fallback.strike),
            raised: self.raised.or(fallback.raised),
        }
    }
}

/// The styles of a package.
#[derive(Debug, Default)]
pub(super) struct Styles {
    named: HashMap<(String, String), Element>,
    body: HashMap<(String, String), Element>,
    master: HashMap<(String, String), Element>,
    defaults: HashMap<String, Element>,
    body_lists: HashMap<String, Element>,
    master_lists: HashMap<String, Element>,
    named_lists: HashMap<String, Element>,
    fonts: HashMap<String, String>,
    pub outline: Option<Element>,
    pub page_layouts: HashMap<String, Element>,
    pub master_pages: Vec<Element>,
    /// The `draw:gradient` of each name.
    gradients: HashMap<String, Element>,
    /// The `xlink:href` of each `draw:fill-image`.
    fill_images: HashMap<String, String>,
}

impl Styles {
    /// The styles of the two parts, either of which may be missing.
    pub(super) fn read(content: Option<&Element>, styles: Option<&Element>) -> Self {
        let mut read = Self::default();
        for root in [styles, content].into_iter().flatten() {
            if let Some(faces) = root.child(OFFICE, "font-face-decls") {
                for face in faces.elements().filter(|face| face.is(STYLE, "font-face")) {
                    if let (Some(name), Some(family)) = (
                        face.attribute(STYLE, "name"),
                        face.attribute(SVG, "font-family"),
                    ) {
                        read.fonts.insert(name.to_owned(), unquote(family));
                    }
                }
            }
        }
        if let Some(styles) = styles {
            if let Some(named) = styles.child(OFFICE, "styles") {
                for style in named.elements() {
                    match (style.namespace, style.name.as_str()) {
                        (STYLE, "style") => insert_style(&mut read.named, style),
                        (STYLE, "default-style") => {
                            if let Some(family) = style.attribute(STYLE, "family") {
                                read.defaults.insert(family.to_owned(), style.clone());
                            }
                        }
                        (TEXT, "list-style") => insert_list(&mut read.named_lists, style),
                        (TEXT, "outline-style") => read.outline = Some(style.clone()),
                        (DRAW, "gradient") => {
                            if let Some(name) = style.attribute(DRAW, "name") {
                                read.gradients.insert(name.to_owned(), style.clone());
                            }
                        }
                        (DRAW, "fill-image") => {
                            if let (Some(name), Some(href)) = (
                                style.attribute(DRAW, "name"),
                                style.attribute(XLINK, "href"),
                            ) {
                                read.fill_images.insert(name.to_owned(), href.to_owned());
                            }
                        }
                        _ => {}
                    }
                }
            }
            if let Some(automatic) = styles.child(OFFICE, "automatic-styles") {
                for style in automatic.elements() {
                    match (style.namespace, style.name.as_str()) {
                        (STYLE, "style") => insert_style(&mut read.master, style),
                        (STYLE, "page-layout") => {
                            if let Some(name) = style.attribute(STYLE, "name") {
                                read.page_layouts.insert(name.to_owned(), style.clone());
                            }
                        }
                        (TEXT, "list-style") => insert_list(&mut read.master_lists, style),
                        _ => {}
                    }
                }
            }
            if let Some(masters) = styles.child(OFFICE, "master-styles") {
                read.master_pages = masters
                    .elements()
                    .filter(|page| page.is(STYLE, "master-page"))
                    .cloned()
                    .collect();
            }
        }
        if let Some(automatic) =
            content.and_then(|content| content.child(OFFICE, "automatic-styles"))
        {
            for style in automatic.elements() {
                match (style.namespace, style.name.as_str()) {
                    (STYLE, "style") => insert_style(&mut read.body, style),
                    (TEXT, "list-style") => insert_list(&mut read.body_lists, style),
                    _ => {}
                }
            }
        }
        read
    }

    fn style(&self, scope: Scope, family: &str, name: &str) -> Option<&Element> {
        let key = (family.to_owned(), name.to_owned());
        let automatic = match scope {
            Scope::Body => &self.body,
            Scope::Master => &self.master,
        };
        automatic.get(&key).or_else(|| self.named.get(&key))
    }

    /// A style and its parents, nearest first, then the family's default style.
    pub(super) fn chain(&self, scope: Scope, family: &str, name: Option<&str>) -> Vec<&Element> {
        let mut chain = Vec::new();
        let mut next = name.map(str::to_owned);
        while let Some(name) = next.take() {
            if chain.len() >= MAX_CHAIN {
                break;
            }
            let Some(style) = self.style(scope, family, &name) else {
                break;
            };
            chain.push(style);
            next = style
                .attribute(STYLE, "parent-style-name")
                .filter(|parent| *parent != name)
                .map(str::to_owned);
        }
        if let Some(default) = self.defaults.get(family) {
            chain.push(default);
        }
        chain
    }

    /// The `draw:gradient` of a name.
    pub(super) fn gradient(&self, name: &str) -> Option<&Element> {
        self.gradients.get(name)
    }

    /// The picture a `draw:fill-image` of a name points at.
    pub(super) fn fill_image(&self, name: &str) -> Option<&str> {
        self.fill_images.get(name).map(String::as_str)
    }

    /// The list style of a name, the part's own first.
    pub(super) fn list_style(&self, scope: Scope, name: &str) -> Option<&Element> {
        let automatic = match scope {
            Scope::Body => &self.body_lists,
            Scope::Master => &self.master_lists,
        };
        automatic.get(name).or_else(|| self.named_lists.get(name))
    }

    /// The text look of a text or paragraph style and its parents.
    pub(super) fn text_look(
        &self,
        scope: Scope,
        family: &str,
        name: Option<&str>,
    ) -> (RunLook, TextFlags) {
        self.chain(scope, family, name)
            .into_iter()
            .filter_map(|style| style.child(STYLE, "text-properties"))
            .map(|properties| self.text_properties(properties))
            .fold(
                (RunLook::default(), TextFlags::default()),
                |(look, flags), (other, other_flags)| (look.or(other), flags.or(other_flags)),
            )
    }

    /// The paragraph look of a paragraph style and its parents, its font included, and the
    /// alignment it sets.
    pub(super) fn paragraph_look(
        &self,
        scope: Scope,
        name: Option<&str>,
    ) -> (ParagraphLook, Option<Alignment>, TextFlags) {
        let mut look = ParagraphLook::default();
        let mut align = None;
        for style in self.chain(scope, "paragraph", name) {
            if let Some(properties) = style.child(STYLE, "paragraph-properties") {
                let (own, own_align) = paragraph_look_of(properties);
                look = look.or(own);
                align = align.or(own_align);
            }
        }
        let (font, flags) = self.text_look(scope, "paragraph", name);
        look.font = font;
        (look, align, flags)
    }

    /// The master page a paragraph style starts, through its parents.
    pub(super) fn master_page_of(&self, scope: Scope, name: Option<&str>) -> Option<String> {
        self.chain(scope, "paragraph", name)
            .into_iter()
            .find_map(|style| style.attribute(STYLE, "master-page-name"))
            .filter(|page| !page.is_empty())
            .map(str::to_owned)
    }

    /// The list style a paragraph style names, through its parents.
    pub(super) fn list_style_of(&self, scope: Scope, name: Option<&str>) -> Option<String> {
        self.chain(scope, "paragraph", name)
            .into_iter()
            .find_map(|style| style.attribute(STYLE, "list-style-name"))
            .filter(|list| !list.is_empty())
            .map(str::to_owned)
    }

    /// A style's properties element of one kind: `table-cell-properties`, ...
    pub(super) fn properties<'s>(
        &'s self,
        scope: Scope,
        family: &str,
        name: Option<&str>,
        kind: &str,
    ) -> Vec<&'s Element> {
        self.chain(scope, family, name)
            .into_iter()
            .filter_map(|style| {
                style
                    .child(STYLE, kind)
                    .or_else(|| style.child(Namespace::LoExt, kind))
            })
            .collect()
    }

    /// The font and flags of a `style:text-properties`.
    pub(super) fn text_properties(&self, properties: &Element) -> (RunLook, TextFlags) {
        let attribute = |namespace: Namespace, name: &str| properties.attribute(namespace, name);
        let family = attribute(STYLE, "font-name")
            .map(|name| {
                self.fonts
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| name.to_owned())
            })
            .or_else(|| attribute(FO, "font-family").map(unquote))
            .filter(|family| !family.is_empty());
        let size_half_points = attribute(FO, "font-size")
            .and_then(points)
            .filter(|size| *size > 0.0)
            .map(|size| (size * 2.0).round().clamp(2.0, 3_276.0))
            .map(|size| {
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "the size is clamped to a small positive range"
                )]
                let size = size as u32;
                size
            });
        let line = |name: &str| attribute(STYLE, name).map(|style| style != "none");
        let raised = attribute(STYLE, "text-position").and_then(|position| {
            let first = position.split_whitespace().next()?;
            match first {
                "super" => Some(true),
                "sub" => Some(false),
                percent => {
                    let value = percent.trim_end_matches('%').parse::<f64>().ok()?;
                    (value != 0.0).then_some(value > 0.0)
                }
            }
        });
        let look = RunLook {
            family,
            size_half_points,
            colour: attribute(FO, "color").and_then(colour),
            bold: attribute(FO, "font-weight").map(|weight| {
                weight == "bold" || weight.parse::<u32>().is_ok_and(|weight| weight >= 600)
            }),
            italic: attribute(FO, "font-style")
                .map(|style| style == "italic" || style == "oblique"),
            highlight: attribute(FO, "background-color").and_then(colour),
            caps: attribute(FO, "text-transform").map(|transform| transform == "uppercase"),
            small_caps: attribute(FO, "font-variant").map(|variant| variant == "small-caps"),
            field: None,
        };
        let flags = TextFlags {
            underline: line("text-underline-style"),
            strike: line("text-line-through-style"),
            raised,
        };
        (look, flags)
    }
}

fn insert_style(map: &mut HashMap<(String, String), Element>, style: &Element) {
    if let (Some(name), Some(family)) = (
        style.attribute(STYLE, "name"),
        style.attribute(STYLE, "family"),
    ) {
        map.insert((family.to_owned(), name.to_owned()), style.clone());
    }
}

fn insert_list(map: &mut HashMap<String, Element>, style: &Element) {
    if let Some(name) = style.attribute(STYLE, "name") {
        map.insert(name.to_owned(), style.clone());
    }
}

/// A font family as `svg:font-family` writes it, quotes removed.
fn unquote(family: &str) -> String {
    family
        .trim()
        .trim_matches(|quote| quote == '\'' || quote == '"')
        .to_owned()
}

/// The look and alignment of a `style:paragraph-properties`.
pub(super) fn paragraph_look_of(properties: &Element) -> (ParagraphLook, Option<Alignment>) {
    let attribute = |namespace: Namespace, name: &str| properties.attribute(namespace, name);
    let unsigned = |name: &str| {
        attribute(FO, name)
            .and_then(twips)
            .map(|twips| u32::try_from(twips.max(0)).unwrap_or(0))
    };
    let line_spacing = if let Some(least) = attribute(STYLE, "line-height-at-least").and_then(twips)
    {
        Some(LineSpacing::AtLeast(
            u32::try_from(least.max(0)).unwrap_or(0),
        ))
    } else {
        attribute(FO, "line-height").and_then(|height| {
            if let Some(percent) = height.strip_suffix('%') {
                let percent = percent.trim().parse::<f64>().ok()?;
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "the multiple is clamped to a small positive range"
                )]
                let multiple = (percent * 2.4).round().clamp(24.0, 2_400.0) as u32;
                Some(LineSpacing::Auto(multiple))
            } else {
                twips(height)
                    .map(|exact| LineSpacing::Exact(u32::try_from(exact.max(0)).unwrap_or(0)))
            }
        })
    };
    let border = |name: &str| attribute(FO, name).map(border);
    let all = border("border");
    let look = ParagraphLook {
        space_before_twips: unsigned("margin-top"),
        space_after_twips: unsigned("margin-bottom"),
        line_spacing,
        indent_left_twips: attribute(FO, "margin-left").and_then(twips),
        indent_right_twips: attribute(FO, "margin-right").and_then(twips),
        indent_first_line_twips: attribute(FO, "text-indent").and_then(twips),
        page_break_before: attribute(FO, "break-before") == Some("page"),
        tabs: properties
            .child(STYLE, "tab-stops")
            .map(tab_stops)
            .unwrap_or_default(),
        shading: attribute(FO, "background-color").and_then(colour),
        borders: Borders {
            top: border("border-top").or_else(|| all.clone()),
            left: border("border-left").or_else(|| all.clone()),
            bottom: border("border-bottom").or_else(|| all.clone()),
            right: border("border-right").or_else(|| all.clone()),
            inside_horizontal: None,
            inside_vertical: None,
        },
        ..ParagraphLook::default()
    };
    let align = attribute(FO, "text-align").and_then(|align| match align {
        "start" | "left" => Some(Alignment::Left),
        "center" => Some(Alignment::Center),
        "end" | "right" => Some(Alignment::Right),
        "justify" => Some(Alignment::Justify),
        _ => None,
    });
    (look, align)
}

fn tab_stops(stops: &Element) -> Vec<TabStop> {
    let mut tabs: Vec<TabStop> = stops
        .elements()
        .filter(|stop| stop.is(STYLE, "tab-stop"))
        .take(64)
        .filter_map(|stop| {
            let position = stop.attribute(STYLE, "position").and_then(twips)?;
            let kind = match stop.attribute(STYLE, "type") {
                Some("center") => TabKind::Center,
                Some("right") => TabKind::Right,
                Some("char") => TabKind::Decimal,
                _ => TabKind::Left,
            };
            let leader = match stop.attribute(STYLE, "leader-text").or_else(|| {
                stop.attribute(STYLE, "leader-style")
                    .filter(|style| *style != "none")
                    .map(|_| ".")
            }) {
                Some(".") => TabLeader::Dot,
                Some("-") => TabLeader::Hyphen,
                Some("_") => TabLeader::Underscore,
                Some("\u{b7}") => TabLeader::MiddleDot,
                _ => TabLeader::None,
            };
            Some(TabStop {
                position_twips: position,
                kind,
                leader,
            })
        })
        .collect();
    tabs.sort_by_key(|tab| tab.position_twips);
    tabs
}

/// A border as `fo:border` writes it: `0.06pt solid #000000`.
pub(super) fn border(value: &str) -> Border {
    if value.trim() == "none" {
        return Border {
            style: BorderStyle::None,
            width_eighths: 0,
            colour: None,
        };
    }
    let mut width = None;
    let mut style = BorderStyle::Single;
    let mut found_colour = None;
    for part in value.split_whitespace() {
        if let Some(points) = points(part) {
            width = Some(points);
        } else if let Some(found) = colour(part) {
            found_colour = Some(found);
        } else {
            style = match part {
                "double" => BorderStyle::Double,
                "dotted" => BorderStyle::Dotted,
                "dashed" => BorderStyle::Dashed,
                "none" | "hidden" => BorderStyle::None,
                _ => style,
            };
        }
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the width is clamped to a small positive range"
    )]
    let width_eighths = (width.unwrap_or(0.75) * 8.0).round().clamp(1.0, 96.0) as u32;
    Border {
        style,
        width_eighths,
        colour: found_colour,
    }
}

/// A cell's vertical alignment.
pub(super) fn vertical_alignment(value: &str) -> Option<VerticalAlignment> {
    match value {
        "top" => Some(VerticalAlignment::Top),
        "middle" => Some(VerticalAlignment::Center),
        "bottom" => Some(VerticalAlignment::Bottom),
        _ => None,
    }
}

/// A colour as `#RRGGBB`, upper case without the hash; `transparent` is none.
pub(super) fn colour(value: &str) -> Option<String> {
    let hex = value.trim().strip_prefix('#')?;
    (hex.len() == 6 && hex.chars().all(|character| character.is_ascii_hexdigit()))
        .then(|| hex.to_ascii_uppercase())
}

/// A length in points: `2cm`, `20mm`, `1in`, `12pt`, `1pc`, `96px`.
pub(super) fn points(length: &str) -> Option<f64> {
    let length = length.trim();
    let split = length
        .find(|character: char| character.is_ascii_alphabetic())
        .unwrap_or(length.len());
    let (number, unit) = length.split_at(split);
    let number = number
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())?;
    let factor = match unit {
        "pt" => 1.0,
        "cm" => 72.0 / 2.54,
        "mm" => 72.0 / 25.4,
        "in" | "inch" => 72.0,
        "pc" => 12.0,
        "px" => 0.75,
        _ => return None,
    };
    Some(number * factor)
}

/// A length in twentieths of a point.
pub(super) fn twips(length: &str) -> Option<i32> {
    let points = points(length)?;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the length is clamped to the size of a page"
    )]
    let twips = (points * TWIPS_PER_POINT)
        .round()
        .clamp(-MAX_TWIPS, MAX_TWIPS) as i32;
    Some(twips)
}

#[cfg(test)]
mod tests;
