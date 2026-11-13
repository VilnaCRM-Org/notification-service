---
stepsCompleted: [1, 2, 3, 4, 5, 6]
inputDocuments:
  - scope-decisions.md
  - research/capability-catalog.md
  - product-brief.md
  - research/php-app-code.md
  - research/php-tests-and-quality.md
  - research/php-docker-make-load.md
  - research/php-ci-github-contracts.md
  - research/php-ai-docs-audit.md
  - research/rust-http-api-stack.md
  - research/rust-data-cache-messaging.md
  - research/rust-quality-gates.md
  - research/rust-testing-delivery.md
workflowType: 'research'
lastStep: 6
research_type: 'technical'
research_topic: 'Parity of rust-service-template with php-service-template: PHP capability inventory, enterprise audit, verified Rust stack'
research_goals: 'Establish what the PHP template really provides, which of its defects must not be copied, and which Rust crates and tools deliver each capability, with every version verified by running where possible'
user_name: 'BMad'
date: '2026-11-10'
web_research_enabled: true
source_verification: true
---

# Research Report: technical

**Date:** 2026-11-10
**Author:** BMad (BMAD `analyst` / `technical-research`, autonomous run)
**Research Type:** technical research digest for `VilnaCRM-Org/rust-service-template`

---

## Research Overview

This file is the tracked digest of the analysis phase. Nine raw reports (five PHP inventories and four Rust technical reports, 9,295 lines in total) stay local and untracked; a reader who has only this file should be able to see what was studied, what was found and what was decided.

Three results carry the rest of the planning:

1. **The PHP template is small in code and large in tooling.** It has 32 PHP files (818 lines) and two real REST operations, surrounded by 68 Make targets, 20 workflows, 50 Bats tests and ten AI-assistant skills. The inventories record 196 capabilities and 188 defects. Several of its gates cannot fail.
2. **Every capability that is carried over has a Rust or language-neutral counterpart on the stable toolchain.** Four spikes were compiled and run on rustc 1.97.0 with Docker. Every gate in the quality-gates spike was observed both passing and failing. The PHP Gherkin health feature, the PHP Spectral ruleset and the PHP K6 scripts ran unchanged against Rust output.
3. **The disposition is fixed.** Of the 196 PHP capability ids, 81 are kept, 95 are kept with a named defect fixed, 9 are extended and 11 are dropped; 42 capabilities are added. Of the 28 audit problems, 3 are non-goals and 25 are in scope in full or in part.

`scope-decisions.md` is binding. Where it overrides a research recommendation, section 4.5 shows both. `prd.md` did not exist when this digest was written; `scope-decisions.md` and the capability catalog were used as the authority.

---

## 1. Purpose, method and evidence base

### 1.1 Purpose

The request was to set up the Rust-ecosystem counterpart of `VilnaCRM-Org/php-service-template` with BMAD and bmalph. "The same template" is read as parity of capability and intent in idiomatic Rust, without copying PHP mechanisms or PHP defects (`scope-decisions.md` section 1). The research therefore had to answer three questions: what the PHP template really does, which parts are defects, and which Rust mechanisms deliver each capability.

### 1.2 What was read

