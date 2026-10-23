//! Parity fixtures ported 1:1 from Gör't `test/nlp-sv.test.js` and
//! `test/nlp-suggest.test.js`, plus pins for the divergences D8-D10.

use std::collections::HashMap;

use doene_core::dates::Date;
use doene_core::entities::{RecurrenceRule, RecurrenceType, RecurrenceUnit, Weekday};
use doene_core::i18n;
use doene_core::nlp::{
    ImportanceLevel, NearMiss, ParseOptions, ParsedQuickAdd, edit_distance, find_near_miss_ranges,
    parse_quick_add, replace_range,
};

const TODAY: &str = "2026-01-07"; // a Wednesday

fn date(s: &str) -> Date {
    Date::parse(s).expect("valid date")
}

fn opts(locale: Option<&str>) -> ParseOptions {
    ParseOptions {
        locale: locale.map(str::to_owned),
        tags: true,
    }
}

fn pl(text: &str, today: &str, locale: Option<&str>) -> ParsedQuickAdd {
    parse_quick_add(text, date(today), &opts(locale))
}

fn p(text: &str) -> ParsedQuickAdd {
    pl(text, TODAY, Some("sv"))
}

/// English (default locale) parse.
fn en(text: &str) -> ParsedQuickAdd {
    pl(text, TODAY, None)
}

fn d(text: &str) -> Option<String> {
    p(text).due_date.map(|x| x.to_string())
}

fn due(r: &ParsedQuickAdd) -> Option<String> {
    r.due_date.map(|x| x.to_string())
}

fn t(text: &str) -> Option<String> {
    p(text).time
}

fn r(text: &str) -> Option<RecurrenceRule> {
    p(text).recurrence
}

fn fixed(kind: RecurrenceType) -> Option<RecurrenceRule> {
    Some(RecurrenceRule {
        kind,
        interval: None,
        unit: None,
        days: None,
    })
}

fn every(interval: u32, unit: RecurrenceUnit) -> Option<RecurrenceRule> {
    Some(RecurrenceRule {
        kind: RecurrenceType::Custom,
        interval: Some(interval),
        unit: Some(unit),
        days: None,
    })
}

/// `{ type: 'custom', unit: 'week', interval: 1, days }` from dow numbers.
fn weekly_on(days: &[u8]) -> Option<RecurrenceRule> {
    Some(RecurrenceRule {
        kind: RecurrenceType::Custom,
        interval: Some(1),
        unit: Some(RecurrenceUnit::Week),
        days: Some(
            days.iter()
                .map(|d| Weekday::from_dow(*d).unwrap())
                .collect(),
        ),
    })
}

fn chips(r: &ParsedQuickAdd, lang: &str) -> Vec<String> {
    r.chips
        .iter()
        .map(|c| {
            let vars: HashMap<String, Option<String>> = c
                .vars
                .iter()
                .map(|(k, v)| (k.clone(), Some(v.clone())))
                .collect();
            i18n::tr(&c.key, &vars, Some(lang))
        })
        .collect()
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_owned()).collect()
}

fn tags(r: &ParsedQuickAdd) -> Vec<String> {
    r.tags.iter().map(|t| t.as_str().to_owned()).collect()
}

fn some(x: &str) -> Option<String> {
    Some(x.to_owned())
}

#[test]
fn relative_day_words() {
    assert_eq!(d("Betala hyran imorgon"), some("2026-01-08"));
    assert_eq!(d("Jobb i morgon"), some("2026-01-08"));
    assert_eq!(d("Jobb idag"), some(TODAY));
    assert_eq!(d("Jobb i dag"), some(TODAY));
    assert_eq!(d("Jobb i övermorgon"), some("2026-01-09"));
    assert_eq!(d("Jobb igår"), some("2026-01-06"));
    assert_eq!(d("Jobb i går"), some("2026-01-06"));
    assert_eq!(p("Betala hyran imorgon").subject, "Betala hyran");
    let kv = p("Jobb ikväll");
    assert_eq!(due(&kv), some(TODAY));
    assert_eq!(kv.time.as_deref(), Some("20:00"));
    assert_eq!(chips(&kv, "en"), s(&["due: 2026-01-07", "at: 20:00"]));
    assert_eq!(p("Jobb i kväll").time.as_deref(), Some("20:00"));
    assert_eq!(p("Jobb ikväll kl 19").time.as_deref(), Some("19:00"));
    assert_eq!(p("Jobb IMORGON").due_date, Some(date("2026-01-08")));
}

