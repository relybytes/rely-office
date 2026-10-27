// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! A `PowerPoint` 97-2003 presentation read into the [`Presentation`] a pptx gives: each slide's
//! background and shapes, the master's shapes behind them, with their frames, outlines, fills,
//! pictures and text, the text styled by its runs over the master's text styles.

use std::collections::HashMap;

use super::freeform::Freeform;
use super::persist::{Persist, read_persist};
use super::text_style::{
    CharacterStyle, Colour, ParagraphStyle, character_exception, master_levels, style_runs,
};
use crate::compound::{open_compound_file, read_stream, u16_at, u32_at};
use crate::document::{Alignment, Media, bullet_character, image_pixel_size, sniff_image_type};
use crate::geometry::{flip_paths, preset_geometry};
use crate::office_art::{
    CHILD_ANCHOR, CLIENT_ANCHOR, CLIENT_DATA, CLIENT_TEXTBOX, DRAWING_CONTAINER, GROUP_CONTAINER,
    GROUP_SHAPE, PROPERTIES, Record, SHAPE, SHAPE_CONTAINER, SHAPE_GROUP_CONTAINER,
    STORE_CONTAINER, TERTIARY_PROPERTIES, child, complex_properties, flag, properties, records,
    store_images,
};
use crate::slides::{
    Bullet, Frame, Gradient, GradientKind, GradientStop, Paint, Presentation, ShapeContent,
    ShapeGeometry, ShapeLine, Slide, SlideParagraph, SlideRun, SlideShape, TextAnchor, TextBody,
};
use crate::{Limits, OfficeError};

/// A pixel at 96 to the inch.
const PIXEL_EMU: i64 = 9_525;
const DOCUMENT: u16 = 0x03E8;
const DOCUMENT_ATOM: u16 = 0x03E9;
const SLIDE: u16 = 0x03EE;
const SLIDE_ATOM: u16 = 0x03EF;
const ENVIRONMENT: u16 = 0x03F2;
const SLIDE_PERSIST: u16 = 0x03F3;
const MAIN_MASTER: u16 = 0x03F8;
const TITLE_MASTER: u16 = 0x0FC3;
const DRAWING_GROUP: u16 = 0x040B;
const DRAWING: u16 = 0x040C;
const COLOUR_SCHEME: u16 = 0x07F0;
const FONT_COLLECTION: u16 = 0x07D5;
const FONT_ENTITY: u16 = 0x0FB7;
const PLACEHOLDER: u16 = 0x0BC3;
const OUTLINE_TEXT_REFERENCE: u16 = 0x0F9E;
const TEXT_HEADER: u16 = 0x0F9F;
const TEXT_CHARS: u16 = 0x0FA0;
const STYLE_TEXT: u16 = 0x0FA1;
const MASTER_STYLE: u16 = 0x0FA3;
const DEFAULT_CHARACTER: u16 = 0x0FA4;
const TEXT_BYTES: u16 = 0x0FA8;
const SLIDE_LIST: u16 = 0x0FF0;
/// Master units are 576 to the inch.
const EMU_PER_MASTER_UNIT: f64 = 914_400.0 / 576.0;
/// The slide size `PowerPoint` 97-2003 lays out when the file says none: 10 by 7.5 inches.
const DEFAULT_SIZE: (i64, i64) = (9_144_000, 6_858_000);
/// The insets of a text frame when its shape names none: a tenth and a twentieth of an inch.
const DEFAULT_INSETS: [i64; 4] = [91_440, 45_720, 91_440, 45_720];
const MAX_SLIDES: usize = 2_000;
const MAX_SHAPES: usize = 5_000;
const MAX_GROUP_DEPTH: usize = 32;
/// The text type of a title, a centred title, a body and the rest (`TextHeaderAtom`).
const TITLE_TYPES: [u32; 2] = [0, 6];
/// The colours of a scheme when the slide and its master give none: background, text, shadow,
/// title, fill, accent, hyperlink and followed hyperlink.
const DEFAULT_SCHEME: [&str; 8] = [
    "FFFFFF", "000000", "808080", "000000", "BBE0E3", "333399", "009999", "99CC00",
];

/// Read a .ppt as slides to draw.
///
/// # Errors
///
/// [`OfficeError::TooLarge`] past the input or stream limits, [`OfficeError::Encrypted`] for a
/// protected presentation, [`OfficeError::DamagedContainer`] for a broken compound file, and
/// [`OfficeError::UnreadableDocument`] when the streams hold no presentation.
pub(crate) fn read_ppt_presentation(
    bytes: &[u8],
    limits: &Limits,
) -> Result<Presentation, OfficeError> {
    if bytes.len() as u64 > limits.max_input_bytes {
        return Err(OfficeError::TooLarge {
            what: "input".to_owned(),
            limit: limits.max_input_bytes,
        });
    }
    let unreadable = |reason: &str| OfficeError::UnreadableDocument(reason.to_owned());
    let mut compound_file = open_compound_file(bytes)?;
    if compound_file.is_stream("/EncryptedSummary") {
        return Err(OfficeError::Encrypted);
    }
    let stream = read_stream(
        &mut compound_file,
        "/PowerPoint Document",
        limits.max_member_bytes,
    )?
    .ok_or_else(|| unreadable("the file has no PowerPoint Document stream"))?;
    let current_user = read_stream(&mut compound_file, "/Current User", limits.max_member_bytes)?
        .unwrap_or_default();
    let pictures =
        read_stream(&mut compound_file, "/Pictures", limits.max_member_bytes)?.unwrap_or_default();
    let persist = read_persist(&current_user, &stream).unwrap_or_default();
    if persist.encrypted {
        return Err(OfficeError::Encrypted);
    }
    let document = persist
        .object(&stream, persist.document)
        .and_then(|bytes| records(bytes).next())
        .or_else(|| records(&stream).find(|record| record.kind == DOCUMENT))
        .filter(|record| record.kind == DOCUMENT)
        .ok_or_else(|| unreadable("the presentation has no document"))?;
    let mut reader = Reader::new(&stream, &persist, document, &pictures, limits);
    let slides = reader.slides(document);
    Ok(Presentation {
        width_emu: reader.size.0,
        height_emu: reader.size.1,
        slides,
        media: reader.media,
        truncated: reader.truncated,
    })
}

