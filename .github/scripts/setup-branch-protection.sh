#!/usr/bin/env bash
# Protect main: PR-only, CI Success, approving review, no direct pushes.
# Requires: gh auth with admin rights on the repository.
set -euo pipefail

REPO="${GITHUB_REPOSITORY:-$(gh repo view --json nameWithOwner -q .nameWithOwner)}"
BRANCH="${1:-main}"

echo "Applying branch protection on ${REPO}@${BRANCH}..."

# GitHub API: branch protection
gh api --method PUT \
  -H "Accept: application/vnd.github+json" \
  "/repos/${REPO}/branches/${BRANCH}/protection" \
  --input - <<'JSON'
{
  "required_status_checks": {
    "strict": true,
    "contexts": ["CI Success"]
  },
  "enforce_admins": true,
  "required_pull_request_reviews": {
    "required_approving_review_count": 1,
    "dismiss_stale_reviews": true,
    "require_code_owner_reviews": false,
    "require_last_push_approval": false
  },
  "restrictions": null,
  "allow_force_pushes": false,
  "allow_deletions": false,
  "block_creations": false,
  "required_conversation_resolution": true,
  "lock_branch": false,
  "allow_fork_syncing": true
}
JSON

# Repo preferences: squash-only, delete branch on merge, no auto-merge required
gh repo edit "$REPO" \
  --enable-squash-merge \
  --enable-merge-commit=false \
  --enable-rebase-merge=false \
  --delete-branch-on-merge \
  --allow-update-branch \
  2>/dev/null || true

# Best-effort: default branch name
echo "Repository merge settings updated (squash preferred)."
echo "Protected branch: ${BRANCH}"
echo "Required check: CI Success"
echo "Required: pull request + 1 approving review + conversation resolution"
echo "Direct pushes to ${BRANCH} are blocked (enforce_admins=true)."
