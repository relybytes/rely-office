// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! The engineering functions of complex numbers, written as Excel writes them, `3+4i` or `-2j`,
//! and the Bessel functions of the first and second kind and their modified forms.

#![allow(
    clippy::float_cmp,
    reason = "a zero part or a whole order is compared exactly, as Excel compares it"
)]
#![allow(
    clippy::unreadable_literal,
    reason = "the coefficients of the Bessel approximations are copied as published"
)]

use super::Evaluator;
use crate::formula::Expr;
use crate::value::{ErrorKind, Value};

type Answer = Option<Result<Value, ErrorKind>>;

/// A complex number and the letter it was written with, `i` or `j`; `None` for a real number,
/// which takes the letter of whatever it meets.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Complex {
    real: f64,
    imaginary: f64,
    suffix: Option<char>,
}

impl Complex {
    const fn new(real: f64, imaginary: f64, suffix: Option<char>) -> Self {
        Self {
            real,
            imaginary,
            suffix,
        }
    }

    fn modulus(self) -> f64 {
        self.real.hypot(self.imaginary)
    }

    fn argument(self) -> f64 {
        self.imaginary.atan2(self.real)
    }

    fn times(self, other: Self) -> Self {
        Self::new(
            self.real * other.real - self.imaginary * other.imaginary,
            self.real * other.imaginary + self.imaginary * other.real,
            self.suffix,
        )
    }

    fn divided(self, other: Self) -> Result<Self, ErrorKind> {
        let denominator = other.real * other.real + other.imaginary * other.imaginary;
        if denominator == 0.0 {
            return Err(ErrorKind::Num);
        }
        Ok(Self::new(
            (self.real * other.real + self.imaginary * other.imaginary) / denominator,
            (self.imaginary * other.real - self.real * other.imaginary) / denominator,
            self.suffix,
        ))
    }

    fn sin(self) -> Self {
        Self::new(
            self.real.sin() * self.imaginary.cosh(),
            self.real.cos() * self.imaginary.sinh(),
            self.suffix,
        )
    }

    fn cos(self) -> Self {
        Self::new(
            self.real.cos() * self.imaginary.cosh(),
            -self.real.sin() * self.imaginary.sinh(),
            self.suffix,
        )
    }

    fn sinh(self) -> Self {
        Self::new(
            self.real.sinh() * self.imaginary.cos(),
            self.real.cosh() * self.imaginary.sin(),
            self.suffix,
        )
    }

    fn cosh(self) -> Self {
        Self::new(
            self.real.cosh() * self.imaginary.cos(),
            self.real.sinh() * self.imaginary.sin(),
            self.suffix,
        )
    }

    fn ln(self) -> Result<Self, ErrorKind> {
        if self.real == 0.0 && self.imaginary == 0.0 {
            return Err(ErrorKind::Num);
        }
        Ok(Self::new(self.modulus().ln(), self.argument(), self.suffix))
    }

    fn scaled(self, factor: f64) -> Self {
        Self::new(self.real * factor, self.imaginary * factor, self.suffix)
    }

    fn reciprocal(self) -> Result<Self, ErrorKind> {
        Self::new(1.0, 0.0, self.suffix).divided(self)
    }

    /// The letter of two numbers met in one operation; two different letters are `#VALUE!`.
    fn suffix_with(self, other: Self) -> Result<Option<char>, ErrorKind> {
        match (self.suffix, other.suffix) {
            (Some(first), Some(second)) if first != second => Err(ErrorKind::Value),
            (Some(letter), _) | (None, Some(letter)) => Ok(Some(letter)),
            (None, None) => Ok(None),
        }
    }
}

/// A part of a complex number as Excel writes it: fifteen significant digits, scientific
/// notation below 1E-9 and from 1E15.
fn part_text(value: f64) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }
    let rounded: f64 = format!("{value:.14e}").parse().unwrap_or(value);
    let magnitude = rounded.abs();
    if !(1e-9..1e15).contains(&magnitude) {
        let text = format!("{rounded:.14e}");
        let (mantissa, exponent) = text.split_once('e').unwrap_or((&text, "0"));
        let mantissa = mantissa.trim_end_matches('0').trim_end_matches('.');
        let exponent: i32 = exponent.parse().unwrap_or(0);
        let sign = if exponent < 0 { '-' } else { '+' };
        return format!("{mantissa}E{sign}{:02}", exponent.abs());
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a decimal count between 0 and 24"
    )]
    let decimals = (14.0 - magnitude.log10().floor()).clamp(0.0, 24.0) as usize;
    let text = format!("{rounded:.decimals$}");
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text
    }
}

