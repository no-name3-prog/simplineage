//! HTTP API server for SimpLineage.
//!
//! Placeholder for a future REST/gRPC surface serving lineage queries.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

use simplineage_core::Settings;

/// Server handle (not yet listening).
#[derive(Debug)]
pub struct Server {
    settings: Settings,
}

impl Server {
    /// Create a server configured from `settings`.
    pub fn new(settings: Settings) -> Self {
        Self { settings }
    }

    /// Address string this server would bind to.
    pub fn bind_addr(&self) -> String {
        format!(
            "{}:{}",
            self.settings.server.host, self.settings.server.port
        )
    }

    /// Start the server (stub — returns immediately).
    pub fn run(&self) -> simplineage_core::Result<()> {
        tracing::info!(addr = %self.bind_addr(), "server stub; not listening yet");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use simplineage_core::Settings;

    #[test]
    fn bind_addr_from_defaults() {
        let server = Server::new(Settings::default());
        assert_eq!(server.bind_addr(), "127.0.0.1:8080");
    }
}
