#!/usr/bin/env bash
# Fail on a breaking change of the OpenAPI snapshot against a base ref (architecture-delivery.md DD-10,
# DD-22, "Command Surface").
#
# Usage: scripts/openapi-diff.sh [<base ref>]    (default origin/main)
#
# Reads OPENAPI_DIFF_IMAGE and OASDIFF_FLAGS from the environment (Make exports them); the optional
# BREAKING_CHANGE_APPROVED=true prints the findings and passes; DOCKER overrides the container engine
# command. Mounts are read-only and the container runs as the invoking user.
# Exit status: 0 compatible, no snapshot on the base ref (notice), or approved; 1 breaking change;
# 2 base ref not resolvable, head snapshot missing, or the tool itself failed; an unset variable exits 1 (the ${NAME:?} check).
set -euo pipefail

: "${OPENAPI_DIFF_IMAGE:?OPENAPI_DIFF_IMAGE is not set; run through make}"
: "${OASDIFF_FLAGS:?OASDIFF_FLAGS is not set; run through make}"

base_ref="${1:-origin/main}"
snapshot=".github/openapi-spec/spec.yaml"
root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
head_dir="${root_dir}/.github/openapi-spec"

if [[ ! -f "${head_dir}/spec.yaml" ]]; then
  echo "Error: ${head_dir}/spec.yaml not found. Run 'make generate-openapi-spec' first." >&2
  exit 2
fi

if ! git -C "${root_dir}" rev-parse --verify --quiet "${base_ref}^{commit}" >/dev/null; then
  echo "Error: base ref '${base_ref}' cannot be resolved." >&2
  exit 2
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "${tmp_dir}"' EXIT

if ! git -C "${root_dir}" show "${base_ref}:${snapshot}" >"${tmp_dir}/spec.yaml" 2>/dev/null; then
  echo "Notice: ${base_ref} holds no ${snapshot}; this is the first version of the contract, nothing to compare."
  exit 0
fi

status=0
# OASDIFF_FLAGS is a space-separated list of flags and is split on purpose.
# shellcheck disable=SC2086
"${DOCKER:-docker}" run --rm \
  --user "$(id -u):$(id -g)" \
  -v "${tmp_dir}:/base:ro" \
  -v "${head_dir}:/head:ro" \
  "${OPENAPI_DIFF_IMAGE}" \
  breaking /base/spec.yaml /head/spec.yaml ${OASDIFF_FLAGS} || status=$?

if [[ "${status}" -eq 0 ]]; then
  exit 0
fi

if [[ "${status}" -ne 1 ]]; then
  echo "Error: the comparison tool failed with exit status ${status}." >&2
  exit 2
fi

if [[ "${BREAKING_CHANGE_APPROVED:-}" == "true" ]]; then
  echo "Breaking changes found; approved by BREAKING_CHANGE_APPROVED=true."
  exit 0
fi

echo "Breaking changes found against ${base_ref}. Approve with the label approved-breaking-change" >&2
exit 1
