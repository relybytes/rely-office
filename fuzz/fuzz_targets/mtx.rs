// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

#![no_main]

use libfuzzer_sys::fuzz_target;
use rely_office_core::decompress_mtx;

const MAX_BYTES: usize = 1 << 20;

// Any bytes give a TrueType file within the size allowed, or nothing.
fuzz_target!(|bytes: &[u8]| {
    if let Some(font) = decompress_mtx(bytes, MAX_BYTES) {
        assert!(font.len() <= MAX_BYTES);
    }
});
