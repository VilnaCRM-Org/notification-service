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

# Caller-settable variables (a value set by the caller always wins, FR64). `CARGO_BUILD_WARNINGS` is declared in
# the header above and must stay undefined until then, because of the target-specific line for `ci`.
RUN_IN_CONTAINER ?=
CI ?=
IN_DEV_CONTAINER ?=
DEPS ?= database localstack
base_ref ?= origin/main
SCHEMATHESIS_URL ?= http://localhost:$(LOAD_TEST_APP_PORT)
scenario ?=
runSmoke ?= true
runAverage ?= true
runStress ?= true
runSpike ?= true
LOCAL_MODE_ENV ?= false
region ?=
ami ?=
instance_type ?=
instance_tag ?=
role ?=
branch ?=
repo_url ?=
security_group ?=
load_target ?=
name ?=
group ?=
context ?=
MUTANTS_SHARD ?=
MUTANTS_IN_PLACE ?=
ARCH_ARGS ?=
DENY_ARGS ?=
BATS_PATH ?= tests/CLI/bats
BATS_ARGS ?=
DOCKERFILES ?= Dockerfile
IMAGE_TO_SCAN ?=
BMALPH_PLATFORM ?= claude-code
BMALPH_DRY_RUN ?= false

# Helpers of the recipes.
REQUIRE_DOCKER = @$(DOCKER) info >/dev/null 2>&1 || { echo 'This target needs a reachable container engine (docker).' >&2; exit 2; }
NODE_RUN = $(DOCKER_RUN) -e HOME=/tmp -e COREPACK_HOME=/tmp/corepack -v "$(CURDIR):/repo" -w /repo/tests/Load $(NODE_IMAGE)
BATS = $(if $(wildcard .tools/bats/core/bin/bats),.tools/bats/core/bin/bats,bats)
SCHEMATHESIS_RUN = $(DOCKER) run --rm --network host -v "$(CURDIR)/.github/openapi-spec:/data:ro" $(SCHEMATHESIS_IMAGE) run /data/spec.yaml --url $(SCHEMATHESIS_URL) $(SCHEMATHESIS_FLAGS)
AWS_OPTIONS = $(if $(region),-r "$(region)") $(if $(ami),-a "$(ami)") $(if $(instance_type),-t "$(instance_type)") $(if $(instance_tag),-i "$(instance_tag)") $(if $(role),-o "$(role)") $(if $(branch),-b "$(branch)") $(if $(repo_url),-u "$(repo_url)") $(if $(security_group),-s "$(security_group)") $(if $(load_target),-l "$(load_target)")
PRETTIER_REPO_ARGS = --config /repo/.prettierrc.yml --ignore-path /repo/.gitignore --ignore-path /repo/.prettierignore /repo
PNPM_INSTALL = corepack pnpm install --frozen-lockfile --ignore-scripts --store-dir /tmp/pnpm-store

# The seven stages of `make ci` (DD-02), in order; `ci-list` prints the targets of all of them.
CI_PREFLIGHT_TARGETS := validate-configuration validate-invariants secrets-scan lint-repo lint-load-tests validate-architecture-model
CI_STATIC_ANALYSIS_TARGETS := format-check validate-manifest check-crate-isolation static-analysis complexity architecture check-security unused-dependencies check-requirements docs-check
CI_TESTS_TARGETS := setup-test-db validate-sqlx-metadata tests-with-coverage doc-tests e2e-tests
CI_CONTRACTS_TARGETS := validate-contract-snapshots validate-openapi-spec openapi-diff graphql-diff
CI_MUTATION_TARGETS := mutation-tests-diff
CI_DELIVERY_TARGETS := build-prod-image image-scan schemathesis-validate smoke-load-tests
CI_CLI_TARGETS := bats
CI_GATES = $(CI_PREFLIGHT_TARGETS) $(CI_STATIC_ANALYSIS_TARGETS) $(CI_TESTS_TARGETS) $(CI_CONTRACTS_TARGETS) $(CI_MUTATION_TARGETS) $(CI_DELIVERY_TARGETS) $(CI_CLI_TARGETS)

# A stage of `make ci` has no gate list of its own: its recipe is generated from the CI_*_TARGETS variable that
# `ci-list`, the invariants check and AGENTS.md read. One recipe line per gate, so the first failing gate stops the stage.
define NEWLINE


