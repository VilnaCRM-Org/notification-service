#!/usr/bin/env bats
# Tests of the release flow (FR98, architecture-delivery.md DD-11 and "CI/CD Workflows", release.yml step 6):
# scripts/release.sh, its pre-bump hook scripts/set-version.sh, cog.toml and the changelog template. Every test
# works in a throw-away clone, on branch main, of a small workspace that holds copies of those files; `origin` is a
# local bare repository and `gh` is a stub first on PATH, so nothing leaves the machine. cocogitto must be the
# pinned version (make/tool-versions.mk COCOGITTO_VERSION): the tests fail instead of skipping when it is missing.
# The tests run cargo on the host, so they skip only where the host has no Rust toolchain (container mode, NFR37);
# where cargo is present, a missing or different cocogitto fails the file.

bats_require_minimum_version 1.8.0
load test_helper

# bats file_tags=repository

setup_file() {
  local root template expected found
  command -v cargo >/dev/null 2>&1 || return 0
  root="$(cd "${BATS_TEST_DIRNAME}/../../.." && pwd)"
  expected="$(sed -n -E 's/^export[[:space:]]+COCOGITTO_VERSION[[:space:]]*:=[[:space:]]*([^[:space:]]+).*/\1/p' \
    "${root}/make/tool-versions.mk")"
  found="$(cog --version 2>/dev/null || true)"
  if [[ "${found}" != "cog ${expected}" ]]; then
    echo "cocogitto ${expected} is required (make install installs it); found: '${found:-no cog on PATH}'" >&2
    return 1
  fi

  isolate_git
  template="${BATS_FILE_TMPDIR}/template"
  mkdir -p "${template}/scripts" "${template}/.cog" "${template}/crates/a/src"
  cp "${root}/scripts/release.sh" "${root}/scripts/set-version.sh" "${template}/scripts/"
  cp "${root}/cog.toml" "${root}/CHANGELOG.md" "${root}/rust-toolchain.toml" "${template}/"
  cp "${root}/.cog/changelog.tera" "${template}/.cog/"
  cat >"${template}/Cargo.toml" <<'EOF'
[workspace]
members = ["crates/a"]
resolver = "3"

[workspace.package]
version = "0.1.0"
edition = "2024"

[workspace.metadata.example]
version = "9.9.9"
EOF
  printf '[package]\nname = "a"\nversion.workspace = true\nedition.workspace = true\n' >"${template}/crates/a/Cargo.toml"
  : >"${template}/crates/a/src/lib.rs"
  printf 'target/\n' >"${template}/.gitignore"
  (
    cd "${template}" &&
      cargo generate-lockfile --offline --quiet &&
      git init --quiet --initial-branch=main &&
      git add -A &&
      git commit --quiet -m 'chore(#1): initial workspace'
  )
}

# Git and cocogitto (libgit2) read no configuration of the caller: no global hooks, identity or signing.
isolate_git() {
  export RUSTUP_HOME="${RUSTUP_HOME:-${HOME}/.rustup}" CARGO_HOME="${CARGO_HOME:-${HOME}/.cargo}"
  export HOME="${BATS_FILE_TMPDIR}/home"
  mkdir -p "${HOME}"
  export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
  export GIT_AUTHOR_NAME='Release Test' GIT_AUTHOR_EMAIL='release@example.org'
  export GIT_COMMITTER_NAME='Release Test' GIT_COMMITTER_EMAIL='release@example.org'
  unset GH_TOKEN GITHUB_TOKEN GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE
}

setup() {
  _common_setup
  require_command cargo
  isolate_git
  origin="${BATS_TEST_TMPDIR}/origin.git"
  work="${BATS_TEST_TMPDIR}/work"
  stubs="${BATS_TEST_TMPDIR}/bin"
  gh_log="${BATS_TEST_TMPDIR}/gh.log"
  git clone --quiet --bare "${BATS_FILE_TMPDIR}/template" "${origin}"
  git clone --quiet "${origin}" "${work}"
  git -C "${work}" config user.name 'Release Test'
  git -C "${work}" config user.email 'release@example.org'
  mkdir -p "${stubs}"
  printf '#!/usr/bin/env bash\nprintf "%%s\\n" "$*" >>"%s"\n' "${gh_log}" >"${stubs}/gh"
  chmod +x "${stubs}/gh"
  cd "${work}" || return 1
}

teardown() {
  restore_planted
}

# Commits an empty change with the given message.
commit() {
  git commit --quiet --allow-empty -m "$1"
}

