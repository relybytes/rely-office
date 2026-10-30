// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::geometry::PathCommand;
use crate::xml_tree::parse_tree;

const NAMESPACES: &str = concat!(
    r#"xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" "#,
    r#"xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0""#
);

fn element(name: &str, attributes: &str) -> Element {
    parse_tree(
        &format!("<draw:{name} {NAMESPACES} {attributes}/>"),
        |_, _| false,
    )
    .expect("the element parses")
}

fn points(path: &ShapePath) -> Vec<(i64, i64)> {
    path.commands
        .iter()
        .filter_map(|command| match command {
            PathCommand::MoveTo(point)
            | PathCommand::LineTo(point)
            | PathCommand::CubicTo(_, _, point) => Some((point.x, point.y)),
            PathCommand::Close => None,
        })
        .collect()
}

fn path(data: &str) -> ShapePath {
    let element = element(
        "path",
        &format!(r#"svg:viewBox="0 0 100 100" svg:d="{data}""#),
    );
    svg_path_geometry(&element, 100, 100)
        .expect("a path")
        .remove(0)
}

#[test]
fn splits_path_data_into_commands_and_numbers() {
    assert_eq!(
        tokens("M1.5-2e1,.5.5L3 z#"),
        vec![
            Token::Command('M'),
            Token::Number(1.5),
            Token::Number(-20.0),
            Token::Number(0.5),
            Token::Number(0.5),
            Token::Command('L'),
            Token::Number(3.0),
            Token::Command('z'),
        ]
    );
}

#[test]
fn draws_a_polygon_closed_and_a_polyline_open() {
    let polygon = element(
        "polygon",
        r#"svg:viewBox="0 0 10 10" draw:points="0,0 10,0 10,10 5""#,
    );
    let paths = svg_path_geometry(&polygon, 100, 200).expect("a polygon");
    assert_eq!(points(&paths[0]), vec![(0, 0), (100, 0), (100, 200)]);
    assert_eq!(paths[0].commands.last(), Some(&PathCommand::Close));
    assert!(paths[0].filled);
    let polyline = element("polyline", r#"draw:points="0,0 10,5""#);
    let paths = svg_path_geometry(&polyline, 100, 100).expect("a polyline");
    assert_eq!(points(&paths[0]), vec![(0, 0), (10, 5)]);
    assert!(!paths[0].filled);
    assert_eq!(svg_path_geometry(&element("polygon", ""), 10, 10), None);
    assert_eq!(
        svg_path_geometry(&element("polygon", r#"draw:points="""#), 10, 10),
        None
    );
}

#[test]
fn draws_absolute_and_relative_lines() {
    let lines = path("M 10 10 20 10 l 0 10 H 50 h 5 V 0 v 5 L 1 1 Z m 2 2 l 1 1");
    assert_eq!(
        points(&lines),
        vec![
            (10, 10),
            (20, 10),
            (20, 20),
            (50, 20),
            (55, 20),
            (55, 0),
            (55, 5),
            (1, 1),
            (12, 12),
            (13, 13)
        ]
    );
    assert!(lines.filled, "a closed figure is filled");
    let open = path("M 0 0 L 10 10");
    assert!(!open.filled);
}

#[test]
fn draws_curves_and_their_smooth_continuations() {
    let curves = path(
        "M 0 0 C 0 10 10 10 10 0 S 20 -10 20 0 Q 25 10 30 0 T 40 0 c 1 1 2 2 3 3 s 1 1 2 2 q 1 1 2 2 t 2 2",
    );
    let commands = &curves.commands;
    assert_eq!(commands.len(), 9);
    let PathCommand::CubicTo(first, _, end) = commands[2] else {
        panic!("a smooth curve");
    };
    assert_eq!(
        (first.x, first.y, end.x, end.y),
        (10, -10, 20, 0),
        "the reflection of the last control"
    );
    assert_eq!(points(&curves).last(), Some(&(49, 9)));
    let smooth = path("M 0 0 S 10 10 20 0 T 30 0");
    let PathCommand::CubicTo(first, _, _) = smooth.commands[1] else {
        panic!("a smooth curve");
    };
    assert_eq!((first.x, first.y), (0, 0), "no curve before it to reflect");
}

#[test]
fn draws_elliptical_arcs() {
    let half = path("M 0 50 A 50 50 0 0 1 100 50");
    let ends = points(&half);
    assert_eq!(ends.len(), 17);
    assert_eq!(
        ends[8],
        (50, 0),
        "the top of the circle when sweeping clockwise"
    );
    assert_eq!(*ends.last().expect("a point"), (100, 50));
    let other_way = path("M 0 50 a 50 50 0 0 0 100 0");
    assert_eq!(points(&other_way)[8], (50, 100));
    let large = path("M 0 50 A 10 10 0 1 1 100 50");
    assert_eq!(
        *points(&large).last().expect("a point"),
        (100, 50),
        "a radius too small grows"
    );
    let rotated = path("M 0 50 A 50 25 90 0 1 100 50");
    assert_eq!(*points(&rotated).last().expect("a point"), (100, 50));
    let flat = path("M 0 0 A 0 10 0 0 1 10 10 A 5 5 0 0 1 10 10");
    assert_eq!(points(&flat), vec![(0, 0), (10, 10), (10, 10)]);
}

#[test]
fn skips_what_it_cannot_read() {
    let skipped = path("M 0 0 L 5 X 1 2 L 3 3");
    assert_eq!(points(&skipped), vec![(0, 0), (3, 3)]);
    let element = element("path", "");
    assert_eq!(svg_path_geometry(&element, 10, 10), None);
    let no_view_box = element_with_data("M 0 0 L 7 7");
    let paths = svg_path_geometry(&no_view_box, 100, 100).expect("a path");
    assert_eq!(points(&paths[0]), vec![(0, 0), (7, 7)]);
}

fn element_with_data(data: &str) -> Element {
    element("path", &format!(r#"svg:d="{data}""#))
}
