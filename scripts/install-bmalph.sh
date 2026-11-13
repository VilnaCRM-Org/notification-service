#!/usr/bin/env bash
# Install bmalph at BMALPH_VERSION, verify it and optionally initialise it in this project
# (architecture-delivery.md "AI Guidance and Documentation", ".claude/commands and bmalph").
#
# Usage: scripts/install-bmalph.sh [options]
#
# BMALPH_VERSION comes from the environment, else from `export BMALPH_VERSION := value` in
# make/tool-versions.mk. The package is installed under ${HOME}/.npm-global when the command is missing.
# Exit status: 0 success; 1 failure or usage error.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root_dir="$(dirname "${script_dir}")"
versions_file="${root_dir}/make/tool-versions.mk"
config_file="${root_dir}/bmalph/config.json"

die() {
  echo "Error: $*" >&2
  exit 1
}

usage() {
  cat <<'EOM'
Usage: bash scripts/install-bmalph.sh [options]

Install the bmalph CLI at the pinned version and optionally preview or run the
project initialisation.

Options:
  --platform <claude-code|codex>  Platform for the verification and the init (default: claude-code)
  --project-dir <path>            Project directory for --init (default: the repository root)
  --project-name <name>           Project name passed to bmalph init (default: from bmalph/config.json)
  --project-description <text>    Project description passed to bmalph init (default: from bmalph/config.json)
  --upgrade                       Reinstall the pinned package even when the command exists
  --init                          Run 'bmalph init' in the project directory
  --dry-run                       With --init: preview only, change nothing
  --skip-verify                   Skip the dry-run verification after the installation
  -h, --help                      Show this help text

Examples:
  bash scripts/install-bmalph.sh --platform claude-code
  bash scripts/install-bmalph.sh --platform codex --init --dry-run
EOM
}

# Value of NAME: the environment first, then the `export NAME := value` line of the fragment.
get_var() {
  local name="$1" line
  if [[ -n "${!name+set}" ]]; then
    printf '%s\n' "${!name}"
    return 0
  fi
  [[ -r "${versions_file}" ]] || return 1
  line="$(sed -n -E "s/^export[[:space:]]+${name}[[:space:]]*:=[[:space:]]*(.*[^[:space:]])[[:space:]]*$/\1/p" "${versions_file}" | head -n 1)"
  [[ -n "${line}" ]] || return 1
  printf '%s\n' "${line}"
}

# String value of a top-level key of bmalph/config.json (the file is flat for these keys).
config_value() {
  [[ -r "${config_file}" ]] || return 0
  sed -n -E "s/^[[:space:]]*\"$1\"[[:space:]]*:[[:space:]]*\"(.*)\",?[[:space:]]*$/\1/p" "${config_file}" | head -n 1
}

# Path fragment that the init output names for a platform.
expected_marker() {
  case "$1" in
    codex) printf '.agents/skills/' ;;
    *) printf '.claude/commands/' ;;
  esac
}

# Companion CLI that the platform expects.
platform_cli() {
  case "$1" in
    codex) printf 'codex' ;;
    *) printf 'claude' ;;
  esac
}

# Tracked files with unstaged or staged changes, one per line, sorted.
tracked_dirty_files() {
  local dir="$1"
  if ! git -C "${dir}" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    return 0
  fi
  {
    git -C "${dir}" diff --name-only
    git -C "${dir}" diff --name-only --cached
  } | sort -u
}

# Restore every tracked file that bmalph modified, so prefaces, CLAUDE.md and .gitignore stay as committed.
restore_tracked_changes() {
  local dir="$1" path
  while IFS= read -r path; do
    [[ -n "${path}" ]] || continue
    echo "Restoring tracked file modified only by bmalph: ${path}"
    git -C "${dir}" restore --staged --worktree -- "${path}"
  done < <(tracked_dirty_files "${dir}")
}

project_assets_present() {
  local dir="$1"
  [[ -f "${dir}/_bmad/COMMANDS.md" && -f "${dir}/.ralph/ralph_loop.sh" && -d "${dir}/.ralph/lib" ]]
}

# Run a disposable `bmalph init --dry-run` and check that the output names the platform marker.
verify_dry_run() {
  local platform="$1" name="$2" description="$3" marker tmp_dir tmp_out
  marker="$(expected_marker "${platform}")"
  tmp_dir="$(mktemp -d)"
  tmp_out="$(mktemp)"
  if ! bmalph -C "${tmp_dir}" init --platform "${platform}" --name "${name}" \
    --description "${description}" --dry-run >"${tmp_out}" 2>&1; then
    echo "Error: bmalph dry-run verification failed for platform '${platform}'." >&2
    sed -n '1,160p' "${tmp_out}" >&2
    rm -rf "${tmp_dir}" "${tmp_out}"
    return 1
  fi
  if ! grep -Fq -- "${marker}" "${tmp_out}"; then
    echo "Error: bmalph dry-run did not report the expected platform output '${marker}'." >&2
    sed -n '1,160p' "${tmp_out}" >&2
    rm -rf "${tmp_dir}" "${tmp_out}"
    return 1
  fi
  rm -rf "${tmp_dir}" "${tmp_out}"
}

