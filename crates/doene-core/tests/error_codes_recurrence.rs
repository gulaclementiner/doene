//! Parity fixtures ported 1:1 from Gör't `test/module-error-codes.test.js`
//! (the three recurrence tests; the smart-list/quick-add/view error-code
//! tests land with their modules).

use doene_core::recurrence::{RawRecurrence, validate};

fn raw(kind: &str) -> RawRecurrence {
    RawRecurrence {
        kind: Some(kind.to_owned()),
        interval: None,
        unit: None,
        days: None,
    }
}

#[test]
fn recurrence_type_error_carries_code_and_params() {
    let e = validate(Some(&raw("nope"))).unwrap_err();
    assert_eq!(e.code, "error.recurrenceTypeInvalid");
    assert_eq!(e.param("type"), Some("nope"));
    assert_eq!(e.message, "bad recurrence type: nope");
}

#[test]
fn custom_recurrence_without_unit_carries_code() {
    let e = validate(Some(&raw("custom"))).unwrap_err();
    assert_eq!(e.code, "error.recurrenceUnitInvalid");
}

/// Ported from "unknown recurrence in nextDate carries code" — adapted per
/// divergence D2 (docs/divergences.md): the typed `RecurrenceRule` makes
/// `next_date` total, so an unknown type is rejected by `validate` with
/// `error.recurrenceTypeInvalid`; there is no `error.recurrenceInvalid`.
#[test]
fn unknown_type_rejected_by_validate() {
    let e = validate(Some(&raw("bogus"))).unwrap_err();
    assert_eq!(e.code, "error.recurrenceTypeInvalid");
}
