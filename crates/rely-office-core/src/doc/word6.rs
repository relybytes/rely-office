// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! Word 6 and Word 95 documents (`nFib` below 192) in the shape the Word 97 reader takes.
//! Their File Information Block is shorter, every structure lies in the `WordDocument` stream,
//! the text is 8-bit Windows-1252, a bin table names its pages in two bytes, a paragraph run's
//! entry is seven bytes, a style's name is 8-bit and the property modifiers have one-byte codes.
//! Each structure is read and rewritten as Word 97 stores it, so one converter builds the tree.

use encoding_rs::WINDOWS_1252;

use super::fib::Stories;
use super::fkp::{CharacterRun, ParagraphRun};
use super::sections::{SectionLayout, read_sections};
use super::unreadable;
use crate::OfficeError;
use crate::compound::{u16_at, u32_at};

/// The first `nFib` of Word 97; below it the document is Word 6 or Word 95.
pub(super) const FIRST_WORD_97_FIB: u16 = 0x00C0;
/// Word 6 and Word 95 mark their File Information Block with this; some writers keep Word 97's.
const FIB_IDENTIFIER: u16 = 0xA5DC;
const WORD_97_IDENTIFIER: u16 = 0xA5EC;
const FLAG_COMPLEX: u16 = 0x0004;
const FLAG_ENCRYPTED: u16 = 0x0100;
/// Where the `fc`/`lcb` pairs begin, in the order Word 97 kept.
const PAIRS_OFFSET: usize = 0x58;
/// The pairs every Word 6 FIB holds; those up to `PAIR_COUNT` are read where the FIB reaches.
const REQUIRED_PAIRS: usize = 34;
const PAIR_COUNT: usize = 60;
/// From this pair on, the pairs stand after five words (a spare and the bin table page counts)
/// that Word 97 moved elsewhere.
const SPLIT_PAIR: usize = 38;
const SPLIT_GAP: usize = 10;
/// The size of a Word 6 `ATRD`, a comment's reference.
const WORD6_ATRD_SIZE: usize = 20;
/// The size of a Word 97 `ATRDPre10`, which the comment reader takes.
const ATRD_SIZE: usize = 30;
/// The mapping modes of a picture: a metafile up to this one, a TIFF file, a bitmap.
const LAST_METAFILE_MODE: i16 = 8;
const TIFF_MODE: i16 = 98;
const BITMAP_MODE: i16 = 99;
/// Where the story lengths begin: main text, footnotes, headers, macros, comments, endnotes,
/// text boxes and header text boxes.
const STORIES_OFFSET: usize = 0x34;
const PAGE_SIZE: usize = 512;
/// A paragraph run's entry of a page: the offset of its properties and the six bytes of a
/// paragraph height.
const PARAGRAPH_ENTRY: usize = 7;
/// No grpprl of a style, a run or a section is longer than a page.
const MAX_GRPPRL: usize = 4_096;

/// Whether a `WordDocument` stream is a Word 6 or Word 95 document.
pub(super) fn is_word6(word_document: &[u8]) -> bool {
    matches!(
        u16_at(word_document, 0),
        Some(FIB_IDENTIFIER | WORD_97_IDENTIFIER)
    ) && u16_at(word_document, 2).is_some_and(|version| version < FIRST_WORD_97_FIB)
}

/// What the File Information Block of Word 6 says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Word6Fib {
    pub(super) encrypted: bool,
    complex: bool,
    text_start: usize,
    pub(super) stories: Stories,
    pairs: Vec<(usize, usize)>,
}

