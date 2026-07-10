//! Validation for metadata models and snapshots.

use std::collections::{BTreeMap, BTreeSet};

use super::ids::ObjectId;
use super::snapshot::Snapshot;
use crate::error::{Error, Result};

/// Validate a value, collecting a single error message on failure.
pub trait Validate {
    /// Return `Ok(())` if valid.
    fn validate(&self) -> Result<()>;
}

impl Validate for Snapshot {
    fn validate(&self) -> Result<()> {
        self.ensure_compatible_version()?;

        let mut errors: Vec<String> = Vec::new();

        // Unique object ids among catalog objects
        let mut seen: BTreeMap<String, Vec<&'static str>> = BTreeMap::new();
        let mut push_id = |id: &str, kind: &'static str| {
            seen.entry(id.to_string()).or_default().push(kind);
        };
        for c in &self.catalogs {
            push_id(c.meta.id.as_str(), "catalog");
        }
        for d in &self.databases {
            push_id(d.meta.id.as_str(), "database");
        }
        for s in &self.schemas {
            push_id(s.meta.id.as_str(), "schema");
        }
        for t in &self.tables {
            push_id(t.meta.id.as_str(), "table");
        }
        for v in &self.views {
            push_id(v.meta.id.as_str(), "view");
        }
        for m in &self.materialized_views {
            push_id(m.meta.id.as_str(), "materialized_view");
        }
        for c in &self.columns {
            push_id(c.meta.id.as_str(), "column");
        }

        for (id, kinds) in &seen {
            if kinds.len() > 1 {
                errors.push(format!(
                    "duplicate object id '{id}' used by: {}",
                    kinds.join(", ")
                ));
            }
        }

        let object_ids: BTreeSet<String> = seen.keys().cloned().collect();

        // Edge ids unique and distinct from object ids ideally; allow separate namespace but unique among edges
        let mut edge_ids: BTreeSet<String> = BTreeSet::new();
        for r in &self.relationships {
            if !edge_ids.insert(r.id.as_str().to_string()) {
                errors.push(format!("duplicate relationship id '{}'", r.id));
            }
            if !object_ids.contains(r.from_id.as_str()) {
                errors.push(format!(
                    "relationship '{}' from_id '{}' not found",
                    r.id, r.from_id
                ));
            }
            if !object_ids.contains(r.to_id.as_str()) {
                errors.push(format!(
                    "relationship '{}' to_id '{}' not found",
                    r.id, r.to_id
                ));
            }
            for m in &r.column_mappings {
                if !object_ids.contains(m.from_column_id.as_str()) {
                    errors.push(format!(
                        "relationship '{}' maps unknown from_column_id '{}'",
                        r.id, m.from_column_id
                    ));
                }
                if !object_ids.contains(m.to_column_id.as_str()) {
                    errors.push(format!(
                        "relationship '{}' maps unknown to_column_id '{}'",
                        r.id, m.to_column_id
                    ));
                }
            }
        }
        for d in &self.dependencies {
            if !edge_ids.insert(d.id.as_str().to_string()) {
                errors.push(format!("duplicate dependency/edge id '{}'", d.id));
            }
            if !object_ids.contains(d.from_id.as_str()) {
                errors.push(format!(
                    "dependency '{}' from_id '{}' not found",
                    d.id, d.from_id
                ));
            }
            if !object_ids.contains(d.to_id.as_str()) {
                errors.push(format!(
                    "dependency '{}' to_id '{}' not found",
                    d.id, d.to_id
                ));
            }
            if let Some(c) = d.confidence {
                if !(0.0..=1.0).contains(&c.0) || c.0.is_nan() {
                    errors.push(format!(
                        "dependency '{}' confidence must be in [0, 1], got {}",
                        d.id, c.0
                    ));
                }
            }
        }

