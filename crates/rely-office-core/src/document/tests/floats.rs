// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! Floating pictures and text boxes: where they stand and the frame a text box draws.

use super::*;

fn frames(blocks: &[Block]) -> Vec<Option<TextFrame>> {
    blocks
        .iter()
        .map(|block| match block {
            Block::Paragraph { look, .. } => look.frame.clone(),
            _ => None,
        })
        .collect()
}

fn text_box(shape_properties: &str, value: &str) -> String {
    format!(
        concat!(
            r#"<w:r><w:drawing><wp:anchor behindDoc="1"><wp:positionH relativeFrom="page"><wp:posOffset>127000</wp:posOffset></wp:positionH>"#,
            r#"<wp:extent cx="1270000" cy="635000"/><wp:wrapNone/><a:graphic>"#,
            r#"<a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape">"#,
            r#"<wps:wsp><wps:spPr>{}</wps:spPr><wps:txbx><w:txbxContent><w:p><w:r><w:t>{}</w:t></w:r></w:p>"#,
            r#"</w:txbxContent></wps:txbx></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r>"#
        ),
        shape_properties, value
    )
}

#[test]
fn keeps_the_anchor_of_a_floating_picture() {
    let rels = rels_xml(&[("rId5", "image", "media/image1.png", false)]);
    let body = concat!(
        r#"<w:p><w:r><w:drawing><wp:anchor distL="114300"><wp:positionH relativeFrom="margin"><wp:align>center</wp:align></wp:positionH>"#,
        r#"<wp:positionV relativeFrom="page"><wp:posOffset>254000</wp:posOffset></wp:positionV>"#,
        r#"<wp:extent cx="952500" cy="476250"/><wp:wrapTopAndBottom/><wp:docPr id="1" name="Foto"/><a:graphic>"#,
        r#"<a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">"#,
        r#"<pic:pic><pic:blipFill><a:blip r:embed="rId5"/></pic:blipFill></pic:pic>"#,
        r#"</a:graphicData></a:graphic></wp:anchor></w:drawing></w:r></w:p>"#
    );
    let read = tree_with(
        body,
        &[(RELS_PART, rels.as_bytes()), ("word/media/image1.png", PNG)],
    );
    let Block::Paragraph { content, .. } = &read.blocks[0] else {
        panic!("a paragraph");
    };
    assert_eq!(
        content,
        &vec![Inline::Image {
            media: 0,
            alt: None,
            width: Some(100),
            height: Some(50),
            anchor: Some(Box::new(Anchor {
                horizontal: AnchorPosition {
                    base: AnchorBase::Margin,
                    offset_emu: 0,
                    align: Some(AnchorAlign::Center),
                },
                vertical: AnchorPosition {
                    base: AnchorBase::Page,
                    offset_emu: 254_000,
                    align: None,
                },
                width_emu: 952_500,
                height_emu: 476_250,
                wrap: Wrap::TopAndBottom,
                distance_emu: [0, 0, 114_300, 0],
            })),
        }]
    );
}

#[test]
fn frames_each_drawingml_text_box_with_its_fill_and_outline() {
    let body = format!(
        "<w:p><w:r><w:t>prima</w:t></w:r>{}{}{}</w:p>",
        text_box(
            r#"<a:solidFill><a:srgbClr val="fff2cc"/></a:solidFill><a:ln w="25400"><a:solidFill><a:srgbClr val="BF9000"/></a:solidFill></a:ln>"#,
            "uno"
        ),
        text_box(
            r#"<a:noFill/><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill><a:ln><a:noFill/></a:ln>"#,
            "due"
        ),
        text_box(
            r#"<a:ln><a:solidFill><a:srgbClr val="000000"/></a:solidFill></a:ln>"#,
            "tre"
        ),
    );
    let read = tree(&body);
    let frames = frames(&read.blocks);
    assert_eq!(frames.len(), 4);
    assert_eq!(frames[0], None);
    let anchor = Anchor {
        horizontal: AnchorPosition {
            base: AnchorBase::Page,
            offset_emu: 127_000,
            align: None,
        },
        vertical: AnchorPosition {
            base: AnchorBase::Paragraph,
            offset_emu: 0,
            align: None,
        },
        width_emu: 1_270_000,
        height_emu: 635_000,
        wrap: Wrap::Behind,
        distance_emu: [0; 4],
    };
    assert_eq!(
        frames[1],
        Some(TextFrame {
            shape: None,
            id: 1,
            anchor,
            fill: Some("FFF2CC".to_owned()),
            line: Some(("BF9000".to_owned(), 16)),
        })
    );
    assert_eq!(
        frames[2],
        Some(TextFrame {
            shape: None,
            id: 2,
            anchor,
            fill: None,
            line: None,
        })
    );
    assert_eq!(
        frames[3],
        Some(TextFrame {
            shape: None,
            id: 3,
            anchor,
            fill: None,
            line: Some(("000000".to_owned(), 6)),
        })
    );
    assert_eq!(read.report.text_boxes, 3);
}

