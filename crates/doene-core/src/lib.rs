//! Döne core: pure domain logic and entity types.
//!
//! No I/O, no UI dependencies (docs/plan.md §3). The public API is
//! deliberately small: entities, read-model builders, and pure rule functions.

pub mod agenda;
pub mod agenda_order;
pub mod dates;
pub mod dnd;
pub mod entities;
pub mod errors;
pub mod i18n;
pub mod labels;
pub mod nlp;
pub mod quickadd;
pub mod recurrence;
pub mod search;
pub mod smartlists;
pub mod tree;
pub mod view_rules;

pub use agenda::{DEFAULT_SORT, FocusMode, GroupKey, SortOrder, SortSpec, is_picked};
pub use agenda_order::{
    AgendaLayoutSave, DEFAULT_ORDER, GROUP_KEYS, ORDER_KEYS, SECTION_BREAK, agenda_layout_save,
    agenda_order_for, clean_agenda_order, default_order,
};
pub use dates::{Date, InvalidDate};
pub use dnd::{DropOutcome, DropZone, GroupRect, TASK_EDGE};
pub use entities::{
    HydratedTask, Importance, MatchMode, RecurrenceRule, RecurrenceType, RecurrenceUnit, Step, Tag,
    Task, TaskStatus, TrashEntry, TrashKind, View, Weekday,
};
pub use errors::CodedError;
pub use nlp::{ParseOptions, ParsedQuickAdd, parse_quick_add};
pub use recurrence::{RawRecurrence, TYPES, next_date, next_on_or_after, validate};
pub use search::{score, search_tasks};
pub use smartlists::{SMART, SmartList, SmartSettings, smart_tasks, visible_order};
pub use tree::ParentMap;
pub use view_rules::{
    RawSortLevel, RawView, SORT_KEYS, SortDirection, SortKey, SortLevel, ViewRules, normalise_view,
    sort_tasks, view_matches,
};
