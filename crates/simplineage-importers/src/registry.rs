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
            .filter(|p| p.is_file() && self.detect(p).is_some())
            .collect();
        paths.sort();
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
        if !options.skip_validation {
            use simplineage_core::model::Validate;
            // Merged ids may collide; re-validate best-effort
            if let Err(e) = out.validate() {
                tracing::warn!(error = %e, "merged snapshot validation warning");
            }
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

/// Naive merge: concatenate collections (does not re-key colliding ids).
fn merge_snapshots(mut a: Snapshot, b: Snapshot) -> Snapshot {
    a.catalogs.extend(b.catalogs);
    a.databases.extend(b.databases);
    a.schemas.extend(b.schemas);
    a.tables.extend(b.tables);
    a.views.extend(b.views);
    a.materialized_views.extend(b.materialized_views);
    a.columns.extend(b.columns);
    a.relationships.extend(b.relationships);
    a.dependencies.extend(b.dependencies);
    if a.source.is_none() {
        a.source = b.source;
    }
    a
}
