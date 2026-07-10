# Core metadata model (Phase 1)

Vendor-agnostic types live in `simplineage-core::model`.

## Hierarchy

```text
Catalog → Database → Schema → Table | View | MaterializedView → Column
```

Levels may be omitted when a source system does not use them (e.g. no catalog).
Importers map vendor concepts into this shape rather than forking the model.

## Types

| Type | Role |
|------|------|
| `Catalog` | Top-level namespace / metastore |
| `Database` | Mid-level namespace |
| `Schema` | Container for relations |
| `Table` | Base table |
| `View` | Non-materialized view |
| `MaterializedView` | Materialized view |
| `Column` | Column of a relation |
| `Relationship` | Structural link (FK, unique, …) |
| `Dependency` | Directed lineage edge (upstream → downstream) |
| `Snapshot` | Versioned point-in-time capture of the above |

## Versioning

- Constant: `MODEL_VERSION` (`1.0.0`)
- Field: `Snapshot.model_version`
- Compatibility: same **major** version is accepted on load

## Serialization

All model types implement `Serialize` / `Deserialize` (JSON via `serde_json`).

```rust
let json = snapshot.to_json_pretty()?;
let snap = Snapshot::from_json(json.as_bytes())?;
snap.validate()?;
```

## Validation

`Validate` / `validate_snapshot` checks:

- model version compatibility
- unique object ids
- parent / FK / dependency referential integrity
- non-empty names

## Non-goals (Phase 1)

- Vendor-specific type enums
- Graph algorithms (later phases)
- Persistence backends
