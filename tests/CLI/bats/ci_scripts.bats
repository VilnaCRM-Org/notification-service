#!/usr/bin/env bats
# Tests of the three scripts under scripts/ci that the workflows call (FR93, FR94, NFR39, G15; Story 10.1):
# detect-changes.sh (the `code` output that lets the code gates of a pull request run or report success without
# running), mutation-plan.sh (the size of the mutation matrix) and summarize-tests.sh (the names and results of
# G11, G12 and G14 in the job summary). Every case runs in a throw-away git repository or a temporary copy of the
# scripts, with stand-ins for cargo and scripts/mutation-diff.sh; nothing reads the network or GitHub, and the
# variables of a hosted runner (GITHUB_OUTPUT, GITHUB_STEP_SUMMARY, CI) do not change the results.

bats_require_minimum_version 1.8.0
load test_helper

# bats file_tags=repository

setup() {
  _common_setup
  export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
  export GIT_AUTHOR_NAME='CI Test' GIT_AUTHOR_EMAIL='ci@example.org'
  export GIT_COMMITTER_NAME='CI Test' GIT_COMMITTER_EMAIL='ci@example.org'
  unset BASE_SHA HEAD_SHA EVENT_NAME BASE_REF TESTS_OUTCOME JUNIT_FILE LCOV_FILE GIT_DIR GIT_WORK_TREE
  scratch="${BATS_TEST_TMPDIR}/scratch"
  mkdir -p "${scratch}"
}

teardown() {
  restore_planted
}

# --- detect-changes.sh -----------------------------------------------------------------------------------------

# A repository whose base commit holds a document and a source file; the commit is tagged `base`.
make_repository() {
  repo="${scratch}/repo"
  git init --quiet --initial-branch=main "${repo}"
  cd "${repo}" || return 1
  mkdir -p docs crates/service/src
  printf 'guide\n' >docs/guide.md
  printf 'pub fn f() {}\n' >crates/service/src/lib.rs
  git add -A
  git commit --quiet -m 'chore(#1): base'
  git tag base
}

# Commits a change of the given paths (each created or modified) on a fresh branch from `base`; sets BASE and HEAD.
change() {
  local path
  git checkout --quiet -B case base
  for path in "$@"; do
    mkdir -p "$(dirname "${path}")"
    printf 'changed %s\n' "${RANDOM}" >>"${path}"
    git add -- "${path}"
  done
  git commit --quiet -m 'test(#1): change'
  BASE="$(git rev-parse base)"
  HEAD="$(git rev-parse HEAD)"
  # Every path must be part of the change the script compares, or an empty change (which also gives code=true)
  # would pass for a real one.
  local listed
  listed="$(git -c core.quotepath=off diff --name-only "${BASE}...${HEAD}")"
  for path in "$@"; do
    grep -qxF -- "${path}" <<<"${listed}" || {
      printf 'the change does not list %s\n' "${path}"
      return 1
    }
  done
}

# Prints the code= line for the change of the given paths (or the reason the change was not made).
code_for() {
  change "$@" || return 1
  BASE_SHA="${BASE}" HEAD_SHA="${HEAD}" "${REPO_ROOT}/scripts/ci/detect-changes.sh" 2>/dev/null
}

# Asserts the answer for each path changed alone; collects every mismatch before failing.
assert_code_for_each() {
  local expected="$1" path got mismatches=''
  shift
  for path in "$@"; do
    got="$(code_for "${path}")"
    [[ "${got}" == "code=${expected}" ]] || mismatches+="${path}: ${got}"$'\n'
  done
  assert_equal "${mismatches}" ''
}

@test "detect-changes: documentation only gives code=false" {
  make_repository
  run code_for docs/x.md README.md
  assert_success
  assert_output 'code=false'
}

@test "detect-changes: documentation plus a source file gives code=true" {
  make_repository
  run code_for docs/x.md README.md crates/service/src/lib.rs
  assert_success
  assert_output 'code=true'
}

@test "detect-changes: each documentation pattern alone gives code=false" {
  make_repository
  assert_code_for_each false LICENSE .github/ISSUE_TEMPLATE/x.md .github/pull_request_template.md \
    .claude/skills/x/SKILL.md specs/x/y.md docs/requests.http crates/service/README.md
  # One file per directory pattern that `*.md` does not answer, so each pattern is needed on its own.
  assert_code_for_each false .github/ISSUE_TEMPLATE/config.yml .claude/skills/x/run.sh specs/x/data.json
}

