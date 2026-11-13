#!/usr/bin/env bats
# Tests of the test-level targets and their negative tests (architecture-delivery.md "CLI Tests (Bats)", file
# make_tests.bats): gates G11 to G15 and G30, the seed of a failing randomised test (FR51) and the `setup-test-db`
# target.
#
# Every plant goes into the leaf crate `xtask`, into `features/` or into `.sqlx/`, so only little is rebuilt; the
# documentation example goes into the test-support crate and the randomised test into the integration tests of the
# health-check probes, a template-owned test binary that depends on the seeded generator of test-support. Make
# reports every failed recipe with its own status 2, so these tests assert "non-zero" and the tool's own output.

bats_require_minimum_version 1.8.0
load test_helper

# bats file_tags=tests

# The CI switch MUTANTS_IN_PLACE of the caller (NFR40), read here because setup() unsets every Make variable of the
# caller. The G15 test passes it on: the hosted job sets it, so cargo-mutants builds in the checkout whose target/
# the Rust cache restores; locally it is unset, so cargo-mutants builds in a scratch copy, as `make ci` does.
CALLER_MUTANTS_IN_PLACE="${MUTANTS_IN_PLACE-}"

setup() {
  _common_setup
}

teardown() {
  restore_planted
  if [[ -n "${marker_database:-}" ]]; then
    server_sql "$(scripts/with-env.sh .env -- printenv DB_USER)" "drop database if exists ${marker_database} with (force)" >/dev/null 2>&1 || true
  fi
  if [[ -n "${marker_table:-}" ]]; then
    dev_sql "drop table if exists ${marker_table}" >/dev/null 2>&1 || true
  fi
}

# Runs a statement on the maintenance database of the server of the compose project ($1: user, $2: statement).
server_sql() {
  "${DOCKER:-docker}" compose exec -T database psql --quiet --no-psqlrc --tuples-only --no-align -U "$1" -d postgres -c "$2"
}

# Runs a statement against the development database `app` in the database container of the compose project.
dev_sql() {
  "${DOCKER:-docker}" compose exec -T database psql --quiet --no-psqlrc --tuples-only --no-align -U app -d app -c "$1"
}

# --- setup-test-db -----------------------------------------------------------------------------------------

@test "setup-test-db creates app_test and leaves the development database untouched" {
  require_docker
  marker_table="bats_marker_$$"
  run make --no-print-directory ensure-database
  assert_success
  run dev_sql "create table ${marker_table} (id integer); insert into ${marker_table} values (7)"
  assert_success

  run make --no-print-directory setup-test-db
  assert_success

  run "${DOCKER:-docker}" compose exec -T database psql --quiet --no-psqlrc --tuples-only --no-align -U app -d postgres \
    -c "select count(*) from pg_database where datname = 'app_test'"
  assert_success
  assert_output '1'
  run dev_sql "select id from ${marker_table}"
  assert_success
  assert_output '7'
}

@test "setup-test-db refuses a database whose name does not end in _test, with status 2 and before it runs sqlx" {
  local url
  mkdir -p "${BATS_TEST_TMPDIR}/bin"
  printf '#!/bin/sh\necho sqlx-was-called >>"%s/sqlx.calls"\n' "${BATS_TEST_TMPDIR}" >"${BATS_TEST_TMPDIR}/bin/sqlx"
  chmod +x "${BATS_TEST_TMPDIR}/bin/sqlx"
  for url in 'postgres://app:x@localhost:5432/app' 'postgres://app:x@localhost:5432/app_test_old' \
    'postgres://app:x@localhost:5432/app?options=_test' 'postgres://app:x@localhost:5432' \
    'postgres://app:x@localhost:5432/' 'postgres://app:x@localhost:5432/_test'; do
    DATABASE_URL="${url}" PATH="${BATS_TEST_TMPDIR}/bin:${PATH}" run scripts/setup-test-db.sh
    assert_failure 2
    assert_output --partial 'refusing to reset the database'
    assert_output --partial 'must end in _test'
  done
  assert_file_not_exists "${BATS_TEST_TMPDIR}/sqlx.calls"
  run env -u DATABASE_URL PATH="${BATS_TEST_TMPDIR}/bin:${PATH}" scripts/setup-test-db.sh
  assert_failure 2
  assert_output --partial 'DATABASE_URL is not set'
}

