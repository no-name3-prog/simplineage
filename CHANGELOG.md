# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Maintainer bot reviews PRs only; **human final approval** is required to merge (no automated squash-merge)

### Added

- Phase 6 professional CLI (`simplineage`): clap subcommands import/build/search/upstream/downstream/impact/validate/stats/compare/export with colored output, progress bars, JSON mode, and exporters (JSON/CSV/GraphML/HTML/analysis)
- BigQuery INFORMATION_SCHEMA CSV demo (`examples/bigquery_information_schema`) with table_catalog/table_type/is_nullable mapping, directory merge + FQN reconcile
- Phase 5 SQLite storage: snapshots, incremental imports, materialized edges, migrations, benchmarks
- Phase 4 analysis engine: impact, dependency validation, unused/orphan detection, cycles, critical tables, longest chains, quality checks
- Phase 3 lineage graph engine: upstream/downstream traversal, shortest path, all paths, cycle detection, statistics, Criterion benchmarks
- Plugin-based metadata import framework: auto-detect CSV/JSON/Parquet/Excel, normalize to Snapshot, sample warehouse crate, CLI `import`
- Phase 1 core metadata model: vendor-agnostic Catalog, Database, Schema, Table, View, MaterializedView, Column, Relationship, Dependency, Snapshot with JSON serialization, validation, and model versioning
- Phase 0.3 repository governance: mandatory PR workflow, senior maintainer review automation, branch protection helper, and [docs/GOVERNANCE.md](docs/GOVERNANCE.md) as the standard process for every future phase
- Phase 0.2 engineering standards: GitHub Actions CI (rustfmt, Clippy, nextest,
  rustdoc, cargo-deny, cargo-audit, llvm-cov, Docker), Dependabot, release
  workflow, Criterion benches, Docker/devcontainer for optional local full tooling,
  and open-source governance files
- CI-first contribution model: heavy tools run in CI (or Docker), not required on host

## [0.1.0] - 2026-07-10

### Added

- Cargo workspace, Hello World CLI, config, logging, dual license, architecture docs

[Unreleased]: https://github.com/no-name3-prog/simplineage/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/no-name3-prog/simplineage/releases/tag/v0.1.0
