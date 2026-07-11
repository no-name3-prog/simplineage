# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **Performance / dedup (P0–P1):** cheaper catalog lookups (`Snapshot::kind_index` / `contains_object`); analysis engine avoids repeated full `object_index` clones; shared tabular CSV/Excel/Parquet row mapping; unified `DependencyKind`/`DependencyLevel` parse/display; snapshot merge without double-cloning collections; Mermaid export skips HTML layered layout
- Offline HTML reports: **click a node** auto-highlights its full upstream/downstream and dims the rest; **search** keeps matches plus their lineage neighborhood bright (others dim). Upstream/Downstream/Both still narrow focus; Clear or empty-canvas click resets selection.
- Maintainer bot reviews PRs only; **human final approval** is required to merge (no automated squash-merge)

### Added

- **Column-level lineage UX (CLI + HTML):** `upstream` / `downstream` / `impact` accept `schema.table.column`, `--column`, and `--level all|relation|column` (`ImpactOptions::columns_only`); text output shows `[kind] fqn`; JSON keeps id arrays and adds `subject_fqn` / `upstream_nodes` / `downstream_nodes`. HTML report embeds `parent_id` / data types, table **Columns** panel, smaller column nodes, dashed column edges, and focus-reveals columns under *Relations only*
- **HTML layout:** pack **relation** edges first (vertically centered layers), nest columns under `parent_id` — avoids huge vertical gaps when *Relations only* hides column nodes
- **Importers:** unique dependency ids for multi-column edges between the same tables; blank `from_column`/`to_column` cells treated as relation-level
- Docker usage guide ([docs/docker.md](docs/docker.md)): runtime CLI image, volume mounts for samples/HTML export, and dev profile
- `Snapshot::kind_index` / `contains_object`; `build_export_graph` for lightweight Mermaid/text exports; `importers::tabular` shared field aliases
- Phase 7 offline interactive HTML lineage report: pan/zoom, search, filters, metadata panel, client-side impact analysis, dark mode, SVG + Mermaid export (no server); `export -f mermaid`
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
