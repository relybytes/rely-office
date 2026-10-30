// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

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
