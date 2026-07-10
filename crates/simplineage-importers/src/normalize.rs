//! Convert [`IntermediateCatalog`] into a validated [`Snapshot`].

use std::collections::BTreeMap;

use simplineage_core::model::graph::{
    ColumnMapping, Dependency, DependencyKind, DependencyLevel, Relationship, RelationshipKind,
};
use simplineage_core::model::ids::{FullyQualifiedName, ObjectId};
use simplineage_core::model::objects::{
    Catalog, Column, Database, MaterializedView, ObjectMeta, Schema, Table, View,
};
use simplineage_core::model::types::DataType;
use simplineage_core::model::{Snapshot, Validate};
use simplineage_core::{Error, Result};

use crate::intermediate::{IntermediateCatalog, IntermediateColumn};
use crate::options::ImportOptions;

/// Build a [`Snapshot`] from intermediate rows.
pub fn intermediate_to_snapshot(
    cat: &IntermediateCatalog,
    options: &ImportOptions,
) -> Result<Snapshot> {
    let mut snap = Snapshot::new();
    snap.source = options
        .source
        .clone()
        .or_else(|| cat.source.clone())
        .or_else(|| Some("import".into()));
    snap.label = options.label.clone();

    let default_schema = options
        .default_schema
        .clone()
        .unwrap_or_else(|| "public".into());

    let mut catalogs: BTreeMap<String, ObjectId> = BTreeMap::new();
    let mut databases: BTreeMap<String, ObjectId> = BTreeMap::new();
    let mut schemas: BTreeMap<String, ObjectId> = BTreeMap::new();
    let mut relations: BTreeMap<String, (ObjectId, String)> = BTreeMap::new();
    let mut columns_idx: BTreeMap<String, ObjectId> = BTreeMap::new();

    if let Some(ref c) = options.default_catalog {
        ensure_catalog(c, &mut catalogs, &mut snap)?;
    }
    if let Some(ref d) = options.default_database {
        ensure_database(
            d,
            options.default_catalog.as_deref(),
            &mut catalogs,
            &mut databases,
            &mut snap,
        )?;
    }

    for t in &cat.tables {
        if t.name.trim().is_empty() {
            return Err(Error::import("table row missing name"));
        }
        let catalog = t.catalog.as_deref().or(options.default_catalog.as_deref());
        let database = t
            .database
            .as_deref()
            .or(options.default_database.as_deref());
        let schema_name = t.schema.as_deref().unwrap_or(default_schema.as_str());
        add_table(
            &mut snap,
            &mut catalogs,
            &mut databases,
            &mut schemas,
            &mut relations,
            catalog,
            database,
            schema_name,
            t.name.as_str(),
            t.kind.as_deref(),
            t.definition.clone(),
            t.description.clone(),
        )?;
    }

    for c in &cat.columns {
        ensure_parent_table(
            &mut snap,
            &mut catalogs,
            &mut databases,
            &mut schemas,
            &mut relations,
            c,
            options,
            &default_schema,
        )?;
        add_column(
            &mut snap,
            &relations,
            &mut columns_idx,
            c,
            options,
            &default_schema,
        )?;
    }

    for r in &cat.relationships {
        let from_schema = r.from_schema.as_deref().unwrap_or(default_schema.as_str());
        let to_schema = r.to_schema.as_deref().unwrap_or(default_schema.as_str());
        let from_id = find_relation(&relations, from_schema, &r.from_table).ok_or_else(|| {
            Error::import(format!(
                "relationship from_table not found: {from_schema}.{}",
                r.from_table
            ))
        })?;
        let to_id = find_relation(&relations, to_schema, &r.to_table).ok_or_else(|| {
            Error::import(format!(
                "relationship to_table not found: {to_schema}.{}",
                r.to_table
            ))
        })?;

        let mut mappings = Vec::new();
        if let (Some(fc), Some(tc)) = (&r.from_column, &r.to_column) {
            if let (Some(fk), Some(tk)) = (
                find_relation_key(&relations, from_schema, &r.from_table),
                find_relation_key(&relations, to_schema, &r.to_table),
            ) {
                let fck = format!("{fk}.{fc}");
                let tck = format!("{tk}.{tc}");
                if let (Some(fid), Some(tid)) = (columns_idx.get(&fck), columns_idx.get(&tck)) {
                    mappings.push(ColumnMapping {
                        from_column_id: fid.clone(),
                        to_column_id: tid.clone(),
                    });
                }
            }
        }

        snap.relationships.push(Relationship {
            id: ObjectId::from_trusted(format!(
                "rel:{from_schema}:{}->{to_schema}:{}",
                r.from_table, r.to_table
            )),
            kind: parse_rel_kind(r.kind.as_deref()),
            from_id,
            to_id,
            column_mappings: mappings,
            name: r.name.clone(),
            attributes: Default::default(),
        });
    }

    // Ensure tables exist for dependency endpoints (lineage-only CSV files).
    for d in &cat.dependencies {
        let from_schema = d.from_schema.as_deref().unwrap_or(default_schema.as_str());
        let to_schema = d.to_schema.as_deref().unwrap_or(default_schema.as_str());
        ensure_stub_table(
            &mut snap,
            &mut catalogs,
            &mut databases,
            &mut schemas,
            &mut relations,
            None,
            None,
            from_schema,
            &d.from_table,
        )?;
        ensure_stub_table(
            &mut snap,
            &mut catalogs,
            &mut databases,
            &mut schemas,
            &mut relations,
            None,
            None,
            to_schema,
            &d.to_table,
        )?;
    }

    for d in &cat.dependencies {
        let from_schema = d.from_schema.as_deref().unwrap_or(default_schema.as_str());
        let to_schema = d.to_schema.as_deref().unwrap_or(default_schema.as_str());
        let (from_id, level_a) = resolve_endpoint(
            &relations,
            &columns_idx,
            from_schema,
            &d.from_table,
            d.from_column.as_deref(),
        )?;
        let (to_id, level_b) = resolve_endpoint(
            &relations,
            &columns_idx,
            to_schema,
            &d.to_table,
            d.to_column.as_deref(),
        )?;
        let level = if matches!(level_a, DependencyLevel::Column)
            || matches!(level_b, DependencyLevel::Column)
        {
            DependencyLevel::Column
        } else {
            DependencyLevel::Relation
        };
        snap.dependencies.push(Dependency {
            id: ObjectId::from_trusted(format!(
                "dep:{from_schema}:{}->{to_schema}:{}",
                d.from_table, d.to_table
            )),
            from_id,
            to_id,
            kind: parse_dep_kind(d.kind.as_deref()),
            level,
            confidence: None,
            attributes: Default::default(),
        });
    }

    if !options.skip_validation {
        snap.validate()?;
    }
    Ok(snap)
}

