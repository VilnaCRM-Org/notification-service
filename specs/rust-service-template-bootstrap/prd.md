---
stepsCompleted:
  - step-01-init
  - step-02-discovery
  - step-02b-vision
  - step-02c-executive-summary
  - step-03-success
  - step-04-journeys
  - step-05-domain
  - step-06-innovation
  - step-07-project-type
  - step-08-scoping
  - step-09-functional
  - step-10-nonfunctional
  - step-11-polish
  - step-12-complete
  - step-e-01-discovery
  - step-e-02-review
  - step-e-03-edit
  - step-e-04-complete
inputDocuments:
  - _bmad-output/planning-artifacts/scope-decisions.md
  - _bmad-output/planning-artifacts/product-brief.md
  - _bmad-output/planning-artifacts/research.md
  - _bmad-output/planning-artifacts/research/capability-catalog.md
  - _bmad-output/planning-artifacts/research/php-app-code.md
  - _bmad-output/planning-artifacts/research/php-tests-and-quality.md
  - _bmad-output/planning-artifacts/research/php-docker-make-load.md
  - _bmad-output/planning-artifacts/research/php-ci-github-contracts.md
  - _bmad-output/planning-artifacts/research/php-ai-docs-audit.md
  - _bmad-output/planning-artifacts/research/rust-http-api-stack.md
  - _bmad-output/planning-artifacts/research/rust-data-cache-messaging.md
  - _bmad-output/planning-artifacts/research/rust-quality-gates.md
  - _bmad-output/planning-artifacts/research/rust-testing-delivery.md
documentCounts:
  briefCount: 1
  researchCount: 11
  brainstormingCount: 0
  projectDocsCount: 1
workflowType: 'prd'
workflow: 'edit'
classification:
  projectType: developer_tool (service template) with an api_backend runtime
  domain: general
  complexity: low domain complexity, high breadth
  projectContext: greenfield
date: 2026-11-10
lastEdited: '2026-11-12'
editHistory:
  - date: '2026-11-10'
    changes: >-
      Amendment from the architecture validation round (verified fact: an image reference with a digest
      ignores its tag, so `postgres:${DB_VERSION}-alpine@sha256:...` cannot honour `DB_VERSION`):
      FR125(b) and NFR13 exempt the PostgreSQL image of the compose stacks from the digest pin. No id changed.
  - date: '2026-11-10'
    changes: >-
      edit-prd after two validation reviews (70 findings). Added FR121 to FR126 and gates G29 to G31;
      lettered the clauses of bundled requirements; defined protected items, the mutation scope, the
      400/422 boundary, the GraphQL contract, start-up and shutdown behaviour, the configuration table,
      target classes and supported platforms; gave every NFR a verification method; added the sections
      Target Users and Risks and Mitigations and an NFR digest for the bmalph context extract.
  - date: '2026-11-11'
    changes: >-
      Amendment from the FR/NFR review gate (iteration 1): NFR40 records two accepted exceptions to its cache
      clause, the `surface` Bats group and the weekly `devcontainer-build` job (finding F-10 of the readiness
      review). No id changed.
  - date: '2026-11-11'
    changes: >-
      Amendment from the FR/NFR review gate (iteration 1): FR98(a) counts a `BREAKING CHANGE:` footer
      whatever the type of the commit, a footer being a line of the footer section at the end of the message;
      NFR27 and K-06 count a third digest line accepted by decision, the unchanged PHP Spectral ruleset
      (catalog note CI:D-28), and the delegated decision on the Spectral response rules is marked as taken.
      No id changed.
  - date: '2026-11-12'
    changes: >-
      Amendment from the FR/NFR review gate (iteration 2): NFR40 records a third accepted exception, for its
      rerun clause: the third-party crates that a pull-request mutation shard compiles for packages outside
      the saver's selection (architecture-delivery.md, open item 5 (3) (the fourth since pass D6); readiness F-10). No id changed.
  - date: '2026-11-12'
    changes: >-
      Amendment from the FR/NFR review gate (iteration 2): NFR40 records a further accepted exception to its
      cache clause, the weekly `clean-bootstrap-ci` job, which FR95 and NFR34 require to start without a
      restored cache (the specific requirement wins); it is the third exception in architecture-delivery.md,
      open item 5, and the rerun exception of the mutation shards becomes the fourth (readiness F-10). No id
      changed.
  - date: '2026-11-12'
    changes: >-
      Amendment from the FR/NFR review gate (iteration 3): FR66(d) admits as CI-only steps the scheduled-only
      steps that FR95, FR118 and NFR35 require: the full sharded mutation run, the build of the development
      container with the checks run in it, and the stop and the timed start of the development stack in the
      clean-clone job `clean-bootstrap-ci`; each is listed with its reason in the Make-target reference
      (architecture-delivery.md, reading 11; readiness, row FR66(d)). No id changed.
  - date: '2026-11-12'
    changes: >-
      Amendment from the FR/NFR review gate (iteration 5): FR100(b) records how the pinned sync action meets
      it: a run for the template commit of the open sync pull request updates that pull request in place, and a
      run for a newer template commit opens that commit's pull request and closes the older open sync pull
      requests in the same run (`is_pr_cleanup`), so no two are open at once; J5 step 2 says the same. No id
      changed.
  - date: '2026-11-12'
    changes: >-
      Wording fix from the review of pull request #2: the rerun clause of NFR40 (0 third-party dependencies
      recompiled) applies apart from its own fourth exception, the crates a pull-request mutation shard compiles
      for packages outside the saver's selection. No id changed.
  - date: '2026-11-12'
    changes: >-
      Amendment from the review of pull request #2: FR100(b) keeps the branches of the older sync pull
      requests that a run closes (`is_keep_branch_on_pr_cleanup`), so commits a maintainer pushed to them stay
      on them; they are not carried to the new sync branch. No id changed.
  - date: '2026-11-12'
    changes: >-
      Amendments from the second review of pull request #2: NFR16 lets an inline directive of shellcheck,
      hadolint, yamllint, markdownlint, ESLint or zizmor, which have no item scope, apply to one file when it
      names one rule and carries a reason (readiness, reading NFR16, FR125(g)); the rerun clause of NFR40
      applies apart from all four of its recorded exceptions. No id changed.
author: BMad
---

# Product Requirements Document - rust-service-template

**Author:** BMad
**Date:** 2026-11-10

Produced by the BMAD `create-prd` workflow in an autonomous run and revised by `edit-prd` from two validation reviews. Stakeholder answers come from `scope-decisions.md` (binding; it prevails on any difference), the product brief and the research files listed in the front matter. Decisions this PRD takes where those inputs are silent are listed under "Assumptions and Decisions".

Ids used in traces: `CAP-*` and `NEW-*` are capability ids and `P1` to `P28` are audit problems, all defined in `research/capability-catalog.md`. `K-01` to `K-18` are the indicators of the product brief. `G1` to `G31` are the gates of the gate list in "Non-Functional Requirements". This PRD is the capability contract: later stages design and build only what it lists.

## Executive Summary

`rust-service-template` is the repository a VilnaCRM team copies to start a Rust backend service. It is the Rust counterpart of `VilnaCRM-Org/php-service-template` and gives a new service the platform concerns it needs before its first feature: a running HTTP service (REST under `/api`, GraphQL, health and liveness endpoints), a DDD + CQRS + hexagonal layout with one real example slice, PostgreSQL persistence, a cache port and an SQS client, container images and compose stacks, a Makefile command surface, quality gates, contract checks, load tests, CI workflows, release and template-sync automation, documentation, and guidance for AI coding assistants.

The problem has two parts. A team that starts from an empty Cargo project rebuilds that surface by hand; the PHP equivalent is 196 catalogued capabilities, 68 Make targets, 20 workflows and 50 command-line tests. And the PHP template cannot be ported file by file, because its execution has 188 recorded defects and 28 audit problems: `make ci` cannot pass, several gates cannot fail, the architecture guard leaves 14 of 32 classes unchecked, the GraphQL schema is accidental, and the assistant guidance contradicts itself.

The first release delivers parity of capability and intent with the PHP template in idiomatic Rust. Language-neutral contracts stay identical: URL paths, status codes, port `8081`, snapshot paths under `.github/`, capability-named Make targets, the K6 load kit, the commit convention, and the release and sync flows. Known defects are not replicated.

The users are backend engineers, AI coding assistants, platform maintainers and reviewers (see "Target Users").

Success is judged from the repository and its CI: a clean clone reaches a green `make ci` after `make bootstrap` with no manual step, every pass condition of every gate fails on a planted violation, every PHP capability id is delivered, replaced or listed as out of scope with a reason, and contract snapshots are byte-stable.

### What Makes This Special

1. **Contract compatibility with the PHP template.** The PHP health feature file, the PHP Spectral ruleset, the Schemathesis flags, the K6 kit, 42 Make target names and the `approved-breaking-change` and `template_sync` labels are reused unchanged, so shared tooling works on services in either language.
2. **Every gate is real.** Each gate has a threshold, a CI check and an automated negative test for every pass condition, and a Make target unless the gate list marks it CI-only. Coverage has an in-repository floor; mutation testing fails on any surviving mutant.
3. **Structural architecture enforcement.** Rules follow layer position: a new bounded context is covered without editing a protected rule file, no code is unassigned, and one context cannot reach into another except through the shared kernel.
4. **Working exemplars.** Every pattern the guidance names (command path, query path, event path, repository adapter, migration, validation) exists as tested code in `CompanySubdomain/SomeModule`.
5. **Safe to synchronise.** Names derive from one project-name variable, sync workflows have an ignore list and do nothing in the template itself, and workflows skip cleanly when organisation secrets are absent.
6. **Guidance that is checked.** One verification list equal to the gate order of `make ci`, a documentation-drift check, a protected-configuration guard that runs in CI and a committed assistant settings file.

## Target Users

- **Backend engineer** who starts or maintains a Rust service created from the template. Needs a verified baseline on the first day, one command per task, and exemplars to copy.
- **AI coding assistant** that receives tasks in such a repository and works with files and shell commands only. Needs one entry file, commands and paths that exist, and gates it cannot weaken.
- **Platform maintainer** who owns both templates and delivers fixes to services through template sync. Needs sync-safe files, bounded CI cost and workflows that do not fail without organisation secrets.
- **Reviewer or tech lead** who approves pull requests. Needs every gate reported under its own name and contract changes visible as snapshot diffs.

## Project Classification

| Attribute | Value |
| --- | --- |
| Project type | Developer tool (a template repository consumed by copy and by template sync) whose runtime deliverable is an API backend. Both rows of the BMAD project-type table apply; see "API Backend and Developer Tool Specific Requirements". |
| Domain | General software platform (internal developer platform). No regulated domain; the BMAD domain step adds no compliance section. |
| Complexity | Low domain complexity; high breadth: 185 PHP capability ids to deliver plus 42 additions. |
| Project context | Greenfield repository. The reference system is the PHP template, used as a capability source, not as code to migrate. |
| Innovation analysis | Not applicable. The product is a parity port; the BMAD innovation step adds no section. |
| Planning format | `bmalph implement` parses this file. The second-level headings `Executive Summary`, `Product Scope`, `Functional Requirements` and `Non-Functional Requirements` are fixed (scope-decisions section 7). bmalph copies only the first 5,000 characters of `Executive Summary`, `Success Criteria`, `Product Scope`, `Target Users` and `Non-Functional Requirements` into the implementation context, so each of these sections is kept within that length or opens with its most important content. |

## Success Criteria

### User Success

- **Backend engineer.** Reaches a green `make ci` from a clean clone with no help and no undocumented step (K-01, K-02); adds a bounded context with one command and edits no protected item (K-10); when a check fails, runs the same Make target locally and gets the same result (K-04).
- **AI coding assistant.** Finds one entry file and one verification list; every command and path the guidance cites exists (K-12); every pattern it is told to follow has an exemplar; an edit to a protected item is blocked locally and fails in CI.
- **Platform maintainer.** A template change reaches downstream repositories as a labelled pull request that overwrites no service-owned file and needs no workflow edit (K-13); workflows do not fail when organisation secrets are absent (K-14); CI time stays within budget (K-11).
- **Reviewer or tech lead.** Each gate reports under its own name and can fail (K-03); contract changes arrive as snapshot diffs in the same pull request; a breaking change fails until the `approved-breaking-change` label records the decision.

### Business Success

- **BO-1 Parity.** A Rust service at VilnaCRM starts with the same platform capabilities as a PHP service (K-05, K-08).
- **BO-2 Same-day start.** A new service goes from clone to a verified baseline in one working session (K-01, K-02).
- **BO-3 One platform, two languages.** Language-neutral contracts are identical in both templates (K-08, K-13).
- **BO-4 Trustworthy automation.** A green pipeline means the change meets every stated rule, for human and assistant authors alike (K-03, K-04, K-09, K-12).
- **BO-5 Audit problems closed.** Each of P1 to P28 is delivered, partly delivered with the remainder recorded, or declared out of scope; see "Traceability".

### Technical Success

- `make ci` passes after `make bootstrap` on a clean checkout, and CI runs it or its parts; the two differ only by the CI-only steps of FR66(d).
- No gate exists without a threshold, a CI check and a negative test; every gate has a Make target except those the gate list marks CI-only, each with its reason.
- Everything shipped is used: no interface without an implementation, no configuration file that no command reads, no dependency nothing imports, no test without an assertion.
- Contract snapshots are generated deterministically, need no running stack, and are compared with the committed files in CI.
- Nothing depends on the template's repository name, and nothing secret-shaped is committed.

### Measurable Outcomes

| Id | Outcome | Target | Requirement |
| --- | --- | --- | --- |
| K-01 | Clean clone to green | exit 0 on the release commit and on every scheduled run without a new external finding or outage; zero manual steps | NFR34 |
| K-02 | Time to a verified baseline | bootstrap at most 15 min, `make ci` at most 45 min, `make start` at most 60 s | NFR35 |
| K-03 | Every gate can fail | 100% of gate pass conditions have a negative test | NFR21 |
| K-04 | Local and hosted CI agree | 0 unexplained differences | NFR22 |
| K-05 | Parity coverage | 196 of 196 PHP ids mapped; 42 of 42 additions delivered | NFR27 |
| K-06 | Defects not replicated | 0 replicated, apart from the 3 accepted by decision | NFR27 |
| K-07 | Deterministic snapshots | 20 of 20 generations identical; 0 bytes from committed | NFR7 |
| K-08 | Wire parity with PHP | 4 of 4 reused scenarios pass; 0 lint findings; 0 failed conformance checks | NFR24, NFR30 |
| K-09 | Test strength | 100% line and function coverage; 0 surviving mutants; 0 undefined steps | NFR19, NFR20, NFR30 |
| K-10 | Architecture | 0 violations; 0 unassigned crates; 0 protected items edited for a new context | NFR18 |
| K-11 | CI time | pull request at most 20 min warm, 40 min cold (mutation check apart), 5 min documentation-only | NFR39 |
| K-12 | Guidance is true | 0 dangling commands; exactly 1 verification list | NFR43 |
| K-13 | Sync safety | 0 occurrences of the repository name outside three definitions | NFR38 |
| K-14 | Clean degradation without secrets | 0 runs failed by a missing secret | NFR8 |
| K-15 | No dead scaffolding | 0 unused dependencies, unused configuration files or assertion-free tests | NFR23 |
| K-16 | Workflow hygiene | 100% of actions pinned by SHA; 100% of workflows declare permissions | NFR13, NFR14 |
| K-17 | Security gates | 0 advisories, policy violations, secret findings or HIGH/CRITICAL image findings | NFR9, NFR10, NFR11 |
| K-18 | Smoke load on pull requests | p99 at most 500 ms; checks above 99%; results uploaded | NFR1, FR82 |

The release is complete when, on the release commit on `main`: every indicator meets its target (a time or load budget may be replaced by a measured value with a recorded reason, see NFR1, NFR35 and NFR39); every pull-request check and the scheduled run are green; each journey J1 to J8 can be followed from the repository's own documentation; and the parity matrix and the P1 to P28 disposition are tracked in the repository.

## Product Scope

### MVP - Minimum Viable Product

The MVP is the whole first release: a platform MVP whose value is that every part is present and verified, so no feature group can be dropped. The eight groups restate `scope-decisions.md` section 3.1.

| Group | Content | Requirements |
| --- | --- | --- |
| F1 Service runtime | One binary, plain HTTP on port `8081`; REST under `/api` with an OpenAPI 3.1 document and docs UI; GraphQL; problem responses; `GET /api/health`, `GET /ping`; JSON logs, request id, security headers, graceful shutdown, timeouts; typed configuration, `check-config` | FR1-FR18, FR126 |
| F2 Example bounded context | `CompanySubdomain/SomeModule`: create, read by id and list `ExampleApiResource` over REST and GraphQL; one audit subscriber; no update or delete | FR19-FR24 |
| F3 Shared kernel | Aggregate root, domain events, command, query and event buses with in-process adapters, UUID value object and generator | FR25-FR29 |
| F4 Persistence, cache, messaging | PostgreSQL repository adapter, reversible SQL migrations, `migrate` and rollback commands, committed query metadata; cache port with in-process adapter; SQS client against LocalStack | FR30-FR35, FR121 |
| F5 Architecture | Three layers per module, framework-free Domain, structural boundary gate, crate isolation, scaffolding, Structurizr model validated in CI | FR36-FR39, FR119, FR120, FR122 |
| F6 Quality gates | The 31 gates of the gate list: each has a threshold, a CI check and negative tests, and a Make target unless marked CI-only | FR40-FR59, FR71-FR76, FR83, FR84, FR123-FR125, NFR16-NFR27 |
| F7 Delivery | Images and compose stacks, Makefile command surface, CI workflows, commit convention, release, Dependabot, template sync, K6 load kit with the AWS runner | FR60-FR70, FR77-FR82, FR85-FR103 |
| F8 AI guidance and documentation | `AGENTS.md`, `CLAUDE.md`, skills, drift check, assistant settings, BMAD and bmalph integration, README and in-repository documents | FR104-FR118 |

### Out of Scope

