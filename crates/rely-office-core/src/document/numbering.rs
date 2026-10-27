// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The numbering part (`word/numbering.xml`): whether a numbered paragraph shows a bullet or a
//! number, the label it shows, and how far each level is indented.

use std::collections::{HashMap, HashSet};

use super::look::{LabelSuffix, ListLabel, ParagraphLook, RunLook};
use crate::OfficeError;
use crate::xml_tree::{Element, Namespace, parse_tree};

const W: Namespace = Namespace::Word;
/// Word numbers nine levels, 0 to 8.
const LEVELS: usize = 9;
/// The longest label kept; a real one is a few characters.
const MAX_LABEL_CHARS: usize = 64;
/// A number past this is a crafted file.
const MAX_NUMBER: u32 = 99_999;

/// One level of a list definition.
#[derive(Debug, Clone, Default)]
struct Level {
    format: Option<String>,
    /// `w:lvlText`, where `%1` to `%9` stand for the numbers of the levels.
    text: Option<String>,
    start: Option<u32>,
    suffix: Option<LabelSuffix>,
    font: RunLook,
    look: ParagraphLook,
}

/// The numbers each list has reached, as a document is read from the top.
#[derive(Debug, Default)]
pub(crate) struct ListCounters {
    /// Abstract list to the number of each level; `None` for a level not started yet.
    numbers: HashMap<u32, [Option<u32>; LEVELS]>,
    /// The list instances already met, whose start overrides have been applied.
    seen: HashSet<u32>,
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
    /// A level of a list instance: its override over the abstract definition, field by field.
    fn level(&self, num_id: u32, level: u8) -> Option<(u32, Level)> {
        let (abstract_id, overrides) = self.instances.get(&num_id)?;
        let defined = self
            .abstract_levels
            .get(abstract_id)
            .and_then(|levels| levels.get(&level))
            .cloned()
            .unwrap_or_default();
        let Some(overridden) = overrides.get(&level).cloned() else {
            return Some((*abstract_id, defined));
        };
        Some((
            *abstract_id,
            Level {
                format: overridden.format.or(defined.format),
                text: overridden.text.or(defined.text),
                start: overridden.start.or(defined.start),
                suffix: overridden.suffix.or(defined.suffix),
                font: overridden.font.or(defined.font),
                look: overridden.look.or(defined.look),
            },
        ))
    }

