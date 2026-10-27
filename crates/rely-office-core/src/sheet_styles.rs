// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The cell styles of an xlsx (`xl/styles.xml`): fonts, fills, borders, alignment and number
//! formats, resolved into one style per cell format, with theme and palette colours turned into
//! plain colours.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::document::{Border, BorderStyle, VerticalAlignment};
use crate::number_format::builtin_format_code;
use crate::sheet_rules::DifferentialLook;
use crate::xml_text::attribute_value;

/// The most cell formats read; Excel's own limit is 64,000.
const MAX_FORMATS: usize = 65_536;

/// How a cell's content sits across its width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub enum HorizontalAlignment {
    Left,
    Center,
    Right,
    Justify,
}

/// The font of a cell.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is a font property a cell format sets on its own"
)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase", default)
)]
pub struct CellFont {
    pub family: Option<String>,
    /// The size in tenths of a point.
    pub size_tenths: Option<u32>,
    /// The colour as `RRGGBB`; unset is automatic, black.
    pub colour: Option<String>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
}

/// The borders of a cell.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase", default)
)]
pub struct CellBorders {
    pub top: Option<Border>,
    pub right: Option<Border>,
    pub bottom: Option<Border>,
    pub left: Option<Border>,
}

impl CellBorders {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        [&self.top, &self.right, &self.bottom, &self.left]
            .into_iter()
            .all(|edge| edge.as_ref().is_none_or(|edge| !edge.is_visible()))
    }
}

/// How a cell looks: what an xlsx keeps in one of its cell formats.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase", default)
)]
pub struct CellStyle {
    pub font: CellFont,
    /// The background as `RRGGBB`.
    pub fill: Option<String>,
    pub borders: CellBorders,
    pub horizontal: Option<HorizontalAlignment>,
    pub vertical: Option<VerticalAlignment>,
    /// Long text wraps inside the cell instead of running over its neighbours.
    pub wrap: bool,
    /// The number format code; unset is General.
    pub number_format: Option<String>,
}

/// The twelve colours of a theme in the order cells refer to them: light 1, dark 1, light 2,
/// dark 2, six accents, hyperlink and followed hyperlink.
pub(crate) type ThemeColours = [String; 12];

/// The colours of Office's default theme.
pub(crate) fn default_theme() -> ThemeColours {
    [
        "FFFFFF", "000000", "E7E6E6", "44546A", "4472C4", "ED7D31", "A5A5A5", "FFC000", "5B9BD5",
        "70AD47", "0563C1", "954F72",
    ]
    .map(str::to_owned)
}

/// The colours of a theme part, `a:clrScheme`, in the order cells refer to them.
pub(crate) fn theme_colours(xml: &str) -> Option<ThemeColours> {
    const ORDER: [&str; 12] = [
        "lt1", "dk1", "lt2", "dk2", "accent1", "accent2", "accent3", "accent4", "accent5",
        "accent6", "hlink", "folHlink",
    ];
    let mut colours = default_theme();
    let mut reader = Reader::from_str(xml);
    let mut current: Option<usize> = None;
    let mut found = false;
    loop {
        match reader.read_event().ok()? {
            Event::Start(element) | Event::Empty(element) => {
                let name = element.local_name();
                let name: &str = name.as_ref();
                if let Some(index) = ORDER.iter().position(|slot| *slot == name) {
                    current = Some(index);
                } else if let Some(index) = current {
                    let value = match name {
                        "srgbClr" => attribute(&element, "val"),
                        "sysClr" => attribute(&element, "lastClr"),
                        _ => None,
                    };
                    if let Some(value) = value.and_then(|value| hex(&value)) {
                        colours[index] = value;
                        found = true;
                        current = None;
                    }
                }
            }
            Event::End(element) if element.local_name().as_ref() == "clrScheme" => break,
            Event::Eof => break,
            _ => {}
        }
    }
    found.then_some(colours)
}

#[derive(Debug, Clone, Default)]
struct Format {
    number_format_id: u32,
    font: usize,
    fill: usize,
    border: usize,
    horizontal: Option<HorizontalAlignment>,
    vertical: Option<VerticalAlignment>,
    wrap: bool,
}

