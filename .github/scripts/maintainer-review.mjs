#!/usr/bin/env node
/**
 * Senior open-source maintainer — automated static review for SimpLineage PRs.
 *
 * Reviews architecture signals, maintainability, correctness risks, API surface,
 * docs, performance smells, edge cases, and testing. Posts detailed feedback.
 *
 * Exit 0 = approve (no blockers). Exit 1 = request changes.
 * Merge still requires green CI Success (enforced by maintainer workflow).
 */
import { execSync } from "node:child_process";
import fs from "node:fs";

function sh(cmd) {
  return execSync(cmd, { encoding: "utf8" }).trim();
}

function section(title, lines) {
  return [`### ${title}`, ...lines, ""].join("\n");
}

const base = process.env.BASE_SHA || "origin/main";
const head = process.env.HEAD_SHA || "HEAD";

let files = [];
try {
  files = sh(`git diff --name-only ${base}...${head}`).split("\n").filter(Boolean);
} catch {
  try {
    files = sh("git diff --name-only origin/main...HEAD").split("\n").filter(Boolean);
  } catch {
    files = [];
  }
}

let diff = "";
try {
  diff = sh(`git diff ${base}...${head}`);
} catch {
  diff = "";
}

const blockers = [];
const nits = [];
const findings = [];

// ── Secrets / credentials ────────────────────────────────
const secretPaths = [
  /^\.env$/,
  /\.env\.local$/,
  /\.env\.production$/,
  /id_rsa/,
  /\.pem$/,
  /credentials\.json$/i,
  /\.p12$/,
];
for (const f of files) {
  const baseName = f.split("/").pop();
  if (secretPaths.some((p) => p.test(f) || p.test(baseName))) {
    blockers.push(`Possible secret or local env file committed: \`${f}\``);
  }
}

