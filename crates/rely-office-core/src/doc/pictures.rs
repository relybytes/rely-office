// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! Pictures and drawings: an inline picture's `PICF` in the Data stream ([MS-DOC] 2.9.192), the
//! `OfficeArt` records that hold the image bytes ([MS-ODRAW]), and the floating shapes the
//! main text anchors (`PlcfSpa`).

use std::collections::HashMap;

use super::fkp::plex;
use crate::compound::{u16_at, u32_at};
use crate::office_art::{
    DRAWING_CONTAINER, PROPERTIES, Record, SHAPE, SHAPE_CONTAINER, STORE_CONTAINER, STORE_ENTRY,
    TERTIARY_PROPERTIES, flag, properties, records, store_entry, store_images,
};

/// The property naming a picture's store entry, 1-based, and a text box's text.
const PICTURE_PROPERTY: u16 = 0x0104;
const TEXT_PROPERTY: u16 = 0x0080;
/// A `PICF` whose picture is a shape file names it after its header.
const SHAPE_FILE: i16 = 0x0066;
/// An `FSPA` is 26 bytes.
const ANCHOR_SIZE: usize = 26;
/// No drawing nests records deeper than this.
const MAX_DEPTH: usize = 16;

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
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is a shape property of its own"
)]
pub(super) struct ShapeContent {
    /// The shape type, `MSOSPT`, and whether the shape is mirrored across and down.
    pub(super) kind: u16,
    pub(super) flip_horizontal: bool,
    pub(super) flip_vertical: bool,
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
    /// `adjustValue` to `adjust10Value`, in the shape type's 21,600 units.
    pub(super) adjust: [Option<i64>; 10],
}

impl Default for ShapeContent {
    fn default() -> Self {
        Self {
            kind: 1,
            flip_horizontal: false,
            flip_vertical: false,
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
            adjust: [None; 10],
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
                    drawings.images = store_images(record.body, delay);
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
            position += 1 + drawing.length();
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
    records(content)
        .next()
        .map_or(content.len(), |first| first.length())
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
                    SHAPE => {
                        identifier = u32_at(part.body, 0);
                        content.kind = part.instance;
                        let flags = u32_at(part.body, 4).unwrap_or(0);
                        content.flip_horizontal = flags & 0x40 != 0;
                        content.flip_vertical = flags & 0x80 != 0;
                    }
                    PROPERTIES | TERTIARY_PROPERTIES => shape_properties(part, &mut content),
                    _ => {}
                }
            }
            if let Some(identifier) = identifier {
                shapes.insert(identifier, content);
            }
        }
        if record.is_container() {
            collect_shapes(record.body, shapes, depth + 1);
        }
    }
}

/// What a shape's `OfficeArtFOPT` or `OfficeArtTertiaryFOPT` names, over what is known.
fn shape_properties(record: Record<'_>, content: &mut ShapeContent) {
    for (identifier, value) in properties(record) {
        match identifier {
            PICTURE_PROPERTY => content.picture = Some(value),
            TEXT_PROPERTY => content.text = Some(value),
            0x0181 => content.fill = Some(value),
            0x01C0 => content.line = Some(value),
            0x01CB => content.line_width = Some(value),
            0x01BF => content.filled = flag(value, 4).unwrap_or(content.filled),
            0x01FF => content.stroked = flag(value, 3).unwrap_or(content.stroked),
            0x038F => content.horizontal_align = Some(value),
            0x0390 => content.horizontal_base = Some(value),
            0x0391 => content.vertical_align = Some(value),
            0x0392 => content.vertical_base = Some(value),
            0x0147..=0x0150 => {
                content.adjust[usize::from(identifier - 0x0147)] =
                    Some(i64::from(i32::from_le_bytes(value.to_le_bytes())));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
