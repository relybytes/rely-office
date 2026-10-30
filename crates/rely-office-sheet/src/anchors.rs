// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! The anchors of a sheet's drawing moved with its rows and columns, as Excel moves them: a
//! chart or a picture that moves and sizes with its cells (`twoCellAnchor`, the default) has both
//! corners follow the cells under them, so rows inserted inside it make it taller and rows deleted
//! inside it make it shorter; one that moves but keeps its size (`oneCellAnchor`, or a
//! `twoCellAnchor` with `editAs="oneCell"`) has its top-left corner follow its cell; one placed
//! on the sheet (`absoluteAnchor`, or `editAs="absolute"`) stays where it is. An anchor whose
//! cells are all deleted moves to the edge of the deletion and keeps its size.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::address::{MAX_COLUMNS, MAX_ROWS};
use crate::edit::Shift;
use crate::model::Sheet;

/// One corner of an anchor: the cell index along the shifted axis and the offset into it, with
/// where their texts sit in the part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Marker {
    index: i64,
    offset: i64,
    index_span: (usize, usize),
    offset_span: Option<(usize, usize)>,
}

/// How an anchor follows its cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Follows {
    /// Both corners follow their cells: moves and sizes with them.
    Cells,
    /// The top-left corner follows its cell, the size stays.
    Corner,
    /// Placed on the sheet, not on cells.
    Nothing,
}

/// A number read from the part, with where its text sits.
type Read = (i64, (usize, usize));

/// A corner as an index along the shifted axis and the offset into it.
type Corner = (i64, i64);

#[derive(Debug, Default)]
struct FoundAnchor {
    follows: Option<Follows>,
    from: Option<Marker>,
    to: Option<Marker>,
}

/// Where a corner lands: past an insertion at or above it, it moves down; inside a deletion it
/// goes to the deletion's edge with no offset; below one it moves up.
fn moved(shift: Shift, index: i64, offset: i64) -> (i64, i64) {
    let limit = i64::from(if shift.rows() { MAX_ROWS } else { MAX_COLUMNS }) - 1;
    match shift {
        Shift::InsertRows { at, count } | Shift::InsertColumns { at, count } => {
            if index >= i64::from(at) {
                ((index + i64::from(count)).min(limit), offset)
            } else {
                (index, offset)
            }
        }
        Shift::DeleteRows { at, count } | Shift::DeleteColumns { at, count } => {
            let (at, count) = (i64::from(at), i64::from(count));
            if index < at {
                (index, offset)
            } else if index < at + count {
                (at, 0)
            } else {
                (index - count, offset)
            }
        }
    }
}

/// Where the bottom-right corner of an anchor that sizes with its cells lands: rows inserted
/// at the row it ends on, with no offset into it, fall below it and leave it where it is.
fn moved_end(shift: Shift, index: i64, offset: i64) -> (i64, i64) {
    match shift {
        Shift::InsertRows { at, .. } | Shift::InsertColumns { at, .. }
            if index == i64::from(at) && offset == 0 =>
        {
            (index, offset)
        }
        _ => moved(shift, index, offset),
    }
}

/// The new corners of an anchor, or `None` when it stays.
fn new_corners(
    shift: Shift,
    follows: Follows,
    from: Marker,
    to: Option<Marker>,
) -> Option<(Corner, Option<Corner>)> {
    let limit = i64::from(if shift.rows() { MAX_ROWS } else { MAX_COLUMNS }) - 1;
    let start = moved(shift, from.index, from.offset);
    let delta = start.0 - from.index;
    let keep_size = |to: Marker| ((to.index + delta).clamp(start.0, limit), to.offset);
    match (follows, to) {
        (Follows::Nothing, _) => None,
        (Follows::Corner, to) => Some((start, to.map(keep_size))),
        (Follows::Cells, None) => Some((start, None)),
        (Follows::Cells, Some(to)) => {
            let end = moved_end(shift, to.index, to.offset);
            let had_extent = (to.index, to.offset) > (from.index, from.offset);
            // All its cells were deleted: it moves to the edge of the deletion with its size.
            if had_extent && end <= start {
                Some((start, Some(keep_size(to))))
            } else {
                Some((start, Some(end)))
            }
        }
    }
}

