// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! How paragraphs, runs and tables look on the page: spacing, indents, fonts, column widths and
//! borders. A renderer needs them and an editor does not, so they never reach the JSON of the tree.

use super::anchor::TextFrame;
use super::edges::{
    Borders, TabStop, hex_colour, highlight_colour, merge_tabs, parse_tabs, shading_fill,
};
use super::format::ParagraphFormat;
use super::styles::on_off;
use crate::xml_tree::{Element, Namespace};

const W: Namespace = Namespace::Word;
/// Word's own page limit, 22 inches: a longer length is a crafted file.
const MAX_TWIPS: u32 = 31_680;
/// The space Word gives a paragraph whose spacing is automatic, as HTML paragraphs are.
const AUTO_SPACING_TWIPS: u32 = 280;
/// Ten times single spacing; more is a crafted file.
const MAX_LINE_MULTIPLE: u32 = 2_400;

/// The height of the lines of a paragraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineSpacing {
    /// A multiple of single spacing, in 240ths: 240 is single, 360 one and a half.
    Auto(u32),
    /// Exactly this height, in twentieths of a point.
    Exact(u32),
    /// At least this height, in twentieths of a point.
    AtLeast(u32),
}

/// The font of a run, where every property may be unset so a style can supply it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunLook {
    pub family: Option<String>,
    pub size_half_points: Option<u32>,
    /// The colour as `RRGGBB`; unset means automatic.
    pub colour: Option<String>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    /// The highlight or shading behind the text, as `RRGGBB`.
    pub highlight: Option<String>,
    pub caps: Option<bool>,
    pub small_caps: Option<bool>,
    /// The page number field this run shows the last result of.
    pub field: Option<PageField>,
}

/// A field whose result depends on the page it is drawn on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub enum PageField {
    /// `PAGE`: the number of the page.
    Page,
    /// `NUMPAGES` and `SECTIONPAGES`: how many pages there are.
    Pages,
}

impl RunLook {
    /// Read `w:rPr`.
    pub(crate) fn parse(properties: &Element) -> Self {
        let family = properties.child(W, "rFonts").and_then(|fonts| {
            ["ascii", "hAnsi"]
                .iter()
                .find_map(|name| fonts.attribute(W, name))
                .map(str::to_owned)
                .or_else(|| {
                    ["asciiTheme", "hAnsiTheme"]
                        .iter()
                        .find_map(|name| fonts.attribute(W, name))
                        .map(theme_font)
                })
        });
        let size_half_points = properties
            .child(W, "sz")
            .and_then(|size| size.attribute(W, "val"))
            .and_then(|size| size.parse::<u32>().ok())
            .filter(|size| (2..=3_276).contains(size));
        let colour = hex_colour(
            properties
                .child(W, "color")
                .and_then(|colour| colour.attribute(W, "val")),
        );
        let highlight = properties
            .child(W, "highlight")
            .and_then(|highlight| highlight.attribute(W, "val"))
            .and_then(highlight_colour)
            .map(str::to_owned)
            .or_else(|| properties.child(W, "shd").and_then(shading_fill));
        Self {
            family,
            size_half_points,
            colour,
            bold: properties.child(W, "b").map(on_off),
            italic: properties.child(W, "i").map(on_off),
            highlight,
            caps: properties.child(W, "caps").map(on_off),
            small_caps: properties.child(W, "smallCaps").map(on_off),
            field: None,
        }
    }

    /// This look with what it leaves unset taken from `fallback`.
    #[must_use]
    pub(crate) fn or(self, fallback: Self) -> Self {
        Self {
            family: self.family.or(fallback.family),
            size_half_points: self.size_half_points.or(fallback.size_half_points),
            colour: self.colour.or(fallback.colour),
            bold: self.bold.or(fallback.bold),
            italic: self.italic.or(fallback.italic),
            highlight: self.highlight.or(fallback.highlight),
            caps: self.caps.or(fallback.caps),
            small_caps: self.small_caps.or(fallback.small_caps),
            field: self.field.or(fallback.field),
        }
    }
}

thread_local! {
    /// The major and minor fonts of the theme of the document being read.
    static THEME_FONTS: std::cell::RefCell<Option<(String, String)>> =
        const { std::cell::RefCell::new(None) };
}

/// The theme fonts of the document being read, for the theme references of its runs; they
/// apply until the scope is dropped.
pub(crate) struct ThemeFontsScope;

impl ThemeFontsScope {
    pub(crate) fn enter(major: String, minor: String) -> Self {
        THEME_FONTS.with(|fonts| *fonts.borrow_mut() = Some((major, minor)));
        Self
    }
}

impl Drop for ThemeFontsScope {
    fn drop(&mut self) {
        THEME_FONTS.with(|fonts| *fonts.borrow_mut() = None);
    }
}

