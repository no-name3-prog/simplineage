//! Shared CLI context: loading snapshots from store or files.

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use simplineage_analysis::AnalysisEngine;
use simplineage_core::{ObjectId, Snapshot};
use simplineage_storage::MetadataStore;

use crate::output::OutputStyle;

/// Default local data directory for the SQLite store.
pub const DEFAULT_DATA_DIR: &str = ".simplineage";

/// Shared runtime options for commands that read lineage state.
#[derive(Debug, Clone)]
pub struct AppContext {
    /// SQLite data directory.
    pub data_dir: PathBuf,
    /// Output formatting.
    pub style: OutputStyle,
}

impl AppContext {
    /// Open the metadata store (creates dir + DB if needed).
    pub fn open_store(&self) -> anyhow::Result<MetadataStore> {
        MetadataStore::open(&self.data_dir)
            .with_context(|| format!("open store at {}", self.data_dir.display()))
    }

    /// Load the current head snapshot from the store.
    pub fn load_current_snapshot(&self) -> anyhow::Result<Snapshot> {
        let store = self.open_store()?;
        store
            .load_current()
            .context("load current snapshot")?
            .with_context(|| {
                format!(
                    "no current snapshot in store at {} — run `simplineage import` first",
                    self.data_dir.display()
                )
            })
    }

    /// Load a snapshot from a store id, JSON file path, or current head.
    ///
    /// Resolution order when `spec` is `Some`:
    /// 1. Existing file path → JSON snapshot
    /// 2. Snapshot id in the store
    ///
    /// When `spec` is `None`, loads the current head.
    pub fn load_snapshot(&self, spec: Option<&str>) -> anyhow::Result<Snapshot> {
        match spec {
            None => self.load_current_snapshot(),
            Some(s) => {
                let path = Path::new(s);
                if path.is_file() {
                    let bytes = std::fs::read(path)
                        .with_context(|| format!("read snapshot file {}", path.display()))?;
                    return Snapshot::from_json(bytes)
                        .with_context(|| format!("parse snapshot JSON {}", path.display()));
                }
                let store = self.open_store()?;
                let id = ObjectId::from_trusted(s.to_string());
                store
                    .load_snapshot(&id)
                    .with_context(|| format!("load snapshot id '{s}' from store"))
            }
        }
    }

    /// Build an analysis engine from a snapshot spec.
    pub fn analysis_engine(&self, spec: Option<&str>) -> anyhow::Result<AnalysisEngine> {
        let snap = self.load_snapshot(spec)?;
        Ok(AnalysisEngine::from_snapshot(snap))
    }
}

/// Resolve a user-supplied object reference (id or FQN fragment) against a snapshot.
pub fn resolve_object(snapshot: &Snapshot, query: &str) -> anyhow::Result<ObjectId> {
    resolve_object_ex(snapshot, query, None)
}

