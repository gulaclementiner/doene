//! Parity fixtures ported 1:1 from Gör't `test/nlp-search.test.js` and the
//! date/time/importance/tag cases of `test/nlp-dates-recurrence-mutants.test.js`.
//! (The search test lands with ticket #06's search module; recurrence phrases
//! land in slice 3, keyword suggestions in slice 4.)

use std::collections::HashMap;

use doene_core::dates::Date;
use doene_core::entities::{RecurrenceRule, RecurrenceType, RecurrenceUnit, Weekday};
use doene_core::i18n;
use doene_core::nlp::{ImportanceLevel, ParseOptions, ParsedQuickAdd, parse_quick_add};

const TODAY: &str = "2025-03-12"; // Wednesday

fn today() -> Date {
    Date::parse(TODAY).expect("valid fixture date")
}

fn d(s: &str) -> Date {
    Date::parse(s).expect("valid fixture date")
}

fn p(text: &str) -> ParsedQuickAdd {
    parse_quick_add(text, today(), &ParseOptions::default())
}

/// Parse against an explicit "today" (Gör't's per-case `parseQuickAdd(t, date)`).
fn p_on(text: &str, day: &str) -> ParsedQuickAdd {
    parse_quick_add(text, d(day), &ParseOptions::default())
}

/// A fixed-kind recurrence rule (`daily`, …).
fn rtype(kind: RecurrenceType) -> RecurrenceRule {
    RecurrenceRule { kind, interval: None, unit: None, days: None }
}

/// A custom `every N <unit>` rule.
fn rcust(interval: u32, unit: RecurrenceUnit) -> RecurrenceRule {
    RecurrenceRule {
        kind: RecurrenceType::Custom,
        interval: Some(interval),
        unit: Some(unit),
        days: None,
    }
}

/// A custom weekly rule on the given weekdays.
fn rweek(days: Vec<Weekday>) -> RecurrenceRule {
    RecurrenceRule {
        kind: RecurrenceType::Custom,
        interval: Some(1),
        unit: Some(RecurrenceUnit::Week),
        days: Some(days),
    }
}

/// Render chips through the EN locale table, like Gör't's `chips()` helper.
fn chips(r: &ParsedQuickAdd) -> Vec<String> {
    r.chips
        .iter()
        .map(|c| {
            let vars: HashMap<String, Option<String>> = c
                .vars
                .iter()
                .map(|(k, v)| (k.clone(), Some(v.clone())))
                .collect();
            i18n::tr(&c.key, &vars, Some("en"))
        })
        .collect()
}

#[test]
fn plain_text_is_untouched() {
    let r = p("Buy milk");
    assert_eq!(r.subject, "Buy milk");
    assert_eq!(r.due_date, None);
    assert!(r.chips.is_empty());
}

// --- Gör't `nlp: due dates` (recurrence lines land in slice 3) -------------

#[test]
fn due_dates() {
    let cases = [
        ("x today", "2025-03-12"),
        ("x TODAY", "2025-03-12"),
        ("x tomorrow", "2025-03-13"),
        ("x tmrw", "2025-03-13"),
        ("x yesterday", "2025-03-11"),
        ("x tonight", "2025-03-12"),
        ("x in 3 days", "2025-03-15"),
        ("x in 1 day", "2025-03-13"),
        ("x in 2 weeks", "2025-03-26"),
        ("x in 2 months", "2025-05-12"),
        ("x next week", "2025-03-17"),
        ("x next month", "2025-04-12"),
        ("x friday", "2025-03-14"),
        ("x on Fri", "2025-03-14"),
        ("x next monday", "2025-03-17"),
        // A weekday name is the *next* occurrence: today's own weekday skips ahead.
        ("x wednesday", "2025-03-19"),
        ("x sun", "2025-03-16"),
        ("x tues", "2025-03-18"),
        ("x thurs", "2025-03-13"),
        ("x thu", "2025-03-13"),
        ("x thursday", "2025-03-13"),
        ("x sat", "2025-03-15"),
        ("x 2025-04-01", "2025-04-01"),
        ("x 2025-12-25", "2025-12-25"),
    ];
    for (text, due) in cases {
        assert_eq!(p(text).due_date, Some(d(due)), "{}", text);
    }
    // 'tonight' also defaults the time to 20:00; an explicit time wins.
    let r = p("x tonight");
    assert_eq!(r.time.as_deref(), Some("20:00"));
    assert_eq!(r.reminder_at, Some("2025-03-12T20:00".to_owned()));
    assert_eq!(p("x tonight at 7pm").time.as_deref(), Some("19:00"));

    // 'sun' inside a word is not a weekday; a lone date word is consumed.
    assert_eq!(p("buy sunflowers").due_date, None);
    assert_eq!(p("a tomorrow b").subject, "a b");
}

