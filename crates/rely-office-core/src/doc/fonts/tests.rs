// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use super::*;
use crate::doc::test_doc::{embedded_font_table, eot, font};
use crate::mtx::test_mtx::{PACKED, compress};

#[test]
fn reads_each_font_name_in_order() {
    let mut table = vec![3, 0, 0, 0];
    table.extend(font("Times New Roman"));
    table.extend(font("Città"));
    table.extend(font(""));
    assert_eq!(font_names(&table), vec!["Times New Roman", "Città", ""]);
}

#[test]
fn stops_at_a_font_cut_short() {
    let mut table = vec![2, 0, 0, 0];
    table.extend(font("Arial"));
    table.push(200);
    assert_eq!(font_names(&table), vec!["Arial"]);
    let mut short = vec![2, 0, 0, 0];
    short.extend(font("Arial"));
    assert_eq!(
        font_names(&short),
        vec!["Arial"],
        "the second font is missing"
    );
    assert_eq!(font_names(&[1]), Vec::<String>::new());
}

const REGULAR: &[u8] = include_bytes!("../../../../../fonts/Carlito-Regular.ttf");

#[test]
fn reads_embedded_faces_plain_and_hidden_by_xor() {
    let plain = eot(REGULAR, "Carlito", 0);
    let mut stream = vec![0_u8; 16];
    stream.extend_from_slice(&plain);
    let hidden_at = stream.len();
    stream.extend_from_slice(&eot(REGULAR, "Carlito", 0x1000_0000));
    let table = embedded_font_table(&[(16, 1, false, false), (hidden_at, 1, true, true)]);
    let faces = embedded_faces(&table, &stream);
    assert_eq!(faces.len(), 2);
    assert_eq!(faces[0].font, 1);
    assert_eq!(faces[0].style, FaceStyle::Regular);
    assert_eq!(faces[0].bytes.as_deref(), Some(REGULAR));
    assert_eq!(faces[1].style, FaceStyle::BoldItalic);
    assert_eq!(faces[1].bytes.as_deref(), Some(REGULAR));
}

#[test]
fn reads_a_face_compressed_with_micro_type_express() {
    let compressed = compress(REGULAR, PACKED);
    let mut stream = vec![0_u8; 8];
    stream.extend_from_slice(&eot(&compressed, "Carlito", 0x0000_0004));
    let hidden_at = stream.len();
    stream.extend_from_slice(&eot(&compressed, "Carlito", 0x1000_0004));
    let table = embedded_font_table(&[(8, 0, true, false), (hidden_at, 0, false, false)]);
    let faces = embedded_faces(&table, &stream);
    let expected = decompress_mtx(&compressed, 1 << 24).unwrap();
    assert!(crate::is_font_file(&expected));
    assert_eq!(faces[0].style, FaceStyle::Bold);
    assert_eq!(faces[0].bytes.as_deref(), Some(expected.as_slice()));
    assert_eq!(faces[1].bytes.as_deref(), Some(expected.as_slice()));
}

#[test]
fn leaves_out_a_compressed_or_broken_face() {
    let mut stream = vec![0_u8; 4];
    stream.extend_from_slice(&eot(REGULAR, "Carlito", 0x0000_0004));
    let broken_at = stream.len();
    let mut broken = eot(REGULAR, "Carlito", 0);
    broken[34] = 0;
    stream.extend_from_slice(&broken);
    let short_at = stream.len();
    stream.extend_from_slice(&eot(REGULAR, "Carlito", 0)[..200]);
    let table = embedded_font_table(&[
        (4, 0, false, false),
        (broken_at, 0, false, false),
        (short_at, 0, false, false),
        (0, 0, false, false),
    ]);
    let faces = embedded_faces(&table, &stream);
    assert_eq!(faces.len(), 4);
    assert!(faces.iter().all(|face| face.bytes.is_none()));
}

#[test]
fn reads_no_more_faces_than_the_table_has() {
    let mut table = embedded_font_table(&[(4, 0, false, false)]);
    table[2] = 9;
    assert_eq!(embedded_faces(&table, &[]).len(), 1);
    assert_eq!(embedded_faces(&[], &[]), Vec::new());
    assert_eq!(
        embedded_faces(&[0, 0, 0xFF, 0xFF, 64, 0, 0, 0, 10, 0], &[]),
        Vec::new()
    );
}
