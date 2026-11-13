---
stepsCompleted:
  - step-01-document-discovery
  - step-02-prd-analysis
  - step-03-epic-coverage-validation
  - step-04-ux-alignment
  - step-05-epic-quality-review
  - step-06-final-assessment
inputDocuments:
  - _bmad-output/planning-artifacts/scope-decisions.md
  - _bmad-output/planning-artifacts/product-brief.md
  - _bmad-output/planning-artifacts/prd.md
  - _bmad-output/planning-artifacts/architecture.md
  - _bmad-output/planning-artifacts/architecture-delivery.md
  - _bmad-output/planning-artifacts/epics.md
  - _bmad-output/planning-artifacts/research.md
  - _bmad-output/planning-artifacts/research/capability-catalog.md
  - _bmad-output/implementation-artifacts/architecture-revision-changes.json
  - _bmad-output/implementation-artifacts/story-index.json
workflowType: 'implementation-readiness'
project_name: 'rust-service-template'
date: '2026-11-11'
overallStatus: 'READY WITH CONDITIONS'
blockingIssues: 0
---

# Implementation Readiness Assessment Report

**Date:** 2026-11-11
**Project:** rust-service-template
**Assessor:** BMAD Architect persona (`implementation-readiness`), acting as an independent reviewer who wrote none of the assessed artifacts
**Overall status:** READY WITH CONDITIONS

Produced by the BMAD `implementation-readiness` workflow (steps 1 to 6) in an autonomous run. Every count in this report was taken from the artifacts themselves or computed from them with scripts written for this review: the coverage map against the `Implements` lines, the dependency graph, the wave plan, file ownership against both file manifests, Make target names, stale names, and a parse of `epics.md` with bmalph's own story parser. Where a statement rests on reading instead of a computation, it says so.

Short result: the PRD, the two architecture documents and the epics agree with each other; all 204 requirement ids (FR1 to FR126, NFR1 to NFR47, G1 to G31) are traced to stories; the dependency graph has no forward reference; no name removed by the architecture revisions survives in a story. Nothing blocks the start. Two points have to be settled before the first story (F-01, F-02), and a set of constraints applies during implementation (section "Constraints for the Implementation Loop"). What only GitHub can prove is listed under "Risks and What Cannot Be Verified Before the First Push or Merge".

## Document Inventory

### Files assessed

All paths are relative to the repository root. No document exists in both a whole and a sharded form, so there is no duplicate to resolve.

| File | Role | Size | Status |
| --- | --- | --- | --- |
| `_bmad-output/planning-artifacts/scope-decisions.md` | Binding orchestrator decisions, sections 1 to 9 (section 7: bmalph format rules; section 9: model allocation) | 27,774 bytes, 484 lines | Binding, living record |
| `_bmad-output/planning-artifacts/product-brief.md` | Product brief: users, journeys J1 to J8, indicators K-01 to K-18 | 42,895 bytes, 404 lines | Final |
| `_bmad-output/planning-artifacts/research.md` | Research digest with the pitfall checklist PF-01 to PF-66 | 89,402 bytes, 679 lines | Final |
| `_bmad-output/planning-artifacts/research/capability-catalog.md` | 196 PHP capability ids, 42 additions, contracts to preserve, 150-line defect digest | 121,565 bytes, 883 lines | Final |
| `_bmad-output/planning-artifacts/research/php-*.md`, `rust-*.md` | Nine raw research reports (five PHP inventories, four Rust reports with spikes) | 813,179 bytes in total | Working material, not tracked |
| `_bmad-output/planning-artifacts/prd.md` | Requirement baseline: FR1 to FR126, NFR1 to NFR47, G1 to G31, PD-1 to PD-31 | 152,055 bytes, 975 lines | Final (one amendment from the architecture round) |
| `_bmad-output/planning-artifacts/architecture.md` | Core architecture: AD-01 to AD-27, 13 crates, QG-01 to QG-20, test architecture, requirement mapping | 319,682 bytes, 3,478 lines | Final, revised after validation |
| `_bmad-output/planning-artifacts/architecture-delivery.md` | Delivery architecture: DD-01 to DD-25, 107 Make targets, 24 scripts, 16 Bats files, 11 workflows, file manifest | 267,235 bytes, 2,449 lines | Final, revised after validation |
| `_bmad-output/planning-artifacts/epics.md` | 10 epics, 57 stories, coverage map, waves and exclusive resources | 286,619 bytes, 3,074 lines | Final, revised after validation |
| `_bmad-output/implementation-artifacts/story-index.json` | Machine-readable story index (57 stories, 14 waves, 9 exclusive resources) | 117,900 bytes | Generated from `epics.md` |
| `_bmad-output/implementation-artifacts/architecture-revision-changes.json` | Names, commands, values, files and counts changed by the two architecture revisions; summaries; open questions | 27,110 bytes | Final |
| UX design document | none | - | Not applicable (see "UX Alignment Assessment") |

bmalph format (scope-decisions section 7), checked by running bmalph's transition code on the directory: the preflight passes with 0 errors and 0 warnings; the PRD has the four fixed headings; `## Tech Stack` yields the cargo commands; `epics.md` parses into 57 stories in 10 epics with 0 parser warnings, 339 acceptance criteria (4 to 8 per story) and no heading inside a story.

### How the artifacts were produced and validated

| Stage | BMAD command | Artifact | Validation and outcome |
| --- | --- | --- | --- |
| Analysis | `analyst`, `technical-research` | Nine raw reports; `research.md`; `capability-catalog.md` | The catalog states its totals as verified by counting: 196 PHP ids (185 to deliver, 11 dropped with a reason), 42 additions, 188 source defects reduced to 150 digest lines, 26 analyst decisions. The Rust reports ran their spikes on rustc 1.97.0 |
| Analysis | `create-brief` | `product-brief.md` | Input to the PRD; the artifact records no separate validation round |
| Planning | `create-prd`, `validate-prd`, `edit-prd` | `prd.md` | Two validation reviews with 70 findings, applied by `edit-prd`: FR121 to FR126 and G29 to G31 added, bundled requirements lettered, every NFR given a verification method (front matter `editHistory`) |
| Solutioning | `create-architecture`, `validate-architecture` | `architecture.md` | Three independent reviews (requirements against the final PRD, Rust feasibility on a compiled 13-crate skeleton with 149 source files, delivery and CI). 54 findings touch the core document: 53 applied, 1 not applied (DV-04, the time budgets). The document lists 8 findings as applied in an adapted form; the revision summary says 9 (F-06) |
| Solutioning | same round | `architecture-delivery.md` | 53 findings, all processed and applied, 20 of them in an adapted form (revision summary). The disposition table of the document names all 53 ids |
| Planning | amendment | `prd.md` | One amendment from the architecture round, ids unchanged: FR125(b) and NFR13 exempt the PostgreSQL image of the compose stacks from the digest pin (a digest overrides the tag, verified by running) |
| Solutioning | `create-epics-stories`, `validate-epics-stories` | `epics.md` | The draft had 55 stories and was written against the core document in mid-revision and the delivery document before its revision. The final version has 57: Story 9.11 and Story 10.4 are new through splits, six stories are retitled (1.2, 1.3, 8.5, 9.4, 9.6, 9.10). The document states the corrections it applies but no finding count |
| Solutioning | `implementation-readiness` | this report | Independent assessment, findings F-01 to F-16 |

## PRD Analysis

### Requirement counts

| Item | Count | Note |
| --- | --- | --- |
| Functional requirements | 126 (FR1 to FR126) | 29 of them have lettered clauses, 149 clauses in total; FR110 has 7 numbered rules, FR125 has 8 lettered rules |
| Non-functional requirements | 47 (NFR1 to NFR47) | Nine groups from Performance to Documentation and AI-readiness |
| Gates | 31 (G1 to G31) | Every gate has a pass condition, an FR and an NFR; G28 is the only gate without a Make target of its own |
| PRD decisions | 31 (PD-1 to PD-31) | Assumptions A-1 to A-8; 23 topics delegated to the architect |
| Journeys and indicators | 8 (J1 to J8) and 18 (K-01 to K-18) | Each indicator names its NFR |
| Traceability | 196 PHP ids and 42 additions mapped; P1 to P28 dispositioned | 11 `DROP` ids listed with reasons |

The PRD is complete and unusually testable: every requirement ends with its trace, numeric limits are ceilings with a named owner of the final value, and each of the 23 delegated topics is answered in one of the two architecture documents (checked through the mapping section of `architecture.md`).

### Requirements that are ambiguous or cannot be tested as written

None of these stops implementation. Each has a reading; the last column says where the reading is recorded.