#[test]
fn due_date_chips_carry_the_iso_label() {
    let r = p("call mom tomorrow");
    assert_eq!(r.subject, "call mom");
    assert_eq!(chips(&r), vec!["due: 2025-03-13".to_owned()]);
}

#[test]
fn an_impossible_iso_date_stays_in_the_subject() {
    // Divergence D6: Gör't's JS Date.parse accepts impossible calendar days
    // (V8 rolls '2025-02-31' over), so it would set due_date to the raw string.
    // Döne rejects them at parse level (D1), so the token stays in the subject.
    let r = p("x 2025-02-31");
    assert_eq!(r.due_date, None);
    assert_eq!(r.subject, "x 2025-02-31");
}

#[test]
fn an_out_of_range_iso_date_stays_in_the_subject() {
    let r = p("x 2025-13-45");
    assert_eq!(r.due_date, None);
    assert_eq!(r.subject, "x 2025-13-45");
}

#[test]
fn weekday_names_are_consumed_as_dates() {
    assert_eq!(
        p("Read monday morning blues book").subject,
        "Read morning blues book"
    );
}

// --- Gör't `nlp: bare-hour times` ------------------------------------------

#[test]
fn bare_hour_times() {
    let cases = [
        ("x at 7", "07:00"),
        ("x at 9am", "09:00"),
        ("lunch 12:30pm", "12:30"),
        ("x at noon", "12:00"),
        ("x at midnight", "00:00"),
        ("x at 0", "00:00"),
    ];
    for (text, time) in cases {
        let r = p(text);
        assert_eq!(r.time.as_deref(), Some(time), "{text}");
        // A time without a date word pins the reminder to today.
        assert_eq!(r.due_date, Some(today()), "{text}");
        assert_eq!(r.reminder_at, Some(format!("2025-03-12T{time}")), "{text}");
    }
    // Out-of-range hours are not times.
    let r = p("x at 24");
    assert_eq!(r.time, None);
    assert_eq!(r.subject, "x at 24");
    // A bare number without 'at' is not a time.
    assert_eq!(p("x 7").time, None);
}

#[test]
fn time_chips() {
    let r = p("x at 7");
    assert_eq!(chips(&r), vec!["due: 2025-03-12".to_owned(), "at: 07:00".to_owned()]);
}

// --- Gör't `nlp: time sets a reminder` (nlp-search.test.js) ------------------

#[test]
fn time_sets_a_reminder() {
    let r = p("Standup tomorrow at 9am");
    assert_eq!(r.due_date, Some(d("2025-03-13")));
    assert_eq!(r.reminder_at.as_deref(), Some("2025-03-13T09:00"));

    let r = p("Call mom tonight");
    assert_eq!(r.due_date, Some(d("2025-03-12")));
    assert_eq!(r.reminder_at.as_deref(), Some("2025-03-12T20:00"));

    let r = p("Buy milk");
    assert_eq!(r.reminder_at, None);
}

// --- Gör't `nlp: importance and tags` (recurrence part lands in slice 3) ----

