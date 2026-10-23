//! Task drawer view-model: field edits through the service, the recurrence
//! editor draft, section ordering (prefs) and the activity view. No egui types.

use std::collections::HashMap;

use doene_core::i18n::tr;
use doene_core::recurrence::{RawRecurrence, TYPES};
use doene_core::{Date, Importance, RecurrenceRule, TaskStatus};
use doene_service::{Service, ServiceError, StepPatch, TaskPatch};

/// Drawer sections in default order (ids match the `section.*` locale keys).
pub const SECTIONS: [&str; 8] =
    ["customize", "tags", "subtasks", "steps", "refs", "referencedBy", "notes", "completion"];

/// The pref scope holding a task's section order.
pub fn pref_scope(task_id: &str) -> String {
    format!("drawer:{task_id}")
}

/// Parse a stored section order: unknown/duplicate ids dropped, missing ones
/// appended in default order. Bad JSON yields the default order.
pub fn parse_order(stored: Option<&str>) -> Vec<String> {
    let saved: Vec<String> = stored.and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default();
    let mut out: Vec<String> = vec![];
    for id in saved.iter().map(String::as_str).chain(SECTIONS) {
        if SECTIONS.contains(&id) && !out.iter().any(|o| o == id) {
            out.push(id.to_owned());
        }
    }
    out
}

/// Load a task's section order from prefs.
pub fn load_order(service: &Service, task_id: &str) -> Vec<String> {
    parse_order(service.pref(&pref_scope(task_id)).ok().flatten().as_deref())
}

/// Move the section at `index` by `delta` (clamped) and persist; returns the new order.
pub fn move_section(
    service: &Service,
    task_id: &str,
    index: usize,
    delta: i32,
) -> Result<Vec<String>, ServiceError> {
    let mut order = load_order(service, task_id);
    let to = (index as i32 + delta).clamp(0, order.len() as i32 - 1) as usize;
    if index < order.len() && to != index {
        let s = order.remove(index);
        order.insert(to, s);
        service.set_pref(&pref_scope(task_id), &serde_json::to_string(&order).unwrap_or_default())?;
    }
    Ok(order)
}

/// Editable recurrence state (the editor's draft).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecurrenceDraft {
    /// `None` = does not repeat; else one of [`TYPES`].
    pub kind: Option<&'static str>,
    pub interval: u32,
    /// `day | week | month | year` (custom only).
    pub unit: &'static str,
    /// Custom weekly weekday set, dow 0 = Sunday … 6.
    pub days: Vec<u8>,
}

impl Default for RecurrenceDraft {
    fn default() -> Self {
        Self { kind: None, interval: 1, unit: "week", days: vec![] }
    }
}

const UNITS: [&str; 4] = ["day", "week", "month", "year"];

impl RecurrenceDraft {
    pub fn from_rule(rule: Option<&RecurrenceRule>) -> Self {
        let Some(r) = rule else { return Self::default() };
        let raw = RawRecurrence::from(r.clone());
        let kind = TYPES.iter().copied().find(|t| Some(*t) == raw.kind.as_deref());
        let unit = UNITS.iter().copied().find(|u| Some(*u) == raw.unit.as_deref()).unwrap_or("week");
        let mut days: Vec<u8> = raw.days.unwrap_or_default().iter().filter_map(|d| d.parse().ok()).collect();
        days.sort_unstable();
        Self { kind, interval: raw.interval.map_or(1, |i| i as u32), unit, days }
    }

    /// Toggle a weekday in the custom set.
    pub fn toggle_day(&mut self, dow: u8) {
        match self.days.iter().position(|d| *d == dow) {
            Some(i) => {
                self.days.remove(i);
            }
            None => {
                self.days.push(dow);
                self.days.sort_unstable();
            }
        }
    }