const secretContent = [
  /api[_-]?key\s*[:=]\s*['"][^'"]{8,}/i,
  /secret\s*[:=]\s*['"][^'"]{8,}/i,
  /password\s*[:=]\s*['"][^'"]{4,}/i,
  /-----BEGIN (RSA |OPENSSH |EC )?PRIVATE KEY-----/,
  /ghp_[A-Za-z0-9]{20,}/,
  /gho_[A-Za-z0-9]{20,}/,
  /github_pat_[A-Za-z0-9_]{20,}/,
  /AKIA[0-9A-Z]{16}/,
];
for (const re of secretContent) {
  if (re.test(diff)) {
    blockers.push(
      `Diff matches sensitive pattern \`${re}\` — remove secrets before merge.`
    );
  }
}

// ── Change classification ────────────────────────────────
const isRust = (f) => f.endsWith(".rs");
const isToml = (f) => f.endsWith("Cargo.toml") || f === "Cargo.lock" || f === "deny.toml";
const isCi = (f) => f.startsWith(".github/");
const isDoc = (f) =>
  f.endsWith(".md") ||
  f.startsWith("docs/") ||
  f.includes("README");
const isTest = (f) =>
  f.includes("/tests/") ||
  f.endsWith("_test.rs") ||
  /#\[cfg\(test\)\]/.test(diff) ||
  f.includes("/benches/");
const isCrateSrc = (f) => f.startsWith("crates/") && isRust(f) && !f.includes("/benches/");
const isCore = (f) => f.startsWith("crates/simplineage-core/");
const isPublicApi = (f) =>
  isCrateSrc(f) && (f.endsWith("lib.rs") || f.includes("/src/"));
const isUnsafe = /unsafe\s*\{/.test(diff);
const isUnwrapHeavy =
  (diff.match(/\.unwrap\(\)/g) || []).length >= 8 &&
  files.some(isCrateSrc);

const touchesRust = files.some(isRust);
const touchesTests =
  files.some(
    (f) =>
      f.includes("/tests/") ||
      f.includes("benches/") ||
      f.endsWith("tests.rs")
  ) || /#\[test\]/.test(diff);
const touchesCi = files.some(isCi);
const touchesDocs = files.some(isDoc);
const docsOnly = files.every(
  (f) =>
    isDoc(f) ||
    f.startsWith(".github/ISSUE_TEMPLATE") ||
    f === "LICENSE" ||
    f === "LICENSE-MIT" ||
    f === "LICENSE-APACHE" ||
    f.endsWith("dependabot.yml")
);
const governanceOnly = files.every(
  (f) =>
    isDoc(f) ||
    isCi(f) ||
    f === "deny.toml" ||
    f === "release.toml" ||
    f.startsWith("Dockerfile") ||
    f === "docker-compose.yml" ||
    f === "Makefile" ||
    f === ".dockerignore" ||
    f.startsWith(".devcontainer/")
);

// ── Architecture: core dependency direction ──────────────
// simplineage-core must not depend on other workspace crates
if (files.includes("crates/simplineage-core/Cargo.toml")) {
  try {
    const coreToml = sh("git show HEAD:crates/simplineage-core/Cargo.toml");
    if (/simplineage-(importers|storage|analysis|exporters|cli|server|bindings)/.test(coreToml)) {
      blockers.push(
        "`simplineage-core` must not depend on other workspace crates (dependency arrow must point inward)."
      );
    }
  } catch {
    /* ignore */
  }
}

// ── Unsafe Rust ──────────────────────────────────────────
if (isUnsafe && touchesRust) {
  nits.push(
    "Diff introduces `unsafe` blocks. Prefer safe APIs; if required, document invariants and safety comments."
  );
  if (!/SAFETY:|\/\/ SAFETY/.test(diff)) {
    nits.push("Add `// SAFETY:` comments explaining why each `unsafe` block is sound.");
  }
}

// ── Unwrap / expect in library crates ────────────────────
if (isUnwrapHeavy) {
  nits.push(
    "Many `.unwrap()` calls in the diff — prefer `Result`/`Option` propagation or expect with context in library code."
  );
}

// ── Tests for non-trivial Rust changes ───────────────────
const rustAppFiles = files.filter(
  (f) => isCrateSrc(f) && !f.includes("main.rs")
);
if (
  rustAppFiles.length >= 2 &&
  !touchesTests &&
  !docsOnly &&
  !governanceOnly &&
  !touchesCi
) {
  blockers.push(
    "Non-trivial Rust source changes without new/updated tests. Add unit/integration tests (or justify in the PR under Testing)."
  );
} else if (rustAppFiles.length === 1 && !touchesTests && !docsOnly && !governanceOnly) {
  nits.push(
    "Rust source changed without accompanying tests — consider a regression test for the behavior change."
  );
}

// ── Cargo.lock discipline ────────────────────────────────
const cargoTomlChanged = files.some((f) => f.endsWith("Cargo.toml"));
const lockChanged = files.includes("Cargo.lock");
if (cargoTomlChanged && !lockChanged) {
  nits.push(
    "`Cargo.toml` changed without `Cargo.lock` — run `cargo generate-lockfile` / build and commit the lockfile if dependencies changed."
  );
}
if (lockChanged && !cargoTomlChanged && !governanceOnly) {
  findings.push("`Cargo.lock` updated without manifest change — confirm intentional dependency refresh.");
}

// ── Public API / docs ────────────────────────────────────
if (isCore && touchesRust && !touchesDocs && /pub (fn|struct|enum|trait|type|mod)/.test(diff)) {
  nits.push(
    "New/changed `pub` items in core — ensure rustdoc comments exist (`missing_docs` is warned) and update architecture docs if the design shifted."
  );
}

// ── Workflow safety ──────────────────────────────────────
if (files.some((f) => f.startsWith(".github/workflows/"))) {
  if (/curl .+\|\s*(ba)?sh/i.test(diff)) {
    blockers.push(
      "Workflow pipes a remote script to a shell — avoid for supply-chain safety."
    );
  }
  if (/permissions:\s*\n\s*contents:\s*write/i.test(diff) && /pull_request_target/i.test(diff)) {
    blockers.push(
      "`pull_request_target` with write permissions is high risk — double-check fork PR safety."
    );
  }
  findings.push("CI/workflow files modified — verify least-privilege `permissions` and secret usage.");
}

// ── Large PRs ────────────────────────────────────────────
if (files.length > 60) {
  nits.push(
    `Large PR (${files.length} files). Prefer smaller, reviewable slices when possible for future phases.`
  );
}

// ── Direct main push is never acceptable (process note) ──
findings.push(
  "Process: feature branch → PR → CI Success → senior maintainer review → merge. Never push directly to `main`."
);

// ── Architecture / maintainability narrative ─────────────
const reviewLens = [
  "**Architecture** — crate boundaries, dependency direction (→ core), offline-first fit",
  "**Maintainability** — clarity, module size, naming, unnecessary complexity",
  "**Correctness** — error handling, invariants, config/logging edge cases",
  "**API design** — public surface stability, semver impact, discoverability",
  "**Documentation** — rustdoc, README/CHANGELOG/architecture when user-facing",
  "**Performance** — avoid needless clones/allocs on hot paths (when applicable)",
  "**Edge cases** — empty inputs, large graphs (future), invalid config",
  "**Testing** — unit coverage for new behavior; CI is authoritative",
];

const approved = blockers.length === 0;
const verdict = approved ? "APPROVE" : "REQUEST_CHANGES";

const body = [
  "## Senior maintainer review",
  "",
  "Acting as a **highly experienced open-source maintainer** for SimpLineage.",
  "Review focus:",
  "",
  ...reviewLens.map((l) => `- ${l}`),
  "",
  section("Verdict", [
    approved
      ? "**Approve** — no static merge blockers. **CI Success** must still be green before merge."
      : "**Request changes** — resolve blockers, push to the feature branch, and re-run the full CI + review cycle.",
  ]),
  section("Change map", [
    `- Files changed: **${files.length}**`,
    `- Rust: ${touchesRust ? "yes" : "no"} · Tests: ${touchesTests ? "yes" : "no"} · Docs: ${touchesDocs ? "yes" : "no"} · CI: ${touchesCi ? "yes" : "no"}`,
    docsOnly
      ? "- Classified as **docs-only**"
      : governanceOnly
        ? "- Classified as **governance / tooling**"
        : "- Includes application or infrastructure code",
    "",
    "<details><summary>Files</summary>",
    "",
    files.map((f) => `- \`${f}\``).join("\n") || "- _(none)_",
    "",
    "</details>",
  ]),
  blockers.length
    ? section(
        "Blockers (must fix)",
        blockers.map((b) => `- ${b}`)
      )
    : section("Blockers (must fix)", ["- None"]),
  nits.length
    ? section(
        "Detailed feedback / nits",
        nits.map((n) => `- ${n}`)
      )
    : section("Detailed feedback / nits", ["- None beyond normal CI expectations"]),
  findings.length
    ? section(
        "Notes",
        findings.map((f) => `- ${f}`)
      )
    : "",
  section("Maintainer checklist", [
    `- [${approved ? "x" : " "}] No secrets or credentials in the diff`,
    `- [${touchesTests || docsOnly || governanceOnly || !touchesRust ? "x" : " "}] Tests appropriate for the change`,
    "- [ ] PR body includes **Summary**, **Design decisions**, **Testing**, **Future considerations**",
    "- [x] Merge only via PR after **CI Success** + maintainer approval",
    "- [x] Squash merge preferred; delete feature branch after merge",
  ]),
  section("Review cycle", [
    "1. Author addresses every blocker (and ideally nits).",
    "2. Push to the **same feature branch** (never to `main`).",
    "3. Full CI pipeline re-runs automatically.",
    "4. Maintainer re-reviews until approved.",
    "5. Squash-merge into `main` when CI Success is green and review is clean.",
  ]),
  "---",
  "_Automated senior-maintainer bot for SimpLineage · see [docs/GOVERNANCE.md](../docs/GOVERNANCE.md)._",
  "<!-- maintainer-bot-review -->",
].join("\n");

fs.writeFileSync("/tmp/maintainer-review.md", body);
fs.writeFileSync(
  "/tmp/maintainer-verdict.json",
  JSON.stringify({ verdict, approved, blockers, nits, files }, null, 2)
);
if (process.env.GITHUB_STEP_SUMMARY) {
  fs.appendFileSync(process.env.GITHUB_STEP_SUMMARY, body + "\n");
}

console.log(body);
console.log(`\nVERDICT=${verdict}`);
process.exit(approved ? 0 : 1);