/// Resolve an object, optionally scoping to a column under a parent relation.
///
/// When `column` is `Some("email")` and `query` resolves to a table/view, returns
/// the child column whose leaf name matches (case-insensitive).
///
/// `query` may also be a full column FQN (`schema.table.column`) or id.
pub fn resolve_object_ex(
    snapshot: &Snapshot,
    query: &str,
    column: Option<&str>,
) -> anyhow::Result<ObjectId> {
    let q = query.trim();
    if q.is_empty() {
        bail!("object id / FQN must not be empty");
    }

    if let Some(col) = column.map(str::trim).filter(|c| !c.is_empty()) {
        return resolve_column_under(snapshot, q, col);
    }

    let index = snapshot.object_index();

    // Exact id
    let as_id = ObjectId::from_trusted(q.to_string());
    if index.contains_key(&as_id) {
        return Ok(as_id);
    }
    // Exact id among dependency endpoints (edge-only nodes)
    for dep in &snapshot.dependencies {
        if dep.from_id.as_str() == q {
            return Ok(dep.from_id.clone());
        }
        if dep.to_id.as_str() == q {
            return Ok(dep.to_id.clone());
        }
    }

    // Exact FQN (case-sensitive then case-insensitive)
    for (id, obj) in &index {
        if obj.meta().fqn.to_dotted() == q {
            return Ok(id.clone());
        }
    }
    let q_lower = q.to_ascii_lowercase();
    let exact_fqn: Vec<_> = index
        .iter()
        .filter(|(_, obj)| obj.meta().fqn.to_dotted().eq_ignore_ascii_case(q))
        .map(|(id, _)| id.clone())
        .collect();
    if exact_fqn.len() == 1 {
        return Ok(exact_fqn[0].clone());
    }

    // Prefer column when query looks like schema.table.column (≥3 dotted parts)
    let part_count = q.split('.').filter(|p| !p.is_empty()).count();
    if part_count >= 3 {
        let col_hits: Vec<_> = index
            .iter()
            .filter(|(_, obj)| {
                obj.kind_name() == "column"
                    && (obj.meta().fqn.to_dotted().eq_ignore_ascii_case(q)
                        || obj.meta().fqn.to_dotted().to_ascii_lowercase() == q_lower)
            })
            .map(|(id, obj)| (id.clone(), obj.meta().fqn.to_dotted()))
            .collect();
        if col_hits.len() == 1 {
            return Ok(col_hits[0].0.clone());
        }
    }

    // Case-insensitive id / fqn / name
    let mut matches: Vec<(ObjectId, String, &'static str)> = Vec::new();
    for (id, obj) in &index {
        let fqn = obj.meta().fqn.to_dotted();
        let name = obj.meta().name.clone();
        if id.as_str().eq_ignore_ascii_case(q)
            || fqn.eq_ignore_ascii_case(q)
            || name.eq_ignore_ascii_case(q)
            || fqn.to_ascii_lowercase().contains(&q_lower)
            || id.as_str().to_ascii_lowercase().contains(&q_lower)
        {
            matches.push((id.clone(), fqn, obj.kind_name()));
        }
    }

    // Also match edge-only endpoints by substring
    for dep in &snapshot.dependencies {
        for endpoint in [&dep.from_id, &dep.to_id] {
            if endpoint.as_str().to_ascii_lowercase().contains(&q_lower)
                && !matches.iter().any(|(i, _, _)| i == endpoint)
            {
                matches.push((endpoint.clone(), endpoint.to_string(), "unknown"));
            }
        }
    }

    match matches.len() {
        0 => bail!("no object matched '{q}'"),
        1 => Ok(matches.remove(0).0),
        _ => {
            // Prefer exact leaf name match
            let exact_name: Vec<_> = matches
                .iter()
                .filter(|(id, fqn, _)| {
                    index
                        .get(id)
                        .map(|o| o.meta().name.eq_ignore_ascii_case(q))
                        .unwrap_or(false)
                        || fqn
                            .rsplit('.')
                            .next()
                            .is_some_and(|leaf| leaf.eq_ignore_ascii_case(q))
                })
                .cloned()
                .collect();
            if exact_name.len() == 1 {
                return Ok(exact_name[0].0.clone());
            }
            // When the query has 3+ parts, prefer columns among exact leaf matches
            if part_count >= 3 {
                let cols: Vec<_> = exact_name
                    .iter()
                    .filter(|(_, _, kind)| *kind == "column")
                    .cloned()
                    .collect();
                if cols.len() == 1 {
                    return Ok(cols[0].0.clone());
                }
            }
            let preview: Vec<String> = matches
                .iter()
                .take(8)
                .map(|(id, fqn, kind)| format!("{id} ({kind}: {fqn})"))
                .collect();
            bail!(
                "ambiguous object '{q}' — matches {}: {}",
                matches.len(),
                preview.join(", ")
            )
        }
    }
}

/// Resolve `column_name` under a parent relation identified by `parent_query`.
fn resolve_column_under(
    snapshot: &Snapshot,
    parent_query: &str,
    column_name: &str,
) -> anyhow::Result<ObjectId> {
    let parent = resolve_object(snapshot, parent_query)?;
    let col_lower = column_name.to_ascii_lowercase();

    // Direct child columns via parent_id
    let mut hits: Vec<(ObjectId, String)> = snapshot
        .columns
        .iter()
        .filter(|c| c.parent_id == parent)
        .filter(|c| {
            c.meta.name.eq_ignore_ascii_case(column_name)
                || c.meta
                    .fqn
                    .to_dotted()
                    .to_ascii_lowercase()
                    .ends_with(&format!(".{col_lower}"))
        })
        .map(|c| (c.meta.id.clone(), c.meta.fqn.to_dotted()))
        .collect();

    if hits.is_empty() {
        // Fallback: FQN = parent_fqn.column or id contains parent + column
        let index = snapshot.object_index();
        let parent_fqn = index
            .get(&parent)
            .map(|o| o.meta().fqn.to_dotted())
            .unwrap_or_else(|| parent.to_string());
        let want = format!("{parent_fqn}.{column_name}").to_ascii_lowercase();
        for c in &snapshot.columns {
            let fqn = c.meta.fqn.to_dotted().to_ascii_lowercase();
            if fqn == want
                || (fqn.ends_with(&format!(".{col_lower}"))
                    && fqn.contains(&parent_fqn.to_ascii_lowercase()))
            {
                hits.push((c.meta.id.clone(), c.meta.fqn.to_dotted()));
            }
        }
    }

    // Dedup
    hits.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
    hits.dedup_by(|a, b| a.0 == b.0);

    match hits.len() {
        0 => bail!(
            "no column '{column_name}' under object resolved from '{parent_query}' ({parent})"
        ),
        1 => Ok(hits.remove(0).0),
        _ => {
            let preview: Vec<String> = hits
                .iter()
                .take(8)
                .map(|(id, fqn)| format!("{id} ({fqn})"))
                .collect();
            bail!(
                "ambiguous column '{column_name}' under '{parent_query}': {}",
                preview.join(", ")
            )
        }
    }
}

/// Display label for an object (prefer FQN, fall back to id).
pub fn object_display(snapshot: &Snapshot, id: &ObjectId) -> String {
    if let Some(obj) = snapshot.object_index().get(id) {
        let fqn = obj.meta().fqn.to_dotted();
        if !fqn.is_empty() {
            return fqn;
        }
    }
    id.to_string()
}

/// Kind label for an object when known.
pub fn object_kind_label(snapshot: &Snapshot, id: &ObjectId) -> &'static str {
    snapshot
        .object_index()
        .get(id)
        .map(|o| o.kind_name())
        .unwrap_or("unknown")
}