@test "detect-changes: near misses of the documentation patterns give code=true" {
  make_repository
  assert_code_for_each true Makefile .github/workflows/x.yml scripts/x.sh Cargo.toml .claude/settings.json \
    tests/CLI/bats/fixtures/misspelt.md.txt LICENSE.txt specs.rs .github/ISSUE_TEMPLATE.yml
}

@test "detect-changes: a file renamed from docs/ to crates/ gives code=true" {
  make_repository
  git checkout --quiet -B case base
  git mv docs/guide.md crates/service/guide.rs
  git commit --quiet -m 'test(#1): rename'
  run env BASE_SHA="$(git rev-parse base)" HEAD_SHA="$(git rev-parse HEAD)" "${REPO_ROOT}/scripts/ci/detect-changes.sh"
  assert_success
  assert_output 'code=true'
}

@test "detect-changes: a file renamed from crates/ to docs/ gives code=true" {
  make_repository
  git checkout --quiet -B case base
  # The content is unchanged, so without --no-renames git lists only the new path, docs/lib.md, and a change that
  # removes a source file would count as documentation only.
  git mv crates/service/src/lib.rs docs/lib.md
  git commit --quiet -m 'test(#1): rename'
  run env BASE_SHA="$(git rev-parse base)" HEAD_SHA="$(git rev-parse HEAD)" "${REPO_ROOT}/scripts/ci/detect-changes.sh"
  assert_success
  assert_output 'code=true'
}

@test "detect-changes: unset commits, an empty change and an unresolvable commit give code=true with status 0" {
  make_repository
  run "${REPO_ROOT}/scripts/ci/detect-changes.sh"
  assert_success
  assert_output 'code=true'
  run env BASE_SHA="$(git rev-parse base)" "${REPO_ROOT}/scripts/ci/detect-changes.sh"
  assert_success
  assert_output 'code=true'
  run env BASE_SHA="$(git rev-parse base)" HEAD_SHA="$(git rev-parse base)" "${REPO_ROOT}/scripts/ci/detect-changes.sh"
  assert_success
  assert_output 'code=true'
  run --separate-stderr env BASE_SHA=0123456789abcdef0123456789abcdef01234567 HEAD_SHA="$(git rev-parse base)" \
    "${REPO_ROOT}/scripts/ci/detect-changes.sh"
  assert_success
  assert_output 'code=true'
  [[ "${stderr}" == *'could not compare 0123456789abcdef0123456789abcdef01234567'*'treating the change as code'* ]]
}

@test "detect-changes: a change of any protected item alone gives code=true" {
  local files directories path
  files="$(awk '/^files = \[/ { on = 1; next } on && /^\]/ { exit } on { gsub(/[",[:space:]]/, ""); if ($0 != "") print }' \
    "${REPO_ROOT}/protected-config.toml")"
  directories="$(sed -n -E 's/^directories = \[(.*)\]$/\1/p' "${REPO_ROOT}/protected-config.toml" | tr -d '" ' | tr ',' '\n')"
  # The lists must have been read, or the loop below proves nothing.
  [[ "$(grep -c . <<<"${files}")" -ge 40 ]]
  [[ "$(grep -c . <<<"${directories}")" -ge 4 ]]
  make_repository
  local -a paths=()
  while IFS= read -r path; do
    [[ -n "${path}" ]] && paths+=("${path}")
  done <<<"${files}"
  while IFS= read -r path; do
    [[ -n "${path}" ]] && paths+=("${path}planted.txt")
  done <<<"${directories}"
  assert_code_for_each true "${paths[@]}"
}

# --- mutation-plan.sh ------------------------------------------------------------------------------------------

# A copy of scripts/ci/mutation-plan.sh with a stand-in scripts/mutation-diff.sh (prints $DIFF_OUTPUT, exits
# $DIFF_STATUS) and a stand-in cargo first on PATH (prints $CARGO_LINES lines for `cargo mutants --list`); both
# record their arguments.
make_plan_copy() {
  plan="${scratch}/copy"
  mkdir -p "${plan}/scripts/ci" "${scratch}/bin"
  cp "${REPO_ROOT}/scripts/ci/mutation-plan.sh" "${plan}/scripts/ci/"
  cat >"${plan}/scripts/mutation-diff.sh" <<EOF
#!/usr/bin/env bash
printf '%s\n' "\$*" >>"${scratch}/diff.log"
printf '%s' "\${DIFF_OUTPUT-}"
exit "\${DIFF_STATUS:-0}"
EOF
  cat >"${scratch}/bin/cargo" <<EOF
#!/usr/bin/env bash
printf '%s\n' "\$*" >>"${scratch}/cargo.log"
[[ "\$*" == 'mutants --workspace --list' ]] || exit 9
for ((i = 1; i <= \${CARGO_LINES:-0}; i++)); do printf 'src/lib.rs:%d: replace f with ()\n' "\$i"; done
EOF
  chmod +x "${plan}/scripts/mutation-diff.sh" "${scratch}/bin/cargo"
}

