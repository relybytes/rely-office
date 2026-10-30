// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

use crate::address::CellRef;
use crate::model::Workbook;
use crate::value::{ErrorKind, Locale, Value};

/// A1:A6 numbers, B1:B6 names, C1:C6 regions, D1 a date.
const DATA: &[(&str, &str)] = &[
    ("A1", "10"),
    ("A2", "20"),
    ("A3", "20"),
    ("A4", "30"),
    ("A5", "5"),
    ("A6", "testo"),
    ("B1", "Rossi"),
    ("B2", "Bianchi"),
    ("B3", "Verdi"),
    ("B4", "Neri"),
    ("B5", "Gialli"),
    ("C1", "Nord"),
    ("C2", "Sud"),
    ("C3", "Nord"),
    ("C4", "Nord"),
    ("C5", "Sud"),
    ("D1", "=DATE(2026,10,27)"),
    ("E1", "-1000"),
    ("E2", "300"),
    ("E3", "400"),
    ("E4", "500"),
];

fn value(formula: &str) -> Value {
    let mut workbook = Workbook::new();
    let inputs: Vec<(CellRef, String)> = DATA
        .iter()
        .map(|(address, typed)| (CellRef::parse(address).unwrap(), (*typed).to_owned()))
        .collect();
    workbook.set_inputs(0, &inputs, Locale::English).unwrap();
    workbook.set_today(Some(46_322.5));
    workbook
        .set_input(0, CellRef::parse("H8").unwrap(), formula, Locale::English)
        .unwrap();
    workbook.value(0, CellRef::parse("H8").unwrap())
}

fn number(formula: &str) -> f64 {
    match value(formula) {
        Value::Number(number) => number,
        other => panic!("{formula} gave {other:?}"),
    }
}

fn text(formula: &str) -> String {
    match value(formula) {
        Value::Text(text) => text,
        other => panic!("{formula} gave {other:?}"),
    }
}

fn error(formula: &str) -> ErrorKind {
    match value(formula) {
        Value::Error(kind) => kind,
        other => panic!("{formula} gave {other:?}, not an error"),
    }
}

fn close(formula: &str, expected: f64) {
    let actual = number(formula);
    assert!(
        (actual - expected).abs() < 1e-6 * expected.abs().max(1.0),
        "{formula} gave {actual}, not {expected}"
    );
}

#[test]
fn computes_more_mathematics() {
    assert_eq!(
        number("=SUMPRODUCT(A1:A2,A3:A4)"),
        10.0 * 20.0 + 20.0 * 30.0
    );
    assert_eq!(error("=SUMPRODUCT(A1:A2,A1:A3)"), ErrorKind::Value);
    assert_eq!(number("=CEILING(2.1,0.5)"), 2.5);
    assert_eq!(number("=CEILING(-2.5,2)"), -2.0);
    assert_eq!(number("=CEILING(5,0)"), 0.0);
    assert_eq!(error("=CEILING(2,-1)"), ErrorKind::Num);
    assert_eq!(number("=FLOOR(2.9,0.5)"), 2.5);
    assert_eq!(number("=FLOOR(-2.5,2)"), -4.0);
    assert_eq!(error("=FLOOR(2,0)"), ErrorKind::Div0);
    assert_eq!(number("=CEILING.MATH(-5.5,2)"), -4.0);
    assert_eq!(number("=CEILING.MATH(-5.5,2,1)"), -6.0);
    assert_eq!(number("=FLOOR.MATH(-5.5,2)"), -6.0);
    assert_eq!(number("=FLOOR.MATH(-5.5,2,1)"), -4.0);
    assert_eq!(number("=FLOOR.MATH(5.5)"), 5.0);
    assert_eq!(number("=CEILING.MATH(5,0)"), 0.0);
    assert_eq!(number("=MROUND(10,3)"), 9.0);
    assert_eq!(number("=MROUND(-10,-3)"), -9.0);
    assert_eq!(number("=MROUND(5,0)"), 0.0);
    assert_eq!(error("=MROUND(5,-2)"), ErrorKind::Num);
    assert_eq!(number("=TRUNC(8.9)"), 8.0);
    assert_eq!(number("=TRUNC(-8.97,1)"), -8.9);
    assert_eq!(number("=TRUNC(1234,-2)"), 1_200.0);
    close("=EXP(1)", std::f64::consts::E);
    close("=LN(EXP(2))", 2.0);
    assert_eq!(error("=LN(0)"), ErrorKind::Num);
    assert_eq!(number("=LOG(100)"), 2.0);
    assert_eq!(number("=LOG(8,2)"), 3.0);
    assert_eq!(error("=LOG(8,1)"), ErrorKind::Div0);
    assert_eq!(error("=LOG(-1)"), ErrorKind::Num);
    assert_eq!(number("=LOG10(1000)"), 3.0);
    assert_eq!(error("=LOG10(0)"), ErrorKind::Num);
    assert_eq!(number("=EVEN(1.5)"), 2.0);
    assert_eq!(number("=EVEN(-3)"), -4.0);
    assert_eq!(number("=EVEN(2)"), 2.0);
    assert_eq!(number("=ODD(2)"), 3.0);
    assert_eq!(number("=ODD(-1.5)"), -3.0);
    assert_eq!(number("=ODD(0)"), 1.0);
    assert_eq!(number("=FACT(5)"), 120.0);
    assert_eq!(error("=FACT(-1)"), ErrorKind::Num);
    assert_eq!(error("=FACT(171)"), ErrorKind::Num);
    assert_eq!(number("=GCD(24,36)"), 12.0);
    assert_eq!(number("=LCM(4,6)"), 12.0);
    assert_eq!(number("=LCM(0,6)"), 0.0);
    assert_eq!(error("=GCD(-2,4)"), ErrorKind::Num);
    assert_eq!(number("=QUOTIENT(-10,3)"), -3.0);
    assert_eq!(error("=QUOTIENT(1,0)"), ErrorKind::Div0);
    assert_eq!(number("=SUMSQ(3,4)"), 25.0);
    close("=DEGREES(RADIANS(90))", 90.0);
    close("=SIN(PI()/2)", 1.0);
    close("=COS(0)", 1.0);
    close("=TAN(0)", 0.0);
    close("=ASIN(1)", std::f64::consts::FRAC_PI_2);
    close("=ACOS(1)", 0.0);
    assert_eq!(error("=ASIN(2)"), ErrorKind::Num);
    close("=ATAN(1)", std::f64::consts::FRAC_PI_4);
}

