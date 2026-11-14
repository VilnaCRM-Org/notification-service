# Run summary: real-time event delivery hub (issue #4)

This file replaces the run summary of the first plan.

## Bundle

- Directory: `specs/4-realtime-event-delivery-hub/`, revised in place.
- Issue: #4 (VilnaCRM-Org/notification-service), "Real-time event delivery hub: catalog, SSE and WebSocket subscriptions, Kafka fan-out on every node", as updated on 2026-11-14, with 17 acceptance criteria.
- Branch: `claude/vilnacrm-notification-service-br7s47`. Pull request #5.
- Design: Kafka plus stateless Rust nodes. Every node reads the one-partition notifications topic without a consumer group and filters events for its own subscribers. Clients deduplicate by the envelope `id`. PostgreSQL and the example context are removed. The owner decided this on 2026-11-14 (decisions D1 to D18 of the design input).

## Artifacts

| File | Content |
| --- | --- |
| `research.md` | the repository today, reading Kafka with `krafka =0.32.0`, what the PostgreSQL removal touches, protected files, evidence and its limits, risks |
| `product-brief.md` | problem, users, goals, scope, success criteria S1 to S17 traced to the issue |
| `prd.md` | 38 FR, 21 NFR, traceability of the 17 criteria, product scope |
| `architecture.md` | Tech Stack, Constraints, the `notification/event-stream` context (domain, application, infrastructure), composition root, wire protocols, algorithms, failure modes, removal plan, protected changes, test strategy, decision records |
| `epics.md` | 6 epics, 22 stories, wave plan, requirement-to-story table |
| `implementation-readiness.md` | cross-check, four fixes, ten open findings (none blocking), `Verdict: ready` |
| `run-summary.md` | this file |

## How the bundle was written

- The design decisions (D1 to D18), the design brief of the reader and the three fact files (`repo-facts.md`, `kafka-facts.md`, `postgres-removal.md`) were produced on Fable, by the orchestrator and its fact-finding agents. The fact files tag each statement as verified or not; their open points are carried into `research.md`.
- The drafting of the seven files was done on Opus 5.5, one agent per part, under a usage limit that was nearly spent:
  - product brief and PRD;
  - architecture;
  - stories;
  - research, readiness check and this summary.
- No multi-lens validation round was run on this bundle. The only cross-check is the single reading in `implementation-readiness.md`, plus the scripted contract checks.
- No build, test or git command that changes anything was run. Nothing outside the bundle was changed.
- No latency, throughput or capacity figure exists for this design. The capacity figure of the first plan is a design target (D18, NFR-1).

## Self-check rounds

- Readiness cross-check: one round. It made four fixes:
  - `prd.md` FR-14: the replay floor formula;
  - `prd.md` NFR-19: the full allow-list;
  - `product-brief.md`: the allow-list entries;
  - `product-brief.md`: which stories wait for the approval.
- Contract checks: one round of 3 allowed. The commands of the `## Verification` section of the planning skill printed nothing. The staging dry run exited 0: six WOULD lines and its own info line, which says nothing would be written. The directory holds exactly the seven files.

## Protected changes that wait for the maintainer

Every change waits for the label `approved-config-change` on the pull request and a session restarted with `ALLOW_PROTECTED_CONFIG_EDIT=1`. The exact lines are in the "Protected Changes" section of `architecture.md`.

1. **Story 3.1 (feature)**, `architecture.toml`:
   - infrastructure `external` += `krafka`, `tokio`;
   - application `external` += `serde_json`;
   - application `dev_external` += `tokio-tungstenite`.
2. **Story 5.1 (preparatory pull request**, its own branch from `main`, merged first): `protected-config.toml`, remove `.sqlx` and `migrations` from `required_directories`.
3. **Story 5.2, part A (code)**, which also needs `approved-breaking-change`:
   - `scripts/check-docs.sh`;
   - `tests/CLI/bats/check_docs.bats:558`;
   - the hash pin in `tests/CLI/bats/make_general.bats:81-94`;
   - `tests/CLI/bats/make_quality_gates.bats:168-172`.
