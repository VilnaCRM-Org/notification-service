# Run summary: planning bundle of issue #4

This file records the planning run for issue #4. It is the seventh file of the bundle (the FR-16 file of the rust-backend-sdlc plugin) and was written after the readiness check, by an agent that wrote none of the other six files.

- Bundle directory: `specs/4-realtime-event-delivery-hub/`
- Issue: #4 "Real-time event delivery hub: catalog, SSE and WebSocket subscriptions, clustered fan-out", https://github.com/VilnaCRM-Org/notification-service/issues/4
- Branch: `claude/vilnacrm-notification-service-br7s47`, pull request #5
- Readiness: `Verdict: ready` (round 1; 0 blocker, 0 major, 3 minor findings, see "Warnings")
- Assumptions recorded: 172 (research 18, brief 11, PRD 34, architecture 46, epics 63)
- Next step: `/rs-sdlc-implement`

## Artifacts

| File | Content |
| --- | --- |
| `research.md` | State of the template-derived service, the surface of the change, a comparison with Mercure, technical findings, risks and 18 resolved questions |
| `product-brief.md` | Problem, the three internal user groups, goals, scope, 12 success criteria traced to the issue, key risks |
| `prd.md` | FR-1 to FR-29, NFR-1 to NFR-21, the traceability table of the issue criteria, scope with follow-ups, delivery guarantees |
| `architecture.md` | Tech Stack, Constraints (3 838 bytes), both bounded contexts by layer, the composition root, the data model with Q1 to Q13, wire protocols, the four algorithms, failure modes, test strategy, "Revisions after validation" |
| `epics.md` | 6 epics, 22 stories, 318 criteria, the rules for running stories in waves, the eleven waves, the requirement coverage table |
| `implementation-readiness.md` | Independent cross-check of the five files against the issue and the repository: 3 minor findings, 11 residual risks, `Verdict: ready` |
| `run-summary.md` | This record |

## How the bundle was produced

1. **Stages.** Five agents wrote the files in the order research, brief, PRD, architecture, epics. Each took the role of its stage from the agent and command files of the plugin and settled every open question as an `> Assumption:` line instead of a question to the user.
2. **Validation.** Seven lenses reviewed the first version: domain and application (D), infrastructure and SQL (S), composition root (V), quality bar (Q) and security and resilience (R) on `architecture.md`; epics quality (E) on `epics.md`; traceability (T) across the five files. Two skeptics then checked every major finding: one the fact, with a probe where reading did not settle it, and one the remedy, whose result is the agreed fix. Minor findings were not checked adversarially; the owner who resolved one verified it.
3. **Revision.** Three section owners revised `architecture.md` (A: event-stream domain and application, protocols, subscription and relay; B: infrastructure, the cluster context, data model, election and ingestion; C: composition root, environments, tests and decision records). An editor revised `prd.md` (FR-28, FR-29 and NFR-21 are new) and corrected `research.md` and `product-brief.md` in place. A consistency reader ran last and ran the SQL of the data model again on PostgreSQL 16.
4. **Reconcile.** Each lens was reconciled against the revised bundle: every verified and every minor finding was classified as resolved as agreed, resolved differently, open or rejected, and an open point was fixed with the agreed fix. The orchestrator then decided the three points that the owners had left open (see the notes under the table below).
5. **Second epics pass.** `epics.md` was rewritten for the revised design, the findings of lenses E and T and the wave schedule. Five stories changed their id (old to new: 3.5 to 3.4, 5.1 to 3.5, 3.4 to 3.6, 5.2 to 5.1, 5.3 to 5.2).
6. **Readiness.** An independent reader cross-checked the five files against each other, the issue and the repository. Round 1: ready, 0 blocker/major, 3 minor. The loop allows three rounds; one was used.
7. **Run summary.** This file.

The session restarted once during the run (2026-11-14). The revision and reconcile agents that had been cut off were started again and checked the partial edits before they changed anything.

## Findings of the validation

The counts are those of the verdict files. The outcome is the verdict in "Revisions after validation" of `architecture.md`; for lens E it is the resolution in `epics.md`. "As amended" is the fix as the remedy check changed it. "Differently" means that another change removes the defect and an assumption gives the reason.

