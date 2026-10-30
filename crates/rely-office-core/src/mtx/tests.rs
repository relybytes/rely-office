// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use super::ctf::checksum;
use super::test_mtx::{
    Glyph, Magnitudes, PACKED, Packing, compress, font_of, glyphs, lz_pack, mtx_file, parse_glyph,
    tables, to_ctf,
};
use super::*;

const REGULAR: &[u8] = include_bytes!("../../../../fonts/Carlito-Regular.ttf");
const SMALL: &[u8] = include_bytes!("../../../../fonts/Caladea-Regular.ttf");
const MAX: usize = 16 * 1024 * 1024;

/// A `VDMX` group: its two multipliers and the errors of each of its sizes.
type Group<'a> = (i16, i16, &'a [(i32, i32, i32)]);

/// Bytes that look random, the same on every run.
fn noise(count: usize, mut seed: u32) -> Vec<u8> {
    (0..count)
        .map(|_| {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (seed >> 16).to_le_bytes()[0]
        })
        .collect()
}

#[test]
fn lzcomp_gives_back_literals_copies_and_runs() {
    let mut far = noise(3_000, 7);
    let head = far[..900].to_vec();
    far.extend(head);
    let samples = [
        Vec::new(),
        b"a".to_vec(),
        b"abababababababab abcabcabc aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_vec(),
        noise(5_000, 1),
        far,
        [vec![0_u8; 700], vec![9; 3], noise(40, 3), vec![255; 1_000]].concat(),
    ];
    for packing in [
        PACKED,
        Packing {
            copies: false,
            run_length: false,
        },
        Packing {
            copies: true,
            run_length: true,
        },
    ] {
        for sample in &samples {
            let packed = lz_pack(sample, packing);
            assert_eq!(
                lzcomp::decompress(&packed, 3, MAX).as_deref(),
                Some(sample.as_slice()),
                "{} bytes",
                sample.len()
            );
        }
    }
}

#[test]
fn lzcomp_refuses_a_block_longer_than_allowed() {
    let packed = lz_pack(&noise(2_000, 5), PACKED);
    assert_eq!(lzcomp::decompress(&packed, 3, 1_999), None);
    assert_eq!(
        lzcomp::decompress(&packed[..packed.len() / 2], 3, MAX),
        None
    );
}

fn table(tables: &[([u8; 4], Vec<u8>)], tag: [u8; 4]) -> &[u8] {
    &tables.iter().find(|(name, _)| *name == tag).unwrap().1
}

/// The decompressed font is a well-formed file: tables sorted, checksums right, and the whole
/// file summing to the magic number through `head`.
fn assert_well_formed(ttf: &[u8]) {
    let count = usize::from(u16::from_be_bytes([ttf[4], ttf[5]]));
    let mut previous = [0; 4];
    for index in 0..count {
        let entry = &ttf[12 + index * 16..28 + index * 16];
        let tag: [u8; 4] = entry[..4].try_into().unwrap();
        assert!(tag > previous, "sorted by tag");
        previous = tag;
        let offset = u32::from_be_bytes(entry[8..12].try_into().unwrap()) as usize;
        let length = u32::from_be_bytes(entry[12..16].try_into().unwrap()) as usize;
        assert_eq!(offset % 4, 0);
        let mut data = ttf[offset..offset + length].to_vec();
        if &tag == b"head" {
            data[8..12].fill(0);
        }
        assert_eq!(
            u32::from_be_bytes(entry[4..8].try_into().unwrap()),
            checksum(&data)
        );
    }
    assert_eq!(checksum(ttf), 0xB1B0_AFBA);
}

/// Every table of the original is back as it was, but `head`'s checksum and `loca`'s form, and
/// every glyph draws and runs as it did.
fn assert_same_font(original: &[u8], decompressed: &[u8]) {
    assert_well_formed(decompressed);
    let (before, after) = (tables(original), tables(decompressed));
    assert_eq!(before.len(), after.len());
    for (tag, data) in &before {
        match tag {
            b"glyf" | b"loca" => {}
            b"head" => {
                let mut expected = data.clone();
                let mut got = table(&after, *b"head").to_vec();
                for head in [&mut expected, &mut got] {
                    head[8..12].fill(0);
                    head[50..52].fill(0);
                }
                assert_eq!(got, expected);
            }
            _ => assert_eq!(
                table(&after, *tag),
                data.as_slice(),
                "{}",
                String::from_utf8_lossy(tag)
            ),
        }
    }
    let (old, new) = (glyphs(&before), glyphs(&after));
    assert_eq!(old.len(), new.len());
    for (index, (old, new)) in old.iter().zip(&new).enumerate() {
        assert_eq!(parse_glyph(new), parse_glyph(old), "glyph {index}");
    }
}

