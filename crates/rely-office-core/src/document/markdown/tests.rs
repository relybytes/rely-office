// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;
use crate::document::{ParagraphLook, RunLook};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

fn read(text: &str) -> DocumentTree {
    read_markdown_tree(text, &Limits::default()).unwrap()
}

fn text(content: &str, marks: Vec<Mark>) -> Inline {
    Inline::Text {
        text: content.to_owned(),
        marks,
        look: RunLook::default(),
    }
}

fn paragraph_of(content: Vec<Inline>) -> Block {
    Block::Paragraph {
        content,
        align: None,
        look: ParagraphLook::default(),
    }
}

#[test]
fn reads_headings_paragraphs_and_marks() {
    let tree = read(
        "# Titolo\n\nUn **grassetto**, un *corsivo*, ~~barrato~~, `codice` e \
         [un link](https://relybytes.com).\nStessa riga.  \nA capo.",
    );
    assert_eq!(
        tree.blocks[0],
        Block::Heading {
            level: 1,
            content: vec![text("Titolo", Vec::new())],
            align: None,
            look: ParagraphLook::default(),
        }
    );
    let Block::Paragraph { content, .. } = &tree.blocks[1] else {
        panic!("{:?}", tree.blocks[1]);
    };
    assert!(content.contains(&text("grassetto", vec![Mark::Bold])));
    assert!(content.contains(&text("corsivo", vec![Mark::Italic])));
    assert!(content.contains(&text("barrato", vec![Mark::Strike])));
    assert!(content.contains(&text("codice", vec![Mark::Code])));
    assert!(content.contains(&text(
        "un link",
        vec![Mark::Link {
            href: "https://relybytes.com".to_owned()
        }]
    )));
    assert!(content.contains(&Inline::HardBreak));
    assert!(
        matches!(&content[content.len() - 3], Inline::Text { text, .. } if text.ends_with("Stessa riga."))
    );
}

