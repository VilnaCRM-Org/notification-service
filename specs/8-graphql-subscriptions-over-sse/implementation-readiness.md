# Implementation readiness: GraphQL subscriptions over Server-Sent Events (issue #8)

Bundle `specs/8-graphql-subscriptions-over-sse/`, branch `claude/vilnacrm-notification-service-br7s47`, pull request #9, checked on 2026-11-21 in the second readiness round of that day, after the correct-course round and the first readiness round. This check cross-reads the five artifacts (`research.md`, `product-brief.md`, `prd.md`, `architecture.md`, `epics.md`) against each other, against issue #8 as it reads on 2026-11-17 with its 11 acceptance criteria, against the working tree, and against the maintainer decisions M1 (graphql-sse over SSE with the GraphQL over HTTP request formats), M2 (planned with BMAD through the rust-backend-sdlc plugin), M3 (only issue #8 is planned), M4 of 2026-11-20 ("Ignore both, replace ours": the two `deny.toml` entries and one new story that replaces humantime) and M5 of 2026-11-21 ("Approve the bound": `query` at most 4 096 and `operationName` at most 128 characters, refused with the 422 validation problem on both transports and both forms, a recorded change of today's behaviour of the JSON transport).

How the check was done:

- by reading the five files, the repository files they cite, the branch history `c36f708..99da8cb`, the commits `56a8725` (the fix) and `99da8cb` (its record), the working-tree diff of `crates/service` that fixes review finding CQ-4, and `.github/openapi-spec/spec.yaml` (which members carry a `maxLength`);
- by taking the eight findings of the review of the first readiness round as data and checking each against the files and the repository before fixing it;
- by scripts over `prd.md` and `epics.md` (requirement ids, the table "Requirement to stories", the Covers lines, the structure of each story block, the Depends on lines) and over `run-summary.md` (its assumption lists against the `> Assumption:` lines of the artifacts);
- by checking that every test the bundle names for M5 and CQ-4 is defined in `crates/service`.

No test, mutation or load run was made. The self-check ran `make.docs_check`, which builds the service to read its help text, and the repository's Markdown and spelling linters (the targets `lint-markdown` and `lint-spelling`, which have no `make.*` key in the profile).

State of the branch: stories 1.1 to 4.5 are implemented and committed (`892e8c3` to `9bf9de6`). Story 5.1, the serialized verify step, produced the root-cause fix `56a8725` and its record `99da8cb`, and stays open until the CQ-4 commit and a run of the whole suite on the final head. CI on `99da8cb` passed 49 of 49 checks, Schemathesis included. The review panel runs; its findings are fixed in the working tree and not yet committed: CQ-4 in eight files under `crates/service`, CQ-9 in `crates/notification/event-stream/application/src/tail.rs` and `tail/tests.rs` (the tail publishes its state before it writes the fault, readiness and health entries), and CQ-10 in `crates/service/src/graphql/handler.rs` with the regenerated `.github/openapi-spec/spec.yaml`, which changes only by the description of `POST /api/graphql`. The four protected changes the maintainer approved are unchanged: `a374895` (`.spectral.yaml`), `71aab46` (`make/gates.mk`), `c4f83b3` (`deny.toml`) and `49641e7` (`tests/CLI/bats/make_quality_gates.bats`). Neither M5 nor a review fix (CQ-4, CQ-9, CQ-10) touches a protected file.

## 1. Issue criteria and decisions to requirements

The traceability table of `prd.md` maps every criterion and decision; a script confirmed that every id it names is defined.

