// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;

fn record(kind: u32, body: &[u8]) -> Vec<u8> {
    let mut record = kind.to_le_bytes().to_vec();
    let size = u32::try_from(8 + body.len().div_ceil(4) * 4).unwrap();
    record.extend_from_slice(&size.to_le_bytes());
    record.extend_from_slice(body);
    record.resize(usize::try_from(size).unwrap(), 0);
    record
}

fn ints(values: &[i32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn floats(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// An EMF of 100 by 50 device pixels at 4 pixels a millimetre, its records and an end.
fn emf(records: &[Vec<u8>]) -> Vec<u8> {
    let mut header = ints(&[0, 0, 99, 49, 0, 0, 2_500, 1_250]);
    header.extend_from_slice(&SIGNATURE.to_le_bytes());
    header.extend_from_slice(&ints(&[0x0001_0000, 0, 0]));
    header.extend_from_slice(&[0; 16]);
    header.extend_from_slice(&ints(&[400, 200, 100, 50]));
    let mut bytes = record(1, &header);
    for item in records {
        bytes.extend_from_slice(item);
    }
    bytes.extend_from_slice(&record(14, &ints(&[0, 0, 0])));
    bytes
}

fn svg(records: &[Vec<u8>]) -> String {
    emf_svg(&emf(records)).expect("an SVG")
}

#[test]
fn sizes_the_picture_from_its_frame_and_bounds() {
    let drawn = svg(&[record(43, &ints(&[10, 10, 20, 20]))]);
    assert!(drawn.starts_with(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="70.87pt" height="35.43pt" viewBox="0 0 100 50""#
    ));
    assert!(drawn.contains(r##"<path d="M10 10H20V20H10Z" fill="#FFFFFF" fill-rule="evenodd" stroke="#000000" stroke-width="1""##));
    assert_eq!(emf_svg(&emf(&[])), None, "nothing drawn");
    assert_eq!(emf_svg(b"nope"), None);
    let mut no_bounds = emf(&[record(43, &ints(&[0, 0, 4, 4]))]);
    no_bounds[8..24].copy_from_slice(&ints(&[0, 0, -1, -1]));
    assert!(
        emf_svg(&no_bounds)
            .unwrap()
            .contains(r#"viewBox="0 0 100 50""#),
        "from the frame"
    );
}

#[test]
fn draws_with_the_pens_brushes_and_modes_selected() {
    let drawn = svg(&[
        record(38, &ints(&[1, 1, 3, 0, 0x0000_00FF])),
        record(39, &ints(&[2, 0, 0x0000_FF00, 0])),
        record(37, &ints(&[1])),
        record(37, &ints(&[2])),
        record(19, &ints(&[2])),
        record(86, &{
            let mut body = ints(&[0, 0, 0, 0, 3]);
            body.extend(
                [1_i16, 2, 3, 4, 5, 6]
                    .iter()
                    .flat_map(|value| value.to_le_bytes()),
            );
            body
        }),
        record(
            37,
            &ints(&[i32::from_le_bytes(0x8000_0005_u32.to_le_bytes())]),
        ),
        record(
            37,
            &ints(&[i32::from_le_bytes(0x8000_0008_u32.to_le_bytes())]),
        ),
        record(42, &ints(&[0, 0, 10, 10])),
        record(40, &ints(&[1])),
        record(37, &ints(&[1])),
    ]);
    assert!(drawn.contains(r##"<path d="M1 2L3 4L5 6Z" fill="#00FF00" stroke="#FF0000" stroke-width="3" stroke-linejoin="round" stroke-dasharray="9 3"/>"##), "{drawn}");
    assert!(
        !drawn.contains("A5 5"),
        "a hollow brush and a null pen draw nothing"
    );
}

#[test]
fn maps_logical_coordinates_through_the_window_viewport_and_world() {
    let drawn = svg(&[
        record(17, &ints(&[8])),
        record(9, &ints(&[1_000, 1_000])),
        record(11, &ints(&[100, 50])),
        record(10, &ints(&[500, 0])),
        record(43, &ints(&[500, 0, 600, 100])),
        record(17, &ints(&[1])),
        record(10, &ints(&[0, 0])),
        record(35, &floats(&[2.0, 0.0, 0.0, 2.0, 1.0, 1.0])),
        record(36, &{
            let mut body = floats(&[1.0, 0.0, 0.0, 1.0, 3.0, 0.0]);
            body.extend_from_slice(&3_i32.to_le_bytes());
            body
        }),
        record(43, &ints(&[1, 0, 2, 1])),
        record(33, &[]),
        record(36, &{
            let mut body = floats(&[1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
            body.extend_from_slice(&1_i32.to_le_bytes());
            body
        }),
        record(34, &ints(&[-1])),
        record(43, &ints(&[1, 0, 2, 1])),
    ]);
    assert!(
        drawn.contains(r#"d="M0 0H10V5H0Z""#),
        "anisotropic: {drawn}"
    );
    let rect = r#"d="M6 1H8V3H6Z""#;
    assert_eq!(
        drawn.matches(rect).count(),
        2,
        "the world transform, restored: {drawn}"
    );
}

#[test]
fn draws_lines_curves_paths_arcs_and_text() {
    let short = |values: &[i16]| -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    };
    let mut polyline = ints(&[0, 0, 0, 0, 2]);
    polyline.extend(short(&[0, 0, 5, 5]));
    let mut bezier = ints(&[0, 0, 0, 0, 4]);
    bezier.extend(short(&[0, 0, 1, 1, 2, 2, 3, 3]));
    let mut polypolygon = ints(&[0, 0, 0, 0, 2, 6, 3, 3]);
    polypolygon.extend(short(&[0, 0, 1, 0, 1, 1, 5, 5, 6, 5, 6, 6]));
    let mut text = ints(&[0, 0, 0, 0, 1]);
    text.extend(floats(&[1.0, 1.0]));
    text.extend(ints(&[10, 20, 3, 76, 0x02, 0, 0, 40, 10, 84]));
    text.extend("Ciao".encode_utf16().flat_map(u16::to_le_bytes));
    text.extend(ints(&[4, 4, 4]));
    let font = {
        let mut body = ints(&[3, -10, 0, 900, 0, 700]);
        body.extend_from_slice(&[1, 1, 1, 0, 0, 0, 0, 0]);
        body.extend("Arial".encode_utf16().flat_map(u16::to_le_bytes));
        body.resize(4 + 92, 0);
        body
    };
    let drawn = svg(&[
        record(87, &polyline),
        record(85, &bezier),
        record(91, &polypolygon),
        record(27, &ints(&[1, 1])),
        record(54, &ints(&[9, 9])),
        record(59, &[]),
        record(43, &ints(&[0, 0, 2, 2])),
        record(61, &[]),
        record(60, &[]),
        record(62, &ints(&[0, 0, 0, 0])),
        record(47, &ints(&[0, 0, 20, 20, 20, 10, 10, 0])),
        record(57, &ints(&[2])),
        record(45, &ints(&[0, 0, 20, 20, 20, 10, 20, 10])),
        record(82, &font),
        record(37, &ints(&[3])),
        record(24, &ints(&[0xFF])),
        record(22, &ints(&[0x18 | 0x06])),
        record(84, &text),
    ]);
    for expected in [
        r#"d="M0 0L5 5""#,
        r#"d="M0 0C1 1 2 2 3 3""#,
        r#"d="M0 0L1 0L1 1ZM5 5L6 5L6 6Z""#,
        r#"d="M1 1L9 9""#,
        r##"d="M0 0H2V2H0ZZ" fill="#FFFFFF" fill-rule="evenodd"/>"##,
        r#"d="M10 10L20 10A10 10 0 0 0 10 0Z""#,
        r#"d="M20 10A10 10 0 0 1 0 10A10 10 0 0 1 20 10""#,
        r##"<text x="10" y="20" font-family="Arial" font-size="10" fill="#FF0000" text-anchor="middle" font-weight="bold" font-style="italic" text-decoration="underline line-through" textLength="12" lengthAdjust="spacingAndGlyphs" transform="rotate(-90 10 20)" xml:space="preserve">Cia</text>"##,
        r##"<path d="M0 0H40V10H0Z" fill="#FFFFFF"/>"##,
    ] {
        assert!(drawn.contains(expected), "{expected}\n{drawn}");
    }
}

#[test]
fn draws_bitmaps_clips_and_masked_pairs() {
    let dib = |rop: u32, pixels: &[u8; 4]| {
        let mut info = 40_u32.to_le_bytes().to_vec();
        info.extend(ints(&[1, 1]));
        info.extend_from_slice(&1_u16.to_le_bytes());
        info.extend_from_slice(&32_u16.to_le_bytes());
        info.extend_from_slice(&[0; 24]);
        let mut body = ints(&[0, 0, 0, 0, 1, 2, 0, 0, 1, 1]);
        let offset_info = 8 + 72;
        body.extend(ints(&[offset_info, 40, offset_info + 40, 4, 0]));
        body.extend_from_slice(&rop.to_le_bytes());
        body.extend(ints(&[10, 20]));
        body.extend(&info);
        body.extend_from_slice(pixels);
        body
    };
    let drawn = svg(&[
        record(30, &ints(&[0, 0, 50, 25])),
        record(81, &dib(0x00CC_0020, &[0, 0, 255, 0])),
        record(81, &dib(0x00EE_0086, &[255, 255, 255, 0])),
        record(81, &dib(0x0088_00C6, &[0, 255, 0, 0])),
        record(75, &{
            let mut body = ints(&[48, 5, 32, 1, 1, 16, 0, 0, 0, 0]);
            body.extend(ints(&[0, 0, 4, 4]));
            body
        }),
        record(43, &ints(&[0, 0, 1, 1])),
        record(75, &ints(&[0, 5])),
        record(76, &{
            let mut body = ints(&[0, 0, 0, 0, 0, 0, 3, 3]);
            body.extend_from_slice(&0x00FF_0062_u32.to_le_bytes());
            body.resize(4 + 96, 0);
            body
        }),
    ]);
    assert_eq!(
        drawn.matches("<image").count(),
        2,
        "the mask and its image are one picture"
    );
    assert!(drawn.contains(r#"<image x="1" y="2" width="10" height="20" preserveAspectRatio="none" href="data:image/png;base64,"#));
    assert!(drawn.contains(
        r#"<clipPath id="clip1"><path d="M0 0H50V25H0Z"/></clipPath><g clip-path="url(#clip1)">"#
    ));
    assert!(
        drawn.contains(r#"<clipPath id="clip2"><path d="M0 0H4V4H0Z"/></clipPath>"#),
        "a region replaces the clip: {drawn}"
    );
    assert!(
        drawn.contains(r##"</g><path d="M0 0H3V3H0Z" fill="#FFFFFF"/>"##),
        "an empty region is no clip"
    );
}
