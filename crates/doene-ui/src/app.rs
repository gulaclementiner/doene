//! The egui application: rail, list, composer, theme toggle.

use std::collections::HashMap;

use doene_core::i18n::tr;
use doene_core::{SMART, SmartList, SmartSettings};
use doene_service::{SavedView, Service};
use eframe::egui::{self, Color32, RichText};
use serde_json::json;

use crate::drawer::{
    ActivityLine, FieldEdit, RecurrenceDraft, StepEdit, TagEdit, StepRow, SubtaskNode, activity_lines, add_subtask,
    apply_edit, apply_step, apply_tag, tag_names, tag_suggestions, load_order, move_section, step_rows, subtask_tree,
};
use crate::palette::{self, Action, Command};
use crate::views::{SortDraft, ViewDraft, describe, drop_index, rail_rows, sort_key_label, view_rows};
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
    drawer: Option<Drawer>,
    views: Vec<SavedView>,
    /// The open custom view (its id), if the list shown is a view.
    view: Option<String>,
    form: Option<ViewDraft>,
    form_error: Option<String>,
    /// Open palette: (query, highlighted index).
    palette: Option<(String, usize)>,
    focus_composer: bool,
}

/// Open drawer state: edit buffers for one task.
struct Drawer {
    id: String,
    subject: String,
    notes: String,
    due: String,
    reminder: String,
    important: bool,
    rec: RecurrenceDraft,
    order: Vec<String>,
    activity: Vec<ActivityLine>,
    customizing: bool,
    steps: Vec<StepRow>,
    subtasks: Vec<SubtaskNode>,
    new_step: String,
    new_subtask: String,
    tags: Vec<String>,
    tag_suggestions: Vec<String>,
    new_tag: String,
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
            drawer: None,
            views: vec![],
            view: None,
            form: None,
            form_error: None,
            palette: None,
            focus_composer: false,
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
                self.views = self.service.views().unwrap_or_default();
                if self.view.as_ref().is_some_and(|id| !self.views.iter().any(|v| &v.view.id == id)) {
                    self.view = None;
                }
                self.rows = match self.views.iter().find(|v| Some(&v.view.id) == self.view.as_ref()) {
                    Some(v) => view_rows(v, &tasks, self.service.today(), &self.locale),
                    None => list_rows(self.list, &tasks, self.service.today(), settings, &self.locale),
                };
                if let Some(id) = self.drawer.as_ref().map(|d| d.id.clone()) {
                    self.open_drawer(&id);
                }
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    /// (Re)load the drawer buffers for a task; closes the drawer if it is gone.
    fn open_drawer(&mut self, id: &str) {
        let Ok(h) = self.service.task(id) else {
            self.drawer = None;
            return;
        };
        let t = &h.task;
        self.drawer = Some(Drawer {
            id: id.to_owned(),
            subject: t.subject.clone(),
            notes: t.body.clone().unwrap_or_default(),
            due: t.due_date.map(|d| d.to_string()).unwrap_or_default(),
            reminder: t.reminder_at.clone().unwrap_or_default().replace('T', " "),
            important: t.importance == doene_core::Importance::High,
            rec: RecurrenceDraft::from_rule(t.recurrence.as_ref()),
            order: load_order(&self.service, id),
            activity: activity_lines(&self.service, id),
            customizing: self.drawer.as_ref().is_some_and(|d| d.id == id && d.customizing),
            steps: step_rows(&self.service, id),
            subtasks: subtask_tree(&self.service, id),
            new_step: self.drawer.as_ref().filter(|d| d.id == id).map(|d| d.new_step.clone()).unwrap_or_default(),
            new_subtask: self.drawer.as_ref().filter(|d| d.id == id).map(|d| d.new_subtask.clone()).unwrap_or_default(),
            tags: tag_names(&self.service, id),
            tag_suggestions: tag_suggestions(&self.service, id),
            new_tag: self.drawer.as_ref().filter(|d| d.id == id).map(|d| d.new_tag.clone()).unwrap_or_default(),
        });
    }

