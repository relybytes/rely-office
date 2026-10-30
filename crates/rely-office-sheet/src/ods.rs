// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! A workbook written as ods: each sheet a table with its column widths, row heights, merged
//! cells, cell styles and number formats, every formula in `OpenFormula` with its computed
//! value, and the conditional formats and validations as `LibreOffice` keeps them. Pictures,
//! charts and pivot tables are written to xlsx only.

mod formula;
mod rules;
mod styles;

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;
use std::io::{Cursor, Write};

use rely_office_core::{OfficeError, odf_chart_object_xml};
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

use self::formula::odf_formula;
use self::rules::{
    ConditionalStyles, conditional_formats_xml, content_validations_xml, validation_name,
};
use self::styles::OdsStyles;
use crate::address::{CellRef, column_name};
use crate::dates::date_from_serial;
use crate::formula::number_text;
use crate::model::{Comment, NumberFormat, Sheet, Workbook};
use crate::value::{ErrorKind, Input, Locale, Value};

const MIMETYPE: &str = "application/vnd.oasis.opendocument.spreadsheet";
/// The most rows and columns a validation's ranges stretch a table to, past its content.
const MAX_VALIDATED_ROWS: u32 = 65_536;
const MAX_VALIDATED_COLUMNS: u32 = 1_024;
/// Excel's default column as a file stores its width, 8.43 characters and the padding.
const DEFAULT_COLUMN_CHARACTERS: f64 = 9.140_625;

const NAMESPACES: &str = concat!(
    r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
    r#"xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" "#,
    r#"xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" "#,
    r#"xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" "#,
    r#"xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" "#,
    r#"xmlns:number="urn:oasis:names:tc:opendocument:xmlns:datastyle:1.0" "#,
    r#"xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" "#,
    r#"xmlns:of="urn:oasis:names:tc:opendocument:xmlns:of:1.2" "#,
    r#"xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0" "#,
    r#"xmlns:calcext="urn:org:documentfoundation:names:experimental:calc:xmlns:calcext:1.0" "#,
    r#"xmlns:loext="urn:org:documentfoundation:names:experimental:office:xmlns:loext:1.0" "#,
    r#"xmlns:dc="http://purl.org/dc/elements/1.1/" "#,
    r#"xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" "#,
    r#"xmlns:xlink="http://www.w3.org/1999/xlink" "#,
    r#"office:version="1.3""#
);

/// Save a workbook as ods.
///
/// # Errors
///
/// [`OfficeError::DamagedContainer`] when the zip cannot be written, which only a failing
/// allocator causes.
pub fn write_ods(workbook: &Workbook) -> Result<Vec<u8>, OfficeError> {
    let damaged = |error: &dyn std::fmt::Display| OfficeError::DamagedContainer(error.to_string());
    let mut package = zip::ZipWriter::new(Cursor::new(Vec::new()));
    // The mimetype comes first and uncompressed, so a reader knows the file by its first bytes.
    package
        .start_file(
            "mimetype",
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
        )
        .map_err(|error| damaged(&error))?;
    package
        .write_all(MIMETYPE.as_bytes())
        .map_err(|error| damaged(&error))?;
    let conditional = ConditionalStyles::new(workbook);
    let objects = Objects::new(workbook);
    let manifest = MANIFEST.replace(
        "</manifest:manifest>",
        &format!("{}</manifest:manifest>", objects.manifest),
    );
    for (name, content) in [
        ("META-INF/manifest.xml", manifest.into_bytes()),
        ("meta.xml", META.as_bytes().to_vec()),
        ("settings.xml", settings_part(workbook).into_bytes()),
        ("styles.xml", styles_part(&conditional).into_bytes()),
        (
            "content.xml",
            content_part(workbook, &conditional, &objects).into_bytes(),
        ),
    ]
    .into_iter()
    .chain(
        objects
            .files
            .iter()
            .map(|(name, bytes)| (name.as_str(), bytes.clone())),
    ) {
        package
            .start_file(name, SimpleFileOptions::default())
            .map_err(|error| damaged(&error))?;
        package
            .write_all(&content)
            .map_err(|error| damaged(&error))?;
    }
    package
        .finish()
        .map(Cursor::into_inner)
        .map_err(|error| damaged(&error))
}

