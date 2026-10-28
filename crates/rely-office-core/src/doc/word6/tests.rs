// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;
use crate::document::{Alignment, Block, Inline, Mark, plain_text};
use crate::test_support::build_compound_file;
use crate::{Limits, extract_text, read_document_tree};
use std::ops::Range;

const TEXT_START: usize = 0x300;
const WORD6_FIB: u16 = 101;

/// A paragraph of the document: its text up to and with its mark, its style and its Word 6
/// paragraph modifiers.
struct Word6Paragraph {
    text: Vec<u8>,
    style: u16,
    grpprl: Vec<u8>,
}

/// A Word 6 document as Word 6 writes it: the File Information Block, the 8-bit text from
/// [`TEXT_START`], one page of character runs, one page of paragraph runs and the tables.
struct Word6Builder {
    paragraphs: Vec<Word6Paragraph>,
    /// Character runs by their range of characters, with their modifiers.
    characters: Vec<(Range<usize>, Vec<u8>)>,
    flags: u16,
    complex: bool,
}

impl Word6Builder {
    fn new() -> Self {
        Self {
            paragraphs: Vec::new(),
            characters: Vec::new(),
            flags: 0,
            complex: false,
        }
    }

    fn paragraph(mut self, text: &str, style: u16, grpprl: &[u8]) -> Self {
        let (encoded, _, _) = WINDOWS_1252.encode(text);
        self.paragraphs.push(Word6Paragraph {
            text: encoded.into_owned(),
            style,
            grpprl: grpprl.to_vec(),
        });
        self
    }

    fn character(mut self, range: Range<usize>, grpprl: &[u8]) -> Self {
        self.characters.push((range, grpprl.to_vec()));
        self
    }

    fn text(&self) -> Vec<u8> {
        self.paragraphs
            .iter()
            .flat_map(|paragraph| paragraph.text.clone())
            .collect()
    }

    fn character_page(&self, total: usize) -> Vec<u8> {
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
        let mut page = vec![0_u8; PAGE_SIZE];
        let count = runs.len();
        let mut free = PAGE_SIZE - 1;
        for (index, (start, end, grpprl)) in runs.iter().enumerate() {
            page[index * 4..index * 4 + 4].copy_from_slice(&stream_offset(*start).to_le_bytes());
            page[index * 4 + 4..index * 4 + 8].copy_from_slice(&stream_offset(*end).to_le_bytes());
            if grpprl.is_empty() {
                continue;
            }
            free -= grpprl.len() + 1;
            free -= free % 2;
            page[free] = u8::try_from(grpprl.len()).unwrap();
            page[free + 1..free + 1 + grpprl.len()].copy_from_slice(grpprl);
            page[(count + 1) * 4 + index] = u8::try_from(free / 2).unwrap();
        }
        page[PAGE_SIZE - 1] = u8::try_from(count).unwrap();
        page
    }

    fn paragraph_page(&self) -> Vec<u8> {
        let mut page = vec![0_u8; PAGE_SIZE];
        let count = self.paragraphs.len();
        let mut free = PAGE_SIZE - 1;
        let mut start = 0;
        for (index, paragraph) in self.paragraphs.iter().enumerate() {
            let end = start + paragraph.text.len();
            page[index * 4..index * 4 + 4].copy_from_slice(&stream_offset(start).to_le_bytes());
            page[index * 4 + 4..index * 4 + 8].copy_from_slice(&stream_offset(end).to_le_bytes());
            start = end;
            let mut properties = paragraph.style.to_le_bytes().to_vec();
            properties.extend_from_slice(&paragraph.grpprl);
            if properties.len() % 2 == 1 {
                properties.push(0);
            }
            free -= properties.len() + 1;
            free -= free % 2;
            page[free] = u8::try_from(properties.len() / 2).unwrap();
            page[free + 1..free + 1 + properties.len()].copy_from_slice(&properties);
            page[(count + 1) * 4 + index * PARAGRAPH_ENTRY] = u8::try_from(free / 2).unwrap();
        }
        page[PAGE_SIZE - 1] = u8::try_from(count).unwrap();
        page
    }

