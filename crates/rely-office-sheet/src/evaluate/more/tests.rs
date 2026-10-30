// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use crate::address::CellRef;
use crate::formula::{FUNCTION_NAMES, function_names};
use crate::model::Workbook;
use crate::value::{ErrorKind, Locale, Value};

/// A1:A6 and B1:B6 two samples, C1:C5 cash flows and D1:D5 their dates, E1:E3 mixed values,
/// F1:F3 rates.
const DATA: &[(&str, &str)] = &[
    ("A1", "2"),
    ("A2", "4"),
    ("A3", "4"),
    ("A4", "4"),
    ("A5", "5"),
    ("A6", "7"),
    ("B1", "1"),
    ("B2", "3"),
    ("B3", "2"),
    ("B4", "5"),
    ("B5", "4"),
    ("B6", "6"),
    ("C1", "-10000"),
    ("C2", "2750"),
    ("C3", "4250"),
    ("C4", "3250"),
    ("C5", "2750"),
    ("D1", "=DATE(2008,1,1)"),
    ("D2", "=DATE(2008,3,1)"),
    ("D3", "=DATE(2008,10,30)"),
    ("D4", "=DATE(2009,2,15)"),
    ("D5", "=DATE(2009,4,1)"),
    ("E1", "testo"),
    ("E2", "TRUE"),
    ("E3", "3"),
    ("F1", "0.09"),
    ("F2", "0.11"),
    ("F3", "0.1"),
];

fn value(formula: &str) -> Value {
    let mut workbook = Workbook::new();
    let inputs: Vec<(CellRef, String)> = DATA
        .iter()
        .map(|(address, typed)| (CellRef::parse(address).unwrap(), (*typed).to_owned()))
        .collect();
    workbook.set_inputs(0, &inputs, Locale::English).unwrap();
    let target = CellRef::parse("Z99").unwrap();
    workbook
        .set_input(0, target, formula, Locale::English)
        .unwrap();
    workbook.value(0, target)
}

fn close(formula: &str, expected: f64, tolerance: f64) {
    match value(formula) {
        Value::Number(number) => assert!(
            (number - expected).abs() <= tolerance,
            "{formula} gave {number}, expected {expected}"
        ),
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
        other => panic!("{formula} gave {other:?}"),
    }
}

#[test]
fn computes_hyperbolic_reciprocal_and_combinatorial_mathematics() {
    close("=SINH(1)", 1.175_201_193_643_801_4, 1e-12);
    close("=COSH(1)", 1.543_080_634_815_243_7, 1e-12);
    close("=TANH(0.5)", 0.462_117_157_260_009_8, 1e-12);
    close("=ACOSH(2)", 1.316_957_896_924_816_6, 1e-12);
    assert_eq!(error("=ACOSH(0.5)"), ErrorKind::Num);
    assert_eq!(error("=ATANH(1)"), ErrorKind::Num);
    close("=COT(1)", 0.642_092_615_934_330_7, 1e-12);
    close("=SEC(0)", 1.0, 1e-12);
    assert_eq!(error("=CSC(0)"), ErrorKind::Div0);
    close("=ACOT(1)", std::f64::consts::FRAC_PI_4, 1e-12);
    close("=ATAN2(1,1)", std::f64::consts::FRAC_PI_4, 1e-12);
    assert_eq!(error("=ATAN2(0,0)"), ErrorKind::Div0);
    close("=SQRTPI(1)", std::f64::consts::PI.sqrt(), 1e-12);
    close("=FACTDOUBLE(7)", 105.0, 0.0);
    close("=COMBIN(8,2)", 28.0, 0.0);
    close("=COMBINA(4,3)", 20.0, 0.0);
    close("=PERMUT(10,3)", 720.0, 0.0);
    close("=PERMUTATIONA(3,2)", 9.0, 0.0);
    close("=MULTINOMIAL(2,3,4)", 1_260.0, 0.0);
    assert_eq!(error("=COMBIN(2,3)"), ErrorKind::Num);
    close("=SUMX2MY2(A1:A3,B1:B3)", 22.0, 1e-12);
    close("=SUMX2PY2(A1:A3,B1:B3)", 50.0, 1e-12);
    close("=SUMXMY2(A1:A3,B1:B3)", 6.0, 1e-12);
    close("=CEILING.PRECISE(-4.3,2)", -4.0, 1e-12);
    close("=FLOOR.PRECISE(4.3,2)", 4.0, 1e-12);
    close("=ISO.CEILING(4.3)", 5.0, 1e-12);
    close("=DELTA(5,5)", 1.0, 0.0);
    close("=GESTEP(4,5)", 0.0, 0.0);
}

