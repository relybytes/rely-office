// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

fn read(html: &str) -> DocumentTree {
    read_html_tree(html, &Limits::default()).unwrap()
}

fn text(content: &str, marks: Vec<Mark>) -> Inline {
    Inline::Text {
        text: content.to_owned(),
        marks,
        look: RunLook::default(),
    }
}

fn paragraph(content: Vec<Inline>, align: Option<Alignment>) -> Block {
    Block::Paragraph {
        content,
        align,
        look: ParagraphLook::default(),
    }
}

#[test]
fn reads_the_text_and_drops_what_is_not_text() {
    let tree = read(
        "<!DOCTYPE html><html><head><title>T</title><style>p{color:red}</style>\
         <script>alert(1)</script></head><body><h2 style=\"text-align:center\">Titolo</h2>\
         <p>Uno   <b>grassetto</b> e <i>corsivo</i>\n con <a href=\"https://relybytes.com\">link</a>\
         <a href=\"javascript:x()\">e no</a><br>a capo &amp; &egrave;</p><iframe src=x>nascosto</iframe>\
         <div>Libero <span style=\"font-weight:bold;color:#c00;font-size:16px\">misto</span></div></body></html>",
    );
    assert_eq!(
        tree.blocks[0],
        Block::Heading {
            level: 2,
            content: vec![text("Titolo", Vec::new())],
            align: Some(Alignment::Center),
            look: ParagraphLook::default(),
        }
    );
    assert_eq!(
        tree.blocks[1],
        paragraph(
            vec![
                text("Uno ", Vec::new()),
                text("grassetto", vec![Mark::Bold]),
                text(" e ", Vec::new()),
                text("corsivo", vec![Mark::Italic]),
                text(" con ", Vec::new()),
                text(
                    "link",
                    vec![Mark::Link {
                        href: "https://relybytes.com".to_owned()
                    }]
                ),
                text("e no", Vec::new()),
                Inline::HardBreak,
                text("a capo & è", Vec::new()),
            ],
            None
        )
    );
    assert_eq!(
        tree.blocks[2],
        paragraph(
            vec![
                text("Libero ", Vec::new()),
                text(
                    "misto",
                    vec![
                        Mark::Bold,
                        Mark::TextStyle {
                            font_family: None,
                            font_size: Some("12pt".to_owned()),
                            color: Some("#CC0000".to_owned()),
                        }
                    ]
                ),
            ],
            None
        )
    );
    assert_eq!(tree.blocks.len(), 3);
}

#[test]
fn reads_lists_tables_quotes_code_and_images() {
    let image = format!("data:image/png;base64,{}", crate::base64::encode(PNG));
    let tree = read(&format!(
        "<ul><li>uno<ul><li>dentro</li></ul></li><li>due</ol>\
         <ol><li>primo</ol><table><thead><tr><th>A<th>B</thead>\
         <tr><td colspan=2 align=right>larga</td></tr></table>\
         <blockquote><p>citato</p></blockquote><pre>\n  riga uno\n  <b>riga</b> due\n</pre><hr>\
         <p><img src=\"{image}\" alt=\"logo\" width=\"40\"><img src=\"https://relybytes.com/a.png\" alt=\"fuori\"></p>"
    ));
    let items: Vec<(bool, u8, String)> = tree
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::ListItem {
                ordered,
                level,
                content,
                ..
            } => Some((
                *ordered,
                *level,
                content
                    .iter()
                    .filter_map(|inline| match inline {
                        Inline::Text { text, .. } => Some(text.clone()),
                        _ => None,
                    })
                    .collect(),
            )),
            _ => None,
        })
        .collect();
    assert_eq!(
        items,
        [
            (false, 0, "uno".to_owned()),
            (false, 1, "dentro".to_owned()),
            (false, 0, "due".to_owned()),
            (true, 0, "primo".to_owned()),
        ]
    );
    let table = tree
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Table { rows, .. } => Some(rows),
            _ => None,
        })
        .unwrap();
    assert!(table[0].header);
    assert_eq!(table[0].cells.len(), 2);
    assert_eq!(table[1].cells[0].col_span, 2);
    assert_eq!(
        table[1].cells[0].blocks,
        [paragraph(
            vec![text("larga", Vec::new())],
            Some(Alignment::Right)
        )]
    );
    assert!(tree.blocks.contains(&Block::Blockquote {
        blocks: vec![paragraph(vec![text("citato", Vec::new())], None)]
    }));
    assert!(tree.blocks.contains(&Block::CodeBlock {
        text: "  riga uno\n  riga due".to_owned()
    }));
    assert!(tree.blocks.contains(&Block::HorizontalRule));
    let Some(Block::Paragraph { content, .. }) = tree.blocks.last() else {
        panic!("{:?}", tree.blocks.last());
    };
    assert_eq!(
        content[0],
        Inline::Image {
            media: 0,
            alt: Some("logo".to_owned()),
            width: Some(40),
            height: None,
            anchor: None,
        }
    );
    assert_eq!(
        content[1],
        text(
            "fuori",
            vec![Mark::Link {
                href: "https://relybytes.com/a.png".to_owned()
            }]
        )
    );
}

