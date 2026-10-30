// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! The colours of `DrawingML`: plain ones, the theme's, the system's and the named ones, with the
//! luminance, tint and shade changes a colour element carries.

use std::collections::HashMap;

use crate::sheet_styles::{from_hsl, to_hsl};
use crate::xml_tree::{Element, Namespace, parse_tree};

const A: Namespace = Namespace::Drawing;
const PERCENT: f64 = 100_000.0;

/// The colours and fonts of a theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Theme {
    colours: HashMap<String, String>,
    pub major_font: String,
    pub minor_font: String,
}

impl Default for Theme {
    /// Office's default theme.
    fn default() -> Self {
        let colours = [
            ("dk1", "000000"),
            ("lt1", "FFFFFF"),
            ("dk2", "44546A"),
            ("lt2", "E7E6E6"),
            ("accent1", "4472C4"),
            ("accent2", "ED7D31"),
            ("accent3", "A5A5A5"),
            ("accent4", "FFC000"),
            ("accent5", "5B9BD5"),
            ("accent6", "70AD47"),
            ("hlink", "0563C1"),
            ("folHlink", "954F72"),
        ]
        .into_iter()
        .map(|(name, colour)| (name.to_owned(), colour.to_owned()))
        .collect();
        Self {
            colours,
            major_font: "Calibri Light".to_owned(),
            minor_font: "Calibri".to_owned(),
        }
    }
}

impl Theme {
    /// A theme part; what it does not hold keeps Office's defaults.
    pub(crate) fn parse(xml: &str) -> Self {
        let mut theme = Self::default();
        let Ok(root) = parse_tree(xml, |_, _| false) else {
            return theme;
        };
        if let Some(scheme) = root.descendant(A, "clrScheme") {
            for slot in scheme.elements() {
                let value = slot
                    .elements()
                    .find_map(|colour| match colour.name.as_str() {
                        "srgbClr" => colour.attribute(Namespace::None, "val").and_then(hex),
                        "sysClr" => colour.attribute(Namespace::None, "lastClr").and_then(hex),
                        _ => None,
                    });
                if let Some(value) = value {
                    theme.colours.insert(slot.name.clone(), value);
                }
            }
        }
        if let Some(fonts) = root.descendant(A, "fontScheme") {
            let face = |name: &str| {
                fonts
                    .child(A, name)
                    .and_then(|font| font.child(A, "latin"))
                    .and_then(|latin| latin.attribute(Namespace::None, "typeface"))
                    .filter(|typeface| !typeface.is_empty())
                    .map(str::to_owned)
            };
            if let Some(major) = face("majorFont") {
                theme.major_font = major;
            }
            if let Some(minor) = face("minorFont") {
                theme.minor_font = minor;
            }
        }
        theme
    }

    /// A theme colour by its name, the master's colour map applied to the aliases of text and
    /// background.
    pub(crate) fn scheme(&self, name: &str) -> Option<String> {
        let name = match name {
            // A placeholder colour stands for the shape's own text colour; dark is the default.
            "tx1" | "phClr" => "dk1",
            "bg1" => "lt1",
            "tx2" => "dk2",
            "bg2" => "lt2",
            other => other,
        };
        self.colours.get(name).cloned()
    }

    /// A typeface that may name a theme font: `+mj-lt` the major, `+mn-lt` the minor.
    pub(crate) fn typeface(&self, typeface: &str) -> String {
        match typeface {
            "+mj-lt" | "+mj-ea" | "+mj-cs" => self.major_font.clone(),
            "+mn-lt" | "+mn-ea" | "+mn-cs" => self.minor_font.clone(),
            other => other.to_owned(),
        }
    }
}

/// The colour an element holding one colour child shows: `a:solidFill`, `a:fgClr`, a gradient
/// stop.
pub(crate) fn colour_in(parent: &Element, theme: &Theme) -> Option<String> {
    parent
        .elements()
        .find_map(|colour| colour_element(colour, theme))
}

/// The colour of one colour element, with its modifiers.
pub(crate) fn colour_element(colour: &Element, theme: &Theme) -> Option<String> {
    if colour.namespace != A {
        return None;
    }
    let base = match colour.name.as_str() {
        "srgbClr" => colour.attribute(Namespace::None, "val").and_then(hex),
        "schemeClr" => colour
            .attribute(Namespace::None, "val")
            .and_then(|name| theme.scheme(name)),
        "sysClr" => colour
            .attribute(Namespace::None, "lastClr")
            .and_then(hex)
            .or_else(|| {
                colour
                    .attribute(Namespace::None, "val")
                    .map(|name| if name == "window" { "FFFFFF" } else { "000000" }.to_owned())
            }),
        "prstClr" => colour
            .attribute(Namespace::None, "val")
            .and_then(named)
            .map(str::to_owned),
        _ => None,
    }?;
    Some(modified(&base, colour))
}

