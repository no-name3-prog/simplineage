# Metadata import framework

Plugin-based import of offline metadata exports into the common
[`Snapshot`](metadata-model.md) model.

## Architecture

```text
                    ┌─────────────────────────┐
  CSV/JSON/…  ───►  │  MetadataImporter trait │  ◄── warehouse crates
  Parquet/Excel     │  (simplineage-importers)│      (one crate each)
                    └───────────┬─────────────┘
                                │
                                ▼
                         IntermediateCatalog
                                │
                                ▼
                            Snapshot
```

- **Format plugins** (built-in): `csv`, `json`, `parquet`, `excel`
- **Warehouse plugins**: separate crates implementing `MetadataImporter` and
  calling `ImporterRegistry::register`
- **Auto-detection**: `ImporterRegistry::detect` / `import_path` pick the best
  plugin by extension + content probes

## Adding a warehouse

1. `cargo new --lib crates/simplineage-importer-<name>`
2. Depend on `simplineage-importers` and `simplineage-core`
3. Implement `MetadataImporter`
4. Export `pub fn register(registry: &mut ImporterRegistry)`
5. From CLI/app: `my_crate::register(&mut registry);`

See `simplineage-importer-sample` for a complete example.

No changes to `simplineage-core` or built-in format importers are required.

## Intermediate CSV / Excel layout

### Tables sheet / file

| schema | table | kind | definition |
|--------|-------|------|------------|
| public | orders | table | |

### Columns sheet / file

| schema | table | column | data_type | nullable | ordinal |
|--------|-------|--------|-----------|----------|---------|
| public | orders | id | BIGINT | false | 0 |

### Multi-entity CSV

Include `entity_type` = `table` | `column` | `relationship` | `dependency`.

## JSON

1. Full SimpLineage `Snapshot` JSON, or
2. Intermediate object:

```json
{
  "tables": [{"schema": "public", "name": "orders", "kind": "table"}],
  "columns": [{"schema": "public", "table": "orders", "name": "id", "data_type": "BIGINT"}]
}
```

## CLI

```bash
cargo run -p simplineage-cli -- import ./metadata.json
cargo run -p simplineage-cli -- import ./exports/ --output snapshot.json
cargo run -p simplineage-cli -- import ./file.csv --importer csv
cargo run -p simplineage-cli -- import --list-importers /dev/null
```
