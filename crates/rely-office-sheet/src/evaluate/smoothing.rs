// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes S.r.l. <https://relybytes.com>

//! The forecasts of `FORECAST.ETS` and its three companions: Holt-Winters exponential smoothing
//! with an additive trend and an additive season, the version Excel calls AAA. The timeline is
//! sorted, values on the same date aggregated and missing steps filled; the season is found from
//! the autocorrelation of the series once its trend is removed; the three smoothing constants
//! are those that make the one-step forecasts of the history closest to it.

#![allow(
    clippy::float_cmp,
    reason = "options are whole numbers compared exactly, as Excel compares them"
)]

use super::Evaluator;
use super::more::normal_inverse;
use crate::formula::Expr;
use crate::value::{ErrorKind, Value};

type Answer = Option<Result<Value, ErrorKind>>;

/// The longest season Excel accepts, a year of hours.
const MAX_SEASON: usize = 8_760;
/// The most points a series may have once its missing steps are filled.
const MAX_POINTS: usize = 100_000;
/// The rounds of the search for the smoothing constants.
const SEARCH_ROUNDS: usize = 12;

/// A series at a constant step, ready to be smoothed.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Series {
    pub(super) values: Vec<f64>,
    /// The timeline's last point and its step.
    pub(super) last: f64,
    pub(super) step: f64,
}

/// How values on the same date are combined, Excel's `aggregation` from 1 to 7.
fn aggregate(values: &[f64], aggregation: f64) -> f64 {
    #[expect(clippy::cast_precision_loss, reason = "a handful of values")]
    let count = values.len() as f64;
    match aggregation {
        2.0 | 3.0 => count,
        4.0 => values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        5.0 => {
            let mut sorted = values.to_vec();
            sorted.sort_by(f64::total_cmp);
            let middle = sorted.len() / 2;
            if sorted.len().is_multiple_of(2) {
                f64::midpoint(sorted[middle - 1], sorted[middle])
            } else {
                sorted[middle]
            }
        }
        6.0 => values.iter().copied().fold(f64::INFINITY, f64::min),
        7.0 => values.iter().sum(),
        _ => values.iter().sum::<f64>() / count,
    }
}

/// A timeline and its values as a series at a constant step: sorted, aggregated by date, the
/// missing steps filled by interpolation (`completion` 1) or with zeros (0).
pub(super) fn series(
    timeline: &[f64],
    values: &[Option<f64>],
    completion: f64,
    aggregation: f64,
) -> Result<Series, ErrorKind> {
    if !(completion == 0.0 || completion == 1.0) || !(1.0..=7.0).contains(&aggregation) {
        return Err(ErrorKind::Num);
    }
    let mut points: Vec<(f64, Option<f64>)> = timeline
        .iter()
        .copied()
        .zip(values.iter().copied())
        .collect();
    points.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut dates: Vec<(f64, Vec<f64>)> = Vec::new();
    for (date, value) in points {
        match dates.last_mut() {
            Some((last, collected)) if *last == date => collected.extend(value),
            _ => dates.push((date, value.into_iter().collect())),
        }
    }
    if dates.len() < 3 {
        return Err(ErrorKind::Num);
    }
    let step = dates
        .windows(2)
        .map(|pair| pair[1].0 - pair[0].0)
        .fold(f64::INFINITY, f64::min);
    if !(step > 0.0 && step.is_finite()) {
        return Err(ErrorKind::Num);
    }
    let mut filled: Vec<Option<f64>> = Vec::new();
    let first = dates[0].0;
    for (date, collected) in &dates {
        let offset = (date - first) / step;
        if (offset - offset.round()).abs() > 1e-6 {
            return Err(ErrorKind::Num);
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a non-negative offset checked against the cap"
        )]
        let index = offset.round() as usize;
        if index >= MAX_POINTS {
            return Err(ErrorKind::Num);
        }
        filled.resize(index, None);
        filled.push(if collected.is_empty() {
            None
        } else {
            Some(aggregate(collected, aggregation))
        });
    }
    let known: Vec<usize> = (0..filled.len())
        .filter(|index| filled[*index].is_some())
        .collect();
    if known.len() < 2 {
        return Err(ErrorKind::Num);
    }
    let values = (0..filled.len())
        .map(|index| {
            if let Some(value) = filled[index] {
                return value;
            }
            if completion == 0.0 {
                return 0.0;
            }
            let after = known.partition_point(|known| *known < index);
            match (
                after.checked_sub(1).map(|before| known[before]),
                known.get(after),
            ) {
                (Some(before), Some(&after)) => {
                    let (from, to) = (filled[before].unwrap_or(0.0), filled[after].unwrap_or(0.0));
                    #[expect(clippy::cast_precision_loss, reason = "indices below the cap")]
                    let share = (index - before) as f64 / (after - before) as f64;
                    from + (to - from) * share
                }
                (Some(before), None) => filled[before].unwrap_or(0.0),
                (None, Some(&after)) => filled[after].unwrap_or(0.0),
                (None, None) => 0.0,
            }
        })
        .collect();
    Ok(Series {
        values,
        last: dates[dates.len() - 1].0,
        step,
    })
}