endef
# The make that runs this Makefile, handed to the doctor. The indirection keeps `make -n` from running the doctor
# (a recipe line that names MAKE literally is executed even in a dry run).
DOCTOR_MAKE = $(MAKE)
STAGE_RECIPE = $(foreach gate,$(1),@$(MAKE) --no-print-directory $(gate)$(NEWLINE))

# ---------------------------------------------------------------------------------------------------------------
# Lifecycle
# ---------------------------------------------------------------------------------------------------------------
.PHONY: help doctor bootstrap start up start-deps ensure-deps ensure-database build stop down sh logs new-logs purge clean

help: ## Show this help
	@printf '%s\n' 'Usage:' '  make [target] [arg="val"...]' 'Targets:'
	@grep -hE '^[a-zA-Z0-9_-]+:.*## ' $(MAKEFILE_LIST) | LC_ALL=C sort -t: -k1,1 | awk 'BEGIN {FS = ":.*## "} {printf "%-28s %s\n", $$1, $$2}'

doctor: ## Check host prerequisites and pinned tool versions
	@MAKE_COMMAND="$(DOCTOR_MAKE)" scripts/doctor.sh

bootstrap: ## Prepare a fresh clone: check, install tools, hooks, build, start, migrate
	MAKE_COMMAND="$(DOCTOR_MAKE)" scripts/doctor.sh --prerequisites
ifeq ($(EXEC),)
	@$(MAKE) --no-print-directory install
else
	scripts/install-bats.sh
endif
	@$(MAKE) --no-print-directory install-hooks
	MAKE_COMMAND="$(DOCTOR_MAKE)" scripts/doctor.sh
	@$(MAKE) --no-print-directory start
	@scripts/with-env.sh .env -- bash -c 'echo "Service:   http://localhost:$${APP_PORT}/ping"; echo "API docs:  http://localhost:$${APP_PORT}/api/docs"; echo "GraphQL:   http://localhost:$${APP_PORT}/api/graphql"; echo "Diagrams:  http://localhost:$${STRUCTURIZR_PORT}/workspace/1/diagrams (profile docs)"'

start: ## Build missing or stale images, start the development environment and wait until it is healthy
	$(REQUIRE_DOCKER)
	$(DOCKER_COMPOSE) up --detach --wait --wait-timeout 900 $(if $(filter 0,$(BUILD_IMAGE)),--no-build,--build)

up: start ## Alias of start

start-deps: ## Start the backing services named in DEPS if they are not running and wait until they are healthy
	$(REQUIRE_DOCKER)
	$(DOCKER_COMPOSE) up --detach --wait --no-recreate $(DEPS)

ensure-deps: ## Start PostgreSQL and LocalStack unless this shell is the development container
	@[ "$(IN_DEV_CONTAINER)" = 1 ] || $(MAKE) --no-print-directory start-deps

ensure-database: ## Start PostgreSQL unless this shell is the development container
	@[ "$(IN_DEV_CONTAINER)" = 1 ] || $(MAKE) --no-print-directory start-deps DEPS=database

build: ## Rebuild the development images from scratch
	$(REQUIRE_DOCKER)
	$(DOCKER_COMPOSE) build --pull --no-cache

stop: ## Stop the development environment
	$(REQUIRE_DOCKER)
	$(DOCKER_COMPOSE) stop

down: ## Remove the development containers and network (volumes are kept)
	$(REQUIRE_DOCKER)
	$(DOCKER_COMPOSE) down --remove-orphans

sh: ## Open a shell in the development container
	$(REQUIRE_DOCKER)
	$(DOCKER_COMPOSE) exec --user $(HOST_UID):$(HOST_GID) app bash

logs: ## Follow all logs
	$(REQUIRE_DOCKER)
	$(DOCKER_COMPOSE) logs --follow

new-logs: ## Follow only new log lines
	$(REQUIRE_DOCKER)
	$(DOCKER_COMPOSE) logs --tail=0 --follow

