// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use std::io::{Cursor, Read, Write as _};

use rely_office_core::{
    CellRange, ComparisonOperator, ConditionalFormat, Limits, ScalePoint, read_xlsx_layouts,
};

use super::*;
use crate::value::Locale;
use crate::xlsx::{read_spreadsheet, write_xlsx};

fn range(reference: &str) -> CellRange {
    CellRange::parse(reference).unwrap()
}

fn rule(condition: Condition, look: Option<DifferentialLook>, priority: u32) -> ConditionalRule {
    ConditionalRule {
        condition,
        look,
        priority,
        stop_if_true: priority == 2,
    }
}

#[expect(clippy::too_many_lines, reason = "one rule of every kind")]
fn every_rule() -> Vec<ConditionalRule> {
    let look = DifferentialLook {
        fill: Some("FFC7CE".to_owned()),
        font_colour: Some("9C0006".to_owned()),
        bold: Some(true),
        italic: Some(false),
        underline: Some(false),
        strike: Some(true),
        number_format: Some("0.0%".to_owned()),
    };
    let plain = DifferentialLook {
        underline: Some(true),
        ..DifferentialLook::default()
    };
    let conditions = vec![
        Condition::CellIs {
            operator: ComparisonOperator::NotBetween,
            formulas: vec!["1".to_owned(), "\"a<b\"".to_owned()],
        },
        Condition::Expression {
            formula: "AND($A1>0,B1<>\"\")".to_owned(),
        },
        Condition::Top {
            rank: 5,
            percent: true,
            bottom: true,
        },
        Condition::Average {
            below: true,
            or_equal: true,
            deviations: Some(2),
        },
        Condition::Text {
            operator: TextOperator::Contains,
            text: "a \"b\"".to_owned(),
        },
        Condition::Text {
            operator: TextOperator::NotContains,
            text: "x".to_owned(),
        },
        Condition::Text {
            operator: TextOperator::BeginsWith,
            text: "R".to_owned(),
        },
        Condition::Text {
            operator: TextOperator::EndsWith,
            text: "o".to_owned(),
        },
        Condition::Duplicates { unique: true },
        Condition::Duplicates { unique: false },
        Condition::Blanks { blank: true },
        Condition::Blanks { blank: false },
        Condition::Errors { error: true },
        Condition::Errors { error: false },
        Condition::TimePeriod {
            period: "lastMonth".to_owned(),
        },
    ];
    let mut rules: Vec<ConditionalRule> = conditions
        .into_iter()
        .enumerate()
        .map(|(index, condition)| {
            let own = if index % 2 == 0 { &look } else { &plain };
            rule(
                condition,
                Some(own.clone()),
                u32::try_from(index).unwrap() + 1,
            )
        })
        .collect();
    for condition in [
        Condition::ColorScale {
            points: vec![
                ScalePoint {
                    value: ScaleValue::Min,
                    colour: "F8696B".to_owned(),
                },
                ScalePoint {
                    value: ScaleValue::Percentile("50".to_owned()),
                    colour: "FFEB84".to_owned(),
                },
                ScalePoint {
                    value: ScaleValue::Formula("$Z$1".to_owned()),
                    colour: "63BE7B".to_owned(),
                },
            ],
        },
        Condition::DataBar {
            min: ScaleValue::Number("0".to_owned()),
            max: ScaleValue::Max,
            colour: "638EC6".to_owned(),
            show_value: false,
        },
        Condition::IconSet {
            icons: "5Rating".to_owned(),
            thresholds: vec![
                ScaleValue::Percent("0".to_owned()),
                ScaleValue::Percent("20".to_owned()),
                ScaleValue::Number("40".to_owned()),
                ScaleValue::Percentile("60".to_owned()),
                ScaleValue::Formula("$Z$2".to_owned()),
            ],
            reverse: true,
            show_value: false,
        },
    ] {
        let priority = u32::try_from(rules.len()).unwrap() + 1;
        rules.push(rule(condition, None, priority));
    }
    rules
}

