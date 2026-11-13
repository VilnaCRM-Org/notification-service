#!/usr/bin/env bash
# Check the host prerequisites and their minimum versions, and print one line per check.
#
# Usage: doctor.sh [--prerequisites]
#   --prerequisites  limit the check to what `make install` (or, in container mode, `make start`) needs
#
# Lines: `[OK] <name>`, `[MISSING] <name>: needed by <targets>; <how to install>`,
# `[OUTDATED] <name>: found <v>, need <v>; needed by <targets>; <how to install>`, `[OPTIONAL] <name>: ...` (never
# fails the check) and `[SKIP] ...`. The first line is the platform.
# The make check looks at the GNU Make that runs the doctor: the Makefile passes it as MAKE_COMMAND (`gmake` on a
# macOS host that installed GNU Make next to the old system `make`); without it the `make` on PATH is checked.
# Exit status: 0 all required checks passed; 1 otherwise; 2 on a wrong argument.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
only_prerequisites=false
case "${1-}" in
  '') ;;
  --prerequisites) only_prerequisites=true ;;
  *) echo "usage: doctor.sh [--prerequisites]" >&2; exit 2 ;;
esac

failed=0
container_mode=false
case "${RUN_IN_CONTAINER-}" in 1 | true) [ -n "${CI-}" ] || container_mode=true ;; esac

docker_cmd=${DOCKER:-docker}
read -r -a compose_cmd <<<"${DOCKER_COMPOSE:-docker compose}"

ok() { echo "[OK] $1"; }
missing() { # <name> <needed by> <how to install>
  echo "[MISSING] $1: needed by $2; $3"
  failed=1
}
outdated() { # <name> <found> <need> <needed by> <how to install>
  echo "[OUTDATED] $1: found $2, need $3; needed by $4; $5"
  failed=1
}
optional() { echo "[OPTIONAL] $1"; }

# The first x.y or x.y.z of the output of a command, or nothing.
version_of() {
  "$@" 2>&1 | grep -Eo '[0-9]+\.[0-9]+(\.[0-9]+)?' | head -n 1 || true
}

at_least() { # <found> <minimum>: true when found >= minimum
  [ "$(printf '%s\n%s\n' "$2" "$1" | sort -V | head -n 1)" = "$2" ]
}

# check_min <name> <needed by> <how to install> <minimum> <found version or empty when the tool is absent>
check_min() {
  local name=$1 needed=$2 how=$3 min=$4 found=${5-}
  if [ -z "$found" ]; then
    missing "$name" "$needed" "$how"
  elif at_least "$found" "$min"; then
    ok "$name $found"
  else
    outdated "$name" "$found" "$min" "$needed" "$how"
  fi
}

# check_exact <name> <needed by> <how to install> <pinned version> <found version or empty>
check_exact() {
  local name=$1 needed=$2 how=$3 pinned=$4 found=${5-}
  if [ -z "$found" ]; then
    missing "$name" "$needed" "$how"
  elif [ "$found" = "$pinned" ]; then
    ok "$name $found"
  else
    outdated "$name" "$found" "$pinned" "$needed" "$how"
  fi
}

have() { command -v "$1" >/dev/null 2>&1; }

pinned() { # <NAME> from make/tool-versions.mk (`export NAME := value`), or the environment
  local value=${!1-}
  if [ -z "$value" ] && [ -f "$root/make/tool-versions.mk" ]; then
    value=$(sed -n "s/^export $1[[:space:]]*:=[[:space:]]*//p" "$root/make/tool-versions.mk" | head -n 1)
  fi
  printf '%s' "$value"
}

print_platform() {
  local os arch verified='not verified in CI'
  os=$(uname -s | tr '[:upper:]' '[:lower:]')
  arch=$(uname -m)
  if [ "$os" = linux ] && grep -qi microsoft /proc/version 2>/dev/null; then
    os=wsl2
  elif [ "$os" = linux ] && [ "$arch" = x86_64 ]; then
    verified='verified in CI'
  fi
  echo "Platform: $os/$arch ($verified)"
}

check_base_tools() {
  local make_v bash_v git_v curl_v
  local make_cmd=${MAKE_COMMAND:-make}
  make_v=$(have "$make_cmd" && version_of "$make_cmd" --version || true)
  check_min make "every target" \
    "install GNU Make 4.0 or newer (on macOS: brew install make, then run gmake or put \$(brew --prefix)/opt/make/libexec/gnubin first on PATH)" \
    4.0 "$make_v"
  bash_v=${BASH_VERSINFO[0]}.${BASH_VERSINFO[1]}
  check_min bash "every script" "install Bash 4.4 or newer (on macOS: brew install bash)" 4.4 "$bash_v"
  git_v=$(have git && version_of git --version || true)
  check_min git "install, install-hooks, validate-configuration" "install git 2.30 or newer" 2.30 "$git_v"
  curl_v=$(have curl && version_of curl --version || true)
  check_min curl "install, install-tools.sh, install-bats.sh" "install curl 7.76 or newer" 7.76 "$curl_v"
}

