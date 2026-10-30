// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use super::*;
use crate::document::{Inline, ParagraphLook, TableLook};
use crate::xml_tree::parse_tree;

const WP_URI: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";

/// Parse a `wp:` fragment, its root declaring the namespace.
fn element(xml: &str) -> Element {
    let name_end = xml
        .find(|character: char| character.is_whitespace() || character == '>' || character == '/')
        .expect("a root element");
    let declared = format!(
        "{} xmlns:wp=\"{WP_URI}\"{}",
        &xml[..name_end],
        &xml[name_end..]
    );
    parse_tree(&declared, |_, _| false).expect("the fragment parses")
}

fn frame(id: u32) -> TextFrame {
    TextFrame {
        shape: None,
        id,
        anchor: Anchor::from_vml_style("position:absolute;width:10pt;height:10pt")
            .expect("anchored"),
        fill: None,
        line: None,
    }
}

#[test]
fn reads_an_anchor_aligned_on_the_column_and_offset_on_the_paragraph() {
    let anchor = Anchor::parse(&element(concat!(
        r#"<wp:anchor distT="10" distB="20" distL="114300" distR="-5" behindDoc="0">"#,
        r#"<wp:positionH relativeFrom="column"><wp:align>right</wp:align></wp:positionH>"#,
        r#"<wp:positionV relativeFrom="paragraph"><wp:posOffset>12700</wp:posOffset></wp:positionV>"#,
        r#"<wp:extent cx="914400" cy="457200"/><wp:wrapSquare wrapText="bothSides"/></wp:anchor>"#
    )))
    .expect("an anchor");
    assert_eq!(
        anchor,
        Anchor {
            horizontal: AnchorPosition {
                base: AnchorBase::Column,
                offset_emu: 0,
                align: Some(AnchorAlign::End),
            },
            vertical: AnchorPosition {
                base: AnchorBase::Paragraph,
                offset_emu: 12_700,
                align: None,
            },
            width_emu: 914_400,
            height_emu: 457_200,
            wrap: Wrap::Around,
            distance_emu: [10, 20, 114_300, 0],
        }
    );
}

#[test]
fn reads_each_base_and_alignment_of_an_anchor() {
    let position = |relative: &str, inner: &str, vertical: bool| {
        let name = if vertical { "positionV" } else { "positionH" };
        let anchor = Anchor::parse(&element(&format!(
            r#"<wp:anchor><wp:{name} relativeFrom="{relative}">{inner}</wp:{name}></wp:anchor>"#
        )))
        .expect("an anchor");
        if vertical {
            anchor.vertical
        } else {
            anchor.horizontal
        }
    };
    assert_eq!(position("page", "", false).base, AnchorBase::Page);
    assert_eq!(position("leftMargin", "", false).base, AnchorBase::Margin);
    assert_eq!(position("bottomMargin", "", true).base, AnchorBase::Margin);
    assert_eq!(position("paragraph", "", false).base, AnchorBase::Paragraph);
    assert_eq!(position("character", "", false).base, AnchorBase::Line);
    assert_eq!(position("line", "", true).base, AnchorBase::Line);
    assert_eq!(position("insideMargin", "", true).base, AnchorBase::Margin);
    assert_eq!(position("text", "", true).base, AnchorBase::Paragraph);
    assert_eq!(position("text", "", false).base, AnchorBase::Column);
    let align =
        |value: &str| position("page", &format!("<wp:align>{value}</wp:align>"), false).align;
    assert_eq!(align("left"), Some(AnchorAlign::Start));
    assert_eq!(align("inside"), Some(AnchorAlign::Start));
    assert_eq!(align("center"), Some(AnchorAlign::Center));
    assert_eq!(align("outside"), Some(AnchorAlign::End));
    assert_eq!(align("bottom"), Some(AnchorAlign::End));
    assert_eq!(align("diagonal"), None);
    let offset = position("page", "<wp:posOffset>not a number</wp:posOffset>", true);
    assert_eq!(offset.offset_emu, 0);
    let huge = position("page", "<wp:posOffset>99999999999</wp:posOffset>", true);
    assert_eq!(huge.offset_emu, MAX_EMU);
}

