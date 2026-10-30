// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! A Word 97-2003 document read into the [`DocumentTree`] a docx gives: paragraphs with their
//! styles and direct formatting, headings, lists, tables, pictures in the line and floating,
//! text boxes, notes, fields, sections with their pages, headers and footers.

mod table;

use std::collections::HashMap;
use std::ops::Range;

use super::comments::{CommentTables, StoredComment, date_of, read_comments, reviewers};
use super::fib::{Fib, Stories, pair};
use super::fkp::{
    CharacterRun, ParagraphRun, character_runs, paragraph_runs, plex, positions, run_at,
};
use super::fonts::font_names;
use super::lists::Lists;
use super::pictures::{
    Drawings, InlinePicture, ShapeAnchor, ShapeContent, inline_picture, shape_anchors,
};
use super::properties::{Character, Paragraph, Row, colour_reference};
use super::sections::{SectionLayout, read_sections};
use super::styles::StyleSheet;
use super::{decode_piece, read_piece_table, unreadable, word6};
use crate::compound::{open_compound_file, read_stream, u16_at, u32_at};
use crate::document::objects::{self, EmbeddedObject};
use crate::document::{
    Alignment, Anchor, AnchorAlign, AnchorBase, AnchorPosition, Block, Borders, Comment,
    DocumentTree, HeadersFooters, ImportReport, Inline, LabelSuffix, LineSpacing, ListLabel, Mark,
    Media, Note, NoteKind, PageField, PageLayout, ParagraphLook, RunLook, Section, TextFrame,
    TrackedChanges, TreeOptions, Wrap, bullet_character, format_number, push_block, set_frame,
    sniff_image_type,
};
use crate::geometry::{
    DrawnShape, flip_paths, preset_geometry, preset_geometry_with_values, rectangle_paths,
    shapes_svg,
};
use crate::office_art::{drawingml_adjustments, preset_of_shape_type, upside_down_from_preset};
use crate::{Limits, OfficeError};
use table::TableBuilder;

/// A footnote or endnote reference (`FRD`) is two bytes; a text box entry (`FTXBXS`) 22.
const NOTE_REFERENCE_SIZE: usize = 2;
const TEXT_BOX_SIZE: usize = 22;
/// Where an `FTXBXS` names the shape it belongs to; it is reused when its flag at 8 is set.
const TEXT_BOX_SHAPE_OFFSET: usize = 14;
/// The header plex begins with six entries for the note separators.
const SEPARATOR_ENTRIES: usize = 6;
const HEADER_ENTRIES: usize = 6;
/// Twips to English Metric Units, and to CSS pixels.
const EMU_PER_TWIP: i64 = 635;
const TWIPS_PER_PIXEL: u32 = 15;
/// Word 97 sets text it is told nothing about at ten points.
const DEFAULT_SIZE: u16 = 20;
const DEFAULT_TAB_TWIPS: u32 = 720;
/// The headings a style can be: `heading 1` to `heading 9` are built-in styles 1 to 9.
const MAX_HEADING: u16 = 9;
/// How deep tables nest before the deeper ones are read into the cells of the deepest.
const MAX_TABLE_DEPTH: usize = 8;
/// No field nests deeper than this.
const MAX_FIELD_DEPTH: usize = 32;

/// Read a .doc as a tree of blocks, with its images and an import report.
///
/// # Errors
///
/// [`OfficeError::TooLarge`] past the input or stream limits, [`OfficeError::Encrypted`] for a
/// protected document, [`OfficeError::DamagedContainer`] for a broken compound file, and
/// [`OfficeError::UnreadableDocument`] when the streams do not hold a Word 97-2003 document.
#[expect(
    clippy::too_many_lines,
    reason = "every table of the file the tree reads is located here, in one place"
)]
pub(crate) fn read_doc_tree(
    bytes: &[u8],
    limits: &Limits,
    options: TreeOptions,
) -> Result<DocumentTree, OfficeError> {
    if bytes.len() as u64 > limits.max_input_bytes {
        return Err(OfficeError::TooLarge {
            what: "input".to_owned(),
            limit: limits.max_input_bytes,
        });
    }
    let mut compound_file = open_compound_file(bytes)?;
    let word_document = read_stream(&mut compound_file, "/WordDocument", limits.max_member_bytes)?
        .ok_or_else(|| unreadable("the file has no WordDocument stream"))?;
    if word6::is_word6(&word_document) {
        return read_word6_tree(&word_document, limits, options);
    }
    let fib = Fib::read(&word_document)?;
    if fib.encrypted {
        return Err(OfficeError::Encrypted);
    }
    let table = read_stream(
        &mut compound_file,
        fib.table_stream,
        limits.max_member_bytes,
    )?
    .ok_or_else(|| unreadable("the file has no table stream"))?;
    let data =
        read_stream(&mut compound_file, "/Data", limits.max_member_bytes)?.unwrap_or_default();
    let clx = fib
        .slice(&table, pair::PIECE_TABLE)
        .ok_or_else(|| unreadable("the piece table lies outside the table stream"))?;

    let total = fib.stories.total();
    let mut text = Vec::new();
    let mut offsets = Vec::new();
    for piece in read_piece_table(clx)? {
        if piece.start >= total {
            break;
        }
        let count = piece.end.min(total) - piece.start;
        let width = if piece.compressed { 1 } else { 2 };
        for (index, character) in decode_piece(&word_document, &piece, count)?
            .into_iter()
            .enumerate()
        {
            text.push(character);
            offsets.push(u32::try_from(piece.offset + index * width).unwrap_or(u32::MAX));
        }
    }
    let structure = |index: usize| fib.slice(&table, index).unwrap_or_default();
    let styles = StyleSheet::read(structure(pair::STYLES));
    let fonts = font_names(structure(pair::FONTS));
    let lists = Lists::read(
        &table,
        fib.pair(pair::LISTS).map(|(offset, _)| offset),
        fib.pair(pair::LIST_OVERRIDES).map(|(offset, _)| offset),
    );
    let document_properties = structure(pair::DOCUMENT_PROPERTIES);
    let converter = Converter {
        text: &text,
        offsets: &offsets,
        characters: character_runs(&word_document, structure(pair::CHARACTER_PAGES)),
        paragraphs: paragraph_runs(&word_document, structure(pair::PARAGRAPH_PAGES), &data),
        styles,
        fonts,
        lists,
        data: &data,
        word_document: &word_document,
        drawings: Drawings::read(structure(pair::DRAWINGS), &word_document),
        anchors: shape_anchors(structure(pair::MAIN_SHAPES)),
        footnotes: references(
            structure(pair::FOOTNOTE_REFERENCES),
            structure(pair::FOOTNOTE_TEXTS),
            fib.stories.footnotes_start(),
        ),
        endnotes: references(
            structure(pair::ENDNOTE_REFERENCES),
            structure(pair::ENDNOTE_TEXTS),
            fib.stories.endnotes_start(),
        ),
        text_boxes: text_box_ranges(structure(pair::TEXT_BOXES), fib.stories.text_boxes_start()),
        stories: fib.stories,
        limits,
        report: ImportReport {
            comments: plex(structure(pair::COMMENT_REFERENCES), 30)
                .map_or(0, |(_, entries)| entries.len()),
            ..ImportReport::default()
        },
        stored_comments: read_comments(
            &CommentTables {
                references: structure(pair::COMMENT_REFERENCES),
                texts: structure(pair::COMMENT_TEXTS),
                owners: structure(pair::COMMENT_OWNERS),
                bookmark_names: structure(pair::COMMENT_BOOKMARK_NAMES),
                bookmark_firsts: structure(pair::COMMENT_BOOKMARK_FIRSTS),
                bookmark_lasts: structure(pair::COMMENT_BOOKMARK_LASTS),
            },
            fib.stories.comments_start(),
        ),
        comments: Vec::new(),
        reviewers: reviewers(structure(pair::REVIEWERS)),
        tracked_changes: options.tracked_changes,
        media: Vec::new(),
        media_bytes: 0,
        pictures: HashMap::new(),
        notes: Vec::new(),
        built_notes: HashMap::new(),
        floating: Vec::new(),
        next_frame: 0,
        numbers: HashMap::new(),
        remaining: limits.max_text_chars,
        style_properties: HashMap::new(),
        page_break_pending: false,
        in_note: false,
        object_pool: object_pool(&mut compound_file, limits),
        objects: Vec::new(),
        legacy_pictures: false,
        word6_drawings: HashMap::new(),
    };
    let mut converter = converter;
    converter.read_comments();
    Ok(converter.convert(
        &read_sections(&word_document, structure(pair::SECTIONS)),
        &positions(structure(pair::HEADERS)),
        document_properties,
    ))
}