    /// Count a paragraph of level `level` of list `num_id` and give the label it shows; `None`
    /// when the paragraph is not a list item. Instances of one abstract list share their numbers,
    /// as in Word, unless an instance overrides the start.
    pub(crate) fn next_label(
        &self,
        counters: &mut ListCounters,
        num_id: u32,
        level: u8,
    ) -> Option<ListLabel> {
        self.is_ordered(num_id, level)?;
        let (abstract_id, definition) = self.level(num_id, level)?;
        let index = usize::from(level).min(LEVELS - 1);
        let numbers = counters
            .numbers
            .entry(abstract_id)
            .or_insert([None; LEVELS]);
        if counters.seen.insert(num_id)
            && let Some((_, overrides)) = self.instances.get(&num_id)
        {
            for (overridden_level, overridden) in overrides {
                if overridden.start.is_some()
                    && let Some(slot) = numbers.get_mut(usize::from(*overridden_level))
                {
                    *slot = None;
                }
            }
        }
        numbers[index] = Some(match numbers[index] {
            Some(number) => number.saturating_add(1).min(MAX_NUMBER),
            None => definition.start.unwrap_or(1).min(MAX_NUMBER),
        });
        for deeper in &mut numbers[index + 1..] {
            *deeper = None;
        }
        let numbers = *numbers;

        let format = definition.format.as_deref().unwrap_or("decimal");
        let text = match &definition.text {
            Some(text) => text.clone(),
            None if format == "bullet" => "\u{2022}".to_owned(),
            None => format!("%{}.", index + 1),
        };
        let mut label = String::new();
        let mut characters = text.chars().peekable();
        while let Some(character) = characters.next() {
            let digit = characters
                .peek()
                .and_then(|digit| digit.to_digit(10))
                .filter(|digit| (1..=9).contains(digit));
            match digit {
                Some(digit) if character == '%' => {
                    characters.next();
                    let referenced = usize::try_from(digit - 1).unwrap_or(0);
                    let referenced_level = self
                        .level(num_id, u8::try_from(referenced).unwrap_or(0))
                        .map(|(_, level)| level)
                        .unwrap_or_default();
                    let referenced_format = if referenced == index {
                        format
                    } else {
                        referenced_level.format.as_deref().unwrap_or("decimal")
                    };
                    let number =
                        numbers[referenced].unwrap_or_else(|| referenced_level.start.unwrap_or(1));
                    label.push_str(&format_number(number, referenced_format));
                }
                _ => label.push(bullet_character(
                    character,
                    definition.font.family.as_deref(),
                )),
            }
        }
        Some(ListLabel {
            text: label.chars().take(MAX_LABEL_CHARS).collect(),
            suffix: definition.suffix.unwrap_or(LabelSuffix::Tab),
            font: definition.font,
        })
    }

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

/// Every `w:lvl` under the element; a `w:lvlOverride` with only a start override restarts its
/// level.
fn levels(element: &Element) -> HashMap<u8, Level> {
    let mut levels: HashMap<u8, Level> = element
        .elements()
        .filter(|child| child.is(W, "lvl"))
        .filter_map(|level| {
            let index = level.attribute(W, "ilvl")?.parse::<u8>().ok()?;
            Some((index, read_level(level)))
        })
        .collect();
    if element.is(W, "lvlOverride")
        && let Some(index) = element
            .attribute(W, "ilvl")
            .and_then(|index| index.parse::<u8>().ok())
        && let Some(start) = element
            .child(W, "startOverride")
            .and_then(|start| number_attribute(start, "val"))
    {
        levels.entry(index).or_default().start = Some(start);
    }
    levels
}

fn read_level(level: &Element) -> Level {
    Level {
        format: level
            .child(W, "numFmt")
            .and_then(|format| format.attribute(W, "val"))
            .map(str::to_owned),
        text: level
            .child(W, "lvlText")
            .and_then(|text| text.attribute(W, "val"))
            .map(str::to_owned),
        start: level
            .child(W, "start")
            .and_then(|start| number_attribute(start, "val")),
        suffix: level
            .child(W, "suff")
            .and_then(|suffix| suffix.attribute(W, "val"))
            .map(|suffix| match suffix {
                "space" => LabelSuffix::Space,
                "nothing" => LabelSuffix::Nothing,
                _ => LabelSuffix::Tab,
            }),
        font: level
            .child(W, "rPr")
            .map(RunLook::parse)
            .unwrap_or_default(),
        look: level
            .child(W, "pPr")
            .map(ParagraphLook::parse)
            .unwrap_or_default(),
    }
}

/// A number written in a level's format; a format with no rule of its own is decimal.
fn format_number(number: u32, format: &str) -> String {
    match format {
        "lowerLetter" => alphabetic(number),
        "upperLetter" => alphabetic(number).to_ascii_uppercase(),
        "lowerRoman" => roman(number),
        "upperRoman" => roman(number).to_ascii_uppercase(),
        "decimalZero" => format!("{number:02}"),
        "ordinal" => format!("{number}\u{ba}"),
        "bullet" | "none" => String::new(),
        _ => number.to_string(),
    }
}

/// A label character as it shows: the private-use glyphs of the symbol fonts become the bullets
/// they draw there, and the `o` of Courier New a white bullet.
fn bullet_character(character: char, font: Option<&str>) -> char {
    match character {
        '\u{f0a7}' | '\u{f06e}' | '\u{f0a8}' => '\u{25aa}',
        '\u{f0d8}' | '\u{f0e8}' => '\u{27a2}',
        '\u{f0fc}' => '\u{2713}',
        '\u{f076}' => '\u{2756}',
        '\u{f02d}' | '\u{f0be}' => '\u{2013}',
        '\u{f000}'..='\u{f0ff}' | '\u{b7}' => '\u{2022}',
        'o' if font.is_some_and(|font| font.to_ascii_lowercase().contains("courier")) => '\u{25e6}',
        other => other,
    }
}

fn alphabetic(number: u32) -> String {
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