/// The font a theme reference names: the document's theme while it is read, else Word 2007's
/// default theme, which a document without a theme part keeps.
fn theme_font(reference: &str) -> String {
    let major = reference.starts_with("major");
    THEME_FONTS.with(|fonts| match &*fonts.borrow() {
        Some((major_font, minor_font)) => {
            if major {
                major_font.clone()
            } else {
                minor_font.clone()
            }
        }
        None if major => "Cambria".to_owned(),
        None => "Calibri".to_owned(),
    })
}

/// The spacing and indents of a paragraph, and the font its runs inherit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParagraphLook {
    pub space_before_twips: Option<u32>,
    pub space_after_twips: Option<u32>,
    pub line_spacing: Option<LineSpacing>,
    pub indent_left_twips: Option<i32>,
    pub indent_right_twips: Option<i32>,
    /// Positive indents the first line, negative hangs it.
    pub indent_first_line_twips: Option<i32>,
    /// The paragraph starts a new page.
    pub page_break_before: bool,
    /// The font of the paragraph's style chain over the document defaults.
    pub font: RunLook,
    /// The tab stops of the paragraph and its style; cleared ones stay as [`TabKind::Clear`].
    ///
    /// [`TabKind::Clear`]: super::TabKind::Clear
    pub tabs: Vec<TabStop>,
    /// The background as `RRGGBB`.
    pub shading: Option<String>,
    pub borders: Borders,
    /// The label a numbered or bulleted paragraph shows, as the numbering part writes it.
    pub list_label: Option<ListLabel>,
    /// The text box the paragraph is in.
    pub frame: Option<TextFrame>,
    /// What the paragraph sets itself, the part an editor changes and the JSON tree carries.
    pub format: ParagraphFormat,
}

/// What separates a list label from the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelSuffix {
    Tab,
    Space,
    Nothing,
}

/// The label of a list paragraph: `1.`, `a)`, `2.3`, a bullet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListLabel {
    pub text: String,
    pub suffix: LabelSuffix,
    /// The font the level gives its label.
    pub font: RunLook,
}

impl ParagraphLook {
    /// Read the spacing, indents and page break of a `w:pPr`; the font comes from the styles.
    pub(crate) fn parse(properties: &Element) -> Self {
        let mut look = Self::default();
        if let Some(spacing) = properties.child(W, "spacing") {
            look.space_before_twips = if is_on(spacing, "beforeAutospacing") {
                Some(AUTO_SPACING_TWIPS)
            } else {
                twips(spacing, "before")
            };
            look.space_after_twips = if is_on(spacing, "afterAutospacing") {
                Some(AUTO_SPACING_TWIPS)
            } else {
                twips(spacing, "after")
            };
            look.line_spacing =
                twips(spacing, "line").map(|line| match spacing.attribute(W, "lineRule") {
                    Some("exact") => LineSpacing::Exact(line),
                    Some("atLeast") => LineSpacing::AtLeast(line),
                    _ => LineSpacing::Auto(line.clamp(1, MAX_LINE_MULTIPLE)),
                });
        }
        if let Some(indent) = properties.child(W, "ind") {
            look.indent_left_twips =
                signed_twips(indent, "left").or_else(|| signed_twips(indent, "start"));
            look.indent_right_twips =
                signed_twips(indent, "right").or_else(|| signed_twips(indent, "end"));
            look.indent_first_line_twips = signed_twips(indent, "hanging")
                .map(|hanging| -hanging)
                .or_else(|| signed_twips(indent, "firstLine"));
        }
        look.page_break_before = properties.child(W, "pageBreakBefore").is_some_and(on_off);
        look.tabs = properties
            .child(W, "tabs")
            .map(parse_tabs)
            .unwrap_or_default();
        look.shading = properties.child(W, "shd").and_then(shading_fill);
        look.borders = properties
            .child(W, "pBdr")
            .map(Borders::parse)
            .unwrap_or_default();
        look
    }

    /// This look with what it leaves unset taken from `fallback`.
    #[must_use]
    pub(crate) fn or(self, fallback: Self) -> Self {
        Self {
            space_before_twips: self.space_before_twips.or(fallback.space_before_twips),
            space_after_twips: self.space_after_twips.or(fallback.space_after_twips),
            line_spacing: self.line_spacing.or(fallback.line_spacing),
            indent_left_twips: self.indent_left_twips.or(fallback.indent_left_twips),
            indent_right_twips: self.indent_right_twips.or(fallback.indent_right_twips),
            indent_first_line_twips: self
                .indent_first_line_twips
                .or(fallback.indent_first_line_twips),
            page_break_before: self.page_break_before || fallback.page_break_before,
            font: self.font.or(fallback.font),
            tabs: merge_tabs(self.tabs, fallback.tabs),
            shading: self.shading.or(fallback.shading),
            borders: self.borders.or(fallback.borders),
            list_label: self.list_label.or(fallback.list_label),
            frame: self.frame.or(fallback.frame),
            // What a paragraph sets itself is its own: a style never sets it for it.
            format: self.format,
        }
    }
}

