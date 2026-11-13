---
stepsCompleted: [1, 2, 3, 4, 5, 6]
inputDocuments:
  - _bmad-output/planning-artifacts/scope-decisions.md
  - _bmad-output/planning-artifacts/research/capability-catalog.md
  - _bmad-output/planning-artifacts/research/php-app-code.md
  - _bmad-output/planning-artifacts/research/php-tests-and-quality.md
  - _bmad-output/planning-artifacts/research/php-docker-make-load.md
  - _bmad-output/planning-artifacts/research/php-ci-github-contracts.md
  - _bmad-output/planning-artifacts/research/php-ai-docs-audit.md
  - _bmad-output/planning-artifacts/research/rust-http-api-stack.md
  - _bmad-output/planning-artifacts/research/rust-data-cache-messaging.md
  - _bmad-output/planning-artifacts/research/rust-quality-gates.md
date: 2026-11-10
author: BMad
---

# Product Brief: rust-service-template

Produced by the BMAD `create-brief` workflow in an autonomous run. Stakeholder answers come from `scope-decisions.md` (binding) and the research files listed above; where neither answers a question, the answer is recorded under "Assumptions". Ids used below (`CAP-*`, `NEW-*`, `P1`-`P28`, defect references such as `APP:D-24` or `TQ:2`) are defined in `research/capability-catalog.md`. `research/rust-testing-delivery.md` did not exist when this brief was written.

## Executive Summary

`rust-service-template` is the repository a VilnaCRM team copies to start a Rust backend service. It is the Rust counterpart of `VilnaCRM-Org/php-service-template`: a running HTTP service with REST, GraphQL, health and liveness endpoints, a DDD + CQRS + hexagonal layout with one real example slice, PostgreSQL, a cache port and an SQS client, and around it the platform surface a service needs before its first feature: containers, a Makefile command surface, quality gates, contract checks, load tests, CI workflows, release and template-sync automation, and guidance for AI coding assistants.

The product exists for two reasons. First, a team that starts from an empty cargo project has to rebuild that surface itself: the PHP template's equivalent is 196 catalogued capabilities, 68 Make targets, 20 workflows and 50 command-line tests. Second, the PHP template shows what goes wrong when such a surface grows without verification: 188 recorded defects (150 after deduplication) and 28 audit problems, including a `make ci` that cannot pass, several gates that cannot fail, and assistant guidance that contradicts itself. A greenfield template can avoid most of these by construction.

The first release delivers parity of capability and intent with the PHP template, in idiomatic Rust, with the language-neutral contracts kept identical (URL paths, status codes, snapshot paths, capability-named Make targets, the K6 load kit, the commit convention, release and sync flows). Success is judged from the repository and its CI: a clean clone reaches a green `make ci` with no manual step, every gate fails on a planted violation, every PHP capability id is delivered, replaced or listed as a non-goal, and contract snapshots are byte-stable.

---

## Core Vision

### Problem Statement

A VilnaCRM backend engineer who starts a Rust service today starts from `cargo new`. The PHP template gives a PHP service the following on day one; a Rust service has none of it:

| Area (catalog areas) | What has to be rebuilt without the template |
| --- | --- |
| Runtime and API (1, 2, 13) | HTTP service under `/api`, problem-document errors, `GET /api/health` and `GET /ping`, OpenAPI and GraphQL documents generated from code and exported to committed snapshots |
| Domain building blocks (3, 4, 5, 6) | Aggregate root, domain events, command/query/event buses, repository port and PostgreSQL adapter, migrations, cache port, SQS client, enforced layer boundaries |
| Verification (7 to 11) | Formatting, static analysis, complexity, dependency and secret checks, unit, integration and Gherkin end-to-end tests, coverage, mutation testing |
| Command surface (12) | One Make target per gate and per maintenance action, an aggregate `make ci`, and tests of the Makefile itself |
| Contract and load checks (13, 14) | OpenAPI lint, breaking-change checks for both contracts, Schemathesis, K6 profiles (smoke, average, stress, spike) |
| Delivery (15 to 18) | Development and production images, compose stacks, CI workflows, release automation, Dependabot, template sync, commit convention |
| Guidance and docs (19 to 21) | `AGENTS.md`, `CLAUDE.md`, task skills, README, CONTRIBUTING, SECURITY, architecture model |

The PHP template is also not a safe thing to copy. Its intent is sound; its execution has defects that a line-by-line port would reproduce:

