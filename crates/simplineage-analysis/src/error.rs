//! Analysis / graph engine errors.

use thiserror::Error;

/// Result alias for analysis operations.
pub type Result<T> = std::result::Result<T, GraphError>;

/// Errors from the lineage graph engine.
#[derive(Debug, Error)]
pub enum GraphError {
    /// Referenced node is not in the graph.
    #[error("unknown node: {0}")]
    UnknownNode(String),

    /// Operation failed for another reason.
    #[error("{0}")]
    Message(String),
}

impl GraphError {
    /// Convenience constructor.
    pub fn msg(s: impl Into<String>) -> Self {
        Self::Message(s.into())
    }
}