#[test]
fn importance_and_tags() {
    let r = p("Pay rent at 9am #home !high #bills");
    assert_eq!(r.subject, "Pay rent");
    assert_eq!(r.importance, ImportanceLevel::High);
    // Chips are in parse order: importance, tags, due, time.
    assert_eq!(
        chips(&r),
        vec![
            "importance: high".to_owned(),
            "tag: home".to_owned(),
            "tag: bills".to_owned(),
            "due: 2025-03-12".to_owned(),
            "at: 09:00".to_owned()
        ]
    );

    let r = p("x !low");
    assert_eq!(r.importance, ImportanceLevel::Low);
    assert_eq!(chips(&r), vec!["importance: low".to_owned()]);

    // A single '!' is not a marker (Gör't's regex needs `!high`, `!low` or
    // the double-bang `!!`) and stays in the subject.
    let r = p("x !");
    assert_eq!(r.importance, ImportanceLevel::Normal);
    assert_eq!(r.subject, "x !");
    assert!(r.chips.is_empty());

    // The double-bang is Gör't's bare high marker.
    let r = p("x !!");
    assert_eq!(r.importance, ImportanceLevel::High);
    assert_eq!(r.subject, "x");
}

// --- Gör't `nlp: recurrence keywords` ---------------------------------------

#[test]
fn recurrence_every_n_unit() {
    assert_eq!(p("x every 2 days").recurrence, Some(rcust(2, RecurrenceUnit::Day)));
    assert_eq!(p("x every 3 weeks").recurrence, Some(rcust(3, RecurrenceUnit::Week)));
    assert_eq!(p("x every 2 months").recurrence, Some(rcust(2, RecurrenceUnit::Month)));
    assert_eq!(p("x every 5 years").recurrence, Some(rcust(5, RecurrenceUnit::Year)));
    // Interval 1 collapses to the fixed kind.
    assert_eq!(p("x every 1 day").recurrence, Some(rtype(RecurrenceType::Daily)));
    assert_eq!(p("x every 1 week").recurrence, Some(rtype(RecurrenceType::Weekly)));
    assert_eq!(p("x every 1 month").recurrence, Some(rtype(RecurrenceType::Monthly)));
    assert_eq!(p("x every 1 year").recurrence, Some(rtype(RecurrenceType::Yearly)));
}

#[test]
fn recurrence_bare_keywords() {
    let cases = [
        ("daily", RecurrenceType::Daily),
        ("weekly", RecurrenceType::Weekly),
        ("monthly", RecurrenceType::Monthly),
        ("yearly", RecurrenceType::Yearly),
        ("annually", RecurrenceType::Yearly),
        ("every weekdays", RecurrenceType::Weekdays),
        ("every weekday", RecurrenceType::Weekdays),
        ("weekdays", RecurrenceType::Weekdays),
        ("Weekdays", RecurrenceType::Weekdays),
        ("every daily", RecurrenceType::Daily),
    ];
    for (word, kind) in cases {
        assert_eq!(p(&format!("x {word}")).recurrence, Some(rtype(kind)), "{}", word);
    }
    let every = [
        ("day", RecurrenceType::Daily),
        ("week", RecurrenceType::Weekly),
        ("month", RecurrenceType::Monthly),
        ("year", RecurrenceType::Yearly),
        ("weekday", RecurrenceType::Weekdays),
    ];
    for (word, kind) in every {
        assert_eq!(p(&format!("x every {word}")).recurrence, Some(rtype(kind)), "{}", word);
    }
    // Singular 'weekday' without 'every' is plain text.
    assert_eq!(p("x weekday").recurrence, None);
    // Case-insensitive.
    assert_eq!(p("x Daily").recurrence, Some(rtype(RecurrenceType::Daily)));
    assert_eq!(p("x ANNUALLY").recurrence, Some(rtype(RecurrenceType::Yearly)));
}

#[test]
fn recurrence_first_occurrence_due_date() {
    // Without a recFirst hook the due date is today, labelled 'first occurrence'.
    let r = p("x daily");
    assert_eq!(r.due_date, Some(today()));
    assert_eq!(chips(&r), vec!["due: first occurrence".to_owned(), "repeat: daily".to_owned()]);
    assert_eq!(r.subject, "x");

    let r = p("x every 2 weeks");
    assert_eq!(r.due_date, Some(today()));
    assert_eq!(chips(&r), vec!["due: first occurrence".to_owned(), "repeat: every 2 weeks".to_owned()]);

    for text in ["x weekly", "x monthly", "x yearly"] {
        assert_eq!(p(text).due_date, Some(today()), "{}", text);
    }

    // An explicit date wins over the first occurrence.
    assert_eq!(p("x daily tomorrow").due_date, Some(d("2025-03-13")));
}

