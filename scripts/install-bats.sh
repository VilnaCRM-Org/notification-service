#!/usr/bin/env bash
# Install Bats and three helper libraries at pinned commits (architecture-delivery.md DD-06).
#
# Usage: scripts/install-bats.sh [--prefix <dir>]
#
# The pins are read from make/tool-versions.mk (lines of the form `export NAME := <tag> <commit sha>`);
# a value in the environment takes precedence over the file, as inside Make.
# Exit status: 0 success; 1 commit mismatch or clone failure; 2 usage error or pins not readable.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root_dir="$(dirname "${script_dir}")"
versions_file="${root_dir}/make/tool-versions.mk"

prefix="${root_dir}/.tools/bats"

usage() {
  echo "usage: scripts/install-bats.sh [--prefix <dir>]" >&2
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix)
      if [[ $# -lt 2 || -z "$2" ]]; then
        usage
        exit 2
      fi
      prefix="$2"
      shift 2
      ;;
    --prefix=*)
      prefix="${1#--prefix=}"
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

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

# install_one <variable> <repository> <destination relative to the prefix>
install_one() {
  local var="$1" repo="$2" dest="${prefix}/$3"
  local pin tag sha
  if ! pin="$(get_var "${var}")"; then
    echo "error: ${var} is not set in the environment or in ${versions_file}" >&2
    exit 2
  fi
  read -r tag sha <<<"${pin}"
  if [[ -z "${tag}" || ! "${sha}" =~ ^[0-9a-f]{40}$ ]]; then
    echo "error: ${var} must be '<tag> <40-hex commit sha>', got '${pin}'" >&2
    exit 2
  fi

  if [[ -d "${dest}/.git" && "$(git -C "${dest}" rev-parse HEAD 2>/dev/null || true)" == "${sha}" ]]; then
    echo "${repo} ${tag}: already installed at ${sha}"
    return 0
  fi

  local work
  work="$(mktemp -d "${prefix}/.clone.XXXXXX")"
  if ! git clone --quiet --depth 1 --branch "${tag}" "https://github.com/${repo}.git" "${work}/src" 2>/dev/null; then
    rm -rf "${work}"
    echo "error: ${repo}: cannot clone tag ${tag}" >&2
    exit 1
  fi
  local actual
  actual="$(git -C "${work}/src" rev-parse HEAD)"
  if [[ "${actual}" != "${sha}" ]]; then
    rm -rf "${work}"
    echo "error: ${repo}: tag ${tag} is commit ${actual}, expected ${sha}" >&2
    exit 1
  fi
  rm -rf "${dest}"
  mkdir -p "$(dirname "${dest}")"
  mv "${work}/src" "${dest}"
  rm -rf "${work}"
  echo "${repo} ${tag}: installed at ${sha}"
}

mkdir -p "${prefix}/lib"
install_one BATS_CORE bats-core/bats-core core
install_one BATS_SUPPORT bats-core/bats-support lib/bats-support
install_one BATS_ASSERT bats-core/bats-assert lib/bats-assert
install_one BATS_FILE bats-core/bats-file lib/bats-file