#[test]
fn weekdays_with_and_without_pa() {
    for (text, want) in [
        ("Möte på fredag", "2026-01-09"),
        ("Möte fredag", "2026-01-09"),
        ("Möte på fre", "2026-01-09"),
        ("Möte på måndag", "2026-01-12"),
        ("Möte på mån", "2026-01-12"),
        ("Möte på tisdag", "2026-01-13"),
        ("Möte på tis", "2026-01-13"),
        ("Möte på onsdag", "2026-01-14"),
        ("Möte på ons", "2026-01-14"),
        ("Möte på torsdag", "2026-01-08"),
        ("Möte på tors", "2026-01-08"),
        ("Möte på lördag", "2026-01-10"),
        ("Möte på lör", "2026-01-10"),
        ("Möte på söndag", "2026-01-11"),
        ("Möte på sön", "2026-01-11"),
    ] {
        assert_eq!(d(text), some(want), "{text}");
    }
    assert_eq!(p("Möte på fredag").subject, "Möte");
}

#[test]
fn definite_weekday_form_is_parsed() {
    let r = p("Möte på söndagen");
    assert_eq!(due(&r), some("2026-01-11"));
    assert_eq!(r.subject, "Möte");
    assert_eq!(d("Möte på måndagen"), some("2026-01-12"));
}

#[test]
fn om_n_units() {
    for (text, want) in [
        ("Jobb om 3 dagar", "2026-01-10"),
        ("Jobb om 1 dag", "2026-01-08"),
        ("Jobb om 2 veckor", "2026-01-21"),
        ("Jobb om 1 vecka", "2026-01-14"),
        ("Jobb om 3 månader", "2026-04-07"),
        ("Jobb om 1 månad", "2026-02-07"),
    ] {
        assert_eq!(d(text), some(want), "{text}");
    }
    assert_eq!(p("Jobb om 3 dagar").subject, "Jobb");
}

#[test]
fn nasta_and_i_veckan() {
    assert_eq!(d("Jobb nästa vecka"), some("2026-01-12"));
    assert_eq!(d("Jobb nästa månad"), some("2026-02-07"));
    assert_eq!(d("Jobb nästa måndag"), some("2026-01-12"));
    assert_eq!(d("Jobb nästa onsdag"), some("2026-01-14"));
    assert_eq!(d("Lunch i veckan"), some("2026-01-09"));
    assert_eq!(p("Lunch i veckan").subject, "Lunch");
}

#[test]
fn times_with_kl_klockan_vid() {
    for (text, want) in [
        ("Träna kl 18", Some("18:00")),
        ("Träna kl 18:30", Some("18:30")),
        ("Träna kl 08:05", Some("08:05")),
        ("Ring klockan 9", Some("09:00")),
        ("Ring klockan 9:15", Some("09:15")),
        ("Ring vid 9", Some("09:00")),
        ("Ring vid 21:45", Some("21:45")),
        ("Träna kl. 8", Some("08:00")),
        ("Träna kl 8.15", Some("08:15")),
        ("Träna KL 18", Some("18:00")),
        ("Träna kl 24", None),
        ("Träna kl 18:60", None),
        ("Träna 18", None),
    ] {
        assert_eq!(t(text).as_deref(), want, "{text}");
    }
    assert_eq!(p("Träna kl 18:60").subject, "Träna kl 18:60");
    let kl = p("Träna kl 18");
    assert_eq!(due(&kl), some(TODAY));
    assert_eq!(kl.reminder_at, some("2026-01-07T18:00"));
    assert_eq!(chips(&kl, "en"), s(&["due: 2026-01-07", "at: 18:00"]));
    assert_eq!(kl.subject, "Träna");
}

