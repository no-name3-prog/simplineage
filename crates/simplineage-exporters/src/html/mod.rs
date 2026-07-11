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
    /// Parent relation id for columns (enables table → column panel UX).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// Short data-type label for columns when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_type: Option<String>,
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
    build_report_data_inner(snapshot, true)
}

/// Nodes + edges only (positions zeroed). Prefer for Mermaid/text exports.
#[must_use]
pub fn build_export_graph(snapshot: &Snapshot) -> ReportData {
    build_report_data_inner(snapshot, false)
}

fn build_report_data_inner(snapshot: &Snapshot, layout: bool) -> ReportData {
    let index = snapshot.object_index();
    let mut nodes_map: BTreeMap<String, ReportNode> = BTreeMap::new();

    // Column extras keyed by object id.
    let mut column_meta: BTreeMap<String, (String, Option<String>)> = BTreeMap::new();
    for c in &snapshot.columns {
        let dtype = c
            .raw_type
            .clone()
            .or_else(|| Some(data_type_label(&c.data_type)));
        column_meta.insert(c.meta.id.to_string(), (c.parent_id.to_string(), dtype));
    }

    for (id, obj) in &index {
        let meta = obj.meta();
        let key = id.as_str().to_string();
        let (parent_id, data_type) = column_meta
            .get(&key)
            .map(|(p, d)| (Some(p.clone()), d.clone()))
            .unwrap_or((None, None));
        nodes_map.insert(
            key,
            ReportNode {
                id: id.to_string(),
                kind: obj.kind_name().to_string(),
                name: meta.name.clone(),
                fqn: meta.fqn.to_dotted(),
                description: meta.description.clone(),
                parent_id,
                data_type,
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
                parent_id: None,
                data_type: None,
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
            kind: dep_kind_label(&d.kind),
            level: d.level.as_str().into(),
        })
        .collect();

    if layout {
        assign_layers_and_positions(&mut nodes_map, &edges);
    }

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
        <p class="hint">Uncheck <em>Relations only</em> to plot column nodes. Selecting a column (or a column in Details) still focuses field-level lineage while relations-only is on.</p>
        <h2>Impact</h2>
        <p class="hint">Click a node to auto-highlight its upstream and downstream (others dim). Search does the same for matches — column hits reveal column paths. Narrow with Upstream / Downstream / Both.</p>
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
        <div class="canvas-hint muted">Scroll to zoom · drag to pan · click a node (table or column) to focus lineage · click empty canvas to clear</div>
      </section>
      <aside class="sidebar right" id="detail-panel">
        <h2>Details</h2>
        <div id="detail-empty" class="muted">Select a node to inspect metadata; its upstream and downstream are highlighted automatically.</div>
        <div id="detail-body" class="hidden">
          <div class="detail-kind" id="d-kind"></div>
          <h3 id="d-name"></h3>
          <p class="mono" id="d-fqn"></p>
          <p class="mono muted" id="d-id"></p>
          <p id="d-desc" class="desc"></p>
          <p id="d-dtype" class="mono muted hidden"></p>
          <p id="d-parent" class="mono muted hidden"></p>
          <h3 id="d-columns-heading" class="hidden">Columns</h3>
          <div id="d-columns" class="hidden"></div>
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

fn is_relation_kind(kind: &str) -> bool {
    matches!(kind, "table" | "view" | "materialized_view" | "unknown")
}

fn assign_layers_and_positions(nodes: &mut BTreeMap<String, ReportNode>, edges: &[ReportEdge]) {
    // ── Primary layout: relations only (table / view / MV) ───────────────
    //
    // Mixing hundreds of column nodes into the same layered pack used to make
    // tables sit thousands of pixels apart (columns took vertical slots). When
    // the HTML "Relations only" filter hides columns, those gaps remain — the
    // graph looks sparse, misaligned, and "lower nodes very low".
    //
    // Fix: layer + pack **relation-level** structure first; nest columns next
    // to their parent table via `parent_id`.

    const X_GAP: f64 = 220.0;
    const Y_GAP: f64 = 72.0;
    const X0: f64 = 40.0;
    const Y0: f64 = 40.0;
    const NODE_H: f64 = 44.0;
    const COL_X_OFFSET: f64 = 18.0;
    const COL_Y_GAP: f64 = 40.0;
    const COL_STACK_MAX: usize = 12; // wrap extra columns into a second stack

    // Relation-level edges (and any edge whose endpoints are both relations).
    let primary_edges: Vec<&ReportEdge> = edges
        .iter()
        .filter(|e| {
            if e.level == "column" {
                return false;
            }
            let fk = nodes
                .get(&e.from)
                .map(|n| n.kind.as_str())
                .unwrap_or("unknown");
            let tk = nodes
                .get(&e.to)
                .map(|n| n.kind.as_str())
                .unwrap_or("unknown");
            is_relation_kind(fk) && is_relation_kind(tk)
        })
        .collect();

    let mut primary: BTreeSet<String> = BTreeSet::new();
    for e in &primary_edges {
        primary.insert(e.from.clone());
        primary.insert(e.to.clone());
    }
    // Include relation nodes even if only column edges touch them (still place
    // them as free roots in layer 0 so parents exist for column nesting).
    for (id, n) in nodes.iter() {
        if is_relation_kind(&n.kind) {
            // Prefer nodes that appear on any edge, else leave for isolated grid.
            let on_edge = edges.iter().any(|e| e.from == *id || e.to == *id);
            if on_edge {
                primary.insert(id.clone());
            }
        }
    }

    let mut outs: HashMap<String, Vec<String>> = HashMap::new();
    let mut ins: HashMap<String, Vec<String>> = HashMap::new();
    let mut indeg: HashMap<String, usize> = HashMap::new();
    for id in &primary {
        indeg.entry(id.clone()).or_insert(0);
        outs.entry(id.clone()).or_default();
        ins.entry(id.clone()).or_default();
    }
    for e in &primary_edges {
        if !primary.contains(&e.from) || !primary.contains(&e.to) {
            continue;
        }
        outs.entry(e.from.clone()).or_default().push(e.to.clone());
        ins.entry(e.to.clone()).or_default().push(e.from.clone());
        *indeg.entry(e.to.clone()).or_insert(0) += 1;
        indeg.entry(e.from.clone()).or_insert(0);
    }

    let mut layer: HashMap<String, usize> = HashMap::new();
    let mut q: VecDeque<String> = VecDeque::new();
    for id in &primary {
        if *indeg.get(id).unwrap_or(&0) == 0 {
            q.push_back(id.clone());
            layer.insert(id.clone(), 0);
        }
    }
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
    for id in &primary {
        layer.entry(id.clone()).or_insert(0);
    }

    let mut by_layer: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for (id, l) in &layer {
        by_layer.entry(*l).or_default().push(id.clone());
    }
    for ids in by_layer.values_mut() {
        ids.sort();
    }

    // Barycenter ordering on the primary (relation) graph only.
    const ORDER_PASSES: usize = 6;
    for _ in 0..ORDER_PASSES {
        let layers: Vec<usize> = by_layer.keys().copied().collect();
        for l in layers.iter().copied().skip(1) {
            let ranks: HashMap<String, usize> = match by_layer.get(&(l.saturating_sub(1))) {
                Some(p) if !p.is_empty() => p
                    .iter()
                    .enumerate()
                    .map(|(i, id)| (id.clone(), i))
                    .collect(),
                _ => continue,
            };
            if let Some(ids) = by_layer.get_mut(&l) {
                ids.sort_by(|a, b| {
                    let ba = barycenter(a, &ins, &ranks);
                    let bb = barycenter(b, &ins, &ranks);
                    ba.partial_cmp(&bb)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.cmp(b))
                });
            }
        }
        for l in layers.into_iter().rev().skip(1) {
            let ranks: HashMap<String, usize> = match by_layer.get(&(l + 1)) {
                Some(n) if !n.is_empty() => n
                    .iter()
                    .enumerate()
                    .map(|(i, id)| (id.clone(), i))
                    .collect(),
                _ => continue,
            };
            if let Some(ids) = by_layer.get_mut(&l) {
                ids.sort_by(|a, b| {
                    let ba = barycenter(a, &outs, &ranks);
                    let bb = barycenter(b, &outs, &ranks);
                    ba.partial_cmp(&bb)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.cmp(b))
                });
            }
        }
    }

    // Compact pack: each layer is a vertical strip. Shorter layers are
    // vertically centered against the tallest layer so cross-layer edges
    // don't all dive toward the top-left.
    let mut max_y = Y0;
    let mut placed: BTreeSet<String> = BTreeSet::new();
    let max_layer_len = by_layer.values().map(Vec::len).max().unwrap_or(1);
    let max_band = (max_layer_len.saturating_sub(1) as f64) * Y_GAP;
    for (l, ids) in &by_layer {
        let band = (ids.len().saturating_sub(1) as f64) * Y_GAP;
        let y_center = (max_band - band) / 2.0;
        for (i, id) in ids.iter().enumerate() {
            if let Some(n) = nodes.get_mut(id) {
                n.layer = *l;
                n.x = X0 + (*l as f64) * X_GAP;
                n.y = Y0 + y_center + (i as f64) * Y_GAP;
                max_y = max_y.max(n.y + NODE_H);
                placed.insert(id.clone());
            }
        }
    }

    // ── Columns: nest under parent relation ──────────────────────────────
    let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut orphan_columns: Vec<String> = Vec::new();
    for (id, n) in nodes.iter() {
        if n.kind != "column" {
            continue;
        }
        if let Some(p) = n.parent_id.as_ref() {
            if nodes.contains_key(p) {
                children.entry(p.clone()).or_default().push(id.clone());
                continue;
            }
        }
        orphan_columns.push(id.clone());
    }
    for kids in children.values_mut() {
        kids.sort_by(|a, b| {
            let na = nodes.get(a).map(|n| n.name.as_str()).unwrap_or("");
            let nb = nodes.get(b).map(|n| n.name.as_str()).unwrap_or("");
            na.cmp(nb).then_with(|| a.cmp(b))
        });
    }

    for (parent_id, kids) in &children {
        let (px, py, player) = match nodes.get(parent_id) {
            Some(p) if placed.contains(parent_id) => (p.x, p.y, p.layer),
            _ => continue,
        };
        for (i, kid) in kids.iter().enumerate() {
            if let Some(n) = nodes.get_mut(kid) {
                let stack = i / COL_STACK_MAX;
                let row = i % COL_STACK_MAX;
                // Sit just under the parent, slight right offset per stack wrap.
                n.layer = player;
                n.x = px + COL_X_OFFSET + (stack as f64) * 150.0;
                n.y = py + NODE_H + 8.0 + (row as f64) * COL_Y_GAP;
                max_y = max_y.max(n.y + 36.0);
                placed.insert(kid.clone());
            }
        }
    }

    // ── Remaining (schemas, catalogs, orphan columns, free relations) ────
    let mut isolated: Vec<String> = nodes
        .keys()
        .filter(|id| !placed.contains(*id))
        .cloned()
        .collect();
    isolated.sort();
    // Prefer putting orphan columns first in the isolated grid for predictability.
    isolated.sort_by(|a, b| {
        let ka = nodes.get(a).map(|n| n.kind.as_str()).unwrap_or("");
        let kb = nodes.get(b).map(|n| n.kind.as_str()).unwrap_or("");
        let rank = |k: &str| if k == "column" { 0 } else { 1 };
        rank(ka).cmp(&rank(kb)).then_with(|| a.cmp(b))
    });
    let _ = orphan_columns;

    const ISO_COLS: usize = 4;
    const ISO_X_GAP: f64 = 180.0;
    const ISO_Y_GAP: f64 = 56.0;
    let iso_y0 = max_y + 100.0;
    let max_layer = by_layer.keys().next_back().copied().unwrap_or(0);
    let iso_x0 = X0 + (max_layer as f64 + 1.5) * X_GAP;

    for (i, id) in isolated.iter().enumerate() {
        if let Some(n) = nodes.get_mut(id) {
            let col = i % ISO_COLS;
            let row = i / ISO_COLS;
            n.layer = 0;
            n.x = iso_x0 + (col as f64) * ISO_X_GAP;
            n.y = iso_y0 + (row as f64) * ISO_Y_GAP;
            placed.insert(id.clone());
        }
    }

    // Graphs with no primary edges: compact grid of relation nodes, columns nested.
    if primary_edges.is_empty() && by_layer.is_empty() {
        let mut rels: Vec<String> = nodes
            .iter()
            .filter(|(_, n)| is_relation_kind(&n.kind))
            .map(|(id, _)| id.clone())
            .collect();
        rels.sort();
        for (i, id) in rels.iter().enumerate() {
            if let Some(n) = nodes.get_mut(id) {
                let col = i % 6;
                let row = i / 6;
                n.layer = col;
                n.x = X0 + (col as f64) * X_GAP;
                n.y = Y0 + (row as f64) * Y_GAP;
            }
        }
    }
}