    /// The service value: `None` = clear.
    pub fn to_raw(&self) -> Option<RawRecurrence> {
        let kind = self.kind?;
        let custom = kind == "custom";
        Some(RawRecurrence {
            kind: Some(kind.to_owned()),
            interval: custom.then_some(f64::from(self.interval.max(1))),
            unit: custom.then(|| self.unit.to_owned()),
            days: (custom && self.unit == "week" && !self.days.is_empty())
                .then(|| self.days.iter().map(u8::to_string).collect()),
        })
    }
}

/// Localised message for a service error (by code and params).
pub fn error_message(e: &ServiceError, locale: &str) -> String {
    match e {
        ServiceError::Coded(c) => {
            let p: HashMap<String, Option<String>> =
                c.params.iter().map(|(k, v)| ((*k).to_owned(), Some(v.clone()))).collect();
            tr(c.code, &p, Some(locale))
        }
        other => other.to_string(),
    }
}

/// One field edit from the drawer.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldEdit {
    /// Empty text clears.
    Notes(String),
    /// `YYYY-MM-DD`; empty clears.
    Due(String),
    /// `YYYY-MM-DDTHH:MM` (or with a space); empty clears.
    Reminder(String),
    Important(bool),
    Recurrence(RecurrenceDraft),
}

fn valid_reminder(s: &str) -> Option<String> {
    let s = s.trim().replacen(' ', "T", 1);
    let (d, t) = s.split_once('T')?;
    Date::parse(d).ok()?;
    let (h, m) = t.split_once(':')?;
    let ok = h.len() == 2 && m.len() == 2 && h.parse::<u8>().is_ok_and(|h| h < 24) && m.parse::<u8>().is_ok_and(|m| m < 60);
    ok.then_some(s)
}

/// Apply a field edit through the service; `Err` is a localised message.
pub fn apply_edit(service: &Service, id: &str, edit: FieldEdit, locale: &str) -> Result<(), String> {
    let bad = |key: &str| tr(key, &HashMap::new(), Some(locale));
    let mut patch = TaskPatch::default();
    match edit {
        FieldEdit::Notes(s) => patch.body = Some(if s.trim().is_empty() { None } else { Some(s) }),
        FieldEdit::Due(s) if s.trim().is_empty() => patch.due_date = Some(None),
        FieldEdit::Due(s) => {
            patch.due_date = Some(Some(Date::parse(s.trim()).map_err(|_| bad("error.invalidDate"))?))
        }
        FieldEdit::Reminder(s) if s.trim().is_empty() => patch.reminder_at = Some(None),
        FieldEdit::Reminder(s) => {
            patch.reminder_at = Some(Some(valid_reminder(&s).ok_or_else(|| bad("error.invalidReminder"))?))
        }
        FieldEdit::Important(on) => {
            patch.importance = Some(if on { Importance::High } else { Importance::Normal })
        }
        FieldEdit::Recurrence(d) => patch.recurrence = Some(d.to_raw()),
    }
    service.update_task(id, patch).map(|_| ()).map_err(|e| error_message(&e, locale))
}

/// One step action from the drawer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepEdit {
    Add(String),
    /// Flip open/completed.
    Toggle(String),
    Delete(String),
    /// Turn the step into a real subtask.
    Promote(String),
}

/// Apply a step action to `task_id`; `Err` is a localised message.
pub fn apply_step(service: &Service, task_id: &str, edit: StepEdit, locale: &str) -> Result<(), String> {
    let r = match edit {
        StepEdit::Add(s) => service.create_step(task_id, &s).map(|_| ()),
        StepEdit::Toggle(id) => service.task(task_id).and_then(|h| {
            let cur = h.steps.iter().find(|s| s.id == id).map(|s| s.status);
            let next = if cur == Some(TaskStatus::Completed) { TaskStatus::Open } else { TaskStatus::Completed };
            service.update_step(&id, StepPatch { status: Some(next), ..Default::default() }).map(|_| ())
        }),
        StepEdit::Delete(id) => service.delete_step(&id),
        StepEdit::Promote(id) => service.step_to_task(&id).map(|_| ()),
    };
    r.map_err(|e| error_message(&e, locale))
}

