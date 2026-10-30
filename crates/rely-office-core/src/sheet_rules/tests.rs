// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::sheet_styles::default_theme;

fn formats(xml: &str) -> Vec<ReadFormat> {
    let mut reader = Reader::from_str(xml);
    let mut found = Vec::new();
    loop {
        match reader.read_event().unwrap() {
            Event::Start(element) if element.local_name().as_ref() == "conditionalFormatting" => {
                let element = element.into_owned();
                if let Some(read) =
                    read_conditional_formatting(&mut reader, &element, &default_theme())
                {
                    found.push(read);
                }
            }
            Event::Eof => return found,
            _ => {}
        }
    }
}

fn validations(xml: &str) -> Vec<DataValidation> {
    let mut reader = Reader::from_str(xml);
    let mut found = Vec::new();
    loop {
        let event = reader.read_event().unwrap();
        match &event {
            Event::Start(element) | Event::Empty(element)
                if element.local_name().as_ref() == "dataValidation" =>
            {
                let element = element.clone().into_owned();
                let empty = matches!(event, Event::Empty(_));
                found.extend(read_data_validation(&mut reader, &element, empty));
            }
            Event::Eof => return found,
            _ => {}
        }
    }
}

#[test]
#[expect(clippy::too_many_lines, reason = "one format with every kind of rule")]
fn reads_every_kind_of_rule_with_its_differential_format() {
    let read = formats(concat!(
        r#"<conditionalFormatting sqref="A1:A10 C2"><cfRule type="cellIs" dxfId="2" priority="1" operator="between"><formula>1</formula><formula>&quot;x&quot;</formula></cfRule>"#,
        r#"<cfRule type="expression" priority="2" stopIfTrue="1"><formula>MOD(ROW(),2)=0</formula></cfRule>"#,
        r#"<cfRule type="colorScale" priority="3"><colorScale><cfvo type="min"/><cfvo type="percentile" val="50"/><cfvo type="max"/><color rgb="FFF8696B"/><color rgb="FFFFEB84"/><color theme="9"/></colorScale></cfRule>"#,
        r#"<cfRule type="dataBar" priority="4"><dataBar showValue="0"><cfvo type="num" val="0"/><cfvo type="max"/><color rgb="FF638EC6"/></dataBar></cfRule>"#,
        r#"<cfRule type="iconSet" priority="5"><iconSet iconSet="3Arrows" reverse="1"><cfvo type="percent" val="0"/><cfvo type="percent" val="33"/><cfvo type="percent" val="67"/></iconSet></cfRule>"#,
        r#"<cfRule type="top10" priority="6" rank="3" bottom="1" percent="1"/><cfRule type="aboveAverage" aboveAverage="0" equalAverage="1" stdDev="2" priority="7"/>"#,
        r#"<cfRule type="beginsWith" operator="beginsWith" text="Ro" priority="8"><formula>LEFT(A1,2)="Ro"</formula></cfRule>"#,
        r#"<cfRule type="uniqueValues" priority="9"/><cfRule type="containsBlanks" priority="10"/><cfRule type="notContainsErrors" priority="11"/>"#,
        r#"<cfRule type="timePeriod" timePeriod="last7Days" priority="12"/><cfRule type="mystery" priority="13"/><cfRule type="cellIs" operator="equal" priority="14"/>"#,
        "</conditionalFormatting>",
        r#"<conditionalFormatting sqref="B:B"><cfRule type="duplicateValues" dxfId="0" priority="15"/></conditionalFormatting>"#,
        r#"<conditionalFormatting sqref=""><cfRule type="duplicateValues" priority="16"/></conditionalFormatting>"#,
    ));
    assert_eq!(read.len(), 2, "a format with no range is dropped");
    let first = &read[0].format;
    assert_eq!(
        first.ranges,
        vec![
            CellRange::parse("A1:A10").unwrap(),
            CellRange::parse("C2").unwrap()
        ]
    );
    let conditions: Vec<&Condition> = first.rules.iter().map(|rule| &rule.condition).collect();
    assert_eq!(
        conditions.len(),
        12,
        "an unknown rule and a cellIs with no formula are dropped"
    );
    assert_eq!(
        conditions[0],
        &Condition::CellIs {
            operator: ComparisonOperator::Between,
            formulas: vec!["1".to_owned(), "\"x\"".to_owned()]
        }
    );
    assert_eq!(read[0].looks[0], Some(2));
    assert!(first.rules[1].stop_if_true);
    assert_eq!(first.rules[1].priority, 2);
    let Condition::ColorScale { points } = conditions[2] else {
        panic!("{:?}", conditions[2]);
    };
    assert_eq!(points[1].value, ScaleValue::Percentile("50".to_owned()));
    assert_eq!(points[0].colour, "F8696B");
    assert_eq!(points[2].colour, default_theme()[9]);
    assert_eq!(
        conditions[3],
        &Condition::DataBar {
            min: ScaleValue::Number("0".to_owned()),
            max: ScaleValue::Max,
            colour: "638EC6".to_owned(),
            show_value: false
        }
    );
    let Condition::IconSet {
        icons,
        thresholds,
        reverse,
        show_value,
    } = conditions[4]
    else {
        panic!();
    };
    assert_eq!(
        (icons.as_str(), thresholds.len(), *reverse, *show_value),
        ("3Arrows", 3, true, true)
    );
    assert_eq!(
        conditions[5],
        &Condition::Top {
            rank: 3,
            percent: true,
            bottom: true
        }
    );
    assert_eq!(
        conditions[6],
        &Condition::Average {
            below: true,
            or_equal: true,
            deviations: Some(2)
        }
    );
    assert_eq!(
        conditions[7],
        &Condition::Text {
            operator: TextOperator::BeginsWith,
            text: "Ro".to_owned()
        }
    );
    assert_eq!(conditions[8], &Condition::Duplicates { unique: true });
    assert_eq!(conditions[9], &Condition::Blanks { blank: true });
    assert_eq!(conditions[10], &Condition::Errors { error: false });
    assert_eq!(
        conditions[11],
        &Condition::TimePeriod {
            period: "last7Days".to_owned()
        }
    );
    assert_eq!(
        read[1].format.ranges[0].last_row, LAST_ROW,
        "a whole column"
    );
}

