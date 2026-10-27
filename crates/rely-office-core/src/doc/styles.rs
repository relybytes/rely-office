// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The style sheet ([MS-DOC] 2.9.271, `STSH`): each style's name, kind, base style and the
//! paragraph and character modifiers it applies.

use crate::compound::u16_at;

/// A style with no base names this index.
const NO_STYLE: u16 = 0x0FFF;
/// No chain of base styles is longer than this.
const MAX_CHAIN: usize = 16;

/// What a style formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StyleKind {
    Paragraph,
    Character,
    Table,
    List,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Style {
    pub(super) name: String,
    /// The built-in style this is: 0 the normal style, 1 to 9 the headings.
    pub(super) built_in: u16,
    pub(super) kind: StyleKind,
    pub(super) base: Option<usize>,
    pub(super) paragraph: Vec<u8>,
    pub(super) character: Vec<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct StyleSheet {
    pub(super) styles: Vec<Option<Style>>,
    /// The font of text no style names one for, by its index in the font table.
    pub(super) default_font: Option<u16>,
}

impl StyleSheet {
    pub(super) fn read(bytes: &[u8]) -> Self {
        let Some(header_length) = u16_at(bytes, 0).map(usize::from) else {
            return Self::default();
        };
        let header = bytes.get(2..2 + header_length).unwrap_or_default();
        let count = u16_at(header, 0).unwrap_or(0);
        let base_length = usize::from(u16_at(header, 2).unwrap_or(10));
        let default_font = u16_at(header, 12);
        let mut styles = Vec::with_capacity(usize::from(count));
        let mut position = 2 + header_length;
        for _ in 0..count {
            let Some(length) = u16_at(bytes, position).map(usize::from) else {
                break;
            };
            let start = position + 2;
            position = start + length;
            styles.push(
                bytes
                    .get(start..start + length)
                    .filter(|definition| !definition.is_empty())
                    .and_then(|definition| read_style(definition, base_length)),
            );
        }
        Self {
            styles,
            default_font,
        }
    }

    pub(super) fn style(&self, index: usize) -> Option<&Style> {
        self.styles.get(index)?.as_ref()
    }

    /// A style and its bases, the farthest base first.
    pub(super) fn chain(&self, index: usize) -> Vec<&Style> {
        let mut chain = Vec::new();
        let mut next = Some(index);
        while let Some(index) = next {
            if chain.len() >= MAX_CHAIN {
                break;
            }
            let Some(style) = self.style(index) else {
                break;
            };
            chain.push(style);
            next = style.base.filter(|base| *base != index);
        }
        chain.reverse();
        chain
    }
}

/// An `STD`: the fixed part, the name and the property exceptions the kind carries.
fn read_style(definition: &[u8], base_length: usize) -> Option<Style> {
    let built_in = u16_at(definition, 0)? & 0x0FFF;
    let second = u16_at(definition, 2)?;
    let kind = match second & 0x000F {
        1 => StyleKind::Paragraph,
        2 => StyleKind::Character,
        3 => StyleKind::Table,
        _ => StyleKind::List,
    };
    let base = second >> 4;
    let mut position = base_length;
    let name_length = usize::from(u16_at(definition, position)?);
    let units: Vec<u16> = definition
        .get(position + 2..position + 2 + name_length * 2)?
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .collect();
    let name = String::from_utf16_lossy(&units);
    position += 2 + name_length * 2 + 2;
    let exception_count = match kind {
        StyleKind::Paragraph => 2,
        StyleKind::Character | StyleKind::List => 1,
        StyleKind::Table => 3,
    };
    let mut exceptions: Vec<&[u8]> = Vec::new();
    for _ in 0..exception_count {
        let Some(length) = u16_at(definition, position).map(usize::from) else {
            break;
        };
        let Some(exception) = definition.get(position + 2..position + 2 + length) else {
            break;
        };
        exceptions.push(exception);
        position += 2 + length + length % 2;
    }
    let paragraph_exception = |exception: Option<&&[u8]>| {
        exception
            .and_then(|exception| exception.get(2..))
            .unwrap_or_default()
            .to_vec()
    };
    let (paragraph, character) = match kind {
        StyleKind::Paragraph => (
            paragraph_exception(exceptions.first()),
            exceptions
                .get(1)
                .map(|exception| exception.to_vec())
                .unwrap_or_default(),
        ),
        StyleKind::Character => (
            Vec::new(),
            exceptions
                .first()
                .map(|exception| exception.to_vec())
                .unwrap_or_default(),
        ),
        StyleKind::Table => (
            paragraph_exception(exceptions.get(1)),
            exceptions
                .get(2)
                .map(|exception| exception.to_vec())
                .unwrap_or_default(),
        ),
        StyleKind::List => (paragraph_exception(exceptions.first()), Vec::new()),
    };
    Some(Style {
        name,
        built_in,
        kind,
        base: (base != NO_STYLE).then_some(usize::from(base)),
        paragraph,
        character,
    })
}

#[cfg(test)]
mod tests;
