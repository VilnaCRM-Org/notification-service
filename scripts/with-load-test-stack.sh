#!/usr/bin/env bash
# Run a command against the production image in the isolated compose project of the load-test environment.
#
# Usage: with-load-test-stack.sh -- <command...>
# Builds the production image unless BUILD_IMAGE=0, starts the project $LOAD_TEST_PROJECT (default:
# <directory name>-load) from docker-compose.yml plus docker-compose.load_test.override.yml, waits for health,
# prepares its database (setup-load-test-db.sh), exports API_HOST=localhost and API_PORT=$LOAD_TEST_APP_PORT and
# runs the command. When the command fails, the logs of the project are printed to standard error before the
# project is removed, so that a failed run keeps the service's side of the story (a 500 carries no detail by
# design). The project is removed with its volumes on every exit path.
# Reads LOAD_TEST_PROJECT, LOAD_TEST_APP_PORT, APP_IMAGE, BUILD_IMAGE (and DOCKER for the image build).
# Exit status: the command's; 2 the environment did not become healthy (its logs are printed) or usage error.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

[ "${1-}" = "--" ] && [ "$#" -ge 2 ] || { echo "Usage: with-load-test-stack.sh -- <command...>" >&2; exit 2; }
shift

default_project="$(basename "$ROOT" | tr '[:upper:]' '[:lower:]' | tr -c 'a-z0-9_\n-' '-')"
export LOAD_TEST_PROJECT="${LOAD_TEST_PROJECT:-${default_project}-load}"
export LOAD_TEST_APP_PORT="${LOAD_TEST_APP_PORT:-18081}"
export APP_IMAGE="${APP_IMAGE:-${default_project}-app:prod}"
BUILD_IMAGE="${BUILD_IMAGE:-1}"

# The development project must never be started or removed by this script.
if [ "$LOAD_TEST_PROJECT" = "${COMPOSE_PROJECT_NAME-}" ] || [ "$LOAD_TEST_PROJECT" = "$default_project" ]; then
  echo "with-load-test-stack.sh: refusing to use project '${LOAD_TEST_PROJECT}': it must differ from the development project" >&2
  exit 2
fi

compose() {
  docker compose -p "$LOAD_TEST_PROJECT" -f docker-compose.yml -f docker-compose.load_test.override.yml "$@"
}

trap 'compose down --volumes --remove-orphans >/dev/null 2>&1 || echo "with-load-test-stack.sh: could not remove project ${LOAD_TEST_PROJECT}" >&2' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

unhealthy() {
  echo "with-load-test-stack.sh: the isolated environment '${LOAD_TEST_PROJECT}' did not become healthy" >&2
  compose logs --no-color >&2 || true
  exit 2
}

if [ "$BUILD_IMAGE" != 0 ]; then
  "${DOCKER:-docker}" build --target prod --tag "$APP_IMAGE" .
fi

compose up --detach --wait --wait-timeout 120 --no-build || unhealthy
scripts/setup-load-test-db.sh || unhealthy

export API_HOST=localhost
export API_PORT="$LOAD_TEST_APP_PORT"
status=0
"$@" || status=$?
if [ "$status" -ne 0 ]; then
  echo "with-load-test-stack.sh: the command failed with exit status ${status}; logs of project '${LOAD_TEST_PROJECT}':" >&2
  compose logs --no-color >&2 || true
fi
exit "$status"
