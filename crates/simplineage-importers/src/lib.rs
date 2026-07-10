//! Metadata importers for SimpLineage.
//!
//! Future phases will add CSV, JSON, Excel, Parquet, and warehouse-specific
//! plugins behind a common [`Importer`] trait.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

use simplineage_core::Result;

/// Trait implemented by metadata importers.
pub trait Importer: Send + Sync {
    /// Stable importer identifier (e.g. `"csv"`, `"snowflake-export"`).
    fn name(&self) -> &str;

    /// Import metadata from the given path into the storage layer.
    fn import(&self, path: &std::path::Path) -> Result<()>;
}

/// Placeholder no-op importer used until real plugins land.
#[derive(Debug, Default)]
pub struct StubImporter;

impl Importer for StubImporter {
    fn name(&self) -> &str {
        "stub"
    }

    fn import(&self, path: &std::path::Path) -> Result<()> {
        tracing::debug!(?path, importer = self.name(), "stub import (no-op)");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_importer_name() {
        assert_eq!(StubImporter.name(), "stub");
    }
}
