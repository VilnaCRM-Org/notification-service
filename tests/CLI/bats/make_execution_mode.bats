#!/usr/bin/env bats
# Tests of the execution switch RUN_IN_CONTAINER and of the environment inside the development container
# (architecture-delivery.md DD-01 and "CLI Tests (Bats)", file make_execution_mode.bats).
#
# The tests that run a command in the container need the development environment. The file starts its own
# compose project, with a name and host ports of its own, and removes it with its volumes at the end, so it
# cannot collide with the stack of a developer (NFR38). The Makefile derives every name from PROJECT_NAME, so
# exporting it (and no COMPOSE_PROJECT_NAME) is all that addresses the private project.
#
# The `make start` of setup_file takes about two minutes and is not a test. The one test whose own body can exceed
# a minute, `make lint-rustdoc` in the container (58 to 99 s measured), is tagged `slow`.

bats_require_minimum_version 1.8.0
load test_helper

# bats file_tags=surface

# Name of the development image that the Makefile derives from the checkout directory.
_default_dev_image() {
  printf '%s-app:dev' "$(basename "${REPO_ROOT}" | tr '[:upper:]' '[:lower:]' | tr -c 'a-z0-9_\n-' '-')"
}

# Removes the private project with its volumes (and, when the file built it, its image).
_remove_project() {
  [[ "${PROJECT_NAME:-}" == bats-* ]] || return 0
  docker compose --profile '*' -p "${PROJECT_NAME}" down --volumes --remove-orphans >/dev/null 2>&1 || true
  if [[ -n "${BUILT_IMAGE:-}" ]]; then
    docker image rm "${PROJECT_NAME}-app:dev" >/dev/null 2>&1 || true
  fi
}

setup_file() {
  REPO_ROOT="$(cd "${BATS_TEST_DIRNAME}/../../.." && pwd)"
  cd "${REPO_ROOT}" || return 1
  "${DOCKER:-docker}" info >/dev/null 2>&1 || return 0
  local suffix="${BASHPID}-${RANDOM}"
  isolate_make_environment
  unset COMPOSE_PROJECT_NAME CI IN_DEV_CONTAINER RUN_IN_CONTAINER
  export PROJECT_NAME="bats-exec-${suffix}"
  # free_port of the helper (20000 to 32767, below the ephemeral ports) keeps the chosen ports in a file, so the
  # four command substitutions cannot draw the same port twice.
  DB_PORT="$(free_port)"
  LOCALSTACK_PORT="$(free_port)"
  APP_PORT="$(free_port)"
  STRUCTURIZR_PORT="$(free_port)"
  export DB_PORT LOCALSTACK_PORT APP_PORT STRUCTURIZR_PORT
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
  make --no-print-directory start >&3 2>&3
}

teardown_file() {
  cd "${REPO_ROOT}" || return 0
  _remove_project
}

setup() {
  _common_setup
  require_docker
  tmp="$(mktemp -d)"
  # Every test addresses the private project or none at all.
  [[ "${PROJECT_NAME:-}" == bats-* ]] || fail "PROJECT_NAME is not a private project: '${PROJECT_NAME:-}'"
}

teardown() {
  restore_planted
  [[ -z "${tmp:-}" ]] || rm -rf -- "${tmp}"
}

# A makefile that runs a command behind $(EXEC) with the variables of the real Makefile.
probe_makefile() {
  printf "probe:\n\t@\$(EXEC) %s\n" "$1" >"${tmp}/probe.mk"
}

# --- the command line (no environment needed) ----------------------------------------------------------------

@test "make -n format-check runs natively" {
  run make --no-print-directory -n format-check
  assert_success
  assert_line --index 0 'cargo fmt --all --check'
  refute_output --partial 'docker compose exec'
}

@test "RUN_IN_CONTAINER=1 make -n format-check starts with docker compose exec -T and carries the five -e options" {
  RUN_IN_CONTAINER=1 run make --no-print-directory -n format-check
  assert_success
  assert_line --index 0 --regexp '^docker compose exec -T --user [0-9]+:[0-9]+ -e TEST_SEED -e PROPTEST_RNG_SEED -e CONFIG_CHANGE_APPROVED -e CARGO_BUILD_WARNINGS -e DATABASE_STATEMENT_TIMEOUT app cargo fmt --all --check$'
}

