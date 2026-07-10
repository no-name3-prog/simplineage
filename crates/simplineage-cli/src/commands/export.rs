//! `simplineage export` — write snapshots and analysis reports to disk.

use std::path::PathBuf;

use anyhow::Context;
use simplineage_exporters::{ExportFormat, export_snapshot, write_json_value};

use crate::context::AppContext;
use crate::output::{self};
use crate::progress;

#[derive(Debug)]
pub struct ExportArgs {
    pub format: String,
    pub output: PathBuf,
    pub snapshot: Option<String>,
    pub analysis: bool,
    pub no_progress: bool,
}

pub fn run_export(ctx: &AppContext, args: ExportArgs) -> anyhow::Result<()> {
    let style = ctx.style;

    let spinner = if args.no_progress || style.quiet || style.json {
        None
    } else {
        Some(progress::spinner("Exporting…"))
    };

    let snapshot = ctx.load_snapshot(args.snapshot.as_deref())?;

    if args.analysis || args.format.eq_ignore_ascii_case("analysis") {
        let engine = simplineage_analysis::AnalysisEngine::from_snapshot(snapshot.clone());
        let report = engine.analyze_all();
        write_json_value(&report, &args.output, true)
            .with_context(|| format!("write analysis to {}", args.output.display()))?;
        if let Some(pb) = spinner {
            progress::finish_ok(pb, format!("Wrote {}", args.output.display()), style.quiet);
        }
        if style.json {
            output::print_json(&serde_json::json!({
                "path": args.output.display().to_string(),
                "format": "analysis",
                "snapshot_id": snapshot.id.to_string(),
            }))?;
        } else {
            output::success(
                style,
                &format!("Exported analysis → {}", args.output.display()),
            );
        }
        return Ok(());
    }

    let format = ExportFormat::parse(&args.format).with_context(|| {
        format!(
            "unknown format '{}' (choose one of: {}, analysis)",
            args.format,
            ExportFormat::all_names().join(", ")
        )
    })?;

    export_snapshot(&snapshot, &args.output, format)
        .with_context(|| format!("export to {}", args.output.display()))?;

    if let Some(pb) = spinner {
        progress::finish_ok(pb, format!("Wrote {}", args.output.display()), style.quiet);
    }

    if style.json {
        output::print_json(&serde_json::json!({
            "path": args.output.display().to_string(),
            "format": format.as_str(),
            "snapshot_id": snapshot.id.to_string(),
            "objects": snapshot.object_count(),
            "dependencies": snapshot.dependencies.len(),
        }))?;
    } else {
        output::success(
            style,
            &format!(
                "Exported {} ({}) → {}",
                snapshot.id,
                format.as_str(),
                args.output.display()
            ),
        );
    }
    Ok(())
}
