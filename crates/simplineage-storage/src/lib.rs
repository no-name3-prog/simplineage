//! Storage backends for SimpLineage.
//!
//! Planned: DuckDB and/or SQLite for offline-first metadata persistence.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use simplineage_core::Result;

/// Handle to a local metadata store.
#[derive(Debug, Clone)]
pub struct Store {
    data_dir: PathBuf,
}

impl Store {
    /// Open (or create) a store rooted at `data_dir`.
    pub fn open(data_dir: impl Into<PathBuf>) -> Result<Self> {
        let data_dir = data_dir.into();
        tracing::debug!(?data_dir, "opening storage (placeholder)");
        Ok(Self { data_dir })
    }

    /// Directory where store files live.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_store() {
        let store = Store::open(".simplineage-test").unwrap();
        assert_eq!(store.data_dir(), Path::new(".simplineage-test"));
    }
}
