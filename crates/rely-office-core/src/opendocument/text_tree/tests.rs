// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;
use crate::document::{Alignment, BorderStyle, LineSpacing, TabKind, VerticalAlignment};
use crate::read_document_tree;
use crate::test_support::build_zip;

const NAMESPACES: &str = concat!(
    r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
    r#"xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" "#,
    r#"xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" "#,
    r#"xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" "#,
    r#"xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" "#,
    r#"xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" "#,
    r#"xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" "#,
    r#"xmlns:xlink="http://www.w3.org/1999/xlink""#
);
const PNG: &[u8] = &[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 0, 0];

const STYLES: &str = concat!(
    r#"<office:font-face-decls><style:font-face style:name="Calibri1" svg:font-family="'Calibri'"/></office:font-face-decls>"#,
    "<office:styles>",
    r#"<style:default-style style:family="paragraph"><style:paragraph-properties style:tab-stop-distance="1.25cm"/>"#,
    r#"<style:text-properties style:font-name="Calibri1" fo:font-size="11pt"/></style:default-style>"#,
    r#"<style:style style:name="Standard" style:family="paragraph"><style:paragraph-properties fo:margin-bottom="0.2cm" fo:line-height="115%"/></style:style>"#,
    r#"<style:style style:name="Heading_20_1" style:display-name="Heading 1" style:family="paragraph" style:parent-style-name="Standard">"#,
    r#"<style:text-properties fo:font-size="18pt" fo:font-weight="bold"/></style:style>"#,
    r#"<style:style style:name="Quotations" style:family="paragraph" style:parent-style-name="Standard"/>"#,
    r#"<style:style style:name="Preformatted_20_Text" style:family="paragraph" style:parent-style-name="Standard"/>"#,
    r#"<style:style style:name="Strong" style:family="text"><style:text-properties fo:font-weight="700"/></style:style>"#,
    r#"<text:list-style style:name="Elenco"><text:list-level-style-bullet text:level="1" text:bullet-char="&#xF0B7;">"#,
    r#"<style:list-level-properties text:list-level-position-and-space-mode="label-alignment">"#,
    r#"<style:list-level-label-alignment text:label-followed-by="space" fo:margin-left="1.27cm" fo:text-indent="-0.635cm"/>"#,
    r#"</style:list-level-properties></text:list-level-style-bullet></text:list-style>"#,
    r#"<text:outline-style style:name="Outline"><text:outline-level-style text:level="1" style:num-format="1" style:num-suffix="."/>"#,
    r#"<text:outline-level-style text:level="2" style:num-format="1" text:display-levels="2"/></text:outline-style>"#,
    "</office:styles>",
    "<office:automatic-styles>",
    r#"<style:page-layout style:name="pm1"><style:page-layout-properties fo:page-width="29.7cm" fo:page-height="21cm" fo:margin-top="1cm" "#,
    r#"fo:margin-bottom="1.5cm" fo:margin-left="2cm" fo:margin-right="2cm"><style:columns fo:column-count="2" fo:column-gap="1cm"/></style:page-layout-properties>"#,
    r#"<style:header-style><style:header-footer-properties fo:min-height="0.5cm" fo:margin-bottom="0.25cm"/></style:header-style><style:footer-style/></style:page-layout>"#,
    r#"<style:style style:name="MP1" style:family="paragraph" style:parent-style-name="Standard"><style:paragraph-properties fo:text-align="end"/></style:style>"#,
    r#"<style:page-layout style:name="pm2"><style:page-layout-properties fo:page-width="21cm" fo:page-height="29.7cm"/></style:page-layout>"#,
    "</office:automatic-styles>",
    r#"<office:master-styles><style:master-page style:name="Standard" style:page-layout-name="pm1">"#,
    r#"<style:header><text:p text:style-name="MP1">Pagina <text:page-number>3</text:page-number> di <text:page-count>9</text:page-count></text:p></style:header>"#,
    r#"<style:header-first><text:p>Prima</text:p></style:header-first><style:footer style:display="false"><text:p>Nascosto</text:p></style:footer>"#,
    r#"<style:footer-left><text:p>Pari</text:p></style:footer-left>"#,
    "</style:master-page>",
    r#"<style:master-page style:name="Orizzontale" style:page-layout-name="pm2"><style:header><text:p>Larga</text:p></style:header></style:master-page>"#,
    "</office:master-styles>"
);

