// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

use crate::address::CellRef;
use crate::model::Workbook;
use crate::value::{ErrorKind, Locale, Value};

/// A1:A10 and B1:B10 two samples, C1:C3 values and D1:D3 their probabilities, E1:F3 observed
/// and G1:H3 expected counts.
const DATA: &[(&str, &str)] = &[
    ("A1", "3"),
    ("A2", "4"),
    ("A3", "5"),
    ("A4", "8"),
    ("A5", "9"),
    ("A6", "1"),
    ("A7", "2"),
    ("A8", "4"),
    ("A9", "5"),
    ("A10", "6"),
    ("B1", "6"),
    ("B2", "19"),
    ("B3", "3"),
    ("B4", "2"),
    ("B5", "14"),
    ("B6", "4"),
    ("B7", "5"),
    ("B8", "17"),
    ("B9", "1"),
    ("B10", "7"),
    ("C1", "0"),
    ("C2", "1"),
    ("C3", "2"),
    ("D1", "0.2"),
    ("D2", "0.3"),
    ("D3", "0.5"),
    ("E1", "58"),
    ("E2", "11"),
    ("F1", "35"),
    ("F2", "25"),
    ("G1", "45.35"),
    ("G2", "17.56"),
    ("H1", "47.65"),
    ("H2", "18.44"),
    ("E3", "10"),
    ("F3", "23"),
    ("G3", "16.09"),
    ("H3", "16.91"),
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

fn error(formula: &str) -> ErrorKind {
    match value(formula) {
        Value::Error(kind) => kind,
        other => panic!("{formula} gave {other:?}"),
    }
}

#[test]
fn computes_the_error_function_and_gauss() {
    close("=ERF(0.745)", 0.707_928_920, 1e-9);
    close("=ERF(1,2)", 0.152_621_472, 1e-8);
    close("=ERF.PRECISE(1)", 0.842_700_793, 1e-9);
    close("=ERFC(1)", 0.157_299_207, 1e-9);
    close("=ERFC.PRECISE(-1)", 1.842_700_793, 1e-9);
    close("=GAUSS(2)", 0.477_249_868, 1e-9);
}

#[test]
fn computes_the_beta_distribution_and_its_inverse() {
    close("=BETA.DIST(2,8,10,TRUE,1,3)", 0.685_470_581, 1e-8);
    close("=BETA.DIST(2,8,10,FALSE,1,3)", 1.483_764_648, 1e-8);
    close("=BETADIST(2,8,10,1,3)", 0.685_470_581, 1e-8);
    close("=BETA.INV(0.685470581,8,10,1,3)", 2.0, 1e-7);
    close("=BETAINV(0.685470581,8,10,1,3)", 2.0, 1e-7);
    assert_eq!(error("=BETA.DIST(4,8,10,TRUE,1,3)"), ErrorKind::Num);
    assert_eq!(error("=BETA.DIST(2,0,10,TRUE)"), ErrorKind::Num);
    assert_eq!(error("=BETA.INV(0,8,10)"), ErrorKind::Num);
}

#[test]
fn computes_binomial_ranges_and_their_inverse() {
    close("=BINOM.DIST.RANGE(60,0.75,48)", 0.083_974_967, 1e-8);
    close("=BINOM.DIST.RANGE(60,0.75,45,50)", 0.523_629_793, 1e-8);
    close("=BINOM.INV(6,0.5,0.75)", 4.0, 0.0);
    close("=CRITBINOM(6,0.5,0.75)", 4.0, 0.0);
    assert_eq!(error("=BINOM.DIST.RANGE(60,0.75,50,45)"), ErrorKind::Num);
    assert_eq!(error("=BINOM.INV(6,1.5,0.75)"), ErrorKind::Num);
}

#[test]
fn computes_the_chi_squared_distribution_and_test() {
    close("=CHISQ.DIST(0.5,1,TRUE)", 0.520_499_877_8, 1e-9);
    close("=CHISQ.DIST(2,3,FALSE)", 0.207_553_749, 1e-9);
    close("=CHISQ.DIST.RT(18.307,10)", 0.050_000_589, 1e-9);
    close("=CHIDIST(18.307,10)", 0.050_000_589, 1e-9);
    close("=CHISQ.INV(0.93,1)", 3.283_020_287, 1e-8);
    close("=CHISQ.INV.RT(0.050001,10)", 18.306_973_13, 1e-6);
    close("=CHIINV(0.050001,10)", 18.306_973_13, 1e-6);
    close("=CHISQ.TEST(E1:F3,G1:H3)", 0.000_308_192, 1e-9);
    close("=CHITEST(E1:F3,G1:H3)", 0.000_308_192, 1e-9);
    assert_eq!(error("=CHISQ.DIST(-1,2,TRUE)"), ErrorKind::Num);
    assert_eq!(error("=CHISQ.TEST(E1:F3,G1:G3)"), ErrorKind::NotAvailable);
}

#[test]
fn computes_the_f_distribution_and_test() {
    close("=F.DIST(15.2069,6,4,TRUE)", 0.99, 1e-6);
    close("=F.DIST(15.2069,6,4,FALSE)", 0.001_223_792, 1e-8);
    close("=F.DIST.RT(15.2069,6,4)", 0.01, 1e-6);
    close("=FDIST(15.2069,6,4)", 0.01, 1e-6);
    close("=F.INV(0.01,6,4)", 0.109_309_914, 1e-7);
    close("=F.INV.RT(0.01,6,4)", 15.206_864_87, 1e-5);
    close("=FINV(0.01,6,4)", 15.206_864_87, 1e-5);
    close(
        "=F.TEST({6,7,9,15,21},{20,28,31,38,40})",
        0.648_317_847,
        1e-8,
    );
    close(
        "=FTEST({6,7,9,15,21},{20,28,31,38,40})",
        0.648_317_847,
        1e-8,
    );
    assert_eq!(error("=F.DIST(1,0,4,TRUE)"), ErrorKind::Num);
}

#[test]
fn computes_the_gamma_distribution_and_its_inverse() {
    close("=GAMMA.DIST(10.00001131,9,2,FALSE)", 0.032_639, 1e-6);
    close("=GAMMA.DIST(10.00001131,9,2,TRUE)", 0.068_094, 1e-6);
    close("=GAMMADIST(10.00001131,9,2,TRUE)", 0.068_094, 1e-6);
    close("=GAMMA.INV(0.068094,9,2)", 10.000_011_3, 1e-4);
    close("=GAMMAINV(0.068094,9,2)", 10.000_011_3, 1e-4);
    assert_eq!(error("=GAMMA.DIST(-1,9,2,TRUE)"), ErrorKind::Num);
}

#[test]
fn computes_the_hypergeometric_distribution() {
    close("=HYPGEOM.DIST(1,4,8,20,TRUE)", 0.465_428_277, 1e-9);
    close("=HYPGEOM.DIST(1,4,8,20,FALSE)", 0.363_261_094, 1e-9);
    close("=HYPGEOMDIST(1,4,8,20)", 0.363_261_094, 1e-9);
    assert_eq!(error("=HYPGEOM.DIST(5,4,8,20,TRUE)"), ErrorKind::Num);
}

#[test]
fn computes_the_lognormal_distribution_and_its_inverse() {
    close("=LOGNORM.DIST(4,3.5,1.2,TRUE)", 0.039_083_556, 1e-9);
    close("=LOGNORM.DIST(4,3.5,1.2,FALSE)", 0.017_617_597, 1e-9);
    close("=LOGNORMDIST(4,3.5,1.2)", 0.039_083_556, 1e-9);
    close("=LOGNORM.INV(0.039084,3.5,1.2)", 4.000_025_4, 1e-5);
    close("=LOGINV(0.039084,3.5,1.2)", 4.000_025_4, 1e-5);
    assert_eq!(error("=LOGNORM.DIST(0,3.5,1.2,TRUE)"), ErrorKind::Num);
}

#[test]
fn computes_the_negative_binomial_distribution() {
    close("=NEGBINOM.DIST(10,5,0.25,TRUE)", 0.313_514_058, 1e-9);
    close("=NEGBINOM.DIST(10,5,0.25,FALSE)", 0.055_048_660, 1e-9);
    close("=NEGBINOMDIST(10,5,0.25)", 0.055_048_660, 1e-9);
    assert_eq!(error("=NEGBINOM.DIST(10,0,0.25,TRUE)"), ErrorKind::Num);
}

#[test]
fn computes_students_t_distribution_its_inverse_and_test() {
    close("=T.DIST(60,1,TRUE)", 0.994_695_326, 1e-9);
    close("=T.DIST(8,3,FALSE)", 0.000_736_906, 1e-9);
    close("=T.DIST.2T(1.959999998,60)", 0.054_644_93, 1e-8);
    close("=T.DIST.RT(1.959999998,60)", 0.027_322_465, 1e-8);
    close("=TDIST(1.959999998,60,2)", 0.054_644_93, 1e-8);
    close("=TDIST(1.959999998,60,1)", 0.027_322_465, 1e-8);
    close("=T.INV(0.75,2)", 0.816_496_581, 1e-8);
    close("=T.INV.2T(0.546449,60)", 0.606_533_076, 1e-7);
    close("=TINV(0.546449,60)", 0.606_533_076, 1e-7);
    close("=T.TEST(A1:A9,B1:B9,2,1)", 0.196_015_785, 1e-8);
    close("=TTEST(A1:A9,B1:B9,2,1)", 0.196_015_785, 1e-8);
    close("=T.TEST(A1:A9,B1:B9,2,2)", 0.191_995_887, 1e-8);
    close("=T.TEST(A1:A9,B1:B9,1,3)", 0.101_146_962, 1e-8);
    assert_eq!(error("=T.DIST(1,0,TRUE)"), ErrorKind::Num);
    assert_eq!(error("=TDIST(-1,2,1)"), ErrorKind::Num);
    assert_eq!(error("=T.TEST(A1:A9,B1:B9,3,1)"), ErrorKind::Num);
}

#[test]
fn computes_weibull_and_the_z_test() {
    close("=WEIBULL.DIST(105,20,100,TRUE)", 0.929_581_390, 1e-9);
    close("=WEIBULL.DIST(105,20,100,FALSE)", 0.035_588_864, 1e-9);
    close("=WEIBULL(105,20,100,TRUE)", 0.929_581_390, 1e-9);
    close("=Z.TEST(A1:A10,4)", 0.187_641_574, 1e-8);
    close("=ZTEST(A1:A10,6)", 0.950_177_931, 1e-8);
    assert_eq!(error("=WEIBULL.DIST(-1,20,100,TRUE)"), ErrorKind::Num);
}

#[test]
fn computes_confidence_prob_and_population_skew() {
    close("=CONFIDENCE.T(0.05,1,50)", 0.284_196_855, 1e-8);
    close("=PROB(C1:C3,D1:D3,2)", 0.5, 1e-12);
    close("=PROB(C1:C3,D1:D3,1,2)", 0.8, 1e-12);
    close("=SKEW.P({3,4,5,2,3,4,5,6,4,7})", 0.303_193_339, 1e-8);
    assert_eq!(error("=CONFIDENCE.T(0.05,1,1)"), ErrorKind::Div0);
    assert_eq!(error("=PROB(C1:C3,C1:C3,1)"), ErrorKind::Num);
}
