// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use super::*;

fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn xst(text: &str) -> Vec<u8> {
    let mut bytes = u16::try_from(text.encode_utf16().count())
        .unwrap()
        .to_le_bytes()
        .to_vec();
    bytes.extend(utf16(text));
    bytes
}

fn plex_of(positions: &[u32], elements: &[&[u8]]) -> Vec<u8> {
    let mut bytes: Vec<u8> = positions
        .iter()
        .flat_map(|position| position.to_le_bytes())
        .collect();
    for element in elements {
        bytes.extend_from_slice(element);
    }
    bytes
}

fn record(initials: &str, owner: u16, tag: i32) -> Vec<u8> {
    let mut record = vec![0_u8; ATRD_SIZE];
    let units: Vec<u16> = initials.encode_utf16().collect();
    record[0..2].copy_from_slice(&u16::try_from(units.len()).unwrap().to_le_bytes());
    for (index, unit) in units.iter().enumerate() {
        record[2 + index * 2..4 + index * 2].copy_from_slice(&unit.to_le_bytes());
    }
    record[20..22].copy_from_slice(&owner.to_le_bytes());
    record[26..30].copy_from_slice(&tag.to_le_bytes());
    record
}

#[test]
fn reads_comments_their_owners_and_ranges() {
    let references = plex_of(&[5, 20, 21], &[&record("AB", 1, 42), &record("", 0, -1)]);
    let texts = plex_of(&[0, 8, 15, 16], &[]);
    let owners = [xst("Anna"), xst("Bruno")].concat();
    let mut names = vec![0xFF, 0xFF, 1, 0, 10, 0, 0, 0, 0, 0];
    names.extend_from_slice(&42_i32.to_le_bytes());
    names.extend_from_slice(&[0; 4]);
    let firsts = plex_of(&[2, 100], &[&[0, 0, 0, 0]]);
    let lasts = plex_of(&[5, 100], &[]);
    let comments = read_comments(
        &CommentTables {
            references: &references,
            texts: &texts,
            owners: &owners,
            bookmark_names: &names,
            bookmark_firsts: &firsts,
            bookmark_lasts: &lasts,
        },
        1_000,
    );
    assert_eq!(
        comments,
        vec![
            StoredComment {
                reference: 5,
                text: 1_000..1_008,
                author: "Bruno".to_owned(),
                initials: Some("AB".to_owned()),
                covers: Some(2..5),
            },
            StoredComment {
                reference: 20,
                text: 1_008..1_015,
                author: "Anna".to_owned(),
                initials: None,
                covers: None,
            },
        ]
    );
}

#[test]
fn reads_nothing_from_damaged_tables() {
    let empty = CommentTables {
        references: &[1, 2, 3],
        texts: &[],
        owners: &[],
        bookmark_names: &[],
        bookmark_firsts: &[],
        bookmark_lasts: &[],
    };
    assert_eq!(read_comments(&empty, 0), Vec::new());
    assert_eq!(
        strings(&[5, 0, b'A', 0]),
        Vec::<String>::new(),
        "a string cut short"
    );
    assert_eq!(bookmark_tags(&[]), None);
}

#[test]
fn reads_reviewer_names() {
    let mut table = vec![0xFF, 0xFF, 2, 0, 0, 0];
    table.extend(xst("Unknown"));
    table.extend(xst("Bruno"));
    assert_eq!(reviewers(&table), vec!["Unknown", "Bruno"]);
    assert!(reviewers(&[2, 0]).is_empty(), "not an extended table");
}

#[test]
fn reads_dates_and_refuses_impossible_ones() {
    assert_eq!(
        date_of(0x47EA_DA5E).as_deref(),
        Some("2026-10-27T09:30:00Z")
    );
    assert_eq!(date_of(0), None);
    assert_eq!(date_of(0x1F | 1 << 11 | 13 << 16), None, "month 13");
    assert_eq!(date_of(0x3C | 1 << 11 | 1 << 16), None, "minute 60");
}
