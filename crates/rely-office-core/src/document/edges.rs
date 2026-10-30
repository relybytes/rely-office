// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! Borders, shading and tab stops: what paragraphs and table cells draw around and inside them.

use crate::xml_tree::{Element, Namespace};

const W: Namespace = Namespace::Word;
/// Word draws borders from a quarter of a point to six points, in eighths.
const MIN_BORDER_EIGHTHS: u32 = 2;
const MAX_BORDER_EIGHTHS: u32 = 96;
const DEFAULT_BORDER_EIGHTHS: u32 = 4;
/// Word's own limit of tab stops per paragraph.
const MAX_TAB_STOPS: usize = 64;
const MAX_TWIPS: i64 = 31_680;

/// How a border line is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub enum BorderStyle {
    /// Explicitly no line, which hides the line a style would draw.
    None,
    Single,
    Double,
    Dotted,
    Dashed,
}

/// One edge of a border.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct Border {
    pub style: BorderStyle,
    /// The width in eighths of a point.
    pub width_eighths: u32,
    /// The colour as `RRGGBB`; unset means automatic, black.
    pub colour: Option<String>,
}

impl Border {
    /// Read an edge element (`w:top`, `w:insideH`, ...).
    pub(crate) fn parse(edge: &Element) -> Option<Self> {
        let style = match edge.attribute(W, "val")? {
            "nil" | "none" => BorderStyle::None,
            "double" | "triple" | "thinThickSmallGap" | "thickThinSmallGap"
            | "thinThickMediumGap" | "thickThinMediumGap" | "thinThickLargeGap"
            | "thickThinLargeGap" => BorderStyle::Double,
            "dotted" | "dotDash" | "dotDotDash" => BorderStyle::Dotted,
            "dashed" | "dashSmallGap" | "dashDotStroked" => BorderStyle::Dashed,
            _ => BorderStyle::Single,
        };
        let width_eighths = edge
            .attribute(W, "sz")
            .and_then(|size| size.parse::<u32>().ok())
            .map_or(DEFAULT_BORDER_EIGHTHS, |size| {
                size.clamp(MIN_BORDER_EIGHTHS, MAX_BORDER_EIGHTHS)
            });
        Some(Self {
            style,
            width_eighths,
            colour: hex_colour(edge.attribute(W, "color")),
        })
    }

    /// Whether the edge draws a line.
    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.style != BorderStyle::None
    }
}

/// The edges of a paragraph, a cell or a table, each unset unless the document names it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Borders {
    pub top: Option<Border>,
    pub left: Option<Border>,
    pub bottom: Option<Border>,
    pub right: Option<Border>,
    /// Between rows of a table, or between paragraphs that share the same borders.
    pub inside_horizontal: Option<Border>,
    /// Between the columns of a table.
    pub inside_vertical: Option<Border>,
}

impl Borders {
    /// Read a borders element: `w:pBdr`, `w:tcBorders` or `w:tblBorders`.
    pub(crate) fn parse(borders: &Element) -> Self {
        let edge = |names: &[&str]| {
            names
                .iter()
                .find_map(|name| borders.child(W, name))
                .and_then(Border::parse)
        };
        Self {
            top: edge(&["top"]),
            left: edge(&["left", "start"]),
            bottom: edge(&["bottom"]),
            right: edge(&["right", "end"]),
            inside_horizontal: edge(&["insideH", "between"]),
            inside_vertical: edge(&["insideV"]),
        }
    }

    /// These edges with the ones they leave unset taken from `fallback`.
    #[must_use]
    pub(crate) fn or(self, fallback: Self) -> Self {
        Self {
            top: self.top.or(fallback.top),
            left: self.left.or(fallback.left),
            bottom: self.bottom.or(fallback.bottom),
            right: self.right.or(fallback.right),
            inside_horizontal: self.inside_horizontal.or(fallback.inside_horizontal),
            inside_vertical: self.inside_vertical.or(fallback.inside_vertical),
        }
    }

