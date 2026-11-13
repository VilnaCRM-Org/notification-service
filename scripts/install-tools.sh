#!/usr/bin/env bash
# Install the cargo tools of make/tool-versions.mk (architecture-delivery.md DD-04).
#
# Usage: scripts/install-tools.sh [--with-watchexec]
#
# Versions come from make/tool-versions.mk (lines of the form `export NAME := value`); a value in the
# environment takes precedence. A tool already present at its pinned version is skipped. Other tools are
# installed with cargo-binstall (fetched as a release archive by URL and checked against the recorded
# SHA-256) with `--disable-strategies compile`; when no prebuilt binary exists the script says so and runs
# `cargo install --locked --version <v>`. GITHUB_TOKEN is optional and raises the GitHub rate limit.
# Exit status: 0 success; 1 a tool could not be installed; 2 usage error or versions not readable.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root_dir="$(dirname "${script_dir}")"
versions_file="${root_dir}/make/tool-versions.mk"

# Value of NAME: the environment first, then the `export NAME := value` line of the fragment.
get_var() {
  local name="$1"
  if [[ -n "${!name+set}" ]]; then
    printf '%s\n' "${!name}"
    return 0
  fi
  [[ -r "${versions_file}" ]] || return 1
  local line
  line="$(sed -n -E "s/^export[[:space:]]+${name}[[:space:]]*:=[[:space:]]*(.*[^[:space:]])[[:space:]]*$/\1/p" "${versions_file}" | head -n 1)"
  [[ -n "${line}" ]] || return 1
  printf '%s\n' "${line}"
}

require_var() {
  local value
  if ! value="$(get_var "$1")"; then
    echo "error: $1 is not set in the environment or in ${versions_file}" >&2
    exit 2
  fi
  printf '%s\n' "${value}"
}

# One tool per line: <crate>|<version variable>|<version command>|<extra cargo install arguments>
# The version command prints a line that contains the version as a separate word.
tools=(
  "cargo-nextest|CARGO_NEXTEST_VERSION|cargo nextest --version|"
  "cargo-llvm-cov|CARGO_LLVM_COV_VERSION|cargo llvm-cov --version|"
  "cargo-mutants|CARGO_MUTANTS_VERSION|cargo mutants --version|"
  "cargo-deny|CARGO_DENY_VERSION|cargo deny --version|"
  "cargo-shear|CARGO_SHEAR_VERSION|cargo shear --version|"
  "cargo-hack|CARGO_HACK_VERSION|cargo hack --version|"
  "sqlx-cli|SQLX_CLI_VERSION|sqlx --version|--no-default-features --features postgres,rustls"
  "clippy-sarif|CLIPPY_SARIF_VERSION|clippy-sarif --version|"
  "taplo-cli|TAPLO_VERSION|taplo --version|"
  "typos-cli|TYPOS_VERSION|typos --version|"
  "zizmor|ZIZMOR_VERSION|zizmor --version|"
  "cocogitto|COCOGITTO_VERSION|cog --version|"
)
watchexec_tool="watchexec-cli|WATCHEXEC_VERSION|watchexec --version|"

# True when the version command reports exactly the wanted version as one of its words.
has_version() {
  local version_cmd="$1" want="$2" out word
  # shellcheck disable=SC2086 # the command is a word list by construction
  out="$(${version_cmd} 2>/dev/null)" || return 1
  for word in ${out}; do
    [[ "${word}" == "${want}" ]] && return 0
  done
  return 1
}

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d ' ' -f 1
  else
    shasum -a 256 "$1" | cut -d ' ' -f 1
  fi
}

binstall_dir=""
binstall_bin=""
cleanup() {
  if [[ -n "${binstall_dir}" ]]; then
    rm -rf "${binstall_dir}"
  fi
}

