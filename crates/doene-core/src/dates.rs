//! Date values: `YYYY-MM-DD`, computed in UTC (docs/plan.md §5 conventions).
use std::fmt;

/// A calendar date in the canonical `YYYY-MM-DD` form used across Döne.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: i32,
    /// 1..=12
    pub month: u8,
    /// 1..=days in the month
    pub day: u8,
}

/// Returned when a string is not a valid `YYYY-MM-DD` calendar date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidDate;

impl fmt::Display for InvalidDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not a valid YYYY-MM-DD calendar date")
    }
}

impl std::error::Error for InvalidDate {}

impl Date {
    /// Parse a `YYYY-MM-DD` string. Rejects malformed input and impossible
    /// calendar dates (e.g. Feb 30); leap years follow the Gregorian rule.
    pub fn parse(s: &str) -> Result<Self, InvalidDate> {
        let parts: Vec<&str> = s.split('-').collect();
        let [y, m, d] = parts.as_slice() else {
            return Err(InvalidDate);
        };
        if y.len() != 4 || m.len() != 2 || d.len() != 2 {
            return Err(InvalidDate);
        }
        let (year, month, day) = match (y.parse::<i32>(), m.parse::<u8>(), d.parse::<u8>()) {
            (Ok(year), Ok(month), Ok(day)) => (year, month, day),
            _ => return Err(InvalidDate),
        };
        if !(1..=12).contains(&month) || !((1..=Self::days_in_month(month, year)).contains(&day)) {
            return Err(InvalidDate);
        }
        Ok(Self { year, month, day })
    }

    /// Number of days in `month` (1..=12) of `year`, Gregorian.
    pub fn days_in_month(month: u8, year: i32) -> u8 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                    29
                } else {
                    28
                }
            }
            _ => 0,
        }
    }

    /// Days since 1970-01-01 in the proleptic Gregorian calendar (UTC civil);
    /// negative before the epoch.
    pub fn days_since_epoch(self) -> i64 {
        let y = if self.month <= 2 {
            self.year as i64 - 1
        } else {
            self.year as i64
        };
        let era = y.div_euclid(400);
        let yoe = y - era * 400; // 0..=399
        let mp = if self.month > 2 {
            self.month as i64 - 3
        } else {
            self.month as i64 + 9
        }; // 0..=11
        let doy = (153 * mp + 2) / 5 + self.day as i64 - 1;
        let doe = yoe * 365 + yoe.div_euclid(4) - yoe.div_euclid(100) + doy;
        era * 146097 + doe - 719468
    }

    /// Add days (negative to go back), crossing month/year boundaries.
    pub fn add_days(self, n: i64) -> Date {
        Self::from_days_since_epoch(self.days_since_epoch() + n)
    }

    /// Add months, clamping the day to the last day of the target month
    /// (Jan 31 + 1 month = Feb 28/29). The result's day never exceeds the
    /// original day.
    pub fn add_months(self, n: i64) -> Date {
        let total = self.year as i64 * 12 + self.month as i64 - 1 + n;
        let year = total.div_euclid(12);
        let month = (total.rem_euclid(12) + 1) as u8;
        let day = self.day.min(Self::days_in_month(month, year as i32));
        Date {
            year: year as i32,
            month,
            day,
        }
    }

    /// Day of week, Gör't convention: 0 = Sunday … 6 = Saturday.
    pub fn dow(self) -> u8 {
        // 1970-01-01 was a Thursday (dow 4).
        let z = self.days_since_epoch();
        ((((z % 7) + 7) % 7 + 4) % 7) as u8
    }

    fn from_days_since_epoch(z: i64) -> Date {
        let z = z + 719468;
        let era = z.div_euclid(146097);
        let doe = (z - era * 146097) as u64; // 0..=146096
        let yoe = ((doe - doe / 1460 + doe / 36524 - doe / 146096) as i64) / 365; // 0..=399
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100) as u64;
        let mp = (5 * doy + 2) / 153; // 0..=11
        let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        Date {
            // i64 → i32 is safe for any date within ±500 million years.
            year: (if month <= 2 { y + 1 } else { y }) as i32,
            month: month as u8,
            day,
        }
    }
}

/// The seven weekday numbers in week order starting at `start`
/// (0 = Sunday, 1 = Monday; Gör't's `weekDaysFrom`).
pub fn week_days_from(start: u8) -> [u8; 7] {
    let mut out = [0u8; 7];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = (start + i as u8) % 7;
    }
    out
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