| Lens | Major, verified | Major, refuted | Minor | Outcome |
| --- | --- | --- | --- | --- |
| D, domain and application | 8 | 0 | 4 | 8 applied; 3 applied with one part differently (D4, D6, D7); 1 differently (D1) |
| S, infrastructure and SQL | 3 | 0 | 7 | 7 applied; 3 differently (S1, S5, S10) |
| V, composition root | 5 | 0 | 6 | 9 applied; 2 as amended (V1, V5) |
| Q, quality bar | 5 | 1 | 3 | 5 applied; 2 as amended (Q1, Q2); 1 differently (Q7); Q4 refuted |
| R, security and resilience | 7 | 1 | 4 | 6 applied; 3 as amended (R1, R2, R7); 2 differently (R5, and the budget value of R6); R8 refuted |
| E, epics quality | 4 | 0 | 5 | 6 applied; 1 applied in part (the cluster variables stay in story 1.2); 2 differently (the wave of story 3.6, the wave schedule) |
| T, traceability | 6 | 0 | 5 | 9 applied; 2 differently (T1, T5) |
| Total | 38 | 2 | 34 | 74 findings; the section UNVERIFIED is empty in every verdict file |

- The two refuted findings changed nothing: Q4 (three mutants that tests can see after all) and R8 (the single delete of the prune had already been replaced by batches).
- Three resolutions depart from the agreed fix by a recorded decision: the 503 problem `the node is starting` in place of a stream that waits (D1, R4), replay windows with a start floor in place of the anchored statement (R5), and a replay page of 1 MiB in place of 64 KiB (R6). The orchestrator accepted the last two and decided for the transport bound `TCP_USER_TIMEOUT` (D4, D6, R2, T2).
- T5 and the first finding of lens E asked for a wave of its own for the story with the log queries (3.6). The wave list keeps story 4.1 beside it; readiness finding 1 names the two gaps that this leaves in the text.

## Self-check rounds

The plan stage allows five rounds of the commands of "Verification" in the planning skill of the plugin, together with the staging dry run. The counter is never reset.

| Round | Run by | Result |
| --- | --- | --- |
| 1/5 | the readiness stage, before this file existed | one line, `MISSING: run-summary.md`; every other check printed nothing; the dry run of `stage-planning-artifacts.sh` ended with status 0 and six `WOULD: stage` lines |
| 2/5 | the stage that wrote this file, with all seven files in place | passed: the commands printed nothing (`## Constraints` 3 838 bytes; 22 stories, 22 `Depends on:` lines, 22 criteria blocks); the dry run ended with status 0 and the same six `WOULD: stage` lines and wrote nothing; the directory holds the seven files only; this file passes the wording rule of the readiness file |

> Assumption: round 1 is counted although its only line named the file of the stage after it, because the plan stage counts every run of the commands. A repeat before the closing commit is round 3/5.

## Deviations from the plugin procedure

The orchestrator took these decisions. Each is listed with its reason.

- **Branch.** The plugin plans and implements on `feat/4-<slug>`, cut from the default branch. The work stays on the session branch `claude/vilnacrm-notification-service-br7s47`, the only branch the session may push. It is not the default branch, so the branch check of the plan stage holds.
- **Base of the branch guards.** The setup commit of the plugin (`a7995a2`, the profile `.claude/rust-sdlc.yml` and the governance block) is on this branch and not on `main`. The guards that compare the branch with a base (`protected_changes`) take that commit as their base; against `origin/main` they would report the profile itself as changed.
- **GitHub access.** The `gh` CLI has no valid credentials in the session. The issue was read, and its body was updated, through the GitHub connector. Plugin scripts that call `gh` cannot run in the session.
- **Stage agents.** Plugin agent types and commands load only after a restart of the session. Until then the stages were run by general agents that read the agent and command files of the plugin and followed them.
- **Implementation by waves.** `/rs-sdlc-implement` of the plugin runs the sequential Ralph loop. Here, after `bmalph implement` has staged the bundle, the stories are implemented by parallel implementer waves in the form of the plugin's own workflow `rs-sdlc-feature`: one implementer per story, a review, one serialized verify step per wave and one commit per story. The loop is not started, so every report carries the note "bmalph run: not used (workflow path)".
- **Single-writer rule.** The first version of `epics.md` gave every shared file one writer per round. The wave schedule relaxes this for added lines: two stories of a wave may each add their own lines to the same manifest, `lib.rs`, `http.rs`, test harness file, `.templatesyncignore`, `.env` or `docs/environment.md`, and never rewrite the same item; only the tree at the end of a wave has to build and pass the gates (rules (i) to (v) of `epics.md`). The reason is parallel width: eleven waves for 22 stories, with up to four stories in a wave.
- **Draft commits.** The plugin commits the bundle once, after a passing self-check round. The five files were committed as drafts while they were written and revised (six `docs(#4)` commits, `e1fdee6` to `563160e`); the closing commit adds the readiness file and this file under the subject of the plugin, `docs(#4): add planning bundle 4-realtime-event-delivery-hub`. This deviation was read in the git history by the stage that wrote this file.

