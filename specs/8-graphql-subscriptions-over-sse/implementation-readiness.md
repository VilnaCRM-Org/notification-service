# Implementation readiness: GraphQL subscriptions over Server-Sent Events (issue #8)

Bundle `specs/8-graphql-subscriptions-over-sse/`, branch `claude/vilnacrm-notification-service-br7s47`, pull request #9. This check cross-reads the five artifacts (`research.md`, `product-brief.md`, `prd.md`, `architecture.md`, `epics.md`) against each other and against issue #8 as it reads on 2026-11-17, with its 11 acceptance criteria, and against the three maintainer decisions (M1 graphql-sse over SSE with GraphQL over HTTP request formats; M2 planned with BMAD through the rust-backend-sdlc plugin; M3 only issue #8 is planned).

How the check was done: by reading the five files, the issue body and the repository files they cite; by scripts over `prd.md` and `epics.md` (requirement ids, the requirement-to-story table, the Covers lines, the story structure, the waves derived from the Depends on lines); by reading the wheel of Schemathesis 4.33.1 (the image pinned in `make/tool-versions.mk`) and the registry sources of utoipa 6.0.0 and utoipa-axum 0.3.0. No build, test or container was run; the planning session has no container engine.

## 1. The main finding: two criteria of the issue were missing

Issue #8 was updated on 2026-11-17 by a maintainer decision. It now asks, besides the GraphQL subscription, that every route of the service be in the OpenAPI document and on the Swagger page, fully documented, with a test that fails for a route served without an entry (AC9), and that `make validate-openapi-spec`, `make openapi-diff` and `make schemathesis-validate` pass, a streaming endpoint being excluded from Schemathesis only when it cannot be tested (AC10). The former last criterion (documents and `make ci`) became AC11.

The bundle had been written against the nine criteria of 2026-11-16. It had no requirement, design or story for AC9 and AC10, and it stated the opposite in four places: `prd.md` NFR-16 ("The OpenAPI document and its snapshot do not change ... GraphQL and the streams are outside the OpenAPI document"), `architecture.md` "Contract and Snapshot Changes" ("OpenAPI does not change"), `epics.md` ("The OpenAPI snapshot never changes") and the criteria of stories 2.2 and 5.1 (`make.openapi_diff` "reports no change"). This check fixed it in all five artifacts (section 6, F1 to F8).

## 2. Issue criteria to functional requirements

The traceability table of `prd.md` maps every criterion; a script confirmed that every id it names is defined. Result: pass.

| Criterion | FR and NFR | Brief | Check |
| --- | --- | --- | --- |
| AC1 `Subscription` root in the snapshot; `make graphql-diff` not breaking | FR-1, NFR-16 | S1 | pass |
| AC2 subscription for `a` over SSE: `200 text/event-stream`, one `next` per event of `a`, none of another type | FR-2, FR-3, FR-4, FR-7 to FR-10 | S2, S3 | pass |
| AC3 resume with a last event id, in order, before live events, no id twice | FR-11, FR-12, FR-19 | S4, S10 | pass |
| AC4 invalid type name, more than 100 types, malformed cursor: `VALIDATION_FAILED`, no stream of events | FR-6, FR-13, FR-14, FR-15 | S5 | pass |
| AC5 starting, at capacity, shutting down: refused before a stream with the reason; counts against `MAX_SUBSCRIBERS` | FR-9, FR-16, FR-19 | S6, S10 | pass |
| AC6 keep-alive every `STREAM_KEEP_ALIVE`; a client that stops reading delays nobody | FR-17, FR-18, NFR-6 | S7 | pass |
| AC7 a query over SSE: one `next`, then `complete` | FR-8, FR-20 | S8 | pass |
| AC8 browser access through `CORS_ALLOWED_ORIGINS` only | FR-4, FR-21 | S9 | pass |
| AC9 every route in the OpenAPI document and on the Swagger page, fully documented; a test fails for a route without an entry | FR-25 (ADR 0022), FR-26, FR-27, FR-28 | S13 | pass after F1 |
| AC10 the OpenAPI gates and Schemathesis pass; an exclusion only when a streaming endpoint cannot be tested | FR-2 (GET rule), FR-29, NFR-16, NFR-20 | S14 | pass after F1 and F3 |
| AC11 the two documents describe the subscription; every local gate of `make ci`, 100 % coverage, no surviving mutant | FR-23, FR-24, FR-25, NFR-10 to NFR-16, NFR-19 | S11, S12 | pass |
| M1 graphql-sse over SSE, GraphQL over HTTP request formats, every protocol choice recorded | FR-2 to FR-8, FR-25 (ADR 0021) | S11 | pass |

