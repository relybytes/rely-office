// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

#![no_main]

use libfuzzer_sys::fuzz_target;
use rely_office_core::{Limits, extract_text};

// Any bytes, under any of the names that steer the delimited formats: an error, never a panic.
fuzz_target!(|bytes: &[u8]| {
    let limits = Limits {
        max_member_bytes: 4 * 1024 * 1024,
        max_text_chars: 10_000,
        ..Limits::default()
    };
    for name in [None, Some("a.csv"), Some("a.tsv")] {
        let _ = extract_text(bytes, name, &limits);
    }
});