# Tags the current commit as the given release and publishes the tag.
released() {
  git tag "$1"
  git push --quiet origin "$1"
}

# Runs the release script of the clone with the stub gh.
release() {
  run env PATH="${stubs}:${PATH}" scripts/release.sh "$@"
}

# Subject of the head of main on the remote.
remote_subject() {
  git -C "${origin}" log -1 --format=%s main
}

# Asserts a release of the given tag: the remote main head is the release commit, the tag (local and remote) points
# at it, the manifests carry the version and gh created the release.
assert_released() {
  local tag="$1" version="${1#v}"
  assert_equal "$(remote_subject)" "chore(release): ${tag} [skip ci]"
  assert_equal "$(git -C "${origin}" rev-parse "${tag}^{commit}")" "$(git -C "${origin}" rev-parse main)"
  assert_equal "$(git rev-parse "${tag}^{commit}")" "$(git rev-parse HEAD)"
  run sed -n '/^\[workspace.package\]/,/^\[/p' Cargo.toml
  assert_line "version = \"${version}\""
  run grep -A1 '^name = "a"$' Cargo.lock
  assert_line "version = \"${version}\""
  run cat "${gh_log}"
  assert_line --regexp "^release create ${tag} "
}

@test "a feat without a tag releases the manifest version v0.1.0 with a Features section" {
  commit 'feat(#1): add the example'
  release
  assert_success
  assert_released v0.1.0
  run head -n 1 CHANGELOG.md
  assert_output '# Changelog'
  run cat CHANGELOG.md
  assert_line --regexp '^## v0\.1\.0 \([0-9]{4}-[0-9]{2}-[0-9]{2}\)$'
  assert_line '### Features'
  assert_line --partial 'add the example'
  assert_line --partial 'Changelog generated by [cocogitto]'
}

@test "a fix without a tag also releases v0.1.0" {
  commit 'fix(#1): repair the example'
  release
  assert_success
  assert_released v0.1.0
  run cat CHANGELOG.md
  assert_line '### Bug Fixes'
}

@test "after v0.1.0 a fix releases v0.1.1 and keeps the earlier release in the changelog" {
  commit 'feat(#1): add the example'
  release
  assert_success
  commit 'docs(#3): describe the example'
  commit 'chore(#3): tidy the example'
  commit 'fix(#2): repair the example'
  release
  assert_success
  assert_released v0.1.1
  run cat CHANGELOG.md
  assert_line --regexp '^## v0\.1\.1 \('
  assert_line --regexp '^## v0\.1\.0 \('
  assert_line '### Bug Fixes'
  assert_line '### Features'
  # The omit_from_changelog entries of cog.toml keep docs and chore out: no section but these two.
  assert_equal "$(grep '^###' CHANGELOG.md | sort -u)" $'### Bug Fixes\n### Features'
  refute_output --partial 'describe the example'
  refute_output --partial 'tidy the example'
}

@test "after v0.1.0 a feat releases v0.2.0" {
  released v0.1.0
  commit 'feat(#2): add another example'
  release
  assert_success
  assert_released v0.2.0
}

@test "a feat with a BREAKING CHANGE footer below 1.0.0 raises the minor number, not the major" {
  released v0.1.0
  commit $'feat(#2): change the example\n\nBREAKING CHANGE: the field name changed'
  release
  assert_success
  assert_released v0.2.0
  run git tag --list 'v1*'
  assert_output ''
}

# FR98(a): the footer raises the version whatever the type is; cocogitto alone ignores it below 1.0.0.
@test "after v0.1.0 a fix with a BREAKING CHANGE footer releases v0.2.0 and the changelog marks it" {
  released v0.1.0
  commit $'fix(#2): rename the field\n\nBREAKING CHANGE: the field name changed'
  release
  assert_success
  assert_released v0.2.0
  run cat CHANGELOG.md
  assert_line '### Bug Fixes'
  assert_line --regexp '^- \*\*BREAKING\*\* \*\*#2:\*\* rename the field \([0-9a-f]{7}\)$'
}

@test "after v0.1.0 a docs commit with a BREAKING CHANGE footer releases v0.2.0 and the changelog lists it" {
  released v0.1.0
  commit $'docs(#2): drop the old setting\n\nBREAKING CHANGE: the setting is gone'
  release
  assert_success
  assert_released v0.2.0
  run cat CHANGELOG.md
  assert_line '### Documentation'
  assert_line --regexp '^- \*\*BREAKING\*\* \*\*#2:\*\* drop the old setting \([0-9a-f]{7}\)$'
}