The issue leaves six points to the plan (field and arguments, endpoint path, GET for `EventSource`, the mapping of `Last-Event-ID` and of the 503 states, the single connection mode). Each is decided by an `> Assumption:` line of `research.md` section 8 and restated with its reason in `prd.md`. The issue's out-of-scope items (a GraphQL WebSocket protocol, subscriber authorization) are out of scope in `prd.md` "Product Scope".

## 3. Requirements to stories

`prd.md` defines FR-1 to FR-29 and NFR-1 to NFR-20, with no gap in either numbering. A script confirmed, for the table "Requirement to stories" of `epics.md`:

- every FR and NFR id is in the table, and every id of the table is defined in `prd.md`;
- every story the table names exists;
- every id a story's Covers line names is defined, and the table lists that story for it;
- every story the table lists for an id names that id in its Covers line;
- AC1 to AC11 are each in the table and in the traceability table of `prd.md`.

Result: pass. NFR-10, NFR-11 and NFR-12 point to story 5.1 and to the gates every story names, which carry their own coverage, mutation and lint bars.

## 4. Stories, criteria and order

A script read each `### Story N.M:` block of `epics.md` up to the next story or epic heading. For all 18 stories (1.1 to 1.3, 2.1 to 2.7, 3.1, 3.2, 4.1 to 4.5, 5.1):

- the block holds exactly one `**Acceptance Criteria:**` line, at least one `**Given**`, `**When**` and `**Then**`, and every criterion line carries `**When**` and `**Then**`;
- the block holds exactly one `**Depends on:**` line and no inner level-2 or level-3 heading;
- the ids ascend in file order and are unique;
- every id a Depends on line names is an earlier story.

Every criterion names an observable result (a status, a header, bytes of the body, a log entry, a document entry or a gate by its `make.*` key), so each is testable. The waves derived from the Depends on lines (a story is ready once its dependencies are done) equal the table "Waves": 1.1, 1.2 and 2.1; 1.3; 2.2 and 4.4; 2.3; 2.4; 2.5, 3.1 and 4.3; 2.6; 2.7, 3.2, 4.1 and 4.2; 4.5; 5.1. No two stories of one wave edit the same file. Result: pass.