fn complex_text(number: Complex) -> Result<Value, ErrorKind> {
    if !number.real.is_finite() || !number.imaginary.is_finite() {
        return Err(ErrorKind::Num);
    }
    let suffix = number.suffix.unwrap_or('i');
    let real = if number.real == 0.0 {
        None
    } else {
        Some(part_text(number.real))
    };
    let imaginary = match number.imaginary {
        0.0 => None,
        1.0 => Some(String::new()),
        -1.0 => Some("-".to_owned()),
        value => Some(part_text(value)),
    };
    Ok(Value::Text(match (real, imaginary) {
        (None, None) => "0".to_owned(),
        (Some(real), None) => real,
        (None, Some(imaginary)) => format!("{imaginary}{suffix}"),
        (Some(real), Some(imaginary)) => {
            let sign = if imaginary.starts_with('-') { "" } else { "+" };
            format!("{real}{sign}{imaginary}{suffix}")
        }
    }))
}

/// A real number written as text, with only the characters a number holds.
fn real_text(text: &str) -> Result<f64, ErrorKind> {
    if text.is_empty()
        || !text.chars().all(|character| {
            character.is_ascii_digit() || matches!(character, '.' | 'e' | 'E' | '+' | '-')
        })
    {
        return Err(ErrorKind::Num);
    }
    text.parse().map_err(|_| ErrorKind::Num)
}

/// A complex number as Excel reads one: `a`, `bi`, `a+bi`, `a-bj`, `i`, `-i`.
fn parse_complex(text: &str) -> Result<Complex, ErrorKind> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(Complex::new(0.0, 0.0, None));
    }
    let Some(suffix) = text.chars().last().filter(|last| matches!(last, 'i' | 'j')) else {
        return Ok(Complex::new(real_text(text)?, 0.0, None));
    };
    let body = &text[..text.len() - 1];
    let bytes = body.as_bytes();
    let split = (1..bytes.len()).rev().find(|&index| {
        matches!(bytes[index], b'+' | b'-') && !matches!(bytes[index - 1], b'e' | b'E')
    });
    let (real, imaginary) = match split {
        Some(index) => (real_text(&body[..index])?, &body[index..]),
        None => (0.0, body),
    };
    let imaginary = match imaginary {
        "" | "+" => 1.0,
        "-" => -1.0,
        coefficient => real_text(coefficient)?,
    };
    Ok(Complex::new(real, imaginary, Some(suffix)))
}

fn bessel_j0(x: f64) -> f64 {
    let ax = x.abs();
    if ax < 8.0 {
        let y = x * x;
        let numerator = 57568490574.0
            + y * (-13362590354.0
                + y * (651619640.7 + y * (-11214424.18 + y * (77392.33017 + y * (-184.9052456)))));
        let denominator = 57568490411.0
            + y * (1029532985.0 + y * (9494680.718 + y * (59272.64853 + y * (267.8532712 + y))));
        numerator / denominator
    } else {
        let z = 8.0 / ax;
        let y = z * z;
        let shifted = ax - 0.785398164;
        let first = 1.0
            + y * (-0.1098628627e-2
                + y * (0.2734510407e-4 + y * (-0.2073370639e-5 + y * 0.2093887211e-6)));
        let second = -0.1562499995e-1
            + y * (0.1430488765e-3
                + y * (-0.6911147651e-5 + y * (0.7621095161e-6 - y * 0.934935152e-7)));
        (std::f64::consts::FRAC_2_PI / ax).sqrt()
            * (shifted.cos() * first - z * shifted.sin() * second)
    }
}

