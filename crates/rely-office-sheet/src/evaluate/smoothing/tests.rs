// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

use super::{detect_season, fit, series};
use crate::address::CellRef;
use crate::model::Workbook;
use crate::value::{ErrorKind, Locale, Value};

/// A series with a trend and a season of four, exact: every smoothing fits it.
const SEASONAL: [f64; 16] = [
    9.0, 15.0, 14.0, 23.0, 17.0, 23.0, 22.0, 31.0, 25.0, 31.0, 30.0, 39.0, 33.0, 39.0, 38.0, 47.0,
];
const NOISY: [f64; 18] = [
    12.0, 15.0, 14.0, 18.0, 20.0, 19.0, 25.0, 27.0, 24.0, 30.0, 33.0, 31.0, 36.0, 39.0, 37.0, 43.0,
    45.0, 44.0,
];

fn book(values: &[f64]) -> Workbook {
    let mut workbook = Workbook::new();
    let inputs: Vec<(CellRef, String)> = values
        .iter()
        .enumerate()
        .flat_map(|(index, value)| {
            let row = u32::try_from(index).unwrap();
            [
                (CellRef::new(row, 0), (index + 1).to_string()),
                (CellRef::new(row, 1), value.to_string()),
            ]
        })
        .collect();
    workbook.set_inputs(0, &inputs, Locale::English).unwrap();
    workbook
}

fn value(values: &[f64], formula: &str) -> Value {
    let mut workbook = book(values);
    let target = CellRef::parse("Z1").unwrap();
    workbook
        .set_input(0, target, formula, Locale::English)
        .unwrap();
    workbook.value(0, target)
}

fn number(values: &[f64], formula: &str) -> f64 {
    match value(values, formula) {
        Value::Number(number) => number,
        other => panic!("{formula} gave {other:?}"),
    }
}

#[test]
fn forecasts_an_exact_seasonal_series_as_libreoffice_does() {
    // LibreOffice gives 55 and 41, and a season of 4.
    let forecast = number(&SEASONAL, "=FORECAST.ETS(20,B1:B16,A1:A16)");
    assert!((forecast - 55.0).abs() < 1e-6, "{forecast}");
    let next = number(&SEASONAL, "=FORECAST.ETS(17,B1:B16,A1:A16)");
    assert!((next - 41.0).abs() < 1e-6, "{next}");
    assert!((number(&SEASONAL, "=FORECAST.ETS.SEASONALITY(B1:B16,A1:A16)") - 4.0).abs() < 1e-12);
    assert!((number(&SEASONAL, "=FORECAST.ETS.STAT(B1:B16,A1:A16,8)") - 1.0).abs() < 1e-12);
    let interval = number(&SEASONAL, "=FORECAST.ETS.CONFINT(20,B1:B16,A1:A16)");
    assert!(interval.abs() < 1e-6, "{interval}");
    // Halfway between two steps the forecast is halfway between theirs.
    let half = number(&SEASONAL, "=FORECAST.ETS(17.5,B1:B16,A1:A16)");
    let after = number(&SEASONAL, "=FORECAST.ETS(18,B1:B16,A1:A16)");
    assert!((half - f64::midpoint(next, after)).abs() < 1e-9);
}

#[test]
fn forecasts_a_noisy_series_with_sensible_statistics() {
    assert!((number(&NOISY, "=FORECAST.ETS.SEASONALITY(B1:B18,A1:A18)") - 3.0).abs() < 1e-12);
    let forecast = number(&NOISY, "=FORECAST.ETS(19,B1:B18,A1:A18)");
    assert!((44.0..52.0).contains(&forecast), "{forecast}");
    let without = number(&NOISY, "=FORECAST.ETS(19,B1:B18,A1:A18,0)");
    assert!((44.0..52.0).contains(&without), "{without}");
    let near = number(&NOISY, "=FORECAST.ETS.CONFINT(19,B1:B18,A1:A18)");
    let far = number(&NOISY, "=FORECAST.ETS.CONFINT(25,B1:B18,A1:A18)");
    assert!(near > 0.0 && far > near, "{near} {far}");
    let wider = number(&NOISY, "=FORECAST.ETS.CONFINT(19,B1:B18,A1:A18,0.99)");
    assert!(wider > near);
    for statistic in 1..=3 {
        let constant = number(
            &NOISY,
            &format!("=FORECAST.ETS.STAT(B1:B18,A1:A18,{statistic})"),
        );
        assert!((0.0..=1.0).contains(&constant), "{statistic}: {constant}");
    }
    let mae = number(&NOISY, "=FORECAST.ETS.STAT(B1:B18,A1:A18,6)");
    let rmse = number(&NOISY, "=FORECAST.ETS.STAT(B1:B18,A1:A18,7)");
    assert!(mae > 0.0 && rmse >= mae);
    assert!(number(&NOISY, "=FORECAST.ETS.STAT(B1:B18,A1:A18,4)") > 0.0);
    assert!(number(&NOISY, "=FORECAST.ETS.STAT(B1:B18,A1:A18,5)") > 0.0);
}