/// The column widths and borders of a table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TableLook {
    /// The widths of the grid columns, `w:tblGrid`, in twentieths of a point.
    pub column_widths_twips: Vec<u32>,
    /// Whether the table draws its cell borders.
    pub borders: bool,
    /// The default cell margins, `w:tblCellMar`: top, left, bottom, right, in twentieths of a
    /// point; unset means Word's defaults.
    pub cell_margins_twips: [Option<u32>; 4],
    /// The table's borders, its own over its style's.
    pub edges: Borders,
    /// The text box the table is in.
    pub frame: Option<TextFrame>,
}

impl TableLook {
    /// Read the grid and the borders of a `w:tbl`: the table's own borders, those of its style
    /// by name, or those of its first cell.
    pub(crate) fn parse(table: &Element) -> Self {
        let column_widths_twips = table
            .child(W, "tblGrid")
            .map(|grid| {
                grid.elements()
                    .filter(|column| column.is(W, "gridCol"))
                    .map(|column| twips(column, "w").unwrap_or(0))
                    .collect()
            })
            .unwrap_or_default();
        let properties = table.child(W, "tblPr");
        let table_borders = properties
            .and_then(|properties| properties.child(W, "tblBorders"))
            .is_some_and(has_visible_border);
        let grid_style = properties
            .and_then(|properties| properties.child(W, "tblStyle"))
            .and_then(|style| style.attribute(W, "val"))
            .is_some_and(|style| {
                let style = style.to_ascii_lowercase();
                style.contains("grid") || style.contains("griglia")
            });
        let cell_borders = table
            .descendant(W, "tcBorders")
            .is_some_and(has_visible_border);
        let margins = properties.and_then(|properties| properties.child(W, "tblCellMar"));
        let margin = |names: &[&str]| {
            margins.and_then(|margins| {
                names
                    .iter()
                    .find_map(|name| margins.child(W, name))
                    .and_then(|margin| twips(margin, "w"))
            })
        };
        Self {
            column_widths_twips,
            borders: table_borders || grid_style || cell_borders,
            edges: properties
                .and_then(|properties| properties.child(W, "tblBorders"))
                .map(Borders::parse)
                .unwrap_or_default(),
            cell_margins_twips: [
                margin(&["top"]),
                margin(&["left", "start"]),
                margin(&["bottom"]),
                margin(&["right", "end"]),
            ],
            frame: None,
        }
    }
}

/// Whether a borders element (`w:tblBorders`, `w:tcBorders`) draws any edge.
fn has_visible_border(borders: &Element) -> bool {
    borders.elements().any(|edge| {
        edge.namespace == W
            && edge
                .attribute(W, "val")
                .is_some_and(|value| !matches!(value, "nil" | "none"))
    })
}

fn is_on(element: &Element, name: &str) -> bool {
    element
        .attribute(W, name)
        .is_some_and(|value| matches!(value, "1" | "true" | "on"))
}

fn twips(element: &Element, name: &str) -> Option<u32> {
    let value = element.attribute(W, name)?.parse::<i64>().ok()?;
    u32::try_from(value.clamp(0, i64::from(MAX_TWIPS))).ok()
}

