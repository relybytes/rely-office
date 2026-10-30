// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! The distributions beyond the normal one and the tests built on them: beta, binomial ranges,
//! chi-squared, F, gamma, hypergeometric, lognormal, negative binomial, Student's t and Weibull,
//! with the error function and the tests of samples. Each distribution is computed from the
//! regularized incomplete gamma and beta functions, and each inverse by bisection on its
//! cumulative function, which is monotonic.

#![allow(
    clippy::float_cmp,
    reason = "the flags and whole numbers compared here are read from cells, exact by construction"
)]

use super::Evaluator;
use super::more::{erf, gamma_ln, mean, normal_cdf, normal_inverse, variance};
use crate::formula::Expr;
use crate::value::{ErrorKind, Value};

type Answer = Option<Result<Value, ErrorKind>>;

const MAX_ITERATIONS: usize = 300;
const EPSILON: f64 = 1e-15;
/// The smallest positive double a continued fraction divides by instead of zero.
const TINY: f64 = 1e-300;

/// The regularized lower incomplete gamma function `P(a, x)`, by its series below `a + 1` and
/// its continued fraction above.
#[expect(
    clippy::many_single_char_names,
    reason = "the names of the published algorithm"
)]
pub(super) fn gamma_p(a: f64, x: f64) -> Result<f64, ErrorKind> {
    if x <= 0.0 {
        return Ok(0.0);
    }
    let log_gamma = gamma_ln(a)?;
    if x < a + 1.0 {
        let mut term = 1.0 / a;
        let mut sum = term;
        let mut denominator = a;
        for _ in 0..MAX_ITERATIONS {
            denominator += 1.0;
            term *= x / denominator;
            sum += term;
            if term.abs() < sum.abs() * EPSILON {
                break;
            }
        }
        Ok((sum * (-x + a * x.ln() - log_gamma).exp()).clamp(0.0, 1.0))
    } else {
        let mut b = x + 1.0 - a;
        let mut c = 1.0 / TINY;
        let mut d = 1.0 / b;
        let mut fraction = d;
        for index in 1..MAX_ITERATIONS {
            let i = f64::from(u32::try_from(index).unwrap_or(u32::MAX));
            let an = -i * (i - a);
            b += 2.0;
            d = an * d + b;
            if d.abs() < TINY {
                d = TINY;
            }
            c = b + an / c;
            if c.abs() < TINY {
                c = TINY;
            }
            d = 1.0 / d;
            let delta = d * c;
            fraction *= delta;
            if (delta - 1.0).abs() < EPSILON {
                break;
            }
        }
        Ok((1.0 - (-x + a * x.ln() - log_gamma).exp() * fraction).clamp(0.0, 1.0))
    }
}

/// The continued fraction of the incomplete beta function, by the modified Lentz method.
#[expect(
    clippy::many_single_char_names,
    reason = "the names of the published algorithm"
)]
fn beta_fraction(a: f64, b: f64, x: f64) -> f64 {
    let (qab, qap, qam) = (a + b, a + 1.0, a - 1.0);
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < TINY {
        d = TINY;
    }
    d = 1.0 / d;
    let mut fraction = d;
    for index in 1..MAX_ITERATIONS {
        let m = f64::from(u32::try_from(index).unwrap_or(u32::MAX));
        let m2 = 2.0 * m;
        let even = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + even * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + even / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        fraction *= d * c;
        let odd = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + odd * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + odd / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let delta = d * c;
        fraction *= delta;
        if (delta - 1.0).abs() < EPSILON {
            break;
        }
    }
    fraction
}

/// The regularized incomplete beta function, `I_x(a, b)`.
pub(super) fn beta_i(a: f64, b: f64, x: f64) -> Result<f64, ErrorKind> {
    if x <= 0.0 {
        return Ok(0.0);
    }
    if x >= 1.0 {
        return Ok(1.0);
    }
    let front =
        (gamma_ln(a + b)? - gamma_ln(a)? - gamma_ln(b)? + a * x.ln() + b * (1.0 - x).ln()).exp();
    let value = if x < (a + 1.0) / (a + b + 2.0) {
        front * beta_fraction(a, b, x) / a
    } else {
        1.0 - front * beta_fraction(b, a, 1.0 - x) / b
    };
    Ok(value.clamp(0.0, 1.0))
}