#[test]
fn reads_validations_of_the_sheet_and_of_the_extension() {
    let found = validations(concat!(
        r#"<dataValidations count="3"><dataValidation type="list" allowBlank="1" showInputMessage="1" showErrorMessage="1" errorStyle="warning" errorTitle="No" error="Scegli" promptTitle="" prompt="Uno dei due" sqref="A1:A5 C1"><formula1>&quot;Sì,No&quot;</formula1></dataValidation>"#,
        r#"<dataValidation type="whole" operator="greaterThan" showDropDown="1" sqref="B2"><formula1>0</formula1><formula2>9</formula2></dataValidation>"#,
        r#"<dataValidation type="decimal" sqref="D1"><formula1>1</formula1><formula2>9</formula2></dataValidation>"#,
        r#"<dataValidation sqref="E1"/><dataValidation type="list" sqref=""/></dataValidations>"#,
        r#"<x14:dataValidations><x14:dataValidation type="list"><x14:formula1><xm:f>Elenco!$A$1:$A$3</xm:f></x14:formula1><xm:sqref>F1:F9</xm:sqref></x14:dataValidation></x14:dataValidations>"#,
    ));
    assert_eq!(found.len(), 5);
    let list = &found[0];
    assert_eq!(list.kind, ValidationKind::List);
    assert_eq!(list.formulas, vec!["\"Sì,No\"".to_owned()]);
    assert_eq!(list.ranges.len(), 2);
    assert!(list.allow_blank && list.dropdown && list.show_error && list.show_prompt);
    assert_eq!(list.error_style, ErrorStyle::Warning);
    assert_eq!(
        (list.error_title.as_deref(), list.prompt_title.as_deref()),
        (Some("No"), None)
    );
    let whole = &found[1];
    assert_eq!(whole.operator, ComparisonOperator::Greater);
    assert!(!whole.dropdown, "showDropDown hides the list");
    assert_eq!(whole.formulas.len(), 2, "kept as read");
    assert_eq!(found[2].operator, ComparisonOperator::Between);
    assert_eq!(found[3].kind, ValidationKind::Any);
    let extension = &found[4];
    assert_eq!(extension.formulas, vec!["Elenco!$A$1:$A$3".to_owned()]);
    assert_eq!(extension.ranges, vec![CellRange::parse("F1:F9").unwrap()]);
}

#[test]
fn parses_and_writes_references() {
    assert_eq!(
        parse_ranges("$A$1:$B$2 3:4 C:A Foglio!D4 nonsense"),
        vec![
            CellRange::parse("A1:B2").unwrap(),
            CellRange {
                first_row: 2,
                first_column: 0,
                last_row: 3,
                last_column: LAST_COLUMN
            },
            CellRange {
                first_row: 0,
                first_column: 0,
                last_row: LAST_ROW,
                last_column: 2
            },
            CellRange::parse("D4").unwrap(),
        ]
    );
    assert_eq!(
        range_reference(&CellRange::parse("AA10:AB12").unwrap()),
        "AA10:AB12"
    );
    assert_eq!(range_reference(&CellRange::parse("C3").unwrap()), "C3");
    assert_eq!(column_letters(16_383), "XFD");
}

#[test]
fn names_operators_both_ways() {
    for operator in [
        ComparisonOperator::Between,
        ComparisonOperator::NotBetween,
        ComparisonOperator::Equal,
        ComparisonOperator::NotEqual,
        ComparisonOperator::Greater,
        ComparisonOperator::Less,
        ComparisonOperator::GreaterOrEqual,
        ComparisonOperator::LessOrEqual,
    ] {
        assert_eq!(
            ComparisonOperator::from_name(operator.name()),
            Some(operator)
        );
    }
    assert_eq!(ComparisonOperator::from_name("above"), None);
    for kind in [
        ValidationKind::Any,
        ValidationKind::Whole,
        ValidationKind::Decimal,
        ValidationKind::List,
        ValidationKind::Date,
        ValidationKind::Time,
        ValidationKind::TextLength,
        ValidationKind::Custom,
    ] {
        assert_eq!(ValidationKind::from_name(kind.name()), kind);
    }
    assert!(DifferentialLook::default().is_empty());
}
