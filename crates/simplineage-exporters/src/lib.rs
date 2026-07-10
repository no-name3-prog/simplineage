//! Exporters for lineage artifacts (JSON, CSV, GraphML, HTML, Mermaid).
//!
//! # Offline HTML report (Phase 7)
//!
//! [`html::write_html_report`] emits a **self-contained** HTML file with an
//! interactive lineage graph. No backend server or network access is required
//! to view the report in a browser.
//!
//! Features: pan/zoom, search, kind filters, metadata side panel, client-side
//! impact analysis, dark mode, SVG download, Mermaid download.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod html;
pub mod mermaid;

use std::fs;
use std::io::Write;
use std::path::Path;

use simplineage_core::{Error, Result, Snapshot};

pub use html::{ReportData, build_report_data, render_html_report, write_html_report};
pub use mermaid::{MermaidOptions, render_mermaid, write_mermaid};

/// Supported export formats for CLI and library callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// Compact snapshot JSON.
    Json,
    /// Pretty-printed snapshot JSON.
    JsonPretty,
    /// One catalog object per CSV row.
    ObjectsCsv,
    /// One dependency edge per CSV row.
    EdgesCsv,
    /// GraphML for desktop graph tools.
    GraphMl,
    /// Interactive offline HTML lineage report.
    Html,
    /// Mermaid flowchart (`.mmd` text).
    Mermaid,
}

impl ExportFormat {
    /// Parse a format name (case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "json" => Some(Self::Json),
            "json-pretty" | "json_pretty" | "pretty" => Some(Self::JsonPretty),
            "objects-csv" | "objects_csv" | "csv" => Some(Self::ObjectsCsv),
            "edges-csv" | "edges_csv" | "edges" => Some(Self::EdgesCsv),
            "graphml" | "graph-ml" => Some(Self::GraphMl),
            "html" | "report" | "lineage-html" => Some(Self::Html),
            "mermaid" | "mmd" => Some(Self::Mermaid),
            _ => None,
        }
    }

    /// Canonical CLI name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::JsonPretty => "json-pretty",
            Self::ObjectsCsv => "objects-csv",
            Self::EdgesCsv => "edges-csv",
            Self::GraphMl => "graphml",
            Self::Html => "html",
            Self::Mermaid => "mermaid",
        }
    }

    /// All format names (for help text).
    #[must_use]
    pub fn all_names() -> &'static [&'static str] {
        &[
            "json",
            "json-pretty",
            "objects-csv",
            "edges-csv",
            "graphml",
            "html",
            "mermaid",
        ]
    }
}

/// Trait for export backends.
pub trait Exporter: Send + Sync {
    /// Stable exporter name.
    fn name(&self) -> &str;

    /// Export lineage data to `path`.
    fn export(&self, path: &Path) -> Result<()>;
}

/// Export a [`Snapshot`] to `path` in the given format.
pub fn export_snapshot(snapshot: &Snapshot, path: &Path, format: ExportFormat) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(Error::Io)?;
        }
    }

    match format {
        ExportFormat::Json => {
            let json = snapshot.to_json()?;
            fs::write(path, json).map_err(Error::Io)?;
        }
        ExportFormat::JsonPretty => {
            let json = snapshot.to_json_pretty()?;
            fs::write(path, json).map_err(Error::Io)?;
        }
        ExportFormat::ObjectsCsv => write_objects_csv(snapshot, path)?,
        ExportFormat::EdgesCsv => write_edges_csv(snapshot, path)?,
        ExportFormat::GraphMl => write_graphml(snapshot, path)?,
        ExportFormat::Html => write_html_report(snapshot, path)?,
        ExportFormat::Mermaid => write_mermaid(snapshot, path, &MermaidOptions::default())?,
    }

    tracing::info!(?path, format = format.as_str(), "exported snapshot");
    Ok(())
}

