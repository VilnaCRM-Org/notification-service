#!/usr/bin/env bats
# Tests of scripts/check-invariants.sh (gate G31, rules (a) to (h)) on small trees generated in the test's
# temporary directory (architecture-delivery.md "CLI Tests (Bats)", file check_invariants.bats). The script is
# called directly, because Make reports every failed recipe with status 2 and the tests assert status 1.

bats_require_minimum_version 1.8.0
load test_helper

# bats file_tags=repository

setup() {
  _common_setup
  script="${REPO_ROOT}/scripts/check-invariants.sh"
  tree="${BATS_TEST_TMPDIR}/tree"
  d64="$(printf 'a%.0s' {1..64})"
  d40="$(printf 'b%.0s' {1..40})"
  repo_name="rust-service-"
  repo_name+="template"
  make_tree
}

teardown() {
  restore_planted
}

# --- the tree ----------------------------------------------------------------------------------------------

# Replaces the placeholders of standard input: @T@ (a tab at the start of a line), @D64@ and @D40@, @NAME@ (the
# repository name), and @CURL@ and @WGET@ (the download commands: this file is a shell script that rule (b) reads,
# so a planted download written out would count as a download of this file).
expand() {
  sed -e 's/^@T@/\t/' -e "s/@D64@/${d64}/g" -e "s/@D40@/${d40}/g" -e "s/@NAME@/${repo_name}/g" \
    -e 's/@CURL@/curl/g' -e 's/@WGET@/wget/g'
}

# Writes a file of the tree from standard input, placeholders replaced.
put() {
  local path="${tree}/$1"
  mkdir -p "$(dirname "${path}")"
  expand >"${path}"
}

# Writes a Bats file of the tree with the given file tags (the tag lines are assembled, because Bats reads a tag
# line wherever it stands in this file, also inside a here-document).
put_bats() {
  local name="$1" tag
  shift
  {
    for tag in "$@"; do
      printf '# bats %s=%s\n' file_tags "${tag}"
    done
    printf '@test "x" { true; }\n'
  } | put "tests/CLI/bats/${name}"
}

# A tree that satisfies every rule: two gates and the Bats job, three workflows, images, pins and sources.
make_tree() {
  mkdir -p "${tree}"
  put Makefile <<'EOF'
include make/tool-versions.mk
include make/gates.mk
EXEC :=

.PHONY: ci-list fmt lint bats install check

ci-list:
@T@@printf '%s\n' fmt lint bats

fmt:
@T@$(EXEC) $(FMT_CMD)

lint:
@T@$(EXEC) cargo clippy --locked

bats:
@T@scripts/hello.sh

install:
@T@scripts/install-tools.sh
@T@cargo fetch --locked

check:
@T@$(EXEC) cargo run --locked -p service -- check-config
@T@$(EXEC) cargo tree --locked | scripts/pipe.sh
EOF
  put make/gates.mk <<'EOF'
FMT_CMD = cargo fmt --all --check
EOF
  put make/tool-versions.mk <<'EOF'
export CARGO_NEXTEST_VERSION := 0.9.151
export TAPLO_VERSION         := 0.10.0
export CARGO_CHEF_VERSION    := 0.1.78
export DEVCONTAINER_CLI_VERSION := 0.90.0
export CARGO_BINSTALL_SHA256_X86_64 := @D64@
export BATS_CORE := v1.14.0 @D40@
export STRUCTURIZR_IMAGE := structurizr/structurizr:2026.10.26@sha256:@D64@
export K6_IMAGE ?= grafana/k6:2.3.0@sha256:@D64@
EOF
  put Dockerfile <<'EOF'
# syntax=docker/dockerfile:1.27@sha256:@D64@
FROM rust:1.99.1-slim-trixie@sha256:@D64@ AS base
WORKDIR /app
FROM base AS chef
ARG CARGO_CHEF_VERSION=0.1.78
RUN cargo install cargo-chef --locked --version "${CARGO_CHEF_VERSION}"
FROM chef AS builder
RUN cargo build --release
FROM gcr.io/distroless/cc-debian13:nonroot@sha256:@D64@ AS prod
LABEL org.opencontainers.image.licenses="CC0-1.0"
COPY --from=builder /app/target/release/service /app/service
EOF
  put docker-compose.yml <<'EOF'
services:
  app:
    image: ${APP_IMAGE:-service:prod}
  database:
    image: postgres:${DB_VERSION:-16}-alpine
  viewer:
    image: structurizr/structurizr:2026.10.26@sha256:@D64@
EOF
  put .devcontainer/devcontainer.json <<'EOF'
{
  "name": "service-dev",
  "features": {
    // docker-in-docker
    "ghcr.io/devcontainers/features/docker-in-docker@sha256:@D64@": {
      "moby": false
    },
    "ghcr.io/devcontainers/features/common-utils@sha256:@D64@": {}
  },
  "remoteUser": "dev"
}
EOF
  put rust-toolchain.toml <<'EOF'
[toolchain]
channel = "1.99.1"
profile = "minimal"
EOF
  put Cargo.toml <<'EOF'
[workspace]
members = ["crates/a"]

[workspace.package]
version = "0.1.0"
rust-version = "1.99.1"
license = "CC0-1.0"
EOF
  put crates/a/Cargo.toml <<'EOF'
[package]
name = "a"
version.workspace = true
license.workspace = true
EOF
  put crates/a/src/lib.rs <<'EOF'
//! Mentions of `#[allow` in a comment are not attributes.
#![forbid(unsafe_code)]

pub mod inner;
EOF
  put crates/a/src/inner.rs <<'EOF'
#![expect(clippy::unwrap_used, reason = "the unused word of a reason is not a lint group")]

pub fn inner() {}
EOF
  put LICENSE <<'EOF'
Creative Commons Legal Code

CC0 1.0 Universal
EOF
  put README.md <<'EOF'
# Service

## License

This project is released under [CC0-1.0](LICENSE).
EOF
  put tests/Load/package.json <<'EOF'
{
  "name": "load",
  "license": "CC0-1.0"
}
EOF
  put .github/openapi-spec/spec.yaml <<'EOF'
openapi: 3.1.0
info:
  title: Service API
  license:
    name: CC0-1.0
    url: https://creativecommons.org/publicdomain/zero/1.0/
  version: 1.0.0
EOF
  put scripts/hello.sh <<'EOF'
#!/usr/bin/env bash
echo hello
EOF
  put scripts/pipe.sh <<'EOF'
#!/usr/bin/env bash
cat
EOF
  put scripts/install-tools.sh <<'EOF'
#!/usr/bin/env bash
cargo install --locked --version 1.0.0 something
EOF
  put_bats sample.bats surface
  put docs/repository-settings.md <<'EOF'
# Repository settings

| Job | Check |
| --- | --- |
| `fmt` | Formatting |
| `lint` | Lint |
| `bats-group` | Bats group |
| `bats` | Bats |
EOF
  put .github/workflows/quality.yml <<'EOF'
name: Quality
on:
  pull_request:
    branches: [main]
permissions:
  contents: read
concurrency:
  group: quality
  cancel-in-progress: true
jobs:
  fmt:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@@D40@ # v7.0.1
      - uses: taiki-e/install-action@@D40@ # v2.90.3
        with:
          tool: cargo-nextest@0.9.151,taplo@0.10.0
      - run: make fmt
  lint:
    runs-on: ubuntu-24.04
    timeout-minutes: 20
    permissions:
      contents: read
      pull-requests: read
    steps:
      - uses: actions/checkout@@D40@ # v7.0.1
      - run: |
          make lint
EOF
  put .github/workflows/tests.yml <<'EOF'
name: Tests
on:
  pull_request:
    branches: [main]
permissions:
  contents: read
concurrency:
  group: tests
  cancel-in-progress: true
jobs:
  bats-group:
    runs-on: ubuntu-24.04
    timeout-minutes: 30
    strategy:
      matrix:
        include:
          - group: surface
          - group: repository
    steps:
      - uses: actions/checkout@@D40@ # v7.0.1
      - run: make bats BATS_ARGS="--filter-tags $GROUP"
        env:
          GROUP: ${{ matrix.group }}
  bats:
    runs-on: ubuntu-24.04
    timeout-minutes: 5
    steps:
      - run: echo done
EOF
}

# Runs the script on the tree.
check() {
  run "${script}" --root "${tree}"
}

# Asserts a violation: status 1 and a line that starts with the rule letter and contains the text.
assert_violation() {
  local rule="$1" text="$2"
  assert_failure 1
  assert_line --regexp "^\\(${rule}\\) .*${text}"
}

# Replaces the first occurrence of a text in a file of the tree and fails when the text is not there.
replace() {
  local path="${tree}/$1" old="$2" new="$3" content
  content="$(cat "${path}")"
  [[ "${content}" == *"${old}"* ]] || {
    echo "the text '${old}' is not in $1" >&2
    return 1
  }
  printf '%s\n' "${content/"${old}"/"${new}"}" >"${path}"
}

# Appends text to a file of the tree, placeholders replaced.
append() {
  mkdir -p "$(dirname "${tree}/$1")"
  printf '%s\n' "$2" | expand >>"${tree}/$1"
}

# --- the valid tree and the command line ---------------------------------------------------------------------

@test "a tree that satisfies every rule passes and prints nothing" {
  check
  assert_success
  assert_output ''
}

@test "an unknown option exits 2" {
  run "${script}" --nope
  assert_failure 2
}

@test "a root that is not a directory exits 2" {
  run "${script}" --root "${tree}/missing"
  assert_failure 2
}

@test "the real repository has no finding of the rules (b), (c), (e), (f), (g) and (h)" {
  run "${script}" --root "${REPO_ROOT}"
  local findings="${output}"
  run grep -E '^\((b|c|e|f|g|h)\) ' <<<"${findings}"
  assert_failure 1
}

@test "the release job and the sync job may write with their own permissions block (a)" {
  put .github/workflows/release.yml <<'EOF'
name: Release
on:
  push:
    branches: [main]
permissions:
  contents: read
concurrency:
  group: release
  cancel-in-progress: false
jobs:
  release:
    runs-on: ubuntu-24.04
    timeout-minutes: 15
    permissions:
      contents: write
    steps:
      - run: echo release
EOF
  check
  assert_success
  assert_output ''
}

# --- rule (a) ------------------------------------------------------------------------------------------------

@test "(a) uses: actions/checkout@v7 is not pinned to a commit" {
  replace .github/workflows/quality.yml "actions/checkout@${d40} # v7.0.1" 'actions/checkout@v7'
  check
  assert_violation a 'actions/checkout@v7 is not pinned'
}

@test "(a) a uses: pinned to a short SHA is refused" {
  replace .github/workflows/quality.yml "actions/checkout@${d40} # v7.0.1" 'actions/checkout@3d3c42e # v7.0.1'
  check
  assert_violation a 'is not pinned'
}

@test "(a) a workflow without permissions is refused" {
  replace .github/workflows/quality.yml $'permissions:\n  contents: read\n' ''
  check
  assert_violation a 'declares no permissions'
}

@test "(a) a workflow without concurrency is refused" {
  replace .github/workflows/quality.yml $'concurrency:\n  group: quality\n  cancel-in-progress: true\n' ''
  check
  assert_violation a 'declares no concurrency'
}

@test "(a) a job without timeout-minutes is refused" {
  replace .github/workflows/quality.yml $'    timeout-minutes: 10\n' ''
  check
  assert_violation a 'job fmt has no timeout-minutes'
}

@test "(a) a job-level permissions block without contents: read is refused" {
  replace .github/workflows/quality.yml $'      contents: read\n      pull-requests: read' '      pull-requests: read'
  check
  assert_violation a 'job lint has a permissions block without contents: read'
}

@test "(a) a top-level permissions block that grants write access is refused" {
  replace .github/workflows/quality.yml $'permissions:\n  contents: read\nconcurrency:' $'permissions:\n  contents: write\nconcurrency:'
  check
  assert_violation a 'quality.yml: line 6: the top-level permissions block grants write access'
  # One finding: the write is not reported a second time as a value.
  assert_equal "${#lines[@]}" 1
}

@test "(a) a top-level permissions: write-all is refused and a comment that says write is not" {
  replace .github/workflows/quality.yml $'permissions:\n  contents: read\nconcurrency:' $'permissions: write-all\nconcurrency:'
  check
  assert_violation a 'the top-level permissions block grants write access'
  assert_equal "${#lines[@]}" 1
  replace .github/workflows/quality.yml 'permissions: write-all' $'permissions:\n  contents: read # never write here'
  check
  assert_success
}

@test "(a) a top-level permissions value or scope behind an alias is refused" {
  # The alias names no write but stands for write-all (prettier, yamllint, actionlint and zizmor accept it).
  replace .github/workflows/quality.yml $'permissions:\n  contents: read\nconcurrency:' \
    $'env:\n  LEVEL: &level write-all\npermissions: *level\nconcurrency:'
  check
  assert_failure 1
  assert_output '(a) .github/workflows/quality.yml: the workflow has permissions: *level, but only read-all, write-all or a mapping of scopes is read'
  replace .github/workflows/quality.yml $'  LEVEL: &level write-all\npermissions: *level' \
    $'  LEVEL: &level write\npermissions:\n  contents: *level'
  check
  assert_failure 1
  assert_output '(a) .github/workflows/quality.yml: the workflow gives contents the value *level; a scope takes read, write or none'
}

