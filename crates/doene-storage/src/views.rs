//! Saved views: `views` row, `view_tags` and `view_sorts`. Plain SQL; the
//! service owns validation and renumbering rules.

use doene_core::{MatchMode, SortDirection, SortKey, SortLevel, Tag, View};
use rusqlite::{Connection, OptionalExtension, params};

/// A view with its chained sort levels (level 0 first).
#[derive(Debug, Clone, PartialEq)]
pub struct SavedView {
    pub view: View,
    pub sorts: Vec<SortLevel>,
}

fn key_token(k: SortKey) -> &'static str {
    match k {
        SortKey::Name => "name",
        SortKey::Due => "due",
        SortKey::Created => "created",
        SortKey::Manual => "manual",
    }
}

const VIEW_COLUMNS: &str = "id, name, emoji, match_mode, show_done, position";

fn load(conn: &Connection, mut view: View) -> rusqlite::Result<SavedView> {
    view.tags = conn
        .prepare("SELECT tag FROM view_tags WHERE view_id = ?1 ORDER BY tag")?
        .query_map(params![view.id], |r| r.get::<_, String>(0))?
        .filter_map(|t| t.ok().and_then(|t| Tag::new(&t).ok()))
        .collect();
    let sorts = conn
        .prepare("SELECT by, dir FROM view_sorts WHERE view_id = ?1 ORDER BY level")?
        .query_map(params![view.id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .filter_map(|r| r.ok())
        .filter_map(|(by, dir)| {
            Some(SortLevel {
                by: SortKey::from_token(&by)?,
                dir: if dir == "desc" { SortDirection::Desc } else { SortDirection::Asc },
            })
        })
        .collect();
    Ok(SavedView { view, sorts })
}

fn view_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<View> {
    Ok(View {
        id: r.get(0)?,
        name: r.get(1)?,
        emoji: r.get(2)?,
        match_mode: if r.get::<_, String>(3)? == "all" { MatchMode::All } else { MatchMode::Any },
        show_done: r.get::<_, i64>(4)? != 0,
        position: r.get(5)?,
        tags: vec![],
    })
}

/// Every view in rail order, with tags and sorts.
pub fn all(conn: &Connection) -> rusqlite::Result<Vec<SavedView>> {
    let views: Vec<View> = conn
        .prepare(&format!("SELECT {VIEW_COLUMNS} FROM views ORDER BY position, id"))?
        .query_map([], view_from_row)?
        .collect::<rusqlite::Result<_>>()?;
    views.into_iter().map(|v| load(conn, v)).collect()
}

/// One view by id.
pub fn get(conn: &Connection, id: &str) -> rusqlite::Result<Option<SavedView>> {
    let v = conn
        .query_row(&format!("SELECT {VIEW_COLUMNS} FROM views WHERE id = ?1"), params![id], view_from_row)
        .optional()?;
    v.map(|v| load(conn, v)).transpose()
}

fn write_children(conn: &Connection, v: &SavedView) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM view_tags WHERE view_id = ?1", params![v.view.id])?;
    conn.execute("DELETE FROM view_sorts WHERE view_id = ?1", params![v.view.id])?;
    for t in &v.view.tags {
        conn.execute(
            "INSERT OR IGNORE INTO view_tags (view_id, tag) VALUES (?1, ?2)",
            params![v.view.id, t.as_str()],
        )?;
    }
    for (i, s) in v.sorts.iter().enumerate() {
        conn.execute(
            "INSERT INTO view_sorts (view_id, level, by, dir) VALUES (?1, ?2, ?3, ?4)",
            params![
                v.view.id,
                i as i64,
                key_token(s.by),
                if s.dir == SortDirection::Desc { "desc" } else { "asc" }
            ],
        )?;
    }
    Ok(())
}

fn mode(m: MatchMode) -> &'static str {
    if m == MatchMode::All { "all" } else { "any" }
}

/// Insert a view with its tags and sorts.
pub fn insert(conn: &Connection, v: &SavedView) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO views (id, name, emoji, match_mode, show_done, position) VALUES (?1,?2,?3,?4,?5,?6)",
        params![v.view.id, v.view.name, v.view.emoji, mode(v.view.match_mode), v.view.show_done, v.view.position],
    )?;
    write_children(conn, v)
}

/// Overwrite a view's fields, tags and sorts (position included).
pub fn update(conn: &Connection, v: &SavedView) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE views SET name=?2, emoji=?3, match_mode=?4, show_done=?5, position=?6 WHERE id=?1",
        params![v.view.id, v.view.name, v.view.emoji, mode(v.view.match_mode), v.view.show_done, v.view.position],
    )?;
    write_children(conn, v)
}

/// Set only a view's rail position.
pub fn set_position(conn: &Connection, id: &str, position: f64) -> rusqlite::Result<()> {
    conn.execute("UPDATE views SET position = ?2 WHERE id = ?1", params![id, position])?;
    Ok(())
}

/// Delete a view (tags and sorts cascade).
pub fn delete(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM views WHERE id = ?1", params![id])?;
    Ok(())
}

/// Highest position, if any view exists.
pub fn max_position(conn: &Connection) -> rusqlite::Result<Option<f64>> {
    conn.query_row("SELECT MAX(position) FROM views", [], |r| r.get(0))
}
