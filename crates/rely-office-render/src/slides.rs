// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! A presentation written out as Typst source: one page per slide at the slide's size, its
//! background, and every shape placed at its frame with its fill, outline, text, picture or
//! table, the first shape at the back.

use std::collections::HashMap;
use std::fmt::Write;

use rely_office_core::{
    Alignment, Bullet, FillShade, Frame, Gradient, GradientKind, Media, Paint, PathCommand,
    PathPoint, Presentation, ShapeContent, ShapeGeometry, ShapePath, Slide, SlideParagraph,
    SlideRun, SlideShape, SlideTable, TextAnchor, TextBody,
};

use crate::fonts::body_fonts;
use crate::markup::{font_list, points, string_literal};
use crate::metrics::FontLines;

const EMU_PER_POINT: f64 = 12_700.0;
/// `PowerPoint` sets a single line at 1.2 times the font size.
const SINGLE_LINE_EM: f64 = 1.2;
/// The corner of a rounded rectangle, a sixth of its shorter side as `PowerPoint` draws it.
const ROUNDED_CORNER: f64 = 1.0 / 6.0;

fn emu(value: i64) -> f64 {
    #[expect(clippy::cast_precision_loss, reason = "slide coordinates fit in f64")]
    let value = value as f64;
    value / EMU_PER_POINT
}

/// The source and the image files of a presentation. With `with_images` false every picture is
/// drawn as an empty frame, for one whose images Typst cannot decode.
pub(crate) fn presentation_source(
    presentation: &Presentation,
    with_images: bool,
    lines: &FontLines,
) -> (String, Vec<(String, Vec<u8>)>) {
    let mut writer = Writer {
        media: &presentation.media,
        with_images,
        lines,
        files: HashMap::new(),
        page: (presentation.width_emu, presentation.height_emu),
    };
    let mut source = String::new();
    let _ = writeln!(
        source,
        "#set page(width: {}, height: {}, margin: 0pt)",
        points(emu(presentation.width_emu)),
        points(emu(presentation.height_emu))
    );
    source.push_str(concat!(
        "#set text(lang: \"it\", hyphenate: false, top-edge: \"ascender\", bottom-edge: \"descender\")\n",
        "#set par(spacing: 0pt, linebreaks: \"simple\")\n#set block(spacing: 0pt)\n",
    ));
    if presentation.slides.is_empty() {
        source.push_str("#page[]\n");
    }
    for slide in &presentation.slides {
        source.push_str(&writer.slide(slide));
    }
    let mut files: Vec<(String, Vec<u8>)> = writer
        .files
        .into_iter()
        .map(|(index, path)| (path, presentation.media[index].bytes.clone()))
        .collect();
    files.sort();
    (source, files)
}

struct Writer<'a> {
    media: &'a [Media],
    with_images: bool,
    lines: &'a FontLines,
    files: HashMap<usize, String>,
    /// The slide size, in English Metric Units.
    page: (i64, i64),
}

