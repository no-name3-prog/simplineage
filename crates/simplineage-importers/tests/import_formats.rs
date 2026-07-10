//! Integration tests for built-in format importers.

use std::io::Write;
use std::sync::Arc;

use arrow_array::{ArrayRef, Int32Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use parquet::arrow::ArrowWriter;
use simplineage_importers::{ImportOptions, ImporterRegistry};
use tempfile::{NamedTempFile, tempdir};

fn registry() -> ImporterRegistry {
    ImporterRegistry::with_builtins()
}

#[test]
fn detects_and_imports_json_intermediate() {
    let mut f = NamedTempFile::with_suffix(".json").unwrap();
    write!(
        f,
        r#"{{
          "source": "unit",
          "tables": [{{"schema":"public","name":"orders","kind":"table"}}],
          "columns": [{{"schema":"public","table":"orders","name":"id","data_type":"BIGINT","nullable":false}}]
        }}"#
    )
    .unwrap();
    let reg = registry();
    let det = reg.detect(f.path()).unwrap();
    assert_eq!(det.importer.id(), "json");
    let snap = reg
        .import_path(f.path(), &ImportOptions::default())
        .unwrap();
    assert_eq!(snap.tables.len(), 1);
    assert_eq!(snap.columns.len(), 1);
}

#[test]
fn imports_csv_entity_type() {
    let mut f = NamedTempFile::with_suffix(".csv").unwrap();
    writeln!(f, "entity_type,schema,table,column,data_type,kind,nullable").unwrap();
    writeln!(f, "table,public,orders,, ,table,").unwrap();
    writeln!(f, "column,public,orders,id,BIGINT,,false").unwrap();
    writeln!(f, "column,public,orders,customer_id,BIGINT,,true").unwrap();
    let snap = registry()
        .import_path(f.path(), &ImportOptions::default())
        .unwrap();
    assert_eq!(snap.tables.len(), 1);
    assert_eq!(snap.columns.len(), 2);
}

#[test]
fn imports_parquet_columns() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("columns.parquet");
    let file = std::fs::File::create(&path).unwrap();
    let schema = Arc::new(Schema::new(vec![
        Field::new("schema", DataType::Utf8, true),
        Field::new("table", DataType::Utf8, false),
        Field::new("column", DataType::Utf8, false),
        Field::new("data_type", DataType::Utf8, true),
        Field::new("ordinal", DataType::Int32, true),
    ]));
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(StringArray::from(vec!["public", "public"])) as ArrayRef,
            Arc::new(StringArray::from(vec!["orders", "orders"])) as ArrayRef,
            Arc::new(StringArray::from(vec!["id", "amount"])) as ArrayRef,
            Arc::new(StringArray::from(vec!["BIGINT", "DECIMAL"])) as ArrayRef,
            Arc::new(Int32Array::from(vec![Some(0), Some(1)])) as ArrayRef,
        ],
    )
    .unwrap();
    let mut writer = ArrowWriter::try_new(file, schema, None).unwrap();
    writer.write(&batch).unwrap();
    writer.close().unwrap();

    let reg = registry();
    assert_eq!(reg.detect(&path).unwrap().importer.id(), "parquet");
    let snap = reg.import_path(&path, &ImportOptions::default()).unwrap();
    assert_eq!(snap.columns.len(), 2);
    assert_eq!(snap.tables.len(), 1); // synthetic parent
}

#[test]
fn list_builtin_ids() {
    let ids = registry().ids();
    for expected in ["csv", "json", "parquet", "excel"] {
        assert!(ids.contains(&expected), "missing {expected}");
    }
}