/// The local name of an element, without its prefix.
fn local(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// How the anchor an element starts follows its cells, or `None` when it starts no anchor.
fn anchor_kind(element: &quick_xml::events::BytesStart) -> Option<Follows> {
    match local(element.name().as_ref()) {
        "twoCellAnchor" => {
            let edit_as = element
                .attributes()
                .flatten()
                .find(|attribute| local(attribute.key.as_ref()) == "editAs")
                .map(|attribute| attribute.value.into_owned());
            Some(match edit_as.as_deref() {
                Some("oneCell") => Follows::Corner,
                Some("absolute") => Follows::Nothing,
                _ => Follows::Cells,
            })
        }
        "oneCellAnchor" => Some(Follows::Corner),
        "absoluteAnchor" => Some(Follows::Nothing),
        _ => None,
    }
}

/// The anchors of a drawing part with the corners along one axis.
fn find_anchors(xml: &str, rows: bool) -> Vec<FoundAnchor> {
    let (index_name, offset_name) = if rows {
        ("row", "rowOff")
    } else {
        ("col", "colOff")
    };
    let mut reader = Reader::from_str(xml);
    let mut found: Vec<FoundAnchor> = Vec::new();
    // The element names from the anchor down, while inside one.
    let mut path: Vec<String> = Vec::new();
    let mut inside = false;
    // The corner being read: whether it is `from`, and its index and offset with their texts.
    let mut corner: Option<(bool, Option<Read>, Option<Read>)> = None;
    loop {
        let Ok(before) = usize::try_from(reader.buffer_position()) else {
            return Vec::new();
        };
        let Ok(event) = reader.read_event() else {
            return Vec::new();
        };
        let Ok(after) = usize::try_from(reader.buffer_position()) else {
            return Vec::new();
        };
        match event {
            Event::Start(element) => {
                let name = local(element.name().as_ref()).to_owned();
                if inside {
                    if path.is_empty() && (name == "from" || name == "to") {
                        corner = Some((name == "from", None, None));
                    }
                    path.push(name);
                    continue;
                }
                if let Some(follows) = anchor_kind(&element) {
                    inside = true;
                    path.clear();
                    found.push(FoundAnchor {
                        follows: Some(follows),
                        ..FoundAnchor::default()
                    });
                }
            }
            Event::End(element) => {
                if !inside {
                    continue;
                }
                if path.is_empty() {
                    let name = local(element.name().as_ref()).to_owned();
                    if matches!(
                        name.as_str(),
                        "twoCellAnchor" | "oneCellAnchor" | "absoluteAnchor"
                    ) {
                        inside = false;
                    }
                    continue;
                }
                path.pop();
                if path.is_empty()
                    && let Some((is_from, Some((index, index_span)), offset)) = corner.take()
                    && let Some(anchor) = found.last_mut()
                {
                    let marker = Marker {
                        index,
                        offset: offset.map_or(0, |(value, _)| value),
                        index_span,
                        offset_span: offset.map(|(_, span)| span),
                    };
                    if is_from {
                        anchor.from = Some(marker);
                    } else {
                        anchor.to = Some(marker);
                    }
                }
            }
            Event::Text(text) if inside && path.len() == 2 => {
                let Some((_, index, offset)) = corner.as_mut() else {
                    continue;
                };
                let Ok(value) = text.xml10_content().trim().parse::<i64>() else {
                    continue;
                };
                let name = path[1].as_str();
                if name == index_name {
                    *index = Some((value, (before, after)));
                } else if name == offset_name {
                    *offset = Some((value, (before, after)));
                }
            }
            Event::Eof => return found,
            _ => {}
        }
    }
}

/// The drawing part with its anchors moved past a shift, or `None` when none moves.
pub(crate) fn moved_drawing(xml: &str, shift: Shift) -> Option<String> {
    let mut edits: Vec<((usize, usize), String)> = Vec::new();
    for anchor in find_anchors(xml, shift.rows()) {
        let (Some(follows), Some(from)) = (anchor.follows, anchor.from) else {
            continue;
        };
        let Some((start, end)) = new_corners(shift, follows, from, anchor.to) else {
            continue;
        };
        let mut write = |marker: Marker, (index, offset): (i64, i64)| {
            if index != marker.index {
                edits.push((marker.index_span, index.to_string()));
            }
            if offset != marker.offset
                && let Some(span) = marker.offset_span
            {
                edits.push((span, offset.to_string()));
            }
        };
        write(from, start);
        if let (Some(to), Some(end)) = (anchor.to, end) {
            write(to, end);
        }
    }
    if edits.is_empty() {
        return None;
    }
    edits.sort_by_key(|(span, _)| std::cmp::Reverse(span.0));
    let mut written = xml.to_owned();
    for ((start, end), text) in edits {
        written.replace_range(start..end, &text);
    }
    Some(written)
}

/// Move the anchors of a sheet's drawing, its charts and pictures, with its rows or columns.
pub(crate) fn move_drawing_anchors(sheet: &mut Sheet, shift: Shift) {
    let Some(drawing) = sheet.drawing.as_mut() else {
        return;
    };
    let name = drawing.drawing_part.clone();
    let Some(part) = drawing.parts.iter_mut().find(|part| part.name == name) else {
        return;
    };
    if let Ok(xml) = std::str::from_utf8(&part.bytes)
        && let Some(moved) = moved_drawing(xml, shift)
    {
        part.bytes = moved.into_bytes();
    }
}

#[cfg(test)]
mod tests;
