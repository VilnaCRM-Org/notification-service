# Parity Matrix

Traceability from every PHP capability id and every added `NEW` id to its disposition, the requirements of the PRD traceability index and the stories of [epics.md](epics.md) that implement those requirements. The capability rows come from [capability-catalog.md](capability-catalog.md), the requirement ids from the "Traceability" section of [prd.md](prd.md), and the story ids from the `Implements` line of each story in [epics.md](epics.md).

## How to read the matrix

- **Disposition** is the catalog disposition (`KEEP`, `FIX`, `EXTEND`, `DROP`; `NEW` for an addition beyond PHP). **Status** is `Delivered` for every id that a requirement covers and `Dropped` for the 11 `DROP` ids.
- **Requirements** are the ids of the PRD index. **Stories** are all stories whose `Implements` line names at least one of those requirements; a requirement that several stories share (for example a non-functional requirement) lists all of them.
- A `DROP` row has no requirement and no story; its reason is given instead.

## Capability matrix

### PHP capability ids (196)

| Id | Area | Capability | Disposition | Status | Requirements | Stories |
| --- | --- | --- | --- | --- | --- | --- |
| CAP-01 | Service runtime and HTTP surface | One deployable HTTP service whose behaviour is selected by an environment mode (dev, test, prod) with per-mode settings. | KEEP | Delivered | FR1 | 7.6 |
| CAP-02 | Service runtime and HTTP surface | An operator CLI shares the service configuration and can run migrations, check configuration and export the API contracts. | KEEP | Delivered | FR2 | 7.1, 7.2, 7.4, 7.6 |
| CAP-03 | Service runtime and HTTP surface | All deploy-specific settings come from environment variables, with committed non-secret defaults for local use. | FIX | Delivered | FR3, NFR10 | 1.1, 3.3, 7.1 |
| CAP-04 | Service runtime and HTTP surface | Components are wired by constructor injection in one place, and every implementation of a contract (probes, handlers, subscribers) can be supplied as a collection. | KEEP | Delivered | FR5, FR28 | 4.4, 7.5 |
| CAP-05 | Service runtime and HTTP surface | The production build carries start-up optimisations and no debug artefacts. | KEEP | Delivered | FR87 | 3.1, 7.6 |
| CAP-06 | Service runtime and HTTP surface | All API routes are served under the `/api` path prefix. | KEEP | Delivered | FR6 | 4.3, 7.5 |
| CAP-07 | Health and liveness | `GET /api/health` returns `204 No Content` with an empty body when the database, cache and message-broker probes all succeed. | KEEP | Delivered | FR14 | 5.1 |
| CAP-08 | Health and liveness | When a probe fails, the health endpoint returns `500` with an `application/problem+json` body that contains the failure message. | FIX | Delivered | FR15, FR126, NFR30 | 4.5, 5.1, 7.6, 7.7 |
| CAP-09 | Health and liveness | The database probe runs a trivial query (`SELECT 1`) on the primary PostgreSQL connection. | KEEP | Delivered | FR16 | 5.2 |
| CAP-10 | Health and liveness | The cache probe does a read-through fetch of key `health_check` (storing `ok` on a miss) on the application cache. | FIX | Delivered | FR16 | 5.2 |
| CAP-11 | Health and liveness | The message-broker probe calls the SQS-compatible endpoint. | FIX | Delivered | FR16 | 5.2 |
| CAP-12 | Health and liveness | Health probes are pluggable: adding a probe adds one component and does not change the endpoint handler. | FIX | Delivered | FR17, NFR2 | 5.1, 5.2, 7.5, 7.6 |
| CAP-13 | Example bounded context | An example resource endpoint `GET /api/example_api_resources` returns `200 application/json` with `name` and `description` and demonstrates the module layout. | EXTEND | Delivered | FR19, FR22, FR57 | 4.1, 6.2, 6.3, 7.7 |
| CAP-14 | Service runtime and HTTP surface | The API is JSON-only: success bodies are `application/json` and every error is a problem document. | FIX | Delivered | FR6, FR7, NFR15 | 4.3, 6.3, 7.3, 7.5 |
| CAP-15 | Architecture and its enforcement | Transport and framework metadata are declared outside domain types, so the Domain layer carries no framework code. | KEEP | Delivered | FR37 | 1.1, 2.1, 2.2, 4.1, 6.1 |
| CAP-16 | Contract artifacts and contract checks | An OpenAPI 3.1 document is generated from the application's own definitions, carries API metadata and documented responses, and is exported by CLI to a versioned file. | FIX | Delivered | FR71, FR73, NFR7 | 5.1, 6.3, 7.4, 10.3 |
| CAP-17 | Contract artifacts and contract checks | The generated OpenAPI document carries human-readable tag descriptions. | KEEP | Delivered | FR71 | 5.1, 6.3, 7.4 |
| CAP-18 | Service runtime and HTTP surface | Human-browsable API documentation is served at `/api/docs` with HTTP 200. | KEEP | Delivered | FR8 | 7.4 |
| CAP-19 | Service runtime and HTTP surface | A GraphQL endpoint is served at `/api/graphql`, its schema is exportable as SDL to a versioned file, and the browser IDE is disabled. | FIX | Delivered | FR9, FR24 | 6.3, 7.4, 7.5 |
| CAP-20 | Service runtime and HTTP surface | Development-only error-page preview route | DROP | Dropped | none (reason: Part of HTML templating (Out of Scope: "Mercure, Vulcain, HTML templating, sessions")) | none |
| CAP-21 | Persistence, cache and messaging | PostgreSQL connectivity is configured from one URL variable, and tests use a separate database. | KEEP | Delivered | FR30, FR90, NFR29 | 1.1, 3.1, 4.2, 4.5, 6.4 |
| CAP-22 | Persistence, cache and messaging | Versioned, reversible schema migrations are kept in the repository and are runnable from the CLI. | FIX | Delivered | FR32, FR121, NFR32 | 1.1, 3.1, 4.5, 6.4, 7.2 |
| CAP-23 | Persistence, cache and messaging | Persistence mapping has a defined place and a snake_case naming convention. | FIX | Delivered | FR31 | 6.1, 6.4 |
| CAP-24 | Unit and integration tests | Empty data-fixture loader and its target | DROP | Dropped | none (reason: Scope principles 5 and 7: dead scaffolding; tests create their own data) | none |
| CAP-25 | Shared kernel | An aggregate-root base records domain events and hands them over exactly once, in recording order. | KEEP | Delivered | FR25 | 4.1, 6.1 |
| CAP-26 | Shared kernel | A domain-event contract defines event id, occurrence time, stable event name and conversion to and from primitive values. | FIX | Delivered | FR25, NFR28 | 1.1, 4.1, 4.4, 6.1, 9.5 |
| CAP-27 | Shared kernel | An event-bus port with an in-process synchronous adapter routes each event to its subscribers and surfaces a failing subscriber's error. | FIX | Delivered | FR26 | 4.4 |
| CAP-28 | Shared kernel | A command-bus port sends each command to exactly one handler, returns nothing, and raises a dedicated error when no handler is registered. | FIX | Delivered | FR27 | 4.1, 4.4, 6.2 |
| CAP-29 | Shared kernel | Handlers and subscribers are registered without editing the bus. | KEEP | Delivered | FR28 | 4.4, 7.5 |
| CAP-30 | Shared kernel | A UUID value object with a factory is available. | FIX | Delivered | FR29 | 4.1, 4.4 |
| CAP-31 | Persistence, cache and messaging | A configured client for an SQS-compatible broker points at a local emulator by default. | FIX | Delivered | FR35 | 4.5 |
| CAP-32 | Persistence, cache and messaging | An application cache abstraction lets the backend be swapped. | KEEP | Delivered | FR34 | 4.1, 4.4 |
| CAP-33 | Persistence, cache and messaging | Configuration slot for an asynchronous transport | DROP | Dropped | none (reason: Out of Scope: "Asynchronous message consumer / worker") | none |
| CAP-34 | Service runtime and HTTP surface | A request input validation facility is available. | EXTEND | Delivered | FR7, FR20 | 4.3, 6.1, 6.2, 6.3 |
| CAP-35 | Service runtime and HTTP surface | Translation and locale facility | DROP | Dropped | none (reason: Out of Scope: "Internationalisation (P20)") | none |
| CAP-36 | Service runtime and HTTP surface | Server-side HTML templating and sessions | DROP | Dropped | none (reason: Out of Scope: "Mercure, Vulcain, HTML templating, sessions") | none |
| CAP-37 | Architecture and its enforcement | The source layout is mandatory: bounded context, module, then Application / Domain / Infrastructure, plus a Shared kernel with the same layers and an Internal context for non-business features. | KEEP | Delivered | FR36 | 1.1 |
| CAP-38 | Architecture and its enforcement | Automated dependency-rule enforcement between layers fails on violations and on code assigned to no layer, and Domain depends on nothing. | FIX | Delivered | FR38, FR122, NFR18 | 1.3, 2.1, 2.2, 2.4, 3.7, 9.6 |
| CAP-39 | Architecture and its enforcement | Source units that belong to no layer can be listed. | KEEP | Delivered | FR38 | 2.1, 2.2 |
| CAP-40 | Architecture model (Structurizr) | The architecture is kept as code: a C4 model in the repository that is browsable locally. | FIX | Delivered | FR119, FR120 | 3.7, 9.10 |
| CAP-41 | Formatting, static analysis and complexity | A guard fails when a protected quality-configuration file is modified or a required directory is missing. | FIX | Delivered | FR44, NFR26 | 1.1, 2.3, 8.1, 10.2 |
| CAP-TQ-01 | Makefile command surface and its tests | One command exists per test level, plus an aggregate. | KEEP | Delivered | FR62, FR63 | 1.2 |
| CAP-TQ-02 | Unit and integration tests | The unit suite runs with no database, broker, cache or network. | KEEP | Delivered | FR50, NFR28 | 1.1, 4.1, 4.2, 4.4, 6.1, 9.2, 9.5 |
| CAP-TQ-03 | Unit and integration tests | Any collaborator can be replaced by a test double. | FIX | Delivered | FR50 | 4.1, 4.2, 6.1, 9.2 |
| CAP-TQ-04 | Unit and integration tests | Each test level has shared test support with a random data generator. | FIX | Delivered | FR51, NFR29 | 1.1, 4.2, 6.4, 10.3 |
| CAP-TQ-05 | Unit and integration tests | Every production unit has its tests at a predictable location. | KEEP | Delivered | FR50 | 4.1, 4.2, 6.1, 9.2 |
| CAP-TQ-06 | Unit and integration tests | The integration suite runs against real backing services, locally and in CI. | KEEP | Delivered | FR52 | 4.5, 9.2 |
| CAP-TQ-07 | Unit and integration tests | One command drops, recreates and migrates the test database, and a second does the same for the load-test stack. | KEEP | Delivered | FR53 | 4.5, 9.1, 9.6 |
| CAP-TQ-08 | Unit and integration tests | Tests run under a dedicated configuration that never touches development data. | KEEP | Delivered | FR53 | 4.5, 9.1, 9.6 |
| CAP-TQ-09 | End-to-end tests | Acceptance tests written in Gherkin drive the HTTP entry point, locally and in CI. | FIX | Delivered | FR55, NFR30 | 5.1, 7.7 |
| CAP-TQ-10 | Health and liveness | `GET /api/health` returns `204 No Content` with an empty body when the database, cache and message-broker probes all succeed. | KEEP | Delivered | FR14 | 5.1 |
| CAP-TQ-11 | End-to-end tests | For each dependency a scenario injects a failure and expects `500` with that dependency's message. | KEEP | Delivered | FR56 | 7.7 |
| CAP-TQ-12 | End-to-end tests | A fault injected in one scenario does not leak into the next. | KEEP | Delivered | FR56, NFR30 | 5.1, 7.7 |
| CAP-TQ-13 | End-to-end tests | A smoke test checks that the API documentation endpoint answers `200`. | FIX | Delivered | FR57 | 7.7 |
| CAP-TQ-14 | Unit and integration tests | Each dependency probe is verified at unit level and at integration level. | FIX | Delivered | FR54 | 5.1, 5.2 |
| CAP-TQ-15 | Unit and integration tests | Empty data-fixture loader and its target | DROP | Dropped | none (reason: Scope principles 5 and 7: dead scaffolding; tests create their own data) | none |
| CAP-TQ-16 | Coverage and mutation testing | Coverage over all production source from unit and integration runs is reported in machine-readable and HTML form and published to an external coverage service. | FIX | Delivered | FR58, NFR19, NFR41 | 1.2, 9.2, 10.1, 10.4 |
| CAP-TQ-17 | Coverage and mutation testing | Mutation testing mutates production source, judges it with the test suite in parallel and writes logs. | FIX | Delivered | FR59, NFR20 | 9.3, 9.6, 10.1 |
| CAP-TQ-18 | Makefile command surface and its tests | Every developer-facing command is executed by an automated test that checks exit status and output, locally and in CI. | FIX | Delivered | FR67 | 9.4, 9.5, 9.6, 9.7, 9.10, 9.11, 10.4 |
| CAP-TQ-19 | Makefile command surface and its tests | For each gate a negative test plants a bad artefact and proves that the gate fails. | FIX | Delivered | FR67, NFR21 | 2.1, 2.2, 2.3, 9.4, 9.5, 9.6, 9.7, 9.10, 9.11, 10.4 |
| CAP-TQ-20 | Formatting, static analysis and complexity | Static correctness analysis runs over production and test code with zero findings. | FIX | Delivered | FR41, NFR16 | 1.1, 1.2, 9.8 |
| CAP-TQ-21 | Dependency and security checks | Security-focused static analysis fails the build on findings and publishes them to the repository's code-scanning view. | FIX | Delivered | FR49 | 10.1, 10.2 |
| CAP-TQ-22 | Formatting, static analysis and complexity | A quality gate applies numeric minimums for code quality, complexity, architecture and style. | KEEP | Delivered | FR41, FR123, NFR17 | 1.1, 1.2 |
| CAP-TQ-23 | Formatting, static analysis and complexity | Structural code rules are enforced automatically. | KEEP | Delivered | FR41, NFR16 | 1.1, 1.2, 9.8 |
| CAP-TQ-24 | Formatting, static analysis and complexity | Complexity and mess detection runs with a relaxed profile for tests. | FIX | Delivered | FR123, NFR17 | 1.1, 1.2 |
| CAP-TQ-25 | Formatting, static analysis and complexity | Code is formatted automatically with a pinned rule set. | FIX | Delivered | FR40, NFR16 | 1.1, 1.2, 9.8 |
| CAP-TQ-26 | Formatting, static analysis and complexity | An automated modernisation check fails when machine-applicable rewrites are pending, and a separate mode applies them. | KEEP | Delivered | FR42 | 1.2 |
| CAP-TQ-27 | CI workflows | Bot that commits formatting fixes to `main` | DROP | Dropped | none (reason: Scope 3.1: no bot commits other than the release identity; replaced by the failing format check (FR40, FR96)) | none |
| CAP-TQ-28 | Git hooks and commit convention | Commit messages follow `type(#issue): subject` with a fixed type list, and the convention drives semantic versioning and is documented for contributors. | FIX | Delivered | FR102 | 3.6, 10.2 |
| CAP-TQ-29 | Git hooks and commit convention | A pre-commit pipeline runs quality checks before a commit is accepted. | FIX | Delivered | FR103 | 3.6 |
| CAP-TQ-30 | Dependency and security checks | Locked dependencies are audited for known vulnerabilities, and any advisory fails the build. | EXTEND | Delivered | FR46, NFR9 | 1.1, 1.3 |
| CAP-TQ-31 | Formatting, static analysis and complexity | A guard fails when a protected quality-configuration file is modified or a required directory is missing. | FIX | Delivered | FR44 | 2.3, 10.2 |
| CAP-TQ-32 | Makefile command surface and its tests | One aggregate local command runs all gates and prints a success line, and conventional aliases exist. | FIX | Delivered | FR66, FR125, NFR22 | 1.2, 9.8, 10.4, 10.5 |
| CAP-TQ-33 | CI workflows | Each gate is a separately attributable status check on every pull request to the default branch. | KEEP | Delivered | FR93 | 10.1, 10.2, 10.5 |
| CAP-TQ-34 | Makefile command surface and its tests | Every command works both by executing inside the development container and by running directly. | FIX | Delivered | FR65, NFR36 | 1.2, 7.1, 9.8, 9.11 |
| CAP-TQ-35 | Containers and compose stacks | Test tooling and development files stay out of the production image. | FIX | Delivered | FR87 | 3.1, 7.6 |
| CAP-TQ-36 | AI-assistant guidance | Guidance tells assistants which gate commands to run and forbids weakening quality configuration to obtain a pass. | FIX | Delivered | FR109 | 8.1 |
| CAP-DM-01 | Containers and compose stacks | One multi-stage image definition has a shared base and separate development and production targets. | KEEP | Delivered | FR86 | 3.1 |
| CAP-DM-02 | Containers and compose stacks | The production image is minimal and runs as a non-root user. | EXTEND | Delivered | FR87, NFR12 | 3.1, 7.6, 9.7 |
| CAP-DM-03 | Containers and compose stacks | The development image carries the full toolchain needed for every gate and Make target. | KEEP | Delivered | FR86 | 3.1 |
| CAP-DM-04 | Containers and compose stacks | Dependencies are cached in an image layer separate from source. | KEEP | Delivered | NFR40 | 3.1, 10.1, 10.2 |
| CAP-DM-05 | Dependency and security checks | Third-party binaries and tool images are pinned by tag or digest and verified. | FIX | Delivered | FR125, NFR13 | 1.1, 1.3, 3.1, 7.4, 9.8, 10.1, 10.2, 10.3, 10.5 |
| CAP-DM-06 | Containers and compose stacks | The image build context is lean. | KEEP | Delivered | NFR40 | 3.1, 10.1, 10.2 |
| CAP-DM-07 | Containers and compose stacks | An image-level health check calls the liveness path and allows a start grace period. | KEEP | Delivered | FR87, NFR3 | 3.1, 7.6, 9.7 |
| CAP-DM-08 | Containers and compose stacks | Start-up waits for the database with a bounded retry and exits non-zero with the error when it never becomes reachable. | KEEP | Delivered | FR88, NFR33 | 3.1, 7.2, 9.11 |
| CAP-DM-09 | Persistence, cache and messaging | A started stack has its pending migrations applied without a manual step. | FIX | Delivered | FR32 | 3.1, 4.5, 7.2 |
| CAP-DM-10 | Containers and compose stacks | The development container never runs with dependencies older than the lock file. | KEEP | Delivered | FR45 | 1.1, 1.2 |
| CAP-DM-11 | Containers and compose stacks | Source changes reload the service automatically in local development, and not in production or CI. | KEEP | Delivered | FR89 | 3.1, 9.11 |
| CAP-DM-12 | Containers and compose stacks | Arguments given to the container are passed to the service command. | KEEP | Delivered | FR87 | 3.1, 7.6 |
| CAP-DM-13 | Containers and compose stacks | One command boots the complete local stack in the background and returns only when it is healthy, and CI uses the same command. | KEEP | Delivered | FR88, NFR35 | 3.1, 9.11, 10.4, 10.5 |
| CAP-DM-14 | Containers and compose stacks | The local stack consists of the application, PostgreSQL with a persistent volume and readiness probe, an AWS-API emulator and an architecture-diagram viewer. | KEEP | Delivered | FR88, NFR33 | 3.1, 7.2, 9.11 |
| CAP-DM-15 | Containers and compose stacks | The application starts only after the database is ready. | KEEP | Delivered | FR88 | 3.1, 9.11 |
| CAP-DM-16 | Containers and compose stacks | Every published port and the database credentials are configurable through one committed env file with working defaults. | FIX | Delivered | FR90 | 1.1, 3.1 |
| CAP-DM-17 | Containers and compose stacks | Host source edits are visible in the development container without rebuilding the image. | KEEP | Delivered | FR89, NFR37 | 1.2, 1.3, 3.1, 9.4, 9.11 |
| CAP-DM-18 | Service runtime and HTTP surface | A plain-HTTP listener on port 8081 serves tooling (health probe, load tests, API fuzzing, CLI tests). | KEEP | Delivered | FR1 | 7.6 |
| CAP-DM-19 | Service runtime and HTTP surface | HTTPS, HTTP/3 and compression in the service container | DROP | Dropped | none (reason: Out of Scope: "Reverse proxy, in-process TLS, HTTP/3"; request logging moves into the service (FR10)) | none |
| CAP-DM-20 | Health and liveness | A liveness path separate from the dependency health endpoint answers `GET /ping` with `200` and body `pong` without touching any dependency. | KEEP | Delivered | FR18, FR126 | 4.5, 7.3, 7.4, 7.6 |
| CAP-DM-21 | Containers and compose stacks | A production compose file forces production mode and requires real secrets. | EXTEND | Delivered | FR91, NFR12 | 3.1, 9.7 |
| CAP-DM-22 | Containers and compose stacks | Debugger call-back wiring to the host IDE | DROP | Dropped | none (reason: PHP-specific mechanism; the dev container covers in-container debugging (FR118)) | none |
| CAP-DM-23 | Makefile command surface and its tests | Stack lifecycle commands start, stop, remove and rebuild the stack, open a shell, and follow all logs or only new logs. | KEEP | Delivered | FR62, FR63 | 1.2 |
| CAP-DM-24 | Makefile command surface and its tests | The task runner documents itself: with no argument it prints usage and every documented target. | KEEP | Delivered | FR60, NFR45 | 1.2, 8.5, 9.4, 9.9 |
| CAP-DM-25 | Makefile command surface and its tests | One named command exists per quality gate, test level, contract tool and maintenance action, and CI, hooks, documentation and assistant guidance all call it. | KEEP | Delivered | FR61, FR62, FR63 | 1.2 |
| CAP-DM-26 | Makefile command surface and its tests | Every command works both by executing inside the development container and by running directly. | FIX | Delivered | FR65, NFR36 | 1.2, 7.1, 9.8, 9.11 |
| CAP-DM-27 | Makefile command surface and its tests | Quality and test commands run under the test configuration without the caller setting it. | FIX | Delivered | FR64 | 1.2 |
| CAP-DM-28 | Makefile command surface and its tests | One aggregate local command runs all gates and prints a success line, and conventional aliases exist. | FIX | Delivered | FR66 | 1.2, 10.4, 10.5 |
| CAP-DM-29 | Unit and integration tests | One command drops, recreates and migrates the test database, and a second does the same for the load-test stack. | KEEP | Delivered | FR53 | 4.5, 9.1, 9.6 |
| CAP-DM-30 | Makefile command surface and its tests | Dependency management commands install from the lock, update, validate, check requirements and audit. | KEEP | Delivered | FR62, FR63 | 1.2 |
| CAP-DM-31 | Makefile command surface and its tests | Migration and maintenance commands exist. | KEEP | Delivered | FR63 | 1.2 |
| CAP-DM-32 | Coverage and mutation testing | Coverage commands write machine-readable and HTML reports to known paths. | FIX | Delivered | FR58 | 9.2, 10.1 |
| CAP-DM-33 | Formatting, static analysis and complexity | A guard fails when a protected quality-configuration file is modified or a required directory is missing. | FIX | Delivered | FR44 | 2.3, 10.2 |
| CAP-DM-34 | Contract artifacts and contract checks | Commands export the REST contract and the GraphQL schema to versioned files. | KEEP | Delivered | FR72 | 7.4 |
| CAP-DM-35 | Contract artifacts and contract checks | The REST contract is linted against a house ruleset with zero tolerance. | KEEP | Delivered | FR74, NFR24 | 3.2, 7.4, 9.1 |
| CAP-DM-36 | Contract artifacts and contract checks | The REST contract of a change is compared with the default branch for backward compatibility. | FIX | Delivered | FR75, NFR24 | 3.2, 7.4, 9.1, 10.2 |
| CAP-DM-37 | Contract artifacts and contract checks | Contract-driven conformance and fuzz tests run against the running service. | FIX | Delivered | FR76, NFR24 | 3.2, 7.4, 9.1 |
| CAP-DM-38 | Load testing | Load tests run against a production-build stack that one command builds, starts, health-gates, initialises and always tears down. | FIX | Delivered | FR77 | 9.1 |
| CAP-DM-39 | Load testing | Four standard load profiles per endpoint (smoke, average, stress, spike) run one by one or in a single run. | KEEP | Delivered | FR78 | 3.4 |
| CAP-DM-40 | Load testing | Per-endpoint load configuration is declarative in one JSON file, with a local override file and environment overrides for host and port. | KEEP | Delivered | FR79 | 3.4 |
| CAP-DM-41 | Load testing | Performance gates pass or fail a run: p99 latency per profile and a check success rate above 99%. | FIX | Delivered | FR80, NFR1 | 3.4, 8.7, 9.1, 10.5 |
| CAP-DM-42 | Load testing | Adding one scenario file adds an endpoint to every runner. | KEEP | Delivered | FR79 | 3.4 |
| CAP-DM-43 | Load testing | A single scenario can be run with per-profile switches. | FIX | Delivered | FR81 | 3.4 |
| CAP-DM-44 | Load testing | Each run writes one self-contained HTML report to an ignored results directory and prints a console summary. | EXTEND | Delivered | FR82 | 3.4, 10.2 |
| CAP-DM-45 | Load testing | The load generator is a locally built container image, so no host installation is needed. | FIX | Delivered | FR77 | 9.1 |
| CAP-DM-46 | Load testing | A baseline scenario for the health endpoint asserts the documented success status. | KEEP | Delivered | FR78 | 3.4 |
| CAP-DM-47 | Load testing | The load-test scripts are statically analysed. | FIX | Delivered | FR83, NFR25 | 1.4, 3.4, 9.10 |
| CAP-DM-48 | Load testing | A cloud load run provisions temporary infrastructure with one command, runs the tests on an instance, uploads reports and terminates, and a second command removes everything. | FIX | Delivered | FR85 | 3.5, 9.7 |
| CAP-DM-49 | Load testing | The cloud run is parameterised by region, image, instance type, tag, role, branch and security group. | FIX | Delivered | FR85 | 3.5, 9.7 |
| CAP-DM-50 | Load testing | The cloud provisioning script can be dry-run offline against the local emulator, and a CLI test covers it. | KEEP | Delivered | FR85 | 3.5, 9.7 |
| CAP-CI-01 | Containers and compose stacks | One command boots the complete local stack in the background and returns only when it is healthy, and CI uses the same command. | KEEP | Delivered | FR88 | 3.1, 9.11 |
| CAP-CI-02 | CI workflows | Each gate is a separately attributable status check on every pull request to the default branch. | KEEP | Delivered | FR93 | 10.1, 10.2, 10.5 |
| CAP-CI-03 | Unit and integration tests | The integration suite runs against real backing services, locally and in CI. | KEEP | Delivered | FR52 | 4.5, 9.2 |
| CAP-CI-04 | Coverage and mutation testing | Coverage over all production source from unit and integration runs is reported in machine-readable and HTML form and published to an external coverage service. | FIX | Delivered | FR58, NFR41 | 1.2, 9.2, 10.1, 10.4 |
| CAP-CI-05 | End-to-end tests | Acceptance tests written in Gherkin drive the HTTP entry point, locally and in CI. | FIX | Delivered | FR55 | 7.7 |
| CAP-CI-06 | Makefile command surface and its tests | Every developer-facing command is executed by an automated test that checks exit status and output, locally and in CI. | FIX | Delivered | FR67 | 9.4, 9.5, 9.6, 9.7, 9.10, 9.11, 10.4 |
| CAP-CI-07 | Coverage and mutation testing | Mutation testing mutates production source, judges it with the test suite in parallel and writes logs. | FIX | Delivered | FR59 | 9.3, 9.6 |
| CAP-CI-08 | Architecture and its enforcement | Automated dependency-rule enforcement between layers fails on violations and on code assigned to no layer, and Domain depends on nothing. | FIX | Delivered | FR38 | 2.1, 2.2 |
| CAP-CI-09 | Formatting, static analysis and complexity | Static correctness analysis runs over production and test code with zero findings. | FIX | Delivered | FR41 | 1.1 |
| CAP-CI-10 | Dependency and security checks | Security-focused static analysis fails the build on findings and publishes them to the repository's code-scanning view. | FIX | Delivered | FR49 | 10.1, 10.2 |
| CAP-CI-11 | Formatting, static analysis and complexity | A quality gate applies numeric minimums for code quality, complexity, architecture and style. | KEEP | Delivered | FR41, FR123 | 1.1 |
| CAP-CI-12 | Formatting, static analysis and complexity | An automated modernisation check fails when machine-applicable rewrites are pending, and a separate mode applies them. | KEEP | Delivered | FR42 | 1.2 |
| CAP-CI-13 | CI workflows | Bot that commits formatting fixes to `main` | DROP | Dropped | none (reason: Scope 3.1: no bot commits other than the release identity; replaced by the failing format check (FR40, FR96)) | none |
| CAP-CI-14 | Dependency and security checks | The dependency manifest and lockfile are validated. | KEEP | Delivered | FR45, FR122, NFR9 | 1.1, 1.2, 1.3 |
| CAP-CI-15 | Service runtime and HTTP surface | A check confirms that the runtime environment meets the service's requirements. | KEEP | Delivered | FR4, NFR36 | 7.1, 9.8, 9.11 |
| CAP-CI-16 | Dependency and security checks | Locked dependencies are audited for known vulnerabilities, and any advisory fails the build. | EXTEND | Delivered | FR46 | 1.3 |
| CAP-CI-17 | Load testing | A smoke load test runs on pull requests against the production build with latency and success-rate thresholds. | EXTEND | Delivered | FR84, NFR1 | 3.4, 8.7, 9.1, 10.2, 10.5 |
| CAP-CI-18 | Load testing | The load-test scripts are statically analysed. | FIX | Delivered | FR83 | 1.4, 3.4 |
| CAP-CI-19 | Contract artifacts and contract checks | An OpenAPI 3.1 document is generated from the application's own definitions, carries API metadata and documented responses, and is exported by CLI to a versioned file. | FIX | Delivered | FR71, FR73 | 5.1, 6.3, 7.4 |
| CAP-CI-20 | Contract artifacts and contract checks | The REST contract is linted against a house ruleset with zero tolerance. | KEEP | Delivered | FR74 | 3.2, 7.4 |
| CAP-CI-21 | Contract artifacts and contract checks | The REST contract of a change is compared with the default branch for backward compatibility. | FIX | Delivered | FR75 | 3.2, 10.2 |
| CAP-CI-22 | Contract artifacts and contract checks | Contract-driven conformance and fuzz tests run against the running service. | FIX | Delivered | FR76 | 9.1 |
| CAP-CI-23 | Contract artifacts and contract checks | The GraphQL schema snapshot is generated from code and kept current at a fixed path. | FIX | Delivered | FR72, FR73, FR96, NFR7 | 7.4, 10.1, 10.2, 10.3 |
| CAP-CI-24 | Contract artifacts and contract checks | GraphQL breaking changes against the default branch are detected, and a label approves an intentional break. | KEEP | Delivered | FR75, NFR24 | 3.2, 7.4, 9.1, 10.2 |
| CAP-CI-25 | Release, dependency updates and template sync | A merge to the default branch bumps the version from conventional commits, regenerates the changelog, writes the version to the manifest, pushes a release commit and a `v` tag, and creates a hosted release without a CI loop. | KEEP | Delivered | FR96, FR98, NFR8 | 3.6, 10.1, 10.2, 10.3 |
| CAP-CI-26 | Git hooks and commit convention | Commit messages follow `type(#issue): subject` with a fixed type list, and the convention drives semantic versioning and is documented for contributors. | FIX | Delivered | FR102 | 3.6, 10.2 |
| CAP-CI-27 | Release, dependency updates and template sync | Downstream repositories receive template changes as scheduled, labelled, self-updating pull requests, with two authentication options. | FIX | Delivered | FR100, NFR8, NFR38 | 1.4, 9.8, 9.11, 10.1, 10.2, 10.3 |
| CAP-CI-28 | Release, dependency updates and template sync | Dependency-update pull requests are opened automatically, grouped per ecosystem, with commit prefixes compatible with the release convention. | EXTEND | Delivered | FR99, NFR42 | 10.3 |
| CAP-CI-29 | Human documentation | Issue templates and a pull-request checklist carry the process rules. | FIX | Delivered | FR116 | 8.4 |
| CAP-CI-30 | CI workflows | Automated AI code review is configured to request changes. | KEEP | Delivered | FR97, NFR8 | 8.4, 10.1, 10.2, 10.3 |
| CAP-CI-31 | Health and liveness | A liveness path separate from the dependency health endpoint answers `GET /ping` with `200` and body `pong` without touching any dependency. | KEEP | Delivered | FR18 | 7.3, 7.4 |
| CAP-CI-32 | Makefile command surface and its tests | One aggregate local command runs all gates and prints a success line, and conventional aliases exist. | FIX | Delivered | FR66, NFR22 | 1.2, 9.8, 10.4, 10.5 |
| CAP-CI-33 | Formatting, static analysis and complexity | A guard fails when a protected quality-configuration file is modified or a required directory is missing. | FIX | Delivered | FR44 | 2.3, 10.2 |
| CAP-CI-34 | Formatting, static analysis and complexity | Formatting and lint conventions exist for non-application files (YAML, Markdown, JavaScript), with generated contract files excluded. | FIX | Delivered | FR43, NFR25 | 1.4, 3.4, 9.10 |
| CAP-CI-35 | Release, dependency updates and template sync | Setup documentation covers the release and template-sync automation: credentials, permissions and how a downstream repository receives updates. | FIX | Delivered | FR101 | 8.7 |
| CAP-CI-36 | Makefile command surface and its tests | Ignore rules cover local environment files, tool caches and generated output. | FIX | Delivered | FR68 | 10.4, 10.5 |
| CAP-AD-01 | AI-assistant guidance | A cross-tool agent entry file at the repository root states project identity, the mandatory reading order, working rules, the verification baseline and the skill inventory. | FIX | Delivered | FR104, NFR43 | 8.1, 9.9 |
| CAP-AD-02 | AI-assistant guidance | A tool-specific entry file only delegates to the cross-tool file and adds a project snapshot and a command reference. | KEEP | Delivered | FR105 | 8.1 |
| CAP-AD-03 | AI-assistant guidance | A skills directory provides an index, a usage guide and a decision guide. | KEEP | Delivered | FR106 | 8.2 |
| CAP-AD-04 | AI-assistant guidance | Task skills are small Markdown files with `name` and `description` metadata that any assistant can use. | FIX | Delivered | FR107, NFR44 | 8.2, 9.9 |
| CAP-AD-05 | AI-assistant guidance | A decision tree maps each task intent to one primary skill and lists multi-skill scenarios and non-negotiable rules. | KEEP | Delivered | FR106 | 8.2 |
| CAP-AD-06 | AI-assistant guidance | Standard flows (code change, API change, architecture change) tell an agent which skills to chain. | KEEP | Delivered | FR106 | 8.2 |
| CAP-AD-07 | AI-assistant guidance | A skill covers adding or extending an API resource end to end with a framework-free domain and regenerated contracts. | FIX | Delivered | FR108, NFR44 | 8.2, 9.9 |
| CAP-AD-08 | AI-assistant guidance | A skill defines the full ordered verification stack, routes each failing check to a skill and defines an honest close-out. | FIX | Delivered | FR108 | 8.2 |
| CAP-AD-09 | AI-assistant guidance | A skill quotes the numeric maintainability thresholds and gives refactoring guidance instead of threshold changes. | KEEP | Delivered | FR108 | 8.2 |
| CAP-AD-10 | AI-assistant guidance | A skill explains how to fix architecture-boundary violations by moving code and catalogues common violation patterns. | KEEP | Delivered | FR108 | 8.2 |
| CAP-AD-11 | AI-assistant guidance | A skill enumerates documentation surfaces, the triggers for updating them and typical sync patterns. | FIX | Delivered | FR108 | 8.2 |
| CAP-AD-12 | AI-assistant guidance | A skill covers adding and running load-test scenarios, smoke first. | KEEP | Delivered | FR108 | 8.2 |
| CAP-AD-13 | AI-assistant guidance | A skill keeps the generated contract snapshots current and in the same change as the code. | FIX | Delivered | FR108 | 8.2 |
| CAP-AD-14 | AI-assistant guidance | A reference skill lists every protected quality gate with its value and command. | FIX | Delivered | FR108 | 8.2 |
| CAP-AD-15 | AI-assistant guidance | A skill gives criteria for when the architecture model must be updated. | FIX | Delivered | FR108 | 8.2 |
| CAP-AD-16 | AI-assistant guidance | A skill maps test surfaces to commands and gives risk-tiered run orders, debugging notes and done criteria. | KEEP | Delivered | FR108 | 8.2 |
| CAP-AD-17 | AI-assistant guidance | The command policy is wrapper-first: use the repository's commands, not raw tool invocations. | FIX | Delivered | FR109 | 8.1 |
| CAP-AD-18 | AI-assistant guidance | Guidance tells assistants which gate commands to run and forbids weakening quality configuration to obtain a pass. | FIX | Delivered | FR109 | 8.1 |
| CAP-AD-19 | AI-assistant guidance | The domain-purity policy is stated in guidance and backed by the automated architecture check. | KEEP | Delivered | FR109 | 8.1 |
| CAP-AD-20 | AI-assistant guidance | Contract snapshots and the architecture model change in the same commit as the code they describe. | FIX | Delivered | FR109, FR120 | 3.7, 8.1, 9.10 |
| CAP-AD-21 | AI-assistant guidance | An agent states what was not run and why before claiming completion. | KEEP | Delivered | FR108 | 8.2 |
| CAP-AD-22 | AI-assistant guidance | Guidance avoids service-specific assumptions so it can be synced into downstream services. | FIX | Delivered | FR92, FR107, NFR38 | 1.1, 1.2, 1.4, 8.2, 9.8, 9.11, 10.3 |
| CAP-AD-23 | AI-assistant guidance | A recommended prompt pattern tells humans how to instruct an assistant. | KEEP | Delivered | FR113 | 8.4 |
| CAP-AD-24 | Human documentation | A README explains the purpose, the baseline, a one-command quick start with entry-point URLs, the AI workflow, local commands, the CI model, downstream sync, cloud load testing, documentation links and the licence. | FIX | Delivered | FR113, NFR46 | 8.4, 8.5, 8.6, 8.7 |
| CAP-AD-25 | Human documentation | A badge row signals code health, coverage and analysis scores, preceded by a solidarity banner. | FIX | Delivered | FR113 | 8.4 |
| CAP-AD-26 | Human documentation | A contributing guide covers issue, branch, commit convention, pull request, review and merge. | FIX | Delivered | FR114 | 8.4 |
| CAP-AD-27 | Git hooks and commit convention | Commit messages follow `type(#issue): subject` with a fixed type list, and the convention drives semantic versioning and is documented for contributors. | FIX | Delivered | FR102, FR114 | 3.6, 8.4, 10.2 |
| CAP-AD-28 | Human documentation | A security policy gives a private disclosure channel. | FIX | Delivered | FR115, NFR46 | 8.4, 8.5, 8.6, 8.7 |
| CAP-AD-29 | Release, dependency updates and template sync | A merge to the default branch bumps the version from conventional commits, regenerates the changelog, writes the version to the manifest, pushes a release commit and a `v` tag, and creates a hosted release without a CI loop. | KEEP | Delivered | FR98 | 3.6, 10.3 |
| CAP-AD-30 | Human documentation | A public-domain dedication is the licence and is stated consistently. | FIX | Delivered | FR125, NFR47 | 1.1, 7.4, 8.4, 9.8, 10.5 |
| CAP-AD-31 | Human documentation | Issue templates and a pull-request checklist carry the process rules. | FIX | Delivered | FR116 | 8.4 |
| CAP-AD-32 | Release, dependency updates and template sync | Setup documentation covers the release and template-sync automation: credentials, permissions and how a downstream repository receives updates. | FIX | Delivered | FR101 | 8.7 |
| CAP-AD-33 | Human documentation | A new quality-audit document | DROP | Dropped | none (reason: Scope principle 7; the PHP audit is consumed as input (table below)) | none |