@test "setup-test-db passes the name guard for a URL that ends in _test and reaches the reachability check" {
  # Nothing listens on port 1; the stub of sleep makes the ten attempts instant.
  mkdir -p "${BATS_TEST_TMPDIR}/bin"
  printf '#!/bin/sh\nexit 0\n' >"${BATS_TEST_TMPDIR}/bin/sleep"
  printf '#!/bin/sh\nexit 0\n' >"${BATS_TEST_TMPDIR}/bin/sqlx"
  chmod +x "${BATS_TEST_TMPDIR}/bin/sleep" "${BATS_TEST_TMPDIR}/bin/sqlx"
  DATABASE_URL='postgres://app:x@127.0.0.1:1/x_test?sslmode=disable' PATH="${BATS_TEST_TMPDIR}/bin:${PATH}" run scripts/setup-test-db.sh
  assert_failure 2
  refute_output --partial 'refusing to reset'
  assert_output --partial 'the database did not accept connections after 10 attempts'
}

@test "the test targets take DATABASE_URL from .env.test even when the caller exports another one, and keep every other caller value" {
  printf '%s\n' '.RECIPEPREFIX = >' 'probe:' "> @\$(TEST_ENV) printenv APP_ENV DATABASE_URL" >"${BATS_TEST_TMPDIR}/probe.mk"
  DATABASE_URL=postgres://dev:dev@localhost:5432/app run make --no-print-directory -f Makefile -f "${BATS_TEST_TMPDIR}/probe.mk" probe
  assert_success
  assert_line --index 0 'test'
  assert_line --index 1 --regexp '^postgres://[^@]+@[^/]+/[A-Za-z0-9_]+_test$'
  # A caller value of any other variable still wins over both files.
  APP_ENV=prod DATABASE_URL=postgres://dev:dev@localhost:5432/app run make --no-print-directory -f Makefile -f "${BATS_TEST_TMPDIR}/probe.mk" probe
  assert_success
  assert_line --index 0 'prod'
  assert_line --index 1 --regexp '_test$'
  # The files follow DB_PORT, so a private stack needs no DATABASE_URL of its own.
  DB_PORT=25999 DATABASE_URL=postgres://dev:dev@localhost:5432/app run make --no-print-directory -f Makefile -f "${BATS_TEST_TMPDIR}/probe.mk" probe
  assert_success
  assert_line --index 1 --regexp ':25999/[A-Za-z0-9_]+_test$'
}

@test "setup-test-db leaves the database of a development DATABASE_URL in the caller's environment untouched" {
  require_docker
  require_host_mode "the exported DATABASE_URL does not reach the container"
  local dev_db="bats_dev_${BASHPID}_${RANDOM}" user url
  user="$(scripts/with-env.sh .env -- printenv DB_USER)"
  run make --no-print-directory ensure-database
  assert_success
  marker_database="${dev_db}"
  run server_sql "${user}" "create database ${dev_db}"
  assert_success
  url="$(scripts/with-env.sh .env -- bash -c "printf '%s' \"postgres://\${DB_USER}:\${DB_PASSWORD}@\${DB_HOST}:\${DB_PORT}/\$1\"" _ "${dev_db}")"
  run "${DOCKER:-docker}" compose exec -T database psql --quiet --no-psqlrc --tuples-only --no-align -U "${user}" -d "${dev_db}" \
    -c "create table bats_marker (id integer); insert into bats_marker values (7)"
  assert_success

  DATABASE_URL="${url}" run make --no-print-directory setup-test-db
  assert_success

  run "${DOCKER:-docker}" compose exec -T database psql --quiet --no-psqlrc --tuples-only --no-align -U "${user}" -d "${dev_db}" \
    -c "select id from bats_marker"
  assert_success
  assert_output '7'
  run server_sql "${user}" "select count(*) from pg_database where datname = 'app_test'"
  assert_success
  assert_output '1'
}