#[test]
fn computes_more_statistics() {
    assert_eq!(number("=MEDIAN(A1:A5)"), 20.0);
    assert_eq!(number("=MEDIAN(1,2,3,4)"), 2.5);
    assert_eq!(error("=MEDIAN(B1:B2)"), ErrorKind::Num);
    assert_eq!(number("=LARGE(A1:A6,1)"), 30.0);
    assert_eq!(number("=SMALL(A1:A6,2)"), 10.0);
    assert_eq!(error("=LARGE(A1:A6,9)"), ErrorKind::Num);
    assert_eq!(number("=RANK(20,A1:A5)"), 2.0);
    assert_eq!(number("=RANK.EQ(20,A1:A5,1)"), 3.0);
    assert_eq!(error("=RANK(99,A1:A5)"), ErrorKind::NotAvailable);
    close("=STDEV(2,4,4,4,5,5,7,9)", 2.138_089_935);
    close("=STDEV.P(2,4,4,4,5,5,7,9)", 2.0);
    close("=STDEVP(2,4,4,4,5,5,7,9)", 2.0);
    close("=VAR.S(1,2,3,4)", 1.666_666_667);
    close("=VAR(1,2,3,4)", 1.666_666_667);
    close("=VARP(1,2,3,4)", 1.25);
    close("=VAR.P(1,2,3,4)", 1.25);
    assert_eq!(error("=STDEV(1)"), ErrorKind::Div0);
    assert_eq!(number("=MODE(A1:A5)"), 20.0);
    assert_eq!(number("=MODE.SNGL(1,2,2,3,3)"), 2.0);
    assert_eq!(error("=MODE(1,2,3)"), ErrorKind::NotAvailable);
    assert_eq!(number("=MAXIFS(A1:A5,C1:C5,\"Nord\")"), 30.0);
    assert_eq!(number("=MINIFS(A1:A5,C1:C5,\"Nord\")"), 10.0);
    assert_eq!(
        number("=AVERAGEIFS(A1:A5,C1:C5,\"Nord\",A1:A5,\">10\")"),
        25.0
    );
    assert_eq!(number("=MAXIFS(A1:A5,C1:C5,\"Ovest\")"), 0.0);
    assert_eq!(error("=AVERAGEIFS(A1:A5,C1:C5,\"Ovest\")"), ErrorKind::Div0);
    assert_eq!(error("=MAXIFS(A1:A5,C1:C5)"), ErrorKind::Value);
}

