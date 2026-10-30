// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use super::*;

fn cell(address: &str) -> CellRef {
    CellRef::parse(address).unwrap()
}

fn range(text: &str) -> CellRange {
    CellRange::parse(text).unwrap()
}

/// Sales by region, product and month, one amount left blank.
pub(crate) fn sales() -> Workbook {
    let mut workbook = Workbook::new();
    let rows = [
        ["Regione", "Prodotto", "Mese", "Importo"],
        ["Nord", "Mele", "Gen", "10"],
        ["Nord", "Pere", "Gen", "20"],
        ["Sud", "Mele", "Feb", "5"],
        ["Sud", "Mele", "Gen", "7"],
        ["Nord", "Mele", "Feb", "3"],
        ["Centro", "Pere", "Feb", ""],
    ];
    let mut inputs = Vec::new();
    for (row, line) in rows.iter().enumerate() {
        for (column, text) in line.iter().enumerate() {
            #[expect(clippy::cast_possible_truncation, reason = "a small fixture")]
            inputs.push((CellRef::new(row as u32, column as u32), (*text).to_owned()));
        }
    }
    workbook.set_inputs(0, &inputs, Locale::English).unwrap();
    workbook
}

pub(crate) fn by_region(anchor: &str) -> PivotTable {
    let mut table = PivotTable::new("Vendite", "Foglio1", range("A1:D7"), cell(anchor));
    table.locale = Locale::English;
    table.layout = PivotLayout::Tabular;
    table.rows = vec![PivotField {
        field: 0,
        hidden: Vec::new(),
    }];
    table.values = vec![PivotValue {
        field: 3,
        function: PivotFunction::Sum,
        name: None,
    }];
    table
}

