#!/usr/bin/env bats
# Tests of scripts/check-docs.sh, the documentation-drift gate G26: a passing run on a small valid tree with help
# texts, and one negative test per rule (1) to (7) on a generated tree passed with --root and --service-help
# (architecture-delivery.md "CLI Tests (Bats)", file check_docs.bats; DD-17).

bats_require_minimum_version 1.8.0
load test_helper

# bats file_tags=repository

setup() {
  _common_setup
  tmp="$(mktemp -d)"
  tree="${tmp}/tree"
  help="${tmp}/help"
  make_tree "${tree}" "${help}"
}

teardown() {
  restore_planted
  [[ -z "${tmp:-}" ]] || rm -rf -- "${tmp}"
}

# Writes the small valid tree (a Makefile with three gate targets, the guidance, the reference documents, one skill
# and one slash command) and the four help texts of the binary, then makes the tree a git repository.
make_tree() {
  local dir="$1" helpdir="$2"
  mkdir -p "${dir}/docs" "${dir}/.claude/commands" "${dir}/.claude/skills/demo-skill" "${dir}/.cargo" "${dir}/specs" "${helpdir}"

  cat >"${dir}/Makefile" <<'EOF'
.RECIPEPREFIX = >
.PHONY: help ci-list alpha beta gamma
FLAVOUR ?= plain
LISTED = alpha beta ci-list gamma help

help: ## Show the targets
> @printf 'Usage:\n  make [target] [arg="val"...]\nTargets:\n'
> @printf '%-12s %s\n' $(foreach name,$(LISTED),$(name) text)

ci-list: ## Print the gate targets
> @printf '%s\n' alpha beta gamma

alpha: ## First gate
> @true

beta: ## Second gate
> @true

gamma: ## Third gate
> @true
EOF

  printf 'target/\n_bmad/\n' >"${dir}/.gitignore"
  printf 'DB_USER=app\n' >"${dir}/.env"
  printf '[env]\nTEST_SEED = "1"\n' >"${dir}/.cargo/config.toml"
  printf 'kept so that specs/ is a tracked directory\n' >"${dir}/specs/keep.txt"

  cat >"${dir}/docker-compose.yml" <<'EOF'
services:
  app:
    ports:
      - "${DB_PORT:-5432}:5432"
    healthcheck:
      test: ["CMD", "curl", "http://localhost:8081/ping"]
  database:
    environment:
      POSTGRES_USER: $${ESCAPED}
EOF

  cat >"${dir}/AGENTS.md" <<'EOF'
# Agents

## Verification

Run the targets below. The list is the output of the list target.

<!-- verification:start -->

```bash
make alpha
make beta
make gamma
```

<!-- verification:end -->

For a change of documentation only, run `make help` first.
EOF

  cat >"${dir}/README.md" <<'EOF'
# Demo

Run `make alpha`, or `make beta` and `make gamma`. To make changes you make a branch and run make; the verb in
plain prose is not a citation. The assistant commands are listed by `/dev`; the liveness route is `/ping`.

Start the binary with `service serve`, or revert with `service migrate revert`; `cargo run -p service -- serve`
works too. The variables are in `docs/environment.md` and the targets in `docs/make-targets.md`. A report is
written to `target/reports/html`, the planning bundle to `specs/<bundle-id>/`, the local state is in `_bmad/` and
the references are `docs/make-targets*` and `.claude/skills/README.md#skills`.
EOF

  cat >"${dir}/docs/environment.md" <<'EOF'
# Environment

## Settings

| Name | Default | Consumer |
| --- | --- | --- |
| `APP_PORT` | `8081` | the service |
| `DB_PORT` | `5432` | compose |
| `DB_USER` | `app` | the env file |
| `FLAVOUR` | `plain` | the Makefile |
| `TEST_SEED` | `1` | the tests |

## Old names

| Old name | New name |
| --- | --- |
| `OLD_NAME` | `APP_PORT` |
EOF

  cat >"${dir}/docs/make-targets.md" <<'EOF'
# Make targets

## All targets

| Target | Help text |
| --- | --- |
| `alpha` | First gate |
| `beta` | Second gate |
| `ci-list` | Print the gates |
| `gamma` | Third gate |
| `help` | Show the targets |

## Other

| `unrelated` | not a row of the table All targets |
EOF

  printf '# dev\n' >"${dir}/.claude/commands/dev.md"
  cat >"${dir}/.claude/skills/README.md" <<'EOF'
# Skills

- `demo-skill`: use when demonstrating the check.
EOF
  cat >"${dir}/.claude/skills/demo-skill/SKILL.md" <<'EOF'
---
name: demo-skill
description: Use when demonstrating the documentation check.
---

# Demo Skill
EOF

  cat >"${helpdir}/root.txt" <<'EOF'
Demo

Usage: service <COMMAND>

Commands:
  serve    Serve until a signal
  migrate  Apply the migrations
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
EOF
  cat >"${helpdir}/serve.txt" <<'EOF'
Usage: service serve [OPTIONS]

Options:
      --app-port <APP_PORT>  TCP port (default 8081) [env: APP_PORT]
  -h, --help                 Print help
EOF
  cat >"${helpdir}/migrate.txt" <<'EOF'
Usage: service migrate [OPTIONS] [COMMAND]

Commands:
  revert  Revert the most recently applied migration
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
EOF
  cat >"${helpdir}/healthcheck.txt" <<'EOF'
Usage: service healthcheck [OPTIONS]

Options:
      --port <PORT>  TCP port (default 8081) [env: APP_PORT]
  -h, --help         Print help
EOF

  git -C "${dir}" init -q
  git -C "${dir}" add -A
}