| Criterion | FR and NFR | Brief | Check |
| --- | --- | --- | --- |
| AC1 `Subscription` root in the snapshot; `make graphql-diff` not breaking | FR-1, NFR-16 | S1 | pass |
| AC2 subscription for `a` over SSE: `200 text/event-stream`, one `next` per event of `a`, none of another type | FR-2, FR-3, FR-4, FR-7 to FR-10 | S2, S3 | pass |
| AC3 resume with a last event id, in order, before live events, no id twice | FR-11, FR-12, FR-19 | S4, S10 | pass |
| AC4 invalid type name, more than 100 types, malformed cursor: `VALIDATION_FAILED`, no stream of events | FR-6, FR-13, FR-14, FR-15 | S5 | pass |
| AC5 starting, at capacity, shutting down: refused before a stream with the reason; counts against `MAX_SUBSCRIBERS` | FR-9, FR-16, FR-19 | S6, S10 | pass |
| AC6 keep-alive every `STREAM_KEEP_ALIVE`; a client that stops reading delays nobody | FR-17, FR-18, NFR-6 | S7 | pass |
| AC7 a query over SSE: one `next`, then `complete` | FR-8, FR-20 | S8 | pass (S8 names the length refusal of M5, and the Vary assumption of FR-5 now quotes it so) |
| AC8 browser access through `CORS_ALLOWED_ORIGINS` only | FR-4, FR-21 | S9 | pass |
| AC9 every route in the OpenAPI document and on the Swagger page; a test fails for a route without an entry | FR-25 (ADR 0022), FR-26, FR-27, FR-28 | S13 | pass (the one GET handler of CQ-4 keeps the document unchanged) |
| AC10 the OpenAPI gates and Schemathesis pass; an exclusion only when a streaming endpoint cannot be tested | FR-2 (GET rule), FR-29, NFR-16, NFR-20 | S14 | pass (CI on `99da8cb`, Schemathesis included; FR-29 states the narrowing that oasdiff cannot report) |
| AC11 the two documents; every local gate of `make ci`, 100 % coverage, no surviving mutant | FR-23, FR-24, FR-25, NFR-10 to NFR-16, NFR-19, NFR-21 | S11, S12 | pass |
| M1 graphql-sse over SSE, GraphQL over HTTP request formats, every protocol choice recorded | FR-2 to FR-8, FR-25 (ADR 0021) | S11 | pass |
| M4 (2026-11-20) the two advisories ignored by the approved `deny.toml` entries; humantime replaced in the two service crates, the documented duration syntax and the envelope time rule kept | NFR-21, NFR-15, NFR-17 | S15 | pass |
| M5 (2026-11-21) `query` at most 4 096 and `operationName` at most 128 characters, documented as `maxLength`, refused with the 422 length problem on both transports and both forms; a recorded change of the JSON transport | FR-3, FR-4, FR-5, FR-6, FR-23, FR-29, NFR-3, NFR-8, NFR-17 | S8, S14 | pass after D5 (FR-23 and NFR-3 added to the row) |

The issue's six open points are decided by `> Assumption:` lines of `research.md` section 8 and restated in `prd.md`. The issue's out-of-scope items (a GraphQL WebSocket protocol, subscriber authorization) are out of scope in `prd.md` "Product Scope". M5 is a decision of the maintainer and is cited as such wherever the bound changes today's behaviour: `prd.md` (the sources, the Executive Summary, P18, FR-3 to FR-6, FR-23, FR-29, NFR-3, NFR-8, NFR-17, the traceability table and the scope), `architecture.md`, `epics.md` (Epic 2, stories 2.3, 4.5 and 5.1), `research.md` (the inputs, section 3.11, the negotiation assumption of section 8 and a risk of section 7), `product-brief.md` (goal 2, S8, S14) and `run-summary.md`. The reason for applying the bound to the JSON transport too is the `> Assumption:` line under FR-5. Only `query` and `operationName` are bounded by characters: `variables`, `extensions` and unknown members stay bounded only by `HTTP_MAX_BODY_BYTES`, so a schema-valid request with a large `variables` can still get the 413 or the 414, and the bundle says so (D2).

## 2. Requirements to stories

`prd.md` defines FR-1 to FR-29 and NFR-1 to NFR-21 with no gap. A script confirmed, for the table "Requirement to stories" of `epics.md`:

- every FR and NFR id is in the table, and every id of the table is defined in `prd.md`;
- for every FR, every NFR, M4 and M5, the stories the table lists and the stories whose Covers line names the id are the same set (M5: story 5.1; FR-3, FR-4, FR-5, FR-6, FR-23, FR-29, NFR-3 and NFR-8 list story 5.1 for the bound, FR-23 also for the tests of the explorer flag);
- AC1 to AC11, M1, M4 and M5 each have a row.

Result: pass. NFR-10, NFR-11 and NFR-12 point to story 5.1 and to the gates every story names.

## 3. Stories, criteria and order

A script read each `### Story N.M:` block of `epics.md` up to the next story or epic heading. For all 19 stories (1.1 to 1.4, 2.1 to 2.7, 3.1, 3.2, 4.1 to 4.5, 5.1):