    fn edit_field(&mut self, edit: FieldEdit) {
        let Some(id) = self.drawer.as_ref().map(|d| d.id.clone()) else { return };
        match apply_edit(&self.service, &id, edit, &self.locale) {
            Ok(()) => self.error = None,
            Err(m) => self.error = Some(m),
        }
        self.reload();
    }

    /// Run a drawer action (steps/subtasks) and refresh; errors surface as feedback.
    fn drawer_action(&mut self, f: impl FnOnce(&Service, &str, &str) -> Result<(), String>) {
        let Some(id) = self.drawer.as_ref().map(|d| d.id.clone()) else { return };
        match f(&self.service, &id, &self.locale) {
            Ok(()) => self.error = None,
            Err(m) => self.error = Some(m),
        }
        self.reload();
    }

    fn move_drawer_section(&mut self, index: usize, delta: i32) {
        let Some(id) = self.drawer.as_ref().map(|d| d.id.clone()) else { return };
        match move_section(&self.service, &id, index, delta) {
            Ok(order) => {
                if let Some(d) = &mut self.drawer {
                    d.order = order;
                }
            }
            Err(e) => self.error = Some(crate::drawer::error_message(&e, &self.locale)),
        }
    }

    fn submit(&mut self) {
        let text = std::mem::take(&mut self.composer);
        if text.trim().is_empty() {
            return;
        }
        match self.service.quick_add(&text, self.view.clone().map(doene_core::quickadd::QuickAddView::Custom).as_ref(), false) {
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

    fn run_action(&mut self, a: Action, ctx: &egui::Context) {
        match a {
            Action::OpenList(l) => {
                self.list = l;
                self.view = None;
                self.show_trash = false;
                self.form = None;
                self.reload();
            }
            Action::OpenView(id) => {
                self.view = Some(id);
                self.show_trash = false;
                self.form = None;
                self.reload();
            }
            Action::OpenTask(id) => {
                self.show_trash = false;
                self.form = None;
                self.open_drawer(&id);
            }
            Action::Run(Command::NewTask) => {
                self.show_trash = false;
                self.form = None;
                self.focus_composer = true;
            }
            Action::Run(Command::NewView) => self.open_form(None),
            Action::Run(Command::ToggleTheme) => {
                let dark = self.is_dark(ctx);
                self.set_theme(if dark { Theme::Light } else { Theme::Dark });
            }
        }
    }

    fn is_dark(&self, ctx: &egui::Context) -> bool {
        match self.theme {
            Theme::Dark => true,
            Theme::Light => false,
            Theme::System => ctx.global_style().visuals.dark_mode,
        }
    }

    fn palette_ui(&mut self, ctx: &egui::Context) {
        let Some((mut query, mut sel)) = self.palette.take() else { return };
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            return;
        }
        let hits = self.service.search(&query, palette::TASK_LIMIT).unwrap_or_default();
        let entries = palette::build_entries(&query, &self.views, &hits, self.is_dark(ctx), &self.locale);
        let n = entries.len();
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)) {
            sel = palette::move_sel(sel, n, 1);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)) {
            sel = palette::move_sel(sel, n, -1);
        }
        sel = sel.min(n.saturating_sub(1));
        let mut chosen = ctx
            .input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
            .then(|| entries.get(sel).map(|e| e.action.clone()))
            .flatten();
        let mut open = true;
        egui::Window::new(self.t("palette.dialogLabel"))
            .title_bar(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 60.0])
            .fixed_size([480.0, 0.0])
            .show(ctx, |ui| {
                let r = ui.add(
                    egui::TextEdit::singleline(&mut query)
                        .hint_text(self.t("palette.placeholder"))
                        .desired_width(f32::INFINITY),
                );
                r.request_focus();
                if r.changed() {
                    sel = 0;
                }
                if n == 0 {
                    ui.weak(self.t("palette.noResults"));
                }
                let mut last = None;
                egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                    for (i, e) in entries.iter().enumerate() {
                        if e.section != last {
                            if let Some(h) = e.section {
                                ui.add_space(4.0);
                                ui.weak(self.t(h));
                            }
                            last = e.section;
                        }
                        let resp = ui.selectable_label(i == sel, &e.label);
                        if i == sel {
                            resp.scroll_to_me(None);
                        }
                        if resp.clicked() {
                            chosen = Some(e.action.clone());
                        }
                    }
                });
                ui.separator();
                ui.weak(format!(
                    "↑↓ {}   ⏎ {}   Esc {}",
                    self.t("palette.navigate"),
                    self.t("palette.open"),
                    self.t("palette.close")
                ));
            });
        if let Some(a) = chosen {
            open = false;
            self.run_action(a, ctx);
        }
        if open {
            self.palette = Some((query, sel));
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
    fn open_form(&mut self, draft: Option<ViewDraft>) {
        self.form = Some(draft.unwrap_or_default());
        self.form_error = None;
        self.show_trash = false;
    }

    /// Custom views in the rail: emoji + colour, click to open, drag to reorder.
    fn views_rail_ui(&mut self, ui: &mut egui::Ui) {
        let rows = rail_rows(&self.views);
        let mut open = None;
        let mut moved = None;
        let mut rects: Vec<(f32, f32)> = vec![];
        for (i, r) in rows.iter().enumerate() {
            let color = Color32::from_rgb(r.color[0], r.color[1], r.color[2]);
            let selected = !self.show_trash && self.view.as_deref() == Some(r.id.as_str());
            let resp = ui
                .horizontal(|ui| {
                    ui.label(RichText::new(&r.icon).color(color));
                    ui.add(egui::Button::selectable(selected, RichText::new(&r.name)).sense(egui::Sense::click_and_drag()))
                })
                .inner;
            rects.push((resp.rect.top(), resp.rect.bottom()));
            if resp.clicked() {
                open = Some(r.id.clone());
            }
            if resp.drag_stopped() {
                if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
                    moved = Some((i, p.y));
                }
            }
        }
        if let Some(id) = open {
            self.view = Some(id);
            self.show_trash = false;
            self.form = None;
            self.reload();
        }
        if let Some((from, y)) = moved {
            let to = drop_index(&rects, from, y);
            if to != from {
                match self.service.move_view(&rows[from].id, to) {
                    Ok(()) => self.error = None,
                    Err(e) => self.error = Some(e.to_string()),
                }
                self.reload();
            }
        }
        if ui.button(format!("+ {}", self.t("nav.newView"))).clicked() {
            self.open_form(None);
        }
    }

    fn save_form(&mut self) {
        let Some(form) = self.form.clone() else { return };
        if let Err(m) = form.validate(&self.locale) {
            self.form_error = Some(m);
            return;
        }
        match form.save(&self.service) {
            Ok(id) => {
                self.view = Some(id);
                self.form = None;
                self.form_error = None;
            }
            Err(e) => self.form_error = Some(crate::drawer::error_message(&e, &self.locale)),
        }
        self.reload();
    }

    fn delete_form_view(&mut self) {
        let Some(id) = self.form.as_ref().and_then(|f| f.id.clone()) else { return };
        match self.service.delete_view(&id) {
            Ok(_) => {
                let m = self.t("trash.deleted");
                self.toasts.push(m, true, self.now);
                self.form = None;
                self.view = None;
            }
            Err(e) => self.form_error = Some(crate::drawer::error_message(&e, &self.locale)),
        }
        self.reload();
    }

    fn view_form_ui(&mut self, ui: &mut egui::Ui) {
        let Some(mut f) = self.form.take() else { return };
        let (mut save, mut cancel, mut delete) = (false, false, false);
        let editing = f.id.is_some();
        ui.heading(self.t(if editing { "viewForm.editViewTitle" } else { "viewForm.newViewTitle" }));
        ui.add_space(6.0);
        egui::Grid::new("view_form").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.label(self.t("viewForm.name"));
            ui.text_edit_singleline(&mut f.name);
            ui.end_row();
            ui.label(self.t("viewForm.emoji"));
            ui.add(egui::TextEdit::singleline(&mut f.emoji).desired_width(60.0));
            ui.end_row();
            ui.label(self.t("viewForm.tags"));
            ui.vertical(|ui| {
                if f.tags.is_empty() {
                    ui.weak(self.t("viewForm.tagListEmpty"));
                }
                let mut remove = None;
                ui.horizontal_wrapped(|ui| {
                    for (i, t) in f.tags.iter().enumerate() {
                        if ui.small_button(format!("#{t} ✕")).on_hover_text(self.t("viewForm.removeTag")).clicked() {
                            remove = Some(i);
                        }
                    }
                });
                if let Some(i) = remove {
                    f.tags.remove(i);
                }
                let hint = self.t("viewForm.tagPlaceholder");
                let r = ui.add(egui::TextEdit::singleline(&mut f.new_tag).hint_text(hint));
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    f.commit_tag();
                    r.request_focus();
                }
            });
            ui.end_row();
            ui.label(self.t("viewForm.match"));
            ui.horizontal(|ui| {
                ui.selectable_value(&mut f.all, false, self.t("viewForm.any"));
                ui.selectable_value(&mut f.all, true, self.t("viewForm.all"));
            });
            ui.end_row();
            ui.label(self.t("viewForm.sort"));
            ui.vertical(|ui| {
                let mut remove = None;
                for (i, s) in f.sorts.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt(("sort_by", i))
                            .selected_text(self.t(sort_key_label(s.by)))
                            .show_ui(ui, |ui| {
                                for k in doene_core::SortKey::ALL {
                                    ui.selectable_value(&mut s.by, k, self.t(sort_key_label(k)));
                                }
                            });
                        if s.by != doene_core::SortKey::Manual {
                            let label = self.t(if s.desc { "viewForm.descending" } else { "viewForm.ascending" });
                            if ui.button(label).clicked() {
                                s.desc = !s.desc;
                            }
                        }
                        if ui.small_button("✕").on_hover_text(self.t("viewForm.removeSort")).clicked() {
                            remove = Some(i);
                        }
                    });
                }
                if let Some(i) = remove {
                    f.remove_sort(i);
                }
                if f.sorts.len() < doene_core::MAX_SORTS && ui.button(self.t("viewForm.addSort")).clicked() {
                    f.sorts.push(SortDraft { by: doene_core::SortKey::Name, desc: false });
                }
            });
            ui.end_row();
            ui.label("");
            ui.checkbox(&mut f.show_done, self.t("viewForm.showDone"));
            ui.end_row();
        });
        ui.add_space(4.0);
        ui.weak(describe(&f, &self.locale));
        if let Some(e) = &self.form_error {
            ui.colored_label(Color32::from_rgb(0xd6, 0x45, 0x45), e);
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            save = ui.button(self.t("viewForm.save")).clicked();
            cancel = ui.button(self.t("viewForm.cancel")).clicked();
            if editing {
                delete = ui.button(self.t("viewForm.delete")).clicked();
            }
        });
        self.form = Some(f);
        if save {
            self.save_form();
        } else if delete {
            self.delete_form_view();
        } else if cancel {
            self.form = None;
            self.form_error = None;
        }
    }

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