/// Write arbitrary JSON (e.g. analysis report) to a path.
pub fn write_json_value<T: serde::Serialize>(value: &T, path: &Path, pretty: bool) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(Error::Io)?;
        }
    }
    let json = if pretty {
        serde_json::to_string_pretty(value)?
    } else {
        serde_json::to_string(value)?
    };
    fs::write(path, json).map_err(Error::Io)?;
    Ok(())
}

fn write_objects_csv(snapshot: &Snapshot, path: &Path) -> Result<()> {
    let mut f = fs::File::create(path).map_err(Error::Io)?;
    writeln!(f, "id,kind,fqn,name,description").map_err(Error::Io)?;
    for (id, obj) in snapshot.object_index() {
        let meta = obj.meta();
        let desc = meta
            .description
            .as_deref()
            .unwrap_or("")
            .replace('"', "\"\"");
        writeln!(
            f,
            "{},{},{},{},\"{}\"",
            csv_escape(id.as_str()),
            obj.kind_name(),
            csv_escape(&meta.fqn.to_dotted()),
            csv_escape(&meta.name),
            desc
        )
        .map_err(Error::Io)?;
    }
    Ok(())
}

fn write_edges_csv(snapshot: &Snapshot, path: &Path) -> Result<()> {
    let mut f = fs::File::create(path).map_err(Error::Io)?;
    writeln!(f, "id,from_id,to_id,kind,level").map_err(Error::Io)?;
    for dep in &snapshot.dependencies {
        let kind = dep_kind_label(&dep.kind);
        let level = match dep.level {
            simplineage_core::model::graph::DependencyLevel::Relation => "relation",
            simplineage_core::model::graph::DependencyLevel::Column => "column",
            simplineage_core::model::graph::DependencyLevel::Unknown => "unknown",
        };
        writeln!(
            f,
            "{},{},{},{},{}",
            csv_escape(dep.id.as_str()),
            csv_escape(dep.from_id.as_str()),
            csv_escape(dep.to_id.as_str()),
            kind,
            level
        )
        .map_err(Error::Io)?;
    }
    Ok(())
}

fn write_graphml(snapshot: &Snapshot, path: &Path) -> Result<()> {
    let mut f = fs::File::create(path).map_err(Error::Io)?;
    writeln!(
        f,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<graphml xmlns="http://graphml.graphdrawing.org/xmlns">
  <key id="kind" for="node" attr.name="kind" attr.type="string"/>
  <key id="fqn" for="node" attr.name="fqn" attr.type="string"/>
  <key id="edge_kind" for="edge" attr.name="kind" attr.type="string"/>
  <graph id="lineage" edgedefault="directed">"#
    )
    .map_err(Error::Io)?;

    let index = snapshot.object_index();
    let mut seen = std::collections::BTreeSet::new();
    for (id, obj) in &index {
        seen.insert(id.as_str().to_string());
        let kind = obj.kind_name();
        let fqn = obj.meta().fqn.to_dotted();
        writeln!(
            f,
            r#"    <node id="{}"><data key="kind">{}</data><data key="fqn">{}</data></node>"#,
            xml_escape(id.as_str()),
            xml_escape(kind),
            xml_escape(&fqn)
        )
        .map_err(Error::Io)?;
    }
    for dep in &snapshot.dependencies {
        for endpoint in [&dep.from_id, &dep.to_id] {
            if seen.insert(endpoint.as_str().to_string()) {
                writeln!(
                    f,
                    r#"    <node id="{}"><data key="kind">unknown</data><data key="fqn">{}</data></node>"#,
                    xml_escape(endpoint.as_str()),
                    xml_escape(endpoint.as_str())
                )
                .map_err(Error::Io)?;
            }
        }
        let kind = dep_kind_label(&dep.kind);
        writeln!(
            f,
            r#"    <edge id="{}" source="{}" target="{}"><data key="edge_kind">{}</data></edge>"#,
            xml_escape(dep.id.as_str()),
            xml_escape(dep.from_id.as_str()),
            xml_escape(dep.to_id.as_str()),
            xml_escape(kind)
        )
        .map_err(Error::Io)?;
    }
    writeln!(f, "  </graph>\n</graphml>").map_err(Error::Io)?;
    Ok(())
}

