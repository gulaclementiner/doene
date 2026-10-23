//! My Day group order (Gör't `src/agenda-order.js`): the validation and
//! persistence shape for the user-saved agenda group order.
//!
//! The rules mirror the drawer section order, extended with one movable
//! section break: unknown keys are dropped, missing groups re-appended in
//! default order, and exactly one break is kept — a missing break is
//! re-inserted at its default position.

/// The one section-break key: groups before it render under "Do first",
/// groups after under "Continue with".
pub const SECTION_BREAK: &str = "break";

/// The five My Day groups in default order. Radar sits below due today; it
/// stays in saved orders even while hidden — visibility is a settings
/// concern, never the registry.
pub const GROUP_KEYS: [&str; 5] = ["overdue", "starred-due", "shortlisted", "due-today", "radar"];

/// Where the section break sits by default: after the second group.
const BREAK_AT: usize = 2;

/// The spec's default group order: one section break, radar below due today.
pub const DEFAULT_ORDER: [&str; 6] = [
    "overdue",
    "starred-due",
    SECTION_BREAK,
    "shortlisted",
    "due-today",
    "radar",
];

/// Every key a saved agenda order may contain: the five groups plus exactly
/// one section break (length 6).
pub const ORDER_KEYS: [&str; 6] = [
    GROUP_KEYS[0],
    GROUP_KEYS[1],
    GROUP_KEYS[2],
    GROUP_KEYS[3],
    GROUP_KEYS[4],
    SECTION_BREAK,
];

/// Clean a stored My Day group order into a valid one (the drawer-order
/// rules, plus the break): non-string and unknown keys are dropped,
/// duplicates keep their first occurrence, missing groups are re-appended in
/// default order, and exactly one section break is kept — a missing break
/// goes back to its default position.
pub fn clean_agenda_order(a: &[Option<String>]) -> Vec<String> {
    // Keep only known string keys; the first occurrence wins.
    let mut out: Vec<String> = vec![];
    for key in a.iter().filter_map(|k| k.as_ref()) {
        if ORDER_KEYS.contains(&key.as_str()) && !out.iter().any(|x| x == key) {
            out.push(key.clone());
        }
    }
    // Re-append missing keys in default order; a missing break goes back to
    // its default position. By the time the break is processed, the two
    // groups before it are already in `out`, so index BREAK_AT is valid.
    for key in DEFAULT_ORDER {
        if out.iter().any(|x| x == key) {
            continue;
        }
        if key == SECTION_BREAK {
            out.insert(BREAK_AT, key.to_owned());
        } else {
            out.push(key.to_owned());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A string element of a stored order (Gör't's plain-string keys).
    fn k(s: &str) -> Option<String> {
        Some(s.to_owned())
    }

    // Gör't test/agenda-order.test.js — "the known keys are the five groups
    // plus exactly one section break (#98 adds radar)".
    #[test]
    fn order_keys_are_the_five_groups_plus_one_section_break() {
        assert_eq!(ORDER_KEYS.len(), 6);
        for key in ["overdue", "starred-due", "shortlisted", "due-today", "radar", SECTION_BREAK] {
            assert!(ORDER_KEYS.contains(&key), "missing {key}");
        }
    }

    // Gör't — "cleanAgendaOrder drops unknown and non-string keys and
    // re-appends missing groups in default order". (`42` and `null` in the
    // JS fixture are the non-string values; `None` here.)
    #[test]
    fn clean_drops_unknown_and_non_string_keys_and_reappends_missing_groups() {
        assert_eq!(
            clean_agenda_order(&[k("due-today"), k("bogus"), None, None]),
            ["due-today", "overdue", SECTION_BREAK, "starred-due", "shortlisted", "radar"]
        );
    }

    // Gör't — "cleanAgendaOrder drops duplicate keys; the first occurrence wins".
    #[test]
    fn clean_keeps_the_first_duplicate() {
        assert_eq!(
            clean_agenda_order(&[k("due-today"), k("overdue"), k("due-today")]),
            ["due-today", "overdue", SECTION_BREAK, "starred-due", "shortlisted", "radar"]
        );
    }

    // Gör't — "a missing break is re-inserted at its default position".
    #[test]
    fn a_missing_break_is_reinserted_at_its_default_position() {
        assert_eq!(
            clean_agenda_order(&[k("overdue"), k("starred-due"), k("shortlisted"), k("due-today")]),
            DEFAULT_ORDER.to_vec()
        );
    }

    // Gör't — "the radar key stays in a saved order even while hidden:
    // missing ones are appended, kept ones stay put (#98)".
    #[test]
    fn radar_stays_in_saved_orders_even_while_hidden() {
        // A pre-radar saved order gains radar at its default position (below due today).
        assert_eq!(
            clean_agenda_order(&[
                k("overdue"),
                k("starred-due"),
                k(SECTION_BREAK),
                k("shortlisted"),
                k("due-today")
            ]),
            DEFAULT_ORDER.to_vec()
        );
        // A user who dragged radar up keeps that position.
        let o = [
            k("radar"),
            k("overdue"),
            k(SECTION_BREAK),
            k("starred-due"),
            k("shortlisted"),
            k("due-today"),
        ];
        assert_eq!(clean_agenda_order(&o), o.into_iter().map(|x| x.unwrap()).collect::<Vec<_>>());
    }

    // Gör't — "at most one break survives; the first occurrence wins".
    #[test]
    fn at_most_one_break_survives_first_occurrence_wins() {
        assert_eq!(
            clean_agenda_order(&[k(SECTION_BREAK), k("overdue"), k(SECTION_BREAK)]),
            [SECTION_BREAK, "overdue", "starred-due", "shortlisted", "due-today", "radar"]
        );
    }

    // Gör't — "the break may sit anywhere in a saved order and stays there".
    #[test]
    fn the_break_may_sit_anywhere_and_stays_there() {
        let o = [
            k(SECTION_BREAK),
            k("overdue"),
            k("starred-due"),
            k("shortlisted"),
            k("due-today"),
            k("radar"),
        ];
        assert_eq!(clean_agenda_order(&o), o.into_iter().map(|x| x.unwrap()).collect::<Vec<_>>());
        let p = [
            k("overdue"),
            k("starred-due"),
            k("shortlisted"),
            k("due-today"),
            k("radar"),
            k(SECTION_BREAK),
        ];
        assert_eq!(clean_agenda_order(&p), p.into_iter().map(|x| x.unwrap()).collect::<Vec<_>>());
    }

    // Gör't — "a full custom group order round-trips unchanged".
    #[test]
    fn a_full_custom_group_order_round_trips_unchanged() {
        let o = [
            k("shortlisted"),
            k("due-today"),
            k(SECTION_BREAK),
            k("overdue"),
            k("starred-due"),
            k("radar"),
        ];
        assert_eq!(clean_agenda_order(&o), o.into_iter().map(|x| x.unwrap()).collect::<Vec<_>>());
    }
}
