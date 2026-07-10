//! SimpLineage command-line interface.

use anyhow::Context;
use clap::{Parser, Subcommand};
use simplineage_core::{Engine, PRODUCT_NAME, Settings, VERSION, init_tracing};

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
    config_dir: std::path::PathBuf,

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
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let mut settings = Settings::load_from(&cli.config_dir).unwrap_or_else(|_| Settings::default());

    if let Some(level) = &cli.log_level {
        settings.logging.level = level.clone();
    }

    init_tracing(&settings.logging).context("initialize logging")?;

    let engine = Engine::new(settings);

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
    }

    Ok(())
}
