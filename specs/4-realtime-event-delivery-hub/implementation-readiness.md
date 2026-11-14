# Implementation readiness: real-time event delivery hub (issue #4)

Round 1 of the readiness stage. The reviewer wrote none of the five bundle files (`research.md`, `product-brief.md`, `prd.md`, `architecture.md`, `epics.md`). This file states what was checked and how, the open findings, the risks that stay with the implementers, and the verdict.

Rule of this stage for the verdict: it is "ready" when neither a blocker nor a major finding is open. Minor findings are listed with their fix and with the wave before which the fix must be in the bundle.

Verdict: ready

## What was checked and how

### Format contract of bmalph and of the plugin

Run from the repository root with `N=4` and `SPECS_DIR=specs/4-realtime-event-delivery-hub`, the commands of the section "Verification" of the planning skill of the plugin:

| Check | Result |
| --- | --- |
| `prd.md` has the level-2 headings Executive Summary, Functional Requirements, Non-Functional Requirements, Product Scope, in this order and nothing else at level 2 | passed (lines 5, 26, 275, 358) |
| `architecture.md` has `## Tech Stack` before `## Constraints`; the first names the Rust toolchain and a table of crates only | passed |
| `## Constraints` up to the next level-2 heading | 3 838 bytes (limit of this run 4 300, limit of the script 4 500); part (a) with the literal `(#4)` and the form `story N.M` comes first; part (b) holds the ten keys `make.format`, `make.lint_code`, `make.unit_tests`, `make.architecture`, `make.complexity`, `make.setup_test_db`, `make.integration_tests`, `make.tests_with_coverage`, `make.e2e_tests`, `make.sqlx_prepare`; the section holds neither a fenced block nor a table |
| `epics.md`: only `## Epic N: Title` and `### Story N.M: Title` at levels 2 and 3 | passed: 6 epics, 22 stories, 22 `**Acceptance Criteria:**` lines, 22 `**Depends on:**` lines, every criterion in the form Given, When, Then (a script of this review read all 318 criteria), ids unique |
| Dependency order | passed: every `Depends on:` id is lower than its story; the waves computed from the 22 lines are exactly the eleven waves of the introduction |
| Story-1 rule | passed: `[workspace].members` of `Cargo.toml` holds neither a path under `crates/notification/event-stream/` nor one under `crates/internal/cluster/`; story 1.1 scaffolds the first context and story 1.2 the second, each with the criteria C1 to C12 of the scaffolding checklist; story 1.1 has `Depends on: none` |
| Wording rule of this file | checked with the regular expression of the task after the file was written: clean |
| Staging dry run of the plugin (`stage-planning-artifacts.sh specs/4-realtime-event-delivery-hub --dry-run`) | passed after this file was written; the output is in the last section |

`run-summary.md` does not exist yet; its check belongs to the stage that writes it.

### Issue criteria, requirements and stories

The eleven acceptance criteria of issue #4 were read from the design input and followed through the traceability table of `prd.md` to the stories that demonstrate them:

| Criterion | FR ids | Stories that demonstrate it |
| --- | --- | --- |
| 1. REST and GraphQL catalog, both snapshots | FR-1, FR-2, FR-3 | 1.1 (handlers, snapshots, feature), 6.6 (contract gates) |
| 2. SSE stream delivers `id`, `event`, `data` of subscribed types only | FR-4, FR-5, FR-6, FR-15, FR-17 | 4.3 (route), 6.2 (feature), 6.4 (served binary, a message sent to a real queue) |
| 3. WebSocket with run-time subscribe and unsubscribe | FR-7 to FR-10 | 4.4 (session, pump, real socket), 6.2, 6.4 |
| 4. Resume with `Last-Event-ID` or `lastEventId` | FR-11, FR-12 | 4.2 (replay, then live), 4.3, 6.2 (both carriers) |
| 5. Delivery across nodes on one database | FR-18 | 6.1 (two applications in process), 6.4 (two served binaries) |
| 6. One leader, failover, `GET /api/cluster` | FR-19, FR-20, FR-21 | 1.2 (lease, elector, endpoint), 5.1 (loops), 6.1 (release and forced expiry), 6.4 (leader killed, takeover inside the bound, ingestion continues) |
| 7. Ingestion through a port, SQS first | FR-14 to FR-17 | 3.1 (port), 3.4 (use case; neither the domain nor the application crate names SQS), 3.5 (adapter), 2.2 (queue of the local stack) |
| 8. A slow client never blocks others or ingestion | FR-22, FR-23 | 4.1 (bounded queue, falling behind, cap), 4.2 (catch-up), 4.4 (write deadline), 5.2 (send bound of every connection), 6.1 (backlog larger than the queue) |
| 9. RFC 9457 problems for invalid input | FR-1, FR-13 | 1.1, 4.3, 4.4, 6.2 |
| 10. K6 smoke scenario | FR-26 | 6.3 |
| 11. Local gates, 100 % coverage, zero surviving mutants | FR-25 to FR-27, NFR-16 to NFR-20 | every wave (its verify step and its mutation run), 6.6 |