#[test]
fn chooses_and_tells_values_apart() {
    assert_eq!(text("=IFS(A1>15,\"alto\",A1>5,\"medio\")"), "medio");
    assert_eq!(error("=IFS(A1>99,1)"), ErrorKind::NotAvailable);
    assert_eq!(error("=IFS(A1>99)"), ErrorKind::Value);
    assert_eq!(text("=SWITCH(C2,\"Nord\",\"N\",\"Sud\",\"S\")"), "S");
    assert_eq!(text("=SWITCH(C2,\"Est\",\"E\",\"altro\")"), "altro");
    assert_eq!(error("=SWITCH(C2,\"Est\",\"E\")"), ErrorKind::NotAvailable);
    assert_eq!(number("=SWITCH(2,1,10,2,20)"), 20.0);
    assert_eq!(error("=SWITCH(1/0,1,2)"), ErrorKind::Div0);
    assert_eq!(value("=XOR(TRUE,FALSE,TRUE)"), Value::Bool(false));
    assert_eq!(value("=XOR(TRUE,FALSE)"), Value::Bool(true));
    assert_eq!(text("=IFNA(NA(),\"manca\")"), "manca");
    assert_eq!(number("=IFNA(5,0)"), 5.0);
    assert_eq!(error("=NA()"), ErrorKind::NotAvailable);
    assert_eq!(value("=ISNA(NA())"), Value::Bool(true));
    assert_eq!(value("=ISERR(NA())"), Value::Bool(false));
    assert_eq!(value("=ISERR(1/0)"), Value::Bool(true));
    assert_eq!(value("=ISLOGICAL(TRUE)"), Value::Bool(true));
    assert_eq!(value("=ISNONTEXT(A1)"), Value::Bool(true));
    assert_eq!(value("=ISEVEN(4)"), Value::Bool(true));
    assert_eq!(value("=ISODD(4)"), Value::Bool(false));
    assert_eq!(error("=ISEVEN(\"x\")"), ErrorKind::Value);
}

#[test]
fn works_on_text() {
    assert_eq!(text("=TEXT(1234.5,\"#,##0.00\")"), "1,234.50");
    assert_eq!(text("=TEXT(1234.5,\"#.##0,00 €\")"), "1.234,50 €");
    assert_eq!(text("=TEXT(1234.5,\"#.##0\")"), "1.235");
    assert_eq!(text("=TEXT(0.5,\"0,0\")"), "0,5");
    assert_eq!(text("=TEXT(1234.5,\"#,##0\")"), "1,235");
    assert_eq!(text("=TEXT(D1,\"dd/mm/yyyy\")"), "27/10/2026");
    assert_eq!(text("=TEXT(D1,\"gg mmmm aaaa\")"), "27 ottobre 2026");
    assert_eq!(text("=TEXT(\"abc\",\"@!\")"), "abc!");
    assert_eq!(error("=TEXT(1/0,\"0\")"), ErrorKind::Div0);
    assert_eq!(number("=VALUE(\"1,234.5\")"), 1_234.5);
    assert_eq!(number("=VALUE(\"1.234,5\")"), 1_234.5);
    assert_eq!(number("=VALUE(\"12%\")"), 0.12);
    assert_eq!(number("=VALUE(\"27/10/2026\")"), 46_322.0);
    assert_eq!(number("=VALUE(\"12:00\")"), 0.5);
    assert_eq!(error("=VALUE(\"abc\")"), ErrorKind::Value);
    assert_eq!(text("=SUBSTITUTE(\"a-b-c\",\"-\",\"+\")"), "a+b+c");
    assert_eq!(text("=SUBSTITUTE(\"a-b-c\",\"-\",\"+\",2)"), "a-b+c");
    assert_eq!(text("=SUBSTITUTE(\"a-b-c\",\"-\",\"+\",5)"), "a-b-c");
    assert_eq!(text("=SUBSTITUTE(\"abc\",\"\",\"x\")"), "abc");
    assert_eq!(
        error("=SUBSTITUTE(\"abc\",\"b\",\"x\",0)"),
        ErrorKind::Value
    );
    assert_eq!(text("=REPLACE(\"abcdef\",2,3,\"XY\")"), "aXYef");
    assert_eq!(text("=REPLACE(\"abc\",9,1,\"Z\")"), "abcZ");
    assert_eq!(error("=REPLACE(\"abc\",0,1,\"Z\")"), ErrorKind::Value);
    assert_eq!(number("=FIND(\"b\",\"abcb\")"), 2.0);
    assert_eq!(number("=FIND(\"b\",\"abcb\",3)"), 4.0);
    assert_eq!(error("=FIND(\"B\",\"abc\")"), ErrorKind::Value);
    assert_eq!(number("=SEARCH(\"B\",\"abc\")"), 2.0);
    assert_eq!(number("=SEARCH(\"c?e\",\"abcde\")"), 3.0);
    assert_eq!(error("=SEARCH(\"z\",\"abc\")"), ErrorKind::Value);
    assert_eq!(error("=FIND(\"a\",\"abc\",9)"), ErrorKind::Value);
    assert_eq!(text("=REPT(\"ab\",3)"), "ababab");
    assert_eq!(error("=REPT(\"ab\",99999)"), ErrorKind::Value);
    assert_eq!(
        text("=TEXTJOIN(\", \",TRUE,B1:B3,\"\",\"fine\")"),
        "Rossi, Bianchi, Verdi, fine"
    );
    assert_eq!(text("=TEXTJOIN(\"-\",FALSE,\"a\",\"\",\"b\")"), "a--b");
    assert_eq!(value("=EXACT(\"Ab\",\"Ab\")"), Value::Bool(true));
    assert_eq!(value("=EXACT(\"Ab\",\"ab\")"), Value::Bool(false));
    assert_eq!(
        text("=PROPER(\"mario ROSSI-bianchi\")"),
        "Mario Rossi-Bianchi"
    );
    assert_eq!(text("=CHAR(65)"), "A");
    assert_eq!(error("=CHAR(0)"), ErrorKind::Value);
    assert_eq!(number("=CODE(\"A\")"), 65.0);
    assert_eq!(error("=CODE(\"\")"), ErrorKind::Value);
    assert_eq!(text("=CLEAN(\"a\"&CHAR(9)&\"b\")"), "ab");
    assert_eq!(text("=T(\"x\")"), "x");
    assert_eq!(text("=T(5)"), "");
    assert_eq!(number("=N(TRUE)"), 1.0);
    assert_eq!(number("=N(\"x\")"), 0.0);
    assert_eq!(number("=N(7)"), 7.0);
    assert_eq!(error("=N(1/0)"), ErrorKind::Div0);
}

