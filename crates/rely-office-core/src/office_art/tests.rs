// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;

pub(crate) fn record(version: u16, instance: u16, kind: u16, body: &[u8]) -> Vec<u8> {
    let mut bytes = (version | (instance << 4)).to_le_bytes().to_vec();
    bytes.extend_from_slice(&kind.to_le_bytes());
    bytes.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(body);
    bytes
}

#[test]
fn reads_each_kind_of_blip() {
    let blip_of = |kind: u16, instance: u16, identifiers: usize, tag: bool, image: &[u8]| {
        let mut body = vec![0_u8; 16 * identifiers + usize::from(tag)];
        body.extend_from_slice(image);
        blip(records(&record(0, instance, kind, &body)).next().unwrap())
    };
    assert_eq!(
        blip_of(0xF01D, 0x46A, 1, true, b"jpeg"),
        Some(b"jpeg".to_vec())
    );
    assert_eq!(
        blip_of(0xF01D, 0x46B, 2, true, b"jpeg"),
        Some(b"jpeg".to_vec())
    );
    assert_eq!(
        blip_of(0xF01E, 0x6E1, 2, true, b"png"),
        Some(b"png".to_vec())
    );
    assert_eq!(
        blip_of(0xF029, 0x6E5, 2, true, b"tiff"),
        Some(b"tiff".to_vec())
    );
    assert_eq!(
        blip_of(0xF02A, 0x6E2, 1, true, b"cmyk"),
        Some(b"cmyk".to_vec())
    );
    assert_eq!(
        blip_of(0xF01A, 0x3D4, 1, false, b"emf"),
        None,
        "a metafile is not drawn"
    );
    let mut dib = vec![0_u8; 40];
    dib[0..4].copy_from_slice(&40_u32.to_le_bytes());
    dib[14..16].copy_from_slice(&8_u16.to_le_bytes());
    let bitmap = blip_of(0xF01F, 0x7A8, 1, true, &dib).unwrap();
    assert_eq!(&bitmap[0..2], b"BM");
    assert_eq!(u32::from_le_bytes(bitmap[2..6].try_into().unwrap()), 54);
    assert_eq!(
        u32::from_le_bytes(bitmap[10..14].try_into().unwrap()),
        14 + 40 + 1_024
    );
    let mut true_colour = dib.clone();
    true_colour[14..16].copy_from_slice(&24_u16.to_le_bytes());
    assert_eq!(
        u32::from_le_bytes(bitmap_file(&true_colour)[10..14].try_into().unwrap()),
        54
    );
    let mut counted = dib;
    counted[32..36].copy_from_slice(&2_u32.to_le_bytes());
    assert_eq!(
        u32::from_le_bytes(bitmap_file(&counted)[10..14].try_into().unwrap()),
        62
    );
    assert_eq!(records(&[1, 2, 3]).count(), 0);
}

#[test]
fn walks_records_and_finds_a_child() {
    let mut bytes = record(0xF, 3, 0xF004, &record(0, 0, 0xF00A, &[1, 2]));
    bytes.extend(record(0, 1, 0xF00B, &[]));
    let all: Vec<Record<'_>> = records(&bytes).collect();
    assert_eq!(all.len(), 2);
    assert!(all[0].is_container());
    assert_eq!((all[0].instance, all[0].length()), (3, 18));
    assert!(!all[1].is_container());
    assert_eq!(
        child(all[0].body, SHAPE).map(|found| found.body),
        Some(&[1_u8, 2][..])
    );
    assert_eq!(child(all[0].body, PROPERTIES), None);
    bytes.extend_from_slice(&[0, 0, 0x0B, 0xF0, 0xFF, 0, 0, 0]);
    assert_eq!(
        records(&bytes).count(),
        2,
        "a record longer than the bytes ends the walk"
    );
}

#[test]
fn reads_store_entries_and_property_tables() {
    let mut blip_body = vec![0_u8; 17];
    blip_body.extend_from_slice(b"png");
    let blip = record(0, 0x6E0, 0xF01E, &blip_body);
    let mut embedded = vec![0_u8; 36];
    embedded.extend_from_slice(&blip);
    let mut delayed = vec![0_u8; 36];
    delayed[20..24].copy_from_slice(&u32::try_from(blip.len()).unwrap().to_le_bytes());
    delayed[28..32].copy_from_slice(&4_u32.to_le_bytes());
    let mut named = vec![0_u8; 36];
    named[33] = 2;
    named.extend_from_slice(b"xy");
    named.extend_from_slice(&blip);
    let mut store = record(2, 6, STORE_ENTRY, &embedded);
    store.extend(record(2, 6, STORE_ENTRY, &delayed));
    store.extend(record(2, 6, STORE_ENTRY, &named));
    store.extend(record(2, 6, STORE_ENTRY, &[0; 10]));
    store.extend(record(0, 0, 0xF00B, &[]));
    let mut delay = vec![0_u8; 4];
    delay.extend_from_slice(&blip);
    let images = store_images(&store, &delay);
    assert_eq!(
        images,
        vec![
            Some(b"png".to_vec()),
            Some(b"png".to_vec()),
            Some(b"png".to_vec()),
            None
        ]
    );
    assert_eq!(store_images(&store, &[])[1], None);

    let mut table = Vec::new();
    for (identifier, value) in [(0x4104_u16, 7_u32), (0x0181, 0x00FF_0000)] {
        table.extend_from_slice(&identifier.to_le_bytes());
        table.extend_from_slice(&value.to_le_bytes());
    }
    let encoded = record(3, 3, PROPERTIES, &table);
    let found: Vec<(u16, u32)> = properties(records(&encoded).next().unwrap()).collect();
    assert_eq!(
        found,
        vec![(0x0104, 7), (0x0181, 0x00FF_0000)],
        "the third is cut short"
    );
    assert_eq!(flag(1 << 20 | 1 << 4, 4), Some(true));
    assert_eq!(flag(1 << 20, 4), Some(false));
    assert_eq!(flag(1 << 4, 4), None);
}
