# Implementation readiness: real-time event delivery hub (issue #4)

Bundle `specs/4-realtime-event-delivery-hub/`. This file replaces the readiness check of the first plan. It cross-checks the new `prd.md` (38 FR, 21 NFR), `architecture.md` (1 444 lines) and `epics.md` (22 stories in 6 epics, 860 lines) against each other, against the issue's 17 criteria and against decisions D1 to D18 of the design input of 2026-11-14.

How the check was done: by reading the three files and by scripted checks (grep and a short Python script over `epics.md` and `prd.md`). No build and no test was run. Two review lenses were run afterwards (semantics, implementability; section 10 and `run-summary.md`).

## 1. Issue criteria to functional requirements

The traceability table at the end of the FRs in `prd.md` maps every criterion. Result: pass.

| Criterion | FR | Check |
| --- | --- | --- |
| 1 Catalog over REST and GraphQL, from configuration, in the snapshots | FR-1, FR-2, FR-3 | pass |
| 2 SSE delivery of subscribed types | FR-5, FR-6, FR-7, FR-18 | pass |
| 3 WebSocket with run-time subscribe and unsubscribe | FR-8 to FR-11 | pass |
| 4 Resume on any node, in order, before live events, no repeat | FR-12 to FR-15 | pass |
| 5 At least once; the client deduplicates by `id` | FR-18, FR-23 | pass; FR-18 sends the whole envelope so the client sees the envelope `id` |
| 6 No consumer group; no leader, election, lease, lock or link | FR-17, FR-22 | pass |
| 7 Subscriptions only when caught up; three 503 answers; status route | FR-4, FR-20, FR-25 | pass |
| 8 PostgreSQL and the example context removed | FR-31 to FR-34 | pass |
| 9 Kafka client behind a domain port | FR-30 | pass |
| 10 Invalid records skipped, logged, counted | FR-19, FR-4 | pass |
| 11 Slow clients never block others | FR-24, FR-25, FR-11 | pass |
| 12 RFC 9457 problems for invalid input | FR-16, FR-1 | pass |
| 13 `CORS_ALLOWED_ORIGINS` | FR-28, FR-29 | pass |
| 14 Dead reader loop: exit status 1 after the normal shutdown | FR-26, FR-27 | pass |
| 15 One broker in the stack and CI; tests against it | FR-35, FR-36 | pass |
| 16 K6 smoke scenario with SSE and WebSocket subscribers | FR-37 | pass |
| 17 Every local gate of `make ci`, 100 % coverage, no surviving mutant | FR-29, FR-32, FR-38, NFR-17 to NFR-21 | pass |

## 2. Requirements to stories

The table "Requirement to stories" at the end of `epics.md` lists FR-1 to FR-38 and NFR-1 to NFR-21. A script confirmed three things:

- every id is in the table;
- every id in the table is defined in `prd.md`;
- every story named in the table exists.

Result: pass.

NFR-17 and NFR-18 point to story 6.1 and to the gates that every story names. That is enough, because each story carries its own coverage and mutation bar.

## 3. Names across the three files

| Kind | Check | Result |
| --- | --- | --- |
| Variables | The 23 variables of FR-29 appear with the same spelling, defaults and ranges in `architecture.md` and `epics.md`; they are those of section 9 of the design brief, except the defaults of `KAFKA_BOOTSTRAP_SERVERS` and `KAFKA_TOPIC` (forced by the protected image test, an assumption of FR-29) and the four security variables (an addition the brief allows). The first plan's variables appear only in "gone" lists. | pass |
| Routes | `GET /api/event-types`, `GET /api/events/stream`, `GET /api/events/ws`, `GET /api/events/status`, GraphQL `eventTypes` and `eventStreamStatus` agree in all three files. | pass |
| Status fields | The PRD names `state`, `reason`, `brokerReachable`, `position`, `endOffset`, `lag`, `windowFirst`, `windowLast`, `subscribers`, `rejectedRecords`, `retentionLosses` and `replayCapped`. The architecture names the same fields as `NodeStatus` and `EventStreamStatusResponse`, with camel case on the wire. The reasons `broker-unreachable`, `topic-not-found`, `partition-count` and `catching-up` map to `StartReason` in kebab case. | pass |
| Ports and types | `TopicProbe`, `TopicTail`, `TopicReplay`, `EventPublisher`, `SubscriberCount`, `TailEvents`, `Window`, `Hub`, `Subscriptions`, `StoredEvent`, `EventCatalog`, `EventStreamQuery`, `KafkaProbe`, `KafkaTail`, `KafkaReplay`, `EventStreamProbe` and `Dependency::EventStream` are used in `epics.md` as the architecture defines them. | pass |
| Crates | `event-stream-domain`, `event-stream-application` and `event-stream-infrastructure` under `crates/notification/event-stream/`, plus `crates/service`. | pass |
| Log messages and 503 details | `event tail ready`, `background task failed`, `the node is starting` and the others of NFR-15 and FR-25 agree. | pass |
| Load test | `subscriptions.js`, `load.test`, `sentAtMs`, `event_delivery_latency` and `deliveryThreshold` agree. | pass |
| Stack | `ensure-kafka` and `docker-compose.kafka-host.yml` are named by the architecture and the stories. The PRD leaves both to the architecture (its assumption 29). | pass |

