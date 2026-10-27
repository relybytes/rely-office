// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The SVG a metafile is drawn into: paths, text and images, each in the coordinates of the
//! device the metafile was recorded on.

use std::fmt::Write as _;

use super::MAX_SVG_BYTES;
use super::bitmap::base64;

/// How a path is painted.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Paint {
    /// `RRGGBB`, or none.
    pub fill: Option<String>,
    pub even_odd: bool,
    /// `RRGGBB`, or none.
    pub stroke: Option<String>,
    pub stroke_width: f64,
    /// Dash and gap lengths, empty for a solid line.
    pub dashes: Vec<f64>,
}

/// A run of text placed at its reference point.
#[derive(Debug, Clone, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is a font property a metafile font sets on its own"
)]
pub(super) struct TextRun {
    pub x: f64,
    pub y: f64,
    pub text: String,
    pub family: String,
    pub size: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    /// `RRGGBB`.
    pub colour: String,
    /// `start`, `middle` or `end`.
    pub anchor: &'static str,
    /// How far below the reference point the baseline sits, in device units.
    pub baseline_shift: f64,
    /// Counter-clockwise, in degrees.
    pub angle: f64,
    /// The width the text is stretched to, as the metafile spaced its characters.
    pub width: Option<f64>,
    /// The box behind the text, when the background is opaque.
    pub background: Option<(String, [f64; 4])>,
}

#[derive(Debug, Default)]
pub(super) struct Canvas {
    body: String,
    full: bool,
    /// The clip the elements are drawn through: the device paths intersected, nearest last.
    clip: Vec<String>,
    /// Whether a clipped group is open.
    clipped: bool,
    clips_defined: usize,
}

