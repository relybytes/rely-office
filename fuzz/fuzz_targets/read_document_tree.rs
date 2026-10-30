// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

#![no_main]

use libfuzzer_sys::fuzz_target;
use rely_office_core::{Limits, read_document_tree};

fuzz_target!(|bytes: &[u8]| {
    let limits = Limits {
        max_member_bytes: 4 * 1024 * 1024,
        max_text_chars: 10_000,
        max_images: 4,
        max_image_bytes: 256 * 1024,
        max_total_image_bytes: 512 * 1024,
        ..Limits::default()
    };
    if let Ok(tree) = read_document_tree(bytes, &limits) {
        assert!(tree.media.len() <= limits.max_images);
    }
});
