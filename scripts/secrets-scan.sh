#!/usr/bin/env bash
# Scan the git history and the working files for secrets with gitleaks
# (architecture-delivery.md DD-21, gate G9).
#
# Usage: scripts/secrets-scan.sh
#
# Steps: `gitleaks git` over the full history; then `gitleaks dir` over a temporary copy of the
# files that `git ls-files --cached --others --exclude-standard` lists (tracked files and untracked
# files that are not ignored), run from inside the copy so that fingerprints are relative to the
# repository. The copy uses only `cp` and `mkdir` with options that GNU and BSD (macOS) share, and skips a listed
# file that no longer exists (a tracked file deleted in the working tree). Findings are printed with rule, file and fingerprint, and the secret redacted.
# The configuration is .gitleaks.toml and the accepted findings are the fingerprints of .gitleaksignore.
# Environment: GITLEAKS_IMAGE (exported by the Makefile from make/tool-versions.mk), DOCKER (default docker).
# Exit status: 0 no leak found; 1 a leak was found; 2 usage error or a tool that could not run.
set -euo pipefail

usage() {
  echo "usage: scripts/secrets-scan.sh" >&2
}

if [[ $# -eq 1 && ( "$1" == "-h" || "$1" == "--help" ) ]]; then
  usage
  exit 0
fi
if [[ $# -ne 0 ]]; then
  usage
  exit 2
fi

docker_cmd="${DOCKER:-docker}"

if [[ -z "${GITLEAKS_IMAGE:-}" ]]; then
  echo "secrets-scan: GITLEAKS_IMAGE is not set (the Makefile exports it from make/tool-versions.mk)" >&2
  exit 2
fi
if ! command -v "${docker_cmd}" > /dev/null 2>&1; then
  echo "secrets-scan: ${docker_cmd} is not available" >&2
  exit 2
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${root}"
if ! git rev-parse --is-inside-work-tree > /dev/null 2>&1; then
  echo "secrets-scan: ${root} is not a git work tree" >&2
  exit 2
fi
common_dir="$(cd "$(git rev-parse --git-common-dir)" && pwd)"

copy_dir="$(mktemp -d)" || { echo "secrets-scan: could not create a temporary directory" >&2; exit 2; }
listing="$(mktemp)" || { rm -rf "${copy_dir}"; echo "secrets-scan: could not create a temporary file" >&2; exit 2; }
trap 'rm -rf "${copy_dir}" "${listing}"' EXIT

# The repository is mounted read-only at its own path, so a linked worktree resolves its git directory.
mounts=(-v "${root}:${root}:ro")
if [[ "${common_dir}" != "${root}"/* ]]; then
  mounts+=(-v "${common_dir}:${common_dir}:ro")
fi
# Linked worktrees keep their git data below the common directory; the scanner runs as the caller.
git_env=(-e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e "GIT_CONFIG_VALUE_0=*")
run_scanner=("${docker_cmd}" run --rm --network none --user "$(id -u):$(id -g)" "${git_env[@]}")

status=0

echo "secrets-scan: scanning the git history"
if ! "${run_scanner[@]}" "${mounts[@]}" -w "${root}" "${GITLEAKS_IMAGE}" \
  git --config .gitleaks.toml --gitleaks-ignore-path .gitleaksignore --redact --verbose --no-banner "${root}"; then
  status=1
fi

# Copies one listed path below the copy directory. A path that is gone (deleted in the working tree, or removed
# while the copy runs) and a directory entry (a submodule) are skipped; any other failure is returned.
copy_listed_path() {
  local path="$1" target
  [[ -e "${path}" || -L "${path}" ]] || return 0
  [[ -L "${path}" || ! -d "${path}" ]] || return 0
  target="${copy_dir}/${path}"
  mkdir -p "$(dirname -- "${target}")" || return 1
  cp -P -- "${path}" "${target}" || { [[ -e "${path}" || -L "${path}" ]] || return 0; return 1; }
}

echo "secrets-scan: scanning the working files"
if ! git ls-files -z --cached --others --exclude-standard >"${listing}"; then
  echo "secrets-scan: could not list the working files" >&2
  exit 2
fi
while IFS= read -r -d '' listed; do
  copy_listed_path "${listed}" || { echo "secrets-scan: could not copy the working files" >&2; exit 2; }
done <"${listing}"
if ! "${run_scanner[@]}" -v "${copy_dir}:/scan:ro" -v "${root}/.gitleaks.toml:/config/.gitleaks.toml:ro" \
  -v "${root}/.gitleaksignore:/config/.gitleaksignore:ro" -w /scan "${GITLEAKS_IMAGE}" \
  dir --config /config/.gitleaks.toml --gitleaks-ignore-path /config/.gitleaksignore --redact --verbose --no-banner .; then
  status=1
fi

if [[ "${status}" -eq 0 ]]; then
  echo "secrets-scan: no leaks found"
else
  echo "secrets-scan: leaks found" >&2
fi
exit "${status}"