Identical to `scope-decisions.md` section 3.2; the catalog's `DROP` rows are listed in "Traceability".

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

Anything not listed as in scope is out of scope for this release (scope principle 7).

### Growth Features (Post-MVP)

A deferred item starts when its precondition exists.

| Deferred item | Precondition |
| --- | --- |
| Tracing and metrics (OpenTelemetry exporters, Prometheus endpoint) | Observability backend chosen; the seam is documented now (FR117) |
| Authentication and authorisation | Identity-provider and token-format decision |
| Rate limiting | Edge policy that leaves load and fuzz gates unaffected |
| Published container image with SBOM, signature and provenance | Registry and signing identity |
| Orchestrator manifests and autoscaling examples | Target platform decision |
| Asynchronous consumer, outbox, idempotency keys, chaos tests, kill switches | A first service that needs them |
| Redis adapter | A service that needs a shared cache |
| CODEOWNERS, rulesets, auto-merge, stale bot, OSSF Scorecard | Governance decisions |
| Multi-architecture images | Runner and registry support |
| Stored performance baselines and trend comparison | Storage location for results |
| API versioning and deprecation policy, upgrade guide, code of conduct, flaky-test policy | Maintainer decisions (outside scope 3.1) |

### Vision (Future)

The template stays a baseline, not a framework. Both templates converge on shared language-neutral files (Spectral ruleset, K6 kit, release and sync guides, portable skills), so one change serves both.

## Risks and Mitigations

| Risk | Effect | Mitigation |
| --- | --- | --- |
| Scope is large for a first release (185 PHP ids plus 42 additions) | Late or partial delivery | Every requirement traces to the catalog; epics follow F1 to F8; anything unlisted is refused |
| 100% line coverage and zero surviving mutants | Slow pipelines; pressure to add exclusions | Mutation runs on changed code in pull requests and in full on `main` (FR59, NFR20); every exclusion is an entry with a reason in a protected record (NFR19, NFR20) |
| Mutants in code that only real services can exercise | Survivors by construction, or a mutation run that needs the stack | The architect chooses the judging suites; source they cannot observe is listed in the mutation scope record with a reason (FR59) |
| One negative test per pass condition | A large suite; each Rust-level negative test may recompile code; the Bats job exceeds 20 min | Per-rule tests of template-owned checkers run at the checker's test level; Make-level tests are per tool or policy (NFR21, NFR39) |
| LocalStack community tags are frozen | No fixes; future incompatibility | Pinned tag (NFR13); an alternative SQS emulator was verified by the research as a contingency |
| The AWS load-test runner cannot be exercised without AWS credentials | Fixes verified only against the LocalStack dry run | Dry run covered by a Bats test (FR85); the limitation is stated in the load-testing documentation |
| Container registry rate limits (observed once during research) | Intermittent CI failures | Image pinning and layer caching (NFR13, NFR40); an outage does not count against K-01 (NFR34) |
| The pinned toolchain (`1.99.1`) is newer than the one the research spikes ran on (`1.97.0`) | A version-sensitive finding may not hold | Re-verified during implementation (scope-decisions 8.1); every gate has negative tests (NFR21) |
| A new advisory or image vulnerability appears with no code change | The scheduled run and open pull requests turn red | Expected behaviour (NFR34); exceptions are dated entries with a reason (NFR9, NFR11) |
| Protected items inside files that change often (workspace manifest, load configuration) | An approval label on routine changes, or an unguarded lint policy | Section-level protection (FR44(b)); only the named sections need the label |
| Maintainers have not confirmed the surrogate decisions (platform list, security-policy values, squash merges) | Rework after review | Each is recorded as an assumption (A-3, A-7, A-8) |

## User Journeys

Persona names are illustrative. Targets whose names `scope-decisions.md` does not fix are described by capability.

**J1. First hour with the template (Olena, backend engineer).**

1. Creates a repository from the template and clones it.
2. Runs `make doctor`; it names each missing host prerequisite and says whether her platform is verified in CI.
3. Runs `make bootstrap`; it prepares the clone and ends with the stack healthy.
4. Checks `GET http://localhost:8081/ping` (`200`, `pong`), `GET /api/health` (`204`), `/api/docs`, `/api/graphql` and the diagrams in the Structurizr viewer.
5. Runs `make ci`; it ends with `CI checks successfully passed!`.
6. Follows the guide to creating a service: sets the project-name variable and the package identity in the workspace manifest, and edits no workflow.

Value moment: a green `make ci` on an unmodified clone. Failure path: a missing prerequisite is reported by name in step 2, not as a tool error in step 5.

**J2. Adding a bounded context (Olena).**

1. Runs the scaffolding command with a context name and a module name.
2. The generated module has Domain, Application and Infrastructure, compiles and passes the architecture gate; no protected item was edited.
3. Copies the command, query and event paths from `CompanySubdomain/SomeModule`, adds a reversible migration and applies it with `migrate`. The running development service rebuilds when she saves a source file.
4. A dependency from her Domain code on a framework, or on another context's internals, fails the architecture gate with a message naming both ends.
5. Adds the module's components to `workspace.dsl`; until she does, the model gate fails and names the missing components.

**J3. Changing an API (Olena, reviewed by Iryna, tech lead).**

1. Changes the handler and its types in code.
2. Runs `make generate-openapi-spec` and `make generate-graphql-spec` and commits the snapshots with the code.
3. If she forgets, CI fails because the committed snapshot differs from the generated one.
4. CI lints the OpenAPI document with the Spectral ruleset, runs Schemathesis against the production image and compares both contracts with `main`.
5. Runs `make smoke-load-tests`; the report is written to `tests/Load/results/`, and CI uploads the same report for the pull request.
6. A breaking change fails until Iryna adds the `approved-breaking-change` label; the check is evaluated again without a new commit, and the label is the record of the decision.

**J4. Fixing a failing gate (Olena or the assistant).**

1. The pre-commit hook has already run the format, lint and architecture checks; a later gate fails in CI and the failed result names that one gate.
2. She runs the Make target of that gate locally; it is the command CI ran.
3. The troubleshooting guide and the matching skill explain how to fix the code. Weakening the gate is not available as a fix: the protected-configuration guard fails in CI unless a maintainer adds the approval label.
4. For a failing test with random data, the printed seed reproduces the failure.

**J5. Receiving a template-sync pull request (Taras, platform maintainer, and a service owner).**

1. Taras merges an improvement into the template.
2. In a downstream repository the scheduled sync workflow opens a pull request labelled `template_sync`, or updates the one that is already open while the template's `main` has not moved; a run for a newer template commit opens the pull request of that commit and closes the older one. Files on the sync ignore list are untouched.
3. The pull request runs the same gates as any other change and needs no workflow edit, because nothing depends on the template's name. If it modifies a protected item, it needs the approval label like any other pull request.
4. The service owner reviews and merges. In the template repository itself the sync workflows do nothing.

**J6. An AI assistant taking a task.**

1. Reads `AGENTS.md`; `CLAUDE.md` delegates to it. Chooses a skill through the decision guide; each skill description states when to use it.
2. Finds the exemplar for the required pattern in the example slice.
3. Works through `make` targets; the committed settings file allows the gate, test and export targets without prompts, asks before destructive or billable targets, and blocks edits to protected items.
4. Runs the canonical verification list in `AGENTS.md`, which equals the gate order of `make ci`.
5. Closes with what was run, what was not run and why.

**J7. Operating a deployed service (Taras or the engineer on call).**

1. Sets the environment variables from the environment-variable reference and runs `check-config`; an invalid value is named before anything starts.
2. Runs `migrate` as its own step, then starts the service from the production image; the platform probes `GET /ping` for liveness and `GET /api/health` for dependency health. The service starts and answers `/ping` even while a dependency is down.
3. A dependency fails: `GET /api/health` returns `500` and names every failing dependency; the health-probe runbook maps each message to causes and checks; logs are JSON lines that carry the request id.
4. A release goes wrong: reverts the last migration with the documented rollback command.
5. The platform sends SIGTERM during a deploy: in-flight requests finish and the process exits 0.

**J8. Releasing and maintaining the template (Taras).**

1. Merges a pull request whose title follows `type(#issue): subject`; the title check has verified it.
2. The release workflow bumps the version, updates `CHANGELOG.md`, tags and creates a GitHub release without starting another CI run; a merge with no `feat`, `fix` or breaking commit makes no release; without the App secrets the workflow skips with a notice.
3. Dependabot opens grouped update pull requests for six ecosystems; they pass the same gates and cut no release. An update that modifies a protected item, such as the toolchain pin, needs the approval label.
4. The weekly scheduled run fails, and its job summary names the gate and the finding, when a new advisory appears or the clean-clone path breaks on `main`.

### Journey Requirements Summary

| Journey | Capabilities it needs | Requirements |
| --- | --- | --- |
| J1 | Prerequisite check, one-shot bootstrap, healthy stack, endpoints, aggregate verification, new-service guide, project-name variable | FR69, FR70, FR88, FR18, FR14, FR8, FR9, FR66, FR117, FR92, FR119 |
| J2 | Scaffolding, layered layout, architecture gate, exemplars, migrations, edit-rebuild loop, model validation | FR39, FR36-FR38, FR19-FR24, FR32, FR33, FR89, FR119, FR120, FR122 |
| J3 | Code-first contracts, export targets, snapshot equality, lint, conformance, breaking-change checks with approval label, load kit and smoke profile | FR71-FR76, FR24, FR77-FR82, FR84 |
| J4 | Git hooks, per-gate results and targets, protected-configuration guard, seeded tests, troubleshooting guide, skills | FR103, FR93, FR61, FR44, FR51, FR117, FR108 |
| J5 | Template sync with ignore list and self-guard, name independence, setup guides | FR100, FR92, FR101, FR44 |
| J6 | Entry files, skills, exemplars, settings file, verification list, drift check | FR104-FR111, FR19-FR24 |
| J7 | Configuration check, start without dependencies, explicit migrations and rollback, health and liveness contracts, logs, graceful shutdown, production image and compose reference, runbook | FR2-FR4, FR126, FR10, FR12, FR14-FR18, FR32, FR33, FR87, FR91, FR117 |
| J8 | Commit convention, release, Dependabot, scheduled run, clean skip without secrets | FR102, FR98, FR99, FR95, FR96 |

## API Backend and Developer Tool Specific Requirements

### Project-Type Overview

The product is a developer tool whose runtime deliverable is an API backend. As a developer tool its public surface is the Make command surface, the service command line, the configuration variables and the repository layout. As an API backend its public surface is the HTTP contract below. This section fixes both surfaces; the functional requirements refer to it.

### Endpoint Specification

| Method and path | Purpose | Success | Error responses | In OpenAPI |
| --- | --- | --- | --- | --- |
| `GET /ping` | Liveness; no dependency access | `200`, body `pong` | none of its own | no |
| `GET /api/health` | Dependency health | `204`, empty body | `500` | yes |
| `POST /api/example_api_resources` | Create an `ExampleApiResource` | `201`, JSON, `Location` header | `400`, `413`, `415`, `422` | yes |
| `GET /api/example_api_resources` | List one bounded page | `200`, JSON array | `400`, `422` | yes |
| `GET /api/example_api_resources/{id}` | Read one by id | `200`, JSON | `400`, `404` | yes |
| `GET /api/docs` | API documentation page | `200`, HTML, no redirect | none of its own | no |
| `GET` on sub-paths of `/api/docs` | Assets of the documentation page and the OpenAPI document it renders; the architect records the paths | `200` | `404` | no |
| `POST /api/graphql` | GraphQL operations | `200`, GraphQL response | `400`, `413`, `415`, `422` | no (GraphQL snapshot) |
| `GET /api/graphql` | Interactive explorer, `APP_ENV=dev` only | `200`, HTML | `405` in `test` and `prod` | no |

Every route can also return `500` (unhandled failure) and `503` (request over its time budget). Unknown paths return `404` and unsupported methods `405` with an `Allow` header. Every route of this table, the documentation sub-paths included, carries the headers of FR11 and `X-Request-Id`. No update or delete operation exists on the example resource.

### Data Schemas

- `ExampleApiResource`: `id` (UUID string), `name` (string), `description` (string).
- Create input: `name` (required, 1 to 255) and `description` (required, 1 to 1024). Length is counted in Unicode code points. A value that contains U+0000 is rejected with `422`; no other character is forbidden and white space is not trimmed. Unknown fields are rejected with `422`.
- List parameters: `page` (integer, first page `1`, default `1`) and `itemsPerPage` (integer, minimum `1`; default and maximum recorded by the architect, maximum at most 100). Items are in a total order by `id`; the architect records the direction.
- Problem document (RFC 9457): `type`, `title`, `status`, `detail`. `type` is `about:blank` unless the architect records a URI reference for an error class; `title` is the status phrase. A validation failure adds `violations`, an array whose entries have `propertyPath` (the wire name of the field) and `message`. A health failure adds `failures`, an array with one entry per failing dependency that carries its message of FR15, and its `detail` joins those messages.
- GraphQL: one object type for `ExampleApiResource` with the same three fields, one input type for creation, the two queries `exampleApiResource(id)` and `exampleApiResources(page, itemsPerPage)` and the mutation `createExampleApiResource(input)` (FR24).

### Error Codes

All error responses are `application/problem+json`.

| Status | Case |
| --- | --- |
| `400` | The body is not well-formed JSON; a path or query parameter cannot be parsed as its type (an id that is not a UUID, a page value that is not an integer) |
| `404` | Unknown route; `ExampleApiResource` not found |
| `405` | Method not supported on the route; the response carries an `Allow` header |
| `413` | Request body above the configured size limit |
| `415` | Missing or unsupported request media type |
| `422` | Well-formed input that violates the schema or a rule: missing or null required field, wrong JSON type, unknown field, length, forbidden character, page value outside its bounds, a JSON body on the GraphQL route that is not a GraphQL request |
| `500` | Health check failure (body lists the failing dependencies); unhandled failure or panic (no internal detail) |
| `503` | Request exceeded its time budget |

GraphQL operation errors are not in this table: they return HTTP `200` with the GraphQL `errors` member (FR24).

### Authentication Model and Rate Limits

None in this release. Every endpoint is unauthenticated and unthrottled; both are out of scope (see "Out of Scope"). The service emits no cross-origin headers; cross-origin policy, TLS and transport security headers belong to the platform edge.

### API Documentation and Versioning

- The OpenAPI document is version `3.1.0`, generated from code, committed at `.github/openapi-spec/spec.yaml` and rendered at `/api/docs`. The GraphQL schema is committed at `.github/graphql-spec/spec`.
- Paths carry no version segment (parity with the PHP template). Compatibility is enforced by the breaking-change gates and the `approved-breaking-change` label.
- A release never leaves a committed snapshot different from the generated one (FR73). The architect decides how `info.version` of the OpenAPI document is sourced under that constraint and records it (scope-decisions section 4).

### Language and Toolchain Matrix

| Element | Requirement |
| --- | --- |
| Language and build | Rust on the stable channel, edition 2024, built with Cargo; one exact version, `1.99.1` at the first release, pinned in `rust-toolchain.toml` and stated identically wherever else it appears (NFR13) |
| Database | PostgreSQL, `DB_VERSION` default `16` |
| Message broker | AWS SQS; LocalStack locally and in CI, pinned to the tokenless community tag `3.4.0` |
| Containers | Docker with the Compose plugin |
| CI | GitHub Actions on GitHub-hosted runners |
| Secondary ecosystem | JavaScript for the K6 scripts, with one npm manifest in `tests/Load`. JavaScript-based tools (load-script lint, GraphQL schema diff, Markdown lint, bmalph) run from pinned images or on Node.js as a host prerequisite (NFR37) |
| Host platforms | Linux x86_64 is verified in CI; other platforms as stated in NFR37 |

### Installation Methods

1. Create a repository from the template on GitHub (or receive it through template sync in an existing downstream repository).
2. `make doctor` to check host prerequisites.
3. `make bootstrap` to prepare the clone and start the stack.
4. Alternatively open the repository in the dev container (FR118), in which the toolchain targets run and the container engine is reachable, and run the same two targets.

### Command and Configuration Surface

Service commands (FR2). `migrate` and `check-config` are fixed names; the architect names the others.

| Command | Purpose | Needs dependencies |
| --- | --- | --- |
| serve | Run the HTTP service | none to start; database and broker to serve API requests (FR126) |
| `migrate` | Apply pending migrations | database |
| rollback | Revert the most recent migration | database |
| `check-config` | Validate the configuration of the serve command | none |
| liveness probe | Request `GET /ping` of the running service; used by the image health check | running service |
| OpenAPI export | Write the OpenAPI document | none |
| GraphQL export | Write the GraphQL schema | none |

Service configuration (FR3, FR4). "Required by" names the commands that exit non-zero when the variable is missing.

| Variable | Required by | Default | Validation |
| --- | --- | --- | --- |
| `DATABASE_URL` | serve, `migrate`, rollback, `check-config` | none | a PostgreSQL connection URL |
| `AWS_REGION` | serve, `check-config` | none | not empty |
| `AWS_ENDPOINT_URL_SQS` | none | unset: the regular AWS endpoint | an `http` or `https` URL |
| `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` | none | unset: the provider's default credential chain | both set or both unset |
| `APP_ENV` | none | `prod` | `dev`, `test` or `prod` |
| Listening port (name: architect) | none | `8081` | a port number |
| Log level, timeouts, retry limit, body limit, shutdown grace period (names: architect) | none | within the ceilings of NFR2, NFR4, NFR5 and PD-15 | typed value |

The export commands read no variable. The liveness probe reads only the listening port.

Compose-level variables with PHP names kept: `DB_VERSION`, `DB_NAME`, `DB_USER`, `DB_PASSWORD`, `DB_PORT`, `LOCALSTACK_PORT`, `STRUCTURIZR_PORT`, `APP_ENV` (FR90). Tooling variables with fixed names: `API_HOST`, `API_PORT`, `SCHEMATHESIS_URL`, `LOCAL_MODE_ENV`, `OPENAPI_DIFF_IMAGE`. Fixed Make targets are those of FR62 plus `doctor`, `bootstrap` and `mutation-tests`; fixed Make parameters are `scenario`, `runSmoke`, `runAverage`, `runStress`, `runSpike` and `base_ref`.