/// Search hit (owned fields for display).
#[derive(Debug, Clone)]
pub struct SearchMatch {
    /// Object id.
    pub id: ObjectId,
    /// Kind name.
    pub kind: String,
    /// Fully qualified name.
    pub fqn: String,
    /// Simple name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
}

/// Search objects by free-text query.
pub fn search_objects(
    snapshot: &Snapshot,
    query: &str,
    kind: Option<&str>,
    limit: usize,
) -> Vec<SearchMatch> {
    let q = query.trim().to_ascii_lowercase();
    let mut out = Vec::new();
    for (id, obj) in snapshot.object_index() {
        if let Some(k) = kind {
            if !obj.kind_name().eq_ignore_ascii_case(k) {
                continue;
            }
        }
        if q.is_empty()
            || id.as_str().to_ascii_lowercase().contains(&q)
            || obj.meta().fqn.to_dotted().to_ascii_lowercase().contains(&q)
            || obj.meta().name.to_ascii_lowercase().contains(&q)
            || obj
                .meta()
                .description
                .as_deref()
                .map(|d| d.to_ascii_lowercase().contains(&q))
                .unwrap_or(false)
        {
            out.push(SearchMatch {
                id,
                kind: obj.kind_name().to_string(),
                fqn: obj.meta().fqn.to_dotted(),
                name: obj.meta().name.clone(),
                description: obj.meta().description.clone(),
            });
            if out.len() >= limit {
                break;
            }
        }
    }
    out
}

/// Diff two snapshots at the object/edge id level.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SnapshotDiff {
    /// Left snapshot id.
    pub left_id: String,
    /// Right snapshot id.
    pub right_id: String,
    /// Object ids only in left.
    pub objects_only_left: Vec<String>,
    /// Object ids only in right.
    pub objects_only_right: Vec<String>,
    /// Objects present in both (by id).
    pub objects_shared: usize,
    /// Dependency ids only in left.
    pub edges_only_left: Vec<String>,
    /// Dependency ids only in right.
    pub edges_only_right: Vec<String>,
    /// Edges shared.
    pub edges_shared: usize,
    /// Counts.
    pub left_object_count: usize,
    /// Counts.
    pub right_object_count: usize,
    /// Counts.
    pub left_edge_count: usize,
    /// Counts.
    pub right_edge_count: usize,
}

/// Compare two snapshots by object and dependency ids.
pub fn compare_snapshots(left: &Snapshot, right: &Snapshot) -> SnapshotDiff {
    use std::collections::BTreeSet;

    let left_objs: BTreeSet<_> = left.object_index().into_keys().collect();
    let right_objs: BTreeSet<_> = right.object_index().into_keys().collect();
    let left_edges: BTreeSet<_> = left.dependencies.iter().map(|d| d.id.clone()).collect();
    let right_edges: BTreeSet<_> = right.dependencies.iter().map(|d| d.id.clone()).collect();

    let objects_only_left: Vec<String> = left_objs
        .difference(&right_objs)
        .map(|o| o.to_string())
        .collect();
    let objects_only_right: Vec<String> = right_objs
        .difference(&left_objs)
        .map(|o| o.to_string())
        .collect();
    let objects_shared = left_objs.intersection(&right_objs).count();

    let edges_only_left: Vec<String> = left_edges
        .difference(&right_edges)
        .map(|o| o.to_string())
        .collect();
    let edges_only_right: Vec<String> = right_edges
        .difference(&left_edges)
        .map(|o| o.to_string())
        .collect();
    let edges_shared = left_edges.intersection(&right_edges).count();

    SnapshotDiff {
        left_id: left.id.to_string(),
        right_id: right.id.to_string(),
        objects_only_left,
        objects_only_right,
        objects_shared,
        edges_only_left,
        edges_only_right,
        edges_shared,
        left_object_count: left.object_count(),
        right_object_count: right.object_count(),
        left_edge_count: left.dependencies.len(),
        right_edge_count: right.dependencies.len(),
    }
}