#[test]
fn varje_unit_and_weekday() {
    assert_eq!(r("Städa varje dag"), fixed(RecurrenceType::Daily));
    assert_eq!(r("Städa varje vecka"), fixed(RecurrenceType::Weekly));
    assert_eq!(r("Städa varje månad"), fixed(RecurrenceType::Monthly));
    assert_eq!(r("Städa varje år"), fixed(RecurrenceType::Yearly));
    let wd = p("Städa varje måndag");
    assert_eq!(wd.recurrence, fixed(RecurrenceType::Weekly));
    assert_eq!(due(&wd), some("2026-01-12"));
    assert_eq!(wd.subject, "Städa");
    let many = p("Städa varje måndag och fredag");
    assert_eq!(many.recurrence, weekly_on(&[1, 5]));
    assert_eq!(due(&many), some("2026-01-09"));
    assert!(chips(&many, "sv").contains(&"upprepa: varje 1 vecka på mån,fre".to_owned()));
    let day = p("Städa varje dag");
    assert_eq!(due(&day), some(TODAY));
    assert_eq!(
        chips(&day, "sv"),
        s(&["förfaller: första tillfället", "upprepa: dagligen"])
    );
    let keys = |r: &ParsedQuickAdd| r.chips.iter().map(|c| c.key.clone()).collect::<Vec<_>>();
    assert_eq!(
        keys(&p("Städa varje måndag")),
        keys(&en("Clean every monday"))
    );
}

#[test]
fn varannan_and_var_tredje() {
    assert_eq!(r("Städa varannan dag"), every(2, RecurrenceUnit::Day));
    assert_eq!(r("Städa varannan vecka"), every(2, RecurrenceUnit::Week));
    assert_eq!(r("Städa varannan månad"), every(2, RecurrenceUnit::Month));
    assert_eq!(r("Städa varannat år"), every(2, RecurrenceUnit::Year));
    assert_eq!(r("Städa var tredje dag"), every(3, RecurrenceUnit::Day));
    assert_eq!(r("Städa var fjärde vecka"), every(4, RecurrenceUnit::Week));
    assert_eq!(r("Städa var andra månad"), every(2, RecurrenceUnit::Month));
    assert_eq!(p("Städa var tredje dag").due_date, Some(date(TODAY)));
    assert_eq!(p("Städa var tredje dag").subject, "Städa");
    assert_eq!(r("Städa varannan"), None);
}

#[test]
fn importance_and_tags() {
    let m = p("Möte på fredag #jobb !hög");
    assert_eq!(tags(&m), s(&["jobb"]));
    assert_eq!(m.importance, ImportanceLevel::High);
    assert_eq!(due(&m), some("2026-01-09"));
    assert_eq!(m.subject, "Möte");
    assert_eq!(
        chips(&m, "en"),
        s(&["importance: high", "tag: jobb", "due: 2026-01-09"])
    );
    assert_eq!(p("Möte !låg").importance, ImportanceLevel::Low);
    assert_eq!(p("Möte !!").importance, ImportanceLevel::High);
    assert_eq!(p("Möte !HÖG").importance, ImportanceLevel::High);
    assert_eq!(p("Möte !high").importance, ImportanceLevel::High);
    assert_eq!(p("Möte !low").importance, ImportanceLevel::Low);
    assert_eq!(tags(&p("Möte #bröd")), s(&["bröd"]));
    assert_eq!(p("Möte #jobb").subject, "Möte");
}

#[test]
fn explicit_dates_iso_and_dmy() {
    assert_eq!(d("Möte 2026-03-15"), some("2026-03-15"));
    assert_eq!(d("Möte 15/3"), some("2026-03-15"));
    assert_eq!(d("Möte 15/3 2027"), some("2027-03-15"));
    assert_eq!(d("Möte 1/12"), some("2026-12-01"));
    let on = |text, today| pl(text, today, Some("sv")).due_date.map(|x| x.to_string());
    assert_eq!(on("Möte 15/3", "2026-06-01"), some("2027-03-15"));
    assert_eq!(on("Möte 15/3", "2026-03-15"), some("2026-03-15"));
    assert_eq!(d("Möte 31/2"), None);
    assert_eq!(p("Möte 31/2").subject, "Möte 31/2");
    assert_eq!(d("Möte 31/2 2027"), None);
    assert_eq!(p("Möte 31/2 2027").subject, "Möte 31/2 2027");
    assert_eq!(d("Möte 15/13"), None);
    assert_eq!(p("Möte 15/13").subject, "Möte 15/13");
    assert_eq!(d("Möte 15/0"), None);
    assert_eq!(d("Möte 0/3"), None);
    assert_eq!(d("Möte 2026-13-45"), None);
    assert_eq!(on("Möte 29/2", "2028-06-01"), some("2032-02-29"));
    assert_eq!(on("Möte 29/2", "2096-03-01"), some("2104-02-29"));
    assert_eq!(d("Möte 29/2 2027"), None);
}

