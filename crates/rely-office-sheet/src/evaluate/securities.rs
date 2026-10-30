// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! Securities: the coupon dates of a bond and their day counts, its price, yield and duration,
//! accrued interest, the bonds whose first or last period is odd, discounted securities and
//! Treasury bills, and the French and variable depreciations. Days are counted on the five bases Excel names: 0 US 30/360, 1 actual/actual,
//! 2 actual/360, 3 actual/365, 4 European 30/360.

#![allow(
    clippy::float_cmp,
    reason = "bases, frequencies and whole periods are compared exactly, as Excel compares them"
)]

use super::Evaluator;
use crate::dates::{date_from_serial, serial_from_date};
use crate::formula::Expr;
use crate::value::{ErrorKind, Value};

type Answer = Option<Result<Value, ErrorKind>>;

fn leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn parts(serial: f64) -> Result<(i64, i64, i64), ErrorKind> {
    date_from_serial(serial).ok_or(ErrorKind::Num)
}

fn last_of_february(year: i64, month: i64, day: i64) -> bool {
    month == 2 && day == days_in_month(year, month)
}

/// Days between two dates on a 30/360 basis: the US one with its February rule, or the
/// European one.
fn days_360(start: f64, end: f64, european: bool) -> Result<f64, ErrorKind> {
    let (start_year, start_month, mut start_day) = parts(start)?;
    let (end_year, end_month, mut end_day) = parts(end)?;
    if european {
        start_day = start_day.min(30);
        end_day = end_day.min(30);
    } else {
        let start_february = last_of_february(start_year, start_month, start_day);
        if start_february && last_of_february(end_year, end_month, end_day) {
            end_day = 30;
        }
        if start_february || start_day == 31 {
            start_day = 30;
        }
        if end_day == 31 && start_day >= 30 {
            end_day = 30;
        }
    }
    #[expect(clippy::cast_precision_loss, reason = "a span of days is small")]
    let days = ((end_year - start_year) * 360
        + (end_month - start_month) * 30
        + (end_day - start_day)) as f64;
    Ok(days)
}

/// The fraction of a year between two dates on a basis, as `YEARFRAC` computes it.
pub(super) fn year_fraction(start: f64, end: f64, basis: i64) -> Result<f64, ErrorKind> {
    let (start, end) = if start <= end {
        (start, end)
    } else {
        (end, start)
    };
    Ok(match basis {
        0 => days_360(start, end, false)? / 360.0,
        4 => days_360(start, end, true)? / 360.0,
        2 => (end - start) / 360.0,
        3 => (end - start) / 365.0,
        1 => {
            if start == end {
                return Ok(0.0);
            }
            let (start_year, start_month, start_day) = parts(start)?;
            let (end_year, end_month, end_day) = parts(end)?;
            let within_a_year = start_year == end_year
                || (end_year == start_year + 1
                    && (start_month > end_month
                        || (start_month == end_month && start_day >= end_day)));
            let length = if within_a_year {
                let february = |year: i64| serial_from_date(year, 2, 29).filter(|_| leap(year));
                let holds_leap_day = [start_year, end_year]
                    .iter()
                    .any(|year| february(*year).is_some_and(|day| (start..=end).contains(&day)));
                if (start_year == end_year && leap(start_year)) || holds_leap_day {
                    366.0
                } else {
                    365.0
                }
            } else {
                let years: Vec<f64> = (start_year..=end_year)
                    .map(|year| if leap(year) { 366.0 } else { 365.0 })
                    .collect();
                #[expect(clippy::cast_precision_loss, reason = "few years")]
                let count = years.len() as f64;
                years.iter().sum::<f64>() / count
            };
            (end - start) / length
        }
        _ => return Err(ErrorKind::Num),
    })
}

/// A date moved by whole months; with `end_of_month` it lands on the month's last day.
fn add_months(serial: f64, months: i64, end_of_month: bool) -> Result<f64, ErrorKind> {
    let (year, month, day) = parts(serial)?;
    let total = year * 12 + (month - 1) + months;
    let (year, month) = (total.div_euclid(12), total.rem_euclid(12) + 1);
    let last = days_in_month(year, month);
    let day = if end_of_month { last } else { day.min(last) };
    serial_from_date(year, month, day).ok_or(ErrorKind::Num)
}

