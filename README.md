# SimpLineage

**Offline-first metadata intelligence for data engineers.**

SimpLineage ingests exported metadata, builds lineage graphs, runs impact analysis,
and produces interactive offline reports — without requiring a cloud control plane.

> **Status:** Core model, import, graph, analysis, SQLite storage, and **professional CLI**. HTML/export visualization still evolving.

[![CI](https://github.com/no-name3-prog/simplineage/actions/workflows/ci.yml/badge.svg)](https://github.com/no-name3-prog/simplineage/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](rust-toolchain.toml)

## Contribute without heavy local installs

| You want… | You need… |
|-----------|-----------|
| Open a PR and let quality gates run | git + GitHub |
| Build/run the CLI locally | [rustup](https://rustup.rs/) only |
| Full CI tools without polluting the host | **Docker** or **Codespaces** / Dev Container |
| Match CI on bare metal | optional (not required) — see [CONTRIBUTING.md](CONTRIBUTING.md) |

**GitHub Actions is the source of truth.** Every PR runs rustfmt, Clippy, nextest,
rustdoc, cargo-deny, cargo-audit, coverage, and Docker, then aggregates under
**CI Success**.

```bash
# Optional light local smoke (rustup only)
cargo test --workspace
cargo run -p simplineage-cli -- hello

# Optional full tools in Docker (no cargo install on host)
docker compose --profile dev run --rm dev
```

## Features

- Plugin-based metadata importers (CSV, JSON, Excel, Parquet, sample warehouse)
- Normalized common metadata model
- Directed lineage graph with upstream / downstream traversal
- Impact analysis, cycle detection, orphan discovery, quality checks
- Lightweight SQLite metadata store (offline-first)
- Professional CLI (`import`, `build`, `search`, `upstream`, `downstream`, `impact`, `validate`, `stats`, `compare`, `export`)
- Offline HTML / GraphML / CSV export
- Language bindings (Python, Node) — planned

See [docs/cli.md](docs/cli.md) for the full command reference.

## Architecture

```text
Importers → Normalizer → Storage → Graph Engine → Analysis → CLI / Server / UI
```

See [docs/architecture.md](docs/architecture.md).

## Quick start

### Prerequisites

- **Minimum:** git  
- **Optional local build:** Rust 1.85+ via rustup  
- **Optional full tooling:** Docker (see `Dockerfile.dev`)

### Build & run (optional local Rust)

```bash
git clone https://github.com/no-name3-prog/simplineage.git
cd simplineage
cargo build
cargo run -p simplineage-cli -- --help
cargo run -p simplineage-cli -- hello --name engineer
```

### Docker runtime image

```bash
docker compose build
docker compose run --rm simplineage
```

### Tests

```bash
make check          # light local: fmt + clippy + test + doc
# Full suite: open a pull request (CI Success)
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

## Configuration

Default: [`config/default.toml`](config/default.toml). Override with
`SIMPLINEAGE_*` env vars (nested via `__`) or `RUST_LOG`.

## Project docs

- [Contributing](CONTRIBUTING.md) — **CI-first** workflow  
- [Governance](docs/GOVERNANCE.md) — **PR-only** process for every phase  
- [Metadata model](docs/metadata-model.md) — vendor-agnostic catalog types  
- [Importers](docs/importers.md) — plugin import framework  
- [Graph engine](docs/graph-engine.md) — lineage traversal & stats  
- [Analysis engine](docs/analysis-engine.md) — impact, quality, criticality  
- [Storage](docs/storage.md) — DuckDB persistence  
- [Development](docs/development.md)  
- [Code of Conduct](CODE_OF_CONDUCT.md)  
- [Security](SECURITY.md)  
- [Changelog](CHANGELOG.md)

## Semantic versioning

[SemVer](https://semver.org/). Tag `vX.Y.Z` on `main` to trigger the release
workflow. Details in CONTRIBUTING.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