check_container_engine() {
  local engine compose buildx
  if have "$docker_cmd" && engine=$("$docker_cmd" info --format '{{.ServerVersion}}' 2>/dev/null) && [ -n "$engine" ]; then
    check_min "Docker Engine" "start, build, every image-based tool, load tests, bats" \
      "install Docker Engine 25.0 or newer (the healthcheck key start_interval needs it)" 25.0 "$(version_of echo "$engine")"
  else
    missing "Docker Engine" "start, build, every image-based tool, load tests, bats" \
      "install Docker Engine 25.0 or newer and make it reachable (docker info must succeed)"
  fi
  compose=$("${compose_cmd[@]}" version --short 2>/dev/null || true)
  check_min "Docker Compose plugin" "start, start-deps, sh, logs" \
    "install the Compose plugin 2.24 or newer (the !reset tag needs it)" 2.24 "$(version_of echo "$compose")"
  buildx=$("$docker_cmd" buildx version 2>/dev/null || true)
  check_min "Docker Buildx" "build, build-prod-image" "install the Buildx plugin 0.12 or newer" 0.12 "$(version_of echo "$buildx")"
}

check_toolchain() {
  local channel found output
  if ! have rustup; then
    missing rustup "every cargo target" "install rustup 1.26 or newer from https://rustup.rs"
  else
    check_min rustup "every cargo target" "install rustup 1.26 or newer from https://rustup.rs" 1.26 "$(version_of rustup --version)"
  fi
  channel=$(sed -n 's/^channel[[:space:]]*=[[:space:]]*"\(.*\)"/\1/p' "$root/rust-toolchain.toml" 2>/dev/null | head -n 1)
  if ! have cargo; then
    missing cargo "every cargo target" "install rustup, which installs the toolchain $channel of rust-toolchain.toml"
    return
  fi
  # Installed or not is decided by the exit status: when the pinned toolchain is missing, rustup fails and its error
  # text names the channel, so a version taken from that text would look like the pinned one. The version is read
  # only from the line that rustc itself prints (`rustc <version> (...)`).
  if ! output=$(cd "$root" && RUSTUP_AUTO_INSTALL=0 rustc --version 2>&1); then
    missing "toolchain $channel" "every cargo target" "run: rustup toolchain install $channel"
    return
  fi
  found=$(printf '%s\n' "$output" | grep -E '^rustc ' | grep -Eo '[0-9]+\.[0-9]+(\.[0-9]+)?' | head -n 1 || true)
  if [ -z "$channel" ] || [ "$found" = "$channel" ]; then
    ok "cargo, toolchain ${found:-$channel}"
  elif [ -z "$found" ]; then
    missing "toolchain $channel" "every cargo target" "run: rustup toolchain install $channel"
  else
    outdated "toolchain" "$found" "$channel" "every cargo target" "run: rustup toolchain install $channel"
  fi
}

# check_cargo_tool <name> <PIN variable> <needed by> <command...>
check_cargo_tool() {
  local name=$1 pin=$2 needed=$3
  shift 3
  local found=''
  if have cargo; then
    found=$(cd "$root" && version_of "$@")
  fi
  check_exact "$name" "$needed" "run: make install" "$(pinned "$pin")" "$found"
}

check_cargo_tools() {
  check_cargo_tool cargo-nextest CARGO_NEXTEST_VERSION "unit-tests, integration-tests, coverage" cargo nextest --version
  check_cargo_tool cargo-llvm-cov CARGO_LLVM_COV_VERSION "tests-with-coverage, coverage-html, coverage-xml" cargo llvm-cov --version
  check_cargo_tool cargo-mutants CARGO_MUTANTS_VERSION "mutation-tests, mutation-tests-diff" cargo mutants --version
  check_cargo_tool cargo-deny CARGO_DENY_VERSION "check-security, check-security-offline" cargo deny --version
  check_cargo_tool cargo-shear CARGO_SHEAR_VERSION "unused-dependencies" cargo shear --version
  check_cargo_tool cargo-hack CARGO_HACK_VERSION "check-crate-isolation" cargo hack --version
  check_cargo_tool sqlx-cli SQLX_CLI_VERSION "setup-test-db, validate-sqlx-metadata, create-migration" sqlx --version
  check_cargo_tool clippy-sarif CLIPPY_SARIF_VERSION "static-analysis-sarif" clippy-sarif --version
  check_cargo_tool taplo TAPLO_VERSION "lint-toml, format-repo" taplo --version
  check_cargo_tool typos TYPOS_VERSION "lint-spelling" typos --version
  check_cargo_tool zizmor ZIZMOR_VERSION "lint-actions" zizmor --version
  check_cargo_tool cocogitto COCOGITTO_VERSION "bats (make_release.bats)" cog --version
}

check_bats() {
  if [ -x "$root/.tools/bats/core/bin/bats" ] || have bats; then
    ok "bats"
  else
    missing bats "bats" "run: scripts/install-bats.sh"
  fi
}

check_optional() {
  local node_v
  node_v=$(have node && version_of node --version || true)
  if [ -z "$node_v" ]; then
    optional "node: needed by the bmalph targets only; install Node.js 20 or newer with npm"
  elif at_least "$node_v" 20.0 && have npm; then
    ok "node $node_v (optional)"
  else
    optional "node: found ${node_v}, the bmalph targets need Node.js 20 or newer with npm"
  fi
  if have jq; then
    ok "jq (optional)"
  else
    optional "jq: needed by the assistant hook only; install jq"
  fi
}

print_platform
check_base_tools
if $container_mode; then
  check_container_engine
elif ! $only_prerequisites; then
  check_container_engine
fi
if $container_mode; then
  echo "[SKIP] toolchain and cargo tools: provided by the development container"
else
  check_toolchain
  $only_prerequisites || check_cargo_tools
fi
if ! $only_prerequisites; then
  check_bats
  check_optional
fi

exit "$failed"
