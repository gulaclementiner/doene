//! Döne core: pure domain logic and entity types.
//!
//! No I/O, no UI dependencies (docs/plan.md §3). The public API is
//! deliberately small: entities, read-model builders, and pure rule functions.

pub mod dates;
pub mod entities;
pub mod errors;
pub mod recurrence;

pub use dates::{Date, InvalidDate};
pub use entities::{
    Importance, MatchMode, RecurrenceRule, RecurrenceType, RecurrenceUnit, Step, Tag, Task,
    TaskStatus, TrashEntry, TrashKind, View, Weekday,
};
pub use errors::CodedError;
pub use recurrence::{RawRecurrence, TYPES, next_date, next_on_or_after, validate};
