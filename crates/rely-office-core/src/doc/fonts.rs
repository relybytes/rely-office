// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! The font table ([MS-DOC] 2.9.270, `SttbfFfn`): the name of each font a run names by index;
//! and the faces Word embedded ([MS-DOC] 2.9.296, `SttbTtmbd`), each an Embedded `OpenType`
//! structure in the `WordDocument` stream.

use crate::compound::{u16_at, u32_at};
use crate::{FaceStyle, MAX_FONT_FILE_BYTES, decompress_mtx};

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

/// The most faces `SttbTtmbd` lists.
const MAX_EMBEDDED_FACES: usize = 64;
/// The size of a `Ttmbd`: the offset, the font's index, the face bits and the subset order.
const TTMBD_SIZE: usize = 12;
/// The `MagicNumber` of an Embedded `OpenType` header.
const EOT_MAGIC: u16 = 0x504C;
/// The Embedded `OpenType` versions: 1.0, 2.1 and 2.2.
const EOT_VERSIONS: [u32; 3] = [0x0001_0000, 0x0002_0001, 0x0002_0002];
/// `TTEMBED_TTCOMPRESSED`: the font data is `MicroType` Express compressed.
const EOT_COMPRESSED: u32 = 0x0000_0004;
/// `TTEMBED_XORENCRYPTDATA`: every byte of the font data is combined with 0x50.
const EOT_XOR: u32 = 0x1000_0000;
const EOT_XOR_KEY: u8 = 0x50;

/// A face of a font the document embeds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EmbeddedFace {
    /// The index of its family in the font table.
    pub(super) font: usize,
    pub(super) style: FaceStyle,
    /// The font file; `None` when it cannot be read: damaged, cut short or too large.
    pub(super) bytes: Option<Vec<u8>>,
}

/// The faces listed in `SttbTtmbd`, their files read from the `WordDocument` stream.
pub(super) fn embedded_faces(table: &[u8], word_document: &[u8]) -> Vec<EmbeddedFace> {
    let count = u16_at(table, 2)
        .and_then(|count| usize::try_from(i16::from_le_bytes(count.to_le_bytes())).ok())
        .unwrap_or(0)
        .min(MAX_EMBEDDED_FACES);
    let start = u16_at(table, 8).map_or(10, usize::from);
    (0..count)
        .map_while(|index| table.get(start + index * TTMBD_SIZE..start + (index + 1) * TTMBD_SIZE))
        .filter_map(|entry| {
            let offset = u32_at(entry, 0)? as usize;
            let font = usize::try_from(i16::from_le_bytes(u16_at(entry, 4)?.to_le_bytes())).ok()?;
            let bits = u16_at(entry, 6)?;
            Some(EmbeddedFace {
                font,
                style: match (bits & 1 != 0, bits & 2 != 0) {
                    (false, false) => FaceStyle::Regular,
                    (true, false) => FaceStyle::Bold,
                    (false, true) => FaceStyle::Italic,
                    (true, true) => FaceStyle::BoldItalic,
                },
                bytes: (offset != 0)
                    .then(|| word_document.get(offset..))
                    .flatten()
                    .and_then(eot_font_data),
            })
        })
        .collect()
}

/// The font file inside an Embedded `OpenType` structure: its last member in every version,
/// taken back from the `XOR` that may hide it, then from `MicroType` Express when it is
/// compressed.
fn eot_font_data(eot: &[u8]) -> Option<Vec<u8>> {
    let size = u32_at(eot, 0)? as usize;
    let data_size = u32_at(eot, 4)?;
    if !EOT_VERSIONS.contains(&u32_at(eot, 8)?)
        || u16_at(eot, 34)? != EOT_MAGIC
        || u64::from(data_size) > MAX_FONT_FILE_BYTES
    {
        return None;
    }
    let flags = u32_at(eot, 12)?;
    let start = size.checked_sub(data_size as usize)?;
    let mut data = eot.get(start.max(36)..size)?.to_vec();
    if data.len() != data_size as usize {
        return None;
    }
    if flags & EOT_XOR != 0 {
        for byte in &mut data {
            *byte ^= EOT_XOR_KEY;
        }
    }
    if flags & EOT_COMPRESSED != 0 {
        return decompress_mtx(&data, usize::try_from(MAX_FONT_FILE_BYTES).ok()?);
    }
    Some(data)
}

#[cfg(test)]
mod tests;
