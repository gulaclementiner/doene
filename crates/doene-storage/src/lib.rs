//! Döne storage: SQLite persistence, migrations, and JSON export/import.
//!
//! Depends on `doene-core` only (docs/plan.md §4). Owns the schema of
//! docs/plan.md §5 and maps rows to and from core entities. It holds no
//! domain rules: those live in `doene-core` and the `doene-service` layer.

use std::path::Path;

use rusqlite::Connection;

pub mod migrations;

pub use migrations::SCHEMA_VERSION;

/// Errors raised by the storage layer (I/O or SQLite failures).
pub type StorageResult<T> = Result<T, rusqlite::Error>;

/// An open Döne database.
#[derive(Debug)]
pub struct Storage {
    conn: Connection,
}

impl Storage {
    /// Open (or create) the database file at `path`, applying migrations.
    pub fn open(path: &Path) -> StorageResult<Self> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Open a fresh in-memory database, applying migrations.
    pub fn open_in_memory() -> StorageResult<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(conn: Connection) -> StorageResult<Self> {
        conn.pragma_update(None, "foreign_keys", true)?;
        migrations::migrate(&conn)?;
        Ok(Self { conn })
    }

    /// The underlying connection (crate-internal access for repositories).
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// The schema version recorded in `PRAGMA user_version`.
    pub fn schema_version(&self) -> StorageResult<u32> {
        self.conn.query_row("PRAGMA user_version", [], |r| r.get(0))
    }
}