#[test]
fn refuses_what_it_cannot_forecast() {
    let error = |formula: &str| match value(&NOISY, formula) {
        Value::Error(kind) => kind,
        other => panic!("{formula} gave {other:?}"),
    };
    assert_eq!(error("=FORECAST.ETS(10,B1:B18,A1:A18)"), ErrorKind::Num);
    assert_eq!(
        error("=FORECAST.ETS(19,B1:B18,A1:A17)"),
        ErrorKind::NotAvailable
    );
    assert_eq!(error("=FORECAST.ETS(19,B1:B18,A1:A18,-1)"), ErrorKind::Num);
    assert_eq!(error("=FORECAST.ETS(19,B1:B18,A1:A18,10)"), ErrorKind::Num);
    assert_eq!(error("=FORECAST.ETS(19,B1:B18,A1:A18,1,2)"), ErrorKind::Num);
    assert_eq!(
        error("=FORECAST.ETS(19,B1:B18,A1:A18,1,1,8)"),
        ErrorKind::Num
    );
    assert_eq!(
        error("=FORECAST.ETS.CONFINT(19,B1:B18,A1:A18,1)"),
        ErrorKind::Num
    );
    assert_eq!(error("=FORECAST.ETS.STAT(B1:B18,A1:A18,9)"), ErrorKind::Num);
    assert_eq!(error("=FORECAST.ETS(19,B1:B2,A1:A2)"), ErrorKind::Num);
}

#[test]
fn builds_a_series_at_a_constant_step() {
    // Sorted, a missing step interpolated, two values on one date averaged.
    let built = series(
        &[3.0, 1.0, 5.0, 5.0],
        &[Some(30.0), Some(10.0), Some(40.0), Some(60.0)],
        1.0,
        1.0,
    )
    .unwrap();
    assert_eq!(built.values, vec![10.0, 30.0, 50.0]);
    assert!((built.step - 2.0).abs() < 1e-12);
    let filled = series(
        &[1.0, 2.0, 4.0],
        &[Some(1.0), Some(2.0), Some(4.0)],
        1.0,
        1.0,
    )
    .unwrap();
    assert_eq!(filled.values, vec![1.0, 2.0, 3.0, 4.0]);
    let zeros = series(
        &[1.0, 2.0, 4.0],
        &[Some(1.0), Some(2.0), Some(4.0)],
        0.0,
        1.0,
    )
    .unwrap();
    assert_eq!(zeros.values, vec![1.0, 2.0, 0.0, 4.0]);
    let missing = series(&[1.0, 2.0, 3.0], &[Some(1.0), None, Some(5.0)], 1.0, 1.0).unwrap();
    assert_eq!(missing.values, vec![1.0, 3.0, 5.0]);
    for (aggregation, expected) in [
        (2.0, 2.0),
        (3.0, 2.0),
        (4.0, 6.0),
        (5.0, 4.0),
        (6.0, 2.0),
        (7.0, 8.0),
    ] {
        let built = series(
            &[1.0, 1.0, 2.0, 3.0],
            &[Some(2.0), Some(6.0), Some(1.0), Some(1.0)],
            1.0,
            aggregation,
        )
        .unwrap();
        assert!((built.values[0] - expected).abs() < 1e-12, "{aggregation}");
    }
    assert_eq!(
        series(&[1.0, 2.0, 3.5], &[Some(1.0); 3], 1.0, 1.0),
        Err(ErrorKind::Num)
    );
}

#[test]
fn finds_no_season_in_a_straight_line() {
    let line: Vec<f64> = (0..20).map(|index| f64::from(index) * 3.0 + 1.0).collect();
    assert_eq!(detect_season(&line), 1);
    let model = fit(&line, 1).unwrap();
    assert!((model.forecast(2.0) - (line[19] + 6.0)).abs() < 1e-6);
    assert!(fit(&line[..4], 3).is_err());
}
