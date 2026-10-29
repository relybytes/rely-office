// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The charts of a sheet, made and changed in the drawing part the sheet carries: a new chart
//! is an anchor and a chart part added to it, a changed one has its part written again, and
//! everything else the drawing holds, pictures and the charts no editor changes, stays as the
//! file had it. A sheet read from an ods gets a drawing of its own built from its pictures and
//! charts, so it is edited, drawn and saved the same way.

use std::fmt::Write as _;

use quick_xml::Reader;
use quick_xml::events::Event;
use rely_office_core::{
    CarriedDrawing, CarriedPart, ChartSpec, SheetDrawing, SheetLayout, chart_formula_range,
    chart_part_xml, drawing_anchors, read_chart_spec,
};

use crate::address::{MAX_COLUMNS, MAX_ROWS};
use crate::model::{EditError, Workbook};

const DRAWING_TYPE: &str = "application/vnd.openxmlformats-officedocument.drawing+xml";
const CHART_TYPE: &str = "application/vnd.openxmlformats-officedocument.drawingml.chart+xml";
const RELATIONSHIPS_TYPE: &str = "application/vnd.openxmlformats-package.relationships+xml";
const CHART_RELATIONSHIP: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart";
const IMAGE_RELATIONSHIP: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const NAMESPACES: &str = concat!(
    r#"xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" "#,
    r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
    r#"xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" "#,
    r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships""#
);
/// The most charts one sheet holds.
pub const MAX_CHARTS: usize = 64;
/// The largest side of a chart, about two metres.
const MAX_SIDE_EMU: i64 = 914_400 * 80;
const MAX_SERIES: usize = 64;
const MAX_TITLE_CHARS: usize = 255;

/// Where a chart sits: the cell its corner is in, how far into it, and its size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChartPlacement {
    pub row: u32,
    pub column: u32,
    pub offset_emu: (i64, i64),
    pub width_emu: i64,
    pub height_emu: i64,
}

/// A chart of a sheet: what it shows, when an editor can change it, and where it sits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetChart {
    pub spec: Option<ChartSpec>,
    pub placement: ChartPlacement,
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The relationships part of a part, `xl/drawings/_rels/drawing1.xml.rels`.
fn relationships_name(part: &str) -> String {
    match part.rsplit_once('/') {
        Some((folder, file)) => format!("{folder}/_rels/{file}.rels"),
        None => format!("_rels/{part}.rels"),
    }
}

/// A relationship target as a member name, resolved against the folder of its source part.
fn resolve(source: &str, target: &str) -> String {
    if let Some(absolute) = target.strip_prefix('/') {
        return absolute.to_owned();
    }
    let mut segments: Vec<&str> = source.split('/').collect();
    segments.pop();
    for segment in target.split('/') {
        match segment {
            ".." => {
                segments.pop();
            }
            "." | "" => {}
            other => segments.push(other),
        }
    }
    segments.join("/")
}

/// A member name as a target relative to the folder of `source`.
fn relative(source: &str, target: &str) -> String {
    let source: Vec<&str> = source.split('/').collect();
    let target: Vec<&str> = target.split('/').collect();
    let folder = &source[..source.len().saturating_sub(1)];
    let common = folder
        .iter()
        .zip(&target)
        .take_while(|(first, second)| first == second)
        .count();
    let mut parts: Vec<&str> = vec![".."; folder.len() - common];
    parts.extend(&target[common..]);
    parts.join("/")
}

/// The internal relationships of a relationships part: id and target.
fn relationships(xml: &str) -> Vec<(String, String)> {
    let mut reader = Reader::from_str(xml);
    let mut found = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(element) | Event::Empty(element))
                if element.local_name().as_ref() == "Relationship" =>
            {
                let attribute = |name: &str| {
                    element
                        .attributes()
                        .flatten()
                        .find(|attribute| attribute.key.as_ref() == name)
                        .and_then(|attribute| {
                            attribute
                                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .ok()
                                .map(std::borrow::Cow::into_owned)
                        })
                };
                if attribute("TargetMode").as_deref() != Some("External")
                    && let (Some(id), Some(target)) = (attribute("Id"), attribute("Target"))
                {
                    found.push((id, target));
                }
            }
            Ok(Event::Eof) | Err(_) => return found,
            _ => {}
        }
    }
}

