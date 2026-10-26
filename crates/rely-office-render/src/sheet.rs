// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The sheets of a workbook printed as a spreadsheet prints them: the page, margins, column
//! widths, header, footer and grid lines of each sheet, the used range from A1, and the columns
//! that do not fit across a page continued on the next pages, all rows first.
//!
//! A cell never wraps: text longer than its column runs over into the next one, as on screen.

use std::fmt::Write;

use rely_office_core::{Sheet, SheetLayout, Workbook};

use crate::fonts::body_fonts;
use crate::markup::{font_list, points, string_literal};

const A4_POINTS: (f64, f64) = (595.28, 841.89);
/// Excel's margins for a sheet that names none, in inches: top, right, bottom, left, header,
/// footer.
const DEFAULT_MARGINS_INCHES: [f64; 6] = [0.75, 0.7, 0.75, 0.7, 0.3, 0.3];
/// Excel's default column of 8.43 characters as a file stores it, padding included, in 256ths.
const DEFAULT_COLUMN_WIDTH: u32 = 2_340;
/// The widest digit of the default fonts at 96 dpi, in pixels, the unit of a column width.
const MAX_DIGIT_PIXELS: f64 = 7.0;
const POINTS_PER_PIXEL: f64 = 0.75;
const DEFAULT_FONT_POINTS: f64 = 11.0;
/// A row of the default height is this much taller than its font.
const ROW_HEIGHT_PER_FONT_POINT: f64 = 1.28;
const CELL_INSET_X_POINTS: f64 = 2.0;

pub(crate) fn workbook_source(workbook: &Workbook) -> String {
    let mut source = String::new();
    for (index, sheet) in workbook.sheets.iter().enumerate() {
        if index > 0 {
            source.push_str("#pagebreak(weak: true)\n");
        }
        write_sheet(&mut source, sheet);
    }
    source
}

fn write_sheet(source: &mut String, sheet: &Sheet) {
    let layout = &sheet.layout;
    let (width, height) = if layout.landscape {
        (A4_POINTS.1, A4_POINTS.0)
    } else {
        A4_POINTS
    };
    let [top, right, bottom, left, header, footer] = layout.margins_mils.map_or(
        DEFAULT_MARGINS_INCHES.map(|inches| inches * 72.0),
        |margins| margins.map(|mils| f64::from(mils) * 72.0 / 1_000.0),
    );
    let font_size = layout
        .font_size_tenths
        .map_or(DEFAULT_FONT_POINTS, |size| f64::from(size) / 10.0);
    let fonts = font_list(&body_fonts(
        layout.font_family.as_deref().or(Some("Calibri")),
    ));

    let _ = write!(
        source,
        "#set page(width: {}, height: {}, margin: (top: {}, right: {}, bottom: {}, left: {})",
        points(width),
        points(height),
        points(top),
        points(right),
        points(bottom),
        points(left)
    );
    match layout
        .header
        .as_deref()
        .map(|codes| header_footer(codes, &sheet.name))
    {
        Some(Some(content)) => {
            let _ = write!(
                source,
                ", header: {content}, header-ascent: {}",
                points((top - header - font_size * 1.2).max(0.0))
            );
        }
        _ => source.push_str(", header: none"),
    }
    match layout
        .footer
        .as_deref()
        .map(|codes| header_footer(codes, &sheet.name))
    {
        Some(Some(content)) => {
            let _ = write!(
                source,
                ", footer: {content}, footer-descent: {}",
                points((bottom - footer - font_size * 1.2).max(0.0))
            );
        }
        _ => source.push_str(", footer: none"),
    }
    source.push_str(")\n");
    let _ = writeln!(
        source,
        "#set text(font: {fonts}, size: {}, lang: \"it\", hyphenate: false, top-edge: \"ascender\", bottom-edge: \"descender\")",
        points(font_size)
    );

    write_tables(source, sheet, width - left - right, font_size);
}

