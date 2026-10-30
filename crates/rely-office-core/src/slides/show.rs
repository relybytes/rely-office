// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! How a slide plays in a show: its transition, and the effects of its shapes read from the
//! timing tree of a pptx slide and written back as one.

use std::fmt::Write as _;

use super::{
    Animation, AnimationClass, AnimationTrigger, SlideShape, TRANSITION_EFFECTS, Transition,
};
use crate::xml_tree::{Element, Namespace};

const P: Namespace = Namespace::Presentation;

/// No slide plays more effects than this.
const MAX_ANIMATIONS: usize = 500;
/// The longest delay or duration kept, an hour.
const MAX_MILLISECONDS: u32 = 3_600_000;
/// The longest motion path kept, in characters.
const MAX_PATH_CHARS: usize = 4_096;
/// The effects whose way is an orientation, `horz` or `vert`, rather than a direction.
const ORIENTED_EFFECTS: [&str; 4] = ["blinds", "checker", "comb", "randomBar"];

/// The transition of a slide: its own `p:transition`, or the one inside the markup
/// compatibility block `PowerPoint` 2010 writes, whose fallback names an effect of the 2007
/// schema. An effect only newer versions know is read as a fade.
pub(super) fn read_transition(slide: &Element) -> Option<Transition> {
    let mut candidates: Vec<&Element> = slide
        .elements()
        .filter(|child| child.is(P, "transition"))
        .collect();
    for block in slide
        .elements()
        .filter(|child| child.is(Namespace::MarkupCompatibility, "AlternateContent"))
    {
        for branch in ["Fallback", "Choice"] {
            if let Some(transition) = block
                .child(Namespace::MarkupCompatibility, branch)
                .and_then(|branch| branch.child(P, "transition"))
            {
                candidates.push(transition);
            }
        }
    }
    let transition = *candidates.first()?;
    let effect_element = candidates.iter().find_map(|candidate| {
        candidate.elements().find(|effect| {
            effect.namespace == P && TRANSITION_EFFECTS.contains(&effect.name.as_str())
        })
    });
    let effect = effect_element.map_or_else(|| "fade".to_owned(), |effect| effect.name.clone());
    let direction = effect_element.and_then(|effect| {
        effect
            .attribute(Namespace::None, "dir")
            .or_else(|| effect.attribute(Namespace::None, "orient"))
            .filter(|value| direction_allowed(&effect.name, value))
            .map(str::to_owned)
    });
    let attribute = |name: &str| {
        candidates
            .iter()
            .find_map(|candidate| candidate.attribute(Namespace::None, name))
    };
    let duration_ms = candidates
        .iter()
        .find_map(|candidate| {
            // `p14:dur`, the duration in milliseconds of PowerPoint 2010.
            candidate
                .attribute(Namespace::Other, "dur")
                .and_then(|value| value.parse::<u32>().ok())
        })
        .or_else(|| match attribute("spd") {
            Some("slow") => Some(1_000),
            Some("med") => Some(750),
            Some("fast") => Some(500),
            _ => None,
        })
        .map(|duration| duration.min(MAX_MILLISECONDS));
    Some(Transition {
        effect,
        direction,
        duration_ms,
        advance_on_click: transition.attribute(Namespace::None, "advClick") != Some("0"),
        advance_after_ms: attribute("advTm")
            .and_then(|time| time.parse::<u32>().ok())
            .map(|time| time.min(MAX_MILLISECONDS)),
    })
}

/// Whether `value` is a way `effect` can move in.
fn direction_allowed(effect: &str, value: &str) -> bool {
    if ORIENTED_EFFECTS.contains(&effect) {
        return matches!(value, "horz" | "vert");
    }
    match effect {
        "split" => matches!(value, "horz" | "vert" | "in" | "out"),
        "zoom" => matches!(value, "in" | "out"),
        "strips" => matches!(value, "lu" | "ru" | "ld" | "rd"),
        "cover" | "pull" => matches!(value, "l" | "r" | "u" | "d" | "lu" | "ru" | "ld" | "rd"),
        "push" | "wipe" => matches!(value, "l" | "r" | "u" | "d"),
        _ => false,
    }
}

/// The effects of a slide's `p:timing`, each on the shape whose id it targets; an effect on a
/// shape that is not among `shapes` is left out.
pub(super) fn read_animations(slide: &Element, shapes: &[SlideShape]) -> Vec<Animation> {
    let Some(timing) = slide.child(P, "timing") else {
        return Vec::new();
    };
    let mut animations = Vec::new();
    collect_effects(timing, shapes, &mut animations, 0);
    animations
}

