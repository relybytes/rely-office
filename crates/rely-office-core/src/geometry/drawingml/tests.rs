// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use super::*;
use crate::geometry::{PathCommand, PathPoint};
use crate::xml_tree::parse_tree;

const A_URI: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";

fn element(xml: &str) -> Element {
    let name_end = xml
        .find(|character: char| character.is_whitespace() || character == '>' || character == '/')
        .expect("a root element");
    let declared = format!(
        "{} xmlns:a=\"{A_URI}\"{}",
        &xml[..name_end],
        &xml[name_end..]
    );
    parse_tree(&declared, |_, _| false).expect("the fragment parses")
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

fn formula(text: &str) -> f64 {
    let mut guides = built_in_guides(200.0, 100.0);
    guides.insert("g".to_owned(), 30.0);
    Formula::parse(text).expect("a formula").evaluate(&guides)
}

#[test]
fn evaluates_every_guide_operator() {
    assert_eq!(formula("*/ w 3 4"), 150.0);
    assert_eq!(formula("*/ w 3 0"), 0.0);
    assert_eq!(formula("+- w g 50"), 180.0);
    assert_eq!(formula("+/ w g 2"), 115.0);
    assert_eq!(formula("+/ w g 0"), 0.0);
    assert_eq!(formula("?: g 1 2"), 1.0);
    assert_eq!(formula("?: -1 1 2"), 2.0);
    assert_eq!(formula("abs -7"), 7.0);
    assert!((formula("at2 1 1") - 2_700_000.0).abs() < 1e-6);
    assert!((formula("cat2 10 1 0") - 10.0).abs() < 1e-9);
    assert!((formula("sat2 10 0 1") - 10.0).abs() < 1e-9);
    assert!((formula("cos 10 cd2") + 10.0).abs() < 1e-9);
    assert!((formula("sin 10 cd4") - 10.0).abs() < 1e-9);
    assert!((formula("tan 10 cd8") - 10.0).abs() < 1e-9);
    assert_eq!(formula("max g 40"), 40.0);
    assert_eq!(formula("min g 40"), 30.0);
    assert_eq!(formula("mod 3 4 0"), 5.0);
    assert_eq!(formula("pin 0 g 20"), 20.0);
    assert_eq!(formula("pin 40 g 50"), 40.0);
    assert_eq!(formula("pin 0 g 50"), 30.0);
    assert_eq!(formula("sqrt 16"), 4.0);
    assert_eq!(formula("sqrt -16"), 0.0);
    assert_eq!(formula("val unknown"), 0.0);
    assert_eq!(formula("val"), 0.0);
    assert_eq!(formula("nonsense 1 2 3"), 0.0);
    assert_eq!(formula("*/ 1e308 1e308 1"), 0.0);
    assert_eq!(Formula::parse("   "), None);
}

#[test]
fn knows_the_built_in_guides() {
    let guides = built_in_guides(200.0, 100.0);
    let guide = |name: &str| guides[name];
    assert_eq!(
        (guide("r"), guide("b"), guide("hc"), guide("vc")),
        (200.0, 100.0, 100.0, 50.0)
    );
    assert_eq!(
        (guide("ss"), guide("ls"), guide("wd8"), guide("hd3")),
        (100.0, 200.0, 25.0, 100.0 / 3.0)
    );
    assert_eq!(
        (guide("ssd32"), guide("cd4"), guide("7cd8")),
        (100.0 / 32.0, 5_400_000.0, 18_900_000.0)
    );
}

#[test]
fn draws_a_preset_at_its_size_with_its_adjust_values() {
    let arrow = preset_geometry("rightArrow", None, 200, 100).expect("a preset");
    assert_eq!(
        points(&arrow[0]),
        vec![
            (0, 25),
            (150, 25),
            (150, 0),
            (200, 50),
            (150, 100),
            (150, 75),
            (0, 75)
        ]
    );
    assert!(arrow[0].filled && arrow[0].stroked);
    let adjusted = element(
        r#"<a:avLst><a:gd name="adj1" fmla="val 20000"/><a:gd name="adj2" fmla="val 100000"/><a:gd fmla="val 1"/><a:other/></a:avLst>"#,
    );
    let thin = preset_geometry("rightArrow", Some(&adjusted), 200, 100).expect("a preset");
    assert_eq!(
        points(&thin[0]),
        vec![
            (0, 40),
            (100, 40),
            (100, 0),
            (200, 50),
            (100, 100),
            (100, 60),
            (0, 60)
        ]
    );
    assert_eq!(preset_geometry("noSuchShape", None, 200, 100), None);
}

#[test]
fn draws_an_alias_as_its_preset() {
    assert_eq!(
        preset_geometry("flowChartDecision", None, 40, 20),
        preset_geometry("diamond", None, 40, 20)
    );
}

#[test]
fn scales_a_path_with_its_own_coordinate_space() {
    let bolt = preset_geometry("lightningBolt", None, 216, 432).expect("a preset");
    let first = points(&bolt[0])[0];
    assert_eq!(first, (85, 0));
    assert!(points(&bolt[0]).contains(&(216, 432)));
}

#[test]
fn draws_regular_polygons_and_stars() {
    let heptagon = preset_geometry("heptagon", None, 100, 100).expect("a preset");
    assert_eq!(points(&heptagon[0]).len(), 7);
    assert_eq!(points(&heptagon[0])[0], (50, 0));
    let star = preset_geometry("star4", None, 100, 100).expect("a preset");
    assert_eq!(
        points(&star[0]),
        vec![
            (50, 0),
            (59, 41),
            (100, 50),
            (59, 59),
            (50, 100),
            (41, 59),
            (0, 50),
            (41, 41)
        ]
    );
    let mut builder = PathBuilder::default();
    polygon(&mut builder, 10.0, 10.0, 1, None);
    assert_eq!(
        builder.finish(true, true).expect("a path").commands.len(),
        4
    );
}

#[test]
fn keeps_a_path_open_and_unstroked_as_it_says() {
    let paths = preset_geometry("borderCallout1", None, 100, 100).expect("a preset");
    assert_eq!(paths.len(), 2);
    assert!(paths[0].filled);
    assert!(!paths[1].filled);
    assert_eq!(points(&paths[1]), vec![(-8, 19), (-38, 113)]);
}

#[test]
fn draws_an_arc_through_the_pen_at_its_visual_angles() {
    let mut builder = PathBuilder::default();
    builder.move_to(0.0, 50.0);
    arc_to(&mut builder, (100.0, 50.0), PI, PI / 2.0);
    let (x, y) = builder.current();
    assert!((x - 100.0).abs() < 1e-6 && y.abs() < 1e-6);
    let mut builder = PathBuilder::default();
    builder.move_to(200.0, 50.0);
    arc_to(&mut builder, (100.0, 50.0), 0.0, -TAU);
    let (x, y) = builder.current();
    assert!((x - 200.0).abs() < 1e-6 && (y - 50.0).abs() < 1e-6);
    assert_eq!(
        builder.finish(true, true).expect("a path").commands.len(),
        5
    );
    let mut builder = PathBuilder::default();
    builder.move_to(200.0, 50.0);
    arc_to(&mut builder, (100.0, 50.0), 0.0, -PI / 2.0 - TAU);
    let (x, y) = builder.current();
    assert!((x - 100.0).abs() < 1e-6 && y.abs() < 1e-6);
}

#[test]
fn reads_a_preset_table() {
    let table = concat!(
        "stray line before a head\n",
        "[one]\n",
        "av adj 5\n",
        "av broken\n",
        "gd x */ w adj 10\n",
        "gd\n",
        "path fill=none stroke=false w=10 h=20 other=1\n",
        "M x 0\n",
        "L 1\n",
        "C 1 2 3\n",
        "regular five\n",
        "unknown 1 2\n",
        "[two=one]\n",
        "[three=missing]\n",
        "[four]\n",
        "Q 1 2 3 4\n",
    );
    let presets = parse_presets(table);
    assert_eq!(presets.len(), 3);
    let one = &presets["one"];
    assert_eq!(one.adjust.len(), 1);
    assert_eq!(one.guides.len(), 1);
    assert_eq!(one.paths.len(), 1);
    assert_eq!(
        (
            one.paths[0].width,
            one.paths[0].height,
            one.paths[0].filled,
            one.paths[0].stroked
        ),
        (Some(10.0), Some(20.0), false, false)
    );
    assert_eq!(one.paths[0].steps.len(), 1);
    assert_eq!(presets["two"], *one);
    assert_eq!(presets["four"].paths[0].steps.len(), 1);
    assert!(!presets.contains_key("three"));
}

#[test]
fn parses_every_preset_of_the_table_into_a_drawn_outline() {
    for (name, definition) in presets() {
        let paths = definition.evaluate(300.0, 200.0, &[]);
        assert!(!paths.is_empty(), "{name} draws nothing");
        for path in &paths {
            for command in &path.commands {
                if let PathCommand::MoveTo(PathPoint { x, y })
                | PathCommand::LineTo(PathPoint { x, y }) = command
                {
                    assert!(
                        (-400..=700).contains(x) && (-400..=700).contains(y),
                        "{name} reaches {x}, {y}"
                    );
                }
            }
        }
    }
    assert!(presets().len() > 80);
}

#[test]
fn draws_a_preset_from_adjust_values_in_their_order() {
    let thin = preset_geometry_with_values("rightArrow", &[20_000.0, 100_000.0, 7.0], 200, 100)
        .expect("a preset");
    assert_eq!(points(&thin[0])[0], (0, 40));
    assert_eq!(
        preset_geometry_with_values("noSuchShape", &[], 10, 10),
        None
    );
}

#[test]
fn draws_a_custom_geometry() {
    let geometry = element(concat!(
        r#"<a:custGeom><a:avLst><a:gd name="adj" fmla="val 50"/></a:avLst>"#,
        r#"<a:gdLst><a:gd name="half" fmla="*/ w adj 100"/></a:gdLst><a:pathLst>"#,
        r#"<a:path w="100" h="100"><a:moveTo><a:pt x="0" y="0"/></a:moveTo><a:lnTo><a:pt x="half" y="0"/></a:lnTo>"#,
        r#"<a:arcTo wR="10" hR="10" stAng="16200000" swAng="5400000"/>"#,
        r#"<a:quadBezTo><a:pt x="10" y="10"/><a:pt x="20" y="20"/></a:quadBezTo>"#,
        r#"<a:cubicBezTo><a:pt x="1" y="1"/><a:pt x="2" y="2"/><a:pt x="3" y="3"/></a:cubicBezTo>"#,
        r#"<a:cubicBezTo><a:pt x="1" y="1"/></a:cubicBezTo><a:lnTo/><a:arcTo wR="1"/><a:close/></a:path>"#,
        r#"<a:path fill="none" stroke="0"><a:moveTo><a:pt x="5" y="5"/></a:moveTo></a:path>"#,
        r#"<a:path/><a:other/></a:pathLst></a:custGeom>"#,
    ));
    let paths = custom_geometry(&geometry, 200, 100).expect("a geometry");
    assert_eq!(paths.len(), 2);
    assert_eq!(
        points(&paths[0]),
        vec![(0, 0), (200, 0), (220, 10), (40, 20), (6, 3)]
    );
    assert_eq!(paths[0].commands.last(), Some(&PathCommand::Close));
    assert_eq!((paths[1].filled, paths[1].stroked), (false, false));
    assert_eq!(points(&paths[1]), vec![(5, 5)]);
    assert_eq!(custom_geometry(&element("<a:custGeom/>"), 10, 10), None);
}
