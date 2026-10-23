//! Parity fixtures ported 1:1 from Gör't `test/search-recurrence-mutants.test.js`
//! (the three recurrence tests; the search tests land with the search module).

use doene_core::dates::Date;
use doene_core::entities::{RecurrenceRule, RecurrenceType, RecurrenceUnit, Weekday};
use doene_core::recurrence::{RawRecurrence, TYPES, next_date, next_on_or_after, validate};

fn d(s: &str) -> Date {
    Date::parse(s).expect("valid fixture date")
}

/// Gör't's `T` helper: a raw rule with just a type.
fn t(kind: &str) -> RawRecurrence {
    RawRecurrence {
        kind: Some(kind.to_owned()),
        interval: None,
        unit: None,
        days: None,
    }
}

/// A validated simple-type rule (Gör't passes plain objects to nextDate; the
/// typed equivalent carries no custom fields).
fn simple(kind: RecurrenceType) -> RecurrenceRule {
    RecurrenceRule {
        kind,
        interval: None,
        unit: None,
        days: None,
    }
}

#[test]
fn recurrence_validate() {
    // validate(null/undefined) == null (Rust's None covers both).
    assert_eq!(validate(None), Ok(None));

    // Non-custom types drop the interval.
    let mut raw = t("daily");
    raw.interval = Some(5.0);
    assert_eq!(
        validate(Some(&raw)),
        Ok(Some(simple(RecurrenceType::Daily)))
    );

    // Unknown type → coded error with the English message.
    let err = validate(Some(&t("nope"))).unwrap_err();
    assert_eq!(err.message, "bad recurrence type: nope");

    // Custom without a unit (or with an unknown unit) → coded error.
    for raw in [
        RawRecurrence {
            kind: Some("custom".to_owned()),
            interval: None,
            unit: None,
            days: None,
        },
        RawRecurrence {
            kind: Some("custom".to_owned()),
            interval: None,
            unit: Some("hour".to_owned()),
            days: None,
        },
    ] {
        let err = validate(Some(&raw)).unwrap_err();
        assert!(
            err.message.contains("needs unit"),
            "message: {}",
            err.message
        );
    }

    // Each valid unit is preserved.
    for (unit, expected) in [
        ("day", RecurrenceUnit::Day),
        ("week", RecurrenceUnit::Week),
        ("month", RecurrenceUnit::Month),
        ("year", RecurrenceUnit::Year),
    ] {
        let raw = RawRecurrence {
            kind: Some("custom".to_owned()),
            interval: None,
            unit: Some(unit.to_owned()),
            days: None,
        };
        assert_eq!(validate(Some(&raw)).unwrap().unwrap().unit, Some(expected));
    }

    // Interval: default 1; clamped to ≥ 1; floored.
    let mut raw = RawRecurrence {
        kind: Some("custom".to_owned()),
        interval: None,
        unit: Some("day".to_owned()),
        days: None,
    };
    assert_eq!(validate(Some(&raw)).unwrap().unwrap().interval, Some(1));
    for (input, expected) in [(0.0, 1), (-4.0, 1), (3.9, 3), (7.0, 7)] {
        raw.interval = Some(input);
        assert_eq!(
            validate(Some(&raw)).unwrap().unwrap().interval,
            Some(expected)
        );
    }

    // Days: deduped, sorted, filtered to 0..=6 (numeric strings coerced).
    let mut raw = RawRecurrence {
        kind: Some("custom".to_owned()),
        interval: None,
        unit: Some("week".to_owned()),
        days: None,
    };
    raw.days = Some(
        vec!["3", "1", "3", "5", "9", "-1", "6", "0"]
            .into_iter()
            .map(String::from)
            .collect(),
    );
    assert_eq!(
        validate(Some(&raw)).unwrap().unwrap().days,
        Some(vec![
            Weekday::Sun,
            Weekday::Mon,
            Weekday::Wed,
            Weekday::Fri,
            Weekday::Sat
        ])
    );

    // Days: empty list → no days.
    raw.days = Some(vec![]);
    assert_eq!(validate(Some(&raw)).unwrap().unwrap().days, None);

    // Non-numeric weekday tokens are dropped, not coerced (divergence D3:
    // Gör't's JS `Number("")` would coerce "" to Sunday).
    raw.days = Some(vec!["", "1e0", "3"].into_iter().map(String::from).collect());
    assert_eq!(
        validate(Some(&raw)).unwrap().unwrap().days,
        Some(vec![Weekday::Wed])
    );

    // No days → none.
    let raw2 = RawRecurrence {
        kind: Some("custom".to_owned()),
        interval: None,
        unit: Some("week".to_owned()),
        days: None,
    };
    assert_eq!(validate(Some(&raw2)).unwrap().unwrap().days, None);

    // Days only for weeks (dropped for other units).
    let mut raw3 = RawRecurrence {
        kind: Some("custom".to_owned()),
        interval: None,
        unit: Some("day".to_owned()),
        days: None,
    };
    raw3.days = Some(vec!["1".to_owned()]);
    assert_eq!(validate(Some(&raw3)).unwrap().unwrap().days, None);

    // Boundary weekdays 0 (Sun) and 6 (Sat) are kept.
    let mut raw4 = RawRecurrence {
        kind: Some("custom".to_owned()),
        interval: None,
        unit: Some("week".to_owned()),
        days: None,
    };
    raw4.days = Some(vec!["0".to_owned(), "6".to_owned()]);
    assert_eq!(
        validate(Some(&raw4)).unwrap().unwrap().days,
        Some(vec![Weekday::Sun, Weekday::Sat])
    );

    // Six kinds.
    assert_eq!(TYPES.len(), 6);
}