fn part_text<'d>(drawing: &'d CarriedDrawing, name: &str) -> Option<&'d str> {
    drawing
        .parts
        .iter()
        .find(|part| part.name == name)
        .and_then(|part| std::str::from_utf8(&part.bytes).ok())
}

fn set_part(drawing: &mut CarriedDrawing, name: &str, content_type: &str, bytes: Vec<u8>) {
    match drawing.parts.iter_mut().find(|part| part.name == name) {
        Some(part) => part.bytes = bytes,
        None => drawing.parts.push(CarriedPart {
            name: name.to_owned(),
            content_type: content_type.to_owned(),
            bytes,
        }),
    }
}

/// The span of the first child element of an anchor whose local name is `name`, in the
/// anchor's own text.
fn child_span(anchor: &str, name: &str) -> Option<(usize, usize)> {
    let mut reader = Reader::from_str(anchor);
    let mut depth = 0_usize;
    let mut start = None;
    loop {
        let before = usize::try_from(reader.buffer_position()).ok()?;
        let event = reader.read_event().ok()?;
        let after = usize::try_from(reader.buffer_position()).ok()?;
        match event {
            Event::Start(element) => {
                if depth == 1 && start.is_none() && element.local_name().as_ref() == name {
                    start = Some((before, depth));
                }
                depth += 1;
            }
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                if let Some((begin, level)) = start
                    && depth == level
                {
                    return Some((begin, after));
                }
            }
            Event::Empty(element)
                if depth == 1 && start.is_none() && element.local_name().as_ref() == name =>
            {
                return Some((before, after));
            }
            Event::Eof => return None,
            _ => {}
        }
    }
}

/// An anchor placing `frame`, a picture or a graphic frame, at a cell with its size.
fn anchor_xml(placement: ChartPlacement, frame: &str) -> String {
    format!(
        concat!(
            "<xdr:oneCellAnchor {}><xdr:from><xdr:col>{}</xdr:col><xdr:colOff>{}</xdr:colOff>",
            "<xdr:row>{}</xdr:row><xdr:rowOff>{}</xdr:rowOff></xdr:from>",
            r#"<xdr:ext cx="{}" cy="{}"/>{}<xdr:clientData/></xdr:oneCellAnchor>"#
        ),
        NAMESPACES,
        placement.column,
        placement.offset_emu.0,
        placement.row,
        placement.offset_emu.1,
        placement.width_emu,
        placement.height_emu,
        frame
    )
}

fn chart_frame(id: u32, relationship: &str) -> String {
    format!(
        concat!(
            r#"<xdr:graphicFrame macro=""><xdr:nvGraphicFramePr><xdr:cNvPr id="{0}" name="Chart {0}"/>"#,
            "<xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr>",
            r#"<xdr:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></xdr:xfrm><a:graphic>"#,
            r#"<a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart">"#,
            r#"<c:chart r:id="{1}"/></a:graphicData></a:graphic></xdr:graphicFrame>"#
        ),
        id,
        escape(relationship)
    )
}

