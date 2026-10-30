// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::doc::test_doc::{LevelSpec, lists, overrides};

/// A level whose template writes digits 0 to 8 for the level numbers they stand for.
fn level(start: u32, format: u8, template: &str) -> LevelSpec {
    LevelSpec {
        start,
        format,
        template: template
            .chars()
            .map(|character| match character.to_digit(10) {
                Some(digit) if digit < 9 => u16::try_from(digit).unwrap(),
                _ => u16::try_from(u32::from(character)).unwrap(),
            })
            .collect(),
        follow: 1,
        paragraph: vec![0x5E, 0x84, 0xD0, 0x02],
        character: vec![0x35, 0x08, 0x01],
    }
}

fn level_of(format: u8) -> Level {
    Level {
        format,
        ..Level::default()
    }
}

#[test]
fn reads_lists_with_their_levels_and_overrides() {
    let mut levels: Vec<LevelSpec> = (0..9)
        .map(|index| level(1, 0, &format!("{index}.")))
        .collect();
    levels[1] = level(3, 4, "0.1)");
    let definitions = lists(&[
        (7, false, levels),
        (9, true, vec![level(1, 23, "\u{f0b7}")]),
    ]);
    let entries = overrides(&[
        (7, vec![(1, Some(5), None)]),
        (9, vec![(0, None, Some(level(2, 0, "0")))]),
        (99, Vec::new()),
    ]);
    let mut table = vec![0_u8; 4];
    table.extend_from_slice(&definitions);
    let override_offset = table.len();
    table.extend_from_slice(&entries);
    let lists = Lists::read(&table, Some(4), Some(override_offset));

    let (id, levels) = lists.list(1).unwrap();
    assert_eq!((id, levels.len()), (7, 9));
    assert_eq!(levels[1].start, 3);
    assert_eq!(levels[1].format_name(), "lowerLetter");
    assert_eq!(
        levels[1].template,
        vec![0, u16::from(b'.'), 1, u16::from(b')')]
    );
    assert_eq!(levels[1].follow, 1);
    assert_eq!(levels[1].paragraph, vec![0x5E, 0x84, 0xD0, 0x02]);
    assert_eq!(levels[1].character, vec![0x35, 0x08, 0x01]);
    assert!(levels[1].ordered());
    assert_eq!(lists.start_override(1, 1), Some(5));
    assert_eq!(lists.start_override(1, 0), None);

    let (id, levels) = lists.list(2).unwrap();
    assert_eq!((id, levels.len()), (9, 1), "a simple list has one level");
    assert!(!levels[0].ordered());
    assert_eq!(levels[0].format_name(), "bullet");
    assert_eq!(
        lists.start_override(2, 0),
        Some(2),
        "a redefined level starts where it says"
    );

    assert_eq!(
        lists.list(3),
        None,
        "an override of a list that does not exist"
    );
    assert_eq!(lists.list(0), None);
    assert_eq!(lists.list(9), None);
    assert_eq!(lists.start_override(0, 0), None);
}

#[test]
fn names_every_number_format() {
    let names: Vec<&str> = [0, 1, 2, 3, 4, 5, 22, 23, 255, 9]
        .into_iter()
        .map(|format| level_of(format).format_name())
        .collect();
    assert_eq!(
        names,
        vec![
            "decimal",
            "upperRoman",
            "lowerRoman",
            "upperLetter",
            "lowerLetter",
            "ordinal",
            "decimalZero",
            "bullet",
            "none",
            "decimal"
        ]
    );
    assert!(!level_of(255).ordered());
}

#[test]
fn reads_what_is_there_when_the_lists_are_cut_short() {
    let definitions = lists(&[(7, false, vec![level(1, 0, "0.")])]);
    let read = Lists::read(&definitions, Some(0), None);
    assert_eq!(read.definitions[&7].len(), 1, "eight levels are missing");
    let mut huge_start = level(1, 0, "0");
    huge_start.start = 0x8000_0000;
    let definitions = lists(&[(1, true, vec![huge_start])]);
    assert_eq!(
        Lists::read(&definitions, Some(0), None).definitions[&1][0].start,
        1
    );
    assert_eq!(Lists::read(&[], Some(0), Some(0)), Lists::default());
    let entries = overrides(&[(7, vec![(0, Some(4), Some(level(1, 0, "0")))])]);
    let cut = Lists::read(&entries[..entries.len() - 6], None, Some(0));
    assert_eq!(cut.overrides.len(), 1);
    assert_eq!(cut.overrides[0].1.get(&0), Some(&4));
    assert_eq!(read_level(&[0; 10], 0), None);
}
