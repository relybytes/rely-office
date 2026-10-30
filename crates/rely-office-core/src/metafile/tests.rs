// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use super::*;

#[test]
fn draws_only_metafiles() {
    assert_eq!(convert_metafile_to_svg(b"\x89PNG\r\n\x1a\n"), None);
    assert_eq!(convert_metafile_to_svg(b""), None);
    // A WMF of no placeable header that draws one rectangle.
    let mut wmf: Vec<u8> = [1_u16, 9, 0x0300, 0, 0, 0, 0, 0, 0]
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect();
    for (function, parameters) in [
        (0x020C_u16, vec![10_i16, 10]),
        (0x041B, vec![5, 5, 0, 0]),
        (0, Vec::new()),
    ] {
        let size = u32::try_from(3 + parameters.len()).unwrap();
        wmf.extend_from_slice(&size.to_le_bytes());
        wmf.extend_from_slice(&function.to_le_bytes());
        wmf.extend(parameters.iter().flat_map(|word| word.to_le_bytes()));
    }
    let svg = String::from_utf8(convert_metafile_to_svg(&wmf).unwrap()).unwrap();
    assert!(svg.contains(r#"d="M0 0H5V5H0Z""#), "{svg}");
}
