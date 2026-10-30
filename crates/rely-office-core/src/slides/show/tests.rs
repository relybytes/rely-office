// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::slides::{Frame, ShapeContent, ShapeGeometry};
use crate::xml_tree::parse_tree;

const NAMESPACES: &str = concat!(
    r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
    r#"xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" "#,
    r#"xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" "#,
    r#"xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main""#
);

fn slide(inner: &str) -> Element {
    parse_tree(&format!("<p:sld {NAMESPACES}>{inner}</p:sld>"), |_, _| {
        false
    })
    .unwrap()
}

fn shape(id: u32) -> SlideShape {
    SlideShape {
        id: Some(id),
        inherited: false,
        frame: Frame::default(),
        rotation: 0.0,
        geometry: ShapeGeometry::Rectangle,
        fill: None,
        line: None,
        content: ShapeContent::None,
    }
}

#[test]
fn reads_a_transition_and_its_fallback() {
    let plain = slide(
        r#"<p:transition spd="slow" advClick="0" advTm="3000"><p:push dir="u"/></p:transition>"#,
    );
    assert_eq!(
        read_transition(&plain),
        Some(Transition {
            effect: "push".to_owned(),
            direction: Some("u".to_owned()),
            duration_ms: Some(1_000),
            advance_on_click: false,
            advance_after_ms: Some(3_000),
        })
    );
    let newer = slide(concat!(
        r#"<mc:AlternateContent><mc:Choice Requires="p14"><p:transition spd="med" p14:dur="1600"><p14:vortex dir="r"/></p:transition></mc:Choice>"#,
        r#"<mc:Fallback><p:transition spd="med"><p:blinds orient="vert"/></p:transition></mc:Fallback></mc:AlternateContent>"#
    ));
    let transition = read_transition(&newer).unwrap();
    assert_eq!(
        transition.effect, "blinds",
        "the fallback names a known effect"
    );
    assert_eq!(transition.direction.as_deref(), Some("vert"));
    assert_eq!(
        transition.duration_ms,
        Some(1_600),
        "p14:dur wins over the speed"
    );
    let unknown = slide(r#"<p:transition><p:wipe dir="sideways"/></p:transition>"#);
    assert_eq!(
        read_transition(&unknown).unwrap().direction,
        None,
        "a way wipe cannot take"
    );
    assert_eq!(read_transition(&slide("")), None);
}

#[test]
fn writes_a_transition_with_its_speed_and_way() {
    let xml = transition_xml(&Transition {
        effect: "split".to_owned(),
        direction: Some("horz".to_owned()),
        duration_ms: Some(700),
        advance_on_click: false,
        advance_after_ms: Some(2_000),
    });
    assert_eq!(
        xml,
        r#"<p:transition spd="med" advClick="0" advTm="2000"><p:split orient="horz"/></p:transition>"#
    );
    let written = transition_xml(&Transition {
        effect: "<script>".to_owned(),
        direction: Some("\"/><x".to_owned()),
        duration_ms: None,
        advance_on_click: true,
        advance_after_ms: None,
    });
    assert_eq!(
        written, r#"<p:transition spd="fast"><p:fade/></p:transition>"#,
        "an unknown effect and way never reach the file"
    );
}

#[test]
fn reads_the_effects_of_the_timing_tree() {
    let xml = slide(concat!(
        r#"<p:timing><p:tnLst><p:par><p:cTn id="1" nodeType="tmRoot"><p:childTnLst><p:seq><p:cTn id="2" nodeType="mainSeq"><p:childTnLst>"#,
        r#"<p:par><p:cTn id="3"><p:childTnLst><p:par><p:cTn id="4"><p:childTnLst>"#,
        r#"<p:par><p:cTn id="5" presetID="10" presetClass="entr" presetSubtype="0" nodeType="clickEffect"><p:stCondLst><p:cond delay="250"/></p:stCondLst>"#,
        r#"<p:childTnLst><p:animEffect filter="fade"><p:cBhvr><p:cTn id="6" dur="800"/><p:tgtEl><p:spTgt spid="7"/></p:tgtEl></p:cBhvr></p:animEffect></p:childTnLst></p:cTn></p:par>"#,
        r#"<p:par><p:cTn id="8" presetID="1" presetClass="path" nodeType="afterEffect"><p:childTnLst><p:animMotion path="M 0 0 L 0.25 0 E"><p:cBhvr><p:cTn id="9" dur="2000"/><p:tgtEl><p:spTgt spid="9"/></p:tgtEl></p:cBhvr></p:animMotion></p:childTnLst></p:cTn></p:par>"#,
        r#"<p:par><p:cTn id="10" presetID="1" presetClass="entr" nodeType="withEffect"><p:childTnLst><p:set><p:cBhvr><p:cTn id="11" dur="1"/><p:tgtEl><p:spTgt spid="99"/></p:tgtEl></p:cBhvr></p:set></p:childTnLst></p:cTn></p:par>"#,
        r#"</p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn></p:seq></p:childTnLst></p:cTn></p:par></p:tnLst></p:timing>"#
    ));
    let shapes = [shape(7), shape(9)];
    let animations = read_animations(&xml, &shapes);
    assert_eq!(
        animations.len(),
        2,
        "an effect on a shape that is not there is left out"
    );
    assert_eq!(
        animations[0],
        Animation {
            shape: 0,
            class: AnimationClass::Entrance,
            preset: 10,
            preset_subtype: 0,
            trigger: AnimationTrigger::OnClick,
            delay_ms: 250,
            duration_ms: 800,
            motion_path: None,
        }
    );
    assert_eq!(animations[1].shape, 1);
    assert_eq!(animations[1].trigger, AnimationTrigger::AfterPrevious);
    assert_eq!(
        animations[1].motion_path.as_deref(),
        Some("M 0 0 L 0.25 0 E")
    );
}

#[test]
fn writes_effects_that_read_back_the_same() {
    let animation = |shape, class, preset, trigger| Animation {
        shape,
        class,
        preset,
        preset_subtype: 4,
        trigger,
        delay_ms: 100,
        duration_ms: 600,
        motion_path: None,
    };
    let animations = vec![
        animation(0, AnimationClass::Entrance, 2, AnimationTrigger::OnClick),
        animation(
            1,
            AnimationClass::Emphasis,
            8,
            AnimationTrigger::WithPrevious,
        ),
        animation(0, AnimationClass::Exit, 10, AnimationTrigger::AfterPrevious),
        Animation {
            motion_path: Some("M 0 0 L 0 0.5 E".to_owned()),
            ..animation(1, AnimationClass::Path, 0, AnimationTrigger::OnClick)
        },
        animation(0, AnimationClass::Entrance, 37, AnimationTrigger::OnClick),
        animation(5, AnimationClass::Entrance, 1, AnimationTrigger::OnClick),
    ];
    let timing = timing_xml(&animations, &[Some(4), Some(6)]);
    let read = read_animations(&slide(&timing), &[shape(4), shape(6)]);
    assert_eq!(
        read.len(),
        5,
        "an effect on a shape not written is left out"
    );
    assert_eq!(read[..4], animations[..4]);
    assert_eq!(
        (read[4].class, read[4].preset),
        (AnimationClass::Entrance, 10),
        "a preset this writer cannot play becomes a fade"
    );
    let mut ids: Vec<&str> = timing
        .match_indices(r#"<p:cTn id=""#)
        .map(|(start, _)| {
            let rest = &timing[start + 11..];
            &rest[..rest.find('"').unwrap()]
        })
        .collect();
    let count = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), count, "every time node has an id of its own");
    assert_eq!(timing_xml(&animations, &[None, None]), "");
}