#[test]
fn english_keywords_keep_working_in_swedish() {
    for (text, want) in [
        ("Betala hyran tomorrow", "2026-01-08"),
        ("Betala hyran today", TODAY),
        ("x in 3 days", "2026-01-10"),
        ("x in 2 weeks", "2026-01-21"),
        ("x in 2 months", "2026-03-07"),
        ("x next week", "2026-01-12"),
        ("x next month", "2026-02-07"),
        ("x on Fri", "2026-01-09"),
        ("x next monday", "2026-01-12"),
        ("x 2026-12-25", "2026-12-25"),
    ] {
        assert_eq!(d(text), some(want), "{text}");
    }
    assert_eq!(r("x every day"), fixed(RecurrenceType::Daily));
    assert_eq!(r("x every 2 weeks"), every(2, RecurrenceUnit::Week));
    assert_eq!(r("x every friday"), fixed(RecurrenceType::Weekly));
    assert_eq!(r("x every mon, wed and fri"), weekly_on(&[1, 3, 5]));
    assert_eq!(r("x every 1st"), fixed(RecurrenceType::Monthly));
    assert_eq!(r("x daily"), fixed(RecurrenceType::Daily));
    assert_eq!(r("x weekdays"), fixed(RecurrenceType::Weekdays));
    assert_eq!(t("x at 9am").as_deref(), Some("09:00"));
    assert_eq!(t("x at 7pm").as_deref(), Some("19:00"));
    assert_eq!(t("x at 7:45").as_deref(), Some("07:45"));
    assert_eq!(t("x at 7").as_deref(), Some("07:00"));
    assert_eq!(p("x tonight").time.as_deref(), Some("20:00"));
    assert_eq!(t("x noon").as_deref(), Some("12:00"));
}

#[test]
fn full_swedish_sentence() {
    let q = p("  Betala   hyran imorgon kl 9:30 #hemma !hög ");
    assert_eq!(q.subject, "Betala hyran");
    assert_eq!(due(&q), some("2026-01-08"));
    assert_eq!(q.time.as_deref(), Some("09:30"));
    assert_eq!(q.reminder_at, some("2026-01-08T09:30"));
    assert_eq!(tags(&q), s(&["hemma"]));
    assert_eq!(q.importance, ImportanceLevel::High);
    let lunch = p("Lunch idag kl 12");
    assert_eq!(due(&lunch), some(TODAY));
    assert_eq!(lunch.time.as_deref(), Some("12:00"));
    assert_eq!(lunch.subject, "Lunch");
    assert_eq!(chips(&lunch, "en"), s(&["due: 2026-01-07", "at: 12:00"]));
}

#[test]
fn english_default_ignores_swedish_keywords() {
    assert_eq!(en("Betala hyran imorgon").due_date, None);
    assert_eq!(en("Betala hyran imorgon").subject, "Betala hyran imorgon");
    assert_eq!(en("Möte på fredag").due_date, None);
    assert_eq!(en("Träna kl 18").time, None);
    assert_eq!(en("Ring vid 9").time, None);
    for text in [
        "Städa varje dag",
        "Städa varannan dag",
        "Städa var tredje dag",
    ] {
        assert_eq!(en(text).recurrence, None, "{text}");
    }
    for text in [
        "Jobb nästa vecka",
        "Lunch i veckan",
        "Jobb om 3 dagar",
        "Möte 15/3",
    ] {
        assert_eq!(en(text).due_date, None, "{text}");
    }
    assert_eq!(en("x at 7.45").time, None);
    assert_eq!(en("Möte !hög").importance, ImportanceLevel::Normal);
    assert_eq!(en("Möte !låg").importance, ImportanceLevel::Normal);
    assert_eq!(
        pl("Möte !hög", TODAY, Some("en")).importance,
        ImportanceLevel::Normal
    );
    assert_eq!(tags(&en("Möte #jobb")), s(&["jobb"]));
}

