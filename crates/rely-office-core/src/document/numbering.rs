// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The numbering part (`word/numbering.xml`): whether a numbered paragraph shows a bullet or a
//! number, and how far each level is indented.

use std::collections::HashMap;

use super::look::ParagraphLook;
use crate::OfficeError;
use crate::xml_tree::{Element, Namespace, parse_tree};

const W: Namespace = Namespace::Word;

/// One level of a list definition.
#[derive(Debug, Clone, Default)]
struct Level {
    format: Option<String>,
    look: ParagraphLook,
}

/// The number format and indents of each level of each list definition.
#[derive(Debug, Default)]
pub(crate) struct Numbering {
    /// `w:abstractNum` id to its levels.
    abstract_levels: HashMap<u32, HashMap<u8, Level>>,
    /// `w:num` id to its abstract definition and the levels it overrides.
    instances: HashMap<u32, (u32, HashMap<u8, Level>)>,
}

impl Numbering {
    /// Parse the numbering part; a document without one has no lists.
    pub(crate) fn parse(xml: Option<&str>) -> Result<Self, OfficeError> {
        let Some(xml) = xml else {
            return Ok(Self::default());
        };
        let root = parse_tree(xml, |_, _| false)?;
        let mut numbering = Self::default();
        for element in root.elements() {
            if element.is(W, "abstractNum") {
                if let Some(id) = number_attribute(element, "abstractNumId") {
                    numbering.abstract_levels.insert(id, levels(element));
                }
            } else if element.is(W, "num")
                && let Some(id) = number_attribute(element, "numId")
                && let Some(abstract_id) = element
                    .child(W, "abstractNumId")
                    .and_then(|child| number_attribute(child, "val"))
            {
                let overrides = element
                    .elements()
                    .filter(|child| child.is(W, "lvlOverride"))
                    .flat_map(levels)
                    .collect();
                numbering.instances.insert(id, (abstract_id, overrides));
            }
        }
        Ok(numbering)
    }

    /// Whether level `level` of list `num_id` is numbered (`Some(true)`), bulleted
    /// (`Some(false)`), or not a list at all (`None`: id 0, an unknown list, or format `none`).
    pub(crate) fn is_ordered(&self, num_id: u32, level: u8) -> Option<bool> {
        let (abstract_id, overrides) = self.instances.get(&num_id)?;
        let format = overrides
            .get(&level)
            .and_then(|level| level.format.as_deref())
            .or_else(|| {
                self.abstract_levels
                    .get(abstract_id)?
                    .get(&level)?
                    .format
                    .as_deref()
            })
            .unwrap_or("decimal");
        match format {
            "none" => None,
            "bullet" => Some(false),
            _ => Some(true),
        }
    }
}

impl Numbering {
    /// The indents level `level` of list `num_id` gives its paragraphs, an override first.
    pub(crate) fn level_look(&self, num_id: u32, level: u8) -> ParagraphLook {
        let Some((abstract_id, overrides)) = self.instances.get(&num_id) else {
            return ParagraphLook::default();
        };
        let overridden = overrides
            .get(&level)
            .map(|level| level.look.clone())
            .unwrap_or_default();
        let defined = self
            .abstract_levels
            .get(abstract_id)
            .and_then(|levels| levels.get(&level))
            .map(|level| level.look.clone())
            .unwrap_or_default();
        overridden.or(defined)
    }
}

fn number_attribute(element: &Element, name: &str) -> Option<u32> {
    element.attribute(W, name)?.parse().ok()
}

/// The `w:numFmt` and the `w:pPr` indents of every `w:lvl` under the element.
fn levels(element: &Element) -> HashMap<u8, Level> {
    element
        .elements()
        .filter(|child| child.is(W, "lvl"))
        .filter_map(|level| {
            let index = level.attribute(W, "ilvl")?.parse::<u8>().ok()?;
            let format = level
                .child(W, "numFmt")
                .and_then(|format| format.attribute(W, "val"))
                .map(str::to_owned);
            let look = level
                .child(W, "pPr")
                .map(ParagraphLook::parse)
                .unwrap_or_default();
            Some((index, Level { format, look }))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NUMBERING: &str = concat!(
        r#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
        r#"<w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/></w:lvl>"#,
        r#"<w:lvl w:ilvl="1"><w:numFmt w:val="lowerRoman"/></w:lvl></w:abstractNum>"#,
        r#"<w:abstractNum w:abstractNumId="1"><w:lvl w:ilvl="0"><w:numFmt w:val="none"/></w:lvl></w:abstractNum>"#,
        r#"<w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>"#,
        r#"<w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num>"#,
        r#"<w:num w:numId="3"><w:abstractNumId w:val="0"/><w:lvlOverride w:ilvl="0">"#,
        r#"<w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl></w:lvlOverride></w:num>"#,
        "</w:numbering>",
    );

    #[test]
    fn tells_bullets_from_numbers_per_level() {
        let numbering = Numbering::parse(Some(NUMBERING)).unwrap();
        assert_eq!(numbering.is_ordered(1, 0), Some(false));
        assert_eq!(numbering.is_ordered(1, 1), Some(true));
        assert_eq!(numbering.is_ordered(1, 5), Some(true));
    }

    #[test]
    fn applies_level_overrides() {
        let numbering = Numbering::parse(Some(NUMBERING)).unwrap();
        assert_eq!(numbering.is_ordered(3, 0), Some(true));
        assert_eq!(numbering.is_ordered(3, 1), Some(true));
    }

    #[test]
    fn treats_format_none_unknown_lists_and_id_zero_as_no_list() {
        let numbering = Numbering::parse(Some(NUMBERING)).unwrap();
        assert_eq!(numbering.is_ordered(2, 0), None);
        assert_eq!(numbering.is_ordered(0, 0), None);
        assert_eq!(numbering.is_ordered(99, 0), None);
        assert_eq!(Numbering::parse(None).unwrap().is_ordered(1, 0), None);
    }

    #[test]
    fn reads_the_indents_of_each_level_an_override_first() {
        let numbering = Numbering::parse(Some(concat!(
            r#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
            r#"<w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/>"#,
            r#"<w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr></w:lvl></w:abstractNum>"#,
            r#"<w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>"#,
            r#"<w:num w:numId="2"><w:abstractNumId w:val="0"/><w:lvlOverride w:ilvl="0">"#,
            r#"<w:lvl w:ilvl="0"><w:pPr><w:ind w:left="1440"/></w:pPr></w:lvl></w:lvlOverride></w:num>"#,
            "</w:numbering>",
        )))
        .unwrap();
        let look = numbering.level_look(1, 0);
        assert_eq!(
            (look.indent_left_twips, look.indent_first_line_twips),
            (Some(720), Some(-360))
        );
        let overridden = numbering.level_look(2, 0);
        assert_eq!(
            (
                overridden.indent_left_twips,
                overridden.indent_first_line_twips
            ),
            (Some(1_440), Some(-360))
        );
        assert_eq!(numbering.is_ordered(2, 0), Some(true));
        assert_eq!(numbering.level_look(9, 0), ParagraphLook::default());
        assert_eq!(numbering.level_look(1, 4), ParagraphLook::default());
    }

    #[test]
    fn refuses_a_malformed_numbering_part() {
        assert!(Numbering::parse(Some("<w:numbering><w:num>")).is_err());
    }
}
