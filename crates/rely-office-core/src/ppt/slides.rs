// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! A `PowerPoint` 97-2003 presentation read into the [`Presentation`] a pptx gives: each slide's
//! background and shapes, the master's shapes behind them, with their frames, outlines, fills,
//! pictures and text, the text styled by its runs over the master's text styles.

use std::collections::HashMap;

use super::persist::{Persist, read_persist};
use super::text_style::{
    CharacterStyle, Colour, ParagraphStyle, character_exception, master_levels, style_runs,
};
use crate::compound::{open_compound_file, read_stream, u16_at, u32_at};
use crate::document::{Alignment, Media, bullet_character, image_pixel_size, sniff_image_type};
use crate::geometry::{flip_paths, preset_geometry, preset_geometry_with_values};
use crate::office_art::freeform::Freeform;
use crate::office_art::{
    CHILD_ANCHOR, CLIENT_ANCHOR, CLIENT_DATA, CLIENT_TEXTBOX, DRAWING_CONTAINER, GROUP_CONTAINER,
    GROUP_SHAPE, PROPERTIES, Record, SHAPE, SHAPE_CONTAINER, SHAPE_GROUP_CONTAINER,
    STORE_CONTAINER, TERTIARY_PROPERTIES, child, complex_properties, drawingml_adjustments, flag,
    legacy_shape_paths, preset_of_shape_type, properties, records, store_images,
    upside_down_from_preset,
};
use crate::slides::{
    Animation, AnimationClass, AnimationTrigger, Bullet, Frame, Gradient, GradientKind,
    GradientStop, Paint, Presentation, ShapeContent, ShapeGeometry, ShapeLine, Slide,
    SlideParagraph, SlideRun, SlideShape, TextAnchor, TextBody, Transition,
};
use crate::{Limits, OfficeError};