# A workflow that a push starts, with one job and the given job-level permissions block (two lines).
put_push_workflow() {
  local file="$1" job="$2" scope="$3"
  put ".github/workflows/${file}" <<EOF
name: Push
on:
  push:
    branches: [main]
permissions:
  contents: read
concurrency:
  group: ${file}
  cancel-in-progress: false
jobs:
  ${job}:
    runs-on: ubuntu-24.04
    timeout-minutes: 15
    permissions:
      contents: read
      ${scope}
    steps:
      - run: echo ${job}
EOF
}

@test "(a) a pull-request workflow job with pull-requests: write is refused" {
  replace .github/workflows/quality.yml '      pull-requests: read' '      pull-requests: write # comment on the pull request'
  check
  assert_violation a 'quality.yml: job lint grants pull-requests: write$'
  # The same key quoted, in single quotes (the form prettier writes) and in double quotes.
  replace .github/workflows/quality.yml '      pull-requests: write # comment on the pull request' "      'pull-requests': write"
  check
  assert_violation a 'quality.yml: job lint grants pull-requests: write$'
  replace .github/workflows/quality.yml "      'pull-requests': write" '      "pull-requests": write'
  check
  assert_violation a 'quality.yml: job lint grants pull-requests: write$'
  # An inline mapping with a quoted key and a quoted value; its quoted contents: read counts.
  replace .github/workflows/quality.yml $'    permissions:\n      contents: read\n      "pull-requests": write' \
    "    permissions: { 'contents': read, issues: 'write' }"
  check
  assert_violation a 'quality.yml: job lint grants issues: write$'
  refute_line --partial 'without contents: read'
  # An inline mapping on the line after the key.
  replace .github/workflows/quality.yml "    permissions: { 'contents': read, issues: 'write' }" \
    $'    permissions:\n      { contents: read, packages: write }'
  check
  assert_violation a 'quality.yml: job lint grants packages: write$'
  refute_line --partial 'without contents: read'
}

@test "(a) a flow mapping that prettier breaks over several lines is read" {
  # The form prettier writes for a long flow mapping: each brace and each member on a line of its own, the members
  # deeper than the keys of a block mapping would stand.
  replace .github/workflows/quality.yml $'    permissions:\n      contents: read\n      pull-requests: read' \
    $'    permissions:\n      {\n        contents: read,\n        actions: read,\n        pull-requests: write,\n      }'
  check
  assert_failure 1
  assert_output '(a) .github/workflows/quality.yml: job lint grants pull-requests: write'
}

@test "(a) a job-level scope whose value is not read, write or none is refused" {
  # An explicit tag, a folded block scalar and an anchor with its alias spell write without the plain value; each
  # passes prettier, yamllint, actionlint and zizmor.
  replace .github/workflows/quality.yml '      pull-requests: read' '      pull-requests: !!str write'
  check
  assert_failure 1
  assert_output '(a) .github/workflows/quality.yml: job lint gives pull-requests the value !!str write; a scope takes read, write or none'
  replace .github/workflows/quality.yml '      pull-requests: !!str write' $'      pull-requests: >-\n        write'
  check
  assert_failure 1
  assert_output '(a) .github/workflows/quality.yml: job lint gives pull-requests the value >- write; a scope takes read, write or none'
  replace .github/workflows/quality.yml $'      pull-requests: >-\n        write' \
    $'      security-events: &level write\n      pull-requests: *level'
  check
  assert_violation a 'quality.yml: job lint gives security-events the value &level write; a scope takes read, write or none$'
  assert_line '(a) .github/workflows/quality.yml: job lint gives pull-requests the value *level; a scope takes read, write or none'
  # A scope without a value and an entry without a scope (actionlint rejects both too).
  replace .github/workflows/quality.yml $'      security-events: &level write\n      pull-requests: *level' \
    '      pull-requests:'
  check
  assert_failure 1
  assert_output '(a) .github/workflows/quality.yml: job lint gives pull-requests no value; a scope takes read, write or none'
  replace .github/workflows/quality.yml $'      contents: read\n      pull-requests:' '      { contents: read, issues }'
  check
  assert_failure 1
  assert_output '(a) .github/workflows/quality.yml: job lint has the permissions entry issues, which is no <scope>: <value> pair'
  # read and none pass, also quoted and with a comment.
  replace .github/workflows/quality.yml '      { contents: read, issues }' \
    $'      contents: read\n      pull-requests: \'read\' # reviews\n      issues: "none"'
  check
  assert_success
  assert_output ''
}

@test "(a) a permissions block at the end of the workflow is read" {
  append .github/workflows/quality.yml '  label:
    runs-on: ubuntu-24.04
    timeout-minutes: 5
    steps:
      - run: echo label
    permissions:
      contents: read
      issues: write'
  append docs/repository-settings.md "| \`label\` | Label |"
  check
  assert_failure 1
  assert_output '(a) .github/workflows/quality.yml: job label grants issues: write'
}

@test "(a) a job-level permissions value with an alias or a tag is refused, also in the release job" {
  # The release job of a workflow that a push starts may write any scope, but not write-all, which the alias hides.
  put_push_workflow publish.yml release 'packages: write'
  replace .github/workflows/publish.yml 'concurrency:' $'env:\n  LEVEL: &level write-all\nconcurrency:'
  replace .github/workflows/publish.yml $'    permissions:\n      contents: read\n      packages: write' '    permissions: *level'
  check
  assert_failure 1
  assert_output '(a) .github/workflows/publish.yml: job release has permissions: *level, but only read-all, write-all or a mapping of scopes is read'
  replace .github/workflows/publish.yml $'env:\n  LEVEL: &level write-all\n' ''
  replace .github/workflows/publish.yml '    permissions: *level' $'    permissions: !!map\n      contents: read\n      packages: write'
  check
  assert_failure 1
  assert_output '(a) .github/workflows/publish.yml: job release has permissions: !!map, but only read-all, write-all or a mapping of scopes is read'
  # read-all is a value of its own.
  replace .github/workflows/publish.yml $'    permissions: !!map\n      contents: read\n      packages: write' '    permissions: read-all'
  check
  assert_success
  assert_output ''
}

@test "(a) a quoted key is read as the key it names" {
  # A quoted jobs key, job id and timeout key: the timeout of the job is still read.
  replace .github/workflows/quality.yml $'jobs:\n  fmt:\n    runs-on: ubuntu-24.04\n    timeout-minutes: 10' \
    $'\'jobs\':\n  "fmt":\n    runs-on: ubuntu-24.04\n    \'timeout-minutes\': 360'
  check
  assert_violation a 'quality.yml: job fmt has timeout-minutes 360, above the limit of 45 of NFR39$'
  # A quoted permissions key of a job: its scopes are still read.
  replace .github/workflows/quality.yml $'    permissions:\n      contents: read\n      pull-requests: read' \
    $'    "permissions":\n      contents: read\n      pull-requests: write'
  check
  assert_line '(a) .github/workflows/quality.yml: job lint grants pull-requests: write'
  # A quoted uses key: the action must still be pinned to a commit.
  replace .github/workflows/quality.yml "      - uses: actions/checkout@${d40} # v7.0.1" "      - 'uses': actions/checkout@v7"
  check
  assert_line --regexp '^\(a\) \.github/workflows/quality\.yml: line [0-9]+: uses: actions/checkout@v7 is not pinned'
}

@test "(a) a quoted pull_request trigger is a pull request" {
  put_push_workflow target.yml release 'contents: write'
  replace .github/workflows/target.yml $'on:\n  push:\n    branches: [main]' $'\'on\':\n  \'pull_request\':'
  check
  assert_violation a 'target.yml: job release grants contents: write$'
  replace .github/workflows/target.yml $'\'on\':\n  \'pull_request\':' '"on": [push, "pull_request"]'
  check
  assert_violation a 'target.yml: job release grants contents: write$'
  replace .github/workflows/target.yml '"on": [push, "pull_request"]' $'\'on\':\n  - "pull_request_target"'
  check
  assert_violation a 'target.yml: job release grants contents: write$'
}

@test "(a) a job outside release and sync with packages: write is refused" {
  put_push_workflow publish.yml publish 'packages: write'
  check
  assert_violation a 'publish.yml: job publish grants packages: write$'
  # The release job of a workflow that a push starts may write the scope it uses.
  put_push_workflow publish.yml release 'packages: write'
  check
  assert_success
  assert_output ''
}

@test "(a) the release and sync jobs of a pull-request workflow may not write" {
  append .github/workflows/quality.yml '  sync:
    runs-on: ubuntu-24.04
    timeout-minutes: 15
    permissions:
      contents: write
      pull-requests: write
    steps:
      - run: echo sync'
  append docs/repository-settings.md "| \`sync\` | Sync |"
  check
  assert_violation a 'quality.yml: job sync grants contents: write$'
  assert_line '(a) .github/workflows/quality.yml: job sync grants pull-requests: write'
  # A pull_request_target trigger is a pull request too.
  put_push_workflow target.yml release 'contents: write'
  replace .github/workflows/target.yml $'  push:\n    branches: [main]' '  pull_request_target:'
  check
  assert_line '(a) .github/workflows/target.yml: job release grants contents: write'
}

@test "(a) a job-level permissions: write-all is refused, also in the release job" {
  replace .github/workflows/quality.yml $'    permissions:\n      contents: read\n      pull-requests: read' '    permissions: write-all'
  check
  assert_violation a 'quality.yml: job lint grants write-all'
  replace .github/workflows/quality.yml '    permissions: write-all' $'    permissions:\n      contents: read\n      pull-requests: read'
  put_push_workflow publish.yml release 'packages: write'
  replace .github/workflows/publish.yml $'    permissions:\n      contents: read\n      packages: write' '    permissions: write-all'
  check
  assert_violation a 'publish.yml: job release grants write-all'
}

@test "(a) a write permission among the inputs of a step is no scope of the job token" {
  append .github/workflows/quality.yml '      - uses: actions/create-github-app-token@@D40@ # v3.3.0
        with:
          permission-contents: write
          permission-pull-requests: write'
  check
  assert_success
  assert_output ''
}

@test "(a) security-events: write in the code-scanning jobs is accepted" {
  # In any other job the scope is refused.
  replace .github/workflows/quality.yml '      pull-requests: read' '      security-events: write'
  check
  assert_violation a 'quality.yml: job lint grants security-events: write$'
  replace .github/workflows/quality.yml '      security-events: write' '      pull-requests: read'
  local job
  for job in codeql clippy-sarif-upload; do
    append .github/workflows/quality.yml "  ${job}:
    runs-on: ubuntu-24.04
    timeout-minutes: 30
    permissions:
      contents: read
      security-events: write # upload of the results
    steps:
      - run: echo ${job}"
    append docs/repository-settings.md "| \`${job}\` | ${job} |"
  done
  put_push_workflow scheduled.yml code-scanning 'security-events: write'
  check
  assert_success
  assert_output ''
}

@test "(a) a job timeout above 45 minutes is refused" {
  replace .github/workflows/quality.yml '    timeout-minutes: 10' '    timeout-minutes: 46'
  check
  assert_violation a 'quality.yml: job fmt has timeout-minutes 46, above the limit of 45 of NFR39'
}

@test "(a) a job timeout that is not an integer literal is refused" {
  replace .github/workflows/quality.yml '    timeout-minutes: 10' "    timeout-minutes: \${{ fromJSON('360') }}"
  check
  assert_violation a 'quality.yml: job fmt has timeout-minutes .*, which is not an integer literal \(limit 45 of NFR39\)'
}

@test "(a) the clean-clone job above 75 minutes is refused" {
  put_push_workflow scheduled.yml clean-bootstrap-ci 'actions: read'
  replace .github/workflows/scheduled.yml '    timeout-minutes: 15' '    timeout-minutes: 76'
  check
  assert_violation a 'scheduled.yml: job clean-bootstrap-ci has timeout-minutes 76, above the limit of 75 of NFR39'
}

@test "(a) timeouts of exactly 45 and 75 are accepted" {
  replace .github/workflows/quality.yml '    timeout-minutes: 10' '    timeout-minutes: 45 # the limit'
  put_push_workflow scheduled.yml clean-bootstrap-ci 'actions: read'
  replace .github/workflows/scheduled.yml '    timeout-minutes: 15' '    timeout-minutes: 75'
  check
  assert_success
  assert_output ''
  # The limit of 75 belongs to clean-bootstrap-ci of scheduled.yml alone.
  replace .github/workflows/quality.yml '  fmt:' '  clean-bootstrap-ci:'
  replace docs/repository-settings.md "| \`fmt\` |" "| \`clean-bootstrap-ci\` |"
  replace .github/workflows/quality.yml '    timeout-minutes: 45 # the limit' '    timeout-minutes: 75'
  check
  assert_violation a 'quality.yml: job clean-bootstrap-ci has timeout-minutes 75, above the limit of 45 of NFR39'
}

