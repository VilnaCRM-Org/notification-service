#!/usr/bin/env bash
# Set [workspace.package].version, refresh Cargo.lock and verify it (pre-bump hook of cog.toml; the
# release job runs it through cocogitto, architecture-delivery.md DD-11, PF-07).
#
# Usage: scripts/set-version.sh <version>
# Exit status: 0 success; 1 failure (bad version, no version line, cargo failure).
set -euo pipefail

if [[ $# -ne 1 || ! "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "usage: scripts/set-version.sh <major.minor.patch>" >&2
  exit 1
fi
version="$1"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$(dirname "${script_dir}")"

tmp="$(mktemp "Cargo.toml.XXXXXX")"
trap 'rm -f "${tmp}"' EXIT

# Replace the first `version = ...` line inside the [workspace.package] table only.
if ! awk -v version="${version}" '
  /^\[/ { in_table = ($0 == "[workspace.package]") }
  in_table && !done && /^version[[:space:]]*=/ { print "version = \"" version "\""; done = 1; next }
  { print }
  END { if (!done) exit 3 }
' Cargo.toml >"${tmp}"; then
  echo "set-version: no version line in [workspace.package] of Cargo.toml" >&2
  exit 1
fi
cat "${tmp}" >Cargo.toml

cargo update --workspace
cargo metadata --locked --format-version 1 >/dev/null