## Warnings

- **Container image.** The image cannot be built from the repository Dockerfile inside the session, because a build container does not trust the proxy certificate of the session. The gates that need the image (`build-prod-image`, `make.image_scan`, `make.schemathesis_validate`, `make.smoke_load_tests`, and `make.ci_delivery` of checklist item C12) run locally against a stand-in image with `BUILD_IMAGE=0` and `APP_IMAGE` set, and against the real image in CI. The close-out of story 6.6 names the image of each result.
- **Subscribers are not authorized.** This issue has no subscriber authorization: any client that reaches the service can read the catalog and subscribe to any type (NFR-13). The service must carry only events that are safe for every client; the allow-list of origins (FR-28) binds browsers only. Authorization and limits per client are follow-up issues.
- **Three minor readiness findings are open.** (1) Wave 5: `make.sqlx_prepare` of story 3.6 also fails on a compile error in a file of story 4.1, and part (b) of "Constraints" lets parallel agents run only the DB-free targets; to fix in `epics.md` before wave 5. (2) The text does not say whether `Elector::resign` writes the entry `leadership changed`; to fix in `architecture.md`, story 1.2 and NFR-15 before wave 2. (3) The extractor-check test of C5 does not allow the `HeaderMap` and the WebSocket upgrade parameters of the handlers `stream` and `ws`; to fix before wave 7, best before wave 1.
- **Criterion numbers of the issue.** The issue body was updated to the validated criteria on 2026-11-14 and now lists thirteen criteria. `prd.md` ("Traceability") and the readiness file number the eleven criteria of the first text. The two added criteria, numbers 10 and 11, are FR-28 (browser origins) and FR-29 (a dead loop ends the node), which the table of the PRD lists as "no criterion of the issue"; the K6 and the gate criteria are now numbers 12 and 13; and the three departures from the wording that the PRD lists (`types`, the takeover bound, the lagging client) are now the wording of the issue. Every criterion of the current issue has an FR id; only the numbers and these two passages of the PRD are stale.
- **Residual risks.** `implementation-readiness.md` lists eleven. The three with the widest effect: NFR-1 (10 000 subscribers, p99 below 100 ms) is a design target that the change does not measure, and story 6.6 records it as not verified; that a client which reads nothing loses its connection after `STREAM_KEEP_ALIVE` is a QA observation of story 5.2 and not a test; stories 1.2 and 5.2 have 21 criteria each and may need more than one implementation round.

## Assumptions

Every `> Assumption:` line of the five files, in file order and shortened to one line. The full text, with its reason, is at the place named.

> Assumption: each assumption keeps a line of its own, which makes this file longer than a summary usually is. The plugin asks for every assumption, and a merged line would hide which decision belongs to which requirement or story.

### research.md (18, all in "Assumptions")

