// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The drawing records Word and `PowerPoint` 97-2003 share ([MS-ODRAW]): records and
//! containers, the image store with its blips, and the property tables of shapes.

use crate::compound::{u16_at, u32_at};

const HEADER_SIZE: usize = 8;
pub(crate) const CONTAINER_VERSION: u16 = 0xF;
pub(crate) const GROUP_CONTAINER: u16 = 0xF000;
pub(crate) const STORE_CONTAINER: u16 = 0xF001;
pub(crate) const DRAWING_CONTAINER: u16 = 0xF002;
pub(crate) const SHAPE_GROUP_CONTAINER: u16 = 0xF003;
pub(crate) const SHAPE_CONTAINER: u16 = 0xF004;
pub(crate) const STORE_ENTRY: u16 = 0xF007;
pub(crate) const GROUP_SHAPE: u16 = 0xF009;
pub(crate) const SHAPE: u16 = 0xF00A;
pub(crate) const PROPERTIES: u16 = 0xF00B;
pub(crate) const CLIENT_TEXTBOX: u16 = 0xF00D;
pub(crate) const CHILD_ANCHOR: u16 = 0xF00F;
pub(crate) const CLIENT_ANCHOR: u16 = 0xF010;
pub(crate) const CLIENT_DATA: u16 = 0xF011;
/// The properties later versions add, the position ones among them.
pub(crate) const TERTIARY_PROPERTIES: u16 = 0xF122;

/// A record: its version, instance, type and body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Record<'a> {
    pub(crate) version: u16,
    pub(crate) instance: u16,
    pub(crate) kind: u16,
    pub(crate) body: &'a [u8],
}

impl Record<'_> {
    pub(crate) fn is_container(&self) -> bool {
        self.version == CONTAINER_VERSION
    }

    /// The length of the record with its header.
    pub(crate) fn length(&self) -> usize {
        HEADER_SIZE + self.body.len()
    }
}

/// The records one after another in `bytes`, up to the first that does not fit.
pub(crate) fn records(bytes: &[u8]) -> impl Iterator<Item = Record<'_>> {
    let mut position = 0;
    std::iter::from_fn(move || {
        let first = u16_at(bytes, position)?;
        let kind = u16_at(bytes, position + 2)?;
        let length = u32_at(bytes, position + 4)? as usize;
        let start = position + HEADER_SIZE;
        let body = bytes.get(start..start.checked_add(length)?)?;
        position = start + length;
        Some(Record {
            version: first & 0x000F,
            instance: first >> 4,
            kind,
            body,
        })
    })
}

/// The first child of a container that has a type.
pub(crate) fn child(body: &[u8], kind: u16) -> Option<Record<'_>> {
    records(body).find(|record| record.kind == kind)
}

/// The image bytes of a blip record; metafiles, which nothing here draws, are left out.
pub(crate) fn blip(record: Record<'_>) -> Option<Vec<u8>> {
    let identifiers = match (record.kind, record.instance) {
        (0xF01D | 0xF02A, 0x46B | 0x6E3) | (0xF01E, 0x6E1) | (0xF01F, 0x7A9) | (0xF029, 0x6E5) => 2,
        (0xF01D | 0xF01E | 0xF01F | 0xF029 | 0xF02A, _) => 1,
        _ => return None,
    };
    let start = 16 * identifiers + 1;
    let bytes = record.body.get(start..)?;
    if record.kind == 0xF01F {
        return Some(bitmap_file(bytes));
    }
    Some(bytes.to_vec())
}

/// A device-independent bitmap with the file header a `.bmp` begins with.
pub(crate) fn bitmap_file(dib: &[u8]) -> Vec<u8> {
    let header_length = u32_at(dib, 0).unwrap_or(40);
    let bit_count = u16_at(dib, 14).unwrap_or(24);
    let colours = match u32_at(dib, 32).unwrap_or(0) {
        0 if bit_count <= 8 => 1_u32 << bit_count,
        used => used,
    };
    let pixels_offset = 14 + header_length + colours * 4;
    let total = u32::try_from(dib.len() + 14).unwrap_or(u32::MAX);
    let mut file = Vec::with_capacity(dib.len() + 14);
    file.extend_from_slice(b"BM");
    file.extend_from_slice(&total.to_le_bytes());
    file.extend_from_slice(&[0; 4]);
    file.extend_from_slice(&pixels_offset.to_le_bytes());
    file.extend_from_slice(dib);
    file
}