impl DöneApp {
    fn drawer_ui(&mut self, ui: &mut egui::Ui) {
        let locale = self.locale.clone();
        let activity_title = self.t("drawer.activity");
        let Some(d) = self.drawer.as_mut() else { return };
        let mut edit = None;
        let mut step_edit: Option<StepEdit> = None;
        let mut sub_add: Option<String> = None;
        let mut tag_edit: Option<TagEdit> = None;
        let mut sub_toggle: Option<(String, bool)> = None;
        let mut open_sub: Option<String> = None;
        let mut close = false;
        let mut move_sec = None;
        let weekday_keys = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];
        let t = |k: &str| tr(k, &HashMap::new(), Some(&locale));
        ui.horizontal(|ui| {
            ui.heading(&d.subject);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(t("drawer.close")).on_hover_text(t("drawer.closeTip")).clicked() {
                    close = true;
                }
                if ui.selectable_label(d.customizing, "⇅").on_hover_text(t("drawer.reorderSections")).clicked() {
                    d.customizing = !d.customizing;
                }
            });
        });
        egui::ScrollArea::vertical().show(ui, |ui| {
            if d.customizing {
                for (i, sec) in d.order.clone().iter().enumerate() {
                    let p = HashMap::from([("section".to_owned(), Some(t(&format!("section.{sec}"))))]);
                    ui.horizontal(|ui| {
                        ui.label(t(&format!("section.{sec}")));
                        if ui.small_button("▲").on_hover_text(tr("layout.moveUp", &p, Some(&locale))).clicked() {
                            move_sec = Some((i, -1));
                        }
                        if ui.small_button("▼").on_hover_text(tr("layout.moveDown", &p, Some(&locale))).clicked() {
                            move_sec = Some((i, 1));
                        }
                    });
                }
                ui.separator();
            }
            for sec in d.order.clone() {
                match sec.as_str() {
                    "customize" => {
                        ui.label(RichText::new(t("section.customize")).strong());
                        if ui.checkbox(&mut d.important, t("task.markImportant")).changed() {
                            edit = Some(FieldEdit::Important(d.important));
                        }
                        ui.horizontal(|ui| {
                            ui.label(t("drawer.dueDate"));
                            let r = ui.add(egui::TextEdit::singleline(&mut d.due).hint_text("YYYY-MM-DD").desired_width(110.0));
                            if r.lost_focus() {
                                edit = Some(FieldEdit::Due(d.due.clone()));
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(t("drawer.reminder"));
                            let r = ui.add(egui::TextEdit::singleline(&mut d.reminder).hint_text("YYYY-MM-DD HH:MM").desired_width(150.0));
                            if r.lost_focus() {
                                edit = Some(FieldEdit::Reminder(d.reminder.clone()));
                            }
                        });
                        let rec_label = |k: Option<&str>| match k {
                            None => t("drawer.repeatNone"),
                            Some("custom") => t("recurrence.customEvery").replace(" {n} {unit}", "…"),
                            Some(k) => t(&format!("recurrence.{k}")),
                        };
                        let mut rec_changed = false;
                        ui.horizontal(|ui| {
                            ui.label(t("drawer.repeat"));
                            egui::ComboBox::from_id_salt("rec_kind")
                                .selected_text(rec_label(d.rec.kind))
                                .show_ui(ui, |ui| {
                                    rec_changed |= ui.selectable_value(&mut d.rec.kind, None, rec_label(None)).changed();
                                    for k in doene_core::recurrence::TYPES {
                                        rec_changed |= ui.selectable_value(&mut d.rec.kind, Some(k), rec_label(Some(k))).changed();
                                    }
                                });
                        });
                        if d.rec.kind == Some("custom") {
                            ui.horizontal(|ui| {
                                ui.label(t("recurrence.customEvery").split(" {n}").next().unwrap_or("").to_owned());
                                rec_changed |= ui.add(egui::DragValue::new(&mut d.rec.interval).range(1..=999)).changed();
                                egui::ComboBox::from_id_salt("rec_unit").selected_text(t(&format!("recurrence.{}", d.rec.unit))).show_ui(ui, |ui| {
                                    for u in ["day", "week", "month", "year"] {
                                        rec_changed |= ui.selectable_value(&mut d.rec.unit, u, t(&format!("recurrence.{u}"))).changed();
                                    }
                                });
                            });
                            if d.rec.unit == "week" {
                                ui.horizontal(|ui| {
                                    for (dow, k) in [1u8, 2, 3, 4, 5, 6, 0].into_iter().map(|n| (n, weekday_keys[n as usize])) {
                                        let on = d.rec.days.contains(&dow);
                                        if ui.selectable_label(on, t(&format!("weekday.{k}"))).clicked() {
                                            d.rec.toggle_day(dow);
                                            rec_changed = true;
                                        }
                                    }
                                });
                            }
                        }
                        if rec_changed {
                            edit = Some(FieldEdit::Recurrence(d.rec.clone()));
                        }
                    }
                    "notes" => {
                        ui.label(RichText::new(t("section.notes")).strong());
                        let r = ui.add(
                            egui::TextEdit::multiline(&mut d.notes)
                                .hint_text(t("drawer.notesPlaceholder"))
                                .desired_width(f32::INFINITY),
                        );
                        if r.lost_focus() {
                            edit = Some(FieldEdit::Notes(d.notes.clone()));
                        }
                    }
                    "steps" => {
                        ui.label(RichText::new(t("section.steps")).strong());
                        for st in &d.steps {
                            ui.horizontal(|ui| {
                                let p = HashMap::from([("name".to_owned(), Some(st.subject.clone()))]);
                                let mut done = st.done;
                                if ui.checkbox(&mut done, "").on_hover_text(tr("drawer.completeStep", &p, Some(&locale))).changed() {
                                    step_edit = Some(StepEdit::Toggle(st.id.clone()));
                                }
                                ui.label(if st.done { RichText::new(&st.subject).strikethrough().weak() } else { RichText::new(&st.subject) });
                                if ui.small_button("✕").on_hover_text(tr("drawer.deleteStep", &p, Some(&locale))).clicked() {
                                    step_edit = Some(StepEdit::Delete(st.id.clone()));
                                }
                                if ui.small_button("⤴").on_hover_text(t("task.dropToStep")).clicked() {
                                    step_edit = Some(StepEdit::Promote(st.id.clone()));
                                }
                            });
                        }
                        let r = ui.add(egui::TextEdit::singleline(&mut d.new_step).hint_text(t("drawer.addStepPlaceholder")).desired_width(f32::INFINITY));
                        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !d.new_step.trim().is_empty() {
                            step_edit = Some(StepEdit::Add(std::mem::take(&mut d.new_step)));
                            r.request_focus();
                        }
                    }
                    "subtasks" => {
                        ui.label(RichText::new(t("section.subtasks")).strong());
                        if d.subtasks.is_empty() {
                            ui.label(RichText::new(t("drawer.subtasksHint")).small().weak());
                        }
                        for n in &d.subtasks {
                            ui.horizontal(|ui| {
                                ui.add_space(14.0 * n.depth as f32);
                                let p = HashMap::from([("name".to_owned(), Some(n.subject.clone()))]);
                                let mut done = n.done;
                                let tip = tr(if n.done { "drawer.reopenSubtask" } else { "drawer.completeSubtask" }, &p, Some(&locale));
                                if ui.checkbox(&mut done, "").on_hover_text(tip).changed() {
                                    sub_toggle = Some((n.id.clone(), n.done));
                                }
                                if ui.link(&n.subject).clicked() {
                                    open_sub = Some(n.id.clone());
                                }
                            });
                        }
                        let r = ui.add(egui::TextEdit::singleline(&mut d.new_subtask).hint_text(t("drawer.addSubtaskPlaceholder")).desired_width(f32::INFINITY));
                        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !d.new_subtask.trim().is_empty() {
                            sub_add = Some(std::mem::take(&mut d.new_subtask));
                            r.request_focus();
                        }
                    }
                    "tags" => {
                        ui.label(RichText::new(t("section.tags")).strong());
                        ui.horizontal_wrapped(|ui| {
                            for g in &d.tags {
                                let p = HashMap::from([("tag".to_owned(), Some(g.clone()))]);
                                ui.label(format!("#{g}"));
                                if ui.small_button("✕").on_hover_text(tr("drawer.removeTag", &p, Some(&locale))).clicked() {
                                    tag_edit = Some(TagEdit::Remove(g.clone()));
                                }
                            }
                        });
                        let r = ui.add(egui::TextEdit::singleline(&mut d.new_tag).hint_text(t("drawer.addTagExample")).desired_width(f32::INFINITY));
                        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !d.new_tag.trim().is_empty() {
                            tag_edit = Some(TagEdit::Add(std::mem::take(&mut d.new_tag)));
                            r.request_focus();
                        }
                        let needle = d.new_tag.trim().trim_start_matches(['#', '@']).to_lowercase();
                        ui.horizontal_wrapped(|ui| {
                            for g in d.tag_suggestions.iter().filter(|g| g.contains(&needle)).take(8) {
                                if ui.small_button(format!("#{g}")).clicked() {
                                    tag_edit = Some(TagEdit::Add(g.clone()));
                                }
                            }
                        });
                    }
                    // References and completion arrive in later tickets.
                    _ => {}
                }
                ui.add_space(6.0);
            }
            ui.separator();
            ui.label(RichText::new(activity_title.clone()).strong());
            for a in &d.activity {
                let line = match &a.detail {
                    Some(x) => format!("{}  {} · {}", a.at, a.action, x),
                    None => format!("{}  {}", a.at, a.action),
                };
                ui.label(RichText::new(line).small().weak());
            }
        });
        if close {
            self.drawer = None;
        }
        if let Some((i, delta)) = move_sec {
            self.move_drawer_section(i, delta);
        }
        if let Some(e) = edit {
            self.edit_field(e);
        }
        if let Some(e) = step_edit {
            self.drawer_action(|s, id, l| apply_step(s, id, e, l));
        }
        if let Some(e) = tag_edit {
            self.drawer_action(|s, id, l| apply_tag(s, id, e, l));
        }
        if let Some(sub) = sub_add {
            self.drawer_action(|s, id, l| add_subtask(s, id, &sub, l));
        }
        if let Some((sid, was_done)) = sub_toggle {
            self.toggle(&sid, was_done);
        }
        if let Some(sid) = open_sub {
            self.open_drawer(&sid);
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

        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::K)) {
            self.palette = if self.palette.is_some() { None } else { Some((String::new(), 0)) };
        }

        egui::Panel::left("rail").exact_size(190.0).show(ui, |ui| {
            ui.add_space(8.0);
            ui.heading("Döne");
            ui.add_space(8.0);
            for l in SMART {
                if ui.selectable_label(!self.show_trash && self.view.is_none() && self.list == l, self.t(nav_key(l))).clicked() {
                    self.list = l;
                    self.view = None;
                    self.show_trash = false;
                    self.reload();
                }
            }
            ui.separator();
            self.views_rail_ui(ui);
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

        if self.drawer.is_some() && !self.show_trash {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !ctx.egui_wants_keyboard_input() {
                self.drawer = None;
            } else {
                egui::Panel::right("drawer").default_size(340.0).show(ui, |ui| self.drawer_ui(ui));
            }
        }

        egui::CentralPanel::default().show(ui, |ui| {
            if self.show_trash {
                self.trash_ui(ui);
                return;
            }
            if self.form.is_some() {
                self.view_form_ui(ui);
                return;
            }
            let title = match self.views.iter().find(|v| Some(&v.view.id) == self.view.as_ref()) {
                Some(v) => format!("{} {}", v.view.emoji.clone().unwrap_or_default(), v.view.name).trim().to_owned(),
                None => self.t(if self.list == SmartList::All { "nav.allTitle" } else { nav_key(self.list) }),
            };
            ui.horizontal(|ui| {
                ui.heading(title);
                if let Some(id) = self.view.clone() {
                    if ui.small_button("✎").on_hover_text(self.t("viewForm.editView")).clicked() {
                        self.open_form(self.views.iter().find(|v| v.view.id == id).map(ViewDraft::from_view));
                    }
                }
            });
            ui.add_space(6.0);
            let hint = self.t("composer.placeholder");
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.composer)
                    .hint_text(hint)
                    .desired_width(f32::INFINITY),
            );
            if std::mem::take(&mut self.focus_composer) {
                resp.request_focus();
            }
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
                let mut opened = None;
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
                        if ui.add(egui::Label::new(subject).sense(egui::Sense::click())).clicked() {
                            opened = Some(r.id.clone());
                        }
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
                if let Some(id) = opened {
                    self.open_drawer(&id);
                }
            });
        });
        self.palette_ui(&ctx);
        self.show_toasts(&ctx);
    }
}
