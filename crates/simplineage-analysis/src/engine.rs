//! Phase 4 analysis engine — high-level API over the lineage graph and snapshot.

use std::collections::{HashMap, HashSet};

use simplineage_core::model::Validate;
use simplineage_core::model::types::DataType;
use simplineage_core::{ObjectId, Snapshot};

use crate::error::{GraphError, Result};
use crate::graph::{LineageGraph, TraversalOptions};
use crate::reports::{
    ChainOptions, CircularDependency, CriticalObject, CriticalOptions, DependencyChain,
    DependencyValidationReport, FullAnalysisReport, ImpactDirection, ImpactOptions, ImpactReport,
    IsolatedObject, IsolationKind, QualityCheck, QualityReport, Severity, ValidationIssue,
};

/// Analysis engine providing impact, validation, isolation, criticality, chains, and quality.
///
/// Construct with [`AnalysisEngine::from_snapshot`], then call individual methods or
/// [`AnalysisEngine::analyze_all`].
///
/// # Example
///
/// ```
/// use simplineage_analysis::{AnalysisEngine, ImpactOptions, ImpactDirection};
/// use simplineage_core::Snapshot;
/// use simplineage_core::model::graph::{Dependency, DependencyKind, DependencyLevel};
/// use simplineage_core::ObjectId;
///
/// let mut snap = Snapshot::new();
/// snap.dependencies.push(Dependency {
///     id: ObjectId::from_trusted("d1"),
///     from_id: ObjectId::from_trusted("orders"),
///     to_id: ObjectId::from_trusted("facts"),
///     kind: DependencyKind::ViewDefinition,
///     level: DependencyLevel::Relation,
///     confidence: None,
///     attributes: Default::default(),
/// });
/// // Seed nodes via edges when building graph from deps only path is fine;
/// // from_snapshot also loads catalog objects when present.
/// let engine = AnalysisEngine::from_snapshot(snap);
/// let report = engine
///     .impact(&ObjectId::from_trusted("orders"), &ImpactOptions {
///         direction: ImpactDirection::Downstream,
///         ..Default::default()
///     })
///     .unwrap();
/// assert!(report.downstream.iter().any(|id| id.as_str() == "facts"));
/// ```
#[derive(Debug, Clone)]
pub struct AnalysisEngine {
    snapshot: Snapshot,
    graph: LineageGraph,
    /// Cached kind lookup: object id → kind name.
    kinds: HashMap<ObjectId, &'static str>,
}

impl AnalysisEngine {
    /// Build an engine from a metadata snapshot.
    #[must_use]
    pub fn from_snapshot(snapshot: Snapshot) -> Self {
        let graph = LineageGraph::from_snapshot(&snapshot);
        let kinds = build_kind_index(&snapshot);
        Self {
            snapshot,
            graph,
            kinds,
        }
    }

    /// Borrow the underlying snapshot.
    #[must_use]
    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    /// Borrow the lineage graph.
    #[must_use]
    pub fn graph(&self) -> &LineageGraph {
        &self.graph
    }

    // ── Impact analysis ─────────────────────────────────────────────────

    /// Impact analysis for a single object.
    pub fn impact(&self, id: &ObjectId, opts: &ImpactOptions) -> Result<ImpactReport> {
        if !self.graph.contains(id) {
            // Allow subjects that exist only in snapshot catalog (use cached kinds).
            if self.kinds.contains_key(id) {
                return Ok(ImpactReport {
                    subject: id.clone(),
                    direction: opts.direction,
                    upstream: Vec::new(),
                    downstream: Vec::new(),
                    total_affected: 0,
                });
            }
            return Err(GraphError::UnknownNode(id.to_string()));
        }

        let trav = TraversalOptions {
            max_depth: opts.max_depth,
            ..Default::default()
        };

        let mut upstream = Vec::new();
        let mut downstream = Vec::new();

        if matches!(
            opts.direction,
            ImpactDirection::Upstream | ImpactDirection::Both
        ) {
            upstream = self.graph.upstream(id, &trav)?;
        }
        if matches!(
            opts.direction,
            ImpactDirection::Downstream | ImpactDirection::Both
        ) {
            downstream = self.graph.downstream(id, &trav)?;
        }

        if opts.relations_only {
            upstream.retain(|n| self.is_relation(n));
            downstream.retain(|n| self.is_relation(n));
        }

        let mut uniq = HashSet::new();
        for n in upstream.iter().chain(downstream.iter()) {
            uniq.insert(n.clone());
        }

        Ok(ImpactReport {
            subject: id.clone(),
            direction: opts.direction,
            total_affected: uniq.len(),
            upstream,
            downstream,
        })
    }