### Added capabilities (42)

| Id | Area | Capability | Disposition | Status | Requirements | Stories |
| --- | --- | --- | --- | --- | --- | --- |
| NEW-01 | Service runtime and HTTP surface | The service writes structured JSON logs. | NEW | Delivered | FR10, NFR31 | 7.3, 7.6 |
| NEW-02 | Service runtime and HTTP surface | A request/correlation id is accepted from or generated for every request, echoed in the response and attached to every log line. | NEW | Delivered | FR10, NFR15, NFR31 | 4.3, 7.3, 7.5, 7.6 |
| NEW-03 | Service runtime and HTTP surface | Security response headers are set on every response. | NEW | Delivered | FR11, NFR15 | 4.3, 7.3, 7.4, 7.5 |
| NEW-04 | Service runtime and HTTP surface | The service shuts down gracefully on SIGTERM. | NEW | Delivered | FR12, NFR5 | 7.6 |
| NEW-05 | Service runtime and HTTP surface | Configuration is typed and validated at start-up, and the service refuses to start on an invalid or missing value. | NEW | Delivered | FR4 | 7.1 |
| NEW-06 | Service runtime and HTTP surface | Every outbound call (database, broker, any remote dependency) has an explicit timeout. | NEW | Delivered | FR13, NFR4 | 4.3, 4.5, 5.2, 6.4, 7.1, 7.5 |
| NEW-07 | Example bounded context | An `ExampleApiResource` is created through the command path: validated input, command bus, aggregate, domain event. | NEW | Delivered | FR19, FR20 | 6.1, 6.2, 6.3 |
| NEW-08 | Example bounded context | One `ExampleApiResource` is read by id through the query path. | NEW | Delivered | FR21, FR22 | 4.1, 6.2, 6.3 |
| NEW-09 | Example bounded context | An in-process domain-event subscriber writes a structured audit log line. | NEW | Delivered | FR23 | 6.2 |
| NEW-10 | Example bounded context | GraphQL exposes the queries that match the REST reads and the create mutation, and nothing else. | NEW | Delivered | FR24 | 6.3, 7.4 |
| NEW-11 | Shared kernel | A query-bus port has a working in-process adapter, and a missing query handler is an error. | NEW | Delivered | FR27 | 4.1, 4.4, 6.2 |
| NEW-12 | Persistence, cache and messaging | A repository port has a PostgreSQL adapter and a migration for the example resource. | NEW | Delivered | FR31, FR32, FR121 | 1.1, 3.1, 4.5, 6.1, 6.4, 7.2 |
| NEW-13 | Persistence, cache and messaging | A documented command rolls a migration back. | NEW | Delivered | FR33, NFR32 | 4.5, 6.4, 7.2 |
| NEW-14 | Architecture and its enforcement | Architecture rules forbid one bounded context from reaching into another except through the shared kernel. | NEW | Delivered | FR38, NFR18 | 1.3, 2.1, 2.2, 2.4, 3.7, 9.6 |
| NEW-15 | Architecture and its enforcement | A scaffolding command creates a new bounded-context module that compiles and passes the architecture gate. | NEW | Delivered | FR39, NFR18 | 1.3, 2.1, 2.2, 2.4, 3.7, 9.6 |
| NEW-16 | Architecture model (Structurizr) | The Structurizr model is validated in CI. | NEW | Delivered | FR120, NFR18 | 1.3, 2.1, 2.2, 2.4, 3.7, 9.6, 9.10 |
| NEW-17 | Dependency and security checks | An unused-dependency check is a gate. | NEW | Delivered | FR124, NFR9 | 1.1, 1.3, 9.5 |
| NEW-18 | Dependency and security checks | Secrets scanning is a gate. | NEW | Delivered | FR47, NFR10 | 1.1, 3.3 |
| NEW-19 | Dependency and security checks | The production image is scanned for vulnerabilities. | NEW | Delivered | FR48, NFR11 | 3.3 |
| NEW-20 | Formatting, static analysis and complexity | Workflow files, shell scripts and Dockerfiles are linted. | NEW | Delivered | FR43, NFR25 | 1.4, 3.4, 9.10 |
| NEW-21 | Makefile command surface and its tests | A `doctor` target checks host prerequisites. | NEW | Delivered | FR69, NFR37 | 1.2, 1.3, 1.4, 3.1, 9.4 |
| NEW-22 | Makefile command surface and its tests | A one-shot `bootstrap` target prepares a fresh clone. | NEW | Delivered | FR70, NFR34, NFR35 | 1.2, 9.4, 10.3, 10.4, 10.5 |
| NEW-23 | Containers and compose stacks | Image, container and compose project names derive from one project-name variable. | NEW | Delivered | FR92, FR125, NFR38 | 1.1, 1.2, 1.4, 9.8, 9.11, 10.3, 10.5 |
| NEW-24 | Containers and compose stacks | The toolchain is pinned in `rust-toolchain.toml`. | NEW | Delivered | FR86, NFR13 | 1.1, 1.3, 3.1, 7.4, 9.8, 10.1, 10.2, 10.3, 10.5 |
| NEW-25 | CI workflows | CI caches dependencies and build output. | NEW | Delivered | NFR40 | 3.1, 10.1, 10.2 |
| NEW-26 | CI workflows | CI cancels superseded runs and every job has a timeout. | NEW | Delivered | NFR39, NFR40 | 3.1, 10.1, 10.2, 10.5 |
| NEW-27 | CI workflows | Workflows declare least-privilege permissions, and third-party actions are pinned by commit SHA. | NEW | Delivered | FR125, NFR13, NFR14 | 1.1, 1.3, 3.1, 7.4, 9.8, 10.1, 10.2, 10.3, 10.5 |
| NEW-28 | CI workflows | Heavy jobs are skipped for documentation-only changes. | NEW | Delivered | FR94, NFR39 | 10.1, 10.2, 10.5 |
| NEW-29 | CI workflows | The security and CI checks run on a schedule on `main`. | NEW | Delivered | FR95, NFR34, NFR42 | 10.3, 10.5 |
| NEW-30 | AI-assistant guidance | A documentation-drift check verifies the commands cited in the guidance. | NEW | Delivered | FR110, NFR43 | 8.1, 9.9 |
| NEW-31 | AI-assistant guidance | A committed assistant settings file carries an allowlist and a guard on protected files. | NEW | Delivered | FR111, NFR26 | 1.1, 2.3, 8.1, 9.4 |
| NEW-32 | AI-assistant guidance | BMAD and bmalph are integrated following the `core-service` convention: tracked `bmalph/config.json`, command wrappers, Make targets to install and initialise bmalph, the `bmad-autonomous-planning` skill, generated assets kept out of git, planning bundles tracked under `specs/`. | NEW | Delivered | FR112 | 8.3, 8.8 |
| NEW-33 | Human documentation | An architecture overview with decision records lives in the repository and states the statelessness assumptions. | NEW | Delivered | FR117, NFR6 | 7.5, 7.7, 8.5, 8.6, 8.7 |
| NEW-34 | Human documentation | A troubleshooting guide lives in the repository. | NEW | Delivered | FR117 | 8.5, 8.6, 8.7 |
| NEW-35 | Human documentation | An environment-variable reference lists every variable and documents the PHP-to-Rust name mapping. | NEW | Delivered | FR117, NFR45 | 8.5, 8.6, 8.7, 9.9 |
| NEW-36 | Human documentation | A Make-target reference lists every target and documents the PHP-to-Rust name mapping. | NEW | Delivered | FR117, NFR45 | 8.5, 8.6, 8.7, 9.9 |
| NEW-37 | Human documentation | Operations notes cover migrations and rollback and give a health-probe runbook. | NEW | Delivered | FR117 | 8.5, 8.6, 8.7 |
| NEW-38 | Human documentation | A guide explains how to create a service from the template. | NEW | Delivered | FR117 | 8.5, 8.6, 8.7 |
| NEW-39 | Human documentation | An `.editorconfig` file is committed. | NEW | Delivered | FR118 | 1.1, 3.1, 8.5, 10.3 |
| NEW-40 | Human documentation | A dev container definition is committed. | NEW | Delivered | FR118 | 1.1, 3.1, 8.5, 10.3 |
| NEW-41 | Human documentation | A request collection covers the endpoints. | NEW | Delivered | FR118 | 1.1, 3.1, 8.5, 10.3 |
| NEW-42 | Human documentation | Extension seams are documented: a Redis adapter through the cache port, and the tracing and metrics seam. | NEW | Delivered | FR117 | 8.5, 8.6, 8.7 |

