#!/usr/bin/env bats
# Negative tests of the repository gates: one test per linter of G23 (lint-repo) plus a dry run that shows lint-repo
# still runs every linter, and the Make-level tests of the documentation-drift gate G26, the architecture-model gate
# G27 and the invariants gate G31. There is no passing-run test of a gate: the passing run of lint-repo is the gate
# itself (DD-02, DD-07), and the dry run runs no linter. The template-sync checks (the synchronised Rust files, Bats
# files and fixtures, the sync ignore list read as the sync action reads it, and the inputs of the sync step that
# leave at most one sync pull request open, FR100(b)) have no Make target, so their passing runs are tests here, next
# to their negatives (architecture-delivery.md "CLI Tests (Bats)", file
# make_repository.bats). The same holds for the check of the home directory of the development stage of the
# Dockerfile (FR95).
#
# A violation is planted under a name that the linter picks up (the fixtures keep the extension .txt and are excluded
# from every linter) and is removed by restore_planted, which the teardown calls.

bats_require_minimum_version 1.8.0
load test_helper

# bats file_tags=repository

setup() {
  _common_setup
  fixtures="${REPO_ROOT}/tests/CLI/bats/fixtures"
  tmp="$(mktemp -d)"
  # The misspelt word is assembled so that this file itself passes the spelling check.
  misspelt="reci""eve"
}

teardown() {
  restore_planted
  [[ -z "${tmp:-}" ]] || rm -rf -- "${tmp}"
}

# --- G23: one test per linter -------------------------------------------------------------------------------

@test "lint-actions: a workflow with a syntax error fails with the finding of actionlint" {
  require_docker
  plant_new .github/workflows/planted_syntax_error.yml "${fixtures}/workflow_syntax_error.yml.txt"
  run make --no-print-directory lint-actions
  assert_failure
  assert_output --partial 'planted_syntax_error.yml'
  assert_output --regexp '\[(syntax-check|shell-name|expression)\]'
}

@test "lint-actions: a workflow with an unpinned action fails with the finding of zizmor" {
  require_docker
  require_command zizmor
  plant_new .github/workflows/planted_unpinned_action.yml "${fixtures}/workflow_unpinned_action.yml.txt"
  run make --no-print-directory lint-actions
  assert_failure
  assert_output --partial 'unpinned-uses'
  assert_output --partial 'planted_unpinned_action.yml'
}

@test "lint-yaml: a YAML file with a duplicate key fails with the finding of yamllint" {
  require_docker
  plant_new planted_duplicate_key.yml "${fixtures}/duplicate_key.yml.txt"
  run make --no-print-directory lint-yaml
  assert_failure
  assert_output --partial 'planted_duplicate_key.yml'
  assert_output --partial 'duplication of key'
}

@test "lint-markdown: a Markdown file with a skipped heading level fails with the finding of markdownlint" {
  require_docker
  plant_new planted_skipped_heading.md "${fixtures}/skipped_heading.md.txt"
  run make --no-print-directory lint-markdown
  assert_failure
  assert_output --partial 'planted_skipped_heading.md'
  assert_output --partial 'MD001'
}

@test "lint-shell: a script with an unquoted expansion fails with the finding of shellcheck" {
  require_docker
  plant_new scripts/planted_unquoted_expansion.sh "${fixtures}/unquoted_expansion.sh.txt"
  run make --no-print-directory lint-shell
  assert_failure
  assert_output --partial 'planted_unquoted_expansion.sh'
  assert_output --partial 'SC2086'
}

@test "lint-docker: a Dockerfile with the tag latest fails with the finding of hadolint" {
  require_docker
  run make --no-print-directory lint-docker DOCKERFILES=tests/CLI/bats/fixtures/Dockerfile.latest.txt
  assert_failure
  assert_output --partial 'DL3007'
}

@test "lint-toml: an unformatted TOML file fails with the finding of taplo" {
  require_command taplo
  plant_new planted_unformatted.toml "${fixtures}/unformatted.toml.txt"
  run make --no-print-directory lint-toml
  assert_failure
  assert_output --regexp 'not properly formatted path="[^"]*/planted_unformatted\.toml"'
}

@test "lint-spelling: a misspelt word fails with the finding of typos" {
  require_command typos
  plant_new planted_misspelt.md "${fixtures}/misspelt.md.txt"
  run make --no-print-directory lint-spelling
  assert_failure
  assert_output --partial "${misspelt}"
  assert_output --partial 'planted_misspelt.md'
}

@test "lint-spelling: a misspelt file name fails with the finding of typos" {
  require_command typos
  plant_new "planted_${misspelt}_name.md" "${fixtures}/skipped_heading.md.txt"
  run make --no-print-directory lint-spelling
  assert_failure
  assert_output --partial "${misspelt}"
  assert_output --partial "planted_${misspelt}_name.md"
}