@test "after v0.1.0 a refactor with a BREAKING CHANGE footer after other footers releases v0.2.0" {
  released v0.1.0
  commit 'chore(#2): tidy'
  commit $'refactor(#2): split the module\n\nThe module is split in two.\n\nRefs: #2\nBREAKING CHANGE: the path changed'
  commit 'test(#2): cover the split'
  release
  assert_success
  assert_released v0.2.0
}

@test "from v1.0.0 a fix with a BREAKING CHANGE footer releases v2.0.0" {
  released v1.0.0
  commit $'fix(#2): rename the field\n\nBREAKING CHANGE: the field name changed'
  release
  assert_success
  assert_released v2.0.0
}

@test "without a tag a docs commit with a BREAKING CHANGE footer releases the manifest version" {
  commit $'docs(#2): drop the old setting\n\nBREAKING CHANGE: the setting is gone'
  release
  assert_success
  assert_released v0.1.0
}

@test "without a tag a breaking change in the oldest commit releases the manifest version" {
  git commit --quiet --amend -m $'docs(#1): initial workspace\n\nBREAKING CHANGE: the first layout'
  git push --quiet --force origin main
  release
  assert_success
  assert_released v0.1.0
}

@test "with only the prerelease tag v0.1.0-rc.1 a breaking change releases the manifest version, not a major bump" {
  released v0.1.0-rc.1
  commit $'feat(#2): rename the field\n\nBREAKING CHANGE: the field name changed'
  release
  assert_success
  assert_released v0.1.0
}

@test "after v0.1.0 a ! before the colon of a docs subject releases v0.2.0" {
  released v0.1.0
  commit 'docs(#2)!: drop the old setting'
  release
  assert_success
  assert_released v0.2.0
}

# The footer section is the run of paragraphs at the end of the message, after the subject, that each start with a
# footer (a token, then `: ` or ` #`); a breaking change is a line of it that starts with the upper-case token, a
# colon, a blank and a value. A paragraph of other footers after it does not hide it.
@test "after v0.1.0 a BREAKING CHANGE footer followed by a paragraph of other footers releases v0.2.0" {
  released v0.1.0
  commit $'fix(#2): rename the field\n\nThe field is renamed.\n\nBREAKING CHANGE: the field name changed\n\nCo-authored-by: Other Author <other@example.org>'
  release
  assert_success
  assert_released v0.2.0
}

# Inside the footer section: words in a sentence, a lower-case token, a missing blank after the colon and an indented
# line (the continuation of a value) are no breaking change. cocogitto marks the commit breaking for the line in the
# middle paragraph, so the script reads every line of the footer section.
@test "a BREAKING CHANGE that is not a footer token at the start of a line does not count" {
  released v0.1.0
  commit $'fix(#2): repair the example\n\nThe parser reads a line such as\nBREAKING CHANGE: y\nonly in the footer section.\n\nRefs: #2\nThis is not a BREAKING CHANGE: the field stays.\nbreaking change: lower case is no token\nBREAKING CHANGE:no blank after the colon\n  BREAKING CHANGE: an indented line continues the value'
  release
  assert_success
  assert_released v0.1.1
}

@test "after v0.1.0 a fix with a BREAKING CHANGE line inside a paragraph of its body releases v0.1.1" {
  released v0.1.0
  commit $'fix(#2): repair the example\n\nThe parser reads a line such as\nBREAKING CHANGE: y\nonly in the footer section.\n\nRefs: #2'
  release
  assert_success
  assert_released v0.1.1
}

@test "after v0.1.0 a docs commit that quotes the footer as an indented example releases nothing" {
  local before
  released v0.1.0
  commit $'docs(#2): explain the footer\n\nWrite it like this:\n\n    BREAKING CHANGE: the field name changed\n\nas the last paragraph.'
  git push --quiet origin main
  before="$(git rev-parse HEAD)"
  release
  assert_success
  assert_output --partial 'No conventional commits for your repository that required a bump'
  assert_equal "$(git -C "${origin}" rev-parse main)" "${before}"
  assert_equal "$(git tag --list)" 'v0.1.0'
  assert_file_not_exists "${gh_log}"
}

# From 1.0.0 on cocogitto alone would make a major release for these lines; the script takes the fix or feat bump.
@test "from v1.0.0 a fix with a BREAKING CHANGE line in the last paragraph of its body releases v1.0.1" {
  released v1.0.0
  commit $'fix(#2): repair the example\n\nThe parser used to read\nBREAKING CHANGE: y\nanywhere in the body.'
  release
  assert_success
  assert_released v1.0.1
}

