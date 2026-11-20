# Implementation readiness: GraphQL subscriptions over Server-Sent Events (issue #8)

Bundle `specs/8-graphql-subscriptions-over-sse/`, branch `claude/vilnacrm-notification-service-br7s47`, pull request #9, checked on 2026-11-20 after the amendment round of the same day. This check cross-reads the five artifacts (`research.md`, `product-brief.md`, `prd.md`, `architecture.md`, `epics.md`) against each other, against issue #8 as it reads on 2026-11-17 with its 11 acceptance criteria, and against the maintainer decisions M1 (graphql-sse over SSE with the GraphQL over HTTP request formats), M2 (planned with BMAD through the rust-backend-sdlc plugin), M3 (only issue #8 is planned) and M4 of 2026-11-20 ("Ignore both, replace ours": the two `deny.toml` entries and one new story that replaces humantime).

How the check was done:

- by reading the five files, the repository files they cite and the branch history `c36f708..c4f83b3`;
- by scripts over `prd.md` and `epics.md` (requirement ids, the table "Requirement to stories", the Covers lines, the structure of each story block, the waves from the Depends on lines, the scope overlap between stories);
- by reading the registry source of humantime 2.4.0, the crate file and the crates.io index entries of jiff 1.0.1, jiff-core 1.0.1, jiff-static 1.0.1 and portable-atomic-util 0.2.8, and `Cargo.lock`, `deny.toml`, `make/gates.mk`, the `update` target of the Makefile and the plugin's `agents/rust-implementer.md`.

No build, test or cargo command was run.

State of the branch: stories 1.1 to 1.3, 2.1 to 2.4 and 4.4 are implemented and committed; 1.4, 2.5 to 2.7, 3.1, 3.2, 4.1 to 4.3, 4.5 and 5.1 are still to do. The maintainer approved three protected changes, committed outside the stories: `a374895` (`.spectral.yaml`), `71aab46` (`make/gates.mk`) and `c4f83b3` (`deny.toml`).

## 1. Issue criteria and decisions to requirements

The traceability table of `prd.md` maps every criterion and decision; a script confirmed that every id it names is defined. Only the rows that changed since the check of 2026-11-17 are marked.

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
| AC9 every route in the OpenAPI document and on the Swagger page; a test fails for a route without an entry | FR-25 (ADR 0022), FR-26, FR-27, FR-28 | S13 | pass |
| AC10 the OpenAPI gates and Schemathesis pass; an exclusion only when a streaming endpoint cannot be tested | FR-2 (GET rule), FR-29, NFR-16, NFR-20 | S14 | pass after R7 (the approved exclusion of `GET /api/events/ws` is now stated in FR-29, NFR-20, S14 and story 4.5) |
| AC11 the two documents; every local gate of `make ci`, 100 % coverage, no surviving mutant | FR-23, FR-24, FR-25, NFR-10 to NFR-16, NFR-19, NFR-21 | S11, S12 | pass (NFR-21 added on 2026-11-20) |
| M1 graphql-sse over SSE, GraphQL over HTTP request formats, every protocol choice recorded | FR-2 to FR-8, FR-25 (ADR 0021) | S11 | pass |
| M4 (2026-11-20) the two advisories ignored by the approved `deny.toml` entries; humantime replaced in the two service crates, the documented duration syntax and the envelope time rule kept | NFR-21, NFR-15, NFR-17 | S15 | pass after R1, R2, R3 and R6 |

The issue's six open points are decided by `> Assumption:` lines of `research.md` section 8 and restated in `prd.md`. The issue's out-of-scope items (a GraphQL WebSocket protocol, subscriber authorization) are out of scope in `prd.md` "Product Scope". M4's own decisions (jiff for the durations, a calendar rule without a crate for the envelope, the bare `0` kept, the other moved duration edges taking jiff's answer) are `> Assumption:` lines of `research.md`, `prd.md` NFR-21 and `architecture.md` "Durations and Timestamps without humantime".

## 2. Requirements to stories

`prd.md` defines FR-1 to FR-29 and NFR-1 to NFR-21 with no gap. A script confirmed, for the table "Requirement to stories" of `epics.md`:

- every FR and NFR id is in the table, and every id of the table is defined in `prd.md`;
- for every FR, every NFR and M4, the stories the table lists and the stories whose Covers line names the id are the same set (NFR-21: stories 1.4 and 5.1; M4: stories 1.4 and 5.1);
- AC1 to AC11, M1 and M4 each have a row.

