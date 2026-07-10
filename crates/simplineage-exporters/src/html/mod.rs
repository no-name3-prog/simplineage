//! Offline interactive HTML lineage report (no server required).

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fs;
use std::path::Path;

use serde::Serialize;
use simplineage_core::model::graph::DependencyKind;
use simplineage_core::{Error, Result, Snapshot};

const REPORT_CSS: &str = include_str!("report.css");
const REPORT_JS: &str = include_str!("report.js");

/// Graph payload embedded in the HTML document.
#[derive(Debug, Clone, Serialize)]
pub struct ReportData {
    /// Snapshot / report metadata.
    pub meta: ReportMeta,
    /// Aggregate counts.
    pub stats: ReportStats,
    /// Layout-ready nodes.
    pub nodes: Vec<ReportNode>,
    /// Directed lineage edges (upstream → downstream).
    pub edges: Vec<ReportEdge>,
}

/// Header metadata for the report.
#[derive(Debug, Clone, Serialize)]
pub struct ReportMeta {
    /// Snapshot id.
    pub snapshot_id: String,
    /// Optional label.
    pub label: Option<String>,
    /// Optional source system.
    pub source: Option<String>,
    /// Model version string.
    pub model_version: String,
    /// Product that generated the report.
    pub generator: String,
    /// Product version.
    pub generator_version: String,
}

/// Counts shown in the report chrome.
#[derive(Debug, Clone, Serialize)]
pub struct ReportStats {
    /// Catalog objects.
    pub catalogs: usize,
    /// Databases.
    pub databases: usize,
    /// Schemas.
    pub schemas: usize,
    /// Tables.
    pub tables: usize,
    /// Views.
    pub views: usize,
    /// Materialized views.
    pub materialized_views: usize,
    /// Columns.
    pub columns: usize,
    /// Dependency edges.
    pub dependencies: usize,
    /// Graph nodes (includes edge-only stubs).
    pub graph_nodes: usize,
    /// Graph edges.
    pub graph_edges: usize,
}

/// A node in the interactive graph.
#[derive(Debug, Clone, Serialize)]
pub struct ReportNode {
    /// Object id.
    pub id: String,
    /// Kind label (`table`, `view`, …).
    pub kind: String,
    /// Simple name.
    pub name: String,
    /// Fully qualified name (or id for stubs).
    pub fqn: String,
    /// Optional description.
    pub description: Option<String>,
    /// Layout layer (0 = sources).
    pub layer: usize,
    /// Suggested x coordinate (layout units).
    pub x: f64,
    /// Suggested y coordinate (layout units).
    pub y: f64,
}

/// A directed edge in the interactive graph.
#[derive(Debug, Clone, Serialize)]
pub struct ReportEdge {
    /// Dependency id.
    pub id: String,
    /// Upstream id.
    pub from: String,
    /// Downstream id.
    pub to: String,
    /// Dependency kind label.
    pub kind: String,
    /// Granularity label.
    pub level: String,
}