/// The pictures and charts of every sheet as `OpenDocument` writes them: a frame in the cell
/// each is anchored to, a chart object or a picture for each, and the manifest entries.
struct Objects {
    frames: Vec<HashMap<CellRef, String>>,
    files: Vec<(String, Vec<u8>)>,
    manifest: String,
}

const EMU_PER_INCH: f64 = 914_400.0;

impl Objects {
    fn new(workbook: &Workbook) -> Self {
        let mut objects = Self {
            frames: Vec::with_capacity(workbook.sheets.len()),
            files: Vec::new(),
            manifest: String::new(),
        };
        let mut number = 0;
        for index in 0..workbook.sheets.len() {
            let mut frames: HashMap<CellRef, String> = HashMap::new();
            for drawing in workbook.pictures(index) {
                number += 1;
                #[expect(clippy::cast_precision_loss, reason = "lengths of a few pages")]
                let inches = |emu: i64| emu as f64 / EMU_PER_INCH;
                // A frame in a cell is placed from the cell's corner.
                let geometry = format!(
                    r#"svg:x="{:.4}in" svg:y="{:.4}in" svg:width="{:.4}in" svg:height="{:.4}in""#,
                    inches(drawing.offset_emu.0),
                    inches(drawing.offset_emu.1),
                    inches(drawing.width_emu),
                    inches(drawing.height_emu)
                );
                let content = if let Some(spec) = &drawing.chart {
                    let folder = format!("Object {number}");
                    objects.files.push((
                        format!("{folder}/content.xml"),
                        odf_chart_object_xml(spec).into_bytes(),
                    ));
                    let _ = write!(
                        objects.manifest,
                        concat!(
                            r#"<manifest:file-entry manifest:full-path="{0}/" manifest:media-type="application/vnd.oasis.opendocument.chart"/>"#,
                            r#"<manifest:file-entry manifest:full-path="{0}/content.xml" manifest:media-type="text/xml"/>"#
                        ),
                        escape(&folder)
                    );
                    format!(
                        r#"<draw:object xlink:href="./{}" xlink:type="simple" xlink:show="embed" xlink:actuate="onLoad"/>"#,
                        escape(&folder)
                    )
                } else {
                    let extension = match drawing.content_type {
                        "image/jpeg" => "jpeg",
                        "image/gif" => "gif",
                        "image/svg+xml" => "svg",
                        _ => "png",
                    };
                    let name = format!("Pictures/image{number}.{extension}");
                    let _ = write!(
                        objects.manifest,
                        r#"<manifest:file-entry manifest:full-path="{}" manifest:media-type="{}"/>"#,
                        escape(&name),
                        drawing.content_type
                    );
                    objects.files.push((name.clone(), drawing.bytes.clone()));
                    format!(
                        r#"<draw:image xlink:href="{}" xlink:type="simple" xlink:show="embed" xlink:actuate="onLoad"/>"#,
                        escape(&name)
                    )
                };
                let title = drawing
                    .description
                    .as_deref()
                    .map_or_else(String::new, |text| {
                        format!("<svg:title>{}</svg:title>", escape(text))
                    });
                let _ = write!(
                    frames
                        .entry(CellRef::new(drawing.row, drawing.column))
                        .or_default(),
                    r#"<draw:frame draw:z-index="{number}" draw:name="Object {number}" {geometry}>{content}{title}</draw:frame>"#
                );
            }
            objects.frames.push(frames);
        }
        objects
    }
}

/// A sheet name as an `OpenDocument` address names it, quoted.
fn quoted_sheet(name: &str) -> String {
    format!("'{}'", name.replace('\'', "''"))
}

