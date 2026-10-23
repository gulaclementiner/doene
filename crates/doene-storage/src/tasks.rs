//! Task repository: row mapping for `tasks`, `steps` and `task_tags`.
//!
//! Plain SQL over a `Connection`; no domain rules (those live in
//! `doene-core` and `doene-service`). Recurrence is stored in the normalized
//! `recurrence_*` columns (plan §5); weekday sets as CSV of dow numbers.

use doene_core::{
    Date, HydratedTask, Importance, RecurrenceRule, RecurrenceType, RecurrenceUnit, Step, Tag,
    Task, TaskStatus, Weekday,
};
use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter};

const TASK_COLUMNS: &str = "id, subject, status, importance, due_date, reminder_at, body, \
    focus_date, postponed_date, created_at, completed_at, position, spawned_from, parent_id, \
    recurrence_type, recurrence_interval, recurrence_unit, recurrence_days";

fn status_token(s: TaskStatus) -> &'static str {
    match s {
        TaskStatus::Open => "open",
        TaskStatus::Completed => "completed",
    }
}

fn status_from(token: &str) -> TaskStatus {
    if token == "completed" {
        TaskStatus::Completed
    } else {
        TaskStatus::Open
    }
}

fn importance_token(i: Importance) -> &'static str {
    match i {
        Importance::Normal => "normal",
        Importance::High => "high",
    }
}

fn importance_from(token: &str) -> Importance {
    if token == "high" {
        Importance::High
    } else {
        Importance::Normal
    }
}

fn date_text(d: Option<Date>) -> Option<String> {
    d.map(|d| d.to_string())
}

fn date_from(text: Option<String>) -> Option<Date> {
    text.and_then(|s| Date::parse(&s).ok())
}

fn recurrence_days_text(days: &Option<Vec<Weekday>>) -> Option<String> {
    days.as_ref().map(|ws| {
        ws.iter()
            .map(|w| w.to_dow().to_string())
            .collect::<Vec<_>>()
            .join(",")
    })
}

fn recurrence_days_from(text: Option<String>) -> Option<Vec<Weekday>> {
    text.map(|s| {
        s.split(',')
            .filter_map(|n| n.parse::<u8>().ok().and_then(Weekday::from_dow))
            .collect()
    })
}

fn task_from_row(r: &Row<'_>) -> rusqlite::Result<Task> {
    let kind: Option<String> = r.get("recurrence_type")?;
    let recurrence = kind.and_then(|k| RecurrenceType::from_token(&k)).map(|kind| RecurrenceRule {
        kind,
        interval: r
            .get::<_, Option<i64>>("recurrence_interval")
            .ok()
            .flatten()
            .and_then(|n| u32::try_from(n).ok()),
        unit: r
            .get::<_, Option<String>>("recurrence_unit")
            .ok()
            .flatten()
            .and_then(|u| RecurrenceUnit::from_token(&u)),
        days: recurrence_days_from(r.get("recurrence_days").ok().flatten()),
    });
    Ok(Task {
        id: r.get("id")?,
        subject: r.get("subject")?,
        status: status_from(&r.get::<_, String>("status")?),
        importance: importance_from(&r.get::<_, String>("importance")?),
        due_date: date_from(r.get("due_date")?),
        reminder_at: r.get("reminder_at")?,
        body: r.get("body")?,
        focus_date: date_from(r.get("focus_date")?),
        postponed_date: date_from(r.get("postponed_date")?),
        created_at: r.get("created_at")?,
        completed_at: r.get("completed_at")?,
        position: r.get("position")?,
        spawned_from: r.get("spawned_from")?,
        parent_id: r.get("parent_id")?,
        recurrence,
    })
}

/// Insert a task row (no tags or steps).
pub fn insert_task(conn: &Connection, t: &Task) -> rusqlite::Result<()> {
    let (kind, interval, unit, days) = match &t.recurrence {
        Some(r) => (
            Some(r.kind.as_token()),
            r.interval.map(i64::from),
            r.unit.map(RecurrenceUnit::as_token),
            recurrence_days_text(&r.days),
        ),
        None => (None, None, None, None),
    };
    conn.execute(
        &format!(
            "INSERT INTO tasks ({TASK_COLUMNS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)"
        ),
        params![
            t.id,
            t.subject,
            status_token(t.status),
            importance_token(t.importance),
            date_text(t.due_date),
            t.reminder_at,
            t.body,
            date_text(t.focus_date),
            date_text(t.postponed_date),
            t.created_at,
            t.completed_at,
            t.position,
            t.spawned_from,
            t.parent_id,
            kind,
            interval,
            unit,
            days,
        ],
    )?;
    Ok(())
}

