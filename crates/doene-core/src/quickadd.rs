//! Quick add — a port of Gör't's `src/quickadd.js`: turns raw composer text
//! plus the current view into the final task fields. View-derived defaults
//! (my-day/shortlisted focus, important -> high, planned -> due today, a view
//! tags the task) live here and nowhere else.

use std::collections::HashSet;

use crate::dates::Date;
use crate::entities::{MatchMode, RecurrenceRule, Tag, View};
use crate::errors::CodedError;
use crate::nlp::{ImportanceLevel, ParseOptions, parse_quick_add};

/// The view the composer is open in (Gör't's `{ kind, id }`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickAddView {
    /// A built-in view: `my-day`, `shortlisted`, `important`, `planned`, `all`, …
    Smart(String),
    /// A user-made view, by id.
    Custom(String),
}

/// Options for [`quick_add`].
#[derive(Debug, Clone)]
pub struct QuickAddOptions {
    pub today: Date,
    /// The user's saved views, searched for a `Custom` view's tags.
    pub views: Vec<View>,
    /// Literal-escape: keep the text as the subject, parse nothing.
    pub literal: bool,
    /// Locale for the parser (None = English).
    pub locale: Option<String>,
}

/// The final task fields (Gör't's `{ body }`).
#[derive(Debug, Clone, PartialEq)]
pub struct TaskDraft {
    pub subject: String,
    pub shortlisted: bool,
    pub importance: ImportanceLevel,
    pub due_date: Option<Date>,
    pub reminder_at: Option<String>,
    pub recurrence: Option<RecurrenceRule>,
    /// Lowercased, deduplicated; view tags first.
    pub tags: Vec<Tag>,
}

/// Build a task draft from raw text and the current view.
///
/// # Errors
/// `error.textRequired` when the trimmed text is empty.
pub fn quick_add(
    raw: &str,
    view: Option<&QuickAddView>,
    opts: &QuickAddOptions,
) -> Result<TaskDraft, CodedError> {
    let v = raw.trim();
    if v.is_empty() {
        return Err(CodedError::new("error.textRequired", "text required"));
    }
    let parse_opts = ParseOptions {
        locale: opts.locale.clone(),
        tags: true,
    };
    let parsed = if opts.literal {
        None
    } else {
        Some(parse_quick_add(v, opts.today, &parse_opts))
    };

    // Inside a view with tags a new task is tagged to stay in it: every tag
    // for "all", the first for "any".
    let inherited: Vec<Tag> = match view {
        Some(QuickAddView::Custom(id)) => opts
            .views
            .iter()
            .find(|x| &x.id == id)
            .map(|open| match open.match_mode {
                MatchMode::All => open.tags.clone(),
                MatchMode::Any => open.tags.iter().take(1).cloned().collect(),
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    let smart = match view {
        Some(QuickAddView::Smart(id)) => Some(id.as_str()),
        _ => None,
    };

    let (subject, importance, due_date, reminder_at, recurrence, typed_tags) = match parsed {
        Some(p) => (
            p.subject,
            p.importance,
            p.due_date,
            p.reminder_at,
            p.recurrence,
            p.tags,
        ),
        None => (
            String::new(),
            ImportanceLevel::Normal,
            None,
            None,
            None,
            Vec::new(),
        ),
    };

    let mut seen = HashSet::new();
    let tags = inherited
        .iter()
        .chain(typed_tags.iter())
        .filter_map(|t| Tag::new(&t.as_str().to_lowercase()).ok())
        .filter(|t| seen.insert(t.as_str().to_owned()))
        .collect();

    Ok(TaskDraft {
        // Nothing left after parsing (e.g. just "weekdays"): keep the typed text.
        subject: if subject.is_empty() {
            v.to_owned()
        } else {
            subject
        },
        shortlisted: matches!(smart, Some("my-day" | "shortlisted")),
        importance: if importance != ImportanceLevel::Normal {
            importance
        } else if smart == Some("important") {
            ImportanceLevel::High
        } else {
            ImportanceLevel::Normal
        },
        due_date: due_date.or(if smart == Some("planned") {
            Some(opts.today)
        } else {
            None
        }),
        reminder_at,
        recurrence,
        tags,
    })
}