#[allow(clippy::too_many_arguments)]
fn add_table(
    snap: &mut Snapshot,
    catalogs: &mut BTreeMap<String, ObjectId>,
    databases: &mut BTreeMap<String, ObjectId>,
    schemas: &mut BTreeMap<String, ObjectId>,
    relations: &mut BTreeMap<String, (ObjectId, String)>,
    catalog: Option<&str>,
    database: Option<&str>,
    schema_name: &str,
    name: &str,
    kind: Option<&str>,
    definition: Option<String>,
    description: Option<String>,
) -> Result<()> {
    let schema_id = ensure_schema(
        schema_name,
        database,
        catalog,
        catalogs,
        databases,
        schemas,
        snap,
    )?;
    let rel_key = relation_key(catalog, database, schema_name, name);
    if relations.contains_key(&rel_key) {
        return Ok(());
    }
    let kind = normalize_kind(kind);
    let parts: Vec<&str> = [catalog, database, Some(schema_name), Some(name)]
        .into_iter()
        .flatten()
        .collect();
    let fqn = FullyQualifiedName::new(parts)?;
    let id = ObjectId::from_trusted(format!("{kind}:{rel_key}"));
    let mut meta = ObjectMeta::new(id.clone(), fqn);
    meta.description = description;

    match kind {
        "view" => snap.views.push(View {
            meta,
            schema_id: Some(schema_id),
            column_ids: Vec::new(),
            definition,
        }),
        "materialized_view" => snap.materialized_views.push(MaterializedView {
            meta,
            schema_id: Some(schema_id),
            column_ids: Vec::new(),
            definition,
        }),
        _ => snap.tables.push(Table {
            meta,
            schema_id: Some(schema_id),
            column_ids: Vec::new(),
        }),
    }
    relations.insert(rel_key, (id, kind.to_string()));
    Ok(())
}

