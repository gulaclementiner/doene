//! English quick-add NLP parser — a faithful port of Gör't's `src/nlp.js`
//! (ticket #03).
//!
//! The reference parses by repeatedly matching regexes against a space-padded
//! copy of the input and replacing each matched phrase with a single space;
//! what remains is the subject. Döne ports that algorithm 1:1 for English so
//! the Gör't fixture suites (`test/nlp-search.test.js`,
//! `test/nlp-dates-recurrence-mutants.test.js`,
//! `test/nlp-keywords-mutants.test.js`) pin identical behaviour. Swedish
//! patterns land in ticket #04; until then every locale parses as English.
//!
//! One mechanical translation: Gör't's lookaheads `(?=\s)` become a consumed
//! trailing `\s`. The replaced span is therefore one space wider, but the
//! surviving string collapses to the same subject and no pattern depends on
//! double spaces, so all fixture behaviour is preserved.
//!
//! Divergences: impossible ISO dates stay in the subject (D6).

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;

use crate::dates::Date;
use crate::entities::{RecurrenceRule, RecurrenceType, RecurrenceUnit, Tag, Weekday};
use crate::i18n;
use crate::labels;

/// Options for [`parse_quick_add`].
#[derive(Debug, Clone)]
pub struct ParseOptions {
    /// BCP-47 tag; only the base language is consulted. Swedish lands in
    /// ticket #04 — until then every locale parses as English.
    pub locale: Option<String>,
    /// Parse `#tag` words (Gör't's `{ tags = true }`).
    pub tags: bool,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            locale: None,
            tags: true,
        }
    }
}

/// Importance levels understood by the parser (Gör't's `'normal'|'high'|'low'`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportanceLevel {
    Normal,
    High,
    Low,
}

/// A structured chip describing one parsed field (Gör't's `{ key, vars }`),
/// rendered through the locale tables by the UI.
#[derive(Debug, Clone, PartialEq)]
pub struct Chip {
    pub key: String,
    pub vars: HashMap<String, String>,
}

/// The structured result of parsing one quick-add line.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedQuickAdd {
    /// Subject with all parsed phrases removed and whitespace collapsed.
    pub subject: String,
    /// Due date if a date phrase was recognised.
    pub due_date: Option<Date>,
    /// Time of day `HH:MM` if one was recognised.
    pub time: Option<String>,
    /// `YYYY-MM-DDTHH:MM` reminder when a time was recognised (the due date
    /// defaults to today).
    pub reminder_at: Option<String>,
    /// Recurrence rule if a recurrence phrase was recognised.
    pub recurrence: Option<RecurrenceRule>,
    /// Importance marker (`!high`, `!low`, bare `!`).
    pub importance: ImportanceLevel,
    /// `#tag` words, lowercased by the caller of quick-add.
    pub tags: Vec<Tag>,
    /// Chips in parse order (importance, due, repeat, at).
    pub chips: Vec<Chip>,
    /// "Did you mean …?" hints for subject words within edit distance 2 of a
    /// date keyword (the word itself stays in the subject).
    pub suggestions: Vec<NearMiss>,
}

/// One near-miss hint (Gör't's `{ word, suggestion }`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NearMiss {
    /// The lowercased subject word.
    pub word: String,
    /// The keyword it probably meant.
    pub suggestion: String,
}

/// Date words a user might mistype (Gör't's `EN_KEYWORDS`). Short words are
/// left out: within distance 2 almost every short word would light up.
const EN_KEYWORDS: [&str; 23] = [
    "today",
    "tonight",
    "tomorrow",
    "tmrw",
    "yesterday",
    "noon",
    "midnight",
    "daily",
    "weekly",
    "monthly",
    "yearly",
    "annually",
    "weekdays",
    "sunday",
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "week",
    "month",
    "year",
];

