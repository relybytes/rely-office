// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

//! Dates as spreadsheets count them: days since 30 December 1899, so 1 March 1900 is 61, as in
//! Excel's 1900 system after the day it wrongly counts as 29 February 1900.

/// The serial of a calendar date, or `None` before 1 March 1900 or past 9999.
#[must_use]
pub fn serial_from_date(year: i64, month: i64, day: i64) -> Option<f64> {
    // Months past 12 and days past the month roll over, as DATE does.
    let total_months = year * 12 + (month - 1);
    let year = total_months.div_euclid(12);
    let month = total_months.rem_euclid(12) + 1;
    let days = days_from_civil(year, month, 1) + (day - 1) - days_from_civil(1899, 12, 30);
    #[allow(clippy::cast_precision_loss)]
    let serial = days as f64;
    (61.0..=2_958_465.0).contains(&serial).then_some(serial)
}

/// The calendar date of a serial: year, month, day.
#[must_use]
pub fn date_from_serial(serial: f64) -> Option<(i64, i64, i64)> {
    if !(61.0..2_958_466.0).contains(&serial) {
        return None;
    }
    #[allow(clippy::cast_possible_truncation)]
    let days = serial.floor() as i64 + days_from_civil(1899, 12, 30);
    Some(civil_from_days(days))
}

/// Days since 1 January 1970 of a proleptic Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_days_as_excel_does() {
        assert_eq!(serial_from_date(1900, 3, 1), Some(61.0));
        assert_eq!(serial_from_date(2026, 10, 27), Some(46_322.0));
        assert_eq!(serial_from_date(2024, 13, 1), serial_from_date(2025, 1, 1));
        assert_eq!(serial_from_date(2024, 3, 0), serial_from_date(2024, 2, 29));
        assert_eq!(serial_from_date(1900, 1, 1), None);
        assert_eq!(serial_from_date(10_000, 1, 1), None);
        assert_eq!(date_from_serial(46_322.75), Some((2026, 10, 27)));
        assert_eq!(date_from_serial(61.0), Some((1900, 3, 1)));
        assert_eq!(date_from_serial(10.0), None);
    }
}
