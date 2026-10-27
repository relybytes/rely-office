// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The sheets of a workbook printed as a spreadsheet prints them: the page, margins, column
//! widths, row heights, header, footer and grid lines of each sheet, the used range from A1 with
//! each cell's font, fill, borders and alignment and the merged cells, and the columns that do
//! not fit across a page continued on the next pages, all rows first.
//!
//! A cell wraps only when its format says so: other text longer than its column runs over into
//! the next one, as on screen.

use std::collections::HashMap;
use std::fmt::Write;

use rely_office_core::{
    Border, BorderStyle, CellRange, CellStyle, HorizontalAlignment, Sheet, SheetDrawing,
    SheetLayout, VerticalAlignment, Workbook,
};

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
const GRID_STROKE: &str = "0.4pt + luma(190)";
const EMU_PER_POINT: f64 = 12_700.0;

/// The source of a workbook and the image files of its drawings. With `with_images` false the
/// drawings are left out, for a workbook whose images Typst cannot decode.
pub(crate) fn workbook_source(
    workbook: &Workbook,
    with_images: bool,
) -> (String, Vec<(String, Vec<u8>)>) {
    let mut source = String::new();
    let mut files = Vec::new();
    for (index, sheet) in workbook.sheets.iter().enumerate() {
        if index > 0 {
            source.push_str("#pagebreak(weak: true)\n");
        }
        let text_width = write_sheet(&mut source, sheet);
        if with_images {
            write_drawings(&mut source, &mut files, index, sheet, text_width);
        }
    }
    (source, files)
}

/// The pictures and charts of a sheet, after its cells, each at its anchor's size and never
/// wider than the page.
fn write_drawings(
    source: &mut String,
    files: &mut Vec<(String, Vec<u8>)>,
    sheet_index: usize,
    sheet: &Sheet,
    text_width: f64,
) {
    let mut drawings: Vec<&SheetDrawing> = sheet.layout.drawings.iter().collect();
    drawings.sort_by_key(|drawing| (drawing.row, drawing.column));
    for (index, drawing) in drawings.into_iter().enumerate() {
        let Some(extension) = image_extension(drawing.content_type) else {
            continue;
        };
        #[expect(clippy::cast_precision_loss, reason = "drawing sizes fit in f64")]
        let (width, height) = (
            drawing.width_emu as f64 / EMU_PER_POINT,
            drawing.height_emu as f64 / EMU_PER_POINT,
        );
        let scale = (text_width / width).min(1.0);
        let path = format!("/media/sheets/{sheet_index}-{index}.{extension}");
        let _ = writeln!(
            source,
            "#block(above: 12pt, breakable: false, image({}, width: {}, height: {}, fit: \"stretch\"))",
            string_literal(&path),
            points(width * scale),
            points(height * scale)
        );
        files.push((path, drawing.bytes.clone()));
    }
}

fn image_extension(content_type: &str) -> Option<&'static str> {
    match content_type {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/svg+xml" => Some("svg"),
        _ => None,
    }
}

/// Write a sheet's page and its cells, and give back the width of the text area.
fn write_sheet(source: &mut String, sheet: &Sheet) -> f64 {
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
    // The lines of a wrapped cell sit as close as a spreadsheet sets them.
    source.push_str("#set par(leading: 0.1em)\n");

    write_tables(source, sheet, width - left - right, font_size);
    width - left - right
}

/// What a cell of a printed group of columns is: drawn alone, the first cell of the part of a
/// merged range the group shows, or covered by such a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placement {
    Single,
    /// Spans `columns` and `rows` and shows the content of the merge's own first cell.
    Span {
        columns: usize,
        rows: usize,
        origin: (usize, usize),
    },
    Covered,
}

