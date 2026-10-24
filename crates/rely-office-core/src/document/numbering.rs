// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The numbering part (`word/numbering.xml`): whether a numbered paragraph shows a bullet or a
//! number.

use std::collections::HashMap;

use crate::OfficeError;
use crate::xml_tree::{Element, Namespace, parse_tree};

const W: Namespace = Namespace::Word;

/// The number format of each level of each list definition.
#[derive(Debug, Default)]
pub(crate) struct Numbering {
    /// `w:abstractNum` id to its level formats.
    abstract_formats: HashMap<u32, HashMap<u8, String>>,
    /// `w:num` id to its abstract definition and the levels it overrides.
    instances: HashMap<u32, (u32, HashMap<u8, String>)>,
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
                    numbering
                        .abstract_formats
                        .insert(id, level_formats(element));
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
                    .flat_map(level_formats)
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
            .or_else(|| self.abstract_formats.get(abstract_id)?.get(&level))
            .map_or("decimal", String::as_str);
        match format {
            "none" => None,
            "bullet" => Some(false),
            _ => Some(true),
        }
    }
}

fn number_attribute(element: &Element, name: &str) -> Option<u32> {
    element.attribute(W, name)?.parse().ok()
}

/// The `w:numFmt` of every `w:lvl` under the element.
fn level_formats(element: &Element) -> HashMap<u8, String> {
    element
        .elements()
        .filter(|child| child.is(W, "lvl"))
        .filter_map(|level| {
            let index = level.attribute(W, "ilvl")?.parse::<u8>().ok()?;
            let format = level.child(W, "numFmt")?.attribute(W, "val")?;
            Some((index, format.to_owned()))
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
    fn refuses_a_malformed_numbering_part() {
        assert!(Numbering::parse(Some("<w:numbering><w:num>")).is_err());
    }
}