#[test]
fn writes_roman_and_other_bases() {
    assert_eq!(text("=ROMAN(1999)"), "MCMXCIX");
    assert_eq!(text("=ROMAN(0)"), "");
    assert_eq!(error("=ROMAN(4000)"), ErrorKind::Value);
    close("=ARABIC(\"MCMXCIX\")", 1_999.0, 0.0);
    close("=ARABIC(\"-XIV\")", -14.0, 0.0);
    assert_eq!(error("=ARABIC(\"ABC\")"), ErrorKind::Value);
    assert_eq!(text("=BASE(255,16)"), "FF");
    assert_eq!(text("=BASE(5,2,8)"), "00000101");
    close("=DECIMAL(\"FF\",16)", 255.0, 0.0);
    close("=DECIMAL(\"zz\",36)", 1_295.0, 0.0);
    assert_eq!(error("=DECIMAL(\"FF\",40)"), ErrorKind::Num);
}

#[test]
fn subtotals_by_code() {
    close("=SUBTOTAL(9,A1:A6)", 26.0, 1e-12);
    close("=SUBTOTAL(109,A1:A6)", 26.0, 1e-12);
    close("=SUBTOTAL(1,A1:A3)", 10.0 / 3.0, 1e-12);
    close("=SUBTOTAL(4,A1:A6)", 7.0, 0.0);
    assert_eq!(error("=SUBTOTAL(12,A1:A6)"), ErrorKind::Value);
    assert_eq!(error("=SUBTOTAL(50,A1:A6)"), ErrorKind::Value);
}

#[test]
fn computes_the_statistics_of_samples() {
    close("=AVERAGEA(E1:E3)", 4.0 / 3.0, 1e-12);
    close("=MAXA(E1:E3)", 3.0, 0.0);
    close("=MINA(E1:E3)", 0.0, 0.0);
    close("=VARA(E1:E3)", 7.0 / 3.0, 1e-12);
    close("=GEOMEAN(A1:A6)", 4.060_189_798_424_973_5, 1e-12);
    assert_eq!(error("=GEOMEAN(A1:A6,-1)"), ErrorKind::Num);
    close("=HARMEAN(A1:A6)", 3.766_816_143_497_758, 1e-12);
    close("=AVEDEV(A1:A6)", 1.111_111_111_111_111, 1e-12);
    close("=DEVSQ(A1:A6)", 13.333_333_333_333_332, 1e-9);
    close("=KURT(A1:A6)", 1.668_75, 1e-9);
    close("=SKEW(A1:A6)", 0.443_970_015_879_452_06, 1e-9);
    assert_eq!(error("=KURT(1,2,3)"), ErrorKind::Div0);
    close("=PERCENTILE(A1:A6,0.3)", 4.0, 1e-12);
    close("=PERCENTILE.INC(A1:A6,0.9)", 6.0, 1e-12);
    close("=PERCENTILE.EXC(A1:A6,0.25)", 3.5, 1e-12);
    assert_eq!(error("=PERCENTILE.EXC(A1:A6,0)"), ErrorKind::Num);
    close("=QUARTILE(A1:A6,1)", 4.0, 1e-12);
    close("=QUARTILE.EXC(A1:A6,3)", 5.5, 1e-12);
    close("=PERCENTRANK(A1:A6,4)", 0.2, 1e-12);
    close("=PERCENTRANK.INC(A1:A6,4.5)", 0.5, 1e-12);
    assert_eq!(error("=PERCENTRANK(A1:A6,10)"), ErrorKind::NotAvailable);
    close("=RANK.AVG(4,A1:A6)", 4.0, 1e-12);
    close("=RANK.AVG(4,A1:A6,1)", 3.0, 1e-12);
    assert_eq!(error("=RANK.AVG(3,A1:A6)"), ErrorKind::NotAvailable);
    close("=TRIMMEAN(A1:A6,0.4)", 4.25, 1e-12);
    close("=STANDARDIZE(42,40,1.5)", 1.333_333_333_333_333_3, 1e-12);
}