- The stream parameter is the comma-separated `types`, not `?type=<name>` of the issue text; it matches the WebSocket command field.
- The ingestion adapter does not use the SQS timeouts of the health probe; the architecture stage chooses between an overridden receive timeout and a second client.
- Timestamps cross the sqlx boundary as integers converted in SQL; `occurred_at` is `SystemTime` in the domain.
- `PgLeaseStore::release` keeps the row and sets `expires_at` to the past; the term starts at 1 and grows by 1 with every change of holder.
- Criterion 6 is read as a takeover within `CLUSTER_LEASE_TTL + CLUSTER_RENEW_INTERVAL` after a crash and within one renew interval after a graceful stop, each plus two lease calls; only a node with `EVENT_QUEUE_URL` campaigns.
- `3 * CLUSTER_RENEW_INTERVAL <= CLUSTER_LEASE_TTL` is a cross-field rule of `config.rs`; "at most half" is not enough.
- The leader gating of ingest and prune is wired in `crates/service` from the cluster status channel; the use cases know nothing about leadership.
- `EventSignal::changed` returns a `Result`; on an error the relay waits for its poll tick and tries again.
- Without the attribute `event-id` the SQS `MessageId` is the event id; an id that is not a UUID makes the message invalid, and none is generated.
- `event-time` is optional RFC 3339 text in UTC, parsed in the application layer; absent means `Clock::now()`, unparsable means an invalid message.
- A message with empty data, U+0000, data above `EVENT_MAX_DATA_BYTES` or an invalid type is logged at warn and acknowledged; a dead-letter queue is a deployment concern.
- The GraphQL query is `eventTypes(page: Int! = 1, itemsPerPage: Int! = 30): [EventType!]!`, the committed convention.
- `API_VERSION` becomes `1.1.0`, and the OpenAPI tags gain `EventStream` and `Cluster`.
- The K6 smoke scenario uses the stock image, publishes through a loopback LocalStack port of the load-test project and records delivery latency as a custom trend; the `health` budgets do not change.
- The streaming routes are plain `axum` routes outside the OpenAPI document and are specified in a protocol document under `docs/`.
- A cursor older than the retention window is served from the oldest retained event without an error, as Mercure does.
- The advisory-lock key is one documented constant of `event-stream-infrastructure`; the service takes advisory locks in that crate only.
- The example context `company-subdomain/some-module` stays untouched; its migration test is narrowed to its own table.

### product-brief.md (11, all in "Assumptions")

- The SSE parameter is the comma-separated `types`, not `?type=<name>`.
- Leader election is a PostgreSQL lease with a fencing term; Raft is rejected, because its crates are not on the protected allow-list and it would add a second consensus system.
- The first ingestion adapter is SQS on the local stack; the Kafka adapter is a follow-up, because its client is not on the allow-list.
- Event `data` is opaque UTF-8 text limited by `EVENT_MAX_DATA_BYTES` (default 65536); a schema is not validated until the schema repository exists.
- The service is an alternative to the Mercure hub, not an implementation of the Mercure protocol.
- The positioning relies on the public description of Mercure; the PRD adds finer claims only with a source.
- S12 (10 000 subscribers, p99 below 100 ms) is a design target and not a merge gate; the performance merge gate is the K6 smoke scenario.
- The latency budget of the K6 smoke scenario is the one defined under `tests/Load/`; the brief sets none.
- Make targets are named by profile key, formed from the target name.
- The three user groups are internal to VilnaCRM; external integrators wait for subscriber authorization.
- "Ingestion continues" in criterion 6 means that the new leader resumes reading the queue; unacknowledged messages are redelivered and deduplicated by event id.

### prd.md (34, by requirement group)

