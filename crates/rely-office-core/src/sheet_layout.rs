// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! How the sheets of an xlsx are printed: orientation, margins, column widths, header and footer,
//! grid lines and the default font. The worksheet parts are read as a stream that skips the cells,
//! which the spreadsheet reader has already read.

use std::collections::HashMap;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::archive::{open_package, read_text_member};
use crate::relationships::{main_part_path, read_relationships};
use crate::xml_text::attribute_value;
use crate::{Limits, OfficeError};

const WORKBOOK_PART: &str = "xl/workbook.xml";
/// The most columns given a width; Excel's own limit is 16,384.
const MAX_WIDTH_COLUMNS: u32 = 16_384;

/// The print settings of a sheet; unset means the defaults of the suite that prints it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SheetLayout {
    pub landscape: bool,
    /// Page margins in thousandths of an inch: top, right, bottom, left, then the distance of the
    /// header from the top and of the footer from the bottom.
    pub margins_mils: Option<[u32; 6]>,
    /// The width of each column from A in 256ths of a character, 0 for a hidden column; a column
    /// past the end, or `None`, has the default width.
    pub column_widths: Vec<Option<u32>>,
    /// The default column width in 256ths of a character.
    pub default_column_width: Option<u32>,
    /// The header and footer as Excel codes them (`&C&A`, `&CPage &P`).
    pub header: Option<String>,
    pub footer: Option<String>,
    pub grid_lines: bool,
    /// The family and size, in tenths of a point, of the workbook's default font.
    pub font_family: Option<String>,
    pub font_size_tenths: Option<u32>,
}

/// The layout of every sheet of an xlsx, by sheet name; a package it cannot read gives none, as
/// the sheets themselves were read by then and print with the defaults.
pub(crate) fn read_xlsx_layouts(bytes: &[u8], limits: &Limits) -> HashMap<String, SheetLayout> {
    read_layouts(bytes, limits).unwrap_or_default()
}

fn read_layouts(
    bytes: &[u8],
    limits: &Limits,
) -> Result<HashMap<String, SheetLayout>, OfficeError> {
    let mut package = open_package(bytes)?;
    let workbook_part = main_part_path(&mut package, limits, WORKBOOK_PART)?;
    let Some(workbook) = read_text_member(&mut package, &workbook_part, limits)? else {
        return Ok(HashMap::new());
    };
    let relationships = read_relationships(&mut package, &workbook_part, limits)?;
    let (font_family, font_size_tenths) = relationships
        .iter()
        .find(|relationship| !relationship.external && relationship.is_kind("styles"))
        .map(|relationship| read_text_member(&mut package, &relationship.target, limits))
        .transpose()?
        .flatten()
        .map(|styles| default_font(&styles))
        .unwrap_or_default();

    let mut layouts = HashMap::new();
    for (name, relationship_id) in sheet_entries(&workbook)? {
        let Some(relationship) = relationships
            .iter()
            .find(|relationship| relationship.id == relationship_id && !relationship.external)
        else {
            continue;
        };
        let Some(sheet) = read_text_member(&mut package, &relationship.target, limits)? else {
            continue;
        };
        let mut layout = sheet_layout(&sheet)?;
        layout.font_family.clone_from(&font_family);
        layout.font_size_tenths = font_size_tenths;
        layouts.insert(name, layout);
    }
    Ok(layouts)
}

/// The name and relationship id of each `sheet` of the workbook part.
fn sheet_entries(workbook: &str) -> Result<Vec<(String, String)>, OfficeError> {
    let mut reader = Reader::from_str(workbook);
    let mut entries = Vec::new();
    loop {
        match reader.read_event().map_err(malformed)? {
            Event::Start(element) | Event::Empty(element)
                if element.local_name().as_ref() == "sheet" =>
            {
                if let (Some(name), Some(id)) = (
                    attribute(&element, "name"),
                    attribute_ending(&element, ":id"),
                ) {
                    entries.push((name, id));
                }
            }
            Event::Eof => return Ok(entries),
            _ => {}
        }
    }
}