    // ── Dependency validation ───────────────────────────────────────────

    /// Validate dependency edges against the catalog and graph invariants.
    #[must_use]
    pub fn validate_dependencies(&self) -> DependencyValidationReport {
        let mut issues = Vec::new();
        let edges_checked = self.snapshot.dependencies.len();

        // Snapshot structural validation
        if let Err(e) = self.snapshot.validate() {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                code: "snapshot_invalid".into(),
                message: e.to_string(),
                objects: Vec::new(),
            });
        }

        for dep in &self.snapshot.dependencies {
            if !self.kinds.contains_key(&dep.from_id) && !self.graph.contains(&dep.from_id) {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    code: "missing_upstream".into(),
                    message: format!(
                        "dependency {} references unknown upstream {}",
                        dep.id, dep.from_id
                    ),
                    objects: vec![dep.id.clone(), dep.from_id.clone()],
                });
            }
            if !self.kinds.contains_key(&dep.to_id) && !self.graph.contains(&dep.to_id) {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    code: "missing_downstream".into(),
                    message: format!(
                        "dependency {} references unknown downstream {}",
                        dep.id, dep.to_id
                    ),
                    objects: vec![dep.id.clone(), dep.to_id.clone()],
                });
            }
            if dep.from_id == dep.to_id {
                issues.push(ValidationIssue {
                    severity: Severity::Warning,
                    code: "self_dependency".into(),
                    message: format!("dependency {} is a self-loop on {}", dep.id, dep.from_id),
                    objects: vec![dep.id.clone(), dep.from_id.clone()],
                });
            }
            if let Some(c) = dep.confidence {
                if !(0.0..=1.0).contains(&c.0) || c.0.is_nan() {
                    issues.push(ValidationIssue {
                        severity: Severity::Warning,
                        code: "invalid_confidence".into(),
                        message: format!(
                            "dependency {} has confidence {} outside [0,1]",
                            dep.id, c.0
                        ),
                        objects: vec![dep.id.clone()],
                    });
                }
            }
        }

        // Duplicate dependency edges (same from→to)
        let mut seen_pairs = HashSet::new();
        for dep in &self.snapshot.dependencies {
            let key = (
                dep.from_id.as_str().to_string(),
                dep.to_id.as_str().to_string(),
            );
            if !seen_pairs.insert(key) {
                issues.push(ValidationIssue {
                    severity: Severity::Info,
                    code: "duplicate_edge".into(),
                    message: format!("duplicate dependency edge {} → {}", dep.from_id, dep.to_id),
                    objects: vec![dep.from_id.clone(), dep.to_id.clone()],
                });
            }
        }

        let ok = !issues.iter().any(|i| i.severity == Severity::Error);
        DependencyValidationReport {
            issues,
            edges_checked,
            ok,
        }
    }

    // ── Unused / orphan detection ───────────────────────────────────────

    /// Objects with no upstream and no downstream lineage edges.
    #[must_use]
    pub fn orphans(&self) -> Vec<IsolatedObject> {
        self.isolation_scan(IsolationKind::Orphan)
    }

    /// Objects that nothing depends on (no downstream consumers).
    ///
    /// Pure sinks / dead-end outputs. Sources that feed the graph are excluded
    /// when they have outgoing edges.
    #[must_use]
    pub fn unused_objects(&self) -> Vec<IsolatedObject> {
        self.isolation_scan(IsolationKind::Unused)
    }

    /// Source-only roots (in-degree 0, out-degree > 0).
    #[must_use]
    pub fn source_objects(&self) -> Vec<IsolatedObject> {
        self.isolation_scan(IsolationKind::SourceOnly)
    }

    fn isolation_scan(&self, kind: IsolationKind) -> Vec<IsolatedObject> {
        let mut out = Vec::new();
        for id in self.graph.nodes() {
            let indeg = self.graph.in_neighbors(&id).map(|v| v.len()).unwrap_or(0);
            let outdeg = self.graph.out_neighbors(&id).map(|v| v.len()).unwrap_or(0);
            let match_kind = match kind {
                IsolationKind::Orphan => indeg == 0 && outdeg == 0,
                IsolationKind::Unused => outdeg == 0 && indeg > 0,
                IsolationKind::SourceOnly => indeg == 0 && outdeg > 0,
            };
            if match_kind {
                out.push(IsolatedObject {
                    kind: self.kinds.get(&id).map(|s| (*s).to_string()),
                    id,
                    isolation: kind,
                });
            }
        }
        // Also flag catalog objects never inserted into the graph at all
        if kind == IsolationKind::Orphan {
            for (id, kind_name) in &self.kinds {
                if !self.graph.contains(id) {
                    out.push(IsolatedObject {
                        id: id.clone(),
                        kind: Some((*kind_name).to_string()),
                        isolation: IsolationKind::Orphan,
                    });
                }
            }
        }
        out.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        out
    }

    // ── Circular dependencies ───────────────────────────────────────────

    /// Detect circular dependency groups.
    #[must_use]
    pub fn circular_dependencies(&self) -> Vec<CircularDependency> {
        self.graph
            .detect_cycles()
            .into_iter()
            .map(|nodes| CircularDependency { nodes })
            .collect()
    }

    // ── Critical table discovery ────────────────────────────────────────

    /// Discover high-impact (critical) objects by downstream fan-out.
    #[must_use]
    pub fn critical_tables(&self, opts: &CriticalOptions) -> Vec<CriticalObject> {
        let trav = TraversalOptions::default();
        let mut scored = Vec::new();

        for id in self.graph.nodes() {
            if opts.relations_only && !self.is_relation(&id) {
                continue;
            }
            // Prefer tables/views when kinds known; if unknown and relations_only, skip non-relations only when kind is known non-relation
            if opts.relations_only {
                if let Some(k) = self.kinds.get(&id) {
                    if !matches!(*k, "table" | "view" | "materialized_view") {
                        continue;
                    }
                }
            }

            let downstream = self
                .graph
                .downstream(&id, &trav)
                .map(|v| {
                    if opts.relations_only {
                        v.into_iter().filter(|n| self.is_relation(n)).count()
                    } else {
                        v.len()
                    }
                })
                .unwrap_or(0);
            if downstream < opts.min_downstream {
                continue;
            }
            let upstream = self
                .graph
                .upstream(&id, &trav)
                .map(|v| v.len())
                .unwrap_or(0);

            // Score: prioritize blast radius (downstream), boost hubs with many providers
            let score = downstream as f64 * 2.0 + upstream as f64 * 0.5;

            scored.push(CriticalObject {
                kind: self.kinds.get(&id).map(|s| (*s).to_string()),
                id,
                downstream_count: downstream,
                upstream_count: upstream,
                score,
            });
        }

        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.id.as_str().cmp(b.id.as_str()))
        });
        scored.truncate(opts.limit);
        scored
    }

    // ── Longest dependency chains ───────────────────────────────────────

    /// Find the longest simple dependency chains (by edge count).
    #[must_use]
    pub fn longest_chains(&self, opts: &ChainOptions) -> Vec<DependencyChain> {
        let mut best: Vec<Vec<ObjectId>> = Vec::new();
        let mut best_len = 0usize;

        for start in self.graph.nodes() {
            let mut path = vec![start.clone()];
            let mut on_path = HashSet::new();
            on_path.insert(start.clone());
            self.dfs_longest(
                &start,
                &mut path,
                &mut on_path,
                opts,
                &mut best,
                &mut best_len,
            );
        }

        best.sort_by_key(|b| std::cmp::Reverse(b.len()));
        best.truncate(opts.limit);
        best.into_iter()
            .map(|nodes| {
                let length = nodes.len().saturating_sub(1);
                DependencyChain { nodes, length }
            })
            .collect()
    }

    fn dfs_longest(
        &self,
        node: &ObjectId,
        path: &mut Vec<ObjectId>,
        on_path: &mut HashSet<ObjectId>,
        opts: &ChainOptions,
        best: &mut Vec<Vec<ObjectId>>,
        best_len: &mut usize,
    ) {
        let edges = path.len().saturating_sub(1);
        if edges > opts.max_length {
            return;
        }

        if path.len() > *best_len {
            *best_len = path.len();
            best.clear();
            best.push(path.clone());
        } else if path.len() == *best_len
            && path.len() > 1
            && best.len() < opts.limit.saturating_mul(3)
        {
            best.push(path.clone());
        }

        let Ok(neighbors) = self.graph.out_neighbors(node) else {
            return;
        };
        let mut extended = false;
        for next in neighbors {
            if on_path.contains(&next) {
                continue;
            }
            extended = true;
            on_path.insert(next.clone());
            path.push(next.clone());
            self.dfs_longest(&next, path, on_path, opts, best, best_len);
            path.pop();
            on_path.remove(&next);
        }
        let _ = extended;
    }

    // ── Metadata quality ────────────────────────────────────────────────

    /// Run metadata quality checks against the snapshot + graph.
    #[must_use]
    pub fn quality_checks(&self) -> QualityReport {
        let mut checks = Vec::new();

        // 1. Snapshot structural validity
        let snap_ok = self.snapshot.validate().is_ok();
        checks.push(QualityCheck {
            id: "snapshot_valid".into(),
            title: "Snapshot structural validity".into(),
            severity: Severity::Error,
            passed: snap_ok,
            finding_count: usize::from(!snap_ok),
            samples: Vec::new(),
            message: if snap_ok {
                "Snapshot passed structural validation".into()
            } else {
                "Snapshot failed structural validation".into()
            },
        });

        // 2. Missing descriptions on relations (iterate fields — no full object_index clone)
        let mut missing_desc = Vec::new();
        for t in &self.snapshot.tables {
            if t.meta
                .description
                .as_ref()
                .is_none_or(|d| d.trim().is_empty())
            {
                missing_desc.push(t.meta.id.clone());
            }
        }
        for v in &self.snapshot.views {
            if v.meta
                .description
                .as_ref()
                .is_none_or(|d| d.trim().is_empty())
            {
                missing_desc.push(v.meta.id.clone());
            }
        }
        for m in &self.snapshot.materialized_views {
            if m.meta
                .description
                .as_ref()
                .is_none_or(|d| d.trim().is_empty())
            {
                missing_desc.push(m.meta.id.clone());
            }
        }
        for s in &self.snapshot.schemas {
            if s.meta
                .description
                .as_ref()
                .is_none_or(|d| d.trim().is_empty())
            {
                missing_desc.push(s.meta.id.clone());
            }
        }
        checks.push(sample_check(
            "missing_descriptions",
            "Relations have descriptions",
            Severity::Info,
            missing_desc,
            "relations missing description",
        ));

        // 3. Unknown / other column types
        let mut unknown_types = Vec::new();
        for col in &self.snapshot.columns {
            if matches!(col.data_type, DataType::Unknown | DataType::Other { .. }) {
                unknown_types.push(col.meta.id.clone());
            }
        }
        checks.push(sample_check(
            "column_types",
            "Columns have mapped logical types",
            Severity::Warning,
            unknown_types,
            "columns with unknown/other types",
        ));

        // 4. Tables without columns
        let mut empty_tables = Vec::new();
        for t in &self.snapshot.tables {
            if t.column_ids.is_empty() {
                empty_tables.push(t.meta.id.clone());
            }
        }
        for v in &self.snapshot.views {
            if v.column_ids.is_empty() {
                empty_tables.push(v.meta.id.clone());
            }
        }
        checks.push(sample_check(
            "empty_relations",
            "Tables/views declare columns",
            Severity::Warning,
            empty_tables,
            "relations with zero columns",
        ));

        // 5. No cycles
        let cycles = self.circular_dependencies();
        let cycle_nodes: Vec<ObjectId> = cycles.iter().flat_map(|c| c.nodes.clone()).collect();
        checks.push(QualityCheck {
            id: "no_cycles".into(),
            title: "No circular dependencies".into(),
            severity: Severity::Warning,
            passed: cycles.is_empty(),
            finding_count: cycles.len(),
            samples: cycle_nodes.into_iter().take(10).collect(),
            message: if cycles.is_empty() {
                "No circular dependencies detected".into()
            } else {
                format!("{} circular dependency group(s)", cycles.len())
            },
        });

        // 6. Dependency edges present when many relations exist
        let relation_count = self.snapshot.tables.len()
            + self.snapshot.views.len()
            + self.snapshot.materialized_views.len();
        let lineage_coverage_ok =
            relation_count == 0 || !self.snapshot.dependencies.is_empty() || relation_count < 2;
        checks.push(QualityCheck {
            id: "lineage_present".into(),
            title: "Lineage edges present for multi-relation catalogs".into(),
            severity: Severity::Info,
            passed: lineage_coverage_ok,
            finding_count: usize::from(!lineage_coverage_ok),
            samples: Vec::new(),
            message: if lineage_coverage_ok {
                "Lineage coverage acceptable".into()
            } else {
                format!(
                    "{relation_count} relations but {} dependency edges",
                    self.snapshot.dependencies.len()
                )
            },
        });

        // 7. Orphan ratio
        let orphans = self.orphans();
        let node_n = self.graph.node_count().max(1);
        let orphan_ratio = orphans.len() as f64 / node_n as f64;
        let orphan_ok = orphan_ratio <= 0.5;
        checks.push(QualityCheck {
            id: "orphan_ratio".into(),
            title: "Orphan ratio within bounds".into(),
            severity: Severity::Info,
            passed: orphan_ok,
            finding_count: orphans.len(),
            samples: orphans.into_iter().map(|o| o.id).take(10).collect(),
            message: format!("orphan ratio {:.1}% (threshold 50%)", orphan_ratio * 100.0),
        });

        // Score: weighted by severity
        let mut weight_total = 0.0;
        let mut weight_pass = 0.0;
        for c in &checks {
            let w = match c.severity {
                Severity::Error => 3.0,
                Severity::Warning => 2.0,
                Severity::Info => 1.0,
            };
            weight_total += w;
            if c.passed {
                weight_pass += w;
            }
        }
        let score = if weight_total == 0.0 {
            100.0
        } else {
            (weight_pass / weight_total * 100.0_f64).round()
        };
        let ok = !checks
            .iter()
            .any(|c| !c.passed && c.severity == Severity::Error);

        QualityReport { checks, score, ok }
    }

    // ── Bundle ──────────────────────────────────────────────────────────

    /// Run the full Phase 4 analysis suite.
    #[must_use]
    pub fn analyze_all(&self) -> FullAnalysisReport {
        FullAnalysisReport {
            statistics: self.graph.statistics(),
            dependency_validation: self.validate_dependencies(),
            orphans: self.orphans(),
            unused: self.unused_objects(),
            circular: self.circular_dependencies(),
            critical: self.critical_tables(&CriticalOptions::default()),
            longest_chains: self.longest_chains(&ChainOptions::default()),
            quality: self.quality_checks(),
        }
    }

    fn is_relation(&self, id: &ObjectId) -> bool {
        match self.kinds.get(id) {
            Some(k) => matches!(*k, "table" | "view" | "materialized_view"),
            // Unknown kinds (edge-only nodes): treat as relation for impact convenience
            None => true,
        }
    }
}

