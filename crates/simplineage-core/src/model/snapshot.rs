//! Point-in-time metadata snapshots and model envelopes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::graph::{Dependency, Relationship};
use super::ids::ObjectId;
use super::objects::{
    Catalog, Column, Database, MaterializedView, MetadataObject, Schema, Table, View,
};
use super::types::Attributes;
use super::version::ModelVersion;
use crate::error::{Error, Result};

/// A versioned, self-contained capture of catalog metadata and lineage edges.
///
/// Snapshots are the unit of persistence, comparison, and export. They are
/// **vendor-agnostic**: only normalized objects appear here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Unique id for this snapshot instance.
    pub id: ObjectId,
    /// Metadata model schema version for this payload.
    pub model_version: ModelVersion,
    /// Optional human label (e.g. `"prod-2026-07-10"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Creation time as RFC 3339 string (timezone-aware recommended).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Source system name (tool/export), not a SQL dialect requirement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Free-form snapshot attributes.
    #[serde(default, skip_serializing_if = "Attributes::is_empty")]
    pub attributes: Attributes,

    /// Catalogs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalogs: Vec<Catalog>,
    /// Databases.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub databases: Vec<Database>,
    /// Schemas.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub schemas: Vec<Schema>,
    /// Tables.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tables: Vec<Table>,
    /// Views.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<View>,
    /// Materialized views.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub materialized_views: Vec<MaterializedView>,
    /// Columns.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<Column>,
    /// Structural relationships.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relationships: Vec<Relationship>,
    /// Lineage dependencies.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<Dependency>,
}

impl Snapshot {
    /// Create an empty snapshot with a new id and current model version.
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: ObjectId::from_trusted(Uuid::new_v4().to_string()),
            model_version: ModelVersion::current(),
            label: None,
            created_at: None,
            source: None,
            attributes: Attributes::new(),
            catalogs: Vec::new(),
            databases: Vec::new(),
            schemas: Vec::new(),
            tables: Vec::new(),
            views: Vec::new(),
            materialized_views: Vec::new(),
            columns: Vec::new(),
            relationships: Vec::new(),
            dependencies: Vec::new(),
        }
    }

    /// Serialize to pretty JSON.
    pub fn to_json_pretty(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Serialize to compact JSON.
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    /// Deserialize from JSON bytes or string.
    pub fn from_json(json: impl AsRef<[u8]>) -> Result<Self> {
        let snap: Self = serde_json::from_slice(json.as_ref())?;
        snap.ensure_compatible_version()?;
        Ok(snap)
    }

    /// Reject unsupported model versions.
    pub fn ensure_compatible_version(&self) -> Result<()> {
        if self.model_version.is_compatible_with_current() {
            Ok(())
        } else {
            Err(Error::UnsupportedModelVersion {
                found: self.model_version.to_string(),
                supported: format!("{}.x.x", super::version::MODEL_VERSION_MAJOR),
            })
        }
    }

    /// Total number of metadata objects (excluding edges).
    #[must_use]
    pub fn object_count(&self) -> usize {
        self.catalogs.len()
            + self.databases.len()
            + self.schemas.len()
            + self.tables.len()
            + self.views.len()
            + self.materialized_views.len()
            + self.columns.len()
    }

    /// Index all objects by id.
    ///
    /// **Note:** this clones every catalog object. Prefer [`Self::kind_index`],
    /// [`Self::contains_object`], or field iteration when full objects are not required.
    #[must_use]
    pub fn object_index(&self) -> BTreeMap<ObjectId, MetadataObject> {
        let mut map = BTreeMap::new();
        for c in &self.catalogs {
            map.insert(c.meta.id.clone(), MetadataObject::Catalog(c.clone()));
        }
        for d in &self.databases {
            map.insert(d.meta.id.clone(), MetadataObject::Database(d.clone()));
        }
        for s in &self.schemas {
            map.insert(s.meta.id.clone(), MetadataObject::Schema(s.clone()));
        }
        for t in &self.tables {
            map.insert(t.meta.id.clone(), MetadataObject::Table(t.clone()));
        }
        for v in &self.views {
            map.insert(v.meta.id.clone(), MetadataObject::View(v.clone()));
        }
        for m in &self.materialized_views {
            map.insert(
                m.meta.id.clone(),
                MetadataObject::MaterializedView(m.clone()),
            );
        }
        for c in &self.columns {
            map.insert(c.meta.id.clone(), MetadataObject::Column(c.clone()));
        }
        map
    }

    /// Cheap id → kind map without cloning full objects (catalog objects only).
    #[must_use]
    pub fn kind_index(&self) -> BTreeMap<ObjectId, &'static str> {
        let mut ids = BTreeMap::new();
        for c in &self.catalogs {
            ids.insert(c.meta.id.clone(), "catalog");
        }
        for d in &self.databases {
            ids.insert(d.meta.id.clone(), "database");
        }
        for s in &self.schemas {
            ids.insert(s.meta.id.clone(), "schema");
        }
        for t in &self.tables {
            ids.insert(t.meta.id.clone(), "table");
        }
        for v in &self.views {
            ids.insert(v.meta.id.clone(), "view");
        }
        for m in &self.materialized_views {
            ids.insert(m.meta.id.clone(), "materialized_view");
        }
        for c in &self.columns {
            ids.insert(c.meta.id.clone(), "column");
        }
        ids
    }

    /// Whether a catalog object with this id exists (no full index clone).
    #[must_use]
    pub fn contains_object(&self, id: &ObjectId) -> bool {
        let s = id.as_str();
        self.catalogs.iter().any(|c| c.meta.id.as_str() == s)
            || self.databases.iter().any(|d| d.meta.id.as_str() == s)
            || self.schemas.iter().any(|x| x.meta.id.as_str() == s)
            || self.tables.iter().any(|t| t.meta.id.as_str() == s)
            || self.views.iter().any(|v| v.meta.id.as_str() == s)
            || self
                .materialized_views
                .iter()
                .any(|m| m.meta.id.as_str() == s)
            || self.columns.iter().any(|c| c.meta.id.as_str() == s)
    }

    /// Collect every object id present in the snapshot (objects + edges).
    #[must_use]
    pub fn all_ids(&self) -> BTreeMap<ObjectId, &'static str> {
        let mut ids = self.kind_index();
        for r in &self.relationships {
            ids.insert(r.id.clone(), "relationship");
        }
        for d in &self.dependencies {
            ids.insert(d.id.clone(), "dependency");
        }
        ids
    }
}