/// The list of the styles part being read; the differential formats of conditional formatting
/// hold fonts, fills and borders too, which are not the cells' own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Other,
    /// The differential formats of conditional formats, which no cell format names.
    Differential,
    Fonts,
    Fills,
    Borders,
    CellFormats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edge {
    Top,
    Right,
    Bottom,
    Left,
}

/// The parts of a styles part, each a list cell formats refer to by index.
#[derive(Debug, Default)]
struct Parts {
    number_formats: Vec<(u32, String)>,
    fonts: Vec<CellFont>,
    fills: Vec<Option<String>>,
    /// Whether the pattern of the fill being read paints its foreground colour.
    solid: bool,
    borders: Vec<CellBorders>,
    edge: Option<Edge>,
    formats: Vec<Format>,
}

/// One style per cell format of the styles part, `cellXfs`, in order.
pub(crate) fn cell_styles(xml: &str, theme: &ThemeColours) -> Vec<CellStyle> {
    let mut parts = Parts::default();
    let mut section = Section::Other;
    let mut reader = Reader::from_str(xml);
    loop {
        let (element, empty) = match reader.read_event() {
            Ok(Event::Eof) | Err(_) => break,
            Ok(Event::Start(element)) => (element, false),
            Ok(Event::Empty(element)) => (element, true),
            Ok(Event::End(element)) => {
                match element.local_name().as_ref() {
                    "fonts" | "fills" | "borders" | "cellXfs" | "dxfs" => {
                        section = Section::Other;
                    }
                    "left" | "right" | "top" | "bottom" | "start" | "end" => {
                        parts.edge = None;
                    }
                    _ => {}
                }
                continue;
            }
            Ok(_) => continue,
        };
        let name = element.local_name();
        match (section, name.as_ref()) {
            (Section::Differential, _) => {}
            (_, "dxfs") if !empty => section = Section::Differential,
            (_, "numFmt") => {
                if let (Some(id), Some(code)) = (
                    attribute(&element, "numFmtId").and_then(|id| id.parse().ok()),
                    attribute(&element, "formatCode"),
                ) {
                    parts.number_formats.push((id, code));
                }
            }
            (_, "fonts") if !empty => section = Section::Fonts,
            (_, "fills") if !empty => section = Section::Fills,
            (_, "borders") if !empty => section = Section::Borders,
            (_, "cellXfs") if !empty => section = Section::CellFormats,
            (Section::Fonts, "font") => parts.fonts.push(CellFont::default()),
            (Section::Fonts, _) => read_font(&element, theme, &mut parts),
            (Section::Fills, "fill") => {
                parts.fills.push(None);
                parts.solid = false;
            }
            (Section::Fills, _) => read_fill(&element, theme, &mut parts),
            (Section::Borders, "border") => parts.borders.push(CellBorders::default()),
            (Section::Borders, _) => read_border(&element, theme, &mut parts),
            (Section::CellFormats, "xf") if parts.formats.len() < MAX_FORMATS => {
                let index = |name: &str| {
                    attribute(&element, name)
                        .and_then(|value| value.parse::<usize>().ok())
                        .unwrap_or(0)
                };
                parts.formats.push(Format {
                    number_format_id: u32::try_from(index("numFmtId")).unwrap_or(0),
                    font: index("fontId"),
                    fill: index("fillId"),
                    border: index("borderId"),
                    ..Format::default()
                });
            }
            (Section::CellFormats, "alignment") => {
                if let Some(format) = parts.formats.last_mut() {
                    read_alignment(&element, format);
                }
            }
            _ => {}
        }
    }
    resolve(&parts)
}

