// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use super::*;

fn info(width: i32, height: i32, bit_count: u16, compression: u32, colours: &[[u8; 4]]) -> Vec<u8> {
    let mut info = 40_u32.to_le_bytes().to_vec();
    info.extend_from_slice(&width.to_le_bytes());
    info.extend_from_slice(&height.to_le_bytes());
    info.extend_from_slice(&1_u16.to_le_bytes());
    info.extend_from_slice(&bit_count.to_le_bytes());
    info.extend_from_slice(&compression.to_le_bytes());
    info.extend_from_slice(&[0; 12]);
    info.extend_from_slice(&u32::try_from(colours.len()).unwrap().to_le_bytes());
    info.extend_from_slice(&[0; 4]);
    for colour in colours {
        info.extend_from_slice(colour);
    }
    info
}

fn pixels(decoded: Option<Decoded>) -> (usize, usize, Vec<u8>) {
    match decoded {
        Some(Decoded::Pixels {
            width,
            height,
            rgba,
        }) => (width, height, rgba),
        _ => panic!("pixels"),
    }
}

#[test]
fn decodes_palette_and_direct_colour_bitmaps_bottom_up_or_top_down() {
    let palette = [[0, 0, 255, 0], [0, 255, 0, 0]];
    let one = pixels(decode_dib(
        &info(2, 2, 1, 0, &palette),
        &[0b0100_0000, 0, 0, 0, 0b1000_0000, 0, 0, 0],
    ));
    assert_eq!(
        one,
        (
            2,
            2,
            vec![
                0, 255, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255
            ]
        )
    );
    let four = pixels(decode_dib(&info(2, -1, 4, 0, &palette), &[0x10, 0, 0, 0]));
    assert_eq!(four.2, vec![0, 255, 0, 255, 255, 0, 0, 255]);
    let eight = pixels(decode_dib(&info(1, 1, 8, 0, &palette), &[1, 0, 0, 0]));
    assert_eq!(eight.2, vec![0, 255, 0, 255]);
    let sixteen = pixels(decode_dib(&info(1, 1, 16, 0, &[]), &[0x00, 0x7C, 0, 0]));
    assert_eq!(sixteen.2, vec![255, 0, 0, 255]);
    let mut masked_info = info(1, 1, 16, 3, &[]);
    masked_info.truncate(40);
    for mask in [0xF800_u32, 0x07E0, 0x001F] {
        masked_info.extend_from_slice(&mask.to_le_bytes());
    }
    assert_eq!(
        pixels(decode_dib(&masked_info, &[0xE0, 0x07, 0, 0])).2,
        vec![0, 255, 0, 255]
    );
    let twenty_four = pixels(decode_dib(&info(1, 1, 24, 0, &[]), &[1, 2, 3, 0]));
    assert_eq!(twenty_four.2, vec![3, 2, 1, 255]);
    let opaque = pixels(decode_dib(&info(1, 1, 32, 0, &[]), &[1, 2, 3, 0]));
    assert_eq!(opaque.2, vec![3, 2, 1, 255], "no alpha anywhere is opaque");
    let translucent = pixels(decode_dib(
        &info(2, 1, 32, 0, &[]),
        &[1, 2, 3, 0, 4, 5, 6, 128],
    ));
    assert_eq!(translucent.2, vec![3, 2, 1, 0, 6, 5, 4, 128]);
    let mut core = 12_u32.to_le_bytes().to_vec();
    core.extend_from_slice(&[1, 0, 1, 0, 1, 0, 8, 0]);
    core.extend_from_slice(&[0, 0, 255, 0, 255, 0]);
    assert_eq!(
        pixels(decode_dib(&core, &[1, 0, 0, 0])).2,
        vec![0, 255, 0, 255]
    );
}