impl Default for Snapshot {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ids::FullyQualifiedName;
    use crate::model::objects::ObjectMeta;
    use crate::model::types::DataType;
    use crate::model::version::MODEL_VERSION;

    fn sample_snapshot() -> Snapshot {
        let mut s = Snapshot::new();
        s.label = Some("demo".into());
        s.created_at = Some("2026-07-10T00:00:00Z".into());
        s.source = Some("unit-test".into());

        let schema_id = ObjectId::from_trusted("schema:public");
        let table_id = ObjectId::from_trusted("table:orders");
        let col_id = ObjectId::from_trusted("column:orders.id");

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
            data_type: DataType::Integer { bits: Some(64) },
            nullable: false,
            is_primary_key: Some(true),
            raw_type: Some("BIGINT".into()),
        });
        s
    }

    #[test]
    fn json_roundtrip_preserves_model_version() {
        let s = sample_snapshot();
        let json = s.to_json_pretty().unwrap();
        assert!(json.contains(MODEL_VERSION));
        let back = Snapshot::from_json(json.as_bytes()).unwrap();
        assert_eq!(s.tables.len(), back.tables.len());
        assert_eq!(s.columns[0].data_type, back.columns[0].data_type);
        assert_eq!(s.model_version, back.model_version);
    }

    #[test]
    fn rejects_incompatible_version_on_load() {
        let mut s = sample_snapshot();
        s.model_version = ModelVersion::new("99.0.0");
        let json = s.to_json().unwrap();
        let err = Snapshot::from_json(json.as_bytes()).unwrap_err();
        match err {
            Error::UnsupportedModelVersion { found, .. } => assert_eq!(found, "99.0.0"),
            other => panic!("unexpected {other:?}"),
        }
    }
}
