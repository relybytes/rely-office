// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The pictures and charts a worksheet's drawing anchors to its cells, each with the cell it
//! starts at and the size its anchor gives it.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Cursor;

use calamine::{Data, Range, Reader, Sheets, open_workbook_auto_from_rs};

use crate::archive::{ZipPackage, read_member_bytes, read_text_member};
use crate::chart::{CellSource, ChartCell, chart_picture, chart_reference};
use crate::document::sniff_image_type;
use crate::relationships::{Relationship, read_relationships};
use crate::sheet_layout::{CellRange, SheetLayout};
use crate::theme::Theme;
use crate::xml_tree::{Element, Namespace, parse_tree};
use crate::{Limits, OfficeError};

const XDR: Namespace = Namespace::SpreadsheetDrawing;
const A: Namespace = Namespace::Drawing;
const EMU_PER_PIXEL: i64 = 9_525;
/// A twentieth of a point.
const EMU_PER_TWIP: i64 = 635;
/// Excel's default column, 8.43 characters, is 64 pixels.
const DEFAULT_COLUMN_PIXELS: i64 = 64;
/// Excel's default row, 15 points, in twentieths.
const DEFAULT_ROW_TWIPS: i64 = 300;
const MAX_DRAWINGS: usize = 200;
/// No anchor reaches further than this, about a hundred metres.
const MAX_EMU: i64 = 914_400 * 4_000;

/// A picture or a chart placed on a sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetDrawing {
    /// The cell its top-left corner is anchored in, from 0.
    pub row: u32,
    pub column: u32,
    /// How far into that cell the corner sits, across and down.
    pub offset_emu: (i64, i64),
    pub width_emu: i64,
    pub height_emu: i64,
    pub content_type: &'static str,
    pub bytes: Vec<u8>,
    /// The description a picture carries, or a chart's title.
    pub description: Option<String>,
    /// What the chart is, for one an editor can change; `None` for a picture or a chart it
    /// cannot.
    pub chart: Option<crate::chart_spec::ChartSpec>,
}

/// A part of a package kept as the file had it, to be written back unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarriedPart {
    /// The member name, `xl/charts/chart1.xml`.
    pub name: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

/// The drawing of a sheet with every part it leads to: its pictures, its charts and what they
/// embed, and the relationships between them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarriedDrawing {
    /// The member name of the drawing part the sheet points at.
    pub drawing_part: String,
    pub parts: Vec<CarriedPart>,
}

/// A pivot table of a sheet, written back as the file had it with the cache it reads: the cells
/// it shows are ordinary cells of the sheet, and what an editor changes in them the next
/// refresh replaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarriedPivot {
    /// The member name of the pivot table part the sheet points at.
    pub table_part: String,
    /// The member name of its cache definition, which the workbook lists.
    pub cache_part: String,
    /// The id the workbook gives the cache, which the pivot table names it by.
    pub cache_id: u32,
    /// The table, the cache, its records and their relationships.
    pub parts: Vec<CarriedPart>,
}

/// The most parts one drawing carries.
const MAX_CARRIED_PARTS: usize = 512;
/// The most pivot tables one sheet carries.
const MAX_PIVOTS: usize = 64;
const RELATIONSHIPS_TYPE: &str = "application/vnd.openxmlformats-package.relationships+xml";

/// The content types of a package: by part name, else by extension.
#[derive(Debug, Default)]
pub(crate) struct ContentTypes {
    defaults: HashMap<String, String>,
    overrides: HashMap<String, String>,
}

impl ContentTypes {
    pub(crate) fn parse(xml: &str) -> Self {
        let mut types = Self::default();
        let Ok(root) = parse_tree(xml, |_, _| false) else {
            return types;
        };
        for entry in root.elements() {
            let attribute = |name: &str| entry.attribute(Namespace::None, name);
            match (entry.name.as_str(), attribute("ContentType")) {
                ("Default", Some(kind)) => {
                    if let Some(extension) = attribute("Extension") {
                        types
                            .defaults
                            .insert(extension.to_ascii_lowercase(), kind.to_owned());
                    }
                }
                ("Override", Some(kind)) => {
                    if let Some(part) = attribute("PartName") {
                        types.overrides.insert(
                            part.trim_start_matches('/').to_ascii_lowercase(),
                            kind.to_owned(),
                        );
                    }
                }
                _ => {}
            }
        }
        types
    }