# Runs the copy with the stand-in cargo; extra arguments are NAME=value settings.
plan() {
  run --separate-stderr env PATH="${scratch}/bin:${PATH}" "$@" "${plan}/scripts/ci/mutation-plan.sh"
}

# The shards line for n shards.
shards_of() {
  local n="$1" i line='['
  for ((i = 1; i <= n; i++)); do
    ((i == 1)) || line+=','
    line+="{\"index\":${i},\"shard\":\"$((i - 1))/${n}\"}"
  done
  printf 'shards=%s]' "${line}"
}

@test "mutation-plan: on a pull request 0, 1, 150, 151 and 1300 mutants give 0, 1, 1, 2 and 8 shards" {
  make_plan_copy
  local pair count shards
  for pair in 0:0 1:1 150:1 151:2 1300:8; do
    count="${pair%%:*}"
    shards="${pair#*:}"
    plan EVENT_NAME=pull_request BASE_REF=main DIFF_OUTPUT=$'Listing mutants\n'"${count}"$'\n'
    assert_success
    assert_equal "${output}" "count=${count}"$'\n'"$(shards_of "${shards}")"
  done
  run shards_of 2
  assert_output 'shards=[{"index":1,"shard":"0/2"},{"index":2,"shard":"1/2"}]'
  run shards_of 0
  assert_output 'shards=[]'
  run cat "${scratch}/diff.log"
  assert_line 'origin/main --list -- cargo mutants --workspace'
  assert_file_not_exists "${scratch}/cargo.log"
}

@test "mutation-plan: 'No mutants to filter' or no output gives count=0" {
  make_plan_copy
  plan EVENT_NAME=pull_request BASE_REF=main DIFF_OUTPUT=$'No mutants to filter\n'
  assert_success
  assert_output $'count=0\nshards=[]'
  plan EVENT_NAME=pull_request BASE_REF=main DIFF_OUTPUT=''
  assert_success
  assert_output $'count=0\nshards=[]'
}

@test "mutation-plan: a push counts the lines of cargo mutants --workspace --list" {
  make_plan_copy
  plan EVENT_NAME=push CARGO_LINES=151
  assert_success
  assert_output "count=151"$'\n'"$(shards_of 2)"
  run cat "${scratch}/cargo.log"
  assert_output 'mutants --workspace --list'
  assert_file_not_exists "${scratch}/diff.log"
}

@test "mutation-plan: an unexpected last line, an unresolvable base and missing variables exit 2" {
  make_plan_copy
  plan EVENT_NAME=pull_request BASE_REF=main DIFF_OUTPUT=$'12\nsomething else\n'
  assert_failure 2
  assert_output ''
  [[ "${stderr}" == *'unexpected output of scripts/mutation-diff.sh: something else'* ]]
  plan EVENT_NAME=pull_request BASE_REF=gone DIFF_STATUS=2
  assert_failure 2
  [[ "${stderr}" == *'could not be listed'* ]]
  plan
  assert_failure 2
  [[ "${stderr}" == *'EVENT_NAME is not set'* ]]
  plan EVENT_NAME=pull_request
  assert_failure 2
  [[ "${stderr}" == *'BASE_REF is not set'* ]]
}

# --- summarize-tests.sh ----------------------------------------------------------------------------------------

# Writes a JUnit file with one test case per argument `<suite>:<name>:<pass|fail|error>`.
junit() {
  local entry suite name result
  {
    printf '<?xml version="1.0" encoding="UTF-8"?>\n<testsuites name="nextest-run">\n'
    for entry in "$@"; do
      IFS=: read -r suite name result <<<"${entry//::/@@}"
      suite="${suite//@@/::}"
      printf '<testsuite name="%s" tests="1"><testcase name="%s" classname="%s" time="0.1">' "${suite}" "${name}" "${suite}"
      case "${result}" in
        fail) printf '<failure type="test failure">assertion failed</failure>' ;;
        error) printf '<error type="error">panicked</error>' ;;
        *) ;;
      esac
      printf '</testcase></testsuite>\n'
    done
    printf '</testsuites>\n'
  } >"${scratch}/junit.xml"
}

