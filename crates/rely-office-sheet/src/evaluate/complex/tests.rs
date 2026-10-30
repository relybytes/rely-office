// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use super::{parse_complex, part_text};
use crate::address::CellRef;
use crate::model::Workbook;
use crate::value::{ErrorKind, Locale, Value};

fn value(formula: &str) -> Value {
    let mut workbook = Workbook::new();
    workbook
        .set_input(0, CellRef::new(0, 0), "3+4i", Locale::English)
        .unwrap();
    workbook
        .set_input(0, CellRef::new(1, 0), "5-3i", Locale::English)
        .unwrap();
    let target = CellRef::parse("Z99").unwrap();
    workbook
        .set_input(0, target, formula, Locale::English)
        .unwrap();
    workbook.value(0, target)
}

fn text(formula: &str) -> String {
    match value(formula) {
        Value::Text(text) => text,
        other => panic!("{formula} gave {other:?}"),
    }
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

fn error(formula: &str) -> ErrorKind {
    match value(formula) {
        Value::Error(kind) => kind,
        other => panic!("{formula} gave {other:?}"),
    }
}

#[test]
fn reads_complex_numbers_as_excel_writes_them() {
    let parsed = |text: &str| {
        parse_complex(text).map(|number| (number.real, number.imaginary, number.suffix))
    };
    assert_eq!(parsed("3+4i"), Ok((3.0, 4.0, Some('i'))));
    assert_eq!(parsed("-2.5j"), Ok((0.0, -2.5, Some('j'))));
    assert_eq!(parsed("i"), Ok((0.0, 1.0, Some('i'))));
    assert_eq!(parsed("-i"), Ok((0.0, -1.0, Some('i'))));
    assert_eq!(parsed("1e2-1E-1i"), Ok((100.0, -0.1, Some('i'))));
    assert_eq!(parsed("7"), Ok((7.0, 0.0, None)));
    assert_eq!(parsed(""), Ok((0.0, 0.0, None)));
    assert_eq!(parsed("3+4k"), Err(ErrorKind::Num));
    assert_eq!(parsed("3 + 4i"), Err(ErrorKind::Num));
    assert_eq!(parsed("inf"), Err(ErrorKind::Num));
}

#[test]
fn writes_parts_with_fifteen_digits() {
    assert_eq!(part_text(0.0), "0");
    assert_eq!(part_text(0.833_730_025_131_149_1), "0.833730025131149");
    assert_eq!(part_text(-6.548_120_040_911), "-6.548120040911");
    assert_eq!(part_text(1.224_646_799_147_353e-16), "1.22464679914735E-16");
    assert_eq!(part_text(2.5e15), "2.5E+15");
    assert_eq!(part_text(46.0), "46");
}

#[test]
fn builds_and_takes_apart_complex_numbers() {
    assert_eq!(text("=COMPLEX(3,4)"), "3+4i");
    assert_eq!(text("=COMPLEX(3,4,\"j\")"), "3+4j");
    assert_eq!(text("=COMPLEX(0,1)"), "i");
    assert_eq!(text("=COMPLEX(0,-1)"), "-i");
    assert_eq!(text("=COMPLEX(1,0)"), "1");
    assert_eq!(text("=COMPLEX(0,0)"), "0");
    assert_eq!(text("=COMPLEX(3,-1)"), "3-i");
    assert_eq!(error("=COMPLEX(3,4,\"k\")"), ErrorKind::Value);
    close("=IMABS(\"5+12i\")", 13.0, 0.0);
    close("=IMAGINARY(\"3+4i\")", 4.0, 0.0);
    close("=IMAGINARY(\"0-j\")", -1.0, 0.0);
    close("=IMAGINARY(4)", 0.0, 0.0);
    close("=IMREAL(\"6-9i\")", 6.0, 0.0);
    close("=IMARGUMENT(\"3+4i\")", 0.927_295_218, 1e-9);
    assert_eq!(error("=IMARGUMENT(\"0\")"), ErrorKind::Div0);
    assert_eq!(text("=IMCONJUGATE(\"3+4i\")"), "3-4i");
    assert_eq!(error("=IMREAL(TRUE)"), ErrorKind::Value);
}

#[test]
fn computes_the_trigonometry_of_complex_numbers() {
    assert_eq!(
        text("=IMCOS(\"1+i\")"),
        "0.833730025131149-0.988897705762865i"
    );
    assert_eq!(
        text("=IMSIN(\"4+3i\")"),
        "-7.61923172032141-6.548120040911i"
    );
    assert_eq!(
        text("=IMSINH(\"4+3i\")"),
        "-27.0168132580039+3.85373803791938i"
    );
    assert_eq!(
        text("=IMCOSH(\"4+3i\")"),
        "-27.0349456030742+3.85115333481178i"
    );
    assert_eq!(
        text("=IMTAN(\"4+3i\")"),
        "0.00490825806749606+1.00070953606723i"
    );
    assert_eq!(
        text("=IMCOT(\"4+3i\")"),
        "0.00490118239430447-0.999266927805902i"
    );
    assert_eq!(
        text("=IMSEC(\"4+3i\")"),
        "-0.065294027857947-0.0752249603027732i"
    );
    assert_eq!(
        text("=IMCSC(\"4+3i\")"),
        "-0.0754898329158637+0.0648774713706355i"
    );
    assert_eq!(
        text("=IMSECH(\"4+3i\")"),
        "-0.0362534969158689-0.00516434460775318i"
    );
    assert_eq!(
        text("=IMCSCH(\"4+3i\")"),
        "-0.036275889628626-0.0051744731840194i"
    );
}

#[test]
fn computes_the_arithmetic_and_logarithms_of_complex_numbers() {
    assert_eq!(text("=IMDIV(\"-238+240i\",\"10+24i\")"), "5+12i");
    assert_eq!(error("=IMDIV(\"1+i\",0)"), ErrorKind::Num);
    assert_eq!(error("=IMDIV(\"1+i\",\"2j\")"), ErrorKind::Value);
    assert_eq!(
        text("=IMEXP(\"1+i\")"),
        "1.46869393991589+2.28735528717884i"
    );
    assert_eq!(
        text("=IMLN(\"3+4i\")"),
        "1.6094379124341+0.927295218001612i"
    );
    assert_eq!(error("=IMLN(0)"), ErrorKind::Num);
    assert_eq!(
        text("=IMLOG10(\"3+4i\")"),
        "0.698970004336019+0.402719196273373i"
    );
    assert_eq!(
        text("=IMLOG2(\"3+4i\")"),
        "2.32192809488736+1.33780421245098i"
    );
    close("=IMREAL(IMPOWER(\"2+3i\",3))", -46.0, 1e-12);
    close("=IMAGINARY(IMPOWER(\"2+3i\",3))", 9.0, 1e-12);
    assert_eq!(error("=IMPOWER(0,-1)"), ErrorKind::Num);
    assert_eq!(text("=IMPRODUCT(\"3+4i\",\"5-3i\")"), "27+11i");
    assert_eq!(text("=IMPRODUCT(A1:A2,2)"), "54+22i");
    assert_eq!(
        text("=IMSQRT(\"1+i\")"),
        "1.09868411346781+0.455089860562227i"
    );
    assert_eq!(text("=IMSUB(\"13+4i\",\"5+3i\")"), "8+i");
    assert_eq!(text("=IMSUM(\"3+4i\",\"5-3i\")"), "8+i");
    assert_eq!(text("=IMSUM(A1:A2)"), "8+i");
    assert_eq!(text("=IMSUM(\"2j\",1)"), "1+2j");
    assert_eq!(error("=IMSUM(\"2j\",\"i\")"), ErrorKind::Value);
}

#[test]
fn computes_the_bessel_functions() {
    close("=BESSELI(1.5,1)", 0.981_666_428, 1e-8);
    close("=BESSELJ(1.9,2)", 0.329_925_728, 1e-8);
    close("=BESSELJ(-1.9,3)", -0.113_423_407, 1e-8);
    close("=BESSELJ(20,1)", 0.066_833_124, 1e-7);
    close("=BESSELJ(20,30)", 0.000_124_015, 1e-8);
    close("=BESSELK(1.5,1)", 0.277_387_804, 1e-7);
    close("=BESSELK(3,2)", 0.061_510_458, 1e-7);
    close("=BESSELY(2.5,1)", 0.145_918_138, 1e-7);
    close("=BESSELY(10,3)", -0.251_362_657, 1e-7);
    assert_eq!(error("=BESSELY(0,1)"), ErrorKind::Num);
    assert_eq!(error("=BESSELJ(1,-1)"), ErrorKind::Num);
}
