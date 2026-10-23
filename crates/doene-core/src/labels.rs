//! Due-date and recurrence wording, shared by the task list, the drawer and the
//! composer chips (Gör't #88). Ported 1:1 from Gör't `src/labels.js`: wording is
//! the locale's natural casing (lowercase in en/sv); call sites capitalise where they want.
use std::collections::HashMap;

use crate::dates::Date;
use crate::entities::{Importance, RecurrenceRule, RecurrenceType, RecurrenceUnit, Weekday};
use crate::i18n::{js_number, locale_base, tr};

/// Index = recurrence day number, 0 = Sunday (`dow()` in dates.js). The NLP parser maps typed words to it too.
pub const WD: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

/// The `recurrence.*` key suffix for a recurrence kind.
fn type_key(kind: RecurrenceType) -> &'static str {
    match kind {
        RecurrenceType::Daily => "daily",
        RecurrenceType::Weekdays => "weekdays",
        RecurrenceType::Weekly => "weekly",
        RecurrenceType::Monthly => "monthly",
        RecurrenceType::Yearly => "yearly",
        RecurrenceType::Custom => "custom",
    }
}

/// The `recurrence.*` key suffix for a step unit.
fn unit_key(unit: RecurrenceUnit) -> &'static str {
    match unit {
        RecurrenceUnit::Day => "day",
        RecurrenceUnit::Week => "week",
        RecurrenceUnit::Month => "month",
        RecurrenceUnit::Year => "year",
    }
}

/// The day number (0 = Sunday) of a weekday.
fn dow(d: Weekday) -> u8 {
    match d {
        Weekday::Mon => 1,
        Weekday::Tue => 2,
        Weekday::Wed => 3,
        Weekday::Thu => 4,
        Weekday::Fri => 5,
        Weekday::Sat => 6,
        Weekday::Sun => 0,
    }
}

/// Pinned CLDR short month names per locale base (Gör't reads them from Intl;
/// Döne pins the exact strings — see docs/divergences.md). Unknown bases use English.
fn month_short(base: &str, month: u8) -> &'static str {
    match base {
        "sv" => match month {
            1 => "jan.",
            2 => "feb.",
            3 => "mars",
            4 => "apr.",
            5 => "maj",
            6 => "juni",
            7 => "juli",
            8 => "aug.",
            9 => "sep.",
            10 => "okt.",
            11 => "nov.",
            _ => "dec.",
        },
        _ => match month {
            1 => "Jan",
            2 => "Feb",
            3 => "Mar",
            4 => "Apr",
            5 => "May",
            6 => "Jun",
            7 => "Jul",
            8 => "Aug",
            9 => "Sep",
            10 => "Oct",
            11 => "Nov",
            _ => "Dec",
        },
    }
}

/// The weekday numbers in week order, starting at `start` (0 = Sunday, 1 = Monday).
pub fn week_days_from(start: u8) -> Vec<u8> {
    (0..7).map(|i| (start + i) % 7).collect()
}

/// "15 Mar" or "15 Mar 2027" in the given locale, day first as the list has always shown it.
/// The parts are reassembled day-first because Intl's own order is locale-dependent
/// (en renders month-first).
pub fn short_date(d: Date, locale: Option<&str>, with_year: bool) -> String {
    let base = locale_base(locale);
    let month = month_short(&base, d.month);
    if with_year {
        format!("{} {} {}", d.day, month, d.year)
    } else {
        format!("{} {}", d.day, month)
    }
}

/// The due-date label relative to today: today/tomorrow/yesterday, else the short date
/// (with the year when it differs from today's).
pub fn due_label(date_str: &str, today: &str, locale: Option<&str>) -> String {
    let date = Date::parse(date_str).expect("a valid YYYY-MM-DD due date");
    let t0 = Date::parse(today).expect("a valid YYYY-MM-DD today");
    let none = HashMap::new();
    if date == t0 {
        return tr("task.today", &none, locale);
    }
    if date == t0.add_days(1) {
        return tr("task.tomorrow", &none, locale);
    }
    if date == t0.add_days(-1) {
        return tr("task.yesterday", &none, locale);
    }
    short_date(date, locale, date.year != t0.year)
}