/// The text of a placeholder kept in the slide list, and what kind of text it is.
#[derive(Debug, Clone, Default)]
struct TextGroup {
    kind: u32,
    text: String,
    style: Vec<u8>,
}

/// What a master gives the slides that follow it.
#[derive(Debug, Clone, Default)]
struct Master {
    shapes: Vec<SlideShape>,
    background: Option<Paint>,
    scheme: Vec<String>,
    /// The levels of each text type's style.
    styles: HashMap<u16, Vec<(ParagraphStyle, CharacterStyle)>>,
}

/// A group's coordinates and the frame they map onto.
#[derive(Debug, Clone, Copy)]
struct Transform {
    from: (i64, i64, i64, i64),
    to: Frame,
}

impl Transform {
    fn apply(&self, frame: Frame) -> Frame {
        let (left, top, right, bottom) = self.from;
        let scale = |value: i64, from_start: i64, from_end: i64, to_start: i64, to_length: i64| {
            if from_end == from_start {
                return to_start;
            }
            #[expect(clippy::cast_precision_loss, reason = "slide coordinates")]
            let mapped = to_start as f64
                + (value - from_start) as f64 * to_length as f64 / (from_end - from_start) as f64;
            #[expect(clippy::cast_possible_truncation, reason = "slide coordinates")]
            let mapped = mapped.round() as i64;
            mapped
        };
        let x = scale(frame.x, left, right, self.to.x, self.to.width);
        let y = scale(frame.y, top, bottom, self.to.y, self.to.height);
        let x_end = scale(frame.x + frame.width, left, right, self.to.x, self.to.width);
        let y_end = scale(
            frame.y + frame.height,
            top,
            bottom,
            self.to.y,
            self.to.height,
        );
        Frame {
            x,
            y,
            width: (x_end - x).max(0),
            height: (y_end - y).max(0),
        }
    }
}

struct Reader<'a> {
    stream: &'a [u8],
    persist: &'a Persist,
    limits: &'a Limits,
    size: (i64, i64),
    fonts: Vec<String>,
    default_character: CharacterStyle,
    default_styles: HashMap<u16, Vec<(ParagraphStyle, CharacterStyle)>>,
    images: Vec<Option<Vec<u8>>>,
    media: Vec<Media>,
    media_bytes: u64,
    /// Each store image's media index once kept, `None` when it could not be.
    kept: HashMap<u32, Option<usize>>,
    masters: HashMap<u32, Master>,
    shape_count: usize,
    remaining_chars: usize,
    truncated: bool,
}

fn emu(master_units: i64) -> i64 {
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        reason = "slide coordinates"
    )]
    let value = (master_units as f64 * EMU_PER_MASTER_UNIT).round() as i64;
    value
}

fn signed(bytes: &[u8], position: usize) -> Option<i64> {
    u32_at(bytes, position).map(|value| i64::from(i32::from_le_bytes(value.to_le_bytes())))
}

fn signed_short(bytes: &[u8], position: usize) -> Option<i64> {
    u16_at(bytes, position).map(|value| i64::from(i16::from_le_bytes(value.to_le_bytes())))
}

impl<'a> Reader<'a> {
    fn new(
        stream: &'a [u8],
        persist: &'a Persist,
        document: Record<'a>,
        pictures: &[u8],
        limits: &'a Limits,
    ) -> Self {
        let mut reader = Self {
            stream,
            persist,
            limits,
            size: DEFAULT_SIZE,
            fonts: Vec::new(),
            default_character: CharacterStyle::default(),
            default_styles: HashMap::new(),
            images: Vec::new(),
            media: Vec::new(),
            media_bytes: 0,
            kept: HashMap::new(),
            masters: HashMap::new(),
            shape_count: 0,
            remaining_chars: limits.max_text_chars,
            truncated: false,
        };
        for record in records(document.body) {
            match record.kind {
                DOCUMENT_ATOM => {
                    if let (Some(width), Some(height)) =
                        (signed(record.body, 0), signed(record.body, 4))
                        && width > 0
                        && height > 0
                    {
                        reader.size = (emu(width), emu(height));
                    }
                }
                ENVIRONMENT => reader.environment(record.body),
                DRAWING_GROUP => {
                    if let Some(store) = child(record.body, GROUP_CONTAINER)
                        .and_then(|group| child(group.body, STORE_CONTAINER))
                    {
                        reader.images = store_images(store.body, pictures);
                    }
                }
                _ => {}
            }
        }
        reader
    }

    fn environment(&mut self, body: &[u8]) {
        for record in records(body) {
            match record.kind {
                FONT_COLLECTION => {
                    self.fonts = records(record.body)
                        .filter(|font| font.kind == FONT_ENTITY)
                        .map(|font| {
                            let units: Vec<u16> = font
                                .body
                                .get(..64)
                                .unwrap_or(font.body)
                                .as_chunks::<2>()
                                .0
                                .iter()
                                .map(|pair| u16::from_le_bytes(*pair))
                                .take_while(|unit| *unit != 0)
                                .collect();
                            String::from_utf16_lossy(&units)
                        })
                        .collect();
                }
                DEFAULT_CHARACTER => {
                    if let Some((style, _)) = character_exception(record.body, 0) {
                        self.default_character = style;
                    }
                }
                MASTER_STYLE => {
                    self.default_styles
                        .insert(record.instance, master_levels(record.body, record.instance));
                }
                _ => {}
            }
        }
    }