#[test]
fn regional_and_padded_locale_tags() {
    for loc in ["sv-SE", "SV_se", " sv "] {
        let q = pl("Jobb imorgon", TODAY, Some(loc));
        assert_eq!(due(&q), some("2026-01-08"), "{loc}");
    }
    assert_eq!(pl("Jobb imorgon", TODAY, Some("de")).due_date, None);
    assert_eq!(pl("Jobb imorgon", TODAY, Some("en-GB")).due_date, None);
    assert_eq!(pl("Jobb imorgon", TODAY, None).due_date, None);
}

#[test]
fn tags_option_turns_tag_parsing_off() {
    let o = ParseOptions {
        locale: Some("sv".into()),
        tags: false,
    };
    let q = parse_quick_add("Möte på fredag #jobb", date(TODAY), &o);
    assert!(q.tags.is_empty());
    assert_eq!(due(&q), some("2026-01-09"));
    assert_eq!(q.subject, "Möte #jobb");
}

#[test]
fn keywords_are_case_insensitive() {
    for (text, want) in [
        ("Jobb I MORGON", "2026-01-08"),
        ("Jobb I ÖVERMORGON", "2026-01-09"),
        ("Jobb IDAG", TODAY),
        ("Möte PÅ FREDAG", "2026-01-09"),
        ("Möte PÅ SÖNDAGEN", "2026-01-11"),
        ("Möte NÄSTA VECKA", "2026-01-12"),
        ("Möte NÄSTA MÅNAD", "2026-02-07"),
        ("Möte NÄSTA MÅNDAG", "2026-01-12"),
        ("Lunch I VECKAN", "2026-01-09"),
        ("Jobb OM 3 DAGAR", "2026-01-10"),
        ("Jobb OM 2 VECKOR", "2026-01-21"),
        ("Jobb OM 3 MÅNADER", "2026-04-07"),
    ] {
        assert_eq!(d(text), some(want), "{text}");
    }
    assert_eq!(r("Städa VARJE DAG"), fixed(RecurrenceType::Daily));
    assert_eq!(r("Städa VARJE VECKA"), fixed(RecurrenceType::Weekly));
    assert_eq!(r("Städa VARJE MÅNAD"), fixed(RecurrenceType::Monthly));
    assert_eq!(r("Städa VARJE ÅR"), fixed(RecurrenceType::Yearly));
    assert_eq!(r("Städa VARJE MÅNDAG"), fixed(RecurrenceType::Weekly));
    assert_eq!(r("Städa VARJE MÅNDAG OCH FREDAG"), weekly_on(&[1, 5]));
    assert_eq!(r("Städa VARANNAN DAG"), every(2, RecurrenceUnit::Day));
    assert_eq!(r("Städa VARANNAT ÅR"), every(2, RecurrenceUnit::Year));
    assert_eq!(r("Städa VAR TREDJE DAG"), every(3, RecurrenceUnit::Day));
    assert_eq!(t("Träna KL 18:30").as_deref(), Some("18:30"));
    assert_eq!(t("Ring KLOCKAN 9").as_deref(), Some("09:00"));
    assert_eq!(t("Ring VID 9").as_deref(), Some("09:00"));
    assert_eq!(
        p("Möte PÅ FREDAG #JOBB !HÖG").importance,
        ImportanceLevel::High
    );
    assert_eq!(t("x AT 7:45").as_deref(), Some("07:45"));
    assert_eq!(en("x AT 7:45").time.as_deref(), Some("07:45"));
    assert_eq!(en("x AT 7").time.as_deref(), Some("07:00"));
    assert_eq!(
        en("x EVERY FRIDAY").recurrence,
        fixed(RecurrenceType::Weekly)
    );
    assert_eq!(en("x EVERY DAY").recurrence, fixed(RecurrenceType::Daily));
    assert_eq!(en("x EVERY MON AND FRI").recurrence, weekly_on(&[1, 5]));
    assert_eq!(due(&en("x NEXT WEEK")), some("2026-01-12"));
    assert_eq!(due(&en("x IN 3 DAYS")), some("2026-01-10"));
    assert_eq!(due(&en("x ON FRI")), some("2026-01-09"));
}