fn picture_frame(
    id: u32,
    relationship: &str,
    description: Option<&str>,
    placement: ChartPlacement,
) -> String {
    let description =
        description.map_or_else(String::new, |text| format!(r#" descr="{}""#, escape(text)));
    format!(
        concat!(
            r#"<xdr:pic><xdr:nvPicPr><xdr:cNvPr id="{0}" name="Picture {0}"{1}/><xdr:cNvPicPr>"#,
            r#"<a:picLocks noChangeAspect="1"/></xdr:cNvPicPr></xdr:nvPicPr><xdr:blipFill>"#,
            r#"<a:blip r:embed="{2}"/><a:stretch><a:fillRect/></a:stretch></xdr:blipFill>"#,
            r#"<xdr:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{3}" cy="{4}"/></a:xfrm>"#,
            r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></xdr:spPr></xdr:pic>"#
        ),
        id,
        description,
        escape(relationship),
        placement.width_emu,
        placement.height_emu
    )
}

/// The largest `cNvPr` id a drawing uses.
fn largest_shape_id(xml: &str) -> u32 {
    xml.match_indices("cNvPr ")
        .filter_map(|(at, _)| {
            let rest = &xml[at..];
            let value = rest.split_once(" id=\"")?.1;
            value.split('"').next()?.parse::<u32>().ok()
        })
        .max()
        .unwrap_or(1)
}

/// A relationship id the relationships of a part do not use yet.
fn free_relationship(existing: &[(String, String)], stem: &str) -> String {
    // One more number than ids there are always leaves one free.
    (1..=existing.len() + 1)
        .map(|number| format!("{stem}{number}"))
        .find(|id| existing.iter().all(|(known, _)| known != id))
        .unwrap_or_default()
}

fn with_relationship(xml: Option<&str>, id: &str, kind: &str, target: &str) -> String {
    let entry = format!(
        r#"<Relationship Id="{}" Type="{kind}" Target="{}"/>"#,
        escape(id),
        escape(target)
    );
    match xml.and_then(|xml| xml.rfind("</Relationships>").map(|at| (xml, at))) {
        Some((xml, at)) => format!("{}{entry}{}", &xml[..at], &xml[at..]),
        None => format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{entry}</Relationships>"#
        ),
    }
}

fn without_relationship(xml: &str, id: &str) -> String {
    let needle = format!(r#"Id="{id}""#);
    let Some(at) = xml.find(&needle) else {
        return xml.to_owned();
    };
    let start = xml[..at].rfind('<').unwrap_or(at);
    let end = xml[at..].find("/>").map_or(xml.len(), |end| at + end + 2);
    format!("{}{}", &xml[..start], &xml[end..])
}

fn image_extension(content_type: &str) -> &'static str {
    match content_type {
        "image/jpeg" => "jpeg",
        "image/gif" => "gif",
        "image/svg+xml" => "svg",
        "image/bmp" => "bmp",
        _ => "png",
    }
}

impl Workbook {
    /// The sizes of a sheet's rows and columns, as its drawings anchor to them.
    pub(crate) fn drawing_layout(&self, sheet: usize) -> SheetLayout {
        let Some(content) = self.sheets.get(sheet) else {
            return SheetLayout::default();
        };
        let last_column = content
            .column_widths
            .keys()
            .next_back()
            .map_or(0, |last| last + 1);
        SheetLayout {
            column_widths: (0..last_column)
                .map(|column| {
                    content.column_widths.get(&column).map(|width| {
                        #[expect(
                            clippy::cast_possible_truncation,
                            clippy::cast_sign_loss,
                            reason = "widths are clamped to 255"
                        )]
                        let units = (width * 256.0).round() as u32;
                        units
                    })
                })
                .collect(),
            row_heights: content
                .row_heights
                .iter()
                .map(|(row, height)| {
                    #[expect(
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss,
                        reason = "heights are clamped to 409 points"
                    )]
                    let twips = (height * 20.0).round() as u32;
                    (*row, twips)
                })
                .collect(),
            ..SheetLayout::default()
        }
    }

    /// The anchors of a sheet's drawing that hold a chart, with the part of each.
    fn chart_anchors(&self, sheet: usize) -> Vec<(rely_office_core::DrawingAnchor, String)> {
        let Some(drawing) = self
            .sheets
            .get(sheet)
            .and_then(|content| content.drawing.as_ref())
        else {
            return Vec::new();
        };
        let Some(xml) = part_text(drawing, &drawing.drawing_part) else {
            return Vec::new();
        };
        let links = part_text(drawing, &relationships_name(&drawing.drawing_part))
            .map(relationships)
            .unwrap_or_default();
        drawing_anchors(xml, &self.drawing_layout(sheet))
            .into_iter()
            .filter_map(|anchor| {
                let id = anchor.chart_relationship.as_ref()?;
                let target = &links.iter().find(|(known, _)| known == id)?.1;
                let part = resolve(&drawing.drawing_part, target);
                Some((anchor, part))
            })
            .collect()
    }

    /// The charts of a sheet, in the order its drawing lists them.
    #[must_use]
    pub fn charts(&self, sheet: usize) -> Vec<SheetChart> {
        let Some(drawing) = self
            .sheets
            .get(sheet)
            .and_then(|content| content.drawing.as_ref())
        else {
            return Vec::new();
        };
        self.chart_anchors(sheet)
            .into_iter()
            .map(|(anchor, part)| SheetChart {
                spec: part_text(drawing, &part).and_then(read_chart_spec),
                placement: ChartPlacement {
                    row: anchor.row,
                    column: anchor.column,
                    offset_emu: anchor.offset_emu,
                    width_emu: anchor.width_emu,
                    height_emu: anchor.height_emu,
                },
            })
            .collect()
    }

    fn check_chart(&self, spec: &ChartSpec) -> Result<(), EditError> {
        let reads_a_range = |formula: &str| {
            chart_formula_range(formula).is_some_and(|(sheet, range)| {
                self.sheet_index(&sheet).is_some()
                    && range.last_row < MAX_ROWS
                    && range.last_column < MAX_COLUMNS
            })
        };
        let valid = !spec.series.is_empty()
            && spec.series.len() <= MAX_SERIES
            && spec
                .title
                .as_ref()
                .is_none_or(|title| title.chars().count() <= MAX_TITLE_CHARS)
            && spec.series.iter().all(|series| {
                reads_a_range(&series.values) && series.name.as_deref().is_none_or(reads_a_range)
            })
            && spec.categories.as_deref().is_none_or(reads_a_range);
        if valid {
            Ok(())
        } else {
            Err(EditError::InvalidChart)
        }
    }

    fn check_placement(placement: ChartPlacement) -> Result<(), EditError> {
        let side = 1..=MAX_SIDE_EMU;
        let offset = 0..=MAX_SIDE_EMU;
        if placement.row < MAX_ROWS
            && placement.column < MAX_COLUMNS
            && side.contains(&placement.width_emu)
            && side.contains(&placement.height_emu)
            && offset.contains(&placement.offset_emu.0)
            && offset.contains(&placement.offset_emu.1)
        {
            Ok(())
        } else {
            Err(EditError::OutOfBounds)
        }
    }

    /// The member names every sheet's drawing already uses.
    fn carried_names(&self) -> Vec<String> {
        self.sheets
            .iter()
            .filter_map(|sheet| sheet.drawing.as_ref())
            .flat_map(|drawing| drawing.parts.iter().map(|part| part.name.clone()))
            .chain(
                self.sheets
                    .iter()
                    .flat_map(|sheet| sheet.pivots.iter())
                    .flat_map(|pivot| pivot.parts.iter().map(|part| part.name.clone())),
            )
            .collect()
    }

    /// Add a chart to a sheet; the index it gets among the sheet's charts.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::InvalidChart`] for a chart whose ranges are not
    /// ranges of the workbook, [`EditError::OutOfBounds`] for a placement off the grid or past
    /// [`MAX_CHARTS`].
    pub fn add_chart(
        &mut self,
        sheet: usize,
        spec: &ChartSpec,
        placement: ChartPlacement,
    ) -> Result<usize, EditError> {
        if sheet >= self.sheets.len() {
            return Err(EditError::NoSuchSheet(sheet));
        }
        self.check_chart(spec)?;
        Self::check_placement(placement)?;
        let count = self.charts(sheet).len();
        if count >= MAX_CHARTS {
            return Err(EditError::OutOfBounds);
        }
        let names = self.carried_names();
        let free = |stem: &str, extension: &str| {
            (1..=names.len() + 1)
                .map(|number| format!("{stem}{number}.{extension}"))
                .find(|name| !names.contains(name))
                .unwrap_or_default()
        };
        let chart_part = free("xl/charts/chart", "xml");
        let drawing_part = free("xl/drawings/drawing", "xml");
        let drawing = self.sheets[sheet].drawing.get_or_insert_with(|| CarriedDrawing {
            drawing_part: drawing_part.clone(),
            parts: vec![CarriedPart {
                name: drawing_part,
                content_type: DRAWING_TYPE.to_owned(),
                bytes: format!(
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><xdr:wsDr {NAMESPACES}></xdr:wsDr>"#
                )
                .into_bytes(),
            }],
        });
        let drawing_name = drawing.drawing_part.clone();
        let relationships_part = relationships_name(&drawing_name);
        let links_xml = part_text(drawing, &relationships_part).map(str::to_owned);
        let links = links_xml.as_deref().map(relationships).unwrap_or_default();
        let relationship = free_relationship(&links, "rIdRelyChart");
        let xml = part_text(drawing, &drawing_name)
            .unwrap_or_default()
            .to_owned();
        let anchor = anchor_xml(
            placement,
            &chart_frame(largest_shape_id(&xml) + 1, &relationship),
        );
        let Some(end) = xml.rfind("</") else {
            return Err(EditError::InvalidChart);
        };
        let xml = format!("{}{anchor}{}", &xml[..end], &xml[end..]);
        set_part(drawing, &drawing_name, DRAWING_TYPE, xml.into_bytes());
        let target = relative(&drawing_name, &chart_part);
        set_part(
            drawing,
            &relationships_part,
            RELATIONSHIPS_TYPE,
            with_relationship(
                links_xml.as_deref(),
                &relationship,
                CHART_RELATIONSHIP,
                &target,
            )
            .into_bytes(),
        );
        set_part(
            drawing,
            &chart_part,
            CHART_TYPE,
            chart_part_xml(spec).into_bytes(),
        );
        self.touch();
        Ok(count)
    }

    /// Change a chart: what it shows, written as a new part, and where it sits, each when given.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::OutOfBounds`] for a chart the sheet does not
    /// have or a placement off the grid, [`EditError::InvalidChart`].
    pub fn set_chart(
        &mut self,
        sheet: usize,
        index: usize,
        spec: Option<&ChartSpec>,
        placement: Option<ChartPlacement>,
    ) -> Result<(), EditError> {
        if sheet >= self.sheets.len() {
            return Err(EditError::NoSuchSheet(sheet));
        }
        if let Some(spec) = spec {
            self.check_chart(spec)?;
        }
        if let Some(placement) = placement {
            Self::check_placement(placement)?;
        }
        let anchors = self.chart_anchors(sheet);
        let Some((anchor, part)) = anchors.get(index).cloned() else {
            return Err(EditError::OutOfBounds);
        };
        let Some(drawing) = self.sheets[sheet].drawing.as_mut() else {
            return Err(EditError::OutOfBounds);
        };
        if let Some(spec) = spec {
            set_part(
                drawing,
                &part,
                CHART_TYPE,
                chart_part_xml(spec).into_bytes(),
            );
        }
        if let Some(placement) = placement {
            let name = drawing.drawing_part.clone();
            let xml = part_text(drawing, &name).unwrap_or_default().to_owned();
            let (start, end) = anchor.span;
            let text = &xml[start..end];
            let frame = child_span(text, "graphicFrame")
                .map(|(from, to)| text[from..to].to_owned())
                .ok_or(EditError::InvalidChart)?;
            let xml = format!(
                "{}{}{}",
                &xml[..start],
                anchor_xml(placement, &frame),
                &xml[end..]
            );
            set_part(drawing, &name, DRAWING_TYPE, xml.into_bytes());
        }
        self.touch();
        Ok(())
    }

    /// Remove a chart from a sheet, with its part and the parts only it used.
    ///
    /// # Errors
    ///
    /// [`EditError::NoSuchSheet`], [`EditError::OutOfBounds`] for a chart the sheet does not
    /// have.
    pub fn remove_chart(&mut self, sheet: usize, index: usize) -> Result<(), EditError> {
        if sheet >= self.sheets.len() {
            return Err(EditError::NoSuchSheet(sheet));
        }
        let anchors = self.chart_anchors(sheet);
        let Some((anchor, part)) = anchors.get(index).cloned() else {
            return Err(EditError::OutOfBounds);
        };
        let Some(drawing) = self.sheets[sheet].drawing.as_mut() else {
            return Err(EditError::OutOfBounds);
        };
        let name = drawing.drawing_part.clone();
        let xml = part_text(drawing, &name).unwrap_or_default().to_owned();
        let (start, end) = anchor.span;
        set_part(
            drawing,
            &name,
            DRAWING_TYPE,
            format!("{}{}", &xml[..start], &xml[end..]).into_bytes(),
        );
        let relationships_part = relationships_name(&name);
        if let (Some(links), Some(id)) = (
            part_text(drawing, &relationships_part),
            &anchor.chart_relationship,
        ) {
            let links = without_relationship(links, id);
            set_part(
                drawing,
                &relationships_part,
                RELATIONSHIPS_TYPE,
                links.into_bytes(),
            );
        }
        // The chart's own parts go with it, unless another relationship still names them.
        let chart_links = relationships_name(&part);
        let owned: Vec<String> = part_text(drawing, &chart_links)
            .map(relationships)
            .unwrap_or_default()
            .into_iter()
            .map(|(_, target)| resolve(&part, &target))
            .collect();
        drawing
            .parts
            .retain(|carried| carried.name != part && carried.name != chart_links);
        let still_named: Vec<String> = drawing
            .parts
            .iter()
            .filter(|carried| {
                std::path::Path::new(&carried.name)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("rels"))
            })
            .flat_map(|carried| {
                let source = carried
                    .name
                    .replace("_rels/", "")
                    .trim_end_matches(".rels")
                    .to_owned();
                std::str::from_utf8(&carried.bytes)
                    .map(relationships)
                    .unwrap_or_default()
                    .into_iter()
                    .map(move |(_, target)| resolve(&source, &target))
            })
            .collect();
        drawing.parts.retain(|carried| {
            !owned.contains(&carried.name) || still_named.contains(&carried.name)
        });
        self.touch();
        Ok(())
    }
}

