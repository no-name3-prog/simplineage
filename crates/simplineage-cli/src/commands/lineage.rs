//! Upstream, downstream, and impact analysis commands.

use serde::Serialize;
use simplineage_analysis::{ImpactDirection, ImpactOptions};
use simplineage_core::ObjectId;

use crate::context::{self, AppContext};
use crate::output::{self};
use crate::progress;

#[derive(Debug)]
pub struct LineageArgs {
    pub object: String,
    pub snapshot: Option<String>,
    pub max_depth: Option<usize>,
    pub relations_only: bool,
    /// Lineage level filter: `all`, `relation`, or `column`.
    pub level: String,
    /// Optional column name when `object` is a parent relation.
    pub column: Option<String>,
    pub no_progress: bool,
}

#[derive(Debug)]
pub struct ImpactArgs {
    pub object: String,
    pub snapshot: Option<String>,
    pub direction: String,
    pub max_depth: Option<usize>,
    pub relations_only: bool,
    /// Lineage level filter: `all`, `relation`, or `column`.
    pub level: String,
    /// Optional column name when `object` is a parent relation.
    pub column: Option<String>,
    pub no_progress: bool,
}

/// Enriched node row (JSON detail + text formatting).
#[derive(Debug, Serialize)]
struct ImpactNodeOut {
    id: String,
    fqn: String,
    kind: String,
}

#[derive(Debug, Serialize)]
struct ImpactOut {
    /// Subject object id (stable for scripting).
    subject: String,
    /// Human-readable FQN when known.
    subject_fqn: String,
    /// Catalog kind (`table`, `column`, …).
    subject_kind: String,
    direction: String,
    /// Active level filter: `all` | `relation` | `column`.
    level: String,
    total_affected: usize,
    /// Upstream object ids (backward-compatible).
    upstream: Vec<String>,
    /// Downstream object ids (backward-compatible).
    downstream: Vec<String>,
    /// Enriched upstream rows.
    upstream_nodes: Vec<ImpactNodeOut>,
    /// Enriched downstream rows.
    downstream_nodes: Vec<ImpactNodeOut>,
}

pub fn run_upstream(ctx: &AppContext, args: LineageArgs) -> anyhow::Result<()> {
    run_direction(ctx, args, ImpactDirection::Upstream, "Upstream")
}

pub fn run_downstream(ctx: &AppContext, args: LineageArgs) -> anyhow::Result<()> {
    run_direction(ctx, args, ImpactDirection::Downstream, "Downstream")
}

fn run_direction(
    ctx: &AppContext,
    args: LineageArgs,
    direction: ImpactDirection,
    title: &str,
) -> anyhow::Result<()> {
    let style = ctx.style;
    let spinner = if args.no_progress || style.quiet || style.json {
        None
    } else {
        Some(progress::spinner(format!("Traversing {title}…")))
    };

    let engine = ctx.analysis_engine(args.snapshot.as_deref())?;
    let id = context::resolve_object_ex(engine.snapshot(), &args.object, args.column.as_deref())?;
    let level = resolve_level(&args.level, args.relations_only)?;
    let report = engine.impact(
        &id,
        &ImpactOptions {
            direction,
            max_depth: args.max_depth,
            relations_only: level.relations_only,
            columns_only: level.columns_only,
        },
    )?;

    if let Some(pb) = spinner {
        progress::finish_clear(pb);
    }

    let out = build_impact_out(engine.snapshot(), &report, direction, &level);

    if style.json {
        output::print_json(&out)?;
        return Ok(());
    }

    let side = match direction {
        ImpactDirection::Upstream => &out.upstream_nodes,
        ImpactDirection::Downstream => &out.downstream_nodes,
        ImpactDirection::Both => &out.downstream_nodes,
    };

    output::header(
        style,
        &format!("{title} of {} ({})", out.subject_fqn, out.subject_kind),
    );
    output::kv(style, "level", &out.level);
    output::kv(style, "count", side.len());
    if let Some(d) = args.max_depth {
        output::kv(style, "max_depth", d);
    }
    if side.is_empty() {
        output::muted(style, "No related objects in this direction.");
    } else {
        for n in side {
            output::bullet(style, &format!("[{}] {}", n.kind, n.fqn));
        }
    }
    Ok(())
}

