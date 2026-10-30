// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use crate::geometry::{FillShade, PathCommand, PathPoint, ShapePath};
use crate::slides::{Bullet, Gradient, GradientKind, GradientStop, Paint, ShapeGeometry};

fn round_trip<T>(value: &T) -> (String, T)
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let json = serde_json::to_string(value).unwrap();
    let back = serde_json::from_str(&json).unwrap();
    (json, back)
}

#[test]
fn tags_paints_by_type() {
    let solid = Paint::Solid("FF0000".to_owned());
    let (json, back) = round_trip(&solid);
    assert_eq!(json, r#"{"type":"solid","colour":"FF0000"}"#);
    assert_eq!(back, solid);
    let gradient = Paint::Gradient(Gradient {
        stops: vec![
            GradientStop {
                offset: 0.0,
                colour: "000000".to_owned(),
            },
            GradientStop {
                offset: 1.0,
                colour: "FFFFFF".to_owned(),
            },
        ],
        kind: GradientKind::Radial {
            centre: (0.5, 0.25),
        },
    });
    let (json, back) = round_trip(&gradient);
    assert!(
        json.contains(r#""kind":{"type":"radial","centre":[0.5,0.25]}"#),
        "{json}"
    );
    assert_eq!(back, gradient);
    let picture = Paint::Picture {
        media: 2,
        tile_emu: None,
    };
    let (json, back) = round_trip(&picture);
    assert_eq!(json, r#"{"type":"picture","media":2}"#);
    assert_eq!(back, picture);
}

#[test]
fn tags_geometries_bullets_and_path_steps_by_type() {
    let path = ShapeGeometry::Path(vec![ShapePath {
        commands: vec![
            PathCommand::MoveTo(PathPoint { x: 0, y: 0 }),
            PathCommand::CubicTo(
                PathPoint { x: 1, y: 2 },
                PathPoint { x: 3, y: 4 },
                PathPoint { x: 5, y: 6 },
            ),
            PathCommand::Close,
        ],
        filled: true,
        stroked: false,
        shade: FillShade::Darken,
    }]);
    let (json, back) = round_trip(&path);
    assert!(
        json.starts_with(
            r#"{"type":"path","paths":[{"commands":[{"type":"moveTo","to":{"x":0,"y":0}}"#
        ),
        "{json}"
    );
    assert!(json.contains(r#"{"type":"close"}"#) && json.contains(r#""shade":"darken""#));
    assert_eq!(back, path);
    let (json, back) = round_trip(&ShapeGeometry::Line { flipped: true });
    assert_eq!(json, r#"{"type":"line","flipped":true}"#);
    assert_eq!(back, ShapeGeometry::Line { flipped: true });
    let bullet = Bullet::Number("1.".to_owned());
    let (json, back) = round_trip(&bullet);
    assert_eq!(json, r#"{"type":"number","text":"1."}"#);
    assert_eq!(back, bullet);
}