    fn of(&self, name: &str) -> String {
        let lower = name.to_ascii_lowercase();
        self.overrides
            .get(&lower)
            .or_else(|| {
                lower
                    .rsplit_once('.')
                    .and_then(|(_, extension)| self.defaults.get(extension))
            })
            .cloned()
            .unwrap_or_else(|| "application/octet-stream".to_owned())
    }
}

/// The relationships part of a part, `xl/drawings/_rels/drawing1.xml.rels`.
fn relationships_part(name: &str) -> String {
    match name.rsplit_once('/') {
        Some((folder, file)) => format!("{folder}/_rels/{file}.rels"),
        None => format!("_rels/{name}.rels"),
    }
}

/// The drawing of a worksheet part and every part it reaches, read as bytes; `None` for a sheet
/// without one, or one too large to carry whole.
pub(crate) fn carried_drawing(
    package: &mut ZipPackage<'_>,
    sheet_part: &str,
    limits: &Limits,
    types: &ContentTypes,
) -> Option<CarriedDrawing> {
    let drawing_part = read_relationships(package, sheet_part, limits)
        .ok()?
        .into_iter()
        .find(|relationship| !relationship.external && relationship.is_kind("drawing"))?
        .target;
    let parts = carried_tree(package, &drawing_part, limits, types)?;
    Some(CarriedDrawing {
        drawing_part,
        parts,
    })
}

/// The pivot tables of a sheet, each with the cache the workbook names by id; a table whose
/// cache the workbook does not list is left out, since nothing could refresh it.
pub(crate) fn carried_pivots(
    package: &mut ZipPackage<'_>,
    sheet_part: &str,
    caches: &HashMap<String, u32>,
    limits: &Limits,
    types: &ContentTypes,
) -> Vec<CarriedPivot> {
    let Ok(relationships) = read_relationships(package, sheet_part, limits) else {
        return Vec::new();
    };
    let mut pivots = Vec::new();
    for relationship in relationships
        .into_iter()
        .filter(|relationship| !relationship.external && relationship.is_kind("pivotTable"))
        .take(MAX_PIVOTS)
    {
        let Some(cache_part) = read_relationships(package, &relationship.target, limits)
            .ok()
            .and_then(|links| {
                links
                    .into_iter()
                    .find(|link| !link.external && link.is_kind("pivotCacheDefinition"))
            })
            .map(|link| link.target)
        else {
            continue;
        };
        let Some(cache_id) = caches.get(&cache_part).copied() else {
            continue;
        };
        if let Some(parts) = carried_tree(package, &relationship.target, limits, types) {
            pivots.push(CarriedPivot {
                table_part: relationship.target,
                cache_part,
                cache_id,
                parts,
            });
        }
    }
    pivots
}

/// A part with every part its relationships lead to, and those relationships; `None` past the
/// carrying limits or when a part cannot be read.
fn carried_tree(
    package: &mut ZipPackage<'_>,
    start: &str,
    limits: &Limits,
    types: &ContentTypes,
) -> Option<Vec<CarriedPart>> {
    let mut parts = Vec::new();
    let mut seen = HashSet::from([start.to_owned()]);
    let mut queue = VecDeque::from([start.to_owned()]);
    let mut total: u64 = 0;
    while let Some(name) = queue.pop_front() {
        let bytes = read_member_bytes(package, &name, limits.max_member_bytes).ok()??;
        total += bytes.len() as u64;
        parts.push(CarriedPart {
            content_type: types.of(&name),
            name: name.clone(),
            bytes,
        });
        let relationships_name = relationships_part(&name);
        if let Some(bytes) =
            read_member_bytes(package, &relationships_name, limits.max_member_bytes)
                .ok()
                .flatten()
        {
            total += bytes.len() as u64;
            parts.push(CarriedPart {
                name: relationships_name,
                content_type: RELATIONSHIPS_TYPE.to_owned(),
                bytes,
            });
            for relationship in read_relationships(package, &name, limits).ok()? {
                if !relationship.external && seen.insert(relationship.target.clone()) {
                    queue.push_back(relationship.target);
                }
            }
        }
        if parts.len() > MAX_CARRIED_PARTS || total > limits.max_input_bytes {
            return None;
        }
    }
    Some(parts)
}

