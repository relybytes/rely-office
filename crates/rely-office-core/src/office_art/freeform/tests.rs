// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;
use crate::geometry::{PathCommand, PathPoint};

/// An `IMsoArray` of elements of `size` bytes.
fn array(size: u16, elements: &[Vec<u8>]) -> Vec<u8> {
    let count = u16::try_from(elements.len()).unwrap();
    let mut bytes = count.to_le_bytes().to_vec();
    bytes.extend_from_slice(&count.to_le_bytes());
    bytes.extend_from_slice(&size.to_le_bytes());
    for element in elements {
        bytes.extend_from_slice(element);
    }
    bytes
}

fn short_points(points: &[(i16, i16)]) -> Vec<u8> {
    let elements: Vec<Vec<u8>> = points
        .iter()
        .map(|(x, y)| [x.to_le_bytes(), y.to_le_bytes()].concat())
        .collect();
    array(4, &elements)
}

fn segments(values: &[u16]) -> Vec<u8> {
    let elements: Vec<Vec<u8>> = values
        .iter()
        .map(|value| value.to_le_bytes().to_vec())
        .collect();
    array(2, &elements)
}

fn ends(paths: &[ShapePath]) -> Vec<(i64, i64)> {
    paths[0]
        .commands
        .iter()
        .filter_map(|command| match command {
            PathCommand::MoveTo(PathPoint { x, y })
            | PathCommand::LineTo(PathPoint { x, y })
            | PathCommand::CubicTo(_, _, PathPoint { x, y }) => Some((*x, *y)),
            PathCommand::Close => None,
        })
        .collect()
}

#[test]
fn draws_vertices_through_their_segments() {
    let mut freeform = Freeform::default();
    freeform.read_vertices(&short_points(&[
        (0, 0),
        (21_600, 0),
        (21_600, 21_600),
        (0, 0),
        (100, 100),
        (200, 200),
        (0, 21_600),
    ]));
    freeform.read_segments(&segments(&[
        0x4000, 0x0002, 0x2001, 0x0001, 0x6001, 0xAA01, 0x8000,
    ]));
    let paths = freeform.paths(100, 200).unwrap();
    assert_eq!(paths.len(), 1);
    assert_eq!(
        ends(&paths),
        vec![(0, 0), (100, 0), (100, 200), (1, 2), (0, 200)]
    );
    assert_eq!(paths[0].commands[5], PathCommand::Close);
}

#[test]
fn ends_a_path_and_starts_another() {
    let mut freeform = Freeform::default();
    freeform.read_vertices(&short_points(&[(0, 0), (10, 10), (20, 20), (30, 30)]));
    freeform.read_segments(&segments(&[0x4000, 0x0001, 0x8000, 0x4000, 0x0001, 0x8000]));
    freeform.bounds = [0, 0, 100, 100];
    let paths = freeform.paths(100, 100).unwrap();
    assert_eq!(paths.len(), 2);
    assert_eq!(ends(&paths[1..]), vec![(20, 20), (30, 30)]);
}

#[test]
fn joins_vertices_with_lines_when_there_are_no_segments() {
    let mut freeform = Freeform::default();
    freeform.read_vertices(&short_points(&[(0, 0), (21_600, 21_600), (0, 21_600)]));
    let paths = freeform.paths(216, 216).unwrap();
    assert_eq!(ends(&paths), vec![(0, 0), (216, 216), (0, 216)]);
    assert_eq!(Freeform::default().paths(10, 10), None);
}