| What went wrong in PHP | Evidence (catalog defect digest) |
| --- | --- |
| Gates that cannot fail | `make ci` calls a tool that is not installed and no workflow runs it (TQ:1, DM:D-01); the OpenAPI diff has no fail flag (CI:D-01); mutation testing has no minimum score (TQ:2); no coverage floor exists in the repository (TQ:3); the formatter always runs in fix mode (TQ:4); static analysis runs at its most lenient level (TQ:6); the SARIF upload is skipped exactly when findings exist (CI:D-04) |
| An architecture guard weaker than it looks | 14 of 32 classes belong to no layer; rules hard-code the placeholder names in a locked file; Application and Infrastructure may depend on each other (APP:D-24, D-25, D-26) |
| Building blocks that do not work | The command-bus adapter does not exist; the event bus keeps one subscriber per event and treats zero subscribers as an error; the UUID value object validates nothing (APP:D-01, D-03, D-04, D-08) |
| A health check that proves little | The three failure messages exist only in test mocks; probes run one after another with no timeout; the broker probe creates a queue on every request; the cache probe reads a key that never expires (APP:D-12 to D-15) |
| An accidental API contract | The GraphQL schema exposes Relay and CRUD operations no code implements, yet is diffed as a contract; the OpenAPI snapshot can drift from the code (APP:D-19, CI:D-10) |
| Costly and permissive CI | No caching, concurrency control or timeouts, 15 uncached development-image builds per push; 12 of 20 workflows declare no `permissions`; mutable action tags where write tokens are present; bots commit to pull-request branches and to `main` (CI:D-16, D-20, D-19, D-09, D-11) |
| Not safe to synchronise downstream | Container and image names hard-coded to the template; the sync workflow has a file-name typo, no ignore list and runs in the template against itself; the release workflow fails on every push when its secrets are absent (CI:D-06, D-13, D-14) |
| Hygiene | An env file with secret-shaped values is tracked although git-ignored; the manifest says `proprietary` while the licence is CC0; generated artefacts are not ignored (APP:D-35, APP:D-33, TQ:40) |
| Load tests that can damage data and cannot detect a regression | The load stack shares project, volumes and ports with the development stack and drops its database; the only enforced budget is smoke p99 below 9000 ms; results are not uploaded (DM:D-03, DM:D-25, DM:D-40) |
| Guidance that is not true | Five diverging verification command lists, none equal to `make ci` or hosted CI; the protected-configuration guard runs in no workflow; skills cite a directory that does not exist and patterns with no exemplar; no assistant settings file; README promises that are false; `SECURITY.md` says `TBD` (AD:D-AD-04, D-AD-01, D-AD-11, D-AD-12, D-AD-18, D-AD-19, D-AD-26) |

The PHP enterprise audit (2026-07-02) confirms the pattern: 94 quality attributes averaged 3.48 out of 5, none scored 5, and the weakest categories were reliability and resilience (2.69), observability and operability (3.10) and security and trust (3.12). The gaps were reduced to 28 root-cause problems, P1 to P28.

### Problem Impact

- **Backend engineers** repeat platform work that has no business value, and each service ends up with different target names, ports and contracts. Shared tooling (load tests, Schemathesis, Bats, CI wait loops) depends on port `8081`, `/ping` and fixed snapshot paths, and stops working when those differ.
- **Platform maintainers** cannot propagate a fix to Rust services, because there is nothing to synchronise from, and cannot reuse PHP-side tooling if the Rust side invents its own contracts.
- **Reviewers and tech leads** cannot trust a green check. In the PHP template the OpenAPI compatibility job, the mutation job and the code-style job cannot fail, undefined Gherkin steps pass, and the protected-configuration guard runs in no workflow.
- **AI coding assistants** receive instructions that disagree with each other and with CI, are told to follow patterns that exist nowhere in the code, and are not mechanically prevented from weakening a gate to obtain a pass.

### Why Existing Solutions Fall Short

| Option | What it provides | Where it falls short |
| --- | --- | --- |
| Empty cargo project | Compiler, formatter, linter and test runner | None of the 196 catalogued capabilities; every team makes its own choices, so services diverge |
| Generic community Rust template | Usually a web-framework skeleton with format, lint and test in CI (see assumption A-8) | Does not carry VilnaCRM's contracts: `/api` prefix, `204`/`500` health contract with its three messages, `/ping`, port `8081`, snapshot paths under `.github/`, the Spectral ruleset, the K6 kit, `type(#issue): subject` commits, the release and template-sync flows, the skills layout. Retrofitting them is the same work as building the template |
| File-by-file port of the PHP template | Familiar structure | Reproduces the defects above. The container and Makefile research states that copying files and renaming `php` "will reproduce most of them" |

### Proposed Solution

A template repository, `VilnaCRM-Org/rust-service-template`, built to seven rules taken from `scope-decisions.md` section 2:

