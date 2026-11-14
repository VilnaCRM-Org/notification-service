# Architecture: real-time event delivery hub (issue #4)

This document turns `prd.md` of this bundle and the design input of 2026-11-14 (Kafka plus stateless Rust nodes that all read the notifications topic and filter events themselves, deduplication on the client side, PostgreSQL removed) into a design that can be coded story by story. It replaces the first plan (PostgreSQL event log, leader lease, SQS source) completely. The PRD binds the requirements, the variable names and the routes; the design brief of the reader (positions, ports, start-up, tail round, window, subscription, replay) binds the algorithms, except where the PRD decided otherwise, which is stated at the place. A fact about the repository was read in the code, a fact about a library in the source of the locked or pinned version; each is cited where it is used or in "Verified Library Facts". Decisions of this stage are marked `> Assumption:`. No latency, throughput or capacity figure exists for this design; the capacity figure of NFR-1 is a design target.

## Tech Stack

- Rust toolchain: channel 1.99.1 (`rust-toolchain.toml`), edition 2024, `rust-version` 1.99.1.
- Crates of `[workspace.dependencies]` that the change uses or adds:

| Crate | Requirement | What the change uses it for |
| --- | --- | --- |
| `krafka` | `=0.32.0`, `default-features = false`, features `rustls-aws-lc-rs`; added. The feature `test-broker` only on dev-dependency lines | reading the topic: metadata, watermarks, fetch without a consumer group (event-stream infrastructure); producing in tests and the test publisher |
| `axum` | 0.8.9, features `macros` and, added, `ws` | routes, extractors, `response::sse::{Sse, Event, KeepAlive}`, `extract::ws::{WebSocketUpgrade, WebSocket, Message, Utf8Bytes, CloseFrame}`, `Json::from_bytes` |
| `tokio` | 1.54.1, features unchanged | `sync::{mpsc, watch, Semaphore}`, `time`, `select!`, `task::JoinSet` in the composition root; `time::timeout` in the Kafka adapter; dev feature `test-util` |
| `serde` | 1.0.229 | query parameters, WebSocket commands, the envelope, response types |
| `serde_json` | 1.0.154 | parsing the envelope and writing the event object (application, normal dependency); test oracles |
| `async-trait` | 0.1.92 | the topic ports |
| `thiserror` | 2.0.21 | error types |
| `uuid` | 1.29.2 | the envelope `id` |
| `validator` | 0.21.0 | `ValidatedQuery` of the catalog and stream parameters |
| `humantime` | 2.4.0 | `parse_rfc3339` for `occurredAt`; durations in the configuration |
| `futures-util` | 0.3.34 | `stream::unfold` for the SSE body |
| `tracing` | 0.1.44 | the log entries of NFR-15 |
| `utoipa`, `utoipa-axum` | 6.0.0, 0.3.0 | the two documented operations |
| `async-graphql` | 7.2.1 | `eventTypes`, `eventStreamStatus` |
| `clap` | 4.6.7 | the new `serve` arguments |
| `tower-http` | 0.7.1, features unchanged plus, added, `cors` | `cors::{CorsLayer, AllowOrigin}` |
| `tower` | 0.5.3 | `ServiceBuilder::option_layer` |
| `socket2` | 0.6.5 (already in `Cargo.lock`), feature `all`; added | `SockRef::set_tcp_user_timeout` on every accepted connection (`crates/service`) |
| `tokio-tungstenite` | 0.29.0 (what `axum`'s `ws` resolves); added, dev only | WebSocket client of the tests in `crates/service` and `tests/e2e` |
| `proptest`, `rstest`, `http-body-util` | as in the manifest | tests |
| `event-stream-domain`, `event-stream-application`, `event-stream-infrastructure` | path entries; added by the scaffold | the three new crates |

- Removed by the PostgreSQL removal stories: `sqlx`, `some-module-domain`, `some-module-application`, `some-module-infrastructure`, and `fake` when no fixture uses it any more.

## Constraints

(a) Commit convention: commit each story with the hooks active as `<type>(#4): story N.M <title>` (type one of feat, fix, refactor, build, chore, ci, docs, style, perf, test). Stay on the checked-out feature branch and never commit on `main`; never push, Ralph never pushes and the plugin command pushes; never bypass a hook.

(b) Story dispatch: a story whose `Depends on:` stories are done is independent.
When the rust-backend-sdlc:rust-implementer agent is available, independent stories may run as parallel subagents under the DB rule; otherwise implement stories sequentially under the same DB rule.
DB rule: parallel agents run only the DB-free targets `make.format` (make format), `make.lint_code` (make lint-code), `make.unit_tests` (make unit-tests), `make.architecture` (make architecture) and `make.complexity` (make complexity); one serialized verify step at a time runs the DB-backed targets `make.setup_test_db` (make setup-test-db), `make.integration_tests` (make integration-tests), `make.tests_with_coverage` (make tests-with-coverage), `make.e2e_tests` (make e2e-tests) and `make.sqlx_prepare` (make sqlx-prepare). A target that reads not available is skipped and reported.

- After the PostgreSQL removal story, make.setup_test_db and make.sqlx_prepare read not available: capability absent; the Kafka-backed targets stay in the serialized step (one shared broker).
- Protected: an item of `protected-config.toml` changes only in the story that names it, only after the maintainer's approval (label approved-config-change, session with ALLOW_PROTECTED_CONFIG_EDIT=1), never to make a check pass; example routes go only with approved-breaking-change.
- No suppression: no `#[allow]`, `#[mutants::skip]`, `#[coverage(off)]` or coverage or mutation exclusion; `#[expect]` only with a reason.
- Tools only through make targets; never cargo, docker or a script directly.
- Placement: new code only in `crates/notification/event-stream/{domain,application,infrastructure}` and `crates/service`; event-stream and health-check never depend on each other.
- Kafka: `krafka` only in event-stream-infrastructure (and tests); domain and application name no Kafka type; the service only reads: no producer, group or commit.
- Delivery: publishing never awaits a subscriber (`try_send`); no lock across `.await`; ids on a connection strictly increase; offsets are compared with the cursor in the service.
- Contracts: `/api/events/stream` and `/api/events/ws` stay out of OpenAPI; a route change regenerates both snapshots in the same commit; no GraphQL description holds node, Connection, update or delete.
- Time and ids only through `Clock`, tokio time and `UuidGenerator`; logic in inherent methods, not in `#[async_trait]` bodies; every awaiting test is bounded by `tokio::time::timeout`.
- Quality: 100 % line and function coverage, zero surviving or timed-out mutants, clippy pedantic, cognitive complexity 8, 50 lines, 5 arguments.
- A new variable lands in `cli.rs`, `config.rs`, `docs/environment.md` and `.env` together; a new crate adds its `workspace.dsl` component; a changed synchronised file joins `.templatesyncignore`.
- No latency, throughput or capacity figure is claimed; NFR-1 is a design target.

## Context and Goals

The service becomes an open-source alternative to Mercure for VilnaCRM frontends: a frontend reads the catalog of event types (FR-1 to FR-3), subscribes over SSE or WebSocket (FR-5 to FR-11), resumes on any node with the last id (FR-12 to FR-15) and receives every event that producers write to the one-partition Kafka topic `KAFKA_TOPIC` (FR-17 to FR-23).

What exists today (read in the code): one binary `service` with the contexts `shared`, `internal/health-check` and the template's example `company-subdomain/some-module` (PostgreSQL-backed), no background task, an SQS client used only by a health probe, and a `serve` command that drains HTTP on SIGTERM (`crates/service/src/command/serve.rs:104-167`).

What this change adds:

- one bounded context `notification/event-stream` with three crates (domain, application, infrastructure);
- one supervised background loop in the composition root, the tail of the topic, and a hub that fans each event out to the local subscribers;
- a memory window of recent events per node and a replay from Kafka below it;
- two documented REST operations (`GET /api/event-types`, `GET /api/events/status`), two GraphQL queries (`eventTypes`, `eventStreamStatus`), two undocumented streaming routes (`GET /api/events/stream`, `GET /api/events/ws`), and a fourth readiness probe;
- fifteen configuration variables (FR-29), one Kafka broker in the compose environments and CI, a K6 scenario with a publisher;
- the removal of PostgreSQL and of the example context, as the last stories, after approval.

Data flow on every node:

```text
producers --> Kafka topic (1 partition) --+--> node A: TailEvents --> Window (memory) --> Hub --> queue per subscriber --> SSE / WebSocket
                                          +--> node B: TailEvents --> Window --> Hub --> ...
                                          +--> any node: TopicReplay (4 permits) <-- Subscription below its window
```

Every node reads the whole topic without a consumer group, keeps its position in memory and filters events against its own subscribers. There is no leader, no election, no lease, no lock, no link between nodes and no shared state besides Kafka (FR-22). The wire id of an event is its Kafka offset plus 1 and is the same on every node, so a client resumes anywhere (FR-12).

Goals that shape the design: the publish path never waits for a client (NFR-10); a connected client causes no broker read per event (NFR-3); every loop is a one-round use case testable with paused tokio time and fakes (NFR-20); the Kafka client sits behind ports of the domain and can be replaced by touching the infrastructure crate, the composition root and the manifests only (FR-30).

## Verified Library Facts

Each fact below was read in the source or probed, as the fact files of this re-plan record (`kafka-facts.md`, `repo-facts.md`, `postgres-removal.md` in the planning scratchpad); where this document adds a fact it says so.

1. **krafka 0.32.0, the read path** (source under `krafka-0.32.0/src`, probes against `apache/kafka:4.1.1`):
   - Handle: `Kafka::builder(bootstrap).client_id(..).security(AuthConfig).request_timeout(Duration).connect_timeout(Duration).connect().await -> Result<Kafka>` (`client.rs:75`, `:~527`). `connect` makes ONE attempt (`network/pool.rs:9-12`); a refused port fails in under 5 ms with `KrafkaError::Network` (probe 07). `request_timeout >= connect_timeout` is enforced (probe 07). One data connection per broker per handle, shared by all its clients (`pool.rs:3-8`); a parked long poll delays any other call on the same handle by seconds (probe 12), so the tail, the probe and each replay reader need a handle of their own.
   - Consumer without a group: `kafka.consumer_without_group()` and the builder calls `isolation_level`, `auto_offset_reset`, `metrics_push`, `fetch_min_bytes`, `fetch_max_wait`, `max_poll_records`, `max_buffered_records`, `max_partition_fetch_bytes`, `fetch_max_bytes`, `build().await` (contacts no broker) (`consumer/builder.rs:51-364`). `assign(topic, vec![0])` (`consumer/mod.rs:436`), `seek(topic, 0, offset)` (local, clears the per-partition fetch backoff, `:473`, `state.rs:~432-449`), `poll(timeout)` (cancel safe, `:694`), `position(topic, 0)` (`:1029`), `fetch_watermarks(topic, 0) -> (low, high)` (`:634`), `fetch_metadata(Some(topic))` (`:649`).
   - Defaults that must be overridden: isolation read-uncommitted (delivers aborted records, probe 06), offset reset `Latest` (jumps silently to the end below the log start, probe 04), request time-out 30 s and connect time-out 10 s, `metrics_push` on, client id `krafka` (`consumer/config.rs:343-379`).
   - `poll` never reports a broker outage: stopped, killed or hung broker, it returns `Ok(empty)` (probes 08 to 11); it is bounded by the request time-out, not by its argument (probe 09). Only `fetch_watermarks` returns the error. After an unclean outage the internal backoff (up to 30 s, not configurable) delayed new records by about 20 s; a `seek` to the own position brought them in 0.12 s to 0.48 s (probe 11).
   - With `AutoOffsetReset::None` a position below the log start makes every `poll` return `Err(NoOffset)` (probe 04). A seek above the end is silently rewound to the end (probe 04). Under read-committed the end is the last stable offset; transaction markers and aborted records take offsets, so offsets are not dense, and `position()` moves past them (probe 06).
   - A missing topic: `fetch_metadata` returns `Ok` with no topic, `assign` and `poll` succeed, `fetch_watermarks` fails (probe 05). The client never asks the broker to create a topic (`allow_auto_create_topics` false, probe 05).
   - A batch that cannot be decoded makes `poll` return `Err(Protocol { .. })` on every call until the application seeks past it (`fetcher/mod.rs:36-68`; not probed).
   - Fetch limits are soft: a 2 MiB record was delivered with a 1 KiB partition limit (probe 14). All five compression codecs decode without features (probe 13).
   - Errors are `#[non_exhaustive]` (`error.rs:54`); the adapter tests them with `matches!`, never with `match`, because of `wildcard_enum_match_arm`.
   - Security (source, read for this document in `src/auth/mod.rs`): `AuthConfig::plaintext()` (:979), `AuthConfig::ssl(TlsConfig::new())` (:987, :715), `AuthConfig::sasl_plain(user, password)` (:1031), `sasl_scram_sha256` (:1042), `sasl_scram_sha512` (:1053), and `.with_tls(TlsConfig)` turns a SASL plaintext configuration into `SASL_SSL` (:1015-1018). Not probed against a broker.
   - Test support: feature `test-broker`, `krafka::testing::FakeBroker` (`start_in_memory`, `crash`, `restart`, hooks `on`/`on_once`/`on_times` with `Control::{Delay, Disconnect, Silence, CorruptRecords, Error}`, `create_topic`, `with_state` to move the log start). It serves Produce and Fetch with transactions; it does NOT serve OffsetForLeaderEpoch, so a seek above the end becomes `NoOffset` there, and it does not check a fetch below the log start. Topics are created on a real broker with `kafka.admin().create_topics(..)` (`src/admin/topics.rs:303`; read for this document, not run).
   - Features (`Cargo.toml`, read for this document): `default = ["ring"]`; `rustls-aws-lc-rs` selects aws-lc-rs; compression codecs are unconditional dependencies.
2. **axum 0.8.9**: the feature `ws` adds `tokio-tungstenite 0.29.0`, `sha1`, `base64`; `WebSocketUpgrade::on_upgrade` runs the callback in a detached `tokio::spawn` (`src/extract/ws.rs:359-376`) that the graceful shutdown does not wait for; `max_message_size`, `max_frame_size`, `read_buffer_size` exist (`:159-204`); upgrade rejections are plain text (`:951-987`) and must become problems. `Sse::keep_alive(KeepAlive::new().interval(d))` writes the comment `:` after `d` without an event, on a `tokio::time::Sleep`. `Event::data` starts a new `data:` line at every line break; `Event::id` and `Event::event` panic on a line break. `axum::serve::ListenerExt::tap_io` runs a closure on every accepted stream (`src/serve/listener.rs:66-135`). `OpenApiRouter::route` serves a route and leaves it out of the document (`utoipa-axum-0.3.0/src/router.rs:251`).
3. **tokio 1.54.1**: `sync::{mpsc, watch, Semaphore}` behind `sync`, `time` behind `time`, `JoinSet` behind `rt`; the semaphore is first in, first out. Domain crates may not use tokio; today infrastructure has it only as a dev-dependency (`architecture.toml:52-61`).
4. **socket2 0.6.5**: `SockRef::set_tcp_user_timeout` with feature `all` (`src/sys/unix.rs:2552`), Linux, Android, Fuchsia and Cygwin only.
5. **Repository rules** (`repo-facts.md`): handlers use only `ApiPath`, `ValidatedQuery`, `ValidatedJson`, `ApiJson`, plus what a context's extractor test allows (`crates/shared/application/src/http/extract.rs`, `crates/service/src/http/router/tests.rs:423-520`); `std::time::SystemTime::now`, `Instant::now`, `TcpListener::bind` and `std::env::var*` are disallowed methods (`clippy.toml:30-50`); no `mod.rs`; sibling `tests.rs`; `PageRequest::new(page, items_per_page) -> Result<PageRequest, InvalidPageRequest>` (`crates/shared/domain/src/value_object/page_request.rs:34`); `HealthProbe { fn dependency(&self) -> Dependency; async fn check(&self) -> Result<(), BoxError> }` (`crates/internal/health-check/domain/src/port.rs:9-19`); `Adapters::connect` touches no network (test `tests/integration/bootstrap.rs:83-108`); `ring` and `aws-lc-rs` are both already in `Cargo.lock` (checked for this document: `Cargo.lock:2972` names `ring`).
6. **The pinned K6 image** `grafana/k6:2.3.0` has `k6/ws`, `k6/websockets`, `k6/timers`, `http.asyncRequest` and no Kafka module; importing `k6/x/kafka` makes it download a binary (probe `probes/k6/`). The Kafka image ships `kafka-console-producer.sh`, `bash`, `date +%s%3N` and `/proc/sys/kernel/random/uuid` (probe of `repo-facts.md`, section 7).

> Assumption: `krafka` is built with `default-features = false` and the feature `rustls-aws-lc-rs`. aws-lc-rs is already the crypto provider of the workspace (sqlx `tls-rustls-aws-lc-rs` today, the AWS SDK), so the change adds no second provider choice, and the crate states that aws-lc-rs wins when both rustls providers are enabled. `make.check_security` (`cargo deny`) was never run on `krafka`; the story that adds it runs it, and a refused licence or advisory is escalated, never answered by an edit of `deny.toml`.

## Bounded Context notification/event-stream

Created with `make.new_context` (`group=notification context=event-stream`): `crates/notification/event-stream/{domain,application,infrastructure}`, packages `event-stream-domain`, `event-stream-application`, `event-stream-infrastructure`. The architecture gate derives the context name `event-stream` from the package names. The scaffold writes three manifests without dependencies and a `lib.rs` with a doc comment per layer (`xtask/templates/*`), so the context exists before any crate of the example context is removed (a protected test needs at least 13 workspace crates). This is the only new bounded context; every other new component is in `crates/service`.

### Domain crate

Manifest: `async-trait`, `shared-domain`, `thiserror`, `uuid`; dev `proptest`, `rstest`. No I/O, no tokio, no serde (the domain allow-list: `async-trait`, `thiserror`, `uuid`).

| File under `crates/notification/event-stream/domain/src/` | Content |
| --- | --- |
| `lib.rs` | module declarations and re-exports |
| `error.rs` | `InvalidEvent`, `InvalidCursor` |
| `event.rs` | `StoredEvent` |
| `value_object.rs` | module declarations |
| `value_object/event_type.rs` | `EventType` |
| `value_object/offset.rs` | `Offset` |
| `value_object/sequence.rs` | `Sequence` |
| `value_object/cursor.rs` | `Cursor` |
| `value_object/watermarks.rs` | `Watermarks` |
| `value_object/page_limit.rs` | `PageLimit` |
| `port.rs`, `port/topic.rs` | `TopicError`, `TopicRecord`, `Batch`, `TopicProbe`, `TopicTail`, `TopicReplay` |

Each file has a sibling `tests.rs` declared `#[cfg(test)] mod tests;`.

```rust
/// 1 to 128 characters matching ^[a-z][a-z0-9]*([._-][a-z0-9]+)*$ (checked by hand, no regex crate).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventType(String);
impl EventType {
    pub const MAX_CHARS: usize = 128;
    pub fn new(value: String) -> Result<Self, InvalidEvent>;
    pub fn as_str(&self) -> &str;
}
impl std::fmt::Display for EventType { /* the name */ }

/// The Kafka offset of a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Offset(u64);
impl Offset {
    pub const fn new(value: u64) -> Self;
    pub const fn value(self) -> u64;
    /// The wire id: offset + 1 (an offset is at most i64::MAX, so the sum never overflows; written saturating).
    pub const fn sequence(self) -> Sequence;
}

/// The wire id of an event, 1 or greater.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sequence(u64);
impl Sequence {
    pub fn new(value: u64) -> Result<Self, InvalidEvent>;   // 0 -> InvalidEvent::SequenceZero
    pub const fn value(self) -> u64;
}
impl std::fmt::Display for Sequence { /* decimal */ }

/// "Everything up to this id was seen". A cursor c is also the Kafka position c: the next offset to read is c.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cursor(u64);
impl Cursor {
    pub const START: Self = Self(0);
    pub const fn new(value: u64) -> Self;
    /// ASCII digits only (no sign, no blank, not empty), then u64.
    pub fn parse(text: &str) -> Result<Self, InvalidCursor>;
    pub const fn value(self) -> u64;
    /// The offset to read next for this cursor: the same number.
    pub const fn next_offset(self) -> Offset;
    /// The cursor n positions lower, saturating at 0.
    pub const fn rewound_by(self, n: u64) -> Self;
    /// The cursor n positions higher, saturating at u64::MAX.
    pub const fn advanced_by(self, n: u64) -> Self;
    /// `a - b`, 0 when b >= a.
    pub const fn distance_from(self, lower: Self) -> u64;
}
impl From<Sequence> for Cursor { /* the same number */ }
impl std::fmt::Display for Cursor { /* decimal */ }

/// The two ends of the partition as positions, read at one instant: `earliest` is the log start offset,
/// `end` the last stable offset (read-committed). An empty topic has earliest == end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Watermarks { pub earliest: Cursor, pub end: Cursor }

/// A valid event as the node holds it. `json` is the compact event object of FR-18
/// ({"id":..,"type":..,"occurredAt":..,"data":..}), built once by the application from the record value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEvent { pub sequence: Sequence, pub id: Uuid, pub event_type: EventType, pub json: String }
impl StoredEvent {
    /// Memory cost of the event in the window: json bytes + type bytes + 128 (FR-13's per-event overhead).
    pub fn cost(&self) -> usize;
}

/// Bounds of one replay page: a number of events and a number of bytes of `json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageLimit { pub rows: usize, pub bytes: usize }
impl PageLimit {
    /// True when the events reach `rows`, or their json bytes reach `bytes`.
    pub fn reached(&self, rows: usize, bytes: usize) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InvalidEvent {
    #[error("An event type must contain between 1 and 128 characters.")] TypeLength,
    #[error("An event type must match ^[a-z][a-z0-9]*([._-][a-z0-9]+)*$.")] TypePattern,
    #[error("A sequence must be 1 or greater.")] SequenceZero,
}
impl InvalidEvent { /// `length`, `pattern`, `range`.
    pub const fn code(self) -> &'static str; }

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("A cursor must be a decimal unsigned 64-bit integer.")]
pub struct InvalidCursor;
```

`Uuid` is `shared_domain::Uuid` (re-exported `uuid::Uuid`). Arithmetic is written with `saturating_*` (lint `arithmetic_side_effects`).

> Assumption: `StoredEvent` carries the encoded event object as text, not the parsed members. The domain may not use `serde_json`, the window and the replay pages hold events for many connections, and every transport writes exactly this object; parsing once and keeping the text makes the encoding cost of NFR-2 one serialisation per event per node.

#### Ports (`port/topic.rs`)

```rust
#[derive(Debug, thiserror::Error)]
pub enum TopicError {
    /// The broker could not be read: connect, request, time-out, or any other client error.
    #[error("the topic is unavailable")] Unavailable(#[source] BoxError),
    /// The requested position is below the log start; `earliest` is the log start read just now.
    #[error("the position is below the earliest retained offset")] BelowEarliest { earliest: Cursor },
    /// The record batch at `at` cannot be decoded by the client.
    #[error("the record batch cannot be decoded")] Undecodable { at: Cursor },
}

/// One record of the partition; key and headers are not carried (D4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicRecord { pub offset: Offset, pub value: Option<Vec<u8>> }

/// Records at or above the requested position, in offset order, and the reader position after them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Batch { pub records: Vec<TopicRecord>, pub next: Cursor }

#[async_trait::async_trait]
pub trait TopicProbe: std::fmt::Debug + Send + Sync {
    /// The partition count of the topic; `None` when the metadata answer holds no such topic.
    async fn partition_count(&self) -> Result<Option<u32>, TopicError>;
    /// The log start offset and the last stable offset of partition 0.
    async fn watermarks(&self) -> Result<Watermarks, TopicError>;
}

#[async_trait::async_trait]
pub trait TopicTail: std::fmt::Debug + Send {
    /// Sets the reader position; always calls the client's seek, which also clears its fetch backoff.
    async fn seek(&mut self, position: Cursor) -> Result<(), TopicError>;
    /// One long poll from the reader position; returns early with records, or empty after the wait.
    async fn fetch(&mut self) -> Result<Batch, TopicError>;
}

#[async_trait::async_trait]
pub trait TopicReplay: std::fmt::Debug + Send + Sync {
    /// One page of records at positions >= `from`: a seek, then one short poll.
    async fn page(&self, from: Cursor) -> Result<Batch, TopicError>;
}
```

Rules every adapter of these ports keeps (the fakes of `tests/support` keep them too):

1. `Batch::records` holds only offsets at or above the requested position, ascending. `Batch::next >= ` the requested position and `>=` the last offset plus 1.
2. A call never waits without bound: it returns or fails with `Unavailable` within the adapter's time limit.
3. A position below the log start is reported as `BelowEarliest`, never skipped silently.
4. All three traits are dyn-compatible (`&self` or `&mut self`, no generic method) and used as `Arc<dyn TopicProbe>`, `Box<dyn TopicTail>` (one owner, the tail loop) and `Arc<dyn TopicReplay>`.

> Assumption: the port has a third error, `Undecodable { at }`, beyond the two of the design brief. FR-19 requires the tail to skip an undecodable batch, log it with the reason `undecodable batch` and count it once; without a variant of its own the application cannot tell it from an outage, which must not skip anything.

### Application crate

Manifest: `async-graphql`, `async-trait`, `axum` (feature `ws` through the workspace entry), `event-stream-domain`, `futures-util`, `humantime`, `serde`, `serde_json`, `shared-application`, `shared-domain`, `thiserror`, `tokio`, `tracing`, `utoipa`, `utoipa-axum`, `uuid`, `validator`; dev `http-body-util`, `proptest`, `rstest`, `test-support`, `tokio` with `test-util` and `io-util`, `tokio-tungstenite`, `tower`.

| File under `crates/notification/event-stream/application/src/` | Content |
| --- | --- |
| `lib.rs` | modules, re-exports, the module rule below |
| `error.rs` | `EventStreamError` and `From<EventStreamError> for Problem` |
| `catalog.rs` | `EventCatalog` |
| `query.rs`, `query/list_event_types.rs` | `EventTypeView`, `ListEventTypes`, `ListEventTypesHandler` |
| `envelope.rs` | `Envelope`, `RejectReason`, `parse_event` |
| `window.rs` | `Window`, `WindowRead` |
| `tail.rs` | `EventPublisher`, `TailEvents`, `TailSettings`, `TailState`, `TailFault`, `RoundFailed`, constants |
| `status.rs` | `NodeStatus`, `NodeState`, `StartReason`, `StatusSource` |
| `http.rs` | `EventStreamState`, `StreamSettings`, `router()` |
| `http/dto.rs` | `ListEventTypesParams`, `EventTypeResponse`, `EventStreamStatusResponse`, `StreamParams`, `WsParams`, `Command`, `TypesError`, `parse_names`, `parse_types`, `parse_cursor` |
| `http/handler.rs` | `list_event_types`, `event_stream_status`, `stream`, `ws`, `list_types` (shared with GraphQL), `MAX_MESSAGE_BYTES`, `READ_BUFFER_BYTES` |
| `http/frame.rs` | `Frame`, `ws_json`, `error_json` |
| `http/hub.rs` | `Hub`, `HubSettings`, `HubError`, `Delivery`, `Subscriber` |
| `http/subscription.rs` | `Subscriptions`, `ReplaySettings`, `Subscription`, `SubscriptionRequest`, `Step`, `EndReason`, `TooManyTypes` |
| `http/session.rs` | `Session`, `Inbound` (the socket-free WebSocket state machine) |
| `http/socket.rs` | `Connection`, `impl Connection for WebSocket`, `serve`, `CLOSE_TIMEOUT` |
| `graphql.rs` | `EventTypeObject`, `EventStreamStatusObject`, `EventStreamQuery` |
| `tests/integration/{main.rs,support.rs,ws.rs}` (crate root) | the real-socket test of the upgrade |

Module rule, written into `lib.rs`: `catalog`, `query`, `envelope`, `window`, `tail`, `status` and `error` do not import `axum`, `utoipa` or `async_graphql`; only `http` and `graphql` may. The hub carries frames already encoded for both transports (`axum` types), so it is a delivery adapter in `http`, and the tail reaches it through the trait `EventPublisher` that `tail.rs` owns.

> Assumption: `serde_json` becomes a normal dependency of the application crate (protected allow-list entry, "Protected Changes"). The envelope is JSON (D4), the client must receive its `id` (D8, FR-18), and the domain and infrastructure layers may not use `serde_json` either; the application layer is where record values become events.

#### Catalog and query

```rust
// catalog.rs
/// The names of EVENT_TYPES, distinct, sorted in byte order (the order of `EventType: Ord`). At most 1 000.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventCatalog(Vec<EventType>);
impl EventCatalog {
    pub fn new(types: BTreeSet<EventType>) -> Self;
    /// The page `page` of `items_per_page` names; empty after the last page.
    pub fn page(&self, request: PageRequest) -> Vec<EventType>;
}

// query.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventTypeView { pub name: String }

// query/list_event_types.rs
#[derive(Debug, Clone, Copy)]
pub struct ListEventTypes { pub page: u32, pub items_per_page: u32 }
impl Query for ListEventTypes { type Output = Vec<EventTypeView>; type Error = EventStreamError; const NAME: &'static str = "ListEventTypes"; }
pub struct ListEventTypesHandler { catalog: Arc<EventCatalog> }
impl ListEventTypesHandler { pub const fn new(catalog: Arc<EventCatalog>) -> Self; }
// handle: PageRequest::new(page, items_per_page)? then catalog.page(request), mapped to views.

// error.rs
#[derive(Debug, thiserror::Error)]
pub enum EventStreamError { #[error(transparent)] InvalidPage(#[from] InvalidPageRequest) }
// InvalidPage -> 422 with one violation on `page` or `itemsPerPage` (code `range`).
```

`EventCatalog::page` uses `PageRequest::offset()` and `limit()` converted with `usize::try_from(..).unwrap_or(usize::MAX)` and `iter().skip(..).take(..)`; it reads nothing and answers on a starting node (FR-1).

#### Envelope (`envelope.rs`)

```rust
/// Why a record is not an event (FR-19). The text is the `reason` field of `event record rejected`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RejectReason {
    #[error("missing value")] Missing,
    #[error("value too large")] TooLarge,
    #[error("not a JSON object with the envelope members")] Shape,   // not UTF-8, not JSON, not an object, a duplicate or missing member, a wrong JSON type
    #[error("invalid id")] Id,
    #[error("invalid type")] Type,
    #[error("invalid occurredAt")] OccurredAt,
    #[error("undecodable batch")] Undecodable,                       // used by the tail only
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Envelope { id: String, r#type: String, occurred_at: String, data: serde_json::Value }

/// Turns a record value into an event: size check first (`value.len() > max_bytes` -> TooLarge),
/// then `serde_json::from_slice::<Envelope>` (unknown members ignored, duplicate members refused by the
/// derive), then `Uuid::try_parse` (36-character hyphenated form, either case), `EventType::new`, and
/// `occurredAt`: a trailing `+00:00` is read as `Z`, then `humantime::parse_rfc3339` must accept it and the
/// year must be 1970 to 9999. The event object is written once with `serde_json::to_string` of a struct whose
/// fields are `id` (lower-case hyphenated), `type`, `occurredAt` (as written), `data`, in this order.
pub fn parse_event(offset: Offset, value: Option<&[u8]>, max_bytes: usize) -> Result<StoredEvent, RejectReason>;
```

> Assumption: `data` is held as `serde_json::Value`, so member order inside it and the spelling of numbers may change (FR-18 allows both) and a number beyond the range of `i64`, `u64` and `f64` loses precision. The alternative, `RawValue`, needs a new feature of `serde_json` and keeps the producer's white space, which FR-18 removes.
>
> Assumption: `occurredAt` ending in `+00:00` is rewritten to `Z` before parsing and forwarded as written. This accepts exactly the two forms of FR-18 whether or not `humantime::parse_rfc3339` itself accepts `+00:00`, which this plan did not verify.

#### Window (`window.rs`)

```rust
/// The most recent valid events of this node, in id order, bounded by bytes (FR-13).
#[derive(Debug)]
pub struct Window { /* std::sync::RwLock<Inner { events: VecDeque<Arc<StoredEvent>>, bytes: usize, start: Cursor }>, max_bytes: usize */ }
#[derive(Debug)]
pub enum WindowRead { Evicted, Page { events: Vec<Arc<StoredEvent>>, settled: Cursor } }
impl Window {
    pub const SCAN: usize = 10_000;
    pub fn new(max_bytes: usize) -> Self;              // empty, start Cursor::START
    /// Clears the window and sets its floor F (the start position of the tail, FR-20).
    pub fn restart(&self, start: Cursor);
    /// Appends; then pops the front while more than one event is held and bytes > max_bytes;
    /// each pop sets `start` to the popped event's sequence.
    pub fn push(&self, event: Arc<StoredEvent>);
    /// The floor F: every valid event with an id in (F, tail position] is held.
    pub fn start(&self) -> Cursor;
    /// The id of the newest event held, `None` when empty.
    pub fn last(&self) -> Option<Sequence>;
    pub fn bytes(&self) -> usize;
    pub fn read(&self, after: Cursor, up_to: Cursor, types: &BTreeMap<EventType, Cursor>, limit: PageLimit) -> WindowRead;
}
```

`read` rules: `after < start` gives `Evicted`. An empty `types` gives an empty page with `settled = up_to`. Otherwise `partition_point` finds the first sequence above `after`; at most `SCAN` entries with a sequence at or below `up_to` are examined, and an entry is collected when its type is in `types` with a since below its sequence, until `limit.reached(..)`. `settled` is the last collected sequence when the limit cut, the last examined sequence when the scan bound cut, else `up_to`. The lock is a `std::sync::RwLock`, held for one call, never across an await; a poisoned lock is taken over with `PoisonError::into_inner`.

> Assumption: the window is bounded by bytes only, as FR-13 states, with at least one event kept so that a single large event does not empty it. The cost is `StoredEvent::cost`: the json bytes, the type bytes and 128 bytes for the `Arc`, the deque slot and the struct. A count bound (the brief's `EVENT_WINDOW_MAX_EVENTS`) is not needed: 128 bytes per event already bound the count to `EVENT_WINDOW_BYTES / 128`.

#### Tail (`tail.rs`)

```rust
pub trait EventPublisher: std::fmt::Debug + Send + Sync {
    /// The node reached its start goal: from now on the publisher accepts subscribers. Called once.
    fn started(&self, position: Cursor);
    /// Hands one event to the local subscribers. Never waits. Returns how many fell behind with it.
    fn publish(&self, event: &StoredEvent) -> usize;
    /// The tail passed offsets without a valid event (markers, invalid records): the position becomes the max.
    fn advance(&self, position: Cursor);
}

#[derive(Debug, Clone, Copy)]
pub struct TailSettings { pub max_data_bytes: usize, pub preload: u64, pub request_timeout: Duration }

/// Why a node is not reading yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TailFault { BrokerUnreachable, TopicNotFound, PartitionCount }

/// What the tail publishes after every round (a `tokio::sync::watch` value).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TailState {
    pub fault: Option<TailFault>,
    pub started: bool,                  // the node reached the end it read at start (FR-20); never reset
    pub position: Option<Cursor>,       // P; None before the first seek
    pub earliest: Option<Cursor>,       // L of the last successful watermark read
    pub end: Option<Cursor>,            // E of the last successful watermark read
    pub broker_reachable: bool,         // the last watermark read succeeded
    pub rejected: u64,                  // invalid records and undecodable batches skipped by the tail
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the round failed")]
pub struct RoundFailed;

#[derive(Debug)]
pub struct TailEvents { /* probe, tail, window, publisher, clock, state: watch::Sender<TailState>,
                           settings, goal: Option<Cursor>, last_observation: Option<tokio::time::Instant>,
                           failing: bool, undecodable: Option<Cursor>, last_fault: Option<TailFault> */ }
impl TailEvents {
    pub const WAIT: Duration = Duration::from_millis(500);           // long-poll wait of the tail
    pub const OBSERVE_EVERY: Duration = Duration::from_secs(1);      // watermark period (FR-21)
    pub fn new(probe: Arc<dyn TopicProbe>, tail: Box<dyn TopicTail>, window: Arc<Window>,
               publisher: Arc<dyn EventPublisher>, settings: TailSettings) -> (Self, watch::Receiver<TailState>);
    /// One round: the start-up steps until the first seek, then a tail round ("Algorithms").
    pub async fn round(&mut self) -> Result<(), RoundFailed>;
    /// The longest a round may take: WAIT + 3 x request_timeout. The loop cuts a round after it.
    pub const fn round_limit(&self) -> Duration;
}
```

`new` has five arguments, the limit. The loop around `round` (pause, back-off, cut, stop flag) is in `crates/service/src/background.rs`. `round` writes the log entries of the tail (NFR-15); each entry is one call of a private function, so that no loop body raises its cognitive complexity above 8.

> Assumption: the long-poll wait of the tail is a constant of 500 ms, the client's default `fetch_max_wait`. FR-29 makes it a constant; a shorter wait costs more requests, a longer one delays a new event by nothing (the poll returns early with records) but lengthens the round limit.

#### Status (`status.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeState { Starting, Ready, ShuttingDown }               // "starting", "ready", "shutting-down"
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartReason { BrokerUnreachable, TopicNotFound, PartitionCount, CatchingUp }  // kebab-case on the wire
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeStatus {
    pub state: NodeState, pub reason: Option<StartReason>, pub broker_reachable: bool,
    pub position: Option<Cursor>, pub end_offset: Option<Cursor>, pub lag: Option<u64>,
    pub window_first: Option<Cursor>, pub window_last: Option<Sequence>,
    pub subscribers: usize, pub rejected_records: u64,
}
/// Reads the state the node already holds; no broker call (FR-4).
pub trait SubscriberCount: std::fmt::Debug + Send + Sync { fn closed(&self) -> bool; fn count(&self) -> usize; }
#[derive(Debug, Clone)]
pub struct StatusSource { tail: watch::Receiver<TailState>, window: Arc<Window>, hub: Arc<dyn SubscriberCount> }
impl StatusSource {
    pub fn new(tail: watch::Receiver<TailState>, window: Arc<Window>, hub: Arc<dyn SubscriberCount>) -> Self;
    pub fn snapshot(&self) -> NodeStatus;
}
```

`snapshot`: `state` is `ShuttingDown` when the hub is closed, else `Ready` when `started`, else `Starting`; `reason` (only while starting) is the fault mapped one to one, else `CatchingUp`; `lag` is `end.distance_from(position)` when both are known; `window_first` is `Some(window.start())` once `position` is `Some`; `window_last` is `window.last()`. `Hub` implements `SubscriberCount`, so `status.rs` needs nothing from `http`.

#### Hub (`http/hub.rs`)

The hub of the first plan is kept, with `advance` added and the cursor taken from the tail:

```rust
#[derive(Debug, Clone, Copy)]
pub struct HubSettings { pub max_subscribers: usize, pub buffer: usize }
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HubError {
    #[error("the node is starting")] NotReady,
    #[error("the node is at its subscriber capacity")] AtCapacity,
    #[error("the node is shutting down")] Closed,
}
// From<HubError> for Problem: 503 `about:blank`, the text as detail; logged at warn as
// `subscription rejected` with the detail (FR-25, NFR-15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Delivery { pub delivered: usize, pub behind: usize }
pub struct Hub { settings: HubSettings, registry: std::sync::Mutex<Registry>, count: tokio::sync::watch::Sender<usize> }
impl Hub {
    pub fn new(settings: HubSettings) -> Arc<Self>;
    /// Under one lock, in this order: Closed, NotReady, AtCapacity (FR-25's precedence); registers nothing.
    pub fn check(&self) -> Result<(), HubError>;
    /// The same checks as `check` under one lock, then registers.
    pub fn register(self: &Arc<Self>, types: &BTreeSet<EventType>) -> Result<Subscriber, HubError>;
    /// Sets the position when the hub has none; a second call changes nothing.
    pub fn start(&self, position: Cursor);
    /// Sets the position to the event's sequence and queues one shared frame for every subscriber of its
    /// type with `try_send`. `None` when the type has no subscriber: nothing is encoded.
    pub fn deliver(&self, event: &StoredEvent) -> Option<Delivery>;
    /// position = max(position, given); no effect before `start`.
    pub fn advance(&self, position: Cursor);
    pub fn close(&self);
    pub fn is_closed(&self) -> bool;
    pub fn subscriber_count(&self) -> usize;
    pub fn type_count(&self) -> usize;
    /// Completes when no subscriber is registered.
    pub async fn drained(&self);
}
impl EventPublisher for Hub { /* started -> start; publish -> deliver(..).map_or(0, |d| d.behind); advance -> advance */ }
impl SubscriberCount for Hub { /* is_closed, subscriber_count */ }

pub struct Subscriber { id: u64, position: Cursor, receiver: mpsc::Receiver<Arc<Frame>>, hub: Arc<Hub> }
impl Subscriber {
    /// The hub position at the last arming (registration or resume).
    pub const fn position(&self) -> Cursor;
    pub async fn recv(&mut self) -> Option<Arc<Frame>>;
    /// A new, empty queue after `recv` returned None; returns and stores the hub position at re-arming.
    pub fn resume(&mut self) -> Result<Cursor, HubError>;
    /// Adds the types; returns the hub position at that moment.
    #[must_use] pub fn subscribe(&self, types: &BTreeSet<EventType>) -> Cursor;
    pub fn unsubscribe(&self, types: &BTreeSet<EventType>);
    pub fn hub_closed(&self) -> bool;
}
impl Drop for Subscriber { /* removes the entry and its queues and publishes the new count */ }
// private: Entry { sender: Option<mpsc::Sender<Arc<Frame>>>, types: BTreeSet<EventType> };
// Registry { closed, position: Option<Cursor>, next_id, subscribers: BTreeMap<u64, Entry>,
//            by_type: HashMap<EventType, BTreeMap<u64, mpsc::Sender<Arc<Frame>>>> }
```

Rules of the registry (unchanged from the first plan; all hold after every method, and every method is one synchronous section under the mutex):

- **The entry owns the queue.** `Entry::sender` is the subscriber's sender; `by_type` holds one clone under each of the entry's types and under no other key. `subscribe` and `unsubscribe` change both in one section. A subscriber with no type keeps an open queue (FR-8, FR-10).
- **Falling behind.** When `try_send` fails, `Entry::sender` becomes `None` and its clones leave `by_type`; the entry and its types stay. `recv` returns the queued frames, then `None`. `resume` gives the entry a new channel of `buffer` frames and puts the clones back. While the sender is `None`, `subscribe` and `unsubscribe` change only `Entry::types`.
- **No empty key.** A `by_type` entry whose map became empty is removed at once; `close` clears `by_type`.
- **The count is the number of connections.** `subscribers.len()` is compared with `max_subscribers` (`>=` answers `AtCapacity`) and published with `send_replace` under the lock in `register` and `Drop`. Falling behind and `close` do not change the count; a slot is free when the `Subscriber` is dropped (FR-25).
- **Position.** `None` until `start`, then the tail position: `deliver` sets it to the event's sequence, `advance` raises it. `register` and `resume` copy it into the subscriber, `subscribe` returns it.
- `by_type` is only read and written by key; everything iterated is a `BTreeMap` or `BTreeSet` (`iter_over_hash_type`). A poisoned mutex is taken over with `PoisonError::into_inner`. `try_send` is judged with `is_err()`; `drained` is `count.subscribe().wait_for(|n| *n == 0)`.
- The frame is encoded under the lock after the lookup found a subscriber (NFR-2: once per event, shared; not at all without a subscriber).

#### Frame (`http/frame.rs`)

```rust
/// One event encoded once for both transports.
#[derive(Debug)]
pub struct Frame {
    pub sequence: Sequence,
    pub event_type: EventType,
    pub sse: axum::response::sse::Event,     // id: <sequence>, event: <type>, data: <event json, one line>
    pub json: axum::extract::ws::Utf8Bytes,  // {"id":"<sequence>","type":"<type>","data":<event json>}
}
impl Frame { pub fn encode(event: &StoredEvent) -> Self; }
/// The WebSocket frame: `{"id":"` + sequence + `","type":` + serde_json::to_string(type) + `,"data":` + json + `}`.
pub(crate) fn ws_json(event: &StoredEvent) -> String;
/// `{"error":` + serde_json::to_string(reason) + `}`.
pub(crate) fn error_json(reason: &str) -> String;
```

`StoredEvent::json` is compact JSON, so it holds no line break and is exactly one `data:` line (FR-6); `Event::id` and `Event::event` get text without line breaks (a decimal number, an event type name), so they cannot panic.

#### Subscriptions (`http/subscription.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionRequest { pub types: BTreeSet<EventType>, pub cursor: Option<Cursor> }
#[derive(Debug, Clone, Copy)]
pub struct ReplaySettings { pub max_offsets: u64, pub max_data_bytes: usize, pub request_timeout: Duration }
#[derive(Debug)]
pub struct Subscriptions { hub: Arc<Hub>, window: Arc<Window>, replay: Arc<dyn TopicReplay>,
                           tail: watch::Receiver<TailState>, permits: Arc<Semaphore>, settings: ReplaySettings }
impl Subscriptions {
    pub const REPLAY_PERMITS: usize = 4;
    pub fn new(hub: Arc<Hub>, window: Arc<Window>, replay: Arc<dyn TopicReplay>,
               tail: watch::Receiver<TailState>, settings: ReplaySettings) -> Self;
    /// Registers in the hub. No broker read, nothing awaited.
    pub fn open(&self, request: SubscriptionRequest) -> Result<Subscription, HubError>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason { HubClosed, TopicFailed }
#[derive(Debug)]
pub enum Step { Frame(Arc<Frame>), Idle, End(EndReason) }
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a connection subscribes to at most 100 event types")]
pub struct TooManyTypes;
#[derive(Debug)]
pub struct Subscription {
    subscriber: Subscriber, window: Arc<Window>, replay: Arc<dyn TopicReplay>, tail: watch::Receiver<TailState>,
    permits: Arc<Semaphore>, settings: ReplaySettings,
    types: BTreeMap<EventType, Cursor>,   // type -> since: only events above it are written
    floor: Cursor,                        // everything at or below it is settled for this connection
    target: Option<Cursor>,               // None: a replay must start; Some(t): replay while floor < t, else live
    pending: VecDeque<Arc<StoredEvent>>,  // the rest of the last page
    stalled_since: Option<tokio::time::Instant>,
}
impl Subscription {
    pub const MAX_TYPES: usize = 100;
    pub const REPLAY_PAGE: PageLimit = PageLimit { rows: 500, bytes: 1_048_576 };
    pub async fn step(&mut self) -> Step;
    /// `step` until a frame (Some) or the end (None).
    pub async fn next(&mut self) -> Option<Arc<Frame>>;
    /// True when the next step reads Kafka (a permit wait and one page read).
    pub fn replaying(&self) -> bool;
    pub fn subscribe(&mut self, types: BTreeSet<EventType>) -> Result<(), TooManyTypes>;
    pub fn unsubscribe(&mut self, types: &BTreeSet<EventType>);
    pub fn type_count(&self) -> usize;
    pub const fn position(&self) -> Cursor;
}
```

`Subscriptions::new` has five arguments. The steps are in "Algorithms". `replaying()` is true when `pending` is empty, `target` is `Some(t)` with `floor < t`, and `floor < window.start()` (the next page reads Kafka). The start step and window pages await nothing that reads the broker, so the pump may race them against inbound frames.

#### WebSocket session, pump and handlers

`http/session.rs`, `http/socket.rs` and the handlers are the first plan's, with the log replaced by the window and Kafka:

```rust
// http/session.rs: no socket, no timer
pub(crate) enum Inbound { Continue, Reply(Message), Close }
pub(crate) struct Session { subscription: Subscription }
impl Session {
    /// 100 names of 128 characters take 13 135 bytes.
    pub(crate) const MAX_COMMAND_BYTES: usize = 16_384;
    pub(crate) const fn new(subscription: Subscription) -> Self;
    /// Text -> command; Binary -> error frame; Ping and Pong -> Continue; Close -> Close (every variant named).
    pub(crate) fn on_message(&mut self, message: Message) -> Inbound;
    pub(crate) async fn step(&mut self) -> Step;
    pub(crate) fn replaying(&self) -> bool;
    /// HubClosed -> 1001, TopicFailed -> 1011.
    pub(crate) fn close_message(reason: EndReason) -> Message;
    pub(crate) fn type_count(&self) -> usize;
    pub(crate) const fn position(&self) -> Cursor;
}

// http/socket.rs
pub(crate) trait Connection: Send {
    fn receive(&mut self) -> impl Future<Output = Option<Result<Message, axum::Error>>> + Send;
    fn transmit(&mut self, message: Message) -> impl Future<Output = Result<(), axum::Error>> + Send;
}
impl Connection for axum::extract::ws::WebSocket { /* recv and send */ }
pub(crate) const CLOSE_TIMEOUT: Duration = Duration::from_secs(1);
/// One task per connection; `keep_alive` is the ping period and the deadline of every write.
pub(crate) async fn serve<C: Connection>(connection: C, session: Session, keep_alive: Duration);
```

The pump (`socket::serve`), step by step:

1. Every write is `tokio::time::timeout(keep_alive, connection.transmit(message))`. Written: continue. Transport error: stop with `Gone`. Deadline: stop with `Stalled`.
2. Before each turn the pump asks `session.replaying()`.
   - **Replay turn** (true: the step waits for a permit and reads one Kafka page): the step future is pinned and polled to its end; a tick of `ping` (a `tokio::time::Interval` of `keep_alive`, first tick after `keep_alive`, `MissedTickBehavior::Delay`) writes a ping meanwhile; no inbound frame is read in this turn.
   - **Live turn** (false): one `tokio::select!` over `connection.receive()`, `session.step()` and `ping.tick()`, in a function that only reports which arm completed. Inbound `None` or `Some(Err(_))` stops with `Gone`; `Some(Ok(message))` goes to `on_message`: `Continue`, `Reply(frame)` written, `Close` stops with `ClientClosed`.
   - A step result: `Frame(frame)` writes `Message::Text(frame.json.clone())`; `Idle` continues; `End(reason)` stops with `Ended(reason)`.
3. After the last turn: `Gone` does nothing; `Stalled` logs `subscriber evicted` (warn; `transport` `websocket`, `types` count, `last_id` = `session.position()`); `ClientClosed` lingers; `Ended(reason)` writes `close_message(reason)` and, when written, lingers. Lingering is `tokio::time::timeout(CLOSE_TIMEOUT, ..)` around `receive()` until `None`.
4. The function returns; `Drop for Subscriber` frees the slot.

Why: `WebSocket::send` has no deadline and its flush stays pending while the peer reads nothing (`axum-0.8.9/src/extract/ws.rs:560-565`); a page read must not be interrupted by inbound frames, because a cancelled read re-enters the permit queue at its end and a chatty client would stop its own replay; tungstenite writes the reply to a client's close frame only on the next read, hence the lingering (first plan, "The WebSocket pump", measured there). Written as `serve`, `run`, `replay_turn`, `pick`, `live_turn`, `on_inbound`, `on_step`, `send`, `finish`, `linger` with `ControlFlow<Stop>`, each stays under the complexity limits.

Handlers (`http/handler.rs`) and DTOs (`http/dto.rs`):

```rust
#[derive(Debug, Clone, Copy, Deserialize, IntoParams, Validate)]
#[serde(rename_all = "camelCase", default)]
#[into_params(parameter_in = Query)]
pub struct ListEventTypesParams { pub page: i32, pub items_per_page: i32 }   // defaults 1 and 30; validated by PageRequest

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[schema(as = EventType)]
pub struct EventTypeResponse { #[schema(min_length = 1, max_length = 128, pattern = r"^[a-z][a-z0-9]*([._-][a-z0-9]+)*$", example = "user.created")] pub name: String }

/// u64 values as decimal strings (FR-4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = EventStreamStatus)]
pub struct EventStreamStatusResponse {
    pub state: String, pub reason: Option<String>, pub broker_reachable: bool,
    pub position: Option<String>, pub end_offset: Option<String>, pub lag: Option<String>,
    pub window_first: Option<String>, pub window_last: Option<String>,
    pub subscribers: usize, pub rejected_records: String,
}
impl From<NodeStatus> for EventStreamStatusResponse { /* the field mapping above */ }

#[derive(Debug, Deserialize, Validate)] #[serde(rename_all = "camelCase")]
pub struct StreamParams { pub types: String, pub last_event_id: Option<String> }
#[derive(Debug, Deserialize, Validate)] #[serde(rename_all = "camelCase")]
pub struct WsParams { pub types: Option<String>, pub last_event_id: Option<String> }

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "lowercase", deny_unknown_fields)]
pub(crate) enum Command { Subscribe { types: Vec<String> }, Unsubscribe { types: Vec<String> } }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypesError { Empty, Blank, Name(InvalidEvent), TooMany }
/// The one rule for a list of names: first fault in order Blank, Name, TooMany (at the 101st distinct name);
/// Empty when nothing was given; a repeated name counts once.
pub(crate) fn parse_names<'a, I: Iterator<Item = &'a str>>(names: I) -> Result<BTreeSet<EventType>, TypesError>;
/// `parse_names(raw.split(','))`; the error is the 422 problem with one violation on `types`.
pub(crate) fn parse_types(raw: &str) -> Result<BTreeSet<EventType>, Problem>;
/// `Last-Event-ID` wins over `lastEventId`; the error is the 400 problem naming the carrier.
pub(crate) fn parse_cursor(headers: &HeaderMap, query: Option<&str>) -> Result<Option<Cursor>, Problem>;

#[utoipa::path(get, path = "/event-types", operation_id = "api_event_types_get", tag = "EventStream", /* 200, 400, 422, default */)]
pub async fn list_event_types(State(state): State<EventStreamState>, ValidatedQuery(params): ValidatedQuery<ListEventTypesParams>)
    -> Result<Json<Vec<EventTypeResponse>>, Problem>;

#[utoipa::path(get, path = "/events/status", operation_id = "api_events_status_get", tag = "EventStream", /* 200, default */)]
pub async fn event_stream_status(State(state): State<EventStreamState>) -> Json<EventStreamStatusResponse>;

pub async fn stream(State(state): State<EventStreamState>, headers: HeaderMap,
    ValidatedQuery(params): ValidatedQuery<StreamParams>) -> Result<Response, Problem>;

pub async fn ws(State(state): State<EventStreamState>, headers: HeaderMap,
    ValidatedQuery(params): ValidatedQuery<WsParams>,
    upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>) -> Result<Response, Problem>;
```

- Order inside `stream` and `ws` (FR-16): the 403 is answered by the origin middleware before the handler; `ValidatedQuery` rejects before a handler body runs, so the 503 answers come from the middleware `availability` of `http.rs` (`pub(crate) async fn availability(State(hub): State<Arc<Hub>>, request: Request, next: Next) -> Response`), layered on the two streaming routes only. It calls `Hub::check()`, which answers `Closed`, `NotReady`, `AtCapacity` in this order under the registry lock without registering. `register` repeats the same checks under the lock, so a request that passed the middleware in the instant before the cap filled gets the capacity 503 after validation; that race is the only exception to the order.
- `stream`: parse types and cursor, `subscriptions.open(..)` (503 at capacity), then `Sse::new(unfold(subscription, ..)).keep_alive(KeepAlive::new().interval(keep_alive))` with the extra header `x-accel-buffering: no` (axum sets `content-type` and `cache-control`).
- `ws`: parse (422 or 400), `open` (503), then the upgrade result: a rejection becomes `Problem::new(rejection.status()).with_detail(rejection.body_text())`; a success becomes `upgrade.max_message_size(MAX_MESSAGE_BYTES).max_frame_size(MAX_MESSAGE_BYTES).read_buffer_size(READ_BUFFER_BYTES).on_upgrade(move |socket| socket::serve(socket, Session::new(subscription), keep_alive))`, with `MAX_MESSAGE_BYTES = 32_768`, `READ_BUFFER_BYTES = 4_096`.
- `event_stream_status`: `Json(state.status.snapshot().into())`; never 503, answers in every state (FR-4); the availability middleware is not on this route.
- `TypesError` mapping: `Empty` -> violation code `required` / frame `types must hold 1 to 100 event type names`; `Blank` -> `required` / `invalid event type name`; `Name(r)` -> `r.code()` / `invalid event type name`; `TooMany` -> `max_items` / `types must hold 1 to 100 event type names`. Other error frames: `invalid command` (not JSON, unknown action or member, `types` missing), `a connection subscribes to at most 100 event types`, `binary frames are not supported`, `the command is too large` (FR-11).
- The extractor-check test of this crate allows `State<EventStreamState>`, `ValidatedQuery<`, `HeaderMap` and `Result<WebSocketUpgrade, WebSocketUpgradeRejection>`; none can be rejected outside a problem.

> Assumption: the three 503 answers are checked twice: by the `availability` middleware before validation (FR-16's order) and again by `register` under the lock, which alone decides the cap race-free. A request that passes the first check while the last slot is being taken gets the capacity 503 after its parameters were validated; nothing else changes order.

`http.rs`:

```rust
#[derive(Debug, Clone, Copy)]
pub struct StreamSettings { pub keep_alive: Duration }
#[derive(Debug, Clone)]
pub struct EventStreamState { pub query_bus: Arc<dyn QueryBus>, pub subscriptions: Arc<Subscriptions>,
                              pub status: Arc<StatusSource>, pub hub: Arc<Hub>, pub settings: StreamSettings }
pub fn router<S>() -> OpenApiRouter<S> where S: Clone + Send + Sync + 'static, EventStreamState: FromRef<S> {
    // routes!(handler::list_event_types), routes!(handler::event_stream_status),
    // .route("/events/stream", get(handler::stream)), .route("/events/ws", get(handler::ws)),
    // the two undocumented routes wrapped in `availability`
}
```

GraphQL (`graphql.rs`): `#[graphql(name = "EventType")] struct EventTypeObject { name: String }`; `#[graphql(name = "EventStreamStatus")] struct EventStreamStatusObject` with `state: String`, `reason: Option<String>`, `broker_reachable: bool`, `position`, `end_offset`, `lag`, `window_first`, `window_last: Option<String>`, `subscribers: i32` (`i32::try_from(n).unwrap_or(i32::MAX)`), `rejected_records: String`; `EventStreamQuery::event_types(ctx, #[graphql(default = 1)] page: i32, #[graphql(default = 30)] items_per_page: i32) -> async_graphql::Result<Vec<EventTypeObject>>` through `handler::list_types` and `problem_to_error`; `EventStreamQuery::event_stream_status(ctx) -> EventStreamStatusObject` from `StatusSource::snapshot`. SDL: `eventTypes(itemsPerPage: Int! = 30, page: Int! = 1): [EventType!]!` and `eventStreamStatus: EventStreamStatus!`. Descriptions avoid the words `node`, `Connection`, `update`, `delete` (existing SDL test).

### Infrastructure crate

Manifest: `async-trait`, `event-stream-domain`, `krafka`, `shared-domain`, `thiserror`, `tokio` (feature `time`), `tracing`; dev `krafka` with `test-broker`, `rstest`, `test-support`, `tokio` with `test-util`.

| File under `crates/notification/event-stream/infrastructure/src/` | Content |
| --- | --- |
| `lib.rs` | modules, re-exports |
| `kafka.rs` | `KafkaSettings`, `KafkaSecurity`, `SaslMechanism`, `SaslCredentials`, `KafkaTopic`, `CLIENT_ID`, private `connect_consumer`, `map_error` |
| `kafka/probe.rs` | `KafkaProbe` (implements `TopicProbe`) |
| `kafka/tail.rs` | `KafkaTail` (implements `TopicTail`) |
| `kafka/replay.rs` | `KafkaReplay` (implements `TopicReplay`) |
| `tests/integration/{main.rs,support.rs,real_broker.rs,fake_broker.rs}` | adapter tests |

```rust
pub const CLIENT_ID: &str = "notification-service";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaslMechanism { Plain, ScramSha256, ScramSha512 }
#[derive(Clone, PartialEq, Eq)]
pub struct SaslCredentials { pub mechanism: SaslMechanism, pub username: String, pub password: String }
// Debug by hand: the password is printed as "<redacted>".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KafkaSecurity { Plaintext, Ssl, SaslPlaintext(SaslCredentials), SaslSsl(SaslCredentials) }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KafkaSettings { pub bootstrap_servers: String, pub topic: String, pub security: KafkaSecurity, pub request_timeout: Duration }

/// Factory; builds no connection.
#[derive(Debug, Clone)]
pub struct KafkaTopic { settings: Arc<KafkaSettings> }
impl KafkaTopic {
    pub fn new(settings: KafkaSettings) -> Self;
    pub fn probe(&self) -> KafkaProbe;
    pub fn tail(&self) -> KafkaTail;
    pub fn replay(&self) -> KafkaReplay;
}
```

Which client call implements which port method (every consumer of the adapter is created by `connect_consumer` on a `Kafka` handle of its own):

| Port method | krafka calls | Time limit |
| --- | --- | --- |
| (lazy connect, before the first call of each adapter and after a failed call) | `Kafka::builder(bootstrap).client_id(CLIENT_ID).security(auth).request_timeout(t).connect_timeout(t).connect()`, then `consumer_without_group()` with the settings below, `build()`, `assign(topic, vec![0])` | `t` around `connect` |
| `TopicProbe::partition_count` | `consumer.fetch_metadata(Some(topic))`: no topic -> `None`, else `partition_count()` as `u32` | `t` |
| `TopicProbe::watermarks` | `consumer.fetch_watermarks(topic, 0)` -> `(low, high)` -> `Watermarks { earliest: low, end: high }` | `t` |
| `TopicTail::seek` | `consumer.seek(topic, 0, position)` (local) | `t` |
| `TopicTail::fetch` | `consumer.poll(TailEvents::WAIT)`, then `consumer.position(topic, 0)` | `t + WAIT` |
| `TopicReplay::page` | take a free reader, `seek(topic, 0, from)`, `poll(REPLAY_WAIT)`, `position`; give the reader back | `t + REPLAY_WAIT` |

Consumer settings, each with its reason: `isolation_level(IsolationLevel::ReadCommitted)` (D9; the default delivers aborted records); `auto_offset_reset(AutoOffsetReset::None)` (the default `Latest` skips to the end without an error); `metrics_push(false)` (no KIP-714 telemetry to the broker); `fetch_min_bytes(1)`; `fetch_max_wait(TailEvents::WAIT)`; `max_poll_records(500)`, `max_buffered_records(500)`, `max_partition_fetch_bytes(1_048_576)`, `fetch_max_bytes(1_048_576)` (one replay page, soft bound); `allow_auto_create_topics` left false. Handle settings: `client_id("notification-service")` (default `krafka`); `request_timeout` and `connect_timeout` both `KAFKA_REQUEST_TIMEOUT` (defaults 30 s and 10 s would make one poll block 30 s). Security: `Plaintext` -> `AuthConfig::plaintext()`; `Ssl` -> `AuthConfig::ssl(TlsConfig::new())`; `SaslPlaintext(c)` -> `sasl_plain`, `sasl_scram_sha256` or `sasl_scram_sha512` by mechanism; `SaslSsl(c)` -> the same `.with_tls(TlsConfig::new())`.

Conversions and errors (`map_error`):

- `i64` offsets go through `u64::try_from`; a negative value is `Unavailable`. A record's value `Option<Bytes>` becomes `Option<Vec<u8>>`; key and headers are dropped.
- `matches!(error, KrafkaError::NoOffset { .. })`: the adapter reads `fetch_watermarks` on the same consumer and returns `BelowEarliest { earliest: low }` (a failing read: `Unavailable`).
- `matches!(error, KrafkaError::Protocol { .. })` on `poll`: `Undecodable { at: position or the requested position }`.
- Everything else, and an elapsed `tokio::time::timeout`: `Unavailable(Box::new(error))`. After `Unavailable` the adapter drops its consumer and handle; the next call connects again (one attempt; retries belong to the application).
- `Batch`: records with an offset below the requested position are dropped; `next = max(requested, position(), last offset + 1)` (rule 1 of the ports).

`KafkaTail` keeps `position: Cursor` (set by `seek`, moved to `batch.next` after a fetch) so that a reconnect re-seeks to it. `KafkaReplay` keeps `std::sync::Mutex<Vec<Reader>>`; a call pops a reader or creates an unconnected one, and pushes it back after a successful call; a failed reader is dropped. The application's semaphore of 4 permits bounds the readers to 4 (FR-14). `REPLAY_WAIT` is 100 ms. Logic sits in inherent methods; the `#[async_trait]` bodies only delegate.

> Assumption: `tokio` joins the infrastructure allow-list as a normal dependency (protected change), because the adapter must wrap every client call in its own time-out: `poll` is bounded by the request time-out and not by its argument (probe 09). The first plan kept tokio out of infrastructure; that rule cannot hold for this client.
>
> Assumption: a reader is not closed explicitly when it is dropped; the client then logs a warning, which the service's log filter hides (`krafka=error`, "Composition Root"). A `close` method on the ports would add a call that every fake and test must honour for no behaviour a client can see.

## Composition Root

Everything that wires, schedules, supervises and serves is in `crates/service` (external `*`): configuration, adapters, the readiness probe of the event stream, the origin rules, the tail loop and its supervision, the serve sequence with the transport bound, and the shutdown.

### Configuration

Each variable is an `Option<String>` clap argument of `ServeArgs` (and of `check-config`) with `env = "<NAME>"` and `hide_env_values = true`; `config.rs` collects every problem with `collector.collect("NAME", parser(..))` in two new group functions, `kafka_settings(&mut Collector, &ServeArgs) -> Option<KafkaSettings>` and `stream_settings(&mut Collector, &ServeArgs) -> Option<StreamConfig>`, plus `allowed_origins(..)`; the order of problems is the order of the table; cross-field rules come last. An invalid value is reported `<VARIABLE>: <reason>` without the value, exit status 2 (`check-config` on stderr, `serve` as one `invalid configuration` entry per problem).

| Variable | Parsed into | Default | Rule (reason text) |
| --- | --- | --- | --- |
| `KAFKA_BOOTSTRAP_SERVERS` | `KafkaSettings::bootstrap_servers` (validated, joined with `,`) | `localhost:9092` | 1 to 16 entries `host:port`; host a DNS name, an IPv4 address or a bracketed IPv6 address; port 1 to 65 535; no empty entry (`must be a comma-separated list of host:port addresses`) |
| `KAFKA_TOPIC` | `KafkaSettings::topic` | `notifications` | 1 to 249 of `A-Za-z0-9._-`, not `.` or `..` (`must be a Kafka topic name`) |
| `KAFKA_SECURITY_PROTOCOL` | `KafkaSecurity` with the three below | `plaintext` | `plaintext`, `ssl`, `sasl_plaintext`, `sasl_ssl` (`must be one of plaintext, ssl, sasl_plaintext, sasl_ssl`) |
| `KAFKA_SASL_MECHANISM` | `SaslMechanism` | unset | `PLAIN`, `SCRAM-SHA-256`, `SCRAM-SHA-512`; cross-field rule below |
| `KAFKA_SASL_USERNAME` | `SaslCredentials::username` | unset | not empty; cross-field rule below |
| `KAFKA_SASL_PASSWORD` | `SaslCredentials::password` | unset | not empty; cross-field rule below; never logged |
| `KAFKA_REQUEST_TIMEOUT` | `KafkaSettings::request_timeout` | `5s` | `1s` to `1m` (`must be a duration from 1s to 1m`) |
| `EVENT_TYPES` | `EventCatalog` | unset (empty) | 1 to 1 000 distinct valid names, no empty value or entry (`must be a comma-separated list of 1 to 1000 event type names`) |
| `EVENT_MAX_DATA_BYTES` | `usize` | `65536` | 1 to 1 048 576 |
| `EVENT_WINDOW_BYTES` | `usize` | `33554432` | 1 048 576 to 1 073 741 824 |
| `EVENT_WINDOW_PRELOAD` | `u64` | `10000` | 0 to 1 000 000 |
| `EVENT_REPLAY_MAX_OFFSETS` | `u64` | `1000000` | 0 to 100 000 000 |
| `SUBSCRIBER_BUFFER` | `usize` | `256` | 1 to 65 536 |
| `MAX_SUBSCRIBERS` | `usize` | `100000` | 1 to 1 000 000 |
| `STREAM_KEEP_ALIVE` | `Duration` | `15s` | `1s` to `5m` |
| `CORS_ALLOWED_ORIGINS` | `AllowedOrigins` | unset (`NoOrigin`) | exactly `*`, or origins as a browser sends them (scheme `http` or `https`, host, optional port, lower case, nothing else) (`must be * or a comma-separated list of origins such as https://app.example.com`) |

Integers use the existing reason `must be an integer from <a> to <b>`. Cross-field rule: with `KAFKA_SECURITY_PROTOCOL` `sasl_plaintext` or `sasl_ssl` the three SASL variables are each `is required when KAFKA_SECURITY_PROTOCOL is sasl_plaintext or sasl_ssl`; otherwise each that is set is `must not be set unless KAFKA_SECURITY_PROTOCOL is sasl_plaintext or sasl_ssl`. The rule runs in its own function, `sasl_rule`, because `cross_field_rules` already has five parameters.

`Config` gains `kafka: KafkaSettings`, `streams: StreamConfig { catalog: EventCatalog, max_data_bytes, window_bytes, preload, replay_max_offsets, buffer, max_subscribers, keep_alive }` and `allowed_origins: AllowedOrigins`. The removal stories delete `PostgresSettings`, `MigrateSettings`, the `DATABASE_*` parsing and the rule `HEALTH_PROBE_TIMEOUT >= DATABASE_ACQUIRE_TIMEOUT` ("Removal of PostgreSQL").

`.env` gets one line per variable with a default, written with that default, except `KAFKA_BOOTSTRAP_SERVERS=localhost:9095` (the host listener of the local broker, "Local Stack and Load Test"); `EVENT_TYPES`, the three `KAFKA_SASL_*` and `CORS_ALLOWED_ORIGINS` get none. `docs/environment.md` gets a row per variable (docs-check rule 5). The tests that pin the variable set change with the variables: `config/tests.rs` `FIELDS`, `cli/tests.rs`, `tests/integration/check_config.rs` `FULL_ENVIRONMENT`, `tests/integration/support.rs` `VALID_ENVIRONMENT`.

> Assumption: `.env` names the host listener `localhost:9095` and not the default `localhost:9092`. The test targets run on the host and export `.env` and `.env.test`; in container mode the app container's own `KAFKA_BOOTSTRAP_SERVERS=kafka:9092` from compose wins, because `scripts/with-env.sh` never changes a variable the caller already has.

### Adapters and wiring

```rust
// bootstrap.rs
pub struct Adapters { /* existing fields */, pub topic: KafkaTopic }
// Adapters::connect(&Config) builds `KafkaTopic::new(config.kafka.clone())`: no network access (pinned test).

pub struct Application { pub router: Router, pub background: Background }
pub fn application(adapters: Adapters, config: &Config) -> Result<Application, BootstrapError>;
pub fn router(adapters: Adapters, config: &Config) -> Result<Router, BootstrapError>;   // application(..)?.router
```

`application` does, besides today's wiring (in two private functions so that it stays under 50 lines):

1. `let window = Arc::new(Window::new(streams.window_bytes));`
2. `let hub = Hub::new(HubSettings { max_subscribers, buffer });`
3. `let (tail, tail_state) = TailEvents::new(Arc::new(topic.probe()), Box::new(topic.tail()), Arc::clone(&window), hub.clone() as Arc<dyn EventPublisher>, TailSettings { max_data_bytes, preload, request_timeout });`
4. `let subscriptions = Arc::new(Subscriptions::new(hub.clone(), window.clone(), Arc::new(topic.replay()), tail_state.clone(), ReplaySettings { max_offsets, max_data_bytes, request_timeout }));`
5. `let status = Arc::new(StatusSource::new(tail_state.clone(), window, hub.clone()));`
6. `register_queries` also registers `ListEventTypesHandler::new(Arc::new(streams.catalog.clone()))`;
7. `AppState` gains `event_stream: EventStreamState { query_bus, subscriptions, status, hub: hub.clone(), settings: StreamSettings { keep_alive } }` (derive `FromRef`); the GraphQL schema gets `.data(event_stream.clone())`;
8. the health probes get `EventStreamProbe::new(tail_state.clone())` after the existing ones;
9. `Background { tail, hub }`.

`bootstrap::router` drops the `Background`; the hub of such a router is never started, so the streaming routes answer the 503 `the node is starting` while the catalog, the status and every existing route work. Tests that subscribe build an `Application` and drive the tail with fakes.

Routes: `openapi::api_router()` adds `.nest("/api", event_stream_application::http::router())`; `ApiDoc` gains the tag `EventStream` ("Event catalog and event stream status endpoints."). GraphQL: `pub struct Query(ExampleApiResourceQuery, EventStreamQuery);` until the removal, then `pub struct Query(EventStreamQuery);` with `EmptyMutation`. The pinning tests `src/openapi/tests.rs` and `src/graphql/schema/tests.rs` and the three `Adapters { .. }` literals (`src/bootstrap/tests.rs`, `src/http/router/tests.rs`, `tests/integration/panic.rs`) change with them.

### Readiness probe of the event stream

The health-check domain gains `Dependency::EventStream` (identifier `event_stream`, failure message `Event stream is not ready`). The probe is implemented in the composition root, because `event-stream` and `health-check` may not depend on each other:

```rust
// crates/service/src/event_stream_probe.rs
#[derive(Debug)]
pub struct EventStreamProbe { tail: watch::Receiver<TailState> }
impl EventStreamProbe { pub const fn new(tail: watch::Receiver<TailState>) -> Self; }
#[async_trait] impl HealthProbe for EventStreamProbe {
    fn dependency(&self) -> Dependency { Dependency::EventStream }
    async fn check(&self) -> Result<(), BoxError>;   // Ok when `started`, else Err("the event stream is starting")
}
```

It reads no broker. It fails while the node is starting and passes for the rest of the process (FR-20): a broker outage after the start does not fail it (NFR-6). The cache and message-broker probes stay; the database probe leaves with the removal.

### Browser origins

Unchanged from the first plan (FR-28):

```rust
// config.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllowedOrigins { NoOrigin, Any, List(Vec<HeaderValue>) }
impl AllowedOrigins { pub fn allows(&self, origin: &HeaderValue) -> bool; }   // NoOrigin false, Any true, List byte for byte

// http/origin.rs (new)
pub const PREFLIGHT_MAX_AGE: Duration = Duration::from_secs(600);
/// `None` for NoOrigin: the router then has no CORS layer at all.
pub fn cors(origins: &AllowedOrigins) -> Option<CorsLayer>;
/// The 403 problem `the origin is not allowed` for a handshake (it has `sec-websocket-key`) whose
/// `origin` is present and not allowed; otherwise `next.run(request)`.
pub(crate) async fn websocket_origin(State(origins): State<Arc<AllowedOrigins>>, request: Request, next: Next) -> Response;
```

- `cors`: `CorsLayer::new().allow_origin(AllowOrigin::any() | AllowOrigin::list(..)).allow_methods([GET, POST]).allow_headers([CONTENT_TYPE, "last-event-id"]).max_age(PREFLIGHT_MAX_AGE)`; no credentials, nothing exposed.
- Order in `http::router::assemble`, outermost first: request id (three layers), security headers, `TraceLayer`, `.option_layer(origin::cors(..))`, `from_fn_with_state(.., origin::websocket_origin)`, `CatchPanicLayer`, request budget, body limit. Between `TraceLayer` and `CatchPanicLayer` every answer of an allowed origin carries the header, the 404, 405, 500 and 503 problems and the SSE head included (first plan, run there).
- The layer is absent, not empty, when no origin is allowed: an installed `CorsLayer` answers every `OPTIONS` with 200 and adds `vary: origin`, which would change existing answers (NFR-21).
- `websocket_origin` is `pub(crate)`, not a handler, so the handler-file test of `crates/service/src/http` is unchanged.

### Telemetry

`telemetry::init` adds the directive `krafka=error` to the operator's `LOG_LEVEL` directives (`EnvFilter::add_directive`).

> Assumption: the client's own log is limited to errors. A missing topic makes `krafka::metadata` write about 10 warnings per second, a dropped consumer writes a warning, and the spawned-binary tests compare the whole log of an idle node (`tests/integration/serve.rs:128-148`); the service's own entries (NFR-15) report every state an operator needs.

### Background task, supervision and shutdown

`crates/service/src/background.rs` (new, declared in `lib.rs`) loops, supervises and stops; the use case is `TailEvents` of the application crate.

```rust
/// How long the shutdown waits for the streams to end after the hub was closed (FR-27).
pub const STREAM_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);
pub const RETRY_FIRST: Duration = Duration::from_secs(1);
pub const RETRY_MAX: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub struct Background { pub tail: TailEvents, pub hub: Arc<Hub> }
impl Background { #[must_use] pub fn spawn(self) -> Running; }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause { Signal(&'static str), TaskFailed(&'static str) }
impl Cause { pub const fn result(self) -> Result<(), BackgroundTaskFailed>; }
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("background task {task} failed")]
pub struct BackgroundTaskFailed { pub task: &'static str }

#[derive(Debug)]
pub struct Running { /* stop: watch::Sender<bool>, tasks: JoinSet<Ended>, hub: Arc<Hub> */ }
impl Running {
    /// The signal or a dead loop, whichever is first (the signal when both are ready); writes the entry of the cause.
    pub async fn stopped<S: Future<Output = &'static str> + Send>(&mut self, signal: S) -> Cause;
    /// The shutdown up to the HTTP drain: the loop, the hub, the streams.
    pub async fn shutdown(self);
}

// private
type Stop = watch::Receiver<bool>;
type Ended = (&'static str, Result<(), JoinError>);
async fn until_stopped<F: Future + Send>(stop: &mut Stop, work: F) -> Option<F::Output>;   // biased select, stop first
fn start<F: Future<Output = ()> + Send + 'static>(tasks: &mut JoinSet<Ended>, name: &'static str, work: F);
/// Next pause: 1 s after a success, doubled after each failure, at most 30 s.
const fn next_pause(previous: Option<Duration>) -> Duration;

async fn tail(mut tail: TailEvents, mut stop: Stop) {
    let mut pause = None;
    loop {
        let limit = tail.round_limit();
        let Some(outcome) = until_stopped(&mut stop, tokio::time::timeout(limit, tail.round())).await else { return; };
        pause = after_round(outcome, pause);       // Ok(Ok(())) -> None; a failure or an elapsed limit -> Some(next_pause(pause))
        if let Some(wait) = pause {
            if until_stopped(&mut stop, tokio::time::sleep(wait)).await.is_none() { return; }
        }
    }
}
```

Rules (the first plan's, reduced to one loop):

- **The stop flag is read in one place**, `until_stopped`, the first arm of a biased `select!`. No `while running(..)` and no pause helper returning `bool` (a mutant would leave a loop without an await and the test would time out). A dropped `Running` ends the loop because the sender is gone.
- **Every round has an await no mutant can remove**: the round and the pause are awaited in the loop body; `after_round` only decides.
- **A round cannot hang**: `tokio::time::timeout(round_limit, ..)` cuts it; a cut round is a failed round (`event tail failed` when it is the first failure after a success). Cutting is safe: `TailEvents` stores its position after every batch, and the next round continues from `P` (FR-26).
- **A failed round is not a failed loop**: the loop never returns an error; the pause grows 1 s, 2 s, 4 s ... 30 s and returns to 1 s after a success (FR-26). The pause after a success is zero; the long poll is the wait.
- **Supervision**: `Running::failed` is `while let Some(joined) = self.tasks.join_next().await { if let Ok((name, Err(_panic))) = joined { return name; } }` then `std::future::pending().await`. The loop task is spawned inside an outer task that reports its name. A panic of the tail task gives `Cause::TaskFailed("tail")`, the entry `background task failed` (error, `task`), the same shutdown sequence as a signal, and exit status 1 (FR-26).

`serve` (`command/serve.rs`) stays one straight path:

```rust
async fn serve(config: &Config) -> Result<Shutdown, BoxError> {
    let signals = Signals::install()?;
    let adapters = Adapters::connect(config).await?;
    let application = bootstrap::application(adapters, config)?;
    let listener = bind(config).await?;
    log_listening(listener.local_addr()?, mode(config.app_env));
    let keep_alive = config.streams.keep_alive;
    let listener = listener.tap_io(move |stream| bound_stalled_peer(stream, keep_alive));
    let (stop, stopping) = watch::channel(false);
    let server = tokio::spawn(axum::serve(listener, application.router).with_graceful_shutdown(stopped(stopping)).into_future());
    let mut running = application.background.spawn();
    let cause = running.stopped(signals.received()).await;
    stop.send_replace(true);
    match tokio::time::timeout(config.shutdown_grace_period, drain(running, server)).await {
        Ok(joined) => { joined??; cause.result()?; log_stopped(); Ok(Shutdown::Drained) }
        Err(_elapsed) => { log_forced(config.shutdown_grace_period); Ok(Shutdown::Forced) }
    }
}
async fn drain(running: Running, server: JoinHandle<std::io::Result<()>>) -> Result<std::io::Result<()>, JoinError> {
    running.shutdown().await;
    server.await
}
#[cfg(target_os = "linux")]
fn bound_stalled_peer<S: AsFd>(socket: &S, limit: Duration) { drop(socket2::SockRef::from(socket).set_tcp_user_timeout(Some(limit))); }
#[cfg(not(target_os = "linux"))]
fn bound_stalled_peer<S: AsFd>(_socket: &S, _limit: Duration) {}
```

Order and why:

1. The loop is spawned after `bind` and `log_listening`: `listening` stays the first entry (existing tests read `lines[0]`), and a failed bind writes only `serve failed`.
2. `running.stopped(..)` returns at the signal or at a dead loop.
3. `stop.send_replace(true)` at once: the listener closes and new connections are refused. Responses being written, SSE streams included, go on; an upgraded WebSocket is a detached task the flag does not reach.
4. `running.shutdown()`, inside `SHUTDOWN_GRACE_PERIOD` (FR-27):
   1. set the stop flag and join the loop: the round in flight is cancelled at once, also a broker call to a broker that does not answer (`poll` is cancel safe; the adapter future is dropped). Replay reads end with their streams in step 2;
   2. `hub.close()`: every SSE response ends after its queued frames, every WebSocket pump writes its queued frames and close code 1001; a replay that waits for a permit or a page ends at its next step with `End(HubClosed)`; from now on a subscription request gets the 503 `the node is shutting down`;
   3. `tokio::time::timeout(STREAM_DRAIN_TIMEOUT, hub.drained())`, either outcome returns;
5. `server.await`: the HTTP drain as today, then `stopped`, or `grace period elapsed, forcing shutdown` and exit status 1.

Exit statuses (NFR-21): 0 after a signal when everything ended inside the grace period; 1 when the grace period ended first, when the tail loop died (`background task failed`, then `serve failed`), and for a bind or runtime failure; 2 for an invalid configuration. An idle node stops within milliseconds, also with the broker down, because the round in flight is cancelled and nothing is committed.

Step 4.3 exists because the drain of `axum::serve` does not wait for upgraded WebSockets (`axum-0.8.9/src/extract/ws.rs:359-376`); without it, close code 1001 races with the drop of the runtime (first plan, measured there with 2 000 sockets).

**The transport bound.** `tap_io` runs `bound_stalled_peer` on every accepted stream; it sets `TCP_USER_TIMEOUT` to `STREAM_KEEP_ALIVE` through `socket2::SockRef`. The kernel then ends a connection whose peer leaves sent data unacknowledged, or announces a zero window, for longer than that: a stalled HTTP/1.1 client of either streaming route is ended, its body or pump is dropped and `Drop for Subscriber` frees the slot (FR-24). A healthy idle or slow-reading client is not affected (the limit counts only while the kernel waits for the peer). Not covered: an HTTP/2 stream stalled by its own flow control (PRD, out of scope). With a stalled SSE client that stalled less than `STREAM_KEEP_ALIVE` before the signal, the HTTP drain waits for the kernel, so a deployment that needs exit status 0 then sets `STREAM_KEEP_ALIVE` below `SHUTDOWN_GRACE_PERIOD` (FR-27). The result of the call is dropped: no test could reach a branch on its error. The non-Linux form has an empty body, which cargo-mutants does not mutate and coverage does not see. A unit test in `command/serve/tests.rs` reads the option of an unconnected `tokio::net::TcpSocket::new_v4()` before (`None`) and after (`Some(limit)`) the helper, for 15 s and 1 500 ms.

### Log entries (NFR-15)

| Message | Level | Fields | Written by |
| --- | --- | --- | --- |
| `event tail ready` | info | `position` | `TailEvents`, once, when `position >= goal` the first time |
| `event topic invalid` | error | `reason` (`topic-not-found`, `partition-count`) | `TailEvents`, once per change of the fault, during start-up |
| `event tail failed` | warn | `error` (chain) | `TailEvents` or the loop (a cut round), at the first failed round after a successful one |
| `event tail recovered` | info | `position` | `TailEvents`, at the first successful round after a logged failure |
| `event tail behind retention` | warn | `from`, `to` | `TailEvents`, when the tail position fell below the log start and jumped to it |
| `event record rejected` | warn | `offset`, `reason` | `TailEvents`, for an invalid record or an undecodable batch |
| `replay failed` | warn | `transport`, `error` (chain) | the SSE body or the WebSocket pump, when a stream ends with `TopicFailed` |
| `subscribers fell behind` | warn | `count` | `TailEvents`, once per batch in which at least one subscriber fell behind |
| `subscriber evicted` | warn | `transport`, `types`, `last_id` | `socket::serve`, when a write misses its deadline (best effort) |
| `subscription rejected` | warn | `detail` | `From<HubError> for Problem` |
| `background task failed` | error | `task` (`tail`) | `Running::stopped` |
| `shutdown signal received` | info | `signal` | `Running::stopped` (the existing entry, moved) |

Failures before the first successful round are not logged: a node that cannot reach its broker at start writes nothing but `listening` and shows `broker-unreachable` on its status route and a failing readiness probe; `event topic invalid` is written as soon as the broker answers with a bad topic. This keeps the log of an idle node without a broker exactly `listening`, `shutdown signal received`, `stopped`, which the spawned-binary tests compare (`tests/integration/serve.rs:128-148`).

> Assumption: the entry `event tail behind retention` is added to the table of NFR-15. Retention that overtakes the tail loses events for every client of the node, and the PRD's entries do not cover it; NFR-15 lists what must be written, not everything that may be.
>
> Assumption: `TailState::started` is not affected by a broker outage, and nothing is logged for failures before the first success. NFR-15 defines `event tail failed` as "the first failed or cut round after a success".

Files of the template sync that this change touches and that are not yet in `.templatesyncignore`: `crates/service/src/lib.rs`, `src/command/serve.rs`, `src/http/router.rs`, `src/telemetry.rs`, `tests/integration/main.rs`, `tests/integration/support.rs`, `tests/integration/check_config.rs`, `docker-compose.yml`, `docker-compose.override.yml`, `docker-compose.load_test.override.yml`, `docker-compose.prod.yml`, `docs/architecture.md`, `docs/operations.md`, `docs/load-testing.md`, `docs/README.md`, `SECURITY.md`, `crates/internal/health-check/domain/src/value_object/dependency.rs` (already ignored per `.templatesyncignore:33-35`, so no entry). Each goes into `.templatesyncignore` by full path in the story that first changes it; the removal stories add their own (section "Removal of PostgreSQL").

## Wire Protocols

The normative text for clients is `docs/streaming-protocol.md` (new, FR-38); this section is its source. `docs/requests.http` gets one example per new route. The cursor of every event is its Kafka offset plus 1, in decimal, and is the same on every node (D3).

### Subscription request (both streaming routes)

- `types`: comma-separated event type names; required on `/api/events/stream`, optional on `/api/events/ws`; 1 to 100 distinct names (a repeated name counts once; 0 on the WebSocket route when the parameter is absent).
- Cursor: header `Last-Event-ID` or query parameter `lastEventId`; the header wins; a decimal `u64`. It means "every id up to this one was seen". `0` asks for every retained event. A cursor ahead of the node's tail, also ahead of the topic end, is accepted and waits (FR-15).
- Answers before a stream opens, in this order of checks (FR-16):

  | Status | When |
  | --- | --- |
  | 403, detail `the origin is not allowed` | WebSocket route only: a handshake whose `Origin` is not allowed by `CORS_ALLOWED_ORIGINS`; no subscriber is registered |
  | 503, detail `the node is shutting down` | after the hub was closed |
  | 503, detail `the node is starting` | the node has not yet reached the end of the topic it read at start, or cannot reach its broker or topic at start |
  | 503, detail `the node is at its subscriber capacity` | `MAX_SUBSCRIBERS` connections are open |
  | 400 | `types` missing on the SSE route; a cursor that is not a decimal `u64` (the detail names the carrier) |
  | 422 | a violation on `types` (codes `required`, `length`, `pattern`, `max_items`) |
  | 4xx of the upgrade, as a problem | WebSocket route: a request that is not a valid upgrade |

- No answer depends on a broker read. A subscription with a cursor below the window is accepted while Kafka cannot be read; its stream then ends (SSE: the response ends; WebSocket: close code 1011).
- A 503 is safe to retry on any node. A browser `EventSource` does not retry it by itself (it fails the connection for every status but 200), so the page creates a new `EventSource` and passes `lastEventId`.

### SSE frames

Response head: `200`, `content-type: text/event-stream`, `cache-control: no-cache`, `x-accel-buffering: no`. One frame per event:

```text
id: 1043
event: user.created
data: {"id":"8f14e45f-ceea-467f-a8f4-3d1b5c4e1a2b","type":"user.created","occurredAt":"2026-11-14T10:00:00Z","data":{"userId":"5f2c"}}

```

- `id` is the cursor (offset + 1), `event` the type, `data` the event object of FR-18 as one line of compact JSON (compact JSON escapes every line break inside strings).
- Keep-alive: the comment `:` and an empty line after `STREAM_KEEP_ALIVE` without a frame; it does not change the client's last id.
- Ids of one stream strictly increase, also after the connection fell behind and caught up. Ids have gaps (markers, aborted and invalid records, other types); a gap is not a loss.
- The stream ends without a final frame at shutdown and when a replay cannot read Kafka. It is never ended because the client is slow. A client whose connection accepts no data for `STREAM_KEEP_ALIVE` loses it (the transport bound) without an end of stream and resumes with its last id.

### WebSocket frames and commands

Server to client, one text frame per event; `id` is a string, `data` the event object:

```json
{"id":"1043","type":"user.created","data":{"id":"8f14e45f-ceea-467f-a8f4-3d1b5c4e1a2b","type":"user.created","occurredAt":"2026-11-14T10:00:00Z","data":{"userId":"5f2c"}}}
```

Client to server: `{"action":"subscribe","types":["user.created","order.paid"]}` and `{"action":"unsubscribe","types":["order.paid"]}`. A valid command has no answer. An invalid one gets one frame `{"error":"<reason>"}`, the set stays as it was and the connection stays open:

| Cause | `error` text |
| --- | --- |
| not JSON, unknown `action`, unknown member, `types` missing | `invalid command` |
| `types` empty or more than 100 distinct names | `types must hold 1 to 100 event type names` |
| a name that is not an event type, an empty name included | `invalid event type name` |
| the set would exceed 100 types | `a connection subscribes to at most 100 event types` |
| a binary frame | `binary frames are not supported` |
| a text frame above 16 384 bytes | `the command is too large` |

Checks run in this order: size, JSON and shape (`Json::<Command>::from_bytes`), names (`parse_names`), the limit of the set. A message or single frame above 32 768 bytes is not answered and ends the connection without a close frame; a frame header declaring more is refused before its payload is buffered.

- **Run-time subscribe**: the connection gets events of an added type with an id above the hub position at the command and above its own floor; it does not replay, also not when the connection later falls behind (FR-10).
- **Unsubscribe**: no later frame of the removed types is written, also not one already queued or read from the window or Kafka.
- **When commands are read**: between two reads of the connection, never during a Kafka page read; a command sent during a replay takes effect for everything not yet written.
- **Ping** every `STREAM_KEEP_ALIVE`, also during a replay; a client ping is answered with a pong at the next read.
- **Write deadline**: a message the socket does not accept within `STREAM_KEEP_ALIVE` ends the connection without a close frame; the transport bound can end it first.
- **Close codes**: 1001 at shutdown after the queued frames; 1011 when a replay cannot read Kafka; after its close frame the server waits up to 1 s for the client's. A client close frame is answered and ends the subscription.

### Event object and duplicates

The `data` of an SSE frame and of a WebSocket frame is the same object: `{"id":"<uuid, lower case>","type":"<type>","occurredAt":"<as written>","data":<producer data>}` (FR-18). The service removes no duplicates: an event a producer wrote twice arrives twice, with the same envelope `id` and two cursor ids; the client deduplicates by the envelope `id` (D8, FR-23). Cursors become invalid when the topic is deleted and recreated; a client then receives nothing until the new topic passes its cursor (FR-15), which the protocol document tells operators and clients.

### Falling behind, reconnecting and browser access

- **Falling behind**: a connection more than `SUBSCRIBER_BUFFER` events behind is not closed; it gets its queued frames, then what it missed from the window or Kafka, then live events; no loss within the replay reach, no duplicate, the same order.
- **Reconnecting**: after an ended stream, close code 1001 or 1011, or a lost connection, the client reconnects to any node with the last id it received, after a delay that grows and has a random part. 400, 403 and 422 are not retried; 503 is.
- **Replay reach**: a resume reaches back to the earliest retained offset of the topic and at most `EVENT_REPLAY_MAX_OFFSETS` offsets below the serving node's tail; older events are skipped without a signal (FR-14, FR-23).
- **Browser access** (FR-28): a page of another origin can read the catalog, the status, GraphQL and an SSE stream only when its origin is in `CORS_ALLOWED_ORIGINS` or the value is `*`; a WebSocket handshake carrying `Origin` is accepted only for an allowed origin, and with the variable unset never; a request without `Origin` is never restricted. The list is a rule for browsers, not an authorization.

## Algorithms

Notation: `P` the tail position (next offset to read; equal to the id of the last offset passed), `E` the end and `L` the earliest retained offset of the last successful watermark read, `F` the window floor, `p` the hub position at the arming of a subscriber, `c` the cursor of a request. `MAX` is `EVENT_REPLAY_MAX_OFFSETS`, `PRELOAD` is `EVENT_WINDOW_PRELOAD`.

### 1. Start position and readiness (`TailEvents::round` before the first seek)

1. `probe.partition_count()`. `Err`: fault `BrokerUnreachable`, `broker_reachable = false`, the round failed. `Ok(None)`: fault `TopicNotFound`. `Ok(Some(n))` with `n != 1`: fault `PartitionCount`. Each of the last two writes `event topic invalid` when the fault differs from the last one and fails the round. `Ok(Some(1))`: continue.
2. `probe.watermarks()`. `Err`: as the first case of step 1. `Ok(w)`: `broker_reachable = true`, `earliest = w.earliest`, `end = w.end`.
3. `start = max(w.earliest, w.end.rewound_by(PRELOAD))`; `tail.seek(start)` (`Err`: as step 1); `window.restart(start)`; `P = start`; `goal = w.end` (fixed for the life of the process); fault cleared; the observation clock starts.
4. When `P >= goal` (also at once for an empty topic or `PRELOAD = 0` on a topic whose end is its start): **ready**: `publisher.started(P)`, `started = true`, log `event tail ready`.
5. Publish `TailState`. Until ready, the tail fills the window only: records are pushed to the window and not published, and `Hub::check` answers `NotReady`, so no subscription can miss an event written around the start (FR-20).

The metadata check runs in every round until it passes, and never after: a partition count that changes later is not noticed (FR-17, follow-up).

### 2. The tail round (after the first seek)

1. `tail.fetch()`.
2. `Ok(batch)`: for each record in order, `parse_event(offset, value, EVENT_MAX_DATA_BYTES)`. `Err(reason)`: `rejected += 1`, log `event record rejected` (offset, reason), continue. `Ok(event)`: `window.push(Arc::new(event))` first, then, when ready, `behind += publisher.publish(&event)`. After the batch: `P = batch.next`; `publisher.advance(P)`; when `behind > 0` log `subscribers fell behind` once; when not ready and `P >= goal`, become ready (step 1.4). Clear the undecodable mark.
3. `Err(BelowEarliest { earliest })`: log `event tail behind retention` (from `P` to `earliest`); `P = earliest`; `tail.seek(P)`; `publisher.advance(P)`. Events between the old `P` and `earliest` are gone from the topic.
4. `Err(Undecodable { at })`: when the undecodable mark already holds `at` (the second consecutive round at that position): `rejected += 1`, log `event record rejected` with offset `at` and reason `undecodable batch`, `P = at + 1`, `tail.seek(P)`, `publisher.advance(P)`, clear the mark. Otherwise set the mark to `at` and fail the round. A batch of several records is passed one offset per two rounds and counted once, because the mark moves with `P` and only a new position after a success counts again.
5. `Err(Unavailable)`: the round failed.
6. **Observation** when the fetch returned no record, or failed, or the last watermark read is `OBSERVE_EVERY` (1 s) old: `probe.watermarks()`. `Ok(w)`: `broker_reachable = true`, `earliest`, `end` updated. `Err`: `broker_reachable = false`; the round failed.
7. **Re-seek** `tail.seek(P)` when the fetch failed (`Unavailable`), when a failed watermark read is followed by a successful one, or when the fetch returned no record and the observation shows `end > P` (FR-21: the client's fetch backoff must not delay delivery after an outage).
8. Publish `TailState` (`lag = end - P`, saturating, computed by the status).
9. Result: `Err(RoundFailed)` when a step failed, else `Ok(())`. The first failure after a success logs `event tail failed`; the first success after a logged failure logs `event tail recovered`; a node that never succeeded logs neither. The loop pauses 1 s doubling to 30 s after failures (FR-26) and cuts a round after `round_limit` (`WAIT + 3 x KAFKA_REQUEST_TIMEOUT`).

> Assumption: the tail skips an undecodable batch only when the second consecutive round fails at the same position. A single `Protocol` error may be transient (it was not probed), and skipping on it would lose events; FR-19 asks for the skip, the brief's open point preferred no skip, and two rounds is the smallest rule that keeps both.
>
> Assumption: the variables, constants and readiness rules of the PRD replace those of the design brief where they differ (`EVENT_WINDOW_BYTES`, `EVENT_WINDOW_PRELOAD`, `EVENT_REPLAY_MAX_OFFSETS`, a single `KAFKA_REQUEST_TIMEOUT` for request and connect, the retry pause of 1 s to 30 s, 4 permits as a constant, no `EVENT_TAIL_STALE_AFTER`, a fixed start goal, no replay stall count but a stall time of `KAFKA_REQUEST_TIMEOUT`). The task binds this document to the PRD's names, and the PRD was written after the brief with these choices recorded.

### 3. Subscription (`Subscriptions::open` and `Subscription::step`)

`open(request)`, synchronous, no broker read:

1. `hub.register(&request.types)`: `Closed`, `NotReady`, `AtCapacity` end here (503). The subscriber carries `p`, the hub position at registration.
2. Every requested type enters `types` with since `Cursor::START`.
3. Without a cursor: `floor = target = p` (live from the node's position). With a cursor `c`: `floor = c`, `target = None`.

`step()`, one of four cases in this order:

1. **Pending** (`pending` not empty): take events from the front until one is wanted (its type is in `types` and its sequence is above the type's since); encode it with `Frame::encode` and return `Frame`. When `pending` runs empty without a wanted event, put `VecDeque::new()` in its place and return `Idle`. Awaits nothing.
2. **Start** (`target` is `None`): when the hub is closed, `End(HubClosed)`. Let `p` be the subscriber's position.
   - no type: `floor = max(floor, p)`;
   - `floor >= p` (cursor at or ahead of the tail, FR-15): nothing to read; queued frames at or below `floor` will be dropped;
   - `floor < F` (below the window): `floor = max(floor, L, min(F, p.rewound_by(MAX)))`, with `L` from `TailState::earliest` (or `Cursor::START` when unknown);
   - otherwise (inside the window) the floor stays.
   Then `target = Some(p)`; return `Idle`.
3. **Page** (`floor < target`): when the hub is closed, `End(HubClosed)`. `window.read(floor, target, &types, REPLAY_PAGE)`:
   - `Page { events, settled }`: append to `pending`, `floor = settled`; when no event came back, `tokio::task::yield_now().await`; `Idle`.
   - `Evicted` (the floor is below `F`): acquire a permit (`Semaphore` of 4, first in first out, held to the end of the step); when the hub is closed, `End(HubClosed)`; `replay.page(floor)`:
     - `Ok(batch)`: keep the records with sequence at or below `target` that parse (`parse_event`; invalid ones are skipped without a log entry or count) and are wanted; `new = min(batch.next, target)`. When `new == floor` and nothing was kept: `stalled_since` is set if unset, and when it is `KAFKA_REQUEST_TIMEOUT` old, `End(TopicFailed)`; else `stalled_since = None`, `floor = new`, kept events to `pending`. `Idle`.
     - `Err(BelowEarliest { earliest })`: `floor = max(floor, min(earliest, window.start()))`; `Idle`.
     - `Err(Undecodable { at })`: `floor = max(floor, at.advanced_by(1))`; `Idle`.
     - `Err(Unavailable)`: `End(TopicFailed)`.
4. **Live** (`floor >= target`): `subscriber.recv().await`. A frame with sequence at or below `floor`: `Idle`. Another frame: `floor` = its sequence; `Frame` when wanted, else `Idle`. `None`: `subscriber.resume()`; `Err`: `End(HubClosed)`; `Ok(p')`: `target = None`; `Idle` (falling behind, section 5).

`next()` calls `step` until `Frame` or `End`. Every path that returns `Idle` without awaiting is followed by a path that awaits (the window path yields, the Kafka path awaits the permit and the read, the live path the queue), so `next` cannot spin.

`subscribe(types)`: when the set plus the new names exceeds 100, `TooManyTypes`, nothing changes; else `since = subscriber.subscribe(&types)` and each new name enters with `since` (an existing name keeps its since). `unsubscribe(types)`: the names leave the hub entry and the map.

> Assumption: the replay cap never raises the floor above the window floor `F`: the rule is `max(c, L, min(F, p - MAX))`, which equals FR-14's `max(c, L, P - MAX)` whenever `p - MAX <= F`, and gives FR-14's stated behaviour for `MAX = 0` (a cursor below `F` starts at `F`). The cap bounds Kafka reads; events still in memory are always served.
>
> Assumption: a cursor ahead of the tail waits without a broker read and without the brief's two-observation wait: the floor is the cursor, the subscription is live at once and drops queued frames at or below it. This is FR-15's rule (neither clamped nor refused), which the PRD chose over the brief's clamp.
>
> Assumption: a replay that meets an undecodable batch skips one offset per page read without a log entry or count, as FR-19 says for invalid records met by a replay; the tail has logged and counted it when it passed.

### 4. Resume cases

| Cursor `c` | What the subscription does |
| --- | --- |
| none | `floor = target = p`: live; ids above `p` |
| `F <= c <= p` | window pages, no broker read, no permit, then live |
| `L <= c < F` and `c >= p - MAX` | Kafka pages from `c` under the permits until the floor reaches `F`, then window pages, then live |
| `c < F` and `c < p - MAX` | floor raised to `max(L, min(F, p - MAX))`; the client gets the events above it; no error, no marker |
| `c < L` | floor raised to `L`; when retention moved further meanwhile, the first page answers `BelowEarliest` and the floor moves to `min(earliest, F)` |
| `c == p` | live, no read |
| `p < c <= E`, or `c > E` | no read; live with `floor = c`; frames at or below `c` dropped; ids above `c` delivered as the tail passes them |
| any `c`, `MAX = 0` | below `F`: starts at `F`; never reads Kafka |

### 5. Falling behind and catching up

When `try_send` fails, the hub drops the subscriber's sender (registry rule). `recv` returns the queued frames, then `None`; `resume()` re-arms the subscriber at the hub position `p'` (a closed hub: `End(HubClosed)`); `target = None`; the start step and the page steps replay `(floor, p']` from the window, or from Kafka when the floor was evicted (with the same floor rule, so a client more than `MAX` behind skips the oldest), on the same connection; then live. The subscriber count does not change. A second burst during the replay ends the new queue again and the next round starts from a higher floor.

### 6. Why there is no gap and no duplicate

Arming (registration or `resume`) happens under the registry lock and yields `p`. The tail pushes every valid event to the window before it publishes it, and publishes in offset order. So at arming every event with id at or below `p` is in the window or, when evicted, in Kafka at or above `L` (or gone by retention), and every event above `p` is queued to the subscriber after the arming (or the queue ends and section 5 covers it). The replay covers `(floor, p]`; the queue covers everything above `p`; a queued frame at or below the floor is dropped; each written frame becomes the floor. Ids on a connection therefore strictly increase, no offset is written twice on a connection, and no id at or below the cursor is written (D8). A run-time subscribe gets the hub position as the type's since, so no later replay writes an older event of that type.

### 7. An invalid record

The tail skips it, logs `event record rejected` (offset, reason, never the value, key, headers or a member value) and counts it in `rejectedRecords`; it never reaches the window, a client or a replay page, and the next record is processed normally. A replay page that contains it skips it silently (it was counted when the tail passed it, or before this process started).

### 8. Topic missing or with more than one partition

At start, step 1.1 keeps the node `starting` with reason `topic-not-found` or `partition-count`, its readiness probe failing, its streaming routes answering 503 `the node is starting`, and `event topic invalid` logged once per change. The node never creates the topic. It retries with the back-off of the loop and becomes ready as soon as the topic exists with one partition. A topic deleted after the start makes the watermark reads fail (`brokerReachable: false`); the node stays ready (NFR-6).

### 9. Broker away and back

While the partition cannot be read: `poll` returns empty or the round is cut; the observation fails, so `brokerReachable` turns false and the round fails; the loop pauses 1 s, 2 s, ... 30 s; open connections stay open with keep-alives; a ready node stays ready and accepts subscriptions; a resume inside the window is served; a replay below the window ends its stream after `KAFKA_REQUEST_TIMEOUT` without progress or at the first failed page (1011 on WebSocket); a starting node stays starting. When the broker is back: the first successful observation after a failed one triggers a re-seek to `P`, which clears the client's fetch backoff; the tail continues from `P`, so every event still retained is delivered, in order, with no repeat on a connection. Each adapter reconnects with one attempt per call; the retries are the rounds of the loop.

## Failure Modes

| Failure | What the node does | What subscribers see |
| --- | --- | --- |
| Broker unreachable at start | stays `starting` (`broker-unreachable`), retries with back-off, logs nothing until it answers | 503 `the node is starting` on this node; the status route shows the reason |
| Topic missing or not one partition at start | stays `starting` (`topic-not-found`, `partition-count`), logs `event topic invalid` once per change | 503 `the node is starting` |
| Broker or partition leader away after start | rounds fail, `brokerReachable: false`, `event tail failed` once, back-off up to 30 s, re-seek when back | open streams stay with keep-alives; no new events until the broker is back, then every retained event in order; new subscriptions accepted; window resumes served |
| Broker hangs (connection open, no answer) | every call cut by the adapter's time-out, the round by `round_limit` | as above |
| Replay read fails or makes no progress for `KAFKA_REQUEST_TIMEOUT` | the stream ends; `replay failed` logged | SSE stream ends (EventSource reconnects with `Last-Event-ID`); WebSocket close 1011; the client resumes on any node |
| Invalid record | skipped, logged, counted | nothing; a gap in the ids |
| Undecodable batch at the tail | the first round fails; the second skips one offset, logs and counts once | nothing; a gap |
| Retention overtakes the tail | jump to `L`, `event tail behind retention` | events below `L` are lost for every client of this node |
| Cursor below `L` or below `P - MAX` | floor raised | the oldest events are skipped without a signal (FR-23) |
| Slow client (queue full) | sender dropped, replay from window or Kafka on the connection | a delay, no loss within the replay reach, no duplicate |
| Client takes no data for `STREAM_KEEP_ALIVE` | the kernel (transport bound) or the write deadline ends the connection | connection lost without end or close frame; resume with the last id |
| Reader loop panics | `background task failed`, shutdown sequence, exit status 1 | SSE end of stream, WebSocket 1001; reconnect to another node |
| SIGTERM | listener closed, loop stopped, hub closed, 2 s drain wait, HTTP drain | SSE end of stream, WebSocket 1001 after queued frames; reconnect elsewhere |
| Node at its cap | 503 `the node is at its subscriber capacity` | retry on another node |
| Topic deleted and recreated | offsets restart lower; the tail position is above the new end and waits; cursors above the new end wait too | nothing new until the node restarts (operators restart nodes after recreating the topic; `docs/operations.md`) |
| An event written twice by a producer | delivered twice with two cursors | the client deduplicates by the envelope `id` |

## Performance Design

No latency, throughput or capacity figure exists for this design. NFR-1 (10 000 subscribers on one node of 4 vCPU and 8 GiB, p99 fan-out below 100 ms at 100 events/s) is a design target that this change does not measure; the K6 trend measures another interval.

Per-event work of a node on the live path:

1. one share of a fetch (the tail long-polls once per node for all clients, NFR-3);
2. one size check and one `serde_json` parse and serialisation of the record value, whatever the number of subscribers;
3. one window push: a write lock, one deque append, pops while over the byte bound;
4. one hub lock: one `HashMap` lookup by type; when the type has subscribers, one encoding of `Frame` (SSE event and WebSocket text) and one `try_send` of an `Arc<Frame>` per subscriber of that type (NFR-2); nothing per connection without the type;
5. per subscriber task: one socket write of the shared bytes.

What bounds a node: the subscriber cap, `SUBSCRIBER_BUFFER` shared frame references per subscriber, at most one replay page (500 events, 1 MiB plus one batch) per replaying connection, 4 replay permits and 4 Kafka readers per node, the window's `EVENT_WINDOW_BYTES`, the WebSocket buffers (4 096-byte read buffer, 32 768-byte frame and message). The sizing rule of `docs/operations.md` (NFR-9): `MAX_SUBSCRIBERS` times the cost of an idle connection, plus `EVENT_WINDOW_BYTES`, plus 4 replay pages, below half the memory limit; the per-connection costs of the first plan (about 6 KiB idle WebSocket, 20 KiB idle SSE) are estimates. The event rate of the system is bounded by one partition and by what one node can read and validate; adding nodes adds subscriber room, not event rate (NFR-12).

The hub test that hands one event to 10 000 subscriber queues of one type is a structural test of NFR-2 and NFR-10 (each queue holds the frame, the publish returns without awaiting); it asserts no time.

## Removal of PostgreSQL and of the Example Context

Source: `postgres-removal.md` (inventory with file and line). The removal runs after the notification feature, as its last stories, after the maintainer's approval, and lands in two pull requests because the configuration guard reads `required_directories` from the base branch.

Order:

1. **Before any removal** (feature stories, no protected file): `notification/event-stream` is scaffolded (13 crates stay after the 3 example crates go: 10 + 3); `eventTypes` and `eventStreamStatus` are wired next to the example roots (the schema is never empty, and `EventStreamStatus`, which sorts before `EventType`, has several single-line fields for the protected G20 test); the Kafka broker joins the stack through `make/service.mk` without touching `DEPS`.
2. **Preparatory pull request** (label `approved-config-change`): `protected-config.toml` loses `.sqlx` and `migrations` from `required_directories` (lines 158, 164) and nothing else. It is merged into `main` before pull request #5 deletes the directories.
3. **Removal story A, code** (one commit; hooks need clippy, `cargo deny` unused and shear green together): delete `crates/company-subdomain/` and its members and workspace entries; delete `sqlx` and (when unused) `fake` from `[workspace.dependencies]`; delete `crates/shared/infrastructure/src/persistence.rs`, its tests and `build.rs`; delete the PostgreSQL probe and its tests; remove `Dependency::Database`; delete `service migrate` (`cli.rs` `Migrate`, `MigrateAction`, `DatabaseArgs`, `MigrateArgs`; `command/migrate.rs`; `tests/integration/migrate.rs`); delete `PostgresSettings`, the `DATABASE_*` parsing and the rule `HEALTH_PROBE_TIMEOUT >= DATABASE_ACQUIRE_TIMEOUT`; `bootstrap.rs` without pool, repository, `PostgresProbe` and example handlers (probes: cache, message broker, event stream); `graphql/schema.rs` with `Query(EventStreamQuery)` and `EmptyMutation`; `http.rs` and `openapi.rs` without the example state, tag and router; rewrite the unit and integration tests of `crates/service` on the remaining routes (body limit, timeout, panic and request id through `/api/graphql`, `/api/events/status` and `/api/health`); `tests/support` without `fixture.rs` and `repository.rs`; `tests/e2e` without database code and `steps/example_api_resource.rs`; delete `features/example_api_resource.feature`, rewrite `features/graphql.feature`, `features/http_errors.feature`, `features/api_docs.feature`, and remove the "database is unavailable" scenario of `features/health_check.feature`; regenerate both snapshots; `workspace.dsl` without the database container and the example group; reword `xtask/templates/infrastructure/lib.rs.tmpl:5`; refresh the lock without raising other versions. Protected files: `scripts/check-docs.sh`, `tests/CLI/bats/check_docs.bats`, the hash pin in `tests/CLI/bats/make_general.bats`, `tests/CLI/bats/make_quality_gates.bats`. Labels: `approved-config-change` and `approved-breaking-change`.
4. **Removal story B, stack and gates** (with story A green in `make ci` only together, so the same pull request, one or two commits): Makefile targets `migrate`, `migrate-rollback`, `create-migration`, `setup-test-db`, `setup-load-test-db`, `sqlx-prepare`, `validate-sqlx-metadata`, `ensure-database` deleted and no target depends on them; `EXEC_ENV` without `DATABASE_STATEMENT_TIMEOUT`; `TEST_ENV` without `env -u DATABASE_URL`; `DEPS ?= localstack`; `CI_TESTS_TARGETS = tests-with-coverage doc-tests e2e-tests`; help and `.PHONY` lines; `docs-check` without `migrate --help`; `make/gates.mk` without `SQLX_CHECK_CMD`; scripts `setup-test-db.sh`, `check-sqlx-metadata.sh`, `setup-load-test-db.sh` deleted; `scripts/with-load-test-stack.sh` without the call; `scripts/install-tools.sh` and `scripts/doctor.sh` without sqlx-cli; compose files, `Dockerfile`, `.env`, `.env.test`, `.devcontainer/devcontainer.json` without database parts; `.github/workflows/quality.yml` without the job `query-metadata`; `tests.yml` and `scheduled.yml` without sqlx-cli; `AGENTS.md` gate list equal to `make ci-list`; `.claude/rust-sdlc.yml` regenerated (`persistence.engine: null`, `migrations_dir: null`, `offline_metadata: false`, `make.setup_test_db`, `make.create_migration`, `make.migrate`, `make.migrate_rollback`, `make.sqlx_prepare`, `make.validate_sqlx_metadata` set to `null`, required check `Persistence query metadata` removed); `.templatesyncignore` gets every synchronised path the removal changes or deletes; `migrations/` and `.sqlx/` deleted. Protected files: the rest of "Protected Changes", part B.
5. **Documentation and skills**: the files of `postgres-removal.md` section 1(k); a new ADR supersedes `docs/adr/0005`. Gate: `make lint-repo docs-check`.

The maintainer also removes the required status check "Persistence query metadata" from the branch rules of `main`, or the pull request waits for a check that never reports. The template's SQS parts stay (D11): `AWS_REGION` stays required and the SQS probe stays in the readiness report.

> Assumption: `DEPS` becomes `localstack` and not `localstack kafka`, because the broker is started by `ensure-kafka` with its host listener ("Local Stack and Load Test"); the two protected lines that pin the dependency list change to that text.

## Protected Changes

Every change below waits for the maintainer's approval: the label `approved-config-change` on the pull request and a session restarted with `ALLOW_PROTECTED_CONFIG_EDIT=1` (`AGENTS.md:16`). Each is made only in the story that names it, placed first among the stories that need it; every story that does not need them can be implemented before the approval.

**Feature story (Kafka allow-list), before the Kafka adapter and the envelope parser:**

| File | Exact change | Why |
| --- | --- | --- |
| `architecture.toml` | infrastructure layer `external`: add `"krafka"` and `"tokio"` | the adapter uses the client and `tokio::time::timeout` (D9, FR-21) |
| `architecture.toml` | application layer `external`: add `"serde_json"` | the envelope is parsed and the event object written in the application (FR-18) |
| `architecture.toml` | application layer `dev_external`: add `"tokio-tungstenite"` | the real-socket WebSocket test of the application crate |

No other protected file changes for the feature: the test fakes stay `std`-only in `tests/support`; Kafka test helpers live in the infrastructure tests, `crates/service` and `tests/e2e` (external `*`); the publisher of the load test is a shell loop in the load-test compose override; `DEPS` and the Bats files are untouched because the broker's host listener is in a file that `make start` does not read.

**Preparatory pull request:** `protected-config.toml`: remove `.sqlx` (line 158) and `migrations` (line 164) from `required_directories`.

**Removal story A (code), with `approved-breaking-change` as well:**

| File | Exact change |
| --- | --- |
| `scripts/check-docs.sh` | `:81` `root serve healthcheck`; `:453` `serve healthcheck`; comments `:19-20`, `:36` (the `migrate` command is gone) |
| `tests/CLI/bats/check_docs.bats` | `:558` append to `healthcheck.txt` instead of `migrate.txt` |
| `tests/CLI/bats/make_general.bats` | `:81-94` the new sha256 of `features/health_check.feature` without the database scenario |
| `tests/CLI/bats/make_quality_gates.bats` | `:168-172` G6 with another invalid variable than `DATABASE_URL` (for example `KAFKA_TOPIC=..`) |

**Removal story B (stack and gates):**

| File | Exact change |
| --- | --- |
| `protected-config.toml` | remove `:32` (`scripts/setup-test-db.sh`) and `:33` (`scripts/check-sqlx-metadata.sh`) |
| `scripts/setup-test-db.sh`, `scripts/check-sqlx-metadata.sh` | delete |
| `make/gates.mk` | delete `:14` (`SQLX_CHECK_CMD`) |
| `scripts/with-load-test-stack.sh` | delete `:52` (the call of `setup-load-test-db.sh`); reword `:6-7` |
| `tests/CLI/bats/make_database.bats` | delete |
| `tests/CLI/bats/make_tests.bats` | delete tests `:47-141` and `:412-449`; helpers `:27-32`, `:35-43`; header `:3-6` |
| `tests/CLI/bats/make_general.bats` | `:61-66` list without `setup-test-db`, `setup-load-test-db`, count 42 -> 40; `:143` `setup-test-db` -> `tests-with-coverage`; `:583` `DEPS=[localstack]` |
| `tests/CLI/bats/make_execution_mode.bats` | `:44`, `:49` `DB_PORT` removed; `:102-106` four `-e` options without `DATABASE_STATEMENT_TIMEOUT`; `:136` `up --detach --wait --no-recreate localstack`; delete `:192-195`; `:197-216` without `DATABASE_URL`; delete `:233-244` |
| `tests/CLI/bats/make_lifecycle.bats` | `:15-17` comment; `:47`, `:51-52`, `:55`, `:260`, `:272-274` `DB_PORT` removed; `:132`, `:142-144` migrate step removed; `:168`, `:171`, `:213` `container_of database` removed; `:232-248` stand-in images and `migrate` service removed |
| `tests/CLI/bats/make_load_tests.bats` | delete `:108-140` |
| `tests/CLI/bats/make_security.bats` | `:85`, `:339` `DATABASE_URL` removed; `:370-371`, `:374`, `:382-384` `migrate` service removed; delete `:388-410`; `:418-419` |
| `make/tool-versions.mk` | delete `:16` (`SQLX_CLI_VERSION`) |
| `.cargo/config.toml` | delete `:5-6` (`SQLX_OFFLINE`) |
| `architecture.toml` | remove `"sqlx"` from infrastructure `external` (`:57`); keep the `forbidden_transitive` entries |
| `.claude/settings.json` | remove `:31`, `:68`, `:93` (permissions of deleted targets) |

The line numbers are those of head `ecc5d17`; the stories re-read them. Protected files that mention the database and need no change: `scripts/check-invariants.sh`, `tests/CLI/bats/check_invariants.bats`, `tests/CLI/bats/make_contracts.bats`, `tests/CLI/bats/test_helper.bash`, `xtask/tests/**`, `xtask/src/arch/**`, `taplo.toml`, `_typos.toml`, `deny.toml`, `.cargo/mutants.toml`, `.githooks/*`, `scripts/with-env.sh`, `scripts/mutation-diff.sh`, the K6 thresholds.

> Assumption: the plugin skill `bounded-context-scaffolding` names "no protected file changed" as a success criterion of a scaffold; story 1.1 meets it (the scaffold changes no protected file), and the allow-list entries come in their own approved story, as D16 requires.

## Contract Impact

- **OpenAPI** (`.github/openapi-spec/spec.yaml`): adds `GET /api/event-types` (`api_event_types_get`, 200 array of `EventType`, 400, 422, default problem) and `GET /api/events/status` (`api_events_status_get`, 200 `EventStreamStatus`, default problem), the schemas `EventType` and `EventStreamStatus`, the tag `EventStream`; the health operation's description names the event stream. `make.openapi_diff` reports additions only during the feature. The removal deletes the example paths and schemas (breaking; `approved-breaking-change`). The pinning test `crates/service/src/openapi/tests.rs` (operation count, schemas, tags) changes in the same commits. Spectral: each new operation has summary, description, tag, a 2xx and a 4xx or `default`, and described parameters.
- **Streaming routes** stay out of OpenAPI (`OpenApiRouter::route`): OpenAPI cannot describe WebSocket, and a never-ending response would hang the Schemathesis gate (FR-38's assumption). They are specified in `docs/streaming-protocol.md`.
- **GraphQL** (`.github/graphql-spec/spec`): adds `eventTypes(itemsPerPage: Int! = 30, page: Int! = 1): [EventType!]!`, `eventStreamStatus: EventStreamStatus!`, `type EventType { name: String! }` and `type EventStreamStatus` with the ten fields of FR-4. The removal deletes the example query and mutation fields and the mutation root (breaking). `make.graphql_diff` reports additions only during the feature. The SDL test (no `node`, `Connection`, `update`, `delete`, `type Subscription`) holds.
- Both snapshots are regenerated with `make generate-openapi-spec` and `make generate-graphql-spec` and committed in the story that changes the routes (`make validate-contract-snapshots`).

## Architecture Model Impact

`workspace.dsl`: one component per new crate inside the service container (`event-stream-domain`, `event-stream-application`, `event-stream-infrastructure`), one relationship per workspace-internal `[dependencies]` edge (application -> domain, infrastructure -> domain, service -> each, shared crates as used), and a new external software system `Kafka` ("The VilnaCRM event bus; the notifications topic") with the relationship "event-stream-infrastructure reads the notifications topic from" it. The browser frontend relationship gains "subscribes over SSE and WebSocket". The removal deletes the `database` container (`:12-14`), the group `CompanySubdomain/SomeModule` (`:51-67`) and their relationships, and rewords the descriptions that name PostgreSQL. SQS stays. Check: `make validate-architecture-model`.

## Local Stack and Load Test

**Broker.** `docker-compose.yml` gets a service `kafka`: image `apache/kafka:4.1.1@sha256:0bc1bb2478f45b6cea78864df86acdc11e8df2c5172477819a4d12942cbe5d40` (tag and digest, rule (b) of `check-invariants.sh`; the digest was read from the local image, the story confirms it against the registry), KRaft single node:

- `KAFKA_NODE_ID=1`, `KAFKA_PROCESS_ROLES=broker,controller`, `KAFKA_CONTROLLER_QUORUM_VOTERS=1@localhost:9093`, `KAFKA_CONTROLLER_LISTENER_NAMES=CONTROLLER`, `KAFKA_INTER_BROKER_LISTENER_NAME=INTERNAL`;
- `KAFKA_LISTENERS=PLAINTEXT://:9092,INTERNAL://:9094,CONTROLLER://:9093,EXTERNAL://:9095`, `KAFKA_ADVERTISED_LISTENERS=PLAINTEXT://kafka:9092,INTERNAL://localhost:9094,EXTERNAL://localhost:9095`, `KAFKA_LISTENER_SECURITY_PROTOCOL_MAP=CONTROLLER:PLAINTEXT,PLAINTEXT:PLAINTEXT,INTERNAL:PLAINTEXT,EXTERNAL:PLAINTEXT`;
- `KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR=1`, `KAFKA_TRANSACTION_STATE_LOG_REPLICATION_FACTOR=1`, `KAFKA_TRANSACTION_STATE_LOG_MIN_ISR=1`, `KAFKA_GROUP_INITIAL_REBALANCE_DELAY_MS=0`, `KAFKA_AUTO_CREATE_TOPICS_ENABLE=false`, `KAFKA_LOG_DIRS=/var/lib/kafka/data` on the volume `kafka_data` (the image writes to `/tmp` otherwise);
- health check: `kafka-topics.sh --bootstrap-server localhost:9094 --create --if-not-exists --topic notifications --partitions 1 --replication-factor 1` followed by `--describe --topic notifications`, interval 10 s, start period 60 s; the broker is healthy only when the topic exists with one partition;
- `app` gets `depends_on: kafka: condition: service_healthy` and `KAFKA_BOOTSTRAP_SERVERS: kafka:9092`.

`docker-compose.prod.yml`: `kafka` under profile `local`; `app` gets `KAFKA_BOOTSTRAP_SERVERS: ${KAFKA_BOOTSTRAP_SERVERS:-kafka:9092}` and `MAX_SUBSCRIBERS: ${MAX_SUBSCRIBERS:-5000}` (NFR-9), and no `:?` variable for Kafka (a protected test renders the reference with only `DATABASE_URL` and `AWS_REGION`, later only `AWS_REGION`). `docker-compose.load_test.override.yml`: `kafka` without a host port and `restart: 'no'`. `docker-compose.override.yml`: no Kafka port. Data and the topic survive a restart of the stack's service (volume).

**Host listener for the tests.** A new file `docker-compose.kafka-host.yml` holds only `services: kafka: ports: ["127.0.0.1:9095:9095"]`. A new `make/service.mk` (loaded by `Makefile:566`) adds:

- `ensure-kafka: ## Start the Kafka broker with its host listener unless this shell is the development container` — `[ "$(IN_DEV_CONTAINER)" = 1 ] || $(DOCKER_COMPOSE) -f docker-compose.yml -f docker-compose.override.yml -f docker-compose.kafka-host.yml up --detach --wait kafka`;
- `ensure-deps: ensure-kafka` (a prerequisite added to the template's target, so `make.integration_tests`, `make.tests_with_coverage`, `make.e2e_tests` and the mutation targets start the broker first).

Both names get a row in a new `docs/make-targets-service.md` (docs-check rule 6). CI calls the same targets, so no workflow changes for start-up; the isolated load-test stack starts every service of `docker-compose.yml`, the broker included, with no change to the protected stack script.

> Assumption: the host reaches the broker through a fourth listener `EXTERNAL://localhost:9095`, published only by `docker-compose.kafka-host.yml`, which `make start` does not read. A host port in the files `make start` reads would collide between the development project and the private projects of the protected Bats suite, which get free ports only for `DB_PORT`, `LOCALSTACK_PORT`, `APP_PORT` and `STRUCTURIZR_PORT`; this keeps the feature free of Bats edits. The cost: `ensure-kafka` recreates a running `kafka` container of the development project when it lacks the port (compose recreates on a changed configuration); its topic data survives on the volume.
>
> Assumption: the stack creates the topic in the broker's health check (`--if-not-exists`, not probed), so no one-shot service is needed and every `up --wait` waits until the topic exists.

**Publisher of the load test.** A service `load-test-publisher` in `docker-compose.load_test.override.yml` uses the Kafka image: `bash -c` waits until `kafka-topics.sh --describe --topic notifications` succeeds, then pipes a loop into one `kafka-console-producer.sh --bootstrap-server kafka:9092 --topic notifications --producer-property linger.ms=0 --producer-property acks=all`. The loop writes one envelope every 100 ms: `{"id":"<from /proc/sys/kernel/random/uuid>","type":"load.test","occurredAt":"<date -u +%Y-%m-%dT%H:%M:%SZ>","data":{"sentAtMs":<date +%s%3N>}}`. Embedded `$` are written `$$` (compose). No production route publishes, no Rust code and no new image are involved, and no protected file changes.

**Scenario** `tests/Load/scripts/subscriptions.js` (sorts after `health`): `setup` polls `GET /api/events/status` until `state` is `ready`; the smoke profile holds 50 SSE subscribers (`http.asyncRequest` with a timeout, without the profile's `test_type` tag) and 50 WebSocket subscribers (`k6/websockets`, `types=load.test`); each iteration checks a catalog request (200), that the WebSocket opened (101), and that the held SSE response received at least one event; the WebSocket handler records `Date.now() - data.data.sentAtMs` in the trend `event_delivery_latency`. `tests/Load/config.json.dist` gets `endpoints.subscriptions` with all four profiles, the catalog p99 budget (500 ms in smoke) and `deliveryThreshold` (2 000 ms in smoke); both are budgets, not measurements. The first seconds of the publisher (JVM start) are excluded by the `setup` wait and the scenario's ramp.

> Assumption: the publisher writes at a fixed 10 events per second set in the stack, not driven by the K6 profile. It is the smallest honest publisher the pinned K6 image allows (no Kafka module); a profile-driven rate is a follow-up.

## Test Strategy

General rules: unit tests in sibling `tests.rs`; paused tokio time wherever a timer is involved; every awaited call of a test inside `tokio::time::timeout`; a scripted fake that a loop waits on pends (`std::future::pending`) after its script instead of answering for ever, so a mutant fails an assertion and never hangs a test; one Kafka topic per test (nextest runs tests in parallel processes against one broker).

**Fakes (`tests/support`, `std` and `async-trait` only, depends on `event-stream-domain`):** `ScriptedTopicProbe`, `ScriptedTopicTail` and `ScriptedTopicReplay`, each a `Mutex<VecDeque<Result<..>>>` of scripted answers that records its calls (seek positions, page positions); `StaticTopic`, an in-memory partition (records, log start, end) that serves all three ports with the port rules, for subscription and two-node tests without a broker.

**Domain:** `EventType` (length 128 and 129, the pattern), `Cursor::parse` (digits, sign, blank, empty, overflow), the saturating arithmetic of `Cursor` and `Offset::sequence`, `Sequence::new(0)`, `PageLimit::reached`, `StoredEvent::cost`.

**Application:**

- `parse_event`: every `RejectReason`; size at `max_bytes` and one byte above; a duplicate member; unknown members dropped; `data` of every JSON type; an upper-case UUID written lower case; `+00:00` and `Z`; years 1969, 1970, 9999; the output equals the byte string of `serde_json::to_string` of a test struct with `id`, `type`, `occurredAt`, `data` in this order (never a `serde_json::Value` comparison: `preserve_order` differs between a workspace run and a single-crate mutation run).
- `ws_json` and `error_json`: byte equality with the same oracle, including strings with quotes, backslashes, control characters and characters outside the Basic Multilingual Plane.
- `Window`: push and eviction by bytes (floor moves to the popped id, one event always kept); `restart`; every branch of `read` (evicted, no types, limit by rows, by bytes, scan bound, `settled` in each case).
- `TailEvents` with scripted fakes and `LogRecorder`: each start-up case of "Algorithms" 1 (with the log entry once per change of fault); `PRELOAD` 0 and an empty topic ready at once; the window filled but nothing published before ready; every case of the tail round (valid, invalid, null value, `BelowEarliest`, `Undecodable` once and twice, `Unavailable`); the observation period and the three re-seek triggers (the fake records seeks); the failed/recovered entries only at transitions and none before the first success; `subscribers fell behind` once per batch.
- `Hub`: the registry rules of the first plan (`check` and `register` order, cap, falling behind and resume, no empty key, count published under the lock, `advance`, `drained`); one event handed to 10 000 subscriber queues of its type (NFR-1 structural).
- `Subscription` with a real `Hub`, real `Window` and scripted replay: every row of "Resume cases"; the floor rule with `MAX` 0, small and large; permits (with four replays parked in gated page reads, a fifth makes no call until one of them returns); the stall rule under paused time; `BelowEarliest`, `Undecodable`, `Unavailable`; closed hub before a read; run-time subscribe and unsubscribe during a replay; falling behind with a burst larger than the queue (no `End`, count unchanged); `pending` capacity 0 after drain; the 100-type limit.
- `Session`, the pump through a scripted `Connection` (write deadline, ping during a replay turn, lingering), handlers through `oneshot` (every problem of FR-16 in its order, the 503s from `availability`, the SSE head, the upgrade rejection as a problem), `StatusSource::snapshot` (every state and reason, `lag`, nulls), GraphQL (`eventTypes` limits, `eventStreamStatus`).
- `tests/integration/ws.rs`: the real-socket cases of the first plan (upgrade, no types then subscribe, ping and pong, 32 768-byte frame answered, a header declaring 32 769 bytes ends the connection, a burst larger than the queue disconnects nobody).

**Infrastructure** (`tests/integration`):

- `real_broker.rs` (the broker of the stack, `KAFKA_BOOTSTRAP_SERVERS` from the environment through a helper with `#[expect(clippy::disallowed_methods, reason = "..")]`): topics created with `admin().create_topics`, records produced with the krafka `Producer` (`acks All`, `linger 0`, partition 0); partition count 1 and 3; a missing topic gives `None`; watermarks of an empty and a filled topic; fetch and page return records at or above the position with `next` past them; read-committed: an aborted transaction's records are never returned and the position passes the markers; a position below the log start (records deleted through the admin client, or a topic with small `retention.ms` and `segment.ms` when deletion is not available in the client) gives `BelowEarliest` with the new start; the largest page is recorded (open point 2 of the brief).
- `fake_broker.rs` (`FakeBroker::start_in_memory` with paused time): a silent broker (`Control::Silence`) gives `Unavailable` within the time limit; `Disconnect` gives `Unavailable` and the next call reconnects; `CorruptRecords` gives `Undecodable`; `crash` and `restart`; `require_sasl_plain` with `KafkaSecurity::SaslPlaintext`.

**Service:** `background.rs` under paused time with a `TailEvents` on scripted fakes (back-off 1, 2, 4 ... 30 s and reset; a cut round; the stop flag ends the loop and the pause; a panicking round gives `TaskFailed("tail")`); `EventStreamProbe`; config (every variable as the only problem, the SASL rule, `FIELDS` updated); `check-config`; `bound_stalled_peer`; origins; spawned-binary tests updated (the idle log stays three entries with no broker); an `Application` with `StaticTopic` adapters serving SSE and WebSocket over real sockets; the two-node test: two tails and hubs on one real topic, one subscriber on each, every event on both (FR-22).

**End to end** (`features/`, `tests/e2e`, the broker of the stack, a topic per scenario, events produced by the world with the krafka producer): catalog, status, SSE delivery of a subscribed type only, WebSocket delivery with run-time subscribe, resume from the window, resume from Kafka (a small `EVENT_WINDOW_BYTES`), an invalid record skipped and counted, the 503 of a starting node (a topic that does not exist yet).

**Coverage and mutation:** one coverage run over `--lib --bins --test integration`, 100 % lines and functions; the Gherkin suite is not instrumented, so every branch is reached by unit or integration tests. Mutation: each mutant is judged by its own crate's tests; the fakes pend after their script; no test waits on a real timer for more than about one second; the non-Linux form of `bound_stalled_peer` has an empty body (no mutant). The real-broker tests are in the serialized verify step (one broker).

## Decision Records

New records under `docs/adr/`, numbered after the last one:

1. **Every node reads Kafka directly.** Each node tails the one topic without a consumer group and filters for its own subscribers. Reason: the owner's decision; no coordination, any node serves any client, nodes are stateless. Cost: the broker serves the whole topic once per node; every node validates every event; the event rate is bounded by one partition and one node.
2. **No election, no lock, no link between nodes.** Nothing needs a single writer: the service writes nothing to Kafka, and the cursor (offset + 1) is the same on every node. Supersedes the lease and leader of the first plan.
3. **One topic, one partition.** One partition gives one total order and a single number as the cursor. Growth path, not designed: several partitions with a short list of offsets as the cursor.
4. **krafka pinned behind a port, rdkafka as the alternative.** `krafka =0.32.0` passes the dependency policy without lifting the OpenSSL ban, needs no C toolchain for its Kafka code, has time-outs and an in-memory fake broker. Its weakness: one author, pre-1.0, 16 releases in 14 weeks with breaking minor releases. Limited by the exact pin, the domain ports (`TopicProbe`, `TopicTail`, `TopicReplay`) and read-only use. `rdkafka` replaces only the infrastructure crate, the composition root and the manifests; it needs the OpenSSL ban lifted for TLS and a C toolchain in the builder image.
5. **At least once, client-side deduplication.** The service delivers every retained event above the cursor in offset order and removes no duplicates; the client deduplicates by the envelope `id`, which the event object carries for that reason.
6. **PostgreSQL is removed.** The service stores nothing; Kafka holds all shared state. Supersedes `docs/adr/0005`. The example context goes with it.
7. **SQS stays.** The template's SQS parts and LocalStack stay as shipped; the notification code does not use them. Their removal is a follow-up the owner may ask for.
8. **Readiness means "caught up once".** A node is ready when it reached the end it read at start; a later broker outage does not make it unready, because every node is equally affected.

## Differences from the First Plan

- The PostgreSQL event log, its three tables and migrations, the relay, the listener, pruning, the seed and the catalog table are gone; the catalog comes from `EVENT_TYPES`.
- The context `internal/cluster`, the leader lease, the election, `NODE_ID`, `CLUSTER_*` and `GET /api/cluster` are gone; the status is `GET /api/events/status` and the GraphQL query `eventStreamStatus`.
- The SQS event source, `EVENT_QUEUE_URL`, `EVENT_RETENTION`, `EVENT_RELAY_POLL_INTERVAL` and the LocalStack queue of the load test are gone; producers write to Kafka.
- New: the `TailEvents` loop reads Kafka on every node; the memory `Window`; replay from Kafka below the window under 4 permits and pages of 500; the ports `TopicProbe`, `TopicTail`, `TopicReplay`; the Kafka adapter; the readiness probe of the event stream.
- The cursor is the Kafka offset plus 1 instead of the log sequence; ids have gaps.
- The event on the wire is the whole envelope object (`id`, `type`, `occurredAt`, `data`) instead of opaque data text; `write_json_string` is replaced by `serde_json`.
- A cursor ahead of the topic end waits instead of being clamped to the latest sequence.
- The replay is capped by `EVENT_REPLAY_MAX_OFFSETS` below the tail and by retention instead of by `EVENT_RETENTION`.
- Six background loops become one; the supervision, the stop flag, the shutdown order (without the lease release), the 2 s drain wait, the transport bound, the close codes and the exit statuses are kept.
- The 503 answers of a starting or shutting-down node move into a middleware so that they precede validation (FR-16).
- Kept in substance: the hub and its registry rules, the subscription state machine (start, pages, pending, live), the SSE and WebSocket protocols, limits, commands, error frames and the pump, the browser origin rules, the configuration style.
- Protected changes: the first plan had none; this plan has three allow-list entries for the feature and the removal set of PostgreSQL.
