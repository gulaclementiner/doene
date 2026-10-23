use std::time::{SystemTime, UNIX_EPOCH};

use doene_core::quickadd::{QuickAddOptions, QuickAddView, quick_add};
use doene_core::recurrence::{RawRecurrence, next_on_or_after, validate};
use doene_core::{
    CodedError, Date, RawView, View, ViewRules, normalise_view, FocusMode, HydratedTask, Importance, Step, Tag, Task, TaskStatus, TrashKind,
    nlp::ImportanceLevel, search_tasks, tree,
};
use doene_storage::aux::{self, ActivityRow, TrashRow};
use doene_storage::views::{self, SavedView};
use doene_storage::{Storage, tasks};
use rusqlite::Connection;
use serde_json::{Value, json};

use crate::snapshot::{task_from_json, task_json, view_from_json, view_json};

/// Days a trashed entry is kept before it is purged.
pub const TRASH_DAYS: i64 = 30;

/// Source of "now" as an ISO-8601 UTC timestamp (`2025-03-01T09:00:00.000Z`).
pub type Clock = Box<dyn Fn() -> String + Send>;

/// Service errors: a coded domain error, or a storage failure.
#[derive(Debug)]
pub enum ServiceError {
    /// A structured `{code, params}` error (Gör't's coded errors).
    Coded(CodedError),
    /// SQLite failure.
    Storage(rusqlite::Error),
}

impl ServiceError {
    /// The error code, when this is a coded domain error.
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::Coded(c) => Some(c.code),
            Self::Storage(_) => None,
        }
    }
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Coded(c) => c.fmt(f),
            Self::Storage(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ServiceError {}

impl From<CodedError> for ServiceError {
    fn from(e: CodedError) -> Self {
        Self::Coded(e)
    }
}

impl From<rusqlite::Error> for ServiceError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Storage(e)
    }
}

type Res<T> = Result<T, ServiceError>;

fn not_found(what: &'static str, label: &str) -> ServiceError {
    ServiceError::Coded(CodedError {
        code: "error.notFound",
        message: format!("{label} not found"),
        params: vec![("what", what.to_owned())],
    })
}

fn coded(code: &'static str, message: &str) -> ServiceError {
    CodedError::new(code, message).into()
}

/// Fields for a new task (Gör't `createTask` body).
#[derive(Debug, Clone, Default)]
pub struct NewTask {
    pub subject: String,
    pub importance: Option<Importance>,
    pub due_date: Option<Date>,
    pub reminder_at: Option<String>,
    pub body: Option<String>,
    pub recurrence: Option<RawRecurrence>,
    /// Pick the task for today.
    pub shortlisted: bool,
    pub parent_id: Option<String>,
    /// Raw tag words; normalised (lowercase, no `#`/`@`) and deduplicated.
    pub tags: Vec<String>,
}

/// A partial task update (Gör't `updateTask` body). `None` = leave alone;
/// nested `Option` fields use `Some(None)` to clear.
#[derive(Debug, Clone, Default)]
pub struct TaskPatch {
    pub subject: Option<String>,
    pub status: Option<TaskStatus>,
    pub importance: Option<Importance>,
    pub due_date: Option<Option<Date>>,
    pub reminder_at: Option<Option<String>>,
    pub body: Option<Option<String>>,
    pub position: Option<f64>,
    pub recurrence: Option<Option<RawRecurrence>>,
    pub parent_id: Option<Option<String>>,
    pub shortlisted: Option<bool>,
    pub tags: Option<Vec<String>>,
}

/// A partial step update.
#[derive(Debug, Clone, Default)]
pub struct StepPatch {
    pub subject: Option<String>,
    pub status: Option<TaskStatus>,
    pub position: Option<f64>,
}

/// Result of [`Service::update_task`]: the task, plus the next occurrence when
/// completing a repeating task spawned one.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateOutcome {
    pub task: HydratedTask,
    pub next: Option<HydratedTask>,
}

/// A trash list entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashItem {
    pub id: String,
    pub kind: TrashKind,
    pub label: Option<String>,
    pub deleted_at: String,
}

/// What a restore brought back.
#[derive(Debug, Clone, PartialEq)]
pub enum Restored {
    Task(HydratedTask),
    View { id: String },
}

/// Persisted settings used by task operations (Gör't defaults).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub add_tasks_to_top: bool,
    pub focus_mode: FocusMode,
    pub language: String,
    /// `light`, `dark` or `system` (default).
    pub theme: String,
}

struct Ctx {
    now: String,
    today: Date,
}

/// The application service over one database.
pub struct Service {
    storage: Storage,
    clock: Clock,
    undo: std::sync::Mutex<Vec<UndoEntry>>,
}

/// Most recent mutations kept for undo.
const UNDO_DEPTH: usize = 50;

/// What kind of mutation an undo entry reverses (drives the toast wording).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndoKind {
    Create,
    Edit,
    Delete,
    Move,
    Restore,
    Trash,
}

