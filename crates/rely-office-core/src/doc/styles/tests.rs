// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;
use crate::doc::test_doc::{definition, sheet};

#[test]
fn reads_each_kind_of_style() {
    let bytes = sheet(
        &[
            Some(definition(
                0,
                1,
                NO_STYLE,
                "Normal",
                &[&[0, 0, 0x03, 0x24, 0x01], &[0x35, 0x08, 0x01]],
            )),
            Some(definition(1, 1, 0, "heading 1", &[&[0, 0], &[]])),
            None,
            Some(definition(
                65,
                2,
                NO_STYLE,
                "Enfasi",
                &[&[0x36, 0x08, 0x01]],
            )),
            Some(definition(
                105,
                3,
                NO_STYLE,
                "Tabella",
                &[&[1], &[0, 0, 7], &[8]],
            )),
            Some(definition(107, 4, NO_STYLE, "Elenco", &[&[0, 0, 9]])),
        ],
        4,
    );
    let sheet = StyleSheet::read(&bytes);
    assert_eq!(sheet.default_font, Some(4));
    assert_eq!(sheet.styles.len(), 6);
    let normal = sheet.style(0).unwrap();
    assert_eq!(
        (
            normal.name.as_str(),
            normal.built_in,
            normal.kind,
            normal.base
        ),
        ("Normal", 0, StyleKind::Paragraph, None)
    );
    assert_eq!(normal.paragraph, vec![0x03, 0x24, 0x01]);
    assert_eq!(normal.character, vec![0x35, 0x08, 0x01]);
    assert_eq!(sheet.style(1).unwrap().base, Some(0));
    assert_eq!(sheet.style(2), None);
    let emphasis = sheet.style(3).unwrap();
    assert_eq!(
        (emphasis.kind, emphasis.character.clone()),
        (StyleKind::Character, vec![0x36, 0x08, 0x01])
    );
    let table = sheet.style(4).unwrap();
    assert_eq!(
        (table.kind, table.paragraph.clone(), table.character.clone()),
        (StyleKind::Table, vec![7], vec![8])
    );
    let list = sheet.style(5).unwrap();
    assert_eq!(
        (list.kind, list.paragraph.clone()),
        (StyleKind::List, vec![9])
    );
    assert_eq!(sheet.style(99), None);
}

#[test]
fn chains_a_style_to_its_bases_the_farthest_first() {
    let bytes = sheet(
        &[
            Some(definition(0, 1, NO_STYLE, "Normal", &[&[0, 0], &[]])),
            Some(definition(1, 1, 0, "heading 1", &[&[0, 0], &[]])),
            Some(definition(2, 1, 1, "heading 2", &[&[0, 0], &[]])),
            Some(definition(3, 1, 3, "loop", &[&[0, 0], &[]])),
            Some(definition(4, 1, 9, "dangling", &[&[0, 0], &[]])),
        ],
        0,
    );
    let sheet = StyleSheet::read(&bytes);
    let names = |index: usize| {
        sheet
            .chain(index)
            .iter()
            .map(|style| style.name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(2), vec!["Normal", "heading 1", "heading 2"]);
    assert_eq!(names(3), vec!["loop"], "a style that is its own base");
    assert_eq!(names(4), vec!["dangling"]);
    assert_eq!(names(9), Vec::<String>::new());
}

#[test]
fn stops_at_what_is_cut_short() {
    assert_eq!(StyleSheet::read(&[]), StyleSheet::default());
    let mut bytes = sheet(
        &[Some(definition(0, 1, NO_STYLE, "Normal", &[&[0, 0], &[]]))],
        0,
    );
    bytes.truncate(bytes.len() - 3);
    let sheet = StyleSheet::read(&bytes);
    assert_eq!(
        sheet.styles,
        vec![None],
        "a definition past the end is an empty slot"
    );
    assert_eq!(read_style(&[1, 0], 10), None);
    assert_eq!(
        read_style(&definition(0, 1, NO_STYLE, "Lungo", &[]), 10).map(|style| style.paragraph),
        Some(Vec::new())
    );
    let mut cut_name = definition(0, 1, NO_STYLE, "Normal", &[]);
    cut_name.truncate(14);
    assert_eq!(read_style(&cut_name, 10), None);
}