/// The cells of a sheet as tables, one group of columns that fits the page after the other;
/// hidden rows are left out.
fn write_tables(source: &mut String, sheet: &Sheet, text_width: f64, font_size: f64) {
    let layout = &sheet.layout;
    let widths = column_widths(sheet);
    let inset_y = (font_size * (ROW_HEIGHT_PER_FONT_POINT - 1.15) / 2.0).max(0.0);
    let stroke = if layout.grid_lines {
        GRID_STROKE
    } else {
        "none"
    };
    let visible_rows: Vec<usize> = (0..sheet.rows.len())
        .filter(|row| {
            u32::try_from(*row)
                .ok()
                .and_then(|row| layout.row_heights.get(&row))
                != Some(&0)
        })
        .collect();
    let merges = merged_cells(layout, sheet.rows.len(), widths.len());
    for (chunk_index, columns) in column_chunks(&widths, text_width).into_iter().enumerate() {
        if chunk_index > 0 {
            source.push_str("#pagebreak()\n");
        }
        source.push_str("#table(columns: (");
        for &column in &columns {
            let _ = write!(source, "{}, ", points(widths[column]));
        }
        source.push_str("), rows: (");
        for &row in &visible_rows {
            let height = u32::try_from(row)
                .ok()
                .and_then(|row| layout.row_heights.get(&row))
                .map_or_else(
                    || "auto".to_owned(),
                    |height| points(f64::from(*height) / 20.0),
                );
            let _ = write!(source, "{height}, ");
        }
        let _ = write!(
            source,
            "), stroke: {stroke}, align: bottom, inset: (x: {}, y: {}),",
            points(CELL_INSET_X_POINTS),
            points(inset_y)
        );
        for (row_position, &row) in visible_rows.iter().enumerate() {
            for (column_position, &column) in columns.iter().enumerate() {
                let placement = placement(
                    &merges,
                    (row, column),
                    &visible_rows[row_position..],
                    &columns[column_position..],
                    &visible_rows[..row_position],
                    &columns[..column_position],
                );
                let (origin, spans) = match placement {
                    Placement::Covered => continue,
                    Placement::Single => ((row, column), None),
                    Placement::Span {
                        columns,
                        rows,
                        origin,
                    } => (origin, Some((columns, rows))),
                };
                let value = sheet
                    .rows
                    .get(origin.0)
                    .and_then(|cells| cells.get(origin.1))
                    .map_or("", String::as_str);
                let style = u32::try_from(origin.0)
                    .ok()
                    .zip(u32::try_from(origin.1).ok())
                    .and_then(|(row, column)| layout.style_of(row, column));
                let colour = sheet.value_colours.get(&origin).copied();
                source.push(' ');
                source.push_str(&cell_markup(value, style, colour, spans));
                source.push(',');
            }
        }
        source.push_str(")\n");
    }
}

/// The merged range each shown cell belongs to, by row and column.
fn merged_cells(
    layout: &SheetLayout,
    rows: usize,
    columns: usize,
) -> HashMap<(usize, usize), CellRange> {
    let mut cells = HashMap::new();
    for merge in &layout.merges {
        let (Ok(first_row), Ok(first_column)) = (
            usize::try_from(merge.first_row),
            usize::try_from(merge.first_column),
        ) else {
            continue;
        };
        let last_row = usize::try_from(merge.last_row)
            .unwrap_or(usize::MAX)
            .min(rows.saturating_sub(1));
        let last_column = usize::try_from(merge.last_column)
            .unwrap_or(usize::MAX)
            .min(columns.saturating_sub(1));
        if first_row >= rows || first_column >= columns {
            continue;
        }
        for row in first_row..=last_row {
            for column in first_column..=last_column {
                cells.entry((row, column)).or_insert(*merge);
            }
        }
    }
    cells
}

/// How a cell is drawn in a group of columns: a merged range starts at its first shown row and
/// column there, which shows the merge's content when the merge's own first cell is shown.
fn placement(
    merges: &HashMap<(usize, usize), CellRange>,
    cell: (usize, usize),
    rows_after: &[usize],
    columns_after: &[usize],
    rows_before: &[usize],
    columns_before: &[usize],
) -> Placement {
    let Some(merge) = merges.get(&cell) else {
        return Placement::Single;
    };
    let inside_rows = |row: &usize| {
        u32::try_from(*row).is_ok_and(|row| (merge.first_row..=merge.last_row).contains(&row))
    };
    let inside_columns = |column: &usize| {
        u32::try_from(*column)
            .is_ok_and(|column| (merge.first_column..=merge.last_column).contains(&column))
    };
    if rows_before.iter().any(inside_rows) || columns_before.iter().any(inside_columns) {
        return Placement::Covered;
    }
    let rows = rows_after.iter().take_while(|row| inside_rows(row)).count();
    let columns = columns_after
        .iter()
        .take_while(|column| inside_columns(column))
        .count();
    let origin = (
        usize::try_from(merge.first_row).unwrap_or(0),
        usize::try_from(merge.first_column).unwrap_or(0),
    );
    let origin_shown = origin == cell;
    if rows <= 1 && columns <= 1 && origin_shown {
        return Placement::Single;
    }
    Placement::Span {
        columns: columns.max(1),
        rows: rows.max(1),
        origin: if origin_shown {
            origin
        } else {
            (usize::MAX, usize::MAX)
        },
    }
}

