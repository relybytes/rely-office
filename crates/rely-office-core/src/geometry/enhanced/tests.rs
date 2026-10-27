// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;
use crate::geometry::PathCommand;
use crate::xml_tree::parse_tree;

const NAMESPACES: &str = concat!(
    r#"xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" "#,
    r#"xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0""#
);

/// A frame of 21,600 by 21,600 hundredths of a millimetre drawn at one unit to the unit.
const SIDE: i64 = 21_600;

fn geometry(attributes: &str, equations: &str) -> Element {
    let xml = format!(
        "<draw:enhanced-geometry {NAMESPACES} {attributes}>{equations}</draw:enhanced-geometry>"
    );
    parse_tree(&xml, |_, _| false).expect("the geometry parses")
}

fn drawn(path: &str) -> Vec<ShapePath> {
    let geometry = geometry(&format!(r#"draw:enhanced-path="{path}""#), "");
    enhanced_geometry(&geometry, SIDE, SIDE).expect("a path")
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

fn context(modifiers: Vec<f64>, equations: &[&str]) -> Context {
    let mut variables = HashMap::new();
    variables.insert("width", 100.0);
    variables.insert("pi", PI);
    Context {
        variables,
        modifiers,
        equations: equations
            .iter()
            .map(|formula| parse_formula(formula))
            .collect(),
        values: RefCell::new(HashMap::new()),
    }
}

fn value(formula: &str) -> f64 {
    let context = context(vec![3.0], &["2", "?f0 *10", "?f3", "?f2"]);
    context.evaluate(&parse_formula(formula).expect("a formula"), 0)
}

#[test]
fn evaluates_formulas_with_their_precedence_and_functions() {
    assert_eq!(value("1 + 2 * 3 - 4 / 2"), 5.0);
    assert_eq!(value("(1 + 2) * 3"), 9.0);
    assert_eq!(value("-width + +4"), -96.0);
    assert_eq!(value("$0 * ?f1"), 60.0);
    assert_eq!(value("$7 + ?f9 + nothing"), 0.0);
    assert_eq!(value("4 / 0"), 0.0);
    assert_eq!(value("abs(-2) + sqrt(16) + sqrt(-1)"), 6.0);
    assert!((value("sin(pi/2) + cos(0) + tan(0) + atan(1) * 4 / pi") - 3.0).abs() < 1e-9);
    assert!((value("atan2(1, 0)") - PI / 2.0).abs() < 1e-9);
    assert_eq!(value("min(3, 9) + max(3, 9)"), 12.0);
    assert_eq!(value("if(1, 5, 6) + if(-1, 5, 6)"), 11.0);
    assert_eq!(value("unknown(1) + abs()"), 0.0);
    assert_eq!(value("1.5e1"), 15.0);
    assert_eq!(
        value("?f2"),
        0.0,
        "equations that refer to each other in a loop are 0"
    );
}

#[test]
fn refuses_a_formula_it_cannot_read() {
    for broken in ["", "(1", "1 +", "?g1", "$x", "min(1 2)", "1 )", "#", "?f"] {
        assert_eq!(parse_formula(broken), None, "{broken}");
    }
    let deep = format!("{}1{}", "(".repeat(200), ")".repeat(200));
    assert_eq!(parse_formula(&deep), None);
    let negated = format!("{}1", "-".repeat(200));
    assert_eq!(parse_formula(&negated), None);
    let context = context(Vec::new(), &[]);
    let mut nested = Expression::Number(1.0);
    for _ in 0..(MAX_DEPTH + 2) {
        nested = Expression::Negate(Box::new(nested));
    }
    assert_eq!(context.evaluate(&nested, 0), 0.0);
}

#[test]
fn reads_path_parameters() {
    let context = context(vec![4.0], &["7"]);
    assert_eq!(context.parameter("?f0"), 7.0);
    assert_eq!(context.parameter("$0"), 4.0);
    assert_eq!(context.parameter("$9"), 0.0);
    assert_eq!(context.parameter("12.5"), 12.5);
    assert_eq!(context.parameter("x"), 0.0);
}

#[test]
fn splits_a_path_into_commands_and_parameters() {
    assert_eq!(
        tokens("M0 0L?f1,$0 Z N"),
        vec!["M", "0", "0", "L", "?f1", "$0", "Z", "N"]
    );
}

#[test]
fn draws_lines_curves_and_closes() {
    let paths = drawn("M 0 0 100 0 L 100 100 C 1 2 3 4 5 6 Q 10 10 20 20 Z N F S M 0 0 L 9 9");
    assert_eq!(paths.len(), 2);
    assert_eq!(
        points(&paths[0]),
        vec![(0, 0), (100, 0), (100, 100), (5, 6), (20, 20)]
    );
    assert_eq!(paths[0].commands.last(), Some(&PathCommand::Close));
    assert!(paths[0].filled && paths[0].stroked);
    assert!(!paths[1].filled && !paths[1].stroked);
}

#[test]
fn skips_commands_it_does_not_know_and_incomplete_parameters() {
    let paths = drawn("M 0 0 L 10 K 5 H M 1 1 L 2");
    assert_eq!(points(&paths[0]), vec![(0, 0), (1, 1)]);
}

#[test]
fn draws_angle_ellipses() {
    let full = drawn("U 10800 10800 10800 10800 0 360 Z");
    assert_eq!(points(&full[0])[0], (21_600, 10_800));
    assert_eq!(*points(&full[0]).last().expect("a point"), (21_600, 10_800));
    assert_eq!(points(&full[0]).len(), 5);
    let arc = drawn("M 0 0 T 10800 10800 10800 10800 90 0");
    let arc = points(&arc[0]);
    assert_eq!(arc[1], (10_800, 0), "a line to where the arc starts");
    assert_eq!(
        arc.len(),
        5,
        "three quarters, the end angle taken past the start"
    );
    assert_eq!(*arc.last().expect("a point"), (21_600, 10_800));
}

#[test]
fn draws_arcs_through_their_bounding_box() {
    let counter_clockwise = drawn("M 21600 10800 A 0 0 21600 21600 21600 10800 10800 0");
    assert_eq!(
        *points(&counter_clockwise[0]).last().expect("a point"),
        (10_800, 0)
    );
    assert_eq!(points(&counter_clockwise[0]).len(), 2, "a quarter, upwards");
    let clockwise = drawn("W 0 0 21600 21600 21600 10800 10800 0 N");
    let clockwise = points(&clockwise[0]);
    assert_eq!(clockwise[0], (21_600, 10_800));
    assert_eq!(clockwise.len(), 4, "three quarters, downwards");
    let arc = drawn("B 0 0 21600 21600 21600 10800 21600 10800");
    assert_eq!(points(&arc[0]).len(), 5, "a full turn when both ends meet");
    let clockwise_arc = drawn("V 0 0 21600 21600 21600 10800 21600 10800");
    assert_eq!(points(&clockwise_arc[0]).len(), 5);
    let flat = drawn("M 5 5 A 0 0 0 21600 1 1 2 2 L 6 6");
    assert_eq!(points(&flat[0]), vec![(5, 5), (6, 6)]);
}

#[test]
fn draws_elliptical_quadrants_alternating_their_start() {
    let paths = drawn("M 0 10800 Y 10800 0 21600 10800 X 10800 21600");
    let commands = &paths[0].commands;
    let PathCommand::CubicTo(first, _, end) = commands[1] else {
        panic!("a quadrant");
    };
    assert_eq!(
        (first.x, end),
        (0, crate::geometry::PathPoint { x: 10_800, y: 0 })
    );
    let PathCommand::CubicTo(first, _, _) = commands[2] else {
        panic!("a quadrant");
    };
    assert_eq!(first.y, 0, "the second quadrant leaves horizontally");
    let PathCommand::CubicTo(first, _, _) = commands[3] else {
        panic!("a quadrant");
    };
    assert_eq!(first.y, 10_800, "a fresh X leaves horizontally again");
}

#[test]
fn draws_an_arc_by_its_angles() {
    let paths = drawn("M 0 10800 G 10800 10800 180 90");
    assert_eq!(*points(&paths[0]).last().expect("a point"), (10_800, 0));
}

#[test]
fn maps_the_view_box_onto_the_frame() {
    let scaled = geometry(
        r#"svg:viewBox="100 100 200 400" draw:enhanced-path="M 100 100 L 300 500""#,
        "",
    );
    let paths = enhanced_geometry(&scaled, 2_000, 2_000).expect("a path");
    assert_eq!(points(&paths[0]), vec![(0, 0), (2_000, 2_000)]);
    let logical = geometry(
        r#"svg:viewBox="0 0 0 0" draw:enhanced-path="M 0 0 L logwidth 0""#,
        "",
    );
    let paths = enhanced_geometry(&logical, 3_600, 3_600).expect("a path");
    assert_eq!(
        points(&paths[0]),
        vec![(0, 0)],
        "a variable is no parameter"
    );
    let logical = geometry(
        r#"svg:viewBox="0 0 0 0" draw:enhanced-path="M 0 0 L ?f0 ?f1""#,
        r#"<draw:equation draw:name="f0" draw:formula="logwidth"/><draw:equation draw:name="f1" draw:formula="logheight/2"/><draw:equation draw:formula="1"/><draw:equation draw:name="g2" draw:formula="1"/>"#,
    );
    let paths = enhanced_geometry(&logical, 3_600, 7_200).expect("a path");
    assert_eq!(points(&paths[0]), vec![(0, 0), (3_600, 3_600)]);
    let broken = geometry(
        r#"svg:viewBox="1 2" draw:enhanced-path="M 0 0 L 21600 21600""#,
        "",
    );
    let paths = enhanced_geometry(&broken, 100, 100).expect("a path");
    assert_eq!(points(&paths[0]), vec![(0, 0), (100, 100)]);
}

#[test]
fn mirrors_and_reads_modifiers() {
    let mirrored = geometry(
        r#"draw:modifiers="5400" draw:mirror-horizontal="true" draw:enhanced-path="M 0 0 L $0 0""#,
        "",
    );
    let paths = enhanced_geometry(&mirrored, SIDE, SIDE).expect("a path");
    assert_eq!(points(&paths[0]), vec![(21_600, 0), (16_200, 0)]);
}

#[test]
fn draws_a_drawingml_preset_from_its_type_and_modifiers() {
    let preset = geometry(
        r#"draw:type="ooxml-rightArrow" draw:modifiers="20000 100000" draw:mirror-vertical="true" draw:enhanced-path="M 0 0 Z""#,
        "",
    );
    let paths = enhanced_geometry(&preset, 200, 100).expect("a preset");
    assert_eq!(points(&paths[0])[0], (0, 60));
    let unknown = geometry(
        r#"draw:type="ooxml-noSuchShape" draw:enhanced-path="M 0 0 L 1 1""#,
        "",
    );
    assert_eq!(
        enhanced_geometry(&unknown, SIDE, SIDE).map(|paths| paths.len()),
        Some(1)
    );
}

#[test]
fn has_no_geometry_without_a_path() {
    assert_eq!(enhanced_geometry(&geometry("", ""), SIDE, SIDE), None);
    assert_eq!(
        enhanced_geometry(&geometry(r#"draw:enhanced-path="N""#, ""), SIDE, SIDE),
        None
    );
}

#[test]
fn draws_every_preset_of_the_enhanced_table_inside_its_frame() {
    assert!(enhanced_presets().len() > 90);
    for name in enhanced_presets().keys() {
        let paths = enhanced_preset_geometry(name, &[], &[], 200, 100)
            .unwrap_or_else(|| panic!("{name} draws nothing"));
        for path in &paths {
            for (x, y) in points(path) {
                assert!(
                    (-200..=400).contains(&x) && (-200..=300).contains(&y),
                    "{name} reaches {x}, {y}"
                );
            }
        }
    }
    assert_eq!(
        enhanced_preset_geometry("noSuchShape", &[], &[], 10, 10),
        None
    );
}

#[test]
fn reads_enhanced_preset_modifiers_by_name_over_their_position() {
    let crest = |paths: &[ShapePath]| points(&paths[0])[0].1;
    let flat = enhanced_preset_geometry("wave", &[0.0], &[], 21_600, 21_600).expect("a wave");
    assert_eq!(crest(&flat), 0);
    let positional =
        enhanced_preset_geometry("wave", &[20_000.0], &[], 21_600, 21_600).expect("a wave");
    let named = enhanced_preset_geometry(
        "wave",
        &[0.0],
        &[("adj1".to_owned(), 20_000.0)],
        21_600,
        21_600,
    )
    .expect("a wave");
    assert_eq!(crest(&positional), 4_320);
    assert_eq!(crest(&named), 4_320, "the name wins over the position");
    let single = enhanced_preset_geometry("diagStripe", &[], &[("adj1".to_owned(), 0.0)], 100, 100)
        .expect("a stripe");
    let default = enhanced_preset_geometry("diagStripe", &[], &[], 100, 100).expect("a stripe");
    assert_ne!(
        single, default,
        "adj1 reaches a preset whose only modifier is adj"
    );
}

#[test]
fn scales_each_subpath_from_its_own_view() {
    let paths = enhanced_preset_geometry("chartPlus", &[], &[], 200, 100).expect("a chart");
    assert_eq!(points(&paths[0])[..2], [(100, 0), (100, 100)]);
    assert!(!paths[0].filled && paths[0].stroked);
    assert!(paths[1].filled && !paths[1].stroked);
}

#[test]
fn shades_the_faces_the_path_marks() {
    let paths =
        drawn("M 0 0 L 1 1 H N M 0 0 L 1 1 I N M 0 0 L 1 1 J N M 0 0 L 1 1 K N M 0 0 L 1 1");
    let shades: Vec<FillShade> = paths.iter().map(|path| path.shade).collect();
    assert_eq!(
        shades,
        [
            FillShade::Darken,
            FillShade::DarkenLess,
            FillShade::Lighten,
            FillShade::LightenLess,
            FillShade::Normal
        ]
    );
    let can = enhanced_preset_geometry("can", &[], &[], 100, 200).expect("a can");
    assert_eq!(
        can[1].shade,
        FillShade::Lighten,
        "the top of a can is lighter"
    );
}

#[test]
fn computes_a_chain_of_equations_longer_than_the_depth_limit() {
    let mut equations = vec!["1".to_owned()];
    for index in 0..200 {
        equations.push(format!("?f{index} +1"));
    }
    let formulas: Vec<&str> = equations.iter().map(String::as_str).collect();
    let chained = context(Vec::new(), &formulas);
    chained.compute_in_order();
    assert!((chained.equation(200, 0) - 201.0).abs() < f64::EPSILON);
}