# --- G11: unit tests ---------------------------------------------------------------------------------------

@test "G11: a failing unit test makes unit-tests fail" {
  plant_append xtask/src/cli/tests.rs "${BATS_TEST_DIRNAME}/fixtures/failing_unit_test.rs.txt"
  run make --no-print-directory unit-tests
  assert_failure
  assert_output --partial 'planted_failing_unit_test'
}

@test "G11: a failing documentation example makes doc-tests fail" {
  plant_append tests/support/src/lib.rs "${BATS_TEST_DIRNAME}/fixtures/failing_doc_test.rs.txt"
  run make --no-print-directory doc-tests
  assert_failure
  assert_output --regexp 'tests/support/src/lib\.rs - planted_failing_doc_test \(line [0-9]+\)'
  assert_output --partial 'test result: FAILED'
}

# --- G12: integration tests --------------------------------------------------------------------------------

# bats test_tags=slow
@test "G12: a failing integration test makes integration-tests fail" {
  require_docker
  plant_append xtask/tests/integration/main.rs "${BATS_TEST_DIRNAME}/fixtures/failing_integration_test.rs.txt"
  run make --no-print-directory integration-tests
  assert_failure
  assert_output --partial 'planted_failing_integration_test'
}

# --- G11 and G12: the target that make ci and the hosted job run -------------------------------------------

# `make ci` (stage 3) and the hosted job `tests` run the unit and integration suites only once, through
# tests-with-coverage (DD-02 "One execution of the suites"); unit-tests and integration-tests above are the targets of
# pre-push and of the developer. The planted tests run to their failing assertion, so the coverage floor that the
# recipe checks afterwards still holds: only the failing run can fail the target, and a run line whose status the
# recipe ignores lets this test fail. The nextest profile ci has fail-fast = false, so both planted tests are reported.
# bats test_tags=slow
@test "G11, G12: a failing unit test and a failing integration test make tests-with-coverage fail and name both" {
  require_docker
  plant_append xtask/src/cli/tests.rs "${BATS_TEST_DIRNAME}/fixtures/failing_unit_test.rs.txt"
  plant_append xtask/tests/integration/main.rs "${BATS_TEST_DIRNAME}/fixtures/failing_integration_test.rs.txt"
  run make --no-print-directory tests-with-coverage
  assert_failure
  assert_output --partial 'planted_failing_unit_test'
  assert_output --partial 'planted_failing_integration_test'
}

# --- the seed of a failing randomised test (FR51, NFR29) ------------------------------------------------------

# bats test_tags=slow
@test "a failing randomised test prints TEST_SEED with the seed of the caller" {
  require_docker
  # The plant goes into an integration test binary that the template owns and synchronises and that depends on
  # test-support; it names nothing of the example context, which a service changes or removes (FR100). 7 is not the
  # seed recorded in .cargo/config.toml, so the line shows the seed that the caller set.
  plant_append crates/internal/health-check/infrastructure/tests/integration/main.rs \
    "${BATS_TEST_DIRNAME}/fixtures/failing_seeded_test.rs.txt"
  TEST_SEED=7 run make --no-print-directory integration-tests
  assert_failure
  assert_output --partial 'planted_failing_seeded_test'
  assert_output --partial 'planted by the Bats negative test (seed 7)'
  assert_line --regexp '^[[:space:]]*TEST_SEED=7$'
}

# --- G13: end-to-end tests ---------------------------------------------------------------------------------

# bats test_tags=slow
@test "G13: a feature with an undefined step makes e2e-tests fail with the unmatched step" {
  require_docker
  plant_new features/planted_undefined_step.feature "${BATS_TEST_DIRNAME}/fixtures/undefined_step.feature.txt"
  run make --no-print-directory e2e-tests
  assert_failure
  assert_output --partial "Step doesn't match any function"
}

