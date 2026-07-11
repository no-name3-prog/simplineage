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

impl DependencyKind {
    /// Canonical wire / storage label (`view_definition`, `pipeline`, …).
    ///
    /// `Other` values are emitted as `other:{name}`.
    #[must_use]
    pub fn as_str(&self) -> std::borrow::Cow<'_, str> {
        match self {
            Self::ViewDefinition => "view_definition".into(),
            Self::Pipeline => "pipeline".into(),
            Self::ForeignKey => "foreign_key".into(),
            Self::Manual => "manual".into(),
            Self::Inferred => "inferred".into(),
            Self::Other(s) => std::borrow::Cow::Owned(format!("other:{s}")),
        }
    }

    /// Parse a storage or import label into a kind.
    ///
    /// Accepts canonical names plus common aliases (`view`, `etl`, `fk`, …).
    #[must_use]
    pub fn parse(s: &str) -> Self {
        let t = s.trim();
        if let Some(rest) = t.strip_prefix("other:") {
            return Self::Other(rest.to_string());
        }
        match t.to_ascii_lowercase().as_str() {
            "view_definition" | "view" => Self::ViewDefinition,
            "pipeline" | "etl" | "dbt_model" => Self::Pipeline,
            "foreign_key" | "fk" | "foreign key" => Self::ForeignKey,
            "manual" => Self::Manual,
            "inferred" => Self::Inferred,
            "" => Self::Inferred,
            other => Self::Other(other.to_string()),
        }
    }
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

impl DependencyLevel {
    /// Canonical wire / storage label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Relation => "relation",
            Self::Column => "column",
            Self::Unknown => "unknown",
        }
    }

    /// Parse a storage or import label.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "relation" | "table" => Self::Relation,
            "column" => Self::Column,
            _ => Self::Unknown,
        }
    }
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

    #[test]
    fn dependency_kind_parse_roundtrip() {
        for k in [
            DependencyKind::ViewDefinition,
            DependencyKind::Pipeline,
            DependencyKind::ForeignKey,
            DependencyKind::Manual,
            DependencyKind::Inferred,
            DependencyKind::Other("dbt".into()),
        ] {
            let wire = k.as_str();
            assert_eq!(DependencyKind::parse(&wire), k);
        }
        assert_eq!(DependencyKind::parse("etl"), DependencyKind::Pipeline);
        assert_eq!(DependencyKind::parse("fk"), DependencyKind::ForeignKey);
        assert_eq!(DependencyLevel::parse("column"), DependencyLevel::Column);
        assert_eq!(DependencyLevel::Relation.as_str(), "relation");
    }
}