### Code Examples

The example slice is the code example of the template. Each pattern has exactly one exemplar:

| Pattern | Exemplar | Requirement |
| --- | --- | --- |
| Validated input and command path | Create `ExampleApiResource` | FR19, FR20 |
| Query path | Read by id; list with a bounded page | FR21, FR22 |
| Domain event and subscriber | `ExampleApiResourceCreated` and the audit subscriber | FR23, FR25, FR26 |
| Repository port and adapter, migration | `ExampleApiResource` persistence | FR31, FR32 |
| Second transport over the same use cases | GraphQL queries and mutation | FR24 |
| Pluggable infrastructure probe | Health probes | FR16, FR17 |

### Migration Guide

There is no code migration from PHP. The guide a PHP-experienced engineer needs is the mapping of names: PHP environment variables to Rust ones (environment-variable reference) and PHP Make targets to Rust ones (Make-target reference), both required by FR117. Downstream services receive later template changes through template sync (FR100); the sync guide lists the changes that do not propagate (FR101).

### Implementation Considerations

- Contracts marked "preserved" in the catalog section "Contracts to preserve" are binding; the functional requirements restate each value they depend on.
- Toolchain targets work in three places: the development container, a host with the pinned toolchain, and a GitHub Actions runner. Orchestration targets need the container engine wherever they run (FR65).
- Pull requests from forks and from Dependabot run with a read-only token and no secrets; every gate works under those conditions (FR96).
- Organisation secrets (`VILNACRM_APP_ID`, `VILNACRM_APP_PRIVATE_KEY`, `PERSONAL_ACCESS_TOKEN`, `CODECOV_TOKEN`) may be absent; affected steps skip with a notice (NFR8).
- No git submodules (FR67, NFR37).
- Contract, fuzz and load tests run against the production image, not a development build (FR76, FR77, FR84).

### Definitions

- **Source unit:** a workspace crate, the unit the architecture gate assigns to a layer.
- **Production source:** defined in NFR19.
- **Backing services:** the PostgreSQL and LocalStack services of the compose stack.
- **Healthy stack:** every service that declares a health check reports healthy, and `GET /ping` of the service returns `200`.
- **Secret-shaped value:** a value that matches a rule of the pinned rule set of the secrets-scanning gate (G9).
- **Blanket suppression:** a lint suppression that covers a whole crate, names a group of rules instead of one rule, or carries no reason.
- **Base ref:** the git ref a comparison gate compares with: the parameter `base_ref`, default `origin/main`; in CI the pull request's base branch.
- **Protected item:** a file or file section on the single protected list (FR44(b), NFR26).
- **Toolchain target, orchestration target:** the two classes of Make targets (FR65).
- **Template-owned, service-owned:** whether template sync overwrites a file (FR100).

## Functional Requirements

Each requirement ends with its trace. "The architect records" marks a value this PRD bounds and the architecture stage fixes. A requirement with lettered clauses states one testable capability per clause; stories and the parity matrix cite clause ids, for example FR88(c). Ids are stable: FR1 to FR120 keep the numbers of the first version of this PRD, and FR121 to FR126 were appended by the edit of 2026-11-10 and stand in their capability areas, out of numeric order. Terms such as source unit, protected item, base ref and backing services are used as defined under "Definitions" at the end of the previous section.

### 1. Service runtime and HTTP surface

