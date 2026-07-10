//! Importer registry: registration, auto-detection, and import orchestration.

use std::path::{Path, PathBuf};

use simplineage_core::{Error, Result, Snapshot};

use crate::formats::{CsvImporter, ExcelImporter, JsonImporter, ParquetImporter};
use crate::options::ImportOptions;
use crate::plugin::{DetectConfidence, MetadataImporter};

/// Registry of metadata import plugins.
///
/// Built-in format importers are registered by [`ImporterRegistry::with_builtins`].
/// Warehouse plugins register additional crates via [`ImporterRegistry::register`].
pub struct ImporterRegistry {
    importers: Vec<Box<dyn MetadataImporter>>,
}

impl std::fmt::Debug for ImporterRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImporterRegistry")
            .field(
                "importers",
                &self.importers.iter().map(|i| i.id()).collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl Default for ImporterRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

impl ImporterRegistry {
    /// Empty registry (no plugins).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            importers: Vec::new(),
        }
    }

    /// Registry with CSV, JSON, Parquet, and Excel plugins.
    #[must_use]
    pub fn with_builtins() -> Self {
        let mut reg = Self::empty();
        reg.register(Box::new(CsvImporter));
        reg.register(Box::new(JsonImporter));
        reg.register(Box::new(ParquetImporter));
        reg.register(Box::new(ExcelImporter));
        reg
    }

    /// Register a plugin. Later registrations win ties of equal confidence when
    /// listed later (we still pick highest confidence first; ties break by order).
    pub fn register(&mut self, importer: Box<dyn MetadataImporter>) {
        // Replace same id if re-registered
        if let Some(pos) = self.importers.iter().position(|i| i.id() == importer.id()) {
            self.importers[pos] = importer;
        } else {
            self.importers.push(importer);
        }
    }

    /// List registered importer ids.
    #[must_use]
    pub fn ids(&self) -> Vec<&'static str> {
        self.importers.iter().map(|i| i.id()).collect()
    }

    /// Get importer by id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&dyn MetadataImporter> {
        self.importers
            .iter()
            .find(|i| i.id() == id)
            .map(|i| i.as_ref())
    }

    /// Auto-detect the best importer for `path`.
    #[must_use]
    pub fn detect(&self, path: &Path) -> Option<DetectResult<'_>> {
        let mut best: Option<DetectResult<'_>> = None;
        for imp in &self.importers {
            let conf = imp.detect(path);
            if conf == DetectConfidence::None {
                continue;
            }
            match &best {
                None => {
                    best = Some(DetectResult {
                        importer: imp.as_ref(),
                        confidence: conf,
                    });
                }
                Some(cur) if conf > cur.confidence => {
                    best = Some(DetectResult {
                        importer: imp.as_ref(),
                        confidence: conf,
                    });
                }
                Some(cur) if conf == cur.confidence => {
                    // Prefer later registration (warehouse plugins register after builtins).
                    best = Some(DetectResult {
                        importer: imp.as_ref(),
                        confidence: conf,
                    });
                }
                _ => {}
            }
        }
        best
    }

    /// Detect all importers with non-none confidence, sorted by confidence desc.
    #[must_use]
    pub fn detect_all(&self, path: &Path) -> Vec<DetectResult<'_>> {
        let mut out: Vec<_> = self
            .importers
            .iter()
            .filter_map(|imp| {
                let confidence = imp.detect(path);
                (confidence != DetectConfidence::None).then_some(DetectResult {
                    importer: imp.as_ref(),
                    confidence,
                })
            })
            .collect();
        out.sort_by_key(|b| std::cmp::Reverse(b.confidence));
        out
    }

    /// Import using auto-detection.
    pub fn import_path(&self, path: &Path, options: &ImportOptions) -> Result<Snapshot> {
        let path = normalize_path(path)?;
        let detected = self.detect(&path).ok_or_else(|| {
            Error::import(format!(
                "no importer detected for {} (supported: {})",
                path.display(),
                self.ids().join(", ")
            ))
        })?;
        tracing::info!(
            importer = detected.importer.id(),
            confidence = ?detected.confidence,
            path = %path.display(),
            "auto-detected metadata importer"
        );
        let mut opts = options.clone();
        if opts.origin.is_none() {
            opts.origin = Some(path.clone());
        }
        detected.importer.import(&path, &opts)
    }

    /// Import using a specific importer id.
    pub fn import_with(
        &self,
        importer_id: &str,
        path: &Path,
        options: &ImportOptions,
    ) -> Result<Snapshot> {
        let path = normalize_path(path)?;
        let imp = self
            .get(importer_id)
            .ok_or_else(|| Error::import(format!("unknown importer id '{importer_id}'")))?;
        let mut opts = options.clone();
        if opts.origin.is_none() {
            opts.origin = Some(path.clone());
        }
        imp.import(&path, &opts)
    }

    /// Import a directory: import every recognized file and merge snapshots.
    pub fn import_dir(&self, dir: &Path, options: &ImportOptions) -> Result<Snapshot> {
        if !dir.is_dir() {
            return Err(Error::import(format!("not a directory: {}", dir.display())));
        }
        let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
            .map_err(|e| Error::import(format!("read_dir {}: {e}", dir.display())))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                if !p.is_file() || self.detect(p).is_none() {
                    return false;
                }
                // Skip docs/config files that may weakly match text formats.
                let ext = p
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                matches!(
                    ext.as_str(),
                    "csv" | "tsv" | "json" | "parquet" | "parq" | "xlsx" | "xlsm" | "xls"
                )
            })
            .collect();
        // Prefer table catalogs, then columns, then lineage/other (BigQuery dumps).
        paths.sort_by_key(|p| {
            let name = p
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let rank = if name.contains("table") && !name.contains("column") {
                0
            } else if name.contains("column") {
                1
            } else if name.contains("lineage") || name.contains("depend") {
                2
            } else {
                3
            };
            (rank, name)
        });
        if paths.is_empty() {
            return Err(Error::import(format!(
                "no supported metadata files in {}",
                dir.display()
            )));
        }

        let mut merged: Option<Snapshot> = None;
        for path in paths {
            let snap = self.import_path(&path, options)?;
            merged = Some(match merged {
                None => snap,
                Some(acc) => merge_snapshots(acc, snap),
            });
        }
        let mut out = merged.unwrap();
        if let Some(label) = &options.label {
            out.label = Some(label.clone());
        }
        reconcile_relation_aliases(&mut out);
        if !options.skip_validation {
            use simplineage_core::model::Validate;
            out.validate()?;
        }
        Ok(out)
    }
}

