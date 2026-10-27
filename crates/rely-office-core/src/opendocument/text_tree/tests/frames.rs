// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! Where odt frames stand, and the frame a text box draws.

use super::*;
use crate::document::{Anchor, AnchorAlign, AnchorBase, TextFrame, Wrap};

fn picture_tree(frames: &str) -> DocumentTree {
    let body = format!("<text:p>{frames}</text:p>");
    read_document_tree(
        &odt(&body, &[("Pictures/logo.png", PNG)]),
        &Limits::default(),
    )
    .unwrap()
}

fn anchors(tree: &DocumentTree) -> Vec<Option<Anchor>> {
    let (content, _, _) = paragraph(&tree.blocks[0]);
    content
        .iter()
        .filter_map(|inline| match inline {
            Inline::Image { anchor, .. } => Some(anchor.as_deref().copied()),
            _ => None,
        })
        .collect()
}

fn frames(blocks: &[Block]) -> Vec<Option<TextFrame>> {
    blocks
        .iter()
        .map(|block| match block {
            Block::Paragraph { look, .. } => look.frame.clone(),
            _ => None,
        })
        .collect()
}

fn image(attributes: &str) -> String {
    format!(
        r#"<draw:frame {attributes} svg:width="2cm" svg:height="1cm"><draw:image xlink:href="Pictures/logo.png"/></draw:frame>"#
    )
}

#[test]
fn has_no_anchor_for_a_frame_set_as_a_character() {
    let tree = picture_tree(&format!(
        "{}{}{}",
        image(r#"text:anchor-type="as-char""#),
        image(""),
        image(r#"text:anchor-type="sideways""#)
    ));
    assert_eq!(anchors(&tree), vec![None, None, None]);
}

#[test]
fn reads_the_position_and_wrapping_of_a_floating_picture() {
    let tree = picture_tree(&format!(
        "{}{}",
        image(r#"text:anchor-type="paragraph" draw:style-name="fr3" svg:x="1in" svg:y="0.5in""#),
        image(r#"text:anchor-type="page" svg:x="2in" svg:y="3in""#),
    ));
    let anchors = anchors(&tree);
    let around = anchors[0].expect("an anchor");
    assert_eq!(around.wrap, Wrap::Around);
    assert_eq!(around.horizontal.base, AnchorBase::Column);
    assert_eq!(around.horizontal.align, Some(AnchorAlign::End));
    assert_eq!(around.horizontal.offset_emu, 914_400);
    assert_eq!(around.vertical.base, AnchorBase::Paragraph);
    assert_eq!(around.vertical.align, None);
    assert_eq!(around.vertical.offset_emu, 457_200);
    assert_eq!(around.width_emu, 720_000);
    assert_eq!(around.height_emu, 360_000);
    assert_eq!(around.distance_emu, [0, 0, 91_440, 0]);
    let page = anchors[1].expect("an anchor");
    assert_eq!(page.wrap, Wrap::TopAndBottom);
    assert_eq!(page.horizontal.base, AnchorBase::Page);
    assert_eq!(page.vertical.base, AnchorBase::Page);
    assert_eq!(page.horizontal.offset_emu, 1_828_800);
    assert_eq!(page.vertical.offset_emu, 2_743_200);
}

#[test]
fn reads_the_bases_and_layers_of_a_frame_style() {
    let tree = picture_tree(&format!(
        "{}{}{}",
        image(r#"text:anchor-type="char" draw:style-name="gr1""#),
        image(r#"text:anchor-type="frame" draw:style-name="fr2""#),
        image(r#"text:anchor-type="char" draw:style-name="fr4""#),
    ));
    let anchors = anchors(&tree);
    let front = anchors[0].expect("an anchor");
    assert_eq!(front.wrap, Wrap::InFront);
    assert_eq!(front.horizontal.base, AnchorBase::Margin);
    assert_eq!(front.vertical.base, AnchorBase::Line);
    let behind = anchors[1].expect("an anchor");
    assert_eq!(behind.wrap, Wrap::Behind);
    assert_eq!(behind.horizontal.base, AnchorBase::Page);
    assert_eq!(behind.vertical.base, AnchorBase::Margin);
    assert_eq!(behind.vertical.align, Some(AnchorAlign::Center));
    let content = anchors[2].expect("an anchor");
    assert_eq!(content.vertical.base, AnchorBase::Paragraph);
    assert_eq!(content.horizontal.base, AnchorBase::Column);
}

#[test]
fn frames_a_text_box_with_the_fill_and_outline_of_its_style() {
    let body = concat!(
        r#"<text:p>Prima<draw:frame text:anchor-type="paragraph" draw:style-name="gr1" svg:width="5cm" fo:min-height="2cm">"#,
        r#"<draw:text-box><text:p>uno</text:p></draw:text-box></draw:frame>"#,
        r#"<draw:frame text:anchor-type="paragraph" draw:style-name="fr2" svg:width="5cm"><draw:text-box><text:p>due</text:p></draw:text-box></draw:frame>"#,
        r#"<draw:frame text:anchor-type="paragraph" draw:style-name="Fr" svg:width="5cm"><draw:text-box><text:p>tre</text:p></draw:text-box></draw:frame>"#,
        r#"<draw:frame text:anchor-type="paragraph" draw:style-name="fr4" svg:width="5cm"><draw:text-box><text:p>quattro</text:p></draw:text-box></draw:frame>"#,
        r#"<draw:frame text:anchor-type="as-char"><draw:text-box><text:p>in riga</text:p></draw:text-box></draw:frame>"#,
        r#"<draw:custom-shape text:anchor-type="paragraph" draw:style-name="gr1" svg:width="3cm" svg:height="1cm"><text:p>forma</text:p></draw:custom-shape>"#,
        "</text:p>"
    );
    let frames = frames(&tree(body).blocks);
    assert_eq!(frames.len(), 7);
    assert_eq!(frames[0], None);
    let first = frames[1].clone().expect("a frame");
    assert_eq!(first.id, 1);
    assert_eq!(first.fill.as_deref(), Some("FFF2CC"));
    assert_eq!(first.line, Some(("BF9000".to_owned(), 16)));
    assert_eq!(first.anchor.width_emu, 1_800_000);
    assert_eq!(first.anchor.height_emu, 720_000);
    let second = frames[2].clone().expect("a frame");
    assert_eq!((second.id, second.fill, second.line), (2, None, None));
    let third = frames[3].clone().expect("a frame");
    assert_eq!(third.fill, None);
    assert_eq!(third.line, Some(("00FF00".to_owned(), 4)));
    let fourth = frames[4].clone().expect("a frame");
    assert_eq!(fourth.fill.as_deref(), Some("336699"));
    assert_eq!(fourth.line, Some(("000000".to_owned(), 6)));
    assert_eq!(frames[5], None);
    let shape = frames[6].clone().expect("a frame");
    assert_eq!(shape.id, 5);
    assert_eq!(shape.anchor.height_emu, 360_000);
}