#[test]
fn looks_values_up() {
    assert_eq!(text("=CHOOSE(2,\"a\",\"b\",\"c\")"), "b");
    assert_eq!(error("=CHOOSE(4,\"a\",\"b\",\"c\")"), ErrorKind::Value);
    assert_eq!(number("=ROWS(A1:C5)"), 5.0);
    assert_eq!(number("=COLUMNS(A1:C5)"), 3.0);
    assert_eq!(number("=ROWS(A:A)"), 1_048_576.0);
    assert_eq!(number("=ROWS(5)"), 1.0);
    assert_eq!(number("=ROW(B7)"), 7.0);
    assert_eq!(number("=COLUMN(C2:D9)"), 3.0);
    assert_eq!(number("=ROW()"), 8.0, "the cell of the formula, H8");
    assert_eq!(number("=COLUMN()"), 8.0);
    assert_eq!(error("=ROW(1+1)"), ErrorKind::Value);
    assert_eq!(text("=XLOOKUP(20,A1:A5,B1:B5)"), "Bianchi");
    assert_eq!(
        text("=XLOOKUP(20,A1:A5,B1:B5,,0,-1)"),
        "Verdi",
        "from the end"
    );
    assert_eq!(text("=XLOOKUP(99,A1:A5,B1:B5,\"nessuno\")"), "nessuno");
    assert_eq!(error("=XLOOKUP(99,A1:A5,B1:B5)"), ErrorKind::NotAvailable);
    assert_eq!(
        text("=XLOOKUP(25,A1:A5,B1:B5,,-1)"),
        "Bianchi",
        "the next smaller"
    );
    assert_eq!(
        text("=XLOOKUP(25,A1:A5,B1:B5,,1)"),
        "Neri",
        "the next larger"
    );
    assert_eq!(
        text("=XLOOKUP(\"ve*\",B1:B5,C1:C5,,2)"),
        "Nord",
        "a wildcard"
    );
    assert_eq!(number("=LOOKUP(350,E2:E4,A2:A4)"), 20.0);
    assert_eq!(number("=LOOKUP(\"Neri\",B4,A4)"), 30.0);
    assert_eq!(error("=LOOKUP(1,A4:A5)"), ErrorKind::NotAvailable);
}