fn bessel_j1(x: f64) -> f64 {
    let ax = x.abs();
    if ax < 8.0 {
        let y = x * x;
        let numerator = x
            * (72362614232.0
                + y * (-7895059235.0
                    + y * (242396853.1
                        + y * (-2972611.439 + y * (15704.48260 + y * (-30.16036606))))));
        let denominator = 144725228442.0
            + y * (2300535178.0 + y * (18583304.74 + y * (99447.43394 + y * (376.9991397 + y))));
        numerator / denominator
    } else {
        let z = 8.0 / ax;
        let y = z * z;
        let shifted = ax - 2.356194491;
        let first = 1.0
            + y * (0.183105e-2
                + y * (-0.3516396496e-4 + y * (0.2457520174e-5 + y * (-0.240337019e-6))));
        let second = 0.04687499995
            + y * (-0.2002690873e-3
                + y * (0.8449199096e-5 + y * (-0.88228987e-6 + y * 0.105787412e-6)));
        let value = (std::f64::consts::FRAC_2_PI / ax).sqrt()
            * (shifted.cos() * first - z * shifted.sin() * second);
        if x < 0.0 { -value } else { value }
    }
}

/// The power series of `J_n` or, with `modified`, `I_n`, accurate for a moderate `x`.
fn bessel_series(order: u32, x: f64, modified: bool) -> f64 {
    let half = x / 2.0;
    let mut term = half.powi(i32::try_from(order).unwrap_or(i32::MAX));
    for k in 1..=order {
        term /= f64::from(k);
    }
    let mut sum = term;
    let square = half * half;
    for k in 1..500_u32 {
        term *= square / (f64::from(k) * f64::from(k + order));
        if !modified {
            term = -term;
        }
        sum += term;
        if term.abs() <= sum.abs() * 1e-17 {
            break;
        }
    }
    sum
}

pub(super) fn bessel_j(order: u32, x: f64) -> f64 {
    if x.abs() <= 12.0 {
        return bessel_series(order, x, false);
    }
    let ax = x.abs();
    let value = if order == 0 {
        bessel_j0(ax)
    } else if ax > f64::from(order) {
        let (mut before, mut current) = (bessel_j0(ax), bessel_j1(ax));
        for k in 1..order {
            let next = f64::from(k) * 2.0 / ax * current - before;
            before = current;
            current = next;
        }
        current
    } else {
        // Miller's downward recurrence, normalized by the sum of the even orders.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the square root of a small order"
        )]
        let extra = (40.0 * f64::from(order)).sqrt() as u32;
        let start = 2 * order.midpoint(extra);
        let (mut after, mut current, mut answer, mut sum) = (0.0, 1.0, 0.0, 0.0);
        let mut even = false;
        for k in (1..=start).rev() {
            let before = f64::from(k) * 2.0 / ax * current - after;
            after = current;
            current = before;
            if current.abs() > 1e10 {
                current *= 1e-10;
                after *= 1e-10;
                answer *= 1e-10;
                sum *= 1e-10;
            }
            if even {
                sum += current;
            }
            even = !even;
            if k == order {
                answer = after;
            }
        }
        answer / (2.0 * sum - current)
    };
    if x < 0.0 && order % 2 == 1 {
        -value
    } else {
        value
    }
}

fn bessel_y0(x: f64) -> f64 {
    if x < 8.0 {
        let y = x * x;
        let numerator = -2957821389.0
            + y * (7062834065.0
                + y * (-512359803.6 + y * (10879881.29 + y * (-86327.92757 + y * 228.4622733))));
        let denominator = 40076544269.0
            + y * (745249964.8 + y * (7189466.438 + y * (47447.26470 + y * (226.1030244 + y))));
        numerator / denominator + std::f64::consts::FRAC_2_PI * bessel_j0(x) * x.ln()
    } else {
        let z = 8.0 / x;
        let y = z * z;
        let shifted = x - 0.785398164;
        let first = 1.0
            + y * (-0.1098628627e-2
                + y * (0.2734510407e-4 + y * (-0.2073370639e-5 + y * 0.2093887211e-6)));
        let second = -0.1562499995e-1
            + y * (0.1430488765e-3
                + y * (-0.6911147651e-5 + y * (0.7621095161e-6 + y * (-0.934945152e-7))));
        (std::f64::consts::FRAC_2_PI / x).sqrt()
            * (shifted.sin() * first + z * shifted.cos() * second)
    }
}