# The finding for a job that the rule cannot read.
unread_job='a job is not a block mapping under a plain key at two spaces (a flow mapping, an anchor, an alias, a tag or another indentation), so its permissions and timeout cannot be read'

@test "(a) a job written as a flow mapping is refused" {
  # A one-line flow mapping hides the write and the missing timeout (prettier, yamllint, actionlint and zizmor accept
  # it).
  append docs/repository-settings.md "| \`label\` | Label |"
  append .github/workflows/quality.yml '  label: { runs-on: ubuntu-24.04, permissions: { pull-requests: write }, steps: [{ run: "true" }] }'
  check
  assert_failure 1
  assert_output "(a) .github/workflows/quality.yml: line 30: ${unread_job}"
  # The flow mapping on the line after the job key, with the timeout on a line of its own at the indentation of the
  # keys of a job (actionlint accepts it): the job is refused as a whole, its timeout line counts for nothing.
  replace .github/workflows/quality.yml '  label: { runs-on: ubuntu-24.04, permissions: { pull-requests: write }, steps: [{ run: "true" }] }' \
    $'  label:\n    { runs-on: ubuntu-24.04, permissions: { pull-requests: write }, steps: [{ run: "true" }],\n    timeout-minutes: 10\n    }'
  check
  assert_failure 1
  assert_output "(a) .github/workflows/quality.yml: line 31: ${unread_job}"
  # The same with the opening brace deeper, after a blank line, so that the timeout and the closing brace are the
  # only lines at four spaces (actionlint accepts it as well).
  replace .github/workflows/quality.yml '    { runs-on: ubuntu-24.04,' $'\n      { runs-on: ubuntu-24.04,'
  check
  assert_failure 1
  assert_output "(a) .github/workflows/quality.yml: line 32: ${unread_job}"
  # Every job in a flow mapping on the line of the jobs key, and the form prettier writes when that line is too long:
  # each brace on a line of its own (one finding, the closing brace belongs to the refused job).
  put .github/workflows/flow.yml <<'EOF'
name: Flow
on:
  push:
    branches: [main]
permissions:
  contents: read
concurrency:
  group: flow
  cancel-in-progress: false
jobs: { label: { runs-on: ubuntu-24.04, permissions: write-all, steps: [{ run: echo label }] } }
EOF
  check
  assert_line "(a) .github/workflows/flow.yml: line 10: ${unread_job}"
  assert_equal "${#lines[@]}" 2
  replace .github/workflows/flow.yml 'jobs: { label: { runs-on: ubuntu-24.04, permissions: write-all, steps: [{ run: echo label }] } }' \
    $'jobs:\n  {\n    label: { runs-on: ubuntu-24.04, permissions: write-all, steps: [{ run: echo label }] },\n  }'
  check
  assert_line "(a) .github/workflows/flow.yml: line 11: ${unread_job}"
  assert_equal "${#lines[@]}" 2
}

@test "(a) an anchored job and its alias are refused" {
  # The anchored job follows a job without a timeout: that timeout is still reported, and neither the write nor the
  # timeout of the anchored job is counted under it.
  replace .github/workflows/quality.yml $'    timeout-minutes: 20\n' ''
  append .github/workflows/quality.yml '  label: &job
    runs-on: ubuntu-24.04
    timeout-minutes: 360
    permissions:
      contents: read
      pull-requests: write
    steps:
      - run: echo label
  label-again: *job'
  check
  assert_failure 1
  assert_output "$(printf '%s\n' \
    '(a) .github/workflows/quality.yml: job lint has no timeout-minutes' \
    "(a) .github/workflows/quality.yml: line 29: ${unread_job}" \
    "(a) .github/workflows/quality.yml: line 37: ${unread_job}")"
  # A merge key that copies the anchored job under a job key of its own hides the same scopes.
  append docs/repository-settings.md "| \`label-again\` | Label again |"
  replace .github/workflows/quality.yml '  label-again: *job' $'  label-again:\n    <<: *job\n    timeout-minutes: 10'
  check
  assert_failure 1
  assert_line "(a) .github/workflows/quality.yml: line 38: ${unread_job}"
  assert_equal "${#lines[@]}" 3
  # A tag on the job hides it the same way.
  replace .github/workflows/quality.yml '  label: &job' '  label: !!map'
  replace .github/workflows/quality.yml $'  label-again:\n    <<: *job\n    timeout-minutes: 10' ''
  check
  assert_failure 1
  assert_line "(a) .github/workflows/quality.yml: line 29: ${unread_job}"
  assert_equal "${#lines[@]}" 2
}

@test "(a) a job at another indentation than two spaces is refused" {
  # yamllint and prettier refuse the indentation, but the rule must not depend on them: no job is read below it.
  put .github/workflows/indented.yml <<'EOF'
name: Indented
on:
  push:
    branches: [main]
permissions:
  contents: read
concurrency:
  group: indented
  cancel-in-progress: false
jobs:
    label:
        runs-on: ubuntu-24.04
        permissions: write-all
        steps:
            - run: echo label
EOF
  check
  assert_failure 1
  assert_output "(a) .github/workflows/indented.yml: line 11: ${unread_job}"
  # The job key at two spaces, its keys deeper than four.
  replace .github/workflows/indented.yml '    label:' '  label:'
  check
  assert_failure 1
  assert_output "(a) .github/workflows/indented.yml: line 12: ${unread_job}"
  # A comment on the jobs key and a blank line before the first job are no job, and neither a sequence entry nor the
  # closing brace of a flow mapping at the indentation of the keys of a job (yamllint and actionlint accept both) is
  # a key of the job.
  put .github/workflows/indented.yml <<'EOF'
name: Indented
on:
  push:
    branches: [main]
permissions:
  contents: read
concurrency:
  group: indented
  cancel-in-progress: false
jobs: # one job

  label:
    runs-on: ubuntu-24.04
    timeout-minutes: 5
    permissions: {
      contents: read
    }
    steps:
    - run: echo label
EOF
  check
  assert_success
  assert_output ''
}

# --- rule (b) ------------------------------------------------------------------------------------------------

@test "(b) a Dockerfile image with the tag latest is refused" {
  replace Dockerfile 'rust:1.99.1-slim-trixie' 'rust:latest'
  check
  assert_violation b 'uses the tag latest'
}

@test "(b) a Dockerfile image without a digest is refused" {
  replace Dockerfile "cc-debian13:nonroot@sha256:${d64}" 'cc-debian13:nonroot'
  check
  assert_violation b 'has no digest'
}

@test "(b) a Dockerfile image without a version tag is refused" {
  replace Dockerfile "cc-debian13:nonroot@sha256:${d64}" "cc-debian13@sha256:${d64}"
  check
  assert_violation b 'has no version tag'
}

@test "(b) a BuildKit frontend of the syntax line without a digest is refused" {
  replace Dockerfile "# syntax=docker/dockerfile:1.27@sha256:${d64}" '# syntax=docker/dockerfile:1'
  check
  assert_violation b 'Dockerfile:1: image docker/dockerfile:1 has no digest'
}

@test "(b) a BuildKit frontend of the syntax line without a version tag or with the tag latest is refused" {
  replace Dockerfile "# syntax=docker/dockerfile:1.27@sha256:${d64}" "# syntax=docker/dockerfile@sha256:${d64}"
  check
  assert_violation b 'Dockerfile:1: image docker/dockerfile@sha256:.* has no version tag'
  replace Dockerfile "# syntax=docker/dockerfile@sha256:${d64}" "# syntax=docker/dockerfile:latest@sha256:${d64}"
  check
  assert_violation b 'Dockerfile:1: image docker/dockerfile:latest@sha256:.* uses the tag latest'
}

@test "(b) an unpinned syntax line after an escape directive is refused" {
  replace Dockerfile "# syntax=docker/dockerfile:1.27@sha256:${d64}" $'# escape=`\n# check=skip=JSONArgsRecommended\n# syntax=docker/dockerfile:1'
  check
  assert_violation b 'Dockerfile:3: image docker/dockerfile:1 has no digest'
}

@test "(b) a pinned syntax line after an escape directive is accepted" {
  replace Dockerfile "# syntax=docker/dockerfile:1.27@sha256:${d64}" $'# escape=`\n# syntax=docker/dockerfile:1.27@sha256:'"${d64}"
  check
  assert_success
}

@test "(b) a syntax line after a comment is not read as a parser directive even behind an escape directive" {
  replace Dockerfile "# syntax=docker/dockerfile:1.27@sha256:${d64}" $'# escape=`\n# a comment\n# syntax=docker/dockerfile:1'
  check
  assert_success
}

@test "(b) a comment that mentions syntax after the first line is not read as the parser directive" {
  replace Dockerfile 'WORKDIR /app' $'# syntax=anything\nWORKDIR /app'
  check
  assert_success
}

@test "(b) a compose image without a digest is refused" {
  replace docker-compose.yml "structurizr/structurizr:2026.10.26@sha256:${d64}" 'structurizr/structurizr:2026.10.26'
  check
  assert_violation b 'docker-compose.yml.*has no digest'
}

@test "(b) a compose image with the tag latest is refused" {
  append docker-compose.yml '  cache:'
  append docker-compose.yml "    image: redis:latest@sha256:${d64}"
  check
  assert_violation b 'redis:latest.*uses the tag latest'
}

@test "(b) the database image with DB_VERSION is the one exception, another service is not" {
  append docker-compose.yml '  replica:'
  append docker-compose.yml "    image: postgres:\${DB_VERSION:-16}-alpine"
  check
  assert_violation b 'docker-compose.yml:[0-9]+: image postgres:.*DB_VERSION.*has no digest'
  # The exempt line of the database service is not reported: only the appended line is.
  local database_line
  database_line="$(grep -n 'image: postgres:.*DB_VERSION' "${tree}/docker-compose.yml" | head -n 1 | cut -d: -f1)"
  [[ -n "${database_line}" ]]
  refute_line --regexp "^\\(b\\) docker-compose.yml:${database_line}:"
}

@test "(b) a development-container feature referenced by tag is refused" {
  replace .devcontainer/devcontainer.json "common-utils@sha256:${d64}" 'common-utils:2'
  check
  assert_violation b 'feature ghcr.io/devcontainers/features/common-utils:2 is not referenced by digest'
}

@test "(b) a tool image of make/tool-versions.mk without a digest is refused" {
  replace make/tool-versions.mk "grafana/k6:2.3.0@sha256:${d64}" 'grafana/k6:2.3.0'
  check
  assert_violation b 'K6_IMAGE.*has no digest'
}

@test "(b) a tool version of make/tool-versions.mk that is not exact is refused" {
  replace make/tool-versions.mk 'CARGO_NEXTEST_VERSION := 0.9.151' 'CARGO_NEXTEST_VERSION := 0.9'
  check
  assert_violation b 'CARGO_NEXTEST_VERSION has no exact version'
}

@test "(b) a Bats pin without a commit SHA is refused" {
  replace make/tool-versions.mk "BATS_CORE := v1.14.0 ${d40}" 'BATS_CORE := v1.14.0'
  check
  assert_violation b 'BATS_CORE'
}

@test "(b) a tool: entry of a workflow that differs from make/tool-versions.mk is refused" {
  replace .github/workflows/quality.yml 'cargo-nextest@0.9.151' 'cargo-nextest@0.9.150'
  check
  assert_violation b 'cargo-nextest@0.9.150 differs from CARGO_NEXTEST_VERSION 0.9.151'
}

@test "(b) a tool: entry without an exact version is refused" {
  replace .github/workflows/quality.yml 'taplo@0.10.0' 'taplo'
  check
  assert_violation b 'tool taplo has no exact version'
}

@test "(b) a tool: entry that has no pin is refused" {
  replace .github/workflows/quality.yml 'taplo@0.10.0' 'unknown-tool@1.0.0'
  check
  assert_violation b 'tool unknown-tool has no pin'
}

@test "(b) the @devcontainers/cli version of a workflow that differs from the pin is refused" {
  replace .github/workflows/tests.yml '      - run: echo done' '      - run: npx --yes @devcontainers/cli@0.89.0 build --workspace-folder .'
  check
  assert_violation b '@devcontainers/cli@0.89.0 differs from DEVCONTAINER_CLI_VERSION 0.90.0'
}

@test "(b) npx without an exact version is refused" {
  replace .github/workflows/tests.yml '      - run: echo done' '      - run: npx --yes prettier --check .'
  check
  assert_violation b 'npx runs prettier without an exact version'
}

@test "(b) a cargo-chef version of the Dockerfile that differs from the pin is refused" {
  replace Dockerfile 'ARG CARGO_CHEF_VERSION=0.1.78' 'ARG CARGO_CHEF_VERSION=0.1.77'
  check
  assert_violation b 'cargo-chef 0.1.77 differs from CARGO_CHEF_VERSION 0.1.78'
}