The subject is `php-service-template` at commit `3c17721` (`chore(release): v1.11.0 [skip ci]`, 2026-04-12), in a read-only clone. Every file in scope was read in full. Nothing was built or run inside the PHP repository; three behaviours were checked on throwaway copies outside it (`make -n` and `make help` on a copy of the Makefile, GNU Make's export behaviour for `include .env.test`, and the `grep -E` behaviour of the Schemathesis localhost pattern). Statements that depend on runtime semantics are marked "(inferred)" in the inventories.

| Inventory (local, untracked) | Scope read | Capability ids | Defects |
| --- | --- | --- | --- |
| `php-app-code.md` | `src/**`, `config/**`, `composer.json`, `symfony.lock`, `migrations/**`, `.env`, `.env.test`, `deptrac.yaml`, `workspace.dsl` | 41 (`CAP-01` to `CAP-41`) | 38 |
| `php-tests-and-quality.md` | Test suites, quality-tool configuration, git hooks | 36 (`CAP-TQ-01` to `CAP-TQ-36`) | 44 |
| `php-docker-make-load.md` | `Dockerfile`, four compose files, `Makefile`, `scripts/**`, `tests/Load/**` | 50 (`CAP-DM-01` to `CAP-DM-50`) | 40 |
| `php-ci-github-contracts.md` | 20 workflows, Dependabot, templates, setup guides, contract snapshots, Spectral ruleset | 36 (`CAP-CI-01` to `CAP-CI-36`) | 33 |
| `php-ai-docs-audit.md` | `.claude/skills/**`, `AGENTS.md`, `CLAUDE.md`, README, CONTRIBUTING, SECURITY, CHANGELOG, LICENSE, the enterprise audit | 33 (`CAP-AD-01` to `CAP-AD-33`) | 33 |
| **Total** | | **196** | **188** |

The enterprise audit is `php-enterprise-quality-audit-2026-07.md` (1,475 lines, dated 2026-07-02), read in full and cross-checked against the repository. The capability catalog (`research/capability-catalog.md`, tracked with this digest) merges the five inventories into 161 rows in 21 areas, reduces the 188 defects to 150 deduplicated lines, and gives each row one disposition.

For the Rust side, versions, licences, minimum supported Rust versions and release dates were read from the crates.io registry on 2026-11-10; image tags from the Docker Hub tag API; tool versions from npm and PyPI; action tags and commit SHAs with `git ls-remote --tags`. No version in this file is quoted from memory.

### 1.3 What was built and run

All four spikes ran on `rustc 1.97.0` / `cargo 1.97.0`, edition 2024, stable channel only, on a machine with 4 vCPU and 15 GiB of memory (comparable to a GitHub-hosted runner), with Docker Engine 29.9.2 and Compose v5.6.1. The spike sources stay in a local scratch directory and are not part of the repository; the configuration files that passed are reproduced in the raw reports and are to be carried into the architecture document.

| Spike | What was built | Run against | Observed |
| --- | --- | --- | --- |
| HTTP/API stack | One cargo package (11 source files and one test file, 1,554 lines): axum service with OpenAPI and GraphQL export subcommands, problem type, in-memory buses | Spectral 6.15.1, openapi-diff 2.1.2, oasdiff 1.37.0, Schemathesis 4.33.1, graphql-inspector 7.0.2 | 229 crates; cold debug build 58 s, cold release build 2 min 05 s, release binary 14.9 MiB; 17 tests pass; both exports byte-identical over 20 process runs; Schemathesis 62 cases pass with the PHP flags |
| Persistence, cache, messaging | One cargo project: sqlx repository and migrations, moka cache, SQS client, health service with three adapters and fakes | PostgreSQL 16.15, LocalStack 3.4.0 and 4.14.0, ElasticMQ 1.7.1, `redis-server` 7.0.15 | 32 tests pass; clippy clean with `-D warnings`; offline sqlx build with no database; a dead endpoint fails the SQS adapter in 3.6 ms and the PostgreSQL adapter at the 2 s pool timeout |
| Quality gates | Seven-crate workspace (one crate per bounded-context layer, a binary, an `xtask` architecture checker of about 730 lines), about 2,300 lines of Rust | Planted violations for every gate | Every gate observed passing and failing; 57 tests; 100% lines and functions with no exclusion flag; 125 mutants, 12 survivors on the first run and 0 after tests were added; all non-mutation gates in 6.5 s warm |
| Testing and delivery | Gherkin runner, multi-stage Dockerfile, compose design, contract fixtures, K6 kit, Bats tests, drafted workflows, release configuration, Structurizr | Docker builds and runs; actionlint 1.7.12, zizmor 1.32.0, yamllint 1.38.0 on the workflows | PHP `health_check.feature` runs unchanged (4 scenarios, 14 steps, 0.2 s); production image 44.3 MB for a hello-world service, cold build 1 min 27 s, rebuild after a source change 8.0 s; K6 smoke 101 requests in 10 s with both thresholds green; workflows linted, not executed |

### 1.4 What could not be verified

1. **Anything that needs GitHub.** Workflow execution, Codecov upload, SARIF upload, Dependabot behaviour, template sync, the GitHub App token flow. The drafted workflows were linted only. The live state of the PHP repository (branch protection, required checks, installed Apps, issues #176 to #203) was not inspected.
2. **Rust 1.99.1.** `scope-decisions.md` pins 1.99.1; every spike ran on 1.97.0 (the matching image tag carries 1.97.1). `rust-toolchain.toml` itself was documented, not exercised.
3. **Three behaviours that `scope-decisions.md` changes after the spikes.** The read-only broker probe (the spike used `CreateQueue`; only the `awslocal sqs list-queues` CLI call was run, as a compose healthcheck), the report of all failing dependencies (the spike reports the first failure), and `GET /ping` on port 8081 (the spikes used `/api/health/live` and port 8080).
4. **CodeQL for Rust.** Its status and query list come from GitHub documentation; the CLI could not be obtained in the sandbox.
5. **The release flow beyond dry runs.** Commit, tag, push and GitHub Release were not run. `cargo-mutants` exit code 3 (timeout) and `--shard` were not observed.
6. **Image and CI caching details.** `docker/build-push-action` with the `gha` cache and cargo-chef layers; a Docker image build with offline sqlx (the mechanism was verified on the host); installing `sqlx-cli` with `cargo-binstall`; the Compose `!reset` and `!override` tags on the runner's Compose 2.38.2 (verified on v5.6.1 only).
7. **Real AWS.** TLS to an SQS endpoint, IAM-role credentials, PostgreSQL over TLS, and the EC2 + S3 load-test runner (reviewed by reading only). LocalStack 4.14.0 was verified for SQS only, not for the EC2, S3, IAM and STS services the load-test dry run uses.
8. **Tools nobody evaluated.** No secrets scanner, no container image vulnerability scanner and no dev container definition was researched or run. `scope-decisions.md` 8.5 leaves the scanner choice to the architect.
9. **Scale.** All timings for the real service are projections from small spikes. Linux arm64 builds, musl versus glibc throughput and export stability across machines were not measured.
10. **Most rejected alternatives.** They were assessed from registry metadata and published sources. The ones that were run: `archaven`, `cargo-modules`, `cargo-machete`, `cargo-audit`, Semgrep CE, `vacuum`, openapi-diff, git-cliff, Testcontainers, the Redis adapter, ElasticMQ and the static musl image.

Sandbox limits that shaped the method: `api.github.com` returned 403 through the session proxy, so `cargo-binstall` fell back to compiling; Docker Hub answered one anonymous pull with HTTP 429.

---

## 2. PHP template findings

### 2.1 What the PHP template really is

**Runtime surface.** A Symfony 7 / API Platform 4 application on FrankenPHP: 32 PHP files, 818 lines in `src/`. Its real behaviour is two REST operations under `/api`: `GET /api/health` (204 or 500) and a placeholder `GET /api/example_api_resources` that returns one hard-coded object. The framework adds the docs UI at `/api/docs` and a GraphQL endpoint at `/api/graphql`. `make start` boots four containers: the application (HTTPS on 443, HTTP/3 on 443/udp, port 80, and a plain-HTTP tooling listener on 8081), PostgreSQL 16 (5432), Structurizr Lite (8080) and LocalStack 3.4.0 (4566). The web server, not the application, answers `GET /ping` with `200 pong`.

**Health contract.** One framework event fans out to three probes: `SELECT 1` on PostgreSQL, a read-through `get` of cache key `health_check`, and `CreateQueue health-check-queue` on SQS. Success is `204` with an empty body; any exception becomes `500 application/problem+json`. The three messages the Gherkin feature asserts (`Database is not available`, `Cache is not working`, `Message broker is not available`) exist only in test mocks. The configured cache is the local filesystem, not Redis, and the message-transport configuration is malformed and inert.

**Shared kernel.** Aggregate root with recorded events, domain-event base class, event-bus port with an in-process adapter, command-bus ports without an adapter, UUID value object. None of it is used by the two endpoints.

**Tests.** 36 unit tests in 18 classes, 6 integration tests in 3 classes (real PostgreSQL and LocalStack), 5 Gherkin scenarios in 2 features, and 50 Bats tests in 8 files that test the Makefile, 12 of them negative tests that plant bad input.

**Gates.** Psalm (level 8, the most lenient), PHPInsights (the only numeric gates: quality 100, complexity 95, architecture 100, style 100), PHPMD (not installed), PHP-CS-Fixer (fix mode), Rector, Infection 0.27 (no minimum score), Deptrac, CaptainHook (commit-message pattern and an 11-step pre-commit pipeline), and a guard script that locks 5 configuration files and requires 14 directories.

**CI.** 20 workflow files (589 lines), one job each: 17 run on pull requests (16 always, 1 path-filtered), 2 on push to `main` (one of them also on pull requests), 2 on a schedule. 15 of the 17 pull-request jobs rebuild the development image from scratch. The only cache is the pnpm store in one workflow. 12 of the 20 workflows declare no `permissions`.

**Delivery.** A 120-line, 5-stage `Dockerfile`; four compose files; a 307-line `Makefile` with 68 targets (56 with help text); `make ci` with 21 ordered steps; a K6 kit with four load profiles per endpoint and an AWS EC2 runner; automated release from conventional commits; Dependabot for composer and one npm directory; two template-sync workflows. Contract artifacts: an OpenAPI 3.1.0 snapshot (76 lines), a GraphQL SDL snapshot (179 lines) and a 297-line Spectral ruleset.

**AI guidance and documentation.** `AGENTS.md` (64 lines), `CLAUDE.md` (49 lines) and `.claude/skills/` (13 files, 608 lines: an index, a usage guide, a decision guide and ten skills of 33 to 63 lines). No assistant settings file exists. README (171 lines, eight badges of which six are static images), `CONTRIBUTING.md` (70 lines), `SECURITY.md` (7 lines, with an unfilled placeholder where the known-vulnerabilities process should be), a `CHANGELOG.md` that keeps the last five releases, and a CC0-1.0 `LICENSE` that the manifest contradicts (`proprietary`).

### 2.2 Capability and defect totals by area

Source: the capability catalog. "Rows" merge ids that state the same capability in more than one inventory. The disposition per area is in section 6.

| # | Area | Catalog rows | PHP ids | Deduplicated defect lines |
| --- | --- | --- | --- | --- |
| 1 | Service runtime and HTTP surface | 16 | 16 | 6 |
| 2 | Health and liveness | 7 | 9 | 7 |
| 3 | Example bounded context | 1 | 1 | 1 |
| 4 | Shared kernel | 6 | 6 | 10 |
| 5 | Persistence, cache and messaging | 7 | 7 | 5 |
| 6 | Architecture and its enforcement | 4 | 5 | 4 |
| 7 | Formatting, static analysis and complexity | 8 | 14 | 8 |
| 8 | Dependency and security checks | 4 | 6 | 4 |
| 9 | Unit and integration tests | 9 | 12 | 9 |
| 10 | End-to-end tests | 4 | 5 | 2 |
| 11 | Coverage and mutation testing | 3 | 5 | 3 |
| 12 | Makefile command surface and its tests | 12 | 16 | 20 |
| 13 | Contract artifacts and contract checks | 8 | 12 | 7 |
| 14 | Load testing | 14 | 15 | 15 |
| 15 | Containers and compose stacks | 18 | 19 | 5 |
| 16 | CI workflows | 3 | 5 | 9 |
| 17 | Release, dependency updates and template sync | 4 | 6 | 7 |
| 18 | Git hooks and commit convention | 2 | 4 | 2 |
| 19 | AI-assistant guidance | 23 | 24 | 14 |
| 20 | Human documentation | 7 | 8 | 10 |
| 21 | Architecture model (Structurizr) | 1 | 1 | 2 |
| | **Total** | **161** | **196** | **150** |

### 2.3 The most important defects, by theme

Defect references: `APP:D-nn` = `php-app-code.md`; `TQ:nn` = `php-tests-and-quality.md`; `DM:D-nn` = `php-docker-make-load.md`; `CI:D-nn` = `php-ci-github-contracts.md`; `AD:D-AD-nn` = `php-ai-docs-audit.md`. The capability each defect belongs to is given where the catalog names one.

**Gates that cannot fail**

- `make ci` cannot pass at HEAD: it calls a mess detector that is not installed, and no workflow runs it (TQ:1, DM:D-01; CAP-TQ-32).
- Mutation testing has no minimum score; its negative Bats test asserts output text only (TQ:2, CI:D-02; CAP-TQ-17; audit P8).
- No coverage threshold exists in the repository; two workflows run the same suites and upload twice (TQ:3, CI:D-07; CAP-TQ-16; P9).
- The code-style target always runs in fix mode and never fails; its "changed files only" expression expands to nothing (TQ:4, TQ:5, DM:D-02, CI:D-03; CAP-TQ-25).
- The OpenAPI diff is run without a fail flag, so a breaking change passes (DM:D-10, CI:D-01; CAP-DM-36; P24).
- Static analysis runs at the most lenient level (TQ:6; CAP-TQ-20; P11).
- The SARIF upload is skipped exactly when findings exist (CI:D-04; CAP-TQ-21).
- Undefined Gherkin steps do not fail the run (TQ:9; CAP-TQ-09).
- The architecture check leaves 14 of 32 classes outside every layer, including the whole health feature (APP:D-24; CAP-38).
- The protected-configuration guard is run by no workflow and sees nothing on a shallow checkout (TQ:42, DM:D-11, CI:D-05; CAP-41; P27).
- The commit-message rule is enforced only by a local hook (TQ:8; CAP-TQ-28).

**Dead scaffolding**

- The command-bus adapter is referenced by configuration but does not exist (APP:D-01; CAP-28); the rest of the shared kernel is unused (APP:D-10).
- The message transport is malformed and inert (APP:D-16; CAP-33); the only migration creates a foreign `test_user` table (APP:D-30); the ORM maps an empty directory (APP:D-31); the fixture loads nothing (TQ:20; CAP-24).
- The exported GraphQL schema is an accidental Relay and CRUD projection with no code behind it, yet it is diffed as a contract (APP:D-19, CI:D-27; CAP-19).
- Bats helper libraries cannot be obtained: the submodule declarations have doubled paths and no gitlinks (TQ:7, DM:D-26; CAP-TQ-18).
- Lint configuration that nothing runs: a code-sniffer file, an unreferenced security rule set, YAML-lint and Prettier files (TQ:33, CI:D-18; CAP-CI-34).
- Unused Mercure and Vulcain, a Redis extension without a Redis service, unused K6 extensions and dead load-test JavaScript (DM:D-34, DM:D-31, DM:D-30).

**Drift between documents and code**

- README promises that are false at HEAD: a working `make ci`, CodeQL analysis, enforced backward compatibility, subdomain isolation (CI:D-17, AD:D-AD-19; CAP-AD-24).
- Five diverging verification command lists; none equals `make ci` or hosted CI (AD:D-AD-04; CAP-AD-01). `make ci` and hosted CI differ in both directions (CI:D-24).
- A skill points at a directory that does not exist, and skills describe patterns with no exemplar (AD:D-AD-11, AD:D-AD-12; CAP-AD-07).
- `workspace.dsl` disagrees with the code: wrong layers, a false relationship, Redis, a single view (APP:D-28; CAP-40).
- The committed OpenAPI snapshot can drift from the code, and `info.version` is static (CI:D-10, APP:D-22; CAP-16).
- Licence, badge values and language-version statements disagree across files (AD:D-AD-24, APP:D-33, AD:D-AD-20, AD:D-AD-21; CAP-AD-30, CAP-AD-25).
- The audit itself contains factual slips (AD:D-AD-33); section 3 lists them.

**CI cost**

- No caching, concurrency control, timeouts or artifacts; each push starts 15 uncached development-image builds (CI:D-16; audit P17).
- Unit and integration tests run twice per pull request (CI:D-07).
- The specification is generated twice, a readiness loop repeats a health-gated start, and the architecture viewer starts in every job (CI:D-23).
- Dependency bumps use the `feat` prefix and cut a MINOR release each (CI:D-12; CAP-CI-28).
- The pre-commit hook runs integration and acceptance tests and a network audit on every commit (TQ:44; CAP-TQ-29).
- Container and image names are hard-coded to the template's name, which breaks synced workflows downstream (DM:D-16, CI:D-06).

**Security gaps**

- The tracked env file carries secret-shaped defaults (APP:D-35, DM:D-09; CAP-03; P4).
- No application logging, metrics, tracing, authentication or security headers (APP:D-37; P1, P2, P3, P5).
- Mutable action tags are used where write tokens and secrets are present (TQ:43, CI:D-19); the auto-fix workflow pushes to `main` and hands a signing key to an unpinned action (CI:D-11; CAP-TQ-27); the GraphQL workflow holds write permission on pull requests (CI:D-09; CAP-CI-23).
- The broker probe creates a queue on every health request (APP:D-14; CAP-11); the failure text is whatever the driver throws (APP:D-13; CAP-08).
- The load-test stack shares project, volumes and ports with the development stack and drops the development database (DM:D-03; CAP-DM-38).
- The AWS flow creates an IAM policy from an unrendered template and leaves the instance running on error (DM:D-04, DM:D-06; CAP-DM-48); the load-generator image has an unpinned base and core (DM:D-31).
- `SECURITY.md` is a stub with an unfilled placeholder (AD:D-AD-26; CAP-AD-28; P13). No secrets scan and no image scan exist (P4, P6).

---

## 3. The enterprise audit

The audit of 2026-07-02 scored 94 system quality attributes on a 1 to 5 scale. The average is 3.48; no attribute reached 5 (56 scored 4, 28 scored 3, 9 scored 2, 1 scored 1). It reduced the gaps to 28 root-cause problems, P1 to P28, tracked as GitHub issues #176 to #203.

### 3.1 Scores by category

| Category | Attributes | Average | Attributes at 2 or below |
| --- | --- | --- | --- |
| Reliability and Resilience | 13 | 2.69 | redundancy (1); degradability, durability, recoverability, resilience, survivability (2) |
| Observability and Operability | 10 | 3.10 | observability (2) |
| Security and Trust | 8 | 3.12 | confidentiality (2) |
| Performance and Scalability | 7 | 3.29 | elasticity (2) |
| Usability and Developer Experience | 9 | 3.56 | localizability (2) |
| Deployability and Portability | 10 | 3.70 | none |
| Process, Sustainability and AI-Native | 7 | 3.71 | none |
| Maintainability and Architecture | 18 | 3.89 | none |
| Testability and Correctness | 12 | 4.00 | none |

### 3.2 The 28 problems and their disposition

"Attr." is the number of attributes the audit says the problem unblocks. The disposition text is `scope-decisions.md` section 6. "Carried by" points at catalog rows (`CAP-nn`) and added capabilities (`NEW-nn`, section 6.3).

| Problem | Issue | Root cause | Attr. | Disposition | Carried by |
| --- | --- | --- | --- | --- | --- |
| P1 | #176 | No structured logging or correlation IDs | 8 | In scope | NEW-01, NEW-02 |
| P2 | #177 | No observability baseline; health response names no dependency | 10 | Partly: liveness/readiness split and dependency-naming failure response in scope; tracing and metrics deferred | CAP-DM-20, CAP-08, NEW-42 |
| P3 | #179 | No authentication or authorisation | 3 | Non-goal | scope 3.2 |
| P4 | #178 | Committed secrets; no secrets scanning | 4 | In scope: nothing secret-shaped committed, env reference, secrets scanning | CAP-03, CAP-DM-16, NEW-18, NEW-35 |
| P5 | #181 | No rate limiting or security headers | 3 | Security headers in scope; rate limiting non-goal | NEW-03 |
| P6 | #180 | No image scan; runtime hardening stops at non-root | 3 | Advisory/licence/source policy, image vulnerability scan, hardened runtime in scope; OSSF Scorecard follow-up | CAP-TQ-30, NEW-19, CAP-DM-02, CAP-DM-21 |
| P7 | #182 | Release publishes no image, SBOM, signature or provenance | 9 | Non-goal | scope 3.2 |
| P8 | #183 | Mutation testing has no threshold | 12 | In scope | CAP-TQ-17 |
| P9 | #185 | No coverage floor; duplicate coverage jobs | 9 | In scope | CAP-TQ-16 |
| P10 | #184 | Unseeded test data, non-strict runs, placeholder scenario | 7 | In scope | CAP-TQ-04, CAP-TQ-09, CAP-TQ-13 |
| P11 | #187 | Static analysis at the most permissive level | 2 | In scope | CAP-TQ-20 |
| P12 | #186 | Governance is not kept as code; a bot pushes to `main` | 3 | PR-title convention check and no bot pushes in scope; CODEOWNERS and rulesets follow-up | CAP-TQ-28, CAP-TQ-27 |
| P13 | #189 | `SECURITY.md` is a stub | 7 | In scope | CAP-AD-28 |
| P14 | #188 | No backup, recovery or rollback runbooks | 5 | Migration rollback command and operations notes in scope | NEW-13, NEW-37, CAP-DM-09 |
| P15 | #191 | No high-availability or scaling reference | 9 | Statelessness, graceful shutdown, limits and restart policy in scope; orchestrator manifests non-goal | NEW-04, CAP-DM-21, NEW-33 |
| P16 | #190 | No timeouts, retries, outbox, idempotency or chaos tests | 7 | Timeouts on every outbound call in scope; the rest non-goal | NEW-06, CAP-12 |
| P17 | #192 | CI rebuilds everything, no cache, no cancellation | 4 | In scope | NEW-25, NEW-26, NEW-28, CAP-TQ-16 |
| P18 | #193 | Checks run on pull requests only; narrow Dependabot | 6 | Scheduled checks and wider Dependabot in scope; auto-merge and stale bot follow-up | NEW-29, CAP-CI-28 |
| P19 | #195 | The only enforced budget is smoke p99 < 9000 ms; no artifacts | 6 | Realistic budgets and uploaded artifacts in scope; baselines follow-up | CAP-DM-41, CAP-DM-44, CAP-CI-17 |
| P20 | #194 | Empty internationalisation scaffolding | 3 | Non-goal | CAP-35 |
| P21 | #196 | No dev container, `.editorconfig` or preflight; submodules | 6 | Dev container, `.editorconfig`, no submodules, `doctor` in scope; multi-arch follow-up | NEW-40, NEW-39, CAP-TQ-18, NEW-21 |
| P22 | #197 | Architecture and onboarding documents live outside the repository | 11 | In scope | NEW-33 to NEW-36, NEW-22, CAP-CI-29 |
| P23 | #198 | No scaffolding; architecture rules hard-code placeholder names | 6 | In scope | NEW-15, NEW-38, CAP-38 |
| P24 | #199 | Breaking API changes do not fail CI; dead configuration | 6 | In scope | CAP-DM-36, CAP-CI-24, CAP-16, NEW-24 |
| P25 | #200 | GraphQL explorer off in development; no request collection | 4 | GraphQL explorer in dev and request collection in scope | CAP-19, NEW-41 |
| P26 | #201 | Licence mismatch, file-name typo, unpinned actions and images | 6 | In scope | CAP-AD-30, CAP-CI-27, CAP-DM-05, NEW-27, CAP-CI-36 |
| P27 | #203 | Guard not in CI; no docs-drift check; no assistant settings | 4 | In scope | CAP-41, NEW-30, NEW-31 |
| P28 | #202 | CQRS exemplars incomplete (no query bus, validated command or repository pair) | 8 | In scope through the example slice | NEW-07 to NEW-12, CAP-28, CAP-34 |

Three problems are non-goals (P3, P7, P20). The other 25 are in scope in full or in part, exactly as worded above; the deferred parts are listed as non-goals in `scope-decisions.md` 3.2.

### 3.3 Audit statements that do not match the repository

The audit's problem list is reliable; some of its evidence sentences are not. The cross-check found that `.github/linters/` is not empty (it holds an unused YAML-lint file); the command-bus adapter the audit cites does not exist; PHPMD is cited as enforced but is not installed; "all actions are SHA-pinned" contradicts the audit's own P26; two of the ten skills contain no command; and counts of workflows and Make targets vary inside the audit (actual: 20 workflow files, 68 targets). Planning uses the problem list and the inventories' own counts.

---

## 4. Rust ecosystem findings and the chosen stack

Reading the tables: "Verified version" is the exact version the spike resolved on 2026-11-10. "Run?" says whether the choice was compiled and executed here ("yes"), only linted or compiled, or only read in documentation. A choice that `scope-decisions.md` changes or settles is marked with an override id (`O-n`) explained in section 4.5. Template manifests use caret requirements with a committed `Cargo.lock`; the exact versions below are the evidence, not the manifest syntax.

### 4.1 HTTP and API

| Need | Choice | Verified version | Run? | PHP mechanism replaced | Main alternative rejected and why |
| --- | --- | --- | --- | --- | --- |
| Async runtime | `tokio`, multi-thread runtime built by hand in a synchronous `main` | 1.54.1 | yes | FrankenPHP worker process | `#[tokio::main]`: the runtime would start before `.env` is loaded, and mutating the environment with threads running is unsound (`set_var` is `unsafe` in edition 2024) |
| Web framework and middleware | `axum` with `tower` and `tower-http` | 0.8.9 / 0.5.3 / 0.7.1 | yes | Symfony HttpKernel, routing, controllers | `actix-web` 4.19.0: its own middleware model, so `tower-http` layers do not plug in; about 12 times fewer recent downloads |
| TLS and HTTP/3 | None in the service: plain HTTP/1.1 and h2c; the platform edge terminates TLS (port: O-2) | axum `http2` feature | h2c yes | Caddy inside FrankenPHP | In-process HTTP/3: the `h3` crate is at 0.0.8; in-process TLS makes certificates the service's problem |
| Code-first OpenAPI 3.1 | `utoipa` with `utoipa-axum` | 6.0.0 / 0.3.0 | yes | API Platform OpenAPI factory | `aide` 0.15.1: stable line 15 months old, next line in alpha for a year; `poem-openapi` emits 3.0.0 only |
| Docs UI at `/api/docs` | `utoipa-swagger-ui` with the `vendored` feature (Swagger UI 5.32.6) | 10.0.2 | yes | API Platform Swagger UI | `utoipa-scalar`, `-redoc`, `-rapidoc`: load JavaScript from a public CDN without a version pin |
| OpenAPI export | `<bin> openapi-export` printing `OpenApi::to_yaml()`; no database, no environment | utoipa `yaml` feature (`yaml_serde` 0.10.7) | yes; one hash over 20 runs | `api:openapi:export` | Emitting OpenAPI 3.2.0: breaks Spectral 6.15.1 and crashes openapi-diff 2.1.2 (both observed) |
| GraphQL endpoint | `async-graphql` with `async-graphql-axum`, default features off | 7.2.1 / 7.2.1 | yes | API Platform GraphQL | `juniper` 0.18.0: BSD-2-Clause, about 13 times fewer downloads, no complexity limits; `async-graphql` 8.0 is still a release candidate |
| GraphQL SDL export | `<bin> graphql-export` with sorted `SDLExportOptions` | 7.2.1 | yes; one hash over 20 runs | `api:graphql:export` | None considered |
| Problem responses | Hand-written RFC 9457 `Problem` type (about 170 lines) with problem-aware extractors, 404 and 405 fallbacks, panic layer and timeout middleware | n/a | yes | API Platform `rfc_7807_compliant_errors` | `problem_details` 0.11.0: its utoipa feature depends on utoipa 5.4; `tower-http` `TimeoutLayer` returns an empty body |
| Request validation | `validator` derive on adapter-layer DTOs | 0.21.0 | yes | Symfony Validator (enabled, unused) | `garde` 0.23.0: about 6 times less adoption; `axum-valid` 0.25.0 depends on `validator` 0.20 |
| Configuration and `check-config` | `clap` (derive, env) with `dotenvy` and `humantime` | 4.6.7 / 0.15.7 / 2.4.0 | yes | Symfony Dotenv, `check:requirements` | `config` 0.15.27: more machinery than an environment-only service needs, first error only; `figment`: no release in 30 months |
| Structured logs | `tracing` with `tracing-subscriber` JSON output | 0.1.44 / 0.3.23 | yes | None (PHP ships no logger) | None considered |
| Request ids | `tower-http` `request-id` layers on `x-request-id` (set and propagate) | 0.7.1 | yes | None | None considered |
| Graceful shutdown | `axum::serve(..).with_graceful_shutdown(..)` with a bounded grace period | axum 0.8.9 | yes | FrankenPHP signal handling | `tokio-graceful-shutdown` 0.20.0: more than a single-listener service needs |
| Buses, aggregate root, domain events | Hand-written traits (about 85 lines) and `TypeId`-keyed in-memory buses (about 115 lines), using `async-trait`, `thiserror`, `uuid` | 0.1.92 / 2.0.21 / 1.29.2 | yes (unit tests) | Symfony Messenger in-memory buses, `AggregateRoot`, `DomainEvent` | `cqrs-es` 0.5.0: an event-sourcing framework that dictates aggregate shape; `tokio::sync::broadcast`: subscriber errors cannot fail the command |
| Traces and metrics | Not adopted (O-7). Verified for the documented seam only: `opentelemetry` 0.33.1 over OTLP HTTP | 0.33.1 | yes, against a local sink | None | Not applicable |

### 4.2 Persistence, cache and messaging

| Need | Choice | Verified version | Run? | PHP mechanism replaced | Main alternative rejected and why |
| --- | --- | --- | --- | --- | --- |
| PostgreSQL access | `sqlx`, default features off, features `runtime-tokio`, `tls-rustls-aws-lc-rs`, `postgres`, `macros`, `migrate`, `uuid` | 0.9.0 | yes (PostgreSQL 16.15) | Doctrine DBAL and ORM | SeaORM 2.0.4: entities are macro-derived framework types, queries are not checked at compile time, 190 crates against 144; Diesel 2.3.14: async is a 0.x add-on crate |
| SQL checked at compile time without a database | Committed `.sqlx/` metadata, `SQLX_OFFLINE=true`, gate `cargo sqlx prepare --check` | 0.9.0 | yes | None | None considered |
| Migrations | Reversible SQL pairs in `migrations/`, embedded with `sqlx::migrate!`, run by `<bin> migrate`; `sqlx-cli` for authoring and database lifecycle | sqlx-cli 0.9.0 | yes | Doctrine Migrations | `refinery` 0.10.0: a second PostgreSQL client next to sqlx |
| Test database isolation | `#[sqlx::test]`: a fresh migrated database per test | 0.9.0 | yes | `dbname_suffix: _test` | Testcontainers (`testcontainers-modules` 0.15.0): lags the core crate, grows the dev graph to 304 crates, needs the Docker socket (O-13) |
| Cache | A `Cache` port with a `moka` in-process adapter and hand-written fakes | 0.12.16 | yes | Symfony Cache `cache.app` (filesystem), `ArrayAdapter` in tests | Redis by default (`redis` 1.8.1, verified behind the same port): adds a network service and a fourth health dependency PHP does not have; `quick_cache` 0.7.0: its documentation mentions no TTL or expiry |
| SQS client | `aws-sdk-sqs` with `aws-config`, default features off, features `rt-tokio`, `default-https-client`; explicit timeouts and retries | 1.120.0 / 1.13.1 | yes (LocalStack 3.4.0 and 4.14.0, ElasticMQ 1.7.1) | `Aws\Sqs\SqsClient` | `rusoto_sqs` 0.48.0: unmaintained for four years; `omniqueue` 0.3.0: 16.6 thousand total downloads |
| SDK test double | `aws-smithy-mocks` | 0.3.0 | yes | PHPUnit mocks of the client | None considered |
| Local SQS emulator | `localstack/localstack:3.4.0` | 3.4.0 | yes | Same image | `latest` (2026.10.6): exits with code 55 without an auth token; `elasticmq-native` 1.7.1: SQS only, which breaks the load-test dry run |
| Health check | One `DependencyHealthCheck` port, three adapters, checks run concurrently, each under `tokio::time::timeout` (probe and reporting: O-4, O-5) | async-trait 0.1.92, tokio 1.54.1 | yes | `HealthCheckController` and three event subscribers | Sequential checks: the worst case is the sum of the timeouts; returning the raw driver error: leaks internals |
| Integration tests | Compose-provided PostgreSQL and LocalStack; failures through fakes, dead endpoints and SDK mocks | n/a | yes (11 tests in 2.1 to 2.2 s) | `tests/Integration` inside the compose stack | Testcontainers, as above |
| TLS backend | `rustls` with `aws-lc-rs` for both sqlx and the SDK; no `ring` in the production graph | 0.23.46 / 1.19.0 | compiled; no TLS handshake exercised | PHP OpenSSL | None considered |

### 4.3 Quality gates

| Need | Choice | Verified version | Run? | PHP mechanism replaced | Main alternative rejected and why |
| --- | --- | --- | --- | --- | --- |
| Formatting | `rustfmt` with stable keys only (`max_width = 100`); a fix target and a separate failing check | 1.9.0-stable | yes, pass and fail | PHP-CS-Fixer (fix mode only) | Nightly rustfmt for import grouping: results differ between developers |
| Static analysis | Clippy policy in `[workspace.lints]`: `all`, `pedantic`, `nursery`, `cargo` and 70 named lints; `unsafe_code = "forbid"`; `-D warnings`; one pass for production targets, one relaxed pass for all targets, one rustdoc pass | clippy 0.1.97 | yes; 56 errors on a planted file | Psalm level 8, PHPInsights quality and style scores | `dylint` 6.4.0 custom lints: needs nightly; the whole `restriction` group: contains contradictory lints |
| Complexity limits | `clippy.toml`: `cognitive_complexity` 8, `excessive_nesting` 5, `too_many_lines` 50, `too_many_arguments` 5 | clippy 0.1.97 | yes, each lint seen firing | PHPInsights complexity 95, PHPMD | `rust-code-analysis-cli` 0.0.25: last published 2023-01-13 |
| Architecture boundaries | One crate per bounded-context layer, plus `cargo xtask arch`: a `cargo metadata` checker driven by `architecture.toml` (layer model: O-9) | `cargo_metadata` 0.23.1, `toml` 1.2.1 | yes; 8 diagnostics over 7 rule kinds in 1.1 s, clean pass in 0.12 s | Deptrac | `archaven` 1.1.0: missed 2 of 7 planted violations; `cargo_pup` 0.1.9: needs a pinned nightly |
| Manifest and lockfile validity | `--locked` on every cargo command | cargo 1.97.0 | yes | `composer validate` | None considered |
| Advisories, licences, bans, sources | `cargo-deny` | 0.20.2 | yes; 8 planted failures reported | `symfony security:check` | `cargo-audit` 0.23.0: a subset (advisories only) |
| Unused dependencies | `cargo-shear` | 1.14.0 | yes; found 2 real unused dependencies | None | `cargo-machete` 0.9.2: three false positives in a fixture directory; `cargo-udeps`: needs nightly |
| Security analysis with SARIF | CodeQL for Rust, plus SARIF from clippy (`clippy-sarif`) and from `cargo deny` | codeql-action v4.39.3; clippy-sarif 0.8.0 | CodeQL documented only; both SARIF producers run | Psalm taint analysis and SARIF upload | Semgrep CE 1.185.0: 10 Rust rules, no taint rules; missed 2 of 4 planted issues |
| Automated fixes | `cargo clippy --fix` then `cargo fmt`; the lint gate is the dry run | cargo 1.97.0 | yes | Rector | None considered |
| Toolchain and per-crate isolation | `rust-toolchain.toml`, `rust-version`, `cargo hack check --each-feature` (toolchain version: O-1) | cargo-hack 0.6.46 | hack yes; toolchain file documented only | `check:requirements`, PHP version constraint | `cargo-msrv`: a service pins one toolchain |
| Test runner | `cargo-nextest`; unit and integration suites selected by filterset | 0.9.151 | yes | PHPUnit | Plain `cargo test`: no JUnit output, no per-test timeout |
| Coverage floor | `cargo-llvm-cov` with `--fail-under-lines 100 --fail-under-functions 100`; regions reported | 0.9.1 | yes; failed at 96.9%, passed at 100% with no exclusion flag | PHPUnit clover and Codecov (no threshold) | `cargo-tarpaulin` 0.37.6: ptrace engine is Linux-specific; `#[coverage(off)]` is unstable on 1.97 |
| Mutation testing | `cargo-mutants`; the exit code is the gate; `--in-diff` on pull requests | 27.1.0 | yes; exit 2 with 12 survivors, exit 0 with none | Infection (no minimum score) | `mutest-rs`: needs nightly; `mutagen`: unmaintained |
| Git hooks | Committed `.githooks/` activated through `core.hooksPath`; `commit-msg` script with the PHP pattern | git | yes; rejects a bad message and a failing gate | CaptainHook | `cog verify` and `committed` 1.1.12: cannot express the `(#issue)` scope pattern; `lefthook` 2.2.2: an extra binary for every contributor |

### 4.4 Testing and delivery

| Need | Choice | Verified version | Run? | PHP mechanism replaced | Main alternative rejected and why |
| --- | --- | --- | --- | --- | --- |
| Unit tests | In-crate tests with `rstest` for parameterised cases | 0.28.0 | yes | PHPUnit `Unit` suite | A `tests/Unit` mirror tree: loses access to private items |
| Test data | `fake`, seeded; `proptest` where it adds value | 5.1.0 / 1.12.0 | yes | Faker (unseeded) | `cargo-fuzz` 0.13.2: needs nightly (O-12) |
| Gherkin end-to-end tests | `cucumber` with `harness = false`, in-process dispatch through `tower::ServiceExt::oneshot`, `fail_on_skipped()` | 0.23.0 (gherkin 0.16.0) | yes; PHP feature unchanged | Behat 3 | Black-box tests over HTTP only: cannot inject failing adapters in process |
| Makefile tests | bats-core; helpers loaded by name through `BATS_LIB_PATH` | Bats 1.14.0 in CI, 1.11.1 in the dev image; support 0.3.0, assert 2.2.4, file 0.4.0 | yes (Bats 1.13.0 and 1.11.1) | Bats with broken submodules | Git submodules: broken in PHP today and awkward with template sync |
| OpenAPI lint | Spectral image with the PHP `.spectral.yaml` unchanged, `--fail-severity=hint` | `stoplight/spectral:6.17.1` (6.15.1 also run) | yes | Spectral 6.15.1 in a custom image | `vacuum` 0.39.3: adds rules, so a clean file failed with 8 warnings |
| OpenAPI breaking-change check | `oasdiff breaking --fail-on ERR` | `tufin/oasdiff:v1.37.0` | yes | `openapitools/openapi-diff:2.1.2` (cannot fail) | openapi-diff 2.1.7 with `--fail-on-incompatible`: no rule ids, JVM image of 335 MB, crashes on 3.2 |
| GraphQL schema diff | `@graphql-inspector/cli` | 7.0.2 | yes | GraphQL Inspector action | The action: CI-only, cannot be reproduced with `make` |
| API conformance and fuzzing | Schemathesis image with the PHP flags `--checks all --phases=examples,coverage` | `schemathesis/schemathesis:4.33.1` | yes | Schemathesis pinned by digest | `schemathesis/action`: CI-only |
| Load tests | Stock `grafana/k6` image with its built-in dashboard; PHP scripts reused | `grafana/k6:2.3.0` | yes | Custom xk6 build | The xk6 build: its extra extensions are unused and the dashboard is built in |
| Lint for the K6 JavaScript | ESLint flat configuration with Prettier (package manager: O-11) | eslint 10.14.0, prettier 3.9.10 | yes | ESLint 8.57.1 with a frontend configuration | None considered |
| Builder image and dependency caching | Official slim Debian 13 Rust image with `cargo-chef` layers (image tag: O-1) | cargo-chef 0.1.78; spike on `rust:1.97-slim-trixie` | yes | FrankenPHP base stages | BuildKit cache mounts only: cold on every fresh CI runner |
| Production image | `gcr.io/distroless/cc-debian13:nonroot`, numeric user 65532, healthcheck through the binary's own subcommand | digest `sha256:cc66d222f0a7ec996b78a99fe76a969bc80b539a5016ad866c14d1949bc6d0ab` | yes; works read-only with all capabilities dropped | `frankenphp_prod` with curl healthcheck | Static musl on `distroless/static`: 8.3 MB, but musl's allocator is documented as slower under multi-threaded load (not measured here) and `-sys` crates need a musl C toolchain (O-14) |
| Development image | Same base with toolchain components, nextest, `watchexec`, make, git, jq, Bats | watchexec 2.8.0 | yes; 1.81 GB, reload in 1 s | `frankenphp_dev` with Xdebug | `bacon` 3.26.0: AGPL-3.0 |
| Compose stacks | Four files; the load-test stack under its own project name | `postgres:16-alpine` (16.15), `localstack/localstack:3.4.0` | `config` validated for three combinations; app and database run | Four PHP compose files | `services:` containers in workflows: versions duplicated outside compose |
| Architecture viewer | `structurizr/structurizr` with the `local` command | 2026.10.26 | yes; rendered the PHP `workspace.dsl` | `structurizr/lite:2024.07.02` | Structurizr Lite: end of life |
| CI toolchain and cache | `actions-rust-lang/setup-rust-toolchain` (wraps `Swatinem/rust-cache`) | v2.0.1 (rust-cache v2.9.3) | linted only | `docker compose exec` into a rebuilt dev image | `dtolnay/rust-toolchain`: no semver tags, ignores the toolchain file |
| CI tool installation | `taiki-e/install-action` with pinned versions and `fallback: none` | v2.90.3 | documented only | Tools baked into the dev image | `cargo install`: minutes per tool (126 s for `cargo-deny`) |
| Workflow linting | actionlint and zizmor | 1.7.12 / 1.32.0 | yes; zizmor found 2 template injections in the first draft | None | None considered |
| Release and CHANGELOG | cocogitto `cog bump --auto`, GitHub App token, `gh release create` (secrets: O-10) | cocogitto 7.0.0, action v4.2.0 | partly: verify, dry run and changelog run; commit, tag, push not run | `TriPSs/conventional-changelog-action`, `actions/create-release`, `tibdex/github-app-token` | release-please 17.11.2 and release-plz 0.4.3: release-PR flow; git-cliff 2.14.2: changelog only |
| Dependency updates | Dependabot: `cargo`, `rust-toolchain`, `github-actions`, `docker`, `docker-compose`, `npm` (commit prefix: O-10) | n/a | documented only | Dependabot: composer, npm | None considered |
| Template sync | `AndreasAugustin/actions-template-sync` with `actions/create-github-app-token` (flavours: O-10) | v2.6.0 / v3.3.0 | linted only | Same action at `@v2` with a deprecated token action | None considered |
| Repository linters | hadolint, shellcheck, yamllint, markdownlint-cli2, prettier, taplo, typos | 2.15.1 / 0.11.0 / 1.38.0 / 0.24.0 / 3.9.10 / 0.10.0 / 1.52.1 | yes | None (the README claims a super-linter no workflow runs) | super-linter v9.0.0: one all-in-one image, awkward to reproduce locally, and it would lint Rust with its own toolchain copy |

Other image digests recorded by the research: Spectral 6.17.1 `sha256:3b05e6cd9d43375c454cc0c022e91b3285851043b05c3e738342ea3f84cde76d`; oasdiff v1.37.0 `sha256:ff02af37a254b5199b9d72e7bba93dae5837a9119d59459168d1398893b47aea`; Schemathesis 4.33.1 `sha256:ddf8b3c9e0b3ee7edfa65dad4d0c0e05c53fb5d92f99db379d40da01883474c7`; `postgres:16-alpine` `sha256:aa6cb33cf4c19c765cefcf48d96176258800870a9a1e930ad126200f9d825139`.

Action tags and commit SHAs read with `git ls-remote` on 2026-11-10 (every third-party action is pinned by SHA with the tag as a comment):

| Action | Tag | Commit SHA |
| --- | --- | --- |
| `actions/checkout` | v7.0.1 | `3d3c42e5aac5ba805825da76410c181273ba90b1` |
| `actions-rust-lang/setup-rust-toolchain` | v2.0.1 | `a188e68d0fd2bd36ad51dccb2b07ba6e75105f30` |
| `Swatinem/rust-cache` | v2.9.3 | `d1527c8f67ef9af357290475d4af4b0fab8f12e0` |
| `taiki-e/install-action` | v2.90.3 | `cfdb7ed4ccc7c7dce37759eae89dc5674b469b39` |
| `actions/cache` | v6.1.0 | `55cc8345863c7cc4c66a329aec7e433d2d1c52a9` |
| `actions/upload-artifact` | v7.0.2 | `cf430e030ddbb5b0abf93d22962f4752f3646cd9` |
| `actions/setup-node` | v7.1.0 | `949feb2413d6458794dcd2491c4babbbce0c15c1` |
| `pnpm/action-setup` | v6.1.1 | `73e46992e05b19abd31f2f3c20e0e7370d762115` |
| `codecov/codecov-action` | v7.1.1 | `303a32d7a59b442fa8d48b6a1cc6825c09c847a5` |
| `github/codeql-action` | v4.39.3 | `33d7b85da1a04171e211befe2c739303b9a4c066` |
| `bats-core/bats-action` | 4.1.0 | `3566550e0f594451ac0f58fcbfdeca93aa29a45e` |
| `docker/setup-buildx-action` | v4.4.1 | `f87e5991a6d7451dcb8d9637bfbc97413f497069` |
| `docker/build-push-action` | v7.4.0 | `c3c9e263c25d99ce0380d002d59b67737d91b0dc` |
| `actions/create-github-app-token` | v3.3.0 | `64ab729a6294000f5aa4a4bc0b73d1f32bc45510` |
| `cocogitto/cocogitto-action` | v4.2.0 | `52d0023b4396897f1815648e553db2ab8d782f3a` |
| `AndreasAugustin/actions-template-sync` | v2.6.0 | `6e1d0089686f8940604fc15baae62367bf6ee860` |
| `zizmorcore/zizmor-action` | v0.8.1 | `45d7276c9ca790aabd705e31ca99c6185c3e76dd` |

### 4.5 Where `scope-decisions.md` overrides or settles the research

| Id | Topic | What the research did or recommended | Binding decision (scope section) | Consequence |
| --- | --- | --- | --- | --- |
| O-1 | Toolchain | Everything verified on 1.97.0; builder image `rust:1.97-slim-trixie` | Rust 1.99.1 in `rust-toolchain.toml`, `rust-version` to match, edition 2024, builder `rust:1.99.1-slim-trixie` (8.1) | Version-sensitive items (lint policy, tool compatibility) are re-verified on 1.99.1 during implementation |
| O-2 | Service port | Spikes listened on 8080; drafted defaults (`SCHEMATHESIS_URL`, load-test `apiPort`) point at 8080 | Published port 8081 (3.1) | PHP defaults stay: `SCHEMATHESIS_URL=http://localhost:8081`, `API_PORT=8081`; host port 8080 remains the architecture viewer's |
| O-3 | Liveness | `GET /api/health/live` returning 204, documented in OpenAPI | `GET /ping` returning `200` `pong`, absent from the OpenAPI document (3.1, 8.3) | Container healthcheck and CI wait loops use `/ping`, as in PHP |
| O-4 | Broker probe | `CreateQueue health-check-queue`, kept "for parity" | Read-only call, for example `ListQueues` with a limit of one (8.4) | Not run in the spike; the implementation verifies it against LocalStack |
| O-5 | Failure reporting | First failure in registration order; the HTTP spike returned `database is unavailable` | All failing dependencies are reported; the three PHP strings are exact and owned by application code (3.1) | The health use case aggregates results instead of returning the first error |
| O-6 | Health status | Open question: 500 or 503 | `500` problem response, as in PHP (3.1) | The PHP Gherkin feature is reused unchanged |
| O-7 | Telemetry | OpenTelemetry (0.33.1) verified; Prometheus exporter compiled | Neither is added; only the seam is documented (3.2, 8.3) | Saves 17 crates; the OTLP client pitfall is recorded in section 5 for whoever adds it later |
| O-8 | Make execution model | PHP executes in the container by default and only `CI=1` switches; the delivery research recommends native cargo with an opt-in exec switch | Native by default in the current environment; `docker compose exec` is an explicit opt-in; the switch honours `CI=true` and is tested (5, 8.8) | Confirms the research and reverses the PHP default; the architect names the variable |
| O-9 | Layer model | Spike `architecture.toml`: infrastructure may depend on application; domain may use `thiserror` and `uuid`; HTTP DTOs placed in infrastructure or a fourth layer | Three layers; application holds use cases and API-facing adapters, infrastructure holds driven adapters; each depends on domain and not on the other; domain may use `thiserror`, `uuid`, `async-trait` and at most one pure time crate (8.2) | `architecture.toml` is rewritten to this model; ports live in domain; the application allow-list must admit the web, OpenAPI, GraphQL and validation crates |
| O-10 | Release, Dependabot, sync | Release research notes that the App-token action prefers a client id; its Dependabot example uses the `feat` prefix for cargo; it recommends one sync workflow | Same secrets as PHP (`VILNACRM_APP_ID`, `VILNACRM_APP_PRIVATE_KEY`), clean exit when absent; dependency updates must not trigger feature releases; both PAT and App sync flavours with an ignore list and a self-sync guard (8.9) | The cargo prefix is not `feat`; two sync workflows ship, and each runs only when its own credentials exist (product brief assumption A-6) |
| O-11 | JavaScript tooling | npm was used in the sandbox | pnpm is kept (8.7) | Same workflow as PHP for the load-test directory |
| O-12 | Fuzzing | Optional `fuzz/` directory documented | No fuzz directory (8.6) | Schemathesis and `proptest` cover the need |
| O-13 | Testcontainers | Verified; recommended as optional | Not used (8.4) | One place pins service versions: the compose files |
| O-14 | Image variants | Static musl variant measured (8.3 MB) | No musl variant (8.7) | One supported runtime image |
| O-15 | Paths and file names | Research layouts used `tests/cli`, `tests/load` and `docker-compose.load_test.yml` | PHP paths kept: `features/`, `tests/Load/`, `tests/CLI/bats/`, `scripts/`, both snapshot paths, `workspace.dsl`; the four PHP compose file names kept (8.7) | The load-test file keeps the PHP name `docker-compose.load_test.override.yml` |
| O-16 | OpenAPI diff tool | HTTP research left openapi-diff or oasdiff open | oasdiff with label `approved-breaking-change` (8.7) | Same label approves OpenAPI and GraphQL breaks |
| O-17 | Architecture viewer | Structurizr Lite is end of life; unified image verified | Unified image (8.7); section 3.1 of the same document still says "Structurizr Lite stays" | Section 8.7 is followed as the later and more specific decision |

---

## 5. Pitfalls discovered by running: implementation checklist

This list merges the pitfall lists of the four Rust reports, with duplicates removed. Rows were observed by running the spikes, except those about behaviour that could not be executed here (PF-20, PF-29, PF-60 to PF-62, PF-64 to PF-66), which come from upstream source and documentation read during the spikes. Each row is a check: a story that touches the area must satisfy the "Mitigation" column, and a review may cite the id. "Src" names the report: H = HTTP/API, D = data, cache and messaging, Q = quality gates, T = testing and delivery.

| Id | Area | Pitfall | Consequence | Mitigation | Src |
| --- | --- | --- | --- | --- | --- |
| PF-01 | Build | `utoipa-swagger-ui` downloads Swagger UI from GitHub in its build script unless `vendored` is on | Offline and hermetic image builds fail | Enable the `vendored` feature | H |
| PF-02 | Build | The sqlx `query!` macros read a runtime `DATABASE_URL` at compile time, and `.sqlx/` metadata goes stale when SQL or schema changes | Pointing the service at an empty database breaks compilation; offline builds fail on a missing query | Commit `.cargo/config.toml` with `[env] SQLX_OFFLINE = "true"`; commit `.sqlx/`; gate with `cargo sqlx prepare --check -- --all-targets --all-features` | D |
| PF-03 | Build | New migration files are not re-embedded until the crate recompiles for another reason | A binary without the newest migration | Add the `build.rs` that `sqlx migrate build-script` generates | D |
| PF-04 | Build | Default features of `aws-sdk-sqs` and `aws-config` pull a legacy stack (hyper 0.14, rustls 0.21) and SSO crates; `aws-lc-sys` compiles C | Two TLS stacks, longer builds, build failure without a C compiler | `default-features = false` with `rt-tokio` and `default-https-client`; `tls-rustls-aws-lc-rs` in sqlx so one crypto backend is built; builder image with a C compiler | D, T |
| PF-05 | Build | An SDK `BehaviorVersion` pin starts to emit a deprecation warning after a routine SDK bump | With warnings denied, a dependency update fails CI | Pin the behaviour version, move the pin in the same reviewed change, group `aws-*` crates in Dependabot | D |
| PF-06 | Build | `cargo install --locked sqlx-cli` no longer works for 0.9.0; `cargo-binstall` silently compiles from source when the GitHub API is unreachable or rate-limited (6 min 35 s, then a failure) | Slow or failing tool installation in images and CI | Pin exact tool versions; fetch release archives by URL in images; `fallback: none` in the CI install action | D, T |
| PF-07 | Build | Bumping `[workspace.package].version` leaves `Cargo.lock` stale | `cargo build --locked` fails after a release commit | The release script runs `cargo update --workspace` before committing | T |
| PF-08 | Build | Features unify across the workspace | A layer crate compiles only because another crate enables a feature | `cargo hack check --workspace --each-feature` | Q |
| PF-09 | Build | A path dependency inside the workspace root becomes a workspace member automatically | Fixture crates join the real workspace | Fixture workspaces sit outside the member globs; their stand-in crates sit outside the fixture root | Q |
| PF-10 | Build | `std::env::set_var` is `unsafe` in edition 2024 and `dotenvy` mutates the environment | Unsound if `.env` is loaded after threads start | Synchronous `main`: load `.env`, parse configuration, then build the runtime | H |
| PF-11 | Build | The CI toolchain action sets `CARGO_BUILD_WARNINGS=deny` | A build that passes locally fails in CI | Set the same variable in the Makefile's gate path | T |
| PF-12 | HTTP | axum's built-in extractor rejections are `text/plain` | Schemathesis reports an undocumented content type (ST105); not every error is a problem document | Use the problem-aware extractors everywhere; forbid the bare ones in handlers | H |
| PF-13 | HTTP | utoipa takes `info.version` from the crate version by default | Every release changes the committed snapshot and trips the drift and diff gates | Source `info.version` explicitly; the architect records how | H |
| PF-14 | HTTP | Export order and response order are not stable by default (utoipa `preserve_order` features, `validator`'s `HashMap` of errors, GraphQL root type name and SDL order) | Noisy snapshot diffs; non-deterministic violation lists | Leave `preserve_order` off; sort violations; name the root type `Query`; export SDL with the sorted options | H |
| PF-15 | HTTP | OpenAPI 3.2.0 output | Spectral 6.15.1 fails and openapi-diff 2.1.2 crashes | Stay on 3.1.0 | H |
| PF-16 | HTTP | The exported YAML uses unindented block sequences | It fails yamllint and Prettier | Exclude `.github/openapi-spec/` from both | H |
| PF-17 | HTTP | The PHP Spectral ruleset requires an operation id, tag, summary, description, a 2xx response and a 4xx or `default` response on every operation | The lint fails, also for the health operation | Declare all of them, with a `default` problem response on every operation | H, T |
| PF-18 | HTTP | clap echoes an invalid value in its error message | A malformed secret is printed | No validating parsers on secrets; parse into a redacting type and validate afterwards | H |
| PF-19 | HTTP | `tower-http` defaults: `TraceLayer` logs responses at DEBUG, `TimeoutLayer` returns an empty body, and layers added later miss early failures | No access log at `info`; a timeout that is not a problem document; panic and 404 responses without `x-request-id` | Raise `on_response` to INFO; use the template's own timeout middleware; put the request-id layers first | H |
| PF-20 | HTTP | `axum::serve` has no header-read timeout and no connection limit | Unsafe when exposed directly to the internet | Document that a proxy or load balancer must sit in front | H |
| PF-21 | HTTP | `async fn` in traits is not dyn-compatible on 1.97 | Ports cannot be used as `Arc<dyn Trait>` | `async-trait` on ports | H, D |
| PF-22 | HTTP | Validation rules and schema constraints are declared separately | They drift; Schemathesis reports a schema-valid request rejected (ST202) | Keep both on the same DTO; keep the Schemathesis gate | H |
| PF-23 | HTTP | A liveness probe that touches dependencies | A database outage restarts every replica | Container healthchecks use `/ping`; `/api/health` is the readiness check | H, T |
| PF-24 | HTTP | The OTLP exporter with the async `reqwest-client` feature panics on first export | No telemetry reaches the collector | Not shipped now; whoever adds telemetry uses `reqwest-blocking-client` and keeps the four crates on matching minors | H |
| PF-25 | Data | The sqlx pool's default `acquire_timeout` is 30 s | A health request hangs that long when the database is down | `connect_lazy` with an explicit acquire timeout; the health deadline is at least as long | D |
| PF-26 | Data | The first SQS call on a fresh LocalStack container takes 2.2 to 2.3 s | A 2 s attempt timeout fails the first probe | Compose healthcheck `awslocal sqs list-queues` warms the service before anything depends on it | D |
| PF-27 | Data | Current LocalStack images require an auth token | The container exits with code 55 | Pin `3.4.0`; never `latest` or a `2026.x` tag | D |
| PF-28 | Data | `#[sqlx::test]` reads `DATABASE_URL` literally, needs a role that may create databases, and leaves a `_sqlx_test` schema | Tests panic under a renamed variable | Keep the name `DATABASE_URL`; the test role may create databases | D |
| PF-29 | Data | Applied migrations are checksummed | Editing one makes the migrator fail | Never edit an applied migration | D |
| PF-30 | Data | Adapter error branches under a 100% coverage floor | Pressure to stop containers in tests | Reach them with a dead endpoint (`127.0.0.1:1`) and SDK mocks | D |
| PF-31 | Data | A cache probe that only reads | A cache that drops writes still reports healthy | Write, read back and compare on each call | D |
| PF-32 | Gates | `cargo mutants` with `CARGO_TARGET_DIR` exported | The real tree later runs a stale mutated test binary | Run it with the variable unset (`env -u CARGO_TARGET_DIR`) | Q |
| PF-33 | Gates | A per-crate `clippy.toml` replaces the workspace one | Thresholds and banned-API rules disappear for that crate without a message | Exactly one `clippy.toml`, at the root | Q |
| PF-34 | Gates | `cargo deny --format sarif check` exits 0 on failures | A gate that cannot fail | Gate with the human format; produce SARIF in a separate step | Q |
| PF-35 | Gates | Unstable `rustfmt.toml` keys are ignored on stable with a warning and exit 0 | Rules that look enforced but are not | Stable keys only | Q |
| PF-36 | Gates | Inline test modules are measured by coverage, `main` is not executed by unit tests, and rustfmt can put `)?;` on a line of its own | 100% is unreachable without exclusion flags; an untested error path shows as an uncovered line | Unit tests in sibling `tests.rs` files; `main.rs` holds only the entry point; one subprocess smoke test per binary; test each error path | Q |
| PF-37 | Gates | `clippy::cognitive_complexity` counts every assertion | Table-style tests exceed the budget | A second clippy pass for test targets with the complexity lints relaxed | Q |
| PF-38 | Gates | `#![expect(...)]` fails when it is not needed, and lint inheritance is all-or-nothing per crate | Blanket relaxations break the gate | `#[expect(..., reason = "...")]` only where a finding exists | Q |
| PF-39 | Gates | nextest does not run doc tests | Doc tests are skipped without notice | Keep `cargo test --doc` | Q, T |
| PF-40 | Gates | `cargo-llvm-cov` needs the `llvm-tools-preview` component | Coverage fails on a fresh machine | List the component in `rust-toolchain.toml` | Q, T |
| PF-41 | Gates | `#[mutants::skip]` needs the `mutants` crate as a dependency | The domain allow-list rejects it | Exclude in `.cargo/mutants.toml`, which the research proposes as a protected file | Q |
| PF-42 | Gates | The advisory check needs the network, and a new advisory can fail `main` with no code change | Slow or failing commits; unexpected red builds | Hooks run only the offline dependency checks; a scheduled advisory job runs on `main` | Q |
| PF-43 | Gates | The lint policy lives in the root `Cargo.toml`, which changes with every dependency | The file cannot be protected as a whole | The guard compares the `[workspace.lints]` section | Q |
| PF-44 | Gates | A planted Rust file that does not compile breaks every gate, and a stray file no module declares is ignored by `cargo fmt --check` | Negative tests prove nothing | Use fixture workspaces through `--manifest-path`, or append to an existing module and restore it in `teardown`; fixtures use a non-`.rs` extension | Q, T |
| PF-45 | Tests | nextest cannot list a cucumber binary (exit code 104) | nextest fails for the whole workspace | `default-filter = 'not binary(bdd)'`; run the Gherkin target with `cargo test` | T |
| PF-46 | Tests | Without `fail_on_skipped()` an undefined step is reported as skipped | The run passes with unimplemented steps | Call `fail_on_skipped()` | T |
| PF-47 | Tests | `Faker.fake::<Uuid>()` does not produce a version 4 UUID | Assertions on the UUID version fail at random | Use `UUIDv4.fake()` | T |
| PF-48 | Tests | Cucumber runs scenarios concurrently (64 by default) | Scenarios that share state interfere | Each scenario builds its own application; tag shared-state scenarios `@serial` | T |
| PF-49 | Tests | Bats is not preinstalled on GitHub runners, and each negative test recompiles a crate | A missing tool; a slow job (estimated 8 to 15 minutes warm) | Install through the Bats action; choose negative tests to fit the time budget | T |
| PF-50 | Containers | The distroless image has no shell, curl or wget | A `CMD-SHELL` healthcheck reports unhealthy | Exec-form healthcheck that calls the binary's own subcommand, in the Dockerfile and in compose | T |
| PF-51 | Containers | A compose profile removes a service that another service still depends on | `service app depends on undefined service` | Replace the dependency list with `depends_on: !override` in that file | T |
| PF-52 | Containers | `watchexec` watching the project root also watches the `target/` volume | A flood of watcher errors | Explicit `--watch` paths; `target/` in a named volume | T |
| PF-53 | Containers | `.dockerignore` excludes too much | An image build without query metadata or lockfile | Never exclude `.sqlx/`, `migrations/` or `Cargo.lock`; exclude `target/` and `.git/` | T, D |
| PF-54 | Containers | The Structurizr container writes `workspace.json` and `.structurizr/` next to the DSL as uid 65532 and has no shell | Generated files in the repository; no curl healthcheck | A writable mount for the viewer; generated files ignored in git | T |
| PF-55 | Containers | `panic = "abort"` in the release profile (used in the image spike for size) | A panicking handler kills the process | Keep unwinding | T |
| PF-56 | Containers | The first `cargo run` on an empty `target` volume takes minutes | The development container is reported unhealthy | A long `start_period` with a short `start_interval` | T |
| PF-57 | Containers | Hard-coded compose container names | Workflows break in downstream repositories | Address services by name through `docker compose exec` and `cp` | T |
| PF-58 | Containers | Anonymous Docker Hub pulls can be rate-limited (HTTP 429 observed once) | Intermittent CI failures | Pin images by digest; authenticate where the organisation provides credentials | H |
| PF-59 | CI | `${{ github.* }}` expressions inside `run:` scripts | zizmor reports template injection | Pass values through `env:` | T |
| PF-60 | CI | Pull-request runs can only restore caches saved on the base branch | Every pull request compiles from scratch | Run the main CI workflow on pushes to `main`; save caches only there | T |
| PF-61 | CI | Fork pull requests have a read-only token and no secrets | SARIF and Codecov upload steps fail | Guard those steps by repository; neither is a gate | T |
| PF-62 | CI | A release commit pushed with the App token re-triggers push workflows | A release loop | `[skip ci]` in the commit message and an `if:` guard on the release job | T |
| PF-63 | CI | cocogitto validates commit types, not scope patterns, and `core.hooksPath` is a per-clone setting | `feat: ...` without an issue passes; a clone without hooks is unchecked | Keep the pattern script for the `(#issue)` scope in the hook and run the same check on the pull-request title in CI | T, Q |
| PF-64 | CI | Dependabot does not update tool versions held in Makefile variables or `ARG` lines | Stale pins | Keep those pins in one place and review them on the sync cadence | T |
| PF-65 | CI | Template sync merges with `-X theirs`, and workflow files need a token with the workflows permission | Downstream files are overwritten; sync fails on workflow changes | `.templatesyncignore` lists downstream-owned files; the App token carries the permission | T |
| PF-66 | CI | PHP-side pieces that are deprecated or removed: `tibdex/github-app-token`, `actions/create-release`, `codecov-action@v3`, the Dependabot `reviewers` key, ESLint 8 | Copying them ships dead or failing automation | Use the replacements in section 4.4 | T |

---

## 6. Parity matrix summary

The full matrix is `research/capability-catalog.md` (161 rows, each with its PHP mechanism, disposition and note). Dispositions: **Kept** = same contract, Rust mechanism; **Fixed** = intent kept and a named PHP defect corrected; **Extended** = the scope asks for more than PHP has; **Dropped** = non-goal with a cited reason. Counts below are PHP capability ids.

### 6.1 Disposition and Rust mechanism by area

| # | Area | Kept | Fixed | Extended | Dropped | Rust mechanism |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | Service runtime and HTTP surface | 8 | 3 | 1 | 4 | One axum binary on port 8081, `/api` prefix, RFC 9457 errors everywhere, typed environment configuration with `check-config`, JSON logs |
| 2 | Health and liveness | 5 | 4 | 0 | 0 | One probe port with three adapters run concurrently under timeouts; `/ping` answered by the binary itself |
| 3 | Example bounded context | 0 | 0 | 1 | 0 | A real slice at `/api/example_api_resources`: create, read by id, bounded list, event subscriber, matching GraphQL operations |
| 4 | Shared kernel | 2 | 4 | 0 | 0 | Hand-written traits with in-process command, query and event buses registered in one composition root; validated UUID value object |
| 5 | Persistence, cache and messaging | 2 | 4 | 0 | 1 | sqlx with embedded reversible migrations and an explicit `migrate` command; moka behind a cache port; AWS SDK SQS client against LocalStack 3.4.0 |
| 6 | Architecture and its enforcement | 3 | 2 | 0 | 0 | One crate per bounded-context layer, checked by `cargo xtask arch` from `architecture.toml` |
| 7 | Formatting, static analysis and complexity | 5 | 9 | 0 | 0 | `rustfmt` check, clippy workspace policy with warnings denied, `clippy.toml` thresholds, repository linters, protected-configuration guard in CI |
| 8 | Dependency and security checks | 1 | 3 | 2 | 0 | `--locked`, cargo-deny, cargo-shear, CodeQL and SARIF upload, secrets and image scans, images and actions pinned |
| 9 | Unit and integration tests | 7 | 3 | 0 | 2 | nextest, rstest, seeded `fake` data, fakes behind ports, `#[sqlx::test]` against compose-provided services |
| 10 | End-to-end tests | 2 | 3 | 0 | 0 | cucumber-rs running `features/health_check.feature` in process; undefined or skipped steps fail |
| 11 | Coverage and mutation testing | 0 | 5 | 0 | 0 | cargo-llvm-cov with lines 100 and functions 100; cargo-mutants with the exit code as the gate |
| 12 | Makefile command surface and its tests | 6 | 10 | 0 | 0 | Makefile with native execution by default, `help` as default goal, one canonical `make ci`; Bats tests with helpers through `BATS_LIB_PATH` |
| 13 | Contract artifacts and contract checks | 5 | 7 | 0 | 0 | Export subcommands, snapshot equality in CI, Spectral, oasdiff, graphql-inspector, Schemathesis against the production image |
| 14 | Load testing | 5 | 8 | 2 | 0 | The PHP K6 kit on the stock `grafana/k6` image, an isolated compose project, results uploaded as CI artifacts |
| 15 | Containers and compose stacks | 14 | 2 | 2 | 1 | cargo-chef multi-stage Dockerfile, distroless non-root production image, development image with watch-and-rebuild, four compose files |
| 16 | CI workflows | 3 | 0 | 0 | 2 | One status check per gate, native cargo with a build cache, actions pinned by SHA, concurrency and timeouts; no bot commits |
| 17 | Release, dependency updates and template sync | 2 | 3 | 1 | 0 | cocogitto release on push to `main`, Dependabot for six ecosystems, PAT and App sync with an ignore list and a self-sync guard |
| 18 | Git hooks and commit convention | 0 | 4 | 0 | 0 | `.githooks/` through `core.hooksPath`; the VilnaCRM pattern in the hook and in a pull-request title check |
| 19 | AI-assistant guidance | 11 | 13 | 0 | 0 | `AGENTS.md`, `CLAUDE.md`, ten capability-named skills, one verification list equal to `make ci`, drift check, assistant settings |
| 20 | Human documentation | 0 | 7 | 0 | 1 | README, CONTRIBUTING, complete SECURITY policy and in-repository references for architecture, operations, environment and Make targets |
| 21 | Architecture model (Structurizr) | 0 | 1 | 0 | 0 | `workspace.dsl` with context, container and component views, validated in CI |
| | **Total (196 ids)** | **81** | **95** | **9** | **11** | |

By catalog row the totals are 71 kept, 73 fixed, 8 extended and 9 dropped (161 rows).

### 6.2 Dropped capabilities

| Id(s) | PHP capability | Reason |
| --- | --- | --- |
| CAP-20 | Development-only error-page preview route | HTML error pages belong to HTML templating, a non-goal (scope 3.2); every error is a problem document |
| CAP-35 | Translation and locale facility | Internationalisation is a declared non-goal (scope 3.2, audit P20); no empty scaffolding ships |
| CAP-36 | Server-side HTML templating and cookie sessions | Configured but unused in PHP (scope 3.2) |
| CAP-DM-19 | HTTPS, HTTP/3, automatic certificates and compression on the public listener | Left to the platform edge; no reverse proxy ships (scope 3.1, 3.2). Request logging moves into the service (NEW-01, NEW-02) |
| CAP-33 | Configuration slot for an asynchronous message transport | PHP has no working consumer; no consumer and no outbox ship (scope 3.1, 3.2) |
| CAP-24, CAP-TQ-15 | Data-fixture loader and `load-fixtures` command | The PHP fixture loads nothing; no dead scaffolding (scope principles 5 and 7). Tests create their own data |
| CAP-DM-22 | Debugger call-back wiring from the container to the host IDE | Specific to Xdebug; the dev container definition (NEW-40) covers in-container debugging |
| CAP-TQ-27, CAP-CI-13 | A bot commits formatting and refactoring fixes to the default branch | No bot commits other than the release identity (scope 3.1, audit P12); replaced by the failing format check |
| CAP-AD-33 | A dated quality audit document | No new audit is a deliverable of this release (scope principle 7); the PHP audit is consumed as input |

Not carried inside kept rows: the Make targets `cache-clear`, `cache-warmup`, `commands` and `load-fixtures`; the environment variables `APP_SECRET`, `AWS_SQS_VERSION`, `EMAIL_QUEUE_NAME`, `MESSENGER_TRANSPORT_DSN`, `CADDY_MERCURE_JWT_SECRET`, `SERVER_NAME`, `APP_RUNTIME_HTTP_ENV`, `XDEBUG_MODE`, `KERNEL_CLASS`, `PANTHER_*`, `SYMFONY_DEPRECATIONS_HELPER`, `LOAD_TEST_CONFIG`; ports 80 and 443; the secrets `GPG_PRIVATE_KEY` and `GPG_PASSPHRASE`.

### 6.3 Capabilities added beyond PHP

| Id | Capability | Area |
| --- | --- | --- |
| NEW-01 | The service writes structured JSON logs | 1 |
| NEW-02 | A request/correlation id is accepted or generated for every request, echoed in the response and attached to every log line | 1 |
| NEW-03 | Security response headers on every response | 1 |
| NEW-04 | Graceful shutdown on SIGTERM | 1 |
| NEW-05 | Typed configuration validated at start-up; the service refuses to start on an invalid or missing value | 1 |
| NEW-06 | An explicit timeout on every outbound call | 1 |
| NEW-07 | Create an `ExampleApiResource` through the command path: validated input, command bus, aggregate, domain event | 3 |
| NEW-08 | Read one `ExampleApiResource` by id through the query path | 3 |
| NEW-09 | An in-process domain-event subscriber writes a structured audit log line | 3 |
| NEW-10 | GraphQL exposes the queries that match the REST reads and the create mutation, and nothing else | 3 |
| NEW-11 | Query-bus port with a working in-process adapter; a missing query handler is an error | 4 |
| NEW-12 | Repository port with a PostgreSQL adapter and a migration for the example resource | 5 |
| NEW-13 | A documented command rolls a migration back | 5 |
| NEW-14 | Architecture rules forbid one bounded context from reaching into another except through the shared kernel | 6 |
| NEW-15 | A scaffolding command creates a new bounded-context module that compiles and passes the architecture gate | 6 |
| NEW-16 | The Structurizr model is validated in CI | 21 |
| NEW-17 | Unused-dependency check as a gate | 8 |
| NEW-18 | Secrets scanning as a gate | 8 |
| NEW-19 | Vulnerability scan of the production image | 8 |
| NEW-20 | Workflow files, shell scripts and Dockerfiles are linted | 7 |
| NEW-21 | A `doctor` target checks host prerequisites | 12 |
| NEW-22 | A one-shot `bootstrap` target prepares a fresh clone | 12 |
| NEW-23 | Image, container and compose project names derive from one project-name variable | 15 |
| NEW-24 | The toolchain is pinned in `rust-toolchain.toml` | 15 |
| NEW-25 | CI caches dependencies and build output | 16 |
| NEW-26 | CI cancels superseded runs and every job has a timeout | 16 |
| NEW-27 | Least-privilege workflow permissions; third-party actions pinned by commit SHA | 16 |
| NEW-28 | Heavy jobs are skipped for documentation-only changes | 16 |
| NEW-29 | The security and CI checks run on a schedule on `main` | 16 |
| NEW-30 | A documentation-drift check verifies the commands cited in the guidance | 19 |
| NEW-31 | A committed assistant settings file with an allowlist and a guard on protected files | 19 |
| NEW-32 | BMAD and bmalph integration following the `core-service` convention | 19 |
| NEW-33 | Architecture overview with decision records, stating the statelessness assumptions | 20 |
| NEW-34 | Troubleshooting guide in the repository | 20 |
| NEW-35 | Environment-variable reference with the PHP-to-Rust name mapping | 20 |
| NEW-36 | Make-target reference with the PHP-to-Rust name mapping | 20 |
| NEW-37 | Operations notes: migrations, rollback and a health-probe runbook | 20 |
| NEW-38 | A guide to creating a service from the template | 20 |
| NEW-39 | A committed `.editorconfig` | 20 |
| NEW-40 | A committed dev container definition | 20 |
| NEW-41 | A request collection for the endpoints | 20 |
| NEW-42 | Documented extension seams: a Redis adapter through the cache port, and tracing and metrics | 20 |

---

## 7. Risks and open questions

### 7.1 Technical risks

| Id | Risk | Effect | Mitigation |
| --- | --- | --- | --- |
| R-01 | All evidence is on Rust 1.97.0; the template pins 1.99.1. The `pedantic` and `nursery` lint groups gain lints with each release | New lint findings or tool incompatibilities on the first build | The first implementation story re-runs every gate configuration on 1.99.1; later bumps are one deliberate pull request because the toolchain is pinned |
| R-02 | Core crates are pre-1.0 or in transition: axum 0.8 (0.9 with breaking changes is in progress upstream), sqlx 0.9.0 (the only release for six months; its repository is moving to a new organisation), utoipa 6.0.0 (seven weeks old, one primary maintainer), async-graphql 7.2.1 (8.0 has been a release candidate since January 2026), cucumber 0.23 (breaking minor releases) | Breaking migrations in services created from the template | Each crate sits behind a port or in one adapter module; `Cargo.lock` is committed; Dependabot proposes bumps; the contract gates catch behavioural drift |
| R-03 | LocalStack 3.4.0 is frozen: no fixes, a 1.71 GB image, availability on Docker Hub outside our control | The integration and health tests lose their emulator | ElasticMQ 1.7.1 was verified as an SQS-only contingency; it does not cover the load-test dry run, which needs EC2, S3, IAM and STS |
| R-04 | 100% line and function coverage with zero surviving mutants. A full mutation run is estimated at 15 to 45 minutes for a few hundred mutants | Slow pipelines; pressure to add exclusions | Diff-scoped mutation on pull requests, sharded full run on `main` and on a schedule (scope 8.5); exclusions only in protected configuration |
| R-05 | The Bats job is estimated at 8 to 15 minutes warm and 15 to 20 minutes cold, because each negative test recompiles a crate | The slowest pull-request job after mutation testing | Negative tests chosen to keep the job within about 20 minutes (scope 8.6); fixture workspaces where possible |
| R-06 | Three decisions were not exercised by any spike: the read-only broker probe, the report of all failing dependencies, and the layer model of scope 8.2 (the spike's `architecture.toml` let infrastructure depend on application) | Rework if an assumption fails | Scheduled early in implementation; the architecture checker has its own tests and sits under the mutation gate |
| R-07 | Workflows were linted, never executed; the push part of the release flow was not run; Compose merge tags were verified on one Compose version only | The first pull request is the first real test of CI | The bootstrap pull request must pass every job; the release workflow exits cleanly when its secrets are absent |
| R-08 | No secrets scanner and no image scanner was evaluated | Tool choice without spike evidence | The architect chooses; each tool gets a Make target or a documented reason for being CI-only (scope 8.5) |
| R-09 | The architecture checker is template-owned code (about 730 lines in the spike) that reaches downstream services through sync | A defect in it weakens the boundary gate everywhere | It stays under the same test, coverage and mutation gates as the service (scope 8.2) |
| R-10 | `axum::serve` has no header-read timeout or connection limit, and no proxy ships | Unsafe if a team exposes the binary directly | Stated in the README and the operations notes |
| R-11 | GraphQL errors are `200` responses with `errors[]`, not problem documents | Two error shapes in one service | Correct for GraphQL over HTTP; documented |
| R-12 | Job-time figures for the real service are projections from small spikes | Time budgets may be missed | Measured in the first CI runs; caches saved from `main` |
| R-13 | The AWS load-test runner cannot be exercised without AWS credentials, and the PHP default instance (`t2.micro`, 1 vCPU, 1 GiB) cannot build a Rust release build in reasonable time | Script fixes are verified only against the LocalStack dry run | The dry run is covered by a Bats test; the default instance must have at least 4 GiB of memory |
| R-14 | The two reports disagree on Dependabot and the toolchain file: the quality-gates report says Dependabot does not bump `rust-toolchain.toml`; the delivery report found a `rust-toolchain` ecosystem in the options reference (documented, not observed) | The pinned toolchain may go stale without notice | The ecosystem entry ships (scope 8.9), and the operations notes name an owner for a manual bump |

### 7.2 Process risks

- **Scope size.** 196 PHP capability ids plus 42 additions for a first release. The catalog gives traceability; anything not listed is out of scope (scope principle 7).
- **Catalog timing.** The capability catalog was written before the quality-gates and testing-delivery reports existed, so several of its notes say "architect". `scope-decisions.md` section 8 and section 4 of this digest settle most of them; the architecture document must close every remaining one explicitly.
- **Surrogate decisions.** The run is autonomous. `scope-decisions.md` was recorded by the orchestrating agent on the user's behalf; no maintainer has confirmed it. Section 7.4 lists what maintainers should confirm.
- **Parsed artifacts.** `bmalph implement` parses the PRD, architecture, epics and readiness documents; a heading or wording error there stops the implementation loop (scope section 7).
- **Untracked evidence.** The raw reports and spike sources are local. What implementers need from them (versions, digests, SHAs, pitfalls, overrides) is in this digest; the configuration files that passed must be reproduced in the architecture document.

### 7.3 Organisational prerequisites

| Prerequisite | Needed for | Behaviour when absent |
| --- | --- | --- |
| Secrets `VILNACRM_APP_ID`, `VILNACRM_APP_PRIVATE_KEY` | Release commit, tag and GitHub Release; App-flavour template sync | The release workflow exits cleanly and no release is cut; the sync job does not run |
| GitHub App permissions: contents write, pull requests write, workflows write; allowed to push the release commit to `main` | Release; syncing workflow files downstream | Release push or sync of workflow files is rejected |
| Secret `PERSONAL_ACCESS_TOKEN` | PAT-flavour template sync | That workflow does not run |
| Secret `CODECOV_TOKEN` | Coverage upload to the external service | Upload skipped; the in-repository floor still gates |
| Labels `approved-breaking-change`, `template_sync`, `bug`, `enhancement` | Approving an intentional contract break; sync pull requests; issue templates | A breaking change cannot be approved; sync pull requests are unlabelled |
| Required status checks on `main` | Making the gates binding | Gates report but do not block; rulesets and CODEOWNERS are follow-ups (scope 3.2) |
| GitHub code scanning | CodeQL results and SARIF uploads | Free for public repositories; private downstream repositories need GitHub Code Security |
| CodeRabbit App installation | `.coderabbit.yaml` | The file has no effect |
| AWS account and credentials | `make aws-load-tests` outside local mode | Only the LocalStack dry run works |

The secrets `GPG_PRIVATE_KEY` and `GPG_PASSPHRASE` used by PHP are no longer needed.

### 7.4 Open questions for the maintainers

Each question states what the template does until a maintainer answers.

1. **GitHub App credential.** `actions/create-github-app-token` v3 deprecates the `app-id` input in favour of a client id. Will the organisation add a client-id secret? Until then: the existing `VILNACRM_APP_ID` secret is used (scope 8.9).
2. **Release flow.** Direct release commit on `main` (PHP behaviour) or reviewed release pull requests? The answer decides whether the App must bypass branch protection. Until then: direct commit with cocogitto.
3. **LocalStack.** Is a frozen, unpatched, tokenless emulator acceptable in the long term, or will the organisation supply an auth token? Until then: `3.4.0`.
4. **Health failure status.** `500` is kept for wire parity; `503` is the conventional status for an unavailable dependency. A change would apply to every VilnaCRM template and to the shared feature file. Until then: `500`.
5. **Failure detail in production.** The Rust service returns the three stable messages in every environment. Whether the PHP service hides exception messages in its production mode was not verified. Until then: the same detail everywhere.
6. **Image publishing (audit P7).** Which registry and signing identity? Until then: no image is published, and the EC2 load-test runner builds from source.
7. **Coverage service.** Keep the upload to the external coverage service? Until then: a non-gating step that runs only when the token exists.
8. **Governance follow-ups.** Who owns CODEOWNERS, rulesets, auto-merge, the stale bot and OSSF Scorecard, all deferred by scope 3.2 and section 6? Until then: documented as follow-ups.
9. **Toolchain and major-version migrations.** Who bumps the pinned toolchain if Dependabot does not, and who owns the axum 0.9, async-graphql 8.0 and sqlx 0.10 migrations across services created from the template? Until then: the template repository's maintainers, through template sync.
10. **Private downstream repositories.** Where GitHub Code Security is not licensed, are clippy and cargo-deny SARIF enough? Until then: the CodeQL job is the only part that needs the licence; the other gates do not.
11. **Planning files downstream.** Should the tracked BMAD and bmalph files be synchronised into services? Until then: the architect lists them in the sync ignore decision.
12. **Structurizr wording.** `scope-decisions.md` 3.1 says Structurizr Lite stays, 8.7 says the unified image replaces it because Lite is end of life. This digest follows 8.7; maintainers should confirm.

---

## 8. Sources

**Planning inputs (tracked with this digest under `specs/rust-service-template-bootstrap/`)**

- `scope-decisions.md`: binding scope, format and technology decisions, sections 1 to 8.
- `capability-catalog.md`: 161 rows for 196 PHP capability ids, contracts to preserve, 48 threshold rows, 150 defect lines, P1 to P28, NEW-01 to NEW-42.
- `product-brief.md`: constraints, assumptions A-1 to A-10, risks.

**Raw research reports (local, untracked; dated 2026-11-10)**

- `php-app-code.md` (705 lines), `php-tests-and-quality.md` (821), `php-docker-make-load.md` (747), `php-ci-github-contracts.md` (906), `php-ai-docs-audit.md` (1,003).
- `rust-http-api-stack.md` (1,159 lines), `rust-data-cache-messaging.md` (1,019), `rust-quality-gates.md` (1,499), `rust-testing-delivery.md` (1,436).
- Spike sources for the four Rust reports, in a local scratch directory.

**Primary material**

- `VilnaCRM-Org/php-service-template` at commit `3c1772104f3c0a53710d7159209573a6bbc044a4` (`chore(release): v1.11.0 [skip ci]`, 2026-04-12), read-only.
- `php-enterprise-quality-audit-2026-07.md` (1,475 lines, 2026-07-02): 94 attributes, problems P1 to P28, issues #176 to #203.
- crates.io registry and API (`cargo info`, `cargo search`), queried 2026-11-10; crate sources, changelogs and build scripts under the local cargo registry.
- Docker Hub tag API; npm and PyPI registries; `git ls-remote --tags` for every action and tool repository in section 4.4.

**Upstream documentation cited by the reports**

- axum README: <https://github.com/tokio-rs/axum>
- oasdiff OpenAPI 3.1 support notes: <https://github.com/oasdiff/oasdiff/blob/main/docs/OPENAPI-31.md>
- LocalStack announcements: <https://blog.localstack.cloud/the-road-ahead-for-localstack/> and <https://blog.localstack.cloud/localstack-for-aws-release-2026-03-0/>
- AWS SDK for Rust endpoint configuration: <https://docs.aws.amazon.com/sdk-for-rust/latest/dg/endpoints.html>
- `taiki-e/install-action` tool list: <https://github.com/taiki-e/install-action/blob/main/TOOLS.md>
- `cargo-llvm-cov` 0.9.1 README: <https://raw.githubusercontent.com/taiki-e/cargo-llvm-cov/v0.9.1/README.md>
- CodeQL for Rust: <https://github.blog/changelog/2025-10-14-codeql-scanning-rust-and-c-c-without-builds-is-now-generally-available> and <https://docs.github.com/en/code-security/reference/code-scanning/codeql/codeql-queries/rust-built-in-queries>
- Rust coverage attribute tracking issue: <https://github.com/rust-lang/rust/issues/84605>
- GitHub runner image manifest: <https://github.com/actions/runner-images/blob/main/images/ubuntu/Ubuntu2404-Readme.md>
- Dependabot options reference: <https://docs.github.com/en/code-security/dependabot/working-with-dependabot/dependabot-options-reference>
- `actions/create-github-app-token`: <https://github.com/actions/create-github-app-token>; `tibdex/github-app-token` deprecation notice: <https://github.com/tibdex/github-app-token>
- `AndreasAugustin/actions-template-sync` README: <https://github.com/AndreasAugustin/actions-template-sync>
- cucumber-rs README and changelog: <https://github.com/cucumber-rs/cucumber>
- Structurizr local: <https://docs.structurizr.com/local>
- cargo-chef README: <https://github.com/LukeMathWalker/cargo-chef>

**Research status:** complete for the analysis phase. The next BMAD commands consume this digest: `create-prd`, `create-architecture`, `create-epics-stories`, `implementation-readiness`.
