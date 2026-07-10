# Lineage graph engine (Phase 3)

The graph engine lives in `simplineage-analysis` and builds a **directed**
dependency graph from metadata snapshots.

## Edge direction

Follows the core model:

```text
upstream (producer) ──► downstream (consumer)
Dependency.from_id  ──► Dependency.to_id
```

Example: `orders → order_facts` means `order_facts` depends on `orders`.

- **Upstream** of a node = reverse traversal (providers / impact of changing dependents looking back)
- **Downstream** of a node = forward traversal (impact analysis)

## API surface

| API | Description |
|-----|-------------|
| `LineageGraph::from_snapshot` | Build graph from `Snapshot` dependencies (+ all object nodes) |
| `LineageGraph::from_dependencies` | Build from raw edges |
| `upstream` / `downstream` | BFS traversal with optional `max_depth` |
| `shortest_path` | Fewest-hop path (BFS) |
| `all_paths` | Simple paths (DFS), bounded by `max_paths` / `max_path_length` |
| `detect_cycles` / `has_cycle` | Tarjan SCCs + self-loops |
| `statistics` | Density, degrees, components, sources/sinks, cycles |

## Performance

- Compact `NodeIndex` adjacency via **petgraph** `DiGraph`
- `HashMap<ObjectId, NodeIndex>` for O(1) lookup
- BFS for reachability / shortest path; DFS for path enumeration with cycle skips
- Criterion benches: `cargo bench -p simplineage-analysis`

```bash
cargo bench -p simplineage-analysis --bench graph_bench
```

## Example

```rust
use simplineage_analysis::{LineageGraph, TraversalOptions};

let g = LineageGraph::from_snapshot(&snapshot);
let impact = g.downstream(&node_id, &TraversalOptions::default())?;
let cycles = g.detect_cycles();
let stats = g.statistics();
```