impl Workbook {
    /// The chart a selection makes: a series for each column, or for each row of a selection
    /// wider than tall; a first row of text names the series and a first column of text gives
    /// the categories, the x values of a scatter chart when it holds numbers. `None` for a
    /// selection with no numbers to plot.
    #[must_use]
    pub fn chart_for_range(
        &self,
        sheet: usize,
        range: rely_office_core::CellRange,
        chart_type: rely_office_core::ChartType,
    ) -> Option<ChartSpec> {
        use crate::address::CellRef;
        use crate::value::Value;
        use rely_office_core::{ChartSeriesSpec, ChartType, chart_range_formula};
        let name = &self.sheets.get(sheet)?.name;
        let value = |row: u32, column: u32| self.value(sheet, CellRef::new(row, column));
        let text = |row: u32, column: u32| matches!(value(row, column), Value::Text(_));
        let rows = range.last_row - range.first_row + 1;
        let columns = range.last_column - range.first_column + 1;
        let by_column = rows >= columns;
        // Read along the series: `along` is the direction of one series, `across` the other.
        let (first_along, last_along, first_across, last_across) = if by_column {
            (
                range.first_row,
                range.last_row,
                range.first_column,
                range.last_column,
            )
        } else {
            (
                range.first_column,
                range.last_column,
                range.first_row,
                range.last_row,
            )
        };
        let at = |along: u32, across: u32| {
            if by_column {
                (along, across)
            } else {
                (across, along)
            }
        };
        let headed = (first_across..=last_across).any(|across| {
            let (row, column) = at(first_along, across);
            text(row, column)
        }) && last_along > first_along;
        let start = if headed { first_along + 1 } else { first_along };
        let labelled = last_across > first_across
            && (chart_type == ChartType::Scatter
                || (start..=last_along).any(|along| {
                    let (row, column) = at(along, first_across);
                    text(row, column)
                }));
        let first_series = if labelled {
            first_across + 1
        } else {
            first_across
        };
        let line = |across: u32, from: u32| {
            let (first_row, first_column) = at(from, across);
            let (last_row, last_column) = at(last_along, across);
            chart_range_formula(
                name,
                rely_office_core::CellRange {
                    first_row,
                    first_column,
                    last_row,
                    last_column,
                },
            )
        };
        let series: Vec<ChartSeriesSpec> = (first_series..=last_across)
            .filter(|across| {
                (start..=last_along).any(|along| {
                    let (row, column) = at(along, *across);
                    matches!(value(row, column), Value::Number(_))
                })
            })
            .take(MAX_SERIES)
            .map(|across| ChartSeriesSpec {
                name: headed.then(|| {
                    let (row, column) = at(first_along, across);
                    chart_range_formula(
                        name,
                        rely_office_core::CellRange {
                            first_row: row,
                            first_column: column,
                            last_row: row,
                            last_column: column,
                        },
                    )
                }),
                values: line(across, start),
            })
            .collect();
        if series.is_empty() {
            return None;
        }
        let round = matches!(chart_type, ChartType::Pie | ChartType::Doughnut);
        Some(ChartSpec {
            chart_type,
            stacked: false,
            title: None,
            legend: !round || series.len() == 1,
            categories: labelled.then(|| line(first_across, start)),
            series: if round {
                series.into_iter().take(1).collect()
            } else {
                series
            },
        })
    }
}