fn barycenter(
    id: &str,
    neighbors: &HashMap<String, Vec<String>>,
    index: &HashMap<String, usize>,
) -> f64 {
    let Some(ns) = neighbors.get(id) else {
        return f64::MAX / 4.0;
    };
    let mut sum = 0.0;
    let mut count = 0.0;
    for n in ns {
        if let Some(&i) = index.get(n) {
            sum += i as f64;
            count += 1.0;
        }
    }
    if count == 0.0 {
        // Keep stable relative order when no neighbor in the adjacent layer.
        f64::MAX / 4.0
    } else {
        sum / count
    }
}

fn leaf_name(s: &str) -> String {
    s.rsplit(['.', '/', ':']).next().unwrap_or(s).to_string()
}

fn data_type_label(dt: &simplineage_core::model::types::DataType) -> String {
    use simplineage_core::model::types::DataType;
    match dt {
        DataType::Boolean => "boolean".into(),
        DataType::Integer { bits } => bits
            .map(|b| format!("integer({b})"))
            .unwrap_or_else(|| "integer".into()),
        DataType::Decimal { precision, scale } => match (precision, scale) {
            (Some(p), Some(s)) => format!("decimal({p},{s})"),
            (Some(p), None) => format!("decimal({p})"),
            _ => "decimal".into(),
        },
        DataType::Float { bits } => bits
            .map(|b| format!("float({b})"))
            .unwrap_or_else(|| "float".into()),
        DataType::String { max_length, .. } => max_length
            .map(|n| format!("string({n})"))
            .unwrap_or_else(|| "string".into()),
        DataType::Binary { max_length } => max_length
            .map(|n| format!("binary({n})"))
            .unwrap_or_else(|| "binary".into()),
        DataType::Date => "date".into(),
        DataType::Time { .. } => "time".into(),
        DataType::Timestamp { .. } => "timestamp".into(),
        DataType::Json => "json".into(),
        DataType::Array { .. } => "array".into(),
        DataType::Map { .. } => "map".into(),
        DataType::Struct { .. } => "struct".into(),
        DataType::Spatial => "spatial".into(),
        DataType::Uuid => "uuid".into(),
        DataType::Other { name } => name.clone(),
        DataType::Unknown => "unknown".into(),
    }
}

