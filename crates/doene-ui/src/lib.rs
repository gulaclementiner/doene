//! Döne UI: the eframe/egui application (docs/plan.md §8).
//!
//! Depends on `doene-service` (+ `doene-core` for types) only. Structure:
//! `model` holds plain, unit-tested view-model logic; `app` holds the single
//! `DöneApp` state and egui rendering (thin, not unit-tested).

mod app;
pub mod model;

pub use app::DöneApp;
pub use model::{TaskRow, Theme, list_rows};