    /// Whether any edge draws a line.
    #[must_use]
    pub fn any_visible(&self) -> bool {
        [
            &self.top,
            &self.left,
            &self.bottom,
            &self.right,
            &self.inside_horizontal,
            &self.inside_vertical,
        ]
        .into_iter()
        .flatten()
        .any(Border::is_visible)
    }
}

/// Where the text after a tab lines up with a stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabKind {
    Left,
    Center,
    Right,
    /// On the decimal separator; drawn as a right stop.
    Decimal,
    /// A stop a style sets and the paragraph removes.
    Clear,
}

/// What fills the gap before a stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabLeader {
    None,
    Dot,
    Hyphen,
    Underscore,
    MiddleDot,
}

/// A tab stop, at a distance from the left margin in twentieths of a point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabStop {
    pub position_twips: i32,
    pub kind: TabKind,
    pub leader: TabLeader,
}

/// The stops of a `w:tabs` element, in the order of their position.
pub(crate) fn parse_tabs(tabs: &Element) -> Vec<TabStop> {
    let mut stops: Vec<TabStop> = tabs
        .elements()
        .filter(|tab| tab.is(W, "tab"))
        .filter_map(|tab| {
            let position = tab.attribute(W, "pos")?.parse::<i64>().ok()?;
            let kind = match tab.attribute(W, "val")? {
                "center" => TabKind::Center,
                "right" | "end" => TabKind::Right,
                "decimal" => TabKind::Decimal,
                "clear" => TabKind::Clear,
                "bar" | "num" => return None,
                _ => TabKind::Left,
            };
            let leader = match tab.attribute(W, "leader") {
                Some("dot") => TabLeader::Dot,
                Some("hyphen") => TabLeader::Hyphen,
                Some("underscore" | "heavy") => TabLeader::Underscore,
                Some("middleDot") => TabLeader::MiddleDot,
                _ => TabLeader::None,
            };
            Some(TabStop {
                position_twips: i32::try_from(position.clamp(-MAX_TWIPS, MAX_TWIPS)).ok()?,
                kind,
                leader,
            })
        })
        .take(MAX_TAB_STOPS)
        .collect();
    stops.sort_by_key(|stop| stop.position_twips);
    stops
}

/// A paragraph's own stops over its style's: a stop of its own replaces the style's at the same
/// position, and a cleared one removes it.
pub(crate) fn merge_tabs(own: Vec<TabStop>, inherited: Vec<TabStop>) -> Vec<TabStop> {
    if own.is_empty() {
        return inherited;
    }
    let mut merged: Vec<TabStop> = inherited
        .into_iter()
        .filter(|stop| {
            !own.iter()
                .any(|mine| mine.position_twips == stop.position_twips)
        })
        .collect();
    merged.extend(own);
    merged.sort_by_key(|stop| stop.position_twips);
    merged
}

/// How the content of a cell sits in its height.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub enum VerticalAlignment {
    Top,
    Center,
    Bottom,
}

/// How a table cell looks: its fill, its own borders and where its content sits.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CellLook {
    /// The background as `RRGGBB`.
    pub fill: Option<String>,
    pub borders: Borders,
    pub vertical: Option<VerticalAlignment>,
}

impl CellLook {
    /// Read `w:tcPr`.
    pub(crate) fn parse(properties: &Element) -> Self {
        Self {
            fill: properties.child(W, "shd").and_then(shading_fill),
            borders: properties
                .child(W, "tcBorders")
                .map(Borders::parse)
                .unwrap_or_default(),
            vertical: properties
                .child(W, "vAlign")
                .and_then(|align| align.attribute(W, "val"))
                .and_then(|value| match value {
                    "center" => Some(VerticalAlignment::Center),
                    "bottom" => Some(VerticalAlignment::Bottom),
                    "top" => Some(VerticalAlignment::Top),
                    _ => None,
                }),
        }
    }

    /// This look with what it leaves unset taken from `fallback`.
    #[must_use]
    pub(crate) fn or(self, fallback: Self) -> Self {
        Self {
            fill: self.fill.or(fallback.fill),
            borders: self.borders.or(fallback.borders),
            vertical: self.vertical.or(fallback.vertical),
        }
    }
}

