// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use crate::Limits;
use crate::document::{Block, Inline, read_document_tree};
use crate::test_support::build_docx_from_body;

const WPS: &str = "http://schemas.microsoft.com/office/word/2010/wordprocessingShape";
const WPG: &str = "http://schemas.microsoft.com/office/word/2010/wordprocessingGroup";

fn drawing(uri: &str, body: &str) -> String {
    format!(
        concat!(
            r#"<w:r><w:drawing><wp:anchor behindDoc="0"><wp:positionH relativeFrom="column"><wp:posOffset>0</wp:posOffset></wp:positionH>"#,
            r#"<wp:positionV relativeFrom="paragraph"><wp:posOffset>0</wp:posOffset></wp:positionV><wp:extent cx="1270000" cy="635000"/><wp:wrapNone/>"#,
            r#"<a:graphic><a:graphicData uri="{}">{}</a:graphicData></a:graphic></wp:anchor></w:drawing></w:r>"#
        ),
        uri, body
    )
}

fn svgs(body: &str) -> (Vec<String>, usize, Vec<Inline>) {
    let docx = build_docx_from_body(&format!("<w:p>{body}</w:p>"), &[]);
    let tree = read_document_tree(&docx, &Limits::default()).unwrap();
    let svgs = tree
        .media
        .iter()
        .map(|media| String::from_utf8(media.bytes.clone()).unwrap())
        .collect();
    let Some(Block::Paragraph { content, .. }) = tree.blocks.first() else {
        panic!("a paragraph");
    };
    (svgs, tree.report.shapes, content.clone())
}