fn ensure_stub_table(
    snap: &mut Snapshot,
    catalogs: &mut BTreeMap<String, ObjectId>,
    databases: &mut BTreeMap<String, ObjectId>,
    schemas: &mut BTreeMap<String, ObjectId>,
    relations: &mut BTreeMap<String, (ObjectId, String)>,
    catalog: Option<&str>,
    database: Option<&str>,
    schema_name: &str,
    name: &str,
) -> Result<()> {
    let rel_key = relation_key(catalog, database, schema_name, name);
    if relations.contains_key(&rel_key) {
        return Ok(());
    }
    // Already present under a longer key (e.g. catalog.schema.table).
    if find_relation(relations, schema_name, name).is_some() {
        return Ok(());
    }
    add_table(
        snap,
        catalogs,
        databases,
        schemas,
        relations,
        catalog,
        database,
        schema_name,
        name,
        Some("table"),
        None,
        None,
    )
}

fn ensure_parent_table(
    snap: &mut Snapshot,
    catalogs: &mut BTreeMap<String, ObjectId>,
    databases: &mut BTreeMap<String, ObjectId>,
    schemas: &mut BTreeMap<String, ObjectId>,
    relations: &mut BTreeMap<String, (ObjectId, String)>,
    c: &IntermediateColumn,
    options: &ImportOptions,
    default_schema: &str,
) -> Result<()> {
    let catalog = c.catalog.as_deref().or(options.default_catalog.as_deref());
    let database = c
        .database
        .as_deref()
        .or(options.default_database.as_deref());
    let schema_name = c.schema.as_deref().unwrap_or(default_schema);
    let rel_key = relation_key(catalog, database, schema_name, &c.table);
    if relations.contains_key(&rel_key) {
        return Ok(());
    }
    add_table(
        snap,
        catalogs,
        databases,
        schemas,
        relations,
        catalog,
        database,
        schema_name,
        &c.table,
        Some("table"),
        None,
        None,
    )
}

fn add_column(
    snap: &mut Snapshot,
    relations: &BTreeMap<String, (ObjectId, String)>,
    columns_idx: &mut BTreeMap<String, ObjectId>,
    c: &IntermediateColumn,
    options: &ImportOptions,
    default_schema: &str,
) -> Result<()> {
    if c.name.trim().is_empty() || c.table.trim().is_empty() {
        return Err(Error::import("column row requires table and name"));
    }
    let catalog = c.catalog.as_deref().or(options.default_catalog.as_deref());
    let database = c
        .database
        .as_deref()
        .or(options.default_database.as_deref());
    let schema_name = c.schema.as_deref().unwrap_or(default_schema);
    let rel_key = relation_key(catalog, database, schema_name, &c.table);
    let (parent_id, _) = relations
        .get(&rel_key)
        .ok_or_else(|| Error::import(format!("column parent table missing: {rel_key}")))?
        .clone();

    let col_key = format!("{rel_key}.{}", c.name);
    if columns_idx.contains_key(&col_key) {
        return Ok(());
    }
    let parts: Vec<&str> = [
        catalog,
        database,
        Some(schema_name),
        Some(c.table.as_str()),
        Some(c.name.as_str()),
    ]
    .into_iter()
    .flatten()
    .collect();
    let fqn = FullyQualifiedName::new(parts)?;
    let id = ObjectId::from_trusted(format!("column:{col_key}"));
    let mut meta = ObjectMeta::new(id.clone(), fqn);
    meta.description = c.description.clone();
    let raw = c.data_type.clone();
    snap.columns.push(Column {
        meta,
        parent_id: parent_id.clone(),
        ordinal: c.ordinal,
        data_type: parse_data_type(raw.as_deref()),
        nullable: c.nullable.unwrap_or(true),
        is_primary_key: c.is_primary_key,
        raw_type: raw,
    });
    columns_idx.insert(col_key, id.clone());
    attach_column(snap, &parent_id, id);
    Ok(())
}

fn ensure_catalog(
    name: &str,
    catalogs: &mut BTreeMap<String, ObjectId>,
    snap: &mut Snapshot,
) -> Result<ObjectId> {
    if let Some(id) = catalogs.get(name) {
        return Ok(id.clone());
    }
    let id = ObjectId::from_trusted(format!("catalog:{name}"));
    snap.catalogs.push(Catalog {
        meta: ObjectMeta::new(id.clone(), FullyQualifiedName::new([name])?),
    });
    catalogs.insert(name.to_string(), id.clone());
    Ok(id)
}

