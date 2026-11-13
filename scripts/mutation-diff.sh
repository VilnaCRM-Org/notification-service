#!/usr/bin/env bash
# Run a mutation command over the Rust code changed against a base ref (architecture-delivery.md DD-09,
# "Command Surface"; architecture.md QG-18).
#
# Usage: scripts/mutation-diff.sh <base_ref> [--list] -- <mutation command...>
#
# Resolves the base ref, writes `git diff "$(git merge-base <base_ref> HEAD)" -- '*.rs'` to target/mutants.diff
# (the diff runs from the merge base to the working tree, so uncommitted changes of tracked files are judged too)
# with every setting that changes the patch format pinned on the command line, so the file reads the same on every
# machine whatever the git configuration of the user says: `a/` and `b/` prefixes (diff.mnemonicPrefix,
# diff.noprefix), no colour, paths relative to the repository root (diff.relative), no external diff, no textconv,
# no rename detection, unquoted paths (core.quotePath) and three lines of context (GIT_DIFF_OPTS). It runs the
# given mutation command with `--in-diff target/mutants.diff` appended. With `--list` it appends `--list` as well
# and prints only the number of mutants. The script holds no cargo command of its own: the command comes from the
# caller (the Make target runs it behind scripts/with-env.sh).
# Run it from anywhere inside the repository; it works at the repository root.
# Exit status: the status of the mutation command (cargo-mutants: 0, 2 survivors, 3 timeouts); 0 when the diff
# holds no production Rust change; 2 with `mutation-tests-diff could not run: <reason>` when the base ref is not
# available, the arguments are wrong, or the diff cannot be written.
set -euo pipefail

usage='Usage: scripts/mutation-diff.sh <base_ref> [--list] -- <mutation command...>'

fail() {
  echo "mutation-tests-diff could not run: $1" >&2
  exit 2
}

[ "$#" -ge 1 ] || fail "$usage"
base_ref=$1
shift
list=false
if [ "${1-}" = "--list" ]; then
  list=true
  shift
fi
[ "${1-}" = "--" ] || fail "$usage"
shift
[ "$#" -ge 1 ] || fail "$usage"

cd "$(dirname "${BASH_SOURCE[0]}")/.."

if [ -z "$base_ref" ] || ! git rev-parse --verify --quiet "${base_ref}^{commit}" >/dev/null; then
  fail "base ref ${base_ref} is not available"
fi
merge_base=$(git merge-base "$base_ref" HEAD 2>/dev/null) ||
  fail "base ref ${base_ref} is not available: it shares no history with HEAD"

diff_file=target/mutants.diff
mkdir -p target
env -u GIT_DIFF_OPTS -u GIT_EXTERNAL_DIFF git \
  -c core.quotePath=false -c diff.mnemonicPrefix=false -c diff.noprefix=false -c diff.relative=false \
  -c diff.renames=false -c diff.context=3 -c color.ui=false -c color.diff=false \
  diff --no-ext-diff --no-textconv --no-color --no-relative --no-renames --unified=3 \
  --src-prefix=a/ --dst-prefix=b/ "$merge_base" -- '*.rs' >"$diff_file" || fail "the diff against ${base_ref} could not be written"

if [ ! -s "$diff_file" ]; then
  echo "mutation-tests-diff: the diff against ${base_ref} holds no production Rust change; there are no mutants to test" >&2
  [ "$list" = false ] || echo 0
  exit 0
fi

if [ "$list" = false ]; then
  exec "$@" --in-diff "$diff_file"
fi

# The list goes to standard output one mutant per line; only its length is printed.
status=0
mutants=$("$@" --in-diff "$diff_file" --list) || status=$?
[ "$status" -eq 0 ] || exit "$status"
printf '%s\n' "$mutants" | grep -c . || true