#[test]
fn extra_whitespace_inside_phrases() {
    for (text, want) in [
        ("Möte  på  fredag", "2026-01-09"),
        ("Jobb  imorgon", "2026-01-08"),
        ("Jobb nästa  vecka", "2026-01-12"),
        ("Jobb nästa  månad", "2026-02-07"),
        ("Lunch i  veckan", "2026-01-09"),
        ("Jobb om  3  dagar", "2026-01-10"),
        ("Möte 15/3  2027", "2027-03-15"),
    ] {
        assert_eq!(d(text), some(want), "{text}");
    }
    assert_eq!(t("Träna kl  18").as_deref(), Some("18:00"));
    assert_eq!(t("Träna kl  18:30").as_deref(), Some("18:30"));
    assert_eq!(r("Städa varje  måndag  och  fredag"), weekly_on(&[1, 5]));
    assert_eq!(r("Städa varje  dag"), fixed(RecurrenceType::Daily));
    assert_eq!(r("Städa varannan  vecka"), every(2, RecurrenceUnit::Week));
    assert_eq!(r("Städa var  tredje  dag"), every(3, RecurrenceUnit::Day));
    assert_eq!(en("x at  7:45").time.as_deref(), Some("07:45"));
    assert_eq!(en("x at  7").time.as_deref(), Some("07:00"));
    assert_eq!(due(&en("x next  week")), some("2026-01-12"));
    assert_eq!(due(&en("x next  month")), some("2026-02-07"));
    assert_eq!(due(&en("x in  3  days")), some("2026-01-10"));
    assert_eq!(en("x every  2 days").recurrence.unwrap().interval, Some(2));
    assert_eq!(
        en("x every  2 days").recurrence,
        every(2, RecurrenceUnit::Day)
    );
    assert_eq!(en("x every  day").recurrence, fixed(RecurrenceType::Daily));
    assert_eq!(
        en("x every  12 days").recurrence,
        every(12, RecurrenceUnit::Day)
    );
    assert_eq!(
        en("x every  daily").recurrence,
        fixed(RecurrenceType::Daily)
    );
    assert_eq!(en("x 9am").time.as_deref(), Some("09:00"));
    assert_eq!(en("x at  9am").time.as_deref(), Some("09:00"));
}

#[test]
fn rejected_values_stay_in_the_subject() {
    assert_eq!(en("x at 13pm").time, None);
    assert_eq!(en("x at 13pm").subject, "x at 13pm");
    assert_eq!(en("x at 0am").time, None);
    assert_eq!(en("x at 0am").subject, "x at 0am");
    assert_eq!(p("Träna kl 99").subject, "Träna kl 99");
    assert_eq!(p("Möte 99/9").subject, "Möte 99/9");
}

#[test]
fn chips_are_locale_keys_rendered_per_language() {
    let q = p("Betala hyran imorgon kl 18:30 #hem !hög");
    assert_eq!(
        chips(&q, "sv"),
        s(&[
            "viktighet: hög",
            "tagg: hem",
            "förfaller: 2026-01-08",
            "kl: 18:30"
        ])
    );
    assert_eq!(
        chips(&q, "en"),
        s(&[
            "importance: high",
            "tag: hem",
            "due: 2026-01-08",
            "at: 18:30"
        ])
    );
}

