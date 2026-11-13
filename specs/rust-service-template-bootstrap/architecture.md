---
stepsCompleted: [1, 2, 3, 4, 5, 6, 7, 8]
inputDocuments:
  - _bmad-output/planning-artifacts/scope-decisions.md
  - _bmad-output/planning-artifacts/research/capability-catalog.md
  - _bmad-output/planning-artifacts/product-brief.md
  - _bmad-output/planning-artifacts/prd.md
  - _bmad-output/planning-artifacts/research/rust-http-api-stack.md
  - _bmad-output/planning-artifacts/research/rust-data-cache-messaging.md
  - _bmad-output/planning-artifacts/research/rust-quality-gates.md
  - _bmad-output/planning-artifacts/research/rust-testing-delivery.md
  - _bmad-output/planning-artifacts/research/php-app-code.md
  - _bmad-output/planning-artifacts/research/php-tests-and-quality.md
workflowType: 'architecture'
project_name: 'rust-service-template'
user_name: 'BMad'
date: '2026-11-10'
lastStep: 8
status: 'complete'
completedAt: '2026-11-10'
revisedAt: '2026-11-12'
revision: 'aligned with the final PRD (FR1 to FR126, NFR1 to NFR47, G1 to G31, PD-1 to PD-31) after the validation round of three reviewers; the deviation entries of the implementation and of the remediation batches of the FR/NFR review gate (iterations 1 to 4) were applied'
companionDocument: '_bmad-output/planning-artifacts/architecture-delivery.md'
---

# Architecture Decision Document

Core architecture of `rust-service-template`: workspace, layers, runtime design, API contract, data, configuration, quality gates and test architecture. The companion document `architecture-delivery.md` covers the Makefile command surface, containers and compose, CI/CD workflows, release, hooks, load testing, AI guidance and documentation. It depends on the names fixed here: crate names, the binary name and its subcommands, the cargo command of every gate and test level, configuration file names, environment variables and ports.

How to read this document:

- `scope-decisions.md` is binding, and the final PRD (`prd.md`) is the requirement baseline (scope-decisions 8.11). Where this document refines a default of either, the decision says so and gives the reason.
- "Verified" means compiled and run on rustc 1.99.1: for the first version of this document in a throwaway workspace (`spikes/architecture-check`), and in the validation round in a 13-crate skeleton of this design (`spikes/architecture-validate/ws`, 149 source files) and in the probes of the revision (`spikes/core-revision`). The validation section lists what was run and what was not.
- Every name in `code font` is final. Implementers must not rename crates, modules, types, files, commands, variables or strings defined here.

## Project Context Analysis

### Requirements Overview

**Functional scope.** One Rust service binary that gives a new VilnaCRM service the platform baseline the PHP template gives a PHP service:

- HTTP runtime: REST under `/api` with a code-first OpenAPI 3.1.0 document, docs UI at `/api/docs`, GraphQL at `/api/graphql`, `GET /api/health` (204 or 500), `GET /ping` (200 `pong`), problem documents (RFC 9457) for every error.
- Example bounded context `CompanySubdomain/SomeModule`: create, read by id and list `ExampleApiResource` through command bus, query bus, aggregate, domain event, repository port, PostgreSQL adapter and one audit subscriber; the same use cases over GraphQL.
- Shared kernel: aggregate root, domain event, command/query/event bus ports with in-process adapters, UUID value object and generator.
- Persistence, cache and messaging: PostgreSQL with reversible embedded migrations, an in-process cache behind a port, an SQS client used by a read-only probe.
- Architecture enforcement, scaffolding and the full set of Rust-side quality gates (format, lint, complexity, architecture, dependencies, tests at three levels, coverage 100/100, mutation testing).

The final PRD (`prd.md`: FR1 to FR126, NFR1 to NFR47, gates G1 to G31, decisions PD-1 to PD-31) is the requirement baseline (scope-decisions 8.11); the mapping section references its ids.

**Non-functional drivers that shape the design.**

| Driver | Source | Architectural consequence |
| --- | --- | --- |
| Every gate can fail; 100% lines and functions; 0 surviving mutants | scope 3.1, 8.5; NFR19, NFR20 | Sibling `tests.rs` files, thin `main.rs`, fakes under a `tests/` path, a mutation judge that includes adapter integration tests |
| Structural architecture rules, no rule-file edit for a new context | scope 3.1; FR38, FR39 | One crate per bounded-context layer; contexts derived from crate names; `cargo xtask arch` |
| Deterministic contract snapshots that need no environment | scope 8.3; NFR7 | The router is generic over state; documents are built without adapters; sorted maps |
| Wire parity with PHP | scope 3.1, catalog "Contracts to preserve" | Paths, statuses, failure strings, operation ids, tags and metadata fixed in "API Contract" |
| Timeouts on every outbound call, graceful shutdown | scope 6 (P15, P16); NFR2 to NFR5 | Lazy pool with acquire and statement timeouts, SDK timeouts, per-probe deadline, request budget, bounded drain with a non-zero exit when it is cut short |
| Parallel implementers that cannot talk | task constraint | Complete file tree, signatures, exact strings and commands in this document |

### Technical Constraints

- Rust 1.99.1, edition 2024, stable channel only. No nightly-only tool or attribute (`#[coverage(off)]` is unavailable).
- Cargo forbids dependency cycles, so Application and Infrastructure cannot use each other (scope 8.2).
- Domain crates may use only `thiserror`, `uuid` and `async-trait` (scope 8.2). This design needs no time crate in any layer.
- `async fn` in traits is not dyn-compatible, so every port used as `Arc<dyn Trait>` uses `async-trait`. Inside the body of an `#[async_trait]` method llvm-cov records no line counts and clippy reports neither `cognitive_complexity` nor `excessive_nesting` (both measured); rule 21 therefore keeps such bodies to one delegating call.
- tracing's `log` feature is switched on by axum (`tower-log`) and by sqlx-core and cannot be turned off; with it one event macro costs 7 points of cognitive complexity (measured), which shapes rule 17a.
- sqlx compile-time checked queries need committed offline metadata; the build must not need a database.
- LocalStack is frozen at `3.4.0`; the broker probe must work against it.
- No Testcontainers; integration and end-to-end tests use the compose-provided services.
- Non-goals of scope 3.2 stay out: authentication, rate limiting, tracing and metrics exporters, Redis adapter, async consumer, outbox, update and delete operations. Nothing ships that no requirement asks for (scope principle 7).

### Cross-Cutting Concerns

| Concern | Where it is solved |
| --- | --- |
| Error representation | `shared-application::http::problem` (AD-06); one mapping table |
| Request correlation and logging | Middleware stack in `service::http::router` (AD-13) |
| Security headers | `service::http::security_headers` (AD-14) |
| Timeouts and shutdown | Config model plus adapters (AD-07, AD-15) |
| Input validation | Problem-aware extractors and domain value objects (AD-22) |
| Dependency direction and context isolation | `architecture.toml` and `cargo xtask arch` (AD-03) |
| Test doubles and fault injection | `test-support` crate and the composition root's `Adapters` value (AD-23) |
| Contract determinism | `service::openapi`, `service::graphql::schema` (AD-16, AD-17) |

### Scale and Complexity

- Project type: developer tool (service template) whose runtime deliverable is an API backend. Domain complexity is low; breadth is high (196 PHP capability ids plus 42 additions).
- Runtime size: 5 REST operations, 3 GraphQL operations, 1 table, 3 health probes, 1 event subscriber.
- Code size: 13 workspace crates. The validation skeleton, which holds every crate with a reduced `xtask`, has 149 source files, 95 of them production files with 4575 lines; the full architecture checker, guard and scaffolder in `xtask` add roughly a quarter to the first-party code.
- Dependency graph (measured): `Cargo.lock` has 468 packages, 455 of them third-party, for the dependency set of this document (`cargo update --workspace --offline` on the skeleton manifests after the feature changes of this revision; 483 before them). Cold build of the test binaries without debug information about 2 minutes on 4 vCPU (end-to-end target 2 min 07 s, coverage run 1 min 23 s); cold production image build 124 s.

## Starter Template Evaluation

**Primary technology domain:** API backend in Rust, delivered as a repository template.

**Decision: no community starter is used.** The starting point is the set of research spikes that compiled and ran.

| Option | Verdict | Reason |
| --- | --- | --- |
| `cargo generate` templates for axum services | Rejected | None carries the VilnaCRM contracts (paths, failure strings, snapshot paths, Make target names) or a layer-per-crate DDD workspace; adopting one means deleting most of it. The product brief records that no survey of named community templates was run (assumption A-8); the rejection rests on what such a template would have to contain. |
| `loco-rs` (framework on axum) | Rejected | Rails-style models and generators conflict with a framework-free domain (research `rust-http-api-stack.md` D1). |
| Copying the PHP template's structure file by file | Rejected | Parity is of capability and intent (scope 1); PHP mechanisms and defects are not targets. |
| Research spikes as the seed | Selected | Each spike was built, run and shown failing on planted input; versions, configuration files and pitfalls are recorded. |

Spike sources to reuse (paths are relative to the session scratchpad `spikes/` directory named in the research reports):

| Spike | Reuse |
| --- | --- |
| `quality-gates/ws-lock` | `xtask` architecture checker (`arch/{config,model,load,rules,report}.rs`, `cli.rs`), `clippy.toml`, `rustfmt.toml`, `deny.toml`, `.cargo/mutants.toml`, `.config/nextest.toml`, the lint table, sibling `tests.rs` layout, fixture workspace under `xtask/tests/fixtures` |
| `http-api-stack/app` | `problem.rs` (problem type and extractors), `config.rs`, `main.rs` (synchronous entry, graceful shutdown), `http.rs` (middleware order), `kernel.rs` (bus mechanics), `graphql.rs` (SDL export options) |
| `data-cache-messaging/health-spike` | `persistence.rs` (lazy pool, `MIGRATOR`), `cache/moka_cache.rs`, `messaging/sqs.rs` (client factory), `health/{domain,application,infrastructure}.rs`, `.cargo/config.toml` offline guard |
| `testing-delivery/bdd-spike` | cucumber runner with `fail_on_skipped`, step definitions for the PHP health feature |
| `architecture-check` (this document) | Typed bus errors, state-generic `OpenApiRouter`, merged GraphQL roots, custom docs page, GraphQL transport through the JSON extractor |
| `architecture-validate/ws` (validation round) | The 13-crate skeleton of this design that passed QG-01 to QG-04, QG-07 to QG-10, QG-13 to QG-15 and the coverage sequence: manifests, the `Config` collector, `service` commands, the process-level test helper, `test_support::log::LogRecorder`, the reduced `xtask` |
| `core-revision/ws` (this revision) | Forced-shutdown exit status in `command/serve.rs`, JSON configuration errors, `telemetry::init` with a writer, the panic test, item-level suppressions for the extended clippy ban list and for cucumber steps |

**Initialisation.** The first implementation story creates the workspace by hand from the "Workspace and Project Structure" section: root `Cargo.toml`, the tool configuration files, empty crates with manifests, then `cargo generate-lockfile`. No generator command exists to run.

**Decisions the starting point provides:** language and toolchain, workspace layout, lint policy, test layout, gate configuration. Everything else is decided below.

## Tech Stack

This section lists the Rust toolchain, crates and cargo tools only. Container images, contract tools and load tooling are in `architecture-delivery.md`.

Version policy: `[workspace.dependencies]` uses caret requirements at the exact versions below; `Cargo.lock` is committed and every cargo command passes `--locked`. Every version is the one the research reports verified; each was resolved again on this machine with `cargo info` on 2026-11-10 and is still the newest stable release, so no newer patch is adopted. The validation round resolved the whole set with `cargo generate-lockfile` on cargo 1.99.1 in 1.2 s. One deliberate hold: `async-graphql` stays on 7.2.1 because 8.0 is still a release candidate. Two crates are additions to the research lists and were resolved the same way: `toml_edit` 0.25.19 and `rand` 0.10.3.

### Toolchain

| Item | Version | Purpose | Declared in |
| --- | --- | --- | --- |
| Rust (rustc, cargo) | 1.99.1 (stable) | Compiler and build tool | `rust-toolchain.toml`; `rust-version = "1.99.1"` in `[workspace.package]` (the same full version in both places, NFR13; cargo 1.99.1 accepts it, verified) |
| Edition | 2024 | Language edition, resolver 3 | `[workspace.package]` |
| rustfmt | ships with 1.99.1 | Formatting | `rust-toolchain.toml` component |
| clippy | ships with 1.99.1 | Static analysis and complexity | `rust-toolchain.toml` component |
| llvm-tools-preview | ships with 1.99.1 | Coverage instrumentation data | `rust-toolchain.toml` component |

### Runtime crates

| Crate | Version | Features | Purpose | Allowed in layer |
| --- | --- | --- | --- | --- |
| `tokio` | 1.54.1 | `macros`, `rt-multi-thread`, `net`, `signal`, `time`, `sync` | Async runtime, timers, signals | application, composition root; dev-dependency elsewhere |
| `axum` | 0.8.9 | `macros` | HTTP framework, extractors, routing (HTTP/1.1 only; no requirement asks for HTTP/2) | application, composition root |
| `tower` | 0.5.3 | `util` | `ServiceBuilder`; `ServiceExt::oneshot` in tests | composition root; dev-dependency in application |
| `tower-http` | 0.7.1 | `trace`, `request-id`, `catch-panic`, `set-header` | Request id, access log, panic capture, response headers | composition root |
| `utoipa` | 6.0.0 | `axum_extras`, `uuid`, `yaml` | OpenAPI 3.1.0 schema and path derivation, YAML export | application, composition root |
| `utoipa-axum` | 0.3.0 | default | `OpenApiRouter`, `routes!` | application, composition root |
| `utoipa-swagger-ui` | 10.0.2 | `vendored` | Swagger UI assets embedded in the binary | composition root |
| `async-graphql` | 7.2.1 | `default-features = false`; `uuid` | GraphQL schema, execution, SDL export | application, composition root |
| `serde` | 1.0.229 | `derive` | DTO (de)serialisation | application, composition root, tooling |
| `serde_json` | 1.0.154 | default | JSON object of the GraphQL request body (`service`); JSON values of the configuration guard (`xtask`); response bodies in tests | composition root, tooling; dev-dependency in application and `e2e-tests` |
| `validator` | 0.21.0 | `derive` | Declarative DTO validation | application |
| `thiserror` | 2.0.21 | default | Error enums | every layer |
| `async-trait` | 0.1.92 | default | Dyn-compatible async ports | every layer |
| `uuid` | 1.29.2 | `default-features = false`; `std`; `v7` added by `shared-infrastructure` | UUID parsing, formatting, generation | every layer |
| `futures-util` | 0.3.34 | `default-features = false`; `alloc` | `join_all` for concurrent probes | application |
| `humantime` | 2.4.0 | default | Duration parsing in configuration; RFC 3339 formatting of `SystemTime` | application, composition root |
| `tracing` | 0.1.44 | default | Logging facade; log capture in `test-support` | application, infrastructure, composition root, `test-support` |
| `tracing-subscriber` | 0.3.23 | `json`, `env-filter` | JSON log output, level filter | composition root |
| `clap` | 4.6.7 | `derive`, `env` | Subcommands, flags bound to environment variables | composition root, tooling |
| `dotenvy` | 0.15.7 | default | Loads `.env` before the runtime starts | composition root |
| `sqlx` | 0.9.0 | `default-features = false`; `runtime-tokio`, `tls-rustls-aws-lc-rs`, `postgres`, `macros`, `migrate`, `uuid` | PostgreSQL access, checked queries, embedded migrations, `#[sqlx::test]` | infrastructure, composition root |
| `moka` | 0.12.16 | `future` | In-process cache adapter | infrastructure |
| `aws-config` | 1.13.1 | `default-features = false`; `rt-tokio`, `default-https-client` | AWS SDK configuration and credential chain | infrastructure |
| `aws-sdk-sqs` | 1.120.0 | `default-features = false`; `rt-tokio`, `default-https-client`; `test-util` as dev-dependency | SQS client | infrastructure |

### Test crates

| Crate | Version | Features | Purpose | Allowed in |
| --- | --- | --- | --- | --- |
| `rstest` | 0.28.0 | default | Parameterised cases and fixtures | dev-dependency in every layer |
| `proptest` | 1.12.0 | default | Property tests for value objects and validation equivalence | dev-dependency in domain and application |
| `fake` | 5.1.0 | `uuid` (gates `fake::uuid::UUIDv7`, verified) | Random test data | `test-support` |
| `rand` | 0.10.3 | default | Seeded generator (same major as `fake` uses) | `test-support` |
| `cucumber` | 0.23.0 | default | Gherkin runner | `e2e-tests` |
| `http-body-util` | 0.1.5 | default | Collecting response bodies in tests | dev-dependency in application, composition root, `e2e-tests` |
| `aws-smithy-mocks` | 0.3.0 | default | SDK-level stubs for the SQS probe unit tests | dev-dependency in infrastructure |

### Tooling crates (`xtask`)

| Crate | Version | Purpose |
| --- | --- | --- |
| `cargo_metadata` | 0.23.1 | Reads the resolved workspace graph for the architecture gate |
| `toml` | 1.2.1 | Parses `architecture.toml`, `protected-config.toml` and manifests |
| `toml_edit` | 0.25.19 | Format-preserving edit of the root manifest by the scaffolding command |

`xtask` also uses `serde`, `serde_json`, `thiserror` and `clap` from the tables above.

### Cargo tools (gates and developer commands)

| Tool | Version | Purpose | Configuration file |
| --- | --- | --- | --- |
| `cargo-nextest` | 0.9.151 | Test runner for unit and integration tests | `.config/nextest.toml` |
| `cargo-llvm-cov` | 0.9.1 | Coverage with an enforced floor | command-line flags (see "Quality Gates") |
| `cargo-mutants` | 27.1.0 | Mutation testing | `.cargo/mutants.toml` |
| `cargo-deny` | 0.20.2 | Advisories, licences, bans, sources | `deny.toml` |
| `cargo-shear` | 1.14.0 | Unused dependencies | none |
| `cargo-hack` | 0.6.46 | Builds each crate on its own | none |
| `sqlx-cli` | 0.9.0 | Database lifecycle, migration authoring, offline metadata | `DATABASE_URL` |

Crates and features considered and not adopted, with the reason: `async-graphql-axum` (the GraphQL route uses the problem-aware JSON extractor, verified, so transport errors are problem documents and one dependency fewer ships); the `graphiql` feature of `async-graphql` (its page loads unpinned assets from two origins, AD-18); the `http2` feature of `axum`, the `sensitive-headers` feature of `tower-http` and the `output-junit` feature of `cucumber` (no requirement uses them); `time` and `chrono` (no timestamp is stored or serialised; `std::time::SystemTime` plus `humantime` formatting is enough); `opentelemetry*` and `metrics*` (non-goal); `redis` (non-goal); `testcontainers` (scope 8.4); `tokio-util` (no background worker exists).

## Core Architectural Decisions

### Decision Priority Analysis

- **Critical (block implementation):** AD-01 to AD-07, AD-11, AD-16, AD-18, AD-23, AD-24.
- **Important (shape the code):** AD-08 to AD-10, AD-12 to AD-15, AD-17, AD-19 to AD-22, AD-25 to AD-27.
- **Deferred (not in this release, seam named):** tracing and metrics exporters (seam: the `TraceLayer` span in `service::http::trace`), Redis adapter (seam: `Cache` port), transactional unit of work and outbox (seam: AD-08), authentication (seam: a layer in `service::http::router`), asynchronous consumer (seam: `shared_infrastructure::messaging::sqs_client`), GraphQL depth and complexity limits (seam: `service::graphql::schema::builder`).

### AD-01 Workspace and crate granularity

- **Decision.** One virtual Cargo workspace with 13 crates: nine layer crates (three bounded contexts times three layers), the composition root `service` (library plus binary `service`), `test-support` (test doubles) and `e2e-tests` (Gherkin suite), plus `xtask` (tooling). Layer crates are named `<context>-<layer>` and live at `crates/<group>/<context>/<layer>`; the shared kernel has no group and lives at `crates/shared/<layer>`. Workspace members are listed explicitly in the root `Cargo.toml`. No crate declares cargo features.
- **Rationale.** A layer can only use what its manifest lists, so the compiler enforces direction and Cargo rejects cycles (research `rust-quality-gates.md` D4). The tree mirrors PHP `src/<Context>/<Module>/<Layer>` in kebab-case (scope 8.2). Explicit members are required: cargo 1.99.1 fails on a glob such as `crates/*/*/*` when a matching directory has no manifest (`crates/shared/domain/src`), verified.
- **Alternatives rejected.** A single crate with modules (boundaries become syntactic; the best module-level checker missed 2 of 7 planted violations). Glob members (fail as described). A fourth `presentation` layer (scope 8.2 fixes three layer names).
- **Consequences.** The scaffolding command must add three paths to `[workspace].members` (AD-26). Crate names must be unique across contexts, so a context directory name is unique in the workspace. `service` and `xtask` keep neutral names so a downstream service never renames a crate that `architecture.toml` or a workflow refers to.

### AD-02 Meaning of the three layers and allowed dependency directions

- **Decision.** Scope 8.2 is adopted without change to the directions: Domain depends on nothing but other Domain crates; Application and Infrastructure each depend on Domain and never on each other; only `service` depends on all three. Two refinements are recorded:
  1. *Every port that Infrastructure implements lives in a Domain crate*, including technical ports (`Cache`, `Clock`, `UuidGenerator`, the three bus ports, `HealthProbe`). Reason: Infrastructure cannot see Application, so a port placed there could not be implemented. PHP also keeps its bus and UUID factory interfaces in `Shared/Domain`.
  2. *Application crates keep use cases and API adapters in separate top-level modules.* `command`, `query`, `subscriber`, `service` and `error` hold use cases and must not import `axum`, `utoipa` or `async_graphql`; `http` and `graphql` hold the API-facing adapters and may. Reason: the scope default puts both in one crate; the module split keeps use cases transport-free and unit-testable. The split and the module names are a rule in "Implementation Patterns" and are checked in review; no gate enforces module names, so a context that needs another top-level module edits no protected file.

| Element | Layer | Crate and module |
| --- | --- | --- |
| Aggregates, value objects, domain events, domain errors | Domain | `<context>-domain::{aggregate, value_object, event, error}` |
| Repository ports | Domain | `<context>-domain::repository` |
| Bus ports (`CommandBus`, `QueryBus`, `EventBus`), `Command`, `Query`, handler and subscriber traits | Domain | `shared-domain::{bus, event}` |
| Technical ports (`Cache`, `Clock`, `UuidGenerator`) | Domain | `shared-domain::port` |
| Health probe port and health value objects | Domain | `health-check-domain::{port, value_object}` |
| Commands, queries and their handlers, event subscribers, application errors | Application | `<context>-application::{command, query, subscriber, error}` |
| Health orchestration (`HealthService`) | Application | `health-check-application::service` |
| HTTP handlers, request and response DTOs, `#[utoipa::path]` annotations, per-context router | Application | `<context>-application::http` |
| GraphQL object and input types, resolvers | Application | `<context>-application::graphql` |
| Problem type, problem-aware extractors, fallbacks, timeout middleware, GraphQL error helpers | Application | `shared-application::{http, graphql}` |
| In-process bus implementations | Infrastructure | `shared-infrastructure::bus` |
| Cache adapter, clock, UUID generator, pool factory, `MIGRATOR`, SQS client factory | Infrastructure | `shared-infrastructure::{cache, clock, id, persistence, messaging}` |
| Persistence adapters (SQL, row mapping, error translation) | Infrastructure | `<context>-infrastructure::persistence` |
| Health probes (PostgreSQL, cache, SQS) | Infrastructure | `health-check-infrastructure::probe` |
| Configuration, CLI, wiring, router assembly, middleware order, OpenAPI document, merged GraphQL schema, docs UI, `/ping`, telemetry | Composition root | `service` |

- **Rationale.** The scope default keeps the three PHP layer names and gives the strongest isolation Cargo can express: the API-facing code and the driven adapters cannot name each other, and the Domain names neither. The table above removes every placement question an implementer could have.
- **Alternatives rejected.** Classic hexagonal direction (Infrastructure depends on Application, HTTP adapters in Infrastructure): contradicts the scope default and PHP's placement of API Platform processors in Application. Ports in Application: not implementable under the adopted directions.
- **Consequences.** Application crates carry `axum`, `utoipa` and `async-graphql`; their allow-list is long but explicit. A handler cannot run SQL and an adapter cannot call a use case: neither crate can name the other.

### AD-03 Bounded-context isolation and the architecture gate

- **Decision.** `cargo xtask arch` reads `architecture.toml` and `cargo metadata --locked` and enforces eight rules. The spike checker is extended in three ways so that rules are structural: (1) a crate's bounded context is the text matched by `*` in its layer pattern (`some-module-domain` belongs to `some-module`); the `[[context]]` tables of the spike are removed; (2) a context may depend on itself and on the contexts listed in `[contexts].shared` (the shared kernel); (3) rule `arch::path` requires a layered crate to sit at `crates/[<group>/]<context>/<layer>`.

| Rule | Fails when |
| --- | --- |
| `arch::uncovered` | A workspace crate matches no layer pattern; or a local crate (no registry or git source) below the workspace root that is not a member of the workspace and is used by the dependency graph, reported as `` `<crate>` is not assigned to any layer: it is a local crate that is not a member of the workspace `` with the hint ``add the crate to `members` of the workspace and name it `<context>-<layer>`, or add its name to the `crates` of a [[layer]]``. A path dependency outside the workspace root directory stays a third-party crate |
| `arch::ambiguous` | A workspace crate matches more than one layer pattern |
| `arch::layer` | A crate depends on a workspace crate whose layer is not in `depends_on`; for a dev-dependency, not in `depends_on` or `dev_depends_on` |
| `arch::context` | A layered crate depends on a layered crate of another context that is not in `[contexts].shared` |
| `arch::external` | A crate depends directly on a third-party crate outside `external`; for a dev-dependency, outside `external` and `dev_external` |
| `arch::transitive` | A crate in `forbidden_transitive` is reachable through non-dev edges |
| `arch::path` | A layered crate's manifest directory is not `crates/[<group>/]<context>/<directory>` |
| `arch::lints` | A crate's manifest lacks `[lints] workspace = true` |

  Exit status: 0 clean, 1 violations, 2 the check could not run. Output: compiler-style diagnostics naming the crate, both ends of the forbidden edge and a hint; GitHub annotations when `GITHUB_ACTIONS` is set.
- **Rationale.** Adding a context creates crates that already match `*-domain`, `*-application` and `*-infrastructure`; no line of `architecture.toml` changes (FR38, FR39). Cycles are rejected by Cargo before the check starts.
- **Alternatives rejected.** Explicit context tables (need a protected-file edit per context). Module-level checkers `archaven`, `cargo_pup`, `arch_test_core` (misses, nightly, AGPL: research D4). A rule `arch::module` with a per-layer list of allowed top-level module names (in the first version of this document; removed: FR38 does not ask for it, and a context that needed another module name would have to edit the protected rule file, against K-10).
- **Consequences.** The checker is template-owned code under the same test, coverage and mutation gates. The gate does not see `std` use inside a Domain crate; the wall-clock and socket APIs of `std` are banned by clippy (`disallowed-methods`, NFR28), and domain purity beyond that is a review rule. Verified in the validation round: a reduced checker (rules uncovered, layer, context, external, transitive, path) exits 0 on the 13-crate skeleton with dev-dependencies checked against the union of both lists and exits 1 on three planted violations; `cargo tree -e normal,build` shows 0 hits for every `forbidden_transitive` entry.

### AD-04 Composition root

- **Decision.** `crates/service` is the only place where adapters meet use cases. `main.rs` is three lines and calls `service::run()`. The library exposes `bootstrap::Adapters` (every driven port as `Arc<dyn Trait>`: repository, health probes, event bus, clock, UUID generator), `Adapters::connect(&Config)` (production wiring), `Adapters::with_probe` (replace one health probe), `bootstrap::event_bus()` (the in-process event bus with every subscriber registered) and `bootstrap::router(adapters, &Config)` (command and query buses, handlers, state, router, middleware). Per-context routers are generic over the application state: `router<S>() -> OpenApiRouter<S> where <Context>State: FromRef<S>`. `service::http::AppState` derives `FromRef` and holds one state struct per context.
- **Rationale.** One function builds the complete application from a value in which any port can be replaced, which is what tests and the Gherkin world need (FR5). The state-generic router lets the OpenAPI document be produced from the same route definitions that serve traffic, without constructing a single adapter (verified: `api_router().into_openapi()` needs no state value).
- **Alternatives rejected.** A dependency-injection container or discovery by reflection (PHP mechanism, not reproduced: catalog CAP-04). Routers that take state by value (the export commands would need adapters, hence a database URL). A separate `cache` field in `Adapters` (nothing but the cache probe uses the cache; the field would be read by no code).
- **Consequences.** Registration of handlers is explicit code in `bootstrap::router` and registration of subscribers is explicit code in `bootstrap::event_bus`. A unit test in `service` builds the production wiring with fakes and dispatches every command and query once, so a missing registration fails a test. How a test replaces each port (FR5): the repository, the clock, the UUID generator and the event bus by assigning the public field; a health probe with `with_probe`; the cache by `with_probe(Arc::new(CacheProbe::new(Arc::new(StubCache::failing()))))`, because the cache probe is the only user of the cache port; the command and query buses are built from the handlers and are replaced in the handler unit tests of the Application crates (`FakeCommandBus`, `FakeQueryBus`).

### AD-05 Bus dispatch

- **Decision.**
  - *Typing.* `Command` and `Query` are traits with an associated `Error` type (and `Output` for queries) and a `const NAME`. The ports `CommandBus` and `QueryBus` are dyn-compatible and type-erased (`TypeId` plus `Box<dyn Any + Send>`); an inherent `impl dyn CommandBus { async fn dispatch<C: Command>(..) -> Result<(), BusError<C::Error>> }` (and `ask` for queries) restores static types at the call site. Verified on 1.99.1.
  - *Registration.* `InMemoryCommandBus::new().register(handler)?` and `InMemoryQueryBus::new().register(handler)?` insert into a `HashMap<TypeId, Arc<dyn Erased…Handler>>`; a second handler for the same message returns `DuplicateHandler { message }`, which stops start-up (FR28). `InMemoryEventBus::new().subscribe(subscriber)` appends to a `Vec` per event type. Buses are immutable after construction.
  - *Missing handler.* Dispatching a command or query without a handler returns `BusError::NotRegistered { message: C::NAME }`; the HTTP layer maps it to a 500 problem and logs it.
  - *Events.* An event with no subscriber is success. Every subscriber of every published event is invoked in registration order, synchronously within `publish`; a failing subscriber does not stop the others; `publish` returns `EventBusError` with the failure of every failing subscriber in invocation order, each carrying the event name and the event id (FR26, PD-19). The bus does not log: the publisher decides what a failure means (AD-08).
  - *Commands return no data.* The caller generates the aggregate id with `UuidGenerator` before dispatch and reads the result through a query.
- **Rationale.** Keeps the PHP bus seam (catalog CAP-27, CAP-28, NEW-11) while giving handlers typed errors, so no call site downcasts a boxed error. Fixes the PHP defects: missing adapter, one subscriber kept silently, zero subscribers treated as an error.
- **Alternatives rejected.** Injecting `Arc<dyn CommandHandler<C>>` directly (drops the bus seam the guidance is written around). `tokio::sync::broadcast` as event bus (subscriber errors cannot reach the publisher). A boxed `dyn Error` in the port signature (forces downcasts in every adapter). Returning only the first subscriber failure (FR26 requires every failure).
- **Consequences.** Registration is checked at run time, covered by the wiring test of AD-04. One allocation per dispatch. If the erased error cannot be downcast to `C::Error` (a bus implementation bug) the typed helper reports `NotRegistered`. Verified in the validation round: typed `dispatch` and `ask` through `Arc<dyn CommandBus>` and `Arc<dyn QueryBus>` compile and run with the in-memory adapters, including `DuplicateHandler` and `NotRegistered`.

### AD-06 Error model, end to end

- **Decision.** Four levels, each with one conversion:
  1. *Domain errors*: `thiserror` enums per concept (`InvalidUuid`, `InvalidPageRequest`, `InvalidExampleApiResource`, `RepositoryError`, `CacheError`). No HTTP knowledge.
  2. *Application errors*: one enum per bounded context (`ExampleApiResourceError`), the `Error` type of that context's commands and queries.
  3. *Bus errors*: `BusError<E>` wraps the application error or reports a missing handler.
  4. *Problem*: `shared_application::http::Problem` implements `IntoResponse` with `content-type: application/problem+json`. Each application error implements `From<Error> for Problem` in its own crate; `shared-application` provides `From<BusError<E>> for Problem where E: Into<Problem>`, and conversions from axum rejections and `validator::ValidationErrors`.
  Every 5xx problem is created through `Problem::internal(&error)`, which logs the error chain once at `ERROR` and returns a body without internal detail. Problem `type` is `about:blank` when the status says everything, and a relative URI reference for the two problem types that carry extension members.

| Situation | Status | `type` | `title` | `detail` | Extension |
| --- | --- | --- | --- | --- | --- |
| Malformed JSON body; path or query parameter that cannot be parsed as its type (an id that is not a UUID, a page value that is not a 32-bit integer) | 400 | `about:blank` | `Bad Request` | parser message from axum | none |
| Unknown route | 404 | `about:blank` | `Not Found` | `No route matches the request path.` | none |
| `ExampleApiResource` not found | 404 | `about:blank` | `Not Found` | `ExampleApiResource <id> was not found.` | none |
| Method not supported on the route | 405 | `about:blank` | `Method Not Allowed` | absent | `allow` response header |
| Body larger than `HTTP_MAX_BODY_BYTES` | 413 | `about:blank` | `Payload Too Large` | message from axum | none |
| Missing or unsupported request media type | 415 | `about:blank` | `Unsupported Media Type` | message from axum | none |
| Body with an unknown field, a missing field or a wrong JSON type | 422 | `about:blank` | `Unprocessable Entity` | message from axum | none |
| JSON body on the GraphQL route that is not a GraphQL request | 422 | `about:blank` | `Unprocessable Entity` | message from axum (not a JSON object) or `The request body is not a GraphQL request.` | none |
| Constraint violation in body or query; domain validation error | 422 | `/problems/validation-failed` | `Unprocessable Entity` | `The request failed validation.` | `violations[]` |
| At least one health probe failed | 500 | `/problems/dependency-unavailable` | `Internal Server Error` | failure messages joined with `; ` | `failures[]` |
| Handler error, missing bus handler, repository failure, panic | 500 | `about:blank` | `Internal Server Error` | absent | none |
| Request exceeded `HTTP_REQUEST_TIMEOUT` | 503 | `about:blank` | `Service Unavailable` | `The request exceeded its time budget.` | none |

  The status codes are exactly those of PD-1: 400, 404, 405, 413, 415, 422, 500 and 503. No 409 exists: an aggregate id is generated by the service, so a duplicate id cannot be reached through the API; if a repository reports one, it is an internal error (500). A failed event publication after a successful save is not an error of the request (AD-08).
- **Rationale.** One table that every handler, test and document follows; clients can branch on status and, for the two rich cases, on a stable `type`. Relative references avoid a host name in the contract and stay valid in downstream services.
- **Alternatives rejected.** The `problem_details` crate (its schema impl targets utoipa 5). `about:blank` everywhere (clients cannot tell a validation problem from another 422). Absolute `type` URLs under the repository (hard-codes the template's name).
- **Consequences.** GraphQL operation errors are not problem documents (AD-18). `Problem` stays under 128 bytes (`&'static str` for `type` and `title`; `size_of::<Problem>() == 112`, measured) so `clippy::result_large_err` does not fire on `Result<_, Problem>`; `#[schema(value_type = String)]` on the two `&'static str` fields compiles with utoipa 6.0.0 and yields string schemas with `required: [type, title, status]` (verified).

### AD-07 Configuration model

- **Decision.** `clap` derive defines the CLI and binds every setting to an environment variable. Every setting is declared `#[arg(long, env = "NAME", hide_env_values = true)]`, so `--help` names each variable and never prints a value, and every setting is an `Option<String>`, so clap never rejects a value. `Config::from_args(ServeArgs) -> Result<Config, ConfigErrors>` parses and validates all fields, including cross-field rules, and collects every error; each error names the *environment variable* and never echoes a value. `serve` and `check-config` call this function; `migrate` validates its own subset (the database variables and `DATABASE_WAIT_TIMEOUT`) with the same collector, in `MigrateSettings::from_args` of `config.rs` (the collector is private to that file). A configuration error exits with status 2 and is reported in the output format of the command (NFR31):
  - `check-config` and `migrate` print one line `<VARIABLE>: <reason>` per problem on stderr;
  - `serve` writes one JSON log entry `invalid configuration` at ERROR per problem, with the fields `variable` and `reason`, on stdout. For this the JSON subscriber is installed exactly once, after `Config::from_args` has returned: with `LOG_LEVEL` when the configuration is valid, with `info` when it is not.

  `dotenvy::dotenv()` runs first in a synchronous `main`, before the tokio runtime exists. Process environment wins over `.env`. The export subcommands and `healthcheck` take no configuration struct. `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` are bound in `ServeArgs` only to validate them as a pair (both set or both unset; the error names the missing variable); their values are never stored in `Config` and never printed; the AWS SDK reads the two environment variables itself (AD-10), so both arguments carry `hide = true` and are supplied through the environment.
- **Rationale.** Scope 8.3 fixes clap plus dotenvy; FR4 requires a message that names every missing or invalid variable. Plain clap stops at the first invalid value and names the flag, not the variable (research D6). Loading `.env` mutates the environment, which is unsound once threads exist. Without `hide_env_values` clap prints the current value of each bound variable in `--help`, for example `[env: DATABASE_URL=postgres://app:s3cret-password@db/app]`; with it the line ends `[env: DATABASE_URL]` (verified on clap 4.6.7).
- **Alternatives rejected.** Typed clap fields with value parsers (first error only; echoes invalid values). The `config` and `figment` crates (more machinery than an environment-only service needs). Plain-text configuration errors for `serve` (NFR31: every line `serve` writes is JSON). Rejecting configured timeouts above the PRD ceilings (NFR4 and PD-15 bound the defaults, not what an operator may configure; FR13 requires the values to be configurable).
- **Consequences.** `ServeArgs` and `Config` list each setting twice; a unit test asserts that every `ServeArgs` field is consumed. `serve --help` lists every variable and no value. A clap usage error (unknown command or flag) is printed by clap as text with exit status 2 before any command runs; it is not output of the `serve` command. Verified in this revision: `serve` with two invalid variables and a missing one writes three JSON lines `invalid configuration` to stdout, nothing to stderr, and exits 2.

### AD-08 Persistence and transactions

- **Decision.** `sqlx` with compile-time checked `query!`/`query_as!` macros and committed offline metadata. One `PgPool`, created lazily (`connect_lazy_with`) so the service starts while the database is down. Pool options: `max_connections = DATABASE_MAX_CONNECTIONS`, `acquire_timeout = DATABASE_ACQUIRE_TIMEOUT`; every connection sets `statement_timeout = DATABASE_STATEMENT_TIMEOUT`. Repository adapters own SQL, row structs and error translation; Domain types carry no sqlx derive. Each repository method is one statement and therefore atomic; one command changes one aggregate. Events are published after the save succeeds and are not part of a database transaction.
- **Rationale.** Research D1. The PHP template has no entity, so a unit of work would be scaffolding without a user.
- **Alternatives rejected.** SeaORM, Diesel (research D1). A transactional port now (no multi-aggregate use case exists; scope principle 5).
- **Consequences.** A publication failure after a successful save does not fail the command: `CreateExampleApiResourceHandler` logs one entry `event publication failed` at ERROR per failing subscriber with `event_name`, `event_id` and `error`, returns `Ok(())`, and the client receives 201 (PD-18, FR20(e)). Domain-event delivery is at most once; the only shipped subscriber writes a log line. A service that needs guaranteed delivery adds an outbox (non-goal here); the architecture overview and the operations notes state this (FR117(a)). The lazy pool is what lets `serve` start and answer `/ping` while the database is down (FR126, PD-20).

### AD-09 Cache port

- **Decision.** `shared_domain::port::Cache` with two methods, `get` and `set` (TTL mandatory). Adapter `shared_infrastructure::cache::MokaCache`: `moka::future::Cache<String, Entry>` bounded to 10 000 entries, per-entry TTL through `Expiry`. The only user is the cache health probe.
- **Rationale.** PHP parity: the effective PHP cache is local to one container; a remote cache is a documented extension through the same port (scope 3.1). No `delete` and no `get_or_set` ship because nothing would call them (scope principle 5).
- **Alternatives rejected.** Redis by default (non-goal). Using the cache in the example slice (not in scope; NFR6 requires that no response depends on cache content).
- **Consequences.** With an in-process adapter the probe cannot realistically fail; it is kept for wire parity and becomes meaningful with a remote adapter.

### AD-10 Broker client and the read-only probe call

- **Decision.** `shared_infrastructure::messaging::sqs_client(&SqsSettings) -> aws_sdk_sqs::Client` builds the client from `aws_config::defaults(BehaviorVersion::v2026_11_10())` with explicit region, optional endpoint URL, connect timeout `SQS_CONNECT_TIMEOUT`, attempt and operation timeout `SQS_OPERATION_TIMEOUT` and `RetryConfig::standard().with_max_attempts(SQS_MAX_ATTEMPTS)`. Credentials always come from the SDK's default chain (`AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY` locally, roles in AWS). The probe calls `client.list_queues().max_results(1).send()` and ignores the returned list.
- **Rationale.** `ListQueues` reads account state and creates nothing (scope 8.4; PHP defect APP:D-14). It needs only `sqs:ListQueues`. No publisher port ships: PHP routes no message and scope excludes a consumer.
- **Alternatives rejected.** `CreateQueue` (mutating). `GetQueueUrl` (needs a queue that nothing else uses). Hand-rolled SigV4 HTTP (re-implements the SDK).
- **Consequences.** Measured on LocalStack 3.4.0 over five cold starts, with and without `SERVICES=sqs`: the health endpoint of LocalStack answers after 3.2 to 6.3 s; the first `list_queues().max_results(1)` takes 0.11 to 0.15 s and later calls 4 to 7 ms, so the 2 s default has a wide margin (the 2.3 s cold call of the research was not reproduced); the compose healthcheck still warms SQS (delivery document). A dead endpoint fails in under 3 s with the specified timeouts. `health-check-infrastructure/tests/integration/sqs_probe.rs` keeps both cases under test.

### AD-11 Health check orchestration

- **Decision.**
  - *Port and names.* The port is `HealthProbe` (the HTTP spike's `ReadinessProbe` and the data spike's `DependencyHealthCheck` are both retired). `Dependency` has three variants with wire identifiers `database`, `cache`, `message_broker`.
  - *Concurrency.* `HealthService::check` runs all probes concurrently with `futures_util::future::join_all`; total latency is that of the slowest probe.
  - *Timeouts.* Each probe future is wrapped in `tokio::time::timeout(HEALTH_PROBE_TIMEOUT)` (default 3 s). Adapters have their own shorter limits: pool acquire 2 s, SQS operation 2 s. Configuration validation requires `HEALTH_PROBE_TIMEOUT >= DATABASE_ACQUIRE_TIMEOUT` and `>= SQS_OPERATION_TIMEOUT`, so a refused connection is reported with its real cause and the outer deadline only catches a dependency that hangs. A timed-out probe counts as failed.
  - *Aggregation.* The result is a `HealthReport` holding every failure in registration order: database, cache, message broker.
  - *Failure strings* (owned by `Dependency::failure_message`, identical in every environment): `Database is not available`, `Cache is not working`, `Message broker is not available`. The HTTP spike's lower-case `database is unavailable` is not used.
  - *Response.* No failure: `204`, empty body. Otherwise `500` problem with `type` `/problems/dependency-unavailable`, `detail` equal to the failure messages joined with `; `, and `failures` listing each failing dependency. The cause (error chain or `timed out after 3s`) is logged at `WARN` with the dependency and is never returned.
  - *Probes.* Database: `SELECT 1`. Cache: `set("health_check", b"ok", 60 s)` then `get` and compare. Broker: AD-10.
  - *Probe tests (FR54).* `CacheProbe` and `SqsProbe` have unit tests with doubles (`StubCache`, `aws-smithy-mocks`). `PostgresProbe` wraps a concrete `sqlx::PgPool`, which has no test double: its success case and its unreachable case are integration tests (`postgres_probe.rs`), and the handling of a failed and of a timed-out probe is unit-tested once, in `HealthService`, with `StubProbe::failing` and `StubProbe::hanging`. This reading of FR54 is recorded in "Architecture Validation Results".
  - *Wiring.* The handler calls `HealthService` directly, not through the query bus: a health check must not depend on bus registration. This is the only handler allowed to bypass the buses.

  Body with one failing dependency:

  ```json
  {"type":"/problems/dependency-unavailable","title":"Internal Server Error","status":500,"detail":"Database is not available","failures":[{"dependency":"database","message":"Database is not available"}]}
  ```

  Body with several failing dependencies:

  ```json
  {"type":"/problems/dependency-unavailable","title":"Internal Server Error","status":500,"detail":"Database is not available; Message broker is not available","failures":[{"dependency":"database","message":"Database is not available"},{"dependency":"message_broker","message":"Message broker is not available"}]}
  ```

- **Rationale.** Scope 3.1 and FR14 to FR17. The reused Gherkin feature asserts that the body contains each exact string.
- **Alternatives rejected.** Sequential probes (latency is the sum). First failure only (hides the rest). 503 (PHP wire contract is 500).
- **Consequences.** Adding a probe means one adapter plus one line in `Adapters::connect`; the handler does not change. Readiness does not fail during shutdown: no draining flag ships (not in scope). Verified in the validation round: the 8 combinations of failing dependencies (the healthy case and the 7 non-empty ones) and a hanging probe under tokio's paused clock (exactly 3 s of virtual time) pass; a live `GET /api/health` against PostgreSQL and LocalStack answered 204 in 68 ms. Verified in this revision: with the database and the SQS endpoint on a dead port the spawned service answers `/ping` at once and `/api/health` with the 500 problem naming both in 2.0 s (NFR2, FR126).

### AD-12 Liveness

- **Decision.** `GET /ping` returns `200`, `content-type: text/plain; charset=utf-8`, body `pong`. It is registered in `service::http::ping`, outside `/api`, outside the OpenAPI router, and touches no dependency. The spike's `/api/health/live` does not exist. The container healthcheck runs `service healthcheck`, which requests `/ping` on the address of `APP_HOST` and the port `APP_PORT` (the loopback address of the same family when `APP_HOST` is `0.0.0.0` or `::`, and `127.0.0.1` when it is unset).
- **Rationale.** Scope 3.1 and 8.3; `/ping` is the liveness contract of every VilnaCRM template.
- **Alternatives rejected.** Liveness under `/api/health/live` (contradicts scope).
- **Consequences.** `/ping` passes through the same middleware stack, so it carries the request id and security headers and appears in the access log.

### AD-13 Request id and logging

- **Decision.** Header `x-request-id`. An inbound value is accepted when it matches `^[A-Za-z0-9._-]{1,128}$`; otherwise it is dropped and a UUID is generated (`tower_http::request_id::MakeRequestUuid`). The id is echoed on every response, including 404, 405, panic and timeout responses, and recorded on the request span `http.request` with fields `http.request.method`, `http.route` (the route template, or `unmatched`), `url.path` and `request_id`. Logs are JSON, one object per line on stdout, through `tracing-subscriber` (`.json().flatten_event(true).with_current_span(true).with_span_list(true)`: `span` is the innermost open span and `spans` lists every open span, outermost first, so an entry written inside a span that a request opened, such as the `SQS.ListQueues` span of the AWS SDK at `debug` level, still carries the `http.request` span with the request id). The level filter is `LOG_LEVEL`. No pretty format exists (NFR31). The line schema is in "Observability and Operations".
- **Rationale.** Scope 3.1 (P1). Sanitising the inbound id keeps hostile input out of logs and response headers.
- **Alternatives rejected.** Trusting any inbound value. A second, human-readable log format (NFR31 requires every line to be JSON).
- **Consequences.** Log lines written outside a request (start-up, shutdown) carry no request id.

### AD-14 Security headers

- **Decision.** Four headers on every response, set by `tower_http::set_header::SetResponseHeaderLayer::if_not_present`, so a handler may supply a page-specific value:

| Header | Default value |
| --- | --- |
| `x-content-type-options` | `nosniff` |
| `x-frame-options` | `DENY` |
| `referrer-policy` | `no-referrer` |
| `content-security-policy` | `default-src 'none'; frame-ancestors 'none'` |

  Page-specific `content-security-policy` values, each `frame-ancestors 'none'` plus only the source directives the page needs (FR11):

| Page | Constant | Value |
| --- | --- | --- |
| Documentation page `GET /api/docs` | `service::http::docs::DOCS_CONTENT_SECURITY_POLICY` | `default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; frame-ancestors 'none'` |
| Development-only explorer `GET /api/graphql` | `service::graphql::handler::EXPLORER_CONTENT_SECURITY_POLICY` | `default-src 'none'; script-src https://unpkg.com 'unsafe-inline'; style-src https://unpkg.com 'unsafe-inline'; img-src data:; font-src data:; connect-src 'self'; frame-ancestors 'none'` |

  The explorer policy names the one third-party origin the page loads from, `https://unpkg.com` (FR9, PD-27). Why each directive is needed: the page has one inline start-up script and GraphiQL sets inline styles (`'unsafe-inline'`); the GraphiQL style sheet embeds its fonts and three images as `data:` URLs (checked in `graphiql.min.css` of the pinned version: 34 `data:font/*` and 3 `data:image/png` URLs, no other URL); the page sends its operations to `/api/graphql` (`connect-src 'self'`). The implementation opens the page in a browser once; a directive that turns out to be missing or unneeded is changed here and in the constant in the same pull request.
- **Rationale.** FR9, FR11, NFR13, NFR15. `Strict-Transport-Security` and cross-origin headers belong to the platform edge because the service speaks plain HTTP (PRD decision PD-3).
- **Alternatives rejected.** A security-headers crate (four lines of layer configuration do not justify a dependency). Vendoring the explorer's scripts (about 1.5 MB of third-party files in the repository for a development-only page; PD-27 allows one pinned third-party origin). A policy of `frame-ancestors 'none'` alone for the explorer (in the first version of this document; it names no origin and restricts nothing).
- **Consequences.** The explorer needs internet access in the browser. The documentation page loads nothing from a third-party origin (FR8). The service emits no CORS headers.

### AD-15 Request budget, body limit and graceful shutdown

- **Decision.** Every request runs under `HTTP_REQUEST_TIMEOUT` (default 10 s) through `shared_application::http::middleware::request_timeout`; on expiry the response is the 503 problem. Request bodies are limited by `axum::extract::DefaultBodyLimit::max(HTTP_MAX_BODY_BYTES)` (default 65 536); the extractors turn the rejection into the 413 problem. On SIGTERM or SIGINT the server stops accepting connections and lets in-flight requests finish for at most `SHUTDOWN_GRACE_PERIOD` (default 10 s). When all have finished it logs `stopped` and exits with status 0. When the period ends with a connection still open it logs `grace period elapsed, forcing shutdown` at WARN, drops the remaining connections and exits with status 1 within 2 s (PD-21, FR12, NFR5). Mechanism: `command::serve` spawns `axum::serve(..).with_graceful_shutdown(..)`, waits for the signal, then awaits the server task under `tokio::time::timeout(SHUTDOWN_GRACE_PERIOD)`; on expiry it returns the outcome `Forced`, the runtime is shut down with `Runtime::shutdown_timeout(1 s)`, which drops every remaining connection task, and `service::run` returns `ExitCode::FAILURE`. The status is returned from `main` (no `std::process::exit`), so coverage data is still written. Unwinding stays enabled (`panic = "unwind"`), so a panicking handler fails one request (`CatchPanicLayer`) and not the process.
- **Rationale.** FR12, FR13, NFR4, NFR5; all defaults are below the PRD ceilings (30 s request budget, 30 s grace), and the default grace period (10 s) is not shorter than the default request budget (10 s), so with the defaults no request is cut (PD-21). A cut request must be observable to the platform, hence the non-zero status.
- **Alternatives rejected.** `tower_http::timeout::TimeoutLayer` (empty body, not a problem document). `tower_http::limit::RequestBodyLimitLayer` (its 413 is `text/plain`). 504 for the budget (the service is not a gateway). Exit status 0 after a forced end of the grace period (in the first version of this document; contradicts PD-21).
- **Consequences.** `axum::serve` has no header-read timeout or connection limit; the service must sit behind a load balancer or ingress when exposed to the internet (documented in the architecture overview). A connection on which a client has started a request and never finishes it keeps the drain open until the grace period ends; the process-level tests use exactly this to reach both paths without a slow endpoint (a request head sent without its final blank line). Verified in this revision on the skeleton: with `SHUTDOWN_GRACE_PERIOD=1s` and such a connection the process logs the WARN line and exits with status 1 after 1.25 s; when the client completes the head after the signal it receives `200 pong` and the process exits 0; new connections are refused within 10 ms of the `shutdown signal received` line; an idle service exits 0 at once.

### AD-16 OpenAPI document, API version and service identity

- **Decision.** `service::openapi::api_router()` builds one `OpenApiRouter<AppState>` from `ApiDoc::openapi()` and the per-context routers nested under `/api`. Static metadata (contact name and e-mail, licence, tags, server `/`) is in the `ApiDoc` derive. Three values are set in code after the derive: `info.title = concat!(env!("CARGO_PKG_DESCRIPTION"), " API")`, `info.description = "This API provides endpoints to manage <description> data within VilnaCRM."` and `info.contact.url = env!("CARGO_PKG_REPOSITORY")`; both package fields are inherited by the `service` crate from `[workspace.package]` (`description = "Rust Service Template"`, `repository`). `info.version` is the constant `service::openapi::API_VERSION = "1.0.0"`: the version of the API contract, not of the release. It changes only in a pull request that changes the contract: the major number when a breaking change is approved with the `approved-breaking-change` label, the minor number when operations or fields are added.
- **Rationale.** A release bumps `[workspace.package].version` on every merge; if `info.version` followed it, every release would change the committed snapshot and trip the equality gate (scope 4, FR73). A contract version that moves with the contract makes snapshot diffs meaningful. Taking the title and repository URL from the root manifest gives a downstream service one place to state its identity.
- **Alternatives rejected.** `CARGO_PKG_VERSION` (changes every release). The major version only (still couples the snapshot to the release commit, which skips CI). A literal title in Rust source (a second place to rename).
- **Consequences.** The rule for `API_VERSION` is a convention checked in review; the breaking-change gates still detect the change itself. utoipa's `preserve_order` features stay off; OpenAPI stays at 3.1.0.

### AD-17 Contract export determinism

- **Decision.** `service openapi-export` prints `OpenApi::to_yaml()` and `service graphql-export` prints `Schema::sdl_with_options(SDLExportOptions::new().sorted_fields().sorted_arguments().sorted_enum_items().prefer_single_line_descriptions().use_space_ident().indent_width(2))`. Both run before configuration is read, need no environment variable, no `.env` content, no database and no network, and write to stdout or to `--output <path>`. Validation violations and every other list in a response are sorted before serialisation.
- **Rationale.** NFR7 (20 byte-identical generations). utoipa emits sorted maps by default; the SDL options sort everything else. Verified on 1.99.1 in the validation round: both exports run under `env -i` with exit status 0, and 20 in-process generations of each are byte-identical; operation ids, tags, `default` responses, `3.1.0`, the metadata and the five schema names match "API Contract".
- **Alternatives rejected.** Exporting from a running service (needs the stack). Post-processing with a formatter (a second tool that can drift).
- **Consequences.** `service/tests/integration/export.rs` generates each snapshot 20 times and compares bytes. The YAML uses unindented block sequences; the snapshot directories are excluded from YAML linting (delivery document).

### AD-18 GraphQL schema and transport

- **Decision.**
  - *Schema.* Each context's Application crate exposes `#[derive(Default)]` structs with `#[Object]` impls (`ExampleApiResourceQuery`, `ExampleApiResourceMutation`). `service::graphql::schema` merges them with `#[derive(MergedObject)]` into root types named `Query` and `Mutation` and builds `Schema<Query, Mutation, EmptySubscription>`. Resolvers read `SomeModuleState` from the schema data and call the same commands and queries as the REST handlers. No subscription, no batch request, no multipart upload.
  - *Transport.* The route `POST /api/graphql` is an ordinary axum handler. It deserialises the body with `ApiJson<serde_json::Map<String, serde_json::Value>>`, so a body that is not a JSON object (`[]`, `5`) is the 422 problem; it requires a member `query` that is a string and converts the object into `async_graphql::Request` with `serde_json::from_value`; a missing or non-string `query` or a failed conversion is the 422 problem with detail `The request body is not a GraphQL request.` (FR9). The response is `Json<async_graphql::Response>` (`content-type: application/json`). Other transport failures (malformed JSON 400, wrong media type 415, oversized body 413) are problem documents from the extractor.
  - *Operation errors* are returned with status 200 in `errors[]` with a stable `extensions.code` (PD-5, FR24(e)): `VALIDATION_FAILED` with `extensions.violations` for a validation failure, for an `id` that is not a UUID and for a page value outside its bounds; `INTERNAL_SERVER_ERROR` with the message `Internal server error` for everything else a resolver can fail with. A query text that is not a valid document for the schema, which includes an `Int` argument outside the 32-bit range, returns 200 with the library's `errors` entry (FR24(f)).
  - *Explorer.* `GET /api/graphql` serves the committed page `crates/service/src/graphql/explorer.html` (embedded with `include_str!`) when `APP_ENV=dev`; otherwise the method router answers with the 405 problem. The page loads exactly four assets, all from `https://unpkg.com`, each with an exact version and an `integrity` attribute, and has no favicon link:

    | Asset | URL | Size | `integrity` |
    | --- | --- | --- | --- |
    | React | `https://unpkg.com/react@18.3.1/umd/react.production.min.js` | 10 751 bytes | `sha384-DGyLxAyjq0f9SPpVevD6IgztCFlnMF6oW/XQGmfe+IsZ8TqEiDrcHkMLKI6fiB/Z` |
    | React DOM | `https://unpkg.com/react-dom@18.3.1/umd/react-dom.production.min.js` | 131 835 bytes | `sha384-gTGxhz21lVGYNMcdJOyq01Edg0jhn/c22nsx0kyqP0TxaV5WVdsSH1fSDUf5YJj1` |
    | GraphiQL script | `https://unpkg.com/graphiql@3.9.0/graphiql.min.js` | 978 022 bytes | `sha384-8NGfVj4CVlqHajlZj+bPJT4thxPMHMJYn7DWK2CvtopLd02E7qsPHazniBYvVjOO` |
    | GraphiQL style sheet | `https://unpkg.com/graphiql@3.9.0/graphiql.min.css` | 410 559 bytes | `sha384-QMux00XgRtwRLSYIY3kw2rj1ovk5AuuliAchk+HSQbqdbGFnz9GYuqIlOqxhwCE2` |

    Sizes and digests were computed for this revision from the npm tarballs `react-18.3.1.tgz`, `react-dom-18.3.1.tgz` and `graphiql-3.9.0.tgz`, whose files unpkg serves unchanged. Every `script` and `link` element carries `crossorigin="anonymous"`. The inline start-up script is `ReactDOM.createRoot(document.getElementById('graphiql')).render(React.createElement(GraphiQL, { fetcher: GraphiQL.createFetcher({ url: '/api/graphql' }) }));`. The response carries `EXPLORER_CONTENT_SECURITY_POLICY` (AD-14).
- **Rationale.** FR9, FR24, PD-5, PD-27, NFR13. The merged roots are the seam for a second context: one more field in the tuple struct. Using the problem-aware JSON extractor makes transport errors uniform and removes `async-graphql-axum` (verified: malformed body returns `400 application/problem+json`). Deserialising straight into `async_graphql::Request` is not enough: its `query` field has a serde default, so `{}`, `[]` and `{"foo":1}` are accepted and answered with 200 and a parse error (verified on 7.2.1), where FR9 requires 422. GraphiQL 3.9.0 was checked in its npm package: the bundle `graphiql.min.js` exposes `GraphiQL.createFetcher`, starts no web worker (so the policy needs no `worker-src`), and its peer range covers React 18, whose 18.3.1 release still ships UMD files.
- **Alternatives rejected.** `async-graphql-axum` extractor (its rejection is `text/plain` unless wrapped; adds a crate). Relay connections and node interface (the accidental PHP surface, catalog CAP-19). The crate's `GraphiQLSource` (its template loads `react@18`, `react-dom@18` and `graphiql@4` without exact versions from unpkg.com and a favicon from a second origin, graphql.org; NFR13 requires exact pins and FR9 one origin). `u32` page arguments (a negative value fails inside the library without `extensions.code`, verified; AD-22). `limit_depth` and `limit_complexity` on the schema (in the first version of this document; no requirement asks for query limits, and a limit error would be a fourth error class without a stable code; listed as deferred).
- **Consequences.** The `MergedObject` derive expands to code that triggers `clippy::useless_let_if_seq` (observed on 1.99.1); `service/src/graphql/schema.rs` carries a module-level `#![expect(clippy::useless_let_if_seq, reason = "...")]`, the one module-level suppression of the workspace (rule 15). A unit test of the explorer page asserts the pins (NFR13): every `src` and `href` URL of the page starts with `https://unpkg.com/` and contains `@<major>.<minor>.<patch>/`, every such element has an `integrity` attribute, and the policy header names exactly that origin. An upgrade of a pinned asset changes the URL and the digest together.

### AD-19 Docs UI

- **Decision.** `GET /api/docs` returns `200 text/html` directly: a 15-line page committed as `crates/service/src/http/docs.html` that loads Swagger UI assets by absolute path. `GET /api/docs/openapi.json` returns the document as JSON. `GET /api/docs/{*file}` serves the embedded Swagger UI files through `utoipa_swagger_ui::serve(file, config)` (crate feature `vendored`, no `axum` feature); an unknown file is the 404 problem. `/api/docs/` with a trailing slash is not a route.
- **Rationale.** The catalog marks `GET /api/docs -> 200` as preserved; the crate's own axum integration answers the bare path with a 303 redirect (observed). Verified: the custom page and asset route return 200 and the page-specific policy survives the header layer.
- **Alternatives rejected.** Accepting the redirect (changes a preserved contract). Scalar, Redoc, RapiDoc (load scripts from a CDN without a version pin).
- **Consequences.** The page names five asset files of Swagger UI 5; a major upgrade of the crate must re-check them (covered by an end-to-end scenario that fetches the page and by a unit test that resolves each asset the page references).

### AD-20 Migration policy

- **Decision.** Plain SQL migrations in `migrations/` at the workspace root, as reversible pairs `<UTC timestamp>_<snake_case_name>.up.sql` and `.down.sql`, embedded with `sqlx::migrate!` in `shared_infrastructure::persistence::MIGRATOR`. The service never migrates on `serve`. `service migrate` applies all pending migrations after a bounded wait for the database; `service migrate revert` reverts the most recently applied one. Both write plain text (NFR31: only `serve` writes JSON). An applied migration is never edited. Every migration belongs to one bounded context and touches only that context's tables.
- **Rationale.** Scope 3.1, FR32, FR33, NFR32, NFR33. Embedding means the production image needs neither `sqlx-cli` nor the directory.
- **Alternatives rejected.** Migrating on start (PHP defect APP:D-30). `refinery`, `diesel_migrations` (a second database client). One migration directory per context (one database has one history). A `migrate status` command (in the first version of this document; FR2 lists no such command).
- **Consequences.** `shared-infrastructure/build.rs` prints `cargo:rerun-if-changed=../../../migrations` so a new file is re-embedded (verified: with the build script, adding, editing or removing a migration recompiles `shared-infrastructure`; without it a new migration is not re-embedded). Adding a migration requires regenerating `.sqlx/` when queries change.

### AD-21 Identifiers, time and clock

- **Decision.** Aggregate and event ids are UUIDs generated by `UuidV7Generator` (`uuid::Uuid::now_v7()`), so ids are time-ordered and the primary-key index gives creation order. `shared_domain::Uuid` accepts any syntactically valid UUID. Time is `std::time::SystemTime`, obtained only through the `Clock` port; `SystemClock` is the adapter. No timestamp is stored or returned by the API in this release; the only consumer is `DomainEvent::occurred_at`, which the audit subscriber formats with `humantime::format_rfc3339_micros`.
- **Rationale.** The PRD fixes three resource fields (`id`, `name`, `description`); a time crate in the domain allow-list would have no user. UUIDv7 gives the list endpoint a stable newest-first order without an extra column or index.
- **Alternatives rejected.** UUIDv4 plus a `created_at` column (a column nothing reads). The `time` crate in Domain (permitted by scope 8.2, not needed).
- **Consequences.** A context that later stores timestamps follows the rule in "Data Architecture" and adds a time crate to the allow-lists in a reviewed change to `architecture.toml`.

### AD-22 Validation strategy

- **Decision.** Two lines of defence with one set of numbers. (1) Request DTOs derive `validator::Validate`; `ValidatedJson<T>` and `ValidatedQuery<T>` run it and return all violations in one 422 problem, sorted by `propertyPath` then `code`. The same DTO validation is called by the GraphQL resolvers. (2) Domain value objects re-check the invariants in their constructors and return `InvalidExampleApiResource`; the application error maps it to the same 422 shape. OpenAPI schema constraints (`minLength`, `maxLength`, `pattern`, `minimum`, `maximum`) are written next to the validator attributes on the DTO. A property test asserts that the DTO and the value objects accept exactly the same strings. (3) The boundary between 400 and 422 for the page parameters (FR22, PD-1): `page` and `itemsPerPage` are signed 32-bit integers in the DTO and in the GraphQL arguments, so every integer outside the bounds, negative ones included, reaches the validator and is a 422 (`VALIDATION_FAILED` over GraphQL), and only a value that is not a 32-bit integer is a parse error (400 over REST; an invalid document over GraphQL). `page` has no upper bound: a page beyond the last is 200 with an empty array.
- **Rationale.** FR7, FR20, FR22, FR24(e). "Parse, don't validate" in the domain; declarative, complete error reporting at the edge. Schemathesis fails when schema and behaviour drift (research D3).
- **Alternatives rejected.** Validation only in the domain (one violation at a time). `garde`, `axum-valid` (research D5). Unsigned DTO fields (`page=-1` would be a 400 and, over GraphQL, a library error without a code). An upper bound of 100000 for `page` (in the first version of this document; the PRD defines none and requires an empty page instead).
- **Consequences.** Limits appear as literals in three places (schema attribute, validator attribute, domain constant); the property test and Schemathesis guard them. Schemathesis 4.33.1 with `--checks all --phases=examples,coverage` passed 111 of 111 generated cases on 4 of 4 operations of the skeleton in 9 s, with the `pattern` constraint in place; that run used the unsigned page parameters with a maximum, so the conformance run is repeated on the signed parameters during implementation (listed under "Risks").

### AD-23 Test doubles and fault injection

- **Decision.** Test doubles are hand-written fakes in the workspace crate `test-support` at `tests/support/`, which depends on Domain crates only and is used as a dev-dependency by Application, Infrastructure, `service` and `e2e-tests`. Domain crates define the few fakes they need inside their own `tests.rs` files (a dev-dependency back onto `test-support` would compile the Domain crate twice). The Gherkin suite builds the production wiring with real adapters through `Adapters::connect`, then, for a `Given the <dependency> is not …` step, replaces that dependency's probe with `test_support::probe::StubProbe::failing(dependency)` through `Adapters::with_probe` before the request is sent. The world builds a new application instance for every request, in process, and dispatches with `tower::ServiceExt::oneshot`. Every scenario has its own PostgreSQL database ("Test database strategy"), so a scenario sees only the rows it wrote (FR55(d)).
- **Rationale.** FR55, FR56, CAP-TQ-11, CAP-TQ-12: fault injection by composing with a failing port, no leak between scenarios. Building a fresh instance per request also shows statelessness; the test NFR6 asks for is in the integration suite (`service/tests/integration/bootstrap.rs`).
- **Alternatives rejected.** Mocking libraries (`mockall`): generated code is hard to hold to 100% coverage and adds a proc-macro to every port. Stopping containers from tests (slow, flaky; scope 8.4 excludes Testcontainers). Fakes inside each crate behind a cargo feature (feature unification hides missing dependencies).
- **Consequences.** The end-to-end suite needs PostgreSQL and LocalStack, like the integration suite. Files named `tests.rs` and everything under a directory named `tests` are absent from the coverage report without any flag (verified with cargo-llvm-cov 0.9.1) and are excluded from mutation by the globs of `.cargo/mutants.toml`, so `test-support` and `e2e-tests` need no exclusion entry. The end-to-end suite does not count towards coverage (AD-24); everything it exercises is also exercised by a unit or integration test of the crate that owns the code.

### AD-24 Coverage and mutation scoping

- **Decision.**
  - *Coverage.* `cargo llvm-cov` measures the unit and integration suites in one nextest run and gates `--fail-under-lines 100 --fail-under-functions 100`; regions are reported only. The end-to-end suite is not instrumented and does not count (PD-28, FR58(a), NFR19). No `--ignore-filename-regex`, no `--exclude`. Absent from the report without any flag, and therefore excluded by construction (verified with cargo-llvm-cov 0.9.1): files named `tests.rs`, everything under a directory named `tests` (`crates/*/tests/`, `xtask/tests/`, `tests/support/`, `tests/e2e/`) and `build.rs`. Everything else counts, including `main.rs` of both binaries (executed by subprocess tests) and `xtask`.
  - *Coverage exclusion record (NFR19).* The record is the `COVERAGE_*` command group in the protected fragment `make/gates.mk` of the delivery document: an exclusion is an `--ignore-filename-regex` alternative added to those commands with a comment that gives the reason. The first release has none.
  - *Mutation.* `cargo mutants --workspace` with `.cargo/mutants.toml`, which is the mutation scope record of FR59(b). Scope: every production source file of every workspace crate including `xtask`; excluded by glob, each with its reason in the file: `**/main.rs`, `**/build.rs`, `**/tests.rs`, `**/tests/**`. Each mutant is judged by the unit *and* integration tests of the crate that contains it (cargo-mutants tests the mutated package), run with nextest profile `mutants`. The exit code is the gate: 0 pass, 2 a mutant survived, 3 a mutant timed out; a timed-out mutant counts as surviving (FR59(c)); a mutant that does not compile is not counted. Pull requests and `make ci` run the changed-code mode (`--in-diff` with the diff against the merge base of the base ref); the full run happens on `main` and on the schedule, sharded with `--shard k/n`, where k counts from 0 to n-1 (cargo-mutants rejects k = n) and the delivery document sizes n as one shard per 150 mutants, at most 8 (DD-09).
- **Rationale.** Scope 8.5 and 8.11, NFR19, NFR20. Refinement of the research configuration: the research judged mutants with unit tests only, which leaves every SQL adapter and probe unjudged because their behaviour can only be observed against PostgreSQL or LocalStack. Including the package's integration tests makes "0 surviving mutants over all production source" achievable without excluding adapters (verified: mutants in `some-module-infrastructure/src/persistence.rs` were caught by the crate's integration tests against PostgreSQL).
- **Alternatives rejected.** Excluding adapter files from mutation (weakens the gate). Running the whole workspace test suite per mutant (slow; cross-crate tests would mask missing tests in the mutated crate). Counting the end-to-end run towards the floor (in the first version of this document; contradicts PD-28 and runs the scenarios twice). Terminating a hanging test from the nextest profile so that the mutant counts as caught (in the first version of this document; contradicts FR59(c)).
- **Consequences.**
  - Mutation testing needs the `database` and `localstack` services and a prepared test database, like integration tests.
  - `CARGO_TARGET_DIR` must be unset when cargo-mutants runs (research pitfall P1), and so must `CARGO_BUILD_WARNINGS`: with `build.warnings = deny` a mutant that replaces a function body leaves its parameters unused, the build fails, cargo-mutants classes the mutant as unviable and a missing test is never reported (verified on cargo-mutants 27.1.0: `2 caught, 5 unviable`, exit 0 with the variable set; `MISSED`, exit 2 without it; `--cap-lints` does not help). The command lines of QG-17 and QG-18 therefore start with `env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS`.
  - A mutant excluded as equivalent is an `exclude_re` entry with a comment, in the protected scope record.
  - Code that only the end-to-end suite would exercise does not exist: `Adapters::connect` runs in every spawned `serve` process and in `service/tests/integration/bootstrap.rs`; `Adapters::with_probe` and the event-bus replacement have unit tests in `bootstrap/tests.rs` ("Coverage and mutation").
  - Two blind spots of the tools shape the code rules. Inside the body of an `#[async_trait]` method llvm-cov records only the signature and the closing brace, and clippy reports no complexity finding (an `#[async_trait]` method with 39 nested `if`s produced none; the same body in a plain function scored 79 against the limit of 8); rule 21 moves the logic into an inherent `async fn`. A tracing event written over several lines leaves its field lines at count 0 even when it is emitted; rule 17a keeps each event on one line.
  - A function whose only effect is a log line yields the mutant "replace with ()", which survives unless a test observes the log; `test_support::log::LogRecorder` exists for that (rule 6a). Verified: with one assertion through the recorder `replace log_failure with ()` goes from MISSED to caught.
  - Measured on the skeleton (4 vCPU, no debug information): 354 mutants in 60 files; a shard of four (89 mutants) takes 9 min 0 s, mean 5.2 s per mutant (service 6.5 s, shared-application 1.9 s, domain 1.4 s, adapter crates about 3 s); the baseline is 92 s of build plus 2 s of tests. The complete code base (full `xtask`, about 800 to 1000 mutants) extrapolates to 60 to 80 min unsharded; CI runs one shard per 150 mutants, at most 8 (delivery DD-09), about 15 min per shard, inside the 45 min limit of the changed-code check on the bootstrap pull request, whose diff is the whole repository (scope 8.11).

### AD-25 Protected configuration and guard semantics

- **Decision.** One file, `protected-config.toml`, is the single list used by the guard, the guidance and the assistant settings. `cargo xtask config-guard --base <ref>` (default `origin/main`) checks it; the rules and the list are in "Quality Gates". A protected item is a whole file, every file below a directory, or a named section of a file that otherwise changes freely (FR44(b), PD-25). Summary:
  - a protected item that does not exist on the base ref is not a violation;
  - a modified, deleted or renamed protected file is a violation, and so is a protected file whose entry kind changed (regular, executable, symbolic link) or that has a symbolic link in a directory above it; a protected directory that holds files at the base and is now a link or a file is a violation too;
  - the `[workspace.lints]` table of the root `Cargo.toml` is compared as a parsed TOML value, and the budgets of the shipped load scenarios in `tests/Load/config.json.dist` are compared as parsed JSON values at listed JSON pointers; the rest of both files is not protected; a missing file, table or pointer is a modification;
  - once the base ref holds at least one protected item, a new file that would override a protected configuration file for part of the tree (a nested `clippy.toml`, `rustfmt.toml`, `rust-toolchain.toml`, `.cargo/config.toml` and the other names of `override_names`) is a violation (FR44(c));
  - a missing required directory is always a violation;
  - the list itself is read from `protected-config.toml` on the base ref when the base holds that file, and from the working tree only when it does not, so a change to the list takes effect once it is merged; a required directory that a change removes from the list and from the tree is still reported as missing until the base no longer lists it;
  - an unresolvable base ref is exit status 2.

  Modifications and overrides are waived, and listed as approved, when `CONFIG_CHANGE_APPROVED=true`, which CI sets only when the pull request carries the label `approved-config-change`. Without the approval the guard prints the name of that label (FR44(f)).
- **Rationale.** Scope 8.5, FR44, NFR26. Fixes the PHP guard: absolute lock, blind on shallow checkouts, run by no workflow.
- **Alternatives rejected.** A shell script (cannot be held to the coverage and mutation gates). Locking the whole root `Cargo.toml` (it changes with every dependency). Locking the whole `tests/Load/config.json.dist` (every scenario a service adds would need the label, which the PRD excludes). Deriving the override names from the file names of the protected list (a file named like a protected source file, for example a second `arch.rs`, would be reported).
- **Consequences.** The delivery document adds the configuration files of the gates it introduces to the same list, appends the override names of its linters, and must keep any command line that carries a threshold in a protected file. `xtask` gains the dependency `serde_json` for the JSON comparison.

### AD-26 Scaffolding command

- **Decision.** `cargo xtask new-context <group> <context>` creates `crates/<group>/<context>/{domain,application,infrastructure}`, each with a manifest that inherits `[workspace.package]` fields and `[lints] workspace = true`, no dependencies, and a `src/lib.rs` that contains only crate-level documentation describing what belongs in that layer. It adds the three paths to `[workspace].members` of the root `Cargo.toml` with `toml_edit` (formatting preserved), then runs `cargo update --workspace --offline`, which adds the three crates to `Cargo.lock` without network access, and prints the next steps, among them adding the module's components to `workspace.dsl` (FR39(e)). If the lock-file step fails the command exits with status 2. Both names must match `^[a-z][a-z0-9]*(-[a-z0-9]+)*$`; the context must not exist: the command refuses it, before anything is written, with `` the bounded context `<context>` already exists at <path> `` when `crates/<group>/<context>` exists for any group, when `crates/<context>` exists, when a registered member has a package named `<context>-domain`, `<context>-application` or `<context>-infrastructure`, or when a member path ends with `<context>/<layer>` (the path named is the context directory, or the member directory for a package-name match); reserved context names are `shared`, `service`, `xtask`, `test-support` and `e2e-tests`. Exit status: 0 created, 1 refused (invalid or existing name), 2 could not run.
- **Rationale.** FR39: the result compiles and passes the architecture gate and the crate-isolation gate without an edit to a protected file. Crates without dependencies also pass the unused-dependency and workspace-dependency gates, which a pre-wired skeleton would fail. The lock-file step is required: after three members are added without it, `cargo metadata --locked` and `cargo check --locked` exit 101 (`cannot update the lock file ... because --locked was passed`), which would break every gate and the `xtask` alias itself; `cargo update --workspace --offline` adds the three packages with exit status 0 and no network access, after which `--locked` passes again (verified on cargo 1.99.1).
- **Alternatives rejected.** `cargo generate` (another tool to install and pin). Generating a copy of the example slice (dead code in every new context). Leaving the lock-file refresh to the Make recipe (a direct `cargo xtask new-context` would leave the workspace unusable).
- **Consequences.** The developer adds `[workspace.dependencies]` entries and wiring in `service` when the first code arrives; the guide "create a service from the template" lists these steps. An integration test runs the command in a copy of a fixture workspace, checks that `cargo metadata --locked` succeeds, runs the architecture gate on the result and checks each of the three new packages alone with `cargo check --locked --offline --package <name>`, which is what the crate-isolation gate does per crate (FR39(b), NFR18); it also asserts that no path of `protected-config.toml` was touched. The tests of `xtask` that run against the real workspace assert at least 13 crates, never an exact count: `xtask/tests/` is protected, so a service that has run `new-context` could not adjust a pinned count without the approval label.

### AD-27 Lint policy on the pinned toolchain

- **Decision.** The lint table of the quality-gates research is adopted unchanged. `clippy.toml` keeps its thresholds; its list of banned methods is extended with the wall-clock, UUID-generation and socket APIs (NFR28) and its `disallowed-macros` block is removed (see `clippy.toml`). Levels are `warn`; gates pass `-D warnings`. Two passes: production targets with the full policy (QG-02), all targets with `cognitive_complexity` and `too_many_lines` relaxed (QG-03). The complexity gate G3 has its own command, QG-20, which denies only the four complexity lints, so a complexity finding is reported under its own name. Complexity limits: cognitive complexity 8, function length 50 lines, nesting depth 5, parameters 5 (all at or below the PRD ceilings of NFR17); test code is relaxed for cognitive complexity and length only.
- **Rationale.** Scope 8.5, FR41, FR123, NFR16, NFR17, NFR28. Verified on clippy 1.99.1 in the validation round: QG-01, QG-02 and QG-03 exit 0 on the skeleton (about 1.3 s each warm) and the table produces no unknown-lint warning. Verified in this revision: both passes exit 0 with the extended ban list once the suppressions listed in rule 20 are in place, and no entry of the list produces an invalid-path warning.
- **Alternatives rejected.** `deny` levels in the table (block ordinary builds mid-change). The whole `restriction` group. Raising the cognitive-complexity limit to make room for log events (rule 16; rule 17a solves it in the code).
- **Consequences.** Findings observed on 1.99.1 that shape the code rules in "Implementation Patterns":
  - `clippy::unused_async` fires only when the `async fn` is never used as a value in the crate being compiled. Handlers registered in a router of their own crate (`ping`, `docs_page`, `docs_asset`, `openapi_json`, `explorer`) carry no expect; an expect there is rejected as unfulfilled. The shared fallbacks `not_found` and `method_not_allowed` are registered by another crate and by their own unit test, so they carry `#[cfg_attr(not(test), expect(clippy::unused_async, reason = "axum handlers must be async"))]`: with a plain `expect` QG-02 passes and QG-03 fails, without it QG-02 fails.
  - `impl Trait` in argument position is not allowed (`impl_trait_in_params`); `Problem` must stay below 128 bytes.
  - `missing_debug_implementations` requires `Debug` on every public type, so every port that is held as `Arc<dyn Port>` has `Debug` as a supertrait ("Domain and Application Design").
  - With tracing's `log` feature active, one event macro costs 7 points of cognitive complexity: a function with only one event scores 8 of 8, an event plus one `.await` 9, an event inside `if let` 9, two events 15 (measured); rule 17a follows from this.
  - Neither `cognitive_complexity` nor `excessive_nesting` is reported inside an `#[async_trait]` method; rule 21 keeps logic out of such bodies. The delivery document's negative test for G3 plants its function in a plain `fn`.

### Decision Impact Analysis

**Implementation sequence.** (1) Workspace skeleton, tool configuration, `xtask arch` and `config-guard`. (2) `shared-domain`, then `shared-infrastructure` buses and `shared-application` problem type. (3) `service` skeleton: CLI, configuration, telemetry, `/ping`, fallbacks, middleware, exports. (4) Health-check context with fakes, then real probes. (5) Example context: domain, application, migration and adapter, GraphQL. (6) `test-support` grows with each step; `e2e-tests` after (4). (7) Scaffolding command. (8) Coverage and mutation to green.

**Cross-component dependencies.** The error table (AD-06) binds every handler, the API contract and the Gherkin features. The configuration model (AD-07) binds `.env`, `.env.test`, compose files and the Makefile. The `Adapters` value (AD-04) binds `test-support`, `e2e-tests` and `bootstrap`. The gate commands (AD-24, "Quality Gates") bind the Makefile and the workflows. The exit statuses of `serve` (AD-15) bind the compose stop grace period and the process-level tests.

## Workspace and Project Structure

### Complete tree of the Rust workspace

Everything below is part of the first release. Paths owned by the delivery document (Makefile, Dockerfile, compose files, `.github/`, `scripts/`, `tests/Load/`, `tests/CLI/bats/`, `docs/`, `workspace.dsl`) are not repeated here. A file marked `(+tests)` has a sibling `<name>/tests.rs` with its unit tests.

```text
Cargo.toml                         workspace manifest: members, shared package fields, dependency versions, lint policy
Cargo.lock                         committed lock file
rust-toolchain.toml                toolchain pin 1.99.1 with rustfmt, clippy, llvm-tools-preview
rustfmt.toml                       formatter settings (stable keys only)
clippy.toml                        complexity thresholds, test relaxations, banned APIs
deny.toml                          cargo-deny policy: advisories, licences, bans, sources
architecture.toml                  layer and context model checked by `cargo xtask arch`
protected-config.toml              the one list of protected files, sections, override names and required directories
.env                               committed development defaults (local-only placeholders)
.env.test                          committed test overrides: three values on top of `.env`
.cargo/config.toml                 `xtask` alias; SQLX_OFFLINE=true, the recorded test seeds and the Swagger UI download guard (FR8) for every cargo invocation
.cargo/mutants.toml                cargo-mutants scope record, test tool and timeouts
.config/nextest.toml               nextest profiles: default, ci, mutants
.sqlx/query-<hash>.json            sqlx offline query metadata, one file per checked query (generated, committed)
migrations/
  20261110000000_create_example_api_resources.up.sql     creates table example_api_resources
  20261110000000_create_example_api_resources.down.sql   drops table example_api_resources
features/
  health_check.feature             PHP feature, byte-identical (4 scenarios)
  api_docs.feature                 docs page and OpenAPI JSON are served (2 scenarios)
  example_api_resource.feature     create, read, list, validation, not found (7 scenarios)
  graphql.feature                  mutation, queries, mutation and both queries in one scenario, explorer disabled (5 scenarios)
  http_errors.feature              liveness, 404, 405, 415, 400, headers (6 scenarios)

crates/shared/domain/                         crate shared-domain
  Cargo.toml
  src/lib.rs                       crate docs, module declarations, re-exports of the public items
  src/error.rs (+tests)            `BoxError` type alias, `error_chain`
  src/aggregate.rs (+tests)        `AggregateRoot`, `RecordedEvents`
  src/event.rs                     `DomainEvent`, `DomainEventSubscriber`
  src/bus.rs                       module declarations for the four files below
  src/bus/error.rs (+tests)        `BusError`, `ErasedBusError`, `ErasedMessage`
  src/bus/command.rs (+tests)      `Command`, `CommandHandler`, `CommandBus` and typed `dispatch`
  src/bus/query.rs (+tests)        `Query`, `QueryHandler`, `QueryBus` and typed `ask`
  src/bus/event.rs (+tests)        `EventBus`, `EventBusError`, `SubscriberFailure`
  src/port.rs                      module declarations for the three files below
  src/port/cache.rs                `Cache`, `CacheError`
  src/port/clock.rs                `Clock`
  src/port/uuid_generator.rs       `UuidGenerator`
  src/value_object.rs              module declarations for the two files below
  src/value_object/uuid.rs (+tests)          `Uuid`, `InvalidUuid`
  src/value_object/page_request.rs (+tests)  `PageRequest`, `InvalidPageRequest`

crates/shared/application/                    crate shared-application
  Cargo.toml
  src/lib.rs                       crate docs, module declarations
  src/http.rs                      module declarations and re-exports for the four files below
  src/http/problem.rs (+tests)     `Problem`, `Violation`, `DependencyFailure`, type constants, `From` conversions
  src/http/extract.rs (+tests)     `ApiJson`, `ApiPath`, `ValidatedJson`, `ValidatedQuery`
  src/http/fallback.rs (+tests)    `not_found`, `method_not_allowed`, `panic_response`
  src/http/middleware.rs (+tests)  `request_timeout`
  src/graphql.rs (+tests)          `problem_to_error` (GraphQL error helper)

crates/shared/infrastructure/                 crate shared-infrastructure
  Cargo.toml
  build.rs                         `fn main() { println!("cargo:rerun-if-changed=../../../migrations"); }`
  src/lib.rs                       crate docs, module declarations
  src/bus.rs (+tests)              `DuplicateHandler`; module declarations for the three files below
  src/bus/command.rs (+tests)      `InMemoryCommandBus`
  src/bus/query.rs (+tests)        `InMemoryQueryBus`
  src/bus/event.rs (+tests)        `InMemoryEventBus`
  src/cache.rs (+tests)            `MokaCache`
  src/clock.rs                     `SystemClock` (the one caller of `SystemTime::now`)
  src/id.rs                        `UuidV7Generator` (the one caller of `uuid::Uuid::now_v7`)
  src/messaging.rs (+tests)        `SqsSettings`, `sqs_client`
  src/persistence.rs (+tests)      `PostgresSettings`, `connect_lazy`, `MIGRATOR`
  tests/integration/main.rs        module declarations
  tests/integration/support.rs     `SqsSettings` and `PostgresSettings` of the test stack, read from the environment
  tests/integration/clock.rs       two readings of `SystemClock` do not go backwards and lie after 2026-01-01
  tests/integration/id.rs          `UuidV7Generator` returns distinct, ordered version-7 UUIDs
  tests/integration/cache.rs       per-entry TTL expires in real time
  tests/integration/persistence.rs pool against PostgreSQL and against a dead port; migrations apply, revert, re-apply to the same catalog (no table, column or migration count of the example)
  tests/integration/messaging.rs   client against LocalStack and against a dead port within the timeout

crates/internal/health-check/domain/          crate health-check-domain
  Cargo.toml
  src/lib.rs
  src/port.rs                      `HealthProbe`
  src/value_object.rs              module declarations for the two files below
  src/value_object/dependency.rs (+tests)  `Dependency` with identifiers and failure messages
  src/value_object/report.rs (+tests)      `HealthReport`, `ProbeFailure`, `ProbeFailureCause`

crates/internal/health-check/application/     crate health-check-application
  Cargo.toml
  src/lib.rs
  src/service.rs (+tests)          `HealthService` (concurrent probes, per-probe timeout, 7 failure combinations, the logged cause)
  src/http.rs (+tests)             `HealthCheckState`, `router`, handler `health`

crates/internal/health-check/infrastructure/  crate health-check-infrastructure
  Cargo.toml
  src/lib.rs
  src/probe.rs                     module declarations for the three files below
  src/probe/postgres.rs            `PostgresProbe` (no unit test: it wraps a concrete pool, AD-11)
  src/probe/cache.rs (+tests)      `CacheProbe`
  src/probe/sqs.rs (+tests)        `SqsProbe` (unit tests with aws-smithy-mocks)
  tests/integration/main.rs
  tests/integration/support.rs     settings of the test stack, read from the environment
  tests/integration/postgres_probe.rs   healthy database; unreachable database fails within acquire timeout + 1 s
  tests/integration/sqs_probe.rs        LocalStack answers ListQueues; unreachable endpoint fails within timeout + 1 s

crates/company-subdomain/some-module/domain/  crate some-module-domain
  Cargo.toml
  src/lib.rs
  src/error.rs (+tests)            `InvalidExampleApiResource`
  src/aggregate.rs (+tests)        `ExampleApiResource`
  src/event.rs                     `ExampleApiResourceCreated`
  src/repository.rs                `ExampleApiResourceRepository`, `RepositoryError`
  src/value_object.rs              module declarations for the three files below
  src/value_object/id.rs           `ExampleApiResourceId`
  src/value_object/name.rs (+tests)         `ExampleApiResourceName`
  src/value_object/description.rs (+tests)  `ExampleApiResourceDescription`

crates/company-subdomain/some-module/application/   crate some-module-application
  Cargo.toml
  src/lib.rs
  src/error.rs (+tests)            `ExampleApiResourceError` and its conversion to `Problem`
  src/command.rs                   module declaration
  src/command/create_example_api_resource.rs (+tests)   `CreateExampleApiResource`, `CreateExampleApiResourceHandler`
  src/query.rs                     `ExampleApiResourceView`; module declarations
  src/query/get_example_api_resource.rs (+tests)        `GetExampleApiResource`, `GetExampleApiResourceHandler`
  src/query/list_example_api_resources.rs (+tests)      `ListExampleApiResources`, `ListExampleApiResourcesHandler`
  src/subscriber.rs                module declaration
  src/subscriber/audit_example_api_resource_created.rs (+tests)   `AuditExampleApiResourceCreated`
  src/http.rs                      `SomeModuleState`, `router`; module declarations
  src/http/dto.rs (+tests)         `ExampleApiResourceResponse`, `CreateExampleApiResourceRequest`, `ListExampleApiResourcesParams`
  src/http/handler.rs (+tests)     handlers `create`, `get_one`, `list`
  src/graphql.rs (+tests)          `ExampleApiResourceObject`, `CreateExampleApiResourceInput`, `ExampleApiResourceQuery`, `ExampleApiResourceMutation`

crates/company-subdomain/some-module/infrastructure/   crate some-module-infrastructure
  Cargo.toml
  src/lib.rs
  src/persistence.rs               `PgExampleApiResourceRepository`
  tests/integration/main.rs
  tests/integration/migration.rs   the example migration: the table and its named constraints, one reversible migration, the constraints reject an empty name and an oversized description
  tests/integration/repository.rs  save, find, page order and bounds, duplicate id, unreachable database (`#[sqlx::test]`)

crates/service/                               crate service (library + binary `service`)
  Cargo.toml
  src/main.rs                      `fn main() -> ExitCode { service::run() }`
  src/lib.rs                       `run`: load `.env`, parse CLI, dispatch to a command, map the outcome to an exit status
  src/cli.rs (+tests)              `Cli`, `Command`, `MigrateAction`, `ServeArgs`, `DatabaseArgs`, `MigrateArgs`, `ExportArgs`, `HealthcheckArgs`
  src/config.rs (+tests)           `Config`, `MigrateSettings`, `AppEnv`, `ConfigErrors`, `ConfigProblem`
  src/bootstrap.rs (+tests)        `Adapters`, `BootstrapError`, `event_bus`, `router`
  src/telemetry.rs                 `init` (JSON subscriber on a given writer, level filter), panic hook; covered by `tests/integration/{serve,panic}.rs`
  src/openapi.rs (+tests)          `ApiDoc`, `API_VERSION`, `ExportError`, `api_router`, `document`, `yaml`
  src/graphql.rs                   module declarations for the two files below
  src/graphql/schema.rs (+tests)   `Query`, `Mutation`, `ApiSchema`, `builder`, `sdl`
  src/graphql/handler.rs (+tests)  `execute`, `explorer`, `EXPLORER_CONTENT_SECURITY_POLICY`
  src/graphql/explorer.html        the development explorer page (four pinned assets, AD-18)
  src/http.rs                      `AppState`; module declarations for the files below
  src/http/router.rs (+tests)      `assemble`: routes, fallbacks, middleware order
  src/http/docs.rs (+tests)        `docs_page`, `docs_asset`, `openapi_json`, `DOCS_CONTENT_SECURITY_POLICY`
  src/http/docs.html               the documentation page
  src/http/ping.rs (+tests)        `ping`
  src/http/request_id.rs (+tests)  `REQUEST_ID_HEADER`, `sanitize_request_id`
  src/http/security_headers.rs (+tests)   header names, default values, `layer`
  src/http/trace.rs (+tests)       request span and access-log callbacks
  src/command.rs                   module declarations for the five files below; `run` (one dispatch arm per subcommand), `usage_error`, `failure`
  src/command/serve.rs             `run` (bind, serve, drain, forced shutdown) and `reject` (JSON report of an invalid configuration)
  src/command/migrate.rs           bounded database wait; apply, revert; plain-text output
  src/command/check_config.rs      print `configuration OK`
  src/command/export.rs (+tests)   write a document to stdout or a file
  src/command/healthcheck.rs       request `/ping` on the address of `APP_HOST` (the loopback interface by default)
  tests/integration/main.rs        module declarations
  tests/integration/support.rs     spawn the binary with the cleared environment and the forwarded profile variable, read its
                                   JSON log lines, send SIGTERM, open sockets, measure time, build the URL of a test database
  tests/integration/serve.rs       start-up line with the mode; `/ping`; `/api/health` 204; every stdout line is JSON and stderr is
                                   empty; request id in the logs of a request sequence; SIGTERM idle (exit 0 within 2 s); a request
                                   in flight completes (exit 0); a connection attempted after SIGTERM is refused; a connection still
                                   open when a 1 s grace period ends (exit 1 within grace period + 2 s); port in use (exit 1, JSON
                                   line `serve failed`); an invalid variable yields only JSON lines and exit 2; database and SQS
                                   endpoint on a dead port: `/ping` 200 within 5 s of process start, `/api/health` 500 naming both
                                   within `HEALTH_PROBE_TIMEOUT` + 1 s; entries inside nested spans at `debug` level carry the
                                   request id; `serve` applies no migration to a database that was never migrated; `/api/health`
                                   recovers without a restart once the database exists
  tests/integration/bootstrap.rs   production wiring in process against the compose services: `Adapters::connect` then
                                   `bootstrap::router`, dispatched with `oneshot`: `GET /api/health` is 204; a resource created
                                   through one application instance is read through a second one (NFR6)
  tests/integration/panic.rs       production telemetry on a captured writer, application built with `StubProbe::panicking`:
                                   the response is the 500 problem and every captured line, the `panic` entry included, is one
                                   JSON object that carries the request id (NFR31)
  tests/integration/migrate.rs     apply, apply again (nothing changes), revert, re-apply, plain-text output, unreachable database
                                   exits 1 after the wait with the connection error
  tests/integration/check_config.rs   valid environment exits 0; every invalid variable is named; exit status 2; credentials
                                   set singly are reported; `serve --help` does not contain the value of `DATABASE_URL`
  tests/integration/export.rs      both exports with an empty environment; 20 runs are byte-identical; `--output`
  tests/integration/healthcheck.rs exit 0 against a running service, exit 1 with nothing listening; a service bound to `127.0.0.2` is probed through `APP_HOST`

tests/support/                                crate test-support (test doubles; excluded from coverage and mutation by path)
  Cargo.toml
  src/lib.rs
  src/seed.rs                      `TestRng`: seeded generator, prints `TEST_SEED=<n>` when a test panics
  src/fixture.rs                   `example_api_resource`, `example_name`, `example_description` built from `TestRng`
  src/clock.rs                     `FixedClock`
  src/id.rs                        `SequenceUuidGenerator`
  src/bus.rs                       `FakeCommandBus`, `FakeQueryBus`, `RecordingEventBus`
  src/cache.rs                     `StubCache` (working, failing, forgetful, corrupting)
  src/probe.rs                     `StubProbe` (healthy, failing, hanging, panicking)
  src/repository.rs                `InMemoryExampleApiResourceRepository` (working, failing)
  src/log.rs                       `LogRecorder`, `LogLine`: records the tracing events of the current thread

tests/e2e/                                    crate e2e-tests (one test target `e2e`, harness = false)
  Cargo.toml
  main.rs                          cucumber runner: `fail_on_skipped_with(|_, _, _| true)` (every skipped step fails, also under the tag `@allow.skipped`), features from `../../features`, removal of databases left by a
                                   killed run, `after` hook that drops the scenario database
  world.rs                         `AppWorld`: failing dependencies, last response, created ids, scenario database; builds the
                                   app per request
  steps.rs                         module declarations for the four files below
  steps/http.rs                    request, status, body, header and JSON-field steps
  steps/health_check.rs            the three `Given the <dependency> is not …` steps
  steps/example_api_resource.rs    `Given an example API resource exists …`
  steps/graphql.rs                 GraphQL operation and result steps

xtask/                                        crate xtask (binary `xtask`, run as `cargo xtask <command>`)
  Cargo.toml
  src/main.rs                      collects arguments and calls `cli::main`
  src/cli.rs (+tests)              clap definition: `arch`, `config-guard`, `new-context`; exit statuses
  src/arch.rs (+tests)             `run`: load, check, report
  src/arch/config.rs (+tests)      schema of `architecture.toml`, pattern matching, context extraction
  src/arch/model.rs                `Workspace`, `Crate`, `Dependency`, `Reach`
  src/arch/load.rs (+tests)        facts from `cargo metadata --locked` and the manifests
  src/arch/rules.rs (+tests)       the eight rules as pure functions
  src/arch/report.rs (+tests)      diagnostics and GitHub annotations
  src/config_guard.rs (+tests)     `run`: load list, compare with base, apply approval, print the label name
  src/config_guard/list.rs (+tests)   schema of `protected-config.toml`
  src/config_guard/base.rs (+tests)   `BaseTree` trait (blob and paths at the base ref, paths of the working tree) and its git implementation
  src/config_guard/check.rs (+tests)  file, directory, manifest-table, JSON-value, override and required-directory checks
  src/scaffold.rs (+tests)         `run`: validate names, write crates, register members, refresh the lock file, print the next steps
  src/scaffold/name.rs (+tests)    name validation and reserved names
  src/scaffold/template.rs (+tests)   renders the six template files
  src/scaffold/workspace.rs (+tests)  adds members to the root manifest with `toml_edit`
  src/scaffold/lock.rs             runs `cargo update --workspace --offline` in the workspace root; covered by `tests/integration/scaffold.rs`
  templates/domain/Cargo.toml.tmpl, templates/domain/lib.rs.tmpl
  templates/application/Cargo.toml.tmpl, templates/application/lib.rs.tmpl
  templates/infrastructure/Cargo.toml.tmpl, templates/infrastructure/lib.rs.tmpl
  tests/integration/main.rs        module declarations
  tests/integration/support.rs     run the binary with the cleared environment and the forwarded profile variable; temporary git repositories
  tests/integration/arch.rs        binary against the real workspace (exit 0, at least 13 crates, no exact count) and against the fixture (exit 1, 2)
  tests/integration/config_guard.rs   binary in a temporary git repository: new file, modified file, deleted file, lint table, deleted
                                   table, JSON value, missing pointer, nested override, exempt fixture path, approval, label name
                                   in the failure output, missing base, `--list`
  tests/integration/scaffold.rs    binary in a copy of the fixture workspace; `cargo metadata --locked` succeeds; `arch` on the
                                   result; each new package checks alone; no protected path touched; refused names
  tests/fixtures/arch/workspace/   small workspace with one planted violation per rule
  tests/fixtures/arch/vendor/      stand-in third-party crates, outside the fixture workspace root
  tests/fixtures/arch/*.toml       valid, invalid and nothing-covered models
  tests/fixtures/scaffold/workspace/  minimal valid workspace used by the scaffold test
```

`proptest-regressions/` directories appear next to a crate when a property test fails; they are committed.

### Crate table

Layer crates take every third-party version from `[workspace.dependencies]` (`<name>.workspace = true`). "Dev" lists dev-dependencies. No crate has cargo features.

| Crate | Path | Kind | Layer | Context | Workspace dependencies | Third-party dependencies | Dev |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `shared-domain` | `crates/shared/domain` | lib | domain | shared | none | `async-trait`, `thiserror`, `uuid` | `proptest`, `rstest`, `tokio` |
| `shared-application` | `crates/shared/application` | lib | application | shared | `shared-domain` | `async-graphql`, `axum`, `serde`, `tokio`, `tracing`, `utoipa`, `validator` | `test-support`, `http-body-util`, `rstest`, `serde_json`, `tokio` (+`test-util`), `tower` |
| `shared-infrastructure` | `crates/shared/infrastructure` | lib | infrastructure | shared | `shared-domain` | `async-trait`, `aws-config`, `aws-sdk-sqs`, `moka`, `sqlx`, `thiserror`, `uuid` (+`v7`) | `tokio` |
| `health-check-domain` | `crates/internal/health-check/domain` | lib | domain | health-check | `shared-domain` | `async-trait`, `thiserror` | `rstest` |
| `health-check-application` | `crates/internal/health-check/application` | lib | application | health-check | `shared-application`, `shared-domain`, `health-check-domain` | `axum`, `futures-util`, `tokio`, `tracing`, `utoipa`, `utoipa-axum` | `async-trait`, `test-support`, `http-body-util`, `rstest`, `serde_json`, `tokio` (+`test-util`), `tower` |
| `health-check-infrastructure` | `crates/internal/health-check/infrastructure` | lib | infrastructure | health-check | `shared-domain`, `health-check-domain` | `async-trait`, `aws-sdk-sqs`, `sqlx` | `shared-infrastructure`, `test-support`, `aws-sdk-sqs` (+`test-util`), `aws-smithy-mocks`, `tokio` |
| `some-module-domain` | `crates/company-subdomain/some-module/domain` | lib | domain | some-module | `shared-domain` | `async-trait`, `thiserror` | `proptest`, `rstest` |
| `some-module-application` | `crates/company-subdomain/some-module/application` | lib | application | some-module | `shared-domain`, `shared-application`, `some-module-domain` | `async-graphql`, `async-trait`, `axum`, `humantime`, `serde`, `thiserror`, `tracing`, `utoipa`, `utoipa-axum`, `uuid`, `validator` | `test-support`, `http-body-util`, `proptest`, `rstest`, `serde_json`, `tokio`, `tower` |
| `some-module-infrastructure` | `crates/company-subdomain/some-module/infrastructure` | lib | infrastructure | some-module | `shared-domain`, `some-module-domain` | `async-trait`, `sqlx`, `uuid` | `shared-infrastructure`, `test-support`, `tokio` |
| `service` | `crates/service` | lib + bin `service` | composition-root | none | all nine layer crates | `async-graphql`, `axum`, `clap`, `dotenvy`, `humantime`, `serde_json`, `sqlx`, `thiserror`, `tokio`, `tower`, `tower-http`, `tracing`, `tracing-subscriber`, `utoipa`, `utoipa-axum`, `utoipa-swagger-ui` | `test-support`, `http-body-util`, `rstest`, `tokio` (+`test-util`) |
| `test-support` | `tests/support` | lib | test-support | none | `shared-domain`, `health-check-domain`, `some-module-domain` | `async-trait`, `fake`, `rand`, `tracing`, `uuid` | none |
| `e2e-tests` | `tests/e2e` | test target `e2e` only | e2e | none | (dev) `service`, `test-support`, `health-check-domain`, `shared-infrastructure` | none | `axum`, `clap`, `cucumber`, `http-body-util`, `serde_json`, `sqlx`, `tokio`, `tower` |
| `xtask` | `xtask` | bin `xtask` | tooling | none | none | `cargo_metadata`, `clap`, `serde`, `serde_json`, `thiserror`, `toml`, `toml_edit` | none |

Manifest conventions for every member: `version`, `edition`, `rust-version`, `license`, `publish` and `repository` are inherited with `.workspace = true`; each crate has its own one-line `description`, except `service`, which inherits `description.workspace = true` (AD-16); every manifest ends with `[lints] workspace = true`. `e2e-tests` sets `autotests = false` and declares `[[test]] name = "e2e"`, `path = "main.rs"`, `harness = false` (verified: cargo 1.99.1 accepts a package whose only target is a test).

Why the test crates carry these dependencies: `e2e-tests` needs `clap` because `Cli::parse_from` is a method of the `clap::Parser` trait (without it the world does not compile, verified), and `sqlx` plus `shared-infrastructure` for the per-scenario database and `MIGRATOR`; `test-support` needs `tracing` for `LogRecorder`; `shared-application` needs `test-support` for the log assertion on `Problem::internal`. `shared-infrastructure` has no `tracing` dependency: since the event bus returns its failures instead of logging them, nothing in that crate logs.

### `architecture.toml`

```toml
# Architecture model, checked by `cargo xtask arch`.
# A workspace crate must match exactly one layer pattern. Dependencies between
# layers, between bounded contexts and on third-party crates are allow-lists.

[contexts]
# A crate matched by a pattern that contains `*` belongs to the bounded context
# named by the text the `*` matched (`some-module-domain` -> `some-module`).
# A context may depend on itself and on the contexts listed here, nothing else.
shared = ["shared"]

[[layer]]
name = "domain"
crates = ["*-domain"]
directory = "domain"
depends_on = ["domain"]
# Domain is framework-free: these third-party crates and no others.
external = ["async-trait", "thiserror", "uuid"]
dev_external = ["proptest", "rstest", "tokio"]
# These must not even be reachable through non-dev dependencies.
forbidden_transitive = ["axum", "hyper", "reqwest", "sqlx", "tokio"]

[[layer]]
name = "application"
crates = ["*-application"]
directory = "application"
depends_on = ["domain", "application"]
dev_depends_on = ["test-support"]
external = [
    "async-graphql",
    "async-trait",
    "axum",
    "futures-util",
    "humantime",
    "serde",
    "thiserror",
    "tokio",
    "tracing",
    "utoipa",
    "utoipa-axum",
    "uuid",
    "validator",
]
dev_external = ["http-body-util", "proptest", "rstest", "serde_json", "tokio", "tower"]
forbidden_transitive = ["aws-config", "aws-sdk-sqs", "moka", "sqlx"]

[[layer]]
name = "infrastructure"
crates = ["*-infrastructure"]
directory = "infrastructure"
depends_on = ["domain", "infrastructure"]
dev_depends_on = ["test-support"]
external = [
    "async-trait",
    "aws-config",
    "aws-sdk-sqs",
    "moka",
    "sqlx",
    "thiserror",
    "tracing",
    "uuid",
]
dev_external = ["aws-smithy-mocks", "rstest", "tokio"]
forbidden_transitive = ["async-graphql", "axum", "utoipa"]

[[layer]]
name = "composition-root"
crates = ["service"]
depends_on = ["domain", "application", "infrastructure"]
dev_depends_on = ["test-support"]
external = ["*"]

[[layer]]
name = "test-support"
crates = ["test-support"]
depends_on = ["domain"]
# `tracing` is here for the log recorder that tests use to assert on log entries.
external = ["async-trait", "fake", "rand", "tracing", "uuid"]

[[layer]]
name = "e2e"
crates = ["e2e-tests"]
depends_on = ["domain", "application", "infrastructure", "composition-root", "test-support"]
external = ["*"]

[[layer]]
name = "tooling"
crates = ["xtask"]
depends_on = []
external = ["*"]
```

Schema notes for the checker: `crates`, `depends_on`, `external`, `dev_external`, `forbidden_transitive` are lists of names in which one `*` matches any run of characters; the `dev_*` lists extend the normal lists: a dev-dependency is allowed when its target is in `depends_on` or `dev_depends_on` (workspace crates) or in `external` or `dev_external` (third-party crates); `directory` switches on `arch::path` for the layer; unknown keys are an error (exit status 2). Read with the union, the manifests of this document have no violation; read strictly (dev-dependencies against the `dev_*` lists alone) they would have 16 (measured over `cargo metadata` of the skeleton). The top-level module names of a layer (`aggregate`, `bus`, `error`, `event`, `port`, `repository`, `value_object` in Domain; `command`, `error`, `graphql`, `http`, `query`, `service`, `subscriber` in Application; `bus`, `cache`, `clock`, `id`, `messaging`, `persistence`, `probe` in Infrastructure) are a naming convention ("Implementation Patterns"), not a key of this file.

### Root `Cargo.toml`

```toml
[workspace]
resolver = "3"
members = [
    "crates/company-subdomain/some-module/application",
    "crates/company-subdomain/some-module/domain",
    "crates/company-subdomain/some-module/infrastructure",
    "crates/internal/health-check/application",
    "crates/internal/health-check/domain",
    "crates/internal/health-check/infrastructure",
    "crates/service",
    "crates/shared/application",
    "crates/shared/domain",
    "crates/shared/infrastructure",
    "tests/e2e",
    "tests/support",
    "xtask",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
# The same full version as `rust-toolchain.toml` and the builder image tag (NFR13, G31 rule (c)).
rust-version = "1.99.1"
license = "CC0-1.0"
publish = false
# Service identity: the OpenAPI title is "<description> API" and the contact URL is `repository`.
description = "Rust Service Template"
repository = "https://github.com/VilnaCRM-Org/rust-service-template"

[workspace.dependencies]
# Workspace crates
health-check-application = { path = "crates/internal/health-check/application" }
health-check-domain = { path = "crates/internal/health-check/domain" }
health-check-infrastructure = { path = "crates/internal/health-check/infrastructure" }
service = { path = "crates/service" }
shared-application = { path = "crates/shared/application" }
shared-domain = { path = "crates/shared/domain" }
shared-infrastructure = { path = "crates/shared/infrastructure" }
some-module-application = { path = "crates/company-subdomain/some-module/application" }
some-module-domain = { path = "crates/company-subdomain/some-module/domain" }
some-module-infrastructure = { path = "crates/company-subdomain/some-module/infrastructure" }
test-support = { path = "tests/support" }

# Runtime
async-graphql = { version = "7.2.1", default-features = false, features = ["uuid"] }
async-trait = "0.1.92"
aws-config = { version = "1.13.1", default-features = false, features = [
    "rt-tokio",
    "default-https-client",
] }
aws-sdk-sqs = { version = "1.120.0", default-features = false, features = [
    "rt-tokio",
    "default-https-client",
] }
axum = { version = "0.8.9", features = ["macros"] }
clap = { version = "4.6.7", features = ["derive", "env"] }
dotenvy = "0.15.7"
futures-util = { version = "0.3.34", default-features = false, features = ["alloc"] }
humantime = "2.4.0"
moka = { version = "0.12.16", features = ["future"] }
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.154"
sqlx = { version = "0.9.0", default-features = false, features = [
    "runtime-tokio",
    "tls-rustls-aws-lc-rs",
    "postgres",
    "macros",
    "migrate",
    "uuid",
] }
thiserror = "2.0.21"
tokio = { version = "1.54.1", features = [
    "macros",
    "rt-multi-thread",
    "net",
    "signal",
    "time",
    "sync",
] }
tower = { version = "0.5.3", features = ["util"] }
tower-http = { version = "0.7.1", features = ["trace", "request-id", "catch-panic", "set-header"] }
tracing = "0.1.44"
tracing-subscriber = { version = "0.3.23", features = ["json", "env-filter"] }
utoipa = { version = "6.0.0", features = ["axum_extras", "uuid", "yaml"] }
utoipa-axum = "0.3.0"
utoipa-swagger-ui = { version = "10.0.2", features = ["vendored"] }
uuid = { version = "1.29.2", default-features = false, features = ["std"] }
validator = { version = "0.21.0", features = ["derive"] }

# Tests
aws-smithy-mocks = "0.3.0"
cucumber = "0.23.0"
fake = { version = "5.1.0", features = ["uuid"] }
http-body-util = "0.1.5"
proptest = "1.12.0"
rand = "0.10.3"
rstest = "0.28.0"

# Tooling (xtask)
cargo_metadata = "0.23.1"
toml = "1.2.1"
toml_edit = "0.25.19"

[profile.release]
strip = "symbols"

[workspace.lints.rust]
unsafe_code = "forbid"
non_ascii_idents = "forbid"
missing_docs = "warn"
missing_debug_implementations = "warn"
unreachable_pub = "warn"
unused_qualifications = "warn"
unused_lifetimes = "warn"
unused_import_braces = "warn"
unused_macro_rules = "warn"
single_use_lifetimes = "warn"
redundant_lifetimes = "warn"
trivial_casts = "warn"
trivial_numeric_casts = "warn"
unit_bindings = "warn"
unnameable_types = "warn"
let_underscore_drop = "warn"
macro_use_extern_crate = "warn"
meta_variable_misuse = "warn"
explicit_outlives_requirements = "warn"
ambiguous_negative_literals = "warn"
closure_returning_async_block = "warn"
impl_trait_redundant_captures = "warn"
if_let_rescope = "warn"
future_incompatible = { level = "warn", priority = -1 }
keyword_idents = { level = "warn", priority = -1 }
let_underscore = { level = "warn", priority = -1 }
nonstandard_style = { level = "warn", priority = -1 }
refining_impl_trait = { level = "warn", priority = -1 }
rust_2018_idioms = { level = "warn", priority = -1 }
unused = { level = "warn", priority = -1 }
# cargo-llvm-cov sets cfg(coverage); declare it so `unexpected_cfgs` stays quiet.
unexpected_cfgs = { level = "warn", check-cfg = ['cfg(coverage)', 'cfg(coverage_nightly)'] }

[workspace.lints.rustdoc]
broken_intra_doc_links = "warn"
private_intra_doc_links = "warn"
invalid_codeblock_attributes = "warn"
invalid_html_tags = "warn"
invalid_rust_codeblocks = "warn"
bare_urls = "warn"
redundant_explicit_links = "warn"
unescaped_backticks = "warn"
missing_crate_level_docs = "warn"

[workspace.lints.clippy]
# Whole groups (priority -1 so individual lints below can override them).
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
nursery = { level = "warn", priority = -1 }
cargo = { level = "warn", priority = -1 }
# Group opt-outs, each with a recorded reason.
multiple_crate_versions = "allow" # owned by cargo-deny `bans.multiple-versions`
redundant_pub_crate = "allow"     # contradicts rustc `unreachable_pub`, which wins
# Restriction lints: cherry-picked, never the whole group.
# -- no panicking paths in production code
unwrap_used = "warn"
expect_used = "warn"
panic = "warn"
todo = "warn"
unimplemented = "warn"
unreachable = "warn"
indexing_slicing = "warn"
string_slice = "warn"
get_unwrap = "warn"
unwrap_in_result = "warn"
panic_in_result_fn = "warn"
arithmetic_side_effects = "warn"
integer_division_remainder_used = "allow"
float_arithmetic = "allow"
# -- no silent lossy conversions
as_conversions = "warn"
as_underscore = "warn"
lossy_float_literal = "warn"
float_cmp_const = "warn"
# -- no debugging leftovers, no ad-hoc output or process control
dbg_macro = "warn"
print_stdout = "warn"
print_stderr = "warn"
use_debug = "warn"
exit = "warn"
mem_forget = "warn"
# -- error handling discipline
map_err_ignore = "warn"
let_underscore_must_use = "warn"
let_underscore_untyped = "warn"
unused_result_ok = "warn"
try_err = "warn"
error_impl_error = "warn"
# -- suppressions must be justified and self-expiring
allow_attributes = "warn"
allow_attributes_without_reason = "warn"
# -- structure and readability
absolute_paths = "warn"
clone_on_ref_ptr = "warn"
create_dir = "warn"
empty_drop = "warn"
empty_enum_variants_with_brackets = "warn"
empty_structs_with_brackets = "warn"
filetype_is_file = "warn"
fn_to_numeric_cast_any = "warn"
if_then_some_else_none = "warn"
impl_trait_in_params = "warn"
infinite_loop = "warn"
iter_over_hash_type = "warn"
large_include_file = "warn"
mixed_read_write_in_expression = "warn"
mod_module_files = "warn"
multiple_inherent_impl = "warn"
mutex_atomic = "warn"
needless_raw_strings = "warn"
partial_pub_fields = "warn"
pathbuf_init_then_push = "warn"
pub_without_shorthand = "warn"
rc_buffer = "warn"
rc_mutex = "warn"
redundant_type_annotations = "warn"
rest_pat_in_fully_bound_structs = "warn"
same_name_method = "warn"
semicolon_inside_block = "warn"
str_to_string = "warn"
string_add = "warn"
string_lit_chars_any = "warn"
suspicious_xor_used_as_pow = "warn"
undocumented_unsafe_blocks = "warn"
unnecessary_safety_comment = "warn"
unnecessary_safety_doc = "warn"
unneeded_field_pattern = "warn"
unused_trait_names = "warn"
verbose_file_reads = "warn"
wildcard_enum_match_arm = "warn"
# -- complexity budget (thresholds live in clippy.toml)
cognitive_complexity = "warn"
excessive_nesting = "warn"
```

Per-crate feature additions are written in the member manifest, never in the root: `uuid = { workspace = true, features = ["v7"] }` in `shared-infrastructure`; `tokio = { workspace = true, features = ["test-util"] }` and `aws-sdk-sqs = { workspace = true, features = ["test-util"] }` under `[dev-dependencies]` where the crate table says so.

Verified on cargo 1.99.1. In this revision, for the dependency set above: `cargo update --workspace --offline` resolves it to 468 packages (455 third-party) and `cargo metadata --locked` exits 0 with `rust_version` 1.99.1. In the validation round, on the skeleton before the feature removals of this revision (483 packages): `cargo metadata --locked` (QG-07) exits 0 in 0.25 s; `cargo hack check --workspace --each-feature --locked` (QG-08) exits 0 in 13 runs (18.6 s warm); `cargo deny check` with the `deny.toml` of this document reports advisories, bans, licences and sources ok with 13 duplicate-version warnings in 1.3 s, and its offline subset passes; `cargo shear` reports no issue once every dev-dependency is used.

## Domain and Application Design

Signatures are normative: names, parameters, return types and trait bounds must be implemented as written. Bodies are described in comments. Every public item also gets a doc comment (`missing_docs`). Derives shown are the minimum. Three conventions apply to every listing and are not repeated in each: every public function that returns a value carries `#[must_use]`; every public function that returns `Result` has an `# Errors` section in its doc comment; the body of every method in an `#[async_trait] impl` is one delegating call to an inherent `async fn` of the same type (rule 21), and the comments below describe that inherent function. The listings were compiled in the validation skeleton with the changes this revision records (`Debug` supertraits on ports, the bound-free `BusError`, by-value getters on `PageRequest`).

### Shared kernel: `shared-domain`

```rust
// error.rs
pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;
pub fn error_chain(cause: &(dyn std::error::Error + 'static)) -> String;   // the error and every source, outermost
                                                                           // first, joined by ": "; re-exported at the root

// value_object/uuid.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Uuid(uuid::Uuid);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{value}` is not a valid UUID")]
pub struct InvalidUuid { pub value: String }

impl Uuid {
    pub fn parse(value: &str) -> Result<Self, InvalidUuid>;   // uuid::Uuid::try_parse; any version
    pub const fn from_uuid(value: uuid::Uuid) -> Self;
    pub const fn as_uuid(&self) -> uuid::Uuid;
}
impl std::fmt::Display for Uuid { /* lower-case hyphenated form */ }

// value_object/page_request.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest { page: u32, items_per_page: u32 }

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InvalidPageRequest {
    #[error("page must be 1 or greater")] Page,
    #[error("itemsPerPage must be between 1 and 100")] ItemsPerPage,
}

impl PageRequest {
    pub const DEFAULT_PAGE: u32 = 1;
    pub const DEFAULT_ITEMS_PER_PAGE: u32 = 30;
    pub const MAX_ITEMS_PER_PAGE: u32 = 100;
    pub fn new(page: u32, items_per_page: u32) -> Result<Self, InvalidPageRequest>;   // page >= 1; 1 <= items_per_page <= 100
    pub fn limit(self) -> i64;                  // items_per_page; by value: clippy::trivially_copy_pass_by_ref
    pub fn offset(self) -> i64;                 // (page - 1) * items_per_page, computed in i64 (cannot overflow)
}

// port/clock.rs, port/uuid_generator.rs, port/cache.rs
// Every port has `Debug` as a supertrait: `missing_debug_implementations` requires `Debug` on every public
// type, and a struct that holds `Arc<dyn Port>` can derive it only then.
pub trait Clock: std::fmt::Debug + Send + Sync { fn now(&self) -> std::time::SystemTime; }
pub trait UuidGenerator: std::fmt::Debug + Send + Sync { fn generate(&self) -> Uuid; }

#[derive(Debug, thiserror::Error)]
#[error("cache operation failed")]
pub struct CacheError(#[source] pub BoxError);

#[async_trait::async_trait]
pub trait Cache: std::fmt::Debug + Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError>;
    async fn set(&self, key: &str, value: Vec<u8>, ttl: std::time::Duration) -> Result<(), CacheError>;
}

// event.rs
pub trait DomainEvent: std::any::Any + std::fmt::Debug + Send + Sync {
    fn event_id(&self) -> Uuid;
    fn aggregate_id(&self) -> Uuid;
    fn occurred_at(&self) -> std::time::SystemTime;
    fn event_name(&self) -> &'static str;   // stable, `<aggregate_snake_case>.<past_tense_verb>`
}

#[async_trait::async_trait]
pub trait DomainEventSubscriber<E: DomainEvent>: Send + Sync + 'static {
    async fn handle(&self, event: &E) -> Result<(), BoxError>;
}

// aggregate.rs
#[derive(Debug, Default)]
pub struct RecordedEvents(Vec<Box<dyn DomainEvent>>);
impl RecordedEvents {
    pub fn record<E: DomainEvent>(&mut self, event: E);
    pub fn pull(&mut self) -> Vec<Box<dyn DomainEvent>>;   // drains, recording order
}
pub trait AggregateRoot {
    fn recorded_events(&mut self) -> &mut RecordedEvents;
    fn pull_domain_events(&mut self) -> Vec<Box<dyn DomainEvent>> { self.recorded_events().pull() }
}

// bus/error.rs
pub type ErasedMessage = Box<dyn std::any::Any + Send>;

// No bound on `E` here: with `E: std::error::Error + 'static` the thiserror derive repeats the bound and
// clippy reports `trait_duplication_in_bounds`.
#[derive(Debug, thiserror::Error)]
pub enum BusError<E> {
    #[error("no handler is registered for `{message}`")]
    NotRegistered { message: &'static str },
    #[error(transparent)]
    Handler(E),
}

#[derive(Debug)]
pub enum ErasedBusError {
    NotRegistered { message: &'static str },
    Handler(ErasedMessage),   // the handler's error, boxed as `Any`
}
impl ErasedBusError {
    pub(crate) fn into_typed<E: std::error::Error + Send + 'static>(self, message: &'static str) -> BusError<E>;
}

// bus/command.rs
pub trait Command: Send + 'static {
    type Error: std::error::Error + Send + Sync + 'static;
    const NAME: &'static str;   // the type name, used in errors and logs
}

#[async_trait::async_trait]
pub trait CommandHandler<C: Command>: Send + Sync + 'static {
    async fn handle(&self, command: C) -> Result<(), C::Error>;
}

#[async_trait::async_trait]
pub trait CommandBus: std::fmt::Debug + Send + Sync {
    async fn dispatch_erased(
        &self, command_type: std::any::TypeId, name: &'static str, command: ErasedMessage,
    ) -> Result<(), ErasedBusError>;
}
impl dyn CommandBus {
    pub async fn dispatch<C: Command>(&self, command: C) -> Result<(), BusError<C::Error>>;
}

// bus/query.rs
pub trait Query: Send + 'static {
    type Output: Send + 'static;
    type Error: std::error::Error + Send + Sync + 'static;
    const NAME: &'static str;
}

#[async_trait::async_trait]
pub trait QueryHandler<Q: Query>: Send + Sync + 'static {
    async fn handle(&self, query: Q) -> Result<Q::Output, Q::Error>;
}

#[async_trait::async_trait]
pub trait QueryBus: std::fmt::Debug + Send + Sync {
    async fn ask_erased(
        &self, query_type: std::any::TypeId, name: &'static str, query: ErasedMessage,
    ) -> Result<ErasedMessage, ErasedBusError>;
}
impl dyn QueryBus {
    pub async fn ask<Q: Query>(&self, query: Q) -> Result<Q::Output, BusError<Q::Error>>;
}

// bus/event.rs
/// The failure of one subscriber for one event.
#[derive(Debug, thiserror::Error)]
#[error("a subscriber of `{event_name}` failed for event {event_id}")]
pub struct SubscriberFailure { pub event_name: &'static str, pub event_id: Uuid, #[source] pub source: BoxError }

/// Every subscriber failure of one `publish` call, in invocation order; never empty.
#[derive(Debug, thiserror::Error)]
#[error("{} event subscriber(s) failed", .failures.len())]
pub struct EventBusError { pub failures: Vec<SubscriberFailure> }

#[async_trait::async_trait]
pub trait EventBus: std::fmt::Debug + Send + Sync {
    async fn publish(&self, events: Vec<Box<dyn DomainEvent>>) -> Result<(), EventBusError>;
}
```

The example slice takes its list defaults from `PageRequest::DEFAULT_PAGE` and `PageRequest::DEFAULT_ITEMS_PER_PAGE` (`some-module-application`, `http/dto.rs`, converted with `cast_signed`), so every public item of `PageRequest` has a user (NFR23).

### In-process adapters: `shared-infrastructure`

```rust
// bus.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a handler is already registered for `{message}`")]
pub struct DuplicateHandler { pub message: &'static str }

// bus/command.rs — bus/query.rs is symmetrical (InMemoryQueryBus, Query, QueryHandler)
#[derive(Default)]
pub struct InMemoryCommandBus { /* handlers: HashMap<TypeId, Arc<dyn ErasedCommandHandler>> */ }
impl InMemoryCommandBus {
    pub fn new() -> Self;
    pub fn register<C: Command, H: CommandHandler<C>>(self, handler: H) -> Result<Self, DuplicateHandler>;
}
// The erased handlers are not `Debug`, so `Debug` is written by hand (it prints the number of handlers) and has a unit test.
impl std::fmt::Debug for InMemoryCommandBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InMemoryCommandBus").field("handlers", &self.handlers.len()).finish()
    }
}
impl CommandBus for InMemoryCommandBus { /* look up by TypeId; missing -> ErasedBusError::NotRegistered */ }

// bus/event.rs
#[derive(Default)]
pub struct InMemoryEventBus { /* subscribers: HashMap<TypeId, Vec<Arc<dyn ErasedSubscriber>>> */ }
impl InMemoryEventBus {
    pub fn new() -> Self;
    #[must_use]
    pub fn subscribe<E: DomainEvent, S: DomainEventSubscriber<E>>(self, subscriber: S) -> Self;
}
impl std::fmt::Debug for InMemoryEventBus { /* prints the number of event types with subscribers; unit-tested */ }
impl EventBus for InMemoryEventBus {
    // AD-05: every subscriber of every event runs, in registration order; a failure does not stop the others;
    // every failure is returned as a `SubscriberFailure` in invocation order; nothing is logged here.
}

// cache.rs
pub struct MokaCache { /* entries: moka::future::Cache<String, Entry { value, ttl }> */ }
impl MokaCache { pub const MAX_ENTRIES: u64 = 10_000; pub fn new() -> Self; }
impl Default for MokaCache { fn default() -> Self { Self::new() } }   // clippy::new_without_default
impl std::fmt::Debug for MokaCache { /* prints the entry count, never the values; unit-tested */ }
impl Cache for MokaCache {}

// clock.rs, id.rs — the two adapters that own the banned calls (clippy `disallowed-methods`, NFR28)
#[derive(Debug, Clone, Copy, Default)] pub struct SystemClock;
impl Clock for SystemClock {
    #[expect(clippy::disallowed_methods, reason = "the adapter that owns the wall clock")]
    fn now(&self) -> SystemTime { SystemTime::now() }
}
#[derive(Debug, Clone, Copy, Default)] pub struct UuidV7Generator;
impl UuidGenerator for UuidV7Generator {
    #[expect(clippy::disallowed_methods, reason = "the adapter that owns identifier generation")]
    fn generate(&self) -> Uuid { Uuid::from_uuid(uuid::Uuid::now_v7()) }
}

// persistence.rs
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../../migrations");
#[derive(Debug, Clone)]
pub struct PostgresSettings {
    pub url: String, pub max_connections: u32,
    pub acquire_timeout: std::time::Duration, pub statement_timeout: std::time::Duration,
}
pub const MIN_STATEMENT_TIMEOUT: std::time::Duration;   // 1 ms: PostgreSQL reads 0 as no limit
pub const MAX_STATEMENT_TIMEOUT: std::time::Duration;   // 24 days: PostgreSQL refuses more than 2147483647 ms
pub fn connect_lazy(settings: &PostgresSettings) -> Result<sqlx::PgPool, sqlx::Error>;   // no network access; the
                                                        // statement_timeout it sets is clamped to MIN..=MAX

// messaging.rs
#[derive(Debug, Clone)]
pub struct SqsSettings {
    pub region: String, pub endpoint_url: Option<String>,
    pub connect_timeout: std::time::Duration, pub operation_timeout: std::time::Duration, pub max_attempts: u32,
}
pub async fn sqs_client(settings: &SqsSettings) -> aws_sdk_sqs::Client;   // AD-10; no network access
```

The erased handler adapters (`ErasedCommandHandler`, `ErasedQueryHandler`, `ErasedSubscriber` and their generic wrapper structs) are private to their module; they downcast the `ErasedMessage` to the concrete type and box the handler's error as `Any` (verified design). The event bus downcasts `&dyn DomainEvent` to `&E` through trait-object upcasting to `&dyn Any`. `PostgresSettings` derives `Debug` and holds the URL with its password: no code formats a `PostgresSettings` or a `Config` with `{:?}` or records one as a log field (rule 13); `clippy::use_debug` reports `{:?}` in format strings of production code, and the process-level tests assert that no output line contains the password.

### Health-check context

```rust
// health-check-domain: value_object/dependency.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dependency { Database, Cache, MessageBroker }
impl Dependency {
    pub const fn identifier(self) -> &'static str;       // "database" | "cache" | "message_broker"
    pub const fn failure_message(self) -> &'static str;  // "Database is not available" | "Cache is not working"
                                                         // | "Message broker is not available"
}

// health-check-domain: value_object/report.rs
#[derive(Debug, thiserror::Error)]
pub enum ProbeFailureCause {
    #[error("probe failed")] Failed(#[source] BoxError),   // the reason is in the source chain
    #[error("probe timed out after {0:?}")] TimedOut(std::time::Duration),
}
#[derive(Debug)]
pub struct ProbeFailure { pub dependency: Dependency, pub cause: ProbeFailureCause }
#[derive(Debug, Default)]
pub struct HealthReport { failures: Vec<ProbeFailure> }
impl HealthReport {
    pub const fn new(failures: Vec<ProbeFailure>) -> Self;
    pub const fn is_healthy(&self) -> bool;      // const: clippy::missing_const_for_fn
    pub fn failures(&self) -> &[ProbeFailure];   // registration order
}

// health-check-domain: port.rs
#[async_trait::async_trait]
pub trait HealthProbe: std::fmt::Debug + Send + Sync {
    fn dependency(&self) -> Dependency;
    async fn check(&self) -> Result<(), BoxError>;
}

// health-check-application: service.rs
#[derive(Debug)]
pub struct HealthService { probes: Vec<Arc<dyn HealthProbe>>, probe_timeout: Duration }
impl HealthService {
    pub const fn new(probes: Vec<Arc<dyn HealthProbe>>, probe_timeout: Duration) -> Self;
    pub async fn check(&self) -> HealthReport;   // join_all over `self.run(probe)`, failures collected in probe order
    async fn run(&self, probe: &dyn HealthProbe) -> Option<ProbeFailure>;   // probe.check() under tokio::time::timeout;
                                                 // a failure or a timeout becomes a ProbeFailure, logged with
                                                 // log_failure(failure.dependency, &error_chain(&failure.cause))
}
// The event is the only statement of its own function and is written on one line (rule 17a).
fn log_failure(dependency: Dependency, cause: &str) {
    tracing::warn!(dependency = dependency.identifier(), cause = %cause, "health probe failed");
}

// health-check-application: http.rs
#[derive(Debug, Clone)]
pub struct HealthCheckState { pub service: Arc<HealthService> }

pub fn router<S>() -> OpenApiRouter<S>
where S: Clone + Send + Sync + 'static, HealthCheckState: FromRef<S>;   // routes!(health)

#[utoipa::path(get, path = "/health", operation_id = "api_health_get", tag = "HealthCheck", /* responses: "API Contract" */)]
pub async fn health(State(state): State<HealthCheckState>) -> Result<StatusCode, Problem>;
// Ok(StatusCode::NO_CONTENT) when the report is healthy, else Err(Problem::dependency_unavailable(..))

// health-check-infrastructure: probe/{postgres,cache,sqs}.rs — each derives Debug, has `pub const fn new(..) -> Self`
// and implements HealthProbe by delegating to an inherent `async fn probe(&self) -> Result<(), BoxError>` (rule 21)
#[derive(Debug)] pub struct PostgresProbe { pool: sqlx::PgPool }      // sqlx::query("SELECT 1").execute(&pool)
#[derive(Debug)] pub struct CacheProbe { cache: Arc<dyn Cache> }      // KEY = "health_check", VALUE = b"ok", TTL = 60 s; set, get, compare
#[derive(Debug)] pub struct SqsProbe { client: aws_sdk_sqs::Client }  // list_queues().max_results(1)
```

`HealthService::check` written as one function with the timeout match and the event inline scored 10 against the cognitive-complexity limit of 8; the split above (`check`, `run`, `log_failure`) is the form that passed QG-02 in the skeleton.

### Example bounded context

Invariants of `ExampleApiResource`: the id never changes; `name` has 1 to 255 Unicode scalar values and no U+0000; `description` has 1 to 1024 Unicode scalar values and no U+0000; lengths are counted with `chars().count()`; no trimming or normalisation is applied. An aggregate can only be built from valid value objects.

```rust
// some-module-domain: error.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InvalidExampleApiResource {
    #[error("This value must contain between 1 and 255 characters.")] NameLength,
    #[error("This value must not contain the NUL character.")] NameContainsNul,
    #[error("This value must contain between 1 and 1024 characters.")] DescriptionLength,
    #[error("This value must not contain the NUL character.")] DescriptionContainsNul,
}
impl InvalidExampleApiResource {
    pub const fn property_path(self) -> &'static str;   // "name" | "description"
    pub const fn code(self) -> &'static str;            // "length" | "does_not_contain"
}

// some-module-domain: value_object/{id,name,description}.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExampleApiResourceId(Uuid);
impl ExampleApiResourceId { pub const fn new(value: Uuid) -> Self; pub const fn value(self) -> Uuid; }
impl std::fmt::Display for ExampleApiResourceId {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExampleApiResourceName(String);
impl ExampleApiResourceName {
    pub const MAX_CHARS: usize = 255;
    pub fn new(value: String) -> Result<Self, InvalidExampleApiResource>;
    pub fn as_str(&self) -> &str;
}
// ExampleApiResourceDescription: same shape, MAX_CHARS = 1024

// some-module-domain: event.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExampleApiResourceCreated { pub event_id: Uuid, pub aggregate_id: Uuid, pub occurred_at: SystemTime }
impl ExampleApiResourceCreated { pub const NAME: &'static str = "example_api_resource.created"; }
impl DomainEvent for ExampleApiResourceCreated {}

// some-module-domain: aggregate.rs
#[derive(Debug)]
pub struct ExampleApiResource {
    id: ExampleApiResourceId, name: ExampleApiResourceName,
    description: ExampleApiResourceDescription, events: RecordedEvents,
}
impl ExampleApiResource {
    /// Builds a new aggregate and records `ExampleApiResourceCreated`.
    pub fn create(
        id: ExampleApiResourceId, name: ExampleApiResourceName, description: ExampleApiResourceDescription,
        event_id: Uuid, occurred_at: SystemTime,
    ) -> Self;
    /// Rebuilds a stored aggregate; records nothing.
    pub fn restore(id: ExampleApiResourceId, name: ExampleApiResourceName, description: ExampleApiResourceDescription) -> Self;
    pub const fn id(&self) -> ExampleApiResourceId;
    pub const fn name(&self) -> &ExampleApiResourceName;
    pub const fn description(&self) -> &ExampleApiResourceDescription;
}
impl AggregateRoot for ExampleApiResource {}

// some-module-domain: repository.rs
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("the resource already exists")] AlreadyExists,
    #[error("the repository is unavailable")] Unavailable(#[source] BoxError),
}
#[async_trait::async_trait]
pub trait ExampleApiResourceRepository: std::fmt::Debug + Send + Sync {
    async fn save(&self, resource: &ExampleApiResource) -> Result<(), RepositoryError>;           // insert
    async fn find_by_id(&self, id: ExampleApiResourceId) -> Result<Option<ExampleApiResource>, RepositoryError>;
    async fn find_page(&self, page: PageRequest) -> Result<Vec<ExampleApiResource>, RepositoryError>;   // id DESC
}
```

```rust
// some-module-application: error.rs
#[derive(Debug, thiserror::Error)]
pub enum ExampleApiResourceError {
    #[error(transparent)] Invalid(#[from] InvalidExampleApiResource),
    #[error(transparent)] InvalidPage(#[from] InvalidPageRequest),
    #[error("ExampleApiResource {0} already exists.")] AlreadyExists(ExampleApiResourceId),
    #[error("the example API resource storage is unavailable")] Storage(#[source] BoxError),
}
impl From<ExampleApiResourceError> for Problem {}   // AD-06: Invalid/InvalidPage -> 422 with one violation;
                                                    // AlreadyExists/Storage -> Problem::internal
// No variant wraps `EventBusError`: a failed publication is logged by the command handler and is not an error (PD-18).

// some-module-application: command/create_example_api_resource.rs
#[derive(Debug, Clone)]
pub struct CreateExampleApiResource { pub id: Uuid, pub name: String, pub description: String }
impl Command for CreateExampleApiResource {
    type Error = ExampleApiResourceError;
    const NAME: &'static str = "CreateExampleApiResource";
}
#[derive(Debug)]
pub struct CreateExampleApiResourceHandler {
    repository: Arc<dyn ExampleApiResourceRepository>, event_bus: Arc<dyn EventBus>,
    clock: Arc<dyn Clock>, uuid_generator: Arc<dyn UuidGenerator>,
}
impl CreateExampleApiResourceHandler {
    pub const fn new(/* the four fields */) -> Self;
    async fn create(&self, command: CreateExampleApiResource) -> Result<(), ExampleApiResourceError>;
    // 1 build value objects (domain errors -> Invalid)   2 ExampleApiResource::create(.., uuid_generator.generate(), clock.now())
    // 3 repository.save (AlreadyExists -> AlreadyExists(id); Unavailable -> Storage)
    // 4 only after the save succeeded: event_bus.publish(resource.pull_domain_events()); on Err every failure is
    //   passed to log_publication_failure with error_chain(&*failure.source) and the result is still Ok(()) (PD-18, FR20(e))
}
#[async_trait::async_trait]
impl CommandHandler<CreateExampleApiResource> for CreateExampleApiResourceHandler {
    async fn handle(&self, command: CreateExampleApiResource) -> Result<(), ExampleApiResourceError> {
        self.create(command).await   // rule 21: the trait method only delegates
    }
}
fn log_publication_failure(failure: &SubscriberFailure, error: &str) {
    tracing::error!(event_name = failure.event_name, event_id = %failure.event_id, error = %error, "event publication failed");
}

// some-module-application: query.rs and query/*.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExampleApiResourceView { pub id: Uuid, pub name: String, pub description: String }

#[derive(Debug, Clone, Copy)]
pub struct GetExampleApiResource { pub id: Uuid }
impl Query for GetExampleApiResource {
    type Output = Option<ExampleApiResourceView>;
    type Error = ExampleApiResourceError;
    const NAME: &'static str = "GetExampleApiResource";
}
#[derive(Debug)]
pub struct GetExampleApiResourceHandler { repository: Arc<dyn ExampleApiResourceRepository> }   // + new, + inherent `get`

#[derive(Debug, Clone, Copy)]
pub struct ListExampleApiResources { pub page: u32, pub items_per_page: u32 }   // validated values, see "Pagination"
impl Query for ListExampleApiResources {
    type Output = Vec<ExampleApiResourceView>;
    type Error = ExampleApiResourceError;
    const NAME: &'static str = "ListExampleApiResources";
}
#[derive(Debug)]
pub struct ListExampleApiResourcesHandler { repository: Arc<dyn ExampleApiResourceRepository> } // + new, + inherent `list`; PageRequest::new

// some-module-application: subscriber/audit_example_api_resource_created.rs
#[derive(Debug, Clone, Copy, Default)]
pub struct AuditExampleApiResourceCreated;
#[async_trait::async_trait]
impl DomainEventSubscriber<ExampleApiResourceCreated> for AuditExampleApiResourceCreated {
    async fn handle(&self, event: &ExampleApiResourceCreated) -> Result<(), BoxError> { log_event(event); Ok(()) }
}
fn log_event(event: &ExampleApiResourceCreated) {
    tracing::info!(target: "audit", event_name = ExampleApiResourceCreated::NAME, event_id = %event.event_id, aggregate_id = %event.aggregate_id, occurred_at = %humantime::format_rfc3339_micros(event.occurred_at), "domain event");
}

// some-module-infrastructure: persistence.rs
#[derive(Debug)]
pub struct PgExampleApiResourceRepository { pool: sqlx::PgPool }
impl PgExampleApiResourceRepository {
    pub const fn new(pool: sqlx::PgPool) -> Self;
    // inherent async fns `insert`, `select_by_id`, `select_page`: query!/query_as! into a private row struct, then
    // ExampleApiResource::restore; unique violation -> AlreadyExists; a stored row that fails domain validation or
    // any other error -> Unavailable
}
impl ExampleApiResourceRepository for PgExampleApiResourceRepository { /* each method delegates (rule 21) */ }
```

The two log functions above are written on one line each, as rule 17a requires; lines that carry `%` sigils are left alone by rustfmt and may exceed 100 columns.

### HTTP layer

```rust
// shared-application: http/problem.rs
pub const PROBLEM_CONTENT_TYPE: &str = "application/problem+json";
pub const TYPE_BLANK: &str = "about:blank";
pub const TYPE_VALIDATION_FAILED: &str = "/problems/validation-failed";
pub const TYPE_DEPENDENCY_UNAVAILABLE: &str = "/problems/dependency-unavailable";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Violation { pub property_path: String, pub message: String, pub code: String }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct DependencyFailure { pub dependency: String, pub message: String }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Problem {
    #[schema(value_type = String)] pub r#type: &'static str,
    #[schema(value_type = String)] pub title: &'static str,   // StatusCode::canonical_reason
    #[schema(minimum = 100, maximum = 599)] pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")] pub detail: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub violations: Vec<Violation>,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub failures: Vec<DependencyFailure>,
}
impl Problem {
    pub fn new(status: StatusCode) -> Self;                              // type about:blank, no detail
    #[must_use] pub fn with_detail<D: Into<String>>(self, detail: D) -> Self;
    pub fn validation_failed(violations: Vec<Violation>) -> Self;        // 422; sorts by (property_path, code)
    pub fn dependency_unavailable(failures: Vec<DependencyFailure>) -> Self;   // 500; detail = messages joined with "; "
    pub fn internal(cause: &(dyn std::error::Error + 'static)) -> Self;  // logs the chain at ERROR; 500 without detail
}
impl IntoResponse for Problem {}                       // status + JSON body + content-type application/problem+json
impl From<validator::ValidationErrors> for Problem {}  // validation_failed; property paths converted to camelCase
impl From<JsonRejection> for Problem {}                // status and text from the rejection (400, 413, 415, 422)
impl From<PathRejection> for Problem {}                // 400
impl From<QueryRejection> for Problem {}               // 400
impl<E: std::error::Error + Into<Problem> + 'static> From<BusError<E>> for Problem {}   // NotRegistered -> internal

// shared-application: http/extract.rs — the only extractors handlers may use for path, query and body
// `via(Json)` and `via(Path)` name the imported types: the qualified form triggers `unused_qualifications`.
#[derive(FromRequest)]      #[from_request(via(Json), rejection(Problem))]   pub struct ApiJson<T>(pub T);
#[derive(FromRequestParts)] #[from_request(via(Path), rejection(Problem))]   pub struct ApiPath<T>(pub T);
pub struct ValidatedJson<T>(pub T);    // FromRequest: Json<T> then T::validate(); T: DeserializeOwned + Validate
pub struct ValidatedQuery<T>(pub T);   // FromRequestParts: Query<T> then T::validate()

// shared-application: http/fallback.rs and http/middleware.rs
// The two fallbacks are used as values only by other crates and by their own unit test (AD-27).
#[cfg_attr(not(test), expect(clippy::unused_async, reason = "axum handlers must be async"))]
pub async fn not_found() -> Problem;            // 404, detail "No route matches the request path."
#[cfg_attr(not(test), expect(clippy::unused_async, reason = "axum handlers must be async"))]
pub async fn method_not_allowed() -> Problem;   // 405
pub fn panic_response(panic: Box<dyn std::any::Any + Send + 'static>) -> Response;   // 500 problem, for CatchPanicLayer::custom
pub async fn request_timeout(State(budget): State<Duration>, request: Request, next: Next) -> Response;   // 503 problem on expiry

// shared-application: graphql.rs
pub fn problem_to_error(problem: Problem) -> async_graphql::Error;
// extensions.code by status: 422 -> VALIDATION_FAILED, message = detail, extensions.violations = the violations;
// everything else -> INTERNAL_SERVER_ERROR with message "Internal server error"

// some-module-application: http.rs, http/dto.rs, http/handler.rs
#[derive(Debug, Clone)]
pub struct SomeModuleState {
    pub command_bus: Arc<dyn CommandBus>, pub query_bus: Arc<dyn QueryBus>, pub uuid_generator: Arc<dyn UuidGenerator>,
}
pub fn router<S>() -> OpenApiRouter<S>
where S: Clone + Send + Sync + 'static, SomeModuleState: FromRef<S>;   // routes!(list, create), routes!(get_one)

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[schema(as = ExampleApiResource)]
pub struct ExampleApiResourceResponse { pub id: uuid::Uuid, pub name: String, pub description: String }
impl From<ExampleApiResourceView> for ExampleApiResourceResponse {}

#[derive(Debug, Clone, Deserialize, ToSchema, Validate)]
#[serde(deny_unknown_fields)]
pub struct CreateExampleApiResourceRequest { pub name: String, pub description: String }   // attributes: "API Contract"

#[derive(Debug, Clone, Copy, Deserialize, IntoParams, Validate)]
#[serde(rename_all = "camelCase", default)]
#[into_params(parameter_in = Query)]
pub struct ListExampleApiResourcesParams { pub page: i32, pub items_per_page: i32 }   // signed (AD-22)
impl Default for ListExampleApiResourcesParams { fn default() -> Self; }   // page 1, items_per_page 30: the value of a missing parameter
impl ListExampleApiResourcesParams {
    pub const fn into_query(self) -> ListExampleApiResources;   // `unsigned_abs()` of both fields; called after validation
}

pub async fn create(State(state): State<SomeModuleState>, ValidatedJson(body): ValidatedJson<CreateExampleApiResourceRequest>)
    -> Result<(StatusCode, [(HeaderName, String); 1], Json<ExampleApiResourceResponse>), Problem>;
// id = uuid_generator.generate(); dispatch CreateExampleApiResource; ask GetExampleApiResource; 201 + location
pub async fn get_one(State(state): State<SomeModuleState>, ApiPath(id): ApiPath<uuid::Uuid>)
    -> Result<Json<ExampleApiResourceResponse>, Problem>;
pub async fn list(State(state): State<SomeModuleState>, ValidatedQuery(params): ValidatedQuery<ListExampleApiResourcesParams>)
    -> Result<Json<Vec<ExampleApiResourceResponse>>, Problem>;
```

### Composition root: `service`

```rust
// cli.rs — every setting is an `Option<String>` bound to its variable; `--help` never shows a value (AD-07)
#[derive(Debug, Parser)]
#[command(name = "service", version, about)]
pub struct Cli { #[command(subcommand)] pub command: Command }
#[derive(Debug, Subcommand)]
pub enum Command {
    Serve(ServeArgs), Migrate(MigrateArgs), CheckConfig(ServeArgs),
    OpenapiExport(ExportArgs), GraphqlExport(ExportArgs), Healthcheck(HealthcheckArgs),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Subcommand)]
pub enum MigrateAction { Revert }   // `service migrate` applies; `service migrate revert` reverts
// `ServeArgs` (flattening `DatabaseArgs`), `MigrateArgs` and `HealthcheckArgs` have one field per variable, declared like:
//   #[arg(long = "database-url", env = "DATABASE_URL", hide_env_values = true)] pub url: Option<String>,
// and the two credential fields, which exist for the pair check only:
//   #[arg(long, env = "AWS_SECRET_ACCESS_KEY", hide_env_values = true, hide = true)] pub aws_secret_access_key: Option<String>,

// config.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum AppEnv { Dev, Test, Prod }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigProblem { pub variable: &'static str, pub reason: String }   // the reason never contains a value
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the configuration is invalid ({} problem(s))", .0.len())]
pub struct ConfigErrors(Vec<ConfigProblem>);
impl ConfigErrors { pub fn problems(&self) -> &[ConfigProblem]; }   // declaration order of the variables
#[derive(Debug, Clone)]
pub struct Config {
    pub app_env: AppEnv, pub host: IpAddr, pub port: u16, pub log_level: String,
    pub database: PostgresSettings, pub sqs: SqsSettings,
    pub health_probe_timeout: Duration, pub request_timeout: Duration,
    pub max_body_bytes: usize, pub shutdown_grace_period: Duration,
}
impl Config { pub fn from_args(args: ServeArgs) -> Result<Self, ConfigErrors>; }   // AD-07; the only constructor
#[derive(Debug, Clone)]
pub struct MigrateSettings { pub database: PostgresSettings, pub wait_timeout: Duration }
impl MigrateSettings { pub fn from_args(args: &MigrateArgs) -> Result<Self, ConfigErrors>; }   // AD-07: the subset of `migrate`

// http.rs
#[derive(Debug, Clone, FromRef)]
pub struct AppState { pub health_check: HealthCheckState, pub some_module: SomeModuleState }

// bootstrap.rs
#[derive(Debug)]
pub struct Adapters {
    pub repository: Arc<dyn ExampleApiResourceRepository>,
    pub probes: Vec<Arc<dyn HealthProbe>>,          // order: database, cache, message broker
    pub event_bus: Arc<dyn EventBus>,
    pub clock: Arc<dyn Clock>,
    pub uuid_generator: Arc<dyn UuidGenerator>,
}
impl Adapters {
    pub async fn connect(config: &Config) -> Result<Self, BootstrapError>;   // lazy pool, MokaCache behind CacheProbe, sqs_client,
                                                                             // event_bus(); no network access
    #[must_use] pub fn with_probe(self, probe: Arc<dyn HealthProbe>) -> Self;  // replaces the probe with the same dependency();
                                                                               // order and the other probes are kept
}
#[derive(Debug, thiserror::Error)]
pub enum BootstrapError {
    #[error("invalid database settings")] Database(#[from] sqlx::Error),
    #[error(transparent)] DuplicateHandler(#[from] DuplicateHandler),
}
/// The in-process event bus with every subscriber registered.
pub fn event_bus() -> Arc<dyn EventBus>;   // InMemoryEventBus::new().subscribe(AuditExampleApiResourceCreated)
/// Registers every handler, builds the command and query buses, the state, the GraphQL schema and the router.
pub fn router(adapters: Adapters, config: &Config) -> Result<Router, BootstrapError>;

// telemetry.rs
/// Installs the JSON subscriber on `writer` and the panic hook that writes the `panic` entry.
pub fn init<W>(directives: &str, writer: W) -> Result<(), BoxError>
where W: for<'writer> MakeWriter<'writer> + Send + Sync + 'static;   // production passes `std::io::stdout`

// command/serve.rs
pub fn run(config: &Config) -> ExitCode;          // telemetry::init(&config.log_level, std::io::stdout); 0 drained, 1 forced or failed
pub fn reject(errors: &ConfigErrors) -> ExitCode; // telemetry::init("info", std::io::stdout); one `invalid configuration` entry
                                                  // per problem; 2
// private: `async fn bind(config: &Config) -> std::io::Result<TcpListener>` carries
// #[expect(clippy::disallowed_methods, reason = "the serve command owns the listening socket")]

// openapi.rs
pub const API_VERSION: &str = "1.0.0";
#[derive(Debug, OpenApi)] pub struct ApiDoc;       // static metadata of AD-16
#[derive(Debug, thiserror::Error)]
#[error("the OpenAPI document could not be serialised")]
pub struct ExportError(#[source] BoxError);
pub fn api_router() -> OpenApiRouter<AppState>;    // ApiDoc + nest("/api", health_check_application::router())
                                                  //         + nest("/api", some_module_application::http::router())
pub fn document() -> utoipa::openapi::OpenApi;     // api_router().into_openapi()
pub fn yaml() -> Result<String, ExportError>;

// graphql/schema.rs — the doc comments become the descriptions of the root types in the SDL
/// Query root.
#[derive(Debug, MergedObject, Default)] pub struct Query(ExampleApiResourceQuery);
/// Mutation root.
#[derive(Debug, MergedObject, Default)] pub struct Mutation(ExampleApiResourceMutation);
pub type ApiSchema = Schema<Query, Mutation, EmptySubscription>;
pub fn builder() -> SchemaBuilder<Query, Mutation, EmptySubscription>;   // Schema::build(Query::default(), Mutation::default(), EmptySubscription)
pub fn sdl() -> String;                                                  // builder().finish() + AD-17 options

// graphql/handler.rs
pub const EXPLORER_CONTENT_SECURITY_POLICY: &str = "default-src 'none'; script-src https://unpkg.com 'unsafe-inline'; style-src https://unpkg.com 'unsafe-inline'; img-src data:; font-src data:; connect-src 'self'; frame-ancestors 'none'";
pub async fn execute(State(schema): State<ApiSchema>, ApiJson(body): ApiJson<serde_json::Map<String, serde_json::Value>>)
    -> Result<Json<async_graphql::Response>, Problem>;
// a string member `query` is required, then serde_json::from_value::<async_graphql::Request>(Value::Object(body));
// otherwise Problem::new(UNPROCESSABLE_ENTITY).with_detail("The request body is not a GraphQL request.")
pub async fn explorer() -> Response;   // Html(include_str!("explorer.html")) with EXPLORER_CONTENT_SECURITY_POLICY

// http/trace.rs
pub fn make_span(request: &axum::extract::Request) -> tracing::Span;   // span `http.request` of AD-13
pub fn on_response<B>(response: &axum::http::Response<B>, latency: Duration, span: &tracing::Span);
// generic over the body: with a concrete body type the middleware stack does not compile (`FromFn<..>: Service<_>` unsatisfied).
// The event is one line, in the private fn log_request_completed(status: u16, duration_ms: u64): tracing::info!(target: module_path!(), { http.response.status_code = status, duration_ms }, "request completed");

// http/router.rs
pub fn assemble(state: AppState, schema: ApiSchema, config: &Config) -> Router;
```

`assemble` builds the router in this order: REST routes from `api_router().split_for_parts()` with the state applied; `POST /api/graphql` (`execute`), plus `GET /api/graphql` (`explorer`) only when `config.app_env == AppEnv::Dev`; `GET /ping`; `GET /api/docs`, `GET /api/docs/openapi.json`, `GET /api/docs/{*file}`; `fallback(not_found)`; `method_not_allowed_fallback(method_not_allowed)`. Then one `ServiceBuilder`, outermost layer first:

| # | Layer | Effect |
| --- | --- | --- |
| 1 | `axum::middleware::from_fn(sanitize_request_id)` | Drops an inbound `x-request-id` that does not match the allowed pattern |
| 2 | `SetRequestIdLayer::new(REQUEST_ID_HEADER, MakeRequestUuid)` | Generates the id when absent |
| 3 | `PropagateRequestIdLayer::new(REQUEST_ID_HEADER)` | Copies the id to the response |
| 4 | `security_headers::layer()` | Four `SetResponseHeaderLayer::if_not_present` layers |
| 5 | `TraceLayer::new_for_http()` with `trace::make_span` and `trace::on_response`, `on_failure(())` | Request span and access log |
| 6 | `CatchPanicLayer::custom(panic_response)` | Panic becomes the 500 problem |
| 7 | `axum::middleware::from_fn_with_state(config.request_timeout, request_timeout)` | Request budget, 503 problem |
| 8 | `DefaultBodyLimit::max(config.max_body_bytes)` | Body limit read by the extractors, 413 problem |

The order guarantees that 404, 405, panic and timeout responses carry the request id and the security headers and are logged, and that a panic is reported inside the request span, so its log entry carries the request id. No layer marks headers as sensitive: the span records the method, the route, the path and the request id and no other header, and authentication is a non-goal. Verified in the validation round: the stack compiles in this order (with the sensitive-header layer the first version listed; removing a layer does not affect the others), the documentation routes answer 200 for the page, the JSON document and the five assets and the 404 problem for an unknown asset and for `/api/docs/`, a 70 kB body is answered with the 413 problem, and a panicking probe returns the 500 problem with the request id and the security headers.

### GraphQL schema types

```rust
// some-module-application: graphql.rs — every doc comment is a description in the exported SDL (`missing_docs`
// requires them, so the SDL listing in "API Contract" carries them)
/// Template example resource.
#[derive(Debug, Clone, SimpleObject)]
#[graphql(name = "ExampleApiResource")]
pub struct ExampleApiResourceObject {
    /// Identifier.
    pub id: ID,
    /// Name.
    pub name: String,
    /// Description.
    pub description: String,
}

/// Input of the create mutation.
#[derive(Debug, InputObject)]
pub struct CreateExampleApiResourceInput {
    /// Name.
    pub name: String,
    /// Description.
    pub description: String,
}

/// Query root of the example context.
#[derive(Debug, Default)] pub struct ExampleApiResourceQuery;
#[Object]
impl ExampleApiResourceQuery {
    /// Retrieves an example API resource by id.
    async fn example_api_resource(&self, ctx: &Context<'_>, id: ID) -> async_graphql::Result<Option<ExampleApiResourceObject>>;
    /// Retrieves one page of example API resources, newest first.
    async fn example_api_resources(
        &self, ctx: &Context<'_>,
        #[graphql(default = 1, desc = "Page number, from 1.")] page: i32,
        #[graphql(default = 30, desc = "Number of items per page, from 1 to 100.")] items_per_page: i32,
    ) -> async_graphql::Result<Vec<ExampleApiResourceObject>>;
}

/// Mutation root of the example context.
#[derive(Debug, Default)] pub struct ExampleApiResourceMutation;
#[Object]
impl ExampleApiResourceMutation {
    /// Creates an example API resource.
    async fn create_example_api_resource(&self, ctx: &Context<'_>, input: CreateExampleApiResourceInput)
        -> async_graphql::Result<ExampleApiResourceObject>;
}
```

Resolvers read `ctx.data::<SomeModuleState>()`, validate through the REST DTOs (`CreateExampleApiResourceRequest`, `ListExampleApiResourcesParams`) so that both transports share one rule set, call the same command and queries as the REST handlers, and convert every failure with `Problem::from(..)` followed by `problem_to_error`. An `id` argument that is not a UUID is a `VALIDATION_FAILED` error with one violation (`propertyPath` `id`, `code` `uuid`, message `This value is not a valid UUID.`). The page arguments are `i32` (GraphQL `Int`), so `page: -1`, `page: 0` and `itemsPerPage: 101` reach the DTO validation and are `VALIDATION_FAILED` errors with a `range` violation; with `u32` arguments the library rejected `-1` itself with `Failed to parse "Int"` and no `extensions` (verified on 7.2.1).

## API Contract

### REST operations

All paths are served by the service itself on `APP_PORT`. Success bodies are `application/json`; every error body is `application/problem+json` with the `Problem` schema. Every operation also documents `default` with description `Problem response.` and the `Problem` schema (required by the Spectral ruleset and used for 405, 500 and 503).

| Method and path | operationId | Tag | Request body | Parameters | Responses (status: description, schema) |
| --- | --- | --- | --- | --- | --- |
| `GET /api/health` | `api_health_get` | `HealthCheck` | none | none | `204`: `Health check passed with no content to return.`, no body. `500`: `Health check failed.`, `Problem`. |
| `GET /api/example_api_resources` | `api_example_api_resources_get` | `ExampleApiResource` | none | query `page`, `itemsPerPage` | `200`: `ExampleApiResource collection`, array of `ExampleApiResource`. `400`: `Invalid query parameter.`, `Problem`. `422`: `Validation failed.`, `Problem`. |
| `POST /api/example_api_resources` | `api_example_api_resources_post` | `ExampleApiResource` | `CreateExampleApiResourceRequest`, `application/json`, required | none | `201`: `ExampleApiResource resource created`, `ExampleApiResource`, header `location`. `400`: `Invalid input.` `413`: `Request body too large.` `415`: `Unsupported media type.` `422`: `Validation failed.` (all `Problem`). |
| `GET /api/example_api_resources/{id}` | `api_example_api_resources_id_get` | `ExampleApiResource` | none | path `id` (string, format `uuid`, description `ExampleApiResource identifier`) | `200`: `ExampleApiResource resource`, `ExampleApiResource`. `400`: `Invalid identifier.`, `Problem`. `404`: `Resource not found.`, `Problem`. |

Summaries and descriptions are set with the `summary` and `description` arguments of `#[utoipa::path]`, not taken from doc comments:

| operationId | summary | description |
| --- | --- | --- |
| `api_health_get` | `Checks the health of the service.` | `Returns 204 when the database, the cache and the message broker respond. Returns a 500 problem document that lists every failing dependency otherwise.` |
| `api_example_api_resources_get` | `Retrieves the collection of ExampleApiResource resources.` | `Returns one page of resources, newest first.` |
| `api_example_api_resources_post` | `Creates an ExampleApiResource resource.` | `Validates the input, stores the resource and returns it.` |
| `api_example_api_resources_id_get` | `Retrieves an ExampleApiResource resource.` | `Returns the resource identified by id.` |

Routes that are not part of the OpenAPI document:

| Method and path | Response |
| --- | --- |
| `GET /ping` | `200`, `text/plain; charset=utf-8`, body `pong` |
| `GET /api/docs` | `200`, `text/html; charset=utf-8`, the documentation page |
| `GET /api/docs/openapi.json` | `200`, `application/json`, the OpenAPI document |
| `GET /api/docs/{file}` | `200` with the asset's media type, or the 404 problem |
| `POST /api/graphql` | `200`, `application/json`, GraphQL response; transport errors as problems: `400` malformed JSON, `413` body over the limit, `415` wrong media type, `422` a JSON body that is not a GraphQL request (not an object, or no string member `query`) |
| `GET /api/graphql` | `APP_ENV=dev`: `200`, `text/html; charset=utf-8`, the explorer page with its own `content-security-policy` (AD-14, AD-18). Otherwise the 405 problem with `allow: POST` |

OpenAPI metadata: `openapi: 3.1.0`; `info.title` `Rust Service Template API`; `info.description` `This API provides endpoints to manage Rust Service Template data within VilnaCRM.`; `info.version` `1.0.0`; contact name `VilnaCRM Support`, e-mail `support@vilnacrm.com`, URL `https://github.com/VilnaCRM-Org/rust-service-template`; licence name `CC0-1.0`, URL `https://creativecommons.org/publicdomain/zero/1.0/`; one server `/` with description `Current host`; tags `ExampleApiResource` (`Template example resource endpoints.`) and `HealthCheck` (`Runtime health-check endpoints.`). No security scheme.

### Schemas and validation rules

| Schema | Property | Type | Constraints |
| --- | --- | --- | --- |
| `ExampleApiResource` | `id` | string, format `uuid` | required |
| | `name` | string | required; `minLength` 1; `maxLength` 255; `pattern` `^[^\u0000]*$` |
| | `description` | string | required; `minLength` 1; `maxLength` 1024; `pattern` `^[^\u0000]*$` |
| `CreateExampleApiResourceRequest` | `name` | string | as above; example `Example name` |
| | `description` | string | as above; example `Example description` |
| | (object) | | `additionalProperties: false` (from `deny_unknown_fields`) |
| `Problem` | `type`, `title` | string | required |
| | `status` | integer | required; 100 to 599 |
| | `detail` | string | optional |
| | `violations` | array of `Violation` | present only for `/problems/validation-failed` |
| | `failures` | array of `DependencyFailure` | present only for `/problems/dependency-unavailable` |
| `Violation` | `propertyPath`, `message`, `code` | string | all required |
| `DependencyFailure` | `dependency`, `message` | string | both required; `dependency` is `database`, `cache` or `message_broker` |

Field attributes of the two request DTOs (schema and validator numbers must stay equal):

```rust
pub struct CreateExampleApiResourceRequest {
    /// Display name of the resource.
    #[schema(min_length = 1, max_length = 255, pattern = r"^[^\u0000]*$", example = "Example name")]
    #[validate(
        length(min = 1, max = 255, message = "This value must contain between 1 and 255 characters."),
        does_not_contain(pattern = "\0", message = "This value must not contain the NUL character.")
    )]
    pub name: String,
    /// Free-text description of the resource.
    #[schema(min_length = 1, max_length = 1024, pattern = r"^[^\u0000]*$", example = "Example description")]
    #[validate(
        length(min = 1, max = 1024, message = "This value must contain between 1 and 1024 characters."),
        does_not_contain(pattern = "\0", message = "This value must not contain the NUL character.")
    )]
    pub description: String,
}

#[serde(rename_all = "camelCase", default)]   // a missing parameter takes its value from `impl Default`: page 1, items_per_page 30
pub struct ListExampleApiResourcesParams {
    /// Page number, starting at 1.
    #[param(minimum = 1, default = 1)]
    #[validate(range(min = 1, message = "This value must be 1 or greater."))]
    pub page: i32,
    /// Number of items per page.
    #[param(minimum = 1, maximum = 100, default = 30)]
    #[validate(range(min = 1, max = 100, message = "This value must be between 1 and 100."))]
    pub items_per_page: i32,
}
```

Both parameters are `integer` with format `int32` in the OpenAPI document. The fields are signed on purpose (AD-22): a negative value must reach the validator and be a 422, not fail to parse.

Rules:

- Lengths count Unicode scalar values (`validator` and the domain use `chars().count()`, JSON Schema counts code points, PostgreSQL `char_length` agrees). Whitespace-only values are valid. The only forbidden character is U+0000, which PostgreSQL `text` cannot store.
- Violation codes are the validator's: `length`, `does_not_contain`, `range`; plus `uuid` for a GraphQL `id` argument. `propertyPath` is the wire name of the field (`name`, `description`, `page`, `itemsPerPage`, `id`).
- A body with an unknown field, a missing field or a wrong JSON type is a 422 with `about:blank` (axum's data error); malformed JSON is 400; a missing or wrong `content-type` is 415.
- Unknown query parameters are ignored. A query parameter that is not a 32-bit integer (`page=abc`, `page=1.5`, `page=4294967296`) is 400; an integer outside its bounds (`page=0`, `page=-1`, `itemsPerPage=0`, `itemsPerPage=101`) is 422 with a `range` violation (FR22, PD-1). Unit tests cover each of these values on REST, and `page: -1`, `page: 0` and `itemsPerPage: 101` on GraphQL.

### Pagination

- Parameters: `page` (integer, minimum 1, default 1) and `itemsPerPage` (integer, 1 to 100, default 30). The names match the other VilnaCRM services (PD-17). `page` has no upper bound.
- Order: `id` descending. Ids are UUIDv7, so this is newest first and total.
- Response: a bare JSON array of at most `itemsPerPage` resources. No envelope, no total count, no `Link` header. A page past the end is `200` with `[]`. A client reads pages until one is shorter than `itemsPerPage`.
- Values outside the bounds are rejected with 422; they are never clamped.
- SQL: `ORDER BY id DESC LIMIT $1 OFFSET $2` with `limit = itemsPerPage`, `offset = (page - 1) * itemsPerPage`, both bound as `BIGINT` (the largest offset, `(2^31 - 2) * 100`, fits).

### Problem examples

```json
{"type":"about:blank","title":"Bad Request","status":400,"detail":"Failed to parse the request body as JSON: key must be a string at line 1 column 2"}
{"type":"about:blank","title":"Not Found","status":404,"detail":"No route matches the request path."}
{"type":"about:blank","title":"Not Found","status":404,"detail":"ExampleApiResource 01a1c6b5-cb86-7134-868a-7608036facfe was not found."}
{"type":"about:blank","title":"Method Not Allowed","status":405}
{"type":"about:blank","title":"Unsupported Media Type","status":415,"detail":"Expected request with `Content-Type: application/json`"}
{"type":"about:blank","title":"Unprocessable Entity","status":422,"detail":"Failed to deserialize the JSON body into the target type: extra: unknown field `extra`, expected `name` or `description` at line 1 column 37"}
{"type":"about:blank","title":"Unprocessable Entity","status":422,"detail":"The request body is not a GraphQL request."}
{"type":"/problems/validation-failed","title":"Unprocessable Entity","status":422,"detail":"The request failed validation.","violations":[{"propertyPath":"page","message":"This value must be 1 or greater.","code":"range"}]}
{"type":"/problems/validation-failed","title":"Unprocessable Entity","status":422,"detail":"The request failed validation.","violations":[{"propertyPath":"description","message":"This value must contain between 1 and 1024 characters.","code":"length"},{"propertyPath":"name","message":"This value must contain between 1 and 255 characters.","code":"length"}]}
{"type":"/problems/dependency-unavailable","title":"Internal Server Error","status":500,"detail":"Cache is not working","failures":[{"dependency":"cache","message":"Cache is not working"}]}
{"type":"about:blank","title":"Internal Server Error","status":500}
{"type":"about:blank","title":"Service Unavailable","status":503,"detail":"The request exceeded its time budget."}
```

The `detail` text of 400, 413, 415 and `about:blank` 422 responses that comes from axum is not a contract; tests assert status, media type and `type` only. The texts this service writes itself are contract: `No route matches the request path.`, `ExampleApiResource <id> was not found.`, `The request body is not a GraphQL request.`, `The request failed validation.`, `The request exceeded its time budget.` and the violation and failure messages.

### Response headers

| Header | On | Value |
| --- | --- | --- |
| `x-request-id` | every response | the accepted or generated request id |
| `x-content-type-options`, `x-frame-options`, `referrer-policy`, `content-security-policy` | every response | AD-14 |
| `content-type` | every response with a body | `application/json`, `application/problem+json`, `text/plain; charset=utf-8` or `text/html; charset=utf-8` |
| `location` | `201` of the create operation | `/api/example_api_resources/<id>` (path only, no host) |
| `allow` | every `405` | the methods of the matched route (set by axum) |

No `server` header, no CORS headers, no caching headers and no compression are produced.

### GraphQL schema

Content of `.github/graphql-spec/spec`. The file is always the output of `service graphql-export`; that output is authoritative for whitespace and for the built-in directive lines. The listing is the export of the validation skeleton with the one change of this revision (the description of `page`); the descriptions come from the doc comments that `missing_docs` requires.

```graphql
"Input of the create mutation."
input CreateExampleApiResourceInput {
  "Description."
  description: String!
  "Name."
  name: String!
}

"Template example resource."
type ExampleApiResource {
  "Description."
  description: String!
  "Identifier."
  id: ID!
  "Name."
  name: String!
}

"Mutation root."
type Mutation {
  "Creates an example API resource."
  createExampleApiResource(input: CreateExampleApiResourceInput!): ExampleApiResource!
}

"Query root."
type Query {
  "Retrieves an example API resource by id."
  exampleApiResource(id: ID!): ExampleApiResource
  "Retrieves one page of example API resources, newest first."
  exampleApiResources(
    "Number of items per page, from 1 to 100."
    itemsPerPage: Int! = 30,
    "Page number, from 1."
    page: Int! = 1
  ): [ExampleApiResource!]!
}

"Directs the executor to include this field or fragment only when the `if` argument is true."
directive @include(if: Boolean!) on FIELD | FRAGMENT_SPREAD | INLINE_FRAGMENT
"Directs the executor to skip this field or fragment when the `if` argument is true."
directive @skip(if: Boolean!) on FIELD | FRAGMENT_SPREAD | INLINE_FRAGMENT
schema {
  query: Query
  mutation: Mutation
}
```

GraphQL behaviour (FR24):

| Case | HTTP status | Response |
| --- | --- | --- |
| Operation succeeds | 200 | `data` |
| `exampleApiResource` with an unknown id | 200 | `data.exampleApiResource` is `null`, no `errors` |
| Validation failure of the input; `id` that is not a UUID; `page` or `itemsPerPage` outside its bounds | 200 | `errors[0].extensions.code` = `VALIDATION_FAILED`, `errors[0].extensions.violations` = the violations of FR7, `errors[0].message` = `The request failed validation.` |
| Internal failure (storage, missing handler) | 200 | `errors[0].extensions.code` = `INTERNAL_SERVER_ERROR`, `errors[0].message` = `Internal server error`, no other detail |
| Query text that is not a valid document for the schema (syntax error, unknown field, an `Int` outside the 32-bit range) | 200 | the library's `errors` entry |
| Body that is not a GraphQL request, malformed JSON, wrong media type, body over the limit | 422, 400, 415, 413 | problem document (AD-18) |

The schema has no Relay node field, no connection type, no update or delete mutation and no subscription; a unit test of the SDL asserts the three operation names and the absence of `node`, `Connection`, `update`, `delete` and `type Subscription`.

## Data Architecture

### Tables

One table in this release. `migrations/20261110000000_create_example_api_resources.up.sql`:

```sql
CREATE TABLE example_api_resources (
    id UUID NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    CONSTRAINT example_api_resources_pkey PRIMARY KEY (id),
    CONSTRAINT example_api_resources_name_check
        CHECK (char_length(name) BETWEEN 1 AND 255),
    CONSTRAINT example_api_resources_description_check
        CHECK (char_length(description) BETWEEN 1 AND 1024)
);
```

`migrations/20261110000000_create_example_api_resources.down.sql`:

```sql
DROP TABLE example_api_resources;
```

sqlx adds its own bookkeeping table `_sqlx_migrations`; it is not declared in a migration.

| Topic | Decision |
| --- | --- |
| Indexes | The primary-key index only. `find_page` orders by `id DESC`, which scans that index backwards. |
| Ids | Column type `UUID`. Generated by the application (`UuidV7Generator`), never by the database. Bound and read as `uuid::Uuid` through the sqlx `uuid` feature. The creation time of a row is recoverable from its UUIDv7 id (millisecond precision). |
| Timestamps | No table in this release has a timestamp column. Rule for a context that adds one: column type `TIMESTAMPTZ NOT NULL`, value written by the application from the `Clock` port and truncated to microseconds before it is stored, never a database default; the change that adds the first such column also adds one time crate to the Domain, Application and Infrastructure allow-lists and the matching sqlx feature, in a reviewed edit of `architecture.toml`. |
| Naming | Tables are plural snake_case; columns snake_case; constraints `<table>_<column>_<kind>` with kinds `pkey`, `key`, `check`, `fkey`; indexes `<table>_<columns>_idx`. |
| Queries | Three statements, all in `some-module-infrastructure::persistence`: `INSERT INTO example_api_resources (id, name, description) VALUES ($1, $2, $3)`; `SELECT id, name, description FROM example_api_resources WHERE id = $1`; `SELECT id, name, description FROM example_api_resources ORDER BY id DESC LIMIT $1 OFFSET $2`. |
| Error translation | SQLSTATE `23505` on the primary key becomes `RepositoryError::AlreadyExists`; every other error, including a pool acquire timeout, becomes `RepositoryError::Unavailable`. |

### Migration files

- Name: `<YYYYMMDDHHMMSS>_<snake_case_description>.up.sql` and `.down.sql`, created with `sqlx migrate add -r <snake_case_description>`.
- The down file must restore exactly the schema that existed before the up file. `shared-infrastructure/tests/integration/persistence.rs` applies all migrations, reverts all, re-applies all and compares the catalog (`information_schema.columns` and `pg_constraint`) after the first and the second apply.
- A migration contains DDL for one bounded context only and no data changes other than those needed by the DDL.

### Offline query metadata

- `.cargo/config.toml` sets `SQLX_OFFLINE=true` for every cargo invocation, so builds never contact a database and a runtime `DATABASE_URL` cannot break compilation (FR121). Verified: an offline build without the metadata fails with sqlx's message that names the missing query.
- `.sqlx/` at the workspace root is committed. After changing a query or a migration: apply migrations to the development database, then run `cargo sqlx prepare --workspace -- --all-targets --all-features`. Verified: the command writes three files for the three statements, in 50 s cold, although `SQLX_OFFLINE=true` is set in `.cargo/config.toml` (sqlx-cli overrides it for its own build).
- Gate (QG-11, G30): `cargo sqlx prepare --workspace --check -- --all-targets --all-features --locked` fails when `.sqlx/` is stale (verified without `--locked`: exit status 0 when current, 1 after a query change). A committed query file that no current query generates is only a warning of sqlx-cli 0.9.0 (`potentially unused queries found in .sqlx`, exit status 0), so the Make target runs the command through `scripts/check-sqlx-metadata.sh`, which fails on that warning (delivery document, target `validate-sqlx-metadata`). It needs a freshly migrated database: the test database, which `setup-test-db` has just reset.
- Side effect to know: `prepare --workspace`, with or without `--check`, sets the modification time of the source files of every workspace crate that depends on sqlx macros (read in the sqlx-cli 0.9.0 source), so the next build recompiles those crates. No file content changes and `git status` stays clean.
- Test code uses the adapters or unchecked `sqlx::query(..)`; the `query!` macros are used in production code only, so `.sqlx/` holds production queries. sqlx 0.9 rejects a SQL string built at run time (`dynamic SQL strings should be audited for possible injections`); the two statements of the end-to-end world that carry a database name are wrapped in `sqlx::AssertSqlSafe(..)` (verified), and no production code builds SQL at run time.

### Test database strategy

- The test database is `<DB_NAME>_test` (`app_test`) on the compose `database` service. `.env.test` sets `DATABASE_URL` to it. Role: `DB_USER`, which is the superuser of the local PostgreSQL container and may therefore create databases. `app_test` itself holds no test data: it carries the `_sqlx_test` bookkeeping, its URL names the server on which the per-test and per-scenario databases are created, and it is the target of the query-metadata gate.
- `setup-test-db` must, with a `DATABASE_URL` of the caller removed and the variables of `.env` and `.env.test` exported: refuse with exit status 2, before it calls `sqlx`, a database whose name does not end in `_test`; then run `sqlx database reset -y -f --source migrations` (drop `app_test` with force, create it, apply every migration; 0.14 s measured). Nothing else. It requires the `database` service to be healthy and never touches the development database `app`.
- Every test that writes to PostgreSQL has its own database (NFR29, FR55(d)):
  - *Adapter crates.* Integration tests are declared `#[sqlx::test(migrator = "shared_infrastructure::persistence::MIGRATOR")]`. sqlx reads `DATABASE_URL`, creates a fresh database per test next to `app_test`, applies the migrations, passes a `PgPool` to the test, and drops the database when the test passes. Its bookkeeping lives in schema `_sqlx_test` of `app_test`. Databases left by failed tests are removed by sqlx on the next run. Verified in two adapter crates with `DATABASE_URL` from `.env.test`.
  - *`service` integration tests.* Every test that touches PostgreSQL takes its database from `#[sqlx::test]` in the same way (`migrations = false` for `migrate.rs`, which applies and reverts the migrations itself). The helper `support::database_url(&pool)` returns the URL of that database: the name from `SELECT current_database()`, the server part from `DATABASE_URL`. The test passes the URL to the spawned binary as `DATABASE_URL`, or assigns it to `config.database.url` for the in-process test of `bootstrap.rs`. Verified in this revision: the whole `service` integration suite of the skeleton (17 tests, among them the in-process wiring test and the migration round trip) passes in 3.0 s against PostgreSQL and LocalStack.
  - *End-to-end suite.* The world creates one database per scenario, lazily before the first request of the scenario: `CREATE DATABASE "e2e_<process id>_<counter>"` on the server of `DATABASE_URL`, then `MIGRATOR.run` on a connection to it; it builds its `Config` with that URL for every request of the scenario. The runner's `after` hook (`Cucumber::after`, which also runs for failed scenarios) drops the database with `DROP DATABASE IF EXISTS .. WITH (FORCE)`. At start-up the runner drops every database whose name starts with `e2e_`, which removes what a killed run left behind; consequently two end-to-end runs must not share one database server at the same time. Verified in this revision: 5 features, 24 scenarios, 97 steps pass in 5 s with 8 concurrent scenarios, and no `e2e_` database remains afterwards.
- No test reads or writes the development database `app` (FR53). A test command started without the variables of `.env.test` must not be relied on: sqlx's test support loads `.env` on its own when `DATABASE_URL` is unset (its documented behaviour; not run here) and would then keep its bookkeeping and the per-test databases next to `app`. The Make targets always export `.env.test` (contract below), and the guidance tells contributors to run test levels through `make`.
- "Unreachable database" cases use the URL `postgres://app:unused@127.0.0.1:1/app` and never stop a container.

## Configuration

### Environment variables

"Required" means the command fails with exit status 2 when the variable is missing. Durations use the `humantime` syntax (`500ms`, `2s`, `1m`).

| Name | Type | Default | Required | Consumer | PHP name |
| --- | --- | --- | --- | --- | --- |
| `APP_ENV` | `dev`, `test` or `prod` | `prod` | no | `service` (explorer on in `dev` only); compose | `APP_ENV` |
| `APP_HOST` | IP address | `0.0.0.0` | no | `service serve`; `service healthcheck` (the address it probes) | none (Caddy) |
| `APP_PORT` | integer 0 to 65535 (0 = any free port, for tests) | `8081` | no | `service serve`, `service healthcheck`; compose | listener `:8081` in the Caddyfile |
| `LOG_LEVEL` | `tracing` filter directives | `info` | no | `service serve` (the other commands write plain text) | none |
| `DATABASE_URL` | `postgres://` or `postgresql://` URL | none | yes for `serve`, `migrate`, `migrate revert`, `check-config` | `service`; `sqlx-cli`; `#[sqlx::test]`; the end-to-end world | `DB_URL` |
| `DATABASE_MAX_CONNECTIONS` | integer 1 to 100 | `10` | no | `service` | none |
| `DATABASE_ACQUIRE_TIMEOUT` | duration | `2s` | no | `service` | none |
| `DATABASE_STATEMENT_TIMEOUT` | duration from 1 ms to 24 days (otherwise `must be a duration from 1ms to 24d`) | `5s` | no | `service` (every pool connection, `service migrate` included) | none |
| `DATABASE_WAIT_TIMEOUT` | duration | `30s` | no | `service migrate` (one attempt per second until it elapses) | entrypoint loop, 60 tries |
| `AWS_REGION` | string | none | yes for `serve`, `check-config` | `service` | `AWS_SQS_REGION` |
| `AWS_ENDPOINT_URL_SQS` | `http://` or `https://` URL | unset (regional AWS endpoint) | no | `service` | `AWS_SQS_ENDPOINT_BASE` + `LOCALSTACK_PORT` |
| `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` | string | unset (the provider's default credential chain: role credentials) | no; both set or both unset | validated as a pair by `service serve` and `check-config`; the values are read by the AWS SDK credential chain and by nothing else | `AWS_SQS_KEY`, `AWS_SQS_SECRET` |
| `SQS_CONNECT_TIMEOUT` | duration | `500ms` | no | `service` | none |
| `SQS_OPERATION_TIMEOUT` | duration | `2s` | no | `service` (attempt and operation timeout) | none |
| `SQS_MAX_ATTEMPTS` | integer 1 to 5 | `1` | no | `service` | none |
| `HEALTH_PROBE_TIMEOUT` | duration | `3s` | no | `service` | none |
| `HTTP_REQUEST_TIMEOUT` | duration | `10s` | no | `service` | none |
| `HTTP_MAX_BODY_BYTES` | integer 1 024 to 10 485 760 | `65536` | no | `service` | none |
| `SHUTDOWN_GRACE_PERIOD` | duration | `10s` | no | `service` | none |
| `DB_VERSION` | string | `16` | no | compose | `DB_VERSION` |
| `DB_HOST` | host name | `localhost` | no | `.env` interpolation; compose sets `database` inside containers | host part of `DB_URL` |
| `DB_PORT` | integer | `5432` | no | compose (the host port of `database`); `.env` interpolation; compose sets `5432`, the port inside the compose network, in the `app` and `migrate` containers, so a changed host port does not change their URL | `DB_PORT` |
| `DB_NAME`, `DB_USER`, `DB_PASSWORD` | string | `app`, `app`, `local-dev-only` | no | compose; `.env` interpolation | same names |
| `LOCALSTACK_HOST` | host name | `localhost` | no | `.env` interpolation; compose sets `localstack` inside containers | `AWS_SQS_ENDPOINT_BASE` |
| `LOCALSTACK_PORT` | integer | `4566` | no | compose (the host port of `localstack`); `.env` interpolation; compose sets `4566`, the port inside the compose network, in the `app` container | `LOCALSTACK_PORT` |
| `STRUCTURIZR_PORT` | integer | `8080` | no | compose | `STRUCTURIZR_PORT` |
| `GITHUB_TOKEN` | token | unset | no | compose: the source of the build secret `github_token` of the development image; raises the rate limit of the tool downloads | none |
| `TEST_SEED` | unsigned 64-bit integer | `20261110`, the seed recorded in `.cargo/config.toml` (FR51) | no | `test_support::seed::TestRng` | none (PHP data was unseeded) |
| `PROPTEST_RNG_SEED` | unsigned 64-bit integer | `20261110`, recorded in `.cargo/config.toml` | no | property tests (`proptest` reads it) | none |
| `SQLX_OFFLINE` | `true` | `true` from `.cargo/config.toml` | no | sqlx macros at build time | none |
| `SWAGGER_UI_DOWNLOAD_URL` | `vendored-only` | `vendored-only` from `.cargo/config.toml` | no | the build script of `utoipa-swagger-ui` (FR8: ignored by the vendored build; without the `vendored` feature the build stops instead of downloading) | none |

Cross-field rules checked by `Config::from_args`: `HEALTH_PROBE_TIMEOUT >= DATABASE_ACQUIRE_TIMEOUT`; `HEALTH_PROBE_TIMEOUT >= SQS_OPERATION_TIMEOUT`; `HTTP_REQUEST_TIMEOUT > HEALTH_PROBE_TIMEOUT`; `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` both set or both unset (the error names the missing one). Every default is at or below its PRD ceiling (NFR2, NFR4, NFR5, PD-15: acquire 2 s, statement 5 s and SQS operation 2 s against 5 s each; probe 3 s against 3 s; request budget 10 s and grace period 10 s against 30 s; body limit 65 536 bytes against 1 MiB; database wait 30 attempts against 60). The ceilings bound the defaults; a configured value above one is accepted (FR13).

The two seed variables are test settings, not service settings: cargo passes them to every test process from `[env]` of `.cargo/config.toml`, and a value set by the caller wins (verified under `cargo test` and `cargo nextest run`). The scheduled workflow sets both to a fresh value (delivery document).

Not carried from PHP: `APP_SECRET`, `AWS_SQS_VERSION`, `EMAIL_QUEUE_NAME`, `MESSENGER_TRANSPORT_DSN`, `CADDY_MERCURE_JWT_SECRET`, `SERVER_NAME`, `APP_RUNTIME_HTTP_ENV`, `XDEBUG_MODE`, `KERNEL_CLASS`, `PANTHER_*`, `SYMFONY_DEPRECATIONS_HELPER`, `LOAD_TEST_CONFIG`, `HTTP_PORT`, `HTTPS_PORT`, `HTTP3_PORT`.

### Committed environment files

Both files are tracked and are not git-ignored. They contain placeholders for the local compose stack only. `${NAME}` references are resolved by `dotenvy`, by Docker Compose and by `scripts/with-env.sh` (delivery document), and in all three a value already present in the process environment wins (verified for dotenvy 0.15.7 and Compose 5.6.1: `${DB_HOST}` and `${DB_PORT}` are taken from the process environment first, and an explicit `DATABASE_URL` wins). That is how the same file works on a host (`localhost`) and inside a container (compose sets `DB_HOST=database` and `LOCALSTACK_HOST=localstack` and the in-network ports `DB_PORT=5432` and `LOCALSTACK_PORT=4566`, so a host port changed in `.env` changes only what compose publishes). Compose must not inject `DATABASE_URL`, `AWS_ENDPOINT_URL_SQS`, `APP_ENV` or `LOG_LEVEL` into the development container, because an injected value would win over `.env.test` in the test targets. Sourcing a file in a shell (`set -a; . ./.env`) overrides the caller's values and must not be used. `scripts/with-env.sh` reads a value as dotenvy 0.15.7 and Compose read it for the forms on which the two agree: a value in double quotes loses the quotes (`\"`, `\\` and `\n` are unescaped, `${NAME}` is expanded), a value in single quotes loses the quotes and stays literal, an unquoted value ends at whitespace followed by `#`, and an `export ` prefix is accepted. A form that the two read differently (an unquoted value with whitespace, a quote or a backslash, a `$` that does not start `${NAME}`, an unknown escape, an unclosed quote, a comment right after the `=`) makes it exit 2 with `with-env.sh: invalid value in <file>: <line>`, so a committed value has one meaning in all three readers.

`.env` (development):

```dotenv
# Development defaults for the local compose stack and for `cargo run`.
# LOCAL-ONLY placeholders. Nothing in this file is a secret; never reuse a value outside a developer machine.
APP_ENV=dev
APP_PORT=8081
LOG_LEVEL=info

DB_VERSION=16
DB_HOST=localhost
DB_PORT=5432
DB_NAME=app
DB_USER=app
DB_PASSWORD=local-dev-only
DATABASE_URL=postgres://${DB_USER}:${DB_PASSWORD}@${DB_HOST}:${DB_PORT}/${DB_NAME}

LOCALSTACK_HOST=localhost
LOCALSTACK_PORT=4566
AWS_REGION=us-east-1
AWS_ACCESS_KEY_ID=test
AWS_SECRET_ACCESS_KEY=test
AWS_ENDPOINT_URL_SQS=http://${LOCALSTACK_HOST}:${LOCALSTACK_PORT}

STRUCTURIZR_PORT=8080
```

`.env.test` (test levels that need services). It holds only the values that differ from `.env`, so each value is defined in one place (FR90); it is always read after `.env`:

```dotenv
# Test overrides, applied on top of .env: the same local stack, a separate database.
# LOCAL-ONLY placeholders. Nothing in this file is a secret.
APP_ENV=test
LOG_LEVEL=warn
DATABASE_URL=postgres://${DB_USER}:${DB_PASSWORD}@${DB_HOST}:${DB_PORT}/${DB_NAME}_test
```

Contract for the Makefile (delivery document): the integration, end-to-end, coverage, mutation and `setup-test-db` commands run with the variables of `.env` and then of `.env.test` exported, a later file overriding an earlier one and a variable the caller has already set overriding both, except `DATABASE_URL`, which the test targets remove from the caller's environment first, so that they always reach the test database of `.env.test` (a caller's `DB_HOST`, `DB_PORT` and `DB_NAME` still apply through it); the Makefile does not `include` either file. The binary loads `.env` itself, so `cargo run -p service -- serve` works on a host without any wrapper. The production image contains no env file. A spawned test binary never reads `.env`: it runs with a cleared environment in a directory outside the repository ("Test Architecture").

### Binary `service`: subcommands and exit statuses

The seven commands of FR2 (the PRD names `migrate` and `check-config`; the others are named here):

| Command | Arguments | Behaviour | Output | Exit status |
| --- | --- | --- | --- | --- |
| `service serve` | every service variable of the table as `--kebab-case` flag or environment variable (the two AWS credential variables through the environment only) | Validates the configuration, starts logging, binds `APP_HOST:APP_PORT`, serves until SIGTERM or SIGINT, drains | JSON log lines on stdout, nothing on stderr | 0 after a signal when every in-flight request finished; 1 when the grace period ended with a connection still open, on a bind failure or on a runtime failure; 2 invalid configuration |
| `service migrate` | database variables; `--wait-timeout` / `DATABASE_WAIT_TIMEOUT` | Waits for the database, applies all pending migrations; changes nothing when none is pending | text: `waiting for database (attempt <n>): <error>` on stderr per failed attempt; `applied <n> migration(s)` on stdout | 0; 1 database unreachable after the wait (the connection error is printed) or migration failure; 2 invalid configuration |
| `service migrate revert` | same | Reverts the most recently applied migration; changes nothing when none is applied | text: `reverted <version>` or `nothing to revert` on stdout | as `migrate` |
| `service check-config` | same as `serve` | Validates the configuration of `serve` without opening a socket or contacting a dependency | `configuration OK` on stdout; one line `<VARIABLE>: <reason>` per problem on stderr | 0; 2 |
| `service openapi-export` | `--output <path>` (optional) | Writes the OpenAPI YAML to the path or to stdout; reads no configuration | the document | 0; 1 write failure |
| `service graphql-export` | `--output <path>` (optional) | Writes the GraphQL SDL to the path or to stdout; reads no configuration | the document | 0; 1 write failure |
| `service healthcheck` | `--port` / `APP_PORT`; `--host` / `APP_HOST` (value hidden) | Sends `GET /ping` to `<host>:<port>`, where the host is the address of `APP_HOST`, the loopback address of the same family when that is `0.0.0.0` or `::`, and `127.0.0.1` when `APP_HOST` is unset, with a 2 s connect and read timeout over a plain `std::net::TcpStream`; succeeds on status 200 | none | 0 alive; 1 otherwise |
| `service --help`, `service --version` | none | clap output; the version is the release version (`CARGO_PKG_VERSION`); no value of a variable is printed | text | 0 |

An unknown command or flag is a clap usage error with exit status 2, printed by clap before any command runs. A configuration problem never contains the value of a variable. A bind or runtime failure of `serve` is one JSON entry `serve failed` at ERROR with the field `error`; a failure of another command is one line `error: <message>` on stderr. `rollback` of the PRD command table is `service migrate revert`; the liveness probe is `service healthcheck`.

Verified in the validation round by 10 process-level tests of the skeleton (1.4 s): `serve` on `APP_PORT=0`, SIGTERM through `kill -TERM` with exit status 0, port in use with exit status 1, `check-config` 0 and 2, both exports, `healthcheck` 0 and 1, an unreachable database with exit status 1. Verified in this revision: the forced exit status 1, the JSON form of configuration and bind errors of `serve`, and the migration round trip on a per-test database.

## Observability and Operations

### Log line schema

`serve` writes one JSON object per line on stdout and nothing else on stdout or stderr (NFR31); the other commands write plain text. Keys:

| Key | Present | Content |
| --- | --- | --- |
| `timestamp` | always | RFC 3339, UTC, microseconds |
| `level` | always | `ERROR`, `WARN`, `INFO`, `DEBUG`, `TRACE` |
| `message` | always | fixed text from the list below |
| `target` | always | Rust module path, or `audit` for audit entries |
| event fields | per message | flattened into the top level |
| `span` | inside a request | the innermost open span: `{"name":"http.request","http.request.method":…,"http.route":…,"url.path":…,"request_id":…}` of `http.request`, or a span opened inside the request |
| `spans` | inside a request | every open span, outermost first; the first element is always the `http.request` span with the `request_id` |

Fixed messages and their fields:

| Message | Level | Fields | Written by |
| --- | --- | --- | --- |
| `invalid configuration` | ERROR | `variable`, `reason` (one entry per problem; `serve` then exits 2) | `service::command::serve` |
| `listening` | INFO | `addr`, `env`, `version` (start-up marker; `env` is the mode of FR1) | `service::command::serve` |
| `request completed` | INFO | `http.response.status_code`, `duration_ms` (the access log, one per request) | `service::http::trace` |
| `request failed` | ERROR | `error` (the error chain behind a 5xx problem) | `shared_application::http::problem` |
| `panic` | ERROR | `panic` (message and location) | `service::telemetry` (panic hook) |
| `health probe failed` | WARN | `dependency`, `cause` | `health_check_application::service` |
| `event publication failed` | ERROR | `event_name`, `event_id`, `error` (one entry per failing subscriber; the request still succeeds, PD-18) | the command handler that published |
| `domain event` | INFO, target `audit` | `event_name`, `event_id`, `aggregate_id`, `occurred_at` | the audit subscriber |
| `shutdown signal received` | INFO | `signal` | `service::command::serve` |
| `grace period elapsed, forcing shutdown` | WARN | `grace_period` (`serve` then exits 1) | `service::command::serve` |
| `stopped` | INFO | none (`serve` then exits 0) | `service::command::serve` |
| `serve failed` | ERROR | `error` (bind or runtime failure; `serve` then exits 1) | `service::command::serve` |

Each message is written by exactly one private function `log_<event>` (rule 17a), and each such function has a test that asserts level, message and fields (rule 6a). The event bus logs nothing. `migrate` has no log messages: its three texts are in the subcommand table.

Example of an access-log line and of an audit line (wrapped here; each is one line):

```json
{"timestamp":"2026-11-10T18:26:05.318149Z","level":"INFO","message":"request completed","http.response.status_code":201,
 "duration_ms":4,"target":"service::http::trace","span":{"name":"http.request","http.request.method":"POST",
 "http.route":"/api/example_api_resources","url.path":"/api/example_api_resources","request_id":"b2e86631-0be4-4080-a320-5c43459dbfea"},
 "spans":[{"name":"http.request","http.request.method":"POST","http.route":"/api/example_api_resources",
 "url.path":"/api/example_api_resources","request_id":"b2e86631-0be4-4080-a320-5c43459dbfea"}]}
{"timestamp":"2026-11-10T18:26:05.317902Z","level":"INFO","message":"domain event","event_name":"example_api_resource.created",
 "event_id":"01a1c6b5-cb86-7134-868a-7608036facff","aggregate_id":"01a1c6b5-cb86-7134-868a-7608036facfe",
 "occurred_at":"2026-11-10T18:26:05.317880Z","target":"audit","span":{"name":"http.request","http.request.method":"POST",
 "http.route":"/api/example_api_resources","url.path":"/api/example_api_resources","request_id":"b2e86631-0be4-4080-a320-5c43459dbfea"},
 "spans":[{"name":"http.request","http.request.method":"POST","http.route":"/api/example_api_resources",
 "url.path":"/api/example_api_resources","request_id":"b2e86631-0be4-4080-a320-5c43459dbfea"}]}
```

### Operations

- **Log level.** `LOG_LEVEL` takes `tracing_subscriber::EnvFilter` directives: a level (`debug`) or per-target levels (`info,sqlx=warn,audit=info`). An invalid directive is a configuration error. Request and response bodies, request headers other than the request id, and `DATABASE_URL` are never logged.
- **Request id.** Header `x-request-id`; accepted pattern and generation as in AD-13; present on every response and in the first element of `spans` of every log line of the request.
- **Access log.** Exactly one `request completed` line per request, including `/ping`, 404 and 405.
- **Panics.** The panic hook installed by `telemetry::init` writes the `panic` line inside the request span, so it carries the request id; `CatchPanicLayer` answers with the 500 problem; the process keeps running. With the production telemetry on a captured writer a panicking probe yields three lines, `panic`, `request failed` (ERROR, written by `Problem::internal` for every 5xx) and `request completed`, all JSON and all with the request id (`tests/integration/panic.rs`).
- **Shutdown sequence.** t = 0: SIGTERM or SIGINT, line `shutdown signal received`; the listener is closed and new connections are refused (measured: within 10 ms of that line). In-flight requests continue. When the last one completes: line `stopped`, exit 0 (an idle service exits within 2 s). At t = `SHUTDOWN_GRACE_PERIOD` (default 10 s): line `grace period elapsed, forcing shutdown`, the remaining connections are dropped, exit 1 within 2 s (measured: 0.25 s after the period ended). A container orchestrator must allow at least 15 s before SIGKILL. An exit status 1 after a deploy therefore means that a request was cut.
- **Container healthcheck.** Exec form `["/app/service", "healthcheck"]` (the image has no shell); interval 10 s, timeout 3 s, retries 3, start period 10 s. It probes liveness (`/ping`), so a dependency outage never restarts a healthy process. Readiness for load balancers is `GET /api/health`.
- **Start-up.** `serve` does not wait for dependencies: the pool is lazy and `/api/health` reports their state. `migrate` is a separate step that waits for the database (`DATABASE_WAIT_TIMEOUT`, one attempt per second, at most 30 attempts by default, below the ceiling of 60).

## Implementation Patterns and Consistency Rules

Rules are numbered so a review can cite them. "Must" rules are checked by a gate where one is named.

### Naming

| Thing | Rule | Example |
| --- | --- | --- |
| Layer crate | `<context>-<layer>`, kebab-case; directory `crates/[<group>/]<context>/<layer>` (gate: `arch::path`) | `some-module-domain` |
| Module and file | snake_case; one module per file; `foo.rs` plus directory `foo/` for children, never `mod.rs` (gate: `clippy::mod_module_files`) | `value_object/name.rs` |
| Top-level module of a layer crate | the names listed in the schema notes of `architecture.toml`; a context adds another name only when none of them fits (review rule, no gate) | `command`, `http` |
| Aggregate, value object, event | `UpperCamelCase`; event names are past tense; value objects are prefixed with the aggregate name | `ExampleApiResourceCreated`, `ExampleApiResourceName` |
| Event wire name | `<aggregate_snake_case>.<past_tense_verb>` | `example_api_resource.created` |
| Command, query | imperative verb plus aggregate; handler is `<Message>Handler`; file is the snake_case message name | `CreateExampleApiResource`, `get_example_api_resource.rs` |
| Port, adapter | port is a noun without prefix or suffix; adapter is prefixed with its technology | `ExampleApiResourceRepository`, `PgExampleApiResourceRepository`, `MokaCache` |
| Subscriber | `<Purpose><EventName>` | `AuditExampleApiResourceCreated` |
| DTO | `<Resource>Response`, `<Verb><Resource>Request`, `<Verb><Resources>Params`; JSON fields camelCase | `CreateExampleApiResourceRequest` |
| REST path, operationId | plural snake_case resource under `/api`; `api_<path with _>_<method>` | `/api/example_api_resources`, `api_example_api_resources_id_get` |
| GraphQL | type = resource name; fields camelCase; input `<Verb><Resource>Input` | `createExampleApiResource` |
| Test function | a snake_case sentence stating the behaviour, without a `test_` prefix | `rejects_a_name_longer_than_255_characters` |
| Test double | `Fixed…`, `Sequence…`, `Stub…`, `Fake…`, `Recording…`, `InMemory…` | `StubProbe` |
| SQL | plural snake_case tables, snake_case columns, named constraints (see "Data Architecture") | `example_api_resources_pkey` |
| Environment variable | `SCREAMING_SNAKE_CASE`, prefixed by its area | `DATABASE_ACQUIRE_TIMEOUT` |
| Cargo features | none in workspace crates | |

### Structure and tests

1. Unit tests live in a sibling file: `foo.rs` ends with `#[cfg(test)] mod tests;` and the tests are in `foo/tests.rs`. Inline `mod tests { .. }` blocks are forbidden (they would be measured by coverage).
2. A unit test opens no socket, needs no container and no network, reads no file outside the crate, and reads time only through a `Clock` double or tokio's paused clock (`#[tokio::test(start_paused = true)]`). The only processes a unit test may start are `cargo metadata` and `git` on fixtures, in `xtask`. The clippy ban list enforces the socket and wall-clock part (rule 20, NFR28).
3. Tests that need a database, LocalStack, real time, a socket or the built binary live in `tests/integration/` of the crate, one module per subject, declared in `tests/integration/main.rs`. Elapsed real time is measured with `tokio::time::Instant`, which is not on the ban list and reads the real clock outside a paused runtime.
3a. In the root file of a test target (`tests/integration/main.rs`, `tests/e2e/main.rs`) every module is declared `#[cfg(test)] mod <name>;`. `cfg(test)` is set for every test target, also with `harness = false`, and the attribute makes clippy treat the helper functions of those modules as test code, so the relaxations of `clippy.toml` (`allow-unwrap-in-tests` and the others) apply and no suppression is needed for `unwrap`, `expect`, `panic!` or indexing in helpers (verified on the `service` integration tests and on `e2e-tests`: QG-03 exits 0 with no crate-level or module-level `expect`). The `main` function of `tests/e2e/main.rs` is the only code outside such a module and unwraps nothing.
4. Every behaviour needs a test in the crate that owns the code: mutants are judged by the tests of their own crate, and coverage counts the unit and integration suites only (AD-24). A behaviour that an end-to-end scenario exercises still needs a unit or integration test.
5. Random test data comes from `test_support::seed::TestRng` only. A test binds its generator as the first statement of the test function (`let mut rng = TestRng::from_env();`) and keeps it until the test ends; helpers take `&mut TestRng` and never create one. Assertions never depend on a generated value except by comparing with the value that was generated.
6. A test double is added to `test-support` only when a second crate needs it; every method of every double is exercised by at least one test.
6a. Every function that writes a log entry has a test that asserts level, message and fields: through `test_support::log::LogRecorder` in unit tests, or through the JSON lines of the spawned binary in the `service` integration tests. Without such a test the mutant "replace the function with `()`" survives.
7. `main.rs` contains only the call into the library.
8. Imports are grouped std, third-party, workspace crates, `crate`/`super`, separated by blank lines.

### Error handling

9. Production code never calls `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, `unreachable!`, indexes a slice, or uses `as` conversions (gate: clippy). Use `?`, `ok_or`, `get`, `try_from`.
10. A fallible function returns a `thiserror` type of its layer. Lower-level errors are wrapped as `#[source]`, not formatted into the message. `BoxError` is used only at port boundaries.
11. Infrastructure translates driver errors into the domain error of the port it implements; no sqlx or SDK type crosses a port.
12. Handlers return `Result<_, Problem>` and use `?`. A 5xx problem is created only through `Problem::internal`. No handler writes an error body by hand.
13. Error messages, problem details and log fields never contain SQL text, driver messages, file paths, stack traces or secrets; a `Config` or a `PostgresSettings` is never formatted or logged as a whole.

### Lint suppression

14. `#[allow(..)]` is forbidden. A suppression is `#[expect(clippy::<lint>, reason = "<why this lint does not apply here>")]` on the smallest item that needs it. Each attribute names exactly one lint, never a lint group (NFR16). The reason states a fact about the code, for example `reason = "axum handlers must be async"`; "false positive" is not a reason. An `expect` is added only where the lint really fires: an unfulfilled expectation is itself a finding. An inline directive of shellcheck, hadolint, yamllint, markdownlint, ESLint or zizmor follows the same rule: it names exactly one rule and carries a reason, after it where the linter accepts one or on the comment line above (G31 rule (g), delivery document).
15. Module-level `#![expect(..)]` is allowed in exactly one case: a file whose derive macro generates the offending code where no item-level attribute can reach it (`service/src/graphql/schema.rs`, AD-18). No crate root (`lib.rs`, `main.rs`, `build.rs`, the `main.rs` of a test, example, bench or binary target, every `*.rs` file directly in `src/bin/`, `tests/`, `examples/` or `benches/` of a package, which cargo discovers as crate roots, and every file that a manifest names as one: the `path` of `[lib]`, `[[bin]]`, `[[test]]`, `[[example]]` and `[[bench]]` and the `build` string of `[package]` or of the legacy `[project]`) carries an `#![expect(..)]`. Helper functions of test targets need no suppression for the test relaxations (rule 3a). Cucumber fixes the signature of a step function (`&mut World`, owned captures); a step that does not mutate the world carries `#[expect(clippy::needless_pass_by_ref_mut, reason = "cucumber fixes the step signature")]`, and a step that does not consume a capture carries a second attribute for `clippy::needless_pass_by_value`, both on the function itself, below the step attribute (verified: item-level expectations are fulfilled on the `then` steps of the skeleton and QG-03 exits 0; the run used one attribute for both lints, which the one-lint-per-attribute rule splits in two).
16. Thresholds in `clippy.toml` and lint levels in `Cargo.toml` are never changed to make code pass; the code is refactored.

### Logging

17. Log through `tracing` macros with a fixed message and structured fields. No string interpolation in the message.
17a. A tracing event macro (`trace!` to `error!`) is the only statement of a private function `fn log_<event>(..)`, with no branch, no `.await` and no second event, and the macro call is written on one source line: `fn log_failure(dependency: Dependency, cause: &str) { tracing::warn!(dependency = dependency.identifier(), cause = %cause, "health probe failed"); }`. Long values are bound to locals or passed as parameters. Reasons, both measured: with tracing's `log` feature active one event costs 7 of the 8 points of cognitive complexity, and a multi-line event leaves its field lines at coverage count 0 even when it is emitted. A line that carries `%` or `?` sigils is left alone by rustfmt and may exceed 100 columns (a 112-column line passed `cargo fmt --check`); rustfmt wraps an argument list it can parse as soon as the list is longer than its `fn_call_width` (60 characters), even when the line stays within 100 columns; an event with dotted field names keeps one line through an explicit `target:` and a braced field list, which rustfmt leaves alone, for example `tracing::info!(target: module_path!(), { http.response.status_code = status, duration_ms }, "request completed");` (a plain `target:` followed by a dotted field does not compile). Span macros are not affected.
18. `println!`, `eprintln!` and `dbg!` are forbidden in production code (gate: clippy `print_stdout`, `print_stderr`, `dbg_macro`). Command output is written with `writeln!` on a `stdout`/`stderr` handle, and only in `service::command`, `xtask::cli` and `test_support::seed` (the seed line). The build script of `shared-infrastructure` prints its `cargo:` directive with `println!`; the two print lints do not apply to build scripts (verified).
19. Domain crates do not log, and neither does the event bus. An error is logged once, where it is converted to a response or swallowed: `Problem::internal` for a 5xx, the command handler for a failed event publication.
20. Banned APIs (gate: clippy `disallowed_methods`, list in `clippy.toml`). Each of these items carries one item-level `#[expect(clippy::disallowed_methods, reason = ..)]` and no other code may:
    - the process environment is read only by clap (`#[arg(env = ..)]`) in `service::cli` and `xtask::cli`, by `test_support::seed::TestRng::from_env` (through its private helper `read_seed`, which carries the `expect`), and by one helper function in the `support.rs` of each test target that needs the test stack (`reason = "the test stack is described by the environment"`);
    - the wall clock is read only by `SystemClock::now`;
    - UUIDs are generated only by `UuidV7Generator::generate`;
    - sockets are opened only by `service::command::serve::bind`, by `service::command::healthcheck::probe` and by the socket helpers of `service/tests/integration/support.rs`.

### Code shape imposed by the tools

21. The body of a method in an `#[async_trait] impl` is a single delegation to an inherent `async fn` of the same type, for example `async fn handle(&self, command: CreateExampleApiResource) -> Result<(), ExampleApiResourceError> { self.create(command).await }`. Logic never lives directly in the trait method: llvm-cov records no line counts inside such a body and clippy reports no complexity finding there, so the coverage floor and the complexity gate would pass without looking. Synchronous trait methods (`Clock::now`, `HealthProbe::dependency`) are not affected.
22. Every public type implements `Debug` (`missing_debug_implementations`): by derive where every field is `Debug`, which is why ports have `Debug` as a supertrait; by hand, with a unit test, where a field cannot be (`InMemoryCommandBus`, `InMemoryQueryBus`, `InMemoryEventBus`) or must not be printed (`MokaCache`).
23. Every line must be reachable by a unit or integration test (100% lines, AD-24), which constrains how error paths are written:
    - an error that cannot occur in the binary (the logging subscriber is already installed; the first handler registered on a new bus is a duplicate) is propagated with `?` on the line of the call and never handled on a line of its own. Measured in this revision: `if let Err(error) = telemetry::init(..) { return crate::failure(error); }` left two lines of `command/serve.rs` uncovered; with `telemetry::init(..)?;` inside a function that returns `Result` the file is at 100% lines from the unit and integration suites alone;
    - a `?` must share its line with code that runs: when rustfmt leaves `)?);` on a line of its own and the error never occurs in a test, llvm-cov counts that line as uncovered (observed on `bootstrap.rs`); bind long arguments to locals so that `bus.register(handler)?` fits on one line;
    - an output error of a command (`writeln!` to stdout fails) is reached in a process-level test by giving the child `/dev/full` as its standard output.

### Recipes

**Add a REST endpoint.** (1) Add or reuse a command or query in the context's Application crate. (2) Add the DTO to `http/dto.rs` with `ToSchema`, serde and validator attributes. (3) Add the handler to `http/handler.rs` with `#[utoipa::path]`: `operation_id`, `tag`, `summary`, `description`, a 2xx response, the specific problem responses and `default`. Extract with `ApiPath`, `ValidatedQuery`, `ValidatedJson` or `ApiJson` only; bare `Path`, `Query` and `Json` extractors are forbidden because their rejections are `text/plain`. (4) Register it in the context's `router()` with `routes!`. (5) Unit-test the handler with `FakeCommandBus`/`FakeQueryBus`; add a scenario to a feature file. (6) Regenerate `.github/openapi-spec/spec.yaml` in the same change.

**Add a GraphQL field.** (1) Add the method to the context's `…Query` or `…Mutation` in `graphql.rs`, calling the same command or query as REST and converting errors with `problem_to_error`. (2) A new context adds its root structs to the tuple of `Query`/`Mutation` in `service/src/graphql/schema.rs` and its state with `.data(..)` in `bootstrap.rs`. (3) Regenerate `.github/graphql-spec/spec` in the same change.

**Add a command.** (1) `command/<name>.rs`: the command struct, `impl Command` with `Error` = the context's application error and `NAME` = the type name, the handler struct with its ports as `Arc<dyn Port>`, `new` and an inherent `async fn` with the logic; the `CommandHandler` impl only delegates (rule 21). (2) The handler loads or creates one aggregate, saves it, then publishes `pull_domain_events()`; a publication failure is logged per failing subscriber and never fails the command (PD-18). (3) Register it with `.register(..)?` in the private `service::bootstrap::register_commands` (a query: `register_queries`), which `service::bootstrap::router` calls. (4) Extend the wiring test in `service/src/bootstrap/tests.rs`.

**Add a query.** Same as a command, in `query/<name>.rs`, with `Output` set to a view struct or a collection of views. A query handler never mutates state and returns views, not aggregates.

**Add an event subscriber.** (1) `subscriber/<purpose>_<event>.rs` implementing `DomainEventSubscriber<Event>`. (2) Register it with `.subscribe(..)` in `service::bootstrap::event_bus`. (3) A subscriber must be idempotent and must not call the command bus. (4) Delivery is at most once: a subscriber whose work must not be lost needs an outbox, which this template does not ship.

**Add a migration.** (1) `sqlx migrate add -r <snake_case_description>`. (2) Write the up and the down file. (3) Apply it to the development database, update the adapter, run `cargo sqlx prepare --workspace -- --all-targets --all-features`, commit `.sqlx/` with the migration. (4) Never edit a migration that has reached `main`.

**Add a bounded context.** (1) `cargo xtask new-context <group> <context>`; the command registers the three crates and refreshes `Cargo.lock`. (2) Add `[workspace.dependencies]` path entries for the new crates when the first crate depends on them. (3) Add code layer by layer; put its fakes in `test-support`. (4) Wire it in `service`: adapters in `Adapters`, handlers in `bootstrap::router`, a `<Context>State` field in `AppState`, its router in `openapi::api_router`, its GraphQL roots in `graphql::schema`. (5) Add the components to `workspace.dsl`; until then the model gate fails. `architecture.toml` and every other protected item stay untouched.

**Add a health probe.** (1) Add a variant to `Dependency` with its identifier and failure message. (2) Implement `HealthProbe` in `health-check-infrastructure::probe`; a client crate it needs goes into `crates/internal/health-check/infrastructure/Cargo.toml`, which a service then lists in its `.templatesyncignore`. (3) Append it to `Adapters::probes` in `Adapters::connect`. The handler and `HealthService` do not change.

### Forbidden constructs

Each item names its gate; "review" means no gate exists.

| Construct | Gate |
| --- | --- |
| `unsafe` code | rustc `unsafe_code = "forbid"` |
| `#[allow]`; a suppression without a reason | clippy `allow_attributes`, `allow_attributes_without_reason` |
| A suppression that names a lint group, or `#![expect]` in a crate root | G31 rule (g) (delivery document) |
| An inline directive of shellcheck, hadolint, yamllint, markdownlint, ESLint or zizmor that names no rule (or all rules, or a group), names more than one rule, or carries no reason (after it where the linter allows one, or on the comment line above) | G31 rule (g) (delivery document) |
| `mod.rs` files | clippy `mod_module_files` |
| `impl Trait` in argument position (use a named generic) | clippy `impl_trait_in_params` |
| `SystemTime::now()` outside `SystemClock`; `Instant::now()` of `std`; `Uuid::now_v7()` or `new_v4()` outside `UuidV7Generator`; opening a socket outside the two commands that own one; reading the environment outside clap | clippy `disallowed_methods` |
| A second `clippy.toml`, `rustfmt.toml`, `rust-toolchain.toml` or another name of `override_names` anywhere in the tree | config-guard (G25) |
| A workspace crate depending on another context's crate; a layer using a third-party crate outside its allow-list; serde, utoipa, sqlx or axum in a Domain crate | `cargo xtask arch` (G4) |
| Git dependencies; registries other than crates.io; a dependency version outside `[workspace.dependencies]` | cargo-deny (G7) |
| Cargo features in workspace crates | review (QG-08 checks each feature that exists) |
| Inline test modules; glob imports; `async fn` in a trait that is used as `dyn` without `#[async_trait]` (does not compile); SQL outside an Infrastructure crate; reading configuration outside `service`; logic in an `#[async_trait]` method body; a tracing event outside a `log_<event>` function | review |

## Quality Gates

### Gate commands

The delivery document wraps each row in a Make target; the command lines are fixed here and are repeated character for character in its protected fragment `make/gates.mk`. All commands run at the workspace root. "PRD" is the gate id of the PRD gate list (G1 to G31); the gates not listed here (G9, G10, G16, G18 to G24, G26 to G28, G31) belong to the delivery document and are mapped in "Requirements to Architecture Mapping". Runtimes are for a 4 vCPU machine: "measured" values come from the validation skeleton, built without debug information ("cold" = empty `target/`); the others are estimates. Every command that resolves dependencies passes `--locked` (FR45).

| Id | Purpose | Tool and version | Configuration | Command | Fails when | Typical runtime | Replaces (PHP) | PRD |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| QG-01 | Formatting check | rustfmt (toolchain 1.99.1) | `rustfmt.toml` | `cargo fmt --all --check` | any file differs | 1.3 s (measured) | `phpcsfixer` | G1 |
| QG-02 | Static analysis, production targets | clippy (toolchain 1.99.1) | `[workspace.lints]`, `clippy.toml` | `cargo clippy --locked --workspace --lib --bins --all-features -- -D warnings` | any finding | about 1 min cold; 1.3 s warm (measured) | `psalm`, `phpinsights` | G2 |
| QG-03 | Static analysis, all targets (tests relaxed) | clippy | same | `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings -A clippy::cognitive_complexity -A clippy::too_many_lines` | any finding | about 30 s after QG-02; 1.3 s warm (measured) | `phpmd.tests.xml` | G2 |
| QG-04 | Documentation lints | rustdoc (toolchain 1.99.1) | `[workspace.lints.rustdoc]` | `env RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --no-deps --document-private-items` | any finding | 10 to 30 s | part of `psalm` | G2 |
| QG-05 | Architecture boundaries | `xtask` | `architecture.toml` | `cargo xtask arch` | exit 1 (violation) or 2 (could not run) | 0.2 s warm, 6 s when `xtask` compiles | `deptrac`, `deptrac-debug` | G4 |
| QG-06 | Protected configuration | `xtask` | `protected-config.toml` | `cargo xtask config-guard --base origin/main` | exit 1 or 2 | under 1 s | `validate-configuration` | G25 |
| QG-07 | Manifest and lock file | cargo 1.99.1 | `Cargo.lock` | `cargo metadata --locked --format-version 1 > /dev/null` | lock file does not match the manifests | 0.25 s (measured) | `composer-validate` | G5 |
| QG-08 | Crate isolation: each crate builds alone | cargo-hack 0.6.46 | none | `cargo hack check --workspace --each-feature --locked` | a crate relies on a dependency or feature another crate supplies | 13 runs, 18.6 s warm (measured) | none | G29 |
| QG-09 | Advisories, licences, bans, sources | cargo-deny 0.20.2 | `deny.toml` | `cargo deny --locked check` (offline subset for hooks: `cargo deny --locked --offline check bans licenses sources`) | any error | 1.3 s plus advisory fetch; offline subset 0.9 s (measured) | `check-security` | G7 |
| QG-10 | Unused dependencies | cargo-shear 1.14.0 | none | `cargo shear --locked` | any unused or misplaced dependency | 0.4 s (measured) | none | G8 |
| QG-11 | Offline query metadata is current | sqlx-cli 0.9.0 | `.sqlx/` | `cargo sqlx prepare --workspace --check -- --all-targets --all-features --locked` | `.sqlx/` differs from the metadata generated against the freshly migrated test database | 2.5 s warm (measured), one `cargo check` cold | none | G30 |
| QG-12 | Configuration check | `service` | `.env` or environment | `cargo run --locked --quiet -p service -- check-config` | exit status not 0 | seconds after the build | `check-requirements` | G6 |
| QG-13 | Unit tests and documentation tests | cargo-nextest 0.9.151 | `.config/nextest.toml` | `cargo nextest run --locked --workspace --lib --bins` then `cargo test --locked --workspace --doc` | any test fails | about 2 min cold, seconds warm | `unit-tests` | G11 |
| QG-14 | Integration tests | cargo-nextest 0.9.151 | `.config/nextest.toml`, `.env`, `.env.test` | `cargo nextest run --locked --workspace --test integration` | any test fails | about 2 min cold; the `service` suite runs in 3 s (measured) | `integration-tests` | G12 |
| QG-15 | End-to-end tests | cucumber 0.23.0 through `cargo test` | `features/`, `.env`, `.env.test` | `cargo test --locked -p e2e-tests --test e2e` | a step fails, is undefined or is skipped | 2 min 07 s cold, 5 s warm (measured) | `behat` | G13 |
| QG-16 | Coverage floor | cargo-llvm-cov 0.9.1 | flags below | sequence below | lines or functions below 100% | 1 min 23 s cold for the run (measured) | `tests-with-coverage` | G14 |
| QG-17 | Mutation testing, full | cargo-mutants 27.1.0 | `.cargo/mutants.toml` | `env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --workspace --cargo-arg=--locked` (CI shards add `--shard <k>/<n>`, k from 0 to n-1; CI also adds `--in-place` through `MUTANTS_IN_PLACE=1`) | exit 2 (a mutant survived) or 3 (a mutant timed out) | 60 to 80 min unsharded; about 15 min per shard of at most 150 mutants (delivery DD-09; 5.2 s per mutant, measured on 89 mutants) | `infection` | G15 |
| QG-18 | Mutation testing, changed code | cargo-mutants 27.1.0 | same | `git diff --no-ext-diff --no-textconv --no-color --no-relative --no-renames --unified=3 --src-prefix=a/ --dst-prefix=b/ "$(git merge-base origin/main HEAD)" -- '*.rs' > target/mutants.diff`, run by `scripts/mutation-diff.sh` with `-c` pins for `core.quotePath`, `diff.mnemonicPrefix`, `diff.noprefix`, `diff.relative`, `diff.renames`, `diff.context`, `color.ui` and `color.diff` and without `GIT_DIFF_OPTS` and `GIT_EXTERNAL_DIFF`, so the user's git configuration cannot change the file; then the command of QG-17 with `--in-diff target/mutants.diff` | as QG-17; exit 2 when the base ref cannot be resolved | 5.2 s per mutant in the diff (measured mean) | none | G15 |
| QG-19 | Contract snapshots are current | `service` | none | `cargo run --locked --quiet -p service -- openapi-export --output .github/openapi-spec/spec.yaml`, `cargo run --locked --quiet -p service -- graphql-export --output .github/graphql-spec/spec`, then `test -z "$(git status --porcelain -- .github/openapi-spec .github/graphql-spec)"` | a committed snapshot differs from the generated one, or a snapshot is not tracked | seconds after the build | `generate-openapi-spec`, `generate-graphql-spec` | G17 |
| QG-20 | Complexity limits | clippy (toolchain 1.99.1) | `clippy.toml` | `cargo clippy --locked --workspace --lib --bins --all-features -- -A warnings -D clippy::cognitive_complexity -D clippy::too_many_lines -D clippy::excessive_nesting -D clippy::too_many_arguments` | a production function over a limit | seconds after QG-02 | `phpmd`, `phpinsights` complexity | G3 |

Notes on single rows:

- **QG-02 and QG-20.** QG-02 also fails on a complexity finding, because the four lints are part of the policy; QG-20 exists so that G3 reports under its own name with nothing but complexity findings (verified: it passes on clean code and fails with `this function has too many arguments (6/5)` on a planted function while unrelated warnings stay allowed).
- **QG-04.** The `env` prefix makes the line an executable command, so it also works behind `docker compose exec` (a bare `RUSTDOCFLAGS=.. cargo ..` fails there with exit status 127, verified).
- **QG-09, QG-10.** Without `--locked`, `cargo shear` rewrites `Cargo.lock` when a manifest is ahead of it; with it the command exits 2 and leaves the file unchanged (verified). `cargo deny --locked --offline check bans licenses sources` exits 0 on the skeleton.
- **QG-11.** The arguments after `--` go to `cargo check`; with `--locked` there the command exits 0 on current metadata (verified in this revision).
- **QG-13 and QG-14.** The delivery document wraps them in `unit-tests` (which `pre-push` runs) and `integration-tests`. `make ci` and CI run neither: they run the unit and integration suites once, in the coverage sequence of QG-16 (`tests-with-coverage`, delivery DD-02), report G11 and G12 from that run, and add the documentation tests of QG-13 through `doc-tests`. The negative test of G11 and G12 runs both `unit-tests`/`integration-tests` and `tests-with-coverage`.
- **QG-17.** `--cargo-arg=--locked` reaches both phases: cargo-mutants runs `cargo nextest run --no-run .. --locked` and `cargo nextest run .. --locked --profile mutants` (verified in this revision: 4 mutants of one file caught in 12 s, exit status 0). `CARGO_BUILD_WARNINGS` must be unset (AD-24). In CI, `--in-place` makes cargo-mutants build in the checkout, whose `target/` the Rust cache restores, instead of a copy without `target/` (NFR40); it changes neither the outcomes nor the timeouts (verified on 23 mutants: the same 17 caught, 2 missed, 4 unviable both ways).
- **QG-18.** The diff is taken from the merge base to the working tree, so uncommitted changes to tracked files are included; `git diff origin/main...HEAD` would ignore them (verified: 0 bytes for an uncommitted function, and cargo-mutants then prints `Diff file is empty` and exits 0). A file that git does not track yet is not in the diff until it is added or committed. `--in-diff` selects only the mutants of changed functions and ignores test files and non-Rust files; a diff in which every file is new lists every mutant, so the mode works on the bootstrap pull request; `--shard` combines with it; a diff without a production Rust change prints `No mutants to filter` and exits 0. The delivery script that wraps the two commands resolves the base ref first and exits 2 with a message when it cannot (FR59(e)).
- **QG-19.** `git status --porcelain` also reports a snapshot that was never committed, which `git diff --exit-code` does not see.

Coverage sequence (QG-16); reports go to `target/coverage/`, which is git-ignored with `target/`. The floor is the last command, so the reports exist when it fails:

```sh
cargo llvm-cov clean --workspace
cargo llvm-cov --no-report nextest --locked --workspace --lib --bins --test integration --profile ci
cargo llvm-cov report --lcov --output-path target/coverage/lcov.info
cargo llvm-cov report --html --output-dir target/coverage
cargo llvm-cov report --fail-under-lines 100 --fail-under-functions 100
```

Further report command (`coverage-xml` of the delivery document): `cargo llvm-cov report --cobertura --output-path target/coverage/coverage.xml`. The end-to-end suite is not part of the sequence (PD-28). Verified with cargo-llvm-cov 0.9.1 and nextest 0.9.151: `--profile ci` is taken as the nextest profile and the JUnit file lands at `target/nextest/ci/junit.xml`; the floor command exits 1 on an uncovered function; the lcov, cobertura and html reports are written (cargo-llvm-cov appends `html/` to `--output-dir`, so the HTML report is `target/coverage/html/index.html`); the run needs the `e2e` test target to exist in the workspace, because the default filter names it (`no binary names matched`, exit status 96, when it does not). This one run is also the execution from which G11, G12 and G14 are reported (FR66(a), NFR41); documentation tests are run by QG-13 and are not instrumented.

Fix commands (not gates): `cargo fmt --all`; `cargo clippy --fix --allow-dirty --allow-staged --locked --workspace --all-targets --all-features -- -D warnings` followed by `cargo fmt --all`; `cargo sqlx prepare --workspace -- --all-targets --all-features`; `cargo shear --fix`.

Service requirements: QG-11, QG-14, QG-15, QG-16, QG-17 and QG-18 need the compose services `database` and `localstack` and a prepared test database; all other gates need no service. Tool installation on a host: every cargo tool at the version of the table; `sqlx-cli` with `cargo install sqlx-cli --version 0.9.0 --no-default-features --features postgres,rustls` (without `--locked`, its lock file was removed upstream).

Negative tests (NFR21). Every gate of this table has a Make-level negative test in the delivery document's Bats suite; each rule kind of the two template-owned checkers has one at the checker's own test level (`xtask/src/arch/rules/tests.rs` and `xtask/tests/integration/arch.rs` for G4; `xtask/src/config_guard/check/tests.rs` and `xtask/tests/integration/config_guard.rs` for G25), as PD-26 allows. Two properties the Bats plants must have, both measured: a plant for G3 or G14 sits in a plain function, not in an `#[async_trait]` method; the plant for G15 is a function with a parameter and a return value (`fn planted(flag: bool) -> bool { flag }`), run with `CI=true`, so that a hollow mutation gate would be noticed. The plant for the clippy ban list (NFR28) is a `SystemTime::now()` call in `xtask/src/main.rs`, which must fail the static-analysis target with `disallowed_methods`.

### `rust-toolchain.toml`

```toml
[toolchain]
channel = "1.99.1"
profile = "minimal"
components = ["rustfmt", "clippy", "llvm-tools-preview"]
```

### `rustfmt.toml`

```toml
# Stable options only: `cargo fmt` must give the same result on every machine
# without a nightly toolchain.
edition = "2024"
style_edition = "2024"
max_width = 100
newline_style = "Unix"
use_field_init_shorthand = true
use_try_shorthand = true
```

### `clippy.toml`

```toml
# Complexity budget (PHPInsights min-complexity + PHPMD codesize equivalent).
cognitive-complexity-threshold = 8
too-many-lines-threshold = 50
too-many-arguments-threshold = 5
excessive-nesting-threshold = 5
type-complexity-threshold = 200
max-struct-bools = 2
max-fn-params-bools = 1
max-trait-bounds = 3
enum-variant-name-threshold = 3
single-char-binding-names-threshold = 2
pass-by-value-size-limit = 256

# Tests may unwrap, expect, panic, index and print. "Tests" means `#[test]` functions and code inside
# `#[cfg(test)]` modules, which is why every module of a test target is declared `#[cfg(test)] mod ..;`.
allow-unwrap-in-tests = true
allow-expect-in-tests = true
allow-panic-in-tests = true
allow-indexing-slicing-in-tests = true
allow-dbg-in-tests = true
allow-print-in-tests = true
allow-useless-vec-in-tests = true

# Public API hygiene.
avoid-breaking-exported-api = false
missing-docs-in-crate-items = true
check-private-items = false
absolute-paths-max-segments = 3

# Banned APIs (workspace-wide, production and test code). Each allowed use carries an item-level
# `#[expect(clippy::disallowed_methods, reason = "..")]`; the complete list of those items is rule 20.
disallowed-methods = [
    { path = "std::env::var", reason = "read configuration once in the composition root" },
    { path = "std::env::var_os", reason = "read configuration once in the composition root" },
    { path = "std::env::vars", reason = "read configuration once in the composition root" },
    { path = "std::env::vars_os", reason = "read configuration once in the composition root" },
    { path = "std::env::set_var", reason = "process-global mutation" },
    { path = "std::env::remove_var", reason = "process-global mutation" },
    { path = "std::process::exit", reason = "return an error from main instead" },
    { path = "std::time::SystemTime::now", reason = "read time through the Clock port" },
    { path = "std::time::Instant::now", reason = "use tokio time, which tests can pause" },
    { path = "uuid::Uuid::now_v7", reason = "use the UuidGenerator port" },
    { path = "uuid::Uuid::new_v4", reason = "use the UuidGenerator port" },
    { path = "std::net::TcpListener::bind", reason = "sockets belong to the serve and healthcheck commands and to integration tests" },
    { path = "std::net::TcpStream::connect", reason = "sockets belong to the serve and healthcheck commands and to integration tests" },
    { path = "std::net::TcpStream::connect_timeout", reason = "sockets belong to the serve and healthcheck commands and to integration tests" },
    { path = "std::net::UdpSocket::bind", reason = "sockets belong to the serve and healthcheck commands and to integration tests" },
    { path = "tokio::net::TcpListener::bind", reason = "sockets belong to the serve and healthcheck commands and to integration tests" },
    { path = "tokio::net::TcpStream::connect", reason = "sockets belong to the serve and healthcheck commands and to integration tests" },
]
```

Notes. The ten entries after `std::process::exit` are the static-analysis rule NFR28 asks for: wall-clock, UUID-generation and network APIs are forbidden outside the adapters and commands that own them. The environment entries now cover every reading function of `std::env` (`var`, `var_os`, `vars`, `vars_os`), so that rule 20 has no gap; the first version banned `std::env::var` only. Verified in this revision on the skeleton: with this list QG-02 and QG-03 exit 0 once `SystemClock::now`, `UuidV7Generator::generate`, `serve::bind`, `healthcheck::probe`, `TestRng::from_env` and the socket and environment helpers of the test support modules carry their item-level expectation (the test helper that forwards `LLVM_PROFILE_FILE` reads it through the same environment helper); no entry produces an invalid-path warning; `tokio::time::Instant` is not on the list and is what integration tests use to measure elapsed time. The first version of this file also had a `disallowed-macros` block for `println!` and `eprintln!`. It is removed: `print_stdout` and `print_stderr` of the lint table already ban both macros in production code and honour `allow-print-in-tests`, while the block also hit tests and the build script (`use of a disallowed macro std::println` in `build.rs`); without it `println!` in `shared-infrastructure/build.rs` passes QG-02 (verified).

### `deny.toml`

```toml
# cargo-deny 0.20 configuration: `cargo deny check` runs all four checks.
# Replaces `composer validate` (manifest hygiene), `symfony security:check`
# (advisories) and adds licence and source policy.

[graph]
all-features = true

[output]
feature-depth = 1

[advisories]
# RustSec advisory database: vulnerabilities and unsound crates always fail.
yanked = "deny"
unmaintained = "all"
ignore = [
    # { id = "RUSTSEC-0000-0000", reason = "why this is acceptable, and until when" },
]

[licenses]
allow = [
    "MIT",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Unicode-3.0",
    "Zlib",
    "CDLA-Permissive-2.0",
]
confidence-threshold = 0.93
unused-allowed-license = "allow"

[licenses.private]
# Workspace crates are `publish = false`; they carry the repository licence.
ignore = true

[bans]
multiple-versions = "warn"
wildcards = "deny"
allow-wildcard-paths = true
highlight = "all"
deny = [
    { crate = "openssl-sys", reason = "TLS goes through rustls; no system OpenSSL in the image" },
    { crate = "native-tls", reason = "TLS goes through rustls" },
]

[bans.workspace-dependencies]
# Every dependency version is declared once, in [workspace.dependencies].
duplicates = "deny"
include-path-dependencies = true
unused = "deny"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
allow-git = []
```

### `.cargo/config.toml`

```toml
[alias]
xtask = "run --quiet --locked --package xtask --"

[env]
# sqlx macros never contact a database at build time; `cargo sqlx prepare` overrides this itself.
SQLX_OFFLINE = "true"
# The seed recorded in the repository (FR51): the default of every test run. A value set by the caller wins;
# the scheduled workflow sets a fresh one.
TEST_SEED = "20261110"
PROPTEST_RNG_SEED = "20261110"
# FR8, AD-19: Swagger UI comes from the vendored crate; without the feature the build stops instead of downloading
SWAGGER_UI_DOWNLOAD_URL = "vendored-only"
```

`[env]` values do not override a variable that is already set (no `force`), which is the behaviour FR51 needs: verified on 1.99.1 under `cargo test` and `cargo nextest run`, where a caller's `TEST_SEED=7` replaced the recorded value. cargo-mutants and cargo-llvm-cov run cargo in a tree that contains this file, so they see the same seed. `SWAGGER_UI_DOWNLOAD_URL` guards FR8. The build script of `utoipa-swagger-ui` 10.0.2 checks `CARGO_FEATURE_VENDORED` first, so a vendored build ignores the value. Without the feature, a value that starts with neither `http`, `https` nor `file:` makes the build script stop with `` `vendored` feature not enabled and invalid SWAGGER_UI_DOWNLOAD_URL ``, and no download happens.

### `.cargo/mutants.toml`

```toml
# The mutation scope record (FR59(b), NFR20). cargo-mutants exits 2 when a mutant survives and 3 when one
# times out: the exit code is the gate, and a timed-out mutant counts as surviving (FR59(c)).

# Each mutant is judged by the unit and integration tests of its own crate, run with nextest.
test_tool = "nextest"
additional_cargo_test_args = ["--profile", "mutants"]

# Scope: the production source minus these paths, each with its reason.
#   **/main.rs    process entry points: one call into the library, exercised by the process-level tests
#   **/build.rs   build script, not part of the service
#   **/tests.rs   unit tests
#   **/tests/**   integration tests, test-support and end-to-end crates: test code
exclude_globs = ["**/main.rs", "**/build.rs", "**/tests.rs", "**/tests/**"]

# A mutant whose tests do not finish within max(120 s, 3 x baseline) is reported as TIMEOUT; cargo-mutants
# then exits 3 and the gate fails. A mutant that can only hang and is equivalent is listed in exclude_re.
timeout_multiplier = 3.0
minimum_test_timeout = 120

# Equivalent mutants are listed here one by one, each with a comment that explains why.
exclude_re = []
```

### `.config/nextest.toml`

```toml
[profile.default]
# The Gherkin runner is a custom harness that nextest cannot list; it runs with `cargo test`.
default-filter = 'not binary(e2e)'
fail-fast = false
slow-timeout = { period = "30s", terminate-after = 2 }

[profile.ci]
fail-fast = false
retries = 0
failure-output = "immediate-final"

[profile.ci.junit]
path = "junit.xml"

[profile.mutants]
# No `terminate-after`: a hanging mutant must reach the cargo-mutants timeout and count as surviving.
fail-fast = true
retries = 0
slow-timeout = { period = "20s" }
```

Verified with this file: `cargo nextest run --locked --workspace` runs 88 tests across 17 binaries of the skeleton and skips the `e2e` binary through the default filter (with `--ignore-default-filter` listing fails on the cucumber binary: `unexpected argument '--list'`); cargo-mutants passes `--profile mutants` to the nextest test phase.

Adaptations of the files that passed in the spikes: the toolchain channel moves from 1.97.0 to 1.99.1; `deny.toml` adds `CDLA-Permissive-2.0` (the `webpki-roots` licence, found in the data spike's graph); `mutants.toml` replaces the unit-only filter with the `mutants` profile and adds `**/build.rs` (AD-24); `nextest.toml` adds the `default-filter` of the testing research and the `mutants` profile; `clippy.toml` extends `disallowed-methods` and drops `disallowed-macros`; `.cargo/config.toml` records the test seeds. `rustfmt.toml` is unchanged.

### Protected configuration

`protected-config.toml` as committed. It is the one protected list (NFR26): the guard, the assistant settings and the guidance read it. The delivery document appends its own entries to `files`, `directories` and `required_directories` (the Makefile fragments that hold gate command lines and tool versions, hook scripts, linter configurations, its scripts and Bats tests) and, for every load scenario it ships besides `health`, the four budget pointers to `json_values`.

```toml
# The single list of protected quality configuration. Read by `cargo xtask config-guard`,
# by the assistant settings guard and by the guidance. Changing a listed item needs the
# pull-request label named below.
approval_label = "approved-config-change"

# Protected files: any modification, deletion or rename relative to the base ref is a violation.
files = [
    ".cargo/config.toml",
    ".cargo/mutants.toml",
    ".config/nextest.toml",
    ".spectral.yaml",
    "architecture.toml",
    "clippy.toml",
    "deny.toml",
    "protected-config.toml",
    "rust-toolchain.toml",
    "rustfmt.toml",
    "xtask/src/arch.rs",
    "xtask/src/config_guard.rs",
    "xtask/src/cli.rs",
    "xtask/src/main.rs",
]

# Protected directories: every file that exists below them on the base ref is protected.
# `xtask/tests/` holds the negative tests and fixtures of the architecture gate and of this guard (NFR21).
directories = ["xtask/src/arch/", "xtask/src/config_guard/", "xtask/tests/"]

# Protected tables inside TOML files that otherwise change freely.
manifest_tables = [{ file = "Cargo.toml", table = "workspace.lints" }]

# Protected values inside JSON files that otherwise change freely (RFC 6901 pointers):
# the p99 budgets of the shipped load scenarios.
json_values = [
    { file = "tests/Load/config.json.dist", pointers = [
        "/endpoints/health/smoke/threshold",
        "/endpoints/health/average/threshold",
        "/endpoints/health/stress/threshold",
        "/endpoints/health/spike/threshold",
    ] },
]

# Files that would override a protected configuration file for part of the tree. A bare name matches a file
# of that name in any directory; an entry with `/` matches a path that ends with it.
override_names = [
    ".cargo/config",
    ".cargo/config.toml",
    ".cargo/deny.toml",
    ".cargo/mutants.toml",
    ".config/nextest.toml",
    ".clippy.toml",
    "clippy.toml",
    ".rustfmt.toml",
    "rustfmt.toml",
    "rust-toolchain",
    "rust-toolchain.toml",
    ".deny.toml",
    "deny.toml",
    ".taplo.toml",
    "taplo.toml",
    ".typos.toml",
    "_typos.toml",
    "typos.toml",
    ".spectral.js",
    ".spectral.json",
    ".spectral.yaml",
    ".spectral.yml",
    ".hadolint.yaml",
    ".hadolint.yml",
    ".yamllint",
    ".yamllint.yaml",
    ".yamllint.yml",
    ".markdownlint.json",
    ".markdownlint.jsonc",
    ".markdownlint.yaml",
    ".markdownlint.yml",
    ".markdownlint-cli2.jsonc",
    ".markdownlint-cli2.yaml",
    ".markdownlint-cli2.cjs",
    ".markdownlint-cli2.mjs",
    ".markdownlint.cjs",
    ".markdownlint.mjs",
    ".prettierrc",
    ".prettierrc.json",
    ".prettierrc.yaml",
    ".prettierrc.yml",
    ".prettierrc.json5",
    ".prettierrc.toml",
    ".prettierrc.js",
    ".prettierrc.ts",
    ".prettierrc.mjs",
    ".prettierrc.mts",
    ".prettierrc.cjs",
    ".prettierrc.cts",
    "prettier.config.js",
    "prettier.config.ts",
    "prettier.config.mjs",
    "prettier.config.mts",
    "prettier.config.cjs",
    "prettier.config.cts",
    "package.yaml",
    "eslint.config.js",
    "eslint.config.ts",
    "eslint.config.mjs",
    "eslint.config.mts",
    "eslint.config.cjs",
    "eslint.config.cts",
]
# Fixture trees may contain such files.
override_exempt = ["tests/CLI/bats/fixtures/", "xtask/tests/fixtures/"]

# Directories that must exist in the working tree.
required_directories = [
    ".github/graphql-spec",
    ".github/openapi-spec",
    ".sqlx",
    "crates/service",
    "crates/shared/application",
    "crates/shared/domain",
    "crates/shared/infrastructure",
    "features",
    "migrations",
    "tests/e2e",
    "tests/support",
    "xtask",
]
```

What the list covers, against NFR26: the toolchain pin (`rust-toolchain.toml`); format settings (`rustfmt.toml`); the lint policy (`[workspace.lints]`) and lint thresholds (`clippy.toml`); architecture rules (`architecture.toml`, the checker sources); the dependency policy and its exceptions (`deny.toml`); coverage and mutation settings with their records (`.cargo/mutants.toml`, and the `COVERAGE_*` and `MUTANTS_CMD` lines in the delivery fragment `make/gates.mk`); test-runner settings and the recorded seeds (`.config/nextest.toml`, `.cargo/config.toml`); the Spectral ruleset; the K6 budgets; the negative tests and fixtures of the two checkers; the list and the guard themselves. Secrets-scan, image-scan, repository-lint and load-lint configuration, hook scripts, the Bats negative tests and the assistant settings file are appended by the delivery document.

How `cargo xtask config-guard --base <ref>` decides:

1. Resolve `<ref>` (default `origin/main`) with git. If it cannot be resolved (shallow checkout, missing remote), print `configuration guard could not run: base ref <ref> is not available` and exit 2. A guard that cannot compare never passes.
2. For each entry of `files` and each file under `directories` on the base ref: read the blob and its entry kind at the base ref (regular file, executable file, symbolic link). If the path does not exist on the base ref, it is new and allowed. If the working-tree file is missing (deleted or renamed), its bytes differ, its kind or executable bit differs, or a directory above it is a symbolic link, record `Modification of locked configuration file is not allowed: <path>`. Links are never followed: an entry that the base holds as a link is compared as that link. A protected directory that holds files at the base ref and is now a link or a file is recorded with the same line for the directory, without a trailing slash.
3. If the base ref holds at least one protected item: list the files of the working tree that git tracks or would add (`git ls-files --cached --others --exclude-standard`). Every such path that matches an entry of `override_names`, is not itself an entry of `files`, is not under an entry of `override_exempt` and does not exist on the base ref is recorded as `Overriding configuration file is not allowed: <path>`.
4. For each entry of `manifest_tables`: parse the file at the base ref and in the working tree and compare the named table as TOML values (key order and comments are ignored). A difference, a missing table or a missing file in the working tree is recorded as `Modification of locked configuration table is not allowed: <file> [<table>]`. A file or table that does not exist on the base ref is new and allowed.
5. For each entry of `json_values`: parse the file at the base ref and in the working tree and compare the value at each pointer. A different value, a missing pointer or a missing file in the working tree is recorded as `Modification of locked configuration value is not allowed: <file> <pointer>`. A file or pointer that does not exist on the base ref is new and allowed.
6. For each entry of `required_directories`: if it is not a directory in the working tree, record `Required directory is missing: <path>`.
7. Print the header `Configuration Validation`, then every recorded line. With no line: print `[OK] Configuration validation passed`, exit 0.
8. With recorded modifications or overrides, no missing directory, and `CONFIG_CHANGE_APPROVED=true` in the environment: print each as approved, then `[OK] Configuration validation passed (approved by label approved-config-change)`, exit 0. Missing directories are never waived.
9. Otherwise print `[FAIL] Configuration validation failed`, then `Approve with the label approved-config-change`, exit 1 (FR44(f): outside CI no label can be read, so the name of the label is printed).

CI sets `CONFIG_CHANGE_APPROVED=true` only when the pull request carries the label `approved-config-change`; template-sync, Dependabot and fork pull requests follow the same rule (FR44(e)). The variable is read through clap in `xtask::cli` (rule 20). `cargo xtask config-guard --list` prints the protected items, one per line, for the assistant settings guard and the documentation: files as their path, directories with a trailing `/`, sections as `<file>#<table or pointer>`.

On the bootstrap pull request the base ref holds no protected item, so every item is new, step 3 does not apply and the guard passes without a label (scope 8.5, 8.11).

## Test Architecture

### Levels

| Level | Where the tests live | Test binary | Command | Needs |
| --- | --- | --- | --- | --- |
| Unit | sibling `tests.rs` files in every crate | the `lib` (and `bin`) test binaries of each crate | QG-13 | nothing: no service, no network, no wall clock |
| Integration | `tests/integration/main.rs` (plus modules) in `shared-infrastructure`, `health-check-infrastructure`, `some-module-infrastructure`, `service`, `xtask` | one binary named `integration` per crate | QG-14 | `database` and `localstack` up, test database prepared, `.env` and `.env.test` exported (`xtask` needs only git and cargo) |
| End-to-end | `tests/e2e/` and `features/*.feature` | `e2e` (crate `e2e-tests`, custom harness) | QG-15 | same as integration |
| Makefile (Bats) | `tests/CLI/bats/` | defined in `architecture-delivery.md` | | |

Coverage (G14) counts the first two levels; mutants (G15) are judged by the first two levels of the crate that contains them; the end-to-end level counts for neither.

What each level proves:

- **Unit.** Domain invariants; handlers with fakes; HTTP handlers and extractors in process (`tower::ServiceExt::oneshot`); `HealthService` with `StubProbe`, including the per-probe timeout under tokio's paused clock and each of the 7 non-empty combinations of failing dependencies; bus semantics; configuration parsing with every error path; the router with its middleware (request id, the four headers and problem bodies on every route, on an unknown route, on a wrong method and on a forced panic through `StubProbe::panicking`); the wiring test that dispatches every command and query; every `xtask` rule; every log function through `LogRecorder`.
- **Integration.** Real adapters against PostgreSQL and LocalStack, and against a dead port (`127.0.0.1:1`) to prove each timeout bound; migrations apply, revert, re-apply; the production wiring in process (`bootstrap.rs`); the production telemetry with a forced panic (`panic.rs`); the binaries as subprocesses (`env!("CARGO_BIN_EXE_service")`, `env!("CARGO_BIN_EXE_xtask")`), which is also what covers `main.rs`, `service::run` and `service::command::*`.
- **End-to-end.** The complete application with production wiring and real adapters, in process; outages by port substitution (AD-23).

Rules for a spawned binary (`service` or `xtask`), implemented once per crate in `tests/integration/support.rs`:

- The child runs with a cleared environment (`Command::env_clear`) plus (a) the variables the test sets and (b) `LLVM_PROFILE_FILE`, copied from the test process when present. Every read of the test process environment goes through the one helper of `support.rs` that carries the expectation of rule 20 (`fn from_environment(name: &str) -> Option<OsString>`, a call to `std::env::var_os`). cargo-llvm-cov sets that variable; without it the child writes its profile into its working directory and the coverage of the binary is lost (measured: `main.rs`, `lib.rs`, `command/*` and `telemetry.rs` at 0% with the cleared environment; `main.rs`, `lib.rs`, `export.rs` and `healthcheck.rs` at 100% once the variable is forwarded).
- Its working directory is a fresh directory under `std::env::temp_dir()`, outside the repository: never `CARGO_TARGET_TMPDIR` or anything below `target/`, because `dotenvy::dotenv()` also searches parent directories and would load the committed `.env` (observed: under cargo-mutants the copied tree's `.env` was found from `<copy>/target/tmp/..` and the baseline failed with exit status 4).
- A child is always ended through its normal exit path, by SIGTERM or by its own return, never by SIGKILL, so its profile is written; the test waits for the exit status.
- `serve` is started with `APP_PORT=0` and `APP_HOST=127.0.0.1`, and the test reads the bound address from the `listening` log line.
- `xtask` starts `cargo` and `git` itself, so its helper also forwards what those two need to be found and to run: `PATH`, `HOME`, `CARGO_HOME`, `RUSTUP_HOME` and `RUSTUP_TOOLCHAIN` when set. Nothing else is forwarded; in particular not `GITHUB_ACTIONS`, `CONFIG_CHANGE_APPROVED`, `CARGO_TARGET_DIR`, `CARGO_BUILD_WARNINGS` or `RUSTFLAGS`, which a test sets explicitly when it needs them.

```rust
// crates/service/tests/integration/support.rs (shape that ran in the skeleton)
pub(crate) fn service(arguments: &[&str], variables: &[(&str, &str)]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_service"));
    command.args(arguments).env_clear().current_dir(empty_directory());
    if let Some(profile) = from_environment("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    command.envs(variables.iter().copied());
    command
}
#[expect(clippy::disallowed_methods, reason = "the test stack is described by the environment")]
fn from_environment(name: &str) -> Option<OsString> { std::env::var_os(name) }
fn empty_directory() -> PathBuf {   // std::env::temp_dir().join(format!("service-it-{}", std::process::id())), created on demand
}
```

Integration tests run under nextest, which gives every test its own process; `panic.rs` relies on this, because it installs the global subscriber and the panic hook.

### Seeded random data

- `test_support::seed::TestRng::from_env()` reads `TEST_SEED` (a `u64`) and wraps `rand::rngs::StdRng::seed_from_u64(seed)`. Cargo supplies the seed recorded in `.cargo/config.toml` (`20261110`), so pull-request runs and `make ci` are reproducible by default (FR51, NFR29). A test binary started outside cargo without the variable fails with a message that names `TEST_SEED`; no seed is ever drawn silently.
- The scheduled workflow sets `TEST_SEED` and `PROPTEST_RNG_SEED` to a fresh value (the run id) and prints both, so each scheduled run explores other data (delivery document).
- `TestRng` prints `TEST_SEED=<seed>` to stderr when it is dropped while the thread is panicking, so a generator that the test keeps until it ends shows the seed of every failing test; nextest and `cargo test` print captured stderr for failed tests. The writer of that line is a type parameter with the default `std::io::Stderr` (`TestRng<W = Stderr>`); the crate-private constructor `TestRng::with_report(seed, writer)` lets the unit test `a_generator_alive_at_the_panic_writes_its_seed_line` read the line.
- Fixtures in `test_support::fixture` take `&mut TestRng` and generate data with `fake` (`fake_with_rng`). UUIDs come from `SequenceUuidGenerator` or `fake::uuid::UUIDv7` (feature `uuid` of `fake`), never from `Faker.fake::<Uuid>()` (its version bits are random).
- Rerun: `TEST_SEED=<seed> cargo nextest run …` reproduces the data. Property tests (256 cases) take their seed from `PROPTEST_RNG_SEED` in the same way (proptest 1.12.0 reads the variable), print it on failure and persist failing cases under `proptest-regressions/`.

### Coverage and mutation

- Coverage comes from one nextest run of the unit and integration suites (sequence under QG-16); the end-to-end suite does not count (PD-28, FR58(a), NFR19). Measured: every file of every workspace crate except files named `tests.rs`, files under any directory named `tests`, and `build.rs`, all of which are absent from the report without a flag. No source file is excluded by a flag; the exclusion record (AD-24) is empty.
- Where the code that looks end-to-end-only is covered:

| Code | Covered by |
| --- | --- |
| `service/src/main.rs`, `service::run`, `service::command::{serve, migrate, check_config, export, healthcheck}` | the process-level tests of `service/tests/integration` (with `LLVM_PROFILE_FILE` forwarded) |
| `service::telemetry` | every spawned `serve` process; `tests/integration/panic.rs` for the panic hook |
| `service::bootstrap::Adapters::connect` | every spawned `serve` process; `tests/integration/bootstrap.rs` in process |
| `service::bootstrap::Adapters::with_probe` | `bootstrap/tests.rs`: replaces the probe of the same dependency and keeps the order and the other two (its only other caller is the end-to-end world) |
| `service::bootstrap::event_bus`, `bootstrap::router` | `bootstrap/tests.rs` (wiring test; replaced event bus) and `tests/integration/bootstrap.rs` |
| `xtask/src/main.rs`, `xtask::cli`, `xtask::scaffold::lock` | `xtask/tests/integration/*` (binary as subprocess) |

  Measured in this revision on the skeleton, unit and integration suites only: `main.rs`, `lib.rs`, `telemetry.rs` and `command/serve.rs` of `service` are at 100% lines; `bootstrap.rs` misses one line, the `)?);` case of rule 23.
- Mutation: scope, judge and exit codes as in AD-24 and `.cargo/mutants.toml`. A mutant that times out counts as surviving (exit status 3); the limit is `max(120 s, 3 x baseline)`. Nothing terminates a hanging test earlier.

### Required test cases added by the alignment with the PRD

Each line names the file that holds the test; all are unit tests unless the path contains `tests/integration`.

| Requirement | Test file | Cases |
| --- | --- | --- |
| PD-18, FR20(e) | `some-module-application/src/command/create_example_api_resource/tests.rs` | with `RecordingEventBus::failing()` the handler returns `Ok(())`, the resource is stored, and one `event publication failed` entry at ERROR carries the event name, the event id and the error (`LogRecorder`) |
| PD-18 | `some-module-application/src/http/handler/tests.rs`; `service/src/bootstrap/tests.rs` | the create request answers 201 when the event bus fails (handler level with fakes; wiring level with `adapters.event_bus` replaced) |
| FR26, PD-19 | `shared-infrastructure/src/bus/event/tests.rs` | two failing subscribers out of three: the third still runs and both failures are returned in invocation order with event name and event id; no subscriber: `Ok`; `Debug` output |
| FR26 | `shared-domain/src/bus/event/tests.rs` | the messages of `SubscriberFailure` and `EventBusError` |
| FR22, PD-1 | `some-module-application/src/http/handler/tests.rs`, `http/dto/tests.rs` | `page=0`, `page=-1`, `itemsPerPage=0`, `itemsPerPage=101` are 422 with a `range` violation; `page=abc`, `page=1.5`, `page=4294967296` are 400; a page beyond the last is 200 with `[]` |
| FR24(e) | `some-module-application/src/graphql/tests.rs` | `page: -1`, `page: 0`, `itemsPerPage: 101` and an `id` that is not a UUID return `VALIDATION_FAILED` with violations; a storage failure returns `INTERNAL_SERVER_ERROR` without detail |
| FR9 | `service/src/graphql/handler/tests.rs` | bodies `{}`, `[]`, `{"foo":1}` and `{"query":5}` are the 422 problem; a valid body is 200 |
| FR9, NFR13, PD-27 | `service/src/graphql/handler/tests.rs`; `service/src/http/router/tests.rs` (the absence outside `dev`) | the explorer page: every `src` and `href` starts with `https://unpkg.com/` and contains `@<major>.<minor>.<patch>/`, has an `integrity` attribute; the policy header names exactly that origin; the page is absent (405) outside `dev` |
| FR24(d) | `service/src/graphql/schema/tests.rs` | the SDL has exactly the three operations and no node field, connection type, update or delete mutation or subscription |
| PD-21, NFR5 | `service/tests/integration/serve.rs` | idle exit 0 within 2 s; a request in flight completes and the exit status is 0; a connection attempted after SIGTERM is refused; a connection still open when a 1 s grace period ends gives exit status 1 within grace period + 2 s |
| NFR31 | `service/tests/integration/serve.rs`, `panic.rs` | every line of a request sequence is JSON and carries the request id, stderr is empty; an invalid variable yields only JSON lines and exit status 2; the forced panic (in process, production telemetry) |
| FR126, NFR2, NFR3 | `service/tests/integration/serve.rs` | database and SQS endpoint on a dead port: `/ping` 200 within 5 s of process start, `/api/health` 500 naming the database and the message broker, and not the cache, within `HEALTH_PROBE_TIMEOUT` + 1 s |
| NFR6 | `service/tests/integration/bootstrap.rs` | a resource created through one application instance is read through a second one |
| FR4, configuration table | `service/src/config/tests.rs`; `tests/integration/check_config.rs` | one credential variable set without the other is reported and names the missing one; a timeout above its PRD ceiling is accepted; `serve --help` with `DATABASE_URL` set does not contain the URL |
| FR5 | `service/src/bootstrap/tests.rs` | each port replaced once: repository, a probe, the cache behind the cache probe, the event bus, the clock, the UUID generator |
| NFR28 | `shared-infrastructure/tests/integration/{clock,id}.rs` | the two adapters that own the banned calls |
| FR44 | `xtask/src/config_guard/check/tests.rs`; `xtask/tests/integration/config_guard.rs` | deleted file, deleted table, changed and missing JSON value, nested override, exempt fixture path, label name in the failure output |
| FR39(b) | `xtask/tests/integration/scaffold.rs` | `cargo metadata --locked` succeeds after the command; `arch` passes; each new package checks alone |

### Test doubles (crate `test-support`)

| Double | Port | Behaviour |
| --- | --- | --- |
| `clock::FixedClock::at(time)` | `Clock` | always returns `time` |
| `id::SequenceUuidGenerator::starting_at(n)` | `UuidGenerator` | returns version-7-shaped UUIDs with increasing counters |
| `bus::FakeCommandBus::handling(closure)` / `::unregistered()` | `CommandBus` | records each command; answers with the closure's `Result<(), C::Error>` or `NotRegistered` |
| `bus::FakeQueryBus::answering(closure)` / `::unregistered()` | `QueryBus` | answers with the closure's `Result<Q::Output, Q::Error>` or `NotRegistered` |
| `bus::RecordingEventBus::new()` / `::failing()` | `EventBus` | records published events; the failing variant also returns `EventBusError` with one `SubscriberFailure` per published event |
| `cache::StubCache::{working, failing, forgetful, corrupting}()` | `Cache` | stores in a map; errors on every call; accepts writes and returns nothing; returns a different value |
| `probe::StubProbe::{healthy, failing, hanging, panicking}(dependency)` | `HealthProbe` | `Ok`; `Err`; never completes; panics |
| `repository::InMemoryExampleApiResourceRepository::new()` / `::failing()` | `ExampleApiResourceRepository` | a `BTreeMap` behind a mutex with the port's ordering and duplicate rule; the failing variant returns `Unavailable` |
| `log::LogRecorder::install() -> (LogRecorder, DefaultGuard)` | none (a `tracing::Subscriber`) | records level, target and fields of every event on the current thread while the guard lives; `lines()` returns `Vec<LogLine>` |

Every double derives or implements `Debug`, because the ports require it. `LogRecorder` is built on the `tracing` crate alone (no `tracing-subscriber`); the working implementation is in the skeleton (`tests/support/src/log.rs`). A unit test that asserts on a log entry runs single-threaded (`#[test]` or `#[tokio::test]` with the default current-thread runtime), because the recorder is installed for the current thread.

### Gherkin features

Step phrases are an interface shared with the PHP template. The runner calls `fail_on_skipped_with(|_feature, _rule, _scenario| true)`, so an undefined or skipped step fails the run, also in a scenario, rule or feature tagged `@allow.skipped`, which the plain `fail_on_skipped()` of cucumber 0.23.0 lets pass, and `max_concurrent_scenarios(8)`. Step vocabulary (cucumber expressions):

- `Given the cache is not working`, `Given the database is not available`, `Given the message broker is not available`
- `Given an example API resource exists with name {string} and description {string}` (creates it through `POST` and asserts the 201)
- `When {word} request is sent to {string}`, `When {word} request is sent to {string} with body:` (doc string, sent as `application/json`), `When {word} request is sent to {string} with content type {string} and body:`
- `When the GraphQL operation is sent:` (doc string, sent to `POST /api/graphql` as `{"query": …}`)
- `Then the response status code should be {int}`, `the response body should contain {string}`, `the response header {string} should contain {string}`, `the response header {string} should be present`, `the JSON field {string} should equal {string}`, `the JSON array should contain exactly the created resources`, `the GraphQL response should have no errors`, `the GraphQL field {string} should contain exactly the created resources`

The world (`AppWorld`):

- After every response it records a created resource: the `id` of a `201` body, or `data.createExampleApiResource.id` of a GraphQL response. The id is appended to the list of created ids and becomes the remembered id. In a path, a doc string or a string argument the token `{id}` is replaced by the remembered id.
- "Contain exactly the created resources" compares the set of `id` values of the JSON array (the response body, or `data.<field>` for the GraphQL step) with the set of created ids and fails when the scenario created nothing.
- It obtains its `Config` as the binary does: `Cli::parse_from(["service", "serve"])` (the method comes from `clap::Parser`, hence the `clap` dev-dependency) and `Config::from_args`, so the suite reads the same environment variables as the binary. It then replaces `config.database.url` with the URL of the scenario database ("Test database strategy").
- It builds a new application instance for every request (AD-23).

Lint notes for this crate: `main.rs` declares `#[cfg(test)] mod steps;` and `#[cfg(test)] mod world;` (rule 3a); steps that neither mutate the world nor consume a capture carry the item-level expectation of rule 15.

`features/health_check.feature` (the PHP file, unchanged; it has no trailing newline):

```gherkin
Feature: Health Check Operations
  In order to ensure system components are functioning
  As an administrator
  I want to be able to verify the health of various subsystems

  Scenario: Checking the health of the entire system
    When GET request is sent to "/api/health"
    Then the response status code should be 204

  Scenario: Checking the health when cache is unavailable
    Given the cache is not working
    When GET request is sent to "/api/health"
    Then the response status code should be 500
    And the response body should contain "Cache is not working"

  Scenario: Checking the health when the database is unavailable
    Given the database is not available
    When GET request is sent to "/api/health"
    Then the response status code should be 500
    And the response body should contain "Database is not available"

  Scenario: Checking the health when the queue service is unavailable
    Given the message broker is not available
    When GET request is sent to "/api/health"
    Then the response status code should be 500
    And the response body should contain "Message broker is not available"
```

`features/api_docs.feature` (replaces the PHP `demo.feature`, which asserted nothing):

```gherkin
Feature: API documentation
  In order to explore the API
  As a developer
  I want the service to serve its own documentation

  Scenario: The documentation page is served
    When GET request is sent to "/api/docs"
    Then the response status code should be 200
    And the response header "content-type" should contain "text/html"
    And the response body should contain "swagger-ui"

  Scenario: The OpenAPI document is served
    When GET request is sent to "/api/docs/openapi.json"
    Then the response status code should be 200
    And the response body should contain "3.1.0"
    And the response body should contain "/api/example_api_resources"
```

`features/example_api_resource.feature`:

```gherkin
Feature: Example API resource
  In order to learn the patterns of the template
  As an API client
  I want to create, read and list example API resources

  Scenario: Creating a resource
    When POST request is sent to "/api/example_api_resources" with body:
      """
      {"name": "Created by a scenario", "description": "Stored through the command path"}
      """
    Then the response status code should be 201
    And the response header "location" should contain "/api/example_api_resources/"
    And the JSON field "name" should equal "Created by a scenario"
    And the JSON field "description" should equal "Stored through the command path"

  Scenario: Reading a resource by id
    Given an example API resource exists with name "Readable" and description "Read through the query path"
    When GET request is sent to "/api/example_api_resources/{id}"
    Then the response status code should be 200
    And the JSON field "name" should equal "Readable"

  Scenario: Listing resources
    Given an example API resource exists with name "First listed" and description "Created first"
    And an example API resource exists with name "Second listed" and description "Created second"
    When GET request is sent to "/api/example_api_resources"
    Then the response status code should be 200
    And the JSON array should contain exactly the created resources

  Scenario: Rejecting invalid input
    When POST request is sent to "/api/example_api_resources" with body:
      """
      {"name": "", "description": ""}
      """
    Then the response status code should be 422
    And the response body should contain "/problems/validation-failed"
    And the response body should contain "This value must contain between 1 and 255 characters."
    And the response body should contain "This value must contain between 1 and 1024 characters."

  Scenario: Reading an unknown id
    When GET request is sent to "/api/example_api_resources/00000000-0000-7000-8000-000000000000"
    Then the response status code should be 404
    And the response body should contain "was not found"

  Scenario: Reading a malformed id
    When GET request is sent to "/api/example_api_resources/not-a-uuid"
    Then the response status code should be 400
    And the response header "content-type" should contain "application/problem+json"

  Scenario: Requesting a page size above the maximum
    When GET request is sent to "/api/example_api_resources?itemsPerPage=101"
    Then the response status code should be 422
    And the response body should contain "itemsPerPage"
```

`features/graphql.feature`:

```gherkin
Feature: GraphQL operations
  In order to use the same use cases over GraphQL
  As an API client
  I want to query and create example API resources

  Scenario: Creating a resource with the mutation
    When the GraphQL operation is sent:
      """
      mutation { createExampleApiResource(input: {name: "Created by GraphQL", description: "Same command"}) { id name description } }
      """
    Then the response status code should be 200
    And the GraphQL response should have no errors
    And the response body should contain "Created by GraphQL"

  Scenario: Reading a resource with the query
    Given an example API resource exists with name "Queried" and description "Same query"
    When the GraphQL operation is sent:
      """
      { exampleApiResource(id: "{id}") { id name description } }
      """
    Then the response status code should be 200
    And the GraphQL response should have no errors
    And the response body should contain "Queried"

  Scenario: Listing resources with the query
    Given an example API resource exists with name "Listed by GraphQL" and description "First page"
    When the GraphQL operation is sent:
      """
      { exampleApiResources(itemsPerPage: 100) { id name } }
      """
    Then the response status code should be 200
    And the GraphQL response should have no errors
    And the response body should contain "{id}"

  Scenario: Creating and reading through GraphQL
    When the GraphQL operation is sent:
      """
      mutation { createExampleApiResource(input: {name: "Round trip", description: "Mutation and both queries"}) { id name description } }
      """
    Then the response status code should be 200
    And the GraphQL response should have no errors
    When the GraphQL operation is sent:
      """
      { exampleApiResource(id: "{id}") { id name description } }
      """
    Then the GraphQL response should have no errors
    And the response body should contain "Round trip"
    When the GraphQL operation is sent:
      """
      { exampleApiResources { id name } }
      """
    Then the GraphQL response should have no errors
    And the GraphQL field "exampleApiResources" should contain exactly the created resources

  Scenario: The explorer is disabled outside development
    When GET request is sent to "/api/graphql"
    Then the response status code should be 405
    And the response header "content-type" should contain "application/problem+json"
```

`features/http_errors.feature`:

```gherkin
Feature: HTTP surface
  In order to rely on uniform behaviour
  As an API client
  I want liveness, error documents and response headers to follow one contract

  Scenario: Liveness
    When GET request is sent to "/ping"
    Then the response status code should be 200
    And the response body should contain "pong"

  Scenario: Unknown route
    When GET request is sent to "/api/unknown"
    Then the response status code should be 404
    And the response header "content-type" should contain "application/problem+json"

  Scenario: Unsupported method
    When DELETE request is sent to "/api/example_api_resources"
    Then the response status code should be 405
    And the response header "allow" should contain "POST"

  Scenario: Unsupported media type
    When POST request is sent to "/api/example_api_resources" with content type "text/plain" and body:
      """
      name=plain
      """
    Then the response status code should be 415
    And the response header "content-type" should contain "application/problem+json"

  Scenario: Malformed JSON
    When POST request is sent to "/api/example_api_resources" with body:
      """
      {not json
      """
    Then the response status code should be 400
    And the response header "content-type" should contain "application/problem+json"

  Scenario: Request id and security headers
    When GET request is sent to "/ping"
    Then the response header "x-request-id" should be present
    And the response header "x-content-type-options" should contain "nosniff"
    And the response header "x-frame-options" should contain "DENY"
    And the response header "referrer-policy" should contain "no-referrer"
    And the response header "content-security-policy" should contain "frame-ancestors 'none'"
```

Totals: 5 features, 24 scenarios, 97 steps. The end-to-end suite runs with `APP_ENV=test`, so the explorer scenario expects 405. FR57 is covered as follows: the documentation page by `api_docs.feature`; create then read by id, the list with exactly the created items, the validation failure and the unknown id by `example_api_resource.feature`; the create mutation and both queries in one scenario by "Creating and reading through GraphQL". Verified in this revision: the five files as listed here pass (24 scenarios, 97 steps) with one database per scenario; the first version (23 scenarios, 87 steps) passed in the validation round. `features/health_check.feature` as listed equals the PHP file apart from the trailing newline (compared with `diff`).

## Requirements to Architecture Mapping

The baseline is the final PRD: FR1 to FR126, NFR1 to NFR47, gates G1 to G31. Every id has a row below. "Delivery" means `architecture-delivery.md` owns the requirement; the cell names the section (and decision) of that document and what this document contributes.

### Capability-catalog areas

| # | Catalog area | Delivered by (this document) | Gates | Delivery document owns |
| --- | --- | --- | --- | --- |
| 1 | Service runtime and HTTP surface | `service` (`cli`, `config`, `http/*`, `command/*`, `telemetry`); `shared-application::http` | QG-12 to QG-15 | compose wiring of the port |
| 2 | Health and liveness | `health-check-domain`, `-application`, `-infrastructure`; `service::http::ping`; `service healthcheck` | QG-13 to QG-15 | container healthcheck declaration |
| 3 | Example bounded context | `some-module-domain`, `-application`, `-infrastructure`; migration; `features/example_api_resource.feature`, `features/graphql.feature` | QG-13 to QG-15 | none |
| 4 | Shared kernel | `shared-domain`; `shared-infrastructure::bus` | QG-13, QG-17 | none |
| 5 | Persistence, cache and messaging | `shared-infrastructure::{persistence, cache, messaging}`; `some-module-infrastructure`; `migrations/`; `.sqlx/`; `service migrate` | QG-11 (G30), QG-14 | Make targets for migrations, compose services |
| 6 | Architecture and its enforcement | `architecture.toml`; `xtask::arch`; `xtask::scaffold` | QG-05, QG-08 (G29) | `workspace.dsl` |
| 7 | Formatting, static analysis and complexity | `rustfmt.toml`, `clippy.toml`, `[workspace.lints]`, `protected-config.toml`, `xtask::config_guard` | QG-01 to QG-04, QG-06, QG-20 | linters for non-Rust files |
| 8 | Dependency and security checks | `deny.toml`, `Cargo.lock` | QG-07, QG-09, QG-10 | secrets scan, image scan, CodeQL, SARIF upload, repository invariants |
| 9 | Unit and integration tests | sibling `tests.rs`, `tests/integration/`, `test-support`, `.env.test`, recorded seeds | QG-13, QG-14 | `setup-test-db` target |
| 10 | End-to-end tests | `tests/e2e`, `features/` | QG-15 | target name |
| 11 | Coverage and mutation testing | `.cargo/mutants.toml`, `.config/nextest.toml` | QG-16 to QG-18 | Codecov upload, sharding matrix |
| 12 | Makefile command surface and its tests | command lines, exit statuses and the env-file contract defined here | all | Makefile, Bats suite, execution switch |
| 13 | Contract artifacts and contract checks | `service::openapi`, `service::graphql::schema`, export commands | QG-19 | Spectral, oasdiff, GraphQL schema diff, Schemathesis |
| 14 | Load testing | `GET /api/health` contract; production binary; `json_values` of the guard | none | K6 kit, AWS runner |
| 15 | Containers and compose stacks | binary `service`, subcommands, ports, environment table, offline build inputs (`.sqlx/`, `migrations/`) | none | Dockerfile, compose files |
| 16 | CI workflows | gate commands and their service requirements | all | workflows |
| 17 | Release, dependency updates and template sync | version of record `[workspace.package].version`; `API_VERSION` independent of it | QG-07 | release, Dependabot, sync |
| 18 | Git hooks and commit convention | fast gates: QG-01, QG-02, QG-03, QG-05, QG-09 (offline subset), QG-10; pre-push QG-13 | | `.githooks/`, title check |
| 19 | AI-assistant guidance | "Implementation Patterns" recipes; `protected-config.toml` as the single list | QG-06 | `AGENTS.md`, skills, settings, drift check |
| 20 | Human documentation | environment table, command table, operations facts, decisions AD-01 to AD-27 | none | all documents |
| 21 | Architecture model (Structurizr) | crate table and layer placement as the component list | none | `workspace.dsl`, validation gate |

### Functional requirements

| FR | Delivered by | Verified by |
| --- | --- | --- |
| FR1 | `service serve`; `APP_ENV`, `APP_PORT`; AD-12, AD-18; `axum` without `http2`; `listening` entry with `env` | `service/tests/integration/serve.rs`; `http/router/tests.rs` (`no_response_is_compressed_and_no_other_protocol_is_advertised`: `/ping`, `/api/example_api_resources`, `/api/docs/swagger-ui-bundle.js` and `/api/unknown`, each without and with `accept-encoding: gzip, deflate, br, zstd`, carry no `content-encoding`, `alt-svc` or `strict-transport-security` and the same body) |
| FR2 | subcommand table in "Configuration" (seven commands) | `service/tests/integration/*` |
| FR3 | environment table; `.env`, `.env.test` | `service/src/config/tests.rs` |
| FR4 | AD-07; `service check-config`; pair rule for the AWS credentials | `config/tests.rs` (`AWS_ENDPOINT_URL_SQS` is parsed as a URI: scheme `http` or `https`, a non-empty host, and after the optional user information either the host alone or the host, `:` and one or more ASCII digits that give a port from 0 to 65535; no sign and no other text after the host), `integration/check_config.rs`; QG-12 (G6) |
| FR5 | AD-04; `bootstrap::Adapters`, `bootstrap::event_bus`, `bootstrap::router` | `bootstrap/tests.rs` (each port replaced once); `e2e` |
| FR6 | AD-06; `shared-application::http` | handler and router unit tests (in the service crate `handlers_use_only_the_problem_aware_extractors` checks the handlers of `src/http` and `src/graphql`; `GET /api/docs/%FF` is the 400 problem); `http_errors.feature` |
| FR7 | AD-22; `ValidatedJson`, `ValidatedQuery` | `extract/tests.rs`; `example_api_resource.feature` |
| FR8 | AD-19; `service::http::docs`; `SWAGGER_UI_DOWNLOAD_URL = "vendored-only"` in `.cargo/config.toml` (a build without the `vendored` feature stops instead of downloading Swagger UI) | `docs/tests.rs` (`the_swagger_ui_assets_come_from_the_vendored_crate`: the lock file holds `utoipa-swagger-ui-vendored` and `.cargo/config.toml` holds the guard as an active line, so a commented-out guard fails it); `api_docs.feature` |
| FR9 | AD-18, AD-14; `service::graphql::handler` (422 for a body that is not a GraphQL request; pinned explorer page; policy naming the origin) | `graphql/handler/tests.rs`; `graphql.feature` |
| FR10 | AD-13; `service::http::{request_id, trace}`, `telemetry` | `integration/serve.rs` (also `entries_inside_nested_spans_carry_the_request_id`, `LOG_LEVEL=debug`); `request_id/tests.rs`, `trace/tests.rs` |
| FR11 | AD-14; `service::http::security_headers`; the two page policies | `router/tests.rs`, `docs/tests.rs`, `graphql/handler/tests.rs`; `http_errors.feature` |
| FR12 | AD-15; `command/serve.rs` (exit 0 drained, exit 1 forced) | `integration/serve.rs` |
| FR13 | AD-08, AD-10, AD-15; configuration table | dead-port integration tests; `middleware/tests.rs` |
| FR14, FR15 | AD-11; `health-check-application` | `service/tests.rs` (7 combinations), `http/tests.rs`; `health_check.feature` |
| FR16 | `health-check-infrastructure::probe`; AD-10 | probe unit and integration tests |
| FR17 | `HealthService`; `Adapters::probes` | `service/tests.rs` (paused clock) |
| FR18 | AD-12; `service::http::ping` | `ping/tests.rs`; `http_errors.feature`; `openapi/tests.rs` (absent from the document) |
| FR19 | `some-module-application::http` (`create`, `location` header) | handler tests; `example_api_resource.feature` |
| FR20 | AD-22 (a); `CreateExampleApiResourceHandler` (b to d); AD-08 and `log_publication_failure` (e: 201 and an ERROR entry with the event id) | `create_example_api_resource/tests.rs`, `http/handler/tests.rs`, `bootstrap/tests.rs` |
| FR21 | `some-module-application::query::get_example_api_resource`; `ApiPath<uuid::Uuid>` | handler tests; `example_api_resource.feature` |
| FR22 | `ListExampleApiResourcesParams` (signed, AD-22); `PageRequest`; "Pagination" (order `id` descending; default 30, maximum 100) | `http/handler/tests.rs`, `http/dto/tests.rs`; `integration/repository.rs` |
| FR23 | `AuditExampleApiResourceCreated`, `log_event` | subscriber unit test with `LogRecorder` |
| FR24 | `some-module-application::graphql`; `service::graphql::schema`; behaviour table in "API Contract" | `graphql/tests.rs`, `schema/tests.rs`; `graphql.feature`; SDL snapshot |
| FR25 | `shared-domain::{aggregate, event}`; `Clock` | `aggregate/tests.rs` |
| FR26 | AD-05; `EventBus`, `EventBusError`, `SubscriberFailure`; `InMemoryEventBus` (every failure returned) | `shared-domain/src/bus/event/tests.rs`, `shared-infrastructure/src/bus/event/tests.rs` |
| FR27 | AD-05; `CommandBus`, `QueryBus`, `BusError::NotRegistered`; in-memory adapters | bus unit tests |
| FR28 | AD-05; `register`, `subscribe`, `DuplicateHandler`; `bootstrap` | bus unit tests; wiring test |
| FR29 | `shared-domain::value_object::uuid`; `UuidV7Generator` | `uuid/tests.rs`; `integration/id.rs` |
| FR30 | `DATABASE_URL`; test database strategy | `integration/persistence.rs` |
| FR31 | `ExampleApiResourceRepository`; `PgExampleApiResourceRepository` | `integration/repository.rs` |
| FR32 | AD-20; `migrations/`; `service migrate`; (d) Delivery: "Containers and Compose" (the `migrate` step) | `integration/migrate.rs`, `integration/persistence.rs`; `integration/serve.rs` (`serve_applies_no_migration_to_a_database_that_was_never_migrated`: (c)) |
| FR33 | AD-20; `service migrate revert` | `integration/migrate.rs`, `integration/persistence.rs` |
| FR34 | AD-09; `Cache`, `MokaCache` | `cache/tests.rs`, `integration/cache.rs` |
| FR35 | AD-10; `sqs_client`; pair validation of the credentials (AD-07) | `integration/messaging.rs`; `config/tests.rs` |
| FR36, FR37 | AD-01, AD-02; crate table | QG-05 (G4) |
| FR38 | AD-03; `architecture.toml`; `xtask::arch` (eight rules) | `xtask` tests; QG-05 (G4) |
| FR39 | AD-26; `xtask::scaffold` (lock file refreshed) | `xtask/tests/integration/scaffold.rs` |
| FR40 | `rustfmt.toml` | QG-01 (G1) |
| FR41 | `[workspace.lints]`, `clippy.toml`; AD-27 | QG-02, QG-03, QG-04 (G2) |
| FR42 | fix commands under "Quality Gates" | QG-02 fails while a fix is pending |
| FR43 | Delivery: "Repository Lint and Hygiene" | G23 |
| FR44 | AD-25; `protected-config.toml` (files, directories, TOML tables, JSON values, override names); `xtask::config_guard`; CI part in Delivery DD-16 | `xtask` tests; QG-06 (G25) |
| FR45 | `--locked` on every command of the gate table; `Cargo.lock` | QG-07 (G5) |
| FR46 | `deny.toml` | QG-09 (G7) |
| FR47, FR48 | Delivery: DD-21 "Secrets scan and image scan" | G9, G10 |
| FR49 | Delivery: DD-24 "Security analysis in CI"; this document supplies the clippy command line for SARIF | none (publication, not a gate) |
| FR50 | rules 1 to 6a of "Implementation Patterns"; `test-support`; documentation tests in QG-13 | QG-13 (G11) |
| FR51 | `test_support::seed::TestRng`; `TEST_SEED` and `PROPTEST_RNG_SEED` recorded in `.cargo/config.toml`; fresh seed in the scheduled workflow (Delivery: "CI/CD Workflows") | `seed` behaviour exercised by every fixture; verified `[env]` precedence |
| FR52 | integration targets; `.env.test`; process-level tests | QG-14 (G12) |
| FR53 | "Test database strategy" (`setup-test-db` contract); Delivery: "Command Surface" (`setup-test-db`, `setup-load-test-db`) | Delivery Bats tests |
| FR54 | probe unit tests (`cache`, `sqs`); `integration/{postgres_probe,sqs_probe}.rs`; reading for `PostgresProbe` in AD-11 | QG-13, QG-14 |
| FR55 | AD-23; `tests/e2e`; one database per scenario | QG-15 (G13) |
| FR56 | AD-23; `StubProbe::failing` through `Adapters::with_probe`; one application instance per request | QG-15 |
| FR57 | feature files ("Gherkin features", last paragraph) | QG-15 |
| FR58 | coverage sequence without the end-to-end suite; exclusion record (AD-24); (b) to (d) Delivery: "Command Surface", "CI/CD Workflows" | QG-16 (G14) |
| FR59 | AD-24; `.cargo/mutants.toml` (scope record; timeouts count as surviving) | QG-17, QG-18 (G15) |
| FR60 to FR66 | Delivery: "Command Surface" (DD-01 to DD-03); this document supplies every command line, exit status, the env-file contract and the tool versions | Delivery: "CLI Tests (Bats)" |
| FR67 | Delivery: "CLI Tests (Bats)" (DD-06, DD-07) | G16 |
| FR68 | Delivery: "Command Surface", "Repository File Manifest"; this document writes only to `target/`, `.sqlx/` and the two snapshots | Delivery Bats tests |
| FR69, FR70 | Delivery: "Command Surface" (`doctor`, `bootstrap`; DD-04) | Delivery Bats tests |
| FR71 | AD-16; "API Contract" | `openapi/tests.rs`; G18 (Delivery) |
| FR72 | `service openapi-export`, `service graphql-export`; AD-17 | `integration/export.rs` |
| FR73 | `API_VERSION`; AD-16 | QG-19 (G17) |
| FR74 to FR76 | Delivery: "Command Surface" (contract targets), DD-10, DD-19, DD-22; this document supplies the document content the tools check | G18 to G21 |
| FR77 to FR82 | Delivery: "Load Testing" (DD-19, DD-25); this document supplies `GET /api/health` and the production binary | G22 |
| FR83 | Delivery: "Load Testing" (JavaScript lint) | G24 |
| FR84, FR85 | Delivery: "Load Testing"; "CI/CD Workflows" (`load-tests.yml`) | G22; Delivery Bats tests |
| FR86 to FR91 | Delivery: "Containers and Compose" (DD-20); this document supplies binary name, subcommands, `APP_PORT=8081`, environment variables, exit statuses, healthcheck command | Delivery Bats tests |
| FR92 | Delivery: DD-23 "Project name"; this document keeps the repository name out of crate and binary names and states it only in `[workspace.package]` | G31 rule (e) |
| FR93 to FR97 | Delivery: "CI/CD Workflows" (DD-08, DD-09, DD-13); gate commands and service requirements from "Quality Gates" | workflows |
| FR98 to FR101 | Delivery: "CI/CD Workflows" (release, Dependabot, template sync; DD-11, DD-12, DD-14); version of record `[workspace.package].version` (a bump must be followed by `cargo update --workspace`) | release job |
| FR102, FR103 | Delivery: "Git Hooks and Commit Convention" (DD-15); hook gate set listed under catalog area 18 | G28; hooks |
| FR104 to FR112 | Delivery: "AI Guidance and Documentation" (DD-17, DD-18); "Implementation Patterns", the protected list and `cargo xtask config-guard --list` are inputs | G26 |
| FR113 to FR118 | Delivery: "AI Guidance and Documentation" (human documentation), "Containers and Compose" (development container); environment and command tables, AD-08 (at-most-once delivery) and NFR6 statements are inputs | G26 |
| FR119, FR120 | Delivery: "Architecture Model" (DD-05); crate table is the component list | G27 |
| FR121 | "Offline query metadata"; `.sqlx/`; `SQLX_OFFLINE` in `.cargo/config.toml` | QG-11 (G30) |
| FR122 | one manifest per crate, no cargo features; AD-01 | QG-08 (G29) |
| FR123 | `clippy.toml` thresholds; AD-27 | QG-20 (G3) |
| FR124 | manifests; `[bans.workspace-dependencies] unused = "deny"` | QG-10 (G8) |
| FR125 | Delivery: "Command Surface" (`validate-invariants`, `scripts/check-invariants.sh`); this document supplies `rust-version = "1.99.1"`, the `allow_attributes*` lints, rule 15, crate and binary names | G31 |
| FR126 | AD-08 (lazy pool), AD-10 (client built without network access), AD-12 | `integration/serve.rs` with unreachable dependencies, and `health_recovers_without_a_restart_when_the_database_comes_back` (500 naming only the database while it is missing, 204 from the same process once it exists) |

### Non-functional requirements

| NFR | Delivered by | Value fixed here |
| --- | --- | --- |
| NFR1 | Delivery: "Load Testing" (DD-25) | none; the health endpoint latency is bounded by the probes |
| NFR2 | AD-11 | `HEALTH_PROBE_TIMEOUT` default 3 s; unit test with the paused clock and `integration/serve.rs` against dead ports |
| NFR3 | AD-12; "Operations"; Delivery: "Containers and Compose" | interval 10 s, timeout 3 s, retries 3, start period 10 s; `/ping` within 5 s of process start |
| NFR4 | AD-08, AD-10, AD-15 | defaults: acquire 2 s, statement 5 s, SQS 2 s, request budget 10 s |
| NFR5 | AD-15 | grace period 10 s; idle exit 0 under 2 s; forced exit 1 within 2 s; refused connections after SIGTERM |
| NFR6 | AD-09, AD-23 | `integration/bootstrap.rs` reads through a second application instance |
| NFR7 | AD-17; scheduled check in Delivery: "CI/CD Workflows" | `integration/export.rs` runs 20 generations |
| NFR8 | Delivery: DD-13 "Behaviour when secrets are absent" | none |
| NFR9 | `deny.toml`; QG-07, QG-09, QG-10 | licence allow-list of 9 identifiers |
| NFR10, NFR11 | Delivery: DD-21 | `.env` values chosen to be low-entropy placeholders |
| NFR12 | Delivery: "Containers and Compose"; "CLI Tests (Bats)" | binary is self-contained; healthcheck needs no shell |
| NFR13 | `rust-toolchain.toml`, `[workspace.package].rust-version`; pinned explorer assets (AD-18); other pins in Delivery: "Command Surface" (`make/tool-versions.mk`), "CI/CD Workflows" (action pins) | channel and `rust-version` both `1.99.1`; four assets with exact versions and digests |
| NFR14 | Delivery: "CI/CD Workflows" (conventions for every workflow) | none |
| NFR15 | AD-13, AD-14 | router unit tests over every route, unknown route, forced panic |
| NFR16 | `rustfmt.toml`; rules 14 and 15; G31 rule (g) in Delivery | `max_width = 100`; one module-level suppression, for macro-generated code |
| NFR17 | `clippy.toml`; QG-20 | cognitive complexity 8, 50 lines, nesting 5, 5 parameters; tests relaxed for complexity and length only |
| NFR18 | AD-03, AD-26; model part in Delivery: "Architecture Model" | 8 rules; scaffold test runs the architecture gate and checks each new crate alone |
| NFR19 | AD-24 | lines 100, functions 100; unit and integration suites only; exclusion record in `make/gates.mk` with 0 entries |
| NFR20 | AD-24 | 0 survivors; judge = unit + integration tests of the mutated crate; scope record `.cargo/mutants.toml` with 4 path entries and 0 equivalent mutants |
| NFR21 | Delivery: "CLI Tests (Bats)" (DD-07); checker-level tests in `xtask` | each Rust gate accepts a fixture path (`--manifest-path`, `--config`, `--base`) for negative tests; plants as described under "Gate commands" |
| NFR22 | Delivery: DD-02 "Content and order of `make ci`"; G31 rule (d) | gate ids QG-01 to QG-20 as the Rust-side list |
| NFR23 | scope principle 5 applied to the tree | QG-10; `[bans.workspace-dependencies] unused = "deny"`; dead-code analysis in QG-02 |
| NFR24 | "API Contract"; Delivery: contract targets | every operation documents a 2xx, its 4xx and `default` |
| NFR25 | Delivery: "Repository Lint and Hygiene"; "Load Testing" | none |
| NFR26 | AD-25; Delivery: DD-16 | `protected-config.toml`: files, directories, TOML tables, JSON values, override names |
| NFR27 | "Requirements to Architecture Mapping"; planning bundle in Delivery: "AI Guidance and Documentation" | this section |
| NFR28 | clippy `disallowed-methods` list; rules 2, 3 and 20 | 10 banned wall-clock, UUID and socket APIs; 4 production items and the test support helpers carry the reasoned suppression |
| NFR29 | `TestRng`, recorded seeds; `#[sqlx::test]`; one database per scenario; nextest (one process per test) | seed `20261110` |
| NFR30 | `HealthService` unit tests; `health-check-application` `http/tests.rs` | 7 combinations of the report and of the `500` body (`detail`, `failures`, no message of a healthy dependency, no cause); `health_check.feature`, `fail_on_skipped_with` |
| NFR31 | AD-07, AD-13; plain-text `migrate` | `integration/serve.rs` (request sequence, entries of nested spans at `debug` level, invalid configuration, bind failure) and `integration/panic.rs` (forced panic) |
| NFR32 | AD-20; QG-11 | `integration/persistence.rs`, `integration/migrate.rs` |
| NFR33 | AD-20; compose part in Delivery: "Containers and Compose" | `DATABASE_WAIT_TIMEOUT` 30 s, one attempt per second (`config/tests.rs`, `the_migrate_wait_timeout_defaults_to_thirty_seconds`) |
| NFR34, NFR35 | Delivery: DD-02; "CI/CD Workflows" (`scheduled.yml`) | measured runtimes per gate in "Gate commands" |
| NFR36 | Delivery: DD-01 "Make execution model" | `check-config` exit statuses |
| NFR37 | Delivery: DD-04, DD-22; "Command Surface" (`doctor`) | tool versions |
| NFR38 | AD-01, AD-16; Delivery: DD-23 | crate `service`, binary `service`; identity only in `[workspace.package]` |
| NFR39 to NFR42 | Delivery: "CI/CD Workflows" | service requirements and measured runtimes per gate |
| NFR43 to NFR46 | Delivery: "AI Guidance and Documentation" (DD-17) | none |
| NFR47 | `[workspace.package].license`; `ApiDoc` licence; G31 rule (f) in Delivery | `CC0-1.0` |

### Gates

| Gate | Name | Home |
| --- | --- | --- |
| G1 | Formatting check | QG-01 |
| G2 | Static analysis | QG-02, QG-03, QG-04 |
| G3 | Complexity limits | QG-20 |
| G4 | Architecture boundaries | QG-05 |
| G5 | Manifest and lock validation | QG-07 |
| G6 | Configuration check | QG-12 |
| G7 | Dependency policy | QG-09 |
| G8 | Unused dependencies | QG-10 |
| G9 | Secrets scanning | Delivery: DD-21 "Secrets scan and image scan" |
| G10 | Production-image scan | Delivery: DD-21 "Secrets scan and image scan" |
| G11 | Unit tests, with documentation tests | QG-13 |
| G12 | Integration tests | QG-14 |
| G13 | End-to-end tests (Gherkin) | QG-15 |
| G14 | Coverage | QG-16 |
| G15 | Mutation testing | QG-17, QG-18 |
| G16 | Bats tests of the Makefile | Delivery: "CLI Tests (Bats)" |
| G17 | Contract snapshot equality | QG-19 |
| G18 | OpenAPI lint | Delivery: "Command Surface" (`validate-openapi-spec`), DD-22 |
| G19 | OpenAPI breaking-change check | Delivery: "Command Surface" (`openapi-diff`), DD-10 |
| G20 | GraphQL schema diff | Delivery: "Command Surface" (GraphQL schema diff target), DD-10 |
| G21 | Schemathesis | Delivery: "Command Surface" (`schemathesis-validate`), DD-19 |
| G22 | K6 smoke load | Delivery: "Load Testing" |
| G23 | Repository lint | Delivery: "Repository Lint and Hygiene" |
| G24 | Load-script lint | Delivery: "Load Testing" (JavaScript lint) |
| G25 | Protected-configuration guard | QG-06; CI evaluation in Delivery: DD-16 |
| G26 | Documentation-drift check | Delivery: DD-17 "Documentation-drift check" |
| G27 | Architecture-model validation | Delivery: "Architecture Model" |
| G28 | Pull-request title convention | Delivery: "Git Hooks and Commit Convention", DD-15 |
| G29 | Crate isolation | QG-08 |
| G30 | Persistence query metadata | QG-11 |
| G31 | Repository invariants | Delivery: "Command Surface" (`validate-invariants`, `scripts/check-invariants.sh`) |

## Architecture Validation Results

### Coherence

- **Technology compatibility.** One axum (0.8) serves utoipa-axum 0.3 and the hand-written GraphQL route; one tokio; one TLS backend (rustls with aws-lc-rs) for sqlx and the AWS SDK; no OpenSSL (`deny.toml` bans it). The whole set resolves on cargo 1.99.1 (468 packages) and passes `cargo deny check`.
- **Layer rules against the dependency lists.** Every third-party dependency in the crate table is on the allow-list of its crate's layer in `architecture.toml`; no Domain crate uses anything beyond `async-trait`, `thiserror`, `uuid`. Checked by a reduced checker over `cargo metadata` of the skeleton in the validation round, and by hand for the manifest changes of this revision (`serde_json` in `service` and `xtask`, `tracing` in `test-support`, `clap`, `sqlx` and `shared-infrastructure` in `e2e-tests`, `test-support` in `shared-application`, no `tracing` in `shared-infrastructure`).
- **Internal consistency after the revision.** The tree, the crate table, `architecture.toml`, the root manifest, the gate table and the test architecture were re-read against each other: 13 crates in each; the same dependency lists in the crate table and the allow-lists; 20 gate commands in the gate table and in the mapping; the same seven commands in the CLI listing, the subcommand table and the tests; eight architecture rules in AD-03, the tree and NFR18.
- **Spike disagreements, resolved in favour of `scope-decisions.md`:**

| Topic | Spike value | Decision |
| --- | --- | --- |
| Probe port name | `ReadinessProbe` (HTTP), `DependencyHealthCheck` (data), `ReadinessProbe` (quality) | `HealthProbe` |
| Failure detail | lower-case `database is unavailable` (HTTP) | the three exact PHP strings |
| Failures reported | first only (data) | all, in registration order |
| Broker probe | `CreateQueue health-check-queue` (data) | `ListQueues` with `max_results(1)` |
| Listening port | 8080 (HTTP, testing) | `APP_PORT` default 8081 |
| Liveness | `GET /api/health/live` (HTTP) | `GET /ping`, body `pong` |
| Example path | `/api/example-resources` (HTTP) | `/api/example_api_resources` |
| Tracing and metrics | OpenTelemetry and Prometheus features (HTTP) | not shipped |
| Cache port location | Application (data) | `shared-domain::port` (AD-02) |
| Gherkin target | `bdd` test in the `app` crate, features under `tests/features` (testing) | crate `e2e-tests`, target `e2e`, features under `features/` |
| Composition root name | `app` (testing), `service` (quality) | `service` |
| Context tables in `architecture.toml` | explicit `[[context]]` (quality) | derived from crate names (AD-03) |
| Mutation judge | unit tests only (quality) | unit and integration tests of the mutated crate (AD-24) |
| Docs path | `303` to `/api/docs/` (HTTP) | `200` at `/api/docs` (AD-19) |

### Validation round of 2026-11-10

Three independent reviews (requirements against the final PRD, Rust feasibility on a compiled skeleton, delivery and CI) produced 54 findings that touch this document. 53 are applied, some in an adapted form; one is not applied. The first version of this document predated the final PRD; where the two disagreed the PRD prevailed (scope-decisions 8.11).

Alignments with the final PRD:

| PRD item | Before | Now |
| --- | --- | --- |
| Order of authority (scope 8.11) | this document above the PRD | scope decisions, then the PRD, then the architecture documents |
| PD-18, FR20(e) | 500 when event publication fails after the save | 201; one ERROR entry per failing subscriber with the event id |
| PD-19, FR26 | the bus logs and returns the first failure | the bus returns every failure and logs nothing |
| PD-21, FR12, NFR5 | exit 0 after a forced end of the grace period | exit 1 within 2 s; refused connections after SIGTERM are tested |
| PD-28, FR58(a), NFR19 | coverage from three runs, the end-to-end run included | unit and integration suites only; in-process wiring test added; exclusion record named |
| FR59(c) | a hanging mutant counted as caught | a timed-out mutant counts as surviving (exit status 3) |
| FR51, NFR29 | random seed per test process | recorded seed `20261110` by default; fresh seed in the scheduled run |
| FR55(d), FR57, NFR29 | scenarios and `service` tests shared `app_test` | one database per test and per scenario; exact list assertion; one scenario with the mutation and both queries |
| FR22, FR24(e), PD-1 | unsigned page parameters with an upper bound of 100000 | signed parameters, no upper bound for `page`; out-of-bounds is 422 or `VALIDATION_FAILED` |
| FR9 | `{}` and `[]` executed as GraphQL requests | 422 problem for a body that is not a GraphQL request |
| FR9, FR11, NFR13, PD-27 | library explorer page, policy `frame-ancestors 'none'` only | committed page with four pinned assets from one origin; policy names the origin and the needed directives |
| NFR31 | plain-text configuration errors of `serve`; JSON logs of `migrate`; no panic test | JSON entries for `serve`; plain text for `migrate`; `integration/panic.rs` |
| NFR28 | clock and socket rules by review | clippy `disallowed-methods` |
| FR44(b), (c), (f); NFR26 | files, directories and TOML tables | plus JSON values, override names, deleted sections, the label name in the failure output, `xtask/tests/` protected |
| FR39(b) | lock file left stale by the scaffolder | `cargo update --workspace --offline` inside the command |
| NFR13, G31 rule (c) | `rust-version = "1.99"` | `rust-version = "1.99.1"` |
| PD-1 | a 409 and three unreachable GraphQL codes | removed |
| FR4, configuration table | credentials not validated; configured timeouts capped | pair validation; ceilings apply to defaults only |
| FR45 | four gate commands without `--locked` | every command locked |
| NFR16, FR125(g) | module-level expectations in test roots | one module-level suppression in the workspace; test modules declared `#[cfg(test)]` |
| Scope principle 7 | `migrate status`, sensitive-header layer, GraphQL limits, `arch::module`, end-to-end JUnit file, HTTP/2 feature | removed |
| FR121 to FR126, G29 to G31 | not mapped | mapped; QG-20 added for G3 |

Findings applied in an adapted form, with the reason:

| Finding | Proposed | Applied as | Reason |
| --- | --- | --- | --- |
| REQ-10 | override names derived from the file names of `files` | explicit `override_names` list with bare names and path suffixes | a second `arch.rs` or `config.toml` would have been reported |
| REQ-12 | scenario database named `e2e_<uuid>` | `e2e_<process id>_<counter>`, leftovers dropped at start-up | needs no UUID call in test code (banned API) |
| REQ-22 | subscriber installed before `Config::from_args` | installed once, after it, with `LOG_LEVEL` or with `info` | the global subscriber cannot be re-installed with another filter |
| REQ-41 | `cache` field and `event_bus: Option<..>` in `Adapters` | `event_bus: Arc<dyn EventBus>` built by `bootstrap::event_bus()`; cache replaced through the cache probe | no field without a reader; no optional port |
| RF-03 | `Config::from_environment()` in production code | `clap` dev-dependency in `e2e-tests`; the world calls `Cli::parse_from` | the helper needed an unreachable fallback branch and would be production code used only by tests |
| RF-06 | `test-support` dev-dependency in `shared-infrastructure` and `shared-application` | `shared-application` only | the event bus no longer logs, so `shared-infrastructure` has nothing to assert |
| RF-10 (3) | module-level expectation in the step files | item-level expectations on the steps | NFR16 allows a module-level suppression only for macro-generated code; the item-level form works |
| DV-14 | default seed exported by `make/gates.mk` | recorded in `.cargo/config.toml` only | one definition (FR90), also valid for bare cargo |

Not applied: DV-04 (raise NFR35 to 90 min and NFR39 to 120 min in the PRD). The figures behind it are estimates of the Bats suite, not measurements, and NFR35 and NFR39 already say that a budget is revised with a recorded measurement; the decision is left to the orchestrator and listed in the run summary.

Verified by running in the validation round (rustc 1.99.1, 13-crate skeleton with 149 source files, 65 unit tests, 23 integration tests, 23 scenarios):

- Gates: QG-01, QG-02, QG-03 and QG-04 exit 0; QG-07 0.25 s; QG-08 13 runs; QG-09 with the `deny.toml` of this document; QG-10; QG-11 current and stale; QG-13 and QG-14 through nextest (88 tests, 17 binaries); QG-15 with the five feature files of the first version (23 scenarios, 87 steps); the coverage sequence mechanics; the mutation list (354 mutants in 60 files, none from excluded paths), a sample and one shard of four; `--in-diff` selection; QG-20.
- Design: typed bus dispatch over dyn ports; `Problem` at 112 bytes with string schemas; the middleware stack in the documented order; the documentation routes; live `/ping`, `/api/health` (204 in 68 ms), `POST` (201), the explorer in `dev`, a 413 problem for a 70 kB body; the log line schema; both exports under `env -i`, 20 byte-identical generations, contract names as in "API Contract"; Schemathesis 111 of 111 cases; sqlx offline metadata, `#[sqlx::test]` with the shared `MIGRATOR`, the build-script trigger; LocalStack timings; 10 process-level tests; the `LogRecorder`; the reduced architecture checker; env-file precedence in dotenvy and Compose; clap's `hide_env_values`.
- Tool behaviour that changed the design: a cleared child environment loses coverage; `#[async_trait]` bodies are invisible to line coverage and to the complexity lints; multi-line events lose line coverage; tracing's `log` feature costs 7 complexity points per event; `CARGO_BUILD_WARNINGS=deny` empties the mutation gate; `cargo shear` rewrites the lock file without `--locked`; a digest overrides an image tag.

Verified by running for this revision (rustc 1.99.1, a copy of the skeleton with the changes of this document, `spikes/core-revision/ws`; PostgreSQL 16 and LocalStack 3.4.0 in containers):

- `clippy.toml` with the extended ban list and without `disallowed-macros`: QG-02 and QG-03 exit 0 with the item-level expectations of rule 20; `println!` in the build script passes; no invalid-path warning.
- Test targets without any crate-level or module-level expectation: `#[cfg(test)] mod ..;` in the roots of the `service` integration tests and of `e2e-tests`, item-level expectations on the cucumber steps; QG-03 exits 0.
- `command/serve.rs` with the forced-shutdown outcome: exit status 1 after 1.25 s with a 1 s grace period and an unfinished request; a request completed after the signal with exit status 0; new connections refused within 10 ms; `invalid configuration` and `serve failed` as JSON with an empty stderr; unreachable dependencies (`/ping` at once, `/api/health` 500 naming database and broker in 2.0 s).
- `telemetry::init` with a writer and `integration/panic.rs`: two JSON lines (`panic`, `request completed`), both with the request id; 500 problem body.
- Per-test databases for the `service` suite (`#[sqlx::test]`, URL handed to the spawned binary; migration round trip with `migrations = false`; the in-process wiring test): 17 tests pass in 3.0 s.
- The end-to-end suite with one database per scenario, the `after` hook, `sqlx::AssertSqlSafe` for the two DDL statements, the new steps and scenarios, and `clap` in the world: 5 features, 24 scenarios, 97 steps pass in 5 s; no database left behind.
- Coverage from the unit and integration suites only (cold run 1 min 26 s for the workspace, 96 tests): `main.rs`, `lib.rs`, `telemetry.rs` and `command/serve.rs` of `service` at 100% lines after the `?` form of rule 23.
- Command lines: `cargo deny --locked --offline check bans licenses sources` (0.9 s), `cargo shear --locked` (0.4 s), `cargo sqlx prepare --workspace --check -- --all-targets --all-features --locked` (2.5 s warm), `env -u CARGO_TARGET_DIR -u CARGO_BUILD_WARNINGS cargo mutants --workspace --cargo-arg=--locked` on one file (4 mutants caught, 12 s; `--locked` reached both phases).
- Manifests: the dependency set of this document resolves to 468 packages; `rust-version = "1.99.1"` is accepted; `cargo update --workspace --offline` makes `--locked` pass after manifest changes.
- Explorer assets: the four files exist in the npm packages at the pinned versions; sizes and `sha384` digests computed; the style sheet references only `data:` URLs; the script starts no worker.

Not run, to be confirmed during implementation:

- The complete `xtask` (the validation used a reduced checker): `arch::ambiguous`, `arch::lints`, the configuration guard with JSON values and override names, the scaffolder with its lock-file step inside the command.
- The explorer page in a browser (asset loading under the policy, SRI with `crossorigin`).
- The GraphQL route with the JSON-object extractor and the signed page parameters (compiled pieces exist; the combination was not built), and Schemathesis against the signed parameters.
- A full coverage run at 100% lines and functions and a full mutation run with 0 survivors: the skeleton has 96 tests where the first release needs several hundred.
- The recorded seeds with `TestRng::from_env` failing when the variable is absent (the `[env]` precedence itself was verified).
- Anything on Linux aarch64, macOS or WSL 2 (scope 8.11: only Linux x86_64 is claimed).

Readings of requirement wording recorded for the run summary:

- **NFR31, forced panic.** The built binary has no code path that panics, and a switch that forces one would be a back door (scope principle 5). The verification sentence of NFR31 is met by two tests of the integration suite (G12): the request sequence against the built binary (`serve.rs`) and the forced panic in process with the production telemetry on a captured writer (`panic.rs`).
- **NFR31, clap output.** A usage error and `--help` are printed by clap as text before any command runs; they are not counted as lines the serve command writes.
- **FR54, PostgreSQL probe.** `PostgresProbe` holds a concrete pool and has no unit test with a double; its success and unreachable cases are integration tests, and failure and timeout handling is unit-tested in `HealthService` (AD-11).
- **FR24(e), page value outside the `Int` range.** A value that does not fit GraphQL `Int` is an invalid document (FR24(f)), not a bounds violation.

PRD amendment made in this round (smallest change, ids stable): FR125(b) and NFR13 exempt the PostgreSQL image of the compose stacks from the digest pin, because a digest overrides the tag (`postgres:99-alpine@sha256:<digest of 16-alpine>` runs PostgreSQL 16.15), so one reference cannot carry both a digest and a working `DB_VERSION` (FR90).

### Coverage of requirements

- All 21 catalog areas and FR1 to FR126, NFR1 to NFR47 and G1 to G31 are mapped above; requirements owned by the delivery document point to its section and name the inputs they need from this document.
- Every item of scope 3.1 "Service runtime", "Example bounded context", "Shared kernel", "Persistence, cache, messaging" and "Architecture" has a crate, a file and a test location. No non-goal of scope 3.2 appears in the design, and the elements the validation found to be asked for by no requirement are removed.
- No dead scaffolding: every port has an adapter and a caller; every shared-kernel item is used by the example slice or the health check; every configuration file is read by a command in the gate table; no production function exists only for tests.

### Implementation readiness

- Names are final for: 13 crates, 2 binaries, 7 service commands, 3 `xtask` commands, 32 environment variables, 20 gate commands (QG-01 to QG-20), 8 architecture rules, 5 feature files with 24 scenarios, 12 log messages, the error table and the protected list. Implementation rules are numbered 1 to 23 with 3a, 6a and 17a.
- The signatures of "Domain and Application Design" are those of the skeleton that passed QG-01 to QG-04, with the listed changes of this revision; of those, the forced shutdown, the telemetry writer, the JSON configuration errors, the suppressions and the test-target layout were compiled and run, and the bus error with several failures, the signed page parameters, the GraphQL body check and the explorer page were not.
- First version of this document, verified on rustc 1.99.1 (`spikes/architecture-check`): state-generic `OpenApiRouter` with `FromRef` sub-states across crates; `routes!` in a context crate nested under `/api`; `#[schema(as = ExampleApiResource)]`; merged GraphQL roots and default arguments; custom `/api/docs` page with a page-specific policy next to `if_not_present` header layers; cargo's rejection of glob members that match non-crate directories; a package with only a `harness = false` test target.

### Risks

| Risk | Effect | Mitigation |
| --- | --- | --- |
| The full mutation run was extrapolated from one shard (5.2 s per mutant) | 60 to 80 min unsharded; the bootstrap pull request mutates the whole repository | One shard per 150 mutants, at most 8, of about 15 min each in CI (delivery DD-09); diff-scoped runs afterwards; a timeout is a failure, never raised silently (`minimum_test_timeout` is in a protected file) |
| 100% lines under the tool blind spots (`#[async_trait]` bodies, multi-line events, `)?);` lines, unreachable error branches) | A design that is correct but cannot reach the floor, or reaches it vacuously | Rules 17a, 21 and 23; the Bats plants for G3 and G14 sit in plain functions |
| Coverage of the binaries depends on forwarding `LLVM_PROFILE_FILE` and on ending every child through its normal exit path | Coverage of `main.rs` and the commands drops to 0% | One `support.rs` helper per crate does both; rule in "Test Architecture" |
| The explorer page was assembled from package contents, not opened in a browser | A directive of its policy is missing or unneeded | Development-only page; the implementation opens it once and corrects constant and document together; the pin test stays |
| Schemathesis ran against the first form of the page parameters | A conformance failure on the signed parameters | Run early in the example-context story; the rule is fixed (400 for a non-integer, 422 for an out-of-bounds integer) |
| The complete `xtask` (guard with JSON values and override names, scaffolder with the lock step) was designed, not built | Rework inside `xtask` | The reduced checker and every mechanism it relies on (`cargo metadata`, `git`, `cargo update --workspace --offline`, `serde_json` pointers) were verified separately; `xtask` is the first implementation step |
| Two end-to-end runs on one database server at the same time drop each other's scenario databases | Spurious failures | Documented constraint; CI jobs and the isolated stacks have their own servers |
| `pedantic` and `nursery` gain lints with each toolchain | A toolchain bump needs code changes | The toolchain is pinned and protected; bumps are deliberate pull requests |
| A leaked secret through `Debug` of `Config` or `PostgresSettings` | The database password in a log | Rule 13; the process-level tests assert that no output contains the password |
| async-graphql 8, axum 0.9, sqlx 0.10 will bring breaking changes | Upgrade work in every downstream service | Adapters are isolated per crate; contract snapshots and gates catch regressions |
| `axum::serve` has no header-read timeout or connection limit | Exposure when run without a proxy; an unfinished request holds the drain until the grace period ends | Documented deployment assumption (edge in front); the forced exit is bounded and observable |

Closed since the first version: `ListQueues` with `max_results(1)` runs against LocalStack 3.4.0; the `pattern` constraint passes Schemathesis; `#[schema(value_type = String)]` compiles; scenarios no longer share a database; the explorer has a policy that the PRD accepts.

### Intentionally deferred

Tracing and metrics exporters; Redis cache adapter; unit of work, outbox and idempotency keys; asynchronous consumer; authentication and rate limiting; CORS; update and delete operations; stored timestamps and a time crate; cursor pagination and total counts; vendored GraphiQL assets; GraphQL depth and complexity limits; HTTP/2; a `migrate status` command; a gate for top-level module names; publishing API documentation outside the service. Each has a named seam in "Decision Priority Analysis" or a rule in "Data Architecture", or is listed among the rejected alternatives of its decision.

### Completion

- **Status:** revised after the validation round; ready for the revision of the delivery architecture, then epics and implementation.
- **First implementation step:** create the workspace skeleton and tool configuration from "Workspace and Project Structure" and "Quality Gates", then `xtask arch`, so that every later story lands under the gates.
- **Order of authority for implementers:** `scope-decisions.md`; then the final PRD (`prd.md`: FR1 to FR126, NFR1 to NFR47, G1 to G31, PD-1 to PD-31; scope-decisions 8.11); then this document and `architecture-delivery.md` for names and mechanisms; then the research reports.