4. **Story 5.2, part B (stack and gates)**:
   - `protected-config.toml:32-33`;
   - delete `scripts/setup-test-db.sh` and `scripts/check-sqlx-metadata.sh`;
   - `make/gates.mk:14`;
   - `scripts/with-load-test-stack.sh:52`, `:6-7`;
   - delete `tests/CLI/bats/make_database.bats`;
   - `make_tests.bats`, `make_general.bats` (`:61-66`, `:143`, `:583`), `make_execution_mode.bats`, `make_lifecycle.bats`, `make_load_tests.bats:108-140`, `make_security.bats`;
   - `make/tool-versions.mk:16`;
   - `.cargo/config.toml:5-6`;
   - `architecture.toml:57` (`sqlx`);
   - `.claude/settings.json:31`, `:68`, `:93`.
5. **Outside the repository:** the maintainer removes the required status check "Persistence query metadata" from the branch rules of `main` (story 5.2 hand-off).

## Assumptions

Every `> Assumption:` line of the other six files, in file order.

### research.md (4)

1. the facts tagged [V] in the three fact files are taken as given and not re-verified here, because the budget of this run was nearly spent; the [U] items are carried as risks or as checks inside the stories (story 3.4 runs `cargo deny` through `make.check_security`, story 1.3 confirms the image digest against the registry).
2. the election and lock measurements are cited only as the reason those mechanisms were dropped; the unaudited lock results are never quoted as figures.
3. the Kafka probes are functional evidence (behaviour, error kinds, recovery after outages), and the single-run delays they recorded are not presented as performance of this design.
4. the open points of the fact files (failover timing of a multi-broker cluster, `connect` against an address that never answers, a leader change) stay open; the design bounds every broker call by `KAFKA_REQUEST_TIMEOUT` so that none of them can hang a node.

### product-brief.md (3)

1. the client receives the whole envelope as one JSON object in `data`, because it must see the envelope `id` to deduplicate.
2. the status route `GET /api/events/status` is mirrored in GraphQL as `eventStreamStatus`, which keeps the GraphQL snapshot valid for the protected contract test after the example is removed.
3. S16's budgets are those of `tests/Load/config.json.dist`; this brief sets no number.

### prd.md (32)