fn validations() -> Vec<DataValidation> {
    let base = DataValidation {
        ranges: vec![range("A1:A9"), range("C3")],
        kind: ValidationKind::List,
        operator: ComparisonOperator::Between,
        formulas: vec!["\"Sì,No\"".to_owned()],
        allow_blank: true,
        dropdown: true,
        error_style: ErrorStyle::Warning,
        show_error: true,
        error_title: Some("Valore <errato>".to_owned()),
        error: Some("Scegli \"Sì\" o \"No\"".to_owned()),
        show_prompt: true,
        prompt_title: Some("Scelta".to_owned()),
        prompt: Some("Una delle due".to_owned()),
    };
    vec![
        base.clone(),
        DataValidation {
            ranges: vec![range("B1:B9")],
            kind: ValidationKind::Decimal,
            operator: ComparisonOperator::GreaterOrEqual,
            formulas: vec!["$Z$1".to_owned()],
            allow_blank: false,
            dropdown: true,
            error_style: ErrorStyle::Stop,
            show_error: false,
            error_title: None,
            error: None,
            show_prompt: false,
            prompt_title: None,
            prompt: None,
        },
        DataValidation {
            ranges: vec![range("D1")],
            kind: ValidationKind::Whole,
            formulas: vec!["1".to_owned(), "10".to_owned()],
            dropdown: false,
            error_style: ErrorStyle::Information,
            ..base.clone()
        },
        DataValidation {
            ranges: vec![range("E1")],
            kind: ValidationKind::Custom,
            formulas: vec!["E1>D1".to_owned()],
            ..base
        },
    ]
}

#[test]
fn reads_back_every_rule_and_validation_it_writes() {
    let mut book = Workbook::new();
    let formats = vec![
        ConditionalFormat {
            ranges: vec![range("A1:B9"), range("D2")],
            rules: every_rule(),
        },
        ConditionalFormat {
            ranges: vec![range("C1")],
            rules: vec![rule(
                Condition::Expression {
                    formula: "TRUE".to_owned(),
                },
                None,
                40,
            )],
        },
    ];
    book.set_conditional_formats(0, formats.clone()).unwrap();
    book.set_validations(0, validations()).unwrap();
    let bytes = write_xlsx(&book).unwrap();
    let read = read_spreadsheet(
        &bytes,
        Some("regole.xlsx"),
        &Limits::default(),
        Locale::English,
    )
    .unwrap();
    let sheet = &read.sheets()[0];
    assert_eq!(sheet.conditional_formats().len(), 2);
    for (written, back) in formats.iter().zip(sheet.conditional_formats()) {
        assert_eq!(written.ranges, back.ranges);
        for (expected, found) in written.rules.iter().zip(&back.rules) {
            assert_eq!(expected.condition, found.condition);
            assert_eq!(expected.look, found.look, "{:?}", expected.condition);
            assert_eq!(expected.stop_if_true, found.stop_if_true);
        }
        assert_eq!(written.rules.len(), back.rules.len());
    }
    let priorities: Vec<u32> = sheet
        .conditional_formats()
        .iter()
        .flat_map(|format| format.rules.iter().map(|rule| rule.priority))
        .collect();
    assert_eq!(
        priorities,
        (1..=19).collect::<Vec<_>>(),
        "distinct, in order"
    );
    let mut expected = validations();
    // A comparison the operator does not make is not written.
    expected[2].operator = ComparisonOperator::Between;
    assert_eq!(sheet.validations(), expected.as_slice());
}

