// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! A presentation written as a pptx: one slide part per slide, each shape with its frame,
//! outline, fill, text, picture or table set on the slide itself, over a blank layout and
//! master, so the file shows what the presentation held whatever it was read from.

use std::collections::BTreeMap;
use std::fmt::Write;

use super::{
    Bullet, Frame, Presentation, ShapeContent, ShapeGeometry, ShapeLine, Slide, SlideParagraph,
    SlideRun, SlideShape, SlideTable, TextAnchor, TextBody,
};
use crate::OfficeError;
use crate::document::{Alignment, escape_xml as escape, image_extension, zip_members};
use crate::geometry::{PathCommand, PathPoint, ShapePath};

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
            next_id: 2,
        };
        let xml = writer.slide(slide);
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
    let presentation_xml = format!(
        r#"{XML_HEADER}<p:presentation {PRESENTATION_NAMESPACES}><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst><p:sldIdLst>{slide_ids}</p:sldIdLst><p:sldSz cx="{width}" cy="{height}"/><p:notesSz cx="{height}" cy="{width}"/><p:defaultTextStyle/></p:presentation>"#
    );
    let mut parts = package_parts(slide_count, presentation_xml, &presentation_relationships);
    parts.append(&mut members);
    zip_members(&parts)
}

/// The parts every written pptx has besides its slides and media.
fn package_parts(
    slide_count: usize,
    presentation_xml: String,
    presentation_relationships: &[(String, &'static str, String)],
) -> Vec<(String, Vec<u8>)> {
    vec![
        (
            "[Content_Types].xml".to_owned(),
            content_types(slide_count).into_bytes(),
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

fn content_types(slides: usize) -> String {
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
    xml.push_str("</Types>");
    xml
}

struct SlideWriter<'a> {
    presentation: &'a Presentation,
    relationships: Vec<(String, &'static str, String)>,
    media_parts: &'a mut BTreeMap<usize, String>,
    next_id: u32,
}

impl SlideWriter<'_> {
    fn slide(&mut self, slide: &Slide) -> String {
        let background = slide.background.as_ref().map_or_else(String::new, |colour| {
            format!(
                r#"<p:bg><p:bgPr><a:solidFill><a:srgbClr val="{}"/></a:solidFill><a:effectLst/></p:bgPr></p:bg>"#,
                escape(colour)
            )
        });
        let mut shapes = String::new();
        for shape in &slide.shapes {
            self.shape(shape, &mut shapes);
        }
        format!(
            r#"{XML_HEADER}<p:sld {PRESENTATION_NAMESPACES}><p:cSld>{background}<p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#
        )
    }

    fn id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn shape(&mut self, shape: &SlideShape, xml: &mut String) {
        let id = self.id();
        match &shape.content {
            ShapeContent::Picture { media } => self.picture(shape, *media, id, xml),
            ShapeContent::Table(table) => table_frame(shape, table, id, xml),
            ShapeContent::Text(_) | ShapeContent::None => {
                let _ = write!(
                    xml,
                    r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="Forma {id}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>{}{}{}{}</p:spPr>"#,
                    transform(shape),
                    geometry(&shape.geometry, shape.frame),
                    fill(shape.fill.as_deref()),
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

    fn picture(&mut self, shape: &SlideShape, media: usize, id: u32, xml: &mut String) {
        let extension = self
            .presentation
            .media
            .get(media)
            .and_then(|item| image_extension(item.content_type));
        let Some(extension) = extension else {
            let empty = SlideShape {
                content: ShapeContent::None,
                line: shape.line.clone().or(Some(ShapeLine {
                    colour: "BFBFBF".to_owned(),
                    width_emu: 6_350,
                })),
                ..shape.clone()
            };
            self.next_id -= 1;
            self.shape(&empty, xml);
            return;
        };
        let count = self.media_parts.len();
        let part = self
            .media_parts
            .entry(media)
            .or_insert_with(|| format!("media/image{}.{extension}", count + 1))
            .clone();
        let relationship = format!("rId{}", self.relationships.len() + 1);
        self.relationships
            .push((relationship.clone(), "image", format!("../{part}")));
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
        let fill = if path.filled { "" } else { r#" fill="none""# };
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

fn fill(colour: Option<&str>) -> String {
    colour.map_or_else(
        || "<a:noFill/>".to_owned(),
        |colour| {
            format!(
                r#"<a:solidFill><a:srgbClr val="{}"/></a:solidFill>"#,
                escape(colour)
            )
        },
    )
}

fn line(line: Option<&ShapeLine>, is_line: bool) -> String {
    match line {
        Some(line) => format!(
            r#"<a:ln w="{}"><a:solidFill><a:srgbClr val="{}"/></a:solidFill></a:ln>"#,
            line.width_emu.max(0),
            escape(&line.colour)
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
        escape(&run.colour),
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
                fill(cell.fill.as_deref())
            );
        }
        xml.push_str("</a:tr>");
    }
    xml.push_str("</a:tbl></a:graphicData></a:graphic></p:graphicFrame>");
}

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