#[test]
fn recurrence_next_date_every_type() {
    // daily
    assert_eq!(
        next_date(d("2025-03-12"), &simple(RecurrenceType::Daily)),
        d("2025-03-13")
    );
    // weekly
    assert_eq!(
        next_date(d("2025-03-12"), &simple(RecurrenceType::Weekly)),
        d("2025-03-19")
    );
    // monthly (clamped)
    assert_eq!(
        next_date(d("2025-01-31"), &simple(RecurrenceType::Monthly)),
        d("2025-02-28")
    );
    // yearly
    assert_eq!(
        next_date(d("2024-02-29"), &simple(RecurrenceType::Yearly)),
        d("2025-02-28")
    );

    // weekdays: Fri -> Mon, Wed -> Thu, Sat -> Mon, Sun -> Mon
    assert_eq!(
        next_date(d("2025-03-14"), &simple(RecurrenceType::Weekdays)),
        d("2025-03-17")
    );
    assert_eq!(
        next_date(d("2025-03-12"), &simple(RecurrenceType::Weekdays)),
        d("2025-03-13")
    );
    assert_eq!(
        next_date(d("2025-03-15"), &simple(RecurrenceType::Weekdays)),
        d("2025-03-17")
    );
    assert_eq!(
        next_date(d("2025-03-16"), &simple(RecurrenceType::Weekdays)),
        d("2025-03-17")
    );

    // custom day interval 3 / month 2 / year 2
    let mut r = simple(RecurrenceType::Custom);
    r.unit = Some(RecurrenceUnit::Day);
    r.interval = Some(3);
    assert_eq!(next_date(d("2025-03-12"), &r), d("2025-03-15"));
    r.unit = Some(RecurrenceUnit::Month);
    r.interval = Some(2);
    assert_eq!(next_date(d("2025-01-15"), &r), d("2025-03-15"));
    r.unit = Some(RecurrenceUnit::Year);
    r.interval = Some(2);
    assert_eq!(next_date(d("2024-06-10"), &r), d("2026-06-10"));

    // custom week interval 2 (no days)
    r.unit = Some(RecurrenceUnit::Week);
    r.interval = Some(2);
    assert_eq!(next_date(d("2025-03-12"), &r), d("2025-03-26"));

    // custom day/week default interval 1
    r.unit = Some(RecurrenceUnit::Day);
    r.interval = None;
    assert_eq!(next_date(d("2025-03-12"), &r), d("2025-03-13"));
    r.unit = Some(RecurrenceUnit::Week);
    r.interval = None;
    assert_eq!(next_date(d("2025-03-12"), &r), d("2025-03-19"));

    // custom week interval 2 with days [] → plain biweekly
    r.days = Some(vec![]);
    r.interval = Some(2);
    assert_eq!(next_date(d("2025-03-12"), &r), d("2025-03-26"));

    // custom week with a weekday set: later selected day in the same week,
    // else the first selected day n weeks out.
    r.interval = Some(1);
    r.days = Some(vec![Weekday::Mon, Weekday::Fri]);
    assert_eq!(next_date(d("2025-03-12"), &r), d("2025-03-14")); // Wed -> Fri same week
    r.interval = Some(2);
    assert_eq!(next_date(d("2025-03-14"), &r), d("2025-03-24")); // Fri -> Mon 2 weeks out

    // weekly with Monday+Friday: from Friday → next week's Monday
    r.interval = Some(1);
    assert_eq!(next_date(d("2025-03-14"), &r), d("2025-03-17"));

    // Sunday is picked when selected (from Saturday)
    r.days = Some(vec![Weekday::Sun]);
    assert_eq!(next_date(d("2025-03-15"), &r), d("2025-03-16"));

    // Sunday wraps to next week's Sunday when today is the selected day
    assert_eq!(next_date(d("2025-03-16"), &r), d("2025-03-23"));

    // Unknown types are rejected by validate — see error_codes_recurrence.rs (D2).
}

#[test]
fn recurrence_next_on_or_after() {
    // floor in the past: same as nextDate
    assert_eq!(
        next_on_or_after(
            d("2025-03-12"),
            &simple(RecurrenceType::Daily),
            d("2025-03-01")
        ),
        d("2025-03-13")
    );
    // floor == next occurrence: that date
    assert_eq!(
        next_on_or_after(
            d("2025-03-12"),
            &simple(RecurrenceType::Daily),
            d("2025-03-13")
        ),
        d("2025-03-13")
    );
    // floor beyond the next occurrence: advance until on/after
    assert_eq!(
        next_on_or_after(
            d("2025-03-12"),
            &simple(RecurrenceType::Daily),
            d("2025-03-20")
        ),
        d("2025-03-20")
    );
    // weekly: 03-19 < 03-20 → 03-26
    assert_eq!(
        next_on_or_after(
            d("2025-03-12"),
            &simple(RecurrenceType::Weekly),
            d("2025-03-20")
        ),
        d("2025-03-26")
    );
    // safety valve: a far floor terminates (result before the floor)
    let r = next_on_or_after(
        d("2025-03-12"),
        &simple(RecurrenceType::Daily),
        d("9999-01-01"),
    );
    assert!(r < d("9999-01-01"));
}
