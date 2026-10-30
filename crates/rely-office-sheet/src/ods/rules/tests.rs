// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use rely_office_core::{ConditionalFormat, ScalePoint};

use super::*;

fn range(reference: &str) -> CellRange {
    CellRange::parse(reference).unwrap()
}

#[test]
fn writes_each_condition_as_libreoffice_states_it() {
    let value = |condition: Condition| condition_value(&condition, "B2");
    let compared = |operator, formulas: &[&str]| {
        value(Condition::CellIs {
            operator,
            formulas: formulas
                .iter()
                .map(|formula| (*formula).to_owned())
                .collect(),
        })
    };
    assert_eq!(
        compared(ComparisonOperator::Between, &["1", "$A$1"]).as_deref(),
        Some("between(1,[.$A$1])")
    );
    assert_eq!(
        compared(ComparisonOperator::NotBetween, &["1", "2"]).as_deref(),
        Some("not-between(1,2)")
    );
    assert_eq!(compared(ComparisonOperator::Between, &["1"]), None);
    for (operator, written) in [
        (ComparisonOperator::Equal, "=5"),
        (ComparisonOperator::NotEqual, "!=5"),
        (ComparisonOperator::Greater, ">5"),
        (ComparisonOperator::Less, "<5"),
        (ComparisonOperator::GreaterOrEqual, ">=5"),
        (ComparisonOperator::LessOrEqual, "<=5"),
    ] {
        assert_eq!(compared(operator, &["5"]).as_deref(), Some(written));
    }
    assert_eq!(
        value(Condition::Expression {
            formula: "MOD(ROW(),2)=0".to_owned()
        })
        .as_deref(),
        Some("formula-is(MOD(ROW();2)=0)")
    );
    for (rank, percent, bottom, written) in [
        (3, false, false, "top-elements(3)"),
        (3, true, true, "bottom-percent(3)"),
    ] {
        assert_eq!(
            value(Condition::Top {
                rank,
                percent,
                bottom
            })
            .as_deref(),
            Some(written)
        );
    }
    assert_eq!(
        value(Condition::Average {
            below: true,
            or_equal: true,
            deviations: Some(1)
        })
        .as_deref(),
        Some("below-equal-average")
    );
    assert_eq!(
        value(Condition::Text {
            operator: TextOperator::NotContains,
            text: "a\"b".to_owned()
        })
        .as_deref(),
        Some("not-contains-text(\"a\"\"b\")")
    );
    assert_eq!(
        value(Condition::Blanks { blank: false }).as_deref(),
        Some("formula-is(LEN(TRIM([.B2]))>0)")
    );
    assert_eq!(
        value(Condition::Duplicates { unique: true }).as_deref(),
        Some("unique")
    );
    assert_eq!(
        value(Condition::Errors { error: true }).as_deref(),
        Some("is-error")
    );
    assert_eq!(
        value(Condition::TimePeriod {
            period: "today".to_owned()
        }),
        None
    );
    assert_eq!(date_of("last7Days"), "last-7-days");
    assert_eq!(date_of("anything"), "this-month");
}

