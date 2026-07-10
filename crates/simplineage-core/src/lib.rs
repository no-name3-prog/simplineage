//! Core engine for [SimpLineage](https://github.com/no-name3-prog/simplineage).
//!
//! This crate provides:
//! - the **vendor-agnostic metadata model** ([`model`])
//! - configuration loading and logging
//! - a thin engine façade used by CLI/server
//!
//! # Metadata model
//!
//! See [`model`] for [`Catalog`], [`Database`], [`Schema`], [`Table`], [`View`],
//! [`MaterializedView`], [`Column`], [`Relationship`], [`Dependency`], and
//! [`Snapshot`].

#![warn(missing_docs)]
#![warn(clippy::all)]
#![forbid(unsafe_code)]

pub mod config;
pub mod error;
pub mod logging;
pub mod model;

pub use config::Settings;
pub use error::{Error, Result};
pub use logging::init_tracing;
pub use model::{
    Catalog, Column, Database, Dependency, FullyQualifiedName, MODEL_VERSION, MaterializedView,
    ModelVersion, ObjectId, Relationship, Schema, Snapshot, Table, Validate, View,
    validate_snapshot,
};

/// Library version from Cargo package metadata.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Human-readable product name.
pub const PRODUCT_NAME: &str = "SimpLineage";

/// Placeholder entry point for the core engine.
///
/// Future phases will load metadata snapshots, build graphs, and run analysis.
#[derive(Debug, Default, Clone)]
pub struct Engine {
    settings: Settings,
    /// Optional active metadata snapshot.
    snapshot: Option<Snapshot>,
}

impl Engine {
    /// Create a new engine with the given settings.
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            snapshot: None,
        }
    }

    /// Create an engine from the default configuration sources.
    pub fn from_default_config() -> Result<Self> {
        Ok(Self::new(Settings::load()?))
    }

    /// Borrow the active settings.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Load a validated snapshot into the engine.
    pub fn load_snapshot(&mut self, snapshot: Snapshot) -> Result<()> {
        snapshot.validate()?;
        self.snapshot = Some(snapshot);
        Ok(())
    }

    /// Borrow the loaded snapshot, if any.
    pub fn snapshot(&self) -> Option<&Snapshot> {
        self.snapshot.as_ref()
    }

    /// Short status string for CLI / health checks.
    pub fn status(&self) -> String {
        let snap = self
            .snapshot
            .as_ref()
            .map(|s| format!("{} objects", s.object_count()))
            .unwrap_or_else(|| "no snapshot".into());
        format!(
            "{PRODUCT_NAME} v{VERSION} ready (log_level={}, data_dir={}, model={}, {snap})",
            self.settings.logging.level,
            self.settings.storage.data_dir.display(),
            MODEL_VERSION,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ids::FullyQualifiedName;
    use crate::model::objects::{Column, ObjectMeta, Schema, Table};
    use crate::model::types::DataType;

    #[test]
    fn version_is_semver_like() {
        assert!(!VERSION.is_empty());
        assert!(VERSION.contains('.'));
    }

    #[test]
    fn engine_status_mentions_product_and_model() {
        let engine = Engine::new(Settings::default());
        let status = engine.status();
        assert!(status.contains(PRODUCT_NAME));
        assert!(status.contains(VERSION));
        assert!(status.contains(MODEL_VERSION));
    }

    #[test]
    fn engine_loads_valid_snapshot() {
        let mut engine = Engine::new(Settings::default());
        let mut s = Snapshot::new();
        let schema_id = ObjectId::from_trusted("s1");
        let table_id = ObjectId::from_trusted("t1");
        let col_id = ObjectId::from_trusted("c1");
        s.schemas.push(Schema {
            meta: ObjectMeta::new(
                schema_id.clone(),
                FullyQualifiedName::parse_dotted("public").unwrap(),
            ),
            database_id: None,
            catalog_id: None,
        });
        s.tables.push(Table {
            meta: ObjectMeta::new(
                table_id.clone(),
                FullyQualifiedName::parse_dotted("public.orders").unwrap(),
            ),
            schema_id: Some(schema_id),
            column_ids: vec![col_id.clone()],
        });
        s.columns.push(Column {
            meta: ObjectMeta::new(
                col_id,
                FullyQualifiedName::parse_dotted("public.orders.id").unwrap(),
            ),
            parent_id: table_id,
            ordinal: Some(0),
            data_type: DataType::String {
                max_length: Some(32),
                is_char_length: Some(true),
            },
            nullable: false,
            is_primary_key: Some(true),
            raw_type: None,
        });
        engine.load_snapshot(s).unwrap();
        assert_eq!(engine.snapshot().unwrap().object_count(), 3);
        assert!(engine.status().contains("3 objects"));
    }
}