- Catalog: a catalog entry is never removed in this issue; removal needs the schema repository.
- Catalog: a client may subscribe to a well-formed type that is not in the catalog; the catalog is informative.
- SSE: the parameter is the comma-separated `types` on both transports, not `?type=<name>`.
- WebSocket: `types` is optional on the WebSocket route and required on the SSE route.
- WebSocket: a run-time subscribe starts at the position of the node and never replays, also not in a later catch-up.
- WebSocket: the command limit is the constant 16 384 bytes, not `HTTP_MAX_BODY_BYTES`; message and frame limit are twice that, and the read buffer is 4 096 bytes.
- Resume: both routes accept both cursor carriers, the header `Last-Event-ID` and the parameter `lastEventId`.
- Resume: a cursor older than the retention window starts at the oldest retained event, and the loss is not signalled.
- Validation: a malformed cursor is a 400; faults of the type list are constraint violations and return 422.
- Ingestion: an invalid message is dropped after its log entry; a dead-letter queue is a setting of the queue.
- Ingestion: a body holds at least one byte and no U+0000 (PostgreSQL `text`, and the `data:` line of SSE).
- Ingestion: `event-time` is accepted in UTC only (`Z` or `+00:00`), because `humantime` is the one allowed time parser.
- Cluster: the takeover bound of criterion 6 is the lease time plus one renew interval plus two lease calls, 13 s with the defaults and at most the lease time plus three renew intervals.
- Protection: a full queue does not disconnect the subscriber; it catches up from the log on its connection. This departs from the first wording of criterion 8 and keeps its purpose.
- Protection: every accepted connection gets the send bound `TCP_USER_TIMEOUT` with the value of `STREAM_KEEP_ALIVE`; its three limits are stated (HTTP/2 flow control, a stall shortly before a shutdown, a best-effort log entry).
- Protection: the close codes are 1001 for a shutdown and 1011 for a log that cannot be read during a replay; 1013 is not used.
- Protection: new connections are refused from the signal on, before the loops stop; the order of FR-24 governs the work that is already there.
- Protection: the wait for the WebSocket tasks at shutdown is a constant of 2 s, not a variable.
- Configuration: `NODE_ID`, `EVENT_TYPES` and `CORS_ALLOWED_ORIGINS` have no entry in `.env`.
- Configuration: the allow-list of origins is the one variable that the validation added; every other new bound is a constant.
- Configuration: the bounds of the variables are chosen in the PRD; the renew interval is at most a third of the lease time.
- Load test: the smoke budget starts at 500 ms and the delivery ceiling at 2000 ms; both are tightened after the first hosted-runner measurement, and neither is a protected value.
- Load test: the SSE subscriber of K6 is checked by the frames in its body; the delivery time is measured on the WebSocket subscribers.
- Load test: the scenario is named `subscriptions`, so that it runs after `health` and the protected Bats test keeps its outcome.
- Documentation: the two streaming routes stay out of the OpenAPI document (WebSocket cannot be described, and an endless response would hang the Schemathesis gate).
- Browser access: the service answers CORS itself, with an allow-list that allows nothing by default.
- Browser access: the same list guards the WebSocket handshake.
- Browser access: with the variable unset, a WebSocket handshake that carries `Origin` is refused, also from the service's own origin.
- Browser access: with the variable unset the CORS layer is absent, so the two existing `OPTIONS` status codes do not change.
- Browser access: a page can still make browsers open SSE requests without reading them; that limit per client stays with the edge proxy.
- Supervision: a dead loop ends the node; it is neither reported by a readiness probe nor restarted in place.
- Supervision: a hung round is cut and retried and does not end the node.
- Supervision: the limit of a round is derived from the configuration and is neither a variable nor a constant.
- Control plane: the three connections of the loops and the share of the replays are constants; measured, the renewals answered within 10 ms while 30 000 clients reconnected.

### architecture.md (46, by section)

