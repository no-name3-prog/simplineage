//! `simplineage validate` — dependency and metadata quality checks.

use serde::Serialize;
use simplineage_analysis::Severity;

use crate::context::AppContext;
use crate::output::{self};
use crate::progress;

#[derive(Debug)]
pub struct ValidateArgs {
    pub snapshot: Option<String>,
    pub strict: bool,
    pub no_progress: bool,
}

#[derive(Debug, Serialize)]
struct ValidateOut {
    ok: bool,
    edges_checked: usize,
    issue_count: usize,
    errors: usize,
    warnings: usize,
    infos: usize,
    quality_score: f64,
    quality_ok: bool,
    issues: Vec<IssueOut>,
    quality_checks: Vec<QualityOut>,
}

#[derive(Debug, Serialize)]
struct IssueOut {
    severity: String,
    code: String,
    message: String,
    objects: Vec<String>,
}

#[derive(Debug, Serialize)]
struct QualityOut {
    id: String,
    title: String,
    passed: bool,
    finding_count: usize,
    severity: String,
    message: String,
}

pub fn run_validate(ctx: &AppContext, args: ValidateArgs) -> anyhow::Result<()> {
    let style = ctx.style;
    let spinner = if args.no_progress || style.quiet || style.json {
        None
    } else {
        Some(progress::spinner("Validating dependencies & quality…"))
    };

    let engine = ctx.analysis_engine(args.snapshot.as_deref())?;
    let dep = engine.validate_dependencies();
    let quality = engine.quality_checks();

    if let Some(pb) = spinner {
        progress::finish_clear(pb);
    }

    let mut errors = 0usize;
    let mut warnings = 0usize;
    let mut infos = 0usize;
    for i in &dep.issues {
        match i.severity {
            Severity::Error => errors += 1,
            Severity::Warning => warnings += 1,
            Severity::Info => infos += 1,
        }
    }

    let ok = dep.ok && (!args.strict || (warnings == 0 && quality.ok));

    let out = ValidateOut {
        ok,
        edges_checked: dep.edges_checked,
        issue_count: dep.issues.len(),
        errors,
        warnings,
        infos,
        quality_score: quality.score,
        quality_ok: quality.ok,
        issues: dep
            .issues
            .iter()
            .map(|i| IssueOut {
                severity: severity_str(i.severity).into(),
                code: i.code.clone(),
                message: i.message.clone(),
                objects: i.objects.iter().map(|o| o.to_string()).collect(),
            })
            .collect(),
        quality_checks: quality
            .checks
            .iter()
            .map(|c| QualityOut {
                id: c.id.clone(),
                title: c.title.clone(),
                passed: c.passed,
                finding_count: c.finding_count,
                severity: severity_str(c.severity).into(),
                message: c.message.clone(),
            })
            .collect(),
    };

    if style.json {
        output::print_json(&out)?;
    } else {
        output::header(style, "Dependency validation");
        output::kv(style, "edges_checked", out.edges_checked);
        output::kv(style, "ok", out.ok);
        output::kv(
            style,
            "issues",
            format!(
                "{} (errors={} warnings={} info={})",
                out.issue_count, out.errors, out.warnings, out.infos
            ),
        );

        for issue in &out.issues {
            let label = output::severity_label(&issue.severity);
            println!(
                "  [{label}] {} — {}",
                colored::Colorize::bold(issue.code.as_str()),
                issue.message
            );
        }

        output::header(style, "Metadata quality");
        output::kv(style, "score", format!("{:.1}/100", out.quality_score));
        output::kv(style, "quality_ok", out.quality_ok);
        for c in &out.quality_checks {
            let mark = if c.passed {
                colored::Colorize::green("PASS")
            } else {
                colored::Colorize::red("FAIL")
            };
            println!("  {}  {} ({} findings)", mark, c.title, c.finding_count);
        }

        if out.ok {
            output::success(style, "Validation passed");
        } else {
            output::error_line("Validation failed");
        }
    }

    if !out.ok {
        anyhow::bail!("validation failed");
    }
    Ok(())
}

fn severity_str(s: Severity) -> &'static str {
    match s {
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Error => "error",
    }
}