#[test]
fn refuses_a_page_over_the_limit() {
    let limits = Limits {
        max_input_bytes: 4,
        ..Limits::default()
    };
    assert!(matches!(
        read_html_tree("<p>troppo</p>", &limits),
        Err(OfficeError::TooLarge { .. })
    ));
}

#[test]
fn writes_a_page_that_reads_back_to_the_same_tree() {
    let markdown = format!(
        "# Titolo\n\nUn **grassetto *e corsivo***, ~~barrato~~, `codice`, <u>sotto</u>, <mark>evidenza</mark> e [link](https://relybytes.com).\\\nA capo con 3 < 4 & altro.\n\n- uno\n  - dentro\n- due\n\n1. primo\n2. secondo\n\n> citato\n\n```\ncodice <b>\n```\n\n| A | B |\n| --- | ---: |\n| x | 2 |\n\nNota[^1] e $x^2$ ![logo](data:image/png;base64,{})\n\n---\n\n[^1]: Il testo della nota.\n",
        crate::base64::encode(PNG)
    );
    let mut tree = super::super::read_markdown_tree(&markdown, &Limits::default()).unwrap();
    if let Some(Block::Paragraph { content, .. }) = tree.blocks.get_mut(1) {
        content.push(text(
            "colore",
            vec![Mark::TextStyle {
                font_family: Some("Georgia".to_owned()),
                font_size: Some("14pt".to_owned()),
                color: Some("#1F3864".to_owned()),
            }],
        ));
    }
    let page = write_html(&tree, "Prova <1>");
    assert!(page.starts_with("<!DOCTYPE html>"));
    assert!(page.contains("<title>Prova &lt;1&gt;</title>"));
    let again = read(&page);
    assert_eq!(again.blocks, tree.blocks, "{page}");
    assert_eq!(again.notes, tree.notes);
    assert_eq!(again.media, tree.media);
    assert_eq!(write_html(&again, "Prova <1>"), page);
}

#[test]
fn reads_colours_and_lengths_as_the_tree_writes_them() {
    assert_eq!(css_colour("#abc").as_deref(), Some("#AABBCC"));
    assert_eq!(css_colour("rgb(255, 0, 10)").as_deref(), Some("#FF000A"));
    assert_eq!(css_colour("navy").as_deref(), Some("#000080"));
    assert_eq!(css_colour("#12345"), None);
    assert_eq!(css_colour("hsl(1,2,3)"), None);
    assert_eq!(css_points("11px").as_deref(), Some("8.5pt"));
    assert_eq!(css_points("12pt").as_deref(), Some("12pt"));
    assert_eq!(css_points("1em"), None);
    assert_eq!(
        first_family("\"Times New Roman\", serif"),
        "Times New Roman"
    );
}

#[test]
fn a_floating_image_floats_in_the_page() {
    use crate::document::{Anchor, AnchorAlign, AnchorBase, AnchorPosition, Media, Wrap};
    let floating = |wrap: Wrap, align: Option<AnchorAlign>| DocumentTree {
        blocks: vec![Block::Paragraph {
            content: vec![Inline::Image {
                media: 0,
                alt: None,
                width: Some(10),
                height: Some(10),
                anchor: Some(Box::new(Anchor {
                    horizontal: AnchorPosition {
                        base: AnchorBase::Column,
                        offset_emu: 0,
                        align,
                    },
                    vertical: AnchorPosition {
                        base: AnchorBase::Paragraph,
                        offset_emu: 0,
                        align: None,
                    },
                    width_emu: 95_250,
                    height_emu: 95_250,
                    wrap,
                    distance_emu: [0; 4],
                })),
            }],
            align: None,
            look: ParagraphLook::default(),
        }],
        media: vec![Media {
            content_type: "image/png",
            bytes: vec![0x89, b'P', b'N', b'G'],
        }],
        ..DocumentTree::default()
    };
    let page = |tree: &DocumentTree| write_html(tree, "Pagina");
    assert!(page(&floating(Wrap::Around, Some(AnchorAlign::End))).contains("float:right"));
    assert!(page(&floating(Wrap::Behind, Some(AnchorAlign::Start))).contains("float:left"));
    assert!(
        page(&floating(Wrap::TopAndBottom, Some(AnchorAlign::Center)))
            .contains("margin:0.5em auto")
    );
    assert!(page(&floating(Wrap::TopAndBottom, None)).contains("display:block;margin:0.5em 0\""));
}