    fn build(&self) -> Vec<u8> {
        let word_document = self.word_document();
        build_compound_file(&[("WordDocument", &word_document)])
    }

    fn word_document(&self) -> Vec<u8> {
        let text = self.text();
        let total = text.len();
        let mut stream = vec![0_u8; TEXT_START];
        stream.extend_from_slice(&text);
        stream.resize(stream.len().next_multiple_of(PAGE_SIZE), 0);
        let character_page = stream.len() / PAGE_SIZE;
        stream.extend(self.character_page(total));
        let paragraph_page = stream.len() / PAGE_SIZE;
        stream.extend(self.paragraph_page());

        let mut pairs: Vec<(usize, usize, Vec<u8>)> = vec![
            (super::super::fib::pair::STYLES, 0, style_sheet_bytes()),
            (super::super::fib::pair::FONTS, 0, font_table_bytes()),
            (
                super::super::fib::pair::CHARACTER_PAGES,
                0,
                bin_table(total, character_page),
            ),
            (
                super::super::fib::pair::PARAGRAPH_PAGES,
                0,
                bin_table(total, paragraph_page),
            ),
            (super::super::fib::pair::DOCUMENT_PROPERTIES, 0, dop_bytes()),
        ];
        let section_modifiers: &[u8] = &[164, 0x82, 0x2E, 165, 0x7E, 0x41, 166, 0x6E, 0x04];
        let sepx_offset = stream.len();
        stream.extend_from_slice(
            &u16::try_from(section_modifiers.len())
                .unwrap()
                .to_le_bytes(),
        );
        stream.extend_from_slice(section_modifiers);
        let mut sections = Vec::new();
        sections.extend_from_slice(&0_u32.to_le_bytes());
        sections.extend_from_slice(&u32::try_from(total).unwrap().to_le_bytes());
        sections.extend_from_slice(&0_u16.to_le_bytes());
        sections.extend_from_slice(&u32::try_from(sepx_offset).unwrap().to_le_bytes());
        sections.extend_from_slice(&0_u16.to_le_bytes());
        sections.extend_from_slice(&u32::MAX.to_le_bytes());
        pairs.push((super::super::fib::pair::SECTIONS, 0, sections));
        if self.complex {
            let mut clx = vec![2_u8];
            let mut table = Vec::new();
            let half = total / 2;
            for cp in [0, half, total] {
                table.extend_from_slice(&u32::try_from(cp).unwrap().to_le_bytes());
            }
            for start in [0, half] {
                table.extend_from_slice(&0_u16.to_le_bytes());
                table.extend_from_slice(&u32::try_from(TEXT_START + start).unwrap().to_le_bytes());
                table.extend_from_slice(&0_u16.to_le_bytes());
            }
            clx.extend_from_slice(&u32::try_from(table.len()).unwrap().to_le_bytes());
            clx.extend_from_slice(&table);
            pairs.push((super::super::fib::pair::PIECE_TABLE, 0, clx));
        }
        for (_, offset, bytes) in &mut pairs {
            *offset = stream.len();
            stream.extend_from_slice(bytes);
            if stream.len() % 2 == 1 {
                stream.push(0);
            }
        }

        let put16 = |stream: &mut Vec<u8>, at: usize, value: u16| {
            stream[at..at + 2].copy_from_slice(&value.to_le_bytes());
        };
        let put32 = |stream: &mut Vec<u8>, at: usize, value: usize| {
            stream[at..at + 4].copy_from_slice(&u32::try_from(value).unwrap().to_le_bytes());
        };
        put16(&mut stream, 0, FIB_IDENTIFIER);
        put16(&mut stream, 2, WORD6_FIB);
        put16(&mut stream, 4, 0x6027);
        put16(&mut stream, 6, 0x0410);
        let flags = self.flags | if self.complex { FLAG_COMPLEX } else { 0 };
        put16(&mut stream, 10, flags);
        put16(&mut stream, 12, WORD6_FIB);
        put32(&mut stream, 0x18, TEXT_START);
        put32(&mut stream, 0x1C, TEXT_START + total);
        let length = stream.len();
        put32(&mut stream, 0x20, length);
        put32(&mut stream, STORIES_OFFSET, total);
        for (index, offset, bytes) in &pairs {
            put32(&mut stream, PAIRS_OFFSET + index * 8, *offset);
            put32(&mut stream, PAIRS_OFFSET + index * 8 + 4, bytes.len());
        }
        put16(&mut stream, 0x18A, u16::try_from(character_page).unwrap());
        put16(&mut stream, 0x18C, u16::try_from(paragraph_page).unwrap());
        put16(&mut stream, 0x18E, 1);
        put16(&mut stream, 0x190, 1);
        stream
    }
}