/// The coupon period a settlement falls in: the coupon dates before and after it and how many
/// coupons remain.
struct Coupons {
    previous: f64,
    next: f64,
    count: f64,
}

fn coupons(settlement: f64, maturity: f64, frequency: f64) -> Result<Coupons, ErrorKind> {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a frequency of 1, 2 or 4 divides twelve"
    )]
    let months = 12 / frequency as i64;
    let (year, month, day) = parts(maturity)?;
    let end_of_month = day == days_in_month(year, month);
    let mut next = maturity;
    let mut count = 1.0;
    loop {
        #[expect(clippy::cast_possible_truncation, reason = "a coupon count is small")]
        let previous = add_months(maturity, -(count as i64) * months, end_of_month)?;
        if previous <= settlement {
            return Ok(Coupons {
                previous,
                next,
                count,
            });
        }
        next = previous;
        count += 1.0;
    }
}

/// Days from `start` to `end` as a basis counts them.
fn day_count(start: f64, end: f64, basis: i64) -> Result<f64, ErrorKind> {
    match basis {
        0 => days_360(start, end, false),
        4 => days_360(start, end, true),
        _ => Ok(end - start),
    }
}

/// The days of a coupon period as a basis counts them.
fn period_days(previous: f64, next: f64, basis: i64, frequency: f64) -> f64 {
    match basis {
        1 => next - previous,
        3 => 365.0 / frequency,
        _ => 360.0 / frequency,
    }
}

/// The day counts of a bond's current period: from its start to settlement, its length, and
/// from settlement to its end.
struct Period {
    coupons: Coupons,
    accrued: f64,
    length: f64,
    remaining: f64,
}

fn period(settlement: f64, maturity: f64, frequency: f64, basis: i64) -> Result<Period, ErrorKind> {
    let coupons = coupons(settlement, maturity, frequency)?;
    let accrued = day_count(coupons.previous, settlement, basis)?;
    let length = period_days(coupons.previous, coupons.next, basis, frequency);
    let remaining = match basis {
        0 => length - accrued,
        4 => days_360(settlement, coupons.next, true)?,
        _ => coupons.next - settlement,
    };
    Ok(Period {
        coupons,
        accrued,
        length,
        remaining,
    })
}

/// The price per 100 of face value of a bond in its current period, accrued interest left out.
fn bond_price(
    rate: f64,
    yield_rate: f64,
    redemption: f64,
    frequency: f64,
    current: &Period,
) -> f64 {
    let coupon = 100.0 * rate / frequency;
    let fraction = current.remaining / current.length;
    let accrued = current.accrued / current.length * coupon;
    let n = current.coupons.count;
    if n == 1.0 {
        return (redemption + coupon) / (1.0 + fraction * yield_rate / frequency) - accrued;
    }
    let base = 1.0 + yield_rate / frequency;
    let mut price = redemption / base.powf(n - 1.0 + fraction);
    let mut k = 1.0;
    while k <= n {
        price += coupon / base.powf(k - 1.0 + fraction);
        k += 1.0;
    }
    price - accrued
}

/// The root of a decreasing function between brackets widened until they hold it.
fn solve_decreasing(
    target: f64,
    mut low: f64,
    mut high: f64,
    value: impl Fn(f64) -> f64,
) -> Result<f64, ErrorKind> {
    let mut widened = 0;
    while value(high) > target {
        low = high;
        high = high * 2.0 + 1.0;
        widened += 1;
        if widened > 60 {
            return Err(ErrorKind::Num);
        }
    }
    if value(low) < target {
        return Err(ErrorKind::Num);
    }
    for _ in 0..300 {
        let middle = f64::midpoint(low, high);
        if value(middle) > target {
            low = middle;
        } else {
            high = middle;
        }
        if high - low < 1e-14 {
            break;
        }
    }
    Ok(f64::midpoint(low, high))
}

/// The depreciation of a period by the double-declining balance, as `VDB` uses it.
fn declining(cost: f64, salvage: f64, life: f64, period: f64, factor: f64) -> f64 {
    let rate = (factor / life).min(1.0);
    let old = if rate >= 1.0 {
        if period == 1.0 { cost } else { 0.0 }
    } else {
        cost * (1.0 - rate).powf(period - 1.0)
    };
    let new = cost * (1.0 - rate).powf(period);
    let depreciation = if new < salvage {
        old - salvage
    } else {
        old - new
    };
    depreciation.max(0.0)
}

