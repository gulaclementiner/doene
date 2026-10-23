//! Plain view-model logic for saved views: the form draft, rail rows, view
//! task rows and drag-reorder maths. No egui types.

use std::collections::HashMap;

use doene_core::i18n::tr;
use doene_core::view_rules::{ViewRules, normalise_view, sort_tasks, view_matches};
use doene_core::{Date, HydratedTask, MAX_SORTS, MatchMode, RawSortLevel, RawView, SortDirection, SortKey};
use doene_service::{SavedView, Service, ServiceError};

use crate::drawer::error_message;
use crate::model::{TaskRow, task_row};

/// Rail colours (sRGB), assigned per view from a stable hash of its id.
/// There is no colour column in the schema (docs/plan.md §5), so colour is derived.
pub const PALETTE: [[u8; 3]; 8] = [
    [0xe4, 0x57, 0x2e],
    [0x2e, 0x8b, 0xe4],
    [0x3c, 0xa5, 0x6b],
    [0x9b, 0x59, 0xd0],
    [0xc9, 0x8a, 0x1b],
    [0xd0, 0x4a, 0x8a],
    [0x1f, 0xa3, 0xa3],
    [0x6b, 0x7a, 0x8f],
];

/// The view's rail colour: stable for an id, spread over the palette.
pub fn view_color(id: &str) -> [u8; 3] {
    let h = id.bytes().fold(5381u32, |h, b| h.wrapping_mul(33) ^ u32::from(b));
    PALETTE[(h as usize) % PALETTE.len()]
}

/// One rail entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewRailRow {
    pub id: String,
    /// Emoji icon, or a bullet when none.
    pub icon: String,
    pub name: String,
    pub color: [u8; 3],
}

/// Rail rows in stored order.
pub fn rail_rows(views: &[SavedView]) -> Vec<ViewRailRow> {
    views
        .iter()
        .map(|v| ViewRailRow {
            id: v.view.id.clone(),
            icon: v.view.emoji.clone().filter(|e| !e.trim().is_empty()).unwrap_or_else(|| "●".into()),
            name: v.view.name.clone(),
            color: view_color(&v.view.id),
        })
        .collect()
}

/// The rows a view shows: its tag/show-done rule, sorted per its levels.
pub fn view_rows(view: &SavedView, tasks: &[HydratedTask], today: Date, locale: &str) -> Vec<TaskRow> {
    let rules = ViewRules {
        name: view.view.name.clone(),
        emoji: view.view.emoji.clone(),
        tags: view.view.tags.iter().map(|t| t.as_str().to_owned()).collect(),
        match_mode: view.view.match_mode,
        sorts: view.sorts.clone(),
        show_done: view.view.show_done,
    };
    let matched: Vec<HydratedTask> = tasks.iter().filter(|t| view_matches(&rules, t)).cloned().collect();
    let today_s = today.to_string();
    sort_tasks(&matched, &view.sorts).into_iter().map(|h| task_row(h, today, &today_s, locale)).collect()
}

/// One sort level in the form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortDraft {
    pub by: SortKey,
    pub desc: bool,
}

/// Locale key of a sort field's label.
pub fn sort_key_label(k: SortKey) -> &'static str {
    match k {
        SortKey::Name => "viewForm.byName",
        SortKey::Due => "viewForm.byDue",
        SortKey::Created => "viewForm.byCreated",
        SortKey::Manual => "viewForm.byManual",
    }
}

/// Editable state of the view form.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewDraft {
    /// `None` = creating.
    pub id: Option<String>,
    pub name: String,
    pub emoji: String,
    pub tags: Vec<String>,
    pub new_tag: String,
    pub all: bool,
    pub sorts: Vec<SortDraft>,
    pub show_done: bool,
}

impl ViewDraft {
    /// An empty form for a new view.
    pub fn new() -> Self {
        Self {
            id: None,
            name: String::new(),
            emoji: String::new(),
            tags: vec![],
            new_tag: String::new(),
            all: false,
            sorts: vec![],
            show_done: false,
        }
    }

    /// A form pre-filled from a saved view.
    pub fn from_view(v: &SavedView) -> Self {
        Self {
            id: Some(v.view.id.clone()),
            name: v.view.name.clone(),
            emoji: v.view.emoji.clone().unwrap_or_default(),
            tags: v.view.tags.iter().map(|t| t.as_str().to_owned()).collect(),
            new_tag: String::new(),
            all: v.view.match_mode == MatchMode::All,
            sorts: v.sorts.iter().map(|s| SortDraft { by: s.by, desc: s.dir == SortDirection::Desc }).collect(),
            show_done: v.view.show_done,
        }
    }

