//! Plugin-based metadata import framework for SimpLineage.
//!
//! # Overview
//!
//! - [`MetadataImporter`] — trait implemented by each plugin
//! - [`ImporterRegistry`] — registration + **auto-detection** + import
//! - Built-in format plugins: CSV, JSON, Parquet, Excel
//! - Warehouse plugins live in **separate crates** and call
//!   [`ImporterRegistry::register`]
//!
//! # Example
//!
//! ```no_run
//! use std::path::Path;
//! use simplineage_importers::{ImporterRegistry, ImportOptions};
//!
//! let registry = ImporterRegistry::with_builtins();
//! let snap = registry
//!     .import_path(Path::new("./metadata.json"), &ImportOptions::default())
//!     .unwrap();
//! assert!(!snap.tables.is_empty() || !snap.columns.is_empty() || snap.object_count() >= 0);
//! ```
//!
//! # Adding a warehouse
//!
//! 1. Create a new crate (e.g. `simplineage-importer-snowflake`)
//! 2. Depend on `simplineage-importers` + `simplineage-core`
//! 3. Implement [`MetadataImporter`]
//! 4. Expose `pub fn register(registry: &mut ImporterRegistry)`
//!
//! No changes to core or format importers are required.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod detect;
pub mod formats;
#[allow(missing_docs)]
pub mod intermediate;
pub mod normalize;
pub mod options;
pub mod plugin;
pub mod registry;
pub mod tabular;

pub use formats::{CsvImporter, ExcelImporter, JsonImporter, ParquetImporter};
pub use intermediate::{
    IntermediateCatalog, IntermediateColumn, IntermediateDependency, IntermediateRelationship,
    IntermediateTable,
};
pub use normalize::{intermediate_to_snapshot, parse_data_type};
pub use options::ImportOptions;
pub use plugin::{DetectConfidence, MetadataImporter};
pub use registry::{DetectResult, ImporterRegistry};

/// Register all built-in format importers into an existing registry.
pub fn register_builtins(registry: &mut ImporterRegistry) {
    registry.register(Box::new(CsvImporter));
    registry.register(Box::new(JsonImporter));
    registry.register(Box::new(ParquetImporter));
    registry.register(Box::new(ExcelImporter));
}
