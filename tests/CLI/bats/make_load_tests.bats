#!/usr/bin/env bats
# Tests of the load-test targets (architecture-delivery.md "CLI Tests (Bats)", file make_load_tests.bats): the
# single-scenario runner, the profile runners, the start times of the profiles, the AWS dry run against LocalStack
# and its instance script, and the negative tests of gates G22 and G24. JSON is edited with json_query of the
# helper, so the host needs no jq.
#
# The isolated production-image environment is started and removed by the targets under test; tests that start
# it must not run beside another run of it. Local budgets are written to the git-ignored tests/Load/config.json
# with the harness, so teardown removes them. Make reports every failed recipe with its own status 2.

bats_require_minimum_version 1.8.0
load test_helper

# bats file_tags=load

RESULTS_DIR=tests/Load/results

setup() {
  _common_setup
}

teardown() {
  restore_planted
  rm -f -- "${RESULTS_DIR}/health.html" "${RESULTS_DIR}/smoke-health.html" "${RESULTS_DIR}/planted_failing_check.html"
  if [[ -f tests/Load/aws-run.env ]]; then
    make --no-print-directory aws-load-tests-cleanup LOCAL_MODE_ENV=true >/dev/null 2>&1 || true
  fi
}

# Value of a Make variable after the Makefile was read, so the tests follow the caller's environment.
make_var() {
  make --no-print-directory --silent -f Makefile -f - "print-$1" <<'MAKEFILE'
.RECIPEPREFIX = >
print-%:
> @printf '%s' "$($*)"
MAKEFILE
}

# Ids of the containers, volumes and networks that the isolated environment owns; empty when it is gone.
isolated_project_resources() {
  local filter
  filter="label=com.docker.compose.project=$(make_var LOAD_TEST_PROJECT)"
  docker ps --all --quiet --filter "${filter}"
  docker volume ls --quiet --filter "${filter}"
  docker network ls --quiet --filter "${filter}"
}

# Containers of the development project with their state, to compare before and after a run.
development_containers() {
  docker ps --all --filter "label=com.docker.compose.project=$(make_var PROJECT_NAME)" \
    --format '{{.ID}} {{.Names}} {{.State}}' | sort
}

# Plants a local tests/Load/config.json that is config.json.dist with the smoke budget of the health scenario.
plant_smoke_budget() {
  json_query "(data.endpoints.health.smoke.threshold = $1, data)" <tests/Load/config.json.dist \
    >"${BATS_TEST_TMPDIR}/config.json"
  plant_new tests/Load/config.json "${BATS_TEST_TMPDIR}/config.json"
}

# Plants a local tests/Load/config.json whose health profiles have the shipped durations and pause (smoke 10 s,
# average and stress 3 + 10 + 3 s, spike 5 + 5 s, 60 s between profiles). config.json.dist belongs to the service
# once the template is synchronised, so the start-time tests do not read it.
plant_profile_durations() {
  cat >"${BATS_TEST_TMPDIR}/config.json" <<'JSON'
{
  "apiHost": "localhost",
  "apiPort": "8081",
  "delayBetweenScenarios": 60,
  "endpoints": {
    "health": {
      "smoke": { "threshold": 50, "rps": 10, "vus": 5, "duration": 10 },
      "average": { "threshold": 200, "rps": 25, "vus": 25, "duration": { "rise": 3, "plateau": 10, "fall": 3 } },
      "stress": { "threshold": 1000, "rps": 150, "vus": 150, "duration": { "rise": 3, "plateau": 10, "fall": 3 } },
      "spike": { "threshold": 5000, "rps": 200, "vus": 200, "duration": { "rise": 5, "fall": 5 } }
    }
  }
}
JSON
  plant_new tests/Load/config.json "${BATS_TEST_TMPDIR}/config.json"
}

