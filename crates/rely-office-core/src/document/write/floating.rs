// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! The `wp:anchor` of a floating picture or text box, and the text box shape itself.

use std::fmt::Write as _;

use super::escape;
use crate::document::{Anchor, AnchorAlign, AnchorBase, AnchorPosition, TextFrame, Wrap};

/// Word stacks floating objects by `relativeHeight`; this is the base it writes itself.
const BASE_RELATIVE_HEIGHT: u32 = 251_658_240;

/// The opening of a `wp:anchor` up to and including its `wp:docPr`, for an object of
/// `cx` by `cy` English Metric Units.
pub(super) fn anchor_start(
    anchor: &Anchor,
    cx: i64,
    cy: i64,
    doc_properties: &str,
    id: u32,
) -> String {
    let [top, bottom, left, right] = anchor.distance_emu.map(|distance| distance.max(0));
    let behind = u8::from(anchor.wrap == Wrap::Behind);
    let wrap = match anchor.wrap {
        Wrap::Around => r#"<wp:wrapSquare wrapText="bothSides"/>"#.to_owned(),
        Wrap::Contour(outline) => {
            let mut polygon =
                String::from(r#"<wp:wrapTight wrapText="bothSides"><wp:wrapPolygon edited="1">"#);
            for (index, (x, y)) in outline.polygon().into_iter().enumerate() {
                let _ = write!(
                    polygon,
                    r#"<wp:{} x="{x}" y="{y}"/>"#,
                    if index == 0 { "start" } else { "lineTo" }
                );
            }
            polygon.push_str("</wp:wrapPolygon></wp:wrapTight>");
            polygon
        }
        Wrap::TopAndBottom => "<wp:wrapTopAndBottom/>".to_owned(),
        Wrap::InFront | Wrap::Behind => "<wp:wrapNone/>".to_owned(),
    };
    let mut xml = String::new();
    let _ = write!(
        xml,
        concat!(
            r#"<wp:anchor distT="{top}" distB="{bottom}" distL="{left}" distR="{right}" simplePos="0" "#,
            r#"relativeHeight="{height}" behindDoc="{behind}" locked="0" layoutInCell="1" allowOverlap="1">"#,
            r#"<wp:simplePos x="0" y="0"/>"#,
            r#"<wp:positionH relativeFrom="{h_base}">{h}</wp:positionH>"#,
            r#"<wp:positionV relativeFrom="{v_base}">{v}</wp:positionV>"#,
            r#"<wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/>{wrap}{doc}"#,
        ),
        top = top,
        bottom = bottom,
        left = left,
        right = right,
        height = BASE_RELATIVE_HEIGHT.saturating_add(id),
        behind = behind,
        h_base = horizontal_base(anchor.horizontal.base),
        h = position(&anchor.horizontal, false),
        v_base = vertical_base(anchor.vertical.base),
        v = position(&anchor.vertical, true),
        cx = cx.max(1),
        cy = cy.max(1),
        wrap = wrap,
        doc = doc_properties,
    );
    xml
}

/// A run holding a text box at its frame's anchor, with `content` (paragraphs and tables) as
/// its text.
pub(super) fn text_box_run(frame: &TextFrame, content: &str, id: u32) -> String {
    let (cx, cy) = (
        frame.anchor.width_emu.max(1),
        frame.anchor.height_emu.max(1),
    );
    let doc_properties = format!(r#"<wp:docPr id="{id}" name="Text Box {id}"/>"#);
    let fill = frame.fill.as_deref().map_or_else(
        || "<a:noFill/>".to_owned(),
        |colour| {
            format!(
                r#"<a:solidFill><a:srgbClr val="{}"/></a:solidFill>"#,
                escape(colour)
            )
        },
    );
    let line = frame.line.as_ref().map_or_else(
        || "<a:ln><a:noFill/></a:ln>".to_owned(),
        |(colour, eighths)| {
            format!(
                r#"<a:ln w="{}"><a:solidFill><a:srgbClr val="{}"/></a:solidFill></a:ln>"#,
                u64::from(*eighths) * 12_700 / 8,
                escape(colour)
            )
        },
    );
    let content = if content.is_empty() {
        "<w:p/>"
    } else {
        content
    };
    format!(
        concat!(
            "<w:r><w:drawing>{start}<wp:cNvGraphicFramePr/>",
            r#"<a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape">"#,
            r#"<wps:wsp><wps:cNvSpPr txBox="1"/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm>"#,
            r#"<a:prstGeom prst="{preset}"><a:avLst/></a:prstGeom>{fill}{line}</wps:spPr>"#,
            "<wps:txbx><w:txbxContent>{content}</w:txbxContent></wps:txbx>",
            r#"<wps:bodyPr rot="0" vert="horz" wrap="square" lIns="91440" tIns="45720" rIns="91440" bIns="45720" anchor="t"><a:noAutofit/></wps:bodyPr>"#,
            "</wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r>",
        ),
        start = anchor_start(&frame.anchor, cx, cy, &doc_properties, id),
        preset = frame
            .shape
            .as_deref()
            .filter(|preset| {
                !preset.is_empty()
                    && preset
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric())
            })
            .unwrap_or("rect"),
        cx = cx,
        cy = cy,
        fill = fill,
        line = line,
        content = content,
    )
}

fn horizontal_base(base: AnchorBase) -> &'static str {
    match base {
        AnchorBase::Page => "page",
        AnchorBase::Margin => "margin",
        AnchorBase::Column | AnchorBase::Paragraph => "column",
        AnchorBase::Line => "character",
    }
}

fn vertical_base(base: AnchorBase) -> &'static str {
    match base {
        AnchorBase::Page => "page",
        AnchorBase::Margin | AnchorBase::Column => "margin",
        AnchorBase::Paragraph => "paragraph",
        AnchorBase::Line => "line",
    }
}

fn position(position: &AnchorPosition, vertical: bool) -> String {
    match position.align {
        Some(align) => {
            let name = match (align, vertical) {
                (AnchorAlign::Start, false) => "left",
                (AnchorAlign::Start, true) => "top",
                (AnchorAlign::Center, _) => "center",
                (AnchorAlign::End, false) => "right",
                (AnchorAlign::End, true) => "bottom",
            };
            format!("<wp:align>{name}</wp:align>")
        }
        None => format!("<wp:posOffset>{}</wp:posOffset>", position.offset_emu),
    }
}
