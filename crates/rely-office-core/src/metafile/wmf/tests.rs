// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn words(values: &[i16]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn record(function: u16, parameters: &[u8]) -> Vec<u8> {
    let mut parameters = parameters.to_vec();
    if parameters.len() % 2 == 1 {
        parameters.push(0);
    }
    let size = u32::try_from(3 + parameters.len() / 2).unwrap();
    let mut record = size.to_le_bytes().to_vec();
    record.extend_from_slice(&function.to_le_bytes());
    record.extend_from_slice(&parameters);
    record
}

fn header() -> Vec<u8> {
    let mut header = words(&[1, 9, 0x0300]);
    header.extend_from_slice(&[0; 12]);
    header
}

/// A WMF with a placeable header of a 2 by 1 inch box at 1,000 units to the inch.
fn placeable(records: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = PLACEABLE_KEY.to_le_bytes().to_vec();
    bytes.extend(words(&[0, 0, 0, 2_000, 1_000, 1_000]));
    bytes.extend_from_slice(&[0; 6]);
    bytes.extend(header());
    for item in records {
        bytes.extend_from_slice(item);
    }
    bytes.extend(record(0, &[]));
    bytes
}

#[test]
fn draws_a_placeable_wmf_in_its_box() {
    let colour = |value: u32| value.to_le_bytes().to_vec();
    let mut pen = words(&[0, 20, 0]);
    pen.extend(colour(0x0000_00FF));
    let mut brush = words(&[0]);
    brush.extend(colour(0x00FF_0000));
    brush.extend(words(&[0]));
    let mut polygon = words(&[3]);
    polygon.extend(words(&[0, 0, 100, 0, 100, 100]));
    let mut polypolygon = words(&[2, 3, 3]);
    polypolygon.extend(words(&[0, 0, 1, 0, 1, 1, 5, 5, 6, 5, 6, 6]));
    let drawn = wmf_svg(&placeable(&[
        record(0x02FA, &pen),
        record(0x02FC, &brush),
        record(0x012D, &words(&[0])),
        record(0x012D, &words(&[1])),
        record(0x0324, &polygon),
        record(0x0538, &polypolygon),
        record(0x041B, &words(&[100, 200, 0, 0])),
        record(0x061C, &words(&[20, 20, 100, 200, 0, 0])),
        record(0x0418, &words(&[100, 200, 0, 0])),
        record(0x081A, &words(&[0, 100, 50, 200, 100, 200, 0, 0])),
        record(0x0214, &words(&[10, 10])),
        record(0x0213, &words(&[20, 30])),
        record(0x01F0, &words(&[0])),
        record(0x02FA, &pen),
        record(0x061D, &{
            let mut patblt = 0x00FF_0062_u32.to_le_bytes().to_vec();
            patblt.extend(words(&[10, 10, 5, 5]));
            patblt
        }),
    ]))
    .expect("an SVG");
    assert!(drawn.starts_with(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="144pt" height="72pt" viewBox="0 0 2000 1000""#
    ));
    for expected in [
        r##"<path d="M0 0L100 0L100 100Z" fill="#0000FF" fill-rule="evenodd" stroke="#FF0000" stroke-width="20""##,
        r#"d="M0 0L1 0L1 1ZM5 5L6 5L6 6Z""#,
        r#"d="M0 0H200V100H0Z""#,
        r#"d="M10 0H190A10 10 0 0 1 200 10V90A10 10 0 0 1 190 100H10A10 10 0 0 1 0 90V10A10 10 0 0 1 10 0Z""#,
        r#"d="M0 50A100 50 0 1 0 200 50A100 50 0 1 0 0 50Z""#,
        r#"d="M10 10L30 20""#,
        r##"<path d="M5 5H15V15H5Z" fill="#FFFFFF"/>"##,
    ] {
        assert!(drawn.contains(expected), "{expected}\n{drawn}");
    }
    assert!(drawn.contains("A100 50 0 0 0"), "a pie: {drawn}");
}

#[test]
fn writes_text_and_maps_the_window_of_a_wmf_without_a_box() {
    let mut font = words(&[-20, 0, 0, 0, 700]);
    font.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0]);
    font.extend_from_slice(b"Times New Roman\0");
    font.resize(50, 0);
    let mut text_out = words(&[5]);
    text_out.extend_from_slice(b"Ciao!\0");
    text_out.extend(words(&[30, 40]));
    let mut ext = words(&[60, 70, 3, 0x02, 0, 0, 50, 10]);
    ext.extend_from_slice(b"Due\0");
    ext.extend(words(&[5, 5, 5]));
    let mut bytes = header();
    for item in [
        record(0x020B, &words(&[10, 100])),
        record(0x020C, &words(&[-200, 400])),
        record(0x02FB, &font),
        record(0x012D, &words(&[0])),
        record(0x0209, &0x0000_FF00_u32.to_le_bytes()),
        record(0x012E, &words(&[8])),
        record(0x0521, &text_out),
        record(0x0102, &words(&[2])),
        record(0x0A32, &ext),
        record(0x001E, &[]),
        record(0x0416, &words(&[110, 200, 10, 100])),
        record(0x041B, &words(&[20, 120, 10, 100])),
        record(0x0127, &words(&[-1])),
        record(0x012C, &words(&[0])),
    ] {
        bytes.extend(item);
    }
    bytes.extend(record(0, &[]));
    let drawn = wmf_svg(&bytes).expect("an SVG");
    assert!(
        drawn.contains(r#"width="300pt" height="150pt" viewBox="0 0 400 200""#),
        "{drawn}"
    );
    assert!(drawn.contains(r##"font-family="Times New Roman" font-size="20" fill="#00FF00" font-weight="bold" font-style="italic" xml:space="preserve">Ciao!</text>"##), "{drawn}");
    assert!(
        drawn.contains(r#"<text x="-60" y="-24""#),
        "the window's origin and its upward y: {drawn}"
    );
    assert!(drawn.contains(r#"textLength="15""#));
    assert!(
        drawn.contains(r#"<clipPath id="clip1"><path d="M0 -100H100V0H0Z"/></clipPath>"#),
        "{drawn}"
    );
    assert_eq!(wmf_svg(&header()), None, "nothing drawn");
    assert_eq!(wmf_svg(b"nope"), None);
}

#[test]
fn tells_a_wmf_header_from_other_bytes() {
    assert!(is_unplaceable_wmf(&header()));
    assert!(!is_unplaceable_wmf(&words(&[1, 8, 0x0300])));
    assert!(!is_unplaceable_wmf(&words(&[3, 9, 0x0300])));
    assert!(!is_unplaceable_wmf(&[1]));
}
