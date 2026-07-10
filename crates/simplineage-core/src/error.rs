//! Error types for the core crate.

use thiserror::Error;

/// Convenient result alias for SimpLineage core operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors produced by core configuration, I/O, model validation, and engine operations.
#[derive(Debug, Error)]
pub enum Error {
    /// Configuration could not be loaded or validated.
    #[error("configuration error: {0}")]
    Config(String),

    /// Metadata model failed validation.
    #[error("metadata model validation failed: {0}")]
    Validation(String),

    /// Metadata model schema version is unsupported.
    #[error("unsupported metadata model version: {found} (supported: {supported})")]
    UnsupportedModelVersion {
        /// Version found in the payload.
        found: String,
        /// Versions this library understands.
        supported: String,
    },

    /// Metadata import failed.
    #[error("import error: {0}")]
    Import(String),

    /// Serialization or deserialization failed.
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    /// Underlying I/O failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Catch-all for unexpected failures (bridges `anyhow` and similar).
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl Error {
    /// Build a validation error from a message.
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    /// Build an import error from a message.
    pub fn import(msg: impl Into<String>) -> Self {
        Self::Import(msg.into())
    }
}
