//! JSON export / import (ticket #23). Our own versioned format, not Gör't's.
//!
//! Scope: tasks (steps, tags, recurrence, tree, `spawned_from`), views, trash, activity,
//! settings and prefs. `fired_notifications` is deliberately excluded: it is a local
//! dedupe cache keyed by task id, rebuilt by use, and keeping the device's own set stops
//! imported tasks re-notifying. Import wipes and reloads in one transaction. It is not
//! undoable: undo snapshots cover only tasks/trash/views, so a partial undo would leave
//! settings and activity from the file. Instead the undo stack is cleared (its entries
//! would reference the wiped state).

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value, json};

use super::*;

/// The `format` marker of an export file.
pub const EXPORT_FORMAT: &str = "doene-export";
/// Current export `version`.
pub const EXPORT_VERSION: u64 = 1;

/// What an import loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportSummary {
    pub tasks: usize,
    pub views: usize,
    pub trash: usize,
    pub activity: usize,
}

fn invalid(what: impl Into<String>) -> ServiceError {
    let what = what.into();
    ServiceError::Coded(CodedError {
        code: "error.importInvalid",
        message: format!("invalid import file: {what}"),
        params: vec![("what", what)],
    })
}

fn req_str(v: &Value, k: &str, what: &str) -> Res<String> {
    v.get(k)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("{what}.{k}")))
}

fn array<'a>(root: &'a Value, k: &str) -> Res<&'a [Value]> {
    match root.get(k) {
        None => Ok(&[]),
        Some(Value::Array(a)) => Ok(a),
        Some(_) => Err(invalid(k)),
    }
}

impl Service {
    /// The whole database as pretty JSON text (see module docs for scope).
    pub fn export_json(&self) -> Res<String> {
        let c = self.storage.conn();
        let ctx = self.ctx();
        let tasks: Vec<Value> = tasks::all_tasks(c)?
            .iter()
            .map(|h| {
                let mut v = task_json(h);
                v["spawned_from"] = json!(h.task.spawned_from);
                v
            })
            .collect();
        let trash: Vec<Value> = aux::trash(c)?
            .into_iter()
            .map(|t| json!({ "id": t.id, "kind": t.kind, "label": t.label, "data": t.data, "deleted_at": t.deleted_at }))
            .collect();
        let activity: Vec<Value> = aux::all_activity(c)?
            .into_iter()
            .map(|(id, a)| json!({ "id": id, "task_id": a.task_id, "at": a.at, "action": a.action, "detail": a.detail }))
            .collect();
        let settings: Map<String, Value> = aux::all_settings(c)?
            .into_iter()
            .filter_map(|(k, v)| Some((k, serde_json::from_str(&v).ok()?)))
            .collect();
        let prefs: Map<String, Value> = aux::all_prefs(c)?
            .into_iter()
            .map(|(k, v)| (k, Value::String(v)))
            .collect();
        let doc = json!({
            "format": EXPORT_FORMAT, "version": EXPORT_VERSION, "exported_at": ctx.now,
            "tasks": tasks,
            "views": views::all(c)?.iter().map(view_json).collect::<Vec<_>>(),
            "trash": trash, "activity": activity, "settings": settings, "prefs": prefs,
        });
        serde_json::to_string_pretty(&doc).map_err(|_| invalid("serialise"))
    }