# Writes an lcov file with the given LF LH FNF FNH totals, split over two records.
lcov() {
  printf 'SF:a.rs\nLF:%d\nLH:%d\nFNF:%d\nFNH:%d\nend_of_record\nSF:b.rs\nLF:1\nLH:1\nFNF:1\nFNH:1\nend_of_record\n' \
    "$1" "$2" "$3" "$4" >"${scratch}/lcov.info"
}

# Runs the script with the generated reports and prints the summary file.
summarize() {
  : >"${scratch}/summary.md"
  run env JUNIT_FILE="${scratch}/junit.xml" LCOV_FILE="${scratch}/lcov.info" GITHUB_STEP_SUMMARY="${scratch}/summary.md" \
    "$@" "${REPO_ROOT}/scripts/ci/summarize-tests.sh"
  assert_success
  assert_output ''
  run cat "${scratch}/summary.md"
}

@test "summarize-tests: all passed" {
  junit 'service:unit_a:pass' 'service::integration:it_a:pass'
  lcov 10 10 2 2
  summarize
  assert_output $'- G11 Unit tests: passed\n- G12 Integration tests: passed\n- G14 Coverage floor: passed (lines 11/11, functions 3/3)'
}

@test "summarize-tests: a failed integration case fails G12 only and G14 is not run" {
  junit 'service:unit_a:pass' 'service::integration:it_a:fail' 'shared-infrastructure::integration:it_b:pass'
  lcov 10 10 2 2
  summarize
  assert_line '- G11 Unit tests: passed'
  assert_line '- G12 Integration tests: failed (1 failing)'
  assert_line '- G14 Coverage floor: not run (the test run failed before the floor was applied)'
}

@test "summarize-tests: a failed or erroring case of another suite fails G11" {
  junit 'service:unit_a:fail' 'xtask::bin/xtask:unit_b:error' 'service::integration:it_a:pass' 'service::integration_x:it_c:fail'
  lcov 10 10 2 2
  summarize
  assert_line '- G11 Unit tests: failed (3 failing)'
  assert_line '- G12 Integration tests: passed'
}

@test "summarize-tests: uncovered lines or functions fail G14 with the numbers" {
  junit 'service:unit_a:pass'
  lcov 10 9 2 2
  summarize
  assert_line '- G14 Coverage floor: failed (lines 10/11, functions 3/3; the floor is 100%)'
  lcov 10 10 2 1
  summarize
  assert_line '- G14 Coverage floor: failed (lines 11/11, functions 2/3; the floor is 100%)'
}

@test "summarize-tests: without a JUnit file every gate is not run" {
  lcov 10 10 2 2
  summarize
  assert_line "- G11 Unit tests: not run (no ${scratch}/junit.xml)"
  assert_line "- G12 Integration tests: not run (no ${scratch}/junit.xml)"
  assert_line '- G14 Coverage floor: not run (no test report)'
}

@test "summarize-tests: a failed step with clean reports fails G11 with the documentation-test text" {
  junit 'service:unit_a:pass' 'service::integration:it_a:pass'
  lcov 10 10 2 2
  summarize TESTS_OUTCOME=failure
  assert_line '- G11 Unit tests: failed (the documentation tests failed; see the job log)'
  assert_line '- G12 Integration tests: passed'
  assert_line '- G14 Coverage floor: passed (lines 11/11, functions 3/3)'
  summarize TESTS_OUTCOME=success
  assert_line '- G11 Unit tests: passed'
}

@test "summarize-tests: without GITHUB_STEP_SUMMARY the lines go to standard output" {
  junit 'service:unit_a:pass'
  lcov 1 1 1 1
  run env -u GITHUB_STEP_SUMMARY JUNIT_FILE="${scratch}/junit.xml" LCOV_FILE="${scratch}/lcov.info" \
    "${REPO_ROOT}/scripts/ci/summarize-tests.sh"
  assert_success
  assert_line '- G11 Unit tests: passed'
  assert_line '- G14 Coverage floor: passed (lines 2/2, functions 2/2)'
}