- FR1: An operator can run the service as one process that serves plain HTTP on one port (default published port `8081`) in the mode selected by `APP_ENV` (`dev`, `test` or `prod`; default `prod`); the process terminates no TLS, serves no HTTP/3 and applies no response compression; the interactive GraphQL explorer (FR9) is the only behaviour that differs between modes; `test` behaves exactly as `prod` and is the value the test and load-test stacks set; the start-up log entry states the mode. [CAP-01, CAP-DM-18]
- FR2: An operator can run these commands from the service binary, all sharing one configuration model: serve, `migrate`, migration rollback, `check-config`, OpenAPI export, GraphQL schema export and a liveness probe that requests `GET /ping` of the running service and exits 0 or non-zero; the two export commands need no database, no broker and no environment variable. [CAP-02]
- FR3: The service takes every deploy-specific setting from the environment variables of the configuration table ("Command and Configuration Surface"): `DATABASE_URL`, `AWS_REGION`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_ENDPOINT_URL_SQS` and `APP_ENV` under those names, and the listening port, log level, timeouts, retry limit, body limit and shutdown grace period under names the architect records in the environment-variable reference; no committed file contains a secret-shaped value, and committed local defaults are labelled local-only. [CAP-03, P4]
- FR4: The service validates its complete typed configuration before it opens the listener and exits non-zero with a message that names every variable that is invalid, or missing although the command being run requires it (configuration table), and echoes no secret value; the `check-config` command, run by `make check-requirements`, applies the validation of the serve command without opening the listener or contacting a dependency and exits 0 with a success line when the configuration is valid. [NEW-05, CAP-CI-15]
- FR5: The service is assembled in one composition root by explicit construction: health probes, command handlers, query handlers and event subscribers are supplied as collections, and a test can build the complete application with any port replaced by another implementation. [CAP-04]
- FR6: The service serves every API route under the `/api` prefix (`GET /ping` is the only route outside it), accepts and returns `application/json` on REST operations, and returns `application/problem+json` (RFC 9457: `type`, `title`, `status`, `detail`) for every error response: `404` unknown route; `405` wrong method, with an `Allow` header that lists the methods of the route; `415` unsupported media type; `400` a body that is not well-formed JSON or a path or query parameter that cannot be parsed as its type; `413` body over the configured size limit; `422` well-formed input that violates the schema or a validation rule; `503` request over its time budget; `500` unhandled failure or panic. [CAP-06, CAP-14]
- FR7: The service validates request input before application logic runs and reports all violated rules of a request (length, forbidden character, bounds) in one `422` problem response whose `violations` member has one entry per violation, with `propertyPath` (the wire name of the field) and `message`, in a stable order; a structural error (missing or null required field, wrong JSON type, unknown field) is also a `422` problem response and may report only the first such error. [CAP-34, CAP-14]
- FR8: A user can open the API documentation page at `GET /api/docs` (`200`, HTML, no redirect); it renders the service's own OpenAPI document from assets served by the service on sub-paths of `/api/docs`, with no request to a third-party host at run time and no download during the build. [CAP-18]
- FR9: A client can execute GraphQL operations with `POST /api/graphql` (request and response `application/json`); operation errors are returned with HTTP `200` in the GraphQL `errors` member (FR24), while transport-level failures follow FR6 (`400` malformed JSON, `422` a JSON body that is not a GraphQL request, `415` wrong media type, `413` body over the limit); the interactive GraphQL explorer is served at `GET /api/graphql` only when `APP_ENV=dev`, and in `test` and `prod` that request receives a `405` problem response; the explorer page may load its assets from one third-party origin, which its `Content-Security-Policy` names, each asset pinned to an exact version. [CAP-19, P25]
- FR10: The service writes every log entry as one JSON object per line on standard output with at least timestamp, level and message; it writes a start-up entry stating the listening address and the mode and one entry per handled request with method, route, status and duration; for every request it takes the request id from the `X-Request-Id` request header when the value is 1 to 128 characters of the accepted set (visible ASCII, or a subset of it that the architect records) and otherwise generates a UUID, returns the id in the `X-Request-Id` header of the response (error responses included) and adds it to every log entry written while handling that request. [NEW-01, NEW-02, P1]
- FR11: The service sets these headers on every response, error responses included: `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy: no-referrer` and `Content-Security-Policy`; the policy is `default-src 'none'; frame-ancestors 'none'` on every response that is not an HTML page, and on the two HTML pages (documentation page, development-only explorer) it contains `frame-ancestors 'none'` plus only the source directives the page needs, which the architect records. [NEW-03, P5]
- FR12: On SIGTERM the service stops accepting new connections and lets in-flight requests finish within a configurable grace period; when all have finished it exits with status 0; when the grace period ends first it closes the remaining connections and exits with a non-zero status. [NEW-04, P15]
- FR13: The service applies an explicit, configurable timeout to every outbound call (database connection acquisition, database statement, broker call), an explicit retry limit to broker calls, a configurable time budget to every inbound request and a configurable size limit to every request body. [NEW-06, P16]
- FR126: The serve command opens its listener and answers `GET /ping` whether or not the database or the message broker is reachable; it connects to each dependency on first use and never fails start-up because a dependency is unreachable; `GET /api/health` reports the state of the dependencies (FR15). [CAP-DM-20, CAP-08, P2]

### 2. Health and liveness

- FR14: `GET /api/health` returns `204` with an empty body when the database, cache and message-broker probes all succeed. [CAP-07, CAP-TQ-10]
- FR15: `GET /api/health` returns a `500` problem response when at least one probe fails; its `failures` member has one entry per failing dependency that carries that dependency's message, its `detail` joins those messages, and the body contains the message of no healthy dependency; the messages are exactly `Database is not available`, `Cache is not working` and `Message broker is not available`; they are defined in application code and are identical in every environment mode, and the underlying cause is logged, not returned. [CAP-08, P2]
- FR16: The health check has three probes: the database probe executes `SELECT 1` on the service's PostgreSQL connection; the cache probe writes key `health_check` with value `ok` through the cache port and reads it back on every call; the message-broker probe makes one read-only call to the configured SQS endpoint and creates, changes or deletes no broker resource. [CAP-09, CAP-10, CAP-11]
- FR17: The health check runs its probes concurrently, each under its own timeout, and counts a timed-out probe as failed with that dependency's message; a contributor can add a probe by adding one component and registering it in the composition root, without editing the endpoint handler. [CAP-12, P16]
- FR18: `GET /ping` returns `200` with body `pong`, is answered by the service process itself without touching any dependency, and is absent from the OpenAPI document; container health checks and CI wait loops use this path. [CAP-DM-20, CAP-CI-31, P2]

### 3. Example bounded context

- FR19: An API client can create an `ExampleApiResource` with `POST /api/example_api_resources` and a JSON body containing `name` and `description`; the service stores it and responds `201` with `id` (UUID), `name` and `description` and a `Location` header that holds the path of the new resource. [CAP-13, NEW-07, P28]
- FR20: The create operation is the command-path exemplar: (a) input is validated under the rules of "Data Schemas" (`name` 1 to 255 and `description` 1 to 1024 Unicode code points, both required; U+0000 and unknown fields rejected); (b) a command is sent through the command bus to its handler; (c) the handler creates an aggregate that records an `ExampleApiResourceCreated` domain event and saves it through the repository port; (d) the recorded events are published on the event bus only after the save succeeds; (e) when publication reports a failure after a successful save, the operation still succeeds (`201`) and the failure is logged at error level with the event id. [NEW-07, CAP-34, P28]
- FR21: An API client can read one `ExampleApiResource` with `GET /api/example_api_resources/{id}`, served through the query bus: `200` with `id`, `name` and `description`; a `404` problem response when no resource has that id; a `400` problem response when `{id}` is not a UUID. [NEW-08, P28]
- FR22: An API client can list `ExampleApiResource` items with `GET /api/example_api_resources`, served through the query bus: `200` with a JSON array of at most one page, in a total order by `id` whose direction the architect records; the query parameters are `page` (first page `1`, default `1`) and `itemsPerPage` (minimum `1`; default and maximum recorded by the architect, maximum at most 100); a page beyond the last returns `200` with an empty array; a value outside the bounds receives a `422` problem response instead of being silently adjusted, and a value that is not an integer a `400`. [CAP-13, NEW-08, P28]
- FR23: An in-process subscriber to `ExampleApiResourceCreated` writes one structured audit log entry per created resource containing the event name, event id, occurrence time and resource id. [NEW-09, P1]
- FR24: The GraphQL schema exposes exactly three operations, which call the same commands and queries as the REST operations: (a) the query `exampleApiResource(id)`, which returns null when no resource has the id; (b) the query `exampleApiResources(page, itemsPerPage)`, with the bounds and order of FR22; (c) the mutation `createExampleApiResource(input)`, under the validation rules of FR20; (d) the schema contains no Relay node field, no connection type, no update or delete mutation and no subscription; (e) a validation failure, an id that is not a UUID or a page value outside its bounds returns HTTP `200` with an `errors` entry whose `extensions` carries a stable `code` and, for a validation failure, the `violations` list of FR7, and an internal failure returns an `errors` entry with a stable `code` and no internal detail; (f) a request whose query text is not a valid GraphQL document for the schema returns `200` with `errors`. [NEW-10, CAP-19]

### 4. Shared kernel

- FR25: An aggregate root can record domain events and hands them over exactly once in recording order; every domain event carries an event id (UUID), an occurrence time (typed UTC timestamp taken from an injectable clock) and a stable event name. [CAP-25, CAP-26]
- FR26: The event bus port has an in-process synchronous adapter that delivers each published event to every subscriber registered for it, also when one of them fails, treats an event with zero subscribers as success, and returns the error of every failing subscriber to the publisher. [CAP-27]
- FR27: The command bus port and the query bus port each have an in-process adapter that sends a message to exactly one handler; a command returns success or the handler's error and no data, a query returns the handler's result, and a message with no registered handler yields a dedicated error that names the message. [CAP-28, NEW-11, P28]
- FR28: A contributor can register a command handler, query handler or event subscriber in the composition root without editing a bus; registering a second handler for the same command or query stops start-up with an error. [CAP-29, CAP-04]
- FR29: The UUID value object accepts only syntactically valid UUIDs, rejecting any other input with a domain error, and a generator creates new UUIDs. [CAP-30]

### 5. Persistence, cache and messaging

- FR30: The service connects to PostgreSQL through `DATABASE_URL`; the local stacks run the PostgreSQL version given by `DB_VERSION` (default `16`); automated tests use databases separate from the development database. [CAP-21]
- FR31: `ExampleApiResource` is persisted through a repository port declared in the Domain layer and a PostgreSQL adapter in the Infrastructure layer; all persistence mapping lives in that adapter, and table and column names are snake_case. [NEW-12, CAP-23, P28]
- FR32: (a) Schema changes are versioned SQL migrations in the repository, each with an apply step and a revert step, and the first migration creates only the table of the example resource. (b) An operator applies all pending migrations with the `migrate` command, which changes nothing when none is pending. (c) The service process never migrates implicitly. (d) Starting a stack runs `migrate` as its own step, so a started stack needs no manual migration. [CAP-22, CAP-DM-09, NEW-12, P14]
- FR33: An operator can revert the most recently applied migration with one documented command, and applying, reverting and re-applying each migration yields the same schema. [NEW-13, P14]
- FR34: Application code can store and fetch values through a cache port; the shipped adapter is in-process, and replacing the adapter requires no change to code that uses the port. [CAP-32]
- FR35: The service has an SQS client configured by `AWS_REGION`, `AWS_ENDPOINT_URL_SQS` and, when set, `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY`; when the two credential variables are unset the provider's default credential chain applies; the local stacks point the client at LocalStack, an unset `AWS_ENDPOINT_URL_SQS` selects the regular AWS endpoint, and the message-broker probe (FR16) uses the client. [CAP-31]
- FR121: With no reachable database, the workspace compiles, both images build and every gate that does not test against backing services runs, from SQL query metadata committed to the repository; the query-metadata gate (G30), a Make target and CI check, fails when the committed metadata differs from the metadata generated from the current queries against a freshly migrated database. [CAP-22, NEW-12; scope-decisions 8.4]

### 6. Architecture and its enforcement

- FR36: The source is organised as `Shared`, `Internal/HealthCheck` and `CompanySubdomain/SomeModule`, each split into three layers: Domain (aggregates, value objects, events, ports), Application (use cases and API-facing adapters) and Infrastructure (driven adapters for persistence, cache, broker and buses). [CAP-37]
- FR37: Domain-layer code depends on no web, persistence, serialisation-format, messaging or async-runtime framework; API schema, transport and persistence metadata are declared outside the Domain layer. [CAP-15]
- FR38: The architecture gate (G4) fails when Domain depends on Application or Infrastructure, when Application and Infrastructure depend on each other in either direction (only the composition root may depend on every layer), when one bounded context depends on another other than through `Shared`, when a layer uses a third-party dependency outside that layer's allow-list, when a source unit belongs to no layer, or when a workspace crate does not inherit the workspace lint policy; its output names every offending unit and both ends of each forbidden dependency; its rules are defined by layer position, so adding or renaming a bounded context requires no edit to a rule file. [CAP-38, CAP-CI-08, CAP-39, NEW-14, P23]
- FR39: A contributor can run one scaffolding command with a context name and a module name: (a) it creates a bounded-context module with Domain, Application and Infrastructure layers; (b) the result compiles and passes the architecture gate and the crate-isolation gate; (c) each generated crate inherits the workspace lint policy; (d) no protected item is edited; (e) the command prints the remaining steps, among them adding the module's components to `workspace.dsl`, without which the model gate (FR120) fails. [NEW-15, P23]
- FR122: The crate-isolation gate (G29), a Make target and CI check, compiles every workspace crate on its own, with each of its features, and fails when a crate builds only because another workspace crate supplies a dependency or a feature. [CAP-38, CAP-CI-14; scope-decisions 8.5]

### 7. Formatting, static analysis and complexity

- FR40: The formatting check (G1) exits non-zero when any source file differs from the pinned format and modifies no file; a separate fix target rewrites the files. [CAP-TQ-25]
- FR41: The static-analysis gate (G2) analyses production and test code with the rule set the architect records, which at least denies every compiler and linter warning, forbids `unsafe` code, denies panicking shortcuts in production code and requires a reason on every suppression; it covers unused imports, import order and line width. [CAP-TQ-20, CAP-CI-09, CAP-TQ-22, CAP-CI-11, CAP-TQ-23, P11]
- FR123: The complexity gate (G3) fails when a function exceeds the configured limit for cognitive complexity, nesting depth, length or parameter count (NFR17). [CAP-TQ-22, CAP-CI-11, CAP-TQ-24, P11]
- FR42: A contributor can run one fix target that applies every machine-applicable static-analysis fix; while such a fix is pending, the static-analysis gate fails. [CAP-TQ-26, CAP-CI-12]
- FR43: The repository lint gate (G23) fails on findings in GitHub Actions workflow files (syntax and security audit), YAML files, shell scripts, Dockerfiles and Markdown files, and on a misspelling in a tracked file name or workflow name; the generated contract snapshots are excluded. [CAP-CI-34, NEW-20]
- FR44: (a) `make validate-configuration base_ref=<ref>` (G25), run in CI on every pull request, fails when a protected item differs from the base ref or a required directory is missing, and names each offending path or section. (b) A protected item is a whole file or, for a file that also carries content a service changes routinely, a named section of it: for the workspace manifest the lint-policy section, for `tests/Load/config.json.dist` the budgets of the shipped scenarios; the rest of those files is not protected. (c) Deleting or renaming a protected item is a modification; a protected item that does not exist at the base ref is not a violation; once the base ref holds at least one protected item, adding a file that overrides a protected item for part of the tree (a nested copy of a format, lint or build-tool configuration file) is a violation. (d) The check fails with an explicit error when the base ref cannot be resolved. (e) A modification passes only when the pull request carries the approval label; this applies to template-sync, Dependabot and fork pull requests as well, and in CI the check is evaluated again, without a new commit, when the label is added or removed. (f) Outside CI no label can be read: the check fails on a modification and prints the name of the label; the architect may provide an explicit approval variable for local runs, which CI sets only from the label. [CAP-41, CAP-TQ-31, CAP-DM-33, CAP-CI-33, P27]

### 8. Dependency and security checks

- FR45: Every build, test and gate command uses the committed lock file and fails when it does not match the manifest (G5); the manifest-validation target reports a clean checkout as valid with no warning. [CAP-CI-14, CAP-DM-10]
- FR46: `make check-security` (G7) fails when a locked dependency has a known advisory, a licence outside the allow-list or a source outside the allowed registries. [CAP-TQ-30, CAP-CI-16, P6]
- FR124: The unused-dependency gate (G8), a Make target and CI check, fails when a declared dependency is used by no code. [NEW-17]
- FR47: The secrets-scanning gate (G9) fails when a tracked file or a commit under test contains a secret-shaped value. [NEW-18, P4]
- FR48: The image-scan gate (G10) scans the production image and fails on a vulnerability at or above the configured severity (HIGH, see PD-11). [NEW-19, P6]
- FR49: CI runs CodeQL analysis for Rust and uploads the static-analysis findings to GitHub code scanning in SARIF, including on runs in which the static-analysis gate fails; both run on pushes to `main`, on the schedule and on pull requests from branches of the same repository; on fork and Dependabot pull requests, and where code scanning is unavailable, they are skipped with a notice; both are CI-only publications and not gates of the gate list: their findings appear in code scanning and do not fail the pull request, and the failing conditions are carried by G2, G7, G9 and G10. [CAP-TQ-21, CAP-CI-10]
- FR125: The repository-invariants gate (G31), a Make target and CI check, fails when: (a) an action is not pinned by full commit SHA, or a workflow declares no `permissions`; (b) an image reference lacks a version tag and digest or uses `latest`, except the PostgreSQL image of the compose stacks, whose tag is `DB_VERSION` (FR90) and which therefore carries no digest; a tool or test helper library installed by `make install`, the development image or a workflow has no exact version, or a file downloaded by URL has no pinned checksum; (c) the toolchain version differs between the toolchain file, the manifest's minimum version and the builder image tag; (d) the set of gates run by the `ci` target differs from the set run by the pull-request workflows, apart from the CI-only steps of FR66(d); (e) the literal repository name occurs outside its three definitions (FR92(c)); (f) a licence identifier other than `CC0-1.0` is stated for the project (NFR47); (g) a blanket lint suppression exists; (h) a toolchain target runs its tool without the execution switch (FR65). A rule that a linter of G2 or G23 already enforces may be delegated to that linter; its negative test remains. [CAP-DM-05, CAP-AD-30, CAP-TQ-32, NEW-23, NEW-27, P26]

### 9. Unit and integration tests

- FR50: (a) `make unit-tests` (G11) runs the unit suite with no database, broker, cache service or network. (b) Every external dependency (repository, cache, broker client, buses, clock) is behind a port that tests replace with a test double. (c) Unit tests sit with the code they test, and integration and end-to-end tests sit in a tree that follows the bounded-context paths. (d) Documentation examples in the source are compiled and run as part of the unit-test gate. [CAP-TQ-02, CAP-TQ-03, CAP-TQ-05]
- FR51: Tests that use random data draw it from a seeded generator; a failing run prints the seed, and a run started with that seed reproduces the same data; by default (pull-request runs, `make ci`) the seed is one recorded in the repository, and the scheduled workflow overrides it with a fresh seed. [CAP-TQ-04, P10]
- FR52: `make integration-tests` (G12) runs the integration suite against real PostgreSQL and LocalStack provided by the compose stack, locally and in CI; the suite includes the process-level tests that start the built service binary (NFR5, NFR31) and the migration round trip (NFR32). [CAP-TQ-06, CAP-CI-03]
- FR53: `make setup-test-db` drops, recreates and migrates the test database and `make setup-load-test-db` does the same only inside the load-test stack; tests build their own configuration, and no test reads or writes development data. [CAP-TQ-07, CAP-DM-29, CAP-TQ-08]
- FR54: Each health probe has unit tests with test doubles for success, failure and timeout, and the PostgreSQL and SQS probes have integration tests against the real services for success and for an unreachable endpoint. [CAP-TQ-14]

### 10. End-to-end tests

- FR55: The end-to-end target (G13) runs Gherkin features in process against the service's HTTP entry point, locally and in CI: (a) it executes `features/health_check.feature`, whose content is identical to the PHP template's file (4 scenarios); (b) an undefined, pending or skipped step fails the run; (c) scenarios without an injected fault run against the real PostgreSQL, cache and SQS adapters, with the backing services of the compose stack; (d) each scenario that writes data is isolated from the data of other scenarios and from the development database. [CAP-TQ-09, CAP-CI-05, P10]
- FR56: The three failure scenarios (`the database is not available`, `the cache is not working`, `the message broker is not available`) inject the fault by building the application with a failing port implementation, and every scenario builds its own application instance so no fault leaks into another scenario. [CAP-TQ-11, CAP-TQ-12]
- FR57: Gherkin scenarios assert that `GET /api/docs` returns `200` with the API documentation page, and cover the example resource: create then read by id; a list that contains exactly the items the scenario created; a validation failure (`422`); an unknown id (`404`); and one scenario that executes the create mutation and both queries through `POST /api/graphql`. [CAP-TQ-13, CAP-13, P10]

### 11. Coverage and mutation testing

- FR58: (a) `make tests-with-coverage` (G14) runs the unit and integration suites with coverage over the production source (NFR19) and fails below the coverage floor; the end-to-end suite does not count towards the floor. (b) `make coverage-xml` writes a Cobertura XML report and `make coverage-html` an HTML report, both to one git-ignored directory inside the project. (c) Exactly one CI job computes coverage. (d) That job publishes the report to the external coverage service as a non-gating step when `CODECOV_TOKEN` is present and skips publication with a notice when it is absent. [CAP-TQ-16, CAP-CI-04, CAP-DM-32, P9]
- FR59: (a) The mutation gate (G15) mutates the production source in the mutation scope and judges each mutant with the suites the architect records: the unit suite alone, or the unit and integration suites; when the integration suite takes part, the target starts the backing services it needs (FR66(c)). (b) The mutation scope is the production source (NFR19) minus the entries of one tracked, protected scope record, each entry a path or a mutant with a reason; source whose behaviour the judging suites cannot observe (with the unit suite alone: the PostgreSQL and SQS adapters and the process entry points) may be listed there and stays under the coverage floor. (c) A mutant that times out counts as surviving; a mutant that does not compile is not counted. (d) The gate writes a log of each mutant and its outcome and exits non-zero when any mutant survives. (e) `make mutation-tests` runs it over the whole scope (on `main` and in the scheduled run), and a changed-code mode, used by pull requests and by `make ci`, runs it over the code changed relative to the base ref and fails with an explicit error when the base ref cannot be resolved. [CAP-TQ-17, CAP-CI-07, P8]

### 12. Makefile command surface and its tests

- FR60: `make` with no argument runs `help`, which prints `Usage:` and every target with a one-line description; no target is missing from that list. [CAP-DM-24]
- FR61: The Makefile is the single command surface: one target exists per gate (CI-only gates excepted), test level, contract tool and maintenance action; CI workflows, git hooks, documentation and assistant guidance call those targets instead of the underlying tools; repository scripts live under `scripts/`. [CAP-DM-25]
- FR62: These 42 PHP target names exist unchanged with the same capability: `help`, `start`, `up`, `build`, `stop`, `down`, `sh`, `logs`, `new-logs`, `purge`, `clean`, `install`, `update`, `setup-test-db`, `setup-load-test-db`, `validate-configuration`, `check-requirements`, `check-security`, `unit-tests`, `integration-tests`, `all-tests`, `test`, `bats`, `tests-with-coverage`, `coverage-html`, `coverage-xml`, `generate-openapi-spec`, `generate-graphql-spec`, `validate-openapi-spec`, `openapi-diff`, `schemathesis-validate`, `smoke-load-tests`, `average-load-tests`, `stress-load-tests`, `spike-load-tests`, `load-tests`, `execute-load-tests-script`, `aws-load-tests`, `aws-load-tests-cleanup`, `ci`, `all`, `ci-sequential`. [CAP-DM-25, CAP-TQ-01, CAP-DM-23, CAP-DM-30]
- FR63: (a) Each capability behind a tool-named PHP target has a capability-named target (format fix, static analysis, static-analysis fix, complexity, architecture, end-to-end tests, mutation testing (`mutation-tests`), manifest validation, apply migrations, create a migration, roll back a migration, GraphQL schema diff), and no PHP tool name is used as a target name. (b) `start` and `up` build missing or stale images incrementally and start the development stack; `build` rebuilds the images. (c) `stop` stops the stack and `down` removes it. (d) `sh` opens a shell in the development container; `logs` follows all logs and `new-logs` only new ones. (e) `install` installs the pinned development tools on a host and fetches dependencies from the lock file; `update` updates the lock file. (f) `purge` and `clean` remove only generated files. (g) `all-tests` (alias `test`) runs the unit, integration and end-to-end suites. [CAP-DM-25, CAP-TQ-01, CAP-DM-23, CAP-DM-30, CAP-DM-31]
- FR64: Each target sets the configuration it needs; the Makefile imports no environment file and overrides no variable set by the caller. [CAP-DM-27]
- FR65: Every target belongs to one of two classes, recorded per target in the Make-target reference. (a) Toolchain targets (build, format, lint, architecture, tests, coverage, mutation, exports, service commands) work in three places: a host with the pinned toolchain, a shell inside the development container and a GitHub Actions runner; by default they run their tools directly in the current environment; executing through the development container from the host is an explicit opt-in through one variable; `CI=true` always selects direct mode. (b) Orchestration targets (stack lifecycle, image build, tools run from pinned images, load tests, image scan, AWS run) call the container engine from the invoking environment in every mode; where the engine is not reachable, as in a development-container shell without engine access, they exit non-zero with a message that names the requirement. [CAP-TQ-34, CAP-DM-26]
- FR66: (a) `make ci` (aliases `all` and `ci-sequential`) runs every gate of the gate list that has a Make target, in a fixed, documented order, with mutation testing in its changed-code mode and G11, G12 and G14 from one execution of the suites; it stops at the first failure and prints `CI checks successfully passed!` on success. (b) It passes after `make bootstrap` on a clean checkout. (c) Every target that needs backing services starts them when they are not running and never stops or alters a service that was already running. (d) The only CI-only steps are the pull-request title check (G28), the evaluation of approval labels (FR44(e), FR75(d)), publications (coverage upload, the CodeQL analysis and SARIF upload of FR49, artifact uploads), a scanning gate documented as CI-only and the scheduled-only steps that FR95, FR118 and NFR35 require; each is listed with its reason in the Make-target reference. [CAP-TQ-32, CAP-DM-28, CAP-CI-32]
- FR67: (a) A Bats suite under `tests/CLI/bats/` (G16), run locally and in CI, executes the developer-facing command surface and asserts for each tested target its exit status and the tool's own output; a target without a Bats test is listed with its reason in the Make-target reference. (b) It covers the execution switch of FR65: at least one toolchain target in direct mode, one in the opt-in container mode, one with `CI=true` set together with the opt-in variable, and one orchestration target invoked without access to the container engine. (c) It contains the Make-level negative tests of NFR21: each plants a violating fixture, invokes the gate's Make target and asserts a non-zero exit with the gate's diagnostic. (d) Fixtures stay outside every build and analysis scope, and the working tree is restored when a test fails or is interrupted. (e) The helper libraries are provisioned by a documented command at pinned versions, without git submodules. (f) It inspects the production image and the production compose reference (NFR12) and runs one stack test under a different project name (NFR38). [CAP-TQ-18, CAP-CI-06, CAP-TQ-19, P21]
- FR68: Every command that writes into the working tree writes to a defined, git-ignored path, the two contract snapshots and the committed query metadata excepted; after `make bootstrap` and `make ci` on a clean checkout `git status` reports no change. [CAP-CI-36, P26]
- FR69: `make doctor` checks every host prerequisite and its minimum version (NFR37), prints each missing or outdated one by name, prints the detected host platform and whether CI verifies it, and exits non-zero when a check fails. [NEW-21, P21]
- FR70: `make bootstrap` prepares a fresh clone in one run: (a) it checks prerequisites; (b) installs the pinned development tools and the test helper libraries; (c) activates the git hooks; (d) creates any local environment file the stack needs; (e) builds the images; (f) starts the stack and applies migrations; (g) ends with the stack healthy. [NEW-22, P22]

### 13. Contract artifacts and contract checks

- FR71: The service generates an OpenAPI `3.1.0` document from its own route and type definitions; it carries title, description, version, contact (`VilnaCRM Support`, `support@vilnacrm.com`) and licence `CC0-1.0`, a description for each tag, and for each operation a summary, a description, its success response and its problem responses; it defines schemas with properties for `ExampleApiResource`, the create input and the problem document. [CAP-16, CAP-CI-19, CAP-17]
- FR72: `make generate-openapi-spec` writes `.github/openapi-spec/spec.yaml` and `make generate-graphql-spec` writes `.github/graphql-spec/spec`, both through the export commands and without a running stack. [CAP-DM-34, CAP-CI-23]
- FR73: CI fails when a committed contract snapshot differs from the freshly generated one, for both contracts (G17); no workflow commits a snapshot; a release never leaves a committed snapshot different from the generated one, and the architect records how `info.version` of the OpenAPI document is sourced to guarantee it. [CAP-16, CAP-CI-19, CAP-CI-23, P24]
- FR74: `make validate-openapi-spec` (G18) lints the OpenAPI snapshot with Spectral and the PHP ruleset `.spectral.yaml` and fails on a finding of any severity (`--fail-severity=hint`). [CAP-DM-35, CAP-CI-20]
- FR75: (a) `make openapi-diff base_ref=<ref>` (G19) fails when the OpenAPI snapshot is backward-incompatible with the snapshot at the base ref; the image of the diff tool can be overridden with `OPENAPI_DIFF_IMAGE`. (b) The GraphQL schema-diff gate (G20) fails when the GraphQL snapshot has a breaking change against the snapshot at the base ref. (c) Both checks fail with an explicit error when the base ref cannot be resolved, and pass with a notice when the base ref resolves but holds no snapshot at that path (first introduction of the contract). (d) In CI either check passes a breaking change only when the pull request carries the label `approved-breaking-change`, and is evaluated again, without a new commit, when the label is added or removed. (e) Outside CI no label can be read: the check fails on a breaking change and prints the name of the label; the approval variable of FR44(f) may cover these checks as well. [CAP-DM-36, CAP-CI-21, CAP-CI-24, P24]
- FR76: (a) `make schemathesis-validate` (G21) builds the production image, starts it with its own database and emulator in an isolated stack (the stack of FR77 or an equivalent), applies migrations, runs Schemathesis with `--checks all --phases=examples,coverage` against that service and always tears the stack down. (b) It reads and writes no development data and works while the development stack is running. (c) When the caller sets `SCHEMATHESIS_URL`, the target starts nothing and runs Schemathesis against that URL; `localhost` and `127.0.0.1` are handled alike. (d) The architect records how the PHP default `http://localhost:8081` of `SCHEMATHESIS_URL` is honoured when the target manages its own stack. [CAP-DM-37, CAP-CI-22]

### 14. Load testing

