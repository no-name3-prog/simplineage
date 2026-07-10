//! SimpLineage command-line interface.

use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use simplineage_analysis::{AnalysisEngine, ImpactDirection, ImpactOptions};
use simplineage_core::{Engine, PRODUCT_NAME, Settings, VERSION, init_tracing};
use simplineage_importers::{ImportOptions, ImporterRegistry};
use simplineage_storage::{ImportMode, MetadataStore};

/// SimpLineage — offline-first metadata lineage for data engineers.
#[derive(Debug, Parser)]
#[command(
    name = "simplineage",
    version = VERSION,
    about = "Offline-first metadata intelligence SDK for data lineage",
    long_about = None
)]
struct Cli {
    /// Path to a configuration directory containing default.toml / local.toml.
    #[arg(long, global = true, default_value = "config")]
    config_dir: PathBuf,

    /// Override log level (trace, debug, info, warn, error).
    #[arg(long, global = true, env = "SIMPLINEAGE_LOGGING__LEVEL")]
    log_level: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Print a short greeting and engine status (Hello World).
    Hello {
        /// Optional name to greet.
        #[arg(short, long, default_value = "world")]
        name: String,
    },
    /// Show version and configuration summary.
    Version,
    /// Show engine status.
    Status,
    /// Import metadata from a file or directory (auto-detect format).
    Import {
        /// File or directory path.
        path: PathBuf,
        /// Force a specific importer id (csv, json, parquet, excel, sample-warehouse, …).
        #[arg(long)]
        importer: Option<String>,
        /// Write Snapshot JSON to this path.
        #[arg(long, short)]
        output: Option<PathBuf>,
        /// Snapshot label.
        #[arg(long)]
        label: Option<String>,
        /// List registered importers and exit.
        #[arg(long)]
        list_importers: bool,
        /// Persist snapshot into a local SQLite store directory (creates metadata.sqlite).
        #[arg(long)]
        store: Option<PathBuf>,
        /// Run a short analysis summary after import (impact/orphans/quality).
        #[arg(long)]
        analyze: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let mut settings = Settings::load_from(&cli.config_dir).unwrap_or_else(|_| Settings::default());

    if let Some(level) = &cli.log_level {
        settings.logging.level = level.clone();
    }

    init_tracing(&settings.logging).context("initialize logging")?;

    let mut engine = Engine::new(settings);

    match cli.command.unwrap_or(Commands::Hello {
        name: "world".into(),
    }) {
        Commands::Hello { name } => {
            println!("Hello, {name}! Welcome to {PRODUCT_NAME} v{VERSION}.");
            tracing::info!(%name, "hello command");
            println!("{}", engine.status());
        }
        Commands::Version => {
            println!("{PRODUCT_NAME} {VERSION}");
        }
        Commands::Status => {
            println!("{}", engine.status());
        }
        Commands::Import {
            path,
            importer,
            output,
            label,
            list_importers,
            store,
            analyze,
        } => {
            let mut registry = ImporterRegistry::with_builtins();
            simplineage_importer_sample::register(&mut registry);

            if list_importers {
                println!("Registered importers:");
                for id in registry.ids() {
                    if let Some(imp) = registry.get(id) {
                        println!("  - {id}: {} — {}", imp.name(), imp.description());
                    }
                }
                return Ok(());
            }

            let opts = ImportOptions {
                label,
                ..Default::default()
            };

            let snapshot = if let Some(id) = importer {
                registry
                    .import_with(&id, &path, &opts)
                    .with_context(|| format!("import with importer '{id}'"))?
            } else if path.is_dir() {
                registry
                    .import_dir(&path, &opts)
                    .with_context(|| format!("import directory {}", path.display()))?
            } else {
                registry
                    .import_path(&path, &opts)
                    .with_context(|| format!("import {}", path.display()))?
            };

            println!(
                "Imported snapshot {} ({} objects, model {})",
                snapshot.id,
                snapshot.object_count(),
                snapshot.model_version
            );
            println!(
                "  tables={} views={} mvs={} columns={} relationships={} dependencies={}",
                snapshot.tables.len(),
                snapshot.views.len(),
                snapshot.materialized_views.len(),
                snapshot.columns.len(),
                snapshot.relationships.len(),
                snapshot.dependencies.len()
            );

            if let Some(out) = output {
                let json = snapshot.to_json_pretty()?;
                std::fs::write(&out, json).with_context(|| format!("write {}", out.display()))?;
                println!("Wrote {}", out.display());
            }

            if let Some(store_dir) = store {
                let meta = MetadataStore::open(&store_dir)
                    .with_context(|| format!("open store {}", store_dir.display()))?;
                let result = meta
                    .import(snapshot.clone(), ImportMode::Replace, Some("cli-import"))
                    .context("persist snapshot")?;
                println!(
                    "Stored snapshot {} in {} (import {}, objects+{}, edges+{})",
                    result.snapshot_id,
                    store_dir.join("metadata.sqlite").display(),
                    result.import_id,
                    result.objects_added,
                    result.edges_added
                );
                // Prove fast edge load
                let edges = meta.load_dependencies(&result.snapshot_id)?;
                println!(
                    "  Fast edge reload: {} dependency edges from SQLite index",
                    edges.len()
                );
            }

            if analyze {
                let analysis = AnalysisEngine::from_snapshot(snapshot.clone());
                let full = analysis.analyze_all();
                println!("Analysis summary:");
                println!(
                    "  graph: nodes={} edges={} cycles={}",
                    full.statistics.node_count,
                    full.statistics.edge_count,
                    full.circular.len()
                );
                println!(
                    "  orphans={} unused={} quality_score={:.0}",
                    full.orphans.len(),
                    full.unused.len(),
                    full.quality.score
                );
                if let Some(crit) = full.critical.first() {
                    println!(
                        "  top critical: {} (downstream={})",
                        crit.id, crit.downstream_count
                    );
                }
                // Sample impact on first table if any
                if let Some(t) = snapshot.tables.first() {
                    let imp = analysis.impact(
                        &t.meta.id,
                        &ImpactOptions {
                            direction: ImpactDirection::Downstream,
                            ..Default::default()
                        },
                    )?;
                    println!(
                        "  impact downstream of {}: {} objects",
                        t.meta.fqn, imp.total_affected
                    );
                }
            }

            engine
                .load_snapshot(snapshot)
                .context("load snapshot into engine")?;
            println!("{}", engine.status());
        }
    }

    Ok(())
}
