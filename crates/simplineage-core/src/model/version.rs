//! Metadata model schema versioning.
//!
//! The **model version** describes the shape of serialized metadata (JSON, etc.),
//! independent of any database vendor or SimpLineage application semver.

use serde::{Deserialize, Serialize};

/// Current metadata model schema version (semver string).
///
/// Bump when serialized fields are added/removed/renamed in a breaking way.
/// Additive optional fields may keep the same major version.
pub const MODEL_VERSION: &str = "1.0.0";

/// Major version of [`MODEL_VERSION`].
pub const MODEL_VERSION_MAJOR: u32 = 1;

/// Metadata model schema version as carried in snapshots and export envelopes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelVersion(String);

impl ModelVersion {
    /// Create a version from a semver-like string (e.g. `"1.0.0"`).
    pub fn new(version: impl Into<String>) -> Self {
        Self(version.into())
    }

    /// The version currently produced by this library.
    pub fn current() -> Self {
        Self(MODEL_VERSION.to_string())
    }

    /// Borrow the raw version string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Parse the leading major component (`"1.2.3"` → `1`).
    pub fn major(&self) -> Option<u32> {
        self.0.split('.').next().and_then(|p| p.parse().ok())
    }

    /// Whether this version is readable by the current library.
    ///
    /// Policy: same major version is accepted; other majors are rejected.
    pub fn is_compatible_with_current(&self) -> bool {
        self.major() == Some(MODEL_VERSION_MAJOR)
    }
}

impl Default for ModelVersion {
    fn default() -> Self {
        Self::current()
    }
}

impl std::fmt::Display for ModelVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for ModelVersion {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ModelVersion {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_is_compatible() {
        assert!(ModelVersion::current().is_compatible_with_current());
    }

    #[test]
    fn same_major_compatible() {
        assert!(ModelVersion::new("1.9.0").is_compatible_with_current());
    }

    #[test]
    fn other_major_incompatible() {
        assert!(!ModelVersion::new("2.0.0").is_compatible_with_current());
        assert!(!ModelVersion::new("0.9.0").is_compatible_with_current());
    }
}
