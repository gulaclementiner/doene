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

    fn days_in_month(month: u8, year: i32) -> u8 {
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
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// Days since the civil epoch 1970-01-01 (Howard Hinnant's `days_from_civil`).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146097 + doe - 719468
}

/// The inverse of `days_from_civil`.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    ((if m <= 2 { y + 1 } else { y }), m, d)
}

impl Date {
    /// The date `n` days later (negative `n` goes back), across month and year
    /// boundaries — the arithmetic Gör't's `addDays` does in UTC.
    pub fn add_days(self, n: i32) -> Date {
        let z = days_from_civil(self.year as i64, self.month as i64, self.day as i64) + n as i64;
        let (y, m, d) = civil_from_days(z);
        Self {
            year: y as i32,
            month: m as u8,
            day: d as u8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Date;

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

    #[test]
    fn add_days_crosses_month_and_year_boundaries() {
        let d = |s: &str| Date::parse(s).expect("valid date");
        assert_eq!(d("2026-07-15").add_days(1), d("2026-07-16"));
        assert_eq!(d("2026-07-15").add_days(-1), d("2026-07-14"));
        assert_eq!(d("2026-07-31").add_days(1), d("2026-08-01"));
        assert_eq!(d("2026-08-01").add_days(-1), d("2026-07-31"));
        assert_eq!(d("2026-12-31").add_days(1), d("2027-01-01"));
        assert_eq!(d("2027-01-01").add_days(-1), d("2026-12-31"));
    }

    #[test]
    fn add_days_respects_leap_years() {
        let d = |s: &str| Date::parse(s).expect("valid date");
        assert_eq!(d("2024-02-28").add_days(1), d("2024-02-29")); // leap year
        assert_eq!(d("2025-02-28").add_days(1), d("2025-03-01")); // common year
        assert_eq!(d("2000-02-28").add_days(2), d("2000-03-01")); // 400-year leap
        assert_eq!(d("1900-02-28").add_days(1), d("1900-03-01")); // century non-leap
    }
}