- FR77: Each load target builds the production image, starts an isolated load-test stack with its own compose project name, volumes and network that publishes no host port the development stack uses, waits until it is healthy, initialises its database, runs K6 from a pinned stock K6 image (no custom build, no host installation) and always tears the stack down; it neither stops nor alters the development stack or its data. [CAP-DM-38, CAP-DM-45]
- FR78: A developer can run the smoke, average, stress and spike profiles separately (`smoke-load-tests`, `average-load-tests`, `stress-load-tests`, `spike-load-tests`) or in one run (`load-tests`, with the configured pause between profiles, 60 s by default); the kit ships the scenario `health`, which requests `GET /api/health` and checks `is status 204`; only the smoke profile is a gate (G22), and the average, stress and spike profiles are developer tools that no pipeline runs in this release and that enforce their budgets when run. [CAP-DM-39, CAP-DM-46]
- FR79: Load settings per endpoint and profile are declared in `tests/Load/config.json.dist`; an optional git-ignored `tests/Load/config.json` overrides it; `API_HOST` and `API_PORT` override the target; adding `tests/Load/scripts/<scenario>.js` adds that scenario to every runner with no other edit. [CAP-DM-40, CAP-DM-42]
- FR80: A load run fails when the p99 latency of a profile exceeds its configured budget or its check success rate is not above 99%, and a budget that is not a positive number is rejected with `Invalid threshold value` before the run starts. [CAP-DM-41, P19]
- FR81: `make execute-load-tests-script scenario=<name> runSmoke=<bool> runAverage=<bool> runStress=<bool> runSpike=<bool>` runs one scenario with the selected profiles on the load-test stack and exits non-zero with a usage message when `scenario` is empty. [CAP-DM-43]
- FR82: Each load run writes one self-contained HTML report to `tests/Load/results/<prefix><scenario>.html` (prefixes `smoke-`, `average-`, `stress-`, `spike-`, `all-`), prints a console summary, and in CI uploads the results directory as a workflow artifact. [CAP-DM-44, P19]
- FR83: The load-script lint gate (G24) statically analyses the K6 scripts with the linter's recommended rule set and no browser or framework rules, through its own Make target and CI check. [CAP-DM-47, CAP-CI-18]
- FR84: CI runs the smoke profile on every pull request against the production image with the latency and success-rate thresholds enforced (G22). [CAP-CI-17, P19]
- FR85: (a) `make aws-load-tests` provisions temporary AWS infrastructure, runs the load tests on an instance against the current repository and branch, uploads the reports and terminates the instance on success and on failure. (b) `make aws-load-tests-cleanup` removes everything the run created. (c) Region, image, instance type, tag, role, branch and security group can be set when invoking the target. (d) Every wait in the scripts is bounded and its limit is explained where it is set. (e) With `LOCAL_MODE_ENV=true` provisioning and cleanup run offline against LocalStack, and a Bats test covers that dry run. [CAP-DM-48, CAP-DM-49, CAP-DM-50]

### 15. Containers and compose stacks

- FR86: One multi-stage image definition provides a development target, which carries the toolchain pinned in `rust-toolchain.toml` and every tool that the toolchain targets need, and a production target. [CAP-DM-01, CAP-DM-03, NEW-24, P24]
- FR87: (a) The production image contains the optimised service binary and its runtime files only: no build toolchain, test tooling, Makefile, scripts or development configuration. (b) It runs as a non-root user. (c) It uses the service binary as its entry point, so container arguments select the command (`migrate`, `check-config`). (d) It declares a health check that calls `GET /ping` through the binary's own liveness-probe command, because the image has no shell. [CAP-DM-02, CAP-TQ-35, CAP-DM-07, CAP-DM-12, CAP-05, P6]
- FR88: (a) Four compose files keep the PHP file names: `docker-compose.yml` (base), `docker-compose.override.yml` (development), `docker-compose.prod.yml` (production reference) and `docker-compose.load_test.override.yml` (isolated load-test stack). (b) `make start` boots the development stack in the background: the service, PostgreSQL with a persistent volume and a readiness probe that uses the configured user and database, LocalStack with the SQS service and the services the dry run of FR85(e) needs (EC2, S3, IAM, STS), and the Structurizr viewer, started by default or through a profile as the architect records. (c) It starts the service only after the database is ready and the `migrate` step has succeeded, and returns only when the stack is healthy. (d) The `migrate` step waits for the database with a bounded retry and exits non-zero with the connection error when the database stays unreachable. (e) CI starts backing services through Make targets that use the same compose files; a target may start a subset of the stack. [CAP-DM-13, CAP-CI-01, CAP-DM-14, CAP-DM-15, CAP-DM-08]
- FR89: In the development stack, source edits on the host are visible in the container without an image rebuild, the service is rebuilt and restarted automatically after a source change, and dependency and build caches persist across restarts; automatic rebuild is off in the production and load-test stacks and in CI. [CAP-DM-11, CAP-DM-17]
- FR90: Every published port and the database credentials are set through environment variables with working local defaults: `DB_VERSION` (`16`), `DB_NAME`, `DB_USER`, `DB_PASSWORD`, `DB_PORT` (`5432`), `LOCALSTACK_PORT` (`4566`), `STRUCTURIZR_PORT` (`8080`), `APP_ENV` and the service port (`8081`); the `DATABASE_URL` of the local stacks is derived from `DB_USER`, `DB_PASSWORD`, `DB_NAME` and `DB_PORT`, so each value is defined in one place; the defaults are labelled local-only and pass the secrets-scanning gate. [CAP-DM-16, CAP-21, P4]
- FR91: The production compose reference runs the production image with `APP_ENV=prod`, refuses to start when a required secret is unset, and sets resource limits, a restart policy, a read-only root filesystem, dropped capabilities and no-new-privileges. [CAP-DM-21, P6, P15]
- FR92: (a) Image names, container names and compose project names derive from one project-name variable. (b) The package identity `rust-service-template` (scope-decisions section 4) is stated in the workspace manifest, and in the lock file where the build tool mirrors it, and nowhere else; the service binary and the workspace crates have names that do not contain the repository name, and tests, scripts, the Makefile, compose files and the image definition refer to the binary by that name. (c) No workflow, Makefile, image definition, compose file, script, test or skill, and neither `AGENTS.md` nor `CLAUDE.md`, contains the literal repository name outside three definitions: the project-name variable, the template-sync source setting and the workspace manifest with its lock file. (d) The project-name variable and the package identity are the only two values the new-service guide tells a service to change. [NEW-23, CAP-AD-22]

### 16. CI workflows

- FR93: (a) On every pull request to `main` the result of each gate of the gate list is separately attributable under the gate's own name: a job, or a named step whose failure names the gate in the job summary; every step that runs a gate calls a Make target. (b) The architect groups gates into jobs that share a build; G11, G12 and G14 are reported from one execution of the suites, so no test runs twice. (c) The pull-request gate workflows also run on every push to `main`, the release commit excepted. [CAP-TQ-33, CAP-CI-02]
- FR94: A change is documentation-only when every changed path matches the documentation path list the architect records and no protected item is modified; for such a pull request G9, G23, G25, G26, G28 and G31 run, and every other gate reports a passing status without running. [NEW-28, P17]
- FR95: A scheduled workflow on `main` runs G7, G9 and G10, `make mutation-tests`, the 20-generation determinism check of NFR7, a build of the dev container definition (FR118) and, from a clean checkout with no restored cache, `make bootstrap` followed by `make ci`; a failed scheduled run is a failed workflow run whose job summary names the failing gate and the finding. [NEW-29, P18]
- FR96: (a) No workflow pushes a commit to a contributor's pull-request branch or to `main`; the exceptions are the release workflow (release commit and tag on `main`, under the release identity) and the template-sync workflows (their own sync branch). (b) Every pull-request gate also runs, under the same rules, for pull requests from forks and from Dependabot, which have a read-only token and no secrets; such a pull request that modifies a protected item needs the approval label like any other. (c) A workflow whose optional secret is absent skips the affected step with a notice instead of failing. [CAP-CI-23, CAP-CI-25, P12]
- FR97: The repository contains the CodeRabbit configuration `.coderabbit.yaml` with the request-changes workflow enabled. [CAP-CI-30]

### 17. Release, dependency updates and template sync

- FR98: (a) On each push to `main` the release workflow derives the next version from the conventional commits since the last tag (`fix` patch, `feat` minor, footer `BREAKING CHANGE:` major whatever the type of the commit, a footer being a line of the footer section at the end of the message as the conventional-commit definition states); when none of them is a `fix`, a `feat` or a breaking change, it makes no release and ends successfully. (b) It regenerates `CHANGELOG.md` (sections `### Features` and `### Bug Fixes`, all earlier releases kept); `CHANGELOG.md` exists from the first commit. (c) It writes the version to `Cargo.toml` and `Cargo.lock`, pushes the commit `chore(release): vX.Y.Z [skip ci]` and the tag `vX.Y.Z` under the GitHub App identity (`VILNACRM_APP_ID`, `VILNACRM_APP_PRIVATE_KEY`), creates a GitHub release and triggers no further CI run; without those secrets it skips with a notice (FR96(c)). (d) Release runs are serialised and never cancelled. (e) The initial manifest version is `0.1.0`; the architect records in the release guide whether a breaking change below `1.0.0` raises the major or the minor number. [CAP-CI-25, CAP-AD-29]
- FR99: Dependabot opens grouped update pull requests for six ecosystems (Cargo, the Rust toolchain pin, GitHub Actions, Docker, Docker Compose and the npm package in `tests/Load`), with a commit prefix of a type that produces no release, and names no individual as reviewer; an update that modifies a protected item, such as the toolchain pin, needs the approval label like any other pull request. [CAP-CI-28, P18]
- FR100: (a) Two template-sync workflows, `template-sync-pat.yml` (schedule `0 0 1 * *`, secret `PERSONAL_ACCESS_TOKEN`) and `template-sync-app.yml` (schedule `0 9 * * MON`, secrets `VILNACRM_APP_ID` and `VILNACRM_APP_PRIVATE_KEY`), open pull requests labelled `template_sync` from branch `main` of `VilnaCRM-Org/rust-service-template` in downstream repositories; both can also be started manually. (b) A run for the template commit of the open sync pull request updates that pull request in place; a run for a newer template commit opens the pull request of that commit and, in the same run, closes the older open sync pull requests and keeps their branches, so that no two sync pull requests are open at once and no commit that a maintainer pushed to a sync branch is deleted. (c) Each workflow runs only when its credentials exist, only the App flavour runs when both sets exist, and both do nothing in the template repository itself. (d) They never overwrite the files listed in `.templatesyncignore`, which names the service-owned files: changelog, README, workspace manifest and lock file, committed query metadata, contract snapshots, architecture model, migrations, the file that defines the project-name variable, `bmalph/config.json`, source and tests of `CompanySubdomain` and of every context a service adds, feature files other than `features/health_check.feature`, load-test settings and scenarios other than `health`, and planning bundles under `specs/`. (e) Every other tracked file is template-owned and synchronised, among them `Shared`, `Internal/HealthCheck`, the composition root, the tooling crate, the Bats suite, `features/health_check.feature`, the workflows, the Makefile, the guidance and the skills. (f) A sync pull request never deletes what a service added to a template-owned file that services extend (composition root, environment-variable reference, Dependabot configuration); the architect records for each such file whether it is ignored or how it is merged. [CAP-CI-27, P26]
- FR101: Setup guides in the repository cover the release automation and each template-sync flavour: (a) the required secrets and the GitHub App permissions, limited to what the workflows use; (b) how a downstream repository receives updates; (c) that a sync pull request which modifies a protected item needs the approval label of FR44; (d) the template changes that do not propagate because their files are service-owned (the lint policy and dependencies in the workspace manifest, load-test budgets) and how to apply them; (e) the version rules, including the rule below `1.0.0` (FR98(e)). [CAP-CI-35, CAP-AD-32]

### 18. Git hooks and commit convention

- FR102: A `commit-msg` hook rejects a commit message that does not match `/^(feat|fix|refactor|build|chore|ci|docs|style|perf|test)\(\#\d+\): .*/i` and prints the expected format; it accepts the merge and revert messages that git generates; a CI check (G28) applies the same rule to the pull-request title and exempts pull requests authored by Dependabot and pull requests that carry the `template_sync` label. [CAP-TQ-28, CAP-CI-26, CAP-AD-27, P12]
- FR103: Git hooks live in a committed directory, are activated by `make bootstrap` through git configuration and call Make targets only; the pre-commit hook runs the formatting check, static analysis, the architecture gate and the offline dependency checks and needs neither a running stack nor network access; a pre-push hook runs the unit tests. [CAP-TQ-29]

### 19. AI-assistant guidance

- FR104: `AGENTS.md` at the repository root states what the repository is, without the literal repository name; the mandatory reading order; the working rules; the one canonical verification list, which is the ordered list of Make targets that `make ci` runs; and a pointer to the skill index. [CAP-AD-01]
- FR105: `CLAUDE.md` delegates to `AGENTS.md`, adds a project snapshot that does not name the repository and a command reference that links to the canonical verification list instead of repeating it, and keeps the BMAD-generated content of the file intact. [CAP-AD-02]
- FR106: `.claude/skills/` contains an index, a usage guide with the standard flows for a code change, an API change and an architecture change, and a decision guide that maps each task intent to one primary skill and lists multi-skill scenarios and the non-negotiable rules. [CAP-AD-03, CAP-AD-05, CAP-AD-06]
- FR107: Each skill is one Markdown file with `name` and `description` metadata, in a directory named after the skill; the description states when to use the skill; no skill text, and neither `AGENTS.md` nor `CLAUDE.md`, contains the template's repository name or another service-specific assumption. [CAP-AD-04, CAP-AD-22]
- FR108: `.claude/skills/` contains these ten skills, one for each PHP skill, named by capability (the architect records the names): (a) adding or extending an API resource end to end, pointing at the files of the example slice; (b) the verification workflow: a link to the canonical list of FR104, for each gate the skill that handles its failure, and a close-out that lists what was run, what was not run and why; (c) complexity management: refactoring guidance, with limits cited from the quality reference only; (d) fixing architecture-boundary violations, with the violation patterns of the architecture gate; (e) documentation sync: the documentation surfaces, the triggers for updating them and the drift check; (f) load testing: adding and running scenarios, smoke first; (g) API contract development: export, lint, breaking-change check, conformance run and snapshot equality; (h) quality standards: a table of every gate by id with its threshold and Make target, the only place in the guidance where thresholds are stated; (i) architecture-model sync: when `workspace.dsl` must change and the validation command; (j) testing workflow: test levels, commands, run orders by risk, debugging notes and done criteria. The `bmad-autonomous-planning` skill of FR112 is an eleventh skill and is listed in the index. [CAP-AD-07, CAP-AD-08, CAP-AD-09, CAP-AD-10, CAP-AD-11, CAP-AD-12, CAP-AD-13, CAP-AD-14, CAP-AD-15, CAP-AD-16, CAP-AD-21]
- FR109: The guidance states these working rules, each backed by the cited requirement. Enforced by automated checks: never weaken quality configuration to obtain a pass (FR44, FR111); keep Domain code framework-free (FR38); change contract snapshots and the architecture model in the same change as the code they describe (FR73, FR120); follow the commit and pull-request title convention (FR102). Verified by review: run every tool through `make` (FR61); follow the hook, release and migration conventions (FR103, FR98, FR32). [CAP-AD-17, CAP-AD-18, CAP-TQ-36, CAP-AD-19, CAP-AD-20]
- FR110: The documentation-drift check (G26), a Make target and CI check, reads every tracked Markdown file except `CHANGELOG.md` and the planning bundles under `specs/`, and fails when: (1) a cited `make <target>` names no existing target; (2) a cited command line of the service binary names a command the binary's help does not list, or a cited slash command has no file under `.claude/commands/`; (3) a backticked token that begins with the name of a tracked top-level file or directory, or of a generated directory of FR112, names a path that neither exists in a clean checkout nor is matched by a git-ignore rule (a token is checked up to its first placeholder or wildcard character); (4) the verification list in `AGENTS.md` differs from the ordered gate targets of the `ci` target, or another file contains a numbered list or a fenced block that cites three or more gate targets; (5) the environment-variable reference differs from the set of variables read by the service configuration, the compose files and the Makefile; (6) a Make target is absent from `make help` or from the Make-target reference; (7) a skill's `name` differs from its directory, a skill lacks a `description`, or a skill directory is absent from the index. [NEW-30, P27]
- FR111: A committed assistant settings file allows the gate, test, export and stack-start targets and read-only commands without a prompt; requires confirmation for `aws-load-tests`, `aws-load-tests-cleanup`, `update`, `purge`, `clean` and `down`; and blocks edits to protected items through a rule that reads the single protected list; a test submits an edit of a protected item to that rule and asserts the refusal; the guard (FR44), the guidance and the settings file take the protected list from one source. [NEW-31, P27]
- FR112: BMAD and bmalph are integrated following the `core-service` convention: (a) `bmalph/config.json` and the assistant command wrappers are tracked; (b) Make targets install and initialise bmalph; (c) the `bmad-autonomous-planning` skill is present; (d) generated assets (`_bmad/`, `_bmad-output/`, `.ralph/`, `bmalph/state/`) are git-ignored; (e) planning bundles are tracked under `specs/`, and the bundle `specs/rust-service-template-bootstrap/` holds the research digest, capability catalog, product brief, PRD, architecture, epics, implementation readiness and run summary. [NEW-32]

### 20. Human documentation

- FR113: `README.md` explains the purpose, the baseline, a quick start (`make doctor`, `make bootstrap`) with the entry-point URLs and the supported host platforms, the AI workflow with a recommended prompt pattern, local commands, the CI model, downstream sync, cloud load testing, documentation links and the licence; it shows the organisation banner and only badges that are dynamic and backed by an integration the repository configures. [CAP-AD-24, CAP-AD-25, CAP-AD-23, P22]
- FR114: `CONTRIBUTING.md` covers issue, branch, the commit and pull-request title convention with examples and its version rules, the git hooks, pull request, review and merge. [CAP-AD-26, CAP-AD-27]
- FR115: `SECURITY.md` states the supported versions, a private disclosure channel, the response time for a report, the scope (template versus downstream services) and how advisories are published, with the values of assumption A-8, and contains no placeholder text. [CAP-AD-28, P13]
- FR116: The repository contains issue templates written for a backend service: a bug report with default label `bug` that asks for version or commit, endpoint, request and response, log lines with the request id and environment and has no browser or device field; a feature request with default label `enhancement`; and a pull-request template with the process checklist; none of them links to a hard-coded repository. [CAP-CI-29, CAP-AD-31, P22]
- FR117: The repository contains these documents: (a) an architecture overview with decision records, the statelessness assumptions and the statement that domain-event delivery is at most once because no outbox ships; (b) a troubleshooting guide; (c) an environment-variable reference that lists every variable and the PHP-to-Rust name mapping; (d) a Make-target reference that lists every target with its class (FR65), the PHP-to-Rust name mapping and the CI-only steps; (e) operations notes covering migrations, rollback, toolchain updates and a health-probe runbook; (f) a guide to creating a service from the template; (g) extension seams: a Redis adapter through the cache port, and tracing and metrics; (h) repository-settings notes listing the status checks to mark as required, the labels to create (`approved-breaking-change`, `template_sync`, the approval label of FR44, `bug`, `enhancement`), squash merging with the pull-request title as the commit subject, permission for the release identity to push to `main`, private vulnerability reporting, and the governance follow-ups (CODEOWNERS, rulesets, auto-merge, stale bot, OSSF Scorecard, multi-architecture images). [NEW-33, NEW-34, NEW-35, NEW-36, NEW-37, NEW-38, NEW-42, P2, P4, P14, P15, P22, P23]
- FR118: The repository contains an `.editorconfig`; a dev container definition in which the toolchain targets work and the container engine is reachable, built by the scheduled workflow (FR95); and a request collection, in a plain-text format the architect records, with one request for `GET /ping`, for `GET /api/health`, for each REST operation of the example resource and for each GraphQL operation. [NEW-39, NEW-40, NEW-41, P21, P25]

