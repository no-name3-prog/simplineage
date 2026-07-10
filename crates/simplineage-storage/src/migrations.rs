//! Schema migrations for the SQLite store.

use rusqlite::Connection;

use crate::error::{Result, StorageError};

/// Current schema version applied by this library.
pub const SCHEMA_VERSION: i32 = 1;

/// Apply all pending migrations to `conn`.
pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r"
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL
        );
        ",
    )?;

    let current: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    if current < 1 {
        apply_v1(conn)?;
    }

    let after: i32 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |r| r.get(0),
    )?;
    if after != SCHEMA_VERSION {
        return Err(StorageError::Migration(format!(
            "expected schema version {SCHEMA_VERSION}, got {after}"
        )));
    }
    Ok(())
}

fn apply_v1(conn: &Connection) -> Result<()> {
    tracing::info!("applying storage migration v1 (base metadata schema)");
    conn.execute_batch(
        r"
        CREATE TABLE IF NOT EXISTS snapshots (
            id TEXT PRIMARY KEY,
            model_version TEXT NOT NULL,
            label TEXT,
            created_at TEXT,
            source TEXT,
            parent_id TEXT,
            is_current INTEGER NOT NULL DEFAULT 0,
            payload TEXT NOT NULL,
            object_count INTEGER NOT NULL DEFAULT 0,
            edge_count INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS lineage_edges (
            snapshot_id TEXT NOT NULL,
            dep_id TEXT NOT NULL,
            from_id TEXT NOT NULL,
            to_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            level TEXT NOT NULL,
            PRIMARY KEY (snapshot_id, dep_id)
        );
        CREATE INDEX IF NOT EXISTS idx_edges_snapshot ON lineage_edges(snapshot_id);
        CREATE INDEX IF NOT EXISTS idx_edges_from ON lineage_edges(snapshot_id, from_id);
        CREATE INDEX IF NOT EXISTS idx_edges_to ON lineage_edges(snapshot_id, to_id);

        CREATE TABLE IF NOT EXISTS snapshot_objects (
            snapshot_id TEXT NOT NULL,
            object_id TEXT NOT NULL,
            object_type TEXT NOT NULL,
            fqn TEXT,
            PRIMARY KEY (snapshot_id, object_id)
        );
        CREATE INDEX IF NOT EXISTS idx_objects_snapshot ON snapshot_objects(snapshot_id);
        CREATE INDEX IF NOT EXISTS idx_objects_type ON snapshot_objects(snapshot_id, object_type);

        CREATE TABLE IF NOT EXISTS imports (
            id TEXT PRIMARY KEY,
            snapshot_id TEXT NOT NULL,
            parent_snapshot_id TEXT,
            mode TEXT NOT NULL,
            source TEXT,
            created_at TEXT NOT NULL,
            objects_added INTEGER NOT NULL DEFAULT 0,
            edges_added INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_imports_snapshot ON imports(snapshot_id);

        INSERT INTO schema_migrations(version, name, applied_at)
        VALUES (1, 'base_metadata_schema', datetime('now'));
        ",
    )?;
    Ok(())
}

/// Return applied migration versions.
pub fn applied_versions(conn: &Connection) -> Result<Vec<i32>> {
    let mut stmt = conn.prepare("SELECT version FROM schema_migrations ORDER BY version")?;
    let rows = stmt.query_map([], |r| r.get(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}
