// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;

const IT: NumberLocale = NumberLocale::Italian;
const EN: NumberLocale = NumberLocale::English;
/// 27 October 2026, a Tuesday.
const DAY: f64 = 46_322.0;

fn shown(value: f64, code: &str) -> String {
    format_number(value, code, IT).text
}

#[test]
fn groups_thousands_and_rounds_decimals_in_the_locale() {
    assert_eq!(shown(1_234.5, "#,##0.00"), "1.234,50");
    assert_eq!(format_number(1_234.5, "#,##0.00", EN).text, "1,234.50");
    assert_eq!(shown(-1_234.5, "#,##0.00"), "-1.234,50");
    assert_eq!(shown(1_234_567.891, "#,##0"), "1.234.568");
    assert_eq!(
        shown(1_234_567.0, "#,##0,"),
        "1.235",
        "a trailing comma scales by a thousand"
    );
    assert_eq!(shown(1_234_567_890.0, "#,##0.0,,"), "1.234,6");
    assert_eq!(shown(5.0, "0000"), "0005");
    assert_eq!(shown(0.5, "#.##"), ",5");
    assert_eq!(shown(2.0, "#.##"), "2,");
    assert_eq!(shown(1.5, "0.0#"), "1,5");
    assert_eq!(shown(1.234, "0.0#"), "1,23");
    assert_eq!(shown(1.5, "0.0?"), "1,5 ");
    assert_eq!(shown(7.0, "??0"), "  7");
    assert_eq!(shown(0.0, "#,##0"), "0");
    assert_eq!(
        shown(-0.001, "0.00"),
        "0,00",
        "a value that rounds to zero has no sign"
    );
    assert_eq!(shown(5_551_234.0, "000-0000"), "555-1234");
}

#[test]
fn shows_currencies_literals_and_percentages() {
    assert_eq!(shown(1_234.5, "#,##0.00 \"€\""), "1.234,50 €");
    assert_eq!(shown(1_234.5, "[$€-410] #,##0.00"), "€ 1.234,50");
    assert_eq!(shown(1_234.5, "\\€ #,##0"), "€ 1.235");
    assert_eq!(shown(1_234.0, "_-* #,##0 €_-"), " 1.234 € ");
    assert_eq!(shown(0.125, "0.0%"), "12,5%");
    assert_eq!(shown(0.07, "0%"), "7%");
    assert_eq!(shown(42.0, "\"Totale: \"General"), "Totale: 42");
    assert_eq!(shown(42.0, "\"fisso\""), "fisso");
}

#[test]
fn picks_the_section_by_sign_zero_and_condition() {
    let negative = format_number(-5.0, "#,##0;[Red]-#,##0", IT);
    assert_eq!(negative.text, "-5");
    assert_eq!(negative.colour, Some("FF0000"));
    assert_eq!(format_number(5.0, "#,##0;[Red]-#,##0", IT).colour, None);
    assert_eq!(shown(-5.0, "0;(0)"), "(5)");
    assert_eq!(shown(0.0, "#,##0;-#,##0;\"-\""), "-");
    assert_eq!(shown(3.0, "#,##0;-#,##0;\"-\""), "3");
    assert_eq!(shown(1_500.0, "[>=1000]#,##0;0.00"), "1.500");
    assert_eq!(shown(5.0, "[>=1000]#,##0;0.00"), "5,00");
    assert_eq!(shown(5.0, "[<10][Blu]0;0"), "5");
    assert_eq!(format_number(5.0, "[Blue]0", IT).colour, Some("0000FF"));
    assert_eq!(format_number(5.0, "[Color10]0", IT).colour, None);
    assert_eq!(
        shown(-5.0, "@"),
        "-5",
        "a text-only code shows numbers as General"
    );
}

#[test]
fn writes_exponents_and_fractions() {
    assert_eq!(shown(12_345.678, "0.00E+00"), "1,23E+04");
    assert_eq!(shown(0.000_123, "0.00E+00"), "1,23E-04");
    assert_eq!(shown(9.996, "0.00E+00"), "1,00E+01");
    assert_eq!(shown(12_345.0, "##0.0E+0"), "12,3E+3");
    assert_eq!(shown(0.0, "0.00E+00"), "0,00E+00");
    assert_eq!(shown(1.25, "# ?/?"), "1 1/4");
    assert_eq!(shown(0.75, "?/4"), "3/4");
    assert_eq!(shown(3.0, "# ?/?"), "3");
    assert_eq!(shown(0.333, "# ??/??"), "1/3");
    assert_eq!(shown(1.99, "# ?/?"), "2");
}

