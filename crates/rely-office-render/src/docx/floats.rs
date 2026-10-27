// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Floating pictures and text boxes. Typst sets no text around an object, so each wrapping is
//! drawn the nearest way it can: an object in front of or behind the text is placed where its
//! anchor puts it on the page, one the text goes around is set beside its paragraph on the
//! side it stands, and one the text goes above and below is set before its paragraph, moved
//! across as its anchor says.

use std::borrow::Cow;
use std::fmt::Write;

use rely_office_core::{
    Alignment, Anchor, AnchorAlign, AnchorBase, Block, Inline, ParagraphLook, TextFrame, Wrap,
};

use super::{Placed, Writer};
use crate::markup::points;

const EMU_PER_POINT: f64 = 12_700.0;
/// The space Word keeps beside an object the text goes around, when its anchor names none.
const DEFAULT_GAP: f64 = 9.0;
/// The insets of a text box Word draws: 0.1 inch beside, 0.05 above and below.
const TEXT_BOX_INSET: (f64, f64) = (7.2, 3.6);

/// The page a section's objects are placed on, in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PagePlace {
    pub width: f64,
    pub height: f64,
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Default for PagePlace {
    fn default() -> Self {
        Self {
            width: 595.3,
            height: 841.9,
            left: 70.85,
            top: 70.85,
            right: 70.85,
            bottom: 70.85,
        }
    }
}

/// A floating object of a paragraph, written out, with its anchor.
pub(super) struct Float {
    pub expression: String,
    pub anchor: Anchor,
}

fn emu(value: i64) -> f64 {
    #[expect(clippy::cast_precision_loss, reason = "page coordinates fit in f64")]
    let value = value as f64;
    value / EMU_PER_POINT
}

/// Sets a paragraph around an object at one side: each line as wide as the room the object
/// leaves at its height, `reaches` saying how much it takes line by line, then the rest of the
/// words at the full width, the block at least as tall as the object.
pub(super) const FLOW_FUNCTION: &str = r"#let rely-flow(object, on-right, drop, height, reaches, leading, justify, indent, words, style) = layout(size => {
  let full = size.width
  let lines = ()
  let index = 0
  let line = 0
  while index < words.len() and line < reaches.len() {
    let first = if line == 0 { indent } else { 0pt }
    let room = calc.max(full - reaches.at(line) - first, 1pt)
    let count = 1
    while index + count < words.len() and measure(style(words.slice(index, index + count + 1).join([ ]))).width <= room {
      count += 1
    }
    let body = words.slice(index, index + count).join([ ])
    if justify and index + count < words.len() { body = body + linebreak(justify: true) }
    let shift = if on-right { 0pt } else { reaches.at(line) }
    lines.push(pad(left: shift + first, block(width: room, style(body))))
    index += count
    line += 1
  }
  if index < words.len() {
    let first = if line == 0 { indent } else { 0pt }
    lines.push(pad(left: first, block(width: full - first, style(words.slice(index).join([ ])))))
  }
  let body = stack(spacing: leading, ..lines)
  let used = measure(block(width: full, body)).height
  block(width: full, {
    place(top + (if on-right { right } else { left }), dy: drop, object)
    body
    if used < drop + height { v(drop + height - used) }
  })
})
";

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a band index, clamped to the bands first"
)]
fn usize_of(value: f64) -> usize {
    value.clamp(0.0, 1_000.0) as usize
}