# bats test_tags=slow
@test "G13: an undefined step fails e2e-tests even in a scenario tagged @allow.skipped" {
  require_docker
  plant_new features/planted_undefined_step_allowed.feature "${BATS_TEST_DIRNAME}/fixtures/undefined_step_allowed.feature.txt"
  run make --no-print-directory e2e-tests
  assert_failure
  assert_output --partial "Step doesn't match any function"
}

# --- G14: coverage -----------------------------------------------------------------------------------------

# bats test_tags=slow
@test "G14: an uncovered function makes tests-with-coverage fail and is listed with a zero count" {
  require_docker
  plant_append xtask/src/cli.rs "${BATS_TEST_DIRNAME}/fixtures/uncovered_function.rs.txt"
  rm -f target/coverage/lcov.info
  run make --no-print-directory tests-with-coverage
  assert_failure
  assert_file_exists target/coverage/lcov.info
  run grep -E '^FNDA:0,.*planted' target/coverage/lcov.info
  assert_success
}

# --- G15: mutation testing of the changed code -------------------------------------------------------------

# bats test_tags=slow
@test "G15: a function whose tests miss a mutant makes mutation-tests-diff fail with a MISSED line" {
  require_docker
  require_command cargo-mutants
  local status_before
  plant_append xtask/src/cli.rs "${BATS_TEST_DIRNAME}/fixtures/uncovered_function.rs.txt"
  status_before="$(git status --porcelain)"
  CI=true run make --no-print-directory mutation-tests-diff base_ref=HEAD "MUTANTS_IN_PLACE=${CALLER_MUTANTS_IN_PLACE}"
  assert_failure
  assert_output --regexp 'MISSED +xtask/src/cli\.rs:[0-9]+:[0-9]+: replace planted -> bool with true'
  assert_output --partial 'mutation-tests-diff] Error 2'
  # For the job log (NFR40): the compile units of third-party crates in the cargo-mutants builds, 0 on a warm rerun.
  printf '# G15: %s third-party crates compiled by cargo-mutants (MUTANTS_IN_PLACE=%s)\n' \
    "$(cat mutants.out/log/*.log 2>/dev/null | grep -E '^[[:space:]]+Compiling ' | grep -vc '(/' || true)" "${CALLER_MUTANTS_IN_PLACE}" >&3
  rm -rf mutants.out mutants.out.old
  # Built in place, cargo-mutants must have written the planted file back after each mutant and touched no other.
  assert_equal "$(cat xtask/src/cli.rs)" "$(git show HEAD:xtask/src/cli.rs && cat "${BATS_TEST_DIRNAME}/fixtures/uncovered_function.rs.txt")"
  assert_equal "$(git status --porcelain)" "${status_before}"
}

# --- G15: the switch MUTANTS_IN_PLACE (NFR40) ---------------------------------------------------------------

# Prints the cargo-mutants command of a dry run of `make <arguments>`, blanks squeezed and trimmed. Fails unless the
# dry run succeeds and prints exactly one such command (the prerequisites only print theirs under -n).
mutants_command() {
  local out line
  out="$(make --no-print-directory -n "$@")" || return 1
  line="$(sed -n -E '/cargo mutants/{s/[[:space:]]+/ /g;s/^ //;s/ $//;p;}' <<<"${out}")"
  [[ -n "${line}" && "${line}" != *$'\n'* ]] || return 1
  printf '%s\n' "${line}"
}

@test "G15: without MUTANTS_IN_PLACE both mutation targets run cargo-mutants without --in-place" {
  run mutants_command mutation-tests
  assert_success
  assert_output --regexp ' cargo mutants --workspace --cargo-arg=--locked$'
  run mutants_command mutation-tests-diff base_ref=HEAD
  assert_success
  assert_output --regexp ' scripts/mutation-diff\.sh HEAD -- env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --workspace --cargo-arg=--locked$'
}