const AUTOMATIC: &str = concat!(
    "<office:automatic-styles>",
    r#"<style:style style:name="P1" style:family="paragraph" style:parent-style-name="Standard">"#,
    r#"<style:paragraph-properties fo:text-align="justify" fo:margin-left="1cm" fo:text-indent="0.5cm" fo:break-before="page" "#,
    r##"fo:background-color="#ffff00" fo:border="0.5pt solid #ff0000"><style:tab-stops><style:tab-stop style:position="2cm" style:type="right" style:leader-text="."/></style:tab-stops>"##,
    r#"</style:paragraph-properties></style:style>"#,
    r#"<style:style style:name="T1" style:family="text"><style:text-properties fo:font-style="italic" style:text-underline-style="solid" "#,
    r##"fo:color="#1f3864" fo:font-size="14pt" fo:background-color="#00ff00" style:text-position="super 58%"/></style:style>"##,
    r#"<style:style style:name="T2" style:family="text" style:parent-style-name="Strong"><style:text-properties style:text-line-through-style="solid" style:text-position="-33% 58%"/></style:style>"#,
    r#"<style:style style:name="L" style:family="paragraph" style:list-style-name="Elenco"/>"#,
    r#"<style:style style:name="Larga" style:family="paragraph" style:master-page-name="Orizzontale"/>"#,
    r#"<style:style style:name="Torna" style:family="paragraph" style:master-page-name="Standard"/>"#,
    r#"<style:style style:name="Vuota" style:family="paragraph" style:master-page-name=""/>"#,
    r#"<text:list-style style:name="Numeri"><text:list-level-style-number text:level="1" style:num-format="a" style:num-suffix=")" text:start-value="3"/>"#,
    r#"<text:list-level-style-number text:level="2" style:num-format="I" text:display-levels="2" style:num-prefix="("/></text:list-style>"#,
    r#"<style:style style:name="Tab.A" style:family="table-column"><style:table-column-properties style:column-width="3cm"/></style:style>"#,
    r##"<style:style style:name="Tab.C" style:family="table-cell"><style:table-cell-properties fo:background-color="#dddddd" "##,
    r#"fo:border="0.75pt double #808080" fo:border-top="none" fo:padding="0.1cm" style:vertical-align="middle"/></style:style>"#,
    r#"<style:style style:name="Fr" style:family="graphic"><style:graphic-properties fo:border="0.5pt solid #00ff00" fo:margin-left="0.1in"/></style:style>"#,
    r##"<style:style style:name="gr1" style:family="graphic" style:parent-style-name="Fr"><style:graphic-properties draw:stroke="solid" svg:stroke-width="2pt" svg:stroke-color="#bf9000" draw:fill-color="#fff2cc" style:wrap="run-through" style:run-through="foreground" style:horizontal-rel="page-content" style:vertical-rel="char"/></style:style>"##,
    r##"<style:style style:name="fr2" style:family="graphic"><style:graphic-properties style:wrap="run-through" style:run-through="background" style:horizontal-rel="page-start-margin" style:vertical-rel="page-content" style:vertical-pos="middle" draw:fill="none" fo:background-color="#ff0000" fo:border="none"/></style:style>"##,
    r#"<style:style style:name="fr3" style:family="graphic" style:parent-style-name="Fr"><style:graphic-properties style:wrap="parallel" style:horizontal-pos="right" style:horizontal-rel="paragraph" style:vertical-pos="from-top"/></style:style>"#,
    r##"<style:style style:name="fr4" style:family="graphic"><style:graphic-properties draw:stroke="dash" svg:stroke-color="nonsense" fo:background-color="#336699" style:vertical-rel="paragraph-content"/></style:style>"##,
    "</office:automatic-styles>"
);

fn odt(body: &str, extra: &[(&str, &[u8])]) -> Vec<u8> {
    let content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content {NAMESPACES}>{AUTOMATIC}<office:body><office:text>{body}</office:text></office:body></office:document-content>"#
    );
    let styles = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-styles {NAMESPACES}>{STYLES}</office:document-styles>"#
    );
    let mut members: Vec<(&str, &[u8])> = vec![
        ("mimetype", b"application/vnd.oasis.opendocument.text"),
        ("content.xml", content.as_bytes()),
        ("styles.xml", styles.as_bytes()),
    ];
    members.extend_from_slice(extra);
    build_zip(&members)
}

