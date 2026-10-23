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
}
