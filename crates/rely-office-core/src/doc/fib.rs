// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! The File Information Block of a Word 97-2003 document ([MS-DOC] 2.5.1): the length of each
//! story and where each structure lies in the table stream.

use super::unreadable;
use crate::OfficeError;
use crate::compound::{u16_at, u32_at};

const FIB_IDENTIFIER: u16 = 0xA5EC;
/// The first `nFib` of Word 97; earlier versions (Word 6 and 95) store text another way.
const FIRST_WORD_97_FIB: u16 = 0x00C0;
const FLAG_ENCRYPTED: u16 = 0x0100;
const FLAG_TABLE_ONE: u16 = 0x0200;
/// The `ccp*` counts of `FibRgLw97`, in story order: main text, footnotes, headers, comments,
/// endnotes, text boxes and header text boxes.
const STORY_COUNT_INDEXES: [usize; 7] = [3, 4, 5, 7, 8, 9, 10];

/// The pairs of `FibRgFcLcb97` this reader uses, by their index.
pub(super) mod pair {
    pub(in super::super) const STYLES: usize = 1;
    pub(in super::super) const FOOTNOTE_REFERENCES: usize = 2;
    pub(in super::super) const FOOTNOTE_TEXTS: usize = 3;
    pub(in super::super) const COMMENT_REFERENCES: usize = 4;
    pub(in super::super) const COMMENT_TEXTS: usize = 5;
    pub(in super::super) const COMMENT_OWNERS: usize = 36;
    pub(in super::super) const COMMENT_BOOKMARK_NAMES: usize = 37;
    pub(in super::super) const COMMENT_BOOKMARK_FIRSTS: usize = 42;
    pub(in super::super) const COMMENT_BOOKMARK_LASTS: usize = 43;
    pub(in super::super) const REVIEWERS: usize = 51;
    pub(in super::super) const SECTIONS: usize = 6;
    pub(in super::super) const HEADERS: usize = 11;
    pub(in super::super) const CHARACTER_PAGES: usize = 12;
    pub(in super::super) const PARAGRAPH_PAGES: usize = 13;
    pub(in super::super) const FONTS: usize = 15;
    pub(in super::super) const DOCUMENT_PROPERTIES: usize = 31;
    pub(in super::super) const PIECE_TABLE: usize = 33;
    pub(in super::super) const MAIN_SHAPES: usize = 40;
    pub(in super::super) const ENDNOTE_REFERENCES: usize = 46;
    pub(in super::super) const ENDNOTE_TEXTS: usize = 47;
    pub(in super::super) const DRAWINGS: usize = 50;
    pub(in super::super) const TEXT_BOXES: usize = 56;
    pub(in super::super) const EMBEDDED_FONTS: usize = 61;
    pub(in super::super) const LISTS: usize = 73;
    pub(in super::super) const LIST_OVERRIDES: usize = 74;
}

/// How long each story is, in characters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Stories {
    pub(super) main: usize,
    pub(super) footnotes: usize,
    pub(super) headers: usize,
    pub(super) comments: usize,
    pub(super) endnotes: usize,
    pub(super) text_boxes: usize,
    pub(super) header_text_boxes: usize,
}

impl Stories {
    /// Every story's characters together.
    pub(super) fn total(&self) -> usize {
        [
            self.main,
            self.footnotes,
            self.headers,
            self.comments,
            self.endnotes,
            self.text_boxes,
            self.header_text_boxes,
        ]
        .into_iter()
        .fold(0, usize::saturating_add)
    }

    pub(super) fn footnotes_start(&self) -> usize {
        self.main
    }

    pub(super) fn headers_start(&self) -> usize {
        self.main.saturating_add(self.footnotes)
    }

    pub(super) fn comments_start(&self) -> usize {
        self.headers_start().saturating_add(self.headers)
    }

    pub(super) fn endnotes_start(&self) -> usize {
        self.headers_start()
            .saturating_add(self.headers)
            .saturating_add(self.comments)
    }

    pub(super) fn text_boxes_start(&self) -> usize {
        self.endnotes_start().saturating_add(self.endnotes)
    }
}

/// What the File Information Block says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Fib {
    pub(super) encrypted: bool,
    pub(super) table_stream: &'static str,
    pub(super) stories: Stories,
    /// The offset and length of each structure in the table stream.
    pairs: Vec<(usize, usize)>,
}

impl Fib {
    pub(super) fn read(word_document: &[u8]) -> Result<Self, OfficeError> {
        let short = || unreadable("the File Information Block is cut short");
        if u16_at(word_document, 0) != Some(FIB_IDENTIFIER) {
            return Err(unreadable(
                "the WordDocument stream is not a Word 97-2003 document",
            ));
        }
        let fib_version = u16_at(word_document, 2).ok_or_else(short)?;
        if fib_version < FIRST_WORD_97_FIB {
            return Err(unreadable("documents older than Word 97 are not read"));
        }
        let flags = u16_at(word_document, 10).ok_or_else(short)?;

        let short_count = usize::from(u16_at(word_document, 32).ok_or_else(short)?);
        let long_count_offset = 34 + short_count * 2;
        let long_count = usize::from(u16_at(word_document, long_count_offset).ok_or_else(short)?);
        let longs_offset = long_count_offset + 2;
        let read_long = |index: usize| -> Option<usize> {
            if index >= long_count {
                return None;
            }
            u32_at(word_document, longs_offset + index * 4).map(|value| value as usize)
        };
        let counts = STORY_COUNT_INDEXES.map(|index| read_long(index).unwrap_or(0));
        read_long(STORY_COUNT_INDEXES[0]).ok_or_else(short)?;
        let stories = Stories {
            main: counts[0],
            footnotes: counts[1],
            headers: counts[2],
            comments: counts[3],
            endnotes: counts[4],
            text_boxes: counts[5],
            header_text_boxes: counts[6],
        };

        let pair_count_offset = longs_offset + long_count * 4;
        let pair_count = usize::from(u16_at(word_document, pair_count_offset).ok_or_else(short)?);
        if pair_count <= pair::PIECE_TABLE {
            return Err(short());
        }
        let mut pairs = Vec::with_capacity(pair_count);
        for index in 0..pair_count {
            let at = pair_count_offset + 2 + index * 8;
            let (Some(offset), Some(length)) =
                (u32_at(word_document, at), u32_at(word_document, at + 4))
            else {
                if index <= pair::PIECE_TABLE {
                    return Err(short());
                }
                break;
            };
            pairs.push((offset as usize, length as usize));
        }
        Ok(Self {
            encrypted: flags & FLAG_ENCRYPTED != 0,
            table_stream: if flags & FLAG_TABLE_ONE == 0 {
                "/0Table"
            } else {
                "/1Table"
            },
            stories,
            pairs,
        })
    }

    /// The offset and length of a structure; `None` when the document does not have it.
    pub(super) fn pair(&self, index: usize) -> Option<(usize, usize)> {
        self.pairs
            .get(index)
            .copied()
            .filter(|(_, length)| *length > 0)
    }

    /// The bytes of a structure in the table stream; `None` when it is absent or lies outside.
    pub(super) fn slice<'t>(&self, table: &'t [u8], index: usize) -> Option<&'t [u8]> {
        let (offset, length) = self.pair(index)?;
        table.get(offset..offset.checked_add(length)?)
    }
}

#[cfg(test)]
mod tests;