/// Build structured report data from a snapshot (with layered layout).
#[must_use]
pub fn build_report_data(snapshot: &Snapshot) -> ReportData {
    let index = snapshot.object_index();
    let mut nodes_map: BTreeMap<String, ReportNode> = BTreeMap::new();

    for (id, obj) in &index {
        let meta = obj.meta();
        nodes_map.insert(
            id.as_str().to_string(),
            ReportNode {
                id: id.to_string(),
                kind: obj.kind_name().to_string(),
                name: meta.name.clone(),
                fqn: meta.fqn.to_dotted(),
                description: meta.description.clone(),
                layer: 0,
                x: 0.0,
                y: 0.0,
            },
        );
    }

    for dep in &snapshot.dependencies {
        for endpoint in [&dep.from_id, &dep.to_id] {
            let key = endpoint.as_str().to_string();
            nodes_map.entry(key.clone()).or_insert_with(|| ReportNode {
                id: key.clone(),
                kind: "unknown".into(),
                name: leaf_name(&key),
                fqn: key,
                description: None,
                layer: 0,
                x: 0.0,
                y: 0.0,
            });
        }
    }

    let edges: Vec<ReportEdge> = snapshot
        .dependencies
        .iter()
        .map(|d| ReportEdge {
            id: d.id.to_string(),
            from: d.from_id.to_string(),
            to: d.to_id.to_string(),
            kind: dep_kind_label(&d.kind).to_string(),
            level: match d.level {
                simplineage_core::model::graph::DependencyLevel::Relation => "relation".into(),
                simplineage_core::model::graph::DependencyLevel::Column => "column".into(),
                simplineage_core::model::graph::DependencyLevel::Unknown => "unknown".into(),
            },
        })
        .collect();

    assign_layers_and_positions(&mut nodes_map, &edges);

    let mut nodes: Vec<ReportNode> = nodes_map.into_values().collect();
    nodes.sort_by(|a, b| a.layer.cmp(&b.layer).then_with(|| a.fqn.cmp(&b.fqn)));

    ReportData {
        meta: ReportMeta {
            snapshot_id: snapshot.id.to_string(),
            label: snapshot.label.clone(),
            source: snapshot.source.clone(),
            model_version: snapshot.model_version.to_string(),
            generator: simplineage_core::PRODUCT_NAME.to_string(),
            generator_version: simplineage_core::VERSION.to_string(),
        },
        stats: ReportStats {
            catalogs: snapshot.catalogs.len(),
            databases: snapshot.databases.len(),
            schemas: snapshot.schemas.len(),
            tables: snapshot.tables.len(),
            views: snapshot.views.len(),
            materialized_views: snapshot.materialized_views.len(),
            columns: snapshot.columns.len(),
            dependencies: snapshot.dependencies.len(),
            graph_nodes: nodes.len(),
            graph_edges: edges.len(),
        },
        nodes,
        edges,
    }
}