1. **Parity first.** Each of the 196 PHP capability ids is delivered, replaced by a named Rust equivalent, or listed as a non-goal with a reason.
2. **Stable cross-template contracts.** Language-neutral contracts are identical in both templates.
3. **Known defects are not replicated.**
4. **Every gate is real.** Each gate is a Make target and a CI check, has a threshold, and is shown to fail.
5. **No dead scaffolding.** Everything shipped is used by the example slice, the health check or a command.
6. **Verified, not assumed.** Crate and tool versions come from research spikes run on rustc 1.97.
7. **Lean scope.** What is not listed as in scope is out of scope for this release.

The deliverable has eight parts, detailed under "MVP Scope": service runtime; a real example bounded context; shared kernel; persistence, cache and messaging; enforced architecture; quality gates; delivery tooling; AI guidance and documentation.

### Key Differentiators

1. **Contract compatibility with the PHP template.** The reused `features/health_check.feature`, the PHP `.spectral.yaml`, the Schemathesis flags, the K6 kit, the capability-named Make targets and the `approved-breaking-change` and `template_sync` labels are kept unchanged. The research ran the PHP Spectral ruleset, `openapi-diff` and Schemathesis with the PHP flags against Rust-generated output; all three worked without changes.
2. **Gates with negative tests.** Each gate has an automated test that plants a violation and asserts failure. Mutation testing fails on any surviving mutant; coverage has an in-repository floor.
3. **Structural architecture enforcement.** Rules follow layer position, so a new bounded context is covered without editing a protected rule file, no code can be unassigned, and one context cannot reach into another except through the shared kernel.
4. **Working exemplars.** Every pattern the guidance names (command path, query path, event path, repository adapter, migration, validation) exists as tested code in `CompanySubdomain/SomeModule`.
5. **Safe to synchronise.** Image, container and compose names derive from one project-name variable; the sync workflows have an ignore list and do nothing in the template repository itself; workflows skip cleanly when organisation secrets are absent.
6. **Guidance that is checked.** One verification list equal to `make ci`, a documentation-drift check for cited commands, a protected-configuration guard that runs in CI, and a committed assistant settings file with an allowlist.
7. **Traceable origin.** Requirements trace to a capability catalog (196 ids, 42 additions), a defect digest (150 lines) and the audit problem list (28 problems), each with a recorded disposition.

---

## Target Users

### Primary Users

**Olena, backend engineer.** She starts a new Rust service or maintains one created from the template. She knows Rust and the VilnaCRM PHP services; she is not a platform specialist.

- Jobs to be done: get a running, tested service on the first day; add a bounded context without studying the build system; change an API and know exactly which artefacts must change with it; find out why a check failed and reproduce it locally with the same command.
- Today: assembles tooling by hand or copies fragments from another service; discovers conventions from failing CI.
- Success for her: `make help` lists every command; an unmodified clone passes `make ci`; the example slice shows where each kind of code goes; a failing gate names the rule and the file.

**The AI coding assistant (Claude Code, Codex and similar).** It receives a task in a repository created from the template and works with repository files and shell commands only.

- Jobs to be done: find the instructions that apply to the task; run the project's commands without permission prompts for safe operations; copy an existing pattern instead of inventing one; know which commands prove the work is complete; report what was not run.
- Today (PHP template): five verification lists that differ, skills that describe what they are but not when to use them, patterns with no exemplar, and nothing that blocks an edit to a quality configuration file.
- Success for it: one entry file, one verification list, an allowlist for `make` targets, a guard that rejects edits to protected files, and commands in the guidance that are checked to exist.

### Secondary Users

**Taras, platform and DevOps maintainer.** He owns both templates and the CI conventions of the organisation.

- Jobs to be done: fix or improve something once and deliver it to every Rust service through template sync; keep PHP and Rust services operable with the same tooling; keep CI cost bounded; update toolchain and dependencies on a schedule.
- Success for him: sync pull requests arrive labelled and never overwrite service-owned files; no workflow depends on the repository name; scheduled checks on `main` report new advisories; CI uses caches, cancellation and timeouts.

**Iryna, reviewer and tech lead.** She approves pull requests written by engineers and by assistants.

- Jobs to be done: rely on status checks instead of re-verifying by hand; see contract changes as reviewable diffs; make an intentional breaking change an explicit, recorded decision; detect an attempt to weaken a gate.
- Success for her: each gate is a separately named status check; committed snapshots change in the same pull request as the code; a breaking change fails unless the `approved-breaking-change` label is present; an edit to protected configuration fails the guard in CI.

### User Journey

Names of targets that `scope-decisions.md` does not fix (formatting, lint, architecture, scaffolding) are the architect's decision and are described here by capability.

**J1. First hour with the template (Olena).**

1. Creates a repository from the template and clones it.
2. Runs `make doctor`; it names each missing host prerequisite.
3. Runs `make bootstrap`; it prepares the clone and ends with the stack healthy.
4. Checks `GET http://localhost:8081/ping` (`200`, `pong`), `GET /api/health` (`204`), `/api/docs`, `/api/graphql`, and the architecture diagrams in Structurizr Lite.
5. Runs `make ci`; it ends with a success line.
6. Follows the "create a service from the template" guide: sets the project-name variable and edits no workflow.