#[test]
fn counts_dates_and_times() {
    assert_eq!(number("=DATEDIF(DATE(2020,3,15),D1,\"Y\")"), 6.0);
    assert_eq!(number("=DATEDIF(DATE(2020,3,15),D1,\"M\")"), 79.0);
    assert_eq!(number("=DATEDIF(DATE(2026,10,1),D1,\"D\")"), 26.0);
    assert_eq!(number("=DATEDIF(DATE(2020,3,15),D1,\"YM\")"), 7.0);
    assert_eq!(number("=DATEDIF(DATE(2020,3,15),D1,\"MD\")"), 12.0);
    assert_eq!(
        number("=DATEDIF(DATE(2026,1,31),DATE(2026,3,1),\"MD\")"),
        1.0
    );
    assert_eq!(number("=DATEDIF(DATE(2020,3,15),D1,\"YD\")"), 226.0);
    assert_eq!(error("=DATEDIF(D1,DATE(2020,1,1),\"Y\")"), ErrorKind::Num);
    assert_eq!(error("=DATEDIF(DATE(2020,1,1),D1,\"Q\")"), ErrorKind::Num);
    assert_eq!(
        number("=EDATE(DATE(2026,1,31),1)"),
        number("=DATE(2026,2,28)")
    );
    assert_eq!(number("=EDATE(D1,-12)"), number("=DATE(2025,10,27)"));
    assert_eq!(number("=EOMONTH(D1,0)"), number("=DATE(2026,10,31)"));
    assert_eq!(number("=EOMONTH(D1,4)"), number("=DATE(2027,2,28)"));
    assert_eq!(number("=WEEKDAY(D1)"), 3.0, "a Tuesday from Sunday");
    assert_eq!(number("=WEEKDAY(D1,2)"), 2.0);
    assert_eq!(number("=WEEKDAY(D1,3)"), 1.0);
    assert_eq!(number("=WEEKDAY(D1,13)"), 7.0, "from Wednesday");
    assert_eq!(error("=WEEKDAY(D1,9)"), ErrorKind::Num);
    assert_eq!(number("=WEEKNUM(D1)"), 44.0);
    assert_eq!(number("=WEEKNUM(D1,2)"), 44.0);
    assert_eq!(number("=ISOWEEKNUM(D1)"), 44.0);
    assert_eq!(number("=ISOWEEKNUM(DATE(2027,1,1))"), 53.0);
    assert_eq!(number("=WEEKNUM(DATE(2026,1,1))"), 1.0);
    assert_eq!(error("=WEEKNUM(D1,5)"), ErrorKind::Num);
    assert_eq!(
        number("=NETWORKDAYS(DATE(2026,10,26),DATE(2026,11,6))"),
        10.0
    );
    assert_eq!(
        number("=NETWORKDAYS(DATE(2026,10,26),DATE(2026,11,6),DATE(2026,11,2))"),
        9.0
    );
    assert_eq!(
        number("=NETWORKDAYS(DATE(2026,11,6),DATE(2026,10,26))"),
        -10.0
    );
    assert_eq!(number("=WORKDAY(D1,4)"), number("=DATE(2026,11,2)"));
    assert_eq!(number("=WORKDAY(D1,-2)"), number("=DATE(2026,10,23)"));
    assert_eq!(number("=DAYS(DATE(2026,12,25),D1)"), 59.0);
    assert_eq!(number("=HOUR(0.75)"), 18.0);
    assert_eq!(number("=MINUTE(TIME(14,35,10))"), 35.0);
    assert_eq!(number("=SECOND(TIME(14,35,10))"), 10.0);
    assert_eq!(error("=HOUR(-1)"), ErrorKind::Num);
    assert_eq!(number("=TIME(12,0,0)"), 0.5);
    close("=TIME(25,0,0)", 1.0 / 24.0);
    assert_eq!(error("=TIME(-1,0,0)"), ErrorKind::Num);
    assert_eq!(number("=DATEVALUE(\"27/10/2026\")"), 46_322.0);
    assert_eq!(number("=DATEVALUE(\"2026-10-27\")"), 46_322.0);
    assert_eq!(number("=DATEVALUE(\"27.10.26\")"), 46_322.0);
    assert_eq!(error("=DATEVALUE(\"31/02/x\")"), ErrorKind::Value);
    assert_eq!(error("=DATEVALUE(\"40/13/2026\")"), ErrorKind::Value);
    close(
        "=TIMEVALUE(\"18:00:30\")",
        (18.0 * 3_600.0 + 30.0) / 86_400.0,
    );
    assert_eq!(error("=TIMEVALUE(\"25:00\")"), ErrorKind::Value);
    close("=YEARFRAC(DATE(2026,1,1),DATE(2026,7,1))", 0.5);
    close("=YEARFRAC(DATE(2026,7,1),DATE(2026,1,1),3)", 181.0 / 365.0);
    close("=YEARFRAC(DATE(2026,1,1),DATE(2026,7,1),2)", 181.0 / 360.0);
    close("=YEARFRAC(DATE(2026,1,31),DATE(2026,3,31),4)", 60.0 / 360.0);
    close("=YEARFRAC(DATE(2026,1,1),DATE(2027,1,1),1)", 365.0 / 365.0);
    assert_eq!(
        error("=YEARFRAC(DATE(2026,1,1),DATE(2027,1,1),7)"),
        ErrorKind::Num
    );
}