/// The depreciation from period 0 to `period`, switching to straight line once it is larger.
fn variable_declining(
    cost: f64,
    salvage: f64,
    life: f64,
    life_left: f64,
    period: f64,
    factor: f64,
) -> f64 {
    let end = period.ceil();
    let mut total = 0.0;
    let mut remaining = cost - salvage;
    let mut straight = false;
    let mut straight_line = 0.0;
    let mut index = 1.0;
    while index <= end {
        let mut term = if straight {
            straight_line
        } else {
            let declined = declining(cost, salvage, life, index, factor);
            straight_line = remaining / (life_left - (index - 1.0));
            if straight_line > declined {
                straight = true;
                straight_line
            } else {
                remaining -= declined;
                declined
            }
        };
        if index == end {
            term *= period + 1.0 - end;
        }
        total += term;
        index += 1.0;
    }
    total
}

/// The quasi-coupon periods an odd period spans, each as the part of it the odd period covers:
/// the days it counts, the days of it accrued at settlement, the days of it left after
/// settlement, and its normal length.
struct QuasiPeriod {
    counted: f64,
    accrued: f64,
    remaining: f64,
    length: f64,
}

/// The quasi-coupon periods from `start` to `end` stepped by `months`: backwards from `end`
/// for an odd first period, forwards from `start` for an odd last one.
fn quasi_periods(
    start: f64,
    end: f64,
    settlement: f64,
    months: i64,
    backwards: bool,
    basis: i64,
    frequency: f64,
) -> Result<Vec<QuasiPeriod>, ErrorKind> {
    let anchor = if backwards { end } else { start };
    let (year, month, day) = parts(anchor)?;
    let end_of_month = day == days_in_month(year, month);
    let mut bounds = vec![anchor];
    loop {
        let step = i64::try_from(bounds.len()).map_err(|_| ErrorKind::Num)?;
        let next = add_months(
            anchor,
            if backwards { -step } else { step } * months,
            end_of_month,
        )?;
        bounds.push(next);
        if (backwards && next <= start) || (!backwards && next >= end) {
            break;
        }
        if bounds.len() > 4_000 {
            return Err(ErrorKind::Num);
        }
    }
    if backwards {
        bounds.reverse();
    }
    bounds
        .windows(2)
        .map(|window| {
            let (from, to) = (window[0], window[1]);
            let (inside_from, inside_to) = (from.max(start), to.min(end));
            let counted = day_count(inside_from, inside_to, basis)?;
            let accrued = if settlement > inside_from {
                day_count(inside_from, settlement.min(inside_to), basis)?
            } else {
                0.0
            };
            let remaining = if settlement < inside_to {
                day_count(settlement.max(inside_from), inside_to, basis)?
            } else {
                0.0
            };
            Ok(QuasiPeriod {
                counted,
                accrued,
                remaining,
                length: period_days(from, to, basis, frequency),
            })
        })
        .collect()
}

/// A bond whose first period is odd, short or long: what `ODDFPRICE` prices.
struct OddFirst {
    rate: f64,
    redemption: f64,
    frequency: f64,
    /// The odd period's quasi-coupon periods, oldest first.
    periods: Vec<QuasiPeriod>,
    /// The regular coupons after the first one, up to maturity.
    regular: f64,
    /// The whole quasi-coupon periods between settlement and the first coupon.
    whole: f64,
    /// The part of its quasi-coupon period left after settlement.
    fraction: f64,
}

impl OddFirst {
    fn price(&self, yield_rate: f64) -> f64 {
        let coupon = 100.0 * self.rate / self.frequency;
        let base = 1.0 + yield_rate / self.frequency;
        let lead = self.whole + self.fraction;
        let odd: f64 = self
            .periods
            .iter()
            .map(|period| period.counted / period.length)
            .sum();
        let accrued: f64 = self
            .periods
            .iter()
            .map(|period| period.accrued / period.length)
            .sum();
        let mut price =
            self.redemption / base.powf(self.regular + lead) + coupon * odd / base.powf(lead);
        let mut k = 1.0;
        while k <= self.regular {
            price += coupon / base.powf(k + lead);
            k += 1.0;
        }
        price - coupon * accrued
    }
}

