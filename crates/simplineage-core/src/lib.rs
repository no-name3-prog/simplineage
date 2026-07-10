//! Core engine for [SimpLineage](https://github.com/no-name3-prog/simplineage).
//!
//! This crate provides shared types, configuration loading, logging setup, and
//! the foundational APIs that importers, storage, analysis, and exporters build on.
//!
//! # Status
//!
//! Phase 0.1 foundation — public surface will expand as the lineage engine lands.

#![warn(missing_docs)]
#![warn(clippy::all)]
#![forbid(unsafe_code)]

pub mod config;
pub mod error;
pub mod logging;

pub use config::Settings;
pub use error::{Error, Result};
pub use logging::init_tracing;

/// Library version from Cargo package metadata.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Human-readable product name.
pub const PRODUCT_NAME: &str = "SimpLineage";

/// Placeholder entry point for the core engine.
///
/// Future phases will load metadata, build graphs, and run analysis from here.
#[derive(Debug, Default, Clone)]
pub struct Engine {
    settings: Settings,
}

impl Engine {
    /// Create a new engine with the given settings.
    pub fn new(settings: Settings) -> Self {
        Self { settings }
    }

    /// Create an engine from the default configuration sources.
    pub fn from_default_config() -> Result<Self> {
        Ok(Self::new(Settings::load()?))
    }

    /// Borrow the active settings.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Short status string for CLI / health checks.
    pub fn status(&self) -> String {
        format!(
            "{PRODUCT_NAME} v{VERSION} ready (log_level={}, data_dir={})",
            self.settings.logging.level,
            self.settings.storage.data_dir.display()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver_like() {
        assert!(!VERSION.is_empty());
        assert!(VERSION.contains('.'));
    }

    #[test]
    fn engine_status_mentions_product() {
        let engine = Engine::new(Settings::default());
        let status = engine.status();
        assert!(status.contains(PRODUCT_NAME));
        assert!(status.contains(VERSION));
    }
}