Value moment: a green `make ci` on an unmodified clone.

**J2. Adding a bounded context (Olena).**

1. Runs the scaffolding command with a context and a module name.
2. The generated module has Domain, Application and Infrastructure, compiles, and passes the architecture gate; no protected file was edited.
3. Copies the command, query and event paths from `CompanySubdomain/SomeModule`, adds a reversible migration and applies it with the `migrate` command.
4. A dependency from her Domain code on a framework, or on another context's internals, fails the architecture gate with a message naming both ends.
5. Updates `workspace.dsl`; CI validates the model.

**J3. Changing an API (Olena, reviewed by Iryna).**

1. Changes the handler and its types in code.
2. Runs `make generate-openapi-spec` and `make generate-graphql-spec` and commits the snapshots with the code.
3. If she forgets, CI fails because the committed snapshot differs from the generated one.
4. CI lints the OpenAPI document with the Spectral ruleset, runs Schemathesis against the production image, and compares both contracts with `main`.
5. A breaking change fails until Iryna adds the `approved-breaking-change` label; the label is the record of the decision.

**J4. Fixing a failing gate (Olena or the assistant).**

1. The failed status check identifies one gate.
2. She runs the Make target of that gate locally; it is the command CI ran.
3. The troubleshooting guide and the matching skill explain how to fix the code. Weakening the gate is not an available fix: the protected-configuration guard fails in CI when a protected file is modified outside the sanctioned change path.
4. For a failing test with random data, the printed seed reproduces the failure.

**J5. Receiving a template-sync pull request (Taras and a service owner).**

1. Taras merges an improvement into the template.
2. In a downstream repository the scheduled sync workflow opens a pull request labelled `template_sync`. Files on the sync ignore list are untouched.
3. The pull request runs the same gates as any other change and needs no workflow edit, because nothing depends on the template's name.
4. The service owner reviews and merges. In the template repository itself the sync workflows do nothing.

**J6. An AI assistant taking a task.**

1. Reads `AGENTS.md`; `CLAUDE.md` delegates to it. Chooses a skill through the decision guide; each skill description states when to use it.
2. Finds the exemplar for the required pattern in the example slice.
3. Works through `make` targets; the committed settings file allows them without prompts and blocks edits to protected files.
4. Runs the canonical verification list, which equals `make ci`.
5. Closes with what was run, what was not run and why.

---

## Success Metrics

Every metric below is observable from the repository or from its GitHub Actions history. No metric depends on surveys or on usage data from outside the repository.

User success, per user: Olena reaches a green `make ci` from a clean clone without help (K-01, K-02) and adds a context without touching protected files (K-10). The assistant finds only true commands in the guidance (K-12). Taras receives sync-safe output and bounded CI cost (K-11, K-13, K-14, K-16). Iryna can rely on every status check (K-03, K-04, K-07, K-09).

### Business Objectives

- **BO-1 Parity.** A Rust service at VilnaCRM starts with the same platform capabilities as a PHP service (K-05, K-08).
- **BO-2 Same-day start.** A new service goes from clone to a verified baseline in one working session, with no undocumented step (K-01, K-02).
- **BO-3 One platform, two languages.** Language-neutral contracts are identical in both templates, so one team maintains both and shared tooling works on either (K-08, K-13).
- **BO-4 Trustworthy automation.** A green pipeline means the change meets every stated rule, for human and assistant authors alike (K-03, K-04, K-09, K-12).
- **BO-5 Audit problems closed from the first release.** Of P1 to P28, 16 are in scope, 9 are partly in scope with the remainder recorded as follow-up or non-goal, and 3 are declared non-goals (P3, P7, P20). The counts are this brief's reading of `scope-decisions.md` section 6, which holds the binding text (K-05, K-06).

### Key Performance Indicators

Time budgets are initial values derived from the research spike timings on a 4 vCPU machine. The architect confirms or revises them with measurements; a revision needs a recorded reason.