/// The value whose cumulative probability is `probability`, by bisection between `low` and a
/// `high` doubled until it is past it.
fn invert(
    probability: f64,
    mut low: f64,
    mut high: f64,
    cumulative: impl Fn(f64) -> Result<f64, ErrorKind>,
) -> Result<f64, ErrorKind> {
    let mut doublings = 0;
    while cumulative(high)? < probability {
        low = high;
        high *= 2.0;
        doublings += 1;
        if doublings > 1_100 {
            return Err(ErrorKind::Num);
        }
    }
    for _ in 0..MAX_ITERATIONS {
        let middle = f64::midpoint(low, high);
        if cumulative(middle)? < probability {
            low = middle;
        } else {
            high = middle;
        }
        if high - low <= EPSILON * high.abs().max(1e-300) {
            break;
        }
    }
    Ok(f64::midpoint(low, high))
}

fn chi_squared_cdf(x: f64, freedom: f64) -> Result<f64, ErrorKind> {
    gamma_p(freedom / 2.0, x / 2.0)
}

fn f_cdf(x: f64, first: f64, second: f64) -> Result<f64, ErrorKind> {
    if x <= 0.0 {
        return Ok(0.0);
    }
    beta_i(first / 2.0, second / 2.0, first * x / (first * x + second))
}

/// The cumulative probability of Student's t with `freedom` degrees.
pub(super) fn t_cdf(x: f64, freedom: f64) -> Result<f64, ErrorKind> {
    let tail = beta_i(freedom / 2.0, 0.5, freedom / (freedom + x * x))? / 2.0;
    Ok(if x > 0.0 { 1.0 - tail } else { tail })
}

/// The two-tailed probability of Student's t, for a non-negative `x`.
fn t_two_tailed(x: f64, freedom: f64) -> Result<f64, ErrorKind> {
    beta_i(freedom / 2.0, 0.5, freedom / (freedom + x * x))
}

/// The value of Student's t whose cumulative probability is `probability`.
pub(super) fn t_inverse(probability: f64, freedom: f64) -> Result<f64, ErrorKind> {
    if probability == 0.5 {
        return Ok(0.0);
    }
    let upper = probability > 0.5;
    let tail = if upper {
        1.0 - probability
    } else {
        probability
    };
    let magnitude = invert(1.0 - tail, 0.0, 1.0, |x| t_cdf(x, freedom))?;
    Ok(if upper { magnitude } else { -magnitude })
}

fn ln_combinations(n: f64, k: f64) -> Result<f64, ErrorKind> {
    Ok(gamma_ln(n + 1.0)? - gamma_ln(k + 1.0)? - gamma_ln(n - k + 1.0)?)
}

fn binomial_pmf(k: f64, trials: f64, probability: f64) -> Result<f64, ErrorKind> {
    if probability == 0.0 {
        return Ok(if k == 0.0 { 1.0 } else { 0.0 });
    }
    if probability == 1.0 {
        return Ok(if k == trials { 1.0 } else { 0.0 });
    }
    Ok((ln_combinations(trials, k)?
        + k * probability.ln()
        + (trials - k) * (1.0 - probability).ln())
    .exp())
}

fn hypergeometric_pmf(
    successes: f64,
    sample: f64,
    population_successes: f64,
    population: f64,
) -> Result<f64, ErrorKind> {
    Ok((ln_combinations(population_successes, successes)?
        + ln_combinations(population - population_successes, sample - successes)?
        - ln_combinations(population, sample)?)
    .exp())
}

fn finite(value: f64) -> Result<Value, ErrorKind> {
    if value.is_finite() {
        Ok(Value::Number(value))
    } else {
        Err(ErrorKind::Num)
    }
}

