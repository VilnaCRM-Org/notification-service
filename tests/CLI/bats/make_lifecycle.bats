#!/usr/bin/env bats
# Tests of the development stack from start to removal
# (architecture-delivery.md "CLI Tests (Bats)", file make_lifecycle.bats).
#
# The file starts its own compose project, with a name and host ports of its own, and removes it with its
# volumes at the end, so it cannot collide with the stack of a developer (NFR38). The Makefile derives every
# name from PROJECT_NAME, so exporting it (and no COMPOSE_PROJECT_NAME) addresses the private project. The
# tests depend on each other in file order: the first starts the stack, the last two stop and remove it.
#
# Slow tests: `make start` on a fresh private project makes the containers compile the service, which takes
# two minutes or more, so the start test is tagged `slow`. Every test that needs the stack it starts (the mount
# check, `make start-deps` on a running environment, the two log tests, the occupied-port test, the rebuild
# test, `make stop` and `make down`) is tagged `slow` too, so that `--filter-tags '!slow'` skips the whole
# group and no test that stays untagged depends on another one. The two untagged tests start only containers of
# private projects of their own and pass on their own: the second-project test starts its own database and
# LocalStack; the start that never becomes healthy starts its own database and, all from the database image, app,
# migrate and a stand-in for LocalStack (no LocalStack and no service image), and stops at the failed health check
# of app. The rebuild test is slow by itself.

bats_require_minimum_version 1.8.0
load test_helper

# bats file_tags=surface

# Name of the development image that the Makefile derives from the checkout directory.
_default_dev_image() {
  printf '%s-app:dev' "$(basename "${REPO_ROOT}" | tr '[:upper:]' '[:lower:]' | tr -c 'a-z0-9_\n-' '-')"
}

# Removes a private project with its volumes.
_remove_project() {
  [[ "$1" == bats-* ]] || return 0
  docker compose --profile '*' -p "$1" down --volumes --remove-orphans >/dev/null 2>&1 || true
}

setup_file() {
  REPO_ROOT="$(cd "${BATS_TEST_DIRNAME}/../../.." && pwd)"
  cd "${REPO_ROOT}" || return 1
  "${DOCKER:-docker}" info >/dev/null 2>&1 || return 0
  local suffix="${BASHPID}-${RANDOM}"
  isolate_make_environment
  unset COMPOSE_PROJECT_NAME CI IN_DEV_CONTAINER RUN_IN_CONTAINER
  export PROJECT_NAME="bats-life-${suffix}"
  export ALT_PROJECT="bats-alt-${suffix}"
  export BUSY_PROJECT="bats-busy-${suffix}"
  export SICK_PROJECT="bats-sick-${suffix}"
  DB_PORT="$(free_port)"
  LOCALSTACK_PORT="$(free_port)"
  APP_PORT="$(free_port)"
  STRUCTURIZR_PORT="$(free_port)"
  export DB_PORT LOCALSTACK_PORT APP_PORT STRUCTURIZR_PORT
  ALT_DB_PORT="$(free_port)"
  ALT_LOCALSTACK_PORT="$(free_port)"
  BUSY_LOCALSTACK_PORT="$(free_port)"
  export ALT_DB_PORT ALT_LOCALSTACK_PORT BUSY_LOCALSTACK_PORT
  # An image that an earlier `make start` of this checkout built is reused; otherwise `make start` builds it.
  if [[ -z "${DEV_IMAGE:-}" ]]; then
    if docker image inspect "$(_default_dev_image)" >/dev/null 2>&1; then
      DEV_IMAGE="$(_default_dev_image)"
      export DEV_IMAGE
      export BUILD_IMAGE="${BUILD_IMAGE:-0}"
    else
      BUILT_IMAGE=1
      export BUILT_IMAGE
    fi
  fi
  # Nothing may be started unless the Makefile addresses the private project, which holds nothing yet.
  local addressed probe
  probe="$(mktemp)"
  printf "show:\n\t@echo \$\$COMPOSE_PROJECT_NAME\n" >"${probe}"
  addressed="$(make --no-print-directory -f Makefile -f "${probe}" show)"
  rm -f -- "${probe}"
  if [[ "${addressed}" != "${PROJECT_NAME}" || -n "$(docker compose -p "${PROJECT_NAME}" ps --all --quiet)" ]]; then
    echo "make addresses '${addressed}', not the empty private project '${PROJECT_NAME}'" >&2
    return 1
  fi
}

teardown_file() {
  cd "${REPO_ROOT}" || return 0
  [[ -n "${PROJECT_NAME:-}" ]] || return 0
  _remove_project "${PROJECT_NAME}"
  _remove_project "${ALT_PROJECT}"
  _remove_project "${BUSY_PROJECT}"
  _remove_project "${SICK_PROJECT}"
  if [[ -n "${BUILT_IMAGE:-}" ]]; then
    docker image rm "${PROJECT_NAME}-app:dev" >/dev/null 2>&1 || true
  fi
}