The three points where a requirement departs from the wording of a criterion (the parameter `types`, the takeover bound, the lagging client) are recorded as assumptions in `prd.md` and repeated in the close-out of story 6.6.

Coverage in both directions was computed by a script: every one of FR-1 to FR-29 and NFR-1 to NFR-21 stands in a `**Covers:**` line of at least one story; every story has a `**Covers:**` line; the table "Requirement coverage" at the end of `epics.md` equals the union of the 22 lines, row by row.

### Slices against the design

Twelve components were followed from their signature in `architecture.md` to the story that builds them, the tests named there and the gates of the last criterion of that story. Names, constants and test lists agree in each case:

| Component (section of `architecture.md`) | Story | What was compared |
| --- | --- | --- |
| `EventType`, `EventCatalog`, `PgEventCatalog`, Q8, Q9 | 1.1 | derives and `Ord`, both error codes, name order of Q8, the six names of the order test, `COLLATE "C"`, the four pinned migration tests |
| `LeaseStore`, `Elector`, `PgLeaseStore`, Q10 to Q12 | 1.2 | one store call per round bounded by `call_timeout`, `resign` without a store call for a follower, zero-lease takeover, `millis`, `Collector::lease_rule` with its four boundary values |
| `AllowedOrigins`, `origin::cors`, `websocket_origin`, `refused` | 2.1 | the three variants, the accepted and refused origins, nine layers and ten with an allow-list, `PREFLIGHT_MAX_AGE` 600 s |
| `RelayEvents`, `EventPublisher`, `ListenForEvents`, `Frame` | 3.2 | `PAGE_SIZE` 500, `MAX_PAGES_PER_TICK` 20, `started` once before the first page, the byte-equality oracle of the encoder |
| `IngestEvents`, `to_new_event` | 3.4 | order register, append, acknowledge; the five rejection reasons; `duplicates`; the clock read once per batch |
| `SqsEventSource` | 3.5 | batch 10, wait 5 s, `VISIBILITY_TIMEOUT` 30 s, chunks of 10 with ids `"0"` to `"9"`, the `failed` list |
| `PgEventLog`, `PruneLimits`, `PgEventSignal`, Q1 to Q7 and Q13 | 3.6 | `APPEND_LOCK_KEY` 4004000001, `NOTIFY_CHANNEL`, 200 rows and 5 000 statements, the fixed order of the closed-pool test |
| `Hub`, `Subscriber`, `HubError`, `Delivery` | 4.1 | refusal order `Closed`, `NotReady`, `AtCapacity`; the entry owns the queue; the slot until the drop; `drained` |
| `Subscriptions`, `Subscription` | 4.2 | `REPLAY_WINDOW` 10 000, `REPLAY_PAGE` 500 rows and 1 048 576 bytes, the start floor from `retained_range`, the permits |
| `Session`, `socket::serve`, handler `ws` | 4.4 | `MAX_COMMAND_BYTES` 16 384, `MAX_MESSAGE_BYTES` 32 768, `READ_BUFFER_BYTES` 4 096, `CLOSE_TIMEOUT` 1 s, close codes 1001 and 1011, the ten functions of the pump |
| `Background`, `Running`, `Schedule`, the six loops | 5.1 | `until_stopped` as the only stop test, what the stop flag cancels, `STREAM_DRAIN_TIMEOUT` 2 s, the order loops, release, close, drained |
| Pools, `application`, `serve`, `bound_stalled_peer` | 5.2 | 3 and 1 connections beside the request pool, `round_timeout` 41 s with the defaults (2 x 2 s + 6 x 5 s + 7 s, defaults read in `docs/environment.md`), `tap_io`, `socket2` 0.6.5 with the feature `all` |

