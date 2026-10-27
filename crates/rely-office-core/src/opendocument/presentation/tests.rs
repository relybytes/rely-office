// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;
use crate::geometry::{PathCommand, PathPoint};
use crate::read_presentation;
use crate::test_support::build_zip;

const NAMESPACES: &str = concat!(
    r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
    r#"xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" "#,
    r#"xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" "#,
    r#"xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" "#,
    r#"xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" "#,
    r#"xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" "#,
    r#"xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" "#,
    r#"xmlns:presentation="urn:oasis:names:tc:opendocument:xmlns:presentation:1.0" "#,
    r#"xmlns:chart="urn:oasis:names:tc:opendocument:xmlns:chart:1.0" "#,
    r#"xmlns:loext="urn:org:documentfoundation:names:experimental:office:xmlns:loext:1.0" "#,
    r#"xmlns:xlink="http://www.w3.org/1999/xlink""#
);
const PNG: &[u8] = &[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 0, 0];

const STYLES: &str = concat!(
    "<office:styles>",
    r##"<draw:gradient draw:name="Sfumato" draw:start-color="#102030" draw:end-color="#ffffff"/>"##,
    r#"<style:default-style style:family="graphic"><style:text-properties fo:font-size="18pt" style:font-name="Liberation Sans"/></style:default-style>"#,
    r##"<style:style style:name="standard" style:family="graphic"><style:graphic-properties draw:stroke="solid" svg:stroke-color="#3465a4" "##,
    r##"svg:stroke-width="0.1cm" draw:fill="solid" draw:fill-color="#729fcf"/></style:style>"##,
    r##"<style:style style:name="Default-title" style:family="presentation"><style:text-properties fo:font-size="44pt" fo:color="#1f3864"/>"##,
    r#"<style:paragraph-properties fo:text-align="center"/></style:style>"#,
    r#"<style:style style:name="Default-outline1" style:family="presentation"><style:graphic-properties draw:fill="none" draw:stroke="none"/>"#,
    r#"<style:text-properties fo:font-size="32pt"/><style:paragraph-properties fo:margin-top="0.5cm"/></style:style>"#,
    r#"<style:style style:name="Default-outline2" style:family="presentation" style:parent-style-name="Default-outline1"><style:text-properties fo:font-size="28pt"/></style:style>"#,
    "</office:styles>",
    "<office:automatic-styles>",
    r#"<style:page-layout style:name="PM1"><style:page-layout-properties fo:page-width="28cm" fo:page-height="15.75cm"/></style:page-layout>"#,
    r##"<style:style style:name="Mdp1" style:family="drawing-page"><style:drawing-page-properties draw:fill="solid" draw:fill-color="#eeeeee"/></style:style>"##,
    r#"<style:style style:name="Mgr1" style:family="graphic" style:parent-style-name="standard"/>"#,
    "</office:automatic-styles>",
    r#"<office:master-styles><style:master-page style:name="Default" style:page-layout-name="PM1" draw:style-name="Mdp1">"#,
    r#"<draw:rect draw:style-name="Mgr1" svg:x="0cm" svg:y="15cm" svg:width="28cm" svg:height="0.75cm"/>"#,
    r#"<draw:frame presentation:class="title" svg:x="1cm" svg:y="1cm" svg:width="20cm" svg:height="2cm"><draw:text-box><text:p>Titolo del master</text:p></draw:text-box></draw:frame>"#,
    r#"<draw:frame presentation:class="page-number" presentation:placeholder="true" svg:x="25cm" svg:y="14cm" svg:width="2cm" svg:height="1cm">"#,
    r#"<draw:text-box><text:p><text:page-number>&lt;numero&gt;</text:page-number></text:p></draw:text-box></draw:frame>"#,
    r#"<draw:frame presentation:class="footer" svg:x="1cm" svg:y="14cm" svg:width="5cm" svg:height="1cm"><draw:text-box><text:p>Piede</text:p></draw:text-box></draw:frame>"#,
    "</style:master-page></office:master-styles>"
);

