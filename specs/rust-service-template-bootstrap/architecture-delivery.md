---
stepsCompleted: [1, 2, 3, 4, 5, 6, 7, 8]
inputDocuments:
  - _bmad-output/planning-artifacts/scope-decisions.md
  - _bmad-output/planning-artifacts/research/capability-catalog.md
  - _bmad-output/planning-artifacts/architecture.md
  - _bmad-output/planning-artifacts/product-brief.md
  - _bmad-output/planning-artifacts/prd.md
  - _bmad-output/planning-artifacts/research/rust-testing-delivery.md
  - _bmad-output/planning-artifacts/research/rust-quality-gates.md
  - _bmad-output/planning-artifacts/research/php-docker-make-load.md
  - _bmad-output/planning-artifacts/research/php-ci-github-contracts.md
  - _bmad-output/planning-artifacts/research/php-ai-docs-audit.md
workflowType: 'architecture'
project_name: 'rust-service-template'
user_name: 'BMad'
date: '2026-11-10'
lastStep: 8
status: 'complete'
completedAt: '2026-11-10'
companionDocument: '_bmad-output/planning-artifacts/architecture.md'
---

# Delivery Architecture Document

Delivery architecture of `rust-service-template`: the Makefile command surface, container images and compose files, CI/CD workflows, release and template sync, git hooks, load testing, Makefile tests, repository linting, the architecture model, AI guidance and human documentation. The core document `architecture.md` owns the Rust workspace, the runtime design and the Rust-side gate commands (QG-01 to QG-20). Every crate name, binary name, subcommand, cargo command, configuration file, environment variable and port used here is taken from it unchanged.

How to read this document:

- `scope-decisions.md` is binding; the final PRD comes next (scope 8.11), then the core document and this one, then the research. This document was revised after the validation round of 2026-11-10 together with the core document; no deviation from the PRD remains open here, and the measurement items and readings of requirement wording are listed under "Delivery Validation".
- "Verified" means run on 2026-11-10 (Linux x86_64, Docker Engine 29.9, Compose v5.6.1, GNU Make 4.3) for this document, by the validation round or by a research spike. "Linted" and "designed" mean not executed. The validation section lists which is which.
- Every name in `code font` is final. Gate ids `G1` to `G31` are the gate list of the final PRD; `QG-nn` are the core gate commands; `AD-nn` are core decisions; `DD-nn` are the decisions below.
- The repository name `VilnaCRM-Org/rust-service-template` may appear only in the places listed in DD-23. Everything else derives names at run time.

## Delivery Decisions

Priority: DD-01, DD-02, DD-08, DD-16, DD-19, DD-20 and DD-23 block implementation of everything else; the rest shape one area each.

### DD-01 Make execution model

- **Decision.** Every target runs its tools natively in the current environment by default. The opt-in variable is `RUN_IN_CONTAINER`: when it is `1` or `true`, targets of class TOOL (cargo, `sqlx`, `xtask`, the `service` binary, `taplo`, `typos`, `zizmor`) run as `docker compose exec -T --user <uid>:<gid> -e <forwarded variables> app <command>` in the development container. The exec line forwards the variables named in `EXEC_ENV` (`TEST_SEED`, `PROPTEST_RNG_SEED`, `CONFIG_CHANGE_APPROVED`, `CARGO_BUILD_WARNINGS`, `DATABASE_STATEMENT_TIMEOUT`) with `-e NAME`, because `docker compose exec` passes no host environment on its own; a variable the caller has not set stays unset in the container (both verified). The opt-in is ignored, and the command runs natively, when `CI` is non-empty (GitHub sets `CI=true`) or when `IN_DEV_CONTAINER=1` (set by the development override for the `app` container, so also in a `make sh` shell). Targets of class HOST drive Docker themselves (lifecycle, image-based tools, load tests, Bats) and always run where Make runs. The class of each target is in the target table.
- **On a GitHub runner** targets run natively: the toolchain comes from `rust-toolchain.toml`, backing services from `make start-deps`, and no job enters the development container.
- **Container mode needs no toolchain on the host** (NFR37: the supported path on Linux aarch64 and macOS). With the opt-in set and `CI` empty, `make doctor` skips the rustup, toolchain and cargo-tool checks, `make bootstrap` installs only Bats and the git hooks before it starts the environment, and no script that a recipe calls outside `$(EXEC)` starts cargo or a cargo tool itself (G31 rule (h)). Container mode needs the environment to be up (`make start`). Reports still land in the checkout: recipes write them to `target/coverage`, `target/sarif` and `target/reports` below the bind mount, while the build directory is a volume outside it (`/cargo-target`).
- **Environment of the development container.** The compose development override sets `DB_HOST=database`, `DB_PORT=5432`, `LOCALSTACK_HOST=localstack`, `LOCALSTACK_PORT=4566` (the ports inside the compose network, whatever host ports `.env` publishes; the `migrate` container gets `DB_HOST` and `DB_PORT` only) and `IN_DEV_CONTAINER=1`, and removes `APP_ENV`, `LOG_LEVEL`, `DATABASE_URL` and `AWS_ENDPOINT_URL_SQS` from the container environment, because an injected value would win over the env files (core "Committed environment files"). The repository is bind-mounted at `/app`, so the binary finds the committed `.env` and resolves `${DB_HOST}` from the container environment. Test targets wrap their command in `TEST_ENV`, which is `env -u DATABASE_URL scripts/with-env.sh .env .env.test --`: a `DATABASE_URL` of the caller is removed first, so the test targets always reach the test database of `.env.test`, which follows `DB_HOST`, `DB_PORT` and `DB_NAME`; the script then exports the variables of the files in order, a later file overriding an earlier one and a variable the caller has already set overriding both, and expands `${NAME}` references. The same line therefore yields `APP_ENV=test` with a database URL on `localhost` on a host or runner and on `database` in the container (verified with the two core files in all three places). The Makefile includes no environment file.
- **Rationale.** Scope 5 and 8.8; fixes PHP defects TQ:39 and DM:D-12 (`CI=1` only, 17 targets) and DM:D-13 (env file included wholesale).
- **Alternatives rejected.** Container by default (rebuilds an image in every CI job, PHP behaviour). Re-invoking `make` inside the container (needs the Makefile's host-only variables there). A second variable for CI (two switches to test).
- **Consequences.** `tests/CLI/bats/make_execution_mode.bats` covers native mode, the opt-in, the opt-in with `CI=true` and with `IN_DEV_CONTAINER=1`, the forwarded variables, the test environment inside the container and `doctor` without a host toolchain. Class HOST targets print nothing special in container mode; they are listed in the Make-target reference.

### DD-02 Content and order of `make ci`

- **Decision.** `make ci` (aliases `all`, `ci-sequential`) runs `doctor`, then seven stages in this order, stops at the first failure and prints `CI checks successfully passed!`. The target-specific line `ci: export CARGO_BUILD_WARNINGS ?= deny` gives a local run the setting that the CI toolchain action applies on a runner, so a local `make ci` is as strict as hosted CI (verified in GNU Make 4.3: set for every stage, absent when a single target is called, a caller's value kept). It runs every gate of the gate list that has a Make target: G1 to G27 and G29 to G31.

| # | Stage target | Targets in order | Gates |
| --- | --- | --- | --- |
| 1 | `ci-preflight` | `validate-configuration`, `validate-invariants`, `secrets-scan`, `lint-repo`, `lint-load-tests`, `validate-architecture-model` | G25, G31, G9, G23, G24, G27 |
| 2 | `ci-static-analysis` | `format-check`, `validate-manifest`, `check-crate-isolation`, `static-analysis`, `complexity`, `architecture`, `check-security`, `unused-dependencies`, `check-requirements`, `docs-check` | G1, G5, G29, G2, G3, G4, G7, G8, G6, G26 |
| 3 | `ci-tests` | `setup-test-db`, `validate-sqlx-metadata`, `tests-with-coverage`, `doc-tests`, `e2e-tests` | G30, G11, G12, G14, G13 |
| 4 | `ci-contracts` | `validate-contract-snapshots`, `validate-openapi-spec`, `openapi-diff`, `graphql-diff` | G17, G18, G19, G20 |
| 5 | `ci-mutation` | `mutation-tests-diff` | G15 (changed code) |
| 6 | `ci-delivery` | `build-prod-image`, `image-scan`, `schemathesis-validate`, `smoke-load-tests` | G10, G21, G22 |
| 7 | `ci-cli` | `bats` | G16 |

- **One execution of the suites.** `tests-with-coverage` runs the unit and integration suites once under instrumentation and reports G11, G12 and G14; `doc-tests` adds the documentation tests of G11. The end-to-end suite runs once, as G13, without instrumentation, and does not count towards coverage (PD-28). No stage runs `unit-tests` or `integration-tests` a second time.
- **Backing services.** Every target that needs PostgreSQL or LocalStack starts what it uses through its prerequisites (`ensure-database` for the database alone, `ensure-deps` for both, `setup-test-db`) with `docker compose up --no-recreate`, so a service that already runs is never stopped or altered (FR66(c)) and a target starts only the services it uses (NFR41).
- **Stage 7 holds no second passing run of a gate.** The Bats suite keeps the negative test of every gate and the tests of the targets that are not gates. The passing run of a gate target is stages 1 to 6 of the same `make ci` and the gate's own CI job, so the suite does not repeat it; the Make-target reference states this for each gate target (FR67(a)). The estimate per stage is under "Delivery Validation".
- **CI-only steps, with reasons.** G28 pull-request title (needs the pull request); evaluation of the two approval labels (local runs read `CONFIG_CHANGE_APPROVED` and `BREAKING_CHANGE_APPROVED` from the environment and otherwise fail and print the label name); CodeQL analysis and the SARIF upload (publications without a threshold of their own, PD-22; no local runner exists); the Codecov upload and artifact uploads (publication); the full sharded mutation run (`make mutation-tests` exists locally but is not part of `make ci`, scope 8.5); the build of the development container definition in the scheduled workflow and the checks run in it (`npx --yes @devcontainers/cli@<DEVCONTAINER_CLI_VERSION> build --workspace-folder .`, then `up --workspace-folder .` and `exec --workspace-folder .` with `docker info`, `make format-check` and `make start-deps`: it needs the devcontainers CLI through npx, builds an image for several minutes, FR95 and FR118 ask for it on a schedule only and no gate of `make ci` covers it); the stop and the timed start of the development stack in the scheduled clean-clone job (`make stop`, `make start`: the hosted measurement of the `make start` budget of NFR35, which no gate of `make ci` performs).
- **Rationale.** Scope principle 4, FR66; fixes TQ:1, DM:D-01 and CI:D-24. Cheap gates come first; the stage that needs Docker images comes late.
- **Alternatives rejected.** Leaving Bats, Schemathesis and the smoke load test out (PHP behaviour; the local list would differ from CI). Parallel stages (`make -j`): interleaved output and shared `target/` locks.
- **Consequences.** `make ci` needs Docker, network access and a fetched `origin/main`. `CI_STAGES` (default: the seven stages) exists so that Bats can test the aggregate without running it; it lives in the protected fragment `make/gates.mk`. `scripts/check-invariants.sh` compares the gate targets of `make ci` with the targets the workflows run (G31 rule (d), NFR22).

### DD-03 Fix-style and check-style targets

- **Decision.** Every gate is check-style: it changes no tracked file and exits non-zero on a finding. Fix-style targets are separate, are never called by `make ci`, a hook or a workflow, and are: `format` (`cargo fmt --all`), `format-repo` (taplo and Prettier in write mode), `static-analysis-fix` (clippy `--fix`, then `cargo fmt --all`), `unused-dependencies-fix` (`cargo shear --fix`), `sqlx-prepare` (regenerates `.sqlx/`), `generate-openapi-spec` and `generate-graphql-spec` (rewrite the snapshots).
- **Rationale.** FR40, FR42; fixes TQ:4, DM:D-02, CI:D-03 (a formatter gate that always fixed and never failed).
- **Alternatives rejected.** One target with a mode variable (PHP defect TQ:37: the variable never reached the tool).
- **Consequences.** `validate-contract-snapshots` regenerates the two snapshots in place, as QG-19 prescribes; on a clean tree this is a no-op, and a difference or an untracked snapshot is the finding (`git status --porcelain` also sees a snapshot that was never committed).

### DD-04 Tool installation on a host

- **Decision.** `make install` runs `scripts/install-tools.sh`, `scripts/install-bats.sh` and `cargo fetch --locked`. Tool versions and tool image references live in one protected fragment, `make/tool-versions.mk`. Cargo-installable tools (`cargo-nextest`, `cargo-llvm-cov`, `cargo-mutants`, `cargo-deny`, `cargo-shear`, `cargo-hack`, `sqlx-cli`, `clippy-sarif`, `taplo-cli`, `typos-cli`, `zizmor`, `cocogitto`, and `watchexec-cli` in the development image) are installed with `cargo-binstall` 1.26.2, fetched as a release archive by URL and checked against a recorded SHA-256 (recorded for Linux x86_64 and aarch64 only; on any other platform the script says so and uses `cargo install`), with `--disable-strategies compile`; when no prebuilt binary exists the script says so and runs `cargo install --locked --version <v>` (`sqlx-cli` without `--locked` and with `--no-default-features --features postgres,rustls`, as the core document requires). A tool already present at the pinned version is skipped. Every other tool runs from a pinned image (DD-22). The development image runs the same script at build time.
- **Rationale.** Scope 8.8; research finding 6 (binstall silently compiles when the GitHub API is unreachable, so the fallback is explicit here).
- **Alternatives rejected.** `cargo install` for everything (2 to 3 minutes per tool). A second version list in the Dockerfile (drift).
- **Consequences.** CI installs the same versions with `taiki-e/install-action` (scope 8.9) and `fallback: none`, so a version in the fragment may be raised only to one that `manifests/<tool>.json` of the pinned action commit lists; otherwise the action is bumped first (verified: the pinned commit lists every cargo tool at its pinned version, and `typos` up to 1.52.0). `scripts/check-invariants.sh` fails when a `tool:` entry in a workflow differs from `make/tool-versions.mk`. Every line of the fragment is `export NAME := value`: recipes and scripts read the values from the environment, and the three scripts that also run outside Make (`install-tools.sh`, `install-bats.sh`, `tests/Load/execute-load-test.sh`) read the file with that line format. Dependabot cannot update this fragment; it is reviewed with each toolchain update, and `docs/operations.md` holds the procedure.

### DD-05 Structurizr viewer: profile, on by default locally

- **Decision.** The `structurizr` service is in the compose profile `docs`. The Makefile exports `COMPOSE_PROFILES=docs` unless `CI` is non-empty or the caller sets `COMPOSE_PROFILES` (an empty value opts out). `make start` therefore starts the viewer on a developer machine and not on a runner. The service uses `structurizr/structurizr:2026.10.26` with `command: local`, mounts `./workspace.dsl` read-only and keeps its generated files in the named volume `structurizr_data`.
- **Rationale.** Scope 3.1 keeps the viewer in the development stack; research D9 (428 MiB, no shell for a healthcheck, started in every PHP CI job). Verified here: with the file mount and the volume the diagrams page answers 200 after about 21 s and nothing is written into the repository, which removes APP:D-29 and DM:D-36 while `workspace.dsl` stays at the repository root.
- **Alternatives rejected.** Always on (cost in every CI job). Moving the model to `docs/architecture/` (the root path is a preserved contract).
- **Consequences.** `docker compose up` without Make does not start the viewer; `docs/getting-started.md` says so.

### DD-06 Bats provisioning

- **Decision.** One script, `scripts/install-bats.sh [--prefix <dir>]`, clones four repositories at pinned tags, verifies each commit SHA and installs them under `.tools/bats/` (git-ignored): `core/` (bats-core) and `lib/bats-support`, `lib/bats-assert`, `lib/bats-file`. `make install`, the CI Bats job and the development image (prefix `/opt/bats`) all use it. `make bats` runs `.tools/bats/core/bin/bats` when present, else `bats` from `PATH`, with `BATS_LIB_PATH` set to `.tools/bats/lib:/opt/bats/lib:/usr/lib/bats`. Helpers are loaded with `bats_load_library`.

| Repository | Tag | Commit SHA |
| --- | --- | --- |
| `bats-core/bats-core` | v1.14.0 | `eb7f42f8d608ac693d7a4b67474f6714ea68cfc5` |
| `bats-core/bats-support` | v0.3.0 | `24a72e14349690bcbf7c151b9d2d1cdd32d36eb1` |
| `bats-core/bats-assert` | v2.2.4 | `f1e9280eaae8f86cbe278a687e6ba755bc802c1a` |
| `bats-core/bats-file` | v0.4.0 | `13ad5e2ffcc360281432db3d43a306f7b3667d60` |

- **Rationale.** Scope 4 and 8.6 (no submodules); one version everywhere instead of 1.14.0 in CI and 1.11.1 from Debian packages; one third-party action fewer.
- **Alternatives rejected.** `bats-core/bats-action` (CI-only, cannot be tested locally). Committed copies (drift; every linter must exclude them). npm packages (no `bats-file`, needs Node on the host).
- **Consequences.** The four SHAs are in `make/tool-versions.mk`.

### DD-07 Negative Bats tests

- **Decision.** Every gate except G28 (G1 to G27 and G29 to G31) has a negative Bats test that plants a violation, invokes the gate's Make target and asserts a non-zero exit and the gate's own diagnostic; G28 has tests with matching and non-matching titles. Compile-heavy negatives plant their violation in the leaf crate `xtask`, which no other crate depends on and which every downstream service keeps, so only that crate is rebuilt (G1, G2, G3, G8, G11, G12, G14, G15, G29). Three properties of the plants are fixed by measurements. A plant for G3 or G14 is a plain function, never the body of an `#[async_trait]` method: neither the complexity lints nor line coverage see such a body, so a violation planted there is not detected (core AD-24, AD-27). The plant for G15 is `fn planted(flag: bool) -> bool { flag }` and its test runs with `CI=true`, so a mutation gate emptied by `CARGO_BUILD_WARNINGS=deny` would be noticed. The plant for G29 uses `serde_json::value::RawValue`, which the workspace build supplies through feature unification and `xtask` alone does not have (verified on the skeleton: `cargo check --workspace` passes, `cargo check -p xtask` fails with `cannot find type RawValue`). The plant is the function `planted_raw_value(raw: &serde_json::value::RawValue) -> usize` followed by the line `const _: fn(&serde_json::value::RawValue) -> usize = planted_raw_value;`, which uses the function without a lint attribute: under `CARGO_BUILD_WARNINGS=deny`, which the hosted runner sets, an unused planted function would make the precondition `cargo check --locked --workspace` fail. The architecture negative uses the checker's own fixture workspace through `ARCH_ARGS`; the base-comparing gates run with `base_ref=HEAD`, so they also fail correctly on the bootstrap pull request. The list of tests is in "CLI Tests (Bats)".
- **Rationale.** NFR21, scope 8.6; research D5 (a planted file counts only if the compiler reaches it; append to a module, restore in teardown).
- **Alternatives rejected.** A broken file in a shared crate (recompiles the workspace for every test). Fixture workspaces for clippy and tests (would need a copy of the lint table). A package filter such as `-p xtask` on the gate commands (verified: cargo rejects `--lib` for a package that has only a binary target).
- **Consequences.** `ARCH_ARGS`, `DENY_ARGS`, `BATS_PATH`, `DOCKERFILES` and `IMAGE_TO_SCAN` are test hooks defined in the protected fragment. Gates that combine tools or policies get one negative test per tool or policy (each linter of G23; advisory, licence and source for G7; each snapshot for G17). The rule kinds of the template-owned shell checkers (G26, G27, G31) are tested one by one at the checker's own level, on small trees generated in the test (NFR21, PD-26).

### DD-08 Workflow layout

- **Decision.** Eleven workflow files. Gates are grouped by what they share (cache, services, trigger types) and every gate is its own job and therefore its own named status check; the one exception is the job `tests`, which executes the unit and integration suites once and reports G11, G12 and G14, naming the failing gate in its job summary (FR93(b)). Steps are written inline: no composite action and no reusable workflow. Each pull-request workflow starts with a `changes` job that runs `scripts/ci/detect-changes.sh`; every job except those of G9, G23, G25, G26, G28 and G31 carries `if: needs.changes.outputs.code == 'true'`, so a documentation-only pull request skips them and a skipped job counts as a passing check (FR94). Jobs whose result depends on labels or on the title live in `change-control.yml`, the only workflow that also triggers on `edited`, `labeled` and `unlabeled`.
- **Rationale.** FR93, FR94, NFR39 to NFR41; fixes CI:D-16, CI:D-20, CI:D-21. Path filters on the workflow would leave required checks pending; a skipped job does not.
- **Alternatives rejected.** One workflow per gate (31 files, no shared trigger logic). One job with many steps (one status for many gates). `dorny/paths-filter` (a third-party action for 20 lines of shell).
- **Consequences.** Matrix jobs (the mutation shards, the six Bats groups) have an aggregator job with a fixed name that is the required check. The `changes` job of each pull-request workflow is a required check too, under a name of its own (`Changes (Quality)`, `Changes (Tests)`, `Changes (Contracts)`, `Changes (Change control)`, `Changes (Load tests)`, `Changes (Security)`, `Changes (Repository)`): when `changes` fails or is cancelled, every code job that needs it is skipped, and a skipped job counts as passing, so the red `changes` check is the only signal that blocks the merge. The names differ because a required check is matched by name. `docs/repository-settings.md` lists the required check names, and G31 rule (d) fails when a job name of a pull-request workflow is missing from it.

### DD-09 Mutation testing cadence

- **Decision.** Pull requests and `make ci` run `make mutation-tests-diff` (QG-18); pushes to `main` and the weekly scheduled run execute `make mutation-tests` (QG-17). In CI a planning job counts the mutants (`scripts/ci/mutation-plan.sh`: the mutants of the diff on a pull request, of the whole scope otherwise) and emits one shard per 150 mutants, at least 1 and at most 8, as a matrix of `{index, shard}` objects: `index` counts from 1 to n, and `shard` is the `--shard` value `<index-1>/<n>`, because cargo-mutants numbers shards from 0 to n-1. With 0 mutants the shard job is skipped and the aggregator passes. `make mutation-tests` without a shard runs everything locally.
- **Rationale.** Scope 8.5 and 8.11, NFR20, PD-30. Mutants are judged by the unit and integration tests of their crate (core AD-24), measured at a mean of 5.2 s per mutant after a baseline of about 1.5 min, so a shard of 150 mutants takes about 15 min on the measuring machine and leaves room under the 45-minute limit for a slower hosted runner and for unbalanced shards. The bootstrap pull request changes every file: its diff holds every mutant (800 to 1000 by the core estimate) and gets 6 to 8 shards.
- **Alternatives rejected.** Full run on every pull request (60 to 80 minutes unsharded). Diff run only (survivors outside the diff are never found). A fixed plan of one or four shards (in the first version of this document: four shards of the bootstrap diff were estimated at 28 to 38 minutes each, too close to the limit).
- **Consequences.** `scripts/mutation-diff.sh` takes the diff from the merge base to the working tree, so uncommitted changes of tracked files are judged too (QG-18); it exits 2 when the base ref cannot be resolved and 0 when the diff holds no production Rust change (the script then prints `mutation-tests-diff: the diff against <ref> holds no production Rust change; there are no mutants to test` on standard error and does not start cargo-mutants; with `--list` it prints `0`). The first full run on `main` records the number of mutants, the seconds per mutant and the shard times in `docs/operations.md`, and the divisor 150 is revised from that measurement.

### DD-10 Snapshot drift policy

- **Decision.** `.github/openapi-spec/spec.yaml` and `.github/graphql-spec/spec` are regenerated only by a developer (`make generate-openapi-spec`, `make generate-graphql-spec`) and committed with the code change. CI job `contract-snapshots` runs `make validate-contract-snapshots` and fails on any byte of difference. The lint and diff gates read the committed files and need no Rust build. No workflow writes to a branch.
- **Rationale.** Scope 3.1, FR73; fixes CI:D-09 and CI:D-10.
- **Alternatives rejected.** Auto-commit (write token on pull requests, impossible for forks).
- **Consequences.** The diff gates treat a missing snapshot on the base ref as "first version" and pass with a notice; an unresolvable base ref is exit status 2.

### DD-11 Release flow

- **Decision.** `release.yml` runs on every push to `main` and executes `scripts/release.sh`, which uses cocogitto 7.0.0: version from conventional commits, `CHANGELOG.md` through a custom template, `Cargo.toml` and `Cargo.lock` through the pre-bump hook `scripts/set-version.sh`, then commit, tag `vX.Y.Z`, push and `gh release create`. cocogitto names its commit `chore(version): vX.Y.Z`, which is not configurable; the script amends it to `chore(release): vX.Y.Z [skip ci]` and moves the tag before pushing. All earlier releases stay in the changelog. The identity is the GitHub App of `VILNACRM_APP_ID` and `VILNACRM_APP_PRIVATE_KEY`.
- **Rationale.** Scope 8.9, FR98; research D11. Verified here in a scratch repository: `cog bump --auto --dry-run` prints `vX.Y.Z` or a "no bump" sentence with status 0; the custom template renders `### Features` and `### Bug Fixes`; the amend-and-retag sequence leaves one commit and one tag; a commit with a non-conventional subject does not stop the bump; `build(deps)` commits cause no bump; a breaking change below `1.0.0` is ignored by cocogitto itself (see Consequences).
- **Alternatives rejected.** `TriPSs/conventional-changelog-action` (unverified TOML rewrite). Release pull requests (changes the organisation's flow). Keeping `chore(version)` (breaks the preserved commit contract).
- **Consequences.** cocogitto 7.0.0 ignores a breaking change while the version is `0.y.z` (observed: a `fix` with the footer gives `0.1.1`, a `docs` or `refactor` commit with the footer gives no release; from `1.0.0` on it gives the major bump) and marks a commit breaking for any line of the body that starts with the token, also inside a paragraph or indented (observed from `1.0.0`: `2.0.0` for both), wider than the conventional-commit definition. So `scripts/release.sh` decides on breaking changes itself, among the commits cocogitto reads in the range of `cog bump --auto` (`cog changelog <range> --template <file>`, a template that prints the id of every listed commit and whether cocogitto marks it breaking; the range is the highest `vX.Y.Z` tag to `HEAD`, or the whole history of `HEAD`, the oldest commit included, when there is none; merge commits dropped): a commit cocogitto marks is breaking when its subject has `!` before the colon or its footer section (the run of paragraphs at the end of the message, after the subject, that each start with a token and `: ` or ` #`) holds a line that starts with `BREAKING CHANGE: ` or `BREAKING-CHANGE: ` and a value. With such a commit it runs `cog bump --minor` while the latest tag is `0.y.z` and `cog bump --major` from `1.0.0` on; a breaking commit of any type is released, and cocogitto lists it in the changelog under its type, marked **BREAKING**, even for a type that is otherwise omitted. From `1.0.0` on, when cocogitto marks a commit that the rule does not count, it runs `cog bump --minor` when a `feat` is among the commits read, else `cog bump --patch` when a `fix` is, and otherwise makes no release; cocogitto still lists and marks such a commit **BREAKING** in the changelog (its own reading renders the template); `1.0.0` is cut once by a maintainer through the manual run (`workflow_dispatch` input `version`, which runs `scripts/release.sh --version 1.0.0`). The first release is the manifest version: with no release tag `vX.Y.Z` cocogitto derives `v0.1.0` from a `feat` and `v0.0.1` from a `fix` (both observed), so `scripts/release.sh` passes `--version <manifest version>` to the first bump. A prerelease tag such as `v0.1.0-rc.1` is no release: the script reads the latest tag as the highest `vX.Y.Z` tag, and while there is none the next release is the first one. `.github/AUTORELEASE.md` states these rules (FR98(e)). The changelog template prints no links, so `cog.toml` holds no repository name and syncs downstream unchanged.

### DD-12 Dependabot commit prefixes

- **Decision.** `build` for cargo, docker and docker-compose; `ci` for github-actions; `chore` for rust-toolchain and the npm package; all with `include: scope` (for example `build(deps): bump …`). None of these types bumps a version.
- **Rationale.** Scope 8.9; fixes CI:D-12 (`feat` cut a minor release per bump).
- **Alternatives rejected.** `fix` for cargo (a patch release per weekly bump, with nothing but a tag to show for it).
- **Consequences.** Dependency updates are released with the next `feat` or `fix`. Dependabot titles do not match the VilnaCRM form and are exempt from the title check (DD-15).

### DD-13 Behaviour when secrets are absent

- **Decision.** No step reads a secret in an `if:`. The first step of a job that needs an optional secret maps it to an environment variable, writes `available=true` or a `::notice::` to the log and sets a step output; later steps are conditional on that output. This applies to `CODECOV_TOKEN` (coverage upload skipped), `VILNACRM_APP_ID` and `VILNACRM_APP_PRIVATE_KEY` (release and App sync skipped) and `PERSONAL_ACCESS_TOKEN` (PAT sync skipped). The two code-scanning publications follow one rule (step block D): skipped with a notice on fork and Dependabot pull requests; otherwise published when the repository variable `CODE_SCANNING_ENABLED` is `true`, skipped with a notice when it is `false`, and, when it is unset, published only in a public repository, where code scanning is always available. A private repository without code scanning therefore needs no setting to stay green (FR49).
- **Rationale.** FR96, NFR8; fixes AD:D-AD-31 (release failed on every push without the App secrets).
- **Alternatives rejected.** `continue-on-error` (a red step that is "expected" hides real failures).
- **Consequences.** A job in which everything was skipped is green and says why in its log and summary.

### DD-14 Template sync

- **Decision.** Two workflows, `template-sync-app.yml` (weekly, App token) and `template-sync-pat.yml` (monthly, PAT), both of which compare `$GITHUB_REPOSITORY` with their sync source in the first step and end with a notice when they are equal, so they do nothing in the template itself. The source is defined once per file, as workflow `env: TEMPLATE_REPOSITORY`, and is the only place a workflow contains the repository name. The PAT workflow also skips when the App secrets exist. Both set `pr_title: 'chore(template): merge template changes'`, `pr_labels: template_sync`, `is_force_push_pr: true`, `is_pr_cleanup: true` and `is_keep_branch_on_pr_cleanup: true`, and read `.templatesyncignore`. A run for the template commit of the open sync pull request force-pushes its branch and updates that pull request in place. A run for a newer template commit closes the older open `template_sync` pull requests, keeping their branches, and then opens the pull request of its new branch; commits that a maintainer pushed to a closed sync branch stay on it and are not carried to the new branch. At most one sync pull request is therefore open (FR100(b), reading D-B28-5).
- **Rationale.** Scope 8.9, FR100, PD-14; fixes CI:D-13.
- **Alternatives rejected.** One workflow only (scope keeps both flavours). A repository variable for the guard (one more setting to forget). A job-level `if:` with the literal name (a second occurrence of the name in the file).
- **Consequences.** A sync pull request that changes a protected file needs the label `approved-config-change` like any other.

### DD-15 Pull-request title check

- **Decision.** Job `title` in `change-control.yml` runs `make check-commit-message` with the title in the environment variable `COMMIT_TITLE`, the same script as the `commit-msg` hook. It is skipped for pull requests authored by `dependabot[bot]` and for pull requests that carry the label `template_sync` (FR102). The release identity pushes directly and opens no pull request.
- **Rationale.** Scope 3.1, FR102; fixes TQ:8. Squash merges turn the title into the commit subject that the release reads.
- **Alternatives rejected.** `amannn/action-semantic-pull-request` (cannot express the mandatory `(#issue)` scope without a custom regex; adds an action with `pull_request_target` examples).
- **Consequences.** The title is passed through `env:` and read by the script from the environment; it is never interpolated into a workflow script or a Make recipe.

### DD-16 Protected-configuration guard in CI

- **Decision.** Job `validate-configuration` in `change-control.yml` checks out with full history and runs `make validate-configuration base_ref=origin/<base branch>` with `CONFIG_CHANGE_APPROVED` set to `true` only when the pull request carries `approved-config-change`; adding or removing the label re-runs the job without a new commit. Outside CI the guard fails on a modification and prints the label name unless the caller sets `CONFIG_CHANGE_APPROVED=true`. This document appends to `protected-config.toml` of the core document:
  - to `files`: `make/gates.mk`, `make/tool-versions.mk`, `.githooks/commit-msg`, `.githooks/pre-commit`, `.githooks/pre-push`, `scripts/with-env.sh`, `scripts/check-commit-message.sh`, `scripts/check-docs.sh`, `scripts/check-invariants.sh`, `scripts/mutation-diff.sh`, `scripts/setup-test-db.sh`, `scripts/check-sqlx-metadata.sh`, `scripts/validate-openapi-spec.sh`, `scripts/openapi-diff.sh`, `scripts/graphql-diff.sh`, `scripts/with-load-test-stack.sh`, `scripts/secrets-scan.sh`, `scripts/image-scan.sh`, `scripts/validate-architecture-model.sh`, `scripts/claude/guard-protected-files.sh`, `tests/Load/eslint.config.js`, `tests/Load/utils/thresholdsBuilder.js` (the `rate>0.99` threshold of G22, NFR21), `.gitleaks.toml`, `.gitleaksignore`, `.trivyignore`, `.yamllint.yml`, `.markdownlint-cli2.yaml`, `.hadolint.yaml`, `.github/zizmor.yml`, `.prettierrc.yml`, `.prettierignore`, `_typos.toml`, `taplo.toml`, `cog.toml`, `.cog/changelog.tera`, `.claude/settings.json`;
  - to `directories`: `tests/CLI/bats/` (the negative tests and their fixtures);
  - to `required_directories`: `tests/CLI/bats` and `tests/Load`;
  - to `override_names`: `.gitleaks.toml`, `.gitleaksignore`, `.trivyignore`, `.prettierignore`, `.shellcheckrc`, `actionlint.yaml`, `actionlint.yml` (files that would configure a linter of this document for part of the tree);
  - to `json_values`: nothing. The kit ships the scenario `health` only, and the core list already holds its four budget pointers; `tests/Load/config.json.dist` is not an entry of `files`.
- **Rationale.** AD-25, NFR26: every file that configures a gate, the hooks, the K6 budgets and every command line that carries a threshold (all in `make/gates.mk`).
- **Alternatives rejected.** Protecting the whole `Makefile` (every new target would need the label).
- **Consequences.** The top-level `Makefile` holds wiring only; a recipe there must call a variable from `make/gates.mk` for anything that is a gate, and the gate scripts take their thresholds from the flags that `make/gates.mk` exports. The budgets of the shipped scenario are protected as JSON values (core `json_values`); the rest of `tests/Load/config.json.dist` changes freely, so a service adds scenarios and load shapes without the label (FR44(b)).

### DD-17 Documentation-drift check

- **Decision.** `make docs-check` runs `scripts/check-docs.sh` (gate G26, job `docs-check`). It reads every tracked Markdown file except `CHANGELOG.md` and `specs/**` and applies the seven rules of FR110: (1) a cited `make <target>` must be a target; (2) a cited `service <command>` must be in the binary's `--help` (option words between `service` and the command are skipped; after `-p`, `--package` or `--bin` the word `service` names the package, so the command is read only after a `--` word, option words after it skipped, which checks `cargo run -p service -- <command>`, and a cargo command without `--`, such as `cargo test -p service --lib config`, cites no command), and a cited slash command must have a file under `.claude/commands/`; (3) a backticked token that begins with the name of a tracked top-level file or directory, or of `_bmad/`, `_bmad-output/`, `.ralph/` or `bmalph/state/`, must name a path that exists or is matched by a git-ignore rule, checked up to its first placeholder or wildcard character; (4) the block between `<!-- verification:start -->` and `<!-- verification:end -->` in `AGENTS.md` must equal the output of `make ci-list`, and no other file may contain a numbered list or a fenced block that cites three or more gate targets (the two markers exempt the block in `AGENTS.md` only); (5) `docs/environment.md` must list exactly the union of five sets: the variables in the help of `service serve`, `service migrate` and `service healthcheck`; the variables interpolated in the four compose files and the variables that their top-level `secrets:` read with `environment:`; the names assigned or referenced in `.env` and `.env.test`; the variables that the `Makefile` and its two fragments under `make/` declare with `?=`; and the keys of `[env]` in `.cargo/config.toml`; (6) every target must appear in `make help` and in `docs/make-targets.md` (a target defined in `make/service.mk` in `docs/make-targets-service.md`); (7) every skill has a `name` equal to its directory, a `description`, and an entry in the index.
- **Rationale.** FR110, NFR43 to NFR45; fixes AD:D-AD-04, AD:D-AD-09, AD:D-AD-15.
- **Alternatives rejected.** A Markdown link checker only (does not know Make targets). Generating the documents (reviewers lose prose).
- **Consequences.** The script starts no cargo command. The recipe writes the help texts of the binary (`service --help` and the help of `serve`, `migrate` and `healthcheck`) through `$(EXEC)` to `target/reports/docs-check/` and passes the directory with `--service-help`, so rules (2) and (5) work in every execution mode and the job restores the Rust cache of the `service` debug build. Rule (4) shapes the guidance: skills and the README link to the verification section and never list three gate targets in one block. Rule (5) is why every caller-settable Make variable is declared with `?=`, except `PROJECT_NAME`, which is assigned under an origin test (DD-23) so that it can fall back to `COMPOSE_PROJECT_NAME`, and which `docs/environment.md` describes below its table; inputs that only a script reads (`COMMIT_TITLE`, `COMMIT_MSG_FILE`, `ALLOW_PROTECTED_CONFIG_EDIT`, the AWS credentials of the cloud load run) are documented with their target in `docs/make-targets.md`. The script takes `--root <dir>` so that each rule has a negative test on a small generated tree, help texts included.

### DD-18 Assistant settings file

- **Decision.** `.claude/settings.json` is committed with a permission allow-list (one entry per gate, test, export and start target, plus read-only git, cargo and compose commands), an ask-list (`aws-load-tests`, `aws-load-tests-cleanup`, `update`, `purge`, `clean`, `down`, and also `migrate-rollback`, `build`, `stop` and the two fix targets whose names begin with a gate name), a deny-list (`git push --force` and `-f`, `git commit --no-verify` and `-n`, `git push --no-verify`, `git -c core.hooksPath` and `git config core.hooksPath`, access to `.env.local`) and two hooks on `Edit|Write|MultiEdit|NotebookEdit` that run `scripts/claude/guard-protected-files.sh`. The script takes the protected items from `cargo xtask config-guard --list`, the one list (NFR26). Before the edit (`PreToolUse`) it blocks, with exit status 2 and an explanation, a path that is a protected file or lies under a protected directory. For a file that holds a protected section (the root `Cargo.toml`, `tests/Load/config.json.dist`) it records the state before the edit as a commit object (`git stash create`, which changes neither the working tree nor the index) and lets the edit happen; after the edit (`PostToolUse`) it runs `cargo xtask config-guard --base <recorded commit>`, and when the guard reports a modified table or value of that file it restores the file from the recorded commit and exits 2 with the same explanation. A deliberate change is made by a human who starts the assistant with `ALLOW_PROTECTED_CONFIG_EDIT=1`; the pull request then needs the label `approved-config-change`. Details are in "AI Guidance and Documentation".
- **Rationale.** Scope 3.1, FR111, NFR26; fixes AD:D-AD-01, AD:D-AD-18. One list (`protected-config.toml`) feeds the CI guard, the hook and the guidance.
- **Alternatives rejected.** A second list in the settings file (drift). `deny` rules per file (cannot express directories and cannot be overridden deliberately).
- **Consequences.** Without `cargo`, `git` or `jq` on `PATH` the hook warns and allows; CI remains the enforcement (`jq` reads the hook input; it is in the development image and `make doctor` reports it as optional). Two Bats tests cover the rule (FR111): an edit of `clippy.toml` is refused before it happens; an edit of a lint level in the root `Cargo.toml` is undone and refused, while an edit of a dependency in the same file is allowed.

### DD-19 Isolated production-image environment for conformance and load tests

- **Decision.** Schemathesis and K6 run against the production image in a compose project of its own, `<project>-load`, built from `docker-compose.yml` plus `docker-compose.load_test.override.yml`: own network, own volumes, no published database or LocalStack port, the service published on `LOAD_TEST_APP_PORT` (default `18081`). `scripts/with-load-test-stack.sh` builds, starts, waits for health, prepares the database, runs the given command and always removes the project with its volumes. `SCHEMATHESIS_URL` keeps its name and defaults to `http://localhost:$(LOAD_TEST_APP_PORT)`; when the caller sets it, no environment is started and that URL is tested. The load runners receive `API_HOST=localhost` and `API_PORT=$(LOAD_TEST_APP_PORT)`.
- **Rationale.** Scope 5 (production image), scope 3.1 (cannot touch development data); fixes DM:D-03, DM:D-14, CI:D-29. Port 8081 belongs to the development service, which is a debug build; with a separate port `make ci` runs next to a started development environment.
- **Alternatives rejected.** Port 8081 for both (port clash, or a debug build under test). A third compose project for Schemathesis (two identical environments).
- **Consequences.** This is how the PHP default `http://localhost:8081` is honoured (FR76(d), PD-29): the variable keeps its name and meaning, the service inside the managed environment still listens on 8081, and the default URL points at the host port that environment publishes; `make schemathesis-validate SCHEMATHESIS_URL=http://localhost:8081` tests whatever already runs on 8081.

### DD-20 Container images

- **Decision.** One `Dockerfile`, six stages (`base`, `chef`, `planner`, `builder`, `dev`, `prod`). Both base images are written as literal `FROM` references with tag and digest: the builder `rust:1.99.1-slim-trixie` and the runtime `gcr.io/distroless/cc-debian13:nonroot`. The BuildKit frontend of the `# syntax=` parser directive is pinned the same way, to the version that `docker/dockerfile:1` resolved to (`docker/dockerfile:1.27@sha256:4edf897a3ffa55b89f906fc8cc78afdb3f1834cc9c7083565e611a8a7d5fe99e`), and G31 rule (b) reads that line. No build argument replaces an image. Dependency layer through cargo-chef 0.1.78; production image with user `65532:65532`, entry point `/app/service`, healthcheck `["/app/service", "healthcheck"]`. The development image carries the toolchain, the tools of DD-04, Bats and watchexec, runs as the invoking user and keeps cargo's build directory and home outside `/app` (`/cargo-target`, `/cargo`). Operating-system packages of the `dev` stage are governed by the digest-pinned base image; the `.hadolint.yaml` entry for DL3008 carries this reason (NFR13 reads "tool" as cargo tools, Bats libraries, images and actions).
- **Rationale.** Scope 8.7; research D8 (44 MB hello image, 8 s rebuild after a source change). Literal references are what Dependabot's `docker` ecosystem updates and what G31 rules (b) and (c) read; the first version had the build arguments `BUILDER_BASE_IMAGE` and `RUNTIME_IMAGE`, which no requirement asks for and which let a build leave the pinned images. Verified: the revised file passes hadolint 2.15.1 with `.hadolint.yaml`.
- **Alternatives rejected.** `lukemathwalker/cargo-chef` as base (a third-party base to keep in step with the toolchain). musl variant (scope 8.7). BuildKit cache mounts only (cold on every runner).
- **Consequences.** `scripts/check-invariants.sh` fails when the builder image tag, `rust-toolchain.toml` and `rust-version` name different versions (rule (c)). A build behind a TLS-intercepting proxy needs a base image that carries the proxy's CA certificate; it replaces the pinned builder for one build without a change to a tracked file, through a BuildKit named context: `docker build --build-context <builder reference of the Dockerfile>=docker-image://<derived image> --target dev|prod --tag <image name> .`, after which the targets run with `BUILD_IMAGE=0` (the replacement was verified with Buildx 0.38; `docs/troubleshooting.md` describes it).

### DD-21 Secrets scan and image scan

- **Decision.** Secrets: gitleaks 8.30.1 (`make secrets-scan`, G9) over the full history (`gitleaks git`) and over the tracked and untracked-but-not-ignored working files (`gitleaks dir` on an exported copy). Image: Trivy 0.76.2 (`make image-scan`, G10) over a `docker save` archive of the production image with `--severity HIGH,CRITICAL --exit-code 1`; a finding without a fix counts like any other and is accepted only as an entry of `.trivyignore` with the date, the reason and a review date (NFR11). Both run from images pinned by tag and digest and both are Make targets and CI jobs.
- **Rationale.** Scope 8.5, FR47, FR48, PD-11. Verified here: gitleaks reports no leak on the committed `.env` placeholders and exits 1 on a planted key pair.
- **Alternatives rejected.** `gitleaks-action` (needs a licence for organisations). Grype (equivalent; Trivy also reads the distroless package metadata and is the tool the audit names). Mounting the Docker socket into the scanner (wider access than a saved archive).
- **Consequences.** Trivy downloads its vulnerability database on each CI run; it is not cached (the cache directory measured 1.7 GB, and a cache step next to the image build is a cache-poisoning finding of zizmor). Locally the database stays in `~/.cache/trivy`.

### DD-22 Non-Rust tools run from pinned images

- **Decision.** Spectral, oasdiff, Schemathesis, K6, Structurizr, gitleaks, Trivy, actionlint, hadolint, shellcheck, markdownlint-cli2, the AWS CLI of the cloud load run, a Node image (graphql-inspector, ESLint, Prettier through pnpm) and a Python image (yamllint) are started with `docker run` by the Make targets and their scripts, identically on a host and on a runner. Image references are in `make/tool-versions.mk`.
- **Rationale.** NFR37 limits host prerequisites to a container engine, Make, Bash, git and curl; "CI equals make" (research D6, D14).
- **Alternatives rejected.** Tool-specific GitHub Actions (not reproducible with `make`). Host installs of Node and Python.
- **Consequences.** These targets are class HOST. The lint job pulls about 400 MB of images.

### DD-23 Project name

- **Decision.** `PROJECT_NAME` is the caller's value, else the caller's `COMPOSE_PROJECT_NAME`, else derived in the Makefile from the checkout directory name (lower-cased, characters outside `a-z0-9_-` replaced by `-`); it is exported as `COMPOSE_PROJECT_NAME` unless the caller has set that variable (FR64). Image names are `$(PROJECT_NAME)-app:dev` and `$(PROJECT_NAME)-app:prod`; the isolated project is `$(PROJECT_NAME)-load`. In workflows, the Makefile, the image definition, compose files, scripts, tests, skills, `AGENTS.md` and `CLAUDE.md` the literal repository name appears only in the sync source setting of the two sync workflows (FR92(c)); the workspace manifest states it as package identity. The project-name variable needs no literal because it is derived, so a new service changes only the manifest. Files outside those classes that name the repository are `bmalph/config.json`, the `README.md` badges and the planning bundle under `specs/`; all three are service-owned.
- **Rationale.** Scope 4, FR92, NFR38; fixes DM:D-16 and CI:D-06. On a runner the checkout directory equals the repository name, so a downstream repository needs no edit.
- **Alternatives rejected.** A `COMPOSE_PROJECT_NAME` line in `.env` (the file's content is fixed by the core document, and it would be one more literal to rename).
- **Consequences.** `scripts/check-invariants.sh` searches the classes above for the repository name (G31 rule (e)). No workflow refers to a container by name.

### DD-24 Security analysis in CI

- **Decision.** Job `codeql` runs `github/codeql-action` for the languages `rust` (build mode `none`) and `actions`. Job `static-analysis` is the gate and holds a read-only token: after the gate, also when it failed, it runs `make static-analysis-sarif` and stores `target/sarif/clippy.sarif` as the artifact `clippy-sarif`. Job `clippy-sarif-upload` of the same workflow downloads the artifact and uploads it with category `clippy`; it is the only place besides `codeql` and the scheduled `code-scanning` job with `security-events: write`, and it executes no code of the pull request (NFR14). Both publications run on pushes to `main`, in the scheduled workflow and on pull requests from a branch of the same repository, subject to the rule of DD-13. They are publications, not gates (PD-22): their findings appear in code scanning and do not fail a pull request; the failing conditions are carried by G2, G7, G9 and G10.
- **Rationale.** Scope 8.5, FR49; fixes CI:D-04 (upload skipped exactly when findings exist).
- **Alternatives rejected.** Semgrep (no taint rules for Rust, research D6). A `cargo deny` SARIF upload (exits 0 on findings, pitfall P3; the gate already fails).
- **Consequences.** CodeQL has no Make target; it is listed as CI-only.

### DD-25 Load budgets and the AWS instance

- **Decision.** p99 budgets per profile are in `tests/Load/config.json.dist`. The kit ships one scenario, `health` (FR78), with the budgets 50, 200, 1000 and 5000 ms (smoke, average, stress, spike): the smoke budget is derived from the hosted measurement by the rule of NFR1, the other three are the NFR1 ceilings. The AWS runner compiles on the instance and defaults to `c6i.xlarge` (4 vCPU, 8 GiB), with the AMI resolved per region from the public SSM parameter for Ubuntu 24.04.
- **Rationale.** FR80, NFR1, audit P19. NFR1 derives each budget from a measurement on a 4-vCPU GitHub-hosted runner, which exists only after the first pipeline run; until then a value below the ceiling would be a guess and a source of flaky pull requests (each health request makes one LocalStack call, which was not measured under load). The first version shipped three example-resource scenarios with twelve unmeasured budgets; no requirement asks for them (the PHP kit ships `health.js` only), so they are removed.
- **Alternatives rejected.** `t2.micro` (cannot finish a release build). Shipping a locally built image through S3 (fails for developers on arm64 machines).
- **Consequences.** After the first smoke run on a 4-vCPU GitHub-hosted runner the smoke budget is set to min(ceiling, max(50 ms, 3 x measured p99 rounded up to 10 ms)); the measurement, its date and the budget are recorded in `docs/load-testing.md` (NFR1), and the change is a labelled pull request because the value is protected, unless the base branch does not hold the protected item yet: on pull request #1 every protected file is new against `main`, so the guard passes without a label and the budget was set there (50 ms from the hosted p99 of 7.25 ms, job `Smoke load tests` of `db7fe08`, 2026-11-11). The average, stress and spike budgets run in no pipeline (PD-23); they stay at their ceilings until a maintainer measures them on comparable hardware (the AWS runner's default instance has 4 vCPU) and applies the same rule.

### Decision impact

Implementation order for this document: (1) `make/tool-versions.mk`, `make/gates.mk`, `Makefile`, `scripts/with-env.sh`, `scripts/doctor.sh`; (2) `Dockerfile`, compose files; (3) contract, lint and scan targets with their scripts; (4) Bats suite; (5) workflows; (6) hooks, release, sync; (7) load kit; (8) guidance and documents; (9) `scripts/check-docs.sh` and `scripts/check-invariants.sh` last, when everything they check exists.

## Command Surface

The command surface is three files: `Makefile` (targets, help, lifecycle wiring; not protected), `make/tool-versions.mk` (every tool version and tool image; protected) and `make/gates.mk` (every gate command line and threshold; protected). The `Makefile` starts with `include make/tool-versions.mk` and `include make/gates.mk`, sets `.DEFAULT_GOAL := help`, `SHELL := bash`, `.SHELLFLAGS := -euo pipefail -c`, declares every target `.PHONY` and ends with `-include make/service.mk`, the optional fragment in which a downstream service defines its own targets (the template ships no such file; see "Template sync"). Recipes use tabs. Every target has a `## ` help text; `help` prints `Usage:`, `make [target] [arg="val"...]`, `Targets:` and all targets sorted, in a 28-character column.

### Variables

Every variable of this table that a caller may set, except `PROJECT_NAME` (assigned under an origin test so that it can fall back to `COMPOSE_PROJECT_NAME`, DD-23), is declared in the `Makefile` or in one of its two fragments with `?=` (with an empty default where the table says "empty" or "from the environment"), so the caller's value always wins (FR64) and rule (5) of the drift check can read the set. `TEST_SEED` and `PROPTEST_RNG_SEED` are not Make variables: their default is recorded in `.cargo/config.toml` (core document), a caller's value wins, and `EXEC` forwards it.

| Variable | Default | Meaning |
| --- | --- | --- |
| `PROJECT_NAME` | the caller's `COMPOSE_PROJECT_NAME` when set, else the checkout directory name, normalised (DD-23) | Compose project name and image name prefix |
| `COMPOSE_PROJECT_NAME` | `$(PROJECT_NAME)` | Exported; a value set by the caller is kept |
| `RUN_IN_CONTAINER` | empty | `1` or `true`: class TOOL targets run in the `app` container (DD-01) |
| `CI` | from the environment | Non-empty: native mode is forced, `COMPOSE_PROFILES` defaults to empty, `CARGO_BUILD_WARNINGS=deny` is exported |
| `IN_DEV_CONTAINER` | from the environment (`1` in the `app` container of the development environment) | Native mode is forced; `ensure-deps` and `ensure-database` start nothing |
| `CARGO_BUILD_WARNINGS` | `deny` when `CI` is set and inside `make ci`; otherwise from the environment | Cargo's `build.warnings`; unset again for cargo-mutants by `MUTANTS_CMD` |
| `EXEC` | empty, or `$(DOCKER_COMPOSE) exec -T --user $(HOST_UID):$(HOST_GID) -e … app` | Prefix of every class TOOL command; computed, not meant to be set |
| `EXEC_ENV` | `TEST_SEED PROPTEST_RNG_SEED CONFIG_CHANGE_APPROVED CARGO_BUILD_WARNINGS DATABASE_STATEMENT_TIMEOUT` | Variables that `EXEC` forwards into the container; computed |
| `DOCKER`, `DOCKER_COMPOSE` | `docker`, `docker compose` | Exported; container engine commands, also used by the scripts |
| `HOST_UID`, `HOST_GID` | `$(shell id -u)`, `$(shell id -g)` | Exported; user of the development container and of `docker run` tools |
| `COMPOSE_PROFILES` | `docs` (empty when `CI` is set) | Exported; `docs` starts the Structurizr viewer (DD-05) |
| `APP_IMAGE`, `DEV_IMAGE` | `$(PROJECT_NAME)-app:prod`, `$(PROJECT_NAME)-app:dev` | Exported; image names used by the compose files |
| `BUILD_IMAGE` | `1` | Exported; `0`: `start`, `build-prod-image` and the isolated-environment wrapper use images that are already built and pass `--no-build` |
| `DEPS` | `database localstack` | Compose services that `start-deps` starts |
| `base_ref` | `origin/main` | Base of `validate-configuration`, `openapi-diff`, `graphql-diff`, `mutation-tests-diff` |
| `CONFIG_CHANGE_APPROVED`, `BREAKING_CHANGE_APPROVED` | empty | `true` waives the corresponding approval (set by CI from labels) |
| `LOAD_TEST_PROJECT` | `$(PROJECT_NAME)-load` | Exported; compose project of the isolated production-image environment |
| `LOAD_TEST_APP_PORT` | `18081` | Exported; host port of the service in that environment |
| `SCHEMATHESIS_URL` | `http://localhost:$(LOAD_TEST_APP_PORT)` | Target of Schemathesis; when set by the caller nothing is started |
| `scenario`, `runSmoke`, `runAverage`, `runStress`, `runSpike` | empty, `true` x 4 | Parameters of `execute-load-tests-script` |
| `LOCAL_MODE_ENV` | `false` | `true`: the AWS load scripts talk to LocalStack |
| `region`, `ami`, `instance_type`, `instance_tag`, `role`, `branch`, `repo_url`, `security_group`, `load_target` | empty (script defaults apply) | Parameters of `aws-load-tests`, passed as `-r -a -t -i -o -b -u -s -l` |
| `name` | empty | Parameter of `create-migration` (snake_case description) |
| `group`, `context` | empty | Parameters of `new-context` |
| `MUTANTS_SHARD` | empty | `k/n` with k from 0 to n-1 (cargo-mutants numbering): adds `--shard k/n` to `mutation-tests` and `mutation-tests-diff` |
| `MUTANTS_IN_PLACE` | empty | set by CI only: exactly `1` adds `--in-place` to `mutation-tests` and `mutation-tests-diff`, so cargo-mutants builds in the checkout and reuses the restored `target/` (NFR40); any other value adds nothing; locally it stays unset, because in place cargo-mutants rewrites source files of the working tree while it runs |
| `ARCH_ARGS`, `DENY_ARGS`, `BATS_PATH` (`tests/CLI/bats`), `BATS_ARGS`, `DOCKERFILES` (`Dockerfile`), `IMAGE_TO_SCAN` | empty unless noted | Test hooks used by the negative Bats tests |
| `OPENAPI_DIFF_IMAGE` | the pinned oasdiff image | PHP variable name kept; overrides the image of `openapi-diff` (FR75(a)) |
| `CI_STAGES` | the seven stages of DD-02 | Stages run by `ci` |
| `BMALPH_PLATFORM`, `BMALPH_DRY_RUN` | `claude-code`, `false` | Parameters of the bmalph targets |
| `TEST_ENV` | `env -u DATABASE_URL scripts/with-env.sh .env .env.test --` | Prefix of commands that need the test configuration; a `DATABASE_URL` of the caller is removed first, every other caller value still wins; computed |
| `CI_PREFLIGHT_TARGETS`, `CI_STATIC_ANALYSIS_TARGETS`, `CI_TESTS_TARGETS`, `CI_CONTRACTS_TARGETS`, `CI_MUTATION_TARGETS`, `CI_DELIVERY_TARGETS`, `CI_CLI_TARGETS`; `CI_GATES` | the gate targets of each stage of DD-02, in order; their concatenation | The one gate list: the stage recipes, `ci-list`, rule (d) of `scripts/check-invariants.sh` and `AGENTS.md` read it; computed |
| `NEWLINE`, `STAGE_RECIPE`, `DOCTOR_MAKE` | a newline; `$(foreach gate,$(1),@$(MAKE) --no-print-directory $(gate)$(NEWLINE))`; `$(MAKE)` | Helpers: `STAGE_RECIPE` writes one recipe line per gate of a stage; `DOCTOR_MAKE` hands the running make to `scripts/doctor.sh` without naming `MAKE` literally in the recipe, so `make -n` does not run the doctor; computed |

Inputs that only a script reads and that are therefore no Make variables: `COMMIT_TITLE` and `COMMIT_MSG_FILE` (`check-commit-message`; read from the environment so that a title cannot inject shell text into a recipe), `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_SESSION_TOKEN` and `AWS_REGION` (`aws-load-tests`), `ALLOW_PROTECTED_CONFIG_EDIT` (assistant hook).

Header of the `Makefile` (normative; run through GNU Make 4.3 for this revision: the name derivation with and without a caller value, the `ci` setting, the exec line and the `BUILD_IMAGE` switch):

```make
include make/tool-versions.mk
include make/gates.mk

.DEFAULT_GOAL := help
SHELL := bash
.SHELLFLAGS := -euo pipefail -c

ifeq ($(origin PROJECT_NAME),undefined)
PROJECT_NAME := $(or $(COMPOSE_PROJECT_NAME),$(shell basename "$(CURDIR)" | tr '[:upper:]' '[:lower:]' | tr -c 'a-z0-9_\n-' '-'))
endif
export COMPOSE_PROJECT_NAME ?= $(PROJECT_NAME)
export HOST_UID ?= $(shell id -u)
export HOST_GID ?= $(shell id -g)
export APP_IMAGE ?= $(PROJECT_NAME)-app:prod
export DEV_IMAGE ?= $(PROJECT_NAME)-app:dev
export LOAD_TEST_PROJECT ?= $(PROJECT_NAME)-load
export LOAD_TEST_APP_PORT ?= 18081
export BUILD_IMAGE ?= 1
export DOCKER ?= docker
export DOCKER_COMPOSE ?= docker compose
DOCKER_RUN = $(DOCKER) run --rm --user $(HOST_UID):$(HOST_GID)

ifeq ($(strip $(CI)),)
export COMPOSE_PROFILES ?= docs
else
export COMPOSE_PROFILES ?=
export CARGO_BUILD_WARNINGS ?= deny
endif
ci: export CARGO_BUILD_WARNINGS ?= deny

EXEC_ENV := TEST_SEED PROPTEST_RNG_SEED CONFIG_CHANGE_APPROVED CARGO_BUILD_WARNINGS DATABASE_STATEMENT_TIMEOUT
EXEC :=
ifneq ($(filter 1 true,$(RUN_IN_CONTAINER)),)
ifeq ($(strip $(CI))$(strip $(IN_DEV_CONTAINER)),)
EXEC := $(DOCKER_COMPOSE) exec -T --user $(HOST_UID):$(HOST_GID) $(foreach v,$(EXEC_ENV),-e $(v)) app
endif
endif

# The test targets run on the test database of .env.test: a DATABASE_URL exported in the caller's shell (sqlx-cli users
# often have one that names the development database) is removed first, every other caller value still wins.
TEST_ENV = env -u DATABASE_URL scripts/with-env.sh .env .env.test --
```

`PROJECT_NAME` is assigned with `:=` inside the `origin` test because the pair `PROJECT_NAME ?= $(or $(COMPOSE_PROJECT_NAME),…)` and `COMPOSE_PROJECT_NAME ?= $(PROJECT_NAME)` would be a recursive definition.

### `make/tool-versions.mk`

```make
# The single list of tool versions and tool images. Protected file (protected-config.toml).
# Every line is `export NAME := value`: recipes and scripts read the values from the environment, and the
# scripts that also run outside Make read this file with that line format.
# Cargo tools: installed by scripts/install-tools.sh (host, development image) and by taiki-e/install-action
# (CI). A version may be raised only to one listed in manifests/<tool>.json of the pinned action commit.
export CARGO_BINSTALL_VERSION        := 1.26.2
export CARGO_BINSTALL_SHA256_X86_64  := 9e0e064f189a9abb9f8df5fb93c3c810ef5cc19e7212d998c7a64c9e4d3527d5
export CARGO_BINSTALL_SHA256_AARCH64 := a235250cd684af9bf1e3a1e4c216b1d36e6c553553d5d789744f27689d81c2d6
export CARGO_CHEF_VERSION            := 0.1.78
export CARGO_NEXTEST_VERSION         := 0.9.151
export CARGO_LLVM_COV_VERSION        := 0.9.1
export CARGO_MUTANTS_VERSION         := 27.1.0
export CARGO_DENY_VERSION            := 0.20.2
export CARGO_SHEAR_VERSION           := 1.14.0
export CARGO_HACK_VERSION            := 0.6.46
export SQLX_CLI_VERSION              := 0.9.0
export CLIPPY_SARIF_VERSION          := 0.8.0
export TAPLO_VERSION                 := 0.10.0
export TYPOS_VERSION                 := 1.52.0
export ZIZMOR_VERSION                := 1.32.0
export WATCHEXEC_VERSION             := 2.8.0
export GRAPHQL_INSPECTOR_VERSION     := 7.0.2
export YAMLLINT_VERSION              := 1.38.0
export BMALPH_VERSION                := 2.11.0
export DEVCONTAINER_CLI_VERSION      := 0.90.0
# cocogitto: the release flow (scripts/release.sh) and its Bats test; the bats-group job installs it through the
# cargo-binstall fallback of the install action, release.yml through its pinned action.
export COCOGITTO_VERSION             := 7.0.0

# Bats (scripts/install-bats.sh): tag and commit SHA.
export BATS_CORE    := v1.14.0 eb7f42f8d608ac693d7a4b67474f6714ea68cfc5
export BATS_SUPPORT := v0.3.0 24a72e14349690bcbf7c151b9d2d1cdd32d36eb1
export BATS_ASSERT  := v2.2.4 f1e9280eaae8f86cbe278a687e6ba755bc802c1a
export BATS_FILE    := v0.4.0 13ad5e2ffcc360281432db3d43a306f7b3667d60

# Tool images: tag and digest (digests read from the registry on 2026-11-10).
export SPECTRAL_IMAGE     := stoplight/spectral:6.17.1@sha256:3b05e6cd9d43375c454cc0c022e91b3285851043b05c3e738342ea3f84cde76d
export OPENAPI_DIFF_IMAGE ?= tufin/oasdiff:v1.37.0@sha256:ff02af37a254b5199b9d72e7bba93dae5837a9119d59459168d1398893b47aea
export SCHEMATHESIS_IMAGE := schemathesis/schemathesis:4.33.1@sha256:ddf8b3c9e0b3ee7edfa65dad4d0c0e05c53fb5d92f99db379d40da01883474c7
export K6_IMAGE           := grafana/k6:2.3.0@sha256:2dd4756ec6e14e62778916e1eb850108e4b499304c02df56760fa0bdb0fba0b8
export STRUCTURIZR_IMAGE  := structurizr/structurizr:2026.10.26@sha256:8aebde645a7cc8d3b9e1e774cd6d75a84ce45d74ae70eb783876182fe7db31a2
export GITLEAKS_IMAGE     := zricethezav/gitleaks:v8.30.1@sha256:c00b6bd0aeb3071cbcb79009cb16a60dd9e0a7c60e2be9ab65d25e6bc8abbb7f
export TRIVY_IMAGE        := aquasec/trivy:0.76.2@sha256:8f6bb947dbc528902e11f2f5be52f7b519f6a39d5d148f41166f2040499768fb
export ACTIONLINT_IMAGE   := rhysd/actionlint:1.7.12@sha256:b1934ee5f1c509618f2508e6eb47ee0d3520686341fec936f3b79331f9315667
export HADOLINT_IMAGE     := hadolint/hadolint:v2.15.1@sha256:32dac94127fd60b7b7e3fbfc65e1383b9b5e25c9bfd7b8536de7a539fe68a12d
export SHELLCHECK_IMAGE   := koalaman/shellcheck:v0.11.0@sha256:61862eba1fcf09a484ebcc6feea46f1782532571a34ed51fedf90dd25f925a8d
export MARKDOWNLINT_IMAGE := davidanson/markdownlint-cli2:v0.24.0@sha256:70ead7c0352d22d04bbe612cd96b54ec591538d8f4626cafe391bb2ecf3639ca
export NODE_IMAGE         := node:22.23.4-alpine3.24@sha256:0e1754758c2df4cfa3e68d59e2e1b7374bb7fd9ea6328eaaacf15cc934af8202
export PYTHON_IMAGE       := python:3.13-alpine@sha256:1e1f6334529d1fb26a2ef619d7cffb30a7c7d7cc1cc7338ebee283a58d1aeaca
export AWS_CLI_IMAGE      := amazon/aws-cli:2.39.7@sha256:bba262340b0bc7cbf3a8a48612531263b56f4b280c4f036a07816af1b271bda0
# Input of the negative test of the image scan (G10): an old image with known CRITICAL findings.
export VULNERABLE_FIXTURE_IMAGE := alpine:3.10.9@sha256:451eee8bedcb2f029756dc3e9d73bab0e7943c1ac55cff3a4861c52a0fdd3e98
```

The first version of this fragment also held `RUST_VERSION`. It is removed: the toolchain version is stated in the three places NFR13 names (`rust-toolchain.toml`, `rust-version` in the workspace manifest, the builder image tag), which G31 rule (c) keeps equal. `COCOGITTO_VERSION` (7.0.0) is held here again: `scripts/install-tools.sh` installs cocogitto for the release flow and its Bats test (`make_release.bats`), the second `tool:` list of the `bats-group` job installs it through the `cargo-binstall` fallback of the install action, and the release job still installs it through `cocogitto/cocogitto-action`.

### `make/gates.mk`

The cargo command lines are those of the core document (QG-01 to QG-20), character for character; `$(EXEC)` is the execution switch. The three `export` lines carry thresholds into gate scripts, which read them with `${NAME:?}` and hold none of their own.

```make
# Gate command lines and thresholds. Protected file (protected-config.toml).
FORMAT_CHECK_CMD = cargo fmt --all --check
LINT_PROD_CMD    = cargo clippy --locked --workspace --lib --bins --all-features -- -D warnings
LINT_ALL_CMD     = cargo clippy --locked --workspace --all-targets --all-features -- -D warnings -A clippy::cognitive_complexity -A clippy::too_many_lines
LINT_RUSTDOC_CMD = env RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --no-deps --document-private-items
COMPLEXITY_CMD   = cargo clippy --locked --workspace --lib --bins --all-features -- -A warnings -D clippy::cognitive_complexity -D clippy::too_many_lines -D clippy::excessive_nesting -D clippy::too_many_arguments
ARCH_CMD         = cargo xtask arch $(ARCH_ARGS)
CONFIG_GUARD_CMD = cargo xtask config-guard --base $(base_ref)
MANIFEST_CMD     = cargo metadata --locked --format-version 1 > /dev/null
HACK_CMD         = cargo hack check --workspace --each-feature --locked
DENY_CMD         = cargo deny --locked $(DENY_ARGS) check
DENY_OFFLINE_CMD = cargo deny --locked --offline check bans licenses sources
SHEAR_CMD        = cargo shear --locked
SQLX_CHECK_CMD   = cargo sqlx prepare --workspace --check -- --all-targets --all-features --locked
CHECK_CONFIG_CMD = cargo run --locked --quiet -p service -- check-config
UNIT_CMD         = cargo nextest run --locked --workspace --lib --bins
DOCTEST_CMD      = cargo test --locked --workspace --doc
INTEGRATION_CMD  = cargo nextest run --locked --workspace --test integration
E2E_CMD          = cargo test --locked -p e2e-tests --test e2e
# Coverage of the unit and integration suites (QG-16). This group is the coverage exclusion record of NFR19:
# an exclusion is an `--ignore-filename-regex` alternative added to these commands with a comment that gives
# the reason. The first release has none.
COVERAGE_CLEAN_CMD = cargo llvm-cov clean --workspace
COVERAGE_RUN_CMD   = cargo llvm-cov --no-report nextest --locked --workspace --lib --bins --test integration --profile ci
COVERAGE_LCOV_CMD  = cargo llvm-cov report --lcov --output-path target/coverage/lcov.info
COVERAGE_HTML_CMD  = cargo llvm-cov report --html --output-dir target/coverage
COVERAGE_GATE_CMD  = cargo llvm-cov report --fail-under-lines 100 --fail-under-functions 100
COVERAGE_XML_CMD   = cargo llvm-cov report --cobertura --output-path target/coverage/coverage.xml
# MUTANTS_IN_PLACE (set by CI only, NFR40): exactly the value 1 adds --in-place, so cargo-mutants builds in the
# checkout and reuses the target/ that the Rust cache restores; any other value adds nothing, a 1 padded with blanks
# too. It is no argument hook. `$(findstring V,1)` is not empty only when V is the string 1, blanks included.
MUTANTS_CMD      = env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --workspace --cargo-arg=--locked $(if $(MUTANTS_SHARD),--shard $(MUTANTS_SHARD)) $(if $(findstring $(MUTANTS_IN_PLACE),1),--in-place)
OPENAPI_EXPORT_CMD  = cargo run --locked --quiet -p service -- openapi-export --output .github/openapi-spec/spec.yaml
GRAPHQL_EXPORT_CMD  = cargo run --locked --quiet -p service -- graphql-export --output .github/graphql-spec/spec
SNAPSHOT_STATUS_CMD = test -z "$$(git status --porcelain -- .github/openapi-spec .github/graphql-spec)"
SCHEMATHESIS_FLAGS  = --checks all --phases=examples,coverage
export SPECTRAL_FLAGS = --fail-severity=hint
export OASDIFF_FLAGS  = --fail-on ERR
export TRIVY_FLAGS    = --severity HIGH,CRITICAL --exit-code 1 --ignorefile .trivyignore
CI_STAGES ?= ci-preflight ci-static-analysis ci-tests ci-contracts ci-mutation ci-delivery ci-cli
CONFIG_CHANGE_APPROVED ?=
BREAKING_CHANGE_APPROVED ?=
```

Differences from the first version, each from a measurement: the rustdoc line starts with `env`, because a bare `NAME=value command` cannot be executed behind `docker compose exec` (exit status 127); the dependency, unused-dependency, query-metadata and mutation commands pass `--locked` (FR45; `cargo shear` without it rewrote `Cargo.lock`); the mutation command unsets `CARGO_BUILD_WARNINGS`, with which every function-replacement mutant is unviable and the gate cannot fail; the coverage group has no end-to-end run and no `--codecov` report, and the floor is the last command of the sequence, so the reports exist when it fails; the snapshot check uses `git status --porcelain`, which also reports a snapshot that was never committed; `TRIVY_FLAGS` has no `--ignore-unfixed`. `COMPLEXITY_CMD` is QG-20 (verified: it passes on clean code and fails with `this function has too many arguments (6/5)` on a planted function while unrelated warnings stay allowed).

### Targets

Columns: **Class** is TOOL (the PRD's toolchain class: honours `RUN_IN_CONTAINER`) or HOST (the PRD's orchestration class, plus plain host commands: always where Make runs). Every HOST recipe that calls the container engine starts with `$(REQUIRE_DOCKER)`, which exits 2 with `This target needs a reachable container engine (docker).` when `docker info` fails, for example in a development-container shell without engine access (FR65(b)); **Needs** names compose services that must be healthy (`deps` = `database` and `localstack`; `iso` = the target starts and removes the isolated environment itself); **ci** says whether `make ci` runs it. `$(X_CMD)` refers to `make/gates.mk`. A recipe line shown as `$(EXEC) …` is exactly that line.

Lifecycle:

| Target | Help text | Recipe | Prerequisites | Class / needs | ci |
| --- | --- | --- | --- | --- | --- |
| `help` | Show this help | `printf` of the three header lines, then `grep`/`sort`/`awk` over `$(MAKEFILE_LIST)` for `target: ## text` | none | HOST | no |
| `doctor` | Check host prerequisites and pinned tool versions | `MAKE_COMMAND="$(DOCTOR_MAKE)" scripts/doctor.sh` (the doctor checks the make that runs it) | none | HOST | yes, first step |
| `bootstrap` | Prepare a fresh clone: check, install tools, hooks, build, start, migrate | `MAKE_COMMAND="$(DOCTOR_MAKE)" scripts/doctor.sh --prerequisites`; `$(MAKE) install` in native mode, `scripts/install-bats.sh` when `EXEC` is not empty; `$(MAKE) install-hooks`; `MAKE_COMMAND="$(DOCTOR_MAKE)" scripts/doctor.sh`; `$(MAKE) start`; print the entry-point URLs. No local environment file has to be created: `.env` is committed | none | HOST; all services | no |
| `start` | Build missing or stale images, start the development environment and wait until it is healthy | `$(REQUIRE_DOCKER)`; `$(DOCKER_COMPOSE) up --detach --wait --wait-timeout 900 $(if $(filter 0,$(BUILD_IMAGE)),--no-build,--build)` | none | HOST | no |
| `up` | Alias of start | none | `start` | HOST | no |
| `start-deps` | Start the backing services named in DEPS if they are not running and wait until they are healthy | `$(REQUIRE_DOCKER)`; `$(DOCKER_COMPOSE) up --detach --wait --no-recreate $(DEPS)` | none | HOST | via the two `ensure-` targets |
| `ensure-deps` | Start PostgreSQL and LocalStack unless this shell is the development container | `@[ "$(IN_DEV_CONTAINER)" = 1 ] \|\| $(MAKE) --no-print-directory start-deps` | none | HOST | via prerequisites |
| `ensure-database` | Start PostgreSQL unless this shell is the development container | `@[ "$(IN_DEV_CONTAINER)" = 1 ] \|\| $(MAKE) --no-print-directory start-deps DEPS=database` | none | HOST | via prerequisites |
| `build` | Rebuild the development images from scratch | `$(DOCKER_COMPOSE) build --pull --no-cache` | none | HOST | no |
| `stop` | Stop the development environment | `$(DOCKER_COMPOSE) stop` | none | HOST | no |
| `down` | Remove the development containers and network (volumes are kept) | `$(DOCKER_COMPOSE) down --remove-orphans` | none | HOST | no |
| `sh` | Open a shell in the development container | `$(DOCKER_COMPOSE) exec --user $(HOST_UID):$(HOST_GID) app bash` | none | HOST; `app` | no |
| `logs` | Follow all logs | `$(DOCKER_COMPOSE) logs --follow` | none | HOST | no |
| `new-logs` | Follow only new log lines | `$(DOCKER_COMPOSE) logs --tail=0 --follow` | none | HOST | no |
| `purge` | Remove generated reports and load-test results | `rm -rf target/coverage target/sarif target/reports target/mutants.diff mutants.out mutants.out.old tests/Load/results/*.html tests/Load/results/aws` | none | HOST | no |
| `clean` | Alias of purge | none | `purge` | HOST | no |

Build:

| Target | Help text | Recipe | Prerequisites | Class / needs | ci |
| --- | --- | --- | --- | --- | --- |
| `install` | Install the pinned tools and Bats on this machine and fetch dependencies from the lock file | `scripts/install-tools.sh`; `scripts/install-bats.sh`; `cargo fetch --locked` | none | HOST | no |
| `update` | Update Cargo.lock within the version requirements of the manifests | `$(EXEC) cargo update` | none | TOOL | no |
| `build-prod-image` | Build the production image | `$(REQUIRE_DOCKER)`; `[ "$(BUILD_IMAGE)" = 0 ] \|\| $(DOCKER) build --target prod --tag $(APP_IMAGE) .` | none | HOST | yes |

Formatting, lint and analysis:

| Target | Help text | Recipe | Prerequisites | Class / needs | ci |
| --- | --- | --- | --- | --- | --- |
| `format` | Format the Rust code | `$(EXEC) cargo fmt --all` | none | TOOL | no |
| `format-check` | Check the Rust formatting (changes nothing) | `$(EXEC) $(FORMAT_CHECK_CMD)` | none | TOOL | yes (G1) |
| `format-repo` | Format TOML, YAML, JSON and the load-test JavaScript | `$(EXEC) taplo fmt`; Prettier with `--write` in the Node image, once with the arguments of `lint-prettier` and once in `tests/Load` with `--write --config /repo/.prettierrc.yml . "!results/**" "!pnpm-lock.yaml"` (the arguments of `lint-load-tests` with `--write` in place of `--check`) | none | TOOL + HOST | no |
| `lint-code` | Lint production code, then all targets with tests relaxed | `$(EXEC) $(LINT_PROD_CMD)`; `$(EXEC) $(LINT_ALL_CMD)` | none | TOOL | via `static-analysis` |
| `lint-rustdoc` | Lint the documentation comments | `$(EXEC) $(LINT_RUSTDOC_CMD)` | none | TOOL | via `static-analysis` |
| `static-analysis` | Run the strict static analysis with warnings denied | none | `lint-code lint-rustdoc` | TOOL | yes (G2) |
| `static-analysis-fix` | Apply every machine-applicable lint fix, then format | `$(EXEC) cargo clippy --fix --allow-dirty --allow-staged --locked --workspace --all-targets --all-features -- -D warnings`; `$(EXEC) cargo fmt --all` | none | TOOL | no |
| `static-analysis-sarif` | Write the lint findings as SARIF to target/sarif/clippy.sarif | `mkdir -p target/sarif`; `$(EXEC) bash -c 'cargo clippy --locked --workspace --all-targets --all-features --message-format=json -- -D warnings -A clippy::cognitive_complexity -A clippy::too_many_lines \| clippy-sarif > target/sarif/clippy.sarif'` (the pipeline's status is ignored with `\|\| true`; the gate is `static-analysis`) | none | TOOL | no |
| `complexity` | Check the complexity limits of production code | `$(EXEC) $(COMPLEXITY_CMD)` | none | TOOL | yes (G3) |
| `architecture` | Check layer and bounded-context boundaries | `$(EXEC) $(ARCH_CMD)` | none | TOOL | yes (G4) |
| `validate-configuration` | Check protected configuration files and required directories against base_ref | `$(EXEC) $(CONFIG_GUARD_CMD)` | none | TOOL | yes (G25) |
| `validate-manifest` | Check that Cargo.lock matches the manifests | `$(EXEC) $(MANIFEST_CMD)` | none | TOOL | yes (G5) |
| `check-crate-isolation` | Check that every crate builds alone, with each feature | `$(EXEC) $(HACK_CMD)` | none | TOOL | yes (G29) |
| `validate-sqlx-metadata` | Check that the offline query metadata in .sqlx is current | `$(EXEC) $(TEST_ENV) scripts/check-sqlx-metadata.sh $(SQLX_CHECK_CMD)` | `setup-test-db` | TOOL; `database`, test database | yes (G30) |
| `check-requirements` | Check that the service configuration is valid | `$(EXEC) $(CHECK_CONFIG_CMD)` | none | TOOL | yes (G6) |
| `unused-dependencies` | Fail on unused or misplaced dependencies | `$(EXEC) $(SHEAR_CMD)` | none | TOOL | yes (G8) |
| `unused-dependencies-fix` | Remove unused dependencies from the manifests | `$(EXEC) cargo shear --fix` | none | TOOL | no |

Tests:

| Target | Help text | Recipe | Prerequisites | Class / needs | ci |
| --- | --- | --- | --- | --- | --- |
| `unit-tests` | Run the unit tests and the documentation tests | `$(EXEC) $(UNIT_CMD)`; `$(EXEC) $(DOCTEST_CMD)` | none | TOOL | via coverage and `doc-tests` (G11) |
| `doc-tests` | Run the documentation tests | `$(EXEC) $(DOCTEST_CMD)` | none | TOOL | yes (G11) |
| `integration-tests` | Run the integration tests against PostgreSQL and LocalStack | `$(EXEC) $(TEST_ENV) $(INTEGRATION_CMD)` | `ensure-deps setup-test-db` | TOOL; `deps`, test database | via coverage (G12) |
| `e2e-tests` | Run the Gherkin end-to-end tests | `$(EXEC) $(TEST_ENV) $(E2E_CMD)` | `ensure-deps setup-test-db` | TOOL; `deps`, test database | yes (G13) |
| `all-tests` | Run unit, integration and end-to-end tests | none | `unit-tests integration-tests e2e-tests` | TOOL | no |
| `test` | Alias of all-tests | none | `all-tests` | TOOL | no |
| `bats` | Run the Makefile tests | `BATS_LIB_PATH="$(CURDIR)/.tools/bats/lib:/opt/bats/lib:/usr/lib/bats" $(BATS) $(BATS_ARGS) $(BATS_PATH)` where `BATS` is `.tools/bats/core/bin/bats` when it exists, else `bats` | none | HOST; Docker | yes (G16) |

Coverage and mutation:

| Target | Help text | Recipe | Prerequisites | Class / needs | ci |
| --- | --- | --- | --- | --- | --- |
| `tests-with-coverage` | Run the unit and integration tests with coverage and enforce 100% lines and functions | `mkdir -p target/coverage`; `$(EXEC) $(COVERAGE_CLEAN_CMD)`; `$(EXEC) $(TEST_ENV) $(COVERAGE_RUN_CMD)`; `$(EXEC) $(COVERAGE_LCOV_CMD)`; `$(EXEC) $(COVERAGE_HTML_CMD)`; `$(EXEC) $(COVERAGE_GATE_CMD)` | `ensure-deps setup-test-db` | TOOL; `deps`, test database | yes (G11, G12, G14) |
| `coverage-xml` | Write target/coverage/coverage.xml (Cobertura) and lcov.info without enforcing the floor | the clean command and the run command above, then `$(EXEC) $(COVERAGE_XML_CMD)`; `$(EXEC) $(COVERAGE_LCOV_CMD)` | `ensure-deps setup-test-db` | TOOL; `deps`, test database | no |
| `coverage-html` | Write the HTML coverage report to target/coverage/html | the clean command and the run command above, then `$(EXEC) $(COVERAGE_HTML_CMD)` | `ensure-deps setup-test-db` | TOOL; `deps`, test database | no |
| `mutation-tests` | Run mutation testing over all production code (fails on a surviving or timed-out mutant) | `$(EXEC) $(TEST_ENV) $(MUTANTS_CMD)` | `ensure-deps setup-test-db` | TOOL; `deps`, test database | no (main, schedule) |
| `mutation-tests-diff` | Run mutation testing over the code changed against base_ref | `$(EXEC) $(TEST_ENV) scripts/mutation-diff.sh $(base_ref) -- $(MUTANTS_CMD)` | `ensure-deps setup-test-db` | TOOL; `deps`, test database | yes (G15) |

Contracts:

| Target | Help text | Recipe | Prerequisites | Class / needs | ci |
| --- | --- | --- | --- | --- | --- |
| `generate-openapi-spec` | Write the OpenAPI snapshot .github/openapi-spec/spec.yaml | `$(EXEC) $(OPENAPI_EXPORT_CMD)` | none | TOOL | via snapshots |
| `generate-graphql-spec` | Write the GraphQL snapshot .github/graphql-spec/spec | `$(EXEC) $(GRAPHQL_EXPORT_CMD)` | none | TOOL | via snapshots |
| `validate-contract-snapshots` | Fail when a committed contract snapshot differs from the generated one | `@$(SNAPSHOT_STATUS_CMD) \|\| { git status --short -- .github/openapi-spec .github/graphql-spec; echo 'A committed contract snapshot differs from the generated one or is not tracked.' >&2; exit 1; }` | `generate-openapi-spec generate-graphql-spec` | TOOL + HOST | yes (G17) |
| `validate-openapi-spec` | Lint the OpenAPI snapshot with Spectral | `scripts/validate-openapi-spec.sh` | none | HOST | yes (G18) |
| `openapi-diff` | Fail on a breaking OpenAPI change against base_ref | `scripts/openapi-diff.sh $(base_ref)` | none | HOST | yes (G19) |
| `graphql-diff` | Fail on a breaking GraphQL change against base_ref | `scripts/graphql-diff.sh $(base_ref)` | none | HOST | yes (G20) |
| `schemathesis-validate` | Test the production image against the OpenAPI snapshot with Schemathesis | caller did not set `SCHEMATHESIS_URL`: `scripts/with-load-test-stack.sh -- $(SCHEMATHESIS_RUN)`; else `$(SCHEMATHESIS_RUN)`, where `SCHEMATHESIS_RUN = $(DOCKER) run --rm --network host -v "$(CURDIR)/.github/openapi-spec:/data:ro" $(SCHEMATHESIS_IMAGE) run /data/spec.yaml --url $(SCHEMATHESIS_URL) $(SCHEMATHESIS_FLAGS)` | none | HOST; `iso` | yes (G21) |

Load tests:

| Target | Help text | Recipe | Prerequisites | Class / needs | ci |
| --- | --- | --- | --- | --- | --- |
| `smoke-load-tests` | Run every load scenario with the smoke profile | `scripts/with-load-test-stack.sh -- tests/Load/run-smoke-load-tests.sh` | none | HOST; `iso` | yes (G22) |
| `average-load-tests` | Run every load scenario with the average profile | same with `run-average-load-tests.sh` | none | HOST; `iso` | no |
| `stress-load-tests` | Run every load scenario with the stress profile | same with `run-stress-load-tests.sh` | none | HOST; `iso` | no |
| `spike-load-tests` | Run every load scenario with the spike profile | same with `run-spike-load-tests.sh` | none | HOST; `iso` | no |
| `load-tests` | Run every load scenario with all four profiles | same with `run-load-tests.sh` | none | HOST; `iso` | no |
| `execute-load-tests-script` | Run one load scenario, all four profiles unless switched off: scenario=<name> runSmoke=false runAverage=false runStress=false runSpike=false | `test -n "$(scenario)"` (else the usage line `Usage: make execute-load-tests-script scenario=<name> [runSmoke=false] [runAverage=false] [runStress=false] [runSpike=false] (each profile runs unless its variable is false)`, exit 2, before anything starts); `scripts/with-load-test-stack.sh -- tests/Load/execute-load-test.sh "$(scenario)" $(runSmoke) $(runAverage) $(runStress) $(runSpike)` | none | HOST; `iso` | no |
| `aws-load-tests` | Run the load tests on a temporary AWS instance | `LOCAL_MODE_ENV=$(LOCAL_MODE_ENV) tests/Load/aws-execute-load-tests.sh` followed by one option per set parameter | none | HOST; AWS credentials in the environment, or `localstack` in local mode | no |
| `aws-load-tests-cleanup` | Remove everything aws-load-tests created | `LOCAL_MODE_ENV=$(LOCAL_MODE_ENV) tests/Load/cleanup.sh` with the same options | none | HOST | no |
| `lint-load-tests` | Lint and format-check the load-test JavaScript | `$(NODE_RUN) sh -c 'corepack pnpm install --frozen-lockfile --ignore-scripts --store-dir /tmp/pnpm-store && corepack pnpm exec eslint . && corepack pnpm exec prettier --check --config /repo/.prettierrc.yml . "!results/**" "!pnpm-lock.yaml"'` where `NODE_RUN = $(DOCKER_RUN) -e HOME=/tmp -e COREPACK_HOME=/tmp/corepack -v "$(CURDIR):/repo" -w /repo/tests/Load $(NODE_IMAGE)`; the install command is the Makefile variable `PNPM_INSTALL`, and `--store-dir /tmp/pnpm-store` keeps the pnpm store in the container, outside the bind-mounted checkout; `--config` names the protected root file, which switches off Prettier's configuration discovery, so a `prettier` key in `tests/Load/package.json` or a configuration file that the guard does not report cannot override it | none | HOST | yes (G24) |

Database:

| Target | Help text | Recipe | Prerequisites | Class / needs | ci |
| --- | --- | --- | --- | --- | --- |
| `migrate` | Apply all pending migrations to the development database | `$(EXEC) cargo run --locked --quiet -p service -- migrate` | `ensure-database` | TOOL; `database` | no |
| `migrate-rollback` | Revert the most recently applied migration | `$(EXEC) cargo run --locked --quiet -p service -- migrate revert` | `ensure-database` | TOOL; `database` | no |
| `create-migration` | Create a reversible migration pair: name=<snake_case_description> | `test -n "$(name)"` (else usage, exit 2); `$(EXEC) sqlx migrate add -r $(name)` | none | TOOL | no |
| `setup-test-db` | Drop, create and migrate the test database | `$(EXEC) $(TEST_ENV) scripts/setup-test-db.sh` | `ensure-database` | TOOL; `database` | yes |
| `setup-load-test-db` | Drop, create and migrate the database of the isolated load-test environment | `scripts/setup-load-test-db.sh` | none | HOST; `iso` running | via the wrapper |
| `sqlx-prepare` | Regenerate the offline query metadata in .sqlx from the freshly migrated test database | `$(EXEC) $(TEST_ENV) cargo sqlx prepare --workspace -- --all-targets --all-features` | `setup-test-db` | TOOL; `database`, test database | no |

Security:

| Target | Help text | Recipe | Prerequisites | Class / needs | ci |
| --- | --- | --- | --- | --- | --- |
| `check-security` | Check advisories, licences, bans and sources of the locked dependencies | `$(EXEC) $(DENY_CMD)` | none | TOOL; network | yes (G7) |
| `check-security-offline` | Check licences, bans and sources without network access | `$(EXEC) $(DENY_OFFLINE_CMD)` | none | TOOL | no (hook) |
| `secrets-scan` | Scan the history and the working files for secrets | `scripts/secrets-scan.sh` | none | HOST | yes (G9) |
| `image-scan` | Scan the production image for HIGH and CRITICAL vulnerabilities | `$(if $(IMAGE_TO_SCAN),,$(MAKE) build-prod-image)`; `scripts/image-scan.sh "$(or $(IMAGE_TO_SCAN),$(APP_IMAGE))"` | none | HOST; network | yes (G10) |

Docs and architecture model:

| Target | Help text | Recipe | Prerequisites | Class / needs | ci |
| --- | --- | --- | --- | --- | --- |
| `docs-check` | Check that commands, paths and lists cited in the documentation exist | `mkdir -p target/reports/docs-check`; one line per name of `root serve migrate healthcheck`: `$(EXEC) cargo run --locked --quiet -p service -- <command> --help > target/reports/docs-check/<name>.txt` (`root` is `--help` alone); `scripts/check-docs.sh --service-help target/reports/docs-check` | none | TOOL + HOST | yes (G26) |
| `validate-architecture-model` | Validate workspace.dsl, its three views, its components and a relationship for every dependency between them | `$(EXEC) cargo tree --locked --workspace --depth 0 --prefix none \| scripts/validate-architecture-model.sh workspace.dsl -` | none | TOOL + HOST | yes (G27) |

Repository lint:

| Target | Help text | Recipe | Prerequisites | Class / needs | ci |
| --- | --- | --- | --- | --- | --- |
| `lint-repo` | Run every repository linter | none | `lint-actions lint-yaml lint-markdown lint-shell lint-docker lint-toml lint-spelling lint-prettier` | HOST + TOOL | yes (G23) |
| `lint-actions` | Lint the workflows (syntax and security) | `$(DOCKER_RUN) -v "$(CURDIR):/repo:ro" -w /repo $(ACTIONLINT_IMAGE) -color`; `$(EXEC) zizmor --offline --config .github/zizmor.yml .github/workflows` | none | HOST + TOOL | via `lint-repo` |
| `lint-yaml` | Lint the YAML files | `$(DOCKER_RUN) -e HOME=/tmp -v "$(CURDIR):/repo:ro" -w /repo $(PYTHON_IMAGE) sh -c 'pip install --quiet --disable-pip-version-check --no-cache-dir --target /tmp/yamllint yamllint==$(YAMLLINT_VERSION) && PYTHONPATH=/tmp/yamllint python -m yamllint --strict .'` | none | HOST | via `lint-repo` |
| `lint-markdown` | Lint the Markdown files | `$(DOCKER_RUN) -v "$(CURDIR):/workdir:ro" $(MARKDOWNLINT_IMAGE) "**/*.md"` | none | HOST | via `lint-repo` |
| `lint-shell` | Lint shell scripts, hooks and Bats files | `$(DOCKER_RUN) -v "$(CURDIR):/mnt:ro" -w /mnt $(SHELLCHECK_IMAGE) --severity=style $$(git ls-files --cached --others --exclude-standard -- '*.sh' '*.bash' '*.bats' '.githooks/*')` | none | HOST | via `lint-repo` |
| `lint-docker` | Lint the Dockerfile | `$(DOCKER_RUN) -v "$(CURDIR):/repo:ro" -w /repo $(HADOLINT_IMAGE) hadolint --config .hadolint.yaml $(DOCKERFILES)` | none | HOST | via `lint-repo` |
| `lint-toml` | Check the TOML formatting | `$(EXEC) taplo fmt --check` | none | TOOL | via `lint-repo` |
| `lint-spelling` | Check spelling | `$(EXEC) typos` | none | TOOL | via `lint-repo` |
| `lint-prettier` | Check the formatting of YAML and JSON | `$(NODE_RUN) sh -c 'corepack pnpm install --frozen-lockfile --ignore-scripts --store-dir /tmp/pnpm-store && corepack pnpm exec prettier --check --config /repo/.prettierrc.yml --ignore-path /repo/.gitignore --ignore-path /repo/.prettierignore /repo'` (`NODE_RUN` as in `lint-load-tests`; both ignore files are named because one `--ignore-path` switches off Prettier's default use of `.gitignore`) | none | HOST | via `lint-repo` |
| `validate-invariants` | Check the repository invariants: pins, versions, gate set, repository name, licence, suppressions, execution switch | `scripts/check-invariants.sh` | none | HOST | yes (G31) |

Hooks and tooling:

| Target | Help text | Recipe | Prerequisites | Class / needs | ci |
| --- | --- | --- | --- | --- | --- |
| `install-hooks` | Activate the git hooks of .githooks | `git config core.hooksPath .githooks` | none | HOST | no |
| `pre-commit` | Fast gates run by the pre-commit hook | none | `format-check lint-code architecture check-security-offline unused-dependencies` | TOOL | no |
| `pre-push` | Gates run by the pre-push hook | none | `unit-tests` | TOOL | no |
| `check-commit-message` | Check the commit message in COMMIT_MSG_FILE or the title in COMMIT_TITLE | `scripts/check-commit-message.sh` | none | HOST | no |

bmalph, scaffolding and aggregates:

| Target | Help text | Recipe | Prerequisites | Class / needs | ci |
| --- | --- | --- | --- | --- | --- |
| `bmalph-install` | Install and verify bmalph for BMALPH_PLATFORM=claude-code\|codex | `bash scripts/install-bmalph.sh --platform "$(BMALPH_PLATFORM)"` | none | HOST; npm | no |
| `bmalph-claude` | Install and verify bmalph for Claude Code | `@$(MAKE) bmalph-install BMALPH_PLATFORM=claude-code` | none | HOST | no |
| `bmalph-codex` | Install and verify bmalph for Codex | `@$(MAKE) bmalph-install BMALPH_PLATFORM=codex` | none | HOST | no |
| `bmalph-init` | Initialise bmalph in this project; BMALPH_DRY_RUN=true previews | `bash scripts/install-bmalph.sh --platform "$(BMALPH_PLATFORM)" --init $(if $(filter true TRUE 1 yes YES,$(BMALPH_DRY_RUN)),--dry-run,)` | none | HOST | no |
| `bmalph-setup` | Install and initialise bmalph in one step | `@$(MAKE) bmalph-init BMALPH_PLATFORM="$(BMALPH_PLATFORM)" BMALPH_DRY_RUN="$(BMALPH_DRY_RUN)"` | none | HOST | no |
| `new-context` | Create a bounded context: group=<group> context=<context> | `test -n "$(group)" -a -n "$(context)"` (else usage, exit 2); `$(EXEC) cargo xtask new-context $(group) $(context)` (the command refreshes `Cargo.lock` itself, core AD-26) | none | TOOL | no |
| `ci` | Run every gate in order and print the success line | `@$(MAKE) --no-print-directory doctor`; `@for stage in $(CI_STAGES); do $(MAKE) --no-print-directory $$stage; done`; `@echo "CI checks successfully passed!"` | none | all | – |
| `all`, `ci-sequential` | Alias of ci | none | `ci` | all | – |
| `ci-list` | Print the ordered gate targets of make ci, one per line | `@printf '%s\n' $(CI_GATES)` (`CI_GATES` is the concatenation of the seven `CI_<STAGE>_TARGETS` variables) | none | HOST | no |
| `ci-preflight`, `ci-static-analysis`, `ci-tests`, `ci-contracts`, `ci-mutation`, `ci-delivery`, `ci-cli` | Stage N of make ci | `$(call STAGE_RECIPE,$(CI_<STAGE>_TARGETS))`, which expands to one `@$(MAKE) --no-print-directory <target>` line per target of DD-02, in order, from the variable that `ci-list`, rule (d) of `scripts/check-invariants.sh` and `AGENTS.md` read; the first failing gate stops the stage. The generated lines hold no literal `$(MAKE)` in the Makefile text, so GNU Make does not treat them as recursive: `make -n ci` prints, after the `doctor` line, the gate lines, exactly those of `make ci-list`, without running the gates' own recipes, and `make -j<N> ci` runs the nested make of each gate with `-j1` (warning `jobserver unavailable`); the gates of a stage run in sequence anyway, and no hosted job passes `-j` | none | as their targets | yes |

Bats coverage of the targets (FR67(a); the same lists are in `docs/make-targets.md`):

- **Gate targets** have their negative test in the suite and no second passing run: the passing run is `make ci` stage 1 to 6 and the CI job of the gate (DD-02).
- **Targets without a Bats test, with the reason:** `aws-load-tests` without local mode and `aws-load-tests-cleanup` against AWS (no credentials in CI; the LocalStack dry run is tested), `average-load-tests`, `stress-load-tests`, `spike-load-tests`, `load-tests` (minutes of load; the shared runner path is tested through `smoke-load-tests` and `execute-load-tests-script`), `mutation-tests` (up to an hour; it is `MUTANTS_CMD` without the diff, and the same command line is tested through `mutation-tests-diff`), `coverage-xml` and `coverage-html` (a second and third instrumented run of the suites; their report commands are part of the tested `tests-with-coverage` sequence or differ from it in the output format only), `bootstrap` (run by the scheduled workflow), `update` (rewrites the lock file), `build` (minutes; its command line is tested with `make -n` in `make_execution_mode.bats`), `build-prod-image` (minutes of image build; stage 6 of `make ci` runs it, and the hosted jobs build the production image with the cached build action), `format`, `format-repo`, `static-analysis-fix`, `unused-dependencies-fix`, `sqlx-prepare` (fix-style; they rewrite tracked files), `ci` in full (tested with `CI_STAGES` and `make -n`).
- **Aliases and helper targets, which have no test of their own** because the target they point at, or the target that calls them, carries the test: the aliases `up` (of `start`), `clean` (of `purge`), `test` (of `all-tests`), `all` and `ci-sequential` (of `ci`); the helper targets `ensure-deps` and `ensure-database` (prerequisites of the tested targets that need services), `setup-load-test-db` (called by the isolated-environment wrapper), `check-security-offline` (a prerequisite of `pre-commit`; the offline subset of the tested `check-security`), `bmalph-claude` and `bmalph-codex` (call `bmalph-install` with a fixed platform), the stage targets `ci-preflight`, `ci-static-analysis`, `ci-tests`, `ci-contracts`, `ci-mutation`, `ci-delivery` and `ci-cli` (their order is checked through a dry run of `make ci`), `static-analysis-sarif` (writes the lint findings in a publication format; the gate is `static-analysis`), `lint-code` and `lint-rustdoc` (the two halves of `static-analysis`), `all-tests`, `unit-tests` and `integration-tests` (compositions of the suites that the gates G11 to G13 run), `install` (downloads tools; run by `bootstrap`), `pre-push` (runs `unit-tests`) and `sh` (opens an interactive shell).

A test of `make_execution_mode.bats` fails when a target of `make help` is neither invoked by a Bats test nor named in the gate table or the section of targets without a Bats test of `docs/make-targets.md`.

### PHP target to Rust target

All 68 PHP targets. "Kept" means same name, same capability.

| PHP target | Rust target | Note |
| --- | --- | --- |
| `help`, `start`, `up`, `build`, `stop`, `down`, `sh`, `logs`, `new-logs`, `purge`, `clean` | kept | `start` now rebuilds stale images incrementally; `purge` removes real generated paths |
| `install`, `update` | kept | pinned tools plus `cargo fetch --locked`; `cargo update` |
| `composer-validate` | renamed `validate-manifest` | lock file check; the each-crate build is the new `check-crate-isolation` |
| `check-requirements`, `check-security` | kept | `service check-config`; `cargo deny --locked check` |
| `commands` | dropped | `make help` and `service --help` list everything |
| `cache-clear`, `cache-warmup` | dropped | no framework cache exists |
| `doctrine-migrations-migrate` | renamed `migrate` | plus `migrate-rollback` |
| `doctrine-migrations-generate` | renamed `create-migration` | reversible pair through `sqlx migrate add -r` |
| `setup-test-db`, `setup-load-test-db` | kept | the second acts only on the isolated environment |
| `load-fixtures` | dropped | catalog CAP-24: the PHP fixture loaded nothing |
| `phpcsfixer` | renamed `format` (fix) and `format-check` (gate) | DD-03 |
| `psalm` | renamed `static-analysis` | |
| `psalm-security` | replaced by `static-analysis`, `check-security` and the CodeQL job | no taint tool runs locally (DD-24) |
| `psalm-security-sarif` | renamed `static-analysis-sarif` | |
| `phpmd` | renamed `complexity` | |
| `phpinsights` | replaced by `format-check`, `static-analysis`, `complexity`, `architecture` | no composite score exists for Rust |
| `deptrac` | renamed `architecture` | |
| `deptrac-debug` | merged into `architecture` | rule `arch::uncovered` names every unassigned crate |
| `rector-apply` | renamed `static-analysis-fix` | |
| `rector-ci` | merged into `static-analysis` | a pending machine-applicable fix is a finding |
| `validate-configuration` | kept | now `cargo xtask config-guard` |
| `unit-tests`, `integration-tests`, `all-tests`, `test`, `bats` | kept | |
| `behat` | renamed `e2e-tests` | |
| `tests-with-coverage`, `coverage-html`, `coverage-xml` | kept | one output directory, `target/coverage/` |
| `infection` | renamed `mutation-tests` | plus `mutation-tests-diff` |
| `generate-openapi-spec`, `generate-graphql-spec` | kept | need no running service; both in `make help` |
| `build-spectral-docker` | dropped | the stock Spectral image runs with `--user` |
| `validate-openapi-spec`, `openapi-diff`, `schemathesis-validate` | kept | oasdiff fails on breaking changes; Schemathesis tests the production image |
| `build-k6-docker` | dropped | the stock K6 image is used |
| `smoke-load-tests`, `average-load-tests`, `stress-load-tests`, `spike-load-tests`, `load-tests` | kept | isolated environment |
| `execute-load-tests-script` | kept | starts the isolated environment; rejects an empty `scenario` |
| `aws-load-tests`, `aws-load-tests-cleanup` | kept | options can be passed through Make |
| `ci`, `all`, `ci-sequential` | kept | DD-02 |
| `ci-preflight`, `ci-static-analysis`, `ci-mutation` | kept | contents redefined; no mutating step |
| `ci-deptrac` | merged into `ci-static-analysis` | |
| `ci-tests-and-openapi` | split into `ci-tests` and `ci-contracts` | |

New targets without a PHP counterpart: `doctor`, `bootstrap`, `start-deps`, `ensure-deps`, `ensure-database`, `build-prod-image`, `format-repo`, `lint-code`, `lint-rustdoc`, `check-crate-isolation`, `validate-sqlx-metadata`, `doc-tests`, `unused-dependencies`, `unused-dependencies-fix`, `mutation-tests-diff`, `validate-contract-snapshots`, `graphql-diff`, `lint-load-tests`, `migrate-rollback`, `sqlx-prepare`, `check-security-offline`, `secrets-scan`, `image-scan`, `docs-check`, `validate-architecture-model`, the ten `lint-*`/`validate-invariants` targets, `install-hooks`, `pre-commit`, `pre-push`, `check-commit-message`, the five `bmalph-*` targets, `new-context`, `ci-list`, `ci-delivery`, `ci-cli`. Removed since the first version because no requirement asks for them: `run` (ran cargo on the host outside the execution switch; `cargo run -p service -- serve` works without a wrapper) and `migrate-status` (its command was removed from the core document).

### Scripts and commands the targets call

All shell scripts are Bash with `set -euo pipefail`, mode 755, pass shellcheck, take no input from an env file except where stated, and use exit status 0 for success, 1 for a finding and 2 for "could not run" unless noted. Scripts read tool images, tool versions and gate flags from the environment, where the Makefile exports them (`make/tool-versions.mk`, the `export` lines of `make/gates.mk`, the header). A script that a recipe calls outside `$(EXEC)` starts no cargo command and no cargo tool (G31 rule (h)); the two exceptions are the installer and the preflight.

| Path | Purpose | Inputs | Exit status |
| --- | --- | --- | --- |
| `scripts/with-env.sh` | Export each `NAME=value` of the given env files (an `export ` prefix is accepted), in order: a later file overrides an earlier one, a variable that the caller has already set is never changed, and `${NAME}` references are expanded. Values are read as dotenvy 0.15.7 and Compose read them for the forms on which the two agree: a value in double quotes loses the quotes (`\"`, `\\` and `\n` are unescaped), a value in single quotes loses the quotes and stays literal, and an unquoted value ends at whitespace followed by `#`; the forms that the two read differently (an unquoted value with whitespace, a quote or a backslash, a `$` that does not start `${NAME}`, an unknown escape, an unclosed quote, a comment right after the `=`) are refused. Then `exec` the command | `<env-file>… -- <command…>` | the command's; 2 when a file is missing, the separator is absent, no command follows it, or a line is refused (`with-env.sh: invalid value in <file>: <line>`) |
| `scripts/doctor.sh` | Check the host prerequisites and their minimum versions: Docker Engine 25.0 (needed for the healthcheck key `start_interval`), Compose plugin 2.24 (needed for `!reset`), Buildx 0.12, GNU Make 4.0 (the make named by `MAKE_COMMAND`, default `make`, which the `doctor` and `bootstrap` recipes set to the make that runs them; when it is missing or too old, the line names `brew install make` and the `gnubin` directory for macOS), Bash 4.4, git 2.30, curl 7.76, rustup 1.26 with the toolchain of `rust-toolchain.toml` installed and active (decided by the exit status of `RUSTUP_AUTO_INSTALL=0 rustc --version`, the version read only from the line that starts with `rustc `; a toolchain that is not installed gives `[MISSING] toolchain <channel>: needed by every cargo target; run: rustup toolchain install <channel>`), every cargo tool at its pinned version, cocogitto included (`cog --version` against `COCOGITTO_VERSION`, needed by `make_release.bats`), Bats. Reported as optional: Node.js 20 with npm (the bmalph targets only; every other JavaScript tool runs from the pinned Node image) and `jq` (the assistant hook). When `RUN_IN_CONTAINER` is `1` or `true` and `CI` is empty, the rustup, toolchain and cargo-tool checks are replaced by the line `[SKIP] toolchain and cargo tools: provided by the development container`. Prints the platform first (`Platform: linux/x86_64 (verified in CI)`, or `(not verified in CI)` for Linux aarch64, macOS and WSL 2), then one line per check (`[OK]`, `[MISSING] <name>: needed by <targets>; <how to install>`, `[OUTDATED] <name>: found <v>, need <v>; needed by <targets>; <how to install>`; an installed toolchain whose `rustc --version` differs from the channel gives `[OUTDATED] toolchain: found <v>, need <channel>; needed by every cargo target; run: rustup toolchain install <channel>`) | `--prerequisites` limits the check to what `make install` itself needs | 0 all present; 1 otherwise |
| `scripts/install-tools.sh` | Install the cargo tools of `make/tool-versions.mk` (DD-04) | `--with-watchexec` (development image); `GITHUB_TOKEN` optional | 0; 1 a tool could not be installed |
| `scripts/install-bats.sh` | Install Bats and three helper libraries at pinned commits (DD-06) | `--prefix <dir>` (default `.tools/bats`) | 0; 1 SHA mismatch or clone failure |
| `scripts/setup-test-db.sh` | Refuse, before anything else, a database whose name does not end in `_test` (`refusing to reset the database '<name>': the name of a test database must end in _test`); check that `sqlx` exists and the database accepts connections (10 attempts, 1 s apart), then run exactly `sqlx database reset -y -f --source migrations`. Called as a whole behind `$(EXEC) $(TEST_ENV)` | `DATABASE_URL` of `.env.test` through `with-env.sh` | 0; 1 reset failed; 2 `DATABASE_URL` unset, a database that is not a test database, database unreachable or `sqlx` missing |
| `scripts/check-sqlx-metadata.sh` | Run the query-metadata check given as its arguments, print its output and keep its exit status; fail with `check-sqlx-metadata.sh: .sqlx holds a query file that no current query generates; ...` when the output contains `potentially unused queries found in .sqlx`, which sqlx-cli 0.9.0 prints as a warning and then succeeds. Called behind `$(EXEC) $(TEST_ENV)` | the command line | the command's status; 1 an unused query file; 2 no command |
| `scripts/setup-load-test-db.sh` | In project `$LOAD_TEST_PROJECT` only: `dropdb --if-exists --force`, `createdb` in the `database` container, then `docker compose run --rm --no-deps migrate`; refuses the name of the development project | `LOAD_TEST_PROJECT` | 0; 1 a step failed; 2 the project is not running or is the development project |
| `scripts/with-load-test-stack.sh` | Build the production image unless `BUILD_IMAGE=0`; `up --detach --wait --wait-timeout 120 --no-build` the isolated project; `trap` `down --volumes --remove-orphans` on exit; run `setup-load-test-db.sh`; export `API_HOST=localhost`, `API_PORT=$LOAD_TEST_APP_PORT`; run the command | `-- <command…>`; `LOAD_TEST_PROJECT`, `LOAD_TEST_APP_PORT`, `APP_IMAGE`, `BUILD_IMAGE` | the command's (when it is not 0, a line naming the project and status and the project's logs are printed to standard error before the removal); 2 the environment did not become healthy (logs are printed) |
| `scripts/validate-openapi-spec.sh` | Spectral lint of `.github/openapi-spec/spec.yaml` with `.spectral.yaml`, read-only mounts, `--user`, `$SPECTRAL_FLAGS`; `--format github-actions` when `GITHUB_ACTIONS` is set | `SPECTRAL_IMAGE`, `SPECTRAL_FLAGS` | 0; 1 findings; 2 snapshot or ruleset missing |
| `scripts/openapi-diff.sh` | Extract the base snapshot with `git show <base>:<path>` into a temporary directory and run `oasdiff breaking /base/spec.yaml /head/spec.yaml $OASDIFF_FLAGS` in the image `OPENAPI_DIFF_IMAGE` | `[base_ref=origin/main]`; `OASDIFF_FLAGS`, `BREAKING_CHANGE_APPROVED` | 0 compatible, or no snapshot on the base (notice), or approved (findings printed); 1 breaking change, with the line `Approve with the label approved-breaking-change`; 2 base ref not resolvable |
| `scripts/graphql-diff.sh` | Same for `.github/graphql-spec/spec` with `npx --yes @graphql-inspector/cli@$GRAPHQL_INSPECTOR_VERSION diff` in the Node image; both snapshots are mounted as `spec.graphql`, because graphql-inspector 7.0.2 cannot load a file without that extension | same, without flags | same |
| `scripts/mutation-diff.sh` | Resolve the base ref; write `git diff --no-ext-diff --no-textconv --no-color --no-relative --no-renames --unified=3 --src-prefix=a/ --dst-prefix=b/ "$(git merge-base <base> HEAD)" -- '*.rs'` to `target/mutants.diff`, with `-c` pins for `core.quotePath`, `diff.mnemonicPrefix`, `diff.noprefix`, `diff.relative`, `diff.renames`, `diff.context`, `color.ui` and `color.diff` and with `GIT_DIFF_OPTS` and `GIT_EXTERNAL_DIFF` removed from the environment, so the user's git configuration cannot change the file; run the given mutation command with `--in-diff target/mutants.diff` appended. With `--list` it appends `--list` as well and prints only the number of mutants. Called as a whole behind `$(EXEC) $(TEST_ENV)`; contains no cargo command of its own | `<base_ref> [--list] -- <mutation command…>` | cargo-mutants' (0, 2 survivors, 3 timeouts); 2 with `mutation-tests-diff could not run: base ref <ref> is not available` |
| `scripts/validate-architecture-model.sh` | `validate -workspace` in the Structurizr image; `export -format json` to `target/reports/structurizr/`; require one system-context, one container and one component view; read the names of the elements shown in the component view from the export (a JSON query run in the Node image) and compare them with the crate list, minus `xtask`, `test-support` and `e2e-tests`; print every missing and every extra component; then require a relationship between the two components for every workspace-internal entry of a `[dependencies]` table (also `[target.<cfg>.dependencies]` and `[dependencies.<name>]`, a quoted key, a `package =` rename) of a crate manifest below the current directory whose crate and dependency are both modelled, and print each missing one as `missing relationship: <crate> -> <dependency>` | `<dsl-file> <crate-list-file>` (`-` reads the list from standard input: one crate per line, the first word is the name); `STRUCTURIZR_IMAGE`, `NODE_IMAGE` | 0; 1 parse error, a view missing, a component difference or a missing relationship; 2 usage error or a tool that could not run |
| `scripts/secrets-scan.sh` | `gitleaks git` over the history, then `gitleaks dir` over a temporary copy of `git ls-files --cached --others --exclude-standard`; config `.gitleaks.toml`; `--redact` | `GITLEAKS_IMAGE` | 0; 1 leak found |
| `scripts/image-scan.sh` | `docker save` the image to `target/reports/image-scan/image.tar`, run Trivy on the archive with `$TRIVY_FLAGS` and its database in `~/.cache/trivy` | `<image>`; `TRIVY_IMAGE`, `TRIVY_FLAGS` | 0; 1 findings; 2 image not found |
| `scripts/check-docs.sh` | Documentation-drift check, rules (1) to (7) of DD-17; reads the help texts of the binary from a directory | `[--root <dir>] --service-help <dir>` | 0; 1 with one line per violation, prefixed with the rule number |
| `scripts/check-invariants.sh` | Repository invariants (G31, FR125), one rule per letter, listed below | `[--root <dir>]` | 0; 1 with one line per violation, prefixed with the rule letter |
| `scripts/check-commit-message.sh` | Commit convention check ("Git Hooks and Commit Convention") | environment: `COMMIT_MSG_FILE` (hook mode) or `COMMIT_TITLE` (title mode) | 0; 1 with the expected format; 2 neither variable set |
| `scripts/install-bmalph.sh` | Install bmalph at `BMALPH_VERSION`, verify and optionally initialise it ("AI Guidance and Documentation") | `--platform`, `--init`, `--dry-run`, `--upgrade`, `--skip-verify` | 0; 1 |
| `scripts/set-version.sh` | Set `[workspace.package].version` in `Cargo.toml`, run `cargo update --workspace`, verify with `cargo metadata --locked` (release job only) | `<version>` | 0; 1 |
| `scripts/release.sh` | Release flow ("CI/CD Workflows") | `[--version <x.y.z>]` | 0 released or nothing to release; 1 failure |
| `scripts/ci/detect-changes.sh` | Print `code=true\|false` for a pull request (`false` only for a documentation-only change) | `BASE_SHA`, `HEAD_SHA` | 0 |
| `scripts/ci/summarize-tests.sh` | Write one line per gate (G11, G12, G14) with its result to the job summary, from `target/nextest/ci/junit.xml` (a suite whose nextest binary id ends in `::integration` belongs to G12, every other suite to G11) and `target/coverage/lcov.info` (G14 passes when every `LF` equals `LH` and every `FNF` equals `FNH`); a failed test step whose reports show nothing wrong is reported as a failure of G11 (the documentation tests) | `GITHUB_STEP_SUMMARY`; optional `TESTS_OUTCOME` (the outcome of the test step) | 0 |
| `scripts/ci/mutation-plan.sh` | Count the mutants (pull request: `scripts/mutation-diff.sh "origin/$BASE_REF" --list -- cargo mutants --workspace`; otherwise `cargo mutants --workspace --list`) and print `count=<n>` and `shards=<JSON>`: one `{"index": k, "shard": "<k-1>/n"}` object per shard (`index` counts from 1 to n and names the artifact and the shard that saves the Rust cache; `shard` is the 0-based value of `cargo mutants --shard`, for example `{"index":1,"shard":"0/3"}`), with n = the count divided by 150, rounded up, at least 1 and at most 8; `shards=[]` for 0 mutants | `EVENT_NAME`, `BASE_REF` | 0; 2 base ref not resolvable |
| `scripts/claude/guard-protected-files.sh` | Assistant hook that blocks edits of protected items (DD-18) | `pre` or `post`; hook JSON on standard input | 0 allow; 2 block |
| `cargo xtask config-guard` | The configuration guard (core AD-25); `--list` prints the protected items | `--base <ref>`, `CONFIG_CHANGE_APPROVED` | 0; 1; 2 |
| `cargo xtask new-context` | The bounded-context scaffolder (core AD-26) | `<group> <context>` | 0 created; 1 refused; 2 could not run |
| `cargo xtask arch` | The architecture gate (core AD-03) | `--manifest-path`, `--config` | 0; 1; 2 |

Rules of `scripts/check-invariants.sh` (each has a negative test in `check_invariants.bats`):

| Rule | Fails when |
| --- | --- |
| (a) | a `uses:` is not pinned to a 40-character commit SHA; a workflow declares no `permissions`, no `concurrency` or a job without `timeout-minutes`; the top-level `permissions` block grants a write permission, `write-all` included (NFR14: the top-level token is read-only, and write permissions belong to the jobs that need them); a job-level `permissions` block lacks `contents: read` (a job-level block replaces the workflow-level one), the release and sync jobs excepted; a job-level `permissions` block grants a scope `write` (an entry of the block mapping, quoted or not, or a member of a flow mapping, on the line of the key, on the line after it or broken over several lines as prettier writes a long one; the value of `permissions:` ends at the next key of the job, so the `with:` inputs of a step such as `permission-contents: write` of `create-github-app-token` are no scopes of the job token), except `security-events: write` in the code-scanning jobs `codeql`, `clippy-sarif-upload` and `code-scanning` and any scope in the jobs `release` and `sync` of a workflow without a `pull_request` or `pull_request_target` trigger (NFR14); a job-level `permissions: write-all`, in every job, because it names no scope (NFR14: each write is limited to the scopes the job uses); a `permissions` value, of the workflow or of a job, that is not `read-all`, `write-all` or a mapping whose every value, quotes and comment removed, is `read`, `write` or `none`: a tag, an anchor or an alias (`!!str write`, `&w write`, `*w`, `permissions: *all`, `permissions: !!map`), a folded or literal block scalar, an empty value and an entry without a scope hide the value from the rule, which fails closed (NFR14); a job `timeout-minutes` that is not an integer literal or exceeds 45, or 75 for the clean-clone job `clean-bootstrap-ci` of `scheduled.yml` (NFR39). A quoted key at the start of a line (`'jobs':`, `"lint":`, `'timeout-minutes':`, `'permissions':`, `- 'uses':`) is read as the key it names, and so is a quoted trigger key or entry (`'on':`, `'pull_request':`, `- "pull_request_target"`). The jobs are read from a block mapping under a plain key at two spaces (`  <id>:`) whose keys stand at four spaces; anything else hides the permissions and the timeout of a job and fails closed with `line <n>: a job is not a block mapping under a plain key at two spaces (a flow mapping, an anchor, an alias, a tag or another indentation), so its permissions and timeout cannot be read` (NFR14, NFR39): a value on the line of `jobs:` (a flow mapping, an anchor, an alias, a tag; a comment is no value), a line at two spaces below `jobs:` that is not a plain job key (`  lint: { runs-on: ubuntu-latest, permissions: { pull-requests: write }, steps: [...] }`, `  lint: &j`, `  lint2: *j`, `  lint: !!map`, or the opening brace of a flow mapping of the jobs, whose closing bracket belongs to the refused job), a line below `jobs:` before the first job (another indentation), a first line of a job that is not a plain key at four spaces (a flow mapping, an alias, a merge key `<<: *j` or keys at another indentation), and a later line at four spaces of a job that is neither a plain key, a sequence entry nor the closing bracket of a flow collection that a key of the job opened (a merge key, say); what was read of such a job is dropped. The lines below a refused job count under no job until the next plain job key, so a job before it is still checked in full |
| (b) | an image reference in the `Dockerfile`, the compose files, `make/tool-versions.mk` or a workflow lacks a version tag or a digest or uses `latest` (the one exception of FR125(b): the `database` image, whose tag is `DB_VERSION`); the BuildKit frontend named by the `# syntax=` parser directive of the `Dockerfile` lacks a version tag or a digest or uses `latest`; a feature of `.devcontainer/devcontainer.json` is not referenced by digest; a tool has no exact version; a `tool:` entry of a workflow, the version of `@devcontainers/cli` in a workflow, or the cargo-chef or Structurizr version of the `Dockerfile` or a compose file differs from `make/tool-versions.mk`; a file downloaded by URL in the `Dockerfile`, a make file, a workflow or a shell script (the files below `scripts/`, `tests/Load/` and `.githooks/` and every other `*.sh`, `*.bash` and `*.bats` file, without `tests/CLI/bats/fixtures/` and the ignored tool and state directories) has no checksum: such a file holds more download lines than checksum verifications. A verification is a run of `sha256sum`, `sha512sum` or `shasum` in the position of a command (at the start of a line, after `\|`, `;`, `&`, `(`, `{`, `!`, `@`, a backtick or `$(`, or after `then`, `do`, `else`, `if`, `elif`, `while`, `until`, `exec`, `time`, `sudo`, `RUN` or `run:`), or a call of a shell function whose body runs one (such as `sha256_of` of `scripts/install-tools.sh`): each call counts once and the runs inside the body do not, and a function the file never calls counts once (a library function). Comments, image digests (`@sha256:`), words in a message and lookups such as `command -v sha256sum` are no evidence. An `ADD` of an `http://` or `https://` source in the `Dockerfile` (any letter case, continued lines joined, also the JSON form) is a download that needs its own `--checksum=` with a `sha256:` (64), `sha384:` (96) or `sha512:` (128 hexadecimal digits) digest; a request whose only address is the loopback interface is a probe of the service under test, not a download: a loopback URL is the host `localhost`, `127.0.0.1` or `[::1]`, an optional port, an optional path without `@`, then a quote, white space or the end of the line; a probe line holds, beside its loopback URLs, exactly one `curl` or `wget`, no other URL, no `$` expansion, no backtick, no dotted word (a host, an IPv4 address or a file name: a dot between two letters or digits), no word with a `/` that starts like a host name or a bracketed IPv6 address, at the start of the line or after white space or `=`, behind an optional quote (a letter, digit or `_`, then letters, digits and `_.:@%+~-`, then `/`; or `[`, hexadecimal digits, dots and colons, `]`: an address without a scheme such as `intranet/tool`, `'[2001:db8::1]/tool'` or `--url=intranet/tool`, or a file of addresses such as `--input-file=lists/tools`; `/dev/null`, `>/dev/null` and `--output=/dev/null` are no such word), no option that names the address or sends the request elsewhere (`--url`, `--resolve`, `--connect-to`, `--proxy*`, `--preproxy`, `--socks*`, a short option cluster with `x`), no proxy variable (`http_proxy=`, `https_proxy=`, `all_proxy=`, `ftp_proxy=`, any case), and no continuation backslash; a probe may stand in a command substitution that starts with it (`status=$(curl -s -o /dev/null -w '%{http_code}' http://localhost:8080/ping)`), the backtick form is not read as a probe. The decision reads a copy of the line without its loopback URLs; every other line is matched unchanged, so a loopback proxy still counts as the URL of the command (so `http://localhost@host/x`, a loopback proxy with the address in a variable or on a continued line, a probe and a download joined by `&&` or `;`, a second address without a scheme, and `--connect-to localhost:80:host:80` are downloads). Known limits: an expansion in the port position of a loopback URL is trusted, because `make_lifecycle.bats` writes `http://127.0.0.1:${APP_PORT}$1`; a bare word without a dot or a `/` (a one-label host such as `intranet`) cannot be told from an option argument (`--max-time 10`, `--noproxy '*'`, which the probes of the tree use) and is trusted; a proxy exported on an earlier line is not seen; a checksum command behind another prefix (`xargs sha256sum`, `env X=1 sha256sum`) is not counted, and a comment sign after white space ends a line for the evidence even inside quotes (both make the rule stricter, never weaker). |
| (c) | `channel` of `rust-toolchain.toml`, the tag of the builder image in the `Dockerfile` and `[workspace.package].rust-version` do not state the same full version |
| (d) | a gate target of `make ci-list` is run by no workflow with a `pull_request` trigger (FR125(d)), or a workflow of any trigger runs a `make` target, directly or through an `exec` wrapper, that is neither a gate target of `make ci-list` nor in the CI-only list of DD-02. Every `make` command of a `run:` line is read: `make` at the start of a command (the start of the line or after `;`, `&`, `\|` or `(`, each followed by any chain of the words `then`, `do`, `else`, `if`, `elif`, `while`, `until`, `time`, `!` and `{`, each a whole word followed by white space, as in `if make <t>; then`, `; else make <t>`, `! make <t>`, `{ make <t>; }` and `time make <t>`), or behind an `exec` word of the same command with option words and at most one operand between them, an option word being a word with a leading `-` and at most one argument word (`--user dev`) or a `NAME=value` word, as in `npx --yes @devcontainers/cli@<version> exec --workspace-folder . make <target>`, `devcontainer exec --workspace-folder . make <target>` and `docker compose exec <service> make <target>`. A single-line `run:` value that is a quoted YAML scalar apart from a trailing comment is read as the text it stands for (double quotes with backslash escapes, where `\n` starts a new line and `\t` is a tab; single quotes with `''` for a quote), so `run: "make <t>"` and `run: 'make <t>'` are read; block scalars are read as before. Quoted strings, escaped characters and comments are removed first, so a message or a variable value that names `make` (`echo 'make bootstrap failed'`, `target='devcontainers exec make format-check'`) is no make command, and neither is a bare `make` word of an `echo` or an assignment; a file `tests/CLI/bats/*.bats` does not carry exactly one file tag that is a group of the `bats-group` matrix; a job name of a pull-request workflow is missing from `docs/repository-settings.md`. Known limits of the reading of `make` commands: a `make` inside a quoted command string (`bash -c 'make x'`) is not read; neither `env NAME=value make <t>` without an `exec` word before it, `xargs make <t>`, `docker compose run <service> make <t>` and a command continued onto the next line with a trailing backslash (each line is read alone), while `exec env NAME=value make <t>` is read (`env` is the one operand and `NAME=value` an option word); the template's workflows have no such call. A plain word sequence such as `echo please exec this and make that` (two operands) and a word that ends in `exec` (`my-exec app make <t>`) are not read either. A quoted scalar that goes on over several lines is read as written, with its opening quote, so no make command in it is read, and `time -p make <t>` (an option of `time`) is not read; the template's workflows have neither. A keyword after another word is an argument, not the start of a command, so `echo todo make x`, `echo then make x` and `echo if make x` are not read. One form over-reads and therefore fails closed, never open: an unquoted message that contains `exec make <t>` (`echo exec make x`) is read as a make command, so a workflow quotes such prose. |
| (e) | the repository name occurs outside the places DD-23 allows |
| (f) | a licence identifier other than `CC0-1.0` is stated for the project |
| (g) | a Rust source file (a `*.rs` file, or a file that a manifest names as a crate root, whatever its name ends in) contains `#[allow` or `#![allow`; a crate root contains `#![expect(`: `lib.rs`, `main.rs` and `build.rs` by name (so also `tests/*/main.rs`, `src/bin/*/main.rs`, `examples/*/main.rs` and `benches/*/main.rs`), every `*.rs` file directly in the `tests/`, `examples/` or `benches/` directory of a package (a directory with a `Cargo.toml`) or in its `src/bin/`, which cargo discovers as crate roots, and every file that a manifest names: the `path` string of a `[lib]`, `[[bin]]`, `[[test]]`, `[[example]]` or `[[bench]]` table and the `build` string of `[package]` (or of the legacy `[project]`, which cargo still reads below edition 2024; `build = false` names nothing), resolved against the directory of the manifest, from every `Cargo.toml` of the walk of the Rust sources. The manifest is parsed as TOML: white space around `=` and in table headers, basic and literal strings (also multi-line), comments, quoted and dotted keys (`lib.path = ...`), inline tables (`lib = { path = ... }`) and arrays of them (`bin = [{ path = ... }]`) name the same key, while the `path` of a dependency (`[dependencies.<name>] path = ...`, `<name> = { path = ... }`, also in the dev, build, target, workspace and patch tables), a key of another table (`[package.metadata.<tool>] build`) and text in a string or a comment name no crate root; a module file below such a directory (`tests/integration/support.rs`, `src/bin/<name>/<module>.rs`), a module file next to a named root and a module directory named `tests` below `src/` hold no crate root; the manifest reader reads an escape inside a multi-line string as written, and names no file for an absolute path, for a path whose `..` leaves the tree, or for an array value of `build` (unstable in cargo 1.99.1); a file-wide directive of another linter that names one rule and carries a reason is allowed, since those linters have no item scope (implementation-readiness.md, reading NFR16, FR125(g); Rust keeps rule 15 of architecture.md); an `allow` or `expect` names a lint group: the 13 groups that `rustc -W help` of the pinned toolchain prints (`warnings`, `deprecated_safe`, `future_incompatible`, `keyword_idents`, `let_underscore`, `nonstandard_style`, `refining_impl_trait`, `rust_2018_compatibility`, `rust_2018_idioms`, `rust_2021_compatibility`, `rust_2024_compatibility`, `unknown_or_malformed_diagnostic_attributes`, `unused`), the ten clippy groups (`clippy::all`, `clippy::cargo`, `clippy::complexity`, `clippy::correctness`, `clippy::nursery`, `clippy::pedantic`, `clippy::perf`, `clippy::restriction`, `clippy::style`, `clippy::suspicious`) and `rustdoc::all`; the same three checks apply to an `allow(...)` or `expect(...)` inside `#[cfg_attr(` or `#![cfg_attr(` (an inner `cfg_attr` with an `expect` in a crate root counts as `#![expect(`). A missing reason is delegated to `clippy::allow_attributes_without_reason` of G2 (FR125 allows the delegation). No linter of G23 or of the load-test kit refuses an inline suppression directive that names no rule, names several rules or carries no reason, so rule (g) reads the directives of shellcheck, hadolint, yamllint, markdownlint, ESLint and zizmor itself and reports `(g) <file>:<line>: a <linter> directive names no single rule` (no name, `all` in any letter case for shellcheck, hadolint, yamllint and markdownlint, `all` or `*` for zizmor, a bare `eslint-disable` or `markdownlint-disable`, a `yamllint disable` without `rule:` or a name without `rule:`, `yamllint disable-file`, `markdownlint-disable-file` without a rule, a markdownlint tag such as `headings`, or `default`, named in a disable directive or as a key of `markdownlint-configure-file`), `... names more than one rule` (a comma list, several names, several `rule:` entries, several `disable=` keys, a shellcheck range such as `SC2000-SC2099`, several keys of a configuration) or `... carries no reason` (`an ESLint directive ...` for ESLint). A reason is text after the directive where the linter accepts it (`# <reason>` after a shellcheck or hadolint directive; text, with or without a `#`, after the `]` of a zizmor ignore; ` -- <reason>` in an ESLint comment, split off before the directive is read, as ESLint does) or a comment line directly above the directive, for every linter; yamllint and markdownlint accept only the line above (yamllint ignores a directive with text after it, markdownlint reads that text as rule names). A comment line holds a letter or digit and is no directive of the same linter (a shebang is none). The directives are read where the linter reads them: `# shellcheck disable=` in a comment (also one inside `$(...)`, where the shell reads a comment too) of a `*.sh`, `*.bash`, `*.bats` or `.githooks/*` file and of the `run:` block scalars of the workflows, also after an anchor or a tag (`run: &script \|`), outside quotes, `$'...'`, `${...}`, arithmetic, backquotes and here-document bodies (actionlint hands a run block to shellcheck without its content indentation, and the reader removes it too: the indentation indicator added to the column of the node, or else the indentation of the first line that is not empty, so that the end of a here-document in a run block is found as in a shell file); `# hadolint ignore=` and `# hadolint global ignore=` in a comment line of a `Dockerfile*`; `# yamllint disable`, `disable-line` and, on the first line, `disable-file` in a YAML comment outside quoted and block scalars of a `*.yml` or `*.yaml` file; `markdownlint-disable` in an HTML comment, `-disable-line`, `-disable-next-line`, `-disable-file` and `markdownlint-configure-file` anywhere in a line of a `*.md` file, inside code spans and fenced blocks too, as markdownlint matches them; `eslint-disable` and a configuration of rules (`/* eslint <rule>: <setting> */`) in a block comment and `eslint-disable-line` and `-next-line` in a line or block comment of a `*.js`, `*.mjs` or `*.cjs` file below `tests/Load/`, outside strings, template literals and regular expression literals; `# zizmor: ignore[...]` in a YAML comment of a workflow. `shellcheck source=`, `shell=` and `enable=`, `hadolint shell=`, `yamllint enable`, `markdownlint-enable`, `-capture` and `-restore`, `eslint-enable`, `eslint-env`, a line comment `// eslint-disable` and a line comment `// eslint <rule>: ...` suppress nothing and are not read; a block comment `/* global ... */`, `/* globals ... */` or `/* exported ... */` is an ESLint directive (`directivesPattern` of ESLint), so it is no reason line, while a line comment that starts with one of these words is ordinary text (ESLint reads no line comment but `eslint-disable-line` and `-next-line`). The files are those `git ls-files --cached --others --exclude-standard` lists at the root (with `GIT_DIR` and `GIT_WORK_TREE` of the caller unset), every file when the root has no `.git`, without `specs/`, `_bmad*/`, `tests/CLI/bats/fixtures/` and every `node_modules/` and `target/` directory. Known limits of the directive reader, each of which can fail open only for code that the linters themselves treat specially and that the template's files do not contain: the shell reader does not follow a `case` pattern inside `$(...)` (its `)` closes the substitution early, which changes no quote state), starts the body of a here-document after the physical line even when that line is continued with a backslash, and reads `$"..."` as double quotes; the JavaScript reader takes a `/` after `)`, a name or a keyword such as `return` for a division; a YAML quoted scalar that spans lines is read line by line; the keys of `markdownlint-configure-file` and of an ESLint configuration are counted at the top level of a JSON, JSONC or flow-mapping text. One limit over-reads and fails closed: a workflow run block is read as shell whatever its `shell:` is (actionlint hands only bash and sh blocks to shellcheck), so a directive-like comment in a `shell: python` or `pwsh` block is judged too |
| (h) | a recipe line of a class TOOL target starts a toolchain command without `$(EXEC)`; a script that a recipe calls outside `$(EXEC)` contains a `cargo`, `sqlx`, `taplo`, `typos` or `zizmor` command (`install-tools.sh` and `doctor.sh` are the two listed exceptions) |

## Containers and Compose

### `Dockerfile`

One file, six stages. `hadolint` 2.15.1 passes on this text with the configuration under "Repository Lint and Hygiene" (verified).

```dockerfile
# syntax=docker/dockerfile:1.27@sha256:4edf897a3ffa55b89f906fc8cc78afdb3f1834cc9c7083565e611a8a7d5fe99e
# Stages: base -> chef -> planner -> builder -> prod   (production image)
#         base -> dev                                  (development image)
# The tag of this image is the toolchain version: it equals rust-toolchain.toml and rust-version (G31 rule (c)).
FROM rust:1.99.1-slim-trixie@sha256:f6c4a35d21dbb3c89d7a8d056c193e94fc09037f2b58dd858b09c39a47dd02d7 AS base
WORKDIR /app
ENV CARGO_TERM_COLOR=always \
    CARGO_NET_RETRY=10 \
    CARGO_INCREMENTAL=0 \
    SQLX_OFFLINE=true

FROM base AS chef
ARG CARGO_CHEF_VERSION=0.1.78
RUN cargo install cargo-chef --locked --version "${CARGO_CHEF_VERSION}" \
 && rm -rf "${CARGO_HOME}/registry"

# The recipe holds manifests and the lock file only: it changes only when dependencies change.
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Dependency layer: rebuilt only when a manifest or Cargo.lock changes.
RUN cargo chef cook --release --locked --package service --recipe-path recipe.json
# Application layer. `.sqlx/` and `migrations/` arrive with this COPY.
COPY . .
RUN cargo build --release --locked --package service --bin service \
 && install -D target/release/service /out/service

FROM base AS dev
SHELL ["/bin/bash", "-o", "pipefail", "-c"]
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl git jq make xz-utils \
 && rm -rf /var/lib/apt/lists/*
RUN rustup component add clippy rustfmt llvm-tools-preview
COPY make/tool-versions.mk /tmp/tooling/make/tool-versions.mk
COPY scripts/install-tools.sh scripts/install-bats.sh /tmp/tooling/scripts/
RUN --mount=type=secret,id=github_token,env=GITHUB_TOKEN \
    /tmp/tooling/scripts/install-tools.sh --with-watchexec \
 && /tmp/tooling/scripts/install-bats.sh --prefix /opt/bats \
 && rm -rf /tmp/tooling "${CARGO_HOME}/registry"
# Any uid may use the image: cargo state, the home directory and the build directory are world-writable
# mount points outside /app, so nothing is created in a bind-mounted checkout. The home directory has no sticky
# bit: the common-utils feature of .devcontainer/ appends as root to the .bashrc it gave the user there, which
# fs.protected_regular (set on hosted runners) refuses in a sticky world-writable directory owned by root.
RUN mkdir -p /cargo /home/dev /cargo-target \
 && chmod 1777 /cargo /cargo-target \
 && chmod 0777 /home/dev
ENV CARGO_INCREMENTAL=1 \
    CARGO_HOME=/cargo \
    CARGO_TARGET_DIR=/cargo-target \
    HOME=/home/dev \
    BATS_LIB_PATH=/opt/bats/lib \
    PATH=/opt/bats/core/bin:/usr/local/cargo/bin:${PATH}
EXPOSE 8081
CMD ["watchexec", "--restart", "--stop-signal", "SIGTERM", "--watch", "crates", "--watch", "migrations", "--watch", "Cargo.toml", "--watch", "Cargo.lock", "--", "cargo", "run", "--locked", "-p", "service", "--", "serve"]

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:cc66d222f0a7ec996b78a99fe76a969bc80b539a5016ad866c14d1949bc6d0ab AS prod
LABEL org.opencontainers.image.licenses="CC0-1.0"
COPY --from=builder /out/service /app/service
ENV APP_ENV=prod \
    APP_HOST=0.0.0.0 \
    APP_PORT=8081
USER 65532:65532
EXPOSE 8081
STOPSIGNAL SIGTERM
# distroless has no shell, curl or wget: the binary probes itself (exec form is mandatory).
HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=3 CMD ["/app/service", "healthcheck"]
ENTRYPOINT ["/app/service"]
CMD ["serve"]
```

| Stage | Base | Contains | User |
| --- | --- | --- | --- |
| `base` | `rust:1.99.1-slim-trixie`, by digest (Debian 13, Rust 1.99.1) | working directory, build environment, `SQLX_OFFLINE=true` | root (build only) |
| `chef` | `base` | cargo-chef 0.1.78 | root |
| `planner` | `chef` | the build context and `recipe.json` | root |
| `builder` | `chef` | compiled dependencies (one cached layer), then the release binary at `/out/service` | root |
| `dev` | `base` | clippy, rustfmt, llvm-tools; the cargo tools of `make/tool-versions.mk`; watchexec; Bats and helpers in `/opt/bats`; git, make, curl, jq; the empty mount points `/cargo` and `/cargo-target` | the invoking user (compose `user:`); any uid works |
| `prod` | distroless `cc-debian13:nonroot`, by digest | `/app/service` only; no shell, package manager, toolchain, Makefile, script or env file | `65532:65532` |

Build argument: `CARGO_CHEF_VERSION` only (its default equals `make/tool-versions.mk`, G31 rule (b)); no argument replaces a base image (DD-20). Build secret: `github_token` (optional; raises the GitHub rate limit for `cargo-binstall` in the `dev` stage; never stored in a layer). Cache strategy: locally the ordinary layer cache; in CI `docker/build-push-action` with `cache-from: type=gha` and, on `main`, `cache-to: type=gha,mode=max` (scopes `prod-image` and `dev-image`), so the cargo-chef layer survives between runs. Arguments given to the production container select the command (`migrate`, `check-config`); without arguments it serves. Reload in the development container is watchexec with explicit watch paths. `panic = "unwind"` stays (core AD-15).

Why the development build directory is `/cargo-target`: a volume mounted at `/app/target`, inside the bind mount `./:/app`, made Docker create `./target` on the host as `root:root`, after which every native cargo command of the checkout's owner failed with `Permission denied` (verified on a checkout owned by uid 1001). The `dev` stage therefore sets `CARGO_TARGET_DIR=/cargo-target` and the volume is mounted there, outside the bind mount. Reports are not affected: recipes write them to `target/coverage`, `target/sarif` and `target/reports`, which are ordinary directories of the checkout.

Measured on a scratch workspace with this layout: the `prod` target builds in 124 s cold (cargo-chef install 75 s, dependency layer 35 s) and yields an image of 43.7 MB with user `65532:65532`; the full dependency set of the service adds about two minutes to the dependency layer (release build of the data spike with sqlx and the AWS SDK: 2 min 18 s).

### `.dockerignore`

```gitignore
# Version control, CI, guidance and planning: never needed by a build.
.git/
.github/
.claude/
.devcontainer/
.githooks/
.ralph/
.tools/
_bmad/
_bmad-output/
bmalph/
specs/
docs/
# Build output and reports.
target/
mutants.out*/
**/node_modules/
# Tests that are not workspace members, and their data.
features/
tests/CLI/
tests/Load/
# Local configuration and container files.
.env*
*.md
workspace.dsl
Dockerfile
docker-compose*.yml
# The image tag is the toolchain; with this file in the context rustup would download three components
# in the planner stage and again in the builder stage after every source change.
rust-toolchain.toml
```

Not ignored, because the build needs them: `Cargo.toml`, `Cargo.lock`, `.cargo/`, `.sqlx/`, `migrations/`, `crates/`, `xtask/`, `tests/support/`, `tests/e2e/` (workspace members must be present for cargo to load the workspace), `make/tool-versions.mk` and `scripts/` (copied into the `dev` stage). `rust-toolchain.toml` is ignored: the builder uses the toolchain of the image tag, and rule (c) of `validate-invariants` keeps that tag equal to the file (measured: a rebuild after a source change takes 3 s without the file in the context and 14 to 18 s with it). In the development container the file arrives with the bind mount and selects the toolchain the image already holds.

### Compose files

The four PHP file names are kept. Compose service names: `app` (the service), `migrate` (one-shot migration step), `database`, `localstack`, `structurizr`. All four files below were rendered with `docker compose config` on Compose v5.6.1 in the three combinations of the table, and pass `yamllint --strict` with `.yamllint.yml` and `prettier --check` with `.prettierrc.yml` (verified for this revision). `!reset` and `!override` need Compose 2.24 or newer and `start_interval` needs Docker Engine 25.0; `make doctor` checks both.

| Invocation | Files | Project | Result |
| --- | --- | --- | --- |
| `make start` | `docker-compose.yml` + `docker-compose.override.yml` (merged automatically) | `$(PROJECT_NAME)` | development: `app` (target `dev`), `migrate`, `database`, `localstack`, and `structurizr` with profile `docs` |
| `scripts/with-load-test-stack.sh` | `-p $(PROJECT_NAME)-load -f docker-compose.yml -f docker-compose.load_test.override.yml` | `$(PROJECT_NAME)-load` | production image, isolated |
| production reference | `--env-file <production env> -f docker-compose.yml -f docker-compose.prod.yml` | operator's choice | `app` and `migrate` only |

`docker-compose.yml` (base: production-shaped service and its backing services):

```yaml
x-build: &build
  context: .
  target: prod
x-hardening: &hardening
  read_only: true
  cap_drop: [ALL]
  security_opt: ['no-new-privileges:true']

services:
  app:
    <<: *hardening
    image: ${APP_IMAGE:-service:prod}
    build: *build
    depends_on:
      migrate:
        condition: service_completed_successfully
      localstack:
        condition: service_healthy
    environment:
      APP_ENV: ${APP_ENV:-prod}
      APP_PORT: '8081'
      LOG_LEVEL: ${LOG_LEVEL:-info}
      DATABASE_URL: postgres://${DB_USER:-app}:${DB_PASSWORD:-local-dev-only}@database:5432/${DB_NAME:-app}
      AWS_REGION: ${AWS_REGION:-us-east-1}
      # Literal emulator placeholders: credentials of the host shell never reach a local container.
      AWS_ACCESS_KEY_ID: test
      AWS_SECRET_ACCESS_KEY: test
      AWS_ENDPOINT_URL_SQS: http://localstack:4566
    ports:
      - '${APP_PORT:-8081}:8081'
    healthcheck:
      test: ['CMD', '/app/service', 'healthcheck'] # exec form: the image has no shell
      interval: 10s
      timeout: 3s
      retries: 3
      start_period: 10s
      start_interval: 1s
    restart: unless-stopped
    stop_grace_period: 15s

  migrate:
    <<: *hardening
    image: ${APP_IMAGE:-service:prod}
    build: *build
    command: ['migrate']
    depends_on:
      database:
        condition: service_healthy
    environment:
      DATABASE_URL: postgres://${DB_USER:-app}:${DB_PASSWORD:-local-dev-only}@database:5432/${DB_NAME:-app}
    restart: 'no'

  database:
    # The one image without a digest: its tag is DB_VERSION, and a digest would override the tag.
    image: postgres:${DB_VERSION:-16}-alpine
    environment:
      POSTGRES_DB: ${DB_NAME:-app}
      POSTGRES_USER: ${DB_USER:-app}
      POSTGRES_PASSWORD: ${DB_PASSWORD:-local-dev-only}
      # From PostgreSQL 18 the image keeps the cluster in /var/lib/postgresql/<major>/docker and refuses a
      # mount at /var/lib/postgresql/data. Naming the old path keeps the data in db_data for every DB_VERSION.
      # A change of the major version still needs a fresh volume or pg_upgrade.
      PGDATA: /var/lib/postgresql/data
    ports:
      - '127.0.0.1:${DB_PORT:-5432}:5432'
    volumes:
      - db_data:/var/lib/postgresql/data
    healthcheck:
      test: ['CMD-SHELL', 'pg_isready -U "$$POSTGRES_USER" -d "$$POSTGRES_DB"']
      interval: 5s
      timeout: 3s
      retries: 10
      start_period: 10s
      start_interval: 1s
    restart: unless-stopped

  localstack:
    image: localstack/localstack:3.4.0@sha256:54fcf172f6ff70909e1e26652c3bb4587282890aff0d02c20aa7695469476ac0
    environment:
      SERVICES: sqs
      DEBUG: '0'
    ports:
      - '127.0.0.1:${LOCALSTACK_PORT:-4566}:4566'
    volumes:
      - localstack_data:/var/lib/localstack
    healthcheck:
      # Also warms SQS.
      test: ['CMD-SHELL', 'awslocal sqs list-queues > /dev/null']
      interval: 5s
      timeout: 5s
      retries: 20
      start_period: 10s
    restart: unless-stopped

volumes:
  db_data:
  localstack_data:
```

`docker-compose.override.yml` (development):

```yaml
x-dev: &dev
  image: ${DEV_IMAGE:-service:dev}
  build:
    target: dev
    secrets:
      - github_token
  user: '${HOST_UID:-1000}:${HOST_GID:-1000}'
  read_only: false
  volumes:
    - ./:/app
    - cargo_target:/cargo-target # outside the bind mount: Docker creates nothing in the checkout
    - cargo_home:/cargo

services:
  app: # no command: the dev image's CMD (watchexec) applies
    <<: *dev
    environment:
      IN_DEV_CONTAINER: '1'
      # The services inside the compose network, at their in-network ports: DB_PORT and LOCALSTACK_PORT of .env
      # are the published host ports, and the URLs of .env and .env.test are built from these four values.
      DB_HOST: database
      DB_PORT: '5432'
      LOCALSTACK_HOST: localstack
      LOCALSTACK_PORT: '4566'
      # Removed so that the binary reads .env and the test targets read .env.test.
      APP_ENV: !reset null
      LOG_LEVEL: !reset null
      DATABASE_URL: !reset null
      AWS_ENDPOINT_URL_SQS: !reset null
    ports: !override
      - '127.0.0.1:${APP_PORT:-8081}:8081'
    healthcheck:
      test: ['CMD', '/cargo-target/debug/service', 'healthcheck']
      start_period: 60s # the first compile happens in the migrate step
      start_interval: 5s

  migrate:
    <<: *dev
    command: ['cargo', 'run', '--locked', '--quiet', '-p', 'service', '--', 'migrate']
    environment:
      # The database inside the compose network, at its in-network port (DB_PORT of .env is the host port).
      DB_HOST: database
      DB_PORT: '5432'
      DATABASE_URL: !reset null

  localstack:
    environment:
      SERVICES: sqs,ec2,s3,iam,sts # ec2, s3, iam, sts: only for `make aws-load-tests LOCAL_MODE_ENV=true`
      DEBUG: '1'

  structurizr:
    image: structurizr/structurizr:2026.10.26@sha256:8aebde645a7cc8d3b9e1e774cd6d75a84ce45d74ae70eb783876182fe7db31a2
    command: ['local']
    profiles: [docs]
    ports:
      - '127.0.0.1:${STRUCTURIZR_PORT:-8080}:8080'
    volumes:
      - structurizr_data:/usr/local/structurizr
      - ./workspace.dsl:/usr/local/structurizr/workspace.dsl:ro
    restart: unless-stopped

volumes:
  cargo_target:
  cargo_home:
  structurizr_data:

secrets:
  github_token:
    environment: GITHUB_TOKEN
```

`docker-compose.prod.yml` (production reference):

```yaml
# Run with an explicit env file so that the committed development .env is not read:
#   docker compose --env-file /etc/service/prod.env -f docker-compose.yml -f docker-compose.prod.yml up --detach --wait
services:
  app:
    depends_on: !override
      migrate:
        condition: service_completed_successfully
    environment:
      APP_ENV: prod
      DATABASE_URL: ${DATABASE_URL:?DATABASE_URL is required}
      AWS_REGION: ${AWS_REGION:?AWS_REGION is required}
      AWS_ENDPOINT_URL_SQS: !reset null # real SQS through the default credential chain
      AWS_ACCESS_KEY_ID: !reset null
      AWS_SECRET_ACCESS_KEY: !reset null
    deploy:
      resources:
        limits:
          cpus: '1.0'
          memory: 256M
          pids: 256
        reservations:
          memory: 64M
    logging:
      driver: json-file
      options:
        max-size: 10m
        max-file: '3'

  migrate:
    depends_on: !reset null
    environment:
      DATABASE_URL: ${DATABASE_URL:?DATABASE_URL is required}
    deploy:
      resources:
        limits:
          cpus: '0.5'
          memory: 128M

  database:
    profiles: [local]

  localstack:
    profiles: [local]
```

`docker-compose.load_test.override.yml` (isolated production-image environment):

```yaml
# Always used with its own project name and an explicit file list (see DD-19).
services:
  app:
    environment:
      APP_ENV: test
    ports: !override
      - '127.0.0.1:${LOAD_TEST_APP_PORT:-18081}:8081'
    restart: 'no'
  database:
    ports: !reset []
    restart: 'no'
  localstack:
    ports: !reset []
    restart: 'no'
```

Facts that follow from the files:

| Topic | Value |
| --- | --- |
| Published ports (development) | service `APP_PORT` 8081; PostgreSQL `DB_PORT` 5432; LocalStack `LOCALSTACK_PORT` 4566; Structurizr `STRUCTURIZR_PORT` 8080; all bound to `127.0.0.1`, because the database password is a known local placeholder |
| Published ports (isolated) | service `LOAD_TEST_APP_PORT` 18081 on `127.0.0.1` only |
| Published ports (production reference) | service `APP_PORT` on all interfaces; nothing else |
| Volumes | development: `db_data`, `localstack_data`, `cargo_target` (at `/cargo-target`), `cargo_home`, `structurizr_data`; isolated project: its own `db_data` and `localstack_data`, removed after every run |
| Start order | `database` healthy, then `migrate` exits 0, then `app` (and `localstack` healthy). `docker compose up --detach --wait` returned 0 with this shape and 1 with the message `service "migrate" didn't complete successfully` when the step failed (verified) |
| Commands | `app` has no `command`: the production image serves by default (`CMD ["serve"]`) and the development image starts watchexec (its `CMD`), so the watch command line exists once |
| Health | `app`: `service healthcheck` (liveness, `/ping`), 10 s / 3 s / 3 retries / start period 10 s (60 s in development, probing every 5 s: the first compile happens in the `migrate` step, before `app` starts); `database`: `pg_isready` with the configured user and database, 5 s interval, 10 retries. `make start` waits at most 900 s (`--wait-timeout`) and exits non-zero when the environment is not healthy by then (NFR33) |
| Environment of `app` (development) | rendered: `APP_PORT`, `AWS_REGION`, the two emulator credentials `test`, `DB_HOST=database`, `DB_PORT=5432`, `LOCALSTACK_HOST=localstack`, `LOCALSTACK_PORT=4566`, `IN_DEV_CONTAINER=1`; no `APP_ENV`, `LOG_LEVEL`, `DATABASE_URL` or `AWS_ENDPOINT_URL_SQS` (verified, also with real credentials exported in the host shell) |
| Environment mode | development: `dev` from `.env`; isolated environment: `APP_ENV=test`, which behaves as `prod` (FR1); production reference: `APP_ENV=prod` |
| Hardening | `app` and `migrate`: read-only root file system, all capabilities dropped, `no-new-privileges`; relaxed to a writable root only by the development override |
| Limits and restart (production reference) | `app` 1 CPU, 256 MiB, 256 processes, `restart: unless-stopped`, 15 s stop grace period; `migrate` 0.5 CPU, 128 MiB, never restarted |
| Required production values | `DATABASE_URL`, `AWS_REGION`; `docker compose --env-file /dev/null … config` fails and names the missing variables: depending on the Compose version it names both or stops at the first missing one, in no fixed order, so each is named when it is the only one unset (verified) |
| Development files on the host | the container runs as `HOST_UID:HOST_GID`; cargo's build directory (`/cargo-target`) and cargo home (`/cargo`) are volumes outside the bind mount, so Docker creates nothing in the checkout and no file of another user appears in it |

How the isolated environment avoids development data: a different project name gives it its own containers, network and volumes; the explicit `-f` list never includes the development override; no database or LocalStack port is published, so nothing on the host can reach its data and it cannot collide with the development ports; `setup-load-test-db` addresses the project by name and refuses to run when that project is not up; the wrapper removes the project with `down --volumes --remove-orphans` on every exit path.

### Development container definition

`.devcontainer/devcontainer.json` builds the `dev` stage of the `Dockerfile` and adds Docker inside the container, so the container is "a host with the pinned toolchain": `make bootstrap`, `make start` and `make ci` work in it natively and the compose services run in its own Docker daemon.

```jsonc
{
  "name": "service-dev",
  "build": { "dockerfile": "../Dockerfile", "context": "..", "target": "dev" },
  "features": {
    // docker-in-docker 4.1.3; `moby` is false because the feature refuses Moby on Debian trixie.
    "ghcr.io/devcontainers/features/docker-in-docker@sha256:fae40f3c8be5a7c3b0db344c1d30341bc2c91ba38d1d5b14cd2ad72e6d65d084": {
      "moby": false,
      "dockerDashComposeVersion": "v2"
    },
    // common-utils 2.7.1
    "ghcr.io/devcontainers/features/common-utils@sha256:e76a6a8a35d1ad824daad1540725e0ec9f8ed2dc00d2850e990a9830013c7d08": {
      "username": "dev",
      "userUid": "automatic",
      "userGid": "automatic"
    }
  },
  "remoteUser": "dev",
  "workspaceMount": "source=${localWorkspaceFolder},target=/app,type=bind",
  "workspaceFolder": "/app",
  "mounts": [
    "source=devcontainer-cargo-home,target=/cargo,type=volume",
    "source=cargo-target-${devcontainerId},target=/cargo-target,type=volume"
  ],
  "forwardPorts": [8081, 5432, 4566, 8080],
  "postCreateCommand": "make install-hooks && make doctor",
  "customizations": {
    "vscode": {
      "extensions": [
        "rust-lang.rust-analyzer",
        "tamasfe.even-better-toml",
        "EditorConfig.EditorConfig"
      ]
    }
  }
}
```

- The two features are referenced by digest, with the version in a comment (docker-in-docker 4.1.3, common-utils 2.7.1; both digests were read from the registry on 2026-11-10), and G31 rule (b) reads this file. `moby` is `false`: the feature's install script exits 1 when Moby is requested on Debian trixie, which is the base of the `dev` stage.
- `IN_DEV_CONTAINER` is not set here: it comes from the compose override of the `app` container only. In the development container targets therefore start PostgreSQL and LocalStack through `ensure-deps` and `ensure-database`, like on a host (FR66(c), FR118), and `RUN_IN_CONTAINER=1` would address the `app` container of the inner daemon.
- `DB_HOST` and `LOCALSTACK_HOST` are not set, so `.env` resolves to `localhost`, which is correct because the compose services publish their ports inside the container's own Docker daemon.
- The build directory and cargo home come from the image (`CARGO_TARGET_DIR=/cargo-target`, `CARGO_HOME=/cargo`) and are volumes here as well. The build volume is named per development container (`cargo-target-${devcontainerId}`), as the compose stacks are scoped by `COMPOSE_PROJECT_NAME`, so two services made from the template, which share the workspace path `/app` and the crate names, never reuse each other's build artifacts; the cargo home volume `devcontainer-cargo-home` stays shared.

The definition is built, started and checked from inside weekly by the scheduled workflow (FR95, FR118: `postCreateCommand`, `docker info`, `make format-check`, `make start-deps`); it was not built for this document. The first hosted build of the committed definition failed (run 43107219243) and passed after the `Dockerfile` fix (run 43119953189); the checks from inside first ran in run 43151995703 (job 130963101979, a dispatch on the branch of pull request #2 at `da5ea9c`, 2026-11-12) and passed: build 164 s, `up` 7 s (the `postCreateCommand` ran and the doctor printed `[OK]` for every tool), `docker info` inside the container under 1 s, `make format-check` 1 s and `make start-deps` through the inner engine 29 s (both backing services healthy), the hosted proof of FR118.

## CI/CD Workflows

### Summary

| PHP workflow | Rust counterpart (file / job) | Disposition |
| --- | --- | --- |
| `tests.yml` | `tests.yml` / `tests` | kept; one execution of the unit and integration suites reports tests and coverage |
| `codecov.yml` | `tests.yml` / `tests` | merged: one job computes and uploads coverage |
| `E2Etests.yml` | `tests.yml` / `e2e-tests` | merged |
| `bats-tests.yml` | `tests.yml` / `bats` | merged |
| `infection.yml` | `tests.yml` / `mutation-tests` | merged; now fails on survivors |
| `psalm.yml` | `quality.yml` / `static-analysis` and `clippy-sarif-upload`; `security.yml` / `codeql` | replaced |
| `phpinsights.yml` | `quality.yml` / `format-check`, `static-analysis`, `complexity`, `architecture` | replaced by four gates |
| `deptrac.yml` | `quality.yml` / `architecture` | merged |
| `rector.yml` | `quality.yml` / `static-analysis` | merged: a pending fix is a finding |
| `symfony.yml` | `quality.yml` / `manifest`, `check-requirements`, `dependency-policy` | replaced |
| `code-lint-and-prettify.yml` | none | dropped: no bot commits (scope 3.1); the failing `format-check` replaces it |
| `openapi-validate.yml` | `contracts.yml` / `openapi-lint` | merged |
| `openapi-diff.yml` | `change-control.yml` / `openapi-diff` | replaced: oasdiff, fails unless labelled |
| `graphql-diff.yml` | `change-control.yml` / `graphql-diff`; `contracts.yml` / `contract-snapshots` | replaced: no auto-commit |
| `schemathesis.yml` | `contracts.yml` / `schemathesis` | merged; production image |
| `load-tests.yml` | `load-tests.yml` / `smoke-load-tests` | kept; results uploaded |
| `eslint.yaml` | `load-tests.yml` / `lint-load-tests` | merged |
| `autorelease.yml` | `release.yml` | replaced: cocogitto, maintained actions |
| `tempate-sync-pat.yml` | `template-sync-pat.yml` | kept; file name corrected |
| `template-sync-app.yml` | `template-sync-app.yml` | kept |

New workflows and jobs without a PHP counterpart: `quality.yml` / `complexity`, `crate-isolation`, `query-metadata`, `unused-dependencies`; `tests.yml` / `mutation-plan`; `security.yml` (`secrets-scan`, `image-scan`, `codeql`); `repository.yml` (`lint-repo` with workflow, YAML, shell, Dockerfile, Markdown, TOML, spelling and Prettier checks; `invariants`; `docs-check`; `architecture-model`); `change-control.yml` / `title`, `validate-configuration`; `scheduled.yml`.

### Action pins

Tags and SHAs are those of `research/rust-testing-delivery.md`; the validation round read all of them again with `git ls-remote` and they match, and every input used below exists at the pinned commit. `actions/download-artifact` was added in this revision and read the same way; it is the one action outside the research table. `actions/cache` is no longer used. For annotated tags the SHA is the commit the tag points to.

| Action | Tag | Commit SHA | Used in |
| --- | --- | --- | --- |
| `actions/checkout` | v7.0.1 | `3d3c42e5aac5ba805825da76410c181273ba90b1` | every job that reads the repository |
| `actions-rust-lang/setup-rust-toolchain` | v2.0.1 | `a188e68d0fd2bd36ad51dccb2b07ba6e75105f30` | every job that runs cargo |
| `taiki-e/install-action` | v2.90.3 | `cfdb7ed4ccc7c7dce37759eae89dc5674b469b39` | every job that needs a cargo tool |
| `actions/upload-artifact` | v7.0.2 | `cf430e030ddbb5b0abf93d22962f4752f3646cd9` | coverage report, JUnit file, clippy SARIF, mutation output, load results |
| `actions/download-artifact` | v8.0.2 | `9000827ccba6bdab643e8b6fd33ac0654aef8333` | `clippy-sarif-upload` |
| `codecov/codecov-action` | v7.1.1 | `303a32d7a59b442fa8d48b6a1cc6825c09c847a5` | `tests.yml` / `tests` |
| `github/codeql-action` (`init`, `analyze`, `upload-sarif`) | v4.39.3 | `33d7b85da1a04171e211befe2c739303b9a4c066` | `codeql`, `clippy-sarif-upload`, `code-scanning` |
| `docker/setup-buildx-action` | v4.4.1 | `f87e5991a6d7451dcb8d9637bfbc97413f497069` | jobs that build an image |
| `docker/build-push-action` | v7.4.0 | `c3c9e263c25d99ce0380d002d59b67737d91b0dc` | jobs that build an image |
| `actions/create-github-app-token` | v3.3.0 | `64ab729a6294000f5aa4a4bc0b73d1f32bc45510` | `release.yml`, `template-sync-app.yml` |
| `cocogitto/cocogitto-action` | v4.2.0 | `52d0023b4396897f1815648e553db2ab8d782f3a` | `release.yml` (install only; installs cocogitto 7.0.0) |
| `AndreasAugustin/actions-template-sync` | v2.6.0 | `6e1d0089686f8940604fc15baae62367bf6ee860` | both sync workflows |

Not used although tabulated by the research: `Swatinem/rust-cache` (wrapped by setup-rust-toolchain), `actions/cache` (the Trivy database is not cached, DD-21), `actions/setup-node` and `pnpm/action-setup` (Node tools run in the pinned Node image), `bats-core/bats-action` (DD-06), `zizmorcore/zizmor-action` and `oasdiff/oasdiff-action` (the Make targets run the tools), `dtolnay/rust-toolchain`.

### Conventions for every workflow

- `runs-on: ubuntu-24.04` for every job; `timeout-minutes` on every job (at most 45, one documented exception in `scheduled.yml`).
- Top level `permissions: contents: read`. A job-level block replaces the top-level block, it does not add to it, so a job that needs more restates `contents: read` and adds only what it uses (G31 rule (a)); without it the checkout fails in a private repository.
- `concurrency: { group: "${{ github.workflow }}-${{ github.event.pull_request.number || github.ref }}", cancel-in-progress: ${{ github.event_name == 'pull_request' }} }`; release, sync and scheduled workflows never cancel a running run.
- Workflow `env`: `CARGO_TERM_COLOR: always`, `CARGO_INCREMENTAL: '0'`. `CI=true` selects native mode in the Makefile (DD-01) and `CARGO_BUILD_WARNINGS=deny`, which the toolchain action sets as well (its input `build-warnings` defaults to `deny`).
- No `${{ … }}` expression inside `run:`; values reach scripts through `env:`. No `pull_request_target`. No step pushes to a branch except in `release.yml`.
- Pull requests from forks and from Dependabot run every job with a read-only token and without secrets; the steps that need more are skipped by condition (DD-13).
- The 11 workflows of the first version were derived into files by the validation round. With the structural changes of this revision applied to those files (the six-group Bats matrix with both image builds, the sized mutation matrix, the read-only gate job with the separate upload job and block D, the scheduled seeds, the release condition, no cache step in `image-scan`), actionlint 1.7.12 exits 0 and zizmor 1.32.0 (`--offline`, with `.github/zizmor.yml`) reports no finding and one ignored one, the comment of block S2 in `bats-group`. The cache keys of the table below were not part of that run.

Step blocks referenced by the job tables:

| Block | Steps |
| --- | --- |
| S1 checkout | `actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1` with `persist-credentials: false`; `fetch-depth: 0` where the job compares with a base ref ("full") |
| S2 rust | `actions-rust-lang/setup-rust-toolchain@a188e68d0fd2bd36ad51dccb2b07ba6e75105f30 # v2.0.1` with `cache-shared-key: <key>` (the key is given in parentheses) and `cache-save-if`, which is `${{ github.ref == 'refs/heads/main' }}` in the job that the cache table names as saver and `false` in the other jobs of the key; "no cache" means `cache: false` |
| S3 tools | `taiki-e/install-action@cfdb7ed4ccc7c7dce37759eae89dc5674b469b39 # v2.90.3` with `tool: <name>@<version>,…` and `fallback: none`; `sqlx-cli@0.9.0` and `clippy-sarif@0.8.0` (in `bats-group` also `cocogitto@7.0.0`: the second `tool:` list there is `sqlx-cli@0.9.0,clippy-sarif@0.8.0,cocogitto@7.0.0`) are a second step with `fallback: cargo-binstall` (they are not in the action's manifest; a prebuilt `sqlx-cli` 0.9.0 exists in cargo-quickinstall) |
| S4 services | not a step: the job's targets start what they use through their prerequisites (`ensure-database`, `ensure-deps`, `setup-test-db`); S4 marks a job that needs PostgreSQL ("database") or PostgreSQL and LocalStack ("both") |
| S5 image | `docker/setup-buildx-action@f87e5991a6d7451dcb8d9637bfbc97413f497069 # v4.4.1`; `docker/build-push-action@c3c9e263c25d99ce0380d002d59b67737d91b0dc # v7.4.0` with `context: .`, `target: prod`, `load: true`, `tags: ${{ env.APP_IMAGE }}`, `cache-from: type=gha,scope=prod-image`, `cache-to` the same scope with `mode=max` on `main` only. The job sets `env: APP_IMAGE: service-under-test:prod` and `BUILD_IMAGE: '0'`, so its targets use the loaded image |
| S5d development image | as S5 with `target: dev`, `tags: ${{ env.DEV_IMAGE }}` (`DEV_IMAGE: service-under-test:dev`), scope `dev-image` and `secrets: github_token=${{ github.token }}` (raises the rate limit of the tool downloads in the `dev` stage); with `BUILD_IMAGE: '0'` `make start` then uses the loaded image |
| S6 logs | `if: ${{ failure() }}`: `run: docker compose logs --no-color` |
| S7 bats | `run: scripts/install-bats.sh` |
| D publication decision | a first step with `id: publish`, the inputs `vars.CODE_SCANNING_ENABLED`, the event name, the head repository and the author of the pull request in `env:`, and `GH_TOKEN: ${{ github.token }}`. It sets `publish=false` with the notice `Code scanning publication skipped: <reason>` for a fork or Dependabot pull request, for the setting `false`, and for an unset setting in a repository whose visibility (`gh api "repos/$GITHUB_REPOSITORY" --jq .visibility`) is not `public`; otherwise `publish=true`. Every later step of the job has `if: ${{ steps.publish.outputs.publish == 'true' }}` (DD-13) |
| C `changes` job | check name `Changes (<workflow name>)`, for example `Changes (Quality)`: unique across the workflows and a required check (DD-08, `docs/repository-settings.md`); S1 full; `run: scripts/ci/detect-changes.sh >> "$GITHUB_OUTPUT"` with `BASE_SHA` and `HEAD_SHA` from the event; output `code`. On `push` and `schedule` it is `true`. It is `false` only for a documentation-only change: every changed path matches the documentation path list `*.md`, `docs/**`, `LICENSE`, `.github/ISSUE_TEMPLATE/**`, `.github/pull_request_template.md`, `.claude/skills/**`, `specs/**` (none of these is a protected item). Timeout 5 |

Rust cache keys. Jobs that build the same artifacts share a key, so that the caches of one lock file stay within the repository's cache quota; one job per key saves, on `main` only, and pull requests restore (NFR40). A job whose command selects other packages, features or targets than the saver of a key builds third-party units that the saver's entry does not hold, so it has a key of its own (except the mutation shards, whose selection follows the diff and which share the key of the saver shard: the fourth exception of NFR40, open item 5): `crate-isolation` (`cargo hack check --workspace --each-feature --locked`, each crate alone) and `query-metadata` (`cargo sqlx prepare --workspace --check -- --all-targets --all-features --locked`, with the dev-dependencies):

| Key | Saver | Other jobs that restore it |
| --- | --- | --- |
| `check` | `static-analysis` | `complexity`, `code-scanning` (in `scheduled.yml`) |
| `service-debug` | `check-requirements` | `contract-snapshots`, `docs-check` |
| `crate-isolation` | `crate-isolation` | none |
| `query-metadata` | `query-metadata` | none |
| `xtask` | `architecture` | `validate-configuration`, `architecture-model` |
| `tests` | `tests` | none |
| `e2e` | `e2e-tests` | none |
| `mutation-in-place` | `mutation-shard` (the shard with index 1, in `tests.yml` and `scheduled.yml`) | `mutation-plan`, the other shards |
| `bats-<group>`; the group `tests` uses `bats-tests-in-place` | `bats-group`, one key per group | none |

`Swatinem/rust-cache` saves nothing when the restore was an exact hit (`Cache up-to-date.`), so a key whose saver starts building something else is renamed: `mutation` and `bats-tests` were renamed when cargo-mutants started building in the checkout (NFR40).

Jobs marked "code" have `needs: changes` and `if: needs.changes.outputs.code == 'true'`. The jobs of G9, G23, G25, G26, G28 and G31 are never "code"-gated: they run for documentation-only changes too (FR94). Every gate workflow also runs on pushes to `main`; the release commit carries `[skip ci]` and starts none (FR93(c)).

### `quality.yml` — name `Quality`

Triggers: `pull_request` to `main`; `push` to `main` (which also saves the caches that pull requests restore). Jobs: C plus the following, all "code".

| Job id (check name) | Gate | Blocks and tools | Make targets | Timeout | Notes |
| --- | --- | --- | --- | --- | --- |
| `format-check` (Formatting check) | G1 | S1, S2 no cache | `format-check` | 10 | |
| `static-analysis` (Static analysis) | G2 | S1, S2 (`check`), S3 `clippy-sarif` | `static-analysis`; then, with `if: ${{ !cancelled() }}`, `static-analysis-sarif` and `actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2` (`name: clippy-sarif`, `path: target/sarif/clippy.sarif`, `retention-days: 1`) | 20 | read-only token: the job that executes the code of the pull request publishes nothing |
| `clippy-sarif-upload` (Static-analysis findings upload) | none (publication) | D; S1; `actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333 # v8.0.2` (`name: clippy-sarif`, `path: target/sarif`); `github/codeql-action/upload-sarif@33d7b85da1a04171e211befe2c739303b9a4c066 # v4.39.3` (`sarif_file: target/sarif/clippy.sarif`, `category: clippy`) | none | 10 | `needs: [changes, static-analysis]` with `if: ${{ !cancelled() && needs.changes.outputs.code == 'true' }}`, so it also runs when the gate failed (FR49); job permissions `contents: read`, `security-events: write` |
| `complexity` (Complexity limits) | G3 | S1, S2 (`check`) | `complexity` | 20 | |
| `architecture` (Architecture boundaries) | G4 | S1, S2 (`xtask`) | `architecture` | 10 | GitHub annotations come from the checker |
| `manifest` (Manifest and lock validation) | G5 | S1, S2 no cache | `validate-manifest` | 10 | |
| `crate-isolation` (Crate isolation) | G29 | S1, S2 (`crate-isolation`), S3 `cargo-hack` | `check-crate-isolation` | 20 | the gate step runs `set -o pipefail; mkdir -p target; make check-crate-isolation 2>&1 \| tee target/crate-isolation.log`; a following step with `if: ${{ always() }}` prints `Third-party crates compiled: <n>`, the `Compiling` and `Checking` lines of the log without `(/` after the ANSI codes are removed (NFR40) |
| `query-metadata` (Persistence query metadata) | G30 | S1, S2 (`query-metadata`), S3 `sqlx-cli`; S4 database; S6 | `validate-sqlx-metadata` | 20 | as `crate-isolation`, with `make validate-sqlx-metadata` and `target/query-metadata.log`; the count step comes before S6 |
| `check-requirements` (Configuration check) | G6 | S1, S2 (`service-debug`) | `check-requirements` | 20 | |
| `dependency-policy` (Dependency policy) | G7 | S1, S2 no cache, S3 `cargo-deny` | `check-security` | 10 | |
| `unused-dependencies` (Unused dependencies) | G8 | S1, S2 no cache, S3 `cargo-shear` | `unused-dependencies` | 10 | |

### `tests.yml` — name `Tests`

Triggers: `pull_request` to `main`; `push` to `main`. Jobs: C plus the following, all "code".

| Job id (check name) | Gate | Blocks and tools | Make targets | Timeout | Artifacts and notes |
| --- | --- | --- | --- | --- | --- |
| `tests` (Tests and coverage) | G11, G12, G14 | S1, S2 (`tests`), S3 `cargo-nextest`, `cargo-llvm-cov`, `sqlx-cli`; S4 both; S6 | `tests-with-coverage doc-tests`; then, with `if: ${{ !cancelled() }}`, `scripts/ci/summarize-tests.sh`, which names each gate and its result in the job summary | 30 | artifacts `coverage-html` (`target/coverage/html`) and `nextest-junit` (`target/nextest/ci/junit.xml`), uploaded with `if: ${{ !cancelled() }}` (the reports exist when the floor fails); then `codecov/codecov-action@303a32d7a59b442fa8d48b6a1cc6825c09c847a5 # v7.1.1` with `files: target/coverage/lcov.info`, `disable_search: true`, `fail_ci_if_error: false`, only when `CODECOV_TOKEN` is available (DD-13). The only job that computes coverage |
| `e2e-tests` (End-to-end tests) | G13 | S1, S2 (`e2e`), S3 `sqlx-cli`; S4 both; S6 | `e2e-tests` | 25 | no artifact: the result is the job log |
| `mutation-plan` (Mutation plan) | – | S1 full, S2 (`mutation-in-place`), S3 `cargo-mutants` | `scripts/ci/mutation-plan.sh >> "$GITHUB_OUTPUT"` with `EVENT_NAME` and `BASE_REF` in `env:`; outputs `count` and `shards` (DD-09) | 10 | not a gate: it runs no check, so it calls a script, not a Make target |
| `mutation-shard` (Mutation shard k/n, written `name: Mutation shard ${{ matrix.shard }}`, for example `Mutation shard 0/5`) | G15 | `strategy.matrix.include: ${{ fromJSON(needs.mutation-plan.outputs.shards) }}`, `fail-fast: false`; S1 full, S2 (`mutation-in-place`), S3 `cargo-mutants`, `cargo-nextest`, `sqlx-cli`; S4 both; S6; job `env` also `MUTANTS_IN_PLACE: '1'` | pull request: `mutation-tests-diff base_ref=origin/$BASE_REF MUTANTS_SHARD=$SHARD`; push: `mutation-tests MUTANTS_SHARD=$SHARD` (`SHARD: ${{ matrix.shard }}`) | 45 | `if` also requires `needs.mutation-plan.outputs.count != '0'`, because a matrix cannot be empty; artifact `mutants-${{ matrix.index }}` (`mutants.out/`), uploaded always; outside the 20- and 40-minute pull-request budgets (PD-30); after the mutation step a step (`if: ${{ always() }}`) prints `Third-party crates compiled by cargo-mutants in this shard: <n>`, counted from the `Compiling` lines without a path in `mutants.out/log/*.log` (the NFR40 evidence of a rerun, in the job log) |
| `mutation-tests` (Mutation tests) | G15 | none | `if: ${{ always() }}`; fails unless the shard job succeeded or was skipped | 5 | the required check |
| `bats-group` (Bats: group) | G16 | matrix `include` of six `{group, image}` objects: `surface` (`dev`), `quality`, `tests`, `contracts` (`prod`), `load` (`prod`), `repository` (`none` where no image is named); S1 full, S2 (`bats-<group>`, for the group `tests` `bats-tests-in-place`, written `${{ matrix.group == 'tests' && 'bats-tests-in-place' \|\| format('bats-{0}', matrix.group) }}`), S3 all cargo tools, S7; S5 when `matrix.image == 'prod'`, S5d when `matrix.image == 'dev'`; S4; S6 | `bats` with `BATS_ARGS="--filter-tags $GROUP"` | 30 | job `env`: `APP_IMAGE`, `DEV_IMAGE`, `BUILD_IMAGE: '0'`, `MUTANTS_IN_PLACE: '1'` (read by the G15 test of `make_tests.bats`). The `uses:` line of S2 ends with `# zizmor: ignore[cache-poisoning] images are loaded locally, never pushed`, on the same line as the version comment (verified: zizmor reports the Rust cache next to an image build as cache poisoning, and honours the comment only on that line) |
| `bats` (Makefile tests (Bats)) | G16 | none | `if: ${{ always() }}`; fails unless every group succeeded or was skipped | 5 | the required check |

### `contracts.yml` — name `Contracts`

Triggers: `pull_request` to `main`; `push` to `main`. Jobs: C plus the following, all "code".

| Job id (check name) | Gate | Blocks and tools | Make targets | Timeout |
| --- | --- | --- | --- | --- |
| `contract-snapshots` (Contract snapshots) | G17 | S1, S2 (`service-debug`) | `validate-contract-snapshots` | 20 |
| `openapi-lint` (OpenAPI lint) | G18 | S1 | `validate-openapi-spec` | 10 |
| `schemathesis` (Schemathesis) | G21 | S1, S5 | `schemathesis-validate` | 30 |

### `change-control.yml` — name `Change control`

Triggers: `pull_request` to `main` with `types: [opened, edited, reopened, synchronize, labeled, unlabeled]`, so that a title edit or a label change re-evaluates the jobs without a new commit; `push` to `main`. Jobs: C plus:

| Job id (check name) | Gate | Blocks and tools | Command | Timeout | Notes |
| --- | --- | --- | --- | --- | --- |
| `title` (Pull-request title) | G28 | S1 | `make check-commit-message` with `env: COMMIT_TITLE: ${{ github.event.pull_request.title }}` | 5 | pull requests only; skipped for Dependabot's and for those labelled `template_sync` (DD-15) |
| `validate-configuration` (Protected configuration) | G25 | S1 full, S2 (`xtask`) | `make validate-configuration base_ref="$BASE"` with `CONFIG_CHANGE_APPROVED` | 10 | pull request: `BASE` is `origin/<base branch>` and the variable is `${{ contains(github.event.pull_request.labels.*.name, 'approved-config-change') }}`. Push: `BASE` is `github.event.before` and the variable is `true`, because the merged pull request was the approval point; modifications are printed as approved. A push whose `github.event.before` is the all-zero commit (the push that creates `main`, for example the first commit of a repository made from the template) has nothing to compare with: the job has `if: github.event_name == 'pull_request' \|\| github.event.before != '0000000000000000000000000000000000000000'` and does not run, and a skipped job counts as passed for a required check |
| `openapi-diff` (OpenAPI breaking changes), "code" | G19 | S1 full | `make openapi-diff base_ref="$BASE"` with `BREAKING_CHANGE_APPROVED` from the label `approved-breaking-change` | 10 | same base and push rule; the all-zero condition is joined to the "code" condition |
| `graphql-diff` (GraphQL breaking changes), "code" | G20 | S1 full | `make graphql-diff base_ref="$BASE"` with the same variable | 10 | same |

### `load-tests.yml` — name `Load tests`

Triggers: `pull_request` to `main`; `push` to `main`. Jobs: C plus the following, both "code". Only the smoke profile runs in a pipeline (PD-23).

| Job id (check name) | Gate | Blocks | Make targets | Timeout | Artifacts |
| --- | --- | --- | --- | --- | --- |
| `smoke-load-tests` (Smoke load tests) | G22 | S1, S5 | `smoke-load-tests` | 30 | `load-test-results` (`tests/Load/results/`), uploaded with `if: ${{ !cancelled() }}`, retention 14 days |
| `lint-load-tests` (Load-script lint) | G24 | S1 | `lint-load-tests` | 10 | |

### `security.yml` — name `Security`

Triggers: `pull_request` to `main`; `push` to `main`. Jobs: C plus:

| Job id (check name) | Gate | Blocks and tools | Make targets | Timeout | Notes |
| --- | --- | --- | --- | --- | --- |
| `secrets-scan` (Secrets scan) | G9 | S1 full | `secrets-scan` | 10 | always runs, also for documentation-only changes |
| `image-scan` (Production-image scan), "code" | G10 | S1, S5 | `image-scan` | 20 | no cache step: Trivy downloads its database in the job (DD-21) |
| `codeql` (CodeQL), "code", matrix `language: [rust, actions]` | none (publication) | D; S1; `github/codeql-action/init@33d7b85da1a04171e211befe2c739303b9a4c066 # v4.39.3` with `languages` and `build-mode: none`; `github/codeql-action/analyze@33d7b85da1a04171e211befe2c739303b9a4c066 # v4.39.3` with `category: /language:<language>` | none (CI-only, DD-24) | 30 | job permissions `contents: read`, `actions: read`, `security-events: write` |

### `repository.yml` — name `Repository`

Triggers: `pull_request` to `main`; `push` to `main`. Jobs: C plus:

| Job id (check name) | Gate | Blocks and tools | Make targets | Timeout |
| --- | --- | --- | --- | --- |
| `lint-repo` (Repository lint) | G23 | S1, S2 no cache, S3 `taplo`, `typos`, `zizmor` | `lint-repo` | 15 |
| `invariants` (Repository invariants) | G31 | S1 | `validate-invariants` | 10 |
| `docs-check` (Documentation drift) | G26 | S1, S2 (`service-debug`) | `docs-check` | 20 |
| `architecture-model` (Architecture model), "code" | G27 | S1, S2 (`xtask`) | `validate-architecture-model` | 10 |

### `scheduled.yml` — name `Scheduled verification`

Triggers: `schedule: cron '23 4 * * 1'` (Mondays 04:23 UTC) and `workflow_dispatch`. Concurrency group `scheduled`, never cancelled. The schedule runs on the default branch; a `workflow_dispatch` run uses the branch it is started on (no job condition checks `github.ref`; only the cache steps save on `main` alone). Workflow `env` adds `TEST_SEED: ${{ github.run_id }}` and `PROPTEST_RNG_SEED: ${{ github.run_id }}`: the scheduled run replaces the seed recorded in `.cargo/config.toml` with a fresh one (FR51, NFR29), and the jobs that run tests print both values in their job summary so that a failure can be reproduced. `scheduled.yml` also sets `CARGO_PROFILE_DEV_DEBUG: '0'`: its clean-clone job installs the toolchain with `rustup show`, not with the toolchain action that sets both variables for the pull-request jobs. Every job ends with a step (`if: ${{ failure() }}`) that writes the failing target and the last lines of its output to the job summary.

| Job id | Purpose | Steps | Timeout |
| --- | --- | --- | --- |
| `clean-bootstrap-ci` | FR95, NFR34: a clean checkout with no restored cache reaches a passing `make ci`; NFR35: `make start` with images and build output present returns within 60 s | S1 full; `rustup show` (installs the pinned toolchain); `make bootstrap` with `GITHUB_TOKEN: ${{ github.token }}`; `make stop` (step `Stop the stack`, id `stop`) and a timed `make start` (step `Start the stack and time it`, id `start`: log `target/start.log`, the line `make start: <seconds> s (budget 60 s)` in the log and the job summary, exit 1 above 60 s; before `make ci`, so that a dispatch on a branch, whose `make ci` stops at the configuration guard, measures it); `make ci`; the step `Require a clean working tree` (id `clean`) fails with `make bootstrap and make ci changed the working tree` when `git status --porcelain` is not empty (FR68), and `Summarize the failure` names it, or the failed step `make bootstrap`, `make stop`, `make start (budget 60 s)` or `make ci`; on failure upload `mutants.out/` and `tests/Load/results/`. The run is also the determinism check of NFR7: the snapshots are generated after a clean build by `validate-contract-snapshots`, and the integration suite generates each one 20 times | 75 (the limit NFR39 sets for this job) |
| `mutation-plan`, `mutation-shard`, `mutation-tests` | the full mutation run | the three jobs of `tests.yml` as they run on a push | 10, 45, 5 |
| `security-gates` | G7, G9 and G10 against the current advisory and vulnerability databases | S1 full, S2 no cache, S3 `cargo-deny`, S5; `make secrets-scan check-security`; `make image-scan` | 30 |
| `code-scanning` | the two publications of DD-24 | D; as job `codeql` (both languages); then S2 (`check`), S3 `clippy-sarif`, `make static-analysis-sarif` and the SARIF upload; job permissions `contents: read`, `actions: read`, `security-events: write` | 30 |
| `devcontainer-build` | FR118: the development container definition builds, starts, runs its `postCreateCommand`, reaches the inner container engine and runs a toolchain target | S1; with the runner's Node.js, each step with its own log under `target/` that `Summarize the failure` names (with the end of the log): `npx --yes @devcontainers/cli@0.90.0 build --workspace-folder .` (id `build`); `npx --yes @devcontainers/cli@0.90.0 up --workspace-folder .` (id `up`: starts the container with the docker-in-docker entry point and runs `postCreateCommand`, `make install-hooks && make doctor`); `npx --yes @devcontainers/cli@0.90.0 exec --workspace-folder . docker info` (id `engine`); `... exec --workspace-folder . make format-check` (id `toolchain`); `... exec --workspace-folder . make start-deps` (id `deps`: compose on the inner engine); every CLI version equals `DEVCONTAINER_CLI_VERSION` (G31 rule (b)) | 30 |

A failed scheduled run is a failed workflow run with GitHub's standard notification; no issue is opened automatically. A failure caused by a newly published advisory or vulnerability is a correct result (NFR34).

### `release.yml` — name `Release`

Triggers: `push` to `main`; `workflow_dispatch` with the optional input `version` (`x.y.z`). Concurrency group `release` with `cancel-in-progress: false`: a running release is never cancelled; GitHub keeps one queued run per group, so a queued run may be superseded by a newer one, which then releases the same commits. `permissions: contents: read` (all writes use the App token). One job, `release`, timeout 15, with `if: ${{ github.event_name == 'workflow_dispatch' || !contains(github.event.head_commit.message, '[skip ci]') }}`.

1. **Detect credentials.** Map `VILNACRM_APP_ID` and `VILNACRM_APP_PRIVATE_KEY` to environment variables; when either is empty, write the notice `Release skipped: GitHub App secrets are not configured` and set `available=false`. Every later step has `if: steps.credentials.outputs.available == 'true'`.
2. **Token.** `actions/create-github-app-token@64ab729a6294000f5aa4a4bc0b73d1f32bc45510 # v3.3.0` with `app-id`, `private-key`, `permission-contents: write`. (`app-id` is deprecated in favour of `client-id`; it is kept because scope 8.9 fixes the secret names. `.github/AUTORELEASE.md` records the switch for the next major version of the action.)
3. **Checkout.** S1 with `fetch-depth: 0`, `token: <App token>` and `persist-credentials: true`; this is the only checkout of `release.yml` that keeps credentials (the checkouts of the two template-sync workflows keep theirs as well, see "Template sync"), with a `zizmor: ignore[artipacked]` comment that says why.
4. **Tools.** S2 without cache (cargo is needed for `cargo update --workspace`); `cocogitto/cocogitto-action@52d0023b4396897f1815648e553db2ab8d782f3a # v4.2.0` with `install-only: true` (cocogitto 7.0.0).
5. **Identity.** `git config user.name "<app-slug>[bot]"` and `user.email "<bot id>+<app-slug>[bot]@users.noreply.github.com"`, both from the token step's `app-slug` output and `gh api "/users/<app-slug>[bot]"`.
6. **Release.** `scripts/release.sh` with `GH_TOKEN` set to the App token:
   1. `next="$(cog bump --auto --dry-run)"`; then the commits that cocogitto reads in the range of the bump: `cog changelog "<start>..HEAD" --template <temporary file>` with the template `{% for commit in commits %}{{ commit.id }} {{ commit.breaking_change }}` newline `{% endfor %}`, where `<start>` is the highest `vX.Y.Z` tag, or the oldest commit when there is none; the lines are kept whose id `git rev-list --no-merges` lists for `<tag>..HEAD`, or for `HEAD` when there is no tag (cocogitto reads the oldest commit then). A commit that cocogitto marks breaking is a breaking commit when its subject has `!` before the colon or its footer section holds a breaking footer (Version rules); otherwise it is only marked. When no `v*` tag exists, or below `v1.0.0`, or from `v1.0.0` on without a marked commit: when the output of the dry run is not `v<major>.<minor>.<patch>` and there is no breaking commit, print the output and exit 0: nothing to release. From `v1.0.0` on with a marked commit and no breaking commit, take the bump of the `feat` (minor) and `fix` (patch) commits among those read, the type being the subject up to the first `(`, `!` or `:`, any case; without one, print `release: nothing to release since <tag>: ...` and exit 0.
   2. `cog bump --auto --skip-ci`; when no release tag `vX.Y.Z` exists yet (a prerelease tag such as `v0.1.0-rc.1` does not count), `cog bump --version <manifest version> --skip-ci` (the first release, DD-11); with a breaking commit, `cog bump --minor --skip-ci` while the latest tag is `v0.y.z` and `cog bump --major --skip-ci` from `v1.0.0` on; from `v1.0.0` on with a marked commit and no breaking commit, `cog bump --minor --skip-ci` or `cog bump --patch --skip-ci` (6.1); in a manual run with the input, `cog bump --version <x.y.z> --skip-ci`. cocogitto runs the pre-bump hook `scripts/set-version.sh <version>` (sets `[workspace.package].version`, runs `cargo update --workspace`, checks `cargo metadata --locked`), prepends the new section to `CHANGELOG.md`, commits and tags `v<version>`.
   3. `git commit --amend --no-verify -m "chore(release): v<version> [skip ci]"` (an installed `commit-msg` hook would reject this title, which has no issue number); `git tag -f "v<version>"`.
   4. `git push origin HEAD:main`; `git push origin "v<version>"`. A non-fast-forward rejection fails the job; the next push to `main` releases both changes.
   5. `cog changelog --at "v<version>" > target/release-notes.md`; `gh release create "v<version>" --title "v<version>" --notes-file target/release-notes.md --verify-tag`.

Version rules: `fix` patch, `feat` minor, footer `BREAKING CHANGE:` major whatever the type (the title check rejects `!` after the type) (minor while the version is `0.y.z`, DD-11); `build`, `chore`, `ci`, `docs`, `perf`, `refactor`, `style`, `test` cause no release unless the commit carries the footer. Footers follow the body after a blank line: the footer section is the run of paragraphs at the end of the message, after the subject paragraph, that each start with a footer (a token of letters, digits and `-`, or `BREAKING CHANGE`, then `: ` or ` #`); the breaking footer is a line of that section that starts with the upper-case token `BREAKING CHANGE` or `BREAKING-CHANGE`, a colon, a blank and a value (items 8 to 12, 15 and 16 of the conventional-commit definition 1.0.0); a `!` before the colon of the subject counts as well (item 13). The same line in a paragraph of the body, indented, inside a sentence, in lower case or without the blank after the colon is no breaking change. cocogitto writes the changelog with its own, wider reading (any line of the body that starts with the token), so it marks such a commit **BREAKING** although the line raises no version. The release commit carries `[skip ci]`, so it starts no workflow; the job guard is a second barrier. Skip conditions: secrets absent; head commit already a release commit; no releasable commit (no `fix`, no `feat`, no breaking change). The first release is `v0.1.0`, the manifest version, whatever the first releasable commit is: a `feat`, a `fix` or a breaking change of any type (DD-11). A maintainer starts a manual version with the `workflow_dispatch` input `version`, which runs `scripts/release.sh --version <x.y.z>`.

`cog.toml` (validated with cocogitto 7.0.0):

```toml
tag_prefix = "v"
branch_whitelist = ["main"]
ignore_merge_commits = true
skip_ci = "[skip ci]"
pre_bump_hooks = ["scripts/set-version.sh {{version}}"]

[changelog]
path = "CHANGELOG.md"
template = ".cog/changelog.tera"
authors = []

# Only user-visible changes appear in the changelog.
[commit_types]
build = { omit_from_changelog = true }
chore = { omit_from_changelog = true }
ci = { omit_from_changelog = true }
docs = { omit_from_changelog = true }
refactor = { omit_from_changelog = true }
style = { omit_from_changelog = true }
test = { omit_from_changelog = true }
```

`.cog/changelog.tera` (renders `v0.2.0 (2026-11-10)` as a second-level heading, `### Features`, `### Bug Fixes`, and entries such as `- **#12:** add health endpoint (#13) (14e36a3)`). The listing is indented by four spaces for display; the file itself has no indentation:

```text
    {% if version.tag -%}
    ## {{ version.tag }} ({{ date | date(format="%Y-%m-%d") }})
    {% else -%}
    ## Unreleased
    {% endif %}
    {% for value in commits | group_by_type -%}
    ### {{ value.0 | upper_first }}

    {% for commit in value.1 -%}
    - {% if commit.breaking_change %}**BREAKING** {% endif %}{% if commit.scope %}**{{ commit.scope }}:** {% endif %}{{ commit.summary }} ({{ commit.id | truncate(length=7, end="") }})
    {% endfor %}
    {% endfor -%}
```

`CHANGELOG.md` is committed with cocogitto's header (`# Changelog`, the introduction line, `- - -`) and footer, so the first release inserts its section between them.

### Template sync

Both workflows: `workflow_dispatch` plus a schedule; concurrency group `template-sync`, never cancelled; `permissions: contents: read`; workflow `env: TEMPLATE_REPOSITORY: VilnaCRM-Org/rust-service-template` (the sync source setting, the only occurrence of the name in the file); one job `sync`, timeout 15. The first step ends the job with the notice `Template sync skipped: this is the template repository` when `$GITHUB_REPOSITORY` equals `$TEMPLATE_REPOSITORY`; every later step depends on its output.

| | `template-sync-app.yml` (name `Template sync (App)`) | `template-sync-pat.yml` (name `Template sync (PAT)`) |
| --- | --- | --- |
| Schedule | `0 9 * * MON` | `0 0 1 * *` |
| Credentials step | needs `VILNACRM_APP_ID` and `VILNACRM_APP_PRIVATE_KEY`; absent: notice, rest skipped | needs `PERSONAL_ACCESS_TOKEN`; absent: notice, rest skipped; also skipped with a notice when both App secrets exist (PD-14) |
| Token | `actions/create-github-app-token@64ab729a6294000f5aa4a4bc0b73d1f32bc45510 # v3.3.0` with `permission-contents: write`, `permission-pull-requests: write`, `permission-workflows: write` | the PAT (fine-grained: Contents, Pull requests and Workflows read and write; Metadata read) |
| Checkout | S1 with `token:` the token above and `persist-credentials: true` (the action pushes its own sync branch) | same |
| Sync | `AndreasAugustin/actions-template-sync@6e1d0089686f8940604fc15baae62367bf6ee860 # v2.6.0` with `source_gh_token` and `target_gh_token` set to the token, `source_repo_path: ${{ env.TEMPLATE_REPOSITORY }}`, `upstream_branch: main`, `pr_labels: template_sync`, `pr_title: 'chore(template): merge template changes'`, `is_force_push_pr: true` (a run for the template commit of the open sync pull request force-pushes its branch and updates the pull request in place), `is_pr_cleanup: true` (a run for a newer template commit closes the older open `template_sync` pull requests), `is_keep_branch_on_pr_cleanup: true` (it closes them without deleting their branches, so commits that a maintainer pushed to one are kept) | same |

`.templatesyncignore` (no comments; the reasons are in the sync guides and in the table below). The pinned sync action drops blank lines and everything from a `#` to the end of a line and hands every remaining entry to `git reset --` (`handle_templatesyncignore` in `src/sync_template.sh` of `AndreasAugustin/actions-template-sync` v2.6.0, lines 477 to 490), so each entry is a git pathspec relative to the repository root, not a `.gitignore` pattern: a name without a slash (`Cargo.toml`) is the root file only, `dir/` is everything below that directory, `*` also matches `/`, and an entry must not start with `/` or contain `..` (git refuses a path outside the repository and the sync aborts). The crate manifests below `crates/shared/`, `crates/internal/` and `xtask/` and `docs/README.md` are therefore synchronised; of the crate manifests, only the workspace `Cargo.toml`, `Cargo.lock` and `crates/service/Cargo.toml` (by entries of their own) and those of the service-owned crates (by their directory entries) are ignored. The first block is what FR100(d) calls service-owned. The second block is the template-owned files that a service extends (FR100(f)). A repository created from a template shares no history with it, so the sync action merges without a common ancestor and resolves every difference in favour of the template, which would delete a service's additions; these files are therefore ignored, and their template changes are applied by hand from the list in the sync guide. Everything else is synchronised, including `crates/shared/`, the rest of `crates/internal/` and of `crates/service/`, `xtask/`, `features/health_check.feature`, `tests/Load/scripts/health.js`, the Bats suite, the workflows, `Makefile`, `make/`, the scripts, the guidance and the skills. A file that only the service has is never touched by a sync. No entry is needed for the project-name variable, because it is derived (DD-23).

```text
.templatesyncignore
README.md
CHANGELOG.md
Cargo.toml
Cargo.lock
.sqlx/
migrations/
.github/openapi-spec/
.github/graphql-spec/
workspace.dsl
bmalph/config.json
specs/
crates/company-subdomain/
features/api_docs.feature
features/example_api_resource.feature
features/graphql.feature
features/http_errors.feature
tests/Load/config.json.dist
docs/requests.http

.env
.env.test
.github/dependabot.yml
.claude/skills/README.md
.cargo/mutants.toml
.gitleaksignore
.trivyignore
_typos.toml
architecture.toml
deny.toml
docs/adr/
docs/environment.md
crates/internal/health-check/domain/src/value_object/dependency.rs
crates/internal/health-check/domain/src/value_object/dependency/
crates/internal/health-check/infrastructure/src/probe.rs
crates/service/Cargo.toml
crates/service/src/bootstrap.rs
crates/service/src/bootstrap/
crates/service/src/cli.rs
crates/service/src/cli/
crates/service/src/config.rs
crates/service/src/config/
crates/service/src/http.rs
crates/service/src/http/router/tests.rs
crates/service/tests/integration/bootstrap.rs
crates/service/tests/integration/migrate.rs
crates/service/tests/integration/serve.rs
crates/service/tests/integration/panic.rs
crates/service/src/openapi.rs
crates/service/src/openapi/
crates/service/src/graphql/schema.rs
crates/service/src/graphql/schema/
tests/support/
tests/e2e/
```

Template-owned files a service extends, and what the sync does with each (FR100(f)):

| File | What a service adds | Handling |
| --- | --- | --- |
| `.env`, `.env.test`, `docs/environment.md` | the variables of its own adapters and their reference entries (drift rule (5)) | ignored |
| `docs/adr/` | its own decision records | ignored |
| `.github/dependabot.yml` | ecosystems and ignore rules | ignored |
| `.claude/skills/README.md` | the index lines of its own skills (drift rule (7)) | ignored; a new template skill arrives by sync, its index line by hand |
| `architecture.toml` | third-party crates on a layer's allow-list (a labelled change) | ignored |
| `deny.toml`, `.trivyignore`, `.gitleaksignore`, `_typos.toml`, `.cargo/mutants.toml` | dated exceptions: advisories and licences, image findings, scan fingerprints, accepted words, equivalent mutants (NFR9, NFR11, NFR20) | ignored |
| `crates/internal/health-check/domain/src/value_object/dependency.rs` with its `dependency/` tests; `crates/internal/health-check/infrastructure/src/probe.rs` | a dependency and the module declaration of its probe (the health-probe recipe of the core document); a probe that needs a new client crate also changes `crates/internal/health-check/infrastructure/Cargo.toml` | ignored; the manifest is synchronised (an entry is a git pathspec, and `Cargo.toml` is the root file only), so a service whose probe changes it lists it by its full path in its own `.templatesyncignore` in the same change |
| `crates/service/Cargo.toml` and `src/bootstrap`, `src/cli`, `src/config`, `src/http.rs`, `src/openapi`, `src/graphql/schema` of that crate | wiring, commands, configuration, routes and schema roots of its contexts | ignored |
| `crates/service/src/http/router/tests.rs`; `crates/service/tests/integration/bootstrap.rs`, `migrate.rs`, `serve.rs` and `panic.rs` | the fields of its own adapters in the `Adapters` literal of the router test; its own routes, tables and migration versions in place of those of the example context | ignored |
| `tests/support/`, `tests/e2e/` | test doubles, world and step definitions | ignored |
| files changed through an extension seam (`docs/extension-seams.md`) | the change the seam describes | not ignored by the template, so router, telemetry and shutdown fixes keep reaching every service; a service that uses a seam adds each synchronised path it changes to its own `.templatesyncignore` in the same change and applies later template changes by hand |
| `Makefile`, `docs/make-targets.md` | Make targets | synchronised, not ignored: a service defines its targets in `make/service.mk`, which the `Makefile` loads with `-include`, and documents them in `docs/make-targets-service.md`. The template ships neither file, so a sync cannot change them, and drift rule (6) reads both references |

### Dependabot, review bot and templates

`.github/dependabot.yml` (in Prettier's format; it passes `yamllint --strict` with the configuration below):

```yaml
version: 2
updates:
  - package-ecosystem: cargo
    directory: /
    schedule: { interval: weekly, day: monday }
    cooldown: { default-days: 3 }
    groups:
      cargo: { patterns: ['*'] }
    commit-message: { prefix: build, include: scope }
  - package-ecosystem: rust-toolchain
    directory: /
    schedule: { interval: weekly, day: monday }
    groups:
      toolchain: { patterns: ['*'] }
    commit-message: { prefix: chore, include: scope }
  - package-ecosystem: github-actions
    directory: /
    schedule: { interval: weekly, day: monday }
    groups:
      actions: { patterns: ['*'] }
    commit-message: { prefix: ci, include: scope }
  - package-ecosystem: docker
    directory: /
    schedule: { interval: weekly, day: monday }
    groups:
      docker: { patterns: ['*'] }
    commit-message: { prefix: build, include: scope }
  - package-ecosystem: docker-compose
    directory: /
    schedule: { interval: weekly, day: monday }
    groups:
      compose: { patterns: ['*'] }
    ignore:
      - dependency-name: localstack/localstack # frozen at the tokenless community tag 3.4.0
    commit-message: { prefix: build, include: scope }
  - package-ecosystem: npm
    directory: /tests/Load
    schedule: { interval: monthly }
    groups:
      load-test-tooling: { patterns: ['*'] }
    commit-message: { prefix: chore, include: scope }
```

No `reviewers` key (removed from Dependabot in 2025; CODEOWNERS is a follow-up). Every ecosystem has a group, so each opens one pull request per run (FR99). LocalStack is excluded from the compose updates because it is pinned on purpose to the tokenless community tag `3.4.0` (NFR13, scope 8.4). A toolchain update touches three protected places that `validate-invariants` keeps equal (`rust-toolchain.toml`, the builder image tag in the `Dockerfile`, `rust-version` in the workspace manifest): Dependabot opens one pull request for the toolchain file and one for the images, and a maintainer combines them into one labelled pull request that also sets `rust-version`; `docs/operations.md` describes the procedure. Both base images are literal `FROM` references with tag and digest, the form the `docker` ecosystem updates.

`.coderabbit.yaml` is the PHP file unchanged (`reviews.request_changes_workflow: true` with the schema comment). It has no effect until the CodeRabbit App is installed: no workflow step acts on its settings. The job `lint-repo` lints it like every YAML file, and the test "FR97: .coderabbit.yaml is the PHP template file with the request-changes workflow" of `make_general.bats` (job `bats-group`, group `surface`) reads it: it fails when the file is missing, when the setting is not under `reviews:` or when the file differs from the PHP file.

| Template | Front matter | Sections |
| --- | --- | --- |
| `.github/ISSUE_TEMPLATE/bug_report.md` | `name: Bug report`, `labels: bug` | Summary; Steps to reproduce (the fields `Endpoint (method and path)` and `Request (headers and body, secrets removed)`, then the request, Make target or command as numbered steps); Expected behaviour; Actual behaviour (status code, problem body, `x-request-id`, log lines); Environment (commit or version, `make doctor` output, OS, Docker and Compose versions); Additional context; no browser or device field |
| `.github/ISSUE_TEMPLATE/feature_request.md` | `name: Feature request`, `labels: enhancement` | Problem statement; Proposed change; Affected area (API contract, runtime, quality gates, CI, documentation); Tasks; Acceptance criteria (measurable, testable) |
| `.github/pull_request_template.md` | none | Description; Related issue (`Closes #`); Type of change; How it was tested (targets run, targets not run and why); Checklist: title is `type(#issue): subject`; `make ci` or the relevant gates pass; contract snapshots regenerated when the API changed; `workspace.dsl` updated when the architecture changed; documentation updated; no protected configuration changed, or the label `approved-config-change` requested with a reason; breaking change labelled `approved-breaking-change`; link `CONTRIBUTING.md` (relative) |

### Secrets, variables and labels

| Name | Kind | Used by | When absent |
| --- | --- | --- | --- |
| `VILNACRM_APP_ID`, `VILNACRM_APP_PRIVATE_KEY` | secrets | `release.yml`, `template-sync-app.yml` | job succeeds with a notice; nothing is released or synchronised |
| `PERSONAL_ACCESS_TOKEN` | secret | `template-sync-pat.yml` | job succeeds with a notice |
| `CODECOV_TOKEN` | secret | `tests.yml` / `tests` | upload step skipped with a notice; the coverage gate is unaffected |
| `GITHUB_TOKEN` | automatic | read access everywhere; `security-events: write` only in `clippy-sarif-upload`, `codeql` and the scheduled `code-scanning` job; build secret of the development image in the Bats job | n/a |
| `CODE_SCANNING_ENABLED` | repository variable | step block D (`clippy-sarif-upload`, `codeql`, `code-scanning`) | unset: published in a public repository, skipped with a notice in a private one; `true` enables publication in a private repository that has code scanning; `false` switches it off |
| `approved-config-change`, `approved-breaking-change` | labels | `change-control.yml` | gate fails on a protected or breaking change |
| `template_sync` | label | sync workflows, title exemption | created by the sync action |
| `bug`, `enhancement` | labels | issue templates | GitHub defaults |

GitHub App permissions needed: Contents (read and write), Pull requests (read and write), Workflows (read and write), Metadata (read); the App must be allowed to push to `main` in the branch protection. `GPG_PRIVATE_KEY` and `GPG_PASSPHRASE` are not used.

## Git Hooks and Commit Convention

Hooks live in the committed directory `.githooks/` and are activated per clone by `make install-hooks` (`git config core.hooksPath .githooks`), which `make bootstrap` and the development container's `postCreateCommand` run. Hooks are short POSIX shell files, mode 755, that call Make targets only. Each chooses the GNU Make it runs as `make_command`: `$MAKE` when it is set, else `gmake` when it exists (macOS keeps an old make as `make`), else `make`:

| File | Content | What runs | Needs |
| --- | --- | --- | --- |
| `.githooks/commit-msg` | `COMMIT_MSG_FILE="$1" exec "$make_command" --no-print-directory check-commit-message` | `scripts/check-commit-message.sh` in hook mode | nothing |
| `.githooks/pre-commit` | `exec "$make_command" --no-print-directory pre-commit` | `format-check`, `lint-code`, `architecture`, `check-security-offline`, `unused-dependencies` (QG-01, QG-02, QG-03, QG-05, the offline subset of QG-09, QG-10) | no service, no network; seconds when the build is warm |
| `.githooks/pre-push` | `exec "$make_command" --no-print-directory pre-push` | `unit-tests` (QG-13) | no service |

Commit-message rule, applied case-insensitively with `grep -Ei` to the first line that is not a comment:

```text
^(feat|fix|refactor|build|chore|ci|docs|style|perf|test)\(#[0-9]+\): .*
```

- Ten types; the scope is mandatory and is `#` plus an issue number; a colon and one space follow. This is the PHP expression `/^(feat|fix|refactor|build|chore|ci|docs|style|perf|test)\(\#\d+\): .*/i` written as a POSIX extended expression.
- On failure the script prints `The commit message should be in the format: feat|fix|refactor|build|chore|ci|docs|style|perf|test(#1): description`, then `got: <subject>`, and exits 1.
- Hook mode (`COMMIT_MSG_FILE`) accepts without checking subjects that start with `Merge `, `Revert "`, `Reapply "`, `fixup! `, `squash! ` or `amend! `, which git writes itself (for the revert of a revert, git 2.43 and later write `Reapply "<subject>"` and older git `Revert "Revert "<subject>""`). Title mode (`COMMIT_TITLE`) has no exemption.
- Examples accepted: `feat(#12): add health endpoint`, `FIX(#7): handle empty page`. Rejected: `feat: add x`, `feat(api): add x`, `feature(#12): add x`, `feat(#12):missing space`, `chore(release): v1.2.0 [skip ci]`.

Bot subjects and where they are exempt: `chore(release): vX.Y.Z [skip ci]` is created by the release job on a runner, where no hook is installed, and never passes through a pull request; Dependabot subjects (`build(deps): …`, `ci(deps): …`, `chore(deps): …`) and the template-sync title `chore(template): merge template changes` are exempt from the title check through the job condition of DD-15 (authored by Dependabot, or labelled `template_sync`). All of them are valid conventional commits, so cocogitto reads them and none triggers a release.

Pull-request title check: job `title` of `change-control.yml` (gate G28). Repositories are expected to squash-merge, so the checked title becomes the commit subject on `main`; `docs/repository-settings.md` records that setting. `CONTRIBUTING.md` gives the rule, the examples and the version rules, and tells contributors to run `make install-hooks`.

## Load Testing

### Layout of `tests/Load`

| File | Origin | Change |
| --- | --- | --- |
| `config.json.dist` | PHP structure | budgets, one endpoint, unused timeout keys removed (content below); its four budget values are protected (core `json_values`), the rest changes freely |
| `scripts/health.js` | PHP | unchanged: `GET /api/health`, check `is status 204`. The only scenario the kit ships (FR78; the PHP kit ships the same one) |
| `utils/utils.js` | PHP | one line: the rethrown configuration error passes `{ cause }` (ESLint 10 `preserve-caught-error`) |
| `utils/scenarioUtils.js` | PHP | `setupTimeout` and `teardownTimeout` become the constant `'60s'`; `batchPerHost` (key that never existed) and `insecureSkipTLSVerify` (plain HTTP) removed; the three start times of the PHP kit (each set only by the profile before it, so a skipped middle profile started the next one at 0 s) are one cursor `nextStartTime`: each selected profile starts at the cursor, which then advances by the profile's total duration (smoke `duration`, average and stress `rise + plateau + fall`, spike `rise + fall`) plus `delayBetweenScenarios` |
| `utils/scenariosBuilder.js` | PHP | `addSmokeScenario(smokeConfig, startTime = 0)` sets `startTime` like the other profiles; otherwise unchanged |
| `utils/thresholdsBuilder.js` | PHP | unchanged (`http_req_duration{test_type:<profile>}: p(99)<N`, `checks{scenario:<profile>}: rate>0.99`, `Invalid threshold value`) |
| `utils/LoadTestConfigManager.js`, `Dockerfile`, `.eslintrc.js` | PHP | not carried: dead code, custom xk6 build, ESLint 8 configuration |
| `get-load-test-scenarios.sh` | PHP | resolves the directory from its own location; sorted output |
| `execute-load-test.sh` | PHP | `set -euo pipefail`; no `eval`; exits 2 with `Usage: execute-load-test.sh <scenario> [runSmoke] [runAverage] [runStress] [runSpike] [htmlPrefix]` when the scenario is empty or unknown; K6 image from `K6_IMAGE` (read from `make/tool-versions.mk` when unset); dashboard through `K6_WEB_DASHBOARD*` variables |
| `run-smoke-load-tests.sh`, `run-average-load-tests.sh`, `run-stress-load-tests.sh`, `run-spike-load-tests.sh`, `run-load-tests.sh` | PHP | same arguments and prefixes (`smoke-`, `average-`, `stress-`, `spike-`, `all-`); all five fail with `Error: No load test scenarios found.` on an empty list; debug output removed |
| `aws-execute-load-tests.sh`, `cleanup.sh`, `config.sh`, `user-data.sh` | PHP | fixed as described under "AWS runner" |
| `trust-policy.json`, `s3-bucket-policy.json`, `s3-access-policy.json` | PHP | content unchanged (keys, values, the placeholders `$ACCOUNT_ID`, `$ROLE_NAME` and `$BUCKET_NAME`); `trust-policy.json` byte-identical, the two S3 policies in Prettier form, which G24 checks; the access policy is now rendered before use |
| `package.json`, `pnpm-workspace.yaml`, `pnpm-lock.yaml`, `eslint.config.js` | replaced | minimal lint tooling (below) |
| `results/.gitignore` | PHP | unchanged (`*`, `!.gitignore`) |
| `.gitignore` | new | `config.json`, `aws-run.env`, `node_modules/` |

Adding `scripts/<scenario>.js` plus an `endpoints.<scenario>` entry adds the scenario to every runner (FR79); the skill `load-testing` shows the steps with the example resource as its illustration. The image is the stock `grafana/k6:2.3.0` pinned by digest; nothing is built.

`execute-load-test.sh` runs (verified invocation of the research, with the image variable):

```sh
docker run --rm --network host --user "$(id -u):$(id -g)" \
  -v "${LOAD_DIR}:/loadTests" -e API_HOST -e API_PORT \
  -e K6_WEB_DASHBOARD=true -e K6_WEB_DASHBOARD_PORT=-1 -e K6_WEB_DASHBOARD_PERIOD=1s \
  -e K6_WEB_DASHBOARD_EXPORT="/loadTests/results/${html_prefix}${scenario}.html" \
  "${K6_IMAGE}" run --summary-trend-stats='avg,min,med,max,p(95),p(99)' "/loadTests/scripts/${scenario}.js" \
  -e run_smoke="${run_smoke}" -e run_average="${run_average}" -e run_stress="${run_stress}" -e run_spike="${run_spike}"
```

### `config.json.dist`

`threshold` is the p99 budget in milliseconds; shapes are the PHP shapes (NFR1). A local `config.json` (git-ignored) overrides the file; `API_HOST` and `API_PORT` override the target. The file is shown in Prettier's format, which G24 checks.

```json
{
  "apiHost": "localhost",
  "apiPort": "8081",
  "delayBetweenScenarios": 60,
  "endpoints": {
    "health": {
      "smoke": { "threshold": 50, "rps": 10, "vus": 5, "duration": 10 },
      "average": {
        "threshold": 200,
        "rps": 25,
        "vus": 25,
        "duration": { "rise": 3, "plateau": 10, "fall": 3 }
      },
      "stress": {
        "threshold": 1000,
        "rps": 150,
        "vus": 150,
        "duration": { "rise": 3, "plateau": 10, "fall": 3 }
      },
      "spike": { "threshold": 5000, "rps": 200, "vus": 200, "duration": { "rise": 5, "fall": 5 } }
    }
  }
}
```

| Budget | PHP | Rust | Justification |
| --- | --- | --- | --- |
| `health` smoke / average / stress / spike | 9000 / 200 / 1000 / 5000 | 50 / 200 / 1000 / 5000 | smoke derived by the NFR1 rule from the hosted p99 of 7.25 ms (3 x 7.25 = 21.75, rounded up to 30, floor 50); the other three are the NFR1 ceilings (DD-25). Each request makes one `SELECT 1` and one `ListQueues` call to LocalStack; the average, stress and spike profiles have no hosted measurement |

A run fails when a profile's p99 exceeds its budget or its check rate is not above 99%. The smoke profile of the one scenario takes about 15 seconds after the environment is up.

### Results

Each run writes one self-contained HTML dashboard to `tests/Load/results/<prefix><scenario>.html` (prefix empty for `execute-load-tests-script`) with the invoking user's ownership, and prints K6's end-of-run summary. CI uploads the directory as artifact `load-test-results`. `docs/load-testing.md` holds a table of measured p99 values per profile, the date of each measurement and the budget derived from it (NFR1); the first entry is made from the first smoke run on a hosted runner.

### AWS runner

`make aws-load-tests [region=… ami=… instance_type=… instance_tag=… role=… branch=… repo_url=… security_group=… load_target=…]` and `make aws-load-tests-cleanup`. The host needs Docker and git only (NFR37): the AWS CLI runs from the pinned image `AWS_CLI_IMAGE`, the run id comes from `/dev/urandom` and the policy is rendered with `sed`, so neither `aws`, `uuidgen` nor `envsubst` has to be installed. Flow of `aws-execute-load-tests.sh`, with the PHP defects it fixes:

1. `config.sh` reads the `.env` keys it needs with `scripts/with-env.sh` semantics (no `export $(… | xargs)`), checks that `docker` and `git` exist, and parses `-r -a -t -i -o -b -u -s -l` (DM:D-22). It defines the shell function `AWS_CLI`, which runs `docker run --rm --network host --user "$(id -u):$(id -g)" -e HOME=/tmp -e AWS_ACCESS_KEY_ID -e AWS_SECRET_ACCESS_KEY -e AWS_SESSION_TOKEN -e AWS_REGION -v "<temporary directory>:/work" -w /work "$AWS_CLI_IMAGE"` with its arguments appended (in local mode `--endpoint-url http://localhost:$LOCALSTACK_PORT` comes right after the image); callers write `AWS_CLI ec2 describe-vpcs …`, and no command line is kept in a string variable, which would have to be expanded unquoted; without local mode it stops with a message that names the credential variables when they are unset. Defaults: region `us-east-1`; instance type `c6i.xlarge` (4 vCPU, 8 GiB: a cold release build of the image takes about 5 minutes, which `t2.micro` cannot do); AMI resolved from the SSM public parameter `/aws/service/canonical/ubuntu/server/24.04/stable/current/amd64/hvm/ebs-gp3/ami-id` of the chosen region (fixes the region-specific id); repository URL from the option `-u` (an http or https URL; credentials are removed and `http` becomes `https`, as for the remote; with the option no remote is read) or else from `git remote get-url origin`, converted to HTTPS (fixes the hard-coded PHP URL); an origin that is not an HTTPS, SSH or scp-like URL (a local path) stops the script with exit status 2 and a message that names `-u` and `repo_url=`, before any container or AWS call; the script prints `Repository URL: <url>` next to `Repository branch: <name>`; branch from `git rev-parse --abbrev-ref HEAD`; load target `smoke-load-tests`.
2. A run id (8 hexadecimal characters, `od -An -N4 -tx1 /dev/urandom | tr -d ' \n'`) suffixes the bucket, role, instance-profile, policy, security-group and tag names; all of them are written to `tests/Load/aws-run.env`, which `cleanup.sh` reads (DM:D-23: fixed names).
3. Security group, bucket, role, bucket policy (retry 53 times, 5 s apart: IAM propagation takes up to about 4.4 minutes, as the comment at the constant says), managed policy from `s3-access-policy.json` with its three placeholders replaced by `sed` (DM:D-04), instance profile; the rendered files go to the temporary directory that is mounted at `/work`, and the CLI reads them as `file://<name>`.
4. `ec2 run-instances` with the rendered `user-data.sh`, a 30 GB root volume, and shutdown behaviour `terminate`; right after the launch the script prints `Launched instance: <id>` and `You can access the S3 bucket here: <console URL>`, so that the operator can follow the run during the wait.
5. The script then waits (bounded: 45 minutes, poll every 30 s; `wait=false` skips) for the object `<run-id>/DONE` or `<run-id>/FAILED` in the bucket, downloads the results to `tests/Load/results/aws/<run-id>/`, and exits 1 on `FAILED` or timeout (DM:D-06).

`user-data.sh` on the instance: install `docker.io`, `docker-compose-v2`, `docker-buildx`, `git`, `make` from Ubuntu packages and the AWS CLI with `snap install aws-cli --classic` (compose v2 plugin, DM:D-05; no unpinned binary downloads); clone the repository and branch; run `make <load_target>`; an `EXIT` trap uploads `tests/Load/results/` and `/var/log/cloud-init-output.log`, writes `DONE` or `FAILED`, and runs `shutdown -h now`, so the instance terminates on success and on failure. `cleanup.sh` uses the same `AWS_CLI` function as provisioning (local mode included), skips resources that do not exist and removes `aws-run.env`; without `aws-run.env` it prints `Nothing to clean up: tests/Load/aws-run.env not found.` and exits 0.

With `LOCAL_MODE_ENV=true` every call gets `--endpoint-url http://localhost:$LOCALSTACK_PORT` and the placeholder credentials `test`; the AMI is the fixed placeholder `ami-0e86e20dae9224db8`, the wait is skipped because the emulator does not execute user data, and the Bats test asserts the two output markers and a clean `cleanup.sh`. Verified for this revision: the pinned image ran `sts get-caller-identity`, `s3api create-bucket`, `iam create-role` with a `file://` policy from the mounted directory, `ec2 create-security-group`, `ec2 run-instances` and `s3 cp` against LocalStack 3.4.0 published on the loopback interface (as root; with `--user` only `--version` was run).

### JavaScript lint

`tests/Load/package.json`: `"name": "load-tests"`, `"private": true`, `"type": "module"`, `"packageManager": "pnpm@12.18.0"`, no scripts, development dependencies `eslint` 10.14.0, `@eslint/js` 10.0.1, `globals` 17.14.0, `eslint-config-prettier` 10.1.8, `prettier` 3.9.10, `@types/k6` 2.3.1. pnpm is kept (scope 8.7) and runs through corepack inside the pinned Node image, so no host installation is needed. `tests/Load/pnpm-workspace.yaml` holds the settings of the pinned pnpm, which ignores the `pnpm` key of `package.json`: `overrides: { minimatch: 11.0.1 }`, because ESLint 10.14.0 (`^10.2.6`) and `@eslint/config-array` 0.23.5 (`^10.2.4`) resolve minimatch 10.2.6, inside the range `>=7.0.0 <11.0.1` of GHSA-g3hv-75wg-m249 (denial of service); the entry goes once ESLint asks for a fixed minimatch. `tests/Load/eslint.config.js` (verified by the research with ESLint 10.14.0):

```js
import js from '@eslint/js';
import prettier from 'eslint-config-prettier';
import globals from 'globals';

export default [
  { ignores: ['node_modules/', 'results/'] },
  js.configs.recommended,
  {
    files: ['**/*.js'],
    languageOptions: {
      ecmaVersion: 2023,
      sourceType: 'module',
      // k6 is not Node: only its own globals exist at run time.
      globals: {
        ...globals.es2023,
        __ENV: 'readonly',
        __VU: 'readonly',
        __ITER: 'readonly',
        open: 'readonly',
      },
    },
    rules: { 'no-console': 'error', eqeqeq: 'error' },
  },
  prettier,
];
```

Gate G24 is `make lint-load-tests`, job `lint-load-tests`: ESLint with zero findings, then `prettier --check --config /repo/.prettierrc.yml . "!results/**" "!pnpm-lock.yaml"` in `tests/Load` with the root `.prettierrc.yml`, named explicitly so that no configuration discovery takes place. The two negated patterns keep generated dashboards and the lock file out of the check (verified with Prettier 3: without them a result file written by an earlier load run fails the gate).

## CLI Tests (Bats)

Location `tests/CLI/bats/` (PHP path). Every file starts with `bats_require_minimum_version 1.8.0`, `load test_helper` and exactly one file tag (`# bats file_tags=<group>`) out of `surface`, `quality`, `tests`, `contracts`, `load` and `repository`; CI runs one job per tag, and G31 rule (d) fails for a file whose tag is not a group of the matrix. Tests assert exit status and the tool's own output, never the echoed recipe, except where the command line is the subject: the execution-mode file, the dry runs in `make_general.bats` (`make -n ci`, two stage targets with a changed stage variable, `doctor` and `bootstrap`) and the dry runs of `mutation-tests` and `mutation-tests-diff` that test the `MUTANTS_IN_PLACE` switch in `make_tests.bats`. No test depends on the example bounded context being present: plants go into `xtask` and other template-owned files, so the suite also passes in a service that has replaced `crates/company-subdomain/`.

`test_helper.bash`:

- `_common_setup`: sets `REPO_ROOT`, loads `bats-support`, `bats-assert` and `bats-file` with `bats_load_library`, changes to the root, unsets the approval variables `CONFIG_CHANGE_APPROVED`, `BREAKING_CHANGE_APPROVED` and `ALLOW_PROTECTED_CONFIG_EDIT` (only a test that sets one sees it), exports `CARGO_TERM_COLOR=never` and `NO_COLOR=1` and unsets `CLICOLOR_FORCE` and `FORCE_COLOR`, so that the output of cargo, cargo-mutants and the other host tools is colourless whatever the caller exported, calls `isolate_make_environment`, and then calls `restore_planted`, so leftovers of an interrupted run are repaired before any test.
- `isolate_make_environment`: unsets `MAKEFLAGS`, `MFLAGS`, `MAKEOVERRIDES`, `MAKELEVEL` and every caller-settable parameter or test hook that the `Makefile` and `make/*.mk` declare with a plain `NAME ?=` (read from the files), except `RUN_IN_CONTAINER`, `CI` and `IN_DEV_CONTAINER`, so a variable of the outer `make bats …` call never reaches the nested makes of the tests; the exported settings (`export NAME ?=`, such as `DOCKER`, `BUILD_IMAGE`, `APP_IMAGE`, `DEV_IMAGE`, `LOAD_TEST_PROJECT`, `COMPOSE_PROJECT_NAME`) and `DB_PORT` and `LOCALSTACK_PORT` stay. The `setup_file` of `make_bmalph.bats`, `make_execution_mode.bats` and `make_lifecycle.bats` calls it too.
- Planting: `plant_append <file> <fixture>`, `plant_replace <file> <new content file>`, `plant_new <path> <fixture>`, `plant_keep <file>` (registers a file that the test is about to let a tool rewrite, for example `Cargo.lock`), `plant_tree <dir>` (registers a directory that a tool is about to create; the restore removes it with everything below, and nothing is registered when the path already exists) and `plant_index <file>` (registers a path whose index entry the test changes with `git add`; the restore unstages it). Each copies the original (or records "new") into `target/bats-restore/` with a manifest line before anything changes. `restore_planted` replays the manifest in reverse and deletes it; every file's `teardown` calls it, and Bats runs `teardown` when a test fails or is interrupted. Planting refuses to touch a file with uncommitted changes (the test is skipped with a message), so a developer's work is never overwritten.
- Fixtures are in `tests/CLI/bats/fixtures/` with extensions that no tool picks up (`.rs.txt`, `.yml.txt`, `.sh.txt`, `.feature.txt`, `.md.txt`, `.dsl.txt`); the directory is excluded from typos, Prettier, markdownlint, yamllint and taplo. Secrets and vulnerable manifests are generated at run time and never stored. Variants of a tracked file (a manifest with one more dependency, a snapshot with one field removed, a `deny.toml` with one policy emptied) are derived from the file with `sed` in the test, so they cannot go stale and keep what a service added to a template-owned file it extends; no variant of a tracked file is stored as a fixture.
- `require_docker`, `require_command <name>` skip with a reason when a prerequisite of an optional feature (npm for bmalph, `jq` for the assistant hook) is missing; every test that runs `cargo` on the host calls `require_command cargo`. `make_value <NAME>` prints the value of a Make variable after the `Makefile` was read, and `require_host_mode <why>` skips a test in container mode (`EXEC` not empty), which does not forward the host environment the test needs. `pinned_value <NAME>` prints the value of `export <NAME> := <value>` in `make/tool-versions.mk`, and `tree_status` prints `git status --porcelain` of the repository, for comparisons before and after a run.
- `free_port` prints a free TCP port of 127.0.0.1 between 20000 and 32767 (below the ephemeral ports of Linux, 32768 to 60999, and macOS, so an outbound connection cannot take a chosen port before it is used) that no earlier call of the same test file chose (the chosen ports are kept in a file of `BATS_FILE_TMPDIR`, because callers run it in a command substitution, where a variable would be lost; Bash `/dev/tcp`, no python3); `json_query '<JavaScript expression>'` evaluates the expression over the JSON document on standard input (bound to `data`) in the pinned Node image `NODE_IMAGE`, prints a string result as it is and any other result as indented JSON, and exits like `jq -e` (0, 1 for false, null or undefined, 2 for invalid JSON or a failing expression). No Bats file calls `jq` or `python3` on the host outside a test or helper that calls `require_command` for it, so the suite needs only the prerequisites that `make doctor` checks (NFR37).

Files that start compose projects (`make_execution_mode.bats`, `make_lifecycle.bats`) name each project `bats-<kind>-<pid>-<random>` (`bats-exec-…`, `bats-life-…`, `bats-alt-…`, `bats-busy-…`, `bats-sick-…`), export the name as `PROJECT_NAME` with `COMPOSE_PROJECT_NAME` unset, draw every host port at random and keep it only when nothing listens on it on 127.0.0.1 (with `free_port` of the helper, 20000 to 32767), and remove their projects with their volumes at the end, so that parallel runs and a developer's stack cannot collide (NFR38).

The second column lists the tests of targets that are not gates and of surface behaviour. Passing runs of gate targets are not repeated here (DD-02); every gate has its negative test in the third column.

| File (tag) | Tests of non-gate targets and of the surface | Negative tests |
| --- | --- | --- |
| `make_general.bats` (surface) | `make` and `make help` print `Usage:`, `make [target] [arg="val"...]`, `Targets:`; every target of the Makefile database has a help line; the 42 PHP target names of FR62, listed literally in the test, are targets of `make help` (the first column of `make --no-print-directory help \| awk 'NR > 3 {print $1}'`), and the failure message prints every missing name (test "FR62: the 42 PHP target names are targets of make help"); `features/health_check.feature` has the SHA-256 of the PHP template's file, `0ab4c4a9555c85800eeafd6136a368ed634a1166bb4d3190f43e7bc66bdb2818` (computed with `sha256sum`, or with `shasum -a 256` where `sha256sum` is missing), and the failure message names FR55(a) and NFR30 and says that the file changes only together with the PHP template (test "FR55(a): features/health_check.feature is the PHP template file"); `.coderabbit.yaml` exists (otherwise `FR97: .coderabbit.yaml is missing`), has the line `  request_changes_workflow: true` in the mapping under the top-level key `reviews:` (otherwise a message that names `reviews.request_changes_workflow: true`) and has the SHA-256 of the PHP template's file, `f48b563190bff75997715046c487790732c2331e5206b2985a30c41641e78641`, computed like the one above (otherwise a message that names FR97 and says that the file must stay identical to the PHP template's file and changes only together with it) (test "FR97: .coderabbit.yaml is the PHP template file with the request-changes workflow"); `make doctor` passes and prints the platform line; `make ci-list` equals the verification block of `AGENTS.md`; `make -n ci` shows the seven stages in order and, in addition, runs exactly the gates of `make ci-list`, in the same order; a gate added to a stage variable is run by that stage and one removed from it is not; a failing gate stops its stage before the next gate; `scripts/doctor.sh` checks the make named by `MAKE_COMMAND`, not the one on `PATH`, and reports an old make (3.81) with `[OUTDATED] make: found 3.81, need 4.0; needed by every target; install GNU Make 4.0 or newer (on macOS: brew install make, then run gmake or put $(brew --prefix)/opt/make/libexec/gnubin first on PATH)`; the `doctor` and `bootstrap` recipes hand the running make to the doctor, and a dry run does not run it; `make ci CI_STAGES=help` prints `CI checks successfully passed!`; `make purge` removes a planted report and keeps `target/debug`; the assistant hook allows an unprotected path, allows a protected one with `ALLOW_PROTECTED_CONFIG_EDIT=1`, and allows the edit of a dependency line in the root `Cargo.toml` (`pre`, the edit, `post`); `with-env.sh` strips quotes and inline comments like the service and Compose do, reads the shared forms to the values Docker Compose reads, never changes a variable of the caller, quoted or not, and refuses the forms that dotenvy and Compose read differently, with status 2 and the line; the permission policy of the assistant denies every spelling that skips the git hooks, and every target of `make ci-list` has the entry `Bash(make <target>:*)` in `permissions.allow` of `.claude/settings.json`, so that it runs without a prompt (FR111); outer make variables do not reach the nested makes of the tests, the settings of the caller do; `restore_planted` removes a planted tree and unstages a planted index entry, and `plant_tree` leaves a directory that existed before the plant alone; no Bats file calls host `jq` or `python3` outside a test guarded by `require_command`; the scan for host `jq` and `python3` also reports a call followed by `)`, `;`, `\|`, `&`, a quote or a backquote and reports exactly the marked lines of `fixtures/host_tool_calls.bats.txt` | `scripts/doctor.sh` with a reduced `PATH` exits 1 and prints `[MISSING] cargo`; `make ci CI_STAGES='validate-configuration help'` with a planted change exits non-zero without the success line and without starting the second stage (no `Usage:`); `scripts/doctor.sh --prerequisites` with a `rustc` stand-in that fails with rustup's `toolchain '<channel>-...' is not installed` exits 1 and prints `[MISSING] toolchain <channel>: needed by every cargo target; run: rustup toolchain install <channel>`; G16: `make bats BATS_PATH=<copy of fixtures/failing_suite>` exits non-zero and prints `not ok`; the assistant hook given an edit of `clippy.toml` exits 2 with the refusal text, and after an edit of a lint level in the root `Cargo.toml` its `post` call exits 2 and the file is back in its previous state (FR111); `scripts/doctor.sh` with a `cog` stand-in that prints another version reports `[OUTDATED] cocogitto: found 6.5.0, need <pin>; needed by bats (make_release.bats); run: make install`, and with the pinned version `[OK] cocogitto <pin>`; `scripts/doctor.sh --prerequisites` with a `rustc` stand-in that prints `rustc 1.0.0 (...)` exits 1 and prints `[OUTDATED] toolchain: found 1.0.0, need <channel>; needed by every cargo target; run: rustup toolchain install <channel>` (test "doctor: an installed toolchain that differs from the pin is reported with its targets and the install command") |
| `make_execution_mode.bats` (surface) | `make -n format-check` is native; `RUN_IN_CONTAINER=1 make -n format-check` starts with `docker compose exec -T` and carries the five `-e` options; with `CI=true` or `IN_DEV_CONTAINER=1` it is native again; `RUN_IN_CONTAINER=1 make format-check`, `check-requirements`, `lint-rustdoc` (the `env` form behind exec) and `setup-test-db` succeed in the running container (the `lint-rustdoc` test is tagged `slow`: 58 to 101 s measured; the `make start` of the file's `setup_file`, about two minutes, is not a test); inside the container `scripts/with-env.sh .env .env.test -- printenv APP_ENV DATABASE_URL` prints `test` and a URL on `database:5432/app_test`; `TEST_SEED=7` reaches a command behind `EXEC`, and without it the variable is unset there; the same holds for `DATABASE_STATEMENT_TIMEOUT=30m`; `RUN_IN_CONTAINER=1 scripts/doctor.sh` with cargo removed from `PATH` exits 0 and prints the `[SKIP]` line; an orchestration target ignores the switch; `make -n build` prints `docker compose build --pull --no-cache`, and `make -n start` prints `docker compose up --detach --wait --wait-timeout 900 --build` with `BUILD_IMAGE=1` and `... --no-build` with `BUILD_IMAGE=0`; every target of `make help` is invoked by a Bats test or named in the gate table or the section 'Targets without a Bats test' of `docs/make-targets.md` (FR67(a)); with `.env` planted with `DB_PORT=15432` and `LOCALSTACK_PORT=14566`, inside the running container `scripts/with-env.sh .env .env.test -- printenv DATABASE_URL AWS_ENDPOINT_URL_SQS` prints a URL on `database:5432/app_test` and `http://localstack:4566` (FR90) | an orchestration target without a reachable engine (`DOCKER=/nonexistent make lint-docker`) exits 2 with `This target needs a reachable container engine`; `RUN_IN_CONTAINER=1 make format-check` with the environment stopped fails with Docker's "service is not running" message |
| `make_lifecycle.bats` (surface) | `make start` returns with `GET /ping` = `pong` and `GET /api/health` = 204; no mount of the `app` container lies below `/app`, and no entry at the top of the checkout belongs to another user; `make start-deps` on a running environment recreates no container; `timeout 5 make logs` exits 124 and shows `"message":"listening"`; `make new-logs` with a concurrent ping shows `request completed` and `/ping`; a second project (`PROJECT_NAME=bats-alt-<pid>-<random> DB_PORT=<free> LOCALSTACK_PORT=<free> make start-deps`) creates containers whose names start with that name and is removed with `docker compose -p <that name> down --volumes` (NFR38, FR67(f)); one test, tagged `slow`, appends a comment line to `crates/service/src/lib.rs` with `plant_append`, waits until the `app` container logs a second `"message":"listening"` entry and asserts that the container id and its image id are unchanged (no image rebuild, FR89); `make start` on a fresh private project makes the containers compile the service (122 to 258 s measured), so the start test is tagged `slow` as well, and so is every test that needs the stack it starts: the mount check, `make start-deps` on a running environment, the two log tests, the occupied-port test, `make stop` and `make down`; `--filter-tags '!slow'` therefore runs only the second-project test and the unhealthy-start test, which start nothing but their own database and LocalStack (the unhealthy-start test: its own database and stand-ins on the database image), and the order dependence of the tagged tests (the first starts the stack, the last two stop and remove it) is stated in the header comment of the file; `make stop`, `make down` succeed | `make start-deps` with `DB_PORT` set to an occupied port exits non-zero; `BUILD_IMAGE=0 make start` of a private project (`bats-sick-…`) whose `COMPOSE_FILE` adds an override that keeps every dependency satisfied (`migrate` on the database image exits 0, `localstack` is a stand-in on the database image whose health check passes) and gives `app` (the database image running `sleep 600`) a health check that always fails (`CMD false`, 1 retry, 1 s) exits non-zero with `container <project>-app-1 is unhealthy` (NFR33), so only the `--wait` of the recipe turns the state into a failure |
| `make_hooks_and_commits.bats` (surface) | `check-commit-message` accepts the examples of "Git Hooks and Commit Convention" in title and hook mode; hook mode accepts `Merge …`, `Revert "…"`, `Reapply "…"`, `fixup! …`, `squash! …` and `amend! …`, and the subjects that `git revert --no-edit` writes in a scratch repository that reads no git configuration of the caller, for a commit and then for that revert (`Revert "…"`, then `Reapply "…"` on git 2.43 and later or `Revert "Revert "…""` on older git); `install-hooks` sets `core.hooksPath` (previous value restored); the hooks run the GNU Make named by `MAKE`, else `gmake`, else `make` | G28: each rejected example exits 1 with the format line; title mode has no exemption: `Merge …` and `Reapply "…"` exit 1 with `got: <subject>`; hook mode rejects with status 1 and the format line the subjects that only resemble the ones git writes (`Merge: x`, `Mergex`, `Revert x`, `Revert: x`, `Revert:"x`, `Reapply x`, `Reapply: x`, `Reapply:"x`, `fixup!x`, `fixup: x`, `squash!x`, `squash: x`, `amend!x`, `amend: x`), so a test fails when an exemption prefix is widened; neither variable set exits 2 |
| `make_bmalph.bats` (surface) | `make help` lists the five targets; `bmalph-install` in an isolated `HOME` prints `BMALPH installed:` with the pinned version and the dry-run verification line; `bmalph-init BMALPH_DRY_RUN=true` and `bmalph-setup BMALPH_DRY_RUN=true` leave `git status` unchanged | an unsupported `BMALPH_PLATFORM` exits 1 |
| `make_quality_gates.bats` (quality) | `pre-commit` passes; `validate-configuration base_ref=HEAD` passes with a planted change and `CONFIG_CHANGE_APPROVED=true` | G1: unformatted lines appended to `xtask/src/main.rs` (`Diff in`); G2: an `unwrap` (`unwrap_used`), a `SystemTime::now()` call (`disallowed_methods`, NFR28), an `#[expect(clippy::unwrap_used)]` without a reason (`` `expect` attribute without specifying a reason ``, `allow_attributes_without_reason`) and a used, documented item whose comment links to an item that does not exist (rustdoc: ``unresolved link to `NoSuchPlantedItem` ``, `rustdoc::broken-intra-doc-links`); an unused `use std::collections::HashMap;` in a `#[test]` function appended to `xtask/src/cli/tests.rs`, the `cfg(test)` module that only the pass over all targets compiles (`unused import`, the location `xtask/src/cli/tests.rs` and cargo's ``could not compile `xtask` (bin "xtask" test)``, so the production pass passed and the test-code pass failed); a used, documented function holding an `unsafe` block under `#[expect(unsafe_code, reason = ..)]` appended to `xtask/src/main.rs` (``usage of an `unsafe` block`` and `incompatible with previous forbid`: the expectation would lift a `deny` or `warn` level, so the test also fails when the workspace level `unsafe_code = "forbid"` is weakened); G3, one per limit: a six-parameter plain function (`too many arguments (6/5)`), a function of 57 lines (`this function has too many lines (57/50)`), a block nested six levels deep (`this block is too nested`) and ten sequential `if`s (`the function has a cognitive complexity of (11/8)`), each asserting clippy's message because the echoed recipe names every lint of G3; G4: `ARCH_ARGS` pointing at the checker's fixture workspace (exit 1, `arch::layer`); G5: workspace version changed without lock update (cargo's `cannot update the lock file <path>/Cargo.lock because --locked was passed`; the echoed recipe also contains `--locked`, so `--locked` alone is not asserted); G6: `DATABASE_URL=mysql://x` (exit 2, names `DATABASE_URL`); G7, one per policy: `deny.toml` replaced by a variant derived from the current file with `sed` at test time, once with an empty licence allow-list (`allow = []` in `[licenses]`) and once with `allow-registry = []`; each test checks that the variant differs from `deny.toml` and that the targeted key is now empty, so the dated exceptions a service adds to `deny.toml` stay in the variant and cannot fail the test, and `DENY_ARGS="--manifest-path <generated project>"` whose lock file, generated at test time, holds a crate version with a published advisory; G8: `humantime.workspace = true` added under `[dependencies]` of `xtask/Cargo.toml`, with `Cargo.lock` refreshed by `cargo update --workspace --offline` (the gate names the dependency and leaves the lock file as it found it); G29: the `RawValue` function of DD-07 appended to `xtask/src/main.rs` (cargo-hack fails on `xtask`); G25: a comment appended to `clippy.toml` with `base_ref=HEAD` (`Modification of locked configuration file is not allowed: clippy.toml`, `[FAIL]`, `Approve with the label approved-config-change`), and `base_ref=no-such-ref` exits 2 |
| `make_scaffolding.bats` (quality) | `make new-context group=bats-group context=bats-context` creates three crates; `validate-manifest`, `architecture` and `check-crate-isolation` then pass with no protected file edited, and `unused-dependencies` gives the same status and output as on the tree before the scaffolding (it passes when it passed there; the new crates add no finding); teardown removes the crates and restores `Cargo.toml` and `Cargo.lock` | missing parameters exit 2; an existing context name exits 1 |
| `make_database.bats` (quality) | on a throw-away database of the development server named by `DATABASE_URL` (these tests are skipped with a reason in container mode, which forwards no `DATABASE_URL`): `make migrate` twice (the second run changes nothing); `migrate-rollback` then `migrate`; both leave the development database alone | `create-migration` without `name` exits 2 with usage |
| `make_tests.bats` (tests) | `setup-test-db` creates `app_test` and leaves the development database untouched; it refuses a database whose name does not end in `_test`, with status 2 and before it runs `sqlx`, and passes the name guard for a URL that ends in `_test`; the test targets take `DATABASE_URL` from `.env.test` even when the caller exports another one, and keep every other caller value; a marker table of a throw-away database survives `make setup-test-db` with that database's URL exported | G11: a failing test appended to `xtask/src/cli/tests.rs` (`unit-tests`), and a failing documentation example `planted_failing_doc_test` appended to `tests/support/src/lib.rs` (`doc-tests`, `test result: FAILED`); G12: a failing test appended to `xtask/tests/integration/main.rs` (`integration-tests`); G11 and G12 through the target that `make ci` and the hosted job `tests` run: both failing tests appended at once, `make tests-with-coverage` exits non-zero and names `planted_failing_unit_test` and `planted_failing_integration_test` (the nextest profile `ci` has `fail-fast = false`); the planted tests run to their failing assertion, so the coverage floor still holds and a run line whose status the recipe ignores fails this test; a failing randomised test appended to `crates/internal/health-check/infrastructure/tests/integration/main.rs` (a template-owned, synchronised test binary that depends on `test-support`) prints `TEST_SEED=7` under `TEST_SEED=7 make integration-tests` (FR51, NFR29): the fixture binds `test_support::seed::TestRng::from_env()` as its first statement, names nothing of the example context and fails with `planted by the Bats negative test (seed <seed>)`; G13: a feature with an undefined step planted in `features/` (`Step doesn't match any function`), and the same step in a scenario tagged `@allow.skipped`; G14: the fixture function `planted` appended to `xtask/src/cli.rs`, `make tests-with-coverage` exits non-zero and `target/coverage/lcov.info` exists; G15: the same function, `CI=true make mutation-tests-diff base_ref=HEAD MUTANTS_IN_PLACE=<the caller's value>` exits 2 and prints a `MISSED` line with `replace planted -> bool with true`, the planted file and `git status` are unchanged afterwards, and the test prints the number of third-party crates the cargo-mutants builds compiled as a TAP comment; `base_ref=no-such-ref` exits 2 with the base-ref message; a dry run of `mutation-tests` and `mutation-tests-diff` adds `--in-place` exactly when `MUTANTS_IN_PLACE` is `1`, from the environment or the command line, and no other value of it (`0`, `true`, `1 1`, `1 --jobs=4`, `--in-place`, ...) and no `MUTANTS_ARGS` changes the command; a `1` padded with blanks (`' 1 '`, `'1 '`, `' 1'`, a tab before or after it, a line feed after it, from the environment; `' 1 '`, `'1 '` and `1` followed by a tab from the command line, where make drops leading blanks itself) adds no `--in-place` (test "G15: a MUTANTS_IN_PLACE of 1 padded with blanks adds no --in-place"); the diff file is the same under a hostile git configuration (`diff.mnemonicPrefix`, `color.ui=always`); G30: one file of `.sqlx/` removed, and a file of `.sqlx/` that no query generates (`potentially unused queries found in .sqlx` and the message of `scripts/check-sqlx-metadata.sh`); `scripts/check-sqlx-metadata.sh` called directly: without an argument it exits 2 with its usage line, a command that exits 7 keeps status 7, a passing command that prints the warning gives status 1 and the line that names the cause, and a passing command without it gives 0 |
| `make_contracts.bats` (contracts) | both `generate-*` targets leave `git status` empty; `openapi-diff` and `graphql-diff` against a commit without snapshots (an empty tree made with `git commit-tree`) pass with the first-version notice; with `SCHEMATHESIS_URL` set to a closed port no container is started and Schemathesis reports the failed connection | G17, one per snapshot: a changed OpenAPI snapshot, then a changed GraphQL snapshot, staged in the index; the target regenerates and reports the file; teardown unstages; G18: an operation description removed; G19: a required response property removed (`response-required-property-removed`, and the label line), passes with `BREAKING_CHANGE_APPROVED=true`, `base_ref=no-such-ref` exits 2; G20: a field removed (`breaking change`), passes with `BREAKING_CHANGE_APPROVED=true`, `base_ref=no-such-ref` fails and the script exits 2; `openapi-diff` runs the image named by `OPENAPI_DIFF_IMAGE` (a stand-in `DOCKER` records the run command); G21: an undocumented path added to the working snapshot; the managed environment is started and removed by the target, and no container of the isolated project remains |
| `make_load_tests.bats` (load) | `setup-load-test-db` takes the role and the database from the container, not from `DB_USER` and `DB_NAME` of the shell, and refuses the development project without calling docker; `execute-load-tests-script scenario=health runAverage=false runStress=false runSpike=false` writes `health.html` (with a planted smoke budget of 60000 ms: its subject is the report and the removal, and the shipped 50 ms budget stays enforced by `smoke-load-tests` and the hosted job), removes the isolated project, leaves the development containers unchanged and prints neither the failure line of `scripts/with-load-test-stack.sh` nor the logs of the project; `aws-load-tests LOCAL_MODE_ENV=true` with an explicit `repo_url` prints `Repository URL: <that url>`, `Launched instance` and `You can access the S3 bucket here`, and without `repo_url` a clone whose origin is a local path is refused with exit status 2, and `aws-load-tests-cleanup LOCAL_MODE_ENV=true` succeeds; after these runs `lint-prettier`, `lint-yaml` and `lint-toml` still pass (generated results are ignored); the five profile runners pass their switches and dashboard prefixes (`smoke-`, `average-`, `stress-`, `spike-`, `all-`) to the K6 run (a stand-in `docker` records the calls); every `snap wait` of `tests/Load/user-data.sh` is bounded by `timeout`; with a planted `tests/Load/config.json` of the shipped durations and pause (smoke 10 s, average and stress 3 + 10 + 3 s, spike 5 + 5 s, 60 s), `k6 inspect` of the health script in the pinned image with the mount of `execute-load-test.sh` gives the start times smoke 0 s, stress 70 s and spike 146 s without the average profile (FR81), and 0 s, 70 s, 146 s and 222 s for all four profiles (FR78); K6 prints them as `0s`, `1m10s`, `2m26s` and `3m42s` | G22: a local `config.json` with budget `0.001` fails the thresholds of `smoke-load-tests`, `smoke-health.html` is written, and the logs of the isolated project (the `listening` entry of `app`) are printed before it is removed (the test pins `LOG_LEVEL=info`, because the isolated stack takes the level from the shell); a planted scenario whose check expects 200 (`fixtures/failing_check_scenario.js.txt`), given a smoke budget of 60000 ms that no machine crosses, crosses `checks{scenario:smoke}` and no latency threshold (tag `slow`); with budget `-1`, `tests/Load/execute-load-test.sh health` prints `Invalid threshold value` before any request; `execute-load-tests-script` without `scenario` exits 2 with usage and starts nothing; G24: a planted script with `console.log` and `==` (ESLint), and a planted script that ESLint accepts but that is not formatted (`fixtures/unformatted_load_script.js.txt`: Prettier's `Code style issues found` with the file name, and no ESLint finding) |
| `make_security.bats` (load) | the production image runs as `65532`, has no `/bin/sh`, holds no shell, package manager, compiler or test file (its `docker export` listing, read by the helper `image_content_findings`, shows no file or link named `sh`, `ash`, `bash`, `dash`, `ksh`, `mksh`, `zsh`, `busybox` or `toybox` in any directory (a directory of such a name, which `tar -t` writes with a trailing slash, is no shell and is not reported), no `apt`, `apt-get`, `dpkg`, `apk`, `cc`, `gcc`, `c++`, `rustc`, `cargo` or `rustup` in a directory named `bin` or `sbin`, nothing under `app/` other than `app/service`, no `.rs` file, no path through a `tests` or `features` directory and no `Makefile`; NFR12), is at most 100 MiB and becomes healthy through its own healthcheck with a read-only root and no capabilities; the production reference renders with limits, read-only root, dropped capabilities, `no-new-privileges` (NFR12) and the restart policies (`app`: `unless-stopped`, stop grace period `15s`; `migrate`: `no`; FR91); the database readiness probe renders as `pg_isready -U "$POSTGRES_USER" -d "$POSTGRES_DB"` with `POSTGRES_USER` and `POSTGRES_DB` equal to `DB_USER` and `DB_NAME`, an interval of at most 10 s and 10 retries (NFR33) | the planted listing `fixtures/image_listing_planted.txt` makes `image_content_findings` report exactly its eleven forbidden entries (`app/src/main.rs`, `busybox/busybox`, `busybox/sh`, `srv/features/api_docs.feature`, `srv/tests/`, `usr/bin/bash`, `usr/bin/dpkg`, `usr/local/bin/dash`, `usr/local/cargo/bin/rustc`, `usr/src/service/Makefile`, `usr/src/service/lib.rs`; the four shells as `shell:`) and none of its allowed ones; a listing with each of the nine shell names once, at the top level, in a directory named `bin` or `sbin`, or in another directory (`bin/sh`, `./bin/ash`, `usr/bin/bash`, `usr/local/bin/dash`, `opt/shells/ksh`, `usr/lib/mksh`, `usr/bin/zsh`, `usr/local/sbin/busybox`, `toybox`), makes it report exactly those nine as `shell:` and none of its near misses (the directories `busybox/` and `usr/share/zsh/`, and `usr/share/base-files/dot.bashrc`, `usr/bin/bashbug`, `usr/bin/sha256sum`, `etc/shells`); G9: a key pair generated at test time in an untracked file (`leaks found`); a key generated at test time, committed and removed again in a throw-away repository: the history scan exits 1 with `leaks found` while the scan of the working files reports nothing; the copy of the working files skips a listed file that is gone and does not use `tar`; a finding of the scanner exits 1, and a temporary directory that cannot be created exits 2; G10: `IMAGE_TO_SCAN=$VULNERABLE_FIXTURE_IMAGE` exits 1 with a CRITICAL finding; the production reference with `--env-file /dev/null` fails and names each required variable, `DATABASE_URL` and `AWS_REGION`, with one of them unset at a time (a Compose version may stop at the first missing variable, and its order is not fixed) |
| `make_repository.bats` (repository) | none | G23, one per linter: a workflow with a syntax error (actionlint), a workflow with `uses: actions/checkout@v7` (zizmor), a YAML file with a duplicate key, a Markdown file with a skipped heading level, a script with an unquoted expansion, `DOCKERFILES` pointing at a fixture with `latest`, an unformatted TOML file, a misspelt word, a misspelt file name, a misspelt workflow name in `.github/workflows/` and a misspelt file name under `.github/`, unformatted JSON; and the composition of `lint-repo`, which has no recipe of its own: `make -n lint-repo` succeeds and prints the commands of all nine tools of its eight prerequisites (actionlint, zizmor, yamllint, markdownlint-cli2, shellcheck, hadolint, `taplo fmt --check`, typos, `prettier --check`), so a prerequisite dropped from `lint-repo` fails a test (the dry run starts no container); G26: `make no-such-target` added to `README.md`; G27: a line that does not parse appended to `workspace.dsl`; G31: `rust-toolchain.toml` set to another version; template sync, with the sync-ignored set computed as the sync action computes it (every entry of `.templatesyncignore`, comments and blank lines dropped, is a git pathspec from the root: `git ls-files --cached --others --exclude-standard -- <entries>`): no tracked or new Rust file that names `example_api_resource` (singular, which also matches the plural table name), `ExampleApiResource`, `some_module` or `Adapters {` is outside that set, and a planted `crates/shared/infrastructure/tests/integration/planted_example_table.rs` is reported, and so is a planted `crates/shared/infrastructure/tests/integration/planted_example_fixture.rs` that names only the singular identifier (test "a planted synchronised test that names only the singular example identifier is reported"); no synchronised `tests/CLI/bats/*.bats`, `*.bash` or file under `tests/CLI/bats/fixtures/` names `company-subdomain`, `some-module`, `some_module`, `example_api_resource` or `ExampleApiResource` (the names are assembled at run time, so the file passes its own check), and no command line of one (continuation lines joined, comments skipped) runs `diff` or `cmp` on a sync-ignored path and a fixture; a planted Bats file and a planted fixture that name the example context, and a planted Bats file with `sed ... deny.toml \| diff - "${FIXTURES}/deny.toml.txt"` and a continued `cmp -s` of `.env.test` with a fixture, are reported, and a planted Bats file that a copy of the list ignores is not; a scratch repository with the list `Cargo.toml`, `README.md`, `crates/*.rs` and `tests/support/` (a comment line, a trailing comment and a blank line included) gives the same set as the action's own command (`git reset --` with every entry after `git add -A`) and a different set from the `.gitignore` reading; every entry is root-relative (no leading `/`, no `..` component), and a copy with `/Cargo.toml` and `../Cargo.toml` is reported and makes `git ls-files` fail with `is outside repository`; the list keeps no template-owned file out of the sync through an entry that does not name it (nothing below `crates/shared/`, `xtask/`, `tests/CLI/` or `.github/workflows/`, no `docs/README.md` and no manifest `crates/internal/health-check/<layer>/Cargo.toml`, the template-owned manifests that those directories do not hold; the manifests of the example context and of every context a service adds are service-owned, FR100(d), and the template's list ignores `tests/support/` and `tests/e2e/` whole), a copy with `*Cargo.toml` appended is reported, with `crates/internal/health-check/infrastructure/Cargo.toml` among its lines and without the root manifest, `crates/service/Cargo.toml` and the manifests of the example context, a copy with `crates/shared/` appended is reported, a copy that names `crates/shared/infrastructure/Cargo.toml` and `crates/shared/infrastructure/src/cache.rs` exactly is not, and a copy with `crates/billing/` appended reports nothing while an untracked `crates/billing/invoices/domain/Cargo.toml` is planted (the directory of a context that a service adds); FR100(b): the `with:` mapping of the step whose `uses:` starts with `AndreasAugustin/actions-template-sync@`, read by the awk helper `template_sync_input_findings` (mawk- and BusyBox-safe; a comment, a key outside that mapping and the lines of a block value do not count), passes `pr_labels: template_sync`, `is_force_push_pr: true`, `is_pr_cleanup: true` and `is_keep_branch_on_pr_cleanup: true` in both sync workflows. A sync workflow that is missing passes only when its exact path is an entry of `.templatesyncignore`, the way the sync guides keep one flavour; a missing workflow that the list does not name fails (test "FR100(b): both sync workflows update the open sync pull request in place and close older ones"). Copies of a sync workflow (the App workflow, or the PAT workflow when the App workflow is absent) without `is_pr_cleanup`, with `is_pr_cleanup: false` and without `is_force_push_pr` report exactly `missing: is_pr_cleanup: true`, `missing: is_pr_cleanup: true` and `missing: is_force_push_pr: true` (test "FR100(b): a sync workflow without is_pr_cleanup, with it false, or without force push is reported"). Copies without `is_keep_branch_on_pr_cleanup` and with `is_keep_branch_on_pr_cleanup: false` each report exactly `missing: is_keep_branch_on_pr_cleanup: true` (test "FR100(b): a sync workflow without is_keep_branch_on_pr_cleanup, or with it false, is reported"). A workflow whose sync step starts with `uses:`, passes the other three inputs and has `is_pr_cleanup: true` only in the `with:` of another step, in its own `env:` and inside a block value of its own `with:` reports exactly `missing: is_pr_cleanup: true`, and the same workflow without the sync step reports exactly `missing: actions-template-sync step` (test "FR100(b): a sync input outside the with mapping of the sync step, or a workflow without the sync step, is reported"); FR95: the stage of the `Dockerfile` that `.devcontainer/devcontainer.json` builds, with the stages it is built from, does not leave the directory of its `HOME` world-writable with the sticky bit; planted Dockerfiles with `chmod 1777` on it in that stage, `chmod 3777` in its parent stage or a symbolic mode on it are reported, while `chmod 0777`, a later `chmod 777` after `1777`, `1775` or a mode set in another stage are not, and a missing stage exits 2 |
| `check_invariants.bats` (repository) | the script passes on a small valid tree | one test per rule (a) to (h), and per kind within a rule, on a tree generated in the test's temporary directory and passed with `--root`; each asserts the rule letter in the output. Among them: a job-level `permissions` block without `contents: read`, a top-level `permissions` block that grants write access or `write-all`, a job-level write scope in a pull-request workflow or outside the release and sync jobs (also under a quoted key, in an inline mapping, on the line after `permissions:`, in a flow mapping broken over several lines and in a `permissions` block at the end of the workflow), a quoted `jobs`, job, timeout, `permissions` or `uses` key read as the plain key, a quoted pull-request trigger, a job-level `write-all`, a scope value of a job that is not `read`, `write` or `none` (a tag, a folded block scalar, an anchor, an alias, no value, an entry without a scope), a `permissions` value of a job or of the workflow behind an alias or a tag, a job timeout above 45 minutes, above 75 for the clean-clone job or not an integer literal, a job written as a one-line flow mapping, as a flow mapping on the line after its key with the timeout on a line of its own at four spaces (also with the opening brace deeper, after a blank line), or in a flow mapping on the line of `jobs:` or broken over several lines below it, an anchored job and its alias, a tagged job, a merge key in a job, a job key at another indentation than two spaces and a job whose keys stand deeper than four spaces (each reported by its line, a job before an anchored job still checked in full), while `security-events: write` in the code-scanning jobs, `read` and `none` (quoted or not), `permissions: read-all`, timeouts of exactly 45 and 75, a comment on `jobs:`, a blank line before the first job, and a sequence entry and the closing brace of a key's flow mapping at the indentation of the keys of a job pass (a); a development-container feature referenced by tag, and a `# syntax=` line of the `Dockerfile` whose frontend has no digest, no version tag or the tag `latest` (b); a download without a checksum in a script outside `scripts/`, a remote host behind a loopback user name, a download through a loopback proxy, a probe and a download on one line, a download in the `Dockerfile` whose images carry digests, a download in `make/tool-versions.mk` beside its SHA-256 pins, an `ADD` of a URL without a valid `--checksum=`, a second unverified download in a script that verifies the first, and a download excused only by a comment, a message or a lookup, while a request to the loopback interface alone, one verified download per call of a checksum function and one download beside a checksum function the file never calls pass (b); a Bats file without a group tag, a job name missing from the settings document, a gate target that only a workflow without a `pull_request` trigger runs, and a target outside `make ci-list` run through `npx ... @devcontainers/cli@<version> exec`, through `docker compose exec` (also with an option argument such as `--user dev`), through `exec env NAME=value`, after another `make` command on the same line, after an escaped quote outside quotes (`it\'s`), after a `#` inside a word (`${#GROUP}`), after `if`, `elif`, `else`, `while`, `until`, `!`, `{`, `time`, `then` or `do` at the start of a command, or in a `run:` value written as a double-quoted or single-quoted YAML scalar (also with escapes and a trailing comment), while `format-check` and `start-deps` run through the dev-container wrapper, a quoted message or variable value that contains `exec make` or `make` (also with a word after the target), `make` in a comment of a run line, `make` as a word of an `echo` or of an assignment, `make` after an `exec` word with two operands or after a word that ends in `exec` (`my-exec`), `make` after a word that ends in `then` or `do` or after a keyword that is an argument (`echo todo make x`, `echo then make x`, `echo please do make x`, `echo if make x`), and a quoted `run:` scalar whose shell text quotes `make` pass (d); `#[allow`, `#![expect(` in a crate root (also in `src/bin/<name>.rs`, `tests/<name>.rs`, `examples/<name>.rs` and `benches/<name>.rs`), a suppression that names `clippy::pedantic` or one of `deprecated_safe`, `keyword_idents`, `let_underscore`, `refining_impl_trait`, `rust_2018_compatibility`, `rust_2021_compatibility`, `rust_2024_compatibility`, `unknown_or_malformed_diagnostic_attributes` and `rustdoc::all`, `#![cfg_attr(test, expect(` in a crate root and a `cfg_attr` suppression that names a lint group, while `#![expect(` in a module file below `tests/<dir>/`, `src/<dir>/`, `src/bin/<dir>/` or a module directory `src/<dir>/tests/` passes (g); for the inline directives of the linters: a shellcheck directive without a reason (in a `.sh`, `.bash`, `.bats` and `.githooks/` file, after a shebang, with an empty `#`, as a trailing comment, after a here-document `<<` and `<<-`, in a `run:` block of a workflow, there after a here-document, in a run block with an indentation indicator and in a run block after a tag and an anchor), of `all` (any letter case) or of two rules (a list, two `disable=` keys, a range), a hadolint ignore without a reason, a hadolint ignore of `all` or of two rules (a list, two words), a yamllint directive that names no single rule (`disable-file` on the first line, a bare `disable`, two `rule:` entries, a name without `rule:`) and one of one rule without the comment line above (also a trailing `disable-line`), a zizmor ignore without a reason or of two rules, a markdownlint directive that names no rule, a tag or two rules (also upper case, `-disable-file`, a bare `-disable-line` inside a line and `configure-file` with `default`) and one of one rule without the comment line above (also inside a code span or a fenced block, and a `configure-file` over three lines with two keys), an ESLint directive without a rule or without ` -- <reason>` (an empty reason, a list of two rules, a bare `eslint-disable-line` and `eslint-disable`, a reason split off before the rule list, below a block comment `global`, `globals` or `exported`), an ESLint configuration comment without a reason or of two rules, and in a git work tree the directives of a tracked and of a new file, while a shellcheck directive of one rule with `# <reason>` or the comment line above (also in a run block), a zizmor ignore with its reason on its line or above, an ESLint directive of one rule with ` -- <reason>` or the comment line above (also a line comment that starts with `globals`) and a configuration of one rule with a reason, `shellcheck source=`, `shell=` and `enable=`, directive words in strings, `$'...'`, here-documents (also `<<-`, and in a `run:` block with a more indented line that holds the delimiter), a multi-line single-quoted string, after arithmetic with `<<`, `${#..}` and backquotes, in a quoted, plain or block scalar (also a sequence entry after an anchor), a `github-script` block and a YAML comment of a one-line `run:`, a `zizmor` comment outside a workflow, a hadolint word in a `RUN` string, markdownlint words without `<!--` in a code span or fence, ESLint words in strings, template literals and after a regular expression literal, a line comment `eslint-disable` or `eslint <rule>: ...`, `eslint-enable` and `eslint-env`, the files below `specs/`, `_bmad*/`, `tests/CLI/bats/fixtures/`, `node_modules/` and `target/`, and the ignored files of a git work tree pass (g); a script called outside `$(EXEC)` that starts cargo (h) |
| `check_docs.bats` (repository) | the script passes on a small valid tree with help texts | one test per rule (1) to (7) on a generated tree passed with `--root` and `--service-help`; each asserts the rule number in the output |
| `check_architecture_model.bats` (repository) | `scripts/validate-architecture-model.sh` passes on `fixtures/architecture_model.dsl.txt` (two components) with the matching crate list on standard input; a dependency between two modelled crates with a relationship passes, and a development dependency needs none | variants derived from the fixture with `sed`: without the system-context view, without the container view, without the component view, with one component removed from the model, with an extra component, with a component excluded from the component view; each exits 1 and names the view kind or the component (FR120); a dependency between two modelled crates without a relationship exits 1 and names both components (`missing relationship: alpha-crate -> beta-crate`), in every form of a dependency entry (a `[dependencies.<name>]` table, a target table, a `package =` rename, a quoted key, a renamed target table, a quoted key with `.workspace = true`) |
| `ci_scripts.bats` (repository) | `scripts/ci/detect-changes.sh` in throw-away repositories: documentation only gives `code=false`, each documentation pattern also with a file that is not Markdown, near misses, a rename from `docs/` to `crates/` and one from `crates/` to `docs/`, unset, empty and unresolvable commits give `code=true`; `scripts/ci/mutation-plan.sh` in a copy with stand-ins for `cargo` and `scripts/mutation-diff.sh` (0, 1, 150, 151, 1300 mutants, `No mutants to filter`, a push); `scripts/ci/summarize-tests.sh` with generated JUnit and lcov files (all passed, no `GITHUB_STEP_SUMMARY`) | every protected file and directory of `protected-config.toml` gives `code=true`; the exit-2 cases of `mutation-plan.sh`; `summarize-tests.sh` names a failed integration case, a failed case of another suite, uncovered lines or functions, a missing JUnit file and `TESTS_OUTCOME=failure` |
| `make_release.bats` (repository) | `scripts/release.sh` and `scripts/set-version.sh` in a throw-away clone on `main` with a local bare `origin` and a stub `gh`: first release v0.1.0 from a feat or a fix, v0.1.1 after a tag with `docs` and `chore` commits before it and only the sections `### Features` and `### Bug Fixes` in `CHANGELOG.md` (the `omit_from_changelog` entries of `cog.toml`), v0.2.0 after a tag, a feat with a `BREAKING CHANGE:` footer below 1.0.0 releases v0.2.0, not v1.0.0; after `v0.1.0` a `fix` (marked **BREAKING** in `### Bug Fixes`), a `docs` (listed under `### Documentation`) and a `refactor` with the footer (after other footers, among non-releasing commits) release v0.2.0; from `v1.0.0` a `fix` with the footer releases v2.0.0; without a tag a `docs` commit with the footer releases the manifest version, and so does a footer in the oldest commit alone; with only the prerelease tag `v0.1.0-rc.1` a `feat` with the footer releases the manifest version v0.1.0, not v1.0.0 (test "with only the prerelease tag v0.1.0-rc.1 a breaking change releases the manifest version, not a major bump"); after `v0.1.0` a `docs(#2)!:` subject releases v0.2.0, and a footer followed by a paragraph of `Co-authored-by:` trailers releases v0.2.0; in the footer section `BREAKING CHANGE` inside a sentence, in lower case, without the blank after the colon or indented does not count; after `v0.1.0` a fix with `BREAKING CHANGE: y` inside a paragraph of its body releases v0.1.1 and a docs commit that quotes the footer as an indented example releases nothing; from `v1.0.0` a fix with the line in the last paragraph of its body releases v1.0.1, a docs commit with the line in its body releases nothing and a feat then v1.1.0; `--version 1.0.0`, no release from build and docs commits, an upper-case `FIX` type; set-version changes only `[workspace.package].version` and `Cargo.lock` | a bad version and an unknown option, a remote `main` that moved, a malformed version for set-version, a manifest without the version line |

`make_release.bats` needs cocogitto at the pinned `COCOGITTO_VERSION` and fails without it; it runs `cargo` on the host, so its `setup` calls `require_command cargo` and the file skips only where the host has no Rust toolchain (container mode). Both `ci_scripts.bats` and `make_release.bats` have a `teardown` that calls `restore_planted`, as every Bats file does. `make_release.bats` defines its own `isolate_git` (a function of the file, not of the helper), which points `HOME` at a directory of `BATS_FILE_TMPDIR`, sets `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_CONFIG_NOSYSTEM=1` and a fixed author and committer and unsets `GH_TOKEN` and `GITHUB_TOKEN`, so that git and cocogitto read no configuration of the caller.

The rule kinds of the Rust checkers (G4 and G25) beyond the Make-level tests above are proven by the `xtask` unit and integration tests of the core document (PD-26).

Time (estimates for a 4-vCPU runner, from the measurements listed under "Delivery Validation"; none of the groups was run). In CI the six groups run as parallel jobs with a timeout of 30 minutes each. With every cache cold, as on the bootstrap pull request: `surface` about 16 minutes (development image, first compile inside the container), `quality` about 14 (cold clippy, debug build, cargo-hack), `tests` about 15 (test build, one instrumented run, one mutation baseline of `xtask`), `contracts` about 12 and `load` about 12 (production image, four starts of the isolated environment in total), `repository` about 9 (linter images, debug build for the help texts). With restored caches each group is planned under 10 minutes. Run in sequence as stage 7 of `make ci`, after stages 1 to 6 have built everything, the suite is estimated at 18 to 22 minutes. Tests that take more than a minute carry the additional tag `slow`; `make bats BATS_ARGS="--filter-tags '!slow'"` is the quick local subset.

## Repository Lint and Hygiene

Gate G23 is `make lint-repo`, job `lint-repo`; gate G31 is `make validate-invariants`, job `invariants`. Generated snapshots (`.github/openapi-spec/`, `.github/graphql-spec/`), lock files, `.sqlx/`, `specs/`, `CHANGELOG.md`, bmalph-generated directories and the Bats fixtures are excluded from every linter below.

| Concern | Tool and version | Runs as | Configuration | Target |
| --- | --- | --- | --- | --- |
| Workflow syntax, expressions, embedded shell | actionlint 1.7.12 | image | none (defaults) | `lint-actions` |
| Workflow security (pinning, permissions, injection) | zizmor 1.32.0 | binary | `.github/zizmor.yml` | `lint-actions` |
| YAML | yamllint 1.38.0 | Python image | `.yamllint.yml` | `lint-yaml` |
| Markdown | markdownlint-cli2 0.24.0 | image | `.markdownlint-cli2.yaml` | `lint-markdown` |
| Shell, hooks, Bats | shellcheck 0.11.0 | image | none; `--severity=style` | `lint-shell` |
| Dockerfile | hadolint 2.15.1 | image | `.hadolint.yaml` | `lint-docker` |
| TOML | taplo 0.10.0 | binary | `taplo.toml` | `lint-toml` |
| Spelling of file contents, file names and workflow names | typos 1.52.0 (checks file names as well; the newest version the pinned install action lists) | binary | `_typos.toml` | `lint-spelling` |
| YAML and JSON formatting (the load-test JavaScript and JSON belong to G24) | Prettier 3.9.10 | Node image | `.prettierrc.yml` (PHP file unchanged), `.prettierignore` together with `.gitignore` | `lint-prettier` |
| Secrets | gitleaks 8.30.1 | image | `.gitleaks.toml`, `.gitleaksignore` | `secrets-scan` (G9, job `secrets-scan`) |
| Repository invariants (G31): pins, versions, gate set, repository name, licence, suppressions, execution switch | `scripts/check-invariants.sh` | shell | none | `validate-invariants` |

Configuration files:

```yaml
# .yamllint.yml
extends: default
ignore: |
  .github/openapi-spec/
  tests/Load/pnpm-lock.yaml
  tests/CLI/bats/fixtures/
  target/
  mutants.out*/
  .tools/
  _bmad/
  _bmad-output/
  .ralph/
  bmalph/state/
  **/node_modules/
rules:
  braces: { min-spaces-inside: 1, max-spaces-inside: 1 }
  comments: { min-spaces-from-content: 1 }
  document-start: disable
  indentation: { spaces: 2, indent-sequences: consistent }
  line-length: disable # Prettier owns the line width
  truthy: { check-keys: false } # `on:` in workflows
```

```yaml
# .markdownlint-cli2.yaml
config:
  default: true
  MD013: false # prose and tables are not wrapped
  MD024: { siblings_only: true }
  MD033: { allowed_elements: [a, img, p] } # README banner and badges
  MD041: false # README starts with the banner
ignores:
  - CHANGELOG.md
  - specs/**
  - target/**
  - .tools/**
  - _bmad/**
  - _bmad-output/**
  - .ralph/**
  - .claude/commands/**
  - tests/CLI/bats/fixtures/**
  - '**/node_modules/**'
```

```yaml
# .hadolint.yaml
ignored:
  - DL3008 # apt package versions are governed by the digest-pinned base image
trustedRegistries:
  - docker.io
  - gcr.io
```

```yaml
# .github/zizmor.yml
rules:
  unpinned-uses:
    config:
      policies:
        '*': hash-pin
```

```toml
# taplo.toml
include = ["**/*.toml"]
exclude = [
    "target/**",
    "mutants.out*/**",
    "**/node_modules/**",
    ".sqlx/**",
    ".tools/**",
    "_bmad/**",
    ".ralph/**",
    "xtask/tests/fixtures/**",
    "tests/CLI/bats/fixtures/**",
]

[formatting]
column_width = 100
indent_string = "    "
```

```toml
# _typos.toml
[files]
# Read the hidden directories too (.github/, .claude/, .githooks/, .devcontainer/): typos skips them by
# default. The walker then no longer skips .git (a directory, or a file in a worktree), so it is excluded.
ignore-hidden = false
extend-exclude = [
    ".git",
    ".github/openapi-spec/",
    ".github/graphql-spec/",
    ".sqlx/",
    "Cargo.lock",
    "CHANGELOG.md",
    "LICENSE",
    "specs/",
    "tests/Load/pnpm-lock.yaml",
    "tests/CLI/bats/fixtures/",
]
```

```toml
# .gitleaks.toml — the default rule set; accepted findings go to .gitleaksignore, one fingerprint per line
# with a dated comment above it.
[extend]
useDefault = true
```

`.prettierignore`: `.github/openapi-spec/`, `.github/graphql-spec/`, `.sqlx/`, `specs/`, `.tools/`, `_bmad/`, `_bmad-output/`, `.ralph/`, `tests/CLI/bats/fixtures/`, `tests/Load/` (checked by G24 with its own exclusions), `*.md` (Markdown belongs to markdownlint). `lint-prettier` passes this file and `.gitignore`, so generated and ignored files (`target/`, `mutants.out/`, `bmalph/state/`, `.claude/settings.local.json`) are not checked and a second `make ci` passes after the first (verified with Prettier 3 on a tree that holds such files). `.trivyignore` starts empty with a header comment that gives the entry format: the identifier, then a comment with the date, the reason and the review date.

The two YAML tools agree on one format. Prettier with the PHP `.prettierrc.yml` writes flow mappings with one space inside the braces, which the default `braces` rule of yamllint rejects, and it cannot wrap long scalars, which a `line-length` rule would reject; `.yamllint.yml` therefore sets `braces` to exactly one space and leaves the line width to Prettier. Every YAML file is committed in Prettier's format. Verified for this revision: `.yamllint.yml` itself, `.markdownlint-cli2.yaml`, `.hadolint.yaml`, `.github/zizmor.yml`, `.github/dependabot.yml` and the four compose files as listed in this document pass `yamllint --strict` (1.38.0) and `prettier --check` (3.8.1 locally; the pinned version is 3.9.10).

`.editorconfig`:

```ini
root = true

[*]
charset = utf-8
end_of_line = lf
insert_final_newline = true
trim_trailing_whitespace = true
indent_style = space
indent_size = 4

[*.{yml,yaml,json,js,md,tera,http}]
indent_size = 2

[Makefile]
indent_style = tab

[make/*.mk]
indent_style = tab

[*.{bats,bash,sh}]
indent_size = 2

[{features/health_check.feature,.github/graphql-spec/spec,.github/openapi-spec/spec.yaml}]
insert_final_newline = unset
trim_trailing_whitespace = unset
```

`.gitignore` (complete intent): `target/`, `**/*.rs.bk`, `*.pdb`, `mutants.out*/`, `rustc-ice-*.txt`, `.tools/`, `node_modules/`, `tests/Load/config.json`, `tests/Load/aws-run.env`, `workspace.json`, `.structurizr/`, `.env.local`, `.claude/settings.local.json`, `.ralph/`, `_bmad/`, `_bmad-output/`, `bmalph/state/`. `.env` and `.env.test` are tracked and not ignored. After `make ci` on a clean checkout `git status` is empty: every command writes under `target/`, `.tools/`, `mutants.out/` or `tests/Load/results/`.

## Architecture Model

`workspace.dsl` stays at the repository root. It has one component per workspace crate of the service (the crate table of the core document is the component list), grouped by bounded context and layer; PostgreSQL is a container outside the service binary and the message broker is an external system; nothing is modelled that the code does not contain. `xtask`, `test-support` and `e2e-tests` are tooling and tests and are not modelled. The text below was validated with `structurizr/structurizr:2026.10.26 validate` (exit 0), and a copy with one unknown relationship target failed with exit 1.

```text
workspace "Service architecture" "C4 model of the service: system context, containers and components." {
    !identifiers hierarchical
    model {
        properties {
            "structurizr.groupSeparator" "/"
        }
        apiClient = person "API client" "A person or program that calls the REST and GraphQL API."
        messageBroker = softwareSystem "Message broker" "Amazon SQS, or LocalStack in development." {
            tags "External"
        }
        system = softwareSystem "Service" "A VilnaCRM service built from the Rust service template." {
            database = container "Database" "Stores the data of every bounded context." "PostgreSQL 16" {
                tags "Database"
            }
            service = container "Service binary" "Serves REST under /api, GraphQL at /api/graphql, /api/health and /ping; also runs migrations and exports the API contracts." "Rust, axum, tokio" {
                compositionRoot = component "service" "Composition root: CLI, configuration, wiring, router, middleware, OpenAPI document, GraphQL schema." "Rust crate"
                group "Shared" {
                    group "Shared/Domain" {
                        sharedDomain = component "shared-domain" "Aggregate root, domain event, bus ports, technical ports, UUID and page value objects." "Rust crate" {
                            tags "Domain"
                        }
                    }
                    group "Shared/Application" {
                        sharedApplication = component "shared-application" "Problem document, problem-aware extractors, fallbacks, request timeout, GraphQL error helpers." "Rust crate" {
                            tags "Application"
                        }
                    }
                    group "Shared/Infrastructure" {
                        sharedInfrastructure = component "shared-infrastructure" "In-process buses, cache adapter, clock, UUID generator, connection pool, migrator, SQS client factory." "Rust crate" {
                            tags "Infrastructure"
                        }
                    }
                }
                group "Internal/HealthCheck" {
                    group "Internal/HealthCheck/Domain" {
                        healthCheckDomain = component "health-check-domain" "HealthProbe port, Dependency and HealthReport value objects." "Rust crate" {
                            tags "Domain"
                        }
                    }
                    group "Internal/HealthCheck/Application" {
                        healthCheckApplication = component "health-check-application" "HealthService and the GET /api/health handler." "Rust crate" {
                            tags "Application"
                        }
                    }
                    group "Internal/HealthCheck/Infrastructure" {
                        healthCheckInfrastructure = component "health-check-infrastructure" "PostgreSQL, cache and SQS probes." "Rust crate" {
                            tags "Infrastructure"
                        }
                    }
                }
                group "CompanySubdomain/SomeModule" {
                    group "CompanySubdomain/SomeModule/Domain" {
                        someModuleDomain = component "some-module-domain" "ExampleApiResource aggregate, value objects, created event, repository port." "Rust crate" {
                            tags "Domain"
                        }
                    }
                    group "CompanySubdomain/SomeModule/Application" {
                        someModuleApplication = component "some-module-application" "Create command, get and list queries, audit subscriber, REST handlers, GraphQL resolvers." "Rust crate" {
                            tags "Application"
                        }
                    }
                    group "CompanySubdomain/SomeModule/Infrastructure" {
                        someModuleInfrastructure = component "some-module-infrastructure" "PostgreSQL repository adapter." "Rust crate" {
                            tags "Infrastructure"
                        }
                    }
                }
                compositionRoot -> sharedApplication "Uses fallbacks and middleware"
                compositionRoot -> sharedInfrastructure "Builds buses, cache, pool and SQS client"
                compositionRoot -> healthCheckApplication "Mounts the health route"
                compositionRoot -> healthCheckInfrastructure "Registers the probes"
                compositionRoot -> someModuleApplication "Mounts routes, registers handlers and subscribers"
                compositionRoot -> someModuleInfrastructure "Registers the repository"
                compositionRoot -> sharedDomain "Wires the bus, clock and UUID ports"
                compositionRoot -> healthCheckDomain "Collects the probes behind the HealthProbe port"
                compositionRoot -> someModuleDomain "Holds the repository behind the repository port"
                sharedApplication -> sharedDomain "Uses bus errors"
                sharedInfrastructure -> sharedDomain "Implements bus, cache, clock and UUID ports"
                healthCheckDomain -> sharedDomain "Uses the shared error type"
                healthCheckApplication -> healthCheckDomain "Runs probes through the HealthProbe port"
                healthCheckApplication -> sharedApplication "Returns problem documents"
                healthCheckApplication -> sharedDomain "Formats the error chain of a failed probe"
                healthCheckInfrastructure -> healthCheckDomain "Implements HealthProbe"
                healthCheckInfrastructure -> sharedDomain "Uses the Cache port"
                someModuleDomain -> sharedDomain "Uses aggregate root, events and value objects"
                someModuleApplication -> someModuleDomain "Uses the aggregate and the repository port"
                someModuleApplication -> sharedDomain "Dispatches through the bus ports"
                someModuleApplication -> sharedApplication "Uses extractors and problem documents"
                someModuleInfrastructure -> someModuleDomain "Implements the repository port"
                someModuleInfrastructure -> sharedDomain "Uses shared value objects"
                sharedInfrastructure -> database "Opens the connection pool and applies migrations" "SQL over TCP"
                healthCheckInfrastructure -> database "Runs SELECT 1" "SQL over TCP"
                someModuleInfrastructure -> database "Reads and writes example_api_resources" "SQL over TCP"
            }
        }
        apiClient -> system.service "Calls the REST and GraphQL API" "HTTP, JSON"
        system.service.healthCheckInfrastructure -> messageBroker "Calls ListQueues" "HTTPS, AWS SDK"
    }
    views {
        systemContext system "SystemContext" "The service, its callers and the systems it depends on." {
            include *
            autolayout lr
        }
        container system "Containers" "The service binary and its database." {
            include *
            autolayout lr
        }
        component system.service "Components" "One component per workspace crate, grouped by bounded context and layer." {
            include *
            autolayout tb
        }
        styles {
            element "Person" {
                shape Person
            }
            element "Database" {
                shape Cylinder
            }
            element "External" {
                background #999999
                color #ffffff
            }
            element "Domain" {
                background #2e7d32
                color #ffffff
            }
            element "Application" {
                background #1565c0
                color #ffffff
            }
            element "Infrastructure" {
                background #6a1b9a
                color #ffffff
            }
        }
    }
}
```

| Topic | Decision |
| --- | --- |
| Views | `SystemContext` (the service, the API client, the message broker), `Containers` (service binary, database), `Components` (ten components) |
| Served locally | service `structurizr` of the development override (DD-05): `make start`, then `http://localhost:8080/workspace/1/diagrams` (`STRUCTURIZR_PORT`); the file is mounted read-only and generated files stay in the volume `structurizr_data` |
| Validated | `make validate-architecture-model` (gate G27, job `architecture-model`): parse with `validate -workspace`, export to JSON and require all three view kinds (verified: the key `componentViews` is absent from the export when the view is removed), then compare the elements shown in the component view with the workspace crates, name every missing and every extra component, and require a relationship between two modelled components for every workspace-internal entry of a `[dependencies]` table (also `[target.<cfg>.dependencies]` and `[dependencies.<name>]`; a key may be quoted, and a `package =` rename names the crate) of a crate manifest below the current directory, skipping `target`, `.git`, `node_modules`, `.tools` and `fixtures` directories; each missing relationship is printed as `missing relationship: <crate> -> <dependency>` (FR120). The success line is `architecture model OK: <n> components match the workspace crates; every dependency between them has a relationship`. The crate list reaches the script on standard input, from `cargo tree` behind `$(EXEC)` |
| When it changes | a crate, bounded context, external dependency or relationship between crates is added or removed; `cargo xtask new-context` prints this as a next step; skill `architecture-model-sync` |
| Downstream | the title, the system name and the context groups are the only texts a new service edits; the file is in `.templatesyncignore` |

## AI Guidance and Documentation

### Canonical verification list

Exactly one list exists: the block between `<!-- verification:start -->` and `<!-- verification:end -->` in the section "Verification" of `AGENTS.md`. It is the output of `make ci-list` (the gate targets of `make ci` in order) and `make docs-check` fails when the two differ. `CLAUDE.md`, `README.md`, `CONTRIBUTING.md`, `docs/make-targets.md`, `.claude/skills/AI-AGENT-GUIDE.md` and the skills `verification-workflow` and `quality-standards` link to that section and do not repeat it; rule (4) of the drift check fails any other numbered list or fenced block that cites three or more gate targets.

### Entry files

| File | Sections |
| --- | --- |
| `AGENTS.md` | (1) identity paragraph and the note that the file is guidance only, written without the repository name; (2) `Mandatory Workflow For AI Agents`: read `.claude/skills/AI-AGENT-GUIDE.md`, then `SKILL-DECISION-GUIDE.md`, then the matching `SKILL.md`, and follow it before claiming completion; (3) `Working Rules`: run every tool through `make`; never change an item listed in `protected-config.toml` to obtain a pass; keep Domain crates framework-free (`make architecture`); regenerate contract snapshots and update `workspace.dsl` in the same change; follow the commit and pull-request title form; never edit an applied migration; no `#[allow]`, and an inline linter directive names one rule and carries a reason; (4) `Verification`: the canonical block, and for documentation-only changes `make lint-repo docs-check`; (5) `Close-Out`: state which targets were run, which were not and why; (6) pointer to `.claude/skills/README.md` (the only skill inventory) |
| `CLAUDE.md` | `Start Here` (the three files above); `Project Snapshot` (Rust 1.99.1 workspace with one crate per bounded-context layer; axum REST and GraphQL; PostgreSQL through sqlx; snapshots under `.github/`; load tests under `tests/Load/`; model in `workspace.dsl`); `Command Reference` (`make help`, link to the verification section); then the BMAD-generated section "BMAD-METHOD Integration" kept as bmalph writes it, except that the separator rows of its three tables are `\| --- \| --- \| --- \|`, `\| --- \| --- \|` and `\| --- \| --- \| --- \|` (the pinned markdownlint rejects the rows bmalph writes, rule MD060). It states no rule that `AGENTS.md` does not state |

### Skills

`.claude/skills/README.md` is the index (one line per skill, quick start, scope statement). `AI-AGENT-GUIDE.md` explains use (classify, pick, follow, verify, then summarise), the general rules and three flows: code change, API change, architecture change. `SKILL-DECISION-GUIDE.md` has the decision tree (one intent per skill below), four combined scenarios (new endpoint; boundary failure after a refactor; complex handler; new bounded context) and the non-negotiables (fix root causes; use `make`; keep framework concerns out of Domain crates).

| Skill directory | PHP skill | `description` (when to use) | Commands it must cite |
| --- | --- | --- | --- |
| `api-resource-development` | `api-platform-crud` | Use when adding or changing a REST or GraphQL operation, a command, a query, an event subscriber or a migration of a bounded context. | `make new-context`, `make create-migration`, `make migrate`, `make sqlx-prepare`, `make generate-openapi-spec`, `make generate-graphql-spec`, `make unit-tests`, `make e2e-tests`; names the files of the example slice (`crates/company-subdomain/some-module/*`) and the recipes of the core document |
| `verification-workflow` | `ci-workflow` | Use when running the repository checks or fixing a failing gate before declaring a change complete. | `make ci`, `make ci-list`, the seven stage targets; maps each failing gate to a skill; close-out wording |
| `complexity-management` | `complexity-management` | Use when the complexity gate fails or a function approaches a limit. | `make complexity`, `make static-analysis`; limits are cited from `quality-standards` only |
| `architecture-boundary-fixer` | `deptrac-fixer` | Use when `make architecture` reports a violation or code must move between layers or bounded contexts. | `make architecture`, `make unused-dependencies`, `make validate-manifest`; one fix pattern per rule `arch::*` |
| `documentation-sync` | `documentation-sync` | Use when a change affects a command, an environment variable, an endpoint, a workflow or the guidance. | `make docs-check`, `make lint-repo`; lists the documentation surfaces and their triggers |
| `load-testing` | `load-testing` | Use when adding, changing or running a K6 scenario or a latency budget. | `make smoke-load-tests`, `make execute-load-tests-script scenario=<name>`, `make load-tests`, `make lint-load-tests` |
| `api-contract-development` | `openapi-development` | Use when the REST or GraphQL contract changes or a contract gate fails. | `make generate-openapi-spec`, `make generate-graphql-spec`, `make validate-contract-snapshots`, `make validate-openapi-spec`, `make openapi-diff`, `make graphql-diff`, `make schemathesis-validate` |
| `quality-standards` | `quality-standards` | Use when you need the list of gates with their thresholds and commands. | every gate target; the only guidance file that states thresholds, each with the protected file that holds it |
| `architecture-model-sync` | `structurizr-architecture-sync` | Use when a crate, bounded context, external dependency or relationship is added or removed. | `make validate-architecture-model`, `make start` |
| `testing-workflow` | `testing-workflow` | Use when writing, running or debugging tests at any level. | `make unit-tests`, `make integration-tests`, `make e2e-tests`, `make setup-test-db`, `make tests-with-coverage`, `make mutation-tests-diff`, `make bats`; `TEST_SEED` and `PROPTEST_RNG_SEED` for reproduction |
| `bmad-autonomous-planning` | (from `core-service`) | Use when a BMAD planning bundle must be produced from a short task description without interactive menus. | `make bmalph-setup`; adapted: bundles go to `specs/<bundle-id>/`, the artifact format rules of bmalph (`## Tech Stack`, epic and story headings, readiness wording) are stated, model names are replaced by "the strongest available model", and the rule that no production code is written stays |

Skill file format (from the PHP house style, with its defects corrected): path `.claude/skills/<kebab-case-name>/SKILL.md`; front matter with exactly `name` (equal to the directory) and `description` (one sentence that starts with "Use when"); one H1 in Title Case; three to five H2 sections, the first `## Use This Skill When`, then `## Commands`, `## Rules`, and last `## Verification` (a `bash` block of `make` targets, one per line) followed by one sentence that routes to companion skills by name; 30 to 70 lines; no images, external links or repository name; no table except the gate table of `quality-standards` (every gate by id with threshold and Make target); no threshold outside `quality-standards`; at most two gate targets in any fenced block or numbered list, with a link to the verification section of `AGENTS.md` for the full list (rule (4) of DD-17); every path and command named must exist. `make docs-check` verifies name, description, index entry, paths and commands; that a description says when to use the skill is checked by the readiness review (NFR44).

### `.claude/settings.json`

```json
{
  "permissions": {
    "allow": [
      "Bash(make ci:*)",
      "Bash(make format-check:*)",
      "Bash(git status:*)",
      "Bash(git diff:*)",
      "Bash(git log:*)",
      "Bash(git show:*)",
      "Bash(cargo metadata:*)",
      "Bash(cargo tree:*)",
      "Bash(cargo xtask config-guard --list)",
      "Bash(docker compose ps:*)",
      "Bash(docker compose logs:*)"
    ],
    "ask": [
      "Bash(make aws-load-tests:*)",
      "Bash(make aws-load-tests-cleanup:*)",
      "Bash(make update:*)",
      "Bash(make purge:*)",
      "Bash(make clean:*)",
      "Bash(make down:*)",
      "Bash(make migrate-rollback:*)",
      "Bash(make build:*)",
      "Bash(make stop:*)",
      "Bash(make static-analysis-fix:*)",
      "Bash(make unused-dependencies-fix:*)"
    ],
    "deny": [
      "Bash(git push --force:*)",
      "Bash(git push -f:*)",
      "Bash(git commit --no-verify:*)",
      "Bash(git commit -n:*)",
      "Bash(git push --no-verify:*)",
      "Bash(git -c core.hooksPath:*)",
      "Bash(git config core.hooksPath:*)",
      "Read(./.env.local)",
      "Edit(./.env.local)"
    ]
  },
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Edit|Write|MultiEdit|NotebookEdit",
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR\"/scripts/claude/guard-protected-files.sh pre"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Edit|Write|MultiEdit|NotebookEdit",
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR\"/scripts/claude/guard-protected-files.sh post"
          }
        ]
      }
    ]
  }
}
```

The listing shows two of the `make` entries of `allow`. The file holds one entry `Bash(make <target>:*)` for each of these targets and no entry for `make` as a whole (FR111: gate, test, export and start targets and read-only commands run without a prompt):

- aggregates and read-only: `ci`, `all`, `ci-sequential`, `ci-list`, the seven `ci-*` stages, `help`, `doctor`, `logs`, `new-logs`;
- start: `start`, `up`, `start-deps`;
- gates: `format-check`, `lint-code`, `lint-rustdoc`, `static-analysis`, `complexity`, `architecture`, `validate-configuration`, `validate-manifest`, `check-crate-isolation`, `validate-sqlx-metadata`, `check-requirements`, `unused-dependencies`, `check-security`, `check-security-offline`, `secrets-scan`, `image-scan`, `build-prod-image`, `docs-check`, `validate-architecture-model`, `lint-repo` and its eight `lint-*` parts, `lint-load-tests`, `validate-invariants`, `validate-contract-snapshots`, `validate-openapi-spec`, `openapi-diff`, `graphql-diff`, `schemathesis-validate`, `smoke-load-tests`, `pre-commit`, `pre-push`, `check-commit-message`;
- tests: `unit-tests`, `doc-tests`, `integration-tests`, `e2e-tests`, `all-tests`, `test`, `bats`, `setup-test-db`, `tests-with-coverage`, `coverage-xml`, `coverage-html`, `mutation-tests-diff`, `mutation-tests`;
- exports: `generate-openapi-spec`, `generate-graphql-spec`.

Every other target asks, which is the default for a command that no rule names (among them `install`, `bootstrap`, `format`, `migrate`, `create-migration`, `new-context`, `sqlx-prepare`, the manual load profiles and the bmalph targets). Deny rules win over ask rules, and ask rules over allow rules. Rules are written per target so that the lists hold under both readings of the prefix syntax; the two fix targets whose names begin with the name of an allowed gate are named under `ask` for that reason.

`scripts/claude/guard-protected-files.sh <pre|post>` reads the hook JSON from standard input with `jq`, takes `tool_input.file_path` (or `notebook_path`), makes it relative to the project directory and compares it with the protected items: the output of `cargo xtask config-guard --list`, cached in `target/claude-guard/items.txt` and refreshed when `protected-config.toml` is newer.

- `pre`, the path is a listed file or lies under a listed directory: exit 2 with the message `Protected quality configuration: <path>. Do not change it to make a check pass. A maintainer can approve the change: restart the session with ALLOW_PROTECTED_CONFIG_EDIT=1 and add the label approved-config-change to the pull request.` Exit status 2 blocks the tool call and returns the message to the assistant.
- `pre`, the path holds a listed section (an item of the form `<path>#<table or pointer>`): the script records the current state of the tracked files as a commit object with `git stash create` (`HEAD` when nothing is modified), writes its id to `target/claude-guard/base`, and exits 0. Nothing in the working tree, the index or the refs changes.
- `post`, a recorded base exists for the path: the script runs `cargo xtask config-guard --base <recorded id>` without the approval variable. When the output holds a `Modification of locked configuration table` or `Modification of locked configuration value` line for that file, it restores the file with `git restore --source=<recorded id> --worktree -- <path>` and exits 2 with the same message, naming the section; the edit is undone and the assistant is told why. Otherwise it exits 0. An edit of a dependency in the root `Cargo.toml` or of a load shape in `tests/Load/config.json.dist` therefore passes, and an edit of `[workspace.lints]` or of a shipped budget does not.
- With `ALLOW_PROTECTED_CONFIG_EDIT=1` in the environment both modes print a notice and exit 0. Without `cargo`, `git` or `jq` on `PATH`, or when the guard cannot run (no commit yet), they print a warning and exit 0; CI remains the enforcement.

The guard semantics are those of the core document, so the hook needs no second definition of "section": it asks the guard whether the edit changed a protected value, with the state before the edit as base. `.claude/settings.local.json` is git-ignored.

### `.claude/commands` and bmalph

- **Tracked wrappers.** The 54 files that `bmalph init --platform claude-code` writes to `.claude/commands/` (agents such as `architect.md`, workflows such as `create-prd.md`, and `bmalph*.md`) are tracked. Each gets this preface as its first paragraph, before bmalph's own line: "This wrapper requires local BMALPH assets under `_bmad/`, which this repository intentionally keeps out of git. If `_bmad/` is missing in a fresh clone or workspace, run `make bmalph-setup` first, or `bmalph upgrade --force` if BMALPH is already installed for this repo."
- **`bmalph/config.json`** is tracked as it exists today (`name`, `description`, `createdAt`, `platform: "claude-code"`, `upstreamVersions.bmadCommit`). `bmalph/state/` is ignored.
- **`scripts/install-bmalph.sh`** follows `core-service/scripts/local-coder/install-bmalph.sh` with its library functions inlined (no `lib/` directory): options `--platform <claude-code|codex>` (default `claude-code`), `--project-dir`, `--project-name` and `--project-description` (defaults read from `bmalph/config.json`), `--upgrade`, `--init`, `--dry-run`, `--skip-verify`. It installs the npm package `bmalph` at the pinned `BMALPH_VERSION` (2.11.0, `make/tool-versions.mk`) under `${HOME}/.npm-global` when the command is missing, prints `BMALPH installed:` with command, version and package, verifies with a `bmalph init --dry-run` in a temporary directory that the output names `.claude/commands/` (or `.agents/skills/` for codex) and prints `BMALPH dry-run verification passed for platform '<p>'.` With `--init` it refuses to run on a tree with tracked changes (unless `--dry-run`), runs `bmalph -C <dir> init …`, runs `bmalph upgrade --force` when `_bmad/COMMANDS.md` or `.ralph/ralph_loop.sh` is missing afterwards, and restores every tracked file that bmalph modified (`git restore --staged --worktree`), which keeps the prefaces, `CLAUDE.md` and `.gitignore` as committed.
- **Make targets:** `bmalph-install`, `bmalph-claude`, `bmalph-codex`, `bmalph-init`, `bmalph-setup` (table above). **Bats:** `make_bmalph.bats`, modelled on the `core-service` file with `claude-code` as the default platform and an isolated `HOME`.
- **Ignored:** `_bmad/`, `_bmad-output/`, `.ralph/`, `bmalph/state/`.
- **`specs/` layout:** one directory per planning bundle. This release ships `specs/rust-service-template-bootstrap/` with `README.md`, `research.md` (digest), `capability-catalog.md`, `product-brief.md`, `prd.md`, `architecture.md`, `architecture-delivery.md`, `epics.md`, `implementation-readiness.md`, `parity-matrix.md`, `run-summary.md` and `scope-decisions.md` (the binding scope decisions that the PRD, the README and the readiness review cite). Later bundles of the autonomous-planning skill go to `specs/<bundle-id>/` with the same file names.

### Human documentation

| File | Sections |
| --- | --- |
| `README.md` | StandWithUkraine banner; badges; what the template is and its baseline; supported host platforms (Linux x86_64 verified in CI; Linux aarch64 and macOS through `RUN_IN_CONTAINER=1`, Windows through WSL 2, none of the three verified); quick start (`make doctor`, `make bootstrap`) with the URLs `http://localhost:8081/api/docs`, `/api/graphql`, `/api/health`, `/ping` and `http://localhost:8080` (architecture model); AI workflow with the prompt pattern "read `AGENTS.md`, pick a skill, verify with `make ci`"; everyday commands (a link to `docs/make-targets.md`, never a list of gate targets); CI model; creating a service and receiving template updates; cloud load testing; documentation index; licence |
| `CONTRIBUTING.md` | issue first; branch naming; commit and pull-request title rule with examples and version rules; hooks (`make install-hooks`); what a pull request must contain; the two approval labels; review and squash merge; link to the verification section |
| `SECURITY.md` | the values of PRD assumption A-8: supported version is the latest release; reports go through GitHub private vulnerability reporting on the repository; a report is acknowledged within 5 working days; advisories are published as GitHub security advisories; scope (template code and tooling; a downstream service reports to its own maintainers); what the automated gates cover; no placeholder |
| `CHANGELOG.md` | generated by the release (G); header committed |
| `LICENSE` | CC0 1.0 Universal legal code (existing file) |
| `docs/README.md` | index of the documents below |
| `docs/getting-started.md` | prerequisites; `make doctor`; `make bootstrap`; first request; native mode, container mode (`RUN_IN_CONTAINER=1`) and the development container; stopping and cleaning |
| `docs/architecture.md` | overview of layers, crates and bounded contexts; request flow; error model; statelessness assumptions (NFR6); the statement that domain-event delivery is at most once because no outbox ships (PD-18); deployment assumptions (edge proxy, TLS, timeouts); link to `workspace.dsl` and to the decision records |
| `docs/adr/README.md`, `docs/adr/0001` to `0012` | decision records in the form context, decision, consequences, each naming the planning decisions it summarises: 0001 workspace and layers (AD-01, AD-02); 0002 architecture gate (AD-03, AD-25); 0003 error model (AD-06); 0004 configuration (AD-07); 0005 persistence and migrations (AD-08, AD-20); 0006 health and liveness (AD-11, AD-12); 0007 API contracts (AD-16 to AD-19, DD-10); 0008 tests, coverage and mutation (AD-23, AD-24, DD-09); 0009 Make execution model (DD-01, DD-02); 0010 workflow layout (DD-08, DD-13); 0011 release flow (DD-11, DD-12); 0012 container images (DD-19, DD-20) |
| `docs/new-service.md` | create from the template; set the package identity in the workspace manifest (`description`, `repository`), the only value to change besides the checkout directory name that yields the project name; then `bmalph/config.json` and the README badges; remove or keep the example context; `make new-context`; wiring steps; where a service adds its own Make targets (`make/service.mk`, `docs/make-targets-service.md`) and which template-owned files it extends; secrets, variables and labels to create; choosing one sync flavour |
| `docs/environment.md` | exactly the variables of rule (5) of the drift check: the core table, the compose and Makefile variables of this document and the two test seeds, each with default, consumer and the PHP name (FR117(c)); the placeholder policy for `.env` and `.env.test` |
| `docs/make-targets.md` | every target with class, needs and help text; the PHP-to-Rust target table (FR117(d)); CI-only steps with reasons; for each gate target where its passing run and its negative test are; targets without a Bats test with reasons, and the aliases and helper targets with the reason they have no test of their own; the inputs that only a script reads; link to the verification section |
| `docs/operations.md` | migrations and rollback (`make migrate`, `make migrate-rollback`, the rule that an applied migration is never edited); health-probe runbook per failure message; log schema and request ids; shutdown sequence and the meaning of exit status 1; toolchain update procedure (three places, one labelled pull request); tool-version review (a version only up to what the pinned install action lists); the first measurements of `make bootstrap`, `make ci`, the mutation run and the pull-request jobs; build caches in CI (the shared Rust cache keys and their savers, the in-place mutation builds of `MUTANTS_IN_PLACE`, what a restored cache cannot cover, and the log lines that check the rerun clause of NFR40 on hosted runners); `.trivyignore` and `.gitleaksignore` entries |
| `docs/troubleshooting.md` | `make doctor` findings; Docker Engine older than 25.0 or Compose older than 2.24; first start slow; port clashes; stale `.sqlx`; restoring after an interrupted Bats run; rate-limited tool installation; building the images behind a TLS-intercepting proxy (named build context, then `BUILD_IMAGE=0`) |
| `docs/extension-seams.md` | Redis through the `Cache` port; tracing and metrics at the request span; authentication layer; outbox |
| `docs/load-testing.md` | profiles, budgets, the budget rule of NFR1 and the measured values with their dates; adding a scenario; the isolated environment; the AWS runner, the credentials it reads from the environment and its costs |
| `docs/repository-settings.md` | required status checks (the check names of "CI/CD Workflows"); squash merging with the pull-request title as commit subject; permission for the release identity to push to `main`; private vulnerability reporting; the CodeRabbit GitHub App that `.coderabbit.yaml` needs (the file is inert without it); secrets, variables and the labels to create (`approved-breaking-change`, `approved-config-change`, `template_sync`, `bug`, `enhancement`); code scanning setup (CodeQL default setup off; `CODE_SCANNING_ENABLED=true` for a private repository that has code scanning); follow-ups: CODEOWNERS, rulesets, auto-merge, stale bot, OSSF Scorecard, multi-architecture images, image publishing |
| `docs/requests.http` | request collection in the plain-text `.http` format (variable `@baseUrl = http://localhost:8081`), one request each for `GET /ping`, `GET /api/health`, `GET /api/example_api_resources`, `POST /api/example_api_resources`, `GET /api/example_api_resources/{id}` and the three GraphQL operations |
| `.github/AUTORELEASE.md` | release flow; version rules, including that a breaking change raises the minor number below `1.0.0`; App creation and permissions; the two secrets; permission for the App to push to `main`; behaviour without secrets |
| `.github/TEMPLATE_SYNC_APP.md`, `.github/TEMPLATE_SYNC_PAT.md` | one guide per flavour: credentials, permissions, schedule, `.templatesyncignore`, the label needed when a sync changes protected configuration, keeping only one flavour, and the template changes that do not propagate because their files are ignored (lint policy and dependencies in the workspace manifest, load-test budgets, and every file of the table "Template-owned files a service extends") with how to apply them by hand |

Badges in `README.md`, all dynamic: workflow status of `quality.yml`, `tests.yml`, `contracts.yml` and `security.yml` (GitHub's own badge endpoint), Codecov coverage (meaningful once `CODECOV_TOKEN` is configured), latest release and licence (shields.io endpoints that read the repository). No static score image.

Licence statement: `CC0-1.0` in `[workspace.package].license`, the `LICENSE` file, the README licence section, the OpenAPI `info.license`, the image label `org.opencontainers.image.licenses` and `tests/Load/package.json`. `scripts/check-invariants.sh` fails on any other SPDX identifier in these places.

## Repository File Manifest

Everything the finished repository contains outside the Rust workspace tree of the core document (which lists `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, `deny.toml`, `architecture.toml`, `protected-config.toml`, `.env`, `.env.test`, `.cargo/`, `.config/`, `.sqlx/`, `migrations/`, `features/`, `crates/`, `tests/support/`, `tests/e2e/`, `xtask/`). **P** = protected (in `protected-config.toml`); **G** = generated, never edited by hand. There is no `infrastructure/` directory: the PHP one held the web-server configuration, the entrypoint and an empty emulator hook, none of which has a counterpart. There is no `.gitmodules`, no root `package.json` and no `docker/` directory. The template ships neither `make/service.mk` nor `docs/make-targets-service.md`: a downstream service creates them when it adds targets.

Root:

| Path | Purpose |
| --- | --- |
| `Makefile` | command surface: targets, help, lifecycle wiring |
| `make/tool-versions.mk` (P) | tool versions and tool image references |
| `make/gates.mk` (P) | gate command lines, thresholds, the coverage exclusion record, `CI_STAGES` |
| `Dockerfile`, `.dockerignore` | development and production images; build context filter |
| `docker-compose.yml`, `docker-compose.override.yml`, `docker-compose.prod.yml`, `docker-compose.load_test.override.yml` | base, development, production reference, isolated load-test environment |
| `workspace.dsl` | Structurizr architecture model |
| `.spectral.yaml` (P) | the PHP Spectral ruleset, byte-identical (10 custom rules plus `spectral:oas`): FR74 and NFR24 require the unchanged PHP ruleset so that both templates validate the same contract rules; the weakness of catalog note CI:D-28 (the rules `operation-requires-2xx-response` and `operation-requires-problem-response` accept a bare `default` response and enumerate the status codes) is accepted (NFR27); tightening needs a change of both requirements and the approval label `approved-config-change` for the protected file |
| `cog.toml` (P), `.cog/changelog.tera` (P) | release configuration and changelog template |
| `.editorconfig`, `.gitignore`, `.templatesyncignore` | editor settings; ignore rules; files a sync leaves alone |
| `.yamllint.yml` (P), `.markdownlint-cli2.yaml` (P), `.hadolint.yaml` (P), `.prettierrc.yml` (P), `.prettierignore` (P), `taplo.toml` (P), `_typos.toml` (P) | linter and formatter configuration |
| `.gitleaks.toml` (P), `.gitleaksignore` (P), `.trivyignore` (P) | secrets-scan rules and accepted findings; accepted image findings |
| `.coderabbit.yaml` | review-bot configuration |
| `AGENTS.md`, `CLAUDE.md` | assistant entry files |
| `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `LICENSE` | human entry documents; CC0-1.0 legal code |
| `CHANGELOG.md` (G) | release notes written by the release job |

`.github/`:

| Path | Purpose |
| --- | --- |
| `workflows/quality.yml`, `tests.yml`, `contracts.yml`, `change-control.yml`, `load-tests.yml`, `security.yml`, `repository.yml`, `scheduled.yml` | gate workflows |
| `workflows/release.yml`, `workflows/template-sync-app.yml`, `workflows/template-sync-pat.yml` | release and template sync |
| `dependabot.yml`, `zizmor.yml` (P) | dependency updates; workflow audit policy |
| `ISSUE_TEMPLATE/bug_report.md`, `ISSUE_TEMPLATE/feature_request.md`, `pull_request_template.md` | templates |
| `AUTORELEASE.md`, `TEMPLATE_SYNC_APP.md`, `TEMPLATE_SYNC_PAT.md` | setup guides |
| `openapi-spec/spec.yaml` (G), `graphql-spec/spec` (G) | contract snapshots written by the export commands |

`.claude/`, `.devcontainer/`, `.githooks/`, `bmalph/`:

| Path | Purpose |
| --- | --- |
| `.claude/settings.json` (P) | permission lists and the two protected-item hooks |
| `.claude/commands/*.md` (54 files, G by bmalph, preface restored from git) | BMAD and bmalph command wrappers |
| `.claude/skills/README.md`, `AI-AGENT-GUIDE.md`, `SKILL-DECISION-GUIDE.md` | skill index, usage guide, decision guide |
| `.claude/skills/<name>/SKILL.md` for `api-resource-development`, `verification-workflow`, `complexity-management`, `architecture-boundary-fixer`, `documentation-sync`, `load-testing`, `api-contract-development`, `quality-standards`, `architecture-model-sync`, `testing-workflow`, `bmad-autonomous-planning` | the eleven skills |
| `.devcontainer/devcontainer.json` | development container definition |
| `.githooks/commit-msg` (P), `.githooks/pre-commit` (P), `.githooks/pre-push` (P) | git hooks |
| `bmalph/config.json` | bmalph project configuration |

`scripts/`:

| Path | Purpose |
| --- | --- |
| `with-env.sh` (P) | run a command with the variables of env files as defaults |
| `doctor.sh`, `install-tools.sh`, `install-bats.sh`, `install-bmalph.sh` | preflight and installation |
| `setup-test-db.sh` (P), `setup-load-test-db.sh`, `with-load-test-stack.sh` (P) | test database; isolated environment lifecycle |
| `validate-openapi-spec.sh` (P), `openapi-diff.sh` (P), `graphql-diff.sh` (P) | contract lint and breaking-change checks |
| `mutation-diff.sh` (P), `validate-architecture-model.sh` (P), `secrets-scan.sh` (P), `image-scan.sh` (P), `check-sqlx-metadata.sh` (P) | gate wrappers |
| `check-docs.sh` (P), `check-invariants.sh` (P), `check-commit-message.sh` (P) | documentation drift; repository invariants; commit convention |
| `set-version.sh`, `release.sh` | version bump hook; release flow |
| `ci/detect-changes.sh`, `ci/summarize-tests.sh`, `ci/mutation-plan.sh` | classification of a pull request's changed files; per-gate result lines for the job summary; mutant count and shard matrix |
| `claude/guard-protected-files.sh` (P) | assistant hook |

`docs/`:

| Path | Purpose |
| --- | --- |
| `README.md`, `getting-started.md`, `new-service.md`, `troubleshooting.md` | index, first run, creating a service, problems and fixes |
| `architecture.md`, `adr/README.md`, `adr/0001-workspace-and-layers.md`, `adr/0002-architecture-gate.md`, `adr/0003-error-model.md`, `adr/0004-configuration.md`, `adr/0005-persistence-and-migrations.md`, `adr/0006-health-and-liveness.md`, `adr/0007-api-contracts.md`, `adr/0008-tests-coverage-mutation.md`, `adr/0009-make-execution-model.md`, `adr/0010-workflow-layout.md`, `adr/0011-release-flow.md`, `adr/0012-container-images.md` | architecture overview and decision records |
| `environment.md`, `make-targets.md` | the two references, each with its PHP-to-Rust mapping (FR117(c), (d)) |
| `operations.md`, `load-testing.md`, `extension-seams.md`, `repository-settings.md` | runbooks and first measurements, budgets, seams, settings and follow-ups |
| `requests.http` | request collection |

`tests/CLI/bats/` (P as a directory: existing tests and fixtures cannot be changed or removed without the label; new files are free):

| Path | Purpose |
| --- | --- |
| `test_helper.bash` | setup, planting and guaranteed restore |
| `make_general.bats`, `make_execution_mode.bats`, `make_lifecycle.bats`, `make_hooks_and_commits.bats`, `make_bmalph.bats` | tag `surface` |
| `make_quality_gates.bats`, `make_scaffolding.bats`, `make_database.bats` | tag `quality` |
| `make_tests.bats` | tag `tests` |
| `make_contracts.bats` | tag `contracts` |
| `make_load_tests.bats`, `make_security.bats` | tag `load` |
| `make_repository.bats`, `check_invariants.bats`, `check_docs.bats`, `check_architecture_model.bats`, `ci_scripts.bats`, `make_release.bats` | tag `repository` |
| `fixtures/badly_formatted.rs.txt`, `unwrap_used.rs.txt`, `banned_api.rs.txt`, `too_many_arguments.rs.txt`, `too_many_lines.rs.txt`, `excessive_nesting.rs.txt`, `cognitive_complexity.rs.txt`, `suppression_without_reason.rs.txt`, `broken_doc_link.rs.txt`, `unused_import_in_test.rs.txt`, `unsafe_block.rs.txt`, `uncovered_function.rs.txt`, `isolation_violation.rs.txt`, `failing_unit_test.rs.txt`, `failing_doc_test.rs.txt`, `failing_integration_test.rs.txt`, `failing_seeded_test.rs.txt`, `undefined_step.feature.txt`, `undefined_step_allowed.feature.txt`, `workflow_syntax_error.yml.txt`, `workflow_unpinned_action.yml.txt`, `duplicate_key.yml.txt`, `skipped_heading.md.txt`, `unquoted_expansion.sh.txt`, `Dockerfile.latest.txt`, `unformatted.toml.txt`, `misspelt.md.txt`, `unformatted.json.txt`, `bad_load_script.js.txt`, `unformatted_load_script.js.txt`, `failing_check_scenario.js.txt`, `broken_relationship.dsl.txt`, `architecture_model.dsl.txt`, `host_tool_calls.bats.txt`, `shared_env_forms.env.txt`, `shared_env_forms.expected.txt`, `refused_env_lines.txt`, `image_listing_planted.txt`, `failing_suite/failing.bats.txt` | planted inputs of the negative tests; nothing secret-shaped or vulnerable is committed (such inputs are generated at test time). `uncovered_function.rs.txt` is `#[expect(dead_code, reason = "planted by the Bats negative test")] fn planted(flag: bool) -> bool { flag }`, used by G14 and G15; `isolation_violation.rs.txt` is the `RawValue` function of DD-07 followed by the line `const _: fn(&serde_json::value::RawValue) -> usize = planted_raw_value;`, which uses it; `failing_check_scenario.js.txt` is a K6 scenario that requests the health endpoint and checks for status 200 (the service answers 204), used by the G22 checks-threshold test; `host_tool_calls.bats.txt` holds synthetic Bats chunks for the host-tool scan of `make_general.bats`, whose lines ending with `# call` must be reported; `image_listing_planted.txt` is a `tar -t` listing with allowed entries of the distroless production image and eleven planted entries that NFR12 forbids (four of them shells: `busybox/busybox`, `busybox/sh`, `usr/bin/bash`, `usr/local/bin/dash`), used by `make_security.bats`; `shared_env_forms.env.txt` holds the forms of an environment file that the service, Docker Compose and `with-env.sh` read the same way and `shared_env_forms.expected.txt` the values they read, and `refused_env_lines.txt` holds the forms that the service and Docker Compose read differently, which `with-env.sh` refuses with status 2 (all three for `make_general.bats`); `unused_import_in_test.rs.txt` is a `#[test]` function with an unused `use std::collections::HashMap;`, appended to `xtask/src/cli/tests.rs`, the test module of `xtask/src/cli.rs`, by G2; `unsafe_block.rs.txt` is a used, documented `const fn` whose `unsafe` block has a `// SAFETY:` comment and whose `#[expect(unsafe_code, reason = ..)]` only `forbid` overrules, used by G2; `unformatted_load_script.js.txt` is a K6 script that ESLint accepts, written with double quotes and four-space indentation, which Prettier rejects under `.prettierrc.yml` (G24) |

`tests/Load/` and `specs/`:

| Path | Purpose |
| --- | --- |
| `tests/Load/config.json.dist` (P: the four budget values of `health`) | load profiles and budgets |
| `tests/Load/scripts/health.js` | the shipped scenario |
| `tests/Load/utils/utils.js`, `scenarioUtils.js`, `scenariosBuilder.js`, `thresholdsBuilder.js` | scenario helpers |
| `tests/Load/get-load-test-scenarios.sh`, `execute-load-test.sh`, `run-smoke-load-tests.sh`, `run-average-load-tests.sh`, `run-stress-load-tests.sh`, `run-spike-load-tests.sh`, `run-load-tests.sh` | local runners |
| `tests/Load/aws-execute-load-tests.sh`, `cleanup.sh`, `config.sh`, `user-data.sh`, `trust-policy.json`, `s3-bucket-policy.json`, `s3-access-policy.json` | AWS runner |
| `tests/Load/package.json`, `pnpm-workspace.yaml`, `pnpm-lock.yaml` (G), `eslint.config.js` (P), `.gitignore`, `results/.gitignore` | lint tooling and ignore rules |
| `specs/rust-service-template-bootstrap/README.md`, `research.md`, `capability-catalog.md`, `product-brief.md`, `prd.md`, `architecture.md`, `architecture-delivery.md`, `epics.md`, `implementation-readiness.md`, `parity-matrix.md`, `run-summary.md`, `scope-decisions.md` | tracked planning bundle (copies of the planning artifacts) |

Git-ignored and therefore absent from a clean checkout: `target/`, `.tools/`, `mutants.out*/`, `tests/Load/results/*.html`, `tests/Load/config.json`, `tests/Load/aws-run.env`, `**/node_modules/`, `_bmad/`, `_bmad-output/`, `.ralph/`, `bmalph/state/`, `.env.local`, `.claude/settings.local.json`, `workspace.json`, `.structurizr/`.

## Delivery Validation

### Coverage of the catalog areas this document owns

| Catalog area | Home in this document | Gates |
| --- | --- | --- |
| 7 (non-Rust lint; guard in CI): CAP-CI-34, CAP-41 and its merged ids, NEW-20 | "Repository Lint and Hygiene"; DD-16; `change-control.yml` / `validate-configuration` | G23, G25, G31 |
| 8 (security): CAP-TQ-21, CAP-CI-10, CAP-DM-05, NEW-18, NEW-19 | DD-21, DD-24; `security.yml`; pins in `make/tool-versions.mk`; `validate-invariants` | G9, G10 |
| 12 Makefile surface and its tests: CAP-DM-23 to CAP-DM-31, CAP-TQ-01, CAP-TQ-18, CAP-TQ-19, CAP-TQ-32, CAP-TQ-34, CAP-CI-36, NEW-21, NEW-22 | "Command Surface"; "CLI Tests (Bats)"; `.gitignore` | G16 |
| 13 Contract checks: CAP-DM-34 to CAP-DM-37, CAP-CI-20 to CAP-CI-24 | contract targets and scripts; DD-10; `contracts.yml`, `change-control.yml` | G17 to G21 |
| 14 Load testing: CAP-DM-38 to CAP-DM-50, CAP-CI-17, CAP-CI-18 | "Load Testing"; DD-19, DD-25 | G22, G24 |
| 15 Containers and compose: CAP-DM-01 to CAP-DM-21 (CAP-DM-19 and CAP-DM-22 dropped by the catalog), CAP-TQ-35, NEW-23, NEW-24 | "Containers and Compose"; DD-20, DD-23. CAP-DM-11 (reload) is watchexec in the `dev` stage; CAP-DM-08 (bounded wait) is the `migrate` service; CAP-DM-09 (migrations at start) is the `migrate` step before `app` | G10 |
| 16 CI workflows: CAP-TQ-33, CAP-CI-02, CAP-CI-30, NEW-25 to NEW-29 | "CI/CD Workflows"; DD-08, DD-13 | all |
| 17 Release, updates, sync: CAP-CI-25, CAP-CI-27, CAP-CI-28, CAP-CI-35 | release flow; Dependabot file; sync workflows; three guides | none |
| 18 Hooks and commit convention: CAP-TQ-28, CAP-TQ-29 | "Git Hooks and Commit Convention"; DD-15 | G28 |
| 19 AI guidance: CAP-AD-01 to CAP-AD-23, NEW-30 to NEW-32 | "AI Guidance and Documentation"; DD-17, DD-18 | G26 |
| 20 Human documentation: CAP-AD-24 to CAP-AD-31, CAP-CI-29, NEW-33 to NEW-42 | documentation table; templates; `.editorconfig`; development container; request collection | G26 |
| 21 Architecture model: CAP-40, NEW-16 | "Architecture Model"; DD-05 | G27 |

Every gate G1 to G31 has a Make target (the script behind G28 is reachable as `check-commit-message`), a CI check under its own name (G11, G12 and G14 through the summary of the one job that executes the suites) and at least one negative test. The Rust-side gates map to core commands: G3 is QG-20 (`complexity`), G29 is QG-08 (`check-crate-isolation`), G30 is QG-11 (`validate-sqlx-metadata`); G31 is `validate-invariants`. Every delivery requirement of the final PRD (FR43 to FR49 in part, FR58 to FR120, FR121 to FR125) has a home above.

### Behaviour on the bootstrap pull request

The bootstrap pull request is a pull request from a branch of the repository itself, with the title `feat(#1): bootstrap the Rust service template with BMAD + bmalph`, against a `main` that holds only `.gitignore`, `LICENSE` and `README.md`. Every file is new, the diff is the whole repository, no cache exists and no organisation secret may be set; the repository is public and CodeQL default setup is not configured (read from the GitHub API by the validation round). The table says how each pull-request job behaves in that state. "Cold" is the estimated duration on a 4-vCPU runner without any cache, derived from the measurements under "Evidence"; no job was executed. The release, sync and scheduled workflows have no pull-request trigger.

| Workflow / job | Behaviour with everything new | Cold (min) | Timeout |
| --- | --- | --- | --- |
| every `changes` job | the diff holds non-documentation paths, so `code=true` and no job is skipped | 1 | 5 |
| `quality.yml` / `format-check`, `manifest`, `dependency-policy`, `unused-dependencies` | no build; `manifest` and the dependency jobs download the crates | 1 to 2 | 10 |
| `quality.yml` / `static-analysis` | cold clippy build, both passes, rustdoc, then the SARIF artifact | 4 to 6 | 20 |
| `quality.yml` / `clippy-sarif-upload` | same-repository pull request in a public repository with the variable unset: block D publishes; in a private repository without the variable it skips with the notice | 1 | 10 |
| `quality.yml` / `complexity`, `crate-isolation` | cold check build (54 s measured for the skeleton) | 3 to 6 | 20 |
| `quality.yml` / `architecture` | builds `xtask` | 2 | 10 |
| `quality.yml` / `query-metadata`, `check-requirements` | cold check or debug build of the workspace; the first also pulls and starts PostgreSQL | 4 to 5 | 20 |
| `tests.yml` / `tests` | instrumented build and one run of both suites; no `CODECOV_TOKEN`: the upload step is skipped with a notice and the job stays green | 8 to 10 | 30 |
| `tests.yml` / `e2e-tests` | test build (2 min 07 s measured for the skeleton), 24 scenarios | 5 to 6 | 25 |
| `tests.yml` / `mutation-plan` | the diff of `*.rs` from the merge base holds every file, so `--in-diff` lists every mutant (verified: a diff in which every file is new selects all mutants); 800 to 1000 mutants give 6 to 8 shards | 2 | 10 |
| `tests.yml` / `mutation-shard` | at most 150 mutants per shard at 5.2 s each (13 min) after tool set-up, service start and a baseline of about 1.5 min | 20 to 30 | 45 |
| `tests.yml` / `bats-group` | six parallel groups; base-comparing negatives use `base_ref=HEAD`; images are built in the job (no layer cache yet) | 9 to 16 | 30 |
| `tests.yml` / `mutation-tests`, `bats` | the two aggregators: green when every shard and every group is green | 1 | 5 |
| `contracts.yml` / `contract-snapshots` | debug build, export, `git status` | 4 to 5 | 20 |
| `contracts.yml` / `openapi-lint` | Spectral on the committed snapshot | 1 | 10 |
| `contracts.yml` / `schemathesis` | cold production image (5 to 7 min), isolated environment, 9 s of requests | 7 to 9 | 30 |
| `change-control.yml` / `title` | the title matches the commit rule | 1 | 5 |
| `change-control.yml` / `validate-configuration` | the base ref holds no protected item, so every item is new and the guard passes without a label (core "Protected configuration") | 2 to 3 | 10 |
| `change-control.yml` / `openapi-diff`, `graphql-diff` | the base ref resolves and holds no snapshot: both pass with the first-version notice (DD-10) | 1 | 10 |
| `load-tests.yml` / `smoke-load-tests` | cold production image, one scenario, budget 50 ms | 7 to 9 | 30 |
| `load-tests.yml` / `lint-load-tests` | pnpm install in the Node image | 1 to 2 | 10 |
| `security.yml` / `secrets-scan` | history and working files; the committed placeholders are clean (verified with gitleaks) | 1 | 10 |
| `security.yml` / `image-scan` | cold production image, database download; 0 findings on the pinned distroless base at the time of the validation | 7 to 9 | 20 |
| `security.yml` / `codeql` | block D publishes as for the clippy upload; `build-mode: none` | 10 to 20 | 30 |
| `repository.yml` / `lint-repo` | pulls about 400 MB of linter images | 3 to 4 | 15 |
| `repository.yml` / `invariants` | shell only | 1 | 10 |
| `repository.yml` / `docs-check` | debug build for the help texts | 4 to 5 | 20 |
| `repository.yml` / `architecture-model` | Structurizr image, `cargo tree` | 2 to 3 | 10 |

About 55 jobs start. With the 20 concurrent jobs of the smallest plan, the checks other than the mutation shards are estimated to finish within 30 minutes of the push (NFR39: 40 minutes cold); the mutation check has its own limit of 45 minutes per shard (PD-30). Every timeout is at least 1.5 times the upper cold estimate of its job. The first run is the measurement: its job durations are recorded in `docs/operations.md`, and a timeout or the shard divisor is changed only from those numbers.

### Estimate for `make ci`

Estimates for a 4-vCPU machine, in minutes; "cold" is the first run after `make bootstrap` on a clean clone, as in the scheduled job. Nothing below was measured end to end.

| Stage | Cold | Warm | Basis |
| --- | --- | --- | --- |
| `doctor` | under 1 | under 1 | version checks |
| 1 `ci-preflight` | 4 to 5 | 1 to 2 | pull of the linter images (about 400 MB) and of Structurizr; build of `xtask` |
| 2 `ci-static-analysis` | 5 to 7 | 1 | cold check build 54 s (skeleton); clippy 1.3 s per pass warm; debug build with sqlx and the AWS SDK 1 min 28 s (data spike) |
| 3 `ci-tests` | 6 to 8 | 2 to 3 | coverage run 1 min 23 s cold and end-to-end build 2 min 07 s (skeleton, 96 tests); the first release has several times as many tests |
| 4 `ci-contracts` | 1 to 2 | under 1 | exports reuse the debug build; three tool images |
| 5 `ci-mutation` | under 1 | under 1 | on a clean checkout of `main` the diff is empty; on a branch 5.2 s per changed mutant after a baseline of about 1.5 min |
| 6 `ci-delivery` | 9 to 12 | 2 to 3 | production image 5 to 7 min cold (124 s measured for a small workspace, plus about 2 min for the full dependency layer); Trivy database; two starts of the isolated environment at 12 s |
| 7 `ci-cli` | 18 to 22 | 18 to 22 | the Bats suite in sequence (section "CLI Tests (Bats)") |
| Total | 44 to 56 | 25 to 32 | |

`make bootstrap` is estimated at 11 to 13 minutes cold (development image 4 to 5, tool installation 2, image pulls 2, first compile in the container 3), within the 15 minutes of NFR35. The upper end of the `make ci` estimate is above the 45 minutes of NFR35, and bootstrap plus `make ci` is 55 to 69 minutes against the 75-minute limit of the clean-clone job (NFR39). NFR35 calls its figures initial budgets that the architect confirms or revises with measurements; no measurement exists before the first run on a hosted runner, so the budget is left as it is here and the decision is recorded for the run summary: the first scheduled run measures each stage, and if `make ci` then exceeds 45 minutes NFR35 is revised to the measured value with that record. The 75 minutes of the job stay the hard limit: if the measured total comes close to it, the slowest measured stage is shortened with a recorded reason, and the limit is not raised.

### Disposition of the validation findings

The validation round produced 53 findings for this document. All were processed; "adapted" names what differs from the proposed fix.

| Finding | Result |
| --- | --- |
| REQ-04, RF-09, DV-08 | applied: no end-to-end coverage run, help text and DD-02 corrected, exclusion record named in `make/gates.mk` |
| REQ-07, DV-14 | applied as the core document decided: the seed is recorded in `.cargo/config.toml`, not in `make/gates.mk`; `EXEC` forwards both variables; `scheduled.yml` sets both |
| REQ-09, REQ-31 | applied: budgets protected as JSON values, the file not in `files`; `.prettierignore` protected |
| REQ-14 | applied: `new-context` is one command |
| REQ-15, DV-25 (versions) | applied, adapted: rule (c) compares three places, because `RUST_VERSION` and `COCOGITTO_VERSION` are deleted as unused |
| REQ-17 | applied: table of extended files, ignore list, `-include make/service.mk`; `.env`, `.env.test` and `docs/adr/` moved to the second block |
| REQ-18 | applied: no `--ignore-unfixed` |
| REQ-19, DV-03 | applied, adapted: six Bats groups with one cache key each; no second passing run of a gate; estimates per stage and per job. The G29 plant is in `xtask`, not in the example context, so that the suite survives a service that removes that context. The isolated environment is started three times in the suite (once per test that needs the managed path), not once per file. The proposal to raise NFR35 and NFR39 in the PRD (DV-04) is not part of this document's findings and is not applied; see the estimate above |
| REQ-23, DV-10 | applied, adapted: `doctor` and `bootstrap` in container mode; no script outside `$(EXEC)` starts cargo (the recipes hand `check-docs.sh` the help texts and the model script the crate list); `run` deleted. No extra mount for reports: with the build directory outside the bind mount, `target/coverage` and `target/sarif` are ordinary directories of the checkout |
| REQ-24 | applied: `APP_ENV: test` in the isolated environment |
| REQ-27, DV-17 | applied: `--locked` on the four commands; the G8 negative keeps the lock file consistent |
| REQ-28 | applied: rule (g) with crate roots and lint groups |
| REQ-30, DV-12 | applied, adapted: the two features are referenced by digest alone, the form the feature reference takes, with the version in a comment; `moby: false`; the fixture image and the devcontainer CLI are pinned in `make/tool-versions.mk`; DL3008 reason recorded |
| REQ-32, DV-23 | applied: groups for all six ecosystems, LocalStack ignored, literal `FROM` references |
| REQ-33 | applied, adapted: rule (5) reads five sources (no `E2E_JUNIT_FILE`, which no longer exists); the model script compares the elements of the component view; its rule kinds are tested at script level on a small fixture model, and the Make-level negative is a parse error that works for any model; G16 negative added |
| REQ-34, DV-11 | applied, adapted: the gate job is read-only and a separate job uploads; it lives in `quality.yml`, where the artifact of the same run can be downloaded, not in `security.yml`; the visibility comes from the API, which works for every event; job-level permission blocks restate `contents: read` |
| REQ-35 | applied, adapted: per-target allow entries and a longer ask-list; the core guard has no option to check a candidate file, so the hook records the state before the edit and asks the guard after it (DD-18); the working rule cites the list, not a tool |
| REQ-36 | applied, adapted: `PROJECT_NAME` is assigned with `:=` inside an `origin` test, because the proposed pair of `?=` definitions is recursive; `TEST_ENV` uses `--` to end the file list |
| REQ-37 | applied, adapted: `IN_DEV_CONTAINER=1` is set by the compose override for the `app` container and no longer by the image, so the development container definition needs no override of it |
| REQ-38, DV-24 | applied: budgets at the ceilings with the NFR1 rule; shared cache key for `docs-check`; token as build secret of the development image in the Bats job; bmalph pinned |
| REQ-39 | applied and verified against LocalStack: AWS CLI from a pinned image, no `uuidgen`, no `envsubst` |
| REQ-40 | applied, adapted: a second prerequisite target `ensure-database` instead of a parameter (a prerequisite cannot pass one); development image built with a layer cache in the `surface` group |
| REQ-42 | applied: `migrate-status`, `run`, the three example scenarios, the separate PHP-to-Rust mapping document (its two mappings are in the two references), the end-to-end JUnit artifact and `BUILDER_BASE_IMAGE` are removed here; items (4), (5), (6) and (10) were removed in the core document |
| REQ-43, DV-26 | applied: job name `tests`, release triggers, counts, QG-01 to QG-20, `git status --porcelain` |
| RF-05 | applied: plants for G3 and G14 are plain functions; the blind spot is stated in DD-07 |
| RF-15 | applied: healthcheck comment |
| DV-01, DV-13 | applied: `CARGO_BUILD_WARNINGS` unset for cargo-mutants; fixture with a parameter, test with `CI=true` on the changed-code target; diff from the merge base; flags exported to scripts; gate scripts protected. The second negative on `mutation-tests` and the hook `MUTANTS_ARGS` are removed: one mechanism, tested through the target that pull requests and `make ci` use. The hook stays removed. NFR40 adds one switch, `MUTANTS_IN_PLACE`, set by CI only: exactly `1` adds `--in-place` and nothing else, and `make_tests.bats` checks that no other value of it and no `MUTANTS_ARGS` changes the command (batch B9) |
| DV-02 | applied: `/cargo-target`; Bats check of mounts and ownership |
| DV-05 | applied: typos 1.52.0 and the rule for raising a version |
| DV-06 | applied, adapted: no Trivy cache; the ignore comment must stand on the `uses:` line itself, after the version comment (verified; on the line above it has no effect) |
| DV-07, DV-16 | applied and verified: `braces` and `line-length` in `.yamllint.yml`; two ignore paths; additional negated patterns for G24, without which a load result failed that gate too |
| DV-09 | applied, adapted: the override removes `APP_ENV` and `LOG_LEVEL`; the Bats proof is the environment check inside the container, not a second end-to-end run |
| DV-15 | applied, adapted: one shard per 150 mutants up to 8 (a formula in place of three steps) |
| DV-18, DV-19, DV-21, DV-22 | applied: `rust-toolchain.toml` ignored by the build; floor last; literal emulator credentials and loopback ports; Docker Engine 25.0 and a 60 s start period |
| DV-20 | applied: first release at the manifest version; triggers; wording about queued runs |
| DV-25 (rest) | applied: the wait-for-service script and `DEV_ENV` deleted; the watch command line exists once (the base file names no `command` for `app`); rule (d) checks Bats tags and job names |

### Evidence

| Status | Items |
| --- | --- |
| Run for the first version (2026-11-10, Linux x86_64 only) | `workspace.dsl`: validation, failing variant, JSON export with and without the component view, viewer with a read-only file mount (200 after about 21 s). Compose: `up --detach --wait` with a one-shot `migrate` service (exit 0, and exit 1 when the step fails). cocogitto 7.0.0: custom template, omitted commit types, dry-run output, amend-and-retag, non-conventional commit in range, `0.y.z` breaking-change behaviour, `cog changelog --at`. gitleaks 8.30.1: `git` and `dir` modes on the committed `.env` content (clean) and on a planted key (exit 1). The complexity-only clippy command, clean and planted. The Rust image under an arbitrary uid with `CARGO_HOME` elsewhere. Bats commit SHAs with `git ls-remote`; image digests from the registry; cargo-binstall archive checksums; the version of `@devcontainers/cli` from the npm registry |
| Run by the validation round | The `Dockerfile` of the first version on a scratch workspace (124 s cold, 43.7 MB, user `65532:65532`, healthy after 6 s with a read-only root and no capabilities, no shell); the root-owned `./target` with the nested volume; rustup downloads with `rust-toolchain.toml` in the build context; the isolated environment with a built image (`up --wait` in 12 s, migrate, `/ping` on 18081, clean removal); `start-deps` twice without recreation; host credentials injected by the first base file; exec without forwarded environment and the exit status 127 of a bare assignment; the Makefile header of the first version; `CARGO_BUILD_WARNINGS=deny` emptying the mutation gate; `--in-diff` on an all-new diff and on an empty diff; the coverage sequence and its report formats; `cargo shear` with and without `--locked`; the install-action manifests; all 12 action SHAs and every used input; actionlint and zizmor on 11 derived workflows; yamllint and Prettier contradicting each other; Prettier on generated files; cocogitto without a tag; Trivy on a vulnerable and on the built image; the docker-in-docker feature on Debian trixie; bmalph's stack detection on both architecture documents |
| Run for this revision | The revised Makefile header in GNU Make 4.3 (project name from the directory, from `COMPOSE_PROJECT_NAME`, from the command line and from the environment; `CARGO_BUILD_WARNINGS` inside `ci` and outside; the exec line with four `-e` options; `BUILD_IMAGE=0`; `-include make/service.mk`). `scripts/with-env.sh` with several files and the reduced `.env.test` (host, container environment, caller override, usage errors). `docker compose exec -e NAME`: the caller's value is forwarded, an unset name stays unset. The four revised compose files in the three combinations, with real credentials exported in the host shell. yamllint and Prettier on every YAML listing of this document; Prettier on the development container definition and on `config.json.dist`; taplo on `taplo.toml`. hadolint on the revised `Dockerfile`. The BuildKit named context that replaces the digest-pinned builder image. The six-group Bats job, the sized mutation matrix, the read-only gate job with the separate upload job, block D, the scheduled seeds and the release condition, applied to the validators' workflow files: actionlint exit 0, zizmor no finding (one ignored), and the placement rule of the ignore comment. The G29 plant in the revised skeleton (`cargo check -p xtask` fails, `cargo check --workspace` passes in 54 s cold). The AWS CLI image against LocalStack 3.4.0 (six provisioning calls and an upload, run as root; started as another user for `--version` only). Registry reads: the two feature digests, the digest of the fixture image, the AWS CLI tag and digest, the `actions/download-artifact` tag. The two Prettier invocations on a tree with generated files |
| Taken from the research spikes | exec-form healthcheck; contract tool command lines and exit codes; K6 on the stock image with the PHP scripts; ESLint flat configuration; Bats helper loading; hook script; Dependabot keys; linter versions |
| Designed, not run | every workflow on GitHub (GitHub Actions cannot run here); every Bats file and every duration in the two estimate tables; `scripts/*` as specified, among them the assistant hook with `git stash create` and the guard, `mutation-plan.sh` and the model script with its JSON query; the revised `Dockerfile` as a build (its text was linted; the first version was built); the development stack with the build directory at `/cargo-target`; the development container definition; the AWS runner beyond the calls listed above; CodeQL; `corepack pnpm` inside the Node image; yamllint through the Python image; `cargo-binstall` for each tool; everything on Linux aarch64, macOS and WSL 2 |

### Measurement items and readings of requirement wording

No conflict with the PRD remains open in this document. The first five items of the previous version (coverage scope, lock file after scaffolding, section-level protection, shutdown and event failure, toolchain versions) were resolved by the revision of the core document and of this one. What remains is recorded for the run summary:

1. **`make ci` cold (NFR35) and the clean-clone job (NFR39).** Estimated at 44 to 56 minutes and 55 to 69 minutes; NFR35 is revised only from the first measurement (section "Estimate for `make ci`"). Measured on the first scheduled run on `main` (run 43107219243, commit `3e0bd3e`, 2026-11-11, runner label `ubuntu-24.04`): `make bootstrap` 7 min 27 s, `make ci` 30 min 28 s, the clean-clone job 38 min 14 s against its 75-minute limit; the budgets of NFR35 stand (`docs/operations.md`, "First measurements").
2. **Pull-request durations and the Bats job (NFR39).** Estimates per job are in "Behaviour on the bootstrap pull request"; the Bats requirement of 20 minutes is read per group, since the six groups are separate jobs. The first run is the measurement.
3. **Documentation-only budget (NFR39).** `docs-check` needs the `service` debug binary for the help texts; with the restored cache of key `service-debug` the job is planned under 3 minutes, and without it (a changed lock file before the next push to `main`) at 4 to 5. Measured on the first documentation-only pull request.
4. **Load budgets (NFR1).** The smoke budget is 50 ms, derived from the first hosted-runner measurement (p99 7.25 ms); the other three are the ceilings until they are measured (DD-25).
5. **Restored caches (NFR40), four recorded exceptions (F-10 decided: accepted).** (1) The `surface` Bats group: its tests run `make` targets inside the development container with the build directory `/cargo-target` on the compose volume `cargo_target`, which a fresh runner creates empty, so the workspace is compiled inside the container on every run; a restored host cache does not apply there, because the host target directory is not the container's. The image layers are cached once `main` holds a cache (`type=gha,scope=dev-image`). Measured on commit db7fe0801376a660dfd3ad728952549889bdcc4a (Tests run 43058614985, job 130654843585 `Bats: surface`): job 10 min 19 s, of which the dev-image build step 2 min 54 s and `make bats --filter-tags surface` 6 min 41 s. No job of that run restored a Rust cache or an image layer cache: caches are saved on `main` only and `main` held no workflow yet (log of job 130654843492: `No cache found.`). The `make bats` steps of `Bats: quality` and `Bats: repository` took 4 min 11 s and 3 min 26 s (jobs 4 min 41 s and 3 min 56 s), equally without a cache. What the exception costs against a restored cache can only be measured by the rerun after the first push to `main` (C12). (2) The scheduled job `devcontainer-build`: it builds the development-container definition (Dockerfile target `dev` plus two features) from nothing once a week to prove that the definition still builds; a layer cache would hide exactly the failures it exists to find (a moved base image, a removed package, a feature digest that no longer resolves). It compiles no workspace crate. Hosted durations: 175 s, failed in the common-utils feature, on `3e0bd3e` (run 43107219243, job 130813575932, `workflow_dispatch` on `main`), and 175 s, passed, after the `Dockerfile` fix on `027e1f1` (run 43119953189, job 130857597533, `workflow_dispatch` on the branch of pull request #2); the build step took 169 s and 171 s, of which the tool installation of the `dev` stage about 110 s. Run 43151995703 (job 130963101979, `workflow_dispatch` on the same branch, head `da5ea9c`) built it in 164 s and then passed the start and the checks from inside the container (FR118). (3) The scheduled job `clean-bootstrap-ci`: FR95 and NFR34 require it to run `make bootstrap` and `make ci` from a clean checkout with no restored cache, so it restores neither a Rust cache nor an image layer cache and builds everything from nothing once a week (38 min 14 s against its 75-minute limit in run 43107219243, item 1); the specific requirement wins over the cache clause. (4) The rerun clause, for the mutation shards of a pull request: on a rerun of a pull-request commit, a shard compiles the third-party crates that its packages need and the saver shard on `main` did not build. cargo-mutants builds its baseline with `--package` for every package with mutants in the shard and each mutant with `--package` for its own package, and cargo resolves features per selection, so the saver's entry covers only the saver's selections. Measured on commit `027e1f1` (the cargo-mutants build commands replayed from a target directory holding exactly what the saver builds, 508 compile units from empty): 0 to 77 third-party compile units per pull-request shard (17 for `xtask`, 15 for `shared-domain` or `shared-infrastructure`, 77 for the three crates of `some-module`), against 321 to 345 before the in-place switch; the saver's own selections recompile 0. Warming the saver does not close the gap (213 more compile units and build output from 2.80 GB to 4.64 GB on the saver, and still 2 to 38 crates in 4 of 9 sampled selections, of 2047 possible), and stable cargo 1.99.1 cannot unify features across the workspace. A cache saved per pull request would close it, but would cost about 800 MB of the 10 GB cache quota per pull request and commit and could evict the entries of `main` that every other job restores. Accepted: the clause holds with 0 for every other job and for the saver's selections. None of these jobs is on the path of a pull request's required time except `surface`, whose job stays under the 20-minute Bats limit of NFR39. The rerun of one unchanged commit after the first push to `main` (C12) still records NFR40 for every other job. Two jobs of `quality.yml` did not hold the rerun clause before pull request #2: `crate-isolation` and `query-metadata` restored the keys `check` and `service-debug`, which `static-analysis` and `check-requirements` save, and compiled 224 and 354 third-party compile units on every run, also after an exact-key restore (`e9eef29`, jobs 130883700953 and 130883700960; on `6a9dd24`, jobs 131262929871 and 131262929917, the same counts), because `cargo hack check --workspace --each-feature --locked` checks each crate alone and `cargo sqlx prepare --workspace --check -- --all-targets --all-features --locked` also builds the dev-dependencies, and cargo resolves features per selection. Neither is an exception: each job now saves a key of its own on `main` and prints `Third-party crates compiled: <n>`; the two entries add to the cache quota about what the neighbouring entries hold (`check` 422 MB, `service-debug` 372 MB, an estimate). On `d0468d9` of pull request #2 the count steps printed `Third-party crates compiled: 554` (`Crate isolation`, job 131312193338) and `409` (`Persistence query metadata`, job 131312193254), cold builds, because pull requests do not save and the new keys have no entry before the merge. The first push to `main` after the merge of pull request #2 is to save the two entries, and the rerun of an unchanged commit after it is to show 0 for both (`docs/operations.md`, "Checking the rerun clause on hosted runners"; to be measured after the merge of pull request #2). If the 10 GB quota cannot hold the two entries (an entry of `main` evicted in that rerun), the counts are recorded as a fifth exception instead, here, in NFR40 and in `docs/operations.md`.
6. **Image pin exception (FR125(b), NFR13).** The `database` image is `postgres:${DB_VERSION:-16}-alpine` without a digest; the PRD records the exception since the core revision (a digest overrides the tag).
7. **NFR13, "tool".** Read as cargo tools, Bats libraries, images, development-container features, npm packages run by a target and actions. Operating-system packages installed with `apt-get` in the `dev` stage are governed by the digest-pinned base image and carry no version of their own (DD-20).
8. **FR111, blocking an edit of a protected section.** A whole protected file is refused before the edit. A protected section is refused by undoing the edit immediately after it, because whether an edit touches the section can only be decided on the result (DD-18).
9. **Snapshot gate (core QG-19).** The gate regenerates in place; its negative test stages the planted snapshot. Kept as specified.
10. **Core TOML listings.** Files are committed in taplo's format; where a listing of the core document differs in whitespace only, the formatter decides.
11. **FR61 and FR66(d), scheduled-only steps.** FR61 asks for Make targets for gates, test levels, contract tools and maintenance actions. FR66(d) closes the list of the CI-only steps: those of pull-request CI (the title check, the evaluation of approval labels, publications and a scanning gate documented as CI-only) and the scheduled-only steps that FR95, FR118 and NFR35 require. The scheduled-only steps that DD-02 lists with their reasons are required by their own requirements, run only on `main` or on the schedule, and are admitted as CI-only steps listed in the Make-target reference: the full sharded mutation run (FR95; scope decisions: "a full, sharded run happens on `main` and on a schedule"); the build of the development container definition and the checks run in it, `docker info`, `make format-check` and `make start-deps` (FR95, FR118); and the stop and the timed start of the development stack in the scheduled clean-clone job `clean-bootstrap-ci`, `make stop` and `make start` (NFR35). The dev-container build has no Make target. `ci_only_targets` of `scripts/check-invariants.sh` cites this reading. This reading is accepted.

### Deferred

Image publishing with SBOM and provenance; multi-architecture images; CODEOWNERS, rulesets, auto-merge and a stale bot; OSSF Scorecard; stored performance baselines; an automatic issue on a failed scheduled run; automatic update of the versions in `make/tool-versions.mk`; a black-box subset of the Gherkin scenarios against the production image; an orchestrator manifest; example-resource load scenarios; a pipeline run of the average, stress and spike profiles. Each is listed in `docs/repository-settings.md` as a follow-up.

### Risks

| Risk | Why it cannot be closed before the first merge to `main` | Mitigation |
| --- | --- | --- |
| Workflows were designed and linted, never executed | GitHub Actions run only on GitHub | actionlint and zizmor pass on the derived files and gate the real ones; the bootstrap pull request exercises every pull-request job; the first push to `main` exercises the push paths |
| Job durations on the bootstrap pull request are estimates | no cache exists and nothing was run on a hosted runner | every timeout is at least 1.5 times the upper estimate; the mutation run is sized by the number of mutants; the first run is recorded |
| The cache quota of the repository (10 GB) against 14 Rust cache keys and two image scopes | sizes are known only after the first push to `main` | jobs share keys per artifact kind; `crate-isolation` and `query-metadata` have keys of their own, because their selections build other third-party units (about 0.8 GB together, estimated from the neighbouring entries `check`, 422 MB, and `service-debug`, 372 MB); GitHub evicts the least recently used entry, which costs time and never correctness |
| Release, GitHub App token, tag push and GitHub Release | need the App secrets and a push to `main` | the job skips cleanly without secrets; the `scripts/release.sh` sequence was run in a scratch repository except for the push and the release call |
| Template sync | runs only in a downstream repository | the guard makes it a no-op in the template; the first downstream run is supervised with `workflow_dispatch` |
| CodeQL for Rust and the SARIF upload | need code scanning on the hosted repository | publications, not gates; `CODE_SCANNING_ENABLED=false` switches both off; default setup must stay off (documented) |
| Codecov upload | needs the token and the hosted service | non-gating step; the coverage floor is local |
| Dependabot behaviour: six ecosystems, the `rust-toolchain` ecosystem, the second comment on the pinned `uses:` line of `bats-group` | Dependabot runs only on GitHub | the file follows the options reference; toolchain bumps are combined by hand; if an update drops the zizmor comment, G23 fails on that pull request and the comment is restored, or the ignore moves to `.github/zizmor.yml` |
| `!reset` and `!override` on the runner's Compose version | verified on 5.6.1 only | `make doctor` requires 2.24 or newer; the Bats lifecycle tests run on the runner |
| The development stack with the build directory outside `/app` was rendered, not started | the dev image was not rebuilt for this revision | the change follows the validators' reproduction of the defect; `make_lifecycle.bats` checks mounts and ownership |
| The assistant hook depends on the hook events and input fields of the assistant product | not run here | two Bats tests feed it recorded input; CI is the enforcement |
| The JSON query of the model script runs in the Node image against Structurizr's export format | written from the export seen in the first version, not run | script-level tests for every rule kind on a fixture model |
| The average, stress and spike budgets are the ceilings until measured (the smoke budget was set from the first hosted run) | those profiles run in no pipeline (PD-23) | recorded in `docs/load-testing.md`; a change needs the configuration label |
| `cargo-binstall` without a token may be rate-limited during image builds | depends on the network | optional build secret, passed in CI; explicit fallback to `cargo install` with a message |
| Tool images and tool versions are not updated by Dependabot | they live in a Makefile fragment | one protected file; reviewed with each toolchain update |
| The development container definition was not built | no devcontainer runtime here | built weekly by the scheduled workflow; `moby: false` follows the feature's install script |
| The AWS runner on real AWS | no credentials | the provisioning calls were run against LocalStack from the pinned CLI image; assumption A-6 of the PRD |

### Completion

- **Status:** revised after the validation round; ready for epics and implementation together with `architecture.md`.
- **Names fixed here:** 107 Make targets, 11 workflow files and their check names, 5 compose services, 4 compose files, 25 scripts, 18 Bats files with 6 group tags, 11 skills, the opt-in variable `RUN_IN_CONTAINER` with the forwarded variables of `EXEC_ENV`, the isolated project `<project>-load` on port 18081, the build directory `/cargo-target` of the development image, the labels `approved-config-change` and `approved-breaking-change`.
- **Order of authority for implementers:** `scope-decisions.md`; then the final PRD for requirements (scope 8.11); then `architecture.md` and this document for names and mechanisms; then the research reports.
