// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! The text properties of slides: a paragraph and run style that each level of a list style,
//! a placeholder or a master gives, merged nearest first.

use crate::document::Alignment;
use crate::theme::{Theme, colour_in};
use crate::xml_tree::{Element, Namespace};

const A: Namespace = Namespace::Drawing;
/// Nine outline levels, as `lvl1pPr` to `lvl9pPr` name them.
pub(super) const LEVELS: usize = 9;
/// The largest size a run may claim, 4,000 points, in hundredths.
const MAX_SIZE: u32 = 400_000;
const MAX_EMU: i64 = 51_206_400;

/// What a paragraph starts with, as a style sets it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum BulletStyle {
    None,
    Character(String),
    Number { scheme: String, start: u32 },
}

/// The paragraph and run properties one level of a style sets; unset ones come from further.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct LevelStyle {
    pub size: Option<u32>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    pub colour: Option<String>,
    pub family: Option<String>,
    pub align: Option<Alignment>,
    pub bullet: Option<BulletStyle>,
    pub margin_left: Option<i64>,
    pub indent: Option<i64>,
    pub line_spacing: Option<u32>,
    pub space_before: Option<u32>,
    pub space_after: Option<u32>,
}

impl LevelStyle {
    /// A paragraph's properties, `a:pPr` or `a:lvlNpPr`, with the run properties of its
    /// `a:defRPr`.
    pub(super) fn paragraph(properties: &Element, theme: &Theme) -> Self {
        let attribute = |name: &str| properties.attribute(Namespace::None, name);
        let emu = |name: &str| {
            attribute(name)
                .and_then(|value| value.parse::<i64>().ok())
                .map(|value| value.clamp(-MAX_EMU, MAX_EMU))
        };
        let points = |name: &str| {
            properties
                .child(A, name)
                .and_then(|spacing| spacing.child(A, "spcPts"))
                .and_then(|points| points.attribute(Namespace::None, "val"))
                .and_then(|value| value.parse::<u32>().ok())
                .map(|value| value.min(MAX_SIZE))
        };
        let mut style = properties
            .child(A, "defRPr")
            .map(|run| Self::run(run, theme))
            .unwrap_or_default();
        style.align = attribute("algn").and_then(|value| match value {
            "l" => Some(Alignment::Left),
            "ctr" => Some(Alignment::Center),
            "r" => Some(Alignment::Right),
            "just" | "dist" | "justLow" | "thaiDist" => Some(Alignment::Justify),
            _ => None,
        });
        style.margin_left = emu("marL");
        style.indent = emu("indent");
        style.line_spacing = properties
            .child(A, "lnSpc")
            .and_then(|spacing| spacing.child(A, "spcPct"))
            .and_then(|percent| percent.attribute(Namespace::None, "val"))
            .and_then(|value| value.parse::<u32>().ok())
            .map(|value| (value / 1_000).clamp(10, 1_000));
        style.space_before = points("spcBef");
        style.space_after = points("spcAft");
        style.bullet = if properties.child(A, "buNone").is_some() {
            Some(BulletStyle::None)
        } else if let Some(character) = properties.child(A, "buChar") {
            character
                .attribute(Namespace::None, "char")
                .map(|bullet| BulletStyle::Character(bullet_character(bullet)))
        } else {
            properties
                .child(A, "buAutoNum")
                .map(|number| BulletStyle::Number {
                    scheme: number
                        .attribute(Namespace::None, "type")
                        .unwrap_or("arabicPeriod")
                        .to_owned(),
                    start: number
                        .attribute(Namespace::None, "startAt")
                        .and_then(|start| start.parse().ok())
                        .unwrap_or(1),
                })
        };
        style
    }

    /// A run's properties, `a:rPr`, `a:defRPr` or `a:endParaRPr`.
    pub(super) fn run(properties: &Element, theme: &Theme) -> Self {
        let attribute = |name: &str| properties.attribute(Namespace::None, name);
        let flag = |name: &str| attribute(name).map(|value| matches!(value, "1" | "true"));
        Self {
            size: attribute("sz")
                .and_then(|value| value.parse::<u32>().ok())
                .filter(|size| *size >= 100)
                .map(|size| size.min(MAX_SIZE)),
            bold: flag("b"),
            italic: flag("i"),
            underline: attribute("u").map(|value| value != "none"),
            strike: attribute("strike").map(|value| value != "noStrike"),
            colour: properties
                .child(A, "solidFill")
                .and_then(|fill| colour_in(fill, theme)),
            family: properties
                .child(A, "latin")
                .and_then(|latin| latin.attribute(Namespace::None, "typeface"))
                .filter(|typeface| !typeface.is_empty())
                .map(|typeface| theme.typeface(typeface)),
            ..Self::default()
        }
    }

