#!/usr/bin/env bash
# Fail on a breaking change of the GraphQL snapshot against a base ref (architecture-delivery.md DD-10,
# DD-22, "Command Surface").
#
# Usage: scripts/graphql-diff.sh [<base ref>]    (default origin/main)
#
# Reads NODE_IMAGE and GRAPHQL_INSPECTOR_VERSION from the environment (Make exports them); the optional
# BREAKING_CHANGE_APPROVED=true prints the findings and passes; DOCKER overrides the container engine
# command. Mounts are read-only and the container runs as the invoking user.
# Exit status: 0 compatible, no snapshot on the base ref (notice), or approved; 1 breaking change;
# 2 base ref not resolvable, head snapshot missing, or the tool itself failed; an unset variable exits 1 (the ${NAME:?} check).
set -euo pipefail

: "${NODE_IMAGE:?NODE_IMAGE is not set; run through make}"
: "${GRAPHQL_INSPECTOR_VERSION:?GRAPHQL_INSPECTOR_VERSION is not set; run through make}"

base_ref="${1:-origin/main}"
snapshot=".github/graphql-spec/spec"
root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
head_dir="${root_dir}/.github/graphql-spec"

if [[ ! -f "${head_dir}/spec" ]]; then
  echo "Error: ${head_dir}/spec not found. Run 'make generate-graphql-spec' first." >&2
  exit 2
fi

if ! git -C "${root_dir}" rev-parse --verify --quiet "${base_ref}^{commit}" >/dev/null; then
  echo "Error: base ref '${base_ref}' cannot be resolved." >&2
  exit 2
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "${tmp_dir}" "${log_file:-}"' EXIT

if ! git -C "${root_dir}" show "${base_ref}:${snapshot}" >"${tmp_dir}/spec.graphql" 2>/dev/null; then
  echo "Notice: ${base_ref} holds no ${snapshot}; this is the first version of the contract, nothing to compare."
  exit 0
fi

status=0
log_file="$(mktemp)"
# The snapshot has no file extension, which graphql-inspector needs to recognise a schema file, so both
# files are mounted as spec.graphql. The root file system is read-only apart from a tmpfs that holds the
# npm cache and home (npx runs the package binary from there, so the tmpfs allows execution).
"${DOCKER:-docker}" run --rm \
  --user "$(id -u):$(id -g)" \
  --read-only \
  --tmpfs /tmp:exec \
  -e HOME=/tmp \
  -e npm_config_cache=/tmp/npm \
  -v "${tmp_dir}:/base:ro" \
  -v "${head_dir}/spec:/head/spec.graphql:ro" \
  "${NODE_IMAGE}" \
  npx --yes "@graphql-inspector/cli@${GRAPHQL_INSPECTOR_VERSION}" diff /base/spec.graphql /head/spec.graphql >"${log_file}" 2>&1 || status=$?
cat "${log_file}"

if [[ "${status}" -eq 0 ]]; then
  exit 0
fi

# graphql-inspector exits 1 both for a breaking change and for a schema it cannot load; only the former
# reports "breaking change".
if [[ "${status}" -ne 1 ]] || ! grep -q 'breaking change' "${log_file}"; then
  echo "Error: the comparison tool failed with exit status ${status}." >&2
  exit 2
fi

if [[ "${BREAKING_CHANGE_APPROVED:-}" == "true" ]]; then
  echo "Breaking changes found; approved by BREAKING_CHANGE_APPROVED=true."
  exit 0
fi

echo "Breaking changes found against ${base_ref}. Approve with the label approved-breaking-change" >&2
exit 1
