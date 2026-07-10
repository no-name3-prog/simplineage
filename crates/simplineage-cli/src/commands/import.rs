//! `simplineage import` — ingest metadata files into a snapshot / store.

use std::path::PathBuf;

use anyhow::Context;
use simplineage_analysis::AnalysisEngine;
use simplineage_importers::{ImportOptions, ImporterRegistry};
use simplineage_storage::ImportMode;

use crate::context::AppContext;
use crate::output::{self};
use crate::progress;

#[derive(Debug)]
pub struct ImportArgs {
    pub path: Option<PathBuf>,
    pub importer: Option<String>,
    pub output: Option<PathBuf>,
    pub label: Option<String>,
    pub source: Option<String>,
    pub list_importers: bool,
    pub store: bool,
    pub mode: String,
    pub analyze: bool,
    pub no_progress: bool,
}

pub fn run_import(ctx: &AppContext, args: ImportArgs) -> anyhow::Result<()> {
    let style = ctx.style;
    let mut registry = ImporterRegistry::with_builtins();
    simplineage_importer_sample::register(&mut registry);

    if args.list_importers {
        if style.json {
            let list: Vec<_> = registry
                .ids()
                .into_iter()
                .filter_map(|id| {
                    registry.get(id).map(|imp| {
                        serde_json::json!({
                            "id": id,
                            "name": imp.name(),
                            "description": imp.description(),
                        })
                    })
                })
                .collect();
            output::print_json(&list)?;
        } else {
            output::header(style, "Registered importers");
            for id in registry.ids() {
                if let Some(imp) = registry.get(id) {
                    println!(
                        "  {}  {} — {}",
                        colored::Colorize::bold(colored::Colorize::cyan(id)),
                        imp.name(),
                        colored::Colorize::dimmed(imp.description())
                    );
                }
            }
        }
        return Ok(());
    }

    let path = args
        .path
        .as_ref()
        .context("path is required (or pass --list-importers)")?;

    let opts = ImportOptions {
        label: args.label.clone(),
        source: args.source.clone(),
        ..Default::default()
    };

    let show_progress = !(args.no_progress || style.quiet || style.json);

    // Determinate bar for directories (file count); spinner for single files.
    let file_count = if path.is_dir() {
        count_importable_files(path).unwrap_or(0)
    } else {
        1
    };
    let pb = if show_progress {
        if path.is_dir() && file_count > 1 {
            let b = progress::bar(file_count, format!("Importing {} files…", file_count));
            // Advance partially while import runs (registry is sync / opaque).
            b.set_position(0);
            Some(b)
        } else {
            Some(progress::spinner(format!("Importing {}…", path.display())))
        }
    } else {
        None
    };

    let snapshot = if let Some(id) = &args.importer {
        registry
            .import_with(id, path, &opts)
            .with_context(|| format!("import with importer '{id}'"))?
    } else if path.is_dir() {
        registry
            .import_dir(path, &opts)
            .with_context(|| format!("import directory {}", path.display()))?
    } else {
        registry
            .import_path(path, &opts)
            .with_context(|| format!("import {}", path.display()))?
    };

    if let Some(pb) = pb {
        if path.is_dir() && file_count > 1 {
            pb.set_position(file_count);
        }
        progress::finish_ok(
            pb,
            format!(
                "Imported {} objects, {} edges",
                snapshot.object_count(),
                snapshot.dependencies.len()
            ),
            style.quiet,
        );
    }

    #[derive(serde::Serialize)]
    struct ImportOut {
        snapshot_id: String,
        objects: usize,
        dependencies: usize,
        tables: usize,
        views: usize,
        materialized_views: usize,
        columns: usize,
        model_version: String,
        store: Option<StoreOut>,
    }

    #[derive(serde::Serialize)]
    struct StoreOut {
        snapshot_id: String,
        import_id: String,
        mode: String,
        objects_added: i64,
        edges_added: i64,
        data_dir: String,
    }

    let mut store_out = None;

    // `args.store` is computed by the CLI: default on unless `--no-store` or only `--output`.
    if args.store {
        let mode = parse_mode(&args.mode)?;
        let store = ctx.open_store()?;
        let result = store
            .import(snapshot.clone(), mode, args.source.as_deref())
            .context("persist snapshot to store")?;
        store_out = Some(StoreOut {
            snapshot_id: result.snapshot_id.to_string(),
            import_id: result.import_id.to_string(),
            mode: result.mode,
            objects_added: result.objects_added,
            edges_added: result.edges_added,
            data_dir: ctx.data_dir.display().to_string(),
        });
        output::success(
            style,
            &format!(
                "Stored snapshot {} (mode {}, +{} objects, +{} edges)",
                result.snapshot_id,
                store_out.as_ref().unwrap().mode,
                result.objects_added,
                result.edges_added
            ),
        );
    }

    if let Some(out) = &args.output {
        let json = snapshot.to_json_pretty()?;
        if let Some(parent) = out.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(out, json).with_context(|| format!("write {}", out.display()))?;
        output::success(style, &format!("Wrote {}", out.display()));
    }

    if args.analyze {
        let engine = AnalysisEngine::from_snapshot(snapshot.clone());
        let stats = engine.graph().statistics();
        output::header(style, "Graph statistics");
        output::kv(style, "nodes", stats.node_count);
        output::kv(style, "edges", stats.edge_count);
        output::kv(style, "has_cycle", stats.has_cycle);
        output::kv(style, "components", stats.weakly_connected_components);
    }

    let out = ImportOut {
        snapshot_id: snapshot.id.to_string(),
        objects: snapshot.object_count(),
        dependencies: snapshot.dependencies.len(),
        tables: snapshot.tables.len(),
        views: snapshot.views.len(),
        materialized_views: snapshot.materialized_views.len(),
        columns: snapshot.columns.len(),
        model_version: snapshot.model_version.to_string(),
        store: store_out,
    };

    if style.json {
        output::print_json(&out)?;
    } else if !style.quiet {
        output::header(style, "Import summary");
        output::kv(style, "snapshot", &out.snapshot_id);
        output::kv(style, "model", &out.model_version);
        output::kv(style, "objects", out.objects);
        output::kv(
            style,
            "breakdown",
            format!(
                "tables={} views={} mvs={} columns={} deps={}",
                out.tables, out.views, out.materialized_views, out.columns, out.dependencies
            ),
        );
        if let Some(s) = &out.store {
            output::kv(style, "data_dir", &s.data_dir);
        }
    }

    Ok(())
}

fn parse_mode(s: &str) -> anyhow::Result<ImportMode> {
    match s.to_ascii_lowercase().as_str() {
        "replace" => Ok(ImportMode::Replace),
        "merge" => Ok(ImportMode::Merge),
        other => anyhow::bail!("unknown import mode '{other}' (use replace or merge)"),
    }
}

fn count_importable_files(dir: &std::path::Path) -> std::io::Result<u64> {
    let mut n = 0u64;
    fn walk(path: &std::path::Path, n: &mut u64) -> std::io::Result<()> {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let p = entry.path();
            if p.is_dir() {
                walk(&p, n)?;
            } else if p.is_file() {
                *n += 1;
            }
        }
        Ok(())
    }
    walk(dir, &mut n)?;
    Ok(n)
}