### Totals

| Group | Rows |
| --- | --- |
| PHP ids delivered | 185 |
| PHP ids dropped | 11 |
| PHP ids in total | 196 |
| Added ids delivered | 42 |

## Audit problems P1 to P28

Disposition text is from `scope-decisions.md` section 6 as the PRD "Traceability" section states it. A problem with no requirement is a non-goal and is not delivered.

| Problem | Disposition | Requirements | Stories | Not carried |
| --- | --- | --- | --- | --- |
| P1 logging and correlation IDs | In scope | FR10, FR23, NFR31 | 6.2, 7.3, 7.6 | none |
| P2 observability | Partly: liveness/readiness split and dependency-naming failure response in scope; tracing and metrics deferred | FR15, FR18, FR117, FR126 | 4.5, 5.1, 7.3, 7.4, 7.6, 8.5, 8.6, 8.7 | Tracing and metrics exporters, Prometheus endpoint (deferred; the seam is documented, FR117(g)) |
| P3 authentication | Non-goal | none (see "Out of Scope") | none | All of it (see "Out of Scope") |
| P4 secrets | In scope: nothing secret-shaped committed, env reference, secrets scanning | FR3, FR47, FR90, FR117, NFR10 | 1.1, 3.1, 3.3, 7.1, 8.5, 8.6, 8.7 | none |
| P5 hardening | Security headers in scope; rate limiting non-goal | FR11, NFR15 | 4.3, 7.3, 7.4, 7.5 | Rate limiting (non-goal) |
| P6 supply chain | Advisory/licence/source policy, image vulnerability scan, hardened runtime in scope; OSSF Scorecard follow-up | FR46, FR48, FR87, FR91, NFR9, NFR11, NFR12 | 1.1, 1.3, 3.1, 3.3, 7.6, 9.7 | OSSF Scorecard (follow-up) |
| P7 release artifacts | Non-goal | none (see "Out of Scope") | none | All of it (see "Out of Scope") |
| P8 mutation threshold | In scope | FR59, NFR20 | 9.3, 9.6, 10.1 | none |
| P9 coverage floor, one coverage job | In scope | FR58, NFR19, NFR41 | 1.2, 9.2, 10.1, 10.4 | none |
| P10 deterministic strict tests | In scope | FR51, FR55, FR57, NFR29, NFR30 | 1.1, 4.2, 5.1, 6.4, 7.7, 10.3 | A flaky-test policy in the contributing guide (not in scope 3.1) |
| P11 strict static analysis | In scope | FR41, FR123, NFR16 | 1.1, 1.2, 9.8 | none |
| P12 governance | PR-title convention check and no bot pushes in scope; CODEOWNERS and rulesets follow-up | FR96, FR102 | 3.6, 10.1, 10.2, 10.3 | CODEOWNERS and rulesets (follow-up) |
| P13 SECURITY.md | In scope | FR115, NFR46 | 8.4, 8.5, 8.6, 8.7 | none |
| P14 runbooks | Migration rollback command and operations notes in scope | FR32, FR33, FR117, NFR32 | 3.1, 4.5, 6.4, 7.2, 8.5, 8.6, 8.7 | none |
| P15 HA reference | Statelessness, graceful shutdown, limits and restart policy in scope; orchestrator manifests non-goal | FR12, FR91, FR117, NFR5, NFR6 | 3.1, 7.5, 7.6, 7.7, 8.5, 8.6, 8.7, 9.7 | Orchestrator manifests and autoscaling examples (non-goal) |
| P16 resilience | Timeouts on every outbound call in scope; the rest non-goal | FR13, FR17, NFR2, NFR4 | 4.3, 4.5, 5.1, 5.2, 6.4, 7.1, 7.5, 7.6 | Outbox, idempotency keys, chaos tests, kill switches (non-goal) |
| P17 CI cost | In scope | FR94, NFR39-NFR41 | 1.2, 3.1, 9.2, 10.1, 10.2, 10.4, 10.5 | none |
| P18 autonomous maintenance | Scheduled checks and wider Dependabot in scope; auto-merge and stale bot follow-up | FR95, FR99, NFR42 | 10.3 | Auto-merge and stale bot (follow-up) |
| P19 performance budgets | Realistic budgets and uploaded artifacts in scope; baselines follow-up | FR80, FR82, FR84, NFR1 | 3.4, 8.7, 9.1, 10.2, 10.5 | Stored baselines and trend comparison (follow-up) |
| P20 i18n | Non-goal | none (see "Out of Scope") | none | All of it (see "Out of Scope") |
| P21 dev environment | Dev container, `.editorconfig`, no submodules, `doctor` in scope; multi-arch follow-up | FR67, FR69, FR118, NFR37 | 1.1, 1.2, 1.3, 1.4, 3.1, 8.5, 9.4, 9.5, 9.6, 9.7, 9.10, 9.11, 10.3, 10.4 | Multi-architecture builds (follow-up); Windows guidance beyond the WSL 2 statement of NFR37 (not in scope 3.1) |
| P22 in-repo docs | In scope | FR70, FR113, FR116, FR117, NFR45, NFR46 | 1.2, 8.4, 8.5, 8.6, 8.7, 9.4, 9.9, 10.5 | A code of conduct in the repository (not in scope 3.1) |
| P23 scaffolding | In scope | FR38, FR39, FR117 | 2.1, 2.2, 2.4, 8.5, 8.6, 8.7, 9.6 | none |
| P24 contract enforcement | In scope | FR73, FR75, FR86, NFR7, NFR24 | 3.1, 3.2, 7.4, 9.1, 10.2, 10.3 | An API versioning and deprecation policy and an upgrade guide (not in scope 3.1); whether the API version follows the release version is left to the architect (scope-decisions section 4, FR73) |
| P25 debugging tooling | GraphQL explorer in dev and request collection in scope | FR9, FR118 | 1.1, 3.1, 6.3, 7.4, 7.5, 8.5, 10.3 | none |
| P26 hygiene | In scope | FR68, FR100, FR125, NFR13, NFR14, NFR47 | 1.1, 1.3, 3.1, 7.4, 8.4, 9.8, 10.1, 10.2, 10.3, 10.4, 10.5 | none |
| P27 AI-native hardening | In scope | FR44, FR110, FR111, NFR26, NFR43 | 1.1, 2.3, 8.1, 9.4, 9.9, 10.2 | none |
| P28 CQRS exemplars | In scope through the example slice | FR19-FR22, FR27, FR31 | 4.1, 4.4, 6.1, 6.2, 6.3, 6.4 | none |