setup() {
  _common_setup
  require_docker
  require_command curl
  tmp="$(mktemp -d)"
  # Every test addresses a private project.
  [[ "${PROJECT_NAME:-}" == bats-* ]] || fail "PROJECT_NAME is not a private project: '${PROJECT_NAME:-}'"
}

teardown() {
  restore_planted
  [[ -z "${tmp:-}" ]] || rm -rf -- "${tmp}"
  # A log follower that outlived its test must not keep the project's containers busy.
  if [[ -n "${follower:-}" ]]; then
    kill "${follower}" 2>/dev/null || true
  fi
}

# The entries at the top of the checkout with their owners.
top_entries() {
  find . -mindepth 1 -maxdepth 1 -printf '%u %p\n' | LC_ALL=C sort
}

# Container id of a service of the private project.
container_of() {
  docker compose -p "${PROJECT_NAME}" ps --all --quiet "$1"
}

# HTTP status of a GET request to the service of the private project.
http_status() {
  curl --noproxy '*' --silent --output /dev/null --write-out '%{http_code}' --max-time 10 "http://127.0.0.1:${APP_PORT}$1"
}

# Number of `listening` entries in the log of the app container.
listening_entries() {
  docker compose -p "${PROJECT_NAME}" logs --no-log-prefix app 2>&1 | grep -c '"message":"listening"' || true
}

# --- start ---------------------------------------------------------------------------------------------------------

# bats test_tags=slow
@test "make start returns with /ping answering pong and /api/health answering 204 after the migrate step" {
  local before after
  before="$(top_entries)"
  run make --no-print-directory start
  assert_success
  run curl --noproxy '*' --silent --fail --max-time 10 "http://127.0.0.1:${APP_PORT}/ping"
  assert_success
  assert_output 'pong'
  run http_status /api/health
  assert_output '204'
  run docker inspect --format '{{.State.Status}} {{.State.ExitCode}}' "$(container_of migrate)"
  assert_success
  assert_output 'exited 0'
  after="$(top_entries)"
  # No new entry at the top of the checkout and no entry that changed its owner (also meaningful as root).
  assert_equal "${after}" "${before}"
}

