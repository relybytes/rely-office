// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Text formatting of `PowerPoint` 97-2003 ([MS-PPT] 2.9): the paragraph and character
//! exceptions, each a mask of what it sets followed by those fields in a fixed order, as runs
//! of a text (`StyleTextPropAtom`) and as the levels of a master style (`TxMasterStyleAtom`).

use crate::compound::{u16_at, u32_at};

/// No text has more runs, nor a master style more levels.
const MAX_RUNS: usize = 4_096;
const MAX_LEVELS: usize = 9;

/// A colour: red, green and blue, or an index into the slide's colour scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Colour {
    Rgb(u8, u8, u8),
    Scheme(u8),
}

/// A `ColorIndexStruct`.
fn colour(bytes: &[u8], position: usize) -> Option<Colour> {
    let [red, green, blue, index] = bytes.get(position..position + 4)?.try_into().ok()?;
    match index {
        0xFE => Some(Colour::Rgb(red, green, blue)),
        0..=7 => Some(Colour::Scheme(index)),
        _ => None,
    }
}

/// What a paragraph exception sets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct ParagraphStyle {
    pub(super) bullet: Option<bool>,
    pub(super) bullet_character: Option<u16>,
    pub(super) bullet_font: Option<u16>,
    pub(super) bullet_colour: Option<Colour>,
    /// 0 left, 1 centred, 2 right, 3 justified.
    pub(super) align: Option<u16>,
    /// Positive in percent of a line, negative in master units.
    pub(super) line_spacing: Option<i16>,
    pub(super) space_before: Option<i16>,
    pub(super) space_after: Option<i16>,
    /// In master units, 576 to the inch.
    pub(super) left_margin: Option<i16>,
    pub(super) indent: Option<i16>,
}

impl ParagraphStyle {
    /// This style's values over `fallback`'s.
    pub(super) fn or(self, fallback: Self) -> Self {
        Self {
            bullet: self.bullet.or(fallback.bullet),
            bullet_character: self.bullet_character.or(fallback.bullet_character),
            bullet_font: self.bullet_font.or(fallback.bullet_font),
            bullet_colour: self.bullet_colour.or(fallback.bullet_colour),
            align: self.align.or(fallback.align),
            line_spacing: self.line_spacing.or(fallback.line_spacing),
            space_before: self.space_before.or(fallback.space_before),
            space_after: self.space_after.or(fallback.space_after),
            left_margin: self.left_margin.or(fallback.left_margin),
            indent: self.indent.or(fallback.indent),
        }
    }
}

/// What a character exception sets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct CharacterStyle {
    pub(super) bold: Option<bool>,
    pub(super) italic: Option<bool>,
    pub(super) underline: Option<bool>,
    /// The font's index in the font collection.
    pub(super) font: Option<u16>,
    /// In points.
    pub(super) size: Option<u16>,
    pub(super) colour: Option<Colour>,
}

impl CharacterStyle {
    pub(super) fn or(self, fallback: Self) -> Self {
        Self {
            bold: self.bold.or(fallback.bold),
            italic: self.italic.or(fallback.italic),
            underline: self.underline.or(fallback.underline),
            font: self.font.or(fallback.font),
            size: self.size.or(fallback.size),
            colour: self.colour.or(fallback.colour),
        }
    }
}

/// Reads the optional fields of an exception in order; a field cut short marks it failed.
struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
    masks: u32,
    failed: bool,
}

impl Cursor<'_> {
    /// The two bytes of a field when any of `mask` is set, the cursor moved past them.
    fn word(&mut self, mask: u32) -> Option<u16> {
        if self.masks & mask == 0 {
            return None;
        }
        let value = u16_at(self.bytes, self.position);
        self.failed |= value.is_none();
        self.position += 2;
        value
    }

    fn signed(&mut self, mask: u32) -> Option<i16> {
        self.word(mask)
            .map(|value| i16::from_le_bytes(value.to_le_bytes()))
    }

    fn colour(&mut self, mask: u32) -> Option<Colour> {
        if self.masks & mask == 0 {
            return None;
        }
        self.failed |= self.bytes.get(self.position..self.position + 4).is_none();
        let value = colour(self.bytes, self.position);
        self.position += 4;
        value
    }

    fn skip(&mut self, mask: u32, length: usize) {
        if self.masks & mask != 0 {
            self.failed |= self
                .bytes
                .get(self.position..self.position + length)
                .is_none();
            self.position += length;
        }
    }
}