/// A pixel at 96 to the inch.
const PIXEL_EMU: i64 = 9_525;
const DOCUMENT: u16 = 0x03E8;
const DOCUMENT_ATOM: u16 = 0x03E9;
const SLIDE: u16 = 0x03EE;
const SLIDE_ATOM: u16 = 0x03EF;
const NOTES: u16 = 0x03F0;
const SLIDE_SHOW_INFO: u16 = 0x03F9;
const ANIMATION_INFO: u16 = 0x1014;
const ANIMATION_INFO_ATOM: u16 = 0x0FF1;
const PROGRAMMABLE_TAGS: u16 = 0x1388;
const BINARY_TAG: u16 = 0x138A;
const BINARY_TAG_DATA: u16 = 0x138B;
const TIME_NODE: u16 = 0xF144;
const TIME_NODE_ATOM: u16 = 0xF127;
const TIME_PROPERTIES: u16 = 0xF13D;
const TIME_VARIANT: u16 = 0xF142;
const TIME_CONDITION: u16 = 0xF125;
const TIME_CONDITION_ATOM: u16 = 0xF128;
const VISUAL_SHAPE: u16 = 0x2AFB;
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
        ..Presentation::default()
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
    /// The persist id of each notes page, by the notes id a slide names.
    notes_ids: HashMap<u32, u32>,
    /// The build effects of the shapes of the drawing being read, by shape index.
    builds: Vec<(usize, Build)>,
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
            notes_ids: HashMap::new(),
            builds: Vec::new(),
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
        for list in lists.iter().filter(|list| matches!(list.instance, 1 | 2)) {
            for persist in records(list.body).filter(|record| record.kind == SLIDE_PERSIST) {
                if let (Some(persist_id), Some(slide_id)) =
                    (u32_at(persist.body, 0), u32_at(persist.body, 12))
                {
                    if list.instance == 1 {
                        master_ids.insert(slide_id, persist_id);
                    } else {
                        self.notes_ids.insert(slide_id, persist_id);
                    }
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
        self.builds.clear();
        let (mut shapes, background) =
            self.drawing(container.body, &scheme, texts, &master.styles, false);
        let mut builds = std::mem::take(&mut self.builds);
        let background = if flags & 0x4 != 0 {
            master.background.clone()
        } else {
            background.or(master.background.clone())
        };
        if flags & 0x1 != 0 {
            let mut all = master.shapes.clone();
            for shape in &mut all {
                shape.inherited = true;
            }
            for (index, _) in &mut builds {
                *index += all.len();
            }
            all.append(&mut shapes);
            shapes = all;
        }
        builds.sort_by_key(|(_, build)| build.order);
        let show = child(container.body, SLIDE_SHOW_INFO);
        let notes = atom
            .and_then(|atom| u32_at(atom.body, 16))
            .filter(|id| *id != 0)
            .and_then(|id| self.notes(id, &scheme, &master.styles));
        let mut animations = timeline(container.body, &shapes);
        if animations.is_empty() {
            animations = builds
                .into_iter()
                .map(|(shape, build)| build.animation(shape))
                .collect();
        }
        Slide {
            background,
            shapes,
            notes,
            hidden: show
                .and_then(|show| u16_at(show.body, 10))
                .is_some_and(|show_flags| show_flags & 0x4 != 0),
            transition: show.and_then(|show| transition(show.body)),
            animations,
        }
    }

    /// The speaker notes of a slide: the text of the notes page the slide names, its body being
    /// the shape that holds the most text.
    fn notes(
        &mut self,
        notes_id: u32,
        scheme: &[String],
        styles: &HashMap<u16, Vec<(ParagraphStyle, CharacterStyle)>>,
    ) -> Option<TextBody> {
        let persist_id = *self.notes_ids.get(&notes_id)?;
        let container = self
            .persist
            .object(self.stream, persist_id)
            .and_then(|bytes| records(bytes).next())
            .filter(|record| record.kind == NOTES)?;
        let (shapes, _) = self.drawing(container.body, scheme, &[], styles, false);
        shapes
            .into_iter()
            .filter_map(|shape| match shape.content {
                ShapeContent::Text(body) => Some(body),
                _ => None,
            })
            .max_by_key(|body| {
                body.paragraphs
                    .iter()
                    .flat_map(|paragraph| &paragraph.runs)
                    .map(|run| run.text.trim().len())
                    .sum::<usize>()
            })
            .filter(|body| {
                body.paragraphs
                    .iter()
                    .any(|paragraph| paragraph.runs.iter().any(|run| !run.text.trim().is_empty()))
            })
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
                    if let Some(mut shape) = self.shape(record.body, context, transform) {
                        self.shape_count += 1;
                        if !context.is_master {
                            shape.id = records(record.body)
                                .find(|atom| atom.kind == SHAPE)
                                .and_then(|atom| u32_at(atom.body, 0));
                        }
                        if !context.is_master
                            && let Some(build) = Build::read(record.body)
                        {
                            self.builds.push((shapes.len(), build));
                        }
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
                &options.adjustments,
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
            id: None,
            inherited: false,
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
/// The effects of the timeline `PowerPoint` 2002 keeps in the slide's `___PPT10` binary tag:
/// each time node whose properties name a preset class, on the shape its visual element names.
fn timeline(slide: &[u8], shapes: &[SlideShape]) -> Vec<Animation> {
    let mut animations = Vec::new();
    for tags in records(slide).filter(|record| record.kind == PROGRAMMABLE_TAGS) {
        for tag in records(tags.body).filter(|record| record.kind == BINARY_TAG) {
            if let Some(data) = child(tag.body, BINARY_TAG_DATA) {
                for root in records(data.body).filter(|record| record.kind == TIME_NODE) {
                    time_effects(root.body, shapes, &mut animations, 0);
                }
            }
        }
    }
    animations
}

fn time_effects(node: &[u8], shapes: &[SlideShape], animations: &mut Vec<Animation>, depth: usize) {
    if depth > 32 || animations.len() >= 500 {
        return;
    }
    let property = |id: u16| {
        child(node, TIME_PROPERTIES).and_then(|list| {
            records(list.body)
                .find(|variant| variant.kind == TIME_VARIANT && variant.instance == id)
                .filter(|variant| variant.body.first() == Some(&1))
                .and_then(|variant| u32_at(variant.body, 1))
        })
    };
    if let Some(class) = property(0x0B) {
        if let Some(animation) = time_effect(node, class, &property, shapes) {
            animations.push(animation);
        }
        return;
    }
    for inner in records(node).filter(|record| record.kind == TIME_NODE) {
        time_effects(inner.body, shapes, animations, depth + 1);
    }
}

/// One effect node: its preset (`0x09`), variant (`0x0A`), class (`0x0B`) and trigger
/// (`0x14`) properties, its delay and the longest of the behaviours it holds.
fn time_effect(
    node: &[u8],
    class: u32,
    property: &dyn Fn(u16) -> Option<u32>,
    shapes: &[SlideShape],
) -> Option<Animation> {
    let class = match class {
        1 => AnimationClass::Entrance,
        2 => AnimationClass::Exit,
        3 => AnimationClass::Emphasis,
        4 => AnimationClass::Path,
        _ => return None,
    };
    let target = visual_shape(node, 0)?;
    let shape = shapes
        .iter()
        .position(|shape| !shape.inherited && shape.id == Some(target))?;
    let delay_ms = child(node, TIME_CONDITION)
        .and_then(|condition| child(condition.body, TIME_CONDITION_ATOM))
        .and_then(|atom| u32_at(atom.body, 12))
        .map(|delay| i32::from_le_bytes(delay.to_le_bytes()).clamp(0, 3_600_000))
        .and_then(|delay| u32::try_from(delay).ok())
        .unwrap_or(0);
    Some(Animation {
        shape,
        class,
        preset: property(0x09).unwrap_or(0),
        preset_subtype: property(0x0A).unwrap_or(0),
        trigger: match property(0x14) {
            Some(2) => AnimationTrigger::WithPrevious,
            Some(3) => AnimationTrigger::AfterPrevious,
            _ => AnimationTrigger::OnClick,
        },
        delay_ms,
        duration_ms: longest_time(node, 0).unwrap_or(500),
        motion_path: None,
    })
}

/// The shape id the first visual element under a time node names.
fn visual_shape(node: &[u8], depth: usize) -> Option<u32> {
    if depth > 16 {
        return None;
    }
    records(node).find_map(|record| {
        if record.kind == VISUAL_SHAPE {
            return u32_at(record.body, 8);
        }
        record
            .is_container()
            .then(|| visual_shape(record.body, depth + 1))
            .flatten()
    })
}

/// The longest duration of the behaviours under a time node, in milliseconds.
fn longest_time(node: &[u8], depth: usize) -> Option<u32> {
    if depth > 16 {
        return None;
    }
    records(node)
        .filter_map(|record| {
            if record.kind == TIME_NODE_ATOM {
                return u32_at(record.body, 24)
                    .map(|duration| i32::from_le_bytes(duration.to_le_bytes()))
                    .filter(|duration| *duration > 0)
                    .and_then(|duration| u32::try_from(duration.min(3_600_000)).ok());
            }
            record
                .is_container()
                .then(|| longest_time(record.body, depth + 1))
                .flatten()
        })
        .max()
}

/// How a shape is built on its slide, the `AnimationInfoAtom` of `PowerPoint` 97: its effect
/// and direction, whether it starts by itself and after how long, and its place in the order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Build {
    order: u16,
    automatic: bool,
    delay_ms: u32,
    effect: u8,
    direction: u8,
}

impl Build {
    /// The build of a shape container, kept in its client data.
    fn read(shape: &[u8]) -> Option<Self> {
        let atom = records(shape)
            .find(|record| record.kind == CLIENT_DATA)
            .and_then(|data| child(data.body, ANIMATION_INFO))
            .and_then(|info| child(info.body, ANIMATION_INFO_ATOM))?;
        let body = atom.body;
        Some(Self {
            order: u16_at(body, 16)?,
            automatic: u16_at(body, 4).is_some_and(|flags| flags & 0x4 != 0),
            delay_ms: u32_at(body, 12)
                .map(|delay| i32::from_le_bytes(delay.to_le_bytes()).clamp(0, 3_600_000))
                .and_then(|delay| u32::try_from(delay).ok())
                .unwrap_or(0),
            effect: *body.get(21)?,
            direction: body.get(22).copied().unwrap_or(0),
        })
    }

    /// The entrance effect of the gallery the build looks like, on shape `shape`.
    fn animation(self, shape: usize) -> Animation {
        let (preset, preset_subtype) = match self.effect {
            0x00 => (1, 0),
            0x02 => (3, if self.direction == 0 { 10 } else { 5 }),
            0x03 => (5, 0),
            0x05 => (9, 0),
            0x08 => (14, 0),
            0x09 => (18, 0),
            0x0A => (
                22,
                [8, 1, 2, 4]
                    .get(usize::from(self.direction))
                    .copied()
                    .unwrap_or(4),
            ),
            0x0B => (53, 0),
            0x0C => (
                2,
                [8, 1, 2, 4]
                    .get(usize::from(self.direction))
                    .copied()
                    .unwrap_or(4),
            ),
            0x0D => (16, 0),
            0x11 => (8, 0),
            0x12 => (13, 0),
            0x13 => (20, 0),
            0x1A => (21, 0),
            _ => (10, 0),
        };
        Animation {
            shape,
            class: AnimationClass::Entrance,
            preset,
            preset_subtype,
            trigger: if self.automatic {
                AnimationTrigger::AfterPrevious
            } else {
                AnimationTrigger::OnClick
            },
            delay_ms: self.delay_ms,
            duration_ms: 500,
            motion_path: None,
        }
    }
}

/// The transition of a `SlideShowSlideInfoAtom`: its effect and direction in the numbering of
/// [MS-PPT], its speed and how the slide advances. A cut is no transition.
fn transition(atom: &[u8]) -> Option<Transition> {
    let time = u32_at(atom, 0).unwrap_or(0);
    let direction = atom.get(8).copied().unwrap_or(0);
    let effect = atom.get(9).copied().unwrap_or(0);
    let flags = u16_at(atom, 10).unwrap_or(0);
    let speed = atom.get(12).copied().unwrap_or(0);
    let side = |value: u8| {
        ["l", "u", "r", "d", "lu", "ru", "ld", "rd"]
            .get(usize::from(value))
            .map(|side| (*side).to_owned())
    };
    let orientation = |vertical: bool| Some(if vertical { "vert" } else { "horz" }.to_owned());
    let (name, way) = match effect {
        0 if flags & 0x80 == 0 && flags & 0x1 != 0 => return None,
        0 => ("cut", None),
        1 => ("random", None),
        2 => ("blinds", orientation(direction == 0)),
        3 => ("checker", orientation(direction == 1)),
        4 => ("cover", side(direction)),
        5 => ("dissolve", None),
        7 => ("pull", side(direction)),
        8 => ("randomBar", orientation(direction == 1)),
        9 => ("strips", side(direction.saturating_add(4).min(7))),
        10 => ("wipe", side(direction.min(3))),
        11 => (
            "zoom",
            Some(if direction == 0 { "out" } else { "in" }.to_owned()),
        ),
        13 => (
            "split",
            Some(if direction >= 2 { "vert" } else { "horz" }.to_owned()),
        ),
        17 => ("diamond", None),
        18 => ("plus", None),
        19 => ("wedge", None),
        20 => ("push", side(direction.min(3))),
        21 => ("comb", orientation(direction == 1)),
        22 => ("newsflash", None),
        26 => ("wheel", None),
        27 => ("circle", None),
        _ => ("fade", None),
    };
    let automatic = flags & 0x80 != 0;
    Some(Transition {
        effect: name.to_owned(),
        direction: way,
        duration_ms: Some(match speed {
            0 => 1_000,
            1 => 750,
            _ => 500,
        }),
        advance_on_click: flags & 0x1 != 0 || !automatic,
        advance_after_ms: automatic.then_some(time.min(3_600_000)),
    })
}

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

/// The outline of a shape type (`MSOSPT`), drawn from the `DrawingML` preset of the same shape
/// with the adjust values the shape sets.
fn geometry(
    kind: u16,
    adjustments: &[Option<i64>; 10],
    frame: Frame,
    flip_horizontal: bool,
    flip_vertical: bool,
) -> ShapeGeometry {
    if let Some(mut paths) = legacy_shape_paths(kind, adjustments, frame.width, frame.height) {
        flip_paths(
            &mut paths,
            frame.width,
            frame.height,
            flip_horizontal,
            flip_vertical,
        );
        return ShapeGeometry::Path(paths);
    }
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
    let paths = match drawingml_adjustments(kind, adjustments, frame.width, frame.height) {
        Some(values) => preset_geometry_with_values(preset, &values, frame.width, frame.height),
        None => preset_geometry(preset, None, frame.width, frame.height),
    };
    match paths {
        Some(mut paths) => {
            flip_paths(
                &mut paths,
                frame.width,
                frame.height,
                flip_horizontal,
                flip_vertical != upside_down_from_preset(kind),
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
    /// The adjust values the shape sets, unset ones `None`.
    adjustments: [Option<i64>; 10],
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
                0x0147..=0x0150 => {
                    let index = usize::from(identifier - 0x0147);
                    #[expect(clippy::cast_precision_loss, reason = "adjust values are small")]
                    let adjust = length as f64;
                    self.freeform.adjust[index] = adjust;
                    self.adjustments[index] = Some(length);
                }
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
