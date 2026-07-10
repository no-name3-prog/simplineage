# Storage (Phase 5)

Local offline persistence uses **SQLite** (`metadata.sqlite` under the data directory).

SQLite is intentionally lightweight (fast local builds, small binary, no heavy C++ engine).

## Capabilities

| Feature | API |
|---------|-----|
| Open / migrate | `MetadataStore::open(data_dir)` |
| Save / load snapshots | `save_snapshot`, `load_snapshot`, `load_current`, `list_snapshots` |
| Incremental import | `import(snapshot, ImportMode::Replace \| Merge, source)` |
| Fast graph rebuild | `load_dependencies(snapshot_id)` → `LineageGraph::from_dependencies` |
| Schema migrations | automatic on open (`schema_migrations`, version 1) |

## Schema (v1)

- `snapshots` — catalog row + full JSON `payload`
- `lineage_edges` — materialized dependency edges per snapshot
- `snapshot_objects` — object id/type/fqn index
- `imports` — incremental import audit log
- `schema_migrations` — applied versions

## Incremental modes

- **Replace** — store incoming snapshot as the new current head
- **Merge** — union objects/edges with current head (id last-write-wins), new snapshot id

## Fast graph reconstruction

Prefer edges over full JSON when only lineage is needed:

```rust
let deps = store.load_dependencies(&snapshot_id)?;
let graph = LineageGraph::from_dependencies(deps);
```

## Benchmarks

```bash
cargo bench -p simplineage-storage --bench storage_bench
```

## Example

```rust
use simplineage_storage::{MetadataStore, ImportMode};

let store = MetadataStore::open(".simplineage")?;
store.import(snapshot, ImportMode::Merge, Some("csv:daily"))?;
let head = store.load_current()?.expect("current");
let edges = store.load_dependencies(&head.id)?;
```
