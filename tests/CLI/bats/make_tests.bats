#!/usr/bin/env bats
# Tests of the test-level targets and their negative tests (architecture-delivery.md "CLI Tests (Bats)", file
# make_tests.bats): gates G11 to G15 and the seed of a failing randomised test (FR51).
#
# Every plant goes into the leaf crate `xtask` or into `features/`, so only little is rebuilt; the
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
