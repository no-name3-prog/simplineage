# Contributing to SimpLineage

Thanks for helping improve SimpLineage.

## Code of Conduct

See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## Canonical workflow (all phases)

**Read [docs/GOVERNANCE.md](docs/GOVERNANCE.md).** That document is the standard
process for every future phase.

Short version:

1. **Never push to `main`.** Create a feature branch.
2. Open a **Pull Request** with a complete description:
   - Summary  
   - Design decisions  
   - Testing  
   - Future considerations  
3. Wait for **CI Success** (authoritative automated gate).
4. An **automated senior review** checks architecture, maintainability, correctness,
   API design, docs, performance, edge cases, and testing.
5. Address **all** feedback on the feature branch; CI re-runs; review repeats
   until the automated bar is clean.
6. A **human maintainer** gives final approval and **squash-merges** to `main`
   (the bot does not merge). Delete the feature branch after merge.

```bash
git checkout main && git pull
git checkout -b feat/your-change
# … work …
git push -u origin HEAD
gh pr create --base main
```

## What you need locally

| Path | Need |
|------|------|
| **Minimal** | git + GitHub (CI runs the full suite) |
| **Optional smoke** | [rustup](https://rustup.rs/) only |
| **Full tools without host installs** | Docker / Codespaces (see `Dockerfile.dev`) |

You do **not** need cargo-nextest, cargo-deny, cargo-audit, or cargo-llvm-cov on
your machine. See Phase 0.2 CI-first model in [docs/development.md](docs/development.md).

```bash
# Optional light local smoke
cargo test --workspace
cargo run -p simplineage-cli -- hello

# Optional full tools in Docker
docker compose --profile dev run --rm dev
```

```bash
make check   # light local: fmt + clippy + test + doc
```

## Commit style

Prefer [Conventional Commits](https://www.conventionalcommits.org/):
`feat:`, `fix:`, `docs:`, `chore:`, `ci:`, `refactor:`, `test:`, `perf:`.

## Semantic versioning

[SemVer](https://semver.org/). Maintainers tag `vX.Y.Z` on `main` after a release
PR (never by pushing ad-hoc commits to `main`). See `release.toml` and
`.github/workflows/release.yml`.

## Security

See [SECURITY.md](SECURITY.md).

## Architecture

See [docs/architecture.md](docs/architecture.md).
