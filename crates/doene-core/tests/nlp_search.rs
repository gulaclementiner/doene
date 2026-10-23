//! Parity fixtures ported 1:1 from Gör't `test/nlp-search.test.js` and the
//! date/time/importance/tag cases of `test/nlp-dates-recurrence-mutants.test.js`.
//! (The search test lands with ticket #06's search module; recurrence phrases
//! land in slice 3, keyword suggestions in slice 4.)

use std::collections::HashMap;

use doene_core::dates::Date;
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
        ("x tomorrow", "2025-03-13"),
        ("x tmrw", "2025-03-13"),
        ("x yesterday", "2025-03-11"),
        ("x tonight", "2025-03-12"),
        ("x in 3 days", "2025-03-15"),
        ("x in 2 weeks", "2025-03-26"),
        ("x friday", "2025-03-14"),
        ("x on Fri", "2025-03-14"),
        ("x next week", "2025-03-17"),
        ("x next month", "2025-04-12"),
        ("x 2025-04-01", "2025-04-01"),
    ];
    for (text, due) in cases {
        assert_eq!(p(text).due_date, Some(d(due)), "{text}");
    }
    // 'tonight' also defaults the time to 20:00.
    let r = p("x tonight");
    assert_eq!(r.time.as_deref(), Some("20:00"));
    assert_eq!(r.reminder_at.as_deref(), Some("2025-03-12T20:00"));
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

// --- Gör't `nlp: combined sentence and whitespace` (recurrence part lands in slice 3)

#[test]
fn combined_sentence_and_whitespace() {
    let r = p("Pay rent at 9am #Home !high #bills");
    assert_eq!(r.subject, "Pay rent");
    assert_eq!(r.importance, ImportanceLevel::High);
    // NLP keeps tag words raw; quick-add lowercases and dedupes them.
    let tags: Vec<&str> = r.tags.iter().map(|t| t.as_str()).collect();
    assert_eq!(tags, vec!["Home", "bills"]);
}
