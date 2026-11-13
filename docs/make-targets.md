# Make targets

`make help` lists every target with its help text. This reference adds, for each target, its class, what it needs and whether `make ci` runs it, and then covers the PHP-to-Rust target mapping, the CI-only steps, where each gate's passing run and negative test are, the targets without a Bats test, and the inputs that only a script reads.

The ordered list of gate targets is not repeated here. It lives in one place, the verification section of [`AGENTS.md`](../AGENTS.md#verification), and `make ci-list` prints it.

## Classes and execution

- **TOOL** targets run cargo, `sqlx`, `xtask`, the `service` binary, `taplo`, `typos` or `zizmor`. They run natively in the current environment by default. With `RUN_IN_CONTAINER=1` (or `true`) they run as `docker compose exec` in the development container, so no toolchain is needed on the host; the environment must be up (`make start`). The opt-in is ignored, and the command runs natively, when `CI` is non-empty or when `IN_DEV_CONTAINER=1` (set in the `app` container, so also in a `make sh` shell).
- **HOST** targets drive Docker themselves (lifecycle, image-based tools, load tests, Bats) or are plain host commands. They always run where Make runs and print nothing special in container mode. A HOST target that calls the container engine exits with status 2 and `This target needs a reachable container engine (docker).` when `docker info` fails.
- **Needs** names the compose services that must be healthy: `database` is PostgreSQL, `localstack` is the SQS emulator, `app` is the development container. Targets that need services start them through their prerequisites (`ensure-deps`, `ensure-database`, `setup-test-db`) with `docker compose up --no-recreate`, so a service that already runs is never stopped or altered. "The isolated environment" means the target starts and removes the production-image environment of the load tests itself.
- **The test database.** The targets that use it (`setup-test-db`, `validate-sqlx-metadata`, `integration-tests`, `e2e-tests`, the coverage and mutation targets, `sqlx-prepare`) run behind `scripts/with-env.sh .env .env.test --` with `DATABASE_URL` removed first, so they always use the database `<DB_NAME>_test` of `.env.test`, even when the shell exports a `DATABASE_URL` for the development database (a habit of `sqlx-cli` users). Every other variable of the caller still wins over the two files; set `DB_PORT` or `DB_NAME` to move the test database. `scripts/setup-test-db.sh` itself refuses, with status 2, a database whose name does not end in `_test`, because it drops the database it is given.
- **In `make ci`** says whether the aggregate runs the target; the gate in brackets is the gate it passes (G1 to G31).

## All targets

| Target | Class | Needs | In `make ci` | Help text |
| --- | --- | --- | --- | --- |
| `all` | TOOL + HOST | as the targets it runs (all services) | the aggregate itself | Alias of ci |
| `all-tests` | TOOL | none | no | Run unit, integration and end-to-end tests |
| `architecture` | TOOL | none | yes (G4) | Check layer and bounded-context boundaries |
| `average-load-tests` | HOST | the isolated environment (the target starts and removes it) | no | Run every load scenario with the average profile |
| `aws-load-tests` | HOST | AWS credentials in the environment, or localstack in local mode | no | Run the load tests on a temporary AWS instance |
| `aws-load-tests-cleanup` | HOST | none | no | Remove everything aws-load-tests created |
| `bats` | HOST | Docker | yes (G16) | Run the Makefile tests |
| `bmalph-claude` | HOST | none | no | Install and verify bmalph for Claude Code |
| `bmalph-codex` | HOST | none | no | Install and verify bmalph for Codex |
| `bmalph-init` | HOST | none | no | Initialise bmalph in this project; BMALPH_DRY_RUN=true previews |
| `bmalph-install` | HOST | npm | no | Install and verify bmalph for BMALPH_PLATFORM=claude-code\|codex |
| `bmalph-setup` | HOST | none | no | Install and initialise bmalph in one step |
| `bootstrap` | HOST | all services | no | Prepare a fresh clone: check, install tools, hooks, build, start, migrate |
| `build` | HOST | none | no | Rebuild the development images from scratch |
| `build-prod-image` | HOST | none | yes | Build the production image |
| `check-commit-message` | HOST | none | no | Check the commit message in COMMIT_MSG_FILE or the title in COMMIT_TITLE |
| `check-crate-isolation` | TOOL | none | yes (G29) | Check that every crate builds alone, with each feature |
| `check-requirements` | TOOL | none | yes (G6) | Check that the service configuration is valid |
| `check-security` | TOOL | network | yes (G7) | Check advisories, licences, bans and sources of the locked dependencies |
| `check-security-offline` | TOOL | none | no (hook) | Check licences, bans and sources without network access |
| `ci` | TOOL + HOST | as the targets it runs (all services) | the aggregate itself | Run every gate in order and print the success line |
| `ci-cli` | TOOL + HOST | as the targets it runs | yes | Stage 7 of make ci: the Makefile tests |
| `ci-contracts` | TOOL + HOST | as the targets it runs | yes | Stage 4 of make ci: contract snapshots, OpenAPI lint and diff, GraphQL diff |
| `ci-delivery` | TOOL + HOST | as the targets it runs | yes | Stage 6 of make ci: production image, image scan, Schemathesis, smoke load tests |
| `ci-list` | HOST | none | no | Print the ordered gate targets of make ci, one per line |
| `ci-mutation` | TOOL + HOST | as the targets it runs | yes | Stage 5 of make ci: mutation testing of the changed code |
| `ci-preflight` | TOOL + HOST | as the targets it runs | yes | Stage 1 of make ci: configuration, invariants, secrets, repository lint, model |
| `ci-sequential` | TOOL + HOST | as the targets it runs (all services) | the aggregate itself | Alias of ci |
| `ci-static-analysis` | TOOL + HOST | as the targets it runs | yes | Stage 2 of make ci: formatting, manifest, analysis, complexity, security, docs |
| `ci-tests` | TOOL + HOST | as the targets it runs | yes | Stage 3 of make ci: test database, query metadata, coverage, doc tests, end-to-end tests |
| `clean` | HOST | none | no | Alias of purge |
| `complexity` | TOOL | none | yes (G3) | Check the complexity limits of production code |
| `coverage-html` | TOOL | database and localstack, test database | no | Write the HTML coverage report to target/coverage/html |
| `coverage-xml` | TOOL | database and localstack, test database | no | Write target/coverage/coverage.xml (Cobertura) and lcov.info without enforcing the floor |
| `create-migration` | TOOL | none | no | Create a reversible migration pair: name=\<snake_case_description> |
| `doc-tests` | TOOL | none | yes (G11) | Run the documentation tests |
| `docs-check` | TOOL + HOST | none | yes (G26) | Check that commands, paths and lists cited in the documentation exist |
| `doctor` | HOST | none | yes, first step | Check host prerequisites and pinned tool versions |
| `down` | HOST | none | no | Remove the development containers and network (volumes are kept) |
| `e2e-tests` | TOOL | database and localstack, test database | yes (G13) | Run the Gherkin end-to-end tests |
| `ensure-database` | HOST | none | via prerequisites | Start PostgreSQL unless this shell is the development container |
| `ensure-deps` | HOST | none | via prerequisites | Start PostgreSQL and LocalStack unless this shell is the development container |
| `execute-load-tests-script` | HOST | the isolated environment (the target starts and removes it) | no | Run one load scenario, all four profiles unless switched off: scenario=\<name> runSmoke=false runAverage=false runStress=false runSpike=false |
| `format` | TOOL | none | no | Format the Rust code |
| `format-check` | TOOL | none | yes (G1) | Check the Rust formatting (changes nothing) |
| `format-repo` | TOOL + HOST | none | no | Format TOML, YAML, JSON and the load-test JavaScript |
| `generate-graphql-spec` | TOOL | none | via snapshots | Write the GraphQL snapshot .github/graphql-spec/spec |
| `generate-openapi-spec` | TOOL | none | via snapshots | Write the OpenAPI snapshot .github/openapi-spec/spec.yaml |
| `graphql-diff` | HOST | none | yes (G20) | Fail on a breaking GraphQL change against base_ref |
| `help` | HOST | none | no | Show this help |
| `image-scan` | HOST | network | yes (G10) | Scan the production image for HIGH and CRITICAL vulnerabilities |
| `install` | HOST | none | no | Install the pinned tools and Bats on this machine and fetch dependencies from the lock file |
| `install-hooks` | HOST | none | no | Activate the git hooks of .githooks |
| `integration-tests` | TOOL | database and localstack, test database | via coverage (G12) | Run the integration tests against PostgreSQL and LocalStack |
| `lint-actions` | HOST + TOOL | none | via `lint-repo` | Lint the workflows (syntax and security) |
| `lint-code` | TOOL | none | via `static-analysis` | Lint production code, then all targets with tests relaxed |
| `lint-docker` | HOST | none | via `lint-repo` | Lint the Dockerfile |
| `lint-load-tests` | HOST | none | yes (G24) | Lint and format-check the load-test JavaScript |
| `lint-markdown` | HOST | none | via `lint-repo` | Lint the Markdown files |
| `lint-prettier` | HOST | none | via `lint-repo` | Check the formatting of YAML and JSON |
| `lint-repo` | HOST + TOOL | none | yes (G23) | Run every repository linter |
| `lint-rustdoc` | TOOL | none | via `static-analysis` | Lint the documentation comments |
| `lint-shell` | HOST | none | via `lint-repo` | Lint shell scripts, hooks and Bats files |
| `lint-spelling` | TOOL | none | via `lint-repo` | Check spelling |
| `lint-toml` | TOOL | none | via `lint-repo` | Check the TOML formatting |
| `lint-yaml` | HOST | none | via `lint-repo` | Lint the YAML files |
| `load-tests` | HOST | the isolated environment (the target starts and removes it) | no | Run every load scenario with all four profiles |
| `logs` | HOST | none | no | Follow all logs |
| `migrate` | TOOL | database | no | Apply all pending migrations to the development database |
| `migrate-rollback` | TOOL | database | no | Revert the most recently applied migration |
| `mutation-tests` | TOOL | database and localstack, test database | no (main, schedule) | Run mutation testing over all production code (fails on a surviving or timed-out mutant) |
| `mutation-tests-diff` | TOOL | database and localstack, test database | yes (G15) | Run mutation testing over the code changed against base_ref |
| `new-context` | TOOL | none | no | Create a bounded context: group=\<group> context=\<context> |
| `new-logs` | HOST | none | no | Follow only new log lines |
| `openapi-diff` | HOST | none | yes (G19) | Fail on a breaking OpenAPI change against base_ref |
| `pre-commit` | TOOL | none | no | Fast gates run by the pre-commit hook |
| `pre-push` | TOOL | none | no | Gates run by the pre-push hook |
| `purge` | HOST | none | no | Remove generated reports and load-test results |
| `schemathesis-validate` | HOST | the isolated environment (the target starts and removes it) | yes (G21) | Test the production image against the OpenAPI snapshot with Schemathesis |
| `secrets-scan` | HOST | none | yes (G9) | Scan the history and the working files for secrets |
| `setup-load-test-db` | HOST | the isolated environment already running (the target neither starts nor removes it) | via the wrapper | Drop, create and migrate the database of the isolated load-test environment |
| `setup-test-db` | TOOL | database | yes | Drop, create and migrate the test database |
| `sh` | HOST | app | no | Open a shell in the development container |
| `smoke-load-tests` | HOST | the isolated environment (the target starts and removes it) | yes (G22) | Run every load scenario with the smoke profile |
| `spike-load-tests` | HOST | the isolated environment (the target starts and removes it) | no | Run every load scenario with the spike profile |
| `sqlx-prepare` | TOOL | database, test database | no | Regenerate the offline query metadata in .sqlx from the freshly migrated test database |
| `start` | HOST | none | no | Build missing or stale images, start the development environment and wait until it is healthy |
| `start-deps` | HOST | none | via the two `ensure-` targets | Start the backing services named in DEPS if they are not running and wait until they are healthy |
| `static-analysis` | TOOL | none | yes (G2) | Run the strict static analysis with warnings denied |
| `static-analysis-fix` | TOOL | none | no | Apply every machine-applicable lint fix, then format |
| `static-analysis-sarif` | TOOL | none | no | Write the lint findings as SARIF to target/sarif/clippy.sarif |
| `stop` | HOST | none | no | Stop the development environment |
| `stress-load-tests` | HOST | the isolated environment (the target starts and removes it) | no | Run every load scenario with the stress profile |
| `test` | TOOL | none | no | Alias of all-tests |
| `tests-with-coverage` | TOOL | database and localstack, test database | yes (G11, G12, G14) | Run the unit and integration tests with coverage and enforce 100% lines and functions |
| `unit-tests` | TOOL | none | via coverage and `doc-tests` (G11) | Run the unit tests and the documentation tests |
| `unused-dependencies` | TOOL | none | yes (G8) | Fail on unused or misplaced dependencies |
| `unused-dependencies-fix` | TOOL | none | no | Remove unused dependencies from the manifests |
| `up` | HOST | none | no | Alias of start |
| `update` | TOOL | none | no | Update Cargo.lock within the version requirements of the manifests |
| `validate-architecture-model` | TOOL + HOST | none | yes (G27) | Validate workspace.dsl, its three views, its components and a relationship for every dependency between them |
| `validate-configuration` | TOOL | none | yes (G25) | Check protected configuration files and required directories against base_ref |
| `validate-contract-snapshots` | TOOL + HOST | none | yes (G17) | Fail when a committed contract snapshot differs from the generated one |
| `validate-invariants` | HOST | none | yes (G31) | Check the repository invariants: pins, versions, gate set, repository name, licence, suppressions, execution switch |
| `validate-manifest` | TOOL | none | yes (G5) | Check that Cargo.lock matches the manifests |
| `validate-openapi-spec` | HOST | none | yes (G18) | Lint the OpenAPI snapshot with Spectral |
| `validate-sqlx-metadata` | TOOL | database, test database | yes (G30) | Check that the offline query metadata in .sqlx is current |

## PHP target to Rust target

All 68 targets of the PHP template are accounted for. "Kept" means the same name and the same capability.

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
| `load-fixtures` | dropped | the PHP fixture loaded nothing |
| `phpcsfixer` | renamed `format` (fix) and `format-check` (gate) | fix-style and check-style targets are separate |
| `psalm` | renamed `static-analysis` | |
| `psalm-security` | replaced by `static-analysis`, `check-security` and the CodeQL job | no taint tool runs locally |
| `psalm-security-sarif` | renamed `static-analysis-sarif` | |
| `phpmd` | renamed `complexity` | |
| `phpinsights` | replaced by `format-check`, `static-analysis`, `complexity`, `architecture` | no composite score exists for Rust |
| `deptrac` | renamed `architecture` | |
| `deptrac-debug` | merged into `architecture` | the rule `arch::uncovered` names every unassigned crate |
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
| `aws-load-tests`, `aws-load-tests-cleanup` | kept | options can be passed through Make, `repo_url` (`-u`) among them |
| `ci`, `all`, `ci-sequential` | kept | the seven stages in order |
| `ci-preflight`, `ci-static-analysis`, `ci-mutation` | kept | contents redefined; no mutating step |
| `ci-deptrac` | merged into `ci-static-analysis` | |
| `ci-tests-and-openapi` | split into `ci-tests` and `ci-contracts` | |

New targets without a PHP counterpart: `doctor`, `bootstrap`, `start-deps`, `ensure-deps`, `ensure-database`, `build-prod-image`, `format-repo`, `lint-code`, `lint-rustdoc`, `check-crate-isolation`, `validate-sqlx-metadata`, `doc-tests`, `unused-dependencies`, `unused-dependencies-fix`, `mutation-tests-diff`, `validate-contract-snapshots`, `graphql-diff`, `lint-load-tests`, `migrate-rollback`, `sqlx-prepare`, `check-security-offline`, `secrets-scan`, `image-scan`, `docs-check`, `validate-architecture-model`, the ten `lint-*` and `validate-invariants` targets, `install-hooks`, `pre-commit`, `pre-push`, `check-commit-message`, the five `bmalph-*` targets, `new-context`, `ci-list`, `ci-delivery` and `ci-cli`.

Removed since the first version because no requirement asks for them: `run` (it ran cargo on the host outside the execution switch; `cargo run -p service -- serve` works without a wrapper) and `migrate-status`.

## CI-only steps

These steps of the hosted pipeline have no place in `make ci`. Each has a reason.

| Step | Reason |
| --- | --- |
| G28, the pull-request title check | needs the pull request; `make check-commit-message` with `COMMIT_TITLE` set runs the same rule locally |
| Evaluation of the two approval labels | local runs read `CONFIG_CHANGE_APPROVED` and `BREAKING_CHANGE_APPROVED` from the environment and otherwise fail and print the label name; only CI can read the labels |
| CodeQL analysis and the SARIF upload | publications without a threshold of their own; no local runner exists |
| The Codecov upload and the artifact uploads | publication, not verification |
| The full sharded mutation run | `make mutation-tests` exists locally but is not part of `make ci`: it takes up to an hour, so CI runs it on `main` and on a schedule, and `make ci` runs `mutation-tests-diff` for the changed code |
| The build of the development container definition and the checks run in it (scheduled workflow) | it needs the devcontainers CLI through npx and builds an image for several minutes; FR95 and FR118 ask for it on a schedule only and no gate of `make ci` covers it. The local commands are those of the workflow: `npx --yes @devcontainers/cli@<version> build --workspace-folder .`, then `up --workspace-folder .` and `exec --workspace-folder .` with `docker info`, `make format-check` and `make start-deps`, with the version `DEVCONTAINER_CLI_VERSION` of `make/tool-versions.mk` |
| The stop and the timed start of the development stack (scheduled clean-clone job) | the measurement of the `make start` budget of NFR35 (60 s, images and build output present) on a hosted runner; no gate of `make ci` measures it. Locally `make stop` and `make start` |

## Gate targets: passing run and negative test

A gate target has its negative test in the Bats suite and no second passing run there. The passing run is the stage of `make ci` named below and the CI job of the gate, so the suite does not repeat it. Stage 7 holds the suite itself.

| Gate | Target | Passing run | Negative test (Bats file) |
| --- | --- | --- | --- |
| G1 | `format-check` | stage 2 of `make ci` | `make_quality_gates.bats` (unformatted lines appended to `xtask/src/main.rs`) |
| G2 | `static-analysis` | stage 2 | `make_quality_gates.bats` (an `unwrap`; a `SystemTime::now()` call; a suppression without a reason; a broken intra-doc link, which rustdoc rejects; an unused import in the test module of `xtask`, which only the pass over all targets analyses; an `unsafe` block under an expectation that only the `forbid` level of `unsafe_code` overrules) |
| G3 | `complexity` | stage 2 | `make_quality_gates.bats` (one test per limit: a six-parameter function; a function over the line limit; a function nested six levels deep; a function over the cognitive-complexity limit) |
| G4 | `architecture` | stage 2 | `make_quality_gates.bats` (the checker's fixture workspace) |
| G5 | `validate-manifest` | stage 2 | `make_quality_gates.bats` (version changed without a lock update) |
| G6 | `check-requirements` | stage 2 | `make_quality_gates.bats` (`DATABASE_URL=mysql://x`) |
| G7 | `check-security` | stage 2 | `make_quality_gates.bats` (one test per policy) |
| G8 | `unused-dependencies` | stage 2 | `make_quality_gates.bats` (a misplaced dependency) |
| G9 | `secrets-scan` | stage 1 | `make_security.bats` (a key pair generated at test time; a key that exists only in the history) |
| G10 | `image-scan` | stage 6 | `make_security.bats` (a vulnerable fixture image) |
| G11 | `doc-tests`, `tests-with-coverage` | stage 3 | `make_tests.bats` (a failing test appended to `xtask`, run by `tests-with-coverage` together with the failing integration test of G12 in the test "G11, G12: a failing unit test and a failing integration test make tests-with-coverage fail and name both", and also by `unit-tests`, which `pre-push` runs; a failing documentation example appended to the test-support crate, run by `doc-tests`) |
| G12 | `tests-with-coverage` | stage 3 | `make_tests.bats` (a failing integration test appended to `xtask`, run by `tests-with-coverage` together with the failing unit test of G11 in the test "G11, G12: a failing unit test and a failing integration test make tests-with-coverage fail and name both", and also by `integration-tests`) |
| G13 | `e2e-tests` | stage 3 | `make_tests.bats` (a feature with an undefined step; the same step in a scenario tagged `@allow.skipped`) |
| G14 | `tests-with-coverage` | stage 3 | `make_tests.bats` (an uncovered function) |
| G15 | `mutation-tests-diff` | stage 5 | `make_tests.bats` (an uncovered function: a `MISSED` line) |
| G16 | `bats` | stage 7 | `make_general.bats` (a failing suite fixture) |
| G17 | `validate-contract-snapshots` | stage 4 | `make_contracts.bats` (a changed snapshot per file) |
| G18 | `validate-openapi-spec` | stage 4 | `make_contracts.bats` (an operation description removed) |
| G19 | `openapi-diff` | stage 4 | `make_contracts.bats` (a required response property removed) |
| G20 | `graphql-diff` | stage 4 | `make_contracts.bats` (a field removed; the same change approved with `BREAKING_CHANGE_APPROVED=true`; a base ref that does not exist) |
| G21 | `schemathesis-validate` | stage 6 | `make_contracts.bats` (an undocumented path added) |
| G22 | `smoke-load-tests` | stage 6 | `make_load_tests.bats` (a budget that cannot be met; a scenario whose check cannot pass) |
| G23 | `lint-repo` | stage 1 | `make_repository.bats` (one test per linter, each through its `lint-*` target; and the test "G23: make -n lint-repo runs every linter of the repository lint", a dry run that fails when a linter is dropped from the prerequisites of `lint-repo`) |
| G24 | `lint-load-tests` | stage 1 | `make_load_tests.bats` (a script with `console.log` and `==`, which ESLint rejects; a script that ESLint accepts but that is not formatted, which Prettier rejects) |
| G25 | `validate-configuration` | stage 1 | `make_quality_gates.bats` (a comment appended to `clippy.toml`) |
| G26 | `docs-check` | stage 2 | `make_repository.bats` (an unknown target cited in `README.md`) |
| G27 | `validate-architecture-model` | stage 1 | `make_repository.bats` and `check_architecture_model.bats` |
| G28 | `check-commit-message` | CI only (pull-request job `title`) | `make_hooks_and_commits.bats` (each rejected example) |
| G29 | `check-crate-isolation` | stage 2 | `make_quality_gates.bats` (a function that breaks the build of one crate alone) |
| G30 | `validate-sqlx-metadata` | stage 3 | `make_tests.bats` (one file of `.sqlx/` removed; a file of `.sqlx/` that no query generates, which `scripts/check-sqlx-metadata.sh` turns from a warning of `sqlx-cli` into a failure) |
| G31 | `validate-invariants` | stage 1 | `make_repository.bats` and `check_invariants.bats` (one test per rule) |

## Targets without a Bats test

The suite tests the targets that are not gates and the surface behaviour; the passing run of a gate is not repeated (see above). These targets have no Bats test, each with its reason. A test of `make_execution_mode.bats` fails when a target of `make help` is neither invoked by a Bats test nor named in the gate table above or in this section.

- `aws-load-tests` without local mode and `aws-load-tests-cleanup` against AWS: no credentials in CI. The LocalStack dry run is tested.
- `average-load-tests`, `stress-load-tests`, `spike-load-tests` and `load-tests`: minutes of load. The shared runner path is tested through `smoke-load-tests` and `execute-load-tests-script`.
- `mutation-tests`: up to an hour. It is the mutation command without the diff, and the same command line is tested through `mutation-tests-diff`.
- `coverage-xml` and `coverage-html`: a second and a third instrumented run of the suites. Their report commands are part of the tested `tests-with-coverage` sequence or differ from it in the output format only.
- `bootstrap`: run by the scheduled workflow.
- `update`: rewrites the lock file.
- `build`: minutes of image build. Its command line is tested with a Make dry run (`-n`) in `make_execution_mode.bats`, together with the `--build` and `--no-build` forms of `start`.
- `build-prod-image`: minutes of image build. Stage 6 of `make ci` runs it, and the hosted jobs build the production image with the cached build action instead.
- `format`, `format-repo`, `static-analysis-fix`, `unused-dependencies-fix` and `sqlx-prepare`: fix-style targets that rewrite tracked files.
- `ci` in full: tested with `CI_STAGES` and a Make dry run (`-n`).

The aliases and helper targets have no test of their own either; the target they point at, or the target that calls them, carries the test:

- Aliases: `up` (alias of `start`), `clean` (alias of `purge`), `test` (alias of `all-tests`), `all` and `ci-sequential` (aliases of `ci`). The five are PHP target names of FR62, so the test "FR62: the 42 PHP target names are targets of make help" of `make_general.bats` checks that they exist: it fails and prints every name of FR62 that is no longer a target of `make help`.
- Helper targets: `ensure-deps` and `ensure-database` (prerequisites of the tested targets that need services), `setup-load-test-db` (called by the isolated-environment wrapper), `check-security-offline` (a prerequisite of `pre-commit`; the offline subset of the tested `check-security`), `bmalph-claude` and `bmalph-codex` (call `bmalph-install` with a fixed platform), the stage targets `ci-preflight`, `ci-static-analysis`, `ci-tests`, `ci-contracts`, `ci-mutation`, `ci-delivery` and `ci-cli` (each recipe is generated from its `CI_*_TARGETS` variable, one line per gate, so a failing gate stops its stage; a Bats test checks that a dry run of `make ci` runs exactly the sequence that `make ci-list` prints), `static-analysis-sarif` (writes the lint findings in a publication format; the gate is `static-analysis`), `lint-code` and `lint-rustdoc` (the two halves of `static-analysis`), `all-tests`, `unit-tests` and `integration-tests` (compositions of the suites that the gates G11 to G13 run), `install` (downloads tools; run by `bootstrap`), `pre-push` (runs `unit-tests`) and `sh` (opens an interactive shell).

## Inputs that only a script reads

These inputs are not Make variables, so they are not in the environment reference. A script reads them from the environment.

| Input | Read by | Purpose |
| --- | --- | --- |
| `COMMIT_TITLE` | `check-commit-message` | the pull-request title to check; read from the environment so that a title cannot inject shell text into a recipe |
| `COMMIT_MSG_FILE` | `check-commit-message` | the file of the commit message, set by the commit-msg hook |
| `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_SESSION_TOKEN`, `AWS_REGION` | `aws-load-tests` | credentials of the cloud load run; the instance and bucket it creates cost money until `aws-load-tests-cleanup` removes them |
| `MAKE_COMMAND` | `doctor` | the GNU Make that runs the Makefile, set by the `doctor` and `bootstrap` recipes; without it the doctor checks the `make` on `PATH` (on macOS, with Homebrew, run `gmake` or put its `gnubin` directory first on `PATH`) |
| `MAKE` | the git hooks | the make the hooks run; without it they use `gmake` when it exists, else `make` |
| `ALLOW_PROTECTED_CONFIG_EDIT` | the assistant hook | set to `1` to allow an assistant edit of a protected configuration file |

## Service targets

A service defines its own targets in the file make/service.mk, which the `Makefile` loads with `-include`, and documents them in docs/make-targets-service.md. The template ships neither file, so a template sync cannot change them, and the drift check reads both references.