fn tree(body: &str) -> DocumentTree {
    read_document_tree(&odt(body, &[]), &Limits::default()).unwrap()
}

fn texts(content: &[Inline]) -> String {
    content
        .iter()
        .map(|inline| match inline {
            Inline::Text { text, .. } => text.clone(),
            Inline::HardBreak => "\n".to_owned(),
            Inline::Image { .. } => "[image]".to_owned(),
            Inline::NoteReference { note } => format!("[{note}]"),
            Inline::CommentAnchor { comment } => format!("[comment {comment}]"),
            Inline::Math { latex, .. } => format!("[math {latex}]"),
        })
        .collect()
}

fn paragraph(block: &Block) -> (&[Inline], Option<Alignment>, &ParagraphLook) {
    match block {
        Block::Paragraph {
            content,
            align,
            look,
        }
        | Block::Heading {
            content,
            align,
            look,
            ..
        } => (content, *align, look),
        Block::ListItem { content, look, .. } => (content, None, look),
        other => panic!("not a paragraph: {other:?}"),
    }
}

#[test]
fn reads_paragraphs_with_their_styles_spaces_and_runs() {
    let read = tree(concat!(
        r#"<text:p text:style-name="P1">  Un   testo <text:s text:c="2"/>con<text:tab/>tab<text:line-break/>"#,
        r#"<text:span text:style-name="T1">alto</text:span> e <text:span text:style-name="T2">basso</text:span>   </text:p>"#,
        r#"<text:p><text:a xlink:href="https://relybytes.com"><text:span text:style-name="T1">sito</text:span></text:a> "#,
        r#"<text:a xlink:href="javascript:alert(1)">no</text:a></text:p>"#
    ));
    let (content, align, look) = paragraph(&read.blocks[0]);
    assert_eq!(texts(content), "Un testo   con\ttab\nalto e basso");
    assert_eq!(align, Some(Alignment::Justify));
    assert_eq!(
        (look.indent_left_twips, look.indent_first_line_twips),
        (Some(567), Some(283))
    );
    assert_eq!(look.space_after_twips, Some(113), "from Standard");
    assert_eq!(look.line_spacing, Some(LineSpacing::Auto(276)));
    assert!(look.page_break_before);
    assert_eq!(look.shading.as_deref(), Some("FFFF00"));
    assert_eq!(
        look.borders.top.as_ref().map(|border| border.width_eighths),
        Some(4)
    );
    assert_eq!(
        look.tabs[0].position_twips,
        567 + 1_134,
        "counted from the indent"
    );
    assert_eq!(look.tabs[0].kind, TabKind::Right);
    assert_eq!(look.font.family.as_deref(), Some("Calibri"));
    assert_eq!(look.font.size_half_points, Some(22));
    let Inline::Text {
        marks, look: run, ..
    } = &content[2]
    else {
        panic!("{:?}", content[2]);
    };
    assert_eq!(
        marks,
        &[
            Mark::Italic,
            Mark::Underline,
            Mark::Superscript,
            Mark::TextStyle {
                font_family: None,
                font_size: Some("14pt".to_owned()),
                color: Some("#1F3864".to_owned()),
            },
            Mark::Highlight {
                color: "#00FF00".to_owned()
            },
        ]
    );
    assert_eq!(run.italic, None, "italic is a mark, not a look");
    let Inline::Text { marks, .. } = &content[4] else {
        panic!("{:?}", content[4]);
    };
    assert_eq!(
        marks,
        &[Mark::Bold, Mark::Strike, Mark::Subscript],
        "through the parent text style"
    );

    let (content, _, _) = paragraph(&read.blocks[1]);
    assert_eq!(texts(content), "sito no");
    let Inline::Text { marks, .. } = &content[0] else {
        panic!();
    };
    assert!(marks.contains(&Mark::Link {
        href: "https://relybytes.com".to_owned()
    }));
    assert!(
        !marks.contains(&Mark::Underline),
        "a link draws its own underline"
    );
    assert_eq!(read.report.unsupported_links, 1);
}

