// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! Lists ([MS-DOC] 2.9.147 and 2.9.150): each list's levels with their number format, label
//! template, start, follow character and modifiers, and the overrides paragraphs name.

use std::collections::HashMap;

use crate::compound::{u16_at, u32_at};

/// A list definition (`LSTF`) is 28 bytes, and so is the fixed part of a level (`LVLF`).
const DEFINITION_SIZE: usize = 28;
const LEVEL_SIZE: usize = 28;
/// A list override (`LFO`) is 16 bytes, and each of its level overrides (`LFOLVL`) eight.
const OVERRIDE_SIZE: usize = 16;
const LEVEL_OVERRIDE_SIZE: usize = 8;
/// No document declares more lists, levels or overrides than this.
const MAX_LISTS: usize = 4_096;

/// How a level numbers its paragraphs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Level {
    pub(super) start: u32,
    /// The `nfc`: 0 decimal, 1 and 2 upper and lower roman, 3 and 4 upper and lower letters,
    /// 23 a bullet, 255 nothing.
    pub(super) format: u8,
    /// The label, each character below 9 standing for that level's number.
    pub(super) template: Vec<u16>,
    /// 0 a tab after the label, 1 a space, 2 nothing.
    pub(super) follow: u8,
    pub(super) paragraph: Vec<u8>,
    pub(super) character: Vec<u8>,
}

impl Level {
    pub(super) fn ordered(&self) -> bool {
        self.format != 23 && self.format != 255
    }

    /// The name `format_number` knows the format by.
    pub(super) fn format_name(&self) -> &'static str {
        match self.format {
            1 => "upperRoman",
            2 => "lowerRoman",
            3 => "upperLetter",
            4 => "lowerLetter",
            5 => "ordinal",
            22 => "decimalZero",
            23 => "bullet",
            255 => "none",
            _ => "decimal",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Lists {
    /// Each list's levels by its id.
    definitions: HashMap<i32, Vec<Level>>,
    /// Each override: the list it names and the starts it sets, by level.
    overrides: Vec<(i32, HashMap<u8, u32>)>,
}

impl Lists {
    /// The lists from the table stream: the definitions with their levels after them, and the
    /// overrides with their level data after them.
    pub(super) fn read(table: &[u8], definitions: Option<usize>, overrides: Option<usize>) -> Self {
        let mut lists = Self::default();
        if let Some(offset) = definitions {
            lists.definitions = read_definitions(table, offset);
        }
        if let Some(offset) = overrides {
            lists.overrides = read_overrides(table, offset);
        }
        lists
    }

    /// The list a paragraph's 1-based override names, its id and its levels.
    pub(super) fn list(&self, override_index: u16) -> Option<(i32, &[Level])> {
        let (id, _) = self
            .overrides
            .get(usize::from(override_index).checked_sub(1)?)?;
        Some((*id, self.definitions.get(id)?.as_slice()))
    }

    /// The start an override sets for a level, over the level's own.
    pub(super) fn start_override(&self, override_index: u16, level: u8) -> Option<u32> {
        let (_, starts) = self
            .overrides
            .get(usize::from(override_index).checked_sub(1)?)?;
        starts.get(&level).copied()
    }
}

fn read_definitions(table: &[u8], offset: usize) -> HashMap<i32, Vec<Level>> {
    let mut definitions = HashMap::new();
    let Some(count) = u16_at(table, offset).map(|count| usize::from(count).min(MAX_LISTS)) else {
        return definitions;
    };
    let mut position = offset + 2 + count * DEFINITION_SIZE;
    for index in 0..count {
        let definition = offset + 2 + index * DEFINITION_SIZE;
        let Some(id) = u32_at(table, definition).map(|id| i32::from_le_bytes(id.to_le_bytes()))
        else {
            break;
        };
        let simple = table
            .get(definition + 26)
            .is_some_and(|flags| flags & 1 != 0);
        let mut levels = Vec::new();
        for _ in 0..(if simple { 1 } else { 9 }) {
            let Some((level, next)) = read_level(table, position) else {
                break;
            };
            levels.push(level);
            position = next;
        }
        definitions.insert(id, levels);
    }
    definitions
}

/// An `LVL` and the position after it.
fn read_level(table: &[u8], position: usize) -> Option<(Level, usize)> {
    let fixed = table.get(position..position + LEVEL_SIZE)?;
    let start = u32_at(fixed, 0)?;
    let character_length = usize::from(fixed[24]);
    let paragraph_length = usize::from(fixed[25]);
    let paragraph_start = position + LEVEL_SIZE;
    let character_start = paragraph_start + paragraph_length;
    let text_start = character_start + character_length;
    let paragraph = table.get(paragraph_start..character_start)?.to_vec();
    let character = table.get(character_start..text_start)?.to_vec();
    let text_length = usize::from(u16_at(table, text_start)?);
    let template = table
        .get(text_start + 2..text_start + 2 + text_length * 2)?
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .collect();
    Some((
        Level {
            start: if start > 0x7FFF { 1 } else { start },
            format: fixed[4],
            template,
            follow: fixed[15],
            paragraph,
            character,
        },
        text_start + 2 + text_length * 2,
    ))
}

fn read_overrides(table: &[u8], offset: usize) -> Vec<(i32, HashMap<u8, u32>)> {
    let Some(count) = u32_at(table, offset).map(|count| (count as usize).min(MAX_LISTS)) else {
        return Vec::new();
    };
    let mut overrides = Vec::with_capacity(count);
    let mut position = offset + 4 + count * OVERRIDE_SIZE;
    for index in 0..count {
        let entry = offset + 4 + index * OVERRIDE_SIZE;
        let (Some(id), Some(level_count)) = (u32_at(table, entry), table.get(entry + 12)) else {
            break;
        };
        let mut starts = HashMap::new();
        // Each override's data is a character position, then its level overrides.
        position += 4;
        for _ in 0..*level_count {
            let (Some(start), Some(flags)) = (u32_at(table, position), u32_at(table, position + 4))
            else {
                break;
            };
            position += LEVEL_OVERRIDE_SIZE;
            let level = u8::try_from(flags & 0x0F).unwrap_or(0);
            if flags & 0x10 != 0 {
                starts.insert(level, start);
            }
            if flags & 0x20 != 0 {
                match read_level(table, position) {
                    Some((level, next)) => {
                        starts
                            .entry(u8::try_from(flags & 0x0F).unwrap_or(0))
                            .or_insert(level.start);
                        position = next;
                    }
                    None => break,
                }
            }
        }
        overrides.push((i32::from_le_bytes(id.to_le_bytes()), starts));
    }
    overrides
}

#[cfg(test)]
mod tests;