@test "(b) a cargo install without --version in the Dockerfile is refused" {
  replace Dockerfile "--locked --version \"\${CARGO_CHEF_VERSION}\"" '--locked'
  check
  assert_violation b 'Dockerfile.*has no exact version'
}

@test "(b) a Structurizr version of a compose file that differs from the pin is refused" {
  replace docker-compose.yml "structurizr/structurizr:2026.10.26@sha256:${d64}" "structurizr/structurizr:2026.10.25@sha256:${d64}"
  check
  assert_violation b 'structurizr/structurizr:2026.10.25.*differs from STRUCTURIZR_IMAGE'
}

@test "(b) a container image of a workflow with the tag latest is refused" {
  replace .github/workflows/quality.yml $'  lint:\n    runs-on: ubuntu-24.04' $'  lint:\n    container: alpine:latest\n    runs-on: ubuntu-24.04'
  check
  assert_violation b 'alpine:latest'
}

@test "(b) a file downloaded by URL without a checksum is refused" {
  put scripts/fetch.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ --fail --location --output /tmp/tool.tar.gz https://example.org/tool.tar.gz
EOF
  check
  assert_violation b 'scripts/fetch.sh.*downloaded by URL without a checksum'
}

@test "(b) a download without a checksum in a script outside scripts/ is refused" {
  put tests/Load/fetch.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ --fail --location --output /tmp/tool.tar.gz https://example.org/tool.tar.gz
EOF
  put .githooks/pre-push <<'EOF'
#!/usr/bin/env bash
@WGET@ https://example.org/hook.tar.gz
EOF
  put tests/CLI/bats/helper.bash <<'EOF'
@CURL@ --fail --location --output /tmp/x https://example.org/x
EOF
  check
  assert_violation b 'tests/Load/fetch.sh: a file is downloaded by URL without a checksum'
  assert_line '(b) .githooks/pre-push: a file is downloaded by URL without a checksum'
  assert_line '(b) tests/CLI/bats/helper.bash: a file is downloaded by URL without a checksum'
}

@test "(b) a download in a fixture of the Bats tests is not read" {
  put tests/CLI/bats/fixtures/fetch.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ --fail --location --output /tmp/tool.tar.gz https://example.org/tool.tar.gz
EOF
  check
  assert_success
  assert_output ''
}

@test "(b) a request to the loopback interface is a probe, not a download" {
  put tests/CLI/bats/probe.bash <<'EOF'
probe() {
  @CURL@ --noproxy '*' --silent --output /dev/null --max-time 10 "http://127.0.0.1:${APP_PORT}/ping"
  @CURL@ --silent --fail http://localhost:8081/health
  @WGET@ -q -O /dev/null http://[::1]:8081/ping
  @CURL@ -fsS http://localhost:8080/health || exit 1
}
EOF
  check
  assert_success
  assert_output ''
}

@test "(b) a download from a host that only starts with localhost is refused" {
  put tests/Load/fetch.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ --fail --output /tmp/x http://localhost.example.org/x
EOF
  check
  assert_violation b 'tests/Load/fetch.sh: a file is downloaded by URL without a checksum'
}

@test "(b) a remote host behind a loopback user name is refused" {
  put scripts/fetch.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ --fail --output /tmp/x http://localhost@remote.example.org/x
EOF
  put scripts/fetch-port.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ --fail --output /tmp/x http://127.0.0.1:80@remote.example.org/x
EOF
  check
  assert_violation b 'scripts/fetch.sh: a file is downloaded by URL without a checksum'
  assert_line '(b) scripts/fetch-port.sh: a file is downloaded by URL without a checksum'
}

@test "(b) a download through a loopback proxy from an address in a variable is refused" {
  put scripts/fetch.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ --proxy http://127.0.0.1:3128 --fail --location --output tool.tgz "${TOOL_URL}"
EOF
  put scripts/fetch-comment.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ -fsSL -o tool.tgz "$TOOL_URL" # like http://localhost:1/
EOF
  check
  assert_violation b 'scripts/fetch.sh: a file is downloaded by URL without a checksum'
  assert_line '(b) scripts/fetch-comment.sh: a file is downloaded by URL without a checksum'
}

@test "(b) a download through a loopback proxy on a continued line or without a download option is refused" {
  put scripts/fetch-continued.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ --proxy http://127.0.0.1:3128 \
  -fsSL -o tool.tgz https://example.org/tool.tgz
EOF
  put scripts/fetch-piped.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ --proxy http://127.0.0.1:3128 "${TOOL_URL}" | tar -xz
EOF
  check
  assert_violation b 'scripts/fetch-continued.sh: a file is downloaded by URL without a checksum'
  assert_line '(b) scripts/fetch-piped.sh: a file is downloaded by URL without a checksum'
}

@test "(b) a probe and a download on one line are refused" {
  put scripts/fetch-and.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ -fsS http://localhost:8080/health && @CURL@ -fsSLO example.org/tool.tgz
EOF
  put scripts/fetch-wget.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ -fsS http://localhost:8080/health; @WGET@ -O tool.tgz example.org/tool.tgz
EOF
  put scripts/fetch-two.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ -fsSLo tool.tgz http://localhost:1/ example.org/tool.tgz
EOF
  put scripts/fetch-backtick.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ -fsS http://localhost:8080/health; @CURL@ -fsSLO `cat url.txt`
EOF
  put scripts/fetch-connect.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ --connect-to localhost:80:example.org:80 -fsSLO http://localhost/tool.tgz
EOF
  check
  assert_violation b 'scripts/fetch-and.sh: a file is downloaded by URL without a checksum'
  assert_line '(b) scripts/fetch-wget.sh: a file is downloaded by URL without a checksum'
  assert_line '(b) scripts/fetch-two.sh: a file is downloaded by URL without a checksum'
  assert_line '(b) scripts/fetch-backtick.sh: a file is downloaded by URL without a checksum'
  assert_line '(b) scripts/fetch-connect.sh: a file is downloaded by URL without a checksum'
}

@test "(b) a loopback URL beside a remote address without a scheme or a name override or proxy option is refused" {
  put scripts/fetch-resolve.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ --resolve localhost:80:93.184.216.34 -fsSLO http://localhost/tool.tgz
EOF
  put scripts/fetch-ipv4.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ -fsSLO http://localhost:1/ 93.184.216.34/tool
EOF
  put scripts/fetch-ipv4-bare.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ -fsSL http://localhost:1/ 93.184.216.34 >tool
EOF
  put scripts/fetch-word.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ -fsSLO http://localhost/x intranet/tool
EOF
  put scripts/fetch-socks.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ -x socks5h://127.0.0.1:9050 -fsSLO tool
EOF
  put scripts/fetch-resolve6.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ --resolve localhost:80:[2001:db8::1] -fsSLO http://localhost/tool
EOF
  put scripts/fetch-connect-word.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ --connect-to localhost:80:intranet:80 -fsSLO http://localhost/tool
EOF
  put scripts/fetch-preproxy.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ --preproxy socks5://127.0.0.1:1080 -fsSLO http://localhost/tool
EOF
  put scripts/fetch-socks-option.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ --socks5-hostname 127.0.0.1:9050 -fsSLO http://localhost/tool
EOF
  put scripts/fetch-proxy-cluster.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ -fsSLOx http://127.0.0.1:3128 http://localhost/tool
EOF
  put scripts/fetch-proxy-variable.sh <<'EOF'
#!/usr/bin/env bash
https_proxy=http://127.0.0.1:3128 @CURL@ -fsSLO http://localhost/tool
EOF
  put scripts/fetch-url-option.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ -fsSLO --url=intranet/tool http://localhost/
EOF
  put scripts/fetch-url-word.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ -fsSLO --url intranet http://localhost/
EOF
  put scripts/fetch-ipv6-bare.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ -fsSLO http://localhost/x '[2001:db8::1]/tool'
EOF
  put scripts/fetch-input-file.sh <<'EOF'
#!/usr/bin/env bash
@WGET@ -q --input-file=lists/tools http://localhost/
EOF
  check
  assert_failure
  local name
  for name in resolve ipv4 ipv4-bare word socks resolve6 connect-word preproxy socks-option proxy-cluster proxy-variable \
    url-option url-word ipv6-bare input-file; do
    assert_line "(b) scripts/fetch-${name}.sh: a file is downloaded by URL without a checksum"
  done
}

