//! SQLite-backed metadata store.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use simplineage_core::Snapshot;
use simplineage_core::model::graph::{Dependency, DependencyKind, DependencyLevel};
use simplineage_core::model::ids::ObjectId;
use uuid::Uuid;

use crate::error::{Result, StorageError};
use crate::merge::{ImportMode, merge_snapshots};
use crate::migrations::{self, SCHEMA_VERSION};

/// Lightweight snapshot catalog row (without full payload).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotMeta {
    /// Snapshot id.
    pub id: ObjectId,
    /// Model schema version string.
    pub model_version: String,
    /// Optional label.
    pub label: Option<String>,
    /// Created-at timestamp string.
    pub created_at: Option<String>,
    /// Source system.
    pub source: Option<String>,
    /// Parent snapshot id (incremental lineage).
    pub parent_id: Option<ObjectId>,
    /// Whether this is the current head snapshot.
    pub is_current: bool,
    /// Object count at save time.
    pub object_count: i64,
    /// Edge count at save time.
    pub edge_count: i64,
}

/// Result of an import into the store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportResult {
    /// Resulting snapshot id.
    pub snapshot_id: ObjectId,
    /// Import audit id.
    pub import_id: ObjectId,
    /// Mode used.
    pub mode: String,
    /// Objects added relative to parent (merge heuristic).
    pub objects_added: i64,
    /// Edges added relative to parent.
    pub edges_added: i64,
}

/// Local metadata persistence using **SQLite** (lightweight, offline-first).
///
/// File layout under `data_dir`:
/// - `metadata.sqlite` — catalog, snapshots, edges, imports
pub struct MetadataStore {
    data_dir: PathBuf,
    db_path: PathBuf,
    conn: Connection,
}

impl std::fmt::Debug for MetadataStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetadataStore")
            .field("data_dir", &self.data_dir)
            .field("db_path", &self.db_path)
            .finish_non_exhaustive()
    }
}

impl MetadataStore {
    /// Open (or create) a store at `data_dir`, running migrations.
    pub fn open(data_dir: impl Into<PathBuf>) -> Result<Self> {
        let data_dir = data_dir.into();
        std::fs::create_dir_all(&data_dir)?;
        let db_path = data_dir.join("metadata.sqlite");
        let conn = Connection::open(&db_path)?;
        conn.execute_batch(
            r"
            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = WAL;
            ",
        )?;
        migrations::migrate(&conn)?;
        tracing::info!(?db_path, schema = SCHEMA_VERSION, "opened metadata store");
        Ok(Self {
            data_dir,
            db_path,
            conn,
        })
    }

