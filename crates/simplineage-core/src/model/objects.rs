//! Core metadata objects: catalog hierarchy and relation columns.
//!
//! Hierarchy (outer → inner), all levels optional in a given deployment:
//!
//! ```text
//! Catalog → Database → Schema → { Table | View | MaterializedView } → Column
//! ```
//!
//! Names and structure are **vendor-agnostic**. Importers map source concepts
//! into this shape (e.g. MySQL "database" may map to [`Schema`]).

use serde::{Deserialize, Serialize};

use super::ids::{FullyQualifiedName, ObjectId};
use super::types::{Attributes, DataType};

/// Shared metadata present on every catalog object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectMeta {
    /// Stable id within a snapshot.
    pub id: ObjectId,
    /// Hierarchical name.
    pub fqn: FullyQualifiedName,
    /// Simple name (usually the FQN leaf).
    pub name: String,
    /// Optional human description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Extensible attributes (tags, source system, etc.).
    #[serde(default, skip_serializing_if = "Attributes::is_empty")]
    pub attributes: Attributes,
}

impl ObjectMeta {
    /// Construct meta with id, fqn, and name derived from the FQN leaf when `name` is empty.
    pub fn new(id: ObjectId, fqn: FullyQualifiedName) -> Self {
        let name = fqn.leaf().to_string();
        Self {
            id,
            fqn,
            name,
            description: None,
            attributes: Attributes::new(),
        }
    }
}

/// Top-level catalog (e.g. metastore / account / multi-db catalog).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    /// Object identity and naming.
    #[serde(flatten)]
    pub meta: ObjectMeta,
}

/// Database / catalog-child namespace (vendor-neutral).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Database {
    /// Object identity and naming.
    #[serde(flatten)]
    pub meta: ObjectMeta,
    /// Parent catalog id, when nested under a catalog.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_id: Option<ObjectId>,
}

/// Schema namespace containing tables and views.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schema {
    /// Object identity and naming.
    #[serde(flatten)]
    pub meta: ObjectMeta,
    /// Parent database id, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database_id: Option<ObjectId>,
    /// Parent catalog id when schemas hang directly under a catalog.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_id: Option<ObjectId>,
}

/// Kind of tabular relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    /// Base table.
    Table,
    /// View (non-materialized).
    View,
    /// Materialized view / materialized query table.
    MaterializedView,
}

/// Base table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Table {
    /// Object identity and naming.
    #[serde(flatten)]
    pub meta: ObjectMeta,
    /// Containing schema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_id: Option<ObjectId>,
    /// Column ids in ordinal order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub column_ids: Vec<ObjectId>,
}

/// Non-materialized view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct View {
    /// Object identity and naming.
    #[serde(flatten)]
    pub meta: ObjectMeta,
    /// Containing schema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_id: Option<ObjectId>,
    /// Column ids in ordinal order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub column_ids: Vec<ObjectId>,
    /// Defining SQL or expression, if captured (dialect stored in attributes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub definition: Option<String>,
}

/// Materialized view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaterializedView {
    /// Object identity and naming.
    #[serde(flatten)]
    pub meta: ObjectMeta,
    /// Containing schema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_id: Option<ObjectId>,
    /// Column ids in ordinal order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub column_ids: Vec<ObjectId>,
    /// Defining SQL or expression, if captured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub definition: Option<String>,
}

/// Column belonging to a table, view, or materialized view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Column {
    /// Object identity and naming.
    #[serde(flatten)]
    pub meta: ObjectMeta,
    /// Parent relation (table / view / materialized view).
    pub parent_id: ObjectId,
    /// Zero-based ordinal position when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ordinal: Option<u32>,
    /// Logical data type.
    pub data_type: DataType,
    /// Whether NULL is allowed.
    #[serde(default = "default_true")]
    pub nullable: bool,
    /// Part of the primary key, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_primary_key: Option<bool>,
    /// Source/native type string as exported (vendor-specific, optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_type: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Any named object that can appear in the catalog graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "object_type", rename_all = "snake_case")]
pub enum MetadataObject {
    /// Catalog.
    Catalog(Catalog),
    /// Database.
    Database(Database),
    /// Schema.
    Schema(Schema),
    /// Table.
    Table(Table),
    /// View.
    View(View),
    /// Materialized view.
    MaterializedView(MaterializedView),
    /// Column.
    Column(Column),
}

impl MetadataObject {
    /// Object id.
    #[must_use]
    pub fn id(&self) -> &ObjectId {
        &self.meta().id
    }

    /// Shared meta.
    #[must_use]
    pub fn meta(&self) -> &ObjectMeta {
        match self {
            Self::Catalog(o) => &o.meta,
            Self::Database(o) => &o.meta,
            Self::Schema(o) => &o.meta,
            Self::Table(o) => &o.meta,
            Self::View(o) => &o.meta,
            Self::MaterializedView(o) => &o.meta,
            Self::Column(o) => &o.meta,
        }
    }

    /// Object kind label.
    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Catalog(_) => "catalog",
            Self::Database(_) => "database",
            Self::Schema(_) => "schema",
            Self::Table(_) => "table",
            Self::View(_) => "view",
            Self::MaterializedView(_) => "materialized_view",
            Self::Column(_) => "column",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ids::{FullyQualifiedName, ObjectId};

    #[test]
    fn table_json_has_object_type() {
        let t = Table {
            meta: ObjectMeta::new(
                ObjectId::from_trusted("t1"),
                FullyQualifiedName::parse_dotted("db.public.orders").unwrap(),
            ),
            schema_id: Some(ObjectId::from_trusted("s1")),
            column_ids: vec![ObjectId::from_trusted("c1")],
        };
        let obj = MetadataObject::Table(t);
        let v = serde_json::to_value(&obj).unwrap();
        assert_eq!(v["object_type"], "table");
        assert_eq!(v["name"], "orders");
    }
}
