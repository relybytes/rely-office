// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Slides written as an `OpenDocument` presentation, the odp `LibreOffice` Impress opens and the
//! reader of this crate reads back.
//!
//! Each slide is a page on one empty master: what a slide drew from its layout or master is
//! written on the slide itself, so it looks the same. A shape is a custom shape of its
//! geometry, a text frame, a line, a picture or a table frame; its fill (a colour, a gradient
//! or a picture), its line and the place of its text are an automatic graphic style. Speaker
//! notes, hidden slides, transitions and animations are written as Impress keeps them.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{Cursor, Write as _};

use zip::write::SimpleFileOptions;

use crate::OfficeError;
use crate::document::Alignment;
use crate::geometry::{FillShade, PathCommand};
use crate::slides::{
    Animation, AnimationClass, AnimationTrigger, Bullet, Frame, GradientKind, Paint, Presentation,
    ShapeContent, ShapeGeometry, ShapeLine, Slide, SlideParagraph, SlideShape, SlideTable,
    TextAnchor, TextBody, Transition,
};

const NAMESPACES: &str = concat!(
    r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
    r#"xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" "#,
    r#"xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" "#,
    r#"xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" "#,
    r#"xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" "#,
    r#"xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" "#,
    r#"xmlns:xlink="http://www.w3.org/1999/xlink" "#,
    r#"xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" "#,
    r#"xmlns:presentation="urn:oasis:names:tc:opendocument:xmlns:presentation:1.0" "#,
    r#"xmlns:smil="urn:oasis:names:tc:opendocument:xmlns:smil-compatible:1.0" "#,
    r#"xmlns:anim="urn:oasis:names:tc:opendocument:xmlns:animation:1.0" "#,
    r#"xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0" "#,
    r#"office:version="1.3""#,
);

const EMU_PER_POINT: f64 = 12_700.0;

/// Automatic styles written once and named by everything that shares them.
#[derive(Default)]
struct StyleRegistry {
    xml: String,
    names: HashMap<String, String>,
    count: usize,
}

impl StyleRegistry {
    fn name(&mut self, prefix: &str, key: String, declare: impl FnOnce(&str) -> String) -> String {
        if let Some(name) = self.names.get(&key) {
            return name.clone();
        }
        self.count += 1;
        let name = format!("{prefix}{}", self.count);
        self.xml.push_str(&declare(&name));
        self.names.insert(key, name.clone());
        name
    }
}

struct OdpWriter<'p> {
    presentation: &'p Presentation,
    styles: StyleRegistry,
    /// Gradients and fill images, which `OpenDocument` keeps among the named styles.
    named: StyleRegistry,
    pictures: Vec<Option<String>>,
    picture_files: Vec<(String, &'static str, &'p [u8])>,
    shapes: usize,
}

/// A presentation as an odp.
///
/// # Errors
///
/// [`OfficeError::DamagedContainer`] when the package cannot be assembled.
pub fn write_odp(presentation: &Presentation) -> Result<Vec<u8>, OfficeError> {
    let mut writer = OdpWriter {
        presentation,
        styles: StyleRegistry::default(),
        named: StyleRegistry::default(),
        pictures: vec![None; presentation.media.len()],
        picture_files: Vec::new(),
        shapes: 0,
    };
    let mut pages = String::new();
    for (index, slide) in presentation.slides.iter().enumerate() {
        pages.push_str(&writer.page(slide, index + 1));
    }
    let content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content {NAMESPACES}><office:automatic-styles>{}</office:automatic-styles><office:body><office:presentation>{pages}</office:presentation></office:body></office:document-content>"#,
        writer.styles.xml
    );
    let width = presentation.width_emu.max(91_440);
    let height = presentation.height_emu.max(91_440);
    let styles = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-styles {NAMESPACES}><office:styles>{}<style:default-style style:family="graphic"><style:graphic-properties draw:stroke="none" draw:fill="none"/><style:text-properties fo:font-family="'Liberation Sans'" fo:font-size="18pt"/></style:default-style></office:styles><office:automatic-styles><style:page-layout style:name="PM1"><style:page-layout-properties fo:margin-top="0pt" fo:margin-bottom="0pt" fo:margin-left="0pt" fo:margin-right="0pt" fo:page-width="{}" fo:page-height="{}" style:print-orientation="{}"/></style:page-layout><style:style style:name="dpm" style:family="drawing-page"><style:drawing-page-properties draw:fill="none"/></style:style></office:automatic-styles><office:master-styles><style:master-page style:name="Default" style:page-layout-name="PM1" draw:style-name="dpm"/></office:master-styles></office:document-styles>"#,
        writer.named.xml,
        emu_length(width),
        emu_length(height),
        if width > height {
            "landscape"
        } else {
            "portrait"
        }
    );
    let mut manifest = String::from(concat!(
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        r#"<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.3">"#,
        r#"<manifest:file-entry manifest:full-path="/" manifest:version="1.3" manifest:media-type="application/vnd.oasis.opendocument.presentation"/>"#,
        r#"<manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>"#,
        r#"<manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/>"#,
        r#"<manifest:file-entry manifest:full-path="meta.xml" manifest:media-type="text/xml"/>"#,
    ));
    for (path, content_type, _) in &writer.picture_files {
        let _ = write!(
            manifest,
            r#"<manifest:file-entry manifest:full-path="{path}" manifest:media-type="{content_type}"/>"#
        );
    }
    manifest.push_str("</manifest:manifest>");
    let meta = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-meta {NAMESPACES}><office:meta><meta:generator>rely-office/{}</meta:generator></office:meta></office:document-meta>"#,
        env!("CARGO_PKG_VERSION")
    );
    let mut members: Vec<(String, Vec<u8>)> = vec![
        ("content.xml".to_owned(), content.into_bytes()),
        ("styles.xml".to_owned(), styles.into_bytes()),
        ("meta.xml".to_owned(), meta.into_bytes()),
        ("META-INF/manifest.xml".to_owned(), manifest.into_bytes()),
    ];
    for (path, _, bytes) in &writer.picture_files {
        members.push((path.clone(), bytes.to_vec()));
    }
    package(&members)
}