Across the five files these values were compared and agree: routes and parameters (`/api/event-types`, `/api/events/stream`, `/api/events/ws`, `/api/cluster`, `types`, `lastEventId`, `Last-Event-ID`), the twelve variables with defaults and bounds, the nine new lines of `.env` (two in story 1.2, six in 2.1, one in 2.2), status codes (400, 403, 422, 426, 503 with its three details), close codes, table, column, constraint and index names, the thirteen queries and their three stories, the shutdown order of FR-24, and the election bounds (`3R` step-down, `T + R + 2d` and at most `T + 3R` after a crash, `R + 2d` and at most `3R` after a graceful stop; 13 s, 19 s, 3 s and 9 s with the defaults). No file names close code 1013 as used, an old story id, the rule `2R < T` or two migration pairs. The story ids that `architecture.md` cites (3.6, 4.1, 5.2, 6.5) are the final ones.

### Facts read in the repository and in the locked crates

- `architecture.toml` (protected): a dev-dependency may be any crate of `external` or `dev_external` of its layer (`xtask/src/arch/config.rs`), so the planned manifests pass: `futures-util` and `proptest` as dev-dependencies of the application crate, `aws-sdk-sqs` with `test-util` and `aws-smithy-mocks` in the infrastructure crate (the form `health-check-infrastructure` already has), `std`-only fakes in `tests/support`. No story adds a crate to a layer list.
- `shared_domain::Uuid::parse`, `PageRequest`, `Clock`, `UuidGenerator`, `BoxError` and `error_chain` exist as the design uses them; `Collector::cross_field_rules` already has five parameters.
- `axum 0.8.9`: `ListenerExt::tap_io` exists, the feature `ws` pulls `tokio-tungstenite 0.29.0`. `tower-http 0.7.1`: `cors = []`. `socket2 0.6.5` is in `Cargo.lock`; `set_tcp_user_timeout` and `tcp_user_timeout` need the feature `all`. `humantime 2.4.0`: `parse_rfc3339` accepts only the suffix `Z` or `+00:00`. `cargo-mutants 27.1.0` makes no mutant for a function with an empty block (`src/visit.rs`, `block_is_empty`).
- `make.sqlx_prepare` runs `cargo sqlx prepare --workspace -- --all-targets --all-features`; `make.integration_tests` and the coverage run take every test target named `integration`, so the new `tests/integration` target of the application crate is counted.
- The served-binary tests clear the environment of the child; the in-process tests that parse the command line read `.env`.
- `scripts/check-docs.sh`, rule 5, takes the variables of the compose files from `${NAME}` references, so `LOAD_TEST_LOCALSTACK_PORT` and `MAX_SUBSCRIBERS` need the rows that stories 2.1 and 2.2 write, and `AWS_DEFAULT_REGION` needs none.
- The protected Bats tests assert single members of the rendered production reference and the report of the `health` scenario; the planned compose edits and the scenario name `subscriptions` leave them as they are.
- The fifteen synchronised files that the stories change are each added to `.templatesyncignore` by the story that first changes them (compared with the list at the end of "Composition Root" and with the file as it is today).
- Every `make.*` key named in a story exists in `.claude/rust-sdlc.yml`; the targets without a key (`lint-repo`, `lint-load-tests`, `validate-manifest`, `check-requirements`, `doc-tests`, `bats`, `build-prod-image`, `start-deps`) exist in the `Makefile`. The list of story 6.6 has the 31 targets of `AGENTS.md`.

### Hard limits

Searched in all five files: `allow` attributes, mutation-skip and coverage-off attributes, exclusion options of the coverage and mutation commands, a bypass of the hooks, an edit of a protected item. Result: none is planned. `Cargo.toml` changes only outside `[workspace.lints]`; `tests/Load/config.json.dist` gains a new entry and leaves the four `health` budgets; `deny.toml`, `clippy.toml`, `architecture.toml`, `.cargo/mutants.toml` and the make files are not touched. The only lint expectations are item-level `#[expect(.., reason = "..")]` in forms the repository already has: on test helpers that open a socket (stories 4.4, 5.2, 6.2, 6.4) and the `unused_async` expectation of a handler that awaits nothing (story 1.2). Rule (g) of `make.validate_invariants` permits both (it refuses `allow` and an `expect` in a crate root).