Result: pass. NFR-10, NFR-11 and NFR-12 point to story 5.1 and to the gates every story names.

## 3. Stories, criteria and order

A script read each `### Story N.M:` block of `epics.md` up to the next story or epic heading. For all 19 stories (1.1 to 1.4, 2.1 to 2.7, 3.1, 3.2, 4.1 to 4.5, 5.1):

- the block holds exactly one `**Acceptance Criteria:**` line, at least one `**Given**`, `**When**` and `**Then**`, and every criterion line carries `**When**` and `**Then**`;
- the block holds exactly one `**Depends on:**` line and no inner level-2 or level-3 heading;
- the ids ascend in file order and are unique, and every id a Depends on line names is an earlier story.

Every criterion names an observable result (a status, a header, bytes of the body, a `Duration`, a refusal reason, a lock entry, a document entry or a gate by its `make.*` key).

Waves. Waves 1 to 5 are done (1.1, 1.2, 2.1; 1.3; 2.2, 4.4; 2.3; 2.4). Story 1.4 depends on no story and joins wave 6, the first wave not started, with 2.5, 3.1 and 4.3. The remaining waves follow from the Depends on lines: 2.6; then 2.7, 3.2, 4.1 and 4.2; then 4.5; then 5.1, which depends on 1.4.

Rule (iii) was checked against the Scope lists:

- no two stories of one wave edit the same file;
- no remaining story names a file that story 1.4 edits, so the only edge added is 5.1 on 1.4;
- the one file story 1.4 shares with an implemented story is `crates/service/Cargo.toml`, edited by story 2.3 (`d92f9b1`, wave 4); history already orders the two, and the bundle contract forbids an edge to a later id (finding R5).

Result: pass. The story-1 rule adds nothing: both touched crates are live members of `[workspace].members`, and story 1.4 adds no crate to the workspace.

## 4. Architecture contract

- `prd.md` has `## Executive Summary`, `## Functional Requirements`, `## Non-Functional Requirements` and `## Product Scope`, in this order.
- `architecture.md` has `## Tech Stack`: the Rust toolchain 1.99.1 (edition 2024) and the crates the change uses, with `jiff` 1.0.1 (`default-features = false`, feature `std`) in place of `humantime` 2.4.0, and no other stack.
- `## Constraints` is 4 307 bytes from its heading to the next level-2 heading, under the 4 500 limit, with no fenced block and no inner level-2 heading:
  - part (a) comes first: `(#8)` and the `story N.M` subject form, the checked-out branch, never `main`, never pushed by Ralph, hooks never bypassed;
  - part (b) comes second: the independence rule, the `rust-implementer` rule with its sequential fallback, and the ten `make.*` keys with their targets (`make.setup_test_db` and `make.sqlx_prepare` read `not available: capability absent`);
  - the trailing lines carry the placement, the protected line with the three approved commits, the advisories line (NFR-21), and the tools line, whose one exception is story 1.4's lock refresh `cargo tree -i jiff` (finding R1).
- `architecture.md` "Protected Changes" lists the three approved changes with their exact change, commit, approval date and reason: `.spectral.yaml` `a374895`, `make/gates.mk` `71aab46` and `deny.toml` `c4f83b3`. `prd.md` NFR-15, `research.md` section 6, the brief's approvals, the protected-changes paragraph of `epics.md` and story 5.1 name the same three.
- No story edits a protected file. Story 1.4 changes `[workspace.dependencies]`, which is not a protected path. It keeps `architecture.toml` unchanged: `jiff` enters only `crates/service`, whose layer allows every crate, and the envelope rule needs no crate.

Result: pass.

## 5. Findings of this round and their fixes (specs only)