/// One cell as table content: its text with the font of its format, its fill, borders and
/// alignment; a number aligns right unless the format says otherwise.
fn cell_markup(
    value: &str,
    style: Option<&CellStyle>,
    value_colour: Option<&str>,
    spans: Option<(usize, usize)>,
) -> String {
    let mut arguments = Vec::new();
    if let Some((columns, rows)) = spans {
        if columns > 1 {
            arguments.push(format!("colspan: {columns}"));
        }
        if rows > 1 {
            arguments.push(format!("rowspan: {rows}"));
        }
    }
    let horizontal = style.and_then(|style| style.horizontal).or_else(|| {
        (!value.is_empty() && looks_numeric(value)).then_some(HorizontalAlignment::Right)
    });
    let vertical = style.and_then(|style| style.vertical);
    let horizontal = horizontal.map(|alignment| match alignment {
        HorizontalAlignment::Left | HorizontalAlignment::Justify => "left",
        HorizontalAlignment::Center => "center",
        HorizontalAlignment::Right => "right",
    });
    let vertical = vertical.map(|alignment| match alignment {
        VerticalAlignment::Top => "top",
        VerticalAlignment::Center => "horizon",
        VerticalAlignment::Bottom => "bottom",
    });
    match (horizontal, vertical) {
        (Some(horizontal), Some(vertical)) => {
            arguments.push(format!("align: {horizontal} + {vertical}"));
        }
        (Some(horizontal), None) => arguments.push(format!("align: {horizontal} + bottom")),
        (None, Some(vertical)) => arguments.push(format!("align: {vertical}")),
        (None, None) => {}
    }
    if let Some(style) = style {
        if let Some(fill) = &style.fill {
            arguments.push(format!("fill: rgb(\"#{fill}\")"));
        }
        if !style.borders.is_empty() {
            let mut stroke = String::from("stroke: (");
            for (side, edge) in [
                ("top", &style.borders.top),
                ("right", &style.borders.right),
                ("bottom", &style.borders.bottom),
                ("left", &style.borders.left),
            ] {
                if let Some(edge) = edge {
                    let _ = write!(stroke, "{side}: {}, ", border_stroke(edge));
                }
            }
            stroke.push(')');
            arguments.push(stroke);
        }
    }
    let content = if value.is_empty() {
        // An empty row keeps the height of a line, as a sheet keeps it.
        "hide[0]".to_owned()
    } else {
        let mut text = string_literal(value);
        if let Some(style) = style {
            let font = &style.font;
            let mut text_arguments = Vec::new();
            if let Some(family) = &font.family {
                text_arguments.push(format!("font: {}", font_list(&body_fonts(Some(family)))));
            }
            if let Some(size) = font.size_tenths {
                text_arguments.push(format!("size: {}", points(f64::from(size) / 10.0)));
            }
            if font.bold {
                text_arguments.push("weight: \"bold\"".to_owned());
            }
            if font.italic {
                text_arguments.push("style: \"italic\"".to_owned());
            }
            // The colour of a number format outdoes the font's, as in a spreadsheet.
            if let Some(colour) = value_colour.or(font.colour.as_deref()) {
                text_arguments.push(format!("fill: rgb(\"#{colour}\")"));
            }
            if !text_arguments.is_empty() {
                text = format!("text({}, {text})", text_arguments.join(", "));
            }
            if font.underline {
                text = format!("underline({text})");
            }
            if font.strike {
                text = format!("strike({text})");
            }
        }
        if style.is_none()
            && let Some(colour) = value_colour
        {
            text = format!("text(fill: rgb(\"#{colour}\"), {text})");
        }
        if style.is_some_and(|style| style.wrap) {
            text
        } else {
            format!("box({text})")
        }
    };
    if arguments.is_empty() {
        content
    } else {
        format!("table.cell({}, {content})", arguments.join(", "))
    }
}

