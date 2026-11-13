#!/usr/bin/env bash
# Print the name of every scenario script (tests/Load/scripts/<name>.js), sorted, one per line.
set -euo pipefail

LOAD_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

find "${LOAD_DIR}/scripts" -maxdepth 1 -type f -name '*.js' -exec basename {} .js \; | LC_ALL=C sort