| Id | Severity | Artifact | Finding | Fix |
| --- | --- | --- | --- | --- |
| R1 | blocking | `epics.md`, `architecture.md`, `research.md` | Story 1.4 could not produce its `Cargo.lock` under the bundle's own rules. The Constraints forbade calling cargo directly, every make target that compiles passes `--locked` (`make/gates.mk:3-23`), and `make update` (`Makefile:184-185`) was excluded. The parallel stories of wave 6 share the tree, so their `--locked` targets would fail until the lock matched. | The lock is refreshed by one raw call, the rust-implementer's allowed dependency probe `cargo tree -i jiff` (`agents/rust-implementer.md:190`) without `--locked`. It runs once, as story 1.4's first step right after the manifest edits, and writes only the minimal lock change. The maintainer's check `cargo tree --workspace -e no-dev -i humantime` follows, and its empty output is recorded. This is stated in story 1.4's Scope and criterion, in rule (ii), in the architecture's "Manifests, the lock and the documentation", in research 9.6, and in the Constraints tools line as its one exception (4 253 to 4 307 bytes). |
| R2 | major | `epics.md`, `architecture.md`, `research.md`, `prd.md` | The bundle said the lock gains exactly `jiff` and `jiff-core`. `Cargo.lock` records every non-optional dependency of every target, so it also gains `jiff-static` 1.0.1 (`=1.0.1` behind `cfg(any())`) and `portable-atomic-util` 0.2.8 (behind `cfg(not(target_has_atomic = "ptr"))`). Research 9.4 also gave jiff edition 2021, where its manifest says 2024. | The lock statements now name the four crates and say that no locked version is raised (`portable-atomic` 1.15.0, `syn` 3.0.10, `proc-macro2` and `quote` already satisfy them). The `make.check_security` criterion names their licences: `jiff-static` is `Unlicense OR MIT`, `portable-atomic-util` is `Apache-2.0 OR MIT`, both admitted. Verified Library Fact 9 and research 9.4 now read edition 2024. |
| R3 | major | `research.md`, `prd.md`, `architecture.md`, `epics.md`, `product-brief.md` | The list of duration edges that move was incomplete and partly wrong, and not every edge was pinned by a test. humantime already reads `1.5s`. A bare `0`, a leading space and a space inside a number are accepted today and refused by jiff. `+2s`, `1,5s`, `01:30:00` and `1h1h` move but were not pinned. | Research 9.4 and NFR-21 now list what moves in each direction and what does not move (verified in `humantime-2.4.0/src/duration.rs` and jiff's `src/fmt/friendly/parser.rs` and `src/fmt/util.rs`). A no-break space before the unit is added as a further moved edge (jiff takes ASCII spaces only). `> Assumption:` lines (prd NFR-21, the architecture section, research 33, brief 8) record the decision on `0`: it keeps `Duration::ZERO` through a one-line guard in `parse_duration`, because an unranged variable set to `0` would otherwise stop a node at startup after an upgrade. Every other moved edge takes jiff's answer, because no file uses one. Story 1.4's third criterion and the `config/tests.rs` design pin each edge with its result: `0` reads zero, `+2s` 2 s, `1,5s` 1 500 ms, `01:30:00` 5 400 s; `1d`, `2w`, `1M`, `1y`, `30s1m`, `1h1h`, ` 2s`, `1 0s` and `1\u{a0}s` are refused with `must be a duration such as 500ms, 2s or 1m`; `1.5s` and `1m 30s` read as today. `docs/environment.md` line 9 also names `0`. |
| R4 | major | `epics.md` | Story 5.1's commit criterion allowed only story commits and the single bundle commit. Six commits on the branch (`a374895`, `71aab46`, `c4f83b3`, `584f868`, `f0e5b34`, `0d68c27`) and the commit that records this amendment fail that test. | Every subject since `c36f708` passes `make.check_commit_message` as `<type>(#8): <subject>`. Each story commit reads `<type>(#8): story N.M <title>`. The only other commits are the planning-bundle commits (`docs(#8): ...`), the three approved protected commits and the root-cause fix `584f868`. No hook bypassed, no implementer push. |
| R5 | minor | `epics.md`, `architecture.md`, `run-summary.md` | The wave-6 assumption ("no earlier story edits a file of it") and Build Order step 9 ("shares no file with steps 4 to 8") were false: story 2.3 (`d92f9b1`) edited `crates/service/Cargo.toml`. | The assumption now names that shared file and says history orders the two; story 1.4 keeps "Depends on: none" because no story may depend on a later id. Step 9 now reads that it shares only `crates/service/Cargo.toml` with step 4, which is done. The amendment record in `run-summary.md` is corrected. |
| R6 | minor | `epics.md`, `architecture.md` | `HTTP_REQUEST_TIMEOUT` was the example variable for the duration successes. It must exceed `HEALTH_PROBE_TIMEOUT` (4 s in `valid()`), so `1.5s`, `300ms`, `500ms`, `2s` and `3001ms` would fail on the cross-field rule. | The success values and the moved edges are read through `SHUTDOWN_GRACE_PERIOD`, which is unranged and under no cross-field rule (`crates/service/src/config.rs:157-161`, `:251-269`). `HTTP_REQUEST_TIMEOUT` serves refusals only. |
| R7 | minor | `epics.md`, `prd.md`, `product-brief.md`, `architecture.md` | Story 4.5, FR-29, NFR-20 and S14 still required unchanged Schemathesis flags with all eleven operations tested, which the approved `71aab46` makes impossible on this branch. | Story 4.5 (goal and criterion): `make.schemathesis_validate` runs with the protected flags as amended by `71aab46` and passes with ten of the eleven operations tested; `GET /api/events/ws` is excluded only by that change and stays on the Swagger page. FR-29, NFR-20, the PRD scope and out-of-scope rows, goal 10, S14 and the brief's approvals now say the same. So do the architecture's design goal, OpenAPI section, Schemathesis section and decision record point 6 (which ADR 0022 records). Prd assumption 42 and brief assumption 7 note how the escalation ran. |

## 6. Earlier fixes still in force

- 2026-11-17, F1 to F8: AC9 and AC10 integrated (FR-26 to FR-29, NFR-20, stories 4.4 and 4.5); the criteria renumbered to 11; the GET rule of FR-2 for Schemathesis's `Accept: */*`; one-event schemas `SseFrame` and `GraphQLEvent`; `/api/docs/{file}`; the template-sync entries of `ping.rs` and `docs.rs`; the protected tests G17, G19 and G21; `make.schemathesis_validate` in the serialized verify step. Each is still reflected in the artifacts.
- 2026-11-17, during implementation: the approved Spectral exception (`a374895`), the cursor rule of the root-cause fix `584f868` (FR-11, FR-13, stories 1.1 and 1.3) and the approved Schemathesis exclusion (`71aab46`).
- 2026-11-20, M4: NFR-21, story 1.4, the section "Durations and Timestamps without humantime", "Protected Changes" with `c4f83b3`, S15 and goal 11.

## 7. Risks carried into the stories

These are not open findings. Each has a criterion that proves it, and the bundle states what happens when it fails.

| Id | Risk | Proof or handling |
| --- | --- | --- |
| K1 | Parts of the Schemathesis analysis are inferences from its source, not a run against this service | story 4.5 runs `make.schemathesis_validate`. A failure is fixed at its root, or the story stops and escalates with the exact exclusion. The one exclusion made, `GET /api/events/ws`, was approved by the maintainer (`71aab46`). |
| K2 | A parameter schema can admit a value the route refuses, or the reverse | story 4.4 checked today's values against the schemas; story 4.5's Schemathesis run checks both directions |
| K3 | The Schemathesis job, and the protected G21 test that runs it, grow with the streaming cases | the examples subscribe to `load.test`, so they reach 20 events fast (NFR-20) |
| K4 | In `dev`, curl with its default `*/*` now gets an event stream instead of the explorer | stated in FR-2, ADR 0022 and the documents of story 4.5; browsers keep the explorer |
| K5 | The duration and `occurredAt` equivalences rest on reading the sources of humantime 2.4.0 and jiff 1.0.1, not on a run | story 1.4 pins every documented duration, every moved edge and both sides of every bound of the time rule. Every existing config and envelope test passes, unchanged apart from the expected values of `each_kafka_duration_accepts_its_bounds`, and `make.mutation_tests_diff` must name mutants of `config.rs` and `envelope.rs` and leave none alive. |
| K6 | jiff 1.0.1 was published on 2026-11-18, and the lock refresh resolves it from the registry | it is used only through `SpanParser::parse_unsigned_duration` and pinned by tests. `make.check_security` checks its advisories and licences. Story 1.4's criterion fixes the exact lock change (four crates added, no locked version raised), so a different resolution fails the story. |

## 8. Open findings

None. R1 to R7 are fixed in the artifacts they name, the earlier fixes stand, and the risks of section 7 are carried by story criteria.

## 9. Self-check

Run after the fixes, at most five rounds; the rounds and their output are in `run-summary.md`.

1. `verify-bundle.sh` (the commands of the planning skill's Verification, `N=8`) prints nothing.
2. Each story block of `epics.md` holds its own Acceptance Criteria with Given, When and Then lines and exactly one Depends on line; the ids ascend; every dependency is an earlier story.
3. The staging dry run of the plugin, `stage-planning-artifacts.sh specs/8-graphql-subscriptions-over-sse --dry-run`, exits 0.
4. The directory holds exactly the seven files of the bundle.
5. `git status --porcelain` lists only files under `specs/8-graphql-subscriptions-over-sse/`.

Verdict: ready