    /// Add a sort level (ignored at [`MAX_SORTS`]). Returns whether it was added.
    pub fn add_sort(&mut self) -> bool {
        if self.sorts.len() >= MAX_SORTS {
            return false;
        }
        self.sorts.push(SortDraft { by: SortKey::Name, desc: false });
        true
    }

    /// Remove a sort level by index.
    pub fn remove_sort(&mut self, i: usize) {
        if i < self.sorts.len() {
            self.sorts.remove(i);
        }
    }

    /// Commit the tag input buffer as a tag (`#` stripped, deduped, no spaces).
    pub fn commit_tag(&mut self) {
        let t = self.new_tag.trim().trim_start_matches('#').trim().to_lowercase().replace(char::is_whitespace, "-");
        self.new_tag.clear();
        if !t.is_empty() && !self.tags.contains(&t) {
            self.tags.push(t);
        }
    }

    /// The raw form values handed to the service.
    pub fn to_raw(&self) -> RawView {
        RawView {
            name: Some(self.name.clone()),
            emoji: Some(self.emoji.trim().to_owned()).filter(|e| !e.is_empty()),
            tags: self.tags.clone(),
            match_mode: Some(if self.all { "all" } else { "any" }.into()),
            sorts: self
                .sorts
                .iter()
                .map(|s| RawSortLevel {
                    by: match s.by {
                        SortKey::Name => "name",
                        SortKey::Due => "due",
                        SortKey::Created => "created",
                        SortKey::Manual => "manual",
                    }
                    .into(),
                    dir: Some(if s.desc { "desc" } else { "asc" }.into()),
                })
                .collect(),
            show_done: self.show_done,
        }
    }

    /// Client-side validation, same rules as the service (core `normalise_view`).
    pub fn validate(&self, locale: &str) -> Result<(), String> {
        normalise_view(&self.to_raw()).map(|_| ()).map_err(|e| error_message(&ServiceError::Coded(e), locale))
    }

    /// Save: create or update via the service. Returns the saved view's id.
    pub fn save(&self, service: &Service) -> Result<String, ServiceError> {
        let saved = match &self.id {
            Some(id) => service.update_view(id, self.to_raw())?,
            None => service.create_view(self.to_raw())?,
        };
        Ok(saved.view.id)
    }
}

impl Default for ViewDraft {
    fn default() -> Self {
        Self::new()
    }
}

/// Localised one-line summary of a draft ("Tagged #a or #b, by Name, then Due").
pub fn describe(draft: &ViewDraft, locale: &str) -> String {
    let t = |k: &str, p: &[(&str, String)]| {
        let m: HashMap<String, Option<String>> = p.iter().map(|(k, v)| ((*k).to_owned(), Some(v.clone()))).collect();
        tr(k, &m, Some(locale))
    };
    let mut out = if draft.tags.is_empty() {
        t("viewForm.describeEveryTask", &[])
    } else {
        let sep = format!(" {} ", t(if draft.all { "viewForm.all" } else { "viewForm.describeOr" }, &[]).to_lowercase());
        let tags = draft.tags.iter().map(|g| format!("#{g}")).collect::<Vec<_>>().join(&sep);
        t("viewForm.describeTagged", &[("tags", tags)])
    };
    if !draft.sorts.is_empty() {
        let sorts = draft
            .sorts
            .iter()
            .map(|s| t(sort_key_label(s.by), &[]))
            .collect::<Vec<_>>()
            .join(&format!("{} ", t("viewForm.describeThen", &[]).trim_end()));
        out.push_str(", ");
        out.push_str(&t("viewForm.describeBy", &[("sorts", sorts)]));
    }
    out
}

/// Index a dragged rail row lands at: the number of *other* rows whose
/// midpoint lies above `y`. `rows` are `(top, bottom)` of every row in order.
pub fn drop_index(rows: &[(f32, f32)], from: usize, y: f32) -> usize {
    rows.iter()
        .enumerate()
        .filter(|(i, (top, bottom))| *i != from && (top + bottom) / 2.0 < y)
        .count()
}
