// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! Where each object of a presentation lies ([MS-PPT] 2.3): the `Current User` stream names the
//! last edit, each edit names the one before and its persist directory, and a later edit's
//! offsets win over an earlier one's.

use std::collections::HashMap;

use crate::compound::u32_at;
use crate::office_art::records;

const USER_EDIT: u16 = 0x0FF5;
const PERSIST_DIRECTORY: u16 = 0x1772;
/// The header token of a `CurrentUserAtom` when the presentation is encrypted.
const ENCRYPTED_TOKEN: u32 = 0xF3D1_C4DF;
/// No presentation has been saved incrementally more times than this.
const MAX_EDITS: usize = 256;
/// A persist directory entry names at most this many objects.
const MAX_ENTRIES: usize = 0x0FFF;

/// The offsets of a presentation's objects and the one that is its document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Persist {
    pub(super) offsets: HashMap<u32, usize>,
    pub(super) document: u32,
    pub(super) encrypted: bool,
}

impl Persist {
    /// The bytes of the record an object's offset points at, header included.
    pub(super) fn object<'s>(&self, stream: &'s [u8], identifier: u32) -> Option<&'s [u8]> {
        stream.get(*self.offsets.get(&identifier)?..)
    }
}

/// The persist directories of every edit, the latest first.
pub(super) fn read_persist(current_user: &[u8], stream: &[u8]) -> Option<Persist> {
    let header = records(current_user).next()?;
    let token = u32_at(header.body, 4)?;
    let mut persist = Persist {
        encrypted: token == ENCRYPTED_TOKEN,
        ..Persist::default()
    };
    let mut offset = u32_at(header.body, 8)? as usize;
    let mut visited = Vec::new();
    for index in 0..MAX_EDITS {
        if visited.contains(&offset) {
            break;
        }
        visited.push(offset);
        let edit = records(stream.get(offset..)?)
            .next()
            .filter(|edit| edit.kind == USER_EDIT)?;
        if index == 0 {
            persist.document = u32_at(edit.body, 16)?;
        }
        let directory_offset = u32_at(edit.body, 12)? as usize;
        if let Some(directory) = stream
            .get(directory_offset..)
            .and_then(|bytes| records(bytes).next())
            .filter(|directory| directory.kind == PERSIST_DIRECTORY)
        {
            read_directory(directory.body, &mut persist.offsets);
        }
        let previous = u32_at(edit.body, 8)? as usize;
        if previous == 0 {
            break;
        }
        offset = previous;
    }
    Some(persist)
}

/// One persist directory's entries, each a run of identifiers and their offsets, kept only
/// where a later edit has not set them.
fn read_directory(body: &[u8], offsets: &mut HashMap<u32, usize>) {
    let mut position = 0;
    while let Some(entry) = u32_at(body, position) {
        let first = entry & 0x000F_FFFF;
        let count = ((entry >> 20) as usize).min(MAX_ENTRIES);
        position += 4;
        for index in 0..count {
            let Some(offset) = u32_at(body, position) else {
                return;
            };
            position += 4;
            let identifier = first + u32::try_from(index).unwrap_or(0);
            offsets.entry(identifier).or_insert(offset as usize);
        }
    }
}

#[cfg(test)]
mod tests;