/// Read a Word 6 or Word 95 document: its structures rewritten as Word 97 stores them, then the
/// same conversion, with the pictures and the drawing objects Word 6 stores its own way.
fn read_word6_tree(
    word_document: &[u8],
    limits: &Limits,
    options: TreeOptions,
) -> Result<DocumentTree, OfficeError> {
    let fib = word6::Word6Fib::read(word_document)?;
    if fib.encrypted {
        return Err(OfficeError::Encrypted);
    }
    let (text, offsets) = word6::text(word_document, &fib);
    let structure = |index: usize| fib.structure(word_document, index);
    let comment_references = word6::comment_references(structure(pair::COMMENT_REFERENCES));
    let comment_owners = word6::comment_owners(structure(pair::COMMENT_OWNERS));
    let mut converter = Converter {
        text: &text,
        offsets: &offsets,
        characters: word6::character_runs(word_document, &fib),
        paragraphs: word6::paragraph_runs(word_document, &fib),
        styles: StyleSheet::read(&word6::style_sheet(structure(pair::STYLES))),
        fonts: word6::font_names(structure(pair::FONTS)),
        lists: Lists::read(&[], None, None),
        data: &[],
        word_document,
        drawings: Drawings::read(&[], &[]),
        anchors: HashMap::new(),
        footnotes: references(
            structure(pair::FOOTNOTE_REFERENCES),
            structure(pair::FOOTNOTE_TEXTS),
            fib.stories.footnotes_start(),
        ),
        endnotes: references(
            structure(pair::ENDNOTE_REFERENCES),
            structure(pair::ENDNOTE_TEXTS),
            fib.stories.endnotes_start(),
        ),
        text_boxes: HashMap::new(),
        stories: fib.stories,
        limits,
        report: ImportReport {
            comments: plex(&comment_references, 30).map_or(0, |(_, entries)| entries.len()),
            ..ImportReport::default()
        },
        stored_comments: read_comments(
            &CommentTables {
                references: &comment_references,
                texts: structure(pair::COMMENT_TEXTS),
                owners: &comment_owners,
                bookmark_names: &[],
                bookmark_firsts: &[],
                bookmark_lasts: &[],
            },
            fib.stories.comments_start(),
        ),
        comments: Vec::new(),
        reviewers: Vec::new(),
        tracked_changes: options.tracked_changes,
        media: Vec::new(),
        media_bytes: 0,
        pictures: HashMap::new(),
        notes: Vec::new(),
        built_notes: HashMap::new(),
        floating: Vec::new(),
        next_frame: 0,
        numbers: HashMap::new(),
        remaining: limits.max_text_chars,
        style_properties: HashMap::new(),
        page_break_pending: false,
        in_note: false,
        object_pool: HashMap::new(),
        objects: Vec::new(),
        legacy_pictures: true,
        word6_drawings: word6::drawings(word_document, &fib),
    };
    converter.read_comments();
    Ok(converter.convert(
        &word6::sections(word_document, &fib),
        &positions(structure(pair::HEADERS)),
        structure(pair::DOCUMENT_PROPERTIES),
    ))
}

/// Every storage of the `ObjectPool`, an OLE object each, as a compound file of its own by the
/// number in its name; as many as the document may have pictures.
fn object_pool(
    compound_file: &mut crate::compound::CompoundFile<'_>,
    limits: &Limits,
) -> HashMap<u32, Vec<u8>> {
    let Ok(entries) = compound_file.read_storage("/ObjectPool") else {
        return HashMap::new();
    };
    let numbers: Vec<u32> = entries
        .filter(cfb::Entry::is_storage)
        .filter_map(|entry| entry.name().strip_prefix('_')?.parse().ok())
        .take(limits.max_images)
        .collect();
    numbers
        .into_iter()
        .filter_map(|number| {
            let bytes = objects::storage_as_compound_file(
                compound_file,
                &format!("/ObjectPool/_{number}"),
                limits.max_member_bytes,
            )?;
            Some((number, bytes))
        })
        .collect()
}

/// The notes a story refers to: each reference's character position and the range of its text.
fn references(references: &[u8], texts: &[u8], base: usize) -> HashMap<usize, Range<usize>> {
    let Some((marks, _)) = plex(references, NOTE_REFERENCE_SIZE) else {
        return HashMap::new();
    };
    let texts = positions(texts);
    marks
        .iter()
        .take(marks.len().saturating_sub(1))
        .enumerate()
        .filter_map(|(index, mark)| {
            let start = base + *texts.get(index)? as usize;
            let end = base + *texts.get(index + 1)? as usize;
            Some((*mark as usize, start..end.max(start)))
        })
        .collect()
}