#[test]
fn reads_headings_with_outline_numbers_quotes_and_code() {
    let read = tree(concat!(
        r#"<text:h text:style-name="Heading_20_1" text:outline-level="1">Uno</text:h>"#,
        r#"<text:h text:outline-level="2">Uno punto uno</text:h>"#,
        r#"<text:h text:outline-level="9">Profondo</text:h>"#,
        r#"<text:p text:style-name="Quotations">Citata</text:p><text:p text:style-name="Quotations">ancora</text:p>"#,
        r#"<text:p text:style-name="Preformatted_20_Text">let a = 1;</text:p><text:p text:style-name="Preformatted_20_Text">let b = 2;</text:p>"#
    ));
    let Block::Heading { level, look, .. } = &read.blocks[0] else {
        panic!("{:?}", read.blocks[0]);
    };
    assert_eq!(*level, 1);
    assert_eq!(
        look.list_label.as_ref().map(|label| label.text.as_str()),
        Some("1.")
    );
    assert_eq!(look.font.bold, Some(true));
    assert_eq!(look.font.size_half_points, Some(36));
    let Block::Heading { look, .. } = &read.blocks[1] else {
        panic!();
    };
    assert_eq!(
        look.list_label.as_ref().map(|label| label.text.as_str()),
        Some("1.1")
    );
    assert!(
        matches!(read.blocks[2], Block::Heading { level: 6, .. }),
        "clamped to six"
    );
    let Block::Blockquote { blocks } = &read.blocks[3] else {
        panic!("{:?}", read.blocks[3]);
    };
    assert_eq!(blocks.len(), 2, "consecutive quotes together");
    assert_eq!(
        read.blocks[4],
        Block::CodeBlock {
            text: "let a = 1;\nlet b = 2;".to_owned()
        }
    );
}

#[test]
fn numbers_lists_with_their_styles_levels_and_continuations() {
    let read = tree(concat!(
        r#"<text:list text:style-name="Numeri"><text:list-item><text:p>Primo</text:p><text:p>seguito</text:p>"#,
        r#"<text:list><text:list-item><text:p>Annidato</text:p></text:list-item><text:list-item><text:p>Annidato 2</text:p></text:list-item></text:list>"#,
        r#"</text:list-item><text:list-item><text:p>Secondo</text:p></text:list-item><text:list-header><text:p>Intestazione</text:p></text:list-header></text:list>"#,
        r#"<text:p>Fra</text:p>"#,
        r#"<text:list text:style-name="Numeri" text:continue-numbering="true"><text:list-item><text:p>Terzo</text:p></text:list-item></text:list>"#,
        r#"<text:list><text:list-item text:start-value="7"><text:p text:style-name="L">Puntato</text:p></text:list-item></text:list>"#
    ));
    let labels: Vec<(String, Option<String>, u8, u32, bool)> = read
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::ListItem {
                content,
                look,
                level,
                list_id,
                ordered,
            } => Some((
                texts(content),
                look.list_label.as_ref().map(|label| label.text.clone()),
                *level,
                *list_id,
                *ordered,
            )),
            _ => None,
        })
        .collect();
    let label = |text: &str| Some(text.to_owned());
    assert_eq!(
        labels,
        [
            ("Primo".to_owned(), label("c)"), 0, 0, true),
            ("seguito".to_owned(), None, 0, 0, true),
            ("Annidato".to_owned(), label("(c.I"), 1, 0, true),
            ("Annidato 2".to_owned(), label("(c.II"), 1, 0, true),
            ("Secondo".to_owned(), label("d)"), 0, 0, true),
            ("Intestazione".to_owned(), None, 0, 0, true),
            ("Terzo".to_owned(), label("e)"), 0, 0, true),
            ("Puntato".to_owned(), label("\u{2022}"), 0, 1, false),
        ]
    );
    let bullet = read
        .blocks
        .iter()
        .rev()
        .find_map(|block| match block {
            Block::ListItem { look, .. } => Some(look),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        (bullet.indent_left_twips, bullet.indent_first_line_twips),
        (Some(720), Some(-360))
    );
    assert_eq!(
        bullet.list_label.as_ref().unwrap().suffix,
        LabelSuffix::Space
    );
}

