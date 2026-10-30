// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use std::fmt::Write as _;

use super::*;
use crate::geometry::{PathCommand, PathPoint};
use crate::test_support::{build_docx, build_zip};

const NS: &str = concat!(
    r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
    r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#,
    r#"xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" "#,
    r#"xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006""#
);
const PNG: &[u8] = &[
    0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0,
    0, 20, 0, 0, 0, 10,
];

fn relationships(items: &[(&str, &str, &str)]) -> String {
    let mut body = String::new();
    for (id, kind, target) in items {
        let _ = write!(
            body,
            r#"<Relationship Id="{id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/{kind}" Target="{target}"/>"#
        );
    }
    format!(
        r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{body}</Relationships>"#
    )
}

fn tree(shapes: &str) -> String {
    format!(
        r#"<p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>{shapes}</p:spTree>"#
    )
}

fn placeholder(id: u32, ph: &str, frame: Option<(i64, i64, i64, i64)>, body: &str) -> String {
    let frame = frame.map_or_else(String::new, |(x, y, width, height)| {
        format!(r#"<a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{width}" cy="{height}"/></a:xfrm>"#)
    });
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="P{id}"/><p:cNvSpPr/><p:nvPr>{ph}</p:nvPr></p:nvSpPr><p:spPr>{frame}</p:spPr>{body}</p:sp>"#
    )
}

fn text(paragraphs: &str) -> String {
    format!("<p:txBody><a:bodyPr/><a:lstStyle/>{paragraphs}</p:txBody>")
}

fn master() -> String {
    format!(
        concat!(
            r#"<p:sldMaster {}><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg2"/></p:bgRef></p:bg>{}</p:cSld>"#,
            r#"<p:txStyles><p:titleStyle><a:lvl1pPr algn="ctr"><a:defRPr sz="4400"><a:solidFill><a:schemeClr val="tx2"/></a:solidFill>"#,
            r#"<a:latin typeface="+mj-lt"/></a:defRPr></a:lvl1pPr></p:titleStyle>"#,
            r#"<p:bodyStyle><a:lvl1pPr marL="228600" indent="-228600"><a:spcBef><a:spcPts val="1000"/></a:spcBef>"#,
            r#"<a:buChar char="&#x2022;"/><a:defRPr sz="2800"/></a:lvl1pPr>"#,
            r#"<a:lvl2pPr marL="685800" indent="-228600"><a:buAutoNum type="arabicPeriod"/><a:defRPr sz="2400"/></a:lvl2pPr></p:bodyStyle>"#,
            r#"<p:otherStyle><a:lvl1pPr><a:defRPr sz="1400"/></a:lvl1pPr></p:otherStyle></p:txStyles></p:sldMaster>"#
        ),
        NS,
        tree(&format!(
            "{}{}{}",
            placeholder(
                2,
                r#"<p:ph type="title"/>"#,
                Some((100, 200, 8_000, 1_000)),
                &text("<a:p/>")
            ),
            placeholder(
                3,
                r#"<p:ph type="body" idx="1"/>"#,
                Some((100, 1_500, 8_000, 4_000)),
                &text("<a:p/>")
            ),
            concat!(
                r#"<p:sp><p:nvSpPr><p:cNvPr id="4" name="Bar"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>"#,
                r#"<a:xfrm><a:off x="0" y="6000"/><a:ext cx="9000" cy="500"/></a:xfrm><a:prstGeom prst="rect"/>"#,
                r#"<a:solidFill><a:schemeClr val="accent1"/></a:solidFill></p:spPr></p:sp>"#
            )
        ))
    )
}

