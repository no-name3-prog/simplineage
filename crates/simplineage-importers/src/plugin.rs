//! Plugin trait for metadata importers.
//!
//! Warehouse-specific support is added by implementing [`MetadataImporter`] in a
//! **separate crate** and registering it on an [`crate::ImporterRegistry`].

use std::path::Path;

use simplineage_core::{Result, Snapshot};

use crate::options::ImportOptions;

/// How confidently an importer claims a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DetectConfidence {
    /// Not this format / source.
    None = 0,
    /// Weak signal (e.g. extension only).
    Low = 1,
    /// Solid signal (extension + headers / magic bytes).
    Medium = 2,
    /// Strong signal (content validates as this format).
    High = 3,
}

/// A pluggable metadata importer.
///
/// Implement this in a new crate to support an additional warehouse or export
/// layout. Built-in format importers (CSV, JSON, Parquet, Excel) live in this
/// crate and are registered by default.
pub trait MetadataImporter: Send + Sync {
    /// Stable machine id (e.g. `"csv"`, `"snowflake-export"`).
    fn id(&self) -> &'static str;

    /// Human-readable name.
    fn name(&self) -> &str;

    /// Short description of what this importer accepts.
    fn description(&self) -> &str {
        ""
    }

    /// File extensions this importer typically handles (lowercase, without dot).
    fn extensions(&self) -> &[&str] {
        &[]
    }

    /// Probe whether `path` looks like a source this importer can handle.
    fn detect(&self, path: &Path) -> DetectConfidence;

    /// Import metadata from `path` into the common [`Snapshot`] model.
    fn import(&self, path: &Path, options: &ImportOptions) -> Result<Snapshot>;
}