# Start time of every scenario that K6 builds from the health script with the switches $1 to $4 (smoke, average,
# stress, spike), as `name=startTime` pairs sorted by name on standard output. `k6 inspect` evaluates the options in
# the pinned image with the mount of execute-load-test.sh, so it reads the same tests/Load/config.json, and sends no
# request. Its warnings go to standard error, which the tests keep apart with `run --separate-stderr`.
inspect_start_times() {
  local image
  image="${K6_IMAGE:-$(pinned_value K6_IMAGE)}"
  "${DOCKER:-docker}" run --rm --user "$(id -u):$(id -g)" -v "${REPO_ROOT}/tests/Load:/loadTests" "${image}" \
    inspect -e "run_smoke=$1" -e "run_average=$2" -e "run_stress=$3" -e "run_spike=$4" /loadTests/scripts/health.js \
    >"${BATS_TEST_TMPDIR}/options.json" || return 1
  json_query 'Object.keys(data.scenarios).sort().map((name) => name + "=" + data.scenarios[name].startTime).join(" ")' \
    <"${BATS_TEST_TMPDIR}/options.json"
}

# A stand-in for docker on PATH that records the arguments of every call in $RECORD; no container runs.
record_docker_calls() {
  mkdir -p "${BATS_TEST_TMPDIR}/bin"
  cat >"${BATS_TEST_TMPDIR}/bin/docker" <<'STUB'
#!/bin/sh
printf '%s\n' "$*" >>"${RECORD}"
exit 0
STUB
  chmod +x "${BATS_TEST_TMPDIR}/bin/docker"
}

# --- setup-load-test-db ----------------------------------------------------------------------------------------

@test "setup-load-test-db takes the role and the database from the container, not from DB_USER and DB_NAME of the shell" {
  mkdir -p "${BATS_TEST_TMPDIR}/bin"
  cat >"${BATS_TEST_TMPDIR}/bin/docker" <<'STUB'
#!/bin/bash
printf '%s\n' "$*" >>"${RECORD}"
case "$*" in *' ps '*) echo some-container-id ;; esac
exit 0
STUB
  chmod +x "${BATS_TEST_TMPDIR}/bin/docker"
  PATH="${BATS_TEST_TMPDIR}/bin:${PATH}" RECORD="${BATS_TEST_TMPDIR}/calls" LOAD_TEST_PROJECT=bats-other-load DB_USER=orders DB_NAME=orders \
    run scripts/setup-load-test-db.sh
  assert_success
  # The statements run in the database container and expand the variables of that container.
  local dollar='$'
  run grep -c -F "exec -T database sh -c dropdb -U \"${dollar}POSTGRES_USER\" --if-exists --force \"${dollar}POSTGRES_DB\"" "${BATS_TEST_TMPDIR}/calls"
  assert_output '1'
  run grep -c -F "exec -T database sh -c createdb -U \"${dollar}POSTGRES_USER\" \"${dollar}POSTGRES_DB\"" "${BATS_TEST_TMPDIR}/calls"
  assert_output '1'
  run grep -c -E 'orders|-U app' "${BATS_TEST_TMPDIR}/calls"
  assert_output '0'
}

@test "setup-load-test-db refuses the development project and does not call docker" {
  mkdir -p "${BATS_TEST_TMPDIR}/bin"
  printf '#!/bin/sh\necho called >>"%s/calls"\n' "${BATS_TEST_TMPDIR}" >"${BATS_TEST_TMPDIR}/bin/docker"
  chmod +x "${BATS_TEST_TMPDIR}/bin/docker"
  PATH="${BATS_TEST_TMPDIR}/bin:${PATH}" LOAD_TEST_PROJECT="$(make_var PROJECT_NAME)" run scripts/setup-load-test-db.sh
  assert_failure 2
  assert_output --partial 'refusing to act on project'
  assert_file_not_exists "${BATS_TEST_TMPDIR}/calls"
}

# --- execute-load-tests-script -------------------------------------------------------------------------------

# bats test_tags=slow
@test "execute-load-tests-script writes health.html, removes the isolated project and leaves the development containers unchanged" {
  require_docker
  rm -f -- "${RESULTS_DIR}/health.html"
  # The subject is the report, the removal of the project and the untouched development containers, not the
  # latency: a budget no machine crosses (K6 times a request out after 60 s) keeps the test independent of the
  # machine's speed. The shipped budget stays enforced by smoke-load-tests and the hosted job.
  plant_smoke_budget 60000
  local before after
  before="$(development_containers)"
  run make --no-print-directory execute-load-tests-script scenario=health runAverage=false runStress=false runSpike=false
  assert_success
  # The failure block of scripts/with-load-test-stack.sh (its line and the project's logs) is for failed runs only.
  refute_output --partial 'the command failed with exit status'
  refute_output --regexp '[a-z]-1 +\| '
  assert_file_exists "${RESULTS_DIR}/health.html"
  run isolated_project_resources
  assert_success
  assert_output ''
  after="$(development_containers)"
  assert_equal "${after}" "${before}"
}