#[test]
fn writes_the_conditional_formats_of_a_sheet() {
    let look = DifferentialLook {
        bold: Some(true),
        ..DifferentialLook::default()
    };
    let sheet = Sheet {
        conditional_formats: vec![ConditionalFormat {
            ranges: vec![range("A1:A5"), range("C3")],
            rules: vec![
                ConditionalRule {
                    condition: Condition::ColorScale {
                        points: vec![
                            ScalePoint {
                                value: ScaleValue::Min,
                                colour: "F8696B".to_owned(),
                            },
                            ScalePoint {
                                value: ScaleValue::Formula("$Z$1".to_owned()),
                                colour: "63BE7B".to_owned(),
                            },
                        ],
                    },
                    look: None,
                    priority: 3,
                    stop_if_true: false,
                },
                ConditionalRule {
                    condition: Condition::DataBar {
                        min: ScaleValue::Percent("10".to_owned()),
                        max: ScaleValue::Percentile("90".to_owned()),
                        colour: "638EC6".to_owned(),
                        show_value: false,
                    },
                    look: None,
                    priority: 0,
                    stop_if_true: false,
                },
                ConditionalRule {
                    condition: Condition::IconSet {
                        icons: "3Arrows".to_owned(),
                        thresholds: vec![ScaleValue::Number("0".to_owned())],
                        reverse: false,
                        show_value: false,
                    },
                    look: None,
                    priority: 4,
                    stop_if_true: false,
                },
                ConditionalRule {
                    condition: Condition::TimePeriod {
                        period: "nextWeek".to_owned(),
                    },
                    look: Some(look.clone()),
                    priority: 2,
                    stop_if_true: false,
                },
                ConditionalRule {
                    condition: Condition::Duplicates { unique: false },
                    look: Some(look),
                    priority: 1,
                    stop_if_true: false,
                },
            ],
        }],
        ..Sheet::default()
    };
    let workbook = Workbook::from_sheets(vec![Sheet {
        name: "Due fogli".to_owned(),
        ..sheet.clone()
    }]);
    let styles = ConditionalStyles::new(&workbook);
    assert!(styles.xml().contains(r#"style:name="Condizione1""#));
    let xml = conditional_formats_xml("Due fogli", &sheet, &styles);
    assert!(xml.starts_with(
        r#"<calcext:conditional-formats><calcext:conditional-format calcext:target-range-address="&apos;Due fogli&apos;.A1:&apos;Due fogli&apos;.A5 &apos;Due fogli&apos;.C3"><calcext:condition calcext:apply-style-name="Condizione1" calcext:value="duplicate" calcext:base-cell-address="&apos;Due fogli&apos;.A1"/><calcext:date-is calcext:style="Condizione1" calcext:date="next-week"/><calcext:color-scale>"#
    ), "{xml}");
    for wanted in [
        r##"<calcext:color-scale-entry calcext:value="0" calcext:type="minimum" calcext:color="#f8696b"/>"##,
        r##"<calcext:color-scale-entry calcext:value="[.$Z$1]" calcext:type="formula" calcext:color="#63be7b"/>"##,
        r##"calcext:positive-color="#638ec6" calcext:axis-color="#000000" calcext:show-value="false"><calcext:formatting-entry calcext:value="10" calcext:type="percent"/><calcext:formatting-entry calcext:value="90" calcext:type="percentile"/></calcext:data-bar>"##,
        r#"<calcext:icon-set calcext:icon-set-type="3Arrows" calcext:show-value="false"><calcext:formatting-entry calcext:value="0" calcext:type="number"/></calcext:icon-set>"#,
    ] {
        assert!(xml.contains(wanted), "{wanted}\n{xml}");
    }
    assert!(
        xml.find("icon-set").unwrap() < xml.find("data-bar").unwrap(),
        "an unset priority comes last"
    );
    assert_eq!(conditional_formats_xml("S", &Sheet::default(), &styles), "");
    assert_eq!(
        styles.name(&ConditionalRule {
            condition: Condition::Errors { error: true },
            look: None,
            priority: 1,
            stop_if_true: false
        }),
        "Default"
    );
}

fn validation(
    kind: ValidationKind,
    operator: ComparisonOperator,
    formulas: &[&str],
) -> DataValidation {
    DataValidation {
        ranges: vec![range("B2:B9")],
        kind,
        operator,
        formulas: formulas
            .iter()
            .map(|formula| (*formula).to_owned())
            .collect(),
        allow_blank: false,
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

#[test]
fn writes_each_validation_condition() {
    let condition = |kind, operator, formulas: &[&str]| {
        validation_condition(&validation(kind, operator, formulas))
    };
    for (kind, operator, formulas, written) in [
        (
            ValidationKind::List,
            ComparisonOperator::Between,
            vec!["\"Sì, No,\"\"x\"\"\""],
            "of:cell-content-is-in-list(\"Sì\";\"No\";\"\"\"x\"\"\")",
        ),
        (
            ValidationKind::List,
            ComparisonOperator::Between,
            vec!["Elenco!$A$1:$A$3"],
            "of:cell-content-is-in-list([$Elenco.$A$1:.$A$3])",
        ),
        (
            ValidationKind::Whole,
            ComparisonOperator::Between,
            vec!["1", "20"],
            "of:cell-content-is-whole-number() and cell-content-is-between(1,20)",
        ),
        (
            ValidationKind::Decimal,
            ComparisonOperator::GreaterOrEqual,
            vec!["1.5"],
            "of:cell-content-is-decimal-number() and cell-content()>=1.5",
        ),
        (
            ValidationKind::Date,
            ComparisonOperator::NotBetween,
            vec!["A1", "2"],
            "of:cell-content-is-date() and cell-content-is-not-between([.A1],2)",
        ),
        (
            ValidationKind::Time,
            ComparisonOperator::Less,
            vec!["0.5"],
            "of:cell-content-is-time() and cell-content()<0.5",
        ),
        (
            ValidationKind::TextLength,
            ComparisonOperator::NotEqual,
            vec!["3"],
            "of:cell-content-text-length()!=3",
        ),
        (
            ValidationKind::Custom,
            ComparisonOperator::Between,
            vec!["B2>A2"],
            "of:is-true-formula([.B2]>[.A2])",
        ),
    ] {
        assert_eq!(
            condition(kind, operator, &formulas).as_deref(),
            Some(written)
        );
    }
    assert_eq!(
        condition(ValidationKind::Any, ComparisonOperator::Between, &[]),
        None
    );
    assert_eq!(
        condition(ValidationKind::Whole, ComparisonOperator::Between, &["1"]),
        None
    );
}

#[test]
fn writes_the_validations_of_a_workbook_and_names_them_in_cells() {
    let mut list = validation(
        ValidationKind::List,
        ComparisonOperator::Between,
        &["\"a,b\""],
    );
    list.dropdown = false;
    list.error_style = ErrorStyle::Information;
    list.error_title = Some("Titolo <1>".to_owned());
    list.error = Some("uno\ndue".to_owned());
    list.show_prompt = true;
    list.prompt = Some("scegli".to_owned());
    let mut later = validation(ValidationKind::Any, ComparisonOperator::Between, &[]);
    later.ranges = vec![range("B5")];
    later.error_style = ErrorStyle::Warning;
    let workbook = Workbook::from_sheets(vec![Sheet {
        name: "Fogli".to_owned(),
        validations: vec![list, later],
        ..Sheet::default()
    }]);
    let xml = content_validations_xml(&workbook);
    assert!(xml.starts_with(r#"<table:content-validations><table:content-validation table:name="val1_1" table:condition="of:cell-content-is-in-list(&quot;a&quot;;&quot;b&quot;)" table:allow-empty-cell="false" table:display-list="none" table:base-cell-address="Fogli.B2"><table:help-message table:display="true"><text:p>scegli</text:p></table:help-message><table:error-message table:message-type="information" table:title="Titolo &lt;1&gt;" table:display="true"><text:p>uno</text:p><text:p>due</text:p></table:error-message></table:content-validation>"#), "{xml}");
    assert!(xml.contains(r#"<table:content-validation table:name="val1_2" table:allow-empty-cell="false" table:base-cell-address="Fogli.B5"><table:error-message table:message-type="warning" table:display="true"></table:error-message>"#), "{xml}");
    let sheet = &workbook.sheets[0];
    assert_eq!(
        validation_name(0, sheet, CellRef::new(4, 1)).as_deref(),
        Some("val1_2"),
        "the last one listed"
    );
    assert_eq!(
        validation_name(0, sheet, CellRef::new(2, 1)).as_deref(),
        Some("val1_1")
    );
    assert_eq!(validation_name(0, sheet, CellRef::new(2, 2)), None);
    assert_eq!(content_validations_xml(&Workbook::new()), "");
    assert_eq!(sheet_address("Uno_2"), "Uno_2");
    assert_eq!(sheet_address("L'anno"), "'L''anno'");
}