/// One task row by id, without its tags or steps.
pub fn task(conn: &Connection, id: &str) -> rusqlite::Result<Option<Task>> {
    conn.query_row(
        &format!("SELECT {TASK_COLUMNS} FROM tasks WHERE id = ?1"),
        params![id],
        task_from_row,
    )
    .optional()
}

/// Every task, hydrated with tags and steps, ordered by creation then position.
pub fn all_tasks(conn: &Connection) -> rusqlite::Result<Vec<HydratedTask>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {TASK_COLUMNS} FROM tasks ORDER BY created_at, position, id"
    ))?;
    let rows: Vec<Task> = stmt
        .query_map([], task_from_row)?
        .collect::<rusqlite::Result<_>>()?;
    rows.into_iter().map(|t| hydrate(conn, t)).collect()
}

fn hydrate(conn: &Connection, task: Task) -> rusqlite::Result<HydratedTask> {
    let tags = tags_of(conn, &task.id)?;
    let steps = steps_of(conn, &task.id)?;
    Ok(HydratedTask { task, tags, steps })
}

/// A task's tags, stored order (alphabetical, as written by [`set_tags`]).
pub fn tags_of(conn: &Connection, task_id: &str) -> rusqlite::Result<Vec<Tag>> {
    let mut stmt = conn.prepare("SELECT tag FROM task_tags WHERE task_id = ?1 ORDER BY tag")?;
    let words = stmt.query_map(params![task_id], |r| r.get::<_, String>(0))?;
    words
        .map(|w| {
            let word = w?;
            // Stored words were validated on write; a stray value is skipped, not fatal.
            Ok(Tag::new(&word).ok())
        })
        .filter_map(|r| r.transpose())
        .collect()
}

/// Replace a task's tag set.
pub fn set_tags(conn: &Connection, task_id: &str, tags: &[Tag]) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM task_tags WHERE task_id = ?1", params![task_id])?;
    for tag in tags {
        conn.execute(
            "INSERT INTO task_tags (task_id, tag) VALUES (?1, ?2)",
            params![task_id, tag.as_str()],
        )?;
    }
    Ok(())
}

/// Insert a step row.
pub fn insert_step(conn: &Connection, s: &Step) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO steps (id, task_id, subject, status, position) VALUES (?1,?2,?3,?4,?5)",
        params![s.id, s.task_id, s.subject, status_token(s.status), s.position],
    )?;
    Ok(())
}

/// A task's steps in position order.
pub fn steps_of(conn: &Connection, task_id: &str) -> rusqlite::Result<Vec<Step>> {
    let mut stmt = conn.prepare(
        "SELECT id, task_id, subject, status, position FROM steps WHERE task_id = ?1 ORDER BY position, id",
    )?;
    stmt.query_map(params![task_id], |r| {
        Ok(Step {
            id: r.get(0)?,
            task_id: r.get(1)?,
            subject: r.get(2)?,
            status: status_from(&r.get::<_, String>(3)?),
            position: r.get(4)?,
        })
    })?
    .collect()
}

/// Delete tasks by id; steps and tag rows cascade with them.
pub fn delete_tasks(conn: &Connection, ids: &[String]) -> rusqlite::Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let marks = vec!["?"; ids.len()].join(",");
    // Spawned occurrences keep living when their origin goes (Gör't: ON DELETE SET NULL).
    conn.execute(
        &format!("UPDATE tasks SET spawned_from = NULL WHERE spawned_from IN ({marks})"),
        params_from_iter(ids.iter()),
    )?;
    conn.execute(
        &format!("DELETE FROM tasks WHERE id IN ({marks})"),
        params_from_iter(ids.iter()),
    )?;
    Ok(())
}

