# Scope Decisions — rust-service-template bootstrap

Status: binding input for every BMAD planning stage (brief, PRD, architecture,
epics, readiness). Recorded by the orchestrating agent acting as the user's
surrogate, because the run is autonomous. Where a decision is marked
"architect decides", the architecture stage owns it and must record the choice
as a decision with evidence from `research/`.

## 1. The request

> Using BMAD + BMALPH please setup the same template like we did for PHP, but
> with rust ecosystem — https://github.com/VilnaCRM-Org/php-service-template

Interpretation:

- The deliverable is `VilnaCRM-Org/rust-service-template`: the Rust-ecosystem
  counterpart of `php-service-template`, giving a new Rust service the same
  platform concerns the PHP template gives a PHP service.
- The process is BMAD (Analysis, Planning, Solutioning) followed by the bmalph
  Ralph implementation loop. Planning artifacts are tracked in `specs/`.
- "The same template" means parity of **capability and intent**, expressed in
  idiomatic Rust. It does not mean copying PHP mechanisms, and it does not mean
  copying PHP defects.

## 2. Guiding principles

1. **Parity first.** Every capability listed in the `research/php-*.md`
   inventories is either delivered, replaced by a named Rust equivalent, or
   listed as an explicit non-goal with a reason. Nothing is silently dropped.
2. **Keep wire and tooling contracts stable across VilnaCRM templates** where
   they are language-neutral: URL paths, status codes, snapshot paths under
   `.github/`, Make target names that describe a capability, load-test kit,
   commit convention, release and template-sync flows.
3. **Do not replicate known defects.** The inventories list about 190 defects
   and the PHP enterprise audit lists 28 root-cause problems (P1-P28). A
   greenfield template avoids the ones that are free by construction (see
   section 6).
4. **Every gate is real.** A gate that cannot fail, has no threshold, or is not
   run in CI is a defect. `make ci` must pass on a clean checkout and CI must
   run it or its parts.
5. **No dead scaffolding.** No interface without an implementation, no config
   file that no command uses, no dependency nothing imports, no placeholder
   test that asserts nothing.
6. **Verified, not assumed.** Crate and tool versions come from the research
   spikes (verified on rustc 1.97, November 2026), not from memory.
7. **Lean scope.** Anything not listed as in scope below is out of scope for
   this first release. Do not add features because they are easy.

## 3. Product scope

### 3.1 In scope

**Service runtime**

- A single Rust service binary serving plain HTTP on one port. Default
  published port `8081` (the PHP tooling port that load tests, Schemathesis and
  Bats already target). TLS, HTTP/3 and compression are left to the platform
  edge; no Caddy or reverse proxy ships.
- REST API under the `/api` prefix with a code-first OpenAPI 3.1 document, docs
  UI at `/api/docs`, and GraphQL at `/api/graphql`. The GraphQL schema exposes
  only intentionally implemented operations (not the accidental Relay/CRUD
  surface API Platform generated). The interactive GraphQL explorer is enabled
  in the development environment only.
- All error responses are `application/problem+json` (RFC 9457), including
  unknown routes, wrong methods, unsupported media types, validation failures,
  timeouts and panics.
- `GET /api/health`: `204` with an empty body when database, cache and message
  broker respond; `500` problem response otherwise. This keeps the PHP wire
  contract and lets the PHP Gherkin feature be reused. Improvements over PHP:
  probes run concurrently, each under a timeout; all failing dependencies are
  reported; the failure messages are owned by application code and are exactly
  `Database is not available`, `Cache is not working`,
  `Message broker is not available`; the broker probe is non-mutating.
- `GET /ping`: `200` with body `pong`, no dependency access. This is the
  liveness contract used by container healthchecks and CI wait loops in every
  VilnaCRM template. It is not part of the OpenAPI document.
- Structured JSON logs, a request/correlation ID accepted from or generated
  for every request, echoed in the response and attached to every log line.
  Security response headers on every response. Graceful shutdown on SIGTERM.