- Domain crate: event data holds at least one byte and no U+0000; the rule is checked in the domain.
- Domain crate: `Cursor` is its own value object, because a client cursor may be 0 and a stored `Sequence` may not.
- Domain crate: the stored form is `StoredEvent`, not `Event`, which would fail `clippy::struct_field_names`.
- Domain crate: the size limit of event data is a rule of ingestion only; `EventData::stored` rebuilds a row without it.
- Domain crate: the log has two reads, `read_after` for the relay and `read_between` with `PageLimit` for a replay; `retained_range` gives both ends of the log.
- Domain crate: `PageLimit` has public fields and no validating constructor; both members are 1 or more.
- Application crate: the hub, the frames and the subscription live in the `http` module; the relay reaches the hub through the trait `EventPublisher`.
- Application crate: a subscriber whose queue is full falls behind and catches up from the log; the service disconnects only a client that accepts no data for `STREAM_KEEP_ALIVE`.
- Application crate: the frame is encoded under the registry lock, after the lookup found a subscriber.
- Application crate: the command limit is `Session::MAX_COMMAND_BYTES` (16 384 bytes), not `HTTP_MAX_BODY_BYTES`.
- Application crate: `Subscriptions::open` reads nothing from the database; the replay starts inside the stream, so the only answers from server state are the three 503 of `HubError`.
- Application crate: `IngestReport::duplicates` and the info entry `duplicate events skipped` are the only trace of a skipped duplicate.
- Infrastructure crate (event-stream): the prune bounds are 200 rows per statement and 5 000 statements per call, values of the adapter and not variables.
- Infrastructure crate (event-stream): `prune` keeps its port signature and loops inside the adapter.
- Infrastructure crate (event-stream): `APPEND_LOCK_KEY`, `NOTIFY_CHANNEL`, the lease name, the page sizes and the attribute names are constants; the append always sends the `NOTIFY`.
- Infrastructure crate (cluster): the takeover test uses a lease of zero, not of 1 ms (500 of 500 attempts against 17 of 500).
- Adapters: the control pool has three connections and is not a variable.
- Adapters: `round_timeout` is derived from the configuration (41 s with the defaults), so a batch that is still answered is never cut.
- Browser origins: the allow-list is a variable, and the WebSocket route follows the same list.
- Browser origins: with the variable unset, a WebSocket handshake with an `Origin` is refused, also from the service's own origin.
- Browser origins: a page can still make browsers open SSE requests without reading them; the list does not claim to prevent it.
- Configuration: the long-poll wait, the batch size, the prune period, the retry delay, the two pool sizes, the drain limit and the preflight age are constants; the variables grow from eleven to twelve.
- Configuration: `NODE_ID`, `EVENT_TYPES` and `CORS_ALLOWED_ORIGINS` have no line in `.env`; FR-25 is amended accordingly.
- Configuration: the default of `MAX_SUBSCRIBERS` stays 100 000; the production compose reference sets 5 000, because a protected test pins its memory limit at 256 MiB.
- Supervision and shutdown: a dead loop ends the node; it is not reported by a readiness probe and not restarted in place.
- Supervision and shutdown: a hung round is cut and retried and does not end the node.
- Supervision and shutdown: every accepted connection gets `TCP_USER_TIMEOUT` with the value of `STREAM_KEEP_ALIVE` (decision of the orchestrator after the validation); its three measured limits are stated where they apply.
- Supervision and shutdown: `bootstrap::application`, `Background` and `Running` are the public seam for tests; `bootstrap::router` keeps its signature.
- Supervision and shutdown: the listener is closed before the loops stop; the order of FR-24 governs the work that is already there.
- Supervision and shutdown: a panic of a loop task during the shutdown itself does not become exit status 1.
- Data Model and Queries: each table has its own migration pair, created by the story whose code reads the table.
- Wire Protocols: close code 1011 is used when the log cannot be read during a replay, 1013 is not used, and a valid command gets no acknowledgement frame.
- Wire Protocols: `event-time` is accepted in UTC only, in the forms `Z` and `+00:00` that `humantime::parse_rfc3339` reads.
- Subscription Algorithm: the replay keeps windows of 10 000 sequences with a start floor at the oldest stored event; the anchored statement of the validation is not taken, and the cost is stated.
- Subscription Algorithm: the byte budget of a replay page stays 1 MiB and is not lowered to 64 KiB, because it is also the catch-up rate; the memory cost is stated.
- Relay Algorithm: the start-up gap is closed by the 503 problem `the node is starting` until the relay has started, not by a stream that waits.
- Relay Algorithm: falling behind is logged by the relay once per page with a count; `subscriber evicted` remains for the write deadline of the WebSocket pump.
- Ingestion Algorithm: the visibility timeout is a constant of the adapter (30 s), not the setting of the queue and not a variable.
- Election Algorithm: a node without an event source does not observe the lease either; it reports `leader` `null` with `term` 0.
- Election Algorithm: the lease rule is tightened to a third, instead of a fixed-rate timer or a call timeout derived from `T - 2R`.
- Performance Design: `MAX_SUBSCRIBERS` is sized by memory (100 000 idle connections take about 0.6 GiB as WebSockets and about 2 GiB as SSE streams).
- Performance Design: the registry is one mutex, and sharding by type is a later optimisation; NFR-1 is a design target that this change does not measure.
- Local Stack and Load Test: the K6 SSE subscriber is checked by the frames in its body, not by status 200; delivery latency is measured on the WebSocket subscriber.
- Local Stack and Load Test: the delivery ceiling starts at 2000 ms and is tightened after the first hosted-runner measurement.
- Test Strategy: the Gherkin world may bind one loopback listener for the WebSocket scenarios, with an item-level `#[expect(clippy::disallowed_methods, reason = "..")]`.
- Decision Records: the transport bound gets no record of its own; record 0018 states it, and the list stays 0013 to 0020.

### epics.md (63, by story)

