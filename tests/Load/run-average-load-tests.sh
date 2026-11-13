#!/usr/bin/env bash
# Run every scenario of tests/Load/scripts with the average profile.
set -euo pipefail

LOAD_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

scenarios="$("${LOAD_DIR}/get-load-test-scenarios.sh")"

if [[ -z "${scenarios}" ]]; then
  echo "Error: No load test scenarios found." >&2
  exit 1
fi

for scenario in ${scenarios}; do
  "${LOAD_DIR}/execute-load-test.sh" "$scenario" false true false false average-
done