impl Word6Fib {
    pub(super) fn read(word_document: &[u8]) -> Result<Self, OfficeError> {
        let short = || unreadable("the File Information Block is cut short");
        let flags = u16_at(word_document, 10).ok_or_else(short)?;
        let long = |offset: usize| u32_at(word_document, offset).map(|value| value as usize);
        let story = |index: usize| long(STORIES_OFFSET + index * 4).unwrap_or(0);
        let text_start = long(0x18).ok_or_else(short)?;
        let mut pairs = Vec::with_capacity(PAIR_COUNT);
        for index in 0..PAIR_COUNT {
            let at = pair_offset(index);
            let pair = (long(at), long(at + 4));
            match pair {
                (Some(offset), Some(length)) if index < REQUIRED_PAIRS || at + 8 <= text_start => {
                    pairs.push((offset, length));
                }
                _ if index < REQUIRED_PAIRS => return Err(short()),
                _ => pairs.push((0, 0)),
            }
        }
        Ok(Self {
            encrypted: flags & FLAG_ENCRYPTED != 0,
            complex: flags & FLAG_COMPLEX != 0,
            text_start,
            stories: Stories {
                main: story(0),
                footnotes: story(1),
                headers: story(2),
                comments: story(4),
                endnotes: story(5),
                text_boxes: story(6),
                header_text_boxes: story(7),
            },
            pairs,
        })
    }

    /// The bytes of a structure of the stream; empty when the document has none.
    pub(super) fn structure<'w>(&self, word_document: &'w [u8], index: usize) -> &'w [u8] {
        self.pairs
            .get(index)
            .filter(|(_, length)| *length > 0)
            .and_then(|(offset, length)| word_document.get(*offset..offset.checked_add(*length)?))
            .unwrap_or_default()
    }
}

/// Where the `index`th `fc`/`lcb` pair of the File Information Block stands.
pub(super) fn pair_offset(index: usize) -> usize {
    PAIRS_OFFSET + index * 8 + if index >= SPLIT_PAIR { SPLIT_GAP } else { 0 }
}

/// The comment references (`PlcfandRef`) rewritten as Word 97 stores them: each 20-byte `ATRD`,
/// with its 8-bit initials, widened to an `ATRDPre10` of UTF-16 initials and no bookmark.
pub(super) fn comment_references(plex: &[u8]) -> Vec<u8> {
    let count = plex.len().saturating_sub(4) / (4 + WORD6_ATRD_SIZE);
    let mut rewritten = plex.get(..(count + 1) * 4).unwrap_or_default().to_vec();
    for index in 0..count {
        let at = (count + 1) * 4 + index * WORD6_ATRD_SIZE;
        let record = plex
            .get(at..at + WORD6_ATRD_SIZE)
            .unwrap_or(&[0; WORD6_ATRD_SIZE]);
        let mut modern = vec![0_u8; ATRD_SIZE];
        let length = usize::from(record[0]).min(9);
        let (initials, _) = WINDOWS_1252.decode_without_bom_handling(&record[1..=length]);
        let units: Vec<u16> = initials.encode_utf16().take(9).collect();
        modern[..2].copy_from_slice(&u16::try_from(units.len()).unwrap_or(0).to_le_bytes());
        for (position, unit) in units.iter().enumerate() {
            modern[2 + position * 2..4 + position * 2].copy_from_slice(&unit.to_le_bytes());
        }
        modern[20..22].copy_from_slice(&record[10..12]);
        modern[26..30].copy_from_slice(&(-1_i32).to_le_bytes());
        rewritten.extend(modern);
    }
    rewritten
}

/// The comment authors (`GrpStAtnOwners`), 8-bit strings each after its length byte, as the
/// UTF-16 `Xst`s Word 97 keeps.
pub(super) fn comment_owners(group: &[u8]) -> Vec<u8> {
    let mut rewritten = Vec::new();
    let mut position = 0;
    while let Some(&length) = group.get(position) {
        let Some(name) = group.get(position + 1..position + 1 + usize::from(length)) else {
            break;
        };
        let (name, _) = WINDOWS_1252.decode_without_bom_handling(name);
        let units: Vec<u16> = name.encode_utf16().collect();
        rewritten.extend_from_slice(&u16::try_from(units.len()).unwrap_or(0).to_le_bytes());
        for unit in units {
            rewritten.extend_from_slice(&unit.to_le_bytes());
        }
        position += 1 + usize::from(length);
    }
    rewritten
}

