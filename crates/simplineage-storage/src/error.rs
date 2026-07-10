//! Storage errors.

use thiserror::Error;

/// Result alias for storage operations.
pub type Result<T> = std::result::Result<T, StorageError>;

/// Errors from the SQLite metadata store.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Database / SQL failure.
    #[error("database error: {0}")]
    Database(String),

    /// Requested snapshot was not found.
    #[error("snapshot not found: {0}")]
    NotFound(String),

    /// Migration failure.
    #[error("migration error: {0}")]
    Migration(String),

    /// Core model error.
    #[error(transparent)]
    Core(#[from] simplineage_core::Error),

    /// I/O error.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// JSON error.
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}

impl From<rusqlite::Error> for StorageError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Database(value.to_string())
    }
}