#[test]
fn relates_two_samples() {
    close("=CORREL(A1:A6,B1:B6)", 0.851_049_771_920_370_2, 1e-12);
    close("=PEARSON(A1:A6,B1:B6)", 0.851_049_771_920_370_2, 1e-12);
    close(
        "=RSQ(B1:B6,A1:A6)",
        0.851_049_771_920_370_2_f64.powi(2),
        1e-12,
    );
    close("=SLOPE(B1:B6,A1:A6)", 0.975, 1e-12);
    close("=INTERCEPT(B1:B6,A1:A6)", -0.725, 1e-12);
    close("=FORECAST(8,B1:B6,A1:A6)", 7.075, 1e-12);
    close("=FORECAST.LINEAR(8,B1:B6,A1:A6)", 7.075, 1e-12);
    close("=COVARIANCE.P(A1:A6,B1:B6)", 2.166_666_666_666_666_5, 1e-12);
    close("=COVAR(A1:A6,B1:B6)", 2.166_666_666_666_666_5, 1e-12);
    close("=COVARIANCE.S(A1:A6,B1:B6)", 2.6, 1e-12);
    close("=STEYX(B1:B6,A1:A6)", 1.098_294_131_824_439_7, 1e-9);
    assert_eq!(error("=CORREL(A1:A6,B1:B3)"), ErrorKind::NotAvailable);
}

#[test]
fn computes_the_distributions() {
    close("=NORM.DIST(42,40,1.5,TRUE)", 0.908_788_780_274_132, 1e-12);
    close(
        "=NORM.DIST(42,40,1.5,FALSE)",
        0.109_340_049_783_995_77,
        1e-12,
    );
    close("=NORMSDIST(1)", 0.841_344_746_068_542_9, 1e-12);
    close("=NORM.S.DIST(1,FALSE)", 0.241_970_724_519_143_37, 1e-12);
    close("=NORM.INV(0.908789,40,1.5)", 42.000_002, 1e-6);
    close("=NORM.S.INV(0.975)", 1.959_963_984_540_054, 1e-9);
    close("=NORM.S.INV(0.001)", -3.090_232_306_167_813_5, 1e-9);
    assert_eq!(error("=NORM.S.INV(1)"), ErrorKind::Num);
    close("=CONFIDENCE.NORM(0.05,2.5,50)", 0.692_951_912_174_1, 1e-9);
    close("=EXPON.DIST(0.2,10,TRUE)", 0.864_664_716_763_387, 1e-12);
    close("=EXPON.DIST(0.2,10,FALSE)", 1.353_352_832_366_127, 1e-12);
    close("=POISSON.DIST(2,5,FALSE)", 0.084_224_337_488_568_33, 1e-12);
    close("=POISSON.DIST(2,5,TRUE)", 0.124_652_019_483_081_13, 1e-12);
    close("=BINOM.DIST(6,10,0.5,FALSE)", 0.205_078_125, 1e-12);
    close("=BINOM.DIST(6,10,0.5,TRUE)", 0.828_125, 1e-12);
    close("=FISHER(0.75)", 0.972_955_074_527_656_6, 1e-12);
    close("=FISHERINV(0.972955)", 0.749_999_967_394_148_4, 1e-9);
    close("=GAMMALN(4.5)", 2.453_736_570_842_443, 1e-10);
    close("=GAMMA(5)", 24.0, 1e-9);
    close("=GAMMA(0.5)", std::f64::consts::PI.sqrt(), 1e-10);
    close("=GAMMA(-0.5)", -2.0 * std::f64::consts::PI.sqrt(), 1e-9);
    assert_eq!(error("=GAMMA(-1)"), ErrorKind::Num);
}