impl Writer<'_> {
    /// A paragraph that flows around the one object it anchors beside it, line by line along
    /// the object's outline or its rectangle, then on at the full width below it; `None` for
    /// one that sets no such object, or whose text holds a tab.
    #[expect(
        clippy::too_many_lines,
        reason = "the object's side, gap and outline give the room of each line"
    )]
    pub(super) fn flowed(
        &mut self,
        content: &[Inline],
        align: Option<Alignment>,
        look: &ParagraphLook,
        floats: &mut Vec<Float>,
    ) -> Option<Placed> {
        let [float] = floats.as_slice() else {
            return None;
        };
        let anchor = float.anchor;
        let outline = match anchor.wrap {
            Wrap::Around => None,
            Wrap::Contour(outline) => Some(outline),
            _ => return None,
        };
        if emu(anchor.width_emu) > self.text_width * 0.6 || look.list_label.is_some() {
            return None;
        }
        let words = self.words(content, &look.font)?;
        let parts = self.paragraph_parts(content, look, None, 0.0);
        let width = emu(anchor.width_emu).min(self.text_width * 0.8).max(1.0);
        let height = emu(anchor.height_emu).max(1.0);
        let start = match anchor.horizontal.base {
            AnchorBase::Page => emu(anchor.horizontal.offset_emu) - self.page.left,
            _ => emu(anchor.horizontal.offset_emu),
        };
        let right = match anchor.horizontal.align {
            Some(AnchorAlign::End) => true,
            Some(AnchorAlign::Start) => false,
            Some(AnchorAlign::Center) | None => start + width / 2.0 > self.text_width / 2.0,
        };
        let [above, below, left_gap, right_gap] = anchor.distance_emu.map(emu);
        let gap = match (right, left_gap, right_gap) {
            (true, gap, _) | (false, _, gap) if gap > 0.0 => gap,
            _ => DEFAULT_GAP,
        };
        let top = match (anchor.vertical.base, anchor.vertical.align) {
            (AnchorBase::Paragraph | AnchorBase::Line, None) => {
                emu(anchor.vertical.offset_emu).max(0.0)
            }
            _ => 0.0,
        };
        // The width the object takes at a height of it, between 0 and 1 of its own.
        let taken = |from: f64, to: f64| -> f64 {
            let Some(outline) = outline else {
                return 1.0;
            };
            let bands = rely_office_core::OUTLINE_BANDS;
            #[expect(clippy::cast_precision_loss, reason = "sixteen bands")]
            let scale = bands as f64;
            let first = usize_of((from / height * scale).floor()).min(bands - 1);
            let last = usize_of((to / height * scale).ceil()).clamp(first + 1, bands);
            (first..last)
                .filter(|band| outline.left[*band] <= outline.right[*band])
                .map(|band| {
                    let edge = if right {
                        255 - u32::from(outline.left[band])
                    } else {
                        u32::from(outline.right[band])
                    };
                    f64::from(edge) / 255.0
                })
                .fold(0.0, f64::max)
        };
        let mut reaches = String::new();
        let mut y = 0.0;
        while y < top + height + below {
            let (from, to) = (y - top + above.min(top), y - top + parts.line_height);
            let reach = if to < -above || from > height + below {
                0.0
            } else {
                let share = taken(from.clamp(0.0, height), to.clamp(0.0, height));
                if share > 0.0 {
                    share * width + gap
                } else {
                    0.0
                }
            };
            let _ = write!(reaches, "{}, ", points(reach.min(self.text_width)));
            y += parts.line_height;
        }
        let mut list = String::new();
        for word in &words {
            let _ = write!(list, "[{word}], ");
        }
        let object = std::mem::take(floats).remove(0).expression;
        let mut expression = format!(
            "rely-flow({object}, {right}, {}, {}, ({reaches}), {}, {}, {}, ({list}), it => text({}, par(leading: {}, justify: {}, it)))",
            points(top),
            points(height),
            points(parts.leading),
            align == Some(Alignment::Justify),
            points(parts.first_line.max(0.0)),
            parts.text_arguments,
            points(parts.leading),
            align == Some(Alignment::Justify),
        );
        if parts.left > 0.0 || parts.right > 0.0 {
            expression = format!(
                "pad(left: {}, right: {}, {expression})",
                points(parts.left.max(0.0)),
                points(parts.right.max(0.0))
            );
        }
        Some(Placed {
            expression,
            before: parts.before,
            after: parts.after + parts.leading,
            page_break: parts.page_break,
        })
    }

    /// The content of a paragraph without its floating images, and those images written out.
    pub(super) fn split_floats<'c>(
        &mut self,
        content: &'c [Inline],
    ) -> (Cow<'c, [Inline]>, Vec<Float>) {
        let floating = |inline: &Inline| {
            matches!(
                inline,
                Inline::Image {
                    anchor: Some(_),
                    ..
                }
            )
        };
        if !content.iter().any(floating) {
            return (Cow::Borrowed(content), Vec::new());
        }
        let mut kept = Vec::with_capacity(content.len());
        let mut floats = Vec::new();
        for inline in content {
            match inline {
                Inline::Image {
                    media,
                    width,
                    height,
                    anchor: Some(anchor),
                    ..
                } => floats.push(Float {
                    expression: self.image(*media, *width, *height),
                    anchor: **anchor,
                }),
                other => kept.push(other.clone()),
            }
        }
        (Cow::Owned(kept), floats)
    }

    /// A paragraph with the floating objects it anchors.
    pub(super) fn with_floats(&self, mut placed: Placed, floats: Vec<Float>) -> Placed {
        if floats.is_empty() {
            return placed;
        }
        let mut behind = String::new();
        let mut front = String::new();
        for float in floats {
            let wide = emu(float.anchor.width_emu) > self.text_width * 0.6;
            match float.anchor.wrap {
                Wrap::Behind => {
                    let _ = writeln!(behind, "{}", self.overlay(&float.expression, &float.anchor));
                }
                Wrap::InFront => {
                    let _ = writeln!(front, "{}", self.overlay(&float.expression, &float.anchor));
                }
                Wrap::Around | Wrap::Contour(_) if !wide => {
                    placed.expression =
                        self.beside(&placed.expression, &float.expression, &float.anchor);
                }
                Wrap::Around | Wrap::Contour(_) | Wrap::TopAndBottom => {
                    placed.expression = format!(
                        "stack(dir: ttb, spacing: {}, {}, {})",
                        points(emu(float.anchor.distance_emu[1]).max(0.0)),
                        self.across(&float.expression, &float.anchor),
                        placed.expression
                    );
                }
            }
        }
        if !behind.is_empty() || !front.is_empty() {
            placed.expression = format!(
                "block(width: 100%, context {{\nlet origin = here().position()\n{behind}{}\n{front}}})",
                placed.expression
            );
        }
        placed
    }

    /// An object placed at its anchor's point of the page, the page found from where the
    /// paragraph stands.
    fn overlay(&self, object: &str, anchor: &Anchor) -> String {
        let page = self.page;
        let width = emu(anchor.width_emu);
        let height = emu(anchor.height_emu);
        let text_width = (page.width - page.left - page.right).max(1.0);
        let text_height = (page.height - page.top - page.bottom).max(1.0);
        let along = |offset: f64, align: Option<AnchorAlign>, room: f64, size: f64| match align {
            Some(AnchorAlign::Start) => 0.0,
            Some(AnchorAlign::Center) => (room - size) / 2.0,
            Some(AnchorAlign::End) => room - size,
            None => offset,
        };
        let horizontal = anchor.horizontal;
        let x = match horizontal.base {
            AnchorBase::Page => along(
                emu(horizontal.offset_emu),
                horizontal.align,
                page.width,
                width,
            ),
            AnchorBase::Margin | AnchorBase::Column | AnchorBase::Paragraph | AnchorBase::Line => {
                page.left
                    + along(
                        emu(horizontal.offset_emu),
                        horizontal.align,
                        text_width,
                        width,
                    )
            }
        };
        let vertical = anchor.vertical;
        let dy = match vertical.base {
            AnchorBase::Page => format!(
                "{} - origin.y",
                points(along(
                    emu(vertical.offset_emu),
                    vertical.align,
                    page.height,
                    height
                ))
            ),
            AnchorBase::Margin => format!(
                "{} - origin.y",
                points(
                    page.top
                        + along(
                            emu(vertical.offset_emu),
                            vertical.align,
                            text_height,
                            height
                        )
                )
            ),
            AnchorBase::Column | AnchorBase::Paragraph | AnchorBase::Line => {
                points(emu(vertical.offset_emu))
            }
        };
        format!(
            "place(top + left, dx: {} - origin.x, dy: {dy}, {object})",
            points(x)
        )
    }

    /// A paragraph and an object the text goes around, side by side, the object on the side
    /// its anchor puts it.
    fn beside(&self, paragraph: &str, object: &str, anchor: &Anchor) -> String {
        let page = self.page;
        let width = emu(anchor.width_emu).min(self.text_width * 0.8).max(1.0);
        let start = match anchor.horizontal.base {
            AnchorBase::Page => emu(anchor.horizontal.offset_emu) - page.left,
            _ => emu(anchor.horizontal.offset_emu),
        };
        let right = match anchor.horizontal.align {
            Some(AnchorAlign::End) => true,
            Some(AnchorAlign::Start) => false,
            Some(AnchorAlign::Center) | None => start + width / 2.0 > self.text_width / 2.0,
        };
        let [_, _, left_gap, right_gap] = anchor.distance_emu.map(emu);
        if right {
            let gap = if left_gap > 0.0 {
                left_gap
            } else {
                DEFAULT_GAP
            };
            format!(
                "grid(columns: (1fr, {}), column-gutter: {}, {paragraph}, {object})",
                points(width),
                points(gap)
            )
        } else {
            let gap = if right_gap > 0.0 {
                right_gap
            } else {
                DEFAULT_GAP
            };
            format!(
                "grid(columns: ({}, 1fr), column-gutter: {}, {object}, {paragraph})",
                points(width),
                points(gap)
            )
        }
    }

    /// An object on a line of its own, moved across as its anchor says.
    fn across(&self, object: &str, anchor: &Anchor) -> String {
        match anchor.horizontal.align {
            Some(AnchorAlign::Center) => format!("align(center, {object})"),
            Some(AnchorAlign::End) => format!("align(right, {object})"),
            Some(AnchorAlign::Start) => object.to_owned(),
            None => {
                let offset = match anchor.horizontal.base {
                    AnchorBase::Page => emu(anchor.horizontal.offset_emu) - self.page.left,
                    _ => emu(anchor.horizontal.offset_emu),
                };
                if offset > 0.5 {
                    format!(
                        "pad(left: {}, {object})",
                        points(offset.min(self.text_width))
                    )
                } else {
                    object.to_owned()
                }
            }
        }
    }

    /// The text box a run of blocks is in, drawn with its fill and outline at its anchor's size.
    pub(super) fn frame_box(&mut self, blocks: &[Block], frame: &TextFrame) -> String {
        let width = emu(frame.anchor.width_emu).max(TEXT_BOX_INSET.0 * 2.0 + 1.0);
        let height = emu(frame.anchor.height_emu);
        let outer = (self.text_width, self.open_frame);
        self.text_width = (width - TEXT_BOX_INSET.0 * 2.0).max(1.0);
        self.open_frame = Some(frame.id);
        let content = self.blocks(blocks);
        (self.text_width, self.open_frame) = outer;
        let fill = frame
            .fill
            .as_ref()
            .map_or_else(|| "none".to_owned(), |fill| format!("rgb(\"#{fill}\")"));
        let stroke = frame.line.as_ref().map_or_else(
            || "none".to_owned(),
            |(colour, eighths)| {
                format!("{} + rgb(\"#{colour}\")", points(f64::from(*eighths) / 8.0))
            },
        );
        // A shaped box draws its outline behind the text and no rectangle of its own.
        let outline = frame
            .shape
            .as_deref()
            .filter(|_| height > 0.0)
            .and_then(|preset| {
                rely_office_core::preset_outline(
                    preset,
                    frame.anchor.width_emu,
                    frame.anchor.height_emu,
                )
            })
            .map(|paths| {
                paths
                    .iter()
                    .map(|path| {
                        crate::slides::curve_of(
                            path,
                            if path.filled { &fill } else { "none" },
                            if path.stroked { &stroke } else { "none" },
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" + ")
            });
        let (fill, stroke, content) = match outline {
            Some(curves) => (
                "none".to_owned(),
                "none".to_owned(),
                format!(
                    "#place(top + left, dx: -{}, dy: -{}, box(width: {}, height: {}, {curves}))#align(horizon + center, block[{content}])",
                    points(TEXT_BOX_INSET.0),
                    points(TEXT_BOX_INSET.1),
                    points(width),
                    points(height)
                ),
            ),
            None => (fill, stroke, content),
        };
        let height = if height > 0.0 {
            format!(", height: {}", points(height))
        } else {
            String::new()
        };
        format!(
            "block(width: {}{height}, fill: {fill}, stroke: {stroke}, inset: (x: {}, y: {}), breakable: false)[{content}]",
            points(width),
            points(TEXT_BOX_INSET.0),
            points(TEXT_BOX_INSET.1)
        )
    }
}

/// The text box a block is in.
pub(super) fn frame_of(block: &Block) -> Option<&TextFrame> {
    match block {
        Block::Paragraph { look, .. }
        | Block::Heading { look, .. }
        | Block::ListItem { look, .. } => look.frame.as_ref(),
        Block::Table { look, .. } => look.frame.as_ref(),
        Block::Blockquote { blocks } => blocks.first().and_then(frame_of),
        Block::CodeBlock { .. } | Block::HorizontalRule => None,
    }
}

#[cfg(test)]
mod tests;