purge: ## Remove generated reports and load-test results
	rm -rf target/coverage target/sarif target/reports target/mutants.diff mutants.out mutants.out.old tests/Load/results/*.html tests/Load/results/aws

clean: purge ## Alias of purge

# ---------------------------------------------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------------------------------------------
.PHONY: install update build-prod-image

install: ## Install the pinned tools and Bats on this machine and fetch dependencies from the lock file
	scripts/install-tools.sh
	scripts/install-bats.sh
	cargo fetch --locked

update: ## Update Cargo.lock within the version requirements of the manifests
	$(EXEC) cargo update

build-prod-image: ## Build the production image
	$(REQUIRE_DOCKER)
	@[ "$(BUILD_IMAGE)" = 0 ] || $(DOCKER) build --target prod --tag $(APP_IMAGE) .

# ---------------------------------------------------------------------------------------------------------------
# Formatting, lint and analysis
# ---------------------------------------------------------------------------------------------------------------
.PHONY: format format-check format-repo lint-code lint-rustdoc static-analysis static-analysis-fix static-analysis-sarif complexity architecture validate-configuration validate-manifest check-crate-isolation validate-sqlx-metadata check-requirements unused-dependencies unused-dependencies-fix

format: ## Format the Rust code
	$(EXEC) cargo fmt --all

format-check: ## Check the Rust formatting (changes nothing)
	$(EXEC) $(FORMAT_CHECK_CMD)

format-repo: ## Format TOML, YAML, JSON and the load-test JavaScript
	$(REQUIRE_DOCKER)
	$(EXEC) taplo fmt
	$(NODE_RUN) sh -c '$(PNPM_INSTALL) && corepack pnpm exec prettier --write $(PRETTIER_REPO_ARGS)'
	$(NODE_RUN) sh -c '$(PNPM_INSTALL) && corepack pnpm exec prettier --write --config /repo/.prettierrc.yml . "!results/**" "!pnpm-lock.yaml"'

lint-code: ## Lint production code, then all targets with tests relaxed
	$(EXEC) $(LINT_PROD_CMD)
	$(EXEC) $(LINT_ALL_CMD)

lint-rustdoc: ## Lint the documentation comments
	$(EXEC) $(LINT_RUSTDOC_CMD)

static-analysis: lint-code lint-rustdoc ## Run the strict static analysis with warnings denied

static-analysis-fix: ## Apply every machine-applicable lint fix, then format
	$(EXEC) cargo clippy --fix --allow-dirty --allow-staged --locked --workspace --all-targets --all-features -- -D warnings
	$(EXEC) cargo fmt --all

static-analysis-sarif: ## Write the lint findings as SARIF to target/sarif/clippy.sarif
	mkdir -p target/sarif
	$(EXEC) bash -c 'cargo clippy --locked --workspace --all-targets --all-features --message-format=json -- -D warnings -A clippy::cognitive_complexity -A clippy::too_many_lines | clippy-sarif > target/sarif/clippy.sarif' || true

complexity: ## Check the complexity limits of production code
	$(EXEC) $(COMPLEXITY_CMD)

architecture: ## Check layer and bounded-context boundaries
	$(EXEC) $(ARCH_CMD)

validate-configuration: ## Check protected configuration files and required directories against base_ref
	$(EXEC) $(CONFIG_GUARD_CMD)

validate-manifest: ## Check that Cargo.lock matches the manifests
	$(EXEC) $(MANIFEST_CMD)

check-crate-isolation: ## Check that every crate builds alone, with each feature
	$(EXEC) $(HACK_CMD)

validate-sqlx-metadata: setup-test-db ## Check that the offline query metadata in .sqlx is current
	$(EXEC) $(TEST_ENV) scripts/check-sqlx-metadata.sh $(SQLX_CHECK_CMD)

check-requirements: ## Check that the service configuration is valid
	$(EXEC) $(CHECK_CONFIG_CMD)

unused-dependencies: ## Fail on unused or misplaced dependencies
	$(EXEC) $(SHEAR_CMD)

unused-dependencies-fix: ## Remove unused dependencies from the manifests
	$(EXEC) cargo shear --fix

# ---------------------------------------------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------------------------------------------
.PHONY: unit-tests doc-tests integration-tests e2e-tests all-tests test bats

unit-tests: ## Run the unit tests and the documentation tests
	$(EXEC) $(UNIT_CMD)
	$(EXEC) $(DOCTEST_CMD)

doc-tests: ## Run the documentation tests
	$(EXEC) $(DOCTEST_CMD)

integration-tests: ensure-deps setup-test-db ## Run the integration tests against PostgreSQL and LocalStack
	$(EXEC) $(TEST_ENV) $(INTEGRATION_CMD)

e2e-tests: ensure-deps setup-test-db ## Run the Gherkin end-to-end tests
	$(EXEC) $(TEST_ENV) $(E2E_CMD)

all-tests: unit-tests integration-tests e2e-tests ## Run unit, integration and end-to-end tests

test: all-tests ## Alias of all-tests

bats: ## Run the Makefile tests
	$(REQUIRE_DOCKER)
	BATS_LIB_PATH="$(CURDIR)/.tools/bats/lib:/opt/bats/lib:/usr/lib/bats" $(BATS) $(BATS_ARGS) $(BATS_PATH)

# ---------------------------------------------------------------------------------------------------------------
# Coverage and mutation
# ---------------------------------------------------------------------------------------------------------------
.PHONY: tests-with-coverage coverage-xml coverage-html mutation-tests mutation-tests-diff

tests-with-coverage: ensure-deps setup-test-db ## Run the unit and integration tests with coverage and enforce 100% lines and functions
	mkdir -p target/coverage
	$(EXEC) $(COVERAGE_CLEAN_CMD)
	$(EXEC) $(TEST_ENV) $(COVERAGE_RUN_CMD)
	$(EXEC) $(COVERAGE_LCOV_CMD)
	$(EXEC) $(COVERAGE_HTML_CMD)
	$(EXEC) $(COVERAGE_GATE_CMD)

coverage-xml: ensure-deps setup-test-db ## Write target/coverage/coverage.xml (Cobertura) and lcov.info without enforcing the floor
	mkdir -p target/coverage
	$(EXEC) $(COVERAGE_CLEAN_CMD)
	$(EXEC) $(TEST_ENV) $(COVERAGE_RUN_CMD)
	$(EXEC) $(COVERAGE_XML_CMD)
	$(EXEC) $(COVERAGE_LCOV_CMD)

coverage-html: ensure-deps setup-test-db ## Write the HTML coverage report to target/coverage/html
	mkdir -p target/coverage
	$(EXEC) $(COVERAGE_CLEAN_CMD)
	$(EXEC) $(TEST_ENV) $(COVERAGE_RUN_CMD)
	$(EXEC) $(COVERAGE_HTML_CMD)

mutation-tests: ensure-deps setup-test-db ## Run mutation testing over all production code (fails on a surviving or timed-out mutant)
	$(EXEC) $(TEST_ENV) $(MUTANTS_CMD)

mutation-tests-diff: ensure-deps setup-test-db ## Run mutation testing over the code changed against base_ref
	$(EXEC) $(TEST_ENV) scripts/mutation-diff.sh $(base_ref) -- $(MUTANTS_CMD)

# ---------------------------------------------------------------------------------------------------------------
# Contracts
# ---------------------------------------------------------------------------------------------------------------
.PHONY: generate-openapi-spec generate-graphql-spec validate-contract-snapshots validate-openapi-spec openapi-diff graphql-diff schemathesis-validate

generate-openapi-spec: ## Write the OpenAPI snapshot .github/openapi-spec/spec.yaml
	$(EXEC) $(OPENAPI_EXPORT_CMD)

generate-graphql-spec: ## Write the GraphQL snapshot .github/graphql-spec/spec
	$(EXEC) $(GRAPHQL_EXPORT_CMD)

validate-contract-snapshots: generate-openapi-spec generate-graphql-spec ## Fail when a committed contract snapshot differs from the generated one
	@$(SNAPSHOT_STATUS_CMD) || { git status --short -- .github/openapi-spec .github/graphql-spec; echo 'A committed contract snapshot differs from the generated one or is not tracked.' >&2; exit 1; }

validate-openapi-spec: ## Lint the OpenAPI snapshot with Spectral
	$(REQUIRE_DOCKER)
	scripts/validate-openapi-spec.sh

openapi-diff: ## Fail on a breaking OpenAPI change against base_ref
	$(REQUIRE_DOCKER)
	scripts/openapi-diff.sh $(base_ref)

graphql-diff: ## Fail on a breaking GraphQL change against base_ref
	$(REQUIRE_DOCKER)
	scripts/graphql-diff.sh $(base_ref)

schemathesis-validate: ## Test the production image against the OpenAPI snapshot with Schemathesis
	$(REQUIRE_DOCKER)
ifeq ($(origin SCHEMATHESIS_URL),file)
	scripts/with-load-test-stack.sh -- $(SCHEMATHESIS_RUN)
else
	$(SCHEMATHESIS_RUN)
endif

# ---------------------------------------------------------------------------------------------------------------
# Load tests
# ---------------------------------------------------------------------------------------------------------------
.PHONY: smoke-load-tests average-load-tests stress-load-tests spike-load-tests load-tests execute-load-tests-script aws-load-tests aws-load-tests-cleanup lint-load-tests

smoke-load-tests: ## Run every load scenario with the smoke profile
	$(REQUIRE_DOCKER)
	scripts/with-load-test-stack.sh -- tests/Load/run-smoke-load-tests.sh

average-load-tests: ## Run every load scenario with the average profile
	$(REQUIRE_DOCKER)
	scripts/with-load-test-stack.sh -- tests/Load/run-average-load-tests.sh

stress-load-tests: ## Run every load scenario with the stress profile
	$(REQUIRE_DOCKER)
	scripts/with-load-test-stack.sh -- tests/Load/run-stress-load-tests.sh

spike-load-tests: ## Run every load scenario with the spike profile
	$(REQUIRE_DOCKER)
	scripts/with-load-test-stack.sh -- tests/Load/run-spike-load-tests.sh

load-tests: ## Run every load scenario with all four profiles
	$(REQUIRE_DOCKER)
	scripts/with-load-test-stack.sh -- tests/Load/run-load-tests.sh

execute-load-tests-script: ## Run one load scenario, all four profiles unless switched off: scenario=<name> runSmoke=false runAverage=false runStress=false runSpike=false
	@test -n "$(scenario)" || { echo 'Usage: make execute-load-tests-script scenario=<name> [runSmoke=false] [runAverage=false] [runStress=false] [runSpike=false] (each profile runs unless its variable is false)' >&2; exit 2; }
	$(REQUIRE_DOCKER)
	scripts/with-load-test-stack.sh -- tests/Load/execute-load-test.sh "$(scenario)" $(runSmoke) $(runAverage) $(runStress) $(runSpike)

aws-load-tests: ## Run the load tests on a temporary AWS instance
	$(REQUIRE_DOCKER)
	LOCAL_MODE_ENV=$(LOCAL_MODE_ENV) tests/Load/aws-execute-load-tests.sh $(AWS_OPTIONS)

aws-load-tests-cleanup: ## Remove everything aws-load-tests created
	$(REQUIRE_DOCKER)
	LOCAL_MODE_ENV=$(LOCAL_MODE_ENV) tests/Load/cleanup.sh $(AWS_OPTIONS)

lint-load-tests: ## Lint and format-check the load-test JavaScript
	$(REQUIRE_DOCKER)
	$(NODE_RUN) sh -c '$(PNPM_INSTALL) && corepack pnpm exec eslint . && corepack pnpm exec prettier --check --config /repo/.prettierrc.yml . "!results/**" "!pnpm-lock.yaml"'

# ---------------------------------------------------------------------------------------------------------------
# Database
# ---------------------------------------------------------------------------------------------------------------
.PHONY: migrate migrate-rollback create-migration setup-test-db setup-load-test-db sqlx-prepare

migrate: ensure-database ## Apply all pending migrations to the development database
	$(EXEC) cargo run --locked --quiet -p service -- migrate

migrate-rollback: ensure-database ## Revert the most recently applied migration
	$(EXEC) cargo run --locked --quiet -p service -- migrate revert

create-migration: ## Create a reversible migration pair: name=<snake_case_description>
	@test -n "$(name)" || { echo 'Usage: make create-migration name=<snake_case_description>' >&2; exit 2; }
	$(EXEC) sqlx migrate add -r $(name)

setup-test-db: ensure-database ## Drop, create and migrate the test database
	$(EXEC) $(TEST_ENV) scripts/setup-test-db.sh

setup-load-test-db: ## Drop, create and migrate the database of the isolated load-test environment
	$(REQUIRE_DOCKER)
	scripts/setup-load-test-db.sh

sqlx-prepare: setup-test-db ## Regenerate the offline query metadata in .sqlx from the freshly migrated test database
	$(EXEC) $(TEST_ENV) cargo sqlx prepare --workspace -- --all-targets --all-features

# ---------------------------------------------------------------------------------------------------------------
# Security
# ---------------------------------------------------------------------------------------------------------------
.PHONY: check-security check-security-offline secrets-scan image-scan

check-security: ## Check advisories, licences, bans and sources of the locked dependencies
	$(EXEC) $(DENY_CMD)

check-security-offline: ## Check licences, bans and sources without network access
	$(EXEC) $(DENY_OFFLINE_CMD)

secrets-scan: ## Scan the history and the working files for secrets
	$(REQUIRE_DOCKER)
	scripts/secrets-scan.sh

image-scan: ## Scan the production image for HIGH and CRITICAL vulnerabilities
	$(REQUIRE_DOCKER)
	$(if $(IMAGE_TO_SCAN),,$(MAKE) --no-print-directory build-prod-image)
	scripts/image-scan.sh "$(or $(IMAGE_TO_SCAN),$(APP_IMAGE))"

# ---------------------------------------------------------------------------------------------------------------
# Docs and architecture model
# ---------------------------------------------------------------------------------------------------------------
.PHONY: docs-check validate-architecture-model

docs-check: ## Check that commands, paths and lists cited in the documentation exist
	mkdir -p target/reports/docs-check
	$(EXEC) cargo run --locked --quiet -p service -- --help > target/reports/docs-check/root.txt
	$(EXEC) cargo run --locked --quiet -p service -- serve --help > target/reports/docs-check/serve.txt
	$(EXEC) cargo run --locked --quiet -p service -- migrate --help > target/reports/docs-check/migrate.txt
	$(EXEC) cargo run --locked --quiet -p service -- healthcheck --help > target/reports/docs-check/healthcheck.txt
	scripts/check-docs.sh --service-help target/reports/docs-check

validate-architecture-model: ## Validate workspace.dsl, its three views, its components and a relationship for every dependency between them
	$(REQUIRE_DOCKER)
	$(EXEC) cargo tree --locked --workspace --depth 0 --prefix none | scripts/validate-architecture-model.sh workspace.dsl -

# ---------------------------------------------------------------------------------------------------------------
# Repository lint
# ---------------------------------------------------------------------------------------------------------------
.PHONY: lint-repo lint-actions lint-yaml lint-markdown lint-shell lint-docker lint-toml lint-spelling lint-prettier validate-invariants

lint-repo: lint-actions lint-yaml lint-markdown lint-shell lint-docker lint-toml lint-spelling lint-prettier ## Run every repository linter

lint-actions: ## Lint the workflows (syntax and security)
	$(REQUIRE_DOCKER)
	$(DOCKER_RUN) -v "$(CURDIR):/repo:ro" -w /repo $(ACTIONLINT_IMAGE) -color
	$(EXEC) zizmor --offline --config .github/zizmor.yml .github/workflows

lint-yaml: ## Lint the YAML files
	$(REQUIRE_DOCKER)
	$(DOCKER_RUN) -e HOME=/tmp -v "$(CURDIR):/repo:ro" -w /repo $(PYTHON_IMAGE) sh -c 'pip install --quiet --disable-pip-version-check --no-cache-dir --target /tmp/yamllint yamllint==$(YAMLLINT_VERSION) && PYTHONPATH=/tmp/yamllint python -m yamllint --strict .'

lint-markdown: ## Lint the Markdown files
	$(REQUIRE_DOCKER)
	$(DOCKER_RUN) -v "$(CURDIR):/workdir:ro" $(MARKDOWNLINT_IMAGE) "**/*.md"