/// The text of each text box, by the identifier of the shape that shows it (`FTXBXS.lid`).
fn text_box_ranges(plex_bytes: &[u8], base: usize) -> HashMap<u32, Range<usize>> {
    let Some((starts, entries)) = plex(plex_bytes, TEXT_BOX_SIZE) else {
        return HashMap::new();
    };
    entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| u16_at(entry, 8) == Some(0))
        .filter_map(|(index, entry)| {
            let shape = u32_at(entry, TEXT_BOX_SHAPE_OFFSET)?;
            let start = base + *starts.get(index)? as usize;
            let end = base + *starts.get(index + 1)? as usize;
            Some((shape, start..end.max(start)))
        })
        .collect()
}

/// A field being read: its instruction until the separator, then what its result is.
#[derive(Debug, Default)]
struct Field {
    instruction: String,
    in_result: bool,
    link: Option<String>,
    page: Option<PageField>,
    /// The storage of the object pool an embedding field holds, by its number.
    object: Option<u32>,
}

/// What a run of text looks like, to group characters into runs.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RunKey {
    marks: Vec<Mark>,
    look: RunLook,
}

struct Converter<'a> {
    text: &'a [char],
    offsets: &'a [u32],
    characters: Vec<CharacterRun>,
    paragraphs: Vec<ParagraphRun>,
    styles: StyleSheet,
    fonts: Vec<String>,
    lists: Lists,
    data: &'a [u8],
    word_document: &'a [u8],
    drawings: Drawings,
    anchors: HashMap<usize, ShapeAnchor>,
    footnotes: HashMap<usize, Range<usize>>,
    endnotes: HashMap<usize, Range<usize>>,
    text_boxes: HashMap<u32, Range<usize>>,
    stories: Stories,
    limits: &'a Limits,
    report: ImportReport,
    media: Vec<Media>,
    media_bytes: u64,
    /// Each picture's media index by where its data lies, `None` when it could not be kept.
    pictures: HashMap<usize, Option<usize>>,
    notes: Vec<Note>,
    /// Each note by the position of its reference, once read.
    built_notes: HashMap<usize, usize>,
    /// Text boxes of the paragraph being read, set after it.
    floating: Vec<Block>,
    next_frame: u32,
    /// The current number of each level of each list.
    numbers: HashMap<i32, [Option<u32>; 9]>,
    remaining: usize,
    style_properties: HashMap<u16, (Paragraph, Character)>,
    page_break_pending: bool,
    in_note: bool,
    stored_comments: Vec<StoredComment>,
    comments: Vec<Comment>,
    reviewers: Vec<String>,
    tracked_changes: TrackedChanges,
    /// The objects of the object pool, each a compound file of its own, by storage number.
    object_pool: HashMap<u32, Vec<u8>>,
    objects: Vec<EmbeddedObject>,
    /// The pictures are Word 6 `PIC`s in the `WordDocument` stream, not Word 97 drawings.
    legacy_pictures: bool,
    /// The Word 6 drawing objects, by the position that anchors each.
    word6_drawings: HashMap<usize, word6::Word6Drawing>,
}

