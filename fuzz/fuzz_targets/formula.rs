// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

#![no_main]

use libfuzzer_sys::fuzz_target;
use rely_office_sheet::{parse_formula, print_formula};

// A formula prints back to text that parses to the same formula.
fuzz_target!(|text: &str| {
    if let Ok(expression) = parse_formula(text) {
        let printed = print_formula(&expression);
        let reparsed = parse_formula(&printed).expect("a printed formula parses");
        assert_eq!(print_formula(&reparsed), printed);
    }
});