/// Gör't's `nearMisses` for English.
fn near_misses(subject: &str) -> Vec<NearMiss> {
    let mut out = Vec::new();
    for raw in subject.split_whitespace() {
        let w = raw.to_lowercase();
        if w.chars().count() < 4
            || !w.chars().all(char::is_alphabetic)
            || EN_KEYWORDS.contains(&w.as_str())
        {
            continue;
        }
        let mut best: Option<(usize, &str)> = None;
        for k in EN_KEYWORDS {
            let d = edit_distance(&w, k);
            if d <= 2 && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, k));
            }
        }
        if let Some((_, k)) = best {
            out.push(NearMiss {
                word: w,
                suggestion: k.to_owned(),
            });
        }
    }
    out
}

/// Levenshtein distance over chars (Gör't's `editDistance`).
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i];
        for j in 1..=b.len() {
            let sub = prev[j - 1] + usize::from(a[i - 1] != b[j - 1]);
            cur.push((prev[j] + 1).min(cur[j - 1] + 1).min(sub));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Parse a quick-add line against `today`, mirroring Gör't's `parseQuickAdd`.
pub fn parse_quick_add(text: &str, today: Date, opts: &ParseOptions) -> ParsedQuickAdd {
    let re = en_re();
    // Gör't pads the input with spaces so every phrase is whitespace-bounded.
    let mut s = format!(" {text} ");
    let mut out = ParsedQuickAdd {
        subject: String::new(),
        due_date: None,
        time: None,
        reminder_at: None,
        recurrence: None,
        importance: ImportanceLevel::Normal,
        tags: Vec::new(),
        chips: Vec::new(),
        suggestions: Vec::new(),
    };

    // 1. Importance marker: `!high`, `!low`, or the double-bang `!!` (Gör't's
    //    bare high). A single `!` does not match and stays in the subject.
    if take(&re.imp, &mut s, |m| {
        out.importance = match m[1].to_lowercase().as_str() {
            "low" => ImportanceLevel::Low,
            _ => ImportanceLevel::High, // 'high' or the bare '!'
        };
        true
    }) {
        let level = match out.importance {
            ImportanceLevel::High => "importance.high",
            ImportanceLevel::Low => "importance.low",
            ImportanceLevel::Normal => unreachable!(),
        };
        out.chips.push(chip(
            "chip.importance",
            [("level".to_owned(), level.to_owned())],
        ));
    }

    // 2. Tags: every `#word` (Gör't loops until none remain). Words stay raw
    //    here; quick-add lowercases and dedupes them.
    if opts.tags {
        while take(&re.tag, &mut s, |m| {
            // The tag character class guarantees a non-empty word without spaces.
            out.tags
                .push(Tag::new(&m[1]).expect("tag pattern yields a valid Tag"));
            out.chips
                .push(chip("chip.tag", [("tag".to_owned(), m[1].to_owned())]));
            true
        }) {}
    }

    // 3. Time: `at H(am|pm)`, `H:MM(am|pm)` (optional 'at'), `at H:MM`,
    //    `at H`, `noon`/`midnight`. Out-of-range values are left in place.
    if !take(&re.ampm, &mut s, |m| {
        let h: i64 = m[1].parse().expect("digits");
        if h > 12 || h < 1 {
            return false;
        }
        let min = m.get(2).map(|c| c.as_str()).unwrap_or("00").to_owned();
        let hour = (h % 12)
            + if m[3].eq_ignore_ascii_case("pm") {
                12
            } else {
                0
            };
        out.time = Some(format!("{hour:02}:{min}"));
        true
    }) && !take(&re.hhmm, &mut s, |m| {
        let h: i64 = m[1].parse().expect("digits");
        let min: i64 = m[2].parse().expect("digits");
        if h > 23 || min > 59 {
            return false;
        }
        out.time = Some(format!("{h:02}:{min:02}"));
        true
    }) && !take(&re.hh, &mut s, |m| {
        let h: i64 = m[1].parse().expect("digits");
        if h > 23 {
            return false;
        }
        out.time = Some(format!("{h:02}:00"));
        true
    }) && !take(&re.noonmid, &mut s, |m| {
        out.time = Some(
            if m[1].eq_ignore_ascii_case("noon") {
                "12:00"
            } else {
                "00:00"
            }
            .to_owned(),
        );
        true
    }) {}

    // 4. Recurrence phrases (Gör't parses these before due dates). The first
    //    occurrence is remembered and applied after the due-date pass.
    let mut rec_first: Option<RecFirst> = None;
    if !take(&re.every_n_unit, &mut s, |m| {
        let n: u32 = m[1].parse().expect("digits");
        let unit = match m[2].to_lowercase().as_str() {
            "day" => RecurrenceUnit::Day,
            "week" => RecurrenceUnit::Week,
            "month" => RecurrenceUnit::Month,
            _ => RecurrenceUnit::Year, // 'year'
        };
        out.recurrence = Some(if n == 1 {
            fixed_kind(unit_kind(unit))
        } else {
            RecurrenceRule {
                kind: RecurrenceType::Custom,
                interval: Some(n),
                unit: Some(unit),
                days: None,
            }
        });
        true
    }) && !take(&re.bare_kw, &mut s, |m| {
        // Gör't's guard: the 'every' prefix admits 'weekday', bare words must
        // be one of the six fixed keywords.
        let every = m[0].trim().to_lowercase().starts_with("every");
        let k = m[1].to_lowercase();
        if !every
            && !matches!(
                k.as_str(),
                "daily" | "weekdays" | "weekly" | "monthly" | "yearly" | "annually"
            )
        {
            return false;
        }
        out.recurrence = Some(match k.as_str() {
            _ if k.starts_with("weekday") => fixed_kind(RecurrenceType::Weekdays),
            "daily" => fixed_kind(RecurrenceType::Daily),
            "weekly" => fixed_kind(RecurrenceType::Weekly),
            "monthly" => fixed_kind(RecurrenceType::Monthly),
            _ => fixed_kind(RecurrenceType::Yearly), // 'yearly' | 'annually'
        });
        true
    }) && !take(&re.every_unit, &mut s, |m| {
        out.recurrence = Some(match m[1].to_lowercase().as_str() {
            "day" => fixed_kind(RecurrenceType::Daily),
            "week" => fixed_kind(RecurrenceType::Weekly),
            "month" => fixed_kind(RecurrenceType::Monthly),
            "year" => fixed_kind(RecurrenceType::Yearly),
            _ => fixed_kind(RecurrenceType::Weekdays), // 'weekday'
        });
        true
    }) && !take(&re.every_weekday_list, &mut s, |m| {
        let days = every_weekday_days(&m[1]);
        out.recurrence = Some(if days.len() == 1 {
            fixed_kind(RecurrenceType::Weekly)
        } else {
            RecurrenceRule {
                kind: RecurrenceType::Custom,
                interval: Some(1),
                unit: Some(RecurrenceUnit::Week),
                days: Some(
                    days.iter()
                        .map(|d| Weekday::from_dow(*d).expect("valid dow"))
                        .collect(),
                ),
            }
        });
        rec_first = Some(RecFirst::Weekdays(days));
        true
    }) && !take(&re.every_nth, &mut s, |m| {
        let n: u32 = m[1].parse().expect("digits");
        if !(1..=31).contains(&n) {
            return false;
        }
        out.recurrence = Some(fixed_kind(RecurrenceType::Monthly));
        rec_first = Some(RecFirst::Nth(n));
        true
    }) {}

    // 5. Due date: day words, `in N unit`, `next week/month`, weekday names,
    //    explicit ISO dates.
    if !take(&re.day_words, &mut s, |m| {
        let k = m[1].to_lowercase();
        set_due(
            &mut out,
            match k.as_str() {
                "today" | "tonight" => today,
                "tomorrow" | "tmrw" => today.add_days(1),
                _ => today.add_days(-1), // yesterday
            },
        );
        if k == "tonight" && out.time.is_none() {
            out.time = Some("20:00".to_owned());
        }
        true
    }) && !take(&re.in_n_unit, &mut s, |m| {
        let n: i64 = m[1].parse().expect("digits");
        set_due(
            &mut out,
            match m[2].to_lowercase().as_str() {
                "day" => today.add_days(n),
                "week" => today.add_days(7 * n),
                _ => today.add_months(n), // month
            },
        );
        true
    }) && !take(&re.next_week, &mut s, |_| {
        set_due(&mut out, next_weekday(today, 1));
        true
    }) && !take(&re.next_month, &mut s, |_| {
        set_due(&mut out, today.add_months(1));
        true
    }) && !take(&re.weekday, &mut s, |m| {
        set_due(&mut out, next_weekday(today, wd_index(&m[1])));
        true
    }) && !take(&re.iso, &mut s, |m| match Date::parse(&m[1]) {
        // Divergence D6: Gör't's JS Date.parse rolls impossible days over;
        // Döne rejects them (D1), so the token stays in the subject.
        Ok(date) => {
            set_due(&mut out, date);
            true
        }
        Err(_) => false,
    }) {}

    // 6. A recurrence without an explicit date is due at its first occurrence,
    //    labelled 'first occurrence' through the locale table.
    if out.recurrence.is_some() && out.due_date.is_none() {
        // No explicit date: the first occurrence is due, labelled through the
        // locale table. An explicit date wins over `rec_first`.
        let first = match &rec_first {
            Some(RecFirst::Weekdays(days)) => first_weekday_occurrence(today, days),
            Some(RecFirst::Nth(n)) => next_day_of_month(today, *n),
            None => Some(today),
        };
        if let Some(date) = first {
            set_due_labeled(
                &mut out,
                date,
                i18n::tr("recurrence.firstOccurrence", &HashMap::new(), Some("en")),
            );
        }
    }
    // The repeat chip is emitted for every parsed recurrence, explicit date or
    // not (Gör't emits it after the first-occurrence block).
    if let Some(rule) = &out.recurrence {
        out.chips.push(chip(
            "chip.repeat",
            [(
                "rule".to_owned(),
                labels::rec_label(rule, Some("en"), i18n::week_start(Some("en"))),
            )],
        ));
    }

    // 7. A time pins a reminder: the due date defaults to today.
    if let Some(time) = out.time.clone() {
        if out.due_date.is_none() {
            set_due(&mut out, today);
        }
        let due = out.due_date.expect("set above");
        out.reminder_at = Some(format!("{due}T{time}"));
        out.chips.push(chip("chip.at", [("time".to_owned(), time)]));
    }

    // 8. Subject: collapse all whitespace runs to single spaces (Gör't's
    //    `s.replace(/\s+/g, ' ').trim()`).
    out.subject = s.split_whitespace().collect::<Vec<_>>().join(" ");
    out.suggestions = near_misses(&out.subject);
    out
}

/// The English phrase patterns, compiled once. Each mirrors a Gör't regex with
/// the `(?=\s)` lookahead translated to a consumed trailing `\s`.
struct EnRe {
    imp: Regex,
    tag: Regex,
    ampm: Regex,
    hhmm: Regex,
    hh: Regex,
    noonmid: Regex,
    every_n_unit: Regex,
    bare_kw: Regex,
    every_unit: Regex,
    every_weekday_list: Regex,
    every_nth: Regex,
    day_words: Regex,
    in_n_unit: Regex,
    next_week: Regex,
    next_month: Regex,
    weekday: Regex,
    iso: Regex,
}

/// Gör't's `WDRE`: full or abbreviated English weekday names.
const WD_RE: &str = "(?:sun(?:day)?|mon(?:day)?|tue(?:s(?:day)?)?|wed(?:nesday)?|thu(?:rs(?:day)?)?|fri(?:day)?|sat(?:urday)?)";

fn en_re() -> &'static EnRe {
    static RE: OnceLock<EnRe> = OnceLock::new();
    RE.get_or_init(|| EnRe {
        imp: Regex::new(r"(?i)\s!(high|!|low)\s").expect("valid pattern"),
        tag: Regex::new(r"\s#([\p{L}\p{Nd}_-]+)\s").expect("valid pattern"),
        ampm: Regex::new(r"(?i)\s(?:at\s+)?([0-9]{1,2})(?::([0-9]{2}))?\s*(am|pm)\s")
            .expect("valid pattern"),
        hhmm: Regex::new(r"(?i)\sat\s+([0-9]{1,2}):([0-9]{2})\s").expect("valid pattern"),
        hh: Regex::new(r"(?i)\sat\s+([0-9]{1,2})\s").expect("valid pattern"),
        noonmid: Regex::new(r"(?i)\s(noon|midnight)\s").expect("valid pattern"),
        every_n_unit: Regex::new(r"(?i)\severy\s+([0-9]+)\s*(day|week|month|year)s?\s")
            .expect("valid pattern"),
        bare_kw: Regex::new(
            r"(?i)\s(?:every\s+)?(daily|weekdays?|weekly|monthly|yearly|annually)\s",
        )
        .expect("valid pattern"),
        every_unit: Regex::new(r"(?i)\severy\s+(day|week|month|year|weekday)\s")
            .expect("valid pattern"),
        every_weekday_list: Regex::new(&format!(
            r"(?i)\severy\s+({WD_RE}(?:\s*(?:,|and|&)\s*{WD_RE})*)\s"
        ))
        .expect("valid pattern"),
        every_nth: Regex::new(r"(?i)\severy\s+([0-9]{1,2})(?:st|nd|rd|th)\s")
            .expect("valid pattern"),
        day_words: Regex::new(r"(?i)\s(today|tonight|tomorrow|tmrw|yesterday)\s")
            .expect("valid pattern"),
        in_n_unit: Regex::new(r"(?i)\sin\s+([0-9]+)\s*(day|week|month)s?\s")
            .expect("valid pattern"),
        next_week: Regex::new(r"(?i)\snext\s+week\s").expect("valid pattern"),
        next_month: Regex::new(r"(?i)\snext\s+month\s").expect("valid pattern"),
        weekday: Regex::new(&format!(r"(?i)\s(?:on\s+|next\s+)?({WD_RE})\s"))
            .expect("valid pattern"),
        iso: Regex::new(r"\s([0-9]{4}-[0-9]{2}-[0-9]{2})\s").expect("valid pattern"),
    })
}

