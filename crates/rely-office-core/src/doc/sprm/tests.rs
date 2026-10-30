// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn reads_each_operand_length() {
    let grpprl = [
        0x35, 0x08, 0x01, // a toggle, one byte
        0x43, 0x4A, 0x18, 0x00, // two bytes
        0x70, 0x68, 0x11, 0x22, 0x33, 0x00, // four bytes
        0x01, 0xE0, 0xAA, 0xBB, 0xCC, // three bytes
        0x2D, 0xC6, 0x02, 0x05, 0x06, // a length byte, then the operand
        0x08, 0xD6, 0x03, 0x00, 0x07, 0x08, // the table definition's two-byte length
    ];
    let read: Vec<(u16, Vec<u8>)> = modifiers(&grpprl)
        .map(|(code, operand)| (code, operand.to_vec()))
        .collect();
    assert_eq!(
        read,
        vec![
            (0x0835, vec![1]),
            (0x4A43, vec![0x18, 0]),
            (0x6870, vec![0x11, 0x22, 0x33, 0]),
            (0xE001, vec![0xAA, 0xBB, 0xCC]),
            (0xC62D, vec![5, 6]),
            (0xD608, vec![7, 8]),
        ]
    );
}

#[test]
fn stops_at_a_modifier_cut_short() {
    assert_eq!(modifiers(&[0x35]).count(), 0);
    assert_eq!(modifiers(&[0x43, 0x4A, 0x18]).count(), 0);
    assert_eq!(modifiers(&[0x2D, 0xC6]).count(), 0);
    assert_eq!(modifiers(&[0x2D, 0xC6, 0x09, 1]).count(), 0);
    assert_eq!(modifiers(&[0x08, 0xD6, 0x05]).count(), 0);
    assert_eq!(modifiers(&[]).count(), 0);
}

#[test]
fn reads_operands_as_numbers() {
    assert_eq!(byte(&[7, 8]), 7);
    assert_eq!(byte(&[]), 0);
    assert_eq!(word(&[0x34, 0x12, 9]), 0x1234);
    assert_eq!(word(&[1]), 0);
    assert_eq!(signed_word(&[0xFE, 0xFF]), -2);
    assert_eq!(long(&[1, 0, 0, 0x80]), 0x8000_0001);
    assert_eq!(long(&[1, 2]), 0);
}
