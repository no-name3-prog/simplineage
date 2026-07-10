//! Structured analysis report types.

use serde::{Deserialize, Serialize};
use simplineage_core::ObjectId;

use crate::graph::GraphStatistics;

/// Direction of impact / dependency traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpactDirection {
    /// Providers that feed into the subject (reverse edges).
    Upstream,
    /// Consumers that depend on the subject (forward edges).
    Downstream,
    /// Both directions.
    Both,
}

/// Options for impact analysis.
#[derive(Debug, Clone)]
pub struct ImpactOptions {
    /// Which direction(s) to explore.
    pub direction: ImpactDirection,
    /// Maximum hop depth (`None` = unlimited).
    pub max_depth: Option<usize>,
    /// Restrict results to tables / views / MVs only (exclude columns when known).
    pub relations_only: bool,
}

impl Default for ImpactOptions {
    fn default() -> Self {
        Self {
            direction: ImpactDirection::Downstream,
            max_depth: None,
            relations_only: false,
        }
    }
}

/// Result of impact analysis for one subject node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactReport {
    /// Subject node.
    pub subject: ObjectId,
    /// Direction analyzed.
    pub direction: ImpactDirection,
    /// Upstream ancestors (empty if direction is Downstream only).
    pub upstream: Vec<ObjectId>,
    /// Downstream descendants (empty if direction is Upstream only).
    pub downstream: Vec<ObjectId>,
    /// Total unique affected nodes (union of both sides, excluding subject).
    pub total_affected: usize,
}

/// Severity of a validation or quality finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Informational.
    Info,
    /// Should be fixed but not blocking.
    Warning,
    /// Likely incorrect metadata.
    Error,
}

/// A single dependency validation issue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    /// Severity.
    pub severity: Severity,
    /// Stable machine code (e.g. `missing_endpoint`).
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Related object ids when applicable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub objects: Vec<ObjectId>,
}

/// Report from dependency validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyValidationReport {
    /// Individual issues.
    pub issues: Vec<ValidationIssue>,
    /// Number of dependency edges examined.
    pub edges_checked: usize,
    /// True when no Error-severity issues.
    pub ok: bool,
}

/// Classification of unused / orphan findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IsolationKind {
    /// No upstream and no downstream edges (disconnected from lineage).
    Orphan,
    /// Has no downstream consumers (nothing reads it).
    Unused,
    /// Has no upstream providers (source / root).
    SourceOnly,
}

/// An object flagged by isolation analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IsolatedObject {
    /// Object id.
    pub id: ObjectId,
    /// Object kind when known (`table`, `view`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Why it was flagged.
    pub isolation: IsolationKind,
}

/// Circular dependency (strongly connected group or self-loop).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CircularDependency {
    /// Nodes participating in the cycle.
    pub nodes: Vec<ObjectId>,
}

/// A critical (high-impact) object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CriticalObject {
    /// Object id.
    pub id: ObjectId,
    /// Object kind when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Downstream dependent count (impact radius).
    pub downstream_count: usize,
    /// Upstream provider count.
    pub upstream_count: usize,
    /// Combined criticality score.
    pub score: f64,
}

/// Options for critical object discovery.
#[derive(Debug, Clone)]
pub struct CriticalOptions {
    /// Maximum results to return (sorted by score desc).
    pub limit: usize,
    /// Only consider tables / views / MVs.
    pub relations_only: bool,
    /// Minimum downstream count to qualify.
    pub min_downstream: usize,
}

impl Default for CriticalOptions {
    fn default() -> Self {
        Self {
            limit: 20,
            relations_only: true,
            min_downstream: 1,
        }
    }
}

/// A linear dependency chain (path).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyChain {
    /// Ordered node ids from root to leaf.
    pub nodes: Vec<ObjectId>,
    /// Number of edges (`nodes.len() - 1`).
    pub length: usize,
}

/// Options for longest-chain discovery.
#[derive(Debug, Clone)]
pub struct ChainOptions {
    /// How many longest chains to return.
    pub limit: usize,
    /// Cap DFS depth (edges) for performance.
    pub max_length: usize,
}

impl Default for ChainOptions {
    fn default() -> Self {
        Self {
            limit: 10,
            max_length: 128,
        }
    }
}

/// A metadata quality check result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityCheck {
    /// Stable check id.
    pub id: String,
    /// Short title.
    pub title: String,
    /// Severity of failures for this check.
    pub severity: Severity,
    /// Whether the check passed.
    pub passed: bool,
    /// Finding count (e.g. number of objects missing descriptions).
    pub finding_count: usize,
    /// Sample object ids (capped).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub samples: Vec<ObjectId>,
    /// Details.
    pub message: String,
}

/// Aggregate metadata quality report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityReport {
    /// Individual checks.
    pub checks: Vec<QualityCheck>,
    /// Score in \[0, 100\] (100 = all checks pass).
    pub score: f64,
    /// True when no Error-severity check failed.
    pub ok: bool,
}

/// Full analysis bundle (all Phase 4 reports).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FullAnalysisReport {
    /// Graph-level statistics.
    pub statistics: GraphStatistics,
    /// Dependency validation.
    pub dependency_validation: DependencyValidationReport,
    /// Orphan objects.
    pub orphans: Vec<IsolatedObject>,
    /// Unused objects (no consumers).
    pub unused: Vec<IsolatedObject>,
    /// Circular dependency groups.
    pub circular: Vec<CircularDependency>,
    /// Critical tables / relations.
    pub critical: Vec<CriticalObject>,
    /// Longest dependency chains.
    pub longest_chains: Vec<DependencyChain>,
    /// Metadata quality.
    pub quality: QualityReport,
}