const AUTOMATIC: &str = concat!(
    "<office:automatic-styles>",
    r#"<style:style style:name="dp1" style:family="drawing-page"><style:drawing-page-properties presentation:display-page-number="true"/></style:style>"#,
    r#"<style:style style:name="dp2" style:family="drawing-page"><style:drawing-page-properties draw:fill="gradient" draw:fill-gradient-name="Sfumato" "#,
    r#"presentation:background-objects-visible="false"/></style:style>"#,
    r#"<style:style style:name="pr1" style:family="presentation" style:parent-style-name="Default-title"/>"#,
    r#"<style:style style:name="pr2" style:family="presentation" style:parent-style-name="Default-outline1"/>"#,
    r#"<style:style style:name="gr1" style:family="graphic" style:parent-style-name="standard"><style:graphic-properties draw:textarea-vertical-align="middle" "#,
    r#"fo:padding="0.2cm" fo:wrap-option="no-wrap"/></style:style>"#,
    r#"<style:style style:name="gr2" style:family="graphic"><style:graphic-properties draw:stroke="none" draw:fill="none"/></style:style>"#,
    r#"<style:style style:name="P1" style:family="paragraph"><style:paragraph-properties fo:text-align="end" fo:line-height="150%" fo:margin-bottom="0.2cm"/></style:style>"#,
    r#"<style:style style:name="T1" style:family="text"><style:text-properties fo:font-weight="bold" fo:font-style="italic" style:text-underline-style="solid" "#,
    r##"style:text-line-through-style="solid" fo:color="#ff0000"/></style:style>"##,
    r#"<text:list-style style:name="L1"><text:list-level-style-bullet text:level="1" text:bullet-char="&#x25CF;">"#,
    r#"<style:list-level-properties text:space-before="0.3cm" text:min-label-width="0.9cm"/></text:list-level-style-bullet>"#,
    r#"<text:list-level-style-number text:level="2" style:num-format="a" style:num-suffix=")"/></text:list-style>"#,
    r#"<style:style style:name="co1" style:family="table-column"><style:table-column-properties style:column-width="4cm"/></style:style>"#,
    r#"<style:style style:name="ro1" style:family="table-row"><style:table-row-properties style:row-height="1cm"/></style:style>"#,
    r##"<style:style style:name="ce1" style:family="table-cell"><loext:graphic-properties draw:fill="solid" draw:fill-color="#2f5597"/></style:style>"##,
    r##"<style:style style:name="ce2" style:family="table-cell"><style:table-cell-properties fo:background-color="#d9e2f3"/></style:style>"##,
    "</office:automatic-styles>"
);

const CHART: &str = concat!(
    r#"<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
    r#"xmlns:chart="urn:oasis:names:tc:opendocument:xmlns:chart:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0">"#,
    r#"<office:body><office:chart><chart:chart chart:class="chart:bar"><chart:plot-area>"#,
    r#"<chart:series chart:values-cell-range-address="local-table.$A$1:.$A$1"/></chart:plot-area>"#,
    r#"<table:table table:name="local-table"><table:table-rows><table:table-row><table:table-cell office:value-type="float" office:value="3"/>"#,
    "</table:table-row></table:table-rows></table:table></chart:chart></office:chart></office:body></office:document-content>"
);

fn odp(pages: &str) -> Vec<u8> {
    let content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content {NAMESPACES}>{AUTOMATIC}<office:body><office:presentation>{pages}</office:presentation></office:body></office:document-content>"#
    );
    let styles = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-styles {NAMESPACES}>{STYLES}</office:document-styles>"#
    );
    build_zip(&[
        (
            "mimetype",
            b"application/vnd.oasis.opendocument.presentation",
        ),
        ("content.xml", content.as_bytes()),
        ("styles.xml", styles.as_bytes()),
        ("Pictures/logo.png", PNG),
        ("Object 1/content.xml", CHART.as_bytes()),
        ("ObjectReplacements/Object 1", b"not an image"),
    ])
}

fn read(pages: &str) -> Presentation {
    read_presentation(&odp(pages), &Limits::default()).unwrap()
}

fn text(shape: &SlideShape) -> &TextBody {
    match &shape.content {
        ShapeContent::Text(body) => body,
        other => panic!("not text: {other:?}"),
    }
}

fn joined(paragraph: &SlideParagraph) -> String {
    paragraph
        .runs
        .iter()
        .map(|run| {
            if run.line_break {
                "\n".to_owned()
            } else {
                run.text.clone()
            }
        })
        .collect()
}