/// A bond whose last period is odd: the sums `ODDLPRICE` and `ODDLYIELD` read.
struct OddLast {
    counted: f64,
    accrued: f64,
    remaining: f64,
}

fn odd_last(periods: &[QuasiPeriod]) -> OddLast {
    let sum = |part: fn(&QuasiPeriod) -> f64| -> f64 {
        periods
            .iter()
            .map(|period| part(period) / period.length)
            .sum()
    };
    OddLast {
        counted: sum(|period| period.counted),
        accrued: sum(|period| period.accrued),
        remaining: sum(|period| period.remaining),
    }
}

/// A value rounded half away from zero, as the French depreciations round each period.
fn round(value: f64) -> f64 {
    value.round()
}

impl Evaluator<'_> {
    /// A securities function, or `None` when the name is none of them.
    #[expect(clippy::too_many_lines, reason = "one arm per function, each short")]
    pub(super) fn securities(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        let argument = |index: usize| arguments.get(index);
        let number = |index: usize| self.number(sheet, argument(index));
        let date = |index: usize| number(index).map(f64::trunc);
        let optional = |index: usize, default: f64| match argument(index) {
            Some(Expr::Text(text)) if text.is_empty() => Ok(default),
            Some(expression) => self.scalar(sheet, expression).as_number(),
            None => Ok(default),
        };
        let basis_at = |index: usize| -> Result<i64, ErrorKind> {
            let basis = optional(index, 0.0)?.trunc();
            if (0.0..=4.0).contains(&basis) {
                #[expect(clippy::cast_possible_truncation, reason = "a basis from 0 to 4")]
                Ok(basis as i64)
            } else {
                Err(ErrorKind::Num)
            }
        };
        let frequency_at = |index: usize| -> Result<f64, ErrorKind> {
            let frequency = number(index)?.trunc();
            if [1.0, 2.0, 4.0].contains(&frequency) {
                Ok(frequency)
            } else {
                Err(ErrorKind::Num)
            }
        };
        let bond = || -> Result<(f64, f64), ErrorKind> {
            let (settlement, maturity) = (date(0)?, date(1)?);
            if settlement >= maturity {
                return Err(ErrorKind::Num);
            }
            Ok((settlement, maturity))
        };
        let finite = |value: f64| {
            if value.is_finite() {
                Ok(Value::Number(value))
            } else {
                Err(ErrorKind::Num)
            }
        };
        Some(match name {
            "COUPDAYBS" | "COUPDAYS" | "COUPDAYSNC" | "COUPNCD" | "COUPNUM" | "COUPPCD" => (|| {
                let (settlement, maturity) = bond()?;
                let frequency = frequency_at(2)?;
                let basis = basis_at(3)?;
                let current = period(settlement, maturity, frequency, basis)?;
                finite(match name {
                    "COUPDAYBS" => current.accrued,
                    "COUPDAYS" => current.length,
                    "COUPDAYSNC" => current.remaining,
                    "COUPNCD" => current.coupons.next,
                    "COUPPCD" => current.coupons.previous,
                    _ => current.coupons.count,
                })
            })(
            ),
            "PRICE" => (|| {
                let (settlement, maturity) = bond()?;
                let (rate, yield_rate, redemption) = (number(2)?, number(3)?, number(4)?);
                let frequency = frequency_at(5)?;
                let basis = basis_at(6)?;
                if rate < 0.0 || yield_rate < 0.0 || redemption <= 0.0 {
                    return Err(ErrorKind::Num);
                }
                let current = period(settlement, maturity, frequency, basis)?;
                finite(bond_price(
                    rate, yield_rate, redemption, frequency, &current,
                ))
            })(),
            "YIELD" => (|| {
                let (settlement, maturity) = bond()?;
                let (rate, price, redemption) = (number(2)?, number(3)?, number(4)?);
                let frequency = frequency_at(5)?;
                let basis = basis_at(6)?;
                if rate < 0.0 || price <= 0.0 || redemption <= 0.0 {
                    return Err(ErrorKind::Num);
                }
                let current = period(settlement, maturity, frequency, basis)?;
                if current.coupons.count == 1.0 {
                    let coupon = rate / frequency;
                    let accrued = current.accrued / current.length * coupon;
                    let invested = price / 100.0 + accrued;
                    return finite(
                        ((redemption / 100.0 + coupon) - invested) / invested
                            * (frequency * current.length / current.remaining),
                    );
                }
                finite(solve_decreasing(
                    price,
                    -frequency + 1e-6,
                    1.0,
                    |yield_rate| bond_price(rate, yield_rate, redemption, frequency, &current),
                )?)
            })(),
            "DURATION" | "MDURATION" => (|| {
                let (settlement, maturity) = bond()?;
                let (coupon, yield_rate) = (number(2)?, number(3)?);
                let frequency = frequency_at(4)?;
                let basis = basis_at(5)?;
                if coupon < 0.0 || yield_rate < 0.0 {
                    return Err(ErrorKind::Num);
                }
                let current = period(settlement, maturity, frequency, basis)?;
                let fraction = current.remaining / current.length;
                let base = 1.0 + yield_rate / frequency;
                let (mut weighted, mut total) = (0.0, 0.0);
                let mut k = 1.0;
                while k <= current.coupons.count {
                    let time = k - 1.0 + fraction;
                    let mut flow = 100.0 * coupon / frequency;
                    if k == current.coupons.count {
                        flow += 100.0;
                    }
                    let present = flow / base.powf(time);
                    weighted += time * present;
                    total += present;
                    k += 1.0;
                }
                let duration = weighted / total / frequency;
                finite(if name == "MDURATION" {
                    duration / base
                } else {
                    duration
                })
            })(),
            "ACCRINT" => (|| {
                let (issue, first_interest, settlement) = (date(0)?, date(1)?, date(2)?);
                let (rate, par) = (number(3)?, number(4)?);
                let frequency = frequency_at(5)?;
                let basis = basis_at(6)?;
                let from_issue = match argument(7) {
                    None => true,
                    Some(Expr::Text(text)) if text.is_empty() => true,
                    Some(expression) => self.scalar(sheet, expression).as_bool()?,
                };
                if issue >= settlement || rate <= 0.0 || par <= 0.0 {
                    return Err(ErrorKind::Num);
                }
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "a frequency divides twelve"
                )]
                let months = 12 / frequency as i64;
                let (year, month, day) = parts(first_interest)?;
                let end_of_month = day == days_in_month(year, month);
                let quasi = |k: i64| add_months(first_interest, k * months, end_of_month);
                // The quasi-coupon period the accrual starts in, counted from the first interest.
                let start = if from_issue || settlement <= first_interest {
                    issue
                } else {
                    let mut k = 0;
                    while quasi(k + 1)? <= settlement {
                        k += 1;
                    }
                    quasi(k)?
                };
                let mut k = 0;
                while quasi(k)? > start {
                    k -= 1;
                }
                while quasi(k + 1)? <= start {
                    k += 1;
                }
                let mut sum = 0.0;
                loop {
                    let (period_start, period_end) = (quasi(k)?, quasi(k + 1)?);
                    if period_start >= settlement {
                        break;
                    }
                    let from = period_start.max(start);
                    let to = period_end.min(settlement);
                    sum += day_count(from, to, basis)?
                        / period_days(period_start, period_end, basis, frequency);
                    k += 1;
                }
                finite(par * rate / frequency * sum)
            })(),
            "ACCRINTM" => (|| {
                let (issue, settlement) = (date(0)?, date(1)?);
                let (rate, par) = (number(2)?, number(3)?);
                let basis = basis_at(4)?;
                if issue >= settlement || rate <= 0.0 || par <= 0.0 {
                    return Err(ErrorKind::Num);
                }
                finite(par * rate * year_fraction(issue, settlement, basis)?)
            })(),
            "DISC" | "INTRATE" | "RECEIVED" | "PRICEDISC" | "YIELDDISC" => (|| {
                let (settlement, maturity) = bond()?;
                let (first, second) = (number(2)?, number(3)?);
                let basis = basis_at(4)?;
                if first <= 0.0 || second <= 0.0 {
                    return Err(ErrorKind::Num);
                }
                let years = year_fraction(settlement, maturity, basis)?;
                finite(match name {
                    "DISC" => (second - first) / second / years,
                    "RECEIVED" => {
                        let rest = 1.0 - second * years;
                        if rest <= 0.0 {
                            return Err(ErrorKind::Num);
                        }
                        first / rest
                    }
                    "PRICEDISC" => second - first * second * years,
                    // INTRATE and YIELDDISC: the gain over what was paid, per year.
                    _ => (second - first) / first / years,
                })
            })(),
            "PRICEMAT" | "YIELDMAT" => (|| {
                let (settlement, maturity) = bond()?;
                let issue = date(2)?;
                let (rate, other) = (number(3)?, number(4)?);
                let basis = basis_at(5)?;
                if rate < 0.0 || other < 0.0 || issue > settlement {
                    return Err(ErrorKind::Num);
                }
                let issued_to_maturity = year_fraction(issue, maturity, basis)?;
                let issued_to_settlement = year_fraction(issue, settlement, basis)?;
                let settlement_to_maturity = year_fraction(settlement, maturity, basis)?;
                finite(if name == "PRICEMAT" {
                    (100.0 + issued_to_maturity * rate * 100.0)
                        / (1.0 + settlement_to_maturity * other)
                        - issued_to_settlement * rate * 100.0
                } else {
                    let invested = other / 100.0 + issued_to_settlement * rate;
                    ((1.0 + issued_to_maturity * rate) - invested)
                        / invested
                        / settlement_to_maturity
                })
            })(),
            "TBILLEQ" | "TBILLPRICE" | "TBILLYIELD" => (|| {
                let (settlement, maturity) = bond()?;
                let value = number(2)?;
                let days = maturity - settlement;
                if days > 366.0 || value <= 0.0 {
                    return Err(ErrorKind::Num);
                }
                finite(match name {
                    "TBILLEQ" => 365.0 * value / (360.0 - value * days),
                    "TBILLPRICE" => {
                        let price = 100.0 * (1.0 - value * days / 360.0);
                        if price <= 0.0 {
                            return Err(ErrorKind::Num);
                        }
                        price
                    }
                    _ => (100.0 - value) / value * 360.0 / days,
                })
            })(),
            "ODDFPRICE" | "ODDFYIELD" => (|| {
                let (settlement, maturity) = bond()?;
                let (issue, first_coupon) = (date(2)?, date(3)?);
                let (rate, given, redemption) = (number(4)?, number(5)?, number(6)?);
                let frequency = frequency_at(7)?;
                let basis = basis_at(8)?;
                if !(issue < settlement && settlement < first_coupon && first_coupon <= maturity)
                    || rate < 0.0
                    || given < 0.0
                    || redemption <= 0.0
                    || (name == "ODDFYIELD" && given == 0.0)
                {
                    return Err(ErrorKind::Num);
                }
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "a frequency divides twelve"
                )]
                let months = 12 / frequency as i64;
                let periods = quasi_periods(
                    issue,
                    first_coupon,
                    settlement,
                    months,
                    true,
                    basis,
                    frequency,
                )?;
                let regular = if first_coupon < maturity {
                    coupons(first_coupon, maturity, frequency)?.count
                } else {
                    0.0
                };
                let holding = periods
                    .iter()
                    .position(|period| period.remaining > 0.0)
                    .ok_or(ErrorKind::Num)?;
                let current = &periods[holding];
                #[expect(clippy::cast_precision_loss, reason = "a handful of periods")]
                let whole = (periods.len() - holding - 1) as f64;
                let odd = OddFirst {
                    rate,
                    redemption,
                    frequency,
                    whole,
                    fraction: current.remaining / current.length,
                    regular,
                    periods,
                };
                if name == "ODDFPRICE" {
                    return finite(odd.price(given));
                }
                finite(solve_decreasing(
                    given,
                    -frequency + 1e-6,
                    1.0,
                    |yield_rate| odd.price(yield_rate),
                )?)
            })(),
            "ODDLPRICE" | "ODDLYIELD" => (|| {
                let (settlement, maturity) = bond()?;
                let last_interest = date(2)?;
                let (rate, given, redemption) = (number(3)?, number(4)?, number(5)?);
                let frequency = frequency_at(6)?;
                let basis = basis_at(7)?;
                if last_interest >= settlement
                    || rate < 0.0
                    || given < 0.0
                    || redemption <= 0.0
                    || (name == "ODDLYIELD" && given == 0.0)
                {
                    return Err(ErrorKind::Num);
                }
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "a frequency divides twelve"
                )]
                let months = 12 / frequency as i64;
                let periods = quasi_periods(
                    last_interest,
                    maturity,
                    settlement,
                    months,
                    false,
                    basis,
                    frequency,
                )?;
                let sums = odd_last(&periods);
                let coupon = 100.0 * rate / frequency;
                let paid = redemption + sums.counted * coupon;
                if name == "ODDLPRICE" {
                    return finite(
                        paid / (1.0 + sums.remaining * given / frequency) - sums.accrued * coupon,
                    );
                }
                let invested = given + sums.accrued * coupon;
                finite((paid - invested) / invested * frequency / sums.remaining)
            })(),
            "AMORLINC" | "AMORDEGRC" => (|| {
                let cost = number(0)?;
                let (purchased, first_period) = (date(1)?, date(2)?);
                let (salvage, period, rate) = (number(3)?, number(4)?.trunc(), number(5)?);
                let basis = basis_at(6)?;
                if basis == 2
                    || cost < 0.0
                    || salvage < 0.0
                    || salvage > cost
                    || period < 0.0
                    || rate <= 0.0
                    || purchased > first_period
                {
                    return Err(ErrorKind::Num);
                }
                let first = year_fraction(purchased, first_period, basis)?;
                if name == "AMORLINC" {
                    let full = cost * rate;
                    let opening = first * rate * cost;
                    let full_periods = ((cost - salvage - opening) / full).trunc().max(0.0);
                    return finite(if period == 0.0 {
                        opening
                    } else if period <= full_periods {
                        full
                    } else if period == full_periods + 1.0 {
                        cost - salvage - full * full_periods - opening
                    } else {
                        0.0
                    });
                }
                let life = 1.0 / rate;
                let coefficient = if life < 3.0 {
                    1.0
                } else if life < 5.0 {
                    1.5
                } else if life <= 6.0 {
                    2.0
                } else {
                    2.5
                };
                let rate = rate * coefficient;
                let mut depreciation = round(first * rate * cost);
                let mut value = cost - depreciation;
                let mut rest = value - salvage;
                let mut n = 0.0;
                while n < period {
                    depreciation = round(rate * value);
                    rest -= depreciation;
                    if rest < 0.0 {
                        return finite(if period - n <= 1.0 {
                            round(value * 0.5)
                        } else {
                            0.0
                        });
                    }
                    value -= depreciation;
                    n += 1.0;
                }
                finite(depreciation)
            })(),
            "VDB" => (|| {
                let (cost, salvage, life) = (number(0)?, number(1)?, number(2)?);
                let (start, end) = (number(3)?, number(4)?);
                let factor = optional(5, 2.0)?;
                let no_switch = match argument(6) {
                    None => false,
                    Some(Expr::Text(text)) if text.is_empty() => false,
                    Some(expression) => self.scalar(sheet, expression).as_bool()?,
                };
                if start < 0.0
                    || end < start
                    || end > life
                    || cost < 0.0
                    || salvage > cost
                    || factor <= 0.0
                {
                    return Err(ErrorKind::Num);
                }
                let (whole_start, whole_end) = (start.floor(), end.ceil());
                if no_switch {
                    let mut total = 0.0;
                    let mut index = whole_start + 1.0;
                    while index <= whole_end {
                        let mut term = declining(cost, salvage, life, index, factor);
                        if index == whole_start + 1.0 {
                            term *= end.min(whole_start + 1.0) - start;
                        } else if index == whole_end {
                            term *= end + 1.0 - whole_end;
                        }
                        total += term;
                        index += 1.0;
                    }
                    return finite(total);
                }
                let mut part = 0.0;
                if start != whole_start {
                    let value =
                        cost - variable_declining(cost, salvage, life, life, whole_start, factor);
                    part += (start - whole_start)
                        * variable_declining(value, salvage, life, life - whole_start, 1.0, factor);
                }
                if end != whole_end {
                    let before = whole_end - 1.0;
                    let value =
                        cost - variable_declining(cost, salvage, life, life, before, factor);
                    part += (whole_end - end)
                        * variable_declining(value, salvage, life, life - before, 1.0, factor);
                }
                let value =
                    cost - variable_declining(cost, salvage, life, life, whole_start, factor);
                finite(
                    variable_declining(
                        value,
                        salvage,
                        life,
                        life - whole_start,
                        whole_end - whole_start,
                        factor,
                    ) - part,
                )
            })(),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests;