#[test]
fn evaluates_the_guides_a_vertex_names() {
    let mut adjust = [0.0; 10];
    adjust[0] = 300.0;
    let mut freeform = Freeform {
        bounds: [0, 0, 1_000, 500],
        adjust,
        ..Freeform::default()
    };
    let guide = |formula: u16, first: u16, second: u16, third: u16| {
        [formula, first, second, third]
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<u8>>()
    };
    let guides = vec![
        guide(0x2000 | 0x4000, 0x0142, 0x0147, 0), // right + adjust - 0 = 1300
        guide(0x2001 | 0x8000, 0x0400, 1, 0x04FA), // guide0 * 1 / width = 1.3
        guide(0x2002, 0x0143, 100, 0),             // (bottom + 100) / 2 = 300
        guide(3, 0xFFF6, 0, 0),                    // |-10| = 10
        guide(4, 5, 7, 0),
        guide(5, 5, 7, 0),
        guide(6, 0xFFFF, 1, 2), // -1 is not above zero
        guide(7, 3, 4, 0),
        guide(8, 0, 1, 0),             // atan2 of a quarter turn
        guide(0x4009, 100, 0x0408, 0), // 100 sin 90
        guide(0x400A, 100, 0x0408, 0), // 100 cos 90
        guide(11, 10, 0, 1),
        guide(12, 10, 0, 1),
        guide(13, 16, 0, 0),
        guide(14, 1, 2, 1),
        guide(15, 0, 10, 20),
        guide(16, 10, 0, 0),
        guide(0x1FFF, 1, 1, 1),
        guide(0x2000, 0x04F8, 0, 0),
        guide(0x2000, 0x04F9, 0, 0),
        guide(0x2000, 0x04FB, 0, 0),
        guide(0x2000, 0x0140, 0, 0),
        guide(0x2000, 0x0141, 0, 0),
        guide(0x2000, 0x0600, 0, 0),
        guide(1, 1, 1, 0),
    ];
    freeform.read_guides(&array(8, &guides));
    let values = freeform.guide_values((1.0, 1.0));
    assert_eq!(values[0], 1_300.0);
    assert!((values[1] - 1.3).abs() < 1e-9);
    assert_eq!(values[2], 300.0);
    assert_eq!(values[3], 10.0);
    assert_eq!((values[4], values[5]), (5.0, 7.0));
    assert_eq!(values[6], 2.0);
    assert_eq!(values[7], 5.0);
    assert!((values[8] - 90.0 * ANGLE_UNIT).abs() < 1e-3);
    assert!((values[9] - 100.0).abs() < 1e-9);
    assert!(values[10].abs() < 1e-9);
    assert!((values[11] - 0.0).abs() < 1e-9);
    assert!((values[12] - 10.0).abs() < 1e-9);
    assert_eq!(values[13], 4.0);
    assert_eq!(values[14], 1.0 + ANGLE_UNIT);
    assert_eq!(values[15], 20.0);
    assert_eq!(values[16], 0.0);
    assert_eq!(values[17], 0.0, "a formula it does not know");
    assert_eq!((values[18], values[19], values[20]), (500.0, 250.0, 500.0));
    assert_eq!((values[21], values[22], values[23]), (0.0, 0.0, 0.0));
    assert_eq!(values[24], 0.0, "a product over nothing");

    let wide: Vec<Vec<u8>> = [
        (0x8000_0000_u32, 0_u32),
        (0x8000_0002, 0x8000_0000),
        (500, 250),
    ]
    .iter()
    .map(|(x, y)| [x.to_le_bytes(), y.to_le_bytes()].concat())
    .collect();
    freeform.read_vertices(&array(8, &wide));
    assert_eq!(
        freeform.vertices,
        vec![
            (Coordinate::Guide(0), Coordinate::Number(0)),
            (Coordinate::Guide(2), Coordinate::Guide(0)),
            (Coordinate::Number(500), Coordinate::Number(250)),
        ]
    );
    let paths = freeform.paths(1_000, 500).unwrap();
    assert_eq!(ends(&paths), vec![(1_300, 0), (300, 1_300), (500, 250)]);
}

#[test]
fn takes_the_space_from_the_vertices_when_it_has_no_size() {
    let mut freeform = Freeform {
        bounds: [0, 0, 0, 0],
        ..Freeform::default()
    };
    freeform.read_vertices(&short_points(&[(0, 0), (50, 0), (50, 25)]));
    let paths = freeform.paths(100, 100).unwrap();
    assert_eq!(ends(&paths), vec![(0, 0), (100, 0), (100, 100)]);
}

#[test]
fn reads_only_the_arrays_it_understands() {
    let mut freeform = Freeform::default();
    freeform.read_vertices(&array(3, &[vec![1, 2, 3]]));
    assert_eq!(freeform.vertices, Vec::new());
    freeform.read_guides(&array(4, &[vec![1, 2, 3, 4]]));
    assert_eq!(freeform.guides, Vec::<Guide>::new());
    assert_eq!(coordinate(-5, false), Coordinate::Number(-5));
    assert_eq!(coordinate(-32_765, false), Coordinate::Number(-32_765));
    assert_eq!(
        coordinate(-5, true),
        Coordinate::Number(-5),
        "a negative number is no guide"
    );
}

fn table_freeform(vertices: &[(i64, i64)], segments: &[u16]) -> Freeform {
    Freeform {
        vertices: vertices
            .iter()
            .map(|(x, y)| (Coordinate::Number(*x), Coordinate::Number(*y)))
            .collect(),
        segments: segments.to_vec(),
        ..Freeform::default()
    }
}