#[test]
fn writes_dates_and_times_with_the_names_of_the_locale() {
    assert_eq!(shown(DAY, "dd/mm/yyyy"), "27/10/2026");
    assert_eq!(shown(DAY, "d mmmm yyyy"), "27 ottobre 2026");
    assert_eq!(
        format_number(DAY, "d mmmm yyyy", EN).text,
        "27 October 2026"
    );
    assert_eq!(shown(DAY, "dddd d mmm"), "martedì 27 ott");
    assert_eq!(format_number(DAY, "ddd", EN).text, "Tue");
    assert_eq!(shown(DAY, "mmm-yy"), "ott-26");
    assert_eq!(shown(DAY, "mmmmm"), "o");
    assert_eq!(shown(DAY, "[$-409]mmmm"), "October");
    assert_eq!(shown(DAY, "yyyy-mm-dd"), "2026-10-27");
    assert_eq!(shown(DAY, "dd.mm.yyyy"), "27.10.2026");
    assert_eq!(shown(DAY, "dddd, d mmmm"), "martedì, 27 ottobre");
    assert_eq!(shown(DAY + 0.5, "dd/mm/yyyy hh:mm"), "27/10/2026 12:00");
    assert_eq!(shown(DAY + 0.5, "h:mm AM/PM"), "12:00 PM");
    assert_eq!(shown(0.75, "h:mm AM/PM"), "6:00 PM");
    assert_eq!(shown(0.25, "h:mm a/p"), "6:00 A");
    assert_eq!(shown(0.0, "h:mm AM/PM"), "12:00 AM");
    assert_eq!(shown(1.5, "[h]:mm:ss"), "36:00:00");
    assert_eq!(shown(0.5, "[mm]"), "720");
    assert_eq!(shown(1.5 / 86_400.0, "mm:ss.0"), "00:01,5");
    assert_eq!(shown(1.0 / 1_440.0, "h:m:s"), "0:1:0");
    assert_eq!(
        shown(60.0, "dd/mm/yyyy"),
        "29/02/1900",
        "Excel's false leap day"
    );
    assert_eq!(shown(61.0, "dd/mm/yyyy"), "01/03/1900");
    assert_eq!(shown(1.0, "dd/mm/yyyy"), "01/01/1900");
    assert_eq!(shown(-1.0, "dd/mm/yyyy"), "########");
    assert_eq!(shown(f64::NAN, "0"), "#NUM!");
}

#[test]
fn shows_general_numbers_as_excel_does() {
    assert_eq!(general_number(1_234.5, IT), "1234,5");
    assert_eq!(general_number(0.1 + 0.2, IT), "0,3");
    assert_eq!(general_number(1e12, IT), "1E+12");
    assert_eq!(general_number(123_456_789_012.0, EN), "1.23457E+11");
    assert_eq!(general_number(0.000_000_000_12, EN), "1.2E-10");
    assert_eq!(general_number(0.0, IT), "0");
    assert_eq!(general_number(-2.0, IT), "-2");
    assert_eq!(shown(1_234.5, "General"), "1234,5");
    assert_eq!(shown(-1_234.5, ""), "-1234,5");
}

#[test]
fn shows_text_with_its_section() {
    assert_eq!(format_text("abc", "\"Nome: \"@"), "Nome: abc");
    assert_eq!(format_text("abc", "0.00"), "abc");
    assert_eq!(format_text("abc", "0;0;0;\"T:\"@"), "T:abc");
    assert_eq!(format_text("abc", "0;0;0;\"solo\""), "solo");
}

#[test]
fn knows_the_built_in_codes_and_which_are_dates() {
    assert_eq!(builtin_format_code(4), Some("#,##0.00"));
    assert_eq!(builtin_format_code(14), Some("m/d/yyyy"));
    assert_eq!(builtin_format_code(49), Some("@"));
    assert_eq!(builtin_format_code(999), None);
    assert!(is_date_code("dd/mm/yyyy"));
    assert!(is_date_code("[h]:mm"));
    assert!(is_date_code("[$-410]d mmmm"));
    assert!(!is_date_code("0.00"));
    assert!(!is_date_code("General"));
    assert!(!is_date_code("#,##0 \"mm\""));
    for id in 0..50 {
        if let Some(code) = builtin_format_code(id) {
            let _ = format_number(1_234.5, code, IT);
            let _ = format_number(-0.5, code, EN);
        }
    }
}

#[test]
fn survives_damaged_codes() {
    for code in [
        "\"open",
        "[",
        "[>",
        "0.0E+",
        "#/",
        "\\",
        "_",
        "*",
        "0;;;;;",
        "[$",
        "[h",
        "%%%",
        "0.000000000000000000000000000000000000000",
    ] {
        let _ = format_number(1.5, code, IT);
        let _ = format_number(-1.5, code, IT);
        let _ = format_text("x", code);
    }
    assert_eq!(shown(1.0, &"0".repeat(1_000)).len(), MAX_CODE_CHARS);
}
