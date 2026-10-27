// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;
use crate::doc::test_doc::font;

#[test]
fn reads_each_font_name_in_order() {
    let mut table = vec![3, 0, 0, 0];
    table.extend(font("Times New Roman"));
    table.extend(font("Città"));
    table.extend(font(""));
    assert_eq!(font_names(&table), vec!["Times New Roman", "Città", ""]);
}

#[test]
fn stops_at_a_font_cut_short() {
    let mut table = vec![2, 0, 0, 0];
    table.extend(font("Arial"));
    table.push(200);
    assert_eq!(font_names(&table), vec!["Arial"]);
    let mut short = vec![2, 0, 0, 0];
    short.extend(font("Arial"));
    assert_eq!(
        font_names(&short),
        vec!["Arial"],
        "the second font is missing"
    );
    assert_eq!(font_names(&[1]), Vec::<String>::new());
}