/// Plain-data state before a mutation: every task (with tags and steps) and the trash.
/// Undo is generic: any `undoable` mutation is reversed by restoring this state,
/// so new operations only need to run through [`Service::tx_undoable`].
struct UndoEntry {
    kind: UndoKind,
    tasks: Vec<HydratedTask>,
    trash: Vec<TrashRow>,
    views: Vec<SavedView>,
}

fn system_now() -> String {
    let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = d.as_secs();
    let date = Date { year: 1970, month: 1, day: 1 }.add_days((secs / 86_400) as i64);
    let rem = secs % 86_400;
    format!(
        "{date}T{:02}:{:02}:{:02}.{:03}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60,
        d.subsec_millis()
    )
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn normalise_tag(raw: &str) -> Option<Tag> {
    let w = raw.trim().to_lowercase();
    let w = w.trim_start_matches(['#', '@']);
    let w = w.split_whitespace().collect::<Vec<_>>().join("-");
    Tag::new(&w).ok()
}

fn saved_from_rules(id: String, r: ViewRules, position: f64) -> SavedView {
    SavedView {
        view: View {
            id,
            name: r.name,
            emoji: r.emoji.filter(|e| !e.trim().is_empty()),
            match_mode: r.match_mode,
            show_done: r.show_done,
            position,
            tags: normalise_tags(&r.tags),
        },
        sorts: r.sorts,
    }
}

fn normalise_tags(raw: &[String]) -> Vec<Tag> {
    let mut tags: Vec<Tag> = raw.iter().filter_map(|t| normalise_tag(t)).collect();
    tags.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    tags.dedup();
    tags
}

fn tag_list(tags: &[Tag]) -> String {
    if tags.is_empty() {
        "none".into()
    } else {
        tags.iter().map(Tag::as_str).collect::<Vec<_>>().join(", ")
    }
}

fn log(conn: &Connection, ctx: &Ctx, task: Option<&str>, action: &str, detail: Option<&str>) -> Res<()> {
    aux::insert_activity(conn, &new_id(), task, &ctx.now, action, detail)?;
    Ok(())
}

fn settings_in(conn: &Connection) -> Res<Settings> {
    let get = |k: &str| -> Res<Option<Value>> {
        Ok(aux::setting(conn, k)?.and_then(|s| serde_json::from_str(&s).ok()))
    };
    Ok(Settings {
        add_tasks_to_top: get("addTasksToTop")?.and_then(|v| v.as_bool()).unwrap_or(false),
        focus_mode: match get("focusMode")?.as_ref().and_then(Value::as_str) {
            Some("durable") => FocusMode::Durable,
            _ => FocusMode::Daily,
        },
        language: get("language")?
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| "en".into()),
        theme: get("theme")?
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| "system".into()),
    })
}

fn need(conn: &Connection, id: &str) -> Res<HydratedTask> {
    tasks::hydrated(conn, id)?.ok_or_else(|| not_found("what.task", "task"))
}

/// Gör't `#checkParent`: not itself, must exist, no cycle.
fn check_parent(conn: &Connection, task_id: Option<&str>, parent: Option<&str>) -> Res<()> {
    let Some(parent) = parent else { return Ok(()) };
    if Some(parent) == task_id {
        return Err(coded("error.ownSubtask", "a task cannot be its own subtask"));
    }
    if tasks::task(conn, parent)?.is_none() {
        return Err(not_found("what.parentTask", "parent task"));
    }
    if let Some(id) = task_id {
        if !tree::can_nest(id, parent, &tasks::parent_map(conn)?) {
            return Err(coded("error.subtaskCycle", "subtasks cannot form a cycle"));
        }
    }
    Ok(())
}

fn importance_of(level: ImportanceLevel) -> Importance {
    // The schema stores only normal/high; the parser's `!low` has no stored form.
    if level == ImportanceLevel::High { Importance::High } else { Importance::Normal }
}

fn set_tags_logged(conn: &Connection, ctx: &Ctx, id: &str, raw: &[String]) -> Res<()> {
    let next = normalise_tags(raw);
    let before = tasks::tags_of(conn, id)?;
    if before == next {
        return Ok(());
    }
    tasks::set_tags(conn, id, &next)?;
    log(conn, ctx, Some(id), "tags changed", Some(&format!("{} -> {}", tag_list(&before), tag_list(&next))))
}

fn edge_position(conn: &Connection, parent: Option<&str>, top: bool) -> Res<f64> {
    let sib = tasks::siblings(conn, parent)?;
    Ok(if top {
        sib.first().map_or(0.0, |s| s.1 - 1.0)
    } else {
        sib.last().map_or(0.0, |s| s.1) + 1.0
    })
}