/// A colour with the luminance, tint and shade changes of its element applied.
fn modified(base: &str, colour: &Element) -> String {
    let channel = |start: usize| {
        f64::from(u8::from_str_radix(base.get(start..start + 2).unwrap_or("00"), 16).unwrap_or(0))
            / 255.0
    };
    let mut rgb = [channel(0), channel(2), channel(4)];
    let value = |modifier: &Element| {
        modifier
            .attribute(Namespace::None, "val")
            .and_then(|value| value.parse::<f64>().ok())
            .map(|value| value / PERCENT)
    };
    for modifier in colour.elements() {
        let Some(amount) = value(modifier) else {
            continue;
        };
        match modifier.name.as_str() {
            "lumMod" | "lumOff" => {
                let (hue, saturation, lightness) = to_hsl(rgb[0], rgb[1], rgb[2]);
                let lightness = if modifier.name == "lumMod" {
                    lightness * amount
                } else {
                    lightness + amount
                };
                let (red, green, blue) = from_hsl(hue, saturation, lightness.clamp(0.0, 1.0));
                rgb = [red, green, blue];
            }
            "tint" => rgb = rgb.map(|value| value * amount + (1.0 - amount)),
            "shade" => rgb = rgb.map(|value| value * amount),
            _ => {}
        }
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "channels are clamped to 0..=255 before the conversion"
    )]
    let byte = |value: f64| (value * 255.0).round().clamp(0.0, 255.0) as u8;
    format!(
        "{:02X}{:02X}{:02X}",
        byte(rgb[0]),
        byte(rgb[1]),
        byte(rgb[2])
    )
}

fn hex(value: &str) -> Option<String> {
    (value.len() == 6 && value.chars().all(|character| character.is_ascii_hexdigit()))
        .then(|| value.to_ascii_uppercase())
}

fn named(name: &str) -> Option<&'static str> {
    Some(match name {
        "black" => "000000",
        "white" => "FFFFFF",
        "red" => "FF0000",
        "green" => "008000",
        "blue" => "0000FF",
        "yellow" => "FFFF00",
        "gray" | "grey" => "808080",
        "orange" => "FFA500",
        "purple" => "800080",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element(xml: &str) -> Element {
        parse_tree(
            &format!(
                r#"<a:root xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">{xml}</a:root>"#
            ),
            |_, _| false,
        )
        .unwrap()
        .elements()
        .next()
        .unwrap()
        .clone()
    }

    #[test]
    fn reads_the_colours_and_fonts_of_a_theme() {
        let theme = Theme::parse(concat!(
            r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:themeElements>"#,
            r#"<a:clrScheme name="R"><a:dk1><a:sysClr val="windowText" lastClr="111111"/></a:dk1>"#,
            r#"<a:accent1><a:srgbClr val="ff6600"/></a:accent1></a:clrScheme>"#,
            r#"<a:fontScheme name="F"><a:majorFont><a:latin typeface="Georgia"/></a:majorFont>"#,
            r#"<a:minorFont><a:latin typeface=""/></a:minorFont></a:fontScheme></a:themeElements></a:theme>"#
        ));
        assert_eq!(theme.scheme("tx1").as_deref(), Some("111111"));
        assert_eq!(theme.scheme("accent1").as_deref(), Some("FF6600"));
        assert_eq!(theme.scheme("bg1").as_deref(), Some("FFFFFF"));
        assert_eq!(theme.scheme("nothing"), None);
        assert_eq!(theme.typeface("+mj-lt"), "Georgia");
        assert_eq!(theme.typeface("+mn-lt"), "Calibri");
        assert_eq!(theme.typeface("Arial"), "Arial");
        assert_eq!(Theme::parse("<broken"), Theme::default());
    }

    #[test]
    fn applies_the_modifiers_of_a_colour() {
        let theme = Theme::default();
        let colour = |xml: &str| colour_element(&element(xml), &theme);
        assert_eq!(
            colour(r#"<a:srgbClr val="4472c4"/>"#).as_deref(),
            Some("4472C4")
        );
        assert_eq!(
            colour(r#"<a:schemeClr val="tx1"><a:lumMod val="65000"/><a:lumOff val="35000"/></a:schemeClr>"#)
                .as_deref(),
            Some("595959"),
            "Text 1, lighter 35%"
        );
        assert_eq!(
            colour(r#"<a:srgbClr val="FF0000"><a:tint val="50000"/></a:srgbClr>"#).as_deref(),
            Some("FF8080")
        );
        assert_eq!(
            colour(
                r#"<a:srgbClr val="FF0000"><a:shade val="50000"/><a:alpha val="1"/></a:srgbClr>"#
            )
            .as_deref(),
            Some("800000")
        );
        assert_eq!(
            colour(r#"<a:sysClr val="window"/>"#).as_deref(),
            Some("FFFFFF")
        );
        assert_eq!(
            colour(r#"<a:sysClr val="windowText"/>"#).as_deref(),
            Some("000000")
        );
        assert_eq!(
            colour(r#"<a:prstClr val="red"/>"#).as_deref(),
            Some("FF0000")
        );
        assert_eq!(colour(r#"<a:prstClr val="teal"/>"#), None);
        assert_eq!(colour(r#"<a:srgbClr val="xyz"/>"#), None);
        assert_eq!(colour(r#"<a:hslClr hue="0"/>"#), None);
        let fill = element(r#"<a:solidFill><a:schemeClr val="accent2"/></a:solidFill>"#);
        assert_eq!(colour_in(&fill, &theme).as_deref(), Some("ED7D31"));
    }
}