#[test]
fn depreciates_and_rates_loans() {
    close("=SLN(30000,7500,10)", 2_250.0, 1e-9);
    close("=SYD(30000,7500,10,1)", 4_090.909_090_909_091, 1e-6);
    close("=DDB(2400,300,10,1)", 480.0, 1e-9);
    close("=DDB(2400,300,10,2)", 384.0, 1e-9);
    close("=DB(1000000,100000,6,1,7)", 186_083.333_333_333_3, 1e-6);
    close("=DB(1000000,100000,6,2,7)", 259_639.416_666_666_7, 1e-6);
    close("=EFFECT(0.0525,4)", 0.053_542_667_370_758_6, 1e-12);
    close("=NOMINAL(0.053543,4)", 0.052_500_319_868_356_2, 1e-9);
    close("=ISPMT(0.1/12,1,36,8000000)", -64_814.814_814_814_81, 1e-6);
    close(
        "=CUMIPMT(0.09/12,30*12,125000,13,24,0)",
        -11_135.232_130_750_9,
        1e-4,
    );
    close(
        "=CUMPRINC(0.09/12,30*12,125000,13,24,0)",
        -934.107_123_420_8,
        1e-4,
    );
    close("=MIRR(C1:C5,0.1,0.12)", 0.117_313_314_127_077_46, 1e-9);
    close("=XNPV(0.09,C1:C5,D1:D5)", 2_086.647_602_031_535, 1e-6);
    close("=XIRR(C1:C5,D1:D5)", 0.373_362_535, 1e-6);
    close("=FVSCHEDULE(1,F1:F3)", 1.330_89, 1e-9);
    close("=PDURATION(0.025,2000,2200)", 3.859_866_162_622_655, 1e-9);
    close("=RRI(96,10000,11000)", 0.000_993_307_376_291_33, 1e-12);
    close("=DOLLARDE(1.02,16)", 1.125, 1e-12);
    close("=DOLLARFR(1.125,16)", 1.02, 1e-12);
}

#[test]
fn counts_days_of_custom_weekends() {
    close("=DAYS360(DATE(2011,1,30),DATE(2011,12,31))", 330.0, 0.0);
    close(
        "=DAYS360(DATE(2011,1,30),DATE(2011,12,31),TRUE)",
        330.0,
        0.0,
    );
    close("=DAYS360(DATE(2011,2,28),DATE(2011,3,31))", 30.0, 0.0);
    close(
        "=NETWORKDAYS.INTL(DATE(2006,1,1),DATE(2006,1,31))",
        22.0,
        0.0,
    );
    close(
        "=NETWORKDAYS.INTL(DATE(2006,1,1),DATE(2006,2,1),7)",
        24.0,
        0.0,
    );
    close(
        "=NETWORKDAYS.INTL(DATE(2006,1,1),DATE(2006,1,31),\"0000011\")",
        22.0,
        0.0,
    );
    close(
        "=NETWORKDAYS.INTL(DATE(2006,1,1),DATE(2006,1,31),11)",
        26.0,
        0.0,
    );
    close("=WORKDAY.INTL(DATE(2012,1,1),30,17)", 40_944.0, 0.0);
    close("=WORKDAY.INTL(DATE(2012,1,1),90,11)", 41_013.0, 0.0);
    assert_eq!(
        error("=WORKDAY.INTL(DATE(2012,1,1),30,\"1111111\")"),
        ErrorKind::Value
    );
    assert_eq!(
        error("=NETWORKDAYS.INTL(DATE(2006,1,1),DATE(2006,1,31),9)"),
        ErrorKind::Num
    );
}

#[test]
fn converts_between_bases_and_units() {
    close("=BIN2DEC(\"1100100\")", 100.0, 0.0);
    close("=BIN2DEC(\"1111111111\")", -1.0, 0.0);
    close("=BIN2DEC(1010)", 10.0, 0.0);
    assert_eq!(text("=DEC2BIN(9,4)"), "1001");
    assert_eq!(text("=DEC2BIN(-100)"), "1110011100");
    assert_eq!(error("=DEC2BIN(9,2)"), ErrorKind::Num);
    assert_eq!(error("=DEC2BIN(512)"), ErrorKind::Num);
    assert_eq!(text("=DEC2HEX(100,4)"), "0064");
    assert_eq!(text("=DEC2HEX(-54)"), "FFFFFFFFCA");
    close("=HEX2DEC(\"FFFFFFFF5B\")", -165.0, 0.0);
    assert_eq!(text("=HEX2BIN(\"F\",8)"), "00001111");
    assert_eq!(text("=OCT2HEX(\"100\")"), "40");
    assert_eq!(text("=DEC2OCT(58,3)"), "072");
    close("=BITAND(13,25)", 9.0, 0.0);
    close("=BITOR(23,10)", 31.0, 0.0);
    close("=BITXOR(5,3)", 6.0, 0.0);
    close("=BITLSHIFT(4,2)", 16.0, 0.0);
    close("=BITRSHIFT(13,2)", 3.0, 0.0);
    assert_eq!(error("=BITAND(-1,2)"), ErrorKind::Num);
    close("=CONVERT(1,\"lbm\",\"kg\")", 0.453_592_37, 1e-12);
    close("=CONVERT(68,\"F\",\"C\")", 20.0, 1e-9);
    close(
        "=CONVERT(100,\"km/h\",\"m/s\")",
        27.777_777_777_777_78,
        1e-9,
    );
    assert_eq!(
        error("=CONVERT(2.5,\"ft\",\"sec\")"),
        ErrorKind::NotAvailable
    );
    assert_eq!(
        error("=CONVERT(1,\"parsec\",\"m\")"),
        ErrorKind::NotAvailable
    );
}

