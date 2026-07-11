//! Shared tabular field aliases and row → intermediate mapping helpers.
//!
//! CSV, Excel, and Parquet importers share the same column-name vocabulary and
//! Intermediate* construction logic through this module.

use crate::intermediate::{
    IntermediateCatalog, IntermediateColumn, IntermediateDependency, IntermediateRelationship,
    IntermediateTable,
};

/// Header aliases for relation / table name.
pub const TABLE_NAMES: &[&str] = &["table", "table_name", "name", "relation"];
/// Header aliases for column name.
pub const COLUMN_NAMES: &[&str] = &["column", "column_name", "name", "field"];
/// Header aliases for schema / dataset.
pub const SCHEMA_NAMES: &[&str] = &[
    "schema",
    "schema_name",
    "namespace",
    "table_schema",
    "dataset_name",
];
/// Header aliases for catalog / project.
pub const CATALOG_NAMES: &[&str] = &["catalog", "catalog_name", "table_catalog", "project_id"];
/// Header aliases for database.
pub const DATABASE_NAMES: &[&str] = &["database", "db", "database_name", "dataset_id"];
/// Header aliases for data type.
pub const DATA_TYPE_NAMES: &[&str] = &["data_type", "datatype", "type", "column_type"];
/// Header aliases for nullable.
pub const NULLABLE_NAMES: &[&str] = &["nullable", "is_nullable"];
/// Header aliases for ordinal.
pub const ORDINAL_NAMES: &[&str] = &["ordinal", "ordinal_position", "position", "order"];
/// Header aliases for primary key.
pub const PK_NAMES: &[&str] = &["is_primary_key", "primary_key", "pk"];
/// Header aliases for description.
pub const DESC_NAMES: &[&str] = &["description", "comment"];
/// Header aliases for relation kind / table type.
pub const KIND_NAMES: &[&str] = &["kind", "relation_kind", "table_type"];
/// Header aliases for SQL definition.
pub const DEFINITION_NAMES: &[&str] = &["definition", "sql", "view_definition", "ddl"];

/// Parse common boolean cell text.
#[must_use]
pub fn parse_bool_cell(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "t" | "yes" | "y" => Some(true),
        "0" | "false" | "f" | "no" | "n" => Some(false),
        _ => None,
    }
}

/// Normalize warehouse table_type / kind strings to intermediate kinds.
#[must_use]
pub fn normalize_table_kind(raw: &str) -> String {
    match raw.to_ascii_uppercase().as_str() {
        "BASE TABLE" | "TABLE" | "EXTERNAL" | "CLONE" | "SNAPSHOT" => "table".into(),
        "VIEW" => "view".into(),
        "MATERIALIZED VIEW" | "MATERIALIZED_VIEW" => "materialized_view".into(),
        other => other.to_ascii_lowercase(),
    }
}

/// Generic field accessor used by format-specific importers.
pub trait FieldGet {
    /// First non-empty value among `names` (already lower-cased keys on the format side).
    fn get(&self, names: &[&str]) -> Option<String>;

    /// Boolean cell helper.
    fn get_bool(&self, names: &[&str]) -> Option<bool> {
        self.get(names).as_deref().and_then(parse_bool_cell)
    }

    /// Unsigned integer cell helper.
    fn get_u32(&self, names: &[&str]) -> Option<u32> {
        self.get(names).and_then(|s| s.parse().ok())
    }
}

/// Push a table/view row from a field getter.
pub fn push_table(cat: &mut IntermediateCatalog, row: &impl FieldGet, kind_hint: Option<String>) {
    let Some(name) = row.get(TABLE_NAMES) else {
        return;
    };
    let kind = row
        .get(KIND_NAMES)
        .or(kind_hint)
        .map(|k| normalize_table_kind(&k));
    cat.tables.push(IntermediateTable {
        catalog: row.get(CATALOG_NAMES),
        database: row.get(DATABASE_NAMES),
        schema: row.get(SCHEMA_NAMES),
        name,
        kind,
        definition: row.get(DEFINITION_NAMES),
        description: row.get(DESC_NAMES),
    });
}

