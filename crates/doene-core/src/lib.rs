//! Döne core: pure domain logic and entity types.
//!
//! No I/O, no UI dependencies (docs/plan.md §3). The public API is
//! deliberately small: entities, read-model builders, and pure rule functions.

pub mod dates;
pub mod dnd;
pub mod entities;
pub mod errors;
pub mod i18n;
pub mod labels;
pub mod recurrence;
pub mod tree;
pub mod view_rules;

pub use dates::{Date, InvalidDate};
pub use dnd::{DropOutcome, DropZone, GroupRect, TASK_EDGE};
pub use entities::{
    HydratedTask, Importance, MatchMode, RecurrenceRule, RecurrenceType, RecurrenceUnit, Step,
    Tag, Task, TaskStatus, TrashEntry, TrashKind, View, Weekday,
};
pub use errors::CodedError;
pub use recurrence::{RawRecurrence, TYPES, next_date, next_on_or_after, validate};
pub use tree::ParentMap;
pub use view_rules::{
    SORT_KEYS, RawSortLevel, RawView, SortDirection, SortKey, SortLevel, ViewRules,
    normalise_view, sort_tasks, view_matches,
};
