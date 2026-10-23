//! Döne domain entities (docs/plan.md §5 schema).
//!
//! Plain data: no I/O, no UI types. Field sets mirror the storage schema;
//! string conventions are UUIDv4 ids and ISO-8601 UTC timestamps.

use std::fmt;

use crate::dates::Date;

/// A `#word` on a task or view. Stored canonically without the leading `#`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Tag(String);

/// Returned when a string is not a valid tag word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidTag;

impl fmt::Display for InvalidTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not a valid tag (non-empty word without spaces)")
    }
}

impl std::error::Error for InvalidTag {}

impl Tag {
    /// Canonicalize a tag: strips one leading `#`, rejects empty words and
    /// anything containing whitespace.
    pub fn new(word: &str) -> Result<Self, InvalidTag> {
        let canonical = word.strip_prefix('#').unwrap_or(word);
        if canonical.is_empty() || canonical.chars().any(char::is_whitespace) {
            return Err(InvalidTag);
        }
        Ok(Self(canonical.to_owned()))
    }

    /// The tag word without the `#` marker.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Task lifecycle state (`tasks.status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Open,
    Completed,
}

/// Task importance (`tasks.importance`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Importance {
    Normal,
    High,
}

/// A task (`tasks` row).
#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    /// UUIDv4.
    pub id: String,
    pub subject: String,
    pub status: TaskStatus,
    pub importance: Importance,
    /// `YYYY-MM-DD`.
    pub due_date: Option<Date>,
    /// Local time, `'YYYY-MM-DDTHH:MM'` (no timezone).
    pub reminder_at: Option<String>,
    pub body: Option<String>,
    /// The single date a task was picked for; shortlisted while it is today.
    pub focus_date: Option<Date>,
    pub postponed_date: Option<Date>,
    /// ISO-8601 UTC.
    pub created_at: String,
    /// ISO-8601 UTC.
    pub completed_at: Option<String>,
    /// Manual order within its list; gap-free 1..n renumbering on move.
    pub position: f64,
    /// Id of the task this occurrence was spawned from (recurrence).
    pub spawned_from: Option<String>,
    /// Id of the parent task (subtask nesting); `None` for top-level tasks.
    pub parent_id: Option<String>,
    /// `None` = no recurrence.
    pub recurrence: Option<RecurrenceRule>,
}

/// A checklist step inside a task (`steps` row).
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// UUIDv4.
    pub id: String,
    /// The owning task's id.
    pub task_id: String,
    pub subject: String,
    pub status: TaskStatus,
    /// Manual order within the parent task; gap-free 1..n renumbering on move.
    pub position: f64,
}

/// A task hydrated with its child rows (`task_tags`, `steps`) — the shape
/// the pure read-model rules (views, search, smartlists) consume. Storage
/// builds it from DB rows; Gör't's equivalent is the store's `#hydrate` output.
#[derive(Debug, Clone, PartialEq)]
pub struct HydratedTask {
    pub task: Task,
    /// Tags from `task_tags`, in stored order.
    pub tags: Vec<Tag>,
    /// Steps from `steps`, in stored (position) order.
    pub steps: Vec<Step>,
}

impl HydratedTask {
    /// A task with no child rows.
    pub fn bare(task: Task) -> Self {
        Self {
            task,
            tags: vec![],
            steps: vec![],
        }
    }
}

/// How a view's tags combine (`views.match_mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMode {
    Any,
    All,
}

/// A saved filter the user makes (`views` row + `view_tags`).
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    /// UUIDv4.
    pub id: String,
    pub name: String,
    pub emoji: Option<String>,
    pub match_mode: MatchMode,
    /// Include-completed toggle.
    pub show_done: bool,
    /// Manual order in the rail; gap-free 1..n renumbering on move.
    pub position: f64,
    /// Tags from `view_tags`.
    pub tags: Vec<Tag>,
}

