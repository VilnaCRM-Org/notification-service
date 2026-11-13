#!/usr/bin/env bats
# Tests of the commit convention and of the git hooks (architecture-delivery.md "Git Hooks and Commit
# Convention" and "CLI Tests (Bats)", file make_hooks_and_commits.bats).

bats_require_minimum_version 1.8.0
load test_helper

# bats file_tags=surface

FORMAT_LINE='The commit message should be in the format: feat|fix|refactor|build|chore|ci|docs|style|perf|test(#1): description'

setup() {
  _common_setup
  tmp="$(mktemp -d)"
  hooks_path_before="$(git config --local --get core.hooksPath || true)"
}

teardown() {
  restore_planted
  [[ -z "${tmp:-}" ]] || rm -rf -- "${tmp}"
}

# The checks call the script, whose exit status is the subject (1 rejected, 2 not runnable); Make reports
# every failed recipe with its own status 2. The Make target is tested in its own test below.

# Writes a commit message file and runs the check in hook mode.
check_hook() {
  printf '%s\n' "$1" >"${tmp}/message"
  COMMIT_MSG_FILE="${tmp}/message" run scripts/check-commit-message.sh
}

# Runs the check in title mode.
check_title() {
  COMMIT_TITLE="$1" run scripts/check-commit-message.sh
}

@test "title mode accepts the examples of the convention" {
  local title
  for title in 'feat(#12): add health endpoint' 'FIX(#7): handle empty page' 'chore(#1): x'; do
    check_title "${title}"
    assert_success
    refute_output --partial 'The commit message should be in the format'
  done
}

@test "hook mode accepts the examples of the convention" {
  local title
  for title in 'feat(#12): add health endpoint' 'FIX(#7): handle empty page'; do
    check_hook "${title}"
    assert_success
  done
}

@test "hook mode checks the first line that is not a comment" {
  printf '# a comment written by git\nfix(#3): skip comment lines\n\nbody\n' >"${tmp}/message"
  COMMIT_MSG_FILE="${tmp}/message" run scripts/check-commit-message.sh
  assert_success
}

@test "hook mode accepts the subjects that git writes itself" {
  local title
  for title in 'Merge branch main into feature' 'Revert "feat(#1): add x"' 'Reapply "feat(#1): add x"' \
    'fixup! feat(#1): add x' 'squash! feat(#1): add x' 'amend! feat(#1): add x'; do
    check_hook "${title}"
    assert_success
  done
}

@test "title mode has no exemption for the subjects that git writes itself" {
  check_title 'Merge branch main into feature'
  assert_failure 1
  assert_output --partial 'got: Merge branch main into feature'
  check_title 'Reapply "feat(#1): add x"'
  assert_failure 1
  assert_output --partial 'got: Reapply "feat(#1): add x"'
}

@test "hook mode accepts the subjects that git revert writes, also for a revert of a revert" {
  require_command git
  local repo="${tmp}/revert" subject
  # Git of the throw-away repository reads no configuration and no repository location of the caller: a global
  # signing setting, revert.reference or hook path would change or stop a step, and GIT_DIR of an outer command
  # would point it at this checkout.
  unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR GIT_OBJECT_DIRECTORY GIT_CONFIG_PARAMETERS GIT_CONFIG_COUNT
  scratch_git() {
    GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git -C "${repo}" -c user.name=t \
      -c user.email=t@example.invalid -c commit.gpgsign=false -c core.hooksPath=/dev/null "$@"
  }
  mkdir -p "${repo}"
  scratch_git init --quiet
  printf 'x\n' >"${repo}/x"
  scratch_git add -- x
  scratch_git commit --quiet -m 'feat(#1): add x'
  scratch_git revert --no-edit HEAD >/dev/null
  subject="$(scratch_git log -1 --format=%s)"
  assert_equal "${subject}" 'Revert "feat(#1): add x"'
  check_hook "${subject}"
  assert_success
  # The revert of that revert: git 2.43 and later write `Reapply "<subject>"`, older git `Revert "Revert "<subject>""`.
  scratch_git revert --no-edit HEAD >/dev/null
  subject="$(scratch_git log -1 --format=%s)"
  case "${subject}" in
    'Reapply "feat(#1): add x"' | 'Revert "Revert "feat(#1): add x""') ;;
    *) fail "unexpected subject of the revert of a revert: ${subject}" ;;
  esac
  check_hook "${subject}"
  assert_success
}

# A stand-in that names itself and what it was given, for the make the hooks choose.
hook_make_stub() {
  mkdir -p "${tmp}/bin"
  cat >"$1" <<'STUB'
#!/bin/sh
echo "${0##*/} $* [$COMMIT_MSG_FILE]"
STUB
  chmod +x "$1"
}

