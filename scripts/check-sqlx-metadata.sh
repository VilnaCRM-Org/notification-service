#!/usr/bin/env bash
# Run the query-metadata check of G30 and fail on a file of .sqlx that no current query generates.
#
# Usage: check-sqlx-metadata.sh <command>...
# Runs the command (`cargo sqlx prepare --workspace --check ...`), prints its output and keeps its exit status.
# sqlx-cli fails for a missing or a differing query file, but for a committed file that no current query generates it
# only prints `potentially unused queries found in .sqlx` and succeeds; this script turns that warning into a failure.
# Exit status: the command's when it fails; 1 when it passes but reports such a file; 2 when no command is given.
set -euo pipefail

[ "$#" -gt 0 ] || { echo "usage: check-sqlx-metadata.sh <command>..." >&2; exit 2; }

output=$(mktemp)
trap 'rm -f -- "$output"' EXIT

set +e
"$@" 2>&1 | tee -- "$output"
status=${PIPESTATUS[0]}
set -e
[ "$status" -eq 0 ] || exit "$status"

if grep -F -q -e 'potentially unused queries found in .sqlx' "$output"; then
  echo "check-sqlx-metadata.sh: .sqlx holds a query file that no current query generates; remove it or run make sqlx-prepare" >&2
  exit 1
fi
