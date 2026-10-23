//! Döne UI: the eframe/egui application (docs/plan.md §8).
//!
//! Depends on `doene-service` (+ `doene-core` for types) only. The single
//! `AppState` and its modules land in M3; this crate is scaffolding until then.

/// The Döne window. M0: an empty app that opens and closes cleanly.
pub struct DöneApp;

impl eframe::App for DöneApp {
    fn ui(&mut self, _ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {}
}
