//! Tracing / logging initialization.

use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::LoggingSettings;
use crate::error::Result;

/// Initialize the global tracing subscriber from settings.
///
/// Respects `RUST_LOG` when set; otherwise uses `settings.level`.
/// Safe to call once at process startup. Subsequent calls return `Ok(())`
/// without re-initializing (subscriber already set).
pub fn init_tracing(settings: &LoggingSettings) -> Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(settings.level.clone()));

    let registry = tracing_subscriber::registry().with(filter);

    let result = match settings.format.as_str() {
        "json" => registry
            .with(fmt::layer().json().with_target(true).with_thread_ids(false))
            .try_init(),
        _ => registry
            .with(
                fmt::layer()
                    .with_target(true)
                    .with_thread_ids(false)
                    .compact(),
            )
            .try_init(),
    };

    // Ignore "already initialized" — common in tests and library reuse.
    if let Err(err) = result {
        let msg = err.to_string();
        if !msg.contains("already") {
            tracing::warn!("tracing init note: {msg}");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LoggingSettings;

    #[test]
    fn init_text_logging_succeeds() {
        let settings = LoggingSettings {
            level: "info".into(),
            format: "text".into(),
        };
        assert!(init_tracing(&settings).is_ok());
    }
}
