// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use rely_office_core::ComparisonOperator;

use super::*;
use crate::dates::serial_from_date;
use crate::value::Locale;

fn cell(address: &str) -> CellRef {
    CellRef::parse(address).unwrap()
}

fn book_with(column: &[&str]) -> Workbook {
    let mut book = Workbook::new();
    for (row, text) in column.iter().enumerate() {
        book.set_input(
            0,
            CellRef::new(u32::try_from(row).unwrap(), 0),
            text,
            Locale::English,
        )
        .unwrap();
    }
    book
}

fn red() -> DifferentialLook {
    DifferentialLook {
        fill: Some("FF0000".to_owned()),
        ..DifferentialLook::default()
    }
}

fn rule(condition: Condition, priority: u32) -> ConditionalRule {
    ConditionalRule {
        condition,
        look: Some(red()),
        priority,
        stop_if_true: false,
    }
}

fn format_of(ranges: &str, rules: Vec<ConditionalRule>) -> ConditionalFormat {
    ConditionalFormat {
        ranges: vec![CellRange::parse(ranges).unwrap()],
        rules,
    }
}

/// The rows of column A whose look the single rule changes.
fn marked(column: &[&str], condition: Condition) -> Vec<u32> {
    let mut book = book_with(column);
    let last = column.len();
    book.set_conditional_formats(
        0,
        vec![format_of(&format!("A1:A{last}"), vec![rule(condition, 1)])],
    )
    .unwrap();
    book.conditional_looks(0)
        .keys()
        .map(|cell| cell.row)
        .collect()
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "every kind of rule over the same values"
)]
fn marks_the_cells_each_kind_of_rule_holds_for() {
    let numbers = ["1", "5", "9", "3", "7"];
    assert_eq!(
        marked(
            &numbers,
            Condition::CellIs {
                operator: ComparisonOperator::Greater,
                formulas: vec!["$A$4".to_owned()]
            }
        ),
        vec![1, 2, 4]
    );
    assert_eq!(
        marked(
            &numbers,
            Condition::Expression {
                formula: "MOD(A1,3)=0".to_owned()
            }
        ),
        vec![2, 3]
    );
    assert_eq!(
        marked(
            &numbers,
            Condition::Top {
                rank: 2,
                percent: false,
                bottom: false
            }
        ),
        vec![2, 4]
    );
    assert_eq!(
        marked(
            &numbers,
            Condition::Top {
                rank: 40,
                percent: true,
                bottom: true
            }
        ),
        vec![0, 3]
    );
    assert_eq!(
        marked(
            &numbers,
            Condition::Average {
                below: false,
                or_equal: true,
                deviations: None
            }
        ),
        vec![1, 2, 4]
    );
    assert_eq!(
        marked(
            &numbers,
            Condition::Average {
                below: true,
                or_equal: false,
                deviations: Some(1)
            }
        ),
        vec![0]
    );
    let words = ["Roma", "rovigo", "Bari", "", "Roma", "=1/0"];
    assert_eq!(
        marked(
            &words,
            Condition::Text {
                operator: TextOperator::BeginsWith,
                text: "ro".to_owned()
            }
        ),
        vec![0, 1, 4]
    );
    assert_eq!(
        marked(
            &words,
            Condition::Text {
                operator: TextOperator::EndsWith,
                text: "I".to_owned()
            }
        ),
        vec![2]
    );
    assert_eq!(
        marked(
            &words,
            Condition::Text {
                operator: TextOperator::Contains,
                text: "ovi".to_owned()
            }
        ),
        vec![1]
    );
    assert_eq!(
        marked(
            &words,
            Condition::Text {
                operator: TextOperator::NotContains,
                text: "o".to_owned()
            }
        ),
        vec![2, 3]
    );
    assert_eq!(
        marked(&words, Condition::Duplicates { unique: false }),
        vec![0, 4]
    );
    assert_eq!(
        marked(&words, Condition::Duplicates { unique: true }),
        vec![1, 2, 5]
    );
    assert_eq!(marked(&words, Condition::Blanks { blank: true }), vec![3]);
    assert_eq!(
        marked(&words, Condition::Blanks { blank: false }),
        vec![0, 1, 2, 4, 5]
    );
    assert_eq!(marked(&words, Condition::Errors { error: true }), vec![5]);
}