/// The print settings of one worksheet part.
fn sheet_layout(sheet: &str) -> Result<SheetLayout, OfficeError> {
    let mut reader = Reader::from_str(sheet);
    let mut layout = SheetLayout::default();
    let mut reading: Option<bool> = None;
    loop {
        match reader.read_event().map_err(malformed)? {
            Event::Start(element) | Event::Empty(element) => match element.local_name().as_ref() {
                "sheetFormatPr" => {
                    layout.default_column_width = attribute(&element, "defaultColWidth")
                        .and_then(|width| character_width(&width));
                }
                "col" => apply_column(&mut layout, &element),
                "pageMargins" => layout.margins_mils = page_margins(&element),
                "pageSetup" => {
                    layout.landscape =
                        attribute(&element, "orientation").as_deref() == Some("landscape");
                }
                "printOptions" => {
                    layout.grid_lines = attribute(&element, "gridLines")
                        .is_some_and(|value| value == "1" || value == "true");
                }
                "oddHeader" => reading = Some(true),
                "oddFooter" => reading = Some(false),
                _ => {}
            },
            Event::Text(text) => {
                if let Some(header) = reading {
                    push_header_text(&mut layout, header, &text.xml10_content());
                }
            }
            Event::GeneralRef(reference) => {
                if let Some(header) = reading {
                    let resolved = match reference.resolve_char_ref() {
                        Ok(Some(character)) => Some(character.to_string()),
                        _ => quick_xml::escape::resolve_xml_entity(&reference).map(str::to_owned),
                    };
                    if let Some(resolved) = resolved {
                        push_header_text(&mut layout, header, &resolved);
                    }
                }
            }
            Event::End(element)
                if matches!(element.local_name().as_ref(), "oddHeader" | "oddFooter") =>
            {
                reading = None;
            }
            Event::Eof => return Ok(layout),
            _ => {}
        }
    }
}

fn push_header_text(layout: &mut SheetLayout, header: bool, text: &str) {
    let target = if header {
        &mut layout.header
    } else {
        &mut layout.footer
    };
    target.get_or_insert_with(String::new).push_str(text);
}

fn apply_column(layout: &mut SheetLayout, element: &BytesStart<'_>) {
    let number = |name: &str| {
        attribute(element, name)
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|value| (1..=MAX_WIDTH_COLUMNS).contains(value))
    };
    let (Some(min), Some(max)) = (number("min"), number("max")) else {
        return;
    };
    if max < min {
        return;
    }
    let hidden = attribute(element, "hidden").is_some_and(|value| value == "1" || value == "true");
    let width = if hidden {
        Some(0)
    } else {
        attribute(element, "width").and_then(|width| character_width(&width))
    };
    // Only the columns a sheet can show matter; a style for A:XFD would otherwise fill memory.
    let last = usize::try_from(max.min(min.saturating_add(255))).unwrap_or(0);
    if layout.column_widths.len() < last {
        layout.column_widths.resize(last, None);
    }
    for column in usize::try_from(min).unwrap_or(1)..=last {
        layout.column_widths[column - 1] = width;
    }
}

