// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use serde_json::{Value as Json, json};

use super::*;

fn parse(text: &str) -> Json {
    serde_json::from_str(text).unwrap()
}

#[test]
fn edits_and_shows_a_window_of_cells() {
    let mut document = SheetDocument::create("it").unwrap();
    document
        .set_inputs_json(
            0,
            r#"[[0,0,"1,5"],[1,0,"2"],[2,0,"=SOMMA(A1:A2)"],[0,1,"testo"]]"#,
        )
        .unwrap();
    document
        .set_format_json(0, 2, 0, r#"{"type":"decimals","decimals":2}"#)
        .unwrap();
    let cells = parse(&document.window(0, 0, 0, 2, 0).unwrap());
    assert_eq!(cells.as_array().unwrap().len(), 3);
    assert_eq!(
        cells[2],
        json!({
            "row": 2,
            "column": 0,
            "input": "=SUM(A1:A2)",
            "display": "3,50",
            "kind": "number",
            "format": { "type": "decimals", "decimals": 2 }
        })
    );
    assert_eq!(document.input(0, 2, 0), "=SUM(A1:A2)");
    assert_eq!(document.input(0, 9, 9), "");
    let sheets = parse(&document.sheets_summary().unwrap());
    assert_eq!(
        sheets,
        json!([{ "name": "Foglio1", "rows": 3, "columns": 2, "columnWidths": [] }])
    );
    assert_eq!(document.preview_input("=A1"), "formula");
    assert_eq!(document.preview_input("1.234,5"), "number");
    assert_eq!(document.preview_input("=("), "invalid");
    assert_eq!(document.preview_input(""), "empty");
}

#[test]
fn saves_and_opens_a_workbook() {
    let mut document = SheetDocument::create("en").unwrap();
    document
        .set_inputs_json(0, r#"[[0,0,"2"],[0,1,"=A1*21"]]"#)
        .unwrap();
    let bytes = write_xlsx(&document.workbook).unwrap();
    let opened = SheetDocument::open_file(&bytes, Some("conti.xlsx"), "en").unwrap();
    let cells = parse(&opened.window(0, 0, 0, 0, 1).unwrap());
    assert_eq!(cells[1]["display"], "42");
    assert_eq!(cells[1]["input"], "=A1*21");
}

#[test]
fn gives_failures_their_codes() {
    assert_eq!(
        SheetDocument::create("fr").unwrap_err().code,
        "INVALID_LOCALE"
    );
    let mut document = SheetDocument::create("it").unwrap();
    assert_eq!(
        document.window(3, 0, 0, 0, 0).unwrap_err().code,
        "NO_SUCH_SHEET"
    );
    assert_eq!(
        document.window(0, 5, 0, 1, 0).unwrap_err().code,
        "OUT_OF_BOUNDS"
    );
    assert_eq!(
        document.window(0, 0, 0, 999_999, 999).unwrap_err().code,
        "OUT_OF_BOUNDS"
    );
    assert_eq!(
        document.set_inputs_json(0, "nope").unwrap_err().code,
        "INVALID_INPUT"
    );
    assert_eq!(
        document
            .set_inputs_json(0, r#"[[0,0,"=("]]"#)
            .unwrap_err()
            .code,
        "INVALID_FORMULA"
    );
    assert_eq!(
        document
            .set_format_json(0, 0, 0, r#"{"type":"bold"}"#)
            .unwrap_err()
            .code,
        "INVALID_INPUT"
    );
    assert_eq!(
        SheetDocument::open_file(b"plain", None, "it")
            .unwrap_err()
            .code,
        "UNSUPPORTED_FORMAT"
    );
    for (error, code) in [
        (
            EditError::DuplicateSheetName("x".to_owned()),
            "DUPLICATE_SHEET_NAME",
        ),
        (EditError::InvalidSheetName, "INVALID_SHEET_NAME"),
        (EditError::LastSheet, "LAST_SHEET"),
    ] {
        assert_eq!(Failure::from(error).code, code);
    }
    assert!(Failure::new("X", "y").error_message().starts_with("X: "));
}