fn bessel_y1(x: f64) -> f64 {
    if x < 8.0 {
        let y = x * x;
        let numerator = x
            * (-0.4900604943e13
                + y * (0.1275274390e13
                    + y * (-0.5153438139e11
                        + y * (0.7349264551e9 + y * (-0.4237922726e7 + y * 0.8511937935e4)))));
        let denominator = 0.2499580570e14
            + y * (0.4244419664e12
                + y * (0.3733650367e10
                    + y * (0.2245904002e8 + y * (0.1020426050e6 + y * (0.3549632885e3 + y)))));
        numerator / denominator + std::f64::consts::FRAC_2_PI * (bessel_j1(x) * x.ln() - 1.0 / x)
    } else {
        let z = 8.0 / x;
        let y = z * z;
        let shifted = x - 2.356194491;
        let first = 1.0
            + y * (0.183105e-2
                + y * (-0.3516396496e-4 + y * (0.2457520174e-5 + y * (-0.240337019e-6))));
        let second = 0.04687499995
            + y * (-0.2002690873e-3
                + y * (0.8449199096e-5 + y * (-0.88228987e-6 + y * 0.105787412e-6)));
        (std::f64::consts::FRAC_2_PI / x).sqrt()
            * (shifted.sin() * first + z * shifted.cos() * second)
    }
}

pub(super) fn bessel_y(order: u32, x: f64) -> f64 {
    if order == 0 {
        return bessel_y0(x);
    }
    let (mut before, mut current) = (bessel_y0(x), bessel_y1(x));
    for k in 1..order {
        let next = f64::from(k) * 2.0 / x * current - before;
        before = current;
        current = next;
    }
    current
}

pub(super) fn bessel_i(order: u32, x: f64) -> f64 {
    bessel_series(order, x, true)
}

fn bessel_i0(x: f64) -> f64 {
    bessel_series(0, x, true)
}

fn bessel_i1(x: f64) -> f64 {
    bessel_series(1, x, true)
}

fn bessel_k0(x: f64) -> f64 {
    if x <= 2.0 {
        let y = x * x / 4.0;
        -(x / 2.0).ln() * bessel_i0(x)
            + (-0.57721566
                + y * (0.42278420
                    + y * (0.23069756
                        + y * (0.3488590e-1 + y * (0.262698e-2 + y * (0.10750e-3 + y * 0.74e-5))))))
    } else {
        let y = 2.0 / x;
        (-x).exp() / x.sqrt()
            * (1.25331414
                + y * (-0.7832358e-1
                    + y * (0.2189568e-1
                        + y * (-0.1062446e-1
                            + y * (0.587872e-2 + y * (-0.251540e-2 + y * 0.53208e-3))))))
    }
}

fn bessel_k1(x: f64) -> f64 {
    if x <= 2.0 {
        let y = x * x / 4.0;
        (x / 2.0).ln() * bessel_i1(x)
            + (1.0 / x)
                * (1.0
                    + y * (0.15443144
                        + y * (-0.67278579
                            + y * (-0.18156897
                                + y * (-0.1919402e-1 + y * (-0.110404e-2 + y * (-0.4686e-4)))))))
    } else {
        let y = 2.0 / x;
        (-x).exp() / x.sqrt()
            * (1.25331414
                + y * (0.23498619
                    + y * (-0.3655620e-1
                        + y * (0.1504268e-1
                            + y * (-0.780353e-2 + y * (0.325614e-2 + y * (-0.68245e-3)))))))
    }
}

pub(super) fn bessel_k(order: u32, x: f64) -> f64 {
    if order == 0 {
        return bessel_k0(x);
    }
    let (mut before, mut current) = (bessel_k0(x), bessel_k1(x));
    for k in 1..order {
        let next = before + f64::from(k) * 2.0 / x * current;
        before = current;
        current = next;
    }
    current
}

