//! CSV importer for intermediate catalog rows.

use std::fs::File;
use std::path::Path;

use simplineage_core::{Error, Result, Snapshot};

use crate::detect::{first_text_line, has_extension};
use crate::intermediate::IntermediateCatalog;
use crate::normalize::intermediate_to_snapshot;
use crate::options::ImportOptions;
use crate::plugin::{DetectConfidence, MetadataImporter};
use crate::tabular::{self, FieldGet};

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

    // Pre-compute force flags once (headers are fixed for the file).
    let bq_tables = headers.iter().any(|h| h == "table_name")
        && headers
            .iter()
            .any(|h| h == "table_type" || h == "table_schema")
        && !headers.iter().any(|h| h == "column_name");
    let bq_columns =
        headers.iter().any(|h| h == "column_name") && headers.iter().any(|h| h == "table_name");

    for rec in rdr.records() {
        let rec = rec.map_err(|e| Error::import(format!("csv row: {e}")))?;
        let row = CsvRow {
            headers: &headers,
            rec: &rec,
        };

        let entity = entity_col
            .and_then(|i| rec.get(i))
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();

        let force_table = entity.is_empty()
            && !bq_columns
            && (stem.contains("table")
                || bq_tables
                || headers.iter().any(|h| h == "kind")
                || (headers.iter().any(|h| h == "table_name")
                    && !headers.iter().any(|h| h == "column_name")));
        let force_column = entity.is_empty()
            && (stem.contains("column")
                || bq_columns
                || (headers.iter().any(|h| h == "column" || h == "column_name")
                    && headers.iter().any(|h| h == "table" || h == "table_name")));

        if entity == "table" || entity == "view" || entity == "materialized_view" || force_table {
            let before = cat.tables.len();
            let hint = if entity == "view" || entity == "materialized_view" {
                Some(entity.clone())
            } else {
                None
            };
            tabular::push_table(&mut cat, &row, hint);
            if cat.tables.len() == before {
                return Err(Error::import("csv table row missing name/table"));
            }
        } else if entity == "column" || force_column {
            if !tabular::push_column(&mut cat, &row) {
                return Err(Error::import("csv column row missing table/column"));
            }
        } else if entity == "relationship" || entity == "foreign_key" || entity == "fk" {
            if !tabular::push_relationship(&mut cat, &row) {
                return Err(Error::import("relationship missing from_table/to_table"));
            }
        } else if entity == "dependency" || entity == "lineage" {
            if !tabular::push_dependency(&mut cat, &row) {
                return Err(Error::import("dependency missing from_table/to_table"));
            }
        } else if !entity.is_empty() {
            tracing::debug!(%entity, "skipping csv row with unknown entity_type");
        } else if row.get(&["column", "column_name"]).is_some() {
            if !tabular::push_column(&mut cat, &row) {
                return Err(Error::import("csv row missing table/column"));
            }
        } else {
            tabular::push_table(&mut cat, &row, None);
        }
    }

    if cat.tables.is_empty()
        && cat.columns.is_empty()
        && cat.relationships.is_empty()
        && cat.dependencies.is_empty()
    {
        return Err(Error::import(format!(
            "no table/column/relationship/dependency rows recognized in {}",
            path.display()
        )));
    }
    Ok(cat)
}

/// Adapter so CSV records implement [`FieldGet`].
struct CsvRow<'a> {
    headers: &'a [String],
    rec: &'a csv::StringRecord,
}

impl FieldGet for CsvRow<'_> {
    fn get(&self, names: &[&str]) -> Option<String> {
        let i = self
            .headers
            .iter()
            .position(|h| names.iter().any(|n| h == n))?;
        self.rec
            .get(i)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }
}