fn create_in(conn: &Connection, ctx: &Ctx, n: NewTask) -> Res<HydratedTask> {
    if n.subject.trim().is_empty() {
        return Err(coded("error.subjectRequired", "subject required"));
    }
    check_parent(conn, None, n.parent_id.as_deref())?;
    let recurrence = validate(n.recurrence.as_ref())?;
    let due_date = if recurrence.is_some() { n.due_date.or(Some(ctx.today)) } else { n.due_date };
    let top = settings_in(conn)?.add_tasks_to_top;
    let id = new_id();
    let task = Task {
        id: id.clone(),
        subject: n.subject.trim().to_owned(),
        status: TaskStatus::Open,
        importance: n.importance.unwrap_or(Importance::Normal),
        due_date,
        reminder_at: n.reminder_at,
        body: n.body,
        focus_date: None,
        postponed_date: None,
        created_at: ctx.now.clone(),
        completed_at: None,
        position: edge_position(conn, n.parent_id.as_deref(), top)?,
        spawned_from: None,
        parent_id: n.parent_id,
        recurrence,
    };
    tasks::insert_task(conn, &task)?;
    if !n.tags.is_empty() {
        set_tags_logged(conn, ctx, &id, &n.tags)?;
    }
    if n.shortlisted {
        set_focus(conn, &id, Some(ctx.today))?;
    }
    log(conn, ctx, Some(&id), "created", None)?;
    need(conn, &id)
}

fn set_focus(conn: &Connection, id: &str, date: Option<Date>) -> Res<()> {
    let mut t = tasks::task(conn, id)?.ok_or_else(|| not_found("what.task", "task"))?;
    t.focus_date = date;
    tasks::update_task(conn, &t)?;
    Ok(())
}

fn token(s: TaskStatus) -> &'static str {
    if s == TaskStatus::Completed { "completed" } else { "open" }
}

fn show<T: ToString>(v: Option<T>) -> String {
    v.map(|v| v.to_string()).unwrap_or_default()
}

fn update_in(conn: &Connection, ctx: &Ctx, id: &str, p: TaskPatch) -> Res<UpdateOutcome> {
    let cur = need(conn, id)?.task;
    let mut t = cur.clone();
    if let Some(raw) = &p.recurrence {
        t.recurrence = validate(raw.as_ref())?;
        if t.recurrence.is_some() && cur.due_date.is_none() && p.due_date.is_none() {
            t.due_date = Some(ctx.today);
        }
    }
    if let Some(parent) = &p.parent_id {
        check_parent(conn, Some(id), parent.as_deref())?;
        if *parent != cur.parent_id {
            log(
                conn,
                ctx,
                Some(id),
                "subtask parent changed",
                Some(&format!(
                    "{} -> {}",
                    cur.parent_id.as_deref().unwrap_or("none"),
                    parent.as_deref().unwrap_or("none")
                )),
            )?;
        }
        t.parent_id = parent.clone();
    }
    let field_logged = p.subject.is_some() || p.importance.is_some() || p.due_date.is_some() || p.reminder_at.is_some();
    let changed = |field: &str, old: String, new: String| -> Res<()> {
        if old != new {
            log(conn, ctx, Some(id), &format!("{field} changed"), Some(&format!("{old} -> {new}")))?;
        }
        Ok(())
    };
    if let Some(v) = p.subject {
        changed("subject", cur.subject.clone(), v.clone())?;
        t.subject = v;
    }
    if let Some(v) = p.importance {
        changed("importance", format!("{:?}", cur.importance).to_lowercase(), format!("{v:?}").to_lowercase())?;
        t.importance = v;
    }
    if let Some(v) = p.due_date {
        changed("due_date", show(cur.due_date), show(v))?;
        t.due_date = v;
    }
    if let Some(v) = p.reminder_at {
        changed("reminder_at", show(cur.reminder_at.clone()), show(v.clone()))?;
        t.reminder_at = v;
    }
    if let Some(v) = p.body {
        t.body = v;
    }
    if let Some(v) = p.position {
        t.position = v;
    }
    tasks::update_task(conn, &t)?;
    let mut mutated = t != cur;
    match p.shortlisted {
        Some(true) => {
            set_focus(conn, id, Some(ctx.today))?;
            mutated |= cur.focus_date != Some(ctx.today);
        }
        Some(false) => {
            set_focus(conn, id, None)?;
            mutated |= cur.focus_date.is_some();
        }
        None => {}
    }
    if let Some(tags) = &p.tags {
        set_tags_logged(conn, ctx, id, tags)?;
    }
    if let Some(status) = p.status {
        if status != cur.status {
            return set_status(conn, ctx, id, status);
        }
    }
    // A change not covered by a field log line (body, position, recurrence, focus) still
    // leaves a trace: the activity log records every mutation.
    if mutated && !field_logged {
        log(conn, ctx, Some(id), "updated", None)?;
    }
    Ok(UpdateOutcome { task: need(conn, id)?, next: None })
}

