//! Döne core: pure domain logic and entity types.
//!
//! No I/O, no UI dependencies (docs/plan.md §3). The public API is
//! deliberately small: entities, read-model builders, and pure rule functions.

pub mod dates;
pub mod dnd;
pub mod entities;
pub mod tree;

pub use dates::{Date, InvalidDate};
pub use dnd::{DropOutcome, DropZone, GroupRect, TASK_EDGE};
pub use entities::{
    Importance, MatchMode, RecurrenceRule, RecurrenceType, RecurrenceUnit, Step, Tag, Task,
    TaskStatus, TrashEntry, TrashKind, View, Weekday,
};
pub use tree::ParentMap;