/// Push a column row from a field getter. Returns false if required fields missing.
pub fn push_column(cat: &mut IntermediateCatalog, row: &impl FieldGet) -> bool {
    let Some(table) = row.get(&["table", "table_name", "relation"]) else {
        return false;
    };
    let Some(name) = row.get(COLUMN_NAMES) else {
        return false;
    };
    cat.columns.push(IntermediateColumn {
        catalog: row.get(CATALOG_NAMES),
        database: row.get(DATABASE_NAMES),
        schema: row.get(SCHEMA_NAMES),
        table,
        name,
        data_type: row.get(DATA_TYPE_NAMES),
        nullable: row.get_bool(NULLABLE_NAMES),
        ordinal: row.get_u32(ORDINAL_NAMES),
        is_primary_key: row.get_bool(PK_NAMES),
        description: row.get(DESC_NAMES),
    });
    true
}

/// Push a dependency / lineage row. Returns false if endpoints missing.
pub fn push_dependency(cat: &mut IntermediateCatalog, row: &impl FieldGet) -> bool {
    let Some(from_table) = row.get(&["from_table", "upstream_table", "source_table"]) else {
        return false;
    };
    let Some(to_table) = row.get(&["to_table", "downstream_table", "target_table"]) else {
        return false;
    };
    cat.dependencies.push(IntermediateDependency {
        from_schema: row.get(&["from_schema", "upstream_schema"]),
        from_table,
        from_column: row.get(&["from_column", "upstream_column"]),
        to_schema: row.get(&["to_schema", "downstream_schema"]),
        to_table,
        to_column: row.get(&["to_column", "downstream_column"]),
        kind: row.get(&["kind", "dependency_kind"]),
    });
    true
}

/// Push a relationship / FK row. Returns false if endpoints missing.
pub fn push_relationship(cat: &mut IntermediateCatalog, row: &impl FieldGet) -> bool {
    let Some(from_table) = row.get(&["from_table", "table"]) else {
        return false;
    };
    let Some(to_table) = row.get(&["to_table", "ref_table"]) else {
        return false;
    };
    cat.relationships.push(IntermediateRelationship {
        name: row.get(&["name", "constraint_name"]),
        kind: row
            .get(&["kind", "relationship_kind"])
            .or(Some("foreign_key".into())),
        from_schema: row.get(&["from_schema", "schema"]),
        from_table,
        from_column: row.get(&["from_column", "column"]),
        to_schema: row.get(&["to_schema", "ref_schema"]),
        to_table,
        to_column: row.get(&["to_column", "ref_column"]),
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MapRow(HashMap<&'static str, String>);

    impl FieldGet for MapRow {
        fn get(&self, names: &[&str]) -> Option<String> {
            for n in names {
                if let Some(v) = self.0.get(*n) {
                    if !v.is_empty() {
                        return Some(v.clone());
                    }
                }
            }
            None
        }
    }

    #[test]
    fn push_table_and_dependency() {
        let mut cat = IntermediateCatalog::default();
        let row = MapRow(HashMap::from([
            ("table_name", "orders".into()),
            ("table_schema", "raw".into()),
            ("table_type", "BASE TABLE".into()),
        ]));
        push_table(&mut cat, &row, None);
        assert_eq!(cat.tables.len(), 1);
        assert_eq!(cat.tables[0].kind.as_deref(), Some("table"));

        let dep = MapRow(HashMap::from([
            ("from_table", "orders".into()),
            ("from_schema", "raw".into()),
            ("to_table", "stg_orders".into()),
            ("to_schema", "staging".into()),
            ("kind", "pipeline".into()),
        ]));
        assert!(push_dependency(&mut cat, &dep));
        assert_eq!(cat.dependencies.len(), 1);
    }
}
