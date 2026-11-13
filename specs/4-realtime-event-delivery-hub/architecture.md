# Architecture: real-time event delivery hub (issue #4)

This document turns the design input, `prd.md` and `research.md` of this bundle into a design that can be coded story by story. Every statement about the repository was read in the code. Every statement about a library was read in the source of the locked version (`axum 0.8.9`, `sqlx-postgres 0.9.0`, `sqlx-core 0.9.0`, `aws-sdk-sqs 1.121.0`, `tokio 1.54.1`, `utoipa-axum 0.3.0`, `futures-util 0.3.34`), and every SQL statement was run on PostgreSQL 16. Decisions of this stage are marked `> Assumption:`. The last section lists what was changed against the design input and why.

## Tech Stack

- Rust toolchain: channel 1.99.1 (`rust-toolchain.toml`), edition 2024, `rust-version` 1.99.1.
- Crates of `[workspace.dependencies]` that the change uses or adds. No crate is added to a layer allow-list.

| Crate | Requirement | What the change uses it for |
| --- | --- | --- |
| `axum` | 0.8.9, features `macros` and, added by this change, `ws` | routes, extractors, `response::sse::{Sse, Event, KeepAlive}`, `extract::ws::{WebSocketUpgrade, WebSocket, Message, Utf8Bytes, CloseFrame}`, `Json::from_bytes` |
| `tokio` | 1.54.1, features unchanged (`macros`, `rt-multi-thread`, `net`, `signal`, `time`, `sync`) | `sync::{mpsc, watch, Notify}`, `time`, `select!`, task spawning in the composition root; `test-util` and `io-util` as dev features |
| `sqlx` | 0.9.0, features unchanged | `query!`, transactions, `postgres::PgListener` |
| `aws-sdk-sqs` | 1.120.0, features unchanged | `receive_message`, `delete_message_batch` |
| `aws-config` | 1.13.1 | the client factory of the shared kernel, reused |
| `async-trait` | 0.1.92 | the new ports |
| `thiserror` | 2.0.21 | the new error types |
| `uuid` | 1.29.2 | row and response types |
| `serde` | 1.0.229 | query parameters, WebSocket commands, response types |
| `serde_json` | 1.0.154 | tests only in the layer crates (oracle of the frame encoder) |
| `validator` | 0.21.0 | `ValidatedQuery` |
| `humantime` | 2.4.0 | `parse_rfc3339` for the `event-time` attribute, durations in the configuration |
| `futures-util` | 0.3.34 | `stream::unfold` for the SSE body |
| `tracing` | 0.1.44 | the log entries of NFR-15 |
| `utoipa`, `utoipa-axum` | 6.0.0, 0.3.0 | the two documented operations |
| `async-graphql` | 7.2.1 | the `eventTypes` query |
| `clap` | 4.6.7 | the new `serve` arguments |
| `aws-smithy-mocks`, `proptest`, `rstest`, `http-body-util`, `tower` | as in the manifest | tests |
| `tokio-tungstenite` | 0.29.0 (the version that the `ws` feature of `axum 0.8.9` resolves), added under "Tests" | WebSocket client of the tests in `crates/service` and `tests/e2e` only |
| `event-stream-domain`, `event-stream-application`, `event-stream-infrastructure`, `cluster-domain`, `cluster-application`, `cluster-infrastructure` | path entries, added | the six new crates |

## Constraints

(a) Commit convention: commit each story with the hooks active as `<type>(#4): story N.M <title>` (type one of feat, fix, refactor, build, chore, ci, docs, style, perf, test). Stay on the branch the plugin checked out and never commit on `main`; never push, the plugin command pushes; never bypass a hook.

(b) Story dispatch: a story whose `Depends on:` stories are done is independent.
When the rust-backend-sdlc:rust-implementer agent is available, independent stories may run as parallel subagents under the DB rule; otherwise implement stories sequentially under the same DB rule.
DB rule: parallel agents run only the DB-free targets `make.format` (make format), `make.lint_code` (make lint-code), `make.unit_tests` (make unit-tests), `make.architecture` (make architecture) and `make.complexity` (make complexity); one serialized verify step at a time runs the DB-backed targets `make.setup_test_db` (make setup-test-db), `make.integration_tests` (make integration-tests), `make.tests_with_coverage` (make tests-with-coverage), `make.e2e_tests` (make e2e-tests) and `make.sqlx_prepare` (make sqlx-prepare). A target that reads not available is skipped and reported.

- Protected: never edit an item of `protected-config.toml` (`architecture.toml`, `clippy.toml`, `deny.toml`, `.cargo/mutants.toml`, `make/`, hooks, `[workspace.lints]`, `thresholdsBuilder.js`, the `health` budgets); fix the code.
- Crates: layer crates use only the third-party crates of their `architecture.toml` list; no Kafka or Raft crate; WebSocket only through the axum `ws` feature; `event-stream` and `cluster` crates never depend on each other, leader gating lives in `crates/service`.
- Contracts: `/api/events/stream` and `/api/events/ws` stay out of the OpenAPI document (plain `.route(..)`) and are specified in `docs/streaming-protocol.md`; a story that changes a contract regenerates both snapshots in the same commit.
- Time and ids: time only through `Clock` and tokio time, ids only through `UuidGenerator`; lease expiry and pruning use the database `now()`; no timestamp type crosses sqlx (epoch microseconds as `bigint`).
- Lints: no `#[allow]`, `#[mutants::skip]`, `#[coverage(off)]` or exclusion; cognitive complexity 8, 50 lines, 5 arguments; name every enum variant in a `match`; never iterate a `HashMap`; logic sits in inherent methods, not in `#[async_trait]` bodies.
- Migrations: new pairs only (`make create-migration`), never edit `20261110000000_*`; the story that adds or changes a query runs `make sqlx-prepare` and commits `.sqlx/`.
- Model and docs: the story that adds a crate adds its component and relationships to `workspace.dsl`; a new variable lands in `cli.rs`, `config.rs`, `.env` and `docs/environment.md` together; a changed synchronised file is added to `.templatesyncignore`.
- Quality: 100 % line and function coverage, zero surviving or timed-out mutants; every background loop is a one-round `tick()`/`step()` of an application crate; a loop has a bounded range or an awaited pause; every awaiting test is bounded by `tokio::time::timeout`.
- Layers: infrastructure has no tokio (no sleep, timeout, spawn, select); `test-support` fakes use `std` only; `serde_json` is dev-only in layer crates.
- Delivery: publishing never awaits a subscriber (`try_send`, eviction on a full queue), no lock is held across `.await`; keep the example context and the three health probes; no authorization, Kafka adapter or publish endpoint.

## Context and Goals

The service becomes an open-source alternative to Mercure for VilnaCRM: a frontend reads the catalog of subscribable event types (FR-1, FR-2), subscribes over SSE or WebSocket (FR-4 to FR-11) and receives every event that enters through the VilnaCRM event bus (FR-14 to FR-17). Several nodes run against one PostgreSQL database; any node serves any client (FR-18, NFR-5), and one node at a time, the lease holder, reads the bus (FR-19, FR-20).

What exists today (read in the code): one binary `service` with three bounded contexts (`shared`, `health-check`, `some-module`), no background task, no per-connection state, no timestamp column, an SQS client used only by a health probe, and a `serve` command that drains HTTP on SIGTERM and exits with status 1 when a connection is still open after `SHUTDOWN_GRACE_PERIOD` (`crates/service/src/command/serve.rs`).

What this change adds:

- two bounded contexts, each with three crates: `notification/event-stream` and `internal/cluster`;
- three tables through two new migration pairs;
- four background loops plus two helper loops in the composition root, each scheduling a one-round use case;
- two documented REST operations, one GraphQL query, two streaming routes;
- eleven configuration variables (FR-25).

Structure of the data flow:

```text
SQS queue --(leader only)--> IngestEvents --> EventLog.append  (advisory lock, NOTIFY)
                                                  |
                           every node:  LISTEN or poll tick
                                                  v
                                RelayEvents.tick --> Hub --> bounded queue per subscriber --> SSE / WebSocket
                                                  ^
            Subscription: register in the hub, replay from the log after the cursor, then live
```

Goals that shape the design: the publish path never waits for a client (NFR-10); a connected client causes no database read per event (NFR-3); every loop is testable with paused tokio time and without sockets (NFR-16, NFR-19); no protected file changes (NFR-18).

## Verified Library Facts

The four verification points of the task, each read in the crate source of the locked version.

1. **axum 0.8.9.**
   - The feature is named `ws` (`ws = ["dep:hyper", "tokio", "dep:tokio-tungstenite", "dep:sha1", "dep:base64"]`); it pulls `tokio-tungstenite 0.29.0`, which is not in `Cargo.lock` today, so the lock file changes and `make.check_security` judges the new crates.
   - `axum::extract::ws::WebSocketUpgrade` implements `FromRequestParts` with rejection `WebSocketUpgradeRejection`; `on_upgrade(callback) -> Response` spawns the socket task itself; `max_message_size(usize)` exists. A request without hyper's upgrade state is rejected with 426, so `tower::ServiceExt::oneshot` never reaches a callback.
   - `Message` is `Text(Utf8Bytes) | Binary(Bytes) | Ping(Bytes) | Pong(Bytes) | Close(Option<CloseFrame>)`. `Utf8Bytes` is `Clone` (reference counted) and has `From<String>`, `From<&str>`, `as_str()`. `CloseFrame { code: u16, reason: Utf8Bytes }`; `close_code::AWAY` is 1001 and `close_code::AGAIN` is 1013.
   - `WebSocket::recv(&mut self) -> Option<Result<Message, axum::Error>>` and `WebSocket::send(&mut self, Message) -> Result<(), axum::Error>` are inherent methods. `WebSocket` also implements `Sink`, but `futures-util` is built with `alloc` only (no `sink` feature), so the socket is driven from one task with the inherent methods.
   - `axum::response::sse::{Sse, Event, KeepAlive}` exist under the default `tokio` feature. `Sse::new` takes a stream of `Result<Event, E>`; `Sse::keep_alive(KeepAlive::new().interval(d))` writes the comment `:\n\n` after `d` without an event and resets the timer on every event; the timer is `tokio::time::Sleep`, so paused time drives it. `Event` is `Debug + Clone`.
   - Two behaviours of `Event` that the design must respect: `Event::data("")` writes no `data:` line (a browser then drops the event), and `Event::data` starts a new `data:` line after every `\r` and every `\n`, so `\r\n` would arrive as two line breaks. `Event::id` and `Event::event` panic on a line break.
   - `axum::Json::<T>::from_bytes(&[u8]) -> Result<Json<T>, JsonRejection>` is public.
   - `utoipa_axum::router::OpenApiRouter::route(path, method_router)` is a pass-through to `axum::Router::route`: a route added with it is served and stays out of the document.
2. **tokio.** The workspace enables `macros`, `rt-multi-thread`, `net`, `signal`, `time`, `sync`. `tokio::sync::{mpsc, watch, Notify}` are behind `sync` and `tokio::time` behind `time`, so both are available to application crates, where `tokio` is on the allow-list. Domain crates may not use tokio at all, infrastructure crates only as a dev-dependency, `test-support` not at all. Paused time needs the dev feature `test-util` (already used by `shared-application`).
3. **sqlx 0.9.0.**
   - `sqlx::postgres::PgListener`: `connect_with(&PgPool)` takes one connection out of the pool for the life of the listener; `listen(&str)`; `recv() -> Result<PgNotification, Error>` reconnects and re-subscribes by itself and loses the notifications sent in the gap; `next_buffered() -> Option<PgNotification>` drains what is already buffered; it implements `Debug`.
   - No existing code reads or writes a `timestamptz`: the only table has no time column and the three `.sqlx` files hold none. The enabled features are `runtime-tokio`, `tls-rustls-aws-lc-rs`, `postgres`, `macros`, `migrate`, `uuid`; without `chrono` or `time` the query macros have no Rust type for `timestamptz` (the type table of `sqlx-postgres/src/type_checking.rs` lists the date and time types only in a `datetime-types` block keyed by those two features). The same table maps `void` to `()`, `bigint` to `i64` and arrays to slices, which is what the queries below rely on.
4. **aws-sdk-sqs 1.121.0 (locked).** `client.receive_message().queue_url(..).max_number_of_messages(i32).wait_time_seconds(i32).message_attribute_names(impl Into<String>)` (one call per name); `Message` has `message_id`, `receipt_handle`, `body` and `message_attributes: Option<HashMap<String, MessageAttributeValue>>` with `string_value`; `client.delete_message_batch().queue_url(..).set_entries(Some(Vec<DeleteMessageBatchRequestEntry>))`, the entry builder needs `id` and `receipt_handle` and its `build()` returns a `Result`; the output lists `failed: Vec<BatchResultErrorEntry>` with HTTP 200, so the adapter must read it.

Facts measured against the images the repository pins:

