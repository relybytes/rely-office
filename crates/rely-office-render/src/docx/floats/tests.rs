// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use std::collections::HashMap;

use rely_office_core::{AnchorPosition, Media, ParagraphLook, RunLook, TableLook};

use super::*;
use crate::metrics::FontLines;

static LINES: std::sync::LazyLock<FontLines> = std::sync::LazyLock::new(FontLines::default);

fn writer(media: &[Media]) -> Writer<'_> {
    Writer {
        media,
        with_images: true,
        lines: &LINES,
        body_family: None,
        body_size: 11.0,
        text_width: 400.0,
        default_tab: 35.4,
        spacing_adds_up: false,
        notes: &[],
        files: HashMap::new(),
        list_counters: HashMap::new(),
        endnotes: Vec::new(),
        in_note: false,
        page: PagePlace {
            width: 600.0,
            height: 800.0,
            left: 100.0,
            top: 50.0,
            right: 100.0,
            bottom: 50.0,
        },
        open_frame: None,
    }
}

fn png() -> Vec<Media> {
    vec![Media {
        content_type: "image/png",
        bytes: b"\x89PNG\r\n\x1a\n".to_vec(),
    }]
}

fn position(base: AnchorBase, offset_points: i64, align: Option<AnchorAlign>) -> AnchorPosition {
    AnchorPosition {
        base,
        offset_emu: offset_points * 12_700,
        align,
    }
}

fn anchor(wrap: Wrap, horizontal: AnchorPosition, vertical: AnchorPosition) -> Anchor {
    Anchor {
        horizontal,
        vertical,
        width_emu: 100 * 12_700,
        height_emu: 50 * 12_700,
        wrap,
        distance_emu: [0; 4],
    }
}

fn paragraph() -> Placed {
    Placed {
        expression: "P".to_owned(),
        before: 1.0,
        after: 2.0,
        trailing: 0.0,
        page_break: true,
    }
}

fn float(anchor: Anchor) -> Float {
    Float {
        expression: "O".to_owned(),
        anchor,
    }
}

fn text(value: &str) -> Inline {
    Inline::Text {
        text: value.to_owned(),
        marks: Vec::new(),
        look: RunLook::default(),
    }
}

#[test]
fn leaves_a_paragraph_without_floats_as_it_is() {
    let placed = writer(&[]).with_floats(paragraph(), Vec::new());
    assert_eq!(placed.expression, "P");
    assert!(placed.page_break);
}

#[test]
fn places_an_object_in_front_of_the_text_on_the_page() {
    let page = anchor(
        Wrap::InFront,
        position(AnchorBase::Page, 400, None),
        position(AnchorBase::Page, 600, None),
    );
    let placed = writer(&[]).with_floats(paragraph(), vec![float(page)]);
    assert_eq!(
        placed.expression,
        "block(width: 100%, context {\nlet origin = here().position()\nP\nplace(top + left, dx: 400pt - origin.x, dy: 600pt - origin.y, O)\n})"
    );
    assert_eq!(
        (placed.before, placed.after, placed.page_break),
        (1.0, 2.0, true)
    );
}

#[test]
fn places_an_object_behind_the_text_before_the_paragraph() {
    let behind = anchor(
        Wrap::Behind,
        position(AnchorBase::Margin, 0, Some(AnchorAlign::Center)),
        position(AnchorBase::Margin, 0, Some(AnchorAlign::End)),
    );
    let placed = writer(&[]).with_floats(paragraph(), vec![float(behind)]);
    assert_eq!(
        placed.expression,
        "block(width: 100%, context {\nlet origin = here().position()\nplace(top + left, dx: 250pt - origin.x, dy: 700pt - origin.y, O)\nP\n})"
    );
}

