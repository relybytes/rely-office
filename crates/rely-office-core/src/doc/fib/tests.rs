// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use super::*;

/// A File Information Block with fourteen shorts, 22 longs and `pairs` pairs.
fn fib(flags: u16, counts: [u32; 7], pairs: &[(u32, u32)], pair_count: u16) -> Vec<u8> {
    let mut bytes = vec![0_u8; 154];
    bytes[0..2].copy_from_slice(&FIB_IDENTIFIER.to_le_bytes());
    bytes[2..4].copy_from_slice(&0x00C1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&flags.to_le_bytes());
    bytes[32..34].copy_from_slice(&14_u16.to_le_bytes());
    bytes[62..64].copy_from_slice(&22_u16.to_le_bytes());
    for (index, count) in STORY_COUNT_INDEXES.iter().zip(counts) {
        bytes[64 + index * 4..68 + index * 4].copy_from_slice(&count.to_le_bytes());
    }
    bytes[152..154].copy_from_slice(&pair_count.to_le_bytes());
    for (offset, length) in pairs {
        bytes.extend_from_slice(&offset.to_le_bytes());
        bytes.extend_from_slice(&length.to_le_bytes());
    }
    bytes
}

#[test]
fn reads_the_story_lengths_and_the_structures() {
    let mut pairs = vec![(0, 0); 40];
    pairs[pair::STYLES] = (10, 20);
    pairs[pair::PIECE_TABLE] = (100, 30);
    let bytes = fib(0x0200, [100, 10, 20, 5, 7, 3, 1], &pairs, 40);
    let fib = Fib::read(&bytes).unwrap();
    assert!(!fib.encrypted);
    assert_eq!(fib.table_stream, "/1Table");
    assert_eq!(
        fib.stories,
        Stories {
            main: 100,
            footnotes: 10,
            headers: 20,
            comments: 5,
            endnotes: 7,
            text_boxes: 3,
            header_text_boxes: 1,
        }
    );
    assert_eq!(fib.stories.total(), 146);
    assert_eq!(fib.stories.footnotes_start(), 100);
    assert_eq!(fib.stories.headers_start(), 110);
    assert_eq!(fib.stories.endnotes_start(), 135);
    assert_eq!(fib.stories.text_boxes_start(), 142);
    assert_eq!(fib.pair(pair::STYLES), Some((10, 20)));
    assert_eq!(
        fib.pair(pair::FONTS),
        None,
        "a structure of no length is absent"
    );
    assert_eq!(fib.pair(200), None);
    let table: Vec<u8> = (0..=255).collect();
    assert_eq!(fib.slice(&table, pair::STYLES), Some(&table[10..30]));
    assert_eq!(fib.slice(&table, pair::PIECE_TABLE), Some(&table[100..130]));
    assert_eq!(fib.slice(&table[..50], pair::PIECE_TABLE), None);
}

#[test]
fn reads_the_flags() {
    let pairs = vec![(0, 0); 34];
    let fib = Fib::read(&fib(0x0100, [1, 0, 0, 0, 0, 0, 0], &pairs, 34)).unwrap();
    assert!(fib.encrypted);
    assert_eq!(fib.table_stream, "/0Table");
}

#[test]
fn keeps_the_pairs_that_are_there_when_the_count_runs_past_the_stream() {
    let pairs = vec![(1, 1); 34];
    let fib = Fib::read(&fib(0, [1, 0, 0, 0, 0, 0, 0], &pairs, 60)).unwrap();
    assert_eq!(fib.pair(pair::PIECE_TABLE), Some((1, 1)));
    assert_eq!(fib.pair(pair::LISTS), None);
}

#[test]
fn refuses_what_is_not_a_word_97_block() {
    assert!(Fib::read(&[0; 64]).is_err());
    let mut old = fib(0, [1, 0, 0, 0, 0, 0, 0], &[], 0);
    old[2..4].copy_from_slice(&0x0065_u16.to_le_bytes());
    assert!(Fib::read(&old).is_err());
    let few = fib(0, [1, 0, 0, 0, 0, 0, 0], &[(0, 0); 20], 20);
    assert!(Fib::read(&few).is_err(), "no piece table pair");
    let cut = fib(0, [1, 0, 0, 0, 0, 0, 0], &[(0, 0); 10], 40);
    assert!(
        Fib::read(&cut).is_err(),
        "the piece table pair lies past the end"
    );
    let mut no_main = fib(0, [1, 0, 0, 0, 0, 0, 0], &[], 0);
    no_main[62..64].copy_from_slice(&2_u16.to_le_bytes());
    assert!(
        Fib::read(&no_main).is_err(),
        "too few longs for the main text length"
    );
    for length in [1, 11, 33, 63, 153] {
        let bytes = fib(0, [1, 0, 0, 0, 0, 0, 0], &vec![(0, 0); 34], 34);
        assert!(Fib::read(&bytes[..length]).is_err(), "{length}");
    }
}