| Id | Indicator | How it is observed | Target |
| --- | --- | --- | --- |
| K-01 | Clean clone to green | CI job on a hosted runner with no restored cache: clone, `make bootstrap`, `make ci` | Exit 0 on 100% of scheduled runs; zero manual steps between clone and green |
| K-02 | Time to a verified baseline | Duration of the K-01 job steps | `make bootstrap` to `GET /ping` = `200`: at most 15 min cold. `make ci` after that: at most 45 min cold. `make start` with images present: at most 60 s |
| K-03 | Every gate can fail | Automated negative tests: each plants one violation and asserts a non-zero exit from that gate | 100% of gates in the gate list have at least one negative test; 0 gates without one |
| K-04 | Local and hosted CI agree | Comparison of the gates in `make ci` with the gates run by pull-request workflows | 0 unexplained differences; each CI-only gate is listed with its reason |
| K-05 | Parity coverage | Parity matrix tracked with the planning bundle: one row per capability id with status and pointer | 196 of 196 PHP ids mapped; 185 delivered (KEEP 81, FIX 95, EXTEND 9); 11 DROP ids listed with reason; 42 of 42 `NEW` ids delivered |
| K-06 | Defects not replicated | Readiness review against the 150-line digest | 0 replicated, apart from the 2 the catalog records as accepted by decision (`/ping` as container healthcheck; issue number in the changelog scope) |
| K-07 | Deterministic contract snapshots | CI generates both snapshots 20 times, including once after a clean rebuild, and compares bytes; then compares with the committed files | 20 of 20 identical; 0 bytes of difference from the committed snapshots on `main` |
| K-08 | Wire parity with PHP | Reused `features/health_check.feature`; PHP `.spectral.yaml` at `--fail-severity=hint`; Schemathesis `--checks all --phases=examples,coverage` against the production image | 4 of 4 scenarios pass with unchanged step phrases; 0 lint findings; 0 failed checks |
| K-09 | Test strength | Coverage and mutation gates | 100% line coverage of testable code; 0 surviving mutants; 0 undefined or pending Gherkin steps |
| K-10 | Architecture | Architecture gate; a test that runs the scaffolding command and then the gate | 0 violations; 0 unassigned code units; 0 protected files edited when a context is added |
| K-11 | CI time | Durations in GitHub Actions | Pull request, warm caches: at most 20 min from push to last required check. Cold caches: at most 40 min. Documentation-only pull request: at most 5 min. Every job declares a timeout of at most 45 min |
| K-12 | Guidance is true | Documentation-drift check over `AGENTS.md`, `CLAUDE.md`, `.claude/skills/`, README and CONTRIBUTING | 0 cited commands that do not exist; exactly 1 canonical verification list, equal to `make ci` |
| K-13 | Sync safety | Search for the literal repository name in workflows, Makefile, compose files, scripts and skills; a test that runs the stack and the Makefile tests under a different project name | 0 occurrences outside the project-name definition and the sync-source setting; the renamed run passes; 0 sync pull requests opened in the template repository |
| K-14 | Clean degradation without secrets | Actions history on `main` with `VILNACRM_APP_ID`, `VILNACRM_APP_PRIVATE_KEY`, `PERSONAL_ACCESS_TOKEN` and `CODECOV_TOKEN` absent | 0 failed runs caused by a missing secret; each affected step is skipped with a notice |
| K-15 | No dead scaffolding | Unused-dependency gate; review of configuration files against the commands that read them | 0 unused dependencies; 0 configuration files no command uses; 0 tests without an assertion |
| K-16 | Workflow hygiene | Workflow lint and review | 100% of third-party actions pinned by commit SHA; 100% of workflows declare `permissions`; 0 bot commits other than the release identity |
| K-17 | Security gates | Advisory, licence, source, secrets and image checks | 0 advisories, 0 licence or source violations, 0 secret findings; image findings below the failing severity the architect sets |
| K-18 | Smoke load on pull requests | K6 smoke run against the production image; uploaded artifacts | Check success rate above 99%; p99 below the budget the architect sets, proposed at most 500 ms (PHP: 9000 ms); results uploaded on 100% of runs |

Indicators tracked by the maintainers after release, not part of acceptance: number of repositories created from the template; age of open `template_sync` pull requests downstream; number of guard failures caused by edits to protected configuration.

---

## MVP Scope

### Core Features

The list restates `scope-decisions.md` section 3.1; that document prevails on any difference.

**F1. Service runtime**

- One service binary serving plain HTTP on one port, default published port `8081`. No TLS, HTTP/3, compression or reverse proxy in the template.
- REST under `/api` with a code-first OpenAPI 3.1 document and a docs UI at `/api/docs`. GraphQL at `/api/graphql` with only intentionally implemented operations; the interactive explorer is enabled in the development environment only.
- Every error response is `application/problem+json` (RFC 9457): unknown routes, wrong methods, unsupported media types, validation failures, timeouts and panics.
- `GET /api/health`: `204` with an empty body when database, cache and message broker respond; otherwise a `500` problem response that reports every failing dependency with exactly `Database is not available`, `Cache is not working`, `Message broker is not available`. Probes run concurrently, each under a timeout; the broker probe is non-mutating.
- `GET /ping`: `200`, body `pong`, no dependency access, not part of the OpenAPI document.
- Structured JSON logs; a request/correlation id accepted or generated for every request, echoed in the response and attached to every log line; security headers on every response; graceful shutdown on SIGTERM; an explicit timeout on every outbound call.
- Typed configuration from environment variables with fail-fast validation and a `check-config` style command.