@test "G15: MUTANTS_IN_PLACE=1 from the environment or the command line adds --in-place and nothing else" {
  local target plain
  for target in mutation-tests mutation-tests-diff; do
    plain="$(mutants_command "${target}" base_ref=HEAD MUTANTS_SHARD=1/3)"
    assert_regex "${plain}" ' cargo mutants --workspace --cargo-arg=--locked --shard 1/3$'
    MUTANTS_IN_PLACE=1 run mutants_command "${target}" base_ref=HEAD MUTANTS_SHARD=1/3
    assert_success
    assert_output "${plain} --in-place"
    run mutants_command "${target}" base_ref=HEAD MUTANTS_SHARD=1/3 MUTANTS_IN_PLACE=1
    assert_success
    assert_output "${plain} --in-place"
  done
}

@test "G15: no other value of MUTANTS_IN_PLACE and no MUTANTS_ARGS changes the mutation command" {
  local target plain value
  for target in mutation-tests mutation-tests-diff; do
    plain="$(mutants_command "${target}" base_ref=HEAD)"
    for value in '' 0 2 11 true yes on '1 1' '1 --jobs=4' '--jobs=4 1' --in-place --jobs=4; do
      MUTANTS_IN_PLACE="${value}" run mutants_command "${target}" base_ref=HEAD
      assert_success
      assert_output "${plain}"
      run mutants_command "${target}" base_ref=HEAD "MUTANTS_IN_PLACE=${value}"
      assert_success
      assert_output "${plain}"
    done
    # The generic hook that the recorded decision removed (DV-01, DV-13) must not come back under its old name.
    MUTANTS_ARGS=--jobs=4 run mutants_command "${target}" base_ref=HEAD MUTANTS_ARGS=--jobs=4
    assert_success
    assert_output "${plain}"
  done
}

@test "G15: a MUTANTS_IN_PLACE of 1 padded with blanks adds no --in-place" {
  local target plain value
  for target in mutation-tests mutation-tests-diff; do
    plain="$(mutants_command "${target}" base_ref=HEAD)"
    for value in ' 1 ' '1 ' ' 1' $'1\t' $'\t1' $'1\n'; do
      MUTANTS_IN_PLACE="${value}" run mutants_command "${target}" base_ref=HEAD
      assert_success
      assert_output "${plain}"
    done
    # make drops the leading blanks of a command-line assignment (`MUTANTS_IN_PLACE= 1` is the value 1) and keeps the
    # trailing ones.
    for value in ' 1 ' '1 ' $'1\t'; do
      run mutants_command "${target}" base_ref=HEAD "MUTANTS_IN_PLACE=${value}"
      assert_success
      assert_output "${plain}"
    done
  done
}

@test "G15: mutation-tests-diff with a base ref that does not exist fails with the base-ref message" {
  require_docker
  run make --no-print-directory mutation-tests-diff base_ref=no-such-ref
  assert_failure
  assert_output --partial 'mutation-tests-diff could not run: base ref no-such-ref is not available'
  assert_output --partial 'mutation-tests-diff] Error 2'
}

@test "G15: the script exits 2 for a base ref that does not exist and without the separator" {
  run scripts/mutation-diff.sh no-such-ref -- true
  assert_failure 2
  assert_output 'mutation-tests-diff could not run: base ref no-such-ref is not available'
  run scripts/mutation-diff.sh HEAD true
  assert_failure 2
  assert_output --partial 'mutation-tests-diff could not run: Usage:'
}

@test "G15: an empty diff passes without running the mutation command and counts zero mutants" {
  git diff --quiet HEAD -- '*.rs' || skip "the working tree has uncommitted Rust changes, so the diff is not empty"
  run scripts/mutation-diff.sh HEAD -- false
  assert_success
  assert_output --partial 'holds no production Rust change'
  run --separate-stderr scripts/mutation-diff.sh HEAD --list -- false
  assert_success
  assert_output '0'
}