/// A `TextPFException` at `position`, and where it ends.
pub(super) fn paragraph_exception(
    bytes: &[u8],
    position: usize,
) -> Option<(ParagraphStyle, usize)> {
    let masks = u32_at(bytes, position)?;
    let mut cursor = Cursor {
        bytes,
        position: position + 4,
        masks,
        failed: false,
    };
    let flags = cursor.word(0x000F);
    let mut style = ParagraphStyle {
        bullet: flags
            .filter(|_| masks & 0x1 != 0)
            .map(|flags| flags & 0x1 != 0),
        bullet_character: cursor.word(0x0080),
        bullet_font: cursor.word(0x0010),
        ..ParagraphStyle::default()
    };
    cursor.skip(0x0040, 2);
    style.bullet_colour = cursor.colour(0x0020);
    style.align = cursor.word(0x0800);
    style.line_spacing = cursor.signed(0x1000);
    style.space_before = cursor.signed(0x2000);
    style.space_after = cursor.signed(0x4000);
    style.left_margin = cursor.signed(0x0100);
    style.indent = cursor.signed(0x0400);
    cursor.skip(0x8000, 2);
    if masks & 0x0010_0000 != 0 {
        let count = usize::from(u16_at(bytes, cursor.position)?);
        cursor.position += 2;
        cursor.skip(0x0010_0000, count * 4);
    }
    cursor.skip(0x0001_0000, 2);
    cursor.skip(0x000E_0000, 2);
    cursor.skip(0x0020_0000, 2);
    (!cursor.failed).then_some((style, cursor.position))
}

/// A `TextCFException` at `position`, and where it ends.
pub(super) fn character_exception(
    bytes: &[u8],
    position: usize,
) -> Option<(CharacterStyle, usize)> {
    let masks = u32_at(bytes, position)?;
    let mut cursor = Cursor {
        bytes,
        position: position + 4,
        masks,
        failed: false,
    };
    let font_style = cursor.word(0x0000_3EB7);
    let flag = |mask: u32, bit: u16| {
        font_style
            .filter(|_| masks & mask != 0)
            .map(|style| style & bit != 0)
    };
    let mut style = CharacterStyle {
        bold: flag(0x1, 0x1),
        italic: flag(0x2, 0x2),
        underline: flag(0x4, 0x4),
        font: cursor.word(0x0001_0000),
        ..CharacterStyle::default()
    };
    cursor.skip(0x0020_0000, 2);
    cursor.skip(0x0040_0000, 2);
    cursor.skip(0x0080_0000, 2);
    style.size = cursor.word(0x0002_0000);
    style.colour = cursor.colour(0x0004_0000);
    cursor.skip(0x0008_0000, 2);
    cursor.skip(0x0010_0000, 4);
    cursor.skip(0x0100_0000, 2);
    cursor.skip(0x0200_0000, 2);
    cursor.skip(0x0400_0000, 4);
    (!cursor.failed).then_some((style, cursor.position))
}

/// A paragraph run: how many characters it covers, its outline level and its style.
pub(super) type ParagraphRun = (usize, u16, ParagraphStyle);
/// A character run: how many characters it covers and its style.
pub(super) type CharacterRun = (usize, CharacterStyle);

/// The runs of a `StyleTextPropAtom` over a text of `length` characters, which the runs cover
/// with one more for the paragraph end the text leaves implied.
pub(super) fn style_runs(body: &[u8], length: usize) -> (Vec<ParagraphRun>, Vec<CharacterRun>) {
    let mut paragraphs = Vec::new();
    let mut position = 0;
    let mut covered = 0;
    while covered <= length && paragraphs.len() < MAX_RUNS {
        let (Some(count), Some(level)) = (u32_at(body, position), u16_at(body, position + 4))
        else {
            break;
        };
        let Some((style, next)) = paragraph_exception(body, position + 6) else {
            break;
        };
        paragraphs.push((count as usize, level, style));
        covered += (count as usize).max(1);
        position = next;
    }
    let mut characters = Vec::new();
    covered = 0;
    while covered <= length && characters.len() < MAX_RUNS {
        let Some(count) = u32_at(body, position) else {
            break;
        };
        let Some((style, next)) = character_exception(body, position + 4) else {
            break;
        };
        characters.push((count as usize, style));
        covered += (count as usize).max(1);
        position = next;
    }
    (paragraphs, characters)
}

/// The levels of a `TxMasterStyleAtom`: each level's paragraph and character style.
pub(super) fn master_levels(body: &[u8], instance: u16) -> Vec<(ParagraphStyle, CharacterStyle)> {
    let Some(count) = u16_at(body, 0) else {
        return Vec::new();
    };
    let mut levels = Vec::new();
    let mut position = 2;
    for _ in 0..usize::from(count).min(MAX_LEVELS) {
        if instance >= 5 {
            position += 2;
        }
        let Some((paragraph, next)) = paragraph_exception(body, position) else {
            break;
        };
        let Some((character, next)) = character_exception(body, next) else {
            break;
        };
        levels.push((paragraph, character));
        position = next;
    }
    levels
}

#[cfg(test)]
mod tests;