fn page_margins(element: &BytesStart<'_>) -> Option<[u32; 6]> {
    let mut margins = [0; 6];
    for (index, name) in ["top", "right", "bottom", "left", "header", "footer"]
        .into_iter()
        .enumerate()
    {
        let inches = attribute(element, name)?.parse::<f64>().ok()?;
        if !(0.0..=22.0).contains(&inches) {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let mils = (inches * 1000.0).round() as u32;
        margins[index] = mils;
    }
    Some(margins)
}

/// A width in characters as 256ths of a character, the unit Excel stores internally.
fn character_width(width: &str) -> Option<u32> {
    let width = width.parse::<f64>().ok()?;
    if !(0.0..=255.0).contains(&width) {
        return None;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some((width * 256.0).round() as u32)
}

/// The first font of the styles part, the one every cell without its own uses.
fn default_font(styles: &str) -> (Option<String>, Option<u32>) {
    let mut reader = Reader::from_str(styles);
    let mut in_fonts = false;
    let mut in_font = false;
    let (mut family, mut size) = (None, None);
    loop {
        match reader.read_event() {
            Ok(Event::Start(element) | Event::Empty(element)) => {
                match element.local_name().as_ref() {
                    "fonts" => in_fonts = true,
                    "font" if in_fonts => in_font = true,
                    "name" if in_font => family = attribute(&element, "val"),
                    "sz" if in_font => {
                        size = attribute(&element, "val")
                            .and_then(|value| value.parse::<f64>().ok())
                            .filter(|value| (1.0..=409.0).contains(value))
                            .map(|value| {
                                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                                let tenths = (value * 10.0).round() as u32;
                                tenths
                            });
                    }
                    _ => {}
                }
            }
            Ok(Event::End(element)) if element.local_name().as_ref() == "font" && in_font => {
                return (family, size);
            }
            Ok(Event::Eof) | Err(_) => return (family, size),
            _ => {}
        }
    }
}

fn attribute(element: &BytesStart<'_>, name: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.as_ref() == name)
        .and_then(|attribute| attribute_value(&attribute))
}

/// The value of the first attribute whose qualified name ends with `suffix` (`r:id` in any prefix).
fn attribute_ending(element: &BytesStart<'_>, suffix: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.as_ref().ends_with(suffix))
        .and_then(|attribute| attribute_value(&attribute))
}

fn malformed(error: impl std::fmt::Display) -> OfficeError {
    OfficeError::MalformedXml(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHEET: &str = concat!(
        r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#,
        r#"<sheetFormatPr defaultColWidth="11.5" defaultRowHeight="12.8"/>"#,
        r#"<cols><col min="1" max="1" width="9.92"/><col min="3" max="4" width="20"/>"#,
        r#"<col min="5" max="5" width="30" hidden="1"/><col min="0" max="2" width="3"/></cols>"#,
        r#"<sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>"#,
        r#"<printOptions gridLines="true"/>"#,
        r#"<pageMargins left="0.7875" right="0.7875" top="1.05" bottom="1" header="0.5" footer="0.4"/>"#,
        r#"<pageSetup paperSize="9" orientation="landscape"/>"#,
        r#"<headerFooter><oddHeader>&amp;C&amp;A</oddHeader><oddFooter>&amp;CPagina &amp;P</oddFooter></headerFooter>"#,
        "</worksheet>"
    );

    #[test]
    fn reads_the_print_settings_of_a_sheet() {
        let layout = sheet_layout(SHEET).unwrap();
        assert!(layout.landscape);
        assert!(layout.grid_lines);
        assert_eq!(
            layout.margins_mils,
            Some([1_050, 788, 1_000, 788, 500, 400])
        );
        assert_eq!(layout.default_column_width, Some(2_944));
        assert_eq!(
            layout.column_widths,
            vec![Some(2_540), None, Some(5_120), Some(5_120), Some(0)]
        );
        assert_eq!(layout.header.as_deref(), Some("&C&A"));
        assert_eq!(layout.footer.as_deref(), Some("&CPagina &P"));
    }

    #[test]
    fn keeps_the_defaults_for_what_a_sheet_does_not_say() {
        let layout = sheet_layout(
            "<worksheet><pageMargins left=\"x\"/><col min=\"2\" max=\"1\"/></worksheet>",
        )
        .unwrap();
        assert_eq!(layout, SheetLayout::default());
        assert!(sheet_layout("<worksheet><col").is_err());
    }

    #[test]
    fn caps_a_column_range_that_spans_the_whole_sheet() {
        let layout = sheet_layout(
            r#"<worksheet><cols><col min="1" max="16384" width="9"/></cols></worksheet>"#,
        )
        .unwrap();
        assert_eq!(layout.column_widths.len(), 256);
    }

    #[test]
    fn reads_the_default_font_and_the_sheet_entries() {
        let styles = r#"<styleSheet><fonts count="2"><font><sz val="10"/><name val="Arial"/></font><font><sz val="14"/><name val="Cambria"/></font></fonts></styleSheet>"#;
        assert_eq!(default_font(styles), (Some("Arial".to_owned()), Some(100)));
        assert_eq!(default_font("<styleSheet/>"), (None, None));
        let workbook = r#"<workbook xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Vendite" sheetId="1" r:id="rId3"/><sheet name="Senza id"/></sheets></workbook>"#;
        assert_eq!(
            sheet_entries(workbook).unwrap(),
            vec![("Vendite".to_owned(), "rId3".to_owned())]
        );
    }
}
