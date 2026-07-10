//! `simplineage stats` — store and graph statistics.

use serde::Serialize;

use crate::context::AppContext;
use crate::output::{self};

#[derive(Debug)]
pub struct StatsArgs {
    pub snapshot: Option<String>,
    pub store_only: bool,
}

#[derive(Debug, Serialize)]
struct StatsOut {
    data_dir: String,
    schema_version: i32,
    snapshots: Vec<SnapRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current: Option<CurrentStats>,
}

#[derive(Debug, Serialize)]
struct SnapRow {
    id: String,
    label: Option<String>,
    is_current: bool,
    object_count: i64,
    edge_count: i64,
    source: Option<String>,
}

#[derive(Debug, Serialize)]
struct CurrentStats {
    snapshot_id: String,
    objects: usize,
    tables: usize,
    views: usize,
    materialized_views: usize,
    columns: usize,
    dependencies: usize,
    graph: simplineage_analysis::GraphStatistics,
}

pub fn run_stats(ctx: &AppContext, args: StatsArgs) -> anyhow::Result<()> {
    let style = ctx.style;
    let store = ctx.open_store()?;
    let metas = store.list_snapshots()?;

    let snapshots: Vec<SnapRow> = metas
        .iter()
        .map(|m| SnapRow {
            id: m.id.to_string(),
            label: m.label.clone(),
            is_current: m.is_current,
            object_count: m.object_count,
            edge_count: m.edge_count,
            source: m.source.clone(),
        })
        .collect();

    let current = if args.store_only {
        None
    } else {
        match ctx.load_snapshot(args.snapshot.as_deref()) {
            Ok(snap) => {
                let engine = simplineage_analysis::AnalysisEngine::from_snapshot(snap.clone());
                Some(CurrentStats {
                    snapshot_id: snap.id.to_string(),
                    objects: snap.object_count(),
                    tables: snap.tables.len(),
                    views: snap.views.len(),
                    materialized_views: snap.materialized_views.len(),
                    columns: snap.columns.len(),
                    dependencies: snap.dependencies.len(),
                    graph: engine.graph().statistics(),
                })
            }
            Err(_) if args.snapshot.is_none() && snapshots.is_empty() => None,
            Err(e) => return Err(e),
        }
    };

    let out = StatsOut {
        data_dir: ctx.data_dir.display().to_string(),
        schema_version: store.schema_version(),
        snapshots,
        current,
    };

    if style.json {
        output::print_json(&out)?;
        return Ok(());
    }

    output::header(style, "Store");
    output::kv(style, "data_dir", &out.data_dir);
    output::kv(style, "schema_version", out.schema_version);
    output::kv(style, "snapshots", out.snapshots.len());

    if !out.snapshots.is_empty() {
        let rows: Vec<Vec<String>> = out
            .snapshots
            .iter()
            .map(|s| {
                vec![
                    if s.is_current { "*".into() } else { " ".into() },
                    s.id.chars().take(8).collect::<String>(),
                    s.label.clone().unwrap_or_default(),
                    s.object_count.to_string(),
                    s.edge_count.to_string(),
                ]
            })
            .collect();
        output::print_table(style, &["", "ID", "LABEL", "OBJS", "EDGES"], &rows);
    }

    if let Some(c) = &out.current {
        output::header(style, "Current snapshot");
        output::kv(style, "id", &c.snapshot_id);
        output::kv(style, "objects", c.objects);
        output::kv(
            style,
            "breakdown",
            format!(
                "tables={} views={} mvs={} columns={} deps={}",
                c.tables, c.views, c.materialized_views, c.columns, c.dependencies
            ),
        );
        output::header(style, "Graph");
        output::kv(style, "nodes", c.graph.node_count);
        output::kv(style, "edges", c.graph.edge_count);
        output::kv(style, "density", format!("{:.6}", c.graph.density));
        output::kv(style, "isolated", c.graph.isolated_nodes);
        output::kv(style, "sources", c.graph.source_nodes);
        output::kv(style, "sinks", c.graph.sink_nodes);
        output::kv(style, "has_cycle", c.graph.has_cycle);
        output::kv(style, "wcc", c.graph.weakly_connected_components);
    }

    Ok(())
}