@test "(b) a probe in a command substitution is a probe, a download in one is not" {
  put tests/CLI/bats/probe.bash <<'EOF'
probe() {
  status=$(@CURL@ -s -o /dev/null -w '%{http_code}' http://localhost:8080/ping)
  body="$(@WGET@ -q -O - http://[::1]:8081/ping)"
}
EOF
  check
  assert_success
  assert_output ''
  put scripts/fetch-substitution.sh <<'EOF'
#!/usr/bin/env bash
out=$(@CURL@ -fsSLo tool http://localhost:1/ "$TOOL_URL")
EOF
  check
  assert_violation b 'scripts/fetch-substitution.sh: a file is downloaded by URL without a checksum'
}

@test "(b) a downloaded file with a checksum passes" {
  put scripts/fetch.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ --fail --location --output /tmp/tool.tar.gz https://example.org/tool.tar.gz
echo "expected  /tmp/tool.tar.gz" | sha256sum --check -
EOF
  check
  assert_success
}

@test "(b) a download in the Dockerfile without a checksum is refused although its images carry digests" {
  append Dockerfile 'RUN @CURL@ --fail --location --output /tmp/tool.tar.gz https://example.org/tool.tar.gz'
  check
  assert_violation b 'Dockerfile: a file is downloaded by URL without a checksum'
  append Dockerfile 'RUN echo "@D64@  /tmp/tool.tar.gz" | sha256sum --check -'
  check
  assert_success
  assert_output ''
}

@test "(b) a download in make/tool-versions.mk beside its SHA-256 pins is refused" {
  append make/tool-versions.mk $'fetch-tool:\n@T@@CURL@ --fail --location --output /tmp/tool.tar.gz https://example.org/tool.tar.gz'
  check
  assert_violation b 'make/tool-versions.mk: a file is downloaded by URL without a checksum'
}

@test "(b) ADD of a URL without --checksum is refused and with --checksum=sha256:<64 hex> passes" {
  append Dockerfile 'ADD https://example.org/tool.tar.gz /tmp/tool.tar.gz'
  check
  assert_violation b 'Dockerfile:12: ADD downloads https://example.org/tool.tar.gz without --checksum=$'
  replace Dockerfile 'ADD https://' 'ADD --checksum=sha256:abc https://'
  check
  assert_violation b 'Dockerfile:12: ADD downloads https://example.org/tool.tar.gz with --checksum=sha256:abc, which is not a sha256, sha384 or sha512 digest'
  replace Dockerfile 'ADD --checksum=sha256:abc https://' "ADD --checksum=sha256:${d64} https://"
  check
  assert_success
  assert_output ''
  # An instruction in lower case, continued on the next line, in the JSON form.
  append Dockerfile $'add --chown=1000:1000 \\\n  ["https://example.org/other.tar.gz", "/tmp/other.tar.gz"]'
  check
  assert_violation b 'Dockerfile:13: ADD downloads https://example.org/other.tar.gz without --checksum=$'
}

@test "(b) a second download without a checksum in a script that verifies the first is refused" {
  put scripts/fetch.sh <<'EOF'
#!/usr/bin/env bash
@CURL@ --fail --location --output /tmp/tool.tar.gz https://example.org/tool.tar.gz
echo "expected  /tmp/tool.tar.gz" | sha256sum --check -
@WGET@ --output-document /tmp/other.tar.gz https://example.org/other.tar.gz
EOF
  check
  assert_violation b 'scripts/fetch.sh: a file is downloaded by URL without a checksum'
}

@test "(b) each call of a function that runs a checksum command verifies one download, its body none" {
  put scripts/fetch.sh <<'EOF'
#!/usr/bin/env bash
sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d ' ' -f 1
  else
    shasum -a 256 "$1" | cut -d ' ' -f 1
  fi
}
@CURL@ --fail --location --output /tmp/tool.tar.gz https://example.org/tool.tar.gz
[ "$(sha256_of /tmp/tool.tar.gz)" = "${TOOL_SHA256}" ] || exit 1
EOF
  check
  assert_success
  assert_output ''
  append scripts/fetch.sh '@CURL@ --fail --location --output /tmp/other.tar.gz https://example.org/other.tar.gz'
  check
  assert_violation b 'scripts/fetch.sh: a file is downloaded by URL without a checksum'
  # A second call verifies the second download.
  append scripts/fetch.sh "test \"\$(sha256_of /tmp/other.tar.gz)\" = \"\${OTHER_SHA256}\""
  check
  assert_success
}

@test "(b) a checksum function that the file never calls verifies one download" {
  # A library function that a caller of the script uses counts once.
  put scripts/fetch.sh <<'EOF'
#!/usr/bin/env bash
sha256_of() {
  sha256sum "$1" | cut -d ' ' -f 1
}
@CURL@ --fail --location --output /tmp/tool.tar.gz https://example.org/tool.tar.gz
EOF
  check
  assert_success
  assert_output ''
  append scripts/fetch.sh '@WGET@ --output-document /tmp/other.tar.gz https://example.org/other.tar.gz'
  check
  assert_violation b 'scripts/fetch.sh: a file is downloaded by URL without a checksum'
}

@test "(b) a comment that mentions a checksum does not excuse a download" {
  put scripts/fetch.sh <<'EOF'
#!/usr/bin/env bash
# Download the archive; sha256sum --check verifies its checksum.
@CURL@ --fail --location --output /tmp/tool.tar.gz https://example.org/tool.tar.gz # then sha256sum --check
EOF
  check
  assert_violation b 'scripts/fetch.sh: a file is downloaded by URL without a checksum'
}

@test "(b) a message or a lookup that names a checksum command does not excuse a download" {
  put scripts/fetch.sh <<'EOF'
#!/usr/bin/env bash
command -v sha256sum >/dev/null || echo "install sha256sum to verify the checksum"
@CURL@ --fail --location --output /tmp/tool.tar.gz https://example.org/tool.tar.gz
EOF
  check
  assert_violation b 'scripts/fetch.sh: a file is downloaded by URL without a checksum'
}

# --- rule (c) ------------------------------------------------------------------------------------------------

@test "(c) rust-version that differs from rust-toolchain.toml and the builder tag is refused" {
  replace Cargo.toml 'rust-version = "1.99.1"' 'rust-version = "1.99.0"'
  check
  assert_violation c 'toolchain versions differ'
}

@test "(c) a channel that differs from the builder image tag is refused" {
  replace rust-toolchain.toml 'channel = "1.99.1"' 'channel = "1.99.2"'
  check
  assert_violation c 'toolchain versions differ: rust-toolchain.toml 1.99.2, builder image 1.99.1'
}

@test "(c) a builder image tag that differs from the toolchain is refused" {
  replace Dockerfile 'rust:1.99.1-slim-trixie' 'rust:1.98.0-slim-trixie'
  check
  assert_violation c 'builder image 1.98.0'
}

@test "(c) a channel that is not a full version is refused" {
  replace rust-toolchain.toml 'channel = "1.99.1"' 'channel = "stable"'
  check
  assert_violation c 'channel is missing or is not a full version'
}

# --- rule (d) ------------------------------------------------------------------------------------------------

@test "(d) a workflow set that omits a gate target of make ci-list is refused" {
  replace .github/workflows/quality.yml '      - run: make fmt' '      - run: echo skipped'
  check
  assert_violation d 'gate target fmt of make ci-list is run by no pull-request workflow'
}

@test "(d) a gate target that only a workflow without a pull_request trigger runs is refused" {
  replace .github/workflows/quality.yml '      - run: make fmt' '      - run: echo skipped'
  put .github/workflows/scheduled.yml <<'EOF'
name: Scheduled
on:
  schedule:
    - cron: '0 3 * * 1'
  workflow_dispatch:
permissions:
  contents: read
concurrency:
  group: scheduled
  cancel-in-progress: false
jobs:
  weekly:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make fmt
EOF
  check
  assert_violation d 'gate target fmt of make ci-list is run by no pull-request workflow'
}

@test "(d) a scheduled workflow that repeats a gate of a pull-request workflow or runs a CI-only target is accepted" {
  put .github/workflows/scheduled.yml <<'EOF'
name: Scheduled
on:
  schedule:
    - cron: '0 3 * * 1'
permissions:
  contents: read
concurrency:
  group: scheduled
  cancel-in-progress: false
jobs:
  weekly:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make fmt lint
      - run: make bootstrap && make ci
EOF
  check
  assert_success
  assert_output ''
}

@test "(d) a target outside make ci-list in a workflow without a pull_request trigger is refused" {
  put .github/workflows/scheduled.yml <<'EOF'
name: Scheduled
on:
  schedule:
    - cron: '0 3 * * 1'
permissions:
  contents: read
concurrency:
  group: scheduled
  cancel-in-progress: false
jobs:
  weekly:
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - run: make extra-gate
EOF
  check
  assert_violation d 'make extra-gate, which is not a gate target'
}

@test "(d) a workflow that runs a target outside make ci-list is refused" {
  replace .github/workflows/tests.yml '      - run: echo done' '      - run: make extra-gate'
  check
  assert_violation d 'make extra-gate, which is not a gate target'
}

@test "(d) a CI-only target in a workflow is not a finding" {
  replace .github/workflows/tests.yml '      - run: echo done' '      - run: make check-commit-message'
  check
  assert_success
}

@test "(d) a target outside make ci-list run through devcontainers exec is refused" {
  replace .github/workflows/tests.yml '      - run: echo done' \
    '      - run: npx --yes @devcontainers/cli@0.90.0 exec --workspace-folder . make extra-gate 2>&1 | tee x.log'
  check
  assert_violation d 'a workflow runs make extra-gate, which is not a gate target of make ci-list'
}

@test "(d) a target outside make ci-list run through docker compose exec is refused" {
  replace .github/workflows/tests.yml '      - run: echo done' '      - run: |
          docker compose exec -T app make extra-gate
          docker compose exec --user dev -e A=1 app make other-gate'
  check
  assert_violation d 'a workflow runs make extra-gate, which is not a gate target of make ci-list'
  assert_violation d 'a workflow runs make other-gate, which is not a gate target of make ci-list'
}

@test "(d) a target outside make ci-list after another make command on the same line is refused" {
  replace .github/workflows/tests.yml '      - run: echo done' '      - run: make fmt && make extra-gate'
  check
  assert_violation d 'a workflow runs make extra-gate, which is not a gate target of make ci-list'
}

@test "(d) format-check and start-deps run through devcontainers exec are accepted" {
  replace Makefile "@printf '%s\n' fmt lint bats" "@printf '%s\n' fmt format-check lint bats"
  replace .github/workflows/quality.yml '      - run: make fmt' '      - run: make fmt format-check'
  put .github/workflows/scheduled.yml <<'EOF'
name: Scheduled
on:
  schedule:
    - cron: '0 3 * * 1'
permissions:
  contents: read
concurrency:
  group: scheduled
  cancel-in-progress: false
jobs:
  devcontainer-build:
    runs-on: ubuntu-24.04
    timeout-minutes: 30
    steps:
      - run: |
          npx --yes @devcontainers/cli@0.90.0 exec --workspace-folder . make format-check 2>&1 | tee target/a.log
          npx --yes @devcontainers/cli@0.90.0 exec --workspace-folder . make start-deps 2>&1 | tee target/b.log
          docker compose exec -T app make fmt lint
EOF
  check
  assert_success
  assert_output ''
}

@test "(d) a quoted message that contains exec make is not a make command" {
  # The lines with a word after the target need the single quotes removed: without the word, the only word after
  # make ends in a quote and is dropped anyway.
  replace .github/workflows/tests.yml '      - run: echo done' "      - run: |
          target='devcontainers exec make extra-gate'
          echo 'make extra-gate failed'
          target='devcontainers exec make extra-gate now'
          echo 'cleanup done; make extra-gate twice'
          echo \"the step failed; make extra-gate ran \\\"exec make extra-gate\\\"\" >&2"
  check
  assert_success
  assert_output ''
}

@test "(d) make in a comment of a run line is not a make command" {
  replace .github/workflows/tests.yml '      - run: echo done' '      - run: make fmt # then make extra-gate later'
  check
  assert_success
  assert_output ''
}

@test "(d) a make command after an escaped quote outside quotes is refused" {
  replace .github/workflows/tests.yml '      - run: echo done' "      - run: echo it\\'s ready; make extra-gate"
  check
  assert_violation d 'a workflow runs make extra-gate, which is not a gate target of make ci-list'
}

@test "(d) a # inside a word does not start a comment of a run line" {
  replace .github/workflows/tests.yml '      - run: echo done' "      - run: echo \${#GROUP} groups; make extra-gate"
  check
  assert_violation d 'a workflow runs make extra-gate, which is not a gate target of make ci-list'
}

@test "(d) a target outside make ci-list run through exec with a NAME=value word is refused" {
  replace .github/workflows/tests.yml '      - run: echo done' '      - run: exec env A=1 make extra-gate'
  check
  assert_violation d 'a workflow runs make extra-gate, which is not a gate target of make ci-list'
}

@test "(d) make as a word of an echo or of a variable assignment is not a make command" {
  replace .github/workflows/tests.yml '      - run: echo done' '      - run: |
          echo the next step runs make extra-gate
          next=make extra-gate'
  check
  assert_success
  assert_output ''
}

@test "(d) a target outside make ci-list after if, elif, else, while, until, !, { or time is refused" {
  replace .github/workflows/tests.yml '      - run: echo done' '      - run: |
          if make extra-gate; then echo ok; fi
          if true; then echo a; elif make elif-gate; then echo b; fi
          if true; then echo a; else make else-gate; fi
          while make while-gate; do break; done
          until make until-gate; do break; done
          ! make bang-gate
          { make brace-gate; }
          time make time-gate
          if true; then make then-gate; fi
          for gate in a; do make do-gate; done'
  check
  local target
  for target in extra-gate elif-gate else-gate while-gate until-gate bang-gate brace-gate time-gate then-gate do-gate; do
    assert_violation d "a workflow runs make ${target}, which is not a gate target of make ci-list"
  done
}

@test "(d) make after a word that ends in then or do, or after a keyword that is an argument, is not a make command" {
  # A keyword starts a command only where a command starts: `todo` is a word of its own, and `do` or `if` after echo
  # are arguments of echo.
  replace .github/workflows/tests.yml '      - run: echo done' '      - run: |
          echo todo make extra-gate
          echo then make extra-gate
          echo please do make extra-gate
          echo if make extra-gate'
  check
  assert_success
  assert_output ''
}

@test "(d) a make command in a run value written as a quoted YAML scalar is refused" {
  replace .github/workflows/tests.yml '      - run: echo done' "      - run: \"make extra-gate\"
      - run: 'make other-gate'
      - run: \"echo \\\"ok\\\"; make third-gate\" # a comment
      - run: 'echo ''ok''; make fourth-gate'"
  check
  local target
  for target in extra-gate other-gate third-gate fourth-gate; do
    assert_violation d "a workflow runs make ${target}, which is not a gate target of make ci-list"
  done
}

@test "(d) a quoted run scalar whose shell text quotes make is not a make command" {
  # Without the shell quotes that the scalar holds, each `; make extra-gate` would start a command.
  replace .github/workflows/tests.yml '      - run: echo done' "      - run: \"echo 'make extra-gate'\"
      - run: \"echo 'done; make extra-gate'\"
      - run: \"echo \\\"done; make extra-gate\\\"\"
      - run: 'echo ''done; make extra-gate'''"
  check
  assert_success
  assert_output ''
}

@test "(d) make after an exec word with two operands or after a word ending in exec is not a make command" {
  # An exec word reaches make across option words and at most one operand (`this and` are two), and exec must be a
  # whole word (`my-exec` is another command).
  replace .github/workflows/tests.yml '      - run: echo done' '      - run: |
          echo please exec this and make extra-gate
          my-exec app make extra-gate'
  check
  assert_success
  assert_output ''
}

@test "(d) a tree without workflow files is refused" {
  rm -rf "${tree}/.github/workflows"
  check
  assert_violation d 'no workflow files'
}

@test "(d) a Bats file without a group tag is refused" {
  put_bats untagged.bats
  check
  assert_violation d 'untagged.bats: 0 file tags'
}

@test "(d) a Bats file with two file tags is refused" {
  put_bats double.bats surface quality
  check
  assert_violation d 'double.bats: 2 file tags'
}

@test "(d) a Bats file whose tag is not a group of the matrix is refused" {
  put_bats other.bats elsewhere
  check
  assert_violation d "other.bats: file tag 'elsewhere' is not a group"
}

@test "(d) a job name missing from docs/repository-settings.md is refused" {
  replace docs/repository-settings.md "| \`lint\` | Lint |" ''
  check
  assert_violation d 'quality.yml: job lint is missing from docs/repository-settings.md'
}

@test "(d) a pull-request workflow without the settings document is refused" {
  rm "${tree}/docs/repository-settings.md"
  check
  assert_violation d 'docs/repository-settings.md is missing'
}

@test "(d) a job listed only in the service-owned docs/repository-settings-service.md is accepted" {
  replace docs/repository-settings.md "| \`lint\` | Lint |" ''
  put docs/repository-settings-service.md <<'EOF'
| Job | Check name | Required |
| --- | --- | --- |
| `lint` | Lint | yes |
EOF
  check
  assert_success
}

@test "(d) a job listed in neither settings document is refused when the service document exists" {
  replace docs/repository-settings.md "| \`lint\` | Lint |" ''
  put docs/repository-settings-service.md <<'EOF'
| `other` | Other | yes |
EOF
  check
  assert_violation d 'quality.yml: job lint is missing from docs/repository-settings.md \(or docs/repository-settings-service.md\)'
}

# --- rule (e) ------------------------------------------------------------------------------------------------

@test "(e) the repository name in a script is refused" {
  append scripts/clone.sh "git clone https://example.org/org/${repo_name}"
  check
  assert_violation e 'scripts/clone.sh:1'
}

@test "(e) the repository name in a workflow other than the sync workflows is refused" {
  replace .github/workflows/tests.yml '      - run: echo done' "      - run: echo ${repo_name}"
  check
  assert_violation e '.github/workflows/tests.yml'
}

@test "(e) the repository name in a test of a crate, of xtask or of the features is refused" {
  append crates/a/tests/identity.rs "const NAME: &str = \"${repo_name}\";"
  append xtask/src/tests.rs "// ${repo_name}"
  append features/identity.feature "Scenario: ${repo_name}"
  check
  assert_violation e 'crates/a/tests/identity.rs:1'
  assert_line --regexp '^\(e\) xtask/src/tests\.rs:1'
  assert_line --regexp '^\(e\) features/identity\.feature:1'
}

@test "(e) the repository name in the Makefile and in AGENTS.md is refused" {
  append Makefile "NAME := ${repo_name}"
  append AGENTS.md "The ${repo_name} repository."
  check
  assert_violation e 'Makefile'
  assert_line --regexp '^\(e\) AGENTS.md'
}

@test "(e) the sync source setting of a sync workflow, the README and specs are allowed" {
  put .github/workflows/template-sync-app.yml <<'EOF'
name: Template sync
on:
  workflow_dispatch:
permissions:
  contents: read
concurrency:
  group: template-sync
  cancel-in-progress: false
env:
  TEMPLATE_REPOSITORY: VilnaCRM-Org/@NAME@
jobs:
  sync:
    runs-on: ubuntu-24.04
    timeout-minutes: 15
    permissions:
      contents: write
    steps:
      - run: echo sync
EOF
  append README.md "Badge: https://github.com/VilnaCRM-Org/${repo_name}"
  append specs/plan.md "The ${repo_name} plan."
  append bmalph/config.json "{ \"name\": \"${repo_name}\" }"
  check
  assert_success
  assert_output ''
}

@test "(e) the repository name in a sync workflow outside the source setting is refused" {
  put .github/workflows/template-sync-pat.yml <<'EOF'
name: Template sync
on:
  workflow_dispatch:
permissions:
  contents: read
concurrency:
  group: template-sync
  cancel-in-progress: false
env:
  TEMPLATE_REPOSITORY: VilnaCRM-Org/@NAME@
jobs:
  sync:
    runs-on: ubuntu-24.04
    timeout-minutes: 15
    steps:
      - run: echo @NAME@
EOF
  check
  assert_violation e 'template-sync-pat.yml:16'
  refute_line --regexp 'template-sync-pat.yml:10'
}

# --- rule (f) ------------------------------------------------------------------------------------------------

@test "(f) a second licence identifier in a crate manifest is refused" {
  put crates/b/Cargo.toml <<'EOF'
[package]
name = "b"
license = "CC0-1.0 OR MIT"
EOF
  check
  assert_violation f 'crates/b/Cargo.toml:3: licence identifier CC0-1.0 OR MIT'
}

@test "(f) another licence in the workspace manifest is refused" {
  replace Cargo.toml 'license = "CC0-1.0"' 'license = "MIT"'
  check
  assert_violation f 'Cargo.toml:.*licence identifier MIT'
}

@test "(f) another licence in tests/Load/package.json is refused" {
  replace tests/Load/package.json '"license": "CC0-1.0"' '"license": "ISC"'
  check
  assert_violation f 'tests/Load/package.json: licence identifier ISC'
}

@test "(f) another licence in the image label is refused" {
  replace Dockerfile 'licenses="CC0-1.0"' 'licenses="Apache-2.0"'
  check
  assert_violation f 'Dockerfile: image label.*Apache-2.0'
}

@test "(f) another licence in the OpenAPI snapshot is refused" {
  replace .github/openapi-spec/spec.yaml 'name: CC0-1.0' 'name: MIT'
  check
  assert_violation f 'spec.yaml: info.license states MIT'
}

@test "(f) another licence in the README licence section is refused" {
  replace README.md 'released under [CC0-1.0](LICENSE).' 'released under the MIT licence.'
  check
  assert_violation f 'README.md: the licence section names another licence'
}

@test "(f) a LICENSE file that is not CC0 is refused" {
  put LICENSE <<'EOF'
MIT License

Copyright (c) someone
EOF
  check
  assert_violation f 'LICENSE: the licence text is not CC0'
}

# --- rule (g) ------------------------------------------------------------------------------------------------

@test "(g) #[allow in a module is refused" {
  put crates/a/src/extra.rs <<'EOF'
#[allow(dead_code)]
fn unused_function() {}
EOF
  check
  assert_violation g 'crates/a/src/extra.rs:1: #\[allow'
}

@test "(g) #![allow in a crate root is refused" {
  replace crates/a/src/lib.rs '#![forbid(unsafe_code)]' '#![allow(dead_code)]'
  check
  assert_violation g 'crates/a/src/lib.rs:2: #\[allow or #!\[allow'
}

@test "(g) #![expect( in a crate root is refused" {
  replace crates/a/src/lib.rs '#![forbid(unsafe_code)]' $'#![expect(\n    dead_code,\n    reason = "fixture"\n)]'
  check
  assert_violation g 'crates/a/src/lib.rs:2: #!\[expect\( in a crate root'
}

@test "(g) #![expect( in the main file of an integration test is refused" {
  put crates/a/tests/integration/main.rs <<'EOF'
#![expect(dead_code, reason = "fixture")]
EOF
  check
  assert_violation g 'crates/a/tests/integration/main.rs:1: #!\[expect\( in a crate root'
}

@test "(g) #![expect( in the build script is refused" {
  put crates/a/build.rs <<'EOF'
#![expect(dead_code, reason = "fixture")]
fn main() {}
EOF
  check
  assert_violation g 'crates/a/build.rs:1: #!\[expect\( in a crate root'
}

@test "(g) a suppression that names clippy::pedantic is refused" {
  put crates/a/src/extra.rs <<'EOF'
#[expect(clippy::pedantic, reason = "fixture")]
fn extra() {}
EOF
  check
  assert_violation g 'crates/a/src/extra.rs:1: an expect names a lint group'
}

@test "(g) a suppression that names a rustc or rustdoc lint group is refused" {
  local group
  for group in deprecated_safe keyword_idents let_underscore refining_impl_trait rust_2018_compatibility \
    rust_2021_compatibility rust_2024_compatibility unknown_or_malformed_diagnostic_attributes rustdoc::all; do
    put crates/a/src/extra.rs <<EOF
#[expect(${group}, reason = "fixture")]
fn extra() {}
EOF
    echo "group ${group}"
    check
    assert_violation g 'crates/a/src/extra.rs:1: an expect names a lint group'
  done
}

@test "(g) #![expect( in a binary crate root src/bin/<name>.rs is refused" {
  put crates/a/src/bin/seed.rs <<'EOF'
#![expect(dead_code, reason = "fixture")]
fn main() {}
EOF
  check
  assert_violation g 'crates/a/src/bin/seed.rs:1: #!\[expect\( in a crate root'
}

@test "(g) #![expect( in an integration-test crate root tests/<name>.rs is refused" {
  put crates/a/tests/smoke.rs <<'EOF'
#![expect(dead_code, reason = "fixture")]
EOF
  check
  assert_violation g 'crates/a/tests/smoke.rs:1: #!\[expect\( in a crate root'
}

@test "(g) #![expect( in an example or bench crate root is refused" {
  put crates/a/examples/demo.rs <<'EOF'
#![expect(dead_code, reason = "fixture")]
fn main() {}
EOF
  put crates/a/benches/speed.rs <<'EOF'
#![expect(dead_code, reason = "fixture")]
fn main() {}
EOF
  check
  assert_violation g 'crates/a/examples/demo.rs:1: #!\[expect\( in a crate root'
  assert_violation g 'crates/a/benches/speed.rs:1: #!\[expect\( in a crate root'
}

@test "(g) #![expect( in a module file below tests/<dir>/ or src/<dir>/ is accepted" {
  put crates/a/tests/integration/support.rs <<'EOF'
#![expect(dead_code, reason = "fixture")]
EOF
  put crates/a/src/graphql/schema.rs <<'EOF'
#![expect(clippy::useless_let_if_seq, reason = "fixture")]
EOF
  put crates/a/src/bin/seed/helper.rs <<'EOF'
#![expect(dead_code, reason = "fixture")]
EOF
  # A module directory named tests below src/ is no tests/ directory of the package.
  put crates/a/src/aggregate/tests/cases.rs <<'EOF'
#![expect(dead_code, reason = "fixture")]
EOF
  check
  assert_success
  assert_output ''
}

# Writes a file of the tree whose first line is an inner expect of one lint with a reason.
put_expect() {
  printf '%s\n' '#![expect(dead_code, reason = "fixture")]' | put "$1"
}

@test "(g) #![expect( in a library root named by [lib] path is refused" {
  append crates/a/Cargo.toml $'\n[lib]\npath = "src/core.rs"'
  put_expect crates/a/src/core.rs
  # Cargo compiles the file a manifest names whatever its name ends in and wherever it stands.
  put crates/b/Cargo.toml <<'EOF'
[package]
name = "b"

[lib]
path = "lib.txt"
EOF
  put_expect crates/b/lib.txt
  put crates/c/Cargo.toml <<'EOF'
[package]
name = "c"

[lib]
path = "./../shared/../common/root.rs"
EOF
  put_expect crates/common/root.rs
  check
  assert_violation g 'crates/a/src/core.rs:1: #!\[expect\( in a crate root'
  assert_violation g 'crates/b/lib.txt:1: #!\[expect\( in a crate root'
  assert_violation g 'crates/common/root.rs:1: #!\[expect\( in a crate root'
}

@test "(g) #![expect( in a binary root named by [[bin]] path is refused" {
  append crates/a/Cargo.toml '
[[bin]]
name = "seed"
path = "tools/seed.rs"

[[bin]]
name = "load"
path = "src/cli/load.rs"'
  put_expect crates/a/tools/seed.rs
  put_expect crates/a/src/cli/load.rs
  check
  assert_violation g 'crates/a/tools/seed.rs:1: #!\[expect\( in a crate root'
  assert_violation g 'crates/a/src/cli/load.rs:1: #!\[expect\( in a crate root'
}

@test "(g) #![expect( in a test, example or bench root named by its path is refused" {
  append crates/a/Cargo.toml '
[[test]]
name = "smoke"
path = "checks/smoke.rs"

[[example]]
name = "demo"
path = "samples/demo.rs"

[[bench]]
name = "speed"
path = "perf/speed.rs"'
  put_expect crates/a/checks/smoke.rs
  put_expect crates/a/samples/demo.rs
  put_expect crates/a/perf/speed.rs
  check
  assert_violation g 'crates/a/checks/smoke.rs:1: #!\[expect\( in a crate root'
  assert_violation g 'crates/a/samples/demo.rs:1: #!\[expect\( in a crate root'
  assert_violation g 'crates/a/perf/speed.rs:1: #!\[expect\( in a crate root'
}

@test "(g) #![expect( in a build script named by build = is refused" {
  replace crates/a/Cargo.toml 'name = "a"' $'name = "a"\nbuild = "tools/gen.rs"'
  put_expect crates/a/tools/gen.rs
  # The legacy table [project], which cargo still reads below edition 2024.
  put crates/old/Cargo.toml <<'EOF'
[project]
name = "old"
edition = "2021"
build = "codegen.rs"
EOF
  put_expect crates/old/codegen.rs
  check
  assert_violation g 'crates/a/tools/gen.rs:1: #!\[expect\( in a crate root'
  assert_violation g 'crates/old/codegen.rs:1: #!\[expect\( in a crate root'
}

@test "(g) #![expect( in a module file next to a path-named root is accepted" {
  replace crates/a/Cargo.toml 'name = "a"' $'name = "a"\nbuild = "tools/gen.rs"'
  append crates/a/Cargo.toml '
[lib]
path = "src/core.rs"

[[bin]]
name = "seed"
path = "tools/seed.rs"'
  put crates/a/src/core.rs <<<'#![forbid(unsafe_code)]'
  put crates/a/tools/seed.rs <<<'fn main() {}'
  put crates/a/tools/gen.rs <<<'fn main() {}'
  put_expect crates/a/src/helper.rs
  put_expect crates/a/src/core/inner.rs
  put_expect crates/a/tools/util.rs
  put_expect crates/a/tools/gen/support.rs
  check
  assert_success
  assert_output ''
}

@test "(g) a crate root named in another TOML form of its key is refused" {
  # No white space around =; white space and a tab around =, a literal string, comments after header and value.
  put crates/f1/Cargo.toml <<'EOF'
[lib]
path="src/core.rs"
EOF
  printf '[ lib ]   # the library\npath \t =   %s   # not "src/other.rs"\n' "'src/core.rs'" | put crates/f2/Cargo.toml
  # A quoted key and a # inside the string.
  put crates/f3/Cargo.toml <<'EOF'
[lib]
"path" = "src/c#re.rs" # the root
EOF
  # A dotted key before the first table and a multi-line literal string.
  put crates/f4/Cargo.toml <<'EOF'
lib.path = '''src/core.rs'''

[package]
name = "f4"
EOF
  # An inline table and an escape in a basic string: "src/", a backslash, "u0063ore.rs" (the escape of c).
  printf 'lib = { name = "f5", path = "src/\\%s0063ore.rs" }\n' u | put crates/f5/Cargo.toml
  # An array of inline tables over several lines, the second one broken over lines with a trailing comma.
  put crates/f6/Cargo.toml <<'EOF'
bin = [
  { name = "first", path = "src/core.rs" }, # the first binary
  {
    name = "second",
    path = 'src/other.rs',
  },
]
EOF
  # White space inside the header of an array of tables.
  put crates/f7/Cargo.toml <<'EOF'
[[ bin ]]
name = "seed"
path = "src/core.rs"
EOF
  local file
  local -a roots=(f1/src/core.rs f2/src/core.rs 'f3/src/c#re.rs' f4/src/core.rs f5/src/core.rs f6/src/core.rs
    f6/src/other.rs f7/src/core.rs)
  for file in "${roots[@]}"; do
    put_expect "crates/${file}"
  done
  check
  for file in "${roots[@]}"; do
    echo "root ${file}"
    assert_violation g "crates/${file}:1: #!\\[expect\\( in a crate root"
  done
}

@test "(g) a path of a dependency table and a key in a string, a comment or another table name no crate root" {
  put crates/a/Cargo.toml <<'EOF'
[package]
name = "a"
version.workspace = true
license.workspace = true
build = false
description = 'not a build script: build = "src/text.rs"'

[package.metadata.docs]
build = "src/docs.rs"

[lib]
# path = "src/comment.rs"
name = "a" # not path = "src/note.rs"

[dependencies]
other = { version = "1", path = "src/other.rs" }
lib = { path = "src/libdep.rs" }

[dependencies.helper]
path = "src/helper.rs"

[target.'cfg(unix)'.dependencies.sys]
path = "src/sys.rs"

[dev-dependencies]
fixture = { path = "src/fixture.rs" }

[build-dependencies.gen]
path = "src/gen.rs"
EOF
  append Cargo.toml '
[workspace.dependencies]
shared = { path = "crates/a/src/shared.rs" }

[patch.crates-io]
serde = { path = "crates/a/src/patch.rs" }'
  # A comment inside an array and inside an inline table over several lines.
  put crates/d/Cargo.toml <<'EOF'
bin = [
  # "x", { path = "src/aside.rs" },
  { name = "d", path = "src/main.rs" },
]
lib = {
  # path = "src/aside.rs",
  name = "d",
}
EOF
  put_expect crates/d/src/aside.rs
  local file
  for file in text docs comment note other libdep helper sys fixture gen shared patch; do
    put_expect "crates/a/src/${file}.rs"
  done
  # build = false names nothing, not a file named false.
  put_expect crates/a/false
  check
  assert_success
  assert_output ''
}

@test "(g) a suppression that names a lint group on a later line is refused" {
  put crates/a/src/extra.rs <<'EOF'
#[expect(
    dead_code,
    unused,
    reason = "fixture"
)]
fn extra() {}
EOF
  check
  assert_violation g 'crates/a/src/extra.rs:1: an expect names a lint group'
}