/// The picture whose `PIC` starts at `offset` of the `WordDocument` stream: a metafile drawn as
/// SVG, a bitmap given the file header it lacks, a TIFF file as it is; its size in twips.
pub(super) fn picture(word_document: &[u8], offset: usize) -> Option<(Option<Vec<u8>>, u32, u32)> {
    let total = u32_at(word_document, offset)? as usize;
    let header = usize::from(u16_at(word_document, offset + 4)?);
    let picture = word_document.get(offset..offset.checked_add(total)?)?;
    let mode = i16::from_le_bytes(u16_at(picture, 6)?.to_le_bytes());
    let goal = |at: usize| u32::from(u16_at(picture, at).unwrap_or(0));
    let width = goal(28) * goal(32).max(1) / 1000;
    let height = goal(30) * goal(34).max(1) / 1000;
    let data = picture.get(header..).unwrap_or_default();
    let bytes = match mode {
        1..=LAST_METAFILE_MODE => crate::metafile::convert_metafile_to_svg(data),
        BITMAP_MODE => bitmap_file(data),
        TIFF_MODE => Some(data.to_vec()).filter(|data| !data.is_empty()),
        _ => None,
    };
    Some((bytes, width, height))
}

/// A device-independent bitmap as a BMP file: the file header before its info header, palette
/// and pixels.
fn bitmap_file(dib: &[u8]) -> Option<Vec<u8>> {
    if dib.starts_with(b"BM") {
        return Some(dib.to_vec());
    }
    let info = u32_at(dib, 0)?;
    if !(12..=124).contains(&info) {
        return None;
    }
    let (bits, compression, used) = if info == 12 {
        (u16_at(dib, 10)?, 0, 0)
    } else {
        (u16_at(dib, 14)?, u32_at(dib, 16)?, u32_at(dib, 32)?)
    };
    let entry = if info == 12 { 3 } else { 4 };
    let colours = match (used, bits) {
        (0, 1 | 4 | 8) => 1_u32 << bits,
        (used, _) => used,
    };
    let masks = if compression == 3 && info == 40 {
        12
    } else {
        0
    };
    let pixels = 14 + info + colours * entry + masks;
    let size = u32::try_from(dib.len()).ok()?.checked_add(14)?;
    let mut file = Vec::with_capacity(size as usize);
    file.extend_from_slice(b"BM");
    file.extend_from_slice(&size.to_le_bytes());
    file.extend_from_slice(&[0; 4]);
    file.extend_from_slice(&pixels.to_le_bytes());
    file.extend_from_slice(dib);
    Some(file)
}

/// The characters of every story with the stream offset of each: one stretch from the start of
/// the text, or the pieces of a document saved fast.
pub(super) fn text(word_document: &[u8], fib: &Word6Fib) -> (Vec<char>, Vec<u32>) {
    let total = fib.stories.total();
    let mut stretches: Vec<(usize, usize)> = Vec::new();
    if fib.complex {
        let clx = fib.structure(word_document, super::fib::pair::PIECE_TABLE);
        stretches = pieces(clx);
    }
    if stretches.is_empty() {
        stretches.push((fib.text_start, total));
    }
    let mut characters = Vec::new();
    let mut offsets = Vec::new();
    for (offset, count) in stretches {
        let count = count.min(total.saturating_sub(characters.len()));
        let Some(bytes) = word_document.get(offset..offset.saturating_add(count)) else {
            continue;
        };
        let (decoded, _) = WINDOWS_1252.decode_without_bom_handling(bytes);
        for (index, character) in decoded.chars().enumerate() {
            characters.push(character);
            offsets.push(u32::try_from(offset + index).unwrap_or(u32::MAX));
        }
        if characters.len() >= total {
            break;
        }
    }
    (characters, offsets)
}