impl Converter<'_> {
    /// Keep the object of a storage of the object pool with the picture that shows it.
    fn keep_object(&mut self, storage: u32, media: usize) {
        if self.objects.iter().any(|known| known.media == media) {
            return;
        }
        let Some(bytes) = self.object_pool.remove(&storage) else {
            return;
        };
        self.objects.push(EmbeddedObject {
            media,
            program: objects::program_of_compound_file(&bytes),
            name: format!("_{storage}"),
            bytes,
        });
    }

    /// Read the text of every comment, before the main text marks what they cover.
    fn read_comments(&mut self) {
        for stored in self.stored_comments.clone() {
            self.in_note = true;
            let floating = std::mem::take(&mut self.floating);
            let pending = std::mem::take(&mut self.page_break_pending);
            let mut blocks = self.blocks(stored.text.clone());
            self.floating = floating;
            self.page_break_pending = pending;
            self.in_note = false;
            while matches!(blocks.last(), Some(Block::Paragraph { content, .. }) if content.is_empty())
            {
                blocks.pop();
            }
            self.comments.push(Comment {
                author: stored.author,
                initials: stored.initials,
                date: None,
                blocks,
                parent: None,
                resolved: false,
            });
        }
    }

    /// The marks a character takes beyond its formatting: the comments whose text it is in,
    /// and the change it belongs to when changes are kept.
    fn annotation_marks(&self, position: usize, properties: &Character) -> Vec<Mark> {
        let mut marks = Vec::new();
        if self.in_note {
            return marks;
        }
        let reviewer = |(index, date): (u16, u32)| {
            (
                self.reviewers
                    .get(usize::from(index))
                    .cloned()
                    .unwrap_or_default(),
                date_of(date),
            )
        };
        if self.tracked_changes == TrackedChanges::Keep {
            if properties.deleted {
                let (author, date) = reviewer(properties.deletion);
                marks.push(Mark::Deletion { author, date });
            } else if properties.inserted {
                let (author, date) = reviewer(properties.insertion);
                marks.push(Mark::Insertion { author, date });
            }
        }
        for (index, stored) in self.stored_comments.iter().enumerate() {
            if stored
                .covers
                .as_ref()
                .is_some_and(|covers| covers.contains(&position))
            {
                marks.push(Mark::Comment { comment: index });
            }
        }
        marks
    }
    fn convert(
        mut self,
        sections: &[SectionLayout],
        headers: &[u32],
        document_properties: &[u8],
    ) -> DocumentTree {
        let facing_pages = document_properties
            .first()
            .is_some_and(|flags| flags & 1 != 0);
        let default_tab = u16_at(document_properties, 10)
            .map(u32::from)
            .filter(|tab| *tab > 0)
            .unwrap_or(DEFAULT_TAB_TWIPS);
        let (_, normal) = self.properties_of_style(0);
        let font_family = normal
            .font
            .or(self.styles.default_font)
            .and_then(|font| self.font_name(font));
        let font_size = Some(u32::from(normal.size.unwrap_or(DEFAULT_SIZE)));

        let main_end = self.stories.main.min(self.text.len());
        let mut boundaries: Vec<SectionLayout> = sections
            .iter()
            .filter(|section| section.end <= main_end)
            .cloned()
            .collect();
        if boundaries.last().is_none_or(|last| last.end < main_end) {
            let mut last = sections.last().cloned().unwrap_or_else(|| SectionLayout {
                end: main_end,
                layout: PageLayout::default(),
                continuous: false,
                title_page: false,
            });
            last.end = main_end;
            boundaries.push(last);
        }

        let mut blocks = Vec::new();
        let mut tree_sections = Vec::new();
        let mut start = 0;
        let mut inherited = [const { Vec::new() }; HEADER_ENTRIES];
        for (index, section) in boundaries.iter().enumerate() {
            let section_blocks = self.blocks(start..section.end);
            blocks.extend(section_blocks);
            let parts = self.section_headers(headers, index, &mut inherited);
            let mut layout = section.layout.clone();
            layout.default_tab_twips = default_tab;
            // Word lays out a binary document as Word 2003 did.
            layout.spacing_adds_up = true;
            layout.font_family.clone_from(&font_family);
            layout.font_size_half_points = font_size;
            tree_sections.push(Section {
                end_block: blocks.len(),
                layout,
                headers_footers: HeadersFooters {
                    header: parts[1].clone(),
                    footer: parts[3].clone(),
                    first_header: section.title_page.then(|| parts[4].clone()),
                    first_footer: section.title_page.then(|| parts[5].clone()),
                    even_header: facing_pages.then(|| parts[0].clone()),
                    even_footer: facing_pages.then(|| parts[2].clone()),
                },
                continuous: section.continuous && index > 0,
            });
            start = section.end;
            if self.report.truncated {
                break;
            }
        }
        let last = tree_sections.pop().unwrap_or_else(|| Section {
            end_block: blocks.len(),
            layout: PageLayout::default(),
            headers_footers: HeadersFooters::default(),
            continuous: false,
        });
        if !tree_sections.is_empty() {
            tree_sections.push(last.clone());
        }
        self.report.headers_and_footers = inherited.iter().filter(|part| !part.is_empty()).count();
        DocumentTree {
            comments: std::mem::take(&mut self.comments),
            objects: std::mem::take(&mut self.objects),
            blocks,
            media: self.media,
            report: self.report,
            layout: last.layout,
            notes: self.notes,
            headers_footers: last.headers_footers,
            sections: tree_sections,
            macro_project: None,
        }
    }

    /// The six header and footer parts of a section, each inherited from the section before
    /// when it has none of its own.
    fn section_headers(
        &mut self,
        headers: &[u32],
        section: usize,
        inherited: &mut [Vec<Block>; HEADER_ENTRIES],
    ) -> [Vec<Block>; HEADER_ENTRIES] {
        let base = self.stories.headers_start();
        for (kind, slot) in inherited.iter_mut().enumerate() {
            let index = SEPARATOR_ENTRIES + section * HEADER_ENTRIES + kind;
            let (Some(start), Some(end)) = (headers.get(index), headers.get(index + 1)) else {
                continue;
            };
            if end <= start {
                continue;
            }
            let range = base + *start as usize..base + *end as usize;
            let mut blocks = self.blocks(range);
            while matches!(blocks.last(), Some(Block::Paragraph { content, .. }) if content.is_empty())
            {
                blocks.pop();
            }
            *slot = blocks;
        }
        inherited.clone()
    }

    /// The blocks of a range of characters: paragraphs, and tables gathered from their cells.
    fn blocks(&mut self, range: Range<usize>) -> Vec<Block> {
        let end = range.end.min(self.text.len());
        let mut blocks = Vec::new();
        // The tables being read, the outermost first: a nested table sits in a cell of the one
        // before it.
        let mut tables: Vec<TableBuilder> = Vec::new();
        let mut previous: Option<(u16, bool)> = None;
        let mut position = range.start;
        while position < end {
            if self.remaining == 0 {
                self.report.truncated = true;
                break;
            }
            let terminator = self.paragraph_end(position, end);
            let properties = self.paragraph_properties(terminator.min(end - 1));
            if properties.in_table && properties.table_depth >= 1 {
                let depth = usize::try_from(properties.table_depth)
                    .unwrap_or(MAX_TABLE_DEPTH)
                    .min(MAX_TABLE_DEPTH);
                close_tables(&mut tables, depth, &mut blocks);
                while tables.len() < depth {
                    tables.push(TableBuilder::default());
                }
                let mark = self.text.get(terminator).copied();
                let (row_end, cell_end) = if depth == 1 {
                    (
                        properties.row_end && mark == Some('\u{07}'),
                        mark == Some('\u{07}'),
                    )
                } else {
                    (properties.inner_row_end, properties.inner_cell_end)
                };
                if row_end {
                    let grpprl = self
                        .paragraph_run(terminator)
                        .map(|run| run.grpprl.clone())
                        .unwrap_or_default();
                    if let Some(builder) = tables.last_mut() {
                        builder.end_row(Row::read(&grpprl));
                    }
                } else {
                    let block = self.paragraph(position..terminator, &properties);
                    let floating = std::mem::take(&mut self.floating);
                    if let Some(builder) = tables.last_mut() {
                        builder.push(block);
                        for floating in floating {
                            builder.push(floating);
                        }
                        if cell_end {
                            builder.end_cell();
                        }
                    }
                }
            } else {
                if !tables.is_empty() {
                    close_tables(&mut tables, 0, &mut blocks);
                    previous = None;
                }
                let mut block = self.paragraph(position..terminator, &properties);
                if properties.contextual_spacing && previous == Some((properties.style, true)) {
                    if let Some(look) = blocks.last_mut().and_then(look_of) {
                        look.space_after_twips = Some(0);
                    }
                    if let Some(look) = look_of(&mut block) {
                        look.space_before_twips = Some(0);
                    }
                }
                previous = Some((properties.style, properties.contextual_spacing));
                push_block(&mut blocks, block);
                blocks.append(&mut self.floating);
            }
            position = terminator + 1;
        }
        close_tables(&mut tables, 0, &mut blocks);
        blocks
    }

    /// Where the paragraph that starts at `start` ends: its paragraph mark, cell mark, or the
    /// section mark a section ends with; `end` when the range stops before any.
    fn paragraph_end(&self, start: usize, end: usize) -> usize {
        (start..end)
            .find(|index| {
                matches!(self.text[*index], '\r' | '\u{07}')
                    || (self.text[*index] == '\u{0C}' && index + 1 == end)
            })
            .unwrap_or(end)
    }

    fn paragraph_run(&self, position: usize) -> Option<&ParagraphRun> {
        let offset = *self.offsets.get(position)?;
        run_at(&self.paragraphs, offset, |run| (run.start, run.end))
    }

    fn paragraph_properties(&mut self, terminator: usize) -> Paragraph {
        let (style, grpprl) = self
            .paragraph_run(terminator)
            .map_or((0, Vec::new()), |run| (run.style, run.grpprl.clone()));
        let (mut properties, _) = self.properties_of_style(style);
        properties.apply(&grpprl);
        properties.style = style;
        properties
    }

    /// A style's paragraph and character properties, its bases' applied first.
    fn properties_of_style(&mut self, style: u16) -> (Paragraph, Character) {
        if let Some(known) = self.style_properties.get(&style) {
            return known.clone();
        }
        let mut paragraph = Paragraph::default();
        let mut character = Character::default();
        for definition in self.styles.chain(usize::from(style)) {
            paragraph.apply(&definition.paragraph);
            let base = character.clone();
            character.apply(&definition.character, &base);
        }
        self.style_properties
            .insert(style, (paragraph.clone(), character.clone()));
        (paragraph, character)
    }

    /// The character properties of a position: its paragraph style's, its character style's,
    /// then its own.
    fn character_properties(&mut self, position: usize, paragraph_style: u16) -> Character {
        let (_, mut character) = self.properties_of_style(paragraph_style);
        let Some(run) = self
            .offsets
            .get(position)
            .and_then(|offset| run_at(&self.characters, *offset, |run| (run.start, run.end)))
        else {
            return character;
        };
        let grpprl = run.grpprl.clone();
        let mut probe = Character::default();
        probe.apply(&grpprl, &Character::default());
        if let Some(style) = probe.style.filter(|style| *style != 10 && *style != 0x0FFF) {
            for definition in self.styles.chain(usize::from(style)) {
                let base = character.clone();
                character.apply(&definition.character, &base);
            }
        }
        let style_values = character.clone();
        character.apply(&grpprl, &style_values);
        character
    }

    fn font_name(&self, font: u16) -> Option<String> {
        self.fonts
            .get(usize::from(font))
            .filter(|name| !name.is_empty())
            .cloned()
    }

    /// A paragraph: its runs, pictures, note references and fields, as a heading, a list item
    /// or a plain paragraph.
    fn paragraph(&mut self, range: Range<usize>, properties: &Paragraph) -> Block {
        // A page break inside the paragraph before starts this one on a new page.
        let break_before = std::mem::take(&mut self.page_break_pending);
        let (_, style_character) = self.properties_of_style(properties.style);
        let paragraph_font = self.run_look(&style_character, None);
        let content = self.inlines(range, properties.style, &paragraph_font);
        let mut look = Self::paragraph_look(properties, paragraph_font);
        look.page_break_before |= break_before;
        let align = match properties.alignment {
            Some(1) => Some(Alignment::Center),
            Some(2) => Some(Alignment::Right),
            Some(3..) => Some(Alignment::Justify),
            Some(_) | None => None,
        };
        let built_in = self
            .styles
            .style(usize::from(properties.style))
            .map_or(0, |style| style.built_in);
        let heading = if (1..=MAX_HEADING).contains(&built_in) {
            Some(u8::try_from(built_in).unwrap_or(1))
        } else {
            properties.outline_level.map(|level| level + 1)
        };
        if properties.list_override > 0
            && let Some((list, levels)) = self.lists.list(properties.list_override)
            && let Some(level) = levels.get(usize::from(properties.list_level))
        {
            let ordered = level.ordered();
            let mut level_look = Paragraph::default();
            level_look.apply(&level.paragraph);
            let label = self.list_label(list, properties.list_override, properties.list_level);
            look.indent_left_twips = look
                .indent_left_twips
                .or(level_look.indent_left.map(i32::from));
            look.indent_first_line_twips = look
                .indent_first_line_twips
                .or(level_look.indent_first_line.map(i32::from));
            look.list_label = label;
            return Block::ListItem {
                ordered,
                level: properties.list_level,
                list_id: u32::from(properties.list_override),
                content,
                look,
            };
        }
        match heading {
            Some(level) => Block::Heading {
                level: level.clamp(1, 6),
                content,
                align,
                look,
            },
            None => Block::Paragraph {
                content,
                align,
                look,
            },
        }
    }

    fn paragraph_look(properties: &Paragraph, font: RunLook) -> ParagraphLook {
        ParagraphLook {
            space_before_twips: properties.space_before.map(u32::from),
            space_after_twips: properties.space_after.map(u32::from),
            line_spacing: properties.line_spacing.map(|spacing| {
                let value = u32::from(spacing.value.unsigned_abs());
                if spacing.proportional {
                    LineSpacing::Auto(value)
                } else if spacing.value < 0 {
                    LineSpacing::Exact(value)
                } else {
                    LineSpacing::AtLeast(value)
                }
            }),
            indent_left_twips: properties.indent_left.map(i32::from),
            indent_right_twips: properties.indent_right.map(i32::from),
            indent_first_line_twips: properties.indent_first_line.map(i32::from),
            page_break_before: properties.page_break_before,
            font,
            shading: properties.shading.clone(),
            borders: Borders {
                top: properties.borders[0].clone(),
                left: properties.borders[1].clone(),
                bottom: properties.borders[2].clone(),
                right: properties.borders[3].clone(),
                ..Borders::default()
            },
            ..ParagraphLook::default()
        }
    }

    /// The look of a run: everything for the paragraph's own font, only what differs from the
    /// paragraph's font for a run.
    fn run_look(&self, character: &Character, paragraph: Option<&RunLook>) -> RunLook {
        let family = character.font.and_then(|font| self.font_name(font));
        let size = character.size.map(u32::from);
        let look = RunLook {
            family,
            size_half_points: size,
            colour: character.colour.clone(),
            bold: character.bold.then_some(true),
            italic: character.italic.then_some(true),
            highlight: character.highlight.clone(),
            caps: character.caps.then_some(true),
            small_caps: character.small_caps.then_some(true),
            field: None,
        };
        let Some(paragraph) = paragraph else {
            return look;
        };
        let differs =
            |own: &Option<String>, base: &Option<String>| own.clone().filter(|_| own != base);
        RunLook {
            family: differs(&look.family, &paragraph.family),
            size_half_points: look
                .size_half_points
                .filter(|_| look.size_half_points != paragraph.size_half_points),
            colour: differs(&look.colour, &paragraph.colour),
            bold: Some(character.bold).filter(|bold| Some(*bold) != paragraph.bold.or(Some(false))),
            italic: Some(character.italic)
                .filter(|italic| Some(*italic) != paragraph.italic.or(Some(false))),
            highlight: look.highlight,
            caps: look.caps.filter(|_| paragraph.caps != Some(true)),
            small_caps: look
                .small_caps
                .filter(|_| paragraph.small_caps != Some(true)),
            field: None,
        }
    }

    /// The inline content of a paragraph.
    #[expect(
        clippy::too_many_lines,
        reason = "one arm per special character of Word 97"
    )]
    fn inlines(
        &mut self,
        range: Range<usize>,
        paragraph_style: u16,
        paragraph_font: &RunLook,
    ) -> Vec<Inline> {
        let mut content: Vec<Inline> = Vec::new();
        let mut current: Option<(RunKey, String)> = None;
        let mut fields: Vec<Field> = Vec::new();
        let mut hidden = false;
        let mut deleted = false;
        let mut inserted = false;
        let flush = |current: &mut Option<(RunKey, String)>, content: &mut Vec<Inline>| {
            if let Some((key, text)) = current.take().filter(|(_, text)| !text.is_empty()) {
                content.push(Inline::Text {
                    text,
                    marks: key.marks,
                    look: key.look,
                });
            }
        };
        for position in range {
            let Some(&character) = self.text.get(position) else {
                break;
            };
            match character {
                '\u{13}' => {
                    if fields.len() < MAX_FIELD_DEPTH {
                        fields.push(Field::default());
                    }
                    continue;
                }
                '\u{14}' => {
                    let separator = self.character_properties(position, paragraph_style);
                    if let Some(field) = fields.last_mut() {
                        field.in_result = true;
                        let (link, page) = self.interpret_field(&field.instruction);
                        field.link = link;
                        field.page = page;
                        // The separator of an embedding field names its object's storage.
                        if separator.object {
                            field.object = separator.picture;
                        }
                    }
                    continue;
                }
                '\u{15}' => {
                    fields.pop();
                    continue;
                }
                _ => {}
            }
            if let Some(field) = fields.last_mut().filter(|field| !field.in_result) {
                field.instruction.push(character);
                continue;
            }
            let properties = self.character_properties(position, paragraph_style);
            if properties.hidden {
                if !hidden {
                    self.report.hidden_runs += 1;
                }
                hidden = true;
                continue;
            }
            hidden = false;
            if properties.deleted {
                if !deleted {
                    self.report.tracked_deletions += 1;
                }
                deleted = true;
                if self.tracked_changes == TrackedChanges::Accept || self.in_note {
                    continue;
                }
            } else {
                deleted = false;
            }
            if properties.inserted && !inserted {
                self.report.tracked_insertions += 1;
            }
            inserted = properties.inserted;
            let special = properties.special;
            match character {
                '\u{01}' if special => {
                    flush(&mut current, &mut content);
                    if properties.object {
                        self.report.embedded_objects += 1;
                    }
                    if let Some(image) = properties
                        .picture
                        .and_then(|offset| self.inline_image(offset as usize))
                    {
                        if let Inline::Image { media, .. } = &image
                            && let Some(storage) = fields
                                .iter_mut()
                                .rev()
                                .find_map(|field| field.object.take())
                        {
                            self.keep_object(storage, *media);
                        }
                        content.push(image);
                    }
                    continue;
                }
                '\u{02}' if special => {
                    flush(&mut current, &mut content);
                    if !self.in_note
                        && let Some(note) = self.note_at(position)
                    {
                        content.push(Inline::NoteReference { note });
                    }
                    continue;
                }
                '\u{05}' if special => {
                    flush(&mut current, &mut content);
                    if !self.in_note
                        && let Some(comment) = self
                            .stored_comments
                            .iter()
                            .position(|stored| stored.reference == position)
                        && self.stored_comments[comment].covers.is_none()
                    {
                        content.push(Inline::CommentAnchor { comment });
                    }
                    continue;
                }
                '\u{08}' if special => {
                    flush(&mut current, &mut content);
                    if let Some(image) = self.floating_shape(position) {
                        content.push(image);
                    }
                    continue;
                }
                '\u{0B}' => {
                    flush(&mut current, &mut content);
                    content.push(Inline::HardBreak);
                    continue;
                }
                '\u{0C}' => {
                    self.page_break_pending = true;
                    continue;
                }
                _ => {}
            }
            let character = match character {
                '\u{1E}' => '-',
                '\u{1F}' => continue,
                control if control.is_control() && control != '\t' => continue,
                other => other,
            };
            if self.remaining == 0 {
                self.report.truncated = true;
                break;
            }
            self.remaining -= 1;
            let link = fields.iter().rev().find_map(|field| field.link.clone());
            let page = fields.last().and_then(|field| field.page);
            let mut look = self.run_look(&properties, Some(paragraph_font));
            look.field = page;
            let key = RunKey {
                marks: {
                    let mut marks = marks_of(&properties, &look, link.as_deref());
                    marks.extend(self.annotation_marks(position, &properties));
                    marks
                },
                look,
            };
            match &mut current {
                Some((known, text)) if *known == key => text.push(character),
                _ => {
                    flush(&mut current, &mut content);
                    current = Some((key, character.to_string()));
                }
            }
        }
        flush(&mut current, &mut content);
        content
    }

    /// The link and the page number a field's instruction makes of its result.
    fn interpret_field(&mut self, instruction: &str) -> (Option<String>, Option<PageField>) {
        let mut words = instruction.split_whitespace();
        let keyword = words.next().unwrap_or_default().to_ascii_uppercase();
        match keyword.as_str() {
            "HYPERLINK" => {
                let rest: Vec<&str> = words.collect();
                if rest.contains(&"\\l") {
                    self.report.unsupported_links += 1;
                    return (None, None);
                }
                let target = instruction
                    .split('"')
                    .nth(1)
                    .map(str::to_owned)
                    .or_else(|| rest.first().map(|word| (*word).to_owned()))
                    .unwrap_or_default();
                let lower = target.to_ascii_lowercase();
                if ["http://", "https://", "mailto:"]
                    .iter()
                    .any(|scheme| lower.starts_with(scheme))
                {
                    (Some(target), None)
                } else {
                    self.report.unsupported_links += 1;
                    (None, None)
                }
            }
            "PAGE" => (None, Some(PageField::Page)),
            "NUMPAGES" | "SECTIONPAGES" => (None, Some(PageField::Pages)),
            _ => {
                self.report.fields_kept_as_text += 1;
                (None, None)
            }
        }
    }

    /// The note a reference at `position` points at, read the first time.
    fn note_at(&mut self, position: usize) -> Option<usize> {
        if let Some(known) = self.built_notes.get(&position) {
            return Some(*known);
        }
        let (kind, range) = if let Some(range) = self.footnotes.get(&position) {
            self.report.footnotes += 1;
            (NoteKind::Footnote, range.clone())
        } else {
            let range = self.endnotes.get(&position)?.clone();
            self.report.endnotes += 1;
            (NoteKind::Endnote, range)
        };
        self.in_note = true;
        let floating = std::mem::take(&mut self.floating);
        let pending = std::mem::take(&mut self.page_break_pending);
        let mut blocks = self.blocks(range);
        self.floating = floating;
        self.page_break_pending = pending;
        self.in_note = false;
        while matches!(blocks.last(), Some(Block::Paragraph { content, .. }) if content.is_empty())
        {
            blocks.pop();
        }
        self.notes.push(Note { kind, blocks });
        let index = self.notes.len() - 1;
        self.built_notes.insert(position, index);
        Some(index)
    }

    /// The image of an inline picture, kept once however many times it is shown.
    fn inline_image(&mut self, offset: usize) -> Option<Inline> {
        let picture = if self.legacy_pictures {
            let (bytes, width, height) = word6::picture(self.word_document, offset)?;
            InlinePicture {
                bytes,
                width,
                height,
            }
        } else {
            inline_picture(self.data, offset, self.word_document)?
        };
        let media = if let Some(known) = self.pictures.get(&offset) {
            *known
        } else {
            let kept = if let Some(bytes) = picture.bytes {
                self.keep_image(bytes)
            } else {
                self.report.missing_images += 1;
                None
            };
            self.pictures.insert(offset, kept);
            kept
        }?;
        Some(Inline::Image {
            media,
            alt: None,
            width: Some(picture.width / TWIPS_PER_PIXEL).filter(|width| *width > 0),
            height: Some(picture.height / TWIPS_PER_PIXEL).filter(|height| *height > 0),
            anchor: None,
        })
    }

    fn keep_image(&mut self, bytes: Vec<u8>) -> Option<usize> {
        let size = bytes.len() as u64;
        if size > self.limits.max_image_bytes {
            self.report.oversized_images += 1;
            return None;
        }
        if self.media.len() >= self.limits.max_images
            || self.media_bytes.saturating_add(size) > self.limits.max_total_image_bytes
        {
            self.report.images_over_limit += 1;
            return None;
        }
        self.media_bytes += size;
        self.media.push(Media {
            content_type: sniff_image_type(&bytes),
            bytes,
        });
        Some(self.media.len() - 1)
    }

    /// A floating shape anchored at `position`: a picture in its place, or a text box set after
    /// the paragraph; any other shape is counted.
    fn floating_shape(&mut self, position: usize) -> Option<Inline> {
        if let Some(drawing) = self.word6_drawings.remove(&position) {
            return self.word6_drawing(drawing);
        }
        let anchor = *self.anchors.get(&position)?;
        let shape = self
            .drawings
            .shapes
            .get(&anchor.shape)
            .copied()
            .unwrap_or_default();
        let placed = placement(&anchor, &shape);
        if let Some(picture) = shape.picture {
            let Some(bytes) = self.drawings.image(picture).map(<[u8]>::to_vec) else {
                self.report.missing_images += 1;
                return None;
            };
            let media = self.keep_image(bytes)?;
            let [left, top, right, bottom] = anchor.bounds;
            let pixels = |twips: i32| {
                u32::try_from(twips.max(0))
                    .ok()
                    .map(|twips| twips / TWIPS_PER_PIXEL)
                    .filter(|pixels| *pixels > 0)
            };
            return Some(Inline::Image {
                media,
                alt: None,
                width: pixels(right - left),
                height: pixels(bottom - top),
                anchor: Some(Box::new(placed)),
            });
        }
        let Some(range) = shape
            .text
            .and_then(|_| self.text_boxes.get(&anchor.shape).cloned())
        else {
            let drawn = self.shape_picture(&shape, &anchor);
            if drawn.is_none() {
                self.report.shapes += 1;
            }
            return drawn.map(|(media, width, height)| Inline::Image {
                media,
                alt: None,
                width: Some(width),
                height: Some(height),
                anchor: Some(Box::new(placed)),
            });
        };
        self.report.text_boxes += 1;
        self.next_frame += 1;
        let frame = TextFrame {
            shape: crate::office_art::preset_of_shape_type(shape.kind)
                .filter(|preset| *preset != "rect")
                .map(str::to_owned),
            id: self.next_frame,
            anchor: placed,
            fill: shape
                .filled
                .then(|| {
                    shape
                        .fill
                        .map_or_else(|| Some("FFFFFF".to_owned()), colour_reference)
                })
                .flatten(),
            line: shape.stroked.then(|| {
                (
                    shape
                        .line
                        .and_then(colour_reference)
                        .unwrap_or_else(|| "000000".to_owned()),
                    shape
                        .line_width
                        .map_or(6, |width| (width * 8 / 12_700).max(1)),
                )
            }),
        };
        let outer = std::mem::take(&mut self.floating);
        let mut blocks = self.blocks(range);
        while matches!(blocks.last(), Some(Block::Paragraph { content, .. }) if content.is_empty())
        {
            blocks.pop();
        }
        set_frame(&mut blocks, &frame);
        self.floating = outer;
        self.floating.extend(blocks);
        None
    }

    /// A Word 6 drawing object: the picture of its shapes where it stands, and its text boxes set
    /// after the paragraph, each a frame of its own over the text.
    fn word6_drawing(&mut self, drawing: word6::Word6Drawing) -> Option<Inline> {
        let shape = ShapeContent::default();
        for text_box in drawing.text_boxes {
            self.report.text_boxes += 1;
            self.next_frame += 1;
            let anchor = ShapeAnchor {
                bounds: text_box.bounds,
                ..drawing.anchor
            };
            let frame = TextFrame {
                shape: text_box.rounded.then(|| "roundRect".to_owned()),
                id: self.next_frame,
                anchor: placement(&anchor, &shape),
                fill: text_box.fill,
                line: text_box.line,
            };
            let outer = std::mem::take(&mut self.floating);
            let mut blocks = self.blocks(text_box.text);
            while matches!(blocks.last(), Some(Block::Paragraph { content, .. }) if content.is_empty())
            {
                blocks.pop();
            }
            set_frame(&mut blocks, &frame);
            self.floating = outer;
            self.floating.extend(blocks);
        }
        let bytes = drawing.picture?;
        let media = self.keep_image(bytes)?;
        let [left, top, right, bottom] = drawing.anchor.bounds;
        let pixels =
            |twips: i32| (u32::try_from(twips.max(0)).unwrap_or(0) / TWIPS_PER_PIXEL).max(1);
        Some(Inline::Image {
            media,
            alt: None,
            width: Some(pixels(right - left)),
            height: Some(pixels(bottom - top)),
            anchor: Some(Box::new(placement(&drawing.anchor, &shape))),
        })
    }

    /// A shape with no picture and no text, drawn as an SVG picture of its anchor's size: its
    /// type's outline, filled and lined as its properties say. Its media and size in pixels.
    fn shape_picture(
        &mut self,
        shape: &ShapeContent,
        anchor: &ShapeAnchor,
    ) -> Option<(usize, u32, u32)> {
        let [left, top, right, bottom] = anchor.bounds;
        let (width, height) = (
            i64::from(right - left) * EMU_PER_TWIP,
            i64::from(bottom - top) * EMU_PER_TWIP,
        );
        if width <= 0 && height <= 0 {
            return None;
        }
        // A line along one side has no extent across it; the picture keeps a sliver.
        let (width, height) = (width.max(1), height.max(1));
        let mut paths = match shape.kind {
            1 => rectangle_paths(width, height),
            3 => preset_geometry("ellipse", None, width, height)?,
            20 | 32 => preset_geometry("line", None, width, height)?,
            kind => {
                let preset = preset_of_shape_type(kind)?;
                match drawingml_adjustments(kind, &shape.adjust, width, height) {
                    Some(values) => preset_geometry_with_values(preset, &values, width, height)?,
                    None => preset_geometry(preset, None, width, height)?,
                }
            }
        };
        flip_paths(
            &mut paths,
            width,
            height,
            shape.flip_horizontal,
            shape.flip_vertical != upside_down_from_preset(shape.kind),
        );
        let fill = shape
            .filled
            .then(|| {
                shape
                    .fill
                    .map_or_else(|| Some("FFFFFF".to_owned()), colour_reference)
            })
            .flatten();
        let line = shape.stroked.then(|| {
            (
                shape
                    .line
                    .and_then(colour_reference)
                    .unwrap_or_else(|| "000000".to_owned()),
                shape.line_width.map_or(9_525, i64::from),
            )
        });
        let drawn = DrawnShape {
            x: 0,
            y: 0,
            width,
            height,
            rotation: 0.0,
            paths,
            fill,
            line,
            text: Vec::new(),
        };
        let bytes = shapes_svg(&[drawn], width, height)?;
        let media = self.keep_image(bytes)?;
        let pixels =
            |twips: i32| (u32::try_from(twips.max(0)).unwrap_or(0) / TWIPS_PER_PIXEL).max(1);
        Some((media, pixels(right - left), pixels(bottom - top)))
    }

    /// The label of a list paragraph, its level's number counted on.
    fn list_label(&mut self, list: i32, override_index: u16, level: u8) -> Option<ListLabel> {
        let levels = self.lists.list(override_index)?.1.to_vec();
        let definition = levels.get(usize::from(level))?;
        let index = usize::from(level);
        let start_override = self.lists.start_override(override_index, level);
        let numbers = self.numbers.entry(list).or_insert([None; 9]);
        numbers[index] = Some(match numbers[index] {
            Some(number) => number.saturating_add(1),
            None => start_override.unwrap_or(definition.start),
        });
        for deeper in &mut numbers[index + 1..] {
            *deeper = None;
        }
        let numbers = *numbers;
        let mut character = Character::default();
        character.apply(&definition.character, &Character::default());
        let font = self.run_look(&character, None);
        let mut text = String::new();
        for unit in &definition.template {
            if *unit < 9 {
                let referenced = usize::from(*unit);
                let referenced_level = levels.get(referenced).unwrap_or(definition);
                let number = numbers[referenced].unwrap_or(referenced_level.start);
                text.push_str(&format_number(number, referenced_level.format_name()));
            } else if let Some(character) = char::from_u32(u32::from(*unit)) {
                text.push(bullet_character(character, font.family.as_deref()));
            }
        }
        if !definition.ordered() && text.is_empty() {
            text.push('\u{2022}');
        }
        Some(ListLabel {
            text,
            suffix: match definition.follow {
                1 => LabelSuffix::Space,
                2 => LabelSuffix::Nothing,
                _ => LabelSuffix::Tab,
            },
            font,
        })
    }
}