- the block holds exactly one `**Acceptance Criteria:**` line, at least one `**Given**`, `**When**` and `**Then**`, and every criterion line carries `**When**` and `**Then**`;
- the block holds exactly one `**Depends on:**` line and no inner level-2 or level-3 heading;
- the ids ascend in file order and are unique, and every id a Depends on line names is an earlier story.

Every criterion names an observable result (a status, a header, bytes of the body, a `Duration`, a refusal reason, a lock entry, a document entry, a test by its name or a gate by its `make.*` key). Every test that the criteria of stories 2.6, 4.5 and 5.1 name for M5 and CQ-4 is defined in `crates/service/src/graphql/handler/tests.rs`, `crates/service/src/http/router/tests.rs` or `crates/service/src/openapi/tests.rs`.

Waves. Waves 1 to 9 are done; wave 10 (story 5.1) is in progress. The fix `56a8725` changed files of story 4.5 and is recorded in story 5.1 ("Fix found by a gate"). The CQ-4 refactor changes files of stories 2.6 and 4.5: `handler.rs` and its tests, `bootstrap.rs`, `openapi.rs` and its tests, `http/router.rs` and its tests, and `graphql/document/tests.rs`. Both stories are done. Review finding CQ-9 changes `tail.rs` and its tests, files of story 3.3 of issue #4 and of no story of this bundle, and review finding CQ-10 changes the description of `POST /api/graphql` in `handler.rs` (story 4.5) and so the OpenAPI snapshot; story 5.1 records both ("Fix found by the review"). Story 5.1 depends on both, directly on 4.5 and on 2.6 through 2.7, and the amendment is recorded in stories 2.6, 4.5 and 5.1 ("Fix found by the review"). No story id, wave or Depends on line changes.

Result: pass.

## 4. Architecture contract

- `prd.md` has `## Executive Summary`, `## Functional Requirements`, `## Non-Functional Requirements` and `## Product Scope`, in this order.
- `architecture.md` has `## Tech Stack`: the Rust toolchain 1.99.1 (edition 2024) and the crates the change uses, with `jiff` 1.0.1 in place of `humantime` 2.4.0, and no other stack. M5 and CQ-4 add no crate.
- `## Constraints` is 4 444 bytes from its heading to the next level-2 heading, under the 4 500 limit, with no fenced block and no inner level-2 heading:
  - part (a) comes first: `(#8)` and the `story N.M` subject form, the checked-out branch, never `main`, never pushed by Ralph, hooks never bypassed;
  - part (b) comes second: the independence rule, the `rust-implementer` rule with its sequential fallback, and the ten `make.*` keys with their targets (`make.setup_test_db` and `make.sqlx_prepare` read `not available: capability absent`);
  - the trailing lines carry the placement, the protected line with the four approved commits, the advisories line, the tools line, and the JSON-transport line, which names the length 422 of M5 as its one exception.
- `architecture.md` "Protected Changes" lists the four approved changes, unchanged. The paths that `56a8725` and the working tree of the review fixes CQ-4, CQ-9 and CQ-10 change are none of `protected-config.toml`.
- The design of `graphql/handler.rs` matches the code: one GET handler `subscribe`, the flag `ExplorerPage` in the schema data, `to_request` returning the problem with the length bounds `MAX_QUERY_CHARS` and `MAX_OPERATION_NAME_CHARS`, and `graphql_router()` without an argument. In the error mapping, the length row now follows the three GET rows of `query_string::read`, as in FR-4, the request flow and `get_request` (D1).

Result: pass.

## 5. Findings of this round and their fixes (specs only)

The review of the first readiness round of 2026-11-21 found eight minor issues and no blocking or major one. Each was confirmed against the files before it was fixed.

