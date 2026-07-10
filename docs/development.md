# Development guide

## Philosophy: CI-first

Heavy engineering tools run in **GitHub Actions** (and optionally in Docker /
devcontainers). Contributors should not need a long list of `cargo install`
commands on their laptop.

| Environment | Purpose |
|-------------|---------|
| Host + rustup (optional) | Edit, `cargo build` / `test` / `run` |
| GitHub Actions | Authoritative fmt, clippy, nextest, deny, audit, coverage, docker, docs |
| `Dockerfile.dev` / Codespaces | Same tools as CI without installing them on the host |
| `Dockerfile` | Runtime CLI image |

## CI jobs

See `.github/workflows/ci.yml`. The aggregate job **CI Success** is intended as
the single required branch-protection check.

## Branch protection (maintainers)

```bash
./.github/scripts/setup-branch-protection.sh
```

Requires `gh` with admin rights on the repository.

## Benchmarks

Criterion benches live under `crates/simplineage-core/benches/`. They use only
`cargo bench` (Cargo downloads Criterion as a dev-dependency — no separate
global install).

```bash
cargo bench -p simplineage-core
# or in Docker: docker compose --profile dev run --rm dev cargo bench -p simplineage-core
```

## Coverage

Generated in CI as `lcov.info` (artifact). Optional Codecov upload if
`CODECOV_TOKEN` is set as a repository secret. Local coverage needs
`cargo-llvm-cov` (Docker/devcontainer or optional host install).

## Releases

1. Bump `[workspace.package] version` and `CHANGELOG.md`
2. Commit on `main` via PR
3. `git tag vX.Y.Z && git push origin vX.Y.Z`
4. Release workflow builds CLI archives and creates a GitHub Release