/// The names the workbook defines for `sheet`, or for the whole workbook with `None`, each a
/// named expression in `OpenFormula`, its relative references read from the first cell.
fn named_expressions_xml(workbook: &Workbook, sheet: Option<usize>) -> String {
    let mut xml = String::new();
    let home = &workbook.sheets[sheet.unwrap_or(0)].name;
    for defined in workbook
        .names
        .iter()
        .filter(|defined| defined.sheet == sheet)
    {
        let _ = write!(
            xml,
            r#"<table:named-expression table:name="{}" table:base-cell-address="{}.$A$1" table:expression="of:={}"/>"#,
            escape(&defined.name),
            escape(&format!("${}", quoted_sheet(home))),
            escape(&odf_formula(&defined.expression))
        );
    }
    if xml.is_empty() {
        xml
    } else {
        format!("<table:named-expressions>{xml}</table:named-expressions>")
    }
}

/// The annotation `LibreOffice` shows for a cell's comment. ODF has no threads, so the replies
/// of a thread follow its text, a paragraph each opening with its author.
fn annotation_xml(comment: &Comment) -> String {
    let mut xml = String::from("<office:annotation>");
    if !comment.author.is_empty() {
        let _ = write!(xml, "<dc:creator>{}</dc:creator>", escape(&comment.author));
    }
    if let Some(date) = comment.date.as_deref().and_then(|date| date.get(..19)) {
        let _ = write!(xml, "<dc:date>{}</dc:date>", escape(date));
    }
    for line in comment.text.split('\n') {
        let _ = write!(xml, "<text:p>{}</text:p>", paragraph_text(line));
    }
    for reply in &comment.replies {
        for (number, line) in reply.text.split('\n').enumerate() {
            let line = if number == 0 && !reply.author.is_empty() {
                format!("{}: {line}", reply.author)
            } else {
                line.to_owned()
            };
            let _ = write!(xml, "<text:p>{}</text:p>", paragraph_text(&line));
        }
    }
    xml.push_str("</office:annotation>");
    xml
}

/// The filter of each sheet, as `LibreOffice` keeps it: a database range with filter buttons,
/// every filtered column an "or" of the values it shows.
fn database_ranges_xml(workbook: &Workbook) -> String {
    let mut xml = String::new();
    for (index, sheet) in workbook.sheets.iter().enumerate() {
        let Some(filter) = sheet.auto_filter() else {
            continue;
        };
        let range = filter.range;
        let sheet_name = quoted_sheet(&sheet.name);
        let _ = write!(
            xml,
            r#"<table:database-range table:name="__Anonymous_Sheet_DB__{index}" table:target-range-address="{}.{}{}:{}.{}{}" table:display-filter-buttons="true">"#,
            escape(&sheet_name),
            column_name(range.first_column),
            range.first_row + 1,
            escape(&sheet_name),
            column_name(range.last_column),
            range.last_row + 1
        );
        let columns: Vec<u32> = filter.filtered_columns().collect();
        if !columns.is_empty() {
            xml.push_str("<table:filter><table:filter-and>");
            for column in columns {
                xml.push_str("<table:filter-or>");
                for value in filter.shown(column).into_iter().flatten() {
                    if value.is_empty() {
                        let _ = write!(
                            xml,
                            r#"<table:filter-condition table:field-number="{column}" table:value="" table:operator="empty"/>"#
                        );
                    } else {
                        let _ = write!(
                            xml,
                            r#"<table:filter-condition table:field-number="{column}" table:value="{}" table:operator="="/>"#,
                            escape(value)
                        );
                    }
                }
                xml.push_str("</table:filter-or>");
            }
            xml.push_str("</table:filter-and></table:filter>");
        }
        xml.push_str("</table:database-range>");
    }
    if xml.is_empty() {
        xml
    } else {
        format!("<table:database-ranges>{xml}</table:database-ranges>")
    }
}

