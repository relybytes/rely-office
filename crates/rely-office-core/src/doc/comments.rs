// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! The comments of a Word 97 document and the authors of its tracked changes ([MS-DOC]): each
//! comment's reference in the main text (`PlcfandRef` and its `ATRDPre10`), its text in the
//! comment story (`PlcfandTxt`), its author (`GrpXstAtnOwners`) and the text it covers, kept as
//! a bookmark of its own (`SttbfAtnBkmk`, `PlcfAtnBkf`, `PlcfAtnBkl`); and the names of the
//! reviewers a change names (`SttbfRMark`).

use std::ops::Range;

use super::fkp::{plex, positions};
use crate::compound::{u16_at, u32_at};

/// The size of an `ATRDPre10`.
const ATRD_SIZE: usize = 30;
/// The size of an `FBKF`.
const BOOKMARK_FIRST_SIZE: usize = 4;
/// The extra data of each `SttbfAtnBkmk` entry, an `ATNBE`.
const BOOKMARK_EXTRA_SIZE: usize = 10;
/// No document holds more comments than this.
const MAX_COMMENTS: usize = 10_000;

/// A comment as the document stores it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StoredComment {
    /// Where its reference character stands in the main text.
    pub(super) reference: usize,
    /// Its text, in the positions of the whole document.
    pub(super) text: Range<usize>,
    pub(super) author: String,
    pub(super) initials: Option<String>,
    /// The main text it covers; `None` for a comment on a point.
    pub(super) covers: Option<Range<usize>>,
}

/// The tables the comments are read from, each the bytes of its structure.
pub(super) struct CommentTables<'a> {
    pub(super) references: &'a [u8],
    pub(super) texts: &'a [u8],
    pub(super) owners: &'a [u8],
    pub(super) bookmark_names: &'a [u8],
    pub(super) bookmark_firsts: &'a [u8],
    pub(super) bookmark_lasts: &'a [u8],
}

/// The comments of a document in the order of their references; `story` is where the comment
/// story starts among the positions of the document.
pub(super) fn read_comments(tables: &CommentTables<'_>, story: usize) -> Vec<StoredComment> {
    let Some((references, records)) = plex(tables.references, ATRD_SIZE) else {
        return Vec::new();
    };
    let texts = plex(tables.texts, 0)
        .map(|(texts, _)| texts)
        .unwrap_or_default();
    let owners = strings(tables.owners);
    let ranges = bookmark_ranges(tables);
    references
        .iter()
        .zip(&records)
        .enumerate()
        .take(MAX_COMMENTS)
        .map(|(index, (reference, record))| {
            let initials_length = usize::from(u16_at(record, 0).unwrap_or(0)).min(9);
            let initials: String = char::decode_utf16(
                (0..initials_length).filter_map(|unit| u16_at(record, 2 + unit * 2)),
            )
            .map(|character| character.unwrap_or('\u{FFFD}'))
            .collect();
            let author = u16_at(record, 20)
                .and_then(|owner| owners.get(usize::from(owner)))
                .cloned()
                .unwrap_or_default();
            let tag = u32_at(record, 26).map(|tag| i32::from_le_bytes(tag.to_le_bytes()));
            let text = match (texts.get(index), texts.get(index + 1)) {
                (Some(start), Some(end)) if end > start => {
                    story + *start as usize..story + *end as usize
                }
                _ => story..story,
            };
            StoredComment {
                reference: *reference as usize,
                text,
                author,
                initials: Some(initials).filter(|initials| !initials.is_empty()),
                covers: tag
                    .filter(|tag| *tag >= 0)
                    .and_then(|tag| ranges.iter().find(|(known, _)| *known == tag))
                    .map(|(_, range)| range.clone())
                    .filter(|range| !range.is_empty()),
            }
        })
        .collect()
}