/// The differential formats of the styles part, `dxfs`, in order: what each conditional rule
/// changes in the look of a cell.
pub(crate) fn differential_looks(xml: &str, theme: &ThemeColours) -> Vec<DifferentialLook> {
    let mut looks = Vec::new();
    let mut inside = false;
    let mut in_fill = false;
    let mut in_font = false;
    let mut reader = Reader::from_str(xml);
    loop {
        let (element, empty) = match reader.read_event() {
            Ok(Event::Eof) | Err(_) => break,
            Ok(Event::Start(element)) => (element, false),
            Ok(Event::Empty(element)) => (element, true),
            Ok(Event::End(element)) => {
                match element.local_name().as_ref() {
                    "dxfs" => inside = false,
                    "fill" => in_fill = false,
                    "font" => in_font = false,
                    _ => {}
                }
                continue;
            }
            Ok(_) => continue,
        };
        let name = element.local_name();
        if !inside {
            inside = name.as_ref() == "dxfs" && !empty;
            continue;
        }
        let on = || attribute(&element, "val").is_none_or(|value| is_on(Some(value)));
        match name.as_ref() {
            "dxf" if looks.len() < MAX_FORMATS => looks.push(DifferentialLook::default()),
            "font" => in_font = !empty,
            "fill" => in_fill = !empty,
            _ => {
                let Some(look) = looks.last_mut() else {
                    continue;
                };
                match name.as_ref() {
                    "b" if in_font => look.bold = Some(on()),
                    "i" if in_font => look.italic = Some(on()),
                    "strike" if in_font => look.strike = Some(on()),
                    "u" if in_font => {
                        look.underline =
                            Some(attribute(&element, "val").is_none_or(|value| value != "none"));
                    }
                    "color" if in_font => look.font_colour = colour(&element, theme),
                    // A differential fill keeps a solid colour in its background colour.
                    "bgColor" if in_fill => look.fill = colour(&element, theme),
                    "fgColor" if in_fill && look.fill.is_none() => {
                        look.fill = colour(&element, theme);
                    }
                    "numFmt" => look.number_format = attribute(&element, "formatCode"),
                    _ => {}
                }
            }
        }
    }
    looks
}

fn resolve(parts: &Parts) -> Vec<CellStyle> {
    parts
        .formats
        .iter()
        .map(|format| CellStyle {
            font: parts.fonts.get(format.font).cloned().unwrap_or_default(),
            fill: parts.fills.get(format.fill).cloned().flatten(),
            borders: parts
                .borders
                .get(format.border)
                .cloned()
                .unwrap_or_default(),
            horizontal: format.horizontal,
            vertical: format.vertical,
            wrap: format.wrap,
            number_format: parts
                .number_formats
                .iter()
                .find(|(id, _)| *id == format.number_format_id)
                .map(|(_, code)| code.clone())
                .or_else(|| builtin_format_code(format.number_format_id).map(str::to_owned))
                .filter(|code| !code.eq_ignore_ascii_case("general")),
        })
        .collect()
}

fn read_alignment(element: &BytesStart<'_>, format: &mut Format) {
    format.horizontal = attribute(element, "horizontal").and_then(|value| match value.as_str() {
        "left" => Some(HorizontalAlignment::Left),
        "center" | "centerContinuous" => Some(HorizontalAlignment::Center),
        "right" => Some(HorizontalAlignment::Right),
        "justify" | "distributed" => Some(HorizontalAlignment::Justify),
        _ => None,
    });
    format.vertical = attribute(element, "vertical").and_then(|value| match value.as_str() {
        "top" => Some(VerticalAlignment::Top),
        "center" => Some(VerticalAlignment::Center),
        "bottom" => Some(VerticalAlignment::Bottom),
        _ => None,
    });
    format.wrap = is_on(attribute(element, "wrapText"));
}

fn read_font(element: &BytesStart<'_>, theme: &ThemeColours, parts: &mut Parts) {
    let Some(font) = parts.fonts.last_mut() else {
        return;
    };
    let on = || attribute(element, "val").is_none_or(|value| is_on(Some(value)));
    match element.local_name().as_ref() {
        "b" => font.bold = on(),
        "i" => font.italic = on(),
        "strike" => font.strike = on(),
        "u" => font.underline = attribute(element, "val").is_none_or(|value| value != "none"),
        "sz" => {
            font.size_tenths = attribute(element, "val")
                .and_then(|value| value.parse::<f64>().ok())
                .filter(|size| (1.0..=409.0).contains(size))
                .map(tenths);
        }
        "name" | "rFont" => font.family = attribute(element, "val"),
        "color" => font.colour = colour(element, theme).filter(|colour| colour != "000000"),
        _ => {}
    }
}