# Fetch the cargo-binstall release archive by URL, check its SHA-256 and extract it. Sets binstall_bin.
# Returns 1 when binstall cannot be used on this platform or the archive cannot be fetched or verified;
# the caller then installs with cargo install.
fetch_binstall() {
  [[ -n "${binstall_bin}" ]] && return 0
  local version os arch target expected archive
  version="$(require_var CARGO_BINSTALL_VERSION)"
  os="$(uname -s)"
  arch="$(uname -m)"
  if [[ "${os}" != "Linux" ]]; then
    echo "cargo-binstall: no recorded checksum for ${os}; installing with cargo install" >&2
    return 1
  fi
  case "${arch}" in
    x86_64 | amd64)
      target="x86_64-unknown-linux-musl"
      expected="$(require_var CARGO_BINSTALL_SHA256_X86_64)"
      ;;
    aarch64 | arm64)
      target="aarch64-unknown-linux-musl"
      expected="$(require_var CARGO_BINSTALL_SHA256_AARCH64)"
      ;;
    *)
      echo "cargo-binstall: no recorded checksum for ${arch}; installing with cargo install" >&2
      return 1
      ;;
  esac
  binstall_dir="$(mktemp -d)"
  archive="${binstall_dir}/cargo-binstall.tgz"
  if ! curl --fail --silent --show-error --location --retry 3 --output "${archive}" \
    "https://github.com/cargo-bins/cargo-binstall/releases/download/v${version}/cargo-binstall-${target}.tgz"; then
    echo "cargo-binstall: download of ${version} failed" >&2
    return 1
  fi
  local actual
  actual="$(sha256_of "${archive}")"
  if [[ "${actual}" != "${expected}" ]]; then
    echo "cargo-binstall: SHA-256 mismatch for ${version} (${target}): expected ${expected}, got ${actual}" >&2
    return 1
  fi
  if ! tar -xzf "${archive}" -C "${binstall_dir}" cargo-binstall; then
    echo "cargo-binstall: cannot extract the archive" >&2
    return 1
  fi
  binstall_bin="${binstall_dir}/cargo-binstall"
}

# install_tool <crate> <version> <extra cargo install arguments>
install_tool() {
  local crate="$1" version="$2" extra="$3"
  if fetch_binstall; then
    if "${binstall_bin}" --no-confirm --disable-strategies compile "${crate}@=${version}"; then
      return 0
    fi
    echo "${crate} ${version}: no prebuilt binary available; building with cargo install" >&2
  else
    echo "${crate} ${version}: cargo-binstall is not available; building with cargo install" >&2
  fi
  local locked=(--locked)
  if [[ "${crate}" == "sqlx-cli" ]]; then
    locked=()
  fi
  # shellcheck disable=SC2086 # extra is a word list by construction
  cargo install "${locked[@]}" --version "${version}" ${extra} "${crate}"
}

main() {
  local with_watchexec=0
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --with-watchexec)
        with_watchexec=1
        shift
        ;;
      -h | --help)
        echo "usage: scripts/install-tools.sh [--with-watchexec]"
        return 0
        ;;
      *)
        echo "usage: scripts/install-tools.sh [--with-watchexec]" >&2
        return 2
        ;;
    esac
  done

  trap cleanup EXIT

  local list=("${tools[@]}")
  if [[ "${with_watchexec}" -eq 1 ]]; then
    list+=("${watchexec_tool}")
  fi

  local failed=()
  local entry crate var version_cmd extra version
  for entry in "${list[@]}"; do
    IFS='|' read -r crate var version_cmd extra <<<"${entry}"
    version="$(require_var "${var}")"
    if has_version "${version_cmd}" "${version}"; then
      echo "${crate} ${version}: already installed"
      continue
    fi
    echo "${crate} ${version}: installing"
    if install_tool "${crate}" "${version}" "${extra}" && has_version "${version_cmd}" "${version}"; then
      echo "${crate} ${version}: installed"
    else
      echo "error: ${crate} ${version} could not be installed" >&2
      failed+=("${crate}")
    fi
  done

  if [[ "${#failed[@]}" -gt 0 ]]; then
    echo "error: not installed: ${failed[*]}" >&2
    return 1
  fi
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  main "$@"
fi
