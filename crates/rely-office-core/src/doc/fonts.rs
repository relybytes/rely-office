// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The font table ([MS-DOC] 2.9.270, `SttbfFfn`): the name of each font a run names by index.

use crate::compound::u16_at;

/// Where the name starts inside an `FFN`: the size byte, the family byte, the weight, the
/// character set, the alternate name's index, the PANOSE number and the font signature.
const NAME_OFFSET: usize = 40;

/// The name of each font, by its index; a font that cannot be read has an empty name.
pub(super) fn font_names(table: &[u8]) -> Vec<String> {
    let Some(count) = u16_at(table, 0) else {
        return Vec::new();
    };
    let mut names = Vec::with_capacity(usize::from(count));
    let mut position = 4;
    for _ in 0..count {
        let Some(size) = table.get(position).map(|size| usize::from(*size) + 1) else {
            break;
        };
        let Some(font) = table.get(position..position + size) else {
            break;
        };
        let units: Vec<u16> = font
            .get(NAME_OFFSET..)
            .unwrap_or_default()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .take_while(|unit| *unit != 0)
            .collect();
        names.push(String::from_utf16_lossy(&units));
        position += size;
    }
    names
}

#[cfg(test)]
mod tests;
