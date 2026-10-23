//! Döne service layer: application operations orchestrating core rules and storage.
//!
//! The designated integration seam (ADR-0001, ADR-0002): explicit methods, plain
//! data in and out, no UI types across the boundary, transport-agnostic.
//! Mirrors Gör't's task-endpoint semantics (`store.js`, `server.js`). Every
//! mutation runs in one transaction and writes an `activity` row.
//! Errors are structured `{code, params}` ([`CodedError`]); the English message
//! is the fallback translation.

mod service;
mod snapshot;

pub use doene_core::CodedError;
pub use doene_storage::views::SavedView;
pub use service::{
    Clock, NewTask, Service, ServiceError, Settings, StepPatch, TaskPatch, TrashItem, Restored,
    UndoKind, UpdateOutcome, TRASH_DAYS,
};