# bats test_tags=slow
@test "no mount of the app container lies below /app" {
  local id
  id="$(container_of app)"
  [[ -n "${id}" ]]
  run docker inspect --format '{{range .Mounts}}{{.Destination}}{{"\n"}}{{end}}' "${id}"
  assert_success
  assert_line '/app'
  assert_line '/cargo-target'
  local line
  for line in "${lines[@]}"; do
    [[ "${line}" != /app/* ]] || fail "mount below /app: ${line}"
  done
}

# bats test_tags=slow
@test "make start-deps on a running environment recreates no container" {
  local before after
  before="$(docker inspect --format '{{.Id}} {{.State.StartedAt}}' "$(container_of database)" "$(container_of localstack)")"
  run make --no-print-directory start-deps
  assert_success
  after="$(docker inspect --format '{{.Id}} {{.State.StartedAt}}' "$(container_of database)" "$(container_of localstack)")"
  assert_equal "${after}" "${before}"
}

# --- logs --------------------------------------------------------------------------------------------------------

# bats test_tags=slow
@test "timeout 5 make logs exits 124 and shows the listening entry" {
  local status=0
  timeout 5 make --no-print-directory logs >"${tmp}/logs.txt" 2>&1 || status=$?
  assert_equal "${status}" 124
  run grep -F '"message":"listening"' "${tmp}/logs.txt"
  assert_success
}

# bats test_tags=slow
@test "make new-logs with a concurrent ping shows request completed and /ping" {
  timeout 60 make --no-print-directory new-logs >"${tmp}/new-logs.txt" 2>&1 &
  follower=$!
  for _ in $(seq 1 30); do
    sleep 1
    http_status /ping >/dev/null
    if grep -q 'request completed' "${tmp}/new-logs.txt" && grep -q '/ping' "${tmp}/new-logs.txt"; then
      break
    fi
  done
  kill "${follower}" 2>/dev/null || true
  wait "${follower}" 2>/dev/null || true
  run grep -F 'request completed' "${tmp}/new-logs.txt"
  assert_success
  run grep -F '/ping' "${tmp}/new-logs.txt"
  assert_success
}

# --- a second project ------------------------------------------------------------------------------------------

@test "a second project creates containers named after it and is removed with its volumes" {
  PROJECT_NAME="${ALT_PROJECT}" DB_PORT="${ALT_DB_PORT}" LOCALSTACK_PORT="${ALT_LOCALSTACK_PORT}" \
    run make --no-print-directory start-deps
  assert_success
  run docker ps --all --filter "label=com.docker.compose.project=${ALT_PROJECT}" --format '{{.Names}}'
  assert_success
  assert_line --regexp '^bats-alt-.*database'
  assert_line --regexp '^bats-alt-.*localstack'
  refute_output --regexp "^${PROJECT_NAME}"
  run docker compose -p "${ALT_PROJECT}" down --volumes
  assert_success
  run docker ps --all --quiet --filter "label=com.docker.compose.project=${ALT_PROJECT}"
  assert_output ''
  run docker volume ls --quiet --filter "label=com.docker.compose.project=${ALT_PROJECT}"
  assert_output ''
}

@test "make start exits non-zero when the stack does not become healthy" {
  # A third project whose app never passes its health check while every dependency is satisfied: only the wait of
  # the recipe (`--wait`) turns that into a failure; without it `up --detach` succeeds once app has started. The
  # services run the database image, so the test needs no service image and builds nothing (BUILD_IMAGE=0 gives
  # `--no-build`): migrate exits 0 at once, LocalStack is a stand-in that is healthy at once, and app sleeps with a
  # health check that always fails.
  cat >"${tmp}/unhealthy.yml" <<'OVERRIDE'
services:
  migrate:
    image: postgres:${DB_VERSION:-16}-alpine
    entrypoint: ['true']
    command: !reset null
  localstack:
    image: postgres:${DB_VERSION:-16}-alpine
    entrypoint: ['sleep', '600']
    stop_grace_period: 1s # sleep ignores the stop signal as process 1
    healthcheck:
      test: ['CMD', 'true']
      interval: 1s
      timeout: 1s
      retries: 1
      start_period: 0s
      start_interval: 1s
  app:
    image: postgres:${DB_VERSION:-16}-alpine
    entrypoint: ['sleep', '600']
    stop_grace_period: 1s # sleep ignores the stop signal as process 1
    healthcheck:
      test: ['CMD', 'false']
      interval: 1s
      timeout: 1s
      retries: 1
      start_period: 0s
      start_interval: 1s
OVERRIDE
  # Every published port is its own (the stack of the first test holds the others) and is drawn right before use.
  PROJECT_NAME="${SICK_PROJECT}" DB_PORT="$(free_port)" LOCALSTACK_PORT="$(free_port)" APP_PORT="$(free_port)" \
    STRUCTURIZR_PORT="$(free_port)" \
    COMPOSE_FILE="docker-compose.yml:docker-compose.override.yml:${tmp}/unhealthy.yml" BUILD_IMAGE=0 \
    run make --no-print-directory start
  assert_failure
  assert_output --partial "container ${SICK_PROJECT}-app-1 is unhealthy"
  _remove_project "${SICK_PROJECT}"
  run docker ps --all --quiet --filter "label=com.docker.compose.project=${SICK_PROJECT}"
  assert_output ''
}

# bats test_tags=slow
@test "make start-deps with DB_PORT set to an occupied port exits non-zero" {
  # The port of the running development database is occupied; the second project cannot publish it.
  PROJECT_NAME="${BUSY_PROJECT}" DB_PORT="${DB_PORT}" LOCALSTACK_PORT="${BUSY_LOCALSTACK_PORT}" \
    run make --no-print-directory start-deps
  assert_failure
  _remove_project "${BUSY_PROJECT}"
}

# --- rebuild on a source change ----------------------------------------------------------------------------------

# bats test_tags=slow
@test "a change of a source file of the service crate rebuilds the app container without an image rebuild" {
  local image container before
  container="$(container_of app)"
  image="$(docker inspect --format '{{.Image}}' "${container}")"
  before="$(listening_entries)"
  printf '\n// planted by a Bats test\n' >"${tmp}/comment.txt"
  plant_append crates/service/src/lib.rs "${tmp}/comment.txt"
  for _ in $(seq 1 120); do
    [[ "$(listening_entries)" -gt "${before}" ]] && break
    sleep 5
  done
  assert [ "$(listening_entries)" -gt "${before}" ]
  assert_equal "$(container_of app)" "${container}"
  run docker inspect --format '{{.Image}}' "${container}"
  assert_output "${image}"
}

# --- stop and removal --------------------------------------------------------------------------------------------

# bats test_tags=slow
@test "make stop stops the development environment" {
  run make --no-print-directory stop
  assert_success
  run docker ps --quiet --filter "label=com.docker.compose.project=${PROJECT_NAME}"
  assert_output ''
}

# bats test_tags=slow
@test "make down removes the containers and keeps the volumes" {
  run make --no-print-directory down
  assert_success
  run docker ps --all --quiet --filter "label=com.docker.compose.project=${PROJECT_NAME}"
  assert_output ''
  run docker volume ls --quiet --filter "label=com.docker.compose.project=${PROJECT_NAME}"
  assert_success
  assert_output --regexp '.+'
}