@test "G15: the script appends the diff file and, with --list, prints only the number of lines the command lists" {
  plant_append xtask/src/cli.rs "${BATS_TEST_DIRNAME}/fixtures/uncovered_function.rs.txt"
  run scripts/mutation-diff.sh HEAD -- printf '%s\n'
  assert_success
  assert_output '--in-diff'$'\n''target/mutants.diff'
  assert_file_not_empty target/mutants.diff
  run --separate-stderr scripts/mutation-diff.sh HEAD --list -- printf '%s\n' one two
  assert_success
  assert_output '5'
}

@test "G15: the diff file is the same under a hostile git configuration" {
  local config="${BATS_TEST_TMPDIR}/hostile.gitconfig" attributes="${BATS_TEST_TMPDIR}/attributes" plain
  plant_append xtask/src/cli.rs "${BATS_TEST_DIRNAME}/fixtures/uncovered_function.rs.txt"
  run scripts/mutation-diff.sh HEAD -- true
  assert_success
  plain="$(cat target/mutants.diff)"
  [[ -n "${plain}" ]]
  printf '*.rs diff=hostile\n' >"${attributes}"
  cat >"${config}" <<EOF
[diff]
	mnemonicPrefix = true
	noprefix = true
	relative = true
	renames = copies
	context = 9
	external = /bin/false
[diff "hostile"]
	textconv = tr a-z A-Z
[color]
	ui = always
	diff = always
[core]
	quotePath = true
	attributesFile = ${attributes}
EOF
  GIT_CONFIG_GLOBAL="${config}" GIT_DIFF_OPTS=-u9 run scripts/mutation-diff.sh HEAD -- true
  assert_success
  # Prefixes a/ and b/, no colour escape, the content as written, the same patch as without the configuration.
  run grep -c -E '^(---|\+\+\+) [ab]/xtask/src/cli\.rs$' target/mutants.diff
  assert_output '2'
  run grep -c $'\033' target/mutants.diff
  assert_output '0'
  run grep -c 'fn planted' target/mutants.diff
  assert_output '1'
  assert_equal "$(cat target/mutants.diff)" "${plain}"
}

# --- G30: query metadata -----------------------------------------------------------------------------------

# bats test_tags=slow
@test "G30: a missing file of .sqlx makes validate-sqlx-metadata fail" {
  require_docker
  local -a files=(.sqlx/query-*.json)
  plant_keep "${files[0]}"
  rm -- "${files[0]}"
  run make --no-print-directory validate-sqlx-metadata
  assert_failure
  assert_output --partial '.sqlx is missing one or more queries'
}

# bats test_tags=slow
@test "G30: a file of .sqlx that no query generates makes validate-sqlx-metadata fail" {
  require_docker
  local -a files=(.sqlx/query-*.json)
  local name
  name="query-$(printf 'f%.0s' {1..64}).json"
  plant_new ".sqlx/${name}" "${files[0]}"
  run make --no-print-directory validate-sqlx-metadata
  assert_failure
  assert_output --partial 'potentially unused queries found in .sqlx'
  assert_output --partial 'check-sqlx-metadata.sh: .sqlx holds a query file that no current query generates'
}

# bats test_tags=slow
@test "G30: check-sqlx-metadata.sh prints its usage, keeps a failing status and fails on the unused-query warning" {
  run -2 scripts/check-sqlx-metadata.sh
  assert_output 'usage: check-sqlx-metadata.sh <command>...'

  run -7 scripts/check-sqlx-metadata.sh bash -c 'echo "query data is missing"; exit 7'
  assert_output 'query data is missing'

  run -1 scripts/check-sqlx-metadata.sh bash -c 'echo "warning: potentially unused queries found in .sqlx"'
  assert_line 'warning: potentially unused queries found in .sqlx'
  assert_line 'check-sqlx-metadata.sh: .sqlx holds a query file that no current query generates; remove it or run make sqlx-prepare'

  run -0 scripts/check-sqlx-metadata.sh bash -c 'echo "query data is up to date"'
  assert_output 'query data is up to date'
}
