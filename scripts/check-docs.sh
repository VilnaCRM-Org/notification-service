#!/usr/bin/env bash
# Documentation-drift check (architecture-delivery.md DD-17, gate G26, FR110).
#
# Usage: scripts/check-docs.sh [--root <dir>] --service-help <dir>
#
# Reads every tracked Markdown file of <dir> (default: the current directory) except CHANGELOG.md and
# specs/**, and applies seven rules. Every violation is one line `(<rule>) <file>:<line>: <message>`.
#   (1) a cited `make <target>` must be a Make target;
#   (2) a cited `service <command>` must be in the help of the binary (option words between `service` and the
#       command are skipped; after `-p`, `--package` or `--bin` the word `service` names the package, so the command
#       is read only after a `--` word, option words after it skipped, as in `cargo run -p service -- <command>`,
#       and nothing is checked without one), and a cited slash command must have a file under .claude/commands/;
#   (3) a backticked token that begins with the name of a tracked top-level file or directory, or of _bmad/,
#       _bmad-output/, .ralph/ or bmalph/state/, must name a path that exists or that a git-ignore rule matches,
#       checked up to its first placeholder or wildcard character;
#   (4) the block between <!-- verification:start --> and <!-- verification:end --> in AGENTS.md must equal the
#       output of `make ci-list`, and no other numbered list or fenced block may cite three or more gate targets
#       (the markers exempt a block in AGENTS.md only);
#   (5) docs/environment.md must list exactly the union of: the variables in the help of `service serve`,
#       `migrate` and `healthcheck`; the variables interpolated in the compose files and the variables that their
#       top-level `secrets:` read with `environment:`; the names assigned or
#       referenced in .env and .env.test; the variables that the Makefile and make/*.mk declare with `?=`; the keys
#       of [env] in .cargo/config.toml;
#   (6) every Make target must appear in `make help` and in the table "All targets" of docs/make-targets.md (a
#       target of make/service.mk in docs/make-targets-service.md);
#   (7) every skill under .claude/skills/ has a `name` equal to its directory, a `description` and an entry in
#       .claude/skills/README.md.
#
# What counts as a citation: only inline code spans and the lines of fenced blocks, never plain prose. A slash
# command is a code span that is exactly `/name` (or starts with `/name `). It is accepted when .claude/commands/
# has the file name.md, or when `/name` is the first segment of a path of the OpenAPI snapshot, of a route that
# the service router registers (`/ping` included) or the WORKDIR of the Dockerfile; `/ping` and `/api` are
# therefore not slash commands. Test files and fixtures are never read.
#
# The script starts no cargo command. The help texts of the binary are read from <dir> of --service-help
# (root.txt, serve.txt, migrate.txt and healthcheck.txt, written by the recipe of `make docs-check`). "Tracked" means
# `git ls-files`; a root without a .git entry is read as the set of all its files.
#
# Exit status: 0 no violation; 1 violations; 2 usage error or a tool or input that could not run.
set -euo pipefail

usage() {
  echo "usage: scripts/check-docs.sh [--root <dir>] --service-help <dir>" >&2
}

could_not_run() {
  echo "check-docs could not run: $*" >&2
  exit 2
}

