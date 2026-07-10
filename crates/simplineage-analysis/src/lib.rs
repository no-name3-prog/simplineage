//! Lineage graph analysis for SimpLineage.
//!
//! # Graph engine
//!
//! [`LineageGraph`] builds a directed dependency graph from a metadata
//! [`Snapshot`](simplineage_core::Snapshot) (or raw dependencies) and supports:
//!
//! - upstream / downstream traversal
//! - shortest path
//! - all simple dependency paths
//! - cycle detection
//! - graph statistics
//!
//! Traversals use BFS on compact adjacency lists (via petgraph) for
//! high-performance exploration of large lineage graphs.
//!
//! # Example
//!
//! ```
//! use simplineage_analysis::{LineageGraph, TraversalOptions};
//! use simplineage_core::model::graph::{Dependency, DependencyKind, DependencyLevel};
//! use simplineage_core::model::ids::ObjectId;
//!
//! let deps = [Dependency {
//!     id: ObjectId::from_trusted("e1"),
//!     from_id: ObjectId::from_trusted("orders"),
//!     to_id: ObjectId::from_trusted("order_facts"),
//!     kind: DependencyKind::ViewDefinition,
//!     level: DependencyLevel::Relation,
//!     confidence: None,
//!     attributes: Default::default(),
//! }];
//! let g = LineageGraph::from_dependencies(deps);
//! let down = g
//!     .downstream(&ObjectId::from_trusted("orders"), &TraversalOptions::default())
//!     .unwrap();
//! assert_eq!(down[0].as_str(), "order_facts");
//! ```

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod error;
pub mod graph;

pub use error::{GraphError, Result};
pub use graph::{
    EdgeData, GraphStatistics, LineageGraph, TraversalOptions, downstream_impact, upstream_impact,
};

/// High-level analyzer façade over [`LineageGraph`].
#[derive(Debug, Clone)]
pub struct Analyzer {
    graph: LineageGraph,
}

impl Default for Analyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer {
    /// Empty analyzer (no graph loaded).
    #[must_use]
    pub fn new() -> Self {
        Self {
            graph: LineageGraph::new(),
        }
    }

    /// Build from a snapshot.
    #[must_use]
    pub fn from_snapshot(snapshot: &simplineage_core::Snapshot) -> Self {
        Self {
            graph: LineageGraph::from_snapshot(snapshot),
        }
    }

    /// Borrow the underlying graph.
    #[must_use]
    pub fn graph(&self) -> &LineageGraph {
        &self.graph
    }

    /// Mutable access to the graph.
    pub fn graph_mut(&mut self) -> &mut LineageGraph {
        &mut self.graph
    }

    /// Downstream impact: all dependents of `node_id` (string form of [`simplineage_core::ObjectId`]).
    pub fn impact_downstream(&self, node_id: &str) -> Result<Vec<String>> {
        let id = simplineage_core::ObjectId::from_trusted(node_id);
        Ok(self
            .graph
            .downstream(&id, &TraversalOptions::default())?
            .into_iter()
            .map(|o| o.to_string())
            .collect())
    }

    /// Upstream impact: all providers of `node_id`.
    pub fn impact_upstream(&self, node_id: &str) -> Result<Vec<String>> {
        let id = simplineage_core::ObjectId::from_trusted(node_id);
        Ok(self
            .graph
            .upstream(&id, &TraversalOptions::default())?
            .into_iter()
            .map(|o| o.to_string())
            .collect())
    }

    /// Graph statistics.
    #[must_use]
    pub fn statistics(&self) -> GraphStatistics {
        self.graph.statistics()
    }

    /// Cycle detection.
    #[must_use]
    pub fn cycles(&self) -> Vec<Vec<String>> {
        self.graph
            .detect_cycles()
            .into_iter()
            .map(|c| c.into_iter().map(|o| o.to_string()).collect())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use simplineage_core::ObjectId;
    use simplineage_core::model::graph::{Dependency, DependencyKind, DependencyLevel};

    #[test]
    fn analyzer_impact() {
        let g = LineageGraph::from_dependencies([Dependency {
            id: ObjectId::from_trusted("d1"),
            from_id: ObjectId::from_trusted("sales.orders"),
            to_id: ObjectId::from_trusted("mart.order_facts"),
            kind: DependencyKind::ViewDefinition,
            level: DependencyLevel::Relation,
            confidence: None,
            attributes: Default::default(),
        }]);
        let a = Analyzer { graph: g };
        let down = a.impact_downstream("sales.orders").unwrap();
        assert_eq!(down, vec!["mart.order_facts".to_string()]);
    }
}