/// How much image data a workbook still may load.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ImageBudget {
    pub images: usize,
    pub bytes: u64,
}

impl ImageBudget {
    pub(crate) fn of(limits: &Limits) -> Self {
        Self {
            images: limits.max_images,
            bytes: limits.max_total_image_bytes,
        }
    }

    fn take(&mut self, size: u64) -> bool {
        if self.images == 0 || size > self.bytes {
            return false;
        }
        self.images -= 1;
        self.bytes -= size;
        true
    }
}

/// The most cells one chart formula reads.
const MAX_FORMULA_CELLS: u64 = 16_384;

/// The workbook behind the cells, opened on first use.
enum Opened<'b> {
    NotYet,
    Open(Box<Sheets<Cursor<&'b [u8]>>>),
    Refused,
}

/// The cells of a workbook, for the charts that cache none of the values they plot: the
/// workbook is opened on the first such chart, and each sheet read once.
pub(crate) struct WorkbookCells<'b> {
    bytes: &'b [u8],
    workbook: RefCell<Opened<'b>>,
    ranges: RefCell<HashMap<String, Option<Range<Data>>>>,
}

impl<'b> WorkbookCells<'b> {
    pub(crate) fn new(bytes: &'b [u8]) -> Self {
        Self {
            bytes,
            workbook: RefCell::new(Opened::NotYet),
            ranges: RefCell::new(HashMap::new()),
        }
    }

    /// The cells of a formula naming one range of one sheet, row by row.
    pub(crate) fn cells(&self, formula: &str) -> Option<Vec<ChartCell>> {
        let (sheet, range) = split_formula(formula)?;
        let range = CellRange::parse(&range)?;
        let area = u64::from(range.last_row - range.first_row + 1)
            * u64::from(range.last_column - range.first_column + 1);
        if area > MAX_FORMULA_CELLS {
            return None;
        }
        let mut ranges = self.ranges.borrow_mut();
        if !ranges.contains_key(&sheet) {
            let mut workbook = self.workbook.borrow_mut();
            if matches!(*workbook, Opened::NotYet) {
                *workbook = open_workbook_auto_from_rs(Cursor::new(self.bytes))
                    .map_or(Opened::Refused, |opened| Opened::Open(Box::new(opened)));
            }
            let read = match &mut *workbook {
                Opened::Open(opened) => opened.worksheet_range(&sheet).ok(),
                Opened::NotYet | Opened::Refused => None,
            };
            ranges.insert(sheet.clone(), read);
        }
        let values = ranges.get(&sheet)?.as_ref()?;
        let mut cells = Vec::new();
        for row in range.first_row..=range.last_row {
            for column in range.first_column..=range.last_column {
                let cell = values
                    .get_value((row, column))
                    .map_or(ChartCell::Empty, |value| match value {
                        Data::Float(number) => ChartCell::Number(*number),
                        #[expect(
                            clippy::cast_precision_loss,
                            reason = "a cell's integer is plotted as a float"
                        )]
                        Data::Int(number) => ChartCell::Number(*number as f64),
                        Data::Bool(flag) => ChartCell::Number(f64::from(u8::from(*flag))),
                        Data::DateTime(moment) => ChartCell::Number(moment.as_f64()),
                        Data::String(text) | Data::DateTimeIso(text) | Data::DurationIso(text) => {
                            ChartCell::Text(text.clone())
                        }
                        Data::Error(_) | Data::Empty => ChartCell::Empty,
                    });
                cells.push(cell);
            }
        }
        Some(cells)
    }
}

/// The sheet and the range of a formula, `'Vendite'!$B$2:$B$7`; a formula of several ranges is
/// not one this reader follows.
fn split_formula(formula: &str) -> Option<(String, String)> {
    let formula = formula.trim().trim_start_matches('=');
    let formula = formula
        .strip_prefix('(')
        .and_then(|inner| inner.strip_suffix(')'))
        .unwrap_or(formula);
    if formula.matches('!').count() != 1 {
        return None;
    }
    let (sheet, range) = formula.split_once('!')?;
    let sheet = sheet
        .strip_prefix('\'')
        .and_then(|quoted| quoted.strip_suffix('\''))
        .map_or_else(|| sheet.to_owned(), |quoted| quoted.replace("''", "'"));
    Some((sheet, range.to_owned()))
}