    /// The slides of the slide list, in show order, each over its master.
    fn slides(&mut self, document: Record<'_>) -> Vec<Slide> {
        let lists: Vec<Record<'_>> = records(document.body)
            .filter(|record| record.kind == SLIDE_LIST)
            .collect();
        let mut master_ids: HashMap<u32, u32> = HashMap::new();
        for list in lists.iter().filter(|list| list.instance == 1) {
            for persist in records(list.body).filter(|record| record.kind == SLIDE_PERSIST) {
                if let (Some(persist_id), Some(slide_id)) =
                    (u32_at(persist.body, 0), u32_at(persist.body, 12))
                {
                    master_ids.insert(slide_id, persist_id);
                }
            }
        }
        let mut entries: Vec<(u32, Vec<TextGroup>)> = Vec::new();
        for list in lists.iter().filter(|list| list.instance == 0) {
            let mut current: Option<TextGroup> = None;
            for record in records(list.body) {
                match record.kind {
                    SLIDE_PERSIST => {
                        if let Some(group) = current.take()
                            && let Some((_, groups)) = entries.last_mut()
                        {
                            groups.push(group);
                        }
                        if let Some(persist_id) = u32_at(record.body, 0) {
                            entries.push((persist_id, Vec::new()));
                        }
                    }
                    TEXT_HEADER => {
                        if let Some(group) = current.take()
                            && let Some((_, groups)) = entries.last_mut()
                        {
                            groups.push(group);
                        }
                        current = Some(TextGroup {
                            kind: u32_at(record.body, 0).unwrap_or(4),
                            ..TextGroup::default()
                        });
                    }
                    TEXT_CHARS | TEXT_BYTES => {
                        if let Some(group) = current.as_mut() {
                            group.text = text_of(record);
                        }
                    }
                    STYLE_TEXT => {
                        if let Some(group) = current.as_mut() {
                            group.style = record.body.to_vec();
                        }
                    }
                    _ => {}
                }
            }
            if let Some(group) = current.take()
                && let Some((_, groups)) = entries.last_mut()
            {
                groups.push(group);
            }
        }
        if entries.len() > MAX_SLIDES {
            entries.truncate(MAX_SLIDES);
            self.truncated = true;
        }
        let mut slides = Vec::new();
        for (persist_id, texts) in entries {
            let Some(container) = self
                .persist
                .object(self.stream, persist_id)
                .and_then(|bytes| records(bytes).next())
                .filter(|record| record.kind == SLIDE)
            else {
                continue;
            };
            slides.push(self.slide(container, &texts, &master_ids));
        }
        if slides.is_empty() {
            // A presentation with no persist directory still has its slides in order.
            let containers: Vec<Record<'_>> = records(self.stream)
                .filter(|record| record.kind == SLIDE)
                .collect();
            for container in containers.into_iter().take(MAX_SLIDES) {
                slides.push(self.slide(container, &[], &master_ids));
            }
        }
        slides
    }

    fn slide(
        &mut self,
        container: Record<'_>,
        texts: &[TextGroup],
        master_ids: &HashMap<u32, u32>,
    ) -> Slide {
        let atom = child(container.body, SLIDE_ATOM);
        let master_id = atom.and_then(|atom| u32_at(atom.body, 12)).unwrap_or(0);
        let flags = atom.and_then(|atom| u16_at(atom.body, 20)).unwrap_or(0x7);
        let master = master_ids
            .get(&master_id)
            .copied()
            .and_then(|persist_id| self.master(persist_id))
            .unwrap_or_else(|| Master {
                scheme: DEFAULT_SCHEME
                    .iter()
                    .map(|colour| (*colour).to_owned())
                    .collect(),
                styles: self.default_styles.clone(),
                ..Master::default()
            });
        let scheme = if flags & 0x2 != 0 {
            master.scheme.clone()
        } else {
            colour_scheme(container.body).unwrap_or_else(|| master.scheme.clone())
        };
        let (mut shapes, background) =
            self.drawing(container.body, &scheme, texts, &master.styles, false);
        let background = if flags & 0x4 != 0 {
            master.background.clone()
        } else {
            background.or(master.background.clone())
        };
        if flags & 0x1 != 0 {
            let mut all = master.shapes.clone();
            all.append(&mut shapes);
            shapes = all;
        }
        Slide { background, shapes }
    }

    /// A master, read the first time a slide names it.
    fn master(&mut self, persist_id: u32) -> Option<Master> {
        if let Some(known) = self.masters.get(&persist_id) {
            return Some(known.clone());
        }
        let container = self
            .persist
            .object(self.stream, persist_id)
            .and_then(|bytes| records(bytes).next())
            .filter(|record| matches!(record.kind, MAIN_MASTER | TITLE_MASTER))?;
        let mut styles = self.default_styles.clone();
        for record in records(container.body).filter(|record| record.kind == MASTER_STYLE) {
            styles.insert(record.instance, master_levels(record.body, record.instance));
        }
        let scheme = colour_scheme(container.body).unwrap_or_else(|| {
            DEFAULT_SCHEME
                .iter()
                .map(|colour| (*colour).to_owned())
                .collect()
        });
        let (shapes, background) = self.drawing(container.body, &scheme, &[], &styles, true);
        let master = Master {
            shapes,
            background,
            scheme,
            styles,
        };
        self.masters.insert(persist_id, master.clone());
        Some(master)
    }

