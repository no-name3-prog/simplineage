#!/usr/bin/env bash
# Protect main for a solo (or small) maintainer team.
#
# - PRs required (no direct push to main)
# - CI Success required
# - Approving reviews: 0 by default — GitHub disallows self-approval, so requiring
#   1 review blocks the repo owner from merging their own PRs.
#   Human "final approval" = reading the PR + clicking squash-merge.
# - When you have a second reviewer, set REQUIRED_REVIEWS=1.
set -euo pipefail

REPO="${GITHUB_REPOSITORY:-$(gh repo view --json nameWithOwner -q .nameWithOwner)}"
BRANCH="${1:-main}"
REQUIRED_REVIEWS="${REQUIRED_REVIEWS:-0}"

echo "Applying branch protection on ${REPO}@${BRANCH} (required reviews: ${REQUIRED_REVIEWS})..."

gh api --method PUT \
  -H "Accept: application/vnd.github+json" \
  "/repos/${REPO}/branches/${BRANCH}/protection" \
  --input - <<JSON
{
  "required_status_checks": {
    "strict": true,
    "contexts": ["CI Success"]
  },
  "enforce_admins": true,
  "required_pull_request_reviews": {
    "required_approving_review_count": ${REQUIRED_REVIEWS},
    "dismiss_stale_reviews": true,
    "require_code_owner_reviews": false,
    "require_last_push_approval": false
  },
  "restrictions": null,
  "allow_force_pushes": false,
  "allow_deletions": false,
  "required_conversation_resolution": false,
  "lock_branch": false,
  "allow_fork_syncing": true
}
JSON

gh repo edit "$REPO" \
  --enable-squash-merge \
  --enable-merge-commit=false \
  --enable-rebase-merge=false \
  --delete-branch-on-merge \
  2>/dev/null || true

echo "Done."
echo "  Required check: CI Success"
echo "  Required approving reviews: ${REQUIRED_REVIEWS}"
echo "  Direct pushes to ${BRANCH}: blocked"
echo "  Final merge: human maintainer squash-merge on the PR"