/// Recurrence kinds (`tasks.recurrence_type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecurrenceType {
    Daily,
    Weekdays,
    Weekly,
    Monthly,
    Yearly,
    Custom,
}

impl RecurrenceType {
    /// The Gör't token for this kind (`"daily"`, …).
    pub fn as_token(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekdays => "weekdays",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
            Self::Yearly => "yearly",
            Self::Custom => "custom",
        }
    }

    /// Parse a Gör't token; `None` for unknown kinds.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "daily" => Some(Self::Daily),
            "weekdays" => Some(Self::Weekdays),
            "weekly" => Some(Self::Weekly),
            "monthly" => Some(Self::Monthly),
            "yearly" => Some(Self::Yearly),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

/// Recurrence step unit (`tasks.recurrence_unit`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecurrenceUnit {
    Day,
    Week,
    Month,
    Year,
}

impl RecurrenceUnit {
    /// The Gör't token for this unit (`"day"`, …).
    pub fn as_token(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::Year => "year",
        }
    }

    /// Parse a Gör't token; `None` for unknown units.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "day" => Some(Self::Day),
            "week" => Some(Self::Week),
            "month" => Some(Self::Month),
            "year" => Some(Self::Year),
            _ => None,
        }
    }
}

/// A weekday (element of `tasks.recurrence_days`, custom rules only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weekday {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

impl Weekday {
    /// The Gör't dow number: 0 = Sunday … 6 = Saturday.
    pub fn to_dow(self) -> u8 {
        match self {
            Self::Sun => 0,
            Self::Mon => 1,
            Self::Tue => 2,
            Self::Wed => 3,
            Self::Thu => 4,
            Self::Fri => 5,
            Self::Sat => 6,
        }
    }

    /// From a Gör't dow number; `None` outside 0..=6.
    pub fn from_dow(dow: u8) -> Option<Self> {
        match dow {
            0 => Some(Self::Sun),
            1 => Some(Self::Mon),
            2 => Some(Self::Tue),
            3 => Some(Self::Wed),
            4 => Some(Self::Thu),
            5 => Some(Self::Fri),
            6 => Some(Self::Sat),
            _ => None,
        }
    }
}

/// A recurrence rule (normalized `tasks.recurrence_*` columns;
/// `None` on a task means no recurrence).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecurrenceRule {
    pub kind: RecurrenceType,
    /// Step size in `unit`s, e.g. 2 for "every 2 weeks"; custom rules only
    /// (always ≥ 1 after validation).
    pub interval: Option<u32>,
    /// Custom rules only.
    pub unit: Option<RecurrenceUnit>,
    /// Weekday set (sorted, unique); `Some` is always non-empty. Custom
    /// weekly rules only; `None` means "no weekday restriction".
    pub days: Option<Vec<Weekday>>,
}

/// What kind of entity a trash entry holds (`trash.kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrashKind {
    Task,
    View,
}

/// A trashed entity with its full snapshot (`trash` row).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashEntry {
    /// UUIDv4.
    pub id: String,
    pub kind: TrashKind,
    /// Display name of the trashed entity (task subject / view name).
    pub label: Option<String>,
    /// Full JSON snapshot incl. subtree/steps.
    pub data: String,
    /// ISO-8601 UTC.
    pub deleted_at: String,
}

#[cfg(test)]
mod tests {
    use super::Tag;

    #[test]
    fn new_accepts_plain_word() {
        let tag = Tag::new("errands").expect("valid tag");
        assert_eq!(tag.to_string(), "errands");
    }

    #[test]
    fn new_strips_leading_hash() {
        let tag = Tag::new("#errands").expect("valid tag");
        assert_eq!(tag.to_string(), "errands");
    }

    #[test]
    fn new_rejects_empty_and_whitespace() {
        for bad in ["", "#", " ", "a b"] {
            assert!(Tag::new(bad).is_err(), "expected {bad:?} to be rejected");
        }
    }
}
