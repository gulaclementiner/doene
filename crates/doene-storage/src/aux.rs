//! Trash, activity log and settings rows. Plain SQL; the service owns the rules.

use rusqlite::{Connection, OptionalExtension, params};

/// A raw trash row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashRow {
    pub id: String,
    pub kind: String,
    pub label: Option<String>,
    pub data: String,
    pub deleted_at: String,
}

/// One activity row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityRow {
    pub task_id: Option<String>,
    pub at: String,
    pub action: String,
    pub detail: Option<String>,
}

/// Insert a trash row.
pub fn insert_trash(conn: &Connection, t: &TrashRow) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO trash (id, kind, label, data, deleted_at) VALUES (?1,?2,?3,?4,?5)",
        params![t.id, t.kind, t.label, t.data, t.deleted_at],
    )?;
    Ok(())
}

fn trash_from(r: &rusqlite::Row<'_>) -> rusqlite::Result<TrashRow> {
    Ok(TrashRow {
        id: r.get(0)?,
        kind: r.get(1)?,
        label: r.get(2)?,
        data: r.get(3)?,
        deleted_at: r.get(4)?,
    })
}

/// Every trash row, newest first.
pub fn trash(conn: &Connection) -> rusqlite::Result<Vec<TrashRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, label, data, deleted_at FROM trash ORDER BY deleted_at DESC, rowid DESC",
    )?;
    stmt.query_map([], trash_from)?.collect()
}

/// One trash row by id.
pub fn trash_entry(conn: &Connection, id: &str) -> rusqlite::Result<Option<TrashRow>> {
    conn.query_row(
        "SELECT id, kind, label, data, deleted_at FROM trash WHERE id = ?1",
        params![id],
        trash_from,
    )
    .optional()
}

/// Delete one trash row.
pub fn delete_trash(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM trash WHERE id = ?1", params![id])?;
    Ok(())
}

/// Delete every trash row; returns how many.
pub fn empty_trash(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM trash", [])
}

/// Delete trash rows deleted before `cutoff` (ISO-8601 UTC text compares chronologically).
pub fn purge_trash_before(conn: &Connection, cutoff: &str) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM trash WHERE deleted_at < ?1", params![cutoff])
}

/// Append an activity row.
pub fn insert_activity(
    conn: &Connection,
    id: &str,
    task_id: Option<&str>,
    at: &str,
    action: &str,
    detail: Option<&str>,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO activity (id, task_id, at, action, detail) VALUES (?1,?2,?3,?4,?5)",
        params![id, task_id, at, action, detail],
    )?;
    Ok(())
}

/// A task's activity, newest first.
pub fn activity_of(conn: &Connection, task_id: &str) -> rusqlite::Result<Vec<ActivityRow>> {
    let mut stmt = conn.prepare(
        "SELECT task_id, at, action, detail FROM activity WHERE task_id = ?1 ORDER BY at DESC, rowid DESC",
    )?;
    stmt.query_map(params![task_id], |r| {
        Ok(ActivityRow {
            task_id: r.get(0)?,
            at: r.get(1)?,
            action: r.get(2)?,
            detail: r.get(3)?,
        })
    })?
    .collect()
}

/// A setting's stored text (JSON), if saved.
pub fn setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| r.get(0))
        .optional()
}

/// Upsert a setting's text.
pub fn set_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}