/// A value a chart plots, as an editor gives it from the workbook's current values.
#[derive(Debug, Clone, PartialEq)]
pub enum ChartValue {
    Number(f64),
    Text(String),
    Empty,
}

/// The pictures and charts of a drawing carried from a file, drawn again: each chart from the
/// values `values` gives for its formulas (`'Vendite'!$B$2:$B$7`), so it follows what an editor
/// changed in the cells. `layout` gives the sizes of the rows and columns the drawings anchor to.
#[must_use]
pub fn redraw_carried_drawing(
    drawing: &CarriedDrawing,
    theme: Option<&CarriedPart>,
    layout: &SheetLayout,
    limits: &Limits,
    values: &dyn Fn(&str) -> Option<Vec<ChartValue>>,
) -> Vec<SheetDrawing> {
    use std::io::Write as _;
    const SHEET: &str = "sheet.xml";
    let mut package = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let relationships = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing" Target="{}"/></Relationships>"#,
        drawing
            .drawing_part
            .replace('&', "&amp;")
            .replace('"', "&quot;")
    );
    let mut written = || -> zip::result::ZipResult<()> {
        package.start_file(format!("_rels/{SHEET}.rels"), options)?;
        package.write_all(relationships.as_bytes())?;
        for part in &drawing.parts {
            package.start_file(part.name.as_str(), options)?;
            // A chart reads the values it cached before the cells; the cache of a formula the
            // editor answers goes, so the chart is drawn from the cells as they are now.
            match std::str::from_utf8(&part.bytes) {
                Ok(xml) if part.name.contains("/charts/") && xml.contains("chartSpace") => {
                    package.write_all(without_answered_caches(xml, values).as_bytes())?;
                }
                _ => package.write_all(&part.bytes)?,
            }
        }
        Ok(())
    };
    if written().is_err() {
        return Vec::new();
    }
    let Ok(cursor) = package.finish() else {
        return Vec::new();
    };
    let bytes = cursor.into_inner();
    let Ok(mut opened) = crate::archive::open_package(&bytes) else {
        return Vec::new();
    };
    let chart_theme = theme
        .and_then(|part| std::str::from_utf8(&part.bytes).ok())
        .map(Theme::parse)
        .unwrap_or_default();
    let cells = |formula: &str| {
        values(formula).map(|found| {
            found
                .into_iter()
                .map(|value| match value {
                    ChartValue::Number(number) => ChartCell::Number(number),
                    ChartValue::Text(text) => ChartCell::Text(text),
                    ChartValue::Empty => ChartCell::Empty,
                })
                .collect()
        })
    };
    let mut budget = ImageBudget::of(limits);
    sheet_drawings(
        &mut opened,
        SHEET,
        layout,
        &chart_theme,
        limits,
        &mut budget,
        &cells,
    )
    .unwrap_or_default()
}

/// A chart part without the cached values of the references whose formula `values` answers.
fn without_answered_caches(xml: &str, values: &dyn Fn(&str) -> Option<Vec<ChartValue>>) -> String {
    let prefix = xml
        .find("chartSpace")
        .and_then(|at| {
            let start = xml[..at].rfind('<')? + 1;
            Some(xml[start..at].to_owned())
        })
        .unwrap_or_default();
    let mut text = xml.to_owned();
    for reference in ["numRef", "strRef"] {
        let cache = if reference == "numRef" {
            "numCache"
        } else {
            "strCache"
        };
        let open = format!("<{prefix}{reference}>");
        let close = format!("</{prefix}{reference}>");
        let mut output = String::with_capacity(text.len());
        let mut rest = text.as_str();
        while let Some(start) = rest.find(&open) {
            let Some(length) = rest[start..].find(&close) else {
                break;
            };
            let end = start + length + close.len();
            let block = &rest[start..end];
            output.push_str(&rest[..start]);
            let formula = block
                .split_once(&format!("<{prefix}f>"))
                .and_then(|(_, after)| after.split_once(&format!("</{prefix}f>")))
                .map(|(formula, _)| formula.replace("&apos;", "'").replace("&amp;", "&"));
            let answered = formula.is_some_and(|formula| values(formula.trim()).is_some());
            let cache_open = format!("<{prefix}{cache}>");
            let cache_close = format!("</{prefix}{cache}>");
            match (answered, block.find(&cache_open), block.find(&cache_close)) {
                (true, Some(from), Some(to)) if from < to => {
                    output.push_str(&block[..from]);
                    output.push_str(&block[to + cache_close.len()..]);
                }
                _ => output.push_str(block),
            }
            rest = &rest[end..];
        }
        output.push_str(rest);
        text = output;
    }
    text
}

