#!/usr/bin/env bats
# Tests of scripts/validate-architecture-model.sh, the architecture-model gate G27 (FR120): the script passes on a
# fixture workspace with two components and the matching crate list on standard input, and six variants derived
# from the fixture with sed each exit 1 and name the view kind or the component; a dependency of one modelled crate
# on another in a manifest below the current directory needs a relationship between the two components
# (architecture-delivery.md "CLI Tests (Bats)", file check_architecture_model.bats; "Architecture Model").

bats_require_minimum_version 1.8.0
load test_helper

# bats file_tags=repository

setup() {
  _common_setup
  require_docker
  STRUCTURIZR_IMAGE="$(pinned_value STRUCTURIZR_IMAGE)"
  NODE_IMAGE="$(pinned_value NODE_IMAGE)"
  export STRUCTURIZR_IMAGE NODE_IMAGE
  fixture="${REPO_ROOT}/tests/CLI/bats/fixtures/architecture_model.dsl.txt"
  tmp="$(mktemp -d)"
  # The output of `cargo tree --workspace --depth 0 --prefix none`: the tooling crates are not modelled.
  crates="$(printf 'alpha-crate v0.1.0 (/work/alpha)\n\nbeta-crate v0.1.0 (/work/beta)\nxtask v0.1.0 (/work/xtask)\ntest-support v0.1.0 (/work/ts)\ne2e-tests v0.1.0 (/work/e2e)\n')"
}

teardown() {
  restore_planted
  [[ -z "${tmp:-}" ]] || rm -rf -- "${tmp}"
}

# Runs the script on a DSL file with the crate list on standard input.
check_model() {
  run scripts/validate-architecture-model.sh "$1" - <<<"${crates}"
}

@test "the fixture with two components matches the crate list" {
  check_model "${fixture}"
  assert_success
  assert_output --partial 'architecture model OK: 2 components match the workspace crates'
}

@test "a model without the system-context view exits 1 and names the view" {
  sed '/^        systemContext /,/^        }/d' "${fixture}" >"${tmp}/variant.dsl"
  check_model "${tmp}/variant.dsl"
  assert_failure 1
  assert_output --partial 'missing view: no system-context view'
  refute_output --partial 'no container view'
  refute_output --partial 'no component view'
}

@test "a model without the container view exits 1 and names the view" {
  sed '/^        container system /,/^        }/d' "${fixture}" >"${tmp}/variant.dsl"
  check_model "${tmp}/variant.dsl"
  assert_failure 1
  assert_output --partial 'missing view: no container view'
  refute_output --partial 'no system-context view'
}

@test "a model without the component view exits 1 and names the view" {
  sed '/^        component system.service /,/^        }/d' "${fixture}" >"${tmp}/variant.dsl"
  check_model "${tmp}/variant.dsl"
  assert_failure 1
  assert_output --partial 'missing view: no component view'
  assert_output --partial 'missing component: alpha-crate'
}

@test "a workspace crate that is missing from the model exits 1 and names the component" {
  sed -e '/beta = component/d' -e '/alpha -> beta/d' "${fixture}" >"${tmp}/variant.dsl"
  check_model "${tmp}/variant.dsl"
  assert_failure 1
  assert_output --partial 'missing component: beta-crate'
  refute_output --partial 'alpha-crate'
}

@test "a component that is not a workspace crate exits 1 and names the component" {
  sed '/beta = component/a\                gamma = component "gamma-crate" "Third component." "Rust crate"' "${fixture}" >"${tmp}/variant.dsl"
  check_model "${tmp}/variant.dsl"
  assert_failure 1
  assert_output --partial 'extra component: gamma-crate'
}

@test "a component excluded from the component view exits 1 and names the component" {
  sed '/^        component system.service /,/^        }/ s/include \*/include *\n            exclude system.service.beta/' \
    "${fixture}" >"${tmp}/variant.dsl"
  check_model "${tmp}/variant.dsl"
  assert_failure 1
  assert_output --partial 'missing component: beta-crate'
  refute_output --partial 'alpha-crate'
}