fn collect_effects(
    element: &Element,
    shapes: &[SlideShape],
    animations: &mut Vec<Animation>,
    depth: usize,
) {
    if depth > 64 || animations.len() >= MAX_ANIMATIONS {
        return;
    }
    for child in element.elements() {
        if child.is(P, "cTn")
            && let Some(class) = child.attribute(Namespace::None, "presetClass")
        {
            if let Some(animation) = effect(child, class, shapes) {
                animations.push(animation);
            }
            continue;
        }
        collect_effects(child, shapes, animations, depth + 1);
    }
}

/// One effect of the timing tree: the time node that names its preset.
fn effect(node: &Element, class: &str, shapes: &[SlideShape]) -> Option<Animation> {
    let class = match class {
        "entr" => AnimationClass::Entrance,
        "emph" => AnimationClass::Emphasis,
        "exit" => AnimationClass::Exit,
        "path" => AnimationClass::Path,
        _ => return None,
    };
    let number = |name: &str| {
        node.attribute(Namespace::None, name)
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0)
    };
    let target = descendant(node, "spTgt")?
        .attribute(Namespace::None, "spid")?
        .parse::<u32>()
        .ok()?;
    let shape = shapes
        .iter()
        .position(|shape| !shape.inherited && shape.id == Some(target))?;
    let trigger = match node.attribute(Namespace::None, "nodeType") {
        Some("withEffect") => AnimationTrigger::WithPrevious,
        Some("afterEffect") => AnimationTrigger::AfterPrevious,
        _ => AnimationTrigger::OnClick,
    };
    let delay_ms = node
        .child(P, "stCondLst")
        .and_then(|conditions| conditions.child(P, "cond"))
        .and_then(|condition| condition.attribute(Namespace::None, "delay"))
        .and_then(|delay| delay.parse::<u32>().ok())
        .unwrap_or(0)
        .min(MAX_MILLISECONDS);
    // An effect sets the shape visible in one millisecond and moves it for the rest: its
    // duration is its longest behaviour's.
    let duration_ms = node
        .child(P, "childTnLst")
        .and_then(|children| longest_behaviour(children, 0))
        .unwrap_or(500)
        .min(MAX_MILLISECONDS);
    let motion_path = descendant(node, "animMotion")
        .and_then(|motion| motion.attribute(Namespace::None, "path"))
        .filter(|path| path_allowed(path))
        .map(str::to_owned);
    Some(Animation {
        shape,
        class,
        preset: number("presetID"),
        preset_subtype: number("presetSubtype"),
        trigger,
        delay_ms,
        duration_ms,
        motion_path,
    })
}

/// The longest `dur` of the time nodes under `element`.
fn longest_behaviour(element: &Element, depth: usize) -> Option<u32> {
    if depth > 16 {
        return None;
    }
    element
        .elements()
        .filter_map(|child| {
            let own = child
                .is(P, "cTn")
                .then(|| child.attribute(Namespace::None, "dur"))
                .flatten()
                .and_then(|duration| duration.parse::<u32>().ok());
            own.max(longest_behaviour(child, depth + 1))
        })
        .max()
}

fn descendant<'e>(element: &'e Element, name: &str) -> Option<&'e Element> {
    element.descendant(P, name)
}

/// Whether a motion path holds only the commands, numbers and spaces of the path language.
fn path_allowed(path: &str) -> bool {
    path.len() <= MAX_PATH_CHARS
        && path
            .chars()
            .all(|character| "MLCZEmlcze0123456789 .-+".contains(character))
}