#[test]
fn varje_first_sets_day_of_month() {
    let q = p("Städa varje 1:a");
    assert_eq!(q.recurrence, fixed(RecurrenceType::Monthly));
    assert_eq!(due(&q), some("2026-02-01"));
    assert_eq!(q.subject, "Städa");
    assert_eq!(r("Städa varje 3:e"), fixed(RecurrenceType::Monthly));
    assert_eq!(
        due(&pl("Städa varje 15.", TODAY, Some("sv"))),
        some("2026-01-15")
    );
    assert_eq!(
        due(&pl("Städa varje 1:", TODAY, Some("sv"))),
        some("2026-02-01")
    );
    assert_eq!(r("Städa varje 31:e"), fixed(RecurrenceType::Monthly));
    assert_eq!(r("Städa varje 32:e"), None);
    assert_eq!(r("Städa var 3:e dag"), every(3, RecurrenceUnit::Day));
    assert_eq!(r("Städa var 2:a vecka"), every(2, RecurrenceUnit::Week));
    assert_eq!(r("Städa var 10:e månad"), every(10, RecurrenceUnit::Month));
    assert_eq!(p("Städa var 3:e dag").subject, "Städa");
    assert_eq!(
        p("Städa varje 1:a")
            .chips
            .iter()
            .filter(|c| c.key == "chip.repeat")
            .count(),
        1
    );
}

#[test]
fn bare_hhmm_sets_the_time() {
    assert_eq!(t("Träna 15:30").as_deref(), Some("15:30"));
    assert_eq!(t("Träna 8:05").as_deref(), Some("08:05"));
    assert_eq!(t("Träna 00:00").as_deref(), Some("00:00"));
    assert_eq!(t("Träna 24:00"), None);
    assert_eq!(t("Träna 15:60"), None);
    assert_eq!(p("Träna 15:60").subject, "Träna 15:60");
    let q = p("Träna imorgon 15:30");
    assert_eq!(due(&q), some("2026-01-08"));
    assert_eq!(q.reminder_at, some("2026-01-08T15:30"));
    assert_eq!(q.subject, "Träna");
    assert_eq!(t("Träna 18"), None);
    assert_eq!(en("Träna 15:30").time, None);
}

#[test]
fn recurrence_days_follow_the_locale_week_order() {
    let q = p("Städa varje torsdag och söndag");
    assert_eq!(q.recurrence, weekly_on(&[0, 4]));
    let rule = |q: &ParsedQuickAdd| {
        q.chips
            .iter()
            .find(|c| c.key == "chip.repeat")
            .unwrap()
            .vars["rule"]
            .clone()
    };
    assert_eq!(rule(&q), "varje 1 vecka på tors,sön");
    assert_eq!(
        rule(&en("Clean every thursday and sunday")),
        "every 1 week on sun,thu"
    );
}

// --- nlp-suggest.test.js ----------------------------------------------------

fn nm(word: &str, suggestion: &str) -> NearMiss {
    NearMiss {
        word: word.into(),
        suggestion: suggestion.into(),
    }
}

#[test]
fn edit_distance_cases() {
    assert_eq!(edit_distance("imorgon", "imorgon"), 0);
    assert_eq!(edit_distance("", ""), 0);
    assert_eq!(edit_distance("imorgn", "imorgon"), 1);
    assert_eq!(edit_distance("tomorow", "tomorrow"), 1);
    assert_eq!(edit_distance("mondag", "monday"), 1);
    assert_eq!(edit_distance("veckan", "vecka"), 1);
    assert_eq!(edit_distance("tomorw", "tomorrow"), 2);
    assert_eq!(edit_distance("", "day"), 3);
    assert_eq!(edit_distance("year", ""), 4);
    assert_eq!(
        edit_distance("fredag", "fredga"),
        edit_distance("fredga", "fredag")
    );
    assert_eq!(
        edit_distance("imorgon", "imorgn"),
        edit_distance("imorgn", "imorgon")
    );
}

#[test]
fn sv_near_misses() {
    let q = p("Jobb imorgn");
    assert_eq!(q.suggestions, vec![nm("imorgn", "imorgon")]);
    assert_eq!(q.due_date, None);
    assert_eq!(q.subject, "Jobb imorgn");
    let q = p("Möte på mandag");
    assert_eq!(q.suggestions, vec![nm("mandag", "måndag")]);
    assert_eq!(q.due_date, None);
    assert_eq!(q.subject, "Möte på mandag");
    assert_eq!(d("Möte på måndag"), some("2026-01-12"));
    assert!(p("Möte på måndag").suggestions.is_empty());
    assert_eq!(
        p("Möte på fredga").suggestions,
        vec![nm("fredga", "fredag")]
    );
}

