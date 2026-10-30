// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! `PowerPoint` 97-2003 presentations built for the tests: the persist objects one after the
//! other, a persist directory and a user edit naming them, and the `Current User` stream.

use crate::test_support::{build_compound_file, ppt_record as record};

/// A presentation under construction: its persist objects by identifier, in stream order.
#[derive(Debug, Default)]
pub(super) struct PptBuilder {
    pub(super) objects: Vec<(u32, Vec<u8>)>,
    pub(super) document: u32,
    pub(super) pictures: Vec<u8>,
    pub(super) encrypted_token: bool,
    pub(super) without_current_user: bool,
    /// Bytes before the first object, so that offsets are not all zero.
    pub(super) prefix: Vec<u8>,
}

impl PptBuilder {
    pub(super) fn object(mut self, identifier: u32, record: Vec<u8>) -> Self {
        self.objects.push((identifier, record));
        self
    }

    pub(super) fn build(&self) -> Vec<u8> {
        let mut stream = self.prefix.clone();
        let mut directory = Vec::new();
        for (identifier, bytes) in &self.objects {
            directory.extend_from_slice(&(identifier | (1 << 20)).to_le_bytes());
            directory.extend_from_slice(&u32::try_from(stream.len()).unwrap().to_le_bytes());
            stream.extend_from_slice(bytes);
        }
        let directory_offset = u32::try_from(stream.len()).unwrap();
        stream.extend(record(0, 0, 0x1772, &directory));
        let edit_offset = u32::try_from(stream.len()).unwrap();
        let mut edit = vec![0_u8; 28];
        edit[12..16].copy_from_slice(&directory_offset.to_le_bytes());
        edit[16..20].copy_from_slice(&self.document.to_le_bytes());
        stream.extend(record(0, 0, 0x0FF5, &edit));
        let mut user = vec![0_u8; 20];
        user[0..4].copy_from_slice(&0x14_u32.to_le_bytes());
        let token: u32 = if self.encrypted_token {
            0xF3D1_C4DF
        } else {
            0xE391_C05F
        };
        user[4..8].copy_from_slice(&token.to_le_bytes());
        user[8..12].copy_from_slice(&edit_offset.to_le_bytes());
        let current_user = record(0, 0, 0x0FF6, &user);
        let mut streams: Vec<(&str, &[u8])> = vec![
            ("/PowerPoint Document", &stream),
            ("/Pictures", &self.pictures),
        ];
        if !self.without_current_user {
            streams.push(("/Current User", &current_user));
        }
        build_compound_file(&streams)
    }
}

/// A container record.
pub(super) fn container(kind: u16, instance: u16, children: &[Vec<u8>]) -> Vec<u8> {
    record(0xF, instance, kind, &children.concat())
}

/// An atom record.
pub(super) fn atom(kind: u16, instance: u16, body: &[u8]) -> Vec<u8> {
    record(0, instance, kind, body)
}

/// A UTF-16 `TextCharsAtom`.
pub(super) fn text_chars(text: &str) -> Vec<u8> {
    let body: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
    atom(0x0FA0, 0, &body)
}

/// A property table of simple values and complex data.
pub(super) fn property_table(
    kind: u16,
    simple: &[(u16, u32)],
    complex: &[(u16, Vec<u8>)],
) -> Vec<u8> {
    let mut entries = Vec::new();
    let mut data = Vec::new();
    for (identifier, value) in simple {
        entries.extend_from_slice(&identifier.to_le_bytes());
        entries.extend_from_slice(&value.to_le_bytes());
    }
    for (identifier, bytes) in complex {
        entries.extend_from_slice(&(identifier | 0x8000).to_le_bytes());
        // An array's length leaves out its six-byte header.
        let length = bytes.len().saturating_sub(6);
        entries.extend_from_slice(&u32::try_from(length).unwrap().to_le_bytes());
        data.extend_from_slice(bytes);
    }
    entries.extend(data);
    record(
        3,
        u16::try_from(simple.len() + complex.len()).unwrap(),
        kind,
        &entries,
    )
}

/// An `IMsoArray` of elements of `size` bytes.
pub(super) fn array(size: u16, elements: &[Vec<u8>]) -> Vec<u8> {
    let count = u16::try_from(elements.len()).unwrap();
    let mut bytes = count.to_le_bytes().to_vec();
    bytes.extend_from_slice(&count.to_le_bytes());
    bytes.extend_from_slice(&size.to_le_bytes());
    for element in elements {
        bytes.extend_from_slice(element);
    }
    bytes
}

/// A shape: its type, `FSP` flags, a slide anchor of top, left, right and bottom, and the
/// records it holds after them.
pub(super) fn shape(kind: u16, flags: u32, anchor: Option<[i16; 4]>, rest: &[Vec<u8>]) -> Vec<u8> {
    let mut children = vec![atom_versioned(
        2,
        kind,
        0xF00A,
        &[1_u32.to_le_bytes(), flags.to_le_bytes()].concat(),
    )];
    if let Some(anchor) = anchor {
        let body: Vec<u8> = anchor
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        children.push(atom(0xF010, 0, &body));
    }
    children.extend_from_slice(rest);
    container(0xF004, 0, &children)
}

/// An atom of a version other than zero.
pub(super) fn atom_versioned(version: u16, instance: u16, kind: u16, body: &[u8]) -> Vec<u8> {
    record(version, instance, kind, body)
}

/// A paragraph exception of these masks and fields.
pub(super) fn exception(masks: u32, fields: &[u8]) -> Vec<u8> {
    let mut bytes = masks.to_le_bytes().to_vec();
    bytes.extend_from_slice(fields);
    bytes
}