/// The paragraph look of a block that has one.
fn look_of(block: &mut Block) -> Option<&mut ParagraphLook> {
    match block {
        Block::Paragraph { look, .. }
        | Block::Heading { look, .. }
        | Block::ListItem { look, .. } => Some(look),
        Block::Table { .. }
        | Block::Blockquote { .. }
        | Block::CodeBlock { .. }
        | Block::HorizontalRule => None,
    }
}

/// Where a floating shape stands, from its `FSPA` and, where the shape names them, its own
/// position properties, which the `FSPA`'s bases give way to.
fn placement(anchor: &ShapeAnchor, shape: &ShapeContent) -> Anchor {
    let [left, top, right, bottom] = anchor.bounds.map(i64::from);
    let horizontal = match shape.horizontal_base {
        Some(0) => AnchorBase::Margin,
        Some(1) => AnchorBase::Page,
        Some(2) => AnchorBase::Column,
        Some(3) => AnchorBase::Line,
        _ => match anchor.horizontal_base {
            1 => AnchorBase::Page,
            2 => AnchorBase::Column,
            _ => AnchorBase::Margin,
        },
    };
    let vertical = match shape.vertical_base {
        Some(0) => AnchorBase::Margin,
        Some(1) => AnchorBase::Page,
        Some(2) => AnchorBase::Paragraph,
        Some(3) => AnchorBase::Line,
        _ => match anchor.vertical_base {
            1 => AnchorBase::Page,
            2 => AnchorBase::Paragraph,
            _ => AnchorBase::Margin,
        },
    };
    let align = |value: Option<u32>| match value {
        Some(1 | 4) => Some(AnchorAlign::Start),
        Some(2) => Some(AnchorAlign::Center),
        Some(3 | 5) => Some(AnchorAlign::End),
        _ => None,
    };
    Anchor {
        horizontal: AnchorPosition {
            base: horizontal,
            offset_emu: left * EMU_PER_TWIP,
            align: align(shape.horizontal_align),
        },
        vertical: AnchorPosition {
            base: vertical,
            offset_emu: top * EMU_PER_TWIP,
            align: align(shape.vertical_align),
        },
        width_emu: ((right - left) * EMU_PER_TWIP).max(0),
        height_emu: ((bottom - top) * EMU_PER_TWIP).max(0),
        wrap: match anchor.wrap {
            1 => Wrap::TopAndBottom,
            3 if anchor.behind_text => Wrap::Behind,
            3 => Wrap::InFront,
            _ => Wrap::Around,
        },
        distance_emu: [0; 4],
    }
}