fn layout(shows_master: bool) -> String {
    let flag = if shows_master {
        ""
    } else {
        r#" showMasterSp="0""#
    };
    format!(
        r"<p:sldLayout {NS}{flag}><p:cSld>{}</p:cSld></p:sldLayout>",
        tree(&format!(
            "{}{}",
            placeholder(2, r#"<p:ph type="title"/>"#, None, &text("<a:p/>")),
            placeholder(
                3,
                r#"<p:ph idx="1"/>"#,
                Some((300, 1_600, 7_000, 3_000)),
                concat!(
                    r#"<p:txBody><a:bodyPr anchor="ctr" lIns="0"/><a:lstStyle><a:lvl1pPr>"#,
                    r#"<a:defRPr b="1"/></a:lvl1pPr></a:lstStyle><a:p/></p:txBody>"#
                )
            )
        ))
    )
}

const THEME: &str = concat!(
    r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:themeElements>"#,
    r#"<a:clrScheme name="T"><a:dk2><a:srgbClr val="1F3864"/></a:dk2><a:lt2><a:srgbClr val="EEEEEE"/></a:lt2>"#,
    r#"<a:accent1><a:srgbClr val="2F5597"/></a:accent1></a:clrScheme>"#,
    r#"<a:fontScheme name="F"><a:majorFont><a:latin typeface="Georgia"/></a:majorFont>"#,
    r#"<a:minorFont><a:latin typeface="Verdana"/></a:minorFont></a:fontScheme></a:themeElements></a:theme>"#
);

/// A presentation of one master, one layout, the given slides and a picture.
fn presentation(slides: &[&str], size: &str, shows_master: bool) -> Vec<u8> {
    presentation_with(slides, size, shows_master, "")
}

fn presentation_with(
    slides: &[&str],
    size: &str,
    shows_master: bool,
    slide_flags: &str,
) -> Vec<u8> {
    let mut slide_list = String::new();
    for index in 0..slides.len() {
        let _ = write!(
            slide_list,
            r#"<p:sldId id="{}" r:id="rId{}"/>"#,
            256 + index,
            index + 2
        );
    }
    let main = format!(
        concat!(
            r#"<p:presentation {}><p:sldIdLst>{}</p:sldIdLst>{}<p:defaultTextStyle><a:lvl1pPr>"#,
            r#"<a:defRPr sz="1800"/></a:lvl1pPr></p:defaultTextStyle></p:presentation>"#
        ),
        NS, slide_list, size
    );
    let mut main_relationships = vec![(
        "rId1".to_owned(),
        "slideMaster",
        "slideMasters/slideMaster1.xml".to_owned(),
    )];
    for index in 0..slides.len() {
        main_relationships.push((
            format!("rId{}", index + 2),
            "slide",
            format!("slides/slide{}.xml", index + 1),
        ));
    }
    let main_relationships = relationships(
        &main_relationships
            .iter()
            .map(|(id, kind, target)| (id.as_str(), *kind, target.as_str()))
            .collect::<Vec<_>>(),
    );
    let slide_relationships = relationships(&[
        ("rId1", "slideLayout", "../slideLayouts/slideLayout1.xml"),
        ("rId2", "image", "../media/image1.png"),
        ("rId3", "image", "../media/missing.png"),
        ("rId4", "chart", "../charts/chart1.xml"),
        ("rId5", "diagramData", "../diagrams/data1.xml"),
        ("rId6", "diagramDrawing", "../diagrams/drawing1.xml"),
    ]);
    let slide_parts: Vec<(String, String)> = slides
        .iter()
        .enumerate()
        .map(|(index, slide)| {
            (
                format!("ppt/slides/slide{}.xml", index + 1),
                format!(r"<p:sld {NS}{slide_flags}>{slide}</p:sld>"),
            )
        })
        .collect();
    let slide_relationship_parts: Vec<String> = (0..slides.len())
        .map(|index| format!("ppt/slides/_rels/slide{}.xml.rels", index + 1))
        .collect();
    let content_types = concat!(
        r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">"#,
        r#"<Default Extension="xml" ContentType="application/xml"/>"#,
        r#"<Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/></Types>"#
    );
    let package_relationships =
        relationships(&[("rId1", "officeDocument", "ppt/presentation.xml")]);
    let master_relationships = relationships(&[("rId1", "theme", "../theme/theme1.xml")]);
    let layout_relationships =
        relationships(&[("rId1", "slideMaster", "../slideMasters/slideMaster1.xml")]);
    let master = master();
    let layout = layout(shows_master);
    let mut members: Vec<(&str, &[u8])> = vec![
        ("[Content_Types].xml", content_types.as_bytes()),
        ("_rels/.rels", package_relationships.as_bytes()),
        ("ppt/presentation.xml", main.as_bytes()),
        (
            "ppt/_rels/presentation.xml.rels",
            main_relationships.as_bytes(),
        ),
        ("ppt/slideMasters/slideMaster1.xml", master.as_bytes()),
        (
            "ppt/slideMasters/_rels/slideMaster1.xml.rels",
            master_relationships.as_bytes(),
        ),
        ("ppt/slideLayouts/slideLayout1.xml", layout.as_bytes()),
        (
            "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
            layout_relationships.as_bytes(),
        ),
        ("ppt/theme/theme1.xml", THEME.as_bytes()),
        ("ppt/media/image1.png", PNG),
        ("ppt/charts/chart1.xml", CHART.as_bytes()),
        ("ppt/diagrams/data1.xml", DIAGRAM_DATA.as_bytes()),
        ("ppt/diagrams/drawing1.xml", DIAGRAM_DRAWING.as_bytes()),
    ];
    for ((path, xml), relationship_path) in slide_parts.iter().zip(&slide_relationship_parts) {
        members.push((path, xml.as_bytes()));
        members.push((relationship_path, slide_relationships.as_bytes()));
    }
    build_zip(&members)
}

const CHART: &str = concat!(
    r#"<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart><c:plotArea>"#,
    r#"<c:pieChart><c:ser><c:val><c:numLit><c:ptCount val="2"/><c:pt idx="0"><c:v>1</c:v></c:pt>"#,
    r#"<c:pt idx="1"><c:v>3</c:v></c:pt></c:numLit></c:val></c:ser></c:pieChart></c:plotArea></c:chart></c:chartSpace>"#
);

const DIAGRAM_DATA: &str = concat!(
    r#"<dgm:dataModel xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">"#,
    r#"<dgm:extLst><a:ext uri="x"><dsp:dataModelExt xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" relId="rId6"/></a:ext></dgm:extLst></dgm:dataModel>"#
);

const DIAGRAM_DRAWING: &str = concat!(
    r#"<dsp:drawing xmlns:dsp="http://schemas.microsoft.com/office/drawing/2008/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><dsp:spTree>"#,
    r#"<dsp:sp><dsp:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="914400" cy="457200"/></a:xfrm><a:prstGeom prst="ellipse"/>"#,
    r#"<a:solidFill><a:srgbClr val="70AD47"/></a:solidFill></dsp:spPr><dsp:txBody><a:bodyPr/><a:p><a:r><a:t>Fase</a:t></a:r></a:p></dsp:txBody></dsp:sp>"#,
    "</dsp:spTree></dsp:drawing>"
);

const WIDE: &str = r#"<p:sldSz cx="12192000" cy="6858000"/>"#;

fn read(slides: &[&str]) -> Presentation {
    read_presentation(&presentation(slides, WIDE, true), &Limits::default()).unwrap()
}

fn texts(shape: &SlideShape) -> &TextBody {
    match &shape.content {
        ShapeContent::Text(body) => body,
        other => panic!("not text: {other:?}"),
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one slide checks every inherited property of its placeholders"
)]
fn places_and_styles_placeholders_from_their_layout_and_master() {
    let slide = format!(
        "<p:cSld>{}</p:cSld>",
        tree(&format!(
            "{}{}",
            placeholder(
                2,
                r#"<p:ph type="title"/>"#,
                None,
                &text("<a:p><a:r><a:t>Titolo</a:t></a:r></a:p>")
            ),
            placeholder(
                3,
                r#"<p:ph idx="1"/>"#,
                None,
                &text(concat!(
                    r#"<a:p><a:r><a:rPr lang="it-IT"/><a:t>Uno</a:t></a:r></a:p>"#,
                    r#"<a:p><a:pPr lvl="1"/><a:r><a:t>Primo</a:t></a:r></a:p>"#,
                    r#"<a:p><a:pPr lvl="1"/><a:r><a:rPr i="1" sz="2000"><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></a:rPr><a:t>Secondo</a:t></a:r><a:br/><a:r><a:t>dopo</a:t></a:r></a:p>"#,
                    r#"<a:p><a:r><a:t>Due</a:t></a:r></a:p>"#,
                    r#"<a:p><a:pPr lvl="1"/><a:r><a:t>Ancora</a:t></a:r></a:p>"#
                ))
            )
        ))
    );
    let presentation = read(&[&slide]);
    assert_eq!(
        (presentation.width_emu, presentation.height_emu),
        (12_192_000, 6_858_000)
    );
    let slide = &presentation.slides[0];
    assert_eq!(
        slide.background.as_ref().and_then(Paint::colour),
        Some("EEEEEE"),
        "bg2 from the master"
    );
    assert_eq!(
        slide.shapes.len(),
        3,
        "the master's bar, then the slide's two placeholders"
    );
    assert_eq!(
        slide.shapes[0].fill.as_ref().and_then(Paint::colour),
        Some("2F5597")
    );
    assert_eq!(slide.shapes[0].content, ShapeContent::None);

    let title = &slide.shapes[1];
    assert_eq!(
        title.frame,
        Frame {
            x: 100,
            y: 200,
            width: 8_000,
            height: 1_000
        },
        "the layout names no frame, so the master's applies"
    );
    let title = texts(title);
    let run = &title.paragraphs[0].runs[0];
    assert_eq!(
        (run.size, run.colour.as_str(), run.family.as_str()),
        (4_400, "1F3864", "Georgia")
    );
    assert_eq!(title.paragraphs[0].align, Alignment::Center);
    assert_eq!(title.paragraphs[0].bullet, None, "titles carry no bullets");

    let body_shape = &slide.shapes[2];
    assert_eq!(
        body_shape.frame.x, 300,
        "the layout's frame wins over the master's"
    );
    let body = texts(body_shape);
    assert_eq!(body.anchor, TextAnchor::Middle);
    assert_eq!(body.insets_emu, [0, 45_720, 91_440, 45_720]);
    let labels: Vec<Option<Bullet>> = body
        .paragraphs
        .iter()
        .map(|paragraph| paragraph.bullet.clone())
        .collect();
    assert_eq!(
        labels,
        vec![
            Some(Bullet::Character("\u{2022}".to_owned())),
            Some(Bullet::Number("1.".to_owned())),
            Some(Bullet::Number("2.".to_owned())),
            Some(Bullet::Character("\u{2022}".to_owned())),
            Some(Bullet::Number("1.".to_owned())),
        ],
        "a numbered list restarts under each item of the level above"
    );
    let first = &body.paragraphs[0];
    assert_eq!(
        (first.margin_left_emu, first.indent_emu, first.space_before),
        (228_600, -228_600, 1_000)
    );
    assert!(first.runs[0].bold, "from the layout's list style");
    assert_eq!(first.runs[0].family, "Verdana");
    let second = &body.paragraphs[2];
    assert_eq!(second.level, 1);
    assert_eq!(second.runs.len(), 3);
    assert_eq!((second.runs[0].size, second.runs[0].italic), (2_000, true));
    assert_eq!(second.runs[0].colour, "FF0000");
    assert!(second.runs[1].line_break);
    assert_eq!(second.runs[2].size, 2_400);
    assert!(!presentation.truncated);
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one slide carries every kind of shape"
)]
fn reads_shapes_pictures_groups_tables_and_fields() {
    let shapes = concat!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="Box"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>"#,
        r#"<a:xfrm rot="5400000"><a:off x="10" y="20"/><a:ext cx="300" cy="400"/></a:xfrm><a:prstGeom prst="roundRect"/></p:spPr>"#,
        r#"<p:style><a:lnRef idx="2"><a:schemeClr val="accent1"/></a:lnRef><a:fillRef idx="1"><a:srgbClr val="00FF00"/></a:fillRef>"#,
        r#"<a:fontRef idx="minor"><a:srgbClr val="FFFFFF"/></a:fontRef></p:style>"#,
        r#"<p:txBody><a:bodyPr wrap="none" anchor="b"><a:normAutofit fontScale="50000" lnSpcReduction="20000"/></a:bodyPr>"#,
        r#"<a:p><a:pPr><a:lnSpc><a:spcPct val="90000"/></a:lnSpc><a:spcAft><a:spcPts val="600"/></a:spcAft></a:pPr>"#,
        r#"<a:r><a:t>Pagina </a:t></a:r><a:fld id="{1}" type="slidenum"><a:t>9</a:t></a:fld>"#,
        r#"<a:fld id="{2}" type="datetime1"><a:t>27/10</a:t></a:fld></a:p></p:txBody></p:sp>"#,
        r#"<p:sp><p:nvSpPr><p:cNvPr id="3" name="Empty"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>"#,
        r#"<a:xfrm><a:off x="0" y="0"/><a:ext cx="1" cy="1"/></a:xfrm><a:noFill/></p:spPr></p:sp>"#,
        r#"<p:cxnSp><p:nvCxnSpPr><p:cNvPr id="4" name="Line"/><p:cNvCxnSpPr/><p:nvPr/></p:nvCxnSpPr><p:spPr>"#,
        r#"<a:xfrm flipV="1"><a:off x="0" y="0"/><a:ext cx="100" cy="50"/></a:xfrm><a:prstGeom prst="line"/>"#,
        r#"<a:ln w="38100"><a:solidFill><a:srgbClr val="ED7D31"/></a:solidFill></a:ln></p:spPr></p:cxnSp>"#,
        r#"<p:grpSp><p:nvGrpSpPr><p:cNvPr id="5" name="G"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr>"#,
        r#"<a:xfrm><a:off x="1000" y="1000"/><a:ext cx="500" cy="500"/><a:chOff x="0" y="0"/><a:chExt cx="1000" cy="1000"/></a:xfrm></p:grpSpPr>"#,
        r#"<p:sp><p:nvSpPr><p:cNvPr id="6" name="In"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr>"#,
        r#"<a:xfrm><a:off x="200" y="400"/><a:ext cx="600" cy="200"/></a:xfrm><a:prstGeom prst="ellipse"/>"#,
        r#"<a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="123456"/></a:gs></a:gsLst></a:gradFill></p:spPr></p:sp></p:grpSp>"#,
        r#"<p:pic><p:nvPicPr><p:cNvPr id="7" name="Logo"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr>"#,
        r#"<p:blipFill><a:blip r:embed="rId2"/></p:blipFill><p:spPr><a:xfrm><a:off x="5" y="5"/><a:ext cx="50" cy="50"/></a:xfrm></p:spPr></p:pic>"#,
        r#"<p:pic><p:nvPicPr><p:cNvPr id="8" name="Again"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr>"#,
        r#"<p:blipFill><a:blip r:embed="rId2"/></p:blipFill><p:spPr><a:xfrm><a:off x="6" y="6"/><a:ext cx="50" cy="50"/></a:xfrm></p:spPr></p:pic>"#,
        r#"<p:pic><p:nvPicPr><p:cNvPr id="9" name="Missing"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr>"#,
        r#"<p:blipFill><a:blip r:embed="rId3"/></p:blipFill><p:spPr><a:xfrm><a:off x="6" y="6"/><a:ext cx="50" cy="50"/></a:xfrm></p:spPr></p:pic>"#,
        r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="10" name="T"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>"#,
        r#"<p:xfrm><a:off x="100" y="100"/><a:ext cx="2000" cy="800"/></p:xfrm><a:graphic><a:graphicData><a:tbl>"#,
        r#"<a:tblGrid><a:gridCol w="1000"/><a:gridCol w="1000"/></a:tblGrid>"#,
        r#"<a:tr h="400"><a:tc gridSpan="2"><a:txBody><a:bodyPr/><a:p><a:r><a:t>Testata</a:t></a:r></a:p></a:txBody>"#,
        r#"<a:tcPr marL="10" anchor="ctr"><a:solidFill><a:srgbClr val="DDDDDD"/></a:solidFill></a:tcPr></a:tc><a:tc hMerge="1"/></a:tr>"#,
        r#"<a:tr h="400"><a:tc><a:txBody><a:bodyPr/><a:p><a:r><a:t>A</a:t></a:r></a:p></a:txBody></a:tc>"#,
        r#"<a:tc><a:txBody><a:bodyPr/><a:p><a:r><a:t>B</a:t></a:r></a:p></a:txBody><a:tcPr><a:noFill/></a:tcPr></a:tc></a:tr>"#,
        r#"</a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#,
        r#"<mc:AlternateContent><mc:Choice Requires="x"><p:sp><p:nvSpPr><p:cNvPr id="11" name="Alt"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>"#,
        r#"<p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="10" cy="10"/></a:xfrm><a:solidFill><a:srgbClr val="ABCDEF"/></a:solidFill></p:spPr></p:sp></mc:Choice></mc:AlternateContent>"#,
        r#"<p:sp><p:nvSpPr><p:cNvPr id="12" name="NoFrame"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:solidFill><a:srgbClr val="ABCDEF"/></a:solidFill></p:spPr></p:sp>"#
    );
    let slide = format!(
        r#"<p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val="FAFAFA"/></a:solidFill></p:bgPr></p:bg>{}</p:cSld>"#,
        tree(shapes)
    );
    let presentation = read(&[&slide, &slide]);
    let slide = &presentation.slides[1];
    assert_eq!(
        slide.background.as_ref().and_then(Paint::colour),
        Some("FAFAFA")
    );
    let shapes = &slide.shapes[1..];

    let boxed = &shapes[0];
    let ShapeGeometry::Path(paths) = &boxed.geometry else {
        panic!("a rounded rectangle drawn as its preset");
    };
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].commands.len(), 9);
    assert!((boxed.rotation - 90.0).abs() < f64::EPSILON);
    assert_eq!(
        boxed.fill.as_ref().and_then(Paint::colour),
        Some("00FF00"),
        "from the style's fill reference"
    );
    assert_eq!(
        boxed.line,
        Some(ShapeLine {
            colour: "2F5597".to_owned(),
            width_emu: DEFAULT_LINE_EMU
        })
    );
    let body = texts(boxed);
    assert_eq!((body.anchor, body.wrap), (TextAnchor::Bottom, false));
    let paragraph = &body.paragraphs[0];
    assert_eq!(
        paragraph.line_spacing_percent, 70,
        "90% less the autofit reduction"
    );
    assert_eq!(paragraph.space_after, 300, "scaled by the autofit");
    let runs: Vec<&str> = paragraph.runs.iter().map(|run| run.text.as_str()).collect();
    assert_eq!(
        runs,
        ["Pagina ", "2", "27/10"],
        "the slide number is this slide's"
    );
    assert_eq!(paragraph.runs[0].size, 900, "18 points, halved");
    assert_eq!(
        paragraph.runs[0].colour, "FFFFFF",
        "from the style's font reference"
    );

    assert_eq!(
        shapes[1].geometry,
        ShapeGeometry::Line { flipped: true },
        "the empty shape is gone"
    );
    assert_eq!(shapes[1].line.as_ref().unwrap().width_emu, 38_100);

    let grouped = &shapes[2];
    assert_eq!(
        grouped.frame,
        Frame {
            x: 1_100,
            y: 1_200,
            width: 300,
            height: 100
        }
    );
    assert_eq!(
        (
            grouped.geometry.clone(),
            grouped.fill.as_ref().and_then(Paint::colour)
        ),
        (ShapeGeometry::Ellipse, Some("123456"))
    );

    assert_eq!(shapes[3].content, ShapeContent::Picture { media: 0 });
    assert_eq!(
        shapes[4].content,
        ShapeContent::Picture { media: 0 },
        "one image, loaded once"
    );
    assert_eq!(presentation.media.len(), 1);
    assert_eq!(presentation.media[0].content_type, "image/png");

    let ShapeContent::Table(table) = &shapes[5].content else {
        panic!("not a table: {:?}", shapes[5].content);
    };
    assert_eq!(table.column_widths_emu, [1_000, 1_000]);
    assert_eq!(table.rows[0].height_emu, 400);
    let header = &table.rows[0].cells[0];
    assert_eq!(
        (header.col_span, header.fill.as_deref()),
        (2, Some("DDDDDD"))
    );
    assert_eq!(
        (header.body.anchor, header.body.insets_emu[0]),
        (TextAnchor::Middle, 10)
    );
    assert_eq!(
        header.body.paragraphs[0].runs[0].size, 1_800,
        "the presentation's default style, not the master's other style"
    );
    assert!(table.rows[0].cells[1].covered);
    assert_eq!(table.rows[1].cells[1].fill, None);
    assert_eq!(
        shapes[6].fill.as_ref().and_then(Paint::colour),
        Some("ABCDEF"),
        "the alternate content's choice"
    );
    assert_eq!(shapes.len(), 7, "a shape without a frame is not drawn");
}