/// Gör't's `take`: run the regex, apply the handler (which may veto by
/// returning false), and replace the matched span with a single space.
fn take(re: &Regex, s: &mut String, f: impl FnOnce(&regex::Captures) -> bool) -> bool {
    let m = match re.captures(s) {
        Some(m) => m,
        None => return false,
    };
    if !f(&m) {
        return false;
    }
    let span = m.get(0).expect("group 0 always matches").range();
    s.replace_range(span, " ");
    true
}

/// Gör't's `set`: record the due date and its chip (the label defaults to
/// the ISO date; recurrence first-occurrence dates use a translated label).
fn set_due(out: &mut ParsedQuickAdd, date: Date) {
    out.due_date = Some(date);
    out.chips
        .push(chip("chip.due", [("date".to_owned(), date.to_string())]));
}

/// `set` with an explicit chip label (Gör't's `set(d, label)`).
fn set_due_labeled(out: &mut ParsedQuickAdd, date: Date, label: String) {
    out.due_date = Some(date);
    out.chips
        .push(chip("chip.due", [("date".to_owned(), label)]));
}

fn chip(key: &str, vars: impl IntoIterator<Item = (String, String)>) -> Chip {
    Chip {
        key: key.to_owned(),
        vars: vars.into_iter().collect(),
    }
}