#[test]
fn reads_tables_with_spans_headers_and_cell_looks() {
    let read = tree(concat!(
        r#"<table:table><table:table-column table:style-name="Tab.A" table:number-columns-repeated="2"/>"#,
        r#"<table:table-header-rows><table:table-row><table:table-cell table:style-name="Tab.C" table:number-columns-spanned="2">"#,
        r#"<text:p>Testata</text:p></table:table-cell><table:covered-table-cell/></table:table-row></table:table-header-rows>"#,
        r#"<table:table-row table:number-rows-repeated="2"><table:table-cell><text:p>A</text:p></table:table-cell>"#,
        r#"<table:table-cell table:number-columns-repeated="1"><text:p>B</text:p></table:table-cell></table:table-row>"#,
        r#"<table:table-row table:number-rows-repeated="1000"><table:table-cell/><table:table-cell/></table:table-row>"#,
        r#"</table:table><table:table/>"#
    ));
    assert_eq!(read.blocks.len(), 1, "an empty table is left out");
    let Block::Table { rows, look } = &read.blocks[0] else {
        panic!("{:?}", read.blocks[0]);
    };
    assert_eq!(look.column_widths_twips, [1_701, 1_701]);
    assert!(look.borders);
    assert_eq!(look.cell_margins_twips, [Some(57); 4]);
    assert_eq!(
        rows.len(),
        4,
        "two repeated rows and one empty row of the thousand"
    );
    assert!(rows[0].header && !rows[1].header);
    let header = &rows[0].cells[0];
    assert_eq!(header.col_span, 2);
    assert_eq!(
        rows[0].cells.len(),
        1,
        "the covered cell is folded into the span"
    );
    assert_eq!(header.look.fill.as_deref(), Some("DDDDDD"));
    assert_eq!(header.look.vertical, Some(VerticalAlignment::Center));
    assert_eq!(
        header.look.borders.top.as_ref().map(|border| border.style),
        Some(BorderStyle::None)
    );
    assert_eq!(
        header.look.borders.left.as_ref().map(|border| border.style),
        Some(BorderStyle::Double)
    );
    assert_eq!(rows[1], rows[2]);
}

