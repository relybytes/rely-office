// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::test_support::ppt_record as record;

fn edit(previous: u32, directory: u32, document: u32) -> Vec<u8> {
    let mut body = vec![0_u8; 28];
    body[8..12].copy_from_slice(&previous.to_le_bytes());
    body[12..16].copy_from_slice(&directory.to_le_bytes());
    body[16..20].copy_from_slice(&document.to_le_bytes());
    record(0, 0, USER_EDIT, &body)
}

fn directory(entries: &[(u32, &[u32])]) -> Vec<u8> {
    let mut body = Vec::new();
    for (first, offsets) in entries {
        let count = u32::try_from(offsets.len()).unwrap();
        body.extend_from_slice(&(first | (count << 20)).to_le_bytes());
        for offset in *offsets {
            body.extend_from_slice(&offset.to_le_bytes());
        }
    }
    record(0, 0, PERSIST_DIRECTORY, &body)
}

fn current_user(token: u32, edit: u32) -> Vec<u8> {
    let mut body = vec![0_u8; 20];
    body[4..8].copy_from_slice(&token.to_le_bytes());
    body[8..12].copy_from_slice(&edit.to_le_bytes());
    record(0, 0, 0x0FF6, &body)
}

#[test]
fn takes_the_latest_offset_of_every_object_through_the_edits() {
    let mut stream = vec![0_u8; 100];
    let first_directory = u32::try_from(stream.len()).unwrap();
    stream.extend(directory(&[(1, &[10, 20]), (5, &[50])]));
    let first_edit = u32::try_from(stream.len()).unwrap();
    stream.extend(edit(0, first_directory, 1));
    let second_directory = u32::try_from(stream.len()).unwrap();
    stream.extend(directory(&[(2, &[99])]));
    let second_edit = u32::try_from(stream.len()).unwrap();
    stream.extend(edit(first_edit, second_directory, 2));
    let persist = read_persist(&current_user(0xE391_C05F, second_edit), &stream).unwrap();
    assert_eq!(persist.document, 2, "the latest edit names the document");
    assert!(!persist.encrypted);
    assert_eq!(persist.offsets.get(&1), Some(&10));
    assert_eq!(persist.offsets.get(&2), Some(&99), "a later edit wins");
    assert_eq!(persist.offsets.get(&5), Some(&50));
    assert_eq!(persist.object(&stream, 1), stream.get(10..));
    assert_eq!(persist.object(&stream, 9), None);
}

#[test]
fn stops_at_an_edit_that_loops_or_is_missing() {
    let mut stream = Vec::new();
    stream.extend(directory(&[(1, &[4])]));
    let edit_offset = u32::try_from(stream.len()).unwrap();
    stream.extend(edit(edit_offset, 0, 1));
    let persist = read_persist(&current_user(0xF3D1_C4DF, edit_offset), &stream).unwrap();
    assert!(persist.encrypted);
    assert_eq!(persist.offsets.get(&1), Some(&4));
    assert_eq!(read_persist(&current_user(0, 9_999), &stream), None);
    assert_eq!(read_persist(&[], &stream), None);
    let mut cut = directory(&[(1, &[4, 5])]);
    cut.truncate(cut.len() - 2);
    let mut offsets = HashMap::new();
    read_directory(&cut[8..], &mut offsets);
    assert_eq!(offsets.len(), 1, "an entry cut short keeps what it holds");
}
