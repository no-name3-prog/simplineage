# BigQuery INFORMATION_SCHEMA sample dump

Synthetic CSV exports shaped like BigQuery `INFORMATION_SCHEMA` results:

| File | Source shape |
|------|----------------|
| `TABLES.csv` | `INFORMATION_SCHEMA.TABLES` |
| `COLUMNS.csv` | `INFORMATION_SCHEMA.COLUMNS` |
| `LINEAGE.csv` | Optional dependency edges (not native IS; for graph demo) |

## Export similarly from BigQuery

```sql
-- TABLES
SELECT table_catalog, table_schema, table_name, table_type, creation_time, ddl
FROM `project.region-us.INFORMATION_SCHEMA.TABLES`
WHERE table_schema IN ('raw', 'staging', 'marts');

-- COLUMNS
SELECT table_catalog, table_schema, table_name, column_name,
       ordinal_position, is_nullable, data_type, is_partitioning_column
FROM `project.region-us.INFORMATION_SCHEMA.COLUMNS`
WHERE table_schema IN ('raw', 'staging', 'marts');
```

Then:

```bash
cargo run -q -p simplineage-cli -- \
  --data-dir /tmp/simplineage-bq-store \
  import examples/bigquery_information_schema \
  --label bq-demo \
  --output /tmp/bq-snapshot.json \
  --analyze

# then: search / impact / export
cargo run -q -p simplineage-cli -- --data-dir /tmp/simplineage-bq-store search orders
cargo run -q -p simplineage-cli -- --data-dir /tmp/simplineage-bq-store impact orders --direction both
```
