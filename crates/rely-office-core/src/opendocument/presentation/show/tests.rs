// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use super::*;
use crate::xml_tree::parse_tree;

const NAMESPACES: &str = concat!(
    r#"xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" "#,
    r#"xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" "#,
    r#"xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" "#,
    r#"xmlns:presentation="urn:oasis:names:tc:opendocument:xmlns:presentation:1.0" "#,
    r#"xmlns:smil="urn:oasis:names:tc:opendocument:xmlns:smil-compatible:1.0" "#,
    r#"xmlns:anim="urn:oasis:names:tc:opendocument:xmlns:animation:1.0""#
);

fn element(inner: &str) -> Element {
    parse_tree(
        &format!("<draw:page {NAMESPACES}>{inner}</draw:page>"),
        |_, _| false,
    )
    .unwrap()
}

fn transition_of(attributes: &str) -> Option<Transition> {
    let page = element(&format!("<style:drawing-page-properties {attributes}/>"));
    let properties: Vec<&Element> = page.elements().collect();
    read_transition(&properties)
}

#[test]
fn turns_smil_transitions_into_their_effects() {
    let push = transition_of(
        r#"smil:type="pushWipe" smil:subtype="fromLeft" presentation:transition-speed="slow""#,
    )
    .unwrap();
    assert_eq!(
        (
            push.effect.as_str(),
            push.direction.as_deref(),
            push.duration_ms
        ),
        ("push", Some("r"), Some(1_000))
    );
    let pull =
        transition_of(r#"smil:type="slideWipe" smil:subtype="fromTop" smil:direction="reverse""#)
            .unwrap();
    assert_eq!(
        (pull.effect.as_str(), pull.direction.as_deref()),
        ("pull", Some("d"))
    );
    let automatic = transition_of(
        r#"smil:type="fade" smil:dur="1.5s" presentation:transition-type="automatic" presentation:duration="PT00H00M05S""#,
    )
    .unwrap();
    assert_eq!(automatic.effect, "fade");
    assert_eq!(automatic.duration_ms, Some(1_500));
    assert!(!automatic.advance_on_click);
    assert_eq!(automatic.advance_after_ms, Some(5_000));
    assert_eq!(transition_of("").map(|transition| transition.effect), None);
    assert_eq!(
        transition_of(r#"smil:type="somethingNew""#).unwrap().effect,
        "fade"
    );
}

#[test]
fn reads_the_effects_of_the_animation_tree() {
    let page = element(concat!(
        r#"<anim:par presentation:node-type="timing-root"><anim:seq presentation:node-type="main-sequence"><anim:par smil:begin="indefinite"><anim:par smil:begin="0s">"#,
        r#"<anim:par smil:begin="0.5s" presentation:node-type="on-click" presentation:preset-class="entrance" presentation:preset-id="ooo-entrance-fly-in" presentation:preset-sub-type="from-left">"#,
        r#"<anim:set smil:dur="0.001s" smil:targetElement="id2" smil:attributeName="visibility" smil:to="visible"/>"#,
        r#"<anim:animate smil:dur="0.8s" smil:targetElement="id2" smil:attributeName="x"/></anim:par>"#,
        r#"<anim:par presentation:node-type="with-previous" presentation:preset-class="motion-path" presentation:preset-id="ooo-motionpath-4-point-star">"#,
        r#"<anim:animateMotion smil:dur="2s" smil:targetElement="id3" svg:path="M 0,0 L 0.1,0.2 E"/></anim:par>"#,
        r#"<anim:par presentation:node-type="after-previous" presentation:preset-class="exit" presentation:preset-id="ooo-exit-dissolve">"#,
        r#"<anim:transitionFilter smil:dur="1s" smil:targetElement="id9"/></anim:par>"#,
        "</anim:par></anim:par></anim:seq></anim:par>"
    ));
    let ids = [(4, "id2".to_owned()), (5, "id3".to_owned())];
    let animations = read_animations(&page, &ids);
    assert_eq!(
        animations.len(),
        2,
        "an effect on a shape not read is left out"
    );
    assert_eq!(
        animations[0],
        Animation {
            shape: 4,
            class: AnimationClass::Entrance,
            preset: 2,
            preset_subtype: 8,
            trigger: AnimationTrigger::OnClick,
            delay_ms: 500,
            duration_ms: 800,
            motion_path: None,
        }
    );
    assert_eq!(animations[1].shape, 5);
    assert_eq!(animations[1].class, AnimationClass::Path);
    assert_eq!(animations[1].trigger, AnimationTrigger::WithPrevious);
    assert_eq!(
        animations[1].motion_path.as_deref(),
        Some("M 0 0 L 0.1 0.2 E")
    );
}

#[test]
fn reads_times_in_seconds_and_iso_durations() {
    assert_eq!(seconds("0.25s"), Some(250));
    assert_eq!(seconds("2"), Some(2_000));
    assert_eq!(seconds("PT01M30S"), Some(90_000));
    assert_eq!(seconds("PT2H"), Some(MAX_MILLISECONDS));
    assert_eq!(seconds("-1s"), None);
    assert_eq!(seconds("soon"), None);
}