/// Render a full offline interactive HTML report as a string.
pub fn render_html_report(snapshot: &Snapshot) -> Result<String> {
    let data = build_report_data(snapshot);
    let json = serde_json::to_string(&data)?;
    // Prevent `</script>` breakout in embedded JSON.
    let json_safe = json.replace('<', "\\u003c");

    let title = snapshot.label.as_deref().unwrap_or("Lineage report");
    let title_esc = html_escape(title);

    Ok(format!(
        r#"<!DOCTYPE html>
<html lang="en" data-theme="light">
<head>
  <meta charset="utf-8"/>
  <meta name="viewport" content="width=device-width, initial-scale=1"/>
  <meta name="color-scheme" content="light dark"/>
  <title>SimpLineage — {title_esc}</title>
  <style>
{css}
  </style>
</head>
<body>
  <div id="app">
    <header class="topbar">
      <div class="brand">
        <span class="logo">◇</span>
        <div>
          <div class="title">SimpLineage</div>
          <div class="subtitle" id="report-subtitle"></div>
        </div>
      </div>
      <div class="toolbar">
        <label class="search-wrap">
          <span class="sr-only">Search</span>
          <input id="search" type="search" placeholder="Search id, FQN, name…" autocomplete="off"/>
        </label>
        <button type="button" id="btn-fit" class="btn" title="Fit graph">Fit</button>
        <button type="button" id="btn-zoom-in" class="btn" title="Zoom in">+</button>
        <button type="button" id="btn-zoom-out" class="btn" title="Zoom out">−</button>
        <button type="button" id="btn-theme" class="btn" title="Toggle dark mode">Theme</button>
        <button type="button" id="btn-export-svg" class="btn" title="Download SVG">SVG</button>
        <button type="button" id="btn-export-mermaid" class="btn" title="Download Mermaid">Mermaid</button>
      </div>
    </header>
    <div class="main">
      <aside class="sidebar left" id="filters-panel">
        <h2>Filters</h2>
        <div class="filter-group" id="kind-filters"></div>
        <label class="check"><input type="checkbox" id="hide-isolated"/> Hide isolated</label>
        <label class="check"><input type="checkbox" id="relations-only" checked/> Relations only</label>
        <h2>Impact</h2>
        <p class="hint">Select a node, then run impact analysis.</p>
        <div class="btn-row">
          <button type="button" id="btn-up" class="btn" disabled>Upstream</button>
          <button type="button" id="btn-down" class="btn" disabled>Downstream</button>
          <button type="button" id="btn-both" class="btn" disabled>Both</button>
          <button type="button" id="btn-clear-impact" class="btn ghost" disabled>Clear</button>
        </div>
        <div id="impact-summary" class="impact-summary muted"></div>
        <h2>Stats</h2>
        <dl id="stats-list" class="stats"></dl>
      </aside>
      <section class="canvas-wrap" id="canvas-wrap">
        <svg id="graph" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Lineage graph">
          <defs>
            <marker id="arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
              <path d="M 0 0 L 10 5 L 0 10 z" class="arrow-head"/>
            </marker>
          </defs>
          <g id="viewport">
            <g id="edges"></g>
            <g id="nodes"></g>
          </g>
        </svg>
        <div class="canvas-hint muted">Scroll to zoom · drag background to pan · click a node for details</div>
      </section>
      <aside class="sidebar right" id="detail-panel">
        <h2>Details</h2>
        <div id="detail-empty" class="muted">Select a node to inspect metadata and run impact analysis.</div>
        <div id="detail-body" class="hidden">
          <div class="detail-kind" id="d-kind"></div>
          <h3 id="d-name"></h3>
          <p class="mono" id="d-fqn"></p>
          <p class="mono muted" id="d-id"></p>
          <p id="d-desc" class="desc"></p>
          <h3>Neighbors</h3>
          <div id="d-neighbors"></div>
        </div>
      </aside>
    </div>
    <footer class="footer muted">
      Offline report · no server · generated by SimpLineage
    </footer>
  </div>
  <script id="report-data" type="application/json">{json}</script>
  <script>
{js}
  </script>
</body>
</html>
"#,
        title_esc = title_esc,
        css = REPORT_CSS,
        json = json_safe,
        js = REPORT_JS,
    ))
}

/// Write the interactive offline HTML report to `path`.
pub fn write_html_report(snapshot: &Snapshot, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(Error::Io)?;
        }
    }
    let html = render_html_report(snapshot)?;
    fs::write(path, html).map_err(Error::Io)?;
    tracing::info!(?path, "wrote interactive HTML lineage report");
    Ok(())
}

fn assign_layers_and_positions(nodes: &mut BTreeMap<String, ReportNode>, edges: &[ReportEdge]) {
    // Adjacency for longest-path layering (sources = layer 0).
    let mut outs: HashMap<String, Vec<String>> = HashMap::new();
    let mut indeg: HashMap<String, usize> = HashMap::new();
    for id in nodes.keys() {
        indeg.entry(id.clone()).or_insert(0);
        outs.entry(id.clone()).or_default();
    }
    for e in edges {
        outs.entry(e.from.clone()).or_default().push(e.to.clone());
        *indeg.entry(e.to.clone()).or_insert(0) += 1;
        indeg.entry(e.from.clone()).or_insert(0);
    }

    let mut layer: HashMap<String, usize> = HashMap::new();
    let mut q: VecDeque<String> = VecDeque::new();
    for (id, &d) in &indeg {
        if d == 0 {
            q.push_back(id.clone());
            layer.insert(id.clone(), 0);
        }
    }
    // Kahn-like with longest path
    let mut remaining = indeg.clone();
    let mut seen = BTreeSet::new();
    while let Some(u) = q.pop_front() {
        if !seen.insert(u.clone()) {
            continue;
        }
        let lu = *layer.get(&u).unwrap_or(&0);
        for v in outs.get(&u).into_iter().flatten() {
            let next = lu + 1;
            let cur = layer.entry(v.clone()).or_insert(0);
            if next > *cur {
                *cur = next;
            }
            if let Some(r) = remaining.get_mut(v) {
                *r = r.saturating_sub(1);
                if *r == 0 {
                    q.push_back(v.clone());
                }
            }
        }
    }
    // Unvisited (cycles / leftovers)
    for id in nodes.keys() {
        layer.entry(id.clone()).or_insert(0);
    }

    // Bucket by layer for y packing
    let mut by_layer: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for (id, l) in &layer {
        by_layer.entry(*l).or_default().push(id.clone());
    }
    for ids in by_layer.values_mut() {
        ids.sort();
    }

    const X_GAP: f64 = 220.0;
    const Y_GAP: f64 = 72.0;
    const X0: f64 = 40.0;
    const Y0: f64 = 40.0;

    for (l, ids) in &by_layer {
        for (i, id) in ids.iter().enumerate() {
            if let Some(n) = nodes.get_mut(id) {
                n.layer = *l;
                n.x = X0 + (*l as f64) * X_GAP;
                n.y = Y0 + (i as f64) * Y_GAP;
            }
        }
    }
}