@test "(g) a suppression of one lint with a reason that names a group passes" {
  put crates/a/src/extra.rs <<'EOF'
// A line comment with #[allow(unused)] is not an attribute.
#[expect(clippy::unwrap_used, reason = "clippy::pedantic and unused are only words here")]
fn extra() {}
EOF
  check
  assert_success
  assert_output ''
}

@test "(g) #![cfg_attr(test, expect( in a crate root is refused" {
  replace crates/a/src/lib.rs '#![forbid(unsafe_code)]' '#![cfg_attr(test, expect(clippy::unwrap_used, reason = "fixture"))]'
  check
  assert_violation g 'crates/a/src/lib.rs:2: #!\[expect\( in a crate root'
}

@test "(g) a cfg_attr suppression that names a lint group is refused" {
  put crates/a/src/extra.rs <<'EOF'
#[cfg_attr(
    test,
    expect(clippy::pedantic, reason = "fixture")
)]
fn extra() {}
EOF
  check
  assert_violation g 'crates/a/src/extra.rs:1: an expect names a lint group'
}

@test "(g) #[cfg_attr(.., allow( is refused" {
  put crates/a/src/extra.rs <<'EOF'
#[cfg_attr(not(test), allow(dead_code))]
fn extra() {}
EOF
  check
  assert_violation g 'crates/a/src/extra.rs:1: #\[allow or #!\[allow'
}