**F2. Example bounded context `CompanySubdomain/SomeModule`**

- Create an `ExampleApiResource` through the command path: validated input, command bus, aggregate, domain event, repository port, PostgreSQL adapter, migration.
- Read one by id and list with a bounded page through the query path.
- One in-process domain-event subscriber that writes a structured audit log line.
- REST path `/api/example_api_resources`; GraphQL exposes the matching queries and the create mutation. No update or delete.

**F3. Shared kernel**

- Aggregate root with recorded domain events; domain event contract; command, query and event bus ports with in-process adapters (an event may have zero or many subscribers; a missing command or query handler is an error); validated UUID value object and generator. Everything shipped is used by F2 or by the health check.

**F4. Persistence, cache and messaging**

- PostgreSQL through a repository port and adapter; reversible SQL migrations; an explicit `migrate` command and a documented rollback command; default `DB_VERSION=16`.
- A cache port with an in-process adapter. Redis is a documented extension through the same port and is not shipped.
- AWS SQS client wiring against LocalStack, used by the broker probe. No asynchronous consumer and no outbox. LocalStack pinned to the tokenless community tag the research verified.

**F5. Architecture**

- `Shared`, `Internal/HealthCheck` and `CompanySubdomain/SomeModule`, each split into Domain, Application and Infrastructure; Domain is framework-free.
- A hard boundary gate with zero unassigned code, structural rules, and a rule that forbids one context from reaching into another except through the shared kernel.
- A scaffolding command that creates a bounded-context module which compiles and passes the gate, and a guide for creating a service from the template.
- `workspace.dsl` with context, container and component views that match the code, validated in CI; Structurizr Lite in the development stack.

**F6. Quality gates** (each a Make target and a CI check, each able to fail)

- Code: formatting check with a separate fix target; strict static analysis with warnings denied; complexity limits; architecture boundaries.
- Supply chain and secrets: dependency advisories, licences and sources; unused dependencies; secrets scanning; production-image vulnerability scan.
- Tests: unit; integration against real PostgreSQL and LocalStack; Gherkin end-to-end reusing the PHP health feature; coverage with an in-repository floor of 100% lines for testable code; mutation testing that fails on surviving mutants; Bats tests of the Makefile including negative tests; seeded random test data with the seed printed on failure.
- Contracts: committed snapshots at `.github/openapi-spec/spec.yaml` and `.github/graphql-spec/spec`, generated deterministically by CLI commands, with CI failing when committed and generated differ; OpenAPI lint with the PHP Spectral ruleset; OpenAPI breaking-change check and GraphQL schema diff, both failing unless the approval label is present; Schemathesis against the running service.
- Load: K6 smoke on pull requests.
- Repository: workflow, YAML, shell, Dockerfile and Markdown linting; lint of the load-test scripts (catalog decision 22); the protected-configuration guard running in CI.

**F7. Delivery**

- Multi-stage Dockerfile with a development target and a minimal, non-root, hardened production target. Compose files: base, development override, production, and an isolated load-test stack that cannot touch development data. The production compose reference has resource limits, a restart policy, a read-only root filesystem, dropped capabilities and no-new-privileges.
- A Makefile as the single command surface with `help` as the default goal; capability-named PHP targets keep their names, tool-named ones get capability names, and the mapping is documented; `doctor` and `bootstrap` targets. Targets work inside the development container, on a host with the pinned toolchain and on a GitHub runner, with an explicit, tested switch that honours `CI=true`.
- GitHub Actions with dependency and build caching, concurrency cancellation, timeouts, least-privilege permissions, third-party actions pinned by commit SHA, heavy jobs skipped for documentation-only changes, and a scheduled run of the security and CI checks on `main`.
- Conventional commits in the form `type(#issue): subject`, enforced by a local hook and by a pull-request title check (bots exempt).
- Automated release with CHANGELOG from conventional commits; Dependabot for cargo, GitHub Actions, Docker and the K6 npm package; both template-sync workflows (PAT and App) with the file-name typo fixed, a sync ignore list and a guard for the template repository; CodeRabbit configuration.
- K6 load-test kit ported from PHP (smoke, average, stress, spike; local stack and the AWS runner), results uploaded as CI artifacts, budgets suited to a compiled service, known AWS-flow defects fixed where the fix is local to the scripts. Contract, fuzz and load tests run against the production image.

**F8. AI guidance and documentation**