/// The view settings `LibreOffice` reads the frozen rows and columns of each sheet from.
fn settings_part(workbook: &Workbook) -> String {
    let mut tables = String::new();
    for sheet in &workbook.sheets {
        if sheet.frozen_rows == 0 && sheet.frozen_columns == 0 {
            continue;
        }
        let item = |name: &str, kind: &str, value: u32| {
            format!(
                r#"<config:config-item config:name="{name}" config:type="{kind}">{value}</config:config-item>"#
            )
        };
        let horizontal = if sheet.frozen_columns > 0 { 2 } else { 0 };
        let vertical = if sheet.frozen_rows > 0 { 2 } else { 0 };
        let _ = write!(
            tables,
            r#"<config:config-item-map-entry config:name="{}">{}{}{}{}{}{}{}{}{}</config:config-item-map-entry>"#,
            escape(&sheet.name),
            item("HorizontalSplitMode", "short", horizontal),
            item("VerticalSplitMode", "short", vertical),
            item("HorizontalSplitPosition", "int", sheet.frozen_columns),
            item("VerticalSplitPosition", "int", sheet.frozen_rows),
            item("ActiveSplitRange", "short", 2),
            item("PositionLeft", "int", 0),
            item("PositionRight", "int", sheet.frozen_columns),
            item("PositionTop", "int", 0),
            item("PositionBottom", "int", sheet.frozen_rows),
        );
    }
    let views = if tables.is_empty() {
        String::new()
    } else {
        format!(
            concat!(
                r#"<config:config-item-set config:name="ooo:view-settings"><config:config-item-map-indexed config:name="Views">"#,
                r#"<config:config-item-map-entry><config:config-item config:name="ViewId" config:type="string">view1</config:config-item>"#,
                r#"<config:config-item-map-named config:name="Tables">{}</config:config-item-map-named>"#,
                "</config:config-item-map-entry></config:config-item-map-indexed></config:config-item-set>"
            ),
            tables
        )
    };
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8"?>"#,
            r#"<office:document-settings xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
            r#"xmlns:config="urn:oasis:names:tc:opendocument:xmlns:config:1.0" office:version="1.3">"#,
            "<office:settings>{}</office:settings></office:document-settings>"
        ),
        views
    )
}

const MANIFEST: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    r#"<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.3">"#,
    r#"<manifest:file-entry manifest:full-path="/" manifest:version="1.3" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/>"#,
    r#"<manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>"#,
    r#"<manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/>"#,
    r#"<manifest:file-entry manifest:full-path="meta.xml" manifest:media-type="text/xml"/>"#,
    r#"<manifest:file-entry manifest:full-path="settings.xml" manifest:media-type="text/xml"/>"#,
    "</manifest:manifest>"
);

const META: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    r#"<office:document-meta xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0" office:version="1.3">"#,
    "<office:meta><meta:generator>rely-office</meta:generator></office:meta></office:document-meta>"
);

fn styles_part(conditional: &ConditionalStyles) -> String {
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8"?><office:document-styles {}>"#,
            "<office:styles>",
            r#"<style:default-style style:family="table-cell"><style:paragraph-properties style:tab-stop-distance="0.5in"/>"#,
            r#"<style:text-properties fo:font-family="Calibri" fo:font-size="11pt"/></style:default-style>"#,
            r#"<style:style style:name="Default" style:family="table-cell"/>"#,
            "{}",
            "</office:styles>",
            r#"<office:automatic-styles><style:page-layout style:name="pm1"><style:page-layout-properties fo:page-width="8.268in" fo:page-height="11.693in" style:print-orientation="portrait" fo:margin-top="0.75in" fo:margin-bottom="0.75in" fo:margin-left="0.7in" fo:margin-right="0.7in"/></style:page-layout></office:automatic-styles>"#,
            r#"<office:master-styles><style:master-page style:name="Default" style:page-layout-name="pm1"/></office:master-styles>"#,
            "</office:document-styles>"
        ),
        NAMESPACES,
        conditional.xml()
    )
}