fn set_status(conn: &Connection, ctx: &Ctx, id: &str, status: TaskStatus) -> Res<UpdateOutcome> {
    let cur = need(conn, id)?;
    let mut out_next = None;
    if status == TaskStatus::Completed {
        for d in tasks::descendants(conn, id)? {
            let mut dt = need(conn, &d)?.task;
            if dt.status != TaskStatus::Completed {
                dt.status = TaskStatus::Completed;
                dt.completed_at = Some(ctx.now.clone());
                tasks::update_task(conn, &dt)?;
                log(conn, ctx, Some(&d), "completed", Some(&format!("with parent {}", cur.task.subject)))?;
            }
        }
        let mut t = cur.task.clone();
        t.status = TaskStatus::Completed;
        t.completed_at = Some(ctx.now.clone());
        tasks::update_task(conn, &t)?;
        log(conn, ctx, Some(id), "completed", None)?;
        if let Some(rule) = &cur.task.recurrence {
            let from = cur.task.due_date.unwrap_or(ctx.today);
            let due = next_on_or_after(from, rule, ctx.today);
            let next = create_in(
                conn,
                ctx,
                NewTask {
                    subject: cur.task.subject.clone(),
                    importance: Some(cur.task.importance),
                    due_date: Some(due),
                    body: cur.task.body.clone(),
                    recurrence: Some(rule.clone().into()),
                    ..Default::default()
                },
            )?;
            let mut n = next.task.clone();
            n.spawned_from = Some(id.to_owned());
            tasks::update_task(conn, &n)?;
            for s in &cur.steps {
                tasks::insert_step(conn, &Step {
                    id: new_id(),
                    task_id: n.id.clone(),
                    subject: s.subject.clone(),
                    status: TaskStatus::Open,
                    position: s.position,
                })?;
            }
            out_next = Some(need(conn, &n.id)?);
        }
    } else {
        let mut t = cur.task.clone();
        t.status = status;
        t.completed_at = None;
        tasks::update_task(conn, &t)?;
        log(conn, ctx, Some(id), "reopened", None)?;
        if cur.task.status == TaskStatus::Completed {
            // Undo the completion: remove the occurrence it spawned (and its subtree).
            let spawned: Vec<String> = tasks::all_tasks(conn)?
                .into_iter()
                .filter(|h| h.task.spawned_from.as_deref() == Some(id))
                .map(|h| h.task.id)
                .collect();
            let mut doomed = Vec::new();
            for s in spawned {
                doomed.push(s.clone());
                doomed.extend(tasks::descendants(conn, &s)?);
            }
            tasks::delete_tasks(conn, &doomed)?;
        }
    }
    Ok(UpdateOutcome { task: need(conn, id)?, next: out_next })
}

impl Service {
    /// A service over `storage` using the system clock.
    pub fn new(storage: Storage) -> Self {
        Self::with_clock(storage, Box::new(system_now))
    }

    /// A service with an injected clock (tests).
    pub fn with_clock(storage: Storage, clock: Clock) -> Self {
        Self { storage, clock, undo: Default::default() }
    }

    fn ctx(&self) -> Ctx {
        let now = (self.clock)();
        let today = Date::parse(&now[..10]).expect("clock returns an ISO-8601 timestamp");
        Ctx { now, today }
    }

    /// Run `f` in one transaction; roll back on error.
    fn tx<T>(&self, f: impl FnOnce(&Connection, &Ctx) -> Res<T>) -> Res<T> {
        let ctx = self.ctx();
        let tx = self.storage.conn().unchecked_transaction()?;
        let out = f(&tx, &ctx)?;
        tx.commit()?;
        Ok(out)
    }

    /// Like [`Self::tx`], but records the prior state so [`Self::undo`] can reverse it.
    /// A failed mutation records nothing.
    fn tx_undoable<T>(&self, kind: UndoKind, f: impl FnOnce(&Connection, &Ctx) -> Res<T>) -> Res<T> {
        let ctx = self.ctx();
        let tx = self.storage.conn().unchecked_transaction()?;
        let entry = UndoEntry { kind, tasks: tasks::all_tasks(&tx)?, trash: aux::trash(&tx)?, views: views::all(&tx)? };
        let out = f(&tx, &ctx)?;
        tx.commit()?;
        let mut stack = self.undo.lock().unwrap_or_else(|e| e.into_inner());
        stack.push(entry);
        if stack.len() > UNDO_DEPTH {
            stack.remove(0);
        }
        Ok(out)
    }

    /// Whether there is anything to undo, and of what kind (the newest entry).
    pub fn undo_peek(&self) -> Option<UndoKind> {
        self.undo.lock().unwrap_or_else(|e| e.into_inner()).last().map(|e| e.kind)
    }