## 4. Order of the stories

- A script confirmed the story structure of `epics.md`:
  - story ids ascend in file order;
  - each of the 22 stories has `**Given**`, `**When**` and `**Then**` lines and exactly one `**Depends on:**` line;
  - no story depends on itself or on a later story.
- Story 1.1 is "Scaffold notification/event-stream", depends on none, and carries C1 to C12. `notification/event-stream` is absent from the workspace members, so the story 1 rule applies. Result: pass.
- Decision D16 requires three things. Each holds:
  - Every protected change the feature needs, and the shortened `required_directories` the removal needs in the base branch, are one story, 3.1: the preparatory pull request against `main`, made and merged by the maintainer, first story of Epic 3. The rust-implementer agent never runs it.
  - The removal's other protected changes are in story 5.1 only, made in an approved operator session in pull request #5.
  - Stories 1.1 to 2.4 change no protected file and need no entry of story 3.1; they come first, in their order. Stories 3.2 to 4.4 change no protected file and run after story 3.1 is merged into `main` and `main` into the feature branch; see finding R1.
- D10 requires four things. Each holds:
  - The context is scaffolded (1.1) before any crate is removed (5.1).
  - `eventTypes` and `eventStreamStatus` (both 2.4) exist before the example roots go.
  - The removal is the last epic before the verification.
  - The proof stories (Epic 4) come before the removal.

Result: pass.

## 5. Nothing of the first plan is left

These terms were searched in `prd.md`, `architecture.md`, `epics.md` and `product-brief.md`: `lease`, `leader`, `relay`, `prune`, `SQS queue`, `EVENT_QUEUE_URL`, `internal/cluster`, `notification_events`, `LISTEN` and `DATABASE_URL`.

Every match is one of three kinds:

- a statement that the thing is gone or does not exist;
- the Kafka term "partition leader" (NFR-5, NFR-6 and the failure table of the architecture);
- `DATABASE_URL` in the removal inventory, or in the protected rendering test that story 1.3 must keep passing before the removal.

No requirement, design element or story needs any of them. Result: pass.

## 6. The Constraints section

`## Tech Stack` (line 5) comes before `## Constraints` (line 36). The section was checked against the rules of step 5 of `rs-sdlc-plan.md`:

- It is 3 449 bytes from its heading to the next level-2 heading, below the limit of 4 500.
- Part (a) comes first. It holds `(#4)` and `story N.M`: hooks active, the feature branch, never `main`, never pushed by Ralph.
- Part (b) comes second. It holds the independence rule, the `rust-implementer` rule with its sequential fallback, and all ten `make.*` keys, each with its resolved target from `.claude/rust-sdlc.yml`.
- After the removal story, `make.setup_test_db` and `make.sqlx_prepare` read `not available: capability absent`.
- It holds no fenced block and no further level-2 heading.

Result: pass.

## 7. Fixes made in this check

1. `prd.md` FR-14: the replay floor was `max(c, L, P - EVENT_REPLAY_MAX_OFFSETS)`, which contradicted FR-14's own sentence that with `EVENT_REPLAY_MAX_OFFSETS` = 0 a cursor below `F` starts at `F` (the formula gives `P`). It also differed from `architecture.md` (algorithm 3) and from the test of story 3.3. It then read `max(c, L, min(F, P - EVENT_REPLAY_MAX_OFFSETS))`. The semantics review replaced it by the design brief's rule `max(c, P - EVENT_REPLAY_MAX_EVENTS)` with `BelowEarliest` found by the first Kafka page (section 9).
2. `prd.md` NFR-19: it named `krafka` (and maybe `tokio`) as the only protected change of the feature. The architecture also adds `serde_json` to the application allow-list and `tokio-tungstenite` to the application dev list, in the same approved story. NFR-19 now lists all four entries.
3. `product-brief.md`, section "Needs the maintainer's approval": the same incomplete allow-list was replaced by the four entries.
4. `product-brief.md`: the sentence "The feature epics do not wait for these approvals" contradicted `epics.md`, where stories 3.2 to 4.4 follow the allow-list story (now the preparatory pull request, story 3.1). It now says that the feature does not wait for the removal approvals, and which feature stories come before and after its own allow-list approval.

