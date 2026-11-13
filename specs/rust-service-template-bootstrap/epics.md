---
stepsCompleted:
  - step-01-validate-prerequisites
  - step-02-design-epics
  - step-03-create-stories
  - step-04-final-validation
inputDocuments:
  - _bmad-output/planning-artifacts/scope-decisions.md
  - _bmad-output/planning-artifacts/prd.md
  - _bmad-output/planning-artifacts/architecture.md
  - _bmad-output/planning-artifacts/architecture-delivery.md
  - _bmad-output/planning-artifacts/research.md
  - _bmad-output/planning-artifacts/research/capability-catalog.md
  - _bmad-output/planning-artifacts/product-brief.md
---

# rust-service-template - Epic Breakdown

## Overview

This document breaks the first release of `rust-service-template` into 10 epics and 57 stories. It decomposes the final PRD (FR1 to FR126, NFR1 to NFR47, gates G1 to G31), the core architecture (`architecture.md`) and the delivery architecture (`architecture-delivery.md`), both in their revision after the validation round, into units that one implementing agent completes in one session and one independent verifying agent checks by running the story's `Verify` commands and acceptance criteria (scope-decisions section 9).

How to read a story:

- The user story states who benefits and why. The metadata lines that follow are part of the story description: `Track` (A service code, B `xtask`, C delivery tooling, D guidance and documentation, E integration), `Depends on` (stories that must be complete first; never a later story), `Owns` (paths the story creates; every file of the two architecture documents is owned by exactly one story), `Touches` (existing paths the story modifies, with the reason), `Implements` (PRD ids), `Reference` (the section headings and decision ids of the architecture documents that are normative for the story), `Pitfalls` (ids of the research checklist PF-01 to PF-66 that apply) and `Verify` (the commands whose success proves the story).
- A file that a later revision of the architecture adds to a crate or directory belongs to the story that owns the neighbouring files of the same module (for example a new `tests/integration/support.rs` belongs to the story that owns the other integration tests of that crate), unless the revision names another owner.
- The architecture documents are the source of names, signatures, command lines and file content. A story points to the section by its heading and does not copy it; the implementer reads the section in its current revision. Where a story states a behaviour, the PRD and `scope-decisions.md` are its source.
- A story is done when its `Verify` commands and every acceptance criterion pass, including the format check and both lint passes for the crates it touches, not when the code compiles. Where a criterion says that a Make target fails with "the script's exit status 2", the script exits 2 and Make reports the failure with its own non-zero status.
- Test strength is proven at the story. Every code story ships the unit tests the architecture lists for its files (and the integration tests where it lists them). A story of Epics 2, 4, 5, 6 and 7 that owns production source ends its `Verify` line with a coverage command and a mutation command for its package (behind `scripts/with-env.sh .env .env.test --` where the package has tests that need the backing services), and its last criterion requires 100% lines and functions and no surviving or timed-out mutant for the production files it owns. While iterating, the implementer may narrow the mutation run to one file with `-f <path>`; the verifier runs the line as written. The test-support and end-to-end crates are test code and have no such commands. Stories 9.2 and 9.3 close what only the complete workspace shows; they do not write the test suite.
- Workflow files under `.github/workflows/` are added in the last epic. Until Story 10.1 no `Verify` line contains `lint-actions` or `lint-repo` (zizmor cannot run on a missing or empty workflow directory); the other seven `lint-*` targets are run by name. The full local gate run (Story 10.4) and the clean-clone proof (Story 10.5) follow the workflow files. Handed-over stories of Epics 1 to 9 are pushed as they land, because no workflow exists yet; from the first workflow file (Story 10.1) nothing is pushed until Story 10.5 is verified.
- Implementation environment. The implementation machine reaches the network through a TLS-intercepting proxy. This is handled by environment preparation, which the orchestrator provides before Story 1.4 starts, and is never a reason to change a tracked file. (a) Images are built with `docker build --build-context <builder reference of the Dockerfile>=docker-image://<builder image that carries the proxy CA> --target dev|prod --tag <image name> .` (architecture-delivery.md DD-20), and every target that would build is then run with `BUILD_IMAGE=0`. Where a `Verify` line says `make build-prod-image` or `docker compose build app`, it means this command. (b) Targets whose tool container downloads at run time (`lint-yaml`, `lint-prettier`, `lint-load-tests`, `format-repo`, `graphql-diff`, `image-scan`) are run with the `DOCKER` wrapper of the orchestrator, which adds the proxy variables and the CA to `docker run`; `DOCKER` is a caller-settable variable of the Makefile. (c) The development container downloads crates when it first compiles (`make start` in Stories 9.11 and 10.5): its image is the CA-carrying one of (a), and the orchestrator confirms before Story 9.11 that the proxy variables reach compose containers. (d) A failure with a certificate error is reported as an environment finding, not fixed in the repository.
- Commit protocol. A story is handed over as one commit with a subject in the VilnaCRM form (`<type>(#1): story N.M <short title>`) that contains exactly the paths of its `Owns` and `Touches` lines (`git add -- <paths>`, then `git commit -- <paths>`, which commits these paths only). Each story has its own git worktree and therefore its own index (see "Rules for concurrent execution"). No git hook is active in any implementation worktree (`core.hooksPath` is shared by all worktrees and stays unset): the pre-commit hook runs workspace-wide gates, and `unused-dependencies` reports the declared dependencies of every crate that is still a stub, until Story 7.7. The verifier runs the `Verify` line on that commit in the story's worktree, right after the story is implemented. A criterion that works in a temporary clone or a temporary git repository uses the story's commit; while implementing, the story's uncommitted files are copied into it. In a criterion, a statement about `git status --porcelain` means that its output is the same before and after the commands of the criterion, unless the criterion says "on a committed tree".
- Deviations. When a normative listing, name or command of an architecture document does not work as written, the implementer changes the smallest thing that makes the story's criteria pass, never a threshold or a protected value, and appends an entry to `_bmad-output/implementation-artifacts/deviations.md`: story id, document and heading, old text, new text, and the command output that proves the need. The verifier checks every entry; the orchestrator applies the entries to the architecture documents and to `specs/` before the last story. Listing equality is checked mechanically: the verifier extracts the fenced block under the named heading and runs `diff` against the file; every difference must have a deviation entry.

## Requirements Inventory

### Functional Requirements

- FR1: One process, plain HTTP on one port, mode from `APP_ENV`
- FR2: Service commands: serve, `migrate`, rollback, `check-config`, exports, liveness probe
- FR3: Deploy settings from environment variables; no committed secret
- FR4: Fail-fast typed configuration validation and `check-config`
- FR5: One composition root with replaceable ports
- FR6: `/api` prefix, JSON, problem documents for every error status
- FR7: Input validation with all violations in one 422
- FR8: API documentation page at `GET /api/docs` from own assets
- FR9: GraphQL endpoint, transport errors, explorer in `dev` only
- FR10: JSON logs, start-up and access entries, request id
- FR11: Four security headers on every response
- FR12: Graceful shutdown on SIGTERM with a grace period
- FR13: Explicit timeouts, retry limit, request budget, body limit
- FR14: `GET /api/health` returns 204 when all probes succeed
- FR15: Health failure is a 500 problem naming every failing dependency
- FR16: Three probes: `SELECT 1`, cache write and read-back, read-only SQS call
- FR17: Concurrent probes with per-probe timeout; pluggable probes
- FR18: `GET /ping` returns 200 `pong` without dependencies
- FR19: Create `ExampleApiResource` with `POST`, 201 and `Location`
- FR20: Create is the command-path exemplar (validation, bus, aggregate, event)
- FR21: Read one resource by id through the query bus
- FR22: List one bounded page in a total order by id
- FR23: Audit subscriber logs each created resource
- FR24: GraphQL schema with exactly three operations
- FR25: Aggregate root records domain events; event contract
- FR26: In-process event bus delivers to every subscriber
- FR27: In-process command and query buses, one handler per message
- FR28: Explicit registration; duplicate handler stops start-up
- FR29: UUID value object and generator
- FR30: PostgreSQL through `DATABASE_URL`; separate test databases
- FR31: Repository port in Domain, PostgreSQL adapter in Infrastructure
- FR32: Versioned reversible migrations; explicit `migrate`
- FR33: Rollback of the most recent migration
- FR34: Cache port with an in-process adapter
- FR35: SQS client configured from environment
- FR36: Three contexts, each split into three layers
- FR37: Framework-free Domain layer
- FR38: Architecture gate with structural rules
- FR39: Scaffolding command for a bounded context
- FR40: Formatting check and separate fix target
- FR41: Strict static analysis over production and test code
- FR42: One fix target for machine-applicable fixes
- FR43: Repository lint gate for non-Rust files and spelling
- FR44: Protected-configuration guard with approval label
- FR45: Lock file enforced on every command; manifest validation
- FR46: Dependency policy: advisories, licences, sources
- FR47: Secrets-scanning gate
- FR48: Production-image scan gate
- FR49: CodeQL analysis and SARIF upload as CI-only publications
- FR50: Unit suite without services; ports replaced by doubles
- FR51: Seeded random test data with printed seed
- FR52: Integration suite against real PostgreSQL and LocalStack
- FR53: `setup-test-db` and `setup-load-test-db`
- FR54: Unit and integration tests for each health probe
- FR55: Gherkin end-to-end target with the PHP health feature
- FR56: Fault injection by failing port implementation
- FR57: Scenarios for docs page, example resource and GraphQL
- FR58: Coverage gate, reports, one coverage job
- FR59: Mutation gate with scope record and changed-code mode
- FR60: `make` runs `help`, which lists every target
- FR61: Makefile is the single command surface
- FR62: 42 PHP target names kept
- FR63: Capability-named targets for tool-named PHP targets
- FR64: Targets set their own configuration; no env file import
- FR65: Toolchain and orchestration target classes; execution switch
- FR66: `make ci` runs every gate in a fixed order
- FR67: Bats suite for the command surface and negative tests
- FR68: Commands write only to git-ignored paths
- FR69: `make doctor` prerequisite check
- FR70: `make bootstrap` prepares a fresh clone
- FR71: Code-first OpenAPI 3.1.0 document with full metadata
- FR72: Export targets write both snapshots without a stack
- FR73: Snapshot equality gate; stable `info.version`
- FR74: Spectral lint with the PHP ruleset
- FR75: OpenAPI and GraphQL breaking-change gates with approval label
- FR76: Schemathesis against the production image in an isolated stack
- FR77: Isolated load-test stack on the production image
- FR78: Four load profiles; scenario `health`; smoke is the gate
- FR79: Load settings file, local override, scenario discovery
- FR80: p99 budgets and check rate; invalid threshold rejected
- FR81: `execute-load-tests-script` for one scenario
- FR82: HTML report per run; CI artifact
- FR83: Load-script lint gate
- FR84: Smoke profile on every pull request
- FR85: AWS load runner with bounded waits and LocalStack dry run
- FR86: One multi-stage image definition (development, production)
- FR87: Minimal non-root production image with binary health check
- FR88: Four compose files; `make start` boots a healthy stack
- FR89: Rebuild on source change in the development stack
- FR90: Ports and credentials from variables with local defaults
- FR91: Hardened production compose reference
- FR92: Names derive from one project-name variable
- FR93: Each gate separately attributable in CI
- FR94: Documentation-only pull requests skip heavy gates
- FR95: Scheduled workflow with clean-clone verification
- FR96: No bot pushes; fork and Dependabot safe; clean skip without secrets
- FR97: CodeRabbit configuration
- FR98: Automated release from conventional commits
- FR99: Dependabot for six ecosystems
- FR100: Two template-sync workflows with ignore list and self-guard
- FR101: Setup guides for release and template sync
- FR102: Commit-message hook and pull-request title check
- FR103: Committed git hooks that call Make targets
- FR104: `AGENTS.md` with the canonical verification list
- FR105: `CLAUDE.md` delegates to `AGENTS.md`
- FR106: Skill index, usage guide and decision guide
- FR107: Skill file format; no repository name
- FR108: Ten capability skills plus the planning skill
- FR109: Working rules backed by requirements
- FR110: Documentation-drift check
- FR111: Assistant settings file with protected-file guard
- FR112: BMAD and bmalph integration; planning bundle
- FR113: `README.md`
- FR114: `CONTRIBUTING.md`
- FR115: `SECURITY.md`
- FR116: Issue and pull-request templates
- FR117: Architecture, troubleshooting, reference and operations documents
- FR118: `.editorconfig`, dev container, request collection
- FR119: `workspace.dsl` with three views
- FR120: Architecture-model validation gate
- FR121: Committed query metadata and its gate
- FR122: Crate-isolation gate
- FR123: Complexity gate
- FR124: Unused-dependency gate
- FR125: Repository-invariants gate
- FR126: Start-up without reachable dependencies

### NonFunctional Requirements

- NFR1: K6 load shapes and p99 budgets
- NFR2: Health response time bounded by the probe timeout
- NFR3: `/ping` within 5 s of start; container health-check timing
- NFR4: Explicit timeout on every outbound call
- NFR5: Shutdown timing and exit statuses
- NFR6: Statelessness between requests
- NFR7: Deterministic contract generation
- NFR8: No workflow failure from absent secrets
- NFR9: Dependency gates report zero findings
- NFR10: Zero secret-shaped values committed
- NFR11: Zero HIGH or CRITICAL image findings
- NFR12: Hardened production container, image at most 100 MiB
- NFR13: Pins for actions, images, tools and the toolchain
- NFR14: Workflow permissions declared and minimal
- NFR15: Headers and request id on every response; no internal detail
- NFR16: Formatting and static analysis at zero findings; no blanket suppression
- NFR17: Complexity limits
- NFR18: Architecture, isolation, scaffolding and model at zero differences
- NFR19: 100% line and function coverage of production source
- NFR20: Zero surviving mutants
- NFR21: A negative test for every gate and pass condition
- NFR22: `make ci` and pull-request gates differ by zero unexplained items
- NFR23: No dead scaffolding
- NFR24: Contract gates at zero findings
- NFR25: Repository and load-script lint at zero findings
- NFR26: Exactly one protected list
- NFR27: Parity matrix in the planning bundle
- NFR28: Unit suite without network or wall clock
- NFR29: Order-independent, isolated, seeded tests
- NFR30: Health feature 4 of 4; seven failure combinations
- NFR31: Every serve output line is JSON with request id
- NFR32: Migration round trip and query metadata at zero differences
- NFR33: Bounded database readiness and wait
- NFR34: Clean checkout reaches green without manual steps
- NFR35: Time budgets for bootstrap, `make ci` and `make start`
- NFR36: Execution switch honoured by every toolchain target
- NFR37: Host prerequisites and supported platforms
- NFR38: Repository name only in its three definitions
- NFR39: CI time budgets and job timeouts
- NFR40: Cancellation and caching in CI
- NFR41: One coverage computation; no duplicated runs
- NFR42: Weekly scheduled run; monthly Dependabot minimum
- NFR43: No dangling command or path; one verification list
- NFR44: Skill metadata and one skill inventory
- NFR45: Complete environment and Make-target references
- NFR46: Journey documents in the repository; no placeholder
- NFR47: Exactly one licence identifier

### Gates

- G1: Formatting check
- G2: Static analysis
- G3: Complexity limits
- G4: Architecture boundaries
- G5: Manifest and lock validation
- G6: Configuration check
- G7: Dependency policy
- G8: Unused dependencies
- G9: Secrets scanning
- G10: Production-image scan
- G11: Unit tests with documentation tests
- G12: Integration tests
- G13: End-to-end tests (Gherkin)
- G14: Coverage
- G15: Mutation testing
- G16: Bats tests of the Makefile
- G17: Contract snapshot equality
- G18: OpenAPI lint
- G19: OpenAPI breaking-change check
- G20: GraphQL schema diff
- G21: Schemathesis
- G22: K6 smoke load
- G23: Repository lint
- G24: Load-script lint
- G25: Protected-configuration guard
- G26: Documentation-drift check
- G27: Architecture-model validation
- G28: Pull-request title convention
- G29: Crate isolation
- G30: Persistence query metadata
- G31: Repository invariants

### Additional Requirements

- No community starter template is used (architecture "Starter Template Evaluation"): Story 1.1 creates the workspace by hand from the architecture.
- No UX design document exists: the product has no user interface beyond the API documentation page and the development-only GraphQL explorer, both covered by FR8 and FR9.
- Corrections of the architecture validation round that the stories apply: a create whose event publication fails after the save returns 201 and logs the failure; a forced end of the shutdown grace period exits non-zero; coverage counts the unit and integration suites only; every port trait has `Debug` as a supertrait; tests that spawn a binary forward `LLVM_PROFILE_FILE`; mutation runs unset `CARGO_BUILD_WARNINGS` and the shard count scales with the mutant count; the development stack creates no root-owned `./target`; the scaffolder refreshes the lock file; `service --help` hides environment values.
- Corrections of the two architecture revisions that this version of the stories applies: no 409 and only two GraphQL error codes; page numbers without an upper bound; one K6 scenario (`health`) with the NFR1 ceilings as budgets; six Bats groups in eighteen files and no second passing run of a gate in the suite; `scripts/ci/mutation-plan.sh` with one shard per 150 mutants; the seeds `TEST_SEED` and `PROPTEST_RNG_SEED` recorded in `.cargo/config.toml`; the build directory of the development image at `/cargo-target`; the read-only static-analysis job with a separate upload job; no end-to-end JUnit file, no `migrate status`, no `docs/php-to-rust.md` and no `scripts/wait-for-service.sh`.
- The stories are scheduled with one git worktree per story; section "Implementation Order and Parallel Tracks" states the waves, the rules for concurrent execution and the exclusive resources.
- Model allocation (scope-decisions section 9): each story is implemented by one agent in one session and verified by an independent agent that runs the acceptance criteria.

### FR Coverage Map

Every functional requirement, non-functional requirement and gate with the stories that implement it. A gate row names, in order of story id, the stories in which the gate first passes, the Bats story that adds its Make-level negative test, and the workflow story that adds its CI check.

Functional requirements:

| Id | Stories |
| --- | --- |
| FR1 | 7.6 |
| FR2 | 7.1, 7.2, 7.4, 7.6 |
| FR3 | 7.1 |
| FR4 | 7.1 |
| FR5 | 7.5 |
| FR6 | 4.3, 7.5 |
| FR7 | 4.3, 6.3 |
| FR8 | 7.4 |
| FR9 | 6.3, 7.4, 7.5 |
| FR10 | 7.3, 7.6 |
| FR11 | 7.3, 7.4, 7.5 |
| FR12 | 7.6 |
| FR13 | 4.3, 4.5, 5.2, 7.1, 7.5 |
| FR14 | 5.1 |
| FR15 | 5.1 |
| FR16 | 5.2 |
| FR17 | 5.1, 7.5 |
| FR18 | 7.3, 7.4 |
| FR19 | 6.3 |
| FR20 | 6.1, 6.2, 6.3 |
| FR21 | 6.2, 6.3 |
| FR22 | 4.1, 6.2, 6.3 |
| FR23 | 6.2 |
| FR24 | 6.3, 7.4 |
| FR25 | 4.1, 6.1 |
| FR26 | 4.4 |
| FR27 | 4.1, 4.4, 6.2 |
| FR28 | 4.4, 7.5 |
| FR29 | 4.1, 4.4 |
| FR30 | 4.5 |
| FR31 | 6.1, 6.4 |
| FR32 | 3.1, 4.5, 7.2 |
| FR33 | 4.5, 7.2 |
| FR34 | 4.1, 4.4 |
| FR35 | 4.5 |
| FR36 | 1.1 |
| FR37 | 1.1, 2.1, 2.2, 4.1, 6.1 |
| FR38 | 2.1, 2.2 |
| FR39 | 2.4, 9.6 |
| FR40 | 1.2 |
| FR41 | 1.1 |
| FR42 | 1.2 |
| FR43 | 1.4, 9.10 |
| FR44 | 2.3, 10.2 |
| FR45 | 1.1, 1.2 |
| FR46 | 1.3 |
| FR47 | 3.3 |
| FR48 | 3.3 |
| FR49 | 10.1, 10.2 |
| FR50 | 4.1, 4.2, 6.1, 9.2 |
| FR51 | 4.2, 10.3 |
| FR52 | 4.5, 9.2 |
| FR53 | 4.5, 9.1, 9.6 |
| FR54 | 5.1, 5.2 |
| FR55 | 7.7 |
| FR56 | 7.7 |
| FR57 | 7.7 |
| FR58 | 9.2, 10.1 |
| FR59 | 9.3, 9.6 |
| FR60 | 1.2, 9.4 |
| FR61 | 1.2 |
| FR62 | 1.2 |
| FR63 | 1.2 |
| FR64 | 1.2 |
| FR65 | 1.2, 9.11 |
| FR66 | 1.2, 10.4, 10.5 |
| FR67 | 9.4, 9.5, 9.6, 9.7, 9.10, 9.11, 10.4 |
| FR68 | 10.4, 10.5 |
| FR69 | 1.2, 1.4 |
| FR70 | 1.2, 9.4, 10.5 |
| FR71 | 5.1, 6.3, 7.4 |
| FR72 | 7.4 |
| FR73 | 7.4 |
| FR74 | 3.2, 7.4 |
| FR75 | 3.2, 10.2 |
| FR76 | 9.1 |
| FR77 | 9.1 |
| FR78 | 3.4 |
| FR79 | 3.4 |
| FR80 | 3.4 |
| FR81 | 3.4 |
| FR82 | 3.4, 10.2 |
| FR83 | 1.4, 3.4 |
| FR84 | 9.1, 10.2 |
| FR85 | 3.5, 9.7 |
| FR86 | 3.1 |
| FR87 | 3.1, 7.6 |
| FR88 | 3.1, 9.11 |
| FR89 | 3.1, 9.11 |
| FR90 | 1.1, 3.1 |
| FR91 | 3.1, 9.7 |
| FR92 | 1.1, 1.2, 9.8 |
| FR93 | 10.1, 10.2, 10.5 |
| FR94 | 10.1, 10.2 |
| FR95 | 10.3 |
| FR96 | 10.1, 10.2, 10.3 |
| FR97 | 8.4 |
| FR98 | 3.6, 10.3 |
| FR99 | 10.3 |
| FR100 | 10.3 |
| FR101 | 8.7 |
| FR102 | 3.6, 10.2 |
| FR103 | 3.6 |
| FR104 | 8.1 |
| FR105 | 8.1 |
| FR106 | 8.2 |
| FR107 | 8.2 |
| FR108 | 8.2 |
| FR109 | 8.1 |
| FR110 | 9.9 |
| FR111 | 8.1, 9.4 |
| FR112 | 8.3, 8.8 |
| FR113 | 8.4 |
| FR114 | 8.4 |
| FR115 | 8.4 |
| FR116 | 8.4 |
| FR117 | 8.5, 8.6, 8.7 |
| FR118 | 1.1, 3.1, 8.5, 10.3 |
| FR119 | 3.7 |
| FR120 | 3.7, 9.10 |
| FR121 | 1.1, 6.4 |
| FR122 | 1.3 |
| FR123 | 1.1 |
| FR124 | 9.5 |
| FR125 | 9.8, 10.5 |
| FR126 | 4.5, 7.6 |

Non-functional requirements:

| Id | Stories |
| --- | --- |
| NFR1 | 3.4, 8.7, 9.1, 10.5 |
| NFR2 | 5.1, 5.2, 7.6 |
| NFR3 | 3.1, 7.6, 9.7 |
| NFR4 | 4.5, 5.2, 6.4, 7.1 |
| NFR5 | 7.6 |
| NFR6 | 7.5, 7.7, 8.6 |
| NFR7 | 7.4, 10.3 |
| NFR8 | 10.1, 10.2, 10.3 |
| NFR9 | 1.1, 1.3 |
| NFR10 | 1.1, 3.3 |
| NFR11 | 3.3 |
| NFR12 | 3.1, 9.7 |
| NFR13 | 1.1, 1.3, 3.1, 7.4, 9.8, 10.1, 10.2, 10.3, 10.5 |
| NFR14 | 9.8, 10.1, 10.2, 10.3, 10.5 |
| NFR15 | 4.3, 7.3, 7.5 |
| NFR16 | 1.1, 1.2, 9.8 |
| NFR17 | 1.1, 1.2 |
| NFR18 | 1.3, 2.1, 2.2, 2.4, 3.7, 9.6 |
| NFR19 | 9.2 |
| NFR20 | 9.3, 10.1 |
| NFR21 | 2.1, 2.2, 2.3, 9.4, 9.5, 9.6, 9.7, 9.10, 10.4 |
| NFR22 | 9.8, 10.4, 10.5 |
| NFR23 | 9.5, 10.4 |
| NFR24 | 3.2, 7.4, 9.1 |
| NFR25 | 1.4, 3.4, 9.10 |
| NFR26 | 1.1, 2.3, 8.1 |
| NFR27 | 8.8, 10.5 |
| NFR28 | 1.1, 4.4, 9.5 |
| NFR29 | 1.1, 4.2, 6.4 |
| NFR30 | 5.1, 7.7 |
| NFR31 | 7.6 |
| NFR32 | 4.5, 6.4, 7.2 |
| NFR33 | 3.1, 7.2, 9.11 |
| NFR34 | 10.3, 10.5 |
| NFR35 | 10.4, 10.5 |
| NFR36 | 7.1, 9.8, 9.11 |
| NFR37 | 1.2, 1.3, 3.1, 9.4 |
| NFR38 | 1.4, 9.8, 9.11, 10.3 |
| NFR39 | 10.1, 10.2, 10.5 |
| NFR40 | 3.1, 10.1, 10.2 |
| NFR41 | 1.2, 9.2, 10.1, 10.4 |
| NFR42 | 10.3 |
| NFR43 | 8.1, 9.9 |
| NFR44 | 8.2, 9.9 |
| NFR45 | 8.5, 9.9 |
| NFR46 | 8.4, 8.5, 8.6, 8.7 |
| NFR47 | 1.1, 7.4, 8.4, 9.8 |

Gates:

| Id | Stories |
| --- | --- |
| G1 | 1.2, 9.5, 10.1 |
| G2 | 1.2, 9.5, 10.1 |
| G3 | 1.2, 9.5, 10.1 |
| G4 | 2.1, 2.2, 9.5, 10.1 |
| G5 | 1.2, 9.5, 10.1 |
| G6 | 7.1, 9.5, 10.1 |
| G7 | 1.3, 9.5, 10.1 |
| G8 | 9.5, 10.1 |
| G9 | 3.3, 9.7, 10.2 |
| G10 | 3.3, 9.7, 10.2 |
| G11 | 4.1, 9.2, 9.6, 10.1 |
| G12 | 4.5, 9.2, 9.6, 10.1 |
| G13 | 7.7, 9.6, 10.1 |
| G14 | 9.2, 9.6, 10.1 |
| G15 | 9.3, 9.6, 10.1 |
| G16 | 9.4, 9.11, 10.1, 10.4 |
| G17 | 7.4, 9.7, 10.2 |
| G18 | 3.2, 9.7, 10.2 |
| G19 | 3.2, 9.7, 10.2 |
| G20 | 3.2, 9.7, 10.2 |
| G21 | 9.1, 9.7, 10.2 |
| G22 | 9.1, 9.7, 10.2 |
| G23 | 1.4, 9.10, 10.2, 10.4 |
| G24 | 3.4, 9.7, 10.2 |
| G25 | 2.3, 9.5, 10.2 |
| G26 | 9.9, 9.10, 10.2 |
| G27 | 3.7, 9.10, 10.2 |
| G28 | 3.6, 9.4, 10.2 |
| G29 | 1.3, 9.5, 10.1 |
| G30 | 6.4, 9.6, 10.1 |
| G31 | 9.8, 9.10, 10.2, 10.4, 10.5 |

## Epic List

- **Epic 1: Workspace Foundation and Command Surface** (Stories 1.1 to 1.4). A clone has every crate, the pinned toolchain and tool versions, the lint policy, one Make command per task, a preflight, the tool installers and repository linters. Requirements: FR36, FR37, FR40 to FR43, FR45, FR46, FR60 to FR66, FR69, FR122, FR123.
- **Epic 2: Architecture Enforcement and Scaffolding** (Stories 2.1 to 2.4). Structural boundary rules, the protected-configuration guard and one command for a new bounded context. Requirements: FR38, FR39, FR44.
- **Epic 3: Images, Local Stacks and Delivery Gate Tooling** (Stories 3.1 to 3.7). Images and compose stacks, contract checks, scans, the K6 kit with its AWS runner, commit convention with hooks and release tooling, the architecture model. Requirements: FR47, FR48, FR74, FR75, FR78 to FR83, FR85 to FR91, FR98, FR102, FR103, FR119, FR120.
- **Epic 4: Shared Kernel and Platform Adapters** (Stories 4.1 to 4.5). Shared domain kernel, test doubles, problem documents, in-process buses, cache, PostgreSQL pool with migrations, SQS client. Requirements: FR6, FR7, FR13, FR25 to FR30, FR32 to FR35, FR50 to FR53.
- **Epic 5: Dependency Health Check** (Stories 5.1 and 5.2). Health orchestration, the health handler and three real probes. Requirements: FR14 to FR17, FR54.
- **Epic 6: Example Bounded Context** (Stories 6.1 to 6.4). The `ExampleApiResource` slice: domain, use cases, REST and GraphQL adapters, PostgreSQL repository. Requirements: FR19 to FR24, FR31, FR121.
- **Epic 7: Service Runtime and Composition Root** (Stories 7.1 to 7.7). The binary and its commands, configuration, HTTP surface, contract snapshots, composition root, graceful shutdown, an early conformance run and the Gherkin suite. Requirements: FR1 to FR5, FR8 to FR12, FR18, FR55 to FR57, FR71 to FR73, FR126.
- **Epic 8: AI Guidance and Documentation** (Stories 8.1 to 8.8). Entry files, skills, settings, bmalph integration, README and policies, in-repository documents, planning bundle. Requirements: FR97, FR101, FR104 to FR109, FR111 to FR118.
- **Epic 9: Every Gate Proven Locally** (Stories 9.1 to 9.11). Isolated production-image stack with Schemathesis and smoke load, coverage, mutation testing, the Bats suite in eighteen files, the invariants and documentation-drift gates. Requirements: FR58, FR59, FR67, FR76, FR77, FR84, FR110, FR124, FR125.
- **Epic 10: Hosted CI, Release and Maintenance Automation** (Stories 10.1 to 10.5). Eleven workflows, Dependabot, the template-sync ignore list, the full local gate run and the clean-clone verification. Requirements: FR49, FR66, FR68, FR93 to FR96, FR99, FR100.

## Epic 1: Workspace Foundation and Command Surface

A backend engineer or AI assistant who clones the repository gets a Cargo workspace with every crate, the pinned toolchain, the pinned tool versions, the lint policy and the protected list in place, one Make command per task, a preflight that names missing tools, and repository linters for every non-Rust file type, so that every later story lands under working gates.

### Story 1.1: Workspace skeleton and Rust tool configuration

As a backend engineer,
I want the complete Cargo workspace with all thirteen crates, the pinned toolchain, the pinned tool versions and the committed tool configuration,
So that every later change compiles against final manifests and is checked by the lint policy from its first line.