fn dep_kind_label(kind: &DependencyKind) -> String {
    match kind {
        // Keep short label for Other (not other: prefix) in the HTML payload.
        DependencyKind::Other(s) => s.clone(),
        other => other.as_str().into_owned(),
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
    use simplineage_core::model::objects::{Column, ObjectMeta, Table, View};
    use simplineage_core::model::types::DataType;

    fn sample() -> Snapshot {
        let mut s = Snapshot::new();
        s.label = Some("html-test".into());
        s.tables.push(Table {
            meta: ObjectMeta::new(
                ObjectId::from_trusted("table:orders"),
                FullyQualifiedName::parse_dotted("raw.orders").unwrap(),
            ),
            schema_id: None,
            column_ids: vec![ObjectId::from_trusted("column:orders.email")],
        });
        s.views.push(View {
            meta: ObjectMeta::new(
                ObjectId::from_trusted("view:stg"),
                FullyQualifiedName::parse_dotted("staging.stg_orders").unwrap(),
            ),
            schema_id: None,
            column_ids: vec![ObjectId::from_trusted("column:stg.email")],
            definition: None,
        });
        s.columns.push(Column {
            meta: ObjectMeta::new(
                ObjectId::from_trusted("column:orders.email"),
                FullyQualifiedName::parse_dotted("raw.orders.email").unwrap(),
            ),
            parent_id: ObjectId::from_trusted("table:orders"),
            ordinal: Some(0),
            data_type: DataType::String {
                max_length: None,
                is_char_length: None,
            },
            nullable: true,
            is_primary_key: None,
            raw_type: Some("VARCHAR".into()),
        });
        s.columns.push(Column {
            meta: ObjectMeta::new(
                ObjectId::from_trusted("column:stg.email"),
                FullyQualifiedName::parse_dotted("staging.stg_orders.email").unwrap(),
            ),
            parent_id: ObjectId::from_trusted("view:stg"),
            ordinal: Some(0),
            data_type: DataType::String {
                max_length: None,
                is_char_length: None,
            },
            nullable: true,
            is_primary_key: None,
            raw_type: None,
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
        s.dependencies.push(Dependency {
            id: ObjectId::from_trusted("d-col"),
            from_id: ObjectId::from_trusted("column:orders.email"),
            to_id: ObjectId::from_trusted("column:stg.email"),
            kind: DependencyKind::ViewDefinition,
            level: DependencyLevel::Column,
            confidence: None,
            attributes: Default::default(),
        });
        s
    }

    #[test]
    fn builds_layers() {
        let data = build_report_data(&sample());
        assert_eq!(data.nodes.len(), 4);
        assert_eq!(data.edges.len(), 2);
        let orders = data.nodes.iter().find(|n| n.id == "table:orders").unwrap();
        let stg = data.nodes.iter().find(|n| n.id == "view:stg").unwrap();
        assert!(stg.layer >= orders.layer);
        assert!(stg.x >= orders.x);
        let col = data
            .nodes
            .iter()
            .find(|n| n.id == "column:orders.email")
            .unwrap();
        assert_eq!(col.kind, "column");
        assert_eq!(col.parent_id.as_deref(), Some("table:orders"));
        assert_eq!(col.data_type.as_deref(), Some("VARCHAR"));
    }

    #[test]
    fn connected_nodes_not_pushed_by_isolated_catalog() {
        let mut s = sample();
        // Many isolated columns used to pack into layer 0 and push sources down.
        for i in 0..40 {
            let fqn = format!("raw.orders.c{i}");
            s.columns.push(Column {
                meta: ObjectMeta::new(
                    ObjectId::from_trusted(format!("column:c{i}")),
                    FullyQualifiedName::parse_dotted(&fqn).unwrap(),
                ),
                parent_id: ObjectId::from_trusted("table:orders"),
                ordinal: Some(i as u32),
                data_type: DataType::String {
                    max_length: None,
                    is_char_length: None,
                },
                nullable: true,
                is_primary_key: None,
                raw_type: None,
            });
        }
        let data = build_report_data(&s);
        let orders = data.nodes.iter().find(|n| n.id == "table:orders").unwrap();
        let stg = data.nodes.iter().find(|n| n.id == "view:stg").unwrap();
        // Source and next layer should sit on a compact vertical band (relations only).
        assert!(orders.y < 200.0, "orders.y={}", orders.y);
        assert!(stg.y < 200.0, "stg.y={}", stg.y);
        assert!((orders.y - stg.y).abs() < 120.0);
        // Columns nest under their parent — not interleaved into the relation pack.
        let col = data.nodes.iter().find(|n| n.id == "column:c0").unwrap();
        assert!(
            (col.x - orders.x).abs() < 200.0 && col.y >= orders.y,
            "column should nest near parent orders: col=({}, {}) orders=({}, {})",
            col.x,
            col.y,
            orders.x,
            orders.y
        );
        // Relation nodes stay compact even with many child columns in the payload.
        let rel_ys: Vec<f64> = data
            .nodes
            .iter()
            .filter(|n| matches!(n.kind.as_str(), "table" | "view"))
            .map(|n| n.y)
            .collect();
        let span = rel_ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            - rel_ys.iter().cloned().fold(f64::INFINITY, f64::min);
        assert!(
            span < 200.0,
            "relation y-span should stay compact, got {span}"
        );
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
        // Focus / dim behaviour is embedded in the offline JS payload.
        assert!(html.contains("computeFocus"));
        assert!(html.contains("lineageSets"));
        assert!(html.contains("auto-highlight"));
        // Column-level UX
        assert!(html.contains("d-columns"));
        assert!(html.contains("childrenByParent"));
        assert!(html.contains("parent_id"));
        assert!(html.contains("column:orders.email"));
    }
}