impl Writer<'_> {
    fn slide(&mut self, slide: &Slide) -> String {
        let fill = match slide.background.as_ref() {
            Some(paint) => self.paint(paint, FillShade::Normal, self.page),
            None => "white".to_owned(),
        };
        let mut markup = format!("#page(fill: {fill})[");
        for shape in &slide.shapes {
            let drawn = self.shape(shape);
            let Frame { x, y, .. } = shape.frame;
            let _ = write!(
                markup,
                "#place(top + left, dx: {}, dy: {}, {drawn})",
                points(emu(x)),
                points(emu(y))
            );
        }
        markup.push_str("]\n");
        markup
    }

    fn shape(&mut self, shape: &SlideShape) -> String {
        let width = points(emu(shape.frame.width));
        let height = points(emu(shape.frame.height));
        let size = (shape.frame.width, shape.frame.height);
        let fill = shape.fill.as_ref().map_or_else(
            || "none".to_owned(),
            |paint| self.paint(paint, FillShade::Normal, size),
        );
        let stroke = shape.line.as_ref().map_or_else(
            || "none".to_owned(),
            |line| {
                format!(
                    "{} + rgb(\"#{}\")",
                    points(emu(line.width_emu).max(0.25)),
                    line.colour
                )
            },
        );
        let drawn = match (&shape.geometry, &shape.content) {
            (ShapeGeometry::Line { flipped }, _) => {
                let (start, end) = if *flipped {
                    (format!("(0pt, {height})"), format!("({width}, 0pt)"))
                } else {
                    ("(0pt, 0pt)".to_owned(), format!("({width}, {height})"))
                };
                let stroke = if shape.line.is_some() {
                    stroke
                } else {
                    "0.75pt + black".to_owned()
                };
                format!(
                    "box(width: {width}, height: {height}, place(line(start: {start}, end: {end}, stroke: {stroke})))"
                )
            }
            (_, ShapeContent::Picture { media }) => self.picture(*media, &width, &height, &stroke),
            (_, ShapeContent::Table(table)) => self.table(table, &width),
            (ShapeGeometry::Path(paths), content) => {
                let (inset, body) = match content {
                    ShapeContent::Text(text) => (insets(text), self.text(text)),
                    _ => ("0pt".to_owned(), "[]".to_owned()),
                };
                // Every face is filled before any outline is drawn, so a stroke-only path
                // over a filled one stays visible.
                let mut layers = String::new();
                for path in paths.iter().filter(|path| path.filled) {
                    let path_fill = shape.fill.as_ref().map_or_else(
                        || "none".to_owned(),
                        |paint| self.paint(paint, path.shade, size),
                    );
                    let _ = write!(
                        layers,
                        "#place(top + left, {})",
                        curve_of(path, &path_fill, "none")
                    );
                }
                for path in paths.iter().filter(|path| path.stroked) {
                    let _ = write!(
                        layers,
                        "#place(top + left, {})",
                        curve_of(path, "none", &stroke)
                    );
                }
                format!(
                    "block(width: {width}, height: {height})[{layers}#block(width: 100%, height: 100%, inset: {inset}, {body})]"
                )
            }
            (geometry, content) => {
                let (inset, body) = match content {
                    ShapeContent::Text(text) => (insets(text), self.text(text)),
                    _ => ("0pt".to_owned(), "[]".to_owned()),
                };
                let frame = if *geometry == ShapeGeometry::Ellipse {
                    "ellipse"
                } else {
                    "block"
                };
                let radius = if *geometry == ShapeGeometry::RoundedRectangle {
                    format!(
                        ", radius: {}",
                        points(emu(shape.frame.width.min(shape.frame.height)) * ROUNDED_CORNER)
                    )
                } else {
                    String::new()
                };
                format!(
                    "{frame}(width: {width}, height: {height}, fill: {fill}, stroke: {stroke}{radius}, inset: {inset}, {body})"
                )
            }
        };
        if shape.rotation.abs() > f64::EPSILON {
            format!(
                "rotate({}deg, origin: center + horizon, {drawn})",
                round(shape.rotation)
            )
        } else {
            drawn
        }
    }

    /// A Typst fill for a paint over a frame of `size`: a colour, a gradient, or the image
    /// as a tiling stretched over the frame or repeated at its tile size.
    fn paint(&mut self, paint: &Paint, shade: FillShade, size: (i64, i64)) -> String {
        match paint {
            Paint::Solid(colour) => format!("rgb(\"#{}\")", shade.apply(colour)),
            Paint::Gradient(gradient) => gradient_fill(gradient, shade),
            Paint::Picture { media, tile_emu } => {
                let Some(path) = self.image_file(*media) else {
                    return "luma(230)".to_owned();
                };
                let (width, height) = tile_emu.unwrap_or(size);
                let (width, height) = (
                    points(emu(width.max(12_700))),
                    points(emu(height.max(12_700))),
                );
                format!(
                    "tiling(size: ({width}, {height}))[#image({}, width: {width}, height: {height}, fit: \"stretch\")]",
                    string_literal(&path)
                )
            }
        }
    }

    /// The path an image is handed to Typst at; `None` for an image Typst cannot decode, or
    /// when images are left out.
    fn image_file(&mut self, media: usize) -> Option<String> {
        let extension = self
            .media
            .get(media)
            .and_then(|media| image_extension(media.content_type))
            .filter(|_| self.with_images)?;
        Some(
            self.files
                .entry(media)
                .or_insert_with(|| format!("/media/slides/{media}.{extension}"))
                .clone(),
        )
    }

    fn picture(&mut self, media: usize, width: &str, height: &str, stroke: &str) -> String {
        match self.image_file(media) {
            Some(path) => {
                format!(
                    "block(width: {width}, height: {height}, stroke: {stroke}, image({}, width: 100%, height: 100%, fit: \"stretch\"))",
                    string_literal(&path)
                )
            }
            _ => format!("block(width: {width}, height: {height}, stroke: 0.5pt + luma(180))"),
        }
    }

    fn table(&mut self, table: &SlideTable, width: &str) -> String {
        let columns = table.column_widths_emu.len().max(
            table
                .rows
                .iter()
                .map(|row| row.cells.len())
                .max()
                .unwrap_or(0),
        );
        if columns == 0 {
            return "[]".to_owned();
        }
        let mut markup = String::from("table(columns: (");
        if table.column_widths_emu.len() == columns {
            for column in &table.column_widths_emu {
                let _ = write!(markup, "{}, ", points(emu(*column)));
            }
        } else {
            markup.push_str(&"1fr, ".repeat(columns));
        }
        markup.push_str("), rows: (");
        for row in &table.rows {
            if row.height_emu > 0 {
                let _ = write!(markup, "{}, ", points(emu(row.height_emu)));
            } else {
                markup.push_str("auto, ");
            }
        }
        markup.push_str("), stroke: 0.5pt + luma(160), inset: 0pt,");
        for row in &table.rows {
            for cell in &row.cells {
                if cell.covered {
                    continue;
                }
                let mut arguments = vec![format!("inset: {}", insets(&cell.body))];
                if cell.col_span > 1 {
                    arguments.push(format!("colspan: {}", cell.col_span));
                }
                if cell.row_span > 1 {
                    arguments.push(format!("rowspan: {}", cell.row_span));
                }
                if let Some(fill) = &cell.fill {
                    arguments.push(format!("fill: rgb(\"#{fill}\")"));
                }
                let _ = write!(
                    markup,
                    " table.cell({}, {}),",
                    arguments.join(", "),
                    self.text(&cell.body)
                );
            }
        }
        markup.push(')');
        format!("block(width: {width}, {markup})")
    }

    /// A text body as content: its paragraphs one under the other, set where the anchor puts
    /// them.
    fn text(&mut self, body: &TextBody) -> String {
        let vertical = match body.anchor {
            TextAnchor::Top => "top",
            TextAnchor::Middle => "horizon",
            TextAnchor::Bottom => "bottom",
        };
        let mut markup = String::new();
        let mut previous_after = 0.0;
        for (index, paragraph) in body.paragraphs.iter().enumerate() {
            let before = f64::from(paragraph.space_before) / 100.0;
            let gap = if index == 0 {
                0.0
            } else {
                previous_after + before
            };
            if gap > 0.0 {
                let _ = write!(markup, "#v({})", points(gap));
            }
            markup.push('#');
            markup.push_str(&self.paragraph(paragraph));
            markup.push('\n');
            previous_after = f64::from(paragraph.space_after) / 100.0;
        }
        let content = if body.wrap {
            format!("block(width: 100%)[{markup}]")
        } else {
            format!("box[{markup}]")
        };
        format!("align({vertical}, {content})")
    }

    fn paragraph(&mut self, paragraph: &SlideParagraph) -> String {
        let size = paragraph
            .runs
            .iter()
            .map(|run| run.size)
            .max()
            .unwrap_or(1_800);
        let size = f64::from(size) / 100.0;
        let family = paragraph
            .runs
            .first()
            .map(|run| run.family.clone())
            .unwrap_or_default();
        let families = body_fonts(Some(&family));
        let metrics = self.lines.line(&families);
        let line = SINGLE_LINE_EM.max(metrics.single_em)
            * f64::from(paragraph.line_spacing_percent)
            / 100.0;
        // An office suite sets the first baseline the font's ascent below the top of the line and
        // gives every line, the first of the next paragraph too, the same height; Typst's own
        // ascender is the typographic one, short of that in fonts such as Liberation Sans.
        let top = (metrics.single_em - metrics.descent_em).min(line) * size;
        let bottom = top - line * size;
        let mut runs = String::new();
        for run in &paragraph.runs {
            runs.push('#');
            runs.push_str(&run_markup(run));
        }
        if runs.is_empty() {
            let _ = write!(runs, "#text(size: {})[#hide[x]]", points(size));
        }
        let justify = if paragraph.align == Alignment::Justify {
            ", justify: true"
        } else {
            ""
        };
        let alignment = match paragraph.align {
            Alignment::Center => "center",
            Alignment::Right => "right",
            Alignment::Left | Alignment::Justify => "left",
        };
        let margin = emu(paragraph.margin_left_emu);
        let indent = emu(paragraph.indent_emu);
        let text = format!(
            "text(font: {}, size: {}, top-edge: {}, bottom-edge: {}, par(leading: 0pt{justify})[{runs}])",
            font_list(&families),
            points(size),
            points(top),
            points(bottom)
        );
        let body = match &paragraph.bullet {
            Some(bullet) => {
                let label = match bullet {
                    Bullet::Character(character) | Bullet::Number(character) => character,
                };
                let colour = paragraph
                    .runs
                    .first()
                    .map_or_else(|| "000000".to_owned(), |run| run.colour.clone());
                let hanging = (-indent).max(size * 0.6);
                format!(
                    "pad(left: {}, grid(columns: ({}, 1fr), text(font: {}, size: {}, fill: rgb(\"#{colour}\"), {}), {text}))",
                    points((margin + indent.min(0.0)).max(0.0)),
                    points(hanging),
                    font_list(&families),
                    points(size),
                    string_literal(label)
                )
            }
            None => format!(
                "pad(left: {}, {text})",
                points((margin + indent.max(0.0)).max(0.0))
            ),
        };
        format!("align({alignment}, {body})")
    }
}