# Writes the manifests of the two fixture crates below ${tmp}/ws: alpha-crate depends on beta-crate through the
# given table (`dependencies` or `dev-dependencies`).
write_manifests() {
  mkdir -p "${tmp}/ws/alpha" "${tmp}/ws/beta" "${tmp}/ws/tests/fixtures/gamma"
  printf '[package]\nname = "alpha-crate"\n\n[%s]\nbeta-crate = { path = "../beta" }\nserde.workspace = true\n' "$1" \
    >"${tmp}/ws/alpha/Cargo.toml"
  printf '[package]\nname = "beta-crate"\n\n[dependencies]\n' >"${tmp}/ws/beta/Cargo.toml"
  # A fixture manifest is not a crate of the workspace: its dependency is not compared.
  printf '[package]\nname = "beta-crate"\n\n[dependencies]\nalpha-crate.workspace = true\n' \
    >"${tmp}/ws/tests/fixtures/gamma/Cargo.toml"
}

# Runs the script from ${tmp}/ws, so that the manifests below it are read.
check_model_in_ws() {
  local dsl="$1"
  run bash -c 'cd "$1" && "$2" "$3" - <<<"$4"' _ "${tmp}/ws" "${REPO_ROOT}/scripts/validate-architecture-model.sh" "${dsl}" "${crates}"
}

@test "a dependency between two modelled crates with a relationship passes" {
  write_manifests dependencies
  check_model_in_ws "${fixture}"
  assert_success
  assert_output --partial 'every dependency between them has a relationship'
}

@test "a dependency between two modelled crates without a relationship exits 1 and names both components" {
  write_manifests dependencies
  sed '/alpha -> beta/d' "${fixture}" >"${tmp}/variant.dsl"
  check_model_in_ws "${tmp}/variant.dsl"
  assert_failure 1
  assert_output --partial 'missing relationship: alpha-crate -> beta-crate'
  refute_output --partial 'beta-crate -> alpha-crate'
  refute_output --partial 'missing component'
}

@test "every form of a dependency entry between two modelled crates needs a relationship" {
  write_manifests dependencies
  sed '/alpha -> beta/d' "${fixture}" >"${tmp}/variant.dsl"
  local form failures=''
  # Each form names beta-crate once: a table of its own, a target table, a rename, a quoted key, a renamed target
  # table and a quoted key with a dotted value.
  for form in \
    '[dependencies.beta-crate]\npath = "../beta"' \
    "[target.'cfg(unix)'.dependencies]\nbeta-crate = { path = \"../beta\" }" \
    '[dependencies]\nrenamed = { package = "beta-crate", path = "../beta" }' \
    '[dependencies]\n"beta-crate" = { path = "../beta" }' \
    "[target.'cfg(unix)'.dependencies.renamed]\npackage = \"beta-crate\"\npath = \"../beta\"" \
    '[dependencies]\n"beta-crate".workspace = true'; do
    printf '[package]\nname = "alpha-crate"\n\n%b\n' "${form}" >"${tmp}/ws/alpha/Cargo.toml"
    check_model_in_ws "${tmp}/variant.dsl"
    if [[ "${status}" -ne 1 || "${output}" != *'missing relationship: alpha-crate -> beta-crate'* ]]; then
      failures+="${form}: status ${status}"$'\n'
    fi
  done
  assert_equal "${failures}" ''
}

@test "a development dependency needs no relationship" {
  write_manifests dev-dependencies
  sed '/alpha -> beta/d' "${fixture}" >"${tmp}/variant.dsl"
  check_model_in_ws "${tmp}/variant.dsl"
  assert_success
}

@test "a usage error exits 2" {
  run scripts/validate-architecture-model.sh
  assert_failure 2
  assert_output --partial 'usage: scripts/validate-architecture-model.sh'
}