#[cfg(test)]
mod tests {
    use super::{Date, week_days_from};

    #[test]
    fn parses_valid_date() {
        let d = Date::parse("2025-06-15").expect("valid date");
        assert_eq!((d.year, d.month, d.day), (2025, 6, 15));
    }

    #[test]
    fn rejects_day_beyond_month_end() {
        assert!(Date::parse("2025-02-30").is_err());
        assert!(Date::parse("2025-04-31").is_err());
    }

    #[test]
    fn accepts_leap_day_in_leap_year() {
        assert!(Date::parse("2024-02-29").is_ok());
    }

    #[test]
    fn rejects_leap_day_in_century_non_leap_year() {
        assert!(Date::parse("1900-02-29").is_err());
    }

    #[test]
    fn accepts_leap_day_in_400_year_leap_year() {
        assert!(Date::parse("2000-02-29").is_ok());
    }

    #[test]
    fn rejects_out_of_range_month_and_day() {
        assert!(Date::parse("2025-13-01").is_err());
        assert!(Date::parse("2025-00-10").is_err());
        assert!(Date::parse("2025-06-00").is_err());
    }

    #[test]
    fn rejects_malformed_input() {
        for bad in [
            "",
            "2025-6-15", // not zero-padded
            "20250615",  // no separators
            "2025-06-15T00:00:00Z",
            "abc",
            "-01-01",
        ] {
            assert!(Date::parse(bad).is_err(), "expected {bad:?} to be rejected");
        }
    }

    #[test]
    fn display_round_trips() {
        let d = Date::parse("2025-06-15").expect("valid date");
        assert_eq!(d.to_string(), "2025-06-15");
    }

    // --- Arithmetic (ticket #02) -----------------------------------------

    fn d(s: &str) -> Date {
        Date::parse(s).expect("valid fixture date")
    }

    #[test]
    fn add_days_crosses_month_and_year_boundaries() {
        assert_eq!(d("2025-03-31").add_days(1), d("2025-04-01"));
        assert_eq!(d("2025-12-31").add_days(1), d("2026-01-01"));
        assert_eq!(d("2025-03-12").add_days(7), d("2025-03-19")); // Gör't: nextDate weekly
        assert_eq!(d("2025-03-14").add_days(-2), d("2025-03-12"));
    }

    #[test]
    fn add_months_clamps_month_end() {
        // Ticket #02: Jan 31 + 1mo → Feb 28/29.
        assert_eq!(d("2025-01-31").add_months(1), d("2025-02-28"));
        assert_eq!(d("2024-01-31").add_months(1), d("2024-02-29")); // leap year
        assert_eq!(d("2025-03-31").add_months(1), d("2025-04-30"));
        assert_eq!(d("2025-01-31").add_months(12), d("2026-01-31")); // Gör't: yearly
    }

    #[test]
    fn add_months_negative_and_zero() {
        assert_eq!(d("2025-03-15").add_months(-1), d("2025-02-15"));
        assert_eq!(d("2025-03-31").add_months(-1), d("2025-02-28")); // clamps down too
        assert_eq!(d("2025-03-15").add_months(0), d("2025-03-15"));
    }

    #[test]
    fn dow_is_sunday_zero() {
        // Literals from Gör't fixtures: 2025-03-12 is a Wednesday, the week
        // runs Fri 03-14 / Sat 03-15 / Sun 03-16 / Mon 03-17.
        assert_eq!(d("2025-03-12").dow(), 3);
        assert_eq!(d("2025-03-14").dow(), 5);
        assert_eq!(d("2025-03-15").dow(), 6);
        assert_eq!(d("2025-03-16").dow(), 0);
        assert_eq!(d("2025-03-17").dow(), 1);
    }

    #[test]
    fn week_days_from_orders_seven_days() {
        assert_eq!(week_days_from(0), [0, 1, 2, 3, 4, 5, 6]); // Sunday start (Gör't default)
        assert_eq!(week_days_from(1), [1, 2, 3, 4, 5, 6, 0]); // Monday start
    }

    #[test]
    fn days_since_epoch_is_a_linear_day_count() {
        assert_eq!(d("1970-01-01").days_since_epoch(), 0);
        assert_eq!(
            d("2025-03-19").days_since_epoch() - d("2025-03-12").days_since_epoch(),
            7
        );
    }
}
