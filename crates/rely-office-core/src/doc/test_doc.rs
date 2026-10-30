// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! Word 97 documents built for the tests: the File Information Block, one uncompressed piece,
//! the formatted disk pages and whatever table structures a test adds.

use std::collections::HashMap;
use std::ops::Range;

use super::fib::pair;
use crate::test_support::build_compound_file;

/// Where the text begins in the `WordDocument` stream, and where the section modifiers and
/// the delayed pictures lie.
pub(super) const TEXT_OFFSET: usize = 1_024;
pub(super) const SECTION_OFFSET: usize = 0x6000;
pub(super) const DELAY_OFFSET: usize = 0x7000;
const PAGE_SIZE: usize = 512;
/// No page holds more runs than this, so every run's properties fit beside them.
const RUNS_PER_PAGE: usize = 12;

/// A paragraph run of a page: its start and end in the stream, its style and its modifiers.
pub(super) type ParagraphRunSpec = (u32, u32, u16, Vec<u8>);

/// An `STD` of a ten-byte base: the built-in index, the kind, the base, the name and the
/// property exceptions, each padded to an even length.
pub(super) fn definition(
    built_in: u16,
    kind: u16,
    base: u16,
    name: &str,
    exceptions: &[&[u8]],
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&built_in.to_le_bytes());
    bytes.extend_from_slice(&(kind | (base << 4)).to_le_bytes());
    bytes.extend_from_slice(&[0; 6]);
    let units: Vec<u16> = name.encode_utf16().collect();
    bytes.extend_from_slice(&u16::try_from(units.len()).unwrap().to_le_bytes());
    for unit in units {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes.extend_from_slice(&[0, 0]);
    for exception in exceptions {
        bytes.extend_from_slice(&u16::try_from(exception.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(exception);
        if exception.len() % 2 == 1 {
            bytes.push(0);
        }
    }
    bytes
}

/// A style sheet of these definitions, an empty slot for each `None`, and a default font.
pub(super) fn sheet(definitions: &[Option<Vec<u8>>], default_font: u16) -> Vec<u8> {
    let mut header = vec![0_u8; 18];
    header[0..2].copy_from_slice(&u16::try_from(definitions.len()).unwrap().to_le_bytes());
    header[2..4].copy_from_slice(&10_u16.to_le_bytes());
    header[12..14].copy_from_slice(&default_font.to_le_bytes());
    let mut bytes = u16::try_from(header.len()).unwrap().to_le_bytes().to_vec();
    bytes.extend_from_slice(&header);
    for definition in definitions {
        let definition = definition.clone().unwrap_or_default();
        bytes.extend_from_slice(&u16::try_from(definition.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&definition);
    }
    bytes
}

/// An `FFN` naming a font.
pub(super) fn font(name: &str) -> Vec<u8> {
    let mut data = vec![0_u8; 39];
    for unit in name.encode_utf16() {
        data.extend_from_slice(&unit.to_le_bytes());
    }
    data.extend_from_slice(&[0, 0]);
    let mut entry = vec![u8::try_from(data.len()).unwrap()];
    entry.extend_from_slice(&data);
    entry
}

/// A plex of positions and elements.
/// An Embedded `OpenType` structure of version 2.1 around a font file, its names in UTF-16 and
/// the data hidden as `flags` say.
pub(super) fn eot(font: &[u8], family: &str, flags: u32) -> Vec<u8> {
    let mut data = font.to_vec();
    if flags & 0x1000_0000 != 0 {
        for byte in &mut data {
            *byte ^= 0x50;
        }
    }
    let mut header = vec![0_u8; 80];
    header[8..12].copy_from_slice(&0x0002_0001_u32.to_le_bytes());
    header[12..16].copy_from_slice(&flags.to_le_bytes());
    header[34..36].copy_from_slice(&0x504C_u16.to_le_bytes());
    let utf16 =
        |text: &str| -> Vec<u8> { text.encode_utf16().flat_map(u16::to_le_bytes).collect() };
    for name in [
        utf16(family),
        utf16("Regular"),
        utf16("Version 1.0"),
        utf16(family),
        Vec::new(),
    ] {
        header.extend_from_slice(&[0, 0]);
        header.extend_from_slice(&u16::try_from(name.len()).unwrap().to_le_bytes());
        header.extend_from_slice(&name);
    }
    let size = header.len() + data.len();
    header[0..4].copy_from_slice(&u32::try_from(size).unwrap().to_le_bytes());
    header[4..8].copy_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
    header.extend_from_slice(&data);
    header
}

/// `SttbTtmbd`: each face's offset in the `WordDocument` stream, its font's index and whether it
/// is bold and italic.
pub(super) fn embedded_font_table(faces: &[(usize, u16, bool, bool)]) -> Vec<u8> {
    let mut table = vec![0_u8; 10];
    table[2..4].copy_from_slice(&u16::try_from(faces.len()).unwrap().to_le_bytes());
    table[4..6].copy_from_slice(&64_u16.to_le_bytes());
    table[8..10].copy_from_slice(&10_u16.to_le_bytes());
    for (offset, font, bold, italic) in faces {
        table.extend_from_slice(&u32::try_from(*offset).unwrap().to_le_bytes());
        table.extend_from_slice(&font.to_le_bytes());
        table.extend_from_slice(&(u16::from(*bold) | u16::from(*italic) << 1).to_le_bytes());
        table.extend_from_slice(&u32::MAX.to_le_bytes());
    }
    table
}

pub(super) fn plex(positions: &[u32], elements: &[&[u8]]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for position in positions {
        bytes.extend_from_slice(&position.to_le_bytes());
    }
    for element in elements {
        bytes.extend_from_slice(element);
    }
    bytes
}

/// An `OfficeArt` record.
pub(super) fn record(version: u16, instance: u16, kind: u16, body: &[u8]) -> Vec<u8> {
    let mut bytes = (version | (instance << 4)).to_le_bytes().to_vec();
    bytes.extend_from_slice(&kind.to_le_bytes());
    bytes.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(body);
    bytes
}

/// A PNG blip record of one identifier and its tag byte.
pub(super) fn png_blip(image: &[u8]) -> Vec<u8> {
    let mut body = vec![0_u8; 17];
    body.extend_from_slice(image);
    record(0, 0x6E0, 0xF01E, &body)
}

/// A store entry (`FBSE`) that embeds a blip, or names `size` bytes at `delay` without one.
pub(super) fn store_entry(blip: Option<&[u8]>, size: u32, delay: u32) -> Vec<u8> {
    let mut body = vec![0_u8; 36];
    body[20..24].copy_from_slice(&size.to_le_bytes());
    body[28..32].copy_from_slice(&delay.to_le_bytes());
    if let Some(blip) = blip {
        body.extend_from_slice(blip);
    }
    record(2, 6, 0xF007, &body)
}

/// A Word 97 document under construction.
#[derive(Debug, Default)]
pub(super) struct DocBuilder {
    pub(super) main: String,
    pub(super) footnotes: String,
    pub(super) headers: String,
    pub(super) comments: String,
    pub(super) endnotes: String,
    pub(super) text_boxes: String,
    /// Character modifiers over ranges of character positions; the rest have none.
    pub(super) characters: Vec<(Range<usize>, Vec<u8>)>,
    /// The style and modifiers of the paragraph whose mark is at a position.
    pub(super) paragraphs: HashMap<usize, (u16, Vec<u8>)>,
    pub(super) styles: Vec<u8>,
    pub(super) fonts: Vec<&'static str>,
    /// Each section's end and its modifiers, kept at [`SECTION_OFFSET`].
    pub(super) sections: Vec<(u32, Vec<u8>)>,
    /// Other structures of the table stream, by their pair.
    pub(super) structures: Vec<(usize, Vec<u8>)>,
    pub(super) data: Vec<u8>,
    /// Bytes at [`DELAY_OFFSET`] of the `WordDocument` stream.
    pub(super) delay: Vec<u8>,
    pub(super) encrypted: bool,
}

impl DocBuilder {
    pub(super) fn new(main: &str) -> Self {
        Self {
            main: main.to_owned(),
            ..Self::default()
        }
    }

    pub(super) fn character(mut self, range: Range<usize>, grpprl: &[u8]) -> Self {
        self.characters.push((range, grpprl.to_vec()));
        self
    }

    pub(super) fn paragraph(mut self, mark: usize, style: u16, grpprl: &[u8]) -> Self {
        self.paragraphs.insert(mark, (style, grpprl.to_vec()));
        self
    }

    pub(super) fn structure(mut self, index: usize, bytes: Vec<u8>) -> Self {
        self.structures.push((index, bytes));
        self
    }

    fn text(&self) -> String {
        [
            &self.main,
            &self.footnotes,
            &self.headers,
            &self.comments,
            &self.endnotes,
            &self.text_boxes,
        ]
        .into_iter()
        .map(String::as_str)
        .collect()
    }

    fn offset(position: usize) -> u32 {
        u32::try_from(TEXT_OFFSET + position * 2).unwrap()
    }

    /// The character runs, every position covered, in pages.
    fn character_pages(&self, total: usize) -> Vec<Vec<(u32, u32, Vec<u8>)>> {
        let mut runs: Vec<(usize, usize, Vec<u8>)> = Vec::new();
        let mut ranges = self.characters.clone();
        ranges.sort_by_key(|(range, _)| range.start);
        let mut position = 0;
        for (range, grpprl) in ranges {
            if range.start > position {
                runs.push((position, range.start, Vec::new()));
            }
            runs.push((range.start, range.end, grpprl));
            position = range.end;
        }
        if position < total {
            runs.push((position, total, Vec::new()));
        }
        runs.chunks(RUNS_PER_PAGE)
            .map(|chunk| {
                chunk
                    .iter()
                    .map(|(start, end, grpprl)| {
                        (Self::offset(*start), Self::offset(*end), grpprl.clone())
                    })
                    .collect()
            })
            .collect()
    }

    /// The paragraph runs, one for each mark, in pages.
    fn paragraph_pages(&self, text: &[char]) -> Vec<Vec<ParagraphRunSpec>> {
        let mut runs = Vec::new();
        let mut start = 0;
        for (position, character) in text.iter().enumerate() {
            if matches!(character, '\r' | '\u{07}' | '\u{0C}') || position + 1 == text.len() {
                let (style, grpprl) = self.paragraphs.get(&position).cloned().unwrap_or_default();
                runs.push((
                    Self::offset(start),
                    Self::offset(position + 1),
                    style,
                    grpprl,
                ));
                start = position + 1;
            }
        }
        runs.chunks(RUNS_PER_PAGE).map(<[_]>::to_vec).collect()
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the streams are laid out in the order the offsets need"
    )]
    pub(super) fn build(&self) -> Vec<u8> {
        let text: Vec<char> = self.text().chars().collect();
        let total = text.len();
        let mut word_document = vec![0_u8; TEXT_OFFSET];
        put_u16(&mut word_document, 0, 0xA5EC);
        put_u16(&mut word_document, 2, 0x00C1);
        put_u16(
            &mut word_document,
            10,
            0x0200 | if self.encrypted { 0x0100 } else { 0 },
        );
        put_u16(&mut word_document, 32, 14);
        put_u16(&mut word_document, 62, 22);
        let lengths = [
            (3, self.main.chars().count()),
            (4, self.footnotes.chars().count()),
            (5, self.headers.chars().count()),
            (7, self.comments.chars().count()),
            (8, self.endnotes.chars().count()),
            (9, self.text_boxes.chars().count()),
        ];
        for (index, length) in lengths {
            put_u32(&mut word_document, 64 + index * 4, length);
        }
        put_u16(&mut word_document, 152, 93);
        for unit in text.iter().collect::<String>().encode_utf16() {
            word_document.extend_from_slice(&unit.to_le_bytes());
        }

        let mut pages = Vec::new();
        let character_pages = self.character_pages(total);
        let paragraph_pages = self.paragraph_pages(&text);
        word_document.resize(word_document.len().next_multiple_of(PAGE_SIZE), 0);
        let mut character_plex = Vec::new();
        for runs in &character_pages {
            pages.push(word_document.len() / PAGE_SIZE);
            word_document.extend_from_slice(&character_page(runs));
            character_plex.push(runs[0].0);
        }
        let mut paragraph_plex = Vec::new();
        for runs in &paragraph_pages {
            pages.push(word_document.len() / PAGE_SIZE);
            word_document.extend_from_slice(&paragraph_page(runs));
            paragraph_plex.push(runs[0].0);
        }
        assert!(
            word_document.len() <= SECTION_OFFSET,
            "the test document is too long"
        );
        word_document.resize(SECTION_OFFSET, 0);
        let mut section_offsets = Vec::new();
        for (_, grpprl) in &self.sections {
            section_offsets.push(word_document.len());
            word_document.extend_from_slice(&u16::try_from(grpprl.len()).unwrap().to_le_bytes());
            word_document.extend_from_slice(grpprl);
        }
        assert!(word_document.len() <= DELAY_OFFSET);
        word_document.resize(DELAY_OFFSET, 0);
        word_document.extend_from_slice(&self.delay);

        let mut table = vec![0_u8; 4];
        let mut pairs: Vec<(usize, usize, usize)> = Vec::new();
        let mut add = |table: &mut Vec<u8>, index: usize, bytes: &[u8]| {
            pairs.push((index, table.len(), bytes.len()));
            table.extend_from_slice(bytes);
        };
        let mut clx = vec![0x02];
        let pieces = plex(
            &[0, u32::try_from(total).unwrap()],
            &[&[0, 0, 0, 4, 0, 0, 0, 0]],
        );
        clx.extend_from_slice(&u32::try_from(pieces.len()).unwrap().to_le_bytes());
        clx.extend_from_slice(&pieces);
        add(&mut table, pair::PIECE_TABLE, &clx);
        let mut ends: Vec<u32> = character_pages
            .iter()
            .map(|runs| runs[runs.len() - 1].1)
            .collect();
        let page_numbers: Vec<[u8; 4]> = pages
            .iter()
            .map(|page| u32::try_from(*page).unwrap().to_le_bytes())
            .collect();
        let (character_numbers, paragraph_numbers) = page_numbers.split_at(character_pages.len());
        let mut positions = character_plex.clone();
        positions.push(ends.pop().unwrap_or(Self::offset(0)));
        add(
            &mut table,
            pair::CHARACTER_PAGES,
            &plex(
                &positions,
                &character_numbers
                    .iter()
                    .map(<[u8; 4]>::as_slice)
                    .collect::<Vec<_>>(),
            ),
        );
        let mut positions = paragraph_plex.clone();
        positions.push(
            paragraph_pages
                .last()
                .map_or(Self::offset(0), |runs| runs[runs.len() - 1].1),
        );
        add(
            &mut table,
            pair::PARAGRAPH_PAGES,
            &plex(
                &positions,
                &paragraph_numbers
                    .iter()
                    .map(<[u8; 4]>::as_slice)
                    .collect::<Vec<_>>(),
            ),
        );
        if !self.styles.is_empty() {
            add(&mut table, pair::STYLES, &self.styles);
        }
        if !self.fonts.is_empty() {
            let mut fonts = u16::try_from(self.fonts.len())
                .unwrap()
                .to_le_bytes()
                .to_vec();
            fonts.extend_from_slice(&[0, 0]);
            for name in &self.fonts {
                fonts.extend(font(name));
            }
            add(&mut table, pair::FONTS, &fonts);
        }
        if !self.sections.is_empty() {
            let mut positions = vec![0_u32];
            positions.extend(self.sections.iter().map(|(end, _)| *end));
            let descriptors: Vec<Vec<u8>> = section_offsets
                .iter()
                .map(|offset| {
                    let mut descriptor = vec![0_u8; 12];
                    descriptor[2..6]
                        .copy_from_slice(&u32::try_from(*offset).unwrap().to_le_bytes());
                    descriptor
                })
                .collect();
            add(
                &mut table,
                pair::SECTIONS,
                &plex(
                    &positions,
                    &descriptors.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                ),
            );
        }
        for (index, bytes) in &self.structures {
            add(&mut table, *index, bytes);
        }
        for (index, offset, length) in pairs {
            put_u32(&mut word_document, 154 + index * 8, offset);
            put_u32(&mut word_document, 158 + index * 8, length);
        }
        build_compound_file(&[
            ("/WordDocument", &word_document),
            ("/1Table", &table),
            ("/Data", &self.data),
        ])
    }
}

fn put_u16(buffer: &mut [u8], offset: usize, value: u16) {
    buffer[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(buffer: &mut [u8], offset: usize, value: usize) {
    buffer[offset..offset + 4].copy_from_slice(&u32::try_from(value).unwrap().to_le_bytes());
}

/// A character page with runs `[start, end)` and their modifiers.
pub(super) fn character_page(runs: &[(u32, u32, Vec<u8>)]) -> Vec<u8> {
    let mut page = vec![0_u8; PAGE_SIZE];
    let count = runs.len();
    for (index, (start, ..)) in runs.iter().enumerate() {
        page[index * 4..index * 4 + 4].copy_from_slice(&start.to_le_bytes());
    }
    page[count * 4..count * 4 + 4].copy_from_slice(&runs[count - 1].1.to_le_bytes());
    let mut end = PAGE_SIZE - 1;
    for (index, (_, _, grpprl)) in runs.iter().enumerate() {
        if grpprl.is_empty() {
            continue;
        }
        let start = (end - grpprl.len() - 1) & !1;
        page[start] = u8::try_from(grpprl.len()).unwrap();
        page[start + 1..start + 1 + grpprl.len()].copy_from_slice(grpprl);
        page[(count + 1) * 4 + index] = u8::try_from(start / 2).unwrap();
        end = start;
    }
    page[PAGE_SIZE - 1] = u8::try_from(count).unwrap();
    page
}

/// A paragraph page with runs `[start, end)`, their style and modifiers.
pub(super) fn paragraph_page(runs: &[ParagraphRunSpec]) -> Vec<u8> {
    let mut page = vec![0_u8; PAGE_SIZE];
    let count = runs.len();
    for (index, (start, ..)) in runs.iter().enumerate() {
        page[index * 4..index * 4 + 4].copy_from_slice(&start.to_le_bytes());
    }
    page[count * 4..count * 4 + 4].copy_from_slice(&runs[count - 1].1.to_le_bytes());
    let mut end = PAGE_SIZE - 1;
    for (index, (_, _, style, grpprl)) in runs.iter().enumerate() {
        let mut properties = style.to_le_bytes().to_vec();
        properties.extend_from_slice(grpprl);
        let mut encoded = if properties.len() % 2 == 1 {
            vec![u8::try_from(properties.len().div_ceil(2)).unwrap()]
        } else {
            vec![0, u8::try_from(properties.len() / 2).unwrap()]
        };
        encoded.extend_from_slice(&properties);
        let start = (end - encoded.len()) & !1;
        page[start..start + encoded.len()].copy_from_slice(&encoded);
        page[(count + 1) * 4 + index * 13] = u8::try_from(start / 2).unwrap();
        end = start;
    }
    page[PAGE_SIZE - 1] = u8::try_from(count).unwrap();
    page
}

/// A list level for [`lists`]: start, format, label template, follow character and modifiers.
#[derive(Debug, Clone, Default)]
pub(super) struct LevelSpec {
    pub(super) start: u32,
    pub(super) format: u8,
    pub(super) template: Vec<u16>,
    pub(super) follow: u8,
    pub(super) paragraph: Vec<u8>,
    pub(super) character: Vec<u8>,
}

fn level_bytes(level: &LevelSpec) -> Vec<u8> {
    let mut bytes = vec![0_u8; 28];
    bytes[0..4].copy_from_slice(&level.start.to_le_bytes());
    bytes[4] = level.format;
    bytes[15] = level.follow;
    bytes[24] = u8::try_from(level.character.len()).unwrap();
    bytes[25] = u8::try_from(level.paragraph.len()).unwrap();
    bytes.extend_from_slice(&level.paragraph);
    bytes.extend_from_slice(&level.character);
    bytes.extend_from_slice(&u16::try_from(level.template.len()).unwrap().to_le_bytes());
    for unit in &level.template {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes
}

/// The `PlfLst` of these lists, each an id, whether it is simple, and its levels, with the
/// levels after it.
pub(super) fn lists(definitions: &[(i32, bool, Vec<LevelSpec>)]) -> Vec<u8> {
    let mut bytes = u16::try_from(definitions.len())
        .unwrap()
        .to_le_bytes()
        .to_vec();
    for (id, simple, _) in definitions {
        let mut definition = vec![0_u8; 28];
        definition[0..4].copy_from_slice(&id.to_le_bytes());
        definition[26] = u8::from(*simple);
        bytes.extend_from_slice(&definition);
    }
    for (_, _, levels) in definitions {
        for level in levels {
            bytes.extend(level_bytes(level));
        }
    }
    bytes
}

/// A level override for [`overrides`]: the level, the start it sets, and a level it redefines.
pub(super) type LevelOverride = (u8, Option<u32>, Option<LevelSpec>);

/// The `PlfLfo` of these overrides, each a list id and its level overrides.
pub(super) fn overrides(entries: &[(i32, Vec<LevelOverride>)]) -> Vec<u8> {
    let mut bytes = u32::try_from(entries.len()).unwrap().to_le_bytes().to_vec();
    for (id, levels) in entries {
        let mut entry = vec![0_u8; 16];
        entry[0..4].copy_from_slice(&id.to_le_bytes());
        entry[12] = u8::try_from(levels.len()).unwrap();
        bytes.extend_from_slice(&entry);
    }
    for (_, levels) in entries {
        bytes.extend_from_slice(&[0; 4]);
        for (level, start, redefined) in levels {
            bytes.extend_from_slice(&start.unwrap_or(0).to_le_bytes());
            let mut flags = u32::from(*level);
            if start.is_some() {
                flags |= 0x10;
            }
            if redefined.is_some() {
                flags |= 0x20;
            }
            bytes.extend_from_slice(&flags.to_le_bytes());
            if let Some(redefined) = redefined {
                bytes.extend(level_bytes(redefined));
            }
        }
    }
    bytes
}