Track: A.
Depends on: none.
Owns: `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, `deny.toml`, `architecture.toml`, `protected-config.toml`, `.env`, `.env.test`, `.cargo/config.toml`, `.cargo/mutants.toml`, `.config/nextest.toml`, `.gitignore`, `.editorconfig`, `LICENSE` (existing file, kept byte-identical), `make/tool-versions.mk`, `crates/shared/domain/Cargo.toml`, `crates/shared/domain/src/lib.rs`, `crates/shared/application/Cargo.toml`, `crates/shared/application/src/lib.rs`, `crates/shared/infrastructure/Cargo.toml`, `crates/shared/infrastructure/src/lib.rs`, `crates/internal/health-check/domain/Cargo.toml`, `crates/internal/health-check/domain/src/lib.rs`, `crates/internal/health-check/application/Cargo.toml`, `crates/internal/health-check/application/src/lib.rs`, `crates/internal/health-check/infrastructure/Cargo.toml`, `crates/internal/health-check/infrastructure/src/lib.rs`, `crates/company-subdomain/some-module/domain/Cargo.toml`, `crates/company-subdomain/some-module/domain/src/lib.rs`, `crates/company-subdomain/some-module/application/Cargo.toml`, `crates/company-subdomain/some-module/application/src/lib.rs`, `crates/company-subdomain/some-module/infrastructure/Cargo.toml`, `crates/company-subdomain/some-module/infrastructure/src/lib.rs`, `crates/service/Cargo.toml`, `crates/service/src/lib.rs`, `crates/service/src/main.rs`, `tests/support/Cargo.toml`, `tests/support/src/lib.rs`, `tests/e2e/Cargo.toml`, `tests/e2e/main.rs`, `xtask/Cargo.toml`, `xtask/src/main.rs`.
Touches: none.
Implements: FR36, FR37, FR41, FR45, FR90, FR92, FR118, FR121, FR123, NFR9, NFR10, NFR13, NFR16, NFR17, NFR26, NFR28, NFR29, NFR47.
Reference: architecture.md "Workspace and Project Structure" (all four subsections: "Complete tree of the Rust workspace", "Crate table", "`architecture.toml`", "Root `Cargo.toml`"), "Quality Gates" (the seven configuration-file subsections and "Protected configuration"), "Configuration" ("Committed environment files"), AD-01, AD-02, AD-27; architecture-delivery.md DD-04, DD-06, DD-16 (entries appended to the protected list), "Command Surface" ("`make/tool-versions.mk`"), "Repository Lint and Hygiene" (`.editorconfig`, `.gitignore`).
Pitfalls: PF-01, PF-02, PF-04, PF-08, PF-33, PF-35, PF-40, PF-55.
Verify: `cargo metadata --locked --format-version 1 > /dev/null`; `cargo check --locked --workspace --all-targets`; `cargo fmt --all --check`; `cargo clippy --locked --workspace --lib --bins --all-features -- -D warnings`; `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings -A clippy::cognitive_complexity -A clippy::too_many_lines`; `env RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --no-deps --document-private-items`; `git ls-files --error-unmatch Cargo.lock .env .env.test make/tool-versions.mk`.

**Acceptance Criteria:**

**Given** a clean checkout with toolchain 1.99.1 and no database, broker or `DATABASE_URL`
**When** `cargo metadata --locked --format-version 1 > /dev/null` and `cargo check --locked --workspace --all-targets` are run
**Then** both exit 0 and `git diff --exit-code -- Cargo.lock` exits 0 afterwards
**And** `cargo metadata --locked --format-version 1 --no-deps` lists exactly the thirteen packages of the crate table, each with the dependency and dev-dependency names of its row (including the per-crate feature additions the architecture lists)

**Given** the root `Cargo.toml`
**When** it is compared with the section "Root `Cargo.toml`"
**Then** `[workspace].members`, `[workspace.package]` (version `0.1.0`, licence `CC0-1.0`, `rust-version = "1.99.1"`, the same full version as `rust-toolchain.toml`), every `[workspace.dependencies]` entry and the three `[workspace.lints.*]` tables equal the listing, and every member manifest ends with `[lints] workspace = true` and declares no cargo feature

**Given** the stub sources (one documented `lib.rs` per library crate, `crates/service/src/main.rs` in its final form `fn main() -> ExitCode { service::run() }`, `xtask/src/main.rs` and `tests/e2e/main.rs` as minimal entry points)
**When** the format check, both clippy passes and the rustdoc command of the Verify line are run
**Then** each exits 0 with no warning, and `grep -rn "#\[allow\|#!\[allow" crates tests xtask` prints nothing

**Given** the committed configuration files
**When** each is compared with its listing in the architecture
**Then** `rust-toolchain.toml` pins channel `1.99.1` with `rustfmt`, `clippy` and `llvm-tools-preview`; `.cargo/config.toml` sets the `xtask` alias and `[env]` `SQLX_OFFLINE = "true"`, `TEST_SEED = "20261110"`, `PROPTEST_RNG_SEED = "20261110"` and `SWAGGER_UI_DOWNLOAD_URL = "vendored-only"` (FR8); `rustfmt.toml`, `clippy.toml`, `deny.toml`, `.cargo/mutants.toml`, `.config/nextest.toml` and `architecture.toml` equal their listings; exactly one `clippy.toml` exists in the tree

**Given** `protected-config.toml`
**When** it is read
**Then** it holds `approval_label = "approved-config-change"`, the core entries of "Protected configuration" (`files`, `directories`, `manifest_tables`, `json_values`, `override_names`, `override_exempt`, `required_directories`) and every entry DD-16 appends, so that no later story edits the file

**Given** `make/tool-versions.mk`
**When** it is compared with its listing in "Command Surface"
**Then** it is equal: every line is `export NAME := value` (`OPENAPI_DIFF_IMAGE` alone with `?=`), every cargo tool version equals the Tech Stack table, the four Bats entries hold the tags and commit SHAs of DD-06, and every image reference carries a tag and a `sha256` digest

**Given** `.env`, `.env.test`, `.gitignore` and `.editorconfig`
**When** they are read
**Then** both env files are tracked, carry the `LOCAL-ONLY` header comment and only the placeholder values of the architecture; `.env.test` holds only `APP_ENV`, `LOG_LEVEL` and `DATABASE_URL`; `.gitignore` lists every path of the delivery document's `.gitignore` intent and does not ignore `.env` or `.env.test`; `git diff --exit-code HEAD -- LICENSE` exits 0

### Story 1.2: Makefile command surface, execution switch and host preflight

As a backend engineer,
I want one Makefile that exposes every task of the repository as a named target with a help line, and a preflight that names every missing prerequisite,
So that I, CI, git hooks and AI assistants all run the same commands and a missing tool is reported by name before a gate fails with a tool error.

Track: C.
Depends on: 1.1.
Owns: `Makefile`, `make/gates.mk`, `scripts/with-env.sh`, `scripts/doctor.sh`.
Touches: none.
Implements: FR40, FR42, FR45, FR60, FR61, FR62, FR63, FR64, FR65, FR66, FR69, FR70, FR92, NFR16, NFR17, NFR37, NFR41, G1, G2, G3, G5.
Reference: architecture-delivery.md "Command Surface" (the introduction, "Variables" with the normative header of the `Makefile`, "`make/gates.mk`", "Targets", "PHP target to Rust target", "Scripts and commands the targets call": rows `with-env.sh` and `doctor.sh`), DD-01, DD-02, DD-03, DD-19, DD-23; architecture.md "Quality Gates" ("Gate commands": the command lines of QG-01 to QG-20 are repeated character for character, with the coverage sequence that excludes the end-to-end suite and the mutation command that unsets both `CARGO_TARGET_DIR` and `CARGO_BUILD_WARNINGS`).
Pitfalls: PF-11, PF-32, PF-34, PF-39, PF-57.
Verify: `make`; `make help`; `make ci-list`; `make -n ci`; `make -n bootstrap`; `make format-check`; `make validate-manifest`; `make lint-code`; `make complexity`; `make -n format-check`; `RUN_IN_CONTAINER=1 make -n format-check`; `CI=true RUN_IN_CONTAINER=1 make -n format-check`; `IN_DEV_CONTAINER=1 RUN_IN_CONTAINER=1 make -n format-check`.

**Acceptance Criteria:**

**Given** the repository after Story 1.1
**When** `make` is run with no argument
**Then** it exits 0 and prints `Usage:`, `make [target] [arg="val"...]` and `Targets:`, followed by one line per target with the target name in the first column and its help text
**And** `make help | awk 'NR>3{print $1}' | sort -u | wc -l` prints `107`, the list holds the 42 PHP names of FR62 and the aliases `up`, `clean`, `test`, `all` and `ci-sequential`, and `comm -23 <(make -pRrq : 2>/dev/null | sed -n 's/^[.]PHONY: //p' | tr ' ' '\n' | sort -u) <(make help | awk 'NR>3{print $1}' | sort -u)` prints nothing (no phony target without a help line)

**Given** the Makefile
**When** `make -n ci`, `make ci-list` and `make -n bootstrap` are run
**Then** `make -n ci` shows `doctor` first, then the seven stages of DD-02 in order, then the echo of `CI checks successfully passed!`; `make ci-list` prints the ordered gate targets, one per line; `make -n bootstrap` shows `scripts/doctor.sh --prerequisites`, the install step, `install-hooks`, `scripts/doctor.sh` and `start` in this order

**Given** a host with the toolchain and no container running
**When** the four `make -n format-check` commands of the Verify line are run
**Then** the first, third and fourth print `cargo fmt --all --check` with no prefix, and the second prints it prefixed by `docker compose exec -T --user <uid>:<gid> -e TEST_SEED -e PROPTEST_RNG_SEED -e CONFIG_CHANGE_APPROVED -e CARGO_BUILD_WARNINGS -e DATABASE_STATEMENT_TIMEOUT app`
**And** `DOCKER=/nonexistent make lint-docker` (a class HOST target without a reachable container engine) exits 2 and prints `This target needs a reachable container engine (docker).`

**Given** the gates that need only the toolchain
**When** `make format-check`, `make validate-manifest`, `make lint-code` and `make complexity` are run on the clean tree
**Then** each exits 0
**And** after a badly indented line is appended to `xtask/src/main.rs`, `make format-check` exits non-zero and prints `Diff in`, the file is not modified by the gate, and `git checkout -- xtask/src/main.rs` restores it

**Given** `make/gates.mk` and the header of the `Makefile`
**When** they are compared with their listings
**Then** every `*_CMD` value equals the corresponding QG command line, the three exported flag variables are `SPECTRAL_FLAGS`, `OASDIFF_FLAGS` and `TRIVY_FLAGS` (without `--ignore-unfixed`), the header equals the normative listing of "Variables", every caller-settable variable is declared with `?=`, and `grep -n "^include\|^-include" Makefile` shows the two fragment includes and the final `-include make/service.mk` (no environment file is included, and the template ships no `make/service.mk`)

**Given** an env file containing `A=1`, `B=${A}-x` and `C=file`
**When** `C=caller scripts/with-env.sh <file> -- sh -c 'echo $A $B $C'` is run, then `scripts/with-env.sh /nonexistent -- true`, then `scripts/with-env.sh <file> true`
**Then** the first prints `1 1-x caller` and exits 0, the second exits 2 naming the missing file, and the third exits 2 because the separator is absent; with two files a later file overrides an earlier one

**Given** a `PATH` that holds only the directories of `make`, `bash`, `git` and coreutils
**When** `make doctor` is run
**Then** it exits non-zero, its first line is the platform line (`Platform: linux/x86_64 (verified in CI)` on that platform), and it prints `[MISSING] cargo` with the targets that need it and how to install it; Node.js and `jq` are reported as optional and never make the result fail

### Story 1.3: Pinned tool and Bats installation

As a backend engineer,
I want one script that installs every pinned cargo tool and one that installs Bats with its helper libraries at pinned commits,
So that a host, the development image and CI use the same tool versions without git submodules.

Track: C.
Depends on: 1.1.
Owns: `scripts/install-tools.sh`, `scripts/install-bats.sh`.
Touches: none.
Implements: FR46, FR122, NFR9, NFR13, NFR18, NFR37, G7, G29.
Reference: architecture-delivery.md DD-04, DD-06, "Command Surface" ("Scripts and commands the targets call": rows `install-tools.sh`, `install-bats.sh`; "`make/tool-versions.mk`": the line format both scripts read); architecture.md "Tech Stack" ("Cargo tools (gates and developer commands)"), "Quality Gates" ("Gate commands": QG-08, QG-09 and the paragraph on tool installation).
Pitfalls: PF-06, PF-49.
Verify: `scripts/install-tools.sh`; `scripts/install-bats.sh`; `cargo fetch --locked`; `.tools/bats/core/bin/bats --version`; `cargo deny --locked check`; `cargo hack check --workspace --each-feature --locked`.

**Acceptance Criteria:**

**Given** a host with the container engine, GNU Make, Bash, git, curl and rustup
**When** `scripts/install-tools.sh` and `scripts/install-bats.sh` are each run twice
**Then** all four runs exit 0, every cargo tool of `make/tool-versions.mk` reports its pinned version, `.tools/bats/` holds bats-core and the three helper libraries at the pinned commits, and the second runs reinstall nothing
**And** the output of `git status --porcelain` is the same before and after the runs (`.tools/` is ignored)

**Given** `scripts/install-tools.sh`
**When** it is read
**Then** it takes the versions from `make/tool-versions.mk` in the `export NAME := value` format, with a value from the environment taking precedence; it fetches `cargo-binstall` as a release archive by URL and checks it against the recorded SHA-256; it installs with `--disable-strategies compile`, and when no prebuilt binary exists it says so and runs `cargo install --locked --version <v>` (`sqlx-cli` without `--locked` and with `--no-default-features --features postgres,rustls`); `--with-watchexec` adds watchexec for the development image

**Given** a wrong commit SHA for one helper library
**When** `BATS_ASSERT='v2.2.4 0000000000000000000000000000000000000000' scripts/install-bats.sh --prefix "$(mktemp -d)"` is run
**Then** it exits 1 and names `bats-core/bats-assert` (a value from the environment takes precedence over the fragment, as inside Make)

**Given** the skeleton workspace and the installed tools
**When** `cargo deny --locked check` and `cargo hack check --workspace --each-feature --locked` (QG-09 and QG-08) are run
**Then** both exit 0: advisories, licences, bans and sources of the lock file pass, and each of the thirteen crates checks alone

### Story 1.4: Repository lint and format tooling

As a reviewer,
I want workflows, YAML, shell, Dockerfile, Markdown, TOML, JSON, JavaScript and spelling checked by pinned linters behind one Make target,
So that non-Rust files meet one standard and each linter can fail.

Track: C.
Depends on: 1.2, 1.3.
Owns: `.yamllint.yml`, `.markdownlint-cli2.yaml`, `.hadolint.yaml`, `.prettierrc.yml`, `.prettierignore`, `taplo.toml`, `_typos.toml`, `.github/zizmor.yml`, `tests/Load/package.json`, `tests/Load/pnpm-workspace.yaml`, `tests/Load/pnpm-lock.yaml`, `tests/Load/eslint.config.js`, `tests/Load/.gitignore`.
Touches: the TOML files created by Story 1.1 (whitespace only, to match `taplo fmt`; no key or value changes); `README.md` (final newline) and `CLAUDE.md` (the three table separator rows of the section "BMAD-METHOD Integration" get three dashes per column, the form markdownlint accepts; no other byte changes); `Makefile` (the `PNPM_INSTALL` variable gains `--store-dir`, so the pnpm store stays outside the repository; recorded as a deviation).
Implements: FR43, FR69, FR83, NFR25, NFR38, G23.
Reference: architecture-delivery.md "Repository Lint and Hygiene" (tool table and every configuration listing), "Load Testing" ("JavaScript lint"), DD-22, "Command Surface" (targets `install`, `doctor`, `lint-repo`, `lint-actions`, `lint-yaml`, `lint-markdown`, `lint-shell`, `lint-docker`, `lint-toml`, `lint-spelling`, `lint-prettier`, `lint-load-tests`, `format-repo`), "Delivery Validation" ("Measurement items and readings of requirement wording": item 10).
Pitfalls: PF-16, PF-59, PF-66.
Verify: `make install`; `make doctor`; `make ci CI_STAGES=help`; `make check-security`; `make check-crate-isolation`; `make lint-toml`; `make lint-yaml`; `make lint-markdown`; `make lint-shell`; `make lint-spelling`; `make lint-prettier`; `make lint-load-tests`.

**Acceptance Criteria:**

**Given** the repository after Stories 1.1 to 1.3
**When** `make install` is run twice, then `make doctor`, `make check-security`, `make check-crate-isolation` and `make ci CI_STAGES=help`
**Then** every command exits 0, the second `make install` reinstalls nothing, `make doctor` prints the platform line and one `[OK]` line per prerequisite of the `doctor.sh` row, and the last line of the `ci` run is `CI checks successfully passed!`
**And** `make doctor CARGO_NEXTEST_VERSION=0.0.1` exits non-zero and prints `[OUTDATED] cargo-nextest: found 0.9.151, need 0.0.1`

**Given** the repository with the configuration files of this story
**When** `make lint-toml`, `make lint-yaml`, `make lint-markdown`, `make lint-shell`, `make lint-spelling`, `make lint-prettier` and `make lint-load-tests` are run
**Then** each exits 0, and each tool runs from the image or binary pinned in `make/tool-versions.mk` (no host Node.js or Python is needed)
**And** if `typos` reports words inside the bmalph-generated files under `.claude/commands/`, that directory is added to `extend-exclude` in `_typos.toml` and the addition is recorded as a deviation

**Given** each configuration file
**When** it is compared with its listing in "Repository Lint and Hygiene"
**Then** the content matches, the generated snapshot directories, `.sqlx/`, `specs/`, lock files and `tests/CLI/bats/fixtures/` are excluded where the listing says so, and `.prettierrc.yml` is byte-identical to `${PHP_TEMPLATE_DIR}/.prettierrc.yml`

**Given** `README.md` and `CLAUDE.md` after this story
**When** `git diff HEAD~1 -- README.md CLAUDE.md` is read on the story's commit
**Then** the only changes are the final newline of `README.md` and the separator rows of the three tables of `CLAUDE.md`, now `| --- | --- | --- |`, `| --- | --- |` and `| --- | --- | --- |` (rule MD060 of the pinned markdownlint rejects the rows bmalph writes), which is what lets `make lint-markdown` pass before Epic 8 replaces or extends the two files (the kept BMAD section is otherwise as bmalph writes it; the change is recorded as a deviation from "Entry files")

**Given** a temporary untracked file planted for one linter at a time: a YAML file with a duplicate key, a shell script with an unquoted expansion, a TOML file with misaligned formatting, a Markdown file that skips a heading level, a file containing the word `recieve`, an unformatted JSON file
**When** the matching target (`lint-yaml`, `lint-shell`, `lint-toml`, `lint-markdown`, `lint-spelling`, `lint-prettier`) is run
**Then** it exits non-zero and its output names the planted file, and after the file is removed the target exits 0 again

**Given** a temporary minimal workflow `.github/workflows/tmp.yml` (`on: push`, top-level `permissions: contents: read`, one job on `ubuntu-24.04` with a timeout whose only step is `uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1` with `persist-credentials: false`)
**When** `make lint-actions` is run
**Then** it exits 0
**And** after the `uses:` value is changed to `actions/checkout@v7` it exits non-zero and zizmor reports the unpinned action; the file and the directory are removed afterwards, and until Story 10.1 no `Verify` line contains `lint-actions` or `lint-repo`, because zizmor cannot run on a missing or empty workflow directory

**Given** `tests/Load/package.json` and `tests/Load/eslint.config.js`
**When** they are read
**Then** the package is private, names `pnpm@12.18.0` as package manager, has no scripts and exactly the development dependencies of "JavaScript lint", its licence field is `CC0-1.0`, and `pnpm-lock.yaml` was generated from it (`corepack pnpm install --frozen-lockfile` succeeds inside the pinned Node image)

## Epic 2: Architecture Enforcement and Scaffolding

A backend engineer gets layer and bounded-context rules that are enforced structurally by `make architecture`, a protected-configuration guard that stops a gate from being weakened without an approval, and one command that creates a new bounded context which already passes both, so that the architecture of a service built from the template cannot erode unnoticed.

### Story 2.1: Architecture gate with layer, coverage and lint-inheritance rules

As a backend engineer,
I want `cargo xtask arch` to read `architecture.toml` and the resolved workspace and fail on unassigned crates, ambiguous crates, forbidden layer dependencies and crates outside the lint policy,
So that a boundary violation is reported with both ends of the dependency before review.

Track: B.
Depends on: 1.2, 1.3.
Owns: `xtask/src/cli.rs`, `xtask/src/cli/`, `xtask/src/arch.rs`, `xtask/src/arch/`, `xtask/tests/integration/main.rs`, `xtask/tests/integration/support.rs`, `xtask/tests/integration/arch.rs`, `xtask/tests/fixtures/arch/workspace/`, `xtask/tests/fixtures/arch/valid.toml`, `xtask/tests/fixtures/arch/invalid.toml`, `xtask/tests/fixtures/arch/nothing-covered.toml`.
Touches: `xtask/src/main.rs` (replaces the stub with the call to `cli::main`).
Implements: FR37, FR38, NFR18, NFR21, G4.
Reference: architecture.md AD-03, "Workspace and Project Structure" ("`architecture.toml`" with its schema notes; the `xtask/` part of the tree), "Implementation Patterns and Consistency Rules" (rules 1 to 23, in particular "Structure and tests", "Logging" rule 18, "Code shape imposed by the tools"), "Test Architecture" ("Levels"), "Quality Gates" ("Gate commands": QG-05 and the paragraph "Negative tests (NFR21)").
Pitfalls: PF-09, PF-36, PF-38, PF-44.
Verify: `make format-check`; `make lint-code`; `make complexity`; `make unit-tests`; `cargo nextest run --locked -p xtask --test integration`; `make architecture`; `make validate-manifest`; `cargo llvm-cov nextest --locked -p xtask --profile ci --summary-only`; `env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --cargo-arg=--locked -p xtask`.

**Acceptance Criteria:**

**Given** the real workspace after Story 1.1
**When** `make architecture` is run
**Then** it exits 0 and reports that every one of the thirteen workspace crates is assigned to exactly one layer

**Given** the fixture workspace `xtask/tests/fixtures/arch/workspace/`, which sits outside the real workspace members and plants one violation per rule
**When** `cargo xtask arch --manifest-path <fixture>/Cargo.toml --config xtask/tests/fixtures/arch/valid.toml` is run
**Then** it exits 1 and prints one compiler-style diagnostic for each of `arch::uncovered`, `arch::ambiguous`, `arch::layer` and `arch::lints`, each naming the offending crate, and for `arch::layer` both ends of the forbidden dependency and a hint
**And** a dev-dependency on a layer listed only in `dev_depends_on` is not reported, while the same edge as a normal dependency is

**Given** `xtask/tests/fixtures/arch/invalid.toml` (an unknown key) or a manifest path that does not exist
**When** the command is run with it
**Then** it exits 2 and prints why the check could not run; with `nothing-covered.toml` every crate is reported as `arch::uncovered` and the exit status is 1

**Given** `GITHUB_ACTIONS=true` in the environment
**When** the command reports a violation
**Then** the output also contains one `::error` annotation line per violation

**Given** the unit tests in the sibling `tests.rs` files and the integration tests
**When** `make unit-tests` and `cargo nextest run --locked -p xtask --test integration` are run
**Then** all pass; the unit tests cover pattern matching with `*`, context extraction (`some-module-domain` gives `some-module`), every branch of the four rules and the report formats; the integration tests run the built binary through `tests/integration/support.rs`, which clears the environment and forwards `LLVM_PROFILE_FILE` to the child

**Given** the new sources
**When** `make format-check`, `make lint-code` and `make complexity` are run
**Then** each exits 0, `xtask/src/main.rs` contains only the call into `cli`, and command output is written with `writeln!` in `xtask::cli` only

**Given** the production files this story owns (`xtask/src/cli.rs`, `xtask/src/arch.rs` and the files under `xtask/src/arch/`)
**When** the coverage command and the mutation command of the Verify line are run
**Then** each of those files shows 100% lines and functions in the summary and cargo-mutants exits 0 with no missed and no timed-out mutant; a line or mutant that only code of a later story can reach is listed in the completion notes with that story's id

### Story 2.2: Context-isolation, third-party allow-list and path rules

As a backend engineer,
I want the architecture gate to also fail when one bounded context reaches into another, when a layer uses a third-party crate outside its allow-list or can reach a forbidden one, and when a layer crate sits at the wrong path,
So that Domain code stays framework-free and contexts stay isolated without a rule-file edit per context.

Track: B.
Depends on: 2.1.
Owns: `xtask/tests/fixtures/arch/vendor/`.
Touches: `xtask/src/arch/rules.rs` and `xtask/src/arch/rules/tests.rs` (four more rules), `xtask/src/arch/load.rs` and its tests (third-party and transitive facts), `xtask/src/arch/config.rs` and `xtask/src/arch/config/tests.rs` (the configuration fields of the new rules), `xtask/src/arch/model.rs` (the `reach` field of a crate), `xtask/src/arch/tests.rs`, `xtask/src/arch/report/tests.rs` and `xtask/src/cli/tests.rs` (literals and fixture counts that the new fields change), `xtask/tests/fixtures/arch/workspace/` (four more planted violations), `xtask/tests/integration/arch.rs` (assertions for the new rules).
Implements: FR37, FR38, NFR18, NFR21, G4.
Reference: architecture.md AD-03 (rule table: `arch::context`, `arch::external`, `arch::transitive`, `arch::path`), "Workspace and Project Structure" ("`architecture.toml`": `external`, `dev_external`, `forbidden_transitive`, `directory`, `[contexts].shared`), AD-02.
Pitfalls: PF-09, PF-44.
Verify: `make format-check`; `make lint-code`; `make complexity`; `make unit-tests`; `cargo nextest run --locked -p xtask --test integration`; `make architecture`; `cargo llvm-cov nextest --locked -p xtask --profile ci --summary-only`; `env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --cargo-arg=--locked -p xtask`.

**Acceptance Criteria:**

**Given** the real workspace
**When** `make architecture` is run
**Then** it exits 0 with all eight rules active, which proves that every third-party dependency of every crate is on its layer's allow-list and that no Domain crate can reach `axum`, `hyper`, `reqwest`, `sqlx` or `tokio` through non-dev edges
**And** the run in the story's worktree includes every crate that finished stories have filled by then, so their dependency lists are checked by the complete rule set before any later story builds on them

**Given** the fixture workspace with a Domain crate that depends on another context's Domain crate, a Domain crate that depends on a vendored stand-in for `serde`, an Application crate from which a stand-in for `sqlx` is reachable transitively, and a layer crate placed outside `crates/[<group>/]<context>/<layer>`
**When** `cargo xtask arch --manifest-path <fixture>/Cargo.toml --config xtask/tests/fixtures/arch/valid.toml` is run
**Then** it exits 1 and the output contains `arch::context`, `arch::external`, `arch::transitive` and `arch::path`, each with the crate name and, for the dependency rules, both ends of the edge

**Given** a context listed in `[contexts].shared`
**When** another context's crate depends on its crate of an allowed layer
**Then** no `arch::context` diagnostic is produced

**Given** the stand-in crates under `xtask/tests/fixtures/arch/vendor/`
**When** `cargo metadata --locked --format-version 1 --no-deps` is run at the repository root
**Then** neither the fixture workspace nor a vendor crate appears among the workspace members

**Given** the rule unit tests
**When** `make unit-tests` is run
**Then** each of the eight rules has at least one test that produces its diagnostic and one that produces none, and `make lint-code` and `make complexity` exit 0

**Given** the production files this story owns (the rule and loader files under `xtask/src/arch/` that it extends)
**When** the coverage command and the mutation command of the Verify line are run
**Then** each of those files shows 100% lines and functions in the summary and cargo-mutants exits 0 with no missed and no timed-out mutant; a line or mutant that only code of a later story can reach is listed in the completion notes with that story's id

### Story 2.3: Protected-configuration guard

As a reviewer,
I want `make validate-configuration base_ref=<ref>` to fail when a protected file, table, JSON value or required directory differs from the base ref, unless the change is approved,
So that nobody weakens a gate to obtain a pass.

Track: B.
Depends on: 2.2.
Owns: `xtask/src/config_guard.rs`, `xtask/src/config_guard/`, `xtask/tests/integration/config_guard.rs`.
Touches: `xtask/src/main.rs` (module declaration `mod config_guard;`), `xtask/src/cli.rs` and `xtask/src/cli/tests.rs` (subcommand `config-guard` with `--base`, `--list` and the approval variable), `xtask/tests/integration/main.rs` (module declaration), `xtask/tests/integration/support.rs` (temporary git repositories).
Implements: FR44, NFR21, NFR26, G25.
Reference: architecture.md AD-25, "Quality Gates" ("Protected configuration": the list format and the nine decision steps), "Test Architecture" ("Required test cases added by the alignment with the PRD": row FR44); architecture-delivery.md DD-16.
Pitfalls: PF-43, PF-44.
Verify: `make format-check`; `make lint-code`; `make complexity`; `make unit-tests`; `cargo nextest run --locked -p xtask --test integration`; `cargo xtask config-guard --list`; `cargo llvm-cov nextest --locked -p xtask --profile ci --summary-only`; `env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --cargo-arg=--locked -p xtask`.

**Acceptance Criteria:**

**Given** a temporary git repository whose base commit holds a protected-config list, `clippy.toml`, a `Cargo.toml` with `[workspace.lints]` and `tests/Load/config.json.dist`
**When** the working tree changes `clippy.toml`, deletes another protected file, changes a lint level, changes one protected JSON budget and adds a nested `crates/x/clippy.toml`, and `cargo xtask config-guard --base HEAD` is run
**Then** it exits 1 and prints `Configuration Validation`, one line each of `Modification of locked configuration file is not allowed: <path>` (twice), `Modification of locked configuration table is not allowed: Cargo.toml [workspace.lints]`, `Modification of locked configuration value is not allowed: tests/Load/config.json.dist <pointer>`, `Overriding configuration file is not allowed: crates/x/clippy.toml`, then `[FAIL] Configuration validation failed` and `Approve with the label approved-config-change`

**Given** the same changes and `CONFIG_CHANGE_APPROVED=true`
**When** the command is run
**Then** it exits 0, prints each modification as approved and ends with `[OK] Configuration validation passed (approved by label approved-config-change)`
**And** a missing required directory still exits 1 with `Required directory is missing: <path>` in both cases

**Given** a base ref on which no protected item exists (the bootstrap case), a new dependency added to `Cargo.toml` outside the lint tables, or a file under an `override_exempt` fixture path
**When** the command is run
**Then** it exits 0 with `[OK] Configuration validation passed`

**Given** `--base no-such-ref`
**When** the command is run
**Then** it exits 2 and prints `configuration guard could not run: base ref no-such-ref is not available`

**Given** the real repository
**When** `cargo xtask config-guard --list` is run
**Then** it prints one protected item per line and exits 0: every entry of `files` as its path, every entry of `directories` with a trailing `/`, and every entry of `manifest_tables` and `json_values` as `<file>#<table or pointer>`

**Given** the guard's tests
**When** `make unit-tests` and the `xtask` integration tests are run
**Then** every case of the FR44 row of the required-test table passes (deleted file, deleted table, changed and missing JSON value, nested override, exempt fixture path, label name in the failure output, missing base, `--list`), with git reached through the `BaseTree` trait so unit tests use an in-memory double

**Given** the production files this story owns (`xtask/src/config_guard.rs` and the files under `xtask/src/config_guard/`)
**When** the coverage command and the mutation command of the Verify line are run
**Then** each of those files shows 100% lines and functions in the summary and cargo-mutants exits 0 with no missed and no timed-out mutant; a line or mutant that only code of a later story can reach is listed in the completion notes with that story's id

### Story 2.4: Bounded-context scaffolding command

As a backend engineer,
I want `make new-context group=<group> context=<context>` to create the three layer crates of a new bounded context and register them in the workspace,
So that I start a context that compiles and passes the architecture and crate-isolation gates without touching a protected file.

Track: B.
Depends on: 2.3.
Owns: `xtask/src/scaffold.rs`, `xtask/src/scaffold/`, `xtask/templates/`, `xtask/tests/integration/scaffold.rs`, `xtask/tests/fixtures/scaffold/`.
Touches: `xtask/src/main.rs` (module declaration `mod scaffold;`), `xtask/src/cli.rs` and `xtask/src/cli/tests.rs` (subcommand `new-context`), `xtask/tests/integration/main.rs` (module declaration), `xtask/tests/integration/arch.rs` (the crate count of the real workspace is asserted as at least 13, so the tests pass after `new-context`), `xtask/tests/integration/support.rs` (`collect` and a new `forward` become `pub(crate)`, so the integration target keeps one environment helper).
Implements: FR39, NFR18.
Reference: architecture.md AD-26, "Workspace and Project Structure" (the `xtask/` part of the tree: `scaffold*`, `templates/`), "Implementation Patterns and Consistency Rules" ("Recipes": "Add a bounded context"), "Test Architecture" (required-test row FR39(b)); architecture-delivery.md "Command Surface" (target `new-context`: the command refreshes `Cargo.lock` itself).
Pitfalls: PF-07, PF-09.
Verify: `make format-check`; `make lint-code`; `make complexity`; `make unit-tests`; `cargo nextest run --locked -p xtask --test integration`; `make architecture`; `cargo llvm-cov nextest --locked -p xtask --profile ci --summary-only`; `env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --cargo-arg=--locked -p xtask`.

**Acceptance Criteria:**

**Given** a copy of the fixture workspace `xtask/tests/fixtures/scaffold/workspace/`
**When** `cargo xtask new-context billing invoice-ledger` is run in it
**Then** it exits 0 and creates `crates/billing/invoice-ledger/{domain,application,infrastructure}`, each with a manifest that inherits the workspace package fields, has no dependency and ends with `[lints] workspace = true`, and a `src/lib.rs` that holds only crate documentation for its layer
**And** the three paths are added to `[workspace].members` with the existing formatting and comments preserved, and `Cargo.lock` is refreshed by the command itself (`cargo update --workspace --offline`, `scaffold/lock.rs`) so that `cargo metadata --locked` succeeds in the copy

**Given** the scaffolded copy
**When** `cargo xtask arch` and `cargo check --locked -p invoice-ledger-domain` (and the two sibling packages) are run in it
**Then** all exit 0, and `git status --porcelain` in the copy lists no path printed by `cargo xtask config-guard --list`

**Given** the command output
**When** it is read
**Then** it lists the remaining steps, among them adding `[workspace.dependencies]` entries, wiring the context in the composition root and adding the three components to `workspace.dsl`

**Given** an invalid name (`Billing`, `1x`, `a--b`), a reserved context name (`shared`, `service`, `xtask`, `test-support`, `e2e-tests`) or a context that already exists
**When** the command is run
**Then** it exits 1, names the reason and changes no file; a root manifest that cannot be parsed exits 2

**Given** the Make target
**When** `make new-context` is run without `group` or `context`
**Then** it exits 2 and prints the usage line

**Given** the production files this story owns (`xtask/src/scaffold.rs` and the files under `xtask/src/scaffold/`, with `scaffold/lock.rs` covered by the integration test)
**When** the coverage command and the mutation command of the Verify line are run
**Then** each of those files shows 100% lines and functions in the summary and cargo-mutants exits 0 with no missed and no timed-out mutant; a line or mutant that only code of a later story can reach is listed in the completion notes with that story's id

## Epic 3: Images, Local Stacks and Delivery Gate Tooling

A platform maintainer and a reviewer get the container images, the four compose stacks and the gate tooling that does not depend on finished service code: contract lint and breaking-change checks, secrets and image scans, the K6 load kit with its AWS runner, the commit convention with hooks and release tooling, and the validated architecture model, each as a Make target that is shown passing and failing. The backing services start with `make start-deps`; `make start` becomes healthy with Story 7.6.

### Story 3.1: Container images, compose stacks and dev container

As a backend engineer,
I want one multi-stage image definition and the four compose files with the PHP file names,
So that backing services start with one command, the production image is minimal and hardened, and the development stack never leaves foreign-owned files in my checkout.

Track: C.
Depends on: 1.4.
Owns: `Dockerfile`, `.dockerignore`, `docker-compose.yml`, `docker-compose.override.yml`, `docker-compose.prod.yml`, `docker-compose.load_test.override.yml`, `.devcontainer/devcontainer.json`.
Touches: none.
Implements: FR32, FR86, FR87, FR88, FR89, FR90, FR91, FR118, NFR3, NFR12, NFR13, NFR33, NFR37, NFR40.
Reference: architecture-delivery.md "Containers and Compose" (all subsections: "`Dockerfile`", "`.dockerignore`", "Compose files", "Development container definition"), DD-01, DD-05, DD-19, DD-20, DD-23, "Command Surface" (targets `start`, `start-deps`, `ensure-deps`, `ensure-database`, `build`, `build-prod-image`, `stop`, `down`; variables `BUILD_IMAGE`, `DEPS`); architecture.md "Observability and Operations" ("Operations": container healthcheck, start-up), "Configuration" ("Committed environment files").
Pitfalls: PF-04, PF-26, PF-27, PF-50, PF-51, PF-52, PF-53, PF-54, PF-55, PF-56, PF-57, PF-58.
Verify: `make lint-docker`; `make lint-yaml`; `make lint-prettier`; `make build-prod-image`; `docker compose build app`; `docker compose config --quiet`; `docker compose -p check-load -f docker-compose.yml -f docker-compose.load_test.override.yml config --quiet`; `make start-deps`; `docker compose ps --format json database localstack`.

**Acceptance Criteria:**

**Given** the repository after Story 1.4
**When** `make build-prod-image` and `docker compose build app` are run
**Then** both exit 0; the production image has entry point `/app/service`, user `65532:65532`, a `HEALTHCHECK` in exec form calling `/app/service healthcheck` with interval 10 s, timeout 3 s, retries 3 and start period 10 s, and `docker run --rm --entrypoint /bin/sh <prod image>` fails because the image has no shell
**And** `docker image inspect --format '{{.Size}}' <prod image>` is at most 104857600

**Given** the `Dockerfile`
**When** it is read and `make lint-docker` is run
**Then** hadolint exits 0; both base images are literal `FROM` references with tag and digest and no build argument replaces an image; dependencies are compiled in a cargo-chef layer before the source is copied; the `dev` stage sets `CARGO_TARGET_DIR=/cargo-target` and does not set `IN_DEV_CONTAINER`; `.dockerignore` excludes `.git/`, `target/`, `rust-toolchain.toml` and the other paths of the listing and does not exclude `Cargo.lock`, `.sqlx/`, `migrations/`, `.cargo/`, `make/tool-versions.mk` or `scripts/`

**Given** the four compose files
**When** `docker compose config --quiet`, the isolated combination of the Verify line and `docker compose --env-file /dev/null -f docker-compose.yml -f docker-compose.prod.yml config` are run
**Then** the first two exit 0 and the third exits non-zero with messages that name `DATABASE_URL`, `AWS_REGION` or both (depending on the Compose version it names every missing variable or stops at the first, in no fixed order)
**And** the rendered isolated combination publishes only `${LOAD_TEST_APP_PORT:-18081}` on `127.0.0.1`, sets `APP_ENV` to `test` and publishes no database or LocalStack port, and the rendered production reference (with both variables set) shows `read_only: true`, `cap_drop: [ALL]`, `no-new-privileges:true`, CPU, memory and process limits and `restart: unless-stopped` for `app`

