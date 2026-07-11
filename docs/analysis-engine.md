# Analysis engine (Phase 4)

High-level API in `simplineage-analysis` built on the [graph engine](graph-engine.md).

## Entry point

```rust
use simplineage_analysis::{AnalysisEngine, ImpactOptions, ImpactDirection};

let engine = AnalysisEngine::from_snapshot(snapshot);

// Individual analyses
let impact = engine.impact(&node_id, &ImpactOptions {
    direction: ImpactDirection::Both,
    ..Default::default()
})?;

let validation = engine.validate_dependencies();
let orphans = engine.orphans();
let unused = engine.unused_objects();
let cycles = engine.circular_dependencies();
let critical = engine.critical_tables(&Default::default());
let chains = engine.longest_chains(&Default::default());
let quality = engine.quality_checks();

// Or everything at once
let full = engine.analyze_all();
```

## Capabilities

| Method | Description |
|--------|-------------|
| `impact` | Upstream / downstream / both blast radius (`relations_only` / `columns_only` filters) |
| `object_kind` | Catalog kind label for an id when known |
| `validate_dependencies` | Missing endpoints, self-loops, confidence, duplicates |
| `unused_objects` | Nodes with consumers missing (in>0, out=0) |
| `orphans` | Fully disconnected nodes (in=0, out=0) |
| `circular_dependencies` | SCC-based cycle groups |
| `critical_tables` | Ranked by downstream impact score |
| `longest_chains` | Longest simple dependency paths |
| `quality_checks` | Descriptions, types, empty relations, cycles, orphan ratio, score 0–100 |
| `analyze_all` | Bundle of the above + graph statistics |

## Reports

All report types live in `simplineage_analysis::reports` and derive `Serialize` for JSON export.

Prefer `AnalysisEngine` over the thinner `Analyzer` façade (kept for compatibility).
