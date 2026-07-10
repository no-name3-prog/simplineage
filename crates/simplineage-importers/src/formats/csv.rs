//! CSV importer for intermediate catalog rows.

use std::fs::File;
use std::path::Path;

use simplineage_core::{Error, Result, Snapshot};

use crate::detect::{first_text_line, has_extension};
use crate::intermediate::{
    IntermediateCatalog, IntermediateColumn, IntermediateDependency, IntermediateRelationship,
    IntermediateTable,
};
use crate::normalize::intermediate_to_snapshot;
use crate::options::ImportOptions;
use crate::plugin::{DetectConfidence, MetadataImporter};

/// Imports metadata from CSV exports.
///
/// Supported layouts:
/// - **Wide multi-entity** rows with an `entity_type` / `object_type` column
///   (`table`, `column`, `relationship`, `dependency`)
/// - **Tables file**: headers include `table` / `name` and optional `schema`, `kind`
/// - **Columns file**: headers include `table` and `column` / `name`
#[derive(Debug, Default)]
pub struct CsvImporter;

impl MetadataImporter for CsvImporter {
    fn id(&self) -> &'static str {
        "csv"
    }

    fn name(&self) -> &str {
        "CSV metadata"
    }

    fn description(&self) -> &str {
        "Tabular catalog export (tables/columns/relationships/dependencies)"
    }

    fn extensions(&self) -> &[&str] {
        &["csv", "tsv"]
    }

    fn detect(&self, path: &Path) -> DetectConfidence {
        if !path.is_file() {
            return DetectConfidence::None;
        }
        let ext_ok = has_extension(path, &["csv", "tsv"]);
        let header = first_text_line(path)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let looks = header.contains("table")
            || header.contains("column")
            || header.contains("entity_type")
            || header.contains("object_type")
            || header.contains("schema");
        match (ext_ok, looks) {
            (true, true) => DetectConfidence::High,
            (true, false) => DetectConfidence::Medium,
            (false, true) => DetectConfidence::Low,
            _ => DetectConfidence::None,
        }
    }

    fn import(&self, path: &Path, options: &ImportOptions) -> Result<Snapshot> {
        let cat = parse_csv_file(path)?;
        let mut opts = options.clone();
        if opts.source.is_none() {
            opts.source = Some(format!("csv:{}", path.display()));
        }
        intermediate_to_snapshot(&cat, &opts)
    }
}

