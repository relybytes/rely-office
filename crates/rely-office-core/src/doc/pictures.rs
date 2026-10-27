// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Pictures and drawings: an inline picture's `PICF` in the Data stream ([MS-DOC] 2.9.192), the
//! `OfficeArt` records that hold the image bytes ([MS-ODRAW]), and the floating shapes the
//! main text anchors (`PlcfSpa`).

use std::collections::HashMap;

use super::fkp::plex;
use crate::compound::{u16_at, u32_at};

const HEADER_SIZE: usize = 8;
const CONTAINER_VERSION: u16 = 0xF;
const STORE_CONTAINER: u16 = 0xF001;
const DRAWING_CONTAINER: u16 = 0xF002;
const SHAPE_CONTAINER: u16 = 0xF004;
const STORE_ENTRY: u16 = 0xF007;
const SHAPE: u16 = 0xF00A;
const PROPERTIES: u16 = 0xF00B;
/// The properties Word 2000 and later add, the position ones among them.
const TERTIARY_PROPERTIES: u16 = 0xF122;
/// The property naming a picture's store entry, 1-based, and a text box's text.
const PICTURE_PROPERTY: u16 = 0x0104;
const TEXT_PROPERTY: u16 = 0x0080;
/// A `PICF` whose picture is a shape file names it after its header.
const SHAPE_FILE: i16 = 0x0066;
/// An `FSPA` is 26 bytes.
const ANCHOR_SIZE: usize = 26;
/// No drawing nests records deeper than this.
const MAX_DEPTH: usize = 16;

/// An `OfficeArt` record: its instance, type and body.
#[derive(Debug, Clone, Copy)]
struct Record<'a> {
    version: u16,
    instance: u16,
    kind: u16,
    body: &'a [u8],
}

fn records(bytes: &[u8]) -> impl Iterator<Item = Record<'_>> {
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

/// The image bytes of a blip record; metafiles, which nothing here draws, are left out.
fn blip(record: Record<'_>) -> Option<Vec<u8>> {
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
fn bitmap_file(dib: &[u8]) -> Vec<u8> {
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
fn store_entry(record: Record<'_>, delay: &[u8]) -> Option<Vec<u8>> {
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

/// An inline picture: its image bytes and its size in twips.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct InlinePicture {
    pub(super) bytes: Option<Vec<u8>>,
    pub(super) width: u32,
    pub(super) height: u32,
}

/// The picture whose `PICF` starts at `offset` in the Data stream.
pub(super) fn inline_picture(data: &[u8], offset: usize, delay: &[u8]) -> Option<InlinePicture> {
    let total = u32_at(data, offset)? as usize;
    let header = usize::from(u16_at(data, offset + 4)?);
    let picture = data.get(offset..offset.checked_add(total)?)?;
    let mode = i16::from_le_bytes(u16_at(picture, 6)?.to_le_bytes());
    let goal_width = u32::from(u16_at(picture, 28)?);
    let goal_height = u32::from(u16_at(picture, 30)?);
    let scale_x = u32::from(u16_at(picture, 32)?);
    let scale_y = u32::from(u16_at(picture, 34)?);
    let mut position = header;
    if mode == SHAPE_FILE {
        position += 1 + usize::from(*picture.get(position)?);
    }
    let art = picture.get(position..).unwrap_or_default();
    let mut bytes = None;
    let mut parts = records(art);
    parts.next();
    for record in parts {
        if record.kind == STORE_ENTRY
            && let Some(found) = store_entry(record, delay)
        {
            bytes = Some(found);
            break;
        }
    }
    Some(InlinePicture {
        bytes,
        width: goal_width * scale_x / 1000,
        height: goal_height * scale_y / 1000,
    })
}

/// Where a floating shape is anchored and how it stands (`FSPA`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ShapeAnchor {
    pub(super) shape: u32,
    /// The left, top, right and bottom edges, in twips.
    pub(super) bounds: [i32; 4],
    /// 0 the margin, 1 the page, 2 the column.
    pub(super) horizontal_base: u8,
    /// 0 the margin, 1 the page, 2 the paragraph.
    pub(super) vertical_base: u8,
    /// 1 above and below, 2, 4 and 5 around, 3 none, 0 around without room on either side.
    pub(super) wrap: u8,
    pub(super) behind_text: bool,
}

/// The floating shapes of a story, by the character position that anchors each.
pub(super) fn shape_anchors(plex_bytes: &[u8]) -> HashMap<usize, ShapeAnchor> {
    let Some((positions, elements)) = plex(plex_bytes, ANCHOR_SIZE) else {
        return HashMap::new();
    };
    let signed = |element: &[u8], offset: usize| {
        u32_at(element, offset).map_or(0, |value| i32::from_le_bytes(value.to_le_bytes()))
    };
    positions
        .iter()
        .zip(elements)
        .map(|(position, element)| {
            let flags = u16_at(element, 20).unwrap_or(0);
            (
                *position as usize,
                ShapeAnchor {
                    shape: u32_at(element, 0).unwrap_or(0),
                    bounds: [4, 8, 12, 16].map(|offset| signed(element, offset)),
                    horizontal_base: u8::try_from((flags >> 1) & 0x3).unwrap_or(0),
                    vertical_base: u8::try_from((flags >> 3) & 0x3).unwrap_or(0),
                    wrap: u8::try_from((flags >> 5) & 0xF).unwrap_or(0),
                    behind_text: flags & 0x4000 != 0,
                },
            )
        })
        .collect()
}

/// What a drawing's shape shows, a picture's store entry or a text box's text, and how it is
/// drawn and placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ShapeContent {
    pub(super) picture: Option<u32>,
    pub(super) text: Option<u32>,
    /// The `COLORREF`s of the fill and the line, and the line's width in English Metric Units.
    pub(super) fill: Option<u32>,
    pub(super) line: Option<u32>,
    pub(super) line_width: Option<u32>,
    pub(super) filled: bool,
    pub(super) stroked: bool,
    /// `posh` and `posv`: 0 at the offset, 1 left or top, 2 centred, 3 right or bottom.
    pub(super) horizontal_align: Option<u32>,
    pub(super) vertical_align: Option<u32>,
    /// `posrelh` and `posrelv`: 0 the margin, 1 the page, 2 the column or paragraph, 3 the
    /// character or line.
    pub(super) horizontal_base: Option<u32>,
    pub(super) vertical_base: Option<u32>,
}