### Findings of the validation

All 38 verified findings and the minor ones have a row in "Revisions after validation" of `architecture.md` or a resolution in `epics.md`. Eighteen were followed into the texts:

| Finding (lens) | Where the resolution is | Result |
| --- | --- | --- |
| A subscription before the first relay round loses events (domain and application, security, trace) | `HubError::NotReady`, "Relay Algorithm" step 1; stories 3.2, 4.1, 4.3, 6.1 | resolved; the three agreed fixes differed (wait inside the stream, or keep a gate), the bundle keeps the gate and gives the reason |
| A subscriber without types is closed (domain and application, quality) | hub rule "the entry owns the queue"; stories 4.1, 4.2, 4.4 | resolved |
| `max_message_size` alone does not bound a connection (domain and application, security) | limits of one WebSocket; story 4.4, header-only frame test | resolved |
| A stalled client is never disconnected (domain and application, security, trace) | the transport bound, the write deadline, the slot until the drop; stories 4.1, 4.3, 4.4, 5.2 | resolved, with the limit stated under "Residual risks" |
| The shutdown does not wait for the WebSocket tasks (domain and application, service, trace) | `Hub::drained`, `STREAM_DRAIN_TIMEOUT`; stories 4.1, 4.4, 5.1, 6.4 | resolved |
| A node without a queue can hold the lease (infrastructure, trace) | "Election Algorithm", who campaigns; stories 5.1, 5.2, 6.1, 6.4 | resolved |
| New migration pairs break four existing tests (infrastructure, service) | table in "Data Model and Queries"; story 1.1 | resolved |
| `next_buffered()` does not fold notifications (infrastructure) | `PgEventSignal::changed` is one `recv()`; stories 3.2, 3.6 | resolved |
| The oracle of the JSON encoder leaves mutants alive (quality) | byte equality, exhaustive test; story 3.2 | resolved |
| Loop tests can spin (quality) | `until_stopped`, fakes that pend, call budgets; stories 3.1, 5.1 | resolved |
| Error branches out of reach of the tests (quality) | pure `restore` functions, `fetch_one` for Q11; stories 1.2, 3.6 | resolved |
| Client reads share the pool with the loops (security) | three pools, `replay_permits`; stories 1.2, 4.2, 4.3, 5.2 | resolved |
| A replay page is not bounded in scanned rows (security) | windows of 10 000 sequences with the start floor; story 4.2 | resolved differently; the anchored statement of the agreed fix is not taken, the reason and the cost are recorded |
| No CORS decision (security) | FR-28, "Browser origins"; stories 2.1, 5.2, 6.1, 6.5 | resolved |
| The production reference inherits the local queue URL (service) | "Local Stack and Load Test"; story 2.2 | resolved |
| The test doubles lack controls (epics) | stories 1.1 (C7), 1.2 (C7), 3.1 and their assumptions | resolved, item by item of the agreed fix |
| The mutation gate does not run between story 1.2 and 6.6 (epics) | introduction of `epics.md`: the run after the commits of each wave, never vacuous | resolved |
| The story with the log queries needs a wave of its own (epics, trace) | rules (i) and (v), stories 3.6 and 4.1 | resolved differently; see finding 1 |

## Findings