fn parse_csv_file(path: &Path) -> Result<IntermediateCatalog> {
    let file =
        File::open(path).map_err(|e| Error::import(format!("open {}: {e}", path.display())))?;
    let delim = if has_extension(path, &["tsv"]) {
        b'\t'
    } else {
        b','
    };
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(file);

    let headers = rdr
        .headers()
        .map_err(|e| Error::import(format!("csv headers: {e}")))?
        .iter()
        .map(|h| h.to_ascii_lowercase())
        .collect::<Vec<_>>();

    let idx = |names: &[&str]| -> Option<usize> {
        headers.iter().position(|h| names.iter().any(|n| h == n))
    };

    let mut cat = IntermediateCatalog {
        source: Some(path.display().to_string()),
        ..Default::default()
    };

    let entity_col = idx(&["entity_type", "object_type", "entity", "type"]);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    for rec in rdr.records() {
        let rec = rec.map_err(|e| Error::import(format!("csv row: {e}")))?;
        let get = |names: &[&str]| -> Option<String> {
            idx(names)
                .and_then(|i| rec.get(i).map(|s| s.trim().to_string()))
                .filter(|s| !s.is_empty())
        };
        let get_bool = |names: &[&str]| -> Option<bool> {
            get(names).and_then(|s| match s.to_ascii_lowercase().as_str() {
                "1" | "true" | "t" | "yes" | "y" => Some(true),
                "0" | "false" | "f" | "no" | "n" => Some(false),
                _ => None,
            })
        };
        let get_u32 = |names: &[&str]| -> Option<u32> { get(names).and_then(|s| s.parse().ok()) };

        let entity = entity_col
            .and_then(|i| rec.get(i))
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();

        let force_table = entity.is_empty()
            && (stem.contains("table") || headers.iter().any(|h| h == "kind" || h == "table_name"));
        let force_column = entity.is_empty()
            && (stem.contains("column")
                || (headers.iter().any(|h| h == "column" || h == "column_name")
                    && headers.iter().any(|h| h == "table" || h == "table_name")));

        if entity == "table" || entity == "view" || entity == "materialized_view" || force_table {
            let name = get(&["table", "table_name", "name", "relation"])
                .ok_or_else(|| Error::import("csv table row missing name/table"))?;
            let kind = get(&["kind", "relation_kind", "table_type"]).or_else(|| {
                if entity == "view" || entity == "materialized_view" {
                    Some(entity.clone())
                } else {
                    None
                }
            });
            cat.tables.push(IntermediateTable {
                catalog: get(&["catalog", "catalog_name"]),
                database: get(&["database", "db", "database_name"]),
                schema: get(&["schema", "schema_name", "namespace"]),
                name,
                kind,
                definition: get(&["definition", "sql", "view_definition"]),
                description: get(&["description", "comment"]),
            });
        } else if entity == "column" || force_column {
            let table = get(&["table", "table_name", "relation"])
                .ok_or_else(|| Error::import("csv column row missing table"))?;
            let name = get(&["column", "column_name", "name", "field"])
                .ok_or_else(|| Error::import("csv column row missing column name"))?;
            cat.columns.push(IntermediateColumn {
                catalog: get(&["catalog", "catalog_name"]),
                database: get(&["database", "db", "database_name"]),
                schema: get(&["schema", "schema_name", "namespace"]),
                table,
                name,
                data_type: get(&["data_type", "datatype", "type", "column_type"]),
                nullable: get_bool(&["nullable", "is_nullable"]),
                ordinal: get_u32(&["ordinal", "ordinal_position", "position", "order"]),
                is_primary_key: get_bool(&["is_primary_key", "primary_key", "pk"]),
                description: get(&["description", "comment"]),
            });
        } else if entity == "relationship" || entity == "foreign_key" || entity == "fk" {
            cat.relationships.push(IntermediateRelationship {
                name: get(&["name", "constraint_name"]),
                kind: get(&["kind", "relationship_kind"]).or(Some("foreign_key".into())),
                from_schema: get(&["from_schema", "schema"]),
                from_table: get(&["from_table", "table"])
                    .ok_or_else(|| Error::import("relationship missing from_table"))?,
                from_column: get(&["from_column", "column"]),
                to_schema: get(&["to_schema", "ref_schema"]),
                to_table: get(&["to_table", "ref_table"])
                    .ok_or_else(|| Error::import("relationship missing to_table"))?,
                to_column: get(&["to_column", "ref_column"]),
            });
        } else if entity == "dependency" || entity == "lineage" {
            cat.dependencies.push(IntermediateDependency {
                from_schema: get(&["from_schema", "upstream_schema"]),
                from_table: get(&["from_table", "upstream_table", "source_table"])
                    .ok_or_else(|| Error::import("dependency missing from_table"))?,
                from_column: get(&["from_column", "upstream_column"]),
                to_schema: get(&["to_schema", "downstream_schema"]),
                to_table: get(&["to_table", "downstream_table", "target_table"])
                    .ok_or_else(|| Error::import("dependency missing to_table"))?,
                to_column: get(&["to_column", "downstream_column"]),
                kind: get(&["kind", "dependency_kind"]),
            });
        } else if !entity.is_empty() {
            // ignore unknown entity types
            tracing::debug!(%entity, "skipping csv row with unknown entity_type");
        } else {
            // Heuristic: if we have both table-like and column-like, prefer column when column present
            if get(&["column", "column_name"]).is_some() {
                let table = get(&["table", "table_name"])
                    .ok_or_else(|| Error::import("csv row missing table"))?;
                let name = get(&["column", "column_name", "name"])
                    .ok_or_else(|| Error::import("csv row missing column"))?;
                cat.columns.push(IntermediateColumn {
                    schema: get(&["schema", "schema_name"]),
                    table,
                    name,
                    data_type: get(&["data_type", "type"]),
                    nullable: get_bool(&["nullable"]),
                    ordinal: get_u32(&["ordinal", "ordinal_position"]),
                    is_primary_key: get_bool(&["is_primary_key", "pk"]),
                    ..Default::default()
                });
            } else if let Some(name) = get(&["table", "table_name", "name"]) {
                cat.tables.push(IntermediateTable {
                    schema: get(&["schema", "schema_name"]),
                    name,
                    kind: get(&["kind"]),
                    ..Default::default()
                });
            }
        }
    }

    if cat.tables.is_empty() && cat.columns.is_empty() {
        return Err(Error::import(format!(
            "no table/column rows recognized in {}",
            path.display()
        )));
    }
    Ok(cat)
}