impl Canvas {
    pub(super) fn path(&mut self, data: &str, paint: &Paint) {
        if data.is_empty() || (paint.fill.is_none() && paint.stroke.is_none()) {
            return;
        }
        let fill = paint
            .fill
            .as_ref()
            .map_or_else(|| "none".to_owned(), |colour| format!("#{colour}"));
        let mut element = format!(r#"<path d="{data}" fill="{fill}""#);
        if paint.even_odd && paint.fill.is_some() {
            element.push_str(r#" fill-rule="evenodd""#);
        }
        if let Some(stroke) = &paint.stroke {
            let _ = write!(
                element,
                r##" stroke="#{stroke}" stroke-width="{}" stroke-linejoin="round""##,
                number(paint.stroke_width.max(0.01))
            );
            if !paint.dashes.is_empty() {
                let dashes: Vec<String> = paint.dashes.iter().map(|dash| number(*dash)).collect();
                let _ = write!(element, r#" stroke-dasharray="{}""#, dashes.join(" "));
            }
        }
        element.push_str("/>");
        self.push(&element);
    }

    pub(super) fn text(&mut self, run: &TextRun) {
        if run.text.trim().is_empty() && run.background.is_none() {
            return;
        }
        if let Some((colour, [left, top, right, bottom])) = &run.background {
            self.push(&format!(
                r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#{colour}"/>"##,
                number(*left),
                number(*top),
                number((right - left).max(0.0)),
                number((bottom - top).max(0.0))
            ));
        }
        if run.text.trim().is_empty() {
            return;
        }
        let mut element = format!(
            r##"<text x="{}" y="{}" font-family="{}" font-size="{}" fill="#{}""##,
            number(run.x),
            number(run.y + run.baseline_shift),
            escape(&run.family),
            number(run.size.max(0.1)),
            run.colour
        );
        if run.anchor != "start" {
            let _ = write!(element, r#" text-anchor="{}""#, run.anchor);
        }
        if run.bold {
            element.push_str(r#" font-weight="bold""#);
        }
        if run.italic {
            element.push_str(r#" font-style="italic""#);
        }
        let decoration = match (run.underline, run.strike) {
            (true, true) => Some("underline line-through"),
            (true, false) => Some("underline"),
            (false, true) => Some("line-through"),
            (false, false) => None,
        };
        if let Some(decoration) = decoration {
            let _ = write!(element, r#" text-decoration="{decoration}""#);
        }
        if let Some(width) = run.width.filter(|width| *width > 0.0) {
            let _ = write!(
                element,
                r#" textLength="{}" lengthAdjust="spacingAndGlyphs""#,
                number(width)
            );
        }
        if run.angle.abs() > 0.01 {
            let _ = write!(
                element,
                r#" transform="rotate({} {} {})""#,
                number(-run.angle),
                number(run.x),
                number(run.y)
            );
        }
        let _ = write!(
            element,
            r#" xml:space="preserve">{}</text>"#,
            escape(&run.text)
        );
        self.push(&element);
    }

    /// An image of `media_type` stretched over a rectangle; a negative size mirrors it.
    pub(super) fn image(
        &mut self,
        media_type: &str,
        bytes: &[u8],
        [x, y, width, height]: [f64; 4],
    ) {
        if width.abs() < 0.01 || height.abs() < 0.01 {
            return;
        }
        let (left, flip_x) = if width < 0.0 {
            (x + width, true)
        } else {
            (x, false)
        };
        let (top, flip_y) = if height < 0.0 {
            (y + height, true)
        } else {
            (y, false)
        };
        let mut element = format!(
            r#"<image x="{}" y="{}" width="{}" height="{}" preserveAspectRatio="none""#,
            number(left),
            number(top),
            number(width.abs()),
            number(height.abs())
        );
        if flip_x || flip_y {
            let centre_x = left + width.abs() / 2.0;
            let centre_y = top + height.abs() / 2.0;
            let _ = write!(
                element,
                r#" transform="translate({} {}) scale({} {}) translate({} {})""#,
                number(centre_x),
                number(centre_y),
                if flip_x { -1 } else { 1 },
                if flip_y { -1 } else { 1 },
                number(-centre_x),
                number(-centre_y)
            );
        }
        let _ = write!(
            element,
            r#" href="data:{media_type};base64,{}"/>"#,
            base64(bytes)
        );
        self.push(&element);
    }

    /// Draw what follows through the intersection of these device paths; none is no clip.
    pub(super) fn set_clip(&mut self, clip: &[String]) {
        if self.clip == clip {
            return;
        }
        if self.clipped {
            self.push("</g>");
            self.clipped = false;
        }
        clip.clone_into(&mut self.clip);
        if clip.is_empty() {
            return;
        }
        let mut parent: Option<usize> = None;
        let mut definitions = String::new();
        for data in clip {
            self.clips_defined += 1;
            let id = self.clips_defined;
            let inherited = parent.map_or_else(String::new, |parent| {
                format!(r#" clip-path="url(#clip{parent})""#)
            });
            let _ = write!(
                definitions,
                r#"<clipPath id="clip{id}"{inherited}><path d="{data}"/></clipPath>"#
            );
            parent = Some(id);
        }
        if let Some(last) = parent {
            let _ = write!(definitions, r#"<g clip-path="url(#clip{last})">"#);
            self.push(&definitions);
            self.clipped = !self.full;
        }
    }

    fn push(&mut self, element: &str) {
        if self.full || self.body.len() + element.len() > MAX_SVG_BYTES {
            self.full = true;
            return;
        }
        self.body.push_str(element);
    }

    /// The SVG document: the device rectangle `view_box` shown at `width` by `height` points.
    pub(super) fn finish(
        mut self,
        view_box: [f64; 4],
        (width, height): (f64, f64),
    ) -> Option<String> {
        if self.clipped {
            // The closing tag is let through past the size cap, so the document stays whole.
            self.body.push_str("</g>");
        }
        if !self.body.contains("<path")
            && !self.body.contains("<text")
            && !self.body.contains("<image")
            && !self.body.contains("<rect")
        {
            return None;
        }
        if view_box[2] <= 0.0 || view_box[3] <= 0.0 {
            return None;
        }
        Some(format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}pt" height="{}pt" viewBox="{} {} {} {}" preserveAspectRatio="none">{}</svg>"#,
            number(width.max(1.0)),
            number(height.max(1.0)),
            number(view_box[0]),
            number(view_box[1]),
            number(view_box[2]),
            number(view_box[3]),
            self.body
        ))
    }
}

/// A coordinate as SVG writes it: at most two decimals, no trailing zeros.
pub(super) fn number(value: f64) -> String {
    if !value.is_finite() {
        return "0".to_owned();
    }
    let mut text = format!("{:.2}", (value * 100.0).round() / 100.0);
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    if text == "-0" {
        "0".clone_into(&mut text);
    }
    text
}

/// Text for an attribute or an element: the characters XML gives a meaning escaped, those it
/// cannot hold dropped.
pub(super) fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\t' | '\n' | '\r' => escaped.push(' '),
            character if u32::from(character) < 0x20 => {}
            '\u{FFFE}' | '\u{FFFF}' => {}
            character => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests;