    /// This style with what it leaves unset taken from `fallback`.
    #[must_use]
    pub(super) fn or(self, fallback: &Self) -> Self {
        Self {
            size: self.size.or(fallback.size),
            bold: self.bold.or(fallback.bold),
            italic: self.italic.or(fallback.italic),
            underline: self.underline.or(fallback.underline),
            strike: self.strike.or(fallback.strike),
            colour: self.colour.or_else(|| fallback.colour.clone()),
            family: self.family.or_else(|| fallback.family.clone()),
            align: self.align.or(fallback.align),
            bullet: self.bullet.or_else(|| fallback.bullet.clone()),
            margin_left: self.margin_left.or(fallback.margin_left),
            indent: self.indent.or(fallback.indent),
            line_spacing: self.line_spacing.or(fallback.line_spacing),
            space_before: self.space_before.or(fallback.space_before),
            space_after: self.space_after.or(fallback.space_after),
        }
    }
}

/// The styles of each level of a list style (`a:lstStyle`, `p:titleStyle`, `p:bodyStyle`),
/// each over the list's `a:defPPr`.
pub(super) fn list_styles(list: Option<&Element>, theme: &Theme) -> Vec<LevelStyle> {
    let Some(list) = list else {
        return vec![LevelStyle::default(); LEVELS];
    };
    let base = list
        .child(A, "defPPr")
        .map(|properties| LevelStyle::paragraph(properties, theme))
        .unwrap_or_default();
    (1..=LEVELS)
        .map(|level| {
            list.child(A, &format!("lvl{level}pPr"))
                .map(|properties| LevelStyle::paragraph(properties, theme))
                .unwrap_or_default()
                .or(&base)
        })
        .collect()
}

/// A bullet character as it shows: the private-use glyphs of the symbol fonts become the
/// bullets they draw there.
fn bullet_character(bullet: &str) -> String {
    bullet
        .chars()
        .map(|character| match character {
            '\u{f0a7}' | '\u{f06e}' | '\u{a7}' => '\u{25aa}',
            '\u{f0d8}' | '\u{d8}' => '\u{27a2}',
            '\u{f0fc}' | '\u{fc}' => '\u{2713}',
            '\u{f000}'..='\u{f0ff}' => '\u{2022}',
            other => other,
        })
        .collect()
}

/// The label of the `number`th paragraph of an automatically numbered list, as its scheme
/// writes it: `arabicPeriod` is `1.`, `alphaLcParenR` is `a)`, `romanUcPeriod` is `I.`.
pub(super) fn number_label(scheme: &str, number: u32) -> String {
    let lower = scheme.to_ascii_lowercase();
    let digits = if lower.starts_with("alpha") {
        let letters = letters(number);
        if lower.starts_with("alphauc") {
            letters.to_ascii_uppercase()
        } else {
            letters
        }
    } else if lower.starts_with("roman") {
        let numeral = roman(number);
        if lower.starts_with("romanuc") {
            numeral.to_ascii_uppercase()
        } else {
            numeral
        }
    } else {
        number.to_string()
    };
    if lower.ends_with("parenboth") {
        format!("({digits})")
    } else if lower.ends_with("parenr") {
        format!("{digits})")
    } else if lower.ends_with("plain") {
        digits
    } else {
        format!("{digits}.")
    }
}

fn letters(number: u32) -> String {
    let mut number = number.max(1);
    let mut label = Vec::new();
    while number > 0 {
        number -= 1;
        label.push(char::from(b'a' + u8::try_from(number % 26).unwrap_or(0)));
        number /= 26;
    }
    label.iter().rev().collect()
}