/// The cells of a sheet as tables, one group of columns that fits the page after the other.
fn write_tables(source: &mut String, sheet: &Sheet, text_width: f64, font_size: f64) {
    let widths = column_widths(sheet);
    let inset_y = (font_size * (ROW_HEIGHT_PER_FONT_POINT - 1.15) / 2.0).max(0.0);
    let stroke = if sheet.layout.grid_lines {
        "0.4pt + luma(190)"
    } else {
        "none"
    };
    for (chunk_index, columns) in column_chunks(&widths, text_width).into_iter().enumerate() {
        if chunk_index > 0 {
            source.push_str("#pagebreak()\n");
        }
        source.push_str("#table(columns: (");
        for &column in &columns {
            let _ = write!(source, "{}, ", points(widths[column]));
        }
        let _ = write!(
            source,
            "), stroke: {stroke}, inset: (x: {}, y: {}),",
            points(CELL_INSET_X_POINTS),
            points(inset_y)
        );
        for row in &sheet.rows {
            for &column in &columns {
                let value = &row[column];
                if value.is_empty() {
                    source.push_str(" [],");
                } else if looks_numeric(value) {
                    let _ = write!(
                        source,
                        " table.cell(align: right, box({})),",
                        string_literal(value)
                    );
                } else {
                    let _ = write!(source, " box({}),", string_literal(value));
                }
            }
        }
        source.push_str(")\n");
    }
}

/// The width of each column in points, from the sheet's layout; a hidden column is 0.
fn column_widths(sheet: &Sheet) -> Vec<f64> {
    let layout: &SheetLayout = &sheet.layout;
    let columns = sheet.rows.first().map_or(0, Vec::len);
    let default = layout.default_column_width.unwrap_or(DEFAULT_COLUMN_WIDTH);
    (0..columns)
        .map(|column| {
            let width = layout
                .column_widths
                .get(column)
                .copied()
                .flatten()
                .unwrap_or(default);
            column_points(width)
        })
        .collect()
}

/// A column width in 256ths of a character as Excel turns it into pixels, then points.
fn column_points(width: u32) -> f64 {
    if width == 0 {
        return 0.0;
    }
    let padding = (128.0 / MAX_DIGIT_PIXELS).trunc();
    let pixels = ((f64::from(width) + padding) / 256.0 * MAX_DIGIT_PIXELS).trunc();
    pixels * POINTS_PER_PIXEL
}

/// The visible columns grouped so each group fits the text width; a column is never split.
fn column_chunks(widths: &[f64], text_width: f64) -> Vec<Vec<usize>> {
    let mut chunks = Vec::new();
    let mut chunk = Vec::new();
    let mut used = 0.0;
    for (column, width) in widths.iter().enumerate() {
        if *width <= 0.0 {
            continue;
        }
        if !chunk.is_empty() && used + width > text_width {
            chunks.push(std::mem::take(&mut chunk));
            used = 0.0;
        }
        chunk.push(column);
        used += width;
    }
    if !chunk.is_empty() {
        chunks.push(chunk);
    }
    chunks
}

/// A header or footer coded as Excel codes them, as Typst content: `&L`, `&C` and `&R` open the
/// three sections; `&A` is the sheet name, `&P` the page, `&N` the pages, `&&` an ampersand;
/// `&"font,style"` and `&` with a size set the font of the rest of the section; every other code
/// is dropped. `None` when every section is empty.
fn header_footer(codes: &str, sheet_name: &str) -> Option<String> {
    let (sections, fonts) = header_sections(codes, sheet_name);
    if sections.iter().all(Vec::is_empty) {
        return None;
    }
    let cells: Vec<String> = sections
        .iter()
        .zip(&fonts)
        .zip(["left", "center", "right"])
        .map(|((parts, (family, size)), alignment)| {
            let mut joined = String::from("[");
            for part in parts {
                joined.push('#');
                joined.push_str(part);
            }
            joined.push(']');
            let mut arguments = Vec::new();
            if let Some(family) = family {
                arguments.push(format!("font: {}", font_list(&body_fonts(Some(family)))));
            }
            if let Some(size) = size {
                arguments.push(format!("size: {}", points(*size)));
            }
            let content = if arguments.is_empty() {
                joined
            } else {
                format!("text({}, {joined})", arguments.join(", "))
            };
            format!("align({alignment}, {content})")
        })
        .collect();
    Some(format!(
        "context grid(columns: (1fr, 1fr, 1fr), {})",
        cells.join(", ")
    ))
}

