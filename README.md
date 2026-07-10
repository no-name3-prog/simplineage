# SimpLineage

**Offline-first metadata intelligence for data engineers.**

SimpLineage ingests exported metadata, builds lineage graphs, runs impact analysis,
and produces interactive offline reports — without requiring a cloud control plane.

> **Status:** Phase 0.1 — repository foundation. Core engine features are not
> implemented yet; the workspace, CLI stub, config, and logging are in place.

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

## Features (planned)

- Plugin-based metadata importers (CSV, JSON, Excel, Parquet, warehouses)
- Normalized common metadata model
- Directed lineage graph with upstream / downstream traversal
- Impact analysis, cycle detection, orphan discovery
- Offline HTML visualization and CLI workflows
- Language bindings (Python, Node) on top of a Rust core

## Architecture

```text
Importers → Normalizer → Storage → Graph Engine → Analysis → CLI / Server / UI
```

See [docs/architecture.md](docs/architecture.md) for crate layout and design rules.

## Quick start

### Prerequisites

- [Rust](https://rustup.rs/) 1.85+ (stable toolchain; see `rust-toolchain.toml`)

### Build

```bash
git clone https://github.com/no-name3-prog/simplineage.git
cd simplineage
cargo build
```

### Run the CLI

```bash
# Hello World (default command)
cargo run -p simplineage-cli

# Explicit hello
cargo run -p simplineage-cli -- hello --name engineer

# Version / status
cargo run -p simplineage-cli -- version
cargo run -p simplineage-cli -- status

# After `cargo install --path crates/simplineage-cli`:
simplineage hello
```

### Tests

```bash
cargo test --workspace
```

### Lints (recommended)

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

## Workspace crates

| Crate | Role |
|-------|------|
| `simplineage-core` | Config, logging, engine façade |
| `simplineage-importers` | Metadata import plugins |
| `simplineage-storage` | Local storage backends |
| `simplineage-analysis` | Graph analysis |
| `simplineage-exporters` | HTML/JSON and other exports |
| `simplineage-cli` | `simplineage` binary |
| `simplineage-server` | HTTP API (placeholder) |
| `simplineage-bindings` | FFI / language bindings (placeholder) |

Also reserved: `web/` (frontend), `bindings/` (packaging for other languages).

## Configuration

Default file: [`config/default.toml`](config/default.toml).

| Key | Env override | Default |
|-----|--------------|---------|
| `logging.level` | `SIMPLINEAGE_LOGGING__LEVEL` | `info` |
| `logging.format` | `SIMPLINEAGE_LOGGING__FORMAT` | `text` |
| `storage.data_dir` | `SIMPLINEAGE_STORAGE__DATA_DIR` | `.simplineage` |
| `server.host` | `SIMPLINEAGE_SERVER__HOST` | `127.0.0.1` |
| `server.port` | `SIMPLINEAGE_SERVER__PORT` | `8080` |

`RUST_LOG` is honored when set.

## Project layout

```text
crates/          Rust workspace members
config/          Default configuration
docs/            Architecture and design docs
web/             Frontend placeholder
bindings/        Multi-language packaging placeholder
```

## Contributing

This project uses a **pull-request workflow**:

1. Create a feature branch from `main` (do not push directly to `main`).
2. Implement changes with tests where practical.
3. Open a PR; address review feedback.
4. Merge only after checks pass and review is approved (squash merge preferred).

## License

Licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.

## Acknowledgments

Inspired by the needs of data platform engineers who want lineage that works on a laptop, from exports they already have.