#[test]
fn measures_an_overlay_from_each_base() {
    let writer = writer(&[]);
    let overlay =
        |horizontal, vertical| writer.overlay("O", &anchor(Wrap::InFront, horizontal, vertical));
    assert_eq!(
        overlay(
            position(AnchorBase::Page, 0, Some(AnchorAlign::End)),
            position(AnchorBase::Page, 0, Some(AnchorAlign::Center)),
        ),
        "place(top + left, dx: 500pt - origin.x, dy: 375pt - origin.y, O)"
    );
    assert_eq!(
        overlay(
            position(AnchorBase::Column, 20, None),
            position(AnchorBase::Paragraph, 30, None),
        ),
        "place(top + left, dx: 120pt - origin.x, dy: 30pt, O)"
    );
    assert_eq!(
        overlay(
            position(AnchorBase::Line, 0, Some(AnchorAlign::Start)),
            position(AnchorBase::Margin, 0, Some(AnchorAlign::Start)),
        ),
        "place(top + left, dx: 100pt - origin.x, dy: 50pt - origin.y, O)"
    );
    assert_eq!(
        overlay(
            position(AnchorBase::Paragraph, 0, None),
            position(AnchorBase::Line, -5, None),
        ),
        "place(top + left, dx: 100pt - origin.x, dy: -5pt, O)"
    );
}

#[test]
fn sets_an_object_the_text_goes_around_beside_its_paragraph() {
    let writer = writer(&[]);
    let around = |horizontal, distance: [i64; 4]| {
        let mut anchor = anchor(
            Wrap::Around,
            horizontal,
            position(AnchorBase::Paragraph, 0, None),
        );
        anchor.distance_emu = distance;
        writer
            .with_floats(paragraph(), vec![float(anchor)])
            .expression
    };
    assert_eq!(
        around(
            position(AnchorBase::Column, 0, Some(AnchorAlign::End)),
            [0; 4]
        ),
        "grid(columns: (1fr, 100pt), column-gutter: 9pt, P, O)"
    );
    assert_eq!(
        around(
            position(AnchorBase::Column, 0, Some(AnchorAlign::Start)),
            [0, 0, 0, 12_700 * 4]
        ),
        "grid(columns: (100pt, 1fr), column-gutter: 4pt, O, P)"
    );
    assert_eq!(
        around(position(AnchorBase::Page, 450, None), [0, 0, 12_700 * 3, 0]),
        "grid(columns: (1fr, 100pt), column-gutter: 3pt, P, O)"
    );
    assert_eq!(
        around(
            position(AnchorBase::Column, 10, Some(AnchorAlign::Center)),
            [0; 4]
        ),
        "grid(columns: (100pt, 1fr), column-gutter: 9pt, O, P)"
    );
}

#[test]
fn sets_a_wide_object_or_one_with_text_above_and_below_on_a_line_of_its_own() {
    let writer = writer(&[]);
    let mut wide = anchor(
        Wrap::Around,
        position(AnchorBase::Column, 0, Some(AnchorAlign::Center)),
        position(AnchorBase::Paragraph, 0, None),
    );
    wide.width_emu = 300 * 12_700;
    wide.distance_emu = [0, 6 * 12_700, 0, 0];
    assert_eq!(
        writer
            .with_floats(paragraph(), vec![float(wide)])
            .expression,
        "stack(dir: ttb, spacing: 6pt, align(center, O), P)"
    );
    let below = |horizontal| {
        let anchor = anchor(
            Wrap::TopAndBottom,
            horizontal,
            position(AnchorBase::Paragraph, 0, None),
        );
        writer
            .with_floats(paragraph(), vec![float(anchor)])
            .expression
    };
    assert_eq!(
        below(position(AnchorBase::Column, 0, Some(AnchorAlign::End))),
        "stack(dir: ttb, spacing: 0pt, align(right, O), P)"
    );
    assert_eq!(
        below(position(AnchorBase::Column, 0, Some(AnchorAlign::Start))),
        "stack(dir: ttb, spacing: 0pt, O, P)"
    );
    assert_eq!(
        below(position(AnchorBase::Page, 130, None)),
        "stack(dir: ttb, spacing: 0pt, pad(left: 30pt, O), P)"
    );
    assert_eq!(
        below(position(AnchorBase::Column, 0, None)),
        "stack(dir: ttb, spacing: 0pt, O, P)"
    );
    assert_eq!(
        below(position(AnchorBase::Column, 900, None)),
        "stack(dir: ttb, spacing: 0pt, pad(left: 400pt, O), P)"
    );
}

