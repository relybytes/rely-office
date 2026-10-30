// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

#![no_main]

use libfuzzer_sys::fuzz_target;
use rely_office_core::{Limits, read_workbook};

fuzz_target!(|bytes: &[u8]| {
    let limits = Limits {
        max_member_bytes: 4 * 1024 * 1024,
        max_rows: 50,
        max_columns: 20,
        ..Limits::default()
    };
    if let Ok(workbook) = read_workbook(bytes, Some("a.csv"), &limits) {
        for sheet in &workbook.sheets {
            assert!(sheet.rows.len() <= limits.max_rows);
            assert!(sheet.rows.iter().all(|row| row.len() <= limits.max_columns));
        }
    }
    let _ = read_workbook(bytes, None, &limits);
});
