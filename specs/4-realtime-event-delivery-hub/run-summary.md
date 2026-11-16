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
| `implementation-readiness.md` | cross-check, four fixes, eight open findings (none blocking), the corrections of the semantics review, the review of 2026-11-14 (S1 to S16, I1 to I18, all resolved), `Verdict: ready` |
| `run-summary.md` | this file |

## How the bundle was written

- The design decisions (D1 to D18), the design brief of the reader and the three fact files (`repo-facts.md`, `kafka-facts.md`, `postgres-removal.md`) were produced on Fable, by the orchestrator and its fact-finding agents. The fact files tag each statement as verified or not; their open points are carried into `research.md`.
- The drafting of the seven files was done on Opus 5.5, one agent per part, under a usage limit that was nearly spent:
  - product brief and PRD;
  - architecture;
  - stories;
  - research, readiness check and this summary.
- A review round followed the drafting (see "Self-check rounds"): two lenses on Fable, semantics and implementability; the third lens, contract and traceability, was not run because the Fable usage limit was nearly reached, and the contract checks were run by script instead.
- No build, test or git command that changes anything was run. Nothing outside the bundle was changed.
- No latency, throughput or capacity figure exists for this design. The capacity figure of the first plan is a design target (D18, NFR-1).

## Self-check rounds

- Readiness cross-check: one round. It made four fixes:
  - `prd.md` FR-14: the replay floor formula;
  - `prd.md` NFR-19: the full allow-list;
  - `product-brief.md`: the allow-list entries;
  - `product-brief.md`: which stories wait for the approval.