- 1.1: `InvalidEvent` starts with the variants `TypeLength` and `TypePattern`; story 3.1 adds the other four.
- 1.1: `API_VERSION` becomes `1.1.0` in this story; story 1.2 stays inside the same unreleased minor version.
- 1.1: the application manifest declares `tokio` only as a dev-dependency; later stories add what their production code first uses.
- 1.1: the adapter field of the story is `Adapters.catalog_reader` on the request pool; `event_catalog` on the control pool comes with story 5.2.
- 1.1: `InMemoryEventCatalog` carries call records and run-time controls beyond the table of fakes, so that later stories leave `tests/support` alone.
- 1.1: `make.ci_delivery` of C12 runs against a stand-in image when the session cannot build the Dockerfile; CI judges the real image.
- 1.2: the three cluster variables and the lease rule stay in this story and are not moved to story 2.1.
- 1.2: `bootstrap` builds the `Elector` and hands its status to `ClusterState`; the loops campaign only from story 5.2 on.
- 1.2: the story introduces the control pool (`pool_settings`, `CONTROL_POOL_CONNECTIONS`, `Pools`).
- 1.2: the helper `bounded_duration` and the struct for the new members of `Config` are written here; story 2.1 extends both.
- 1.2: the decision record 0013 is written here; the records 0014 to 0020 are written in story 6.5.
- 1.2: `InMemoryLeaseStore` carries call records, a gate and run-time controls; its mode is one enum.
- 1.2: the GraphQL schema does not change; `make.generate_graphql_spec` must leave the snapshot unchanged.
- 2.1: FR-28 has no story of its own; the allow-list is one more variable of `Config`, read by `assemble` in the same change.
- 2.1: two cases of the router test list move to stories 5.2 and 6.1, which have the routes they need.
- 2.1: `.env` gets no line for `EVENT_TYPES`, `CORS_ALLOWED_ORIGINS` or `EVENT_QUEUE_URL` here; story 2.2 adds the queue line.
- 2.1: the entries of `EVENT_TYPES` are not trimmed, and a repeated name is kept once.
- 2.1: the row of `STREAM_KEEP_ALIVE` in `docs/environment.md` is written here with its final text.
- 2.2: the dependency containers of a running session are re-created after this story, so that LocalStack runs the new ready hook.
- 3.1: the three fakes carry call records, a gate and run-time controls beyond the table of fakes.
- 3.1: `InMemoryEventLog::insert` and `remove_through` act from the test side without a recorded port call.
- 3.2: the manifest lines `tokio` and `tracing` have this story as their one writer; story 3.4 relies on the second.
- 3.2: `error_json` is written by story 4.4, its first caller.
- 3.2: the SSE half of a frame is asserted by rendering it through `axum::response::sse::Sse`; `futures-util` enters as a dev-dependency.
- 3.2: the recording `EventPublisher` of the relay tests is defined in the tests of the application crate.
- 3.4: the tests count the clock reads with a local double `CountingClock`; `FixedClock` is not changed.
- 3.4: the manifest line `tracing` comes from story 3.2; this story adds only `humantime`.
- 3.5: `thiserror` enters the infrastructure manifest here; story 3.6 uses the same line.
- 3.6: this is the third and last migration pair, with only the table `notification_events`; the four pinning tests are not touched.
- 3.6: the session runs `make.sqlx_prepare` after each adapter file and after every change of a query text; story 4.1 stays in the wave.
- 4.1: the criteria follow the hub test list and assert effects; none expects an eviction by the hub.
- 4.1: a workspace-wide target can fail to compile in the infrastructure crate while story 3.6 has no metadata; that is not a failure of this story.
- 4.2: every test uses a real `Hub` that its arrangement starts, real channels and `InMemoryEventLog` with an exact call budget, under paused time.
- 4.2: a replay starts at or above the oldest stored event (`retained_range`), with windows of 10 000 sequences and pages of 500 events or 1 048 576 bytes.
- 4.3: the story adds `Adapters.replay_log` and builds the hub and the subscriptions in `bootstrap`.
- 4.3: until story 5.2 the hub of the composition root is never started, so the route answers 503 through `bootstrap::router`.
- 4.4: the integration target of the application crate needs no database; the socket helpers of its hand-written client carry the item-level lint expectation.
- 4.4: the pong that answers a client ping is written by the library; the real-socket test proves it.
- 4.4: the scripted `Connection` of the pump tests is defined in the tests of the crate.
- 4.4: the entry `subscriber evicted` is asserted with the scripted connection only; on a real socket it is best effort.
- 5.1: the supervisor is two stories: the loops here, `Background` from the adapters and `serve` in story 5.2.
- 5.1: every double of these tests comes from `tests/support`, because `crates/service` has no `async-trait` dependency.
- 5.1: the outage criterion stays one test over several loops, because NFR-11 needs the recovery of the whole node shown once.
- 5.2: the tests of the pools and of the shutdown with open streams are the new modules `tests/integration/pools.rs` and `shutdown.rs`.
- 5.2: a served-binary test that needs a leader points `EVENT_QUEUE_URL` at the queue of the local stack and sends no message.
- 5.2: the transport bound (`TCP_USER_TIMEOUT` with the value of `STREAM_KEEP_ALIVE`) is part of this story, which owns `serve.rs`.
- 5.2: a test asserts only the value of the socket option; that a stalled SSE client is ended is a QA observation.
- 5.2: the helper has two forms chosen by `cfg(target_os = "linux")`; the other form has an empty body, which is not an exclusion.
- 5.2: a served binary of this story is asked for the SSE route only; the WebSocket half is asserted in story 6.4.
- 5.2: the cases "whose database returns" are proved in process (stories 5.1 and 6.1), not with a served binary.
- 6.1: the two module names are chosen in the story.
- 6.1: the subscribers are SSE subscribers read through `oneshot`; the cross-node case with a WebSocket subscriber is in story 6.4.
- 6.1: every `Application` is built by `bootstrap::application` without spawned loops; an expired lease is forced with a zero-lease renewal.
- 6.2: `tests/e2e/Cargo.toml` gets a context crate only if a step names an item of it.
- 6.2: the world binds one loopback listener with one item-level `#[expect(..)]`.
- 6.3: the profiles `average`, `stress` and `spike` are sized for a generator on the machine of the node; none holds 10 000 subscribers.
- 6.3: `make.smoke_load_tests` runs locally against a stand-in image and in CI against the real image.
- 6.4: each test creates its own queue in LocalStack and sends with `aws-sdk-sqs` as a dev-dependency of `crates/service`.
- 6.4: the takeover test uses a lease of 1 s and a renew interval of 200 ms and asserts the bound `T + 3R` plus a scheduling margin.
- 6.4: the module name is `streaming.rs`; a subscription request is repeated while the answer is the 503 problem `the node is starting`.
- 6.4: a stalled client is not part of any test of this story.
- 6.5: the rows of the twelve new variables were written by stories 1.2, 2.1 and 2.2; this story changes two existing rows.
- 6.6: the four targets that need the container image run locally against a stand-in image and in CI against the real one; the close-out names the image of each result.