fn read_fill(element: &BytesStart<'_>, theme: &ThemeColours, parts: &mut Parts) {
    let solid = &mut parts.solid;
    let Some(fill) = parts.fills.last_mut() else {
        return;
    };
    match element.local_name().as_ref() {
        "patternFill" => {
            *solid = attribute(element, "patternType").is_some_and(|pattern| pattern != "none");
        }
        "fgColor" if *solid => *fill = colour(element, theme),
        // A gradient is drawn as its first colour.
        "color" if fill.is_none() => *fill = colour(element, theme),
        _ => {}
    }
}

fn read_border(element: &BytesStart<'_>, theme: &ThemeColours, parts: &mut Parts) {
    let Some(borders) = parts.borders.last_mut() else {
        return;
    };
    let edge = match element.local_name().as_ref() {
        "top" => Some(Edge::Top),
        "right" | "end" => Some(Edge::Right),
        "bottom" => Some(Edge::Bottom),
        "left" | "start" => Some(Edge::Left),
        "color" => {
            if let Some(edge) = parts.edge
                && let Some(border) = slot(borders, edge).as_mut()
            {
                border.colour = colour(element, theme).filter(|colour| colour != "000000");
            }
            None
        }
        _ => None,
    };
    if let Some(edge) = edge {
        *slot(borders, edge) = attribute(element, "style").map(|style| Border {
            style: border_style(&style),
            width_eighths: border_width(&style),
            colour: None,
        });
        parts.edge = Some(edge);
    }
}

fn slot(borders: &mut CellBorders, edge: Edge) -> &mut Option<Border> {
    match edge {
        Edge::Top => &mut borders.top,
        Edge::Right => &mut borders.right,
        Edge::Bottom => &mut borders.bottom,
        Edge::Left => &mut borders.left,
    }
}

fn border_style(style: &str) -> BorderStyle {
    match style {
        "none" => BorderStyle::None,
        "double" => BorderStyle::Double,
        "dotted" | "hair" => BorderStyle::Dotted,
        "dashed" | "mediumDashed" | "dashDot" | "mediumDashDot" | "dashDotDot"
        | "mediumDashDotDot" | "slantDashDot" => BorderStyle::Dashed,
        _ => BorderStyle::Single,
    }
}

/// A border's width in eighths of a point: thin is half a point, medium one, thick one and a half.
fn border_width(style: &str) -> u32 {
    match style {
        "hair" => 2,
        "medium" | "mediumDashed" | "mediumDashDot" | "mediumDashDotDot" | "slantDashDot"
        | "double" => 8,
        "thick" => 12,
        _ => 4,
    }
}

/// A colour element: an `rgb`, a `theme` colour with its `tint`, or an `indexed` one.
pub(crate) fn colour(element: &BytesStart<'_>, theme: &ThemeColours) -> Option<String> {
    let base = if let Some(rgb) = attribute(element, "rgb") {
        hex(&rgb)?
    } else if let Some(index) =
        attribute(element, "theme").and_then(|value| value.parse::<usize>().ok())
    {
        theme.get(index)?.clone()
    } else {
        let index = attribute(element, "indexed")?.parse::<usize>().ok()?;
        (*INDEXED.get(index)?).to_owned()
    };
    let tint = attribute(element, "tint")
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|tint| tint.is_finite() && *tint != 0.0)
        .map(|tint| tint.clamp(-1.0, 1.0));
    Some(match tint {
        Some(tint) => tinted(&base, tint),
        None => base,
    })
}

/// `RRGGBB` from `RRGGBB` or `AARRGGBB`.
fn hex(value: &str) -> Option<String> {
    let value = value.trim();
    let value = if value.len() == 8 { &value[2..] } else { value };
    (value.len() == 6 && value.chars().all(|character| character.is_ascii_hexdigit()))
        .then(|| value.to_ascii_uppercase())
}