#[test]
fn computes_loans_and_investments() {
    close("=PMT(0.08/12,10,10000)", -1_037.032_089);
    close("=PMT(0,10,1000)", -100.0);
    close("=PMT(0.08/12,10,10000,0,1)", -1_030.164_327);
    assert_eq!(error("=PMT(0.1,0,1000)"), ErrorKind::Num);
    close("=FV(0.06/12,10,-200,-500,1)", 2_581.403_374);
    close("=FV(0,10,-100)", 1_000.0);
    close("=PV(0.08/12,240,500)", -59_777.145_85);
    close("=PV(0,10,-100)", 1_000.0);
    close("=NPER(0.12/12,-100,-1000,10000,1)", 59.673_865_24);
    close("=NPER(0,-100,1000)", 10.0);
    assert_eq!(error("=NPER(0,0,1000)"), ErrorKind::Num);
    close("=NPV(0.1,-10000,3000,4200,6800)", 1_188.443_412);
    close("=IRR(E1:E4)", 0.088_963_15);
    assert_eq!(error("=IRR(A1:A2)"), ErrorKind::Num);
    close("=RATE(48,-200,8000)", 0.007_701_472);
    close("=IPMT(0.1/12,1,36,8000)", -66.666_666_67);
    close("=IPMT(0.1/12,2,36,8000)", -65.071_076_41);
    close("=PPMT(0.1/12,1,36,8000)", -191.470_830_88);
    close("=IPMT(0.1/12,1,36,8000,0,1)", 0.0);
    assert_eq!(error("=IPMT(0.1/12,40,36,8000)"), ErrorKind::Num);
}

#[test]
fn reads_italian_names_and_writes_the_prefixes_of_newer_functions() {
    let mut workbook = Workbook::new();
    let at = CellRef::parse("A1").unwrap();
    workbook
        .set_input(0, at, "=CERCA.X(20;B1:B5;C1:C5)", Locale::Italian)
        .unwrap();
    assert_eq!(
        crate::formula::print_formula(workbook.formula(0, at).unwrap()),
        "XLOOKUP(20,B1:B5,C1:C5)"
    );
    let bytes = crate::xlsx::write_xlsx(&workbook).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
    let mut sheet_xml = String::new();
    std::io::Read::read_to_string(
        &mut archive.by_name("xl/worksheets/sheet1.xml").unwrap(),
        &mut sheet_xml,
    )
    .unwrap();
    assert!(
        sheet_xml.contains("<f>_xlfn.XLOOKUP(20,B1:B5,C1:C5)</f>"),
        "{sheet_xml}"
    );
    let reopened = crate::xlsx::read_spreadsheet(
        &bytes,
        None,
        &rely_office_core::Limits::default(),
        Locale::Italian,
    )
    .unwrap();
    assert_eq!(
        crate::formula::print_formula(reopened.formula(0, at).unwrap()),
        "XLOOKUP(20,B1:B5,C1:C5)",
        "the prefix the file needs is gone after a read"
    );
    for (italian, english) in [
        ("=TESTO.UNISCI(\"-\";VERO;\"a\")", "TEXTJOIN"),
        ("=FINE.MESE(OGGI();0)", "EOMONTH"),
        ("=GIORNI.LAVORATIVI.TOT(1;2)", "NETWORKDAYS"),
        ("=RATA(0,1;2;3)", "PMT"),
        ("=MEDIANA(1;2)", "MEDIAN"),
    ] {
        workbook.set_input(0, at, italian, Locale::Italian).unwrap();
        assert!(
            crate::formula::print_formula(workbook.formula(0, at).unwrap()).starts_with(english),
            "{italian}"
        );
    }
}
