//! JSON snapshots of tasks for the trash (`trash.data`): the task, its steps and
//! tags, plus its whole subtree, parents first so foreign keys hold on restore.

use doene_core::{
    Date, HydratedTask, Importance, RecurrenceRule, RecurrenceType, RecurrenceUnit, Step, Tag,
    Task, TaskStatus, Weekday,
};
use doene_core::{MatchMode, SortDirection, SortKey, SortLevel, View};
use doene_storage::views::SavedView;
use serde_json::{Value, json};

fn status(s: TaskStatus) -> &'static str {
    if s == TaskStatus::Completed { "completed" } else { "open" }
}

fn status_from(s: &str) -> TaskStatus {
    if s == "completed" { TaskStatus::Completed } else { TaskStatus::Open }
}

pub fn task_json(h: &HydratedTask) -> Value {
    let t = &h.task;
    let recurrence = t.recurrence.as_ref().map(|r| {
        json!({
            "type": r.kind.as_token(),
            "interval": r.interval,
            "unit": r.unit.map(RecurrenceUnit::as_token),
            "days": r.days.as_ref().map(|d| d.iter().map(|w| w.to_dow()).collect::<Vec<_>>()),
        })
    });
    json!({
        "id": t.id, "subject": t.subject, "status": status(t.status),
        "importance": if t.importance == Importance::High { "high" } else { "normal" },
        "due_date": t.due_date.map(|d| d.to_string()), "reminder_at": t.reminder_at,
        "body": t.body, "focus_date": t.focus_date.map(|d| d.to_string()),
        "postponed_date": t.postponed_date.map(|d| d.to_string()),
        "created_at": t.created_at, "completed_at": t.completed_at,
        "position": t.position, "parent_id": t.parent_id, "recurrence": recurrence,
        "tags": h.tags.iter().map(Tag::as_str).collect::<Vec<_>>(),
        "steps": h.steps.iter().map(|s| json!({
            "id": s.id, "subject": s.subject, "status": status(s.status), "position": s.position
        })).collect::<Vec<_>>(),
    })
}

fn opt_str(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::to_owned)
}

fn opt_date(v: &Value, k: &str) -> Option<Date> {
    v.get(k).and_then(Value::as_str).and_then(|s| Date::parse(s).ok())
}

pub fn task_from_json(v: &Value) -> Option<HydratedTask> {
    let id = opt_str(v, "id")?;
    let recurrence = v.get("recurrence").filter(|r| !r.is_null()).and_then(|r| {
        Some(RecurrenceRule {
            kind: RecurrenceType::from_token(r.get("type")?.as_str()?)?,
            interval: r.get("interval").and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok()),
            unit: r.get("unit").and_then(Value::as_str).and_then(RecurrenceUnit::from_token),
            days: r.get("days").and_then(Value::as_array).map(|a| {
                a.iter()
                    .filter_map(|n| n.as_u64().and_then(|n| u8::try_from(n).ok()).and_then(Weekday::from_dow))
                    .collect()
            }),
        })
    });
    let task = Task {
        id: id.clone(),
        subject: opt_str(v, "subject")?,
        status: status_from(v.get("status")?.as_str()?),
        importance: if v.get("importance").and_then(Value::as_str) == Some("high") {
            Importance::High
        } else {
            Importance::Normal
        },
        due_date: opt_date(v, "due_date"),
        reminder_at: opt_str(v, "reminder_at"),
        body: opt_str(v, "body"),
        focus_date: opt_date(v, "focus_date"),
        postponed_date: opt_date(v, "postponed_date"),
        created_at: opt_str(v, "created_at")?,
        completed_at: opt_str(v, "completed_at"),
        position: v.get("position").and_then(Value::as_f64).unwrap_or(0.0),
        spawned_from: None, // transient lifecycle link, not restored
        parent_id: opt_str(v, "parent_id"),
        recurrence,
    };
    let tags = v
        .get("tags")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|t| t.as_str().and_then(|s| Tag::new(s).ok())).collect())
        .unwrap_or_default();
    let steps = v
        .get("steps")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|s| {
                    Some(Step {
                        id: opt_str(s, "id")?,
                        task_id: id.clone(),
                        subject: opt_str(s, "subject")?,
                        status: status_from(s.get("status")?.as_str()?),
                        position: s.get("position").and_then(Value::as_f64).unwrap_or(0.0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Some(HydratedTask { task, tags, steps })
}

pub fn view_json(v: &SavedView) -> Value {
    json!({
        "id": v.view.id, "name": v.view.name, "emoji": v.view.emoji,
        "match": if v.view.match_mode == MatchMode::All { "all" } else { "any" },
        "show_done": v.view.show_done, "position": v.view.position,
        "tags": v.view.tags.iter().map(Tag::as_str).collect::<Vec<_>>(),
        "sorts": v.sorts.iter().map(|s| json!({
            "by": match s.by { SortKey::Name => "name", SortKey::Due => "due", SortKey::Created => "created", SortKey::Manual => "manual" },
            "dir": if s.dir == SortDirection::Desc { "desc" } else { "asc" },
        })).collect::<Vec<_>>(),
    })
}

pub fn view_from_json(v: &Value) -> Option<SavedView> {
    Some(SavedView {
        view: View {
            id: opt_str(v, "id")?,
            name: opt_str(v, "name")?,
            emoji: opt_str(v, "emoji"),
            match_mode: if v.get("match").and_then(Value::as_str) == Some("all") { MatchMode::All } else { MatchMode::Any },
            show_done: v.get("show_done").and_then(Value::as_bool).unwrap_or(false),
            position: v.get("position").and_then(Value::as_f64).unwrap_or(0.0),
            tags: v
                .get("tags")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|t| t.as_str().and_then(|s| Tag::new(s).ok())).collect())
                .unwrap_or_default(),
        },
        sorts: v
            .get("sorts")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|s| {
                        Some(SortLevel {
                            by: SortKey::from_token(s.get("by")?.as_str()?)?,
                            dir: if s.get("dir").and_then(Value::as_str) == Some("desc") { SortDirection::Desc } else { SortDirection::Asc },
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}