    /// The shapes and background of a slide's or master's drawing; a master's placeholders are
    /// templates and are left out.
    fn drawing(
        &mut self,
        container: &[u8],
        scheme: &[String],
        texts: &[TextGroup],
        styles: &HashMap<u16, Vec<(ParagraphStyle, CharacterStyle)>>,
        is_master: bool,
    ) -> (Vec<SlideShape>, Option<Paint>) {
        let Some(drawing) =
            child(container, DRAWING).and_then(|drawing| child(drawing.body, DRAWING_CONTAINER))
        else {
            return (Vec::new(), None);
        };
        let mut shapes = Vec::new();
        let mut background = None;
        let context = Context {
            scheme,
            texts,
            styles,
            is_master,
        };
        for record in records(drawing.body) {
            match record.kind {
                SHAPE_GROUP_CONTAINER => self.group(record.body, &context, None, 0, &mut shapes),
                SHAPE_CONTAINER => {
                    let options = Options::read(record.body);
                    if options.background {
                        background = self.paint(&options, scheme);
                    }
                }
                _ => {}
            }
        }
        (shapes, background)
    }

    fn group(
        &mut self,
        body: &[u8],
        context: &Context<'_>,
        transform: Option<Transform>,
        depth: usize,
        shapes: &mut Vec<SlideShape>,
    ) {
        let mut children = records(body);
        let Some(first) = children.next() else {
            return;
        };
        let options = Options::read(first.body);
        let transform = match (options.group_coordinates, options.frame(transform)) {
            (Some(from), Some(to)) => Some(Transform { from, to }),
            _ => transform,
        };
        for record in children {
            if self.shape_count >= MAX_SHAPES {
                self.truncated = true;
                return;
            }
            match record.kind {
                SHAPE_GROUP_CONTAINER if depth < MAX_GROUP_DEPTH => {
                    self.group(record.body, context, transform, depth + 1, shapes);
                }
                SHAPE_CONTAINER => {
                    if let Some(shape) = self.shape(record.body, context, transform) {
                        self.shape_count += 1;
                        shapes.push(shape);
                    }
                }
                _ => {}
            }
        }
    }

    fn shape(
        &mut self,
        body: &[u8],
        context: &Context<'_>,
        transform: Option<Transform>,
    ) -> Option<SlideShape> {
        let options = Options::read(body);
        if options.deleted || options.background {
            return None;
        }
        if context.is_master && options.placeholder {
            return None;
        }
        let frame = options.frame(transform)?;
        let shape_content = if let Some(picture) = options.picture {
            self.picture(picture)
                .map_or(ShapeContent::None, |media| ShapeContent::Picture { media })
        } else {
            match self.text(body, &options, context) {
                Some(text) => ShapeContent::Text(text),
                None => ShapeContent::None,
            }
        };
        let geometry = options.custom_geometry(frame).unwrap_or_else(|| {
            geometry(
                options.kind,
                frame,
                options.flip_horizontal,
                options.flip_vertical,
            )
        });
        let is_text_box = matches!(options.kind, 202 | 75) || options.placeholder;
        let fill = options
            .filled
            .unwrap_or(!is_text_box)
            .then(|| self.paint(&options, context.scheme))
            .flatten()
            .filter(|_| !matches!(geometry, ShapeGeometry::Line { .. }));
        let line = options
            .stroked
            .unwrap_or(!is_text_box || matches!(geometry, ShapeGeometry::Line { .. }))
            .then(|| ShapeLine {
                colour: options
                    .line_colour
                    .and_then(|colour| reference_colour(colour, context.scheme))
                    .unwrap_or_else(|| {
                        context
                            .scheme
                            .get(1)
                            .cloned()
                            .unwrap_or_else(|| "000000".to_owned())
                    }),
                width_emu: i64::from(options.line_width.unwrap_or(9_525)),
            });
        if shape_content == ShapeContent::None && fill.is_none() && line.is_none() {
            return None;
        }
        Some(SlideShape {
            frame,
            rotation: options.rotation,
            geometry,
            fill,
            line,
            content: shape_content,
        })
    }

    /// A shape's fill ([MS-ODRAW] 2.3.7): a colour, a shaded blend of its two colours, or a
    /// picture, pattern or texture of the blip store.
    fn paint(&mut self, options: &Options, scheme: &[String]) -> Option<Paint> {
        let colour = options.fill_colour(scheme)?;
        match options.fill_type {
            1..=3 => {
                let media = options.fill_blip.and_then(|index| self.picture(index));
                let Some(media) = media else {
                    return Some(Paint::Solid(colour));
                };
                let tile_emu = (options.fill_type != 3)
                    .then(|| {
                        let (width, height) = image_pixel_size(&self.media.get(media)?.bytes)?;
                        Some((i64::from(width) * PIXEL_EMU, i64::from(height) * PIXEL_EMU))
                    })
                    .flatten();
                Some(Paint::Picture { media, tile_emu })
            }
            4..=8 => {
                let back = options
                    .fill_back
                    .and_then(|value| reference_colour(value, scheme))
                    .unwrap_or_else(|| "FFFFFF".to_owned());
                Some(shaded_fill(options, &colour, &back))
            }
            _ => Some(Paint::Solid(colour)),
        }
    }

    /// A store image as media, kept once.
    fn picture(&mut self, index: u32) -> Option<usize> {
        if let Some(known) = self.kept.get(&index) {
            return *known;
        }
        let kept = self
            .images
            .get((index as usize).checked_sub(1)?)
            .cloned()
            .flatten()
            .and_then(|bytes| {
                let size = bytes.len() as u64;
                if self.media.len() >= self.limits.max_images
                    || size > self.limits.max_image_bytes
                    || self.media_bytes.saturating_add(size) > self.limits.max_total_image_bytes
                {
                    self.truncated = true;
                    return None;
                }
                self.media_bytes += size;
                self.media.push(Media {
                    content_type: sniff_image_type(&bytes),
                    bytes,
                });
                Some(self.media.len() - 1)
            });
        self.kept.insert(index, kept);
        kept
    }