/// The recurrence wording in the locale's natural casing; custom day names are listed in week order, lowercased.
pub fn rec_label(rule: &RecurrenceRule, locale: Option<&str>, start_of_week: u8) -> String {
    let none = HashMap::new();
    if rule.kind != RecurrenceType::Custom {
        return tr(
            &format!("recurrence.{}", type_key(rule.kind)),
            &none,
            locale,
        );
    }
    let unit_name = unit_key(rule.unit);
    let unit_key = format!(
        "recurrence.{unit_name}{}",
        if rule.interval == 1 { "" } else { "s" }
    );
    let unit = tr(&unit_key, &none, locale);
    let mut params: HashMap<String, Option<String>> =
        [("n".to_owned(), Some(js_number(rule.interval as f64)))]
            .into_iter()
            .collect();
    params.insert("unit".to_owned(), Some(unit));
    let base = tr("recurrence.customEvery", &params, locale);
    let Some(days) = &rule.days else {
        return base;
    };
    let week = week_days_from(start_of_week);
    let mut ordered: Vec<u8> = days.iter().copied().map(dow).collect();
    ordered.sort_by_key(|d| week.iter().position(|w| w == d).unwrap_or(7));
    let names: Vec<String> = ordered
        .iter()
        .map(|d| tr(&format!("weekday.{}", WD[*d as usize]), &none, locale).to_lowercase())
        .collect();
    let joined = names.join(",");
    let on_days_params: HashMap<String, Option<String>> =
        [("days".to_owned(), Some(joined))].into_iter().collect();
    format!(
        "{} {}",
        base,
        tr("recurrence.onDays", &on_days_params, locale)
    )
}