/// The pieces of a `Clx`, each its stream offset and its count of characters; the property runs
/// before them are skipped.
fn pieces(clx: &[u8]) -> Vec<(usize, usize)> {
    let mut position = 0;
    while clx.get(position) == Some(&1) {
        let length = usize::from(u16_at(clx, position + 1).unwrap_or(0));
        position += 3 + length;
    }
    if clx.get(position) != Some(&2) {
        return Vec::new();
    }
    let Some(length) = u32_at(clx, position + 1).map(|length| length as usize) else {
        return Vec::new();
    };
    let table = clx
        .get(position + 5..position + 5 + length)
        .unwrap_or_default();
    let count = table.len().saturating_sub(4) / 12;
    (0..count)
        .filter_map(|index| {
            let start = u32_at(table, index * 4)? as usize;
            let end = u32_at(table, index * 4 + 4)? as usize;
            let offset = u32_at(table, (count + 1) * 4 + index * 8 + 2)? as usize;
            Some((offset, end.saturating_sub(start)))
        })
        .collect()
}

/// The pages a bin table names, each two bytes in Word 6.
fn pages<'w>(word_document: &'w [u8], plex: &[u8]) -> Vec<&'w [u8]> {
    let count = plex.len().saturating_sub(4) / 6;
    (0..count)
        .filter_map(|index| {
            let page = usize::from(u16_at(plex, (count + 1) * 4 + index * 2)?);
            word_document.get(page * PAGE_SIZE..(page + 1) * PAGE_SIZE)
        })
        .collect()
}

/// The character runs of every page, their modifiers as Word 97 codes them.
pub(super) fn character_runs(word_document: &[u8], fib: &Word6Fib) -> Vec<CharacterRun> {
    let plex = fib.structure(word_document, super::fib::pair::CHARACTER_PAGES);
    let mut runs = Vec::new();
    for page in pages(word_document, plex) {
        let count = usize::from(page[PAGE_SIZE - 1]);
        for index in 0..count {
            let (Some(start), Some(end)) = (u32_at(page, index * 4), u32_at(page, index * 4 + 4))
            else {
                break;
            };
            let offset = usize::from(page.get((count + 1) * 4 + index).copied().unwrap_or(0)) * 2;
            let grpprl = if offset == 0 {
                Vec::new()
            } else {
                let length = usize::from(page.get(offset).copied().unwrap_or(0));
                modern_modifiers(
                    page.get(offset + 1..offset + 1 + length)
                        .unwrap_or_default(),
                )
            };
            runs.push(CharacterRun { start, end, grpprl });
        }
    }
    runs.sort_by_key(|run| run.start);
    runs
}