1. the catalog is capped at 1 000 distinct names. Every external input is bounded (NFR-14), and a catalog page is built from the whole list on every request.
2. the route is `GET /api/events/status`, next to the two streaming routes. Issue criterion 7 asks for a status route; the first plan's `GET /api/cluster` is gone with the leader.
3. the status is also a GraphQL query. When the example context is removed (FR-33), `EventType` would be the first object type of the sorted GraphQL snapshot with a single field, and the protected contract test removes the first field of the first type, which leaves an empty type the diff tool cannot load. `EventStreamStatus` sorts before `EventType` and has several single-line fields, and the query is useful to a frontend on its own.
4. the parameter is the comma-separated `types`, the same on both transports, as the issue writes it.
5. `types` is optional on the WebSocket route because criterion 3 lets the client subscribe at run time; it stays required on the SSE route, which has no other way to name types.
6. a run-time subscribe starts at the position of the node and does not replay, also not when the connection later falls behind and catches up (FR-24). Replay belongs to the connection request, which is where the cursor is given.
7. the size limit of a command is a constant, 16 384 bytes: the largest valid command, 100 names of 128 characters, takes 13 135 bytes. The transport limit for a message and for a frame is twice the command limit, and the read buffer of a connection is 4 096 bytes. `HTTP_MAX_BODY_BYTES` does not apply to WebSocket messages.
8. both routes accept both carriers. A browser `EventSource` sends the header by itself on reconnect; a browser WebSocket cannot set headers, so it uses the query parameter.
9. the replay is capped by a number of offsets below the tail, `EVENT_REPLAY_MAX_OFFSETS` (default 1 000 000), and not by age. An age cap needs record timestamps, which producers set and which can be skewed, or a time index lookup the plan has not verified on the pinned client; a count of offsets is exact and costs nothing to compute. At 100 events per second the default reaches back about 2.8 hours; the topic's retention bounds it further.
10. the 4 permits and the page of 500 records are constants. No deployment has to tune them, and they bound what one reconnect storm can make a node read at a time (NFR-14).
11. a cursor ahead of the topic end is not clamped and not refused, although design decision D7 offered those two forms. Lowering it to `E` would deliver ids at or below the cursor, which decision D8 forbids. Refusing it needs the true end of the topic at request time: the node's own `E` can be older than the cursor a faster node issued a moment ago, so a valid cursor would be refused, and a fresh broker read in the request path is excluded by FR-12. Waiting is exact for every cursor a node issued. A cursor from another topic (a recreated topic) receives nothing until the topic passes it; the protocol document tells operators and clients that cursors become invalid when the topic is recreated.
12. a malformed cursor is a 400 because it cannot be parsed, as `page=abc` is; the type list parses as text, so its faults are constraint violations and return 422.
13. the 403 and the 503 answers come before validation. A node that cannot serve says so to every request, and a refused origin learns nothing about the parameters.
14. a topic whose partition count grows while the node runs is not noticed until the node restarts. The pinned client does not report it to an assigned reader; a periodic re-check is a follow-up.
15. the event data is any JSON value and the client receives the whole envelope as one JSON object, instead of the first plan's opaque text in `data`. Design decision D4 makes the record value a JSON envelope, and decision D8 makes the client deduplicate by the envelope `id`, so the client must receive that `id`; the SSE frame has no field for it besides `data`. One object with the same shape on both transports is the smallest form that carries it.
16. `EVENT_MAX_DATA_BYTES` limits the whole record value, checked before the value is parsed. The name is kept from the first plan; the parse cost and the window then have a hard bound per record. Members other than the four are dropped so that a producer cannot enlarge what every client receives.
17. `occurredAt` accepts UTC only because the application layer may use `humantime` and no other time parser (protected allow-list), and `humantime::parse_rfc3339` accepts `Z` and `+00:00` only.
18. a skipped record is dropped after the log entry; dead-letter handling is a concern of the producers and of a follow-up.
19. a broker outage after the start does not fail the readiness probe and does not refuse new subscriptions. Every node is equally affected, so a failing probe on all of them would take the catalog and the window resumes away without moving a client anywhere better. The status route reports the outage (`brokerReachable`, `lag`).
20. no gap signal is sent when a cursor is below the replay floor. A gap signal is a follow-up; the client reloads its state through the owning service when it suspects a loss.
21. a full queue does not disconnect the subscriber. Issue criterion 11 asks that a client that lags past the buffer catches up on its own connection, which this requirement does; only a client that takes no data at all is disconnected.
22. the pause after a failed round grows from 1 s to 30 s. Every node retries on its own, so during a broker outage the number of connection attempts per node stays small; a recovered broker is seen within 30 s, and FR-21 brings the backlog in by a seek.
23. a dead loop ends the node; it is not restarted in place. The lint policy leaves no panicking path in the service's code, so a panic comes from a dependency in a state nobody has analysed.
24. the close codes are 1001 (going away) for shutdown and 1011 (internal error) for a replay that cannot read Kafka, from RFC 6455. The 2 s wait is a constant: an upgraded WebSocket is outside the HTTP drain, and no deployment has to tune the wait.
25. when the variable is unset the CORS layer is absent, not installed with an empty list, so no existing status code changes (NFR-21). With the variable unset a WebSocket handshake that carries `Origin` is refused, also from the service's own origin, because behind a proxy the service does not know its public origin.
26. `KAFKA_BOOTSTRAP_SERVERS` has a default instead of being required. A protected test runs the production image with no Kafka variable and waits for the container to be healthy (liveness `GET /ping`); a required variable would break it. A node pointed at an address without a broker stays `starting`, which its status route, its readiness probe and its log show.
27. Kafka security covers plaintext, TLS with the public roots, SASL PLAIN and SCRAM. The pinned client compiles TLS (rustls) and these SASL mechanisms with its own dependencies (verified in its source, not probed against a broker); the architecture confirms each against the dependency policy and drops a value it cannot support, recording that in its decision record. OAUTHBEARER, AWS MSK IAM, a custom root and client certificates are follow-ups (decision D17).
28. the connect time-out equals `KAFKA_REQUEST_TIMEOUT` (the client requires the request time-out to be at least the connect time-out), so one variable covers both.
29. how the host reaches the broker (a host port and the variable for it, and any protected Bats edit that needs) is decided by the architecture, which lists it among the protected changes if it needs one.
30. the publisher is part of the load-test stack and not of K6, because the pinned stock K6 image has no Kafka client and a custom image is not reproducible. The architecture chooses its form (design decision D14).
31. the smoke budgets start at the documented smoke ceiling of 500 ms for the catalog and 2 000 ms for delivery; they are budgets, not measurements, and are tightened by the rule of `docs/load-testing.md` after the first hosted-runner run. The scenario name sorts after `health`, which a protected test needs.
32. the two streaming routes stay out of the OpenAPI document because OpenAPI cannot describe WebSocket and a never-ending response would hang the Schemathesis gate.