/// A probability of an inverse: strictly between 0 and 1, or up to 1 when `closed`.
fn probability_of(value: f64, closed: bool) -> Result<f64, ErrorKind> {
    if value <= 0.0 || value > 1.0 || (!closed && value == 1.0) {
        Err(ErrorKind::Num)
    } else {
        Ok(value)
    }
}

/// Degrees of freedom, truncated, at least 1 and at most 10^10.
fn freedom_of(value: f64) -> Result<f64, ErrorKind> {
    let freedom = value.trunc();
    if (1.0..=1e10).contains(&freedom) {
        Ok(freedom)
    } else {
        Err(ErrorKind::Num)
    }
}

impl Evaluator<'_> {
    /// A distribution or a test of this set, or `None` when the name is none of them.
    #[expect(clippy::too_many_lines, reason = "one arm per function, each short")]
    pub(super) fn probability(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        let argument = |index: usize| arguments.get(index);
        let number = |index: usize| self.number(sheet, argument(index));
        let optional = |index: usize, default: f64| match argument(index) {
            Some(Expr::Text(text)) if text.is_empty() => Ok(default),
            Some(expression) => self.scalar(sheet, expression).as_number(),
            None => Ok(default),
        };
        let flag = |index: usize| -> Result<bool, ErrorKind> {
            argument(index).map_or(Err(ErrorKind::Value), |expression| {
                self.scalar(sheet, expression).as_bool()
            })
        };
        Some(match name {
            "ERF" => (|| {
                let lower = number(0)?;
                match argument(1) {
                    Some(_) => finite(erf(number(1)?) - erf(lower)),
                    None => finite(erf(lower)),
                }
            })(),
            "ERF.PRECISE" => (|| finite(erf(number(0)?)))(),
            "ERFC" | "ERFC.PRECISE" => (|| finite(1.0 - erf(number(0)?)))(),
            "GAUSS" => (|| finite(normal_cdf(number(0)?) - 0.5))(),
            "BETA.DIST" | "BETADIST" => (|| {
                let (x, alpha, beta) = (number(0)?, number(1)?, number(2)?);
                let legacy = name == "BETADIST";
                let cumulative = if legacy { true } else { flag(3)? };
                let bounds = if legacy { 3 } else { 4 };
                let (low, high) = (optional(bounds, 0.0)?, optional(bounds + 1, 1.0)?);
                if alpha <= 0.0 || beta <= 0.0 || low >= high || x < low || x > high {
                    return Err(ErrorKind::Num);
                }
                let z = (x - low) / (high - low);
                if cumulative {
                    finite(beta_i(alpha, beta, z)?)
                } else {
                    let log_beta = gamma_ln(alpha)? + gamma_ln(beta)? - gamma_ln(alpha + beta)?;
                    finite(
                        ((alpha - 1.0) * z.ln() + (beta - 1.0) * (1.0 - z).ln() - log_beta).exp()
                            / (high - low),
                    )
                }
            })(),
            "BETA.INV" | "BETAINV" => (|| {
                let probability = probability_of(number(0)?, true)?;
                let (alpha, beta) = (number(1)?, number(2)?);
                let (low, high) = (optional(3, 0.0)?, optional(4, 1.0)?);
                if alpha <= 0.0 || beta <= 0.0 || low >= high {
                    return Err(ErrorKind::Num);
                }
                let z = invert(probability, 0.0, 1.0, |z| beta_i(alpha, beta, z))?;
                finite(low + z * (high - low))
            })(),
            "BINOM.DIST.RANGE" => (|| {
                let (trials, probability) = (number(0)?.trunc(), number(1)?);
                let first = number(2)?.trunc();
                let last = optional(3, first)?.trunc();
                if trials < 0.0
                    || !(0.0..=1.0).contains(&probability)
                    || first < 0.0
                    || first > trials
                    || last < first
                    || last > trials
                {
                    return Err(ErrorKind::Num);
                }
                let mut total = 0.0;
                let mut k = first;
                while k <= last {
                    total += binomial_pmf(k, trials, probability)?;
                    k += 1.0;
                }
                finite(total.min(1.0))
            })(),
            "BINOM.INV" | "CRITBINOM" => (|| {
                let (trials, probability, alpha) = (number(0)?.trunc(), number(1)?, number(2)?);
                if trials < 0.0
                    || !(0.0..=1.0).contains(&probability)
                    || !(0.0..=1.0).contains(&alpha)
                {
                    return Err(ErrorKind::Num);
                }
                let mut total = 0.0;
                let mut k = 0.0;
                while k < trials {
                    total += binomial_pmf(k, trials, probability)?;
                    if total >= alpha - 1e-12 {
                        break;
                    }
                    k += 1.0;
                }
                finite(k)
            })(),
            "CHISQ.DIST" => (|| {
                let (x, freedom) = (number(0)?, freedom_of(number(1)?)?);
                if x < 0.0 {
                    return Err(ErrorKind::Num);
                }
                if flag(2)? {
                    finite(chi_squared_cdf(x, freedom)?)
                } else if x == 0.0 {
                    finite(match freedom {
                        1.0 => f64::INFINITY,
                        2.0 => 0.5,
                        _ => 0.0,
                    })
                } else {
                    let half = freedom / 2.0;
                    finite(
                        ((half - 1.0) * x.ln()
                            - x / 2.0
                            - half * std::f64::consts::LN_2
                            - gamma_ln(half)?)
                        .exp(),
                    )
                }
            })(),
            "CHISQ.DIST.RT" | "CHIDIST" => (|| {
                let (x, freedom) = (number(0)?, freedom_of(number(1)?)?);
                if x < 0.0 {
                    return Err(ErrorKind::Num);
                }
                finite(1.0 - chi_squared_cdf(x, freedom)?)
            })(),
            "CHISQ.INV" => (|| {
                let probability = number(0)?;
                let freedom = freedom_of(number(1)?)?;
                if !(0.0..1.0).contains(&probability) {
                    return Err(ErrorKind::Num);
                }
                if probability == 0.0 {
                    return Ok(Value::Number(0.0));
                }
                finite(invert(probability, 0.0, freedom.max(1.0), |x| {
                    chi_squared_cdf(x, freedom)
                })?)
            })(),
            "CHISQ.INV.RT" | "CHIINV" => (|| {
                let probability = probability_of(number(0)?, true)?;
                let freedom = freedom_of(number(1)?)?;
                if probability == 1.0 {
                    return Ok(Value::Number(0.0));
                }
                finite(invert(1.0 - probability, 0.0, freedom.max(1.0), |x| {
                    chi_squared_cdf(x, freedom)
                })?)
            })(),
            "CHISQ.TEST" | "CHITEST" => (|| {
                let actual = self.grid(sheet, argument(0))?;
                let expected = self.grid(sheet, argument(1))?;
                let rows = actual.len();
                let columns = actual.first().map_or(0, Vec::len);
                if rows != expected.len() || columns != expected.first().map_or(0, Vec::len) {
                    return Err(ErrorKind::NotAvailable);
                }
                let mut statistic = 0.0;
                for (observed, wanted) in actual.iter().flatten().zip(expected.iter().flatten()) {
                    if let (Value::Number(observed), Value::Number(wanted)) = (observed, wanted) {
                        if *wanted == 0.0 {
                            return Err(ErrorKind::Div0);
                        }
                        statistic += (observed - wanted).powi(2) / wanted;
                    }
                }
                #[expect(clippy::cast_precision_loss, reason = "grid sizes are small")]
                let freedom = if rows > 1 && columns > 1 {
                    ((rows - 1) * (columns - 1)) as f64
                } else {
                    (rows * columns).saturating_sub(1) as f64
                };
                if freedom < 1.0 {
                    return Err(ErrorKind::NotAvailable);
                }
                finite(1.0 - chi_squared_cdf(statistic, freedom)?)
            })(),
            "F.DIST" => (|| {
                let x = number(0)?;
                let (first, second) = (freedom_of(number(1)?)?, freedom_of(number(2)?)?);
                if x < 0.0 {
                    return Err(ErrorKind::Num);
                }
                if flag(3)? {
                    finite(f_cdf(x, first, second)?)
                } else if x == 0.0 {
                    finite(if first < 2.0 {
                        f64::INFINITY
                    } else if first == 2.0 {
                        1.0
                    } else {
                        0.0
                    })
                } else {
                    let half_sum = f64::midpoint(first, second);
                    let log_beta =
                        gamma_ln(first / 2.0)? + gamma_ln(second / 2.0)? - gamma_ln(half_sum)?;
                    finite(
                        ((first / 2.0) * (first / second).ln() + (first / 2.0 - 1.0) * x.ln()
                            - half_sum * (1.0 + first * x / second).ln()
                            - log_beta)
                            .exp(),
                    )
                }
            })(),
            "F.DIST.RT" | "FDIST" => (|| {
                let x = number(0)?;
                let (first, second) = (freedom_of(number(1)?)?, freedom_of(number(2)?)?);
                if x < 0.0 {
                    return Err(ErrorKind::Num);
                }
                finite(1.0 - f_cdf(x, first, second)?)
            })(),
            "F.INV" | "F.INV.RT" | "FINV" => (|| {
                let probability = number(0)?;
                let (first, second) = (freedom_of(number(1)?)?, freedom_of(number(2)?)?);
                if !(0.0..=1.0).contains(&probability) {
                    return Err(ErrorKind::Num);
                }
                let left = if name == "F.INV" {
                    probability
                } else {
                    1.0 - probability
                };
                if left == 0.0 {
                    return Ok(Value::Number(0.0));
                }
                if left == 1.0 {
                    return Err(ErrorKind::Num);
                }
                finite(invert(left, 0.0, 1.0, |x| f_cdf(x, first, second))?)
            })(),
            "F.TEST" | "FTEST" => (|| {
                let first = self.numbers(sheet, &arguments[..1.min(arguments.len())])?;
                let second = self.numbers(sheet, arguments.get(1..2).unwrap_or(&[]))?;
                let (first_variance, second_variance) =
                    (variance(&first, true)?, variance(&second, true)?);
                if first_variance == 0.0 || second_variance == 0.0 {
                    return Err(ErrorKind::Div0);
                }
                #[expect(clippy::cast_precision_loss, reason = "sample sizes are small")]
                let (first_freedom, second_freedom) =
                    ((first.len() - 1) as f64, (second.len() - 1) as f64);
                let cumulative = f_cdf(
                    first_variance / second_variance,
                    first_freedom,
                    second_freedom,
                )?;
                finite((2.0 * cumulative.min(1.0 - cumulative)).min(1.0))
            })(),
            "GAMMA.DIST" | "GAMMADIST" => (|| {
                let (x, alpha, beta) = (number(0)?, number(1)?, number(2)?);
                if x < 0.0 || alpha <= 0.0 || beta <= 0.0 {
                    return Err(ErrorKind::Num);
                }
                if flag(3)? {
                    finite(gamma_p(alpha, x / beta)?)
                } else if x == 0.0 {
                    finite(if alpha < 1.0 {
                        f64::INFINITY
                    } else if alpha == 1.0 {
                        1.0 / beta
                    } else {
                        0.0
                    })
                } else {
                    finite(
                        ((alpha - 1.0) * x.ln() - x / beta - alpha * beta.ln() - gamma_ln(alpha)?)
                            .exp(),
                    )
                }
            })(),
            "GAMMA.INV" | "GAMMAINV" => (|| {
                let (probability, alpha, beta) = (number(0)?, number(1)?, number(2)?);
                if !(0.0..1.0).contains(&probability) || alpha <= 0.0 || beta <= 0.0 {
                    return Err(ErrorKind::Num);
                }
                if probability == 0.0 {
                    return Ok(Value::Number(0.0));
                }
                finite(invert(probability, 0.0, (alpha * beta).max(1.0), |x| {
                    gamma_p(alpha, x / beta)
                })?)
            })(),
            "HYPGEOM.DIST" | "HYPGEOMDIST" => (|| {
                let successes = number(0)?.trunc();
                let sample = number(1)?.trunc();
                let population_successes = number(2)?.trunc();
                let population = number(3)?.trunc();
                let cumulative = if name == "HYPGEOMDIST" {
                    false
                } else {
                    flag(4)?
                };
                if successes < 0.0
                    || successes > sample
                    || successes > population_successes
                    || successes < (sample - population + population_successes).max(0.0)
                    || sample <= 0.0
                    || sample > population
                    || population_successes <= 0.0
                    || population_successes > population
                    || population <= 0.0
                {
                    return Err(ErrorKind::Num);
                }
                if !cumulative {
                    return finite(hypergeometric_pmf(
                        successes,
                        sample,
                        population_successes,
                        population,
                    )?);
                }
                let mut total = 0.0;
                let mut k = (sample - population + population_successes).max(0.0);
                while k <= successes {
                    total += hypergeometric_pmf(k, sample, population_successes, population)?;
                    k += 1.0;
                }
                finite(total.min(1.0))
            })(),
            "LOGNORM.DIST" | "LOGNORMDIST" => (|| {
                let (x, average, deviation) = (number(0)?, number(1)?, number(2)?);
                if x <= 0.0 || deviation <= 0.0 {
                    return Err(ErrorKind::Num);
                }
                let z = (x.ln() - average) / deviation;
                let cumulative = if name == "LOGNORMDIST" {
                    true
                } else {
                    flag(3)?
                };
                if cumulative {
                    finite(normal_cdf(z))
                } else {
                    finite(
                        (-0.5 * z * z).exp()
                            / (x * deviation * (2.0 * std::f64::consts::PI).sqrt()),
                    )
                }
            })(),
            "LOGNORM.INV" | "LOGINV" => (|| {
                let (probability, average, deviation) = (number(0)?, number(1)?, number(2)?);
                if probability <= 0.0 || probability >= 1.0 || deviation <= 0.0 {
                    return Err(ErrorKind::Num);
                }
                finite((average + deviation * normal_inverse(probability)?).exp())
            })(),
            "NEGBINOM.DIST" | "NEGBINOMDIST" => (|| {
                let (failures, successes, probability) =
                    (number(0)?.trunc(), number(1)?.trunc(), number(2)?);
                let cumulative = if name == "NEGBINOMDIST" {
                    false
                } else {
                    flag(3)?
                };
                if !(0.0..=1.0).contains(&probability) || failures < 0.0 || successes < 1.0 {
                    return Err(ErrorKind::Num);
                }
                if cumulative {
                    finite(beta_i(successes, failures + 1.0, probability)?)
                } else {
                    finite(
                        (ln_combinations(failures + successes - 1.0, successes - 1.0)?
                            + successes * probability.ln()
                            + failures * (1.0 - probability).ln())
                        .exp(),
                    )
                }
            })(),
            "T.DIST" => (|| {
                let (x, freedom) = (number(0)?, number(1)?.trunc());
                if freedom < 1.0 {
                    return Err(ErrorKind::Num);
                }
                if flag(2)? {
                    finite(t_cdf(x, freedom)?)
                } else {
                    finite(
                        (gamma_ln(f64::midpoint(freedom, 1.0))?
                            - gamma_ln(freedom / 2.0)?
                            - f64::midpoint(freedom, 1.0) * (1.0 + x * x / freedom).ln())
                        .exp()
                            / (freedom * std::f64::consts::PI).sqrt(),
                    )
                }
            })(),
            "T.DIST.2T" | "T.DIST.RT" | "TDIST" => (|| {
                let (x, freedom) = (number(0)?, number(1)?.trunc());
                if freedom < 1.0 {
                    return Err(ErrorKind::Num);
                }
                let tails = match name {
                    "T.DIST.2T" => 2.0,
                    "T.DIST.RT" => 1.0,
                    _ => number(2)?.trunc(),
                };
                if tails != 1.0 && tails != 2.0 {
                    return Err(ErrorKind::Num);
                }
                if name != "T.DIST.RT" && x < 0.0 {
                    return Err(ErrorKind::Num);
                }
                if tails == 2.0 {
                    finite(t_two_tailed(x, freedom)?)
                } else {
                    finite(1.0 - t_cdf(x, freedom)?)
                }
            })(),
            "T.INV" => (|| {
                let probability = number(0)?;
                let freedom = number(1)?.trunc();
                if probability <= 0.0 || probability >= 1.0 || freedom < 1.0 {
                    return Err(ErrorKind::Num);
                }
                finite(t_inverse(probability, freedom)?)
            })(),
            "T.INV.2T" | "TINV" => (|| {
                let probability = probability_of(number(0)?, true)?;
                let freedom = number(1)?.trunc();
                if freedom < 1.0 {
                    return Err(ErrorKind::Num);
                }
                finite(t_inverse(1.0 - probability / 2.0, freedom)?.abs())
            })(),
            "T.TEST" | "TTEST" => self.t_test(sheet, arguments),
            "WEIBULL.DIST" | "WEIBULL" => (|| {
                let (x, alpha, beta) = (number(0)?, number(1)?, number(2)?);
                if x < 0.0 || alpha <= 0.0 || beta <= 0.0 {
                    return Err(ErrorKind::Num);
                }
                let power = (x / beta).powf(alpha);
                if flag(3)? {
                    finite(1.0 - (-power).exp())
                } else {
                    finite(alpha / beta.powf(alpha) * x.powf(alpha - 1.0) * (-power).exp())
                }
            })(),
            "Z.TEST" | "ZTEST" => (|| {
                let numbers = self.numbers(sheet, &arguments[..1.min(arguments.len())])?;
                if numbers.is_empty() {
                    return Err(ErrorKind::NotAvailable);
                }
                let x = number(1)?;
                let deviation = match argument(2) {
                    Some(_) => number(2)?,
                    None => variance(&numbers, true)?.sqrt(),
                };
                if deviation == 0.0 {
                    return Err(ErrorKind::Div0);
                }
                #[expect(clippy::cast_precision_loss, reason = "sample sizes are small")]
                let count = numbers.len() as f64;
                finite(1.0 - normal_cdf((mean(&numbers) - x) / (deviation / count.sqrt())))
            })(),
            "CONFIDENCE.T" => (|| {
                let (alpha, deviation, size) = (number(0)?, number(1)?, number(2)?.trunc());
                if alpha <= 0.0 || alpha >= 1.0 || deviation <= 0.0 || size < 1.0 {
                    return Err(ErrorKind::Num);
                }
                if size == 1.0 {
                    return Err(ErrorKind::Div0);
                }
                finite(t_inverse(1.0 - alpha / 2.0, size - 1.0)? * deviation / size.sqrt())
            })(),
            "PROB" => (|| {
                let flatten =
                    |grid: Vec<Vec<Value>>| grid.into_iter().flatten().collect::<Vec<_>>();
                let values = flatten(self.grid(sheet, argument(0))?);
                let probabilities = flatten(self.grid(sheet, argument(1))?);
                if values.len() != probabilities.len() {
                    return Err(ErrorKind::NotAvailable);
                }
                let lower = number(2)?;
                let upper = optional(3, lower)?;
                let mut total = 0.0;
                let mut inside = 0.0;
                for (value, probability) in values.iter().zip(&probabilities) {
                    let Value::Number(probability) = probability else {
                        continue;
                    };
                    if !(0.0..=1.0).contains(probability) {
                        return Err(ErrorKind::Num);
                    }
                    total += probability;
                    if let Value::Number(value) = value
                        && (lower..=upper).contains(value)
                    {
                        inside += probability;
                    }
                }
                if (total - 1.0).abs() > 1e-9 {
                    return Err(ErrorKind::Num);
                }
                finite(inside)
            })(),
            "SKEW.P" => (|| {
                let numbers = self.numbers(sheet, arguments)?;
                let deviation = variance(&numbers, false)?.sqrt();
                if deviation == 0.0 {
                    return Err(ErrorKind::Div0);
                }
                let average = mean(&numbers);
                let third: f64 = numbers
                    .iter()
                    .map(|value| ((value - average) / deviation).powi(3))
                    .sum();
                #[expect(clippy::cast_precision_loss, reason = "sample sizes are small")]
                let count = numbers.len() as f64;
                finite(third / count)
            })(),
            _ => return None,
        })
    }

    /// Student's t-test of two samples: `type` 1 paired, 2 of equal variance, 3 of unequal.
    fn t_test(&self, sheet: usize, arguments: &[Expr]) -> Result<Value, ErrorKind> {
        let tails = self.number(sheet, arguments.get(2))?.trunc();
        let kind = self.number(sheet, arguments.get(3))?.trunc();
        if tails != 1.0 && tails != 2.0 {
            return Err(ErrorKind::Num);
        }
        let (statistic, freedom) = if kind == 1.0 {
            let (first, second) = self.paired(sheet, arguments.first(), arguments.get(1))?;
            let differences: Vec<f64> = first.iter().zip(&second).map(|(a, b)| a - b).collect();
            if differences.len() < 2 {
                return Err(ErrorKind::Div0);
            }
            let deviation = variance(&differences, true)?.sqrt();
            if deviation == 0.0 {
                return Err(ErrorKind::Div0);
            }
            #[expect(clippy::cast_precision_loss, reason = "sample sizes are small")]
            let count = differences.len() as f64;
            (mean(&differences) / (deviation / count.sqrt()), count - 1.0)
        } else if kind == 2.0 || kind == 3.0 {
            let first = self.numbers(sheet, arguments.get(..1).unwrap_or(&[]))?;
            let second = self.numbers(sheet, arguments.get(1..2).unwrap_or(&[]))?;
            if first.len() < 2 || second.len() < 2 {
                return Err(ErrorKind::Div0);
            }
            let (first_variance, second_variance) =
                (variance(&first, true)?, variance(&second, true)?);
            #[expect(clippy::cast_precision_loss, reason = "sample sizes are small")]
            let (n1, n2) = (first.len() as f64, second.len() as f64);
            let difference = mean(&first) - mean(&second);
            if kind == 2.0 {
                let pooled =
                    ((n1 - 1.0) * first_variance + (n2 - 1.0) * second_variance) / (n1 + n2 - 2.0);
                let error = (pooled * (1.0 / n1 + 1.0 / n2)).sqrt();
                if error == 0.0 {
                    return Err(ErrorKind::Div0);
                }
                (difference / error, n1 + n2 - 2.0)
            } else {
                let (a, b) = (first_variance / n1, second_variance / n2);
                if a + b == 0.0 {
                    return Err(ErrorKind::Div0);
                }
                let freedom = (a + b).powi(2) / (a * a / (n1 - 1.0) + b * b / (n2 - 1.0));
                (difference / (a + b).sqrt(), freedom)
            }
        } else {
            return Err(ErrorKind::Num);
        };
        // A fractional degree of freedom, from Welch's test, is kept: Excel keeps it too.
        let two = t_two_tailed(statistic.abs(), freedom)?;
        finite(if tails == 1.0 { two / 2.0 } else { two })
    }

    /// The numbers of two same-sized ranges where both cells hold one.
    fn paired(
        &self,
        sheet: usize,
        first: Option<&Expr>,
        second: Option<&Expr>,
    ) -> Result<(Vec<f64>, Vec<f64>), ErrorKind> {
        let flatten = |grid: Vec<Vec<Value>>| grid.into_iter().flatten().collect::<Vec<_>>();
        let first = flatten(self.grid(sheet, first)?);
        let second = flatten(self.grid(sheet, second)?);
        if first.len() != second.len() {
            return Err(ErrorKind::NotAvailable);
        }
        Ok(first
            .iter()
            .zip(&second)
            .filter_map(|pair| match pair {
                (Value::Number(a), Value::Number(b)) => Some((*a, *b)),
                _ => None,
            })
            .unzip())
    }
}

#[cfg(test)]
mod tests;