/// Overwrite every column of an existing task row with `t` (tags and steps untouched).
pub fn update_task(conn: &Connection, t: &Task) -> rusqlite::Result<()> {
    let (kind, interval, unit, days) = match &t.recurrence {
        Some(r) => (
            Some(r.kind.as_token()),
            r.interval.map(i64::from),
            r.unit.map(RecurrenceUnit::as_token),
            recurrence_days_text(&r.days),
        ),
        None => (None, None, None, None),
    };
    conn.execute(
        "UPDATE tasks SET subject=?2, status=?3, importance=?4, due_date=?5, reminder_at=?6, body=?7, \
         focus_date=?8, postponed_date=?9, created_at=?10, completed_at=?11, position=?12, \
         spawned_from=?13, parent_id=?14, recurrence_type=?15, recurrence_interval=?16, \
         recurrence_unit=?17, recurrence_days=?18 WHERE id=?1",
        params![
            t.id,
            t.subject,
            status_token(t.status),
            importance_token(t.importance),
            date_text(t.due_date),
            t.reminder_at,
            t.body,
            date_text(t.focus_date),
            date_text(t.postponed_date),
            t.created_at,
            t.completed_at,
            t.position,
            t.spawned_from,
            t.parent_id,
            kind,
            interval,
            unit,
            days,
        ],
    )?;
    Ok(())
}

/// One task with tags and steps.
pub fn hydrated(conn: &Connection, id: &str) -> rusqlite::Result<Option<HydratedTask>> {
    task(conn, id)?.map(|t| hydrate(conn, t)).transpose()
}

/// Direct children of `parent` (`None` = top level) as `(id, position)`, by position then id.
pub fn siblings(conn: &Connection, parent: Option<&str>) -> rusqlite::Result<Vec<(String, f64)>> {
    let mut stmt = conn.prepare(
        "SELECT id, position FROM tasks WHERE parent_id IS ?1 ORDER BY position, id",
    )?;
    stmt.query_map(params![parent], |r| Ok((r.get(0)?, r.get(1)?)))?.collect()
}

/// Every descendant id of a task, breadth-first (parents before children).
pub fn descendants(conn: &Connection, id: &str) -> rusqlite::Result<Vec<String>> {
    let mut out = Vec::new();
    let mut frontier = vec![id.to_owned()];
    let mut stmt = conn.prepare("SELECT id FROM tasks WHERE parent_id = ?1 ORDER BY position, id")?;
    while !frontier.is_empty() {
        let mut next = Vec::new();
        for f in &frontier {
            for c in stmt.query_map(params![f], |r| r.get::<_, String>(0))? {
                let c = c?;
                out.push(c.clone());
                next.push(c);
            }
        }
        frontier = next;
    }
    Ok(out)
}

/// Id -> parent id for every task (feeds the core tree guards).
pub fn parent_map(conn: &Connection) -> rusqlite::Result<std::collections::HashMap<String, Option<String>>> {
    let mut stmt = conn.prepare("SELECT id, parent_id FROM tasks")?;
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect()
}

/// One step by id.
pub fn step(conn: &Connection, id: &str) -> rusqlite::Result<Option<Step>> {
    conn.query_row(
        "SELECT id, task_id, subject, status, position FROM steps WHERE id = ?1",
        params![id],
        |r| {
            Ok(Step {
                id: r.get(0)?,
                task_id: r.get(1)?,
                subject: r.get(2)?,
                status: status_from(&r.get::<_, String>(3)?),
                position: r.get(4)?,
            })
        },
    )
    .optional()
}

/// Overwrite a step's subject, status and position.
pub fn update_step(conn: &Connection, s: &Step) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE steps SET subject=?2, status=?3, position=?4 WHERE id=?1",
        params![s.id, s.subject, status_token(s.status), s.position],
    )?;
    Ok(())
}

/// Delete one step.
pub fn delete_step(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM steps WHERE id = ?1", params![id])?;
    Ok(())
}

/// Smallest and largest position among a task's steps.
pub fn step_position_bounds(conn: &Connection, task_id: &str) -> rusqlite::Result<(Option<f64>, Option<f64>)> {
    conn.query_row(
        "SELECT MIN(position), MAX(position) FROM steps WHERE task_id = ?1",
        params![task_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
}