@test "the hooks run the GNU Make named by MAKE, else gmake, else make" {
  local hook
  hook_make_stub "${tmp}/bin/make"
  hook_make_stub "${tmp}/bin/gmake"
  hook_make_stub "${tmp}/bin/custom-make"
  # gmake is preferred to the make on PATH (macOS keeps an old make there and Homebrew installs gmake).
  run env -i PATH="${tmp}/bin" .githooks/pre-commit
  assert_success
  assert_output 'gmake --no-print-directory pre-commit []'
  run env -i PATH="${tmp}/bin" .githooks/pre-push
  assert_output 'gmake --no-print-directory pre-push []'
  run env -i PATH="${tmp}/bin" .githooks/commit-msg message-file
  assert_output 'gmake --no-print-directory check-commit-message [message-file]'
  # MAKE wins over both.
  run env -i PATH="${tmp}/bin" MAKE="${tmp}/bin/custom-make" .githooks/pre-commit
  assert_output 'custom-make --no-print-directory pre-commit []'
  # Without gmake the make on PATH runs.
  rm -- "${tmp}/bin/gmake"
  for hook in pre-commit pre-push; do
    run env -i PATH="${tmp}/bin" ".githooks/${hook}"
    assert_success
    assert_output "make --no-print-directory ${hook} []"
  done
}

@test "install-hooks sets core.hooksPath and the previous value is restored" {
  require_command git
  # The recipe runs in a throw-away clone: core.hooksPath is shared by every worktree of this repository.
  git clone --quiet --local --no-hardlinks "${REPO_ROOT}" "${tmp}/clone"
  local previous
  previous="$(git -C "${tmp}/clone" config --local --get core.hooksPath || true)"
  run make --no-print-directory -C "${tmp}/clone" install-hooks
  assert_success
  run git -C "${tmp}/clone" config --local --get core.hooksPath
  assert_success
  assert_output '.githooks'
  assert_file_executable "${tmp}/clone/.githooks/commit-msg"
  if [[ -n "${previous}" ]]; then
    git -C "${tmp}/clone" config --local core.hooksPath "${previous}"
  else
    git -C "${tmp}/clone" config --local --unset core.hooksPath
  fi
  run git -C "${tmp}/clone" config --local --get core.hooksPath
  assert_output "${previous}"
  # No hook became active in this checkout.
  run git config --local --get core.hooksPath
  assert_output "${hooks_path_before}"
}

@test "G28: every rejected example exits 1 with the format line in title mode" {
  local title
  for title in 'feat: add x' 'feat(api): add x' 'feature(#12): add x' 'feat(#12):missing space' 'chore(release): v1.2.0 [skip ci]'; do
    check_title "${title}"
    assert_failure 1
    assert_output --partial "${FORMAT_LINE}"
    assert_output --partial "got: ${title}"
  done
}

@test "G28: every rejected example exits 1 with the format line in hook mode" {
  local title
  for title in 'feat: add x' 'feat(api): add x' 'feature(#12): add x' 'feat(#12):missing space' 'chore(release): v1.2.0 [skip ci]'; do
    check_hook "${title}"
    assert_failure 1
    assert_output --partial "${FORMAT_LINE}"
    assert_output --partial "got: ${title}"
  done
}

@test "G28: hook mode rejects the subjects that only resemble the ones git writes" {
  # The exemptions are the exact prefixes `Merge `, `Revert "`, `Reapply "`, `fixup! `, `squash! ` and `amend! `.
  # Each subject below starts with the word of one of them but not with the whole prefix (one character after the
  # word, the space, the quote or the bang, is missing or replaced), so it is exempt only if that prefix is widened.
  local title
  for title in 'Merge: x' 'Mergex' 'Revert x' 'Revert: x' 'Revert:"x' 'Reapply x' 'Reapply: x' 'Reapply:"x' \
    'fixup!x' 'fixup: x' 'squash!x' 'squash: x' 'amend!x' 'amend: x'; do
    check_hook "${title}"
    [[ "${status}" -eq 1 ]] || fail "hook mode did not reject '${title}' with status 1 (status ${status})"
    assert_output --partial "${FORMAT_LINE}"
    assert_output --partial "got: ${title}"
  done
}

@test "G28: neither variable set exits 2 with the usage message" {
  run env -u COMMIT_MSG_FILE -u COMMIT_TITLE scripts/check-commit-message.sh
  assert_failure 2
  assert_output --partial 'set COMMIT_MSG_FILE (hook mode) or COMMIT_TITLE (title mode)'
  run env -u COMMIT_MSG_FILE -u COMMIT_TITLE make --no-print-directory check-commit-message
  assert_failure
}

@test "G28: an unreadable message file exits 2" {
  COMMIT_MSG_FILE="${tmp}/missing" run scripts/check-commit-message.sh
  assert_failure 2
  assert_output --partial "cannot read ${tmp}/missing"
}

@test "the Make target passes the accepted title and fails the rejected one" {
  COMMIT_TITLE='feat(#12): add health endpoint' run make --no-print-directory check-commit-message
  assert_success
  COMMIT_TITLE='feat: add x' run make --no-print-directory check-commit-message
  assert_failure
  assert_output --partial "${FORMAT_LINE}"
  assert_output --partial 'got: feat: add x'
}