# --- the profile runners ----------------------------------------------------------------------------------------

@test "the profile runners pass their switches and report prefixes to the K6 run" {
  record_docker_calls
  local runner smoke average stress spike dashboard expected
  # runner, then the switches (smoke average stress spike) and the dashboard of the health scenario it must give.
  for expected in \
    'run-smoke-load-tests.sh true false false false smoke-health.html' \
    'run-average-load-tests.sh false true false false average-health.html' \
    'run-stress-load-tests.sh false false true false stress-health.html' \
    'run-spike-load-tests.sh false false false true spike-health.html' \
    'run-load-tests.sh true true true true all-health.html'; do
    read -r runner smoke average stress spike dashboard <<<"${expected}"
    rm -f -- "${BATS_TEST_TMPDIR}/calls"
    PATH="${BATS_TEST_TMPDIR}/bin:${PATH}" RECORD="${BATS_TEST_TMPDIR}/calls" run "tests/Load/${runner}"
    assert_success
    run grep -c -F -- "K6_WEB_DASHBOARD_EXPORT=/loadTests/results/${dashboard} " "${BATS_TEST_TMPDIR}/calls"
    assert_output '1'
    run grep -c -F -- "/loadTests/scripts/health.js -e run_smoke=${smoke} -e run_average=${average} -e run_stress=${stress} -e run_spike=${spike}" \
      "${BATS_TEST_TMPDIR}/calls"
    assert_output '1'
  done
}

# --- start times of the profiles -------------------------------------------------------------------------------
# Each selected profile starts after the profiles selected before it and the pause. K6 prints a start time in
# minutes and seconds: 70 s is 1m10s, 146 s is 2m26s and 222 s is 3m42s.

@test "FR81: a selection without the average profile starts stress and spike after the pause" {
  require_docker
  plant_profile_durations
  run --separate-stderr inspect_start_times true false true true
  assert_success
  # stress after smoke (10 s) and the pause (60 s), spike after stress (16 s) and the pause; average is not built.
  assert_output 'smoke=0s spike=2m26s stress=1m10s'
}

@test "FR78: the run of all four profiles keeps 70 s, 146 s and 222 s" {
  require_docker
  plant_profile_durations
  run --separate-stderr inspect_start_times true true true true
  assert_success
  assert_output 'average=1m10s smoke=0s spike=3m42s stress=2m26s'
}

# --- aws-load-tests against LocalStack -------------------------------------------------------------------------

# bats test_tags=slow
@test "aws-load-tests with LOCAL_MODE_ENV=true prints the instance and bucket lines, and the cleanup succeeds" {
  require_docker
  run make --no-print-directory start-deps DEPS=localstack
  assert_success
  # The branch and the repository URL are given: a pull-request job checks out a detached HEAD, and a clone made
  # from a local path has no HTTPS origin, which the script refuses on purpose.
  run make --no-print-directory aws-load-tests LOCAL_MODE_ENV=true branch=load-dry-run repo_url=https://example.com/acme/service.git
  assert_success
  assert_output --partial 'Repository URL: https://example.com/acme/service.git'
  assert_output --partial 'Repository branch: load-dry-run'
  assert_output --partial 'Launched instance'
  assert_output --partial 'You can access the S3 bucket here'
  assert_file_exists tests/Load/aws-run.env
  run make --no-print-directory aws-load-tests-cleanup LOCAL_MODE_ENV=true
  assert_success
  assert_file_not_exists tests/Load/aws-run.env
}