/// The importance marker text (Gör't's NLP chip): high importance renders the
/// localized "importance: high" / "viktighet: hög"; normal has no marker.
pub fn importance_label(importance: Importance, locale: Option<&str>) -> Option<String> {
    if importance != Importance::High {
        return None;
    }
    let params: HashMap<String, Option<String>> =
        [("level".to_owned(), Some("importance.high".to_owned()))]
            .into_iter()
            .collect();
    Some(tr("chip.importance", &params, locale))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{RecurrenceType, RecurrenceUnit};

    fn rule(kind: RecurrenceType) -> RecurrenceRule {
        RecurrenceRule {
            kind,
            interval: 1,
            unit: RecurrenceUnit::Week,
            days: None,
        }
    }

    // Ported 1:1 from Gör't test/labels.test.js.

    #[test]
    fn short_date_day_first_with_the_locale_month_en_and_sv() {
        let d = Date::parse("2027-03-15").expect("valid");
        assert_eq!(short_date(d, Some("en"), false), "15 Mar");
        assert_eq!(short_date(d, Some("sv"), false), "15 mars");
    }

    #[test]
    fn short_date_with_year_appends_the_year() {
        let d = Date::parse("2027-03-15").expect("valid");
        assert_eq!(short_date(d, Some("en"), true), "15 Mar 2027");
        assert_eq!(short_date(d, Some("sv"), true), "15 mars 2027");
    }

    #[test]
    fn short_date_a_non_current_month_uses_its_own_month_name() {
        assert_eq!(
            short_date(Date::parse("2027-01-05").expect("valid"), Some("en"), false),
            "5 Jan"
        );
        assert_eq!(
            short_date(Date::parse("2027-12-01").expect("valid"), Some("sv"), false),
            "1 dec."
        );
    }

    #[test]
    fn due_label_today_tomorrow_and_yesterday_en_and_sv() {
        assert_eq!(due_label("2026-07-15", "2026-07-15", Some("en")), "Today");
        assert_eq!(
            due_label("2026-07-16", "2026-07-15", Some("en")),
            "Tomorrow"
        );
        assert_eq!(
            due_label("2026-07-14", "2026-07-15", Some("en")),
            "Yesterday"
        );
        assert_eq!(due_label("2026-07-15", "2026-07-15", Some("sv")), "Idag");
        assert_eq!(due_label("2026-07-16", "2026-07-15", Some("sv")), "Imorgon");
        assert_eq!(due_label("2026-07-14", "2026-07-15", Some("sv")), "Igår");
    }

    #[test]
    fn due_label_a_far_date_in_todays_year_has_no_year_en_and_sv() {
        assert_eq!(due_label("2026-09-01", "2026-07-15", Some("en")), "1 Sep");
        assert_eq!(due_label("2026-09-01", "2026-07-15", Some("sv")), "1 sep.");
    }

    #[test]
    fn due_label_a_far_date_in_a_different_year_carries_the_year_en_and_sv() {
        assert_eq!(
            due_label("2027-03-15", "2026-07-15", Some("en")),
            "15 Mar 2027"
        );
        assert_eq!(
            due_label("2025-03-15", "2026-07-15", Some("en")),
            "15 Mar 2025"
        );
        assert_eq!(
            due_label("2027-03-15", "2026-07-15", Some("sv")),
            "15 mars 2027"
        );
    }

    #[test]
    fn due_label_the_year_test_is_against_todays_year_at_the_calendar_edge() {
        // a different year from today → the year shows
        assert_eq!(
            due_label("2027-01-05", "2026-12-31", Some("en")),
            "5 Jan 2027"
        );
        // an earlier month of today's year → no year
        assert_eq!(due_label("2026-01-05", "2026-12-31", Some("en")), "5 Jan");
    }

    #[test]
    fn rec_label_simple_types_in_the_locales_natural_casing_en_and_sv() {
        for (kind, name) in [
            (RecurrenceType::Daily, "daily"),
            (RecurrenceType::Weekdays, "weekdays"),
            (RecurrenceType::Weekly, "weekly"),
            (RecurrenceType::Monthly, "monthly"),
            (RecurrenceType::Yearly, "yearly"),
        ] {
            assert_eq!(rec_label(&rule(kind), Some("en"), 0), name);
        }
        assert_eq!(
            rec_label(&rule(RecurrenceType::Daily), Some("sv"), 1),
            "dagligen"
        );
        assert_eq!(
            rec_label(&rule(RecurrenceType::Weekdays), Some("sv"), 1),
            "vardagar"
        );
        assert_eq!(
            rec_label(&rule(RecurrenceType::Weekly), Some("sv"), 1),
            "veckovis"
        );
        assert_eq!(
            rec_label(&rule(RecurrenceType::Monthly), Some("sv"), 1),
            "månadsvis"
        );
        assert_eq!(
            rec_label(&rule(RecurrenceType::Yearly), Some("sv"), 1),
            "årligen"
        );
    }

    #[test]
    fn rec_label_a_custom_rule_counts_its_unit_singular_and_plural() {
        let r = |unit: RecurrenceUnit, interval: u32| RecurrenceRule {
            kind: RecurrenceType::Custom,
            unit,
            interval,
            days: None,
        };
        assert_eq!(
            rec_label(&r(RecurrenceUnit::Week, 1), Some("en"), 0),
            "every 1 week"
        );
        assert_eq!(
            rec_label(&r(RecurrenceUnit::Week, 2), Some("en"), 0),
            "every 2 weeks"
        );
        assert_eq!(
            rec_label(&r(RecurrenceUnit::Month, 3), Some("en"), 0),
            "every 3 months"
        );
        assert_eq!(
            rec_label(&r(RecurrenceUnit::Day, 1), Some("sv"), 1),
            "varje 1 dag"
        );
        assert_eq!(
            rec_label(&r(RecurrenceUnit::Week, 2), Some("sv"), 1),
            "varje 2 veckor"
        );
    }

    #[test]
    fn rec_label_custom_days_are_listed_in_week_order_and_lowercased() {
        let r = |days: Vec<Weekday>| RecurrenceRule {
            kind: RecurrenceType::Custom,
            unit: RecurrenceUnit::Week,
            interval: 1,
            days: Some(days),
        };
        // Sunday-start: Sunday first
        assert_eq!(
            rec_label(&r(vec![Weekday::Sun, Weekday::Sat]), Some("en"), 0),
            "every 1 week on sun,sat"
        );
        // Monday-start: Saturday before Sunday
        assert_eq!(
            rec_label(&r(vec![Weekday::Sun, Weekday::Sat]), Some("en"), 1),
            "every 1 week on sat,sun"
        );
        // Monday-start: Monday first
        assert_eq!(
            rec_label(&r(vec![Weekday::Sat, Weekday::Mon]), Some("en"), 1),
            "every 1 week on mon,sat"
        );
        assert_eq!(
            rec_label(&r(vec![Weekday::Wed, Weekday::Mon]), Some("sv"), 1),
            "varje 1 vecka på mån,ons"
        );
        assert_eq!(
            rec_label(&r(vec![Weekday::Sun, Weekday::Fri]), Some("en"), 0),
            "every 1 week on sun,fri"
        );
    }

    #[test]
    fn drift_gone_the_shared_implementation_is_natural_cased_so_the_composer_chip_stays_lowercase()
    {
        assert_eq!(
            rec_label(&rule(RecurrenceType::Daily), Some("en"), 1),
            "daily"
        );
    }

    // The importance marker (Gör't NLP chip `chip('chip.importance', { level })`):
    // the param value is itself a locale key and resolves to its localized string.
    #[test]
    fn importance_marker_high_renders_the_localized_chip_text_normal_has_no_marker() {
        assert_eq!(
            importance_label(Importance::High, Some("en")),
            Some("importance: high".to_owned())
        );
        assert_eq!(
            importance_label(Importance::High, Some("sv")),
            Some("viktighet: hög".to_owned())
        );
        assert_eq!(importance_label(Importance::Normal, Some("en")), None);
    }
}
