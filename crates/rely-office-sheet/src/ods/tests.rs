// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use std::io::Read;

use rely_office_core::{
    Border, CellRange, ComparisonOperator, Condition, ConditionalFormat, ConditionalRule,
    DataValidation, DifferentialLook, ErrorStyle, Limits, ValidationKind,
};

use super::*;
use crate::styling::{BorderPlacement, StylePatch};
use crate::xlsx::read_spreadsheet;

fn cell(address: &str) -> CellRef {
    CellRef::parse(address).unwrap()
}

fn range(reference: &str) -> CellRange {
    CellRange::parse(reference).unwrap()
}

#[expect(
    clippy::too_many_lines,
    reason = "a workbook with every part an ods keeps"
)]
fn sample() -> Workbook {
    let mut book = Workbook::new();
    let inputs: Vec<(CellRef, String)> = [
        ("A1", "Voce  con   spazi"),
        ("B1", "12.5"),
        ("C1", "=B1*2"),
        ("D1", "TRUE"),
        ("E1", "=1/0"),
        ("F1", "46322"),
        ("G1", "0.25"),
        ("H1", "=SUM(B1:C1)&\" €\""),
        ("A2", "prima\nseconda"),
        ("B2", "=IF(B1>10,\"alto\",\"basso\")"),
        ("C2", "=Riepilogo!A1+1"),
        ("D2", "46322.75"),
    ]
    .iter()
    .map(|(address, text)| (cell(address), (*text).to_owned()))
    .collect();
    book.set_inputs(0, &inputs, Locale::English).unwrap();
    book.set_format(0, cell("F1"), NumberFormat::Date).unwrap();
    book.set_format(0, cell("G1"), NumberFormat::Percent)
        .unwrap();
    book.set_format(0, cell("B1"), NumberFormat::Decimals(2))
        .unwrap();
    book.set_format(0, cell("D2"), NumberFormat::DateTime)
        .unwrap();
    book.set_style(
        0,
        (cell("A1"), cell("B2")),
        &StylePatch {
            bold: Some(true),
            fill: Some("FFF2CC".to_owned()),
            font_colour: Some("C00000".to_owned()),
            ..StylePatch::default()
        },
    )
    .unwrap();
    book.set_borders(
        0,
        (cell("A1"), cell("A1")),
        BorderPlacement::Outline,
        Some(&Border {
            style: rely_office_core::BorderStyle::Double,
            width_eighths: 12,
            colour: Some("1F4E79".to_owned()),
        }),
    )
    .unwrap();
    book.merge_cells(0, (cell("A4"), cell("C5"))).unwrap();
    book.set_column_width(0, 0, Some(30.0)).unwrap();
    book.set_column_width(0, 4, Some(0.0)).unwrap();
    book.set_row_height(0, 1, Some(40.0)).unwrap();
    book.set_row_height(0, 2, Some(0.0)).unwrap();
    book.set_conditional_formats(
        0,
        vec![ConditionalFormat {
            ranges: vec![range("B1:C2")],
            rules: vec![
                ConditionalRule {
                    condition: Condition::CellIs {
                        operator: ComparisonOperator::Greater,
                        formulas: vec!["$B$1".to_owned()],
                    },
                    look: Some(DifferentialLook {
                        fill: Some("FFC7CE".to_owned()),
                        bold: Some(true),
                        ..DifferentialLook::default()
                    }),
                    priority: 1,
                    stop_if_true: false,
                },
                ConditionalRule {
                    condition: Condition::Expression {
                        formula: "MOD(ROW(),2)=0".to_owned(),
                    },
                    look: None,
                    priority: 2,
                    stop_if_true: false,
                },
            ],
        }],
    )
    .unwrap();
    book.set_validations(
        0,
        vec![DataValidation {
            ranges: vec![range("J1:J20")],
            kind: ValidationKind::List,
            operator: ComparisonOperator::Between,
            formulas: vec!["\"Sì,No\"".to_owned()],
            allow_blank: true,
            dropdown: true,
            error_style: ErrorStyle::Warning,
            show_error: true,
            error_title: Some("Scelta".to_owned()),
            error: Some("Sì o No".to_owned()),
            show_prompt: false,
            prompt_title: None,
            prompt: None,
        }],
    )
    .unwrap();
    let second = book.add_sheet("Riepilogo").unwrap();
    book.set_input(second, cell("A1"), "=Foglio1!C1*2", Locale::English)
        .unwrap();
    book
}

