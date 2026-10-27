// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;

fn context() -> Context {
    Context::new((4.0, 4.0))
}

#[test]
fn maps_through_every_mapping_mode() {
    let mut context = context();
    assert_eq!(context.device(3.0, 4.0), (3.0, 4.0));
    context.state.map_mode = 7;
    context.state.window_extent = (100.0, 100.0);
    context.state.viewport_extent = (50.0, -200.0);
    assert_eq!(
        context.device(10.0, 10.0),
        (5.0, -5.0),
        "isotropic keeps the smaller unit"
    );
    context.state.map_mode = 8;
    context.state.window_extent = (0.0, 100.0);
    assert_eq!(
        context.device(10.0, 10.0),
        (10.0, -20.0),
        "no window extent is a unit"
    );
    context.state.viewport_extent = (1.0, 1.0);
    context.state.window_extent = (1.0, 1.0);
    for (mode, expected) in [
        (2, (0.4, -0.4)),
        (3, (0.04, -0.04)),
        (4, (1.016, -1.016)),
        (5, (0.1016, -0.1016)),
    ] {
        context.state.map_mode = mode;
        let (x, y) = context.device(1.0, 1.0);
        assert!(
            (x - expected.0).abs() < 1e-9 && (y - expected.1).abs() < 1e-9,
            "{mode}"
        );
    }
    context.state.map_mode = 6;
    let (x, _) = context.device(1_440.0, 0.0);
    assert!((x - 101.6).abs() < 1e-9, "a twip");
    context.state.map_mode = 1;
    context.state.world = Transform {
        m11: 0.0,
        m12: 1.0,
        m21: -1.0,
        m22: 0.0,
        dx: 0.0,
        dy: 0.0,
    };
    assert_eq!(context.device(1.0, 0.0), (0.0, 1.0), "a quarter turn");
    assert_eq!(context.scale(), (1.0, 1.0));
    let turn = context.state.world;
    let back = turn.then(&turn).then(&turn).then(&turn);
    assert!((back.m11 - 1.0).abs() < 1e-12 && back.m12.abs() < 1e-12);
}

#[test]
fn saves_restores_and_keeps_its_objects() {
    let mut context = context();
    context.state.text_colour = "FF0000".to_owned();
    context.save();
    context.state.text_colour = "00FF00".to_owned();
    context.save();
    context.state.text_colour = "0000FF".to_owned();
    context.restore(-1);
    assert_eq!(context.state.text_colour, "00FF00");
    context.save();
    context.restore(1);
    assert_eq!(
        context.state.text_colour, "FF0000",
        "the first save by its number"
    );
    context.restore(-5);
    context.restore(0);
    assert_eq!(
        context.state.text_colour, "FF0000",
        "a save that is not there"
    );

    let pen = Object::Pen(Pen {
        style: 0,
        width: 2.0,
        cosmetic: false,
        colour: "123456".to_owned(),
    });
    context.add_object(pen.clone());
    context.add_object(Object::Other);
    context.delete_object(0);
    context.add_object(Object::Brush(Brush {
        hollow: false,
        colour: "ABCDEF".to_owned(),
    }));
    context.select_object(0);
    context.select_object(1);
    context.select_object(9);
    assert_eq!(
        context.state.brush.colour, "ABCDEF",
        "the freed slot taken again"
    );
    context.set_object(5, pen);
    context.set_object(usize::MAX, Object::Other);
    context.select_object(5);
    assert_eq!(context.state.pen.colour, "123456");
    context.add_object(Object::Font(Font {
        family: "Verdana".to_owned(),
        ..State::default().font
    }));
    context.select_object(2);
    assert_eq!(context.state.font.family, "Verdana");
    for (stock, check) in [(4, "000000"), (0, "FFFFFF"), (3, "404040")] {
        context.select_stock_object(stock);
        assert_eq!(context.state.brush.colour, check);
    }
    context.select_stock_object(8);
    assert_eq!(context.state.pen.style, 5);
    context.select_stock_object(13);
    assert_eq!(context.state.font.family, "Arial");
    context.select_stock_object(99);
    assert_eq!(colour_of(0x0012_3456), "563412");
}

