// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! How an odp page plays in a show: the transition its drawing-page style names in SMIL terms,
//! turned into the `PresentationML` effect that looks the same, and the effects of the
//! animation tree on its shapes, each named by the preset `LibreOffice` gives it.

use crate::slides::{Animation, AnimationClass, AnimationTrigger, Transition};
use crate::xml_tree::{Element, Namespace};

const SMIL: Namespace = Namespace::OdfSmil;
const ANIM: Namespace = Namespace::OdfAnimation;
const PRESENTATION: Namespace = Namespace::OdfPresentation;

const MAX_ANIMATIONS: usize = 500;
const MAX_MILLISECONDS: u32 = 3_600_000;

/// The transition of a page from the attributes of its drawing-page style, nearest first.
pub(super) fn read_transition(properties: &[&Element]) -> Option<Transition> {
    let attribute = |namespace: Namespace, name: &str| {
        properties
            .iter()
            .find_map(|element| element.attribute(namespace, name))
    };
    let kind = attribute(SMIL, "type");
    let advance = attribute(PRESENTATION, "transition-type");
    if kind.is_none() && advance.is_none() {
        return None;
    }
    let (effect, direction) = smil_effect(
        kind.unwrap_or("fade"),
        attribute(SMIL, "subtype").unwrap_or(""),
        attribute(SMIL, "direction") == Some("reverse"),
    );
    let duration_ms = attribute(SMIL, "dur").and_then(seconds).or(
        match attribute(PRESENTATION, "transition-speed") {
            Some("slow") => Some(1_000),
            Some("medium") => Some(750),
            Some("fast") => Some(500),
            _ => None,
        },
    );
    let automatic = matches!(advance, Some("automatic" | "semi-automatic"));
    Some(Transition {
        effect: effect.to_owned(),
        direction,
        duration_ms,
        advance_on_click: advance != Some("automatic"),
        advance_after_ms: automatic
            .then(|| attribute(PRESENTATION, "duration").and_then(seconds))
            .flatten(),
    })
}

/// The `PresentationML` effect and way that look like a SMIL transition.
fn smil_effect(kind: &str, subtype: &str, reverse: bool) -> (&'static str, Option<String>) {
    let side = |from: &str| -> Option<String> {
        Some(
            match from {
                "fromBottom" | "bottom" => "u",
                "fromTop" | "top" => "d",
                "fromLeft" | "left" => "r",
                "fromRight" | "right" => "l",
                "fromTopLeft" => "rd",
                "fromTopRight" => "ld",
                "fromBottomLeft" => "ru",
                "fromBottomRight" => "lu",
                _ => return None,
            }
            .to_owned(),
        )
    };
    match kind {
        "pushWipe" => ("push", side(subtype)),
        "slideWipe" if reverse => ("pull", side(subtype)),
        "slideWipe" => ("cover", side(subtype)),
        "barWipe" => (
            "wipe",
            match (subtype, reverse) {
                ("topToBottom", false) => Some("d".to_owned()),
                ("topToBottom", true) => Some("u".to_owned()),
                ("leftToRight", true) => Some("l".to_owned()),
                _ => Some("r".to_owned()),
            },
        ),
        "barnDoorWipe" => (
            "split",
            Some(
                if subtype == "horizontal" {
                    "horz"
                } else {
                    "vert"
                }
                .to_owned(),
            ),
        ),
        "blindsWipe" => (
            "blinds",
            Some(
                if subtype == "horizontal" {
                    "horz"
                } else {
                    "vert"
                }
                .to_owned(),
            ),
        ),
        "checkerBoardWipe" => ("checker", None),
        "randomBarWipe" => (
            "randomBar",
            Some(
                if subtype == "horizontal" {
                    "horz"
                } else {
                    "vert"
                }
                .to_owned(),
            ),
        ),
        "irisWipe" if subtype == "diamond" => ("diamond", None),
        "irisWipe" => ("zoom", Some(if reverse { "out" } else { "in" }.to_owned())),
        "ellipseWipe" => ("circle", None),
        "fanWipe" => ("wedge", None),
        "pinWheelWipe" => ("wheel", None),
        "waterfallWipe" => ("strips", None),
        "dissolve" => ("dissolve", None),
        "random" => ("random", None),
        "zoom" => ("zoom", None),
        "fourBoxWipe" => ("plus", None),
        _ => ("fade", None),
    }
}

/// A time of `0.5s`, `2s` or the ISO 8601 `PT00H00M05S`, in milliseconds.
fn seconds(value: &str) -> Option<u32> {
    let value = value.trim();
    let total = if let Some(rest) = value.strip_prefix("PT") {
        let mut total = 0.0;
        let mut number = String::new();
        for character in rest.chars() {
            match character {
                'H' => total += number.parse::<f64>().ok()? * 3_600.0,
                'M' => total += number.parse::<f64>().ok()? * 60.0,
                'S' => total += number.parse::<f64>().ok()?,
                _ => {
                    number.push(character);
                    continue;
                }
            }
            number.clear();
        }
        total
    } else {
        value
            .strip_suffix('s')
            .unwrap_or(value)
            .parse::<f64>()
            .ok()?
    };
    if !total.is_finite() || total < 0.0 {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to an hour first"
    )]
    let milliseconds = (total * 1_000.0).round().min(f64::from(MAX_MILLISECONDS)) as u32;
    Some(milliseconds)
}