/// Result of format detection.
#[derive(Clone, Copy)]
pub struct DetectResult<'a> {
    /// Chosen importer.
    pub importer: &'a dyn MetadataImporter,
    /// Confidence score.
    pub confidence: DetectConfidence,
}

fn normalize_path(path: &Path) -> Result<PathBuf> {
    if !path.exists() {
        return Err(Error::import(format!("path not found: {}", path.display())));
    }
    Ok(path.to_path_buf())
}

/// Merge snapshots by object/edge id (incoming wins on conflict).
fn merge_snapshots(mut a: Snapshot, b: Snapshot) -> Snapshot {
    a.catalogs = union_by_id(a.catalogs, b.catalogs, |c| c.meta.id.as_str());
    a.databases = union_by_id(a.databases, b.databases, |d| d.meta.id.as_str());
    a.schemas = union_by_id(a.schemas, b.schemas, |s| s.meta.id.as_str());
    a.tables = union_by_id(a.tables, b.tables, |t| t.meta.id.as_str());
    a.views = union_by_id(a.views, b.views, |v| v.meta.id.as_str());
    a.materialized_views = union_by_id(a.materialized_views, b.materialized_views, |m| {
        m.meta.id.as_str()
    });
    a.columns = union_by_id(a.columns, b.columns, |c| c.meta.id.as_str());
    a.relationships = union_by_id(a.relationships, b.relationships, |r| r.id.as_str());
    a.dependencies = union_by_id(a.dependencies, b.dependencies, |d| d.id.as_str());
    if b.source.is_some() {
        a.source = b.source;
    }
    for (k, v) in b.attributes {
        a.attributes.insert(k, v);
    }
    a
}