/// The font a section of a header or footer sets: family and size in points.
type SectionFont = (Option<String>, Option<f64>);

/// The three sections of a header or footer: the Typst expressions of each, and its font.
fn header_sections(codes: &str, sheet_name: &str) -> ([Vec<String>; 3], [SectionFont; 3]) {
    let mut sections: [Vec<String>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let mut fonts: [SectionFont; 3] = Default::default();
    let mut current = 1;
    let mut text = String::new();
    let mut characters = codes.chars().peekable();
    let flush = |text: &mut String, section: &mut Vec<String>| {
        if !text.is_empty() {
            section.push(string_literal(&std::mem::take(text)));
        }
    };
    while let Some(character) = characters.next() {
        if character != '&' {
            text.push(character);
            continue;
        }
        let Some(code) = characters.next() else {
            break;
        };
        match code {
            '&' => text.push('&'),
            'L' | 'C' | 'R' => {
                flush(&mut text, &mut sections[current]);
                current = match code {
                    'L' => 0,
                    'C' => 1,
                    _ => 2,
                };
            }
            'A' => text.push_str(sheet_name),
            'P' | 'N' => {
                flush(&mut text, &mut sections[current]);
                sections[current].push(
                    if code == 'P' {
                        "counter(page).display()"
                    } else {
                        "counter(page).final().first()"
                    }
                    .to_owned(),
                );
            }
            '"' => {
                let mut font = String::new();
                for next in characters.by_ref() {
                    if next == '"' {
                        break;
                    }
                    font.push(next);
                }
                let family = font.split(',').next().unwrap_or_default().trim().to_owned();
                if !family.is_empty() && family != "-" {
                    fonts[current].0 = Some(family);
                }
            }
            digit if digit.is_ascii_digit() => {
                let mut size = digit.to_string();
                while let Some(next) = characters.peek().copied().filter(char::is_ascii_digit) {
                    size.push(next);
                    characters.next();
                }
                fonts[current].1 = size
                    .parse::<f64>()
                    .ok()
                    .filter(|size| (1.0..=400.0).contains(size));
            }
            _ => {}
        }
    }
    flush(&mut text, &mut sections[current]);
    (sections, fonts)
}

/// Whether a displayed value reads as a number, which a sheet aligns right.
fn looks_numeric(value: &str) -> bool {
    let trimmed = value
        .trim()
        .trim_end_matches('%')
        .trim_start_matches(['€', '$', '-', '+']);
    let trimmed = trimmed.trim();
    !trimmed.is_empty()
        && trimmed.chars().any(|character| character.is_ascii_digit())
        && trimmed
            .chars()
            .all(|character| character.is_ascii_digit() || matches!(character, '.' | ',' | ' '))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rely_office_core::DocumentFormat;

    fn sheet(name: &str, rows: &[&[&str]]) -> Sheet {
        Sheet {
            name: name.to_owned(),
            rows: rows
                .iter()
                .map(|row| row.iter().map(|value| (*value).to_owned()).collect())
                .collect(),
            total_rows: rows.len(),
            total_columns: rows.first().map_or(0, |row| row.len()),
            layout: SheetLayout::default(),
        }
    }

    #[test]
    fn prints_a_sheet_with_the_defaults_of_excel() {
        let workbook = Workbook {
            format: DocumentFormat::Csv,
            sheets: vec![
                sheet(
                    "",
                    &[&["Cliente", "Importo"], &["Rossi \"srl\"", "1.234,50"]],
                ),
                sheet("Vuoto", &[]),
            ],
        };
        let source = workbook_source(&workbook);
        assert!(source.starts_with(
            "#set page(width: 595.28pt, height: 841.89pt, margin: (top: 54pt, right: 50.4pt, bottom: 54pt, left: 50.4pt), header: none, footer: none)\n"
        ));
        assert!(
            source.contains(r#"font: ("Calibri", "Carlito", "Liberation Sans", ), size: 11pt"#)
        );
        assert!(source.contains("#table(columns: (48pt, 48pt, ), stroke: none,"));
        assert!(
            source.contains(r#" box("Rossi \"srl\""), table.cell(align: right, box("1.234,50")),"#)
        );
        assert!(source.contains("#pagebreak(weak: true)\n#set page("));
        assert_eq!(source.matches("#table(").count(), 1);
    }

    #[test]
    fn follows_the_layout_of_the_sheet() {
        let mut printed = sheet("Vendite", &[&["a", "b", "c"]]);
        printed.layout = SheetLayout {
            landscape: true,
            margins_mils: Some([1_000, 500, 1_000, 500, 500, 500]),
            column_widths: vec![Some(5_120), Some(0)],
            default_column_width: Some(2_944),
            header: Some("&C&A".to_owned()),
            footer: Some("&L&\"Times New Roman,Normale\"&12Pagina &P di &N&R&&".to_owned()),
            grid_lines: true,
            font_family: Some("Arial".to_owned()),
            font_size_tenths: Some(100),
        };
        let source = workbook_source(&Workbook {
            format: DocumentFormat::Xlsx,
            sheets: vec![printed],
        });
        assert!(source.starts_with(
            "#set page(width: 841.89pt, height: 595.28pt, margin: (top: 72pt, right: 36pt, bottom: 72pt, left: 36pt), header: context grid("
        ));
        assert!(source.contains(r#"align(center, [#"Vendite"])"#));
        assert!(source.contains(
            r##"align(left, text(font: ("Times New Roman", "Liberation Serif", "Carlito", "Liberation Sans", ), size: 12pt, [#"Pagina "#counter(page).display()#" di "#counter(page).final().first()]))"##
        ));
        assert!(source.contains(r#"align(right, [#"&"])"#));
        assert!(source.contains("header-ascent: 24pt"));
        assert!(source.contains(r#"font: ("Arial", "Liberation Sans", "Carlito", ), size: 10pt"#));
        assert!(source.contains("#table(columns: (105pt, 60pt, ), stroke: 0.4pt + luma(190),"));
        assert!(!source.contains(r#""b""#), "a hidden column is not printed");
    }

    #[test]
    fn splits_columns_that_do_not_fit_across_pages() {
        let widths = vec![200.0; 7];
        assert_eq!(
            column_chunks(&widths, 500.0),
            vec![vec![0, 1], vec![2, 3], vec![4, 5], vec![6]]
        );
        assert_eq!(column_chunks(&[], 500.0), Vec::<Vec<usize>>::new());
        assert_eq!(
            column_chunks(&[2_000.0, 0.0, 10.0], 500.0),
            vec![vec![0], vec![2]]
        );
    }

    #[test]
    fn turns_excel_widths_into_points() {
        assert!((column_points(DEFAULT_COLUMN_WIDTH) - 48.0).abs() < f64::EPSILON);
        assert!((column_points(2_540) - 51.75).abs() < f64::EPSILON);
        assert!(column_points(0).abs() < f64::EPSILON);
    }

    #[test]
    fn reads_header_codes() {
        assert_eq!(header_footer("", "x"), None);
        assert_eq!(header_footer("&C&B&I", "x"), None);
        assert_eq!(
            header_footer("solo", "x").unwrap(),
            r#"context grid(columns: (1fr, 1fr, 1fr), align(left, []), align(center, [#"solo"]), align(right, []))"#
        );
        assert!(
            header_footer("&R&\"-,Bold\"&A&", "Fogli")
                .unwrap()
                .contains(r#"align(right, [#"Fogli"])"#)
        );
    }

    #[test]
    fn tells_numbers_from_text() {
        for number in ["12", "1.234,50", "-3,5", "€ 10", "45%", "1 000"] {
            assert!(looks_numeric(number), "{number}");
        }
        for text in ["", "abc", "12a", "-", "2026-10-26", "€"] {
            assert!(!looks_numeric(text), "{text}");
        }
    }
}
