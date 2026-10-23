//! Recurrence rules and next-occurrence logic — a port of Gör't
//! `src/recurrence.js` (see docs/divergences.md for the one intentional
//! difference: no `error.recurrenceInvalid`, since `next_date` is total over
//! the closed `RecurrenceType`).

use crate::dates::Date;
use crate::entities::{RecurrenceRule, RecurrenceType, RecurrenceUnit, Weekday};
use crate::errors::CodedError;

/// The six recurrence kinds (Gör't's `TYPES`).
pub const TYPES: [&str; 6] = ["daily", "weekdays", "weekly", "monthly", "yearly", "custom"];

/// Raw (unvalidated) recurrence shape, as stored in JSON or produced by NLP.
/// (`PartialEq` only: the raw interval is an `f64`, which is not `Eq`.)
#[derive(Debug, Clone, PartialEq)]
pub struct RawRecurrence {
    /// Kind token (`"daily"` … `"custom"`).
    pub kind: Option<String>,
    /// Step size; defaults to 1 for custom rules.
    pub interval: Option<f64>,
    /// Custom-rule unit token (`"day" | "week" | "month" | "year"`).
    pub unit: Option<String>,
    /// Weekday tokens as dow strings (0 = Sunday … 6 = Saturday).
    pub days: Option<Vec<String>>,
}

impl From<RecurrenceRule> for RawRecurrence {
    fn from(r: RecurrenceRule) -> Self {
        Self {
            kind: Some(r.kind.as_token().to_owned()),
            interval: r.interval.map(f64::from),
            unit: r.unit.map(|u| u.as_token().to_owned()),
            days: r
                .days
                .map(|days| days.into_iter().map(|w| w.to_dow().to_string()).collect()),
        }
    }
}

/// Validate a raw recurrence into a typed rule. `None` → `Ok(None)` (Gör't's
/// `null`). Unknown kinds/units are rejected with coded errors; the interval
/// is floored to ≥ 1 and weekday sets are deduped, sorted and filtered to
/// 0..=6 (empty sets become `None`).
pub fn validate(raw: Option<&RawRecurrence>) -> Result<Option<RecurrenceRule>, CodedError> {
    let Some(raw) = raw else { return Ok(None) };

    let kind = RecurrenceType::from_token(raw.kind.as_deref().unwrap_or("")).ok_or_else(|| {
        let token = raw.kind.clone().unwrap_or_default();
        CodedError {
            code: "error.recurrenceTypeInvalid",
            message: format!("bad recurrence type: {token}"),
            params: vec![("type", token)],
        }
    })?;

    if kind != RecurrenceType::Custom {
        // Non-custom types carry no interval/unit/days (Gör't drops them).
        return Ok(Some(RecurrenceRule {
            kind,
            interval: None,
            unit: None,
            days: None,
        }));
    }

    let unit = RecurrenceUnit::from_token(raw.unit.as_deref().unwrap_or("")).ok_or_else(|| {
        CodedError::new(
            "error.recurrenceUnitInvalid",
            "custom recurrence needs unit",
        )
    })?;

    let interval = raw.interval.map(|i| i.floor()).unwrap_or(1.0).max(1.0) as u32;

    // Weekday sets only apply to weekly custom rules (Gör't drops them for
    // other units); empty/absent sets become None.
    let days = if unit == RecurrenceUnit::Week {
        raw.days
            .as_ref()
            .map(|days| {
                let set: std::collections::BTreeSet<u8> = days
                    .iter()
                    .filter_map(|t| t.trim().parse::<i64>().ok())
                    .filter(|d| (0..=6).contains(d))
                    .map(|d| d as u8)
                    .collect();
                let v: Vec<Weekday> = set.into_iter().flat_map(Weekday::from_dow).collect();
                if v.is_empty() { None } else { Some(v) }
            })
            .unwrap_or(None)
    } else {
        None
    };

    Ok(Some(RecurrenceRule {
        kind,
        interval: Some(interval),
        unit: Some(unit),
        days,
    }))
}

/// Next occurrence strictly after `from` (Gör't's `nextDate`). Total over the
/// closed `RecurrenceType`; a custom rule without a unit falls back to weekly.
pub fn next_date(from: Date, r: &RecurrenceRule) -> Date {
    match r.kind {
        RecurrenceType::Daily => from.add_days(1),
        RecurrenceType::Weekdays => {
            let mut d = from.add_days(1);
            while matches!(d.dow(), 0 | 6) {
                d = d.add_days(1);
            }
            d
        }
        RecurrenceType::Weekly => from.add_days(7),
        RecurrenceType::Monthly => from.add_months(1),
        RecurrenceType::Yearly => from.add_months(12),
        RecurrenceType::Custom => {
            let n = r.interval.unwrap_or(1) as i64;
            match r.unit {
                Some(RecurrenceUnit::Day) => from.add_days(n),
                Some(RecurrenceUnit::Month) => from.add_months(n),
                Some(RecurrenceUnit::Year) => from.add_months(12 * n),
                // Week (and the unit-less fallback): a weekday set picks a
                // later selected day in the same week, else the first
                // selected day n weeks out; without one, plain n-week step.
                _ => {
                    if let Some(days) = &r.days
                        && !days.is_empty()
                    {
                        let from_dow = from.dow();
                        if let Some(later) = days.iter().find(|w| w.to_dow() > from_dow) {
                            return from.add_days(later.to_dow() as i64 - from_dow as i64);
                        }
                        return from.add_days(n * 7 - from_dow as i64 + days[0].to_dow() as i64);
                    }
                    from.add_days(7 * n)
                }
            }
        }
    }
}

/// Next occurrence that is not before `floor` (Gör't's `nextOnOrAfter`): used
/// when completing an overdue repeating task. Bounded by a 10 000-step safety
/// valve, after which the last candidate is returned as-is.
pub fn next_on_or_after(from: Date, r: &RecurrenceRule, floor: Date) -> Date {
    let mut d = next_date(from, r);
    for _ in 0..10_000 {
        if d >= floor {
            break;
        }
        d = next_date(d, r);
    }
    d
}
