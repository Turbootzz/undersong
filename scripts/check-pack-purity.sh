#!/usr/bin/env bash
# Gate P8: the Skalden pack commits may touch ONLY content/ + assets/.
# Usage: scripts/check-pack-purity.sh <from-ref> <to-ref>
set -euo pipefail
FROM="${1:?from ref}"; TO="${2:?to ref}"
IMPURE=$(git diff --name-only "$FROM".."$TO" | grep -vE '^(content/|assets/)' || true)
if [ -n "$IMPURE" ]; then
  echo "pack purity violated — engine paths in the pack diff:" >&2
  echo "$IMPURE" >&2
  exit 1
fi
echo "pack purity holds: only content/ + assets/ between $FROM and $TO"
