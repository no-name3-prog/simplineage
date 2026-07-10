//! Mermaid flowchart export for lineage graphs.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use simplineage_core::{Error, Result, Snapshot};

use crate::html::{ReportData, build_report_data};

/// Options controlling Mermaid diagram generation.
#[derive(Debug, Clone)]
pub struct MermaidOptions {
    /// Prefer relation nodes (table/view/MV) when true.
    pub relations_only: bool,
    /// Maximum nodes to emit (diagram readability). `None` = unlimited.
    pub max_nodes: Option<usize>,
    /// Flowchart direction: `LR` or `TD`.
    pub direction: String,
}

impl Default for MermaidOptions {
    fn default() -> Self {
        Self {
            relations_only: true,
            max_nodes: Some(200),
            direction: "LR".into(),
        }
    }
}

/// Render a Mermaid `flowchart` document from a snapshot.
#[must_use]
pub fn render_mermaid(snapshot: &Snapshot, opts: &MermaidOptions) -> String {
    render_mermaid_from_data(&build_report_data(snapshot), opts)
}

/// Render Mermaid from prebuilt report data.
#[must_use]
pub fn render_mermaid_from_data(data: &ReportData, opts: &MermaidOptions) -> String {
    let dir =
        if opts.direction.eq_ignore_ascii_case("TD") || opts.direction.eq_ignore_ascii_case("TB") {
            "TD"
        } else {
            "LR"
        };

    let mut nodes: Vec<_> = data.nodes.iter().collect();
    if opts.relations_only {
        nodes.retain(|n| {
            matches!(
                n.kind.as_str(),
                "table" | "view" | "materialized_view" | "unknown"
            )
        });
    }
    nodes.sort_by(|a, b| a.fqn.cmp(&b.fqn));
    if let Some(max) = opts.max_nodes {
        nodes.truncate(max);
    }
    let id_set: BTreeSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();

    let mut out = String::new();
    out.push_str(&format!(
        "%%{{init: {{'theme':'neutral'}}}}%%\nflowchart {dir}\n"
    ));
    out.push_str("%% Generated offline by SimpLineage\n");

    for n in &nodes {
        let sid = mermaid_id(&n.id);
        let label = mermaid_label(&format!("{}\\n{}", n.kind, n.fqn));
        out.push_str(&format!("  {sid}[\"{label}\"]\n"));
    }

    for e in &data.edges {
        if id_set.contains(e.from.as_str()) && id_set.contains(e.to.as_str()) {
            out.push_str(&format!(
                "  {} -->|{}| {}\n",
                mermaid_id(&e.from),
                mermaid_edge_label(&e.kind),
                mermaid_id(&e.to)
            ));
        }
    }
    out
}

/// Write Mermaid text to `path`.
pub fn write_mermaid(snapshot: &Snapshot, path: &Path, opts: &MermaidOptions) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(Error::Io)?;
        }
    }
    let text = render_mermaid(snapshot, opts);
    fs::write(path, text).map_err(Error::Io)?;
    Ok(())
}

fn mermaid_id(id: &str) -> String {
    let mut s: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if s.is_empty() || s.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        s.insert_str(0, "n_");
    }
    s
}

fn mermaid_label(s: &str) -> String {
    s.replace('"', "'").replace('[', "(").replace(']', ")")
}

fn mermaid_edge_label(s: &str) -> String {
    let t = s.replace('|', "/");
    if t.len() > 24 {
        format!("{}…", &t[..23])
    } else {
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use simplineage_core::ObjectId;
    use simplineage_core::model::graph::{Dependency, DependencyKind, DependencyLevel};
    use simplineage_core::model::ids::FullyQualifiedName;
    use simplineage_core::model::objects::{ObjectMeta, Table};

    #[test]
    fn mermaid_contains_nodes_and_edge() {
        let mut s = Snapshot::new();
        s.tables.push(Table {
            meta: ObjectMeta::new(
                ObjectId::from_trusted("t1"),
                FullyQualifiedName::parse_dotted("a.orders").unwrap(),
            ),
            schema_id: None,
            column_ids: vec![],
        });
        s.tables.push(Table {
            meta: ObjectMeta::new(
                ObjectId::from_trusted("t2"),
                FullyQualifiedName::parse_dotted("b.facts").unwrap(),
            ),
            schema_id: None,
            column_ids: vec![],
        });
        s.dependencies.push(Dependency {
            id: ObjectId::from_trusted("d1"),
            from_id: ObjectId::from_trusted("t1"),
            to_id: ObjectId::from_trusted("t2"),
            kind: DependencyKind::Pipeline,
            level: DependencyLevel::Relation,
            confidence: None,
            attributes: Default::default(),
        });
        let mmd = render_mermaid(&s, &MermaidOptions::default());
        assert!(mmd.contains("flowchart LR"));
        assert!(mmd.contains("t1"));
        assert!(mmd.contains("t2"));
        assert!(mmd.contains("-->"));
    }
}