    /// Reverse the most recent undoable mutation. `None` when there is nothing to undo.
    pub fn undo(&self) -> Res<Option<UndoKind>> {
        let Some(entry) = self.undo.lock().unwrap_or_else(|e| e.into_inner()).pop() else {
            return Ok(None);
        };
        let r = self.tx(|c, ctx| {
            c.pragma_update(None, "defer_foreign_keys", true)?;
            let now = tasks::all_tasks(c)?;
            let keep: std::collections::HashSet<&str> = entry.tasks.iter().map(|h| h.task.id.as_str()).collect();
            let gone: Vec<String> = now.iter().filter(|h| !keep.contains(h.task.id.as_str())).map(|h| h.task.id.clone()).collect();
            tasks::delete_tasks(c, &gone)?;
            let cur: std::collections::HashMap<&str, &HydratedTask> = now.iter().map(|h| (h.task.id.as_str(), h)).collect();
            for h in &entry.tasks {
                match cur.get(h.task.id.as_str()) {
                    Some(c0) if **c0 == *h => {}
                    Some(c0) => {
                        tasks::update_task(c, &h.task)?;
                        if c0.tags != h.tags {
                            tasks::set_tags(c, &h.task.id, &h.tags)?;
                        }
                        if c0.steps != h.steps {
                            for s in &c0.steps {
                                tasks::delete_step(c, &s.id)?;
                            }
                            for s in &h.steps {
                                tasks::insert_step(c, s)?;
                            }
                        }
                    }
                    None => {
                        tasks::insert_task(c, &h.task)?;
                        tasks::set_tags(c, &h.task.id, &h.tags)?;
                        for s in &h.steps {
                            tasks::insert_step(c, s)?;
                        }
                    }
                }
            }
            if aux::trash(c)? != entry.trash {
                aux::empty_trash(c)?;
                for t in &entry.trash {
                    aux::insert_trash(c, t)?;
                }
            }
            if views::all(c)? != entry.views {
                for v in views::all(c)? {
                    views::delete(c, &v.view.id)?;
                }
                for v in &entry.views {
                    views::insert(c, v)?;
                }
            }
            log(c, ctx, None, "undone", None)
        });
        r.map(|()| Some(entry.kind))
    }

    /// Today (UTC) per the service clock.
    pub fn today(&self) -> Date {
        self.ctx().today
    }

    // ---- reads

    /// One task, hydrated.
    pub fn task(&self, id: &str) -> Res<HydratedTask> {
        need(self.storage.conn(), id)
    }

    /// Every task, hydrated.
    pub fn all_tasks(&self) -> Res<Vec<HydratedTask>> {
        Ok(tasks::all_tasks(self.storage.conn())?)
    }

    /// A task's activity, newest first.
    pub fn activity(&self, id: &str) -> Res<Vec<ActivityRow>> {
        Ok(aux::activity_of(self.storage.conn(), id)?)
    }

    /// Ranked search (core scoring); all words required.
    pub fn search(&self, q: &str, limit: usize) -> Res<Vec<HydratedTask>> {
        let all = self.all_tasks()?;
        Ok(search_tasks(&all, q, limit).into_iter().cloned().collect())
    }

    // ---- task mutations

    /// Create a task.
    pub fn create_task(&self, n: NewTask) -> Res<HydratedTask> {
        self.tx_undoable(UndoKind::Create, |c, ctx| create_in(c, ctx, n))
    }

    /// Quick-add: parse `text` (NLP, view defaults) and create the task.
    pub fn quick_add(&self, text: &str, view: Option<&QuickAddView>, literal: bool) -> Res<HydratedTask> {
        self.tx_undoable(UndoKind::Create, |c, ctx| {
            let opts = QuickAddOptions {
                today: ctx.today,
                views: aux::views(c)?,
                literal,
                locale: Some(settings_in(c)?.language),
            };
            let d = quick_add(text, view, &opts)?;
            create_in(
                c,
                ctx,
                NewTask {
                    subject: d.subject,
                    importance: Some(importance_of(d.importance)),
                    due_date: d.due_date,
                    reminder_at: d.reminder_at,
                    body: None,
                    recurrence: d.recurrence.map(Into::into),
                    shortlisted: d.shortlisted,
                    parent_id: None,
                    tags: d.tags.iter().map(|t| t.as_str().to_owned()).collect(),
                },
            )
        })
    }

    /// Patch a task. Completing a repeating task spawns its next occurrence.
    pub fn update_task(&self, id: &str, patch: TaskPatch) -> Res<UpdateOutcome> {
        self.tx_undoable(UndoKind::Edit, |c, ctx| update_in(c, ctx, id, patch))
    }

    /// Move a task and its subtree to the trash; returns the trash entry id.
    pub fn delete_task(&self, id: &str) -> Res<String> {
        self.tx_undoable(UndoKind::Delete, |c, ctx| {
            let t = need(c, id)?;
            let subtree: Vec<HydratedTask> = tasks::descendants(c, id)?
                .iter()
                .map(|d| need(c, d))
                .collect::<Res<_>>()?;
            let trash_id = new_id();
            let data = json!({
                "task": task_json(&t),
                "subtasks": subtree.iter().map(task_json).collect::<Vec<_>>(),
            });
            aux::insert_trash(c, &TrashRow {
                id: trash_id.clone(),
                kind: "task".into(),
                label: Some(t.task.subject.clone()),
                data: data.to_string(),
                deleted_at: ctx.now.clone(),
            })?;
            let mut ids: Vec<String> = subtree.iter().map(|h| h.task.id.clone()).collect();
            ids.push(id.to_owned());
            tasks::delete_tasks(c, &ids)?;
            log(c, ctx, Some(id), "deleted", Some(&t.task.subject))?;
            Ok(trash_id)
        })
    }