#[test]
fn takes_the_floating_images_out_of_a_paragraph() {
    let media = png();
    let mut writer = writer(&media);
    let inline = vec![text("a")];
    let (kept, floats) = writer.split_floats(&inline);
    assert!(matches!(kept, Cow::Borrowed(_)));
    assert!(floats.is_empty());

    let page = anchor(
        Wrap::InFront,
        position(AnchorBase::Page, 0, None),
        position(AnchorBase::Page, 0, None),
    );
    let content = vec![
        text("a"),
        Inline::Image {
            media: 0,
            alt: None,
            width: Some(80),
            height: Some(40),
            anchor: Some(Box::new(page)),
        },
        Inline::Image {
            media: 0,
            alt: None,
            width: Some(8),
            height: None,
            anchor: None,
        },
    ];
    let (kept, floats) = writer.split_floats(&content);
    assert_eq!(kept.len(), 2);
    assert!(matches!(kept[1], Inline::Image { anchor: None, .. }));
    assert_eq!(floats.len(), 1);
    assert_eq!(
        floats[0].expression,
        "box(image(\"/media/0.png\", width: 60pt, height: 30pt, fit: \"stretch\"))"
    );
    assert_eq!(floats[0].anchor, page);
}

#[test]
fn draws_a_text_box_with_its_fill_outline_and_size() {
    let mut writer = writer(&[]);
    let frame = TextFrame {
        shape: None,
        id: 3,
        anchor: anchor(
            Wrap::InFront,
            position(AnchorBase::Page, 0, None),
            position(AnchorBase::Page, 0, None),
        ),
        fill: Some("FFF2CC".to_owned()),
        line: Some(("BF9000".to_owned(), 8)),
    };
    let blocks = vec![Block::Paragraph {
        content: vec![text("dentro")],
        align: None,
        look: ParagraphLook {
            frame: Some(frame.clone()),
            ..ParagraphLook::default()
        },
    }];
    let drawn = writer.frame_box(&blocks, &frame);
    assert!(drawn.starts_with(
        "block(width: 100pt, height: 50pt, fill: rgb(\"#FFF2CC\"), stroke: 1pt + rgb(\"#BF9000\"), inset: (x: 7.2pt, y: 3.6pt), breakable: false)[#text("
    ));
    assert!(drawn.contains("dentro"));
    assert!(!drawn.contains("place("));
    assert_eq!(writer.text_width, 400.0);
    assert_eq!(writer.open_frame, None);

    let shaped = TextFrame {
        shape: Some("ellipse".to_owned()),
        ..frame.clone()
    };
    let drawn = writer.frame_box(&blocks, &shaped);
    assert!(
        drawn.starts_with("block(width: 100pt, height: 50pt, fill: none, stroke: none,"),
        "{drawn}"
    );
    assert!(
        drawn.contains("#place(top + left, dx: -7.2pt, dy: -3.6pt, box(width: 100pt, height: 50pt, curve(fill: rgb(\"#FFF2CC\")"),
        "the outline behind the text: {drawn}"
    );
    assert!(drawn.contains("curve.cubic(") && drawn.contains("#align(horizon + center, block["));
    let unknown = TextFrame {
        shape: Some("noSuchShape".to_owned()),
        ..frame.clone()
    };
    assert!(
        writer
            .frame_box(&blocks, &unknown)
            .starts_with("block(width: 100pt, height: 50pt, fill: rgb(")
    );

    let bare = TextFrame {
        fill: None,
        line: None,
        anchor: Anchor {
            width_emu: 0,
            height_emu: 0,
            ..frame.anchor
        },
        ..frame
    };
    let drawn = writer.frame_box(&[], &bare);
    assert_eq!(
        drawn,
        "block(width: 15.4pt, fill: none, stroke: none, inset: (x: 7.2pt, y: 3.6pt), breakable: false)[]"
    );
}

#[test]
fn writes_a_text_box_from_the_paragraph_before_it() {
    let mut writer = writer(&[]);
    let frame = TextFrame {
        shape: None,
        id: 1,
        anchor: anchor(
            Wrap::InFront,
            position(AnchorBase::Page, 0, None),
            position(AnchorBase::Page, 0, None),
        ),
        fill: None,
        line: None,
    };
    let framed = |value: &str| Block::Paragraph {
        content: vec![text(value)],
        align: None,
        look: ParagraphLook {
            frame: Some(frame.clone()),
            ..ParagraphLook::default()
        },
    };
    let plain = Block::Paragraph {
        content: vec![text("fuori")],
        align: None,
        look: ParagraphLook::default(),
    };
    let markup = writer.blocks(&[plain, framed("uno"), framed("due")]);
    assert_eq!(markup.matches("place(").count(), 1);
    assert_eq!(markup.matches("#block(width: 100%, context").count(), 1);
    let fuori = markup.find("fuori").expect("the anchoring paragraph");
    let uno = markup.find("uno").expect("the first line of the box");
    let due = markup.find("due").expect("the second line of the box");
    assert!(fuori < uno && uno < due);

    let alone = writer.blocks(&[framed("solo")]);
    assert!(
        alone.starts_with(
            "#block(width: 100%, context {\nlet origin = here().position()\n[]\nplace("
        )
    );
}

