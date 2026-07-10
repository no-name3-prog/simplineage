# Repository governance & development workflow

This document is the **standard process for every phase** of SimpLineage
(0.x foundation work, features, fixes, docs, and releases).  
It is mandatory for humans, bots, and future automation.

## Principles

1. **Never push directly to `main`.** All changes land via pull request.
2. **Feature branch → Pull Request → CI → Senior maintainer review → Merge.**
3. **CI Success is authoritative** for automated quality (see Phase 0.2).
4. **A senior maintainer reviews every PR** for architecture, maintainability,
   correctness, API design, documentation, performance, edge cases, and testing.
5. **Review is a cycle**, not a one-shot: address feedback, re-run CI, re-review
   until approved.
6. **Squash-merge** into `main`; delete the feature branch after merge.

```text
┌─────────────┐     ┌──────────┐     ┌─────────────┐     ┌────────────────┐     ┌─────────┐
│ Feature     │────▶│ Open PR  │────▶│ CI Success  │────▶│ Senior         │────▶│ Squash  │
│ branch      │     │ (+ body) │     │ (required)  │     │ maintainer     │     │ merge   │
└─────────────┘     └──────────┘     └─────────────┘     │ review cycle   │     └─────────┘
                                                         └────────────────┘
```

## Roles

| Role | Responsibility |
|------|----------------|
| **Author** | Implements on a feature branch; writes a complete PR description; responds to review |
| **CI** | Runs fmt, Clippy, tests, docs, deny, audit, coverage, Docker on every PR |
| **Senior maintainer** | Reviews quality bar; requests changes or approves; merges when safe |
| **Code owners** | Listed in `.github/CODEOWNERS` for critical paths |

The repository ships an **automated senior-maintainer bot**
(`.github/workflows/maintainer.yml` + `.github/scripts/maintainer-review.mjs`)
that encodes this bar and may squash-merge when CI is green and the review is clean.
Human maintainers may always override, re-request changes, or merge manually under the same rules.

## Branch rules

| Branch | Rules |
|--------|--------|
| `main` | Protected. No direct commits. PR required. **CI Success** required. Review required. |
| `feat/*`, `fix/*`, `docs/*`, `chore/*`, `ci/*` | Development branches. Open PRs targeting `main`. |
| Tags `v*.*.*` | Created from `main` after release PR; trigger release workflow |

### Creating work for a phase

```bash
git fetch origin
git checkout main
git pull origin main
git checkout -b feat/phase-X-short-description   # or fix/…, docs/…
# … implement …
git push -u origin HEAD
gh pr create --base main   # fill the full PR template
```

**Forbidden:**

```bash
git push origin main          # never for feature work
git commit on main && push    # never
```

## Pull request requirements

Every PR **must** use the template (`.github/pull_request_template.md`) and include:

| Section | Purpose |
|---------|---------|
| **Summary** | What changed and why (user-visible impact) |
| **Design decisions** | Alternatives considered; why this approach; crate/API boundaries |
| **Testing** | What you ran, or “relying on CI” with what CI should prove; edge cases covered |
| **Future considerations** | Follow-ups, known limits, semver notes, debt |

Also:

- Prefer [Conventional Commits](https://www.conventionalcommits.org/) on the branch
- Update `CHANGELOG.md` under `[Unreleased]` when user-facing
- No secrets, credentials, or local `.env` files
- Keep PRs reviewable (prefer smaller phase slices)

Draft PRs are welcome for early feedback; the maintainer bot **will not merge** drafts.

## Senior maintainer review bar

Every PR is reviewed for:

| Lens | Questions |
|------|-----------|
| **Architecture** | Correct crate? Dependency direction toward `simplineage-core`? Offline-first? |
| **Maintainability** | Clear names, small modules, no unnecessary complexity? |
| **Correctness** | Errors handled? Config/logging edge cases? |
| **API design** | Stable `pub` surface? Semver impact documented? |
| **Documentation** | rustdoc / README / architecture / CHANGELOG as needed? |
| **Performance** | Hot-path allocations or clones avoided when it matters? |
| **Edge cases** | Empty input, invalid config, large-scale caveats noted? |
| **Testing** | Behavior covered by tests or an explicit CI-only justification? |

### Outcomes

| Outcome | Meaning |
|---------|---------|
| **Request changes** | Blockers listed in detail. Author must fix on the **feature branch**. |
| **Approve** | Static review clean; merge still waits for **CI Success**. |
| **Merge** | Squash into `main` when review is approved **and** CI Success is green. |

### Review cycle (repeat until done)

1. Maintainer leaves detailed comments / blockers.
2. Author addresses **all** feedback on the feature branch (not `main`).
3. Push triggers the **complete CI pipeline** again.
4. Maintainer re-reviews.
5. When approved + CI green → squash-merge and delete branch.

Do not open a second PR to “fix review” unless the original was closed; push to the same branch.

## CI and local tooling

- **Authoritative gate:** GitHub Actions **CI Success** (Phase 0.2).
- Contributors are **not** required to install nextest, deny, audit, or llvm-cov locally.
- Optional: rustup for smoke tests; Docker/devcontainer for full tool parity.

See [CONTRIBUTING.md](../CONTRIBUTING.md) and [development.md](development.md).

## Branch protection (maintainers)

Apply repository settings (requires admin):

```bash
./.github/scripts/setup-branch-protection.sh
```

Expected protection on `main`:

- Require pull request before merging  
- Require status check: **CI Success**  
- Require at least one approving review  
- Require conversation resolution  
- No force push / no deletion of `main`  
- Squash merge preferred; delete head branch on merge  

## How this applies to future phases

For **Phase 0.4+, Phase 1+, features, refactors, and hotfixes**:

1. One phase (or cohesive change) → one feature branch → one PR (or a small stack of PRs).
2. PR body always includes Summary / Design decisions / Testing / Future considerations.
3. Full CI must pass; maintainer review must approve; then merge.
4. Update this governance doc only via the same PR workflow if the process itself changes.

## Related files

| Path | Role |
|------|------|
| `.github/pull_request_template.md` | Required PR sections |
| `.github/workflows/ci.yml` | Quality pipeline |
| `.github/workflows/maintainer.yml` | Senior maintainer automation |
| `.github/scripts/maintainer-review.mjs` | Static review heuristics |
| `.github/scripts/setup-branch-protection.sh` | Protect `main` |
| `.github/CODEOWNERS` | Ownership |
| [CONTRIBUTING.md](../CONTRIBUTING.md) | Contributor entry point |