#[test]
fn draws_figures_rectangles_ellipses_arcs_text_and_pictures() {
    let mut context = context();
    assert_eq!(context.figure(&[], true, false), "");
    context.state.position = (1.0, 1.0);
    assert_eq!(context.figure(&[(2.0, 2.0)], false, true), "M1 1L2 2");
    assert_eq!(context.curves(&[], false), "");
    assert_eq!(
        context.curves(&[(1.0, 1.0), (2.0, 2.0), (3.0, 3.0)], true),
        "M1 1C1 1 2 2 3 3"
    );
    assert_eq!(
        context.rectangle([4.0, 4.0, 0.0, 0.0], Some((100.0, 1.0))),
        "M2 0H2A2 0.5 0 0 1 4 0.5V3.5A2 0.5 0 0 1 2 4H2A2 0.5 0 0 1 0 3.5V0.5A2 0.5 0 0 1 2 0Z",
        "the corners clamped to half the sides"
    );
    assert_eq!(context.ellipse([0.0, 0.0, 0.0, 5.0]), "");
    assert_eq!(
        context.arc([0.0, 0.0, 0.0, 5.0], (0.0, 0.0), (1.0, 1.0), ArcKind::Open),
        ""
    );
    assert_eq!(
        context.arc([0.0, 0.0, 2.0, 2.0], (2.0, 1.0), (2.0, 1.0), ArcKind::Chord),
        "M2 1A1 1 0 0 0 0 1A1 1 0 0 0 2 1Z",
        "a whole turn in two halves"
    );
    context.path = Some(String::new());
    context.draw("M0 0", true, true);
    context.clip_to_path(true);
    assert_eq!(context.state.clip, vec!["M0 0".to_owned()]);
    context.clip_to_path(false);
    context.clip_to_region(&[[0.0, 0.0, 1.0, 1.0]], false);
    context.intersect_clip([0.0, 0.0, 2.0, 2.0]);
    assert_eq!(context.state.clip.len(), 3);
    context.clip_to_region(&[], true);
    assert_eq!(context.state.clip, Vec::<String>::new());
    context.draw_path(true, true);

    context.state.font.height = 20.0;
    context.state.text_align = 8 | 2;
    context.state.opaque = true;
    context.text((0.0, 10.0), "basso", None, Some([0.0, 0.0, 10.0, 10.0]));
    context.state.font.height = 0.0;
    context.state.text_align = 0;
    context.text((0.0, 0.0), "alto", Some(30.0), None);
    let picture = Picture {
        media_type: "image/png",
        bytes: vec![1],
    };
    context.picture(&picture, [0.0, 0.0, 4.0, 4.0]);
    context.fill_rectangle([0.0, 0.0, 1.0, 1.0], "00FF00");
    let mut pen_context = Context::new((1.0, 1.0));
    pen_context.state.pen = Pen {
        style: 4,
        width: 2.0,
        cosmetic: false,
        colour: "000000".to_owned(),
    };
    let paint = pen_context.shape_paint(true, true);
    assert_eq!(paint.dashes, vec![6.0, 2.0, 2.0, 2.0, 2.0, 2.0]);
    pen_context.state.pen.style = 3;
    assert_eq!(pen_context.shape_paint(true, true).dashes.len(), 4);
    pen_context.state.pen.style = 2;
    assert_eq!(pen_context.shape_paint(true, true).dashes, vec![2.0, 2.0]);
    let svg = context
        .canvas
        .finish([0.0, 0.0, 10.0, 10.0], (10.0, 10.0))
        .unwrap();
    assert!(
        svg.contains(r#"y="6.72" font-family="Arial" font-size="16.4""#),
        "{svg}"
    );
    assert!(svg.contains(r#"text-anchor="end""#));
    assert!(svg.contains(r##"<rect x="0" y="0" width="10" height="10" fill="#FFFFFF"/>"##));
    assert!(svg.contains(r#"font-size="12""#) && svg.contains(r#"textLength="30""#));
    assert!(svg.contains("<image"));
    assert!(svg.contains(r##"fill="#00FF00""##));
}