#[test]
fn finds_the_text_box_a_block_is_in() {
    let frame = TextFrame {
        shape: None,
        id: 9,
        anchor: anchor(
            Wrap::InFront,
            position(AnchorBase::Page, 0, None),
            position(AnchorBase::Page, 0, None),
        ),
        fill: None,
        line: None,
    };
    let look = ParagraphLook {
        frame: Some(frame.clone()),
        ..ParagraphLook::default()
    };
    let paragraph = Block::Paragraph {
        content: Vec::new(),
        align: None,
        look: look.clone(),
    };
    assert_eq!(frame_of(&paragraph), Some(&frame));
    assert_eq!(
        frame_of(&Block::Table {
            rows: Vec::new(),
            look: TableLook {
                frame: Some(frame.clone()),
                ..TableLook::default()
            },
        }),
        Some(&frame)
    );
    assert_eq!(
        frame_of(&Block::Blockquote {
            blocks: vec![paragraph]
        }),
        Some(&frame)
    );
    assert_eq!(frame_of(&Block::Blockquote { blocks: Vec::new() }), None);
    assert_eq!(frame_of(&Block::HorizontalRule), None);
    assert_eq!(
        frame_of(&Block::CodeBlock {
            text: String::new()
        }),
        None
    );
    assert_eq!(
        frame_of(&Block::Heading {
            level: 1,
            content: Vec::new(),
            align: None,
            look,
        }),
        Some(&frame)
    );
}

#[test]
fn places_a_page_on_a4_by_default() {
    let page = PagePlace::default();
    assert_eq!((page.width, page.height), (595.3, 841.9));
    assert_eq!(page.left, 70.85);
}