    /// Place a task at `index` among the siblings under `parent_id` (manual order).
    pub fn place_task(&self, id: &str, parent_id: Option<&str>, index: usize) -> Res<HydratedTask> {
        self.tx_undoable(UndoKind::Move, |c, ctx| {
            let mut cur = need(c, id)?.task;
            check_parent(c, Some(id), parent_id)?;
            let sib: Vec<(String, f64)> = tasks::siblings(c, parent_id)?
                .into_iter()
                .filter(|s| s.0 != id)
                .collect();
            let at = index.min(sib.len());
            let before = at.checked_sub(1).map(|i| sib[i].1);
            let after = sib.get(at).map(|s| s.1);
            let mut pos = match (before, after) {
                (None, None) => 1.0,
                (None, Some(a)) => a - 1.0,
                (Some(b), None) => b + 1.0,
                (Some(b), Some(a)) => (b + a) / 2.0,
            };
            if let (Some(b), Some(a)) = (before, after) {
                if !(pos > b && pos < a) {
                    // No gap left between neighbours: renumber 1..n with the task in its slot.
                    let mut ordered: Vec<&str> = sib.iter().map(|s| s.0.as_str()).collect();
                    ordered.insert(at, id);
                    for (k, sid) in ordered.iter().enumerate() {
                        if *sid != id {
                            let mut s = need(c, sid)?.task;
                            s.position = (k + 1) as f64;
                            tasks::update_task(c, &s)?;
                        }
                    }
                    pos = (at + 1) as f64;
                }
            }
            if parent_id != cur.parent_id.as_deref() {
                log(c, ctx, Some(id), "subtask parent changed", Some(&format!(
                    "{} -> {}",
                    cur.parent_id.as_deref().unwrap_or("none"),
                    parent_id.unwrap_or("none")
                )))?;
            }
            cur.parent_id = parent_id.map(str::to_owned);
            cur.position = pos;
            tasks::update_task(c, &cur)?;
            log(c, ctx, Some(id), "moved", Some(&format!("position {pos}")))?;
            need(c, id)
        })
    }

    // ---- steps

    /// Add a step to a task.
    pub fn create_step(&self, task_id: &str, subject: &str) -> Res<Step> {
        self.tx_undoable(UndoKind::Edit, |c, ctx| {
            if subject.trim().is_empty() {
                return Err(coded("error.subjectRequired", "subject required"));
            }
            need(c, task_id)?;
            let (_, max) = tasks::step_position_bounds(c, task_id)?;
            let step = Step {
                id: new_id(),
                task_id: task_id.to_owned(),
                subject: subject.trim().to_owned(),
                status: TaskStatus::Open,
                position: max.unwrap_or(0.0) + 1.0,
            };
            tasks::insert_step(c, &step)?;
            log(c, ctx, Some(task_id), "step added", Some(&step.subject))?;
            Ok(step)
        })
    }

    /// Patch a step.
    pub fn update_step(&self, id: &str, p: StepPatch) -> Res<Step> {
        self.tx_undoable(UndoKind::Edit, |c, ctx| {
            let mut s = tasks::step(c, id)?.ok_or_else(|| not_found("what.step", "step"))?;
            if let Some(v) = p.subject {
                s.subject = v;
            }
            if let Some(v) = p.status {
                s.status = v;
            }
            if let Some(v) = p.position {
                s.position = v;
            }
            tasks::update_step(c, &s)?;
            log(c, ctx, Some(&s.task_id), "step updated", Some(&format!("{} [{}]", s.subject, token(s.status))))?;
            Ok(s)
        })
    }

    /// Delete a step.
    pub fn delete_step(&self, id: &str) -> Res<()> {
        self.tx_undoable(UndoKind::Edit, |c, ctx| {
            let s = tasks::step(c, id)?.ok_or_else(|| not_found("what.step", "step"))?;
            tasks::delete_step(c, id)?;
            log(c, ctx, Some(&s.task_id), "step deleted", Some(&s.subject))
        })
    }

    /// Promote a step to a subtask of its task.
    pub fn step_to_task(&self, id: &str) -> Res<HydratedTask> {
        self.tx_undoable(UndoKind::Edit, |c, ctx| {
            let s = tasks::step(c, id)?.ok_or_else(|| not_found("what.step", "step"))?;
            let t = create_in(c, ctx, NewTask {
                subject: s.subject.clone(),
                parent_id: Some(s.task_id.clone()),
                ..Default::default()
            })?;
            tasks::delete_step(c, id)?;
            log(c, ctx, Some(&s.task_id), "step promoted", Some(&s.subject))?;
            Ok(t)
        })
    }

    // ---- trash

    /// The trash, newest first; entries older than [`TRASH_DAYS`] are purged first.
    pub fn trash(&self) -> Res<Vec<TrashItem>> {
        let ctx = self.ctx();
        let cutoff = format!("{}{}", ctx.today.add_days(-TRASH_DAYS), &ctx.now[10..]);
        aux::purge_trash_before(self.storage.conn(), &cutoff)?;
        Ok(aux::trash(self.storage.conn())?
            .into_iter()
            .map(|r| TrashItem {
                id: r.id,
                kind: if r.kind == "view" { TrashKind::View } else { TrashKind::Task },
                label: r.label,
                deleted_at: r.deleted_at,
            })
            .collect())
    }

