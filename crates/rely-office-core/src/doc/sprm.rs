// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! Property modifiers ([MS-DOC] 2.2.5): a list of `Sprm`s, each a two-byte code whose top three
//! bits say how long its operand is.

/// The one modifier whose operand length takes two bytes, the cell layout of a table row.
const TABLE_DEFINITION: u16 = 0xD608;

/// The modifiers of a `grpprl`, each with its operand; a modifier cut short ends the list.
pub(super) fn modifiers(grpprl: &[u8]) -> Modifiers<'_> {
    Modifiers {
        bytes: grpprl,
        position: 0,
    }
}

pub(super) struct Modifiers<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Iterator for Modifiers<'a> {
    type Item = (u16, &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        let code = u16::from_le_bytes([
            *self.bytes.get(self.position)?,
            *self.bytes.get(self.position + 1)?,
        ]);
        let start = self.position + 2;
        let (operand_start, length) = match code >> 13 {
            0 | 1 => (start, 1),
            2 | 4 | 5 => (start, 2),
            3 => (start, 4),
            7 => (start, 3),
            _ if code == TABLE_DEFINITION => {
                let size =
                    u16::from_le_bytes([*self.bytes.get(start)?, *self.bytes.get(start + 1)?]);
                (start + 2, usize::from(size).saturating_sub(1))
            }
            _ => (start + 1, usize::from(*self.bytes.get(start)?)),
        };
        let end = operand_start.checked_add(length)?;
        let operand = self.bytes.get(operand_start..end)?;
        self.position = end;
        Some((code, operand))
    }
}

pub(super) fn byte(operand: &[u8]) -> u8 {
    operand.first().copied().unwrap_or(0)
}

pub(super) fn word(operand: &[u8]) -> u16 {
    match operand {
        [low, high, ..] => u16::from_le_bytes([*low, *high]),
        _ => 0,
    }
}

pub(super) fn signed_word(operand: &[u8]) -> i16 {
    i16::from_le_bytes(word(operand).to_le_bytes())
}

pub(super) fn long(operand: &[u8]) -> u32 {
    match operand {
        [a, b, c, d, ..] => u32::from_le_bytes([*a, *b, *c, *d]),
        _ => 0,
    }
}

#[cfg(test)]
mod tests;