| # | Artifact | Section | Severity | Problem | Fix |
| --- | --- | --- | --- | --- | --- |
| 1 | `epics.md`; `architecture.md` | "How the stories run", rule (v); story 3.6, second assumption; story 4.1, second assumption; "Constraints", part (b) | minor | Wave 5 runs story 3.6, which owns the database and runs `make.migrate` and `make.sqlx_prepare` in its own session, beside story 4.1. The agreed fix of three verified findings was a wave of its own for that story; the wave list keeps the pair by a recorded decision of the orchestrator. Two gaps are left in the text. (a) `make.sqlx_prepare` builds every target of the workspace, so it fails while a file of story 4.1 does not compile; the rules name only the opposite case (story 4.1 fails on the missing metadata), so each session can wait for the other. (b) Part (b) of "Constraints" lets parallel agents run only the DB-free targets, and story 3.6 is a parallel agent in this wave. | Before wave 5, one of two edits in `epics.md`. Either add the order edge `**Depends on:** 3.2, 3.6` to story 4.1 and renumber the wave list to twelve waves (3.6 alone), which is the agreed fix. Or keep eleven waves and add to rule (v) and to the second assumption of story 3.6: "`make.sqlx_prepare` builds every target of the workspace, so it also fails on a compile error in a file of story 4.1. Rule (i) covers that: the session of story 3.6 runs the target again and changes no file of story 4.1, and the session of story 4.1 leaves `http/hub.rs` and its tests compiling whenever it stops editing (the output of `make.lint_code` names the crate of each error). In this wave story 3.6 is the one session that runs DB-backed targets; story 4.1 runs only the DB-free ones, which is the serialized use of the database that part (b) of 'Constraints' asks for." |
| 2 | `architecture.md`; `epics.md`; `prd.md` | "Election Algorithm", `Elector::resign`; story 1.2, criterion on `resign`; story 5.2, criterion on SIGTERM of a leader; NFR-15, row `leadership changed` | minor | Whether `resign` writes `leadership changed` is not stated. NFR-15 says the entry is written when the role of the node changed, and `resign` changes it from leader to follower; step 4 of `campaign` is the only place that names the entry. Story 5.2 expects exactly one such entry in a run that ends with `resign`, so an implementer of story 1.2 who logs in `resign` makes a criterion of wave 8 fail in a file of wave 2. | Before wave 2: add to `Elector::resign` in "Election Algorithm" and to the contract of `Elector` "`resign` writes no `leadership changed` entry; only `campaign` writes it"; add "and no `leadership changed` entry is written" to the `resign` criterion of story 1.2; word the NFR-15 row "an election round changed the role or the leader of the node". |
| 3 | `architecture.md`; `epics.md` | "Bounded Context notification/event-stream", application crate, handlers; story 1.1 (C5), story 4.3, story 4.4 | minor | C5 gives `http/handler.rs` the extractor-check test of the template: every parameter of a `pub async fn` must have a type of a fixed list (`State<..>`, `ApiPath<`, `ValidatedQuery<`, `ValidatedJson<`). The handlers `stream` and `ws` of the design take `headers: HeaderMap` and `upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>`, which that list does not hold. Neither file says that the list of the test changes, so story 4.3 meets a failing guard test without a rule for it. | Before wave 7 (best before wave 1, where the test is written): add one sentence to the handler notes of `architecture.md` and to the scope of stories 4.3 and 4.4: "the allowed list of the extractor-check test of this crate is `State<EventStreamState>` and `ValidatedQuery<`; story 4.3 adds `HeaderMap` and story 4.4 adds `Result<WebSocketUpgrade, WebSocketUpgradeRejection>`. Both extractions cannot be rejected, so every rejection of the two handlers is still a problem; the cases that plant a violation stay." |

Resolution after round 1: the orchestrator applied the three fixes above to the bundle before the bundle commit. Finding 1: rule (v) of `epics.md` and the second assumption of story 3.6 now state that `make.sqlx_prepare` builds every target, what each session does, and that story 4.1 starts after the implement step of story 3.6. Finding 2: `architecture.md` ("Election Algorithm" and the contract of `Elector`), the `resign` criterion of story 1.2 and the NFR-15 row of `prd.md` state that only `campaign` writes `leadership changed`. Finding 3: the handler notes of `architecture.md` and the scope of stories 4.3 and 4.4 name the allowed list of the extractor-check test. The findings table therefore holds only closed findings at the commit.

No blocker and no major finding is open.

## Residual risks an implementer should know