/// The sheet and the range a chart formula names, `'Vendite'!$B$2:$B$7`; `None` for a formula
/// that is not one range of one sheet.
#[must_use]
pub fn chart_formula_range(formula: &str) -> Option<(String, CellRange)> {
    let (sheet, range) = split_formula(formula)?;
    Some((sheet, CellRange::parse(&range)?))
}

/// An anchor of a drawing part as an editor changes it: where its text lies in the part, where
/// it places its drawing, and the relationship of the chart it holds, if it holds one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawingAnchor {
    /// The bytes of the part the anchor element spans, its end excluded.
    pub span: (usize, usize),
    pub row: u32,
    pub column: u32,
    pub offset_emu: (i64, i64),
    pub width_emu: i64,
    pub height_emu: i64,
    pub chart_relationship: Option<String>,
    pub picture: bool,
}

/// The anchors of a drawing part, in the order the part lists them; one that places nothing is
/// left out. `layout` gives the sizes of the rows and columns they anchor to.
#[must_use]
pub fn drawing_anchors(xml: &str, layout: &SheetLayout) -> Vec<DrawingAnchor> {
    let Ok(root) = parse_tree(xml, |_, _| false) else {
        return Vec::new();
    };
    let spans = element_spans(xml);
    let elements: Vec<&Element> = root.elements().collect();
    if spans.len() != elements.len() {
        return Vec::new();
    }
    elements
        .into_iter()
        .zip(spans)
        .filter(|(anchor, _)| anchor.namespace == XDR)
        .filter_map(|(anchor, span)| {
            let frame = anchor_frame(anchor, layout)?;
            let chart_relationship = anchor
                .child(XDR, "graphicFrame")
                .and_then(chart_reference)
                .and_then(|chart| chart.attribute(Namespace::Relationships, "id"))
                .map(str::to_owned);
            Some(DrawingAnchor {
                span,
                row: frame.row,
                column: frame.column,
                offset_emu: frame.offset_emu,
                width_emu: frame.width_emu,
                height_emu: frame.height_emu,
                chart_relationship,
                picture: anchor.child(XDR, "pic").is_some(),
            })
        })
        .collect()
}

/// The byte spans of the root's child elements.
fn element_spans(xml: &str) -> Vec<(usize, usize)> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut depth = 0_usize;
    let mut spans = Vec::new();
    let mut start = 0;
    loop {
        let before = usize::try_from(reader.buffer_position()).unwrap_or(0);
        let Ok(event) = reader.read_event() else {
            return Vec::new();
        };
        let after = usize::try_from(reader.buffer_position()).unwrap_or(0);
        match event {
            Event::Start(_) => {
                if depth == 1 {
                    start = before;
                }
                depth += 1;
            }
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                if depth == 1 {
                    spans.push((start, after));
                }
            }
            Event::Empty(_) if depth == 1 => spans.push((before, after)),
            Event::Eof => return spans,
            _ => {}
        }
    }
}