| Id | Severity | Artifact | Finding | Fix |
| --- | --- | --- | --- | --- |
| D1 | minor | `architecture.md` | In "Error and 503 Mapping", the length row of M5 ("after every row above that applies to the form") stood above the three GET rows, so it read as if a GET met the length check before the query-string checks. That contradicted FR-4, the request flow and `get_request`. | The row moved below the row "GET: `variables` or `extensions` not a JSON object"; its text stays. |
| D2 | minor | `prd.md`, `architecture.md`, `run-summary.md` | The bundle said that no request within the bounds meets the 413 or the 414, and that the document never admits a request that only the size limit refuses. `variables`, `extensions` and unknown members have no bound in the document, so a schema-valid request can still get the 413 or the 414. | The FR-5 assumption (and its copy, prd item 44) says that `query` and `operationName` within their bounds never bring a request to the 413 or the 414, and that `variables`, `extensions` and unknown members stay bounded only by `HTTP_MAX_BODY_BYTES`. The `to_request` bullet of `architecture.md` says that no `query` or `operationName` that the document admits is refused by the size limit alone. |
| D3 | minor | `prd.md`, `run-summary.md` | The Vary assumption of FR-5 (prd item 17) quoted the brief's S8 as "same status and body", but S8 carries the M5 exception. | It reads "same status and body apart from the length refusal of decision M5". |
| D4 | minor | `epics.md`, `research.md`, `run-summary.md` | Story 2.3's goal said that every POST without an event stream keeps today's answer, and the negotiation assumption of `research.md` section 8 (research item 3) said that every other POST keeps the JSON transport unchanged. Neither named M5. | Story 2.3's goal names the length refusal approved as decision M5 (story 5.1). The research assumption and its copy say that the answers of the JSON transport are unchanged apart from the length bounds of decision M5 (amended on 2026-11-21). |
| D5 | minor | `prd.md`, `epics.md` | NFR-3 cites M5 and FR-23 lists the bound tests and the explorer-flag tests, which only story 5.1 pins. Yet the M5 traceability row omitted both, the FR-23 and NFR-3 rows of "Requirement to stories" did not list 5.1, and story 5.1's Covers line named neither. | FR-23 and NFR-3 joined the M5 row of `prd.md` and section 1 of this file. Story 5.1's Covers line names FR-23 (the tests of the bounds and of the explorer flag) and NFR-3 (the character bounds, M5). The table rows list 5.1 with the same reasons. |
| D6 | minor | `epics.md`, `run-summary.md` | Story 5.1's review-fix paragraph listed "`handler.rs`, `bootstrap.rs`, `openapi.rs`, `http/router.rs` and their tests", which implies bootstrap tests that did not change and leaves out `graphql/document/tests.rs`. The run summary counted nine CQ-4 files, but the working tree changes eight. | The paragraph and the "Read" bullet of the correct-course record list the eight files exactly, and the count reads eight. |
| D7 | minor | `implementation-readiness.md` | The first readiness round said that every story, 1.1 to 5.1, was implemented and committed and that all ten waves were done. Story 5.1 has no story commit and stays open until the CQ-4 commit and a run of the whole suite on the final head. | The state of the branch and section 3 say so, and the run summary's record of that round is corrected. |
| D8 | minor | `run-summary.md` | The warning on documents outside the write scope recorded only ADR 0022 point 2 as stale. Line 80 of `docs/architecture.md` still says that every JSON client keeps its answer, which contradicts M5. | The warning names that line and the wording it needs with the CQ-4 commit (`documentation-sync`). |

The two user documents in this round's scope were checked again. `SECURITY.md` names both limits, counted in code points, for the body of a POST on either transport and for the query string of a GET, with the 422 problem whose violations name the bound and never the value. `docs/requests.http` names both limits and the 422 problem on the JSON-transport query example, on the POST subscription ("in that order, then") and on the GET subscription, after the 414, 400 and 422 query-string problems. Neither needed a change in this round.

## 6. Earlier fixes still in force

- 2026-11-21, first readiness round, C1 to C8: the one GET handler of CQ-4 described across the bundle (C1); every statement of the bound citing decision M5 (C2); FR-3 with the six problems of the POST form in order, FR-4 row 4 and FR-6 step 2 (C3); FR-29 naming the narrowing that oasdiff cannot report (C4); NFR-3, NFR-8 and NFR-17 (C5); the handler section, the error mapping, the request flow, the OpenAPI table and ADR 0022 point 6 of `architecture.md` (C6); Schemathesis's oversized strings in `research.md` 3.11 and section 7 (C7); story 5.1's Covers, criteria and commit criterion (C8). Each is still reflected in the artifacts; D1 to D6 refine C3, C5, C6 and C8.
- 2026-11-20, R1 to R7: the lock refresh of story 1.4 by `cargo tree -i jiff` (R1); the four crates the lock gains (R2); the moved duration edges, each pinned, and the bare `0` kept (R3); story 5.1's commit criterion with the planning-bundle commits, the protected commits and `584f868` (R4, extended by C8); the file story 1.4 shares with story 2.3 (R5); the duration successes read through `SHUTDOWN_GRACE_PERIOD` (R6); the approved Schemathesis exclusion `71aab46` stated everywhere (R7).
- 2026-11-17, F1 to F8: AC9 and AC10 integrated (FR-26 to FR-29, NFR-20, stories 4.4 and 4.5); the criteria renumbered to 11; the GET rule of FR-2 for Schemathesis's `Accept: */*`; one-event schemas `SseFrame` and `GraphQLEvent`; `/api/docs/{file}`; the template-sync entries of `ping.rs` and `docs.rs`; the protected tests G17, G19 and G21; `make.schemathesis_validate` in the serialized verify step.
- During implementation: the approved Spectral exception (`a374895`), the cursor rule of the root-cause fix `584f868`, the approved Schemathesis exclusion (`71aab46`), M4 with story 1.4 and `c4f83b3`, the approved plant of `jiff` in test G8 (`49641e7`), and M5 with the root-cause fix `56a8725`.