@test "lint-spelling: a misspelt workflow name fails with the finding of typos" {
  require_command typos
  printf 'name: Planted %s\non: push\n' "${misspelt}" >"${tmp}/workflow.yml"
  plant_new .github/workflows/planted_spelling.yml "${tmp}/workflow.yml"
  run make --no-print-directory lint-spelling
  assert_failure
  assert_output --partial "${misspelt}"
  assert_output --partial '.github/workflows/planted_spelling.yml'
}

@test "lint-spelling: a misspelt file name in a hidden directory fails with the finding of typos" {
  require_command typos
  plant_new ".github/planted_${misspelt}_name.md" "${fixtures}/skipped_heading.md.txt"
  run make --no-print-directory lint-spelling
  assert_failure
  assert_output --partial "${misspelt}"
  assert_output --partial ".github/planted_${misspelt}_name.md"
}

@test "lint-prettier: an unformatted JSON file fails with the finding of Prettier" {
  require_docker
  plant_new planted_unformatted.json "${fixtures}/unformatted.json.txt"
  run make --no-print-directory lint-prettier
  assert_failure
  assert_output --partial 'planted_unformatted.json'
  assert_output --partial 'Code style issues found'
}

# lint-repo has no recipe of its own: it is the list of the eight lint-* targets above, which `make ci` (stage 1) and
# the hosted job run. A prerequisite dropped from that list would drop its linter from both while every test above,
# which runs a lint-* target directly, stays green. The dry run starts no container.
@test "G23: make -n lint-repo runs every linter of the repository lint" {
  local tool
  run make --no-print-directory -n lint-repo
  assert_success
  for tool in actionlint zizmor yamllint markdownlint-cli2 shellcheck hadolint 'taplo fmt --check' typos 'prettier --check'; do
    assert_output --partial "${tool}"
  done
}

# --- Template sync: synchronised tests stay free of what a service owns --------------------------------------
#
# A sync restores the template's copy of every file that the ignore list does not name, so a synchronised test must
# not depend on what a service owns, changes or removes (FR100). The names of the example context are assembled at
# run time below, so that this file passes its own checks.

# The entries of a sync ignore list ($1, default .templatesyncignore) as the sync action reads them: carriage returns
# removed, comments and blank lines dropped, one entry per line.
sync_ignore_entries() {
  tr -d '\r' <"${1:-.templatesyncignore}" | sed '/^[[:blank:]]*#/d;s/#.*//' |
    awk '{ for (i = 1; i <= NF; i++) print $i }'
}

# The files a commit would contain (tracked, or untracked and not ignored by git) that the sync leaves alone, for the
# ignore list $1 (default .templatesyncignore). The pinned sync action hands every entry to `git reset --`
# (handle_templatesyncignore in src/sync_template.sh of AndreasAugustin/actions-template-sync v2.6.0, lines 477 to
# 490), so an entry is a git pathspec from the root of the repository, not a .gitignore pattern: `Cargo.toml` is the
# root manifest only, `dir/` is everything below that directory and `*` also matches `/`. An entry that git refuses
# fails this function, as it aborts the sync.
sync_ignored_files() {
  local files
  files="$(tr -d '\r' <"${1:-.templatesyncignore}" | sed '/^[[:blank:]]*#/d;s/#.*//' | awk NF |
    xargs -r git ls-files --cached --others --exclude-standard --)" || return 1
  [[ -z "${files}" ]] || printf '%s\n' "${files}" | LC_ALL=C sort -u
}

# The names of the example context that a synchronised Bats file or fixture must not contain.
example_context_names() {
  printf '%s\n' "company""-subdomain" "some""-module" "some""_module" "example""_api_resource" "Example""ApiResource"
}

# The Rust files a commit would contain that name the example context or build the `Adapters` literal and that the
# sync does not leave alone.
synchronised_rust_files_naming_the_example() {
  local pattern='example''_api_resource|Example''ApiResource|some''_module|Adapters \{' naming
  naming="$(git ls-files -z --cached --others --exclude-standard -- '*.rs' |
    xargs -0 -r grep -l -s -E "${pattern}" -- | LC_ALL=C sort -u)"
  [[ -n "${naming}" ]] || return 0
  LC_ALL=C comm -23 <(printf '%s\n' "${naming}") <(sync_ignored_files)
}