/// The effects of a page's animation tree, each on the shape whose `xml:id` it targets, found
/// among `ids` as the index of the shape it was read into.
pub(super) fn read_animations(page: &Element, ids: &[(usize, String)]) -> Vec<Animation> {
    let mut animations = Vec::new();
    for root in page.elements().filter(|child| child.is(ANIM, "par")) {
        collect(root, ids, &mut animations, 0);
    }
    animations
}

fn collect(
    element: &Element,
    ids: &[(usize, String)],
    animations: &mut Vec<Animation>,
    depth: usize,
) {
    if depth > 64 || animations.len() >= MAX_ANIMATIONS {
        return;
    }
    for child in element.elements().filter(|child| child.namespace == ANIM) {
        if let Some(class) = child.attribute(PRESENTATION, "preset-class") {
            if let Some(animation) = effect(child, class, ids) {
                animations.push(animation);
            }
            continue;
        }
        collect(child, ids, animations, depth + 1);
    }
}

fn effect(node: &Element, class: &str, ids: &[(usize, String)]) -> Option<Animation> {
    let class = match class {
        "entrance" => AnimationClass::Entrance,
        "emphasis" => AnimationClass::Emphasis,
        "exit" => AnimationClass::Exit,
        "motion-path" => AnimationClass::Path,
        _ => return None,
    };
    let target = first_target(node, 0)?;
    let shape = ids
        .iter()
        .find(|(_, id)| id == target)
        .map(|(index, _)| *index)?;
    let trigger = match node.attribute(PRESENTATION, "node-type") {
        Some("with-previous") => AnimationTrigger::WithPrevious,
        Some("after-previous") => AnimationTrigger::AfterPrevious,
        _ => AnimationTrigger::OnClick,
    };
    let preset_name = node.attribute(PRESENTATION, "preset-id").unwrap_or("");
    let preset = preset_number(class, preset_name);
    let motion_path = node
        .elements()
        .find(|child| child.is(ANIM, "animateMotion"))
        .and_then(|motion| motion.attribute(Namespace::OdfSvg, "path"))
        .map(|path| path.trim().to_owned())
        .filter(|path| {
            path.len() <= 4_096
                && path
                    .chars()
                    .all(|character| "MLCZEmlcze0123456789 .,-+".contains(character))
        })
        .map(|path| path.replace(',', " "));
    Some(Animation {
        shape,
        class,
        preset,
        preset_subtype: node
            .attribute(PRESENTATION, "preset-sub-type")
            .map_or(0, preset_subtype),
        trigger,
        delay_ms: node.attribute(SMIL, "begin").and_then(seconds).unwrap_or(0),
        duration_ms: node
            .elements()
            .filter_map(|behaviour| behaviour.attribute(SMIL, "dur").and_then(seconds))
            .max()
            .unwrap_or(500),
        motion_path,
    })
}

fn first_target(element: &Element, depth: usize) -> Option<&str> {
    if depth > 8 {
        return None;
    }
    element.attribute(SMIL, "targetElement").or_else(|| {
        element
            .elements()
            .find_map(|child| first_target(child, depth + 1))
    })
}

/// The number `PowerPoint` gives the preset `LibreOffice` names; `0` for one it has no
/// counterpart for, which is written back as a fade.
fn preset_number(class: AnimationClass, name: &str) -> u32 {
    let name = name
        .trim_start_matches("ooo-entrance-")
        .trim_start_matches("ooo-exit-")
        .trim_start_matches("ooo-emphasis-");
    match (class, name) {
        (AnimationClass::Entrance, "appear") | (AnimationClass::Exit, "disappear") => 1,
        (AnimationClass::Entrance, "fly-in") | (AnimationClass::Exit, "fly-out") => 2,
        (AnimationClass::Entrance, "fade-in") | (AnimationClass::Exit, "fade-out") => 10,
        (AnimationClass::Entrance | AnimationClass::Exit, "wipe") => 22,
        (AnimationClass::Entrance | AnimationClass::Exit, "zoom") => 53,
        (AnimationClass::Emphasis, "spin") => 8,
        (AnimationClass::Emphasis, "grow-and-shrink") => 6,
        _ => 0,
    }
}

/// The side a fly in comes from, as `PowerPoint` numbers it.
fn preset_subtype(name: &str) -> u32 {
    match name {
        "from-top" => 1,
        "from-right" => 2,
        "from-left" => 8,
        "from-bottom" => 4,
        _ => 0,
    }
}

#[cfg(test)]
mod tests;