#[test]
fn reads_notes_images_text_boxes_and_fields() {
    let read = read_document_tree(
        &odt(
            concat!(
                r#"<text:p>Nota<text:note text:note-class="footnote"><text:note-citation>1</text:note-citation>"#,
                r#"<text:note-body><text:p>Testo della nota</text:p></text:note-body></text:note>"#,
                r#" e fine<text:note text:note-class="endnote"><text:note-body><text:p>Finale</text:p></text:note-body></text:note></text:p>"#,
                r#"<text:p><draw:frame draw:name="Logo" svg:width="2cm" svg:height="1cm"><draw:image xlink:href="Pictures/logo.png"/>"#,
                r#"<svg:title>Il logo</svg:title></draw:frame><draw:frame><draw:image xlink:href="Pictures/logo.png"/></draw:frame>"#,
                r#"<draw:frame><draw:image xlink:href="https://example.com/a.png"/></draw:frame>"#,
                r#"<draw:frame><draw:image xlink:href="Pictures/missing.png"/></draw:frame>"#,
                r#"<draw:frame><draw:text-box><text:p>In una casella</text:p></draw:text-box></draw:frame>"#,
                r#"<draw:frame><draw:object/></draw:frame><draw:custom-shape/><draw:rect><text:p>Forma</text:p></draw:rect>"#,
                r#"<text:date>27/10/2026</text:date> <text:hidden-text text:is-hidden="true">x</text:hidden-text>"#,
                r#"<office:annotation><text:p>Commento</text:p></office:annotation></text:p>"#,
                r#"<draw:frame svg:width="1cm" svg:height="1cm"><draw:image xlink:href="./Pictures/logo.png"/></draw:frame>"#,
                r#"<text:section><text:p>Sezione</text:p></text:section>"#,
                r#"<text:table-of-content><text:index-body><text:index-title><text:p>Indice</text:p></text:index-title></text:index-body></text:table-of-content>"#
            ),
            &[("Pictures/logo.png", PNG)],
        ),
        &Limits::default(),
    )
    .unwrap();
    let (content, _, _) = paragraph(&read.blocks[0]);
    assert_eq!(texts(content), "Nota[0] e fine[1]");
    assert_eq!(read.notes.len(), 2);
    assert_eq!(read.notes[0].kind, NoteKind::Footnote);
    assert_eq!(read.notes[1].kind, NoteKind::Endnote);
    assert_eq!(
        texts(paragraph(&read.notes[0].blocks[0]).0),
        "Testo della nota"
    );
    let (content, _, _) = paragraph(&read.blocks[1]);
    assert_eq!(
        content[0],
        Inline::Image {
            media: 0,
            alt: Some("Il logo".to_owned()),
            width: Some(76),
            height: Some(38),
            anchor: None,
        }
    );
    assert!(
        matches!(content[1], Inline::Image { media: 0, .. }),
        "one image, loaded once"
    );
    assert_eq!(read.media.len(), 1);
    assert_eq!(texts(&content[2..]), "27/10/2026 [comment 0]");
    assert_eq!(read.comments.len(), 1);
    assert_eq!(texts(paragraph(&read.comments[0].blocks[0]).0), "Commento");
    assert_eq!(texts(paragraph(&read.blocks[2]).0), "In una casella");
    assert_eq!(texts(paragraph(&read.blocks[3]).0), "Forma");
    assert!(
        matches!(
            paragraph(&read.blocks[4]).0[0],
            Inline::Image { media: 0, .. }
        ),
        "a frame anchored to the page"
    );
    assert_eq!(texts(paragraph(&read.blocks[5]).0), "Sezione");
    assert_eq!(texts(paragraph(&read.blocks[6]).0), "Indice");
    let report = &read.report;
    assert_eq!(
        (report.footnotes, report.endnotes, report.text_boxes),
        (1, 1, 2)
    );
    assert_eq!(
        (
            report.missing_images,
            report.embedded_objects,
            report.shapes
        ),
        (2, 1, 1)
    );
    assert_eq!(
        (
            report.fields_kept_as_text,
            report.hidden_runs,
            report.comments
        ),
        (1, 1, 1)
    );
}

#[test]
fn takes_the_page_headers_and_footers_of_the_master_page() {
    let read = tree("<text:p>Corpo</text:p>");
    let layout = &read.layout;
    assert_eq!((layout.width_twips, layout.height_twips), (16_838, 11_906));
    assert_eq!(
        layout.margins_twips,
        [567 + 283 + 142, 1_134, 850, 1_134],
        "the header's room on top"
    );
    assert_eq!(
        (layout.header_distance_twips, layout.footer_distance_twips),
        (567, 850)
    );
    assert_eq!((layout.columns, layout.column_gap_twips), (2, 567));
    assert_eq!(layout.default_tab_twips, 709);
    assert_eq!(layout.font_family.as_deref(), Some("Calibri"));
    let headers = &read.headers_footers;
    let (content, align, _) = paragraph(&headers.header[0]);
    assert_eq!(texts(content), "Pagina 3 di 9");
    assert_eq!(
        align,
        Some(Alignment::Right),
        "the master's own automatic style"
    );
    let Inline::Text { look, .. } = &content[1] else {
        panic!();
    };
    assert_eq!(look.field, Some(PageField::Page));
    assert!(headers.footer.is_empty(), "a footer that is not displayed");
    assert_eq!(
        texts(paragraph(&headers.first_header.as_ref().unwrap()[0]).0),
        "Prima"
    );
    assert_eq!(
        texts(paragraph(&headers.even_footer.as_ref().unwrap()[0]).0),
        "Pari"
    );
    assert_eq!(read.report.headers_and_footers, 3);
}

#[test]
fn honours_the_text_limit_and_refuses_a_package_without_content() {
    let limits = Limits {
        max_text_chars: 5,
        ..Limits::default()
    };
    let read = read_document_tree(
        &odt("<text:p>Lungo testo</text:p><text:p>Altro</text:p>", &[]),
        &limits,
    )
    .unwrap();
    assert!(read.report.truncated);
    assert_eq!(texts(paragraph(&read.blocks[0]).0), "Lungo");
    assert_eq!(read.blocks.len(), 1);
    let empty = build_zip(&[
        ("mimetype", b"application/vnd.oasis.opendocument.text"),
        ("styles.xml", b"<x/>"),
    ]);
    assert!(matches!(
        read_document_tree(&empty, &Limits::default()),
        Err(OfficeError::UnreadableDocument(_))
    ));
    let no_text = build_zip(&[
        ("mimetype", b"application/vnd.oasis.opendocument.text"),
        (
            "content.xml",
            br#"<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"><office:body/></office:document-content>"#,
        ),
    ]);
    assert!(matches!(
        read_document_tree(&no_text, &Limits::default()),
        Err(OfficeError::UnreadableDocument(_))
    ));
}