@test "from v1.0.0 a docs commit with a BREAKING CHANGE line in its body releases nothing, a feat then v1.1.0" {
  local before
  released v1.0.0
  commit $'docs(#2): explain the footer\n\nA line such as\nBREAKING CHANGE: y\nbelongs in the footer section.'
  git push --quiet origin main
  before="$(git rev-parse HEAD)"
  release
  assert_success
  assert_output --partial 'nothing to release'
  assert_equal "$(git -C "${origin}" rev-parse main)" "${before}"
  assert_equal "$(git tag --list)" 'v1.0.0'
  assert_file_not_exists "${gh_log}"
  commit 'feat(#3): add another example'
  release
  assert_success
  assert_released v1.1.0
}

@test "--version 1.0.0 releases v1.0.0" {
  released v0.1.0
  commit 'fix(#2): repair the example'
  release --version 1.0.0
  assert_success
  assert_released v1.0.0
}

@test "only build and docs commits release nothing and end successfully" {
  local before
  commit 'build(deps): bump something'
  commit 'docs(#2): describe something'
  git push --quiet origin main
  before="$(git rev-parse HEAD)"
  release
  assert_success
  assert_output --partial 'No conventional commits for your repository that required a bump'
  assert_equal "$(git rev-parse HEAD)" "${before}"
  assert_equal "$(git -C "${origin}" rev-parse main)" "${before}"
  assert_equal "$(git tag --list)" ''
  assert_equal "$(git -C "${origin}" tag --list)" ''
  assert_file_not_exists "${gh_log}"
}

@test "a version that is not x.y.z and an unknown option exit 1" {
  commit 'fix(#1): repair the example'
  release --version 1.0
  assert_failure 1
  assert_output --partial 'the version must be x.y.z, got: 1.0'
  release --unknown
  assert_failure 1
  assert_output --partial 'usage: scripts/release.sh [--version <x.y.z>]'
  assert_equal "$(git tag --list)" ''
  assert_file_not_exists "${gh_log}"
}

@test "a remote main that moved makes the push fail and no GitHub release is created" {
  local other="${BATS_TEST_TMPDIR}/other"
  git clone --quiet "${origin}" "${other}"
  git -C "${other}" commit --quiet --allow-empty -m 'fix(#3): a change pushed meanwhile'
  git -C "${other}" push --quiet origin main
  commit 'feat(#1): add the example'
  release
  assert_failure
  assert_equal "$(remote_subject)" 'fix(#3): a change pushed meanwhile'
  assert_equal "$(git -C "${origin}" tag --list)" ''
  assert_file_not_exists "${gh_log}"
}

@test "an upper-case FIX type still produces a patch release" {
  released v0.1.0
  commit 'FIX(#7): repair the example'
  release
  assert_success
  assert_released v0.1.1
}

@test "set-version changes only [workspace.package].version and Cargo.lock, and the lock file is consistent" {
  run scripts/set-version.sh 0.3.0
  assert_success
  run git diff --name-only
  assert_output $'Cargo.lock\nCargo.toml'
  run git diff --unified=0 --no-color Cargo.toml
  assert_line '-version = "0.1.0"'
  assert_line '+version = "0.3.0"'
  assert_equal "$(git diff --unified=0 --no-color Cargo.toml | grep -c '^[-+][^-+]')" 2
  run grep -A1 '^name = "a"$' Cargo.lock
  assert_line 'version = "0.3.0"'
  run cargo metadata --locked --offline --format-version 1 --no-deps
  assert_success
}

@test "set-version with a malformed version exits 1 with the usage line" {
  run scripts/set-version.sh 1.2
  assert_failure 1
  assert_output 'usage: scripts/set-version.sh <major.minor.patch>'
  run git status --porcelain
  assert_output ''
}

@test "set-version on a manifest without the version line exits 1 and leaves Cargo.toml unchanged" {
  local before
  sed -i.bak '/^\[workspace.package\]/,/^\[/{/^version = /d;}' Cargo.toml
  rm -f Cargo.toml.bak
  before="$(cat Cargo.toml)"
  run scripts/set-version.sh 0.3.0
  assert_failure 1
  assert_output --partial 'no version line in [workspace.package] of Cargo.toml'
  assert_equal "$(cat Cargo.toml)" "${before}"
  run git diff --name-only -- Cargo.lock
  assert_output ''
}
