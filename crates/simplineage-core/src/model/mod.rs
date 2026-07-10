//! Vendor-agnostic core metadata model.
//!
//! # Objects
//!
//! - [`Catalog`], [`Database`], [`Schema`]
//! - [`Table`], [`View`], [`MaterializedView`], [`Column`]
//! - [`Relationship`], [`Dependency`]
//! - [`Snapshot`]
//!
//! # Design
//!
//! The model never encodes a specific database vendor. Dialect-specific details
//! belong in optional [`crate::model::types::Attributes`] or [`Column::raw_type`].
//!
//! # Versioning
//!
//! Serialized payloads carry [`ModelVersion`] ([`MODEL_VERSION`]). Major version
//! must match for load compatibility.
//!
//! # Validation
//!
//! Call [`Validate::validate`] / [`validate_snapshot`] before persisting or analyzing.

pub mod graph;
pub mod ids;
pub mod objects;
pub mod snapshot;
pub mod types;
pub mod validate;
pub mod version;

pub use graph::{
    ColumnMapping, Dependency, DependencyKind, DependencyLevel, OrderedFloatCompat, Relationship,
    RelationshipKind,
};
pub use ids::{FullyQualifiedName, ObjectId};
pub use objects::{
    Catalog, Column, Database, MaterializedView, MetadataObject, ObjectMeta, RelationKind, Schema,
    Table, View,
};
pub use snapshot::Snapshot;
pub use types::{Attributes, DataType, StructField};
pub use validate::{Validate, validate_snapshot};
pub use version::{MODEL_VERSION, MODEL_VERSION_MAJOR, ModelVersion};