## 8. Open findings

None is blocking.

| Id | Severity | Finding | Why it does not block |
| --- | --- | --- | --- |
| R1 | medium | D16 asks that every story that does not need a protected change can run before the approval. Stories 3.2 to 4.4 change no protected file but wait for the preparatory pull request of story 3.1, because the envelope parser needs `serde_json` in the application layer and the plugin refuses protected changes on the feature branch. Most of the feature therefore waits for the maintainer's merge. | Recorded in `epics.md`. Story 3.1 is requested in wave 1, so the merge can happen while waves 1 to 5 (scaffold, catalog, configuration, broker, domain, window, hub, status) run. |
| R2 | medium | `cargo deny` and `cargo tree` were never run with `krafka` (`default-features = false`, `rustls-aws-lc-rs`). Whether `aws-lc-rs` stays in the lock after `sqlx` leaves was not checked. | Story 3.5 runs `make.check_security` and escalates a refusal instead of editing `deny.toml`. |
| R5 | low | The Kafka image digest was read from a local image. | The operator confirms it against the registry; the implementer of story 1.3 uses the recorded digest. |
| R6 | low | The fake broker of `krafka` was read from source only. It differs from a real broker (seek above the end, log start). | Used only for fault cases. Behaviour cases run against the real broker (FR-36). |
| R7 | low | Story 5.1 holds removal parts A and B, two commits, and is large; `make ci` passes only with the documentation of story 5.2. | `postgres-removal.md` shows the parts are green in `make ci` only together; story 5.2 carries the `make ci` criterion. |
| R8 | low | No capacity, latency or throughput figure exists. NFR-1 is a design target. | Stated as such in `prd.md`, `architecture.md` and `research.md`, as D18 requires. |
| R9 | low | Two of the three review lenses ran (semantics, implementability); the third, contract and traceability, did not, because the Fable usage limit was nearly reached. | The contract checks ran by script (section 10). Story 6.1 and the plugin's review stages review the change again. |
| R10 | low | The required status check "Persistence query metadata" must be removed from the branch rules of `main` by the maintainer. | A hand-off in story 5.1. It needs no repository change. |

## 9. Corrections after the semantics review

The semantics review (findings S1 to S16) found the bundle departing from the binding design brief. The three main files now follow the brief in every point the review named, and the former findings R3 (cursor above the topic end) and R4 (six departures from the brief) are closed:

- a cursor above the topic end is clamped after two watermark reads (FR-15, brief 6.1);
- a batch the client cannot decode is `Unavailable`, nothing is skipped, and the node turns not ready (FR-19, brief 10.1);
- readiness follows `EVENT_TAIL_STALE_AFTER` (FR-20, brief 3.7);
- the pause after a failed round is the fixed `KAFKA_RETRY_DELAY` and the loop cuts no round (FR-26, brief 4.8 and 4.9);
- the replay floor is `max(c, P - EVENT_REPLAY_MAX_EVENTS)` with the counter `replayCapped`, and the stall rule counts three pages (FR-14, brief 6.1 and 6.2);
- the variables, defaults and ranges are those of brief section 9 (FR-29);
- the 503 answers follow validation again (FR-16, the first plan's order kept by D5);
- `Hub::close` drops every sender, `Hub::deliver` moves the position without a subscriber, the adapter takes the long-poll wait from `KafkaSettings`, the re-seek after a failed fetch is in the same round, the log entries `event topic unreachable`, `events lost to retention` and `replay capped` and the counter `retentionLosses` are added, and `StaticTopic` yields on an idle fetch.

## 10. Review of 2026-11-14

Two review lenses ran on Fable: semantics (S1 to S16) and implementability (I1 to I18). The third lens, contract and traceability, was not run because the Fable usage limit was nearly reached; its checks were run by script (`verify-bundle.sh` printed nothing; the staging dry run exited 0 with six WOULD lines; a script confirmed that the 22 story ids ascend in file order, that each story has its own Acceptance Criteria with Given, When and Then lines and exactly one Depends on line, and that each Depends on line names only earlier stories). Each finding below is resolved or open.

| Id | Severity | Status | Resolution |
| --- | --- | --- | --- |
| S1 | blocker | resolved | a cursor ahead waits for two observations and is clamped to the end, never refused (FR-15, story 3.4) |
| S2 | blocker | resolved | an undecodable batch is `Unavailable`; nothing is skipped (FR-19, stories 2.1, 3.3, 3.5) |
| S3 | major | resolved | readiness follows `EVENT_TAIL_STALE_AFTER` with four reasons (FR-20, story 3.8) |
| S4 | major | resolved | the pause after a failed round is the fixed `KAFKA_RETRY_DELAY` (story 3.8) |
| S5 | major | resolved | the replay floor is `max(c, p - MAX)` with `replayCapped` and `replay capped` (story 3.4) |
| S6 | major | resolved | the stall rule counts three pages without progress (story 3.4) |
| S7 | major | resolved | the 23 variables follow brief section 9 in the three main files (story 1.2) |
| S8 | major | resolved | `Hub::close` drops every sender and keeps the entries (story 2.3) |
| S9 | major | resolved | `deliver` moves the hub position without a subscriber (stories 2.3, 3.4) |
| S10 | major | resolved | the long-poll wait is `KAFKA_FETCH_MAX_WAIT` in `KafkaSettings` (stories 1.2, 3.3, 3.5) |
| S11 | major | resolved | the re-seek after a failed fetch or seek is in the same round (story 3.3) |
| S12 | minor | resolved | the first plan's order 403, 400/422, 503, upgrade 4xx is back (stories 3.6, 3.7) |
| S13 | minor | resolved | FR-13 cost, FR-21 re-seek trigger and FR-27 steps corrected |
| S14 | minor | resolved | transport `ws` or `sse`; the ping is sent at every interval (story 3.7) |
| S15 | minor | resolved | `TopicFault`, `event topic unreachable`, `events lost to retention`, no `round_limit` (stories 2.2, 3.3, 3.8) |
| S16 | minor | resolved | `StaticTopic::fetch` yields; pooled reader and current-thread tests (stories 2.1, 3.5, 3.8) |
| I1 | blocker | resolved | all protected changes the feature needs, with the shortened `required_directories`, are the maintainer's preparatory pull request, story 3.1; the removal runs in an approved operator session (stories 5.1, 5.2, 6.1); Constraints, "Protected Changes" and `run-summary.md` say so |
| I2 | blocker | resolved | story 3.8 scope holds `tests/e2e/world.rs` and the service tests that expect 204; `send` waits for readiness; criterion for the unchanged `features/health_check.feature` |
| I3 | blocker | resolved | the GraphQL catalog moved from story 1.1 to story 2.4, so `EventStreamStatus` and `EventType` enter the snapshot in one commit; FR-2 maps to 2.4 |
| I4 | blocker | resolved | story 5.1 no longer claims `docs-check` or `make ci`; story 5.2 carries `make ci` with both approvals and `make.ci_cli` |
| I5 | major | resolved | story 1.3 adds the two `docs/environment.md` rows and `ensure-kafka` in `docs/make-targets.md`, with a criterion each |
| I6 | major | resolved | resolved differently: S12 deleted the `availability` middleware, so `router<S>()` needs no hub value; recorded as an Assumption in `architecture.md` |
| I7 | major | resolved | `Adapters` holds `topic_probe`, `topic_tail` and `topic_replay` as trait objects (stories 3.6, 3.8) |
| I8 | major | resolved | story 1.1 domain has `thiserror` only; story 2.1 adds `async-trait`, `shared-domain`, `uuid` and the `workspace.dsl` edge with its gate; story 3.5 adds its edges and entries |
| I9 | major | resolved | `Uuid` is `uuid::Uuid`; `id.len() == 36` before `try_parse`; story 3.2 rejects simple, braced and URN forms |
| I10 | major | resolved | the removal keeps the `event_type` fixture of `fixture.rs` (architecture, story 5.1) |
| I11 | major | resolved | the frame wrapper is written by hand in `architecture.md`, tested against `serde_json` as a dev-dependency |
| I12 | major | resolved | `StaticTopic` end-of-log behaviour stated; the supervised-loop test uses a pacing adapter that sleeps `KAFKA_FETCH_MAX_WAIT` (Assumption: `TailEvents::WAIT` no longer exists) |
| I13 | major | resolved | story 4.3 probes `http.asyncRequest` first and falls back to the subscriber count; FR-37 and `architecture.md` mirror it |
| I14 | minor | resolved | `docker-compose.prod.yml` `app.depends_on` becomes `!reset null` (story 5.1) |
| I15 | minor | resolved | the two health-check files, full-path listing and test files with `Adapters {` are in the template-sync rules (stories 3.8, 4.1, 5.1) |
| I16 | minor | resolved | Kafka tools by `/opt/kafka/bin/..`; restart through `make.start`, `make stop`, `make.start`; the operator confirms the digest |
| I17 | minor | resolved | `clock` left the field comment of `TailEvents` |
| I18 | minor | resolved | stories 3.6 and 3.7 name the extractor exception in a criterion |

Earlier findings changed by this review: R1, R2, R5, R7, R9 and R10 (section 8) and the D16 and D10 checks of section 4. No blocker or major finding is open.

Verdict: ready