fn last(paths: &[ShapePath]) -> (i64, i64) {
    *ends(paths).last().unwrap()
}

#[test]
fn draws_arcs_quadrants_and_quadratic_curves() {
    // A counter-clockwise arc over the top half of a circle, from its right to its left.
    let arc = table_freeform(
        &[(0, 0), (21_600, 21_600), (21_600, 10_800), (0, 10_800)],
        &[0xA404, 0x8000],
    );
    let paths = arc.paths(216, 216).unwrap();
    assert_eq!(ends(&paths)[0], (216, 108));
    assert_eq!(last(&paths), (0, 108));
    let top = ends(&paths).iter().map(|(_, y)| *y).min().unwrap();
    assert!(top <= 1, "{top}");
    // Clockwise the same ends go under.
    let below = table_freeform(
        &[(0, 0), (21_600, 21_600), (21_600, 10_800), (0, 10_800)],
        &[0xA604, 0x8000],
    );
    let lowest = ends(&below.paths(216, 216).unwrap())
        .iter()
        .map(|(_, y)| *y)
        .max()
        .unwrap();
    assert!(lowest >= 215, "{lowest}");
    // A rectangle written from its far corner on one axis turns it round.
    let turned = table_freeform(
        &[(21_600, 0), (0, 21_600), (21_600, 10_800), (0, 10_800)],
        &[0xA404, 0x8000],
    );
    let turned = turned.paths(216, 216).unwrap();
    assert_eq!(ends(&turned)[0], (0, 108));
    // Quadrants alternate across and down: a rounded corner and its way back.
    let quadrant = table_freeform(
        &[(0, 21_600), (21_600, 0), (0, 21_600)],
        &[0x4000, 0xA702, 0x8000],
    );
    let paths = quadrant.paths(100, 100).unwrap();
    assert_eq!(last(&paths), (0, 100));
    assert!(matches!(paths[0].commands[1], PathCommand::CubicTo(..)));
    let curve = table_freeform(
        &[(0, 0), (10_800, 21_600), (21_600, 0)],
        &[0x4000, 0xA902, 0x8000],
    );
    assert_eq!(last(&curve.paths(100, 100).unwrap()), (100, 0));
}

#[test]
fn draws_an_angle_ellipse_and_leaves_unfilled_and_unlined_paths() {
    let mut ellipse = table_freeform(
        &[(10_800, 10_800), (10_800, 10_800), (0, 90 * 65_536)],
        &[0xA203, 0xAA00, 0x8000, 0x4000, 0x0001, 0xAB00, 0x8000],
    );
    ellipse.vertices.extend([
        (Coordinate::Number(0), Coordinate::Number(0)),
        (Coordinate::Number(21_600), Coordinate::Number(0)),
    ]);
    let paths = ellipse.paths(216, 216).unwrap();
    assert_eq!(paths.len(), 2);
    // From the right a quarter turn counter-clockwise reaches the top.
    assert_eq!(ends(&paths)[0], (216, 108));
    assert_eq!(last(&paths[..1]), (108, 0));
    assert!(!paths[0].filled && paths[0].stroked);
    assert!(paths[1].filled && !paths[1].stroked);
}

#[test]
fn reads_libreoffice_angles_in_degrees_and_guides_written_later() {
    let freeform = Freeform {
        degrees: true,
        guides: vec![
            Guide {
                operation: 0x6000,
                parameters: [0x0401, 0x0147, 0],
            },
            Guide {
                operation: 0x4009,
                parameters: [100, 0x0148, 0],
            },
            Guide {
                operation: 0x000E,
                parameters: [10, 30, 5],
            },
            Guide {
                operation: 0x2000,
                parameters: [0x0402, 0, 0],
            },
        ],
        adjust: [1.0, 90.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        ..Freeform::default()
    };
    let values = freeform.guide_values((1.0, 1.0));
    assert!((values[0] - 101.0).abs() < 1e-9, "{values:?}");
    assert!((values[1] - 100.0).abs() < 1e-9);
    assert!((values[2] - 35.0).abs() < 1e-9);
    assert!((values[3] - 35.0).abs() < 1e-9);
    let looping = Freeform {
        guides: vec![Guide {
            operation: 0x2000,
            parameters: [0x0400, 0, 0],
        }],
        ..Freeform::default()
    };
    assert_eq!(looping.guide_values((1.0, 1.0)), vec![0.0]);
}