pub fn run_impact(ctx: &AppContext, args: ImpactArgs) -> anyhow::Result<()> {
    let style = ctx.style;
    let direction = parse_direction(&args.direction)?;

    let spinner = if args.no_progress || style.quiet || style.json {
        None
    } else {
        Some(progress::spinner("Computing impact…"))
    };

    let engine = ctx.analysis_engine(args.snapshot.as_deref())?;
    let id = context::resolve_object_ex(engine.snapshot(), &args.object, args.column.as_deref())?;
    let level = resolve_level(&args.level, args.relations_only)?;
    let report = engine.impact(
        &id,
        &ImpactOptions {
            direction,
            max_depth: args.max_depth,
            relations_only: level.relations_only,
            columns_only: level.columns_only,
        },
    )?;

    if let Some(pb) = spinner {
        progress::finish_clear(pb);
    }

    let out = build_impact_out(engine.snapshot(), &report, direction, &level);

    if style.json {
        output::print_json(&out)?;
        return Ok(());
    }

    output::header(
        style,
        &format!(
            "Impact analysis: {} ({})",
            out.subject_fqn, out.subject_kind
        ),
    );
    output::kv(style, "direction", &out.direction);
    output::kv(style, "level", &out.level);
    output::kv(style, "total_affected", out.total_affected);

    if !out.upstream_nodes.is_empty() {
        output::header(style, &format!("Upstream ({})", out.upstream_nodes.len()));
        for n in &out.upstream_nodes {
            output::bullet(style, &format!("[{}] {}", n.kind, n.fqn));
        }
    }
    if !out.downstream_nodes.is_empty() {
        output::header(
            style,
            &format!("Downstream ({})", out.downstream_nodes.len()),
        );
        for n in &out.downstream_nodes {
            output::bullet(style, &format!("[{}] {}", n.kind, n.fqn));
        }
    }
    if out.upstream_nodes.is_empty() && out.downstream_nodes.is_empty() {
        output::muted(style, "No affected objects.");
    }
    Ok(())
}

fn build_impact_out(
    snapshot: &simplineage_core::Snapshot,
    report: &simplineage_analysis::ImpactReport,
    direction: ImpactDirection,
    level: &LevelFlags,
) -> ImpactOut {
    let map_nodes = |ids: &[ObjectId]| -> Vec<ImpactNodeOut> {
        ids.iter()
            .map(|id| ImpactNodeOut {
                id: id.to_string(),
                fqn: context::object_display(snapshot, id),
                kind: context::object_kind_label(snapshot, id).to_string(),
            })
            .collect()
    };
    let upstream_nodes = map_nodes(&report.upstream);
    let downstream_nodes = map_nodes(&report.downstream);

    ImpactOut {
        subject: report.subject.to_string(),
        subject_fqn: context::object_display(snapshot, &report.subject),
        subject_kind: context::object_kind_label(snapshot, &report.subject).to_string(),
        direction: direction_str(direction).into(),
        level: level.label.to_string(),
        total_affected: report.total_affected,
        upstream: upstream_nodes.iter().map(|n| n.id.clone()).collect(),
        downstream: downstream_nodes.iter().map(|n| n.id.clone()).collect(),
        upstream_nodes,
        downstream_nodes,
    }
}

struct LevelFlags {
    relations_only: bool,
    columns_only: bool,
    label: &'static str,
}

/// Resolve `--level` / `--relations-only` into engine flags.
///
/// Default `all` preserves historical behavior (no filtering).
fn resolve_level(level: &str, relations_only_flag: bool) -> anyhow::Result<LevelFlags> {
    let l = level.trim().to_ascii_lowercase();

    // Explicit column level always wins over --relations-only.
    if matches!(l.as_str(), "column" | "columns") {
        return Ok(LevelFlags {
            relations_only: false,
            columns_only: true,
            label: "column",
        });
    }

    if relations_only_flag || matches!(l.as_str(), "relation" | "relations" | "table" | "tables") {
        return Ok(LevelFlags {
            relations_only: true,
            columns_only: false,
            label: "relation",
        });
    }

    match l.as_str() {
        "" | "all" | "auto" | "both" => Ok(LevelFlags {
            relations_only: false,
            columns_only: false,
            label: "all",
        }),
        other => anyhow::bail!("unknown level '{other}' (all|relation|column)"),
    }
}

fn parse_direction(s: &str) -> anyhow::Result<ImpactDirection> {
    match s.to_ascii_lowercase().as_str() {
        "upstream" | "up" => Ok(ImpactDirection::Upstream),
        "downstream" | "down" => Ok(ImpactDirection::Downstream),
        "both" | "all" => Ok(ImpactDirection::Both),
        other => anyhow::bail!("unknown direction '{other}' (upstream|downstream|both)"),
    }
}

fn direction_str(d: ImpactDirection) -> &'static str {
    match d {
        ImpactDirection::Upstream => "upstream",
        ImpactDirection::Downstream => "downstream",
        ImpactDirection::Both => "both",
    }
}