    /// The text of a shape: its own text box, or the slide list's text it refers to.
    fn text(&mut self, body: &[u8], options: &Options, context: &Context<'_>) -> Option<TextBody> {
        let textbox = child(body, CLIENT_TEXTBOX)?;
        let mut group = TextGroup {
            kind: 4,
            ..TextGroup::default()
        };
        for record in records(textbox.body) {
            match record.kind {
                TEXT_HEADER => group.kind = u32_at(record.body, 0).unwrap_or(4),
                TEXT_CHARS | TEXT_BYTES => group.text = text_of(record),
                STYLE_TEXT => group.style = record.body.to_vec(),
                OUTLINE_TEXT_REFERENCE => {
                    if let Some(found) =
                        u32_at(record.body, 0).and_then(|index| context.texts.get(index as usize))
                    {
                        group = found.clone();
                    }
                }
                _ => {}
            }
        }
        if group.text.is_empty() {
            return None;
        }
        let master_style = master_style_of(group.kind);
        let levels = context
            .styles
            .get(&master_style)
            .or_else(|| self.default_styles.get(&master_style))
            .cloned()
            .unwrap_or_default();
        let text: Vec<char> = group.text.chars().collect();
        let (paragraph_runs, character_runs) = style_runs(&group.style, text.len());
        let mut paragraphs = Vec::new();
        let mut start = 0;
        while start <= text.len() {
            let end = text[start..]
                .iter()
                .position(|character| *character == '\r')
                .map_or(text.len(), |offset| start + offset);
            let (level, own) = run_at(&paragraph_runs, start)
                .map_or((0, ParagraphStyle::default()), |(level, style)| {
                    (level, *style)
                });
            let level = level.min(8);
            let (master_paragraph, master_character) =
                levels.get(usize::from(level)).copied().unwrap_or_default();
            let style = own.or(master_paragraph);
            let mut runs = Vec::new();
            let mut position = start;
            while position < end {
                let (run_end, character_style) = character_run_at(&character_runs, position)
                    .map_or((end, CharacterStyle::default()), |(run_end, style)| {
                        (run_end.min(end), style)
                    });
                let run_end = run_end.max(position + 1);
                let character_style = character_style
                    .or(master_character)
                    .or(self.default_character);
                let piece: String = text[position..run_end].iter().collect();
                for (index, line) in piece.split('\u{0B}').enumerate() {
                    if index > 0 {
                        runs.push(self.run("", &character_style, context.scheme, true));
                    }
                    let cleaned: String = line
                        .chars()
                        .filter(|character| !character.is_control() || *character == '\t')
                        .collect();
                    if cleaned.is_empty() {
                        continue;
                    }
                    let taken: String = cleaned.chars().take(self.remaining_chars).collect();
                    let count = taken.chars().count();
                    if count < cleaned.chars().count() {
                        self.truncated = true;
                    }
                    self.remaining_chars -= count;
                    if !taken.is_empty() {
                        runs.push(self.run(&taken, &character_style, context.scheme, false));
                    }
                }
                position = run_end;
            }
            let size = runs.first().map_or(1_800, |run: &SlideRun| run.size);
            paragraphs.push(self.paragraph(style, level, runs, size));
            start = end + 1;
        }
        if paragraphs.iter().all(|paragraph| paragraph.runs.is_empty()) {
            return None;
        }
        let title = TITLE_TYPES.contains(&group.kind);
        Some(TextBody {
            paragraphs,
            anchor: options.anchor_text.unwrap_or(if title {
                TextAnchor::Middle
            } else {
                TextAnchor::Top
            }),
            insets_emu: options.insets,
            wrap: options.wrap,
        })
    }

    fn run(
        &self,
        text: &str,
        style: &CharacterStyle,
        scheme: &[String],
        line_break: bool,
    ) -> SlideRun {
        SlideRun {
            text: text.to_owned(),
            size: u32::from(style.size.unwrap_or(18)) * 100,
            bold: style.bold.unwrap_or(false),
            italic: style.italic.unwrap_or(false),
            underline: style.underline.unwrap_or(false),
            strike: false,
            colour: style
                .colour
                .and_then(|colour| resolve(colour, scheme))
                .unwrap_or_else(|| {
                    scheme
                        .get(1)
                        .cloned()
                        .unwrap_or_else(|| "000000".to_owned())
                }),
            family: style
                .font
                .and_then(|font| self.fonts.get(usize::from(font)))
                .cloned()
                .unwrap_or_else(|| "Arial".to_owned()),
            line_break,
        }
    }

    fn paragraph(
        &self,
        style: ParagraphStyle,
        level: u16,
        runs: Vec<SlideRun>,
        size: u32,
    ) -> SlideParagraph {
        let spacing = |value: Option<i16>| match value {
            Some(value) if value < 0 => u32::from(value.unsigned_abs()) * 100 / 8,
            Some(value) => u32::from(value.unsigned_abs()) * size / 100,
            None => 0,
        };
        let left = i64::from(style.left_margin.unwrap_or(0));
        let indent = i64::from(style.indent.unwrap_or(0));
        let bullet = style.bullet.filter(|bullet| *bullet).map(|_| {
            let character = style
                .bullet_character
                .and_then(|character| char::from_u32(u32::from(character)))
                .unwrap_or('\u{2022}');
            let font = style
                .bullet_font
                .and_then(|font| self.fonts.get(usize::from(font)));
            Bullet::Character(bullet_character(character, font.map(String::as_str)).to_string())
        });
        SlideParagraph {
            level: u8::try_from(level).unwrap_or(0),
            align: match style.align {
                Some(1) => Alignment::Center,
                Some(2) => Alignment::Right,
                Some(3) => Alignment::Justify,
                _ => Alignment::Left,
            },
            bullet,
            margin_left_emu: emu(left),
            indent_emu: emu(indent - left),
            line_spacing_percent: match style.line_spacing {
                Some(value) if value > 0 => u32::from(value.unsigned_abs()),
                _ => 100,
            },
            space_before: spacing(style.space_before),
            space_after: spacing(style.space_after),
            runs,
        }
    }
}