impl Default for ShapeContent {
    fn default() -> Self {
        Self {
            picture: None,
            text: None,
            fill: None,
            line: None,
            line_width: None,
            filled: true,
            stroked: true,
            horizontal_align: None,
            vertical_align: None,
            horizontal_base: None,
            vertical_base: None,
        }
    }
}

/// The drawings of a document (`OfficeArtContent`): the images of the store and what each
/// shape shows.
#[derive(Debug, Clone, Default)]
pub(super) struct Drawings {
    pub(super) images: Vec<Option<Vec<u8>>>,
    pub(super) shapes: HashMap<u32, ShapeContent>,
}

impl Drawings {
    pub(super) fn read(content: &[u8], delay: &[u8]) -> Self {
        let mut drawings = Self::default();
        let mut parts = records(content);
        if let Some(group) = parts.next() {
            for record in records(group.body) {
                if record.kind == STORE_CONTAINER {
                    drawings.images = records(record.body)
                        .filter(|entry| entry.kind == STORE_ENTRY)
                        .map(|entry| store_entry(entry, delay))
                        .collect();
                }
            }
        }
        // The drawings follow the group, each after the byte that says whose it is.
        let rest = content.get(parts_end(content)..).unwrap_or_default();
        let mut position = 0;
        while position < rest.len() {
            let Some(drawing) = rest
                .get(position + 1..)
                .and_then(|bytes| records(bytes).next())
            else {
                break;
            };
            position += 1 + HEADER_SIZE + drawing.body.len();
            if drawing.kind == DRAWING_CONTAINER {
                collect_shapes(drawing.body, &mut drawings.shapes, 0);
            }
        }
        drawings
    }

    /// The image of a store entry, 1-based as shapes name them.
    pub(super) fn image(&self, index: u32) -> Option<&[u8]> {
        self.images
            .get((index as usize).checked_sub(1)?)?
            .as_deref()
    }
}

/// Where the first record of `content` ends.
fn parts_end(content: &[u8]) -> usize {
    u32_at(content, 4).map_or(content.len(), |length| HEADER_SIZE + length as usize)
}

fn collect_shapes(container: &[u8], shapes: &mut HashMap<u32, ShapeContent>, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    for record in records(container) {
        if record.kind == SHAPE_CONTAINER {
            let mut identifier = None;
            let mut content = ShapeContent::default();
            for part in records(record.body) {
                match part.kind {
                    SHAPE => identifier = u32_at(part.body, 0),
                    PROPERTIES | TERTIARY_PROPERTIES => shape_properties(part, &mut content),
                    _ => {}
                }
            }
            if let Some(identifier) = identifier {
                shapes.insert(identifier, content);
            }
        }
        if record.version == CONTAINER_VERSION {
            collect_shapes(record.body, shapes, depth + 1);
        }
    }
}

/// What a shape's `OfficeArtFOPT` or `OfficeArtTertiaryFOPT` names, over what is known.
fn shape_properties(record: Record<'_>, content: &mut ShapeContent) {
    for index in 0..usize::from(record.instance) {
        let (Some(identifier), Some(value)) = (
            u16_at(record.body, index * 6),
            u32_at(record.body, index * 6 + 2),
        ) else {
            break;
        };
        match identifier & 0x3FFF {
            PICTURE_PROPERTY => content.picture = Some(value),
            TEXT_PROPERTY => content.text = Some(value),
            0x0181 => content.fill = Some(value),
            0x01C0 => content.line = Some(value),
            0x01CB => content.line_width = Some(value),
            // A boolean set holds each flag with a "use" bit sixteen places above it.
            0x01BF if value & (1 << 20) != 0 => content.filled = value & (1 << 4) != 0,
            0x01FF if value & (1 << 19) != 0 => content.stroked = value & (1 << 3) != 0,
            0x038F => content.horizontal_align = Some(value),
            0x0390 => content.horizontal_base = Some(value),
            0x0391 => content.vertical_align = Some(value),
            0x0392 => content.vertical_base = Some(value),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