#[test]
fn collapses_whitespace_as_opendocument_lays_it_out() {
    let mut state = TextState { space_before: true };
    assert_eq!(collapse("  a \n\t b  ", &mut state), "a b ");
    assert!(state.space_before);
    assert_eq!(collapse(" c", &mut state), "c");
    let mut content = vec![
        Inline::Text {
            text: "fine  ".to_owned(),
            marks: Vec::new(),
            look: RunLook::default(),
        },
        Inline::Text {
            text: " ".to_owned(),
            marks: vec![Mark::Bold],
            look: RunLook::default(),
        },
    ];
    trim_end(&mut content);
    assert_eq!(texts(&content), "fine");
}

#[test]
fn draws_a_chart_object_as_an_image() {
    const CHART: &str = concat!(
        r#"<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
        r#"xmlns:chart="urn:oasis:names:tc:opendocument:xmlns:chart:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" "#,
        r#"xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"><office:body><office:chart><chart:chart chart:class="chart:line">"#,
        r#"<chart:title><text:p>Andamento</text:p></chart:title><chart:plot-area><chart:series chart:values-cell-range-address="local-table.$A$1:.$A$1"/></chart:plot-area>"#,
        r#"<table:table table:name="local-table"><table:table-rows><table:table-row><table:table-cell office:value-type="float" office:value="3"/>"#,
        "</table:table-row></table:table-rows></table:table></chart:chart></office:chart></office:body></office:document-content>"
    );
    let read = read_document_tree(
        &odt(
            concat!(
                r#"<text:p><draw:frame svg:width="8cm" svg:height="6cm"><draw:object xlink:href="./Object 1"/></draw:frame>"#,
                r#"<draw:frame svg:width="8cm" svg:height="6cm"><draw:object xlink:href="./Object 2"/></draw:frame>"#,
                r#"<draw:frame><draw:object xlink:href="./Object 1"/></draw:frame></text:p>"#
            ),
            &[("Object 1/content.xml", CHART.as_bytes())],
        ),
        &Limits::default(),
    )
    .unwrap();
    let (content, _, _) = paragraph(&read.blocks[0]);
    assert_eq!(
        content,
        &[Inline::Image {
            media: 0,
            alt: Some("Andamento".to_owned()),
            width: Some(302),
            height: Some(227),
            anchor: None,
        }]
    );
    assert_eq!(read.media[0].content_type, "image/svg+xml");
    assert_eq!(
        (read.report.charts, read.report.embedded_objects),
        (1, 2),
        "an object that is no chart, and one without a size"
    );
}

#[test]
fn starts_a_section_where_a_paragraph_names_another_master_page() {
    let read = tree(concat!(
        "<text:p>Uno</text:p>",
        r#"<text:p text:style-name="Larga">Due</text:p><text:p text:style-name="Vuota">ancora</text:p>"#,
        r#"<text:p text:style-name="Torna">Tre</text:p>"#
    ));
    let sections = &read.sections;
    assert_eq!(
        sections
            .iter()
            .map(|section| section.end_block)
            .collect::<Vec<_>>(),
        [1, 3, 4]
    );
    assert_eq!(
        (
            sections[0].layout.width_twips,
            sections[1].layout.width_twips
        ),
        (16_838, 11_906)
    );
    assert_eq!(
        texts(paragraph(&sections[1].headers_footers.header[0]).0),
        "Larga"
    );
    assert_eq!(
        sections[2].layout, sections[0].layout,
        "back to the first master"
    );
    assert_eq!(read.layout, sections[2].layout);
    assert_eq!(
        read.report.headers_and_footers, 4,
        "each master page read once"
    );
}

mod frames;