    /// Permanently remove one trash entry.
    pub fn delete_forever(&self, id: &str) -> Res<()> {
        self.tx_undoable(UndoKind::Trash, |c, ctx| {
            aux::delete_trash(c, id)?;
            log(c, ctx, None, "trash entry purged", Some(id))
        })
    }

    /// Permanently remove every trash entry.
    pub fn empty_trash(&self) -> Res<()> {
        self.tx_undoable(UndoKind::Trash, |c, ctx| {
            let n = aux::empty_trash(c)?;
            log(c, ctx, None, "trash emptied", Some(&n.to_string()))
        })
    }

    /// Restore a trashed task (with its subtree, steps and tags).
    pub fn restore(&self, id: &str) -> Res<Restored> {
        self.tx_undoable(UndoKind::Restore, |c, ctx| {
            let row = aux::trash_entry(c, id)?.ok_or_else(|| not_found("what.trashEntry", "trash entry"))?;
            let data: Value = serde_json::from_str(&row.data).unwrap_or(Value::Null);
            if row.kind == "view" {
                let mut v = view_from_json(&data["view"]).ok_or_else(|| not_found("what.trashEntry", "trash entry"))?;
                if views::get(c, &v.view.id)?.is_some() {
                    return Err(coded("error.viewExists", "view already exists"));
                }
                v.view.position = views::max_position(c)?.unwrap_or(0.0) + 1.0;
                views::insert(c, &v)?;
                aux::delete_trash(c, id)?;
                log(c, ctx, None, "view restored", Some(&v.view.name))?;
                return Ok(Restored::View { id: v.view.id });
            }
            let mut items: Vec<HydratedTask> = std::iter::once(&data["task"])
                .chain(data["subtasks"].as_array().into_iter().flatten())
                .filter_map(task_from_json)
                .collect();
            let root_id = items.first().map(|h| h.task.id.clone()).ok_or_else(|| not_found("what.trashEntry", "trash entry"))?;
            // A parent that no longer exists is dropped: the task returns at top level.
            if let Some(p) = items[0].task.parent_id.clone() {
                if tasks::task(c, &p)?.is_none() {
                    items[0].task.parent_id = None;
                }
            }
            for h in &items {
                tasks::insert_task(c, &h.task)?;
                tasks::set_tags(c, &h.task.id, &h.tags)?;
                for s in &h.steps {
                    tasks::insert_step(c, s)?;
                }
            }
            aux::delete_trash(c, id)?;
            log(c, ctx, Some(&root_id), "restored", Some(&items[0].task.subject))?;
            Ok(Restored::Task(need(c, &root_id)?))
        })
    }

    // ---- views

    /// Saved views in rail order.
    pub fn views(&self) -> Res<Vec<SavedView>> {
        Ok(views::all(self.storage.conn())?)
    }

    /// Create a view from raw form fields (validated by core `normalise_view`).
    /// It is appended to the rail.
    pub fn create_view(&self, raw: RawView) -> Res<SavedView> {
        self.tx_undoable(UndoKind::Create, |c, ctx| {
            let rules = normalise_view(&raw)?;
            let v = saved_from_rules(new_id(), rules, views::max_position(c)?.unwrap_or(0.0) + 1.0);
            views::insert(c, &v)?;
            log(c, ctx, None, "view created", Some(&v.view.name))?;
            Ok(v)
        })
    }

    /// Replace a view's fields (name, emoji, tags, match, sorts, include-completed).
    pub fn update_view(&self, id: &str, raw: RawView) -> Res<SavedView> {
        self.tx_undoable(UndoKind::Edit, |c, ctx| {
            let cur = views::get(c, id)?.ok_or_else(|| not_found("what.view", "view"))?;
            let v = saved_from_rules(id.to_owned(), normalise_view(&raw)?, cur.view.position);
            views::update(c, &v)?;
            log(c, ctx, None, "view updated", Some(&v.view.name))?;
            Ok(v)
        })
    }

    /// Move a view to the trash (restorable); returns the trash entry id.
    pub fn delete_view(&self, id: &str) -> Res<String> {
        self.tx_undoable(UndoKind::Delete, |c, ctx| {
            let v = views::get(c, id)?.ok_or_else(|| not_found("what.view", "view"))?;
            let trash_id = new_id();
            aux::insert_trash(c, &TrashRow {
                id: trash_id.clone(),
                kind: "view".into(),
                label: Some(v.view.name.clone()),
                data: json!({ "view": view_json(&v) }).to_string(),
                deleted_at: ctx.now.clone(),
            })?;
            views::delete(c, id)?;
            log(c, ctx, None, "view deleted", Some(&v.view.name))?;
            Ok(trash_id)
        })
    }

    /// Move a view to `index` in the rail; positions are renumbered gap-free 1..n.
    pub fn move_view(&self, id: &str, index: usize) -> Res<()> {
        self.tx_undoable(UndoKind::Move, |c, ctx| {
            let mut order: Vec<String> = views::all(c)?.into_iter().map(|v| v.view.id).collect();
            let from = order.iter().position(|v| v == id).ok_or_else(|| not_found("what.view", "view"))?;
            let moved = order.remove(from);
            order.insert(index.min(order.len()), moved);
            for (i, vid) in order.iter().enumerate() {
                views::set_position(c, vid, (i + 1) as f64)?;
            }
            log(c, ctx, None, "view moved", Some(id))
        })
    }