#[test]
fn writes_text_rules_with_the_formula_excel_checks() {
    let mut book = Workbook::new();
    book.set_conditional_formats(
        0,
        vec![ConditionalFormat {
            ranges: vec![range("B3:B9")],
            rules: every_rule(),
        }],
    )
    .unwrap();
    let bytes = write_xlsx(&book).unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut sheet = String::new();
    archive
        .by_name("xl/worksheets/sheet1.xml")
        .unwrap()
        .read_to_string(&mut sheet)
        .unwrap();
    for wanted in [
        r#"<cfRule type="containsText" dxfId="0" priority="5" operator="containsText" text="a &quot;b&quot;"><formula>NOT(ISERROR(SEARCH(&quot;a &quot;&quot;b&quot;&quot;&quot;,B3)))</formula>"#,
        "<formula>ISERROR(SEARCH(&quot;x&quot;,B3))</formula>",
        "<formula>LEFT(B3,LEN(&quot;R&quot;))=&quot;R&quot;</formula>",
        "<formula>RIGHT(B3,LEN(&quot;o&quot;))=&quot;o&quot;</formula>",
        "<formula>LEN(TRIM(B3))=0</formula>",
        "<formula>NOT(ISERROR(B3))</formula>",
        r#"timePeriod="lastMonth"><formula>AND(MONTH(B3)=MONTH(EDATE(TODAY(),0-1)),YEAR(B3)=YEAR(EDATE(TODAY(),0-1)))</formula>"#,
        r#"<dataBar showValue="0"><cfvo type="num" val="0"/><cfvo type="max"/><color rgb="FF638EC6"/></dataBar>"#,
        r#"<iconSet iconSet="5Rating" reverse="1" showValue="0">"#,
        r#"stopIfTrue="1""#,
    ] {
        assert!(sheet.contains(wanted), "{wanted}\n{sheet}");
    }
    assert!(
        !sheet.contains(r#"type="colorScale" dxfId"#),
        "a scale names no format"
    );
    let mut styles = String::new();
    archive
        .by_name("xl/styles.xml")
        .unwrap()
        .read_to_string(&mut styles)
        .unwrap();
    assert!(styles.contains(r#"<dxfs count="2"><dxf><font><b/><i val="0"/><strike/><u val="none"/><color rgb="FF9C0006"/></font><numFmt numFmtId="300" formatCode="0.0%"/><fill><patternFill patternType="solid"><bgColor rgb="FFFFC7CE"/></patternFill></fill></dxf><dxf><font><u/></font></dxf></dxfs></styleSheet>"#), "{styles}");
    let periods: Vec<String> = [
        "today",
        "yesterday",
        "tomorrow",
        "last7Days",
        "thisWeek",
        "lastWeek",
        "nextWeek",
        "thisMonth",
        "nextMonth",
    ]
    .iter()
    .map(|period| {
        let mut xml = String::new();
        rule_xml(
            &mut xml,
            &rule(
                Condition::TimePeriod {
                    period: (*period).to_owned(),
                },
                None,
                1,
            ),
            1,
            "A1",
            &Differentials { looks: Vec::new() },
        );
        xml
    })
    .collect();
    assert!(periods[0].contains("<formula>FLOOR(A1,1)=TODAY()</formula>"));
    assert!(periods[4].contains("TODAY()-WEEKDAY(TODAY())+1"));
    assert!(periods[5].contains("TODAY()-WEEKDAY(TODAY())-6"));
    assert!(periods[8].contains("EDATE(TODAY(),0+1)"));
    assert_eq!(hex("12345G"), "000000");
}

#[test]
fn reads_rules_another_writer_made() {
    use rust_xlsxwriter::{
        ConditionalFormatCell, ConditionalFormatCellRule, DataValidation as WriterValidation,
        Format, Workbook as WriterWorkbook,
    };
    let mut workbook = WriterWorkbook::new();
    let sheet = workbook.add_worksheet();
    for row in 0..5u32 {
        sheet.write_number(row, 0, f64::from(row) * 3.0).unwrap();
    }
    let format = ConditionalFormatCell::new()
        .set_rule(ConditionalFormatCellRule::GreaterThan(5))
        .set_format(Format::new().set_background_color("#FFC7CE").set_bold());
    sheet.add_conditional_format(0, 0, 4, 0, &format).unwrap();
    let list = WriterValidation::new()
        .allow_list_strings(&["uno", "due"])
        .unwrap();
    sheet.add_data_validation(0, 1, 4, 1, &list).unwrap();
    let bytes = workbook.save_to_buffer().unwrap();
    let book = read_spreadsheet(
        &bytes,
        Some("altro.xlsx"),
        &Limits::default(),
        Locale::English,
    )
    .unwrap();
    let format = &book.sheets()[0].conditional_formats()[0];
    assert_eq!(format.ranges, vec![range("A1:A5")]);
    assert_eq!(
        format.rules[0].condition,
        Condition::CellIs {
            operator: ComparisonOperator::Greater,
            formulas: vec!["5".to_owned()]
        }
    );
    let look = format.rules[0].look.clone().unwrap();
    assert_eq!(
        (look.fill.as_deref(), look.bold),
        (Some("FFC7CE"), Some(true))
    );
    assert_eq!(
        book.conditional_looks(0)
            .keys()
            .map(|cell| cell.row)
            .collect::<Vec<_>>(),
        vec![2, 3, 4]
    );
    let validation = &book.sheets()[0].validations()[0];
    assert_eq!(validation.kind, ValidationKind::List);
    assert_eq!(
        book.validation_items(0, crate::address::CellRef::new(2, 1)),
        vec!["uno".to_owned(), "due".to_owned()]
    );
    // The printing layout of core reads the same rules, for the renderer.
    let layouts = read_xlsx_layouts(&bytes, &Limits::default());
    assert_eq!(layouts["Sheet1"].conditional_formats.len(), 1);
    assert_eq!(layouts["Sheet1"].validations.len(), 1);
}

/// A package of the parts named, every one stored as given.
fn package(parts: &[(&str, &str)]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content) in parts {
        writer
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(content.as_bytes()).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn writes_back_the_pivot_tables_of_the_file() {
    const MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
    const RELS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
    const KIND: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    let workbook = format!(
        r#"<workbook xmlns="{MAIN}" xmlns:r="{KIND}"><sheets><sheet name="Dati" sheetId="1" r:id="rId1"/></sheets><pivotCaches><pivotCache cacheId="7" r:id="rId9"/></pivotCaches></workbook>"#
    );
    let workbook_rels = format!(
        r#"<Relationships xmlns="{RELS}"><Relationship Id="rId1" Type="{KIND}/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId9" Type="{KIND}/pivotCacheDefinition" Target="pivotCache/pivotCacheDefinition1.xml"/></Relationships>"#
    );
    let sheet = format!(
        r#"<worksheet xmlns="{MAIN}"><sheetData><row r="1"><c r="A1"><v>3</v></c></row></sheetData></worksheet>"#
    );
    let sheet_rels = format!(
        r#"<Relationships xmlns="{RELS}"><Relationship Id="rId1" Type="{KIND}/pivotTable" Target="../pivotTables/pivotTable1.xml"/></Relationships>"#
    );
    let table_rels = format!(
        r#"<Relationships xmlns="{RELS}"><Relationship Id="rId1" Type="{KIND}/pivotCacheDefinition" Target="../pivotCache/pivotCacheDefinition1.xml"/></Relationships>"#
    );
    let cache_rels = format!(
        r#"<Relationships xmlns="{RELS}"><Relationship Id="rId1" Type="{KIND}/pivotCacheRecords" Target="pivotCacheRecords1.xml"/></Relationships>"#
    );
    let types = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/pivotTables/pivotTable1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml"/><Override PartName="/xl/pivotCache/pivotCacheDefinition1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml"/></Types>"#;
    let bytes = package(&[
        ("[Content_Types].xml", types),
        (
            "_rels/.rels",
            &format!(
                r#"<Relationships xmlns="{RELS}"><Relationship Id="rId1" Type="{KIND}/officeDocument" Target="xl/workbook.xml"/></Relationships>"#
            ),
        ),
        ("xl/workbook.xml", &workbook),
        ("xl/_rels/workbook.xml.rels", &workbook_rels),
        ("xl/worksheets/sheet1.xml", &sheet),
        ("xl/worksheets/_rels/sheet1.xml.rels", &sheet_rels),
        (
            "xl/pivotTables/pivotTable1.xml",
            r#"<pivotTableDefinition cacheId="7"/>"#,
        ),
        ("xl/pivotTables/_rels/pivotTable1.xml.rels", &table_rels),
        (
            "xl/pivotCache/pivotCacheDefinition1.xml",
            "<pivotCacheDefinition/>",
        ),
        (
            "xl/pivotCache/_rels/pivotCacheDefinition1.xml.rels",
            &cache_rels,
        ),
        (
            "xl/pivotCache/pivotCacheRecords1.xml",
            "<pivotCacheRecords/>",
        ),
    ]);
    let book = read_spreadsheet(
        &bytes,
        Some("pivot.xlsx"),
        &Limits::default(),
        Locale::English,
    )
    .unwrap();
    assert_eq!(book.sheets()[0].pivot_count(), 1);
    let written = write_xlsx(&book).unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(written.clone())).unwrap();
    let mut text = |name: &str| {
        let mut content = String::new();
        archive
            .by_name(name)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        content
    };
    assert!(
        text("xl/workbook.xml").contains(
            r#"<pivotCaches><pivotCache cacheId="7" r:id="rIdPivotCache1"/></pivotCaches>"#
        )
    );
    assert!(text("xl/_rels/workbook.xml.rels").contains(r#"Id="rIdPivotCache1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheDefinition" Target="pivotCache/pivotCacheDefinition1.xml""#));
    assert!(
        text("xl/worksheets/_rels/sheet1.xml.rels")
            .contains(r#"Target="../pivotTables/pivotTable1.xml""#)
    );
    assert_eq!(
        text("xl/pivotCache/pivotCacheRecords1.xml"),
        "<pivotCacheRecords/>"
    );
    assert!(text("[Content_Types].xml").contains(r#"PartName="/xl/pivotTables/pivotTable1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml""#));
    let again = read_spreadsheet(
        &written,
        Some("pivot.xlsx"),
        &Limits::default(),
        Locale::English,
    )
    .unwrap();
    assert_eq!(
        again.sheets()[0].pivot_count(),
        1,
        "kept through a second round trip"
    );
}