#[test]
fn reads_back_what_it_writes_as_ods() {
    let original = sample();
    let bytes = write_ods(&original).unwrap();
    let read = read_spreadsheet(
        &bytes,
        Some("conti.ods"),
        &Limits::default(),
        Locale::English,
    )
    .unwrap();
    assert_eq!(read.sheets().len(), 2);
    assert_eq!(read.sheets()[1].name, "Riepilogo");
    for (address, expected) in [
        ("A1", "Voce  con   spazi"),
        ("B1", "12.5"),
        ("C1", "=B1*2"),
        ("H1", "=SUM(B1:C1)&\" €\""),
        ("A2", "prima\nseconda"),
        ("B2", "=IF(B1>10,\"alto\",\"basso\")"),
        ("C2", "=Riepilogo!A1+1"),
    ] {
        assert_eq!(
            read.sheets()[0].cell(cell(address)).unwrap().input.text(),
            expected,
            "{address}"
        );
    }
    assert_eq!(
        read.sheets()[1].cell(cell("A1")).unwrap().input.text(),
        "=Foglio1!C1*2"
    );
    assert_eq!(read.value(0, cell("C1")), Value::Number(25.0));
    assert_eq!(read.value(0, cell("D1")), Value::Bool(true));
    for (address, format) in [
        ("F1", NumberFormat::Date),
        ("G1", NumberFormat::Percent),
        ("B1", NumberFormat::Decimals(2)),
        ("D2", NumberFormat::DateTime),
    ] {
        assert_eq!(
            read.sheets()[0].cell(cell(address)).unwrap().format,
            format,
            "{address}"
        );
    }
    assert_eq!(read.value(0, cell("D2")), Value::Number(46322.75));
    let style = read.style(0, cell("B2"));
    assert!(style.font.bold);
    assert_eq!(style.fill.as_deref(), Some("FFF2CC"));
    assert_eq!(style.font.colour.as_deref(), Some("C00000"));
    let border = read.style(0, cell("A1")).borders.top.clone().unwrap();
    assert_eq!(
        (border.style, border.width_eighths, border.colour.as_deref()),
        (rely_office_core::BorderStyle::Double, 12, Some("1F4E79"))
    );
    assert_eq!(read.merges(0), &[range("A4:C5")]);
    let widths = &read.sheets()[0].column_widths;
    assert!((widths[&0] - 30.0).abs() < 0.2, "{widths:?}");
    assert_eq!(widths.get(&4), Some(&0.0), "a hidden column");
    let heights = &read.sheets()[0].row_heights;
    assert!((heights[&1] - 40.0).abs() < 0.1);
    assert_eq!(heights.get(&2), Some(&0.0), "a hidden row");
    assert_eq!(
        read.sheets()[0].conditional_formats(),
        original.sheets()[0].conditional_formats()
    );
    assert_eq!(
        read.sheets()[0].validations(),
        original.sheets()[0].validations()
    );
    assert_eq!(read.conditional_looks(0), original.conditional_looks(0));
}

#[test]
fn starts_with_its_mimetype_stored() {
    let bytes = write_ods(&Workbook::new()).unwrap();
    assert_eq!(&bytes[30..38], b"mimetype");
    assert_eq!(&bytes[38..38 + MIMETYPE.len()], MIMETYPE.as_bytes());
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    assert_eq!(
        archive.by_index(0).unwrap().compression(),
        zip::CompressionMethod::Stored
    );
    let mut content = String::new();
    archive
        .by_name("content.xml")
        .unwrap()
        .read_to_string(&mut content)
        .unwrap();
    assert!(content.contains(r#"<table:table table:name="Foglio1">"#));
    assert!(
        content.contains("<table:table-row"),
        "an empty sheet has one row"
    );
    let read = read_spreadsheet(
        &write_ods(&Workbook::new()).unwrap(),
        Some("vuoto.ods"),
        &Limits::default(),
        Locale::English,
    )
    .unwrap();
    assert_eq!(read.sheets()[0].name, "Foglio1");
}

#[test]
fn folds_repeated_rows_and_cells() {
    let mut book = Workbook::new();
    for row in [0, 9] {
        book.set_input(0, CellRef::new(row, 0), "=$C$1/2", Locale::English)
            .unwrap();
        book.set_input(0, CellRef::new(row, 1), "=$C$1/2", Locale::English)
            .unwrap();
    }
    let bytes = write_ods(&book).unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut content = String::new();
    archive
        .by_name("content.xml")
        .unwrap()
        .read_to_string(&mut content)
        .unwrap();
    assert!(
        content.contains(r#"table:formula="of:=[.$C$1]/2" office:value-type="float" office:value="0" calcext:value-type="float" table:number-columns-repeated="2">"#),
        "{content}"
    );
    assert!(
        content.contains(r#"table:number-rows-repeated="8""#),
        "{content}"
    );
    assert_eq!(
        with_attribute(r#"<a b="x>y"/>"#, " c=\"1\""),
        r#"<a b="x>y" c="1"/>"#
    );
    assert_eq!(
        with_attribute("<a>text</a>", " c=\"1\""),
        r#"<a c="1">text</a>"#
    );
    assert_eq!(runs(vec![1, 1, 2, 1]), vec![(1, 2), (2, 1), (1, 1)]);
}

#[test]
fn keeps_spaces_dates_and_markup_characters() {
    assert_eq!(paragraph_text("a  b"), r#"a <text:s text:c="1"/>b"#);
    assert_eq!(
        paragraph_text("  a\tb "),
        r#"<text:s text:c="2"/>a<text:tab/>b<text:s text:c="1"/>"#
    );
    assert_eq!(paragraph_text("<&>"), "&lt;&amp;&gt;");
    assert_eq!(
        iso_date(46322.0, NumberFormat::Date).as_deref(),
        Some("2026-10-27")
    );
    assert_eq!(
        iso_date(46322.5, NumberFormat::DateTime).as_deref(),
        Some("2026-10-27T12:00:00")
    );
    assert_eq!(iso_date(-5.0, NumberFormat::Date), None);
    assert_eq!(escape("a'\"\u{1}"), "a&apos;&quot;");
    assert!((column_inches(DEFAULT_COLUMN_CHARACTERS) - 64.0 / 96.0).abs() < 1e-9);
}