- **PostgreSQL 16.** Every statement of the section "Data Model and Queries" was prepared and executed; the parameter and result types are in that section.
- **LocalStack 3.4.0.** `SendMessage` and `ReceiveMessage` work with the JSON protocol and no signature; the queue URL is resolved by its path, whatever the host; `MessageId` is a version-4 UUID; an empty long poll with `WaitTimeSeconds=3` returned after 3.02 s; an empty `MessageBody` is accepted (AWS refuses it); a missing queue answers `QueueDoesNotExist`.
- **grafana/k6 v2.3.0 (the pinned image).** `k6/ws` and `k6/websockets` exist; there is no SSE module, and `k6/x/sse` would make K6 build another binary from the network. `http.asyncRequest` on a response that never ends resolves at its `timeout` with `status` 0, `error_code` 1050 and the part of the body received so far. A request-level tag `test_type` replaces the tag of the scenario, so such a request is outside `http_req_duration{test_type:smoke}`.

## Bounded Context notification/event-stream

Created with `make.new_context` (`group=notification context=event-stream`): `crates/notification/event-stream/{domain,application,infrastructure}`, packages `event-stream-domain`, `event-stream-application`, `event-stream-infrastructure`. The architecture gate derives the context name `event-stream` from the package names.

### Domain crate

Manifest: `async-trait`, `shared-domain`, `thiserror`; dev `proptest`, `rstest`.

| File under `crates/notification/event-stream/domain/src/` | Content |
| --- | --- |
| `lib.rs` | module declarations and re-exports |
| `error.rs` | `InvalidEvent`, `InvalidCursor` |
| `event.rs` | `NewEvent`, `Event` |
| `value_object.rs`, `value_object/event_type.rs` | `EventType` |
| `value_object/event_data.rs` | `EventData` |
| `value_object/sequence.rs` | `Sequence` |
| `value_object/cursor.rs` | `Cursor` |
| `port.rs`, `port/event_log.rs` | `EventLog`, `EventLogError` |
| `port/event_catalog.rs` | `EventCatalog`, `EventCatalogError` |
| `port/event_source.rs` | `EventSource`, `EventSourceError`, `InboundMessage` |
| `port/event_signal.rs` | `EventSignal`, `EventSignalError` |

Each file has a sibling `tests.rs` directory module, as in the example context.

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

/// Opaque UTF-8 text: at least 1 byte, at most `max_bytes` bytes, no U+0000.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventData(String);
impl EventData {
    pub fn new(value: String, max_bytes: usize) -> Result<Self, InvalidEvent>;
    pub fn as_str(&self) -> &str;
}

/// Position of a stored event in the log, 1 or greater.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sequence(u64);
impl Sequence {
    pub fn new(value: u64) -> Result<Self, InvalidEvent>;
    pub const fn value(self) -> u64;
}
impl std::fmt::Display for Sequence { /* decimal */ }

/// "Everything after this position"; 0 means the start of the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cursor(u64);
impl Cursor {
    pub const START: Self = Self(0);
    pub const fn new(value: u64) -> Self;
    /// ASCII digits only (no sign, no blank), then `u64`.
    pub fn parse(text: &str) -> Result<Self, InvalidCursor>;
    pub const fn value(self) -> u64;
}
impl From<Sequence> for Cursor { /* the same number */ }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEvent { pub id: Uuid, pub event_type: EventType, pub data: EventData, pub occurred_at: SystemTime }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event { pub sequence: Sequence, pub id: Uuid, pub event_type: EventType, pub data: EventData, pub occurred_at: SystemTime }
```

`Uuid` is `shared_domain::Uuid`; `SystemTime` is `std::time::SystemTime`, the type of the `Clock` port.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InvalidEvent {
    #[error("An event type must contain between 1 and 128 characters.")] TypeLength,
    #[error("An event type must match ^[a-z][a-z0-9]*([._-][a-z0-9]+)*$.")] TypePattern,
    #[error("A sequence must be 1 or greater.")] SequenceZero,
    #[error("Event data must not be empty.")] DataEmpty,
    #[error("Event data exceeds the size limit.")] DataTooLarge,
    #[error("Event data must not contain the NUL character.")] DataContainsNul,
}
impl InvalidEvent {
    /// `length`, `pattern`, `range`, `required`, `max_bytes`, `does_not_contain`.
    pub const fn code(self) -> &'static str;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("A cursor must be a decimal unsigned 64-bit integer.")]
pub struct InvalidCursor;
```

Ports. Each error is a struct around `BoxError`, like `shared_domain::CacheError`:

```rust
#[derive(Debug, thiserror::Error)] #[error("the event log is unavailable")]
pub struct EventLogError(#[source] pub BoxError);
#[derive(Debug, thiserror::Error)] #[error("the event catalog is unavailable")]
pub struct EventCatalogError(#[source] pub BoxError);
#[derive(Debug, thiserror::Error)] #[error("the event source is unavailable")]
pub struct EventSourceError(#[source] pub BoxError);
#[derive(Debug, thiserror::Error)] #[error("the event signal is unavailable")]
pub struct EventSignalError(#[source] pub BoxError);

#[async_trait::async_trait]
pub trait EventLog: std::fmt::Debug + Send + Sync {
    /// Appends the events in the given order. An event whose id is already stored is skipped.
    /// Returns the number of new rows.
    async fn append(&self, events: &[NewEvent]) -> Result<u64, EventLogError>;
    /// At most `limit` events with a sequence above `cursor`, in sequence order; `types` filters by type.
    async fn read_after(&self, cursor: Cursor, types: Option<&[EventType]>, limit: u32)
        -> Result<Vec<Event>, EventLogError>;
    /// The highest stored sequence, `None` for an empty log.
    async fn latest_sequence(&self) -> Result<Option<Sequence>, EventLogError>;
    /// Deletes the events recorded more than `retention` ago, by the clock of the store. Returns the count.
    async fn prune(&self, retention: std::time::Duration) -> Result<u64, EventLogError>;
}

#[async_trait::async_trait]
pub trait EventCatalog: std::fmt::Debug + Send + Sync {
    /// Registers the types; a type that is already registered stays as it is.
    async fn register(&self, types: &[EventType]) -> Result<(), EventCatalogError>;
    /// One page of types ordered by name ascending.
    async fn find_page(&self, page: PageRequest) -> Result<Vec<EventType>, EventCatalogError>;
}

/// One message of the bus, not validated. `receipt` is what `acknowledge` takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundMessage {
    pub receipt: String,
    pub message_id: String,
    pub event_type: Option<String>,
    pub event_id: Option<String>,
    pub event_time: Option<String>,
    pub body: String,
}

#[async_trait::async_trait]
pub trait EventSource: std::fmt::Debug + Send + Sync {
    /// Waits at most the adapter's wait time and returns at most one batch, possibly empty.
    async fn receive(&self) -> Result<Vec<InboundMessage>, EventSourceError>;
    /// Removes the messages from the bus; a partial failure is an error.
    async fn acknowledge(&self, receipts: &[String]) -> Result<(), EventSourceError>;
}

#[async_trait::async_trait]
pub trait EventSignal: std::fmt::Debug + Send {
    /// Completes when the log may hold new rows. An error means "no signal for now".
    async fn changed(&mut self) -> Result<(), EventSignalError>;
}
```

All four traits are dyn-compatible: no generic method, `&self` or `&mut self` receivers, used as `Arc<dyn ..>` (`Box<dyn EventSignal>` for the one with `&mut self`, which has a single owner).

> Assumption: event data must hold at least one byte and no U+0000. PostgreSQL `text` cannot store U+0000 (the insert would fail on every redelivery and block the batch), and an SSE event without a `data:` line is dropped by browsers. SQS itself refuses an empty body; LocalStack does not, so the rule is checked in the domain.
>
> Assumption: `Cursor` is its own value object because a client cursor may be 0 (FR-11) and a stored `Sequence` may not.

### Application crate

Manifest: `async-graphql`, `async-trait`, `axum`, `event-stream-domain`, `futures-util`, `humantime`, `serde`, `shared-application`, `shared-domain`, `thiserror`, `tokio`, `tracing`, `utoipa`, `utoipa-axum`, `validator`; dev `http-body-util`, `proptest`, `rstest`, `serde_json`, `test-support`, `tokio` with `test-util` and `io-util`, `tower`.

| File under `crates/notification/event-stream/application/src/` | Content |
| --- | --- |
| `lib.rs` | modules, re-exports, the rule below |
| `error.rs` | `EventStreamError` and `From<EventStreamError> for Problem` |
| `query.rs`, `query/list_event_types.rs` | `EventTypeView`, `ListEventTypes`, `ListEventTypesHandler` |
| `ingest.rs`, `ingest/envelope.rs` | `IngestEvents`, `IngestReport`, `IngestError`; `to_new_event`, `Rejected`, `InvalidMessage` |
| `relay.rs` | `EventPublisher`, `RelayEvents`, `RelayOutcome` |
| `listen.rs` | `ListenForEvents` |
| `prune.rs` | `PruneEvents` |
| `seed.rs` | `SeedEventTypes` |
| `http.rs` | `EventStreamState`, `StreamSettings`, `router()` |
| `http/dto.rs` | `ListEventTypesParams`, `EventTypeResponse`, `StreamParams`, `WsParams`, `Command`, `parse_types`, `parse_cursor` |
| `http/handler.rs` | `list_event_types`, `stream`, `ws`, `list_types` (shared with GraphQL) |
| `http/frame.rs` | `Frame`, `event_json`, `error_json`, `write_json_string` |
| `http/hub.rs` | `Hub`, `HubSettings`, `HubError`, `Delivery`, `Subscriber` |
| `http/subscription.rs` | `Subscriptions`, `Subscription`, `SubscriptionRequest`, `SubscribeError`, `EndReason` |
| `http/session.rs` | `Session`, `Inbound` (the socket-free WebSocket state machine) |
| `http/socket.rs` | `Connection`, `impl Connection for WebSocket`, `serve` |
| `graphql.rs` | `EventTypeObject`, `EventStreamQuery` |
| `tests/integration/{main.rs,support.rs,ws.rs}` (crate root, not `src/`) | the real-socket test of the upgrade |

Module rule, written into `lib.rs`: the use-case modules (`query`, `ingest`, `relay`, `listen`, `prune`, `seed`, `error`) do not import `axum`, `utoipa` or `async_graphql`; only `http` and `graphql` may.

> Assumption: the hub, the frames and the subscription live in the `http` module, and the relay reaches the hub through the trait `EventPublisher` that `relay.rs` owns. The hub carries frames that are already encoded for both transports (`axum` types), so it is a delivery adapter, and the use case stays free of `axum` as the module rule of the template demands.

Use cases:

```rust
// query.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventTypeView { pub name: String }

// query/list_event_types.rs
#[derive(Debug, Clone, Copy)]
pub struct ListEventTypes { pub page: u32, pub items_per_page: u32 }
impl Query for ListEventTypes { type Output = Vec<EventTypeView>; type Error = EventStreamError; const NAME: &'static str = "ListEventTypes"; }
pub struct ListEventTypesHandler { catalog: Arc<dyn EventCatalog> }
impl ListEventTypesHandler { pub const fn new(catalog: Arc<dyn EventCatalog>) -> Self; }

// error.rs
#[derive(Debug, thiserror::Error)]
pub enum EventStreamError {
    #[error(transparent)] InvalidPage(#[from] InvalidPageRequest),
    #[error("the event stream storage is unavailable")] Storage(#[source] BoxError),
}
// InvalidPage -> 422 violation on `page` or `itemsPerPage` (code `range`), Storage -> Problem::internal

// relay.rs
pub trait EventPublisher: std::fmt::Debug + Send + Sync {
    /// Hands one stored event to the local subscribers. Never waits.
    fn publish(&self, event: &Event);
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayOutcome { Drained, More }
#[derive(Debug)]
pub struct RelayEvents { log: Arc<dyn EventLog>, publisher: Arc<dyn EventPublisher>, poll_interval: Duration, cursor: Option<Cursor> }
impl RelayEvents {
    pub const PAGE_SIZE: u32 = 500;
    pub const MAX_PAGES_PER_TICK: u32 = 20;
    pub fn new(log: Arc<dyn EventLog>, publisher: Arc<dyn EventPublisher>, poll_interval: Duration) -> Self;
    /// One round: see "Relay Algorithm".
    pub async fn tick(&mut self) -> Result<RelayOutcome, EventLogError>;
    /// Returns when `wake` was notified or `poll_interval` elapsed, whichever is first.
    pub async fn wait(&self, wake: &tokio::sync::Notify);
    pub const fn cursor(&self) -> Option<Cursor>;
}

// listen.rs
#[derive(Debug)]
pub struct ListenForEvents { wake: Arc<tokio::sync::Notify>, retry_delay: Duration }
impl ListenForEvents {
    pub const fn new(wake: Arc<tokio::sync::Notify>, retry_delay: Duration) -> Self;
    /// One round: awaits `signal.changed()`; on success notifies `wake` once; on error logs
    /// `event signal failed` at warn and sleeps `retry_delay`.
    pub async fn step(&self, signal: &mut dyn EventSignal);
}

// ingest.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IngestReport { pub received: usize, pub appended: u64, pub invalid: usize }
#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error(transparent)] Source(#[from] EventSourceError),
    #[error(transparent)] Log(#[from] EventLogError),
    #[error(transparent)] Catalog(#[from] EventCatalogError),
}
#[derive(Debug)]
pub struct IngestEvents { source: Arc<dyn EventSource>, log: Arc<dyn EventLog>, catalog: Arc<dyn EventCatalog>, clock: Arc<dyn Clock>, max_data_bytes: usize }
impl IngestEvents {
    pub const fn new(source: Arc<dyn EventSource>, log: Arc<dyn EventLog>, catalog: Arc<dyn EventCatalog>,
                     clock: Arc<dyn Clock>, max_data_bytes: usize) -> Self;
    /// Phase 1, safe to cancel: one receive of the source.
    pub async fn receive(&self) -> Result<Vec<InboundMessage>, IngestError>;
    /// Phase 2, run to its end: validate, register, append, acknowledge. See "Ingestion Algorithm".
    pub async fn process(&self, batch: Vec<InboundMessage>) -> Result<IngestReport, IngestError>;
}

// ingest/envelope.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum InvalidMessage {
    #[error("the event-type attribute is missing")] MissingType,
    #[error("the event-type attribute is not an event type name")] Type,
    #[error("the event id is not a UUID")] Id,
    #[error("the event-time attribute is not an RFC 3339 UTC time")] Time,
    #[error("the body is empty, too large or holds the NUL character")] Data,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Rejected { pub(crate) message_id: String, pub(crate) reason: InvalidMessage }
pub(crate) fn to_new_event(message: InboundMessage, received_at: SystemTime, max_data_bytes: usize)
    -> Result<NewEvent, Rejected>;

// prune.rs
#[derive(Debug)]
pub struct PruneEvents { log: Arc<dyn EventLog>, retention: Duration }
impl PruneEvents {
    pub const fn new(log: Arc<dyn EventLog>, retention: Duration) -> Self;
    pub async fn tick(&self) -> Result<u64, EventLogError>;
}

// seed.rs
#[derive(Debug)]
pub struct SeedEventTypes { catalog: Arc<dyn EventCatalog>, types: Vec<EventType> }
impl SeedEventTypes {
    pub const fn new(catalog: Arc<dyn EventCatalog>, types: Vec<EventType>) -> Self;
    /// Registers `EVENT_TYPES`; with an empty list it succeeds without a call.
    pub async fn tick(&self) -> Result<(), EventCatalogError>;
}
```