#[test]
fn leaves_out_the_master_shapes_when_asked_and_takes_the_default_size() {
    let slide = format!("<p:cSld>{}</p:cSld>", tree(""));
    let bytes = presentation(&[&slide], "", false);
    let presentation = read_presentation(&bytes, &Limits::default()).unwrap();
    assert_eq!(
        (presentation.width_emu, presentation.height_emu),
        DEFAULT_SIZE
    );
    assert_eq!(presentation.slides[0].shapes, Vec::<SlideShape>::new());

    let bytes = presentation_with(&[&slide], WIDE, true, r#" showMasterSp="0""#);
    assert_eq!(
        read_presentation(&bytes, &Limits::default())
            .unwrap()
            .slides[0]
            .shapes,
        Vec::<SlideShape>::new()
    );
}

#[test]
fn refuses_what_is_not_a_presentation_and_honours_the_limits() {
    assert!(matches!(
        read_presentation(&build_docx(&["Testo"]), &Limits::default()),
        Err(OfficeError::UnsupportedFormat(DocumentFormat::Docx))
    ));
    let slide = format!(
        "<p:cSld>{}</p:cSld>",
        tree(&placeholder(
            2,
            r#"<p:ph type="title"/>"#,
            None,
            &text("<a:p><a:r><a:t>Titolo lungo</a:t></a:r></a:p>")
        ))
    );
    let bytes = presentation(&[&slide], WIDE, true);
    let small = Limits {
        max_input_bytes: 10,
        ..Limits::default()
    };
    assert!(matches!(
        read_presentation(&bytes, &small),
        Err(OfficeError::TooLarge { .. })
    ));
    let short = Limits {
        max_text_chars: 6,
        ..Limits::default()
    };
    let presentation = read_presentation(&bytes, &short).unwrap();
    assert!(presentation.truncated);
    let title = texts(&presentation.slides[0].shapes[1]);
    assert_eq!(title.paragraphs[0].runs[0].text, "Titolo");
}

#[test]
fn maps_group_coordinates_and_reads_frames() {
    let xfrm = |xml: &str| {
        parse_tree(
            &format!(r#"<a:xfrm xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" {xml}</a:xfrm>"#),
            |_, _| false,
        )
        .unwrap()
    };
    let outer = Transform::identity().then_group(&xfrm(
        r#"><a:off x="100" y="100"/><a:ext cx="200" cy="200"/><a:chOff x="0" y="0"/><a:chExt cx="400" cy="400"/>"#,
    ));
    let inner = outer.then_group(&xfrm(
        r#"><a:off x="200" y="200"/><a:ext cx="100" cy="100"/>"#,
    ));
    let frame = Frame {
        x: 200,
        y: 200,
        width: 100,
        height: 100,
    };
    assert_eq!(
        inner.apply(frame),
        Frame {
            x: 200,
            y: 200,
            width: 50,
            height: 50
        }
    );
    assert_eq!(
        outer.then_group(&xfrm(">")),
        outer,
        "a group without a frame changes nothing"
    );
    let (frame, rotation, flipped) = read_frame(&xfrm(
        r#"rot="-5400000" flipH="1"><a:off x="1" y="2"/><a:ext cx="-3" cy="4"/>"#,
    ))
    .unwrap();
    assert_eq!((frame.width, frame.height), (0, 4));
    assert!((rotation + 90.0).abs() < f64::EPSILON);
    assert!(flipped);
    assert!(read_frame(&xfrm(r#"><a:off x="1" y="2"/>"#)).is_none());
}

#[test]
fn draws_the_charts_of_a_slide_in_the_theme_of_its_master() {
    let frame = |id: &str| {
        format!(
            concat!(
                r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="2" name="G"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>"#,
                r#"<p:xfrm><a:off x="0" y="0"/><a:ext cx="3810000" cy="2540000"/></p:xfrm><a:graphic>"#,
                r#"<a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart">"#,
                r#"<c:chart xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" r:id="{}"/></a:graphicData></a:graphic></p:graphicFrame>"#
            ),
            id
        )
    };
    let slide = format!(
        "<p:cSld>{}</p:cSld>",
        tree(&[frame("rId4"), frame("rId9")].concat())
    );
    let presentation = read(&[&slide]);
    let shapes = &presentation.slides[0].shapes[1..];
    assert_eq!(shapes.len(), 1, "the chart without a part is left out");
    assert_eq!(shapes[0].content, ShapeContent::Picture { media: 0 });
    assert_eq!(shapes[0].frame.width, 3_810_000);
    assert_eq!(presentation.media[0].content_type, "image/svg+xml");
    let svg = String::from_utf8(presentation.media[0].bytes.clone()).unwrap();
    assert!(
        svg.contains("fill=\"#2F5597\""),
        "the master theme's first accent: {svg}"
    );
}

#[test]
fn draws_presets_and_custom_geometries_as_paths_mirrored_as_their_frame_says() {
    let shape = |geometry: &str, flips: &str| {
        format!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="2" name="s"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm {flips}><a:off x="0" y="0"/><a:ext cx="200" cy="100"/></a:xfrm>{geometry}<a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></p:spPr></p:sp>"#
        )
    };
    let slide = format!("<p:cSld>{}</p:cSld>", tree(&[
        shape(r#"<a:prstGeom prst="rightArrow"><a:avLst/></a:prstGeom>"#, r#"flipH="1""#),
        shape(
            r#"<a:custGeom><a:pathLst><a:path w="2" h="2"><a:moveTo><a:pt x="0" y="0"/></a:moveTo><a:lnTo><a:pt x="2" y="2"/></a:lnTo><a:close/></a:path></a:pathLst></a:custGeom>"#,
            r#"flipV="true""#,
        ),
        shape(r#"<a:prstGeom prst="noSuchPreset"/>"#, ""),
        shape(r#"<a:prstGeom prst="ellipse"/>"#, ""),
        shape("<a:prstGeom/>", ""),
        shape("", ""),
        shape("<a:custGeom/>", ""),
        shape(r#"<a:prstGeom prst="straightConnector1"/>"#, r#"flipH="1""#),
    ]
    .concat()));
    let presentation = read(&[&slide]);
    let shapes = &presentation.slides[0].shapes;
    let shapes = &shapes[shapes.len() - 8..];
    let ShapeGeometry::Path(arrow) = &shapes[0].geometry else {
        panic!("an arrow drawn as its path");
    };
    assert_eq!(
        arrow[0].commands[0],
        PathCommand::MoveTo(PathPoint { x: 200, y: 25 }),
        "mirrored left to right"
    );
    let ShapeGeometry::Path(custom) = &shapes[1].geometry else {
        panic!("a custom geometry drawn as its path");
    };
    assert_eq!(
        custom[0].commands[..2],
        [
            PathCommand::MoveTo(PathPoint { x: 0, y: 100 }),
            PathCommand::LineTo(PathPoint { x: 200, y: 0 }),
        ]
    );
    assert_eq!(shapes[2].geometry, ShapeGeometry::Other);
    assert_eq!(shapes[3].geometry, ShapeGeometry::Ellipse);
    assert_eq!(shapes[4].geometry, ShapeGeometry::Rectangle);
    assert_eq!(shapes[5].geometry, ShapeGeometry::Rectangle);
    assert_eq!(shapes[6].geometry, ShapeGeometry::Other);
    assert_eq!(shapes[7].geometry, ShapeGeometry::Line { flipped: true });
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one slide carries every kind of fill"
)]
fn reads_gradients_and_pictures_as_fills() {
    let shape = |id: u32, fill: &str| {
        format!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="S{id}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="100" cy="100"/></a:xfrm>{fill}</p:spPr></p:sp>"#
        )
    };
    let shapes = [
        shape(
            2,
            concat!(
                r#"<a:gradFill><a:gsLst><a:gs pos="100000"><a:srgbClr val="FFFFFF"/></a:gs>"#,
                r#"<a:gs pos="0"><a:schemeClr val="accent1"/></a:gs><a:gs pos="nope"><a:srgbClr val="000000"/></a:gs>"#,
                r#"</a:gsLst><a:lin ang="2700000"/></a:gradFill>"#
            ),
        ),
        shape(
            3,
            concat!(
                r#"<a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="FF0000"/></a:gs><a:gs pos="100000"><a:srgbClr val="0000FF"/></a:gs></a:gsLst>"#,
                r#"<a:path path="circle"><a:fillToRect l="100000" t="0" r="0" b="100000"/></a:path></a:gradFill>"#
            ),
        ),
        shape(
            4,
            r#"<a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="FF0000"/></a:gs><a:gs pos="1"><a:srgbClr val="00FF00"/></a:gs></a:gsLst></a:gradFill>"#,
        ),
        shape(
            5,
            r#"<a:blipFill><a:blip r:embed="rId2"/><a:stretch/></a:blipFill>"#,
        ),
        shape(
            6,
            r#"<a:blipFill><a:blip r:embed="rId2"/><a:tile sx="50000" sy="200000"/></a:blipFill>"#,
        ),
        shape(
            7,
            r#"<a:blipFill><a:blip r:embed="rId3"/></a:blipFill>"#,
        ),
        shape(
            8,
            r#"<a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="ABCDEF"/></a:gs></a:gsLst></a:gradFill>"#,
        ),
    ]
    .concat();
    let slide = format!(
        r#"<p:cSld><p:bg><p:bgPr><a:blipFill><a:blip r:embed="rId2"/></a:blipFill></p:bgPr></p:bg>{}</p:cSld>"#,
        tree(&shapes)
    );
    let presentation = read(&[&slide]);
    let slide = &presentation.slides[0];
    assert_eq!(
        slide.background,
        Some(Paint::Picture {
            media: 0,
            tile_emu: None
        })
    );
    let fills: Vec<Option<Paint>> = slide.shapes[1..]
        .iter()
        .map(|shape| shape.fill.clone())
        .collect();
    let stop = |offset: f64, colour: &str| GradientStop {
        offset,
        colour: colour.to_owned(),
    };
    assert_eq!(
        fills[0],
        Some(Paint::Gradient(Gradient {
            stops: vec![
                stop(0.0, "2F5597"),
                stop(0.0, "000000"),
                stop(1.0, "FFFFFF")
            ],
            kind: GradientKind::Linear { angle: 45.0 },
        })),
        "the stops in order of their position, an unreadable one at the start"
    );
    assert_eq!(
        fills[1],
        Some(Paint::Gradient(Gradient {
            stops: vec![stop(0.0, "FF0000"), stop(1.0, "0000FF")],
            kind: GradientKind::Radial { centre: (1.0, 0.0) },
        }))
    );
    assert!(
        matches!(
            &fills[2],
            Some(Paint::Gradient(Gradient {
                kind: GradientKind::Linear { angle },
                ..
            })) if (*angle - 90.0).abs() < f64::EPSILON
        ),
        "no line and no path is a gradient from top to bottom"
    );
    assert_eq!(
        fills[3],
        Some(Paint::Picture {
            media: 0,
            tile_emu: None
        })
    );
    assert_eq!(
        fills[4],
        Some(Paint::Picture {
            media: 0,
            tile_emu: Some((95_250, 190_500))
        }),
        "a tile of 20 by 10 pixels at half and double size"
    );
    assert_eq!(
        slide.shapes.len(),
        7,
        "a picture fill that is missing is no fill"
    );
    assert_eq!(fills[5], Some(Paint::Solid("ABCDEF".to_owned())));
}

#[test]
fn draws_a_smart_art_as_a_picture_of_its_drawing() {
    let frame = concat!(
        r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="4" name="D"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>"#,
        r#"<p:xfrm><a:off x="914400" y="914400"/><a:ext cx="1828800" cy="914400"/></p:xfrm><a:graphic>"#,
        r#"<a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/diagram">"#,
        r#"<dgm:relIds xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram" r:dm="rId5" r:lo="rId5" r:qs="rId5" r:cs="rId5"/>"#,
        "</a:graphicData></a:graphic></p:graphicFrame>"
    );
    let slide = format!("<p:cSld>{}</p:cSld>", tree(frame));
    let read = read(&[&slide]);
    let shape = read.slides[0]
        .shapes
        .iter()
        .find(|shape| !shape.inherited && shape.id == Some(4))
        .expect("the SmartArt");
    assert_eq!(
        shape.frame,
        Frame {
            x: 914_400,
            y: 914_400,
            width: 1_828_800,
            height: 914_400,
        }
    );
    let ShapeContent::Picture { media } = shape.content else {
        panic!("a picture: {:?}", shape.content);
    };
    let svg = String::from_utf8(read.media[media].bytes.clone()).unwrap();
    assert!(
        svg.contains(r##"fill="#70AD47""##) && svg.contains(">Fase</text>"),
        "{svg}"
    );
}