The story-1 rule: the change touches `crates/notification/event-stream/application` and `crates/service`, both live members of `[workspace].members` (read with the planning skill's one-liner), so no scaffold story is needed and none is written. Result: pass.

## 5. Architecture contract

- `prd.md` has `## Executive Summary`, `## Functional Requirements`, `## Non-Functional Requirements` and `## Product Scope`, in this order.
- `architecture.md` has `## Tech Stack`, which names the Rust toolchain 1.99.1 (edition 2024) and the crates the change uses, utoipa 6.0.0 and utoipa-axum 0.3.0 included, and no other stack.
- `## Constraints` is 3 888 bytes from its heading to the next level-2 heading, below 4 500. Part (a) comes first with `(#8)` and the `story N.M` subject form, the checked-out branch, never `main`, never pushed by Ralph, hooks never bypassed. Part (b) comes second with the independence rule, the `rust-implementer` rule and its sequential fallback, and the ten `make.*` keys with the targets of `.claude/rust-sdlc.yml`: `make.setup_test_db` and `make.sqlx_prepare` read `not available: capability absent`, the other eight their target. The trailing lines add the OpenAPI rule. The section holds no fenced block and no further level-2 heading.
- No protected file changes, so no "Protected Changes" heading is needed: the allow-lists already hold `async-graphql`, `axum`, `futures-util`, `tokio`, `serde_json`, `utoipa` and `utoipa-axum`; the protected Spectral rules are met by a `default` problem response on every operation; the protected Schemathesis flags stay; the protected Bats tests G17, G19, G20 and G21 keep their meaning (F7).

Result: pass.

## 6. Fixes made in this check (specs only)

| Id | Severity | Artifact | Finding | Fix |
| --- | --- | --- | --- | --- |
| F1 | blocker | all five | AC9 and AC10 (2026-11-17) had no requirement, design or story, and four statements contradicted them (section 1) | `prd.md` FR-26 (the eleven operations and what each documents), FR-27 (the completeness tests), FR-28 (the Swagger page), FR-29 (the gates, Schemathesis, root-cause rule and escalation), NFR-16 rewritten, NFR-20, traceability, scope; `architecture.md` section "OpenAPI Document of Every Route", Tech Stack, Constraints, placement, tests, documents, ADR 0022, build order, failure modes; `epics.md` stories 4.4 and 4.5, story 5.1, the waves and the table; `product-brief.md` goal 10, S13, S14; `research.md` sections 2.8, 3.11, 4.1, risks and assumptions |
| F2 | major | `product-brief.md`, `prd.md`, `epics.md` | the criteria were numbered 1 to 9; the issue now lists 11, and the former AC9 is AC11 | renumbered in every table, Covers line and criterion |
| F3 | major | `prd.md`, `architecture.md`, `epics.md` | Schemathesis sends `Accept: */*`; a documented `GET /api/graphql` would answer it with the 405 of FR-5, which its check `positive_data_acceptance` refuses, so AC10 could not pass | the GET rule of FR-2: on GET a wildcard admits the event stream (RFC 9110 12.5.1), a missing `Accept` keeps today's 405, which the "Accept" section of GraphQL over HTTP allows, and `text/html` keeps the explorer; story 2.6 implements and tests it; every existing test, which sends no `Accept`, keeps its answer |
| F4 | major | `prd.md`, `architecture.md`, `epics.md` | Schemathesis 4.33.1 validates each event of a `text/event-stream` response as `{data, event, id}`; a plain string schema would fail every event | the event schemas `SseFrame` and `GraphQLEvent` of one event, `data` with `contentMediaType` and `contentSchema`, the empty data of `complete` admitted |
| F5 | minor | `prd.md`, `architecture.md`, `epics.md` | utoipa-axum writes the annotated path verbatim, and `/api/docs/{*file}` is not an OpenAPI path template | the asset route becomes `/api/docs/{file}`, and story 4.5 proves every asset of the page is served |
| F6 | minor | `prd.md`, `architecture.md`, `epics.md`, `research.md` | `crates/service/src/http/ping.rs`, `docs.rs` and their test directories are template files that `.templatesyncignore` does not list | story 4.5 adds the four entries |
| F7 | minor | `prd.md`, `architecture.md`, `research.md` | the effect of the larger document on the protected tests G17, G19 and G21, on the protected Spectral rules (a 101 is not a 2xx) and on `scripts/check-docs.sh` was not stated | stated in NFR-16 and in the architecture: every operation keeps a `default` problem response; G17 now removes the description of `GET /api/docs` and still fails Spectral; G19 is unchanged; G21's run still ends because Schemathesis bounds every stream |
| F8 | minor | `epics.md` | `make.schemathesis_validate` needs the load-test stack, which the parallel agents do not own | it runs in the serialized verify step for the wave whose story names it (story 4.5) |

## 7. Risks carried into the stories

These are not open findings: each has a criterion of story 2.6, 4.4 or 4.5 that proves it, and FR-29 states what happens when it fails.

| Id | Risk | Proof or handling |
| --- | --- | --- |
| K1 | Parts of the Schemathesis analysis are inferences from its source, not a run against this service: the client's handling of a `101` and the resend after the reused connection drops; the positive values it draws for `variables` and `extensions` under `contentSchema` | story 4.5 runs `make.schemathesis_validate`; a failure is fixed at its root in the document or the code, or the story stops and escalates with the exact exclusion; the plan excludes nothing |
| K2 | A parameter schema can admit a value the route refuses, or refuse one it accepts (the cursor range, a repeated type name) | story 4.4 checks every value today's tests send against the documented schemas; story 4.5's Schemathesis run checks both directions |
| K3 | The Schemathesis job, and the protected G21 test that runs it, grow by up to 10 s per silent streaming case | the examples subscribe to `load.test`, which the load-test publisher writes, so they reach 20 events fast (NFR-20) |
| K4 | In `dev`, curl with its default `*/*` now gets an event stream instead of the explorer | stated in FR-2, ADR 0022 and the documents of story 4.5; browsers keep the explorer |

## 8. Open findings

None. Every finding of sections 1 and 6 is fixed in the artifacts, and the risks of section 7 are carried by story criteria.

## 9. Self-check

Run after the fixes, at most five rounds:

1. `verify-bundle.sh` (the commands of the planning skill's Verification) prints nothing.
2. The story structure of `epics.md` passes the per-block script of section 4.
3. The staging dry run of the plugin, `stage-planning-artifacts.sh specs/8-graphql-subscriptions-over-sse --dry-run`, exits 0 with only WOULD lines.
4. The directory holds exactly the seven files of the bundle.

The rounds and their output are in `run-summary.md`.

Verdict: ready