/// The season of a series found from its autocorrelation once its linear trend is removed: the
/// lag with the strongest positive correlation that is a peak, or 1 when none stands out.
pub(super) fn detect_season(values: &[f64]) -> usize {
    let n = values.len();
    if n < 6 {
        return 1;
    }
    #[expect(clippy::cast_precision_loss, reason = "indices below the cap")]
    let xs: Vec<f64> = (0..n).map(|index| index as f64).collect();
    #[expect(clippy::cast_precision_loss, reason = "counts below the cap")]
    let count = n as f64;
    let mean_x = xs.iter().sum::<f64>() / count;
    let mean_y = values.iter().sum::<f64>() / count;
    let sxy: f64 = xs
        .iter()
        .zip(values)
        .map(|(x, y)| (x - mean_x) * (y - mean_y))
        .sum();
    let sxx: f64 = xs.iter().map(|x| (x - mean_x).powi(2)).sum();
    let slope = sxy / sxx;
    let residuals: Vec<f64> = xs
        .iter()
        .zip(values)
        .map(|(x, y)| y - mean_y - slope * (x - mean_x))
        .collect();
    let variance: f64 = residuals.iter().map(|value| value * value).sum();
    if variance
        <= 1e-12
            * values
                .iter()
                .map(|value| value * value)
                .sum::<f64>()
                .max(1.0)
    {
        return 1;
    }
    let correlation = |lag: usize| -> f64 {
        residuals[lag..]
            .iter()
            .zip(&residuals)
            .map(|(later, earlier)| later * earlier)
            .sum::<f64>()
            / variance
    };
    let last = (n / 2).min(MAX_SEASON);
    let correlations: Vec<f64> = (0..=last + 1)
        .map(|lag| if lag < n { correlation(lag) } else { 0.0 })
        .collect();
    let mut best = (1, 0.0);
    for lag in 2..=last {
        let peak =
            correlations[lag] > correlations[lag - 1] && correlations[lag] >= correlations[lag + 1];
        if peak && correlations[lag] > 0.3 && correlations[lag] > best.1 + 1e-9 {
            best = (lag, correlations[lag]);
        }
    }
    best.0
}

/// A fitted model: its constants, its states after the last point and the one-step errors of
/// the history.
#[derive(Debug, Clone)]
pub(super) struct Fit {
    pub(super) alpha: f64,
    pub(super) beta: f64,
    pub(super) gamma: f64,
    level: f64,
    trend: f64,
    /// The seasonal state of each phase, by the index of the last point that updated it.
    seasonal: Vec<f64>,
    season: usize,
    count: usize,
    pub(super) errors: Vec<f64>,
    pub(super) fitted: Vec<(f64, f64)>,
}