fn ensure_database(
    name: &str,
    catalog: Option<&str>,
    catalogs: &mut BTreeMap<String, ObjectId>,
    databases: &mut BTreeMap<String, ObjectId>,
    snap: &mut Snapshot,
) -> Result<ObjectId> {
    let key = match catalog {
        Some(c) => format!("{c}.{name}"),
        None => name.to_string(),
    };
    if let Some(id) = databases.get(&key) {
        return Ok(id.clone());
    }
    let catalog_id = match catalog {
        Some(c) => Some(ensure_catalog(c, catalogs, snap)?),
        None => None,
    };
    let fqn = match catalog {
        Some(c) => FullyQualifiedName::new([c, name])?,
        None => FullyQualifiedName::new([name])?,
    };
    let id = ObjectId::from_trusted(format!("database:{key}"));
    snap.databases.push(Database {
        meta: ObjectMeta::new(id.clone(), fqn),
        catalog_id,
    });
    databases.insert(key, id.clone());
    Ok(id)
}

fn ensure_schema(
    schema: &str,
    database: Option<&str>,
    catalog: Option<&str>,
    catalogs: &mut BTreeMap<String, ObjectId>,
    databases: &mut BTreeMap<String, ObjectId>,
    schemas: &mut BTreeMap<String, ObjectId>,
    snap: &mut Snapshot,
) -> Result<ObjectId> {
    let key = [catalog, database, Some(schema)]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(".");
    if let Some(id) = schemas.get(&key) {
        return Ok(id.clone());
    }
    let database_id = match database {
        Some(db) => Some(ensure_database(db, catalog, catalogs, databases, snap)?),
        None => None,
    };
    let catalog_id = if database.is_none() {
        catalog
            .map(|c| ensure_catalog(c, catalogs, snap))
            .transpose()?
    } else {
        None
    };
    let parts: Vec<&str> = [catalog, database, Some(schema)]
        .into_iter()
        .flatten()
        .collect();
    let fqn = FullyQualifiedName::new(parts)?;
    let id = ObjectId::from_trusted(format!("schema:{key}"));
    snap.schemas.push(Schema {
        meta: ObjectMeta::new(id.clone(), fqn),
        database_id,
        catalog_id,
    });
    schemas.insert(key, id.clone());
    Ok(id)
}

fn attach_column(snap: &mut Snapshot, parent_id: &ObjectId, col_id: ObjectId) {
    for t in &mut snap.tables {
        if &t.meta.id == parent_id {
            t.column_ids.push(col_id);
            return;
        }
    }
    for v in &mut snap.views {
        if &v.meta.id == parent_id {
            v.column_ids.push(col_id);
            return;
        }
    }
    for m in &mut snap.materialized_views {
        if &m.meta.id == parent_id {
            m.column_ids.push(col_id);
            return;
        }
    }
}

fn relation_key(
    catalog: Option<&str>,
    database: Option<&str>,
    schema: &str,
    table: &str,
) -> String {
    [catalog, database, Some(schema), Some(table)]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(".")
}

fn find_relation(
    relations: &BTreeMap<String, (ObjectId, String)>,
    schema: &str,
    table: &str,
) -> Option<ObjectId> {
    let suffix = format!("{schema}.{table}");
    relations
        .iter()
        .find(|(k, _)| *k == &suffix || k.ends_with(&format!(".{suffix}")))
        .map(|(_, (id, _))| id.clone())
        .or_else(|| {
            relations
                .iter()
                .find(|(k, _)| k.ends_with(&format!(".{table}")) || *k == table)
                .map(|(_, (id, _))| id.clone())
        })
}

fn find_relation_key(
    relations: &BTreeMap<String, (ObjectId, String)>,
    schema: &str,
    table: &str,
) -> Option<String> {
    let suffix = format!("{schema}.{table}");
    relations
        .keys()
        .find(|k| *k == &suffix || k.ends_with(&format!(".{suffix}")))
        .cloned()
}

fn resolve_endpoint(
    relations: &BTreeMap<String, (ObjectId, String)>,
    columns: &BTreeMap<String, ObjectId>,
    schema: &str,
    table: &str,
    column: Option<&str>,
) -> Result<(ObjectId, DependencyLevel)> {
    if let Some(col) = column {
        if let Some(key) = find_relation_key(relations, schema, table) {
            let ck = format!("{key}.{col}");
            if let Some(id) = columns.get(&ck) {
                return Ok((id.clone(), DependencyLevel::Column));
            }
        }
    }
    let id = find_relation(relations, schema, table)
        .ok_or_else(|| Error::import(format!("dependency endpoint not found: {schema}.{table}")))?;
    Ok((id, DependencyLevel::Relation))
}