#[test]
fn shades_scales_bars_and_icons_along_the_values() {
    let mut book = book_with(&["0", "5", "10", "testo"]);
    book.set_conditional_formats(
        0,
        vec![
            format_of(
                "A1:A4",
                vec![ConditionalRule {
                    condition: Condition::ColorScale {
                        points: vec![
                            ScalePoint {
                                value: ScaleValue::Min,
                                colour: "000000".to_owned(),
                            },
                            ScalePoint {
                                value: ScaleValue::Max,
                                colour: "FFFFFF".to_owned(),
                            },
                        ],
                    },
                    look: None,
                    priority: 1,
                    stop_if_true: false,
                }],
            ),
            format_of(
                "A1:A4",
                vec![
                    ConditionalRule {
                        condition: Condition::DataBar {
                            min: ScaleValue::Number("0".to_owned()),
                            max: ScaleValue::Percentile("100".to_owned()),
                            colour: "638EC6".to_owned(),
                            show_value: false,
                        },
                        look: None,
                        priority: 2,
                        stop_if_true: false,
                    },
                    ConditionalRule {
                        condition: Condition::IconSet {
                            icons: "3Arrows".to_owned(),
                            thresholds: vec![
                                ScaleValue::Percent("0".to_owned()),
                                ScaleValue::Percent("33".to_owned()),
                                ScaleValue::Percent("67".to_owned()),
                            ],
                            reverse: true,
                            show_value: true,
                        },
                        look: None,
                        priority: 3,
                        stop_if_true: false,
                    },
                ],
            ),
        ],
    )
    .unwrap();
    let looks = book.conditional_looks(0);
    assert_eq!(looks[&cell("A1")].fill.as_deref(), Some("000000"));
    assert_eq!(looks[&cell("A2")].fill.as_deref(), Some("808080"));
    assert_eq!(looks[&cell("A3")].fill.as_deref(), Some("FFFFFF"));
    assert!(!looks.contains_key(&cell("A4")), "text is left out");
    let bar = looks[&cell("A2")].bar.clone().unwrap();
    assert!((bar.length - 0.5).abs() < 1e-9);
    assert!(!bar.show_value);
    assert!((looks[&cell("A1")].bar.as_ref().unwrap().length - 0.1).abs() < 1e-9);
    let icons: Vec<usize> = ["A1", "A2", "A3"]
        .iter()
        .map(|address| looks[&cell(address)].icon.as_ref().unwrap().index)
        .collect();
    assert_eq!(icons, vec![2, 1, 0], "reversed");
}

#[test]
fn gives_the_earlier_rule_each_property_and_stops_where_asked() {
    let mut book = book_with(&["5", "50"]);
    let bold = DifferentialLook {
        bold: Some(true),
        fill: Some("00FF00".to_owned()),
        ..DifferentialLook::default()
    };
    book.set_conditional_formats(
        0,
        vec![format_of(
            "A1:A2",
            vec![
                ConditionalRule {
                    condition: Condition::Expression {
                        formula: "TRUE".to_owned(),
                    },
                    look: Some(bold),
                    priority: 9,
                    stop_if_true: false,
                },
                ConditionalRule {
                    condition: Condition::CellIs {
                        operator: ComparisonOperator::Greater,
                        formulas: vec!["10".to_owned()],
                    },
                    look: Some(red()),
                    priority: 1,
                    stop_if_true: true,
                },
            ],
        )],
    )
    .unwrap();
    let looks = book.conditional_looks(0);
    assert_eq!(looks[&cell("A2")].fill.as_deref(), Some("FF0000"));
    assert_eq!(
        looks[&cell("A2")].bold,
        None,
        "stopped after the first rule"
    );
    assert_eq!(looks[&cell("A1")].fill.as_deref(), Some("00FF00"));
    assert_eq!(looks[&cell("A1")].bold, Some(true));
    assert!(book.conditional_looks(4).is_empty());
}

#[test]
fn reads_dates_in_their_period_from_today() {
    let today = serial_from_date(2026, 10, 27).unwrap();
    for (period, date, expected) in [
        ("today", today, true),
        ("yesterday", today - 1.0, true),
        ("tomorrow", today, false),
        ("last7Days", today - 6.0, true),
        ("last7Days", today - 7.0, false),
        // 27 October 2026 is a Tuesday: its week runs from Sunday the 25th.
        ("thisWeek", today - 2.0, true),
        ("thisWeek", today - 3.0, false),
        ("lastWeek", today - 3.0, true),
        ("nextWeek", today + 5.0, true),
        ("thisMonth", today - 26.0, true),
        ("lastMonth", today - 27.0, true),
        ("nextMonth", today + 5.0, true),
        ("someday", today, false),
    ] {
        assert_eq!(in_period(period, date, today), expected, "{period} {date}");
    }
    let mut book = book_with(&["1"]);
    book.set_input(0, cell("A1"), &format!("{today}"), Locale::English)
        .unwrap();
    book.set_conditional_formats(
        0,
        vec![format_of(
            "A1",
            vec![rule(
                Condition::TimePeriod {
                    period: "today".to_owned(),
                },
                1,
            )],
        )],
    )
    .unwrap();
    assert!(book.conditional_looks(0).is_empty(), "no today, no period");
    book.set_today(Some(today));
    assert_eq!(book.conditional_looks(0).len(), 1);
}

#[test]
fn blends_and_interpolates() {
    assert_eq!(blend("000000", "FF8040", 0.5), "804020");
    assert_eq!(percentile(&[1.0, 2.0, 3.0, 4.0], 0.5), 2.5);
    assert_eq!(percentile(&[7.0], 0.9), 7.0);
    assert_eq!(
        count_key(&Value::Text("A".to_owned())),
        count_key(&Value::Text("a".to_owned()))
    );
    assert_eq!(shown_text(&Value::Bool(true)), "TRUE");
    assert_eq!(
        cells_of(&[CellRange::parse("A1:Z99").unwrap()], (0, 0)),
        Vec::new()
    );
    assert_eq!(
        cells_of(
            &[
                CellRange::parse("A1:B2").unwrap(),
                CellRange::parse("B2:C2").unwrap()
            ],
            (2, 3)
        )
        .len(),
        5,
        "each cell once, within the used area"
    );
}
