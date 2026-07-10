//! JSON importer — full [`Snapshot`] or intermediate catalog export.

use std::fs;
use std::path::Path;

use simplineage_core::{Error, Result, Snapshot};

use crate::detect::{has_extension, looks_like_json};
use crate::intermediate::IntermediateCatalog;
use crate::normalize::intermediate_to_snapshot;
use crate::options::ImportOptions;
use crate::plugin::{DetectConfidence, MetadataImporter};

/// Imports metadata from JSON files.
#[derive(Debug, Default)]
pub struct JsonImporter;

impl MetadataImporter for JsonImporter {
    fn id(&self) -> &'static str {
        "json"
    }

    fn name(&self) -> &str {
        "JSON metadata"
    }

    fn description(&self) -> &str {
        "SimpLineage Snapshot JSON or intermediate {tables,columns,...} export"
    }

    fn extensions(&self) -> &[&str] {
        &["json"]
    }

    fn detect(&self, path: &Path) -> DetectConfidence {
        if !path.is_file() {
            return DetectConfidence::None;
        }
        // Defer to warehouse plugins that use specialized *.sample.json names.
        let fname = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if fname.ends_with(".sample.json") {
            return DetectConfidence::None;
        }
        if has_extension(path, &["json"]) && looks_like_json(path) {
            return DetectConfidence::High;
        }
        if looks_like_json(path) {
            return DetectConfidence::Medium;
        }
        if has_extension(path, &["json"]) {
            return DetectConfidence::Low;
        }
        DetectConfidence::None
    }

    fn import(&self, path: &Path, options: &ImportOptions) -> Result<Snapshot> {
        let text = fs::read_to_string(path)
            .map_err(|e| Error::import(format!("failed to read {}: {e}", path.display())))?;
        // Prefer full snapshot
        if let Ok(mut snap) = Snapshot::from_json(text.as_bytes()) {
            if let Some(label) = &options.label {
                snap.label = Some(label.clone());
            }
            if let Some(source) = &options.source {
                snap.source = Some(source.clone());
            }
            if !options.skip_validation {
                use simplineage_core::model::Validate;
                snap.validate()?;
            }
            return Ok(snap);
        }
        let mut cat: IntermediateCatalog = serde_json::from_str(&text).map_err(|e| {
            Error::import(format!(
                "JSON is neither a Snapshot nor IntermediateCatalog: {e}"
            ))
        })?;
        if cat.source.is_none() {
            cat.source = Some(format!("json:{}", path.display()));
        }
        intermediate_to_snapshot(&cat, options)
    }
}