/// A Typst stroke for a cell border: hair and thin lines as Excel prints them, thicker ones by
/// their width.
fn border_stroke(border: &Border) -> String {
    if border.style == BorderStyle::None {
        return "none".to_owned();
    }
    let paint = border.colour.as_ref().map_or_else(
        || "black".to_owned(),
        |colour| format!("rgb(\"#{colour}\")"),
    );
    let width = f64::from(border.width_eighths) / 8.0
        * if border.style == BorderStyle::Double {
            2.0
        } else {
            1.0
        };
    let dash = match border.style {
        BorderStyle::Dotted => ", dash: \"dotted\"",
        BorderStyle::Dashed => ", dash: \"dashed\"",
        _ => "",
    };
    format!("(paint: {paint}, thickness: {}{dash})", points(width))
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

/// Whether a displayed value reads as a number or a date, which a sheet aligns right: digits
/// with their separators, a sign, a currency or a percent.
fn looks_numeric(value: &str) -> bool {
    let trimmed = value
        .trim()
        .trim_matches(['%', '€', '$', '£', '-', '+', '(', ')', ' ', '\u{a0}']);
    !trimmed.is_empty()
        && trimmed.chars().any(|character| character.is_ascii_digit())
        && trimmed.chars().all(|character| {
            character.is_ascii_digit()
                || matches!(
                    character,
                    '.' | ',' | ' ' | '/' | ':' | 'E' | '+' | '-' | '\u{a0}'
                )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rely_office_core::DocumentFormat;

    fn sheet(name: &str, rows: &[&[&str]]) -> Sheet {
        Sheet {
            value_colours: std::collections::HashMap::new(),
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
    fn places_the_drawings_of_a_sheet_after_its_cells_within_the_page() {
        let drawing = |row: u32, width_emu: i64, content_type: &'static str| SheetDrawing {
            row,
            column: 0,
            width_emu,
            height_emu: 1_270_000,
            content_type,
            bytes: vec![u8::try_from(row).unwrap()],
            description: None,
        };
        let mut drawn = sheet("Vendite", &[&["a"]]);
        drawn.layout.drawings = vec![
            drawing(9, 12_700_000, "image/svg+xml"),
            drawing(2, 127_000_000, "image/png"),
            drawing(5, 12_700, "image/x-emf"),
        ];
        let workbook = Workbook {
            format: DocumentFormat::Xlsx,
            sheets: vec![drawn],
        };
        let (source, files) = workbook_source(&workbook, true);
        assert_eq!(
            files,
            vec![
                ("/media/sheets/0-0.png".to_owned(), vec![2]),
                ("/media/sheets/0-2.svg".to_owned(), vec![9]),
            ],
            "in the order of their anchors, an image Typst cannot read left out"
        );
        let first = source.find("0-0.png").unwrap();
        let second = source.find("0-2.svg").unwrap();
        assert!(source.find("\"a\"").unwrap() < first && first < second);
        assert!(
            source.contains("0-2.svg\", width: 494.48pt, height: 49.45pt"),
            "never wider than the page: {source}"
        );
        let (without, files) = workbook_source(&workbook, false);
        assert_eq!(files, Vec::<(String, Vec<u8>)>::new());
        assert!(!without.contains("/media/"));
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
        let source = workbook_source(&workbook, true).0;
        assert!(source.starts_with(
            "#set page(width: 595.28pt, height: 841.89pt, margin: (top: 54pt, right: 50.4pt, bottom: 54pt, left: 50.4pt), header: none, footer: none)\n"
        ));
        assert!(
            source.contains(r#"font: ("Calibri", "Carlito", "Liberation Sans", ), size: 11pt"#)
        );
        assert!(source.contains(
            "#table(columns: (48pt, 48pt, ), rows: (auto, auto, ), stroke: none, align: bottom,"
        ));
        assert!(source.contains(
            r#" box("Rossi \"srl\""), table.cell(align: right + bottom, box("1.234,50")),"#
        ));
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
            ..SheetLayout::default()
        };
        let (source, _) = workbook_source(
            &Workbook {
                format: DocumentFormat::Xlsx,
                sheets: vec![printed],
            },
            true,
        );
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
        assert!(source.contains(
            "#table(columns: (105pt, 60pt, ), rows: (auto, ), stroke: 0.4pt + luma(190),"
        ));
        assert!(!source.contains(r#""b""#), "a hidden column is not printed");
    }

    #[test]
    fn prints_cell_formats_row_heights_and_merged_cells() {
        use rely_office_core::{CellBorders, CellFont};
        let mut printed = sheet(
            "Conti",
            &[
                &["Titolo", "", "", "x"],
                &["nascosta", "", "", ""],
                &["a", "b", "c", "d"],
                &["e", "f", "g", "h"],
            ],
        );
        let border = Border {
            style: BorderStyle::Double,
            width_eighths: 4,
            colour: Some("FF0000".to_owned()),
        };
        printed.layout.styles = vec![
            CellStyle::default(),
            CellStyle {
                font: CellFont {
                    family: Some("Arial".to_owned()),
                    size_tenths: Some(140),
                    colour: Some("FFFFFF".to_owned()),
                    bold: true,
                    italic: true,
                    underline: true,
                    strike: true,
                },
                fill: Some("4472C4".to_owned()),
                borders: CellBorders {
                    bottom: Some(border),
                    left: Some(Border {
                        style: BorderStyle::Dotted,
                        width_eighths: 2,
                        colour: None,
                    }),
                    top: Some(Border {
                        style: BorderStyle::None,
                        width_eighths: 4,
                        colour: None,
                    }),
                    right: None,
                },
                horizontal: Some(HorizontalAlignment::Center),
                vertical: Some(VerticalAlignment::Center),
                wrap: true,
                number_format: None,
            },
            CellStyle {
                vertical: Some(VerticalAlignment::Top),
                ..CellStyle::default()
            },
        ];
        printed.layout.cell_styles.insert((0, 0), 1);
        printed.layout.cell_styles.insert((2, 3), 2);
        printed.layout.row_heights.insert(0, 600);
        printed.layout.row_heights.insert(1, 0);
        printed.layout.merges = vec![
            CellRange::parse("A1:C1").unwrap(),
            CellRange::parse("B3:C4").unwrap(),
            CellRange::parse("Z99:Z100").unwrap(),
        ];
        let (source, _) = workbook_source(
            &Workbook {
                format: DocumentFormat::Xlsx,
                sheets: vec![printed],
            },
            true,
        );
        assert!(source.contains("rows: (30pt, auto, auto, ),"), "{source}");
        assert!(!source.contains("nascosta"), "a hidden row is left out");
        assert!(source.contains(concat!(
            r##"table.cell(colspan: 3, align: center + horizon, fill: rgb("#4472C4"), "##,
            r##"stroke: (top: none, bottom: (paint: rgb("#FF0000"), thickness: 1pt), "##,
            r##"left: (paint: black, thickness: 0.25pt, dash: "dotted"), ), "##,
            r##"strike(underline(text(font: ("Arial", "Liberation Sans", "Carlito", ), size: 14pt, "##,
            r##"weight: "bold", style: "italic", fill: rgb("#FFFFFF"), "Titolo"))))"##
        )));
        assert!(source.contains(r#"table.cell(colspan: 2, rowspan: 2, box("b"))"#));
        assert!(!source.contains(r#""f""#), "a covered cell is not drawn");
        assert!(source.contains(r#"table.cell(align: top, box("d"))"#));

        let mut negative = sheet("Saldo", &[&["-5"]]);
        negative.value_colours.insert((0, 0), "FF0000");
        let (source, _) = workbook_source(
            &Workbook {
                format: DocumentFormat::Xlsx,
                sheets: vec![negative],
            },
            true,
        );
        assert!(
            source.contains(r##"box(text(fill: rgb("#FF0000"), "-5"))"##),
            "{source}"
        );
    }

    #[test]
    fn draws_the_part_of_a_merge_a_group_of_columns_shows() {
        let mut layout = SheetLayout::default();
        layout.merges.push(CellRange::parse("A1:B2").unwrap());
        let merges = merged_cells(&layout, 3, 3);
        assert_eq!(merges.len(), 4);
        assert_eq!(
            placement(&merges, (0, 0), &[0, 1, 2], &[0, 1, 2], &[], &[]),
            Placement::Span {
                columns: 2,
                rows: 2,
                origin: (0, 0)
            }
        );
        assert_eq!(
            placement(&merges, (0, 1), &[0, 1, 2], &[1, 2], &[], &[0]),
            Placement::Covered
        );
        assert_eq!(
            placement(&merges, (0, 1), &[0, 1, 2], &[1, 2], &[], &[]),
            Placement::Span {
                columns: 1,
                rows: 2,
                origin: (usize::MAX, usize::MAX)
            },
            "a group that starts inside the merge draws its part empty"
        );
        assert_eq!(
            placement(&merges, (1, 0), &[1, 2], &[0, 1, 2], &[0], &[]),
            Placement::Covered
        );
        assert_eq!(
            placement(&merges, (2, 2), &[2], &[2], &[0, 1], &[0, 1]),
            Placement::Single
        );
        layout.merges = vec![CellRange::parse("A1").unwrap()];
        let single = merged_cells(&layout, 3, 3);
        assert_eq!(
            placement(&single, (0, 0), &[0, 1], &[0, 1], &[], &[]),
            Placement::Single
        );
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
        assert!(looks_numeric("1.200,50 €"));
        assert!(looks_numeric("(1.200)"));
        assert!(looks_numeric("27/10/2026"));
        assert!(looks_numeric("12:30"));
        assert!(looks_numeric("1,23E+04"));
        assert!(!looks_numeric("Rif. 12"));
        for number in [
            "12",
            "1.234,50",
            "-3,5",
            "€ 10",
            "45%",
            "1 000",
            "2026-10-26",
        ] {
            assert!(looks_numeric(number), "{number}");
        }
        for text in ["", "abc", "12a", "-", "€"] {
            assert!(!looks_numeric(text), "{text}");
        }
    }
}