/// The range each comment bookmark covers, by the tag its `ATNBE` gives it.
fn bookmark_ranges(tables: &CommentTables<'_>) -> Vec<(i32, Range<usize>)> {
    let Some(tags) = bookmark_tags(tables.bookmark_names) else {
        return Vec::new();
    };
    let Some((firsts, links)) = plex(tables.bookmark_firsts, BOOKMARK_FIRST_SIZE) else {
        return Vec::new();
    };
    let lasts = positions(tables.bookmark_lasts);
    tags.iter()
        .zip(firsts.iter().zip(&links))
        .filter_map(|(tag, (first, link))| {
            let last = lasts.get(usize::from(u16_at(link, 0)?))?;
            Some((*tag, *first as usize..*last as usize))
        })
        .collect()
}

/// The `lTag` of each entry of `SttbfAtnBkmk`, a string table whose strings are empty and
/// whose extra data is an `ATNBE`.
fn bookmark_tags(table: &[u8]) -> Option<Vec<i32>> {
    let extended = u16_at(table, 0)? == 0xFFFF;
    let header = if extended { 6 } else { 4 };
    let count = usize::from(u16_at(table, if extended { 2 } else { 0 })?).min(MAX_COMMENTS);
    let extra = usize::from(u16_at(table, if extended { 4 } else { 2 })?);
    let mut offset = header;
    let mut tags = Vec::with_capacity(count);
    for _ in 0..count {
        let length = if extended {
            usize::from(u16_at(table, offset)?) * 2 + 2
        } else {
            usize::from(*table.get(offset)?) + 1
        };
        offset += length;
        if extra >= BOOKMARK_EXTRA_SIZE {
            tags.push(i32::from_le_bytes(u32_at(table, offset + 2)?.to_le_bytes()));
        }
        offset += extra;
    }
    Some(tags)
}

/// The strings of a sequence of `Xst`: a count of UTF-16 units, then the units.
fn strings(bytes: &[u8]) -> Vec<String> {
    let mut found = Vec::new();
    let mut offset = 0;
    while let Some(length) = u16_at(bytes, offset).map(usize::from) {
        let Some(units) = bytes.get(offset + 2..offset + 2 + length * 2) else {
            break;
        };
        found.push(
            char::decode_utf16(
                units
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|unit| u16::from_le_bytes(*unit)),
            )
            .map(|character| character.unwrap_or('\u{FFFD}'))
            .collect(),
        );
        offset += 2 + length * 2;
        if found.len() >= MAX_COMMENTS {
            break;
        }
    }
    found
}

/// The names of the reviewers, `SttbfRMark`: an extended string table of UTF-16 strings.
pub(super) fn reviewers(table: &[u8]) -> Vec<String> {
    if u16_at(table, 0) != Some(0xFFFF) {
        return Vec::new();
    }
    let (Some(count), Some(extra)) = (u16_at(table, 2), u16_at(table, 4)) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    let mut offset = 6;
    for _ in 0..usize::from(count).min(MAX_COMMENTS) {
        let Some(length) = u16_at(table, offset).map(usize::from) else {
            break;
        };
        let Some(units) = table.get(offset + 2..offset + 2 + length * 2) else {
            break;
        };
        names.push(
            char::decode_utf16(
                units
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|unit| u16::from_le_bytes(*unit)),
            )
            .map(|character| character.unwrap_or('\u{FFFD}'))
            .collect(),
        );
        offset += 2 + length * 2 + usize::from(extra);
    }
    names
}

/// A `DTTM` as ISO 8601: minutes, hours, day, month and years since 1900 packed in 32 bits;
/// `None` for zero, which stands for no date.
pub(super) fn date_of(dttm: u32) -> Option<String> {
    if dttm == 0 {
        return None;
    }
    let minute = dttm & 0x3F;
    let hour = (dttm >> 6) & 0x1F;
    let day = (dttm >> 11) & 0x1F;
    let month = (dttm >> 16) & 0xF;
    let year = ((dttm >> 20) & 0x1FF) + 1_900;
    ((1..=12).contains(&month) && (1..=31).contains(&day) && hour < 24 && minute < 60)
        .then(|| format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:00Z"))
}

#[cfg(test)]
mod tests;
