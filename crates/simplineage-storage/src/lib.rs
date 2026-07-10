//! Local metadata persistence for SimpLineage using **SQLite**.
//!
//! Lightweight offline-first storage (no heavy native analytics engines).
//!
//! # Features
//!
//! - Snapshot storage (full JSON payload + indexes)
//! - Materialized lineage edges for **fast graph reconstruction**
//! - **Incremental imports** (`Replace` / `Merge`)
//! - Schema **migrations**
//!
//! # Example
//!
//! ```
//! use simplineage_storage::{MetadataStore, ImportMode};
//! use simplineage_core::Snapshot;
//!
//! let store = MetadataStore::open_in_memory().unwrap();
//! let snap = Snapshot::new();
//! let id = snap.id.clone();
//! store.save_snapshot(&snap, true).unwrap();
//! let loaded = store.load_current().unwrap().unwrap();
//! assert_eq!(loaded.id, id);
//! let _ = ImportMode::Replace;
//! ```

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod error;
pub mod merge;
pub mod migrations;
pub mod store;

pub use error::{Result, StorageError};
pub use merge::{ImportMode, merge_snapshots};
pub use migrations::SCHEMA_VERSION;
pub use store::{ImportResult, MetadataStore, SnapshotMeta, Store};