/// The width in inches of a column whose file width is `characters`, as Excel turns it into
/// pixels of its default font at 96 dpi.
fn column_inches(characters: f64) -> f64 {
    ((characters * 256.0 + 18.0) / 256.0 * 7.0).trunc() / 96.0
}

fn content_part(workbook: &Workbook, conditional: &ConditionalStyles, objects: &Objects) -> String {
    let styles = OdsStyles::new(workbook);
    let mut column_styles: Vec<String> = Vec::new();
    let mut row_styles: Vec<String> = Vec::new();
    let mut tables = String::new();
    for (index, sheet) in workbook.sheets.iter().enumerate() {
        tables.push_str(&table_xml(
            workbook,
            index,
            sheet,
            (&styles, conditional),
            &objects.frames[index],
            &mut column_styles,
            &mut row_styles,
        ));
    }
    let mut automatic = String::new();
    for (index, width) in column_styles.iter().enumerate() {
        let _ = write!(
            automatic,
            r#"<style:style style:name="co{}" style:family="table-column"><style:table-column-properties fo:break-before="auto" style:column-width="{width}"/></style:style>"#,
            index + 1
        );
    }
    for (index, height) in row_styles.iter().enumerate() {
        let properties = if height.is_empty() {
            r#"style:use-optimal-row-height="true""#.to_owned()
        } else {
            format!(r#"style:row-height="{height}" style:use-optimal-row-height="false""#)
        };
        let _ = write!(
            automatic,
            r#"<style:style style:name="ro{}" style:family="table-row"><style:table-row-properties fo:break-before="auto" {properties}/></style:style>"#,
            index + 1
        );
    }
    automatic.push_str(&styles.xml);
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content {}>"#,
            "<office:automatic-styles>{}</office:automatic-styles>",
            "<office:body><office:spreadsheet>{}{}{}{}</office:spreadsheet></office:body>",
            "</office:document-content>"
        ),
        NAMESPACES,
        automatic,
        content_validations_xml(workbook),
        tables,
        named_expressions_xml(workbook, None),
        database_ranges_xml(workbook)
    )
}

/// The index, from 1, of a style among those already made, adding it when it is new.
fn style_index(styles: &mut Vec<String>, value: String) -> usize {
    styles
        .iter()
        .position(|known| *known == value)
        .unwrap_or_else(|| {
            styles.push(value);
            styles.len() - 1
        })
        + 1
}

/// The repeats of identical neighbours folded into one: `(item, count)` in order.
fn runs<T: PartialEq>(items: Vec<T>) -> Vec<(T, u32)> {
    let mut folded: Vec<(T, u32)> = Vec::new();
    for item in items {
        match folded.last_mut() {
            Some((last, count)) if *last == item => *count += 1,
            _ => folded.push((item, 1)),
        }
    }
    folded
}

