#!/usr/bin/env bash
# Validate the Structurizr workspace and compare its components with the workspace crates
# (architecture-delivery.md "Architecture Model", gate G27).
#
# Usage: scripts/validate-architecture-model.sh <dsl-file> <crate-list-file>
#
# <crate-list-file> holds one crate per line, the first word is the name (the output of
# `cargo tree --workspace --depth 0 --prefix none`); `-` reads the list from standard input.
# Steps: `validate -workspace` in the Structurizr image; `export -format json` to
# target/reports/structurizr/ (relative to the current directory); require a system-context,
# a container and a component view; read the names of the elements shown in the component view
# from the export (a JSON query run in the Node image) and compare them with the crate list,
# minus xtask, test-support and e2e-tests. Every missing and every extra component is printed.
# Then the relationships: every workspace-internal entry of a `[dependencies]` table (also
# `[target.<cfg>.dependencies]` and `[dependencies.<name>]`; a key may be quoted, and a `package =`
# rename names the crate) of a crate manifest below the current
# directory (target, .git, node_modules, .tools and fixtures directories are skipped) whose crate and
# dependency are both modelled components needs a relationship between the two components in the
# model; every missing relationship is printed with both component names.
# Environment: STRUCTURIZR_IMAGE, NODE_IMAGE (both exported by the Makefile), DOCKER (default docker).
# Exit status: 0 consistent; 1 parse error, a view missing, a component difference or a missing
# relationship; 2 usage error or a tool that could not run.
set -euo pipefail

usage() {
  echo "usage: scripts/validate-architecture-model.sh <dsl-file> <crate-list-file>" >&2
}

