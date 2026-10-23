//! The egui application: rail, list, composer, theme toggle.

use std::collections::HashMap;

use doene_core::i18n::tr;
use doene_core::{SMART, SmartList, SmartSettings};
use doene_service::Service;
use eframe::egui::{self, Color32, RichText};
use serde_json::json;

use crate::model::{
    TaskRow, Theme, Toasts, Toggled, TrashRow, delete_with_toast, list_rows, toggle_complete, trash_rows,
    undo_message,
};

const CORAL: Color32 = Color32::from_rgb(0xe4, 0x57, 0x2e);
const AMBER: Color32 = Color32::from_rgb(0xc9, 0x8a, 0x1b);

/// The single app state.
pub struct DöneApp {
    service: Service,
    list: SmartList,
    theme: Theme,
    locale: String,
    composer: String,
    error: Option<String>,
    toasts: Toasts,
    /// Current time in seconds (updated each frame) for toast expiry.
    now: f64,
    show_trash: bool,
    trash: Vec<TrashRow>,
    rows: Vec<TaskRow>,
    applied_theme: Option<Theme>,
}

fn nav_key(l: SmartList) -> &'static str {
    match l {
        SmartList::MyDay => "nav.myDay",
        SmartList::Shortlisted => "nav.shortlisted",
        SmartList::Important => "nav.important",
        SmartList::Planned => "nav.planned",
        SmartList::All => "nav.all",
        SmartList::Completed => "nav.completed",
    }
}

impl DöneApp {
    /// Build the app over an opened service; loads settings and tasks.
    pub fn new(service: Service) -> Self {
        let mut app = Self {
            service,
            list: SmartList::All,
            theme: Theme::System,
            locale: "en".into(),
            composer: String::new(),
            error: None,
            toasts: Toasts::default(),
            now: 0.0,
            show_trash: false,
            trash: vec![],
            rows: vec![],
            applied_theme: None,
        };
        app.reload();
        app
    }

    fn t(&self, key: &str) -> String {
        tr(key, &HashMap::new(), Some(&self.locale))
    }

