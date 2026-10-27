// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;
use crate::xml_tree::parse_tree;

fn element(xml: &str) -> Element {
    parse_tree(
        &format!(
            concat!(
                r##"<root xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" "##,
                r##"xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" "##,
                r##"xmlns:calcext="urn:org:documentfoundation:names:experimental:calc:xmlns:calcext:1.0">{}</root>"##
            ),
            xml
        ),
        |_, _| false,
    )
    .unwrap()
    .elements()
    .next()
    .unwrap()
    .clone()
}

#[test]
fn turns_open_formulas_into_excel_notation() {
    for (odf, excel) in [
        ("of:=SUM([.A1:.B2];1)", "SUM(A1:B2,1)"),
        ("of:=[$Uno.A3]+[$'Due Fogli'.B1]", "Uno!A3+'Due Fogli'!B1"),
        (
            "of:=IF([.$A$1]>1;\"a;b\";\"c\"\"d\")",
            "IF($A$1>1,\"a;b\",\"c\"\"d\")",
        ),
        ("of:=COM.MICROSOFT.IFS([.A1];1)", "IFS(A1,1)"),
        ("=[Foglio.A1:Foglio.B2]", "Foglio!A1:B2"),
        ("oooc:=[.A:.A]", "A:A"),
        ("1.5E3+TRUE()", "1.5E3+TRUE()"),
        ("[$'Ra]ro'.A1]", "'Ra]ro'!A1"),
        ("[$Two Words.A1]", "'Two Words'!A1"),
        ("[.A1", "A"),
    ] {
        assert_eq!(excel_formula_from_odf(odf), excel, "{odf}");
    }
    assert_eq!(
        odf_ranges("Foglio1.A1:Foglio1.B2 'Due fogli'.C3 $S.$D$4 bad"),
        vec![
            CellRange::parse("A1:B2").unwrap(),
            CellRange::parse("C3").unwrap(),
            CellRange::parse("D4").unwrap()
        ]
    );
}

