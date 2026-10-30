// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! The sheets of a workbook printed as a spreadsheet prints them: the page, margins, column
//! widths, row heights, header, footer and grid lines of each sheet, the used range from A1 with
//! each cell's font, fill, borders and alignment and the merged cells, and the columns that do
//! not fit across a page continued on the next pages, all rows first.
//!
//! A cell wraps only when its format says so: other text longer than its column runs over into
//! the next one, as on screen.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use rely_office_core::{
    Border, BorderStyle, CellRange, CellStyle, HorizontalAlignment, Limits, Sheet, SheetDrawing,
    SheetLayout, VerticalAlignment, Workbook,
};
use rely_office_sheet::{CellLook, IconLook, Locale};

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
/// The table reaches as far as a drawing's anchor cell, but never past this row or column.
const MAX_DRAWING_REACH: usize = 10_000;

/// The look the conditional formats give the cells of a sheet, by row and column.
pub(crate) type SheetLooks = HashMap<(usize, usize), CellLook>;

/// The looks of the cells of each sheet of a workbook, computed by the editable model from the
/// same bytes; none when no sheet has a conditional format or the model cannot read the file.
pub(crate) fn conditional_looks(
    bytes: &[u8],
    file_name: Option<&str>,
    limits: &Limits,
    workbook: &Workbook,
) -> Vec<SheetLooks> {
    if workbook
        .sheets
        .iter()
        .all(|sheet| sheet.layout.conditional_formats.is_empty())
    {
        return Vec::new();
    }
    let Ok(model) = rely_office_sheet::read_spreadsheet(bytes, file_name, limits, Locale::Italian)
    else {
        return Vec::new();
    };
    workbook
        .sheets
        .iter()
        .map(|sheet| {
            model
                .sheet_index(&sheet.name)
                .map(|index| {
                    model
                        .conditional_looks(index)
                        .into_iter()
                        .filter_map(|(cell, look)| {
                            Some((
                                (
                                    usize::try_from(cell.row).ok()?,
                                    usize::try_from(cell.column).ok()?,
                                ),
                                look,
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect()
}

/// The source of a workbook and the image files of its drawings, each sheet's cells with the
/// looks of `looks` at its index. With `with_images` false the drawings are left out, for a
/// workbook whose images Typst cannot decode.
pub(crate) fn workbook_source(
    workbook: &Workbook,
    looks: &[SheetLooks],
    with_images: bool,
) -> (String, Vec<(String, Vec<u8>)>) {
    let no_looks = SheetLooks::new();
    let mut source = String::new();
    let mut files = Vec::new();
    for (index, sheet) in workbook.sheets.iter().enumerate() {
        if index > 0 {
            source.push_str("#pagebreak(weak: true)\n");
        }
        let drawings = if with_images {
            placed_drawings(&mut files, index, sheet)
        } else {
            Vec::new()
        };
        let sheet_looks = looks.get(index).unwrap_or(&no_looks);
        let (text_width, anchored) = write_sheet(&mut source, sheet, sheet_looks, &drawings);
        write_loose_drawings(&mut source, &drawings, &anchored, text_width);
    }
    (source, files)
}

/// A picture or chart of a sheet, its image handed to Typst at `path`, and the label of the
/// marker its anchor cell carries.
struct PlacedDrawing {
    label: String,
    path: String,
    row: usize,
    column: usize,
    /// In points, across and down from the corner of the anchor cell.
    offset: (f64, f64),
    width: f64,
    height: f64,
}

/// The drawings of a sheet Typst can show, in the order of their anchors, their images added to
/// `files`.
fn placed_drawings(
    files: &mut Vec<(String, Vec<u8>)>,
    sheet_index: usize,
    sheet: &Sheet,
) -> Vec<PlacedDrawing> {
    let mut drawings: Vec<&SheetDrawing> = sheet.layout.drawings.iter().collect();
    drawings.sort_by_key(|drawing| (drawing.row, drawing.column));
    let mut placed = Vec::new();
    for (index, drawing) in drawings.into_iter().enumerate() {
        let Some(extension) = image_extension(drawing.content_type) else {
            continue;
        };
        #[expect(clippy::cast_precision_loss, reason = "drawing sizes fit in f64")]
        let point = |emu: i64| emu as f64 / EMU_PER_POINT;
        let path = format!("/media/sheets/{sheet_index}-{index}.{extension}");
        files.push((path.clone(), drawing.bytes.clone()));
        placed.push(PlacedDrawing {
            label: format!("rely-drawing-{sheet_index}-{index}"),
            path,
            row: usize::try_from(drawing.row).unwrap_or(usize::MAX),
            column: usize::try_from(drawing.column).unwrap_or(usize::MAX),
            offset: (point(drawing.offset_emu.0), point(drawing.offset_emu.1)),
            width: point(drawing.width_emu),
            height: point(drawing.height_emu),
        });
    }
    placed
}

/// The page foreground that draws each drawing over the cells, where the marker of its anchor
/// cell landed; `None` for a sheet without drawings.
fn drawings_foreground(drawings: &[PlacedDrawing]) -> Option<String> {
    if drawings.is_empty() {
        return None;
    }
    let mut markup = String::from("context { let page = here().page(); ");
    for drawing in drawings {
        let _ = write!(
            markup,
            "{{ let found = query(<{}>); if found.len() > 0 {{ let at = found.first().location().position(); if at.page == page {{ place(top + left, dx: at.x + {}, dy: at.y + {}, image({}, width: {}, height: {}, fit: \"stretch\")) }} }} }}; ",
            drawing.label,
            points(drawing.offset.0),
            points(drawing.offset.1),
            string_literal(&drawing.path),
            points(drawing.width),
            points(drawing.height)
        );
    }
    markup.push('}');
    Some(markup)
}

/// The drawings whose anchor cell is not printed (a hidden row or column, a covered part of a
/// merge) after the cells, each at its size and never wider than the page.
fn write_loose_drawings(
    source: &mut String,
    drawings: &[PlacedDrawing],
    anchored: &HashSet<usize>,
    text_width: f64,
) {
    for (index, drawing) in drawings.iter().enumerate() {
        if anchored.contains(&index) {
            continue;
        }
        let scale = (text_width / drawing.width).min(1.0);
        let _ = writeln!(
            source,
            "#block(above: 12pt, breakable: false, image({}, width: {}, height: {}, fit: \"stretch\"))",
            string_literal(&drawing.path),
            points(drawing.width * scale),
            points(drawing.height * scale)
        );
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

/// Write a sheet's page and its cells, and give back the width of the text area and the
/// drawings whose anchor cell carries a marker.
fn write_sheet(
    source: &mut String,
    sheet: &Sheet,
    looks: &SheetLooks,
    drawings: &[PlacedDrawing],
) -> (f64, HashSet<usize>) {
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
    if let Some(foreground) = drawings_foreground(drawings) {
        let _ = write!(source, ", foreground: {foreground}");
    }
    source.push_str(")\n");
    let _ = writeln!(
        source,
        "#set text(font: {fonts}, size: {}, lang: \"it\", hyphenate: false, top-edge: \"ascender\", bottom-edge: \"descender\")",
        points(font_size)
    );
    // The lines of a wrapped cell sit as close as a spreadsheet sets them.
    source.push_str("#set par(leading: 0.1em)\n");

    let anchored = write_tables(
        source,
        sheet,
        looks,
        width - left - right,
        font_size,
        drawings,
    );
    (width - left - right, anchored)
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

/// The cells of a sheet as tables, one group of columns that fits the page after the other,
/// reaching as far as the cells its drawings are anchored in; hidden rows are left out. The
/// cell a drawing is anchored in carries its marker; the drawings marked are given back.
#[expect(
    clippy::too_many_lines,
    reason = "the columns, rows and every cell of one table"
)]
fn write_tables(
    source: &mut String,
    sheet: &Sheet,
    looks: &SheetLooks,
    text_width: f64,
    font_size: f64,
    drawings: &[PlacedDrawing],
) -> HashSet<usize> {
    let layout = &sheet.layout;
    let (row_count, column_count) = table_extent(sheet, drawings);
    let widths = column_widths(sheet, column_count);
    let inset_y = (font_size * (ROW_HEIGHT_PER_FONT_POINT - 1.15) / 2.0).max(0.0);
    let mut markers: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    for (index, drawing) in drawings.iter().enumerate() {
        markers
            .entry((drawing.row, drawing.column))
            .or_default()
            .push(index);
    }
    let mut anchored = HashSet::new();
    let stroke = if layout.grid_lines {
        GRID_STROKE
    } else {
        "none"
    };
    let visible_rows: Vec<usize> = (0..row_count)
        .filter(|row| {
            u32::try_from(*row)
                .ok()
                .and_then(|row| layout.row_heights.get(&row))
                != Some(&0)
        })
        .collect();
    let merges = merged_cells(layout, row_count, widths.len());
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
                let marker = markers.get(&(row, column)).map(|marked| {
                    let mut labels = String::new();
                    for index in marked {
                        anchored.insert(*index);
                        let _ = write!(labels, "#metadata(none) <{}>", drawings[*index].label);
                    }
                    format!(
                        "place(top + left, dx: -{}, dy: -{})[{labels}]",
                        points(CELL_INSET_X_POINTS),
                        points(inset_y)
                    )
                });
                source.push(' ');
                let look = looks.get(&origin);
                source.push_str(&cell_markup(
                    value,
                    style,
                    colour,
                    look,
                    spans,
                    marker.as_deref(),
                ));
                source.push(',');
            }
        }
        source.push_str(")\n");
    }
    anchored
}

/// The rows and columns a sheet's table prints: its cells, and as far as the cells its
/// drawings are anchored in.
fn table_extent(sheet: &Sheet, drawings: &[PlacedDrawing]) -> (usize, usize) {
    let reach = |cell: fn(&PlacedDrawing) -> usize, from: usize| {
        drawings
            .iter()
            .map(cell)
            .filter(|index| *index < MAX_DRAWING_REACH)
            .map(|index| index + 1)
            .fold(from, usize::max)
    };
    (
        reach(|drawing| drawing.row, sheet.rows.len()),
        reach(
            |drawing| drawing.column,
            sheet.rows.first().map_or(0, Vec::len),
        ),
    )
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
#[expect(
    clippy::too_many_lines,
    reason = "every part of a cell's look, its conditional one included"
)]
fn cell_markup(
    value: &str,
    style: Option<&CellStyle>,
    value_colour: Option<&str>,
    look: Option<&CellLook>,
    spans: Option<(usize, usize)>,
    marker: Option<&str>,
) -> String {
    // A conditional format outdoes the cell's own look, and its font colour the number
    // format's.
    let looked;
    let (style, value_colour) = match look {
        Some(look) => {
            let mut own = style.cloned().unwrap_or_default();
            if look.fill.is_some() {
                own.fill.clone_from(&look.fill);
            }
            if let Some(bold) = look.bold {
                own.font.bold = bold;
            }
            if let Some(italic) = look.italic {
                own.font.italic = italic;
            }
            if let Some(underline) = look.underline {
                own.font.underline = underline;
            }
            if let Some(strike) = look.strike {
                own.font.strike = strike;
            }
            looked = own;
            (Some(&looked), look.font_colour.as_deref().or(value_colour))
        }
        None => (style, value_colour),
    };
    let shown_value = match look {
        Some(look)
            if look.bar.as_ref().is_some_and(|bar| !bar.show_value)
                || look.icon.as_ref().is_some_and(|icon| !icon.show_value) =>
        {
            ""
        }
        _ => value,
    };
    let value = shown_value;
    let mut arguments = Vec::new();
    if let Some((columns, rows)) = spans {
        if columns > 1 {
            arguments.push(format!("colspan: {columns}"));
        }
        if rows > 1 {
            arguments.push(format!("rowspan: {rows}"));
        }
    }
    arguments.extend(alignment_argument(value, style));
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
        } else if looks_numeric(value) {
            // A number wider than its column shows as hashes, as a spreadsheet prints it.
            format!(
                "layout(size => {{ let shown = box({text}); if measure(shown).width > size.width {{ \"#\" * calc.max(1, int(calc.floor(size.width / measure(\"#\").width))) }} else {{ shown }} }})"
            )
        } else {
            format!("box({text})")
        }
    };
    let content = match look {
        Some(look) if look.bar.is_some() || look.icon.is_some() => {
            let content = look.icon.as_ref().map_or(content.clone(), |icon| {
                format!(
                    "grid(columns: (auto, 1fr), column-gutter: 3pt, align: (left + horizon, auto), {}, {content})",
                    icon_markup(icon)
                )
            });
            match &look.bar {
                Some(bar) => format!(
                    "[#place(left + horizon, dx: -{}, rect(width: {}%, height: 1.1em, fill: gradient.linear(rgb(\"#{}\"), rgb(\"#FFFFFF\"))))#{{{content}}}]",
                    points(CELL_INSET_X_POINTS),
                    (bar.length * 100.0).round(),
                    bar.colour
                ),
                None => content,
            }
        }
        _ => content,
    };
    let content = match style.map_or(0, |style| style.indent) {
        0 => content,
        indent => format!(
            "pad(left: {}, {content})",
            points(f64::from(indent) * rely_office_core::INDENT_POINTS)
        ),
    };
    let content = match marker {
        Some(marker) => format!("[#{marker}#{{{content}}}]"),
        None => content,
    };
    if arguments.is_empty() {
        content
    } else {
        format!("table.cell({}, {content})", arguments.join(", "))
    }
}

/// An icon of a set as Typst draws it: an arrow for the arrow sets, a disc for the others,
/// coloured from the lowest icon to the highest.
fn icon_markup(icon: &IconLook) -> String {
    let count = icon
        .set
        .chars()
        .next()
        .and_then(|digit| digit.to_digit(10))
        .map_or(3, |digit| digit.max(2) as usize);
    let index = icon.index.min(count - 1);
    #[expect(clippy::cast_precision_loss, reason = "a set has five icons at most")]
    let share = index as f64 / (count - 1) as f64;
    let colour = if icon.set.contains("Gray") || icon.set.contains("RedToBlack") {
        let level = 200.0 - share * 150.0;
        format!("luma({})", level.round())
    } else if icon.set.contains("Rating")
        || icon.set.contains("Quarters")
        || icon.set.contains("Boxes")
    {
        format!(
            "rgb(\"#4472C4\").lighten({}%)",
            ((1.0 - share) * 70.0).round()
        )
    } else {
        let (red, yellow, green) = (
            (248.0, 105.0, 107.0),
            (255.0, 192.0, 0.0),
            (99.0, 190.0, 123.0),
        );
        let (from, to, part) = if share <= 0.5 {
            (red, yellow, share * 2.0)
        } else {
            (yellow, green, (share - 0.5) * 2.0)
        };
        let mix = |a: f64, b: f64| (a + (b - a) * part).round();
        format!(
            "rgb({}, {}, {})",
            mix(from.0, to.0),
            mix(from.1, to.1),
            mix(from.2, to.2)
        )
    };
    if icon.set.contains("Arrows") {
        // Down for the lowest, up for the highest, turned in steps between.
        let angle = 180.0 - share * 180.0;
        format!(
            "box(width: 0.85em, height: 0.85em, rotate({}deg, polygon(fill: {colour}, (0.425em, 0em), (0.85em, 0.425em), (0.6em, 0.425em), (0.6em, 0.85em), (0.25em, 0.85em), (0.25em, 0.425em), (0em, 0.425em))))",
            angle.round()
        )
    } else {
        format!("box(height: 0.85em, circle(radius: 0.38em, fill: {colour}))")
    }
}

/// The `align` of a cell: its style's, else right for a value that reads as a number.
fn alignment_argument(value: &str, style: Option<&CellStyle>) -> Option<String> {
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
        (Some(horizontal), Some(vertical)) => Some(format!("align: {horizontal} + {vertical}")),
        (Some(horizontal), None) => Some(format!("align: {horizontal} + bottom")),
        (None, Some(vertical)) => Some(format!("align: {vertical}")),
        (None, None) => None,
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

/// The width of each of the first `columns` columns in points, from the sheet's layout; a
/// hidden column is 0.
fn column_widths(sheet: &Sheet, columns: usize) -> Vec<f64> {
    let layout: &SheetLayout = &sheet.layout;
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
    fn places_the_drawings_of_a_sheet_over_the_cells_they_are_anchored_in() {
        let drawing = |row: u32, width_emu: i64, content_type: &'static str| SheetDrawing {
            row,
            column: 3,
            offset_emu: (12_700, 25_400),
            width_emu,
            height_emu: 1_270_000,
            content_type,
            bytes: vec![u8::try_from(row).unwrap()],
            description: None,
            chart: None,
        };
        let mut drawn = sheet("Vendite", &[&["a"]]);
        drawn.layout.row_heights.insert(9, 0);
        drawn.layout.drawings = vec![
            drawing(9, 12_700_000, "image/svg+xml"),
            drawing(2, 127_000_000, "image/png"),
            drawing(5, 12_700, "image/x-emf"),
        ];
        let workbook = Workbook {
            format: DocumentFormat::Xlsx,
            sheets: vec![drawn],
        };
        let (source, files) = workbook_source(&workbook, &[], true);
        assert_eq!(
            files,
            vec![
                ("/media/sheets/0-0.png".to_owned(), vec![2]),
                ("/media/sheets/0-2.svg".to_owned(), vec![9]),
            ],
            "in the order of their anchors, an image Typst cannot read left out"
        );
        assert!(
            source.contains("columns: (48pt, 48pt, 48pt, 48pt, )"),
            "the table reaches the column of the drawing: {source}"
        );
        assert!(source.contains("[#place(top + left, dx: -2pt, dy: -"));
        assert!(source.contains("[#metadata(none) <rely-drawing-0-0>]#{hide[0]}]"));
        assert!(source.contains(concat!(
            "query(<rely-drawing-0-0>); if found.len() > 0 { let at = found.first().location().position(); ",
            "if at.page == page { place(top + left, dx: at.x + 1pt, dy: at.y + 2pt, ",
            "image(\"/media/sheets/0-0.png\", width: 10000pt, height: 100pt, fit: \"stretch\"))"
        )));
        assert!(
            !source.contains("#metadata(none) <rely-drawing-0-2>"),
            "a hidden row prints no anchor"
        );
        let table = source.find("#table(").unwrap();
        let loose = source.find("#block(above: 12pt").unwrap();
        assert!(table < loose);
        assert!(
            source.contains("0-2.svg\", width: 494.48pt, height: 49.45pt"),
            "a drawing with no printed anchor follows the cells, never wider than the page"
        );
        let (without, files) = workbook_source(&workbook, &[], false);
        assert_eq!(files, Vec::<(String, Vec<u8>)>::new());
        assert!(!without.contains("/media/"));
        assert!(!without.contains("foreground"));
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
        let source = workbook_source(&workbook, &[], true).0;
        assert!(source.starts_with(
            "#set page(width: 595.28pt, height: 841.89pt, margin: (top: 54pt, right: 50.4pt, bottom: 54pt, left: 50.4pt), header: none, footer: none)\n"
        ));
        assert!(
            source.contains(r#"font: ("Calibri", "Carlito", "Liberation Sans", ), size: 11pt"#)
        );
        assert!(source.contains(
            "#table(columns: (48pt, 48pt, ), rows: (auto, auto, ), stroke: none, align: bottom,"
        ));
        assert!(source.contains(concat!(
            r#" box("Rossi \"srl\""), table.cell(align: right + bottom, layout(size => { let shown = box("1.234,50"); "#,
            r##"if measure(shown).width > size.width { "#" * calc.max(1, int(calc.floor(size.width / measure("#").width))) } else { shown } })),"##
        )), "a number that does not fit its column shows hashes: {source}");
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
            &[],
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
                indent: 0,
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
            &[],
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
            &[],
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

    #[test]
    fn draws_the_look_a_conditional_format_gives_a_cell() {
        let own = CellStyle {
            fill: Some("FFFFFF".to_owned()),
            ..CellStyle::default()
        };
        let look = CellLook {
            fill: Some("FFC7CE".to_owned()),
            font_colour: Some("9C0006".to_owned()),
            bold: Some(true),
            italic: Some(true),
            underline: Some(true),
            strike: Some(true),
            ..CellLook::default()
        };
        let drawn = cell_markup("12", Some(&own), Some("FF0000"), Some(&look), None, None);
        assert!(drawn.contains("fill: rgb(\"#FFC7CE\")"), "{drawn}");
        assert!(drawn.contains("weight: \"bold\""));
        assert!(drawn.contains("style: \"italic\""));
        assert!(
            drawn.contains("fill: rgb(\"#9C0006\")"),
            "the rule's colour outdoes the number format's"
        );
        assert!(drawn.contains("strike(underline("));
        let bar = CellLook {
            bar: Some(rely_office_sheet::DataBarLook {
                length: 0.5,
                colour: "638EC6".to_owned(),
                show_value: false,
            }),
            icon: Some(IconLook {
                set: "3Arrows".to_owned(),
                index: 2,
                show_value: true,
            }),
            ..CellLook::default()
        };
        let drawn = cell_markup("7", None, None, Some(&bar), None, None);
        assert!(drawn.contains("rect(width: 50%"), "{drawn}");
        assert!(drawn.contains("grid(columns: (auto, 1fr)"));
        assert!(drawn.contains("hide[0]"), "a bar without its value");
        assert!(drawn.contains("rotate(0deg"), "the highest arrow points up");
    }

    #[test]
    fn colours_the_icons_of_each_kind_of_set() {
        let icon = |set: &str, index: usize| {
            icon_markup(&IconLook {
                set: set.to_owned(),
                index,
                show_value: true,
            })
        };
        assert!(icon("3Arrows", 0).contains("rotate(180deg, polygon(fill: rgb(248, 105, 107)"));
        assert!(icon("5Arrows", 2).contains("rotate(90deg"));
        assert!(
            icon("3TrafficLights1", 2).contains("circle(radius: 0.38em, fill: rgb(99, 190, 123))")
        );
        assert!(icon("3ArrowsGray", 0).contains("luma(200)"));
        assert!(icon("4RedToBlack", 3).contains("luma(50)"));
        assert!(icon("5Rating", 4).contains("lighten(0%)"));
        assert!(
            icon("Mystery", 9).contains("circle"),
            "an unknown set has three discs"
        );
    }

    #[test]
    fn computes_the_looks_of_a_workbook_from_its_bytes() {
        use rust_xlsxwriter::{ConditionalFormatCell, ConditionalFormatCellRule, Format};
        let mut book = rust_xlsxwriter::Workbook::new();
        let sheet = book.add_worksheet();
        for row in 0..3u32 {
            sheet.write_number(row, 0, f64::from(row)).unwrap();
        }
        let rule = ConditionalFormatCell::new()
            .set_rule(ConditionalFormatCellRule::EqualTo(2))
            .set_format(Format::new().set_background_color("#00FF00"));
        sheet.add_conditional_format(0, 0, 2, 0, &rule).unwrap();
        let bytes = book.save_to_buffer().unwrap();
        let limits = Limits::default();
        let workbook = rely_office_core::read_workbook(&bytes, Some("a.xlsx"), &limits).unwrap();
        let looks = conditional_looks(&bytes, Some("a.xlsx"), &limits, &workbook);
        assert_eq!(looks.len(), 1);
        assert_eq!(looks[0][&(2, 0)].fill.as_deref(), Some("00FF00"));
        assert!(
            workbook_source(&workbook, &looks, true)
                .0
                .contains("fill: rgb(\"#00FF00\")")
        );
        let plain = rely_office_core::read_workbook(b"a,b\n1,2\n", Some("a.csv"), &limits).unwrap();
        assert_eq!(
            conditional_looks(b"a,b\n1,2\n", Some("a.csv"), &limits, &plain),
            Vec::<SheetLooks>::new()
        );
    }
}