# The synchronised Bats files and fixtures (the files *.bats and *.bash of tests/CLI/bats and every file below its
# fixtures/ that a commit would contain, minus the files that the ignore list $1 leaves out of the sync) that depend on
# what a service owns or extends, one line `<file>:<line>: <reason>` each:
# - a line that names the example context;
# - a command line (continuation lines joined, comments skipped) that runs `diff` or `cmp` and names both a path that
#   the sync leaves alone and the fixtures: a stored copy of a file that a service extends, which fails as soon as the
#   service extends the file.
synchronised_test_files_depending_on_service_state() {
  local list="${1:-.templatesyncignore}" ignored files file
  local -a scanned=()
  ignored="$(sync_ignored_files "${list}")" || return 1
  files="$(git ls-files --cached --others --exclude-standard -- 'tests/CLI/bats/*.bats' 'tests/CLI/bats/*.bash' \
    'tests/CLI/bats/fixtures/' | LC_ALL=C sort -u | LC_ALL=C comm -23 - <(printf '%s\n' "${ignored}"))"
  [[ -n "${files}" ]] || return 0
  while IFS= read -r file; do
    scanned+=("${file}")
  done <<<"${files}"
  awk -v names="$(example_context_names | tr '\n' ' ')" '
    function path_char(c) {
      return c != "" && index("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._/-", c) > 0
    }
    function mentions(text, name,    start, at) {
      start = 1
      while ((at = index(substr(text, start), name)) > 0) {
        at += start - 1
        if (!path_char(at > 1 ? substr(text, at - 1, 1) : "") && !path_char(substr(text, at + length(name), 1))) {
          return 1
        }
        start = at + 1
      }
      return 0
    }
    BEGIN { count = split(names, name, " ") }
    FILENAME == ARGV[1] { if ($0 != "") ignored[++known] = $0; next }
    FNR == 1 { pending = "" }
    {
      for (i = 1; i <= count; i++) {
        if (index($0, name[i]) > 0) { print FILENAME ":" FNR ": names the example context (" name[i] ")"; break }
      }
      if (pending == "") first = FNR
      if ($0 ~ /\\$/) { pending = pending substr($0, 1, length($0) - 1) " "; next }
      command = pending $0
      pending = ""
      if (command ~ /^[[:space:]]*#/) next
      if (command !~ /(^|[^A-Za-z0-9_-])(diff|cmp)([^A-Za-z0-9_-]|$)/ || tolower(command) !~ /fixtures/) next
      gsub(/[$][{]?REPO_ROOT[}]?\//, "", command)
      for (i = 1; i <= known; i++) {
        if (mentions(command, ignored[i])) {
          print FILENAME ":" first ": compares " ignored[i] " with a stored copy under fixtures/"
          break
        }
      }
    }' <(printf '%s\n' "${ignored}"; sync_ignore_entries "${list}" | sed 's,/$,,' | grep -v '[*?[]') "${scanned[@]}"
}

# The entries of the ignore list $1 (default .templatesyncignore) that are not paths relative to the root of the
# repository: git refuses an entry with a leading `/` or a `..` component as outside the repository, and the sync
# aborts.
sync_ignore_entries_outside_the_root() {
  sync_ignore_entries "$@" | awk '/^\// || /(^|\/)\.\.(\/|$)/'
}

# The files that the ignore list $1 (default .templatesyncignore) keeps out of the sync although the template owns
# them and no entry names them: everything below crates/shared/, xtask/, tests/CLI/ and .github/workflows/,
# docs/README.md, and the manifests of the health-check crates (crates/internal/health-check/<layer>/Cargo.toml, the
# template-owned manifests that the directories above do not hold). No other manifest is checked: those of the
# example context and of every context a service adds are service-owned (FR100(d)), so a service lists their
# directories as it likes, and the template's own list ignores tests/support/ and tests/e2e/ whole. An entry that names
# a file is a decision of the repository (a service that uses an extension seam adds each synchronised file it
# changes), while a directory or a pattern that reaches such a file withholds template changes that nobody chose to
# withhold.
template_owned_files_kept_out_of_sync() {
  local list="${1:-.templatesyncignore}" ignored
  ignored="$(sync_ignored_files "${list}")" || return 1
  [[ -n "${ignored}" ]] || return 0
  awk '
    FILENAME == ARGV[1] { named[$0] = 1; next }
    ($0 in named) { next }
    /^(crates\/shared|xtask|tests\/CLI|\.github\/workflows)\// || $0 == "docs/README.md" { print; next }
    /^crates\/internal\/health-check\/[^\/]+\/Cargo\.toml$/ { print }' \
    <(sync_ignore_entries "${list}") <(printf '%s\n' "${ignored}")
}

@test "no synchronised Rust file names the example context or builds the Adapters literal" {
  run synchronised_rust_files_naming_the_example
  assert_success
  assert_output ''
}

@test "a planted synchronised test that names the example table is reported" {
  printf 'const TABLE: &str = "%s";\n' "example""_api_resources" >"${tmp}/planted.rs"
  plant_new crates/shared/infrastructure/tests/integration/planted_example_table.rs "${tmp}/planted.rs"
  run synchronised_rust_files_naming_the_example
  assert_success
  assert_output 'crates/shared/infrastructure/tests/integration/planted_example_table.rs'
}

@test "a planted synchronised test that names only the singular example identifier is reported" {
  printf 'use test_support::fixture::%s;\n' "example""_api_resource" >"${tmp}/planted.rs"
  # The plant names the singular identifier only, never the plural table name.
  run grep -c "example""_api_resources" "${tmp}/planted.rs"
  assert_output '0'
  plant_new crates/shared/infrastructure/tests/integration/planted_example_fixture.rs "${tmp}/planted.rs"
  run synchronised_rust_files_naming_the_example
  assert_success
  assert_output 'crates/shared/infrastructure/tests/integration/planted_example_fixture.rs'
}

@test "no synchronised Bats file or fixture names the example context or compares a service-extended file with a stored copy" {
  run synchronised_test_files_depending_on_service_state
  assert_success
  assert_output ''
}

@test "a planted synchronised Bats file or fixture that names the example context is reported" {
  printf '@test "planted" {\n  plant_append crates/%s/domain/src/lib.rs x.txt\n}\n' "company""-subdomain" \
    >"${tmp}/planted.bats"
  printf 'use %s_domain::Planted;\n' "some""_module" >"${tmp}/planted.rs.txt"
  plant_new tests/CLI/bats/planted_example_context.bats "${tmp}/planted.bats"
  plant_new tests/CLI/bats/fixtures/planted_example_context.rs.txt "${tmp}/planted.rs.txt"
  run synchronised_test_files_depending_on_service_state
  assert_success
  assert_output "$(printf '%s\n' \
    "tests/CLI/bats/fixtures/planted_example_context.rs.txt:1: names the example context (some""_module)" \
    "tests/CLI/bats/planted_example_context.bats:2: names the example context (company""-subdomain)")"
}

@test "a planted synchronised Bats file that compares a sync-ignored file with a stored copy is reported" {
  local run_diff="d""iff" run_cmp="c""mp"
  printf '@test "planted" {\n%s\n%s\n%s\n}\n' \
    "  sed '/^allow-registry = /s/=.*/= []/' deny.toml | ${run_diff} - \"\${FIXTURES}/deny.toml.txt\"" \
    "  ${run_cmp} -s \\" \
    "    .env.test \"\${FIXTURES}/env.txt\"" >"${tmp}/planted.bats"
  plant_new tests/CLI/bats/planted_stored_copy.bats "${tmp}/planted.bats"
  run synchronised_test_files_depending_on_service_state
  assert_success
  assert_output "$(printf '%s\n' \
    'tests/CLI/bats/planted_stored_copy.bats:2: compares deny.toml with a stored copy under fixtures/' \
    'tests/CLI/bats/planted_stored_copy.bats:3: compares .env.test with a stored copy under fixtures/')"
}

@test "a Bats file that the sync ignore list leaves out of the sync is not checked" {
  printf '  plant_append crates/%s/domain/src/lib.rs x.txt\n' "company""-subdomain" >"${tmp}/planted.bats"
  plant_new tests/CLI/bats/planted_service_test.bats "${tmp}/planted.bats"
  cp .templatesyncignore "${tmp}/list"
  printf 'tests/CLI/bats/planted_service_test.bats\n' >>"${tmp}/list"
  run synchronised_test_files_depending_on_service_state "${tmp}/list"
  assert_success
  assert_output ''
}

@test "the sync ignore list is read as git pathspecs from the root, as the sync action applies it" {
  local repo="${tmp}/repo" path expected
  local -a paths=(Cargo.toml Cargo.lock README.md docs/README.md crates/app/Cargo.toml crates/app/src/lib.rs
    tests/support/Cargo.toml tests/support/src/lib.rs)
  git init -q "${repo}"
  for path in "${paths[@]}"; do
    mkdir -p "${repo}/$(dirname "${path}")"
    printf 'template\n' >"${repo}/${path}"
  done
  printf '%s\n' '# service-owned files' 'Cargo.toml' 'README.md' '' \
    'crates/*.rs  # every Rust file below crates/, because * also matches /' 'tests/support/' \
    >"${repo}/.templatesyncignore"
  expected="$(printf '%s\n' Cargo.toml README.md crates/app/src/lib.rs tests/support/Cargo.toml tests/support/src/lib.rs)"
  cd "${repo}" || return 1
  run sync_ignored_files
  assert_success
  assert_output "${expected}"
  # The command of the pinned action (src/sync_template.sh, line 484) after a change of every file: the files it
  # unstages are the files that the sync leaves alone.
  git add -A
  git -c user.name=bats -c user.email=bats@localhost -c commit.gpgsign=false commit -q --no-verify -m template
  for path in "${paths[@]}"; do
    printf 'service\n' >>"${path}"
  done
  git add -A
  tr -d '\r' <.templatesyncignore | sed '/^[[:blank:]]*#/d;s/#.*//' | awk NF | xargs -r git reset -q --
  run git -c core.quotePath=false diff --no-color --name-only
  assert_output "${expected}"
  # Read as .gitignore patterns, the same list names other files (docs/README.md and crates/app/Cargo.toml, but not
  # crates/app/src/lib.rs), so this fixture tells the two readings apart.
  run git ls-files --cached --ignored --exclude-from=.templatesyncignore
  assert_output "$(printf '%s\n' Cargo.toml README.md crates/app/Cargo.toml docs/README.md tests/support/Cargo.toml \
    tests/support/src/lib.rs)"
  cd "${REPO_ROOT}" || return 1
}

@test "every sync ignore entry is a root-relative pathspec" {
  run sync_ignore_entries_outside_the_root
  assert_success
  assert_output ''
}

@test "a sync ignore entry with a leading slash or a parent-directory component is reported, and git refuses it" {
  cp .templatesyncignore "${tmp}/list"
  printf '/Cargo.toml\n../Cargo.toml  # outside\n' >>"${tmp}/list"
  run sync_ignore_entries_outside_the_root "${tmp}/list"
  assert_success
  assert_output "$(printf '%s\n' /Cargo.toml ../Cargo.toml)"
  run sync_ignored_files "${tmp}/list"
  assert_failure
  assert_output --partial "is outside repository"
}

@test "the sync ignore list keeps no template-owned manifest out of the sync" {
  run template_owned_files_kept_out_of_sync
  assert_success
  assert_output ''
}

@test "a pattern or a directory entry that keeps template-owned files out of the sync is reported, a named file is not" {
  cp .templatesyncignore "${tmp}/pattern"
  printf '*Cargo.toml\n' >>"${tmp}/pattern"
  run template_owned_files_kept_out_of_sync "${tmp}/pattern"
  assert_success
  assert_line 'crates/shared/infrastructure/Cargo.toml'
  assert_line 'xtask/Cargo.toml'
  assert_line 'crates/internal/health-check/infrastructure/Cargo.toml'
  refute_line 'crates/service/Cargo.toml'
  refute_line 'Cargo.toml'
  refute_line "crates/company""-subdomain/some""-module/domain/Cargo.toml"
  cp .templatesyncignore "${tmp}/directory"
  printf 'crates/shared/\n' >>"${tmp}/directory"
  run template_owned_files_kept_out_of_sync "${tmp}/directory"
  assert_success
  assert_line 'crates/shared/infrastructure/Cargo.toml'
  assert_line 'crates/shared/infrastructure/src/cache.rs'
  cp .templatesyncignore "${tmp}/named"
  printf 'crates/shared/infrastructure/Cargo.toml\ncrates/shared/infrastructure/src/cache.rs\n' >>"${tmp}/named"
  run template_owned_files_kept_out_of_sync "${tmp}/named"
  assert_success
  assert_output ''
}

@test "the directory of a context that a service adds keeps its manifests out of the sync without a report" {
  # FR100(d): the source and tests of every context a service adds are service-owned, manifests included.
  printf '[package]\nname = "invoices-domain"\n' >"${tmp}/Cargo.toml"
  plant_new crates/billing/invoices/domain/Cargo.toml "${tmp}/Cargo.toml"
  cp .templatesyncignore "${tmp}/list"
  printf 'crates/billing/\n' >>"${tmp}/list"
  run sync_ignored_files "${tmp}/list"
  assert_line 'crates/billing/invoices/domain/Cargo.toml'
  run template_owned_files_kept_out_of_sync "${tmp}/list"
  assert_success
  assert_output ''
}

# --- Template sync: one open sync pull request (FR100(b)) ---------------------------------------------------
#
# The pinned AndreasAugustin/actions-template-sync v2.6.0 names its branch after the template's main
# (`chore/template_sync_<short hash>`, src/sync_template.sh line 90). With `is_force_push_pr: true` a run for the
# template commit of the open sync pull request force-pushes that branch and edits the pull request (lines 567 and
# 601 to 602, 439 to 455); a run for a newer template commit opens the pull request of a new branch. Only
# `is_pr_cleanup: true` (default false, action.yml lines 60 to 62) makes that run close the older open pull requests
# with the label of `pr_labels` (lines 255 to 294 and 585 to 594), so that no second sync pull request stays open.
# `is_keep_branch_on_pr_cleanup: true` (default false, action.yml lines 63 to 65) closes them without `-d` (lines 285
# to 291), so the branch of a closed sync pull request, and any commit a maintainer pushed to it, is kept.

# The inputs of FR100(b) that the step of AndreasAugustin/actions-template-sync in the workflow $1 does not pass in
# its `with:` mapping, one line `missing: <input>` for each of `pr_labels: template_sync`, `is_force_push_pr: true`,
# `is_pr_cleanup: true` and `is_keep_branch_on_pr_cleanup: true`, or the one line
# `missing: actions-template-sync step` when the workflow has no such step.
# A comment, a key outside the `with:` mapping of that step and the lines of a block value do not count. awk only, so
# that it runs with mawk and BusyBox awk too.
template_sync_input_findings() {
  awk '
    function flush(    i) {
      if (sync) {
        found++
        for (i = 1; i <= count; i++) {
          if (!(required[i] in passed)) print "missing: " required[i]
        }
      }
      sync = 0
      in_with = 0
      split("", passed)
    }
    BEGIN {
      inputs = "pr_labels: template_sync|is_force_push_pr: true|is_pr_cleanup: true"
      count = split(inputs "|is_keep_branch_on_pr_cleanup: true", required, "|")
      steps_indent = -1
      item_indent = -1
      key_indent = -1
    }
    { sub(/\r$/, "") }
    /^[[:space:]]*(#|$)/ { next }
    {
      match($0, /^ */)
      indent = RLENGTH
      line = $0
      sub(/[[:space:]]+#.*$/, "", line)
      sub(/[[:space:]]+$/, "", line)
    }
    # A line left of the steps, or level with them and no item, ends the steps of a job.
    steps_indent >= 0 && (indent < steps_indent || (indent == steps_indent && line !~ /^ *- /)) {
      flush()
      steps_indent = -1
      item_indent = -1
      key_indent = -1
    }
    steps_indent < 0 {
      if (line ~ /^ *steps:$/) steps_indent = indent
      next
    }
    # A new step: its first key is read like the keys below it.
    line ~ /^ *- / && (item_indent < 0 || indent == item_indent) {
      flush()
      item_indent = indent
      key_indent = indent + 2
      sub(/- /, "  ", line)
      indent = key_indent
    }
    key_indent < 0 { next }
    indent == key_indent {
      in_with = 0
      if (line ~ /^ *uses:[[:space:]]*AndreasAugustin\/actions-template-sync@/) sync = 1
      if (line ~ /^ *with:$/) {
        in_with = 1
        entry_indent = -1
      }
      next
    }
    indent > key_indent && in_with {
      if (entry_indent < 0) entry_indent = indent
      if (indent != entry_indent) next
      key = substr(line, indent + 1)
      value = key
      sub(/:.*/, "", key)
      if (value !~ /:/) next
      sub(/^[^:]*:[[:space:]]*/, "", value)
      passed[key ": " value] = 1
    }
    END {
      flush()
      if (!found) print "missing: actions-template-sync step"
    }' "$1"
}

@test "FR100(b): both sync workflows update the open sync pull request in place and close older ones" {
  local workflow findings='' reason='FR100(b): the sync step must update the open sync pull request and close older ones'
  for workflow in .github/workflows/template-sync-app.yml .github/workflows/template-sync-pat.yml; do
    # A repository keeps one flavour: the sync guides delete the workflow of the other one and add its path to the
    # sync ignore list, so that the sync does not restore it.
    if [[ ! -e "${workflow}" ]] && sync_ignore_entries | grep -qxF -- "${workflow}"; then
      continue
    fi
    [[ -f "${workflow}" ]] || fail "FR100(b): ${workflow} is missing and the sync ignore list does not name it"
    run template_sync_input_findings "${workflow}"
    assert_success
    [[ -z "${output}" ]] || findings="${findings}"$'\n'"${workflow}: ${output}"
  done
  [[ -z "${findings}" ]] || fail "${reason}:${findings}"
}

@test "FR100(b): a sync workflow without is_pr_cleanup, with it false, or without force push is reported" {
  local source=.github/workflows/template-sync-app.yml copies="${BATS_TEST_TMPDIR}"
  [[ -f "${source}" ]] || source=.github/workflows/template-sync-pat.yml
  sed '/^[[:space:]]*is_pr_cleanup:/d' "${source}" >"${copies}/no-cleanup.yml"
  sed 's/^\([[:space:]]*is_pr_cleanup:\).*/\1 false/' "${source}" >"${copies}/cleanup-false.yml"
  sed '/^[[:space:]]*is_force_push_pr:/d' "${source}" >"${copies}/no-force-push.yml"
  grep -qx '[[:space:]]*is_pr_cleanup: false' "${copies}/cleanup-false.yml" ||
    fail "the copy of ${source} does not set is_pr_cleanup: false"
  run template_sync_input_findings "${copies}/no-cleanup.yml"
  assert_success
  assert_output 'missing: is_pr_cleanup: true'
  run template_sync_input_findings "${copies}/cleanup-false.yml"
  assert_success
  assert_output 'missing: is_pr_cleanup: true'
  run template_sync_input_findings "${copies}/no-force-push.yml"
  assert_success
  assert_output 'missing: is_force_push_pr: true'
}

@test "FR100(b): a sync workflow without is_keep_branch_on_pr_cleanup, or with it false, is reported" {
  # Without the input the cleanup deletes the branch of each older sync pull request it closes, and with it the
  # commits that a maintainer pushed to that branch.
  local source=.github/workflows/template-sync-app.yml copies="${BATS_TEST_TMPDIR}"
  [[ -f "${source}" ]] || source=.github/workflows/template-sync-pat.yml
  sed '/^[[:space:]]*is_keep_branch_on_pr_cleanup:/d' "${source}" >"${copies}/no-keep-branch.yml"
  sed 's/^\([[:space:]]*is_keep_branch_on_pr_cleanup:\).*/\1 false/' "${source}" >"${copies}/keep-branch-false.yml"
  grep -qx '[[:space:]]*is_keep_branch_on_pr_cleanup: false' "${copies}/keep-branch-false.yml" ||
    fail "the copy of ${source} does not set is_keep_branch_on_pr_cleanup: false"
  run template_sync_input_findings "${copies}/no-keep-branch.yml"
  assert_success
  assert_output 'missing: is_keep_branch_on_pr_cleanup: true'
  run template_sync_input_findings "${copies}/keep-branch-false.yml"
  assert_success
  assert_output 'missing: is_keep_branch_on_pr_cleanup: true'
}

@test "FR100(b): a sync input outside the with mapping of the sync step, or a workflow without the sync step, is reported" {
  # is_pr_cleanup appears in the with mapping of another step, in the env mapping of the sync step and inside a block
  # value of its with mapping, never as an input of the sync step; the sync step starts with its uses key.
  cat >"${tmp}/elsewhere.yml" <<'EOF'
jobs:
  sync:
    steps:
      - name: Check out the repository
        uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
        with:
          is_pr_cleanup: true
      - uses: AndreasAugustin/actions-template-sync@6e1d0089686f8940604fc15baae62367bf6ee860 # v2.6.0
        env:
          is_pr_cleanup: true
        with:
          pr_labels: template_sync # the label
          hooks: |
            is_pr_cleanup: true
          is_force_push_pr: true
          is_keep_branch_on_pr_cleanup: true
  other:
    steps:
      - run: echo
        with:
          is_pr_cleanup: true
EOF
  run template_sync_input_findings "${tmp}/elsewhere.yml"
  assert_success
  assert_output 'missing: is_pr_cleanup: true'
  sed 's,AndreasAugustin/actions-template-sync@,AndreasAugustin/other-action@,' "${tmp}/elsewhere.yml" >"${tmp}/none.yml"
  run template_sync_input_findings "${tmp}/none.yml"
  assert_success
  assert_output 'missing: actions-template-sync step'
}

# --- FR95: the home directory of the development stage ------------------------------------------------------

# Reports a Dockerfile ($1) whose stage $2, with the stages it is built from, leaves the directory that HOME points to
# world-writable with the sticky bit. The common-utils feature of .devcontainer/ gives the user a .bashrc in that
# directory and then appends to it as root, which the kernel setting fs.protected_regular (set on the hosted runners)
# refuses in a sticky world-writable directory of another owner. The numeric modes of the chmod commands are applied
# in order; a symbolic mode on HOME is reported, because it is not evaluated. Exit status 0 nothing to report, 1 a
# finding, 2 the stage or its HOME is missing.
dev_home_finding() {
  awk -v want="$2" '
    function trim(s) { sub(/^[[:space:]]+/, "", s); sub(/[[:space:]]+$/, "", s); return s }
    function bare(p) { gsub(/^["\047]|["\047]$/, "", p); if (p != "/") sub(/\/+$/, "", p); return p }
    function handle(line, at,    n, i, k, ins, img, p, last) {
      n = split(trim(line), w, /[[:space:]]+/)
      ins = toupper(w[1])
      if (ins == "FROM") {
        i = 2
        while (i <= n && w[i] ~ /^--/) i++
        img = w[i]
        cur = (i + 2 <= n && toupper(w[i + 1]) == "AS") ? w[i + 2] : img
        parent[cur] = ((img in known) && img != cur) ? img : ""
        known[cur] = 1
        return
      }
      if (cur == "") return
      if (ins == "ENV") {
        if (n >= 3 && w[2] == "HOME") home[cur] = bare(w[3])
        for (i = 2; i <= n; i++) if (w[i] ~ /^HOME=/) home[cur] = bare(substr(w[i], 6))
        return
      }
      if (ins != "RUN") return
      for (i = 2; i < n; i++) {
        if (w[i] != "chmod") continue
        k = i + 1
        while (k <= n && (w[k] ~ /^--/ || w[k] ~ /^-[RcfvHLP]+$/)) k++
        for (p = k + 1; p <= n; p++) {
          if (w[p] == "&&" || w[p] == "||" || w[p] == ";" || w[p] == "|") break
          last = (w[p] ~ /;$/)
          ops++
          ostage[ops] = cur; omode[ops] = w[k]; oline[ops] = at
          opath[ops] = w[p]
          sub(/;$/, "", opath[ops])
          opath[ops] = bare(opath[ops])
          if (last) break
        }
      }
    }
    /^[[:space:]]*#/ { next }
    {
      if (buf == "") start = NR
      line = $0
      if (line ~ /\\[[:space:]]*$/) { sub(/\\[[:space:]]*$/, "", line); buf = buf line " "; next }
      handle(buf line, start)
      buf = ""
    }
    END {
      if (buf != "") handle(buf, start)
      if (!(want in known)) { printf "stage %s not found in %s\n", want, FILENAME; exit 2 }
      m = 0
      for (s = want; s != "" && m < 100; s = parent[s]) { m++; chain[m] = s; inchain[s] = 1 }
      h = ""
      for (i = m; i >= 1; i--) if (chain[i] in home) h = home[chain[i]]
      if (h == "") { printf "stage %s of %s sets no HOME\n", want, FILENAME; exit 2 }
      bad = 0
      for (i = 1; i <= ops; i++) {
        if (!(ostage[i] in inchain)) continue
        if (opath[i] != h && opath[i] != "$HOME" && opath[i] != "${HOME}") continue
        if (omode[i] !~ /^[0-7]+$/) {
          printf "%s:%d: stage %s changes HOME %s with the symbolic mode %s\n", FILENAME, oline[i], ostage[i], h, omode[i]
          exit 1
        }
        d = substr(omode[i], length(omode[i]), 1) + 0
        sticky = length(omode[i]) >= 4 && (substr(omode[i], length(omode[i]) - 3, 1) + 0) % 2 == 1
        bad = sticky && (d == 2 || d == 3 || d == 6 || d == 7)
        at = oline[i]; mode = omode[i]; stage = ostage[i]
      }
      if (bad) {
        printf "%s:%d: stage %s gives HOME %s the mode %s: world-writable with the sticky bit\n", FILENAME, at, stage, h, mode
        exit 1
      }
    }
  ' "$1"
}

# The stage of the Dockerfile that .devcontainer/devcontainer.json builds.
devcontainer_target() {
  sed -n 's/.*"target"[[:space:]]*:[[:space:]]*"\([A-Za-z0-9_.-]*\)".*/\1/p' .devcontainer/devcontainer.json
}

# A Dockerfile in ${tmp}: stage base runs $1, stage dev (built from base) runs $2 and sets HOME, stage prod runs $3.
planted_dockerfile() {
  printf 'FROM debian:13 AS base\nRUN %s\n\nFROM base AS dev\nRUN mkdir -p /cargo /home/dev \\\n && %s\nENV LANG=C.UTF-8 \\\n    HOME=/home/dev\n\nFROM base AS prod\nRUN %s\n' \
    "$1" "$2" "$3" >"${tmp}/Dockerfile"
}

@test "the development stage does not leave its home directory world-writable with the sticky bit" {
  stage="$(devcontainer_target)"
  [[ -n "${stage}" ]]
  run dev_home_finding Dockerfile "${stage}"
  assert_success
  assert_output ''
}

@test "a world-writable home directory with the sticky bit in the development stage is reported" {
  planted_dockerfile true 'chmod 1777 /cargo /home/dev /cargo-target' true
  run dev_home_finding "${tmp}/Dockerfile" dev
  assert_failure 1
  assert_output "${tmp}/Dockerfile:5: stage dev gives HOME /home/dev the mode 1777: world-writable with the sticky bit"

  planted_dockerfile 'mkdir -p /home/dev && chmod 3777 /home/dev/' true true
  run dev_home_finding "${tmp}/Dockerfile" dev
  assert_failure 1
  assert_output --partial 'stage base gives HOME /home/dev the mode 3777'

  planted_dockerfile true 'chmod 0777 /home/dev && chmod +t /home/dev' true
  run dev_home_finding "${tmp}/Dockerfile" dev
  assert_failure 1
  assert_output --partial 'stage dev changes HOME /home/dev with the symbolic mode +t'
}

@test "a home directory without the sticky bit, or one changed outside the development stage, is not reported" {
  planted_dockerfile true 'chmod 1777 /cargo /cargo-target && chmod 0777 /home/dev' 'chmod 1777 /home/dev'
  run dev_home_finding "${tmp}/Dockerfile" dev
  assert_success
  assert_output ''

  planted_dockerfile 'mkdir -p /home/dev && chmod 1777 /home/dev' 'chmod 777 "/home/dev"' true
  run dev_home_finding "${tmp}/Dockerfile" dev
  assert_success
  assert_output ''

  planted_dockerfile true 'chmod 1775 /home/dev' true
  run dev_home_finding "${tmp}/Dockerfile" dev
  assert_success
  assert_output ''

  run dev_home_finding "${tmp}/Dockerfile" builder
  assert_failure 2
  assert_output "stage builder not found in ${tmp}/Dockerfile"
}

# --- G26, G27, G31 ------------------------------------------------------------------------------------------

# bats test_tags=slow
@test "docs-check: a command that is no Make target fails with a rule (1) line" {
  printf 'Run \x60make no-such-target\x60 to see it fail.\n' >"${tmp}/line"
  plant_append README.md "${tmp}/line"
  run make --no-print-directory docs-check
  assert_failure
  assert_output --regexp '\(1\) README\.md:[0-9]+: .*no-such-target'
}

@test "validate-architecture-model: a line that does not parse fails with the error of the parser" {
  require_docker
  plant_append workspace.dsl "${fixtures}/broken_relationship.dsl.txt"
  run make --no-print-directory validate-architecture-model
  assert_failure
  assert_output --regexp 'Unexpected tokens at line [0-9]+ of /work/workspace\.dsl: planted -> nowhere'
  assert_output --partial 'does not parse'
}

@test "validate-invariants: another toolchain version fails with a rule (c) line" {
  sed -E 's/^(channel[[:space:]]*=[[:space:]]*)"[^"]*"/\1"1.0.0"/' rust-toolchain.toml >"${tmp}/toolchain"
  plant_replace rust-toolchain.toml "${tmp}/toolchain"
  run make --no-print-directory validate-invariants
  assert_failure
  assert_output --regexp '\(c\) .*toolchain versions differ'
}
