#!/usr/bin/env bash
# Size the mutation matrix of the `Tests` workflow (architecture-delivery.md DD-09).
#
# Usage: mutation-plan.sh >> "$GITHUB_OUTPUT"
# Reads EVENT_NAME and BASE_REF. On a pull request it counts the mutants of the changed code
# (`scripts/mutation-diff.sh "origin/$BASE_REF" --list -- cargo mutants --workspace`); otherwise it counts the
# mutants of the whole scope (`cargo mutants --workspace --list`). Prints `count=<n>` and `shards=<JSON>`: one
# `{"index":k,"shard":"<k-1>/n"}` object per shard, n being the count divided by 150 rounded up, at least 1 and
# at most 8; `shards=[]` for 0 mutants. `index` counts from 1 (it names the artifact and picks the shard that
# saves the Rust cache); `shard` is the value of `cargo mutants --shard`, whose first shard is 0.
# Run it at the root of the repository (the directory above scripts/).
# Exit status: 0; 2 the mutants could not be counted (a base ref that cannot be resolved, a missing variable).
set -euo pipefail

per_shard=150
max_shards=8

fail() {
  echo "mutation-plan.sh: $1" >&2
  exit 2
}

[ -n "${EVENT_NAME-}" ] || fail "EVENT_NAME is not set"
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

if [ "$EVENT_NAME" = pull_request ]; then
  [ -n "${BASE_REF-}" ] || fail "BASE_REF is not set"
  output=$(scripts/mutation-diff.sh "origin/${BASE_REF}" --list -- cargo mutants --workspace) ||
    fail "the mutants of the diff against origin/${BASE_REF} could not be listed"
  # mutation-diff.sh prints the number of mutants; a diff without production Rust code may print only cargo-mutants'
  # "No mutants to filter".
  last=$(printf '%s\n' "$output" | awk 'NF { line = $0 } END { print line }')
  if [[ "$last" =~ ^[[:space:]]*([0-9]+)[[:space:]]*$ ]]; then
    count=${BASH_REMATCH[1]}
  elif [ -z "$last" ] || [[ "$output" == *"No mutants to filter"* ]]; then
    count=0
  else
    fail "unexpected output of scripts/mutation-diff.sh: ${last}"
  fi
else
  output=$(cargo mutants --workspace --list) || fail "the mutants could not be listed"
  count=$(printf '%s\n' "$output" | grep -c . || true)
fi

shard_count=$(((count + per_shard - 1) / per_shard))
[ "$shard_count" -le "$max_shards" ] || shard_count=$max_shards

shards='['
for ((index = 1; index <= shard_count; index++)); do
  [ "$index" -eq 1 ] || shards+=','
  shards+="{\"index\":${index},\"shard\":\"$((index - 1))/${shard_count}\"}"
done
shards+=']'

echo "count=${count}"
echo "shards=${shards}"
