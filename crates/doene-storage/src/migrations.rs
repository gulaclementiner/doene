//! Schema migrations, tracked by `PRAGMA user_version` (docs/plan.md §5).
//!
//! Each migration is a numbered step applied inside one transaction. A fresh
//! database runs every step; an existing one runs only the steps it lacks.

use rusqlite::Connection;

/// The schema version this build expects (the number of migration steps).
pub const SCHEMA_VERSION: u32 = 1;

/// Migration steps, index `i` upgrades version `i` to `i + 1`.
const STEPS: &[&str] = &[SCHEMA_V1];

/// Version 1: the normalized schema of docs/plan.md §5.
const SCHEMA_V1: &str = r#"
CREATE TABLE tasks(
  id TEXT PRIMARY KEY,
  subject TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','completed')),
  importance TEXT NOT NULL DEFAULT 'normal' CHECK (importance IN ('normal','high')),
  due_date TEXT,
  reminder_at TEXT,
  body TEXT,
  focus_date TEXT,
  postponed_date TEXT,
  created_at TEXT NOT NULL,
  completed_at TEXT,
  position REAL NOT NULL DEFAULT 0,
  spawned_from TEXT REFERENCES tasks(id),
  parent_id TEXT REFERENCES tasks(id),
  recurrence_type TEXT CHECK (recurrence_type IN ('daily','weekdays','weekly','monthly','yearly','custom')),
  recurrence_interval INTEGER,
  recurrence_unit TEXT CHECK (recurrence_unit IN ('day','week','month','year')),
  recurrence_days TEXT
);
CREATE TABLE steps(
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  subject TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','completed')),
  position REAL NOT NULL DEFAULT 0
);
CREATE TABLE views(
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  emoji TEXT,
  match_mode TEXT NOT NULL DEFAULT 'any' CHECK (match_mode IN ('any','all')),
  show_done INTEGER NOT NULL DEFAULT 0,
  position REAL NOT NULL DEFAULT 0
);
CREATE TABLE view_tags(
  view_id TEXT REFERENCES views(id) ON DELETE CASCADE,
  tag TEXT NOT NULL,
  PRIMARY KEY (view_id, tag)
);
CREATE TABLE view_sorts(
  view_id TEXT NOT NULL REFERENCES views(id) ON DELETE CASCADE,
  level INTEGER NOT NULL,
  by TEXT NOT NULL CHECK (by IN ('name','due','created','manual')),
  dir TEXT NOT NULL DEFAULT 'asc' CHECK (dir IN ('asc','desc')),
  PRIMARY KEY (view_id, level)
);
CREATE TABLE task_tags(
  task_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
  tag TEXT NOT NULL,
  PRIMARY KEY (task_id, tag)
);
CREATE TABLE trash(
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('task','view')),
  label TEXT,
  data TEXT NOT NULL,
  deleted_at TEXT NOT NULL
);
CREATE TABLE activity(
  id TEXT PRIMARY KEY,
  task_id TEXT,
  at TEXT NOT NULL,
  action TEXT NOT NULL,
  detail TEXT
);
CREATE TABLE settings(key TEXT PRIMARY KEY, value TEXT);
CREATE TABLE prefs(scope TEXT PRIMARY KEY, value TEXT);
CREATE TABLE fired_notifications(key TEXT PRIMARY KEY, at TEXT NOT NULL);
"#;

/// Bring `conn` up to [`SCHEMA_VERSION`], one transaction per step.
pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let mut version: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    while (version as usize) < STEPS.len() {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(STEPS[version as usize])?;
        version += 1;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
    }
    Ok(())
}