#[test]
fn draws_a_shape_with_its_fill_its_line_and_its_style() {
    let arrow = drawing(
        WPS,
        concat!(
            r#"<wps:wsp><wps:spPr><a:xfrm rot="5400000" flipH="1"/><a:prstGeom prst="rightArrow"><a:avLst/></a:prstGeom>"#,
            r#"<a:solidFill><a:srgbClr val="70AD47"/></a:solidFill><a:ln w="25400"><a:solidFill><a:srgbClr val="375623"/></a:solidFill></a:ln></wps:spPr>"#,
            r#"<wps:bodyPr/></wps:wsp>"#
        ),
    );
    let styled = drawing(
        WPS,
        concat!(
            r#"<wps:wsp><wps:spPr><a:custGeom><a:pathLst><a:path w="10" h="10"><a:moveTo><a:pt x="0" y="0"/></a:moveTo>"#,
            r#"<a:lnTo><a:pt x="10" y="10"/></a:lnTo></a:path></a:pathLst></a:custGeom><a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="ABCDEF"/></a:gs></a:gsLst></a:gradFill>"#,
            r#"<a:ln><a:noFill/></a:ln></wps:spPr><wps:style><a:lnRef idx="1"><a:srgbClr val="FF0000"/></a:lnRef><a:fillRef idx="0"/></wps:style><wps:bodyPr/></wps:wsp>"#
        ),
    );
    let by_style = drawing(
        WPS,
        concat!(
            r#"<wps:wsp><wps:spPr/><wps:style><a:lnRef idx="2"><a:srgbClr val="00FF00"/></a:lnRef><a:fillRef idx="1"><a:srgbClr val="0000FF"/></a:fillRef></wps:style>"#,
            r#"<wps:bodyPr/></wps:wsp>"#
        ),
    );
    let nothing = drawing(
        WPS,
        r#"<wps:wsp><wps:spPr><a:prstGeom prst="ellipse"/><a:noFill/><a:ln><a:noFill/></a:ln></wps:spPr><wps:bodyPr/></wps:wsp>"#,
    );
    let (svgs, shapes, content) = svgs(&[arrow, styled, by_style, nothing].concat());
    assert_eq!(svgs.len(), 3);
    assert_eq!(shapes, 1, "a shape with no fill and no line");
    assert!(svgs[0].contains("rotate(90 50 25)"), "{}", svgs[0]);
    assert!(
        svgs[0].contains(r##"fill="#70AD47""##)
            && svgs[0].contains(r##"stroke="#375623" stroke-width="2""##)
    );
    assert!(
        svgs[0].contains(r#"viewBox="-1 -1 102 52""#),
        "the line's half kept: {}",
        svgs[0]
    );
    assert!(
        svgs[1].contains(r##"fill="#ABCDEF""##) && !svgs[1].contains("stroke"),
        "{}",
        svgs[1]
    );
    assert!(svgs[2].contains(r##"fill="#0000FF""##) && svgs[2].contains(r##"stroke="#00FF00""##));
    assert!(
        svgs[2].contains(r#"d="M0 0L100 0L100 50L0 50Z""#),
        "no geometry is the frame"
    );
    let Inline::Image {
        width,
        height,
        anchor,
        ..
    } = &content[0]
    else {
        panic!("a picture");
    };
    assert_eq!((*width, *height), (Some(133), Some(67)));
    assert!(anchor.is_some());
}

#[test]
fn draws_a_group_of_shapes_with_their_text() {
    let group = drawing(
        WPG,
        concat!(
            r#"<wpg:wgp><wpg:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1270000" cy="635000"/><a:chOff x="0" y="0"/><a:chExt cx="2540000" cy="1270000"/></a:xfrm></wpg:grpSpPr>"#,
            r#"<wps:wsp><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1270000" cy="1270000"/></a:xfrm><a:prstGeom prst="rect"/><a:solidFill><a:srgbClr val="4472C4"/></a:solidFill></wps:spPr>"#,
            r#"<wps:style><a:fontRef idx="minor"><a:srgbClr val="FFFFFF"/></a:fontRef></wps:style>"#,
            r#"<wps:txbx><w:txbxContent><w:p><w:r><w:rPr><w:b/><w:i/><w:sz w:val="24"/></w:rPr><w:t>Inizio &amp; fine</w:t></w:r></w:p><w:p/>"#,
            r#"<w:p><w:r><w:rPr><w:color w:val="ff0000"/><w:b w:val="0"/></w:rPr><w:t>sotto</w:t></w:r></w:p></w:txbxContent></wps:txbx><wps:bodyPr/></wps:wsp>"#,
            r#"<wpg:grpSp><wpg:grpSpPr><a:xfrm><a:off x="1270000" y="0"/><a:ext cx="1270000" cy="1270000"/><a:chOff x="0" y="0"/><a:chExt cx="100" cy="100"/></a:xfrm></wpg:grpSpPr>"#,
            r#"<wps:wsp><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="100" cy="50"/></a:xfrm><a:prstGeom prst="ellipse"/><a:solidFill><a:srgbClr val="FFC000"/></a:solidFill></wps:spPr><wps:bodyPr/></wps:wsp>"#,
            r#"</wpg:grpSp><wps:wsp><wps:spPr><a:xfrm/></wps:spPr><wps:bodyPr/></wps:wsp></wpg:wgp>"#
        ),
    );
    let (svgs, shapes, _) = svgs(&group);
    assert_eq!((svgs.len(), shapes), (1, 0));
    let svg = &svgs[0];
    assert!(
        svg.contains(
            r##"<g transform="translate(0 0)"><path d="M0 0L50 0L50 50L0 50Z" fill="#4472C4""##
        ),
        "{svg}"
    );
    assert!(svg.contains(r##"font-size="12" fill="#FFFFFF" text-anchor="middle" font-weight="bold" font-style="italic" xml:space="preserve">Inizio &amp; fine</text>"##), "{svg}");
    assert!(
        svg.contains(r##"fill="#FF0000" text-anchor="middle" xml:space="preserve">sotto</text>"##),
        "{svg}"
    );
    assert!(
        svg.contains(r#"<g transform="translate(50 0)"><path d="M"#),
        "the nested group's own place: {svg}"
    );
    assert!(svg.contains(r##"fill="#FFC000""##));
    assert_eq!(
        svg.matches("<g ").count(),
        2,
        "a shape with no size draws nothing"
    );
}

const DIAGRAM: &str = "http://schemas.openxmlformats.org/drawingml/2006/diagram";
const DIAGRAM_DRAWING: &str = "http://schemas.microsoft.com/office/drawing/2008/diagram";

fn smart_art_docx(drawing_relationship: &str) -> Vec<u8> {
    let frame = format!(
        concat!(
            r#"<w:p><w:r><w:drawing><wp:inline><wp:extent cx="2540000" cy="1270000"/><a:graphic>"#,
            r#"<a:graphicData uri="{0}"><dgm:relIds xmlns:dgm="{0}" r:dm="rId1" r:lo="rId2" r:qs="rId3" r:cs="rId4"/></a:graphicData>"#,
            "</a:graphic></wp:inline></w:drawing></w:r></w:p>"
        ),
        DIAGRAM
    );
    let model = format!(
        concat!(
            r#"<dgm:dataModel xmlns:dgm="{0}" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><dgm:ptLst/>"#,
            r#"<dgm:extLst><a:ext uri="http://schemas.microsoft.com/office/drawing/2008/diagram"><dsp:dataModelExt xmlns:dsp="{1}" relId="{2}" minVer="x"/></a:ext></dgm:extLst></dgm:dataModel>"#
        ),
        DIAGRAM, DIAGRAM_DRAWING, drawing_relationship
    );
    let drawing = format!(
        concat!(
            r#"<dsp:drawing xmlns:dsp="{0}" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><dsp:spTree><dsp:nvGrpSpPr/><dsp:grpSpPr/>"#,
            r#"<dsp:sp modelId="1"><dsp:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1270000" cy="635000"/></a:xfrm><a:prstGeom prst="roundRect"/>"#,
            r#"<a:solidFill><a:srgbClr val="4472C4"/></a:solidFill></dsp:spPr><dsp:style><a:fontRef idx="minor"><a:srgbClr val="FFFFFF"/></a:fontRef></dsp:style>"#,
            r#"<dsp:txBody><a:bodyPr/><a:p><a:r><a:rPr sz="1400" b="1"/><a:t>Analisi</a:t></a:r></a:p></dsp:txBody></dsp:sp>"#,
            r#"<dsp:grpSp><dsp:grpSpPr><a:xfrm><a:off x="1270000" y="0"/><a:ext cx="1270000" cy="635000"/><a:chOff x="0" y="0"/><a:chExt cx="100" cy="50"/></a:xfrm></dsp:grpSpPr>"#,
            r#"<dsp:sp><dsp:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="100" cy="50"/></a:xfrm><a:prstGeom prst="rightArrow"/><a:solidFill><a:srgbClr val="A5A5A5"/></a:solidFill></dsp:spPr></dsp:sp>"#,
            "</dsp:grpSp></dsp:spTree></dsp:drawing>"
        ),
        DIAGRAM_DRAWING
    );
    let relationships = crate::test_support::rels_xml(&[
        ("rId1", "diagramData", "diagrams/data1.xml", false),
        ("rId5", "diagramDrawing", "diagrams/drawing1.xml", false),
    ]);
    build_docx_from_body(
        &frame,
        &[
            ("word/_rels/document.xml.rels", relationships.as_bytes()),
            ("word/diagrams/data1.xml", model.as_bytes()),
            ("word/diagrams/drawing1.xml", drawing.as_bytes()),
        ],
    )
}

#[test]
fn draws_a_smart_art_from_its_drawing() {
    for relationship in ["rId5", "rId9"] {
        let tree = read_document_tree(&smart_art_docx(relationship), &Limits::default()).unwrap();
        assert_eq!(tree.report.smart_art, 1);
        assert_eq!(
            tree.media.len(),
            1,
            "{relationship}: the only drawing is the one"
        );
        let svg = String::from_utf8(tree.media[0].bytes.clone()).unwrap();
        assert!(
            svg.contains(r##"fill="#4472C4""##) && svg.contains(">Analisi</text>"),
            "{svg}"
        );
        assert!(svg.contains(r#"font-weight="bold""#) && svg.contains(r##"fill="#FFFFFF""##));
        assert!(
            svg.contains(r##"fill="#A5A5A5""##),
            "the grouped arrow: {svg}"
        );
        let Some(Block::Paragraph { content, .. }) = tree.blocks.first() else {
            panic!("a paragraph");
        };
        assert!(matches!(
            content[0],
            Inline::Image {
                width: Some(267),
                height: Some(133),
                ..
            }
        ));
    }
}

#[test]
fn finds_the_drawing_a_data_model_names() {
    assert_eq!(
        super::smart_art_drawing(r#"<x><dsp:dataModelExt relId="rId7" minVer="m"/></x>"#),
        Some("rId7")
    );
    assert_eq!(
        super::smart_art_drawing(r#"<dsp:dataModelExt relId="a\"b"/>"#),
        None
    );
    assert_eq!(super::smart_art_drawing("<nothing/>"), None);
}
