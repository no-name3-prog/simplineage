# SimpLineage CLI

Professional offline CLI built with [clap](https://docs.rs/clap).

Binary name: **`simplineage`**

## Install / run

```bash
cargo build -p simplineage-cli
cargo run -p simplineage-cli -- --help

# after install
cargo install --path crates/simplineage-cli
simplineage --help
```

## Global flags

| Flag | Env | Description |
|------|-----|-------------|
| `--data-dir` / `-d` | `SIMPLINEAGE_DATA_DIR` | SQLite store directory (default: `.simplineage`) |
| `--config-dir` | `SIMPLINEAGE_CONFIG_DIR` | Config directory (default: `config`) |
| `--log-level` | `SIMPLINEAGE_LOGGING__LEVEL` | `trace`…`error` |
| `--json` | `SIMPLINEAGE_JSON` | Machine-readable JSON output |
| `--quiet` / `-q` | | Less decoration |
| `--no-color` | `NO_COLOR` | Disable ANSI colors |
| `--no-progress` | | Disable spinners / progress bars |

## Commands

### `import`

Ingest a file or directory (CSV / JSON / Parquet / Excel / sample warehouse).

```bash
simplineage import ./export.csv
simplineage import ./exports/ --mode merge --label nightly
simplineage import ./snap.json --importer json --output snap.out.json --no-store
simplineage import --list-importers
```

By default the snapshot is written to the local store (unless `--no-store` or only `--output` without `--store`).

### `build`

Construct the lineage graph from the current store head (or a snapshot file/id).

```bash
simplineage build
simplineage build --full -o analysis.json
simplineage build --snapshot ./snap.json
```

### `search`

Find objects by id, FQN, name, or description substring.

```bash
simplineage search orders
simplineage search orders --kind table -n 20
simplineage search --json "mart."
```

### `upstream` / `downstream`

Traverse lineage from an object (id, FQN, or unique name fragment).
Accepts **column** FQNs (`schema.table.column`) and optional `--column` under a parent table.

```bash
simplineage upstream public.orders
simplineage downstream table:orders --max-depth 3 --relations-only
# Column grain
simplineage downstream public.orders.email --level column
simplineage upstream table:orders --column email --level column
```

| Flag | Meaning |
|------|---------|
| `--level all` | Default — return every related object (tables and columns) |
| `--level relation` | Tables / views / MVs only (same as `--relations-only`) |
| `--level column` | Column objects only |
| `--column NAME` | Resolve `NAME` under the parent identified by `object` |
| `--relations-only` | Alias for `--level relation` (kept for compatibility) |

### `impact`

Bidirectional (or single-direction) impact analysis at table or column grain.

```bash
simplineage impact public.orders --direction both
simplineage impact orders -D downstream --json
# Column impact
simplineage impact public.orders.email --level column -D both
simplineage impact public.orders --column email --level column --json
```

JSON output keeps `upstream` / `downstream` as **id string arrays** for scripting, and adds
`subject_fqn`, `subject_kind`, `level`, plus enriched `upstream_nodes` / `downstream_nodes`
(`id`, `fqn`, `kind`).

### `validate`

Dependency integrity + metadata quality checks (non-zero exit on failure).

```bash
simplineage validate
simplineage validate --strict --json
```

### `stats`

Store catalog + graph statistics.

```bash
simplineage stats
simplineage stats --store-only
```

### `compare`

Diff two snapshots by object/edge ids (store ids or JSON files).

```bash
simplineage compare snap-a.json snap-b.json
simplineage compare <id-a> <id-b> --json
```

### `export`

Write artifacts for tooling / review.

| Format | Description |
|--------|-------------|
| `json` / `json-pretty` | Full snapshot |
| `objects-csv` | Catalog objects |
| `edges-csv` | Dependency edges |
| `graphml` | Desktop graph tools |
| `html` | **Interactive offline lineage report** (pan/zoom/search/filters/impact/dark mode/SVG+Mermaid download) |
| `mermaid` | Mermaid flowchart text (`.mmd`) |
| `analysis` | Full analysis report JSON |

```bash
# Self-contained HTML — open in any browser, no server
simplineage export -f html -o report.html

simplineage export -f mermaid -o lineage.mmd
simplineage export -f graphml -o lineage.graphml
simplineage export --analysis -o full.json
```

See [html-export.md](html-export.md) for the interactive report features.

### Compatibility

`hello`, `version`, and `status` remain available for smoke tests and scripting.

## Typical workflow

```bash
simplineage import ./warehouse_export/ --label prod
simplineage build --full
simplineage search customers
simplineage impact public.customers --direction both
simplineage validate
simplineage export -f html -o lineage.html
```

Run the same workflow without installing Rust: **[docker.md](docker.md)**.

## Progress & rich output

Long-running operations show **spinners** (and a **progress bar** when importing multi-file directories). Colors use ANSI when stdout is a TTY; disable with `--no-color` or `NO_COLOR`.