## 7. Risks and how they are proved

These are not open findings. Each has a criterion or a recorded step that proves it.

| Id | Risk | Proof or handling |
| --- | --- | --- |
| K1 | Parts of the Schemathesis analysis were inferences from its source, not a run against this service | the runs decided: the run on `4dfd92e` led to `584f868` and the approved `71aab46`; the run on `9bf9de6` found the oversized-string case, fixed at its root by `56a8725` (M5); CI on `99da8cb` passed with Schemathesis |
| K2 | A parameter schema can admit a value the route refuses, or the reverse | story 4.4 checked today's values against the schemas; Schemathesis checks both directions, the `maxLength` of `query` and `operationName` included |
| K3 | The Schemathesis job, and the protected G21 test that runs it, grow with the streaming cases | the examples subscribe to `load.test`, so they reach 20 events fast (NFR-20) |
| K4 | In `dev`, curl with its default `*/*` now gets an event stream instead of the explorer | stated in FR-2, ADR 0022 and the documents of story 4.5; browsers keep the explorer |
| K5 | The duration and `occurredAt` equivalences rested on reading the sources of humantime 2.4.0 and jiff 1.0.1 | story 1.4 pins every documented duration, every moved edge and both sides of every bound of the time rule, and its criteria require the mutation gate to name mutants of `config.rs` and `envelope.rs` and leave none alive |
| K6 | jiff 1.0.1 was published on 2026-11-18 | used only through `SpanParser::parse_unsigned_duration` and pinned by tests; `make.check_security` checks its advisories and licences |
| K7 | A JSON client that sends a document over 4 096 characters or an operation name over 128 gets the 422 problem instead of a result (M5), and `make.openapi_diff` cannot report it | the pull request description names it as a behaviour change for JSON clients (FR-29); the violation names the member and its bound; the explorer's introspection query, 2 038 characters, fits; story 5.1's criteria name the tests of both transports and both forms |
| K8 | The CQ-4 refactor is in the working tree and not yet committed, while the bundle describes it; story 5.1 stays open until then | the orchestrator commits it with its gates before or with this bundle; story 5.1's CQ-4 criterion names its four tests, its commit criterion admits the review-fix commit, and its first criterion runs every local gate on the final head |
| K9 | Two documents outside this round's write scope described the state before M5 or CQ-4: `docs/architecture.md` line 80 ("every JSON client keeps its answer") and ADR 0022 point 2 (`graphql_router(app_env)` and both GET handlers) | closed by the CQ-4 change: `documentation-sync` updated `docs/architecture.md` line 80 (a JSON client keeps its answer unless its document or operation name exceeds the bounds of M5) and ADR 0022 points 2 (`graphql_router()` and the one annotated GET handler) and 6 (the approval of the bound as M5), and `run-summary.md` records it in its warnings |

## 8. Open findings

None. D1 to D8 are fixed in the artifacts they name, the earlier fixes stand, and the risks of section 7 are carried by criteria or recorded steps.

## 9. Self-check

Run after the fixes, at most five rounds; the rounds and their output are in `run-summary.md`.

1. `verify-bundle.sh` (the commands of the planning skill's Verification, `N=8`) prints nothing.
2. Each story block of `epics.md` holds its own Acceptance Criteria with Given, When and Then lines and exactly one Depends on line; the ids ascend; every dependency is an earlier story.
3. The staging dry run of the plugin, `stage-planning-artifacts.sh specs/8-graphql-subscriptions-over-sse --dry-run`, exits 0.
4. The directory holds exactly the seven files of the bundle.
5. `make.docs_check` and the repository's Markdown and spelling linters pass.

Verdict: ready