- Typed configuration from environment variables with fail-fast validation and
  a `check-config` style command (replaces Symfony's requirements check).

**Example bounded context (replaces the inert PHP placeholder)**

- `CompanySubdomain/SomeModule` stays as the literally named placeholder
  context, but it is a small, real, fully tested vertical slice so that every
  pattern the guidance tells contributors to follow exists as working code:
  - create an `ExampleApiResource` (command path: validated input, command bus,
    aggregate, domain event, repository port, PostgreSQL adapter, migration);
  - read one by id and list them with a bounded page (query path);
  - one in-process domain-event subscriber that writes a structured audit log
    line (event path).
- REST path stays `/api/example_api_resources` (PHP parity). GraphQL exposes
  the matching queries and the create mutation.
- No update or delete operations. The slice is an exemplar, not a product.

**Shared kernel**

- Aggregate root with recorded domain events; domain event contract; command
  bus, query bus and event bus ports with working in-process adapters (an event
  may have zero or many subscribers; a missing command or query handler is an
  error); validated UUID value object and generator. Everything shipped is used
  by the example slice or the health check.

**Persistence, cache, messaging**

- PostgreSQL through a repository port and adapter, reversible SQL migrations,
  an explicit `migrate` command, and a documented rollback command. Default
  `DB_VERSION=16` for parity with the PHP stack.
- A cache port with an in-process adapter (the PHP template's effective cache
  is node-local too). Redis is a documented extension through the same port and
  is not shipped.
- AWS SQS client wiring against LocalStack, used by the broker health probe.
  No asynchronous consumer, no outbox. LocalStack is pinned to the tokenless
  community tag the research verified.

**Architecture**

- DDD + CQRS + hexagonal, with `Shared`, `Internal/HealthCheck` and
  `CompanySubdomain/SomeModule`, each split into Domain / Application /
  Infrastructure. Domain is framework-free.
- Boundaries are enforced by a hard gate with zero unassigned code. Rules are
  structural (by layer position), so adding a bounded context needs no edit to
  a locked rule file. Rules also forbid one bounded context from reaching into
  another except through the shared kernel. The mechanism is the architect's
  decision, using `research/rust-quality-gates.md`.
- A scaffolding command creates a new bounded-context module that compiles and
  passes the architecture gate, and a guide explains creating a service from
  the template.
- `workspace.dsl` (Structurizr) is kept, matches the code, has context,
  container and component views, and is validated in CI. Structurizr Lite stays
  in the development stack.

**Quality gates** (each a Make target and a CI check, each able to fail)

- Formatting check (with a separate fix target), strict static analysis with
  warnings denied, complexity limits, architecture boundaries, dependency
  advisories + licences + sources, unused-dependency check, secrets scanning,
  unit tests, integration tests against real PostgreSQL and LocalStack,
  Gherkin end-to-end tests (reusing the PHP health feature), coverage with an
  in-repository floor of 100% lines for testable code, mutation testing that
  fails on surviving mutants, Bats tests of the Makefile including negative
  tests, OpenAPI lint (the PHP Spectral ruleset), OpenAPI breaking-change check
  that fails unless an approval label is present, GraphQL schema diff with the
  same label, Schemathesis against the running service, K6 load tests (smoke on
  pull requests), workflow/YAML/shell/Dockerfile/Markdown linting, and the
  protected-configuration guard running in CI.
- Committed contract snapshots at `.github/openapi-spec/spec.yaml` and
  `.github/graphql-spec/spec`, generated deterministically by CLI commands. CI
  fails when a committed snapshot differs from the generated one. No bot
  commits to pull-request branches or to `main`, other than the release
  identity.
- Randomised test data is seeded and the seed is printed on failure.

**Delivery**

- Multi-stage Dockerfile with a development target and a minimal, non-root,
  hardened production target. Compose files: base, development override,
  production, and an isolated load-test stack that cannot touch development
  data. Production compose reference has resource limits, a restart policy,
  read-only root filesystem, dropped capabilities and no-new-privileges.
- A Makefile as the single command surface, `help` as default goal.
  Capability-named PHP targets keep their names; tool-named PHP targets get
  capability names; the mapping is documented. A `doctor` preflight and a
  one-shot `bootstrap` target exist.
- GitHub Actions with dependency/build caching, concurrency cancellation,
  timeouts, least-privilege permissions, third-party actions pinned by commit
  SHA, heavy jobs skipped for documentation-only changes, and a scheduled run
  of the security and CI checks on `main`.
- Conventional commits in the VilnaCRM form `type(#issue): subject`, enforced
  by a local hook and by a pull-request title check in CI (bots exempt).
- Automated release with CHANGELOG from conventional commits (same flow and
  secrets as PHP), Dependabot for cargo, GitHub Actions, Docker and the K6 npm
  package, both template-sync workflows (PAT and App) with the file-name typo
  fixed, a sync ignore list, and a guard so they do nothing in the template
  repository itself. CodeRabbit config.
- K6 load-test kit ported from PHP (smoke, average, stress, spike; local stack
  and the AWS runner), with results uploaded as CI artifacts and budgets suited
  to a compiled service. Known AWS-flow defects are fixed where the fix is
  local to the scripts.

**AI guidance and documentation**

- `AGENTS.md`, `CLAUDE.md`, `.claude/skills/` (index, usage guide, decision
  guide, and a Rust counterpart for each of the ten PHP skills, named by
  capability), one canonical verification command list equal to `make ci`,
  a documentation-drift check for commands cited in the guidance, and a
  committed assistant settings file with an allowlist and a guard on protected
  files.
- BMAD + bmalph integration following the `core-service` convention: tracked
  `bmalph/config.json`, command wrappers, Make targets to install and
  initialise bmalph, the `bmad-autonomous-planning` skill, generated assets
  kept out of git, planning bundles tracked under `specs/`.
- README, CONTRIBUTING, a complete SECURITY policy, CHANGELOG, architecture
  overview with decision records, troubleshooting, an environment-variable
  reference, a Make-target reference, operations notes (migrations and
  rollback, health-probe runbook), `.editorconfig`, a dev container
  definition, backend-appropriate issue templates, a request collection for
  the endpoints. One licence statement (CC0-1.0) everywhere.

### 3.2 Out of scope for this release (explicit non-goals)

| Item | Reason |
| --- | --- |
| Authentication and authorisation (audit P3) | The PHP template has none; needs an identity-provider decision. |
| Rate limiting (part of P5) | Edge concern; interferes with load and fuzz gates; needs policy input. |
| OpenTelemetry tracing and metrics exporters, Prometheus endpoint (part of P2) | Deferred; the logging and request-ID baseline ships now and the seam is documented. |
| Publishing a container image with SBOM, signature and provenance (P7) | Needs registry and identity decisions. |
| Orchestrator manifests, autoscaling examples (part of P15) | Compose reference only. |
| Outbox, idempotency keys, chaos tests, kill switches (part of P16) | Feature work beyond template parity. |
| Internationalisation (P20) | Declared non-goal; no empty scaffolding ships. |
| Asynchronous message consumer / worker | PHP has no working one. |
| Redis adapter | Documented extension through the cache port. |
| Mercure, Vulcain, HTML templating, sessions | Configured but unused in PHP. |
| Reverse proxy, in-process TLS, HTTP/3 | Platform edge. |
| Update/delete on the example resource | Exemplar stays small. |
| CODEOWNERS, branch-protection rulesets, auto-merge, stale bot | Organisation decisions; documented as follow-ups. |
| Multi-architecture image builds | Follow-up. |
| Stored performance baselines and trend comparison (part of P19) | Needs a storage decision. |

## 4. Naming and compatibility decisions

- Environment variables: use ecosystem-standard names where a tool requires
  them (`DATABASE_URL`, `AWS_REGION`, `AWS_ACCESS_KEY_ID`,
  `AWS_SECRET_ACCESS_KEY`, `AWS_ENDPOINT_URL_SQS`); keep the PHP names for
  compose-level settings (`DB_VERSION`, `DB_NAME`, `DB_USER`, `DB_PASSWORD`,
  `DB_PORT`, `LOCALSTACK_PORT`, `STRUCTURIZR_PORT`, `APP_ENV`). Document the
  PHP-to-Rust mapping. No secret-shaped value is committed; local-only
  development defaults are labelled as such.
- Package identity: `rust-service-template`, licence `CC0-1.0`. Image,
  container and compose project names derive from one project-name variable.
- The API `info.version` in the OpenAPI document must not make the snapshot
  change on every release in a way that breaks the diff gate; the architect
  decides how the version is sourced and records it.
- No git submodules. Bats helper libraries are provisioned without them.
- File and workflow names are reviewed for typos before first sync.

## 5. Execution model

- Make targets must work in three places: inside the development container,
  on a host with the pinned toolchain, and on a GitHub runner. The switch
  between "exec into the container" and "run directly" must be explicit, must
  honour GitHub's `CI=true`, and must be covered by tests. The architect
  decides the default and the variable name.
- The pinned toolchain is declared in `rust-toolchain.toml`.
- Contract, fuzz and load tests run against the production image, not a
  development build.

## 6. Disposition of the PHP audit problems

| Problem | Disposition |
| --- | --- |
| P1 logging and correlation IDs | In scope |
| P2 observability | Partly: liveness/readiness split and dependency-naming failure response in scope; tracing and metrics deferred |
| P3 authentication | Non-goal |
| P4 secrets | In scope: nothing secret-shaped committed, env reference, secrets scanning |
| P5 hardening | Security headers in scope; rate limiting non-goal |
| P6 supply chain | Advisory/licence/source policy, image vulnerability scan, hardened runtime in scope; OSSF Scorecard follow-up |
| P7 release artifacts | Non-goal |
| P8 mutation threshold | In scope |
| P9 coverage floor, one coverage job | In scope |
| P10 deterministic strict tests | In scope |
| P11 strict static analysis | In scope |
| P12 governance | PR-title convention check and no bot pushes in scope; CODEOWNERS and rulesets follow-up |
| P13 SECURITY.md | In scope |
| P14 runbooks | Migration rollback command and operations notes in scope |
| P15 HA reference | Statelessness, graceful shutdown, limits and restart policy in scope; orchestrator manifests non-goal |
| P16 resilience | Timeouts on every outbound call in scope; the rest non-goal |
| P17 CI cost | In scope |
| P18 autonomous maintenance | Scheduled checks and wider Dependabot in scope; auto-merge and stale bot follow-up |
| P19 performance budgets | Realistic budgets and uploaded artifacts in scope; baselines follow-up |
| P20 i18n | Non-goal |
| P21 dev environment | Dev container, `.editorconfig`, no submodules, `doctor` in scope; multi-arch follow-up |
| P22 in-repo docs | In scope |
| P23 scaffolding | In scope |
| P24 contract enforcement | In scope |
| P25 debugging tooling | GraphQL explorer in dev and request collection in scope |
| P26 hygiene | In scope |
| P27 AI-native hardening | In scope |
| P28 CQRS exemplars | In scope through the example slice |

## 7. Artifact format constraints (bmalph)

`bmalph implement` parses the planning artifacts, so these are hard rules:

- `prd.md` must contain second-level headings named exactly
  `## Executive Summary`, `## Functional Requirements`,
  `## Non-Functional Requirements` and `## Product Scope`.
- `architecture.md` must contain `## Tech Stack`. That section must describe
  only the Rust toolchain and crates and must not contain the words `node`,
  `npm`, `typescript`, `python` or `pip` (bmalph detects the stack from it).
- `epics.md` must use `## Epic N: Title` and `### Story N.M: Title` headings,
  a short description after each story heading, then `**Acceptance Criteria:**`
  followed by `**Given** … **When** … **Then** … **And** …` lines. No other
  second- or third-level heading may appear inside a story. Story ids are
  unique and in `N.M` form.
- `implementation-readiness.md` must not contain the strings `no-go`, `no go`
  or `nogo` in any letter case (including inside words such as "no good" or
  "no goals"); bmalph treats them as a blocking verdict.

## 8. Decisions on the research open questions

These settle the open questions raised by the four `research/rust-*.md`
reports. They bind the architecture, epics and implementation stages. Where
"architect decides" appears, the architecture document must record the choice.

### 8.1 Toolchain

- Rust **1.99.1** (current stable, installed on the build machine) pinned in
  `rust-toolchain.toml` with `rustfmt`, `clippy` and `llvm-tools-preview`;
  `rust-version` matches; edition 2024. Builder image
  `rust:1.99.1-slim-trixie` (tag verified to exist). The research spikes ran
  on 1.97.0; anything version-sensitive is re-verified on 1.99.1 during
  implementation.

### 8.2 Workspace, layers and boundary enforcement

- Cargo workspace with **one crate per bounded-context layer**, a binary crate
  as the composition root, and an `xtask` crate. Directory layout mirrors the
  PHP `src/<Context>/<Module>/<Layer>` tree in Rust naming.
- Three layer names, as in PHP: domain, application, infrastructure. Default
  meaning (the architect may refine it with a recorded reason): **application**
  holds use cases (commands, queries, handlers, subscribers) and the
  API-facing adapters (HTTP handlers, DTOs, GraphQL resolvers); 
  **infrastructure** holds driven adapters (persistence, cache, broker, bus
  implementations); **domain** holds aggregates, value objects, events, ports.
  Application and infrastructure each depend on domain and not on each other;
  only the composition root depends on everything. Cargo forbids cycles, so the
  PHP rule that let application and infrastructure use each other is not
  reproduced.
- Boundary gate: `cargo xtask arch` driven by `architecture.toml`, as built in
  the quality-gates spike (layer direction, bounded-context isolation,
  per-layer third-party allow-lists, zero uncovered crates, lint inheritance).
  The checker is template-owned code and sits under the same test, coverage
  and mutation gates as the service.
- Domain crates may use only `thiserror`, `uuid` and `async-trait`, plus at
  most one pure time crate if the architect needs one. No serde, OpenAPI,
  GraphQL, SQL, HTTP or async-runtime crates in domain.

### 8.3 HTTP and API stack

- As verified in `research/rust-http-api-stack.md`: tokio, axum 0.8, tower /
  tower-http, utoipa 6 with the vendored Swagger UI, async-graphql 7.2,
  hand-written RFC 9457 problem type with problem-aware extractors, validator,
  clap + dotenvy configuration with a `check-config` command, tracing with JSON
  output, request ids through tower-http, graceful shutdown.
- OpenTelemetry and Prometheus crates are **not** added (non-goal).
- Liveness is `GET /ping` returning `200` `pong`, not `/api/health/live`.
- OpenAPI stays at 3.1.0. Contract export commands need no database and no
  environment and are byte-stable.

### 8.4 Persistence, cache, messaging

- As verified in `research/rust-data-cache-messaging.md`: sqlx 0.9 (rustls,
  no default features) with `DATABASE_URL`, committed offline metadata and
  `SQLX_OFFLINE=true`, reversible embedded migrations and a `migrate`
  subcommand, `#[sqlx::test]` for isolated integration tests; moka behind a
  `Cache` port; aws-sdk-sqs + aws-config with default features off, explicit
  timeouts and retries; LocalStack pinned to `3.4.0`.
- Broker probe is read-only (for example `ListQueues` with a limit of one);
  the implementation verifies it against LocalStack. Every probe has its own
  timeout and an explicit pool acquire timeout.
- No Testcontainers; integration tests use the compose-provided services.

### 8.5 Quality gates

- As verified in `research/rust-quality-gates.md`: rustfmt with stable keys
  only; the strict clippy policy in `[workspace.lints]` with two passes; the
  `clippy.toml` complexity thresholds; cargo-deny; cargo-shear; cargo-hack;
  cargo-nextest; cargo-llvm-cov gating **lines 100 and functions 100**
  (regions are reported, not gated); cargo-mutants with the exit code as the
  gate. Unit tests live in sibling `tests.rs` modules so the coverage gate
  needs no exclusion flags.
- Mutation testing on pull requests is diff-scoped; a full, sharded run
  happens on `main` and on a schedule. `make mutation-tests` runs the full set.
- Security analysis: CodeQL for Rust plus clippy SARIF upload; secrets
  scanning and a container image vulnerability scan (tools: architect
  decides, each must have a Make target or a documented reason it is CI-only).
- Protected configuration guard: the architect lists the protected files. A
  file that does not exist on the base branch is not a violation (otherwise
  the bootstrap pull request could never pass); a modification of a protected
  file fails unless the pull request carries an approval label.

### 8.6 Tests

- Unit tests with rstest; seeded `fake` data with the seed printed on failure;
  proptest only where it adds real value. Integration tests per adapter crate.
  Gherkin end-to-end tests with cucumber-rs running the PHP
  `health_check.feature` unchanged in process, plus scenarios for the example
  slice; undefined or skipped steps fail the run. No fuzz directory.
- Bats: helpers loaded through `BATS_LIB_PATH` (no submodules). Positive tests
  for the command surface and negative tests for each gate, chosen so the Bats
  CI job stays within about 20 minutes.

### 8.7 Contract, load and delivery tooling

- As verified in `research/rust-testing-delivery.md`: Spectral with the PHP
  ruleset; oasdiff `breaking --fail-on ERR` with the override label
  `approved-breaking-change`; graphql-inspector with the same label;
  Schemathesis with the PHP flags against the production-image stack; stock
  `grafana/k6` image with the PHP scripts (no custom xk6 build); pnpm kept for
  the load-test JavaScript tooling.
- Dockerfile: cargo-chef dependency caching; production image on
  `gcr.io/distroless/cc-debian13:nonroot` pinned by digest, healthcheck through
  the binary's own subcommand (the image has no shell or curl); development
  image with the toolchain, cargo tools, Bats and watchexec. No musl variant.
- Compose: the four PHP file names are kept; the load-test stack runs under
  its own project name; Structurizr uses the unified image (Lite is end of
  life). Whether Structurizr starts by default or behind a profile: architect
  decides.
- PHP paths are kept for language-neutral kits: `features/`, `tests/Load/`,
  `tests/CLI/bats/`, `scripts/`, `.github/openapi-spec/spec.yaml`,
  `.github/graphql-spec/spec`, `workspace.dsl`.

### 8.8 Make execution model

- Make targets run cargo **natively by default** in the current environment
  (host with the pinned toolchain, development-container shell, or CI runner).
  Running through `docker compose exec` is an explicit opt-in. The architect
  names the variable. `make install` (or an equivalent) installs the pinned
  cargo tools on a host; the development image has them baked in.

### 8.9 CI, release and maintenance

- Toolchain and cache through `actions-rust-lang/setup-rust-toolchain`; tools
  through `taiki-e/install-action` with pinned versions; backing services
  through the same compose file as local; every action pinned by commit SHA
  (the research report tabulates current SHAs); actionlint and zizmor as
  gates.
- Release: cocogitto-driven version bump, CHANGELOG, tag and GitHub Release on
  push to `main`, using the same organisation secrets as PHP
  (`VILNACRM_APP_ID`, `VILNACRM_APP_PRIVATE_KEY`); the workflow must exit
  cleanly when the secrets are absent. Dependabot: cargo, rust-toolchain,
  github-actions, docker, docker-compose and the load-test npm package;
  dependency updates must not trigger feature releases.
- Git hooks: a committed `.githooks/` directory activated through
  `core.hooksPath`; commit-msg enforces the VilnaCRM regex; pre-commit runs
  format check, lint, architecture and offline dependency checks; pre-push
  runs unit tests.
- Template sync: `AndreasAugustin/actions-template-sync` pinned by SHA, PAT
  and App flavours, a `.templatesyncignore`, and a guard that skips the job in
  the template repository itself.

### 8.10 Tracked planning bundle

- The planning artifacts are copied to `specs/rust-service-template-bootstrap/`
  (research digest, capability catalog, product brief, PRD, architecture,
  epics, implementation readiness, run summary). The raw research reports stay
  local under `_bmad-output/`.

### 8.11 Decisions on the PRD open questions

- The final PRD (126 FRs, 47 NFRs, 31 gates, decisions PD-1 to PD-31) is the
  requirement baseline. Where the architecture draft predates a PRD decision,
  the architecture is brought in line with the PRD, unless a fact verified by
  running makes the PRD requirement infeasible; in that case the PRD is amended
  and the amendment is listed in the run summary.
- A create whose in-process event publication fails after the save returns
  `201` and logs the failure (PD-18). The shipped subscriber only writes a log
  line.
- A forced end of the shutdown grace period exits non-zero (PD-21).
- The development-only GraphQL explorer may load its assets from one
  third-party origin with exact version pins (PD-27).
- Mutants are judged by the tests of the crate that contains them. Whether
  that includes the crate's integration tests (which need the backing
  services) is the architect's decision; the choice must keep the pull-request
  mutation check inside its time limit on the bootstrap pull request, where
  the diff is the whole repository (sharding is expected).
- The average, stress and spike load profiles stay manual; only smoke gates
  pull requests.
- First manifest version is `0.1.0`. The architect records how a breaking
  change bumps the version before `1.0.0`.
- Only Linux x86_64 is claimed as verified.
- Every pull-request workflow must pass on the bootstrap pull request itself:
  all files are new, the diff against the base is the whole repository, fork
  and organisation secrets may be absent.

## 9. Agent allocation (user instruction, 2026-11-10)

- BMAD planning (analysis, planning, solutioning, including the planning
  validation rounds and the readiness check): agents on the session model.
- bmalph implementation phase: implementation and CI work by implementation
  agents on a faster model tier; verification, checks, reviews and control of
  the Ralph loop by independent agents on a stronger model tier.
- The exact models were named by the user in the session and are not recorded
  in the repository.
