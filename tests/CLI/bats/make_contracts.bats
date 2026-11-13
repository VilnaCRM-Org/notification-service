#!/usr/bin/env bats
# Tests of the contract targets (architecture-delivery.md "CLI Tests (Bats)", file make_contracts.bats): the
# snapshot generators, the two diff targets, Schemathesis, and the negative tests of gates G17 to G21.
#
# Every variant of a snapshot is derived from the committed file with awk or sed, so it cannot go stale, and
# is planted with the harness; teardown unstages the snapshots and restores them exactly. Make reports every
# failed recipe with its own status 2, so tests that need a script's own status call the script directly.

bats_require_minimum_version 1.8.0
load test_helper

# bats file_tags=contracts

OPENAPI_SNAPSHOT=.github/openapi-spec/spec.yaml
GRAPHQL_SNAPSHOT=.github/graphql-spec/spec

setup() {
  _common_setup
}

teardown() {
  restore_planted
  git -C "${REPO_ROOT}" restore --staged -- "${OPENAPI_SNAPSHOT}" "${GRAPHQL_SNAPSHOT}" >/dev/null 2>&1 || true
  # Every test leaves both snapshots committed, unstaged and unchanged.
  [[ -z "$(git -C "${REPO_ROOT}" status --porcelain -- .github/openapi-spec .github/graphql-spec)" ]]
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

# The OpenAPI snapshot without the description of its first operation.
openapi_without_operation_description() {
  awk '!done && /^      description: / { done = 1; next } { print }' "${OPENAPI_SNAPSHOT}"
}

# The OpenAPI snapshot without the required property `title` of the schema `Problem`.
openapi_without_required_response_property() {
  awk '
    /^    Problem:$/ { in_schema = 1 }
    in_schema && /^    [A-Za-z]/ && !/^    Problem:$/ { in_schema = 0 }
    in_schema && /^      - title$/ { next }
    in_schema && /^        title:$/ { skipping = 1; next }
    skipping && /^          / { next }
    { skipping = 0; print }
  ' "${OPENAPI_SNAPSHOT}"
}

# The OpenAPI snapshot with one more path, whose only documented response is 200 and which no handler serves.
openapi_with_undocumented_path() {
  awk '
    { print }
    /^paths:$/ {
      print "  /api/planted_undocumented:"
      print "    get:"
      print "      tags:"
      print "      - HealthCheck"
      print "      summary: Planted operation."
      print "      description: Planted operation that no handler serves."
      print "      operationId: planted_undocumented_get"
      print "      responses:"
      print "        '\''200'\'':"
      print "          description: Planted response."
    }
  ' "${OPENAPI_SNAPSHOT}"
}

# The GraphQL snapshot without the first field of its first object type.
graphql_without_field() {
  awk '
    /^type / { in_type = 1; print; next }
    in_type && !removed && /^  "/ { held = $0; next }
    in_type && !removed && /^  [A-Za-z_]/ { removed = 1; held = ""; next }
    { if (held != "") { print held; held = "" } print }
  ' "${GRAPHQL_SNAPSHOT}"
}

# --- generators --------------------------------------------------------------------------------------------

@test "generate-openapi-spec and generate-graphql-spec leave git status empty for both snapshot directories" {
  plant_keep "${OPENAPI_SNAPSHOT}"
  plant_keep "${GRAPHQL_SNAPSHOT}"
  run make --no-print-directory generate-openapi-spec generate-graphql-spec
  assert_success
  run git status --porcelain -- .github/openapi-spec .github/graphql-spec
  assert_success
  assert_output ''
}

# --- diff targets against a commit without snapshots ---------------------------------------------------------

@test "openapi-diff and graphql-diff against a commit without snapshots pass with the first-version notice" {
  require_docker
  local tree commit
  tree="$(git mktree </dev/null)"
  commit="$(GIT_AUTHOR_NAME=bats GIT_AUTHOR_EMAIL=bats@example.invalid GIT_COMMITTER_NAME=bats \
    GIT_COMMITTER_EMAIL=bats@example.invalid git commit-tree "${tree}" -m 'empty tree')"
  run make --no-print-directory openapi-diff "base_ref=${commit}"
  assert_success
  assert_output --partial 'this is the first version of the contract'
  run make --no-print-directory graphql-diff "base_ref=${commit}"
  assert_success
  assert_output --partial 'this is the first version of the contract'
}

# --- Schemathesis against a closed port ------------------------------------------------------------------------

@test "schemathesis-validate with SCHEMATHESIS_URL on a closed port starts no container and reports the failed connection" {
  require_docker
  local port
  port="$(free_port)"
  SCHEMATHESIS_URL="http://127.0.0.1:${port}" run make --no-print-directory schemathesis-validate
  assert_failure
  assert_output --partial 'Connection failed'
  assert_output --partial 'Failed to establish a new connection'
  run isolated_project_resources
  assert_success
  assert_output ''
}

# --- G17: contract snapshots are generated ---------------------------------------------------------------------

@test "G17: a changed OpenAPI snapshot staged in the index makes validate-contract-snapshots regenerate it and fail" {
  { cat "${OPENAPI_SNAPSHOT}"; echo '# planted change'; } >"${BATS_TEST_TMPDIR}/planted"
  plant_replace "${OPENAPI_SNAPSHOT}" "${BATS_TEST_TMPDIR}/planted"
  plant_index "${OPENAPI_SNAPSHOT}"
  git add -- "${OPENAPI_SNAPSHOT}"
  run make --no-print-directory validate-contract-snapshots
  assert_failure
  assert_output --partial "${OPENAPI_SNAPSHOT}"
  assert_output --partial 'A committed contract snapshot differs from the generated one or is not tracked.'
  run git diff --quiet HEAD -- "${OPENAPI_SNAPSHOT}"
  assert_success
}

@test "G17: a changed GraphQL snapshot staged in the index makes validate-contract-snapshots regenerate it and fail" {
  { cat "${GRAPHQL_SNAPSHOT}"; echo '# planted change'; } >"${BATS_TEST_TMPDIR}/planted"
  plant_replace "${GRAPHQL_SNAPSHOT}" "${BATS_TEST_TMPDIR}/planted"
  plant_index "${GRAPHQL_SNAPSHOT}"
  git add -- "${GRAPHQL_SNAPSHOT}"
  run make --no-print-directory validate-contract-snapshots
  assert_failure
  assert_output --partial "${GRAPHQL_SNAPSHOT}"
  assert_output --partial 'A committed contract snapshot differs from the generated one or is not tracked.'
  run git diff --quiet HEAD -- "${GRAPHQL_SNAPSHOT}"
  assert_success
}

# --- G18: OpenAPI lint -----------------------------------------------------------------------------------------

@test "G18: an operation without a description makes validate-openapi-spec fail" {
  require_docker
  openapi_without_operation_description >"${BATS_TEST_TMPDIR}/planted"
  plant_replace "${OPENAPI_SNAPSHOT}" "${BATS_TEST_TMPDIR}/planted"
  run make --no-print-directory validate-openapi-spec
  assert_failure
  assert_output --partial 'operation-must-have-description'
}

# --- G19: breaking OpenAPI change ------------------------------------------------------------------------------

@test "G19: a removed required response property makes openapi-diff fail with the rule and the label line" {
  require_docker
  openapi_without_required_response_property >"${BATS_TEST_TMPDIR}/planted"
  plant_replace "${OPENAPI_SNAPSHOT}" "${BATS_TEST_TMPDIR}/planted"
  run make --no-print-directory openapi-diff base_ref=HEAD
  assert_failure
  assert_output --partial 'response-required-property-removed'
  assert_output --partial 'Approve with the label approved-breaking-change'
}

@test "G19: the same change passes with BREAKING_CHANGE_APPROVED=true" {
  require_docker
  openapi_without_required_response_property >"${BATS_TEST_TMPDIR}/planted"
  plant_replace "${OPENAPI_SNAPSHOT}" "${BATS_TEST_TMPDIR}/planted"
  BREAKING_CHANGE_APPROVED=true run make --no-print-directory openapi-diff base_ref=HEAD
  assert_success
  assert_output --partial 'response-required-property-removed'
  assert_output --partial 'approved by BREAKING_CHANGE_APPROVED=true'
}

@test "G19: a base ref that does not exist makes openapi-diff fail, and the script exits 2" {
  require_docker
  run make --no-print-directory openapi-diff base_ref=no-such-ref
  assert_failure
  assert_output --partial "base ref 'no-such-ref' cannot be resolved"
  OPENAPI_DIFF_IMAGE="$(make_var OPENAPI_DIFF_IMAGE)" OASDIFF_FLAGS="$(make_var OASDIFF_FLAGS)" \
    run scripts/openapi-diff.sh no-such-ref
  assert_failure 2
  assert_output --partial "base ref 'no-such-ref' cannot be resolved"
}

# --- G20: breaking GraphQL change ------------------------------------------------------------------------------

@test "G20: a removed field makes graphql-diff fail with the breaking change" {
  require_docker
  graphql_without_field >"${BATS_TEST_TMPDIR}/planted"
  plant_replace "${GRAPHQL_SNAPSHOT}" "${BATS_TEST_TMPDIR}/planted"
  run make --no-print-directory graphql-diff base_ref=HEAD
  assert_failure
  assert_output --partial 'breaking change'
  assert_output --partial 'Approve with the label approved-breaking-change'
}

@test "G20: the same change passes with BREAKING_CHANGE_APPROVED=true" {
  require_docker
  graphql_without_field >"${BATS_TEST_TMPDIR}/planted"
  plant_replace "${GRAPHQL_SNAPSHOT}" "${BATS_TEST_TMPDIR}/planted"
  BREAKING_CHANGE_APPROVED=true run make --no-print-directory graphql-diff base_ref=HEAD
  assert_success
  assert_output --partial 'breaking change'
  assert_output --partial 'approved by BREAKING_CHANGE_APPROVED=true'
  refute_output --partial 'Approve with the label approved-breaking-change'
}

@test "G20: a base ref that does not exist makes graphql-diff fail, and the script exits 2" {
  require_docker
  run make --no-print-directory graphql-diff base_ref=no-such-ref
  assert_failure
  assert_output --partial "base ref 'no-such-ref' cannot be resolved"
  NODE_IMAGE="$(make_var NODE_IMAGE)" GRAPHQL_INSPECTOR_VERSION="$(make_var GRAPHQL_INSPECTOR_VERSION)" \
    run scripts/graphql-diff.sh no-such-ref
  assert_failure 2
  assert_output --partial "base ref 'no-such-ref' cannot be resolved"
}

# --- the image of the OpenAPI diff can be overridden -------------------------------------------------------------

@test "openapi-diff runs the image named by OPENAPI_DIFF_IMAGE" {
  # A stand-in for the container engine records its arguments; no container runs.
  mkdir -p "${BATS_TEST_TMPDIR}/bin"
  cat >"${BATS_TEST_TMPDIR}/bin/docker" <<'STUB'
#!/bin/sh
printf '%s\n' "$*" >>"${RECORD}"
exit 0
STUB
  chmod +x "${BATS_TEST_TMPDIR}/bin/docker"
  local default_image
  default_image="$(make_var OPENAPI_DIFF_IMAGE)"
  [[ -n "${default_image}" ]]
  RECORD="${BATS_TEST_TMPDIR}/calls" DOCKER="${BATS_TEST_TMPDIR}/bin/docker" OPENAPI_DIFF_IMAGE=registry.example.invalid/oasdiff:planted \
    run make --no-print-directory openapi-diff base_ref=HEAD
  assert_success
  run grep -E '^run .* registry\.example\.invalid/oasdiff:planted breaking /base/spec\.yaml /head/spec\.yaml' "${BATS_TEST_TMPDIR}/calls"
  assert_success
  run grep -c -F -- "${default_image}" "${BATS_TEST_TMPDIR}/calls"
  assert_output '0'
}

# --- G21: Schemathesis against the production image --------------------------------------------------------------

# bats test_tags=slow
@test "G21: an undocumented path in the snapshot makes schemathesis-validate fail and leaves no isolated project" {
  require_docker
  openapi_with_undocumented_path >"${BATS_TEST_TMPDIR}/planted"
  plant_replace "${OPENAPI_SNAPSHOT}" "${BATS_TEST_TMPDIR}/planted"
  run make --no-print-directory schemathesis-validate
  assert_failure
  assert_output --partial 'planted_undocumented'
  run isolated_project_resources
  assert_success
  assert_output ''
}