@test "aws-load-tests refuses a detached HEAD without a branch and exits 2 before any container or AWS call" {
  require_command git
  # A throw-away clone with a detached HEAD: the checkout of the developer is never touched.
  git clone --quiet --local --no-hardlinks "${REPO_ROOT}" "${BATS_TEST_TMPDIR}/clone"
  git -C "${BATS_TEST_TMPDIR}/clone" remote set-url origin https://example.org/org/service.git
  git -C "${BATS_TEST_TMPDIR}/clone" checkout --quiet --detach
  run env LOCAL_MODE_ENV=true "${BATS_TEST_TMPDIR}/clone/tests/Load/aws-execute-load-tests.sh"
  assert_failure 2
  assert_output --partial 'Error: HEAD is detached; give the branch with -b (make aws-load-tests branch=<name>).'
  assert_file_not_exists "${BATS_TEST_TMPDIR}/clone/tests/Load/aws-run.env"
}

@test "aws-load-tests refuses a path origin without a repository URL and exits 2 before any container or AWS call" {
  require_command git
  # A throw-away clone whose origin is a local path (a mirror, or a clone made with --no-local).
  git clone --quiet --local --no-hardlinks "${REPO_ROOT}" "${BATS_TEST_TMPDIR}/clone"
  git -C "${BATS_TEST_TMPDIR}/clone" remote set-url origin "${BATS_TEST_TMPDIR}/mirror.git"
  run env LOCAL_MODE_ENV=true "${BATS_TEST_TMPDIR}/clone/tests/Load/aws-execute-load-tests.sh" -b load-dry-run
  assert_failure 2
  assert_output --partial "Error: the URL of the remote 'origin' cannot be converted to HTTPS; give the repository URL with -u (make aws-load-tests repo_url=<https url>)."
  assert_file_not_exists "${BATS_TEST_TMPDIR}/clone/tests/Load/aws-run.env"
}

@test "lint-prettier, lint-yaml and lint-toml pass with generated load-test files present" {
  require_docker
  printf '<html><body>   generated   dashboard</body></html>\n' >"${BATS_TEST_TMPDIR}/dashboard.html"
  printf '{   "apiHost":"localhost",\n"apiPort":"8081" }\n' >"${BATS_TEST_TMPDIR}/config.json"
  printf 'BUCKET=planted\nROLE=planted\n' >"${BATS_TEST_TMPDIR}/aws-run.env"
  plant_new "${RESULTS_DIR}/health.html" "${BATS_TEST_TMPDIR}/dashboard.html"
  plant_new "${RESULTS_DIR}/smoke-health.html" "${BATS_TEST_TMPDIR}/dashboard.html"
  plant_new tests/Load/config.json "${BATS_TEST_TMPDIR}/config.json"
  plant_new tests/Load/aws-run.env "${BATS_TEST_TMPDIR}/aws-run.env"
  run make --no-print-directory lint-prettier
  assert_success
  run make --no-print-directory lint-yaml
  assert_success
  run make --no-print-directory lint-toml
  assert_success
}

@test "every snap wait of the instance script is bounded by timeout" {
  local unbounded
  # Lines that call `snap wait` (comments aside) without a `timeout <seconds>` in front of it.
  unbounded="$(grep -n -E '(^|[^#[:alnum:]_-])snap[[:space:]]+wait([[:space:]]|$)' tests/Load/user-data.sh |
    grep -v -E '^[0-9]+:[[:space:]]*#' | grep -v -E 'timeout[[:space:]]+[0-9]+[smh]?[[:space:]]+snap[[:space:]]+wait' || true)"
  assert_equal "${unbounded}" ''
  # The script does wait for the seed (the line the test is about exists).
  run grep -c -E '^[[:space:]]*timeout[[:space:]]+[0-9]+[smh]?[[:space:]]+snap[[:space:]]+wait[[:space:]]+system[[:space:]]+seed\.loaded' \
    tests/Load/user-data.sh
  assert_output '1'
}

# --- G22: load budgets -----------------------------------------------------------------------------------------

