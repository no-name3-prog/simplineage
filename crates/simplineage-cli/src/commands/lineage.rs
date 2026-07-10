//! Upstream, downstream, and impact analysis commands.

use serde::Serialize;
use simplineage_analysis::{ImpactDirection, ImpactOptions};

use crate::context::{self, AppContext};
use crate::output::{self};
use crate::progress;

#[derive(Debug)]
pub struct LineageArgs {
    pub object: String,
    pub snapshot: Option<String>,
    pub max_depth: Option<usize>,
    pub relations_only: bool,
    pub no_progress: bool,
}

#[derive(Debug)]
pub struct ImpactArgs {
    pub object: String,
    pub snapshot: Option<String>,
    pub direction: String,
    pub max_depth: Option<usize>,
    pub relations_only: bool,
    pub no_progress: bool,
}

#[derive(Debug, Serialize)]
struct ImpactOut {
    subject: String,
    direction: String,
    total_affected: usize,
    upstream: Vec<String>,
    downstream: Vec<String>,
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
    let id = context::resolve_object(engine.snapshot(), &args.object)?;
    let report = engine.impact(
        &id,
        &ImpactOptions {
            direction,
            max_depth: args.max_depth,
            relations_only: args.relations_only,
        },
    )?;

    if let Some(pb) = spinner {
        progress::finish_clear(pb);
    }

    let nodes = match direction {
        ImpactDirection::Upstream => &report.upstream,
        ImpactDirection::Downstream => &report.downstream,
        ImpactDirection::Both => {
            // shouldn't hit for directional cmds
            &report.downstream
        }
    };

    let out = ImpactOut {
        subject: report.subject.to_string(),
        direction: direction_str(direction).into(),
        total_affected: nodes.len(),
        upstream: report.upstream.iter().map(|o| o.to_string()).collect(),
        downstream: report.downstream.iter().map(|o| o.to_string()).collect(),
    };

    if style.json {
        output::print_json(&out)?;
        return Ok(());
    }

    output::header(style, &format!("{title} of {}", out.subject));
    output::kv(style, "count", nodes.len());
    if let Some(d) = args.max_depth {
        output::kv(style, "max_depth", d);
    }
    if nodes.is_empty() {
        output::muted(style, "No related objects in this direction.");
    } else {
        for n in nodes {
            output::bullet(style, n.as_str());
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
    let id = context::resolve_object(engine.snapshot(), &args.object)?;
    let report = engine.impact(
        &id,
        &ImpactOptions {
            direction,
            max_depth: args.max_depth,
            relations_only: args.relations_only,
        },
    )?;

    if let Some(pb) = spinner {
        progress::finish_clear(pb);
    }

    let out = ImpactOut {
        subject: report.subject.to_string(),
        direction: direction_str(direction).into(),
        total_affected: report.total_affected,
        upstream: report.upstream.iter().map(|o| o.to_string()).collect(),
        downstream: report.downstream.iter().map(|o| o.to_string()).collect(),
    };

    if style.json {
        output::print_json(&out)?;
        return Ok(());
    }

    output::header(style, &format!("Impact analysis: {}", out.subject));
    output::kv(style, "direction", &out.direction);
    output::kv(style, "total_affected", out.total_affected);

    if !out.upstream.is_empty() {
        output::header(style, &format!("Upstream ({})", out.upstream.len()));
        for n in &out.upstream {
            output::bullet(style, n);
        }
    }
    if !out.downstream.is_empty() {
        output::header(style, &format!("Downstream ({})", out.downstream.len()));
        for n in &out.downstream {
            output::bullet(style, n);
        }
    }
    if out.upstream.is_empty() && out.downstream.is_empty() {
        output::muted(style, "No affected objects.");
    }
    Ok(())
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