    /// Open an in-memory store (tests / ephemeral).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        migrations::migrate(&conn)?;
        Ok(Self {
            data_dir: PathBuf::from(":memory:"),
            db_path: PathBuf::from(":memory:"),
            conn,
        })
    }

    /// Data directory root.
    #[must_use]
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Path to the SQLite file.
    #[must_use]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Schema version of this library / applied migrations.
    #[must_use]
    pub fn schema_version(&self) -> i32 {
        SCHEMA_VERSION
    }

    /// Applied migration versions.
    pub fn applied_migrations(&self) -> Result<Vec<i32>> {
        migrations::applied_versions(&self.conn)
    }

    // ── Snapshots ───────────────────────────────────────────────────────

    /// Persist a snapshot (full JSON payload + materialized edges/objects).
    pub fn save_snapshot(&self, snapshot: &Snapshot, set_current: bool) -> Result<()> {
        self.save_snapshot_with_parent(snapshot, None, set_current)
    }

    /// Save snapshot with optional parent linkage.
    pub fn save_snapshot_with_parent(
        &self,
        snapshot: &Snapshot,
        parent_id: Option<&ObjectId>,
        set_current: bool,
    ) -> Result<()> {
        let payload = snapshot.to_json()?;
        let object_count = snapshot.object_count() as i64;
        let edge_count = snapshot.dependencies.len() as i64;
        let id = snapshot.id.as_str();

        let tx = self.conn.unchecked_transaction()?;

        if set_current {
            tx.execute(
                "UPDATE snapshots SET is_current = 0 WHERE is_current = 1",
                [],
            )?;
        }

        tx.execute(
            r"
            INSERT INTO snapshots
              (id, model_version, label, created_at, source, parent_id, is_current,
               payload, object_count, edge_count)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(id) DO UPDATE SET
              model_version = excluded.model_version,
              label = excluded.label,
              created_at = excluded.created_at,
              source = excluded.source,
              parent_id = excluded.parent_id,
              is_current = excluded.is_current,
              payload = excluded.payload,
              object_count = excluded.object_count,
              edge_count = excluded.edge_count
            ",
            params![
                id,
                snapshot.model_version.as_str(),
                snapshot.label.as_deref(),
                snapshot.created_at.as_deref(),
                snapshot.source.as_deref(),
                parent_id.map(|p| p.as_str()),
                i32::from(set_current),
                payload,
                object_count,
                edge_count,
            ],
        )?;

        tx.execute(
            "DELETE FROM lineage_edges WHERE snapshot_id = ?1",
            params![id],
        )?;
        tx.execute(
            "DELETE FROM snapshot_objects WHERE snapshot_id = ?1",
            params![id],
        )?;

        {
            let mut edge_stmt = tx.prepare(
                r"
                INSERT INTO lineage_edges(snapshot_id, dep_id, from_id, to_id, kind, level)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ",
            )?;
            for dep in &snapshot.dependencies {
                edge_stmt.execute(params![
                    id,
                    dep.id.as_str(),
                    dep.from_id.as_str(),
                    dep.to_id.as_str(),
                    dep_kind_str(&dep.kind),
                    dep_level_str(dep.level),
                ])?;
            }
        }

        {
            let mut obj_stmt = tx.prepare(
                r"
                INSERT INTO snapshot_objects(snapshot_id, object_id, object_type, fqn)
                VALUES (?1, ?2, ?3, ?4)
                ",
            )?;
            for (oid, obj) in snapshot.object_index() {
                obj_stmt.execute(params![
                    id,
                    oid.as_str(),
                    obj.kind_name(),
                    obj.meta().fqn.to_dotted(),
                ])?;
            }
        }

        tx.commit()?;
        tracing::debug!(snapshot_id = %id, object_count, edge_count, "saved snapshot");
        Ok(())
    }

    /// Load a full snapshot by id.
    pub fn load_snapshot(&self, id: &ObjectId) -> Result<Snapshot> {
        let payload: String = self
            .conn
            .query_row(
                "SELECT payload FROM snapshots WHERE id = ?1",
                params![id.as_str()],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| StorageError::NotFound(id.to_string()))?;
        Ok(Snapshot::from_json(payload.as_bytes())?)
    }

    /// Load the current (head) snapshot, if any.
    pub fn load_current(&self) -> Result<Option<Snapshot>> {
        let payload: Option<String> = self
            .conn
            .query_row(
                "SELECT payload FROM snapshots WHERE is_current = 1 LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        match payload {
            Some(p) => Ok(Some(Snapshot::from_json(p.as_bytes())?)),
            None => Ok(None),
        }
    }

    /// List snapshot metadata (newest first).
    pub fn list_snapshots(&self) -> Result<Vec<SnapshotMeta>> {
        let mut stmt = self.conn.prepare(
            r"
            SELECT id, model_version, label, created_at, source, parent_id, is_current,
                   object_count, edge_count
            FROM snapshots
            ORDER BY (created_at IS NULL), created_at DESC, id DESC
            ",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(SnapshotMeta {
                id: ObjectId::from_trusted(r.get::<_, String>(0)?),
                model_version: r.get(1)?,
                label: r.get(2)?,
                created_at: r.get(3)?,
                source: r.get(4)?,
                parent_id: r.get::<_, Option<String>>(5)?.map(ObjectId::from_trusted),
                is_current: r.get::<_, i32>(6)? != 0,
                object_count: r.get(7)?,
                edge_count: r.get(8)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Mark a snapshot as current.
    pub fn set_current(&self, id: &ObjectId) -> Result<()> {
        let exists: bool = self.conn.query_row(
            "SELECT COUNT(*) > 0 FROM snapshots WHERE id = ?1",
            params![id.as_str()],
            |r| r.get(0),
        )?;
        if !exists {
            return Err(StorageError::NotFound(id.to_string()));
        }
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE snapshots SET is_current = 0 WHERE is_current = 1",
            [],
        )?;
        tx.execute(
            "UPDATE snapshots SET is_current = 1 WHERE id = ?1",
            params![id.as_str()],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Delete a snapshot and its materialized rows.
    pub fn delete_snapshot(&self, id: &ObjectId) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM lineage_edges WHERE snapshot_id = ?1",
            params![id.as_str()],
        )?;
        tx.execute(
            "DELETE FROM snapshot_objects WHERE snapshot_id = ?1",
            params![id.as_str()],
        )?;
        let n = tx.execute("DELETE FROM snapshots WHERE id = ?1", params![id.as_str()])?;
        tx.commit()?;
        if n == 0 {
            return Err(StorageError::NotFound(id.to_string()));
        }
        Ok(())
    }

    // ── Incremental imports ─────────────────────────────────────────────

    /// Import a snapshot, optionally merging with the current head.
    pub fn import(
        &self,
        incoming: Snapshot,
        mode: ImportMode,
        source: Option<&str>,
    ) -> Result<ImportResult> {
        let parent = self.load_current()?;
        let parent_id = parent.as_ref().map(|p| p.id.clone());

        let (result_snap, objects_added, edges_added) = match mode {
            ImportMode::Replace => {
                let obj = incoming.object_count() as i64;
                let edges = incoming.dependencies.len() as i64;
                (incoming, obj, edges)
            }
            ImportMode::Merge => {
                if let Some(base) = parent.as_ref() {
                    let before_obj = base.object_count() as i64;
                    let before_edges = base.dependencies.len() as i64;
                    let merged = merge_snapshots(base, &incoming);
                    let objects_added = (merged.object_count() as i64 - before_obj).max(0);
                    let edges_added = (merged.dependencies.len() as i64 - before_edges).max(0);
                    (merged, objects_added, edges_added)
                } else {
                    let obj = incoming.object_count() as i64;
                    let edges = incoming.dependencies.len() as i64;
                    (incoming, obj, edges)
                }
            }
        };

        let mut snap = result_snap;
        if let Some(src) = source {
            snap.source = Some(src.to_string());
        }
        if snap.created_at.is_none() {
            snap.created_at = Some(now_string());
        }

        self.save_snapshot_with_parent(&snap, parent_id.as_ref(), true)?;

        let import_id = ObjectId::from_trusted(Uuid::new_v4().to_string());
        self.conn.execute(
            r"
            INSERT INTO imports(id, snapshot_id, parent_snapshot_id, mode, source, created_at,
                                objects_added, edges_added)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
            params![
                import_id.as_str(),
                snap.id.as_str(),
                parent_id.as_ref().map(|p| p.as_str()),
                mode.as_str(),
                source,
                snap.created_at.as_deref().unwrap_or(""),
                objects_added,
                edges_added,
            ],
        )?;

        Ok(ImportResult {
            snapshot_id: snap.id,
            import_id,
            mode: mode.as_str().to_string(),
            objects_added,
            edges_added,
        })
    }

    // ── Fast graph reconstruction ───────────────────────────────────────

    /// Load dependency edges for a snapshot from the materialized edge table
    /// (avoids deserializing the full JSON payload).
    pub fn load_dependencies(&self, snapshot_id: &ObjectId) -> Result<Vec<Dependency>> {
        let mut stmt = self.conn.prepare(
            r"
            SELECT dep_id, from_id, to_id, kind, level
            FROM lineage_edges
            WHERE snapshot_id = ?1
            ",
        )?;
        let rows = stmt.query_map(params![snapshot_id.as_str()], |r| {
            let dep_id: String = r.get(0)?;
            let from_id: String = r.get(1)?;
            let to_id: String = r.get(2)?;
            let kind: String = r.get(3)?;
            let level: String = r.get(4)?;
            Ok(Dependency {
                id: ObjectId::from_trusted(dep_id),
                from_id: ObjectId::from_trusted(from_id),
                to_id: ObjectId::from_trusted(to_id),
                kind: parse_dep_kind(&kind),
                level: parse_dep_level(&level),
                confidence: None,
                attributes: Default::default(),
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Load dependencies for the current snapshot (empty if none).
    pub fn load_current_dependencies(&self) -> Result<Vec<Dependency>> {
        let id: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM snapshots WHERE is_current = 1 LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        match id {
            Some(id) => self.load_dependencies(&ObjectId::from_trusted(id)),
            None => Ok(Vec::new()),
        }
    }

    /// Count objects of a given type in a snapshot (from index).
    pub fn count_objects(&self, snapshot_id: &ObjectId, object_type: Option<&str>) -> Result<i64> {
        if let Some(ty) = object_type {
            Ok(self.conn.query_row(
                "SELECT COUNT(*) FROM snapshot_objects WHERE snapshot_id = ?1 AND object_type = ?2",
                params![snapshot_id.as_str(), ty],
                |r| r.get(0),
            )?)
        } else {
            Ok(self.conn.query_row(
                "SELECT COUNT(*) FROM snapshot_objects WHERE snapshot_id = ?1",
                params![snapshot_id.as_str()],
                |r| r.get(0),
            )?)
        }
    }
}

/// Backward-compatible thin alias used by older call sites.
#[derive(Debug)]
pub struct Store {
    inner: MetadataStore,
}

impl Store {
    /// Open store at `data_dir`.
    pub fn open(data_dir: impl Into<PathBuf>) -> simplineage_core::Result<Self> {
        MetadataStore::open(data_dir)
            .map(|inner| Self { inner })
            .map_err(|e| simplineage_core::Error::Other(anyhow::anyhow!(e)))
    }

    /// Data directory.
    #[must_use]
    pub fn data_dir(&self) -> &Path {
        self.inner.data_dir()
    }

    /// Access the full metadata store API.
    #[must_use]
    pub fn metadata(&self) -> &MetadataStore {
        &self.inner
    }
}

fn dep_kind_str(k: &DependencyKind) -> String {
    match k {
        DependencyKind::ViewDefinition => "view_definition".into(),
        DependencyKind::Pipeline => "pipeline".into(),
        DependencyKind::ForeignKey => "foreign_key".into(),
        DependencyKind::Manual => "manual".into(),
        DependencyKind::Inferred => "inferred".into(),
        DependencyKind::Other(s) => format!("other:{s}"),
    }
}

fn dep_level_str(l: DependencyLevel) -> &'static str {
    match l {
        DependencyLevel::Relation => "relation",
        DependencyLevel::Column => "column",
        DependencyLevel::Unknown => "unknown",
    }
}

fn parse_dep_kind(s: &str) -> DependencyKind {
    if let Some(rest) = s.strip_prefix("other:") {
        return DependencyKind::Other(rest.to_string());
    }
    match s {
        "view_definition" => DependencyKind::ViewDefinition,
        "pipeline" => DependencyKind::Pipeline,
        "foreign_key" => DependencyKind::ForeignKey,
        "manual" => DependencyKind::Manual,
        "inferred" => DependencyKind::Inferred,
        other => DependencyKind::Other(other.to_string()),
    }
}

fn parse_dep_level(s: &str) -> DependencyLevel {
    match s {
        "relation" => DependencyLevel::Relation,
        "column" => DependencyLevel::Column,
        _ => DependencyLevel::Unknown,
    }
}

fn now_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix:{secs}")
}