/// The drawing a sheet read from an ods gets from its pictures and charts: each chart an
/// editor can change as a chart part, anything else as the picture it was drawn as.
/// `counter` numbers the parts across the workbook's sheets.
pub(crate) fn drawing_of_pictures(
    pictures: &[SheetDrawing],
    counter: &mut u32,
) -> Option<CarriedDrawing> {
    if pictures.is_empty() {
        return None;
    }
    *counter += 1;
    let drawing_part = format!("xl/drawings/drawing{counter}.xml");
    let mut anchors = String::new();
    let mut links = String::new();
    let mut parts = Vec::new();
    for (index, picture) in pictures.iter().enumerate() {
        let id = u32::try_from(index).unwrap_or(u32::MAX).saturating_add(2);
        let relationship = format!("rIdRely{}", index + 1);
        let placement = ChartPlacement {
            row: picture.row,
            column: picture.column,
            offset_emu: picture.offset_emu,
            width_emu: picture.width_emu,
            height_emu: picture.height_emu,
        };
        *counter += 1;
        let (frame, kind, member) = if let Some(spec) = &picture.chart {
            let member = format!("xl/charts/chart{counter}.xml");
            parts.push(CarriedPart {
                name: member.clone(),
                content_type: CHART_TYPE.to_owned(),
                bytes: chart_part_xml(spec).into_bytes(),
            });
            (chart_frame(id, &relationship), CHART_RELATIONSHIP, member)
        } else {
            let member = format!(
                "xl/media/image{counter}.{}",
                image_extension(picture.content_type)
            );
            parts.push(CarriedPart {
                name: member.clone(),
                content_type: picture.content_type.to_owned(),
                bytes: picture.bytes.clone(),
            });
            (
                picture_frame(id, &relationship, picture.description.as_deref(), placement),
                IMAGE_RELATIONSHIP,
                member,
            )
        };
        anchors.push_str(&anchor_xml(placement, &frame));
        let _ = write!(
            links,
            r#"<Relationship Id="{relationship}" Type="{kind}" Target="{}"/>"#,
            escape(&relative(&drawing_part, &member))
        );
    }
    parts.push(CarriedPart {
        name: drawing_part.clone(),
        content_type: DRAWING_TYPE.to_owned(),
        bytes: format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><xdr:wsDr {NAMESPACES}>{anchors}</xdr:wsDr>"#
        )
        .into_bytes(),
    });
    parts.push(CarriedPart {
        name: relationships_name(&drawing_part),
        content_type: RELATIONSHIPS_TYPE.to_owned(),
        bytes: format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{links}</Relationships>"#
        )
        .into_bytes(),
    });
    Some(CarriedDrawing {
        drawing_part,
        parts,
    })
}

#[cfg(test)]
mod tests;