# bats test_tags=slow
@test "G22: smoke-load-tests with a local budget of 0.001 fails the thresholds and still writes smoke-health.html" {
  require_docker
  rm -f -- "${RESULTS_DIR}/smoke-health.html"
  plant_smoke_budget 0.001
  # The isolated stack takes LOG_LEVEL from the shell; info pins the level that writes the `listening` entry.
  LOG_LEVEL=info run make --no-print-directory smoke-load-tests
  assert_failure
  assert_output --regexp 'thresholds on metrics .* have been crossed'
  assert_file_exists "${RESULTS_DIR}/smoke-health.html"
  # The failed run prints the logs of the isolated project before it removes it, the service's included.
  assert_output --partial "with-load-test-stack.sh: the command failed with exit status 99; logs of project '$(make_var LOAD_TEST_PROJECT)':"
  assert_output --regexp 'app-1 +\| .*"message":"listening"'
  run isolated_project_resources
  assert_success
  assert_output ''
}

# bats test_tags=slow
@test "G22: a scenario whose check fails crosses the checks threshold" {
  require_docker
  local planted=planted_failing_check
  plant_new "tests/Load/scripts/${planted}.js" "${BATS_TEST_DIRNAME}/fixtures/failing_check_scenario.js.txt"
  # The planted scenario gets the profiles of the health scenario and a smoke budget no machine crosses (K6 times
  # a request out after 60 s), so only the checks threshold can fail, however busy the machine is.
  json_query "(data.endpoints.${planted} = { ...data.endpoints.health, smoke: { ...data.endpoints.health.smoke, threshold: 60000 } }, data)" \
    <tests/Load/config.json.dist \
    >"${BATS_TEST_TMPDIR}/config.json"
  plant_new tests/Load/config.json "${BATS_TEST_TMPDIR}/config.json"
  run make --no-print-directory execute-load-tests-script "scenario=${planted}" runAverage=false runStress=false runSpike=false
  assert_failure
  assert_output --regexp "thresholds on metrics .*'checks\{scenario:smoke\}'.* have been crossed"
  refute_output --regexp "thresholds on metrics .*http_req_duration.* have been crossed"
  run isolated_project_resources
  assert_success
  assert_output ''
}

@test "G22: a budget of -1 makes execute-load-test.sh print Invalid threshold value before any request" {
  require_docker
  plant_smoke_budget -1
  run tests/Load/execute-load-test.sh health
  assert_failure
  assert_output --partial 'Invalid threshold value'
  refute_output --partial 'http_reqs'
  assert_file_not_exists "${RESULTS_DIR}/health.html"
}

@test "G22: execute-load-tests-script without scenario fails with usage and starts nothing" {
  require_docker
  run make --no-print-directory execute-load-tests-script
  assert_failure
  assert_output --partial 'Usage: make execute-load-tests-script scenario=<name>'
  assert_output --partial '[runSmoke=false]'
  assert_output --partial 'each profile runs unless its variable is false'
  refute_output --partial 'runSmoke=true'
  assert_output --partial 'execute-load-tests-script] Error 2'
  run isolated_project_resources
  assert_success
  assert_output ''
}

# --- G24: load-test lint ---------------------------------------------------------------------------------------

@test "G24: a script with console.log and == makes lint-load-tests fail" {
  require_docker
  plant_new tests/Load/scripts/planted_lint.js "${BATS_TEST_DIRNAME}/fixtures/bad_load_script.js.txt"
  run make --no-print-directory lint-load-tests
  assert_failure
  assert_output --partial 'planted_lint.js'
  assert_output --partial 'no-console'
  assert_output --partial 'eqeqeq'
}

@test "G24: an unformatted load script makes lint-load-tests fail with the finding of Prettier" {
  require_docker
  # ESLint accepts the script, so the recipe reaches Prettier, the only formatting check of the K6 scripts.
  plant_new tests/Load/scripts/planted_format.js "${BATS_TEST_DIRNAME}/fixtures/unformatted_load_script.js.txt"
  run make --no-print-directory lint-load-tests
  assert_failure
  assert_output --partial 'planted_format.js'
  assert_output --partial 'Code style issues found'
  refute_output --partial 'no-console'
}

@test "G24: a prettier key in the package manifest of the load tests does not replace the root configuration" {
  require_docker
  json_query '({ ...data, prettier: { semi: false } })' <tests/Load/package.json >"${BATS_TEST_TMPDIR}/package.json"
  plant_replace tests/Load/package.json "${BATS_TEST_TMPDIR}/package.json"
  run make --no-print-directory lint-load-tests
  assert_success
}