Delivery adapter (`http`):

```rust
// http/frame.rs
/// One event encoded once for both transports.
#[derive(Debug)]
pub struct Frame {
    pub sequence: Sequence,
    pub event_type: EventType,
    pub sse: axum::response::sse::Event,          // id, event, data
    pub json: axum::extract::ws::Utf8Bytes,       // {"id":"..","type":"..","data":".."}
}
impl Frame { pub fn encode(event: &Event) -> Self; }
pub(crate) fn event_json(sequence: Sequence, event_type: &EventType, data: &str) -> String;
pub(crate) fn error_json(reason: &str) -> String;
/// Appends `text` as a JSON string: `"` and `\` escaped, U+0000..U+001F as \n, \r, \t or \u00XX, the rest as is.
pub(crate) fn write_json_string(out: &mut String, text: &str);

// http/hub.rs
#[derive(Debug, Clone, Copy)]
pub struct HubSettings { pub max_subscribers: usize, pub buffer: usize }
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HubError {
    #[error("the node is at its subscriber capacity")] AtCapacity,
    #[error("the node is shutting down")] Closed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Delivery { pub delivered: usize, pub evicted: usize }
pub struct Hub { settings: HubSettings, registry: std::sync::Mutex<Registry> }   // Debug by hand: the counts
impl Hub {
    pub fn new(settings: HubSettings) -> Arc<Self>;
    pub fn register(self: &Arc<Self>, types: &BTreeSet<EventType>) -> Result<Subscriber, HubError>;
    /// Fan-out of one encoded frame to the subscribers of its type.
    pub fn deliver(&self, frame: &Arc<Frame>) -> Delivery;
    /// Drops every subscriber; later `register` calls answer `Closed`.
    pub fn close(&self);
    pub fn is_closed(&self) -> bool;
    pub fn subscriber_count(&self) -> usize;
}
impl EventPublisher for Hub { /* delegates to an inherent method: skip when the type has no subscriber, else Frame::encode and deliver */ }

pub struct Subscriber { id: u64, receiver: tokio::sync::mpsc::Receiver<Arc<Frame>>, hub: Arc<Hub> }
impl Subscriber {
    pub async fn recv(&mut self) -> Option<Arc<Frame>>;
    pub fn subscribe(&self, types: &BTreeSet<EventType>);
    pub fn unsubscribe(&self, types: &BTreeSet<EventType>);
    pub fn hub_closed(&self) -> bool;
}
impl Drop for Subscriber { /* removes the subscriber from the hub */ }

// private
struct Registry {
    closed: bool,
    next_id: u64,
    types_of: BTreeMap<u64, BTreeSet<EventType>>,
    by_type: HashMap<EventType, BTreeMap<u64, tokio::sync::mpsc::Sender<Arc<Frame>>>>,
}
```

`by_type` is only read and written by key; everything that is iterated is a `BTreeMap` or a `BTreeSet` (`iter_over_hash_type`). A poisoned mutex is taken over with `unwrap_or_else(PoisonError::into_inner)`, as `test-support` does. The subscriber id is a counter of the hub, not an identifier of the domain, so it does not come from `UuidGenerator`.

```rust
// http/subscription.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionRequest { pub types: BTreeSet<EventType>, pub cursor: Option<Cursor> }
#[derive(Debug, thiserror::Error)]
pub enum SubscribeError {
    #[error(transparent)] Hub(#[from] HubError),
    #[error(transparent)] Log(#[from] EventLogError),
}
// From<SubscribeError> for Problem: Hub(_) -> 503 `about:blank` with the error text as detail (FR-23),
// logged at warn as `subscription rejected`; Log(_) -> Problem::internal (NFR-11).
#[derive(Debug)]
pub struct Subscriptions { hub: Arc<Hub>, log: Arc<dyn EventLog> }
impl Subscriptions {
    pub const fn new(hub: Arc<Hub>, log: Arc<dyn EventLog>) -> Self;
    pub async fn open(&self, request: SubscriptionRequest) -> Result<Subscription, SubscribeError>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason { Evicted, HubClosed, LogFailed }
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a connection subscribes to at most 100 event types")]
pub struct TooManyTypes;
pub struct Subscription { /* subscriber, log, types, floor: Option<Cursor>, replaying: bool, pending: VecDeque<Arc<Frame>>, end: Option<EndReason> */ }
impl Subscription {
    pub const MAX_TYPES: usize = 100;
    pub const REPLAY_PAGE: u32 = 500;
    /// The next frame to write; `None` ends the stream, and `end_reason` then says why.
    pub async fn next(&mut self) -> Option<Arc<Frame>>;
    pub fn subscribe(&mut self, types: BTreeSet<EventType>) -> Result<(), TooManyTypes>;
    pub fn unsubscribe(&mut self, types: &BTreeSet<EventType>);
    pub const fn end_reason(&self) -> Option<EndReason>;
    pub fn type_count(&self) -> usize;
    pub const fn last_sequence(&self) -> Option<Cursor>;
}

// http/session.rs: no socket, no timer
pub(crate) enum Inbound { Continue, Reply(Message), Close }
pub(crate) struct Session { subscription: Subscription, max_command_bytes: usize }
impl Session {
    pub(crate) const fn new(subscription: Subscription, max_command_bytes: usize) -> Self;
    /// Every `Message` variant is named: Text -> command, Binary -> error frame, Ping and Pong -> Continue, Close -> Close.
    pub(crate) fn on_message(&mut self, message: Message) -> Inbound;
    pub(crate) async fn next_frame(&mut self) -> Option<Arc<Frame>>;
    /// Close frame by `end_reason`: Evicted -> 1013, HubClosed -> 1001, LogFailed -> 1011.
    pub(crate) fn close_message(&self) -> Message;
}

// http/socket.rs
pub(crate) trait Connection: Send {
    fn receive(&mut self) -> impl Future<Output = Option<Result<Message, axum::Error>>> + Send;
    fn transmit(&mut self, message: Message) -> impl Future<Output = Result<(), axum::Error>> + Send;
}
impl Connection for axum::extract::ws::WebSocket { /* recv and send, nothing else */ }
/// The pump: one task, `tokio::select!` over `connection.receive()`, `session.next_frame()` and the ping interval.
pub(crate) async fn serve<C: Connection>(connection: C, session: Session, keep_alive: Duration);

// http.rs
#[derive(Debug, Clone, Copy)]
pub struct StreamSettings { pub keep_alive: Duration, pub max_command_bytes: usize }
#[derive(Debug, Clone)]
pub struct EventStreamState { pub query_bus: Arc<dyn QueryBus>, pub subscriptions: Arc<Subscriptions>, pub settings: StreamSettings }
pub fn router<S>() -> OpenApiRouter<S>
where S: Clone + Send + Sync + 'static, EventStreamState: FromRef<S>
{
    OpenApiRouter::new()
        .routes(routes!(handler::list_event_types))
        .route("/events/stream", get(handler::stream))   // served, not documented
        .route("/events/ws", get(handler::ws))           // served, not documented
}
```

The trait methods are named `receive` and `transmit` so that they do not collide with the inherent `recv` and `send` (`same_name_method`).

HTTP handlers (`http/handler.rs`) and DTOs (`http/dto.rs`):

```rust
#[derive(Debug, Clone, Copy, Deserialize, IntoParams, Validate)]
#[serde(rename_all = "camelCase", default)]
#[into_params(parameter_in = Query)]
pub struct ListEventTypesParams { pub page: i32, pub items_per_page: i32 }   // same attributes as ListExampleApiResourcesParams

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[schema(as = EventType)]
pub struct EventTypeResponse {
    #[schema(min_length = 1, max_length = 128, pattern = r"^[a-z][a-z0-9]*([._-][a-z0-9]+)*$", example = "user.created")]
    pub name: String,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct StreamParams { pub types: String, pub last_event_id: Option<String> }
#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct WsParams { pub types: Option<String>, pub last_event_id: Option<String> }

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "lowercase", deny_unknown_fields)]
pub(crate) enum Command { Subscribe { types: Vec<String> }, Unsubscribe { types: Vec<String> } }

/// Comma-separated names -> a set of 1 to 100 valid types; the error is the 422 problem with one violation on `types`.
pub(crate) fn parse_types(raw: &str) -> Result<BTreeSet<EventType>, Problem>;
/// `Last-Event-ID` wins over `lastEventId`; the error is the 400 problem whose detail names the carrier.
pub(crate) fn parse_cursor(headers: &HeaderMap, query: Option<&str>) -> Result<Option<Cursor>, Problem>;

#[utoipa::path(get, path = "/event-types", operation_id = "api_event_types_get", tag = "EventStream", /* 200, 400, 422, default */)]
pub async fn list_event_types(State(state): State<EventStreamState>,
    ValidatedQuery(params): ValidatedQuery<ListEventTypesParams>) -> Result<Json<Vec<EventTypeResponse>>, Problem>;

pub async fn stream(State(state): State<EventStreamState>, headers: HeaderMap,
    ValidatedQuery(params): ValidatedQuery<StreamParams>) -> Result<Response, Problem>;

pub async fn ws(State(state): State<EventStreamState>, headers: HeaderMap,
    ValidatedQuery(params): ValidatedQuery<WsParams>,
    upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>) -> Result<Response, Problem>;
```

- The two parameter structs derive `Validate` without a field rule: `ValidatedQuery` gives the problem-shaped 400 of a query that cannot be parsed (a missing `types` on the SSE route), and `parse_types` gives the 422, so one function serves the query parameter and the run-time commands.
- Violation codes on `types`: `required` (empty list or an empty name), `length`, `pattern`, `max_items` (more than 100 distinct names).
- `stream`: parse, `subscriptions.open(..)`, then `Sse::new(unfold(subscription, ..)).keep_alive(KeepAlive::new().interval(keep_alive)).into_response()`. The `unfold` step is `subscription.next().await` mapped to `Ok::<Event, Infallible>(frame.sse.clone())`; when it ends with `Evicted`, one `subscriber evicted` entry is logged (transport `sse`, type count, last sequence).
- `ws`, in this order: parse (422 or 400), `subscriptions.open(..)` (503 or 500), then the upgrade result: a rejection becomes `Problem::new(rejection.status()).with_detail(rejection.body_text())`, a success becomes `upgrade.max_message_size(max_command_bytes.saturating_mul(2)).on_upgrade(move |socket| socket::serve(socket, session, keep_alive))`. With this order every branch but the last line is reached by `oneshot` in a unit test.
- GraphQL (`graphql.rs`): `#[graphql(name = "EventType")] struct EventTypeObject { name: String }` and `EventStreamQuery::event_types(ctx, #[graphql(default = 1)] page: i32, #[graphql(default = 30)] items_per_page: i32) -> async_graphql::Result<Vec<EventTypeObject>>`. It validates through `ListEventTypesParams` and calls `handler::list_types`, the same function the REST handler calls, then `problem_to_error`. The SDL field is `eventTypes(itemsPerPage: Int! = 30, page: Int! = 1): [EventType!]!`.

> Assumption: a WebSocket text message larger than `HTTP_MAX_BODY_BYTES` is answered with an error frame (FR-10) as long as it is at most twice that size; the transport limit `max_message_size` is twice the value, and a larger message ends the connection. Without a transport limit one client could make the node buffer 64 MiB per connection.

### Infrastructure crate

Manifest: `async-trait`, `aws-sdk-sqs`, `event-stream-domain`, `shared-domain`, `sqlx`, `thiserror`, `uuid`; dev `aws-sdk-sqs` with `test-util`, `aws-smithy-mocks`, `shared-infrastructure`, `test-support`, `tokio`.

| File under `crates/notification/event-stream/infrastructure/src/` | Content |
| --- | --- |
| `lib.rs` | modules and re-exports |
| `persistence.rs`, `persistence/event_log.rs` | `PgEventLog`, `APPEND_LOCK_KEY`, `NOTIFY_CHANNEL` |
| `persistence/event_catalog.rs` | `PgEventCatalog` |
| `persistence/event_signal.rs` | `PgEventSignal` |
| `messaging.rs` | `SqsEventSource`, `SqsEventSourceSettings` |
| `tests/integration/{main.rs,support.rs,migration.rs,event_log.rs,event_catalog.rs,event_signal.rs,event_source.rs}` (crate root) | against PostgreSQL and LocalStack |

```rust
pub const APPEND_LOCK_KEY: i64 = 4_004_000_001;          // the only advisory lock of the service
pub const NOTIFY_CHANNEL: &str = "notification_events";

#[derive(Debug)] pub struct PgEventLog { pool: sqlx::PgPool }
impl PgEventLog { pub const fn new(pool: sqlx::PgPool) -> Self; }

#[derive(Debug)] pub struct PgEventCatalog { pool: sqlx::PgPool }
impl PgEventCatalog { pub const fn new(pool: sqlx::PgPool) -> Self; }

/// Owns a pool of one connection; the listener is created on the first `changed` call.
#[derive(Debug)] pub struct PgEventSignal { pool: sqlx::PgPool, listener: Option<sqlx::postgres::PgListener> }
impl PgEventSignal { pub const fn new(pool: sqlx::PgPool) -> Self; }

#[derive(Debug, Clone)]
pub struct SqsEventSourceSettings { pub queue_url: String, pub max_messages: i32, pub wait: Duration }
#[derive(Debug)] pub struct SqsEventSource { client: aws_sdk_sqs::Client, settings: SqsEventSourceSettings }
impl SqsEventSource { pub const fn new(client: aws_sdk_sqs::Client, settings: SqsEventSourceSettings) -> Self; }
```

- Every `#[async_trait]` method only delegates to an inherent method (ADR 0008, as `PgExampleApiResourceRepository` does); rows are private structs; one `translate` function per adapter maps `sqlx::Error` to the port error.
- `PgEventSignal::changed`: when `listener` is `None`, `PgListener::connect_with(&pool)` and `listen(NOTIFY_CHANNEL)`, stored in `self` before the wait; then `recv()`; on success drain `next_buffered()` until `None` (many notifications become one wake-up); on error set `listener` to `None` and return the error. The adapter never sleeps or retries (no tokio in this layer).
- `SqsEventSource::receive`: `receive_message()` with `max_number_of_messages(settings.max_messages)`, `wait_time_seconds(whole seconds of settings.wait)` and the three attribute names `event-type`, `event-id`, `event-time`. A message without a receipt handle is an error of the call. `acknowledge`: nothing for an empty list; otherwise `delete_message_batch` per chunk of 10, entry ids `"0"` to `"9"`, and a non-empty `failed` list is an `EventSourceError` that carries the count.
- Time conversion in `PgEventLog`: `occurred_at.duration_since(UNIX_EPOCH)` as microseconds through `i64::try_from` (a time before the epoch is an `EventLogError`), and back with `UNIX_EPOCH.checked_add(Duration::from_micros(..))`. A cursor above `i64::MAX` is bound as `i64::MAX`.

> Assumption: `APPEND_LOCK_KEY`, `NOTIFY_CHANNEL`, the lease name `leader`, the page sizes (500 rows, 20 pages per relay round) and the attribute names are constants of the code, not configuration. The append always sends the `NOTIFY`, also when every id was a duplicate: a needless wake-up costs one indexed read per node, and the statement sequence has no branch.

## Bounded Context internal/cluster

Created with `make.new_context` (`group=internal context=cluster`): `crates/internal/cluster/{domain,application,infrastructure}`, packages `cluster-domain`, `cluster-application`, `cluster-infrastructure`.

### Domain crate

Manifest: `async-trait`, `shared-domain`, `thiserror`; dev `rstest`. Files: `lib.rs`, `error.rs` (`InvalidTerm`), `value_object.rs`, `value_object/node_id.rs`, `value_object/term.rs`, `value_object/leadership.rs` (`Leadership`, `Role`), `port.rs` (`LeaseStore`, `LeaseStoreError`).

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(Uuid);                                   // shared_domain::Uuid
impl NodeId { pub const fn new(value: Uuid) -> Self; pub const fn value(self) -> Uuid; }
impl std::fmt::Display for NodeId { /* the UUID */ }

/// Fencing token: 1 or greater, +1 on every change of holder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Term(u64);
impl Term { pub fn new(value: u64) -> Result<Self, InvalidTerm>; pub const fn value(self) -> u64; }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role { Leader, Follower }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Leadership { pub leader: NodeId, pub term: Term }
impl Leadership { pub fn role_of(&self, node: NodeId) -> Role; }

#[derive(Debug, thiserror::Error)] #[error("the lease store is unavailable")]
pub struct LeaseStoreError(#[source] pub BoxError);

#[async_trait::async_trait]
pub trait LeaseStore: std::fmt::Debug + Send + Sync {
    /// One election round for `node`: take the lease when it is free or expired, renew it when `node`
    /// holds it, and report who holds it now. Expiry is judged by the clock of the store.
    async fn acquire_or_renew(&self, node: NodeId, ttl: std::time::Duration) -> Result<Leadership, LeaseStoreError>;
    /// Expires the lease now when `node` holds it; the record and its term stay.
    async fn release(&self, node: NodeId) -> Result<(), LeaseStoreError>;
}
```

### Application crate

Manifest: `axum`, `cluster-domain`, `serde`, `shared-application`, `shared-domain`, `tokio`, `tracing`, `utoipa`, `utoipa-axum`, `uuid`; dev `http-body-util`, `rstest`, `serde_json`, `test-support`, `tokio` with `test-util`, `tower`. Files: `lib.rs`, `elector.rs` (`Elector`, `ElectorSettings`, `ClusterStatus`), `http.rs` (`ClusterState`, `router()`), `http/dto.rs` (`ClusterStatusResponse`, `RoleResponse`), `http/handler.rs` (`status`).

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClusterStatus { pub node_id: NodeId, pub role: Role, pub leader: Option<NodeId>, pub term: Option<Term> }

#[derive(Debug, Clone, Copy)]
pub struct ElectorSettings { pub ttl: Duration, pub call_timeout: Duration }

#[derive(Debug)]
pub struct Elector { node: NodeId, store: Arc<dyn LeaseStore>, settings: ElectorSettings, status: tokio::sync::watch::Sender<ClusterStatus> }
impl Elector {
    /// Starts as follower with no leader and no term.
    pub fn new(node: NodeId, store: Arc<dyn LeaseStore>, settings: ElectorSettings) -> Self;
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<ClusterStatus>;
    /// One election round; see "Election Algorithm". Returns the published status.
    pub async fn campaign(&self) -> ClusterStatus;
    /// Releases the lease when this node is leader and publishes follower. Used at shutdown.
    pub async fn resign(&self) -> ClusterStatus;
}

// http.rs
#[derive(Debug, Clone)]
pub struct ClusterState { pub status: tokio::sync::watch::Receiver<ClusterStatus> }
pub fn router<S>() -> OpenApiRouter<S> where S: Clone + Send + Sync + 'static, ClusterState: FromRef<S>;

// http/dto.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")] #[schema(as = ClusterRole)]
pub enum RoleResponse { Leader, Follower }
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")] #[schema(as = ClusterStatus)]
pub struct ClusterStatusResponse { pub node_id: uuid::Uuid, pub role: RoleResponse, pub leader: Option<uuid::Uuid>, pub term: u64 }

// http/handler.rs
#[utoipa::path(get, path = "/cluster", operation_id = "api_cluster_get", tag = "Cluster", /* 200, default */)]
pub async fn status(State(state): State<ClusterState>) -> Json<ClusterStatusResponse>;
```

The handler reads `state.status.borrow()` and maps it; `term` is 0 while no round has succeeded, and `leader` is serialised as `null` then (FR-21). It awaits nothing, so it carries the attribute the template already uses for such handlers: `#[cfg_attr(not(test), expect(clippy::unused_async, reason = "axum handlers must be async"))]`. It does not use a bus, like the health handler: the status must be readable when the database is down.

### Infrastructure crate

Manifest: `async-trait`, `cluster-domain`, `shared-domain`, `sqlx`, `uuid`; dev `shared-infrastructure`, `test-support`, `tokio`. Files: `lib.rs`, `persistence.rs` (`PgLeaseStore`, `LEASE_NAME`), `tests/integration/{main.rs,migration.rs,lease_store.rs}`.

```rust
pub const LEASE_NAME: &str = "leader";
#[derive(Debug)] pub struct PgLeaseStore { pool: sqlx::PgPool }
impl PgLeaseStore { pub const fn new(pool: sqlx::PgPool) -> Self; }
```

## Composition Root

All changes are in `crates/service`; its manifest gains the six crates, and `tokio-tungstenite` plus `futures-util` with `sink` as dev-dependencies.

### Adapters

New public fields of `bootstrap::Adapters`, each replaceable by a test:

```rust
pub event_log: Arc<dyn EventLog>,
pub event_catalog: Arc<dyn EventCatalog>,
pub event_source: Option<Arc<dyn EventSource>>,     // None when EVENT_QUEUE_URL is unset
pub event_signal: Box<dyn EventSignal>,
pub lease_store: Arc<dyn LeaseStore>,
```

`Adapters::connect` still touches no network:

- `PgEventLog`, `PgEventCatalog` and `PgLeaseStore` share the existing lazy pool.
- `PgEventSignal` gets its own lazy pool: `connect_lazy(&PostgresSettings { max_connections: 1, ..config.database.clone() })`. The listener therefore never takes a connection away from requests, and a node opens at most `DATABASE_MAX_CONNECTIONS + 1` connections (NFR-12).
- When `EVENT_QUEUE_URL` is set, a second SQS client is built for ingestion with the same factory and a longer budget: `sqs_client(&SqsSettings { operation_timeout: config.sqs.operation_timeout + QUEUE_WAIT, ..config.sqs.clone() })`, and `SqsEventSource::new(client, SqsEventSourceSettings { queue_url, max_messages: 10, wait: QUEUE_WAIT })`. `QUEUE_WAIT` is the constant 5 s of FR-15. The client of the health probe, `SQS_OPERATION_TIMEOUT` and the three cross-field rules of `config.rs` are unchanged.

### Application, state, routes, schema

```rust
// bootstrap.rs
pub struct Application { pub router: Router, pub background: Background }
pub fn application(adapters: Adapters, config: &Config) -> Result<Application, BootstrapError>;
pub fn router(adapters: Adapters, config: &Config) -> Result<Router, BootstrapError>;   // kept: application(..)?.router
```

`application` does, besides today's wiring:

1. node id: `config.cluster.node_id`, or `adapters.uuid_generator.generate()` when `NODE_ID` is unset;
2. `Hub::new(HubSettings { max_subscribers, buffer })`, `Subscriptions::new(hub, event_log)`;
3. `Elector::new(node, lease_store, ElectorSettings { ttl: CLUSTER_LEASE_TTL, call_timeout: CLUSTER_RENEW_INTERVAL })`;
4. `register_queries` also registers `ListEventTypesHandler::new(event_catalog)`;
5. `AppState` gains `event_stream: EventStreamState` and `cluster: ClusterState` (the derive `FromRef` covers them);
6. the schema gets `.data(event_stream.clone())`;
7. `Background` is filled (below).

Routes: `openapi::api_router()` adds `.nest("/api", event_stream_application::http::router())` and `.nest("/api", cluster_application::http::router())`, and `ApiDoc` gains the tags `EventStream` ("Event catalog endpoints.") and `Cluster` ("Cluster status endpoints."). `http::router::assemble` needs no change: the streaming routes arrive inside the context router and get the same eight middleware layers. The request budget wraps only the handler future, which returns the response head; the body stream and the upgraded socket are outside it. `DefaultBodyLimit` does not apply to an upgraded socket.

GraphQL: `pub struct Query(ExampleApiResourceQuery, EventStreamQuery);` in `graphql/schema.rs`.

### Configuration

`cli.rs` gets one `Option<String>` argument per variable with `env = ".."` and `hide_env_values = true`; `config.rs` parses them into three new members of `Config`:

```rust
pub struct ClusterConfig { pub node_id: Option<Uuid>, pub lease_ttl: Duration, pub renew_interval: Duration }
pub struct EventsConfig { pub queue_url: Option<String>, pub retention: Duration, pub relay_poll_interval: Duration,
                          pub max_data_bytes: usize, pub types: Vec<EventType> }
pub struct StreamsConfig { pub subscriber_buffer: usize, pub max_subscribers: usize, pub keep_alive: Duration }
```

| Variable | Type | Default | Bounds and reason text |
| --- | --- | --- | --- |
| `NODE_ID` | UUID | unset: generated by `UuidGenerator` | `must be a UUID` |
| `CLUSTER_LEASE_TTL` | duration | `10s` | `1s` to `5m` |
| `CLUSTER_RENEW_INTERVAL` | duration | `3s` | `100ms` or more; cross-field rule below |
| `EVENT_QUEUE_URL` | `http://` or `https://` URL | unset: the node never ingests | the existing `endpoint` check |
| `EVENT_RETENTION` | duration | `1h` | `1m` to `30d` |
| `EVENT_RELAY_POLL_INTERVAL` | duration | `1s` | `100ms` to `1m` |
| `EVENT_MAX_DATA_BYTES` | integer | `65536` | 1 to 1 048 576 |
| `EVENT_TYPES` | comma-separated event type names | unset: nothing registered at start | every name a valid `EventType`: `must be a comma-separated list of event type names` |
| `SUBSCRIBER_BUFFER` | integer | `256` | 1 to 65 536 |
| `MAX_SUBSCRIBERS` | integer | `100000` | 1 to 1 000 000 |
| `STREAM_KEEP_ALIVE` | duration | `15s` | `1s` to `5m` |

Cross-field rule, added to `Collector::cross_field_rules` after the three existing ones: `2 * CLUSTER_RENEW_INTERVAL < CLUSTER_LEASE_TTL`, reported on `CLUSTER_RENEW_INTERVAL` with the reason `must be shorter than half of CLUSTER_LEASE_TTL`. A new helper `bounded_duration(raw, default, range)` serves the duration rows. No reason text holds a value (FR-25). `service check-config` uses the same code.

Constants of `crates/service` that are not variables: `QUEUE_WAIT = 5 s`, `QUEUE_BATCH = 10`, `PRUNE_INTERVAL = 60 s`, `RETRY_DELAY = 1 s`.

> Assumption: the long-poll wait, the batch size, the prune period and the retry delay are constants. FR-25 fixes the list of variables at eleven, and none of the four needs tuning per deployment; the adapter takes them as settings, so tests use shorter values.

Each variable gets its row in `docs/environment.md`, which rule 5 of `make.docs_check` compares with the help of `serve`, the compose files and `.env`. `.env` gets `EVENT_QUEUE_URL=http://${LOCALSTACK_HOST}:${LOCALSTACK_PORT}/000000000000/notification-events` and the nine variables that have a default, each written with that default.

> Assumption: `NODE_ID` and `EVENT_TYPES` have no line in `.env`. A node id in a shared file would give two local nodes the same identity, and an empty `EVENT_TYPES=` is not the same as an unset variable. FR-25 asks for an entry per variable; these two are documented in `docs/environment.md` only.

### Background tasks, supervision and shutdown

`crates/service/src/background.rs` (new, declared in `lib.rs`) holds everything that is scheduled. The use cases are in the application crates; this module only loops.

```rust
pub struct Schedule { pub renew_interval: Duration, pub prune_interval: Duration, pub retry_delay: Duration }
pub struct Background {
    pub elector: Arc<Elector>,
    pub relay: RelayEvents,
    pub listen: ListenForEvents,
    pub signal: Box<dyn EventSignal>,
    pub wake: Arc<tokio::sync::Notify>,
    pub ingest: Option<Arc<IngestEvents>>,
    pub prune: Arc<PruneEvents>,
    pub seed: SeedEventTypes,
    pub hub: Arc<Hub>,
    pub schedule: Schedule,
}
impl Background { pub fn spawn(self) -> Running; }
pub struct Running { /* stop: watch::Sender<bool>, tasks: Vec<JoinHandle<()>>, elector, hub */ }
impl Running { pub async fn shutdown(self); }
```

| Loop | One round | Pause between rounds |
| --- | --- | --- |
| elect | `elector.campaign()` | `renew_interval` |
| seed | `seed.tick()`; the loop ends at the first success | `retry_delay` after a failure |
| listen | `listen.step(signal)`, cancelled only by the stop flag | none (the round waits itself) |
| relay | `relay.tick()`; an error is logged once as `relay failed` | `relay.wait(&wake)`; after `More` only `tokio::task::yield_now()` |
| ingest (only when `ingest` is `Some`) | while the status is leader: `ingest.receive()` (cancelled by the stop flag), then `ingest.process(batch)` to its end; an error is logged once as `ingestion failed` | `retry_delay` after an error; while follower the loop waits for a change of the status or the stop flag |
| prune | when the status is leader: `prune.tick()`; an error is logged as `prune failed` | `prune_interval` |

Rules of the loops:

- Shape: `while running(&stop) { round; pause }`. `running` is false when the flag is true or its sender is gone. Every pause is `tokio::select!` over `tokio::time::sleep(..)` and the stop flag, so every loop ends within one pause and paused time drives every test.
- Leader gating is here and nowhere else: the ingest and prune loops read `watch::Receiver<ClusterStatus>` from `elector.subscribe()` and compare `role` with `Role::Leader`. No closure, port or type of the cluster context enters an `event-stream` crate.
- Supervision: a loop never returns an error; a failed round is logged with its error chain and retried at the next round. A task that ends by a panic is reported at shutdown as `background task failed` (error level); it is not restarted, because the lint policy leaves no panicking path in production code and a panic is a defect.

> Assumption: `bootstrap::application` and the `Background` and `Running` types are the public seam for tests: an integration or end-to-end test builds an `Application`, replaces adapters before that, and either calls the one-round use cases directly or spawns the loops. `bootstrap::router` keeps its signature, so the existing tests and the per-request world of the Gherkin suite compile unchanged.

`serve` (`command/serve.rs`) after `bind`: `let running = application.background.spawn();`, then the server task as today. After the signal, inside one `tokio::time::timeout(config.shutdown_grace_period, ..)` (FR-24):

1. `running.shutdown()`: set the stop flag and join the loop tasks; then `elector.resign()`; then `hub.close()`.
2. `stop.send_replace(true)` and await the server task.

`hub.close()` ends every SSE body (`Subscription::next` returns `None`), and every WebSocket task sends close code 1001 and ends, so the HTTP drain completes and `serve` exits with status 0. Without step 1 an open stream would hold the drain until the grace period ends and the exit status would be 1. A subscription request that arrives after `close` gets the 503 problem. Exit statuses and the entries `shutdown signal received`, `stopped` and `grace period elapsed, forcing shutdown` are unchanged.

Log entries (NFR-15). The message texts are fixed, because the tests read them; no entry holds event data, an event type name taken from a request, or the value of a variable.

| Message | Level | Fields | Written by |
| --- | --- | --- | --- |
| `leadership changed` | info | `node_id`, `role`, `leader`, `term` | `Elector` |
| `lease store failed` | warn | `error` (chain) | `Elector` |
| `subscriber evicted` | warn | `transport` (`sse` or `websocket`), `types` (count), `last_sequence` | the `stream` handler and `socket::serve` |
| `subscription rejected` | warn | `reason` | `From<SubscribeError> for Problem` |
| `inbound message rejected` | warn | `message_id`, `reason` | `IngestEvents` |
| `event signal failed` | warn | `error` (chain) | `ListenForEvents` |
| `ingestion failed`, `relay failed`, `prune failed`, `catalog seed failed` | error | `error` (chain), once per failed round | the loops of `background.rs` |
| `background task failed` | error | `task` | `Running::shutdown` |

Files of the template sync that this section changes and that are not yet ignored: `crates/service/src/command/serve.rs`, `docker-compose.yml`, `docker-compose.override.yml`, `docker-compose.load_test.override.yml`, `docs/architecture.md`, `docs/operations.md`, `docs/load-testing.md`, `docs/README.md`, `SECURITY.md`. Each goes into `.templatesyncignore` in the story that changes it.

## Data Model and Queries

Two reversible pairs, created with `make.create_migration` (`name=create_notification_events`, `name=create_cluster_leases`); the timestamp prefix is the one the command writes. The existing pair `20261110000000_create_example_api_resources` is not touched.

`<timestamp>_create_notification_events.up.sql`:

```sql
CREATE TABLE notification_events (
    sequence BIGINT GENERATED ALWAYS AS IDENTITY,
    id UUID NOT NULL,
    event_type TEXT NOT NULL,
    data TEXT NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT notification_events_pkey PRIMARY KEY (sequence),
    CONSTRAINT notification_events_id_key UNIQUE (id),
    CONSTRAINT notification_events_event_type_check
        CHECK (char_length(event_type) BETWEEN 1 AND 128
            AND event_type ~ '^[a-z][a-z0-9]*([._-][a-z0-9]+)*$'),
    CONSTRAINT notification_events_data_check CHECK (octet_length(data) >= 1)
);

CREATE INDEX notification_events_event_type_sequence_idx
    ON notification_events (event_type, sequence);

CREATE INDEX notification_events_recorded_at_idx
    ON notification_events (recorded_at);

CREATE TABLE notification_event_types (
    name TEXT NOT NULL,
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT notification_event_types_pkey PRIMARY KEY (name),
    CONSTRAINT notification_event_types_name_check
        CHECK (char_length(name) BETWEEN 1 AND 128
            AND name ~ '^[a-z][a-z0-9]*([._-][a-z0-9]+)*$')
);
```

`<timestamp>_create_notification_events.down.sql`:

```sql
DROP TABLE notification_event_types;
DROP TABLE notification_events;
```

`<timestamp>_create_cluster_leases.up.sql`:

```sql
CREATE TABLE cluster_leases (
    name TEXT NOT NULL,
    holder UUID NOT NULL,
    term BIGINT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT cluster_leases_pkey PRIMARY KEY (name),
    CONSTRAINT cluster_leases_term_check CHECK (term >= 1)
);
```

`<timestamp>_create_cluster_leases.down.sql`:

```sql
DROP TABLE cluster_leases;
```

How time crosses the sqlx boundary: it does not cross as a time type.

- `occurred_at` is bound and read as microseconds since the epoch (`i64`) and converted in SQL.
- `recorded_at`, `first_seen_at` and `expires_at` are written by the database (`now()`) and never selected. A duration (lease time, retention) is bound as milliseconds (`i64`).
- The lease statements send no timestamp of a node.

Every query, as it is written in `sqlx::query!` (the text is the key of the `.sqlx` file, so it is used verbatim). The types are what PostgreSQL 16 reported for the prepared statement.

| # | Adapter method | Statement | Parameters | Columns |
| --- | --- | --- | --- | --- |
| Q1 | `PgEventLog::append` | `SELECT pg_advisory_xact_lock($1)` | `bigint` | `void` |
| Q2 | `PgEventLog::append` | see below | `uuid[]`, `text[]`, `text[]`, `bigint[]` | none |
| Q3 | `PgEventLog::append` | `SELECT pg_notify('notification_events', '')` | none | `void` |
| Q4 | `PgEventLog::read_after`, no filter | `SELECT sequence, id, event_type, data, (extract(epoch FROM occurred_at) * 1000000)::bigint AS "occurred_at_micros!" FROM notification_events WHERE sequence > $1 ORDER BY sequence LIMIT $2` | `bigint`, `bigint` | `bigint`, `uuid`, `text`, `text`, `bigint` |
| Q5 | `PgEventLog::read_after`, with types | `SELECT sequence, id, event_type, data, (extract(epoch FROM occurred_at) * 1000000)::bigint AS "occurred_at_micros!" FROM notification_events WHERE sequence > $1 AND event_type = ANY($2::text[]) ORDER BY sequence LIMIT $3` | `bigint`, `text[]`, `bigint` | as Q4 |
| Q6 | `PgEventLog::latest_sequence` | `SELECT max(sequence) AS "latest?" FROM notification_events` | none | `bigint` |
| Q7 | `PgEventLog::prune` | `DELETE FROM notification_events WHERE recorded_at < now() - ($1::bigint * interval '1 millisecond')` | `bigint` | none |
| Q8 | `PgEventCatalog::register` | `INSERT INTO notification_event_types (name) SELECT batch.name FROM UNNEST($1::text[]) AS batch (name) ON CONFLICT (name) DO NOTHING` | `text[]` | none |
| Q9 | `PgEventCatalog::find_page` | `SELECT name FROM notification_event_types ORDER BY name LIMIT $1 OFFSET $2` | `bigint`, `bigint` | `text` |
| Q10 | `PgLeaseStore::acquire_or_renew` | see "Election Algorithm" | `text`, `uuid`, `bigint` | `uuid`, `bigint` |
| Q11 | `PgLeaseStore::acquire_or_renew`, fallback | `SELECT holder, term FROM cluster_leases WHERE name = $1` | `text` | `uuid`, `bigint` |
| Q12 | `PgLeaseStore::release` | `UPDATE cluster_leases SET expires_at = now() WHERE name = $1 AND holder = $2 AND expires_at > now()` | `text`, `uuid` | none |

Q2:

```sql
INSERT INTO notification_events (id, event_type, data, occurred_at)
SELECT batch.id, batch.event_type, batch.data,
       'epoch'::timestamptz + batch.occurred_at_micros * interval '1 microsecond'
FROM UNNEST($1::uuid[], $2::text[], $3::text[], $4::bigint[])
     WITH ORDINALITY AS batch (id, event_type, data, occurred_at_micros, position)
ORDER BY batch.position
ON CONFLICT (id) DO NOTHING
```

`PgEventLog::append` runs Q1, Q2 and Q3 in one transaction (`pool.begin()`, the three statements on `&mut *tx`, `commit()`); an empty slice returns 0 without a transaction. The result is `rows_affected()` of Q2. Measured: a batch with the same id twice inserts one row, and a repeated id consumes a sequence value, so sequences have gaps and no code may assume that `n + 1` follows `n`.

Why the lock: identity values are taken at insert time, not at commit. Without Q1 a transaction could commit sequence `n` after a reader already read past `n + 1`. With the lock taken first, appenders are serial, PostgreSQL makes a commit visible before it releases the transaction's locks, and a reader that pages by `sequence > cursor` never skips a row. The lock wait counts against `statement_timeout` (5 s), so a blocked appender fails instead of hanging.

After the two stories that add queries, `make.sqlx_prepare` writes twelve new files into `.sqlx/`.

## Wire Protocols

The normative text for clients is `docs/streaming-protocol.md` (new, FR-27); this section is its source. `docs/requests.http` gets one example per new route.

### Subscription request (both streaming routes)

- `types`: comma-separated event type names; required on `/api/events/stream`, optional on `/api/events/ws`; 1 to 100 distinct names (0 allowed on the WebSocket route when the parameter is absent).
- Cursor: header `Last-Event-ID`, or query parameter `lastEventId`; the header wins; decimal `u64`.
- Answers before a stream opens: 400 (`types` missing on SSE, cursor not a `u64`), 422 with a violation on `types`, 503 at the subscriber cap or during shutdown, 500 when a cursor is given and the log cannot be read; on the WebSocket route also the 4xx of a request that is not an upgrade (426 without upgrade state).

### SSE frames

Response head: `200`, `content-type: text/event-stream`, `cache-control: no-cache`. One frame per event:

```text
id: 1042
event: user.created
data: {"userId":"5f2c"}

```

- `id` is the sequence in decimal, `event` the event type, `data` the event data.
- Data with line breaks is written as one `data:` line per line. Before encoding, `\r\n` is replaced by `\n`, so a client reads the text with every line break as `\n` (a lone `\r` also arrives as `\n`; the SSE format has no way to carry `\r`). The WebSocket frame carries the text unchanged.
- Keep-alive: the comment frame `:` followed by an empty line, when no frame was written for `STREAM_KEEP_ALIVE`.
- The stream ends without a final frame when the subscriber is evicted, when the node shuts down or when a replay page cannot be read; the client reconnects, and `EventSource` sends `Last-Event-ID` by itself.

### WebSocket frames and commands

Server to client, one text frame per event; `id` is a string:

```json
{"id":"1042","type":"user.created","data":"{\"userId\":\"5f2c\"}"}
```

Client to server, text frames:

```json
{"action":"subscribe","types":["user.created","order.paid"]}
{"action":"unsubscribe","types":["order.paid"]}
```

A valid command has no answer. An invalid one is answered with one error frame, the set of types stays as it was and the connection stays open:

```json
{"error":"invalid event type name"}
```

| Cause | `error` text |
| --- | --- |
| not JSON, unknown `action`, unknown member, `types` missing | `invalid command` |
| `types` empty or more than 100 names | `types must hold 1 to 100 event type names` |
| a name that is not an event type | `invalid event type name` |
| the set would exceed 100 types | `a connection subscribes to at most 100 event types` |
| a binary frame | `binary frames are not supported` |
| a text frame above `HTTP_MAX_BODY_BYTES` | `the command is too large` |

Other rules: a run-time subscribe starts at live delivery (no replay); after an unsubscribe no later frame of the removed types is written, also not one that was already queued; the server sends a ping every `STREAM_KEEP_ALIVE`, and the library answers a client ping with a pong. Close codes sent by the server: 1013 after an eviction, 1001 at shutdown, 1011 when a replay page cannot be read.

> Assumption: close code 1011 (internal error, RFC 6455) is used when a replay page cannot be read; the PRD fixes only 1013 and 1001. A valid command gets no acknowledgement frame, because FR-9 and FR-10 define an answer only for an invalid one.

### SQS envelope

CloudEvents in binary style: the attributes are the metadata, the body is the data.

| Part | Rule |
| --- | --- |
| attribute `event-type` (String) | required, an event type name |
| attribute `event-id` (String) | optional UUID; default: the SQS `MessageId` |
| attribute `event-time` (String) | optional RFC 3339 time in UTC with the suffix `Z` (`humantime::parse_rfc3339`); default: `Clock::now()` at receipt |
| body | the event data: 1 to `EVENT_MAX_DATA_BYTES` bytes of UTF-8 without U+0000 |

Example as the JSON protocol of SQS carries it:

```json
{
  "QueueUrl": "http://localstack:4566/000000000000/notification-events",
  "MessageBody": "{\"userId\":\"5f2c\"}",
  "MessageAttributes": {
    "event-type": { "DataType": "String", "StringValue": "user.created" },
    "event-id": { "DataType": "String", "StringValue": "018f6c1e-7b1a-7c3e-9d2a-1f0e5a6b7c8d" },
    "event-time": { "DataType": "String", "StringValue": "2026-11-13T09:30:00Z" }
  }
}
```

A Kafka adapter maps the same three names from record headers and the record value to the body, and the offset to `receipt`; the domain and application crates do not change (FR-14).

> Assumption: `event-time` with a numeric offset (`+02:00`) is invalid. The application allow-list has `humantime` and no other time parser, and `humantime::parse_rfc3339` accepts only the `Z` form.

## Subscription Algorithm

`Subscriptions::open(request)`:

1. `hub.register(&request.types)`. The subscriber is in the hub before anything is read. `AtCapacity` and `Closed` end here (503).
2. Without a cursor: `floor = None`, `replaying = false`. No database read (NFR-11: accepted while the database is down).
3. With a cursor `c`: read `latest = log.latest_sequence()` (an error ends here with the 500 problem and drops the subscriber). `floor = min(c, latest or 0)`, `replaying = true`. The clamp implements "a cursor above the latest sequence is treated as the latest sequence" (FR-11): without it a cursor of `u64::MAX` would suppress every later event.

`Subscription::next()`:

1. If `pending` holds a frame, return it.
2. If `replaying`: read one page `log.read_after(floor, Some(types), REPLAY_PAGE)`.
   - On an error: `end = LogFailed`, return `None`.
   - Encode every event (`Frame::encode`), push it to `pending`, set `floor` to the last sequence of the page.
   - `replaying = (page length == REPLAY_PAGE)`. Go to step 1.
3. Live: `subscriber.recv().await`.
   - `None`: `end = HubClosed` when `subscriber.hub_closed()`, else `Evicted`; return `None`.
   - A frame whose sequence is not above `floor` is dropped (it was replayed, or the client had it); a frame whose type is no longer in `types` is dropped (unsubscribe). Otherwise set `floor` to its sequence and return it.

Why there is no gap and no duplicate. Let `s` be the sequence of an event of a subscribed type.

- The relay publishes `s` only after it read `s` from the log, so `s` is committed then.
- If the relay publishes `s` after step 1 of `open`, the frame is in the queue; it is written in the live phase unless a replay page already contained it (`s <= floor`), in which case it is dropped. No duplicate, no loss.
- If the relay published `s` before step 1 of `open`, `s` was committed before the first replay read, so the replay returns it when `s > c`. No loss.
- A relay that lags behind the client's cursor (the client got `s` from another node) publishes `s <= c` later; the floor drops it. No duplicate.

Bounds: `pending` holds at most one page; the replay is pulled by the socket, one page at a time (NFR-9). If more than `SUBSCRIBER_BUFFER` live events arrive during a long replay, the subscriber is evicted, the stream ends after the frames it already has, and the client resumes with its last id. `next` is safe to cancel: a page that was being read is read again, and `mpsc::Receiver::recv` loses nothing.

`subscribe(types)`: the union must not exceed 100, else `TooManyTypes`; then the local set and the hub entry are extended. `unsubscribe(types)`: both are reduced.

## Relay Algorithm

Wake-up: the listen loop runs `ListenForEvents::step`: `signal.changed()` returns after a `NOTIFY` on `notification_events`, and the step calls `wake.notify_one()` (a stored permit, so a wake-up between two waits is not lost). On an error (database down, listener connection lost) the step logs and sleeps `retry_delay`, and the next call builds a new listener. The relay loop waits with `RelayEvents::wait`: `tokio::time::timeout(poll_interval, wake.notified())`, either outcome returns. A lost notification therefore costs at most `EVENT_RELAY_POLL_INTERVAL` (FR-18).

`RelayEvents::tick()`:

1. Start cursor. When `cursor` is `None`: `latest = log.latest_sequence()?`; `cursor = Some(latest or Cursor::START)`. A node thus starts at the latest sequence of the moment its first round succeeds; older events reach a client only through replay. Until this read succeeds (database down at start) nothing is relayed, and the next round tries again.
2. Repeat at most `MAX_PAGES_PER_TICK` times (a `for` over a range, never an open loop):
   - `page = log.read_after(cursor, None, PAGE_SIZE)?` (Q4);
   - for every event in order: `publisher.publish(&event)`, then `cursor = event.sequence`;
   - store the cursor; when the page is shorter than `PAGE_SIZE`, return `Drained`.
3. Return `More`; the loop then runs the next round at once.

Properties: one reader task per node, so every subscriber gets its frames in sequence order; an error leaves the cursor at the last published event, so the next round continues without loss and without repeat; the round makes one database read per page for all subscribers of the node, and none per subscriber (NFR-3).

`Hub` as `EventPublisher`: look up the subscribers of the event's type under the lock; when there is none, return (no encoding); otherwise `Frame::encode(event)` outside the lock and `deliver`:

1. lock the registry;
2. for every sender of the type, in subscriber-id order: `try_send(Arc::clone(frame))`; `Full` and `Closed` put the id on a local eviction list;
3. remove every listed id from `types_of` and from each of its `by_type` entries; dropping the senders ends the subscriber's queue after the frames it already holds (FR-22);
4. unlock; return the counts.

## Ingestion Algorithm

`IngestEvents::process(batch)`; an empty batch returns at once.

1. `received_at = clock.now()`, once per batch.
2. Keep the receipt of every message. Convert each message with `to_new_event`:
   - `event-type` missing: `MissingType`; not an `EventType`: `Type`;
   - id: the `event-id` attribute, else `message_id`; `shared_domain::Uuid::parse` fails: `Id`;
   - `event-time` present and `humantime::parse_rfc3339` fails: `Time`; absent: `received_at`;
   - `EventData::new(body, max_data_bytes)` fails: `Data`.
   A rejected message is logged once at warn as `inbound message rejected` with the message id and the reason, never the body or the attribute values (FR-16).
3. When at least one event is valid: `catalog.register(distinct types)`, then `log.append(&events)`. An error returns here; nothing is acknowledged and the bus redelivers the batch (FR-17).
4. `source.acknowledge(all receipts)`, valid and rejected alike. An error is returned; the redelivery finds every id stored and appends nothing.

The order register, append, acknowledge keeps two invariants: the catalog is a superset of the types in the log, and a message leaves the bus only after its event is stored.

## Election Algorithm

One lease row, named `leader`. Q10, the single statement of a round, parameters `(LEASE_NAME, node, ttl in milliseconds)`:

```sql
INSERT INTO cluster_leases AS lease (name, holder, term, expires_at)
VALUES ($1, $2, 1, now() + ($3::bigint * interval '1 millisecond'))
ON CONFLICT (name) DO UPDATE
SET holder = EXCLUDED.holder,
    term = CASE WHEN lease.holder = EXCLUDED.holder THEN lease.term ELSE lease.term + 1 END,
    expires_at = EXCLUDED.expires_at
WHERE lease.holder = EXCLUDED.holder OR lease.expires_at <= now()
RETURNING lease.holder AS "holder!", lease.term AS "term!"
```

`PgLeaseStore::acquire_or_renew`:

1. Run Q10 with `fetch_optional`.
   - A row: this node holds the lease (first insert with term 1, renewal with the same term, or takeover of an expired lease with term + 1).
   - No row: another node holds an unexpired lease.
2. Only when no row came back: run Q11 as a second statement with a fresh snapshot and return its row; a missing row is a `LeaseStoreError`.

Measured on PostgreSQL 16: node A gets term 1; A renews, term 1; B gets no row while A's lease is valid and reads `(A, 1)`; after A's `release`, B gets `(B, 2)`; A then gets no row; B takes its own expired lease again with term 2. The row is never deleted, so the term never decreases, and `release` (Q12) only moves `expires_at` to `now()`.

`Elector::campaign()`, one round:

1. `tokio::time::timeout(settings.call_timeout, store.acquire_or_renew(node, settings.ttl))`.
2. Success with `leadership`: status `{ node_id, role: leadership.role_of(node), leader: Some(leadership.leader), term: Some(leadership.term) }`.
3. Error or timeout: log `lease store failed` at warn with the error chain; status `{ role: Follower, leader: None, term: the last term seen }`. The node steps down at once, which is the safe direction (FR-19).
4. Publish with `watch::Sender::send_replace`; when the role or the leader changed, log `leadership changed` at info with node id, role, leader and term.

`Elector::resign()`: when the current role is leader, `timeout(call_timeout, store.release(node))` (a failure is logged; the lease then simply expires); publish follower with no leader.

Timing, with `T = CLUSTER_LEASE_TTL` and `R = CLUSTER_RENEW_INTERVAL`:

- A crashed leader's lease expires at most `T` after its last renewal; a follower campaigns every `R`; takeover therefore happens at most `T + R` after the crash, 13 s with the defaults (FR-20, NFR-6). After a graceful stop it happens within `R`.
- A leader that cannot reach the database: the round that starts at most `R` after the last renewal ends after at most `call_timeout = R`, so the node is follower at most `2R` after its last renewal. The cross-field rule `2R < T` makes this earlier than the expiry of its lease, so two nodes do not both believe they are leader because of an outage.
- Expiry compares `expires_at` with `now()` of the database only; no node clock takes part (NFR-7).

What the lease is for: it keeps the bus consumer single, which keeps the load on the queue and the log low. Correctness does not depend on it: a node that was paused past its expiry can at worst process one more batch, and that append is idempotent by event id and serial through the advisory lock.

## Failure Modes

| Failure | Behaviour |
| --- | --- |
| A node crashes | Its clients lose the connection and reconnect to any node with their last id; the replay returns what they missed, without duplicates (FR-11). No shared state is lost: the node held only connections and a relay cursor. |
| The leader crashes | Ingestion pauses for at most `CLUSTER_LEASE_TTL + CLUSTER_RENEW_INTERVAL`; messages stay on the bus; received messages that were not acknowledged return after the queue's visibility timeout and are appended once by the new leader. Delivery of stored events continues on every node. |
| The leader stops gracefully | `resign` expires the lease; another node takes it within `CLUSTER_RENEW_INTERVAL`. |
| A paused leader resumes after its lease expired | It can process at most one more batch before its next round makes it follower; the append is idempotent, so the log is unchanged. |
| Database outage | Every election round fails or times out, so every node is follower and ingestion stops. Open streams stay open and get keep-alives. The relay round and the listener fail, are logged once per round and retried; the catalog and a subscription with a cursor answer 500; a subscription without a cursor is accepted. After the outage a leader is elected, the listener reconnects and each relay continues from its cursor (NFR-11). |
| Listener connection lost, or a proxy that does not pass `LISTEN` | `changed` fails; the relay falls back to its poll interval; nothing is lost because the relay reads by cursor. |
| Queue outage or missing queue | `receive` fails; the ingest loop logs `ingestion failed`, waits `RETRY_DELAY` and tries again; everything else keeps working. |
| Slow or stalled client | Its queue fills; the next `try_send` evicts it; its stream ends after the frames it already has (SSE: end of body; WebSocket: close 1013). Other subscribers, the relay and ingestion never wait (FR-22, NFR-10). |
| Subscriber cap reached | The request is answered with the 503 problem; no stream, no upgrade (FR-23). |
| The bus delivers a message twice | `ON CONFLICT (id) DO NOTHING`: no new row, no new sequence, no second delivery; the message is acknowledged (FR-17). |
| Invalid message (type, id, time) or oversized, empty or NUL-holding body | Logged once at warn without the body, acknowledged, not appended; the valid messages of the batch are processed (FR-16). |
| Acknowledge fails after the append | The error is logged; the bus redelivers; the append finds the ids stored. |
| Cursor older than the retention window | The replay starts at the oldest retained event; the loss is not signalled (FR-11 assumption of the PRD). |
| A background task panics | The other loops continue; the failure is reported at shutdown. `GET /api/cluster` and the logs show a node that no longer campaigns or relays. |
| The grace period ends during shutdown | As today: `serve` logs `grace period elapsed, forcing shutdown` and exits with status 1. |

## Performance Design

- **Publish cost follows the interested subscribers.** `Hub::deliver` looks up one key in `by_type` and visits only the senders under it. Subscribers of other types and idle connections are not touched (NFR-2). An event whose type has no subscriber on the node is not even encoded.
- **One encode per event and transport.** `Frame::encode` builds the SSE `Event` and the JSON text once; all subscribers share the `Arc<Frame>`. A WebSocket subscriber clones `Utf8Bytes` (a reference count). An SSE subscriber clones the built `Event` (one copy of the encoded bytes into its own write buffer; `axum` has no public constructor for a shared, finished `Event`). No subscriber encodes, escapes or splits anything, which is how NFR-2 is read here.
- **Bounded queues.** One `mpsc` channel of `SUBSCRIBER_BUFFER` frame references per subscriber, at most `MAX_SUBSCRIBERS` subscribers, at most 100 types each; the replay holds one page (NFR-9). With the defaults the worst case of queued references is 256 x 100 000 pointers, and the frames behind them are shared.
- **No lock across an await.** The registry is a `std::sync::Mutex`; `register`, `deliver`, `subscribe`, `unsubscribe`, the removal in `Drop` and `close` are synchronous and hold it for a bounded number of map operations and `try_send` calls. Encoding happens outside the lock.
- **No database read on the live path.** A node reads the log once per relay round, whatever the number of clients; a client reads only during its own replay, in pages (NFR-3).
- **Latency.** Append, commit, `NOTIFY`, one indexed range read, fan-out: no timer is on the path when the listener works. The poll interval is only the fallback.
- **Ingestion.** One transaction per batch of at most 10 messages, serial through the advisory lock; throughput is that of one leader (NFR-12).

> Assumption: the registry is one mutex. At 10 000 subscribers of one type a fan-out holds it for roughly the time of 10 000 `try_send` calls, during which registrations wait. Sharding the registry by type is a later optimisation that changes no interface; NFR-1 is checked with the load profiles before it is considered.

## Contract Impact

- **OpenAPI** (`.github/openapi-spec/spec.yaml`, regenerated with `make.generate_openapi_spec`): two new paths, `GET /api/event-types` (`api_event_types_get`, tag `EventStream`; 200 with an array of `EventType`, 400, 422, `default`) and `GET /api/cluster` (`api_cluster_get`, tag `Cluster`; 200 with `ClusterStatus`, `default`); new schemas `EventType`, `ClusterStatus`, `ClusterRole`; two new tags. `API_VERSION` in `crates/service/src/openapi.rs` becomes `1.1.0`, because operations are added (the rule in its doc comment). Spectral runs with `--fail-severity=hint`, so every operation and schema property carries a description and an example like the existing ones.
- **GraphQL** (`.github/graphql-spec/spec`, regenerated with `make.generate_graphql_spec`): the type `EventType { name: String! }` and the query field `eventTypes(itemsPerPage: Int! = 30, page: Int! = 1): [EventType!]!`.
- **Not in the OpenAPI document:** `GET /api/events/stream` and `GET /api/events/ws`. OpenAPI 3.1 cannot describe a WebSocket, and `make.schemathesis_validate` calls every documented operation and would wait on a response that never ends. Both routes are specified in `docs/streaming-protocol.md`.
- `make.openapi_diff` and `make.graphql_diff` report additions only; no existing operation, schema or status changes (NFR-20).

## Architecture Model Impact

`workspace.dsl` (checked by `make.validate_architecture_model`, which needs one component per workspace crate and one relationship per workspace-internal `[dependencies]` entry):

- Two groups inside the container `service`: `Notification/EventStream` with the sub-groups `Domain`, `Application`, `Infrastructure`, and `Internal/Cluster` with the same three. Components `eventStreamDomain`, `eventStreamApplication`, `eventStreamInfrastructure`, `clusterDomain`, `clusterApplication`, `clusterInfrastructure`, named after their crates and tagged like the existing ones.
- Relationships between components:
  - `compositionRoot ->` each of the six;
  - `eventStreamDomain -> sharedDomain`; `eventStreamApplication -> eventStreamDomain`, `sharedDomain`, `sharedApplication`; `eventStreamInfrastructure -> eventStreamDomain`, `sharedDomain`;
  - `clusterDomain -> sharedDomain`; `clusterApplication -> clusterDomain`, `sharedDomain`, `sharedApplication`; `clusterInfrastructure -> clusterDomain`, `sharedDomain`.
- Relationships to other elements: `eventStreamInfrastructure -> database` ("Appends and reads the event log, the catalog; LISTEN and NOTIFY"), `clusterInfrastructure -> database` ("Acquires, renews and releases the leader lease"), `system.service.eventStreamInfrastructure -> messageBroker` ("Receives and deletes the messages of the event queue").
- Descriptions: the container `service` also "serves SSE and WebSocket event streams"; the relationship of `apiClient` adds the streams; the message broker is also the event bus.

A component is added in the story that adds its crate; a relationship in the story that adds the manifest entry.

## Local Stack and Load Test

Local stack:

- `docker-compose.yml`: the service `app` gets `EVENT_QUEUE_URL: http://localstack:4566/000000000000/notification-events`; `localstack` mounts a new executable script read-only into `/etc/localstack/init/ready.d/`, which runs `awslocal sqs create-queue --queue-name notification-events`. The script is `docker/localstack/ready.d/create-queues.sh`; it is a shell file, so the repository lint (`make lint-repo`, which has no profile key) checks it.
- `docker-compose.override.yml`: `EVENT_QUEUE_URL: !reset null` under `app`, so that the development binary reads `.env`.
- The health check of `localstack` can pass before the script ran; the ingest loop treats the missing queue as a failed round and retries.

K6 scenario (FR-26), fitted to the harness as it is: the pinned stock image, the protected `thresholdsBuilder.js` (budget on `http_req_duration{test_type:<profile>}` and `checks{scenario:<profile>}`), `ScenarioUtils`, which reads all four profiles.

- Files: `tests/Load/scripts/eventStream.js` and `endpoints.eventStream` in `tests/Load/config.json.dist` with `smoke`, `average`, `stress` and `spike`. Each profile has the usual `threshold`, `rps`, `vus`, `duration` and two more keys, `hold` (seconds) and `deliveryThreshold` (milliseconds). Smoke: `threshold` 500, `rps` 10, `vus` 50, `duration` 20, `hold` 5, `deliveryThreshold` 2000. Top-level keys `brokerPort` (`14566`) and `eventQueue` (`notification-events`) are added.
- `docker-compose.load_test.override.yml`: `localstack` publishes `127.0.0.1:${LOAD_TEST_LOCALSTACK_PORT:-14566}:4566` instead of no port, so that K6 (host network) can reach the queue. The variable gets its row in `docs/environment.md`.
- One iteration (an `async` default function):
  1. `GET /api/event-types`, check status 200. This is the only request under the protected p99 budget.
  2. Start `http.asyncRequest('GET', '/api/events/stream?types=load.test', null, { timeout: hold, tags: { test_type: 'sse_hold' } })`: one held SSE subscriber per virtual user.
  3. Open a `k6/websockets` socket on `/api/events/ws?types=load.test`. Half a second after `open`, publish one event: `http.asyncRequest('POST', ..)` to LocalStack with the JSON protocol (`X-Amz-Target: AmazonSQS.SendMessage`), tagged `test_type: 'publish'`; the body holds the virtual user, the iteration and the send time. On the message that carries this marker, add `now - send time` to the custom trend `event_delivery_latency`. Close the socket after `hold`.
  4. Await the SSE request and check that its body holds a frame with `event: load.test` and the marker.
- Thresholds: the two of `ThresholdsBuilder`, unchanged, and one that the script adds to the options, `event_delivery_latency{test_type:<profile>}` with `p(99) < deliveryThreshold`.
- With 50 virtual users, 10 iterations per second and a hold of 5 s, the smoke profile holds 50 SSE and 50 WebSocket subscribers while about 10 events per second pass from the queue through ingestion, log, relay and hub to all of them.
- The load-script lint (`make lint-load-tests`, which has no profile key) must pass; `console` is forbidden in scenario scripts.

> Assumption: the SSE subscriber is checked by the frames in its body and not by status 200. The pinned K6 reports a response that never ends as a timed-out request (status 0, error code 1050) and still returns the body it read; FR-26's status check is applied to the catalog request and to the WebSocket handshake. Delivery latency is measured on the WebSocket subscriber, which is the only transport on which the stock image sees single messages. Each run logs one `Request Failed` warning per held stream; no threshold reads `http_req_failed`.
>
> Assumption: the delivery ceiling starts at 2000 ms because the path includes LocalStack and the long poll; it is tightened with the budget rule of `docs/load-testing.md` after the first hosted-runner measurement, like `threshold`. Neither value is among the protected pointers, which cover the `health` scenario only.

## Test Strategy

General rules, each a consequence of a gate:

- Coverage counts unit tests and every test target named `integration` of the workspace; the Gherkin suite is not instrumented. A mutant is judged only by the unit and `integration` tests of its own crate, and a timed-out mutant counts as surviving.
- Every production loop has a bounded range or an awaited pause in each round. Every test that awaits is inside `tokio::time::timeout`. A loop test stops the loop from inside the round after a fixed number of rounds and asserts on virtual time, so a mutant fails an assertion instead of hanging.
- Unit tests use `#[tokio::test(start_paused = true)]`; no real sleep, no `Instant::now` of `std`.
- Tests assert on effects (rows, frames, acknowledgements, log entries through `test_support::log::LogRecorder`), not only on completion.

Fakes to add in `tests/support/src/` (`std` only; `tests/support/Cargo.toml` gains `event-stream-domain` and `cluster-domain`):

| File | Double | Behaviour |
| --- | --- | --- |
| `event_log.rs` | `InMemoryEventLog` | a `BTreeMap` behind a mutex with the rules of the port: idempotent by id, gaps on a repeated id, filter, limit, `latest_sequence`, `prune` driven by a test call; `failing()`; `with_call_budget(n)`, after which every call fails, so a runaway loop ends |
| `event_catalog.rs` | `InMemoryEventCatalog` | a `BTreeSet`; `failing()`; `fail_times(n)` for the seed loop |
| `event_source.rs` | `ScriptedEventSource` | a queue of batches, then empty batches; records acknowledged receipts; `failing_receive()`, `failing_acknowledge()`, `pending()` (a receive that never completes, `std::future::pending`) |
| `event_signal.rs` | `ScriptedEventSignal` | `ready(n)` (n successes, then pending), `pending()`, `failing()` |
| `lease_store.rs` | `InMemoryLeaseStore` | holder, term and an `expired` flag set by `expire()`; shared between two electors through `Arc`; `failing()`, `hanging()` |
| `fixture.rs` | `event(rng)`, `new_event(rng)`, `event_type(rng)` | seeded data, as `example_api_resource` |

Per layer:

- **Domain crates**: unit tests and `proptest` for `EventType` (the hand-written matcher against generated strings of the grammar and against mutations of them), `EventData`, `Sequence`, `Cursor::parse`, `Term`, `Leadership::role_of`.
- **`event-stream-application`**:
  - use cases with the fakes: `IngestEvents` (each rejection reason, mixed batch, append failure acknowledges nothing, acknowledge failure, duplicate id), `RelayEvents` (start cursor, paging, `More`, error keeps the cursor, `wait` on notify and on the timer), `ListenForEvents`, `PruneEvents`, `SeedEventTypes`, `ListEventTypesHandler`;
  - `Hub` and `Subscription` with real `mpsc` channels: fan-out by type, eviction on a full queue, cap, close, removal on drop, replay then live, each of the four gap and duplicate cases of "Subscription Algorithm", the clamp, unsubscribe with a queued frame;
  - `write_json_string` and `event_json` property-tested against `serde_json` (parse the frame and compare the three members for arbitrary strings, including quotes, backslashes, control characters and characters outside the Basic Multilingual Plane); `Frame::encode` for the line-break rule;
  - handlers through `tower::ServiceExt::oneshot`: the catalog with `FakeQueryBus`; `stream` (every 400, 422, 503, 500, the frames and the keep-alive read from the body with `http-body-util` under paused time, the end of the body after eviction and after `close`); `ws` up to the upgrade (every problem, and 426 for a request without upgrade state);
  - `Session::on_message` exhaustively (every `Message` variant, every row of the error table);
  - `socket::serve` with a scripted `Connection` defined in the crate's tests: frames out, commands in, ping on the interval, each close code, a failing `transmit`. The double panics when `receive` is called after it returned `None`, so a mutant that ignores the end fails at once;
  - `tests/integration/ws.rs`: the context router with a real `Hub` and `InMemoryEventLog`, served by `axum::serve` on a loopback listener; a client of about fifty lines in `support.rs` speaks the handshake and the frames over `tokio::net::TcpStream` (client frames with a zero mask, which is valid). It covers the two lines only a socket reaches: `on_upgrade` in the handler and `impl Connection for WebSocket`. The two socket helpers carry `#[expect(clippy::disallowed_methods, reason = "integration tests open the sockets their subject talks to")]`, the form `crates/service/tests/integration/support.rs` already uses.
- **`event-stream-infrastructure`** (`#[sqlx::test(migrator = "shared_infrastructure::persistence::MIGRATOR")]` and LocalStack):
  - migration test (tables, named constraints, the check constraints reject an invalid name and empty data);
  - `PgEventLog`: order, idempotency, gaps, filter and limit, time round trip to the microsecond, `prune` keeps younger rows, two concurrent appenders with a paging reader never skip a row, a `NOTIFY` arrives after an append, unreachable database is an error within the acquire timeout;
  - `PgEventCatalog`; `PgEventSignal` (wake after an append, many notifications give one wake, error when the database is unreachable and recovery on the next call);
  - `SqsEventSource`: unit tests with `aws-smithy-mocks` (attribute mapping, missing receipt handle, partial delete failure), integration tests against LocalStack with one queue per test (round trip of the envelope, acknowledge removes the messages, missing queue is an error).
- **`cluster-application`**: `Elector` with `InMemoryLeaseStore` under paused time (leader, follower, step-down on error and on timeout, term kept, `resign`, the log entries); the handler through `oneshot`.
- **`cluster-infrastructure`**: migration test; `PgLeaseStore` with two node ids on one database (the sequence measured in "Election Algorithm", takeover after a 1 ms lease, `release` keeps the row and the term).
- **`crates/service`**:
  - unit: `config` (each variable, each bound, the cross-field rule, no value in a reason), `bootstrap` wiring, each loop of `background` with fakes and paused time, `Running::shutdown` order;
  - integration (`tests/integration/`): two `Application`s on one `#[sqlx::test]` database, a `ScriptedEventSource` on one of them: an event ingested by node A reaches a subscriber of node B (FR-18); two electors with `PgLeaseStore`: exactly one leader, takeover after `resign` and after expiry (FR-19, FR-20); the spawned binary: `GET /api/cluster`, an SSE stream read from a raw socket, `tokio-tungstenite` on `/api/events/ws`, and SIGTERM with an open stream exits with status 0 inside the grace period (FR-24). The existing serve tests that list the entries written outside requests are extended with the entries of the background loops.
- **End to end** (`features/event_catalog.feature`, `event_stream_sse.feature`, `event_stream_websocket.feature`, `event_stream_resume.feature`, `cluster.feature`): the world keeps one `Application` for the scenario instead of one per request. A step that publishes pushes a message into a `ScriptedEventSource`, then calls `ingest.receive`, `ingest.process` and `relay.tick` directly; a step that reads takes the next frame of the kept SSE body with a deadline. The WebSocket scenarios serve the kept router on a loopback listener and use `tokio-tungstenite`; `tests/e2e/Cargo.toml` gains it together with `futures-util` with `sink`, `event-stream-application` and `cluster-application`.
- **Load**: the scenario of "Local Stack and Load Test" inside `make.smoke_load_tests`.

> Assumption: the Gherkin world may bind one loopback listener for the WebSocket scenarios, with an item-level `#[expect(clippy::disallowed_methods, reason = "..")]`. An upgrade cannot be driven through `oneshot`, and the ban on sockets names the integration tests as an allowed place; the end-to-end crate is test code of the same kind.

## Decision Records

New records under `docs/adr/`, indexed in `docs/adr/README.md`:

| Record | Decision | Main reason |
| --- | --- | --- |
| `0013-leader-lease-in-postgresql.md` | Leader election is a PostgreSQL lease with a term, judged by the database clock; Raft is rejected | PostgreSQL is already the consistent store; a consensus crate is not on the allow-list and would be a second failure domain; exclusivity is an optimisation because the append is idempotent |
| `0014-event-log-and-listen-notify.md` | The cluster transport is the event log read by cursor, woken by `LISTEN/NOTIFY`, with a poll fallback | no broker between nodes; a lost signal costs one poll interval and no event; the advisory lock makes sequence order equal commit order |
| `0015-event-source-port-sqs-first.md` | Ingestion goes through the `EventSource` port; the first adapter is SQS; Kafka is a follow-up | `aws-sdk-sqs` and LocalStack are in the stack; a Kafka client needs a labelled change of the protected allow-list; the envelope maps to Kafka headers |
| `0016-streaming-routes-outside-openapi.md` | The SSE and WebSocket routes are served but not documented in OpenAPI; `docs/streaming-protocol.md` is their contract | OpenAPI cannot describe WebSocket; a never-ending response would hang the Schemathesis gate |
| `0017-opaque-text-event-data.md` | Event data is opaque UTF-8 text of 1 to `EVENT_MAX_DATA_BYTES` bytes without U+0000; the service neither parses nor validates it | as in Mercure; the schema repository is a follow-up; the two limits come from PostgreSQL `text` and from the SSE format |

## Corrections to the design input

Each item names what the design input says, what cannot work as written, and what this document does instead.

1. **SQS long poll.** The input says "wait 5 s" on the shared client. `sqs_client` sets the attempt and operation timeout to `SQS_OPERATION_TIMEOUT` (2 s), so every empty poll would be cut. Correction: a second client for ingestion, built by the same factory with `operation_timeout + 5 s`; the probe client and the cross-field rules stay.
2. **Timestamps.** The input declares `timestamptz` columns and gives no mapping. With the enabled sqlx features no Rust type maps to `timestamptz`. Correction: `occurred_at` crosses as epoch microseconds (`bigint`) converted in SQL; every other time column is written by `now()` and never read; durations are bound as milliseconds.
3. **`EventLog::prune`.** The input says "prune older than a time". Correction: `prune(retention: Duration)` compared with the database `now()`, so no node clock and no timestamp parameter is involved.
4. **`release`.** The input says it "clears the lease". A delete would restart the term at 1. Correction: the row stays; `release` sets `expires_at` to `now()`.
5. **Takeover bound and renew rule.** The input gives two readings of the renew rule and "within the lease time". Correction: `2 * CLUSTER_RENEW_INTERVAL < CLUSTER_LEASE_TTL`; takeover within `TTL + RENEW` after a crash; every election call is bounded by the renew interval, which with the rule makes a cut-off leader step down before its lease expires.
6. **Leader gating.** The input lets the ingest and prune use cases act "only while the watch says Leader". An `event-stream` crate may not depend on a `cluster` crate. Correction: the two loops of `crates/service` read the status channel; the use cases know nothing about leadership.
7. **`EventSignal`.** The input declares `async fn changed(&mut self)` without a result and lets the relay select on it. The listener must be created lazily (the service starts while the database is down) and can fail, and the adapter cannot retry or sleep. Correction: `changed` returns a `Result`; a separate one-round use case `ListenForEvents` turns it into a `Notify`, so the driver future is cancelled only at shutdown and the relay waits on `Notify` or its timer. The listener uses its own pool of one connection.
8. **Fakes.** The input asks for fakes in `tests/support`; that crate may not use tokio. Correction: all five fakes use `std` only (`std::future::pending` for a wait).
9. **GraphQL arguments.** The input writes `page: Int = 1`. The committed schema and `async-graphql` defaults give `Int! = 1` and `Int! = 30`. Correction: the non-null form.
10. **WebSocket tests.** The input assumes the handler is unit-testable. The upgrade extractor needs hyper's upgrade state, and the application dev allow-list has no WebSocket client. Correction: a socket-free `Session`, a `Connection` trait with a scripted double, and one integration test inside `event-stream-application` with a hand-written client over a loopback socket, which is where the coverage and mutation gates look for that crate.
11. **K6.** The input asks for "50 SSE subscribers held while events are published, inside the scenario's budget". The stock image has no SSE client, the protected budget is on `http_req_duration`, the load-test stack publishes no LocalStack port, and a scenario needs four profiles. Correction: the scenario of "Local Stack and Load Test": held SSE requests checked by their partial body, delivery latency as a custom trend on WebSocket subscribers, the protected budget on the catalog request, events published through a loopback port of the load-test stack.
12. **Event data.** The input says data is opaque text with only a size limit. PostgreSQL `text` rejects U+0000 and an SSE event without data is dropped (and `Event::data("")` writes no line). Correction: 1 byte or more, no U+0000; such a message is rejected like an oversized one.
13. **`event-time` and the envelope port.** The input lets the adapter interpret the envelope. `humantime` is allowed only in the application layer and accepts only the `Z` form. Correction: the port carries raw attribute strings; the application validates; a time with an offset is invalid.
14. **`EVENT_TYPES` at start.** The input says the types are "registered in the catalog at start". The pool is lazy and `serve` must start without the database. Correction: a seed loop that retries until its first success.
15. **Shutdown order.** The input says the loops stop "before the drain" and the hub closes last. An open stream blocks the drain until the grace period ends, with exit status 1. Correction: the hub is closed before the HTTP stop flag is set; the order is loops, lease, hub, drain (FR-24).
16. **SSE line breaks.** The input does not mention them. `Event::data` starts a line at every `\r` and `\n`. Correction: `\r\n` is folded to `\n` before encoding and the protocol document states that SSE delivers line breaks as `\n`.
17. **Oversized WebSocket message.** The PRD answers it with an error frame. Without a transport limit a client could make the node buffer 64 MiB. Correction: error frame up to twice `HTTP_MAX_BODY_BYTES`, connection end above.
18. **Subscription cursor.** The input drops live events "not above the last replayed one". With an empty replay that leaves no floor, and a relay that lags would deliver a duplicate; a cursor above the latest sequence would suppress everything. Correction: the floor starts at `min(cursor, latest sequence)`, which costs one read at connect for a subscription with a cursor.