#[test]
#[expect(clippy::too_many_lines, reason = "one format with every condition")]
fn reads_every_condition_libreoffice_writes() {
    let table = element(concat!(
        r##"<table:table><calcext:conditional-formats><calcext:conditional-format calcext:target-range-address="S.A1:S.A5 S.C1">"##,
        r##"<calcext:condition calcext:apply-style-name="Rosso" calcext:value="&gt;=[.$B$1]"/>"##,
        r##"<calcext:condition calcext:apply-style-name="Nulla" calcext:value="between(1;[.B1])"/>"##,
        r##"<calcext:condition calcext:value="not-between(1,2)"/><calcext:condition calcext:value="formula-is(AND([.A1]&gt;1;TRUE()))"/>"##,
        r##"<calcext:condition calcext:value="top-elements(3)"/><calcext:condition calcext:value="bottom-percent(20)"/>"##,
        r##"<calcext:condition calcext:value="below-equal-average"/><calcext:condition calcext:value="above-average"/>"##,
        r##"<calcext:condition calcext:value="unique"/><calcext:condition calcext:value="duplicate"/>"##,
        r##"<calcext:condition calcext:value="begins-with(&quot;R&quot;&quot;o&quot;)"/><calcext:condition calcext:value="not-contains-text(&quot;x&quot;)"/>"##,
        r##"<calcext:condition calcext:value="is-no-error"/><calcext:condition calcext:value="!=0"/><calcext:condition calcext:value="mystery"/>"##,
        r##"<calcext:date-is calcext:date="last-7-days" calcext:style="Rosso"/><calcext:date-is calcext:date="this-year"/>"##,
        "</calcext:conditional-format>",
        r##"<calcext:conditional-format calcext:target-range-address="S.B1:S.B9"><calcext:color-scale><calcext:color-scale-entry calcext:value="0" calcext:type="minimum" calcext:color="#f8696b"/>"##,
        r##"<calcext:color-scale-entry calcext:value="50" calcext:type="percentile" calcext:color="#ffeb84"/><calcext:color-scale-entry calcext:value="[.$Z$1]" calcext:type="formula" calcext:color="#63be7b"/></calcext:color-scale>"##,
        r##"<calcext:data-bar calcext:positive-color="#638ec6" calcext:show-value="false"><calcext:formatting-entry calcext:value="0" calcext:type="auto-minimum"/><calcext:formatting-entry calcext:value="7" calcext:type="number"/></calcext:data-bar>"##,
        r##"<calcext:icon-set calcext:icon-set-type="3Arrows"><calcext:formatting-entry calcext:value="0" calcext:type="percent"/><calcext:formatting-entry calcext:value="33" calcext:type="percent"/></calcext:icon-set>"##,
        "</calcext:conditional-format>",
        r##"<calcext:conditional-format calcext:target-range-address=""><calcext:condition calcext:value="unique"/></calcext:conditional-format>"##,
        "</calcext:conditional-formats></table:table>"
    ));
    let red = DifferentialLook {
        fill: Some("FF0000".to_owned()),
        ..DifferentialLook::default()
    };
    let formats = conditional_formats(&table, &|name| (name == "Rosso").then(|| red.clone()));
    assert_eq!(formats.len(), 2, "a format with no range is dropped");
    assert_eq!(formats[0].ranges.len(), 2);
    let conditions: Vec<&Condition> = formats[0]
        .rules
        .iter()
        .map(|rule| &rule.condition)
        .collect();
    assert_eq!(
        conditions,
        vec![
            &Condition::CellIs {
                operator: ComparisonOperator::GreaterOrEqual,
                formulas: vec!["$B$1".to_owned()]
            },
            &Condition::CellIs {
                operator: ComparisonOperator::Between,
                formulas: vec!["1".to_owned(), "B1".to_owned()]
            },
            &Condition::CellIs {
                operator: ComparisonOperator::NotBetween,
                formulas: vec!["1".to_owned(), "2".to_owned()]
            },
            &Condition::Expression {
                formula: "AND(A1>1,TRUE())".to_owned()
            },
            &Condition::Top {
                rank: 3,
                percent: false,
                bottom: false
            },
            &Condition::Top {
                rank: 20,
                percent: true,
                bottom: true
            },
            &Condition::Average {
                below: true,
                or_equal: true,
                deviations: None
            },
            &Condition::Average {
                below: false,
                or_equal: false,
                deviations: None
            },
            &Condition::Duplicates { unique: true },
            &Condition::Duplicates { unique: false },
            &Condition::Text {
                operator: TextOperator::BeginsWith,
                text: "R\"o".to_owned()
            },
            &Condition::Text {
                operator: TextOperator::NotContains,
                text: "x".to_owned()
            },
            &Condition::Errors { error: false },
            &Condition::CellIs {
                operator: ComparisonOperator::NotEqual,
                formulas: vec!["0".to_owned()]
            },
            &Condition::TimePeriod {
                period: "last7Days".to_owned()
            },
        ]
    );
    assert_eq!(formats[0].rules[0].look, Some(red.clone()));
    assert_eq!(formats[0].rules[1].look, None);
    assert_eq!(formats[0].rules[14].look, Some(red));
    let priorities: Vec<u32> = formats
        .iter()
        .flat_map(|format| format.rules.iter().map(|rule| rule.priority))
        .collect();
    assert_eq!(priorities, (1..=18).collect::<Vec<_>>());
    let Condition::ColorScale { points } = &formats[1].rules[0].condition else {
        panic!();
    };
    assert_eq!(points[0].colour, "F8696B");
    assert_eq!(points[2].value, ScaleValue::Formula("$Z$1".to_owned()));
    assert_eq!(
        formats[1].rules[1].condition,
        Condition::DataBar {
            min: ScaleValue::Min,
            max: ScaleValue::Number("7".to_owned()),
            colour: "638EC6".to_owned(),
            show_value: false
        }
    );
    assert!(matches!(
        &formats[1].rules[2].condition,
        Condition::IconSet { icons, thresholds, .. } if icons == "3Arrows" && thresholds.len() == 2
    ));
    assert_eq!(
        conditional_formats(&element("<table:table/>"), &|_| None),
        Vec::new()
    );
}