/// The drawings of a worksheet part; a drawing that cannot be read is left out.
pub(crate) fn sheet_drawings(
    package: &mut ZipPackage<'_>,
    sheet_part: &str,
    layout: &SheetLayout,
    theme: &Theme,
    limits: &Limits,
    budget: &mut ImageBudget,
    cells: CellSource<'_>,
) -> Result<Vec<SheetDrawing>, OfficeError> {
    let sheet_relationships = read_relationships(package, sheet_part, limits)?;
    let Some(drawing_part) = sheet_relationships
        .iter()
        .find(|relationship| !relationship.external && relationship.is_kind("drawing"))
        .map(|relationship| relationship.target.clone())
    else {
        return Ok(Vec::new());
    };
    let Some(xml) = read_text_member(package, &drawing_part, limits)? else {
        return Ok(Vec::new());
    };
    let root = parse_tree(&xml, |_, _| false)?;
    drop(xml);
    let relationships = read_relationships(package, &drawing_part, limits)?;
    let mut drawings = Vec::new();
    for anchor in root.elements().filter(|anchor| anchor.namespace == XDR) {
        if drawings.len() >= MAX_DRAWINGS {
            break;
        }
        let Some(AnchorFrame {
            row,
            column,
            offset_emu,
            width_emu,
            height_emu,
        }) = anchor_frame(anchor, layout)
        else {
            continue;
        };
        if width_emu <= 0 || height_emu <= 0 {
            continue;
        }
        let drawn = if let Some(picture) = anchor.child(XDR, "pic") {
            picture_of(package, picture, &relationships, limits, budget)
                .map(|(content_type, bytes, description)| (content_type, bytes, description, None))
        } else if let Some(chart) = anchor
            .child(XDR, "graphicFrame")
            .or_else(|| {
                // A chart of the 2016 kinds stands in the first choice, a note for older
                // readers in the fallback.
                anchor
                    .child(Namespace::MarkupCompatibility, "AlternateContent")?
                    .child(Namespace::MarkupCompatibility, "Choice")?
                    .child(XDR, "graphicFrame")
            })
            .and_then(chart_reference)
        {
            let frame = (width_emu, height_emu);
            chart_of(
                package,
                chart,
                &relationships,
                theme,
                limits,
                budget,
                frame,
                cells,
            )
        } else {
            None
        };
        if let Some((content_type, bytes, description, chart)) = drawn {
            drawings.push(SheetDrawing {
                row,
                column,
                offset_emu,
                width_emu,
                height_emu,
                content_type,
                bytes,
                description,
                chart,
            });
        }
    }
    Ok(drawings)
}

/// Where an anchor puts its drawing: the cell it starts in, how far into it, and the size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AnchorFrame {
    row: u32,
    column: u32,
    offset_emu: (i64, i64),
    width_emu: i64,
    height_emu: i64,
}

/// The cell an anchor starts in, how far into it, and the size it gives its drawing.
fn anchor_frame(anchor: &Element, layout: &SheetLayout) -> Option<AnchorFrame> {
    let marker = |name: &str| {
        let marker = anchor.child(XDR, name)?;
        let number = |name: &str| {
            marker
                .child(XDR, name)
                .map(Element::text)
                .and_then(|text| text.trim().parse::<i64>().ok())
        };
        Some((
            number("col")?.clamp(0, 16_383),
            number("colOff").unwrap_or(0).clamp(0, MAX_EMU),
            number("row")?.clamp(0, 1_048_575),
            number("rowOff").unwrap_or(0).clamp(0, MAX_EMU),
        ))
    };
    let extent = || {
        let extent = anchor.child(XDR, "ext")?;
        let side = |name: &str| {
            extent
                .attribute(Namespace::None, name)?
                .parse::<i64>()
                .ok()
                .map(|side| side.clamp(0, MAX_EMU))
        };
        Some((side("cx")?, side("cy")?))
    };
    let cell = |row: i64, column: i64| {
        (
            u32::try_from(row).unwrap_or(0),
            u32::try_from(column).unwrap_or(0),
        )
    };
    match anchor.name.as_str() {
        "twoCellAnchor" => {
            let (from_column, from_column_offset, from_row, from_row_offset) = marker("from")?;
            let (to_column, to_column_offset, to_row, to_row_offset) = marker("to")?;
            let width =
                columns_emu(layout, from_column, to_column) - from_column_offset + to_column_offset;
            let height = rows_emu(layout, from_row, to_row) - from_row_offset + to_row_offset;
            let (row, column) = cell(from_row, from_column);
            Some(AnchorFrame {
                row,
                column,
                offset_emu: (from_column_offset, from_row_offset),
                width_emu: width.min(MAX_EMU),
                height_emu: height.min(MAX_EMU),
            })
        }
        "oneCellAnchor" => {
            let (column, column_offset, row, row_offset) = marker("from")?;
            let (width, height) = extent()?;
            let (row, column) = cell(row, column);
            Some(AnchorFrame {
                row,
                column,
                offset_emu: (column_offset, row_offset),
                width_emu: width,
                height_emu: height,
            })
        }
        "absoluteAnchor" => {
            let (width, height) = extent()?;
            let position = anchor.child(XDR, "pos");
            let coordinate = |name: &str| {
                position
                    .and_then(|position| position.attribute(Namespace::None, name))
                    .and_then(|value| value.parse::<i64>().ok())
                    .map_or(0, |value| value.clamp(0, MAX_EMU))
            };
            Some(AnchorFrame {
                row: 0,
                column: 0,
                offset_emu: (coordinate("x"), coordinate("y")),
                width_emu: width,
                height_emu: height,
            })
        }
        _ => None,
    }
}