fn leaf_name(s: &str) -> String {
    s.rsplit(['.', '/', ':']).next().unwrap_or(s).to_string()
}

fn dep_kind_label(kind: &DependencyKind) -> &str {
    match kind {
        DependencyKind::ViewDefinition => "view_definition",
        DependencyKind::Pipeline => "pipeline",
        DependencyKind::ForeignKey => "foreign_key",
        DependencyKind::Manual => "manual",
        DependencyKind::Inferred => "inferred",
        DependencyKind::Other(s) => s.as_str(),
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use simplineage_core::ObjectId;
    use simplineage_core::model::graph::{Dependency, DependencyKind, DependencyLevel};
    use simplineage_core::model::ids::FullyQualifiedName;
    use simplineage_core::model::objects::{ObjectMeta, Table, View};

    fn sample() -> Snapshot {
        let mut s = Snapshot::new();
        s.label = Some("html-test".into());
        s.tables.push(Table {
            meta: ObjectMeta::new(
                ObjectId::from_trusted("table:orders"),
                FullyQualifiedName::parse_dotted("raw.orders").unwrap(),
            ),
            schema_id: None,
            column_ids: vec![],
        });
        s.views.push(View {
            meta: ObjectMeta::new(
                ObjectId::from_trusted("view:stg"),
                FullyQualifiedName::parse_dotted("staging.stg_orders").unwrap(),
            ),
            schema_id: None,
            column_ids: vec![],
            definition: None,
        });
        s.dependencies.push(Dependency {
            id: ObjectId::from_trusted("d1"),
            from_id: ObjectId::from_trusted("table:orders"),
            to_id: ObjectId::from_trusted("view:stg"),
            kind: DependencyKind::ViewDefinition,
            level: DependencyLevel::Relation,
            confidence: None,
            attributes: Default::default(),
        });
        s
    }

    #[test]
    fn builds_layers() {
        let data = build_report_data(&sample());
        assert_eq!(data.nodes.len(), 2);
        assert_eq!(data.edges.len(), 1);
        let orders = data.nodes.iter().find(|n| n.id == "table:orders").unwrap();
        let stg = data.nodes.iter().find(|n| n.id == "view:stg").unwrap();
        assert!(stg.layer >= orders.layer);
        assert!(stg.x >= orders.x);
    }

    #[test]
    fn render_contains_features() {
        let html = render_html_report(&sample()).unwrap();
        assert!(html.contains("btn-export-svg"));
        assert!(html.contains("btn-export-mermaid"));
        assert!(html.contains("btn-theme"));
        assert!(html.contains("report-data"));
        assert!(html.contains("table:orders"));
        assert!(!html.contains("</script><script>")); // sanity
        assert!(html.contains("data-theme"));
    }
}
