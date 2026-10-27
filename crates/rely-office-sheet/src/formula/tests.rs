// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;

fn round_trip(formula: &str) -> String {
    print_formula(&parse_formula(formula).unwrap())
}

#[test]
fn prints_back_what_it_parses() {
    for formula in [
        "SUM(A1:B2)",
        "A1+B1*C1",
        "(A1+B1)*C1",
        "-A1^2",
        "A1&\" \"&B1",
        "IF(A1>=10,\"alto\",\"basso\")",
        "$A$1+A$2+$B3",
        "Foglio2!A1",
        "'Vendite 2026'!B2:C9",
        "'O''Brien'!A1",
        "SUM(A:C)",
        "SUM($A:$A)",
        "10%",
        "0.00000015",
        "A1<>B1",
        "IF(A1,,2)",
        "TRUE",
        "#N/A",
        "NOW()",
    ] {
        assert_eq!(round_trip(formula), formula, "{formula}");
    }
    assert_eq!(round_trip("=sum( a1 , b2 )"), "SUM(A1,B2)");
}

#[test]
fn reads_formulas_typed_in_italian() {
    assert_eq!(round_trip("SOMMA(A1;B1)"), "SUM(A1,B1)");
    assert_eq!(
        round_trip("SE(A1>1,5;\"sì; no\";FALSO)"),
        "IF(A1>1.5,\"sì; no\",FALSE)"
    );
    assert_eq!(
        round_trip("CERCA.VERT(A1;B:C;2;FALSO)"),
        "VLOOKUP(A1,B:C,2,FALSE)"
    );
    assert_eq!(round_trip("ARROTONDA(1,25;1)"), "ROUND(1.25,1)");
}

#[test]
fn respects_precedence_and_associativity() {
    let parsed = parse_formula("1-2-3").unwrap();
    assert!(
        matches!(parsed, Expr::Binary(BinaryOperator::Subtract, ref left, _) if matches!(**left, Expr::Binary(BinaryOperator::Subtract, _, _)))
    );
    let parsed = parse_formula("1+2*3").unwrap();
    assert!(
        matches!(parsed, Expr::Binary(BinaryOperator::Add, _, ref right) if matches!(**right, Expr::Binary(BinaryOperator::Multiply, _, _)))
    );
    let parsed = parse_formula("-2^2").unwrap();
    assert!(
        matches!(parsed, Expr::Binary(BinaryOperator::Power, ref left, _) if matches!(**left, Expr::Unary(UnaryOperator::Minus, _)))
    );
    let parsed = parse_formula("A1=1&2").unwrap();
    assert!(matches!(parsed, Expr::Binary(BinaryOperator::Equal, _, _)));
}

#[test]
fn reads_references() {
    let Expr::Range(range) = parse_formula("'Q1'!$B$7:A2").unwrap() else {
        panic!("not a range");
    };
    assert_eq!(range.sheet.as_deref(), Some("Q1"));
    assert!(range.start.absolute_row && range.start.absolute_column);
    assert_eq!(range.bounds(), (CellRef::new(1, 0), CellRef::new(6, 1)));
    let Expr::Range(columns) = parse_formula("B:D").unwrap() else {
        panic!("not a range");
    };
    assert!(columns.whole_columns);
    assert_eq!(
        columns.bounds(),
        (CellRef::new(0, 1), CellRef::new(MAX_ROWS - 1, 3))
    );
    assert_eq!(
        parse_formula("NOMESCONOSCIUTO").unwrap(),
        Expr::Error(ErrorKind::Name)
    );
}

#[test]
fn refuses_what_is_not_a_formula() {
    for wrong in [
        "SUM(A1",
        "1+",
        "\"open",
        "'Sheet",
        "'Sheet'A1",
        "A1:",
        "A1:1",
        "#OOPS",
        "1A",
        ")",
        "A1 B1",
        "SUM(A1 B1)",
        "Foglio!",
        "@A1",
    ] {
        assert!(parse_formula(wrong).is_err(), "{wrong}");
    }
    let deep = format!("{}1{}", "(".repeat(200), ")".repeat(200));
    assert!(parse_formula(&deep).is_err());
    let long = format!("1{}", "+1".repeat(MAX_FORMULA_CHARS));
    assert!(parse_formula(&long).is_err());
}

#[test]
fn writes_numbers_that_read_back() {
    for number in [0.0, 1.0, -3.0, 0.1, 1.0 / 3.0, 1e20, 1e300, 2.5e-10] {
        let text = number_text(number);
        assert_eq!(text.parse::<f64>().unwrap(), number, "{text}");
    }
    assert_eq!(number_text(12.0), "12");
}
