//! SimpLineage command-line interface.
//!
//! Professional offline CLI for metadata import, lineage graph analysis,
//! validation, comparison, and export.

mod commands;
mod context;
mod output;
mod progress;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use simplineage_core::{PRODUCT_NAME, Settings, VERSION, init_tracing};

use crate::commands::{
    build::BuildArgs,
    compare::CompareArgs,
    export::ExportArgs,
    import::ImportArgs,
    lineage::{ImpactArgs, LineageArgs},
    search::SearchArgs,
    stats::StatsArgs,
    validate::ValidateArgs,
};
use crate::context::{AppContext, DEFAULT_DATA_DIR};
use crate::output::OutputStyle;

/// SimpLineage — offline-first metadata lineage for data engineers.
#[derive(Debug, Parser)]
#[command(
    name = "simplineage",
    version = VERSION,
    about = "Offline-first metadata intelligence for data lineage",
    long_about = "SimpLineage imports warehouse metadata exports, builds a local lineage graph,\n\
                  and runs impact analysis — fully offline.\n\n\
                  Typical workflow:\n  \
                  simplineage import ./export.csv\n  \
                  simplineage build --full\n  \
                  simplineage search orders\n  \
                  simplineage impact public.orders --direction both\n  \
                  simplineage export -f html -o report.html",
    propagate_version = true,
    arg_required_else_help = true,
    styles = clap_styles()
)]
struct Cli {
    /// Path to a configuration directory containing default.toml / local.toml.
    #[arg(
        long,
        global = true,
        default_value = "config",
        env = "SIMPLINEAGE_CONFIG_DIR"
    )]
    config_dir: PathBuf,

    /// Local SQLite store directory (metadata.sqlite).
    #[arg(
        long,
        short = 'd',
        global = true,
        default_value = DEFAULT_DATA_DIR,
        env = "SIMPLINEAGE_DATA_DIR"
    )]
    data_dir: PathBuf,

    /// Override log level (trace, debug, info, warn, error).
    #[arg(long, global = true, env = "SIMPLINEAGE_LOGGING__LEVEL")]
    log_level: Option<String>,

    /// Emit machine-readable JSON on stdout where applicable.
    #[arg(long, global = true, env = "SIMPLINEAGE_JSON")]
    json: bool,

    /// Reduce decorative output.
    #[arg(long, short = 'q', global = true)]
    quiet: bool,

    /// Disable ANSI colors (also honored when `NO_COLOR` is set in the environment).
    #[arg(long, global = true)]
    no_color: bool,

    /// Disable progress spinners and bars.
    #[arg(long, global = true)]
    no_progress: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Import metadata from a file or directory (auto-detect format).
    Import {
        /// File or directory path.
        #[arg(required_unless_present = "list_importers")]
        path: Option<PathBuf>,
        /// Force a specific importer id (csv, json, parquet, excel, sample-warehouse, …).
        #[arg(long)]
        importer: Option<String>,
        /// Write Snapshot JSON to this path.
        #[arg(long, short)]
        output: Option<PathBuf>,
        /// Snapshot label.
        #[arg(long)]
        label: Option<String>,
        /// Snapshot source system label.
        #[arg(long)]
        source: Option<String>,
        /// List registered importers and exit.
        #[arg(long)]
        list_importers: bool,
        /// Persist into the local store (default when --output is omitted).
        #[arg(long)]
        store: bool,
        /// Skip local store even when --output is omitted.
        #[arg(long)]
        no_store: bool,
        /// Import mode when writing to the store.
        #[arg(long, default_value = "replace", value_parser = ["replace", "merge"])]
        mode: String,
        /// Print graph statistics after import.
        #[arg(long)]
        analyze: bool,
    },

    /// Build the lineage graph from the store (or a snapshot file / id).
    Build {
        /// Snapshot id or JSON file (default: current store head).
        #[arg(long, short = 's')]
        snapshot: Option<String>,
        /// Run the full Phase 4 analysis suite.
        #[arg(long)]
        full: bool,
        /// Write build/analysis report JSON to this path.
        #[arg(long, short)]
        output: Option<PathBuf>,
    },

    /// Search metadata objects by id, FQN, name, or description.
    Search {
        /// Free-text query (substring match).
        query: String,
        /// Filter by kind (table, view, column, …).
        #[arg(long, short = 'k')]
        kind: Option<String>,
        /// Maximum results.
        #[arg(long, short = 'n', default_value_t = 50)]
        limit: usize,
        /// Snapshot id or JSON file (default: current).
        #[arg(long, short = 's')]
        snapshot: Option<String>,
    },

    /// List upstream providers for an object (table or column).
    Upstream {
        /// Object id or FQN (or unique name fragment). Accepts `schema.table.column`.
        object: String,
        /// Snapshot id or JSON file.
        #[arg(long, short = 's')]
        snapshot: Option<String>,
        /// Maximum hop depth.
        #[arg(long)]
        max_depth: Option<usize>,
        /// Restrict to tables / views / MVs (alias for `--level relation`).
        #[arg(long)]
        relations_only: bool,
        /// Result grain: `all` (default), `relation`, or `column`.
        #[arg(long, default_value = "all", value_parser = ["all", "auto", "relation", "relations", "column", "columns", "table", "tables", "both"])]
        level: String,
        /// Column name when `object` is a parent table/view (e.g. `--column email`).
        #[arg(long, short = 'c')]
        column: Option<String>,
    },

    /// List downstream consumers for an object (table or column).
    Downstream {
        /// Object id or FQN (or unique name fragment). Accepts `schema.table.column`.
        object: String,
        /// Snapshot id or JSON file.
        #[arg(long, short = 's')]
        snapshot: Option<String>,
        /// Maximum hop depth.
        #[arg(long)]
        max_depth: Option<usize>,
        /// Restrict to tables / views / MVs (alias for `--level relation`).
        #[arg(long)]
        relations_only: bool,
        /// Result grain: `all` (default), `relation`, or `column`.
        #[arg(long, default_value = "all", value_parser = ["all", "auto", "relation", "relations", "column", "columns", "table", "tables", "both"])]
        level: String,
        /// Column name when `object` is a parent table/view (e.g. `--column email`).
        #[arg(long, short = 'c')]
        column: Option<String>,
    },

    /// Impact analysis (upstream, downstream, or both) at table or column grain.
    Impact {
        /// Object id or FQN (or unique name fragment). Accepts `schema.table.column`.
        object: String,
        /// Snapshot id or JSON file.
        #[arg(long, short = 's')]
        snapshot: Option<String>,
        /// Impact direction.
        #[arg(long, short = 'D', default_value = "both", value_parser = ["upstream", "downstream", "both", "up", "down", "all"])]
        direction: String,
        /// Maximum hop depth.
        #[arg(long)]
        max_depth: Option<usize>,
        /// Restrict to tables / views / MVs (alias for `--level relation`).
        #[arg(long)]
        relations_only: bool,
        /// Result grain: `all` (default), `relation`, or `column`.
        #[arg(long, default_value = "all", value_parser = ["all", "auto", "relation", "relations", "column", "columns", "table", "tables", "both"])]
        level: String,
        /// Column name when `object` is a parent table/view (e.g. `--column email`).
        #[arg(long, short = 'c')]
        column: Option<String>,
    },

    /// Validate dependencies and metadata quality.
    Validate {
        /// Snapshot id or JSON file.
        #[arg(long, short = 's')]
        snapshot: Option<String>,
        /// Fail on warnings as well as errors.
        #[arg(long)]
        strict: bool,
    },

    /// Show store catalog and graph statistics.
    Stats {
        /// Snapshot id or JSON file for graph stats (default: current).
        #[arg(long, short = 's')]
        snapshot: Option<String>,
        /// Only list store catalog rows (skip graph stats).
        #[arg(long)]
        store_only: bool,
    },

    /// Compare two snapshots (ids or JSON files).
    Compare {
        /// Left snapshot id or file.
        left: String,
        /// Right snapshot id or file.
        right: String,
        /// Max ids to print per section.
        #[arg(long, short = 'n', default_value_t = 20)]
        limit: usize,
    },

    /// Export a snapshot or analysis report.
    Export {
        /// Output path.
        #[arg(short, long)]
        output: PathBuf,
        /// Format: json, json-pretty, objects-csv, edges-csv, graphml, html, mermaid, analysis.
        #[arg(short = 'f', long, default_value = "json-pretty")]
        format: String,
        /// Snapshot id or JSON file.
        #[arg(long, short = 's')]
        snapshot: Option<String>,
        /// Export full analysis report (same as --format analysis).
        #[arg(long)]
        analysis: bool,
    },

    /// Show version information.
    Version {
        /// Output format.
        #[arg(long, value_enum, default_value_t = VersionFormat::Text)]
        format: VersionFormat,
    },

    /// Show engine / environment status.
    Status,

    /// Print a short greeting (compatibility / smoke test).
    Hello {
        /// Optional name to greet.
        #[arg(short, long, default_value = "world")]
        name: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum VersionFormat {
    Text,
    Json,
}

fn clap_styles() -> clap::builder::Styles {
    use clap::builder::styling::{AnsiColor, Effects, Styles};
    Styles::styled()
        .header(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .usage(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .literal(AnsiColor::Green.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::BrightBlue.on_default())
        .error(AnsiColor::Red.on_default() | Effects::BOLD)
        .valid(AnsiColor::Green.on_default() | Effects::BOLD)
        .invalid(AnsiColor::Yellow.on_default() | Effects::BOLD)
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            // Validation failures already printed a styled line.
            let msg = format!("{err:#}");
            if !msg.contains("validation failed") {
                output::error_line(&msg);
            }
            ExitCode::FAILURE
        }
    }
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let style = OutputStyle {
        json: cli.json,
        quiet: cli.quiet,
        no_color: cli.no_color || std::env::var_os("NO_COLOR").is_some(),
    };
    style.apply_global();

    let mut settings = Settings::load_from(&cli.config_dir).unwrap_or_else(|_| Settings::default());
    if let Some(level) = &cli.log_level {
        settings.logging.level = level.clone();
    } else if style.quiet || style.json {
        // Keep CLI stdout clean for piping.
        if settings.logging.level == "info" {
            settings.logging.level = "warn".into();
        }
    }
    init_tracing(&settings.logging).context("initialize logging")?;

    let ctx = AppContext {
        data_dir: cli.data_dir.clone(),
        style,
    };

    let no_progress = cli.no_progress;

    match cli.command {
        Commands::Import {
            path,
            importer,
            output,
            label,
            source,
            list_importers,
            store,
            no_store,
            mode,
            analyze,
        } => {
            let store_flag = if no_store {
                false
            } else {
                store || output.is_none()
            };
            commands::run_import(
                &ctx,
                ImportArgs {
                    path,
                    importer,
                    output,
                    label,
                    source,
                    list_importers,
                    store: store_flag,
                    mode,
                    analyze,
                    no_progress,
                },
            )
        }
        Commands::Build {
            snapshot,
            full,
            output,
        } => commands::run_build(
            &ctx,
            BuildArgs {
                snapshot,
                full,
                output,
                no_progress,
            },
        ),
        Commands::Search {
            query,
            kind,
            limit,
            snapshot,
        } => commands::run_search(
            &ctx,
            SearchArgs {
                query,
                kind,
                limit,
                snapshot,
            },
        ),
        Commands::Upstream {
            object,
            snapshot,
            max_depth,
            relations_only,
            level,
            column,
        } => commands::run_upstream(
            &ctx,
            LineageArgs {
                object,
                snapshot,
                max_depth,
                relations_only,
                level,
                column,
                no_progress,
            },
        ),
        Commands::Downstream {
            object,
            snapshot,
            max_depth,
            relations_only,
            level,
            column,
        } => commands::run_downstream(
            &ctx,
            LineageArgs {
                object,
                snapshot,
                max_depth,
                relations_only,
                level,
                column,
                no_progress,
            },
        ),
        Commands::Impact {
            object,
            snapshot,
            direction,
            max_depth,
            relations_only,
            level,
            column,
        } => commands::run_impact(
            &ctx,
            ImpactArgs {
                object,
                snapshot,
                direction,
                max_depth,
                relations_only,
                level,
                column,
                no_progress,
            },
        ),
        Commands::Validate { snapshot, strict } => commands::run_validate(
            &ctx,
            ValidateArgs {
                snapshot,
                strict,
                no_progress,
            },
        ),
        Commands::Stats {
            snapshot,
            store_only,
        } => commands::run_stats(
            &ctx,
            StatsArgs {
                snapshot,
                store_only,
            },
        ),
        Commands::Compare { left, right, limit } => {
            commands::run_compare(&ctx, CompareArgs { left, right, limit })
        }
        Commands::Export {
            output,
            format,
            snapshot,
            analysis,
        } => commands::run_export(
            &ctx,
            ExportArgs {
                format,
                output,
                snapshot,
                analysis,
                no_progress,
            },
        ),
        Commands::Version { format } => {
            match format {
                VersionFormat::Text => {
                    println!("{PRODUCT_NAME} {VERSION}");
                }
                VersionFormat::Json => {
                    output::print_json(&serde_json::json!({
                        "name": PRODUCT_NAME,
                        "version": VERSION,
                    }))?;
                }
            }
            Ok(())
        }
        Commands::Status => {
            let engine = simplineage_core::Engine::new(settings);
            if style.json {
                output::print_json(&serde_json::json!({
                    "product": PRODUCT_NAME,
                    "version": VERSION,
                    "status": engine.status(),
                    "data_dir": ctx.data_dir.display().to_string(),
                }))?;
            } else {
                println!("{}", engine.status());
                output::kv(style, "data_dir", ctx.data_dir.display());
            }
            Ok(())
        }
        Commands::Hello { name } => {
            if !style.json {
                println!("Hello, {name}! Welcome to {PRODUCT_NAME} v{VERSION}.");
            } else {
                output::print_json(&serde_json::json!({
                    "hello": name,
                    "product": PRODUCT_NAME,
                    "version": VERSION,
                }))?;
            }
            Ok(())
        }
    }
}
