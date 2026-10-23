//! The egui application: rail, list, composer, theme toggle.

use std::collections::HashMap;

use doene_core::i18n::tr;
use doene_core::{SMART, SmartList, SmartSettings};
use doene_service::Service;
use eframe::egui::{self, Color32, RichText};
use serde_json::json;

use crate::model::{TaskRow, Theme, list_rows};

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

impl eframe::App for DöneApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.apply_theme(&ctx);

        egui::Panel::left("rail").exact_size(190.0).show(ui, |ui| {
            ui.add_space(8.0);
            ui.heading("Döne");
            ui.add_space(8.0);
            for l in SMART {
                if ui.selectable_label(self.list == l, self.t(nav_key(l))).clicked() {
                    self.list = l;
                    self.reload();
                }
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
                for r in &self.rows {
                    ui.horizontal(|ui| {
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
                    });
                }
            });
        });
    }
}
