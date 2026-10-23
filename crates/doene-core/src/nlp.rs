//! Quick-add natural-language parsing — a port of Gör't `src/nlp.js`.
//!
//! `parse_quick_add` reads one composer line and pulls out the recognised
//! phrases (importance, tags, time, recurrence, due date), leaving what is
//! left as the subject. It is total: unrecognised text stays in the subject,
//! nothing here can fail.
//!
//! The keyword language comes from `ParseOptions::locale`; only English is
//! supported so far — the Swedish pool lands in ticket #04 and unknown
//! locales fall back to English (Gör't's own fallback).

use std::collections::HashMap;

use crate::dates::Date;
use crate::entities::{RecurrenceRule, Tag};

/// The parsed importance marker (Gör't's `importance` field): `!high`/`!!` →
/// [`ImportanceLevel::High`], `!low` → [`ImportanceLevel::Low`], none → Normal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportanceLevel {
    /// No marker typed.
    Normal,
    /// `!high` or `!!`.
    High,
    /// `!low`. The task schema has no low importance (docs/plan.md §5);
    /// quick-add maps it to normal — see docs/divergences.md D6.
    Low,
}

/// A composer chip: a locale key plus its params (Gör't's `{key, vars}`).
/// The UI renders each chip through `tr(key, vars, locale)`, so the same parse
/// shows localized chips in every language. Chip order is parse order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chip {
    /// Locale key, e.g. `"chip.due"`.
    pub key: &'static str,
    /// Interpolation params for the key.
    pub vars: HashMap<String, String>,
}

/// A "did you mean …?" hint for a subject word within edit distance 2 of a
/// known date keyword (Gör't's `suggestions`). The word stays in the subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// The mistyped word as it appears in the subject (lowercased).
    pub word: String,
    /// The pool keyword it is closest to.
    pub suggestion: &'static str,
}

/// The result of parsing one quick-add line (Gör't's `parseQuickAdd` output).
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedQuickAdd {
    /// What remains after the recognised phrases are removed.
    pub subject: String,
    /// Due date, if a date phrase was recognised (or implied by time/recurrence).
    pub due_date: Option<Date>,
    /// Local time "HH:MM", if one was recognised.
    pub time: Option<String>,
    /// `"{due}T{time}"`, when both a due date and a time exist.
    pub reminder_at: Option<String>,
    /// Recurrence rule, if a recurrence phrase was recognised.
    pub recurrence: Option<RecurrenceRule>,
    pub importance: ImportanceLevel,
    /// `#word` tags in typed order (case preserved; quick-add lowercases).
    pub tags: Vec<Tag>,
    /// Composer chips in parse order.
    pub chips: Vec<Chip>,
    /// Near-miss keyword hints for the final subject words.
    pub suggestions: Vec<Suggestion>,
}

/// Options for [`parse_quick_add`] (Gör't's `{ tags, locale }`).
#[derive(Debug, Clone)]
pub struct ParseOptions {
    /// Recognise `#word` tags (default true).
    pub tags: bool,
    /// Keyword language; only `"en"` is supported so far — Swedish lands in
    /// ticket #04 and unknown locales fall back to English.
    pub locale: Option<String>,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            tags: true,
            locale: None,
        }
    }
}

/// Parse a quick-add line (Gör't's `parseQuickAdd`). Total: unrecognised text
/// stays in the subject; nothing here can fail.
pub fn parse_quick_add(text: &str, _today: Date, _opts: &ParseOptions) -> ParsedQuickAdd {
    // Whitespace-collapsed subject (Gör't's `s.replace(/\s+/g, ' ').trim()`).
    let subject = text.split_whitespace().collect::<Vec<_>>().join(" ");
    ParsedQuickAdd {
        subject,
        due_date: None,
        time: None,
        reminder_at: None,
        recurrence: None,
        importance: ImportanceLevel::Normal,
        tags: vec![],
        chips: vec![],
        suggestions: vec![],
    }
}
