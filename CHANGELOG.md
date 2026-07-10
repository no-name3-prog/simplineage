# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

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