@test "RUN_IN_CONTAINER=true is the same opt-in" {
  RUN_IN_CONTAINER=true run make --no-print-directory -n format-check
  assert_success
  assert_line --index 0 --regexp '^docker compose exec -T '
}

@test "RUN_IN_CONTAINER=1 with CI=true is native again" {
  RUN_IN_CONTAINER=1 CI=true run make --no-print-directory -n format-check
  assert_success
  assert_line --index 0 'cargo fmt --all --check'
  refute_output --partial 'docker compose exec'
}

@test "RUN_IN_CONTAINER=1 with IN_DEV_CONTAINER=1 is native again" {
  RUN_IN_CONTAINER=1 IN_DEV_CONTAINER=1 run make --no-print-directory -n format-check
  assert_success
  assert_line --index 0 'cargo fmt --all --check'
  refute_output --partial 'docker compose exec'
}

@test "an orchestration target ignores the switch" {
  RUN_IN_CONTAINER=1 run make --no-print-directory -n lint-docker
  assert_success
  assert_output --partial 'hadolint'
  refute_output --partial 'docker compose exec'
  RUN_IN_CONTAINER=1 run make --no-print-directory -n start-deps
  assert_success
  assert_output --partial 'up --detach --wait --no-recreate database localstack'
  refute_output --partial 'compose exec'
}

@test "make -n build rebuilds the images without cache, and make -n start builds unless BUILD_IMAGE=0" {
  local compose
  compose="$(make_value DOCKER_COMPOSE)"
  run make --no-print-directory -n build
  assert_success
  assert_line "${compose} build --pull --no-cache"
  BUILD_IMAGE=1 run make --no-print-directory -n start
  assert_success
  assert_line "${compose} up --detach --wait --wait-timeout 900 --build"
  BUILD_IMAGE=0 run make --no-print-directory -n start
  assert_success
  assert_line "${compose} up --detach --wait --wait-timeout 900 --no-build"
}

@test "every target is invoked by a Bats test or listed in the Make-target reference" {
  local reference=docs/make-targets.md bt listed target missing=''
  printf -v bt '\140'
  # The backticked names of the gate table and of the section of targets without a Bats test.
  listed="$(awk '/^## / {inside = ($0 == "## Gate targets: passing run and negative test" || $0 == "## Targets without a Bats test")} inside' "${reference}" |
    grep -oE "${bt}[a-z][a-z0-9-]*${bt}" | tr -d "${bt}" | sort -u)"
  [[ -n "${listed}" ]]
  while IFS= read -r target; do
    if grep -qxF -- "${target}" <<<"${listed}"; then
      continue
    fi
    # An invocation: the word make, then options or other targets, then the target as a whole word.
    if grep -qE -- "(^|[[:space:]\"'(])make([[:space:]]+[^[:space:]|;&]+)*[[:space:]]+${target}([[:space:]\"');]|\$)" tests/CLI/bats/*.bats; then
      continue
    fi
    missing="${missing} ${target}"
  done < <(make --no-print-directory help | awk 'NR > 3 {print $1}')
  assert_equal "${missing}" ''
}

# --- the targets in the running development container ----------------------------------------------------------

@test "RUN_IN_CONTAINER=1 make format-check succeeds in the running container" {
  RUN_IN_CONTAINER=1 run make --no-print-directory format-check
  assert_success
}

@test "RUN_IN_CONTAINER=1 make check-requirements succeeds in the running container" {
  RUN_IN_CONTAINER=1 run make --no-print-directory check-requirements
  assert_success
}

# bats test_tags=slow
@test "RUN_IN_CONTAINER=1 make lint-rustdoc (the env form behind exec) succeeds in the running container" {
  RUN_IN_CONTAINER=1 run make --no-print-directory lint-rustdoc
  assert_success
}

@test "RUN_IN_CONTAINER=1 make setup-test-db succeeds in the running container" {
  RUN_IN_CONTAINER=1 run make --no-print-directory setup-test-db
  assert_success
}

@test "inside the container the test environment is APP_ENV=test and a URL on database:5432/app_test" {
  probe_makefile "\$(TEST_ENV) printenv APP_ENV DATABASE_URL"
  RUN_IN_CONTAINER=1 run make --no-print-directory -f Makefile -f "${tmp}/probe.mk" probe
  assert_success
  assert_line --index 0 'test'
  assert_line --index 1 --regexp '^postgres://[^@]+@database:5432/app_test$'
}

@test "inside the container the URLs use the in-network ports whatever .env publishes" {
  # Host ports in .env that differ from the in-network ports 5432 and 4566. The container reads the bind-mounted
  # .env, and compose gives it the in-network ports, which win over the file.
  sed -e 's/^DB_PORT=.*/DB_PORT=15432/' -e 's/^LOCALSTACK_PORT=.*/LOCALSTACK_PORT=14566/' .env >"${tmp}/env"
  run grep -c -E '^(DB_PORT=15432|LOCALSTACK_PORT=14566)$' "${tmp}/env"
  assert_output '2'
  plant_replace .env "${tmp}/env"
  probe_makefile "scripts/with-env.sh .env .env.test -- printenv DATABASE_URL AWS_ENDPOINT_URL_SQS"
  RUN_IN_CONTAINER=1 run make --no-print-directory -f Makefile -f "${tmp}/probe.mk" probe
  assert_success
  assert_line --index 0 --regexp '^postgres://[^@]+@database:5432/app_test$'
  assert_line --index 1 'http://localstack:4566'
}