fn normalize_kind(kind: Option<&str>) -> &'static str {
    match kind.map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("view") | Some("views") => "view",
        Some("materialized_view")
        | Some("materialized view")
        | Some("materializedview")
        | Some("mv") => "materialized_view",
        _ => "table",
    }
}

fn parse_rel_kind(kind: Option<&str>) -> RelationshipKind {
    match kind.map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("foreign_key") | Some("fk") | Some("foreign key") => RelationshipKind::ForeignKey,
        Some("primary_key") | Some("pk") => RelationshipKind::PrimaryKey,
        Some("unique") => RelationshipKind::Unique,
        Some("check") => RelationshipKind::Check,
        Some(other) => RelationshipKind::Other(other.to_string()),
        None => RelationshipKind::ForeignKey,
    }
}

fn parse_dep_kind(kind: Option<&str>) -> DependencyKind {
    match kind.map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("view_definition") | Some("view") => DependencyKind::ViewDefinition,
        Some("pipeline") | Some("etl") => DependencyKind::Pipeline,
        Some("foreign_key") | Some("fk") => DependencyKind::ForeignKey,
        Some("manual") => DependencyKind::Manual,
        Some("inferred") => DependencyKind::Inferred,
        Some(other) => DependencyKind::Other(other.to_string()),
        None => DependencyKind::Inferred,
    }
}

/// Map a free-form SQL type string to a logical [`DataType`].
pub fn parse_data_type(raw: Option<&str>) -> DataType {
    let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return DataType::Unknown;
    };
    let lower = raw.to_ascii_lowercase();
    let base = lower.split('(').next().unwrap_or(&lower);
    match base {
        "bool" | "boolean" | "bit" => DataType::Boolean,
        "int" | "integer" | "int4" | "int32" | "smallint" | "int2" | "bigint" | "int8"
        | "tinyint" | "int64" => {
            let bits = if base == "bigint" || base == "int8" || base == "int64" {
                Some(64)
            } else if base == "smallint" || base == "int2" || base == "tinyint" {
                Some(16)
            } else {
                Some(32)
            };
            DataType::Integer { bits }
        }
        "decimal" | "numeric" | "number" | "bignumeric" => DataType::Decimal {
            precision: None,
            scale: None,
        },
        "float" | "float4" | "float8" | "double" | "real" | "float64" => DataType::Float {
            bits: if base.contains('8') || base == "double" || base == "float64" {
                Some(64)
            } else {
                Some(32)
            },
        },
        "varchar" | "string" | "text" | "char" | "nvarchar" | "nchar" | "clob" => {
            DataType::String {
                max_length: None,
                is_char_length: Some(true),
            }
        }
        "binary" | "varbinary" | "bytes" | "blob" | "bytea" => {
            DataType::Binary { max_length: None }
        }
        "date" => DataType::Date,
        "time" => DataType::Time {
            precision: None,
            with_time_zone: None,
        },
        "timestamp" | "datetime" | "timestamptz" => DataType::Timestamp {
            precision: None,
            with_time_zone: Some(base.contains("tz") || lower.contains("with time zone")),
        },
        "json" | "jsonb" | "variant" => DataType::Json,
        "uuid" | "uniqueidentifier" => DataType::Uuid,
        "array" => DataType::Array { element: None },
        "geography" | "geometry" => DataType::Spatial,
        _ => DataType::Other {
            name: raw.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intermediate::{IntermediateCatalog, IntermediateColumn, IntermediateTable};

    #[test]
    fn builds_snapshot_from_rows() {
        let cat = IntermediateCatalog {
            source: Some("test".into()),
            tables: vec![IntermediateTable {
                schema: Some("public".into()),
                name: "orders".into(),
                kind: Some("table".into()),
                ..Default::default()
            }],
            columns: vec![IntermediateColumn {
                schema: Some("public".into()),
                table: "orders".into(),
                name: "id".into(),
                data_type: Some("BIGINT".into()),
                nullable: Some(false),
                ordinal: Some(0),
                is_primary_key: Some(true),
                ..Default::default()
            }],
            ..Default::default()
        };
        let snap = intermediate_to_snapshot(&cat, &ImportOptions::default()).unwrap();
        assert_eq!(snap.tables.len(), 1);
        assert_eq!(snap.columns.len(), 1);
        assert_eq!(snap.schemas.len(), 1);
    }
}