fn stream_offset(position: usize) -> u32 {
    u32::try_from(TEXT_START + position).unwrap()
}

fn bin_table(total: usize, page: usize) -> Vec<u8> {
    let mut plex = Vec::new();
    plex.extend_from_slice(&stream_offset(0).to_le_bytes());
    plex.extend_from_slice(&stream_offset(total).to_le_bytes());
    plex.extend_from_slice(&u16::try_from(page).unwrap().to_le_bytes());
    plex
}

/// A Word 6 `STD`: the eight-byte fixed part, the 8-bit name and the exceptions, each on an
/// even offset.
fn style(sti: u16, paragraph: bool, base: u16, name: &str, exceptions: &[&[u8]]) -> Vec<u8> {
    let mut definition = Vec::new();
    definition.extend_from_slice(&sti.to_le_bytes());
    let kind: u16 = if paragraph { 1 } else { 2 };
    definition.extend_from_slice(&((base << 4) | kind).to_le_bytes());
    let count = u16::try_from(exceptions.len()).unwrap();
    definition.extend_from_slice(&count.to_le_bytes());
    definition.extend_from_slice(&0_u16.to_le_bytes());
    definition.push(u8::try_from(name.len()).unwrap());
    definition.extend_from_slice(name.as_bytes());
    definition.push(0);
    for exception in exceptions {
        if definition.len() % 2 == 1 {
            definition.push(0);
        }
        definition.extend_from_slice(&u16::try_from(exception.len()).unwrap().to_le_bytes());
        definition.extend_from_slice(exception);
    }
    definition
}