#[test]
fn works_on_more_text() {
    assert_eq!(text("=UNICHAR(8364)"), "\u{20ac}");
    close("=UNICODE(\"A\")", 65.0, 0.0);
    assert_eq!(error("=UNICHAR(0)"), ErrorKind::Value);
    assert_eq!(text("=FIXED(1234.567,1)"), "1.234,6");
    assert_eq!(text("=FIXED(1234.567,-1)"), "1.230");
    assert_eq!(text("=FIXED(-1234.567,2,TRUE)"), "-1234,57");
    assert_eq!(text("=DOLLAR(1234.567)"), "1.234,57 \u{20ac}");
    close("=NUMBERVALUE(\"2.500,27\",\",\",\".\")", 2_500.27, 1e-9);
    close("=NUMBERVALUE(\"3.5%\")", 0.035, 1e-12);
    assert_eq!(text("=TEXTBEFORE(\"Rossi, Mario\",\", \")"), "Rossi");
    assert_eq!(text("=TEXTAFTER(\"a-b-c\",\"-\",-1)"), "c");
    assert_eq!(text("=TEXTAFTER(\"a-b-c\",\"-\",2)"), "c");
    assert_eq!(error("=TEXTBEFORE(\"abc\",\"-\")"), ErrorKind::NotAvailable);
    assert_eq!(text("=HYPERLINK(\"https://example.com\",\"Sito\")"), "Sito");
}

#[test]
fn describes_values() {
    assert_eq!(value("=ISREF(A1)"), Value::Bool(true));
    assert_eq!(value("=ISREF(1)"), Value::Bool(false));
    close("=TYPE(A1)", 1.0, 0.0);
    close("=TYPE(E1)", 2.0, 0.0);
    close("=TYPE(TRUE)", 4.0, 0.0);
    close("=TYPE(1/0)", 16.0, 0.0);
    close("=TYPE(A1:A3)", 64.0, 0.0);
    close("=ERROR.TYPE(1/0)", 2.0, 0.0);
    close("=ERROR.TYPE(NA())", 7.0, 0.0);
    assert_eq!(error("=ERROR.TYPE(1)"), ErrorKind::NotAvailable);
    assert_eq!(text("=ADDRESS(2,3)"), "$C$2");
    assert_eq!(text("=ADDRESS(2,3,2)"), "C$2");
    assert_eq!(text("=ADDRESS(2,3,4,TRUE,\"Foglio1\")"), "Foglio1!C2");
    assert_eq!(text("=ADDRESS(1,28,3)"), "$AB1");
}

#[test]
fn lists_every_function_it_computes() {
    for name in FUNCTION_NAMES {
        let formula = format!("={name}()");
        assert_ne!(
            value(&formula),
            Value::Error(ErrorKind::Name),
            "{name} is listed but unknown"
        );
    }
    assert!(FUNCTION_NAMES.len() > 280);
    let mut sorted = FUNCTION_NAMES.to_vec();
    sorted.sort_unstable();
    assert_eq!(sorted, FUNCTION_NAMES);

    let italian = function_names(Locale::Italian);
    assert!(italian.contains(&"SOMMA"));
    assert!(italian.contains(&"DISTRIB.NORM.N"));
    assert!(!italian.contains(&"SUM"));
    assert!(italian.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(function_names(Locale::English).contains(&"NORM.DIST"));
}

#[test]
fn reads_the_new_functions_in_italian() {
    let mut workbook = Workbook::new();
    let target = CellRef::parse("A1").unwrap();
    workbook
        .set_input(
            0,
            target,
            "=ARROTONDA.ECCESSO.PRECISA(4,3;2)",
            Locale::Italian,
        )
        .unwrap();
    assert_eq!(workbook.value(0, target), Value::Number(6.0));
    workbook
        .set_input(0, target, "=CONVERTI(1;\"kg\";\"g\")", Locale::Italian)
        .unwrap();
    assert_eq!(workbook.value(0, target), Value::Number(1_000.0));
}