/// One tag action from the drawer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagEdit {
    /// Raw input; normalised like Gör't (lowercase, no `#`/`@`, spaces to `-`).
    Add(String),
    Remove(String),
}

/// The task's tags without `#`, sorted.
pub fn tag_names(service: &Service, task_id: &str) -> Vec<String> {
    service.task(task_id).map(|h| h.tags.iter().map(|t| t.as_str().to_owned()).collect()).unwrap_or_default()
}

/// Tags in use on any task and not yet on `task_id`, for the add-tag suggestions.
pub fn tag_suggestions(service: &Service, task_id: &str) -> Vec<String> {
    let have = tag_names(service, task_id);
    let mut all: Vec<String> = service
        .all_tasks()
        .unwrap_or_default()
        .iter()
        .flat_map(|h| h.tags.iter().map(|t| t.as_str().to_owned()))
        .filter(|t| !have.contains(t))
        .collect();
    all.sort();
    all.dedup();
    all
}

/// Apply a tag action through the service (logged, undoable via update).
pub fn apply_tag(service: &Service, task_id: &str, edit: TagEdit, locale: &str) -> Result<(), String> {
    let mut tags = tag_names(service, task_id);
    match edit {
        TagEdit::Add(raw) => tags.push(raw),
        TagEdit::Remove(t) => tags.retain(|x| *x != t),
    }
    let patch = TaskPatch { tags: Some(tags), ..Default::default() };
    service.update_task(task_id, patch).map(|_| ()).map_err(|e| error_message(&e, locale))
}

/// A step as shown in the drawer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepRow {
    pub id: String,
    pub subject: String,
    pub done: bool,
}

/// The task's steps in position order.
pub fn step_rows(service: &Service, task_id: &str) -> Vec<StepRow> {
    service
        .task(task_id)
        .map(|h| {
            h.steps
                .into_iter()
                .map(|s| StepRow { done: s.status == TaskStatus::Completed, id: s.id, subject: s.subject })
                .collect()
        })
        .unwrap_or_default()
}

/// A node of the subtask tree under a task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtaskNode {
    pub id: String,
    pub subject: String,
    pub done: bool,
    pub depth: usize,
}

/// The task's descendants, depth-first in position order (renderable tree).
pub fn subtask_tree(service: &Service, task_id: &str) -> Vec<SubtaskNode> {
    let all = service.all_tasks().unwrap_or_default();
    let mut kids: HashMap<&str, Vec<&doene_core::Task>> = HashMap::new();
    for h in &all {
        if let Some(p) = h.task.parent_id.as_deref() {
            kids.entry(p).or_default().push(&h.task);
        }
    }
    for v in kids.values_mut() {
        v.sort_by(|a, b| a.position.total_cmp(&b.position));
    }
    let mut out = vec![];
    let mut seen = std::collections::HashSet::from([task_id.to_owned()]);
    let mut stack: Vec<(&str, usize)> = vec![(task_id, 0)];
    // Iterative DFS; the seen-set guards against any corrupt cycle.
    while let Some((id, depth)) = stack.pop() {
        if depth > 0 {
            let t = all.iter().find(|h| h.task.id == id).map(|h| &h.task);
            if let Some(t) = t {
                out.push(SubtaskNode {
                    id: t.id.clone(),
                    subject: t.subject.clone(),
                    done: t.status == TaskStatus::Completed,
                    depth: depth - 1,
                });
            }
        }
        for k in kids.get(id).into_iter().flatten().rev() {
            if seen.insert(k.id.clone()) {
                stack.push((&k.id, depth + 1));
            }
        }
    }
    out
}

/// Create a full task under `task_id` (quick subtask add).
pub fn add_subtask(service: &Service, task_id: &str, subject: &str, locale: &str) -> Result<(), String> {
    service
        .create_task(doene_service::NewTask {
            subject: subject.to_owned(),
            parent_id: Some(task_id.to_owned()),
            ..Default::default()
        })
        .map(|_| ())
        .map_err(|e| error_message(&e, locale))
}

