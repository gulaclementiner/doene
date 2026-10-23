//! Property tests ported from Gör't `test/properties.test.js` (the date and
//! recurrence properties; the search/NLP/store properties land with their
//! modules). Gör't uses fast-check; Döne uses proptest with the same value
//! ranges.

use doene_core::dates::Date;
use doene_core::entities::{RecurrenceType, RecurrenceUnit};
use doene_core::recurrence::{RawRecurrence, next_date, next_on_or_after, validate};
use proptest::prelude::*;

/// Gör't's `day` arb: uniform valid calendar days in [1990-01-01, 2060-12-31].
fn day() -> impl Strategy<Value = Date> {
    (1990i32..=2060, 1u8..=12).prop_flat_map(|(year, month)| {
        let last = Date::days_in_month(month, year);
        (1u8..=last).prop_map(move |day| Date { year, month, day })
    })
}

/// Gör't's `rec` arb: one of the five simple types, or a custom rule with a
/// unit, interval 1..=30 and an optional sorted unique weekday set.
fn raw_rec() -> impl Strategy<Value = RawRecurrence> {
    prop_oneof![
        // Simple types (no interval/unit/days in the raw shape).
        simple_kind().prop_map(|kind| RawRecurrence {
            kind: Some(kind.to_owned()),
            interval: None,
            unit: None,
            days: None,
        }),
        // Custom rules.
        (
            unit_token(),
            1u32..=30,
            prop_oneof![
                Just(None::<Vec<String>>),
                weekday_set().prop_map(Option::Some),
            ],
        )
            .prop_map(|(unit, interval, days)| RawRecurrence {
                kind: Some("custom".to_owned()),
                interval: Some(f64::from(interval)),
                unit: Some(unit.to_owned()),
                days,
            },),
    ]
}

fn simple_kind() -> impl Strategy<Value = &'static str> {
    (0usize..5).prop_map(|i| match i {
        0 => "daily",
        1 => "weekdays",
        2 => "weekly",
        3 => "monthly",
        _ => "yearly",
    })
}

fn unit_token() -> impl Strategy<Value = &'static str> {
    (0usize..4).prop_map(|i| match i {
        0 => "day",
        1 => "week",
        2 => "month",
        _ => "year",
    })
}

/// Sorted unique weekday set as raw dow tokens (Gör't: `uniqueArray(0..=6).sort()`).
fn weekday_set() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(0u8..=7, 0..7)
        .prop_map(|v| {
            v.into_iter()
                .filter(|d| *d <= 6)
                .collect::<std::collections::BTreeSet<u8>>()
        })
        .prop_map(|set| set.into_iter().map(|d| d.to_string()).collect())
}

proptest! {
    /// Ported from "dates: parse/fmt round-trip; addDays inverse and shifts by n".
    #[test]
    fn dates_parse_fmt_round_trip_and_add_days_inverse(d in day(), n in -4000i64..=4000) {
        // fmt(parse(s)) === s
        assert_eq!(Date::parse(&d.to_string()), Ok(d));
        // addDays is its own inverse
        assert_eq!(d.add_days(n).add_days(-n), d);
        // exact shift: the day-count difference is exactly n
        assert_eq!(d.add_days(n).days_since_epoch() - d.days_since_epoch(), n);
    }

    /// Ported from "dates: addMonths lands in the right month and never overflows".
    #[test]
    fn dates_add_months_lands_in_right_month_and_never_overflows(d in day(), n in -60i64..=60) {
        let r = d.add_months(n);
        // b.year*12 + b.month === a.year*12 + a.month + n
        assert_eq!(
            r.year as i64 * 12 + r.month as i64,
            d.year as i64 * 12 + d.month as i64 + n
        );
        // the result day never exceeds the original day (month-end clamping)
        assert!(r.day <= d.day);
    }

    /// Ported from "recurrence: nextDate is strictly later; nextOnOrAfter honours the floor".
    #[test]
    fn recurrence_next_date_strictly_later_and_floor_honoured(d in day(), r in raw_rec(), k in 0i64..=400) {
        let rule = validate(Some(&r)).unwrap().expect("generator produces valid rules");
        // nextDate is strictly after the input date
        let n = next_date(d, &rule);
        prop_assert!(n > d);

        // floor within 400 days: the result is on/after the floor and still after d
        let floor = d.add_days(k);
        let m = next_on_or_after(d, &rule, floor);
        prop_assert!(m >= floor && m > d);
    }

    /// Ported from "recurrence: weekdays never lands on a weekend; weekday sets are respected".
    #[test]
    fn recurrence_weekdays_never_on_weekend_and_sets_respected(d in day(), r in raw_rec()) {
        let rule = validate(Some(&r)).unwrap().expect("generator produces valid rules");

        if rule.kind == RecurrenceType::Weekdays {
            let n = next_date(d, &rule);
            prop_assert!(!matches!(n.dow(), 0 | 6));
        }

        // custom weekly rules with a weekday set land on a selected day
        if rule.kind == RecurrenceType::Custom && rule.unit == Some(RecurrenceUnit::Week)
            && let Some(days) = &rule.days
        {
            let n = next_date(d, &rule);
            prop_assert!(days.iter().any(|w| w.to_dow() == n.dow()));
        }
    }

    /// Ported from "recurrence: validate is idempotent".
    #[test]
    fn recurrence_validate_is_idempotent(r in raw_rec()) {
        let v1 = validate(Some(&r)).unwrap();
        let rule = v1.clone().expect("generator produces valid rules");
        let v2 = validate(Some(&RawRecurrence::from(rule))).unwrap();
        prop_assert_eq!(v1, v2);
    }
}