1. **Wave 5.** Until finding 1 is closed, the sessions of stories 3.6 and 4.1 depend on each other's compile state. The verify step of the wave runs `make.sqlx_prepare` and the DB-backed targets on the complete tree in any case.
2. **A stalled client is not driven by an automated test.** The tests assert the socket option (`bound_stalled_peer`, story 5.2) and the write deadline of the pump with a scripted connection (story 4.4). That a client which reads nothing loses its SSE connection after `STREAM_KEEP_ALIVE` is a QA observation of story 5.2, because kernel timers decide it in real time. An SSE stream on an HTTP/2 connection that is stalled by its own flow control is out of scope and keeps its slot.
3. **NFR-1 is a design target.** The change tests its structural conditions (the hub test with 10 000 queues, no database read on the live path) and does not measure 10 000 subscribers. The FR/NFR gate will find structural evidence only; story 6.6 records the requirement as not verified.
4. **New lock entries.** The feature `ws` of `axum` brings `tokio-tungstenite 0.29.0`, `tungstenite 0.29.0` and what they need. `make.check_security` judges them for the first time in wave 4 (story 3.2). Their licences (MIT, and MIT or Apache-2.0, read in the registry) are on the allow-list; `deny.toml` is protected, so an advisory against one of them has only one remedy, the escalation to a maintainer.
5. **Large stories.** Stories 1.2 and 5.2 have 21 criteria each, and stories 1.1, 4.1, 4.2 and 4.4 have 17 or 18. Story 1.2 covers three crates, a migration, three variables, a decision record and C12. Plan more than one implementation round for them.
6. **Commits inside a wave.** Several stories of one wave add lines to the same manifest, `lib.rs`, `Cargo.lock`, `.env`, `docs/environment.md` or `.templatesyncignore`. Only the tree at the end of the wave has to build (rule i), so a single story commit may not build alone. Commit in ascending story id: story 3.4 uses the `tracing` line of story 3.2, and the `.env` line of story 2.2 needs the variable of story 2.1.
7. **`.env` gets `EVENT_QUEUE_URL` in wave 3.** From then on every in-process test that parses the command line (`tests/integration/bootstrap.rs`, `panic.rs`, the Gherkin world) builds its adapters with a queue. A test that needs a node without an event source sets `event_source` to `None` itself (node B of story 6.1); a test that publishes replaces it by `ScriptedEventSource`.
8. **Resolutions that differ from the agreed fix.** Three are recorded with their reason and their cost: the 503 `the node is starting` in place of a wait inside the stream; replay windows with a start floor in place of the anchored statement (a replay over a retention of 30 days at 100 events per second costs about 26 000 short reads for a type that does not occur); a replay page of 1 MiB in place of 64 KiB (a slow client with a cursor holds up to 1 MiB plus one event). `docs/operations.md` of story 6.5 carries the sizing rules.
9. **The form of `bound_stalled_peer` for other targets than Linux** has an empty body on purpose: the gates run on Linux, the compiler removes that form there, and `cargo-mutants 27.1.0` makes no mutant for an empty block. A later version of the tool that changes this would report a mutant in code that Linux never compiles.
10. **The image.** A session that cannot build the Dockerfile runs `make.image_scan`, `make.schemathesis_validate` and `make.smoke_load_tests` against a stand-in image with `BUILD_IMAGE=0` and `APP_IMAGE` set; CI judges the real image. The K6 budgets of the new scenario (500 ms and 2 000 ms) are starting values, and its delivery time is measured on the WebSocket subscribers, because the pinned K6 image has no SSE client.
11. **One test waits on a real timer.** The takeover test of two served binaries (story 6.4) waits about one second for a lease to expire and asserts its bound with a scheduling margin.

## Results of the last checks

| Check | Result |
| --- | --- |
| Wording rule of this file (the regular expression of the task) | clean |
| Self-check commands of the planning skill | one line of output, `MISSING: run-summary.md`, for the file that the next stage writes; every other check prints nothing |
| `stage-planning-artifacts.sh specs/4-realtime-event-delivery-hub --dry-run` | exit status 0 and six `WOULD: stage` lines (`prd.md`, `architecture.md`, `epics.md`, `implementation-readiness.md`, `product-brief.md`, and `research.md` as `technical-research.md`); it printed neither a `REFUSED:` line nor a warning, and it wrote nothing |
| Every `make.*` key named in the five bundle files | 43 distinct keys, each present in `.claude/rust-sdlc.yml` with a target |

The dry run was run with this file in place and before `run-summary.md` exists; the stage that writes that file runs it once more.