/// Nest `child` as the last subtask of `parent` (`None` = top level); a cycle
/// or self-nesting is rejected with a localised message and nothing changes.
pub fn nest_task(service: &Service, child: &str, parent: Option<&str>, locale: &str) -> Result<(), String> {
    let at = service
        .all_tasks()
        .map(|a| a.iter().filter(|h| h.task.parent_id.as_deref() == parent && h.task.id != child).count())
        .unwrap_or(0);
    service.place_task(child, parent, at).map(|_| ()).map_err(|e| error_message(&e, locale))
}

/// One activity line: local-ish timestamp, action, detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityLine {
    /// `YYYY-MM-DD HH:MM` (UTC).
    pub at: String,
    pub action: String,
    pub detail: Option<String>,
}

/// The task's activity log, newest first.
pub fn activity_lines(service: &Service, id: &str) -> Vec<ActivityLine> {
    service
        .activity(id)
        .unwrap_or_default()
        .into_iter()
        .map(|a| ActivityLine {
            at: a.at.get(..16).unwrap_or(&a.at).replace('T', " "),
            action: a.action,
            detail: a.detail,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use doene_storage::Storage;

    fn svc() -> (Service, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage::open(&dir.path().join("d.db")).unwrap();
        (Service::with_clock(storage, Box::new(|| "2025-03-10T09:00:00.000Z".into())), dir)
    }

    fn task(s: &Service) -> String {
        s.quick_add("write report", None, true).unwrap().task.id
    }

    #[test]
    fn tags_add_normalise_remove_and_undo() {
        let (s, _d) = svc();
        let id = task(&s);
        apply_tag(&s, &id, TagEdit::Add("  #Home Office ".into()), "en").unwrap();
        apply_tag(&s, &id, TagEdit::Add("home-office".into()), "en").unwrap();
        apply_tag(&s, &id, TagEdit::Add("@Work".into()), "en").unwrap();
        apply_tag(&s, &id, TagEdit::Add("   ".into()), "en").unwrap();
        assert_eq!(tag_names(&s, &id), ["home-office", "work"]);
        let other = s.quick_add("other", None, true).unwrap().task.id;
        assert_eq!(tag_suggestions(&s, &other), ["home-office", "work"]);
        assert!(tag_suggestions(&s, &id).is_empty());
        apply_tag(&s, &id, TagEdit::Remove("work".into()), "en").unwrap();
        assert_eq!(tag_names(&s, &id), ["home-office"]);
        s.undo().unwrap();
        assert_eq!(tag_names(&s, &id), ["home-office", "work"]);
    }

    #[test]
    fn fields_persist_and_clear() {
        let (s, _d) = svc();
        let id = task(&s);
        apply_edit(&s, &id, FieldEdit::Notes("hello".into()), "en").unwrap();
        apply_edit(&s, &id, FieldEdit::Due("2025-04-01".into()), "en").unwrap();
        apply_edit(&s, &id, FieldEdit::Reminder("2025-04-01 08:30".into()), "en").unwrap();
        apply_edit(&s, &id, FieldEdit::Important(true), "en").unwrap();
        let t = s.task(&id).unwrap().task;
        assert_eq!(t.body.as_deref(), Some("hello"));
        assert_eq!(t.due_date, Some(Date::parse("2025-04-01").unwrap()));
        assert_eq!(t.reminder_at.as_deref(), Some("2025-04-01T08:30"));
        assert_eq!(t.importance, Importance::High);
        apply_edit(&s, &id, FieldEdit::Notes("  ".into()), "en").unwrap();
        apply_edit(&s, &id, FieldEdit::Due("".into()), "en").unwrap();
        apply_edit(&s, &id, FieldEdit::Reminder("".into()), "en").unwrap();
        let t = s.task(&id).unwrap().task;
        assert_eq!((t.body, t.due_date, t.reminder_at), (None, None, None));
    }

    #[test]
    fn invalid_input_gives_localised_error_and_changes_nothing() {
        let (s, _d) = svc();
        let id = task(&s);
        assert_eq!(apply_edit(&s, &id, FieldEdit::Due("2025-02-30".into()), "en").unwrap_err(), "invalid date (use YYYY-MM-DD)");
        assert_eq!(
            apply_edit(&s, &id, FieldEdit::Reminder("tomorrow".into()), "sv").unwrap_err(),
            "ogiltig påminnelse (använd ÅÅÅÅ-MM-DD HH:MM)"
        );
        assert_eq!(s.task(&id).unwrap().task.due_date, None);
    }

    #[test]
    fn edit_is_undoable() {
        let (s, _d) = svc();
        let id = task(&s);
        apply_edit(&s, &id, FieldEdit::Important(true), "en").unwrap();
        s.undo().unwrap();
        assert_eq!(s.task(&id).unwrap().task.importance, Importance::Normal);
    }

    #[test]
    fn recurrence_editor_covers_all_six_types() {
        let (s, _d) = svc();
        let id = task(&s);
        for kind in ["daily", "weekdays", "weekly", "monthly", "yearly"] {
            let d = RecurrenceDraft { kind: TYPES.iter().copied().find(|t| *t == kind), ..Default::default() };
            apply_edit(&s, &id, FieldEdit::Recurrence(d.clone()), "en").unwrap();
            let got = s.task(&id).unwrap().task.recurrence;
            assert_eq!(RecurrenceDraft::from_rule(got.as_ref()).kind, Some(d.kind.unwrap()));
        }
        // custom: every 2 weeks on Mon + Fri
        let mut d = RecurrenceDraft { kind: Some("custom"), interval: 2, unit: "week", days: vec![] };
        d.toggle_day(5);
        d.toggle_day(1);
        d.toggle_day(3);
        d.toggle_day(3);
        apply_edit(&s, &id, FieldEdit::Recurrence(d.clone()), "en").unwrap();
        let got = s.task(&id).unwrap().task.recurrence;
        assert_eq!(RecurrenceDraft::from_rule(got.as_ref()), RecurrenceDraft { days: vec![1, 5], ..d });
        // custom monthly interval drops weekdays
        let m = RecurrenceDraft { kind: Some("custom"), interval: 3, unit: "month", days: vec![2] };
        apply_edit(&s, &id, FieldEdit::Recurrence(m), "en").unwrap();
        let got = RecurrenceDraft::from_rule(s.task(&id).unwrap().task.recurrence.as_ref());
        assert_eq!((got.interval, got.unit, got.days.len()), (3, "month", 0));
        // none clears
        apply_edit(&s, &id, FieldEdit::Recurrence(RecurrenceDraft::default()), "en").unwrap();
        assert_eq!(s.task(&id).unwrap().task.recurrence, None);
    }

    #[test]
    fn activity_view_lists_field_changes_newest_first() {
        let (s, _d) = svc();
        let id = task(&s);
        apply_edit(&s, &id, FieldEdit::Due("2025-04-01".into()), "en").unwrap();
        let lines = activity_lines(&s, &id);
        assert!(lines.len() >= 2);
        assert_eq!(lines.last().unwrap().action, "created");
        assert!(lines.iter().any(|l| l.action == "due_date changed" && l.at == "2025-03-10 09:00"));
    }

    #[test]
    fn section_order_persists_per_task() {
        let (s, _d) = svc();
        let a = task(&s);
        let b = s.quick_add("other", None, true).unwrap().task.id;
        assert_eq!(load_order(&s, &a), SECTIONS);
        let moved = move_section(&s, &a, 6, -6).unwrap();
        assert_eq!(moved[0], "notes");
        assert_eq!(load_order(&s, &a)[..2], ["notes", "customize"]);
        assert_eq!(load_order(&s, &b), SECTIONS);
        // moving past the edge is a no-op
        assert_eq!(move_section(&s, &a, 0, -1).unwrap()[0], "notes");
    }

    #[test]
    fn stored_order_is_sanitised() {
        let o = parse_order(Some(r#"["notes","bogus","notes","tags"]"#));
        assert_eq!(o.len(), SECTIONS.len());
        assert_eq!(o[..2], ["notes", "tags"]);
        assert_eq!(parse_order(Some("not json")), SECTIONS);
    }

    #[test]
    fn steps_add_tick_delete_and_promote() {
        let (s, _d) = svc();
        let id = task(&s);
        for n in ["a", "b", "c"] {
            apply_step(&s, &id, StepEdit::Add(n.into()), "en").unwrap();
        }
        assert_eq!(apply_step(&s, &id, StepEdit::Add(" ".into()), "en").unwrap_err(), "subject required");
        let rows = step_rows(&s, &id);
        assert_eq!(rows.iter().map(|r| r.subject.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
        apply_step(&s, &id, StepEdit::Toggle(rows[0].id.clone()), "en").unwrap();
        assert!(step_rows(&s, &id)[0].done);
        apply_step(&s, &id, StepEdit::Toggle(rows[0].id.clone()), "en").unwrap();
        assert!(!step_rows(&s, &id)[0].done);
        apply_step(&s, &id, StepEdit::Delete(rows[1].id.clone()), "en").unwrap();
        apply_step(&s, &id, StepEdit::Promote(rows[2].id.clone()), "en").unwrap();
        assert_eq!(step_rows(&s, &id).len(), 1);
        let kids: Vec<_> = s.all_tasks().unwrap().into_iter().filter(|t| t.task.parent_id.as_deref() == Some(&id)).collect();
        assert_eq!(kids.len(), 1);
        assert_eq!(kids[0].task.subject, "c");
        s.undo().unwrap();
        assert_eq!(step_rows(&s, &id).len(), 2);
    }

    #[test]
    fn subtask_tree_nests_and_rejects_cycles_with_feedback() {
        let (s, _d) = svc();
        let a = task(&s);
        add_subtask(&s, &a, "b", "en").unwrap();
        let b = subtask_tree(&s, &a)[0].id.clone();
        add_subtask(&s, &b, "c", "en").unwrap();
        add_subtask(&s, &a, "d", "en").unwrap();
        let tree: Vec<_> = subtask_tree(&s, &a).into_iter().map(|n| (n.subject, n.depth)).collect();
        assert_eq!(tree, [("b".to_owned(), 0), ("c".to_owned(), 1), ("d".to_owned(), 0)]);
        // cycle: a under its own grandchild; and self
        let c = subtask_tree(&s, &a)[1].id.clone();
        assert_eq!(nest_task(&s, &a, Some(&c), "en").unwrap_err(), "subtasks cannot form a cycle");
        assert_eq!(nest_task(&s, &b, Some(&b), "sv").unwrap_err(), "en uppgift kan inte vara sin egen deluppgift");
        assert_eq!(subtask_tree(&s, &a).len(), 3);
        // un-nest c to top level
        nest_task(&s, &c, None, "en").unwrap();
        assert_eq!(subtask_tree(&s, &a).len(), 2);
    }

    #[test]
    fn deleting_parent_snapshots_subtree_into_trash_and_restores() {
        let (s, _d) = svc();
        let a = task(&s);
        add_subtask(&s, &a, "b", "en").unwrap();
        let b = subtask_tree(&s, &a)[0].id.clone();
        add_subtask(&s, &b, "c", "en").unwrap();
        let entry = s.delete_task(&a).unwrap();
        assert!(s.all_tasks().unwrap().is_empty());
        assert_eq!(s.trash().unwrap().len(), 1);
        s.restore(&entry).unwrap();
        assert_eq!(subtask_tree(&s, &a).len(), 2);
    }
}