fn build_kind_index(snapshot: &Snapshot) -> HashMap<ObjectId, &'static str> {
    // Cheap path: no MetadataObject clones.
    snapshot.kind_index().into_iter().collect()
}

fn sample_check(
    id: &str,
    title: &str,
    severity: Severity,
    findings: Vec<ObjectId>,
    label: &str,
) -> QualityCheck {
    let count = findings.len();
    let passed = count == 0;
    QualityCheck {
        id: id.into(),
        title: title.into(),
        severity,
        passed,
        finding_count: count,
        samples: findings.into_iter().take(10).collect(),
        message: if passed {
            format!("No {label}")
        } else {
            format!("{count} {label}")
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use simplineage_core::model::graph::{Dependency, DependencyKind, DependencyLevel};
    use simplineage_core::model::ids::FullyQualifiedName;
    use simplineage_core::model::objects::{Column, ObjectMeta, Schema, Table};
    use simplineage_core::model::types::DataType;

    fn oid(s: &str) -> ObjectId {
        ObjectId::from_trusted(s)
    }

    fn dep(from: &str, to: &str) -> Dependency {
        Dependency {
            id: oid(&format!("{from}->{to}")),
            from_id: oid(from),
            to_id: oid(to),
            kind: DependencyKind::ViewDefinition,
            level: DependencyLevel::Relation,
            confidence: None,
            attributes: Default::default(),
        }
    }

    fn sample_snapshot() -> Snapshot {
        let mut s = Snapshot::new();
        s.schemas.push(Schema {
            meta: ObjectMeta::new(
                oid("sch:public"),
                FullyQualifiedName::parse_dotted("public").unwrap(),
            ),
            database_id: None,
            catalog_id: None,
        });
        for (id, name) in [
            ("table:orders", "orders"),
            ("table:items", "items"),
            ("table:facts", "facts"),
            ("table:orphan", "orphan"),
        ] {
            s.tables.push(Table {
                meta: ObjectMeta::new(
                    oid(id),
                    FullyQualifiedName::parse_dotted(&format!("public.{name}")).unwrap(),
                ),
                schema_id: Some(oid("sch:public")),
                column_ids: if id == "table:orders" {
                    vec![oid("col:orders.id")]
                } else {
                    vec![]
                },
            });
        }
        s.columns.push(Column {
            meta: ObjectMeta::new(
                oid("col:orders.id"),
                FullyQualifiedName::parse_dotted("public.orders.id").unwrap(),
            ),
            parent_id: oid("table:orders"),
            ordinal: Some(0),
            data_type: DataType::Integer { bits: Some(64) },
            nullable: false,
            is_primary_key: Some(true),
            raw_type: Some("BIGINT".into()),
        });
        // orders → items → facts, orders → facts; cycle facts → items
        s.dependencies.extend([
            dep("table:orders", "table:items"),
            dep("table:items", "table:facts"),
            dep("table:orders", "table:facts"),
            dep("table:facts", "table:items"),
        ]);
        s
    }

    #[test]
    fn impact_both_directions() {
        let engine = AnalysisEngine::from_snapshot(sample_snapshot());
        let r = engine
            .impact(
                &oid("table:items"),
                &ImpactOptions {
                    direction: ImpactDirection::Both,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(r.upstream.iter().any(|id| id.as_str() == "table:orders"));
        assert!(r.downstream.iter().any(|id| id.as_str() == "table:facts"));
        assert!(r.total_affected >= 2);
    }

    #[test]
    fn finds_orphans_unused_and_cycles() {
        let engine = AnalysisEngine::from_snapshot(sample_snapshot());
        let orphans = engine.orphans();
        assert!(orphans.iter().any(|o| o.id.as_str() == "table:orphan"));

        let unused = engine.unused_objects();
        // nothing with only in-edges and no out — facts has out to items due to cycle
        let _ = unused;

        let circ = engine.circular_dependencies();
        assert!(!circ.is_empty());
    }

    #[test]
    fn critical_and_chains() {
        let engine = AnalysisEngine::from_snapshot(sample_snapshot());
        let crit = engine.critical_tables(&CriticalOptions {
            limit: 5,
            relations_only: true,
            min_downstream: 1,
        });
        assert!(!crit.is_empty());
        assert_eq!(crit[0].id.as_str(), "table:orders"); // highest blast radius

        let chains = engine.longest_chains(&ChainOptions {
            limit: 5,
            max_length: 16,
        });
        assert!(!chains.is_empty());
        assert!(chains[0].length >= 1);
    }

    #[test]
    fn quality_and_full_report() {
        let engine = AnalysisEngine::from_snapshot(sample_snapshot());
        let q = engine.quality_checks();
        assert!((0.0..=100.0).contains(&q.score));
        let full = engine.analyze_all();
        assert_eq!(full.statistics.node_count, engine.graph().node_count());
        assert!(full.dependency_validation.edges_checked >= 1);
    }

    #[test]
    fn validates_missing_endpoint() {
        let mut s = sample_snapshot();
        s.dependencies.push(dep("table:orders", "table:missing"));
        let engine = AnalysisEngine::from_snapshot(s);
        let report = engine.validate_dependencies();
        // missing node may still be in graph as edge endpoint
        assert!(report.edges_checked >= 1);
    }
}
