//! Excel (.xlsx / .xls) importer via calamine.

use std::path::Path;

use calamine::{Data, Reader, open_workbook_auto};
use simplineage_core::{Error, Result, Snapshot};

use crate::detect::{has_extension, looks_like_excel};
use crate::intermediate::{
    IntermediateCatalog, IntermediateColumn, IntermediateDependency, IntermediateRelationship,
    IntermediateTable,
};
use crate::normalize::intermediate_to_snapshot;
use crate::options::ImportOptions;
use crate::plugin::{DetectConfidence, MetadataImporter};

/// Imports metadata from Excel workbooks.
///
/// Expected sheet names (case-insensitive): `tables`, `columns`,
/// `relationships`, `dependencies`. Extra sheets are ignored.
#[derive(Debug, Default)]
pub struct ExcelImporter;

impl MetadataImporter for ExcelImporter {
    fn id(&self) -> &'static str {
        "excel"
    }

    fn name(&self) -> &str {
        "Excel metadata"
    }

    fn description(&self) -> &str {
        "XLSX/XLS workbook with tables/columns sheets"
    }

    fn extensions(&self) -> &[&str] {
        &["xlsx", "xlsm", "xls"]
    }

    fn detect(&self, path: &Path) -> DetectConfidence {
        if !path.is_file() {
            return DetectConfidence::None;
        }
        let ext = has_extension(path, &["xlsx", "xlsm", "xls"]);
        let magic = looks_like_excel(path);
        match (ext, magic) {
            (true, true) => DetectConfidence::High,
            (true, false) => DetectConfidence::Medium,
            (false, true) => DetectConfidence::Low,
            _ => DetectConfidence::None,
        }
    }

    fn import(&self, path: &Path, options: &ImportOptions) -> Result<Snapshot> {
        let cat = parse_excel(path)?;
        let mut opts = options.clone();
        if opts.source.is_none() {
            opts.source = Some(format!("excel:{}", path.display()));
        }
        intermediate_to_snapshot(&cat, &opts)
    }
}

fn parse_excel(path: &Path) -> Result<IntermediateCatalog> {
    let mut workbook = open_workbook_auto(path)
        .map_err(|e| Error::import(format!("open excel {}: {e}", path.display())))?;

    let mut cat = IntermediateCatalog {
        source: Some(path.display().to_string()),
        ..Default::default()
    };

    let sheet_names = workbook.sheet_names().to_vec();
    for name in sheet_names {
        let lower = name.to_ascii_lowercase();
        let range = workbook
            .worksheet_range(&name)
            .map_err(|e| Error::import(format!("sheet {name}: {e}")))?;

        let mut rows = range.rows();
        let Some(header_cells) = rows.next() else {
            continue;
        };
        let headers: Vec<String> = header_cells
            .iter()
            .map(cell_string)
            .map(|s| s.to_ascii_lowercase())
            .collect();
        if headers.iter().all(|h| h.is_empty()) {
            continue;
        }

        let idx = |names: &[&str]| -> Option<usize> {
            headers.iter().position(|h| names.iter().any(|n| h == n))
        };

        for row in rows {
            let get = |names: &[&str]| -> Option<String> {
                idx(names)
                    .and_then(|i| row.get(i).map(cell_string))
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            };
            let get_bool = |names: &[&str]| {
                get(names).and_then(|s| match s.to_ascii_lowercase().as_str() {
                    "1" | "true" | "yes" | "y" => Some(true),
                    "0" | "false" | "no" | "n" => Some(false),
                    _ => None,
                })
            };
            let get_u32 = |names: &[&str]| get(names).and_then(|s| s.parse().ok());

            if lower.contains("column") {
                let Some(table) = get(&["table", "table_name", "relation"]) else {
                    continue;
                };
                let Some(col) = get(&["column", "column_name", "name", "field"]) else {
                    continue;
                };
                cat.columns.push(IntermediateColumn {
                    catalog: get(&["catalog"]),
                    database: get(&["database", "db"]),
                    schema: get(&["schema", "schema_name"]),
                    table,
                    name: col,
                    data_type: get(&["data_type", "type"]),
                    nullable: get_bool(&["nullable"]),
                    ordinal: get_u32(&["ordinal", "ordinal_position", "position"]),
                    is_primary_key: get_bool(&["is_primary_key", "pk"]),
                    description: get(&["description", "comment"]),
                });
            } else if lower.contains("relationship") {
                let Some(from_table) = get(&["from_table", "table"]) else {
                    continue;
                };
                let Some(to_table) = get(&["to_table", "ref_table"]) else {
                    continue;
                };
                cat.relationships.push(IntermediateRelationship {
                    name: get(&["name"]),
                    kind: get(&["kind"]),
                    from_schema: get(&["from_schema", "schema"]),
                    from_table,
                    from_column: get(&["from_column", "column"]),
                    to_schema: get(&["to_schema"]),
                    to_table,
                    to_column: get(&["to_column"]),
                });
            } else if lower.contains("depend") || lower.contains("lineage") {
                let Some(from_table) = get(&["from_table", "upstream_table", "source_table"])
                else {
                    continue;
                };
                let Some(to_table) = get(&["to_table", "downstream_table", "target_table"]) else {
                    continue;
                };
                cat.dependencies.push(IntermediateDependency {
                    from_schema: get(&["from_schema"]),
                    from_table,
                    from_column: get(&["from_column"]),
                    to_schema: get(&["to_schema"]),
                    to_table,
                    to_column: get(&["to_column"]),
                    kind: get(&["kind"]),
                });
            } else if lower.contains("table") || lower == "relations" {
                let Some(name) = get(&["table", "table_name", "name", "relation"]) else {
                    continue;
                };
                cat.tables.push(IntermediateTable {
                    catalog: get(&["catalog"]),
                    database: get(&["database", "db"]),
                    schema: get(&["schema", "schema_name"]),
                    name,
                    kind: get(&["kind", "table_type"]),
                    definition: get(&["definition", "sql"]),
                    description: get(&["description", "comment"]),
                });
            }
        }
    }

    if cat.tables.is_empty() && cat.columns.is_empty() {
        return Err(Error::import(format!(
            "no table/column rows found in excel {}",
            path.display()
        )));
    }
    Ok(cat)
}

fn cell_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => {
            if f.fract() == 0.0 {
                format!("{}", *f as i64)
            } else {
                f.to_string()
            }
        }
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(dt) => format!("{dt:?}"),
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("#ERR{e:?}"),
    }
}
