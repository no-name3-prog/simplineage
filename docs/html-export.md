# Offline HTML lineage report (Phase 7)

SimpLineage can generate a **single self-contained HTML file** that visualizes
lineage interactively. Opening the file in a browser is enough — **no backend
server, CDN, or network access** is required.

## Generate

```bash
simplineage import ./metadata_export --label demo
simplineage export -f html -o lineage.html
# open lineage.html in Chrome / Firefox / Safari / Edge
```

Or from a snapshot JSON file:

```bash
simplineage export -f html -s ./snap.json -o lineage.html
```

Using Docker only (no host Rust): see **[docker.md](docker.md)** — mount `examples/` and write HTML to a host folder such as `./out/lineage.html`.

## Features

| Feature | Behavior |
|---------|----------|
| **Zoom** | Mouse wheel / trackpad over the canvas; toolbar `+` / `−` |
| **Pan** | Drag empty canvas background |
| **Search** | Match by id, FQN, name, or kind; **keeps matches + their full upstream/downstream bright** and dims the rest |
| **Filters** | Kind checkboxes; *Relations only*; *Hide isolated* |
| **Metadata side panel** | Click a node for id, FQN, description, neighbors |
| **Lineage focus (click)** | Click a node → **auto-highlight both upstream and downstream**; all other nodes/edges dim. Click empty canvas or **Clear** to reset |
| **Impact analysis** | Upstream / Downstream / Both narrow the auto-focus (client-side BFS on embedded edges) |
| **Dark mode** | Theme toggle (prefers-color-scheme + localStorage) |
| **SVG export** | Download current graph as standalone SVG |
| **Mermaid export** | Download visible graph as Mermaid flowchart (also `export -f mermaid`) |

## Design

- Rust builds a layered layout + JSON payload (`simplineage_exporters::html`)
- CSS/JS are **embedded** via `include_str!` — no external scripts
- Edge direction matches the model: **upstream → downstream**

## Library API

```rust
use simplineage_exporters::{export_snapshot, ExportFormat, write_html_report, write_mermaid, MermaidOptions};
use simplineage_core::Snapshot;

fn demo(snap: &Snapshot) {
    write_html_report(snap, std::path::Path::new("out.html")).unwrap();
    write_mermaid(snap, std::path::Path::new("out.mmd"), &MermaidOptions::default()).unwrap();
    export_snapshot(snap, std::path::Path::new("out2.html"), ExportFormat::Html).unwrap();
}
```

## Limitations

- Very large graphs (thousands of nodes) remain usable with filters, but the
  default layout is a simple layered packing — not a force-directed solver.
- Column nodes are hidden when *Relations only* is checked (default).