/// The `p:transition` of a slide, or nothing for no transition or an effect this writer does
/// not know.
pub(super) fn transition_xml(transition: &Transition) -> String {
    let effect = if TRANSITION_EFFECTS.contains(&transition.effect.as_str()) {
        transition.effect.as_str()
    } else {
        "fade"
    };
    let speed = match transition.duration_ms {
        Some(duration) if duration >= 900 => "slow",
        Some(duration) if duration >= 600 => "med",
        _ => "fast",
    };
    let mut xml = format!(r#"<p:transition spd="{speed}""#);
    if !transition.advance_on_click {
        xml.push_str(r#" advClick="0""#);
    }
    if let Some(after) = transition.advance_after_ms {
        let _ = write!(xml, r#" advTm="{}""#, after.min(MAX_MILLISECONDS));
    }
    xml.push('>');
    let way = transition
        .direction
        .as_deref()
        .filter(|value| direction_allowed(effect, value));
    match way {
        Some(value) if ORIENTED_EFFECTS.contains(&effect) || matches!(value, "horz" | "vert") => {
            let _ = write!(xml, r#"<p:{effect} orient="{value}"/>"#);
        }
        Some(value) => {
            let _ = write!(xml, r#"<p:{effect} dir="{value}"/>"#);
        }
        None => {
            let _ = write!(xml, "<p:{effect}/>");
        }
    }
    xml.push_str("</p:transition>");
    xml
}

/// The behaviours a preset plays, by class and preset, written for the shape with `id`; `None`
/// for a preset this writer cannot write, which is then written as a fade.
fn behaviours(animation: &Animation, id: u32, node: &mut Ids) -> Option<String> {
    let duration = animation.duration_ms.max(1);
    let target = format!(r#"<p:tgtEl><p:spTgt spid="{id}"/></p:tgtEl>"#);
    let visibility = |value: &str, node: &mut Ids| {
        format!(
            r#"<p:set><p:cBhvr><p:cTn id="{}" dur="1" fill="hold"><p:stCondLst><p:cond delay="{}"/></p:stCondLst></p:cTn>{target}<p:attrNameLst><p:attrName>style.visibility</p:attrName></p:attrNameLst></p:cBhvr><p:to><p:strVal val="{value}"/></p:to></p:set>"#,
            node.next(),
            if value == "hidden" { duration - 1 } else { 0 }
        )
    };
    let property = |name: &str, from: &str, to: &str, node: &mut Ids| {
        format!(
            r#"<p:anim calcmode="lin" valueType="num"><p:cBhvr additive="base"><p:cTn id="{}" dur="{duration}" fill="hold"/>{target}<p:attrNameLst><p:attrName>{name}</p:attrName></p:attrNameLst></p:cBhvr><p:tavLst><p:tav tm="0"><p:val><p:strVal val="{from}"/></p:val></p:tav><p:tav tm="100000"><p:val><p:strVal val="{to}"/></p:val></p:tav></p:tavLst></p:anim>"#,
            node.next()
        )
    };
    let filter = |name: &str, transition: &str, node: &mut Ids| {
        format!(
            r#"<p:animEffect transition="{transition}" filter="{name}"><p:cBhvr><p:cTn id="{}" dur="{duration}"/>{target}</p:cBhvr></p:animEffect>"#,
            node.next()
        )
    };
    let fly = |entering: bool, subtype: u32, node: &mut Ids| {
        let (axis, away) = match subtype {
            8 => ("ppt_x", "0-#ppt_w/2"),
            2 => ("ppt_x", "1+#ppt_w/2"),
            1 => ("ppt_y", "0-#ppt_h/2"),
            _ => ("ppt_y", "1+#ppt_h/2"),
        };
        let place = format!("#{axis}");
        if entering {
            property(axis, away, &place, node)
        } else {
            property(axis, &place, away, node)
        }
    };
    Some(match (animation.class, animation.preset) {
        (AnimationClass::Entrance, 1) => visibility("visible", node),
        (AnimationClass::Entrance, 2) => {
            visibility("visible", node) + &fly(true, animation.preset_subtype, node)
        }
        (AnimationClass::Entrance, 10) => visibility("visible", node) + &filter("fade", "in", node),
        (AnimationClass::Entrance, 22) => {
            visibility("visible", node) + &filter("wipe(down)", "in", node)
        }
        (AnimationClass::Entrance, 53) => {
            visibility("visible", node)
                + &property("ppt_w", "0", "#ppt_w", node)
                + &property("ppt_h", "0", "#ppt_h", node)
        }
        (AnimationClass::Exit, 1) => visibility("hidden", node),
        (AnimationClass::Exit, 2) => {
            fly(false, animation.preset_subtype, node) + &visibility("hidden", node)
        }
        (AnimationClass::Exit, 10) => filter("fade", "out", node) + &visibility("hidden", node),
        (AnimationClass::Exit, 22) => {
            filter("wipe(down)", "out", node) + &visibility("hidden", node)
        }
        (AnimationClass::Emphasis, 8) => format!(
            r#"<p:animRot by="21600000"><p:cBhvr><p:cTn id="{}" dur="{duration}" fill="hold"/>{target}<p:attrNameLst><p:attrName>r</p:attrName></p:attrNameLst></p:cBhvr></p:animRot>"#,
            node.next()
        ),
        (AnimationClass::Emphasis, 6) => format!(
            r#"<p:animScale><p:cBhvr><p:cTn id="{}" dur="{duration}" fill="hold"/>{target}</p:cBhvr><p:by x="150000" y="150000"/></p:animScale>"#,
            node.next()
        ),
        (AnimationClass::Path, _) => {
            let path = animation
                .motion_path
                .as_deref()
                .filter(|path| path_allowed(path))?;
            format!(
                r#"<p:animMotion origin="layout" path="{path}" pathEditMode="relative"><p:cBhvr><p:cTn id="{}" dur="{duration}" fill="hold"/>{target}<p:attrNameLst><p:attrName>ppt_x</p:attrName><p:attrName>ppt_y</p:attrName></p:attrNameLst></p:cBhvr></p:animMotion>"#,
                node.next()
            )
        }
        _ => return None,
    })
}

/// The ids of the time nodes, counted up from the root.
struct Ids(u32);

impl Ids {
    fn next(&mut self) -> u32 {
        self.0 += 1;
        self.0
    }
}

/// The `p:timing` of a slide whose shapes were written with `ids`, one click group per effect
/// that starts on a click and the effects that follow it in the same group; empty when no
/// effect targets a written shape.
pub(super) fn timing_xml(animations: &[Animation], ids: &[Option<u32>]) -> String {
    let mut groups: Vec<Vec<(&Animation, u32)>> = Vec::new();
    for animation in animations.iter().take(MAX_ANIMATIONS) {
        let Some(Some(id)) = ids.get(animation.shape) else {
            continue;
        };
        if animation.trigger == AnimationTrigger::OnClick || groups.is_empty() {
            groups.push(Vec::new());
        }
        if let Some(group) = groups.last_mut() {
            group.push((animation, *id));
        }
    }
    if groups.is_empty() {
        return String::new();
    }
    let mut node = Ids(2);
    let mut sequence = String::new();
    for group in &groups {
        let mut steps = String::new();
        let mut offset = 0;
        let mut previous_end = 0;
        for (animation, id) in group {
            if animation.trigger == AnimationTrigger::AfterPrevious {
                offset = previous_end;
            }
            let (effect, preset) = if let Some(effect) = behaviours(animation, *id, &mut node) {
                (effect, animation.preset)
            } else {
                let fallback = Animation {
                    class: if animation.class == AnimationClass::Exit {
                        AnimationClass::Exit
                    } else {
                        AnimationClass::Entrance
                    },
                    preset: 10,
                    ..(*animation).clone()
                };
                (
                    behaviours(&fallback, *id, &mut node).unwrap_or_default(),
                    10,
                )
            };
            let class = match animation.class {
                AnimationClass::Emphasis if preset == animation.preset => "emph",
                AnimationClass::Exit => "exit",
                AnimationClass::Path if preset == animation.preset => "path",
                _ => "entr",
            };
            let node_type = match animation.trigger {
                AnimationTrigger::OnClick => "clickEffect",
                AnimationTrigger::WithPrevious => "withEffect",
                AnimationTrigger::AfterPrevious => "afterEffect",
            };
            let start = offset;
            previous_end = start + animation.delay_ms + animation.duration_ms;
            let _ = write!(
                steps,
                r#"<p:par><p:cTn id="{}" fill="hold"><p:stCondLst><p:cond delay="{start}"/></p:stCondLst><p:childTnLst><p:par><p:cTn id="{}" presetID="{preset}" presetClass="{class}" presetSubtype="{}" fill="hold" nodeType="{node_type}"><p:stCondLst><p:cond delay="{}"/></p:stCondLst><p:childTnLst>{effect}</p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn></p:par>"#,
                node.next(),
                node.next(),
                animation.preset_subtype,
                animation.delay_ms
            );
        }
        let _ = write!(
            sequence,
            r#"<p:par><p:cTn id="{}" fill="hold"><p:stCondLst><p:cond delay="indefinite"/></p:stCondLst><p:childTnLst>{steps}</p:childTnLst></p:cTn></p:par>"#,
            node.next()
        );
    }
    format!(
        concat!(
            r#"<p:timing><p:tnLst><p:par><p:cTn id="1" dur="indefinite" restart="never" nodeType="tmRoot"><p:childTnLst>"#,
            r#"<p:seq concurrent="1" nextAc="seek"><p:cTn id="2" dur="indefinite" nodeType="mainSeq"><p:childTnLst>{}</p:childTnLst></p:cTn>"#,
            r#"<p:prevCondLst><p:cond evt="onPrev" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:prevCondLst>"#,
            r#"<p:nextCondLst><p:cond evt="onNext" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:nextCondLst></p:seq>"#,
            "</p:childTnLst></p:cTn></p:par></p:tnLst></p:timing>"
        ),
        sequence
    )
}

#[cfg(test)]
mod tests;