lint-shell: ## Lint shell scripts, hooks and Bats files
	$(REQUIRE_DOCKER)
	$(DOCKER_RUN) -v "$(CURDIR):/mnt:ro" -w /mnt $(SHELLCHECK_IMAGE) --severity=style $$(git ls-files --cached --others --exclude-standard -- '*.sh' '*.bash' '*.bats' '.githooks/*')

lint-docker: ## Lint the Dockerfile
	$(REQUIRE_DOCKER)
	$(DOCKER_RUN) -v "$(CURDIR):/repo:ro" -w /repo $(HADOLINT_IMAGE) hadolint --config .hadolint.yaml $(DOCKERFILES)

lint-toml: ## Check the TOML formatting
	$(EXEC) taplo fmt --check

lint-spelling: ## Check spelling
	$(EXEC) typos

lint-prettier: ## Check the formatting of YAML and JSON
	$(REQUIRE_DOCKER)
	$(NODE_RUN) sh -c '$(PNPM_INSTALL) && corepack pnpm exec prettier --check $(PRETTIER_REPO_ARGS)'

validate-invariants: ## Check the repository invariants: pins, versions, gate set, repository name, licence, suppressions, execution switch
	scripts/check-invariants.sh

# ---------------------------------------------------------------------------------------------------------------
# Hooks and tooling
# ---------------------------------------------------------------------------------------------------------------
.PHONY: install-hooks pre-commit pre-push check-commit-message