/// A run with its font, size, colour and marks.
fn run_markup(run: &SlideRun) -> String {
    if run.line_break {
        return "linebreak()".to_owned();
    }
    let mut arguments = vec![
        format!("font: {}", font_list(&body_fonts(Some(&run.family)))),
        format!("size: {}", points(f64::from(run.size) / 100.0)),
        format!("fill: rgb(\"#{}\")", run.colour),
    ];
    if run.bold {
        arguments.push("weight: \"bold\"".to_owned());
    }
    if run.italic {
        arguments.push("style: \"italic\"".to_owned());
    }
    let mut expression = format!(
        "text({}, {})",
        arguments.join(", "),
        string_literal(&run.text)
    );
    if run.underline {
        expression = format!("underline({expression})");
    }
    if run.strike {
        expression = format!("strike({expression})");
    }
    expression
}

/// A Typst gradient, each colour shaded; the stops in order, as Typst needs them.
fn gradient_fill(gradient: &Gradient, shade: FillShade) -> String {
    let mut stops: Vec<(f64, String)> = gradient
        .stops
        .iter()
        .map(|stop| (stop.offset.clamp(0.0, 1.0), shade.apply(&stop.colour)))
        .collect();
    stops.sort_by(|first, second| first.0.total_cmp(&second.0));
    match stops.len() {
        0 => return "none".to_owned(),
        1 => return format!("rgb(\"#{}\")", stops[0].1),
        _ => {}
    }
    let mut markup = String::new();
    for (offset, colour) in &stops {
        let _ = write!(markup, "(rgb(\"#{colour}\"), {}%), ", round(offset * 100.0));
    }
    match gradient.kind {
        GradientKind::Linear { angle } => {
            format!("gradient.linear({markup}angle: {}deg)", round(angle))
        }
        GradientKind::Radial { centre: (x, y) } => {
            let (x, y) = (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
            let radius = (x.max(1.0 - x).hypot(y.max(1.0 - y)) * 100.0).max(1.0);
            format!(
                "gradient.radial({markup}center: ({}%, {}%), radius: {}%)",
                round(x * 100.0),
                round(y * 100.0),
                round(radius)
            )
        }
    }
}

/// A path as a Typst curve, in points from the frame's top-left corner.
pub(crate) fn curve_of(path: &ShapePath, fill: &str, stroke: &str) -> String {
    let at = |point: &PathPoint| format!("({}, {})", points(emu(point.x)), points(emu(point.y)));
    let mut curve = format!("curve(fill: {fill}, fill-rule: \"even-odd\", stroke: {stroke}");
    for command in &path.commands {
        let _ = match command {
            PathCommand::MoveTo(point) => write!(curve, ", curve.move({})", at(point)),
            PathCommand::LineTo(point) => write!(curve, ", curve.line({})", at(point)),
            PathCommand::CubicTo(first, second, end) => write!(
                curve,
                ", curve.cubic({}, {}, {})",
                at(first),
                at(second),
                at(end)
            ),
            PathCommand::Close => write!(curve, ", curve.close(mode: \"straight\")"),
        };
    }
    curve.push(')');
    curve
}

fn insets(body: &TextBody) -> String {
    let [left, top, right, bottom] = body.insets_emu.map(emu);
    format!(
        "(left: {}, top: {}, right: {}, bottom: {})",
        points(left.max(0.0)),
        points(top.max(0.0)),
        points(right.max(0.0)),
        points(bottom.max(0.0))
    )
}

fn round(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    let mut text = format!("{rounded:.2}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
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

#[cfg(test)]
mod tests;
