// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

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
