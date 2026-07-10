//! Import options shared by all plugins.

use std::path::PathBuf;

/// Options controlling a single import operation.
#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
    /// Override snapshot label.
    pub label: Option<String>,
    /// Override snapshot source field.
    pub source: Option<String>,
    /// Default schema name when the file omits one.
    pub default_schema: Option<String>,
    /// Default database name when omitted.
    pub default_database: Option<String>,
    /// Default catalog name when omitted.
    pub default_catalog: Option<String>,
    /// Optional path used only for diagnostics / attributes.
    pub origin: Option<PathBuf>,
    /// Skip final snapshot validation (not recommended).
    pub skip_validation: bool,
}

impl ImportOptions {
    /// Builder: set label.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Builder: set source.
    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }
}