@test "(g) an item-level cfg_attr expect of one lint with a reason passes" {
  put crates/a/src/extra.rs <<'EOF'
#[cfg_attr(
    not(test),
    expect(
        clippy::needless_pass_by_value,
        reason = "the signature is fixed by the layer; unused and clippy::pedantic are words here"
    )
)]
fn extra(_value: String) {}

#[cfg_attr(test, derive(Debug))]
struct Plain;
EOF
  check
  assert_success
  assert_output ''
}

# --- rule (g): inline directives of the linters ----------------------------------------------------------------

# Asserts status 1 and exactly the findings given on standard input, one per line, in the order of the script.
assert_findings() {
  local expected
  expected="$(cat)"
  assert_failure 1
  assert_output "${expected}"
}

@test "(g) a shellcheck directive without a reason is refused" {
  put scripts/lint.sh <<'EOF'
#!/usr/bin/env bash
# shellcheck disable=SC2034
unused=1
words="a b"
# shellcheck disable=SC2086
echo $words
# shellcheck disable=SC2086 #
echo $words
echo $words # shellcheck disable=SC2086
cat <<TEXT
body
TEXT
# shellcheck disable=SC2086
echo $words
cat <<-TEXT
@T@body
@T@TEXT
# shellcheck disable=SC2086
echo $words
EOF
  put scripts/lib.bash <<'EOF'
#shellcheck disable=SC2148
true
EOF
  put .githooks/pre-commit <<'EOF'
#!/usr/bin/env bash
	# shellcheck disable=SC2317
true
EOF
  put tests/other/sample.bats <<'EOF'
setup() {
  # shellcheck disable=SC2030
  true
}
EOF
  replace .github/workflows/quality.yml '          make lint' $'          # shellcheck disable=SC2086\n          make lint\n          cat <<TEXT\n          body\n          TEXT\n          # shellcheck disable=SC2086\n          echo done'
  # A block scalar with an indentation indicator (content at column 8 + 2) and one with a tag and an anchor.
  append .github/workflows/quality.yml '      - run: |2
            cat <<TEXT
          body
          TEXT
          # shellcheck disable=SC2086
          echo done
      - run: !!str &script |
            cat <<TEXT
            body
            TEXT
            # shellcheck disable=SC2086
            echo done
      - run: *script'
  check
  assert_findings <<'EOF'
(g) .githooks/pre-commit:2: a shellcheck directive carries no reason
(g) .github/workflows/quality.yml:29: a shellcheck directive carries no reason
(g) .github/workflows/quality.yml:34: a shellcheck directive carries no reason
(g) .github/workflows/quality.yml:40: a shellcheck directive carries no reason
(g) .github/workflows/quality.yml:46: a shellcheck directive carries no reason
(g) scripts/lib.bash:1: a shellcheck directive carries no reason
(g) scripts/lint.sh:2: a shellcheck directive carries no reason
(g) scripts/lint.sh:5: a shellcheck directive carries no reason
(g) scripts/lint.sh:7: a shellcheck directive carries no reason
(g) scripts/lint.sh:9: a shellcheck directive carries no reason
(g) scripts/lint.sh:13: a shellcheck directive carries no reason
(g) scripts/lint.sh:18: a shellcheck directive carries no reason
(g) tests/other/sample.bats:2: a shellcheck directive carries no reason
EOF
}

@test "(g) a shellcheck directive of all or of two rules is refused" {
  put scripts/lint.sh <<'EOF'
#!/usr/bin/env bash
# The script is a fixture.
# shellcheck disable=all
true
# shellcheck disable=ALL # any letter case
true
# shellcheck disable=SC2086,SC2016 # two rules in a list
true
# shellcheck disable=SC2086 disable=SC2016 # two disable keys
true
# shellcheck disable=SC2000-SC2099 # a range of rules
true
EOF
  check
  assert_findings <<'EOF'
(g) scripts/lint.sh:3: a shellcheck directive names no single rule
(g) scripts/lint.sh:5: a shellcheck directive names no single rule
(g) scripts/lint.sh:7: a shellcheck directive names more than one rule
(g) scripts/lint.sh:9: a shellcheck directive names more than one rule
(g) scripts/lint.sh:11: a shellcheck directive names more than one rule
EOF
}

@test "(g) a shellcheck directive of one rule with a reason on its line or on the comment line above passes" {
  put scripts/lint.sh <<'EOF'
#!/usr/bin/env bash
# shellcheck disable=SC2034 # the variable is read by the caller, who sources this file
unused=1
words="a b"
# The words are split on purpose: the list holds flags.
# shellcheck disable=SC2086
echo $words
if true; then
  # shellcheck disable=SC2086 # a # in the reason is text
  echo $words
fi
EOF
  replace .github/workflows/quality.yml '          make lint' \
    $'          # The flags are split on purpose.\n          # shellcheck disable=SC2086\n          make lint'
  check
  assert_success
  assert_output ''
}

@test "(g) a shellcheck source= or shell= directive is not read" {
  put scripts/lint.sh <<'EOF'
#!/usr/bin/env bash
# shellcheck shell=bash
# shellcheck source=/dev/null
. /dev/null
# shellcheck source=scripts/hello.sh disable=SC1091 # the file is generated
. scripts/hello.sh
# shellcheck enable=require-variable-braces
true
EOF
  check
  assert_success
  assert_output ''
}

@test "(g) a hadolint ignore without a reason is refused" {
  replace Dockerfile 'RUN cargo build --release' $'# hadolint ignore=DL3059\nRUN cargo build --release'
  replace Dockerfile 'WORKDIR /app' $'WORKDIR /app\n# The base image pins its packages.\n# hadolint global ignore=DL3008'
  replace Dockerfile 'FROM gcr.io' $'# hadolint ignore=DL3006 # the stage is never pushed\nFROM gcr.io'
  check
  assert_findings <<'EOF'
(g) Dockerfile:10: a hadolint directive carries no reason
EOF
}

@test "(g) a hadolint ignore of all or of two rules is refused" {
  replace Dockerfile 'WORKDIR /app' $'WORKDIR /app\n# Fixture lines.\n# hadolint ignore=DL3008,DL3009\n# hadolint ignore=DL3008 DL3009 # two words\n# hadolint ignore=all # all rules'
  check
  assert_findings <<'EOF'
(g) Dockerfile:5: a hadolint directive names more than one rule
(g) Dockerfile:6: a hadolint directive names more than one rule
(g) Dockerfile:7: a hadolint directive names no single rule
EOF
}

@test "(g) a yamllint disable that names no single rule is refused" {
  put config/settings.yml <<'EOF'
# yamllint disable-file
# The truthy rule reads on: as a boolean.
# yamllint disable
first: yes
# The truthy and comments rules do not apply here.
# yamllint disable rule:truthy rule:comments
second: yes
# A rule is named as rule:<id>.
third: yes  # yamllint disable-line truthy
EOF
  check
  assert_findings <<'EOF'
(g) config/settings.yml:1: a yamllint directive names no single rule
(g) config/settings.yml:1: a yamllint directive carries no reason
(g) config/settings.yml:3: a yamllint directive names no single rule
(g) config/settings.yml:6: a yamllint directive names more than one rule
(g) config/settings.yml:9: a yamllint directive names no single rule
EOF
}

@test "(g) a yamllint directive of one rule needs the comment line above as its reason" {
  put config/settings.yml <<'EOF'
first: yes
# yamllint disable rule:truthy
second: yes
# The value is read as a string by the tool.
# yamllint disable-line rule:truthy
third: yes
fourth: yes  # yamllint disable-line rule:truthy
# yamllint enable
EOF
  check
  assert_findings <<'EOF'
(g) config/settings.yml:2: a yamllint directive carries no reason
(g) config/settings.yml:7: a yamllint directive carries no reason
EOF
}