        // Parent references
        for db in &self.databases {
            if let Some(ref cid) = db.catalog_id {
                if !object_ids.contains(cid.as_str()) {
                    errors.push(format!(
                        "database '{}' catalog_id '{}' not found",
                        db.meta.id, cid
                    ));
                }
            }
            check_name(&db.meta.name, &db.meta.id, &mut errors);
        }
        for sch in &self.schemas {
            if let Some(ref id) = sch.database_id {
                if !object_ids.contains(id.as_str()) {
                    errors.push(format!(
                        "schema '{}' database_id '{}' not found",
                        sch.meta.id, id
                    ));
                }
            }
            if let Some(ref id) = sch.catalog_id {
                if !object_ids.contains(id.as_str()) {
                    errors.push(format!(
                        "schema '{}' catalog_id '{}' not found",
                        sch.meta.id, id
                    ));
                }
            }
            check_name(&sch.meta.name, &sch.meta.id, &mut errors);
        }
        for t in &self.tables {
            if let Some(ref id) = t.schema_id {
                if !object_ids.contains(id.as_str()) {
                    errors.push(format!(
                        "table '{}' schema_id '{}' not found",
                        t.meta.id, id
                    ));
                }
            }
            for cid in &t.column_ids {
                if !object_ids.contains(cid.as_str()) {
                    errors.push(format!(
                        "table '{}' references missing column_id '{}'",
                        t.meta.id, cid
                    ));
                }
            }
            check_name(&t.meta.name, &t.meta.id, &mut errors);
        }
        for v in &self.views {
            if let Some(ref id) = v.schema_id {
                if !object_ids.contains(id.as_str()) {
                    errors.push(format!("view '{}' schema_id '{}' not found", v.meta.id, id));
                }
            }
            for cid in &v.column_ids {
                if !object_ids.contains(cid.as_str()) {
                    errors.push(format!(
                        "view '{}' references missing column_id '{}'",
                        v.meta.id, cid
                    ));
                }
            }
            check_name(&v.meta.name, &v.meta.id, &mut errors);
        }
        for m in &self.materialized_views {
            if let Some(ref id) = m.schema_id {
                if !object_ids.contains(id.as_str()) {
                    errors.push(format!(
                        "materialized_view '{}' schema_id '{}' not found",
                        m.meta.id, id
                    ));
                }
            }
            for cid in &m.column_ids {
                if !object_ids.contains(cid.as_str()) {
                    errors.push(format!(
                        "materialized_view '{}' references missing column_id '{}'",
                        m.meta.id, cid
                    ));
                }
            }
            check_name(&m.meta.name, &m.meta.id, &mut errors);
        }
        for c in &self.columns {
            if !object_ids.contains(c.parent_id.as_str()) {
                errors.push(format!(
                    "column '{}' parent_id '{}' not found",
                    c.meta.id, c.parent_id
                ));
            }
            check_name(&c.meta.name, &c.meta.id, &mut errors);
            if c.meta.fqn.parts.is_empty() {
                errors.push(format!("column '{}' has empty fqn", c.meta.id));
            }
        }
        for cat in &self.catalogs {
            check_name(&cat.meta.name, &cat.meta.id, &mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(Error::validation(errors.join("; ")))
        }
    }
}

fn check_name(name: &str, id: &ObjectId, errors: &mut Vec<String>) {
    if name.trim().is_empty() {
        errors.push(format!("object '{id}' has empty name"));
    }
}

/// Validate and return the snapshot unchanged on success.
pub fn validate_snapshot(snapshot: &Snapshot) -> Result<()> {
    snapshot.validate()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::graph::{Dependency, DependencyKind, DependencyLevel};
    use crate::model::ids::FullyQualifiedName;
    use crate::model::objects::{Column, ObjectMeta, Schema, Table};
    use crate::model::types::DataType;

    fn valid_minimal() -> Snapshot {
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
            data_type: DataType::Integer { bits: Some(64) },
            nullable: false,
            is_primary_key: Some(true),
            raw_type: None,
        });
        s
    }

    #[test]
    fn valid_snapshot_passes() {
        validate_snapshot(&valid_minimal()).unwrap();
    }

    #[test]
    fn duplicate_ids_fail() {
        let mut s = valid_minimal();
        s.tables[0].meta.id = ObjectId::from_trusted("c1");
        let err = validate_snapshot(&s).unwrap_err().to_string();
        assert!(err.contains("duplicate"), "{err}");
    }

    #[test]
    fn missing_parent_fails() {
        let mut s = valid_minimal();
        s.columns[0].parent_id = ObjectId::from_trusted("missing");
        let err = validate_snapshot(&s).unwrap_err().to_string();
        assert!(err.contains("parent_id"), "{err}");
    }

    #[test]
    fn dangling_dependency_fails() {
        let mut s = valid_minimal();
        s.dependencies.push(Dependency {
            id: ObjectId::from_trusted("dep1"),
            from_id: ObjectId::from_trusted("nope"),
            to_id: s.tables[0].meta.id.clone(),
            kind: DependencyKind::Manual,
            level: DependencyLevel::Relation,
            confidence: None,
            attributes: Default::default(),
        });
        let err = validate_snapshot(&s).unwrap_err().to_string();
        assert!(err.contains("dependency"), "{err}");
    }
}