#[test]
#[expect(clippy::too_many_lines, reason = "every condition a validation states")]
fn reads_every_content_validation_libreoffice_writes() {
    let read = |condition: &str| {
        content_validation(&element(&format!(
            r#"<table:content-validation table:name="v" table:condition="{condition}" table:allow-empty-cell="false"/>"#
        )))
        .unwrap()
        .1
    };
    let list =
        read("of:cell-content-is-in-list(&quot;Sì&quot;;&quot;No &quot;&quot;x&quot;&quot;&quot;)");
    assert_eq!(
        (
            list.kind,
            list.formulas.clone(),
            list.allow_blank,
            list.dropdown
        ),
        (
            ValidationKind::List,
            vec!["\"Sì,No \"\"x\"\"\"".to_owned()],
            false,
            true
        )
    );
    assert_eq!(
        read("of:cell-content-is-in-list([$Elenco.$A$1:.$A$3])").formulas,
        vec!["Elenco!$A$1:$A$3".to_owned()]
    );
    for (condition, kind, operator, formulas) in [
        (
            "of:cell-content-is-whole-number() and cell-content-is-between(1,20)",
            ValidationKind::Whole,
            ComparisonOperator::Between,
            vec!["1", "20"],
        ),
        (
            "of:cell-content-is-decimal-number() and cell-content()&gt;=1.5",
            ValidationKind::Decimal,
            ComparisonOperator::GreaterOrEqual,
            vec!["1.5"],
        ),
        (
            "of:cell-content-is-date() and cell-content-is-not-between([.A1];2)",
            ValidationKind::Date,
            ComparisonOperator::NotBetween,
            vec!["A1", "2"],
        ),
        (
            "of:cell-content-is-time() and cell-content()&lt;0.5",
            ValidationKind::Time,
            ComparisonOperator::Less,
            vec!["0.5"],
        ),
        (
            "of:cell-content-text-length-is-between(2,5)",
            ValidationKind::TextLength,
            ComparisonOperator::Between,
            vec!["2", "5"],
        ),
        (
            "of:cell-content-text-length()!=3",
            ValidationKind::TextLength,
            ComparisonOperator::NotEqual,
            vec!["3"],
        ),
        (
            "of:is-true-formula([.M1]&gt;[.A1])",
            ValidationKind::Custom,
            ComparisonOperator::Between,
            vec!["M1>A1"],
        ),
        ("", ValidationKind::Any, ComparisonOperator::Between, vec![]),
    ] {
        let validation = read(condition);
        assert_eq!(validation.kind, kind, "{condition}");
        assert_eq!(validation.operator, operator, "{condition}");
        assert_eq!(validation.formulas, formulas, "{condition}");
    }
    let (name, messages) = content_validation(&element(concat!(
        r##"<table:content-validation table:name="val3" table:display-list="none">"##,
        r##"<table:help-message table:title="Titolo" table:display="true"><text:p>Riga uno</text:p><text:p>riga due</text:p></table:help-message>"##,
        r##"<table:error-message table:message-type="warning" table:display="false" table:title=""/></table:content-validation>"##
    )))
    .unwrap();
    assert_eq!(name, "val3");
    assert!(
        messages.allow_blank && !messages.dropdown && messages.show_prompt && !messages.show_error
    );
    assert_eq!(messages.prompt_title.as_deref(), Some("Titolo"));
    assert_eq!(messages.prompt.as_deref(), Some("Riga uno\nriga due"));
    assert_eq!(
        (messages.error_style, messages.error_title),
        (ErrorStyle::Warning, None)
    );
    assert!(content_validation(&element("<table:content-validation/>")).is_none());
    let mut ranges = Vec::new();
    for row in 0..3 {
        add_validation_cells(&mut ranges, row, 2, 4);
    }
    add_validation_cells(&mut ranges, 5, 2, 4);
    add_validation_cells(&mut ranges, 6, 1, 4);
    assert_eq!(
        ranges,
        vec![
            CellRange::parse("C1:E3").unwrap(),
            CellRange::parse("C6:E6").unwrap(),
            CellRange::parse("B7:E7").unwrap()
        ]
    );
}