@test "(g) a zizmor ignore without a reason or of two rules is refused" {
  replace .github/workflows/quality.yml "actions/checkout@${d40} # v7.0.1" \
    "actions/checkout@${d40} # v7.0.1 # zizmor: ignore[artipacked]"
  replace .github/workflows/tests.yml "actions/checkout@${d40} # v7.0.1" \
    "actions/checkout@${d40} # v7.0.1 # zizmor: ignore[artipacked,cache-poisoning] the job keeps its token"
  check
  assert_findings <<'EOF'
(g) .github/workflows/quality.yml:15: a zizmor directive carries no reason
(g) .github/workflows/tests.yml:20: a zizmor directive names more than one rule
EOF
}

@test "(g) a zizmor ignore of one rule with a reason on its line or above passes" {
  replace .github/workflows/quality.yml "actions/checkout@${d40} # v7.0.1" \
    "actions/checkout@${d40} # v7.0.1 # zizmor: ignore[artipacked] the job pushes with this token"
  replace .github/workflows/tests.yml "      - uses: actions/checkout@${d40} # v7.0.1" \
    $'      # The job pushes with the credentials of this checkout.\n      - uses: actions/checkout@'"${d40}"' # zizmor: ignore[artipacked]'
  check
  assert_success
  assert_output ''
}

@test "(g) a markdownlint-disable that names no rule is refused" {
  put docs/guide.md <<'EOF'
# Guide

<!-- markdownlint-disable -->
<!-- The headings rules do not apply to this page. -->
<!-- markdownlint-disable headings -->
<!-- The table and the banner are wide. -->
<!-- markdownlint-disable MD013 MD033 -->
<!-- markdownlint-disable-file -->
Text <!-- markdownlint-disable-line --> here.
<!-- markdownlint-configure-file { "default": false } -->
<!-- MARKDOWNLINT-DISABLE-NEXT-LINE -->
EOF
  check
  assert_findings <<'EOF'
(g) docs/guide.md:3: a markdownlint directive names no single rule
(g) docs/guide.md:3: a markdownlint directive carries no reason
(g) docs/guide.md:5: a markdownlint directive names no single rule
(g) docs/guide.md:7: a markdownlint directive names more than one rule
(g) docs/guide.md:8: a markdownlint directive names no single rule
(g) docs/guide.md:8: a markdownlint directive carries no reason
(g) docs/guide.md:9: a markdownlint directive names no single rule
(g) docs/guide.md:9: a markdownlint directive carries no reason
(g) docs/guide.md:10: a markdownlint directive names no single rule
(g) docs/guide.md:10: a markdownlint directive carries no reason
(g) docs/guide.md:11: a markdownlint directive names no single rule
(g) docs/guide.md:11: a markdownlint directive carries no reason
EOF
}

@test "(g) a markdownlint directive of one rule needs the comment line above, also inside a code span or fence" {
  put docs/guide.md <<'EOF'
# Guide

<!-- The table is wider than a line. -->
<!-- markdownlint-disable-next-line MD013 -->
<!-- The banner is HTML. -->
<!-- markdownlint-disable no-inline-html -->
<!-- markdownlint-disable-next-line MD013 -->
Write `<!-- markdownlint-disable-line MD013 -->` to suppress a rule on one line.

```markdown
<!-- markdownlint-disable MD001 -->
```

<!-- markdownlint-configure-file
{ "MD013": false, "MD033": { "allowed_elements": ["a"] } }
-->
EOF
  check
  assert_findings <<'EOF'
(g) docs/guide.md:7: a markdownlint directive carries no reason
(g) docs/guide.md:8: a markdownlint directive carries no reason
(g) docs/guide.md:11: a markdownlint directive carries no reason
(g) docs/guide.md:14: a markdownlint directive names more than one rule
(g) docs/guide.md:14: a markdownlint directive carries no reason
EOF
}

@test "(g) an eslint-disable-next-line without a rule or without -- reason is refused" {
  put tests/Load/scripts/smoke.js <<'EOF'
// eslint-disable-next-line
console.log(1);
// eslint-disable-next-line no-console
console.log(2);
/* eslint-disable-next-line no-console -- */
console.log(3);
/* eslint-disable no-console, eqeqeq -- two rules in a list */
console.log(4); // eslint-disable-line
/* eslint-disable */
/* eslint-disable-next-line -- the reason is split off first, so no rule is named */
console.log(5);
/* global foo */
// eslint-disable-next-line no-console
console.log(foo);
/* globals foo, bar */
// eslint-disable-next-line no-console
console.log(bar);
/* exported smoke */
// eslint-disable-next-line no-console
console.log(6);
EOF
  check
  assert_findings <<'EOF'
(g) tests/Load/scripts/smoke.js:1: an ESLint directive names no single rule
(g) tests/Load/scripts/smoke.js:1: an ESLint directive carries no reason
(g) tests/Load/scripts/smoke.js:3: an ESLint directive carries no reason
(g) tests/Load/scripts/smoke.js:5: an ESLint directive carries no reason
(g) tests/Load/scripts/smoke.js:7: an ESLint directive names more than one rule
(g) tests/Load/scripts/smoke.js:8: an ESLint directive names no single rule
(g) tests/Load/scripts/smoke.js:8: an ESLint directive carries no reason
(g) tests/Load/scripts/smoke.js:9: an ESLint directive names no single rule
(g) tests/Load/scripts/smoke.js:9: an ESLint directive carries no reason
(g) tests/Load/scripts/smoke.js:10: an ESLint directive names no single rule
(g) tests/Load/scripts/smoke.js:13: an ESLint directive carries no reason
(g) tests/Load/scripts/smoke.js:16: an ESLint directive carries no reason
(g) tests/Load/scripts/smoke.js:19: an ESLint directive carries no reason
EOF
}

@test "(g) an ESLint configuration comment without a reason or of two rules is refused" {
  put tests/Load/scripts/smoke.js <<'EOF'
/* eslint no-console: "off" */
console.log(1);
/* eslint no-console: "off", eqeqeq: ["warn", "smart"] -- the summary is printed */
console.log(2);
EOF
  check
  assert_findings <<'EOF'
(g) tests/Load/scripts/smoke.js:1: an ESLint directive carries no reason
(g) tests/Load/scripts/smoke.js:3: an ESLint directive names more than one rule
EOF
}

@test "(g) an eslint-disable-next-line of one rule with -- reason passes" {
  put tests/Load/scripts/smoke.js <<'EOF'
// eslint-disable-next-line no-console -- the smoke run prints its summary, eqeqeq is a word here
console.log(1);
// The summary is printed once.
// eslint-disable-next-line no-console
console.log(2);
/* eslint-disable-next-line eqeqeq --
   the loose comparison is the subject of the check */
const loose = 1 == '1';
const text = "// eslint-disable-next-line";
const quoted = '/* eslint-disable */';
const template = `
// eslint-disable-line
${text} ${quoted} ${loose}`;
const pattern = /["'`]/; // eslint-disable-line no-unused-vars -- the pattern documents the quotes
/* eslint no-console: ["warn"] -- the smoke run prints its summary */
// globals of the k6 runtime are printed: a line comment is no ESLint configuration comment
// eslint-disable-next-line no-console
console.log(3);
/* eslint-env node */
// eslint-disable no-console
// eslint no-console: off
// eslint-enable no-console
EOF
  check
  assert_success
  assert_output ''
}

@test "(g) directive words in strings, here-documents, block scalars and code spans are not read" {
  put scripts/lint.sh <<'EOF'
#!/usr/bin/env bash
echo "# shellcheck disable=all" '# shellcheck disable=all' $'# shellcheck disable=all'
cat <<'TEXT'
# shellcheck disable=all
TEXT
@T@cat <<-TEXT
@T@# shellcheck disable=all
@T@TEXT
text='first line
# shellcheck disable=all
last line'
count=$(( 1 << 2 ))
size="${#text} $# ${text#first}"
output=$(printf '%s' "it's")
echo "${count} ${size} ${output}" `echo '# shellcheck disable=all'`
EOF
  append .github/workflows/quality.yml '      - name: "# yamllint disable"
        uses: actions/github-script@@D40@ # v8.0.0
        with:
          script: |
            // eslint-disable-next-line
            # zizmor: ignore[artipacked]
            # shellcheck disable=all
      - run: make lint # shellcheck disable=all
      - run: |
          cat <<TEXT
            TEXT
          # shellcheck disable=all
          TEXT'
  put config/settings.yml <<'EOF'
message: "# yamllint disable"
other: '# yamllint disable'
url: https://example.org/#yamllint-disable
# zizmor: ignore[artipacked] is read in workflows only
script: |
  # yamllint disable
folded: >-
  # yamllint disable-line
list:
  - |
    # yamllint disable
  - &text |
    # yamllint disable
EOF
  append Dockerfile 'RUN echo "# hadolint ignore=DL3008"
# hadolint shell=bash'
  put docs/guide.md <<'EOF'
# Guide

Write `markdownlint-disable MD013` in an HTML comment to suppress a rule.

```text
markdownlint-disable-next-line
```

<!-- A comment about markdownlint-disable is no directive. -->
EOF
  check
  assert_success
  assert_output ''
}

@test "(g) the directives below specs/, _bmad*/, tests/CLI/bats/fixtures/, node_modules/ and target/ are not read" {
  local dir
  for dir in specs _bmad _bmad-output tests/CLI/bats/fixtures tests/Load/node_modules target crates/a/target; do
    printf '%s\n' '# shellcheck disable=all' | put "${dir}/lint.sh"
  done
  printf '%s\n' '/* eslint-disable */' | put tests/Load/node_modules/pkg/index.js
  printf '%s\n' '<!-- markdownlint-disable -->' | put specs/guide.md
  check
  assert_success
  assert_output ''
}

@test "(g) in a git work tree the directives of tracked and new files are read, those of ignored files are not" {
  printf '%s\n' ignored.sh build/ >"${tree}/.gitignore"
  printf '%s\n' '# shellcheck disable=all' | put ignored.sh
  printf '%s\n' '# shellcheck disable=all' | put build/lint.sh
  printf '%s\n' '# shellcheck disable=SC2086' | put scripts/new.sh
  append scripts/hello.sh '# shellcheck disable=SC2034'
  git -C "${tree}" init -q
  git -C "${tree}" add scripts/hello.sh
  check
  assert_findings <<'EOF'
(g) scripts/hello.sh:3: a shellcheck directive carries no reason
(g) scripts/new.sh:1: a shellcheck directive carries no reason
EOF
}

# --- rule (h) ------------------------------------------------------------------------------------------------

@test "(h) a recipe that calls cargo without the execution prefix is refused" {
  replace Makefile "\$(EXEC) cargo clippy --locked" 'cargo clippy --locked'
  check
  assert_violation h 'Makefile:.*the recipe of lint starts cargo without'
}

@test "(h) a recipe that starts a toolchain command through env without the prefix is refused" {
  append Makefile $'doc:\n@T@env RUSTDOCFLAGS="-D warnings" cargo doc'
  check
  assert_violation h 'the recipe of doc starts cargo'
}

@test "(h) a recipe that starts a command variable of a toolchain command without the prefix is refused" {
  replace Makefile "\$(EXEC) \$(FMT_CMD)" "\$(FMT_CMD)"
  check
  assert_violation h 'the recipe of fmt starts \$\(FMT_CMD\) \(cargo\) without'
}

@test "(h) a recipe that starts typos, taplo, zizmor or sqlx without the prefix is refused" {
  append Makefile $'spelling:\n@T@@typos\n@T@taplo fmt --check\n@T@-zizmor .github\n@T@sqlx migrate run'
  check
  assert_violation h 'starts typos'
  assert_line --regexp '^\(h\) .*starts taplo'
  assert_line --regexp '^\(h\) .*starts zizmor'
  assert_line --regexp '^\(h\) .*starts sqlx'
}

@test "(h) a script that a recipe calls outside the prefix and that starts cargo is refused" {
  put scripts/hello.sh <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
cargo build
EOF
  check
  assert_violation h 'scripts/hello.sh:3: called outside \$\(EXEC\) from Makefile:.*starts a toolchain command'
}

@test "(h) a script on the right of a pipe that starts cargo is refused" {
  put scripts/pipe.sh <<'EOF'
#!/usr/bin/env bash
if cargo metadata; then cat; fi
EOF
  check
  assert_violation h 'scripts/pipe.sh:2: called outside'
}

@test "(h) a script behind the prefix may start cargo, and so may the installer and the preflight" {
  append Makefile "migrate:
@T@\$(EXEC) scripts/setup.sh"
  put scripts/setup.sh <<'EOF'
#!/usr/bin/env bash
sqlx database reset
EOF
  put scripts/doctor.sh <<'EOF'
#!/usr/bin/env bash
cargo --version
EOF
  append Makefile $'doctor:\n@T@@scripts/doctor.sh'
  check
  assert_success
  assert_output ''
}

@test "(h) a toolchain word in text or in a comment of a script is not a command" {
  put scripts/hello.sh <<'EOF'
#!/usr/bin/env bash
# cargo is started by the caller
echo "run cargo and typos behind the switch"
EOF
  check
  assert_success
  assert_output ''
}