#[test]
fn draws_each_page_over_its_master_with_the_text_of_its_styles() {
    let presentation = read(concat!(
        r#"<draw:page draw:name="Uno" draw:style-name="dp1" draw:master-page-name="Default">"#,
        r#"<draw:frame presentation:style-name="pr1" presentation:class="title" svg:x="1cm" svg:y="1cm" svg:width="20cm" svg:height="2cm">"#,
        r#"<draw:text-box><text:p>Titolo  della <text:span text:style-name="T1">pagina</text:span><text:line-break/>seconda<text:s text:c="2"/>riga</text:p></draw:text-box></draw:frame>"#,
        r#"<draw:frame presentation:style-name="pr2" presentation:class="outline" svg:x="1cm" svg:y="4cm" svg:width="20cm" svg:height="8cm"><draw:text-box>"#,
        r#"<text:list text:style-name="L1"><text:list-item><text:p>Primo</text:p><text:list><text:list-item><text:p>Sotto</text:p></text:list-item>"#,
        r#"<text:list-item><text:p>Sotto due</text:p></text:list-item></text:list></text:list-item><text:list-item><text:p>Secondo</text:p></text:list-item>"#,
        r#"<text:list-header><text:p>Senza punto</text:p></text:list-header></text:list></draw:text-box></draw:frame>"#,
        r#"<draw:frame presentation:class="outline" presentation:placeholder="true" svg:x="0cm" svg:y="0cm" svg:width="1cm" svg:height="1cm"><draw:text-box/></draw:frame>"#,
        "</draw:page>"
    ));
    assert_eq!(
        (presentation.width_emu, presentation.height_emu),
        (10_080_000, 5_670_000)
    );
    let slide = &presentation.slides[0];
    assert_eq!(
        slide.background.as_deref(),
        Some("EEEEEE"),
        "the master's background"
    );
    assert_eq!(
        slide.shapes.len(),
        4,
        "the master's bar and page number, then the title and the outline"
    );
    assert_eq!(slide.shapes[0].fill.as_deref(), Some("729FCF"));
    assert_eq!(
        slide.shapes[0].line,
        Some(ShapeLine {
            colour: "3465A4".to_owned(),
            width_emu: 36_000
        })
    );
    assert_eq!(
        joined(&text(&slide.shapes[1]).paragraphs[0]),
        "1",
        "the page shows its number"
    );
    let title = text(&slide.shapes[2]);
    let paragraph = &title.paragraphs[0];
    assert_eq!(joined(paragraph), "Titolo della pagina\nseconda  riga");
    assert_eq!(paragraph.align, Alignment::Center);
    assert_eq!(
        (paragraph.runs[0].size, paragraph.runs[0].colour.as_str()),
        (4_400, "1F3864")
    );
    assert_eq!(paragraph.runs[0].family, "Liberation Sans");
    let styled = &paragraph.runs[1];
    assert!(styled.bold && styled.italic && styled.underline && styled.strike);
    assert_eq!(styled.colour, "FF0000");
    let outline = text(&slide.shapes[3]);
    let labels: Vec<(String, Option<Bullet>, u8, u32)> = outline
        .paragraphs
        .iter()
        .map(|paragraph| {
            (
                joined(paragraph),
                paragraph.bullet.clone(),
                paragraph.level,
                paragraph.runs[0].size,
            )
        })
        .collect();
    let bullet = || Some(Bullet::Character("\u{25cf}".to_owned()));
    assert_eq!(
        labels,
        [
            ("Primo".to_owned(), bullet(), 0, 3_200),
            (
                "Sotto".to_owned(),
                Some(Bullet::Number("a)".to_owned())),
                1,
                2_800
            ),
            (
                "Sotto due".to_owned(),
                Some(Bullet::Number("b)".to_owned())),
                1,
                2_800
            ),
            ("Secondo".to_owned(), bullet(), 0, 3_200),
            ("Senza punto".to_owned(), None, 0, 3_200),
        ],
        "each level in its own outline style"
    );
    assert_eq!(
        (
            outline.paragraphs[0].margin_left_emu,
            outline.paragraphs[0].indent_emu
        ),
        (432_000, -324_000)
    );
    assert_eq!(
        outline.paragraphs[0].space_before, 1_415,
        "half a centimetre in hundredths of a point"
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one page carries every kind of shape"
)]
fn places_shapes_lines_groups_rotations_tables_pictures_and_charts() {
    let presentation = read(concat!(
        r#"<draw:page draw:style-name="dp2" draw:master-page-name="Default">"#,
        r#"<draw:custom-shape draw:style-name="gr1" svg:x="1cm" svg:y="1cm" svg:width="4cm" svg:height="2cm"><text:p text:style-name="P1">Forma</text:p>"#,
        r#"<draw:enhanced-geometry draw:type="ooxml-roundRect"/></draw:custom-shape>"#,
        r#"<draw:custom-shape draw:style-name="gr1" svg:x="1cm" svg:y="4cm" svg:width="2cm" svg:height="2cm"><draw:enhanced-geometry draw:type="ellipse"/></draw:custom-shape>"#,
        r#"<draw:custom-shape draw:style-name="gr1" svg:x="1cm" svg:y="4cm" svg:width="2cm" svg:height="2cm"><draw:enhanced-geometry draw:type="star5"/></draw:custom-shape>"#,
        r#"<draw:custom-shape draw:style-name="gr2" svg:width="2cm" svg:height="1cm" draw:transform="rotate (1.5707963267949) translate (5cm 6cm)">"#,
        r#"<text:p>Ruotata</text:p></draw:custom-shape>"#,
        r#"<draw:line svg:x1="1cm" svg:y1="3cm" svg:x2="4cm" svg:y2="1cm"/>"#,
        r#"<draw:g><draw:rect draw:style-name="gr1" svg:x="10cm" svg:y="1cm" svg:width="1cm" svg:height="1cm" draw:corner-radius="0.2cm"/><draw:ellipse draw:style-name="gr1" svg:x="12cm" svg:y="1cm" svg:width="1cm" svg:height="1cm"/></draw:g>"#,
        r#"<draw:frame svg:x="1cm" svg:y="8cm" svg:width="8cm" svg:height="2cm"><table:table><table:table-column table:style-name="co1" table:number-columns-repeated="2"/>"#,
        r#"<table:table-row table:style-name="ro1" table:default-cell-style-name="ce1"><table:table-cell table:number-columns-spanned="2"><text:p>Testata</text:p></table:table-cell>"#,
        r#"<table:covered-table-cell/></table:table-row><table:table-row><table:table-cell table:style-name="ce2"><text:p>A</text:p></table:table-cell><table:table-cell/></table:table-row>"#,
        r#"</table:table><draw:image xlink:href="./ObjectReplacements/Table"/></draw:frame>"#,
        r#"<draw:frame svg:x="15cm" svg:y="1cm" svg:width="2cm" svg:height="2cm"><draw:image xlink:href="./Pictures/logo.png"/></draw:frame>"#,
        r#"<draw:frame svg:x="15cm" svg:y="4cm" svg:width="2cm" svg:height="2cm"><draw:image xlink:href="Pictures/logo.png"/></draw:frame>"#,
        r#"<draw:frame svg:x="15cm" svg:y="7cm" svg:width="2cm" svg:height="2cm"><draw:image xlink:href="https://example.com/a.png"/></draw:frame>"#,
        r#"<draw:frame svg:x="18cm" svg:y="1cm" svg:width="8cm" svg:height="6cm"><draw:object xlink:href="./Object 1"/>"#,
        r#"<draw:image xlink:href="./ObjectReplacements/Object 1"/></draw:frame>"#,
        r#"<draw:frame svg:x="18cm" svg:y="8cm" svg:width="8cm" svg:height="6cm"><draw:image xlink:href="./ObjectReplacements/Object 1"/></draw:frame>"#,
        r#"<draw:frame svg:width="1cm" svg:height="1cm"><draw:text-box><text:p>Senza posto</text:p></draw:text-box></draw:frame>"#,
        r#"<presentation:notes><draw:frame svg:x="0cm" svg:y="0cm" svg:width="1cm" svg:height="1cm"><draw:text-box><text:p>Note</text:p></draw:text-box></draw:frame></presentation:notes>"#,
        "</draw:page>"
    ));
    let slide = &presentation.slides[0];
    assert_eq!(
        slide.background.as_deref(),
        Some("102030"),
        "the first colour of the gradient"
    );
    let shapes = &slide.shapes;
    assert_eq!(
        shapes.len(),
        11,
        "no master shapes on this page: {shapes:#?}"
    );
    assert!(
        matches!(shapes[0].geometry, ShapeGeometry::Path(_)),
        "a DrawingML rounded rectangle drawn as its preset"
    );
    let body = text(&shapes[0]);
    assert_eq!(
        (body.anchor, body.wrap, body.insets_emu),
        (TextAnchor::Middle, false, [72_000; 4])
    );
    let paragraph = &body.paragraphs[0];
    assert_eq!(
        (
            paragraph.align,
            paragraph.line_spacing_percent,
            paragraph.space_after
        ),
        (Alignment::Right, 150, 565)
    );
    assert_eq!(shapes[1].geometry, ShapeGeometry::Ellipse);
    assert_eq!(shapes[2].geometry, ShapeGeometry::Other);
    let rotated = &shapes[3];
    assert!(
        (rotated.rotation - 270.0).abs() < 1e-6,
        "{}",
        rotated.rotation
    );
    assert_eq!(
        (rotated.frame.width, rotated.frame.height),
        (720_000, 360_000)
    );
    assert_eq!(
        (rotated.frame.x, rotated.frame.y),
        (1_620_000, 1_620_000),
        "{:?}",
        rotated.frame
    );
    assert_eq!(shapes[4].geometry, ShapeGeometry::Line { flipped: true });
    assert_eq!(
        shapes[4].frame,
        Frame {
            x: 360_000,
            y: 360_000,
            width: 1_080_000,
            height: 720_000
        }
    );
    assert_eq!(
        (shapes[5].geometry.clone(), shapes[6].geometry.clone()),
        (ShapeGeometry::RoundedRectangle, ShapeGeometry::Ellipse)
    );
    let ShapeContent::Table(table) = &shapes[7].content else {
        panic!("{:?}", shapes[7].content);
    };
    assert_eq!(table.column_widths_emu, [1_440_000, 1_440_000]);
    assert_eq!(table.rows[0].height_emu, 360_000);
    assert_eq!(
        (
            table.rows[0].cells[0].col_span,
            table.rows[0].cells[0].fill.as_deref()
        ),
        (2, Some("2F5597"))
    );
    assert!(table.rows[0].cells[1].covered);
    assert_eq!(table.rows[1].cells[0].fill.as_deref(), Some("D9E2F3"));
    assert_eq!(shapes[8].content, ShapeContent::Picture { media: 0 });
    assert_eq!(
        shapes[9].content,
        ShapeContent::Picture { media: 0 },
        "the image is loaded once"
    );
    assert_eq!(
        shapes[10].content,
        ShapeContent::Picture { media: 1 },
        "the chart, not its replacement"
    );
    assert_eq!(presentation.media[1].content_type, "image/svg+xml");
}

