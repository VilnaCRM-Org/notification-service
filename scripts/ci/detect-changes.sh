#!/usr/bin/env bash
# Classify the change of a pull request for the `changes` job of the workflows.
#
# Usage: detect-changes.sh >> "$GITHUB_OUTPUT"
# Reads BASE_SHA and HEAD_SHA (the base and the head commit of the pull request). Prints `code=false` when every
# changed path is documentation (`*.md`, `docs/**`, `LICENSE`, `.github/ISSUE_TEMPLATE/**`,
# `.github/pull_request_template.md`, `.claude/skills/**`, `specs/**`) and `code=true` otherwise. On a push or a
# schedule no pull-request commits are given and the answer is `code=true`; so it is when the change cannot be
# read or is empty, because a gate that is skipped by mistake is worse than one that runs.
# Exit status: 0 always; the only output on standard output is the `code=` line.
set -euo pipefail

code=true

if [ -n "${BASE_SHA-}" ] && [ -n "${HEAD_SHA-}" ]; then
  if changed=$(git -c core.quotepath=off diff --name-only --no-renames "${BASE_SHA}...${HEAD_SHA}" 2>/dev/null); then
    documentation_only=true
    seen=false
    while IFS= read -r path; do
      [ -n "$path" ] || continue
      seen=true
      # `*.md` also covers .github/pull_request_template.md.
      case "$path" in
        *.md | docs/* | LICENSE | .github/ISSUE_TEMPLATE/* | .claude/skills/* | specs/*) ;;
        *)
          documentation_only=false
          break
          ;;
      esac
    done <<<"$changed"
    if [ "$seen" = true ] && [ "$documentation_only" = true ]; then
      code=false
    fi
  else
    echo "detect-changes.sh: could not compare ${BASE_SHA} with ${HEAD_SHA}; treating the change as code" >&2
  fi
fi

echo "code=${code}"