struct Context<'c> {
    scheme: &'c [String],
    texts: &'c [TextGroup],
    styles: &'c HashMap<u16, Vec<(ParagraphStyle, CharacterStyle)>>,
    is_master: bool,
}

/// The master text style a text type takes: title, body, or other.
fn master_style_of(kind: u32) -> u16 {
    match kind {
        0 | 6 => 0,
        1 | 5 | 7 | 8 => 1,
        2 => 2,
        _ => 4,
    }
}

/// The paragraph run that covers a position, its level and style.
fn run_at(
    runs: &[(usize, u16, ParagraphStyle)],
    position: usize,
) -> Option<(u16, &ParagraphStyle)> {
    let mut start = 0;
    for (count, level, style) in runs {
        if position < start + count {
            return Some((*level, style));
        }
        start += count;
    }
    runs.last().map(|(_, level, style)| (*level, style))
}

/// The character run that covers a position: where it ends and its style.
fn character_run_at(
    runs: &[(usize, CharacterStyle)],
    position: usize,
) -> Option<(usize, CharacterStyle)> {
    let mut start = 0;
    for (count, style) in runs {
        if position < start + count {
            return Some((start + count, *style));
        }
        start += count;
    }
    None
}

/// The text of a `TextCharsAtom` or a `TextBytesAtom`.
fn text_of(record: Record<'_>) -> String {
    if record.kind == TEXT_BYTES {
        return record.body.iter().map(|byte| char::from(*byte)).collect();
    }
    let units = record
        .body
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair));
    char::decode_utf16(units)
        .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

/// The eight colours of a container's `ColorSchemeAtom`.
fn colour_scheme(body: &[u8]) -> Option<Vec<String>> {
    let scheme =
        records(body).find(|record| record.kind == COLOUR_SCHEME && record.instance == 1)?;
    (0..8)
        .map(|index| {
            let colour = scheme.body.get(index * 4..index * 4 + 3)?;
            Some(format!(
                "{:02X}{:02X}{:02X}",
                colour[0], colour[1], colour[2]
            ))
        })
        .collect()
}

fn resolve(colour: Colour, scheme: &[String]) -> Option<String> {
    match colour {
        Colour::Rgb(red, green, blue) => Some(format!("{red:02X}{green:02X}{blue:02X}")),
        Colour::Scheme(index) => scheme.get(usize::from(index)).cloned(),
    }
}

/// An `OfficeArtCOLORREF`: red, green, blue, or a scheme colour when its flags say so.
fn reference_colour(value: u32, scheme: &[String]) -> Option<String> {
    let [red, green, blue, flags] = value.to_le_bytes();
    if flags & 0x08 != 0 {
        return scheme.get(usize::from(red)).cloned();
    }
    if flags & 0x10 != 0 {
        return None;
    }
    Some(format!("{red:02X}{green:02X}{blue:02X}"))
}

