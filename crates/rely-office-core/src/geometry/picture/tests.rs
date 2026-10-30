// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use super::*;

fn shape(fill: Option<&str>, line: Option<(&str, i64)>, text: Vec<TextLine>) -> DrawnShape {
    DrawnShape {
        x: 12_700,
        y: 0,
        width: 127_000,
        height: 63_500,
        rotation: 0.0,
        paths: rectangle_paths(127_000, 63_500),
        fill: fill.map(str::to_owned),
        line: line.map(|(colour, width)| (colour.to_owned(), width)),
        text,
    }
}

#[test]
fn draws_shapes_their_shaded_faces_lines_and_text() {
    let mut shaded = shape(Some("4472C4"), Some(("000000", 0)), Vec::new());
    shaded.paths[0].shade = FillShade::Darken;
    let svg = String::from_utf8(shapes_svg(&[shaded], 254_000, 127_000).unwrap()).unwrap();
    assert!(svg.starts_with(r#"<svg xmlns="http://www.w3.org/2000/svg" width="20pt" height="10pt" viewBox="0 0 20 10" overflow="visible">"#), "{svg}");
    assert!(svg.contains(r##"<g transform="translate(1 0)"><path d="M0 0L10 0L10 5L0 5Z" fill="#294476" fill-rule="evenodd"/>"##));
    assert!(
        svg.contains(r#"stroke-width="0.25""#),
        "a hairline drawn at a quarter point"
    );
    let lines = vec![
        TextLine {
            text: "a<b>\tc\u{1}".to_owned(),
            size_points: 10.0,
            colour: "FF0000".to_owned(),
            bold: false,
            italic: true,
        },
        TextLine {
            text: "  ".to_owned(),
            size_points: 10.0,
            colour: "000000".to_owned(),
            bold: true,
            italic: false,
        },
    ];
    let mut turned = shape(None, None, lines);
    turned.rotation = 45.0;
    let svg = String::from_utf8(shapes_svg(&[turned], 254_000, 127_000).unwrap()).unwrap();
    assert!(svg.contains("translate(1 0) rotate(45 5 2.5)"));
    assert!(svg.contains(r##"<text x="5" y="0" font-family="Calibri, Carlito, sans-serif" font-size="10" fill="#FF0000" text-anchor="middle" font-style="italic" xml:space="preserve">a&lt;b&gt; c</text>"##), "{svg}");
    assert_eq!(
        svg.matches("<text").count(),
        1,
        "a blank line takes its place and draws nothing"
    );
    assert_eq!(shapes_svg(&[shape(None, None, Vec::new())], 10, 10), None);
    assert_eq!(
        shapes_svg(&[shape(Some("FFFFFF"), None, Vec::new())], 0, 10),
        None
    );
    assert_eq!(number(f64::INFINITY), "0");
    assert_eq!(number(-0.001), "0");
    let mut curved = rectangle_paths(10, 10);
    curved[0].commands.push(PathCommand::CubicTo(
        super::super::PathPoint { x: 12_700, y: 0 },
        super::super::PathPoint { x: 0, y: 12_700 },
        super::super::PathPoint {
            x: 25_400,
            y: 25_400,
        },
    ));
    assert!(path_data(&curved[0]).ends_with("ZC1 0 0 1 2 2"));
}
