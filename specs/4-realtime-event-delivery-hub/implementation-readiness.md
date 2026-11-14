# Implementation readiness: real-time event delivery hub (issue #4)

Bundle `specs/4-realtime-event-delivery-hub/`. This file replaces the readiness check of the first plan. It cross-checks the new `prd.md` (38 FR, 21 NFR), `architecture.md` (1 430 lines) and `epics.md` (22 stories in 6 epics) against each other, against the issue's 17 criteria and against decisions D1 to D18 of the design input of 2026-11-14.

How the check was done: by reading the three files and by scripted checks (grep and a short Python script over `epics.md` and `prd.md`). No build and no test was run. No multi-lens validation round was run (see `run-summary.md`).

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
| Variables | The 15 variables of FR-29 appear with the same spelling in `architecture.md` and `epics.md`. `EVENT_TAIL_STALE_AFTER` and `EVENT_WINDOW_MAX_EVENTS` appear only in the architecture's assumption that the design brief's names are replaced by the PRD's. The first plan's variables appear only in "gone" lists. | pass |
| Routes | `GET /api/event-types`, `GET /api/events/stream`, `GET /api/events/ws`, `GET /api/events/status`, GraphQL `eventTypes` and `eventStreamStatus` agree in all three files. | pass |
| Status fields | The PRD names `state`, `reason`, `brokerReachable`, `position`, `endOffset`, `lag`, `windowFirst`, `windowLast`, `subscribers` and `rejectedRecords`. The architecture names the same fields as `NodeStatus` and `EventStreamStatusResponse`, with camel case on the wire. The reasons `broker-unreachable`, `topic-not-found`, `partition-count` and `catching-up` map to `StartReason` in kebab case. | pass |
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
  - The feature allow-list is one story, 3.1. It comes first among the stories that need it.
  - The removal's protected changes are in stories 5.1 and 5.2 only. 5.1 is a separate pull request merged into `main` first.
  - Stories 1.1 to 2.4 and 1.3 change no protected file and run before the approval. Stories 3.2 to 4.4 change no protected file but use the envelope parser of story 3.1, which needs `serde_json` in the application layer. This is recorded in `epics.md` as an assumption; see finding R1.
- D10 requires four things. Each holds:
  - The context is scaffolded (1.1) before any crate is removed (5.2).
  - `eventTypes` (1.1) and `eventStreamStatus` (2.4) exist before the example roots go.
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

- It is 3 241 bytes from its heading to the next level-2 heading, below the limit of 4 500.
- Part (a) comes first. It holds `(#4)` and `story N.M`: hooks active, the feature branch, never `main`, never pushed by Ralph.
- Part (b) comes second. It holds the independence rule, the `rust-implementer` rule with its sequential fallback, and all ten `make.*` keys, each with its resolved target from `.claude/rust-sdlc.yml`.
- After the removal story, `make.setup_test_db` and `make.sqlx_prepare` read `not available: capability absent`.
- It holds no fenced block and no further level-2 heading.

Result: pass.

## 7. Fixes made in this check

1. `prd.md` FR-14: the replay floor was `max(c, L, P - EVENT_REPLAY_MAX_OFFSETS)`, which contradicted FR-14's own sentence that with `EVENT_REPLAY_MAX_OFFSETS` = 0 a cursor below `F` starts at `F` (the formula gives `P`). It also differed from `architecture.md` (algorithm 3) and from the test of story 3.3. It now reads `max(c, L, min(F, P - EVENT_REPLAY_MAX_OFFSETS))`, the architecture's rule.
2. `prd.md` NFR-19: it named `krafka` (and maybe `tokio`) as the only protected change of the feature. The architecture also adds `serde_json` to the application allow-list and `tokio-tungstenite` to the application dev list, in the same approved story. NFR-19 now lists all four entries.
3. `product-brief.md`, section "Needs the maintainer's approval": the same incomplete allow-list was replaced by the four entries.
4. `product-brief.md`: the sentence "The feature epics do not wait for these approvals" contradicted `epics.md`, where stories 3.2 to 4.4 follow the allow-list story 3.1. It now says that the feature does not wait for the removal approvals, and which feature stories come before and after its own allow-list approval.

## 8. Open findings

None is blocking.

| Id | Severity | Finding | Why it does not block |
| --- | --- | --- | --- |
| R1 | medium | D16 asks that every story that does not need a protected change can run before the approval. Stories 3.2 to 4.4 change no protected file but wait for story 3.1, because the envelope parser needs `serde_json` in the application layer. Most of the feature therefore waits for the maintainer. | Recorded as an assumption in `epics.md`. The alternative, a hand-written JSON parser, would be a larger risk than the wait. Waves 1 to 5 (scaffold, catalog, configuration, broker, domain, window, hub, status) still run before the approval. |
| R2 | medium | `cargo deny` and `cargo tree` were never run with `krafka` (`default-features = false`, `rustls-aws-lc-rs`). Whether `aws-lc-rs` stays in the lock after `sqlx` leaves was not checked. | Story 3.4 runs `make.check_security` and escalates a refusal instead of editing `deny.toml`. |
| R3 | low | FR-15 departs from D7: a cursor ahead of the topic end waits instead of being clamped or refused. | Recorded with its reason in `prd.md` and `architecture.md`. Clamping would break D8. |
| R4 | low | The architecture departs from the design brief in six places (variable names, one time-out for request and connect, retry pause, cursor ahead, no stale-readiness rule, replay stall rule). | Each departure follows the PRD and is recorded as an assumption in `architecture.md`. |
| R5 | low | The Kafka image digest was read from a local image. | Story 1.3 confirms it against the registry before it commits. |
| R6 | low | The fake broker of `krafka` was read from source only. It differs from a real broker (seek above the end, log start). | Used only for fault cases. Behaviour cases run against the real broker (FR-36). |
| R7 | low | Story 5.2 holds removal parts A and B, two commits, and is large. | `postgres-removal.md` shows the parts are green in `make ci` only together. |
| R8 | low | No capacity, latency or throughput figure exists. NFR-1 is a design target. | Stated as such in `prd.md`, `architecture.md` and `research.md`, as D18 requires. |
| R9 | low | The bundle had no multi-lens validation round. This check is a single reading under a usage limit. | The contract checks pass. Story 6.1 and the plugin's review stages review the change again. |
| R10 | low | The required status check "Persistence query metadata" must be removed from the branch rules of `main` by the maintainer. | A hand-off in story 5.2. It needs no repository change. |

Verdict: ready
