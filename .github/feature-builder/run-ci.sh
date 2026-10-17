#!/usr/bin/env bash
# Runs the CI workflow on a branch and waits for it (docs/feature-requests.md). The feature
# builder's pushes are made with the workflow's own token, which doesn't start workflows by
# itself, so the builder starts CI explicitly ("Run workflow" is allowed). Prints the run's
# address; exits non-zero if CI fails. A run cancelled by a newer one on the same branch is
# started again, once.
#
#   .github/feature-builder/run-ci.sh <branch>
set -euo pipefail
branch=$1
for try in 1 2; do
  since=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  gh workflow run ci.yml --ref "$branch"
  id=""
  for _ in $(seq 1 60); do
    id=$(gh run list --workflow ci.yml --branch "$branch" --event workflow_dispatch --limit 5 \
      --json databaseId,createdAt --jq "[.[] | select(.createdAt >= \"$since\")][0].databaseId // empty")
    [ -n "$id" ] && break
    sleep 5
  done
  if [ -z "$id" ]; then
    echo "CI didn't start on $branch."
    exit 1
  fi
  url="$GITHUB_SERVER_URL/$GITHUB_REPOSITORY/actions/runs/$id"
  gh run watch "$id" --interval 30 > /dev/null || true
  conclusion=$(gh run view "$id" --json conclusion --jq .conclusion)
  if [ "$conclusion" = success ]; then
    echo "$url"
    exit 0
  fi
  if [ "$conclusion" != cancelled ] || [ "$try" = 2 ]; then
    echo "CI ended with \"$conclusion\": $url"
    exit 1
  fi
done
