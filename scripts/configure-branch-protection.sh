#!/usr/bin/env bash
# Requires `gh auth login` with admin rights on the repository.
set -euo pipefail

repo="$(gh repo view --json nameWithOwner -q .nameWithOwner)"

gh api -X PUT "repos/${repo}/branches/main/protection" \
  --input - <<EOF
{
  "required_status_checks": {
    "strict": true,
    "checks": [
      {"context": "lint", "app_id": null},
      {"context": "ci-ok", "app_id": null}
    ]
  },
  "enforce_admins": false,
  "required_pull_request_reviews": {
    "dismiss_stale_reviews": true,
    "required_approving_review_count": 1
  },
  "restrictions": null
}
EOF

echo "Branch protection updated for ${repo}@main"