/// Gör't's `wdIndex` for English: the first three letters pick the weekday.
fn wd_index(word: &str) -> u8 {
    let s = word.to_lowercase();
    labels::WD
        .iter()
        .position(|w| s.starts_with(w))
        .map(|i| i as u8)
        .expect("weekday pattern yields a known prefix")
}

/// Gör't's `nextWeekday`: the first occurrence of weekday `d` strictly after
/// `from` (Gör't: `(d - dow + 7) % 7 || 7`).
fn next_weekday(from: Date, d: u8) -> Date {
    let n = (i64::from(d) - i64::from(from.dow()) + 7) % 7;
    from.add_days(if n == 0 { 7 } else { n })
}

/// A fixed-kind recurrence rule (`daily`, `weekly`, …).
fn fixed_kind(kind: RecurrenceType) -> RecurrenceRule {
    RecurrenceRule {
        kind,
        interval: None,
        unit: None,
        days: None,
    }
}

/// Gör't's `UNIT_TYPE`: the fixed kind a unit collapses to at interval 1.
fn unit_kind(unit: RecurrenceUnit) -> RecurrenceType {
    match unit {
        RecurrenceUnit::Day => RecurrenceType::Daily,
        RecurrenceUnit::Week => RecurrenceType::Weekly,
        RecurrenceUnit::Month => RecurrenceType::Monthly,
        RecurrenceUnit::Year => RecurrenceType::Yearly,
    }
}