if [[ $# -eq 1 && ( "$1" == "-h" || "$1" == "--help" ) ]]; then
  usage
  exit 0
fi
if [[ $# -ne 2 ]]; then
  usage
  exit 2
fi

dsl_file="$1"
crate_list="$2"
docker_cmd="${DOCKER:-docker}"

if [[ ! -f "${dsl_file}" ]]; then
  echo "validate-architecture-model: ${dsl_file} is not a file" >&2
  exit 2
fi
if [[ "${crate_list}" != "-" && ! -f "${crate_list}" ]]; then
  echo "validate-architecture-model: ${crate_list} is not a file" >&2
  exit 2
fi
for name in STRUCTURIZR_IMAGE NODE_IMAGE; do
  if [[ -z "${!name:-}" ]]; then
    echo "validate-architecture-model: ${name} is not set (the Makefile exports it from make/tool-versions.mk)" >&2
    exit 2
  fi
done
if ! command -v "${docker_cmd}" > /dev/null 2>&1; then
  echo "validate-architecture-model: ${docker_cmd} is not available" >&2
  exit 2
fi

# Read the crate list before any other command can consume standard input.
if [[ "${crate_list}" == "-" ]]; then
  crates_raw="$(cat)"
else
  crates_raw="$(cat -- "${crate_list}")"
fi

dsl_abs="$(cd "$(dirname "${dsl_file}")" && pwd)/$(basename "${dsl_file}")"
mkdir -p target/reports/structurizr
out_dir="$(cd target/reports/structurizr && pwd)"
rm -f "${out_dir}/workspace.json"
user_args=(--user "$(id -u):$(id -g)")

# The file is mounted read-only and the export goes to a separate directory, so nothing is
# written next to the DSL file (PF-54).
dsl_mount=(-v "${dsl_abs}:/work/workspace.dsl:ro")

if ! "${docker_cmd}" run --rm "${user_args[@]}" "${dsl_mount[@]}" "${STRUCTURIZR_IMAGE}" \
  validate -workspace /work/workspace.dsl; then
  echo "validate-architecture-model: ${dsl_file} does not parse" >&2
  exit 1
fi

if ! "${docker_cmd}" run --rm "${user_args[@]}" "${dsl_mount[@]}" -v "${out_dir}:/out" \
  "${STRUCTURIZR_IMAGE}" export -workspace /work/workspace.dsl -format json -output /out > /dev/null; then
  echo "validate-architecture-model: export of ${dsl_file} failed" >&2
  exit 1
fi
if [[ ! -f "${out_dir}/workspace.json" ]]; then
  echo "validate-architecture-model: the export wrote no workspace.json" >&2
  exit 2
fi

# Prints "view <kind> <count>" for each view kind, "component <name>" for each component
# element shown in a component view and "relationship <source> <destination>" for each
# relationship of the model from one component to another.
# shellcheck disable=SC2016 # the JavaScript is passed to node as a literal
query='
const w = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
const views = w.views || {};
for (const kind of ["systemContextViews", "containerViews", "componentViews"]) {
  console.log("view " + kind + " " + (views[kind] || []).length);
}
const components = new Map();
for (const system of (w.model || {}).softwareSystems || []) {
  for (const container of system.containers || []) {
    for (const component of container.components || []) {
      components.set(component.id, component.name);
    }
  }
}
for (const view of views.componentViews || []) {
  for (const element of view.elements || []) {
    if (components.has(element.id)) {
      console.log("component " + components.get(element.id));
    }
  }
}
for (const system of (w.model || {}).softwareSystems || []) {
  for (const container of system.containers || []) {
    for (const component of container.components || []) {
      for (const relationship of component.relationships || []) {
        if (components.has(relationship.destinationId)) {
          console.log("relationship " + component.name + " " + components.get(relationship.destinationId));
        }
      }
    }
  }
}
'
if ! report="$("${docker_cmd}" run --rm "${user_args[@]}" -v "${out_dir}:/data:ro" "${NODE_IMAGE}" \
  node -e "${query}" /data/workspace.json)"; then
  echo "validate-architecture-model: reading the export failed" >&2
  exit 2
fi

status=0
for pair in "systemContextViews:system-context" "containerViews:container" "componentViews:component"; do
  key="${pair%%:*}"
  label="${pair#*:}"
  count="$(awk -v k="${key}" '$1 == "view" && $2 == k { print $3 }' <<< "${report}")"
  if [[ -z "${count}" || "${count}" -lt 1 ]]; then
    echo "missing view: no ${label} view in ${dsl_file}"
    status=1
  fi
done

expected="$(awk 'NF > 0 { print $1 }' <<< "${crates_raw}" \
  | grep -vxF -e xtask -e test-support -e e2e-tests | LC_ALL=C sort -u || true)"
actual="$(awk '$1 == "component" { print $2 }' <<< "${report}" | LC_ALL=C sort -u)"

while IFS= read -r name; do
  [[ -n "${name}" ]] || continue
  echo "missing component: ${name} is a workspace crate but not shown in the component view"
  status=1
done < <(comm -23 <(printf '%s\n' "${expected}") <(printf '%s\n' "${actual}"))
while IFS= read -r name; do
  [[ -n "${name}" ]] || continue
  echo "extra component: ${name} is shown in the component view but is not a workspace crate"
  status=1
done < <(comm -13 <(printf '%s\n' "${expected}") <(printf '%s\n' "${actual}"))

# Prints "<crate> <dependency>" for each entry of the [dependencies] tables of the crate manifests
# below the current directory (development and build dependencies are not relationships of the
# running service).
manifest_dependencies() {
  local manifest
  find . \( -name target -o -name .git -o -name node_modules -o -name .tools -o -name fixtures \) -prune \
    -o -type f -name Cargo.toml -print | LC_ALL=C sort | while IFS= read -r manifest; do
    awk '
      function quoted(s) { sub(/^[^"]*"/, "", s); sub(/".*$/, "", s); return s }
      /^[[:space:]]*\[/ {
        h = $0
        sub(/[[:space:]]*#.*$/, "", h)
        gsub(/[[:space:]]/, "", h)
        section = ""
        if (h == "[package]") section = "package"
        else if (h ~ /^\[(target\..*\.)?dependencies\]$/) section = "dependencies"
        else if (h ~ /^\[(target\..*\.)?dependencies\.[^].]+\]$/) {
          d = h
          sub(/^.*dependencies\./, "", d)
          sub(/\]$/, "", d)
          gsub(/"/, "", d)
          deps[++n] = d
          section = "table"
        }
        next
      }
      section == "package" && /^[[:space:]]*name[[:space:]]*=/ { name = quoted($0) }
      section == "dependencies" && /^[[:space:]]*("[A-Za-z0-9_-]+"|[A-Za-z0-9_-]+)[[:space:]]*[.=]/ {
        d = $0
        sub(/^[[:space:]]*/, "", d)
        if (d ~ /^"/) { d = quoted(d) } else { sub(/[[:space:]]*[.=].*$/, "", d) }
        if ($0 ~ /package[[:space:]]*=[[:space:]]*"/) { d = $0; sub(/^.*package[[:space:]]*=[[:space:]]*/, "", d); d = quoted(d) }
        deps[++n] = d
      }
      section == "table" && /^[[:space:]]*package[[:space:]]*=/ { deps[n] = quoted($0) }
      END { if (name != "") for (i = 1; i <= n; i++) print name " " deps[i] }
    ' "${manifest}"
  done
  return 0
}

relationships="$(awk '$1 == "relationship" { print $2 " " $3 }' <<< "${report}" | LC_ALL=C sort -u)"
while read -r from to; do
  [[ -n "${from}" && -n "${to}" ]] || continue
  grep -qxF -- "${from}" <<< "${actual}" || continue
  grep -qxF -- "${to}" <<< "${actual}" || continue
  if ! grep -qxF -- "${from} ${to}" <<< "${relationships}"; then
    echo "missing relationship: ${from} -> ${to} (${from} depends on ${to} in its manifest, the model has no relationship between the two components)"
    status=1
  fi
done < <(manifest_dependencies | LC_ALL=C sort -u)

if [[ "${status}" -eq 0 ]]; then
  echo "architecture model OK: $(grep -c . <<< "${actual}") components match the workspace crates; every dependency between them has a relationship"
fi
exit "${status}"
