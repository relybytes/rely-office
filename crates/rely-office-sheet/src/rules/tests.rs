// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use rely_office_core::{ConditionalRule, ErrorStyle};

use super::*;
use crate::value::Locale;

fn cell(address: &str) -> CellRef {
    CellRef::parse(address).unwrap()
}

fn range(reference: &str) -> CellRange {
    CellRange::parse(reference).unwrap()
}

fn typed(book: &mut Workbook, address: &str, text: &str) {
    book.set_input(0, cell(address), text, Locale::English)
        .unwrap();
}

fn validation(kind: ValidationKind, ranges: &str, formulas: &[&str]) -> DataValidation {
    DataValidation {
        ranges: vec![range(ranges)],
        kind,
        operator: ComparisonOperator::Between,
        formulas: formulas
            .iter()
            .map(|formula| (*formula).to_owned())
            .collect(),
        allow_blank: true,
        dropdown: true,
        error_style: ErrorStyle::Stop,
        show_error: true,
        error_title: None,
        error: None,
        show_prompt: false,
        prompt_title: None,
        prompt: None,
    }
}

fn expression_format(ranges: &str, formula: &str) -> ConditionalFormat {
    ConditionalFormat {
        ranges: vec![range(ranges)],
        rules: vec![ConditionalRule {
            condition: Condition::Expression {
                formula: formula.to_owned(),
            },
            look: None,
            priority: 1,
            stop_if_true: false,
        }],
    }
}

#[test]
fn checks_values_against_the_validation_of_their_cell() {
    let mut book = Workbook::new();
    let mut whole = validation(ValidationKind::Whole, "A1:A5", &["10", "1"]);
    whole.operator = ComparisonOperator::Between;
    let mut longer = validation(ValidationKind::TextLength, "B1", &["3"]);
    longer.operator = ComparisonOperator::GreaterOrEqual;
    let custom = validation(ValidationKind::Custom, "C1:C3", &["C1>D1"]);
    let list = validation(ValidationKind::List, "E1", &["\"Sì, No ,\"\"Forse\"\"\""]);
    let from_cells = validation(ValidationKind::List, "F1", &["$H$1:$H$3"]);
    book.set_validations(0, vec![whole, longer, custom, list, from_cells])
        .unwrap();
    for (address, text) in [
        ("A1", "5"),
        ("A2", "12"),
        ("A3", "2.5"),
        ("A4", "testo"),
        ("B1", "ab"),
        ("C1", "5"),
        ("D1", "3"),
        ("C2", "1"),
        ("D2", "3"),
        ("E1", "no"),
        ("H1", "rosso"),
        ("H3", "7"),
        ("F1", "7"),
    ] {
        typed(&mut book, address, text);
    }
    assert!(book.is_valid(0, cell("A1")), "between, in either order");
    assert!(!book.is_valid(0, cell("A2")));
    assert!(
        !book.is_valid(0, cell("A3")),
        "a whole number has no decimals"
    );
    assert!(!book.is_valid(0, cell("A4")));
    assert!(
        book.is_valid(0, cell("A5")),
        "an empty cell is always valid"
    );
    assert!(!book.is_valid(0, cell("B1")));
    assert!(book.is_valid(0, cell("C1")));
    assert!(
        !book.is_valid(0, cell("C2")),
        "the formula follows the cell it is checked for"
    );
    assert!(book.is_valid(0, cell("E1")), "an item without case");
    assert!(book.is_valid(0, cell("F1")), "a number among the cells");
    assert!(book.is_valid(0, cell("Z9")), "no validation");
    assert_eq!(
        book.invalid_cells(0),
        vec![cell("A2"), cell("B1"), cell("C2"), cell("A3"), cell("A4")]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
    );
    assert_eq!(
        book.validation_items(0, cell("E1")),
        vec!["Sì".to_owned(), "No".to_owned(), "\"Forse\"".to_owned()]
    );
    assert_eq!(
        book.validation_items(0, cell("F1")),
        vec!["rosso".to_owned(), "7".to_owned()]
    );
    assert_eq!(book.validation_items(0, cell("A1")), Vec::<String>::new());
    assert_eq!(
        book.validation_at(0, cell("A3")).map(|found| found.kind),
        Some(ValidationKind::Whole)
    );
}

#[test]
fn compares_with_every_operator() {
    let number = |value: f64| Some(Value::Number(value));
    let five = Value::Number(5.0);
    for (operator, first, second, expected) in [
        (ComparisonOperator::Between, 1.0, 9.0, true),
        (ComparisonOperator::NotBetween, 1.0, 9.0, false),
        (ComparisonOperator::Equal, 5.0, 0.0, true),
        (ComparisonOperator::NotEqual, 5.0, 0.0, false),
        (ComparisonOperator::Greater, 4.0, 0.0, true),
        (ComparisonOperator::Less, 4.0, 0.0, false),
        (ComparisonOperator::GreaterOrEqual, 5.0, 0.0, true),
        (ComparisonOperator::LessOrEqual, 4.0, 0.0, false),
    ] {
        assert_eq!(
            compares(operator, &five, number(first), number(second)),
            expected,
            "{operator:?}"
        );
    }
    assert!(!compares(
        ComparisonOperator::Between,
        &five,
        number(1.0),
        None
    ));
    assert!(!compares(ComparisonOperator::Equal, &five, None, None));
    assert!(truthy(&Value::Bool(true)) && truthy(&Value::Number(-1.0)));
    assert!(!truthy(&Value::Text("1".to_owned())) && !truthy(&Value::Number(0.0)));
}