#[test]
fn draws_chart_objects_and_leaves_out_what_is_no_image() {
    let presentation = read(concat!(
        r#"<draw:page>"#,
        r#"<draw:frame svg:x="1cm" svg:y="1cm" svg:width="8cm" svg:height="6cm"><draw:object xlink:href="./Object 1"/>"#,
        r#"<draw:image xlink:href="./ObjectReplacements/Object 1"/></draw:frame>"#,
        r#"<draw:frame svg:x="10cm" svg:y="1cm" svg:width="8cm" svg:height="6cm"><draw:image xlink:href="./ObjectReplacements/Object 1"/></draw:frame>"#,
        r#"<draw:frame svg:x="10cm" svg:y="1cm" svg:width="8cm" svg:height="6cm"><draw:image xlink:href="https://example.com/x.png"/></draw:frame>"#,
        r#"<draw:frame svg:x="10cm" svg:y="1cm" svg:width="8cm" svg:height="6cm"><draw:object xlink:href="./Object 9"/></draw:frame>"#,
        "</draw:page>"
    ));
    let slide = &presentation.slides[0];
    assert_eq!(
        slide.shapes.len(),
        2,
        "the first master's bar, then the chart: {:#?}",
        slide.shapes
    );
    assert_eq!(slide.shapes[1].content, ShapeContent::Picture { media: 0 });
    assert_eq!(presentation.media[0].content_type, "image/svg+xml");
    assert_eq!(
        slide.background.as_deref(),
        Some("EEEEEE"),
        "the first master's background"
    );
}

