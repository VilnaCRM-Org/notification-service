#!/usr/bin/env bash
# Lint the OpenAPI snapshot with Spectral and the repository ruleset (architecture-delivery.md DD-22,
# "Command Surface").
#
# Usage: scripts/validate-openapi-spec.sh
#
# Reads SPECTRAL_IMAGE and SPECTRAL_FLAGS from the environment (Make exports them); DOCKER overrides the
# container engine command. Mounts are read-only and the container runs as the invoking user.
# Exit status: 0 no findings; 1 findings or an unset variable (the ${NAME:?} check); 2 snapshot or ruleset missing.
set -euo pipefail

: "${SPECTRAL_IMAGE:?SPECTRAL_IMAGE is not set; run through make}"
: "${SPECTRAL_FLAGS:?SPECTRAL_FLAGS is not set; run through make}"

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
openapi_dir="${root_dir}/.github/openapi-spec"
ruleset="${root_dir}/.spectral.yaml"

if [[ ! -f "${openapi_dir}/spec.yaml" ]]; then
  echo "Error: ${openapi_dir}/spec.yaml not found. Run 'make generate-openapi-spec' first." >&2
  exit 2
fi

if [[ ! -f "${ruleset}" ]]; then
  echo "Error: Spectral ruleset ${ruleset} not found." >&2
  exit 2
fi

format_args=()
if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
  format_args=(--format github-actions)
fi

# SPECTRAL_FLAGS is a space-separated list of flags and is split on purpose.
# shellcheck disable=SC2086
"${DOCKER:-docker}" run --rm \
  --user "$(id -u):$(id -g)" \
  -v "${openapi_dir}:/workspace/openapi:ro" \
  -v "${ruleset}:/workspace/.spectral.yaml:ro" \
  "${SPECTRAL_IMAGE}" \
  lint /workspace/openapi/spec.yaml \
  --ruleset /workspace/.spectral.yaml \
  "${format_args[@]}" \
  ${SPECTRAL_FLAGS}
