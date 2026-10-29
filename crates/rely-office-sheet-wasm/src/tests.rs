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
            "input": "=SOMMA(A1:A2)",
            "display": "3,50",
            "kind": "number",
            "format": { "type": "decimals", "decimals": 2 },
            "style": 0
        })
    );
    assert_eq!(document.input(0, 2, 0), "=SOMMA(A1:A2)");
    assert_eq!(document.input(0, 0, 0), "1,5");
    assert_eq!(document.input(0, 9, 9), "");
    let sheets = parse(&document.sheets_summary().unwrap());
    assert_eq!(
        sheets,
        json!([{
            "name": "Foglio1", "rows": 3, "columns": 2, "columnWidths": [], "rowHeights": [],
            "pivotTables": 0, "frozenRows": 0, "frozenColumns": 0, "autoFilter": null, "pictures": 0
        }])
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
        (EditError::InvalidStyle, "INVALID_STYLE"),
    ] {
        assert_eq!(Failure::from(error).code, code);
    }
    assert!(Failure::new("X", "y").error_message().starts_with("X: "));
}

#[test]
fn styles_merges_and_sizes_cells() {
    let mut document = SheetDocument::create("it").unwrap();
    let range = (CellRef::new(0, 0), CellRef::new(0, 2));
    document
        .set_style_json(
            0,
            range,
            r##"{"bold":true,"fill":"#1F4E79","fontColour":"FFFFFF","horizontal":"center"}"##,
        )
        .unwrap();
    document
        .set_borders_json(
            0,
            range,
            "outline",
            r#"{"style":"single","widthEighths":4,"colour":"FF0000"}"#,
        )
        .unwrap();
    document
        .set_inputs_json(0, r#"[[0,0,"Titolo"],[1,0,"1234,5"]]"#)
        .unwrap();
    document
        .set_format_json(0, 1, 0, r##"{"type":"code","code":"#,##0.00 \"€\""}"##)
        .unwrap();
    document.workbook.merge_cells(0, range).unwrap();
    document.workbook.set_row_height(0, 0, Some(30.0)).unwrap();

    let cells = parse(&document.window(0, 0, 0, 1, 2).unwrap());
    let cells = cells.as_array().unwrap();
    assert_eq!(
        cells.len(),
        4,
        "the styled empty cells of the title come too"
    );
    assert_eq!(cells[1]["input"], "", "{cells:?}");
    assert_eq!(cells[1]["kind"], "empty");
    assert_ne!(cells[1]["style"], 0);
    assert_eq!(cells[3]["display"], "1.234,50 €");
    assert_eq!(
        cells[3]["format"],
        json!({ "type": "code", "code": "#,##0.00 \"€\"" })
    );

    let styles = parse(&document.styles_summary().unwrap());
    let title = &styles[usize::try_from(cells[0]["style"].as_u64().unwrap()).unwrap()];
    assert_eq!(title["font"]["bold"], true);
    assert_eq!(title["font"]["colour"], "FFFFFF");
    assert_eq!(title["fill"], "1F4E79");
    assert_eq!(title["horizontal"], "center");
    assert_eq!(title["borders"]["left"]["style"], "single");
    assert_eq!(title["borders"]["left"]["colour"], "FF0000");
    assert_eq!(styles[0]["fill"], Json::Null);

    assert_eq!(
        parse(&document.merges_summary(0).unwrap()),
        json!([{ "firstRow": 0, "firstColumn": 0, "lastRow": 0, "lastColumn": 2 }])
    );
    assert_eq!(
        parse(&document.sheets_summary().unwrap())[0]["rowHeights"],
        json!([[0, 30.0]])
    );
    assert_eq!(
        document.merges_summary(4).unwrap_err().code,
        "NO_SUCH_SHEET"
    );

    for (placement, border) in [
        ("around", "null"),
        ("all", "{\"style\":\"wavy\"}"),
        (
            "all",
            r#"{"style":"single","widthEighths":0,"colour":null}"#,
        ),
        (
            "all",
            r#"{"style":"single","widthEighths":4,"colour":"red"}"#,
        ),
    ] {
        let code = document
            .set_borders_json(0, range, placement, border)
            .unwrap_err()
            .code;
        assert!(
            matches!(code, "INVALID_INPUT" | "INVALID_STYLE"),
            "{placement} {border}"
        );
    }
    document.set_borders_json(0, range, "all", "null").unwrap();
    assert_eq!(
        document
            .set_style_json(0, range, r#"{"blink":true}"#)
            .unwrap_err()
            .code,
        "INVALID_INPUT"
    );
    assert_eq!(
        document
            .set_style_json(0, range, r#"{"fill":"giallo"}"#)
            .unwrap_err()
            .code,
        "INVALID_STYLE"
    );
}

#[test]
fn sets_and_reads_conditional_formats_and_validations_as_json() {
    let mut document = SheetDocument::create("en").unwrap();
    document
        .set_inputs_json(0, r#"[[0,0,"5"],[1,0,"50"],[0,1,"x"]]"#)
        .unwrap();
    document
        .set_conditional_formats_json(
            0,
            r#"[{"ranges":[{"firstRow":0,"firstColumn":0,"lastRow":1,"lastColumn":0}],"rules":[{"condition":{"kind":"cellIs","operator":"greater","formulas":["10"]},"look":{"fill":"FF0000","bold":true},"priority":1}]},{"ranges":[{"firstRow":0,"firstColumn":0,"lastRow":1,"lastColumn":0}],"rules":[{"condition":{"kind":"dataBar","min":{"kind":"min"},"max":{"kind":"number","value":"100"},"colour":"638EC6","showValue":true},"priority":2}]}]"#,
        )
        .unwrap();
    let formats = parse(&document.conditional_formats_summary(0).unwrap());
    assert_eq!(formats[0]["rules"][0]["condition"]["operator"], "greater");
    assert_eq!(formats[1]["rules"][0]["condition"]["max"]["value"], "100");
    let looks = parse(&document.conditional_looks_summary(0).unwrap());
    assert_eq!(looks[1]["row"], 1);
    assert_eq!(looks[1]["fill"], "FF0000");
    assert_eq!(looks[1]["bold"], true);
    assert_eq!(looks[0]["fill"], Json::Null);
    assert!(looks[0]["bar"]["length"].as_f64().unwrap() < 0.2);
    document
        .set_validations_json(
            0,
            r#"[{"ranges":[{"firstRow":0,"firstColumn":1,"lastRow":4,"lastColumn":1}],"kind":"list","formulas":["\"a,b\""],"dropdown":true}]"#,
        )
        .unwrap();
    assert_eq!(
        parse(&document.validation_summary(0, CellRef::new(0, 1)).unwrap()),
        json!({
            "validation": parse(&document.validations_summary(0).unwrap())[0],
            "valid": false,
            "items": ["a", "b"]
        })
    );
    assert_eq!(
        parse(&document.invalid_cells_summary(0).unwrap()),
        json!([[0, 1]])
    );
    assert_eq!(
        parse(&document.validation_summary(0, CellRef::new(9, 9)).unwrap()),
        json!({ "validation": null, "valid": true, "items": [] })
    );
    let saved = document.workbook.clone();
    let reopened =
        SheetDocument::open_file(&write_xlsx(&saved).unwrap(), Some("a.xlsx"), "en").unwrap();
    assert_eq!(
        parse(&reopened.conditional_formats_summary(0).unwrap()),
        formats
    );
    assert_eq!(
        parse(&reopened.sheets_summary().unwrap())[0]["pivotTables"],
        0
    );
    for (failure, code) in [
        (
            document.set_conditional_formats_json(0, "{"),
            "INVALID_INPUT",
        ),
        (document.set_validations_json(0, "[1]"), "INVALID_INPUT"),
        (
            document.set_validations_json(0, r#"[{"ranges":[],"kind":"whole"}]"#),
            "OUT_OF_BOUNDS",
        ),
    ] {
        assert_eq!(failure.unwrap_err().code, code);
    }
    for summary in [
        document.conditional_formats_summary(3),
        document.validations_summary(3),
        document.conditional_looks_summary(3),
        document.validation_summary(3, CellRef::new(0, 0)),
        document.invalid_cells_summary(3),
    ] {
        assert_eq!(summary.unwrap_err().code, "NO_SUCH_SHEET");
    }
}

#[test]
fn sorts_filters_and_freezes_a_sheet() {
    let mut document = SheetDocument::create("it").unwrap();
    document
        .set_inputs_json(
            0,
            r#"[[0,0,"Zona"],[0,1,"Importo"],[1,0,"Sud"],[1,1,"2,5"],[2,0,"Nord"],[2,1,"1"],[3,0,"Sud"],[3,1,"3"]]"#,
        )
        .unwrap();

    document
        .sort_json(0, 0, 0, 3, 1, r#"[{"column":1,"descending":true}]"#, true)
        .unwrap();
    assert_eq!(document.input(0, 1, 1), "3");
    assert_eq!(document.input(0, 3, 1), "1");
    assert_eq!(
        document
            .sort_json(0, 0, 0, 3, 1, r#"[{"column":1,"order":"up"}]"#, true)
            .unwrap_err()
            .code,
        "INVALID_INPUT"
    );

    document
        .workbook
        .set_auto_filter(0, Some((CellRef::new(0, 0), CellRef::new(3, 1))))
        .unwrap();
    let choices = parse(&document.filter_choices_summary(0, 1).unwrap());
    assert_eq!(choices[1], json!({ "key": "2.5", "label": "2,5" }));
    document.filter_column_json(0, 0, r#"["Sud"]"#).unwrap();
    document.workbook.set_frozen(0, 1, 0).unwrap();

    let sheets = parse(&document.sheets_summary().unwrap());
    assert_eq!(sheets[0]["frozenRows"], 1);
    assert_eq!(sheets[0]["frozenColumns"], 0);
    assert_eq!(
        sheets[0]["autoFilter"],
        json!({
            "top": 0, "left": 0, "bottom": 3, "right": 1,
            "columns": [{ "column": 0, "shown": ["Sud"] }],
            "hiddenRows": [3]
        })
    );
    assert_eq!(sheets[0]["pictures"], 0);

    document.filter_column_json(0, 0, "null").unwrap();
    let sheets = parse(&document.sheets_summary().unwrap());
    assert_eq!(sheets[0]["autoFilter"]["hiddenRows"], json!([]));
    assert_eq!(
        document.filter_column_json(0, 9, "null").unwrap_err().code,
        "OUT_OF_BOUNDS"
    );
}

#[test]
fn fills_a_series_as_one_step() {
    let mut document = SheetDocument::create("it").unwrap();
    document
        .set_inputs_json(0, r#"[[0,0,"1"],[1,0,"2"]]"#)
        .unwrap();
    document.fill_range(0, 0, 0, 1, 0, 0, 0, 4, 0).unwrap();
    assert_eq!(document.input(0, 4, 0), "5");
    assert!(document.undo());
    assert_eq!(document.input(0, 4, 0), "");
}

#[test]
fn lists_the_pictures_and_the_functions() {
    let document = SheetDocument::create("it").unwrap();
    assert_eq!(parse(&document.pictures_summary(0).unwrap()), json!([]));
    assert_eq!(
        document.pictures_summary(4).unwrap_err().code,
        "NO_SUCH_SHEET"
    );
    assert_eq!(document.picture_bytes(0, 0), Vec::<u8>::new());
    let names = parse(&document.function_names_json());
    let names: Vec<&str> = names
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap())
        .collect();
    assert!(names.contains(&"SOMMA"));
    assert!(names.contains(&"CERCA.VERT"));
}

#[test]
fn shows_the_cells_an_array_spills_into() {
    let mut document = SheetDocument::create("it").unwrap();
    document
        .set_inputs_json(0, r#"[[0,0,"=SEQUENZA(3;1;10;5)"]]"#)
        .unwrap();
    let cells = parse(&document.window(0, 0, 0, 3, 0).unwrap());
    let shown: Vec<&str> = cells
        .as_array()
        .unwrap()
        .iter()
        .map(|cell| cell["display"].as_str().unwrap())
        .collect();
    assert_eq!(shown, ["10", "15", "20"]);
    assert_eq!(document.input(0, 1, 0), "");
    assert_eq!(document.input(0, 0, 0), "=SEQUENZA(3;1;10;5)");
}

#[test]
fn saves_a_sheet_as_csv_with_the_separator_it_was_read_with() {
    let mut document = SheetDocument::create("it").unwrap();
    document
        .set_inputs_json(0, r#"[[0,0,"Nome"],[0,1,"12,5"],[1,0,"=2*3"]]"#)
        .unwrap();
    let written = document.save_csv(0, None).unwrap();
    assert_eq!(written, "\u{feff}Nome;12,5\r\n6\r\n".as_bytes());
    assert_eq!(document.delimiter(), None);
    let tabbed = SheetDocument::open(b"a\tb\r\n", Some("a.tsv".to_owned()), "en").unwrap();
    assert_eq!(tabbed.delimiter().as_deref(), Some("\t"));
    assert_eq!(
        tabbed.save_csv(0, None).unwrap(),
        "\u{feff}a\tb\r\n".as_bytes()
    );
    assert_eq!(
        tabbed.save_csv(0, Some(";".to_owned())).unwrap(),
        "\u{feff}a;b\r\n".as_bytes()
    );
}

#[test]
fn lists_names_and_comments_as_json() {
    let mut document = SheetDocument::create("it").unwrap();
    document
        .workbook
        .set_defined_name("Aliquota", None, "=0,22", Locale::Italian)
        .unwrap();
    document
        .workbook
        .set_defined_name("Somma", Some(0), "=SOMMA(A1:A2;1)", Locale::Italian)
        .unwrap();
    assert_eq!(
        parse(&document.defined_names_summary().unwrap()),
        json!([
            { "name": "Aliquota", "sheet": null, "formula": "=0,22", "hidden": false },
            { "name": "Somma", "sheet": 0, "formula": "=SOMMA(Foglio1!A1:A2;1)", "hidden": false }
        ])
    );
    document
        .workbook
        .set_comment(0, CellRef::new(1, 2), "Anna", "Da rivedere")
        .unwrap();
    assert_eq!(
        parse(&document.comments_summary(0).unwrap()),
        json!([{ "row": 1, "column": 2, "author": "Anna", "text": "Da rivedere" }])
    );
    assert_eq!(
        document.comments_summary(4).unwrap_err().code,
        "NO_SUCH_SHEET"
    );
    let failure: Failure = document
        .workbook
        .set_defined_name("A1", None, "1", Locale::Italian)
        .unwrap_err()
        .into();
    assert_eq!(failure.code, "INVALID_NAME");
}
