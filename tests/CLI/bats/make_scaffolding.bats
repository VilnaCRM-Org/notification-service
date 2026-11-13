#!/usr/bin/env bats
# Tests of the scaffolding target (architecture-delivery.md "CLI Tests (Bats)", file make_scaffolding.bats):
# `make new-context group=<group> context=<context>`.
#
# The target rewrites `Cargo.toml` and `Cargo.lock` and creates `crates/bats-group/`. The test registers the two
# files and the two directories that a run can create before it runs the target (`plant_replace`, `plant_keep`,
# `plant_tree`), so the restore of the harness, which also repairs an interrupted run, removes the crates and the
# output of `git status --porcelain` is the same before and after this file. The tests that call `cargo` on the
# host are skipped on a host without a Rust toolchain (container mode).

bats_require_minimum_version 1.8.0
load test_helper

# bats file_tags=quality

setup() {
  _common_setup
  tmp="$(mktemp -d)"
  group_dir=crates/bats-group
}

teardown() {
  restore_planted
  [[ -z "${tmp:-}" ]] || rm -rf -- "${tmp}"
}

# Registers what the scaffolding changes. `Cargo.toml` is replaced by a copy of itself and `Cargo.lock` is kept, and
# the two directories that a run can create (the second only if the refusal of an existing name regresses) are
# registered as trees: the restore of the teardown, and of the next setup after an interrupted run, handles all four.
register_scaffolding_effects() {
  cp Cargo.toml "${tmp}/Cargo.toml"
  plant_replace Cargo.toml "${tmp}/Cargo.toml"
  plant_keep Cargo.lock
  plant_tree "${group_dir}"
  plant_tree crates/other-bats-group
}

@test "new-context creates three crates and the gates of the workspace still pass" {
  require_docker
  require_command cargo
  [[ ! -e "${group_dir}" ]] || skip "refusing to scaffold: ${group_dir} exists"
  local before protected_before
  local -a protected
  # Whatever `unused-dependencies` reports on the tree as it is (a finding of an earlier story is not this test's
  # subject) must be exactly what it reports after the scaffolding: the new crates add no finding.
  run make --no-print-directory unused-dependencies
  local baseline_status="${status}" baseline_output="${output}"
  register_scaffolding_effects
  before="$(tree_status)"
  # Whole files only: an entry with `#` is a section or pointer item of a file that the scaffolding legitimately
  # rewrites (`Cargo.toml`); the guard below covers those.
  mapfile -t protected < <(cargo xtask config-guard --list | grep -v '#')
  protected_before="$(git status --porcelain -- "${protected[@]}")"

  run make --no-print-directory new-context group=bats-group context=bats-context
  assert_success
  assert_output --partial 'Created the bounded context'
  assert_dir_exists "${group_dir}/bats-context/domain"
  assert_dir_exists "${group_dir}/bats-context/application"
  assert_dir_exists "${group_dir}/bats-context/infrastructure"
  assert_file_exists "${group_dir}/bats-context/domain/Cargo.toml"
  run grep -c 'crates/bats-group/bats-context/' Cargo.toml
  assert_output '3'

  run make --no-print-directory validate-manifest
  assert_success
  run make --no-print-directory architecture
  assert_success
  assert_output --partial '0 violation(s)'
  run make --no-print-directory check-crate-isolation
  assert_success
  run make --no-print-directory unused-dependencies
  if [[ "${baseline_status}" -eq 0 ]]; then
    assert_success
  else
    [[ "${status}" -eq "${baseline_status}" ]]
    [[ "${output}" == "${baseline_output}" ]]
  fi

  # No path of the protected-file list is modified: its status is what it was before the target ran.
  mapfile -t protected < <(cargo xtask config-guard --list | grep -v '#')
  [[ "${#protected[@]}" -gt 0 ]]
  [[ "${protected_before}" == "$(git status --porcelain -- "${protected[@]}")" ]]
  # The guard itself compares the working tree with the committed tree, section items included
  # (`Cargo.toml#workspace.lints`), which a pathspec cannot do.
  run cargo xtask config-guard --base HEAD
  assert_success
  [[ "${before}" != "$(tree_status)" ]]
}

@test "new-context without parameters fails with the usage line" {
  run make --no-print-directory new-context
  assert_failure
  assert_output --partial 'Usage: make new-context group=<group> context=<context>'
  assert_output --partial 'new-context] Error 2'
  run make --no-print-directory new-context group=bats-group
  assert_failure
  assert_output --partial 'Usage: make new-context group=<group> context=<context>'
  run make --no-print-directory new-context context=bats-context
  assert_failure
  assert_output --partial 'Usage: make new-context group=<group> context=<context>'
  assert_dir_not_exists "${group_dir}"
}

@test "new-context refuses a context name that already exists, in any group" {
  require_command cargo
  [[ ! -e "${group_dir}" ]] || skip "refusing to scaffold: ${group_dir} exists"
  register_scaffolding_effects
  run cargo xtask new-context bats-group bats-context
  assert_success
  run cargo xtask new-context bats-group bats-context
  assert_failure 1
  assert_output --partial 'new-context refused:'
  assert_output --partial 'already exists'
  run cargo xtask new-context other-bats-group bats-context
  assert_failure 1
  assert_output --partial 'new-context refused:'
}