/// The width of the columns from `first` up to, not including, `last`.
fn columns_emu(layout: &SheetLayout, first: i64, last: i64) -> i64 {
    let default = layout
        .default_column_width
        .map_or(DEFAULT_COLUMN_PIXELS, pixels_of_width);
    (first..last.max(first))
        .take(16_384)
        .map(|column| {
            usize::try_from(column)
                .ok()
                .and_then(|column| layout.column_widths.get(column).copied().flatten())
                .map_or(default, pixels_of_width)
                * EMU_PER_PIXEL
        })
        .sum()
}

/// The height of the rows from `first` up to, not including, `last`.
fn rows_emu(layout: &SheetLayout, first: i64, last: i64) -> i64 {
    let default = layout
        .default_row_height
        .map_or(DEFAULT_ROW_TWIPS, i64::from);
    let span = last.max(first) - first;
    let set: i64 = layout
        .row_heights
        .range(u32::try_from(first).unwrap_or(0)..u32::try_from(last.max(first)).unwrap_or(0))
        .map(|(_, height)| i64::from(*height) - default)
        .sum();
    (span * default + set) * EMU_PER_TWIP
}

/// The pixels of a column `width` 256ths of a character wide, with the 5 pixels of padding Excel
/// adds to the digits of its default font.
fn pixels_of_width(width: u32) -> i64 {
    if width == 0 {
        return 0;
    }
    (i64::from(width) * 7 + 128) / 256 + 5
}

fn picture_of(
    package: &mut ZipPackage<'_>,
    picture: &Element,
    relationships: &[Relationship],
    limits: &Limits,
    budget: &mut ImageBudget,
) -> Option<(&'static str, Vec<u8>, Option<String>)> {
    let embed = picture
        .descendant(A, "blip")?
        .attribute(Namespace::Relationships, "embed")?;
    let target = &relationships
        .iter()
        .find(|relationship| relationship.id == embed && !relationship.external)?
        .target;
    let cap = limits.max_image_bytes.min(limits.max_member_bytes);
    let bytes = read_member_bytes(package, target, cap).ok()??;
    if !budget.take(bytes.len() as u64) {
        return None;
    }
    let description = picture
        .descendant(XDR, "cNvPr")
        .and_then(|properties| {
            properties
                .attribute(Namespace::None, "descr")
                .or_else(|| properties.attribute(Namespace::None, "title"))
        })
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned);
    Some((sniff_image_type(&bytes), bytes, description))
}

/// A drawing as read: its type, its bytes, its description and what the chart is.
type Drawn = (
    &'static str,
    Vec<u8>,
    Option<String>,
    Option<crate::chart_spec::ChartSpec>,
);

#[expect(
    clippy::too_many_arguments,
    reason = "a chart needs its part, the workbook's theme and limits and its own frame"
)]
fn chart_of(
    package: &mut ZipPackage<'_>,
    chart: &Element,
    relationships: &[Relationship],
    theme: &Theme,
    limits: &Limits,
    budget: &mut ImageBudget,
    (width_emu, height_emu): (i64, i64),
    cells: CellSource<'_>,
) -> Option<Drawn> {
    let id = chart.attribute(Namespace::Relationships, "id")?;
    let target = &relationships
        .iter()
        .find(|relationship| relationship.id == id && !relationship.external)?
        .target;
    let xml = read_text_member(package, target, limits).ok().flatten()?;
    let (bytes, title) = chart_picture(&xml, theme, width_emu, height_emu, cells)?;
    if !budget.take(bytes.len() as u64) {
        return None;
    }
    Some((
        "image/svg+xml",
        bytes,
        title,
        crate::chart_spec::read_chart_spec(&xml),
    ))
}

#[cfg(test)]
mod tests;