fn dep_kind_label(kind: &simplineage_core::model::graph::DependencyKind) -> &str {
    match kind {
        simplineage_core::model::graph::DependencyKind::ViewDefinition => "view_definition",
        simplineage_core::model::graph::DependencyKind::Pipeline => "pipeline",
        simplineage_core::model::graph::DependencyKind::ForeignKey => "foreign_key",
        simplineage_core::model::graph::DependencyKind::Manual => "manual",
        simplineage_core::model::graph::DependencyKind::Inferred => "inferred",
        simplineage_core::model::graph::DependencyKind::Other(s) => s.as_str(),
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// HTML exporter implementing [`Exporter`].
#[derive(Debug, Default)]
pub struct HtmlExporter {
    /// Snapshot to export.
    pub snapshot: Option<Snapshot>,
}

impl Exporter for HtmlExporter {
    fn name(&self) -> &str {
        "html"
    }

    fn export(&self, path: &Path) -> Result<()> {
        let snap = self
            .snapshot
            .as_ref()
            .cloned()
            .unwrap_or_else(Snapshot::new);
        export_snapshot(&snap, path, ExportFormat::Html)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use simplineage_core::ObjectId;
    use simplineage_core::model::graph::{Dependency, DependencyKind, DependencyLevel};
    use simplineage_core::model::ids::FullyQualifiedName;
    use simplineage_core::model::objects::{ObjectMeta, Table};

    fn sample() -> Snapshot {
        let mut s = Snapshot::new();
        s.tables.push(Table {
            meta: ObjectMeta::new(
                ObjectId::from_trusted("t1"),
                FullyQualifiedName::parse_dotted("db.public.orders").unwrap(),
            ),
            schema_id: None,
            column_ids: vec![],
        });
        s.dependencies.push(Dependency {
            id: ObjectId::from_trusted("d1"),
            from_id: ObjectId::from_trusted("t1"),
            to_id: ObjectId::from_trusted("t2"),
            kind: DependencyKind::ViewDefinition,
            level: DependencyLevel::Relation,
            confidence: None,
            attributes: Default::default(),
        });
        s
    }

    #[test]
    fn export_json_roundtrip() {
        let dir = std::env::temp_dir().join(format!("sl-export-{}", uuid_stub()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("snap.json");
        let snap = sample();
        export_snapshot(&snap, &path, ExportFormat::JsonPretty).unwrap();
        let loaded = Snapshot::from_json(fs::read(&path).unwrap()).unwrap();
        assert_eq!(loaded.tables.len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_csv_graphml_html_mermaid() {
        let dir = std::env::temp_dir().join(format!("sl-export2-{}", uuid_stub()));
        let _ = fs::create_dir_all(&dir);
        let snap = sample();
        export_snapshot(&snap, &dir.join("o.csv"), ExportFormat::ObjectsCsv).unwrap();
        export_snapshot(&snap, &dir.join("e.csv"), ExportFormat::EdgesCsv).unwrap();
        export_snapshot(&snap, &dir.join("g.graphml"), ExportFormat::GraphMl).unwrap();
        export_snapshot(&snap, &dir.join("h.html"), ExportFormat::Html).unwrap();
        export_snapshot(&snap, &dir.join("m.mmd"), ExportFormat::Mermaid).unwrap();
        let html = fs::read_to_string(dir.join("h.html")).unwrap();
        assert!(html.contains("btn-export-svg"));
        assert!(html.contains("btn-export-mermaid"));
        assert!(html.contains("btn-theme"));
        assert!(html.contains("report-data"));
        let mmd = fs::read_to_string(dir.join("m.mmd")).unwrap();
        assert!(mmd.contains("flowchart"));
        let _ = fs::remove_dir_all(&dir);
    }

    fn uuid_stub() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    }
}
