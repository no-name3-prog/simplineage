//! `simplineage build` — construct the lineage graph and optional full analysis.

use std::path::PathBuf;

use anyhow::Context;
use serde::Serialize;
use simplineage_analysis::AnalysisEngine;

use crate::context::AppContext;
use crate::output::{self};
use crate::progress;

#[derive(Debug)]
pub struct BuildArgs {
    pub snapshot: Option<String>,
    pub full: bool,
    pub output: Option<PathBuf>,
    pub no_progress: bool,
}

#[derive(Debug, Serialize)]
struct BuildReport {
    snapshot_id: String,
    objects: usize,
    dependencies: usize,
    statistics: simplineage_analysis::GraphStatistics,
    #[serde(skip_serializing_if = "Option::is_none")]
    analysis: Option<simplineage_analysis::FullAnalysisReport>,
}

pub fn run_build(ctx: &AppContext, args: BuildArgs) -> anyhow::Result<()> {
    let style = ctx.style;

    let spinner = if args.no_progress || style.quiet || style.json {
        None
    } else {
        Some(progress::spinner("Building lineage graph…"))
    };

    let snapshot = ctx.load_snapshot(args.snapshot.as_deref())?;
    let engine = AnalysisEngine::from_snapshot(snapshot.clone());
    let statistics = engine.graph().statistics();

    let analysis = if args.full {
        if let Some(pb) = &spinner {
            pb.set_message("Running full analysis suite…");
        }
        Some(engine.analyze_all())
    } else {
        None
    };

    if let Some(pb) = spinner {
        progress::finish_ok(
            pb,
            format!(
                "Graph ready: {} nodes, {} edges",
                statistics.node_count, statistics.edge_count
            ),
            style.quiet,
        );
    }

    let report = BuildReport {
        snapshot_id: snapshot.id.to_string(),
        objects: snapshot.object_count(),
        dependencies: snapshot.dependencies.len(),
        statistics: statistics.clone(),
        analysis,
    };

    if let Some(path) = &args.output {
        simplineage_exporters::write_json_value(&report, path, true)
            .with_context(|| format!("write {}", path.display()))?;
        output::success(style, &format!("Wrote {}", path.display()));
    }

    if style.json {
        output::print_json(&report)?;
        return Ok(());
    }

    output::header(style, "Build complete");
    output::kv(style, "snapshot", &report.snapshot_id);
    output::kv(style, "objects", report.objects);
    output::kv(style, "dependencies", report.dependencies);
    output::header(style, "Graph statistics");
    print_stats(style, &report.statistics);

    if let Some(full) = &report.analysis {
        output::header(style, "Analysis highlights");
        output::kv(style, "validation_ok", full.dependency_validation.ok);
        output::kv(style, "orphans", full.orphans.len());
        output::kv(style, "unused", full.unused.len());
        output::kv(style, "cycles", full.circular.len());
        output::kv(style, "quality_score", format!("{:.1}", full.quality.score));
        if !full.critical.is_empty() {
            output::muted(style, "Top critical objects:");
            for c in full.critical.iter().take(5) {
                output::bullet(
                    style,
                    &format!(
                        "{}  downstream={} score={:.1}",
                        c.id, c.downstream_count, c.score
                    ),
                );
            }
        }
    }

    Ok(())
}

fn print_stats(style: crate::output::OutputStyle, s: &simplineage_analysis::GraphStatistics) {
    output::kv(style, "nodes", s.node_count);
    output::kv(style, "edges", s.edge_count);
    output::kv(style, "density", format!("{:.6}", s.density));
    output::kv(style, "avg_in_degree", format!("{:.2}", s.avg_in_degree));
    output::kv(style, "avg_out_degree", format!("{:.2}", s.avg_out_degree));
    output::kv(style, "max_in_degree", s.max_in_degree);
    output::kv(style, "max_out_degree", s.max_out_degree);
    output::kv(style, "isolated", s.isolated_nodes);
    output::kv(style, "sources", s.source_nodes);
    output::kv(style, "sinks", s.sink_nodes);
    output::kv(style, "wcc", s.weakly_connected_components);
    output::kv(style, "scc", s.strongly_connected_components);
    output::kv(style, "cyclic_components", s.cyclic_components);
    output::kv(style, "has_cycle", s.has_cycle);
}