/// The paragraph runs of every page, a run's properties a count of words, its style and its
/// modifiers.
pub(super) fn paragraph_runs(word_document: &[u8], fib: &Word6Fib) -> Vec<ParagraphRun> {
    let plex = fib.structure(word_document, super::fib::pair::PARAGRAPH_PAGES);
    let mut runs = Vec::new();
    for page in pages(word_document, plex) {
        let count = usize::from(page[PAGE_SIZE - 1]);
        for index in 0..count {
            let (Some(start), Some(end)) = (u32_at(page, index * 4), u32_at(page, index * 4 + 4))
            else {
                break;
            };
            let entry = (count + 1) * 4 + index * PARAGRAPH_ENTRY;
            let offset = usize::from(page.get(entry).copied().unwrap_or(0)) * 2;
            let (style, grpprl) = if offset == 0 {
                (0, Vec::new())
            } else {
                let words = usize::from(page.get(offset).copied().unwrap_or(0));
                let properties = page
                    .get(offset + 1..offset + 1 + words * 2)
                    .unwrap_or_default();
                (
                    u16_at(properties, 0).unwrap_or(0),
                    modern_modifiers(properties.get(2..).unwrap_or_default()),
                )
            };
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

/// The style sheet rewritten as Word 97 stores it: each `STD`'s fixed part widened to ten
/// bytes, its 8-bit name in UTF-16, and the modifiers of its exceptions in Word 97 codes.
pub(super) fn style_sheet(bytes: &[u8]) -> Vec<u8> {
    let Some(header_length) = u16_at(bytes, 0).map(usize::from) else {
        return Vec::new();
    };
    let Some(header) = bytes.get(2..2 + header_length) else {
        return Vec::new();
    };
    let count = u16_at(header, 0).unwrap_or(0);
    let base_length = usize::from(u16_at(header, 2).unwrap_or(8));
    let mut new_header = header.to_vec();
    if new_header.len() < 14 {
        new_header.resize(14, 0);
    }
    new_header[2..4].copy_from_slice(&10_u16.to_le_bytes());
    let mut written = u16::try_from(new_header.len())
        .unwrap_or(0)
        .to_le_bytes()
        .to_vec();
    written.extend_from_slice(&new_header);
    let mut position = 2 + header_length;
    for _ in 0..count {
        let Some(length) = u16_at(bytes, position).map(usize::from) else {
            break;
        };
        let definition = bytes
            .get(position + 2..position + 2 + length)
            .unwrap_or_default();
        position += 2 + length;
        let rewritten = if definition.is_empty() {
            Vec::new()
        } else {
            style_definition(definition, base_length)
        };
        written.extend_from_slice(&u16::try_from(rewritten.len()).unwrap_or(0).to_le_bytes());
        written.extend_from_slice(&rewritten);
    }
    written
}

fn style_definition(definition: &[u8], base_length: usize) -> Vec<u8> {
    let Some(base) = definition.get(..base_length.min(definition.len())) else {
        return Vec::new();
    };
    let mut rewritten = base.to_vec();
    rewritten.resize(10, 0);
    let name_length = usize::from(definition.get(base_length).copied().unwrap_or(0));
    let name = definition
        .get(base_length + 1..base_length + 1 + name_length)
        .unwrap_or_default();
    let (name, _) = WINDOWS_1252.decode_without_bom_handling(name);
    let units: Vec<u16> = name.encode_utf16().collect();
    rewritten.extend_from_slice(&u16::try_from(units.len()).unwrap_or(0).to_le_bytes());
    for unit in units {
        rewritten.extend_from_slice(&unit.to_le_bytes());
    }
    rewritten.extend_from_slice(&[0, 0]);
    // The name is followed by a terminating zero, then the exceptions on even offsets.
    let mut position = base_length + 1 + name_length + 1;
    let paragraph = u16_at(definition, 2).is_some_and(|kind| kind & 0x000F == 1);
    let exceptions = if paragraph { 2 } else { 1 };
    for index in 0..exceptions {
        position += position % 2;
        let Some(length) = u16_at(definition, position).map(usize::from) else {
            break;
        };
        let exception = definition
            .get(position + 2..position + 2 + length)
            .unwrap_or_default();
        position += 2 + length;
        let converted = if paragraph && index == 0 {
            let mut converted = exception.get(..2).unwrap_or(&[0, 0]).to_vec();
            converted.extend(modern_modifiers(exception.get(2..).unwrap_or_default()));
            converted
        } else {
            modern_modifiers(exception)
        };
        rewritten.extend_from_slice(&u16::try_from(converted.len()).unwrap_or(0).to_le_bytes());
        rewritten.extend_from_slice(&converted);
        if converted.len() % 2 == 1 {
            rewritten.push(0);
        }
    }
    rewritten
}

/// The names of the font table, `FFN`s of 8-bit names.
pub(super) fn font_names(table: &[u8]) -> Vec<String> {
    let total = usize::from(u16_at(table, 0).unwrap_or(0)).min(table.len());
    let mut names = Vec::new();
    let mut position = 2;
    while position < total {
        let size = usize::from(table[position]) + 1;
        let Some(font) = table.get(position..position + size) else {
            break;
        };
        let name: Vec<u8> = font
            .get(6..)
            .unwrap_or_default()
            .iter()
            .copied()
            .take_while(|byte| *byte != 0)
            .collect();
        names.push(
            WINDOWS_1252
                .decode_without_bom_handling(&name)
                .0
                .into_owned(),
        );
        position += size;
    }
    names
}

/// The sections, each section's modifiers rewritten in Word 97 codes before the Word 97 reader
/// reads them.
pub(super) fn sections(word_document: &[u8], fib: &Word6Fib) -> Vec<SectionLayout> {
    let plex = fib.structure(word_document, super::fib::pair::SECTIONS);
    let count = plex.len().saturating_sub(4) / 16;
    let mut rewritten_plex = plex.get(..(count + 1) * 4).unwrap_or_default().to_vec();
    let mut stream = Vec::new();
    for index in 0..count {
        let descriptor = (count + 1) * 4 + index * 12;
        let mut entry = plex
            .get(descriptor..descriptor + 12)
            .unwrap_or(&[0; 12])
            .to_vec();
        if let Some(offset) = u32_at(&entry, 2).filter(|offset| *offset != u32::MAX) {
            let offset = offset as usize;
            let length = usize::from(u16_at(word_document, offset).unwrap_or(0)).min(MAX_GRPPRL);
            let grpprl = modern_modifiers(
                word_document
                    .get(offset + 2..offset + 2 + length)
                    .unwrap_or_default(),
            );
            let at = u32::try_from(stream.len()).unwrap_or(u32::MAX);
            stream.extend_from_slice(&u16::try_from(grpprl.len()).unwrap_or(0).to_le_bytes());
            stream.extend_from_slice(&grpprl);
            entry[2..6].copy_from_slice(&at.to_le_bytes());
        }
        rewritten_plex.extend_from_slice(&entry);
    }
    read_sections(&stream, &rewritten_plex)
}

/// How long a Word 6 modifier's operand is, from its code and the bytes after it: `None` for a
/// code this reader does not know, after which nothing can be read.
fn operand_length(code: u8, after: &[u8]) -> Option<usize> {
    let variable = || after.first().map(|length| usize::from(*length) + 1);
    Some(match code {
        3 | 12 | 15 | 23 | 52 | 68 | 74 | 77 | 79 | 81 | 82 | 103 | 105 | 106 | 108 | 120 | 133
        | 188 | 191 => {
            return variable();
        }
        // The table layout of a row counts its own length byte in a two-byte length.
        190 => usize::from(u16_at(after, 0)?).saturating_sub(1) + 2,
        0 | 83 | 163 => 0,
        4..=11
        | 13
        | 14
        | 24
        | 25
        | 29
        | 37
        | 44
        | 50
        | 51
        | 65..=67
        | 71
        | 75
        | 85..=92
        | 94
        | 98
        | 100
        | 102
        | 104
        | 117
        | 118
        | 119
        | 131
        | 132
        | 138
        | 139
        | 142
        | 143
        | 146
        | 147
        | 150..=153
        | 158
        | 159
        | 162
        | 185
        | 186 => 1,
        2
        | 16..=19
        | 21
        | 22
        | 26..=28
        | 30..=36
        | 38..=43
        | 45..=49
        | 69
        | 72
        | 80
        | 93
        | 96
        | 97
        | 99
        | 101
        | 107
        | 109..=113
        | 115
        | 116
        | 121..=124
        | 140
        | 141
        | 144
        | 145
        | 148
        | 149
        | 154..=157
        | 160
        | 161
        | 164..=171
        | 182..=184
        | 189
        | 195
        | 197
        | 198 => 2,
        73 | 95 | 136 | 137 => 3,
        20 | 70 | 192 | 194 | 196 | 200 => 4,
        193 | 199 => 5,
        187 => 12,
        _ => return None,
    })
}

/// The Word 97 code of a Word 6 modifier that keeps its operand as it is.
fn modern_code(code: u8) -> Option<u16> {
    Some(match code {
        2 => 0x4600,
        5 => 0x2403,
        7 => 0x2405,
        8 => 0x2406,
        9 => 0x2407,
        16 => 0x840E,
        17 => 0x840F,
        19 => 0x8411,
        20 => 0x6412,
        21 => 0xA413,
        22 => 0xA414,
        24 => 0x2416,
        25 => 0x2417,
        47 => 0x442D,
        51 => 0x2431,
        65 => 0x0800,
        66 => 0x0801,
        68 => 0x6A03,
        71 => 0x0806,
        75 => 0x080A,
        80 => 0x4A30,
        85 => 0x0835,
        86 => 0x0836,
        87 => 0x0837,
        88 => 0x0838,
        89 => 0x0839,
        90 => 0x083A,
        91 => 0x083B,
        92 => 0x083C,
        93 => 0x4A4F,
        94 => 0x2A3E,
        98 => 0x2A42,
        99 => 0x4A43,
        104 => 0x2A48,
        117 => 0x0855,
        142 => 0x3009,
        143 => 0x300A,
        144 => 0x500B,
        145 => 0x900C,
        156 => 0xB017,
        157 => 0xB018,
        162 => 0x301D,
        164 => 0xB01F,
        165 => 0xB020,
        166 => 0xB021,
        167 => 0xB022,
        168 => 0x9023,
        169 => 0x9024,
        _ => return None,
    })
}

/// A Word 6 `grpprl` as Word 97 codes it: the modifiers this reader turns into a Word 97 one,
/// the table layout of a row widened, and the others left out.
pub(super) fn modern_modifiers(grpprl: &[u8]) -> Vec<u8> {
    let grpprl = grpprl
        .get(..grpprl.len().min(MAX_GRPPRL))
        .unwrap_or_default();
    let mut modern = Vec::new();
    let mut position = 0;
    while let Some(&code) = grpprl.get(position) {
        let after = grpprl.get(position + 1..).unwrap_or_default();
        let Some(length) = operand_length(code, after) else {
            break;
        };
        let Some(operand) = after.get(..length) else {
            break;
        };
        position += 1 + length;
        if code == 190 {
            modern.extend(table_definition(operand.get(2..).unwrap_or_default()));
            continue;
        }
        let Some(modern_code) = modern_code(code) else {
            continue;
        };
        // A picture's location is variable in Word 6, its four bytes after the length.
        let operand = if code == 68 {
            operand.get(1..5).unwrap_or(&[0; 4])
        } else {
            operand
        };
        modern.extend_from_slice(&modern_code.to_le_bytes());
        modern.extend_from_slice(operand);
    }
    modern
}

/// A row's `sprmTDefTable` in Word 97's form: the cell edges as they are, each Word 6 cell of
/// ten bytes widened to twenty, a border of the kind its two-byte `BRC` names.
fn table_definition(operand: &[u8]) -> Vec<u8> {
    let count = usize::from(operand.first().copied().unwrap_or(0)).min(63);
    let edges = operand.get(1..1 + (count + 1) * 2).unwrap_or_default();
    let cells_start = 1 + (count + 1) * 2;
    let mut body = vec![u8::try_from(count).unwrap_or(0)];
    body.extend_from_slice(edges);
    body.resize(1 + (count + 1) * 2, 0);
    for index in 0..count {
        let cell = operand
            .get(cells_start + index * 10..cells_start + (index + 1) * 10)
            .unwrap_or(&[0; 10]);
        let mut wide = vec![0_u8; 20];
        wide[..2].copy_from_slice(cell.get(..2).unwrap_or(&[0, 0]));
        for side in 0..4 {
            // Bits 3 and 4 are the line: none, single, thick or double, numbered as in Word 97.
            let kind = (u16_at(cell, 2 + side * 2).unwrap_or(0) >> 3) & 0x3;
            if kind != 0 {
                wide[4 + side * 4] = 8;
                wide[4 + side * 4 + 1] = u8::try_from(kind).unwrap_or(1);
            }
        }
        body.extend_from_slice(&wide);
    }
    let mut modern = 0xD608_u16.to_le_bytes().to_vec();
    modern.extend_from_slice(&u16::try_from(body.len() + 1).unwrap_or(0).to_le_bytes());
    modern.extend_from_slice(&body);
    modern
}

mod drawings;

pub(super) use drawings::{Word6Drawing, drawings};

#[cfg(test)]
mod tests;
