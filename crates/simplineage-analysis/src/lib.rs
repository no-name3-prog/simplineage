//! Lineage graph analysis for SimpLineage.
//!
//! # Modules
//!
//! - [`graph`] — directed lineage graph engine (Phase 3)
//! - [`engine`] — high-level analysis engine (Phase 4)
//! - [`reports`] — structured report types
//!
//! # Analysis engine (Phase 4)
//!
//! [`AnalysisEngine`] provides a clean API for:
//!
//! - impact analysis (upstream / downstream / both)
//! - dependency validation
//! - unused object detection
//! - orphan detection
//! - circular dependency detection
//! - critical table discovery
//! - longest dependency chains
//! - metadata quality checks
//!
//! # Example
//!
//! ```
//! use simplineage_analysis::{AnalysisEngine, ImpactDirection, ImpactOptions};
//! use simplineage_core::ObjectId;
//! use simplineage_core::Snapshot;
//! use simplineage_core::model::graph::{Dependency, DependencyKind, DependencyLevel};
//!
//! let mut snap = Snapshot::new();
//! snap.dependencies.push(Dependency {
//!     id: ObjectId::from_trusted("d1"),
//!     from_id: ObjectId::from_trusted("orders"),
//!     to_id: ObjectId::from_trusted("facts"),
//!     kind: DependencyKind::ViewDefinition,
//!     level: DependencyLevel::Relation,
//!     confidence: None,
//!     attributes: Default::default(),
//! });
//! let engine = AnalysisEngine::from_snapshot(snap);
//! let impact = engine
//!     .impact(
//!         &ObjectId::from_trusted("orders"),
//!         &ImpactOptions {
//!             direction: ImpactDirection::Downstream,
//!             ..Default::default()
//!         },
//!     )
//!     .unwrap();
//! assert_eq!(impact.downstream[0].as_str(), "facts");
//! let full = engine.analyze_all();
//! assert!(full.quality.score <= 100.0);
//! ```
//!
//! # Graph engine
//!
//! [`LineageGraph`] builds a directed dependency graph and supports low-level
//! traversal, paths, cycles, and statistics. See [`graph`].

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod engine;
pub mod error;
pub mod graph;
pub mod reports;

pub use engine::AnalysisEngine;
pub use error::{GraphError, Result};
pub use graph::{
    EdgeData, GraphStatistics, LineageGraph, TraversalOptions, downstream_impact, upstream_impact,
};
pub use reports::{
    ChainOptions, CircularDependency, CriticalObject, CriticalOptions, DependencyChain,
    DependencyValidationReport, FullAnalysisReport, ImpactDirection, ImpactOptions, ImpactReport,
    IsolatedObject, IsolationKind, QualityCheck, QualityReport, Severity, ValidationIssue,
};

/// High-level façade kept for compatibility; prefer [`AnalysisEngine`].
#[derive(Debug, Clone)]
pub struct Analyzer {
    engine: AnalysisEngine,
}

impl Default for Analyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer {
    /// Empty analyzer (empty snapshot / graph).
    #[must_use]
    pub fn new() -> Self {
        Self {
            engine: AnalysisEngine::from_snapshot(simplineage_core::Snapshot::new()),
        }
    }

    /// Build from a snapshot.
    #[must_use]
    pub fn from_snapshot(snapshot: &simplineage_core::Snapshot) -> Self {
        Self {
            engine: AnalysisEngine::from_snapshot(snapshot.clone()),
        }
    }

    /// Build from an owned snapshot.
    #[must_use]
    pub fn from_engine(engine: AnalysisEngine) -> Self {
        Self { engine }
    }

    /// Borrow the analysis engine.
    #[must_use]
    pub fn engine(&self) -> &AnalysisEngine {
        &self.engine
    }

    /// Borrow the underlying graph.
    #[must_use]
    pub fn graph(&self) -> &LineageGraph {
        self.engine.graph()
    }

    /// Downstream impact as strings.
    pub fn impact_downstream(&self, node_id: &str) -> Result<Vec<String>> {
        let id = simplineage_core::ObjectId::from_trusted(node_id);
        let report = self.engine.impact(
            &id,
            &ImpactOptions {
                direction: ImpactDirection::Downstream,
                ..Default::default()
            },
        )?;
        Ok(report
            .downstream
            .into_iter()
            .map(|o| o.to_string())
            .collect())
    }

    /// Upstream impact as strings.
    pub fn impact_upstream(&self, node_id: &str) -> Result<Vec<String>> {
        let id = simplineage_core::ObjectId::from_trusted(node_id);
        let report = self.engine.impact(
            &id,
            &ImpactOptions {
                direction: ImpactDirection::Upstream,
                ..Default::default()
            },
        )?;
        Ok(report.upstream.into_iter().map(|o| o.to_string()).collect())
    }

    /// Graph statistics.
    #[must_use]
    pub fn statistics(&self) -> GraphStatistics {
        self.engine.graph().statistics()
    }

    /// Cycle detection (string node ids).
    #[must_use]
    pub fn cycles(&self) -> Vec<Vec<String>> {
        self.engine
            .circular_dependencies()
            .into_iter()
            .map(|c| c.nodes.into_iter().map(|o| o.to_string()).collect())
            .collect()
    }

    /// Full Phase 4 analysis suite.
    #[must_use]
    pub fn analyze_all(&self) -> FullAnalysisReport {
        self.engine.analyze_all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use simplineage_core::ObjectId;
    use simplineage_core::model::graph::{Dependency, DependencyKind, DependencyLevel};

    #[test]
    fn analyzer_impact() {
        let mut snap = simplineage_core::Snapshot::new();
        snap.dependencies.push(Dependency {
            id: ObjectId::from_trusted("d1"),
            from_id: ObjectId::from_trusted("sales.orders"),
            to_id: ObjectId::from_trusted("mart.order_facts"),
            kind: DependencyKind::ViewDefinition,
            level: DependencyLevel::Relation,
            confidence: None,
            attributes: Default::default(),
        });
        let a = Analyzer::from_snapshot(&snap);
        let down = a.impact_downstream("sales.orders").unwrap();
        assert_eq!(down, vec!["mart.order_facts".to_string()]);
    }
}