fn grid(workbook: &Workbook, area: &str) -> Vec<Vec<Value>> {
    let area = range(area);
    (area.first_row..=area.last_row)
        .map(|row| {
            (area.first_column..=area.last_column)
                .map(|column| workbook.value(0, CellRef::new(row, column)))
                .collect()
        })
        .collect()
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

#[test]
fn a_table_sums_the_records_of_each_row_label() {
    let mut workbook = sales();
    assert_eq!(workbook.add_pivot_table(0, by_region("F1")).unwrap(), 0);
    let table = &workbook.sheets()[0].pivot_tables()[0];
    assert_eq!(table.area(), Some(range("F1:G5")));
    assert_eq!(
        grid(&workbook, "F1:G5"),
        vec![
            vec![text("Regione"), text("Sum of Importo")],
            vec![text("Centro"), Value::Number(0.0)],
            vec![text("Nord"), Value::Number(33.0)],
            vec![text("Sud"), Value::Number(12.0)],
            vec![text("Grand Total"), Value::Number(45.0)],
        ]
    );
}

#[test]
fn columns_spread_the_values_with_their_totals() {
    let mut workbook = sales();
    let mut table = by_region("F1");
    table.columns = vec![PivotField {
        field: 2,
        hidden: Vec::new(),
    }];
    workbook.add_pivot_table(0, table).unwrap();
    assert_eq!(
        grid(&workbook, "F1:I6"),
        vec![
            vec![
                text("Sum of Importo"),
                text("Mese"),
                Value::Empty,
                Value::Empty
            ],
            vec![
                text("Regione"),
                text("Feb"),
                text("Gen"),
                text("Grand Total")
            ],
            vec![
                text("Centro"),
                Value::Number(0.0),
                Value::Empty,
                Value::Number(0.0)
            ],
            vec![
                text("Nord"),
                Value::Number(3.0),
                Value::Number(30.0),
                Value::Number(33.0)
            ],
            vec![
                text("Sud"),
                Value::Number(5.0),
                Value::Number(7.0),
                Value::Number(12.0)
            ],
            vec![
                text("Grand Total"),
                Value::Number(8.0),
                Value::Number(37.0),
                Value::Number(45.0)
            ],
        ]
    );
}

#[test]
fn a_filter_and_hidden_items_leave_records_out() {
    let mut workbook = sales();
    let mut table = by_region("F1");
    table.filters = vec![PivotField {
        field: 1,
        hidden: vec!["Pere".to_owned()],
    }];
    table.rows[0].hidden = vec!["Sud".to_owned()];
    table.values.push(PivotValue {
        field: 3,
        function: PivotFunction::Count,
        name: Some("Righe".to_owned()),
    });
    table.row_grand_totals = false;
    workbook.add_pivot_table(0, table).unwrap();
    assert_eq!(
        grid(&workbook, "F1:H4"),
        vec![
            vec![text("Prodotto"), text("Mele"), Value::Empty],
            vec![Value::Empty, Value::Empty, Value::Empty],
            vec![text("Regione"), text("Sum of Importo"), text("Righe")],
            vec![text("Nord"), Value::Number(13.0), Value::Number(2.0)],
        ]
    );
    assert_eq!(
        workbook.sheets()[0].pivot_tables()[0].area(),
        Some(range("F1:H4"))
    );
}

#[test]
fn every_function_summarises_as_a_spreadsheet_does() {
    let values = [
        Value::Number(2.0),
        Value::Number(4.0),
        text("x"),
        Value::Empty,
    ];
    let expected = [
        (PivotFunction::Sum, 6.0),
        (PivotFunction::Count, 3.0),
        (PivotFunction::Average, 3.0),
        (PivotFunction::Max, 4.0),
        (PivotFunction::Min, 2.0),
        (PivotFunction::Product, 8.0),
        (PivotFunction::CountNumbers, 2.0),
        (PivotFunction::StdDev, 2.0_f64.sqrt()),
        (PivotFunction::StdDevP, 1.0),
        (PivotFunction::Var, 2.0),
        (PivotFunction::VarP, 1.0),
    ];
    for (function, number) in expected {
        let Value::Number(found) = function.summarise(&values) else {
            panic!("{function:?}");
        };
        assert!((found - number).abs() < 1e-12, "{function:?} {found}");
        assert_eq!(PivotFunction::parse(function.name()), Some(function));
        assert_eq!(PivotFunction::from_xlsx(function.xlsx()), Some(function));
        assert_eq!(PivotFunction::from_odf(function.odf()), Some(function));
    }
    let none = [text("x")];
    assert_eq!(
        PivotFunction::Average.summarise(&none),
        Value::Error(ErrorKind::Div0)
    );
    assert_eq!(
        PivotFunction::StdDev.summarise(&[Value::Number(1.0)]),
        Value::Error(ErrorKind::Div0)
    );
    assert_eq!(
        PivotFunction::VarP.summarise(&none),
        Value::Error(ErrorKind::Div0)
    );
    assert_eq!(PivotFunction::Product.summarise(&none), Value::Number(0.0));
    assert_eq!(PivotFunction::parse("median"), None);
}

#[test]
fn an_italian_table_writes_italian_captions() {
    let mut workbook = sales();
    let mut table = by_region("F1");
    table.locale = Locale::Italian;
    table.filters = vec![PivotField {
        field: 2,
        hidden: Vec::new(),
    }];
    workbook.add_pivot_table(0, table).unwrap();
    assert_eq!(workbook.value(0, cell("G1")), text("(Tutto)"));
    assert_eq!(workbook.value(0, cell("G3")), text("Somma di Importo"));
    assert_eq!(workbook.value(0, cell("F7")), text("Totale complessivo"));
}

#[test]
fn a_refresh_reads_the_source_again_and_a_change_moves_the_cells() {
    let mut workbook = sales();
    workbook.add_pivot_table(0, by_region("F1")).unwrap();
    workbook
        .set_input(0, cell("D2"), "100", Locale::English)
        .unwrap();
    assert_eq!(workbook.value(0, cell("G3")), Value::Number(33.0));
    workbook.refresh_pivot_table(0, 0).unwrap();
    assert_eq!(workbook.value(0, cell("G3")), Value::Number(123.0));
    assert_eq!(workbook.refresh_pivot_tables(), 1);

    workbook.set_pivot_table(0, 0, by_region("K10")).unwrap();
    assert_eq!(workbook.value(0, cell("F1")), Value::Empty);
    assert_eq!(workbook.value(0, cell("K10")), text("Regione"));
    workbook.remove_pivot_table(0, 0).unwrap();
    assert_eq!(workbook.value(0, cell("K10")), Value::Empty);
    assert_eq!(workbook.sheets()[0].pivot_tables().len(), 0);
}

#[test]
fn a_table_that_cannot_be_is_refused() {
    let mut workbook = sales();
    assert_eq!(
        workbook.add_pivot_table(0, by_region("B2")),
        Err(EditError::InvalidPivot)
    );
    let mut outside = by_region("F1");
    outside.rows[0].field = 9;
    assert_eq!(
        workbook.add_pivot_table(0, outside),
        Err(EditError::InvalidPivot)
    );
    let mut twice = by_region("F1");
    twice.columns = twice.rows.clone();
    assert_eq!(
        workbook.add_pivot_table(0, twice),
        Err(EditError::InvalidPivot)
    );
    let mut elsewhere = by_region("F1");
    elsewhere.source_sheet = "Nessuno".to_owned();
    assert_eq!(
        workbook.add_pivot_table(0, elsewhere),
        Err(EditError::InvalidPivot)
    );
    assert_eq!(
        workbook.add_pivot_table(3, by_region("F1")),
        Err(EditError::NoSuchSheet(3))
    );
    assert_eq!(
        workbook.add_pivot_table(0, by_region("XFD1")),
        Err(EditError::OutOfBounds)
    );
    assert_eq!(
        workbook.refresh_pivot_table(0, 0),
        Err(EditError::InvalidPivot)
    );
    assert_eq!(
        workbook.remove_pivot_table(0, 0),
        Err(EditError::InvalidPivot)
    );
    assert_eq!(
        workbook.set_pivot_table(0, 0, by_region("F1")),
        Err(EditError::InvalidPivot)
    );
    for _ in 0..MAX_PIVOT_TABLES {
        let mut table = by_region("F1");
        table.anchor = CellRef::new(0, 10);
        workbook.add_pivot_table(0, table).unwrap();
    }
    assert_eq!(
        workbook.add_pivot_table(0, by_region("F1")),
        Err(EditError::InvalidPivot)
    );
}

#[test]
fn a_table_follows_its_sheet_and_its_source() {
    let mut workbook = sales();
    workbook.add_pivot_table(0, by_region("F1")).unwrap();
    workbook.insert_rows(0, 0, 2).unwrap();
    let table = &workbook.sheets()[0].pivot_tables()[0];
    assert_eq!(table.anchor, cell("F3"));
    assert_eq!(table.source, range("A3:D9"));
    assert_eq!(table.area(), Some(range("F3:G7")));
    workbook.rename_sheet(0, "Dati").unwrap();
    assert_eq!(workbook.sheets()[0].pivot_tables()[0].source_sheet, "Dati");
    workbook.refresh_pivot_table(0, 0).unwrap();
    assert_eq!(workbook.value(0, cell("G7")), Value::Number(45.0));
    workbook.delete_rows(0, 2, 1).unwrap();
    assert_eq!(workbook.sheets()[0].pivot_tables().len(), 0);
}

#[test]
fn the_fields_of_a_source_list_their_items() {
    let workbook = sales();
    let fields = workbook.pivot_source_fields(0, range("A1:D7")).unwrap();
    assert_eq!(fields.len(), 4);
    assert_eq!(fields[0].name, "Regione");
    assert_eq!(
        fields[0].items,
        vec![text("Centro"), text("Nord"), text("Sud")]
    );
    assert_eq!(
        fields[3].items,
        vec![
            Value::Number(3.0),
            Value::Number(5.0),
            Value::Number(7.0),
            Value::Number(10.0),
            Value::Number(20.0),
            Value::Empty
        ]
    );
    assert_eq!(
        workbook.pivot_source_fields(5, range("A1:D7")),
        Err(EditError::NoSuchSheet(5))
    );
}

#[test]
fn items_are_labelled_and_ordered_as_a_pivot_table_shows_them() {
    assert_eq!(item_label(&Value::Number(2024.0)), "2024");
    assert_eq!(item_label(&Value::Bool(true)), "TRUE");
    assert_eq!(item_label(&Value::Error(ErrorKind::Div0)), "#DIV/0!");
    let mut keys = [
        Key(Value::Empty),
        Key(text("b")),
        Key(Value::Bool(false)),
        Key(Value::Number(2.0)),
        Key(text("A")),
        Key(Value::Error(ErrorKind::NotAvailable)),
    ];
    keys.sort();
    let order: Vec<Value> = keys.into_iter().map(|key| key.0).collect();
    assert_eq!(
        order,
        vec![
            Value::Number(2.0),
            text("A"),
            text("b"),
            Value::Bool(false),
            Value::Error(ErrorKind::NotAvailable),
            Value::Empty
        ]
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "every function and refusal in one workbook"
)]
fn the_pivot_functions_summarise_arrays_and_read_a_table() {
    let mut workbook = sales();
    workbook.add_pivot_table(0, by_region("F1")).unwrap();
    let formula = |workbook: &mut Workbook, text: &str| {
        workbook
            .set_input(0, cell("Z1"), text, Locale::English)
            .unwrap();
        workbook.value(0, cell("Z1"))
    };
    assert_eq!(
        formula(
            &mut workbook,
            r#"=GETPIVOTDATA("Importo",F1,"Regione","Nord")"#
        ),
        Value::Number(33.0)
    );
    assert_eq!(
        formula(&mut workbook, r#"=GETPIVOTDATA("Sum of Importo",$F$1)"#),
        Value::Number(45.0)
    );
    assert_eq!(
        formula(
            &mut workbook,
            r#"=GETPIVOTDATA("Importo",F1,"Prodotto","Mele")"#
        ),
        Value::Error(ErrorKind::Ref)
    );
    assert_eq!(
        formula(&mut workbook, r#"=GETPIVOTDATA("Importo",A1)"#),
        Value::Error(ErrorKind::Ref)
    );
    assert_eq!(
        formula(
            &mut workbook,
            r#"=GETPIVOTDATA("Importo",F1,"Regione","Est")"#
        ),
        Value::Error(ErrorKind::Ref)
    );
    assert_eq!(
        formula(&mut workbook, r#"=GETPIVOTDATA("Importo")"#),
        Value::Error(ErrorKind::Value)
    );

    let spill = |workbook: &mut Workbook, text: &str, area: &str| {
        workbook
            .set_input(0, cell("Z1"), text, Locale::English)
            .unwrap();
        let area = range(area);
        (area.first_row..=area.last_row)
            .map(|row| {
                (area.first_column..=area.last_column)
                    .map(|column| workbook.value(0, CellRef::new(row, column)))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        spill(&mut workbook, "=GROUPBY(A2:A7,D2:D7,SUM)", "Z1:AA4"),
        vec![
            vec![text("Centro"), Value::Number(0.0)],
            vec![text("Nord"), Value::Number(33.0)],
            vec![text("Sud"), Value::Number(12.0)],
            vec![text("Total"), Value::Number(45.0)],
        ]
    );
    assert_eq!(
        spill(
            &mut workbook,
            "=GROUPBY(A1:A7,D1:D7,LAMBDA(x,MAX(x)),,0,-2)",
            "Z1:AA4"
        ),
        vec![
            vec![text("Regione"), text("Importo")],
            vec![text("Nord"), Value::Number(20.0)],
            vec![text("Sud"), Value::Number(7.0)],
            vec![text("Centro"), Value::Number(0.0)],
        ]
    );
    assert_eq!(
        spill(&mut workbook, "=GROUPBY(A2:B7,D2:D7,SUM,0,2)", "Z1:AB4"),
        vec![
            vec![text("Centro"), text("Pere"), Value::Number(0.0)],
            vec![text("Centro"), Value::Empty, Value::Number(0.0)],
            vec![text("Nord"), text("Mele"), Value::Number(13.0)],
            vec![text("Nord"), text("Pere"), Value::Number(20.0)],
        ]
    );
    assert_eq!(
        spill(
            &mut workbook,
            "=GROUPBY(A2:A7,D2:D7,COUNT,0,1,1,B2:B7=\"Mele\")",
            "Z1:AA3"
        ),
        vec![
            vec![text("Nord"), Value::Number(2.0)],
            vec![text("Sud"), Value::Number(2.0)],
            vec![text("Total"), Value::Number(4.0)],
        ]
    );
    assert_eq!(
        spill(&mut workbook, "=PIVOTBY(A2:A7,C2:C7,D2:D7,SUM)", "Z1:AC5"),
        vec![
            vec![Value::Empty, text("Feb"), text("Gen"), text("Total")],
            vec![
                text("Centro"),
                Value::Number(0.0),
                Value::Empty,
                Value::Number(0.0)
            ],
            vec![
                text("Nord"),
                Value::Number(3.0),
                Value::Number(30.0),
                Value::Number(33.0)
            ],
            vec![
                text("Sud"),
                Value::Number(5.0),
                Value::Number(7.0),
                Value::Number(12.0)
            ],
            vec![
                text("Total"),
                Value::Number(8.0),
                Value::Number(37.0),
                Value::Number(45.0)
            ],
        ]
    );
    assert_eq!(
        formula(&mut workbook, "=GROUPBY(A2:A7,D2:D6,SUM)"),
        Value::Error(ErrorKind::Value)
    );
    assert_eq!(
        formula(&mut workbook, "=GROUPBY(A2:A7,D2:D7,1)"),
        Value::Error(ErrorKind::Value)
    );
    assert_eq!(
        formula(&mut workbook, "=GROUPBY(A2:A7,D2:D7,SUM,7)"),
        Value::Error(ErrorKind::Value)
    );
    assert_eq!(
        formula(&mut workbook, "=PIVOTBY(A2:A7,C2:C6,D2:D7,SUM)"),
        Value::Error(ErrorKind::Value)
    );
}

fn number(value: f64) -> Value {
    Value::Number(value)
}

/// Sales by region then product, in a layout.
fn by_region_and_product(layout: PivotLayout, subtotals: PivotSubtotals) -> PivotTable {
    let mut table = by_region("F1");
    table.rows.push(PivotField {
        field: 1,
        hidden: Vec::new(),
    });
    table.layout = layout;
    table.subtotals = subtotals;
    table
}

#[test]
fn the_compact_form_indents_each_field_in_one_column() {
    let mut workbook = sales();
    let table = by_region_and_product(PivotLayout::Compact, PivotSubtotals::Top);
    workbook.add_pivot_table(0, table).unwrap();
    assert_eq!(
        grid(&workbook, "F1:G9"),
        vec![
            vec![text("Row Labels"), text("Sum of Importo")],
            vec![text("Centro"), number(0.0)],
            vec![text("Pere"), number(0.0)],
            vec![text("Nord"), number(33.0)],
            vec![text("Mele"), number(13.0)],
            vec![text("Pere"), number(20.0)],
            vec![text("Sud"), number(12.0)],
            vec![text("Mele"), number(12.0)],
            vec![text("Grand Total"), number(45.0)],
        ]
    );
    let indent = |workbook: &Workbook, address: &str| workbook.style(0, cell(address)).indent;
    assert_eq!(indent(&workbook, "F2"), 0);
    assert_eq!(indent(&workbook, "F3"), 1);
    assert_eq!(indent(&workbook, "F6"), 1);
    assert_eq!(indent(&workbook, "G3"), 0);

    // Another layout takes the indent off the cells it no longer indents.
    let table = by_region_and_product(PivotLayout::Tabular, PivotSubtotals::Off);
    workbook.set_pivot_table(0, 0, table).unwrap();
    assert_eq!(indent(&workbook, "F3"), 0);
    workbook.remove_pivot_table(0, 0).unwrap();
    assert_eq!(workbook.sheets()[0].styles.len(), 0);
}

#[test]
fn the_outline_form_puts_each_group_on_its_own_row() {
    let mut workbook = sales();
    let table = by_region_and_product(PivotLayout::Outline, PivotSubtotals::Bottom);
    workbook.add_pivot_table(0, table).unwrap();
    assert_eq!(
        grid(&workbook, "F1:H12"),
        vec![
            vec![text("Regione"), text("Prodotto"), text("Sum of Importo")],
            vec![text("Centro"), Value::Empty, Value::Empty],
            vec![Value::Empty, text("Pere"), number(0.0)],
            vec![text("Centro Total"), Value::Empty, number(0.0)],
            vec![text("Nord"), Value::Empty, Value::Empty],
            vec![Value::Empty, text("Mele"), number(13.0)],
            vec![Value::Empty, text("Pere"), number(20.0)],
            vec![text("Nord Total"), Value::Empty, number(33.0)],
            vec![text("Sud"), Value::Empty, Value::Empty],
            vec![Value::Empty, text("Mele"), number(12.0)],
            vec![text("Sud Total"), Value::Empty, number(12.0)],
            vec![text("Grand Total"), Value::Empty, number(45.0)],
        ]
    );
    let mut repeated = by_region_and_product(PivotLayout::Outline, PivotSubtotals::Top);
    repeated.repeat_labels = true;
    repeated.locale = Locale::Italian;
    workbook.set_pivot_table(0, 0, repeated).unwrap();
    assert_eq!(
        grid(&workbook, "F2:H4"),
        vec![
            vec![text("Centro"), Value::Empty, number(0.0)],
            vec![text("Centro"), text("Pere"), number(0.0)],
            vec![text("Nord"), Value::Empty, number(33.0)],
        ]
    );
}

#[test]
fn the_tabular_form_labels_a_group_beside_its_first_item() {
    let mut workbook = sales();
    let table = by_region_and_product(PivotLayout::Tabular, PivotSubtotals::Top);
    workbook.add_pivot_table(0, table).unwrap();
    assert_eq!(
        grid(&workbook, "F1:H9"),
        vec![
            vec![text("Regione"), text("Prodotto"), text("Sum of Importo")],
            vec![text("Centro"), text("Pere"), number(0.0)],
            vec![text("Centro Total"), Value::Empty, number(0.0)],
            vec![text("Nord"), text("Mele"), number(13.0)],
            vec![Value::Empty, text("Pere"), number(20.0)],
            vec![text("Nord Total"), Value::Empty, number(33.0)],
            vec![text("Sud"), text("Mele"), number(12.0)],
            vec![text("Sud Total"), Value::Empty, number(12.0)],
            vec![text("Grand Total"), Value::Empty, number(45.0)],
        ]
    );
    let mut repeated = by_region_and_product(PivotLayout::Tabular, PivotSubtotals::Off);
    repeated.repeat_labels = true;
    repeated.row_grand_totals = false;
    workbook.set_pivot_table(0, 0, repeated).unwrap();
    assert_eq!(
        grid(&workbook, "F2:H5"),
        vec![
            vec![text("Centro"), text("Pere"), number(0.0)],
            vec![text("Nord"), text("Mele"), number(13.0)],
            vec![text("Nord"), text("Pere"), number(20.0)],
            vec![text("Sud"), text("Mele"), number(12.0)],
        ]
    );
    assert_eq!(
        workbook.sheets()[0].pivot_tables()[0].area(),
        Some(range("F1:H5"))
    );
}

#[test]
#[expect(clippy::too_many_lines, reason = "two whole tables written out")]
fn column_fields_nest_with_their_subtotals_and_several_values() {
    let mut workbook = sales();
    let mut table = by_region("F1");
    table.layout = PivotLayout::Compact;
    table.columns = vec![
        PivotField {
            field: 2,
            hidden: Vec::new(),
        },
        PivotField {
            field: 1,
            hidden: Vec::new(),
        },
    ];
    workbook.add_pivot_table(0, table.clone()).unwrap();
    let empty = || Value::Empty;
    assert_eq!(
        grid(&workbook, "F1:M7"),
        vec![
            vec![
                text("Sum of Importo"),
                text("Column Labels"),
                empty(),
                empty(),
                empty(),
                empty(),
                empty(),
                empty()
            ],
            vec![
                empty(),
                text("Feb"),
                empty(),
                text("Feb Total"),
                text("Gen"),
                empty(),
                text("Gen Total"),
                text("Grand Total")
            ],
            vec![
                text("Row Labels"),
                text("Mele"),
                text("Pere"),
                empty(),
                text("Mele"),
                text("Pere"),
                empty(),
                empty()
            ],
            vec![
                text("Centro"),
                empty(),
                number(0.0),
                number(0.0),
                empty(),
                empty(),
                empty(),
                number(0.0)
            ],
            vec![
                text("Nord"),
                number(3.0),
                empty(),
                number(3.0),
                number(10.0),
                number(20.0),
                number(30.0),
                number(33.0)
            ],
            vec![
                text("Sud"),
                number(5.0),
                empty(),
                number(5.0),
                number(7.0),
                empty(),
                number(7.0),
                number(12.0)
            ],
            vec![
                text("Grand Total"),
                number(8.0),
                number(0.0),
                number(8.0),
                number(17.0),
                number(20.0),
                number(37.0),
                number(45.0)
            ],
        ]
    );

    table.columns.truncate(1);
    table.values.push(PivotValue {
        field: 3,
        function: PivotFunction::Count,
        name: Some("Righe".to_owned()),
    });
    table.locale = Locale::Italian;
    workbook.set_pivot_table(0, 0, table).unwrap();
    assert_eq!(
        grid(&workbook, "F1:L4"),
        vec![
            vec![
                empty(),
                text("Etichette di colonna"),
                empty(),
                empty(),
                empty(),
                empty(),
                empty()
            ],
            vec![
                empty(),
                text("Feb"),
                empty(),
                text("Gen"),
                empty(),
                text("Totale Somma di Importo"),
                text("Totale Righe")
            ],
            vec![
                text("Etichette di riga"),
                text("Somma di Importo"),
                text("Righe"),
                text("Somma di Importo"),
                text("Righe"),
                empty(),
                empty()
            ],
            vec![
                text("Centro"),
                number(0.0),
                number(0.0),
                empty(),
                empty(),
                number(0.0),
                number(0.0)
            ],
        ]
    );
}

#[test]
fn the_layouts_and_subtotals_have_stable_names() {
    for layout in PivotLayout::ALL {
        assert_eq!(PivotLayout::parse(layout.name()), Some(layout));
    }
    for subtotals in PivotSubtotals::ALL {
        assert_eq!(PivotSubtotals::parse(subtotals.name()), Some(subtotals));
    }
    assert_eq!(PivotLayout::parse("grid"), None);
    assert_eq!(PivotSubtotals::parse("off"), None);
    let table = PivotTable::new("T", "Foglio1", range("A1:D7"), cell("F1"));
    assert_eq!(
        (table.layout, table.subtotals, table.repeat_labels),
        (PivotLayout::Compact, PivotSubtotals::Top, false)
    );
}

/// What a formula typed in Z1 spills over an area.
fn spilled(workbook: &mut Workbook, formula: &str, area: &str) -> Vec<Vec<Value>> {
    workbook
        .set_input(0, cell("Z1"), formula, Locale::English)
        .unwrap();
    grid(workbook, area)
}

#[test]
fn groupby_takes_several_functions_and_measures_against_the_whole() {
    let mut workbook = sales();
    assert_eq!(
        spilled(
            &mut workbook,
            "=GROUPBY(A1:A7,D1:D7,HSTACK(SUM,COUNT),3,0)",
            "Z1:AB4"
        ),
        vec![
            vec![text("Regione"), text("Importo SUM"), text("Importo COUNT")],
            vec![text("Centro"), number(0.0), number(0.0)],
            vec![text("Nord"), number(33.0), number(3.0)],
            vec![text("Sud"), number(12.0), number(2.0)],
        ]
    );
    assert_eq!(
        spilled(
            &mut workbook,
            "=GROUPBY(A2:A7,D2:D7,PERCENTOF,0,1)",
            "Z1:AA4"
        ),
        vec![
            vec![text("Centro"), number(0.0)],
            vec![text("Nord"), number(33.0 / 45.0)],
            vec![text("Sud"), number(12.0 / 45.0)],
            vec![text("Total"), number(1.0)],
        ]
    );
    assert_eq!(
        spilled(
            &mut workbook,
            "=GROUPBY(A2:A7,D2:D7,LAMBDA(part,whole,SUM(part)/SUM(whole)),0,0)",
            "Z1:AA3"
        ),
        vec![
            vec![text("Centro"), number(0.0)],
            vec![text("Nord"), number(33.0 / 45.0)],
            vec![text("Sud"), number(12.0 / 45.0)],
        ]
    );
}

#[test]
fn groupby_sorts_by_several_columns_and_by_the_hierarchy_or_the_table() {
    let mut workbook = sales();
    assert_eq!(
        spilled(
            &mut workbook,
            "=GROUPBY(A2:B7,D2:D7,SUM,0,0,{-1,2})",
            "Z1:AB4"
        ),
        vec![
            vec![text("Sud"), text("Mele"), number(12.0)],
            vec![text("Nord"), text("Mele"), number(13.0)],
            vec![text("Nord"), text("Pere"), number(20.0)],
            vec![text("Centro"), text("Pere"), number(0.0)],
        ]
    );
    // By product then region, the products ordered by their own totals, down.
    let hierarchy = "=GROUPBY(HSTACK(B2:B7,A2:A7),D2:D7,SUM,0,0,-3)";
    assert_eq!(
        spilled(&mut workbook, hierarchy, "Z1:AB4"),
        vec![
            vec![text("Mele"), text("Nord"), number(13.0)],
            vec![text("Mele"), text("Sud"), number(12.0)],
            vec![text("Pere"), text("Nord"), number(20.0)],
            vec![text("Pere"), text("Centro"), number(0.0)],
        ]
    );
    // As a table each row is sorted on its own value.
    let table = "=GROUPBY(HSTACK(B2:B7,A2:A7),D2:D7,SUM,0,0,-3,,1)";
    assert_eq!(
        spilled(&mut workbook, table, "Z1:AB4"),
        vec![
            vec![text("Pere"), text("Nord"), number(20.0)],
            vec![text("Mele"), text("Nord"), number(13.0)],
            vec![text("Mele"), text("Sud"), number(12.0)],
            vec![text("Pere"), text("Centro"), number(0.0)],
        ]
    );
    assert_eq!(
        spilled(&mut workbook, "=GROUPBY(A2:B7,D2:D7,SUM,0,2,,,1)", "Z1:Z1"),
        vec![vec![Value::Error(ErrorKind::Value)]]
    );
    assert_eq!(
        spilled(&mut workbook, "=GROUPBY(A2:B7,D2:D7,SUM,0,0,4)", "Z1:Z1"),
        vec![vec![Value::Error(ErrorKind::Value)]]
    );
    assert_eq!(
        spilled(&mut workbook, "=GROUPBY(A2:B7,D2:D7,SUM,0,0,,,2)", "Z1:Z1"),
        vec![vec![Value::Error(ErrorKind::Value)]]
    );
}

#[test]
fn groupby_puts_subtotals_and_the_total_at_the_top() {
    let mut workbook = sales();
    assert_eq!(
        spilled(&mut workbook, "=GROUPBY(A2:B7,D2:D7,SUM,0,-2)", "Z1:AB8"),
        vec![
            vec![text("Total"), Value::Empty, number(45.0)],
            vec![text("Centro"), Value::Empty, number(0.0)],
            vec![text("Centro"), text("Pere"), number(0.0)],
            vec![text("Nord"), Value::Empty, number(33.0)],
            vec![text("Nord"), text("Mele"), number(13.0)],
            vec![text("Nord"), text("Pere"), number(20.0)],
            vec![text("Sud"), Value::Empty, number(12.0)],
            vec![text("Sud"), text("Mele"), number(12.0)],
        ]
    );
}

#[test]
#[expect(clippy::too_many_lines, reason = "several spills written out")]
fn pivotby_measures_against_rows_columns_or_the_whole_and_nests_columns() {
    let mut workbook = sales();
    let empty = || Value::Empty;
    assert_eq!(
        spilled(
            &mut workbook,
            "=PIVOTBY(A2:A7,C2:C7,D2:D7,PERCENTOF,0,0,,0,,,1)",
            "Z1:AB4"
        ),
        vec![
            vec![empty(), text("Feb"), text("Gen")],
            vec![text("Centro"), Value::Error(ErrorKind::Div0), empty()],
            vec![text("Nord"), number(3.0 / 33.0), number(30.0 / 33.0)],
            vec![text("Sud"), number(5.0 / 12.0), number(7.0 / 12.0)],
        ]
    );
    assert_eq!(
        spilled(
            &mut workbook,
            "=PIVOTBY(A2:A7,C2:C7,D2:D7,PERCENTOF,0,0,,0)",
            "AA3:AB3"
        ),
        vec![vec![number(3.0 / 8.0), number(30.0 / 37.0)]]
    );
    assert_eq!(
        spilled(
            &mut workbook,
            "=PIVOTBY(A2:A7,C2:C7,D2:D7,PERCENTOF,0,0,,0,,,2)",
            "AA3:AB3"
        ),
        vec![vec![number(3.0 / 45.0), number(30.0 / 45.0)]]
    );
    assert_eq!(
        spilled(
            &mut workbook,
            "=PIVOTBY(A2:A7,HSTACK(C2:C7,B2:B7),D2:D7,SUM,0,0,,2)",
            "Z1:AG4"
        ),
        vec![
            vec![
                empty(),
                text("Feb"),
                text("Feb"),
                text("Feb"),
                text("Gen"),
                text("Gen"),
                text("Gen"),
                text("Total")
            ],
            vec![
                empty(),
                text("Mele"),
                text("Pere"),
                empty(),
                text("Mele"),
                text("Pere"),
                empty(),
                empty()
            ],
            vec![
                text("Centro"),
                empty(),
                number(0.0),
                number(0.0),
                empty(),
                empty(),
                empty(),
                number(0.0)
            ],
            vec![
                text("Nord"),
                number(3.0),
                empty(),
                number(3.0),
                number(10.0),
                number(20.0),
                number(30.0),
                number(33.0)
            ],
        ]
    );
    assert_eq!(
        spilled(
            &mut workbook,
            "=PIVOTBY(A2:A7,C2:C7,D2:D7,SUM,0,0,-2,0,-2)",
            "Z1:AB4"
        ),
        vec![
            vec![empty(), text("Gen"), text("Feb")],
            vec![text("Nord"), number(30.0), number(3.0)],
            vec![text("Sud"), number(7.0), number(5.0)],
            vec![text("Centro"), empty(), number(0.0)],
        ]
    );
    assert_eq!(
        spilled(
            &mut workbook,
            "=PIVOTBY(A2:A7,C2:C7,D2:D7,SUM,0,0,,0,,,5)",
            "Z1:Z1"
        ),
        vec![vec![Value::Error(ErrorKind::Value)]]
    );
}
