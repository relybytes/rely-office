// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;
use crate::doc::test_doc::{character_page, paragraph_page, plex as plex_of};

#[test]
fn reads_a_plex_of_positions_and_elements() {
    let bytes = plex_of(&[0, 5, 9], &[&[1, 2], &[3, 4]]);
    let (positions, elements) = plex(&bytes, 2).unwrap();
    assert_eq!(positions, vec![0, 5, 9]);
    assert_eq!(elements, vec![&[1_u8, 2][..], &[3, 4][..]]);
    assert_eq!(plex(&bytes[..9], 2), None);
    assert_eq!(plex(&[1, 2], 2), None);
    assert_eq!(super::positions(&plex_of(&[3, 7], &[])), vec![3, 7]);
}

#[test]
fn reads_the_character_runs_of_each_page() {
    let mut word_document = vec![0_u8; PAGE_SIZE * 3];
    let first = character_page(&[(1024, 1030, vec![0x35, 0x08, 0x01]), (1030, 1040, vec![])]);
    let second = character_page(&[(1040, 1050, vec![0x36, 0x08, 0x01])]);
    word_document[PAGE_SIZE * 2..].copy_from_slice(&first);
    word_document[PAGE_SIZE..PAGE_SIZE * 2].copy_from_slice(&second);
    let plex_bytes = plex_of(
        &[1024, 1040, 1050],
        &[&2_u32.to_le_bytes(), &1_u32.to_le_bytes()],
    );
    let runs = character_runs(&word_document, &plex_bytes);
    assert_eq!(
        runs,
        vec![
            CharacterRun {
                start: 1024,
                end: 1030,
                grpprl: vec![0x35, 0x08, 0x01]
            },
            CharacterRun {
                start: 1030,
                end: 1040,
                grpprl: Vec::new()
            },
            CharacterRun {
                start: 1040,
                end: 1050,
                grpprl: vec![0x36, 0x08, 0x01]
            },
        ]
    );
    assert_eq!(
        run_at(&runs, 1035, |run| (run.start, run.end)).map(|run| run.start),
        Some(1030)
    );
    assert_eq!(run_at(&runs, 1050, |run| (run.start, run.end)), None);
    assert_eq!(run_at(&runs, 10, |run| (run.start, run.end)), None);
    let missing_page = plex_of(&[0, 10], &[&9_u32.to_le_bytes()]);
    assert_eq!(character_runs(&word_document, &missing_page), Vec::new());
    assert_eq!(character_runs(&word_document, &[1, 2, 3]), Vec::new());
}

#[test]
fn reads_the_paragraph_runs_with_their_styles() {
    let mut word_document = vec![0_u8; PAGE_SIZE * 2];
    let page = paragraph_page(&[
        (1024, 1030, 3, vec![0x03, 0x24, 0x01]),
        (1030, 1040, 0, vec![0x07, 0x24]),
    ]);
    word_document[PAGE_SIZE..].copy_from_slice(&page);
    let plex_bytes = plex_of(&[1024, 1040], &[&1_u32.to_le_bytes()]);
    let runs = paragraph_runs(&word_document, &plex_bytes, &[]);
    assert_eq!(
        runs,
        vec![
            ParagraphRun {
                start: 1024,
                end: 1030,
                style: 3,
                grpprl: vec![0x03, 0x24, 0x01]
            },
            ParagraphRun {
                start: 1030,
                end: 1040,
                style: 0,
                grpprl: vec![0x07, 0x24]
            },
        ]
    );
}

#[test]
fn reads_paragraph_properties_moved_to_the_data_stream() {
    let mut word_document = vec![0_u8; PAGE_SIZE * 2];
    let page = paragraph_page(&[(1024, 1030, 0, vec![0x46, 0x66, 4, 0, 0, 0])]);
    word_document[PAGE_SIZE..].copy_from_slice(&page);
    let plex_bytes = plex_of(&[1024, 1030], &[&1_u32.to_le_bytes()]);
    let data = [9, 9, 9, 9, 3, 0, 0x16, 0x24, 0x01];
    let runs = paragraph_runs(&word_document, &plex_bytes, &data);
    assert_eq!(runs[0].grpprl, vec![0x16, 0x24, 0x01]);
    let runs = paragraph_runs(&word_document, &plex_bytes, &data[..6]);
    assert_eq!(
        runs[0].grpprl,
        vec![0x46, 0x66, 4, 0, 0, 0],
        "kept when the data is cut short"
    );
}

#[test]
fn reads_a_run_without_properties_and_one_cut_short() {
    let mut page = vec![0_u8; PAGE_SIZE];
    page[0..4].copy_from_slice(&0_u32.to_le_bytes());
    page[4..8].copy_from_slice(&10_u32.to_le_bytes());
    page[8] = 0;
    page[PAGE_SIZE - 1] = 1;
    assert_eq!(paragraph_properties(&page, 0x1F0), (0, Vec::new()));
    let mut word_document = page.clone();
    word_document.extend_from_slice(&page);
    let plex_bytes = plex_of(&[0, 10], &[&1_u32.to_le_bytes()]);
    assert_eq!(
        paragraph_runs(&word_document, &plex_bytes, &[]),
        vec![ParagraphRun {
            start: 0,
            end: 10,
            style: 0,
            grpprl: Vec::new()
        }]
    );
    page[0x1FC] = 0x20;
    assert_eq!(
        paragraph_properties(&page, 0x1FC),
        (0, Vec::new()),
        "a length past the page"
    );
}
