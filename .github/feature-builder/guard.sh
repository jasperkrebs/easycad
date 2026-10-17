#!/usr/bin/env bash
# The feature builder's merge rules (docs/feature-requests.md), checked by the workflow, not by
# the AI: a change Claude Code made may be merged without a person only if it stays inside what
# a new step needs. Lives under .github/, which the builder may not change.
#
#   .github/feature-builder/guard.sh <base> <head>
#
# Prints the reason and exits 0 (fine to merge), 2 (a person must decide: protected files, a new
# dependency, fewer tests, too big) or 3 (merge after a person's review: the G-code writer).
set -euo pipefail
base=$1
head=$2
changed=$(git diff --name-only "$base"..."$head")
if [ -z "$changed" ]; then
  echo "The branch changes nothing."
  exit 2
fi

# Files that only a person changes: CI and these rules, the AI proxy, EasyGCode's fixtures and
# the script that makes them, the project rules, dependencies and licenses, vendored code.
protected=$(printf '%s\n' "$changed" | grep -E '^(\.github/|agent-proxy/|fixtures/easygcode/|scripts/easygcode-fixtures\.cjs$|CLAUDE\.md$|Cargo\.(lock|toml)$|crates/[^/]+/Cargo\.toml$|THIRD-PARTY-NOTICES\.md$|web/vendor/|web/config\.json$|LICENSE)' || true)
if [ -n "$protected" ]; then
  echo "It changes files only a person may change: $(echo "$protected" | tr '\n' ' ')"
  exit 2
fi

# Tests may be added, never removed.
count_tests() {
  git grep -c -e '#\[test\]' -e '^test(' "$1" -- crates agent-proxy scripts 2>/dev/null | awk -F: '{s += $NF} END {print s + 0}'
}
before=$(count_tests "$base")
after=$(count_tests "$head")
if [ "$after" -lt "$before" ]; then
  echo "It has fewer tests than before ($after instead of $before)."
  exit 2
fi

# One step at a time: big changes get a person's eye.
lines=$(git diff --numstat "$base"..."$head" -- . ':(exclude)fixtures/hand-checked' | awk '{s += $1 + $2} END {print s + 0}')
if [ "$lines" -gt 3000 ]; then
  echo "It changes $lines lines; more than 3000 needs a person."
  exit 2
fi

# G-code drives real machines: a change to the writer waits for a person's review.
if printf '%s\n' "$changed" | grep -qE '^crates/core/src/gcode\.rs$'; then
  echo "It changes the G-code writer (crates/core/src/gcode.rs)."
  exit 3
fi

echo "Inside the rules: $(printf '%s\n' "$changed" | wc -l) files, $lines lines, $after tests (was $before)."