/// The initial level, trend and seasonal states, and the first point the smoothing updates.
fn initial(values: &[f64], season: usize) -> (f64, f64, Vec<f64>, usize) {
    if season <= 1 {
        return (values[0], values[1] - values[0], vec![0.0], 1);
    }
    let mean = |slice: &[f64]| {
        #[expect(clippy::cast_precision_loss, reason = "a season below the cap")]
        let count = slice.len() as f64;
        slice.iter().sum::<f64>() / count
    };
    let first = mean(&values[..season]);
    let second = mean(&values[season..2 * season]);
    #[expect(clippy::cast_precision_loss, reason = "a season below the cap")]
    let length = season as f64;
    let trend = (second - first) / length;
    let middle = (length - 1.0) / 2.0;
    let seasonal = (0..season)
        .map(|phase| {
            #[expect(clippy::cast_precision_loss, reason = "a season below the cap")]
            let phase_position = phase as f64;
            values[phase] - (first + (phase_position - middle) * trend)
        })
        .collect();
    (first + middle * trend, trend, seasonal, season)
}

fn smooth(values: &[f64], season: usize, alpha: f64, beta: f64, gamma: f64) -> Fit {
    let (mut level, mut trend, mut seasonal, start) = initial(values, season);
    let phases = season.max(1);
    let mut errors = Vec::with_capacity(values.len() - start);
    let mut fitted = Vec::with_capacity(values.len() - start);
    for (index, &actual) in values.iter().enumerate().skip(start) {
        let phase = index % phases;
        let season_value = if season > 1 { seasonal[phase] } else { 0.0 };
        let forecast = level + trend + season_value;
        errors.push(actual - forecast);
        fitted.push((actual, forecast));
        let previous = level;
        level = alpha * (actual - season_value) + (1.0 - alpha) * (level + trend);
        trend = beta * (level - previous) + (1.0 - beta) * trend;
        if season > 1 {
            seasonal[phase] = gamma * (actual - level) + (1.0 - gamma) * season_value;
        }
    }
    Fit {
        alpha,
        beta,
        gamma: if season > 1 { gamma } else { 0.0 },
        level,
        trend,
        seasonal,
        season,
        count: values.len(),
        errors,
        fitted,
    }
}

fn squared_error(fit: &Fit) -> f64 {
    fit.errors.iter().map(|error| error * error).sum()
}

/// The constant in `[0, 1]` that minimizes `cost`, by golden section.
fn golden(cost: impl Fn(f64) -> f64) -> f64 {
    let ratio = (5.0_f64.sqrt() - 1.0) / 2.0;
    let (mut low, mut high) = (0.0_f64, 1.0_f64);
    let mut left = high - ratio * (high - low);
    let mut right = low + ratio * (high - low);
    let (mut left_cost, mut right_cost) = (cost(left), cost(right));
    for _ in 0..40 {
        if left_cost <= right_cost {
            high = right;
            right = left;
            right_cost = left_cost;
            left = high - ratio * (high - low);
            left_cost = cost(left);
        } else {
            low = left;
            left = right;
            left_cost = right_cost;
            right = low + ratio * (high - low);
            right_cost = cost(right);
        }
    }
    let middle = f64::midpoint(low, high);
    // The ends are candidates too: golden section only narrows towards them.
    [0.0, middle, 1.0]
        .into_iter()
        .min_by(|first, second| cost(*first).total_cmp(&cost(*second)))
        .unwrap_or(middle)
}

/// The model of a series with the constants whose one-step forecasts come closest to it.
pub(super) fn fit(values: &[f64], season: usize) -> Result<Fit, ErrorKind> {
    if values.len() < 3 || (season > 1 && values.len() < 2 * season) {
        return Err(ErrorKind::Num);
    }
    let mut best = (0.5, 0.1, 0.1);
    let mut best_cost = f64::INFINITY;
    for alpha in [0.1, 0.3, 0.5, 0.7, 0.9] {
        for beta in [0.01, 0.1, 0.3] {
            for gamma in [0.01, 0.1, 0.3, 0.6] {
                let cost = squared_error(&smooth(values, season, alpha, beta, gamma));
                if cost < best_cost {
                    best_cost = cost;
                    best = (alpha, beta, gamma);
                }
            }
        }
    }
    let (mut alpha, mut beta, mut gamma) = best;
    for _ in 0..SEARCH_ROUNDS {
        alpha = golden(|candidate| squared_error(&smooth(values, season, candidate, beta, gamma)));
        beta = golden(|candidate| squared_error(&smooth(values, season, alpha, candidate, gamma)));
        if season > 1 {
            gamma =
                golden(|candidate| squared_error(&smooth(values, season, alpha, beta, candidate)));
        }
    }
    Ok(smooth(values, season, alpha, beta, gamma))
}