#[expect(
    clippy::too_many_lines,
    reason = "the columns, the merges and the rows of one table"
)]
fn table_xml(
    workbook: &Workbook,
    index: usize,
    sheet: &Sheet,
    (styles, conditional): (&OdsStyles, &ConditionalStyles),
    frames: &HashMap<CellRef, String>,
    column_styles: &mut Vec<String>,
    row_styles: &mut Vec<String>,
) -> String {
    let (mut rows, mut columns) = sheet.extent();
    for cell in sheet.styles.keys() {
        rows = rows.max(cell.row + 1);
        columns = columns.max(cell.column + 1);
    }
    for cell in sheet.comments.keys().chain(frames.keys()) {
        rows = rows.max(cell.row + 1);
        columns = columns.max(cell.column + 1);
    }
    for merge in &sheet.merges {
        rows = rows.max(merge.last_row + 1);
        columns = columns.max(merge.last_column + 1);
    }
    for (_, spill) in workbook.spills(index) {
        rows = rows.max(spill.last_row + 1);
        columns = columns.max(spill.last_column + 1);
    }
    for range in sheet
        .validations
        .iter()
        .flat_map(|validation| &validation.ranges)
    {
        rows = rows.max((range.last_row + 1).min(MAX_VALIDATED_ROWS));
        columns = columns.max((range.last_column + 1).min(MAX_VALIDATED_COLUMNS));
    }
    if let Some((last, _)) = sheet.row_heights.iter().next_back() {
        rows = rows.max(last + 1);
    }
    if let Some((last, _)) = sheet.column_widths.iter().next_back() {
        columns = columns.max(last + 1);
    }
    let columns = columns.max(1);
    let mut xml = format!(r#"<table:table table:name="{}">"#, escape(&sheet.name));
    let column_cells: Vec<(usize, bool)> = (0..columns)
        .map(|column| {
            let width = sheet.column_widths.get(&column).copied();
            let hidden = width == Some(0.0);
            let inches = column_inches(
                width
                    .filter(|width| *width > 0.0)
                    .unwrap_or(DEFAULT_COLUMN_CHARACTERS),
            );
            (style_index(column_styles, format!("{inches:.4}in")), hidden)
        })
        .collect();
    for ((style, hidden), count) in runs(column_cells) {
        let _ = write!(xml, r#"<table:table-column table:style-name="co{style}""#);
        if count > 1 {
            let _ = write!(xml, r#" table:number-columns-repeated="{count}""#);
        }
        if hidden {
            xml.push_str(r#" table:visibility="collapse""#);
        }
        xml.push_str(r#" table:default-cell-style-name="Default"/>"#);
    }
    let covered: BTreeSet<CellRef> = sheet
        .merges
        .iter()
        .flat_map(|merge| {
            (merge.first_row..=merge.last_row).flat_map(move |row| {
                (merge.first_column..=merge.last_column)
                    .map(move |column| CellRef::new(row, column))
                    .filter(move |cell| *cell != CellRef::new(merge.first_row, merge.first_column))
            })
        })
        .collect();
    let spans: HashMap<CellRef, (u32, u32)> = sheet
        .merges
        .iter()
        .map(|merge| {
            (
                CellRef::new(merge.first_row, merge.first_column),
                (
                    merge.last_column - merge.first_column + 1,
                    merge.last_row - merge.first_row + 1,
                ),
            )
        })
        .collect();
    let mut row_lines = Vec::with_capacity(usize::try_from(rows).unwrap_or(0));
    for row in 0..rows {
        let height = sheet.row_heights.get(&row).copied();
        let style = style_index(
            row_styles,
            height
                .filter(|height| *height > 0.0)
                .map_or_else(String::new, |height| format!("{}pt", number_text(height))),
        );
        let mut cells = Vec::with_capacity(usize::try_from(columns).unwrap_or(0));
        for column in 0..columns {
            let cell = CellRef::new(row, column);
            let written = cell_xml(workbook, index, sheet, styles, cell, &covered, &spans);
            cells.push(match frames.get(&cell) {
                Some(frame) => with_frames(&written, frame),
                None => written,
            });
        }
        let mut line = format!(r#"<table:table-row table:style-name="ro{style}""#);
        let filtered = sheet
            .auto_filter()
            .is_some_and(|filter| filter.hidden_rows().any(|hidden| hidden == row));
        if filtered {
            line.push_str(r#" table:visibility="filter""#);
        } else if height == Some(0.0) {
            line.push_str(r#" table:visibility="collapse""#);
        }
        line.push('>');
        for (cell, count) in runs(cells) {
            if count > 1 {
                // A run of the same cell is written once, repeated.
                line.push_str(&with_attribute(
                    &cell,
                    &format!(r#" table:number-columns-repeated="{count}""#),
                ));
            } else {
                line.push_str(&cell);
            }
        }
        line.push_str("</table:table-row>");
        row_lines.push(line);
    }
    for (line, count) in runs(row_lines) {
        if count > 1 {
            xml.push_str(&with_attribute(
                &line,
                &format!(r#" table:number-rows-repeated="{count}""#),
            ));
        } else {
            xml.push_str(&line);
        }
    }
    if rows == 0 {
        let _ = write!(
            xml,
            r#"<table:table-row table:style-name="ro{}"><table:table-cell/></table:table-row>"#,
            style_index(row_styles, String::new())
        );
    }
    xml.push_str(&conditional_formats_xml(&sheet.name, sheet, conditional));
    xml.push_str(&named_expressions_xml(workbook, Some(index)));
    xml.push_str("</table:table>");
    xml
}

#[expect(
    clippy::too_many_lines,
    reason = "one cell's attributes, value and annotation in order"
)]
fn cell_xml(
    workbook: &Workbook,
    index: usize,
    sheet: &Sheet,
    styles: &OdsStyles,
    cell: CellRef,
    covered: &BTreeSet<CellRef>,
    spans: &HashMap<CellRef, (u32, u32)>,
) -> String {
    let element = if covered.contains(&cell) {
        "table:covered-table-cell"
    } else {
        "table:table-cell"
    };
    let content = sheet.cells.get(&cell);
    let format = content.map_or(NumberFormat::General, |content| content.format);
    let style_id = sheet.styles.get(&cell).copied().unwrap_or(0);
    let mut xml = format!("<{element}");
    if let Some(style) = styles.name(style_id, format) {
        let _ = write!(xml, r#" table:style-name="{style}""#);
    }
    if let Some(name) = validation_name(index, sheet, cell) {
        let _ = write!(xml, r#" table:content-validation-name="{name}""#);
    }
    if element == "table:table-cell"
        && let Some((columns, rows)) = spans.get(&cell)
    {
        let _ = write!(
            xml,
            r#" table:number-columns-spanned="{columns}" table:number-rows-spanned="{rows}""#
        );
    }
    // A cell an array spills into has no content of its own but shows the array's value.
    let spilled = content.is_none() && workbook.spill_anchor(index, cell).is_some();
    let annotation = sheet
        .comments
        .get(&cell)
        .filter(|_| element == "table:table-cell")
        .map(annotation_xml)
        .unwrap_or_default();
    if content.is_none() && !spilled {
        if annotation.is_empty() {
            xml.push_str("/>");
        } else {
            let _ = write!(xml, ">{annotation}</{element}>");
        }
        return xml;
    }
    let value = workbook.value(index, cell);
    if let Some(Input::Formula(expression)) = content.map(|content| &content.input) {
        let _ = write!(
            xml,
            r#" table:formula="of:={}""#,
            escape(&odf_formula(expression))
        );
        if let Some((_, spill)) = workbook
            .spills(index)
            .into_iter()
            .find(|(anchor, _)| *anchor == cell)
        {
            let _ = write!(
                xml,
                r#" table:number-matrix-columns-spanned="{}" table:number-matrix-rows-spanned="{}""#,
                spill.last_column - spill.first_column + 1,
                spill.last_row - spill.first_row + 1
            );
        }
    }
    let shown = workbook.display(index, cell, Locale::default());
    match &value {
        Value::Number(number) => match format {
            NumberFormat::Date | NumberFormat::DateTime => match iso_date(*number, format) {
                Some(date) => {
                    let _ = write!(
                        xml,
                        r#" office:value-type="date" office:date-value="{date}" calcext:value-type="date""#
                    );
                }
                None => float_value(&mut xml, *number, "float"),
            },
            NumberFormat::Percent => float_value(&mut xml, *number, "percentage"),
            _ => float_value(&mut xml, *number, "float"),
        },
        Value::Text(_) => {
            xml.push_str(r#" office:value-type="string" calcext:value-type="string""#);
        }
        Value::Bool(flag) => {
            let _ = write!(
                xml,
                r#" office:value-type="boolean" office:boolean-value="{flag}" calcext:value-type="boolean""#
            );
        }
        Value::Error(_) => {
            xml.push_str(
                r#" office:value-type="float" office:value="0" calcext:value-type="error""#,
            );
        }
        Value::Empty => {}
    }
    xml.push('>');
    xml.push_str(&annotation);
    let text = match &value {
        Value::Error(ErrorKind::Circular) => "Err:522".to_owned(),
        Value::Empty => String::new(),
        _ => shown,
    };
    for line in text.split('\n') {
        let _ = write!(xml, "<text:p>{}</text:p>", paragraph_text(line));
    }
    let _ = write!(xml, "</{element}>");
    xml
}

/// A cell with the frames anchored to it as its last content.
fn with_frames(cell: &str, frames: &str) -> String {
    if let Some(open) = cell.strip_suffix("/>") {
        let name = open
            .trim_start_matches('<')
            .split([' ', '>'])
            .next()
            .unwrap_or("table:table-cell");
        return format!("{open}>{frames}</{name}>");
    }
    match cell.rfind("</") {
        Some(end) => format!("{}{frames}{}", &cell[..end], &cell[end..]),
        None => cell.to_owned(),
    }
}

/// An element with an attribute added at the end of its start tag, which ends at the first `>`
/// outside a quoted value.
fn with_attribute(element: &str, attribute: &str) -> String {
    let mut quoted = false;
    let mut end = element.len();
    for (position, character) in element.char_indices() {
        match character {
            '"' => quoted = !quoted,
            '>' if !quoted => {
                end = position;
                break;
            }
            _ => {}
        }
    }
    let end = if element[..end].ends_with('/') {
        end - 1
    } else {
        end
    };
    format!("{}{attribute}{}", &element[..end], &element[end..])
}

fn float_value(xml: &mut String, number: f64, kind: &str) {
    let _ = write!(
        xml,
        r#" office:value-type="{kind}" office:value="{}" calcext:value-type="{kind}""#,
        number_text(number)
    );
}

/// A date serial as `office:date-value` writes it: `2026-10-27`, and its time when the format
/// shows one.
fn iso_date(serial: f64, format: NumberFormat) -> Option<String> {
    let (year, month, day) = date_from_serial(serial)?;
    let mut date = format!("{year:04}-{month:02}-{day:02}");
    if format == NumberFormat::DateTime {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the seconds of one day"
        )]
        let seconds = ((serial - serial.floor()) * 86_400.0).round() as u32;
        let _ = write!(
            date,
            "T{:02}:{:02}:{:02}",
            seconds / 3_600 % 24,
            seconds / 60 % 60,
            seconds % 60
        );
    }
    Some(date)
}

/// The text of a paragraph, with the spaces an ods would otherwise collapse kept as
/// `text:s` and tabs as `text:tab`.
fn paragraph_text(line: &str) -> String {
    let mut xml = String::new();
    let mut spaces = 0_usize;
    let mut first = true;
    let flush = |xml: &mut String, spaces: &mut usize, leading: bool| {
        if *spaces == 0 {
            return;
        }
        // A single space between words is kept as it is; any other run is written out.
        if *spaces == 1 && !leading {
            xml.push(' ');
        } else if leading {
            let _ = write!(xml, r#"<text:s text:c="{spaces}"/>"#);
        } else {
            let _ = write!(xml, r#" <text:s text:c="{}"/>"#, *spaces - 1);
        }
        *spaces = 0;
    };
    for character in line.chars() {
        match character {
            ' ' => spaces += 1,
            '\t' => {
                flush(&mut xml, &mut spaces, first);
                xml.push_str("<text:tab/>");
                first = false;
            }
            other => {
                flush(&mut xml, &mut spaces, first);
                first = false;
                xml.push_str(&escape(&other.to_string()));
            }
        }
    }
    if spaces > 0 {
        let _ = write!(xml, r#"<text:s text:c="{spaces}"/>"#);
    }
    xml
}

/// Text fit for XML content and attributes.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            control if control.is_control() && !matches!(control, '\n' | '\t') => {}
            other => escaped.push(other),
        }
    }
    escaped
}

#[cfg(test)]
mod tests;