    fn reload(&mut self) {
        let result = (|| {
            let s = self.service.settings()?;
            let tasks = self.service.all_tasks()?;
            Ok::<_, doene_service::ServiceError>((s, tasks))
        })();
        match result {
            Ok((s, tasks)) => {
                self.theme = Theme::parse(&s.theme);
                self.locale = s.language.clone();
                let settings = SmartSettings { focus_mode: s.focus_mode, ..Default::default() };
                self.trash = self.service.trash().map(|t| trash_rows(&t, &self.locale)).unwrap_or_default();
                self.rows = list_rows(self.list, &tasks, self.service.today(), settings, &self.locale);
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn submit(&mut self) {
        let text = std::mem::take(&mut self.composer);
        if text.trim().is_empty() {
            return;
        }
        match self.service.quick_add(&text, None, false) {
            Ok(_) => self.error = None,
            Err(e) => {
                self.error = Some(e.to_string());
                self.composer = text;
            }
        }
        self.reload();
    }

    fn toggle(&mut self, id: &str, completed: bool) {
        match toggle_complete(&self.service, id, completed, &self.locale) {
            Ok(Toggled::Completed { next_due: Some(date) }) => {
                let p = HashMap::from([("date".to_owned(), Some(date))]);
                let m = tr("task.completedNext", &p, Some(&self.locale));
                self.toasts.push(m, true, self.now);
                self.error = None;
            }
            Ok(Toggled::Completed { next_due: None }) => {
                let m = self.t("task.completedToast");
                self.toasts.push(m, true, self.now);
                self.error = None;
            }
            Ok(Toggled::Reopened) => {}
            Err(e) => self.error = Some(e.to_string()),
        }
        self.reload();
    }

    fn delete(&mut self, id: &str) {
        match delete_with_toast(&self.service, id, &self.locale) {
            Ok(m) => {
                self.toasts.push(m, true, self.now);
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        self.reload();
    }

    fn undo(&mut self) {
        match self.service.undo() {
            Ok(k) => {
                let m = undo_message(k, &self.locale);
                self.toasts.push(m, false, self.now);
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        self.reload();
    }

    fn trash_action(&mut self, id: &str, action: TrashAction) {
        let r = match action {
            TrashAction::Restore => self.service.restore(id).map(|_| "trash.restored"),
            TrashAction::Forever => self.service.delete_forever(id).map(|_| "trash.deletedForever"),
            TrashAction::Empty => self.service.empty_trash().map(|_| "trash.trashEmptied"),
        };
        match r {
            Ok(key) => {
                let m = self.t(key);
                self.toasts.push(m, action == TrashAction::Restore, self.now);
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        self.reload();
    }

    fn show_toasts(&mut self, ctx: &egui::Context) {
        let live = self.toasts.live(self.now).to_vec();
        if live.is_empty() {
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
        let mut undo = false;
        let mut dismiss = None;
        egui::Area::new(egui::Id::new("toasts"))
            .anchor(egui::Align2::RIGHT_BOTTOM, [-12.0, -12.0])
            .show(ctx, |ui| {
                for (i, t) in live.iter().enumerate() {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(&t.text);
                            if t.undo && ui.button(self.t("undo.undo")).clicked() {
                                undo = true;
                                dismiss = Some(i);
                            }
                            if ui.small_button("✕").clicked() {
                                dismiss = Some(i);
                            }
                        });
                    });
                }
            });
        if let Some(i) = dismiss {
            self.toasts.dismiss(i);
        }
        if undo {
            self.undo();
        }
    }

    fn set_theme(&mut self, theme: Theme) {
        match self.service.set_setting("theme", json!(theme.id())) {
            Ok(_) => self.theme = theme,
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    fn apply_theme(&mut self, ctx: &egui::Context) {
        if self.applied_theme == Some(self.theme) {
            return;
        }
        ctx.set_theme(match self.theme {
            Theme::Light => egui::ThemePreference::Light,
            Theme::Dark => egui::ThemePreference::Dark,
            Theme::System => egui::ThemePreference::System,
        });
        ctx.all_styles_mut(|s| s.visuals.selection.bg_fill = CORAL);
        self.applied_theme = Some(self.theme);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TrashAction {
    Restore,
    Forever,
    Empty,
}

impl DöneApp {
    fn trash_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.t("trash.title"));
        ui.label(RichText::new(self.t("trash.emptyHint")).small().weak());
        if self.trash.is_empty() {
            ui.label(self.t("trash.empty"));
            return;
        }
        if ui.button(self.t("trash.emptyTrash")).clicked() {
            self.trash_action("", TrashAction::Empty);
            return;
        }
        let mut act = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for r in &self.trash {
                ui.horizontal(|ui| {
                    ui.label(&r.label);
                    ui.label(RichText::new(format!("{} · {}", r.kind, r.deleted)).small().weak());
                    if ui.small_button(self.t("trash.restore")).clicked() {
                        act = Some((r.id.clone(), TrashAction::Restore));
                    }
                    if ui.small_button(self.t("trash.deleteForever")).clicked() {
                        act = Some((r.id.clone(), TrashAction::Forever));
                    }
                });
            }
        });
        if let Some((id, a)) = act {
            self.trash_action(&id, a);
        }
    }
}

impl eframe::App for DöneApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.apply_theme(&ctx);
        self.now = ctx.input(|i| i.time);
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) && !ctx.egui_wants_keyboard_input() {
            self.undo();
        }

        egui::Panel::left("rail").exact_size(190.0).show(ui, |ui| {
            ui.add_space(8.0);
            ui.heading("Döne");
            ui.add_space(8.0);
            for l in SMART {
                if ui.selectable_label(!self.show_trash && self.list == l, self.t(nav_key(l))).clicked() {
                    self.list = l;
                    self.show_trash = false;
                    self.reload();
                }
            }
            ui.separator();
            let label = format!("🗑 {}", self.t("trash.title"));
            if ui.selectable_label(self.show_trash, label).clicked() {
                self.show_trash = true;
            }
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.add_space(8.0);
                let mut chosen = None;
                ui.horizontal(|ui| {
                    for th in Theme::ALL {
                        let short = match th {
                            Theme::Light => "☀",
                            Theme::Dark => "☾",
                            Theme::System => "⚙",
                        };
                        if ui
                            .selectable_label(self.theme == th, short)
                            .on_hover_text(self.t(th.label_key()))
                            .clicked()
                        {
                            chosen = Some(th);
                        }
                    }
                });
                if let Some(th) = chosen {
                    self.set_theme(th);
                }
            });
        });

        egui::CentralPanel::default().show(ui, |ui| {
            if self.show_trash {
                self.trash_ui(ui);
                return;
            }
            ui.heading(self.t(if self.list == SmartList::All { "nav.allTitle" } else { nav_key(self.list) }));
            ui.add_space(6.0);
            let hint = self.t("composer.placeholder");
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.composer)
                    .hint_text(hint)
                    .desired_width(f32::INFINITY),
            );
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.submit();
                resp.request_focus();
            }
            if let Some(e) = &self.error {
                ui.colored_label(Color32::from_rgb(0xd6, 0x45, 0x45), e);
            }
            ui.add_space(6.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut clicked = None;
                let mut deleted = None;
                for r in &self.rows {
                    ui.horizontal(|ui| {
                        let mut done = r.completed;
                        let p = HashMap::from([("name".to_owned(), Some(r.subject.clone()))]);
                        let hint = tr("task.complete", &p, Some(&self.locale));
                        if ui.checkbox(&mut done, "").on_hover_text(hint).clicked() {
                            clicked = Some((r.id.clone(), r.completed));
                        }
                        let mut subject = RichText::new(&r.subject);
                        if r.completed {
                            subject = subject.strikethrough().weak();
                        }
                        ui.label(subject);
                        if r.important {
                            ui.label(RichText::new("★").color(AMBER));
                        }
                        if let Some(d) = &r.due {
                            let c = if r.overdue { Color32::from_rgb(0xb4, 0x24, 0x24) } else { CORAL };
                            ui.label(RichText::new(d).small().color(c));
                        }
                        for tag in &r.tags {
                            ui.label(RichText::new(tag).small().weak());
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("🗑").on_hover_text(self.t("trash.deleted")).clicked() {
                                deleted = Some(r.id.clone());
                            }
                        });
                    });
                }
                if let Some((id, was)) = clicked {
                    self.toggle(&id, was);
                }
                if let Some(id) = deleted {
                    self.delete(&id);
                }
            });
        });
        self.show_toasts(&ctx);
    }
}
