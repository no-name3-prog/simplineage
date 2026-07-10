# SimpLineage

**Offline-first metadata intelligence for data engineers.**

SimpLineage ingests exported metadata, builds lineage graphs, runs impact analysis,
and produces interactive offline reports — without requiring a cloud control plane.

> **Status:** Core model, import, graph, analysis, SQLite storage, professional CLI, and **offline interactive HTML lineage reports**.

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
- Offline interactive HTML lineage reports (pan/zoom/search/impact/dark mode/SVG+Mermaid) plus GraphML / CSV / Mermaid export
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

### Docker (no Rust install)

Build the CLI image and run a smoke command:

```bash
docker compose build simplineage
docker compose run --rm simplineage
docker compose run --rm simplineage --help
```

Import a sample and write an HTML report (mount examples + host `./out`):

```bash
mkdir -p ./out

docker compose run --rm \
  -v "$(pwd)/examples:/data/examples:ro" \
  -v "$(pwd)/out:/data/out" \
  -v simplineage-store:/home/simplineage/.simplineage \
  simplineage \
  --data-dir /home/simplineage/.simplineage \
  --no-progress \
  import /data/examples/production_warehouse_lineage \
  --label prod-demo

docker compose run --rm \
  -v "$(pwd)/out:/data/out" \
  -v simplineage-store:/home/simplineage/.simplineage \
  simplineage \
  --data-dir /home/simplineage/.simplineage \
  --no-progress \
  export -f html -o /data/out/lineage.html

open ./out/lineage.html   # macOS; Linux: xdg-open ./out/lineage.html
```

Full guide (volumes, your own files, dev image): **[docs/docker.md](docs/docker.md)**.

### Tests

```bash
make check          # light local: fmt + clippy + test + doc
# Full suite: open a pull request (CI Success)
```

## Quick usage

Use sample data from this repo to try lineage end-to-end: **import → HTML report → open in browser**. No server is required.

Pick one:

| How | Need |
|-----|------|
| **Rust on the host** | [rustup](https://rustup.rs/) — steps below |
| **Docker only** | Docker Desktop / Colima / OrbStack — see [Docker section](#docker-no-rust-install) and [docs/docker.md](docs/docker.md) |

### With Rust (host)

From the repo root:

### 1. Production-style demo (recommended)

A multi-layer warehouse graph (`raw → staging → marts → metrics → serving`), closer to real data-engineering setups.

```bash
# Import the sample metadata into a local store
cargo run -q -p simplineage-cli -- \
  --data-dir .simplineage \
  --no-progress \
  import examples/production_warehouse_lineage \
  --label prod-demo

# Build an offline interactive HTML report
cargo run -q -p simplineage-cli -- \
  --data-dir .simplineage \
  --no-progress \
  export -f html -o lineage.html

# Open the file in your browser (macOS)
open lineage.html
# Linux: xdg-open lineage.html
```

In the HTML page you can zoom, pan, search tables, filter kinds, click a node for details, run upstream/downstream impact, toggle dark mode, and download SVG or Mermaid.

More detail: [examples/production_warehouse_lineage/README.md](examples/production_warehouse_lineage/README.md).

### 2. Smaller BigQuery-style demo

CSV dumps shaped like BigQuery `INFORMATION_SCHEMA` (tables, columns, lineage).

```bash
cargo run -q -p simplineage-cli -- \
  --data-dir .simplineage \
  --no-progress \
  import examples/bigquery_information_schema \
  --label bq-demo

cargo run -q -p simplineage-cli -- \
  --data-dir .simplineage \
  --no-progress \
  export -f html -o lineage-bq.html

open lineage-bq.html
```

More detail: [examples/bigquery_information_schema/README.md](examples/bigquery_information_schema/README.md).

### 3. Your own files

Point `import` at any folder or file (CSV, JSON, Parquet, Excel). Same export step:

```bash
cargo run -q -p simplineage-cli -- \
  --data-dir .simplineage \
  import /path/to/your/metadata_export

cargo run -q -p simplineage-cli -- \
  --data-dir .simplineage \
  export -f html -o lineage.html
```

Optional: Mermaid text (`export -f mermaid -o lineage.mmd`), search (`search orders`), impact (`impact orders --direction both`).

Full CLI: [docs/cli.md](docs/cli.md) · HTML report: [docs/html-export.md](docs/html-export.md) · Docker: [docs/docker.md](docs/docker.md).

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
- [CLI](docs/cli.md) — command reference  
- [Docker](docs/docker.md) — run without installing Rust  
- [HTML export](docs/html-export.md) — offline interactive lineage report  
- [Metadata model](docs/metadata-model.md) — vendor-agnostic catalog types  
- [Importers](docs/importers.md) — plugin import framework  
- [Graph engine](docs/graph-engine.md) — lineage traversal & stats  
- [Analysis engine](docs/analysis-engine.md) — impact, quality, criticality  
- [Storage](docs/storage.md) — local SQLite persistence  
- [Development](docs/development.md)  
- [Examples](examples/) — sample metadata dumps for demos  
- [Code of Conduct](CODE_OF_CONDUCT.md)  
- [Security](SECURITY.md)  
- [Changelog](CHANGELOG.md)

## Semantic versioning

[SemVer](https://semver.org/). Tag `vX.Y.Z` on `main` to trigger the release
workflow. Details in CONTRIBUTING.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