/// The fill of a `w:shd`: its fill colour, or the pattern colour of a solid pattern.
pub(crate) fn shading_fill(shading: &Element) -> Option<String> {
    if shading.attribute(W, "val") == Some("solid") {
        return hex_colour(shading.attribute(W, "color"));
    }
    if shading.attribute(W, "val") == Some("nil") {
        return None;
    }
    hex_colour(shading.attribute(W, "fill")).filter(|fill| fill != "FFFFFF")
}

/// A colour attribute as `RRGGBB`; `auto` and anything else is unset.
pub(crate) fn hex_colour(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| value.len() == 6 && value.chars().all(|c| c.is_ascii_hexdigit()))
        .map(str::to_ascii_uppercase)
}

/// The colour of a `w:highlight` name, as Word draws it.
pub(crate) fn highlight_colour(name: &str) -> Option<&'static str> {
    Some(match name {
        "yellow" => "FFFF00",
        "green" => "00FF00",
        "cyan" => "00FFFF",
        "magenta" => "FF00FF",
        "blue" => "0000FF",
        "red" => "FF0000",
        "darkBlue" => "000080",
        "darkCyan" => "008080",
        "darkGreen" => "008000",
        "darkMagenta" => "800080",
        "darkRed" => "800000",
        "darkYellow" => "808000",
        "darkGray" => "808080",
        "lightGray" => "C0C0C0",
        "black" => "000000",
        "white" => "FFFFFF",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml_tree::parse_tree;

    fn element(xml: &str) -> Element {
        parse_tree(
            &format!(
                r#"<w:root xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">{xml}</w:root>"#
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
    fn reads_borders_with_style_width_and_colour() {
        let borders = Borders::parse(&element(concat!(
            r#"<w:pBdr><w:top w:val="single" w:sz="12" w:color="ff0000"/><w:left w:val="nil"/>"#,
            r#"<w:bottom w:val="double" w:sz="999"/><w:end w:val="dotted" w:sz="0" w:color="auto"/>"#,
            r#"<w:between w:val="dashed"/></w:pBdr>"#
        )));
        assert_eq!(
            borders.top,
            Some(Border {
                style: BorderStyle::Single,
                width_eighths: 12,
                colour: Some("FF0000".to_owned())
            })
        );
        assert_eq!(
            borders.left.as_ref().map(|b| b.style),
            Some(BorderStyle::None)
        );
        assert!(!borders.left.as_ref().unwrap().is_visible());
        assert_eq!(borders.bottom.as_ref().map(|b| b.width_eighths), Some(96));
        assert_eq!(
            borders.bottom.as_ref().map(|b| b.style),
            Some(BorderStyle::Double)
        );
        let right = borders.right.clone().unwrap();
        assert_eq!(
            (right.style, right.width_eighths, right.colour),
            (BorderStyle::Dotted, 2, None)
        );
        assert_eq!(
            borders.inside_horizontal.as_ref().map(|b| b.style),
            Some(BorderStyle::Dashed)
        );
        assert!(borders.any_visible());
        assert!(
            !Borders::parse(&element(r#"<w:pBdr><w:top w:val="none"/></w:pBdr>"#)).any_visible()
        );
        assert_eq!(Border::parse(&element("<w:top/>")), None);
    }

    #[test]
    fn fills_unset_edges_from_a_fallback() {
        let near = Borders {
            top: Border::parse(&element(r#"<w:top w:val="nil"/>"#)),
            ..Borders::default()
        };
        let far = Borders::parse(&element(
            r#"<w:tblBorders><w:top w:val="single"/><w:insideV w:val="single"/></w:tblBorders>"#,
        ));
        let merged = near.or(far);
        assert!(!merged.top.unwrap().is_visible());
        assert!(merged.inside_vertical.unwrap().is_visible());
    }

    #[test]
    fn reads_and_merges_tab_stops() {
        let own = parse_tabs(&element(concat!(
            r#"<w:tabs><w:tab w:val="right" w:leader="dot" w:pos="9000"/><w:tab w:val="clear" w:pos="4513"/>"#,
            r#"<w:tab w:val="center" w:pos="2000"/><w:tab w:val="bar" w:pos="100"/><w:tab w:pos="5"/>"#,
            r#"<w:tab w:val="decimal" w:leader="middleDot" w:pos="99999999"/></w:tabs>"#
        )));
        assert_eq!(
            own.iter()
                .map(|stop| stop.position_twips)
                .collect::<Vec<_>>(),
            vec![2_000, 4_513, 9_000, 31_680]
        );
        assert_eq!(own[2].leader, TabLeader::Dot);
        assert_eq!(own[3].kind, TabKind::Decimal);
        assert_eq!(own[3].leader, TabLeader::MiddleDot);
        let inherited = parse_tabs(&element(concat!(
            r#"<w:tabs><w:tab w:val="center" w:pos="4513"/><w:tab w:val="left" w:leader="hyphen" w:pos="1000"/>"#,
            r#"<w:tab w:val="end" w:leader="underscore" w:pos="7000"/></w:tabs>"#
        )));
        assert_eq!(inherited[0].leader, TabLeader::Hyphen);
        assert_eq!(inherited[2].kind, TabKind::Right);
        assert_eq!(inherited[2].leader, TabLeader::Underscore);
        let merged = merge_tabs(own.clone(), inherited.clone());
        assert_eq!(
            merged
                .iter()
                .map(|stop| (stop.position_twips, stop.kind))
                .collect::<Vec<_>>(),
            vec![
                (1_000, TabKind::Left),
                (2_000, TabKind::Center),
                (4_513, TabKind::Clear),
                (7_000, TabKind::Right),
                (9_000, TabKind::Right),
                (31_680, TabKind::Decimal)
            ]
        );
        assert_eq!(merge_tabs(Vec::new(), inherited.clone()), inherited);
    }

    #[test]
    fn reads_the_look_of_a_cell() {
        let look = CellLook::parse(&element(concat!(
            r#"<w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="d9e2f3"/>"#,
            r#"<w:tcBorders><w:bottom w:val="single" w:sz="8"/></w:tcBorders><w:vAlign w:val="center"/></w:tcPr>"#
        )));
        assert_eq!(look.fill.as_deref(), Some("D9E2F3"));
        assert_eq!(look.vertical, Some(VerticalAlignment::Center));
        assert!(look.borders.bottom.is_some());
        let bottom = CellLook::parse(&element(
            r#"<w:tcPr><w:shd w:val="solid" w:color="00ff00" w:fill="auto"/><w:vAlign w:val="bottom"/></w:tcPr>"#,
        ));
        assert_eq!(bottom.fill.as_deref(), Some("00FF00"));
        assert_eq!(bottom.vertical, Some(VerticalAlignment::Bottom));
        let plain = CellLook::parse(&element(
            r#"<w:tcPr><w:shd w:val="nil" w:fill="ff0000"/><w:vAlign w:val="top"/></w:tcPr>"#,
        ));
        assert_eq!(plain.fill, None);
        assert_eq!(plain.vertical, Some(VerticalAlignment::Top));
        let white = CellLook::parse(&element(
            r#"<w:tcPr><w:shd w:val="clear" w:fill="FFFFFF"/><w:vAlign w:val="both"/></w:tcPr>"#,
        ));
        assert_eq!((white.fill, white.vertical), (None, None));
        let merged = CellLook::default().or(look.clone());
        assert_eq!(merged, look);
    }

    #[test]
    fn maps_highlight_names_and_colours() {
        assert_eq!(highlight_colour("yellow"), Some("FFFF00"));
        assert_eq!(highlight_colour("darkGray"), Some("808080"));
        assert_eq!(highlight_colour("none"), None);
        assert_eq!(hex_colour(Some("abc")), None);
        assert_eq!(hex_colour(Some("auto")), None);
    }
}
