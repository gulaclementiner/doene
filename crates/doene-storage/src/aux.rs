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

/// Saved views as `(id, name, emoji, match_mode, show_done, position, tags)` read for quick-add.
pub fn views(conn: &Connection) -> rusqlite::Result<Vec<doene_core::View>> {
    use doene_core::{MatchMode, Tag, View};
    let mut stmt = conn.prepare(
        "SELECT id, name, emoji, match_mode, show_done, position FROM views ORDER BY position, id",
    )?;
    let mut views: Vec<View> = stmt
        .query_map([], |r| {
            Ok(View {
                id: r.get(0)?,
                name: r.get(1)?,
                emoji: r.get(2)?,
                match_mode: if r.get::<_, String>(3)? == "all" { MatchMode::All } else { MatchMode::Any },
                show_done: r.get::<_, i64>(4)? != 0,
                position: r.get(5)?,
                tags: vec![],
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut tag_stmt = conn.prepare("SELECT tag FROM view_tags WHERE view_id = ?1 ORDER BY tag")?;
    for v in &mut views {
        v.tags = tag_stmt
            .query_map(params![v.id], |r| r.get::<_, String>(0))?
            .filter_map(|t| t.ok().and_then(|t| Tag::new(&t).ok()))
            .collect();
    }
    Ok(views)
}

/// A pref's stored text for a scope, if saved.
pub fn pref(conn: &Connection, scope: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT value FROM prefs WHERE scope = ?1", params![scope], |r| r.get(0))
        .optional()
}

/// Upsert a pref's text for a scope.
pub fn set_pref(conn: &Connection, scope: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO prefs (scope, value) VALUES (?1, ?2) ON CONFLICT(scope) DO UPDATE SET value = excluded.value",
        params![scope, value],
    )?;
    Ok(())
}

/// Keys of already-fired notifications, oldest first.
pub fn fired_keys(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut st = conn.prepare("SELECT key FROM fired_notifications ORDER BY rowid")?;
    st.query_map([], |r| r.get(0))?.collect()
}

/// Record `key` as fired at `at` (no-op if already recorded).
pub fn mark_fired(conn: &Connection, key: &str, at: &str) -> rusqlite::Result<()> {
    conn.execute("INSERT OR IGNORE INTO fired_notifications (key, at) VALUES (?1, ?2)", params![key, at])?;
    Ok(())
}

/// Keep only the newest `keep` fired keys.
pub fn prune_fired(conn: &Connection, keep: usize) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM fired_notifications WHERE rowid NOT IN (SELECT rowid FROM fired_notifications ORDER BY rowid DESC LIMIT ?1)",
        params![keep as i64],
    )
}

/// Every activity row in insertion order (for export).
pub fn all_activity(conn: &Connection) -> rusqlite::Result<Vec<(String, ActivityRow)>> {
    let mut stmt = conn.prepare("SELECT id, task_id, at, action, detail FROM activity ORDER BY rowid")?;
    stmt.query_map([], |r| {
        Ok((
            r.get(0)?,
            ActivityRow { task_id: r.get(1)?, at: r.get(2)?, action: r.get(3)?, detail: r.get(4)? },
        ))
    })?
    .collect()
}

/// Every setting `(key, text)` ordered by key.
pub fn all_settings(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings ORDER BY key")?;
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect()
}

/// Every pref `(scope, text)` ordered by scope.
pub fn all_prefs(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT scope, value FROM prefs ORDER BY scope")?;
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect()
}

/// Delete all activity, settings and prefs rows (the aux part of an import wipe).
pub fn wipe_aux(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("DELETE FROM activity; DELETE FROM settings; DELETE FROM prefs;")
}
