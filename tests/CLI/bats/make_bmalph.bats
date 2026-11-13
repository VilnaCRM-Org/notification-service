#!/usr/bin/env bats
# Tests of the bmalph targets (architecture-delivery.md "AI Guidance and Documentation" and
# "CLI Tests (Bats)", file make_bmalph.bats). bmalph is installed once, into an isolated HOME below the
# temporary directory of the file, so that nothing is installed globally.

bats_require_minimum_version 1.8.0
load test_helper

# bats file_tags=surface

setup_file() {
  command -v npm >/dev/null 2>&1 || return 0
  local root
  root="$(cd "${BATS_TEST_DIRNAME}/../../.." && pwd)"
  isolate_make_environment
  export BMALPH_TEST_HOME="${BATS_FILE_TMPDIR}/home"
  mkdir -p "${BMALPH_TEST_HOME}"
  set +e
  (cd "${root}" && HOME="${BMALPH_TEST_HOME}" make --no-print-directory bmalph-install) \
    >"${BATS_FILE_TMPDIR}/install.out" 2>&1
  echo "$?" >"${BATS_FILE_TMPDIR}/install.status"
}

setup() {
  _common_setup
  BMALPH_TEST_HOME="${BATS_FILE_TMPDIR}/home"
}

teardown() {
  restore_planted
}

@test "make help lists the five bmalph targets" {
  run make --no-print-directory help
  assert_success
  local target
  for target in bmalph-install bmalph-claude bmalph-codex bmalph-init bmalph-setup; do
    assert_line --regexp "^${target} +[A-Z]"
  done
}

@test "bmalph-install in an isolated HOME prints the pinned version and the dry-run verification line" {
  require_command npm
  local version output_text
  version="$(pinned_value BMALPH_VERSION)"
  [[ -n "${version}" ]]
  output_text="$(cat "${BATS_FILE_TMPDIR}/install.out")"
  assert_equal "$(cat "${BATS_FILE_TMPDIR}/install.status")" "0"
  output="${output_text}"
  assert_output --partial 'BMALPH installed:'
  assert_output --partial "  - version: ${version}"
  assert_output --partial "  - package: bmalph@${version}"
  assert_output --partial "  - command: ${BMALPH_TEST_HOME}/.npm-global/bin/bmalph"
  assert_output --partial "BMALPH dry-run verification passed for platform 'claude-code'."
}

@test "a second bmalph-install reuses the installed command" {
  require_command npm
  HOME="${BMALPH_TEST_HOME}" run make --no-print-directory bmalph-install
  assert_success
  assert_output --partial 'BMALPH installed:'
  assert_output --partial "  - version: $(pinned_value BMALPH_VERSION)"
  refute_output --partial 'added '
}

@test "bmalph-init with BMALPH_DRY_RUN=true leaves git status unchanged" {
  require_command npm
  local before
  before="$(tree_status)"
  HOME="${BMALPH_TEST_HOME}" run make --no-print-directory bmalph-init BMALPH_DRY_RUN=true
  assert_success
  assert_output --partial "Running bmalph init in '${REPO_ROOT}' for platform 'claude-code'."
  assert_output --partial "BMALPH dry-run verification passed for platform 'claude-code'."
  assert_equal "$(tree_status)" "${before}"
}

@test "bmalph-setup with BMALPH_DRY_RUN=true leaves git status unchanged" {
  require_command npm
  local before
  before="$(tree_status)"
  HOME="${BMALPH_TEST_HOME}" run make --no-print-directory bmalph-setup BMALPH_DRY_RUN=true
  assert_success
  assert_output --partial "Running bmalph init in '${REPO_ROOT}' for platform 'claude-code'."
  assert_equal "$(tree_status)" "${before}"
}

@test "an unsupported BMALPH_PLATFORM exits 1" {
  HOME="${BATS_TEST_TMPDIR}" run make --no-print-directory bmalph-install BMALPH_PLATFORM=no-such-platform
  assert_failure
  assert_output --partial "unsupported platform 'no-such-platform'"
  run env HOME="${BATS_TEST_TMPDIR}" bash scripts/install-bmalph.sh --platform no-such-platform
  assert_failure 1
}