#[test]
fn honours_the_limits_and_refuses_a_package_without_a_presentation() {
    let limits = Limits {
        max_text_chars: 3,
        ..Limits::default()
    };
    let bytes = odp(
        r#"<draw:page><draw:frame svg:x="0cm" svg:y="0cm" svg:width="1cm" svg:height="1cm"><draw:text-box><text:p>Lungo</text:p></draw:text-box></draw:frame></draw:page>"#,
    );
    let presentation = read_presentation(&bytes, &limits).unwrap();
    assert!(presentation.truncated);
    assert_eq!(
        joined(&text(presentation.slides[0].shapes.last().unwrap()).paragraphs[0]),
        "Lun"
    );
    let empty = build_zip(&[
        (
            "mimetype",
            b"application/vnd.oasis.opendocument.presentation",
        ),
        ("styles.xml", b"<x/>"),
    ]);
    assert!(matches!(
        read_presentation(&empty, &Limits::default()),
        Err(OfficeError::UnreadableDocument(_))
    ));
    let no_body = build_zip(&[
        ("mimetype", b"application/vnd.oasis.opendocument.presentation"),
        (
            "content.xml",
            br#"<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"><office:body/></office:document-content>"#,
        ),
    ]);
    assert!(matches!(
        read_presentation(&no_body, &Limits::default()),
        Err(OfficeError::UnreadableDocument(_))
    ));
    let bare = build_zip(&[
        ("mimetype", b"application/vnd.oasis.opendocument.presentation"),
        (
            "content.xml",
            br#"<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"><office:body><office:presentation/></office:body></office:document-content>"#,
        ),
    ]);
    let presentation = read_presentation(&bare, &Limits::default()).unwrap();
    assert_eq!(
        (presentation.width_emu, presentation.height_emu),
        DEFAULT_SIZE
    );
    assert_eq!(presentation.slides, Vec::<Slide>::new());
}