#[test]
fn gives_back_carlito_glyph_for_glyph() {
    let mtx = compress(REGULAR, PACKED);
    assert!(mtx.len() < REGULAR.len() / 2, "{} bytes", mtx.len());
    let ttf = decompress_mtx(&mtx, MAX).unwrap();
    assert_same_font(REGULAR, &ttf);

    let parsed: Vec<Glyph> = glyphs(&tables(REGULAR))
        .iter()
        .map(|glyph| parse_glyph(glyph))
        .collect();
    assert!(
        parsed
            .iter()
            .any(|glyph| matches!(glyph, Glyph::Composite { .. }))
    );
    assert!(parsed.iter().any(|glyph| matches!(glyph, Glyph::Empty)));
    assert!(
        parsed
            .iter()
            .any(|glyph| matches!(glyph, Glyph::Simple { pushes, .. } if pushes.len() > 8))
    );
}

#[test]
fn gives_back_a_font_packed_every_way() {
    for packing in [
        Packing {
            copies: false,
            run_length: false,
        },
        Packing {
            copies: true,
            run_length: true,
        },
    ] {
        let ttf = decompress_mtx(&compress(SMALL, packing), MAX).unwrap();
        assert_same_font(SMALL, &ttf);
    }
}

/// An `hdmx` and a `VDMX` table in TrueType, and the same in the Compact Table Format.
fn device_tables(font: &[([u8; 4], Vec<u8>)]) -> [([u8; 4], Vec<u8>, Vec<u8>); 2] {
    let head = table(font, *b"head");
    let em = i64::from(u16::from_be_bytes([head[18], head[19]]));
    let glyphs = usize::from(u16::from_be_bytes([
        table(font, *b"maxp")[4],
        table(font, *b"maxp")[5],
    ]));
    let metrics = usize::from(u16::from_be_bytes([
        table(font, *b"hhea")[34],
        table(font, *b"hhea")[35],
    ]));
    let hmtx = table(font, *b"hmtx");
    let advance = |glyph: usize| {
        i64::from(u16::from_be_bytes([
            hmtx[glyph.min(metrics - 1) * 4],
            hmtx[glyph.min(metrics - 1) * 4 + 1],
        ]))
    };

    let record = (2 + glyphs).next_multiple_of(4);
    let mut hdmx = vec![0, 0, 0, 2];
    hdmx.extend(u32::try_from(record).unwrap().to_be_bytes());
    let mut widths_ctf = hdmx.clone();
    let mut bits = Magnitudes::new();
    for ppem in [11_u8, 16] {
        widths_ctf.extend([ppem, 40]);
        let start = hdmx.len();
        hdmx.extend([ppem, 40]);
        for glyph in 0..glyphs {
            let predicted = ((64 * i64::from(ppem) * advance(glyph) + em / 2) / em + 32) / 64;
            let error = match glyph % 7 {
                0 => 1,
                3 if predicted > 2 => -2,
                _ => 0,
            };
            hdmx.push(u8::try_from(predicted + error).unwrap());
            bits.write(i32::try_from(error).unwrap());
        }
        hdmx.resize(start + record, 0);
    }
    widths_ctf.extend(bits.bytes);

    let groups: [Group<'_>; 2] = [
        (1_900, 480, &[(0, 0, 0), (0, 1, -1), (0, 0, 0), (0, -1, 0)]),
        (2_100, 600, &[(12, 0, 1), (2, 3, 0), (0, 0, -2)]),
    ];
    let mut vdmx = vec![0, 1, 0, 2, 0, 2, 1, 1, 1, 1, 1, 0, 0, 0];
    let mut extents_ctf = vdmx.clone();
    let header = vdmx.len() + 4;
    let mut bodies = Vec::new();
    let mut ctf_bodies = Vec::new();
    for (y_max_multiplier, y_min_multiplier, entries) in groups {
        let mut body = u16::try_from(entries.len()).unwrap().to_be_bytes().to_vec();
        let mut ctf_body = body.clone();
        ctf_body.extend(y_max_multiplier.to_be_bytes());
        ctf_body.extend(y_min_multiplier.to_be_bytes());
        let mut bits = Magnitudes::new();
        let mut predicted = 8;
        let mut rows = Vec::new();
        for (ppem_error, y_max_error, y_min_error) in entries {
            let ppem = predicted + ppem_error;
            let y_max = (ppem * i32::from(y_max_multiplier) + 1024) / 2048 + y_max_error;
            let y_min = -((ppem * i32::from(y_min_multiplier) + 1024) / 2048) + y_min_error;
            rows.push((ppem, y_max, y_min));
            for error in [*ppem_error, *y_max_error, *y_min_error] {
                bits.write(error);
            }
            predicted = ppem + 1;
        }
        body.push(u8::try_from(rows[0].0).unwrap());
        body.push(u8::try_from(rows[rows.len() - 1].0).unwrap());
        for (ppem, y_max, y_min) in rows {
            body.extend(u16::try_from(ppem).unwrap().to_be_bytes());
            body.extend(i16::try_from(y_max).unwrap().to_be_bytes());
            body.extend(i16::try_from(y_min).unwrap().to_be_bytes());
        }
        ctf_body.extend(bits.bytes);
        bodies.push(body);
        ctf_bodies.push(ctf_body);
    }
    for (out, bodies) in [(&mut vdmx, &bodies), (&mut extents_ctf, &ctf_bodies)] {
        out.extend(u16::try_from(header).unwrap().to_be_bytes());
        out.extend(
            u16::try_from(header + bodies[0].len())
                .unwrap()
                .to_be_bytes(),
        );
        out.extend(bodies.concat());
    }
    [(*b"VDMX", vdmx, extents_ctf), (*b"hdmx", hdmx, widths_ctf)]
}

