//! Parquet importer — tabular intermediate catalog columns.

use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow_array::{Array, RecordBatch, StringArray};
use arrow_cast::cast;
use arrow_schema::DataType as ArrowDataType;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use simplineage_core::{Error, Result, Snapshot};

use crate::detect::{has_extension, looks_like_parquet};
use crate::intermediate::IntermediateCatalog;
use crate::normalize::intermediate_to_snapshot;
use crate::options::ImportOptions;
use crate::plugin::{DetectConfidence, MetadataImporter};
use crate::tabular::{self, FieldGet};

/// Imports metadata from Parquet files with table/column-style columns.
#[derive(Debug, Default)]
pub struct ParquetImporter;

impl MetadataImporter for ParquetImporter {
    fn id(&self) -> &'static str {
        "parquet"
    }

    fn name(&self) -> &str {
        "Parquet metadata"
    }

    fn description(&self) -> &str {
        "Parquet tables with schema/table/column fields"
    }

    fn extensions(&self) -> &[&str] {
        &["parquet", "parq"]
    }

    fn detect(&self, path: &Path) -> DetectConfidence {
        if !path.is_file() {
            return DetectConfidence::None;
        }
        if looks_like_parquet(path) {
            return DetectConfidence::High;
        }
        if has_extension(path, &["parquet", "parq"]) {
            return DetectConfidence::Medium;
        }
        DetectConfidence::None
    }

    fn import(&self, path: &Path, options: &ImportOptions) -> Result<Snapshot> {
        let cat = parse_parquet(path)?;
        let mut opts = options.clone();
        if opts.source.is_none() {
            opts.source = Some(format!("parquet:{}", path.display()));
        }
        intermediate_to_snapshot(&cat, &opts)
    }
}

fn parse_parquet(path: &Path) -> Result<IntermediateCatalog> {
    let file = File::open(path)
        .map_err(|e| Error::import(format!("open parquet {}: {e}", path.display())))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .map_err(|e| Error::import(format!("parquet reader: {e}")))?;
    let schema = builder.schema().clone();
    let reader = builder
        .build()
        .map_err(|e| Error::import(format!("parquet build: {e}")))?;

    let field_names: Vec<String> = schema
        .fields()
        .iter()
        .map(|f| f.name().to_ascii_lowercase())
        .collect();
    let has_column = field_names.iter().any(|n| {
        matches!(n.as_str(), "column" | "column_name" | "field" | "name")
            && field_names
                .iter()
                .any(|t| t == "table" || t == "table_name")
    });
    // More precise: if both table and column-like present, treat as columns file
    let is_columns = field_names
        .iter()
        .any(|n| n == "column" || n == "column_name")
        || (field_names.iter().any(|n| n == "name")
            && field_names
                .iter()
                .any(|n| n == "table" || n == "table_name")
            && field_names.iter().any(|n| n == "data_type" || n == "type"));

    let mut cat = IntermediateCatalog {
        source: Some(path.display().to_string()),
        ..Default::default()
    };

    for batch in reader {
        let batch = batch.map_err(|e| Error::import(format!("parquet batch: {e}")))?;
        let rows = batch_to_string_rows(&batch)?;
        for row in rows {
            if is_columns {
                let _ = tabular::push_column(&mut cat, &row);
            } else {
                tabular::push_table(&mut cat, &row, None);
            }
        }
    }

    let _ = has_column;
    if cat.tables.is_empty() && cat.columns.is_empty() {
        return Err(Error::import(format!(
            "no table/column rows in parquet {}",
            path.display()
        )));
    }
    Ok(cat)
}

fn batch_to_string_rows(batch: &RecordBatch) -> Result<Vec<HashMapLike>> {
    let n = batch.num_rows();
    let mut cols: Vec<(String, Arc<dyn Array>)> = Vec::new();
    for (i, field) in batch.schema().fields().iter().enumerate() {
        let array = batch.column(i);
        let casted = cast(array.as_ref(), &ArrowDataType::Utf8)
            .map_err(|e| Error::import(format!("cast column {}: {e}", field.name())))?;
        cols.push((field.name().to_ascii_lowercase(), casted));
    }
    let mut rows = Vec::with_capacity(n);
    for row_idx in 0..n {
        let mut map = HashMapLike::default();
        for (name, arr) in &cols {
            let sarr = arr
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| Error::import("expected utf8 array after cast"))?;
            if !sarr.is_null(row_idx) {
                map.0.insert(name.clone(), sarr.value(row_idx).to_string());
            }
        }
        rows.push(map);
    }
    Ok(rows)
}

#[derive(Default)]
struct HashMapLike(std::collections::HashMap<String, String>);

impl FieldGet for HashMapLike {
    fn get(&self, names: &[&str]) -> Option<String> {
        for n in names {
            if let Some(v) = self.0.get(*n) {
                if !v.trim().is_empty() {
                    return Some(v.clone());
                }
            }
        }
        None
    }
}