/// Normal in Arial at eleven points, heading 1 bold at sixteen points, then empty slots to the
/// default paragraph font.
fn style_sheet_bytes() -> Vec<u8> {
    let mut definitions = vec![
        style(
            0,
            true,
            0x0FFF,
            "Normal",
            &[&[0, 0], &[93, 1, 0, 99, 22, 0]],
        ),
        style(
            1,
            true,
            0,
            "heading 1",
            &[&[1, 0, 21, 240, 0], &[85, 1, 99, 32, 0]],
        ),
    ];
    definitions.extend((2..10).map(|_| Vec::new()));
    definitions.push(style(65, false, 0x0FFF, "Default Paragraph Font", &[&[]]));
    let mut bytes = Vec::new();
    let header: [u16; 9] = [
        u16::try_from(definitions.len()).unwrap(),
        8,
        1,
        0x5B,
        0x0F,
        0,
        0,
        0,
        0,
    ];
    bytes.extend_from_slice(&18_u16.to_le_bytes());
    for value in header {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for definition in definitions {
        bytes.extend_from_slice(&u16::try_from(definition.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&definition);
    }
    bytes
}

fn font_table_bytes() -> Vec<u8> {
    let mut fonts = Vec::new();
    for (family, name) in [(0x16_u8, "Times New Roman"), (0x26, "Arial")] {
        let mut font = vec![0, family, 0x90, 0x01, 0, 0];
        font.extend_from_slice(name.as_bytes());
        font.push(0);
        font[0] = u8::try_from(font.len() - 1).unwrap();
        fonts.extend(font);
    }
    let mut table = u16::try_from(fonts.len() + 2)
        .unwrap()
        .to_le_bytes()
        .to_vec();
    table.extend(fonts);
    table
}

fn dop_bytes() -> Vec<u8> {
    let mut dop = vec![0_u8; 84];
    dop[10..12].copy_from_slice(&709_u16.to_le_bytes());
    dop
}

/// A row of two cells, 2000 and 4000 twips wide, the first with a single line all round and the
/// second with a width but no line.
fn row_modifiers() -> Vec<u8> {
    let mut body = vec![2_u8];
    for edge in [0_i16, 2000, 6000] {
        body.extend_from_slice(&edge.to_le_bytes());
    }
    for border in [0x0009_u16, 0x0001] {
        body.extend_from_slice(&[0, 0]);
        for _ in 0..4 {
            body.extend_from_slice(&border.to_le_bytes());
        }
    }
    let mut modifiers = vec![24, 1, 25, 1, 190];
    modifiers.extend_from_slice(&u16::try_from(body.len() + 1).unwrap().to_le_bytes());
    modifiers.extend(body);
    modifiers
}

fn sample() -> Word6Builder {
    Word6Builder::new()
        .paragraph("Titolo del verbale\r", 1, &[])
        .paragraph("Il comandante \u{e8} in sede.\r", 0, &[5, 1, 21, 120, 0])
        .paragraph("Nome\u{7}", 0, &[24, 1])
        .paragraph("Grado\u{7}", 0, &[24, 1])
        .paragraph("\u{7}", 0, &row_modifiers())
        .paragraph("Fine.\r", 0, &[])
        .character(33..43, &[85, 1, 86, 1])
}

fn sample_tree() -> crate::document::DocumentTree {
    read_document_tree(&sample().build(), &Limits::default()).unwrap()
}

#[test]
fn is_word6_tells_a_word6_fib_from_a_word97_one() {
    let stream = sample().word_document();
    assert!(is_word6(&stream));
    let mut word97 = stream;
    word97[2..4].copy_from_slice(&0x00C1_u16.to_le_bytes());
    assert!(!is_word6(&word97));
    assert!(!is_word6(&[0xEC]));
}

#[test]
fn the_fib_reads_flags_stories_and_pairs() {
    let stream = sample().word_document();
    let fib = Word6Fib::read(&stream).unwrap();
    assert!(!fib.encrypted);
    assert!(!fib.complex);
    assert_eq!(fib.text_start, TEXT_START);
    assert_eq!(fib.stories.main, sample().text().len());
    assert_eq!(
        fib.structure(&stream, super::super::fib::pair::STYLES),
        style_sheet_bytes().as_slice()
    );
    assert_eq!(fib.structure(&stream, 30), &[] as &[u8]);
    assert_eq!(fib.structure(&stream, 99), &[] as &[u8]);
}

#[test]
fn a_fib_cut_short_is_unreadable() {
    let stream = sample().word_document();
    assert!(matches!(
        Word6Fib::read(&stream[..0x60]),
        Err(OfficeError::UnreadableDocument(_))
    ));
}

#[test]
fn an_encrypted_document_is_refused() {
    let mut builder = sample();
    builder.flags = FLAG_ENCRYPTED;
    let bytes = builder.build();
    assert!(matches!(
        read_document_tree(&bytes, &Limits::default()),
        Err(OfficeError::Encrypted)
    ));
    assert!(matches!(
        extract_text(&bytes, Some("cifrato.doc"), &Limits::default()),
        Err(OfficeError::Encrypted)
    ));
}

#[test]
fn the_text_is_windows_1252() {
    let stream = sample().word_document();
    let fib = Word6Fib::read(&stream).unwrap();
    let (characters, offsets) = text(&stream, &fib);
    let read: String = characters.iter().collect();
    assert!(read.contains("comandante \u{e8} in sede"));
    assert_eq!(offsets[0], u32::try_from(TEXT_START).unwrap());
    assert_eq!(offsets.len(), characters.len());
}

#[test]
fn a_document_saved_fast_reads_its_pieces() {
    let mut builder = sample();
    builder.complex = true;
    let stream = builder.word_document();
    let fib = Word6Fib::read(&stream).unwrap();
    let read: String = text(&stream, &fib).0.into_iter().collect();
    let plain = sample().word_document();
    let plain_fib = Word6Fib::read(&plain).unwrap();
    let expected: String = text(&plain, &plain_fib).0.into_iter().collect();
    assert_eq!(read, expected);
}

#[test]
fn pieces_skip_property_runs_and_refuse_a_broken_table() {
    let mut clx = vec![1, 2, 0, 0xAA, 0xBB, 2];
    let mut table = Vec::new();
    for cp in [0_u32, 4] {
        table.extend_from_slice(&cp.to_le_bytes());
    }
    table.extend_from_slice(&[0, 0]);
    table.extend_from_slice(&0x400_u32.to_le_bytes());
    table.extend_from_slice(&[0, 0]);
    clx.extend_from_slice(&u32::try_from(table.len()).unwrap().to_le_bytes());
    clx.extend(table);
    assert_eq!(pieces(&clx), vec![(0x400, 4)]);
    assert_eq!(pieces(&[3]), Vec::new());
    assert_eq!(pieces(&[2, 1]), Vec::new());
}

#[test]
fn the_tree_has_its_heading_paragraphs_and_table() {
    let tree = sample_tree();
    let Block::Heading { level, content, .. } = &tree.blocks[0] else {
        panic!("{:?}", tree.blocks[0]);
    };
    assert_eq!(*level, 1);
    assert_eq!(plain_text(content), "Titolo del verbale");
    let Block::Paragraph {
        content,
        align,
        look,
    } = &tree.blocks[1]
    else {
        panic!("{:?}", tree.blocks[1]);
    };
    assert_eq!(*align, Some(Alignment::Center));
    assert_eq!(look.space_before_twips, Some(120));
    let bold = content
        .iter()
        .find_map(|inline| match inline {
            Inline::Text { text, marks, .. } if marks.contains(&Mark::Bold) => Some((text, marks)),
            _ => None,
        })
        .unwrap();
    assert_eq!(bold.0, "\u{e8} in sede.");
    assert!(bold.1.contains(&Mark::Italic));
    let Block::Table { rows, .. } = &tree.blocks[2] else {
        panic!("{:?}", tree.blocks[2]);
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].cells.len(), 2);
    let Block::Paragraph { content, .. } = &rows[0].cells[1].blocks[0] else {
        panic!("{:?}", rows[0].cells[1]);
    };
    assert_eq!(plain_text(content), "Grado");
    let Block::Paragraph { content, .. } = &tree.blocks[3] else {
        panic!("{:?}", tree.blocks[3]);
    };
    assert_eq!(plain_text(content), "Fine.");
}

#[test]
fn the_styles_and_fonts_give_the_look() {
    let tree = sample_tree();
    let Block::Heading { look, .. } = &tree.blocks[0] else {
        panic!("{:?}", tree.blocks[0]);
    };
    assert_eq!(look.space_before_twips, Some(240));
    assert_eq!(look.font.family.as_deref(), Some("Arial"));
    assert_eq!(look.font.size_half_points, Some(32));
    assert_eq!(tree.layout.width_twips, 11_906);
    assert_eq!(tree.layout.height_twips, 16_766);
    assert_eq!(tree.layout.margins_twips[3], 1_134);
}

#[test]
fn the_text_extraction_reads_word6() {
    let extracted =
        extract_text(&sample().build(), Some("verbale.doc"), &Limits::default()).unwrap();
    assert!(extracted.text.contains("Titolo del verbale"));
    assert!(extracted.text.contains("Il comandante \u{e8} in sede."));
    assert!(extracted.text.contains("Nome\tGrado"));
}

#[test]
fn modern_modifiers_translate_codes_and_stop_at_an_unknown_one() {
    assert_eq!(modern_modifiers(&[85, 1]), vec![0x35, 0x08, 1]);
    assert_eq!(modern_modifiers(&[99, 24, 0]), vec![0x43, 0x4A, 24, 0]);
    assert_eq!(
        modern_modifiers(&[20, 240, 0, 1, 0]),
        vec![0x12, 0x64, 240, 0, 1, 0]
    );
    assert_eq!(
        modern_modifiers(&[97, 0x10, 0x04, 85, 1]),
        vec![0x35, 0x08, 1]
    );
    assert_eq!(modern_modifiers(&[3, 2, 9, 9, 85, 1]), vec![0x35, 0x08, 1]);
    assert_eq!(modern_modifiers(&[83, 85, 0]), vec![0x35, 0x08, 0]);
    assert_eq!(modern_modifiers(&[250, 85, 1]), Vec::<u8>::new());
    assert_eq!(modern_modifiers(&[99, 24]), Vec::<u8>::new());
    assert_eq!(
        modern_modifiers(&[68, 4, 0x10, 0, 0, 0]),
        vec![0x03, 0x6A, 0x10, 0, 0, 0]
    );
}

#[test]
fn a_row_layout_is_widened_to_word97_cells() {
    let modern = modern_modifiers(&row_modifiers()[4..]);
    let modifiers: Vec<_> = super::super::sprm::modifiers(&modern).collect();
    assert_eq!(modifiers.len(), 1);
    let (code, operand) = modifiers[0];
    assert_eq!(code, 0xD608);
    assert_eq!(operand[0], 2);
    assert_eq!(operand.len(), 1 + 3 * 2 + 2 * 20);
    let cell = &operand[7..27];
    assert_eq!(&cell[4..6], &[8, 1]);
    let without_line = &operand[27..47];
    assert_eq!(&without_line[4..20], &[0; 16]);
}

#[test]
fn a_style_sheet_is_rewritten_with_utf16_names() {
    let sheet = super::super::styles::StyleSheet::read(&style_sheet(&style_sheet_bytes()));
    let heading = sheet.style(1).unwrap();
    assert_eq!(heading.name, "heading 1");
    assert!(sheet.style(2).is_none());
    assert_eq!(sheet.style(10).unwrap().name, "Default Paragraph Font");
    assert_eq!(style_sheet(&[]), Vec::<u8>::new());
    assert_eq!(style_sheet(&[40, 0, 1]), Vec::<u8>::new());
}

#[test]
fn font_names_are_8_bit() {
    assert_eq!(
        font_names(&font_table_bytes()),
        vec!["Times New Roman", "Arial"]
    );
    assert_eq!(font_names(&[]), Vec::<String>::new());
    assert_eq!(font_names(&[40, 0, 30]), Vec::<String>::new());
}

#[test]
fn sections_read_the_page_through_word97_codes() {
    let stream = sample().word_document();
    let fib = Word6Fib::read(&stream).unwrap();
    let sections = sections(&stream, &fib);
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].layout.width_twips, 11_906);
    assert_eq!(sections[0].end, sample().text().len());
}

/// Writes the sample for a comparison with `LibreOffice`, which still reads Word 6:
/// `cargo test -p rely-office-core write_word6_sample -- --ignored`.
#[test]
#[ignore = "writes a file for a manual comparison"]
fn write_word6_sample() {
    let path = std::env::var("WORD6_SAMPLE").unwrap_or_else(|_| "word6-sample.doc".to_owned());
    std::fs::write(path, sample().build()).unwrap();
}