#[test]
fn sv_no_suggestions() {
    for text in [
        "Städa varannan",
        "Jobb imorgon",
        "Möte på fredag",
        "Jobb i veckan",
        "Träna kl 18",
        "Jobb zzzq",
        "Jobb xyzabc",
        "Köp kaffe",
        "Fixa fönstret",
        "Skicka fakturan till leverantören",
    ] {
        assert!(p(text).suggestions.is_empty(), "{text}");
    }
}

#[test]
fn english_near_misses_by_locale() {
    let q = en("Pay rent tomorow");
    assert_eq!(q.suggestions, vec![nm("tomorow", "tomorrow")]);
    assert_eq!(q.due_date, None);
    assert_eq!(q.subject, "Pay rent tomorow");
    assert!(en("Pay rent tmrw").suggestions.is_empty());
    assert!(en("Buy milk xyz").suggestions.is_empty());
    for text in ["Betala hyran imorgn", "Möte på tisdag", "Jobb i veckan"] {
        assert!(en(text).suggestions.is_empty(), "{text}");
    }
    assert_eq!(
        p("Betala hyran tomorow").suggestions,
        vec![nm("tomorow", "tomorrow")]
    );
}

#[test]
fn near_miss_ranges_and_replace() {
    let imorgn = [nm("imorgn", "imorgon")];
    let rs = find_near_miss_ranges("Jobb imorgn", &imorgn);
    assert_eq!(
        (
            rs[0].start,
            rs[0].end,
            rs[0].word.as_str(),
            rs[0].suggestion.as_str()
        ),
        (5, 11, "imorgn", "imorgon")
    );
    assert_eq!(rs.len(), 1);
    let rs = find_near_miss_ranges("Jobb IMORGN", &imorgn);
    assert_eq!((rs.len(), rs[0].start, rs[0].end), (1, 5, 11));
    assert!(find_near_miss_ranges("ximorgnx", &imorgn).is_empty());
    assert_eq!(find_near_miss_ranges("imorgn", &imorgn).len(), 1);
    let two = [nm("imorgn", "imorgon"), nm("imorgn", "imorgon")];
    let rs = find_near_miss_ranges("imorgn och imorgn", &two);
    assert_eq!((rs.len(), rs[0].start, rs[1].start), (2, 0, 11));
    // Adjacent duplicates must not swallow each other's separator.
    assert_eq!(find_near_miss_ranges("imorgn imorgn", &two).len(), 2);
    assert!(find_near_miss_ranges("Jobb imorgn", &[]).is_empty());
    assert_eq!(
        replace_range("Jobb imorgn", 5, 11, "imorgon"),
        "Jobb imorgon"
    );
    assert_eq!(replace_range("abc", 1, 2, "X"), "aXc");
    assert_eq!(replace_range("abc", 0, 3, ""), "");
}

// --- Divergences D8-D10 -----------------------------------------------------

#[test]
fn d8_ordinal_unit_counts_are_not_capped_and_one_collapses() {
    // Gör't: "var 40:e dag" is rejected (31 cap meant for day-of-month);
    // "var 1:a dag" yields {custom, interval 1} which differs from "varje dag".
    assert_eq!(r("Städa var 40:e dag"), every(40, RecurrenceUnit::Day));
    assert_eq!(r("Städa var 1:a dag"), fixed(RecurrenceType::Daily));
    assert_eq!(r("Städa var 0:e dag"), None);
}

#[test]
fn d10_absurd_counts_stay_in_the_subject() {
    // Gör't: `om 99999999999 dagar` makes an Invalid Date and throws.
    let q = p("Jobb om 99999999999 dagar");
    assert_eq!(q.due_date, None);
    assert_eq!(q.subject, "Jobb om 99999999999 dagar");
    assert_eq!(en("x in 99999999999 days").due_date, None);
    assert_eq!(en("x every 99999999999 days").recurrence, None);
}

#[test]
fn d9_years_below_100_are_literal() {
    // Gör't: Date.UTC maps year 50 to 1950, so "15/3 0050" is 1950-03-15.
    assert_eq!(d("Möte 15/3 0050"), some("0050-03-15"));
}
