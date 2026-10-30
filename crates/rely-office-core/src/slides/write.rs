// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! A presentation written as a pptx: one slide part per slide, each shape with its frame,
//! outline, fill, text, picture or table set on the slide itself, over a blank layout and
//! master, so the file shows what the presentation held whatever it was read from.

use std::collections::{BTreeMap, HashSet};
use std::fmt::Write;

use super::show::{timing_xml, transition_xml};

use super::{
    Bullet, Frame, Gradient, GradientKind, Paint, Presentation, ShapeContent, ShapeGeometry,
    ShapeLine, Slide, SlideParagraph, SlideRun, SlideShape, SlideTable, TextAnchor, TextBody,
};
use crate::OfficeError;
use crate::document::{
    Alignment, escape_xml as escape, image_extension, image_pixel_size, zip_members,
};
use crate::geometry::{FillShade, PathCommand, PathPoint, ShapePath};

const PRESENTATION_NAMESPACES: &str = concat!(
    r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
    r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#,
    r#"xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#
);
const RELATIONSHIP_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const XML_HEADER: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;
/// The smallest slide `PowerPoint` opens, and the largest, in English Metric Units.
const MIN_SIDE: i64 = 914_400;
const MAX_SIDE: i64 = 51_206_400;

/// Write a presentation as a pptx. A picture whose type is not one `PowerPoint` shows (PNG,
/// JPEG, GIF, BMP, TIFF, WebP, SVG) is drawn as its empty frame.
///
/// # Errors
///
/// [`OfficeError::DamagedContainer`] when the zip cannot be written, which only a failing
/// allocator causes.
pub fn write_pptx(presentation: &Presentation) -> Result<Vec<u8>, OfficeError> {
    let width = presentation.width_emu.clamp(MIN_SIDE, MAX_SIDE);
    let height = presentation.height_emu.clamp(MIN_SIDE, MAX_SIDE);
    let mut media_parts: BTreeMap<usize, String> = BTreeMap::new();
    let mut members: Vec<(String, Vec<u8>)> = Vec::new();
    let slide_count = presentation.slides.len().max(1);
    let blank = Slide::default();
    let mut noted: Vec<usize> = Vec::new();
    for index in 0..slide_count {
        let slide = presentation.slides.get(index).unwrap_or(&blank);
        let mut writer = SlideWriter {
            presentation,
            relationships: vec![(
                "rId1".to_owned(),
                "slideLayout",
                "../slideLayouts/slideLayout1.xml".to_owned(),
            )],
            media_parts: &mut media_parts,
        };
        let xml = writer.slide(slide);
        if let Some(notes) = slide.notes.as_ref() {
            let number = index + 1;
            noted.push(number);
            let relationship = format!("rId{}", writer.relationships.len() + 1);
            writer.relationships.push((
                relationship,
                "notesSlide",
                format!("../notesSlides/notesSlide{number}.xml"),
            ));
            members.extend(notes_parts(number, notes));
        }
        let relationships = relationships_xml(&writer.relationships);
        members.push((
            format!("ppt/slides/slide{}.xml", index + 1),
            xml.into_bytes(),
        ));
        members.push((
            format!("ppt/slides/_rels/slide{}.xml.rels", index + 1),
            relationships.into_bytes(),
        ));
    }
    for (media, part) in &media_parts {
        if let Some(item) = presentation.media.get(*media) {
            members.push((format!("ppt/{part}"), item.bytes.clone()));
        }
    }

    let mut slide_ids = String::new();
    let mut presentation_relationships = vec![
        (
            "rId1".to_owned(),
            "slideMaster",
            "slideMasters/slideMaster1.xml".to_owned(),
        ),
        ("rId2".to_owned(), "theme", "theme/theme1.xml".to_owned()),
    ];
    for index in 0..slide_count {
        let id = format!("rId{}", index + 3);
        let _ = write!(slide_ids, r#"<p:sldId id="{}" r:id="{id}"/>"#, 256 + index);
        presentation_relationships.push((id, "slide", format!("slides/slide{}.xml", index + 1)));
    }
    let notes_master = if noted.is_empty() {
        String::new()
    } else {
        let id = format!("rId{}", slide_count + 3);
        presentation_relationships.push((
            id.clone(),
            "notesMaster",
            "notesMasters/notesMaster1.xml".to_owned(),
        ));
        members.extend(notes_master_parts());
        format!(r#"<p:notesMasterIdLst><p:notesMasterId r:id="{id}"/></p:notesMasterIdLst>"#)
    };
    let presentation_xml = format!(
        r#"{XML_HEADER}<p:presentation {PRESENTATION_NAMESPACES}><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst>{notes_master}<p:sldIdLst>{slide_ids}</p:sldIdLst><p:sldSz cx="{width}" cy="{height}"/><p:notesSz cx="{height}" cy="{width}"/><p:defaultTextStyle/></p:presentation>"#
    );
    let mut parts = package_parts(
        slide_count,
        &noted,
        presentation_xml,
        &presentation_relationships,
    );
    parts.append(&mut members);
    let package = zip_members(&parts)?;
    match &presentation.macro_project {
        Some(project) => crate::insert_macro_project(&package, project),
        None => Ok(package),
    }
}

/// The parts every written pptx has besides its slides and media.
fn package_parts(
    slide_count: usize,
    noted: &[usize],
    presentation_xml: String,
    presentation_relationships: &[(String, &'static str, String)],
) -> Vec<(String, Vec<u8>)> {
    vec![
        (
            "[Content_Types].xml".to_owned(),
            content_types(slide_count, noted).into_bytes(),
        ),
        (
            "_rels/.rels".to_owned(),
            relationships_xml(&[(
                "rId1".to_owned(),
                "officeDocument",
                "ppt/presentation.xml".to_owned(),
            )])
            .into_bytes(),
        ),
        (
            "ppt/presentation.xml".to_owned(),
            presentation_xml.into_bytes(),
        ),
        (
            "ppt/_rels/presentation.xml.rels".to_owned(),
            relationships_xml(presentation_relationships).into_bytes(),
        ),
        (
            "ppt/slideMasters/slideMaster1.xml".to_owned(),
            MASTER.as_bytes().to_vec(),
        ),
        (
            "ppt/slideMasters/_rels/slideMaster1.xml.rels".to_owned(),
            relationships_xml(&[
                (
                    "rId1".to_owned(),
                    "slideLayout",
                    "../slideLayouts/slideLayout1.xml".to_owned(),
                ),
                ("rId2".to_owned(), "theme", "../theme/theme1.xml".to_owned()),
            ])
            .into_bytes(),
        ),
        (
            "ppt/slideLayouts/slideLayout1.xml".to_owned(),
            LAYOUT.as_bytes().to_vec(),
        ),
        (
            "ppt/slideLayouts/_rels/slideLayout1.xml.rels".to_owned(),
            relationships_xml(&[(
                "rId1".to_owned(),
                "slideMaster",
                "../slideMasters/slideMaster1.xml".to_owned(),
            )])
            .into_bytes(),
        ),
        ("ppt/theme/theme1.xml".to_owned(), THEME.as_bytes().to_vec()),
    ]
}

/// The id of each shape on its slide: the one it was read with when it is unique, the next
/// free one otherwise. Id 1 is the shape tree's.
fn shape_ids(shapes: &[SlideShape]) -> Vec<u32> {
    let mut used: HashSet<u32> = HashSet::from([1]);
    let kept: Vec<Option<u32>> = shapes
        .iter()
        .map(|shape| shape.id.filter(|id| *id > 1 && used.insert(*id)))
        .collect();
    let mut next = 2;
    kept.into_iter()
        .map(|id| {
            id.unwrap_or_else(|| {
                while used.contains(&next) {
                    next += 1;
                }
                used.insert(next);
                next
            })
        })
        .collect()
}

/// The notes page of a slide: the slide's picture and the notes in the body placeholder.
fn notes_xml(notes: &TextBody) -> String {
    let mut body = String::new();
    text_body(notes, &mut body);
    format!(
        concat!(
            r#"{}<p:notes {}><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>"#,
            r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image"/><p:cNvSpPr><a:spLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvSpPr><p:nvPr><p:ph type="sldImg"/></p:nvPr></p:nvSpPr><p:spPr/></p:sp>"#,
            r#"<p:sp><p:nvSpPr><p:cNvPr id="3" name="Notes"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody>{}</p:txBody></p:sp>"#,
            r#"</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:notes>"#
        ),
        XML_HEADER, PRESENTATION_NAMESPACES, body
    )
}

/// The notes page of slide `number` and its relationships.
fn notes_parts(number: usize, notes: &TextBody) -> [(String, Vec<u8>); 2] {
    [
        (
            format!("ppt/notesSlides/notesSlide{number}.xml"),
            notes_xml(notes).into_bytes(),
        ),
        (
            format!("ppt/notesSlides/_rels/notesSlide{number}.xml.rels"),
            relationships_xml(&[
                (
                    "rId1".to_owned(),
                    "notesMaster",
                    "../notesMasters/notesMaster1.xml".to_owned(),
                ),
                (
                    "rId2".to_owned(),
                    "slide",
                    format!("../slides/slide{number}.xml"),
                ),
            ])
            .into_bytes(),
        ),
    ]
}

/// The notes master every notes page is drawn over, and its theme.
fn notes_master_parts() -> Vec<(String, Vec<u8>)> {
    vec![
        (
            "ppt/notesMasters/notesMaster1.xml".to_owned(),
            NOTES_MASTER.as_bytes().to_vec(),
        ),
        (
            "ppt/notesMasters/_rels/notesMaster1.xml.rels".to_owned(),
            relationships_xml(&[("rId1".to_owned(), "theme", "../theme/theme2.xml".to_owned())])
                .into_bytes(),
        ),
        ("ppt/theme/theme2.xml".to_owned(), THEME.as_bytes().to_vec()),
    ]
}

fn relationships_xml(relationships: &[(String, &str, String)]) -> String {
    let mut xml = format!(
        r#"{XML_HEADER}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#
    );
    for (id, kind, target) in relationships {
        let _ = write!(
            xml,
            r#"<Relationship Id="{id}" Type="{RELATIONSHIP_TYPE}/{kind}" Target="{}"/>"#,
            escape(target)
        );
    }
    xml.push_str("</Relationships>");
    xml
}

fn content_types(slides: usize, noted: &[usize]) -> String {
    let mut xml = format!(
        r#"{XML_HEADER}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>"#
    );
    for (extension, content_type) in [
        ("png", "image/png"),
        ("jpeg", "image/jpeg"),
        ("gif", "image/gif"),
        ("bmp", "image/bmp"),
        ("tiff", "image/tiff"),
        ("webp", "image/webp"),
        ("svg", "image/svg+xml"),
        ("emf", "image/x-emf"),
        ("wmf", "image/x-wmf"),
    ] {
        let _ = write!(
            xml,
            r#"<Default Extension="{extension}" ContentType="{content_type}"/>"#
        );
    }
    let main = "application/vnd.openxmlformats-officedocument.presentationml";
    let _ = write!(
        xml,
        r#"<Override PartName="/ppt/presentation.xml" ContentType="{main}.presentation.main+xml"/><Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="{main}.slideMaster+xml"/><Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="{main}.slideLayout+xml"/><Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>"#
    );
    for index in 1..=slides {
        let _ = write!(
            xml,
            r#"<Override PartName="/ppt/slides/slide{index}.xml" ContentType="{main}.slide+xml"/>"#
        );
    }
    if !noted.is_empty() {
        let _ = write!(
            xml,
            r#"<Override PartName="/ppt/notesMasters/notesMaster1.xml" ContentType="{main}.notesMaster+xml"/><Override PartName="/ppt/theme/theme2.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>"#
        );
    }
    for index in noted {
        let _ = write!(
            xml,
            r#"<Override PartName="/ppt/notesSlides/notesSlide{index}.xml" ContentType="{main}.notesSlide+xml"/>"#
        );
    }
    xml.push_str("</Types>");
    xml
}

struct SlideWriter<'a> {
    presentation: &'a Presentation,
    relationships: Vec<(String, &'static str, String)>,
    media_parts: &'a mut BTreeMap<usize, String>,
}

impl SlideWriter<'_> {
    fn slide(&mut self, slide: &Slide) -> String {
        let background = match slide.background.as_ref() {
            Some(paint) => {
                let fill = self.fill(Some(paint));
                if fill == "<a:noFill/>" {
                    String::new()
                } else {
                    format!(r"<p:bg><p:bgPr>{fill}<a:effectLst/></p:bgPr></p:bg>")
                }
            }
            None => String::new(),
        };
        let ids = shape_ids(&slide.shapes);
        let mut shapes = String::new();
        for (shape, id) in slide.shapes.iter().zip(&ids) {
            self.shape(shape, *id, &mut shapes);
        }
        let hidden = if slide.hidden { r#" show="0""# } else { "" };
        let transition = slide
            .transition
            .as_ref()
            .map(transition_xml)
            .unwrap_or_default();
        let written: Vec<Option<u32>> = ids.iter().copied().map(Some).collect();
        let timing = timing_xml(&slide.animations, &written);
        format!(
            r#"{XML_HEADER}<p:sld {PRESENTATION_NAMESPACES}{hidden}><p:cSld>{background}<p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>{transition}{timing}</p:sld>"#
        )
    }

    fn shape(&mut self, shape: &SlideShape, id: u32, xml: &mut String) {
        match &shape.content {
            ShapeContent::Picture { media } => self.picture(shape, *media, id, xml),
            ShapeContent::Table(table) => table_frame(shape, table, id, xml),
            ShapeContent::Text(_) | ShapeContent::None => {
                let fill = self.fill(shape.fill.as_ref());
                let _ = write!(
                    xml,
                    r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="Forma {id}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>{}{}{}{}</p:spPr>"#,
                    transform(shape),
                    geometry(&shape.geometry, shape.frame),
                    fill,
                    line(
                        shape.line.as_ref(),
                        matches!(shape.geometry, ShapeGeometry::Line { .. })
                    )
                );
                if let ShapeContent::Text(body) = &shape.content {
                    xml.push_str("<p:txBody>");
                    text_body(body, xml);
                    xml.push_str("</p:txBody>");
                }
                xml.push_str("</p:sp>");
            }
        }
    }

    /// The `DrawingML` fill of a paint; a picture `PowerPoint` cannot show is no fill.
    fn fill(&mut self, paint: Option<&Paint>) -> String {
        match paint {
            None => "<a:noFill/>".to_owned(),
            Some(Paint::Solid(colour)) => solid_fill(colour),
            Some(Paint::Gradient(gradient)) => gradient_fill(gradient),
            Some(Paint::Picture { media, tile_emu }) => {
                let Some(relationship) = self.image_relationship(*media) else {
                    return "<a:noFill/>".to_owned();
                };
                let mode = match tile_emu {
                    Some((width, height)) => {
                        let scale = |side: i64, pixels: Option<u32>| {
                            pixels.map_or(100_000, |pixels| {
                                let natural = i64::from(pixels.max(1)) * 9_525;
                                (side.max(1) * 100_000 / natural).clamp(1, 100_000_000)
                            })
                        };
                        let pixels = self
                            .presentation
                            .media
                            .get(*media)
                            .and_then(|item| image_pixel_size(&item.bytes));
                        format!(
                            r#"<a:tile tx="0" ty="0" sx="{}" sy="{}" flip="none" algn="tl"/>"#,
                            scale(*width, pixels.map(|(width, _)| width)),
                            scale(*height, pixels.map(|(_, height)| height))
                        )
                    }
                    None => "<a:stretch><a:fillRect/></a:stretch>".to_owned(),
                };
                format!(r#"<a:blipFill><a:blip r:embed="{relationship}"/>{mode}</a:blipFill>"#)
            }
        }
    }

    /// The relationship of the slide to an image's part, the part written once; `None` for an
    /// image `PowerPoint` cannot show.
    fn image_relationship(&mut self, media: usize) -> Option<String> {
        let extension = self
            .presentation
            .media
            .get(media)
            .and_then(|item| image_extension(item.content_type))?;
        let count = self.media_parts.len();
        let part = self
            .media_parts
            .entry(media)
            .or_insert_with(|| format!("media/image{}.{extension}", count + 1))
            .clone();
        let target = format!("../{part}");
        if let Some((known, _, _)) = self
            .relationships
            .iter()
            .find(|(_, kind, existing)| *kind == "image" && *existing == target)
        {
            return Some(known.clone());
        }
        let relationship = format!("rId{}", self.relationships.len() + 1);
        self.relationships
            .push((relationship.clone(), "image", target));
        Some(relationship)
    }

    fn picture(&mut self, shape: &SlideShape, media: usize, id: u32, xml: &mut String) {
        let Some(relationship) = self.image_relationship(media) else {
            let empty = SlideShape {
                id: None,
                inherited: false,
                content: ShapeContent::None,
                line: shape.line.clone().or(Some(ShapeLine {
                    colour: "BFBFBF".to_owned(),
                    width_emu: 6_350,
                })),
                ..shape.clone()
            };
            self.shape(&empty, id, xml);
            return;
        };
        let _ = write!(
            xml,
            r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="Immagine {id}"/><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="{relationship}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>{}</p:spPr></p:pic>"#,
            transform(shape),
            line(shape.line.as_ref(), false)
        );
    }
}

fn transform(shape: &SlideShape) -> String {
    let frame = shape.frame;
    let rotation = if shape.rotation.abs() > f64::EPSILON {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a turn is 21.6 million units"
        )]
        let units = (shape.rotation.rem_euclid(360.0) * 60_000.0).round() as i64;
        format!(r#" rot="{units}""#)
    } else {
        String::new()
    };
    let flip = if matches!(shape.geometry, ShapeGeometry::Line { flipped: true }) {
        r#" flipV="1""#
    } else {
        ""
    };
    format!(
        r#"<a:xfrm{rotation}{flip}><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm>"#,
        frame.x,
        frame.y,
        frame.width.max(0),
        frame.height.max(0)
    )
}

fn geometry(geometry: &ShapeGeometry, frame: Frame) -> String {
    let preset = |name: &str| format!(r#"<a:prstGeom prst="{name}"><a:avLst/></a:prstGeom>"#);
    match geometry {
        ShapeGeometry::Rectangle | ShapeGeometry::Other => preset("rect"),
        ShapeGeometry::RoundedRectangle => preset("roundRect"),
        ShapeGeometry::Ellipse => preset("ellipse"),
        ShapeGeometry::Line { .. } => preset("line"),
        ShapeGeometry::Path(paths) => custom_geometry(paths, frame.width, frame.height),
    }
}

/// A path geometry as an `a:custGeom` in the frame's own units.
fn custom_geometry(paths: &[ShapePath], width: i64, height: i64) -> String {
    let point = |point: &PathPoint| format!(r#"<a:pt x="{}" y="{}"/>"#, point.x, point.y);
    let (width, height) = (width.max(1), height.max(1));
    let mut xml = String::from(
        r#"<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect l="l" t="t" r="r" b="b"/><a:pathLst>"#,
    );
    for path in paths {
        let fill = match (path.filled, path.shade) {
            (false, _) => r#" fill="none""#.to_owned(),
            (true, FillShade::Normal) => String::new(),
            (true, shade) => format!(r#" fill="{}""#, shade.drawingml_name()),
        };
        let stroke = if path.stroked { "" } else { r#" stroke="0""# };
        let _ = write!(xml, r#"<a:path w="{width}" h="{height}"{fill}{stroke}>"#);
        for command in &path.commands {
            let _ = match command {
                PathCommand::MoveTo(to) => write!(xml, "<a:moveTo>{}</a:moveTo>", point(to)),
                PathCommand::LineTo(to) => write!(xml, "<a:lnTo>{}</a:lnTo>", point(to)),
                PathCommand::CubicTo(first, second, end) => write!(
                    xml,
                    "<a:cubicBezTo>{}{}{}</a:cubicBezTo>",
                    point(first),
                    point(second),
                    point(end)
                ),
                PathCommand::Close => write!(xml, "<a:close/>"),
            };
        }
        xml.push_str("</a:path>");
    }
    xml.push_str("</a:pathLst></a:custGeom>");
    xml
}

/// A colour as `DrawingML` takes it, six hexadecimal digits; black for anything else, which a
/// model edited outside this library may hold.
fn rgb(colour: &str) -> &str {
    if colour.len() == 6 && colour.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        colour
    } else {
        "000000"
    }
}

fn solid_fill(colour: &str) -> String {
    format!(
        r#"<a:solidFill><a:srgbClr val="{}"/></a:solidFill>"#,
        rgb(colour)
    )
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "offsets are fractions and angles a turn at most"
)]
fn gradient_fill(gradient: &Gradient) -> String {
    let mut xml = String::from(r#"<a:gradFill rotWithShape="1"><a:gsLst>"#);
    for stop in &gradient.stops {
        let _ = write!(
            xml,
            r#"<a:gs pos="{}"><a:srgbClr val="{}"/></a:gs>"#,
            (stop.offset.clamp(0.0, 1.0) * 100_000.0).round() as i64,
            rgb(&stop.colour)
        );
    }
    xml.push_str("</a:gsLst>");
    match gradient.kind {
        GradientKind::Linear { angle } => {
            let _ = write!(
                xml,
                r#"<a:lin ang="{}" scaled="0"/>"#,
                (angle.rem_euclid(360.0) * 60_000.0).round() as i64
            );
        }
        GradientKind::Radial { centre: (x, y) } => {
            let inset = |value: f64| (value.clamp(0.0, 1.0) * 100_000.0).round() as i64;
            let _ = write!(
                xml,
                r#"<a:path path="circle"><a:fillToRect l="{}" t="{}" r="{}" b="{}"/></a:path>"#,
                inset(x),
                inset(y),
                inset(1.0 - x),
                inset(1.0 - y)
            );
        }
    }
    xml.push_str("</a:gradFill>");
    xml
}

fn line(line: Option<&ShapeLine>, is_line: bool) -> String {
    match line {
        Some(line) => format!(
            r#"<a:ln w="{}"><a:solidFill><a:srgbClr val="{}"/></a:solidFill></a:ln>"#,
            line.width_emu.max(0),
            rgb(&line.colour)
        ),
        None if is_line => String::new(),
        None => "<a:ln><a:noFill/></a:ln>".to_owned(),
    }
}

fn text_body(body: &TextBody, xml: &mut String) {
    let [left, top, right, bottom] = body.insets_emu;
    let anchor = match body.anchor {
        TextAnchor::Top => "t",
        TextAnchor::Middle => "ctr",
        TextAnchor::Bottom => "b",
    };
    let wrap = if body.wrap { "square" } else { "none" };
    let _ = write!(
        xml,
        r#"<a:bodyPr wrap="{wrap}" lIns="{left}" tIns="{top}" rIns="{right}" bIns="{bottom}" anchor="{anchor}"><a:noAutofit/></a:bodyPr><a:lstStyle/>"#
    );
    if body.paragraphs.is_empty() {
        xml.push_str("<a:p/>");
    }
    for paragraph in &body.paragraphs {
        paragraph_xml(paragraph, xml);
    }
}

fn paragraph_xml(paragraph: &SlideParagraph, xml: &mut String) {
    let align = match paragraph.align {
        Alignment::Left => "l",
        Alignment::Center => "ctr",
        Alignment::Right => "r",
        Alignment::Justify => "just",
    };
    let _ = write!(
        xml,
        r#"<a:p><a:pPr marL="{}" indent="{}" lvl="{}" algn="{align}"><a:lnSpc><a:spcPct val="{}"/></a:lnSpc><a:spcBef><a:spcPts val="{}"/></a:spcBef><a:spcAft><a:spcPts val="{}"/></a:spcAft>"#,
        paragraph.margin_left_emu.max(0),
        paragraph.indent_emu,
        paragraph.level.min(8),
        paragraph.line_spacing_percent.min(1_000) * 1_000,
        paragraph.space_before.min(158_400),
        paragraph.space_after.min(158_400)
    );
    match &paragraph.bullet {
        Some(Bullet::Character(character)) => {
            let _ = write!(xml, r#"<a:buChar char="{}"/>"#, escape(character));
        }
        Some(Bullet::Number(_)) => xml.push_str(r#"<a:buAutoNum type="arabicPeriod"/>"#),
        None => xml.push_str("<a:buNone/>"),
    }
    xml.push_str("</a:pPr>");
    for run in &paragraph.runs {
        run_xml(run, xml);
    }
    if let Some(last) = paragraph.runs.last() {
        let _ = write!(xml, r#"<a:endParaRPr lang="it-IT" sz="{}"/>"#, size(last));
    }
    xml.push_str("</a:p>");
}

fn size(run: &SlideRun) -> u32 {
    run.size.clamp(100, 400_000)
}

fn run_xml(run: &SlideRun, xml: &mut String) {
    let properties = format!(
        r#"lang="it-IT" sz="{}" b="{}" i="{}" u="{}" strike="{}""#,
        size(run),
        u8::from(run.bold),
        u8::from(run.italic),
        if run.underline { "sng" } else { "none" },
        if run.strike { "sngStrike" } else { "noStrike" }
    );
    let look = format!(
        r#"<a:solidFill><a:srgbClr val="{}"/></a:solidFill><a:latin typeface="{}"/>"#,
        rgb(&run.colour),
        escape(&run.family)
    );
    if run.line_break {
        let _ = write!(xml, "<a:br><a:rPr {properties}>{look}</a:rPr></a:br>");
        return;
    }
    let _ = write!(
        xml,
        r#"<a:r><a:rPr {properties} dirty="0">{look}</a:rPr><a:t>{}</a:t></a:r>"#,
        escape(&run.text)
    );
}

fn table_frame(shape: &SlideShape, table: &SlideTable, id: u32, xml: &mut String) {
    let frame = shape.frame;
    let _ = write!(
        xml,
        r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{id}" name="Tabella {id}"/><p:cNvGraphicFramePr><a:graphicFrameLocks noGrp="1"/></p:cNvGraphicFramePr><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></p:xfrm><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table"><a:tbl><a:tblPr firstRow="1" bandRow="1"/><a:tblGrid>"#,
        frame.x,
        frame.y,
        frame.width.max(0),
        frame.height.max(0)
    );
    for width in &table.column_widths_emu {
        let _ = write!(xml, r#"<a:gridCol w="{}"/>"#, (*width).max(0));
    }
    xml.push_str("</a:tblGrid>");
    for row in &table.rows {
        let _ = write!(xml, r#"<a:tr h="{}">"#, row.height_emu.max(0));
        for cell in &row.cells {
            let mut merge = String::new();
            if cell.col_span > 1 {
                let _ = write!(merge, r#" gridSpan="{}""#, cell.col_span);
            }
            if cell.row_span > 1 {
                let _ = write!(merge, r#" rowSpan="{}""#, cell.row_span);
            }
            if cell.covered {
                merge.push_str(r#" hMerge="1""#);
            }
            let _ = write!(xml, "<a:tc{merge}><a:txBody>");
            text_body(&cell.body, xml);
            let _ = write!(
                xml,
                "</a:txBody><a:tcPr>{}</a:tcPr></a:tc>",
                cell.fill
                    .as_deref()
                    .map_or_else(|| "<a:noFill/>".to_owned(), solid_fill)
            );
        }
        xml.push_str("</a:tr>");
    }
    xml.push_str("</a:tbl></a:graphicData></a:graphic></p:graphicFrame>");
}

const NOTES_MASTER: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<p:notesMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
    r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#,
    r#"xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">"#,
    r#"<p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>"#,
    r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image"/><p:cNvSpPr><a:spLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvSpPr><p:nvPr><p:ph type="sldImg" idx="2"/></p:nvPr></p:nvSpPr>"#,
    r#"<p:spPr><a:xfrm><a:off x="1143000" y="685800"/><a:ext cx="4572000" cy="3429000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln w="12700"><a:solidFill><a:prstClr val="black"/></a:solidFill></a:ln></p:spPr></p:sp>"#,
    r#"<p:sp><p:nvSpPr><p:cNvPr id="3" name="Notes"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" sz="quarter" idx="3"/></p:nvPr></p:nvSpPr>"#,
    r#"<p:spPr><a:xfrm><a:off x="685800" y="4343400"/><a:ext cx="5486400" cy="4114800"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>"#,
    r#"<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr/></a:p></p:txBody></p:sp></p:spTree></p:cSld>"#,
    r#"<p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>"#,
    r#"<p:notesStyle><a:lvl1pPr marL="0" algn="l"><a:defRPr sz="1200"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mn-lt"/></a:defRPr></a:lvl1pPr></p:notesStyle>"#,
    "</p:notesMaster>"
);

const MASTER: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
    r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#,
    r#"xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">"#,
    r#"<p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg>"#,
    r#"<p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld>"#,
    r#"<p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" "#,
    r#"accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>"#,
    r#"<p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst>"#,
    r#"<p:txStyles><p:titleStyle/><p:bodyStyle/><p:otherStyle/></p:txStyles></p:sldMaster>"#
);

const LAYOUT: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
    r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#,
    r#"xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="blank" preserve="1">"#,
    r#"<p:cSld name="Vuota"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>"#,
    r#"<p:grpSpPr/></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#
);

const THEME: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Rely Office">"#,
    r#"<a:themeElements><a:clrScheme name="Rely Office">"#,
    r#"<a:dk1><a:srgbClr val="000000"/></a:dk1><a:lt1><a:srgbClr val="FFFFFF"/></a:lt1>"#,
    r#"<a:dk2><a:srgbClr val="44546A"/></a:dk2><a:lt2><a:srgbClr val="E7E6E6"/></a:lt2>"#,
    r#"<a:accent1><a:srgbClr val="4472C4"/></a:accent1><a:accent2><a:srgbClr val="ED7D31"/></a:accent2>"#,
    r#"<a:accent3><a:srgbClr val="A5A5A5"/></a:accent3><a:accent4><a:srgbClr val="FFC000"/></a:accent4>"#,
    r#"<a:accent5><a:srgbClr val="5B9BD5"/></a:accent5><a:accent6><a:srgbClr val="70AD47"/></a:accent6>"#,
    r#"<a:hlink><a:srgbClr val="0563C1"/></a:hlink><a:folHlink><a:srgbClr val="954F72"/></a:folHlink></a:clrScheme>"#,
    r#"<a:fontScheme name="Rely Office"><a:majorFont><a:latin typeface="Calibri Light"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont>"#,
    r#"<a:minorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme>"#,
    r#"<a:fmtScheme name="Rely Office"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill>"#,
    r#"<a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst>"#,
    r#"<a:lnStyleLst><a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>"#,
    r#"<a:ln w="12700"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>"#,
    r#"<a:ln w="19050"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst>"#,
    r#"<a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle>"#,
    r#"<a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill>"#,
    r#"<a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst>"#,
    r#"</a:fmtScheme></a:themeElements></a:theme>"#
);

#[cfg(test)]
mod tests;