    // ---- prefs

    /// A pref value for a scope (e.g. `drawer:<taskId>`), if saved.
    pub fn pref(&self, scope: &str) -> Res<Option<String>> {
        Ok(aux::pref(self.storage.conn(), scope)?)
    }

    /// Save a pref. Scopes are `drawer:<taskId>` and `global` (My Day group order); the value must be non-empty.
    /// Not undoable: layout prefs do not touch task/trash state.
    pub fn set_pref(&self, scope: &str, value: &str) -> Res<()> {
        let drawer_scope = scope.starts_with("drawer:") && scope.len() > "drawer:".len();
        if !drawer_scope && scope != "global" {
            return Err(ServiceError::Coded(CodedError {
                code: "error.unknownPrefScope",
                message: "unknown preference scope".into(),
                params: vec![("scope", scope.to_owned())],
            }));
        }
        if value.trim().is_empty() {
            return Err(coded("error.prefValueRequired", "preference value required"));
        }
        self.tx(|c, _| Ok(aux::set_pref(c, scope, value)?))
    }

    // ---- settings

    /// Current settings.
    pub fn settings(&self) -> Res<Settings> {
        settings_in(self.storage.conn())
    }

    /// Notifications due at `now_local` (`YYYY-MM-DDTHH:MM` local time) that have not fired yet.
    /// They are recorded as fired (persisted, pruned to the newest 500) before being returned,
    /// so none repeats across ticks or restarts.
    pub fn take_due_notifications(&self, now_local: &str) -> Res<Vec<doene_core::notifications::Notification>> {
        use doene_core::notifications as n;
        self.tx(|c, ctx| {
            let fired: std::collections::HashSet<String> = aux::fired_keys(c)?.into_iter().collect();
            let get = |k: &str| -> Res<Option<String>> {
                Ok(aux::setting(c, k)?.and_then(|s| serde_json::from_str::<Value>(&s).ok()).and_then(|v| v.as_str().map(str::to_owned)))
            };
            let time = get("dueNotifyTime")?.unwrap_or_else(|| n::DEFAULT_DUE_NOTIFY_TIME.into());
            let lang = get("language")?.unwrap_or_else(|| "en".into());
            let due = n::due_notifications(&tasks::all_tasks(c)?, now_local, &fired, &time, &lang);
            for x in &due {
                aux::mark_fired(c, &x.key, &ctx.now)?;
            }
            if !due.is_empty() {
                aux::prune_fired(c, n::FIRED_KEEP)?;
            }
            Ok(due)
        })
    }

    /// Set one setting (`addTasksToTop`, `focusMode`, `language`) from its JSON value.
    pub fn set_setting(&self, key: &str, value: Value) -> Res<Settings> {
        self.tx(|c, ctx| {
            match key {
                "language" if !matches!(value.as_str(), Some(l) if doene_core::i18n::locales().contains(&l)) => {
                    return Err(ServiceError::Coded(CodedError {
                        code: "error.unsupportedLanguage",
                        message: "unsupported language".into(),
                        params: vec![("language", show(value.as_str()))],
                    }));
                }
                "focusMode" if !matches!(value.as_str(), Some("daily" | "durable")) => {
                    return Err(ServiceError::Coded(CodedError {
                        code: "error.unsupportedFocusMode",
                        message: "unsupported focus mode".into(),
                        params: vec![("mode", show(value.as_str()))],
                    }));
                }
                "theme" if !matches!(value.as_str(), Some("light" | "dark" | "system")) => {
                    return Err(ServiceError::Coded(CodedError {
                        code: "error.unsupportedTheme",
                        message: "unsupported theme".into(),
                        params: vec![("theme", show(value.as_str()))],
                    }));
                }
                "dueNotifyTime"
                    if !matches!(value.as_str(), Some(t) if t.len() == 5 && t.as_bytes()[2] == b':'
                        && t[..2].parse::<u8>().is_ok_and(|h| h < 24) && t[3..].parse::<u8>().is_ok_and(|m| m < 60)) =>
                {
                    return Err(ServiceError::Coded(CodedError {
                        code: "error.unknownSetting",
                        message: "dueNotifyTime must be HH:MM".into(),
                        params: vec![("k", key.to_owned())],
                    }));
                }
                "addTasksToTop" | "language" | "focusMode" | "theme" | "dueNotifyTime" => {}
                _ => {
                    return Err(ServiceError::Coded(CodedError {
                        code: "error.unknownSetting",
                        message: format!("unknown setting {key}"),
                        params: vec![("k", key.to_owned())],
                    }));
                }
            }
            aux::set_setting(c, key, &value.to_string())?;
            log(c, ctx, None, "setting changed", Some(key))?;
            settings_in(c)
        })
    }
}
