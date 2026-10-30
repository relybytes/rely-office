// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::doc::test_doc::plex as plex_of;

fn descriptor(offset: u32) -> Vec<u8> {
    let mut descriptor = vec![0_u8; DESCRIPTOR_SIZE];
    descriptor[2..6].copy_from_slice(&offset.to_le_bytes());
    descriptor
}

#[test]
fn reads_each_section_with_its_page() {
    let modifiers = [
        0x09, 0x30, 0x00, // continuous
        0x0A, 0x30, 0x01, // a first page of its own
        0x0B, 0x50, 0x01, 0x00, // two columns
        0x0C, 0x90, 0x68, 0x01, // 360 between them
        0x17, 0xB0, 0xD0, 0x02, // header at 720
        0x18, 0xB0, 0x40, 0x01, // footer at 320
        0x1F, 0xB0, 0xE0, 0x3D, // 15840 wide
        0x20, 0xB0, 0xE0, 0x2E, // 12000 high
        0x21, 0xB0, 0xA0, 0x05, // left 1440
        0x22, 0xB0, 0x40, 0x06, // right 1600
        0x23, 0x90, 0x60, 0xFA, // top -1440, a minimum
        0x24, 0x90, 0x60, 0x09, // bottom 2400
        0x1D, 0x30, 0x02, // the orientation, which the size already says
    ];
    let mut word_document = vec![0_u8; 100];
    word_document.extend_from_slice(&u16::try_from(modifiers.len()).unwrap().to_le_bytes());
    word_document.extend_from_slice(&modifiers);
    let plex_bytes = plex_of(&[0, 50, 80], &[&descriptor(100), &descriptor(0xFFFF_FFFF)]);
    let sections = read_sections(&word_document, &plex_bytes);
    assert_eq!(sections.len(), 2);
    let first = &sections[0];
    assert_eq!(
        (first.end, first.continuous, first.title_page),
        (50, true, true)
    );
    assert_eq!(
        first.layout,
        PageLayout {
            width_twips: 15_840,
            height_twips: 12_000,
            margins_twips: [1_440, 1_600, 2_400, 1_440],
            header_distance_twips: 720,
            footer_distance_twips: 320,
            columns: 2,
            column_gap_twips: 360,
            ..PageLayout::default()
        }
    );
    assert_eq!(sections[1].end, 80);
    assert_eq!(
        sections[1].layout,
        default_layout(),
        "a section that names nothing"
    );
    assert!(!sections[1].continuous);
}

#[test]
fn keeps_a_page_no_smaller_than_an_inch() {
    let modifiers = [0x1F, 0xB0, 0x0A, 0x00, 0x0B, 0x50, 0xFF, 0x00];
    let mut word_document = u16::try_from(modifiers.len())
        .unwrap()
        .to_le_bytes()
        .to_vec();
    word_document.extend_from_slice(&modifiers);
    let sections = read_sections(&word_document, &plex_of(&[0, 1], &[&descriptor(0)]));
    assert_eq!(sections[0].layout.width_twips, 1_440);
    assert_eq!(sections[0].layout.columns, 45);
    let cut = read_sections(&word_document[..4], &plex_of(&[0, 1], &[&descriptor(0)]));
    assert_eq!(
        cut[0].layout,
        default_layout(),
        "modifiers past the stream are left out"
    );
    assert_eq!(read_sections(&word_document, &[1, 2, 3]), Vec::new());
}
