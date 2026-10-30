// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! Sections ([MS-DOC] 2.8.26 and 2.9.234): where each section ends and the page it lays out
//! on, from the modifiers each one keeps in the `WordDocument` stream.

use super::fkp::plex;
use super::sprm::{byte, modifiers, signed_word, word};
use crate::compound::{u16_at, u32_at};
use crate::document::PageLayout;

/// A section descriptor (`Sed`) is twelve bytes; its `fcSepx` sits at offset 2.
const DESCRIPTOR_SIZE: usize = 12;
/// A section with no modifiers of its own has this `fcSepx`.
const NO_MODIFIERS: u32 = 0xFFFF_FFFF;

/// A section: the character position it ends at, its page, and how it begins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SectionLayout {
    pub(super) end: usize,
    pub(super) layout: PageLayout,
    /// It goes on on the page the section before it ends on.
    pub(super) continuous: bool,
    /// Its first page has headers and footers of its own.
    pub(super) title_page: bool,
}

/// The page Word 97 lays out when a section names nothing: Letter with 1.25 and 1 inch margins.
fn default_layout() -> PageLayout {
    PageLayout {
        width_twips: 12_240,
        height_twips: 15_840,
        margins_twips: [1_440, 1_800, 1_440, 1_800],
        header_distance_twips: 720,
        footer_distance_twips: 720,
        columns: 1,
        column_gap_twips: 720,
        ..PageLayout::default()
    }
}

/// The sections of the main text, in order.
pub(super) fn read_sections(word_document: &[u8], plex_bytes: &[u8]) -> Vec<SectionLayout> {
    let Some((positions, descriptors)) = plex(plex_bytes, DESCRIPTOR_SIZE) else {
        return Vec::new();
    };
    descriptors
        .iter()
        .enumerate()
        .map(|(index, descriptor)| {
            let mut section = SectionLayout {
                end: positions[index + 1] as usize,
                layout: default_layout(),
                continuous: false,
                title_page: false,
            };
            let offset = u32_at(descriptor, 2).unwrap_or(NO_MODIFIERS);
            if offset != NO_MODIFIERS {
                let offset = offset as usize;
                let length = usize::from(u16_at(word_document, offset).unwrap_or(0));
                if let Some(grpprl) = word_document.get(offset + 2..offset + 2 + length) {
                    apply(&mut section, grpprl);
                }
            }
            section
        })
        .collect()
}

fn apply(section: &mut SectionLayout, grpprl: &[u8]) {
    let twips = |operand: &[u8]| u32::from(word(operand));
    let signed = |operand: &[u8]| u32::from(signed_word(operand).unsigned_abs());
    let layout = &mut section.layout;
    for (code, operand) in modifiers(grpprl) {
        match code {
            0x3009 => section.continuous = byte(operand) == 0,
            0x300A => section.title_page = byte(operand) != 0,
            0x500B => layout.columns = u32::from(word(operand)).saturating_add(1).min(45),
            0x900C => layout.column_gap_twips = twips(operand),
            0xB017 => layout.header_distance_twips = twips(operand),
            0xB018 => layout.footer_distance_twips = twips(operand),
            0xB01F => layout.width_twips = twips(operand).max(1_440),
            0xB020 => layout.height_twips = twips(operand).max(1_440),
            0xB021 => layout.margins_twips[3] = twips(operand),
            0xB022 => layout.margins_twips[1] = twips(operand),
            0x9023 => layout.margins_twips[0] = signed(operand),
            0x9024 => layout.margins_twips[2] = signed(operand),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