# Runs the script on the generated tree, after the planted changes are tracked.
check() {
  [[ ! -e "${tree}/.git" ]] || git -C "${tree}" add -A
  run scripts/check-docs.sh --root "${tree}" --service-help "${help}"
}

# Rewrites a file through a filter command (no sed -i: BSD sed needs a suffix argument).
edit_file() {
  local file="$1"
  shift
  "$@" <"${file}" >"${file}.new"
  mv "${file}.new" "${file}"
}

# Appends text to a file of the generated tree: the second argument, or else standard input.
append() {
  if [[ $# -ge 2 ]]; then
    printf '%s\n' "$2" >>"${tree}/$1"
  else
    cat >>"${tree}/$1"
  fi
}

# --- the passing run ---------------------------------------------------------------------------------------

@test "the script passes on a small valid tree with help texts and prints nothing" {
  check
  assert_success
  assert_output ''
}

@test "a root without .git is read as the set of all its files, and nothing is git-ignored there" {
  rm -rf "${tree}/.git"
  cat >"${tree}/README.md" <<'EOF'
# Demo

See `docs/environment.md` and run `make alpha`.
EOF
  check
  assert_success
  append README.md <<'EOF'
The state is in `_bmad/`.
EOF
  check
  assert_failure 1
  assert_line --regexp '^\(3\) README\.md:[0-9]+: _bmad/ '
}

@test "CHANGELOG.md and specs are not read" {
  append CHANGELOG.md <<'EOF'
Run `make no-such-target` and see `docs/missing.md`.
EOF
  append specs/notes.md <<'EOF'
Run `make no-such-target` and see `docs/missing.md`.
EOF
  check
  assert_success
}

# --- rule (1): Make targets ----------------------------------------------------------------------------------

@test "rule 1: a backticked make command that names no target is reported with the rule number" {
  append README.md <<'EOF'
Run `make no-such-target` first.
EOF
  check
  assert_failure 1
  assert_line --regexp '^\(1\) README\.md:[0-9]+: make no-such-target '
}

@test "rule 1: every target after make is checked, flags and variable assignments are not" {
  append README.md <<'EOF'
Run `make -n alpha no-such-target FLAVOUR=x` and `RUN=1 make beta`.
EOF
  check
  assert_failure 1
  assert_line --regexp '^\(1\) README\.md:[0-9]+: make no-such-target '
  refute_output --partial 'make alpha'
  refute_output --partial 'make beta'
}

@test "rule 1: a line of a fenced block is a citation" {
  printf '%s\n' '```bash' 'make alpha' 'make no-such-target' '```' >>"${tree}/README.md"
  check
  assert_failure 1
  assert_line --regexp '^\(1\) README\.md:[0-9]+: make no-such-target '
}

@test "rule 1: the verb in plain prose is never a citation" {
  append README.md 'Then make sure that you make nothing-here and run make again.'
  check
  assert_success
}

# --- rule (2): service commands and slash commands -------------------------------------------------------------

@test "rule 2: a cited service command that the help does not list is reported" {
  append README.md <<'EOF'
Run `service no-such-command` now.
EOF
  check
  assert_failure 1
  assert_line --regexp '^\(2\) README\.md:[0-9]+: service no-such-command '
}

@test "rule 2: a cited subcommand that the help does not list is reported" {
  append README.md <<'EOF'
Run `service migrate no-such-subcommand` now.
EOF
  check
  assert_failure 1
  assert_line --regexp '^\(2\) README\.md:[0-9]+: service migrate no-such-subcommand '
}

@test "rule 2: a command after service -- is checked" {
  # The valid tree already cites `cargo run -p service -- serve`, which passes.
  append README.md <<'EOF'
Run `cargo run -p service -- no-such-command` or `cargo run --quiet -p service -- migrate no-such-subcommand`.
EOF
  check
  assert_failure 1
  assert_line --regexp '^\(2\) README\.md:[0-9]+: service no-such-command '
  assert_line --regexp '^\(2\) README\.md:[0-9]+: service migrate no-such-subcommand '
  refute_output --partial 'service serve'
}

@test "rule 2: after -p, --package or --bin the command is read only after --, and service as program is still read" {
  append README.md <<'EOF'
Run `cargo nextest run -p service --test integration` and `cargo test -p service --lib config`.
Run `cargo test --package service --test smoke` and `cargo build --bin service --features extra`.
Run `cargo run -p service -- --quiet other-missing` and `service no-such-command`.
EOF
  check
  assert_failure 1
  assert_line --regexp '^\(2\) README\.md:[0-9]+: service other-missing '
  assert_line --regexp '^\(2\) README\.md:[0-9]+: service no-such-command '
  assert_equal "${#lines[@]}" 2
}

@test "rule 2: a slash command without a file under .claude/commands is reported" {
  append README.md <<'EOF'
Start with `/no-such-command` and then `/dev`.
EOF
  check
  assert_failure 1
  assert_line --regexp '^\(2\) README\.md:[0-9]+: slash command /no-such-command has no file \.claude/commands/no-such-command\.md'
  refute_output --partial '/dev '
}

@test "rule 2: a path that another tracked file uses is not a slash command" {
  append README.md <<'EOF'
The route `/ping` and the mount `/app` answer.
EOF
  printf 'WORKDIR /app\n' >"${tree}/Dockerfile"
  check
  assert_success
}

@test "rule 2: a slash command is reported although no other tracked file contains a path" {
  edit_file "${tree}/docker-compose.yml" sed 's|http://localhost:8081/ping|true|'
  append README.md <<'EOF'
Use `/no-such-cmd` here.
EOF
  check
  assert_failure 1
  assert_line --regexp '^\(2\) README\.md:[0-9]+: slash command /no-such-cmd has no file '
}

@test "rule 2: a name that only a test file of the generated tree uses as a path is reported" {
  mkdir -p "${tree}/crates/service/tests"
  printf 'const HELP: &str = "http://localhost/help";\n' >"${tree}/crates/service/tests/help.rs"
  append README.md <<'EOF'
Ask `/help` for the list.
EOF
  check
  assert_failure 1
  assert_line --regexp '^\(2\) README\.md:[0-9]+: slash command /help has no file '
}

@test "rule 2: /ping is accepted without any other file, and so is the first segment of an OpenAPI path" {
  edit_file "${tree}/docker-compose.yml" sed 's|http://localhost:8081/ping|true|'
  mkdir -p "${tree}/.github/openapi-spec"
  printf 'openapi: 3.1.0\npaths:\n  /v2/widgets:\n    get: {}\n  /v2/widgets/{id}:\n    get: {}\n' >"${tree}/.github/openapi-spec/spec.yaml"
  append README.md <<'EOF'
The routes `/ping` and `/v2/widgets` answer.
EOF
  check
  assert_success
}

# --- rule (3): paths -------------------------------------------------------------------------------------------

@test "rule 3: a backticked path under a tracked directory that does not exist is reported" {
  append README.md <<'EOF'
See `docs/missing.md`.
EOF
  check
  assert_failure 1
  assert_line --regexp '^\(3\) README\.md:[0-9]+: docs/missing\.md '
}

@test "rule 3: a path of a tracked top-level file is checked, a prose word is not" {
  append README.md <<'EOF'
See `AGENTS.md#verification`, `Makefile:3` and `./docs/environment.md`, and the word docs in prose.
EOF
  check
  assert_success
  append README.md <<'EOF'
See `Makefile/none` and `.cargo/missing.toml`.
EOF
  check
  assert_failure 1
  assert_line --regexp '^\(3\) README\.md:[0-9]+: Makefile/none '
  assert_line --regexp '^\(3\) README\.md:[0-9]+: \.cargo/missing\.toml '
}

@test "rule 3: the optional sync guide named by its bare file name is not reported, its full path is" {
  append README.md <<'EOF'
The guide `TEMPLATE_SYNC_PAT.md` and the workflow `template-sync-pat.yml` of the other flavour.
EOF
  check
  assert_success
  mkdir -p "${tree}/.github"
  printf 'x\n' >"${tree}/.github/TEMPLATE_SYNC_APP.md"
  append README.md <<'EOF'
The guide `.github/TEMPLATE_SYNC_PAT.md` of the other flavour.
EOF
  check
  assert_failure 1
  assert_line --regexp '^\(3\) README\.md:[0-9]+: \.github/TEMPLATE_SYNC_PAT\.md '
}

@test "rule 3: git-ignored paths and tokens with a placeholder or wildcard are not reported" {
  append README.md <<'EOF'
The state is in `_bmad/`, `_bmad/core/agent.md`, `target/coverage/html`, `specs/<bundle-id>/spec.md`, `docs/*.md`, `docs/make-targets-*.md` and `.claude/skills/*/SKILL.md`.
EOF
  check
  assert_success
}

@test "rule 3: a token is checked up to its first placeholder" {
  append README.md <<'EOF'
Run `docs/<name>/missing.md` and `docs/environment.md`.
EOF
  check
  assert_success
  append README.md <<'EOF'
See `docs/absent/<name>.md`.
EOF
  check
  assert_failure 1
  assert_line --regexp '^\(3\) README\.md:[0-9]+: docs/absent/ '
}

@test "rule 3: a spelled-out citation inside a longer command is found" {
  mkdir -p "${tree}/tests"
  printf 'kept so that tests/ is a tracked directory\n' >"${tree}/tests/keep.txt"
  append README.md <<'EOF'
Run `make alpha BATS_PATH=tests/CLI/bats/check_docs.bats`.
EOF
  check
  assert_failure 1
  assert_line --regexp '^\(3\) README\.md:[0-9]+: tests/CLI/bats/check_docs\.bats '
}

# --- rule (4): the verification list ---------------------------------------------------------------------------

@test "rule 4: a verification block that differs from make ci-list is reported" {
  edit_file "${tree}/AGENTS.md" sed '/^make gamma$/d'
  check
  assert_failure 1
  assert_line --regexp '^\(4\) AGENTS\.md: the verification block differs from the output of make ci-list'
  assert_line --regexp '^\(4\) AGENTS\.md:   \+?-gamma$'
}

@test "rule 4: missing markers are reported" {
  edit_file "${tree}/AGENTS.md" sed '/verification:start/d'
  check
  assert_failure 1
  assert_line --regexp '^\(4\) AGENTS\.md: the markers '
}

@test "rule 4: a second file with a fenced block of three gate targets is reported" {
  printf '%s\n' '# Guide' '' '```bash' 'make alpha' 'make beta' 'make gamma' '```' >"${tree}/docs/guide.md"
  check
  assert_failure 1
  assert_line --regexp '^\(4\) docs/guide\.md:3: a fenced block cites 3 gate targets '
}

@test "rule 4: the markers in another file do not exempt a block" {
  printf '%s\n' '# Guide' '' '<!-- verification:start -->' '' '```bash' 'make alpha' 'make beta' 'make gamma' '```' '' \
    '<!-- verification:end -->' >"${tree}/docs/guide.md"
  check
  assert_failure 1
  assert_line --regexp '^\(4\) docs/guide\.md:5: a fenced block cites 3 gate targets '
  refute_line --regexp '^\(4\) AGENTS\.md'
}

@test "rule 4: a block with two gate targets and a numbered list of three are told apart" {
  cat >"${tree}/docs/guide.md" <<'EOF'
# Guide

```bash
make alpha
make beta
make help
```

1. Run `make alpha`.
2. Run `make beta`.
3. Run `make gamma`.
EOF
  check
  assert_failure 1
  assert_line --regexp '^\(4\) docs/guide\.md:9: a numbered list cites 3 gate targets '
  refute_output --partial 'fenced block'
}

@test "rule 4: two separate numbered lists of two gate targets pass" {
  cat >"${tree}/docs/guide.md" <<'EOF'
# Guide

1. Run `make alpha`.
2. Run `make beta`.

Prose in between.

1. Run `make gamma`.
2. Run `make help`.
EOF
  check
  assert_success
}

# --- rule (5): the environment reference -----------------------------------------------------------------------

@test "rule 5: a variable that a compose file interpolates and the reference lacks is reported" {
  append docker-compose.yml <<'EOF'
      NEW_SETTING: ${NEW_SETTING:-1}
EOF
  check
  assert_failure 1
  assert_line --regexp '^\(5\) docs/environment\.md: NEW_SETTING '
}

@test "rule 5: a variable that a compose secret reads from the environment needs a row" {
  append docker-compose.yml <<'EOF'
secrets:
  planted_token:
    environment: PLANTED_TOKEN
EOF
  check
  assert_failure 1
  assert_line --regexp '^\(5\) docs/environment\.md: PLANTED_TOKEN is read by '
  # With a row in the table of settings the tree passes again.
  edit_file "${tree}/docs/environment.md" awk '{print} /TEST_SEED/ {print "| \140PLANTED_TOKEN\140 | none | a compose secret |"}'
  check
  assert_success
}

@test "rule 5: each of the five sources is read" {
  printf '      --extra <EXTRA_FROM_HELP>  x [env: EXTRA_FROM_HELP]\n' >>"${help}/migrate.txt"
  append .env 'EXTRA_FROM_ENV=1'
  append .env <<'EOF'
DERIVED=${EXTRA_REFERENCED}
EOF
  append Makefile 'EXTRA_FROM_MAKE ?= 1'
  append .cargo/config.toml 'EXTRA_FROM_CARGO = "1"'
  check
  assert_failure 1
  for name in EXTRA_FROM_HELP EXTRA_FROM_ENV EXTRA_REFERENCED DERIVED EXTRA_FROM_MAKE EXTRA_FROM_CARGO; do
    assert_line --regexp "^\\(5\\) docs/environment\\.md: ${name} "
  done
}

@test "rule 5: a row that nothing reads is reported, a name in another kind of table is not" {
  append docs/environment.md <<'EOF'
| `STALE` | none | nowhere |
EOF
  append docs/environment.md <<'EOF'

## More names

| Name | Note |
| --- | --- |
| `ALSO_STALE` | the table is a table of names |
EOF
  check
  assert_failure 1
  assert_line --regexp '^\(5\) docs/environment\.md: ALSO_STALE has a row'
  refute_output --partial 'OLD_NAME'
}

# --- rule (6): targets, help and the reference -----------------------------------------------------------------

@test "rule 6: a target without a row in the table All targets and without a help line is reported" {
  append Makefile 'delta: ## Fourth target'
  append Makefile '> @true'
  check
  assert_failure 1
  assert_line --regexp '^\(6\) docs/make-targets\.md: the target delta has no row'
  assert_line --regexp '^\(6\) Makefile: the target delta is not listed by make help'
}

@test "rule 6: a row outside the table All targets does not count" {
  grep -v 'gamma' "${tree}/docs/make-targets.md" >"${tmp}/targets.md"
  cp "${tmp}/targets.md" "${tree}/docs/make-targets.md"
  append docs/make-targets.md <<'EOF'
| `gamma` | listed in the wrong section |
EOF
  check
  assert_failure 1
  assert_line --regexp '^\(6\) docs/make-targets\.md: the target gamma has no row'
}

@test "rule 6: a target of make/service.mk is accepted with a row in docs/make-targets-service.md" {
  mkdir -p "${tree}/make"
  printf '%s\n' '.PHONY: omega' 'LISTED += omega' 'omega: ## Service target' '> @true' >"${tree}/make/service.mk"
  printf '%s\n' '-include make/service.mk' >>"${tree}/Makefile"
  check
  assert_failure 1
  assert_line --regexp '^\(6\) docs/make-targets-service\.md: the target omega of make/service\.mk has no row'
  cat >"${tree}/docs/make-targets-service.md" <<'EOF'
# Service targets

| Target | Help text |
| --- | --- |
| `omega` | Service target |
EOF
  check
  assert_success
}

# --- rule (7): skills ----------------------------------------------------------------------------------------

@test "rule 7: a skill whose name differs from its directory is reported" {
  edit_file "${tree}/.claude/skills/demo-skill/SKILL.md" sed 's/^name: demo-skill$/name: other-skill/'
  check
  assert_failure 1
  assert_line --regexp "^\\(7\\) \\.claude/skills/demo-skill/SKILL\\.md: the name is 'other-skill' but the directory is demo-skill"
}

@test "rule 7: a skill without a description is reported" {
  edit_file "${tree}/.claude/skills/demo-skill/SKILL.md" sed '/^description:/d'
  check
  assert_failure 1
  assert_line --regexp '^\(7\) \.claude/skills/demo-skill/SKILL\.md: the description is missing'
}

@test "rule 7: a skill without an entry in the index is reported" {
  mkdir -p "${tree}/.claude/skills/unlisted"
  printf '%s\n' '---' 'name: unlisted' 'description: Use when nothing lists the skill.' '---' '' '# Unlisted' >"${tree}/.claude/skills/unlisted/SKILL.md"
  check
  assert_failure 1
  assert_line --regexp '^\(7\) \.claude/skills/README\.md: the skill unlisted has no entry'
}

@test "rule 7: a skill directory without SKILL.md and a prefix of a listed name are reported" {
  mkdir -p "${tree}/.claude/skills/demo" "${tree}/.claude/skills/empty"
  printf '%s\n' '---' 'name: demo' 'description: Use when the index lists only a longer name.' '---' >"${tree}/.claude/skills/demo/SKILL.md"
  : >"${tree}/.claude/skills/empty/.gitkeep"
  check
  assert_failure 1
  assert_line --regexp '^\(7\) \.claude/skills/README\.md: the skill demo has no entry'
  assert_line --regexp '^\(7\) \.claude/skills/empty/SKILL\.md: the skill directory has no SKILL\.md'
}

# --- usage and could-not-run -----------------------------------------------------------------------------------

@test "the script exits 2 without --service-help, with an unknown option and with a missing help text" {
  run scripts/check-docs.sh --root "${tree}"
  assert_failure 2
  assert_output --partial 'usage: scripts/check-docs.sh'
  run scripts/check-docs.sh --root "${tree}" --service-help "${help}" --unknown
  assert_failure 2
  rm -f "${help}/serve.txt"
  run scripts/check-docs.sh --root "${tree}" --service-help "${help}"
  assert_failure 2
  assert_output --partial 'serve.txt is missing'
}

@test "the script exits 2 for a root without a Makefile" {
  rm -f "${tree}/Makefile"
  run scripts/check-docs.sh --root "${tree}" --service-help "${help}"
  assert_failure 2
  assert_output --partial 'check-docs could not run'
}

@test "the findings of several rules are printed together, ordered by rule" {
  append README.md <<'EOF'
Run `make no-such-target`, see `docs/missing.md` and run `service no-such-command`.
EOF
  check
  assert_failure 1
  assert_line --index 0 --regexp '^\(1\) '
  assert_line --index 1 --regexp '^\(2\) '
  assert_line --index 2 --regexp '^\(3\) '
}

@test "the checks leave the repository untouched" {
  local before hooks_before
  before="$(tree_status)"
  # The developer's own setting (make install-hooks sets it in the local configuration) is not under test: the
  # checks must leave it as it was.
  hooks_before="$(git config --get core.hooksPath || true)"
  check
  assert_success
  assert_equal "$(tree_status)" "${before}"
  assert_equal "$(git config --get core.hooksPath || true)" "${hooks_before}"
}