/// The marks of a run, as the docx reading gives them.
fn marks_of(properties: &Character, look: &RunLook, link: Option<&str>) -> Vec<Mark> {
    let flags = [
        (properties.bold, Mark::Bold),
        (properties.italic, Mark::Italic),
        (properties.underline && link.is_none(), Mark::Underline),
        (properties.strike, Mark::Strike),
        (properties.superscript, Mark::Superscript),
        (properties.subscript, Mark::Subscript),
    ];
    let mut marks: Vec<Mark> = flags
        .into_iter()
        .filter(|(on, _)| *on)
        .map(|(_, mark)| mark)
        .collect();
    let font_size = look.size_half_points.map(|size| {
        if size % 2 == 0 {
            format!("{}pt", size / 2)
        } else {
            format!("{}.5pt", size / 2)
        }
    });
    let color = look
        .colour
        .as_ref()
        .filter(|_| link.is_none())
        .map(|colour| format!("#{colour}"));
    if look.family.is_some() || font_size.is_some() || color.is_some() {
        marks.push(Mark::TextStyle {
            font_family: look.family.clone(),
            font_size,
            color,
        });
    }
    if let Some(highlight) = &look.highlight {
        marks.push(Mark::Highlight {
            color: format!("#{highlight}"),
        });
    }
    if let Some(href) = link {
        marks.push(Mark::Link {
            href: href.to_owned(),
        });
    }
    marks
}

/// Finish the tables nested deeper than `depth`, each into the cell of the table around it and
/// the outermost into `blocks`.
fn close_tables(tables: &mut Vec<TableBuilder>, depth: usize, blocks: &mut Vec<Block>) {
    while tables.len() > depth {
        let Some(finished) = tables.pop().and_then(TableBuilder::finish) else {
            continue;
        };
        match tables.last_mut() {
            Some(outer) => outer.push(finished),
            None => blocks.push(finished),
        }
    }
}

#[cfg(test)]
mod tests;