fn roman(number: u32) -> String {
    const NUMERALS: [(u32, &str); 13] = [
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let mut number = number.clamp(1, 3_999);
    let mut label = String::new();
    for (value, numeral) in NUMERALS {
        while number >= value {
            label.push_str(numeral);
            number -= value;
        }
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml_tree::parse_tree;

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
    fn reads_the_paragraph_and_run_properties_of_a_level() {
        let theme = Theme::default();
        let style = LevelStyle::paragraph(
            &element(concat!(
                r#"<a:lvl2pPr marL="742950" indent="-285750" algn="ctr"><a:lnSpc><a:spcPct val="90000"/></a:lnSpc>"#,
                r#"<a:spcBef><a:spcPts val="1000"/></a:spcBef><a:spcAft><a:spcPts val="600"/></a:spcAft>"#,
                r#"<a:buFont typeface="Arial"/><a:buChar char="&#xF0A7;"/>"#,
                r#"<a:defRPr sz="2400" b="1" i="0" u="sng" strike="sngStrike"><a:solidFill><a:schemeClr val="accent1"/></a:solidFill>"#,
                r#"<a:latin typeface="+mj-lt"/></a:defRPr></a:lvl2pPr>"#
            )),
            &theme,
        );
        assert_eq!(style.margin_left, Some(742_950));
        assert_eq!(style.indent, Some(-285_750));
        assert_eq!(style.align, Some(Alignment::Center));
        assert_eq!(style.line_spacing, Some(90));
        assert_eq!(
            (style.space_before, style.space_after),
            (Some(1_000), Some(600))
        );
        assert_eq!(
            style.bullet,
            Some(BulletStyle::Character("\u{25aa}".to_owned()))
        );
        assert_eq!(style.size, Some(2_400));
        assert_eq!((style.bold, style.italic), (Some(true), Some(false)));
        assert_eq!((style.underline, style.strike), (Some(true), Some(true)));
        assert_eq!(style.colour.as_deref(), Some("4472C4"));
        assert_eq!(style.family.as_deref(), Some("Calibri Light"));

        let numbered = LevelStyle::paragraph(
            &element(
                r#"<a:pPr algn="just"><a:buAutoNum type="alphaLcParenR" startAt="3"/></a:pPr>"#,
            ),
            &theme,
        );
        assert_eq!(
            numbered.bullet,
            Some(BulletStyle::Number {
                scheme: "alphaLcParenR".to_owned(),
                start: 3
            })
        );
        assert_eq!(numbered.align, Some(Alignment::Justify));
        let none = LevelStyle::paragraph(&element("<a:pPr><a:buNone/></a:pPr>"), &theme);
        assert_eq!(none.bullet, Some(BulletStyle::None));
        let tiny = LevelStyle::run(&element(r#"<a:rPr sz="50" u="none"/>"#), &theme);
        assert_eq!((tiny.size, tiny.underline), (None, Some(false)));
    }

    #[test]
    fn merges_levels_nearest_first() {
        let theme = Theme::default();
        let list = element(concat!(
            r#"<a:lstStyle><a:defPPr algn="r"><a:defRPr sz="1800"/></a:defPPr>"#,
            r#"<a:lvl1pPr><a:defRPr b="1"/></a:lvl1pPr></a:lstStyle>"#
        ));
        let levels = list_styles(Some(&list), &theme);
        assert_eq!(levels.len(), LEVELS);
        assert_eq!(levels[0].bold, Some(true));
        assert_eq!(levels[0].size, Some(1_800), "from the list's default");
        assert_eq!(levels[3].align, Some(Alignment::Right));
        assert_eq!(
            list_styles(None, &theme),
            vec![LevelStyle::default(); LEVELS]
        );
        let near = LevelStyle {
            size: Some(1_000),
            ..LevelStyle::default()
        };
        let merged = near.or(&levels[0]);
        assert_eq!((merged.size, merged.bold), (Some(1_000), Some(true)));
    }

    #[test]
    fn writes_the_labels_of_numbered_lists() {
        assert_eq!(number_label("arabicPeriod", 3), "3.");
        assert_eq!(number_label("arabicParenR", 3), "3)");
        assert_eq!(number_label("arabicParenBoth", 3), "(3)");
        assert_eq!(number_label("arabicPlain", 3), "3");
        assert_eq!(number_label("alphaLcPeriod", 28), "ab.");
        assert_eq!(number_label("alphaUcParenR", 2), "B)");
        assert_eq!(number_label("romanUcPeriod", 4), "IV.");
        assert_eq!(number_label("romanLcPeriod", 9), "ix.");
        assert_eq!(bullet_character("\u{f0b7}"), "\u{2022}");
        assert_eq!(bullet_character("–"), "–");
    }
}