### 21. Architecture model (Structurizr)

- FR119: `workspace.dsl` at the repository root is a Structurizr model with system-context, container and component views; every bounded-context module and layer of FR36 appears in the component view, PostgreSQL and the SQS broker are modelled as external to the service, and the model contains no element absent from the code; it is browsable in the Structurizr viewer of the development stack on `STRUCTURIZR_PORT` (default `8080`), and the viewer's runtime files are git-ignored. [CAP-40]
- FR120: The architecture-model validation gate (G27), a Make target and CI check, fails when `workspace.dsl` does not parse, lacks one of the three views, or its component view does not contain exactly one component for each bounded-context layer that exists in the workspace; it names every missing and every extra component. [NEW-16, CAP-40, CAP-AD-20]

## Non-Functional Requirements

Digest of the limits the gate list does not show; the numbered requirements below are binding.

- Timeouts: explicit on every outbound call, default at most 5 s; health probe at most 3 s; inbound request budget at most 30 s (NFR2, NFR4).
- Shutdown: on SIGTERM in-flight requests finish within the grace period (default 10 s, at most 30 s) and the process exits 0 (NFR5).
- State: none between requests; PostgreSQL is the only source of truth (NFR6).
- Production image: non-root, read-only root filesystem, no capabilities, no shell, at most 100 MiB (NFR12).
- Pins: actions by commit SHA; images by tag and digest; one exact toolchain version; never `latest` (NFR13).
- Workflow tokens: `permissions` declared in every workflow; read-only on pull requests (NFR14).
- Responses: four security headers and `X-Request-Id` on every response; no internal detail in a `500` (NFR15).
- Logs: one JSON object per line (NFR31).
- Nothing secret-shaped is committed; the repository name occurs only in its three definitions (NFR10, NFR38).
- Time: pull request 20 min warm, 40 min cold, 5 min documentation-only; bootstrap 15 min; `make ci` 45 min (NFR35, NFR39).

### Gate list

Referenced by FR66, FR93, FR108 and NFR21. Every gate has a threshold, a CI check and negative tests. Every gate is a Make target except G28, which needs pull-request context; G9 or G10 may be CI-only when the architect documents the reason (scope-decisions 8.5).

| Gate | Name | Pass condition | FR | NFR |
| --- | --- | --- | --- | --- |
| G1 | Formatting check | 0 differences | FR40 | NFR16 |
| G2 | Static analysis | 0 findings, warnings denied | FR41 | NFR16 |
| G3 | Complexity limits | 0 functions over a limit | FR123 | NFR17 |
| G4 | Architecture boundaries | 0 violations, 0 unassigned crates | FR38 | NFR18 |
| G5 | Manifest and lock validation | lock file matches manifest | FR45 | NFR9 |
| G6 | Configuration check | `check-config` exits 0 | FR4 | NFR36 |
| G7 | Dependency policy | 0 advisory, licence or source violations | FR46 | NFR9 |
| G8 | Unused dependencies | 0 | FR124 | NFR9 |
| G9 | Secrets scanning | 0 findings | FR47 | NFR10 |
| G10 | Production-image scan | 0 HIGH or CRITICAL findings | FR48 | NFR11 |
| G11 | Unit tests, with documentation tests | all pass | FR50 | NFR28 |
| G12 | Integration tests | all pass | FR52 | NFR29 |
| G13 | End-to-end tests (Gherkin) | all pass; 0 undefined or pending steps | FR55 | NFR30 |
| G14 | Coverage | 100% lines and 100% functions | FR58 | NFR19 |
| G15 | Mutation testing | 0 surviving mutants | FR59 | NFR20 |
| G16 | Bats tests of the Makefile | all pass | FR67 | NFR21 |
| G17 | Contract snapshot equality | 0 bytes of difference | FR73 | NFR7 |
| G18 | OpenAPI lint | 0 findings at any severity | FR74 | NFR24 |
| G19 | OpenAPI breaking-change check | 0 unapproved breaking changes | FR75 | NFR24 |
| G20 | GraphQL schema diff | 0 unapproved breaking changes | FR75 | NFR24 |
| G21 | Schemathesis | 0 failed checks | FR76 | NFR24 |
| G22 | K6 smoke load | p99 within budget; checks above 99% | FR84 | NFR1 |
| G23 | Repository lint (workflows, YAML, shell, Dockerfile, Markdown, spelling) | 0 findings | FR43 | NFR25 |
| G24 | Load-script lint | 0 findings | FR83 | NFR25 |
| G25 | Protected-configuration guard | 0 unapproved changes; 0 missing directories | FR44 | NFR26 |
| G26 | Documentation-drift check | 0 violations of FR110 | FR110 | NFR43 |
| G27 | Architecture-model validation | valid, 3 views, components match the workspace | FR120 | NFR18 |
| G28 | Pull-request title convention | title matches the commit rule | FR102 | NFR21 |
| G29 | Crate isolation | every crate builds alone, with each feature | FR122 | NFR18 |
| G30 | Persistence query metadata | 0 differences from generated metadata | FR121 | NFR32 |
| G31 | Repository invariants | 0 violations of FR125 | FR125 | NFR13 |

Where a requirement says "ceiling", the architect records the final value at or below it; a value above a ceiling needs a recorded measurement and reason. Where a requirement names the implementation-readiness review as its verification, the review records the result per requirement id.

### Performance

- NFR1: K6 profiles keep the PHP load shapes (smoke: 10 requests per second for 10 s with 5 virtual users; average: peak 25 requests per second; stress: peak 150; spike: peak 200). Against the production image the p99 of request duration stays within the configured budget and the check success rate stays above 99%. Budget ceilings: smoke 500 ms (PHP: 9000 ms), average 200 ms, stress 1000 ms, spike 5000 ms. Each budget is three times the p99 measured on a 4-vCPU GitHub-hosted runner, rounded up to a multiple of 10 ms, not below 50 ms (so that runner noise does not fail the gate) and not above its ceiling; measurement and budget are recorded in the load-testing documentation. A pipeline enforces the smoke budget only (G22, FR78). [CAP-DM-41, CAP-CI-17, P19; K-18]
- NFR2: `GET /api/health` responds within the probe timeout plus 1 s when every dependency is unreachable or hangs; the probe timeout is configurable with a default of at most 3 s. Verified by a unit test with a controllable clock and an integration test against unreachable endpoints. [CAP-12, P16]
- NFR3: The production container answers `GET /ping` within 5 s of process start, with or without reachable dependencies (FR126). Its health check uses an interval of at most 10 s, a timeout of at most 3 s, 3 retries and a start period of at most 30 s (PHP: 180 s). Verified by a process-level test (G12) and by the image inspection of FR67(f). [CAP-DM-07]

### Reliability

- NFR4: 100% of outbound calls have an explicit timeout and none relies on a driver default; ceilings for the defaults: 5 s each for database connection acquisition, a database statement and a broker call, and 30 s for the inbound request budget. A test against an unreachable database and an unreachable SQS endpoint shows each adapter returning an error within its timeout plus 1 s. [NEW-06, P16]
- NFR5: After SIGTERM an idle service exits with status 0 within 2 s; with a request in flight it completes that request and exits 0 within the grace period (default 10 s, ceiling 30 s); the default grace period is not shorter than the default inbound request budget; when the grace period ends with a connection still open, the process closes it and exits non-zero within 2 s; a connection attempted after SIGTERM is refused. Verified by process-level tests of the integration suite (G12). [NEW-04, P15]
- NFR6: The service is stateless between requests: it keeps no session or per-client state, PostgreSQL is the only source of truth, and 0 endpoint responses depend on the content of the in-process cache. The architecture overview states these assumptions, and a test reads a resource through an application instance other than the one that created it. [NEW-33, P15]
- NFR7: Contract generation is deterministic: 20 consecutive generations of each snapshot are byte-identical, and a generation after a clean rebuild equals them; verified by the scheduled workflow (FR95), whose clean-clone job builds from nothing. The committed snapshots differ from the generated ones by 0 bytes; verified by G17 on every pull request and on every push to `main`. [CAP-16, CAP-CI-23, P24; K-07]
- NFR8: With `VILNACRM_APP_ID`, `VILNACRM_APP_PRIVATE_KEY`, `PERSONAL_ACCESS_TOKEN` and `CODECOV_TOKEN` absent and the CodeRabbit App not installed, 0 workflow runs fail for that reason; each affected step is skipped with a notice. Measured over the workflow runs of `main` and of pull requests, fork pull requests included. [CAP-CI-25, CAP-CI-27, CAP-CI-30; K-14]

### Security and supply chain

- NFR9: The dependency gates (G5, G7, G8) report 0 known advisories, 0 licences outside the allow-list, 0 sources outside the allowed registries, 0 unused dependencies and a lock file that matches the manifest. An exception is a dated entry with a reason in the tracked policy file. [CAP-TQ-30, CAP-CI-14, NEW-17, P6; K-17]
- NFR10: Secrets scanning (G9) reports 0 findings over the tracked files and the full history of `main`; 0 committed files contain a secret-shaped value; a secret-shaped fixture that a negative test needs is generated at test time. [NEW-18, CAP-03, P4; K-17]
- NFR11: The production-image scan (G10) reports 0 findings of severity HIGH or CRITICAL; an ignored finding is a dated entry with a reason in a tracked ignore file. [NEW-19, P6; K-17]
- NFR12: The production container runs under a non-root user id with a read-only root filesystem, all capabilities dropped and no-new-privileges set; the production image contains no shell, package manager, compiler or test file and is at most 100 MiB uncompressed (ceiling). Verified by the Bats suite (G16), which inspects the image and the rendered production compose reference (FR67(f)). [CAP-DM-02, CAP-DM-21, P6]
- NFR13: 100% of actions, GitHub-owned ones included, are pinned by full commit SHA. 100% of base images (builder, development, production runtime) and tool images are pinned by version tag and digest, an image without an immutable version tag by digest alone, with 0 uses of `latest`; the PostgreSQL image of the compose stacks is the one exception: its tag is `DB_VERSION` (FR90) and it carries no digest, because a digest overrides the tag. Every tool installed by `make install`, the development image or a workflow, and each Bats helper library, is pinned to an exact version, and every file downloaded by URL in a workflow, script or image definition is verified against a pinned checksum. The toolchain is pinned to one exact stable version (`1.99.1` at the first release); the toolchain file, the manifest's minimum version and the builder image tag state the same version. LocalStack is pinned to the tokenless community tag `3.4.0`. Assets that a served page loads from a third-party origin are pinned to exact versions (FR9); a test of the page content verifies it. The other pins are verified by G31 rules (a) to (c) and by the workflow security audit of G23. [CAP-DM-05, NEW-24, NEW-27, P26; K-16]
- NFR14: 100% of workflows declare `permissions`; workflows triggered by pull requests hold a read-only token, except `security-events: write` in the code-scanning job for pull requests from the same repository (FR49); other write permissions exist only in the release and template-sync jobs, each limited to the scopes it uses. Verified by G31 rule (a) and by the workflow security audit of G23. [NEW-27, P26; K-16]
- NFR15: 100% of responses, on every route of the endpoint table and on every error path, carry the four headers of FR11 with the values stated there and `X-Request-Id`; 0 `500` responses contain a stack trace, file path, SQL text or driver message. Verified by tests over every route, an unknown route and a forced panic. [NEW-03, NEW-02, CAP-14, P5]

### Maintainability and quality gates

- NFR16: The formatting check reports 0 differences with a line width of at most 100 columns (ceiling), and static analysis reports 0 findings with warnings denied over production and test code; template-owned code contains 0 uses of `unsafe`; every suppression names one rule, carries a reason and applies to one item, or to one module when the finding lies in macro-generated code that an item-level suppression cannot reach, or, for an inline directive of shellcheck, hadolint, yamllint, markdownlint, ESLint or zizmor, which have no item scope, to one file; 0 blanket suppressions exist (G31 rule (g)). [CAP-TQ-25, CAP-TQ-20, CAP-TQ-23, P11]
- NFR17: 0 production functions exceed the complexity limits; ceilings: cognitive complexity 10, 100 lines per function, nesting depth 5 and 5 parameters. The architect records the exact limits and whether test code has a relaxed profile. [CAP-TQ-24, CAP-TQ-22]
- NFR18: The architecture gate reports 0 layer violations, 0 unassigned source units, 0 dependencies between bounded contexts outside `Shared` and 0 crates outside the workspace lint policy; the crate-isolation gate reports 0 crates that fail to build alone; a test runs the scaffolding command and then both gates, with 0 protected items edited; the model validation reports 0 errors and 0 differences between the components of the model and the bounded-context layers of the workspace. [CAP-38, NEW-14, NEW-15, NEW-16; K-10]
- NFR19: Line coverage and function coverage of production source are each 100%, enforced in the repository; region coverage is reported and not gated. Production source is every Rust source file of the service crates, the composition root (process entry point included) and the template-owned tooling (architecture checker, guard, scaffolding command and the other checkers); test modules, test-support crates and end-to-end step definitions are not production source. Coverage is measured from the unit suite and the integration suite, which includes process-level tests that start the built binaries. An exclusion is an entry with a reason in one protected record; the first release has 0 entries. [CAP-TQ-16, P9; K-09]
- NFR20: `make mutation-tests` reports 0 surviving mutants over the mutation scope (FR59(b)), template-owned tooling included, on `main` (on each push to `main` and in the scheduled run); on a pull request and in `make ci` the changed-code mode reports 0 surviving mutants. Exclusions are scope entries and equivalent mutants, each recorded with a reason in the protected scope record; the readiness review reports their count. [CAP-TQ-17, P8; K-09]
- NFR21: Every gate except G28 has at least one automated negative test that plants a violation, invokes the gate's Make target and asserts a non-zero exit and the gate's diagnostic. A gate that combines independent tools or policies has one such test per tool or policy: per linter (G23), per policy (G7: advisory, licence, source) and per snapshot (G17). Every further pass condition of a gate implemented by a template-owned checker (the rule kinds of G4, G25, G26, G27 and G31) has an automated negative test at Make level or at the checker's own test level. A fixture that would itself violate a gate (a secret, a vulnerable manifest) is generated at test time and never committed. The rule of G28 is covered by a test with matching and non-matching titles. [CAP-TQ-19; K-03]
- NFR22: The gates run by `make ci` and the gates run by pull-request workflows differ by 0 unexplained items; each CI-only step is listed with its reason (FR66(d)). Verified by G31 rule (d). [CAP-TQ-32, CAP-CI-32; K-04]
- NFR23: The repository contains 0 unused dependencies (G8) and 0 unused private items in production source (dead-code analysis of G2); and 0 public shared-kernel items unused by the example slice or the health check, 0 interfaces without an implementation, 0 configuration files that no command reads and 0 tests without an assertion (checklist of the implementation-readiness review). [scope principle 5; K-15]
- NFR24: The OpenAPI snapshot has 0 Spectral findings at severity hint or above with the unchanged PHP ruleset (G18), Schemathesis reports 0 failed checks (G21), and 0 breaking changes reach `main` without the `approved-breaking-change` label (G19, G20). [CAP-DM-35, CAP-DM-37, CAP-DM-36, CAP-CI-24, P24; K-08]
- NFR25: The lint gates (G23, G24) report 0 findings for workflow files, YAML, shell scripts, Dockerfiles, Markdown, the spelling of file and workflow names, and the K6 scripts. [NEW-20, CAP-CI-34, CAP-DM-47]
- NFR26: Exactly 1 protected list exists; the guard, the guidance and the assistant settings read it. It names items (a file or a file section) and contains at least: the toolchain pin; format settings; the lint policy and lint thresholds; architecture rules; the dependency policy and its exceptions; secrets-scan and image-scan configuration and ignore files; coverage and mutation settings with their exclusion and scope records; test-runner settings; the Spectral ruleset; repository-lint and load-lint configuration; the K6 budgets of the shipped scenarios; hook scripts; the negative tests and fixtures of NFR21; the assistant settings file; the list and the guard themselves. Every numeric threshold of a gate is stated in a protected item, not only on a command line. Makefile recipes and workflow files are not protected items; weakening a gate there is detected by the negative tests (NFR21) and the gate-set comparison (NFR22). [CAP-41, NEW-31, P27]
- NFR27: A parity matrix tracked in the planning bundle `specs/rust-service-template-bootstrap/`, which is the capability catalog together with the Traceability section of this PRD, maps 196 of 196 PHP capability ids (185 delivered, 11 dropped with a reason) and 42 of 42 `NEW` ids to their requirement; the readiness review finds 0 replicated defects from the catalog's 150-line digest, apart from the 3 accepted by decision (`/ping` as container health check; the issue number in the changelog scope; the unchanged PHP Spectral ruleset (catalog note CI:D-28)). [scope principles 1 and 3; K-05, K-06]

### Testability

- NFR28: The unit suite passes with network access disabled and no container running; 0 unit tests open a socket or read the wall clock, enforced by a static-analysis rule (G2) that forbids wall-clock and network APIs outside the adapters that own them, where each use carries a reasoned suppression, and by the per-layer dependency allow-lists (G4). [CAP-TQ-02, CAP-26]
- NFR29: Tests do not depend on execution order: every unit and integration test passes when run alone and when all run in parallel, and no test shares in-process state with another; verified on every pipeline run, in which the test runner executes the tests in parallel and isolated from one another. 100% of failing randomised tests print their seed; the seed is fixed by default and fresh in the scheduled run (FR51). Integration tests that write to PostgreSQL use isolated databases. [CAP-TQ-04, CAP-21, P10]
- NFR30: 4 of 4 scenarios of the unmodified `features/health_check.feature` pass; 0 undefined or pending steps are tolerated; scenarios pass in any order; a test asserts for each of the 7 non-empty combinations of failing dependencies that the `500` body contains exactly the messages of the failing ones. [CAP-TQ-09, CAP-TQ-12, CAP-08, P10; K-08, K-09]