#[test]
fn recurrence_weekday_lists() {
    let one = p("x every friday");
    assert_eq!(one.recurrence, Some(rtype(RecurrenceType::Weekly)));
    assert_eq!(one.due_date, Some(d("2025-03-14")));

    let many = p("x every mon, wed and fri");
    assert_eq!(
        many.recurrence,
        Some(rweek(vec![Weekday::Mon, Weekday::Wed, Weekday::Fri]))
    );
    assert_eq!(many.due_date, Some(d("2025-03-14")));
    assert!(chips(&many).contains(&"repeat: every 1 week on mon,wed,fri".to_owned()));

    assert_eq!(p("x every mon & tue").due_date, Some(d("2025-03-17")));
    assert_eq!(p("x every sat and sun").due_date, Some(d("2025-03-15")));
    assert_eq!(p("x every sunday, monday").due_date, Some(d("2025-03-16")));

    let r = p("x every wednesday");
    assert_eq!(r.recurrence, Some(rtype(RecurrenceType::Weekly)));
    assert_eq!(r.due_date, Some(d("2025-03-19")));

    // Explicit date wins over the weekday list's first occurrence.
    assert_eq!(p("x every fri tomorrow").due_date, Some(d("2025-03-13")));

    let r = p("x every fri and mon");
    assert_eq!(r.recurrence.as_ref().and_then(|r| r.days.clone()), Some(vec![Weekday::Mon, Weekday::Fri]));

    // Duplicates collapse to a single-day weekly rule.
    assert_eq!(p("x every mon and mon").recurrence, Some(rtype(RecurrenceType::Weekly)));
}

#[test]
fn recurrence_nth_of_month() {
    let m = p("rent every 1st");
    assert_eq!(m.recurrence, Some(rtype(RecurrenceType::Monthly)));
    assert_eq!(m.due_date, Some(d("2025-04-01")));

    // The first occurrence is strictly after today: the 12th itself skips to April.
    let cases = [
        ("x every 15th", "2025-03-15"),
        ("x every 12th", "2025-04-12"),
        ("x every 11th", "2025-04-11"),
        ("x every 2nd", "2025-04-02"),
        ("x every 3rd", "2025-04-03"),
        // March has a 31st and it is still ahead.
        ("x every 31st", "2025-03-31"),
    ];
    for (text, due) in cases {
        assert_eq!(p(text).due_date, Some(d(due)), "{}", text);
    }

    // Month-length clamping: 31st of a shorter month lands on its last day.
    assert_eq!(p_on("x every 31st", "2025-03-31").due_date, Some(d("2025-04-30")));
    assert_eq!(p_on("x every 30th", "2025-01-31").due_date, Some(d("2025-02-28")));
    assert_eq!(p_on("x every 31st", "2025-12-31").due_date, Some(d("2026-01-31")));

    // Out-of-range ordinals are not recurrences.
    assert_eq!(p("x every 32nd").recurrence, None);
    assert_eq!(p("x every 0th").recurrence, None);
}

#[test]
fn bare_weekdays_combines_with_times() {
    let w = p("Weekdays");
    assert_eq!(w.recurrence, Some(rtype(RecurrenceType::Weekdays)));
    assert_eq!(w.subject, "");

    let t = p("standup weekdays at 7");
    assert_eq!(t.recurrence, Some(rtype(RecurrenceType::Weekdays)));
    assert_eq!(t.time.as_deref(), Some("07:00"));
    assert_eq!(t.subject, "standup");
    assert_eq!(t.due_date, Some(today()));
    assert_eq!(t.reminder_at, Some(format!("{TODAY}T07:00")));

    for (text, time) in [
        ("weekdays at 7", "07:00"),
        ("x weekdays at 7am", "07:00"),
        ("x weekdays at 7pm", "19:00"),
        ("x weekdays at 7:30", "07:30"),
        ("x every weekday at 7", "07:00"),
    ] {
        assert_eq!(p(text).time.as_deref(), Some(time), "{}", text);
    }

    // On a Friday the first occurrence is today.
    assert_eq!(p_on("x weekdays", "2025-03-15").due_date, Some(d("2025-03-15")));
}