/// The first-occurrence hook Gör't remembers while parsing a recurrence.
enum RecFirst {
    /// A weekday list (sorted, unique dow numbers 0 = Sunday … 6).
    Weekdays(Vec<u8>),
    /// An `every Nth` day-of-month rule.
    Nth(u32),
}

/// Gör't's `everyWeekday` split: dedupe + sort the weekday list (dow numbers).
/// Separators are `,`, `and`, `&` with optional surrounding whitespace
/// (Gör't's `/\s*(?:,|och|and|&)\s*/i`; `och` is Swedish and ticket #04).
fn every_weekday_days(list: &str) -> Vec<u8> {
    static SEP: OnceLock<Regex> = OnceLock::new();
    let sep = SEP.get_or_init(|| Regex::new(r"(?i)\s*(?:,|and|&)\s*").expect("valid pattern"));
    let mut days: Vec<u8> = sep
        .split(list)
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .map(wd_index)
        .collect();
    days.sort_unstable();
    days.dedup();
    days
}

/// Gör't's `recFirst` for weekday lists: the first listed day strictly after
/// `from` (single-day lists use [`next_weekday`]).
fn first_weekday_occurrence(from: Date, days: &[u8]) -> Option<Date> {
    let dow = from.dow();
    if days.len() == 1 {
        return Some(next_weekday(from, days[0]));
    }
    match days.iter().find(|&&d| d > dow) {
        Some(&d) => Some(from.add_days(i64::from(d - dow))),
        None => Some(from.add_days(7 - i64::from(dow) + i64::from(days[0]))),
    }
}

/// Gör't's `nextDayOfMonth`: the first occurrence of day `n` strictly after
/// `from`, clamped to shorter months (the 31st of a 30-day month lands on its
/// last day).
fn next_day_of_month(from: Date, n: u32) -> Option<Date> {
    for i in 0..14i64 {
        let total = from.year as i64 * 12 + i64::from(from.month) - 1 + i;
        let year = total.div_euclid(12);
        let month = (total.rem_euclid(12) + 1) as u8;
        let day = n.min(u32::from(Date::days_in_month(month, year as i32))) as u8;
        if let Ok(c) = Date::parse(&format!("{year:04}-{month:02}-{day:02}")) {
            if c > from {
                return Some(c);
            }
        }
    }
    None
}
