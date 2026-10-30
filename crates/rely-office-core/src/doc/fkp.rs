// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! Formatted disk pages ([MS-DOC] 2.9.44 and 2.9.175): the property runs of characters and
//! paragraphs, by their position in the `WordDocument` stream, and the plexes that list them.

use crate::compound::{u16_at, u32_at};

const PAGE_SIZE: usize = 512;
/// The page number of a `PnFkp*` is its low 22 bits.
const PAGE_NUMBER_MASK: u32 = 0x003F_FFFF;
/// A paragraph property run's `BxPap` is a byte offset and twelve bytes of height.
const PARAGRAPH_ENTRY: usize = 13;
/// The modifier that moves a paragraph's properties into the Data stream.
const HUGE_PARAGRAPH: u16 = 0x6646;

/// A plex ([MS-DOC] 2.2.2): `n + 1` positions and `n` elements of `size` bytes.
pub(super) fn plex(bytes: &[u8], size: usize) -> Option<(Vec<u32>, Vec<&[u8]>)> {
    let count = bytes.len().checked_sub(4)? / (4 + size);
    if bytes.len() != count * (4 + size) + 4 {
        return None;
    }
    let positions = (0..=count)
        .map(|index| u32_at(bytes, index * 4))
        .collect::<Option<Vec<_>>>()?;
    let base = (count + 1) * 4;
    let elements = (0..count)
        .map(|index| bytes.get(base + index * size..base + (index + 1) * size))
        .collect::<Option<Vec<_>>>()?;
    Some((positions, elements))
}

/// The positions of a plex whose elements carry no data.
pub(super) fn positions(bytes: &[u8]) -> Vec<u32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| u32::from_le_bytes(*chunk))
        .collect()
}

/// A run of characters `[start, end)` of the `WordDocument` stream and its modifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CharacterRun {
    pub(super) start: u32,
    pub(super) end: u32,
    pub(super) grpprl: Vec<u8>,
}

/// A paragraph's run of the `WordDocument` stream, its style and its modifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParagraphRun {
    pub(super) start: u32,
    pub(super) end: u32,
    pub(super) style: u16,
    pub(super) grpprl: Vec<u8>,
}

/// The pages a `PlcBteChpx` or `PlcBtePapx` names, in order.
fn pages<'w>(word_document: &'w [u8], plex_bytes: &[u8]) -> Vec<&'w [u8]> {
    let Some((_, elements)) = plex(plex_bytes, 4) else {
        return Vec::new();
    };
    elements
        .into_iter()
        .filter_map(|element| {
            let page = (u32_at(element, 0)? & PAGE_NUMBER_MASK) as usize;
            let start = page.checked_mul(PAGE_SIZE)?;
            word_document.get(start..start + PAGE_SIZE)
        })
        .collect()
}

/// The character runs of every `ChpxFkp`, sorted by position.
pub(super) fn character_runs(word_document: &[u8], plex_bytes: &[u8]) -> Vec<CharacterRun> {
    let mut runs = Vec::new();
    for page in pages(word_document, plex_bytes) {
        let count = usize::from(page[PAGE_SIZE - 1]);
        for index in 0..count {
            let (Some(start), Some(end)) = (u32_at(page, index * 4), u32_at(page, index * 4 + 4))
            else {
                break;
            };
            let Some(offset) = page
                .get((count + 1) * 4 + index)
                .map(|offset| usize::from(*offset) * 2)
            else {
                break;
            };
            let grpprl = if offset == 0 {
                Vec::new()
            } else {
                let length = usize::from(page.get(offset).copied().unwrap_or(0));
                page.get(offset + 1..offset + 1 + length)
                    .unwrap_or_default()
                    .to_vec()
            };
            runs.push(CharacterRun { start, end, grpprl });
        }
    }
    runs.sort_by_key(|run| run.start);
    runs
}

/// The paragraph runs of every `PapxFkp`, sorted by position; a run whose properties moved to
/// the Data stream is read from there.
pub(super) fn paragraph_runs(
    word_document: &[u8],
    plex_bytes: &[u8],
    data: &[u8],
) -> Vec<ParagraphRun> {
    let mut runs = Vec::new();
    for page in pages(word_document, plex_bytes) {
        let count = usize::from(page[PAGE_SIZE - 1]);
        for index in 0..count {
            let (Some(start), Some(end)) = (u32_at(page, index * 4), u32_at(page, index * 4 + 4))
            else {
                break;
            };
            let entry = (count + 1) * 4 + index * PARAGRAPH_ENTRY;
            let Some(offset) = page.get(entry).map(|offset| usize::from(*offset) * 2) else {
                break;
            };
            let (style, grpprl) = if offset == 0 {
                (0, Vec::new())
            } else {
                paragraph_properties(page, offset)
            };
            let grpprl = huge_properties(&grpprl, data).unwrap_or(grpprl);
            runs.push(ParagraphRun {
                start,
                end,
                style,
                grpprl,
            });
        }
    }
    runs.sort_by_key(|run| run.start);
    runs
}

/// A `PapxInFkp`: its count byte, or a zero and a count of words, then the style and modifiers.
fn paragraph_properties(page: &[u8], offset: usize) -> (u16, Vec<u8>) {
    let count = usize::from(page.get(offset).copied().unwrap_or(0));
    let (start, length) = if count == 0 {
        (
            offset + 2,
            usize::from(page.get(offset + 1).copied().unwrap_or(0)) * 2,
        )
    } else {
        (offset + 1, (count * 2).saturating_sub(1))
    };
    let Some(properties) = page.get(start..start + length) else {
        return (0, Vec::new());
    };
    let style = u16_at(properties, 0).unwrap_or(0);
    (style, properties.get(2..).unwrap_or_default().to_vec())
}

/// The modifiers a `sprmPHugePapx` points at in the Data stream.
fn huge_properties(grpprl: &[u8], data: &[u8]) -> Option<Vec<u8>> {
    let (_, operand) = super::sprm::modifiers(grpprl).find(|(code, _)| *code == HUGE_PARAGRAPH)?;
    let offset = super::sprm::long(operand) as usize;
    let length = usize::from(u16_at(data, offset)?);
    Some(data.get(offset + 2..offset + 2 + length)?.to_vec())
}

/// The run that holds a position, from runs sorted by start.
pub(super) fn run_at<T>(
    runs: &[T],
    position: u32,
    bounds: impl Fn(&T) -> (u32, u32),
) -> Option<&T> {
    let index = runs.partition_point(|run| bounds(run).0 <= position);
    let run = runs.get(index.checked_sub(1)?)?;
    let (start, end) = bounds(run);
    (start..end).contains(&position).then_some(run)
}

#[cfg(test)]
mod tests;