impl Evaluator<'_> {
    fn complex_of(&self, sheet: usize, argument: Option<&Expr>) -> Result<Complex, ErrorKind> {
        let Some(argument) = argument else {
            return Err(ErrorKind::Value);
        };
        match self.scalar(sheet, argument) {
            Value::Number(number) => Ok(Complex::new(number, 0.0, None)),
            Value::Empty => Ok(Complex::new(0.0, 0.0, None)),
            Value::Text(text) => parse_complex(&text),
            Value::Bool(_) => Err(ErrorKind::Value),
            Value::Error(kind) => Err(kind),
        }
    }

    /// Every complex number of the arguments, ranges cell by cell, empty cells left out.
    fn complex_values(&self, sheet: usize, arguments: &[Expr]) -> Result<Vec<Complex>, ErrorKind> {
        let mut found = Vec::new();
        let mut failure = None;
        for argument in arguments {
            self.values_of(sheet, argument, |value, _| {
                if failure.is_some() {
                    return;
                }
                let read = match value {
                    Value::Empty => return,
                    Value::Number(number) => Ok(Complex::new(number, 0.0, None)),
                    Value::Text(text) => parse_complex(&text),
                    Value::Bool(_) => Err(ErrorKind::Value),
                    Value::Error(kind) => Err(kind),
                };
                match read {
                    Ok(number) => found.push(number),
                    Err(kind) => failure = Some(kind),
                }
            });
        }
        failure.map_or(Ok(found), Err)
    }

    /// A complex or Bessel function, or `None` when the name is none of them.
    #[expect(clippy::too_many_lines, reason = "one arm per function, each short")]
    pub(super) fn complex(&self, sheet: usize, name: &str, arguments: &[Expr]) -> Answer {
        let argument = |index: usize| arguments.get(index);
        let first = || self.complex_of(sheet, argument(0));
        let unary = |apply: fn(Complex) -> Result<Complex, ErrorKind>| -> Result<Value, ErrorKind> {
            complex_text(apply(first()?)?)
        };
        let order = |index: usize| -> Result<u32, ErrorKind> {
            let order = self.number(sheet, argument(index))?.trunc();
            if (0.0..=1_000.0).contains(&order) {
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "checked to be a small whole number"
                )]
                let order = order as u32;
                Ok(order)
            } else {
                Err(ErrorKind::Num)
            }
        };
        let finite = |value: f64| {
            if value.is_finite() {
                Ok(Value::Number(value))
            } else {
                Err(ErrorKind::Num)
            }
        };
        Some(match name {
            "COMPLEX" => (|| {
                let real = self.number(sheet, argument(0))?;
                let imaginary = self.number(sheet, argument(1))?;
                let suffix = match argument(2) {
                    None => 'i',
                    Some(Expr::Text(text)) if text.is_empty() => 'i',
                    Some(expression) => match self.scalar(sheet, expression).as_text()?.as_str() {
                        "i" | "" => 'i',
                        "j" => 'j',
                        _ => return Err(ErrorKind::Value),
                    },
                };
                complex_text(Complex::new(real, imaginary, Some(suffix)))
            })(),
            "IMABS" => (|| finite(first()?.modulus()))(),
            "IMAGINARY" => (|| finite(first()?.imaginary))(),
            "IMREAL" => (|| finite(first()?.real))(),
            "IMARGUMENT" => (|| {
                let number = first()?;
                if number.real == 0.0 && number.imaginary == 0.0 {
                    return Err(ErrorKind::Div0);
                }
                finite(number.argument())
            })(),
            "IMCONJUGATE" => {
                unary(|number| Ok(Complex::new(number.real, -number.imaginary, number.suffix)))
            }
            "IMSIN" => unary(|number| Ok(number.sin())),
            "IMCOS" => unary(|number| Ok(number.cos())),
            // tan and cot from the doubled angle, which keeps the digits a quotient of sin and
            // cos loses.
            "IMTAN" => unary(|number| {
                let (twice_real, twice_imaginary) = (2.0 * number.real, 2.0 * number.imaginary);
                let denominator = twice_real.cos() + twice_imaginary.cosh();
                if denominator == 0.0 {
                    return Err(ErrorKind::Num);
                }
                Ok(Complex::new(
                    twice_real.sin() / denominator,
                    twice_imaginary.sinh() / denominator,
                    number.suffix,
                ))
            }),
            "IMCOT" => unary(|number| {
                let (twice_real, twice_imaginary) = (2.0 * number.real, 2.0 * number.imaginary);
                let denominator = twice_imaginary.cosh() - twice_real.cos();
                if denominator == 0.0 {
                    return Err(ErrorKind::Num);
                }
                Ok(Complex::new(
                    twice_real.sin() / denominator,
                    -twice_imaginary.sinh() / denominator,
                    number.suffix,
                ))
            }),
            "IMSEC" => unary(|number| number.cos().reciprocal()),
            "IMCSC" => unary(|number| number.sin().reciprocal()),
            "IMSINH" => unary(|number| Ok(number.sinh())),
            "IMCOSH" => unary(|number| Ok(number.cosh())),
            "IMSECH" => unary(|number| number.cosh().reciprocal()),
            "IMCSCH" => unary(|number| number.sinh().reciprocal()),
            "IMEXP" => unary(|number| {
                let length = number.real.exp();
                Ok(Complex::new(
                    length * number.imaginary.cos(),
                    length * number.imaginary.sin(),
                    number.suffix,
                ))
            }),
            "IMLN" => unary(Complex::ln),
            "IMLOG10" => unary(|number| Ok(number.ln()?.scaled(1.0 / std::f64::consts::LN_10))),
            "IMLOG2" => unary(|number| Ok(number.ln()?.scaled(1.0 / std::f64::consts::LN_2))),
            "IMSQRT" => unary(|number| {
                let length = number.modulus().sqrt();
                let angle = number.argument() / 2.0;
                Ok(Complex::new(
                    length * angle.cos(),
                    length * angle.sin(),
                    number.suffix,
                ))
            }),
            "IMPOWER" => (|| {
                let number = first()?;
                let power = self.number(sheet, argument(1))?;
                let length = number.modulus();
                if length == 0.0 {
                    if power <= 0.0 {
                        return Err(ErrorKind::Num);
                    }
                    return complex_text(Complex::new(0.0, 0.0, number.suffix));
                }
                let angle = number.argument() * power;
                let length = length.powf(power);
                complex_text(Complex::new(
                    length * angle.cos(),
                    length * angle.sin(),
                    number.suffix,
                ))
            })(),
            "IMDIV" => (|| {
                let (numerator, denominator) = (first()?, self.complex_of(sheet, argument(1))?);
                let suffix = numerator.suffix_with(denominator)?;
                let mut quotient = numerator.divided(denominator)?;
                quotient.suffix = suffix;
                complex_text(quotient)
            })(),
            "IMSUB" => (|| {
                let (minuend, subtrahend) = (first()?, self.complex_of(sheet, argument(1))?);
                complex_text(Complex::new(
                    minuend.real - subtrahend.real,
                    minuend.imaginary - subtrahend.imaginary,
                    minuend.suffix_with(subtrahend)?,
                ))
            })(),
            "IMSUM" | "IMPRODUCT" => (|| {
                let numbers = self.complex_values(sheet, arguments)?;
                let mut numbers = numbers.into_iter();
                let Some(mut total) = numbers.next() else {
                    return Err(ErrorKind::Value);
                };
                for number in numbers {
                    let suffix = total.suffix_with(number)?;
                    total = if name == "IMSUM" {
                        Complex::new(
                            total.real + number.real,
                            total.imaginary + number.imaginary,
                            suffix,
                        )
                    } else {
                        let mut product = total.times(number);
                        product.suffix = suffix;
                        product
                    };
                }
                complex_text(total)
            })(),
            "BESSELJ" => (|| finite(bessel_j(order(1)?, self.number(sheet, argument(0))?)))(),
            "BESSELI" => (|| finite(bessel_i(order(1)?, self.number(sheet, argument(0))?)))(),
            "BESSELY" | "BESSELK" => (|| {
                let x = self.number(sheet, argument(0))?;
                let order = order(1)?;
                if x <= 0.0 {
                    return Err(ErrorKind::Num);
                }
                finite(if name == "BESSELY" {
                    bessel_y(order, x)
                } else {
                    bessel_k(order, x)
                })
            })(),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests;