install-hooks: ## Activate the git hooks of .githooks
	git config core.hooksPath .githooks

pre-commit: format-check lint-code architecture check-security-offline unused-dependencies ## Fast gates run by the pre-commit hook

pre-push: unit-tests ## Gates run by the pre-push hook

check-commit-message: ## Check the commit message in COMMIT_MSG_FILE or the title in COMMIT_TITLE
	scripts/check-commit-message.sh

# ---------------------------------------------------------------------------------------------------------------
# bmalph, scaffolding and aggregates
# ---------------------------------------------------------------------------------------------------------------
.PHONY: bmalph-install bmalph-claude bmalph-codex bmalph-init bmalph-setup new-context ci all ci-sequential ci-list ci-preflight ci-static-analysis ci-tests ci-contracts ci-mutation ci-delivery ci-cli

bmalph-install: ## Install and verify bmalph for BMALPH_PLATFORM=claude-code|codex
	bash scripts/install-bmalph.sh --platform "$(BMALPH_PLATFORM)"

bmalph-claude: ## Install and verify bmalph for Claude Code
	@$(MAKE) --no-print-directory bmalph-install BMALPH_PLATFORM=claude-code

bmalph-codex: ## Install and verify bmalph for Codex
	@$(MAKE) --no-print-directory bmalph-install BMALPH_PLATFORM=codex

