// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

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

/// A Word 6 document as Word 6 writes it: the File Information Block, the 8-bit text of its
/// stories from [`TEXT_START`], the pictures and drawing objects after it, one page of
/// character runs, one page of paragraph runs and the tables.
struct Word6Builder {
    paragraphs: Vec<Word6Paragraph>,
    /// The comment story and the text box story.
    comment_story: Vec<Word6Paragraph>,
    box_story: Vec<Word6Paragraph>,
    /// Character runs by their range of characters, with their modifiers and the blob a picture
    /// location names.
    characters: Vec<(Range<usize>, Vec<u8>, Option<usize>)>,
    /// Bytes stored after the text: pictures and drawing objects.
    blobs: Vec<Vec<u8>>,
    /// The drawing objects of the main text: their anchor position, blob and text boxes.
    drawings: Vec<(usize, usize, u16)>,
    /// The comments: their reference position, initials and author.
    comments: Vec<(usize, String, u16)>,
    owners: Vec<String>,
    flags: u16,
    complex: bool,
}

impl Word6Builder {
    fn new() -> Self {
        Self {
            paragraphs: Vec::new(),
            comment_story: Vec::new(),
            box_story: Vec::new(),
            characters: Vec::new(),
            blobs: Vec::new(),
            drawings: Vec::new(),
            comments: Vec::new(),
            owners: Vec::new(),
            flags: 0,
            complex: false,
        }
    }

    fn encoded(text: &str, style: u16, grpprl: &[u8]) -> Word6Paragraph {
        let (encoded, _, _) = WINDOWS_1252.encode(text);
        Word6Paragraph {
            text: encoded.into_owned(),
            style,
            grpprl: grpprl.to_vec(),
        }
    }

    fn paragraph(mut self, text: &str, style: u16, grpprl: &[u8]) -> Self {
        self.paragraphs.push(Self::encoded(text, style, grpprl));
        self
    }

    fn character(mut self, range: Range<usize>, grpprl: &[u8]) -> Self {
        self.characters.push((range, grpprl.to_vec(), None));
        self
    }

    /// A picture at `position` of the main text, its `PIC` stored after the text.
    fn picture(mut self, position: usize, pic: Vec<u8>) -> Self {
        self.blobs.push(pic);
        self.characters.push((
            position..position + 1,
            vec![117, 1],
            Some(self.blobs.len() - 1),
        ));
        self
    }

    /// A drawing object anchored at `position` of the main text, holding `boxes` text boxes.
    fn drawing(mut self, position: usize, object: Vec<u8>, boxes: u16) -> Self {
        self.blobs.push(object);
        self.drawings.push((position, self.blobs.len() - 1, boxes));
        self.characters
            .push((position..position + 1, vec![117, 1], None));
        self
    }

    fn text_box(mut self, text: &str) -> Self {
        self.box_story.push(Self::encoded(text, 0, &[]));
        self
    }

    /// A comment whose reference stands at `position`, by `owner`, with its text.
    fn comment(mut self, position: usize, initials: &str, owner: &str, text: &str) -> Self {
        let owner_index = self
            .owners
            .iter()
            .position(|known| known == owner)
            .unwrap_or_else(|| {
                self.owners.push(owner.to_owned());
                self.owners.len() - 1
            });
        self.comments.push((
            position,
            initials.to_owned(),
            u16::try_from(owner_index).unwrap(),
        ));
        self.comment_story.push(Self::encoded(text, 0, &[]));
        self.characters
            .push((position..position + 1, vec![117, 1], None));
        self
    }

    fn story_text(story: &[Word6Paragraph]) -> Vec<u8> {
        story
            .iter()
            .flat_map(|paragraph| paragraph.text.clone())
            .collect()
    }

    fn text(&self) -> Vec<u8> {
        Self::story_text(&self.paragraphs)
    }

    fn has_stories(&self) -> bool {
        !self.comment_story.is_empty() || !self.box_story.is_empty()
    }

    /// The length, style and modifiers of every paragraph of every story, the final mark after
    /// them included.
    fn all_paragraphs(&self) -> Vec<(usize, u16, Vec<u8>)> {
        let mut all: Vec<(usize, u16, Vec<u8>)> = self
            .paragraphs
            .iter()
            .chain(&self.comment_story)
            .chain(&self.box_story)
            .map(|paragraph| {
                (
                    paragraph.text.len(),
                    paragraph.style,
                    paragraph.grpprl.clone(),
                )
            })
            .collect();
        if self.has_stories() {
            all.push((1, 0, Vec::new()));
        }
        all
    }