@test "TEST_SEED=7 reaches a command behind EXEC" {
  probe_makefile "bash -c 'echo \"seed=[\$\${TEST_SEED-unset}]\"'"
  RUN_IN_CONTAINER=1 TEST_SEED=7 run make --no-print-directory -f Makefile -f "${tmp}/probe.mk" probe
  assert_success
  assert_output 'seed=[7]'
}

@test "without TEST_SEED the variable is unset behind EXEC" {
  probe_makefile "bash -c 'echo \"seed=[\$\${TEST_SEED-unset}]\"'"
  RUN_IN_CONTAINER=1 run env -u TEST_SEED make --no-print-directory -f Makefile -f "${tmp}/probe.mk" probe
  assert_success
  assert_output 'seed=[unset]'
}

@test "DATABASE_STATEMENT_TIMEOUT=30m reaches a command behind EXEC" {
  probe_makefile "bash -c 'echo \"timeout=[\$\${DATABASE_STATEMENT_TIMEOUT-unset}]\"'"
  RUN_IN_CONTAINER=1 DATABASE_STATEMENT_TIMEOUT=30m run make --no-print-directory -f Makefile -f "${tmp}/probe.mk" probe
  assert_success
  assert_output 'timeout=[30m]'
}

@test "without DATABASE_STATEMENT_TIMEOUT the variable is unset behind EXEC" {
  probe_makefile "bash -c 'echo \"timeout=[\$\${DATABASE_STATEMENT_TIMEOUT-unset}]\"'"
  RUN_IN_CONTAINER=1 run env -u DATABASE_STATEMENT_TIMEOUT make --no-print-directory -f Makefile -f "${tmp}/probe.mk" probe
  assert_success
  assert_output 'timeout=[unset]'
}

@test "RUN_IN_CONTAINER=1 scripts/doctor.sh without cargo on PATH exits 0 and prints the SKIP line" {
  local tool path
  mkdir -p "${tmp}/bin"
  for tool in bash sed grep head sort uname tr dirname cat make git curl docker bats; do
    path="$(command -v "${tool}" || true)"
    [[ -z "${path}" ]] || ln -s "${path}" "${tmp}/bin/${tool}"
  done
  run env -i PATH="${tmp}/bin" HOME="${tmp}" RUN_IN_CONTAINER=1 "$(command -v bash)" scripts/doctor.sh
  assert_success
  assert_line '[SKIP] toolchain and cargo tools: provided by the development container'
  refute_output --partial '[MISSING] cargo'
}

# --- negative tests ------------------------------------------------------------------------------------------------

@test "an orchestration target without a reachable engine exits 2 and names the engine" {
  DOCKER=/nonexistent run make --no-print-directory lint-docker
  assert_failure 2
  assert_output --partial 'This target needs a reachable container engine'
}

# Last test of the file: it stops the environment.
@test "RUN_IN_CONTAINER=1 make format-check with the environment stopped fails with the service-is-not-running message" {
  run make --no-print-directory stop
  assert_success
  RUN_IN_CONTAINER=1 run make --no-print-directory format-check
  assert_failure
  assert_output --partial 'is not running'
}