/// A colour lightened (positive tint) or darkened (negative) on its luminance, as Excel tints.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "channels are clamped to 0..=255 before the conversion"
)]
fn tinted(colour: &str, tint: f64) -> String {
    let channel = |start: usize| {
        f64::from(u8::from_str_radix(colour.get(start..start + 2).unwrap_or("00"), 16).unwrap_or(0))
            / 255.0
    };
    let (red, green, blue) = (channel(0), channel(2), channel(4));
    let (hue, saturation, mut lightness) = to_hsl(red, green, blue);
    lightness = if tint < 0.0 {
        lightness * (1.0 + tint)
    } else {
        lightness * (1.0 - tint) + tint
    };
    let (red, green, blue) = from_hsl(hue, saturation, lightness.clamp(0.0, 1.0));
    let byte = |value: f64| (value * 255.0).round().clamp(0.0, 255.0) as u8;
    format!("{:02X}{:02X}{:02X}", byte(red), byte(green), byte(blue))
}

pub(crate) fn to_hsl(red: f64, green: f64, blue: f64) -> (f64, f64, f64) {
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let lightness = f64::midpoint(max, min);
    if (max - min).abs() < f64::EPSILON {
        return (0.0, 0.0, lightness);
    }
    let delta = max - min;
    let saturation = if lightness > 0.5 {
        delta / (2.0 - max - min)
    } else {
        delta / (max + min)
    };
    let hue = if (max - red).abs() < f64::EPSILON {
        (green - blue) / delta + if green < blue { 6.0 } else { 0.0 }
    } else if (max - green).abs() < f64::EPSILON {
        (blue - red) / delta + 2.0
    } else {
        (red - green) / delta + 4.0
    } / 6.0;
    (hue, saturation, lightness)
}

pub(crate) fn from_hsl(hue: f64, saturation: f64, lightness: f64) -> (f64, f64, f64) {
    if saturation == 0.0 {
        return (lightness, lightness, lightness);
    }
    let q = if lightness < 0.5 {
        lightness * (1.0 + saturation)
    } else {
        lightness + saturation - lightness * saturation
    };
    let p = 2.0 * lightness - q;
    let channel = |mut t: f64| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    (
        channel(hue + 1.0 / 3.0),
        channel(hue),
        channel(hue - 1.0 / 3.0),
    )
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the size is checked to lie between 1 and 409 points"
)]
fn tenths(size: f64) -> u32 {
    (size * 10.0).round() as u32
}

fn is_on(value: Option<String>) -> bool {
    value.is_some_and(|value| value == "1" || value == "true")
}

fn attribute(element: &BytesStart<'_>, name: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.local_name().as_ref() == name)
        .and_then(|attribute| attribute_value(&attribute))
}

/// The legacy palette `indexed` colours name, as Excel defines it; 64 and 65 are the system
/// foreground and background.
const INDEXED: [&str; 66] = [
    "000000", "FFFFFF", "FF0000", "00FF00", "0000FF", "FFFF00", "FF00FF", "00FFFF", "000000",
    "FFFFFF", "FF0000", "00FF00", "0000FF", "FFFF00", "FF00FF", "00FFFF", "800000", "008000",
    "000080", "808000", "800080", "008080", "C0C0C0", "808080", "9999FF", "993366", "FFFFCC",
    "CCFFFF", "660066", "FF8080", "0066CC", "CCCCFF", "000080", "FF00FF", "FFFF00", "00FFFF",
    "800080", "800000", "008080", "0000FF", "00CCFF", "CCFFFF", "CCFFCC", "FFFF99", "99CCFF",
    "FF99CC", "CC99FF", "FFCC99", "3366FF", "33CCCC", "99CC00", "FFCC00", "FF9900", "FF6600",
    "666699", "969696", "003366", "339966", "003300", "333300", "993300", "993366", "333399",
    "333333", "000000", "FFFFFF",
];

#[cfg(test)]
mod tests;
