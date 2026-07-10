//! Intermediate catalog export shape used by format importers.
//!
//! CSV / Excel / Parquet rows map into this structure before conversion to
//! [`simplineage_core::Snapshot`]. Warehouse plugins may build a `Snapshot`
//! directly or populate this intermediate form.

use serde::{Deserialize, Serialize};

/// Loose, tabular representation of exported metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IntermediateCatalog {
    /// Optional source label.
    #[serde(default)]
    pub source: Option<String>,
    /// Table / view rows.
    #[serde(default)]
    pub tables: Vec<IntermediateTable>,
    /// Column rows.
    #[serde(default)]
    pub columns: Vec<IntermediateColumn>,
    /// Relationship rows.
    #[serde(default)]
    pub relationships: Vec<IntermediateRelationship>,
    /// Dependency / lineage rows.
    #[serde(default)]
    pub dependencies: Vec<IntermediateDependency>,
}

/// Intermediate relation (table, view, or materialized view).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IntermediateTable {
    /// Catalog name.
    #[serde(default)]
    pub catalog: Option<String>,
    /// Database name.
    #[serde(default)]
    pub database: Option<String>,
    /// Schema name.
    #[serde(default)]
    pub schema: Option<String>,
    /// Relation name (required).
    pub name: String,
    /// `table` | `view` | `materialized_view` (default `table`).
    #[serde(default)]
    pub kind: Option<String>,
    /// View / MV definition SQL if present.
    #[serde(default)]
    pub definition: Option<String>,
    /// Description.
    #[serde(default)]
    pub description: Option<String>,
}

/// Intermediate column.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IntermediateColumn {
    #[serde(default)]
    pub catalog: Option<String>,
    #[serde(default)]
    pub database: Option<String>,
    #[serde(default)]
    pub schema: Option<String>,
    /// Parent table/view name.
    pub table: String,
    /// Column name.
    pub name: String,
    #[serde(default)]
    pub data_type: Option<String>,
    #[serde(default)]
    pub nullable: Option<bool>,
    #[serde(default)]
    pub ordinal: Option<u32>,
    #[serde(default)]
    pub is_primary_key: Option<bool>,
    #[serde(default)]
    pub description: Option<String>,
}

/// Intermediate structural relationship.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IntermediateRelationship {
    #[serde(default)]
    pub name: Option<String>,
    /// `foreign_key` | `unique` | …
    #[serde(default)]
    pub kind: Option<String>,
    pub from_schema: Option<String>,
    pub from_table: String,
    pub from_column: Option<String>,
    pub to_schema: Option<String>,
    pub to_table: String,
    pub to_column: Option<String>,
}

/// Intermediate lineage edge (upstream → downstream).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IntermediateDependency {
    pub from_schema: Option<String>,
    pub from_table: String,
    #[serde(default)]
    pub from_column: Option<String>,
    pub to_schema: Option<String>,
    pub to_table: String,
    #[serde(default)]
    pub to_column: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
}