    /// Replace all data with the export in `text`. Everything is validated before the
    /// database is touched, and the wipe+load is one transaction: on error nothing changes.
    pub fn import_json(&self, text: &str) -> Res<ImportSummary> {
        let root: Value =
            serde_json::from_str(text).map_err(|_| coded("error.invalidJson", "invalid JSON"))?;
        if !root.is_object() || root.get("format").and_then(Value::as_str) != Some(EXPORT_FORMAT) {
            return Err(invalid("format"));
        }
        let version = root.get("version").and_then(Value::as_u64);
        if version != Some(EXPORT_VERSION) {
            return Err(ServiceError::Coded(CodedError {
                code: "error.exportVersion",
                message: "unsupported export version".into(),
                params: vec![(
                    "version",
                    root.get("version")
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                )],
            }));
        }

        let mut tasks_in = Vec::new();
        let mut ids = HashSet::new();
        for (i, t) in array(&root, "tasks")?.iter().enumerate() {
            let mut h = task_from_json(t).ok_or_else(|| invalid(format!("tasks[{i}]")))?;
            for (k, n) in [("tags", h.tags.len()), ("steps", h.steps.len())] {
                if t.get(k).and_then(Value::as_array).map_or(0, Vec::len) != n {
                    return Err(invalid(format!("tasks[{i}].{k}")));
                }
            }
            h.task.spawned_from = t
                .get("spawned_from")
                .and_then(Value::as_str)
                .map(str::to_owned);
            if !ids.insert(h.task.id.clone()) {
                return Err(invalid(format!("tasks[{i}].id duplicate")));
            }
            tasks_in.push(h);
        }
        let step_ids: HashSet<&str> = tasks_in
            .iter()
            .flat_map(|h| h.steps.iter().map(|s| s.id.as_str()))
            .collect();
        if step_ids.len() != tasks_in.iter().map(|h| h.steps.len()).sum::<usize>() {
            return Err(invalid("steps.id duplicate"));
        }
        let mut parents = HashMap::new();
        for (i, h) in tasks_in.iter().enumerate() {
            for (k, r) in [
                ("parent_id", &h.task.parent_id),
                ("spawned_from", &h.task.spawned_from),
            ] {
                if r.as_ref().is_some_and(|r| !ids.contains(r)) {
                    return Err(invalid(format!("tasks[{i}].{k}")));
                }
            }
            parents.insert(h.task.id.clone(), h.task.parent_id.clone());
        }
        for (i, h) in tasks_in.iter().enumerate() {
            if h.task
                .parent_id
                .as_deref()
                .is_some_and(|p| tree::descends_from(p, &parents, &h.task.id) || p == h.task.id)
            {
                return Err(invalid(format!("tasks[{i}].parent_id cycle")));
            }
        }

        let mut views_in = Vec::new();
        let mut view_ids = HashSet::new();
        for (i, v) in array(&root, "views")?.iter().enumerate() {
            let sv = view_from_json(v).ok_or_else(|| invalid(format!("views[{i}]")))?;
            if sv.view.name.trim().is_empty() || !view_ids.insert(sv.view.id.clone()) {
                return Err(invalid(format!("views[{i}]")));
            }
            views_in.push(sv);
        }

        let mut trash_in = Vec::new();
        let mut trash_ids = HashSet::new();
        for (i, t) in array(&root, "trash")?.iter().enumerate() {
            let w = format!("trash[{i}]");
            let row = TrashRow {
                id: req_str(t, "id", &w)?,
                kind: req_str(t, "kind", &w)?,
                label: t.get("label").and_then(Value::as_str).map(str::to_owned),
                data: req_str(t, "data", &w)?,
                deleted_at: req_str(t, "deleted_at", &w)?,
            };
            if !matches!(row.kind.as_str(), "task" | "view")
                || serde_json::from_str::<Value>(&row.data).is_err()
                || !trash_ids.insert(row.id.clone())
            {
                return Err(invalid(w));
            }
            trash_in.push(row);
        }

        let mut activity_in = Vec::new();
        let mut act_ids = HashSet::new();
        for (i, a) in array(&root, "activity")?.iter().enumerate() {
            let w = format!("activity[{i}]");
            let id = req_str(a, "id", &w)?;
            if !act_ids.insert(id.clone()) {
                return Err(invalid(w));
            }
            let row = ActivityRow {
                task_id: a.get("task_id").and_then(Value::as_str).map(str::to_owned),
                at: req_str(a, "at", &w)?,
                action: req_str(a, "action", &w)?,
                detail: a.get("detail").and_then(Value::as_str).map(str::to_owned),
            };
            activity_in.push((id, row));
        }

        let mut settings_in = Vec::new();
        match root.get("settings") {
            None => {}
            Some(Value::Object(m)) => {
                for (k, v) in m {
                    check_setting(k, v)?;
                    settings_in.push((k.clone(), v.to_string()));
                }
            }
            Some(_) => return Err(invalid("settings")),
        }
        let mut prefs_in = Vec::new();
        match root.get("prefs") {
            None => {}
            Some(Value::Object(m)) => {
                for (k, v) in m {
                    let s = v.as_str().ok_or_else(|| invalid(format!("prefs.{k}")))?;
                    check_pref(k, s)?;
                    prefs_in.push((k.clone(), s.to_owned()));
                }
            }
            Some(_) => return Err(invalid("prefs")),
        }

        let summary = ImportSummary {
            tasks: tasks_in.len(),
            views: views_in.len(),
            trash: trash_in.len(),
            activity: activity_in.len(),
        };
        self.tx(|c, _| {
            c.pragma_update(None, "defer_foreign_keys", true)?;
            let existing: Vec<String> = tasks::all_tasks(c)?
                .into_iter()
                .map(|h| h.task.id)
                .collect();
            tasks::delete_tasks(c, &existing)?;
            for v in views::all(c)? {
                views::delete(c, &v.view.id)?;
            }
            aux::empty_trash(c)?;
            aux::wipe_aux(c)?;
            for h in &tasks_in {
                tasks::insert_task(c, &h.task)?;
                tasks::set_tags(c, &h.task.id, &h.tags)?;
                for s in &h.steps {
                    tasks::insert_step(c, s)?;
                }
            }
            for v in &views_in {
                views::insert(c, v)?;
            }
            // Export lists newest first; insert oldest first so rowid tie-breaks match.
            for t in trash_in.iter().rev() {
                aux::insert_trash(c, t)?;
            }
            for (id, a) in &activity_in {
                aux::insert_activity(
                    c,
                    id,
                    a.task_id.as_deref(),
                    &a.at,
                    &a.action,
                    a.detail.as_deref(),
                )?;
            }
            for (k, v) in &settings_in {
                aux::set_setting(c, k, v)?;
            }
            for (k, v) in &prefs_in {
                aux::set_pref(c, k, v)?;
            }
            Ok(())
        })?;
        self.undo.lock().unwrap_or_else(|e| e.into_inner()).clear();
        Ok(summary)
    }
}
