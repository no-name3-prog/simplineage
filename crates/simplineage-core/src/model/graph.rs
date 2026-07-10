//! Relationships (structural) and dependencies (lineage) between objects.

use serde::{Deserialize, Serialize};

use super::ids::ObjectId;
use super::types::Attributes;

/// Structural or logical relationship between metadata objects.
///
/// Examples: foreign key, “view selects from table”, ownership. Prefer
/// [`Dependency`] for directed lineage edges used by impact analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relationship {
    /// Stable id for this relationship edge.
    pub id: ObjectId,
    /// Relationship kind (vendor-neutral).
    pub kind: RelationshipKind,
    /// Source object id.
    pub from_id: ObjectId,
    /// Target object id.
    pub to_id: ObjectId,
    /// Optional ordered column mappings (e.g. FK column pairs).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub column_mappings: Vec<ColumnMapping>,
    /// Optional name (constraint name, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Extensible attributes.
    #[serde(default, skip_serializing_if = "Attributes::is_empty")]
    pub attributes: Attributes,
}

/// Kind of structural relationship.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipKind {
    /// Foreign key (or analogous referential constraint).
    ForeignKey,
    /// Primary key membership is modeled on columns; this marks a PK constraint edge if needed.
    PrimaryKey,
    /// Unique constraint.
    Unique,
    /// Check constraint (expression in attributes).
    Check,
    /// Generic association when no better kind applies.
    Association,
    /// Extension point with a free-form name.
    Other(String),
}

/// Maps a column on the `from` side to a column on the `to` side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnMapping {
    /// Column id on the relationship source.
    pub from_column_id: ObjectId,
    /// Column id on the relationship target.
    pub to_column_id: ObjectId,
}

/// Directed dependency used for lineage (upstream → downstream data flow).
///
/// Convention: `from_id` is **upstream** (producer), `to_id` is **downstream**
/// (consumer). Example: `orders` → `order_facts` means `order_facts` depends on
/// `orders`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    /// Stable id for this dependency edge.
    pub id: ObjectId,
    /// Upstream object (provider).
    pub from_id: ObjectId,
    /// Downstream object (dependent).
    pub to_id: ObjectId,
    /// How the dependency was established.
    pub kind: DependencyKind,
    /// Granularity of the edge.
    #[serde(default)]
    pub level: DependencyLevel,
    /// Confidence in \[0, 1\] when inferred, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<OrderedFloatCompat>,
    /// Extensible attributes (SQL fragment, tool name, etc.).
    #[serde(default, skip_serializing_if = "Attributes::is_empty")]
    pub attributes: Attributes,
}

/// Dependency classification.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    /// Derived from view/MV definition.
    ViewDefinition,
    /// Derived from ETL / pipeline metadata.
    Pipeline,
    /// Derived from foreign key (also structural).
    ForeignKey,
    /// Explicit user-provided lineage.
    Manual,
    /// Inferred by analysis (heuristic).
    Inferred,
    /// Other / tool-specific.
    Other(String),
}

/// Lineage edge granularity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyLevel {
    /// Relation-level (table/view) dependency.
    #[default]
    Relation,
    /// Column-level dependency.
    Column,
    /// Mixed or unknown.
    Unknown,
}

/// Serde-friendly wrapper for a confidence score without depending on `ordered-float`.
///
/// Stored as `f64` in JSON; equality uses bit-level compare for tests.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OrderedFloatCompat(pub f64);

impl PartialEq for OrderedFloatCompat {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for OrderedFloatCompat {}

impl OrderedFloatCompat {
    /// Clamp to \[0, 1\] when building from a raw score.
    #[must_use]
    pub fn clamped(value: f64) -> Self {
        Self(value.clamp(0.0, 1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_json_roundtrip() {
        let d = Dependency {
            id: ObjectId::from_trusted("d1"),
            from_id: ObjectId::from_trusted("orders"),
            to_id: ObjectId::from_trusted("order_facts"),
            kind: DependencyKind::ViewDefinition,
            level: DependencyLevel::Relation,
            confidence: Some(OrderedFloatCompat::clamped(1.0)),
            attributes: Attributes::new(),
        };
        let json = serde_json::to_string(&d).unwrap();
        let back: Dependency = serde_json::from_str(&json).unwrap();
        assert_eq!(d, back);
    }
}
