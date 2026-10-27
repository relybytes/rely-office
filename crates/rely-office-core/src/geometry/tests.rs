// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;

fn at(x: i64, y: i64) -> PathPoint {
    PathPoint { x, y }
}

fn end_of(command: &PathCommand) -> Option<PathPoint> {
    match command {
        PathCommand::MoveTo(point)
        | PathCommand::LineTo(point)
        | PathCommand::CubicTo(_, _, point) => Some(*point),
        PathCommand::Close => None,
    }
}

#[test]
fn starts_a_path_with_its_first_line() {
    let mut builder = PathBuilder::default();
    builder.line_to(10.0, 20.0);
    builder.line_to(30.4, 40.6);
    builder.close();
    let path = builder.finish(true, false).expect("a path");
    assert_eq!(
        path,
        ShapePath {
            commands: vec![
                PathCommand::MoveTo(at(10, 20)),
                PathCommand::LineTo(at(30, 41)),
                PathCommand::Close,
            ],
            filled: true,
            stroked: false,
        }
    );
}

#[test]
fn draws_nothing_without_a_command() {
    let mut builder = PathBuilder::default();
    builder.close();
    assert_eq!(builder.finish(true, true), None);
}

#[test]
fn returns_to_the_start_of_a_figure_when_it_closes() {
    let mut builder = PathBuilder::default();
    builder.move_to(5.0, 5.0);
    builder.line_to(50.0, 5.0);
    builder.close();
    assert_eq!(builder.current(), (5.0, 5.0));
}

#[test]
fn draws_a_quadratic_curve_as_the_same_cubic_curve() {
    let mut builder = PathBuilder::default();
    builder.move_to(0.0, 0.0);
    builder.quad_to((30.0, 30.0), (60.0, 0.0));
    let path = builder.finish(false, true).expect("a path");
    assert_eq!(
        path.commands[1],
        PathCommand::CubicTo(at(20, 20), at(40, 20), at(60, 0))
    );
}

#[test]
fn starts_a_curve_from_the_pen_when_nothing_is_drawn() {
    let mut builder = PathBuilder::default();
    builder.cubic_to((1.0, 1.0), (2.0, 2.0), (3.0, 3.0));
    let path = builder.finish(true, true).expect("a path");
    assert_eq!(path.commands[0], PathCommand::MoveTo(at(0, 0)));
    assert_eq!(path.commands.len(), 2);
}

#[test]
fn draws_an_arc_in_quarter_turns_and_leads_to_it_with_a_line() {
    let mut builder = PathBuilder::default();
    builder.move_to(0.0, 0.0);
    builder.ellipse_arc((100.0, 100.0), (100.0, 50.0), 0.0, TAU);
    let path = builder.finish(true, true).expect("a path");
    assert_eq!(path.commands[1], PathCommand::LineTo(at(200, 100)));
    let ends: Vec<PathPoint> = path.commands[2..].iter().filter_map(end_of).collect();
    assert_eq!(
        ends,
        vec![at(100, 150), at(0, 100), at(100, 50), at(200, 100)]
    );
    let PathCommand::CubicTo(first, second, _) = path.commands[2] else {
        panic!("a curve");
    };
    assert_eq!((first, second), (at(200, 128), at(155, 150)));
}

#[test]
fn starts_an_arc_without_a_line_from_where_the_pen_is() {
    let mut builder = PathBuilder::default();
    builder.ellipse_arc((0.0, 0.0), (10.0, 10.0), 0.0, -FRAC_PI_2);
    builder.ellipse_arc((0.0, 0.0), (10.0, 10.0), -FRAC_PI_2, 0.0);
    builder.ellipse_arc((0.0, 0.0), (10.0, 10.0), -FRAC_PI_2, f64::NAN);
    let path = builder.finish(true, true).expect("a path");
    assert_eq!(path.commands[0], PathCommand::MoveTo(at(10, 0)));
    assert_eq!(path.commands.len(), 2);
    assert_eq!(end_of(&path.commands[1]), Some(at(0, -10)));
}

#[test]
fn keeps_a_path_within_its_limits() {
    let mut builder = PathBuilder::default();
    for index in 0..(MAX_COMMANDS + 10) {
        #[expect(clippy::cast_precision_loss, reason = "a small index")]
        builder.line_to(index as f64, 0.0);
    }
    builder.line_to(f64::INFINITY, 1.0e12);
    let path = builder.finish(true, true).expect("a path");
    assert_eq!(path.commands.len(), MAX_COMMANDS);
    assert_eq!(point(f64::NAN, -1.0e12), at(0, -2_000_000_000));
}

#[test]
fn mirrors_paths_across_their_frame() {
    let original = vec![ShapePath {
        commands: vec![
            PathCommand::MoveTo(at(10, 20)),
            PathCommand::LineTo(at(30, 0)),
            PathCommand::CubicTo(at(1, 2), at(3, 4), at(5, 6)),
            PathCommand::Close,
        ],
        filled: true,
        stroked: true,
    }];
    let mut paths = original.clone();
    flip_paths(&mut paths, 100, 50, false, false);
    assert_eq!(paths, original);
    flip_paths(&mut paths, 100, 50, true, false);
    assert_eq!(paths[0].commands[0], PathCommand::MoveTo(at(90, 20)));
    flip_paths(&mut paths, 100, 50, true, true);
    assert_eq!(paths[0].commands[0], PathCommand::MoveTo(at(10, 30)));
    assert_eq!(
        paths[0].commands[2],
        PathCommand::CubicTo(at(1, 48), at(3, 46), at(5, 44))
    );
    assert_eq!(paths[0].commands[3], PathCommand::Close);
}