fn union_by_id<T, F>(mut base: Vec<T>, incoming: Vec<T>, id_of: F) -> Vec<T>
where
    F: Fn(&T) -> &str,
{
    use std::collections::HashMap;
    let mut index: HashMap<String, usize> = base
        .iter()
        .enumerate()
        .map(|(i, t)| (id_of(t).to_string(), i))
        .collect();
    for item in incoming {
        let id = id_of(&item).to_string();
        if let Some(&i) = index.get(&id) {
            base[i] = item;
        } else {
            index.insert(id, base.len());
            base.push(item);
        }
    }
    base
}

/// Collapse short schema.table stubs into catalog.schema.table nodes and rewrite edges.
/// Drop short schema.table stubs when a longer catalog.schema.table exists; rewrite edges.
fn reconcile_relation_aliases(snap: &mut Snapshot) {
    use simplineage_core::ObjectId;
    use std::collections::HashMap;

    // rank: FQN length primary; prefer view/mv ids over table stubs when equal.
    let mut prefer: HashMap<String, (usize, ObjectId)> = HashMap::new();
    let mut register = |id: &ObjectId, parts: &[String], kind_bonus: usize| {
        if parts.len() < 2 {
            return;
        }
        let short = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
        let rank = parts.len() * 10 + kind_bonus;
        prefer
            .entry(short)
            .and_modify(|(r, oid)| {
                if rank > *r {
                    *r = rank;
                    *oid = id.clone();
                }
            })
            .or_insert((rank, id.clone()));
    };
    for t in &snap.tables {
        register(&t.meta.id, &t.meta.fqn.parts, 0);
    }
    for v in &snap.views {
        register(&v.meta.id, &v.meta.fqn.parts, 2);
    }
    for m in &snap.materialized_views {
        register(&m.meta.id, &m.meta.fqn.parts, 2);
    }

    let mut rewrite: HashMap<String, ObjectId> = HashMap::new();
    let mut collect_rewrite = |id: &ObjectId, parts: &[String]| {
        if parts.len() < 2 {
            return;
        }
        let short = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
        if let Some((_, pref)) = prefer.get(&short) {
            if pref != id {
                rewrite.insert(id.as_str().to_string(), pref.clone());
            }
        }
    };
    for t in &snap.tables {
        collect_rewrite(&t.meta.id, &t.meta.fqn.parts);
    }
    for v in &snap.views {
        collect_rewrite(&v.meta.id, &v.meta.fqn.parts);
    }
    for m in &snap.materialized_views {
        collect_rewrite(&m.meta.id, &m.meta.fqn.parts);
    }

    snap.tables
        .retain(|t| !rewrite.contains_key(t.meta.id.as_str()));
    snap.views
        .retain(|v| !rewrite.contains_key(v.meta.id.as_str()));
    snap.materialized_views
        .retain(|m| !rewrite.contains_key(m.meta.id.as_str()));

    for dep in &mut snap.dependencies {
        if let Some(n) = rewrite.get(dep.from_id.as_str()) {
            dep.from_id = n.clone();
        }
        if let Some(n) = rewrite.get(dep.to_id.as_str()) {
            dep.to_id = n.clone();
        }
    }
    for col in &mut snap.columns {
        if let Some(n) = rewrite.get(col.parent_id.as_str()) {
            col.parent_id = n.clone();
        }
    }
}