#[test]
fn frames_an_inline_text_box_with_nothing() {
    let body = concat!(
        r#"<w:p><w:r><w:drawing><wp:inline><a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape">"#,
        r#"<wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>in riga</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp>"#,
        r#"</a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#
    );
    assert_eq!(frames(&tree(body).blocks), vec![None, None]);
}

#[test]
fn frames_a_vml_text_box_from_its_shape() {
    let shape = |attributes: &str, style: &str| {
        format!(
            r#"<w:r><w:pict><v:shape style="{style}" {attributes}><v:textbox><w:txbxContent><w:p><w:r><w:t>vml</w:t></w:r></w:p></w:txbxContent></v:textbox></v:shape></w:pict></w:r>"#
        )
    };
    let absolute = "position:absolute;margin-left:10pt;margin-top:20pt;width:100pt;height:50pt";
    let body = format!(
        "<w:p>{}{}{}{}</w:p>",
        shape(
            r##"fillcolor="#ffcc00" strokecolor="#336699" strokeweight="2pt""##,
            absolute
        ),
        shape(r#"filled="f" stroked="false""#, absolute),
        shape(r#"fillcolor="giallo" strokeweight="nonsense""#, absolute),
        shape("", "width:100pt;height:50pt"),
    );
    let read = tree(&body);
    let frames = frames(&read.blocks);
    assert_eq!(frames.len(), 5);
    let first = frames[1].clone().expect("a frame");
    assert_eq!(first.id, 1);
    assert_eq!(first.fill.as_deref(), Some("FFCC00"));
    assert_eq!(first.line, Some(("336699".to_owned(), 16)));
    assert_eq!(first.anchor.horizontal.offset_emu, 127_000);
    assert_eq!(first.anchor.vertical.offset_emu, 254_000);
    assert_eq!(first.anchor.wrap, Wrap::Around);
    let second = frames[2].clone().expect("a frame");
    assert_eq!((second.id, second.fill, second.line), (2, None, None));
    let third = frames[3].clone().expect("a frame");
    assert_eq!(third.fill, None);
    assert_eq!(third.line, Some(("000000".to_owned(), 6)));
    assert_eq!(frames[4], None);
}

#[test]
fn keeps_the_anchor_of_a_floating_vml_picture() {
    let rels = rels_xml(&[("rId6", "image", "media/image1.png", false)]);
    let body = concat!(
        r#"<w:p><w:r><w:pict><v:shape style="position:absolute;left:5pt;width:75pt;height:30pt;mso-position-vertical-relative:page;z-index:-1">"#,
        r#"<v:imagedata r:id="rId6"/></v:shape></w:pict></w:r></w:p>"#
    );
    let read = tree_with(
        body,
        &[(RELS_PART, rels.as_bytes()), ("word/media/image1.png", PNG)],
    );
    let Block::Paragraph { content, .. } = &read.blocks[0] else {
        panic!("a paragraph");
    };
    let Inline::Image {
        anchor: Some(anchor),
        ..
    } = &content[0]
    else {
        panic!("a floating image");
    };
    assert_eq!(anchor.horizontal.offset_emu, 63_500);
    assert_eq!(anchor.vertical.base, AnchorBase::Page);
    assert_eq!(anchor.wrap, Wrap::Behind);
}

#[cfg(feature = "serde")]
#[test]
fn a_floating_image_crosses_the_json_boundary_with_its_anchor() {
    let image = Inline::Image {
        media: 0,
        alt: None,
        width: Some(100),
        height: Some(50),
        anchor: Some(Box::new(Anchor {
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
            width_emu: 952_500,
            height_emu: 476_250,
            wrap: Wrap::Contour(WrapOutline {
                left: [0; OUTLINE_BANDS],
                right: [255; OUTLINE_BANDS],
            }),
            distance_emu: [0, 0, 114_300, 114_300],
        })),
    };
    let json = serde_json::to_value(&image).unwrap();
    assert_eq!(
        json["anchor"]["horizontal"],
        serde_json::json!({ "base": "column", "offsetEmu": 0, "align": "end" })
    );
    assert_eq!(
        json["anchor"]["vertical"],
        serde_json::json!({ "base": "paragraph", "offsetEmu": 12_700 })
    );
    assert_eq!(json["anchor"]["wrap"]["type"], "contour");
    assert_eq!(json["anchor"]["widthEmu"], 952_500);
    assert_eq!(serde_json::from_value::<Inline>(json).unwrap(), image);
    let around: Inline = serde_json::from_value(serde_json::json!({
        "type": "image",
        "media": 0,
        "anchor": {
            "horizontal": { "base": "page", "offsetEmu": 0 },
            "vertical": { "base": "page", "offsetEmu": 0 },
            "widthEmu": 10, "heightEmu": 10,
            "wrap": { "type": "around" },
            "distanceEmu": [0, 0, 0, 0]
        }
    }))
    .unwrap();
    assert!(
        matches!(around, Inline::Image { anchor: Some(anchor), .. } if anchor.wrap == Wrap::Around)
    );
    // An image the editor set in the line has no anchor at all.
    let inline = serde_json::to_value(Inline::Image {
        media: 0,
        alt: None,
        width: None,
        height: None,
        anchor: None,
    })
    .unwrap();
    assert!(inline.get("anchor").is_none());
}