- Contract checks: one round of 3 allowed. The commands of the `## Verification` section of the planning skill printed nothing. The staging dry run exited 0: six WOULD lines and its own info line, which says nothing would be written. The directory holds exactly the seven files.
- Semantics review: one round, sixteen findings (S1 to S16). All were applied to the three main files so that they follow the binding design brief (cursor clamp, no skip of an undecodable batch, readiness by `EVENT_TAIL_STALE_AFTER`, fixed `KAFKA_RETRY_DELAY`, replay cap and stall count, the variables of brief section 9, the first plan's order of the 503 answers, the hub's close and position rules, the adapter's long-poll wait, the same-round re-seek, the log entries and counters, the test gaps). `implementation-readiness.md` section 9 lists them.
- Implementability review: one round, eighteen findings (I1 to I18: four blockers, nine major, five minor), applied after the semantics findings and keeping them. The main changes: the protected changes of the feature and the shortened `required_directories` became one maintainer pull request (story 3.1); the GraphQL catalog moved to story 2.4; the end-to-end world waits for readiness (story 3.8); `make ci` of the removal is judged after the documentation (story 5.2); `Adapters` holds trait objects; the domain manifest and `workspace.dsl` edges follow first use; `uuid::Uuid` with a 36-character check; the `event_type` fixture survives the removal; the frame wrapper is written by hand; the end-of-log behaviour of `StaticTopic` and a pacing adapter; a K6 fallback for the SSE check; `/opt/kafka/bin` paths. I6 was resolved differently, because S12 had already deleted the middleware it was about. `implementation-readiness.md` section 10 lists all 34 findings.
- Contract checks after the review round: `verify-bundle.sh` printed nothing; the staging dry run exited 0 with six WOULD lines and its info line; a script confirmed the story structure and that every Depends on line names only earlier stories.
- Alignment with the code: the bundle was aligned with the implemented stories 1.1 to 2.4 on 2026-11-15 (the deviations their implementers and verifiers recorded, written into `architecture.md` and `epics.md`).
- Alignment with story 5.1 as built (2026-11-15): the two story 5.1 tables of "Protected Changes" in `architecture.md` and the scope list of story 5.1 in `epics.md` describe the two protected sub-patches as built, with the sceptic's corrected rows; the `.gitkeep` placeholders are recorded in the removal section of `architecture.md` and under "Open items".

## Changed story list

Old id to new id: 1.1 to 2.4 unchanged; new 3.1 (preparatory pull request, maintainer; it absorbs old 5.1 and the protected part of old 3.1); old 3.1 to 3.2 (envelope parser only); 3.2 to 3.3; 3.3 to 3.4; 3.4 to 3.5; 3.5 to 3.6; 3.6 to 3.7; 3.7 to 3.8; 4.1 to 4.4 unchanged; old 5.2 to 5.1; old 5.3 to 5.2; 6.1 unchanged.

| Id | Title | Depends on | Implemented by |
| --- | --- | --- | --- |
| 1.1 | Scaffold notification/event-stream | none | rust-implementer |
| 1.2 | Configuration variables and the browser-origin layers | 1.1 | rust-implementer |
| 1.3 | Kafka broker in the compose environments and CI | none | rust-implementer |
| 2.1 | Domain value objects, the topic ports and their fakes | 1.1 | rust-implementer |
| 2.2 | Memory window, tail state and node status | 2.1 | rust-implementer |
| 2.3 | Hub with bounded queues, falling behind, the subscriber cap and the frames | 2.2 | rust-implementer |
| 2.4 | Node status route and the GraphQL queries eventTypes and eventStreamStatus | 2.3 | rust-implementer |
| 3.1 | Preparatory pull request: allow-list entries and required directories | none | maintainer |
| 3.2 | Event envelope parser | 2.1, 3.1 | rust-implementer |
| 3.3 | Tail use case TailEvents | 2.2, 3.2 | rust-implementer |
| 3.4 | Subscription state machine with resume from the window and replay from Kafka | 2.2, 2.3, 3.2 | rust-implementer |
| 3.5 | Kafka adapter behind the topic ports | 1.2, 1.3, 2.1, 3.1 | rust-implementer |
| 3.6 | SSE route | 2.4, 3.4, 3.5 | rust-implementer |
| 3.7 | WebSocket session, pump and route | 3.4, 3.6 | rust-implementer |
| 3.8 | Supervision, wiring of the real adapter, readiness probe and graceful shutdown | 3.3, 3.5, 3.6, 3.7 | rust-implementer |
| 4.1 | Two-node and served-binary tests on one broker | 3.8 | rust-implementer |
| 4.2 | End-to-end features of the event stream | 3.8 | rust-implementer |
| 4.3 | K6 subscriptions scenario with its publisher | 1.3, 3.8 | rust-implementer |
| 4.4 | Protocol document, operations notes and decision records | 3.7, 3.8 | rust-implementer |
| 5.1 | Remove the example context, sqlx, the database stack and its gates | 3.1, 4.1, 4.2, 4.3 | maintainer (approved operator session) |
| 5.2 | Documentation and skills after the removal | 4.4, 5.1 | maintainer (approved operator session) |
| 6.1 | Full verification of the change | 5.2 | maintainer (approved operator session) |

## Protected changes that wait for the maintainer

The rust-implementer agent never edits a protected path or the profile, and the plugin refuses a commit that changes one against the merge base. The exact lines are in the "Protected Changes" section of `architecture.md`.

1. **Story 3.1, the preparatory pull request against `main`** (label `approved-config-change`; edited by the maintainer or a session with `ALLOW_PROTECTED_CONFIG_EDIT=1`; merged by the maintainer; requested in wave 1). Exact diff against `main` (`ecc5d17` line numbers):

   ```diff
   --- a/architecture.toml  (application layer)
        "humantime",
        "serde",
   +    "serde_json",
        "thiserror",
   -dev_external = ["http-body-util", "proptest", "rstest", "serde_json", "tokio", "tower"]
   +dev_external = ["http-body-util", "proptest", "rstest", "serde_json", "tokio", "tokio-tungstenite", "tower"]
   --- a/architecture.toml  (infrastructure layer)
        "aws-sdk-sqs",
   +    "krafka",
        "moka",
        "sqlx",
        "thiserror",
   +    "tokio",
        "tracing",
   --- a/protected-config.toml  (required_directories)
        ".github/openapi-spec",
   -    ".sqlx",
        "crates/service",
        "features",
   -    "migrations",
        "tests/CLI/bats",
   ```

   The feature branch then merges `main`, so its merge base holds the six changed lines (three added lines and one changed line in `architecture.toml`, two deleted lines in `protected-config.toml`); checks: `make.architecture`, `make.validate_configuration` with `CONFIG_CHANGE_APPROVED=true base_ref=main`.
2. **Story 5.1, part A (code)**, operator session in pull request #5, also `approved-breaking-change`; sub-patch part A (4 files), line numbers of `3cdb4f2`:
   - `scripts/check-docs.sh:19-20`, `:36`, `:81`, `:453`;
   - `tests/CLI/bats/check_docs.bats:558`;
   - the FR55(a) hunk of `tests/CLI/bats/make_general.bats:81-94` (pin `67eba235…a1e0`, title, comment and messages);
   - `tests/CLI/bats/make_quality_gates.bats:168-172` (G6 with `KAFKA_TOPIC=..`).
3. **Story 5.1, part B (stack and gates)**, same session and pull request; sub-patch part B (16 files, 3 of them deleted):
   - `protected-config.toml:32-33`;
   - delete `scripts/setup-test-db.sh` and `scripts/check-sqlx-metadata.sh`;
   - `make/gates.mk:14`;
   - `scripts/with-load-test-stack.sh:52`, `:7`;
   - delete `tests/CLI/bats/make_database.bats`;
   - `make_tests.bats`, the other three hunks of `make_general.bats` (`:58-68`, `:143`, `:583`), `make_execution_mode.bats`, `make_lifecycle.bats`, `make_load_tests.bats:108-141`, `make_security.bats`;
   - `make/tool-versions.mk:16`;
   - `.cargo/config.toml:5-6`;
   - `architecture.toml:67` (`sqlx`);
   - `.claude/settings.json:31`, `:68`, `:93`;
   - the profile `.claude/rust-sdlc.yml`, regenerated as its own commit.

   `make_general.bats` has hunks in both parts, so the protected patch is applied as these two sub-patches, not split by path; part A and then part B give the single patch byte for byte.
4. **Story 5.2** changes no protected file but runs in the same operator session, because the plugin stages escalate once story 5.1 is on the branch; it carries the `make ci` criterion with `CONFIG_CHANGE_APPROVED=true BREAKING_CHANGE_APPROVED=true`.
5. **Outside the repository:** if "Persistence query metadata" is a required check of the branch rules of `main` (never verified), the maintainer removes it (story 5.1 hand-off).
6. **Secrets scan on the branch history:** the CI check "Secrets scan" fails on the history of the branch, because commit `924c56165aff59d1a3bbdcf4c381b4d5e156fe09` holds the RFC 6455 sample nonce as a `Sec-WebSocket-Key` in three test lines (`crates/service/src/http/origin/tests.rs:59`, `crates/service/src/http/router/tests.rs:771` and `:789`); commit `c83776e` replaced the value. The maintainer either accepts the three fingerprints in `.gitleaksignore` (protected, label `approved-config-change`; it can ride in the pull request of story 3.1) or allows a rewrite of the branch history. The diff of story 3.1 above does not change for this.

## Open items

1. **`.sqlx/` and `migrations/` stay as placeholders in pull request #5.** Story 3.1 was committed on the feature branch (`37acdfe`, `3cdb4f2`) and is not merged into `main`, and the configuration guard reads `required_directories` from `main`, which still lists `.sqlx` and `migrations`. So pull request #5 keeps both directories, each holding only a `.gitkeep`; every other file in them is deleted. A follow-up pull request deletes the two placeholders after #5 is merged; only then is the FR-31 clause "`migrations/` and `.sqlx/` do not exist" met. It belongs in the body of pull request #5 as well.

## Epic 5 decisions that change the plan

The orchestrator's decisions of 2026-11-15 for Epic 5 (the maintainer approved the whole plan; labels `approved-config-change` and `approved-breaking-change` on pull request #5). Those that change the plan:

1. **`API_VERSION` 2.0.0.** `crates/service/src/openapi.rs` sets `API_VERSION` to `2.0.0`, because its doc comment asks for a major bump on an approved breaking change, and the removal of the example operations is one. The plan named no version.
2. **`DEPS ?= localstack`.** The architecture assumption (architecture.md, assumption 16) is now a decision. The protected lines of part B (`make_general.bats:583`, `make_execution_mode.bats:136`) are written to that text, and the broker stays outside `DEPS`, started by `ensure-kafka`.
3. **The `.gitkeep` placeholders.** `.sqlx/` and `migrations/` are emptied down to a `.gitkeep` each instead of deleted (open item 1).
4. **The profile refresh as its own commit.** Pull request #5 gets four commits: part A (code, with sub-patch part A and the new `features/health_check.feature`), part B (stack and gates, with sub-patch part B), story 5.2 (documentation), then the refresh of `.claude/rust-sdlc.yml` on its own, after the documentation instead of inside story 5.1. The refresh may also change `architecture.bounded_contexts` (`company-subdomain/some-module` out, `notification/event-stream` in), which is accepted.

## Assumptions

Every `> Assumption:` line of the other six files, in file order.

### research.md (4)

1. the facts tagged [V] in the three fact files are taken as given and not re-verified here, because the budget of this run was nearly spent; the [U] items are carried as risks or as checks inside the stories (story 3.5 runs `cargo deny` through `make.check_security`; the operator confirms the image digest of story 1.3 against the registry).
2. the election and lock measurements are cited only as the reason those mechanisms were dropped; the unaudited lock results are never quoted as figures.
3. the Kafka probes are functional evidence (behaviour, error kinds, recovery after outages), and the single-run delays they recorded are not presented as performance of this design.
4. the open points of the fact files (failover timing of a multi-broker cluster, `connect` against an address that never answers, a leader change) stay open; the design bounds every broker call by `KAFKA_REQUEST_TIMEOUT` so that none of them can hang a node.

### product-brief.md (3)

1. the client receives the whole envelope as one JSON object in `data`, because it must see the envelope `id` to deduplicate.
2. the status route `GET /api/events/status` is mirrored in GraphQL as `eventStreamStatus`, which keeps the GraphQL snapshot valid for the protected contract test after the example is removed.
3. S16's budgets are those of `tests/Load/config.json.dist`; this brief sets no number.

### prd.md (29)

1. the catalog is capped at 1 000 distinct names. Every external input is bounded (NFR-14), and a catalog page is built from the whole list on every request.
2. the counters the design brief asks for (retention that overtakes the tail, 4.3; capped replays, 6.1) are the status fields `retentionLosses` and `replayCapped`. `retentionLosses` counts occurrences, not offsets; the number of offsets lost each time is the `distance` field of `events lost to retention`. Exposing them on the status route follows NFR-16, which makes the status route the place of every counter until a metrics endpoint exists.
3. the route is `GET /api/events/status`, next to the two streaming routes. Issue criterion 7 asks for a status route; the first plan's `GET /api/cluster` is gone with the leader.
4. the status is also a GraphQL query. From the story that adds `EventType` on, `EventType` would be the first object type of the sorted GraphQL snapshot with a single field (it sorts before `ExampleApiResource`), and the protected contract test removes the first field of the first type, which leaves an empty type the diff tool cannot load. `EventStreamStatus` sorts before `EventType` and has several single-line fields, so both types enter the snapshot in the same story, and the query is useful to a frontend on its own.
5. the parameter is the comma-separated `types`, the same on both transports, as the issue writes it.
6. `types` is optional on the WebSocket route because criterion 3 lets the client subscribe at run time; it stays required on the SSE route, which has no other way to name types.
7. a run-time subscribe starts at the position of the node and does not replay, also not when the connection later falls behind and catches up (FR-24). Replay belongs to the connection request, which is where the cursor is given.
8. the size limit of a command is a constant, 16 384 bytes: the largest valid command, 100 names of 128 characters, takes 13 135 bytes. The transport limit for a message and for a frame is twice the command limit, and the read buffer of a connection is 4 096 bytes. `HTTP_MAX_BODY_BYTES` does not apply to WebSocket messages.
9. both routes accept both carriers. A browser `EventSource` sends the header by itself on reconnect; a browser WebSocket cannot set headers, so it uses the query parameter.
10. the replay is capped by a number of offsets below the tail, `EVENT_REPLAY_MAX_EVENTS` (default 1 000 000, design brief section 9), and not by age. An age cap needs record timestamps, which producers set and which can be skewed, or a time index lookup the plan has not verified on the pinned client; a count of offsets is exact and costs nothing to compute. At 100 events per second the default reaches back about 2.8 hours; the topic's retention bounds it further.
11. the page of 500 records (and 1 MiB) is a constant. No deployment has to tune it; with `EVENT_REPLAY_PERMITS` it bounds what one reconnect storm can make a node read at a time (NFR-14).
12. a malformed cursor is a 400 because it cannot be parsed, as `page=abc` is; the type list parses as text, so its faults are constraint violations and return 422.
13. a topic whose partition count grows while the node runs is not noticed until the node restarts. The pinned client does not report it to an assigned reader; a periodic re-check is a follow-up.
14. the event data is any JSON value and the client receives the whole envelope as one JSON object, instead of the first plan's opaque text in `data`. Design decision D4 makes the record value a JSON envelope, and decision D8 makes the client deduplicate by the envelope `id`, so the client must receive that `id`; the SSE frame has no field for it besides `data`. One object with the same shape on both transports is the smallest form that carries it.
15. `EVENT_MAX_DATA_BYTES` limits the whole record value, checked before the value is parsed. The name is kept from the first plan; the parse cost and the window then have a hard bound per record. Members other than the four are dropped so that a producer cannot enlarge what every client receives.
16. `occurredAt` accepts UTC only because the application layer may use `humantime` and no other time parser (protected allow-list), and `humantime::parse_rfc3339` accepts `Z` and `+00:00` only.
17. a skipped record is dropped after the log entry; dead-letter handling is a concern of the producers and of a follow-up.
18. no gap signal is sent when a cursor is below the replay floor. A gap signal is a follow-up; the client reloads its state through the owning service when it suspects a loss.
19. a full queue does not disconnect the subscriber. Issue criterion 11 asks that a client that lags past the buffer catches up on its own connection, which this requirement does; only a client that takes no data at all is disconnected.
20. a dead loop ends the node; it is not restarted in place. The lint policy leaves no panicking path in the service's code, so a panic comes from a dependency in a state nobody has analysed.
21. the close codes are 1001 (going away) for shutdown and 1011 (internal error) for a replay that cannot read Kafka, from RFC 6455. The 2 s wait is a constant: an upgraded WebSocket is outside the HTTP drain, and no deployment has to tune the wait.
22. when the variable is unset the CORS layer is absent, not installed with an empty list, so no existing status code changes (NFR-21). With the variable unset a WebSocket handshake that carries `Origin` is refused, also from the service's own origin, because behind a proxy the service does not know its public origin.
23. `KAFKA_BOOTSTRAP_SERVERS` and `KAFKA_TOPIC` have defaults (`localhost:9092`, `notifications`) instead of being required as the design brief (section 9) has them. A protected test runs the production image with no Kafka variable and waits for the container to be healthy (liveness `GET /ping`); a required variable would break it. A node pointed at an address without a broker stays `starting`, which its status route, its readiness probe and its log show. The four security variables are an addition the brief allows (its open point 9).
24. Kafka security covers plaintext, TLS with the public roots, SASL PLAIN and SCRAM. The pinned client compiles TLS (rustls) and these SASL mechanisms with its own dependencies (verified in its source, not probed against a broker); the architecture confirms each against the dependency policy and drops a value it cannot support, recording that in its decision record. OAUTHBEARER, AWS MSK IAM, a custom root and client certificates are follow-ups (decision D17).
25. the upper bound of `KAFKA_CONNECT_TIMEOUT` ("to the request time-out", design brief section 9) is a cross-field rule checked after the per-variable rules, with the reason `must not be longer than KAFKA_REQUEST_TIMEOUT`; the lower bound `100ms` is the per-variable rule `must be a duration from 100ms to 30s`. The client refuses a connect time-out above the request time-out (probe 07).
26. how the host reaches the broker (a host port and the variable for it, and any protected Bats edit that needs) is decided by the architecture, which lists it among the protected changes if it needs one.
27. the publisher is part of the load-test stack and not of K6, because the pinned stock K6 image has no Kafka client and a custom image is not reproducible. The architecture chooses its form (design decision D14).
28. the smoke budgets start at the documented smoke ceiling of 500 ms for the catalog and 2 000 ms for delivery; they are budgets, not measurements, and are tightened by the rule of `docs/load-testing.md` after the first hosted-runner run. The scenario name sorts after `health`, which a protected test needs.
29. the two streaming routes stay out of the OpenAPI document because OpenAPI cannot describe WebSocket and a never-ending response would hang the Schemathesis gate.

### architecture.md (20)

1. `krafka` is built with `default-features = false` and the feature `rustls-aws-lc-rs`. aws-lc-rs is already the crypto provider of the workspace (sqlx `tls-rustls-aws-lc-rs` today, the AWS SDK), so the change adds no second provider choice, and the crate states that aws-lc-rs wins when both rustls providers are enabled. `make.check_security` (`cargo deny`) was never run on `krafka`; the story that adds it runs it, and a refused licence or advisory is escalated, never answered by an edit of `deny.toml`.
2. `StoredEvent` carries the encoded event object as text, not the parsed members. The domain may not use `serde_json`, the window and the replay pages hold events for many connections, and every transport writes exactly this object; parsing once and keeping the text makes the encoding cost of NFR-2 one serialisation per event per node.
3. `serde_json` becomes a normal dependency of the application crate (protected allow-list entry, "Protected Changes"). The envelope is JSON (D4), the client must receive its `id` (D8, FR-18), and the domain and infrastructure layers may not use `serde_json` either; the application layer is where record values become events.
4. `data` is held as `serde_json::Value`, so member order inside it and the spelling of numbers may change (FR-18 allows both) and a number beyond the range of `i64`, `u64` and `f64` loses precision. The alternative, `RawValue`, needs a new feature of `serde_json` and keeps the producer's white space, which FR-18 removes.
5. `occurredAt` ending in `+00:00` is rewritten to `Z` before parsing and forwarded as written. This accepts exactly the two forms of FR-18 whether or not `humantime::parse_rfc3339` itself accepts `+00:00`, which this plan did not verify.
6. the counter of capped replays (`replayCapped`, design brief 6.1) lives in the hub (an `AtomicU64` that `Subscriber::note_replay_capped` raises), because every subscription already reaches the hub and `StatusSource` already reads it through `SubscriberCount`; a separate shared counter would push `Subscriptions::new` above five arguments.
7. the implementability review (I6) proposed a separate `streams<S>(hub)` router for an `availability` middleware with `State<Arc<Hub>>`. The semantics review (S12) deleted that middleware, so the two streaming routes stay in `router<S>()`.
8. `tokio` joins the infrastructure allow-list as a normal dependency (protected change), because the adapter must wrap every client call in its own time-out: `poll` is bounded by the request time-out and not by its argument (probe 09). The first plan kept tokio out of infrastructure; that rule cannot hold for this client.
9. a reader is not closed explicitly when it is dropped; the client then logs a warning, which the service's log filter hides (`krafka=error`, "Composition Root"). A `close` method on the ports would add a call that every fake and test must honour for no behaviour a client can see.
10. `.env` names the host listener `localhost:9095` and not the default `localhost:9092`. The test targets run on the host and export `.env` and `.env.test`; in container mode the app container's own `KAFKA_BOOTSTRAP_SERVERS=kafka:9092` from compose wins, because `scripts/with-env.sh` never changes a variable the caller already has.
11. an `observed_at` or `caught_up_at` later than `clock.now()` (a wall clock stepped back) counts as fresh: `SystemTime::duration_since` fails for it and the probe treats the failure as age 0. Design brief 3.7 does not cover a clock that steps back, and failing readiness on every node at once for it would take the service away without a fault.
12. the client's own log is limited to errors. A missing topic makes `krafka::metadata` write about 10 warnings per second, a dropped consumer writes a warning, and the spawned-binary tests compare the whole log of an idle node (`tests/integration/serve.rs:128-148`); the service's own entries (NFR-15) report every state an operator needs.
13. the start-up fault `Unreachable` is logged as `event topic unreachable` (warn, the error chain), next to the existing `event topic invalid`, because design brief 3.1 asks for the entry without naming it and the `event topic` prefix keeps the start-up entries together; the round entry of brief 4.5 is the existing `event tail failed`, which is also written on the transition only.
14. `TailState::started` is not affected by a broker outage (a started node keeps accepting subscriptions, FR-20); its readiness probe fails instead by the staleness rule. NFR-15 defines `event tail failed` as "the first failed round after a success".
15. the count of observations at which the wait of a cursor ahead began is kept in `Subscription::ahead_since`, beyond design brief 6.1, which takes `n` inside the step. The WebSocket live turn drops the step future when an inbound frame arrives first; without the stored count every command would restart the wait, and a chatty client could wait for ever.
16. `DEPS` becomes `localstack` and not `localstack kafka`, because the broker is started by `ensure-kafka` with its host listener ("Local Stack and Load Test"); the two protected lines that pin the dependency list change to that text.
17. the plugin skill `bounded-context-scaffolding` names "no protected file changed" as a success criterion of a scaffold; story 1.1 meets it (the scaffold changes no protected file), and the allow-list entries come in the preparatory pull request of story 3.1, as D16 requires.
18. the host reaches the broker through a fourth listener `EXTERNAL://localhost:9095`, published only by `docker-compose.kafka-host.yml`, which `make start` does not read. A host port in the files `make start` reads would collide between the development project and the private projects of the protected Bats suite, which get free ports only for `DB_PORT`, `LOCALSTACK_PORT`, `APP_PORT` and `STRUCTURIZR_PORT`; this keeps the feature free of Bats edits. The cost: `ensure-kafka` recreates a running `kafka` container of the development project when it lacks the port (compose recreates on a changed configuration); its topic data survives on the volume.
19. the stack creates the topic in the broker's health check (`--if-not-exists`, not probed), so no one-shot service is needed and every `up --wait` waits until the topic exists.
20. the publisher writes at a fixed 10 events per second set in the stack, not driven by the K6 profile. It is the smallest honest publisher the pinned K6 image allows (no Kafka module); a profile-driven rate is a follow-up.

### epics.md (24)

1. the envelope parser and the tail use case come after the preparatory pull request. `parse_event` (used by `TailEvents` and by the Kafka pages of `Subscription::step`) needs `serde_json` as a normal dependency of the application layer, which is an entry of that pull request. So the subscription state machine, the SSE route, the WebSocket session and route, supervision and shutdown, which all reach `parse_event` or `TailEvents`, follow story 3.2. Everything that does not reach it (scaffold, configuration, origins, broker, domain, window, status, hub, frames, status route) comes before it.
2. the frame encoder of story 2.3 writes the WebSocket `type` member and the `error` member as `"` + text + `"` without calling `serde_json`. An event type name only holds `a-z 0-9 . _ -` and every error reason of the protocol is a fixed ASCII text without `"`, `\` or a control character, so the output is byte-equal to `serde_json::to_string` (tested against it as a dev-dependency, which the application layer already allows). This keeps the hub and the frames free of the protected entry; `architecture.md`'s `http/frame.rs` signatures are unchanged.
3. the proof stories (Epic 4) come before the removal (Epic 5), not after it as the task text ordered. FR-34 makes the removal "the last epic", and the notification feature must not wait for the second label `approved-breaking-change` (decision D10). The full verification of the change (Epic 6) runs after the removal.
4. the Kafka broker in the compose environments and CI (story 1.3) needs no approval, because `architecture.md` keeps it out of every protected file (`DEPS` and the Bats files untouched, the host listener in a compose file `make start` does not read). It is therefore placed before the approval.
5. C6 is done with the infrastructure crate as the scaffolder writes it and no adapter. The context stores nothing, so it has no `persistence.rs` and no migration test; a scaffolder file that assumes a database table is removed in this story. The Kafka adapter is added by story 3.5, after the preparatory pull request of story 3.1.
6. the GraphQL part of the catalog (`EventTypeObject`, `EventStreamQuery`, `Query(ExampleApiResourceQuery, EventStreamQuery)`, the SDL pinning test, the GraphQL snapshot, the GraphQL request of `docs/requests.http` and the `eventTypes` criteria) is written by story 2.4, not here. `EventType` sorts before `ExampleApiResource`, so in this story it would be the first type of the snapshot with one field; the protected G20 tests remove the first field of the first type, which leaves an empty type. In story 2.4 `EventStreamStatus` (twelve single-line fields), which sorts before `EventType`, enters the snapshot in the same commit.
7. C7 adds the fixture only. The domain has no port in this story; the fakes of the topic ports are added with the ports in story 2.1.
8. `EVENT_TYPES` is parsed in this story, because the catalog slice needs it; the other variables of FR-29 come in story 1.2. `event-stream-infrastructure` gets its workspace entry in story 1.2, where `crates/service` first uses it.
9. `make.ci_delivery` of C12 runs with `BUILD_IMAGE=0` and `APP_IMAGE` set to a stand-in image built from the head of the story when the session cannot build the Dockerfile; CI builds and judges the real image. A stage that cannot run at all is recorded `not run (<reason>)`.
10. the Kafka setting types live in `kafka.rs` of the infrastructure crate from this story on, as plain data, so that `config.rs` can build `KafkaSettings` before the client crate is allowed; story 3.5 adds `KafkaTopic` and the adapters to the same file.
11. the digest of `apache/kafka:4.1.1` written in `architecture.md` was read from a local image; the operator confirms it against the registry, and the implementer uses the recorded digest (a registry lookup is a direct docker call, which the Constraints forbid).
12. the Kafka tools are called by their absolute path `/opt/kafka/bin/..` in the image, because the fact files ran them only that way; whether they are on `PATH` is not verified.
13. `tail.rs` is created here with its types and trait only; `TailEvents` and its constants are added by story 3.3, after the preparatory pull request of story 3.1, because the round calls `parse_event`.
14. until story 3.8 replaces it with `bootstrap::application`, the composition root builds the status from a hub that is never started and a tail state channel holding `TailState::default()`, so the route answers `starting` with reason `catching-up`. No streaming route exists yet, so nothing else depends on the hub.
15. the commit subject is `chore(#4): story 3.1 allow-list entries and required directories`, on a branch of its own cut from `main`, because the guard must see the changes in the base of the feature pull request. Unused allow-list entries are not reported by the architecture gate (`xtask/src/arch/rules.rs:226-229`), and a directory that leaves `required_directories` may still exist, so the pull request is green before any crate uses an entry and before the directories are deleted.
16. the log entry `replay capped` and the counter call `Subscriber::note_replay_capped` are written by this story; the counter itself is in the hub (story 2.3).
17. `cargo deny` was never run on `krafka`. This story runs `make.check_security`; a refusal by `deny.toml` is escalated to the maintainer and `deny.toml` is not edited.
18. the streaming routes are wired in the composition root in this story, with the `Adapters.topic_replay` field (filled from the `KafkaTopic` of story 3.5) as the source of `TopicReplay`. The hub of the router is not started until story 3.8, so a served request answers `the node is starting`; delivery is tested in the application crate with a started hub and `StaticTopic`.
19. the pacing adapter sleeps `KAFKA_FETCH_MAX_WAIT` (`KafkaSettings::fetch_wait`) after an empty batch, not `TailEvents::WAIT` as the implementability review wrote, because the semantics review deleted that constant (S10); a real fetch at the end of the log waits that long.
20. the broker outage of a served binary is produced by a small TCP forwarder inside the test between the node and the broker of the stack, because stopping the shared broker would break the tests that run in parallel.
21. repo-facts section 7 did not re-run whether the pinned K6 returns the body of a request ended by its own timeout; an SSE response never ends by itself, so the story probes it first and falls back to the subscriber count of the status route.
22. the decision record "PostgreSQL is removed" is written in story 5.2 with the removal, because it supersedes `docs/adr/0005` only once the removal is done; story 4.4 writes the other seven records.
23. parts A and B are one story with two commits, and parts A, B and the documentation of story 5.2 are green in `make ci` only together (stage 3 needs the targets, stage 7 the Bats files, stage 2 `docs-check` the updated documents, which cite the deleted targets, command, paths and variables until story 5.2); each commit passes the hooks on its own (part A keeps clippy, `cargo deny` unused and shear green in one commit).
24. if "Persistence query metadata" is a required check of the branch rules of `main` (never verified), the maintainer removes it; the story records it as a hand-off and does not wait on that check.

## Next step

1. Commit the bundle with `/rs-sdlc-plan` (step 8), or let `/rs-sdlc` continue.
2. Ask the maintainer now for the preparatory pull request of story 3.1 (exact diff above, label `approved-config-change`, merged into `main`).
3. Implement waves 1 to 5 (stories 1.1, 1.3, 1.2, 2.1, 2.2, 2.3, 2.4) with `/rs-sdlc-implement` or the feature workflow, on Opus 5.5.
4. Once story 3.1 is merged, merge `main` into the feature branch (a merge commit) and implement stories 3.2 to 4.4 with the rust-implementer agent.
5. Then ask for `approved-config-change` and `approved-breaking-change` on pull request #5 and run stories 5.1, 5.2 and 6.1 in an approved operator session (`ALLOW_PROTECTED_CONFIG_EDIT=1`); the plugin stages end ESCALATED by design from story 5.1 on, and the operator continues by hand.
