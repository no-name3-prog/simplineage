//! Identity types for metadata objects (vendor-agnostic).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Opaque stable identifier for a metadata object.
///
/// Assigned by importers/normalizers. Not necessarily a UUID; must be unique
/// within a [`crate::model::Snapshot`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ObjectId(String);

impl ObjectId {
    /// Create an object id from a non-empty string.
    pub fn new(id: impl Into<String>) -> Result<Self> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(Error::validation("object id must not be empty"));
        }
        Ok(Self(id))
    }

    /// Create without validation (for trusted internal construction).
    #[must_use]
    pub fn from_trusted(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Borrow the raw id string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ObjectId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Hierarchical fully-qualified name, independent of vendor quoting rules.
///
/// Components are ordered from outermost to innermost, e.g.
/// `["analytics", "warehouse", "public", "orders"]` for
/// catalog / database / schema / table — omit unused levels rather than
/// inventing vendor-specific placeholders.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FullyQualifiedName {
    /// Name path components (non-empty strings).
    pub parts: Vec<String>,
}

impl FullyQualifiedName {
    /// Build an FQN from path parts.
    pub fn new(parts: impl IntoIterator<Item = impl Into<String>>) -> Result<Self> {
        let parts: Vec<String> = parts.into_iter().map(Into::into).collect();
        Self::validate_parts(&parts)?;
        Ok(Self { parts })
    }

    /// Parse a dotted name (`a.b.c`). Dots inside quoted segments are not supported;
    /// importers should pass structured parts when names contain dots.
    pub fn parse_dotted(s: &str) -> Result<Self> {
        let parts: Vec<String> = s
            .split('.')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .collect();
        Self::new(parts)
    }

    fn validate_parts(parts: &[String]) -> Result<()> {
        if parts.is_empty() {
            return Err(Error::validation(
                "fully qualified name must have at least one part",
            ));
        }
        for (i, p) in parts.iter().enumerate() {
            if p.trim().is_empty() {
                return Err(Error::validation(format!(
                    "fully qualified name part {i} must not be empty"
                )));
            }
        }
        Ok(())
    }

    /// Number of path components.
    #[must_use]
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    /// Whether the path is empty (always false for validated FQNs).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// Leaf name (last component).
    #[must_use]
    pub fn leaf(&self) -> &str {
        self.parts.last().map(String::as_str).unwrap_or("")
    }

    /// Parent path without the leaf, if any.
    #[must_use]
    pub fn parent(&self) -> Option<FullyQualifiedName> {
        if self.parts.len() <= 1 {
            return None;
        }
        Some(FullyQualifiedName {
            parts: self.parts[..self.parts.len() - 1].to_vec(),
        })
    }

    /// Join with a child name segment.
    pub fn join(&self, child: impl Into<String>) -> Result<Self> {
        let mut parts = self.parts.clone();
        parts.push(child.into());
        Self::new(parts)
    }

    /// Dotted display form (not for SQL emission).
    #[must_use]
    pub fn to_dotted(&self) -> String {
        self.parts.join(".")
    }
}

impl fmt::Display for FullyQualifiedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_dotted())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_id_rejects_empty() {
        assert!(ObjectId::new("").is_err());
        assert!(ObjectId::new("   ").is_err());
    }

    #[test]
    fn fqn_roundtrip_dotted() {
        let fqn = FullyQualifiedName::parse_dotted("cat.db.sch.tbl").unwrap();
        assert_eq!(fqn.len(), 4);
        assert_eq!(fqn.leaf(), "tbl");
        assert_eq!(fqn.to_dotted(), "cat.db.sch.tbl");
        assert_eq!(fqn.parent().unwrap().to_dotted(), "cat.db.sch");
    }

    #[test]
    fn fqn_rejects_empty() {
        assert!(FullyQualifiedName::new(Vec::<String>::new()).is_err());
    }
}
