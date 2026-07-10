//! Sample warehouse importer plugin.
//!
//! Demonstrates that adding a warehouse only requires a new crate implementing
//! [`MetadataImporter`](simplineage_importers::MetadataImporter) and registering
//! it. This plugin reads a minimal proprietary JSON dump:
//!
//! ```json
//! {
//!   "warehouse": "sample",
//!   "objects": [
//!     { "schema": "public", "table": "orders", "type": "table" },
//!     { "schema": "public", "table": "orders", "column": "id", "data_type": "BIGINT" }
//!   ]
//! }
//! ```

#![warn(missing_docs)]
#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;

use serde::Deserialize;
use simplineage_core::{Error, Result, Snapshot};
use simplineage_importers::detect::{has_extension, looks_like_json};
use simplineage_importers::{
    DetectConfidence, ImportOptions, ImporterRegistry, IntermediateCatalog, IntermediateColumn,
    IntermediateTable, MetadataImporter, intermediate_to_snapshot,
};

/// Sample warehouse export importer.
#[derive(Debug, Default)]
pub struct SampleWarehouseImporter;

/// Register this plugin on a registry.
pub fn register(registry: &mut ImporterRegistry) {
    registry.register(Box::new(SampleWarehouseImporter));
}

#[derive(Debug, Deserialize)]
struct SampleDump {
    #[serde(default)]
    warehouse: Option<String>,
    #[serde(default)]
    objects: Vec<SampleObject>,
}

#[derive(Debug, Deserialize)]
struct SampleObject {
    #[serde(default)]
    schema: Option<String>,
    #[serde(default)]
    table: Option<String>,
    #[serde(default)]
    column: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    data_type: Option<String>,
}

impl MetadataImporter for SampleWarehouseImporter {
    fn id(&self) -> &'static str {
        "sample-warehouse"
    }

    fn name(&self) -> &str {
        "Sample warehouse export"
    }

    fn description(&self) -> &str {
        "Example plugin for proprietary JSON warehouse dumps (*.sample.json)"
    }

    fn extensions(&self) -> &[&str] {
        &["json"]
    }

    fn detect(&self, path: &Path) -> DetectConfidence {
        if !path.is_file() {
            return DetectConfidence::None;
        }
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if name.ends_with(".sample.json") {
            return DetectConfidence::High;
        }
        // Content probe: must include "warehouse" + "objects"
        if has_extension(path, &["json"]) && looks_like_json(path) {
            if let Ok(text) = fs::read_to_string(path) {
                if text.contains("\"warehouse\"") && text.contains("\"objects\"") {
                    return DetectConfidence::Medium;
                }
            }
        }
        DetectConfidence::None
    }

    fn import(&self, path: &Path, options: &ImportOptions) -> Result<Snapshot> {
        let text = fs::read_to_string(path)
            .map_err(|e| Error::import(format!("read {}: {e}", path.display())))?;
        let dump: SampleDump = serde_json::from_str(&text)
            .map_err(|e| Error::import(format!("parse sample warehouse dump: {e}")))?;

        let mut cat = IntermediateCatalog {
            source: dump
                .warehouse
                .clone()
                .or_else(|| Some("sample-warehouse".into())),
            ..Default::default()
        };

        for obj in dump.objects {
            if let Some(col) = obj.column {
                let table = obj
                    .table
                    .ok_or_else(|| Error::import("sample object with column missing table"))?;
                cat.columns.push(IntermediateColumn {
                    schema: obj.schema,
                    table,
                    name: col,
                    data_type: obj.data_type,
                    ..Default::default()
                });
            } else if let Some(table) = obj.table {
                cat.tables.push(IntermediateTable {
                    schema: obj.schema,
                    name: table,
                    kind: obj.kind,
                    ..Default::default()
                });
            }
        }

        let mut opts = options.clone();
        if opts.source.is_none() {
            opts.source = cat.source.clone();
        }
        intermediate_to_snapshot(&cat, &opts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn imports_sample_dump() {
        let mut f = NamedTempFile::new().unwrap();
        write!(
            f,
            r#"{{"warehouse":"demo","objects":[
              {{"schema":"public","table":"orders","type":"table"}},
              {{"schema":"public","table":"orders","column":"id","data_type":"BIGINT"}}
            ]}}"#
        )
        .unwrap();
        // rename to .sample.json for high confidence
        let path = f.path().with_extension("sample.json");
        std::fs::copy(f.path(), &path).unwrap();

        let mut reg = ImporterRegistry::with_builtins();
        register(&mut reg);
        let det = reg.detect(&path).unwrap();
        assert_eq!(det.importer.id(), "sample-warehouse");
        let snap = reg.import_path(&path, &ImportOptions::default()).unwrap();
        assert_eq!(snap.tables.len(), 1);
        assert_eq!(snap.columns.len(), 1);
        let _ = std::fs::remove_file(path);
    }
}
