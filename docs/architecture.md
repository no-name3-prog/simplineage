# SimpLineage Architecture

## Vision

SimpLineage is an **offline-first metadata intelligence SDK** for data engineers.
It ingests exported metadata from supported platforms, normalizes it into a
common model, builds lineage graphs, runs impact analysis, compares snapshots,
and generates interactive offline reports.

## Core principles

| Principle | Meaning |
|-----------|---------|
| Database agnostic | Work from exported metadata, not live warehouse connections by default |
| Offline first | Full value without a cloud control plane |
| SDK first | Core logic as libraries; CLI/server/UI are clients |
| Plugin importers | New sources without forking the engine |
| Scalable graph engine | Large metadata graphs remain practical |
| Open-source ready | Clear crates, licenses, and contribution paths |

## Pipeline

```text
Importers → Normalizer → Storage (DuckDB/SQLite)
                              ↓
                        Graph Engine
                              ↓
                       Analysis Engine
                              ↓
                   SDK / CLI / Server / HTML UI
```

## Workspace layout

```text
simplineage/
├── crates/
│   ├── simplineage-core        # Types, config, logging, engine façade
│   ├── simplineage-importers   # Source plugins (CSV, JSON, warehouses, …)
│   ├── simplineage-storage     # Local persistence backends
│   ├── simplineage-analysis    # Impact, cycles, orphans, stats
│   ├── simplineage-exporters   # HTML, JSON, other export formats
│   ├── simplineage-cli         # `simplineage` binary
│   ├── simplineage-server      # HTTP API (future)
│   └── simplineage-bindings    # FFI surface for other languages
├── web/                        # Frontend placeholder (not in Cargo workspace)
├── bindings/                   # Language packaging placeholders
├── config/                     # Default configuration files
└── docs/                       # Architecture and design notes
```

### Crate dependency direction

Dependencies should point **inward** toward core:

```text
cli ─────────┐
server ──────┤
bindings ────┼──► analysis ──► storage ──► core
exporters ───┤         ▲
importers ───┘         │
                       └── importers/exporters may also use core types only
```

Rules of thumb:

1. **`simplineage-core`** must not depend on other workspace crates.
2. Side-effect crates (importers, exporters, server) depend on **core** (and later storage) only as needed.
3. **CLI** may compose any crate; it is the integration binary, not a library surface.

## Configuration

Settings load from (later wins):

1. Built-in defaults  
2. `config/default.toml`  
3. `config/local.toml` (gitignored)  
4. Environment variables: `SIMPLINEAGE_*` with `__` nesting  
   (e.g. `SIMPLINEAGE_LOGGING__LEVEL=debug`)

Runtime logging also respects `RUST_LOG` when set.

## Logging

Structured logging uses [`tracing`](https://docs.rs/tracing) with
`tracing-subscriber`. Formats: human-readable `text` (default) or `json`.

## Current status (Phase 0.1)

| Area | Status |
|------|--------|
| Cargo workspace & crates | Scaffolded |
| Config & logging | Implemented in core |
| Core metadata model | Implemented (Phase 1) |
| Import framework (CSV/JSON/Parquet/Excel) | Implemented |
| Lineage graph engine | Implemented (Phase 3) |
| Analysis engine | Implemented (Phase 4) |
| Hello World CLI | Implemented |
| Importers / storage / analysis / exporters | Placeholders only |
| Server / web / language bindings | Placeholders only |

## Future phases (summary)

1. Real importer plugin system (CSV, JSON, Excel, Parquet)  
2. Normalization into a common metadata model  
3. Directed graph engine with upstream/downstream traversal  
4. Analysis: impact, cycles, orphans, validation, statistics  
5. Rich CLI and offline HTML visualization  
6. Snapshot comparison / lineage evolution  
7. Performance work for large graphs  
8. Warehouse/tool plugins and multi-language SDKs  

## Design notes

- Prefer **safe Rust** (`forbid(unsafe_code)` at crate roots unless FFI requires otherwise).  
- Keep public APIs documented (`missing_docs` warnings on library crates).  
- Workspace-level dependency versions in the root `Cargo.toml` avoid version skew.

## Metadata model

See [metadata-model.md](metadata-model.md) for the vendor-agnostic catalog types.

## Graph engine

See [graph-engine.md](graph-engine.md).

## Analysis engine

See [analysis-engine.md](analysis-engine.md).
