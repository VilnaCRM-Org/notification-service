#!/usr/bin/env bash
# Drop, create and migrate the database of the isolated load-test environment.
#
# Usage: setup-load-test-db.sh
# Acts only on the compose project $LOAD_TEST_PROJECT (default: <directory name>-load), always with the explicit file
# list of the isolated environment: `dropdb --if-exists --force` and `createdb` in its `database` container, then
# `docker compose run --rm migrate`. The role and the database name are the ones the container was started with
# (POSTGRES_USER and POSTGRES_DB, which Compose takes from .env), not values read from the caller's shell. It refuses to run when that project has no running `database` container.
# Exit status: 0 success; 1 a step failed; 2 the project is not running or is not an isolated project.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

dev_project="$(basename "$ROOT" | tr '[:upper:]' '[:lower:]' | tr -c 'a-z0-9_\n-' '-')"
project="${LOAD_TEST_PROJECT:-${dev_project}-load}"

# The development project must never be the target: refuse its name, however it was given.
if [ "$project" = "${COMPOSE_PROJECT_NAME-}" ] || [ "$project" = "$dev_project" ]; then
  echo "setup-load-test-db.sh: refusing to act on project '${project}': it must differ from the development project" >&2
  exit 2
fi

compose() {
  docker compose -p "$project" -f docker-compose.yml -f docker-compose.load_test.override.yml "$@"
}

if [ -z "$(compose ps --status running -q database 2>/dev/null)" ]; then
  echo "setup-load-test-db.sh: project '${project}' is not running; start it with scripts/with-load-test-stack.sh" >&2
  exit 2
fi

# The variables are expanded by the shell inside the container, so the values come from the container itself.
compose exec -T database sh -c "dropdb -U \"\$POSTGRES_USER\" --if-exists --force \"\$POSTGRES_DB\"" || exit 1
compose exec -T database sh -c "createdb -U \"\$POSTGRES_USER\" \"\$POSTGRES_DB\"" || exit 1
compose run --rm --no-deps migrate || exit 1
