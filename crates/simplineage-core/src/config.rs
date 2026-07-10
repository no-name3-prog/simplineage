//! Configuration loading and types.
//!
//! Sources (later overrides earlier):
//! 1. Built-in defaults
//! 2. `config/default.toml` (relative to process CWD, if present)
//! 3. `config/local.toml` (gitignored local overrides)
//! 4. Environment variables with prefix `SIMPLINEAGE_` (nested via `__`)

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Top-level application settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Settings {
    /// Logging configuration.
    pub logging: LoggingSettings,
    /// Local storage configuration.
    pub storage: StorageSettings,
    /// HTTP server configuration (used by `simplineage-server`).
    pub server: ServerSettings,
}

/// Logging output settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoggingSettings {
    /// Minimum log level: `trace`, `debug`, `info`, `warn`, or `error`.
    pub level: String,
    /// Output format: `text` or `json`.
    pub format: String,
}

impl Default for LoggingSettings {
    fn default() -> Self {
        Self {
            level: "info".into(),
            format: "text".into(),
        }
    }
}

/// On-disk storage settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageSettings {
    /// Directory for local metadata / DuckDB or SQLite files.
    pub data_dir: PathBuf,
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from(".simplineage"),
        }
    }
}

/// HTTP API server settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerSettings {
    /// Bind address.
    pub host: String,
    /// Bind port.
    pub port: u16,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 8080,
        }
    }
}

impl Settings {
    /// Load settings from defaults, optional TOML files, and environment variables.
    pub fn load() -> Result<Self> {
        Self::load_from(Path::new("config"))
    }

    /// Load settings, looking for `default.toml` / `local.toml` under `config_dir`.
    pub fn load_from(config_dir: &Path) -> Result<Self> {
        let mut builder = config::Config::builder()
            .set_default("logging.level", "info")
            .map_err(config_err)?
            .set_default("logging.format", "text")
            .map_err(config_err)?
            .set_default("storage.data_dir", ".simplineage")
            .map_err(config_err)?
            .set_default("server.host", "127.0.0.1")
            .map_err(config_err)?
            .set_default("server.port", 8080)
            .map_err(config_err)?;

        let default_path = config_dir.join("default.toml");
        if default_path.is_file() {
            builder = builder.add_source(config::File::from(default_path));
        }

        let local_path = config_dir.join("local.toml");
        if local_path.is_file() {
            builder = builder.add_source(config::File::from(local_path));
        }

        builder = builder.add_source(
            config::Environment::with_prefix("SIMPLINEAGE")
                .separator("__")
                .try_parsing(true),
        );

        builder
            .build()
            .map_err(config_err)?
            .try_deserialize()
            .map_err(config_err)
    }
}

fn config_err(err: impl ToString) -> Error {
    Error::Config(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn default_settings_are_sane() {
        let s = Settings::default();
        assert_eq!(s.logging.level, "info");
        assert_eq!(s.server.port, 8080);
    }

    #[test]
    fn loads_from_toml_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config_dir = dir.path().join("config");
        fs::create_dir_all(&config_dir).unwrap();
        let mut f = fs::File::create(config_dir.join("default.toml")).unwrap();
        writeln!(
            f,
            r#"
[logging]
level = "debug"
format = "json"

[storage]
data_dir = "/tmp/sl-test"

[server]
host = "0.0.0.0"
port = 9090
"#
        )
        .unwrap();

        let settings = Settings::load_from(&config_dir).expect("load");
        assert_eq!(settings.logging.level, "debug");
        assert_eq!(settings.logging.format, "json");
        assert_eq!(settings.storage.data_dir, PathBuf::from("/tmp/sl-test"));
        assert_eq!(settings.server.host, "0.0.0.0");
        assert_eq!(settings.server.port, 9090);
    }
}