bmalph-init: ## Initialise bmalph in this project; BMALPH_DRY_RUN=true previews
	bash scripts/install-bmalph.sh --platform "$(BMALPH_PLATFORM)" --init $(if $(filter true TRUE 1 yes YES,$(BMALPH_DRY_RUN)),--dry-run,)

bmalph-setup: ## Install and initialise bmalph in one step
	@$(MAKE) --no-print-directory bmalph-init BMALPH_PLATFORM="$(BMALPH_PLATFORM)" BMALPH_DRY_RUN="$(BMALPH_DRY_RUN)"

new-context: ## Create a bounded context: group=<group> context=<context>
	@test -n "$(group)" -a -n "$(context)" || { echo 'Usage: make new-context group=<group> context=<context>' >&2; exit 2; }
	$(EXEC) cargo xtask new-context $(group) $(context)

ci: ## Run every gate in order and print the success line
	@$(MAKE) --no-print-directory doctor
	@for stage in $(CI_STAGES); do $(MAKE) --no-print-directory $$stage; done
	@echo "CI checks successfully passed!"

all: ci ## Alias of ci

ci-sequential: ci ## Alias of ci

ci-list: ## Print the ordered gate targets of make ci, one per line
	@printf '%s\n' $(CI_GATES)

ci-preflight: ## Stage 1 of make ci: configuration, invariants, secrets, repository lint, model
	$(call STAGE_RECIPE,$(CI_PREFLIGHT_TARGETS))

ci-static-analysis: ## Stage 2 of make ci: formatting, manifest, analysis, complexity, security, docs
	$(call STAGE_RECIPE,$(CI_STATIC_ANALYSIS_TARGETS))

ci-tests: ## Stage 3 of make ci: test database, query metadata, coverage, doc tests, end-to-end tests
	$(call STAGE_RECIPE,$(CI_TESTS_TARGETS))

ci-contracts: ## Stage 4 of make ci: contract snapshots, OpenAPI lint and diff, GraphQL diff
	$(call STAGE_RECIPE,$(CI_CONTRACTS_TARGETS))

ci-mutation: ## Stage 5 of make ci: mutation testing of the changed code
	$(call STAGE_RECIPE,$(CI_MUTATION_TARGETS))

ci-delivery: ## Stage 6 of make ci: production image, image scan, Schemathesis, smoke load tests
	$(call STAGE_RECIPE,$(CI_DELIVERY_TARGETS))

ci-cli: ## Stage 7 of make ci: the Makefile tests
	$(call STAGE_RECIPE,$(CI_CLI_TARGETS))

-include make/service.mk