## Next step

`/rs-sdlc-implement`, in the workflow form described under "Deviations from the plugin procedure":

1. Close the three minor readiness findings before the wave that each one names (finding 3 best before wave 1), and commit the bundle.
2. Stage the bundle with the staging protocol of the plugin: `stage-planning-artifacts.sh`, then `bmalph implement`, then the check that `.ralph/PROJECT_CONTEXT.md` holds both parts of "Constraints" (the literal `(#4)` and the ten `make.*` keys).
3. Run the eleven waves of `epics.md`: wave 1: 1.1; wave 2: 1.2; wave 3: 2.1, 2.2, 3.1; wave 4: 3.2, 3.3, 3.4, 3.5; wave 5: 3.6, 4.1; wave 6: 4.2, 5.1; wave 7: 4.3; wave 8: 4.4, 5.2; wave 9: 6.1, 6.2, 6.3, 6.5; wave 10: 6.4; wave 11: 6.6. Per wave: one implementer per story in one working tree, a review, one serialized verify step on the whole gate set, one commit per story as `<type>(#4): story N.M <title>` with the hooks active, then `make.mutation_tests_diff` over the commits of the wave.
4. After wave 11: `/rs-sdlc-review`, `/rs-sdlc-qa` and `/rs-sdlc-finish-pr` on pull request #5.

By instruction of the user (2026-11-14) the implementer agents run on the Opus model; planning, review, verification and orchestration stay on the session model.