/// The `DrawingML` preset that draws a shape type (`MSOSPT`) the same; `None` for a rectangle
/// and for the types with no outline of their own.
#[expect(
    clippy::too_many_lines,
    reason = "one arm per shape type of the binary format"
)]
fn preset_of_shape_type(kind: u16) -> Option<&'static str> {
    Some(match kind {
        2 => "roundRect",
        4 => "diamond",
        5 => "triangle",
        6 => "rtTriangle",
        7 => "parallelogram",
        8 => "trapezoid",
        9 => "hexagon",
        10 => "octagon",
        11 => "plus",
        12 => "star5",
        13 | 14 => "rightArrow",
        15 => "homePlate",
        16 => "cube",
        17 | 62 => "wedgeRoundRectCallout",
        18 | 59 => "star16",
        19 => "arc",
        21 => "plaque",
        22 => "can",
        23 => "donut",
        33 => "bentConnector2",
        34 => "bentConnector3",
        35 => "bentConnector4",
        36 => "bentConnector5",
        37 => "curvedConnector2",
        38 => "curvedConnector3",
        39 => "curvedConnector4",
        40 => "curvedConnector5",
        41 | 178 => "callout1",
        42 => "callout2",
        43 => "callout3",
        44 | 179 => "accentCallout1",
        45 => "accentCallout2",
        46 => "accentCallout3",
        47 | 180 => "borderCallout1",
        48 => "borderCallout2",
        49 => "borderCallout3",
        50 | 181 => "accentBorderCallout1",
        51 => "accentBorderCallout2",
        52 => "accentBorderCallout3",
        53 => "ribbon2",
        54 => "ribbon",
        55 => "chevron",
        56 => "pentagon",
        57 => "noSmoking",
        58 => "star8",
        60 => "star32",
        61 => "wedgeRectCallout",
        63 => "wedgeEllipseCallout",
        64 => "wave",
        65 => "foldedCorner",
        66 => "leftArrow",
        67 => "downArrow",
        68 => "upArrow",
        69 => "leftRightArrow",
        70 => "upDownArrow",
        71 => "irregularSeal1",
        72 => "irregularSeal2",
        73 => "lightningBolt",
        74 => "heart",
        75 => "frame",
        76 => "quadArrow",
        77 => "leftArrowCallout",
        78 => "rightArrowCallout",
        79 => "upArrowCallout",
        80 => "downArrowCallout",
        81 => "leftRightArrowCallout",
        82 => "upDownArrowCallout",
        83 => "quadArrowCallout",
        84 => "bevel",
        85 => "leftBracket",
        86 => "rightBracket",
        87 => "leftBrace",
        88 => "rightBrace",
        89 => "leftUpArrow",
        90 => "bentUpArrow",
        91 => "bentArrow",
        92 => "star24",
        93 => "stripedRightArrow",
        94 => "notchedRightArrow",
        95 => "blockArc",
        96 => "smileyFace",
        97 => "verticalScroll",
        98 => "horizontalScroll",
        99 | 100 => "circularArrow",
        101 => "uturnArrow",
        102 => "curvedRightArrow",
        103 => "curvedLeftArrow",
        104 => "curvedUpArrow",
        105 => "curvedDownArrow",
        106 => "cloudCallout",
        107 => "ellipseRibbon",
        108 => "ellipseRibbon2",
        110 => "flowChartDecision",
        111 => "flowChartInputOutput",
        112 => "flowChartPredefinedProcess",
        113 => "flowChartInternalStorage",
        114 => "flowChartDocument",
        115 => "flowChartMultidocument",
        116 => "flowChartTerminator",
        117 => "flowChartPreparation",
        118 => "flowChartManualInput",
        119 => "flowChartManualOperation",
        121 => "flowChartPunchedCard",
        122 => "flowChartPunchedTape",
        123 => "flowChartSummingJunction",
        124 => "flowChartOr",
        125 => "flowChartCollate",
        126 => "flowChartSort",
        127 => "flowChartExtract",
        128 => "flowChartMerge",
        129 => "flowChartOfflineStorage",
        130 => "flowChartOnlineStorage",
        131 => "flowChartMagneticTape",
        132 => "flowChartMagneticDisk",
        133 => "flowChartMagneticDrum",
        134 => "flowChartDisplay",
        135 => "flowChartDelay",
        176 => "flowChartAlternateProcess",
        177 => "flowChartOffpageConnector",
        182 => "leftRightUpArrow",
        183 => "sun",
        184 => "moon",
        185 => "bracketPair",
        186 => "bracePair",
        187 => "star4",
        188 => "doubleWave",
        189 => "actionButtonBlank",
        190 => "actionButtonHome",
        191 => "actionButtonHelp",
        192 => "actionButtonInformation",
        193 => "actionButtonForwardNext",
        194 => "actionButtonBackPrevious",
        195 => "actionButtonEnd",
        196 => "actionButtonBeginning",
        197 => "actionButtonReturn",
        198 => "actionButtonDocument",
        199 => "actionButtonSound",
        200 => "actionButtonMovie",
        _ => return None,
    })
}

/// A shaded fill: the fill colour blending to the back colour along the fill's angle, the back
/// colour at the focus; a focus inside the shape blends out to both sides, and the types that
/// shade from the centre or the shape's own outline blend outwards from its middle.
fn shaded_fill(options: &Options, colour: &str, back: &str) -> Paint {
    let focus = f64::from(options.fill_focus.clamp(-100, 100)) / 100.0;
    let stop = |offset: f64, colour: &str| GradientStop {
        offset,
        colour: colour.to_owned(),
    };
    let stops = if focus <= -1.0 + f64::EPSILON || focus >= 1.0 - f64::EPSILON {
        vec![stop(0.0, colour), stop(1.0, back)]
    } else if focus.abs() < f64::EPSILON {
        vec![stop(0.0, back), stop(1.0, colour)]
    } else {
        let middle = focus.abs();
        vec![stop(0.0, colour), stop(middle, back), stop(1.0, colour)]
    };
    let kind = if matches!(options.fill_type, 5 | 6) {
        GradientKind::Radial { centre: (0.5, 0.5) }
    } else {
        GradientKind::Linear {
            angle: (90.0 - options.fill_angle).rem_euclid(360.0),
        }
    };
    Paint::Gradient(Gradient { stops, kind })
}

/// The outline of a shape type (`MSOSPT`), drawn from the `DrawingML` preset of the same shape.
fn geometry(kind: u16, frame: Frame, flip_horizontal: bool, flip_vertical: bool) -> ShapeGeometry {
    let preset = match kind {
        3 | 120 => return ShapeGeometry::Ellipse,
        20 | 32 => {
            return ShapeGeometry::Line {
                flipped: flip_horizontal != flip_vertical,
            };
        }
        _ => match preset_of_shape_type(kind) {
            Some(preset) => preset,
            None => return ShapeGeometry::Rectangle,
        },
    };
    match preset_geometry(preset, None, frame.width, frame.height) {
        Some(mut paths) => {
            flip_paths(
                &mut paths,
                frame.width,
                frame.height,
                flip_horizontal,
                flip_vertical,
            );
            ShapeGeometry::Path(paths)
        }
        None => ShapeGeometry::Other,
    }
}

/// What a shape container says about its shape.
#[derive(Debug, Clone, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is a property the shape sets on its own"
)]
struct Options {
    kind: u16,
    deleted: bool,
    background: bool,
    placeholder: bool,
    flip_horizontal: bool,
    flip_vertical: bool,
    anchor: Option<Frame>,
    child_anchor: Option<Frame>,
    group_coordinates: Option<(i64, i64, i64, i64)>,
    picture: Option<u32>,
    fill: Option<u32>,
    fill_type: u32,
    fill_back: Option<u32>,
    fill_blip: Option<u32>,
    /// Degrees, counter-clockwise from a blend running top to bottom.
    fill_angle: f64,
    /// Where the second colour stands, in percent of the way across.
    fill_focus: i32,
    filled: Option<bool>,
    line_colour: Option<u32>,
    line_width: Option<u32>,
    stroked: Option<bool>,
    rotation: f64,
    insets: [i64; 4],
    anchor_text: Option<TextAnchor>,
    wrap: bool,
    freeform: Freeform,
}