root="."
help_dir=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    -h | --help)
      usage
      exit 0
      ;;
    --root)
      [[ $# -ge 2 ]] || { usage; exit 2; }
      root="$2"
      shift 2
      ;;
    --service-help)
      [[ $# -ge 2 ]] || { usage; exit 2; }
      help_dir="$2"
      shift 2
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

[[ -n "${help_dir}" ]] || { usage; exit 2; }
[[ -d "${root}" ]] || could_not_run "the root ${root} is not a directory"
root="$(cd "${root}" && pwd)"
[[ -d "${help_dir}" ]] || could_not_run "the help directory ${help_dir} does not exist"
help_dir="$(cd "${help_dir}" && pwd)"
for name in root serve migrate healthcheck; do
  [[ -f "${help_dir}/${name}.txt" ]] || could_not_run "${help_dir}/${name}.txt is missing (run make docs-check)"
done

work="$(mktemp -d)"
trap 'rm -rf -- "${work}"' EXIT
findings="${work}/findings"
: >"${findings}"

# Appends one violation: rule number, then the text after the prefix.
report() {
  printf '(%s) %s\n' "$1" "$2" >>"${findings}"
}

# --- inputs ----------------------------------------------------------------------------------------------

if [[ -e "${root}/.git" ]]; then
  in_git=1
  git -C "${root}" ls-files >"${work}/files.raw" || could_not_run "git ls-files failed in ${root}"
else
  in_git=0
  (cd "${root}" && find . -type f -not -path './.git/*' | sed 's|^\./||') >"${work}/files.raw"
fi
# A file that is tracked but deleted from the working tree has nothing to read.
while IFS= read -r path; do
  [[ -f "${root}/${path}" ]] && printf '%s\n' "${path}"
done <"${work}/files.raw" | sort >"${work}/files"

grep -E '\.md$' "${work}/files" | grep -vxE 'CHANGELOG\.md|specs/.*' >"${work}/markdown" || true

# Top-level names, plus the local state directories that a checkout never tracks.
{
  cut -d/ -f1 "${work}/files"
  printf '%s\n' _bmad _bmad-output .ralph
} | sort -u >"${work}/tops"
printf '%s\n' _bmad _bmad-output .ralph >"${work}/specials"

make_cmd=(make -C "${root}" --no-print-directory)
make_database="$("${make_cmd[@]}" -pRrq : 2>/dev/null || true)"
printf '%s\n' "${make_database}" |
  awk -F: '/^[a-zA-Z0-9][^$#\/\t=]*:([^=]|$)/ {print $1}' | grep -vx 'Makefile' | sort -u >"${work}/targets" || true
[[ -s "${work}/targets" ]] || could_not_run "make could not list the targets of ${root}"
"${make_cmd[@]}" help 2>/dev/null |
  awk '/^Targets:$/ {found = 1; next} found && NF {print $1}' | sort -u >"${work}/helped" || true
"${make_cmd[@]}" ci-list >"${work}/ci-list" 2>/dev/null || could_not_run "make ci-list failed in ${root}"
sort -u "${work}/ci-list" >"${work}/gates"

# Commands of the binary, and the subcommands of each command (`<command> <subcommand>` per line).
help_commands() {
  awk '/^Commands:/ {found = 1; next} /^[^ ]/ {found = 0} found && /^  [a-z]/ {print $1}' "$1"
}
help_commands "${help_dir}/root.txt" | sort -u >"${work}/commands"
: >"${work}/subcommands"
for file in "${help_dir}"/*.txt; do
  command_name="$(basename "${file}" .txt)"
  [[ "${command_name}" != root ]] || continue
  help_commands "${file}" | sed "s/^/${command_name} /" >>"${work}/subcommands" || true
done

# Accepted names come from closed sources only: the command files, the first path segments of the OpenAPI
# snapshot, the routes of the service router (and /ping), and the WORKDIR of the Dockerfile. No test or
# fixture is read.
{
  find "${root}/.claude/commands" -maxdepth 1 -type f -name '*.md' 2>/dev/null | sed -E 's|.*/([^/]+)\.md$|\1|'
  echo ping
  if [[ -f "${root}/.github/openapi-spec/spec.yaml" ]]; then
    awk '/^paths:/ {in_paths = 1; next} /^[^ #]/ {in_paths = 0} in_paths && /^  \/[a-z]/ {print $1}' \
      "${root}/.github/openapi-spec/spec.yaml" | sed -E 's|^/([a-z][a-z0-9_-]*).*|\1|'
  fi
  if [[ -f "${root}/crates/service/src/http/router.rs" ]]; then
    grep -oE '\.route\("/[a-z][a-z0-9_-]*' "${root}/crates/service/src/http/router.rs" | sed -E 's|.*"/||' || true
  fi
  if [[ -f "${root}/Dockerfile" ]]; then
    awk '$1 == "WORKDIR" && $2 ~ /^\/[a-z]/ {print $2}' "${root}/Dockerfile" | sed -E 's|^/([a-z][a-z0-9_-]*).*|\1|'
  fi
} | sort -u >"${work}/slash"

# --- rules (1) to (4), citations and paths --------------------------------------------------------------

cat >"${work}/rules.awk" <<'AWK'
function load(file, arr,    l) {
  while ((getline l < file) > 0) arr[l] = 1
  close(file)
}
function trim(s) {
  sub(/^[[:space:]]+/, "", s)
  sub(/[[:space:]]+$/, "", s)
  return s
}
function violation_at(rule, at, text) {
  printf "(%s) %s:%d: %s\n", rule, cf, at, text
}
function join(arr,    key, out) {
  out = ""
  for (key in arr) out = out (out == "" ? "" : ", ") key
  return out
}
function count(arr,    key, n) {
  n = 0
  for (key in arr) n++
  return n
}
function cited(target, kind) {
  if (!(target in T)) violation_at(1, FNR, "make " target " cites a target that the Makefile does not define")
  if (target in G) {
    if (kind == "fence") fg[target] = 1
    else if (inlist) lg[target] = 1
  }
}
# `make a b VAR=x`: every word after make that is a plain target name, flags skipped, up to the first other word.
function cite_make(s, kind,    n, w, i, j, t) {
  n = split(s, w, /[[:space:]]+/)
  for (i = 1; i <= n; i++) {
    if (w[i] != "make") continue
    for (j = i + 1; j <= n; j++) {
      t = w[j]
      if (t ~ /^-/) continue
      if (t !~ /^[a-z][a-z0-9_-]*$/) break
      cited(t, kind)
    }
  }
}
# `service <command> [<subcommand>]`; option words after `service` are skipped before the command is read. After
# `-p`, `--package` or `--bin` (`cargo run -p service -- <command>`) `service` names the package, so the command is
# read only after a `--` word (option words after it skipped), and nothing is checked without one
# (`cargo test -p service --lib config`).
function cite_service(s,    n, w, i, k, t, u) {
  n = split(s, w, /[[:space:]]+/)
  for (i = 1; i < n; i++) {
    if (w[i] != "service") continue
    k = i + 1
    if (i > 1 && (w[i - 1] == "-p" || w[i - 1] == "--package" || w[i - 1] == "--bin")) {
      while (k <= n && w[k] != "--") k++
      if (k > n) continue
      k++
    }
    while (k <= n && w[k] ~ /^-/) k++
    if (k > n) continue
    t = w[k]
    if (t !~ /^[a-z][a-z-]*$/) continue
    if (!(t in C)) {
      violation_at(2, FNR, "service " t " is not a command of the binary (service --help)")
      continue
    }
    if ((t in HASSUB) && k + 1 <= n) {
      u = w[k + 1]
      if (u ~ /^[a-z][a-z-]*$/ && !((t SUBSEP u) in SUB))
        violation_at(2, FNR, "service " t " " u " is not a subcommand of service " t)
    }
  }
}
function cite_slash(s,    name) {
  if (s !~ /^\/[a-z][a-z0-9-]*([[:space:]]|$)/) return
  name = s
  sub(/^\//, "", name)
  sub(/[[:space:]].*$/, "", name)
  if (!(name in S)) printf "%s\t%s:%d\n", name, cf, FNR >> slashcand
}
function cite_paths(s,    n, w, i, p, seg, k, pre) {
  n = split(s, w, /[[:space:]]+/)
  for (i = 1; i <= n; i++) {
    p = w[i]
    sub(/^[][("']+/, "", p)
    sub(/[])"',;:.!?]+$/, "", p)
    sub(/^[A-Za-z_][A-Za-z0-9_]*=/, "", p)
    sub(/#.*$/, "", p)
    sub(/:[0-9]+(:[0-9]+)?$/, "", p)
    sub(/^\.\//, "", p)
    if (p == "" || p ~ /:\/\//) continue
    seg = p
    sub(/\/.*$/, "", seg)
    if (!(seg in P) && p !~ /^bmalph\/state(\/|$)/) continue
    if ((seg in SP) && p == seg) p = p "/"
    k = match(p, /[<*?{$[]|…|\.\.\./)
    if (k > 0) {
      pre = substr(p, 1, k - 1)
      if (pre !~ /\/$/) sub(/[^\/]*$/, "", pre)
      p = pre
    }
    if (p == "") continue
    printf "%s\t%s:%d\n", p, cf, FNR >> cand
  }
}
function handle_span(c) {
  c = trim(c)
  if (c == "") return
  cite_make(c, "span")
  cite_service(c)
  cite_slash(c)
  cite_paths(c)
}
# Inline code spans of one line: a run of n backticks closes at the next run of exactly n backticks.
function spans(s,    i, n, run, j, m, found) {
  n = length(s)
  i = 1
  while (i <= n) {
    if (substr(s, i, 1) != BT) { i++; continue }
    run = 0
    while (substr(s, i + run, 1) == BT) run++
    j = i + run
    found = 0
    while (j <= n) {
      if (substr(s, j, 1) != BT) { j++; continue }
      m = 0
      while (substr(s, j + m, 1) == BT) m++
      if (m == run) { found = 1; break }
      j += m
    }
    if (found) {
      handle_span(substr(s, i + run, j - i - run))
      i = j + run
    } else {
      i += run
    }
  }
}
function end_fence(    n) {
  infence = 0
  n = count(fg)
  if (n >= 3 && !block)
    violation_at(4, fstart, "a fenced block cites " n " gate targets (" join(fg) "); link to the verification section of AGENTS.md instead")
  delete fg
}
function end_list(    n) {
  inlist = 0
  pend = 0
  n = count(lg)
  if (n >= 3 && !block)
    violation_at(4, lstart, "a numbered list cites " n " gate targets (" join(lg) "); link to the verification section of AGENTS.md instead")
  delete lg
}
BEGIN {
  BT = "\140"
  load(targets, T)
  load(commands, C)
  load(slashes, S)
  load(gates, G)
  load(tops, P)
  load(specials, SP)
  while ((getline l < subcommands) > 0) {
    split(l, a, " ")
    SUB[a[1] SUBSEP a[2]] = 1
    HASSUB[a[1]] = 1
  }
  close(subcommands)
}
FNR == 1 {
  if (inlist) end_list()
  infence = 0
  block = 0
  pend = 0
  cf = FILENAME
}
{
  line = $0
  s = line
  sub(/^[[:space:]]*/, "", s)
  c = substr(s, 1, 1)
  if (c == BT || c == "~") {
    run = 0
    while (substr(s, run + 1, 1) == c) run++
    if (run >= 3) {
      if (!infence) {
        infence = 1
        fc = c
        fn = run
        fstart = FNR
        delete fg
        next
      }
      if (c == fc && run >= fn && substr(s, run + 1) ~ /^[[:space:]]*$/) {
        end_fence()
        next
      }
    }
  }
  if (infence) {
    if (trim(line) !~ /^#/) {
      cite_make(line, "fence")
      cite_service(line)
    }
    next
  }
  isitem = (line ~ /^[[:space:]]*[0-9]+[.)][[:space:]]/)
  if (inlist && !isitem) {
    if (line ~ /^[[:space:]]*$/) pend = 1
    else if (pend && line !~ /^[[:space:]][[:space:]]/) end_list()
    else pend = 0
  }
  if (isitem) {
    if (!inlist) {
      inlist = 1
      lstart = FNR
      delete lg
    }
    pend = 0
  }
  spans(line)
  # Only the canonical list in AGENTS.md is exempt; the same markers in another file exempt nothing.
  if (cf == "AGENTS.md" && line ~ /<!-- verification:start -->/) block = 1
  if (cf == "AGENTS.md" && line ~ /<!-- verification:end -->/) block = 0
}
END {
  if (inlist) end_list()
}
AWK

: >"${work}/candidates"
: >"${work}/slash-candidates"
if [[ -s "${work}/markdown" ]]; then
  (
    cd "${root}"
    # shellcheck disable=SC2046 # the file list is split into awk operands on purpose
    awk -v targets="${work}/targets" -v commands="${work}/commands" -v subcommands="${work}/subcommands" \
      -v slashes="${work}/slash" -v gates="${work}/gates" -v tops="${work}/tops" -v specials="${work}/specials" \
      -v cand="${work}/candidates" -v slashcand="${work}/slash-candidates" -f "${work}/rules.awk" $(tr '\n' ' ' <"${work}/markdown")
  ) >"${work}/awk-findings"
  while IFS= read -r line; do
    printf '%s\n' "${line}" >>"${findings}"
  done <"${work}/awk-findings"
fi

# Rule (2), slash commands: every candidate left by the awk program is a name outside the accepted set.
if [[ -s "${work}/slash-candidates" ]]; then
  awk -F'\t' '{print "(2) " $2 ": slash command /" $1 " has no file .claude/commands/" $1 ".md and is no route, API path or mount point of the service"}' \
    "${work}/slash-candidates" | sort -u >>"${findings}"
fi

# Rule (3): the candidate paths must exist or be ignored by git.
if [[ -s "${work}/candidates" ]]; then
  cut -f1 "${work}/candidates" | sort -u >"${work}/paths"
  : >"${work}/missing"
  while IFS= read -r path; do
    [[ -e "${root}/${path}" ]] || printf '%s\n' "${path}" >>"${work}/missing"
  done <"${work}/paths"
  : >"${work}/ignored"
  if [[ -s "${work}/missing" && "${in_git}" == 1 ]]; then
    git -C "${root}" check-ignore --stdin <"${work}/missing" 2>/dev/null | sort -u >"${work}/ignored" || true
  fi
  comm -23 "${work}/missing" "${work}/ignored" >"${work}/absent"
  if [[ -s "${work}/absent" ]]; then
    awk -F'\t' 'NR == FNR {absent[$0] = 1; next} ($1 in absent) {print "(3) " $2 ": " $1 " does not exist and no git-ignore rule matches it"}' \
      "${work}/absent" "${work}/candidates" | sort -u >>"${findings}"
  fi
fi

# --- rule (4): the verification block ---------------------------------------------------------------------

if [[ ! -f "${root}/AGENTS.md" ]]; then
  report 4 "AGENTS.md: the file is missing"
elif ! grep -q '<!-- verification:start -->' "${root}/AGENTS.md" || ! grep -q '<!-- verification:end -->' "${root}/AGENTS.md"; then
  report 4 "AGENTS.md: the markers <!-- verification:start --> and <!-- verification:end --> are missing"
else
  awk '/<!-- verification:end -->/ {inside = 0} inside && !/^[[:space:]]*(```|~~~)/ && NF {sub(/^[[:space:]]*make[[:space:]]+/, ""); print} /<!-- verification:start -->/ {inside = 1}' \
    "${root}/AGENTS.md" >"${work}/block"
  if ! diff -u "${work}/ci-list" "${work}/block" >"${work}/block.diff"; then
    report 4 "AGENTS.md: the verification block differs from the output of make ci-list (- make ci-list, + AGENTS.md)"
    tail -n +3 "${work}/block.diff" | grep -E '^[-+]' | while IFS= read -r line; do
      report 4 "AGENTS.md:   ${line}"
    done
  fi
fi

# --- rule (5): docs/environment.md ------------------------------------------------------------------------

if [[ ! -f "${root}/docs/environment.md" ]]; then
  report 5 "docs/environment.md: the file is missing"
else
  # Rows of the tables whose header starts with "Name"; other tables (the PHP name mapping) list other names.
  awk '/^\| Name \|/ {inside = 1; next} !/^\|/ {inside = 0} inside && match($0, /^\| `[A-Za-z_][A-Za-z0-9_]*` \|/) {print substr($0, 4, RLENGTH - 6)}' \
    "${root}/docs/environment.md" | sort -u >"${work}/env-documented"
  {
    for name in serve migrate healthcheck; do
      grep -oE '\[env: [A-Za-z_][A-Za-z0-9_]*\]' "${help_dir}/${name}.txt" | sed -E 's/^\[env: (.*)\]$/\1/' || true
    done
    for file in docker-compose.yml docker-compose.override.yml docker-compose.prod.yml docker-compose.load_test.override.yml; do
      [[ -f "${root}/${file}" ]] || continue
      grep -oE '(^|[^$])\$\{[A-Za-z_][A-Za-z0-9_]*' "${root}/${file}" | sed -E 's/.*\$\{//' || true
      grep -oE '(^|[^$])\$[A-Za-z_][A-Za-z0-9_]*' "${root}/${file}" | sed -E 's/.*\$//' || true
      # The variables that a top-level secret reads: `secrets:` / `  <name>:` / `    environment: NAME`.
      awk -v quote="'" '/^secrets:[[:space:]]*$/ {inside = 1; next} /^[^[:space:]#]/ {inside = 0}
        inside && /^[[:space:]]+environment:/ {
          value = $0
          sub(/^[[:space:]]+environment:[[:space:]]*/, "", value)
          sub(/[[:space:]]*(#.*)?$/, "", value)
          gsub(/"/, "", value)
          gsub(quote, "", value)
          if (value ~ /^[A-Za-z_][A-Za-z0-9_]*$/) print value
        }' "${root}/${file}"
    done
    for file in .env .env.test; do
      [[ -f "${root}/${file}" ]] || continue
      sed -n -E 's/^[[:space:]]*(export[[:space:]]+)?([A-Za-z_][A-Za-z0-9_]*)=.*/\2/p' "${root}/${file}"
      grep -oE '\$\{[A-Za-z_][A-Za-z0-9_]*' "${root}/${file}" | sed -E 's/^\$\{//' || true
    done
    for file in "${root}/Makefile" "${root}"/make/*.mk; do
      [[ -f "${file}" ]] || continue
      grep -oE '(^|[[:space:]])[A-Za-z_][A-Za-z0-9_]*[[:space:]]*\?=' "${file}" | sed -E 's/[[:space:]]*\?=$//; s/^[[:space:]]+//' || true
    done
    if [[ -f "${root}/.cargo/config.toml" ]]; then
      awk '/^\[env\][[:space:]]*$/ {inside = 1; next} /^\[/ {inside = 0} inside && /^[A-Za-z_][A-Za-z0-9_]*[[:space:]]*=/ {sub(/[[:space:]]*=.*$/, ""); print}' \
        "${root}/.cargo/config.toml"
    fi
  } | sort -u >"${work}/env-sources"
  comm -23 "${work}/env-sources" "${work}/env-documented" | while IFS= read -r name; do
    report 5 "docs/environment.md: ${name} is read by the code, the compose files, the env files or the Makefile but has no row"
  done
  comm -13 "${work}/env-sources" "${work}/env-documented" | while IFS= read -r name; do
    report 5 "docs/environment.md: ${name} has a row but nothing reads it"
  done
fi

# --- rule (6): targets, help and the reference ------------------------------------------------------------

comm -23 "${work}/targets" "${work}/helped" | while IFS= read -r target; do
  report 6 "Makefile: the target ${target} is not listed by make help"
done

: >"${work}/service-targets"
if [[ -f "${root}/make/service.mk" ]]; then
  awk '/^[a-zA-Z0-9_][a-zA-Z0-9_ -]*:([^=]|$)/ {sub(/:.*$/, ""); n = split($0, names, " "); for (i = 1; i <= n; i++) print names[i]}' \
    "${root}/make/service.mk" | sort -u >"${work}/service-targets"
fi

# Targets with a row in a reference: in the table "All targets" of docs/make-targets.md, anywhere in the service reference.
rows_of() {
  awk -v only_section="$2" '
    only_section != "" && /^## / {inside = ($0 == "## " only_section); next}
    (only_section == "" || inside) {
      if (match($0, /^\| `[^`]+` \|/)) {
        cell = substr($0, RSTART + 3, RLENGTH - 6)
        print cell
      }
    }' "$1" | sort -u
}
if [[ -f "${root}/docs/make-targets.md" ]]; then
  rows_of "${root}/docs/make-targets.md" "All targets" >"${work}/rows-main"
else
  report 6 "docs/make-targets.md: the file is missing"
  : >"${work}/rows-main"
fi
if [[ -f "${root}/docs/make-targets-service.md" ]]; then
  rows_of "${root}/docs/make-targets-service.md" "" >"${work}/rows-service"
else
  : >"${work}/rows-service"
fi
while IFS= read -r target; do
  if grep -qxF -- "${target}" "${work}/service-targets"; then
    grep -qxF -- "${target}" "${work}/rows-service" ||
      report 6 "docs/make-targets-service.md: the target ${target} of make/service.mk has no row"
  else
    grep -qxF -- "${target}" "${work}/rows-main" ||
      report 6 "docs/make-targets.md: the target ${target} has no row in the table All targets"
  fi
done <"${work}/targets"

# --- rule (7): skills ---------------------------------------------------------------------------------------

skills="${root}/.claude/skills"
if [[ -d "${skills}" ]]; then
  for dir in "${skills}"/*/; do
    [[ -d "${dir}" ]] || continue
    skill="$(basename "${dir}")"
    rel=".claude/skills/${skill}/SKILL.md"
    if [[ ! -f "${dir}SKILL.md" ]]; then
      report 7 "${rel}: the skill directory has no SKILL.md"
      continue
    fi
    # Front matter: the lines between the first two `---` lines; a description may continue on indented lines.
    awk 'NR == 1 {if ($0 != "---") exit; next} /^---[[:space:]]*$/ {exit} {print}' "${dir}SKILL.md" >"${work}/front-matter"
    front_name="$(sed -n -E 's/^name:[[:space:]]*(.*[^[:space:]])[[:space:]]*$/\1/p' "${work}/front-matter" | head -n 1)"
    front_name="${front_name#[\"\']}"
    front_name="${front_name%[\"\']}"
    description="$(awk '/^description:/ {sub(/^description:[[:space:]]*/, ""); sub(/^[>|][-+]?[[:space:]]*/, ""); text = $0; capture = 1; next} capture && /^[[:space:]]+[^[:space:]]/ {text = text " " $0; next} {capture = 0} END {print text}' "${work}/front-matter")"
    if [[ "${front_name}" != "${skill}" ]]; then
      report 7 "${rel}: the name is '${front_name}' but the directory is ${skill}"
    fi
    if [[ -z "${description//[[:space:]]/}" ]]; then
      report 7 "${rel}: the description is missing or empty"
    fi
    if [[ ! -f "${skills}/README.md" ]] ||
      ! grep -Eq "(^|[^A-Za-z0-9_-])${skill}([^A-Za-z0-9_-]|\$)" "${skills}/README.md"; then
      report 7 ".claude/skills/README.md: the skill ${skill} has no entry in the index"
    fi
  done
fi

# --- result ---------------------------------------------------------------------------------------------------

if [[ -s "${findings}" ]]; then
  # By rule and file, then by line number as a number; lines without a number keep their order.
  LC_ALL=C sort -s -t: -k1,1 -k2,2n "${findings}"
  exit 1
fi
exit 0
