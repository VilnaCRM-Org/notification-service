#!/usr/bin/env bash
# Run every scenario of tests/Load/scripts with all four profiles.
set -euo pipefail

LOAD_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

scenarios="$("${LOAD_DIR}/get-load-test-scenarios.sh")"

if [[ -z "${scenarios}" ]]; then
  echo "Error: No load test scenarios found." >&2
  exit 1
fi

for scenario in ${scenarios}; do
  "${LOAD_DIR}/execute-load-test.sh" "$scenario" true true true true all-
done