/// The image of a store entry (`FBSE`): the blip it embeds, or the one at its offset in the
/// delay stream.
pub(crate) fn store_entry(record: Record<'_>, delay: &[u8]) -> Option<Vec<u8>> {
    let name_length = usize::from(*record.body.get(33)?);
    let embedded = record
        .body
        .get(36 + name_length..)
        .filter(|rest| !rest.is_empty());
    if let Some(found) = embedded
        .and_then(|rest| records(rest).next())
        .and_then(blip)
    {
        return Some(found);
    }
    let size = u32_at(record.body, 20)? as usize;
    let offset = u32_at(record.body, 28)? as usize;
    let delayed = delay.get(offset..offset.checked_add(size)?)?;
    records(delayed).next().and_then(blip)
}

/// The images of a store container, in order, `None` for an entry that holds none.
pub(crate) fn store_images(store: &[u8], delay: &[u8]) -> Vec<Option<Vec<u8>>> {
    records(store)
        .filter(|entry| entry.kind == STORE_ENTRY)
        .map(|entry| store_entry(entry, delay))
        .collect()
}

/// The simple properties of an `OfficeArtFOPT` or `OfficeArtTertiaryFOPT`, by their number
/// without the blip and complex flags.
pub(crate) fn properties(record: Record<'_>) -> impl Iterator<Item = (u16, u32)> + '_ {
    (0..usize::from(record.instance)).map_while(move |index| {
        let identifier = u16_at(record.body, index * 6)?;
        let value = u32_at(record.body, index * 6 + 2)?;
        Some((identifier & 0x3FFF, value))
    })
}

/// The complex properties that hold an `IMsoArray`: the vertices, segments, adjust handles,
/// guides, inscribed rectangles and connection sites of a geometry, the shade colours of a
/// fill, a line's dashes and a wrap polygon.
const ARRAY_PROPERTIES: [u16; 9] = [
    0x0145, 0x0146, 0x0155, 0x0156, 0x0157, 0x0158, 0x0197, 0x01CF, 0x0383,
];

/// The data of each complex property of a table, by its number: the data follows the simple
/// entries in the order the entries name it, each entry's value giving its length.
pub(crate) fn complex_properties(record: Record<'_>) -> Vec<(u16, &[u8])> {
    let count = usize::from(record.instance);
    let mut position = count * 6;
    let mut found = Vec::new();
    for index in 0..count {
        let (Some(identifier), Some(length)) = (
            u16_at(record.body, index * 6),
            u32_at(record.body, index * 6 + 2),
        ) else {
            break;
        };
        if identifier & 0x8000 == 0 {
            continue;
        }
        // An array's length leaves out the six bytes of its header.
        let length = if ARRAY_PROPERTIES.contains(&(identifier & 0x3FFF)) {
            length as usize + 6
        } else {
            length as usize
        };
        let Some(data) = record.body.get(position..position.saturating_add(length)) else {
            break;
        };
        found.push((identifier & 0x3FFF, data));
        position += data.len();
    }
    found
}

/// The elements of an `IMsoArray`: a count, a capacity and the size of each element, which is
/// four when the size says `0xFFF0`.
pub(crate) fn array_elements(data: &[u8]) -> Vec<&[u8]> {
    let (Some(count), Some(size)) = (u16_at(data, 0), u16_at(data, 4)) else {
        return Vec::new();
    };
    let size = if size == 0xFFF0 { 4 } else { usize::from(size) };
    if size == 0 {
        return Vec::new();
    }
    data.get(6..)
        .unwrap_or_default()
        .chunks_exact(size)
        .take(usize::from(count))
        .collect()
}

/// A boolean property set: the flag at `bit`, when its "use" bit sixteen places above is set.
pub(crate) fn flag(value: u32, bit: u32) -> Option<bool> {
    (value & (1 << (bit + 16)) != 0).then_some(value & (1 << bit) != 0)
}

#[cfg(test)]
mod tests;
