//! Error types for the core crate.

use thiserror::Error;

/// Convenient result alias for SimpLineage core operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors produced by core configuration, I/O, and engine operations.
#[derive(Debug, Error)]
pub enum Error {
    /// Configuration could not be loaded or validated.
    #[error("configuration error: {0}")]
    Config(String),

    /// Underlying I/O failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Catch-all for unexpected failures (bridges `anyhow` and similar).
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