### Operability

- NFR31: 100% of the lines the serve command writes to standard output and standard error, panic reports included, parse as single-line JSON with timestamp, level and message, and 100% of entries written while handling a request carry its request id; the other commands write human-readable text. Verified by a process-level test (G12) that captures the output of a request sequence and of a forced panic. [NEW-01, NEW-02, P1]
- NFR32: The integration suite (G12) applies, reverts and re-applies every migration and compares the resulting schemas (0 differences); running `migrate` twice changes nothing the second time; the committed query metadata differs from the generated metadata by 0 entries (G30). [NEW-13, CAP-22, P14]
- NFR33: The database readiness probe uses the configured user and database, with an interval of at most 10 s and 10 retries; the bounded database wait makes at most 60 attempts 1 s apart (ceiling) and then exits non-zero; `make start` exits non-zero when the stack is not healthy within its wait limit. [CAP-DM-08, CAP-DM-14]

### Portability and developer experience

- NFR34: From a clean checkout with no restored cache on a GitHub-hosted runner, `make bootstrap` followed by `make ci` exits 0, with 0 manual steps between clone and success, on the release commit and on every scheduled run in which no gate reports a finding published after the commit under test and no external service is unavailable. A scheduled run that fails on a new advisory or image vulnerability is a correct result: it names the finding in the job summary and does not count against this requirement. [NEW-22, NEW-29; K-01]
- NFR35: On a 4-vCPU GitHub-hosted runner, `make bootstrap` reaches `GET /ping` = `200` within 15 min cold, `make ci` then completes within 45 min cold, and `make start` with images and build cache present returns within 60 s. These are initial budgets from research timings; the architect confirms or revises them with measurements and a recorded reason. [NEW-22, CAP-DM-13; K-02]
- NFR36: 100% of toolchain targets honour the execution switch (G31 rule (h)); the Bats suite covers the switch as FR67(b) states; `check-config` passes in all three execution places. [CAP-TQ-34, CAP-DM-26, CAP-CI-15]
- NFR37: Host prerequisites are a container engine with the Compose plugin, GNU Make, Bash, git and curl; the pinned toolchain for direct mode; and Node.js only for the JavaScript-based tools that do not run from a pinned image (load-script lint, GraphQL schema diff, Markdown lint, the bmalph targets; the architect records which). Every other tool is installed by `make install` or carried by the development image. The architect records the minimum version of each prerequisite; `make doctor` reports each missing or outdated one and names the targets that need it. Supported host platforms: Linux x86_64, verified in CI; Linux aarch64 and macOS through the opt-in container mode and Windows only through WSL 2, none of the three verified in CI in this release. The repository has 0 git submodules; after `make ci` the working tree contains 0 files owned by a user other than the invoking one. [NEW-21, CAP-DM-17, P21]
- NFR38: The literal repository name occurs 0 times in workflows, the Makefile, the image definition, compose files, scripts, tests, skills, `AGENTS.md` and `CLAUDE.md` outside its three definitions (the project-name variable, the sync-source setting, the workspace manifest with its lock file); verified by G31 rule (e). The stack test of FR67(f) passes under a different project name. 0 sync pull requests are opened in the template repository. The spelling check of G23 reports 0 findings over tracked file names and workflow names. [NEW-23, CAP-AD-22, CAP-CI-27; K-13]

### CI cost

- NFR39: On 4-vCPU GitHub-hosted runners a pull request completes its last required check within 20 min of the push on warm caches and within 40 min on cold caches; the changed-code mutation check is outside these two figures and has its own timeout of at most 45 min; a documentation-only pull request completes within 5 min; 100% of jobs declare a timeout of at most 45 min, except the clean-clone job of FR95, whose timeout is at most 75 min; the Bats job completes within 20 min. Initial budgets, revised only with a recorded measurement. [NEW-26, NEW-28, P17; K-11]
- NFR40: 100% of pull-request workflows cancel superseded runs of the same ref; 100% of jobs that compile Rust code or build an image restore a dependency and build cache or an image layer cache, except the `surface` Bats group and the weekly `devcontainer-build` job, the first two exceptions recorded in architecture-delivery.md, open item 5, and the weekly `clean-bootstrap-ci` job, which FR95 and NFR34 require to start without a restored cache, recorded there as the third exception, and, on a rerun of a pull-request commit, the third-party crates that a mutation shard compiles because its diff selects packages the saver shard on main did not build (measured 0 to 77 compile units per shard; the saver selection itself recompiles 0), recorded there as the fourth exception; on a rerun of an unchanged commit 0 third-party dependencies are recompiled, apart from the four exceptions above, and 0 unchanged image layers are rebuilt, verified by the readiness review from the log of one rerun; the image definition keeps dependencies in a layer separate from source, and the build context excludes version-control data, build output and test results. [NEW-25, NEW-26, CAP-DM-04, CAP-DM-06, P17]
- NFR41: Coverage is computed and uploaded by exactly 1 job; G11, G12 and G14 are reported from one execution of the suites, in CI and in `make ci`; apart from the determinism check of NFR7, each contract snapshot is generated once per workflow run; a job starts only the stack services it uses. [CAP-TQ-16, CAP-CI-04, P9, P17]
- NFR42: The scheduled run on `main` executes at least weekly, and Dependabot checks each of its six ecosystems at least monthly. [NEW-29, CAP-CI-28, P18]

### Documentation and AI-readiness

- NFR43: 0 commands or repository paths cited in the tracked Markdown files that FR110 reads are missing from a clean checkout, paths matched by a git-ignore rule excepted (among them the generated directories of FR112); exactly 1 canonical verification list exists, in `AGENTS.md`; it equals the gate order of the `ci` target, and every other document links to it instead of repeating it. Verified by G26 rules (1) to (4). [NEW-30, CAP-AD-01, P27; K-12]
- NFR44: 100% of skills have a `name` equal to their directory and a `description`, and exactly 1 skill inventory exists (the index), which lists every skill; verified by G26 rules (3) and (7). 100% of descriptions state when to use the skill, and 100% of code patterns a skill names resolve to an existing file of the example slice, the health check or the shared kernel; verified by the implementation-readiness review. [CAP-AD-04, CAP-AD-07]
- NFR45: The environment-variable reference lists 100% of the variables read by the service, the compose files and the Makefile; the Make-target reference and `make help` each list 100% of the targets. Verified by G26 rules (5) and (6). [NEW-35, NEW-36, CAP-DM-24, P22]
- NFR46: 0 documents needed to follow journeys J1 to J8 are hosted only outside the repository, and `SECURITY.md` contains 0 placeholders. Verified by the implementation-readiness review. [CAP-AD-24, CAP-AD-28, P22, P13]
- NFR47: Exactly 1 licence identifier, `CC0-1.0`, appears for the project: in the package manifest, `LICENSE`, `README.md` and the `info.license` of the OpenAPI document. Verified by G31 rule (f). [CAP-AD-30, P26]

## Assumptions and Decisions

### Decisions taken in this PRD

Each item is a choice the binding inputs leave open. The architect may change one only with a recorded decision and reason.

| Id | Decision | Basis |
| --- | --- | --- |
| PD-1 | Error status codes: `400`, `404`, `405`, `413`, `415`, `422`, `500`, `503` as in the error-code table; `400` for input that cannot be parsed, `422` for well-formed input that violates the schema or a rule; `503` for a request over its time budget | Verified by the HTTP research spike; scope fixes the media type, not the codes |
| PD-2 | Request id header is `X-Request-Id` | Verified by the HTTP research spike |
| PD-3 | Security headers are the four of FR11, with `default-src 'none'; frame-ancestors 'none'` as the policy of every response that is not an HTML page; `Strict-Transport-Security` and cross-origin headers are left to the platform edge because the service speaks plain HTTP | Scope 3.1 (TLS at the edge); audit P5 |
| PD-4 | `ExampleApiResource` has `id` (UUID), `name` (1 to 255 code points) and `description` (1 to 1024 code points); U+0000 is the only forbidden character and white space is not trimmed; create returns `201` with a `Location` header; the list returns a JSON array | PHP fields `name` and `description` preserved; scope asks for read by id and a bounded page; PostgreSQL cannot store U+0000; one minimal rule keeps the schema and the validation identical |
| PD-5 | GraphQL operation errors return HTTP `200` with the GraphQL `errors` member and a stable `code`; the problem-document rule applies to REST responses and to transport-level failures of the GraphQL route | The GraphQL response format cannot carry a problem document without breaking clients |
| PD-6 | The health failure body is the same in every environment mode and never contains driver text | Catalog row CAP-08; brief open question 5 |
| PD-7 | `make ci` runs the pull-request gate set, with mutation testing in its changed-code mode; the full run is `make mutation-tests` on `main` and on the schedule | Scope-decisions 8.5; keeps `make ci` equal to pull-request CI (K-04) and inside its time budget (K-02) |
| PD-8 | The changelog keeps every release | The PHP five-release window was a tool default, not a decision |
| PD-9 | The sync ignore list covers the service-owned files of FR100(d); every other tracked file is synchronised downstream, tracked BMAD and bmalph files other than `bmalph/config.json` and `specs/` included | Brief open question 3; `bmalph/config.json` carries the project name |
| PD-10 | A template-sync pull request that modifies a protected item needs the same approval label as any other pull request, and the sync guide says so | Scope-decisions 8.5 defines one override, the approval label |
| PD-11 | The image scan fails on HIGH and CRITICAL findings | Audit P6 |
| PD-12 | Coverage publication to the external service stays as a non-gating step | Catalog analyst decision 17 |
| PD-13 | The output markers `Usage:` and `CI checks successfully passed!` are kept from PHP | Lets the Bats tests port with few edits |
| PD-14 | Only the App flavour of template sync runs when both credential sets exist | Avoids two overlapping sync pull requests (catalog defect CI:D-13) |
| PD-15 | Numeric ceilings in NFR1 to NFR5, NFR12, NFR16, NFR17, NFR33, NFR35 and NFR39, the 50 ms budget floor of NFR1 and a default request-body limit of at most 1 MiB are PRD bounds; the architect records final values within them | Scope marks these values "architect decides"; the brief proposes the time budgets |
| PD-16 | A request body above the configured size limit returns a `413` problem response | Scope requires every error response to be a problem document |
| PD-17 | Wire names follow the names PHP services expose through API Platform: list parameters `page` and `itemsPerPage`, violation members `propertyPath` and `message`; the GraphQL operations are `exampleApiResource`, `exampleApiResources` and `createExampleApiResource` | Both templates report alike; the snapshots freeze these names, so they are fixed here |
| PD-18 | A create whose event publication fails after the save returns `201`; domain-event delivery is at most once | No outbox ships (scope 3.2); a `500` for a stored resource would misreport state and invite a duplicate on retry |
| PD-19 | The event bus delivers an event to every subscriber even when one fails and returns every failure to the publisher | Scope 3.1 (zero or many subscribers); the same completeness rule as the health check |
| PD-20 | The serve command starts and answers `GET /ping` without reachable dependencies | Liveness and readiness are separate (audit P2); a dependency outage must not restart every replica |
| PD-21 | When the grace period ends with a connection still open, the process exits non-zero; the default grace period covers the default request budget | A cut request must be observable; with the defaults no request is cut |
| PD-22 | CodeQL analysis and the SARIF upload are CI-only publications, not gates | They have no threshold of their own (scope principle 4) and cannot upload from fork or Dependabot pull requests |
| PD-23 | Only the smoke profile is gated in a pipeline; the average, stress and spike profiles are developer tools | Scope 3.1 ("smoke on pull requests") |
| PD-24 | The gate list has 31 gates: the 28 of the first version plus G29 crate isolation and G30 query metadata (checks fixed by scope-decisions 8.4 and 8.5) and G31 repository invariants (the home of checks that NFR13, NFR14, NFR16, NFR22, NFR36, NFR38 and NFR47 need) | A check outside the gate list would have no Make target, CI check or negative test |
| PD-25 | A protected item may be a named section of a file | Scope-decisions 8.5 puts the lint policy in the workspace manifest, which changes with every dependency |
| PD-26 | Negative tests run at Make level once per gate and once per independent tool or policy; the remaining rules of template-owned checkers are proven at the checker's own test level | Scope-decisions 8.6 keeps the Bats job within about 20 minutes |
| PD-27 | The development-only explorer may load assets pinned to exact versions from one third-party origin; the documentation page may not load any | The documentation page is served in production; the explorer is not |
| PD-28 | Coverage is measured from the unit and integration suites; the end-to-end suite does not count | Catalog row CAP-TQ-16 |
| PD-29 | `make schemathesis-validate` manages its own production-image stack; `SCHEMATHESIS_URL` selects an already running service instead | Scope-decisions section 5; catalog defects DM:D-14 and CI:D-29 |
| PD-30 | The changed-code mutation check has its own time limit outside the pull-request budget of NFR39 | The number of mutants in a diff is unbounded |
| PD-31 | Dependabot commits use a type that produces no release | Scope-decisions 8.9 (dependency updates must not trigger feature releases) |

### Assumptions

- A-1: "Parity" means parity of capability and intent. Contracts marked "preserved" in the capability catalog are identical; PHP mechanisms and PHP defects are not parity targets.
- A-2: The catalog's dispositions, including its 26 analyst decisions, stand; this PRD changes none of them.
- A-3: Maintainers configure in the organisation's settings what the template cannot enforce: the required status checks, the labels of FR117(h), squash merging with the pull-request title as the commit subject (the title check of FR102 relies on it), permission for the release identity to push to `main`, and private vulnerability reporting. Rulesets are out of scope, so the template lists these settings (FR117(h)).
- A-4: Section 8 of `scope-decisions.md` names crates and tools; this PRD states the same decisions as capabilities and leaves the names to the architecture document. Three checks of section 8 that the first version of this PRD missed were added by the edit: crate isolation (FR122), committed query metadata (FR121) and lint-policy inheritance (FR38).
- A-5: The research spikes show the scope is feasible on the stable toolchain: every PHP gate has an equivalent that was run passing and failing, contract exports were byte-identical over 20 runs, and the PHP contract tools accepted the Rust output.
- A-6: The AWS load-test flow is verified against the LocalStack dry run only, because no AWS credentials are available to CI.
- A-7: Host platforms (NFR37): Linux x86_64 is the only platform verified in CI, because every research measurement was taken on it. Maintainers confirm or extend the list.
- A-8: Values of the security policy (FR115): reports go through GitHub private vulnerability reporting on the repository; the supported version is the latest release; a report is acknowledged within 5 working days; advisories are published as GitHub security advisories. Maintainers confirm or replace these values before the release.

### Decisions delegated to the architect

The architecture stage fixes these, within `scope-decisions.md` section 8, and records each with evidence from `research/`:

- the name of the opt-in variable of the execution switch, and the class of each Make target (FR65);
- for each JavaScript-based tool whether it runs from a pinned image or on Node.js, and the minimum version of every host prerequisite (FR69, NFR37);
- the source of `info.version` (FR73);
- the capability names of the targets in FR63 other than `mutation-tests`, and of the service commands other than `migrate` and `check-config`;
- the names of the listening-port variable and of the other runtime variables (FR3), and the accepted character set of the request id (FR10);
- compose service names and the on-disk spelling of the module and layer names;
- refinements of the layer meanings of FR36, each with a recorded reason, and the per-layer third-party allow-lists of FR38;
- the final value under every ceiling (PD-15), the default and maximum page size and the sort direction of the list (FR22), and the Dependabot cadence (NFR42);
- the problem `type` value of each error class, the GraphQL error codes (FR24) and the routes of the documentation assets and of the OpenAPI document the page renders (endpoint table);
- the source directives of the two HTML pages and the third-party origin of the explorer (FR9, FR11);
- the static-analysis rule set (FR41) and the complexity limits (NFR17);
- the protected list, the section-level comparison of protected sections, the required directories, the name of the approval label and the optional approval variable for local runs (FR44, NFR26);
- the suites that judge mutants and the content of the mutation scope record (FR59), and the seed recorded for pull-request runs (FR51);
- the stack used by `make schemathesis-validate` and how the default of `SCHEMATHESIS_URL` is honoured (FR76);
- the grouping of gates into CI jobs (FR93) and the documentation path list (FR94);
- the exact paths in `.templatesyncignore`, and for each template-owned file that a service extends whether it is ignored or how it is merged (FR100);
- whether a breaking change below `1.0.0` raises the major or the minor version (FR98);
- the secrets-scanning and image-scanning tools, each with a Make target or a documented reason for being CI-only (FR47, FR48);
- whether the Structurizr viewer starts by default or through a profile (FR88), and how model components are matched to workspace layers (FR120);
- the instance defaults of the AWS load run (FR85);
- whether the Spectral response rules are tightened (catalog note on CI:D-28): decided, not tightened (architecture-delivery.md, row `.spectral.yaml`);
- the OpenAPI title, description and operation ids (FR71);
- the skill names (FR108) and the format of the request collection (FR118).

## Traceability

The index below is derived from the traces at the end of each requirement.

### Catalog areas to requirements

