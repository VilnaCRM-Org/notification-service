#!/usr/bin/env bash
# Assistant hook: keep the protected quality configuration out of reach of an edit.
#
# Usage: guard-protected-files.sh <pre|post>   (hook JSON on standard input)
#
# The protected items come from `cargo xtask config-guard --list`, cached in
# target/claude-guard/items.txt and refreshed when protected-config.toml is newer.
#   pre : a protected file, or a file below a protected directory, is refused (exit 2). For a file that holds
#         a protected section the state before the edit is recorded as a commit object (git stash create).
#   post: for such a file, `cargo xtask config-guard --base <recorded commit>` decides whether the edit changed
#         a protected table or value; if so the file is restored from the recorded commit and the exit status is 2.
# ALLOW_PROTECTED_CONFIG_EDIT=1 turns both modes into a notice. Without cargo, git or jq, or when the guard
# cannot run, the script warns and exits 0; CI remains the enforcement.
set -euo pipefail

mode=${1:-}
case "$mode" in
  pre | post) ;;
  *)
    echo "usage: guard-protected-files.sh <pre|post>" >&2
    exit 2
    ;;
esac

if [ "${ALLOW_PROTECTED_CONFIG_EDIT:-}" = "1" ]; then
  echo "Notice: ALLOW_PROTECTED_CONFIG_EDIT=1 is set; the protected configuration guard is off. The pull request needs the label approved-config-change." >&2
  exit 0
fi

for tool in cargo git jq; do
  if ! command -v "$tool" > /dev/null 2>&1; then
    echo "Warning: $tool is not on PATH; the protected configuration guard did not run. CI enforces the list." >&2
    exit 0
  fi
done

root=${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2> /dev/null || true)}
if [ -z "$root" ] || ! cd "$root" 2> /dev/null; then
  echo "Warning: the project directory is unknown; the protected configuration guard did not run." >&2
  exit 0
fi

path=$(jq -r '.tool_input.file_path // .tool_input.notebook_path // empty' 2> /dev/null || true)
[ -n "$path" ] || exit 0
case "$path" in
  "$root"/*) path=${path#"$root"/} ;;
  /*) exit 0 ;;
esac
path=${path#./}

state_dir=target/claude-guard
items=$state_dir/items.txt
mkdir -p "$state_dir"

refresh_items() {
  local tmp
  tmp=$(mktemp "$state_dir/items.XXXXXX")
  if ! env -u CONFIG_CHANGE_APPROVED cargo xtask config-guard --list > "$tmp" 2> /dev/null; then
    rm -f "$tmp"
    return 1
  fi
  mv "$tmp" "$items"
}

if [ ! -s "$items" ] || [ protected-config.toml -nt "$items" ]; then
  if ! refresh_items; then
    echo "Warning: cargo xtask config-guard --list failed; the protected configuration guard did not run." >&2
    exit 0
  fi
fi

message() {
  echo "Protected quality configuration: $1. Do not change it to make a check pass. A maintainer can approve the change: restart the session with ALLOW_PROTECTED_CONFIG_EDIT=1 and add the label approved-config-change to the pull request." >&2
}

# kind is "whole" for a protected file or directory, "section" for a file that holds a protected section.
kind=
while IFS= read -r item || [ -n "$item" ]; do
  [ -n "$item" ] || continue
  case "$item" in
    */)
      case "$path" in
        "$item"*) kind=whole ;;
      esac
      ;;
    *#*)
      if [ "${item%%#*}" = "$path" ] && [ -z "$kind" ]; then
        kind=section
      fi
      ;;
    *)
      if [ "$item" = "$path" ]; then
        kind=whole
      fi
      ;;
  esac
done < "$items"

[ -n "$kind" ] || exit 0

base_file=$state_dir/base
base_path=$state_dir/base-path

if [ "$mode" = "pre" ]; then
  if [ "$kind" = "whole" ]; then
    message "$path"
    exit 2
  fi
  if ! git rev-parse --verify --quiet HEAD > /dev/null; then
    echo "Warning: the repository has no commit yet; the protected configuration guard did not run." >&2
    exit 0
  fi
  id=$(git stash create 2> /dev/null || true)
  printf '%s\n' "${id:-$(git rev-parse HEAD)}" > "$base_file"
  printf '%s\n' "$path" > "$base_path"
  exit 0
fi

# post
[ "$kind" = "section" ] || exit 0
[ -s "$base_file" ] && [ -s "$base_path" ] || exit 0
[ "$(cat "$base_path")" = "$path" ] || exit 0
base=$(cat "$base_file")
rm -f "$base_file" "$base_path"

guard_status=0
report=$(env -u CONFIG_CHANGE_APPROVED cargo xtask config-guard --base "$base" 2> "$state_dir/guard.err") || guard_status=$?
if [ "$guard_status" -ne 0 ] && grep -qF 'configuration guard could not run' "$state_dir/guard.err"; then
  reason=$(grep -F 'configuration guard could not run' "$state_dir/guard.err" | head -n 1 | sed 's/^.*configuration guard could not run: *//')
  echo "Warning: the protected configuration guard could not run: ${reason}; CI enforces the list." >&2
  exit 0
fi
changed=$(printf '%s\n' "$report" | grep -F 'Modification of locked configuration' | grep -F ": $path " || true)
if [ -n "$changed" ]; then
  git restore --source="$base" --worktree -- "$path"
  message "$path"
  printf 'The edit was undone: %s\n' "$changed" >&2
  exit 2
fi
exit 0
