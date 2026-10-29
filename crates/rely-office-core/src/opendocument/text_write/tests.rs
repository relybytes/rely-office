// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;
use crate::document::{
    CellLook, Note, RunLook, Section, TableLook, TrackedChanges, TreeOptions,
    read_document_tree_with,
};
use crate::{DocumentFormat, Limits, detect_format, read_document_tree};

const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0,
    0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

fn text(value: &str, marks: Vec<Mark>) -> Inline {
    Inline::Text {
        text: value.to_owned(),
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

fn item(ordered: bool, level: u8, value: &str) -> Block {
    Block::ListItem {
        ordered,
        level,
        list_id: 1,
        content: vec![text(value, Vec::new())],
        look: ParagraphLook::default(),
    }
}

fn cell(value: &str, col_span: u32, row_span: u32) -> TableCell {
    TableCell {
        col_span,
        row_span,
        blocks: vec![paragraph(vec![text(value, Vec::new())], None)],
        look: CellLook::default(),
    }
}

fn texts(blocks: &[Block]) -> Vec<String> {
    let mut found = Vec::new();
    for block in blocks {
        match block {
            Block::Heading { content, .. }
            | Block::Paragraph { content, .. }
            | Block::ListItem { content, .. } => found.push(
                content
                    .iter()
                    .filter_map(|inline| match inline {
                        Inline::Text { text, .. } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect(),
            ),
            Block::Table { rows, .. } => {
                for row in rows {
                    for cell in &row.cells {
                        found.extend(texts(&cell.blocks));
                    }
                }
            }
            Block::Blockquote { blocks } => found.extend(texts(blocks)),
            Block::CodeBlock { text } => found.push(text.clone()),
            Block::HorizontalRule => {}
        }
    }
    found
}

#[expect(
    clippy::too_many_lines,
    reason = "one document with every kind of content"
)]
fn rich_tree() -> DocumentTree {
    let link = Mark::Link {
        href: "https://relybytes.com".to_owned(),
    };
    let blocks = vec![
        Block::Heading {
            level: 1,
            content: vec![text("Contratto", Vec::new())],
            align: Some(Alignment::Center),
            look: ParagraphLook::default(),
        },
        Block::Paragraph {
            content: vec![
                text("Il ", Vec::new()),
                text(
                    "committente",
                    vec![Mark::Bold, Mark::Comment { comment: 0 }],
                ),
                text(" e ", vec![Mark::Comment { comment: 0 }]),
                text("corsivo", vec![Mark::Italic]),
                text(", ", Vec::new()),
                text("sotto", vec![Mark::Underline]),
                text("barrato", vec![Mark::Strike]),
                text("2", vec![Mark::Superscript]),
                text("x", vec![Mark::Subscript]),
                text(
                    "rosso",
                    vec![Mark::TextStyle {
                        font_family: Some("Georgia".to_owned()),
                        font_size: Some("14pt".to_owned()),
                        color: Some("#C00000".to_owned()),
                    }],
                ),
                text(
                    "evidenziato",
                    vec![Mark::Highlight {
                        color: "#FFFF00".to_owned(),
                    }],
                ),
                text("sito", vec![link]),
                text("codice()", vec![Mark::Code]),
                Inline::HardBreak,
                text("due  spazi\tcon tab", Vec::new()),
                Inline::NoteReference { note: 0 },
                Inline::Image {
                    media: 0,
                    alt: Some("Logo".to_owned()),
                    width: Some(40),
                    height: Some(20),
                    anchor: None,
                },
                Inline::CommentAnchor { comment: 2 },
            ],
            align: Some(Alignment::Justify),
            look: ParagraphLook {
                space_before_twips: Some(240),
                space_after_twips: Some(120),
                indent_left_twips: Some(720),
                indent_first_line_twips: Some(-360),
                line_spacing: Some(LineSpacing::Auto(360)),
                ..ParagraphLook::default()
            },
        },
        item(false, 0, "uno"),
        item(true, 1, "dentro"),
        item(false, 0, "due"),
        Block::Table {
            rows: vec![
                TableRow {
                    header: true,
                    cells: vec![
                        cell("Voce", 1, 1),
                        cell("Importo", 1, 1),
                        cell("Note", 1, 1),
                    ],
                },
                TableRow {
                    header: false,
                    cells: vec![cell("larga", 2, 1), cell("alta", 1, 2)],
                },
                TableRow {
                    header: false,
                    cells: vec![cell("a", 1, 1), cell("b", 1, 1)],
                },
            ],
            look: TableLook {
                column_widths_twips: vec![2000, 3000, 2000],
                ..TableLook::default()
            },
        },
        Block::Blockquote {
            blocks: vec![paragraph(vec![text("citato", Vec::new())], None)],
        },
        Block::CodeBlock {
            text: "fn main() {\n    run();\n}".to_owned(),
        },
    ];
    DocumentTree {
        blocks,
        media: vec![Media {
            content_type: "image/png",
            bytes: PNG.to_vec(),
        }],
        notes: vec![Note {
            kind: NoteKind::Footnote,
            blocks: vec![paragraph(vec![text("La nota.", Vec::new())], None)],
        }],
        comments: vec![
            Comment {
                author: "Anna".to_owned(),
                initials: None,
                date: Some("2026-10-29T10:00:00Z".to_owned()),
                blocks: vec![paragraph(vec![text("Da rivedere", Vec::new())], None)],
                parent: None,
                resolved: false,
            },
            Comment {
                author: "Bruno".to_owned(),
                initials: None,
                date: None,
                blocks: vec![paragraph(vec![text("Fatto", Vec::new())], None)],
                parent: Some(0),
                resolved: true,
            },
            Comment {
                author: "Carla".to_owned(),
                initials: None,
                date: None,
                blocks: vec![paragraph(vec![text("Qui", Vec::new())], None)],
                parent: None,
                resolved: false,
            },
        ],
        headers_footers: HeadersFooters {
            header: vec![paragraph(vec![text("Riservato", Vec::new())], None)],
            footer: vec![paragraph(
                vec![
                    text("Pagina ", Vec::new()),
                    Inline::Text {
                        text: "1".to_owned(),
                        marks: Vec::new(),
                        look: RunLook {
                            field: Some(PageField::Page),
                            ..RunLook::default()
                        },
                    },
                    text(" di ", Vec::new()),
                    Inline::Text {
                        text: "1".to_owned(),
                        marks: Vec::new(),
                        look: RunLook {
                            field: Some(PageField::Pages),
                            ..RunLook::default()
                        },
                    },
                ],
                None,
            )],
            ..HeadersFooters::default()
        },
        layout: PageLayout {
            margins_twips: [1440, 1134, 1440, 1134],
            header_distance_twips: 708,
            footer_distance_twips: 708,
            ..PageLayout::default()
        },
        ..DocumentTree::default()
    }
}

#[test]
#[expect(clippy::too_many_lines, reason = "one check per kind of content")]
fn writes_an_odt_that_reads_back_to_the_same_document() {
    let original = rich_tree();
    let written = write_odt(&original).unwrap();
    assert_eq!(detect_format(&written, None), DocumentFormat::Odt);
    let read = read_document_tree(&written, &Limits::default()).unwrap();

    assert_eq!(texts(&read.blocks), texts(&original.blocks));
    assert!(matches!(
        &read.blocks[0],
        Block::Heading {
            level: 1,
            align: Some(Alignment::Center),
            ..
        }
    ));
    let Block::Paragraph {
        content,
        align,
        look,
    } = &read.blocks[1]
    else {
        panic!("{:?}", read.blocks[1]);
    };
    assert_eq!(*align, Some(Alignment::Justify));
    assert_eq!(
        (
            look.space_before_twips,
            look.space_after_twips,
            look.indent_left_twips,
            look.indent_first_line_twips,
            look.line_spacing
        ),
        (
            Some(240),
            Some(120),
            Some(720),
            Some(-360),
            Some(LineSpacing::Auto(360))
        )
    );
    let marks_of = |words: &str| {
        content
            .iter()
            .find_map(|inline| match inline {
                Inline::Text { text, marks, .. } if text == words => Some(marks.clone()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{words} in {content:?}"))
    };
    assert!(marks_of("committente").contains(&Mark::Bold));
    assert!(marks_of("committente").contains(&Mark::Comment { comment: 0 }));
    assert_eq!(marks_of("corsivo"), [Mark::Italic]);
    assert_eq!(marks_of("sotto"), [Mark::Underline]);
    assert_eq!(marks_of("barrato"), [Mark::Strike]);
    assert_eq!(marks_of("2"), [Mark::Superscript]);
    assert_eq!(marks_of("x"), [Mark::Subscript]);
    assert_eq!(
        marks_of("rosso"),
        [Mark::TextStyle {
            font_family: Some("Georgia".to_owned()),
            font_size: Some("14pt".to_owned()),
            color: Some("#C00000".to_owned()),
        }]
    );
    assert_eq!(
        marks_of("evidenziato"),
        [Mark::Highlight {
            color: "#FFFF00".to_owned()
        }]
    );
    assert_eq!(
        marks_of("sito"),
        [Mark::Link {
            href: "https://relybytes.com".to_owned()
        }]
    );
    assert_eq!(marks_of("codice()"), [Mark::Code]);
    assert!(content.contains(&Inline::HardBreak));
    assert!(content.contains(&text("due  spazi\tcon tab", Vec::new())));
    assert!(content.contains(&Inline::NoteReference { note: 0 }));
    assert!(content.iter().any(|inline| matches!(
        inline,
        Inline::Image { width: Some(40), height: Some(20), alt: Some(alt), .. } if alt == "Logo"
    )));
    assert_eq!(read.media[0].bytes, PNG);
    assert!(
        content
            .iter()
            .any(|inline| matches!(inline, Inline::CommentAnchor { .. }))
    );

    let items: Vec<(bool, u8)> = read
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::ListItem { ordered, level, .. } => Some((*ordered, *level)),
            _ => None,
        })
        .collect();
    assert_eq!(items, [(false, 0), (true, 1), (false, 0)]);

    let Some(Block::Table { rows, look }) = read
        .blocks
        .iter()
        .find(|block| matches!(block, Block::Table { .. }))
    else {
        panic!("no table");
    };
    assert!(rows[0].header && !rows[1].header);
    assert_eq!(
        (rows[1].cells[0].col_span, rows[1].cells[1].row_span),
        (2, 2)
    );
    assert_eq!(rows[2].cells.len(), 2);
    assert_eq!(look.column_widths_twips, [2000, 3000, 2000]);

    assert_eq!(read.notes[0].blocks.len(), 1);
    assert_eq!(read.comments.len(), 3);
    assert_eq!(read.comments[0].author, "Anna");
    assert_eq!(
        read.comments[0].date.as_deref(),
        Some("2026-10-29T10:00:00Z")
    );
    assert_eq!(read.comments[1].parent, Some(0));
    assert!(read.comments[1].resolved);
    assert_eq!(texts(&read.headers_footers.header), ["Riservato"]);
    let Block::Paragraph {
        content: footer, ..
    } = &read.headers_footers.footer[0]
    else {
        panic!("{:?}", read.headers_footers.footer);
    };
    assert!(footer.iter().any(|inline| matches!(
        inline,
        Inline::Text { look, .. } if look.field == Some(PageField::Pages)
    )));
    assert_eq!(read.layout.margins_twips, original.layout.margins_twips);

    let again = read_document_tree(&write_odt(&read).unwrap(), &Limits::default()).unwrap();
    assert_eq!(again.blocks, read.blocks);
    assert_eq!(again.notes, read.notes);
    assert_eq!(again.comments, read.comments);
    assert_eq!(again.headers_footers, read.headers_footers);
}

#[test]
fn writes_tracked_changes_an_editor_can_accept_or_reject() {
    let tree = DocumentTree {
        blocks: vec![paragraph(
            vec![
                text("Prima ", Vec::new()),
                text(
                    "aggiunto",
                    vec![Mark::Insertion {
                        author: "Anna".to_owned(),
                        date: Some("2026-10-29T10:00:00Z".to_owned()),
                    }],
                ),
                text(
                    "tolto",
                    vec![Mark::Deletion {
                        author: "Bruno".to_owned(),
                        date: None,
                    }],
                ),
            ],
            None,
        )],
        ..DocumentTree::default()
    };
    let written = write_odt(&tree).unwrap();
    let keep = TreeOptions {
        tracked_changes: TrackedChanges::Keep,
    };
    let kept = read_document_tree_with(&written, &Limits::default(), &keep).unwrap();
    let Block::Paragraph { content, .. } = &kept.blocks[0] else {
        panic!("{:?}", kept.blocks);
    };
    assert!(content.iter().any(|inline| matches!(
        inline,
        Inline::Text { text, marks, .. } if text == "aggiunto" && matches!(marks.as_slice(), [Mark::Insertion { author, .. }] if author == "Anna")
    )));
    assert!(content.iter().any(|inline| matches!(
        inline,
        Inline::Text { text, marks, .. } if text == "tolto" && matches!(marks.as_slice(), [Mark::Deletion { author, .. }] if author == "Bruno")
    )));
    let accepted = read_document_tree(&written, &Limits::default()).unwrap();
    assert_eq!(texts(&accepted.blocks), ["Prima aggiunto"]);
}

#[test]
fn writes_each_section_on_its_own_page() {
    let landscape = PageLayout {
        width_twips: 16_838,
        height_twips: 11_906,
        ..PageLayout::default()
    };
    let tree = DocumentTree {
        blocks: vec![
            paragraph(vec![text("verticale", Vec::new())], None),
            paragraph(vec![text("orizzontale", Vec::new())], None),
        ],
        sections: vec![
            Section {
                end_block: 1,
                layout: PageLayout::default(),
                headers_footers: HeadersFooters::default(),
                continuous: false,
            },
            Section {
                end_block: 2,
                layout: landscape.clone(),
                headers_footers: HeadersFooters::default(),
                continuous: false,
            },
        ],
        layout: landscape,
        ..DocumentTree::default()
    };
    let read = read_document_tree(&write_odt(&tree).unwrap(), &Limits::default()).unwrap();
    assert_eq!(read.sections.len(), 2);
    assert_eq!(read.sections[0].end_block, 1);
    assert!(read.sections[1].layout.width_twips > read.sections[1].layout.height_twips);
    assert!(read.sections[0].layout.width_twips < read.sections[0].layout.height_twips);
}

#[test]
fn writes_spaces_and_lengths_as_opendocument_spells_them() {
    assert_eq!(
        text_xml("  a  b "),
        r#"<text:s text:c="2"/>a <text:s text:c="1"/>b<text:s text:c="1"/>"#
    );
    assert_eq!(text_xml("a\tb<&>"), "a<text:tab/>b&lt;&amp;&gt;");
    assert_eq!(length(240), "12pt");
    assert_eq!(signed_length(-360), "-18pt");
    assert_eq!(trim_number(1.5), "1.5");
    assert_eq!(css_pixels(96), "2.54cm");
    let media = Media {
        content_type: "image/png",
        bytes: PNG.to_vec(),
    };
    assert_eq!(image_size(&media, Some(10), None), (10, 5));
    assert_eq!(image_size(&media, None, None), (2, 1));
}