impl Options {
    fn read(body: &[u8]) -> Self {
        let mut options = Self {
            insets: DEFAULT_INSETS,
            wrap: true,
            ..Self::default()
        };
        for record in records(body) {
            match record.kind {
                SHAPE => {
                    options.kind = record.instance;
                    let flags = u32_at(record.body, 4).unwrap_or(0);
                    options.deleted = flags & 0x08 != 0;
                    options.flip_horizontal = flags & 0x40 != 0;
                    options.flip_vertical = flags & 0x80 != 0;
                    options.background = flags & 0x400 != 0;
                }
                GROUP_SHAPE => {
                    if let (Some(left), Some(top), Some(right), Some(bottom)) = (
                        signed(record.body, 0),
                        signed(record.body, 4),
                        signed(record.body, 8),
                        signed(record.body, 12),
                    ) {
                        options.group_coordinates = Some((left, top, right, bottom));
                    }
                }
                CLIENT_ANCHOR => {
                    let bounds = if record.body.len() >= 16 {
                        (
                            signed(record.body, 0),
                            signed(record.body, 4),
                            signed(record.body, 8),
                            signed(record.body, 12),
                        )
                    } else {
                        (
                            signed_short(record.body, 0),
                            signed_short(record.body, 2),
                            signed_short(record.body, 4),
                            signed_short(record.body, 6),
                        )
                    };
                    if let (Some(top), Some(left), Some(right), Some(bottom)) = bounds {
                        options.anchor = Some(Frame {
                            x: emu(left),
                            y: emu(top),
                            width: emu(right - left).max(0),
                            height: emu(bottom - top).max(0),
                        });
                    }
                }
                CHILD_ANCHOR => {
                    if let (Some(left), Some(top), Some(right), Some(bottom)) = (
                        signed(record.body, 0),
                        signed(record.body, 4),
                        signed(record.body, 8),
                        signed(record.body, 12),
                    ) {
                        options.child_anchor = Some(Frame {
                            x: left,
                            y: top,
                            width: (right - left).max(0),
                            height: (bottom - top).max(0),
                        });
                    }
                }
                CLIENT_DATA => options.placeholder = child(record.body, PLACEHOLDER).is_some(),
                PROPERTIES | TERTIARY_PROPERTIES => {
                    options.properties(record);
                    options.complex(record);
                }
                _ => {}
            }
        }
        options
    }

    /// The vertices, segments and guides of a free-form outline.
    fn complex(&mut self, record: Record<'_>) {
        for (identifier, data) in complex_properties(record) {
            match identifier {
                0x0145 => self.freeform.read_vertices(data),
                0x0146 => self.freeform.read_segments(data),
                0x0156 => self.freeform.read_guides(data),
                _ => {}
            }
        }
    }

    /// A free-form outline in the frame, mirrored as the shape is.
    fn custom_geometry(&self, frame: Frame) -> Option<ShapeGeometry> {
        let mut paths = self.freeform.paths(frame.width, frame.height)?;
        flip_paths(
            &mut paths,
            frame.width,
            frame.height,
            self.flip_horizontal,
            self.flip_vertical,
        );
        Some(ShapeGeometry::Path(paths))
    }

    fn properties(&mut self, record: Record<'_>) {
        for (identifier, value) in properties(record) {
            let length = i64::from(i32::from_le_bytes(value.to_le_bytes()));
            match identifier {
                0x0004 => {
                    self.rotation = f64::from(i32::from_le_bytes(value.to_le_bytes())) / 65_536.0;
                }
                0x0081 => self.insets[0] = length,
                0x0082 => self.insets[1] = length,
                0x0083 => self.insets[2] = length,
                0x0084 => self.insets[3] = length,
                0x0085 => self.wrap = value != 2,
                0x0087 => {
                    self.anchor_text = Some(match value {
                        1 | 4 => TextAnchor::Middle,
                        2 | 5 => TextAnchor::Bottom,
                        _ => TextAnchor::Top,
                    });
                }
                0x0104 => self.picture = Some(value),
                0x0140..=0x0143 => self.freeform.bounds[usize::from(identifier - 0x0140)] = length,
                0x0147..=0x0150 => self.freeform.adjust[usize::from(identifier - 0x0147)] = length,
                0x0180 => self.fill_type = value,
                0x0181 => self.fill = Some(value),
                0x0183 => self.fill_back = Some(value),
                0x0186 => self.fill_blip = Some(value),
                0x018B => {
                    self.fill_angle = f64::from(i32::from_le_bytes(value.to_le_bytes())) / 65_536.0;
                }
                0x018C => self.fill_focus = i32::from_le_bytes(value.to_le_bytes()),
                0x01BF => self.filled = flag(value, 4).or(self.filled),
                0x01C0 => self.line_colour = Some(value),
                0x01CB => self.line_width = Some(value),
                0x01FF => self.stroked = flag(value, 3).or(self.stroked),
                _ => {}
            }
        }
    }

    /// The frame of the shape on the slide: its own anchor, or its child anchor mapped through
    /// the groups it is in.
    fn frame(&self, transform: Option<Transform>) -> Option<Frame> {
        match (self.child_anchor, transform) {
            (Some(child), Some(transform)) => Some(transform.apply(child)),
            _ => self.anchor,
        }
    }

    fn fill_colour(&self, scheme: &[String]) -> Option<String> {
        match self.fill {
            Some(value) => reference_colour(value, scheme),
            None => scheme.get(4).cloned(),
        }
    }
}

#[cfg(test)]
mod tests;
