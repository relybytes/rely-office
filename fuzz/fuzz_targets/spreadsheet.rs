// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

#![no_main]

use libfuzzer_sys::fuzz_target;
use rely_office_core::Limits;
use rely_office_sheet::{Locale, read_spreadsheet, write_xlsx};

// Any readable spreadsheet is written as an xlsx this crate reads back, sheet for sheet.
fuzz_target!(|bytes: &[u8]| {
    let limits = Limits {
        max_member_bytes: 2 * 1024 * 1024,
        ..Limits::default()
    };
    for name in [None, Some("a.csv")] {
        if let Ok(workbook) = read_spreadsheet(bytes, name, &limits, Locale::Italian) {
            let written = write_xlsx(&workbook).expect("a workbook is written");
            let read = read_spreadsheet(&written, None, &Limits::default(), Locale::Italian)
                .expect("a written workbook reads back");
            assert_eq!(read.sheets().len(), workbook.sheets().len());
        }
    }
});