| Requirement | Issue | Reading used by the plan | Recorded |
| --- | --- | --- | --- |
| NFR20, NFR23, NFR27, NFR40, NFR44, NFR46 | Each names "the implementation-readiness review" as its verification, but this review runs before any code exists | Judged on the plan here (next table); repeated on the repository after Story 10.5, and NFR40 after the first hosted rerun (F-11, C11, C12) | this report |
| NFR1 | A budget is three times the p99 "measured on a 4-vCPU GitHub-hosted runner"; no such run exists before the first push | The shipped budgets are the ceilings 500, 200, 1000 and 5000 ms; the smoke budget is derived after the first hosted run (DD-25) | delivery, reading 4 |
| NFR35, NFR39 | Initial budgets; the estimates are 44 to 56 minutes for `make ci` against 45, and 55 to 69 minutes for the clean-clone job against 75; "the Bats job within 20 minutes" is read per group | Budgets stay; revised only from measurements (DV-04 not applied) | delivery, readings 1 to 3 |
| NFR8, NFR34, NFR42 | Measured over workflow runs on GitHub | Verifiable only after the first push | delivery "Risks" |
| NFR31 | "process-level test ... of a forced panic": the built binary has no path that panics | Request sequence against the built binary; the panic in process with the production telemetry on a captured writer | core, readings |
| FR54 | Unit test with doubles for every probe; `PostgresProbe` wraps a concrete pool | Integration tests for success and unreachable; failure and timeout unit-tested in `HealthService` | core, readings |
| FR111 | "blocks edits to protected items": for a protected section the edit can only be judged on the result | A protected file is refused before the edit; a protected section is restored right after it | delivery, reading 8 |
| NFR13 | "tool" and "image" are not defined for development-container features and `apt` packages | Features by digest alone; `apt` packages governed by the digest-pinned base image | delivery, readings 6 and 7 |
| FR66(d), NFR22 | FR66(d) closes the list of CI-only steps (title check, approval labels, publications, a scanning gate, and the scheduled-only steps that FR95, FR118 and NFR35 require) and names the scheduled-only steps only through those requirements; the scheduled workflow runs the full sharded mutation run (FR95), the dev-container build with `docker info`, `make format-check` and `make start-deps` in it (FR95, FR118), and the stop and the timed start of the stack in `clean-bootstrap-ci` (NFR35), none of which `make ci` runs | These three are the scheduled-only steps: required by their own requirements and run only on `main` or on the schedule, they are CI-only steps listed with their reasons in the Make-target reference (DD-02); G31 rule (d) allows their targets, `format-check` as a gate target and `mutation-tests`, `start-deps`, `stop` and `start` through `ci_only_targets` | delivery, reading 11 |
| NFR16, FR125(g) | NFR16 said that each suppression applies to one item (amended after the review of pull request #2: an inline directive of these linters may apply to one file), and the definition of a blanket suppression names one that covers a whole crate; the inline directives of shellcheck, hadolint, yamllint, markdownlint, ESLint and zizmor belong to no crate, and several of their forms cover a whole file | A directive of these linters is blanket when it names no rule, a catch-all or several rules, or carries no reason (G31 rule (g)). A file-wide directive of one rule with a reason is allowed, because these linters have no item scope and a file is the narrowest scope several of their forms reach; Rust keeps its stricter rule, where architecture rule 15 allows a module-level `#![expect(lint, reason = ...)]` only in a file whose derive macro generates the offending code. The whole-project scope of these linters is their configuration file, which is a protected item | delivery, row (g) |
| FR6, "Data Schemas" | Lists `type`, `title`, `status`, `detail` without saying which members are required | `detail` is optional: absent on the 405 and on the generic 500 (AD-06 table) | not recorded as a reading (F-08) |
| FR22, error table | "a value that is not an integer" gives 400; the width of an integer is not stated | `page` and `itemsPerPage` are 32-bit; `page=4294967296` is a 400, an out-of-bounds 32-bit value a 422 (AD-06, Story 6.3) | not recorded as a reading (F-08) |
| FR92(d) | "the only two values the new-service guide tells a service to change" | Package identity and the checkout directory name; Story 8.5 then also names `bmalph/config.json` and the README badges, which are service-owned files | not recorded as a reading (F-08) |
| NFR23 | "0 configuration files that no command reads" | `.editorconfig` (FR118) and `.coderabbit.yaml` (FR97) are read by an editor and by an external App, not by a command that acts on their settings; both are required files, and the test "FR97: .coderabbit.yaml is the PHP template file with the request-changes workflow" of `make_general.bats` pins `.coderabbit.yaml` | not recorded as a reading (F-08) |

### Results for the requirements that name this review

The gate list says that the review records the result per requirement id.

| Requirement | Result on the plan | To repeat on the repository |
| --- | --- | --- |
| NFR20 | Planned mutation scope record `.cargo/mutants.toml`: 4 path entries (`**/main.rs`, `**/build.rs`, `**/tests.rs`, `**/tests/**`), 0 equivalent-mutant entries (AD-24) | Story 9.3 states the final number of `exclude_re` entries in its completion notes |
| NFR23 | 0 ports without an adapter and a caller, 0 shared-kernel items without a user in the example slice or the health check (core "Coverage of requirements", read against the crate tree); configuration files: see the reading above; tests without an assertion cannot be judged yet | after Story 10.5 |
| NFR27 | All 150 digest lines were read against the PRD and both architecture documents: 0 lines are replicated by design; the 2 lines accepted by decision are present as accepted (`/ping` as container health check; the issue number in the changelog scope); a third, the unchanged PHP Spectral ruleset (catalog note CI:D-28), was accepted by the architect after the C11 review of the repository (prd.md NFR27). Two lines keep an element in a changed form by a binding decision: `LOCALSTACK_PORT` stays as a compose port and the emulator gets the literal credentials `test` in the local compose file (scope-decisions section 4; FR3, FR90, G9) | after Story 10.5, by the final review |
| NFR40 | Cannot be judged: it needs the log of one rerun on GitHub. The delivery document already expects the `surface` Bats group to compile inside the development container without a restored Rust cache (F-10) | after the first push |
| NFR44 | 11 of 11 skill descriptions in the delivery skill table start with "Use when"; whether each named code pattern resolves to a file cannot be judged yet | after Stories 8.2 and 9.9 |
| NFR46 | Every document that journeys J1 to J8 need is in the file manifest (`docs/`, the three guides under `.github/`, `AGENTS.md`, the skills); Story 8.4 searches `SECURITY.md` for placeholders | after Story 10.5 |

## Epic Coverage Validation

### Method

1. The 204 ids were generated (FR1 to FR126, NFR1 to NFR47, G1 to G31) and looked up in the coverage map of `epics.md`.
2. The `Implements` line of every story was parsed and inverted, then compared with the coverage map and with `story-index.json`.
3. For 173 ids (every FR and NFR) one to eight probes for the distinguishing content of the requirement (a status code, a message, a command, a file name) were searched in the acceptance criteria of the stories the map names; every miss was then read by hand.

### Statistics

- Ids in the PRD: 204. Ids in the coverage map: 204. Ids missing from the map: 0. Ids in the map that the PRD does not have: 0.
- Ids named by at least one `Implements` line: 204 (100%). Differences between the map, the `Implements` lines and `story-index.json`: 0.
- Ids implemented by exactly one story: 63. This is expected for single-purpose requirements (for example FR12 in Story 7.6, FR110 in Story 9.9).
- Every gate except G28 has three kinds of story: the story in which it first passes, a Bats story with its Make-level negative test, and a workflow story with its CI check. G28 has matching and non-matching titles in Story 9.4 and its CI check in Story 10.2.

### Coverage matrix by PRD area

The per-id matrix is the coverage map of `epics.md`, which this review verified mechanically; the table condenses it. "Single" is the number of ids of the row that exactly one story implements.

| PRD area | Ids | Count | Stories | Single |
| --- | --- | --- | --- | --- |
| 1 Service runtime and HTTP surface | FR1 to FR13, FR126 | 14 | 4.3, 4.5, 5.2, 6.3, 7.1 to 7.6 | 6 |
| 2 Health and liveness | FR14 to FR18 | 5 | 5.1, 5.2, 7.3, 7.4, 7.5 | 3 |
| 3 Example bounded context | FR19 to FR24 | 6 | 4.1, 6.1, 6.2, 6.3, 7.4 | 2 |
| 4 Shared kernel | FR25 to FR29 | 5 | 4.1, 4.4, 6.1, 6.2, 7.5 | 1 |
| 5 Persistence, cache and messaging | FR30 to FR35, FR121 | 7 | 1.1, 3.1, 4.1, 4.4, 4.5, 6.1, 6.4, 7.2 | 2 |
| 6 Architecture and its enforcement | FR36 to FR39, FR122 | 5 | 1.1, 1.3, 2.1, 2.2, 2.4, 4.1, 6.1, 9.6 | 2 |
| 7 Formatting, static analysis and complexity | FR40 to FR44, FR123 | 6 | 1.1, 1.2, 1.4, 2.3, 9.10, 10.2 | 4 |
| 8 Dependency and security checks | FR45 to FR49, FR124, FR125 | 7 | 1.1, 1.2, 1.3, 3.3, 9.5, 9.8, 10.1, 10.2, 10.5 | 4 |
| 9 Unit and integration tests | FR50 to FR54 | 5 | 4.1, 4.2, 4.5, 5.1, 5.2, 6.1, 9.1, 9.2, 9.6, 10.3 | 0 |
| 10 End-to-end tests | FR55 to FR57 | 3 | 7.7 | 3 |
| 11 Coverage and mutation testing | FR58, FR59 | 2 | 9.2, 9.3, 9.6, 10.1 | 0 |
| 12 Makefile command surface and its tests | FR60 to FR70 | 11 | 1.2, 1.4, 9.4 to 9.7, 9.10, 9.11, 10.4, 10.5 | 4 |
| 13 Contract artifacts and contract checks | FR71 to FR76 | 6 | 3.2, 5.1, 6.3, 7.4, 9.1, 10.2 | 3 |
| 14 Load testing | FR77 to FR85 | 9 | 1.4, 3.4, 3.5, 9.1, 9.7, 10.2 | 5 |
| 15 Containers and compose stacks | FR86 to FR92 | 7 | 1.1, 1.2, 3.1, 7.6, 9.7, 9.8, 9.11 | 1 |
| 16 CI workflows | FR93 to FR97 | 5 | 8.4, 10.1, 10.2, 10.3, 10.5 | 2 |
| 17 Release, dependency updates and template sync | FR98 to FR101 | 4 | 3.6, 8.7, 10.3 | 3 |
| 18 Git hooks and commit convention | FR102, FR103 | 2 | 3.6, 10.2 | 1 |
| 19 AI-assistant guidance | FR104 to FR112 | 9 | 8.1, 8.2, 8.3, 8.8, 9.4, 9.9 | 7 |
| 20 Human documentation | FR113 to FR118 | 6 | 1.1, 3.1, 8.4 to 8.7, 10.3 | 4 |
| 21 Architecture model | FR119, FR120 | 2 | 3.7, 9.10 | 1 |
| NFR Performance | NFR1 to NFR3 | 3 | 3.1, 3.4, 5.1, 5.2, 7.6, 8.7, 9.1, 9.7, 10.5 | 0 |
| NFR Reliability | NFR4 to NFR8 | 5 | 4.5, 5.2, 6.4, 7.1, 7.4 to 7.7, 8.6, 10.1 to 10.3 | 1 |
| NFR Security and supply chain | NFR9 to NFR15 | 7 | 1.1, 1.3, 3.1, 3.3, 4.3, 7.3 to 7.5, 9.7, 9.8, 10.1 to 10.3, 10.5 | 1 |
| NFR Maintainability and quality gates | NFR16 to NFR27 | 12 | 26 stories of Epics 1, 2, 3, 7, 8, 9 and 10 | 1 |
| NFR Testability | NFR28 to NFR30 | 3 | 1.1, 4.2, 4.4, 5.1, 6.4, 7.7, 9.5 | 0 |
| NFR Operability | NFR31 to NFR33 | 3 | 3.1, 4.5, 6.4, 7.2, 7.6, 9.11 | 1 |
| NFR Portability and developer experience | NFR34 to NFR38 | 5 | 1.2, 1.3, 1.4, 3.1, 7.1, 9.4, 9.8, 9.11, 10.3 to 10.5 | 0 |
| NFR CI cost | NFR39 to NFR42 | 4 | 1.2, 3.1, 9.2, 10.1 to 10.5 | 1 |
| NFR Documentation and AI-readiness | NFR43 to NFR47 | 5 | 1.1, 7.4, 8.1, 8.2, 8.4 to 8.7, 9.8, 9.9 | 0 |

### Gates

| Gate | Name | First passes in | Negative test (Bats story) | CI check (workflow story) |
| --- | --- | --- | --- | --- |
| G1 | Formatting check | 1.2 | 9.5 | 10.1 |
| G2 | Static analysis | 1.2 | 9.5 | 10.1 |
| G3 | Complexity limits | 1.2 | 9.5 | 10.1 |
| G4 | Architecture boundaries | 2.1, 2.2 | 9.5 | 10.1 |
| G5 | Manifest and lock validation | 1.2 | 9.5 | 10.1 |
| G6 | Configuration check | 7.1 | 9.5 | 10.1 |
| G7 | Dependency policy | 1.3 | 9.5 | 10.1 |
| G8 | Unused dependencies | complete workspace (after 7.7) | 9.5 | 10.1 |
| G9 | Secrets scanning | 3.3 | 9.7 | 10.2 |
| G10 | Production-image scan | 3.3 | 9.7 | 10.2 |
| G11 | Unit tests, with documentation tests | 4.1 | 9.2, 9.6 | 10.1 |
| G12 | Integration tests | 4.5 | 9.2, 9.6 | 10.1 |
| G13 | End-to-end tests (Gherkin) | 7.7 | 9.6 | 10.1 |
| G14 | Coverage | 9.2 | 9.6 | 10.1 |
| G15 | Mutation testing | 9.3 | 9.6 | 10.1 |
| G16 | Bats tests of the Makefile | 9.4 | 9.4, 9.11 | 10.1, 10.4 |
| G17 | Contract snapshot equality | 7.4 | 9.7 | 10.2 |
| G18 | OpenAPI lint | 3.2 | 9.7 | 10.2 |
| G19 | OpenAPI breaking-change check | 3.2 | 9.7 | 10.2 |
| G20 | GraphQL schema diff | 3.2 | 9.7 | 10.2 |
| G21 | Schemathesis | 9.1 (early run in 7.6) | 9.7 | 10.2 |
| G22 | K6 smoke load | 9.1 | 9.7 | 10.2 |
| G23 | Repository lint | 1.4 | 9.10 | 10.2, 10.4 |
| G24 | Load-script lint | 3.4 | 9.7 | 10.2 |
| G25 | Protected-configuration guard | 2.3 | 9.5 | 10.2 |
| G26 | Documentation-drift check | 9.9 | 9.10 | 10.2 |
| G27 | Architecture-model validation | 3.7 | 9.10 | 10.2 |
| G28 | Pull-request title convention | 3.6 | 9.4 (title examples) | 10.2 |
| G29 | Crate isolation | 1.3 | 9.5 | 10.1 |
| G30 | Persistence query metadata | 6.4 | 9.6 | 10.1 |
| G31 | Repository invariants | 9.8 | 9.10 | 10.2, 10.4, 10.5 |

### Missing requirements

None. No FR, NFR or gate is without a story.

### Weakly covered requirements

Covered by an `Implements` line and by the architecture listing the story points to, but by no acceptance criterion that names the behaviour.

| Id | Clause | What is missing | Why it is not a gap in the product | Finding |
| --- | --- | --- | --- | --- |
| FR79 | "`API_HOST` and `API_PORT` override the target" | No criterion in any story names the two variables | `scripts/with-load-test-stack.sh` exports both, and the smoke run of Story 9.1 reaches port 18081 only through them | F-03 |
| FR40, FR42 | the fix targets (`format`, `format-repo`, `static-analysis-fix`, `unused-dependencies-fix`) | No criterion and no Bats test runs them | They exist through the listing equality of Story 1.2 (107 targets); the delivery document gives the reason (they rewrite tracked files) | F-04 |
| FR63(b), (d), (e), (g); FR63(a) "create a migration" | `build`, `sh`, `update`, `all-tests` with alias `test`; the passing path of `create-migration` | No criterion runs them; `build` is checked with `make -n`, `create-migration` only for its usage error | Same listing equality | F-04 |
| FR67(a) | "a target without a Bats test is listed with its reason" | The list in the delivery document ("Targets", last paragraph) names 17 targets; aliases (`up`, `clean`, `test`, `all`, `ci-sequential`) and helper targets (`ensure-deps`, `ensure-database`, `install`, `pre-push`, `check-security-offline`, `static-analysis`, `bmalph-claude`, `bmalph-codex`, the seven `ci-*` stages, `sh`, `all-tests`) are in neither the Bats table nor that list | Story 8.5 writes the list into `docs/make-targets.md` and can complete it | F-04 |
| FR9 | browser behaviour of the development-only explorer | Story 7.4 checks the page text and the policy constant; nothing loads the page | Development-only page; pin test stays | F-12 |

The other probe misses were wording differences and are covered. Examples: FR9 "not a GraphQL request" is the 422 criterion of Story 7.4 with the bodies `{}`, `[]`, `{"foo":1}` and `{"query":5}`; FR51 "prints the seed" is the `TEST_SEED=7` criterion of Story 4.2; FR56 is "a new application instance for every request" in Story 7.7; the 20-generation check of NFR7 is the integration test `export.rs` of Story 7.4, which the clean-clone job of `scheduled.yml` runs through `make ci`.

## Architecture Alignment

### PRD against the two architecture documents

The mapping section of `architecture.md` has a row for every FR (126), every NFR (47) and every gate (31); rows owned by the delivery document name its section and decision. 41 requirements from all 21 areas were checked by reading the cited sections and by searching for the exact values:

| Requirement | PRD value | Fixed in the architecture | Result |
| --- | --- | --- | --- |
| FR1 | port 8081, `APP_ENV` in `dev`, `test`, `prod`, default `prod` | `APP_PORT` 8081, `APP_ENV`; `listening` entry with `env` (core "Configuration") | aligned |
| FR2 | seven commands; `migrate` and `check-config` fixed names | `serve`, `migrate`, `migrate revert`, `check-config`, `healthcheck`, `openapi-export`, `graphql-export` | aligned |
| FR6, PD-1 | 400, 404, 405, 413, 415, 422, 500, 503 | AD-06 table; the 409 of the first version removed | aligned (reading on `detail`, F-08) |
| FR9, PD-27 | 422 for a body that is not a GraphQL request; 405 outside `dev`; pinned explorer assets from one origin | AD-18, AD-14; committed page with four pinned assets from `unpkg.com`; `EXPLORER_CONTENT_SECURITY_POLICY` | aligned; page not opened in a browser (F-12) |
| FR10, PD-2 | `X-Request-Id` accepted or generated, echoed, on every log entry | AD-13; `request completed` entry | aligned |
| FR11, PD-3 | four headers; `default-src 'none'; frame-ancestors 'none'` | AD-14 with the two page policies | aligned |
| FR12, NFR5, PD-21 | exit 0 when drained, non-zero when forced, within 2 s | AD-15; exit 1; run in the revision: exit 1 after 1.25 s with a 1 s grace period | aligned |
| FR13, NFR4 | ceilings 5 s, 5 s, 5 s, 30 s | acquire 2 s, statement 5 s, SQS 2 s, request budget 10 s; `SQS_MAX_ATTEMPTS` | aligned |
| FR15 | three exact messages | present character for character in core and in Stories 5.1, 5.2, 7.7 | aligned |
| FR16 | `SELECT 1`; key `health_check`, value `ok`; read-only broker call | AD-11; `ListQueues` with `max_results(1)` (AD-10) | aligned |
| FR20(e), PD-18 | 201 and an error entry with the event id | AD-08, `log_publication_failure` | aligned |
| FR22 | bounds, total order by `id`, maximum at most 100 | signed 32-bit parameters; default 30, maximum 100; `id` descending | aligned (reading on integer width, F-08) |
| FR24 | exactly three operations; stable codes | `VALIDATION_FAILED`, `INTERNAL_SERVER_ERROR`; SDL snapshot | aligned |
| FR26, PD-19 | every subscriber called; every failure returned | `EventBusError { failures }`, `SubscriberFailure` | aligned |
| FR32, FR33 | reversible migrations; explicit `migrate`; one rollback command | `migrations/20261110000000_create_example_api_resources.{up,down}.sql`; `migrate revert`; one-shot `migrate` compose service | aligned |
| FR38, NFR18 | six failure kinds; rules by layer position | eight rules of `cargo xtask arch` driven by `architecture.toml` | aligned |
| FR44, NFR26 | section-level protection, overrides, label, local variable | `protected-config.toml` (`files`, `directories`, `manifest_tables`, `json_values`, `override_names`, `override_exempt`, `required_directories`); label `approved-config-change`; `CONFIG_CHANGE_APPROVED` | aligned |
| FR45 | lock file enforced on every command | `--locked` on every one of QG-01 to QG-20 that takes it | aligned |
| FR51 | recorded seed by default, fresh seed on the schedule | `TEST_SEED` and `PROPTEST_RNG_SEED` = `20261110` in `.cargo/config.toml`; `github.run_id` in `scheduled.yml` | aligned |
| FR58, NFR19, PD-28 | unit and integration suites only; one job | coverage sequence without the end-to-end suite; exclusion record in the protected `make/gates.mk`, 0 entries; job `tests` | aligned |
| FR59, NFR20 | judging suites, scope record, timeouts survive, changed-code mode | AD-24; `.cargo/mutants.toml`; exit status 2 and 3; diff from the merge base | aligned |
| FR62 | 42 PHP target names | all 42 are among the 107 targets (computed) | aligned |
| FR65 | one opt-in variable; `CI=true` selects direct mode | `RUN_IN_CONTAINER`, `IN_DEV_CONTAINER`, `EXEC` with four forwarded variables (DD-01) | aligned |
| FR66 | fixed order; success line | `doctor`, then seven `ci-*` stages; `CI checks successfully passed!` (DD-02) | aligned |
| FR74, FR75, FR76 | `--fail-severity=hint`; label `approved-breaking-change`; Schemathesis flags | `SPECTRAL_FLAGS`, `OASDIFF_FLAGS = --fail-on ERR`, `--checks all --phases=examples,coverage` in `make/gates.mk`; first-version notice (DD-10) | aligned |
| FR78, NFR1 | scenario `health`; ceilings 500, 200, 1000, 5000 ms | one scenario; budgets equal to the ceilings until measured (DD-25) | aligned, measurement open |
| FR80 | `Invalid threshold value` | kept in the kit; Bats negative | aligned |
| FR87, NFR12, NFR3 | non-root, no shell, at most 100 MiB; health check limits | distroless `cc-debian13:nonroot` by digest, uid 65532; interval 10 s, timeout 3 s, 3 retries, start period 10 s; 43.7 MB measured for a scratch workspace | aligned |
| FR88 | four compose file names | kept; Structurizr through a profile that is on locally (DD-05) | aligned |
| FR94 | G9, G23, G25, G26, G28, G31 run for documentation-only changes | `changes` job with `code=false`; Story 10.2 criterion | aligned |
| FR95, NFR42 | weekly scheduled run with the listed content | cron `23 4 * * 1`; `clean-bootstrap-ci` with timeout 75; full mutation run; security gates; development-container build | aligned |
| FR98 | version rules, `chore(release): vX.Y.Z [skip ci]`, skip without secrets, first version `0.1.0` | cocogitto flow (DD-11); `scripts/release.sh` | aligned |
| FR99, FR100 | six ecosystems; schedules `0 0 1 * *` and `0 9 * * MON`; label `template_sync`; one open sync pull request (FR100(b): `is_force_push_pr` updates it in place, `is_pr_cleanup` closes the older ones and `is_keep_branch_on_pr_cleanup` keeps their branches); self-guard | Dependabot file; both sync workflows; `.templatesyncignore` (DD-12, DD-14) | aligned |
| FR102 | the commit regular expression; exemptions | `scripts/check-commit-message.sh`; `title` job (DD-15) | aligned |
| FR110 | seven rules | `scripts/check-docs.sh` (DD-17); rule (5) reads five sources | aligned |
| FR111 | allow, ask and block lists from one source | `.claude/settings.json`, `scripts/claude/guard-protected-files.sh` with `pre` and `post` (DD-18) | aligned (reading) |
| FR120 | names every missing and extra component | `scripts/validate-architecture-model.sh`; six rule kinds tested on a fixture model | aligned |
| FR125 | rules (a) to (h) | `scripts/check-invariants.sh` with one Bats test per rule | aligned |
| NFR13 | pins; one toolchain version in three places | `1.99.1` in `rust-toolchain.toml`, `rust-version` and the builder tag; LocalStack `3.4.0`; 12 actions by SHA; images by tag and digest | aligned (PRD amended for the PostgreSQL image) |
| NFR17 | ceilings 10, 100, 5, 5 | cognitive complexity 8, 50 lines, nesting 5, 5 parameters (`clippy.toml`, QG-20) | aligned |
| NFR33 | at most 60 attempts 1 s apart | `DATABASE_WAIT_TIMEOUT` 30 s, one attempt per second | aligned |

### Decisions and requirements flagged by the validation round

| Item | PRD | Core | Delivery | Stories | Result |
| --- | --- | --- | --- | --- | --- |
| PD-18, FR20(e) | 201 after a failed publication; error entry with the event id | AD-08; `event publication failed` at `ERROR` | none | 6.2 (handler), 6.3 (201 at handler level), 7.5 (201 through the wiring test) | aligned |
| PD-21, FR12, NFR5 | non-zero exit when the grace period ends first | AD-15; exit 1 within 2 s | `docs/operations.md` content | 7.6 (`SHUTDOWN_GRACE_PERIOD=1s`, status 1), 8.7 | aligned |
| PD-28, FR58(a) | end-to-end suite does not count | AD-24; QG-16 sequence | `COVERAGE_*` group, job `tests` | 9.2 (no end-to-end run, no exclusion flag), 10.1 | aligned |
| FR44 | (a) to (f) | AD-25; nine decision steps; guard output lines | DD-16; `change-control.yml` on `labeled` and `unlabeled` | 2.3 (every case of the FR44 test row), 9.5 (Make-level negative), 10.2 (label evaluation) | aligned |
| FR51 | recorded seed; fresh on the schedule | `.cargo/config.toml` `[env]` | `EXEC_ENV` forwarding; `scheduled.yml` | 1.1, 4.2, 9.11, 10.3 | aligned |
| FR58 | (a) to (d) | QG-16 | `coverage-xml`, `coverage-html`, Codecov step without the token | 9.2, 10.1 | aligned |
| FR59 | (a) to (e) | AD-24; QG-17, QG-18 | `scripts/mutation-diff.sh`, `scripts/ci/mutation-plan.sh` (one shard per 150 mutants, at most 8) | 9.3, 9.6, 10.1, 10.5 | aligned; shard figures of the core are stale (F-05) |
| NFR13 | pins, with the PostgreSQL exception | toolchain, explorer assets with digests | `make/tool-versions.mk`, action pins, G31 rules (a) to (c) | 1.1, 1.3, 3.1, 7.4, 9.8, 10.1 to 10.3 | aligned |

### Core against delivery

- **Gate commands.** The 20 command lines of QG-01 to QG-20 and the coverage sequence were compared with the listing of `make/gates.mk` in the delivery document: equal, apart from the Make variables the delivery document adds on purpose (`$(ARCH_ARGS)`, `$(DENY_ARGS)`, `$(base_ref)`, `$(MUTANTS_SHARD)`).
- **Gate ids.** G3 is QG-20 (`complexity`), G29 is QG-08 (`check-crate-isolation`), G30 is QG-11 (`validate-sqlx-metadata`), G31 is `validate-invariants`; both documents say the same.
- **Names.** Binary `service`, the commands it has, the 32 environment variables of the core table, crate names and ports are used unchanged in the compose files, the `Dockerfile` and the scripts. Every name the revisions removed (`migrate status`, `E2E_JUNIT_FILE`, `BUILDER_BASE_IMAGE`, `DEV_ENV`, `MUTANTS_ARGS`, `arch::module`, the 409, the 23-scenario count and the others of `names_changed`) occurs in the two documents only in sentences that describe the removal.
- **Counts.** 107 targets in 13 groups (parsed from the target tables), 24 scripts, 16 Bats files with six group tags, 11 workflows, 13 crates, 8 architecture rules, 5 feature files with 24 scenarios and 97 steps: each stated count matches the listing it summarises.
- **One stale statement (F-05).** The core document still speaks of "four shards" of about 20 minutes (QG-17 runtime column, AD-24, the first row of its risk table), while the delivery document sizes the run as one shard per 150 mutants up to 8, and Story 9.3 runs eight shards. The sharding matrix belongs to the delivery document, so no command is affected.
- **Open questions of the revisions.** Of the 17 open questions in `architecture-revision-changes.json`, the epics settle the ones that concern them (the proxy build through a named build context, the new counts and names, the early Schemathesis run in Story 7.6). Still open for the orchestrator: the time budgets (F-09), the reading of NFR40 (F-10), the GitHub plan behind the estimate of about 55 pull-request jobs with 20 concurrent runners, and the explorer page (F-12).

### Stories against both architecture documents

- **Stale names.** About 60 names, values and counts from `names_changed` were searched in `epics.md`. None is used. The four that occur are in the sentences that say what this version removes (`migrate status`, `docs/php-to-rust.md`, `scripts/wait-for-service.sh`, the 409). The strings that look old but are current were checked against the core document: `waiting for database (attempt <n>): <error>` is the plain-text output of `service migrate`, and "newest first" is the documented meaning of `id` descending.
- **Make targets.** Every `make <target>` cited in a story is one of the 107 targets (the only other name is the deliberate `no-such-target`). 99 targets are named in a story; the other eight (`all-tests` and the seven `ci-*` stages) come with the listing of Story 1.2.
- **Files.** The tree of the core document (about 260 paths) and the manifest of the delivery document (about 200 paths) were parsed and matched with the `Owns` lines: every path has exactly one owning story, no path has two, and no `Owns` entry points outside the two documents. 358 `Owns` entries in total.
- **Touched files.** A story touches only files whose owner is an earlier story, with one stated exception: Story 1.4 formats `README.md` and `CLAUDE.md`, which exist before the first story and are rewritten by Stories 8.4 and 8.1.
- **Implicit dependencies.** For every Make target in a `Verify` line or criterion, the scripts and `xtask` commands of its recipe were resolved to their owning story and compared with the transitive dependencies of the citing story. No story runs a target whose script a later story writes. Story 7.6 uses `schemathesis-validate` with a caller-set `SCHEMATHESIS_URL`, the path that does not need the wrapper of Story 9.1.
- **External sources.** Seven stories read files of the PHP template or of `core-service`; all seven source paths exist on the implementation machine. In the tracked copy of `epics.md` they are written as `${PHP_TEMPLATE_DIR}` and `${CORE_SERVICE_DIR}`, which leads to F-01.

### F-01: the tracked copy of the epics and two Verify lines

The last commit of the planning branch replaced the machine paths of the two reference checkouts by placeholders in `specs/rust-service-template-bootstrap/epics.md` (7 lines) and explained them in the bundle README. The working copy `_bmad-output/planning-artifacts/epics.md`, which `bmalph implement` reads, still holds the machine paths. Stories 8.8 and 10.5 both require `cmp _bmad-output/planning-artifacts/epics.md specs/rust-service-template-bootstrap/epics.md` to exit 0 ("every comparison exits 0"; "the tracked planning bundle equals the final planning artifacts"). As the files stand, that command exits 1. If the implementer of Story 8.8 follows the story and copies the sources again, the machine paths return to a tracked file and the commit is undone without anyone deciding it.

Exact fix, before `bmalph implement`: make the working copy equal to the tracked one and define the two variables.

```sh
cp specs/rust-service-template-bootstrap/epics.md _bmad-output/planning-artifacts/epics.md
# regenerate story-index.json from the new epics.md (its verify and reference strings hold the same paths in 9 places)
export PHP_TEMPLATE_DIR=<checkout of php-service-template>
export CORE_SERVICE_DIR=<checkout of core-service>
```

This review confirmed that the two files differ in exactly those two substitutions (7 lines) and in nothing else, so the copy changes no other text. The variables must be exported in every implementer and verifier shell, because three `Verify` lines (Stories 3.2, 7.7 and 8.4, for example `cmp .spectral.yaml ${PHP_TEMPLATE_DIR}/.spectral.yaml`), one criterion (Story 1.4) and three `Reference` lines (Stories 3.4, 3.5 and 8.3) use them. An alternative with the same effect is to keep the machine paths in the working copy and to take `epics` out of the two `cmp` loops; the copy is the smaller change.

## UX Alignment Assessment

### UX document status

Not found, and not required. No `*ux*` file exists among the planning artifacts, and the product has no end-user interface: it is a template repository whose runtime deliverable is an HTTP API (PRD "Project Classification"). The only pages a browser renders are the API documentation page at `/api/docs` (FR8) and the development-only GraphQL explorer (FR9); both are third-party viewers configured by the service, with their policies fixed in AD-14, AD-18 and AD-19. `epics.md` states the same under "Additional Requirements".

### What stands in for UX

| Surface | Users | Specified in | Kept true by |
| --- | --- | --- | --- |
| Command surface: 107 Make targets, `make help`, `make doctor`, `make bootstrap`, the service command line | engineer, assistant | PRD "Command and Configuration Surface", FR60 to FR70; delivery "Command Surface" | Bats suite (G16), G26 rule (6), G31 rules (d) and (h) |
| API contract: REST and GraphQL, problem documents, headers | API client, reviewer | PRD "Endpoint Specification", "Data Schemas", "Error Codes"; core "API Contract" | snapshots (G17), Spectral (G18), breaking-change checks (G19, G20), Schemathesis (G21), Gherkin features (G13) |
| Messages a person reads: gate diagnostics, guard output, `doctor` lines, health failure messages, log schema | engineer, operator | core "Protected configuration", "Log line schema"; delivery script table | exact strings in acceptance criteria and Bats assertions |
| Documentation and guidance: `README.md`, `docs/`, `AGENTS.md`, `CLAUDE.md`, 11 skills, issue and pull-request templates | all four user groups | FR104 to FR118; delivery "AI Guidance and Documentation" | documentation-drift gate (G26, seven rules), Markdown lint and spelling (G23) |

### Alignment issues and warnings

- The eight journeys J1 to J8 of the PRD each have their steps covered by named targets, endpoints or documents (PRD "Journey Requirements Summary", checked against the target list and the file manifest). No journey step lacks a home.
- Warning, low: nothing in the plan loads the explorer page in a browser (F-12).

## Epic Quality Review

### Epic structure

| Epic | Stories | Beneficiary and outcome stated | Needs only earlier epics | Finding |
| --- | --- | --- | --- | --- |
| 1 Workspace Foundation and Command Surface | 4 | yes | yes (none) | enabling epic |
| 2 Architecture Enforcement and Scaffolding | 4 | yes | yes (1) | none |
| 3 Images, Local Stacks and Delivery Gate Tooling | 7 | yes | yes (1) | enabling epic |
| 4 Shared Kernel and Platform Adapters | 5 | yes | yes (1, 2, 3) | component epic; creates the first migration before its consumer in 6.4 |
| 5 Dependency Health Check | 2 | yes | yes (4) | none |
| 6 Example Bounded Context | 4 | yes | yes (2, 4) | none |
| 7 Service Runtime and Composition Root | 7 | yes | yes (3, 4, 5, 6) | none |
| 8 AI Guidance and Documentation | 8 | yes | yes (1, 2) | none |
| 9 Every Gate Proven Locally | 11 | yes | yes (2, 3, 7, 8) | two open-ended stories |
| 10 Hosted CI, Release and Maintenance Automation | 5 | yes | yes (3, 9) | workflows are the last files added |

Judged by the letter of the BMAD checklist, Epics 1, 3, 4 and 9 are organised by component and build order, not by a user journey, and the hosted pipeline arrives last instead of early (F-15). I record this as a justified deviation, not as a defect: the users of this product are engineers and assistants, the tooling is the product, the PRD declares that no feature group can be dropped from the release, each epic maps to the PRD groups F1 to F8, and every gate runs locally through `make` from Story 1.2 on. The cost of the choice is that all hosted-CI risk is concentrated after the last story; the risk section treats it.

Starter template: none is used (core "Starter Template Evaluation"), and Story 1.1 creates the workspace by hand from the architecture, as the greenfield rule asks.

### Dependencies

Computed from `story-index.json` and compared with the `Depends on` lines of `epics.md` (no difference):

- 57 stories, ids contiguous in every epic. Dependencies on a later or equal story id: 0. Unknown ids: 0. The graph has no cycle.
- Story-id order is therefore a valid order for a sequential loop: every dependency of a story has a smaller id.
- No epic depends on a later epic.
- Each story's wave equals one plus the highest wave of its dependencies; the 14 wave lists match.

### Sizing

| Measure | Value |
| --- | --- |
| Acceptance criteria | 339 in total; 4 to 8 per story, median 6 |
| Story text | 2,195 to 6,914 characters, mean 4,075 |
| Largest stories by text | 7.6, 7.4, 1.1, 1.4, 7.1 |
| Stories with 8 criteria | 6.3, 7.1, 7.4, 7.6 |
| Most owned paths | 1.1 (44, mostly manifests and stubs), 8.2 and 9.10 (13 each), 7.4 and 1.4 (12 each) |
| Most requirement ids | 10.2 (29), 10.1 (28), 1.2 (21) |

Most stories fit one session. The ones to watch:

- **Open-ended closers (F-13).** Stories 9.2 and 9.3 iterate until the workspace-wide coverage and mutation gates pass; Stories 10.4 and 10.5 may touch "any file whose gate still fails". Their effort is bounded only by the rule that every code story of Epics 2 and 4 to 7 already proves 100% lines and functions and zero missed or timed-out mutants for its own files. Story 9.3 also runs eight shards in sequence; the core document extrapolates 60 to 80 minutes for the whole mutant set, to which eight baselines of about 1.5 minutes are added, before any rerun.
- **Conditional scope.** Stories 7.6 and 9.1 may change the OpenAPI snapshot and files under `crates/` if Schemathesis finds a difference.
- **Dense workflow stories (F-14).** Stories 10.1 and 10.2 implement 28 and 29 ids in five and five files; before the first push their criteria can be checked only by actionlint, zizmor and reading.

### Acceptance criteria

- Form: every story has the user-story sentence, the seven metadata lines `Track`, `Depends on`, `Owns`, `Touches`, `Implements`, `Reference` and `Verify`, and `**Acceptance Criteria:**` followed by Given, When, Then blocks; the three keywords are balanced in all 339 criteria; 50 further `And` lines.
- Concreteness: 334 of 339 criteria contain at least one literal command, path, message or value. Expected exit statuses and output lines are quoted, including the error paths (missing base ref, unreachable database, absent tool, wrong parameter).
- Each code story ends with the same last criterion: 100% lines and functions and no missed or timed-out mutant for the production files it owns.
- Independence of the verifier: the `Verify` line is a list of commands; the preamble defines listing equality as a `diff` against the fenced block of the architecture and requires a deviation entry for every difference.
- Cosmetic (F-16): nine stories (3.5 and 8.1 to 8.8) have no `Pitfalls` line although the preamble lists it among the metadata.

### Parallel-safety

- The exclusive-resource table (9 resources) agrees in both directions with the `exclusive` keys of the stories.
- Same-wave pairs that share an exclusive resource exist in waves 4, 5, 6, 7, 10, 11 and 12; each wave has an authored order rule that serialises them.
- Files written by more than one story: the stories of every such file are in different waves. One computed overlap remains, Stories 9.1 and 9.2 in wave 11 under `crates/`, and the order rule puts 9.1 first for exactly that reason.
- The rules are written for one shared checkout with verification at the wave barrier. They are not valid with one worktree per story, as `epics.md` itself says. A purely sequential loop satisfies all of them trivially.

### Critical path

14 stories, one per wave: 1.1, 1.2, 4.1, 4.2, 4.3, 6.2, 6.3, 7.4, 7.5, 7.6, 9.2, 9.3, 10.4, 10.5. It is a real dependency chain (each story depends on its predecessor) and equals the longest path computed from the graph. Track A (19 stories of service code) is the spine and runs one story at a time; tracks B (4), C (10) and D (8) run beside it; track E (16) needs everything before it. Waves 11 and 12 hold twelve track E stories, five of which need the checkout alone (9.5, 9.6, 9.7, 9.10, 9.11), and Stories 10.4 and 10.5 need it alone as well, so the end of the plan is largely serial whatever the number of agents.

### Compliance summary

| Check | Result |
| --- | --- |
| Epics deliver value to a named user | yes, with the deviation recorded as F-15 |
| Epic N needs no later epic | yes (computed) |
| No forward story dependency | yes (computed) |
| Stories sized for one session | yes, with F-13 and F-14 |
| Tables created when first needed | one table, created in Story 4.5, first used in 6.4 (minor) |
| Clear, testable acceptance criteria | yes |
| Traceability to requirements | yes, 204 of 204 |
| Critical violations | 0 |
| Major issues | 2, both outside the story text: F-01 and F-02 |
| Minor concerns | F-03 to F-16 |

## Risks and What Cannot Be Verified Before the First Push or Merge

Nothing is pushed before Story 10.5 is verified, and GitHub Actions cannot run on the implementation machine. The rows are ordered by how much of the product they touch. "Planned mitigation" names only what the artifacts already contain.

| Item | Why it cannot be verified earlier | Planned mitigation |
| --- | --- | --- |
| The 11 workflow files as running pipelines | Workflows run only on GitHub | actionlint and zizmor are part of G23 and of the criteria of Stories 10.1 to 10.3; every gate step is a Make target that Stories 10.4 and 10.5 run locally; Story 10.5 replays the bootstrap pull-request case with the root commit as base; the bootstrap pull request then exercises every pull-request job |
| Time budgets (NFR35, NFR39): `make ci` 44 to 56 min estimated against 45; clean-clone job 55 to 69 against 75; Bats groups 9 to 16 min cold | Every figure is an estimate; no workflow, Bats file or full build was run | Timeouts are at least 1.5 times the upper estimate; Stories 9.3, 10.4 and 10.5 record local durations in `docs/operations.md`; the budget is revised only from a measurement (DV-04 left open, F-09) |
| Runner concurrency and cache quota: about 55 jobs on the bootstrap pull request, 14 Rust cache keys (12 until `crate-isolation` and `query-metadata` received keys of their own) | Depends on the GitHub plan; cache entries are saved on `main` only, so every run before the first merge is cold | Jobs share keys per artifact kind, except two jobs whose selections build other third-party units; eviction costs time, never correctness |
| Behaviour without organisation secrets (NFR8) | The skip-with-notice steps need the hosted context | Each affected step is specified in DD-13 and read in the criteria of Stories 10.1 to 10.3; the release job exits cleanly without the App secrets |
| Release: App token, release commit and tag on `main`, GitHub release | Needs `VILNACRM_APP_ID`, `VILNACRM_APP_PRIVATE_KEY` and a push to `main` | `scripts/release.sh` is run in a scratch repository in Story 3.6 except for the push and the release call; the first release uses the manifest version `0.1.0`; a manual `workflow_dispatch` input exists |
| Template sync, both flavours | Runs only in a downstream repository | The self-guard makes both workflows do nothing in the template; `.templatesyncignore` is compared with its listing; the first downstream run is started by hand |
| Label-driven checks (G19, G20, G25, G28): evaluation again on `labeled`, `unlabeled`, `edited` | Events exist only on GitHub | The approval variables `CONFIG_CHANGE_APPROVED` and `BREAKING_CHANGE_APPROVED` are tested locally in both states by the Bats suite |
| Mutation check on the bootstrap pull request (the diff is the whole repository) | Shard count and duration depend on the real mutant count | `scripts/ci/mutation-plan.sh` sizes the matrix (one shard per 150 mutants, at most 8, 45 min each); Story 9.3 measures seconds per mutant; Story 10.5 checks that the changed-code mode selects every mutant |
| Load budget (NFR1): the smoke budget was shipped at the ceiling of 500 ms until the first hosted run | The rule needs a p99 from a hosted 4-vCPU runner | Story 9.1 records the local p99; the budget was set from the first hosted run on pull request #1 (50 ms from the p99 of 7.25 ms; every protected file is new against `main`, so no label applied) (DD-25) |
| Cache effectiveness (NFR40) | Needs the log of a rerun | Cache keys and the image layer cache are specified; the `surface` Bats group is expected to recompile inside the development container (F-10) |
| CodeQL for Rust, SARIF upload, Codecov upload | Hosted services; code scanning may be unavailable | Publications, not gates (PD-22, PD-12); `CODE_SCANNING_ENABLED` switches the first two; each skips with a notice |
| Dependabot: six ecosystems, the toolchain ecosystem, the comment on the pinned `uses:` line of `bats-group` | Runs only on GitHub | The file follows the options reference; if an update drops the zizmor comment, G23 fails on that pull request and the ignore moves to `.github/zizmor.yml` |
| Documentation-only path (FR94, 5 min) | Needs a second pull request | `scripts/ci/detect-changes.sh` is tested in a temporary repository (Story 10.1) |
| Scheduled run, development-container build, determinism on a clean build (FR95, NFR7, NFR34, NFR42) | Scheduled workflows run on the default branch only | Story 10.5 is the local equivalent of `clean-bootstrap-ci`; `workflow_dispatch` allows a first run right after the merge |
| Repository settings: labels, required checks, squash merging, release identity, private vulnerability reporting | Organisation settings (A-3, A-8) | Listed in `docs/repository-settings.md` (FR117(h)); G31 rule (d) keeps the job names in that document complete |
| AWS load-test runner on real AWS | No credentials (A-6) | LocalStack dry run with a Bats test (FR85(e)); the limitation is documented |
| Platforms other than Linux x86_64 | Only that platform is claimed (scope-decisions 8.11) | `make doctor` prints whether the platform is verified |
| Explorer page under its policy; assistant hook events | Need a browser and the assistant product | Pin test and two Bats tests with recorded input; development-only page (F-12) |

Risks inside the implementation loop itself:

| Risk | Effect | Planned mitigation |
| --- | --- | --- |
| 100% lines and functions and zero surviving mutants at once | Stories 9.2 and 9.3 grow | Per-story coverage and mutation criteria; rules 17a, 21 and 23 of the core document for the tool blind spots; exclusions only as entries in protected records |
| The complete `xtask` (guard with JSON values and override names, scaffolder with the lock step) was designed, not built | Rework in Epic 2 | Epic 2 runs in waves 3 to 6, before any service code depends on it; every mechanism it uses was verified separately |
| Signed page parameters and the GraphQL body check were not run together; Schemathesis ran against the first form | A conformance failure late in the plan | Early Schemathesis run in Story 7.6, repeated against the production image in Story 9.1 |
| TLS-intercepting proxy on the implementation machine | Image builds and tool containers fail with certificate errors | Environment preparation before Story 1.4: named build context, `BUILD_IMAGE=0`, `DOCKER` wrapper; never a change to a tracked file |
| The generated Ralph instructions contradict the plan (F-02) | An implementer runs a bare `cargo test`, accepts 85% coverage or pushes early | Constraint C2 |
| One shared checkout for several agents | A gate fails on another story's unfinished work | Waves, exclusive resources, scoped commands while others are active, verification at the wave barrier |

## Constraints for the Implementation Loop

Binding for the orchestrator, every implementing agent and every verifying agent. C1 and C2 are done before the first story starts.

- **C1 Bundle consistency (F-01).** Before `bmalph implement`: copy `specs/rust-service-template-bootstrap/epics.md` over `_bmad-output/planning-artifacts/epics.md`, regenerate `story-index.json`, and export `PHP_TEMPLATE_DIR` and `CORE_SERVICE_DIR` in every implementer and verifier shell.
- **C2 Ralph instructions (F-02).** After `bmalph implement`, correct `.ralph/@AGENT.md` and check `.ralph/PROMPT.md` against it: the completion check of a story is its `Verify` line and its acceptance criteria, never a bare `cargo test` (the integration and end-to-end targets need the compose services and `scripts/with-env.sh .env .env.test --`); the coverage floor is 100% lines and functions (G14), not the 85% of the template; commits follow the commit protocol of `epics.md` (one commit per story, subject `<type>(#1): story N.M <short title>`, exactly its `Owns` and `Touches` paths), not the form `feat(module): ...` of the template; the template's instruction to push after each feature does not apply: nothing is pushed before Story 10.5 is verified.
- **C3 Order.** Story-id order is a valid dependency order. Concurrency is allowed only as the waves, the table "Exclusive resources" and the order inside the waves say, in one shared checkout; never one worktree per story.
- **C4 Environment.** The proxy handling is prepared before Story 1.4: images through the named build context, then `BUILD_IMAGE=0`; tool containers through the `DOCKER` wrapper. A certificate error is an environment finding, not a reason to edit a tracked file.
- **C5 Authority and names.** `scope-decisions.md`, then the PRD, then the two architecture documents, then the research. Names in code font are final. When a listing does not work as written, change the smallest thing and add an entry to `_bmad-output/implementation-artifacts/deviations.md`. Never change a threshold, a protected value, an exclusion record or a lint suppression to obtain a pass.
- **C6 Verification.** The verifying agent is independent, runs the `Verify` line as written on the story's commit, checks listing equality with `diff` and reads every deviation entry. "It compiles" is not done.
- **C7 Test strength at the story.** Every code story of Epics 2 and 4 to 7 proves 100% lines and functions and zero missed or timed-out mutants for the files it owns. Stories 9.2 and 9.3 only close what the whole workspace shows; if either needs more than a few new tests, name the story that left the gap.
- **C8 Measurements.** Record them where the stories say: p99 of the smoke profile (9.1); mutant count, seconds per mutant and shard durations (9.3); stage and Bats group durations (10.4); `make bootstrap`, `make ci`, `make start` (10.5). NFR35 and NFR39 are revised only from these numbers and from the first hosted run.
- **C9 Small additions through the owning story, each with a deviation entry.** Story 3.4 or 9.7 asserts that `API_HOST` and `API_PORT` override the target (F-03). Story 8.5 lists aliases and helper targets among the targets without a Bats test (F-04). Story 7.4 or the final review opens the explorer page once with `APP_ENV=dev` (F-12).
- **C10 Before Story 10.5.** Apply the deviation entries to the architecture documents; correct the "four shards" statements of the core document (F-05); refresh `specs/rust-service-template-bootstrap/` with the placeholders kept, this report and the run summary included, the README status rows updated and the number of bundle files stated consistently (F-07).
- **C11 After Story 10.5, before the push.** Repeat the review-verified checks NFR20, NFR23, NFR27, NFR44 and NFR46 on the repository and record them in the run summary.
- **C12 First push and first merge.** The first hosted run is the measurement: change a timeout or the shard divisor only from recorded numbers; rerun one unchanged commit for NFR40 (F-10 is decided: four recorded exceptions, delivery open item 5); set the smoke budget from the hosted p99 (done on pull request #1, where every protected file is new against `main`, so no label applies: 50 ms from 7.25 ms); maintainers create the labels, required checks, squash merging, App secrets and code-scanning settings of `docs/repository-settings.md`.

## Summary and Recommendations

### Overall readiness status

**READY WITH CONDITIONS**

The product brief, the PRD, the two architecture documents and the epics are aligned. Requirement coverage is complete (204 of 204 ids), the architecture maps every id, the core and delivery documents agree on every gate command, name and count that was checked, the stories carry no stale name and own every file exactly once, and the dependency graph is valid for a sequential or a wave-based loop. The remaining uncertainty is of two kinds: two inconsistencies outside the story text that are cheap to remove before the start, and everything that only GitHub can prove.

### Blocking issues

None. No finding makes a story impossible to complete as written or would make the loop build the wrong product.

Two findings must still be resolved before the loop starts, because they would otherwise be decided by accident:

- **F-01.** The tracked copy of `epics.md` (placeholders) and the working copy (machine paths) differ, while Stories 8.8 and 10.5 require them to be byte-equal. Exact fix: constraint C1 and the section "F-01" above.
- **F-02.** bmalph's stack detection writes `cargo build`, `cargo test`, `cargo build --release` and `cargo run` into `.ralph/@AGENT.md` (checked by running its detection on both architecture documents). The template text of that file also asks for 85% coverage, shows the commit form `feat(module): ...` and tells the agent to push every completed feature. Each of the four contradicts the plan: a bare `cargo test` cannot pass without the compose services and the two env files; the floor is 100% lines and functions (G14); the commit form is `type(#issue): subject` (FR102); nothing is pushed before Story 10.5 is verified. Exact fix: constraint C2.

### Conditions to observe

C1 to C12 of "Constraints for the Implementation Loop". In one line each: C1 make the two copies of the epics equal and export the two source variables; C2 correct the generated Ralph instructions; C3 keep story-id or wave order in one checkout; C4 prepare the proxy environment; C5 respect authority, names and protected values and log deviations; C6 verify independently with the `Verify` line as written; C7 prove test strength in each code story; C8 record the measurements; C9 add the three small criteria; C10 refresh the documents and the bundle before Story 10.5; C11 repeat the review-verified NFR checks on the repository; C12 treat the first hosted run as the measurement.

### Findings

| Id | Category | Finding | Severity | Handling |
| --- | --- | --- | --- | --- |
| F-01 | bundle | Tracked and working copy of `epics.md` differ; two `Verify` lines require equality | resolve before the start | C1 |
| F-02 | loop set-up | Generated Ralph instructions contradict the stories (test command, 85% coverage, commit form, push after each feature) | resolve before the start | C2 |
| F-03 | coverage | FR79: no criterion for `API_HOST` and `API_PORT` | minor | C9 |
| F-04 | coverage | Fix-style, alias and helper targets have no criterion; the list of targets without a Bats test is incomplete for FR67(a) | minor | C9 |
| F-05 | core against delivery | "four shards" in the core document, one shard per 150 mutants up to 8 in the delivery document and the stories | minor | C10 |
| F-06 | records | 8 adapted findings listed in the core document, 9 stated in the revision summary | cosmetic | run summary |
| F-07 | bundle | The tracked bundle has a twelfth file (`scope-decisions.md`) while the manifest and Story 8.8 count eleven; README rows for this report and the run summary say "Pending" | minor | C10 |
| F-08 | PRD readings | Four readings are used but not recorded: `detail` optional (FR6), 32-bit page values (FR22), "two values" (FR92(d)), files with an external reader (NFR23) | minor | run summary |
| F-09 | partly decided | NFR35 and NFR39 against the estimates (DV-04): the two clean-clone budgets of NFR35 (`make bootstrap` 15 minutes, `make ci` 45 minutes) are met on a GitHub-hosted ubuntu-24.04 runner (run 43107219243: 447 s and 1828 s; `make bootstrap` again 472 s in run 43119953189 and 452 s in run 43151995703); the log does not show the CPU count. NFR39 warm: 9 min 0 s on pull request #2 at `e9eef29` (push 00:29:14Z, last required check 00:38:14Z, caches restored with a full match) and 9 min 21 s at `95547ba`, against 20 minutes; the mutation check ended earlier on both. NFR39 documentation-only: about 1 minute, an estimate from the jobs that run when `code=false` (the last of them ended 57 s and 52 s after the push), against 5 minutes. `make start`: the clean-clone job stops the stack and times `make start` before `make ci` and fails above 60 s; on a hosted runner 12 s (run 43151995703, job 130963102019, a dispatch on the branch of pull request #2 at `da5ea9c`) | closed for the three budgets of NFR35 and the warm budget of NFR39; the documentation-only budget is estimated and closed by the first documentation-only pull request | C8, C12 |
| F-10 | decided | NFR40 holds for every job except four recorded exceptions: the `surface` Bats group (compiles inside the development container), the weekly `devcontainer-build` (built from nothing on purpose) and the weekly `clean-bootstrap-ci` (FR95 and NFR34 require it to start without a restored cache), and, for its rerun clause, the third-party crates that a pull-request mutation shard compiles because its diff selects packages the saver shard on `main` did not build (measured 0 to 77 compile units per shard, 0 for the saver's own selection; a cache per pull request would cost about 800 MB of the 10 GB cache quota per pull request and commit and could evict the entries of `main`); the jobs `crate-isolation` and `query-metadata`, which restored the keys `check` and `service-debug` of other jobs and recompiled 224 and 354 third-party compile units on every run, also after an exact-key restore, are no exception: since pull request #2 each saves a key of its own on `main`, and the rerun count 0 of both is checked after its merge | accepted; the two `quality.yml` jobs closed by keys of their own | C12 (the rerun still checks every other job, `crate-isolation` and `query-metadata` among them) |
| F-11 | process | Six NFRs name this review as verification before any code exists | minor | C11, C12 |
| F-12 | risk | The explorer page is never loaded in a browser | low | C9 |
| F-13 | sizing | Stories 9.2, 9.3, 10.4 and 10.5 are open-ended; 7.6 and 9.1 have conditional scope | minor | C7 |
| F-14 | sizing | Stories 10.1 and 10.2 carry 28 and 29 ids and are verifiable only by lint and reading before the push | minor | C12 |
| F-15 | structure | Epics follow components and build order; workflows come last; one table is created before its consumer | recorded deviation | none |
| F-16 | form | Nine stories without a `Pitfalls` line | cosmetic | none |

### Recommended next steps

1. Apply C1: equalise the two copies of `epics.md`, regenerate `story-index.json`, export `PHP_TEMPLATE_DIR` and `CORE_SERVICE_DIR`.
2. Run `bmalph implement` on `_bmad-output/planning-artifacts/`. The preflight passes today; this report adds no blocking verdict.
3. Apply C2 to `.ralph/@AGENT.md` and prepare the proxy environment (C4).
4. Start the Ralph loop with the agent allocation of scope-decisions section 9: implementation agents implement one story per session; an independent verification agent verifies each story with its `Verify` line and controls the loop. Follow story-id order, or the waves with the exclusive resources.
5. After Story 10.5: apply C10 and C11, then push. Treat the bootstrap pull request and the first merge as the measurement run (C12) and let the final review close F-09 and F-10 with numbers.

### Final note

This assessment recorded 16 findings in 8 categories: 0 blocking, 2 to resolve before the loop starts, 2 open decisions that need hosted measurements, and 12 minor, cosmetic or recorded items. The planning artifacts are detailed enough for an autonomous loop: names are final, every criterion is a command with an expected result, and every gate has a negative test planned. The part of the product that this review could not see working is the hosted pipeline; the plan postpones it to the last epic by design, and the constraints above keep that risk visible until the first runs have been measured.

## Orchestrator Disposition of the Conditions

Added by the orchestrating agent after the assessment above. The assessment itself is unchanged, except the rows F-09 and F-10 of the findings, the row NFR27 of the results for the requirements that name this review, condition C12 and the risk row of the load budget, which were updated after the FR/NFR review gate (iteration 1) decided F-10, the smoke budget was set from the first hosted run, the first scheduled run on `main` met the two clean-clone budgets of NFR35 and the architect accepted the unchanged PHP Spectral ruleset for NFR27. The rows F-09 and F-10 and condition C12 were updated again after the FR/NFR review gate (iteration 2) recorded a third and a fourth exception of NFR40 and the hosted measurements of pull request #2. The row FR66(d), NFR22 of the requirements that are ambiguous or cannot be tested as written was added after the FR/NFR review gate (iteration 3) admitted the scheduled-only steps of FR95, FR118 and NFR35 as CI-only steps, and the row NFR16, FR125(g) after iteration 4 extended rule (g) to the inline directives of the other linters. The row F-10 and the risk row of runner concurrency and cache quota were updated after the FR/NFR review gate (iteration 4) gave the jobs `crate-isolation` and `query-metadata` Rust cache keys of their own.

| Condition | Disposition |
| --- | --- |
| C1 | Done. The working copy and the tracked copy of `epics.md` both use `${PHP_TEMPLATE_DIR}` and `${CORE_SERVICE_DIR}`; `story-index.json` carries the same substitution; both variables are exported in every implementer and verifier shell. |
| C2 | Done right after `bmalph implement`: `.ralph/@AGENT.md` and `.ralph/PROMPT.md` are rewritten to the story protocol (completion means the `Verify` line and every criterion, the 100% lines and functions floor, the commit protocol, pushes by the orchestrator only). |
| C3 | Decided differently, and the documents were made consistent with the decision. The loop runs one git worktree per story, not one shared checkout: in a shared checkout every workspace-wide gate sees the unfinished files of other stories, and an interrupted session would leave half-finished work in the primary checkout. The sections "Rules for concurrent execution" and "Exclusive resources" of `epics.md` and four sentences that named the shared checkout were rewritten for the worktree model, so the two models are not mixed. Story text, dependencies, waves and the critical path are unchanged; bmalph's parser still reads 57 stories with 0 warnings. |
| C4 | Prepared before Story 1.1: a wrapper for `docker run` and `docker build` that adds the session proxy settings, the CA bundle and the named build context; a builder image that carries the CA; notes that every agent reads first. Nothing of this is tracked. |
| C5, C6, C7 | Adopted as rules of the loop and written into the implementer and verifier instructions. |
| C8 | Open until the stories that measure have run; the numbers go into `docs/operations.md` and the run summary. |
| C9 | Carried into Stories 3.4, 8.5 and 7.4 through the deviation rule. |
| C10 | Done before Story 10.5. Two documentation passes applied every entry of `deviations.md` to `architecture.md`, `architecture-delivery.md`, `epics.md` and this bundle's other documents: the first pass the entries recorded during the implementation stories, among them the corrected "four shards" statements of the core document (F-05); the second pass the entries of the fixes after the first hosted runs and of the final review. `specs/rust-service-template-bootstrap/` was then refreshed from `_bmad-output/planning-artifacts/` with the placeholders kept; its README lists the eleven layout files and `scope-decisions.md` and has no pending row (F-07). |
| C11 | Done on 2026-11-11 on commit `aaa0f3f`, after Story 10.5, with the two digest findings closed by batch B7 of the FR/NFR gate (the review-bot setting documented; the unchanged PHP Spectral ruleset accepted by decision): NFR20, NFR23, NFR27, NFR44 and NFR46 were repeated on the repository and recorded in the run summary (table "Review-verified requirements, checked on the repository" under "Outcome of the Implementation Run"). All five pass. |
| C12 | Partly done. The first hosted runs on pull request #1 measured the smoke profile (p99 7.25 ms), and the smoke budget was set from it (50 ms; every protected file is new against `main`, so no label applied). F-10 is decided: NFR40 holds except for four recorded exceptions, the `surface` Bats group, the weekly `devcontainer-build` job and the weekly `clean-bootstrap-ci` job (FR95 and NFR34 require it to start without a restored cache) and, for its rerun clause, the third-party crates a pull-request mutation shard compiles for packages outside the saver's selection (delivery, open item 5). The first scheduled run on `main` (run 43107219243, `workflow_dispatch` after the squash merge of pull request #1, commit `3e0bd3e`, 2026-11-11, runner label `ubuntu-24.04`) measured the clean-clone job: `make bootstrap` 447 s and `make ci` 1828 s from a clean clone with no restored cache, both exit 0, the working tree unchanged; NFR34 is met and the NFR35 budgets of 15 and 45 minutes are confirmed on a hosted runner (`make start` warm was first measured locally, 12 s; the clean-clone job times it before `make ci`, and run 43151995703, a dispatch on the branch of pull request #2 at `da5ea9c`, measured 12 s on a hosted runner, so the third budget of 60 s is confirmed as well); its five mutation shards passed (699 mutants, 459 caught, 240 unviable, none missed or timed out). Its `devcontainer-build` failed (FR95): the `dev` stage gave the home directory the sticky bit, and common-utils then cannot append as root to the `.bashrc` it gave the user under the runner's `fs.protected_regular`; batch B8 of the FR/NFR gate fixed the `Dockerfile` (mode `0777` without the sticky bit) on pull request #2, and a dispatch of the scheduled workflow on that branch proved it: the build failed after 175 s on `3e0bd3e` (run 43107219243) and succeeded in 175 s after the fix (run 43119953189, job 130857597533, head `027e1f1`). The runtime steps of FR118 that batch B10 of iteration 2 added to the job (start and `postCreateCommand`, `docker info`, `make format-check` and `make start-deps` inside the container) passed in run 43151995703 (job 130963101979, head `da5ea9c`): build 164 s, start 7 s, `docker info` under 1 s, `make format-check` 1 s, `make start-deps` through the inner engine 29 s. The rerun of one unchanged commit for NFR40 ran on `a930b44` (Tests run 43114972119 and Load tests run 43114972020, attempt 2): every job of the two workflows recompiled 0 third-party crates and rebuilt 0 unchanged image layers except `Mutation shard 0/1` (job 130846136062), which compiled 321 third-party crates in the scratch copy of cargo-mutants. Batch B9 fixed it (cargo-mutants builds in the checkout with `MUTANTS_IN_PLACE=1`; renamed keys `mutation-in-place` and `bats-tests-in-place`); on head `95547ba` the shard ran in place with the same outcomes (8 of 8 caught) and printed 321 cold, because no entry exists under the new names before the merge. The count on a rerun after the first push to `main`, across all seven pull-request workflows, is still to be measured, by a rerun after that push; a shard outside the saver's selection may compile up to 77 compile units under the fourth recorded exception (F-10). The timeouts and the maintainer settings are not settled by these runs. |

One rule of the assessment is replaced. "Nothing is pushed before the last story is verified" becomes: handed-over stories of Epics 1 to 9 are pushed as they land, because the branch holds no workflow file until Story 10.1 and a push therefore starts no hosted check; from Story 10.1 on nothing is pushed until Story 10.5 is verified. This keeps finished work safe if the session is interrupted.
