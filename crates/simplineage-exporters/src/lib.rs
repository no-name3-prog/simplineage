//! Exporters for lineage artifacts (HTML, JSON, GraphML, etc.).

#![warn(missing_docs)]
#![forbid(unsafe_code)]

use std::path::Path;

use simplineage_core::Result;

/// Trait for export backends.
pub trait Exporter: Send + Sync {
    /// Stable exporter name.
    fn name(&self) -> &str;

    /// Export lineage data to `path`.
    fn export(&self, path: &Path) -> Result<()>;
}

/// Placeholder HTML exporter.
#[derive(Debug, Default)]
pub struct HtmlExporter;

impl Exporter for HtmlExporter {
    fn name(&self) -> &str {
        "html"
    }

    fn export(&self, path: &Path) -> Result<()> {
        tracing::debug!(?path, "html export stub");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_exporter_name() {
        assert_eq!(HtmlExporter.name(), "html");
    }
}