- `AGENTS.md`, `CLAUDE.md`, `.claude/skills/` with an index, a usage guide, a decision guide and a Rust counterpart for each of the ten PHP skills, named by capability; one canonical verification list equal to `make ci`; a documentation-drift check; a committed assistant settings file with an allowlist and a guard on protected files.
- BMAD and bmalph integration following the `core-service` convention: tracked `bmalph/config.json`, command wrappers, Make targets to install and initialise bmalph, the `bmad-autonomous-planning` skill, generated assets kept out of git, planning bundles tracked under `specs/`.
- README, CONTRIBUTING, a complete SECURITY policy, CHANGELOG, architecture overview with decision records, troubleshooting, environment-variable reference, Make-target reference, operations notes (migrations and rollback, health-probe runbook), `.editorconfig`, a dev container definition, backend-appropriate issue templates, a request collection for the endpoints. One licence statement (CC0-1.0) everywhere.

### Out of Scope for MVP

Identical to `scope-decisions.md` section 3.2.

| Item | Reason |
| --- | --- |
| Authentication and authorisation (audit P3) | The PHP template has none; needs an identity-provider decision |
| Rate limiting (part of P5) | Edge concern; interferes with load and fuzz gates; needs policy input |
| OpenTelemetry tracing and metrics exporters, Prometheus endpoint (part of P2) | Deferred; the logging and request-id baseline ships now and the seam is documented |
| Publishing a container image with SBOM, signature and provenance (P7) | Needs registry and identity decisions |
| Orchestrator manifests, autoscaling examples (part of P15) | Compose reference only |
| Outbox, idempotency keys, chaos tests, kill switches (part of P16) | Feature work beyond template parity |
| Internationalisation (P20) | Declared non-goal; no empty scaffolding ships |
| Asynchronous message consumer / worker | PHP has no working one |
| Redis adapter | Documented extension through the cache port |
| Mercure, Vulcain, HTML templating, sessions | Configured but unused in PHP |
| Reverse proxy, in-process TLS, HTTP/3 | Platform edge |
| Update/delete on the example resource | Exemplar stays small |
| CODEOWNERS, branch-protection rulesets, auto-merge, stale bot | Organisation decisions; documented as follow-ups |
| Multi-architecture image builds | Follow-up |
| Stored performance baselines and trend comparison (part of P19) | Needs a storage decision |

PHP mechanisms the capability catalog marks `DROP` under these rows or under the "no dead scaffolding" rule are also absent: the error-page preview route, the empty fixture loader and its target, the debugger call-back wiring, the bot that commits formatting fixes to `main`, and a new audit document.

### MVP Success Criteria

The release is complete when all of the following hold on the release commit on `main`:

1. K-01, K-03 to K-10 and K-12 to K-17 meet their targets.
2. K-02, K-11 and K-18 meet their budgets, or the architect has replaced a budget with a measured value and a recorded reason.
3. Every pull-request check and the scheduled run on `main` are green.
4. Each of the six journeys J1 to J6 can be followed from the repository's own documentation with no step that is missing or wrong.
5. The parity matrix (K-05) and the disposition of P1 to P28 are tracked in the repository.

Work on a deferred item starts only after the decision named for it under "Future Vision" exists.

### Future Vision

The template stays a baseline, not a framework. Later releases add the deferred items when their preconditions are met:

| Deferred item | Precondition |
| --- | --- |
| Tracing and metrics (OpenTelemetry exporters, Prometheus endpoint) | Choice of observability backend; the seam is already documented |
| Authentication and authorisation | Identity-provider and token-format decision |
| Rate limiting | Edge policy; a way to keep load and fuzz gates unaffected |
| Published container image with SBOM, signature and provenance | Registry and signing identity |
| Orchestrator manifests and autoscaling examples | Target platform decision |
| Asynchronous consumer, outbox, idempotency keys, chaos tests, kill switches | A first service that needs them, so the exemplar is real |
| Redis adapter | A service that needs a shared cache; the port already exists |
| CODEOWNERS, rulesets, auto-merge, stale bot, OSSF Scorecard | Organisation governance decisions |
| Multi-architecture images | Runner and registry support |
| Stored performance baselines and trend comparison | Storage location for results |

Beyond these, the two templates should converge on shared language-neutral files (Spectral ruleset, K6 kit, release and sync guides, portable skills), so that one change serves both.

---

## Constraints and Assumptions

### Constraints