#[test]
fn refuses_rules_that_cannot_hold() {
    let mut book = Workbook::new();
    assert_eq!(
        book.set_validations(3, Vec::new()),
        Err(EditError::NoSuchSheet(3))
    );
    let mut empty = validation(ValidationKind::Whole, "A1", &["1"]);
    empty.ranges.clear();
    assert_eq!(
        book.set_validations(0, vec![empty]),
        Err(EditError::OutOfBounds)
    );
    assert_eq!(
        book.set_validations(
            0,
            vec![validation(ValidationKind::Whole, "A1", &["1", "2", "3"])]
        ),
        Err(EditError::OutOfBounds)
    );
    assert!(matches!(
        book.set_validations(0, vec![validation(ValidationKind::Custom, "A1", &["SUM("])]),
        Err(EditError::Formula(_))
    ));
    assert!(matches!(
        book.set_conditional_formats(0, vec![expression_format("A1", "=(")]),
        Err(EditError::Formula(_))
    ));
    let mut no_rules = expression_format("A1", "TRUE");
    no_rules.rules.clear();
    assert_eq!(
        book.set_conditional_formats(0, vec![no_rules]),
        Err(EditError::OutOfBounds)
    );
    let mut past = expression_format("A1", "TRUE");
    past.ranges[0].last_row = MAX_ROWS;
    assert_eq!(
        book.set_conditional_formats(0, vec![past]),
        Err(EditError::OutOfBounds)
    );
    assert_eq!(
        book.set_conditional_formats(5, Vec::new()),
        Err(EditError::NoSuchSheet(5))
    );
    book.set_conditional_formats(0, vec![expression_format("A1:B2", "A1>0")])
        .unwrap();
    assert_eq!(book.sheets()[0].conditional_formats().len(), 1);
}

#[test]
fn moves_rules_and_their_formulas_with_rows_columns_and_sheets() {
    let mut book = Workbook::new();
    book.add_sheet("Elenco").unwrap();
    book.set_conditional_formats(
        0,
        vec![
            expression_format("B2:B5", "B2>$D$1"),
            expression_format("C3", "C3=1"),
        ],
    )
    .unwrap();
    book.set_validations(
        0,
        vec![validation(
            ValidationKind::List,
            "B2:B5",
            &["Elenco!$A$1:$A$3"],
        )],
    )
    .unwrap();
    book.insert_rows(0, 0, 2).unwrap();
    let sheet = &book.sheets()[0];
    assert_eq!(sheet.conditional_formats()[0].ranges, vec![range("B4:B7")]);
    assert_eq!(
        sheet.conditional_formats()[0].rules[0].condition,
        Condition::Expression {
            formula: "B4>$D$3".to_owned()
        }
    );
    book.delete_columns(0, 2, 1).unwrap();
    let sheet = &book.sheets()[0];
    assert_eq!(
        sheet.conditional_formats().len(),
        1,
        "a range all deleted is gone"
    );
    assert_eq!(
        sheet.conditional_formats()[0].rules[0].condition,
        Condition::Expression {
            formula: "B4>$C$3".to_owned()
        }
    );
    book.rename_sheet(1, "Voci").unwrap();
    assert_eq!(
        book.sheets()[0].validations()[0].formulas,
        vec!["Voci!$A$1:$A$3".to_owned()]
    );
    book.remove_sheet(1).unwrap();
    assert_eq!(
        book.sheets()[0].validations()[0].formulas,
        vec!["#REF!".to_owned()]
    );
    book.delete_rows(0, 3, 4).unwrap();
    assert_eq!(book.sheets()[0].validations(), &[] as &[DataValidation]);
    assert_eq!(
        book.sheets()[0].conditional_formats(),
        &[] as &[ConditionalFormat]
    );
}

#[test]
fn keeps_the_text_of_a_formula_no_edit_touches() {
    let mut sheet = Sheet {
        conditional_formats: vec![expression_format("A1", "_xlfn.STDEV.S(A1:A3)>1")],
        ..Sheet::default()
    };
    sheet.validations = vec![validation(
        ValidationKind::Custom,
        "A1",
        &["not a formula ("],
    )];
    rewrite_rule_formulas(&mut sheet, &mut Clone::clone);
    assert_eq!(
        sheet.conditional_formats[0].rules[0].condition,
        Condition::Expression {
            formula: "_xlfn.STDEV.S(A1:A3)>1".to_owned()
        }
    );
    assert_eq!(
        sheet.validations[0].formulas,
        vec!["not a formula (".to_owned()]
    );
    assert_eq!(anchor_of(&[]), CellRef::new(0, 0));
    assert_eq!(anchor_of(&[range("C3:D4"), range("A1")]), cell("C3"));
    assert_eq!(
        formula_in("A1+$B$1", cell("A1"), cell("B3")).map(|expression| print_formula(&expression)),
        Some("B3+$B$1".to_owned())
    );
}