#[test]
fn expands_run_length_bitmaps() {
    let palette = [[0, 0, 0, 0], [255, 255, 255, 0], [0, 0, 255, 0]];
    // Two of index 1, an absolute run of three, the end of the line, a delta, the end.
    let rle8 = [2, 1, 0, 3, 2, 1, 2, 0, 0, 0, 0, 2, 1, 0, 1, 2, 0, 1];
    let decoded = pixels(decode_dib(&info(5, 2, 8, 1, &palette), &rle8));
    let row = |index: usize| decoded.2[index * 4..index * 4 + 3].to_vec();
    assert_eq!(
        row(5),
        vec![255, 255, 255],
        "the first stored row is the bottom one"
    );
    assert_eq!(row(7), vec![255, 0, 0]);
    assert_eq!(row(0), vec![0, 0, 0], "skipped by the delta");
    assert_eq!(row(1), vec![255, 0, 0]);
    let rle4 = [3, 0x12, 0, 3, 0x21, 0x20, 0, 1];
    let decoded = pixels(decode_dib(&info(6, 1, 4, 2, &palette), &rle4));
    let colours: Vec<u8> = decoded.2.chunks(4).map(|pixel| pixel[0]).collect();
    assert_eq!(colours, vec![255, 255, 255, 255, 255, 255]);
    assert_eq!(expand_rle(&[2], 1, 1, false), vec![0, 0, 0, 0]);
    assert_eq!(
        expand_rle(&[0, 2], 1, 1, false),
        vec![0, 0, 0, 0],
        "a delta cut short"
    );
    assert_eq!(
        expand_rle(&[0, 5, 1], 1, 1, false),
        vec![0, 0, 0, 0],
        "a run cut short"
    );
}

#[test]
fn passes_jpeg_and_png_through_and_refuses_what_it_cannot_read() {
    let jpeg = decode_dib(&info(1, 1, 0, 4, &[]), b"jpeg")
        .and_then(Decoded::into_picture)
        .unwrap();
    assert_eq!(
        (jpeg.media_type, jpeg.bytes.as_slice()),
        ("image/jpeg", b"jpeg".as_slice())
    );
    let png = dib_picture(&info(1, 1, 0, 5, &[]), b"png").unwrap();
    assert_eq!(png.media_type, "image/png");
    assert!(
        decode_dib(&info(1, 1, 24, 6, &[]), &[0; 4]).is_none(),
        "an unknown compression"
    );
    assert!(
        decode_dib(&info(0, 1, 24, 0, &[]), &[]).is_none(),
        "no pixels"
    );
    assert!(
        decode_dib(&info(100, 100, 24, 0, &[]), &[0; 4]).is_none(),
        "bits cut short"
    );
    assert!(
        decode_dib(&info(1, 1, 7, 0, &[]), &[0; 4]).is_none(),
        "a depth no DIB has"
    );
    assert!(
        decode_dib(&[20, 0, 0, 0], &[]).is_none(),
        "a header of no known size"
    );
    assert!(
        decode_dib(&info(1_000_000, 1_000_000, 24, 0, &[]), &[]).is_none(),
        "too large"
    );
}

#[test]
fn reads_a_packed_dib_and_masks_an_image() {
    let mut packed = info(1, 1, 8, 0, &[[0, 0, 255, 0]]);
    packed.extend_from_slice(&[0, 0, 0, 0]);
    let picture = packed_dib_picture(&packed).unwrap();
    assert_eq!(picture.media_type, "image/png");
    let mut core = 12_u32.to_le_bytes().to_vec();
    core.extend_from_slice(&[1, 0, 1, 0, 1, 0, 24, 0, 1, 2, 3, 0]);
    assert!(packed_dib_picture(&core).is_some());
    assert!(packed_dib_picture(&[1]).is_none());
    let image = vec![10, 20, 30, 255, 40, 50, 60, 255];
    let mask = vec![255, 255, 255, 255];
    let combined = masked((2, 1, &image), (1, 1, &mask)).unwrap();
    assert!(combined.bytes.starts_with(b"\x89PNG"));
    assert!(masked((2, 1, &image), (0, 1, &mask)).is_none());
    assert!(masked((3, 1, &image), (1, 1, &mask)).is_none());
}

#[test]
fn writes_a_valid_png_and_base64() {
    let png = rgba_png(1, 1, &[255, 0, 0, 255]).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(&png[12..16], b"IHDR");
    assert_eq!(png.len(), 8 + 25 + (12 + 2 + 5 + 5 + 4) + 12);
    assert_eq!(crc32(b"IEND"), 0xAE42_6082);
    assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    let large = rgba_png(200, 100, &vec![7; 80_000]).unwrap();
    assert!(large.len() > 80_000, "stored blocks, one per 65,535 bytes");
    assert!(rgba_png(2, 2, &[0; 4]).is_none());
    assert!(rgba_png(0, 0, &[]).is_none(), "an image with no pixels");
    assert_eq!(crate::base64::encode(b""), "");
    assert_eq!(crate::base64::encode(b"f"), "Zg==");
    assert_eq!(crate::base64::encode(b"fo"), "Zm8=");
    assert_eq!(crate::base64::encode(b"foo"), "Zm9v");
    assert_eq!(crate::base64::encode(&[0xFB, 0xFF]), "+/8=");
    assert_eq!(channel(0x1F, 0x1F), 255);
    assert_eq!(channel(5, 0), 0);
}