| # | Catalog area | Ids to deliver | Functional requirements | Non-functional requirements |
| --- | --- | --- | --- | --- |
| 1 | Service runtime and HTTP surface | 12 PHP, 6 new | FR1-FR13, FR20, FR24, FR28, FR87 | NFR4, NFR5, NFR10, NFR15, NFR31, NFR36 |
| 2 | Health and liveness | 9 PHP, 0 new | FR14-FR18, FR126 | NFR2, NFR30 |
| 3 | Example bounded context | 1 PHP, 4 new | FR19-FR24, FR57 | none |
| 4 | Shared kernel | 6 PHP, 1 new | FR25-FR29 | NFR28 |
| 5 | Persistence, cache and messaging | 6 PHP, 2 new | FR30-FR35, FR90, FR121 | NFR29, NFR32 |
| 6 | Architecture and its enforcement | 5 PHP, 2 new | FR36-FR39, FR122 | NFR18 |
| 7 | Formatting, static analysis and complexity | 14 PHP, 1 new | FR40-FR44, FR123 | NFR16, NFR17, NFR25, NFR26 |
| 8 | Dependency and security checks | 6 PHP, 3 new | FR45-FR49, FR122, FR124, FR125 | NFR9-NFR11, NFR13 |
| 9 | Unit and integration tests | 10 PHP, 0 new | FR50-FR54 | NFR28, NFR29 |
| 10 | End-to-end tests | 5 PHP, 0 new | FR55-FR57 | NFR30 |
| 11 | Coverage and mutation testing | 5 PHP, 0 new | FR58, FR59 | NFR19, NFR20, NFR41 |
| 12 | Makefile command surface and its tests | 16 PHP, 2 new | FR60-FR70, FR125 | NFR21, NFR22, NFR34-NFR37, NFR45 |
| 13 | Contract artifacts and contract checks | 12 PHP, 0 new | FR71-FR76, FR96 | NFR7, NFR24 |
| 14 | Load testing | 15 PHP, 0 new | FR77-FR85 | NFR1, NFR25 |
| 15 | Containers and compose stacks | 18 PHP, 2 new | FR45, FR86-FR92, FR125 | NFR3, NFR12, NFR13, NFR33, NFR35, NFR37, NFR38, NFR40 |
| 16 | CI workflows | 3 PHP, 5 new | FR93-FR95, FR97, FR125 | NFR8, NFR13, NFR14, NFR34, NFR39, NFR40, NFR42 |
| 17 | Release, dependency updates and template sync | 6 PHP, 0 new | FR96, FR98-FR101 | NFR8, NFR38, NFR42 |
| 18 | Git hooks and commit convention | 4 PHP, 0 new | FR102, FR103, FR114 | none |
| 19 | AI-assistant guidance | 24 PHP, 3 new | FR92, FR104-FR113, FR120 | NFR26, NFR38, NFR43, NFR44 |
| 20 | Human documentation | 7 PHP, 10 new | FR113-FR118, FR125 | NFR6, NFR45-NFR47 |
| 21 | Architecture model (Structurizr) | 1 PHP, 1 new | FR119, FR120 | NFR18 |

### Catalog ids to requirements

- **1. Service runtime and HTTP surface.** CAP-01: FR1; CAP-02: FR2; CAP-03: FR3, NFR10; CAP-04: FR5, FR28; CAP-05: FR87; CAP-06: FR6; CAP-14: FR6, FR7, NFR15; CAP-18: FR8; CAP-19: FR9, FR24; CAP-20: DROP; CAP-34: FR7, FR20; CAP-35: DROP; CAP-36: DROP; CAP-DM-18: FR1; CAP-DM-19: DROP; CAP-CI-15: FR4, NFR36; NEW-01: FR10, NFR31; NEW-02: FR10, NFR15, NFR31; NEW-03: FR11, NFR15; NEW-04: FR12, NFR5; NEW-05: FR4; NEW-06: FR13, NFR4.
- **2. Health and liveness.** CAP-07: FR14; CAP-TQ-10: FR14; CAP-08: FR15, FR126, NFR30; CAP-09: FR16; CAP-10: FR16; CAP-11: FR16; CAP-12: FR17, NFR2; CAP-DM-20: FR18, FR126; CAP-CI-31: FR18.
- **3. Example bounded context.** CAP-13: FR19, FR22, FR57; NEW-07: FR19, FR20; NEW-08: FR21, FR22; NEW-09: FR23; NEW-10: FR24.
- **4. Shared kernel.** CAP-25: FR25; CAP-26: FR25, NFR28; CAP-27: FR26; CAP-28: FR27; CAP-29: FR28; CAP-30: FR29; NEW-11: FR27.
- **5. Persistence, cache and messaging.** CAP-21: FR30, FR90, NFR29; CAP-22: FR32, FR121, NFR32; CAP-DM-09: FR32; CAP-23: FR31; CAP-31: FR35; CAP-32: FR34; CAP-33: DROP; NEW-12: FR31, FR32, FR121; NEW-13: FR33, NFR32.
- **6. Architecture and its enforcement.** CAP-37: FR36; CAP-15: FR37; CAP-38: FR38, FR122, NFR18; CAP-CI-08: FR38; CAP-39: FR38; NEW-14: FR38, NFR18; NEW-15: FR39, NFR18.
- **7. Formatting, static analysis and complexity.** CAP-TQ-25: FR40, NFR16; CAP-TQ-20: FR41, NFR16; CAP-CI-09: FR41; CAP-TQ-22: FR41, FR123, NFR17; CAP-CI-11: FR41, FR123; CAP-TQ-23: FR41, NFR16; CAP-TQ-24: FR123, NFR17; CAP-TQ-26: FR42; CAP-CI-12: FR42; CAP-CI-34: FR43, NFR25; CAP-41: FR44, NFR26; CAP-TQ-31: FR44; CAP-DM-33: FR44; CAP-CI-33: FR44; NEW-20: FR43, NFR25.
- **8. Dependency and security checks.** CAP-CI-14: FR45, FR122, NFR9; CAP-TQ-30: FR46, NFR9; CAP-CI-16: FR46; CAP-TQ-21: FR49; CAP-CI-10: FR49; CAP-DM-05: FR125, NFR13; NEW-17: FR124, NFR9; NEW-18: FR47, NFR10; NEW-19: FR48, NFR11.
- **9. Unit and integration tests.** CAP-TQ-02: FR50, NFR28; CAP-TQ-03: FR50; CAP-TQ-04: FR51, NFR29; CAP-TQ-05: FR50; CAP-TQ-06: FR52; CAP-CI-03: FR52; CAP-TQ-07: FR53; CAP-DM-29: FR53; CAP-TQ-08: FR53; CAP-TQ-14: FR54; CAP-24: DROP; CAP-TQ-15: DROP.
- **10. End-to-end tests.** CAP-TQ-09: FR55, NFR30; CAP-CI-05: FR55; CAP-TQ-11: FR56; CAP-TQ-12: FR56, NFR30; CAP-TQ-13: FR57.
- **11. Coverage and mutation testing.** CAP-TQ-16: FR58, NFR19, NFR41; CAP-CI-04: FR58, NFR41; CAP-DM-32: FR58; CAP-TQ-17: FR59, NFR20; CAP-CI-07: FR59.
- **12. Makefile command surface and its tests.** CAP-DM-24: FR60, NFR45; CAP-DM-25: FR61, FR62, FR63; CAP-TQ-01: FR62, FR63; CAP-DM-23: FR62, FR63; CAP-DM-30: FR62, FR63; CAP-DM-31: FR63; CAP-DM-27: FR64; CAP-TQ-34: FR65, NFR36; CAP-DM-26: FR65, NFR36; CAP-TQ-32: FR66, FR125, NFR22; CAP-DM-28: FR66; CAP-CI-32: FR66, NFR22; CAP-TQ-18: FR67; CAP-CI-06: FR67; CAP-TQ-19: FR67, NFR21; CAP-CI-36: FR68; NEW-21: FR69, NFR37; NEW-22: FR70, NFR34, NFR35.
- **13. Contract artifacts and contract checks.** CAP-16: FR71, FR73, NFR7; CAP-CI-19: FR71, FR73; CAP-17: FR71; CAP-DM-34: FR72; CAP-CI-23: FR72, FR73, FR96, NFR7; CAP-DM-35: FR74, NFR24; CAP-CI-20: FR74; CAP-DM-36: FR75, NFR24; CAP-CI-21: FR75; CAP-CI-24: FR75, NFR24; CAP-DM-37: FR76, NFR24; CAP-CI-22: FR76.
- **14. Load testing.** CAP-DM-38: FR77; CAP-DM-39: FR78; CAP-DM-40: FR79; CAP-DM-41: FR80, NFR1; CAP-DM-42: FR79; CAP-DM-43: FR81; CAP-DM-44: FR82; CAP-DM-45: FR77; CAP-DM-46: FR78; CAP-DM-47: FR83, NFR25; CAP-CI-18: FR83; CAP-CI-17: FR84, NFR1; CAP-DM-48: FR85; CAP-DM-49: FR85; CAP-DM-50: FR85.
- **15. Containers and compose stacks.** CAP-DM-01: FR86; CAP-DM-02: FR87, NFR12; CAP-DM-03: FR86; CAP-DM-04: NFR40; CAP-DM-06: NFR40; CAP-TQ-35: FR87; CAP-DM-07: FR87, NFR3; CAP-DM-08: FR88, NFR33; CAP-DM-10: FR45; CAP-DM-11: FR89; CAP-DM-12: FR87; CAP-DM-13: FR88, NFR35; CAP-CI-01: FR88; CAP-DM-14: FR88, NFR33; CAP-DM-15: FR88; CAP-DM-16: FR90; CAP-DM-17: FR89, NFR37; CAP-DM-21: FR91, NFR12; CAP-DM-22: DROP; NEW-23: FR92, FR125, NFR38; NEW-24: FR86, NFR13.
- **16. CI workflows.** CAP-TQ-33: FR93; CAP-CI-02: FR93; CAP-TQ-27: DROP; CAP-CI-13: DROP; CAP-CI-30: FR97, NFR8; NEW-25: NFR40; NEW-26: NFR39, NFR40; NEW-27: FR125, NFR13, NFR14; NEW-28: FR94, NFR39; NEW-29: FR95, NFR34, NFR42.
- **17. Release, dependency updates and template sync.** CAP-CI-25: FR96, FR98, NFR8; CAP-AD-29: FR98; CAP-CI-28: FR99, NFR42; CAP-CI-27: FR100, NFR8, NFR38; CAP-CI-35: FR101; CAP-AD-32: FR101.
- **18. Git hooks and commit convention.** CAP-TQ-28: FR102; CAP-CI-26: FR102; CAP-AD-27: FR102, FR114; CAP-TQ-29: FR103.
- **19. AI-assistant guidance.** CAP-AD-01: FR104, NFR43; CAP-AD-02: FR105; CAP-AD-03: FR106; CAP-AD-04: FR107, NFR44; CAP-AD-05: FR106; CAP-AD-06: FR106; CAP-AD-07: FR108, NFR44; CAP-AD-08: FR108; CAP-AD-09: FR108; CAP-AD-10: FR108; CAP-AD-11: FR108; CAP-AD-12: FR108; CAP-AD-13: FR108; CAP-AD-14: FR108; CAP-AD-15: FR108; CAP-AD-16: FR108; CAP-AD-17: FR109; CAP-AD-18: FR109; CAP-TQ-36: FR109; CAP-AD-19: FR109; CAP-AD-20: FR109, FR120; CAP-AD-21: FR108; CAP-AD-22: FR92, FR107, NFR38; CAP-AD-23: FR113; NEW-30: FR110, NFR43; NEW-31: FR111, NFR26; NEW-32: FR112.
- **20. Human documentation.** CAP-AD-24: FR113, NFR46; CAP-AD-25: FR113; CAP-AD-26: FR114; CAP-AD-28: FR115, NFR46; CAP-AD-30: FR125, NFR47; CAP-CI-29: FR116; CAP-AD-31: FR116; CAP-AD-33: DROP; NEW-33: FR117, NFR6; NEW-34: FR117; NEW-35: FR117, NFR45; NEW-36: FR117, NFR45; NEW-37: FR117; NEW-38: FR117; NEW-39: FR118; NEW-40: FR118; NEW-41: FR118; NEW-42: FR117.
- **21. Architecture model (Structurizr).** CAP-40: FR119, FR120; NEW-16: FR120, NFR18.

### Catalog ids not covered

No `KEEP`, `FIX`, `EXTEND` or `NEW` id is uncovered: all 185 such PHP ids and all 42 `NEW` ids appear in the index above. The seven ids the validation reviews reported as covered in name only are now delivered by a requirement that states the capability: CAP-DM-05 (NFR13, FR125), CAP-40 and CAP-AD-20 (FR120 compares the model with the workspace), CAP-AD-22 (FR107, FR92(c), NFR38), CAP-CI-27 (FR100), CAP-DM-37 and CAP-CI-22 (FR76).

The 11 `DROP` ids are excluded by design and no requirement covers them:

| Id(s) | Capability | Reason |
| --- | --- | --- |
| CAP-20 | Development-only error-page preview route | Part of HTML templating (Out of Scope: "Mercure, Vulcain, HTML templating, sessions") |
| CAP-35 | Translation and locale facility | Out of Scope: "Internationalisation (P20)" |
| CAP-36 | Server-side HTML templating and sessions | Out of Scope: "Mercure, Vulcain, HTML templating, sessions" |
| CAP-DM-19 | HTTPS, HTTP/3 and compression in the service container | Out of Scope: "Reverse proxy, in-process TLS, HTTP/3"; request logging moves into the service (FR10) |
| CAP-33 | Configuration slot for an asynchronous transport | Out of Scope: "Asynchronous message consumer / worker" |
| CAP-24, CAP-TQ-15 | Empty data-fixture loader and its target | Scope principles 5 and 7: dead scaffolding; tests create their own data |
| CAP-DM-22 | Debugger call-back wiring to the host IDE | PHP-specific mechanism; the dev container covers in-container debugging (FR118) |
| CAP-TQ-27, CAP-CI-13 | Bot that commits formatting fixes to `main` | Scope 3.1: no bot commits other than the release identity; replaced by the failing format check (FR40, FR96) |
| CAP-AD-33 | A new quality-audit document | Scope principle 7; the PHP audit is consumed as input (table below) |

### Audit problems P1 to P28

Disposition text is from `scope-decisions.md` section 6. "Not carried" names the parts of the audit's problem statement that this release does not deliver; where the disposition already names them, the cell repeats them, and where it does not, the reason is that the item is not listed in scope-decisions 3.1 (scope principle 7).

| Problem | Disposition | Requirements | Not carried |
| --- | --- | --- | --- |
| P1 logging and correlation IDs | In scope | FR10, FR23, NFR31 | none |
| P2 observability | Partly: liveness/readiness split and dependency-naming failure response in scope; tracing and metrics deferred | FR15, FR18, FR117, FR126 | Tracing and metrics exporters, Prometheus endpoint (deferred; the seam is documented, FR117(g)) |
| P3 authentication | Non-goal | none (see "Out of Scope") | All of it (see "Out of Scope") |
| P4 secrets | In scope: nothing secret-shaped committed, env reference, secrets scanning | FR3, FR47, FR90, FR117, NFR10 | none |
| P5 hardening | Security headers in scope; rate limiting non-goal | FR11, NFR15 | Rate limiting (non-goal) |
| P6 supply chain | Advisory/licence/source policy, image vulnerability scan, hardened runtime in scope; OSSF Scorecard follow-up | FR46, FR48, FR87, FR91, NFR9, NFR11, NFR12 | OSSF Scorecard (follow-up) |
| P7 release artifacts | Non-goal | none (see "Out of Scope") | All of it (see "Out of Scope") |
| P8 mutation threshold | In scope | FR59, NFR20 | none |
| P9 coverage floor, one coverage job | In scope | FR58, NFR19, NFR41 | none |
| P10 deterministic strict tests | In scope | FR51, FR55, FR57, NFR29, NFR30 | A flaky-test policy in the contributing guide (not in scope 3.1) |
| P11 strict static analysis | In scope | FR41, FR123, NFR16 | none |
| P12 governance | PR-title convention check and no bot pushes in scope; CODEOWNERS and rulesets follow-up | FR96, FR102 | CODEOWNERS and rulesets (follow-up) |
| P13 SECURITY.md | In scope | FR115, NFR46 | none |
| P14 runbooks | Migration rollback command and operations notes in scope | FR32, FR33, FR117, NFR32 | none |
| P15 HA reference | Statelessness, graceful shutdown, limits and restart policy in scope; orchestrator manifests non-goal | FR12, FR91, FR117, NFR5, NFR6 | Orchestrator manifests and autoscaling examples (non-goal) |
| P16 resilience | Timeouts on every outbound call in scope; the rest non-goal | FR13, FR17, NFR2, NFR4 | Outbox, idempotency keys, chaos tests, kill switches (non-goal) |
| P17 CI cost | In scope | FR94, NFR39-NFR41 | none |
| P18 autonomous maintenance | Scheduled checks and wider Dependabot in scope; auto-merge and stale bot follow-up | FR95, FR99, NFR42 | Auto-merge and stale bot (follow-up) |
| P19 performance budgets | Realistic budgets and uploaded artifacts in scope; baselines follow-up | FR80, FR82, FR84, NFR1 | Stored baselines and trend comparison (follow-up) |
| P20 i18n | Non-goal | none (see "Out of Scope") | All of it (see "Out of Scope") |
| P21 dev environment | Dev container, `.editorconfig`, no submodules, `doctor` in scope; multi-arch follow-up | FR67, FR69, FR118, NFR37 | Multi-architecture builds (follow-up); Windows guidance beyond the WSL 2 statement of NFR37 (not in scope 3.1) |
| P22 in-repo docs | In scope | FR70, FR113, FR116, FR117, NFR45, NFR46 | A code of conduct in the repository (not in scope 3.1) |
| P23 scaffolding | In scope | FR38, FR39, FR117 | none |
| P24 contract enforcement | In scope | FR73, FR75, FR86, NFR7, NFR24 | An API versioning and deprecation policy and an upgrade guide (not in scope 3.1); whether the API version follows the release version is left to the architect (scope-decisions section 4, FR73) |
| P25 debugging tooling | GraphQL explorer in dev and request collection in scope | FR9, FR118 | none |
| P26 hygiene | In scope | FR68, FR100, FR125, NFR13, NFR14, NFR47 | none |
| P27 AI-native hardening | In scope | FR44, FR110, FR111, NFR26, NFR43 | none |
| P28 CQRS exemplars | In scope through the example slice | FR19-FR22, FR27, FR31 | none |