// --- Gör't `nlp: bare-hour times` (extended) ---------------------------------

#[test]
fn bare_hour_times_extended() {
    let cases = [
        ("x at 7", "07:00"),
        ("x at 0", "00:00"),
        ("x at 23", "23:00"),
        ("x at 12", "12:00"),
        ("x AT 9", "09:00"),
        ("x at 10", "10:00"),
        ("x at 7:45", "07:45"),
    ];
    for (text, time) in cases {
        assert_eq!(p(text).time.as_deref(), Some(time), "{}", text);
    }
    assert_eq!(p("x at 24").time, None);
    assert_eq!(p("x at 99").time, None);
    assert_eq!(chips(&p("x at 7")), vec!["due: 2025-03-12".to_owned(), "at: 07:00".to_owned()]);
}

// --- Gör't `nlp: combined sentence and whitespace` ----------------------------

#[test]
fn combined_sentence_and_whitespace() {
    let r = p("  Pay   rent every 1st at 9am #Home !high #bills ");
    assert_eq!(r.subject, "Pay rent");
    assert_eq!(r.importance, ImportanceLevel::High);
    // NLP keeps tag words raw; quick-add lowercases and dedupes them.
    let tags: Vec<&str> = r.tags.iter().map(|t| t.as_str()).collect();
    assert_eq!(tags, vec!["Home", "bills"]);
    assert_eq!(r.time.as_deref(), Some("09:00"));
    assert_eq!(r.due_date, Some(d("2025-04-01")));
    assert_eq!(r.reminder_at, Some("2025-04-01T09:00".to_owned()));

    // Empty and whitespace-only input parse to an empty subject.
    assert_eq!(p("").subject, "");
    assert_eq!(p("   ").subject, "");
}

// --- Gör't `nlp: keyword near-miss suggestions (en)` --------------------------
// A subject word within edit distance 2 of a known date keyword earns a
// "did you mean …?" hint. Exact matches, words under 4 letters and
// non-letter words are skipped. The word itself stays in the subject.

#[test]
fn keyword_suggestions() {
    for (typo, keyword) in [
        ("xoday", "today"),
        ("xonight", "tonight"),
        ("xomorrow", "tomorrow"),
        ("xmrw", "tmrw"),
        ("xesterday", "yesterday"),
        ("xoon", "noon"),
        ("xidnight", "midnight"),
        ("xaily", "daily"),
        ("xeekly", "weekly"),
        ("xonthly", "monthly"),
        ("xearly", "yearly"),
        ("xnnually", "annually"),
        ("xeekdays", "weekdays"),
        ("xunday", "sunday"),
        ("xonday", "monday"),
        ("xuesday", "tuesday"),
        ("xednesday", "wednesday"),
        ("xhursday", "thursday"),
        ("xriday", "friday"),
        ("xaturday", "saturday"),
        ("xeek", "week"),
        ("xonth", "month"),
        ("xear", "year"),
    ] {
        let s = p(&format!("x {typo}")).suggestions;
        assert!(
            s.iter().any(|c| c.word == typo && c.suggestion == keyword),
            "{typo} -> {keyword}"
        );
    }

    // Exact keywords are the words themselves, not near-misses.
    assert!(p("x today").suggestions.is_empty());
    assert!(p("x weekdays").suggestions.is_empty());
    // Words under 4 letters would light up almost everything within distance 2.
    assert!(p("x at xon").suggestions.iter().all(|c| c.word != "xon"));
    // Non-letter words (digits, punctuation) are skipped; the word stays in the subject.
    assert!(p("x 12345").suggestions.is_empty());
    let r = p("call xoday!");
    assert_eq!(r.subject, "call xoday!");
    assert!(r.suggestions.iter().all(|c| c.word != "xoday!"));
}