impl Fit {
    /// The forecast `steps` whole steps after the last point.
    fn forecast_at(&self, steps: usize) -> f64 {
        let season_value = if self.season > 1 {
            self.seasonal[(self.count - 1 + steps) % self.season]
        } else {
            0.0
        };
        #[expect(clippy::cast_precision_loss, reason = "steps below the cap")]
        let ahead = steps as f64;
        self.level + ahead * self.trend + season_value
    }

    /// The forecast at a number of steps after the last point, between two whole steps
    /// interpolated.
    pub(super) fn forecast(&self, steps: f64) -> f64 {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a non-negative horizon checked against the cap"
        )]
        let before = steps.floor() as usize;
        let share = steps - steps.floor();
        let first = self.forecast_at(before);
        if share == 0.0 {
            return first;
        }
        first + (self.forecast_at(before + 1) - first) * share
    }

    /// The mean square of the one-step errors.
    pub(super) fn variance(&self) -> f64 {
        #[expect(clippy::cast_precision_loss, reason = "counts below the cap")]
        let count = self.errors.len().max(1) as f64;
        squared_error(self) / count
    }

    /// Half the width of the prediction interval `steps` ahead at a confidence level, from the
    /// variance of a forecast that many steps out.
    pub(super) fn interval(&self, steps: f64, confidence: f64) -> Result<f64, ErrorKind> {
        let z = normal_inverse(0.5 + confidence / 2.0)?;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a non-negative horizon checked against the cap"
        )]
        let horizon = steps.ceil().max(1.0) as usize;
        let gamma_state = (1.0 - self.alpha) * self.gamma;
        let mut spread = 1.0;
        for ahead in 1..horizon {
            #[expect(clippy::cast_precision_loss, reason = "steps below the cap")]
            let ahead_value = ahead as f64;
            let seasonal = if self.season > 1 && ahead % self.season == 0 {
                gamma_state
            } else {
                0.0
            };
            let weight = self.alpha * (1.0 + ahead_value * self.beta) + seasonal;
            spread += weight * weight;
        }
        Ok(z * (self.variance() * spread).sqrt())
    }

    /// The statistic Excel numbers 4 to 7: MASE, SMAPE, MAE or RMSE of the one-step forecasts.
    pub(super) fn accuracy(&self, statistic: f64, values: &[f64]) -> f64 {
        #[expect(clippy::cast_precision_loss, reason = "counts below the cap")]
        let count = self.errors.len().max(1) as f64;
        let absolute = self.errors.iter().map(|error| error.abs()).sum::<f64>() / count;
        match statistic {
            4.0 => {
                #[expect(clippy::cast_precision_loss, reason = "counts below the cap")]
                let steps = (values.len() - 1).max(1) as f64;
                let naive = values
                    .windows(2)
                    .map(|pair| (pair[1] - pair[0]).abs())
                    .sum::<f64>()
                    / steps;
                absolute / naive
            }
            5.0 => {
                self.fitted
                    .iter()
                    .map(|(actual, forecast)| {
                        let scale = actual.abs() + forecast.abs();
                        if scale == 0.0 {
                            0.0
                        } else {
                            2.0 * (actual - forecast).abs() / scale
                        }
                    })
                    .sum::<f64>()
                    / count
            }
            6.0 => absolute,
            _ => self.variance().sqrt(),
        }
    }
}

