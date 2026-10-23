//! Slice 1: schema and migrations apply cleanly and hold the plan §5 conventions.

use doene_storage::Storage;

fn tables(store: &Storage) -> Vec<String> {
    let conn = store.conn();
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .expect("prepare");
    stmt.query_map([], |r| r.get::<_, String>(0))
        .expect("query")
        .map(|r| r.expect("row"))
        .collect()
}

#[test]
fn fresh_database_gets_every_plan_table_at_version_1() {
    let store = Storage::open_in_memory().expect("open");
    assert_eq!(store.schema_version().expect("version"), 1);
    assert_eq!(
        tables(&store),
        [
            "activity",
            "fired_notifications",
            "prefs",
            "settings",
            "steps",
            "task_tags",
            "tasks",
            "trash",
            "view_sorts",
            "view_tags",
            "views",
        ]
    );
}

#[test]
fn reopening_an_existing_database_keeps_data_and_version() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("doene.db");
    {
        let store = Storage::open(&path).expect("first open");
        store
            .conn()
            .execute(
                "INSERT INTO settings (key, value) VALUES ('theme', '\"dark\"')",
                [],
            )
            .expect("write");
    }
    let store = Storage::open(&path).expect("reopen");
    assert_eq!(store.schema_version().expect("version"), 1);
    let theme: String = store
        .conn()
        .query_row("SELECT value FROM settings WHERE key='theme'", [], |r| {
            r.get(0)
        })
        .expect("read");
    assert_eq!(theme, "\"dark\"");
}

#[test]
fn check_constraints_reject_out_of_domain_values() {
    let store = Storage::open_in_memory().expect("open");
    let conn = store.conn();
    let bad_status = conn.execute(
        "INSERT INTO tasks (id, subject, status, created_at) VALUES ('a', 'x', 'notStarted', '2025-01-01T00:00:00.000Z')",
        [],
    );
    assert!(bad_status.is_err(), "status must be open|completed");
    let bad_match = conn.execute(
        "INSERT INTO views (id, name, match_mode) VALUES ('v', 'n', 'some')",
        [],
    );
    assert!(bad_match.is_err(), "match_mode must be any|all");
}

#[test]
fn foreign_keys_are_enforced_and_steps_cascade() {
    let store = Storage::open_in_memory().expect("open");
    let conn = store.conn();
    conn.execute(
        "INSERT INTO tasks (id, subject, created_at) VALUES ('t1', 'a', '2025-01-01T00:00:00.000Z')",
        [],
    )
    .expect("insert task");
    conn.execute(
        "INSERT INTO steps (id, task_id, subject, position) VALUES ('s1', 't1', 'step', 1)",
        [],
    )
    .expect("insert step");
    assert!(
        conn.execute(
            "INSERT INTO steps (id, task_id, subject, position) VALUES ('s2', 'missing', 'x', 1)",
            []
        )
        .is_err(),
        "orphan step rejected"
    );
    conn.execute("DELETE FROM tasks WHERE id='t1'", [])
        .expect("delete");
    let steps: i64 = conn
        .query_row("SELECT COUNT(*) FROM steps", [], |r| r.get(0))
        .expect("count");
    assert_eq!(steps, 0, "steps cascade with their task");
}
