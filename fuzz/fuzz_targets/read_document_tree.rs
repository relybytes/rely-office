// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

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
