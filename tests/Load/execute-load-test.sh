#!/usr/bin/env bash
# Run one K6 scenario in the pinned stock K6 image and write its HTML dashboard to
# tests/Load/results/<htmlPrefix><scenario>.html.
set -euo pipefail

usage="Usage: execute-load-test.sh <scenario> [runSmoke] [runAverage] [runStress] [runSpike] [htmlPrefix]"

LOAD_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${LOAD_DIR}/../.." && pwd)"

scenario="${1:-}"
run_smoke="${2:-true}"
run_average="${3:-true}"
run_stress="${4:-true}"
run_spike="${5:-true}"
html_prefix="${6:-}"

if [[ -z "${scenario}" ]] || ! "${LOAD_DIR}/get-load-test-scenarios.sh" | grep -qxF -- "${scenario}"; then
  echo "${usage}" >&2
  exit 2
fi

# The K6 image is pinned in make/tool-versions.mk (lines of the form `export NAME := value`).
if [[ -z "${K6_IMAGE:-}" ]]; then
  K6_IMAGE="$(sed -n 's/^export K6_IMAGE[[:space:]]*:=[[:space:]]*//p' "${REPO_ROOT}/make/tool-versions.mk" | head -n 1)"
fi
if [[ -z "${K6_IMAGE}" ]]; then
  echo "Error: K6_IMAGE is not set and not found in make/tool-versions.mk." >&2
  exit 1
fi

echo "Executing load test for scenario: ${scenario}"
echo "Options - Smoke: ${run_smoke}, Average: ${run_average}, Stress: ${run_stress}, Spike: ${run_spike}"

mkdir -p "${LOAD_DIR}/results"

docker run --rm --network host --user "$(id -u):$(id -g)" \
  -v "${LOAD_DIR}:/loadTests" -e API_HOST -e API_PORT \
  -e K6_WEB_DASHBOARD=true -e K6_WEB_DASHBOARD_PORT=-1 -e K6_WEB_DASHBOARD_PERIOD=1s \
  -e K6_WEB_DASHBOARD_EXPORT="/loadTests/results/${html_prefix}${scenario}.html" \
  "${K6_IMAGE}" run --summary-trend-stats='avg,min,med,max,p(95),p(99)' "/loadTests/scripts/${scenario}.js" \
  -e run_smoke="${run_smoke}" -e run_average="${run_average}" -e run_stress="${run_stress}" -e run_spike="${run_spike}"