#[test]
fn reads_nested_lists_tasks_quotes_code_and_rules() {
    let tree = read(
        "- uno\n  - dentro\n- [x] fatto\n\n1. primo\n2. secondo\n\n> citato\n\n```rust\nfn a() {}\n```\n\n---\n",
    );
    let items: Vec<(bool, u8, u32, String)> = tree
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::ListItem {
                ordered,
                level,
                list_id,
                content,
                ..
            } => Some((
                *ordered,
                *level,
                *list_id,
                content
                    .iter()
                    .filter_map(|inline| match inline {
                        Inline::Text { text, .. } => Some(text.as_str()),
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
            (false, 0, 1, "uno".to_owned()),
            (false, 1, 1, "dentro".to_owned()),
            (false, 0, 1, "☑ fatto".to_owned()),
            (true, 0, 2, "primo".to_owned()),
            (true, 0, 2, "secondo".to_owned()),
        ]
    );
    assert!(tree.blocks.contains(&Block::Blockquote {
        blocks: vec![paragraph_of(vec![text("citato", Vec::new())])]
    }));
    assert!(tree.blocks.contains(&Block::CodeBlock {
        text: "fn a() {}".to_owned()
    }));
    assert_eq!(tree.blocks.last(), Some(&Block::HorizontalRule));
}

#[test]
fn reads_tables_footnotes_math_html_marks_and_images() {
    let image = format!(
        "![logo](data:image/png;base64,{})",
        crate::base64::encode(PNG)
    );
    let tree = read(&format!(
        "| A | B |\n| :-- | --: |\n| uno | due |\n\nNota[^n] e $x^2$ <u>sotto</u><br>{image} ![fuori](https://relybytes.com/a.png)\n\n[^n]: Il testo della nota.\n"
    ));
    let Block::Table { rows, .. } = &tree.blocks[0] else {
        panic!("{:?}", tree.blocks[0]);
    };
    assert!(rows[0].header && !rows[1].header);
    assert_eq!(rows[1].cells.len(), 2);
    assert!(matches!(
        &rows[1].cells[1].blocks[0],
        Block::Paragraph {
            align: Some(Alignment::Right),
            ..
        }
    ));
    let Block::Paragraph { content, .. } = &tree.blocks[1] else {
        panic!("{:?}", tree.blocks[1]);
    };
    assert!(content.contains(&Inline::NoteReference { note: 0 }));
    assert!(content.contains(&Inline::Math {
        latex: "x^2".to_owned(),
        display: false
    }));
    assert!(content.contains(&text("sotto", vec![Mark::Underline])));
    assert!(content.contains(&Inline::HardBreak));
    assert!(
        content
            .iter()
            .any(|inline| matches!(inline, Inline::Image { media: 0, .. }))
    );
    assert!(content.contains(&text(
        "fuori",
        vec![Mark::Link {
            href: "https://relybytes.com/a.png".to_owned()
        }]
    )));
    assert_eq!(tree.media[0].content_type, "image/png");
    assert_eq!(
        tree.notes[0].blocks,
        [paragraph_of(vec![text("Il testo della nota.", Vec::new())])]
    );
}

#[test]
fn refuses_text_over_the_limit_and_drops_unsafe_links() {
    let limits = Limits {
        max_input_bytes: 4,
        ..Limits::default()
    };
    assert!(matches!(
        read_markdown_tree("troppo lungo", &limits),
        Err(OfficeError::TooLarge { .. })
    ));
    let tree = read("[clic](javascript:alert(1))");
    assert_eq!(tree.blocks, [paragraph_of(vec![text("clic", Vec::new())])]);
}

#[test]
fn writes_what_it_reads_back_to_the_same_tree() {
    let image = format!(
        "![logo](data:image/png;base64,{})",
        crate::base64::encode(PNG)
    );
    let source = format!(
        "# Titolo\n\nUn **grassetto *e corsivo***, ~~barrato~~, `a``b`, <u>sotto</u> e [link](https://relybytes.com).\\\nA capo con 3\\*4 e \\# non titolo.\n\n- uno\n  - dentro\n- due\n\n1. primo\n2. secondo\n\n> citato\n\n```\ncodice\n```\n\n| A | B |\n| --- | ---: |\n| x | 2 |\n\nNota[^1] e $x^2$ {image}\n\n---\n\n[^1]: Il testo della nota.\n"
    );
    let tree = read(&source);
    let written = write_markdown(&tree);
    let again = read(&written);
    assert_eq!(again.blocks, tree.blocks, "{written}");
    assert_eq!(again.notes, tree.notes);
    assert_eq!(again.media, tree.media);
    assert_eq!(write_markdown(&again), written);
}

#[test]
fn keeps_two_lists_apart_and_writes_what_markdown_lacks_as_html() {
    let tree = DocumentTree {
        blocks: vec![
            Block::ListItem {
                ordered: false,
                level: 0,
                list_id: 1,
                content: vec![text("a", Vec::new())],
                look: ParagraphLook::default(),
            },
            Block::ListItem {
                ordered: false,
                level: 0,
                list_id: 2,
                content: vec![text("b", Vec::new())],
                look: ParagraphLook::default(),
            },
            paragraph_of(vec![
                text("H", vec![Mark::Subscript]),
                text("2", Vec::new()),
                text(
                    "evidenza",
                    vec![Mark::Highlight {
                        color: "#FFFF00".to_owned(),
                    }],
                ),
                text(
                    "via",
                    vec![Mark::Deletion {
                        author: "A".to_owned(),
                        date: None,
                    }],
                ),
                text(" - 1. ", Vec::new()),
            ]),
            paragraph_of(vec![text("1. non lista", Vec::new())]),
        ],
        ..DocumentTree::default()
    };
    let written = write_markdown(&tree);
    assert!(written.starts_with("- a\n\n* b\n"), "{written}");
    assert!(
        written.contains("<sub>H</sub>2<mark>evidenza</mark> - 1."),
        "{written}"
    );
    assert!(!written.contains("via"));
    assert!(written.contains("1\\. non lista"), "{written}");
    let again = read(&written);
    let ids: Vec<u32> = again
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::ListItem { list_id, .. } => Some(*list_id),
            _ => None,
        })
        .collect();
    assert_eq!(ids, [1, 2]);
}