fn package(members: &[(String, Vec<u8>)]) -> Result<Vec<u8>, OfficeError> {
    let damaged = |error: &dyn std::fmt::Display| OfficeError::DamagedContainer(error.to_string());
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(
        "mimetype",
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )
    .map_err(|error| damaged(&error))?;
    zip.write_all(b"application/vnd.oasis.opendocument.presentation")
        .map_err(|error| damaged(&error))?;
    for (name, bytes) in members {
        zip.start_file(name.as_str(), SimpleFileOptions::default())
            .map_err(|error| damaged(&error))?;
        zip.write_all(bytes).map_err(|error| damaged(&error))?;
    }
    zip.finish()
        .map(Cursor::into_inner)
        .map_err(|error| damaged(&error))
}

impl OdpWriter<'_> {
    fn page(&mut self, slide: &Slide, number: usize) -> String {
        let style = self.page_style(slide);
        let animated: Vec<usize> = slide
            .animations
            .iter()
            .map(|animation| animation.shape)
            .collect();
        let mut shapes = String::new();
        for (index, shape) in slide.shapes.iter().enumerate() {
            let id = animated
                .contains(&index)
                .then(|| format!("id{number}_{index}"));
            shapes.push_str(&self.shape(shape, id.as_deref()));
        }
        let notes = slide
            .notes
            .as_ref()
            .map(|notes| self.notes(notes))
            .unwrap_or_default();
        let animations = animations_xml(&slide.animations, number);
        format!(
            r#"<draw:page draw:name="page{number}" draw:style-name="{style}" draw:master-page-name="Default">{shapes}{animations}{notes}</draw:page>"#
        )
    }

    fn page_style(&mut self, slide: &Slide) -> String {
        let mut properties = match &slide.background {
            Some(paint) => self.fill_attributes(paint),
            None => r#" draw:fill="none""#.to_owned(),
        };
        if slide.hidden {
            properties.push_str(r#" presentation:visibility="hidden""#);
        }
        if let Some(transition) = &slide.transition {
            properties.push_str(&transition_attributes(transition));
        }
        properties.push_str(r#" presentation:background-visible="true" presentation:background-objects-visible="true""#);
        self.styles.name("dp", format!("page|{properties}"), |name| {
            format!(
                r#"<style:style style:name="{name}" style:family="drawing-page"><style:drawing-page-properties{properties}/></style:style>"#
            )
        })
    }

    fn notes(&mut self, notes: &TextBody) -> String {
        let width = self.presentation.width_emu.max(91_440);
        let height = self.presentation.height_emu.max(91_440);
        let frame = Frame {
            x: width / 10,
            y: height / 2,
            width: width * 8 / 10,
            height: height * 4 / 10,
        };
        let style = self.graphic_style(None, None, Some(notes));
        let text = self.paragraphs(&notes.paragraphs);
        format!(
            r#"<presentation:notes><draw:frame presentation:class="notes" draw:style-name="{style}"{}><draw:text-box>{text}</draw:text-box></draw:frame></presentation:notes>"#,
            placement(&frame, 0.0)
        )
    }

    fn shape(&mut self, shape: &SlideShape, id: Option<&str>) -> String {
        self.shapes += 1;
        let identity = id
            .map(|id| format!(r#" draw:id="{id}" xml:id="{id}""#))
            .unwrap_or_default();
        let name = format!(r#" draw:name="Forma {}""#, self.shapes);
        let text = match &shape.content {
            ShapeContent::Text(body) => Some(body),
            _ => None,
        };
        let style = self.graphic_style(shape.fill.as_ref(), shape.line.as_ref(), text);
        match (&shape.content, &shape.geometry) {
            (ShapeContent::Picture { media }, _) => {
                let Some(path) = self.picture_path(*media) else {
                    return String::new();
                };
                format!(
                    r#"<draw:frame draw:style-name="{style}"{name}{identity}{}><draw:image xlink:href="{path}" xlink:type="simple" xlink:show="embed" xlink:actuate="onLoad"/></draw:frame>"#,
                    placement(&shape.frame, shape.rotation)
                )
            }
            (ShapeContent::Table(table), _) => {
                let content = self.table(table);
                format!(
                    r#"<draw:frame draw:style-name="{style}"{name}{identity}{}>{content}</draw:frame>"#,
                    placement(&shape.frame, shape.rotation)
                )
            }
            (_, ShapeGeometry::Line { flipped }) => {
                let frame = shape.frame;
                let (y1, y2) = if *flipped {
                    (frame.y + frame.height, frame.y)
                } else {
                    (frame.y, frame.y + frame.height)
                };
                format!(
                    r#"<draw:line draw:style-name="{style}"{name}{identity} svg:x1="{}" svg:y1="{}" svg:x2="{}" svg:y2="{}"/>"#,
                    emu_length(frame.x),
                    emu_length(y1),
                    emu_length(frame.x + frame.width),
                    emu_length(y2)
                )
            }
            (ShapeContent::Text(body), ShapeGeometry::Rectangle | ShapeGeometry::Other) => {
                let text = self.paragraphs(&body.paragraphs);
                format!(
                    r#"<draw:frame draw:style-name="{style}"{name}{identity}{}><draw:text-box>{text}</draw:text-box></draw:frame>"#,
                    placement(&shape.frame, shape.rotation)
                )
            }
            (content, geometry) => {
                let text = match content {
                    ShapeContent::Text(body) => self.paragraphs(&body.paragraphs),
                    _ => String::new(),
                };
                let geometry = enhanced_geometry(geometry, &shape.frame);
                format!(
                    r#"<draw:custom-shape draw:style-name="{style}"{name}{identity}{}>{text}{geometry}</draw:custom-shape>"#,
                    placement(&shape.frame, shape.rotation)
                )
            }
        }
    }

    /// The graphic style of a shape: its fill, its line and the place of its text.
    fn graphic_style(
        &mut self,
        fill: Option<&Paint>,
        line: Option<&ShapeLine>,
        text: Option<&TextBody>,
    ) -> String {
        let mut properties = match fill {
            Some(paint) => self.fill_attributes(paint),
            None => r#" draw:fill="none""#.to_owned(),
        };
        match line {
            Some(line) => {
                let _ = write!(
                    properties,
                    r##" draw:stroke="solid" svg:stroke-color="#{}" svg:stroke-width="{}""##,
                    hex(&line.colour),
                    emu_length(line.width_emu)
                );
            }
            None => properties.push_str(r#" draw:stroke="none""#),
        }
        if let Some(body) = text {
            let anchor = match body.anchor {
                TextAnchor::Top => "top",
                TextAnchor::Middle => "middle",
                TextAnchor::Bottom => "bottom",
            };
            let [left, top, right, bottom] = body.insets_emu;
            let _ = write!(
                properties,
                r#" draw:textarea-vertical-align="{anchor}" fo:padding-left="{}" fo:padding-top="{}" fo:padding-right="{}" fo:padding-bottom="{}""#,
                emu_length(left),
                emu_length(top),
                emu_length(right),
                emu_length(bottom)
            );
            if !body.wrap {
                properties.push_str(r#" fo:wrap-option="no-wrap""#);
            }
        }
        self.styles.name("gr", format!("graphic|{properties}"), |name| {
            format!(
                r#"<style:style style:name="{name}" style:family="graphic"><style:graphic-properties{properties}/></style:style>"#
            )
        })
    }

    /// The attributes of a fill: a colour, a gradient among the named styles, or a picture.
    fn fill_attributes(&mut self, paint: &Paint) -> String {
        match paint {
            Paint::Solid(colour) => {
                format!(r##" draw:fill="solid" draw:fill-color="#{}""##, hex(colour))
            }
            Paint::Gradient(gradient) => {
                let start = gradient
                    .stops
                    .first()
                    .map_or("FFFFFF", |stop| stop.colour.as_str());
                let end = gradient
                    .stops
                    .last()
                    .map_or(start, |stop| stop.colour.as_str());
                let definition = match gradient.kind {
                    GradientKind::Linear { angle } => {
                        // `OpenDocument` turns counter-clockwise from a gradient starting at the top.
                        let border = gradient
                            .stops
                            .get(1)
                            .filter(|stop| gradient.stops.len() > 2 && stop.colour == start)
                            .map_or(0.0, |stop| stop.offset);
                        format!(
                            r##"draw:style="linear" draw:start-color="#{}" draw:end-color="#{}" draw:angle="{}deg" draw:border="{}%""##,
                            hex(start),
                            hex(end),
                            trim_number((90.0 - angle).rem_euclid(360.0)),
                            trim_number(border * 100.0)
                        )
                    }
                    GradientKind::Radial { centre } => {
                        let border = gradient
                            .stops
                            .iter()
                            .rev()
                            .nth(1)
                            .filter(|_| gradient.stops.len() > 2)
                            .map_or(0.0, |stop| 1.0 - stop.offset);
                        format!(
                            r##"draw:style="radial" draw:start-color="#{}" draw:end-color="#{}" draw:cx="{}%" draw:cy="{}%" draw:border="{}%""##,
                            hex(end),
                            hex(start),
                            trim_number(centre.0 * 100.0),
                            trim_number(centre.1 * 100.0),
                            trim_number(border * 100.0)
                        )
                    }
                };
                let name = self
                    .named
                    .name("Gradient_", format!("gradient|{definition}"), |name| {
                        format!(r#"<draw:gradient draw:name="{name}" {definition}/>"#)
                    });
                format!(r#" draw:fill="gradient" draw:fill-gradient-name="{name}""#)
            }
            Paint::Picture { media, tile_emu } => {
                let Some(path) = self.picture_path(*media) else {
                    return r#" draw:fill="none""#.to_owned();
                };
                let name = self.named.name("Bitmap_", format!("bitmap|{path}"), |name| {
                    format!(
                        r#"<draw:fill-image draw:name="{name}" xlink:href="{path}" xlink:type="simple" xlink:show="embed" xlink:actuate="onLoad"/>"#
                    )
                });
                match tile_emu {
                    Some((width, height)) => format!(
                        r#" draw:fill="bitmap" draw:fill-image-name="{name}" style:repeat="repeat" draw:fill-image-width="{}" draw:fill-image-height="{}""#,
                        emu_length(*width),
                        emu_length(*height)
                    ),
                    None => format!(
                        r#" draw:fill="bitmap" draw:fill-image-name="{name}" style:repeat="stretch""#
                    ),
                }
            }
        }
    }

    /// Paragraphs, those with a bullet in lists nested by their level.
    fn paragraphs(&mut self, paragraphs: &[SlideParagraph]) -> String {
        let mut xml = String::new();
        let mut index = 0;
        while index < paragraphs.len() {
            let paragraph = &paragraphs[index];
            let Some(bullet) = &paragraph.bullet else {
                xml.push_str(&self.paragraph(paragraph));
                index += 1;
                continue;
            };
            let kind = bullet_kind(bullet);
            let end = index
                + paragraphs[index..]
                    .iter()
                    .take_while(|other| {
                        other
                            .bullet
                            .as_ref()
                            .is_some_and(|found| bullet_kind(found) == kind)
                    })
                    .count();
            xml.push_str(&self.list(&paragraphs[index..end], &kind));
            index = end;
        }
        xml
    }

    fn list(&mut self, paragraphs: &[SlideParagraph], kind: &str) -> String {
        let style = self
            .styles
            .name("L", format!("list|{kind}"), |name| list_style(name, kind));
        let mut xml = format!(r#"<text:list text:style-name="{style}">"#);
        let mut depth = 1;
        let mut item_open = vec![false];
        for paragraph in paragraphs {
            let target = usize::from(paragraph.level).min(9) + 1;
            while depth > target {
                if item_open.pop() == Some(true) {
                    xml.push_str("</text:list-item>");
                }
                xml.push_str("</text:list>");
                depth -= 1;
            }
            if item_open.last() == Some(&true) && depth == target {
                xml.push_str("</text:list-item>");
                if let Some(last) = item_open.last_mut() {
                    *last = false;
                }
            }
            while depth < target {
                if item_open.last() != Some(&true) {
                    xml.push_str("<text:list-item>");
                    if let Some(last) = item_open.last_mut() {
                        *last = true;
                    }
                }
                xml.push_str("<text:list>");
                item_open.push(false);
                depth += 1;
            }
            xml.push_str("<text:list-item>");
            if let Some(last) = item_open.last_mut() {
                *last = true;
            }
            xml.push_str(&self.paragraph(paragraph));
        }
        while let Some(open) = item_open.pop() {
            if open {
                xml.push_str("</text:list-item>");
            }
            xml.push_str("</text:list>");
        }
        xml
    }

    fn paragraph(&mut self, paragraph: &SlideParagraph) -> String {
        let align = match paragraph.align {
            Alignment::Left => "start",
            Alignment::Center => "center",
            Alignment::Right => "end",
            Alignment::Justify => "justify",
        };
        let properties = format!(
            r#" fo:text-align="{align}" fo:margin-left="{}" fo:text-indent="{}" fo:line-height="{}%" fo:margin-top="{}pt" fo:margin-bottom="{}pt""#,
            emu_length(paragraph.margin_left_emu),
            emu_length(paragraph.indent_emu),
            paragraph.line_spacing_percent.max(1),
            trim_number(f64::from(paragraph.space_before) / 100.0),
            trim_number(f64::from(paragraph.space_after) / 100.0)
        );
        let style = self.styles.name("P", format!("paragraph|{properties}"), |name| {
            format!(
                r#"<style:style style:name="{name}" style:family="paragraph"><style:paragraph-properties{properties}/></style:style>"#
            )
        });
        let mut runs = String::new();
        for run in &paragraph.runs {
            let mut properties = format!(
                r##" fo:font-size="{}pt" fo:color="#{}" fo:font-family="'{}'""##,
                trim_number(f64::from(run.size) / 100.0),
                hex(&run.colour),
                attribute_xml(&run.family.replace('\'', ""))
            );
            if run.bold {
                properties.push_str(r#" fo:font-weight="bold""#);
            }
            if run.italic {
                properties.push_str(r#" fo:font-style="italic""#);
            }
            if run.underline {
                properties.push_str(r#" style:text-underline-style="solid" style:text-underline-width="auto" style:text-underline-color="font-color""#);
            }
            if run.strike {
                properties.push_str(r#" style:text-line-through-style="solid""#);
            }
            let text_style = self.styles.name("T", format!("text|{properties}"), |name| {
                format!(
                    r#"<style:style style:name="{name}" style:family="text"><style:text-properties{properties}/></style:style>"#
                )
            });
            if run.line_break {
                runs.push_str("<text:line-break/>");
            }
            if !run.text.is_empty() {
                let _ = write!(
                    runs,
                    r#"<text:span text:style-name="{text_style}">{}</text:span>"#,
                    text_xml(&run.text)
                );
            }
        }
        format!(r#"<text:p text:style-name="{style}">{runs}</text:p>"#)
    }

    fn table(&mut self, table: &SlideTable) -> String {
        let mut xml = String::from("<table:table>");
        for width in &table.column_widths_emu {
            let width = (*width).max(0);
            let style = self.styles.name("co", format!("column|{width}"), |name| {
                format!(
                    r#"<style:style style:name="{name}" style:family="table-column"><style:table-column-properties style:column-width="{}"/></style:style>"#,
                    emu_length(width)
                )
            });
            let _ = write!(xml, r#"<table:table-column table:style-name="{style}"/>"#);
        }
        for row in &table.rows {
            let height = row.height_emu.max(0);
            let row_style = self.styles.name("ro", format!("row|{height}"), |name| {
                format!(
                    r#"<style:style style:name="{name}" style:family="table-row"><style:table-row-properties style:row-height="{}"/></style:style>"#,
                    emu_length(height)
                )
            });
            let _ = write!(xml, r#"<table:table-row table:style-name="{row_style}">"#);
            for cell in &row.cells {
                if cell.covered {
                    xml.push_str("<table:covered-table-cell/>");
                    continue;
                }
                let fill = cell.fill.as_ref().map_or_else(
                    || r#" draw:fill="none""#.to_owned(),
                    |fill| format!(r##" draw:fill="solid" draw:fill-color="#{}""##, hex(fill)),
                );
                let [left, top, right, bottom] = cell.body.insets_emu;
                let anchor = match cell.body.anchor {
                    TextAnchor::Top => "top",
                    TextAnchor::Middle => "middle",
                    TextAnchor::Bottom => "bottom",
                };
                let properties = format!(
                    r#"{fill} draw:textarea-vertical-align="{anchor}" fo:padding-left="{}" fo:padding-top="{}" fo:padding-right="{}" fo:padding-bottom="{}""#,
                    emu_length(left),
                    emu_length(top),
                    emu_length(right),
                    emu_length(bottom)
                );
                let cell_style = self.styles.name("ce", format!("cell|{properties}"), |name| {
                    format!(
                        r#"<style:style style:name="{name}" style:family="table-cell"><style:graphic-properties{properties}/><style:table-cell-properties fo:border="0.5pt solid #000000"/></style:style>"#
                    )
                });
                let mut spans = String::new();
                if cell.col_span > 1 {
                    let _ = write!(
                        spans,
                        r#" table:number-columns-spanned="{}""#,
                        cell.col_span
                    );
                }
                if cell.row_span > 1 {
                    let _ = write!(spans, r#" table:number-rows-spanned="{}""#, cell.row_span);
                }
                let text = self.paragraphs(&cell.body.paragraphs);
                let _ = write!(
                    xml,
                    r#"<table:table-cell table:style-name="{cell_style}"{spans}>{text}</table:table-cell>"#
                );
            }
            xml.push_str("</table:table-row>");
        }
        xml.push_str("</table:table>");
        xml
    }

    fn picture_path(&mut self, media: usize) -> Option<String> {
        if let Some(Some(path)) = self.pictures.get(media) {
            return Some(path.clone());
        }
        let presentation = self.presentation;
        let item = presentation.media.get(media)?;
        let extension = crate::document::image_extension(item.content_type)?;
        let path = format!("Pictures/image{}.{extension}", self.picture_files.len() + 1);
        self.picture_files
            .push((path.clone(), item.content_type, item.bytes.as_slice()));
        if let Some(slot) = self.pictures.get_mut(media) {
            *slot = Some(path.clone());
        }
        Some(path)
    }
}

/// Where a shape sits: its frame, turned about its centre by `rotation` degrees clockwise.
fn placement(frame: &Frame, rotation: f64) -> String {
    let size = format!(
        r#" svg:width="{}" svg:height="{}""#,
        emu_length(frame.width.max(0)),
        emu_length(frame.height.max(0))
    );
    if rotation.abs() < 1e-6 {
        return format!(
            r#" svg:x="{}" svg:y="{}"{size}"#,
            emu_length(frame.x),
            emu_length(frame.y)
        );
    }
    // `OpenDocument` turns counter-clockwise about the top left corner, then moves the shape.
    #[expect(clippy::cast_precision_loss, reason = "slide coordinates fit in f64")]
    let (x, y, half_width, half_height) = (
        frame.x as f64,
        frame.y as f64,
        frame.width as f64 / 2.0,
        frame.height as f64 / 2.0,
    );
    let angle = -rotation.to_radians();
    let (sin, cos) = angle.sin_cos();
    let (centre_x, centre_y) = (x + half_width, y + half_height);
    let translate_x = centre_x - half_width * cos - half_height * sin;
    let translate_y = centre_y + half_width * sin - half_height * cos;
    format!(
        r#"{size} draw:transform="rotate ({}) translate ({}pt {}pt)""#,
        trim_number(angle),
        trim_number(translate_x / EMU_PER_POINT),
        trim_number(translate_y / EMU_PER_POINT)
    )
}

/// The geometry of a custom shape: a preset `OpenDocument` names, or the shape's own paths.
fn enhanced_geometry(geometry: &ShapeGeometry, frame: &Frame) -> String {
    let kind = match geometry {
        ShapeGeometry::RoundedRectangle => "round-rectangle",
        ShapeGeometry::Ellipse => "ellipse",
        ShapeGeometry::Path(paths) if !paths.is_empty() => {
            let mut path = String::new();
            for figure in paths {
                for command in &figure.commands {
                    match command {
                        PathCommand::MoveTo(point) => {
                            let _ = write!(path, "M {} {} ", point.x, point.y);
                        }
                        PathCommand::LineTo(point) => {
                            let _ = write!(path, "L {} {} ", point.x, point.y);
                        }
                        PathCommand::CubicTo(first, second, end) => {
                            let _ = write!(
                                path,
                                "C {} {} {} {} {} {} ",
                                first.x, first.y, second.x, second.y, end.x, end.y
                            );
                        }
                        PathCommand::Close => path.push_str("Z "),
                    }
                }
                if !figure.filled {
                    path.push_str("F ");
                }
                if !figure.stroked {
                    path.push_str("S ");
                }
                match figure.shade {
                    FillShade::Darken => path.push_str("H "),
                    FillShade::DarkenLess => path.push_str("I "),
                    FillShade::Lighten => path.push_str("J "),
                    FillShade::LightenLess => path.push_str("K "),
                    FillShade::Normal => {}
                }
                path.push_str("N ");
            }
            return format!(
                r#"<draw:enhanced-geometry svg:viewBox="0 0 {} {}" draw:enhanced-path="{}"/>"#,
                frame.width.max(1),
                frame.height.max(1),
                path.trim_end()
            );
        }
        _ => "rectangle",
    };
    format!(r#"<draw:enhanced-geometry svg:viewBox="0 0 21600 21600" draw:type="{kind}"/>"#)
}

/// The kind of list a bullet makes: its character, or the number format of its label.
fn bullet_kind(bullet: &Bullet) -> String {
    match bullet {
        Bullet::Character(character) => format!("bullet|{character}"),
        Bullet::Number(label) => {
            let start = label
                .find(|character: char| character.is_alphanumeric())
                .unwrap_or(label.len());
            let end = label[start..]
                .find(|character: char| !character.is_alphanumeric())
                .map_or(label.len(), |at| start + at);
            let core = &label[start..end];
            let format = if core.chars().all(|character| character.is_ascii_digit()) {
                "1"
            } else if core.chars().all(|character| "ivxlcdm".contains(character))
                && !core.is_empty()
            {
                "i"
            } else if core.chars().all(|character| "IVXLCDM".contains(character))
                && !core.is_empty()
            {
                "I"
            } else if core.chars().all(|character| character.is_ascii_uppercase()) {
                "A"
            } else {
                "a"
            };
            format!("number|{}|{format}|{}", &label[..start], &label[end..])
        }
    }
}

fn list_style(name: &str, kind: &str) -> String {
    let parts: Vec<&str> = kind.split('|').collect();
    let mut xml = format!(r#"<text:list-style style:name="{name}">"#);
    for level in 1..=10 {
        if parts.first() == Some(&"number") {
            let _ = write!(
                xml,
                r#"<text:list-level-style-number text:level="{level}" style:num-prefix="{}" style:num-suffix="{}" style:num-format="{}"/>"#,
                attribute_xml(parts.get(1).copied().unwrap_or("")),
                attribute_xml(parts.get(3).copied().unwrap_or(".")),
                parts.get(2).copied().unwrap_or("1")
            );
        } else {
            let _ = write!(
                xml,
                r#"<text:list-level-style-bullet text:level="{level}" text:bullet-char="{}"/>"#,
                attribute_xml(parts.get(1).copied().unwrap_or("\u{2022}"))
            );
        }
    }
    xml.push_str("</text:list-style>");
    xml
}

/// The attributes of a drawing-page style a transition sets, as Impress writes them.
fn transition_attributes(transition: &Transition) -> String {
    let direction = transition.direction.as_deref().unwrap_or("");
    let side = |direction: &str| match direction {
        "u" => "fromBottom",
        "d" => "fromTop",
        "l" => "fromRight",
        "rd" => "fromTopLeft",
        "ld" => "fromTopRight",
        "ru" => "fromBottomLeft",
        "lu" => "fromBottomRight",
        _ => "fromLeft",
    };
    let orientation = if direction == "horz" {
        "horizontal"
    } else {
        "vertical"
    };
    let (kind, subtype, reverse) = match transition.effect.as_str() {
        "push" => ("pushWipe", side(direction), false),
        "pull" => ("slideWipe", side(direction), true),
        "cover" => ("slideWipe", side(direction), false),
        "wipe" => match direction {
            "d" => ("barWipe", "topToBottom", false),
            "u" => ("barWipe", "topToBottom", true),
            "l" => ("barWipe", "leftToRight", true),
            _ => ("barWipe", "leftToRight", false),
        },
        "split" => ("barnDoorWipe", orientation, false),
        "blinds" => ("blindsWipe", orientation, false),
        "checker" => ("checkerBoardWipe", "across", false),
        "randomBar" => ("randomBarWipe", orientation, false),
        "diamond" => ("irisWipe", "diamond", false),
        "zoom" if direction == "out" => ("irisWipe", "rectangle", true),
        "zoom" if direction == "in" => ("irisWipe", "rectangle", false),
        "zoom" => ("zoom", "rotateIn", false),
        "circle" => ("ellipseWipe", "circle", false),
        "wedge" => ("fanWipe", "centerTop", false),
        "wheel" => ("pinWheelWipe", "oneBlade", false),
        "strips" => ("waterfallWipe", "horizontalLeft", false),
        "dissolve" => ("dissolve", "", false),
        "random" => ("random", "", false),
        "plus" => ("fourBoxWipe", "cornersIn", false),
        _ => ("fade", "crossfade", false),
    };
    let mut attributes = format!(r#" smil:type="{kind}""#);
    if !subtype.is_empty() {
        let _ = write!(attributes, r#" smil:subtype="{subtype}""#);
    }
    if reverse {
        attributes.push_str(r#" smil:direction="reverse""#);
    }
    if let Some(duration) = transition.duration_ms {
        let _ = write!(
            attributes,
            r#" smil:dur="{}s""#,
            trim_number(f64::from(duration) / 1_000.0)
        );
    }
    let advance = match (transition.advance_on_click, transition.advance_after_ms) {
        (true, Some(_)) => "semi-automatic",
        (false, Some(_)) => "automatic",
        _ => "manual",
    };
    let _ = write!(attributes, r#" presentation:transition-type="{advance}""#);
    if let Some(after) = transition.advance_after_ms {
        let _ = write!(
            attributes,
            r#" presentation:duration="PT{}S""#,
            trim_number(f64::from(after) / 1_000.0)
        );
    }
    attributes
}

/// The animation tree of a page: each click starts a group, an effect with the previous one
/// shares its step, an effect after the previous one starts the next step of the group.
fn animations_xml(animations: &[Animation], number: usize) -> String {
    if animations.is_empty() {
        return String::new();
    }
    let mut clicks: Vec<Vec<Vec<&Animation>>> = Vec::new();
    for animation in animations {
        match animation.trigger {
            AnimationTrigger::OnClick => clicks.push(vec![vec![animation]]),
            AnimationTrigger::WithPrevious => {
                match clicks.last_mut().and_then(|click| click.last_mut()) {
                    Some(step) => step.push(animation),
                    None => clicks.push(vec![vec![animation]]),
                }
            }
            AnimationTrigger::AfterPrevious => match clicks.last_mut() {
                Some(click) => click.push(vec![animation]),
                None => clicks.push(vec![vec![animation]]),
            },
        }
    }
    let mut xml = String::from(
        r#"<anim:par presentation:node-type="timing-root"><anim:seq presentation:node-type="main-sequence">"#,
    );
    for click in &clicks {
        xml.push_str(r#"<anim:par smil:begin="indefinite">"#);
        let mut begin = 0u32;
        for step in click {
            let _ = write!(
                xml,
                r#"<anim:par smil:begin="{}s">"#,
                trim_number(f64::from(begin) / 1_000.0)
            );
            let mut longest = 0;
            for animation in step {
                xml.push_str(&effect_xml(animation, number));
                longest = longest.max(animation.delay_ms + animation.duration_ms);
            }
            xml.push_str("</anim:par>");
            begin += longest;
        }
        xml.push_str("</anim:par>");
    }
    xml.push_str("</anim:seq></anim:par>");
    xml
}

fn effect_xml(animation: &Animation, number: usize) -> String {
    let target = format!("id{number}_{}", animation.shape);
    let node_type = match animation.trigger {
        AnimationTrigger::OnClick => "on-click",
        AnimationTrigger::WithPrevious => "with-previous",
        AnimationTrigger::AfterPrevious => "after-previous",
    };
    let (class, preset) = match animation.class {
        AnimationClass::Entrance => (
            "entrance",
            match animation.preset {
                1 => "ooo-entrance-appear",
                2 => "ooo-entrance-fly-in",
                22 => "ooo-entrance-wipe",
                53 => "ooo-entrance-zoom",
                _ => "ooo-entrance-fade-in",
            },
        ),
        AnimationClass::Exit => (
            "exit",
            match animation.preset {
                1 => "ooo-exit-disappear",
                2 => "ooo-exit-fly-out",
                22 => "ooo-exit-wipe",
                53 => "ooo-exit-zoom",
                _ => "ooo-exit-fade-out",
            },
        ),
        AnimationClass::Emphasis => (
            "emphasis",
            match animation.preset {
                6 => "ooo-emphasis-grow-and-shrink",
                _ => "ooo-emphasis-spin",
            },
        ),
        AnimationClass::Path => ("motion-path", "ooo-motionpath-user-defined"),
    };
    let subtype = match animation.preset_subtype {
        1 => r#" presentation:preset-sub-type="from-top""#,
        2 => r#" presentation:preset-sub-type="from-right""#,
        4 => r#" presentation:preset-sub-type="from-bottom""#,
        8 => r#" presentation:preset-sub-type="from-left""#,
        _ => "",
    };
    let duration = format!(
        "{}s",
        trim_number(f64::from(animation.duration_ms.max(1)) / 1_000.0)
    );
    let behaviour = match animation.class {
        AnimationClass::Entrance => format!(
            r#"<anim:set smil:begin="0s" smil:dur="0.001s" smil:fill="hold" smil:targetElement="{target}" smil:attributeName="visibility" smil:to="visible"/><anim:transitionFilter smil:dur="{duration}" smil:targetElement="{target}" smil:type="fade" smil:subtype="crossfade"/>"#
        ),
        AnimationClass::Exit => format!(
            r#"<anim:transitionFilter smil:dur="{duration}" smil:targetElement="{target}" smil:type="fade" smil:subtype="crossfade" smil:mode="out"/><anim:set smil:begin="{duration}" smil:dur="0.001s" smil:fill="hold" smil:targetElement="{target}" smil:attributeName="visibility" smil:to="hidden"/>"#
        ),
        AnimationClass::Emphasis => format!(
            r#"<anim:animateTransform smil:dur="{duration}" smil:fill="hold" smil:targetElement="{target}" smil:by="360" svg:type="rotate"/>"#
        ),
        AnimationClass::Path => format!(
            r#"<anim:animateMotion smil:dur="{duration}" smil:fill="hold" smil:targetElement="{target}" svg:path="{}" presentation:additive="sum"/>"#,
            attribute_xml(animation.motion_path.as_deref().unwrap_or("M 0 0 L 0 0"))
        ),
    };
    format!(
        r#"<anim:par smil:begin="{}s" smil:fill="hold" presentation:node-type="{node_type}" presentation:preset-class="{class}" presentation:preset-id="{preset}"{subtype}>{behaviour}</anim:par>"#,
        trim_number(f64::from(animation.delay_ms) / 1_000.0)
    )
}

/// English Metric Units as points.
#[expect(clippy::cast_precision_loss, reason = "slide lengths fit in f64")]
fn emu_length(emu: i64) -> String {
    format!("{}pt", trim_number(emu as f64 / EMU_PER_POINT))
}

fn trim_number(value: f64) -> String {
    let text = format!("{value:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

/// A colour of the model as six hexadecimal digits, black for anything else.
fn hex(colour: &str) -> String {
    let colour = colour.trim_start_matches('#');
    if colour.len() == 6
        && colour
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        colour.to_ascii_uppercase()
    } else {
        "000000".to_owned()
    }
}

fn text_xml(text: &str) -> String {
    let mut xml = String::with_capacity(text.len());
    let mut spaces = 0usize;
    for character in text.chars() {
        if character == ' ' {
            spaces += 1;
            continue;
        }
        push_spaces(&mut xml, &mut spaces);
        match character {
            '\t' => xml.push_str("<text:tab/>"),
            '\n' => xml.push_str("<text:line-break/>"),
            '&' => xml.push_str("&amp;"),
            '<' => xml.push_str("&lt;"),
            '>' => xml.push_str("&gt;"),
            '\u{FFFE}' | '\u{FFFF}' => {}
            character if (character as u32) < 0x20 => {}
            character => xml.push(character),
        }
    }
    push_spaces(&mut xml, &mut spaces);
    xml
}

fn push_spaces(xml: &mut String, spaces: &mut usize) {
    match *spaces {
        0 => {}
        1 => xml.push(' '),
        count => {
            xml.push(' ');
            let _ = write!(xml, r#"<text:s text:c="{}"/>"#, count - 1);
        }
    }
    *spaces = 0;
}

fn attribute_xml(text: &str) -> String {
    text.chars()
        .filter(|character| (*character as u32) >= 0x20)
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests;