- **Toolchain.** Pinned in `rust-toolchain.toml`. Stable channel only. Research verified versions on rustc 1.97.0.
- **Docker.** A container engine with the Compose plugin is required for the stack, integration tests, contract tools and load tests. Host prerequisites are checked by `make doctor`.
- **Three execution places.** Every Make target works in the development container, on a host with the pinned toolchain and on a GitHub runner.
- **CI platform.** GitHub Actions on GitHub-hosted runners. Pull requests from forks and from Dependabot run with a read-only token and no secrets; every gate must work under those conditions.
- **Organisation secrets may be absent.** `VILNACRM_APP_ID` and `VILNACRM_APP_PRIVATE_KEY` (release, App-flavour sync), `PERSONAL_ACCESS_TOKEN` (PAT-flavour sync) and `CODECOV_TOKEN` (coverage upload) may not exist when the repository is first used. The affected workflows must skip with a notice and must not fail. The CodeRabbit App may not be installed.
- **Naming.** Ecosystem-standard environment variable names where a tool requires them (`DATABASE_URL`, `AWS_REGION`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_ENDPOINT_URL_SQS`); PHP names for compose-level settings (`DB_VERSION`, `DB_NAME`, `DB_USER`, `DB_PASSWORD`, `DB_PORT`, `LOCALSTACK_PORT`, `STRUCTURIZR_PORT`, `APP_ENV`). Package identity `rust-service-template`, licence `CC0-1.0`. No secret-shaped value is committed.
- **No git submodules.** Bats helper libraries are provisioned another way.
- **Fixed emulator version.** LocalStack stays on a tokenless community tag; newer images require an account token.
- **Planning format.** Later artifacts (`prd.md`, `architecture.md`, `epics.md`, `implementation-readiness.md`) follow the bmalph rules in `scope-decisions.md` section 7.
- **Second ecosystem.** The K6 kit keeps JavaScript files and one package manifest in the repository; Dependabot and a lint gate cover them.

### Assumptions

- **A-1.** "Parity" means parity of capability and intent. Wire and tooling contracts marked "preserved" in the capability catalog are identical; PHP mechanisms and PHP defects are not parity targets.
- **A-2.** The capability catalog's dispositions, including the 26 dispositions taken where scope is silent, stand unless the PRD or the architect records a change.
- **A-3.** Maintainers mark the pull-request checks as required in the organisation's settings. Branch-protection rulesets are out of scope, so the template cannot enforce this itself.
- **A-4.** "Testable code" in the coverage floor means all production source, with no exclusions beyond what the architect records one by one. The research reached 100% lines with no exclusion flag.
- **A-5.** Time budgets (K-02, K-11) and the smoke budget (K-18) are initial proposals from spike timings, not measurements of the final service.
- **A-6.** Each template-sync flavour runs only when its own credentials exist, so a downstream repository does not receive two overlapping sync pull requests.
- **A-7.** The parity matrix and the P1 to P28 disposition are tracked with the planning bundle under `specs/`.
- **A-8.** No survey of community Rust templates was run. The comparison in "Why Existing Solutions Fall Short" rests on what such a template would need in order to meet VilnaCRM's contracts, not on a review of named projects.
- **A-9.** The research spikes show the scope is feasible on the stable toolchain: every PHP gate has an equivalent that was run passing and failing, contract exports were byte-identical over 20 runs, and the PHP contract tools accepted the Rust output. The choice of crates and tools remains the architect's.
- **A-10.** Persona names are illustrative; the four user groups are those named in the task.

### Risks

| Risk | Effect | Mitigation |
| --- | --- | --- |
| Scope is large for a first release (196 PHP ids plus 42 additions) | Late or partial delivery | Traceable catalog; epics follow the eight feature groups; anything not listed is refused |
| 100% line coverage and zero surviving mutants | Slow pipelines; pressure to add exclusions | Mutation scope and cadence decided by the architect within K-11; exclusions recorded one by one |
| LocalStack community tags are frozen | No fixes; future incompatibility | Pinned tag; an alternative SQS emulator was verified by the research as a contingency |
| The AWS load-test runner cannot be exercised without AWS credentials | Fixes verified only against the LocalStack dry run | Dry run covered by a Bats test; the limitation stated in the load-testing guide |
| Docker Hub rate limits (observed once during research) | Intermittent CI failures | Image pinning and caching; registry authentication if the organisation provides it |
| Testing and delivery research not yet available | Gherkin runner, Bats provisioning, image scan and K6 budgets undecided | The architect decides with that report or with a spike |
| Toolchain bumps are not covered by Dependabot | The pinned toolchain goes stale | Ownership and cadence recorded in the operations notes |

### Open Questions for the PRD and Architecture Stages

1. Which gates run inside `make ci` and which are CI-only, and does `make ci` run mutation testing on the whole code base or on the diff?
2. What is the protected-file list, and what is the sanctioned way to change a protected file (K-10, J4)?
3. Which files are on the sync ignore list, and are the tracked BMAD and bmalph files synchronised downstream?
4. Which numeric limits apply: complexity, list page bound, probe and outbound timeouts, K6 budgets per profile, image-scan failing severity?
5. Is the health failure detail the same in every environment?
6. How is `info.version` sourced so that a release does not break the snapshot gate?
7. Does the coverage upload to the external service stay as a non-gating step?
8. How many releases does the changelog retain?

The next BMAD step is `create-prd`: the feature groups F1 to F8 become functional requirements, K-01 to K-18 become non-functional requirements and acceptance criteria, and the out-of-scope table becomes the scope boundary.
