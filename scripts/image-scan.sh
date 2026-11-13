#!/usr/bin/env bash
# Scan a container image for HIGH and CRITICAL vulnerabilities with Trivy
# (architecture-delivery.md DD-21, gate G10).
#
# Usage: scripts/image-scan.sh <image>
#
# Steps: `docker save` the image to target/reports/image-scan/image.tar; run Trivy on the archive with
# $TRIVY_FLAGS. The Docker socket is not mounted. Trivy keeps its database in ~/.cache/trivy
# (TRIVY_CACHE_DIR overrides the directory) and downloads it when it is missing or outdated.
# Environment: TRIVY_IMAGE and TRIVY_FLAGS (exported by the Makefile from make/tool-versions.mk and
# make/gates.mk), DOCKER (default docker).
# Exit status: 0 no finding; 1 findings or a scan that failed; 2 usage error, image not found,
# or a tool that could not run.
set -euo pipefail

usage() {
  echo "usage: scripts/image-scan.sh <image>" >&2
}

if [[ $# -eq 1 && ( "$1" == "-h" || "$1" == "--help" ) ]]; then
  usage
  exit 0
fi
if [[ $# -ne 1 || -z "$1" ]]; then
  usage
  exit 2
fi

image="$1"
docker_cmd="${DOCKER:-docker}"

for name in TRIVY_IMAGE TRIVY_FLAGS; do
  if [[ -z "${!name:-}" ]]; then
    echo "image-scan: ${name} is not set (the Makefile exports it from make/tool-versions.mk and make/gates.mk)" >&2
    exit 2
  fi
done
if ! command -v "${docker_cmd}" > /dev/null 2>&1; then
  echo "image-scan: ${docker_cmd} is not available" >&2
  exit 2
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${root}"

if ! "${docker_cmd}" image inspect "${image}" > /dev/null 2>&1; then
  echo "image-scan: image ${image} not found" >&2
  exit 2
fi

report_dir="${root}/target/reports/image-scan"
mkdir -p "${report_dir}"
rm -f "${report_dir}/image.tar"
if ! "${docker_cmd}" save --output "${report_dir}/image.tar" "${image}"; then
  echo "image-scan: could not save image ${image}" >&2
  exit 2
fi

cache_dir="${TRIVY_CACHE_DIR:-${XDG_CACHE_HOME:-${HOME}/.cache}/trivy}"
mkdir -p "${cache_dir}"
ignore_file="${root}/.trivyignore"
if [[ ! -f "${ignore_file}" ]]; then
  echo "image-scan: ${ignore_file} is missing" >&2
  exit 2
fi

# TRIVY_FLAGS is a list of words by design.
# shellcheck disable=SC2086
"${docker_cmd}" run --rm --user "$(id -u):$(id -g)" \
  -v "${report_dir}:/scan:ro" -v "${cache_dir}:/cache" \
  -v "${ignore_file}:/work/.trivyignore:ro" -w /work \
  "${TRIVY_IMAGE}" image --cache-dir /cache --input /scan/image.tar ${TRIVY_FLAGS}