    fn character_page(&self, total: usize, blob_offsets: &[usize]) -> Vec<u8> {
        let mut runs: Vec<(usize, usize, Vec<u8>)> = Vec::new();
        let mut ranges = self.characters.clone();
        ranges.sort_by_key(|(range, _, _)| range.start);
        let mut position = 0;
        for (range, mut grpprl, blob) in ranges {
            if range.start > position {
                runs.push((position, range.start, Vec::new()));
            }
            if let Some(blob) = blob {
                grpprl.extend_from_slice(&[68, 4]);
                grpprl.extend_from_slice(&u32::try_from(blob_offsets[blob]).unwrap().to_le_bytes());
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
        let paragraphs = self.all_paragraphs();
        let count = paragraphs.len();
        let mut free = PAGE_SIZE - 1;
        let mut start = 0;
        for (index, (length, style, grpprl)) in paragraphs.iter().enumerate() {
            let end = start + length;
            page[index * 4..index * 4 + 4].copy_from_slice(&stream_offset(start).to_le_bytes());
            page[index * 4 + 4..index * 4 + 8].copy_from_slice(&stream_offset(end).to_le_bytes());
            start = end;
            let mut properties = style.to_le_bytes().to_vec();
            properties.extend_from_slice(grpprl);
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

    #[expect(
        clippy::too_many_lines,
        reason = "the stream is laid out in the order the offsets need"
    )]
    fn word_document(&self) -> Vec<u8> {
        use super::super::fib::pair;
        let main = self.text().len();
        let comments = Self::story_text(&self.comment_story);
        let boxes = Self::story_text(&self.box_story);
        let mut text = self.text();
        text.extend_from_slice(&comments);
        text.extend_from_slice(&boxes);
        if self.has_stories() {
            text.push(b'\r');
        }
        let total = text.len();
        let mut stream = vec![0_u8; TEXT_START];
        stream.extend_from_slice(&text);
        let mut blob_offsets = Vec::new();
        for blob in &self.blobs {
            if stream.len() % 2 == 1 {
                stream.push(0);
            }
            blob_offsets.push(stream.len());
            stream.extend_from_slice(blob);
        }
        stream.resize(stream.len().next_multiple_of(PAGE_SIZE), 0);
        let character_page = stream.len() / PAGE_SIZE;
        stream.extend(self.character_page(total, &blob_offsets));
        let paragraph_page = stream.len() / PAGE_SIZE;
        stream.extend(self.paragraph_page());

        let mut pairs: Vec<(usize, usize, Vec<u8>)> = vec![
            (pair::STYLES, 0, style_sheet_bytes()),
            (pair::FONTS, 0, font_table_bytes()),
            (pair::CHARACTER_PAGES, 0, bin_table(total, character_page)),
            (pair::PARAGRAPH_PAGES, 0, bin_table(total, paragraph_page)),
            (pair::DOCUMENT_PROPERTIES, 0, dop_bytes()),
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
        sections.extend_from_slice(&u32::try_from(main).unwrap().to_le_bytes());
        sections.extend_from_slice(&0_u16.to_le_bytes());
        sections.extend_from_slice(&u32::try_from(sepx_offset).unwrap().to_le_bytes());
        sections.extend_from_slice(&0_u16.to_le_bytes());
        sections.extend_from_slice(&u32::MAX.to_le_bytes());
        pairs.push((pair::SECTIONS, 0, sections));
        let cp = |value: usize| u32::try_from(value).unwrap().to_le_bytes();
        if !self.comments.is_empty() {
            let mut references = Vec::new();
            for (position, _, _) in &self.comments {
                references.extend_from_slice(&cp(*position));
            }
            references.extend_from_slice(&cp(main));
            for (_, initials, owner) in &self.comments {
                let mut record = vec![0_u8; 20];
                record[0] = u8::try_from(initials.len()).unwrap();
                record[1..=initials.len()].copy_from_slice(initials.as_bytes());
                record[10..12].copy_from_slice(&owner.to_le_bytes());
                record[16..20].copy_from_slice(&(-1_i32).to_le_bytes());
                references.extend(record);
            }
            pairs.push((pair::COMMENT_REFERENCES, 0, references));
            let mut texts = Vec::new();
            let mut start = 0;
            for paragraph in &self.comment_story {
                texts.extend_from_slice(&cp(start));
                start += paragraph.text.len();
            }
            texts.extend_from_slice(&cp(start));
            pairs.push((pair::COMMENT_TEXTS, 0, texts));
            let mut owners = Vec::new();
            for owner in &self.owners {
                owners.push(u8::try_from(owner.len()).unwrap());
                owners.extend_from_slice(owner.as_bytes());
            }
            pairs.push((pair::COMMENT_OWNERS, 0, owners));
        }
        if !self.drawings.is_empty() {
            let mut objects = Vec::new();
            for (position, _, _) in &self.drawings {
                objects.extend_from_slice(&cp(*position));
            }
            objects.extend_from_slice(&cp(main));
            for (_, blob, boxes) in &self.drawings {
                objects.extend_from_slice(&cp(blob_offsets[*blob]));
                objects.extend_from_slice(&boxes.to_le_bytes());
            }
            pairs.push((DRAWN_OBJECTS_PAIR, 0, objects));
        }
        if !self.box_story.is_empty() {
            let mut texts = Vec::new();
            let mut start = 0;
            for paragraph in &self.box_story {
                texts.extend_from_slice(&cp(start));
                start += paragraph.text.len();
            }
            texts.extend_from_slice(&cp(start));
            // Word closes the plex with a box of the story's last mark, which it does not show.
            texts.extend_from_slice(&cp(start + 1));
            pairs.push((TEXT_BOXES_PAIR, 0, texts));
        }
        if self.complex {
            let mut clx = vec![2_u8];
            let mut table = Vec::new();
            let half = total / 2;
            for value in [0, half, total] {
                table.extend_from_slice(&cp(value));
            }
            for start in [0, half] {
                table.extend_from_slice(&0_u16.to_le_bytes());
                table.extend_from_slice(&cp(TEXT_START + start));
                table.extend_from_slice(&0_u16.to_le_bytes());
            }
            clx.extend_from_slice(&cp(table.len()));
            clx.extend_from_slice(&table);
            pairs.push((pair::PIECE_TABLE, 0, clx));
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
        put32(&mut stream, STORIES_OFFSET, main);
        put32(&mut stream, STORIES_OFFSET + 16, comments.len());
        put32(&mut stream, STORIES_OFFSET + 24, boxes.len());
        for (index, offset, bytes) in &pairs {
            put32(&mut stream, pair_offset(*index), *offset);
            put32(&mut stream, pair_offset(*index) + 4, bytes.len());
        }
        put16(&mut stream, 0x18A, u16::try_from(character_page).unwrap());
        put16(&mut stream, 0x18C, u16::try_from(paragraph_page).unwrap());
        put16(&mut stream, 0x18E, 1);
        put16(&mut stream, 0x190, 1);
        stream
    }
}

const DRAWN_OBJECTS_PAIR: usize = 38;
const TEXT_BOXES_PAIR: usize = 56;

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

/// A metafile of no placeable header that fills a 100 by 50 window with red.
fn wmf() -> Vec<u8> {
    let records: Vec<Vec<u16>> = vec![
        vec![0x020B, 0, 0],
        vec![0x020C, 50, 100],
        vec![0x02FC, 0, 0x00FF, 0, 0],
        vec![0x012D, 0],
        vec![0x041B, 50, 100, 0, 0],
        vec![0x0000],
    ];
    let mut body = Vec::new();
    let mut largest = 0;
    for record in &records {
        let words = u32::try_from(record.len() + 2).unwrap();
        largest = largest.max(words);
        body.extend_from_slice(&words.to_le_bytes());
        for word in record {
            body.extend_from_slice(&word.to_le_bytes());
        }
    }
    let mut metafile = Vec::new();
    for word in [1_u16, 9, 0x0300] {
        metafile.extend_from_slice(&word.to_le_bytes());
    }
    metafile.extend_from_slice(&u32::try_from(9 + body.len() / 2).unwrap().to_le_bytes());
    metafile.extend_from_slice(&1_u16.to_le_bytes());
    metafile.extend_from_slice(&largest.to_le_bytes());
    metafile.extend_from_slice(&0_u16.to_le_bytes());
    metafile.extend(body);
    metafile
}

/// A Word 6 `PIC` of `mode` holding `data`, set at 1,500 by 750 twips.
fn pic(mode: i16, data: &[u8]) -> Vec<u8> {
    const HEADER: usize = 0x3A;
    let mut pic = vec![0_u8; HEADER];
    pic[..4].copy_from_slice(&u32::try_from(HEADER + data.len()).unwrap().to_le_bytes());
    pic[4..6].copy_from_slice(&u16::try_from(HEADER).unwrap().to_le_bytes());
    pic[6..8].copy_from_slice(&mode.to_le_bytes());
    pic[8..10].copy_from_slice(&2_646_u16.to_le_bytes());
    pic[10..12].copy_from_slice(&1_323_u16.to_le_bytes());
    for (at, value) in [(28, 1_500_u16), (30, 750), (32, 1_000), (34, 1_000)] {
        pic[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }
    pic.extend_from_slice(data);
    pic
}

/// A heading, a paragraph with a picture and a comment, and one with four drawing objects: a
/// group of a rectangle, an ellipse and a line, a text box, an arc and a triangle.
fn rich_sample() -> Word6Builder {
    use super::drawings::tests::{
        BLUE, GREEN, RED, YELLOW, arc_body, drawing_object, group, line_body, polyline_body,
        primitive, shape_body,
    };
    let heading = "Disegni e note\r";
    let pictured = "Foto del mezzo \u{1} da rivedere\u{5}.\r";
    let drawn = "Schema:\u{8}\u{8}\u{8}\u{8}\r";
    let picture_at = heading.len() + 15;
    let comment_at = heading.len() + pictured.find('\u{5}').unwrap();
    let drawings_at = heading.len() + pictured.len() + 7;
    let shapes = drawing_object(
        2,
        2,
        &[group(
            [0, 300, 0, 0],
            &[
                primitive(3, [0, 0, 1440, 720], &shape_body(RED, BLUE, 0, false)),
                primitive(4, [1800, 0, 1440, 720], &shape_body(RED, YELLOW, 0, false)),
                primitive(1, [0, 900, 0, 0], &line_body((0, 0), (3240, 0), RED)),
            ],
        )],
    );
    let text_box = drawing_object(
        2,
        2,
        &[primitive(
            2,
            [3600, 300, 2880, 1080],
            &shape_body(BLUE, YELLOW, 0, true),
        )],
    );
    let arc = drawing_object(
        2,
        2,
        &[primitive(
            5,
            [0, 1500, 1440, 1000],
            &arc_body(GREEN, false, true),
        )],
    );
    let triangle = drawing_object(
        2,
        2,
        &[primitive(
            6,
            [2000, 1500, 1440, 1000],
            &polyline_body(&[(0, 1000), (720, 0), (1440, 1000)], true, GREEN),
        )],
    );
    Word6Builder::new()
        .paragraph(heading, 1, &[])
        .paragraph(pictured, 0, &[])
        .paragraph(drawn, 0, &[22, 0x60, 0x09])
        .paragraph("Fine.\r", 0, &[])
        .picture(picture_at, pic(8, &wmf()))
        .comment(
            comment_at,
            "MR",
            "Mario Rossi",
            "\u{5}Controllare la targa.\r",
        )
        .drawing(drawings_at, shapes, 0)
        .drawing(drawings_at + 1, text_box, 1)
        .drawing(drawings_at + 2, arc, 0)
        .drawing(drawings_at + 3, triangle, 0)
        .text_box("Casella di testo\r")
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
    std::fs::write(&path, sample().build()).unwrap();
    std::fs::write(path.replace(".doc", "-disegni.doc"), rich_sample().build()).unwrap();
}

#[test]
fn a_comment_is_read_with_its_author_and_initials() {
    let tree = read_document_tree(&rich_sample().build(), &Limits::default()).unwrap();
    assert_eq!(tree.report.comments, 1);
    assert_eq!(tree.comments.len(), 1);
    let comment = &tree.comments[0];
    assert_eq!(comment.author, "Mario Rossi");
    assert_eq!(comment.initials.as_deref(), Some("MR"));
    let Block::Paragraph { content, .. } = &comment.blocks[0] else {
        panic!("{:?}", comment.blocks);
    };
    assert_eq!(plain_text(content), "Controllare la targa.");
    let Block::Paragraph { content, .. } = &tree.blocks[1] else {
        panic!("{:?}", tree.blocks[1]);
    };
    assert!(
        content
            .iter()
            .any(|inline| matches!(inline, Inline::CommentAnchor { comment: 0 }))
    );
}

#[test]
fn a_metafile_picture_is_drawn_as_svg_at_its_size() {
    let tree = read_document_tree(&rich_sample().build(), &Limits::default()).unwrap();
    let Block::Paragraph { content, .. } = &tree.blocks[1] else {
        panic!("{:?}", tree.blocks[1]);
    };
    let Some(Inline::Image {
        media,
        width,
        height,
        anchor: None,
        ..
    }) = content
        .iter()
        .find(|inline| matches!(inline, Inline::Image { .. }))
    else {
        panic!("{content:?}");
    };
    assert_eq!((*width, *height), (Some(100), Some(50)));
    assert_eq!(tree.media[*media].content_type, "image/svg+xml");
}

#[test]
fn pictures_of_each_mode_are_kept_as_their_files() {
    let stream = [vec![0_u8; 8], pic(99, &bitmap())].concat();
    let (bytes, width, height) = picture(&stream, 8).unwrap();
    let bytes = bytes.unwrap();
    assert!(bytes.starts_with(b"BM"));
    assert_eq!(u32_at(&bytes, 10), Some(14 + 40 + 8));
    assert_eq!((width, height), (1_500, 750));
    let stream = [vec![0_u8; 8], pic(98, b"II*\0tiff")].concat();
    assert_eq!(
        picture(&stream, 8).unwrap().0.as_deref(),
        Some(&b"II*\0tiff"[..])
    );
    let stream = [vec![0_u8; 8], pic(94, b"C:\\foto.bmp")].concat();
    assert_eq!(picture(&stream, 8).unwrap().0, None);
    assert_eq!(picture(&stream, 900), None);
    assert_eq!(bitmap_file(b"BMgia"), Some(b"BMgia".to_vec()));
    assert_eq!(bitmap_file(&[0; 8]), None);
    let mut core = vec![0_u8; 12];
    core[..4].copy_from_slice(&12_u32.to_le_bytes());
    core[10..12].copy_from_slice(&24_u16.to_le_bytes());
    assert_eq!(u32_at(&bitmap_file(&core).unwrap(), 10), Some(14 + 12));
}

/// A one-bit bitmap of one pixel with its two colours.
fn bitmap() -> Vec<u8> {
    let mut dib = vec![0_u8; 40];
    dib[..4].copy_from_slice(&40_u32.to_le_bytes());
    dib[4..8].copy_from_slice(&1_i32.to_le_bytes());
    dib[8..12].copy_from_slice(&1_i32.to_le_bytes());
    dib[12..14].copy_from_slice(&1_u16.to_le_bytes());
    dib[14..16].copy_from_slice(&1_u16.to_le_bytes());
    dib.extend_from_slice(&[0, 0, 0, 0, 255, 255, 255, 0]);
    dib.extend_from_slice(&[0x80, 0, 0, 0]);
    dib
}

#[test]
fn drawing_objects_are_pictures_and_text_boxes_where_they_stand() {
    let tree = read_document_tree(&rich_sample().build(), &Limits::default()).unwrap();
    let Block::Paragraph { content, .. } = &tree.blocks[2] else {
        panic!("{:?}", tree.blocks[2]);
    };
    let anchored: Vec<_> = content
        .iter()
        .filter_map(|inline| match inline {
            Inline::Image {
                media,
                anchor: Some(anchor),
                ..
            } => Some((*media, anchor)),
            _ => None,
        })
        .collect();
    assert_eq!(
        anchored.len(),
        3,
        "the group, the arc and the triangle: {content:?}"
    );
    let (media, anchor) = &anchored[0];
    assert_eq!(tree.media[*media].content_type, "image/svg+xml");
    assert_eq!(anchor.horizontal.offset_emu, 0);
    assert_eq!(anchor.vertical.offset_emu, 300 * 635);
    assert_eq!(anchor.width_emu, 3_240 * 635);
    assert_eq!(anchor.horizontal.base, crate::document::AnchorBase::Column);
    assert_eq!(anchor.vertical.base, crate::document::AnchorBase::Paragraph);
    let framed: Vec<&Block> = tree
        .blocks
        .iter()
        .filter(|block| matches!(block, Block::Paragraph { look, .. } if look.frame.is_some()))
        .collect();
    assert_eq!(framed.len(), 1);
    let Block::Paragraph { content, look, .. } = framed[0] else {
        unreachable!();
    };
    assert_eq!(plain_text(content), "Casella di testo");
    let frame = look.frame.as_ref().unwrap();
    assert_eq!(frame.fill.as_deref(), Some("FFFF00"));
    assert_eq!(frame.anchor.horizontal.offset_emu, 3_600 * 635);
    assert_eq!(tree.report.text_boxes, 1);
}
