#!/usr/bin/env bash
# Check the commit convention (architecture-delivery.md "Git Hooks and Commit Convention").
#
# Hook mode:  COMMIT_MSG_FILE=<message file> scripts/check-commit-message.sh
# Title mode: COMMIT_TITLE=<subject> scripts/check-commit-message.sh
#
# Both inputs come from the environment so that a title can never inject shell text into a recipe.
# Hook mode checks the first line that is not a comment and accepts, without checking, the subjects
# that git writes itself (Merge, Revert, Reapply, fixup!, squash!, amend!). Title mode has no
# exemption. For a revert of a revert, git 2.43 and later write `Reapply "<subject>"`, older git
# `Revert "Revert "<subject>""`.
# Exit status: 0 accepted; 1 rejected, with the expected format; 2 neither variable is set or the
# message file cannot be read.
set -euo pipefail

pattern='^(feat|fix|refactor|build|chore|ci|docs|style|perf|test)\(#[0-9]+\): .*'
format='feat|fix|refactor|build|chore|ci|docs|style|perf|test(#1): description'

hook_mode=0
if [[ -n "${COMMIT_MSG_FILE:-}" ]]; then
  hook_mode=1
  if [[ ! -r "${COMMIT_MSG_FILE}" ]]; then
    echo "check-commit-message: cannot read ${COMMIT_MSG_FILE}" >&2
    exit 2
  fi
  subject=""
  while IFS= read -r line || [[ -n "${line}" ]]; do
    if [[ "${line}" != "#"* ]]; then
      subject="${line}"
      break
    fi
  done <"${COMMIT_MSG_FILE}"
elif [[ -n "${COMMIT_TITLE:-}" ]]; then
  subject="${COMMIT_TITLE%%$'\n'*}"
else
  echo "check-commit-message: set COMMIT_MSG_FILE (hook mode) or COMMIT_TITLE (title mode)" >&2
  exit 2
fi
subject="${subject%$'\r'}"

if [[ "${hook_mode}" -eq 1 ]]; then
  case "${subject}" in
    "Merge "* | 'Revert "'* | 'Reapply "'* | "fixup! "* | "squash! "* | "amend! "*) exit 0 ;;
    *) ;;
  esac
fi

if printf '%s\n' "${subject}" | grep -Eiq "${pattern}"; then
  exit 0
fi

echo "The commit message should be in the format: ${format}" >&2
echo "got: ${subject}" >&2
exit 1