**Given** no container of the project is running
**When** `make start-deps` is run twice
**Then** it exits 0 both times, `database` and `localstack` are healthy, the second run recreates no container, LocalStack is `localstack/localstack:3.4.0` pinned by digest, and the database health check uses `pg_isready` with the configured user and database

**Given** the development override
**When** `docker compose config` is rendered
**Then** `app` and `migrate` use the `dev` target and run as `${HOST_UID}:${HOST_GID}`; `app` sets `IN_DEV_CONTAINER=1`, `DB_HOST=database`, `DB_PORT=5432`, `LOCALSTACK_HOST=localstack` and `LOCALSTACK_PORT=4566` and its rendered environment holds no `APP_ENV`, `LOG_LEVEL`, `DATABASE_URL` or `AWS_ENDPOINT_URL_SQS`; `app` has no `command` (the image's watchexec `CMD` watches `crates`, `migrations`, `Cargo.toml` and `Cargo.lock`); the volume `cargo_target` is mounted at `/cargo-target` and no mount lies below `/app`; `app`, `database`, `localstack` and `structurizr` publish on `127.0.0.1`
**And** `HOST_UID=12345 HOST_GID=12345 DEV_IMAGE=<development image> docker compose run --rm --no-deps app sh -c 'id -u && touch /cargo-target/.probe /cargo/.probe && cargo metadata --locked --no-deps --format-version 1 > /dev/null'` prints `12345` and exits 0, and afterwards `find . -xdev -uid 12345 -not -path './.git/*' -print -quit` prints nothing: any user id can use the image and nothing is created in the checkout, in particular no `./target` owned by another user

**Given** `.devcontainer/devcontainer.json`
**When** it is read
**Then** it builds the `dev` stage of the `Dockerfile`, references both features by digest with `moby: false` for Docker inside the container, mounts volumes at `/cargo` and `/cargo-target`, does not set `IN_DEV_CONTAINER`, forwards ports 8081, 5432, 4566 and 8080 and runs `make install-hooks && make doctor` after creation

### Story 3.2: Contract lint and breaking-change checks

As a reviewer,
I want the OpenAPI snapshot linted with the unchanged PHP Spectral ruleset and both contract snapshots compared with a base ref,
So that a lint finding or an unapproved breaking change fails under its own gate name.

Track: C.
Depends on: 1.4.
Owns: `.spectral.yaml`, `scripts/validate-openapi-spec.sh`, `scripts/openapi-diff.sh`, `scripts/graphql-diff.sh`.
Touches: none.
Implements: FR74, FR75, NFR24, G18, G19, G20.
Reference: architecture-delivery.md DD-10, DD-22, "Command Surface" (targets `validate-openapi-spec`, `openapi-diff`, `graphql-diff`; script rows `validate-openapi-spec.sh`, `openapi-diff.sh`, `graphql-diff.sh`; variables `base_ref`, `BREAKING_CHANGE_APPROVED`, `OPENAPI_DIFF_IMAGE`; "`make/gates.mk`": the exported `SPECTRAL_FLAGS` and `OASDIFF_FLAGS`), "Repository File Manifest" (Root: `.spectral.yaml`).
Pitfalls: PF-15, PF-17.
Verify: `cmp .spectral.yaml ${PHP_TEMPLATE_DIR}/.spectral.yaml`; `make lint-shell`; `make lint-yaml`; the fixture runs of the acceptance criteria below (the real snapshots do not exist until Story 7.4).

**Acceptance Criteria:**

**Given** `.spectral.yaml`
**When** it is compared with the PHP template's file
**Then** `cmp` exits 0 (byte-identical)

**Given** a temporary git repository that contains the three scripts, `.spectral.yaml`, the `Makefile`, `make/` and a minimal valid OpenAPI 3.1.0 document at `.github/openapi-spec/spec.yaml` that satisfies every rule of the ruleset
**When** `make validate-openapi-spec` is run there, and again after one operation `description` is removed
**Then** the first run exits 0 and the second exits non-zero with the Spectral finding; with the snapshot file absent the script's exit status is 2 (the scripts are run through Make, which exports the images and flags they read with `${NAME:?}`)

**Given** that repository with the document committed, and a working copy in which a required response property is removed
**When** `make openapi-diff base_ref=HEAD` is run
**Then** it exits non-zero, the oasdiff output contains `response-required-property-removed` and the line `Approve with the label approved-breaking-change`
**And** with `BREAKING_CHANGE_APPROVED=true` it prints the findings and exits 0; with an unchanged document it exits 0; with `base_ref=no-such-ref` the script's exit status is 2; with a base commit that holds no snapshot it exits 0 with a notice that this is the first version of the contract

**Given** a GraphQL schema committed at `.github/graphql-spec/spec` in that repository and a working copy in which one field is removed
**When** `make graphql-diff base_ref=HEAD` is run
**Then** it exits non-zero and the graphql-inspector output reports a breaking change and prints the label line; the approval variable, the unchanged case, the unresolvable base and the missing base snapshot behave as for OpenAPI

**Given** the scripts
**When** they are read and `make lint-shell` is run
**Then** shellcheck exits 0; each script starts with `set -euo pipefail`; Spectral runs with `$SPECTRAL_FLAGS` (`--fail-severity=hint`) and oasdiff with `breaking ... $OASDIFF_FLAGS` (`--fail-on ERR`) from the image named by `OPENAPI_DIFF_IMAGE`, both flag variables exported by `make/gates.mk`, so the scripts hold no threshold of their own; graphql-inspector runs at the pinned version inside the pinned Node image; all three use read-only mounts and the invoking user's uid

### Story 3.3: Secrets scan and production-image scan

As a platform maintainer,
I want `make secrets-scan` and `make image-scan` to fail on a committed secret and on a HIGH or CRITICAL image vulnerability,
So that neither reaches `main` unnoticed.

Track: C.
Depends on: 3.1.
Owns: `scripts/secrets-scan.sh`, `scripts/image-scan.sh`, `.gitleaks.toml`, `.gitleaksignore`, `.trivyignore`.
Touches: none.
Implements: FR47, FR48, NFR10, NFR11, G9, G10.
Reference: architecture-delivery.md DD-21, "Command Surface" (targets `secrets-scan`, `image-scan`; script rows `secrets-scan.sh`, `image-scan.sh`; hook `IMAGE_TO_SCAN`; "`make/gates.mk`": the exported `TRIVY_FLAGS`; "`make/tool-versions.mk`": `VULNERABLE_FIXTURE_IMAGE`), "Repository Lint and Hygiene" (`.gitleaks.toml`, `.gitleaksignore`, `.trivyignore`).
Pitfalls: PF-58.
Verify: `make secrets-scan`; `make image-scan`; `make lint-shell`; `make lint-toml`.

**Acceptance Criteria:**

**Given** the repository, including the committed `.env` and `.env.test` placeholders
**When** `make secrets-scan` is run
**Then** it exits 0 after scanning the full git history and a copy of the tracked and untracked-but-not-ignored working files, with findings redacted

**Given** an untracked file that holds a private key generated at test time (for example with `openssl genpkey`), never committed
**When** `make secrets-scan` is run
**Then** it exits 1 and the gitleaks output reports `leaks found`, and after the file is deleted the target exits 0

**Given** the production image built by Story 3.1
**When** `make image-scan` is run
**Then** it exits 0; Trivy scans a `docker save` archive under `target/reports/image-scan/` with `$TRIVY_FLAGS` from `make/gates.mk` (`--severity HIGH,CRITICAL --exit-code 1 --ignorefile .trivyignore`, no `--ignore-unfixed`: a finding without a fix counts like any other) and does not mount the Docker socket

**Given** the fixture image pinned as `VULNERABLE_FIXTURE_IMAGE` in `make/tool-versions.mk`
**When** `make image-scan IMAGE_TO_SCAN='$(VULNERABLE_FIXTURE_IMAGE)'` is run
**Then** it exits non-zero and lists at least one CRITICAL finding of the digest-pinned fixture image, without rebuilding the production image; for an image name that does not exist the script's exit status is 2

**Given** `.gitleaks.toml`, `.gitleaksignore` and `.trivyignore`
**When** they are read
**Then** gitleaks extends the default rule set; both ignore files contain no entry, only a header comment that gives the entry format with date, reason and review date

### Story 3.4: K6 load-test kit

As a backend engineer,
I want the PHP K6 kit ported with its `health` scenario, per-profile budgets and local runners that use the stock K6 image,
So that I can add a scenario by adding one script and a load run fails when a budget is broken.

Track: C.
Depends on: 1.4.
Owns: `tests/Load/config.json.dist`, `tests/Load/scripts/`, `tests/Load/utils/`, `tests/Load/get-load-test-scenarios.sh`, `tests/Load/execute-load-test.sh`, `tests/Load/run-smoke-load-tests.sh`, `tests/Load/run-average-load-tests.sh`, `tests/Load/run-stress-load-tests.sh`, `tests/Load/run-spike-load-tests.sh`, `tests/Load/run-load-tests.sh`, `tests/Load/results/.gitignore`.
Touches: none.
Implements: FR78, FR79, FR80, FR81, FR82, FR83, NFR1, NFR25, G24.
Reference: architecture-delivery.md "Load Testing" ("Layout of `tests/Load`", "`config.json.dist`", "Results", "JavaScript lint"), DD-25, "Command Surface" (load-test targets; `scenario`, `runSmoke`, `runAverage`, `runStress`, `runSpike`). Source files to port: `${PHP_TEMPLATE_DIR}/tests/Load/`.
Pitfalls: PF-66.
Verify: `make lint-load-tests`; `make lint-shell`; `make lint-prettier`; `tests/Load/get-load-test-scenarios.sh`; `tests/Load/execute-load-test.sh ""` (expected exit status 2).

**Acceptance Criteria:**

**Given** the kit
**When** `tests/Load/get-load-test-scenarios.sh` is run from any working directory
**Then** it prints `health`, and a new file `tests/Load/scripts/<name>.js` appears in the sorted list with no other edit

**Given** `tests/Load/config.json.dist`
**When** it is compared with the listing in "`config.json.dist`"
**Then** it is equal: one endpoint `health`, the PHP load shapes (smoke 10 requests per second for 10 s with 5 virtual users; average 25; stress 150; spike 200), the budgets 50, 200, 1000 and 5000 ms of DD-25 (smoke derived from the hosted measurement) and `delayBetweenScenarios` 60

**Given** `scripts/health.js`
**When** it is compared with the PHP file
**Then** it is unchanged: it requests `GET /api/health` and checks `is status 204`, and it is the only file under `tests/Load/scripts/`

**Given** an empty or unknown scenario name
**When** `tests/Load/execute-load-test.sh ""` and `tests/Load/execute-load-test.sh nope` are run
**Then** each exits 2 and prints `Usage: execute-load-test.sh <scenario> [runSmoke] [runAverage] [runStress] [runSpike] [htmlPrefix]` before any container starts

**Given** a git-ignored `tests/Load/config.json` in which the smoke budget of `health` is `-1`
**When** `tests/Load/execute-load-test.sh health true false false false` is run with nothing listening on the target port
**Then** K6, started from the image pinned as `K6_IMAGE` with the invoking user's uid, stops before sending a request with `Invalid threshold value` and the script exits non-zero

**Given** the JavaScript and shell files of the kit
**When** `make lint-load-tests`, `make lint-prettier` and `make lint-shell` are run
**Then** each exits 0; no script uses `eval`; each runner fails with `Error: No load test scenarios found.` when the scenario list is empty; reports are written to `tests/Load/results/<prefix><scenario>.html` with the prefixes `smoke-`, `average-`, `stress-`, `spike-` and `all-`, and `git check-ignore tests/Load/results/smoke-health.html tests/Load/config.json` lists both paths

### Story 3.5: AWS load-test runner with LocalStack dry run

As a platform maintainer,
I want `make aws-load-tests` and `make aws-load-tests-cleanup` with every wait bounded and a dry run against LocalStack,
So that cloud load tests can be started and removed safely and the flow is testable without AWS credentials.

Track: C.
Depends on: 3.1, 3.4.
Owns: `tests/Load/aws-execute-load-tests.sh`, `tests/Load/cleanup.sh`, `tests/Load/config.sh`, `tests/Load/user-data.sh`, `tests/Load/trust-policy.json`, `tests/Load/s3-bucket-policy.json`, `tests/Load/s3-access-policy.json`.
Touches: none.
Implements: FR85.
Reference: architecture-delivery.md "Load Testing" ("AWS runner"), DD-25, "Command Surface" (targets `aws-load-tests`, `aws-load-tests-cleanup`; variables `LOCAL_MODE_ENV`, `region`, `ami`, `instance_type`, `instance_tag`, `role`, `branch`, `security_group`, `load_target`). Source files to port: `${PHP_TEMPLATE_DIR}/tests/Load/`.
Verify: `make start-deps`; `make aws-load-tests LOCAL_MODE_ENV=true`; `make aws-load-tests-cleanup LOCAL_MODE_ENV=true`; `make lint-shell`; `make lint-prettier`.

**Acceptance Criteria:**

**Given** `make start-deps` has started LocalStack with the development override (services `sqs,ec2,s3,iam,sts`)
**When** `make aws-load-tests LOCAL_MODE_ENV=true` is run
**Then** it exits 0 and prints `Launched instance:` and `You can access the S3 bucket here:`, every AWS call went to `http://localhost:$LOCALSTACK_PORT`, and `tests/Load/aws-run.env` (git-ignored) records the run id and the names of the bucket, role, instance profile, policy, security group and tag

**Given** that dry run has finished
**When** `make aws-load-tests-cleanup LOCAL_MODE_ENV=true` is run, and then run a second time
**Then** both exit 0, the first removes every resource named in `aws-run.env` and the file itself, skipping a named resource that no longer exists, and the second finds no `aws-run.env` and prints `Nothing to clean up: tests/Load/aws-run.env not found.`

**Given** the scripts
**When** they are read
**Then** every wait loop has a numeric limit with a comment that explains it (bucket policy: 53 attempts, 5 s apart; result wait: 45 minutes, every 30 s); the repository URL comes from `git remote get-url origin` and the branch from `git rev-parse --abbrev-ref HEAD`; the AMI is resolved from the SSM public parameter of the chosen region; `s3-access-policy.json` is rendered with `sed` into the temporary directory mounted at `/work`; every AWS call runs through the shell function `AWS_CLI` of `config.sh` (`docker run ... "$AWS_CLI_IMAGE"`), so neither `aws`, `uuidgen` nor `envsubst` is required on the host; the run id is 8 hexadecimal characters from `/dev/urandom`; no file contains the literal repository name
**And** `user-data.sh` installs an `EXIT` trap that uploads the results, writes `DONE` or `FAILED` and shuts the instance down, so the instance terminates on success and on failure

**Given** the Make parameters
**When** `make -n aws-load-tests region=eu-west-1 instance_type=c6i.large branch=x` is run
**Then** the printed command passes `-r eu-west-1 -t c6i.large -b x` and omits the options whose parameter is empty

**Given** the shell and JSON files
**When** `make lint-shell` and `make lint-prettier` are run
**Then** both exit 0

### Story 3.6: Commit convention, git hooks and release tooling

As a platform maintainer,
I want the VilnaCRM commit form checked by one script that the hook and CI share, hooks that call Make targets only, and a release script driven by conventional commits,
So that every commit on `main` can be turned into a version, a changelog entry and a tag without manual steps.

Track: C.
Depends on: 1.4.
Owns: `.githooks/commit-msg`, `.githooks/pre-commit`, `.githooks/pre-push`, `scripts/check-commit-message.sh`, `cog.toml`, `.cog/changelog.tera`, `CHANGELOG.md`, `scripts/set-version.sh`, `scripts/release.sh`.
Touches: none.
Implements: FR98, FR102, FR103, G28.
Reference: architecture-delivery.md "Git Hooks and Commit Convention", DD-11, DD-12, DD-15, "CI/CD Workflows" ("`release.yml` — name `Release`": step 6 and the version rules, `cog.toml`, `.cog/changelog.tera`), "Command Surface" (targets `install-hooks`, `pre-commit`, `pre-push`, `check-commit-message`; script rows `check-commit-message.sh`, `set-version.sh`, `release.sh`).
Pitfalls: PF-07, PF-42, PF-62, PF-63.
Verify: `COMMIT_TITLE='feat(#12): add health endpoint' make check-commit-message`; `make -n pre-commit`; `make -n pre-push`; `make lint-shell`; `make lint-toml`; `cog --version` (expected `cog 7.0.0`); the scratch-repository runs of the acceptance criteria below.

**Acceptance Criteria:**

**Given** the titles `feat(#12): add health endpoint` and `FIX(#7): handle empty page`
**When** each is passed as `COMMIT_TITLE` to `make check-commit-message`
**Then** it exits 0
**And** for each of `feat: add x`, `feat(api): add x`, `feature(#12): add x`, `feat(#12):missing space` and `chore(release): v1.2.0 [skip ci]` it exits 1 and prints `The commit message should be in the format: feat|fix|refactor|build|chore|ci|docs|style|perf|test(#1): description` and `got: <subject>`; with neither `COMMIT_TITLE` nor `COMMIT_MSG_FILE` set it exits 2

**Given** hook mode (`COMMIT_MSG_FILE` pointing at a message file)
**When** the first non-comment line starts with `Merge `, `Revert "`, `Reapply "`, `fixup! `, `squash! ` or `amend! `
**Then** the script exits 0 without checking the pattern, and a non-matching ordinary subject still exits 1

**Given** `make install-hooks` was run in a clone
**When** `git config core.hooksPath` is read and the three hook files are inspected
**Then** the value is `.githooks`; each hook is an executable POSIX shell file that chooses the GNU Make to run (`$MAKE` when it is set, else `gmake` when it exists, else `make`) and calls only that make (`check-commit-message`, `pre-commit`, `pre-push`)
**And** `make -n pre-commit` shows `format-check`, `lint-code`, `architecture`, `check-security-offline` and `unused-dependencies` and no target that needs a container or the network, and `make -n pre-push` shows `unit-tests`

**Given** a temporary clone of the repository with a local bare remote, cocogitto 7.0.0 and a stub `gh` on `PATH`, and one new commit `feat(#1): first feature` (`make install` installs cocogitto at `COCOGITTO_VERSION` of `make/tool-versions.mk` through `scripts/install-tools.sh`; `tests/CLI/bats/make_release.bats` runs these cases in such a clone)
**When** `scripts/release.sh` is run in it
**Then** it exits 0; the head of `main` on the remote has the subject `chore(release): v0.1.0 [skip ci]`; tag `v0.1.0` points to that commit; `CHANGELOG.md` contains a heading `## v0.1.0 (` with `### Features` and keeps its header; `Cargo.toml` and `Cargo.lock` state the version and `cargo metadata --locked` succeeds; the stub `gh` was called with `release create v0.1.0`
**And** in a clone without any `v*` tag whose only new commit is `fix(#1): first fix`, `scripts/release.sh` also releases `v0.1.0` (the manifest version), and `scripts/release.sh --version 1.0.0` releases `v1.0.0`

**Given** such a clone whose only new commits are `build(deps): bump x` and `docs(#2): y`
**When** `scripts/release.sh` is run
**Then** it prints cocogitto's no-bump sentence, creates no commit and no tag, and exits 0

**Given** `cog.toml`, `.cog/changelog.tera` and `CHANGELOG.md`
**When** they are read
**Then** they equal the listings (tag prefix `v`, `[skip ci]`, the pre-bump hook `scripts/set-version.sh {{version}}`, the seven commit types omitted from the changelog); the template prints no link and no repository name; `CHANGELOG.md` holds only cocogitto's header and footer

### Story 3.7: Architecture model and its validation gate

As a backend engineer,
I want `workspace.dsl` to describe the service with system-context, container and component views and a gate that compares its components with the workspace crates,
So that the diagrams I browse locally cannot drift from the code.

Track: C.
Depends on: 1.4.
Owns: `workspace.dsl`, `scripts/validate-architecture-model.sh`.
Touches: none.
Implements: FR119, FR120, NFR18, G27.
Reference: architecture-delivery.md "Architecture Model" (the DSL listing and the decision table), DD-05, "Command Surface" (target `validate-architecture-model`, which pipes the crate list into the script; script row `validate-architecture-model.sh` with the signature `<dsl-file> <crate-list-file>`).
Pitfalls: PF-54.
Verify: `make validate-architecture-model`; `make lint-shell`; `git status --porcelain` (expected: no `workspace.json`, no `.structurizr/`).

**Acceptance Criteria:**

**Given** `workspace.dsl` at the repository root
**When** `make validate-architecture-model` is run
**Then** it exits 0: the Structurizr image pinned in `make/tool-versions.mk` parses the file, the JSON export under `target/reports/structurizr/` contains `systemContextViews`, `containerViews` and `componentViews`, and the names of the elements shown in the component view, read from the export, equal the workspace crates that `cargo tree --locked --workspace --depth 0 --prefix none` lists on the script's standard input, minus `xtask`, `test-support` and `e2e-tests` (ten components), and every workspace-internal dependency between two modelled crates has a relationship in the model (`architecture model OK: 10 components match the workspace crates; every dependency between them has a relationship`)

**Given** the component `some-module-infrastructure` temporarily removed from `workspace.dsl`
**When** `make validate-architecture-model` is run
**Then** it exits non-zero and names the missing component; with a line that does not parse appended instead, it exits non-zero with the parser's error; `git checkout -- workspace.dsl` restores the file. The remaining rule kinds (each view kind missing, an extra component, a component excluded from the component view, a dependency between two modelled crates without a relationship) are tested at script level in `check_architecture_model.bats` (Story 9.10)

**Given** the model
**When** it is read
**Then** PostgreSQL is a container outside the service binary, the message broker is an external software system, components are grouped as `Shared`, `Internal/HealthCheck` and `CompanySubdomain/SomeModule` with one group per layer, and no element exists that the workspace does not contain

**Given** the validation ran
**When** `git status --porcelain` is run
**Then** it lists no generated file: nothing is written next to `workspace.dsl`

## Epic 4: Shared Kernel and Platform Adapters

A backend engineer gets the shared kernel every bounded context builds on: aggregate root and domain events, typed command, query and event buses with in-process adapters, validated value objects, the problem-document error model with problem-aware extractors, seeded test doubles, and the PostgreSQL, cache and SQS adapters with explicit timeouts and reversible migrations, each proven by unit and integration tests.

### Story 4.1: Shared domain kernel

As a backend engineer,
I want the framework-free shared domain crate with aggregate root, domain events, bus ports with typed dispatch, technical ports and the UUID and page value objects,
So that every bounded context shares one tested vocabulary and no Domain crate needs a framework.

Track: A.
Depends on: 1.2, 1.3.
Owns: `crates/shared/domain/src/error.rs`, `crates/shared/domain/src/aggregate.rs`, `crates/shared/domain/src/aggregate/`, `crates/shared/domain/src/event.rs`, `crates/shared/domain/src/bus.rs`, `crates/shared/domain/src/bus/`, `crates/shared/domain/src/port.rs`, `crates/shared/domain/src/port/`, `crates/shared/domain/src/value_object.rs`, `crates/shared/domain/src/value_object/`.
Touches: `crates/shared/domain/src/lib.rs` (module declarations and re-exports).
Implements: FR22, FR25, FR27, FR29, FR34, FR37, FR50, G11.
Reference: architecture.md "Domain and Application Design" ("Shared kernel: `shared-domain`": every signature is normative, and every port trait has `Debug` as a supertrait), AD-02, AD-05, AD-21, "Implementation Patterns and Consistency Rules" (all rules; "Code shape imposed by the tools" rules 21 to 23), "Test Architecture" (required-test row FR26 for `bus/event/tests.rs`).
Pitfalls: PF-21, PF-36, PF-38.
Verify: `make format-check`; `make lint-code`; `make lint-rustdoc`; `make complexity`; `make unit-tests`; `make check-crate-isolation`; `cargo tree --locked -p shared-domain --edges normal --depth 1 --prefix none`; `cargo llvm-cov nextest --locked -p shared-domain --profile ci --summary-only`; `env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --cargo-arg=--locked -p shared-domain`.

**Acceptance Criteria:**

**Given** the crate
**When** `make unit-tests` is run
**Then** the tests in the sibling `tests.rs` files pass and show: `Uuid::parse` accepts every syntactically valid UUID of any version and rejects other input with `InvalidUuid` carrying the input; `PageRequest::new` accepts every page from 1 (including `u32::MAX`) and items per page 1 to 100, and rejects page `0` with `InvalidPageRequest::Page` and items per page `0` and `101` with `InvalidPageRequest::ItemsPerPage`; `limit` and `offset` return `items_per_page` and `(page - 1) * items_per_page` computed in `i64`
**And** a property test shows that `Uuid::parse(x.to_string())` round-trips for generated UUIDs

**Given** an aggregate that records three events
**When** `pull_domain_events` is called twice
**Then** the first call returns the three events in recording order and the second returns an empty list; `DomainEvent` exposes event id, aggregate id, occurrence time and a stable event name

**Given** test implementations of `CommandBus` and `QueryBus` inside the crate's own `tests.rs` files
**When** `dispatch` and `ask` are called through `&dyn CommandBus` and `&dyn QueryBus`
**Then** a handler result is returned with its static type, a handler error arrives as `BusError::Handler(E)` without a downcast at the call site, and a missing handler arrives as `BusError::NotRegistered { message }` whose text names the message (`no handler is registered for ...`)

**Given** `bus/event.rs`
**When** its tests run
**Then** the messages of `SubscriberFailure` and `EventBusError` are asserted and an `EventBusError` carries every subscriber failure it was built with

**Given** the manifest and the sources
**When** `make check-crate-isolation`, `make lint-code`, `make lint-rustdoc`, `make complexity` and the `cargo tree` command of the Verify line are run
**Then** each exits 0; `cargo tree` prints exactly the crate and its dependencies `async-trait`, `thiserror` and `uuid` (the complete architecture gate of Story 2.2 checks the crate again when that story is verified); no source file mentions `serde`, `axum`, `sqlx`, `tokio::` or `SystemTime::now`; every `#[async_trait]` method body is a single delegating call

**Given** the production files this story owns (every file of `crates/shared/domain/src/` except `lib.rs`)
**When** the coverage command and the mutation command of the Verify line are run
**Then** each of those files shows 100% lines and functions in the summary and cargo-mutants exits 0 with no missed and no timed-out mutant; a line or mutant that only code of a later story can reach is listed in the completion notes with that story's id

### Story 4.2: Seeded test data and kernel test doubles

As a backend engineer,
I want a test-support crate with a seeded generator that prints its seed on failure and hand-written doubles for the clock, the UUID generator, the three buses, the cache and the log,
So that unit tests need no network or wall clock and a failing randomised test can be reproduced.

Track: A.
Depends on: 4.1.
Owns: `tests/support/src/seed.rs`, `tests/support/src/clock.rs`, `tests/support/src/id.rs`, `tests/support/src/bus.rs`, `tests/support/src/cache.rs`, `tests/support/src/log.rs`.
Touches: `tests/support/src/lib.rs` (module declarations for the six files).
Implements: FR50, FR51, NFR29.
Reference: architecture.md AD-23, "Test Architecture" ("Seeded random data", "Test doubles (crate `test-support`)": rows `FixedClock`, `SequenceUuidGenerator`, `FakeCommandBus`, `FakeQueryBus`, `RecordingEventBus`, `StubCache`, and the `LogRecorder` of the tree), "Implementation Patterns and Consistency Rules" (rules 5, 6, 18, 20), "Quality Gates" ("`.cargo/config.toml`": the recorded test seeds).
Pitfalls: PF-36, PF-47.
Verify: `make format-check`; `make lint-code`; `make lint-rustdoc`; `make unit-tests`; `make check-crate-isolation`; `cargo tree --locked -p test-support --edges normal --depth 1 --prefix none`.

**Acceptance Criteria:**

**Given** `TEST_SEED=42` in the environment
**When** two `TestRng::from_env()` instances each draw ten values
**Then** both sequences are equal, and with the variable unset inside a cargo run the seed recorded in `.cargo/config.toml` is used, so a default run is reproducible

**Given** a test that creates a `TestRng` and then panics
**When** it is run with `TEST_SEED=7`
**Then** its captured stderr contains the line `TEST_SEED=7`
**And** `seed.rs` is the only file of the crate that reads the environment, with one `#[expect(clippy::disallowed_methods, reason = "...")]`

**Given** the doubles
**When** they are used through their port traits
**Then** `FixedClock::at(t)` always returns `t`; `SequenceUuidGenerator::starting_at(n)` returns distinct version-7-shaped UUIDs in increasing order; `FakeCommandBus::handling(closure)` records each command and answers with the closure's result while `::unregistered()` answers `NotRegistered`; `FakeQueryBus` behaves alike; `RecordingEventBus::new()` records published events and `::failing()` returns an `EventBusError`; `StubCache` has the four variants `working`, `failing`, `forgetful` and `corrupting`

**Given** `LogRecorder` installed for the current thread
**When** code logs `tracing::error!(event_id = %id, "event publication failed")`
**Then** the recorder returns one `LogLine` with level `ERROR`, that message and the field `event_id`, and events of other threads are not recorded

**Given** the crate
**When** `make check-crate-isolation`, `make lint-code`, `make lint-rustdoc` and the `cargo tree` command of the Verify line are run
**Then** each exits 0; `cargo tree` prints exactly the crate, the three Domain crates and `async-trait`, `fake` (with its `uuid` feature), `rand`, `tracing` and `uuid`; no UUID is produced with `Faker.fake::<Uuid>()`; the crate is test code, so it has no coverage or mutation command of its own

### Story 4.3: Problem documents, problem-aware extractors and request budget

As an API client,
I want every error of the service to be an `application/problem+json` document with a stable shape,
So that I can handle validation failures, unknown routes, wrong methods, oversized bodies, timeouts and internal failures uniformly.

Track: A.
Depends on: 2.2, 4.2.
Owns: `crates/shared/application/src/http.rs`, `crates/shared/application/src/http/`, `crates/shared/application/src/graphql.rs`, `crates/shared/application/src/graphql/`.
Touches: `crates/shared/application/src/lib.rs` (module declarations), `crates/shared/application/Cargo.toml` (the dev-dependency `tokio` gains the feature `test-util`, for tokio's paused clock).
Implements: FR6, FR7, FR13, NFR15.
Reference: architecture.md AD-06 (the status table is normative), AD-15, AD-22, "Domain and Application Design" ("HTTP layer": `problem.rs`, `extract.rs`, `fallback.rs`, `middleware.rs`, `graphql.rs`), "API Contract" ("Schemas and validation rules": `Problem`, `Violation`, `DependencyFailure`; "Problem examples"), "Implementation Patterns and Consistency Rules" ("Error handling" rules 9 to 13).
Pitfalls: PF-12, PF-14, PF-19.
Verify: `make format-check`; `make lint-code`; `make lint-rustdoc`; `make complexity`; `make unit-tests`; `make architecture`; `make check-crate-isolation`; `cargo llvm-cov nextest --locked -p shared-application --profile ci --summary-only`; `env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --cargo-arg=--locked -p shared-application`.

**Acceptance Criteria:**

**Given** each constructor and conversion of `Problem`
**When** the resulting response is inspected in unit tests
**Then** the status, `content-type: application/problem+json`, `type`, `title` and `detail` equal the rows of the AD-06 table; `Problem::validation_failed` returns status 422 with type `/problems/validation-failed`, detail `The request failed validation.` and violations sorted by `propertyPath` then `code`; `Problem::dependency_unavailable` returns status 500 with type `/problems/dependency-unavailable` and the messages joined with `; `
**And** `std::mem::size_of::<Problem>()` is below 128 bytes

**Given** `Problem::internal(&error)` called with an error chain that contains the text `SELECT secret FROM t`
**When** the response body and the `LogRecorder` output are inspected
**Then** the body is exactly `{"type":"about:blank","title":"Internal Server Error","status":500}` and one entry at `ERROR` carries the error chain

**Given** a test router that uses `ApiJson`, `ApiPath`, `ValidatedJson` and `ValidatedQuery`
**When** it receives malformed JSON, a body without `content-type: application/json`, a body with an unknown field, a body with two violated constraints, a path parameter of the wrong type and a query parameter that is not an integer
**Then** the responses are problems with status 400, 415, 422 (`about:blank`), 422 (`/problems/validation-failed` with two violations in stable order and camelCase `propertyPath`), 400 and 400

**Given** a body larger than the limit set with `DefaultBodyLimit`
**When** it is sent to a `ValidatedJson` route
**Then** the response is the 413 problem

**Given** `not_found`, `method_not_allowed`, `panic_response` and `request_timeout` with a 50 ms budget under tokio's paused clock
**When** each is exercised
**Then** they produce the 404 problem with detail `No route matches the request path.`, the 405 problem, the 500 problem without detail, and the 503 problem with detail `The request exceeded its time budget.`

**Given** `problem_to_error`
**When** it converts problems with status 422, 404 and 500
**Then** `extensions.code` is `VALIDATION_FAILED` with the detail as message and `extensions.violations` for 422, and `INTERNAL_SERVER_ERROR` with the message `Internal server error` for every other status (no other code exists)

**Given** the production files this story owns (the files under `crates/shared/application/src/http/` and `src/graphql.rs`)
**When** the coverage command and the mutation command of the Verify line are run
**Then** each of those files shows 100% lines and functions in the summary and cargo-mutants exits 0 with no missed and no timed-out mutant; a line or mutant that only code of a later story can reach is listed in the completion notes with that story's id

### Story 4.4: In-process buses, cache, clock and UUID generator

As a backend engineer,
I want working in-process adapters for the command, query and event buses, an in-process cache, the system clock and a UUIDv7 generator,
So that handlers and subscribers are registered explicitly and every shared-kernel port has an implementation.

Track: A.
Depends on: 4.1.
Owns: `crates/shared/infrastructure/src/bus.rs`, `crates/shared/infrastructure/src/bus/`, `crates/shared/infrastructure/src/cache.rs`, `crates/shared/infrastructure/src/cache/`, `crates/shared/infrastructure/src/clock.rs`, `crates/shared/infrastructure/src/id.rs`, `crates/shared/infrastructure/tests/integration/main.rs`, `crates/shared/infrastructure/tests/integration/clock.rs`, `crates/shared/infrastructure/tests/integration/id.rs`, `crates/shared/infrastructure/tests/integration/cache.rs`.
Touches: `crates/shared/infrastructure/src/lib.rs` (module declarations).
Implements: FR26, FR27, FR28, FR29, FR34, NFR28.
Reference: architecture.md AD-05, AD-09, AD-21, "Domain and Application Design" ("In-process adapters: `shared-infrastructure`"), "Implementation Patterns and Consistency Rules" ("Code shape imposed by the tools" rule 22; "Forbidden constructs"), "Test Architecture" (required-test rows FR26/PD-19 and NFR28).
Pitfalls: PF-21, PF-36.
Verify: `make format-check`; `make lint-code`; `make lint-rustdoc`; `make complexity`; `make unit-tests`; `cargo nextest run --locked -p shared-infrastructure --test integration`; `make check-crate-isolation`; `cargo tree --locked -p shared-infrastructure --edges normal --depth 1 --prefix none`; `cargo llvm-cov nextest --locked -p shared-infrastructure --profile ci --summary-only`; `env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --cargo-arg=--locked -p shared-infrastructure`.

**Acceptance Criteria:**

**Given** an `InMemoryCommandBus` and an `InMemoryQueryBus` with one registered handler each
**When** a registered message and an unregistered message are dispatched through the typed helpers
**Then** the registered one reaches exactly its handler and returns the handler's result or typed error, and the unregistered one returns `BusError::NotRegistered` naming the message
**And** registering a second handler for the same message returns `DuplicateHandler { message }` with the text `a handler is already registered for ...`

**Given** an `InMemoryEventBus` with three subscribers of one event, of which the first and second fail
**When** the event is published
**Then** all three are invoked in registration order and the returned `EventBusError` holds both failures in invocation order with event name and event id
**And** publishing an event that has no subscriber returns `Ok(())`

**Given** a `MokaCache`
**When** a value is set with a TTL and read back, and (integration test, real time) read again after the TTL
**Then** the first read returns the value and the later read returns `None`; the test holds the cache as `Arc<dyn Cache>`, so code that uses the port does not name the adapter

**Given** `SystemClock` and `UuidV7Generator`
**When** the integration tests `clock.rs` and `id.rs` run
**Then** two readings of the clock do not go backwards and lie after 2026-01-01, and generated UUIDs are distinct, version 7 and ordered
**And** `grep -rn "SystemTime::now\|now_v7" crates --include=*.rs` matches only `clock.rs` and `id.rs` of this crate, each call with a reasoned `#[expect]` where the banned-API lint requires one

**Given** the three bus types and `MokaCache`
**When** they are formatted with `{:?}` in a unit test
**Then** the hand-written `Debug` output names the type and prints no handler or cached value; `make lint-code`, `make complexity` and `make check-crate-isolation` exit 0, and the `cargo tree` command of the Verify line prints exactly the crate, `shared-domain` and the third-party dependencies of its row of the crate table

**Given** the production files this story owns (`bus.rs`, the files under `bus/`, `cache.rs`, `clock.rs` and `id.rs` of `crates/shared/infrastructure/src/`)
**When** the coverage command and the mutation command of the Verify line are run
**Then** each of those files shows 100% lines and functions in the summary and cargo-mutants exits 0 with no missed and no timed-out mutant; a line or mutant that only code of a later story can reach is listed in the completion notes with that story's id

### Story 4.5: PostgreSQL pool, reversible migrations, SQS client and test database

As an operator,
I want a lazily connected PostgreSQL pool and an SQS client with explicit timeouts, the first reversible migration embedded in the binary, and one command that rebuilds the test database,
So that the service starts while a dependency is down, schema changes can be reverted and tests never touch development data.

Track: A.
Depends on: 2.2, 3.1, 4.4.
Owns: `crates/shared/infrastructure/build.rs`, `crates/shared/infrastructure/src/persistence.rs`, `crates/shared/infrastructure/src/messaging.rs`, `crates/shared/infrastructure/tests/integration/support.rs`, `crates/shared/infrastructure/tests/integration/persistence.rs`, `crates/shared/infrastructure/tests/integration/messaging.rs`, `migrations/`, `scripts/setup-test-db.sh`.
Touches: `crates/shared/infrastructure/src/lib.rs` and `crates/shared/infrastructure/tests/integration/main.rs` (module declarations).
Implements: FR13, FR30, FR32, FR33, FR35, FR52, FR53, FR126, NFR4, NFR32, G12.
Reference: architecture.md AD-08, AD-10, AD-20, "Domain and Application Design" ("In-process adapters: `shared-infrastructure`": `persistence.rs`, `messaging.rs`), "Data Architecture" (all subsections: "Tables", "Migration files", "Offline query metadata", "Test database strategy"), "Configuration" ("Committed environment files"); architecture-delivery.md "Command Surface" (targets `setup-test-db`, `integration-tests`, `create-migration`; script row `setup-test-db.sh`).
Pitfalls: PF-03, PF-04, PF-05, PF-25, PF-28, PF-29, PF-30.
Verify: `make start-deps`; `make setup-test-db`; `make integration-tests`; `make unit-tests`; `make format-check`; `make lint-code`; `make complexity`; `make lint-shell`; `make architecture`; `make check-crate-isolation`; `scripts/with-env.sh .env .env.test -- cargo llvm-cov nextest --locked -p shared-infrastructure --profile ci --summary-only`; `scripts/with-env.sh .env .env.test -- env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --cargo-arg=--locked -p shared-infrastructure`.

**Acceptance Criteria:**

**Given** `make start-deps` has made `database` healthy
**When** `make setup-test-db` is run twice
**Then** each run exits 0 after dropping, creating and migrating the database `app_test` with exactly `sqlx database reset -y -f --source migrations`, and the development database `app` is not touched (a marker table created in `app` beforehand still exists)
**And** `DATABASE_URL=postgres://app:unused@127.0.0.1:1/app_test scripts/setup-test-db.sh` exits 2 after 10 attempts (a dead port; no container is stopped), and with `sqlx` absent from `PATH` it exits 2 naming the tool

**Given** the migration pair `migrations/20261110000000_create_example_api_resources.{up,down}.sql`
**When** the integration test `persistence.rs` applies all migrations, reverts all and re-applies all on a fresh database
**Then** the catalog (`information_schema.columns` and `pg_constraint`) after the first and the second apply is identical, the table is `example_api_resources` with the columns and named constraints of "Tables", and applying when nothing is pending changes nothing

**Given** `connect_lazy` with the URL `postgres://app:unused@127.0.0.1:1/app` and a 500 ms acquire timeout
**When** the pool is created and then a query is attempted
**Then** creation succeeds without network access and the query fails within the acquire timeout plus 1 s; against the real database a query succeeds and each connection has the configured `statement_timeout`

**Given** `sqs_client` built from `SqsSettings`
**When** the integration test `messaging.rs` calls `ListQueues` against LocalStack and against the endpoint `http://127.0.0.1:1`
**Then** the first call succeeds and the second returns an error within the operation timeout plus 1 s with `max_attempts` respected
**And** the client uses the explicit region, the optional endpoint URL, the default credential chain and `aws-config`/`aws-sdk-sqs` without default features

**Given** `build.rs`
**When** a new file is added under `migrations/`
**Then** cargo rebuilds the crate (`cargo:rerun-if-changed=../../../migrations` is printed by the build script) and `MIGRATOR` embeds the directory with `sqlx::migrate!`

**Given** the production files this story owns (`persistence.rs` and `messaging.rs`; `build.rs` is outside both scopes)
**When** the coverage command and the mutation command of the Verify line are run
**Then** each of those files shows 100% lines and functions in the summary and cargo-mutants exits 0 with no missed and no timed-out mutant; a line or mutant that only code of a later story can reach is listed in the completion notes with that story's id

## Epic 5: Dependency Health Check

An operator gets the health-check bounded context: three probes for PostgreSQL, the cache and the message broker that run concurrently under their own timeouts, and a `GET /api/health` handler that answers 204 or a 500 problem naming every failing dependency with the exact PHP failure strings, proven with test doubles and against the real services.

### Story 5.1: Health orchestration and the health endpoint handler

As an operator,
I want the health service to run all probes concurrently, count a timed-out probe as failed and report every failing dependency by its exact message,
So that one request tells me which dependencies are down without leaking the cause.

Track: A.
Depends on: 4.2, 4.3.
Owns: `crates/internal/health-check/domain/src/port.rs`, `crates/internal/health-check/domain/src/value_object.rs`, `crates/internal/health-check/domain/src/value_object/`, `crates/internal/health-check/application/src/service.rs`, `crates/internal/health-check/application/src/service/`, `crates/internal/health-check/application/src/http.rs`, `crates/internal/health-check/application/src/http/`, `tests/support/src/probe.rs`.
Touches: `crates/internal/health-check/domain/src/lib.rs`, `crates/internal/health-check/application/src/lib.rs` and `tests/support/src/lib.rs` (module declarations).
Implements: FR14, FR15, FR17, FR54, FR71, NFR2, NFR30.
Reference: architecture.md AD-11 (port name, concurrency, timeouts, aggregation, failure strings, response bodies, wiring), "Domain and Application Design" ("Health-check context"), "API Contract" ("REST operations": row and summary of `api_health_get`; "Problem examples"), "Observability and Operations" ("Log line schema": `health probe failed`), "Test Architecture" ("Test doubles (crate `test-support`)": `StubProbe`).
Pitfalls: PF-17, PF-23, PF-36.
Verify: `make format-check`; `make lint-code`; `make lint-rustdoc`; `make complexity`; `make unit-tests`; `make architecture`; `make check-crate-isolation`; `cargo llvm-cov nextest --locked -p health-check-domain -p health-check-application --profile ci --summary-only`; `env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --cargo-arg=--locked -p health-check-domain -p health-check-application`.

**Acceptance Criteria:**

**Given** `Dependency`
**When** its unit tests run
**Then** the identifiers are `database`, `cache` and `message_broker` and the failure messages are exactly `Database is not available`, `Cache is not working` and `Message broker is not available`

**Given** a `HealthService` built from three `StubProbe` values
**When** `check` runs for the healthy case and for each of the 7 non-empty combinations of failing dependencies
**Then** the healthy report has no failure and each other report holds exactly the failing dependencies in the order database, cache, message broker

**Given** a probe built with `StubProbe::hanging` and a probe timeout of 3 s under tokio's paused clock
**When** `check` runs
**Then** it completes when the virtual clock reaches the timeout, that dependency is reported with cause `TimedOut`, the other probes were polled concurrently (total virtual time equals the slowest probe, not the sum), and one `health probe failed` entry at `WARN` per failure carries `dependency` and `cause` (`LogRecorder`)

**Given** the router of `health-check-application` with a state whose probes are all healthy
**When** `GET /health` is dispatched with `tower::ServiceExt::oneshot`
**Then** the status is 204 and the body is empty

**Given** a state in which the database and the message broker probes fail
**When** the same request is dispatched
**Then** the status is 500, the media type is `application/problem+json`, `type` is `/problems/dependency-unavailable`, `detail` is `Database is not available; Message broker is not available`, `failures` has exactly those two entries with `dependency` and `message`, and the body contains neither `Cache is not working` nor the text of the underlying cause

**Given** the `#[utoipa::path]` annotation of the handler
**When** the route's OpenAPI fragment is built in a unit test
**Then** it has operation id `api_health_get`, tag `HealthCheck`, the summary and description of the contract table, the responses `204` and `500` and a `default` problem response

**Given** the production files this story owns (`port.rs` and the files under `value_object/` of the Domain crate; `service.rs` and `http.rs` of the Application crate)
**When** the coverage command and the mutation command of the Verify line are run
**Then** each of those files shows 100% lines and functions in the summary and cargo-mutants exits 0 with no missed and no timed-out mutant; a line or mutant that only code of a later story can reach is listed in the completion notes with that story's id

### Story 5.2: PostgreSQL, cache and SQS probes

As an operator,
I want the three probes implemented against the real adapters: `SELECT 1`, a cache write and read-back, and one read-only SQS call,
So that the health endpoint reflects the real state of each dependency and never changes broker state.

Track: A.
Depends on: 4.5, 5.1.
Owns: `crates/internal/health-check/infrastructure/src/probe.rs`, `crates/internal/health-check/infrastructure/src/probe/`, `crates/internal/health-check/infrastructure/tests/integration/`.
Touches: `crates/internal/health-check/infrastructure/src/lib.rs` (module declaration).
Implements: FR13, FR16, FR54, NFR2, NFR4.
Reference: architecture.md AD-10, AD-11 ("Probes"), "Domain and Application Design" ("Health-check context": the three probe structs), "Workspace and Project Structure" (tree of `crates/internal/health-check/infrastructure/`), AD-10 ("Consequences": measurements on LocalStack 3.4.0).
Pitfalls: PF-25, PF-26, PF-30, PF-31.
Verify: `make setup-test-db`; `make integration-tests`; `make unit-tests`; `make format-check`; `make lint-code`; `make complexity`; `make architecture`; `make check-crate-isolation`; `scripts/with-env.sh .env .env.test -- cargo llvm-cov nextest --locked -p health-check-infrastructure --profile ci --summary-only`; `scripts/with-env.sh .env .env.test -- env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --cargo-arg=--locked -p health-check-infrastructure`.

**Acceptance Criteria:**

**Given** `CacheProbe` over each `StubCache` variant
**When** `check` runs in unit tests
**Then** it succeeds for `working` and fails for `failing`, `forgetful` (the value is not read back) and `corrupting` (a different value is read back); on every call the probe writes key `health_check` with value `ok` and a 60 s TTL, reads it and compares

**Given** `SqsProbe` with an `aws-smithy-mocks` client
**When** the mock answers `ListQueues` successfully and when it answers with a service error
**Then** `check` returns `Ok` and `Err` respectively, and the only operation the probe sends is `ListQueues` with `max_results(1)` (no create, change or delete call)

**Given** the compose services
**When** the integration tests `postgres_probe.rs` and `sqs_probe.rs` run
**Then** `PostgresProbe` succeeds against the test database with `SELECT 1` and `SqsProbe` succeeds against LocalStack 3.4.0, and the list of queues in LocalStack is the same before and after the probe

**Given** a pool for `postgres://app:unused@127.0.0.1:1/app` and an SQS client for `http://127.0.0.1:1`
**When** each probe's `check` runs
**Then** it returns an error within the acquire timeout plus 1 s and within the operation timeout plus 1 s, without any container being stopped

**Given** the crate
**When** `make architecture`, `make check-crate-isolation`, `make lint-code` and `make complexity` are run
**Then** each exits 0 and each probe returns the `Dependency` it stands for from `dependency()`

**Given** the production files this story owns (the three files under `crates/internal/health-check/infrastructure/src/probe/`)
**When** the coverage command and the mutation command of the Verify line are run
**Then** each of those files shows 100% lines and functions in the summary and cargo-mutants exits 0 with no missed and no timed-out mutant; a line or mutant that only code of a later story can reach is listed in the completion notes with that story's id

## Epic 6: Example Bounded Context

A backend engineer and an AI assistant get `CompanySubdomain/SomeModule` as a small, real vertical slice to copy from: an `ExampleApiResource` aggregate with validated value objects and a created event, the create command, the read and list queries, an audit subscriber, REST handlers and GraphQL resolvers over the same use cases, and a PostgreSQL repository with committed query metadata.

### Story 6.1: ExampleApiResource domain model

As a backend engineer,
I want the example aggregate, its value objects, its created event and its repository port in a framework-free Domain crate,
So that the invariants of the resource are enforced in one place and I have an exemplar for my own aggregates.

Track: A.
Depends on: 2.2, 4.2.
Owns: `crates/company-subdomain/some-module/domain/src/error.rs`, `crates/company-subdomain/some-module/domain/src/error/`, `crates/company-subdomain/some-module/domain/src/aggregate.rs`, `crates/company-subdomain/some-module/domain/src/aggregate/`, `crates/company-subdomain/some-module/domain/src/event.rs`, `crates/company-subdomain/some-module/domain/src/repository.rs`, `crates/company-subdomain/some-module/domain/src/value_object.rs`, `crates/company-subdomain/some-module/domain/src/value_object/`, `tests/support/src/repository.rs`, `tests/support/src/fixture.rs`.
Touches: `crates/company-subdomain/some-module/domain/src/lib.rs` and `tests/support/src/lib.rs` (module declarations).
Implements: FR20, FR25, FR31, FR37, FR50.
Reference: architecture.md "Domain and Application Design" ("Example bounded context": invariants and the `some-module-domain` signatures), AD-21, AD-22, "Implementation Patterns and Consistency Rules" ("Naming"), "Test Architecture" ("Test doubles (crate `test-support`)": `InMemoryExampleApiResourceRepository`; "Seeded random data": fixtures).
Pitfalls: PF-21, PF-36.
Verify: `make format-check`; `make lint-code`; `make lint-rustdoc`; `make complexity`; `make unit-tests`; `make architecture`; `make check-crate-isolation`; `cargo llvm-cov nextest --locked -p some-module-domain --profile ci --summary-only`; `env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --cargo-arg=--locked -p some-module-domain`.

**Acceptance Criteria:**

**Given** `ExampleApiResourceName::new` and `ExampleApiResourceDescription::new`
**When** unit tests pass strings of 0, 1, 255 and 256 Unicode scalar values to the name and of 0, 1, 1024 and 1025 to the description, a string of multi-byte characters at the limit, a whitespace-only string and a string containing U+0000
**Then** lengths are counted with `chars().count()`; the empty and over-long values are rejected with `NameLength` or `DescriptionLength`; the U+0000 value with `NameContainsNul` or `DescriptionContainsNul`; whitespace-only and limit-length values are accepted untrimmed

**Given** `InvalidExampleApiResource`
**When** `property_path`, `code` and the display text of each variant are read
**Then** they are `name` or `description`, `length` or `does_not_contain`, and the messages `This value must contain between 1 and 255 characters.`, `This value must contain between 1 and 1024 characters.` and `This value must not contain the NUL character.`

**Given** `ExampleApiResource::create` with an event id and an occurrence time
**When** `pull_domain_events` is called
**Then** it returns exactly one `ExampleApiResourceCreated` with that event id, the aggregate id, that time and the event name `example_api_resource.created`; `ExampleApiResource::restore` records no event

**Given** `InMemoryExampleApiResourceRepository`
**When** it is used through the `ExampleApiResourceRepository` port
**Then** `save` of an existing id returns `RepositoryError::AlreadyExists`, `find_page` returns items ordered by id descending within the page bounds, and the `failing()` variant returns `Unavailable` from every method

**Given** `test_support::fixture`
**When** `example_api_resource`, `example_name` and `example_description` are called with a `TestRng` of a fixed seed twice
**Then** both calls produce equal values that satisfy the domain invariants

**Given** the Domain crate
**When** `make architecture`, `make check-crate-isolation` and `make lint-code` are run
**Then** each exits 0 and the crate depends only on `shared-domain`, `async-trait` and `thiserror`

**Given** the production files this story owns (every file of `crates/company-subdomain/some-module/domain/src/` except `lib.rs`)
**When** the coverage command and the mutation command of the Verify line are run
**Then** each of those files shows 100% lines and functions in the summary and cargo-mutants exits 0 with no missed and no timed-out mutant; a line or mutant that only code of a later story can reach is listed in the completion notes with that story's id

### Story 6.2: Create command, read and list queries, audit subscriber

As a backend engineer,
I want the create command, the get and list queries and the audit subscriber as transport-free use cases behind the buses,
So that I have one exemplar each for the command path, the query path and the event path.

Track: A.
Depends on: 4.3, 6.1.
Owns: `crates/company-subdomain/some-module/application/src/error.rs`, `crates/company-subdomain/some-module/application/src/error/`, `crates/company-subdomain/some-module/application/src/command.rs`, `crates/company-subdomain/some-module/application/src/command/`, `crates/company-subdomain/some-module/application/src/query.rs`, `crates/company-subdomain/some-module/application/src/query/`, `crates/company-subdomain/some-module/application/src/subscriber.rs`, `crates/company-subdomain/some-module/application/src/subscriber/`.
Touches: `crates/company-subdomain/some-module/application/src/lib.rs` (module declarations).
Implements: FR20, FR21, FR22, FR23, FR27.
Reference: architecture.md "Domain and Application Design" ("Example bounded context": `some-module-application` signatures for `error.rs`, `command/`, `query/`, `subscriber/`), AD-05, AD-06, AD-08 (a create whose event publication fails after the save succeeds and is logged), "Implementation Patterns and Consistency Rules" ("Recipes": add a command, add a query, add an event subscriber; rule 21), "Observability and Operations" ("Log line schema": `domain event`, `event publication failed`), "Test Architecture" (required-test row PD-18, FR20(e)).
Pitfalls: PF-21, PF-36.
Verify: `make format-check`; `make lint-code`; `make lint-rustdoc`; `make complexity`; `make unit-tests`; `make architecture`; `make check-crate-isolation`; `cargo llvm-cov nextest --locked -p some-module-application --profile ci --summary-only`; `env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --cargo-arg=--locked -p some-module-application`.

**Acceptance Criteria:**

**Given** `CreateExampleApiResourceHandler` with `InMemoryExampleApiResourceRepository`, `RecordingEventBus::new()`, `FixedClock` and `SequenceUuidGenerator`
**When** a valid command is handled
**Then** the resource is stored, and exactly one `ExampleApiResourceCreated` event is published after the save with the event id from the generator and the occurrence time from the clock

**Given** the same handler with `InMemoryExampleApiResourceRepository::failing()`
**When** a valid command is handled
**Then** the result is `ExampleApiResourceError::Storage` and no event is published; an invalid name or description yields `Invalid` and nothing is stored; a duplicate id yields `AlreadyExists(id)`

**Given** the handler with `RecordingEventBus::failing()`
**When** a valid command is handled
**Then** it returns `Ok(())`, the resource is stored, and `LogRecorder` shows one `event publication failed` entry at `ERROR` with the event name, the event id and the error

**Given** the get and list query handlers over a repository holding three resources
**When** `GetExampleApiResource` is asked for a stored id and an unknown id, and `ListExampleApiResources` for page 1 with 2 items, page 2 with 2 items, page 9 and page 0
**Then** the answers are `Some(view)`, `None`, the two newest views, the remaining view, an empty list, and `ExampleApiResourceError::InvalidPage`; handlers return views, never aggregates

**Given** `AuditExampleApiResourceCreated`
**When** it handles an event
**Then** `LogRecorder` shows one entry at `INFO` with target `audit`, message `domain event` and the fields `event_name`, `event_id`, `aggregate_id` and `occurred_at` in RFC 3339 form

**Given** `From<ExampleApiResourceError> for Problem`
**When** each variant is converted
**Then** `Invalid` and `InvalidPage` become the 422 validation problem with one violation, and `AlreadyExists` and `Storage` both become the 500 problem without detail through `Problem::internal` (no 409 exists)
**And** `grep -rn "axum\|utoipa\|async_graphql" crates/company-subdomain/some-module/application/src/command crates/company-subdomain/some-module/application/src/query crates/company-subdomain/some-module/application/src/subscriber` prints nothing

**Given** the production files this story owns (`error.rs`, `command.rs`, `query.rs`, `subscriber.rs` and the files under `command/`, `query/` and `subscriber/`)
**When** the coverage command and the mutation command of the Verify line are run
**Then** each of those files shows 100% lines and functions in the summary and cargo-mutants exits 0 with no missed and no timed-out mutant; a line or mutant that only code of a later story can reach is listed in the completion notes with that story's id

### Story 6.3: REST handlers and GraphQL resolvers for the example resource

As an API client,
I want to create, read and list `ExampleApiResource` over REST and through three GraphQL operations that call the same commands and queries,
So that both transports apply one set of validation rules and I can use either.

Track: A.
Depends on: 6.2.
Owns: `crates/company-subdomain/some-module/application/src/http.rs`, `crates/company-subdomain/some-module/application/src/http/`, `crates/company-subdomain/some-module/application/src/graphql.rs`, `crates/company-subdomain/some-module/application/src/graphql/`.
Touches: `crates/company-subdomain/some-module/application/src/lib.rs` (module declarations).
Implements: FR7, FR9, FR19, FR20, FR21, FR22, FR24, FR71.
Reference: architecture.md "Domain and Application Design" ("HTTP layer": `SomeModuleState`, `router`, DTOs, handlers; "GraphQL schema types"), "API Contract" ("REST operations", "Schemas and validation rules", "Pagination", "Response headers", "GraphQL schema"), AD-18, AD-22, "Implementation Patterns and Consistency Rules" ("Recipes": add a REST endpoint, add a GraphQL field), "Test Architecture" (required-test rows PD-18, FR22/PD-1 and FR24(e)).
Pitfalls: PF-12, PF-14, PF-17, PF-22.
Verify: `make format-check`; `make lint-code`; `make lint-rustdoc`; `make complexity`; `make unit-tests`; `make architecture`; `make check-crate-isolation`; `cargo llvm-cov nextest --locked -p some-module-application --profile ci --summary-only`; `env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --cargo-arg=--locked -p some-module-application`.

**Acceptance Criteria:**

**Given** the context router with `FakeCommandBus` and `FakeQueryBus`
**When** `POST /example_api_resources` is dispatched with a valid body
**Then** the status is 201, the body has `id`, `name` and `description`, and the `location` header is `/api/example_api_resources/<id>`
**And** the same request answers 201 when the command succeeded although the event bus failed (handler level, with fakes)

**Given** bodies with an empty `name` and `description`, with an unknown field, with a missing field and with a value containing U+0000
**When** each is posted
**Then** the responses are 422: the first with type `/problems/validation-failed` and two violations sorted by `propertyPath` (`description`, then `name`), the second and third with `about:blank`, the fourth with code `does_not_contain`; no command was dispatched in any of these cases

**Given** `GET /example_api_resources/{id}` and `GET /example_api_resources`
**When** they are dispatched with a stored id, an unknown id, `not-a-uuid`, and the query strings `page=0`, `page=-1`, `itemsPerPage=0`, `itemsPerPage=101`, `page=abc`, `page=1.5`, `page=4294967296` and a page beyond the last
**Then** the answers are 200, the 404 problem `ExampleApiResource <id> was not found.`, the 400 problem, 422 with a `range` violation for the first four query strings, 400 for the next three, and 200 with `[]`

**Given** the GraphQL roots executed in a unit-test schema with the same fakes in the schema data
**When** `createExampleApiResource`, `exampleApiResource(id)` for a stored and an unknown id, and `exampleApiResources` are executed
**Then** the mutation returns the created object, the unknown id returns `null` without an error, and the list returns the page newest first

**Given** GraphQL arguments `page: -1`, `page: 0`, `itemsPerPage: 101`, an `id` that is not a UUID, an input with an empty name, and a query bus that returns a storage failure
**When** each operation is executed
**Then** the first five return an `errors` entry with `extensions.code` `VALIDATION_FAILED` and `extensions.violations` (for the id: `propertyPath` `id`, code `uuid`, message `This value is not a valid UUID.`), and the last returns `INTERNAL_SERVER_ERROR` with the message `Internal server error` and no internal detail

**Given** the DTOs
**When** a property test generates strings and the unit tests read the derived schema
**Then** the DTO validation and the domain value objects accept exactly the same strings, and the schema of `CreateExampleApiResourceRequest` carries `minLength`, `maxLength`, `pattern` and `additionalProperties: false` with the numbers of the validator attributes

**Given** the `#[utoipa::path]` annotations
**When** the three operations are inspected in a unit test
**Then** operation ids, tag, summaries, descriptions, parameters and responses equal the "REST operations" tables, each with a `default` problem response, and handlers use only `ApiPath`, `ValidatedQuery` and `ValidatedJson` as extractors

**Given** the production files this story owns (`http.rs`, the files under `http/` and `graphql.rs` of the Application crate)
**When** the coverage command and the mutation command of the Verify line are run
**Then** each of those files shows 100% lines and functions in the summary and cargo-mutants exits 0 with no missed and no timed-out mutant; a line or mutant that only code of a later story can reach is listed in the completion notes with that story's id

### Story 6.4: PostgreSQL repository with committed query metadata

As a backend engineer,
I want the repository port implemented with compile-time checked SQL and the query metadata committed,
So that persistence mapping lives in one adapter and the workspace builds without a database.

Track: A.
Depends on: 4.5, 6.1.
Owns: `crates/company-subdomain/some-module/infrastructure/src/persistence.rs`, `crates/company-subdomain/some-module/infrastructure/tests/integration/`, `.sqlx/`.
Touches: `crates/company-subdomain/some-module/infrastructure/src/lib.rs` (module declaration).
Implements: FR31, FR121, NFR4, NFR29, NFR32, G30.
Reference: architecture.md AD-08, "Domain and Application Design" ("Example bounded context": `PgExampleApiResourceRepository`), "Data Architecture" ("Tables": queries and error translation; "Offline query metadata"; "Test database strategy"), "Quality Gates" ("Gate commands": QG-11); architecture-delivery.md "Command Surface" (targets `sqlx-prepare`, `validate-sqlx-metadata`).
Pitfalls: PF-02, PF-28, PF-30, PF-53.
Verify: `make setup-test-db`; `make sqlx-prepare`; `make validate-sqlx-metadata`; `make integration-tests`; `env -u DATABASE_URL cargo check --locked --workspace`; `make format-check`; `make lint-code`; `make complexity`; `make architecture`; `make check-crate-isolation`; `scripts/with-env.sh .env .env.test -- cargo llvm-cov nextest --locked -p some-module-infrastructure --profile ci --summary-only`; `scripts/with-env.sh .env .env.test -- env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --cargo-arg=--locked -p some-module-infrastructure`.

**Acceptance Criteria:**

**Given** the compose database and a prepared test database
**When** the integration test `repository.rs` runs, each test in its own database through `#[sqlx::test(migrator = "shared_infrastructure::persistence::MIGRATOR")]`
**Then** `save` then `find_by_id` returns an equal aggregate; `find_by_id` of an unknown id returns `None`; `find_page` returns items ordered by id descending with the limit and offset of the page; saving the same id twice returns `RepositoryError::AlreadyExists`

**Given** a repository over the pool for `postgres://app:unused@127.0.0.1:1/app`
**When** `save` is called
**Then** it returns `RepositoryError::Unavailable` within the acquire timeout plus 1 s, and the error text contains no SQL and no driver message

**Given** the three statements of "Tables" written with `query!` or `query_as!`
**When** `make sqlx-prepare` and then `make validate-sqlx-metadata` are run
**Then** `.sqlx/` holds one `query-<hash>.json` per statement, the files are part of the story's commit, and the gate exits 0
**And** after one file of `.sqlx/` is deleted, `make validate-sqlx-metadata` exits non-zero, and `git checkout -- .sqlx` restores the tree

**Given** no reachable database
**When** `cargo check --locked --workspace` is run with `DATABASE_URL` unset and, in a second run, set to `postgres://app:unused@127.0.0.1:1/app` (a dead port; no container is stopped)
**Then** both runs exit 0 from the committed metadata

**Given** the adapter
**When** the sources of the workspace are searched
**Then** SQL text and sqlx types for this table appear only in `some-module-infrastructure`; the Domain types carry no sqlx derive; a stored row that fails domain validation is translated to `Unavailable`

**Given** the production files this story owns (`crates/company-subdomain/some-module/infrastructure/src/persistence.rs`)
**When** the coverage command and the mutation command of the Verify line are run
**Then** each of those files shows 100% lines and functions in the summary and cargo-mutants exits 0 with no missed and no timed-out mutant; a line or mutant that only code of a later story can reach is listed in the completion notes with that story's id

## Epic 7: Service Runtime and Composition Root

An operator and an API client get the running service: one binary with the commands `serve`, `migrate`, `check-config`, `healthcheck` and the two contract exports, typed fail-fast configuration, REST under `/api` with a code-first OpenAPI 3.1.0 document and docs page, GraphQL, `/ping`, JSON logs with request ids, security headers, graceful shutdown, committed contract snapshots, and the PHP health feature passing end to end against the real adapters.

### Story 7.1: Command line, typed configuration and `check-config`

As an operator,
I want the service binary to validate its complete configuration from environment variables and name every invalid or missing variable,
So that a wrong setting is reported before anything starts and never leaks a secret value.

Track: A.
Depends on: 4.5.
Owns: `crates/service/src/cli.rs`, `crates/service/src/cli/`, `crates/service/src/config.rs`, `crates/service/src/config/`, `crates/service/src/command.rs`, `crates/service/src/command/check_config.rs`, `crates/service/src/http.rs` (created with its module documentation and no item, so that Stories 7.2 and 7.3 edit different files), `crates/service/tests/integration/main.rs`, `crates/service/tests/integration/support.rs`, `crates/service/tests/integration/check_config.rs`.
Touches: `crates/service/src/lib.rs` (replaces the stub: it declares the modules, among them `http`, and `run` loads `.env`, parses the CLI, dispatches and maps the outcome to an exit status; the arms of `serve`, `migrate`, `openapi-export`, `graphql-export` and `healthcheck` write `error: <command> is not available yet` to stderr and return status 1 until Stories 7.2, 7.4 and 7.6 replace them).
Implements: FR2, FR3, FR4, FR13, NFR4, NFR36, G6.
Reference: architecture.md AD-07, "Domain and Application Design" ("Composition root: `service`": `cli.rs`, `config.rs`, where `Config` holds the `PostgresSettings` and `SqsSettings` of Story 4.5), "Configuration" (all three subsections: "Environment variables" with the cross-field rules, "Committed environment files", "Binary `service`: subcommands and exit statuses"), "Workspace and Project Structure" (tree of `crates/service/`: `cli.rs`, `config.rs`, `command/check_config.rs`, `tests/integration/support.rs`, `check_config.rs`), "Test Architecture" ("Levels": how a spawned binary is run; required-test row FR4), "Implementation Patterns and Consistency Rules" (rules 7, 18, 20, 23).
Pitfalls: PF-10, PF-18, PF-36.
Verify: `make check-requirements`; `make unit-tests`; `cargo nextest run --locked -p service --test integration`; `make format-check`; `make lint-code`; `make lint-rustdoc`; `make complexity`; `make architecture`; `cargo llvm-cov nextest --locked -p service --profile ci --summary-only`; `env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --cargo-arg=--locked -p service -f crates/service/src/cli.rs -f crates/service/src/config.rs -f crates/service/src/command/check_config.rs`.

**Acceptance Criteria:**

**Given** the committed `.env` in the working directory and no other configuration
**When** `make check-requirements` is run
**Then** it exits 0 and prints `configuration OK`, without opening a socket or contacting a dependency

**Given** a cleared environment with `DATABASE_URL=mysql://user:hunter2@h/db`, `APP_PORT=99999`, `APP_ENV=staging` and no `AWS_REGION`, in an empty temporary working directory
**When** the built binary is run as `service check-config`
**Then** it exits 2, stdout is empty, and stderr has one line `<VARIABLE>: <reason>` for each of `DATABASE_URL`, `APP_PORT`, `APP_ENV` and `AWS_REGION` (all four in one run) and does not contain `hunter2`

**Given** `AWS_ACCESS_KEY_ID` set without `AWS_SECRET_ACCESS_KEY`
**When** `service check-config` is run with an otherwise valid environment
**Then** it exits 2 and the line names the missing variable; with both set or both unset it exits 0

**Given** `DATABASE_URL=postgres://u:topsecret@h/db` in the environment
**When** `service serve --help` and `service --help` are run
**Then** both exit 0, `serve --help` lists a flag for every service variable of the environment table except the two credential arguments `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY`, which are hidden and read from the environment only, and neither output contains `topsecret` or any other variable value

**Given** a temporary working directory with a `.env` file that sets `APP_PORT=9000` and a process environment that sets `APP_PORT=9001`
**When** the configuration is loaded
**Then** the process environment wins, and `.env` is read in a synchronous `main` before any runtime thread exists

**Given** the unit tests of `config` and `cli`
**When** `make unit-tests` is run
**Then** every variable has a test for its default, a valid value and each invalid form; every cross-field rule of the environment table has a failing and a passing case; a configured timeout above its PRD ceiling is accepted; a test asserts that every field of `ServeArgs` is consumed by `Config::from_args`; an unknown command or flag is a clap usage error with status 2

**Given** `tests/integration/support.rs`
**When** it spawns the binary
**Then** the child runs with a cleared environment plus the variables the test sets, in an empty temporary directory outside the repository, and `LLVM_PROFILE_FILE` is forwarded so that coverage of the child process is recorded

**Given** the production files this story owns (`cli.rs`, `config.rs` and `command/check_config.rs`; `lib.rs` is final and measured with Story 7.6)
**When** the coverage command and the mutation command of the Verify line are run
**Then** each of those files shows 100% lines and functions in the summary and cargo-mutants exits 0 with no missed and no timed-out mutant; a line or mutant that only code of a later story can reach is listed in the completion notes with that story's id

### Story 7.2: `migrate` command with bounded database wait

As an operator,
I want to apply pending migrations and revert the most recent one with the service binary as an explicit step,
So that the service never migrates implicitly and a failed release can be rolled back with one documented command.

Track: A.
Depends on: 7.1.
Owns: `crates/service/src/command/migrate.rs`, `crates/service/tests/integration/migrate.rs`.
Touches: `crates/service/src/command.rs` (dispatch arm and module declaration), `crates/service/src/config.rs` (`MigrateSettings` with `MigrateSettings::from_args`: the database variables and `DATABASE_WAIT_TIMEOUT`, validated with the private collector), `crates/service/tests/integration/main.rs` (module declaration), `crates/service/tests/integration/support.rs` (`database_url`: the URL of a fresh test database), `crates/service/tests/integration/check_config.rs` (the cases `migrate` and `migrate revert` leave the test of the commands that are not available yet).
Implements: FR2, FR32, FR33, NFR32, NFR33.
Reference: architecture.md AD-20, "Configuration" ("Binary `service`: subcommands and exit statuses": rows `service migrate` and `service migrate revert`; `DATABASE_WAIT_TIMEOUT`), "Observability and Operations" ("Operations": start-up), "Workspace and Project Structure" (tree: `command/migrate.rs`, `tests/integration/migrate.rs`); architecture-delivery.md "Command Surface" (targets `migrate`, `migrate-rollback`).
Pitfalls: PF-29, PF-36.
Verify: `make setup-test-db`; `scripts/with-env.sh .env .env.test -- cargo nextest run --locked -p service --test integration`; `make migrate`; `make migrate-rollback`; `make migrate`; `make unit-tests`; `make format-check`; `make lint-code`; `make complexity`; `scripts/with-env.sh .env .env.test -- cargo llvm-cov nextest --locked -p service --profile ci --summary-only`; `scripts/with-env.sh .env .env.test -- env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --cargo-arg=--locked -p service -f crates/service/src/command/migrate.rs`.

**Acceptance Criteria:**

**Given** a fresh database created by the integration test (URL built by `tests/integration/support.rs`)
**When** `service migrate` is run twice
**Then** the first run exits 0 and prints `applied 1 migration(s)` on stdout, and the second exits 0, prints `applied 0 migration(s)` and changes nothing

**Given** that migrated database
**When** `service migrate revert` is run twice and then `service migrate` once
**Then** the first revert prints `reverted 20261110000000` and the table `example_api_resources` no longer exists, the second prints `nothing to revert`, and the final `migrate` recreates the table; all exit 0

**Given** `DATABASE_URL=postgres://app:unused@127.0.0.1:1/app` and `DATABASE_WAIT_TIMEOUT=2s`
**When** `service migrate` is run
**Then** stderr shows one line `waiting for database (attempt <n>): <error>` per failed attempt, one attempt per second, and the process exits 1 with the connection error after the wait; a missing `DATABASE_URL` exits 2 naming the variable

**Given** the development stack's database
**When** `make migrate`, `make migrate-rollback` and `make migrate` are run in that order
**Then** each exits 0 and the development database ends with the migration applied

**Given** the production files this story owns (`command/migrate.rs`)
**When** the coverage command and the mutation command of the Verify line are run
**Then** each of those files shows 100% lines and functions in the summary and cargo-mutants exits 0 with no missed and no timed-out mutant; a line or mutant that only code of a later story can reach is listed in the completion notes with that story's id

### Story 7.3: Liveness route, request ids, security headers and access log

As an operator,
I want `GET /ping`, a sanitised request id on every request, four security headers on every response and one access-log entry per request,
So that liveness needs no dependency and every request can be traced in the logs.

Track: A.
Depends on: 4.2, 7.1.
Owns: `crates/service/src/http/ping.rs`, `crates/service/src/http/ping/`, `crates/service/src/http/request_id.rs`, `crates/service/src/http/request_id/`, `crates/service/src/http/security_headers.rs`, `crates/service/src/http/security_headers/`, `crates/service/src/http/trace.rs`, `crates/service/src/http/trace/`.
Touches: `crates/service/src/http.rs` (declarations of the four modules).
Implements: FR10, FR11, FR18, NFR15.
Reference: architecture.md AD-12, AD-13, AD-14, "Domain and Application Design" ("Composition root: `service`": the middleware table, rows 1 to 5), "Observability and Operations" ("Log line schema": `request completed`, the `span` object), "API Contract" ("Response headers").
Pitfalls: PF-19, PF-23.
Verify: `make unit-tests`; `make format-check`; `make lint-code`; `make lint-rustdoc`; `make complexity`; `make architecture`; `make setup-test-db`; `scripts/with-env.sh .env .env.test -- cargo llvm-cov nextest --locked -p service --profile ci --summary-only`; `scripts/with-env.sh .env .env.test -- env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --cargo-arg=--locked -p service -f crates/service/src/http/ping.rs -f crates/service/src/http/request_id.rs -f crates/service/src/http/security_headers.rs -f crates/service/src/http/trace.rs`.

**Acceptance Criteria:**

**Given** a test router that mounts `ping`
**When** `GET /ping` is dispatched
**Then** the status is 200, the content type is `text/plain; charset=utf-8` and the body is `pong`; the handler takes no state

**Given** `sanitize_request_id` followed by the request-id layers in a test router
**When** requests arrive with `x-request-id` values `abc-123_x.y`, a 128-character allowed value, a 129-character value, `bad value!`, an empty value and no header
**Then** the first two are echoed unchanged in the response header and the other four are replaced by a generated UUID; the accepted pattern is `^[A-Za-z0-9._-]{1,128}$`

**Given** `security_headers::layer()` on a test router
**When** a handler sets no header and when a handler sets its own `content-security-policy`
**Then** every response carries `x-content-type-options: nosniff`, `x-frame-options: DENY`, `referrer-policy: no-referrer` and `content-security-policy: default-src 'none'; frame-ancestors 'none'`, and the handler's own policy value is kept (`if_not_present`)

**Given** the trace callbacks on a test router with `LogRecorder`
**When** a request to a matched route and one to an unmatched path complete
**Then** each produces exactly one `request completed` entry at `INFO` with `http.response.status_code` and `duration_ms`, inside a span `http.request` with `http.request.method`, `http.route` (the route template or `unmatched`), `url.path` and `request_id`

**Given** the new modules
**When** `make lint-code`, `make complexity` and `make lint-rustdoc` are run
**Then** each exits 0 and `crates/service/src/http.rs` declares the four modules (Stories 7.4 and 7.5 add `AppState`, `docs` and `router`); this story edits neither `lib.rs` nor a file that Story 7.2 edits

**Given** the production files this story owns (`ping.rs`, `request_id.rs`, `security_headers.rs` and `trace.rs` under `crates/service/src/http/`)
**When** the coverage command and the mutation command of the Verify line are run
**Then** each of those files shows 100% lines and functions in the summary and cargo-mutants exits 0 with no missed and no timed-out mutant; a line or mutant that only code of a later story can reach is listed in the completion notes with that story's id

### Story 7.4: OpenAPI document, docs page, GraphQL schema and committed contract snapshots

As a reviewer,
I want the OpenAPI 3.1.0 document and the GraphQL schema generated from code, served by the service and committed as byte-stable snapshots,
So that every contract change is visible as a snapshot diff in the same pull request.

Track: A.
Depends on: 3.2, 5.1, 6.3, 7.2, 7.3.
Owns: `crates/service/src/openapi.rs`, `crates/service/src/openapi/`, `crates/service/src/http/docs.rs`, `crates/service/src/http/docs/`, `crates/service/src/http/docs.html`, `crates/service/src/graphql.rs`, `crates/service/src/graphql/`, `crates/service/src/command/export.rs`, `crates/service/src/command/export/`, `crates/service/tests/integration/export.rs`, `.github/openapi-spec/spec.yaml`, `.github/graphql-spec/spec`.
Touches: `crates/service/src/http.rs` (`AppState` with `FromRef`, module `docs`), `crates/service/src/lib.rs` (module declarations), `crates/service/src/command.rs` (the two export arms and module declaration), `crates/service/tests/integration/main.rs` (module declaration), `crates/service/tests/integration/check_config.rs` (the cases `openapi-export` and `graphql-export` leave the test of the commands that are not available yet).
Implements: FR2, FR8, FR9, FR11, FR18, FR24, FR71, FR72, FR73, FR74, NFR7, NFR13, NFR24, NFR47, G17.
Reference: architecture.md AD-16, AD-17, AD-18, AD-19, AD-14 (page-specific policies), "Domain and Application Design" ("Composition root: `service`": `http.rs`, `openapi.rs`, `graphql/schema.rs`, `graphql/handler.rs`), "API Contract" (all subsections), "Quality Gates" ("Gate commands": QG-19), "Test Architecture" (required-test rows FR9, FR9/NFR13/PD-27 and FR24(d)); architecture-delivery.md DD-10, "Command Surface" (targets `generate-openapi-spec`, `generate-graphql-spec`, `validate-contract-snapshots`, `validate-openapi-spec`).
Pitfalls: PF-01, PF-13, PF-14, PF-15, PF-16, PF-17.
Verify: `make generate-openapi-spec`; `make generate-graphql-spec`; `make validate-contract-snapshots`; `make validate-openapi-spec`; `make openapi-diff base_ref=HEAD`; `make graphql-diff base_ref=HEAD`; `make unit-tests`; `make setup-test-db`; `scripts/with-env.sh .env .env.test -- cargo nextest run --locked -p service --test integration`; `make format-check`; `make lint-code`; `make complexity`; `make architecture`; `scripts/with-env.sh .env .env.test -- cargo llvm-cov nextest --locked -p service --profile ci --summary-only`; `scripts/with-env.sh .env .env.test -- env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --cargo-arg=--locked -p service -f crates/service/src/openapi.rs -f crates/service/src/http/docs.rs -f crates/service/src/graphql/schema.rs -f crates/service/src/graphql/handler.rs -f crates/service/src/command/export.rs`.

**Acceptance Criteria:**

**Given** a cleared environment, no `.env` file, no database and no network
**When** `service openapi-export` and `service graphql-export` are each run 20 times (integration test `export.rs`), and once with `--output <path>`
**Then** all 20 outputs of each command are byte-identical, the `--output` file equals stdout, a path that cannot be written exits 1 with `error: <message>` on stderr, and an output error on stdout (child given `/dev/full`) also exits 1

**Given** the two snapshots, which are part of this story's commit
**When** `make validate-contract-snapshots` is run on that commit, and again after one character of each snapshot is changed in turn
**Then** the first run exits 0 and each changed run exits non-zero; `git checkout -- .github/openapi-spec .github/graphql-spec` restores the files (the gate also fails for a snapshot that is not tracked yet, so it can pass only on the commit)

**Given** `.github/openapi-spec/spec.yaml`
**When** `make validate-openapi-spec` is run and the document is read
**Then** Spectral reports 0 findings at severity hint or above; the document states `openapi: 3.1.0`, `info.version` `1.0.0` from `API_VERSION`, contact `VilnaCRM Support` and `support@vilnacrm.com`, licence `CC0-1.0`, the two tags with descriptions, the four operations with the ids, summaries, descriptions and responses of "REST operations", and the schemas `ExampleApiResource`, `CreateExampleApiResourceRequest`, `Problem`, `Violation` and `DependencyFailure`; it contains no `/ping` and no `/api/docs` path

**Given** `.github/graphql-spec/spec`
**When** it is read and the unit test of `graphql/schema` runs
**Then** it contains exactly the queries `exampleApiResource` and `exampleApiResources` and the mutation `createExampleApiResource`, with the types of "GraphQL schema", and no node field, connection type, update or delete mutation or subscription

**Given** the docs handlers on a test router
**When** `GET /api/docs`, `GET /api/docs/openapi.json`, each asset path the page references and `GET /api/docs/nope.js` are dispatched
**Then** the page is 200 `text/html` without redirect, carries the page-specific policy `DOCS_CONTENT_SECURITY_POLICY` and references no third-party host; the JSON document and every referenced asset are 200; the unknown asset is the 404 problem

**Given** the GraphQL transport handler `execute` on a test router
**When** it receives a valid operation, the bodies `{}`, `[]`, `{"foo":1}` and `{"query":5}`, malformed JSON, and a body with content type `text/plain`
**Then** the valid operation is 200 `application/json`, the four invalid bodies are the 422 problem, malformed JSON is the 400 problem and the wrong media type is the 415 problem; a document that is not valid for the schema returns 200 with `errors`

**Given** the explorer handler and its page `graphql/explorer.html`
**When** the unit test inspects the response
**Then** every `src` and `href` starts with `https://unpkg.com/`, contains an exact `@<major>.<minor>.<patch>/` version and has an `integrity` attribute, and the `content-security-policy` header is `EXPLORER_CONTENT_SECURITY_POLICY`, which names exactly that origin plus `frame-ancestors 'none'`; a directive the page turns out to need or not to need is corrected in the constant and recorded as a deviation

**Given** the production files this story owns (`openapi.rs`, `http/docs.rs`, `graphql/schema.rs`, `graphql/handler.rs` and `command/export.rs`)
**When** the coverage command and the mutation command of the Verify line are run
**Then** each of those files shows 100% lines and functions in the summary and cargo-mutants exits 0 with no missed and no timed-out mutant; a line or mutant that only code of a later story can reach is listed in the completion notes with that story's id

### Story 7.5: Composition root and assembled router

As a backend engineer,
I want one place that builds the complete application from a value in which any port can be replaced, and one router with the fixed middleware order,
So that registration is explicit, tests can inject faults, and every response on every route carries the same headers and error format.

Track: A.
Depends on: 5.2, 6.4, 7.4.
Owns: `crates/service/src/bootstrap.rs`, `crates/service/src/bootstrap/`, `crates/service/src/http/router.rs`, `crates/service/src/http/router/`, `crates/service/tests/integration/bootstrap.rs`.
Touches: `crates/service/src/http.rs` and `crates/service/src/lib.rs` (module declarations), `crates/service/tests/integration/main.rs` (module declaration).
Implements: FR5, FR6, FR9, FR11, FR13, FR17, FR28, NFR6, NFR15.
Reference: architecture.md AD-04, AD-05 (registration), AD-15, "Domain and Application Design" ("Composition root: `service`": `Adapters`, `BootstrapError`, `event_bus`, `router`, `assemble` and the eight-layer middleware table), "API Contract" ("REST operations": routes outside the OpenAPI document; "Response headers"), "Implementation Patterns and Consistency Rules" ("Code shape imposed by the tools" rule 23), "Test Architecture" ("Levels": unit, integration; required-test rows PD-18, FR5 and NFR6).
Pitfalls: PF-12, PF-19, PF-36.
Verify: `make unit-tests`; `make setup-test-db`; `make integration-tests`; `make format-check`; `make lint-code`; `make lint-rustdoc`; `make complexity`; `make architecture`; `make check-crate-isolation`; `scripts/with-env.sh .env .env.test -- cargo llvm-cov nextest --locked -p service --profile ci --summary-only`; `scripts/with-env.sh .env .env.test -- env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --cargo-arg=--locked -p service -f crates/service/src/bootstrap.rs -f crates/service/src/http/router.rs`.

**Acceptance Criteria:**

**Given** `bootstrap::router` built from an `Adapters` value made of test doubles
**When** the wiring unit test dispatches `POST /api/example_api_resources`, `GET /api/example_api_resources/{id}`, `GET /api/example_api_resources`, the three GraphQL operations and `GET /api/health`
**Then** each succeeds (201, 200, 200, 200 without `errors`, 204), which proves every command handler, query handler, subscriber and probe is registered
**And** the same test replaces each port once (repository, one probe, the cache behind the cache probe, the event bus, the clock, the UUID generator) and observes the replacement's behaviour; with a failing event bus the create request still answers 201

**Given** a second handler registered for a command that already has one
**When** the registration code runs in a unit test
**Then** it returns `BootstrapError::DuplicateHandler`, which stops start-up with an error that names the message

**Given** the assembled router with test doubles
**When** a request is sent to every route of the PRD endpoint table, to `/api/unknown`, with `DELETE` to `/api/example_api_resources`, and to `/api/health` with `StubProbe::panicking`
**Then** every response carries `x-request-id` and the four security headers with the values of AD-14 (page-specific policy on the two HTML pages); the unknown route is the 404 problem; the wrong method is the 405 problem with an `allow` header listing the route's methods; the panic is the 500 problem whose body is exactly `{"type":"about:blank","title":"Internal Server Error","status":500}`

**Given** `HTTP_MAX_BODY_BYTES=1024` and `HTTP_REQUEST_TIMEOUT` shorter than a hanging probe under tokio's paused clock
**When** a 2 KiB body is posted and `GET /api/health` is requested
**Then** the answers are the 413 problem and the 503 problem with detail `The request exceeded its time budget.`, both with request id and security headers

**Given** configurations with `APP_ENV=dev`, `test` and `prod`
**When** `GET /api/graphql` is dispatched
**Then** `dev` answers 200 `text/html` and the other two answer the 405 problem with `allow: POST`; no other behaviour differs between the modes

**Given** the compose services and a prepared test database
**When** the integration test `bootstrap.rs` builds two application instances with `Adapters::connect` and `bootstrap::router`
**Then** `GET /api/health` answers 204 through the real probes, and a resource created through the first instance is read through the second
**And** `Adapters::connect` performs no network access itself, and `Adapters::with_probe` replaces the probe of the same dependency and keeps the order and the other two probes

**Given** the production files this story owns (`bootstrap.rs` and `http/router.rs`; an error path that the coverage tool cannot see is reshaped as rule 23 prescribes)
**When** the coverage command and the mutation command of the Verify line are run
**Then** each of those files shows 100% lines and functions in the summary and cargo-mutants exits 0 with no missed and no timed-out mutant; a line or mutant that only code of a later story can reach is listed in the completion notes with that story's id

### Story 7.6: `serve` and `healthcheck` commands with JSON logs and graceful shutdown

As an operator,
I want the service to start without reachable dependencies, log one JSON object per line, drain in-flight requests on SIGTERM and probe its own liveness,
So that deployments, restarts and outages behave predictably and are observable.

Track: A.
Depends on: 7.5.
Owns: `crates/service/src/command/serve.rs`, `crates/service/src/command/healthcheck.rs`, `crates/service/src/telemetry.rs`, `crates/service/tests/integration/serve.rs`, `crates/service/tests/integration/panic.rs`, `crates/service/tests/integration/healthcheck.rs`.
Touches: `crates/service/src/lib.rs` (module declaration `telemetry`), `crates/service/src/command.rs` (the last two dispatch arms and module declarations), `crates/service/tests/integration/main.rs` (module declarations), `crates/service/tests/integration/check_config.rs` (the test of the commands that are not available yet is removed, since no such command remains), `crates/service/tests/integration/support.rs` (log-line reader, SIGTERM, socket and timing helpers); `.github/openapi-spec/spec.yaml` and files under `crates/` (only where the early Schemathesis run of this story exposes a difference between the document and the behaviour).
Implements: FR1, FR2, FR10, FR12, FR87, FR126, NFR2, NFR3, NFR5, NFR31.
Reference: architecture.md AD-12, AD-13, AD-15 (a forced end of the grace period exits non-zero), "Configuration" ("Binary `service`: subcommands and exit statuses": rows `serve`, `healthcheck`), "Observability and Operations" ("Log line schema", "Operations": shutdown sequence, panics, start-up), "Workspace and Project Structure" (tree: the case lists of `tests/integration/serve.rs`, `panic.rs` and `healthcheck.rs`), "Test Architecture" (required-test rows PD-21/NFR5, NFR31 and FR126/NFR2/NFR3), "Implementation Patterns and Consistency Rules" (rule 23), "Architecture Validation Results" ("Risks": Schemathesis ran against the first form of the page parameters); architecture-delivery.md "Command Surface" (target `schemathesis-validate` with a caller-set `SCHEMATHESIS_URL`).
Pitfalls: PF-10, PF-12, PF-19, PF-20, PF-22, PF-23, PF-36, PF-50.
Verify: `make setup-test-db`; `make integration-tests`; `make unit-tests`; `make format-check`; `make lint-code`; `make lint-rustdoc`; `make complexity`; `make architecture`; `grep -rn "is not available yet" crates/service/src` (expected: prints nothing); `scripts/with-env.sh .env .env.test -- cargo llvm-cov nextest --locked -p service --profile ci --summary-only`; `scripts/with-env.sh .env .env.test -- env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --cargo-arg=--locked -p service -f crates/service/src/lib.rs -f crates/service/src/command/serve.rs -f crates/service/src/command/healthcheck.rs -f crates/service/src/telemetry.rs`; `mkdir -p target/reports && cargo build --locked --quiet -p service; APP_PORT=18082 scripts/with-env.sh .env .env.test -- target/debug/service serve > target/reports/early-serve.log & pid=$!; for i in $(seq 1 120); do curl -fsS http://127.0.0.1:18082/ping > /dev/null && break; sleep 1; done; make schemathesis-validate SCHEMATHESIS_URL=http://127.0.0.1:18082; status=$?; kill -TERM "$pid"; wait "$pid"; serve_status=$?; test "$status" -eq 0 && test "$serve_status" -eq 0` (the built binary is started through `scripts/with-env.sh`, which replaces itself with it, so `$pid` is the service and its exit status after SIGTERM is checked).

**Acceptance Criteria:**

**Given** the built binary started as `service serve` with `APP_PORT=0` and the test configuration
**When** the test reads stdout until the `listening` entry and sends a sequence of requests (`/ping`, `/api/health`, a create, an unknown route) with one fixed `x-request-id`
**Then** the `listening` entry states `addr`, `env` and `version`; `/ping` answers 200 `pong` and `/api/health` 204; every stdout line parses as one JSON object with `timestamp`, `level` and `message`; stderr is empty; every entry written while handling a request carries that request id, with exactly one `request completed` entry per request

**Given** the service with the database URL and the SQS endpoint both pointing at `127.0.0.1:1`
**When** it is started and `/ping` and `/api/health` are requested
**Then** `/ping` answers 200 within 5 s of process start, and `/api/health` answers 500 within `HEALTH_PROBE_TIMEOUT` plus 1 s with a body that names the database and the message broker and not the cache

**Given** a running service
**When** SIGTERM is sent
**Then** an idle service logs `shutdown signal received` and `stopped` and exits 0 within 2 s; with a request in flight the request completes with its normal response and the exit status is 0; a connection attempted after SIGTERM is refused
**And** with `SHUTDOWN_GRACE_PERIOD=1s` and a connection still open when the period ends it logs `grace period elapsed, forcing shutdown`, closes the connection and exits with status 1 within the grace period plus 2 s

**Given** a port that is already in use, and separately an invalid variable
**When** `service serve` is started
**Then** the first exits 1 after one JSON entry `serve failed` at `ERROR` with the field `error`, and the second exits 2 with one JSON entry `invalid configuration` per problem on stdout (fields `variable` and `reason`) and an empty stderr

**Given** production telemetry on a captured writer and an application built with `StubProbe::panicking` (integration test `panic.rs`)
**When** `GET /api/health` is dispatched
**Then** the response is the 500 problem and every captured line, the `panic` entry included, is one JSON object that carries the request id

**Given** `service healthcheck`
**When** it is run with `APP_PORT` of a running service and with a port on which nothing listens
**Then** it exits 0 and 1 respectively, using a plain TCP request with a 2 s timeout and no dependency access; the request goes to `127.0.0.1` when `APP_HOST` is unset and to the address of `APP_HOST` otherwise (the loopback address of the same family for `0.0.0.0` and `::`), so a service bound to `127.0.0.2` is probed there
**And** `grep -rl "MIGRATOR" crates/service/src` prints only `crates/service/src/command/migrate.rs`: start-up never migrates

**Given** the development services, a prepared test database and the service started on the host by the last command of the Verify line (`APP_PORT=18082`, the variables of `.env` and `.env.test`)
**When** `make schemathesis-validate SCHEMATHESIS_URL=http://127.0.0.1:18082` is run
**Then** no isolated project is started and Schemathesis reports 0 failed checks with `--checks all --phases=examples,coverage` on the signed page parameters (400 for a value that is not a 32-bit integer, 422 for an integer out of bounds), and the service exits 0 after SIGTERM; a difference between the document and the behaviour is fixed in the crate that owns the handler or the schema, the snapshot is regenerated, and the fix is listed in the completion notes (this is the early run the core risk row asks for; Story 9.1 repeats it against the production image)

**Given** the production files this story owns (`command/serve.rs`, `command/healthcheck.rs`, `telemetry.rs` and `lib.rs`, which is final with this story)
**When** the coverage command and the mutation command of the Verify line are run
**Then** each of those files shows 100% lines and functions in the summary and cargo-mutants exits 0 with no missed and no timed-out mutant; a line or mutant that only code of a later story can reach is listed in the completion notes with that story's id

### Story 7.7: Gherkin end-to-end suite with the reused PHP health feature

As a reviewer,
I want the PHP `health_check.feature` to run unchanged against the service, together with scenarios for the documentation page, the example resource, GraphQL and the HTTP surface,
So that wire parity with the PHP template is proven by the same feature file and an undefined step can never pass.

Track: A.
Depends on: 7.5.
Owns: `tests/e2e/world.rs`, `tests/e2e/steps.rs`, `tests/e2e/steps/`, `features/health_check.feature`, `features/api_docs.feature`, `features/example_api_resource.feature`, `features/graphql.feature`, `features/http_errors.feature`.
Touches: `tests/e2e/main.rs` (replaces the stub with the cucumber runner).
Implements: FR55, FR56, FR57, NFR6, NFR30, G13.
Reference: architecture.md AD-23, "Test Architecture" ("Gherkin features": the step vocabulary and the five feature listings; "Levels": end-to-end; "Test database strategy" under "Data Architecture"), "Workspace and Project Structure" (tree of `tests/e2e/` and `features/`), "Quality Gates" ("Gate commands": QG-15; "`.config/nextest.toml`": the default filter); architecture-delivery.md "Command Surface" (target `e2e-tests`).
Pitfalls: PF-45, PF-46, PF-48.
Verify: `cmp features/health_check.feature ${PHP_TEMPLATE_DIR}/features/health_check.feature`; `make setup-test-db`; `make e2e-tests`; `make unit-tests`; `make format-check`; `make lint-code`; `make architecture`.

**Acceptance Criteria:**

**Given** `features/health_check.feature`
**When** it is compared with the PHP template's file
**Then** `cmp` exits 0 (byte-identical, 4 scenarios, no trailing newline added)

**Given** the compose services and a prepared test database
**When** `make e2e-tests` is run
**Then** it exits 0 and every scenario of the five feature files passes (24 scenarios, 97 steps), with 0 skipped and 0 undefined steps; scenarios without an injected fault use the real PostgreSQL, cache and SQS adapters built through `Adapters::connect`

**Given** the three scenarios with `Given the database is not available`, `Given the cache is not working` and `Given the message broker is not available`
**When** they run
**Then** the fault is injected by replacing that dependency's probe with `StubProbe::failing` through `Adapters::with_probe`, the response is 500 and contains the exact message of that dependency, and no container is stopped

**Given** scenarios that write data
**When** the suite runs with its default concurrency, and again with `scripts/with-env.sh .env .env.test -- cargo test --locked -p e2e-tests --test e2e -- --concurrency 1`
**Then** both runs pass: every scenario uses its own database `e2e_<pid>_<n>` (dropped by the `after` hook; databases left by a killed run are removed at start, which is why two end-to-end runs never share one database server), the world builds a new application instance for every request, and the development database `app` is never touched

**Given** a temporary feature file with the step `Given something nobody implemented` planted under `features/`
**When** `make e2e-tests` is run
**Then** it exits non-zero and reports the step as unmatched (`fail_on_skipped_with`, which also fails a skipped step in a scenario tagged `@allow.skipped`); after the file is removed the target exits 0

**Given** the workspace
**When** `cargo nextest run --locked --workspace --lib --bins` is run
**Then** it does not try to list the `e2e` binary (default filter `not binary(e2e)`), the suite writes no JUnit file, and `tests/e2e/main.rs` declares `#[cfg(test)] mod steps;` and `#[cfg(test)] mod world;` with no `#![expect]`

**Given** the feature files other than the health feature
**When** they are read
**Then** they equal the listings of "Gherkin features": the documentation page answers 200 with the page, a created resource is read by id, a list contains exactly the items the scenario created, invalid input is 422, an unknown id is 404, and one scenario executes the create mutation and both queries through `POST /api/graphql`

## Epic 8: AI Guidance and Documentation

An AI assistant gets one entry file, one verification list, eleven skills, a committed settings file with a guard on protected files and the BMAD and bmalph integration; a human gets the README, the contribution and security policies, and in-repository documents for the first hour, architecture, operations, environment variables, Make targets and repository settings, so that journeys J1 to J8 can be followed from the repository alone.

### Story 8.1: Assistant entry files, settings and protected-file guard

As an AI coding assistant,
I want one entry file with the working rules and the one canonical verification list, and a committed settings file that lets me run gates freely but blocks edits to protected configuration,
So that I know what to read, what to run and what I must not weaken.

Track: D.
Depends on: 1.4, 2.4.
Owns: `AGENTS.md`, `CLAUDE.md` (existing file: the BMAD-generated section is kept exactly as Story 1.4 left it, the other sections are added), `.claude/settings.json`, `scripts/claude/guard-protected-files.sh`.
Touches: none.
Implements: FR104, FR105, FR109, FR111, NFR26, NFR43.
Reference: architecture-delivery.md "AI Guidance and Documentation" ("Canonical verification list", "Entry files", "`.claude/settings.json`"), DD-17 (rule 4 shapes the guidance), DD-18; architecture.md "Quality Gates" ("Protected configuration": `cargo xtask config-guard --list`).
Verify: `make lint-markdown`; `make lint-shell`; `make lint-prettier`; `make lint-spelling`; `echo '{"tool_input":{"file_path":"clippy.toml"}}' | scripts/claude/guard-protected-files.sh pre` (expected exit status 2); `echo '{"tool_input":{"file_path":"README.md"}}' | scripts/claude/guard-protected-files.sh pre` (expected exit status 0).

**Acceptance Criteria:**

**Given** `AGENTS.md`
**When** it is read
**Then** it has the six sections of "Entry files" (identity, `Mandatory Workflow For AI Agents`, `Working Rules`, `Verification`, `Close-Out`, pointer to `.claude/skills/README.md`), states the rules of FR109, and does not contain the string `rust-service-template`

**Given** the block between `<!-- verification:start -->` and `<!-- verification:end -->` in `AGENTS.md`
**When** its Make targets are compared with the output of `make ci-list`
**Then** they are the same targets in the same order, and no other numbered list or fenced block in `AGENTS.md` or `CLAUDE.md` cites three or more gate targets

**Given** `CLAUDE.md`
**When** it is compared with the version at `HEAD` before this story
**Then** the section "BMAD-METHOD Integration" is byte-identical; the added sections are `Start Here`, `Project Snapshot` and `Command Reference`; the file links to the verification section of `AGENTS.md` instead of repeating it, states no rule that `AGENTS.md` does not state and does not name the repository

**Given** `.claude/settings.json`
**When** it is compared with the listing
**Then** the `allow`, `ask` and `deny` lists and the `PreToolUse` and `PostToolUse` hooks equal it: one `Bash(make <target>:*)` allow entry per gate, test, export and start target of the list under the listing and no entry for `make` as a whole; `aws-load-tests`, `aws-load-tests-cleanup`, `update`, `purge`, `clean`, `down`, `migrate-rollback`, `build`, `stop`, `static-analysis-fix` and `unused-dependencies-fix` ask

**Given** the hook script called with `pre` and a hook JSON on stdin
**When** the edited path is `clippy.toml`, a file under `xtask/src/arch/`, `README.md`, and `clippy.toml` again with `ALLOW_PROTECTED_CONFIG_EDIT=1`
**Then** the exit statuses are 2, 2, 0 and 0; the refusals print `Protected quality configuration: <path>.` with the advice to restart with `ALLOW_PROTECTED_CONFIG_EDIT=1` and add the label `approved-config-change`; the override prints a notice
**And** in a temporary clone of the repository (so that the manifest of the story's worktree is never edited), after an edit of `[workspace.lints]` in the root `Cargo.toml` between a `pre` and a `post` call, the `post` call exits 2 and restores the file from the commit recorded by `pre`, while an edit of a dependency line exits 0; the items come from `cargo xtask config-guard --list`, cached in `target/claude-guard/items.txt` and refreshed when `protected-config.toml` is newer; without `cargo`, `git` or `jq` on `PATH` the script prints a warning and exits 0

**Given** the new Markdown, JSON and shell files
**When** `make lint-markdown`, `make lint-prettier`, `make lint-shell` and `make lint-spelling` are run
**Then** each exits 0

### Story 8.2: Skill index, guides and ten capability skills

As an AI coding assistant,
I want a skill index, a usage guide, a decision guide and one skill per task type, each saying when to use it and which Make targets to run,
So that I pick the right procedure for a task and every command and path it names exists.

Track: D.
Depends on: 1.4.
Owns: `.claude/skills/README.md`, `.claude/skills/AI-AGENT-GUIDE.md`, `.claude/skills/SKILL-DECISION-GUIDE.md`, `.claude/skills/api-resource-development/`, `.claude/skills/verification-workflow/`, `.claude/skills/complexity-management/`, `.claude/skills/architecture-boundary-fixer/`, `.claude/skills/documentation-sync/`, `.claude/skills/load-testing/`, `.claude/skills/api-contract-development/`, `.claude/skills/quality-standards/`, `.claude/skills/architecture-model-sync/`, `.claude/skills/testing-workflow/`.
Touches: none.
Implements: FR106, FR107, FR108, NFR44.
Reference: architecture-delivery.md "AI Guidance and Documentation" ("Skills": the skill table, the three guide files and the skill file format), DD-17 (rules 4 and 7); architecture.md "Implementation Patterns and Consistency Rules" ("Recipes"), AD-03 (rule names for `architecture-boundary-fixer`); PRD gate list (for the table of `quality-standards`).
Verify: `make lint-markdown`; `make lint-spelling`; `for d in .claude/skills/*/; do n=$(basename "$d"); grep -q "^name: $n$" "$d/SKILL.md" || echo "bad $n"; done` (expected: no output for the ten skills of this story); `grep -rl "rust-service-template" .claude/skills` (expected: no output).

**Acceptance Criteria:**

**Given** each of the ten skill directories
**When** its `SKILL.md` is read
**Then** the front matter has exactly `name` (equal to the directory name) and `description` (one sentence starting with `Use when`, equal in meaning to the skill table); the body has one H1, the sections `## Use This Skill When`, `## Commands`, `## Rules` and a final `## Verification` with a `bash` block of at most two gate targets and a sentence that routes to companion skills; the file has 30 to 70 lines, no image, no external link and no repository name

**Given** the "Commands it must cite" column of the skill table
**When** each skill is searched for those `make` targets
**Then** every listed target is cited and exists in `make help`; `api-resource-development` names the files of the example slice under `crates/company-subdomain/some-module/`; `architecture-boundary-fixer` has one fix pattern per rule `arch::*`

**Given** `quality-standards`
**When** it is read
**Then** it holds the only table of the skills: every gate G1 to G31 with its threshold, its Make target (or the reason it is CI-only) and the protected file that holds the threshold; no other guidance file states a numeric threshold

**Given** `.claude/skills/README.md`
**When** it is read
**Then** it lists eleven skills, one line each: the ten of this story and `bmad-autonomous-planning` (delivered by Story 8.3), and it is the only skill inventory

**Given** `AI-AGENT-GUIDE.md` and `SKILL-DECISION-GUIDE.md`
**When** they are read
**Then** the first describes classify, pick, follow, verify, summarise and the three flows (code change, API change, architecture change); the second maps each task intent to one primary skill, lists the four combined scenarios and the non-negotiable rules; neither repeats the verification list

### Story 8.3: BMAD and bmalph integration

As an AI coding assistant,
I want tracked command wrappers, Make targets that install and initialise bmalph, and an autonomous-planning skill,
So that a planning bundle can be produced in a fresh clone although the generated BMAD assets are not in git.

Track: D.
Depends on: 1.4.
Owns: `bmalph/config.json` (existing file, kept), `.claude/commands/` (the 54 existing wrapper files, each given the preface paragraph), `scripts/install-bmalph.sh`, `.claude/skills/bmad-autonomous-planning/`.
Touches: none.
Implements: FR112.
Reference: architecture-delivery.md "AI Guidance and Documentation" ("`.claude/commands` and bmalph"; skill table row `bmad-autonomous-planning`), "Command Surface" (targets `bmalph-install`, `bmalph-claude`, `bmalph-codex`, `bmalph-init`, `bmalph-setup`; script row `install-bmalph.sh`). Sources to adapt: `${CORE_SERVICE_DIR}/scripts/local-coder/install-bmalph.sh` and `${CORE_SERVICE_DIR}/.claude/skills/bmad-autonomous-planning/`.
Verify: `make lint-shell`; `make lint-markdown`; `ls .claude/commands/*.md | wc -l` (expected `54`); `grep -L "requires local BMALPH assets" .claude/commands/*.md` (expected: no output); `git check-ignore _bmad _bmad-output .ralph bmalph/state`; with Node.js 20 and npm available: `make bmalph-init BMALPH_DRY_RUN=true` (expected: the output of `git status --porcelain` is the same before and after).

**Acceptance Criteria:**

**Given** the 54 files under `.claude/commands/`
**When** each is read
**Then** its first paragraph is the preface of the architecture ("This wrapper requires local BMALPH assets under `_bmad/` ..."), followed by bmalph's own content unchanged

**Given** `git check-ignore`
**When** it is asked about `_bmad`, `_bmad-output`, `.ralph` and `bmalph/state`
**Then** all four are ignored, while `bmalph/config.json` and `.claude/commands/` are tracked

**Given** a host with Node.js 20 and npm and an isolated `HOME`
**When** `make bmalph-install` is run
**Then** it exits 0 and prints `BMALPH installed:` with command, the version pinned as `BMALPH_VERSION` and package, and `BMALPH dry-run verification passed for platform 'claude-code'.`; `make bmalph-install BMALPH_PLATFORM=nope` exits non-zero

**Given** a clean tree on such a host
**When** `make bmalph-init BMALPH_DRY_RUN=true` and `make bmalph-setup BMALPH_DRY_RUN=true` are run
**Then** both exit 0 and the output of `git status --porcelain` is the same before and after; without `--dry-run` the script refuses to run on a tree with tracked changes and afterwards restores every tracked file that bmalph modified, so the prefaces, `CLAUDE.md` and `.gitignore` stay as committed (the run without `--dry-run` is tried in a temporary clone, never in the shared checkout)

**Given** `.claude/skills/bmad-autonomous-planning/SKILL.md`
**When** it is read
**Then** it follows the skill file format, cites `make bmalph-setup`, sends bundles to `specs/<bundle-id>/`, states the bmalph artifact format rules (`## Tech Stack`, epic and story headings, readiness wording), names no model and no repository, and keeps the rule that no production code is written

### Story 8.4: README, contribution and security policies, issue and pull-request templates

As a backend engineer,
I want a README with the quick start, a contribution guide with the commit convention, a complete security policy and backend-appropriate templates,
So that I can start, contribute and report a vulnerability without asking anyone.

Track: D.
Depends on: 1.4.
Owns: `README.md` (existing one-line file, replaced), `CONTRIBUTING.md`, `SECURITY.md`, `.github/ISSUE_TEMPLATE/`, `.github/pull_request_template.md`, `.coderabbit.yaml`.
Touches: none.
Implements: FR97, FR113, FR114, FR115, FR116, NFR46, NFR47.
Reference: architecture-delivery.md "AI Guidance and Documentation" ("Human documentation": rows `README.md`, `CONTRIBUTING.md`, `SECURITY.md`; the badge and licence paragraphs), "CI/CD Workflows" ("Dependabot, review bot and templates": `.coderabbit.yaml` and the template table), "Git Hooks and Commit Convention", DD-23; PRD assumption A-8.
Verify: `make lint-markdown`; `make lint-yaml`; `make lint-prettier`; `make lint-spelling`; `cmp .coderabbit.yaml ${PHP_TEMPLATE_DIR}/.coderabbit.yaml`; `make bats BATS_ARGS="--filter FR97" BATS_PATH=tests/CLI/bats/make_general.bats`; `grep -n "TODO\|TBD\|example.com" SECURITY.md` (expected: no output).

**Acceptance Criteria:**

**Given** `README.md`
**When** it is read
**Then** it has the sections of the documentation table: banner, badges, purpose and baseline, supported host platforms (Linux x86_64 verified in CI; the three others as stated), quick start with `make doctor` and `make bootstrap` and the five entry-point URLs, AI workflow with the prompt pattern, everyday commands as a link to `docs/make-targets.md`, CI model, creating a service and receiving updates, cloud load testing, documentation index, licence `CC0-1.0`
**And** every badge is dynamic (workflow status, Codecov, latest release, licence) and no numbered list or fenced block cites three or more gate targets

**Given** `CONTRIBUTING.md`
**When** it is read
**Then** it covers issue first, branch naming, the rule `type(#issue): subject` with accepted and rejected examples and the version rules, `make install-hooks`, pull-request content, the labels `approved-config-change` and `approved-breaking-change`, review and squash merge, and links to the verification section of `AGENTS.md`

**Given** `SECURITY.md`
**When** it is read
**Then** it states: supported version is the latest release; reports go through GitHub private vulnerability reporting; acknowledgement within 5 working days; advisories are published as GitHub security advisories; scope (template code and tooling, not downstream services); what the automated gates cover; and it contains no placeholder text

**Given** the templates
**When** they are read
**Then** the bug report has label `bug` and asks for the endpoint (method and path), the request (headers and body, secrets removed), steps, expected and actual behaviour with status code, problem body, `x-request-id` and log lines, and environment, with no browser or device field; the feature request has label `enhancement`; the pull-request template has the checklist of the table; none contains an absolute link to a repository

**Given** `.coderabbit.yaml`
**When** it is compared with the PHP template's file
**Then** `cmp` exits 0 and `reviews.request_changes_workflow` is `true`

### Story 8.5: Getting-started and reference documents with the request collection

As a backend engineer,
I want a first-run guide, a guide to creating a service, a troubleshooting guide, the environment-variable and Make-target references with the PHP-to-Rust mappings, and a request collection,
So that every variable, target and endpoint is documented in the repository.

Track: D.
Depends on: 1.4.
Owns: `docs/README.md`, `docs/getting-started.md`, `docs/new-service.md`, `docs/troubleshooting.md`, `docs/environment.md`, `docs/make-targets.md`, `docs/requests.http`.
Touches: none.
Implements: FR117, FR118, NFR45, NFR46.
Reference: architecture-delivery.md "AI Guidance and Documentation" ("Human documentation": the rows of these seven files), "Command Surface" ("Variables" with the inputs that only a script reads, "Targets" with class and needs, "PHP target to Rust target", "Bats coverage of the targets"), DD-01, DD-02 (CI-only steps), DD-17 (rules (5) and (6)), DD-23; architecture.md "Configuration" ("Environment variables" with the PHP names, "Committed environment files"), "API Contract" ("REST operations", "GraphQL schema").
Verify: `make lint-markdown`; `make lint-spelling`; `for t in $(make help | awk 'NR>3{print $1}'); do grep -qF "$(printf '\140%s\140' "$t")" docs/make-targets.md || echo "missing $t"; done` (expected: no output); `grep -c "^###\|^GET\|^POST" docs/requests.http`.

**Acceptance Criteria:**

**Given** `docs/environment.md`
**When** it is compared with the environment table of the architecture and the "Variables" table of the command surface
**Then** it lists every variable of both, the two test seeds included, with default, consumer and PHP name, includes the PHP-to-Rust name mapping and the placeholder policy of `.env` and `.env.test`, and contains no variable that neither table has

**Given** `docs/make-targets.md`
**When** the loop of the Verify line is run and the file is read
**Then** the loop prints nothing; every target has its class (TOOL or HOST), needs and help text; the file includes the PHP-to-Rust target table, the CI-only steps with their reasons, for each gate target where its passing run and its negative test are, the targets without a Bats test with their reasons, the inputs that only a script reads, and a link to the verification section of `AGENTS.md` instead of a list of gate targets

**Given** `docs/getting-started.md` and `docs/troubleshooting.md`
**When** they are read
**Then** the first covers prerequisites, `make doctor`, `make bootstrap`, the first request, native mode, `RUN_IN_CONTAINER=1`, the development container, stopping and cleaning, and says that `docker compose up` without Make does not start the Structurizr viewer; the second covers each topic of its table row

**Given** `docs/new-service.md`
**When** it is read
**Then** it names the package identity in the workspace manifest (`description`, `repository`) and the checkout directory name as the only two values a new service changes, then `bmalph/config.json` and the README badges, `make new-context`, the wiring steps, where a service adds its own Make targets (`make/service.mk`, `docs/make-targets-service.md`) and which template-owned files it extends, the secrets, variables and labels to create, and the choice of one sync flavour

**Given** `docs/README.md`
**When** it is read
**Then** it links every file under `docs/` that the documentation table lists; no separate mapping document exists, because the PHP-to-Rust mappings are part of `docs/environment.md` and `docs/make-targets.md`

**Given** `docs/requests.http`
**When** it is read
**Then** it defines `@baseUrl = http://localhost:8081` and one request each for `GET /ping`, `GET /api/health`, `GET /api/example_api_resources`, `POST /api/example_api_resources`, `GET /api/example_api_resources/{id}` and the three GraphQL operations (eight requests)

### Story 8.6: Architecture overview and decision records

As a reviewer,
I want an architecture overview and twelve decision records in the repository,
So that I understand the layers, the request flow and why each structural decision was taken without reading the planning bundle.

Track: D.
Depends on: 1.4.
Owns: `docs/architecture.md`, `docs/adr/`.
Touches: none.
Implements: FR117, NFR6, NFR46.
Reference: architecture-delivery.md "AI Guidance and Documentation" ("Human documentation": rows `docs/architecture.md` and `docs/adr/README.md`, `docs/adr/0001` to `0012` with the planning decisions each summarises), "Repository File Manifest" (`docs/`: the twelve file names); architecture.md "Core Architectural Decisions" (AD-01 to AD-27), architecture-delivery.md "Delivery Decisions" (DD-01 to DD-25).
Verify: `make lint-markdown`; `make lint-spelling`; `ls docs/adr/0*.md | wc -l` (expected `12`).

**Acceptance Criteria:**

**Given** `docs/architecture.md`
**When** it is read
**Then** it describes the three layers and what belongs in each, the thirteen crates and three bounded contexts, the request flow from router to repository, the error model, and links to `workspace.dsl` and the decision records

**Given** the same file
**When** it is searched for the operational assumptions
**Then** it states that the service keeps no session or per-client state, that PostgreSQL is the only source of truth and no response depends on cache content, that domain-event delivery is at most once because no outbox ships, and that a proxy or load balancer terminates TLS and limits header-read time and connections in front of the service

**Given** `docs/adr/`
**When** it is listed and each record is read
**Then** it holds `README.md` (an index) and the twelve files named in the manifest; each record has the parts context, decision and consequences and names the planning decisions it summarises (for example `0001-workspace-and-layers.md` names AD-01 and AD-02)

**Given** the new files
**When** `make lint-markdown` and `make lint-spelling` are run and the files are searched for `make ` citations
**Then** both linters exit 0, every cited Make target exists in `make help`, and no file contains a list or block with three or more gate targets

### Story 8.7: Operations, load-testing, extension and repository-settings documents with the setup guides

As a platform maintainer,
I want runbooks for migrations, rollback and health probes, the load-testing guide, the extension seams, the repository settings to apply, and setup guides for release and both template-sync flavours,
So that operating the service and configuring a repository created from the template need no knowledge outside the repository.

Track: D.
Depends on: 1.4.
Owns: `docs/operations.md`, `docs/load-testing.md`, `docs/extension-seams.md`, `docs/repository-settings.md`, `.github/AUTORELEASE.md`, `.github/TEMPLATE_SYNC_APP.md`, `.github/TEMPLATE_SYNC_PAT.md`.
Touches: none.
Implements: FR101, FR117, NFR1, NFR46.
Reference: architecture-delivery.md "AI Guidance and Documentation" ("Human documentation": the rows of these seven files), DD-04, DD-11, DD-14, DD-25, "CI/CD Workflows" ("Template sync" with `.templatesyncignore`, "Secrets, variables and labels", "Dependabot, review bot and templates": the toolchain update procedure), "Load Testing"; architecture.md "Observability and Operations", AD-11 (failure messages), AD-20.
Verify: `make lint-markdown`; `make lint-spelling`; `grep -c "Database is not available\|Cache is not working\|Message broker is not available" docs/operations.md` (expected: at least `3`).

**Acceptance Criteria:**

**Given** `docs/operations.md`
**When** it is read
**Then** it covers migrations and rollback with `make migrate` and `make migrate-rollback` and the rule that an applied migration is never edited; a health-probe runbook with one entry per failure message giving causes and checks; the log schema and request ids; the shutdown sequence with the non-zero exit after a forced end of the grace period; the toolchain update procedure (three places kept equal, one labelled pull request); the tool-version review (a version only up to what the pinned install action lists); a section for the first measurements of `make bootstrap`, `make ci`, the mutation run and the pull-request jobs (Story 10.5 enters the local values); the entry format of `.trivyignore` and `.gitleaksignore`

**Given** `docs/load-testing.md`
**When** it is read
**Then** it describes the four profiles and their shapes, the budgets of the `health` scenario and the budget rule of NFR1, a table for measured p99 values with a date column (Story 10.5 enters the first local value; the hosted-runner value follows the first pipeline run), adding a scenario, the isolated environment, the AWS runner with the credentials it reads from the environment and its costs, and states that the AWS flow is verified only against the LocalStack dry run

**Given** `docs/extension-seams.md` and `docs/repository-settings.md`
**When** they are read
**Then** the first covers a Redis adapter through the `Cache` port, tracing and metrics at the request span, an authentication layer and an outbox; the second names every job of the pull-request workflows (G31 rule (d)), marking the gate jobs, the `changes` job of each workflow under its own check name (`Changes (Quality)` and so on) and the aggregators `mutation-tests` and `bats` as required status checks and `mutation-plan`, `mutation-shard`, `bats-group`, `clippy-sarif-upload` and `codeql` as not required; it states squash merging with the pull-request title as commit subject, permission for the release identity to push to `main`, private vulnerability reporting, the secrets and variables, the five labels to create, the code-scanning setup (`CODE_SCANNING_ENABLED`) and the governance follow-ups (CODEOWNERS, rulesets, auto-merge, stale bot, OSSF Scorecard, multi-architecture images)

**Given** `.github/AUTORELEASE.md`
**When** it is read
**Then** it covers the release flow, the version rules including that a breaking change raises the minor number below `1.0.0`, App creation with only the permissions the workflows use, the two secrets, the App's permission to push to `main` and the behaviour without secrets

**Given** the two sync guides
**When** they are read
**Then** each covers credentials, permissions, schedule, `.templatesyncignore`, the label a sync needs when it changes protected configuration, keeping only one flavour, and the template changes that do not propagate because their files are ignored, with how to apply them by hand

### Story 8.8: Tracked planning bundle and parity matrix

As a platform maintainer,
I want the planning artifacts of this release tracked under `specs/` together with a parity matrix,
So that every PHP capability id can be traced to a requirement and a story, and later planning bundles have a place and a format.

Track: D.
Depends on: 1.4.
Owns: `specs/rust-service-template-bootstrap/` (the directory already exists with earlier copies; this story adds the parity matrix and the bundle index and copies the sources again).
Touches: none.
Implements: FR112, NFR27.
Reference: architecture-delivery.md "AI Guidance and Documentation" ("`.claude/commands` and bmalph": `specs/` layout), "Repository File Manifest" (`tests/Load/` and `specs/` table: the eleven bundle files of the layout and `scope-decisions.md`); scope-decisions.md section 8.10; PRD "Traceability" (catalog ids to requirements, audit problems P1 to P28); `research/capability-catalog.md`.
Verify: `for f in README research capability-catalog product-brief prd architecture architecture-delivery epics implementation-readiness parity-matrix run-summary; do test -s specs/rust-service-template-bootstrap/$f.md || echo "missing $f"; done` (expected: no output); `for f in prd architecture architecture-delivery epics product-brief research; do cmp _bmad-output/planning-artifacts/$f.md specs/rust-service-template-bootstrap/$f.md; done`.

**Acceptance Criteria:**

**Given** the bundle directory
**When** the first loop of the Verify line is run
**Then** it prints nothing: the eleven files exist and are not empty (a file whose source does not exist yet, such as the run summary, is created with its title and one line that names the orchestrator as its author, and is replaced by the final copy before Story 10.5)

**Given** the files that have a source under `_bmad-output/planning-artifacts/` (including `research/capability-catalog.md`, `implementation-readiness.md` and the run summary when present there)
**When** each is compared with its source using `cmp`
**Then** every comparison exits 0; the sources still change after this story (deviation entries, corrections of the planning documents), so the orchestrator copies them again before the last story and Story 10.5 repeats both loops of the Verify line

**Given** `parity-matrix.md`
**When** it is read
**Then** it has one row for each of the 196 PHP capability ids and each of the 42 `NEW` ids with its disposition, the requirement ids of the PRD traceability index and the story ids of this document that implement those requirements; the 11 `DROP` ids carry their reason; a second table gives the disposition of P1 to P28

**Given** the matrix
**When** its rows are counted by disposition
**Then** 185 PHP ids are delivered, 11 are dropped and 42 additions are delivered, and no row has an empty requirement cell unless it is a `DROP` row

**Given** `README.md` of the bundle
**When** it is read
**Then** it lists the eleven files of the layout and `scope-decisions.md` with one line each and states that the raw research reports stay local under `_bmad-output/`

## Epic 9: Every Gate Proven Locally

A reviewer gets proof that a green pipeline means something: the production image passes Schemathesis and the K6 smoke profile in an isolated stack, coverage reaches 100% lines and functions, no mutant survives, a Bats suite runs the command surface and plants a violation for every gate, the repository-invariants and documentation-drift checkers exist with a test per rule, and every gate has a negative test.

### Story 9.1: Isolated production-image stack with Schemathesis and smoke load

As a reviewer,
I want conformance and load tests to run against the production image in a compose project of its own that is always removed,
So that the contract is checked against what ships and no test can touch development data.

Track: E.
Depends on: 3.1, 3.4, 7.6.
Owns: `scripts/with-load-test-stack.sh`, `scripts/setup-load-test-db.sh`.
Touches: `.github/openapi-spec/spec.yaml` and files under `crates/` (only if Schemathesis against the production image exposes a difference that the early run of Story 7.6 did not show; the fix and the regenerated snapshot are then part of this story, which is why Story 9.2 starts after it).
Implements: FR53, FR76, FR77, FR84, NFR1, NFR24, G21, G22.
Reference: architecture-delivery.md DD-19, "Containers and Compose" ("Compose files": the isolated combination and "How the isolated environment avoids development data"), "Command Surface" (targets `schemathesis-validate`, `smoke-load-tests`, `execute-load-tests-script`, `setup-load-test-db`; script rows `with-load-test-stack.sh`, `setup-load-test-db.sh`; variables `LOAD_TEST_PROJECT`, `LOAD_TEST_APP_PORT`, `SCHEMATHESIS_URL`, `BUILD_IMAGE`), "Load Testing" ("Results"); architecture.md "Architecture Validation Results" ("Risks": Schemathesis ran against the first form of the page parameters).
Pitfalls: PF-12, PF-22, PF-50, PF-57.
Verify: `make schemathesis-validate`; `make smoke-load-tests`; `make execute-load-tests-script scenario=health runAverage=false runStress=false runSpike=false`; `make lint-shell`; `docker compose -p "$(basename "$PWD" | tr '[:upper:]' '[:lower:]')-load" ps -q | wc -l` (expected `0`).

**Acceptance Criteria:**

**Given** the development services are running (`make start-deps`, `make migrate`) with one known row in the `example_api_resources` table of the development database
**When** `make schemathesis-validate` is run
**Then** it exits 0: the production image is built (or, with `BUILD_IMAGE=0`, the image that is already built is used with `--no-build`), the project `<project>-load` starts with its own network and volumes, its database is created and migrated by `setup-load-test-db.sh`, Schemathesis runs with `--checks all --phases=examples,coverage` against `http://localhost:18081` and reports 0 failed checks
**And** afterwards the isolated project has no container and no volume, the development containers have the same ids and start times as before, and the development table still holds exactly that one row

**Given** `SCHEMATHESIS_URL` set by the caller to a closed port (`http://127.0.0.1:9`)
**When** `make schemathesis-validate` is run
**Then** no isolated project is started and Schemathesis reports the failed connection; `localhost` and `127.0.0.1` are handled alike

**Given** the isolated stack cannot become healthy
**When** `make smoke-load-tests LOAD_TEST_APP_PORT=<occupied port>` is run
**Then** the wrapper prints the container logs, ends with its exit status 2 and still removes the project with its volumes; `make setup-load-test-db` run while the project is not up fails with the script's exit status 2

**Given** the kit of Story 3.4
**When** `make smoke-load-tests` is run
**Then** it exits 0, the p99 of the `health` scenario is within its budget and its check rate above 99%, `tests/Load/results/smoke-health.html` exists and is owned by the invoking user, and the isolated project is removed

**Given** a git-ignored `tests/Load/config.json` with the smoke budget of `health` set to `0.001`
**When** `make smoke-load-tests` is run
**Then** it exits non-zero because the threshold is crossed, and the project is still removed

**Given** the first passing smoke run
**When** its K6 summary is read
**Then** the p99 of the `health` smoke profile is stated in the completion notes with the date and the machine it was measured on (Story 10.5 enters it into `docs/load-testing.md`), and the committed budgets are unchanged: `cargo xtask config-guard --base HEAD` passes

### Story 9.2: Coverage at 100% lines and functions

As a reviewer,
I want `make tests-with-coverage` to pass with 100% line and function coverage of production source from the unit and integration suites alone,
So that every line of the service and of the template-owned tooling is exercised by a test that can fail.

Track: E.
Depends on: 2.4, 7.6, 7.7.
Owns: none.
Touches: sibling `tests.rs` files and `tests/integration/` files under `crates/` and `xtask/` (tests added for uncovered lines); production source under `crates/` and `xtask/` only where a line cannot be reached and is reshaped as rule 23 prescribes.
Implements: FR50, FR52, FR58, NFR19, NFR41, G11, G12, G14.
Reference: architecture.md AD-24 ("Coverage"), "Quality Gates" ("Gate commands": QG-16 and the coverage sequence), "Test Architecture" ("Coverage and mutation", "Levels"), "Implementation Patterns and Consistency Rules" ("Structure and tests", "Code shape imposed by the tools" rules 21 to 23); architecture-delivery.md "Command Surface" (targets `tests-with-coverage`, `coverage-xml`, `coverage-html`; "`make/gates.mk`": the coverage group as exclusion record).
Pitfalls: PF-30, PF-36, PF-40.
Verify: `make setup-test-db`; `make tests-with-coverage`; `make coverage-xml`; `make coverage-html`; `make doc-tests`; `git status --porcelain`.

**Acceptance Criteria:**

**Given** the complete workspace and the compose services
**When** `make tests-with-coverage` is run
**Then** it exits 0: the unit and integration suites run once under instrumentation with the nextest profile `ci`, the end-to-end suite is not part of the run, the floor command is the last of the sequence, and the report shows 100% lines and 100% functions with no `--ignore-filename-regex` and no `--exclude` flag on any command

**Given** the gate fails although every code story showed its own files at 100%
**When** the uncovered lines of `target/coverage/html` or `lcov.info` are read
**Then** the story iterates: add a unit or integration test in the crate that owns the code (or reshape an unreachable error path as rule 23 prescribes), run the gate again, and repeat until it exits 0; no exclusion record, no `#[cfg(coverage)]` branch and no test without an assertion is added

**Given** the passing run
**When** the per-file table of the report is read
**Then** every production file of the nine layer crates, of `service` (including `main.rs`, `lib.rs`, `command/*.rs`, `telemetry.rs` and `bootstrap.rs`) and of `xtask` (including `main.rs` and `scaffold/lock.rs`) is listed at 100% lines, and no file under a directory named `tests` and no `tests.rs` file is listed

**Given** an uncalled private function with a reasoned `#[expect(dead_code)]` appended to `xtask/src/cli.rs`
**When** `make tests-with-coverage` is run
**Then** it exits non-zero at the floor command while `lcov.info` and the HTML report were still written; `git checkout -- xtask/src/cli.rs` restores the file

**Given** the report targets
**When** `make coverage-xml` and `make coverage-html` are run
**Then** `target/coverage/coverage.xml` (Cobertura), `target/coverage/lcov.info` and `target/coverage/html/index.html` exist under the git-ignored `target/` directory, `target/nextest/ci/junit.xml` exists, and the output of `git status --porcelain` is the same before and after

### Story 9.3: Mutation testing with zero surviving mutants

As a reviewer,
I want `make mutation-tests` to report no surviving and no timed-out mutant over the whole mutation scope,
So that tests are shown to detect faults, not only to execute lines.

Track: E.
Depends on: 9.2.
Owns: none.
Touches: test files under `crates/` and `xtask/` (assertions added to kill survivors); `.cargo/mutants.toml` (only to record an equivalent mutant in the exclusion list, one entry with a comment that explains the equivalence).
Implements: FR59, NFR20, G15.
Reference: architecture.md AD-24 ("Mutation"), "Quality Gates" ("Gate commands": QG-17 and its note; "`.cargo/mutants.toml`"; "`.config/nextest.toml`": profile `mutants`), "Test Architecture" ("Coverage and mutation"); architecture-delivery.md DD-09, "Command Surface" (target `mutation-tests`; variable `MUTANTS_SHARD`; "`make/gates.mk`": `MUTANTS_CMD`).
Pitfalls: PF-11, PF-32, PF-41.
Verify: `make setup-test-db`; `for k in 0 1 2 3 4 5 6 7; do make mutation-tests MUTANTS_SHARD=$k/8 || exit 1; done`; `make tests-with-coverage`; `git status --porcelain`.

**Acceptance Criteria:**

**Given** the workspace at 100% coverage, in which every code story has already run the mutants of its own package
**When** the eight shard runs of the Verify line are executed
**Then** every run exits 0 and its `mutants.out/outcomes.json` reports 0 missed and 0 timed-out mutants (a timed-out mutant counts as surviving); the command line unsets both `CARGO_TARGET_DIR` and `CARGO_BUILD_WARNINGS` and passes `--cargo-arg=--locked`; each mutant is judged by the unit and integration tests of its own crate with the nextest profile `mutants`

**Given** a run that reports survivors
**When** `mutants.out/missed.txt` is read
**Then** the story iterates: add or strengthen a test in the crate that contains the mutant, rerun the affected file with `scripts/with-env.sh .env .env.test -- env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --workspace --cargo-arg=--locked -f <path>`, and repeat until the shard exits 0; a mutant is excluded only when it is equivalent, as one `exclude_re` entry with a comment, and the number of such entries is stated in the completion notes

**Given** the eight passing runs
**When** their output is read
**Then** the number of mutants, the mean seconds per mutant and the duration of each shard are stated in the completion notes (the input for the shard divisor of DD-09, which Story 10.5 records in `docs/operations.md`)

**Given** the passing full run
**When** `make tests-with-coverage` and `git status --porcelain` are run
**Then** coverage is still 100% and the status output is the same as before the runs (`mutants.out/` is ignored)

### Story 9.4: Bats harness with general, hook and bmalph tests

As a backend engineer,
I want a Bats harness that always restores planted files, and tests of the general targets, the assistant hook, the commit convention and the bmalph targets,
So that the Makefile itself is tested and an interrupted test never leaves my tree modified.

Track: E.
Depends on: 3.6, 8.1, 8.3.
Owns: `tests/CLI/bats/test_helper.bash`, `tests/CLI/bats/make_general.bats`, `tests/CLI/bats/make_hooks_and_commits.bats`, `tests/CLI/bats/make_bmalph.bats`, `tests/CLI/bats/fixtures/failing_suite/`.
Touches: none.
Implements: FR60, FR67, FR70, FR111, NFR21, NFR37, G16, G28.
Reference: architecture-delivery.md "CLI Tests (Bats)" (the harness description, the rows `make_general.bats`, `make_hooks_and_commits.bats` and `make_bmalph.bats`, and the time paragraph), DD-06, DD-07, DD-18, "Command Surface" (target `bats`; hooks `BATS_PATH`, `BATS_ARGS`).
Pitfalls: PF-44, PF-49.
Verify: `make bats BATS_PATH="tests/CLI/bats/make_general.bats tests/CLI/bats/make_hooks_and_commits.bats tests/CLI/bats/make_bmalph.bats"`; `d=$(mktemp -d) && cp tests/CLI/bats/fixtures/failing_suite/failing.bats.txt "$d/failing.bats" && make bats BATS_PATH="$d"` (expected: non-zero exit); `make lint-shell`; `git status --porcelain`.

**Acceptance Criteria:**

**Given** the three files and a running container engine
**When** the first command of the Verify line is run
**Then** it exits 0 and every test of the three table rows, in both columns, is present and passes, each asserting the exit status and the tool's own output

**Given** `test_helper.bash`
**When** a test plants a change with `plant_append`, `plant_replace`, `plant_new` or `plant_keep` (which registers a file that a tool is about to rewrite, such as `Cargo.lock`) and then fails, or the run is interrupted
**Then** `restore_planted` (called by every `teardown` and at the start of `_common_setup`) restores the original files from `target/bats-restore/`, and planting refuses to touch a file with uncommitted changes by skipping the test with a message; `require_docker` and `require_command <name>` skip with a reason
**And** the output of `git status --porcelain` is the same before and after any run

**Given** the general tests
**When** they run
**Then** `make` and `make help` print the three header lines and every target of the Makefile database has a help line; `make doctor` passes and prints the platform line; `make ci-list` equals the verification block of `AGENTS.md`; `make -n ci` shows the seven stages in order; `make ci CI_STAGES=help` prints `CI checks successfully passed!`; `make purge` removes a planted report and keeps `target/debug`
**And** `scripts/doctor.sh` with a reduced `PATH` exits 1 and prints `[MISSING] cargo`, and `make ci CI_STAGES=validate-configuration base_ref=HEAD` with a planted protected change exits non-zero without the success line (the base is `HEAD`, because the default base of the bootstrap branch holds no protected item)

**Given** the assistant-hook tests
**When** they run
**Then** the hook allows an unprotected path, allows a protected one with `ALLOW_PROTECTED_CONFIG_EDIT=1`, and allows the edit of a dependency line in the root `Cargo.toml` (`pre`, the edit, `post`); it refuses an edit of `clippy.toml` (`pre`, exit status 2, the refusal text), and after an edit of a lint level in the root `Cargo.toml` its `post` call exits 2 and the file is back in its previous state

**Given** the hook, commit-message and bmalph tests
**When** they run
**Then** the commit-message examples are accepted and rejected as in Story 3.6, in title mode and in hook mode; `install-hooks` sets `core.hooksPath` and the test restores the previous value, so no hook stays active in the implementation checkout; `bmalph-install` in an isolated `HOME` prints the pinned version, both dry-run targets leave `git status` unchanged and an unsupported platform fails; the bmalph tests skip with a reason when npm is absent

**Given** the fixture suite `tests/CLI/bats/fixtures/failing_suite/failing.bats.txt`
**When** it is copied to a temporary directory as `failing.bats` and `make bats BATS_PATH=<that directory>` is run
**Then** the target exits non-zero and prints `not ok`, which is the negative test of gate G16; in the repository the fixture keeps the extension `.bats.txt`, which no linter and no default Bats run picks up

**Given** the run of the first command of the Verify line
**When** its TAP output is read
**Then** the plan line is not `1..0`, no test is skipped except the bmalph tests when npm is absent and the hook tests when `jq` is absent, and each of the three files has exactly one line `# bats file_tags=surface`

### Story 9.5: Negative Bats tests for the static gates

As a reviewer,
I want one Make-level test per static gate that plants a violation and asserts a non-zero exit with the gate's own diagnostic,
So that a gate which silently stops failing is detected.

Track: E.
Depends on: 2.4, 7.6, 7.7, 9.4.
Owns: `tests/CLI/bats/make_quality_gates.bats`, `tests/CLI/bats/fixtures/badly_formatted.rs.txt`, `tests/CLI/bats/fixtures/unwrap_used.rs.txt`, `tests/CLI/bats/fixtures/banned_api.rs.txt`, `tests/CLI/bats/fixtures/too_many_arguments.rs.txt`, `tests/CLI/bats/fixtures/unused_import_in_test.rs.txt`, `tests/CLI/bats/fixtures/unsafe_block.rs.txt`, `tests/CLI/bats/fixtures/isolation_violation.rs.txt`.
Touches: none.
Implements: FR67, FR124, NFR21, NFR23, NFR28, G1, G2, G3, G4, G5, G6, G7, G8, G25, G29.
Reference: architecture-delivery.md "CLI Tests (Bats)" (row `make_quality_gates.bats`), DD-02 (no second passing run of a gate), DD-07 (plants in the leaf crate `xtask`; test hooks `ARCH_ARGS`, `DENY_ARGS`); architecture.md "Quality Gates" ("Gate commands": the paragraph "Negative tests (NFR21)").
Pitfalls: PF-08, PF-44.
Verify: `make bats BATS_PATH=tests/CLI/bats/make_quality_gates.bats`; `make unused-dependencies`; `make pre-commit`; `git status --porcelain`.

**Acceptance Criteria:**

**Given** the complete workspace
**When** the tests of the second table column run
**Then** `pre-commit` passes, and `validate-configuration base_ref=HEAD` passes with a planted change and `CONFIG_CHANGE_APPROVED=true`; passing runs of gate targets are not repeated in the suite
**And** `make unused-dependencies` of the Verify line exits 0 because every crate now uses what its manifest declares (FR124), which is why this story waits for the complete workspace

**Given** the negative tests for G1, G2, G3 and G4
**When** they plant unformatted lines, an `unwrap`, a `SystemTime::now()` call from `fixtures/banned_api.rs.txt`, a six-parameter plain function (each appended to `xtask/src/main.rs`) and `ARCH_ARGS` pointing at the checker's fixture workspace
**Then** the targets exit non-zero with `Diff in`, `unwrap_used`, `disallowed_methods` (both from `static-analysis`), `too many arguments (6/5)` and `arch::layer` respectively

**Given** the negative tests for G5, G6 and G8
**When** they change the workspace version without a lock update, set `DATABASE_URL=mysql://x`, and add `humantime.workspace = true` to `xtask/Cargo.toml` with `Cargo.lock` registered through `plant_keep` and refreshed by `cargo update --workspace --offline`
**Then** `validate-manifest` fails mentioning `--locked`, `check-requirements` fails with the status 2 of the command and names `DATABASE_URL`, and `unused-dependencies` exits non-zero naming the dependency and leaves the lock file as it found it

**Given** the three negative tests for G7, one per policy
**When** `deny.toml` is replaced by a variant derived from it at test time with an empty licence allow-list, by one with `allow-registry = []`, and `DENY_ARGS` points at a project generated at test time whose lock file holds a crate version with a published advisory
**Then** `check-security` exits non-zero each time with the licence, source and advisory diagnostic respectively, and no vulnerable manifest is committed

**Given** the negative tests for G29 and G25
**When** the `RawValue` function of DD-07 (`fixtures/isolation_violation.rs.txt`) is appended to `xtask/src/main.rs`, and a comment is appended to `clippy.toml` with `base_ref=HEAD`
**Then** `check-crate-isolation` fails on `xtask` although `cargo check --workspace` passes; `validate-configuration` prints `Modification of locked configuration file is not allowed: clippy.toml`, `[FAIL]` and `Approve with the label approved-config-change`, and fails with the status 2 of the command for `base_ref=no-such-ref`

**Given** any test of the file fails or is interrupted
**When** the next test starts
**Then** the planted files were restored, no test depends on a crate other than `xtask`, and the output of `git status --porcelain` is the same as before the run

**Given** the run of the first command of the Verify line
**When** its TAP output is read
**Then** the plan line is not `1..0`, no test is skipped, and the file has exactly one line `# bats file_tags=quality`

### Story 9.6: Changed-code mutation script and Bats tests for the test levels, database targets and scaffolding

As a reviewer,
I want the changed-code mutation mode and Make-level tests for the unit, integration, end-to-end, coverage, mutation and query-metadata gates, the database targets and the scaffolding target,
So that each test-level gate is shown failing on a planted fault and a pull request mutates only the code it changes.

Track: E.
Depends on: 2.4, 7.6, 7.7, 9.4.
Owns: `scripts/mutation-diff.sh`, `tests/CLI/bats/make_tests.bats`, `tests/CLI/bats/make_database.bats`, `tests/CLI/bats/make_scaffolding.bats`, `tests/CLI/bats/fixtures/uncovered_function.rs.txt`, `tests/CLI/bats/fixtures/failing_unit_test.rs.txt`, `tests/CLI/bats/fixtures/failing_integration_test.rs.txt`, `tests/CLI/bats/fixtures/undefined_step.feature.txt`.
Touches: none.
Implements: FR39, FR53, FR59, FR67, NFR18, NFR21, G11, G12, G13, G14, G15, G30.
Reference: architecture-delivery.md "CLI Tests (Bats)" (rows `make_tests.bats`, `make_database.bats`, `make_scaffolding.bats`), DD-07, DD-09, "Command Surface" (target `mutation-tests-diff`; script row `mutation-diff.sh` with the signature `<base_ref> [--list] -- <mutation command>`); architecture.md "Quality Gates" ("Gate commands": QG-18 and its note; the paragraph "Negative tests (NFR21)": where the plants for G14 and G15 must sit).
Pitfalls: PF-11, PF-32, PF-44, PF-46.
Verify: `make bats BATS_PATH="tests/CLI/bats/make_tests.bats tests/CLI/bats/make_database.bats tests/CLI/bats/make_scaffolding.bats"`; `make mutation-tests-diff base_ref=HEAD` (expected on a committed tree: exit 0 with the message that the diff holds no production Rust change); `make mutation-tests-diff base_ref=no-such-ref` (expected: non-zero with the base-ref message); `make lint-shell`; `git status --porcelain`.

**Acceptance Criteria:**

**Given** `scripts/mutation-diff.sh`
**When** `make mutation-tests-diff base_ref=HEAD` is run on a committed tree, with an uncommitted change to one production function, and with `base_ref=no-such-ref`
**Then** the first run exits 0 with a message that the diff contains no production Rust change; the second tests only the mutants of the changed function (the diff is taken from the merge base to the working tree and written to `target/mutants.diff`); the third ends with the script's exit status 2 and `mutation-tests-diff could not run: base ref no-such-ref is not available`
**And** `scripts/with-env.sh .env .env.test -- scripts/mutation-diff.sh <base> --list -- env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --workspace --cargo-arg=--locked` prints only the number of mutants in the diff, which `scripts/ci/mutation-plan.sh` (Story 10.1) uses; the script contains no cargo command of its own

**Given** the compose services
**When** the test of the second column of `make_tests.bats` runs
**Then** `setup-test-db` creates `app_test` and leaves a marker table of the development database untouched

**Given** the negative tests for G11, G12 and G13
**When** a failing test is appended to `xtask/src/cli/tests.rs`, a failing test to `xtask/tests/integration/main.rs`, and a feature with an undefined step is planted under `features/`
**Then** `unit-tests`, `integration-tests` and `e2e-tests` exit non-zero, the last with the report of the unmatched step (`Step doesn't match any function`)
**And** with both failing tests appended at once, `tests-with-coverage` exits non-zero and names both tests

**Given** the negative tests for G14, G15 and G30
**When** the fixture function `planted` (`fixtures/uncovered_function.rs.txt`) is appended to `xtask/src/cli.rs`, and one file of `.sqlx/` is removed
**Then** `tests-with-coverage` exits non-zero, `target/coverage/lcov.info` exists and has an `FNDA:0,` record for `planted`; `CI=true make mutation-tests-diff base_ref=HEAD` exits non-zero with the status 2 of cargo-mutants and prints a `MISSED` line with `replace planted -> bool with true`, and `base_ref=no-such-ref` fails with the base-ref message; `validate-sqlx-metadata` exits non-zero

**Given** a throw-away database on the server of the development compose project, named by `DATABASE_URL` (the tests that apply migrations are skipped with a reason in container mode, which forwards no `DATABASE_URL`)
**When** the tests of `make_database.bats` run
**Then** `make migrate` twice changes nothing the second time, `migrate-rollback` followed by `migrate` restores the schema, neither touches the development database, and `create-migration` without `name` fails with the usage line (exit status 2 of the recipe)

**Given** a committed tree
**When** `make new-context group=bats-group context=bats-context` runs in `make_scaffolding.bats`
**Then** three crates are created, `validate-manifest`, `architecture` and `check-crate-isolation` pass with no path of `cargo xtask config-guard --list` modified, and `unused-dependencies` gives the same status and output as on the tree before the scaffolding (it passes when it passed there; the new crates add no finding); missing parameters fail with the usage line and an existing context name is refused
**And** `teardown` removes the crates and restores `Cargo.toml` and `Cargo.lock` (registered with `plant_replace` and `plant_keep`), so the output of `git status --porcelain` is the same before and after the file

**Given** the run of the first command of the Verify line
**When** its TAP output is read
**Then** the plan line is not `1..0`, no test is skipped, `make_tests.bats` has exactly one line `# bats file_tags=tests`, and `make_database.bats` and `make_scaffolding.bats` each have exactly one line `# bats file_tags=quality`

### Story 9.7: Bats tests for contract, load and security targets

As a reviewer,
I want Make-level tests for the contract, load and security gates, including an inspection of the production image and the production compose reference,
So that the delivery gates and the hardening claims are proven by commands, not by reading files.

Track: E.
Depends on: 3.2, 3.3, 3.5, 9.1, 9.4.
Owns: `tests/CLI/bats/make_contracts.bats`, `tests/CLI/bats/make_load_tests.bats`, `tests/CLI/bats/make_security.bats`, `tests/CLI/bats/fixtures/bad_load_script.js.txt`, `tests/CLI/bats/fixtures/unformatted_load_script.js.txt`, `tests/CLI/bats/fixtures/image_listing_planted.txt`.
Touches: none.
Implements: FR67, FR85, FR91, NFR3, NFR12, NFR21, G9, G10, G17, G18, G19, G20, G21, G22, G24.
Reference: architecture-delivery.md "CLI Tests (Bats)" (rows `make_contracts.bats`, `make_load_tests.bats`, `make_security.bats`), DD-07, DD-19, DD-21, "Containers and Compose" (the table "Facts that follow from the files"), "Delivery Validation" ("Measurement items and readings of requirement wording": item 9, the snapshot gate regenerates in place).
Pitfalls: PF-44, PF-50.
Verify: `make bats BATS_PATH="tests/CLI/bats/make_contracts.bats tests/CLI/bats/make_load_tests.bats tests/CLI/bats/make_security.bats"`; `git status --porcelain`.

**Acceptance Criteria:**

**Given** the committed snapshots
**When** the tests of the second column of `make_contracts.bats` run
**Then** both `generate-*` targets leave `git status` empty for the two snapshot directories; `openapi-diff` and `graphql-diff` against a commit without snapshots (an empty tree made with `git commit-tree`) pass with the first-version notice; with `SCHEMATHESIS_URL` set to a closed port no container starts and Schemathesis reports the failed connection

**Given** the negative contract tests
**When** they run
**Then** G17 fails once for a changed OpenAPI snapshot and once for a changed GraphQL snapshot, each staged in the index, with the target regenerating and reporting the file and `teardown` unstaging it; G18 fails when an operation description is removed; G19 fails with `response-required-property-removed` and the label line, passes with `BREAKING_CHANGE_APPROVED=true` and fails with the script's status 2 for `base_ref=no-such-ref`; G20 fails when a field is removed (`breaking change`); G21 fails when an undocumented path is added to the working snapshot, and no container of the isolated project remains

**Given** the load tests
**When** they run
**Then** `execute-load-tests-script scenario=health runAverage=false runStress=false runSpike=false` writes `health.html`, removes the isolated project and leaves the development containers unchanged; `aws-load-tests LOCAL_MODE_ENV=true` prints `Launched instance` and `You can access the S3 bucket here`, and `aws-load-tests-cleanup LOCAL_MODE_ENV=true` succeeds; after these runs `lint-prettier`, `lint-yaml` and `lint-toml` still pass
**And** G22: `smoke-load-tests` with a local budget of `0.001` fails the thresholds and `smoke-health.html` is written; with the budget `-1`, `tests/Load/execute-load-test.sh health` prints `Invalid threshold value` before any request; `execute-load-tests-script` without `scenario` fails with usage and starts nothing; G24: the planted script with `console.log` and `==` fails `lint-load-tests`, and so does a planted script that ESLint accepts but Prettier does not

**Given** the security tests
**When** they run
**Then** G9: a key pair generated at test time in an untracked file makes `secrets-scan` report `leaks found`; G10: `IMAGE_TO_SCAN=$VULNERABLE_FIXTURE_IMAGE` makes `image-scan` exit non-zero with a CRITICAL finding; passing runs of the two gates are not repeated

**Given** the production image and the rendered production reference
**When** the inspection tests run
**Then** the image runs as uid `65532`, has no `/bin/sh`, holds no shell, package manager, compiler or test file (its `docker export` listing, which has no file or link named `sh`, `ash`, `bash`, `dash`, `ksh`, `mksh`, `zsh`, `busybox` or `toybox` in any directory; NFR12), is at most 100 MiB and becomes healthy through its own healthcheck (interval 10 s, timeout 3 s, 3 retries, start period at most 30 s) with a read-only root and no capabilities; the rendered reference has resource limits, a read-only root file system, all capabilities dropped and `no-new-privileges`; rendering it with `--env-file /dev/null` fails and names each required variable, `DATABASE_URL` and `AWS_REGION`, with one of them unset at a time (a Compose version may stop at the first missing variable, and its order is not fixed)

**Given** the run of the Verify line
**When** its TAP output is read
**Then** the plan line is not `1..0`, no test is skipped, `make_contracts.bats` has exactly one line `# bats file_tags=contracts`, and `make_load_tests.bats` and `make_security.bats` each have exactly one line `# bats file_tags=load`

### Story 9.8: Repository-invariants gate

As a platform maintainer,
I want `make validate-invariants` to check pins, toolchain agreement, the gate set, the repository name, the licence, blanket suppressions and the execution switch,
So that the properties that keep the template safe to synchronise cannot regress silently.

Track: E.
Depends on: 9.4.
Owns: `scripts/check-invariants.sh`, `tests/CLI/bats/check_invariants.bats`.
Touches: none.
Implements: FR92, FR125, NFR13, NFR14, NFR16, NFR22, NFR36, NFR38, NFR47, G31.
Reference: architecture-delivery.md "Command Surface" (script row `check-invariants.sh` and the rule table (a) to (h); target `validate-invariants`), DD-02 (CI-only list), DD-04, DD-08, DD-20, DD-23, "CLI Tests (Bats)" (row `check_invariants.bats`), "Delivery Validation" ("Measurement items and readings of requirement wording": items 6, 7 and 11).
Pitfalls: PF-44, PF-64.
Verify: `make bats BATS_PATH=tests/CLI/bats/check_invariants.bats`; `make lint-shell`; `make validate-invariants 2>&1 | grep -E '^\((b|c|e|f|g|h)\)'` (expected: no output).

**Acceptance Criteria:**

**Given** a small tree generated in the test's temporary directory that satisfies every rule
**When** `scripts/check-invariants.sh --root <tree>` is run
**Then** it exits 0 and prints nothing

**Given** variants of that tree, one per rule (a) to (h) and per kind within a rule
**When** the script is run on each
**Then** each run exits 1 with at least one line prefixed by that rule's letter, and `check_invariants.bats` holds one test per variant that asserts the letter, among them: `uses: actions/checkout@v7`, a workflow without `permissions` and a job-level `permissions` block without `contents: read` (a); an image reference with `latest`, one without a digest and a development-container feature referenced by tag (b); `rust-version` of the workspace manifest differing from `rust-toolchain.toml` and from the builder image tag (c); a workflow set that omits one gate target of `make ci-list`, a Bats file without a group tag and a job name missing from `docs/repository-settings.md` (d); the literal repository name in a script (e); a second licence identifier in a manifest (f); `#[allow`, `#![expect(` in a crate root and a suppression that names `clippy::pedantic` (g); a class TOOL recipe that calls `cargo` without `$(EXEC)` and a script called outside `$(EXEC)` that starts cargo (h)

**Given** the real repository before any workflow file exists
**When** the last command of the Verify line is run
**Then** it prints nothing: no line is prefixed with (b), (c), (e), (f), (g) or (h); the only findings allowed at this point are those of rules (a) and (d) that follow from the absent workflow directory, and Story 10.4 requires the full pass

**Given** rule (b)
**When** the `database` image `postgres:${DB_VERSION:-16}-alpine` is evaluated
**Then** it is the one recorded exception (version tag from `DB_VERSION`, no digest) and is not reported

**Given** rule (e)
**When** the classes of DD-23 are searched
**Then** the repository name is accepted only in the sync-source setting of the two sync workflows and in the workspace manifest with its lock file, and `bmalph/config.json`, the README badges and `specs/` are outside the searched classes

**Given** the run of the first command of the Verify line
**When** its TAP output is read
**Then** the plan line is not `1..0`, no test is skipped, and the file has exactly one line `# bats file_tags=repository`

### Story 9.9: Documentation-drift gate

As an AI coding assistant,
I want `make docs-check` to fail when a document cites a command or path that does not exist, when a second verification list appears, or when a reference document is incomplete,
So that the guidance I follow is true.

Track: E.
Depends on: 7.6, 8.1, 8.2, 8.3, 8.4, 8.5, 8.6, 8.7, 8.8, 9.4.
Owns: `scripts/check-docs.sh`, `tests/CLI/bats/check_docs.bats`.
Touches: tracked Markdown files owned by Stories 8.1 to 8.7 (only to correct a citation the gate reports).
Implements: FR110, NFR43, NFR44, NFR45, G26.
Reference: architecture-delivery.md DD-17 (rules (1) to (7) and the consequences), "Command Surface" (script row `check-docs.sh`; targets `docs-check`, `ci-list`), "AI Guidance and Documentation" ("Canonical verification list", skill file format), "CLI Tests (Bats)" (row `check_docs.bats`).
Pitfalls: PF-44.
Verify: `make docs-check` (expected: exit 0, or only the rule (3) lines that the first criterion allows); `make bats BATS_PATH=tests/CLI/bats/check_docs.bats`; `make lint-shell`; `make lint-markdown`.

**Acceptance Criteria:**

**Given** the repository with all documents of Epic 8 and no workflow file yet
**When** `make docs-check` is run
**Then** it reads every tracked Markdown file except `CHANGELOG.md` and `specs/**` and exits 0, or its only findings are rule (3) lines for paths under `.github/workflows/` and for `.github/dependabot.yml`, which Stories 10.1 to 10.3 create; any other reported citation is corrected in the document, never by weakening a rule

**Given** seven generated trees passed with `--root` and `--service-help`, each violating one rule: (1) `make no-such-target` cited; (2) `service no-such-command` cited, and a slash command without a file under `.claude/commands/`; (3) a backticked `docs/missing.md`; (4) a verification block in `AGENTS.md` that differs from `make ci-list`, and a second file with a fenced block of three gate targets; (5) a variable missing from `docs/environment.md`; (6) a target missing from `docs/make-targets.md`; (7) a skill whose `name` differs from its directory
**When** the script is run on each
**Then** each run exits 1 with a line prefixed by the rule number, and `check_docs.bats` holds one test per rule that asserts the number, plus one passing run on a small valid tree with help texts

**Given** rule (3)
**When** a backticked token names a git-ignored path (`_bmad/`, `target/coverage/html`) or contains a placeholder (`specs/<bundle-id>/`)
**Then** it is not reported: ignored paths are accepted and a token is checked only up to its first placeholder or wildcard character

**Given** rule (2), rule (5) and rule (6)
**When** the script needs the command list, the variable list and the target list
**Then** it reads the help texts that the recipe wrote through `$(EXEC)` to `target/reports/docs-check/` (`--service-help <dir>`) and starts no cargo command itself; rule (5) compares `docs/environment.md` with the union of: the variables in the help of `service serve`, `migrate` and `healthcheck`; the variables interpolated in the four compose files and the variables that their top-level `secrets:` read with `environment:`; the names in `.env` and `.env.test`; the `?=` variables of the `Makefile` and its two fragments; and the `[env]` keys of `.cargo/config.toml`; rule (6) accepts a target of `make/service.mk` that is documented in `docs/make-targets-service.md`

**Given** the skills
**When** rule (7) runs on the repository
**Then** all eleven skill directories have a `name` equal to the directory, a `description`, and an entry in `.claude/skills/README.md`

**Given** the run of the Bats command of the Verify line
**When** its TAP output is read
**Then** the plan line is not `1..0`, no test is skipped, and the file has exactly one line `# bats file_tags=repository`

### Story 9.10: Bats tests for the repository gates

As a platform maintainer,
I want Make-level negative tests for the repository lint, documentation-drift, architecture-model and invariants gates, and script-level tests for every rule kind of the model check,
So that each linter and each repository checker is shown to fail on a planted violation.

Track: E.
Depends on: 3.7, 9.8, 9.9.
Owns: `tests/CLI/bats/make_repository.bats`, `tests/CLI/bats/check_architecture_model.bats`, `tests/CLI/bats/fixtures/workflow_syntax_error.yml.txt`, `tests/CLI/bats/fixtures/workflow_unpinned_action.yml.txt`, `tests/CLI/bats/fixtures/duplicate_key.yml.txt`, `tests/CLI/bats/fixtures/skipped_heading.md.txt`, `tests/CLI/bats/fixtures/unquoted_expansion.sh.txt`, `tests/CLI/bats/fixtures/Dockerfile.latest.txt`, `tests/CLI/bats/fixtures/unformatted.toml.txt`, `tests/CLI/bats/fixtures/misspelt.md.txt`, `tests/CLI/bats/fixtures/unformatted.json.txt`, `tests/CLI/bats/fixtures/broken_relationship.dsl.txt`, `tests/CLI/bats/fixtures/architecture_model.dsl.txt`.
Touches: none.
Implements: FR43, FR67, FR120, NFR21, NFR25, G23, G26, G27, G31.
Reference: architecture-delivery.md "CLI Tests (Bats)" (rows `make_repository.bats` and `check_architecture_model.bats`; the fixture rules of the harness), DD-07, "Repository Lint and Hygiene" (the exclusion of fixtures from every linter), "Architecture Model" (decision table: what the gate validates).
Pitfalls: PF-44, PF-49.
Verify: `make bats BATS_PATH="tests/CLI/bats/make_repository.bats tests/CLI/bats/check_architecture_model.bats"`; `make lint-shell`; `git status --porcelain`.

**Acceptance Criteria:**

**Given** the negative tests for G23, one per linter
**When** they plant a workflow with a syntax error, a workflow with `uses: actions/checkout@v7`, a YAML file with a duplicate key, a Markdown file with a skipped heading level, a script with an unquoted expansion, `DOCKERFILES` pointing at the fixture with `latest`, an unformatted TOML file, a misspelt word and a misspelt file name, and an unformatted JSON file
**Then** `lint-repo` (or the matching `lint-*` target) exits non-zero each time with the finding of actionlint, zizmor, yamllint, markdownlint, shellcheck, hadolint, taplo, typos and Prettier respectively; the row has no passing-run test, because the passing run of `lint-repo` is the gate itself; a dry run `make -n lint-repo` prints the command of each of the nine tools, so no linter can be dropped from `lint-repo` unnoticed

**Given** the negative tests for G26, G27 and G31
**When** `make no-such-target` is added to `README.md`, a line that does not parse (`fixtures/broken_relationship.dsl.txt`) is appended to `workspace.dsl`, and `rust-toolchain.toml` is set to another version
**Then** `docs-check` exits non-zero with a rule (1) line, `validate-architecture-model` exits non-zero with the parser's error, and `validate-invariants` exits non-zero with a line for rule (c)

**Given** `check_architecture_model.bats`
**When** it runs
**Then** `scripts/validate-architecture-model.sh` passes on `fixtures/architecture_model.dsl.txt` (two components) with the matching crate list on standard input, and six variants derived with `sed` (no system-context view, no container view, no component view, one component removed, one extra component, one component excluded from the component view) each exit 1 and name the view kind or the component

**Given** the fixtures of the suite
**When** `make lint-yaml`, `make lint-markdown`, `make lint-toml`, `make lint-spelling`, `make lint-prettier` and `make lint-shell` are run on the committed tree
**Then** none of them reports a file under `tests/CLI/bats/fixtures/`: the extensions end in `.txt` and the directory is excluded where the listings say so

**Given** the run of the first command of the Verify line
**When** its TAP output is read
**Then** the plan line is not `1..0`, no test is skipped, each of the two files has exactly one line `# bats file_tags=repository`, and the output of `git status --porcelain` is the same before and after the run

### Story 9.11: Bats tests of the execution switch and the stack lifecycle

As a backend engineer,
I want Bats tests of the three execution places and of the development stack from start to removal,
So that the opt-in container mode, the forwarded variables and the healthy start of the stack cannot break unnoticed.

Track: E.
Depends on: 7.6, 9.4.
Owns: `tests/CLI/bats/make_execution_mode.bats`, `tests/CLI/bats/make_lifecycle.bats`.
Touches: none.
Implements: FR65, FR67, FR88, FR89, NFR33, NFR36, NFR38, G16.
Reference: architecture-delivery.md "CLI Tests (Bats)" (rows `make_execution_mode.bats` and `make_lifecycle.bats`), DD-01, DD-05, DD-23, "Containers and Compose" ("Compose files": the development override), "Command Surface" (lifecycle targets; variables `RUN_IN_CONTAINER`, `EXEC`, `EXEC_ENV`, `DEPS`, `BUILD_IMAGE`).
Pitfalls: PF-56, PF-57.
Verify: `make bats BATS_PATH="tests/CLI/bats/make_execution_mode.bats tests/CLI/bats/make_lifecycle.bats"`; `make lint-shell`; `git status --porcelain`.

**Acceptance Criteria:**

**Given** the two files, a running container engine and the development image
**When** the first command of the Verify line is run
**Then** it exits 0 and every test of the two table rows, in both columns, is present and passes

**Given** the execution-mode tests
**When** they run
**Then** `make -n format-check` is native; `RUN_IN_CONTAINER=1 make -n format-check` starts with `docker compose exec -T` and carries the five `-e` options; with `CI=true` or `IN_DEV_CONTAINER=1` it is native again; `RUN_IN_CONTAINER=1 make format-check`, `check-requirements`, `lint-rustdoc` and `setup-test-db` succeed in the running development container; inside the container `scripts/with-env.sh .env .env.test -- printenv APP_ENV DATABASE_URL` prints `test` and a URL on `database:5432/app_test`; `TEST_SEED=7` reaches a command behind `EXEC`, and without it the variable is unset there; the same holds for `DATABASE_STATEMENT_TIMEOUT=30m`; `RUN_IN_CONTAINER=1 scripts/doctor.sh` with cargo removed from `PATH` exits 0 and prints the `[SKIP]` line; an orchestration target ignores the switch
**And** `DOCKER=/nonexistent make lint-docker` exits 2 with `This target needs a reachable container engine`, and `RUN_IN_CONTAINER=1 make format-check` with the environment stopped fails with Docker's message that the service is not running

**Given** the lifecycle tests
**When** they run
**Then** `make start` returns with `GET /ping` answering `pong` and `GET /api/health` answering 204 after the `migrate` step succeeded; no mount of the `app` container lies below `/app`, and `make start` creates no new entry at the top of the checkout (the form of "no entry belongs to another user" that also holds when the suite runs as root); `make start-deps` on a running environment recreates no container; `timeout 5 make logs` exits 124 and shows `"message":"listening"`; `make new-logs` with a concurrent ping shows `request completed` and `/ping`; a second project (`PROJECT_NAME=bats-alt-<pid>-<random> DB_PORT=<free> LOCALSTACK_PORT=<free> make start-deps`) creates containers whose names start with that name and is removed with `docker compose -p <that name> down --volumes`; `make stop` and `make down` succeed
**And** `make start-deps` with `DB_PORT` set to an occupied port exits non-zero

**Given** the running development environment
**When** one test, tagged `slow`, appends a comment line with `plant_append` to a source file of the `service` crate
**Then** the `app` container rebuilds and logs a second `listening` entry without an image rebuild, and `teardown` restores the file (FR89; this test is an addition to the table row and is recorded as a deviation)

**Given** the run of the first command of the Verify line
**When** its TAP output is read
**Then** the plan line is not `1..0`, no test is skipped, each of the two files has exactly one line `# bats file_tags=surface`, and the output of `git status --porcelain` is the same before and after the run

## Epic 10: Hosted CI, Release and Maintenance Automation

A reviewer and a platform maintainer get every gate reported under its own name on each pull request, with caching, cancellation, timeouts, read-only tokens and actions pinned by commit SHA; a weekly clean-clone verification; automated releases; Dependabot for six ecosystems; and two template-sync flavours that do nothing in the template itself. The workflow files are added in this last epic; the full local gate run and the clean-clone proof follow them, and nothing is pushed before the last story is verified.

### Story 10.1: Quality and test workflows

As a reviewer,
I want the formatting, analysis, architecture, dependency, test, coverage, mutation and Bats gates to run on every pull request as separately named checks that call Make targets,
So that a failed check names one gate and I can reproduce it with the same command locally.

Track: E.
Depends on: 9.2, 9.6.
Owns: `.github/workflows/quality.yml`, `.github/workflows/tests.yml`, `scripts/ci/detect-changes.sh`, `scripts/ci/summarize-tests.sh`, `scripts/ci/mutation-plan.sh`.
Touches: none.
Implements: FR49, FR58, FR93, FR94, FR96, NFR8, NFR13, NFR14, NFR20, NFR39, NFR40, NFR41, G1, G2, G3, G4, G5, G6, G7, G8, G11, G12, G13, G14, G15, G16, G29, G30.
Reference: architecture-delivery.md "CI/CD Workflows" ("Action pins", "Conventions for every workflow" with the step blocks S1 to S7, S5d and D, the `changes` job and the Rust cache keys, "`quality.yml` — name `Quality`", "`tests.yml` — name `Tests`", "Secrets, variables and labels"), DD-08, DD-09, DD-13, DD-24, "Command Surface" (script rows `ci/detect-changes.sh`, `ci/summarize-tests.sh`, `ci/mutation-plan.sh`), "Delivery Validation" ("Behaviour on the bootstrap pull request").
Pitfalls: PF-06, PF-11, PF-59, PF-60, PF-61, PF-66.
Verify: `make lint-actions`; `make lint-yaml`; `make lint-prettier`; `make lint-shell`; `make lint-spelling`; `grep -En "uses: [^@]+@" .github/workflows/quality.yml .github/workflows/tests.yml | grep -Ev "@[0-9a-f]{40} # v"` (expected: no output).

**Acceptance Criteria:**

**Given** the two workflow files
**When** `make lint-actions` is run and the files are read
**Then** actionlint and zizmor exit 0; both workflows trigger on `pull_request` to `main` and `push` to `main`, declare top-level `permissions: contents: read`, cancel superseded pull-request runs through the concurrency group, run every job on `ubuntu-24.04` with a `timeout-minutes` of at most 45, pin every action by the 40-character SHA of "Action pins" with its version comment, and contain no `${{ ... }}` expression inside a `run:` script

**Given** `quality.yml`
**When** its jobs are compared with the job table
**Then** it has one job per gate with the check names of the table (G1, G2, G3, G4, G5, G29, G30, G6, G7, G8), each gate step is a `make <target>` call (in `crate-isolation` and `query-metadata` written to `target/<job>.log` through `tee` behind `set -o pipefail`, and followed by the count step of NFR40), tools are installed with `taiki-e/install-action` at the versions of `make/tool-versions.mk` and `fallback: none` (`sqlx-cli` and `clippy-sarif` in a second step with `fallback: cargo-binstall`), and each Rust cache key is saved by one job on `main` only
**And** the `static-analysis` job holds a read-only token, runs `make static-analysis-sarif` with `if: ${{ !cancelled() }}` and stores `target/sarif/clippy.sarif` as artifact `clippy-sarif`; the job `clippy-sarif-upload` (`contents: read`, `security-events: write`) downloads it and uploads it with category `clippy`, also when the gate failed, and skips with the notice of block D on fork and Dependabot pull requests, when `CODE_SCANNING_ENABLED` is `false`, and when it is unset in a repository that is not public

**Given** `tests.yml`
**When** its jobs are compared with the job table
**Then** the job `tests` runs `make tests-with-coverage doc-tests` once and `scripts/ci/summarize-tests.sh` writes one line each for G11, G12 and G14 to the job summary; it is the only job that computes coverage; the Codecov upload runs only when `CODECOV_TOKEN` is available and otherwise prints a notice (no secret is read in an `if:`); `e2e-tests` stores no artifact; the `bats-group` matrix has the six groups `surface` (image `dev`), `quality`, `tests`, `contracts` (`prod`), `load` (`prod`) and `repository` and runs `bats` with `BATS_ARGS="--filter-tags $GROUP"`; the aggregator `bats` is the required check

**Given** `scripts/ci/mutation-plan.sh`
**When** it runs with `EVENT_NAME` and `BASE_REF` for a pull request and for a push, in a temporary git repository that holds a small crate
**Then** it prints `count=<n>` and `shards=<JSON>` with one `{index, shard}` object per 150 mutants (`index` from 1 to n, `shard` the 0-based value `<index-1>/<n>` that `cargo mutants --shard` takes), at least 1 and at most 8 (`shards=[]` for 0 mutants; exit status 2 for a base that cannot be resolved); in `tests.yml` the shard job is skipped when `count` is `0`, has a 45-minute timeout and uploads `mutants-${{ matrix.index }}`, and the aggregator `mutation-tests` fails unless every shard succeeded or was skipped

**Given** `scripts/ci/detect-changes.sh` in a temporary git repository
**When** it is run with `BASE_SHA` and `HEAD_SHA` for a change that touches only `docs/x.md` and `README.md`, and for a change that also touches `crates/service/src/lib.rs`
**Then** it prints `code=false` and `code=true` respectively; every gate job of both workflows carries `needs: changes` and the `code` condition, the aggregators `mutation-tests` and `bats` use `if: ${{ always() }}`, and `clippy-sarif-upload` uses `!cancelled() && needs.changes.outputs.code == 'true'`

**Given** `scripts/ci/summarize-tests.sh` with a JUnit file that contains one failed integration test
**When** it is run with `GITHUB_STEP_SUMMARY` pointing at a temporary file
**Then** the file names G12 as failed and G11 as passed, and the script exits 0

### Story 10.2: Contract, change-control, load, security and repository workflows

As a reviewer,
I want the contract, approval-label, load, security and repository-hygiene gates as named checks, with the label-dependent checks evaluated again when a label or the title changes,
So that a breaking change or a protected-configuration change is blocked until a maintainer records the decision with a label.

Track: E.
Depends on: 9.2.
Owns: `.github/workflows/contracts.yml`, `.github/workflows/change-control.yml`, `.github/workflows/load-tests.yml`, `.github/workflows/security.yml`, `.github/workflows/repository.yml`.
Touches: none.
Implements: FR44, FR49, FR75, FR82, FR84, FR93, FR94, FR96, FR102, NFR8, NFR13, NFR14, NFR39, NFR40, G9, G10, G17, G18, G19, G20, G21, G22, G23, G24, G25, G26, G27, G28, G31.
Reference: architecture-delivery.md "CI/CD Workflows" ("`contracts.yml` — name `Contracts`", "`change-control.yml` — name `Change control`", "`load-tests.yml` — name `Load tests`", "`security.yml` — name `Security`", "`repository.yml` — name `Repository`", "Conventions for every workflow" with the step blocks S1 to S7 and D, "Action pins"), DD-08, DD-10, DD-13, DD-15, DD-16, DD-24.
Pitfalls: PF-59, PF-60, PF-61.
Verify: `make lint-actions`; `make lint-yaml`; `make lint-prettier`; `make lint-spelling`; `grep -En "uses: [^@]+@" .github/workflows/*.yml | grep -Ev "@[0-9a-f]{40} # v"` (expected: no output); `grep -L "^permissions:" .github/workflows/*.yml` (expected: no output).

**Acceptance Criteria:**

**Given** the five workflow files
**When** `make lint-actions` is run and the files are compared with their job tables
**Then** actionlint and zizmor exit 0 and every job has the id, check name, Make target, step blocks and timeout of its table, under "Conventions for every workflow"

**Given** `change-control.yml`
**When** its triggers and jobs are read
**Then** it runs on `pull_request` types `opened`, `edited`, `reopened`, `synchronize`, `labeled` and `unlabeled` and on `push` to `main`; the job `title` runs for pull requests only, passes the title through `env: COMMIT_TITLE` to `make check-commit-message` and is skipped for pull requests authored by `dependabot[bot]` and for those labelled `template_sync`; on a pull request the three comparing jobs check out with full history, use `origin/<base branch>` as `base_ref` and set `CONFIG_CHANGE_APPROVED` or `BREAKING_CHANGE_APPROVED` to `true` only when the matching label (`approved-config-change`, `approved-breaking-change`) is present; on a push they use `github.event.before` as base and set the variable to `true`, printing modifications as approved, because the merged pull request was the approval point

**Given** the documentation-only rule
**When** the `if:` conditions of all gate jobs are read
**Then** the jobs of G9, G23, G25, G26, G28 and G31 (`secrets-scan`, `lint-repo`, `validate-configuration`, `docs-check`, `title`, `invariants`) are not gated on `needs.changes.outputs.code`, and every other gate job is

**Given** `contracts.yml`, `load-tests.yml` and `security.yml`
**When** they are read
**Then** the jobs that test the production image build it once with the image cache (`cache-from: type=gha`) and call their target with `BUILD_IMAGE=0`; `image-scan` has no cache step for the Trivy database; `smoke-load-tests` uploads `tests/Load/results/` as artifact `load-test-results` with `if: ${{ !cancelled() }}`; `codeql` analyses `rust` (`build-mode: none`) and `actions` with the job permissions `contents: read`, `actions: read` and `security-events: write` and follows block D: skipped with a notice on fork and Dependabot pull requests, when `CODE_SCANNING_ENABLED` is `false`, and when it is unset in a repository that is not public

**Given** the five workflow files
**When** their token permissions and push steps are searched
**Then** no workflow uses `pull_request_target`, no step pushes to a branch, write permissions exist only as `security-events: write` in `codeql` (and in `clippy-sarif-upload` of Story 10.1), every job-level `permissions` block restates `contents: read`, and every checkout sets `persist-credentials: false`

### Story 10.3: Scheduled verification, release, template sync and Dependabot

As a platform maintainer,
I want a weekly run that proves the clean-clone path, a release on every releasable push to `main`, grouped dependency updates and two template-sync workflows,
So that the template maintains itself and delivers fixes to downstream services without failing when organisation secrets are absent.

Track: E.
Depends on: 3.6, 9.2.
Owns: `.github/workflows/scheduled.yml`, `.github/workflows/release.yml`, `.github/workflows/template-sync-app.yml`, `.github/workflows/template-sync-pat.yml`, `.github/dependabot.yml`, `.templatesyncignore`.
Touches: none.
Implements: FR51, FR95, FR96, FR98, FR99, FR100, FR118, NFR7, NFR8, NFR13, NFR14, NFR34, NFR38, NFR42.
Reference: architecture-delivery.md "CI/CD Workflows" ("`scheduled.yml` — name `Scheduled verification`", "`release.yml` — name `Release`", "Template sync" with the `.templatesyncignore` listing and the table of template-owned files a service extends, "Dependabot, review bot and templates", "Secrets, variables and labels", "Action pins"), DD-11, DD-12, DD-13, DD-14, DD-23.
Pitfalls: PF-05, PF-62, PF-64, PF-65, PF-66.
Verify: `make lint-actions`; `make lint-yaml`; `make lint-prettier`; `make lint-spelling`; `grep -c "rust-service-template" .github/workflows/template-sync-app.yml .github/workflows/template-sync-pat.yml` (expected `1` per file); `grep -l "rust-service-template" .github/workflows/scheduled.yml .github/workflows/release.yml .github/dependabot.yml` (expected: no output).

**Acceptance Criteria:**

**Given** `scheduled.yml`
**When** it is read
**Then** it runs on cron `23 4 * * 1` and on `workflow_dispatch`, never cancels, and has the jobs `clean-bootstrap-ci` (clean checkout, no restored cache, `make bootstrap`, then `make stop` and a timed `make start` that fails above 60 s, then `make ci`, then the step `Require a clean working tree`, which fails when `git status --porcelain` is not empty (FR68), timeout 75), `mutation-plan`, `mutation-shard` and `mutation-tests` (the full run, shards sized by `scripts/ci/mutation-plan.sh`), `security-gates` (G7, G9, G10), `code-scanning` and `devcontainer-build` (`@devcontainers/cli` at `DEVCONTAINER_CLI_VERSION`: the build, the start with `postCreateCommand`, and `docker info`, `make format-check` and `make start-deps` inside the container, FR118); workflow `env` sets `TEST_SEED` and `PROPTEST_RNG_SEED` to `${{ github.run_id }}` and `CARGO_INCREMENTAL` and `CARGO_PROFILE_DEV_DEBUG` to `'0'` and the jobs that run tests print both seeds in their summary; every job ends with a step that writes the failing target and its last output lines to the job summary on failure

**Given** `release.yml`
**When** it is read
**Then** it triggers on `push` to `main` and on `workflow_dispatch` with the optional input `version`, uses the concurrency group `release` without cancellation, runs its job when the event is `workflow_dispatch` or the head commit message does not contain `[skip ci]`, maps `VILNACRM_APP_ID` and `VILNACRM_APP_PRIVATE_KEY` to environment variables in its first step and, when either is empty, writes the notice `Release skipped: GitHub App secrets are not configured` and succeeds; with the secrets it creates the App token, checks out with full history, installs cocogitto 7.0.0 through its pinned action, sets the bot identity and runs `scripts/release.sh`, or `scripts/release.sh --version <x.y.z>` when the input is given

**Given** the two sync workflows
**When** they are read
**Then** `template-sync-app.yml` runs on `0 9 * * MON` and `template-sync-pat.yml` on `0 0 1 * *`, both also manually; the first step of each ends the job with `Template sync skipped: this is the template repository` when `$GITHUB_REPOSITORY` equals the workflow-level `TEMPLATE_REPOSITORY`, which is the only occurrence of the repository name in the file; each skips with a notice when its credentials are absent and the PAT flavour also skips when both App secrets exist; both pass `pr_labels: template_sync`, `pr_title: 'chore(template): merge template changes'`, `is_force_push_pr: true`, `is_pr_cleanup: true` and `is_keep_branch_on_pr_cleanup: true`

**Given** `.templatesyncignore`
**When** it is compared with the listing
**Then** it is equal: the service-owned block (`.templatesyncignore`, README, changelog, workspace manifest and lock file, `.sqlx/`, migrations, both snapshots, `workspace.dsl`, `bmalph/config.json`, `specs/`, `crates/company-subdomain/`, the four feature files other than the health feature, `tests/Load/config.json.dist`, `docs/requests.http`) and the block of template-owned files a service extends (`.env`, `.env.test`, `docs/adr/`, `docs/environment.md`, `.github/dependabot.yml`, the skill index, the policy and exception files, the health-check and `service` wiring files, the five `service` tests that build `Adapters` or name the example context (`src/http/router/tests.rs`, `tests/integration/bootstrap.rs`, `migrate.rs`, `serve.rs` and `panic.rs`), `tests/support/`, `tests/e2e/`)

**Given** `.github/dependabot.yml`
**When** it is read
**Then** it configures the six ecosystems `cargo`, `rust-toolchain`, `github-actions`, `docker`, `docker-compose` and `npm` in `/tests/Load`, each at least monthly and each with a group, with the commit prefixes `build`, `chore`, `ci`, `build`, `build` and `chore` (none of which produces a release), LocalStack excluded from the compose updates, and no `reviewers` key

**Given** all four workflows
**When** `make lint-actions` is run and their permissions are read
**Then** actionlint and zizmor exit 0 (the checkouts that keep credentials carry a reasoned `zizmor: ignore` comment), top-level permissions are `contents: read`, and all writes go through the App token or the PAT

### Story 10.4: Full local gate run with the workflows in place

As a platform maintainer,
I want every stage of `make ci` to pass on the working tree once the workflow files and all eighteen Bats files exist,
So that the clean-clone proof starts from a green tree.

Track: E.
Depends on: 9.1, 9.3, 9.5, 9.6, 9.7, 9.10, 9.11, 10.1, 10.2, 10.3.
Owns: none.
Touches: any file whose gate still fails on the complete tree (each fix is listed in the completion notes with the story that owns the file).
Implements: FR66, FR67, FR68, NFR21, NFR22, NFR23, NFR35, NFR41, G16, G23, G31.
Reference: architecture-delivery.md DD-02, DD-07, "Command Surface" (targets `ci`, `ci-list`, `validate-invariants`, `lint-repo`, `bats`; script row `check-invariants.sh` with the rule table), "CLI Tests (Bats)" (the table and the time paragraph), "Repository Lint and Hygiene" (`.gitignore` intent), "Delivery Validation" ("Estimate for `make ci`").
Pitfalls: PF-11, PF-44, PF-49.
Verify: `make validate-invariants`; `make ci base_ref=HEAD`; `git status --porcelain`.

**Acceptance Criteria:**

**Given** the repository with all eleven workflow files and all eighteen Bats files
**When** `make validate-invariants` is run
**Then** it exits 0 with all eight rules: every action is pinned by SHA, every workflow declares `permissions`, `concurrency` and job timeouts, every `tool:` version equals `make/tool-versions.mk`, every gate target of `make ci-list` is run by a workflow with a `pull_request` trigger, and every workflow runs only gate targets of `make ci-list` and the documented CI-only steps, every Bats file carries exactly one tag of the six groups, and every job name of a pull-request workflow is in `docs/repository-settings.md`

**Given** the committed tree
**When** `make ci base_ref=HEAD` is run
**Then** it exits 0 and its last line is `CI checks successfully passed!`; with `base_ref=HEAD` the configuration guard and both diff gates compare the commit with itself and the changed-code mutation stage sees an empty diff (the full mutation run is Story 9.3); a failing target is fixed at its cause, never in a protected item, and the command is repeated until one uninterrupted run passes

**Given** the Bats stage of that run
**When** its output is read
**Then** every file of the six tag groups (`surface`, `quality`, `tests`, `contracts`, `load`, `repository`) passes, no test is skipped except the bmalph tests when npm is absent, tests longer than a minute carry the tag `slow`, and every gate G1 to G27 and G29 to G31 has at least one negative test in the suite (G28 has matching and non-matching titles)
**And** the duration of each stage of `make ci` and of each of the six Bats groups is stated in the completion notes; a group above 20 minutes is reported

**Given** the run completed
**When** `git status --porcelain` and a listing of the top of the checkout are compared with their state before the run
**Then** both are unchanged: every command wrote only to git-ignored paths, and no new entry exists at the top of the checkout

### Story 10.5: Clean-clone verification of the complete pipeline

As a backend engineer,
I want proof that an unmodified clone reaches a green `make ci` after `make bootstrap` with no manual step, with the workflow files in place,
So that the release commit and every later pull request start from a verified baseline.

Track: E.
Depends on: 10.4.
Owns: none.
Touches: `docs/operations.md` (measured durations of `make bootstrap`, `make ci`, `make start` and the mutation run), `docs/load-testing.md` (the first local p99 of the `health` smoke profile with its date), `docs/repository-settings.md` (the job names, checked against the workflow files); any file whose gate still fails in the clone, with the fix listed in the completion notes.
Implements: FR66, FR68, FR70, FR93, FR125, NFR1, NFR13, NFR14, NFR22, NFR27, NFR34, NFR35, NFR39, G31.
Reference: architecture-delivery.md DD-02, DD-08, DD-20 (building behind a TLS-intercepting proxy), "Command Surface" (targets `bootstrap`, `ci`, `ci-list`; script row `check-invariants.sh` rule (d)), "CI/CD Workflows" (all job tables: check names), "Delivery Validation" ("Behaviour on the bootstrap pull request", "Estimate for `make ci`", "Risks"); scope-decisions.md section 8.11 (every pull-request workflow must pass on the bootstrap pull request); PRD "Success Criteria" (K-01, K-02, K-04).
Pitfalls: PF-11, PF-60.
Verify: `test -z "$(git status --porcelain)" && make down && d=$(mktemp -d) && git clone --no-local . "$d/service-clone" && cd "$d/service-clone" && git update-ref refs/remotes/origin/main HEAD && make bootstrap && make ci && test -z "$(git status --porcelain)"`; `make docs-check`; `make lint-repo`; `for f in README research capability-catalog product-brief prd architecture architecture-delivery epics implementation-readiness parity-matrix run-summary; do test -s specs/rust-service-template-bootstrap/$f.md || echo "missing $f"; done` (expected: no output); `for f in prd architecture architecture-delivery epics product-brief research; do cmp _bmad-output/planning-artifacts/$f.md specs/rust-service-template-bootstrap/$f.md; done`.

**Acceptance Criteria:**

**Given** a committed tree, the development services of the working checkout removed with `make down`, and a fresh clone in a temporary directory with a different directory name and no build cache, in which `origin/main` is set to the cloned commit (the state of the release commit on `main`, so that the base-comparing gates compare the commit with itself)
**When** `make bootstrap` and then `make ci` are run there with no manual step in between
**Then** `make bootstrap` ends with the stack healthy and prints the entry-point URLs, `make ci` exits 0 and its last line is `CI checks successfully passed!`, and `git status --porcelain` in the clone is empty
**And** on the implementation machine the two images for the clone's project name are built with the named build context first and the run uses `BUILD_IMAGE=0`; this is environment preparation, not a manual step of the repository

**Given** that run
**When** its durations are compared with the budgets
**Then** `make bootstrap` reached `GET /ping` = 200 within 15 minutes and `make ci` completed within 45 minutes on a 4-vCPU machine, or the measured value and the reason for a revised budget are recorded; `make start` with images and build cache present returns within 60 s; the three measurements, the numbers of the mutation run of Story 9.3 and the local p99 of the `health` smoke profile are written to `docs/operations.md` and `docs/load-testing.md` with the date and machine, and the committed budgets are unchanged

**Given** the bootstrap pull-request situation (the base branch holds none of the files)
**When** `make validate-configuration`, `make openapi-diff` and `make graphql-diff` are run with `base_ref=$(git rev-list --max-parents=0 HEAD)`
**Then** the guard passes without the approval label because no protected item exists on the base, and both diff gates pass with the first-version notice
**And** `scripts/with-env.sh .env .env.test -- scripts/mutation-diff.sh "$(git rev-list --max-parents=0 HEAD)" --list -- env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --workspace --cargo-arg=--locked` prints the same number as `cargo mutants --workspace --list | wc -l`: the changed-code mode selects every mutant when every file is new, and the mutants are not executed again

**Given** `docs/repository-settings.md`
**When** it is compared with the job names in the pull-request workflows under `.github/workflows/`
**Then** it names every job (G31 rule (d)); it marks the gate jobs, the seven `changes` jobs under their own check names (`Changes (Quality)`, `Changes (Tests)`, `Changes (Contracts)`, `Changes (Change control)`, `Changes (Load tests)`, `Changes (Security)`, `Changes (Repository)`) and the aggregators `mutation-tests` and `bats` as required status checks, and `mutation-plan`, `mutation-shard`, `bats-group`, `clippy-sarif-upload` and `codeql` as not required

**Given** the complete tree
**When** `make docs-check`, `make lint-repo`, the two loops of the Verify line and `grep -rn "rust-service-template" Makefile make scripts Dockerfile docker-compose*.yml tests/CLI tests/Load AGENTS.md CLAUDE.md .claude/skills` are run
**Then** the two targets exit 0, the first loop and the grep print nothing, and every `cmp` exits 0: the tracked planning bundle equals the final planning artifacts

## Implementation Order and Parallel Tracks

This section is for the orchestrator. Apart from the authored texts of the rules, the exclusive resources and the order inside the waves, it is computed from the `Depends on`, `Owns` and `Touches` lines of the stories above; the build script checks the authored texts against the computed waves.

Tracks: A = service code (shared kernel, health check, example context, composition root, end-to-end features, contract snapshots); B = `xtask` (architecture gate, configuration guard, scaffolder); C = delivery tooling that does not need finished service code; D = guidance and documentation; E = integration that needs everything before it.

### All stories

| Story | Title | Track | Depends on | Wave | Exclusive resources |
| --- | --- | --- | --- | --- | --- |
| 1.1 | Workspace skeleton and Rust tool configuration | A | none | 1 | none |
| 1.2 | Makefile command surface, execution switch and host preflight | C | 1.1 | 2 | none |
| 1.3 | Pinned tool and Bats installation | C | 1.1 | 2 | none |
| 1.4 | Repository lint and format tooling | C | 1.2, 1.3 | 3 | none |
| 2.1 | Architecture gate with layer, coverage and lint-inheritance rules | B | 1.2, 1.3 | 3 | none |
| 2.2 | Context-isolation, third-party allow-list and path rules | B | 2.1 | 4 | none |
| 2.3 | Protected-configuration guard | B | 2.2 | 5 | none |
| 2.4 | Bounded-context scaffolding command | B | 2.3 | 6 | none |
| 3.1 | Container images, compose stacks and dev container | C | 1.4 | 4 | `rust-workspace`, `docker-build` |
| 3.2 | Contract lint and breaking-change checks | C | 1.4 | 4 | none |
| 3.3 | Secrets scan and production-image scan | C | 3.1 | 5 | `rust-workspace`, `docker-build` |
| 3.4 | K6 load-test kit | C | 1.4 | 4 | none |
| 3.5 | AWS load-test runner with LocalStack dry run | C | 3.1, 3.4 | 5 | `localstack-aws` |
| 3.6 | Commit convention, git hooks and release tooling | C | 1.4 | 4 | none |
| 3.7 | Architecture model and its validation gate | C | 1.4 | 4 | none |
| 4.1 | Shared domain kernel | A | 1.2, 1.3 | 3 | `rust-workspace` |
| 4.2 | Seeded test data and kernel test doubles | A | 4.1 | 4 | `rust-workspace` |
| 4.3 | Problem documents, problem-aware extractors and request budget | A | 2.2, 4.2 | 5 | `rust-workspace` |
| 4.4 | In-process buses, cache, clock and UUID generator | A | 4.1 | 4 | `rust-workspace` |
| 4.5 | PostgreSQL pool, reversible migrations, SQS client and test database | A | 2.2, 3.1, 4.4 | 5 | `rust-workspace`, `test-db` |
| 5.1 | Health orchestration and the health endpoint handler | A | 4.2, 4.3 | 6 | `rust-workspace` |
| 5.2 | PostgreSQL, cache and SQS probes | A | 4.5, 5.1 | 7 | `rust-workspace`, `test-db` |
| 6.1 | ExampleApiResource domain model | A | 2.2, 4.2 | 5 | `rust-workspace` |
| 6.2 | Create command, read and list queries, audit subscriber | A | 4.3, 6.1 | 6 | `rust-workspace` |
| 6.3 | REST handlers and GraphQL resolvers for the example resource | A | 6.2 | 7 | `rust-workspace` |
| 6.4 | PostgreSQL repository with committed query metadata | A | 4.5, 6.1 | 6 | `rust-workspace`, `test-db` |
| 7.1 | Command line, typed configuration and `check-config` | A | 4.5 | 6 | `rust-workspace` |
| 7.2 | `migrate` command with bounded database wait | A | 7.1 | 7 | `rust-workspace`, `test-db`, `compose` |
| 7.3 | Liveness route, request ids, security headers and access log | A | 4.2, 7.1 | 7 | `rust-workspace`, `test-db` |
| 7.4 | OpenAPI document, docs page, GraphQL schema and committed contract snapshots | A | 3.2, 5.1, 6.3, 7.2, 7.3 | 8 | `rust-workspace`, `test-db` |
| 7.5 | Composition root and assembled router | A | 5.2, 6.4, 7.4 | 9 | `rust-workspace`, `test-db` |
| 7.6 | `serve` and `healthcheck` commands with JSON logs and graceful shutdown | A | 7.5 | 10 | `rust-workspace`, `test-db` |
| 7.7 | Gherkin end-to-end suite with the reused PHP health feature | A | 7.5 | 10 | `rust-workspace`, `test-db`, `e2e` |
| 8.1 | Assistant entry files, settings and protected-file guard | D | 1.4, 2.4 | 7 | none |
| 8.2 | Skill index, guides and ten capability skills | D | 1.4 | 4 | none |
| 8.3 | BMAD and bmalph integration | D | 1.4 | 4 | none |
| 8.4 | README, contribution and security policies, issue and pull-request templates | D | 1.4 | 4 | none |
| 8.5 | Getting-started and reference documents with the request collection | D | 1.4 | 4 | none |
| 8.6 | Architecture overview and decision records | D | 1.4 | 4 | none |
| 8.7 | Operations, load-testing, extension and repository-settings documents with the setup guides | D | 1.4 | 4 | none |
| 8.8 | Tracked planning bundle and parity matrix | D | 1.4 | 4 | none |
| 9.1 | Isolated production-image stack with Schemathesis and smoke load | E | 3.1, 3.4, 7.6 | 11 | `rust-workspace`, `compose`, `load-stack`, `docker-build` |
| 9.2 | Coverage at 100% lines and functions | E | 2.4, 7.6, 7.7 | 11 | `rust-workspace`, `test-db`, `long-run` |
| 9.3 | Mutation testing with zero surviving mutants | E | 9.2 | 12 | `rust-workspace`, `test-db`, `long-run` |
| 9.4 | Bats harness with general, hook and bmalph tests | E | 3.6, 8.1, 8.3 | 8 | `alone` |
| 9.5 | Negative Bats tests for the static gates | E | 2.4, 7.6, 7.7, 9.4 | 11 | `alone` |
| 9.6 | Changed-code mutation script and Bats tests for the test levels, database targets and scaffolding | E | 2.4, 7.6, 7.7, 9.4 | 11 | `alone`, `test-db`, `e2e`, `compose` |
| 9.7 | Bats tests for contract, load and security targets | E | 3.2, 3.3, 3.5, 9.1, 9.4 | 12 | `alone`, `compose`, `load-stack`, `localstack-aws`, `docker-build` |
| 9.8 | Repository-invariants gate | E | 9.4 | 9 | none |
| 9.9 | Documentation-drift gate | E | 7.6, 8.1, 8.2, 8.3, 8.4, 8.5, 8.6, 8.7, 8.8, 9.4 | 11 | none |
| 9.10 | Bats tests for the repository gates | E | 3.7, 9.8, 9.9 | 12 | `alone` |
| 9.11 | Bats tests of the execution switch and the stack lifecycle | E | 7.6, 9.4 | 11 | `alone`, `test-db`, `compose`, `docker-build` |
| 10.1 | Quality and test workflows | E | 9.2, 9.6 | 12 | none |
| 10.2 | Contract, change-control, load, security and repository workflows | E | 9.2 | 12 | none |
| 10.3 | Scheduled verification, release, template sync and Dependabot | E | 3.6, 9.2 | 12 | none |
| 10.4 | Full local gate run with the workflows in place | E | 9.1, 9.3, 9.5, 9.6, 9.7, 9.10, 9.11, 10.1, 10.2, 10.3 | 13 | `alone`, `test-db`, `e2e`, `compose`, `load-stack`, `localstack-aws`, `docker-build`, `long-run` |
| 10.5 | Clean-clone verification of the complete pipeline | E | 10.4 | 14 | `alone`, `test-db`, `e2e`, `compose`, `load-stack`, `localstack-aws`, `docker-build`, `long-run` |

### Waves

A wave holds the stories whose dependencies are all in earlier waves. A wave is implemented and then verified as a whole; the next wave starts when every story of the wave is verified. Stories of one wave overlap only as far as the table of exclusive resources and the order inside the waves allow.

- Wave 1: 1.1 (A)
- Wave 2: 1.2 (C); 1.3 (C)
- Wave 3: 1.4 (C); 2.1 (B); 4.1 (A)
- Wave 4: 2.2 (B); 3.1 (C); 3.2 (C); 3.4 (C); 3.6 (C); 3.7 (C); 4.2 (A); 4.4 (A); 8.2 (D); 8.3 (D); 8.4 (D); 8.5 (D); 8.6 (D); 8.7 (D); 8.8 (D)
- Wave 5: 2.3 (B); 3.3 (C); 3.5 (C); 4.3 (A); 4.5 (A); 6.1 (A)
- Wave 6: 2.4 (B); 5.1 (A); 6.2 (A); 6.4 (A); 7.1 (A)
- Wave 7: 5.2 (A); 6.3 (A); 7.2 (A); 7.3 (A); 8.1 (D)
- Wave 8: 7.4 (A); 9.4 (E)
- Wave 9: 7.5 (A); 9.8 (E)
- Wave 10: 7.6 (A); 7.7 (A)
- Wave 11: 9.1 (E); 9.2 (E); 9.5 (E); 9.6 (E); 9.9 (E); 9.11 (E)
- Wave 12: 9.3 (E); 9.7 (E); 9.10 (E); 10.1 (E); 10.2 (E); 10.3 (E)
- Wave 13: 10.4 (E)
- Wave 14: 10.5 (E)

Critical path (14 stories, one per wave): 1.1, 1.2, 4.1, 4.2, 4.3, 6.2, 6.3, 7.4, 7.5, 7.6, 9.2, 9.3, 10.4, 10.5. Track A is the spine: its stories run one at a time, and Epics 2, 3 and 8 and the checker stories run beside it where the exclusive resources allow.

### Rules for concurrent execution

- One git worktree per story, on its own branch, created from the integration branch when every dependency of the story has been handed over. A worktree has its own working tree and its own git index, so a story sees only finished stories and every gate target can be run as written. The worktrees share one build directory (each `target/` is a link to it), one compose project with one set of backing services, and the Docker daemon. These rules are written for that model; they replace an earlier single-checkout model with verification at a wave barrier, and the two must not be mixed.
- A story is verified in its worktree right after it is implemented: an independent verifier runs the `Verify` line and every acceptance criterion on the story's commit. The verifier hands the story over: it rebases the story branch onto the integration branch, checks that the workspace still builds, fast-forwards the integration branch and ticks the story in the Ralph fix plan. A story that fails goes back to an implementer with the findings.
- A failure in a file the story neither owns nor touches is a defect of a finished story: report it, never edit that file.
- Two stories never write the same file at the same time: where two stories edit one file (a crate's `lib.rs`, the `xtask` command line, a shared test `main.rs`), a dependency chain orders them, so the later story starts from a tree that already holds the earlier edit. The table at the end lists every such file with the stories in order.
- cargo serialises builds in the shared build directory with a lock: concurrent cargo commands wait, they do not fail. The executables `service` and `xtask` are written to one path of that directory, so two stories whose tests start the same executable never overlap (keys `service-binary` and `xtask-binary`).
- No story after 1.1 changes a `Cargo.toml` or `Cargo.lock`, except Story 1.4 (whitespace through `taplo fmt`) and the Bats plants of Stories 9.5 and 9.6, which restore them. A dependency or feature the architecture missed is a deviation.
- Two kinds of command share one directory of the build directory each and are serialised by a lock, as commands, not as stories: every `cargo llvm-cov` command (one profile directory) and every `cargo mutants` command.
- `core.hooksPath` is shared by all worktrees, so no story runs `make install`, `make install-hooks` or `make bootstrap` in a worktree. A criterion that needs installed hooks, or a clean clone, uses a temporary clone of the story's commit.
- The untracked planning directory `_bmad-output/` of the integration checkout is linked into every worktree, so a story that reads it finds it.
- The table "Exclusive resources", the commit protocol and the deviation rule of "How to read a story" apply.
- No story of Epics 1 to 9 creates a file under `.github/workflows/`. Handed-over stories are pushed as they land until Story 10.1; from then on nothing is pushed until Story 10.5 is verified. The first push after Story 10.5 runs every workflow, whose gates Stories 10.4 and 10.5 have run locally.

### Exclusive resources

Two stories that share a key never overlap, unless the rule of the key says otherwise. The keys of each story are also in the table "All stories"; the keys `service-binary` and `xtask-binary` are derived from the stories listed here.

| Key | Resource and reason | Stories | Rule |
| --- | --- | --- | --- |
| `alone` | Every shared resource at once. The story runs every gate, runs `make purge`, or plants and restores files and then runs gates that build, test, start containers and compare with the base. | 9.4, 9.5, 9.6, 9.7, 9.10, 9.11, 10.4, 10.5 | No story that holds any key of this table is active beside it. Stories without a key (documentation, workflow files) may run. |
| `rust-workspace` | Compilation of the service workspace on the 4-vCPU machine. | 3.1, 3.3, 4.1, 4.2, 4.3, 4.4, 4.5, 5.1, 5.2, 6.1, 6.2, 6.3, 6.4, 7.1, 7.2, 7.3, 7.4, 7.5, 7.6, 7.7, 9.1, 9.2, 9.3 | At most two at a time, and only when their dependencies allow it. Track B (`xtask`) runs beside them. |
| `service-binary` | The executable `service` in the shared build directory: a test that starts it must not run while another story rebuilds it. | 7.1, 7.2, 7.3, 7.4, 7.5, 7.6, 7.7, 9.1, 9.2, 9.3 | One at a time. |
| `xtask-binary` | The executable `xtask` in the shared build directory. | 2.1, 2.2, 2.3, 2.4, 9.2, 9.3 and the `alone` stories | One at a time. |
| `test-db` | The PostgreSQL server of the development compose project and its test database: `make setup-test-db`, a prerequisite of every test target, force-drops `app_test`, which holds the bookkeeping of every running `#[sqlx::test]`. | 4.5, 5.2, 6.4, 7.2, 7.3, 7.4, 7.5, 7.6, 7.7, 9.2, 9.3, 9.6, 9.11, 10.4, 10.5 | One at a time. |
| `e2e` | The end-to-end runner, which drops every database named `e2e_*` on the server at start-up. | 7.7, 9.6, 10.4, 10.5 | Two end-to-end runs never share one database server at the same time. |
| `compose` | The development compose project, its default host ports (5432, 4566, 8081, 8080) and the rows of the development database `app`: `migrate-rollback` (7.2, 9.6), `start`, `stop` and `down` (9.11, 10.5); Stories 9.1 and 9.7 assert that the development containers and rows are unchanged. | 7.2, 9.1, 9.6, 9.7, 9.11, 10.4, 10.5 | One at a time. A story that tests `start`, `stop` or `down` uses a private project name and private ports unless it runs alone. Story 10.5 runs `make down` first, because its clone starts a second project on the same ports. |
| `load-stack` | The isolated project `<project>-load` and host port 18081. | 9.1, 9.7, 10.4, 10.5 | One at a time. |
| `localstack-aws` | The EC2, S3 and IAM state of LocalStack that the AWS dry run creates and removes. | 3.5, 9.7, 10.4, 10.5 | One at a time. |
| `docker-build` | The image tags `<project>-app:prod` and `<project>-app:dev` and Docker builds. | 3.1, 3.3, 9.1, 9.7, 9.11, 10.4, 10.5 | One build at a time. |
| `long-run` | Long whole-workspace runs with timing-sensitive tests on the 4-vCPU machine (coverage, the eight mutation shards, `make ci`). | 9.2, 9.3, 10.4, 10.5 | Only stories without a key run beside 9.2 or 9.3: 9.9 and 10.1 to 10.3. |

The waves above are the earliest point at which a story can start. Inside a wave the orchestrator starts every story whose dependencies are handed over and whose keys are free; Story 1.4 formats the TOML files of Story 1.1 with `taplo fmt` as its first step, and Story 3.1 or 3.3 builds its images under the `docker-build` key.

### Files touched by more than one story

The stories of each row are in wave order. The first story creates the file, except `README.md` and `CLAUDE.md`, which exist before the first story and are owned by the Epic 8 story that rewrites them.

| File | Stories in wave order | Why the other stories touch it |
| --- | --- | --- |
| `.cargo/mutants.toml` | 1.1, 9.3 | only to record an equivalent mutant in the exclusion list, one entry with a comment that explains the equivalence |
| `crates/company-subdomain/some-module/application/src/lib.rs` | 1.1, 6.2, 6.3 | module declarations |
| `crates/company-subdomain/some-module/domain/src/lib.rs` | 1.1, 6.1 | module declarations |
| `crates/company-subdomain/some-module/infrastructure/src/lib.rs` | 1.1, 6.4 | module declaration |
| `crates/internal/health-check/application/src/lib.rs` | 1.1, 5.1 | module declarations |
| `crates/internal/health-check/domain/src/lib.rs` | 1.1, 5.1 | module declarations |
| `crates/internal/health-check/infrastructure/src/lib.rs` | 1.1, 5.2 | module declaration |
| `crates/service/src/lib.rs` | 1.1, 7.1, 7.2, 7.4, 7.5, 7.6 | replaces the stub; dispatch arm; module declarations and the two export arms; module declarations; the last two dispatch arms and module declaration |
| `crates/shared/application/src/lib.rs` | 1.1, 4.3 | module declarations |
| `crates/shared/domain/src/lib.rs` | 1.1, 4.1 | module declarations and re-exports |
| `crates/shared/infrastructure/src/lib.rs` | 1.1, 4.4, 4.5 | module declarations |
| `tests/e2e/main.rs` | 1.1, 7.7 | replaces the stub with the cucumber runner |
| `tests/support/src/lib.rs` | 1.1, 4.2, 6.1, 5.1 | module declarations for the six files; module declarations |
| `xtask/src/main.rs` | 1.1, 2.1 | replaces the stub with the call to `cli::main` |
| `CLAUDE.md` | 1.4, 8.1 | the three table separator rows of the section "BMAD-METHOD Integration" get three dashes per column, the form markdownlint accepts |
| `README.md` | 1.4, 8.4 | final newline |
| `xtask/src/arch/load.rs` | 2.1, 2.2 | third-party and transitive facts |
| `xtask/src/arch/rules.rs` | 2.1, 2.2 | four more rules |
| `xtask/src/arch/rules/tests.rs` | 2.1, 2.2 | four more rules |
| `xtask/src/cli.rs` | 2.1, 2.3, 2.4 | subcommand `config-guard` with `--base`, `--list` and the approval variable; subcommand `new-context` |
| `xtask/src/cli/tests.rs` | 2.1, 2.3, 2.4 | subcommand `config-guard` with `--base`, `--list` and the approval variable; subcommand `new-context` |
| `xtask/tests/fixtures/arch/workspace/` | 2.1, 2.2 | four more planted violations |
| `xtask/tests/integration/arch.rs` | 2.1, 2.2 | assertions for the new rules |
| `xtask/tests/integration/main.rs` | 2.1, 2.3, 2.4 | module declaration |
| `xtask/tests/integration/support.rs` | 2.1, 2.3 | temporary git repositories |
| `crates/shared/infrastructure/tests/integration/main.rs` | 4.4, 4.5 | module declarations |
| `crates/service/src/command.rs` | 7.1, 7.2, 7.4, 7.6 | module declarations |
| `crates/service/src/http.rs` | 7.1, 7.3, 7.4, 7.5 | declarations of the four modules; `AppState` with `FromRef`, module `docs`; module declarations |
| `crates/service/tests/integration/main.rs` | 7.1, 7.2, 7.4, 7.5, 7.6 | module declarations; module declaration |
| `crates/service/tests/integration/support.rs` | 7.1, 7.2, 7.6 | `database_url`; log-line reader, SIGTERM, socket and timing helpers |
| `.github/openapi-spec/spec.yaml` | 7.4, 7.6, 9.1 | only where the early Schemathesis run of this story exposes a difference between the document and the behaviour; only if Schemathesis against the production image exposes a difference that the early run of Story 7.6 did not show |
| `docs/load-testing.md` | 8.7, 10.5 | the first local p99 of the `health` smoke profile with its date |
| `docs/operations.md` | 8.7, 10.5 | measured durations of `make bootstrap`, `make ci`, `make start` and the mutation run |
| `docs/repository-settings.md` | 8.7, 10.5 | the job names, checked against the workflow files |
| TOML files created by Story 1.1 | 1.1, 1.4 | whitespace only, to match `taplo fmt` |
| test files and, where a line cannot be reached, production source under `crates/` and `xtask/` | owning stories of Epics 2 and 4 to 7, then 7.6 and 9.1 (only if Schemathesis exposes a difference), 9.2, 9.3 | tests added to reach the coverage floor and to kill surviving mutants |
| tracked Markdown files of Stories 8.1 to 8.7 | owning story, then 9.9 | a citation that the documentation-drift gate reports is corrected |
| any file whose gate still fails on the complete tree | owning story, then 10.4, then 10.5 | fixes found by the full local gate run and by the clean-clone run, listed in the completion notes |