### architecture.md (25)

1. `krafka` is built with `default-features = false` and the feature `rustls-aws-lc-rs`. aws-lc-rs is already the crypto provider of the workspace (sqlx `tls-rustls-aws-lc-rs` today, the AWS SDK), so the change adds no second provider choice, and the crate states that aws-lc-rs wins when both rustls providers are enabled. `make.check_security` (`cargo deny`) was never run on `krafka`; the story that adds it runs it, and a refused licence or advisory is escalated, never answered by an edit of `deny.toml`.
2. `StoredEvent` carries the encoded event object as text, not the parsed members. The domain may not use `serde_json`, the window and the replay pages hold events for many connections, and every transport writes exactly this object; parsing once and keeping the text makes the encoding cost of NFR-2 one serialisation per event per node.
3. the port has a third error, `Undecodable { at }`, beyond the two of the design brief. FR-19 requires the tail to skip an undecodable batch, log it with the reason `undecodable batch` and count it once; without a variant of its own the application cannot tell it from an outage, which must not skip anything.
4. `serde_json` becomes a normal dependency of the application crate (protected allow-list entry, "Protected Changes"). The envelope is JSON (D4), the client must receive its `id` (D8, FR-18), and the domain and infrastructure layers may not use `serde_json` either; the application layer is where record values become events.
5. `data` is held as `serde_json::Value`, so member order inside it and the spelling of numbers may change (FR-18 allows both) and a number beyond the range of `i64`, `u64` and `f64` loses precision. The alternative, `RawValue`, needs a new feature of `serde_json` and keeps the producer's white space, which FR-18 removes.
6. `occurredAt` ending in `+00:00` is rewritten to `Z` before parsing and forwarded as written. This accepts exactly the two forms of FR-18 whether or not `humantime::parse_rfc3339` itself accepts `+00:00`, which this plan did not verify.
7. the window is bounded by bytes only, as FR-13 states, with at least one event kept so that a single large event does not empty it. The cost is `StoredEvent::cost`: the json bytes, the type bytes and 128 bytes for the `Arc`, the deque slot and the struct. A count bound (the brief's `EVENT_WINDOW_MAX_EVENTS`) is not needed: 128 bytes per event already bound the count to `EVENT_WINDOW_BYTES / 128`.
8. the long-poll wait of the tail is a constant of 500 ms, the client's default `fetch_max_wait`. FR-29 makes it a constant; a shorter wait costs more requests, a longer one delays a new event by nothing (the poll returns early with records) but lengthens the round limit.
9. the three 503 answers are checked twice: by the `availability` middleware before validation (FR-16's order) and again by `register` under the lock, which alone decides the cap race-free. A request that passes the first check while the last slot is being taken gets the capacity 503 after its parameters were validated; nothing else changes order.
10. `tokio` joins the infrastructure allow-list as a normal dependency (protected change), because the adapter must wrap every client call in its own time-out: `poll` is bounded by the request time-out and not by its argument (probe 09). The first plan kept tokio out of infrastructure; that rule cannot hold for this client.
11. a reader is not closed explicitly when it is dropped; the client then logs a warning, which the service's log filter hides (`krafka=error`, "Composition Root"). A `close` method on the ports would add a call that every fake and test must honour for no behaviour a client can see.
12. `.env` names the host listener `localhost:9095` and not the default `localhost:9092`. The test targets run on the host and export `.env` and `.env.test`; in container mode the app container's own `KAFKA_BOOTSTRAP_SERVERS=kafka:9092` from compose wins, because `scripts/with-env.sh` never changes a variable the caller already has.
13. the client's own log is limited to errors. A missing topic makes `krafka::metadata` write about 10 warnings per second, a dropped consumer writes a warning, and the spawned-binary tests compare the whole log of an idle node (`tests/integration/serve.rs:128-148`); the service's own entries (NFR-15) report every state an operator needs.
14. the entry `event tail behind retention` is added to the table of NFR-15. Retention that overtakes the tail loses events for every client of the node, and the PRD's entries do not cover it; NFR-15 lists what must be written, not everything that may be.
15. `TailState::started` is not affected by a broker outage, and nothing is logged for failures before the first success. NFR-15 defines `event tail failed` as "the first failed or cut round after a success".
16. the tail skips an undecodable batch only when the second consecutive round fails at the same position. A single `Protocol` error may be transient (it was not probed), and skipping on it would lose events; FR-19 asks for the skip, the brief's open point preferred no skip, and two rounds is the smallest rule that keeps both.
17. the variables, constants and readiness rules of the PRD replace those of the design brief where they differ (`EVENT_WINDOW_BYTES`, `EVENT_WINDOW_PRELOAD`, `EVENT_REPLAY_MAX_OFFSETS`, a single `KAFKA_REQUEST_TIMEOUT` for request and connect, the retry pause of 1 s to 30 s, 4 permits as a constant, no `EVENT_TAIL_STALE_AFTER`, a fixed start goal, no replay stall count but a stall time of `KAFKA_REQUEST_TIMEOUT`). The task binds this document to the PRD's names, and the PRD was written after the brief with these choices recorded.
18. the replay cap never raises the floor above the window floor `F`: the rule is `max(c, L, min(F, p - MAX))`, which equals FR-14's `max(c, L, P - MAX)` whenever `p - MAX <= F`, and gives FR-14's stated behaviour for `MAX = 0` (a cursor below `F` starts at `F`). The cap bounds Kafka reads; events still in memory are always served.
19. a cursor ahead of the tail waits without a broker read and without the brief's two-observation wait: the floor is the cursor, the subscription is live at once and drops queued frames at or below it. This is FR-15's rule (neither clamped nor refused), which the PRD chose over the brief's clamp.
20. a replay that meets an undecodable batch skips one offset per page read without a log entry or count, as FR-19 says for invalid records met by a replay; the tail has logged and counted it when it passed.
21. `DEPS` becomes `localstack` and not `localstack kafka`, because the broker is started by `ensure-kafka` with its host listener ("Local Stack and Load Test"); the two protected lines that pin the dependency list change to that text.
22. the plugin skill `bounded-context-scaffolding` names "no protected file changed" as a success criterion of a scaffold; story 1.1 meets it (the scaffold changes no protected file), and the allow-list entries come in their own approved story, as D16 requires.
23. the host reaches the broker through a fourth listener `EXTERNAL://localhost:9095`, published only by `docker-compose.kafka-host.yml`, which `make start` does not read. A host port in the files `make start` reads would collide between the development project and the private projects of the protected Bats suite, which get free ports only for `DB_PORT`, `LOCALSTACK_PORT`, `APP_PORT` and `STRUCTURIZR_PORT`; this keeps the feature free of Bats edits. The cost: `ensure-kafka` recreates a running `kafka` container of the development project when it lacks the port (compose recreates on a changed configuration); its topic data survives on the volume.
24. the stack creates the topic in the broker's health check (`--if-not-exists`, not probed), so no one-shot service is needed and every `up --wait` waits until the topic exists.
25. the publisher writes at a fixed 10 events per second set in the stack, not driven by the K6 profile. It is the smallest honest publisher the pinned K6 image allows (no Kafka module); a profile-driven rate is a follow-up.

### epics.md (19)

1. the envelope parser and the tail use case come after the approval, not before it as the task text ordered. `architecture.md` makes `serde_json` a protected allow-list entry of the application layer ("Protected Changes"), and `parse_event` (used by `TailEvents` and by the Kafka pages of `Subscription::step`) cannot be written without it. So the subscription state machine, the SSE route, the WebSocket session and route, supervision and shutdown, which all reach `parse_event` or `TailEvents`, follow story 3.1. Everything that does not reach it (scaffold, configuration, origins, broker, domain, window, status, hub, frames, status route) stays before the approval.
2. the frame encoder of story 2.3 writes the WebSocket `type` member and the `error` member as `"` + text + `"` without calling `serde_json`. An event type name only holds `a-z 0-9 . _ -` and every error reason of the protocol is a fixed ASCII text without `"`, `\` or a control character, so the output is byte-equal to `serde_json::to_string` (tested against it as a dev-dependency, which the application layer already allows). This keeps the hub and the frames free of the protected entry; `architecture.md`'s `http/frame.rs` signatures are unchanged.
3. the proof stories (Epic 4) come before the removal (Epic 5), not after it as the task text ordered. FR-34 makes the removal "the last epic", and the notification feature must not wait for the second label `approved-breaking-change` (decision D10). The full verification of the change (Epic 6) runs after the removal.
4. the Kafka broker in the compose environments and CI (story 1.3) needs no approval, because `architecture.md` keeps it out of every protected file (`DEPS` and the Bats files untouched, the host listener in a compose file `make start` does not read). It is therefore placed before the approval.
5. C6 is done with the infrastructure crate as the scaffolder writes it and no adapter. The context stores nothing, so it has no `persistence.rs` and no migration test; a scaffolder file that assumes a database table is removed in this story. The Kafka adapter is added by story 3.4, after the approval.
6. C7 adds the fixture only. The domain has no port in this story; the fakes of the topic ports are added with the ports in story 2.1.
7. `EVENT_TYPES` is parsed in this story, because the catalog slice needs it; the other variables of FR-29 come in story 1.2. `event-stream-infrastructure` gets its workspace entry in story 1.2, where `crates/service` first uses it.
8. `make.ci_delivery` of C12 runs with `BUILD_IMAGE=0` and `APP_IMAGE` set to a stand-in image built from the head of the story when the session cannot build the Dockerfile; CI builds and judges the real image. A stage that cannot run at all is recorded `not run (<reason>)`.
9. the Kafka setting types live in `kafka.rs` of the infrastructure crate from this story on, as plain data, so that `config.rs` can build `KafkaSettings` before the client crate is allowed; story 3.4 adds `KafkaTopic`, `CLIENT_ID` and the adapters to the same file.
10. the digest of `apache/kafka:4.1.1` written in `architecture.md` was read from a local image; this story confirms it against the registry before it is committed and uses the registry digest if they differ.
11. `tail.rs` is created here with its types and trait only; `TailEvents` and its constants are added by story 3.2, after the approval, because the round calls `parse_event`.
12. until story 3.7 replaces it with `bootstrap::application`, the composition root builds the status from a hub that is never started and a tail state channel holding `TailState::default()`, so the route answers `starting` with reason `catching-up`. No streaming route exists yet, so nothing else depends on the hub.
13. `cargo deny` was never run on `krafka`. This story runs `make.check_security`; a refusal by `deny.toml` is escalated to the maintainer and `deny.toml` is not edited.
14. the streaming routes are wired in the composition root in this story, with the `Adapters.topic` field of story 3.4 as the source of `TopicReplay`. The hub of the router is not started until story 3.7, so a served request answers `the node is starting`; delivery is tested in the application crate with a started hub and `StaticTopic`.
15. the broker outage of a served binary is produced by a small TCP forwarder inside the test between the node and the broker of the stack, because stopping the shared broker would break the tests that run in parallel.
16. the decision record "PostgreSQL is removed" is written in story 5.3 with the removal, because it supersedes `docs/adr/0005` only once the removal is done; story 4.4 writes the other seven records.
17. this story is committed on a branch of its own cut from `main` with the subject `chore(#4): story 5.1 drop sqlx and migrations from required directories`, not on the feature branch, because the guard must see the shortened list on the base of the feature pull request; the feature branch picks it up by merging `main` before story 5.2.
18. parts A and B are one story with two commits because `postgres-removal.md` shows they are green in `make ci` only together (stage 3 needs the targets, stage 7 the Bats files); each commit passes the hooks on its own (part A keeps clippy, `cargo deny` unused and shear green in one commit).
19. the maintainer removes the required status check "Persistence query metadata" from the branch rules of `main`; the story records it as a hand-off and does not wait on that check.

## Next step

1. Commit the bundle with `/rs-sdlc-plan` (step 8), or let `/rs-sdlc` continue.
2. Implement waves 1 to 5 (stories 1.1, 1.3, 1.2, 2.1, 2.2, 2.3, 2.4) with `/rs-sdlc-implement` or the feature workflow, on Opus 5.5, before any approval.
3. In parallel, ask the maintainer for the label `approved-config-change` on pull request #5 for story 3.1. Later ask for the removal approvals: story 5.1 as its own pull request, then `approved-breaking-change` for story 5.2.