fn signed_twips(element: &Element, name: &str) -> Option<i32> {
    let value = element.attribute(W, name)?.parse::<i64>().ok()?;
    i32::try_from(value.clamp(-i64::from(MAX_TWIPS), i64::from(MAX_TWIPS))).ok()
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
    fn reads_the_spacing_and_indents_of_a_paragraph() {
        let look = ParagraphLook::parse(&element(concat!(
            r#"<w:pPr><w:spacing w:before="120" w:after="-5" w:line="360" w:lineRule="auto"/>"#,
            r#"<w:ind w:start="720" w:end="99999999" w:hanging="360"/><w:pageBreakBefore/></w:pPr>"#
        )));
        assert_eq!(look.space_before_twips, Some(120));
        assert_eq!(look.space_after_twips, Some(0));
        assert_eq!(look.line_spacing, Some(LineSpacing::Auto(360)));
        assert_eq!(look.indent_left_twips, Some(720));
        assert_eq!(look.indent_right_twips, Some(31_680));
        assert_eq!(look.indent_first_line_twips, Some(-360));
        assert!(look.page_break_before);

        let exact = ParagraphLook::parse(&element(
            r#"<w:pPr><w:spacing w:beforeAutospacing="1" w:afterAutospacing="true" w:line="300" w:lineRule="exact"/><w:ind w:left="-100" w:firstLine="200"/></w:pPr>"#,
        ));
        assert_eq!(exact.space_before_twips, Some(AUTO_SPACING_TWIPS));
        assert_eq!(exact.space_after_twips, Some(AUTO_SPACING_TWIPS));
        assert_eq!(exact.line_spacing, Some(LineSpacing::Exact(300)));
        assert_eq!(exact.indent_left_twips, Some(-100));
        assert_eq!(exact.indent_first_line_twips, Some(200));
        assert!(!exact.page_break_before);

        let least = ParagraphLook::parse(&element(
            r#"<w:pPr><w:spacing w:line="0" w:lineRule="atLeast"/></w:pPr>"#,
        ));
        assert_eq!(least.line_spacing, Some(LineSpacing::AtLeast(0)));
        let auto = ParagraphLook::parse(&element(r#"<w:pPr><w:spacing w:line="0"/></w:pPr>"#));
        assert_eq!(auto.line_spacing, Some(LineSpacing::Auto(1)));
        assert_eq!(
            ParagraphLook::parse(&element("<w:pPr/>")),
            ParagraphLook::default()
        );
    }

    #[test]
    fn reads_the_font_of_a_run() {
        let look = RunLook::parse(&element(
            r#"<w:rPr><w:rFonts w:hAnsi="Arial"/><w:sz w:val="28"/><w:color w:val="1f3864"/><w:b/><w:i w:val="0"/></w:rPr>"#,
        ));
        assert_eq!(
            look,
            RunLook {
                family: Some("Arial".to_owned()),
                size_half_points: Some(28),
                colour: Some("1F3864".to_owned()),
                bold: Some(true),
                italic: Some(false),
                ..RunLook::default()
            }
        );
        let theme = RunLook::parse(&element(
            r#"<w:rPr><w:rFonts w:asciiTheme="majorHAnsi"/><w:sz w:val="1"/><w:color w:val="auto"/></w:rPr>"#,
        ));
        assert_eq!(theme.family.as_deref(), Some("Cambria"));
        assert_eq!(theme.size_half_points, None);
        assert_eq!(theme.colour, None);
    }

    #[test]
    fn fills_what_a_look_leaves_unset_from_its_fallback() {
        let near = ParagraphLook {
            space_after_twips: Some(0),
            font: RunLook {
                bold: Some(true),
                ..RunLook::default()
            },
            ..ParagraphLook::default()
        };
        let far = ParagraphLook {
            space_after_twips: Some(160),
            space_before_twips: Some(240),
            page_break_before: true,
            font: RunLook {
                size_half_points: Some(22),
                bold: Some(false),
                ..RunLook::default()
            },
            ..ParagraphLook::default()
        };
        let merged = near.or(far);
        assert_eq!(merged.space_after_twips, Some(0));
        assert_eq!(merged.space_before_twips, Some(240));
        assert!(merged.page_break_before);
        assert_eq!(merged.font.bold, Some(true));
        assert_eq!(merged.font.size_half_points, Some(22));
    }

    #[test]
    fn reads_the_grid_and_the_borders_of_a_table() {
        let bordered = TableLook::parse(&element(concat!(
            r#"<w:tbl><w:tblPr><w:tblBorders><w:top w:val="single"/></w:tblBorders>"#,
            r#"<w:tblCellMar><w:top w:w="28"/><w:start w:w="55"/></w:tblCellMar></w:tblPr>"#,
            r#"<w:tblGrid><w:gridCol w:w="2000"/><w:gridCol/></w:tblGrid></w:tbl>"#
        )));
        assert_eq!(bordered.column_widths_twips, vec![2_000, 0]);
        assert!(bordered.borders);
        assert_eq!(
            bordered.cell_margins_twips,
            [Some(28), Some(55), None, None]
        );

        let none = TableLook::parse(&element(
            r#"<w:tbl><w:tblPr><w:tblBorders><w:top w:val="nil"/><w:left w:val="none"/></w:tblBorders></w:tblPr></w:tbl>"#,
        ));
        assert!(!none.borders);
        assert_eq!(none.column_widths_twips, [] as [u32; 0]);

        let styled = TableLook::parse(&element(
            r#"<w:tbl><w:tblPr><w:tblStyle w:val="TableGrid"/></w:tblPr></w:tbl>"#,
        ));
        assert!(styled.borders);
        let cells = TableLook::parse(&element(
            r#"<w:tbl><w:tr><w:tc><w:tcPr><w:tcBorders><w:bottom w:val="single"/></w:tcBorders></w:tcPr></w:tc></w:tr></w:tbl>"#,
        ));
        assert!(cells.borders);
    }
}
