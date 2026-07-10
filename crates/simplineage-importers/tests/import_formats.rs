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

#[test]
fn imports_bigquery_information_schema_tables_csv() {
    let mut f = NamedTempFile::with_suffix(".csv").unwrap();
    writeln!(
        f,
        "table_catalog,table_schema,table_name,table_type,creation_time,ddl"
    )
    .unwrap();
    writeln!(
        f,
        "proj,raw,orders,BASE TABLE,2024-01-01,CREATE TABLE raw.orders (...)"
    )
    .unwrap();
    writeln!(
        f,
        "proj,staging,stg_orders,VIEW,2024-01-02,CREATE VIEW staging.stg_orders AS SELECT 1"
    )
    .unwrap();
    writeln!(
        f,
        "proj,marts,orders_daily,MATERIALIZED VIEW,2024-01-03,CREATE MATERIALIZED VIEW marts.orders_daily AS SELECT 1"
    )
    .unwrap();

    let snap = registry()
        .import_path(f.path(), &ImportOptions::default())
        .unwrap();
    assert_eq!(snap.tables.len(), 1, "BASE TABLE → table");
    assert_eq!(snap.views.len(), 1, "VIEW → view");
    assert_eq!(snap.materialized_views.len(), 1, "MATERIALIZED VIEW → mv");
    assert_eq!(snap.tables[0].meta.name, "orders");
    assert!(
        snap.tables[0].meta.fqn.to_dotted().contains("proj")
            || snap.tables[0].meta.fqn.to_dotted().contains("raw"),
        "catalog/schema should appear in FQN: {}",
        snap.tables[0].meta.fqn.to_dotted()
    );
}

#[test]
fn imports_bigquery_information_schema_columns_csv() {
    let mut f = NamedTempFile::with_suffix(".csv").unwrap();
    writeln!(
        f,
        "table_catalog,table_schema,table_name,column_name,ordinal_position,is_nullable,data_type,is_partitioning_column"
    )
    .unwrap();
    writeln!(f, "proj,raw,orders,order_id,1,NO,INT64,NO").unwrap();
    writeln!(f, "proj,raw,orders,amount,2,YES,NUMERIC,NO").unwrap();
    writeln!(f, "proj,raw,orders,email,3,YES,STRING,NO").unwrap();

    let snap = registry()
        .import_path(f.path(), &ImportOptions::default())
        .unwrap();
    // Columns file synthesizes parent table when missing.
    assert_eq!(snap.tables.len() + snap.views.len(), 1);
    assert_eq!(snap.columns.len(), 3);

    let order_id = snap
        .columns
        .iter()
        .find(|c| c.meta.name == "order_id")
        .expect("order_id");
    assert!(!order_id.nullable, "NO → not nullable");
    assert_eq!(order_id.ordinal, Some(1));
    assert!(
        matches!(
            order_id.data_type,
            simplineage_core::model::types::DataType::Integer { bits: Some(64) }
        ),
        "INT64 maps to Integer(64), got {:?}",
        order_id.data_type
    );

    let amount = snap
        .columns
        .iter()
        .find(|c| c.meta.name == "amount")
        .expect("amount");
    assert!(amount.nullable, "YES → nullable");

    let email = snap
        .columns
        .iter()
        .find(|c| c.meta.name == "email")
        .expect("email");
    assert!(
        matches!(
            email.data_type,
            simplineage_core::model::types::DataType::String { .. }
        ),
        "STRING maps to String, got {:?}",
        email.data_type
    );
}

#[test]
fn imports_bigquery_example_directory_end_to_end() {
    let dir = bq_example_dir();
    assert!(
        dir.join("TABLES.csv").is_file(),
        "missing example TABLES.csv at {}",
        dir.display()
    );

    let snap = registry()
        .import_dir(
            &dir,
            &ImportOptions {
                label: Some("bq-demo".into()),
                ..Default::default()
            },
        )
        .expect("import examples/bigquery_information_schema");

    assert_eq!(snap.label.as_deref(), Some("bq-demo"));
    // 4 BASE TABLE + 2 VIEW + 1 MATERIALIZED VIEW
    assert_eq!(snap.tables.len(), 4, "tables: {:?}", table_names(&snap));
    assert_eq!(snap.views.len(), 2, "views");
    assert_eq!(snap.materialized_views.len(), 1, "mvs");
    assert_eq!(snap.columns.len(), 23, "columns");
    assert_eq!(snap.dependencies.len(), 5, "lineage edges");

    // Edges should point at full relation ids (catalog.schema.table), not only short stubs.
    for dep in &snap.dependencies {
        assert!(
            snap.object_index().contains_key(&dep.from_id)
                || snap.tables.iter().any(|t| t.meta.id == dep.from_id)
                || snap.views.iter().any(|v| v.meta.id == dep.from_id)
                || snap
                    .materialized_views
                    .iter()
                    .any(|m| m.meta.id == dep.from_id),
            "upstream {} missing after reconcile",
            dep.from_id
        );
        assert!(
            snap.object_index().contains_key(&dep.to_id)
                || snap.tables.iter().any(|t| t.meta.id == dep.to_id)
                || snap.views.iter().any(|v| v.meta.id == dep.to_id)
                || snap
                    .materialized_views
                    .iter()
                    .any(|m| m.meta.id == dep.to_id),
            "downstream {} missing after reconcile",
            dep.to_id
        );
    }

    // README.md must not break directory import (extension filter).
    assert!(dir.join("README.md").is_file());
}

#[test]
fn directory_import_skips_non_data_files_and_merges() {
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("README.md"),
        "# not metadata\ntable,column\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("TABLES.csv"),
        "table_catalog,table_schema,table_name,table_type\n\
         c,s,a,BASE TABLE\n\
         c,s,b,VIEW\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("COLUMNS.csv"),
        "table_catalog,table_schema,table_name,column_name,ordinal_position,is_nullable,data_type\n\
         c,s,a,id,1,NO,INT64\n\
         c,s,b,x,1,YES,STRING\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("LINEAGE.csv"),
        "entity_type,from_schema,from_table,to_schema,to_table,kind\n\
         dependency,s,a,s,b,view_definition\n",
    )
    .unwrap();

    let snap = registry()
        .import_dir(dir.path(), &ImportOptions::default())
        .unwrap();
    assert_eq!(snap.tables.len(), 1);
    assert_eq!(snap.views.len(), 1);
    assert_eq!(snap.columns.len(), 2);
    assert_eq!(snap.dependencies.len(), 1);
}

#[test]
fn lineage_only_csv_imports_dependencies() {
    let mut f = NamedTempFile::with_suffix(".csv").unwrap();
    // Force filename-like entity_type layout (stem may not contain lineage if tempfile random).
    writeln!(
        f,
        "entity_type,from_schema,from_table,to_schema,to_table,kind"
    )
    .unwrap();
    writeln!(
        f,
        "dependency,raw,orders,staging,stg_orders,view_definition"
    )
    .unwrap();

    let snap = registry()
        .import_path(f.path(), &ImportOptions::default())
        .unwrap();
    assert_eq!(snap.dependencies.len(), 1);
    // Dependency endpoints create relation stubs during normalize.
    assert!(
        snap.tables.len() + snap.views.len() + snap.materialized_views.len() >= 2,
        "expected stub relations for lineage endpoints"
    );
}

fn bq_example_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/bigquery_information_schema")
        .canonicalize()
        .expect("examples/bigquery_information_schema")
}

fn table_names(snap: &simplineage_core::Snapshot) -> Vec<String> {
    snap.tables.iter().map(|t| t.meta.fqn.to_dotted()).collect()
}