impl Evaluator<'_> {
    /// The values and the timeline of an exponential smoothing, paired: a value that is not a
    /// number is missing, a date that is not one makes the whole call `#VALUE!`.
    fn smoothing_series(
        &self,
        sheet: usize,
        values: Option<&Expr>,
        timeline: Option<&Expr>,
        completion: f64,
        aggregation: f64,
    ) -> Result<Series, ErrorKind> {
        let values: Vec<Value> = self.grid(sheet, values)?.into_iter().flatten().collect();
        let dates: Vec<Value> = self.grid(sheet, timeline)?.into_iter().flatten().collect();
        if values.len() != dates.len() {
            return Err(ErrorKind::NotAvailable);
        }
        let mut timeline = Vec::with_capacity(dates.len());
        let mut numbers = Vec::with_capacity(values.len());
        for (date, value) in dates.into_iter().zip(values) {
            match date {
                Value::Number(date) => timeline.push(date),
                Value::Error(kind) => return Err(kind),
                _ => return Err(ErrorKind::Value),
            }
            numbers.push(match value {
                Value::Number(number) => Some(number),
                Value::Error(kind) => return Err(kind),
                _ => None,
            });
        }
        series(&timeline, &numbers, completion, aggregation)
    }

    /// An exponential smoothing function, or `None` when the name is none of them.
    #[expect(clippy::too_many_lines, reason = "one arm per function, each short")]
    pub(super) fn smoothing(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        if !name.starts_with("FORECAST.ETS") {
            return None;
        }
        let argument = |index: usize| arguments.get(index);
        let number_or = |index: usize, default: f64| match argument(index) {
            None => Ok(default),
            Some(Expr::Text(text)) if text.is_empty() => Ok(default),
            Some(expression) => self.scalar(sheet, expression).as_number(),
        };
        let optional = |index: usize, default: f64| number_or(index, default).map(f64::trunc);
        let season_of = |series: &Series, index: usize| -> Result<usize, ErrorKind> {
            let requested = optional(index, 1.0)?;
            if !(0.0..=8_760.0).contains(&requested) {
                return Err(ErrorKind::Num);
            }
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a season checked to lie within the cap"
            )]
            Ok(match requested {
                0.0 => 1,
                1.0 => detect_season(&series.values),
                season => season as usize,
            })
        };
        let horizon = |series: &Series| -> Result<f64, ErrorKind> {
            let target = self.number(sheet, argument(0))?;
            let steps = (target - series.last) / series.step;
            if steps < 0.0 || steps > 1e7 {
                return Err(ErrorKind::Num);
            }
            Ok(steps)
        };
        Some(match name {
            "FORECAST.ETS" => (|| {
                let series = self.smoothing_series(
                    sheet,
                    argument(1),
                    argument(2),
                    optional(4, 1.0)?,
                    optional(5, 1.0)?,
                )?;
                let steps = horizon(&series)?;
                let season = season_of(&series, 3)?;
                Ok(Value::Number(fit(&series.values, season)?.forecast(steps)))
            })(),
            "FORECAST.ETS.CONFINT" => (|| {
                let series = self.smoothing_series(
                    sheet,
                    argument(1),
                    argument(2),
                    optional(5, 1.0)?,
                    optional(6, 1.0)?,
                )?;
                let steps = horizon(&series)?;
                let confidence = number_or(3, 0.95)?;
                if !(confidence > 0.0 && confidence < 1.0) {
                    return Err(ErrorKind::Num);
                }
                let season = season_of(&series, 4)?;
                Ok(Value::Number(
                    fit(&series.values, season)?.interval(steps, confidence)?,
                ))
            })(),
            "FORECAST.ETS.SEASONALITY" => (|| {
                let series = self.smoothing_series(
                    sheet,
                    argument(0),
                    argument(1),
                    optional(2, 1.0)?,
                    optional(3, 1.0)?,
                )?;
                #[expect(clippy::cast_precision_loss, reason = "a season below the cap")]
                let season = detect_season(&series.values) as f64;
                Ok(Value::Number(season))
            })(),
            "FORECAST.ETS.STAT" => (|| {
                let series = self.smoothing_series(
                    sheet,
                    argument(0),
                    argument(1),
                    optional(4, 1.0)?,
                    optional(5, 1.0)?,
                )?;
                let statistic = self.number(sheet, argument(2))?.trunc();
                if !(1.0..=8.0).contains(&statistic) {
                    return Err(ErrorKind::Num);
                }
                if statistic == 8.0 {
                    return Ok(Value::Number(series.step));
                }
                let season = season_of(&series, 3)?;
                let model = fit(&series.values, season)?;
                Ok(Value::Number(match statistic {
                    1.0 => model.alpha,
                    2.0 => model.beta,
                    3.0 => model.gamma,
                    _ => model.accuracy(statistic, &series.values),
                }))
            })(),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests;