#[test]
fn reads_the_wrapping_of_an_anchor() {
    let wrap = |attributes: &str, inner: &str| {
        Anchor::parse(&element(&format!(
            "<wp:anchor {attributes}>{inner}</wp:anchor>"
        )))
        .expect("an anchor")
        .wrap
    };
    assert_eq!(wrap("", "<wp:wrapTopAndBottom/>"), Wrap::TopAndBottom);
    assert_eq!(wrap("", "<wp:wrapTight/>"), Wrap::Around);
    assert_eq!(wrap("", "<wp:wrapThrough/>"), Wrap::Around);
    assert_eq!(wrap(r#"behindDoc="1""#, "<wp:wrapNone/>"), Wrap::Behind);
    assert_eq!(wrap(r#"behindDoc="true""#, ""), Wrap::Behind);
    assert_eq!(wrap(r#"behindDoc="0""#, "<wp:wrapNone/>"), Wrap::InFront);
}

#[test]
fn has_no_anchor_for_an_inline_object() {
    assert_eq!(Anchor::parse(&element("<wp:inline/>")), None);
}

#[test]
fn reads_the_anchor_of_a_vml_shape() {
    let anchor = Anchor::from_vml_style(concat!(
        "position:absolute; margin-left:100pt; margin-top:1in; width:2cm; height:96px; ",
        "mso-position-horizontal-relative:page; mso-position-vertical-relative:margin; ",
        "mso-position-horizontal:center; z-index:-2"
    ))
    .expect("an anchor");
    assert_eq!(
        anchor,
        Anchor {
            horizontal: AnchorPosition {
                base: AnchorBase::Page,
                offset_emu: 1_270_000,
                align: Some(AnchorAlign::Center),
            },
            vertical: AnchorPosition {
                base: AnchorBase::Margin,
                offset_emu: 914_400,
                align: None,
            },
            width_emu: 720_000,
            height_emu: 914_400,
            wrap: Wrap::Behind,
            distance_emu: [0; 4],
        }
    );
}

#[test]
fn reads_the_bases_and_alignments_of_a_vml_shape() {
    let anchor = |style: &str| {
        Anchor::from_vml_style(&format!("position:absolute;{style}")).expect("an anchor")
    };
    let shape = anchor(
        "mso-position-horizontal-relative:char;mso-position-vertical-relative:text;mso-position-horizontal:right;mso-position-vertical:top;left:10pt;top:5pt",
    );
    assert_eq!(shape.horizontal.base, AnchorBase::Line);
    assert_eq!(shape.horizontal.align, Some(AnchorAlign::End));
    assert_eq!(shape.horizontal.offset_emu, 127_000);
    assert_eq!(shape.vertical.base, AnchorBase::Paragraph);
    assert_eq!(shape.vertical.align, Some(AnchorAlign::Start));
    assert_eq!(shape.vertical.offset_emu, 63_500);
    assert_eq!(shape.wrap, Wrap::Around);
    let plain = anchor(
        "mso-position-horizontal-relative:margin;mso-position-vertical-relative:line;mso-position-horizontal:inside;mso-position-vertical:outside",
    );
    assert_eq!(plain.horizontal.base, AnchorBase::Margin);
    assert_eq!(plain.vertical.base, AnchorBase::Line);
    assert_eq!(plain.horizontal.align, Some(AnchorAlign::Start));
    assert_eq!(plain.vertical.align, Some(AnchorAlign::End));
    let fallback = anchor("mso-position-horizontal:middle;width:-4pt");
    assert_eq!(fallback.horizontal.base, AnchorBase::Column);
    assert_eq!(fallback.vertical.base, AnchorBase::Paragraph);
    assert_eq!(fallback.horizontal.align, None);
    assert_eq!(fallback.width_emu, 0);
    assert_eq!(
        anchor("mso-position-vertical:center").vertical.align,
        Some(AnchorAlign::Center)
    );
}

#[test]
fn has_no_anchor_for_a_vml_shape_in_the_line() {
    assert_eq!(Anchor::from_vml_style("width:10pt;height:10pt"), None);
    assert_eq!(Anchor::from_vml_style("position:relative;width:10pt"), None);
    assert_eq!(Anchor::from_vml_style(""), None);
}

#[test]
fn converts_css_lengths_to_english_metric_units() {
    assert_eq!(css_emu("12pt"), Some(152_400));
    assert_eq!(css_emu(" 1in "), Some(914_400));
    assert_eq!(css_emu("2.54cm"), Some(914_400));
    assert_eq!(css_emu("25.4mm"), Some(914_400));
    assert_eq!(css_emu("96px"), Some(914_400));
    assert_eq!(css_emu("6pc"), Some(914_400));
    assert_eq!(css_emu("10"), Some(127_000));
    assert_eq!(css_emu("-1in"), Some(-914_400));
    assert_eq!(css_emu("100000in"), Some(MAX_EMU));
    assert_eq!(css_emu("3em"), None);
    assert_eq!(css_emu("pt"), None);
    assert_eq!(css_emu("inf"), None);
}

#[test]
fn puts_every_block_of_a_text_box_in_its_frame() {
    let paragraph = Block::Paragraph {
        content: vec![Inline::HardBreak],
        align: None,
        look: ParagraphLook::default(),
    };
    let mut blocks = vec![
        paragraph.clone(),
        Block::Heading {
            level: 1,
            content: Vec::new(),
            align: None,
            look: ParagraphLook::default(),
        },
        Block::ListItem {
            ordered: false,
            level: 0,
            list_id: 0,
            content: Vec::new(),
            look: ParagraphLook::default(),
        },
        Block::Table {
            rows: Vec::new(),
            look: TableLook::default(),
        },
        Block::Blockquote {
            blocks: vec![paragraph],
        },
        Block::CodeBlock {
            text: "x".to_owned(),
        },
        Block::HorizontalRule,
    ];
    set_frame(&mut blocks, &frame(7));
    let framed = |block: &Block| match block {
        Block::Paragraph { look, .. }
        | Block::Heading { look, .. }
        | Block::ListItem { look, .. } => look.frame.as_ref().map(|frame| frame.id),
        Block::Table { look, .. } => look.frame.as_ref().map(|frame| frame.id),
        Block::Blockquote { blocks } => match &blocks[0] {
            Block::Paragraph { look, .. } => look.frame.as_ref().map(|frame| frame.id),
            _ => None,
        },
        Block::CodeBlock { .. } | Block::HorizontalRule => None,
    };
    assert_eq!(
        blocks.iter().map(framed).collect::<Vec<_>>(),
        vec![Some(7), Some(7), Some(7), Some(7), Some(7), None, None]
    );
}

#[test]
fn reads_the_outline_a_tight_wrap_follows() {
    let wrap = |inner: &str| {
        Anchor::parse(&element(&format!("<wp:anchor>{inner}</wp:anchor>")))
            .expect("an anchor")
            .wrap
    };
    let diamond = concat!(
        r#"<wp:wrapTight wrapText="bothSides"><wp:wrapPolygon edited="0"><wp:start x="10800" y="0"/>"#,
        r#"<wp:lineTo x="21600" y="10800"/><wp:lineTo x="10800" y="21600"/><wp:lineTo x="0" y="10800"/>"#,
        r#"<wp:lineTo x="10800" y="0"/><wp:lineTo x="bad" y="1"/></wp:wrapPolygon></wp:wrapTight>"#
    );
    let Wrap::Contour(outline) = wrap(diamond) else {
        panic!("a contour");
    };
    // Narrow at the top and the bottom, as wide as the object across the middle.
    assert_eq!((outline.left[0], outline.right[0]), (112, 143));
    assert_eq!((outline.left[7], outline.right[7]), (0, 255));
    assert_eq!((outline.left[6], outline.right[6]), (16, 239));
    assert_eq!((outline.left[8], outline.right[8]), (0, 255));
    assert_eq!((outline.left[15], outline.right[15]), (112, 143));
    assert_eq!(
        wrap(
            r#"<wp:wrapThrough><wp:wrapPolygon><wp:start x="0" y="0"/><wp:lineTo x="5" y="5"/></wp:wrapPolygon></wp:wrapThrough>"#
        ),
        Wrap::Around,
        "a polygon of two points wraps as a rectangle"
    );
    let points = outline.polygon();
    assert_eq!(points.len(), OUTLINE_BANDS * 4);
    assert_eq!(points[0], (i64::from(outline.right[0]) * 21_600 / 255, 0));
    assert_eq!(
        WrapOutline::from_polygon(
            &points
                .iter()
                .map(|(x, y)| (
                    f64::from(i32::try_from(*x).unwrap()),
                    f64::from(i32::try_from(*y).unwrap())
                ))
                .collect::<Vec<_>>()
        )
        .map(|again| again.right[7]),
        Some(outline.right[7]),
        "the outline written back reads the same"
    );
    let half = WrapOutline::from_polygon(&[
        (0.0, 0.0),
        (21_600.0, 0.0),
        (21_600.0, 10_000.0),
        (0.0, 10_000.0),
    ])
    .unwrap();
    assert!(
        half.left[15] > half.right[15],
        "a band the outline leaves empty"
    );
    assert_eq!(half.polygon().len(), 8 * 4);
}