#[test]
fn reads_transforms_and_lengths() {
    let (angle, translate) =
        parse_transform("rotate (0.5) translate (1cm 2cm) skewX (1) rotate (0.25");
    assert!((angle - 0.5).abs() < 1e-9);
    assert_eq!(translate, (360_000.0, 720_000.0));
    assert_eq!(emu("1in"), Some(914_400));
    assert_eq!(emu("x"), None);
    assert_eq!(
        odf_object_part("./Object 1"),
        Some("Object 1/content.xml".to_owned())
    );
}

#[test]
fn draws_enhanced_geometries_polygons_and_paths_as_their_outline() {
    let page = concat!(
        r#"<draw:page draw:master-page-name="Default">"#,
        r#"<draw:custom-shape draw:style-name="gr1" svg:x="0cm" svg:y="0cm" svg:width="2cm" svg:height="2cm">"#,
        r#"<draw:enhanced-geometry svg:viewBox="0 0 21600 21600" draw:enhanced-path="M 0 0 L 21600 0 10800 21600 Z N"/></draw:custom-shape>"#,
        r#"<draw:custom-shape draw:style-name="gr1" svg:x="0cm" svg:y="0cm" svg:width="2cm" svg:height="1cm">"#,
        r#"<draw:enhanced-geometry draw:type="ooxml-rightArrow"/></draw:custom-shape>"#,
        r#"<draw:custom-shape draw:style-name="gr1" svg:x="0cm" svg:y="0cm" svg:width="2cm" svg:height="1cm">"#,
        r#"<draw:enhanced-geometry draw:type="mso-spt100"/></draw:custom-shape>"#,
        r#"<draw:polygon draw:style-name="gr1" svg:x="0cm" svg:y="0cm" svg:width="1cm" svg:height="1cm" svg:viewBox="0 0 10 10" draw:points="0,0 10,0 0,10"/>"#,
        r#"<draw:path draw:style-name="gr1" svg:x="0cm" svg:y="0cm" svg:width="1cm" svg:height="1cm" svg:viewBox="0 0 10 10" svg:d="M 0 0 L 10 10 Z"/>"#,
        r#"<draw:polyline draw:style-name="gr1" svg:x="0cm" svg:y="0cm" svg:width="1cm" svg:height="1cm" draw:points=""/>"#,
        "</draw:page>"
    );
    let presentation = read(page);
    let shapes = &presentation.slides[0].shapes;
    let shapes = &shapes[shapes.len() - 6..];
    let outline = |index: usize| match &shapes[index].geometry {
        ShapeGeometry::Path(paths) => paths[0].commands.clone(),
        other => panic!("{index}: {other:?}"),
    };
    assert_eq!(
        outline(0)[..3],
        [
            PathCommand::MoveTo(PathPoint { x: 0, y: 0 }),
            PathCommand::LineTo(PathPoint { x: 720_000, y: 0 }),
            PathCommand::LineTo(PathPoint {
                x: 360_000,
                y: 720_000
            }),
        ]
    );
    assert_eq!(outline(1).len(), 8, "the arrow of its DrawingML preset");
    assert_eq!(shapes[2].geometry, ShapeGeometry::Other);
    assert_eq!(
        outline(3)[1],
        PathCommand::LineTo(PathPoint { x: 360_000, y: 0 })
    );
    assert_eq!(
        outline(4)[1],
        PathCommand::LineTo(PathPoint {
            x: 360_000,
            y: 360_000
        })
    );
    assert_eq!(shapes[5].geometry, ShapeGeometry::Rectangle);
}
