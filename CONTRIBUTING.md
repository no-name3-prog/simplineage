# Contributing to SimpLineage

Thanks for helping improve SimpLineage.

## Code of Conduct

See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## Pull request workflow

**Direct pushes to `main` are not allowed.** All changes land via pull request.

1. Branch from latest `main` (`feat/…`, `fix/…`, `docs/…`)
2. Make your changes
3. Open a PR and fill in the template
4. Wait for **CI Success** (required)
5. Address review feedback; squash-merge when approved

You do **not** need a full local toolchain to contribute. CI is the authoritative gate.

## What you need locally (choose one path)

### Path A — Minimal (recommended for most contributors)

| Need | Install |
|------|---------|
| Edit & open PR | git + GitHub (browser or `gh`) |
| Optional smoke test | [rustup](https://rustup.rs/) only (`cargo build` / `cargo test` / `cargo run`) |

**Do not install** cargo-nextest, cargo-deny, cargo-audit, cargo-llvm-cov, or
cargo-release on your machine unless you want them. GitHub Actions installs and
runs them on every PR.

```bash
# Optional minimal Rust (skip entirely if you only edit docs / rely on CI)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
cd simplineage
cargo test --workspace
cargo run -p simplineage-cli -- hello
```

### Path B — Zero host Rust: Docker or Codespaces

Full tooling (nextest, deny, audit, llvm-cov, clippy, rustfmt) lives in the image:

```bash
# Docker only on the host
docker compose --profile dev run --rm dev
# inside container:
cargo nextest run --workspace
cargo deny check
cargo audit
cargo llvm-cov --workspace --html
cargo clippy --workspace --all-targets -- -D warnings
```

Or open the repo in **GitHub Codespaces** / VS Code **Dev Containers**
(`.devcontainer/devcontainer.json`).

### Path C — Match CI on the host (optional)

Only if you prefer not using Docker/CI for feedback:

```bash
rustup component add rustfmt clippy llvm-tools-preview
cargo install cargo-nextest cargo-deny cargo-audit cargo-llvm-cov --locked
```

This is **optional** and not required for merge.

## What CI runs on every PR

| Check | Tool | Where |
|-------|------|--------|
| Format | rustfmt | GitHub Actions |
| Lints | Clippy (`-D warnings`) | GitHub Actions |
| Tests | cargo-nextest + doctests | GitHub Actions |
| Docs | `cargo doc` | GitHub Actions |
| Licenses / advisories | cargo-deny | GitHub Actions |
| CVEs | cargo-audit | GitHub Actions |
| Coverage | cargo-llvm-cov (≥ 40% lines) | GitHub Actions |
| Image | Docker build | GitHub Actions |
| Gate | **CI Success** job | required status |

Docs also publish from `main` (GitHub Pages). Tags `v*.*.*` trigger binary releases.

## Local helpers (stock Cargo only)

```bash
make build    # cargo build --workspace
make test     # cargo test --workspace (no nextest required)
make fmt      # cargo fmt
make clippy   # cargo clippy -D warnings
make doc      # cargo doc
make check    # fmt + clippy + test + doc  (light local gate)
make bench    # cargo bench -p simplineage-core
```

`make check` is a convenience smoke gate. **Passing CI Success is what matters for merge.**

## Commit style

Prefer [Conventional Commits](https://www.conventionalcommits.org/):
`feat:`, `fix:`, `docs:`, `chore:`, `ci:`, `refactor:`, `test:`, `perf:`.

## Semantic versioning

We follow [SemVer](https://semver.org/). Maintainers cut releases by:

1. Updating workspace version in root `Cargo.toml` and `[Unreleased]` in `CHANGELOG.md`
2. Tagging `vX.Y.Z` and pushing the tag (triggers `.github/workflows/release.yml`)

`cargo-release` + `release.toml` are optional helpers; they are not required on contributor machines.

## Security

See [SECURITY.md](SECURITY.md).

## Architecture

See [docs/architecture.md](docs/architecture.md) and [docs/development.md](docs/development.md).