#[test]
fn flows_a_paragraph_around_its_object_line_by_line() {
    let mut writer = writer(&[]);
    let look = ParagraphLook::default();
    let content = vec![
        text("Una riga che gira"),
        Inline::Text {
            text: "attorno".to_owned(),
            marks: vec![rely_office_core::Mark::Bold],
            look: RunLook::default(),
        },
        text(" al disegno"),
    ];
    let left = anchor(
        Wrap::Around,
        position(AnchorBase::Column, 0, Some(AnchorAlign::Start)),
        position(AnchorBase::Paragraph, 0, None),
    );
    let mut floats = vec![float(left)];
    let flowed = writer
        .flowed(&content, Some(Alignment::Justify), &look, &mut floats)
        .expect("a paragraph beside its object");
    assert!(floats.is_empty(), "the object is set by the flow");
    assert!(
        flowed
            .expression
            .starts_with("rely-flow(((O, false, 0pt, 0pt), ), 50pt, (109pt, "),
        "{}",
        flowed.expression
    );
    assert!(
        flowed.expression.contains("([#\"Una\"], false), "),
        "{}",
        flowed.expression
    );
    assert!(
        flowed
            .expression
            .contains(r##"([#"gira"#text(weight: "bold", "attorno")], false)"##),
        "runs with no space between stay one word: {}",
        flowed.expression
    );
    assert!(
        flowed.expression.contains(", false, ((["),
        "no tab: {}",
        flowed.expression
    );
    assert!(flowed.expression.contains("justify: true, it)))"));
    let mut outline = rely_office_core::WrapOutline {
        left: [u8::MAX; rely_office_core::OUTLINE_BANDS],
        right: [0; rely_office_core::OUTLINE_BANDS],
    };
    outline.left[0] = 0;
    outline.right[0] = 255;
    let mut contour = anchor(
        Wrap::Contour(outline),
        position(AnchorBase::Column, 0, Some(AnchorAlign::End)),
        position(AnchorBase::Paragraph, 0, None),
    );
    contour.distance_emu = [0, 0, 4 * 12_700, 0];
    let flowed = writer
        .flowed(&content, None, &look, &mut vec![float(contour)])
        .unwrap();
    assert!(
        flowed.expression.starts_with(
            "rely-flow(((O, true, 0pt, 0pt), ), 50pt, (0pt, 0pt, 0pt, 0pt, ), (104pt, 0pt, "
        ),
        "the object reaches only its first band, on the right: {}",
        flowed.expression
    );
    let mut wide = left;
    wide.width_emu = 380 * 12_700;
    assert!(
        writer
            .flowed(&content, None, &look, &mut vec![float(wide)])
            .is_none(),
        "no text fits beside an object nearly as wide as the column"
    );
    let mut behind = left;
    behind.wrap = Wrap::Behind;
    assert!(
        writer
            .flowed(&content, None, &look, &mut vec![float(behind)])
            .is_none()
    );
    let listed = ParagraphLook {
        list_label: Some(rely_office_core::ListLabel {
            text: "1.".to_owned(),
            suffix: rely_office_core::LabelSuffix::Tab,
            font: RunLook::default(),
        }),
        ..ParagraphLook::default()
    };
    assert!(
        writer
            .flowed(&content, None, &listed, &mut vec![float(left)])
            .is_none()
    );
    assert!(FLOW_FUNCTION.starts_with("#let rely-flow("));
}

#[test]
fn flows_a_paragraph_between_several_objects_and_leaves_the_others() {
    let mut writer = writer(&[]);
    let look = ParagraphLook::default();
    let left = anchor(
        Wrap::Around,
        position(AnchorBase::Column, 0, Some(AnchorAlign::Start)),
        position(AnchorBase::Paragraph, 0, None),
    );
    let mut right = left;
    right.horizontal = position(AnchorBase::Column, 0, Some(AnchorAlign::End));
    right.height_emu = 20 * 12_700;
    let mut behind = left;
    behind.wrap = Wrap::Behind;
    let mut floats = vec![float(left), float(right), float(left), float(behind)];
    let flowed = writer
        .flowed(&[text("parole accanto")], None, &look, &mut floats)
        .unwrap();
    assert_eq!(
        floats.len(),
        1,
        "the object behind the text stays for the overlay"
    );
    assert!(
        flowed.expression.starts_with(
            "rely-flow(((O, false, 0pt, 0pt), (O, true, 0pt, 0pt), (O, false, 109pt, 0pt), ), 50pt, (218pt, "
        ),
        "a second object on the same side stands beside the first: {}",
        flowed.expression
    );
    assert!(
        flowed.expression.contains("), (109pt, 109pt, 0pt, "),
        "the right object reaches only its own lines: {}",
        flowed.expression
    );
}

#[test]
fn flows_a_paragraph_with_tabs_against_its_stops() {
    let mut writer = writer(&[]);
    let look = ParagraphLook {
        indent_left_twips: Some(720),
        tabs: vec![rely_office_core::TabStop {
            position_twips: 2_880,
            kind: rely_office_core::TabKind::Left,
            leader: rely_office_core::TabLeader::None,
        }],
        ..ParagraphLook::default()
    };
    let left = anchor(
        Wrap::Around,
        position(AnchorBase::Column, 0, Some(AnchorAlign::Start)),
        position(AnchorBase::Paragraph, 0, None),
    );
    let flowed = writer
        .flowed(
            &[text("Voce\tvalore 7")],
            None,
            &look,
            &mut vec![float(left)],
        )
        .unwrap();
    assert!(
        flowed
            .expression
            .starts_with("pad(left: 36pt, right: 0pt, rely-flow("),
        "{}",
        flowed.expression
    );
    assert!(
        flowed
            .expression
            .contains(", 36pt, 35.4pt, (144pt, ), true, (([#\"Voce\"], false), ([], true), ([#\"valore\"], false), ([#\"7\"], false), ),"),
        "the origin, the default step, the stops and the tab as a word: {}",
        flowed.expression
    );
    assert!(
        writer
            .flowed(&[text("\t")], None, &look, &mut vec![float(left)])
            .is_none(),
        "a paragraph of only a tab has no word"
    );
}
