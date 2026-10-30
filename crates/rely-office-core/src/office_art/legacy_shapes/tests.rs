// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use super::*;
use crate::geometry::{FillShade, PathCommand, PathPoint};

fn points(paths: &[ShapePath]) -> Vec<(i64, i64)> {
    paths
        .iter()
        .flat_map(|path| path.commands.iter())
        .filter_map(|command| match command {
            PathCommand::MoveTo(PathPoint { x, y })
            | PathCommand::LineTo(PathPoint { x, y })
            | PathCommand::CubicTo(_, _, PathPoint { x, y }) => Some((*x, *y)),
            PathCommand::Close => None,
        })
        .collect()
}

fn bounds(paths: &[ShapePath]) -> (i64, i64, i64, i64) {
    points(paths)
        .into_iter()
        .fold((i64::MAX, i64::MAX, i64::MIN, i64::MIN), |found, (x, y)| {
            (
                found.0.min(x),
                found.1.min(y),
                found.2.max(x),
                found.3.max(y),
            )
        })
}

#[test]
fn draws_every_type_inside_its_frame() {
    for kind in [
        83, 89, 90, 91, 99, 101, 102, 103, 104, 105, 107, 108, 182, 189, 190, 191, 192, 193, 194,
        195, 196, 197, 198, 199, 200,
    ] {
        let paths = legacy_shape_paths(kind, &[None; 10], 2_160, 1_080).unwrap();
        let (left, top, right, bottom) = bounds(&paths);
        // The circular arrow's head reaches past its frame, as it does in PowerPoint.
        let reach = if kind == 99 { 400 } else { 1 };
        assert!(
            left >= -reach && top >= -reach && right <= 2_160 + reach && bottom <= 1_080 + reach,
            "type {kind}: {left} {top} {right} {bottom}"
        );
        assert!(right - left > 1_000 && bottom - top > 500, "type {kind}");
    }
    assert!(legacy_shape_paths(1, &[None; 10], 100, 100).is_none());
    assert!(legacy_shape_paths(13, &[Some(5_400); 10], 100, 100).is_none());
}

#[test]
fn places_its_vertices_from_the_adjust_values() {
    // The bent arrow's head starts at the first value across and its shaft at the second down.
    let mut adjust = [None; 10];
    adjust[0] = Some(10_800);
    adjust[1] = Some(2_160);
    let found = points(&legacy_shape_paths(91, &adjust, 21_600, 21_600).unwrap());
    assert!(found.contains(&(10_800, 2_160)), "{found:?}");
    assert!(found.contains(&(21_600, 6_079)));
    // Left unset, a value takes the type's default.
    let found = points(&legacy_shape_paths(91, &[None; 10], 21_600, 21_600).unwrap());
    assert!(found.contains(&(15_100, 2_900)), "{found:?}");
    // The left, right and up arrow reads a guide written after the one that names it.
    let found = points(&legacy_shape_paths(182, &[None; 10], 21_600, 21_600).unwrap());
    assert!(found.contains(&(0, 10_800)) && found.contains(&(21_600, 10_800)));
    let tips = found.iter().filter(|(_, y)| *y == 10_800).count();
    assert_eq!(tips, 2, "{found:?}");
}

#[test]
fn reads_the_circular_arrow_angles_in_sixteenths_of_a_degree() {
    let mut adjust = [None; 10];
    adjust[0] = Some(180 * 65_536);
    adjust[1] = Some(0);
    adjust[2] = Some(5_500);
    let set = legacy_shape_paths(99, &adjust, 21_600, 21_600).unwrap();
    let default = legacy_shape_paths(99, &[None; 10], 21_600, 21_600).unwrap();
    assert_eq!(points(&set), points(&default));
}

#[test]
fn keeps_an_action_button_picture_square_and_shades_its_bevel() {
    let wide = legacy_shape_paths(190, &[None; 10], 43_200, 21_600).unwrap();
    assert_eq!(bounds(&wide), (0, 0, 43_200, 21_600));
    // The house stays as wide as it is high in a frame twice as wide.
    let house = points(&wide[5..6]);
    let (left, right) = house.iter().fold((i64::MAX, i64::MIN), |found, (x, _)| {
        (found.0.min(*x), found.1.max(*x))
    });
    let square = points(&legacy_shape_paths(190, &[None; 10], 21_600, 21_600).unwrap()[5..6]);
    let (square_left, square_right) = square.iter().fold((i64::MAX, i64::MIN), |found, (x, _)| {
        (found.0.min(*x), found.1.max(*x))
    });
    assert!(((right - left) - (square_right - square_left)).abs() <= 2);
    assert_eq!(
        wide.iter()
            .take(5)
            .map(|path| path.shade)
            .collect::<Vec<_>>(),
        vec![
            FillShade::Normal,
            FillShade::LightenLess,
            FillShade::Darken,
            FillShade::DarkenLess,
            FillShade::Lighten,
        ]
    );
    // The curved arrows draw their underside darker.
    let curved = legacy_shape_paths(102, &[None; 10], 21_600, 21_600).unwrap();
    assert_eq!(
        (curved[0].shade, curved[1].shade),
        (FillShade::Normal, FillShade::Darken)
    );
}