main() {
  local platform="claude-code" project_dir="${root_dir}" project_name="" project_description=""
  local upgrade=false run_init=false dry_run=false skip_verify=false

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --platform) platform="${2:?Missing value for --platform}"; shift 2 ;;
      --project-dir) project_dir="${2:?Missing value for --project-dir}"; shift 2 ;;
      --project-name) project_name="${2:?Missing value for --project-name}"; shift 2 ;;
      --project-description) project_description="${2:?Missing value for --project-description}"; shift 2 ;;
      --upgrade) upgrade=true; shift ;;
      --init) run_init=true; shift ;;
      --dry-run) dry_run=true; shift ;;
      --skip-verify) skip_verify=true; shift ;;
      -h | --help) usage; exit 0 ;;
      *) echo "Error: unknown argument '$1'." >&2; usage >&2; exit 1 ;;
    esac
  done

  case "${platform}" in
    claude-code | codex) ;;
    *) die "unsupported platform '${platform}'. Use 'claude-code' or 'codex'." ;;
  esac

  [[ -n "${project_name}" ]] || project_name="$(config_value name)"
  [[ -n "${project_description}" ]] || project_description="$(config_value description)"
  [[ -n "${project_name}" ]] || die "no project name: pass --project-name or provide bmalph/config.json."

  local version package npm_bin npm_prefix
  version="$(get_var BMALPH_VERSION)" || die "BMALPH_VERSION is not set in the environment or in ${versions_file}."
  package="bmalph@${version}"

  npm_bin="${BMALPH_NPM_GLOBAL_BIN:-${HOME}/.npm-global/bin}"
  npm_bin="${npm_bin%/}"
  npm_prefix="${npm_bin%/bin}"
  export PATH="${npm_bin}:${PATH}"

  if [[ "${upgrade}" == true ]] || ! command -v bmalph >/dev/null 2>&1; then
    command -v npm >/dev/null 2>&1 || die "required command 'npm' is not installed (Node.js 20 or newer is needed)."
    mkdir -p "${npm_prefix}"
    npm install -g --prefix "${npm_prefix}" "${package}"
    command -v bmalph >/dev/null 2>&1 || die "bmalph was installed but is not in PATH; add ${npm_bin} to PATH."
  fi

  echo "BMALPH installed:"
  echo "  - command: $(command -v bmalph)"
  echo "  - version: $(bmalph --version)"
  echo "  - package: ${package}"

  local cli
  cli="$(platform_cli "${platform}")"
  if ! command -v "${cli}" >/dev/null 2>&1; then
    echo "Warning: expected platform CLI '${cli}' is not installed or not in PATH." >&2
  fi

  if [[ "${skip_verify}" != true ]]; then
    verify_dry_run "${platform}" "${project_name}" "${project_description}"
    echo "BMALPH dry-run verification passed for platform '${platform}'."
  fi

  if [[ "${dry_run}" == true && "${run_init}" != true ]]; then
    echo "Warning: --dry-run has no effect unless --init is also provided." >&2
  fi

  if [[ "${run_init}" != true ]]; then
    echo "BMALPH CLI is ready."
    echo "To preview the project initialisation, run:"
    echo "  make bmalph-init BMALPH_PLATFORM=${platform} BMALPH_DRY_RUN=true"
    return 0
  fi

  [[ -d "${project_dir}" ]] || die "--project-dir '${project_dir}' does not exist or is not a directory."
  [[ -w "${project_dir}" ]] || die "--project-dir '${project_dir}' is not writable."

  if [[ "${dry_run}" != true && -n "$(tracked_dirty_files "${project_dir}")" ]]; then
    cat >&2 <<'EOM'
Error: refusing to run bmalph init with existing tracked changes.
Commit or stash tracked files first, or use BMALPH_DRY_RUN=true to preview safely.
EOM
    exit 1
  fi

  echo "Running bmalph init in '${project_dir}' for platform '${platform}'."
  local -a init_cmd=(bmalph -C "${project_dir}" init --platform "${platform}"
    --name "${project_name}" --description "${project_description}")
  if [[ "${dry_run}" == true ]]; then
    init_cmd+=(--dry-run)
  fi
  "${init_cmd[@]}"

  if [[ "${dry_run}" != true ]]; then
    if ! project_assets_present "${project_dir}"; then
      echo "bmalph assets are incomplete after init; running 'bmalph upgrade --force' to restore the local files."
      bmalph -C "${project_dir}" upgrade --force
    fi
    restore_tracked_changes "${project_dir}"
  fi
}

# Run only when executed, so that the functions can be sourced by tests.
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  main "$@"
fi