/// The first block with tables replaced, `loca` left without a place.
fn replace_tables(rest: &[u8], replacements: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut tables = tables(rest);
    for (tag, data) in &mut tables {
        if let Some((_, replacement)) = replacements.iter().find(|(name, _)| name == tag) {
            data.clone_from(replacement);
        }
    }
    let mut rest = font_of(&tables);
    for (index, (tag, _)) in tables.iter().enumerate() {
        if tag == b"loca" {
            rest[12 + index * 16 + 8..12 + index * 16 + 16].fill(0);
        }
    }
    rest
}

#[test]
fn predicts_the_device_widths_and_extents_back() {
    let mut font = tables(SMALL);
    let devices = device_tables(&font);
    for (tag, data, _) in &devices {
        font.push((*tag, data.clone()));
    }
    font.sort_by_key(|(tag, _)| *tag);
    let ttf = font_of(&font);
    let (rest, push, code) = to_ctf(&ttf);
    let compact: Vec<([u8; 4], Vec<u8>)> = devices
        .iter()
        .map(|(tag, _, ctf)| (*tag, ctf.clone()))
        .collect();
    let rest = replace_tables(&rest, &compact);
    let decompressed = decompress_mtx(&mtx_file(&rest, &push, &code, PACKED), MAX).unwrap();
    assert_same_font(&ttf, &decompressed);

    let kept: Vec<([u8; 4], Vec<u8>)> = devices
        .iter()
        .map(|(tag, data, _)| {
            let mut kept = data.clone();
            let version = 0xFFFF - u16::from_be_bytes([kept[0], kept[1]]);
            kept[..2].copy_from_slice(&version.to_be_bytes());
            (*tag, kept)
        })
        .collect();
    let rest = replace_tables(&to_ctf(&ttf).0, &kept);
    let decompressed = decompress_mtx(&mtx_file(&rest, &push, &code, PACKED), MAX).unwrap();
    assert_same_font(&ttf, &decompressed);
}

#[test]
fn leaves_out_device_tables_it_cannot_read() {
    let mut font = tables(SMALL);
    font.push((*b"hdmx", vec![0, 0, 0, 1, 0, 0, 0, 8, 12]));
    font.sort_by_key(|(tag, _)| *tag);
    let decompressed = decompress_mtx(&compress(&font_of(&font), PACKED), MAX).unwrap();
    assert_well_formed(&decompressed);
    assert!(tables(&decompressed).iter().all(|(tag, _)| tag != b"hdmx"));
}

#[test]
fn refuses_what_is_not_a_compressed_font() {
    let mtx = compress(SMALL, PACKED);
    assert_eq!(decompress_mtx(SMALL, MAX), None, "a TrueType file");
    assert_eq!(decompress_mtx(&mtx[..9], MAX), None);
    let mut version = mtx.clone();
    version[0] = 4;
    assert_eq!(decompress_mtx(&version, MAX), None);
    let mut order = mtx.clone();
    order[4..10].copy_from_slice(&[0, 0, 20, 0, 0, 12]);
    assert_eq!(decompress_mtx(&order, MAX), None);
    assert_eq!(decompress_mtx(&mtx, 10_000), None, "past the size allowed");
}

#[test]
fn survives_damaged_bytes() {
    let mtx = compress(SMALL, PACKED);
    for cut in (10..mtx.len()).step_by(997) {
        let _ = decompress_mtx(&mtx[..cut], MAX);
    }
    let positions = noise(600, 11);
    for (index, pair) in positions.as_chunks::<3>().0.iter().enumerate() {
        let mut damaged = mtx.clone();
        let at = (usize::from(pair[0]) << 8 | usize::from(pair[1])) * mtx.len() / 65_536;
        damaged[at.max(10)] ^= pair[2] | 1;
        if index % 5 == 0 {
            damaged[(at * 7 + 3) % mtx.len()] = pair[1];
        }
        let _ = decompress_mtx(&damaged, MAX);
    }
}
