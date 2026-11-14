# Architecture: real-time event delivery hub (issue #4)

This document turns the design input, `prd.md` and `research.md` of this bundle into a design that can be coded story by story. Every statement about the repository was read in the code. Every statement about a library was read in the source of the locked version (`axum 0.8.9`, `sqlx-postgres 0.9.0`, `sqlx-core 0.9.0`, `aws-sdk-sqs 1.121.0`, `tokio 1.54.1`, `utoipa-axum 0.3.0`, `futures-util 0.3.34`, `tower-http 0.7.1`, `tower 0.5.3`), and every SQL statement was run on PostgreSQL 16. Decisions of this stage are marked `> Assumption:`. The last section lists what was changed against the design input and why.

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
| `tower-http` | 0.7.1, features `trace`, `request-id`, `catch-panic`, `set-header` and, added by this change, `cors` | `cors::{CorsLayer, AllowOrigin}`: the cross-origin response headers of the router |
| `tower` | 0.5.3, feature `util` unchanged | `ServiceBuilder::option_layer` in the router of the composition root |
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
- Model and docs: the story that adds a crate adds its component and relationships to `workspace.dsl`; a new variable lands in `cli.rs`, `config.rs` and `docs/environment.md` together, and in `.env` when it has a default or a local value; a changed synchronised file is added to `.templatesyncignore`.
- Quality: 100 % line and function coverage, zero surviving or timed-out mutants; every background loop is a one-round `tick()`/`step()` of an application crate; a loop has a bounded range, or awaits in its own body under the stop flag (`until_stopped`), never behind a `bool` helper; a fake of a loop test pends after its script or call budget; every awaiting test is bounded by `tokio::time::timeout`; `make.mutation_tests_diff` (make mutation-tests-diff) resets the test database, so only the serialized verify step runs it.
- Layers: infrastructure has no tokio (no sleep, timeout, spawn, select); `test-support` fakes use `std` only; `serde_json` is dev-only in layer crates.
- Delivery: publishing never awaits a subscriber (`try_send`; a subscriber whose queue is full falls behind and catches up from the log on its connection), no lock is held across `.await`; keep the example context and the three health probes; no authorization, Kafka adapter or publish endpoint.

## Context and Goals

The service becomes an open-source alternative to Mercure for VilnaCRM: a frontend reads the catalog of subscribable event types (FR-1, FR-2), subscribes over SSE or WebSocket (FR-4 to FR-11) and receives every event that enters through the VilnaCRM event bus (FR-14 to FR-17). Several nodes run against one PostgreSQL database; any node serves any client (FR-18, NFR-5), and one node at a time, the lease holder, reads the bus (FR-19, FR-20).

What exists today (read in the code): one binary `service` with three bounded contexts (`shared`, `health-check`, `some-module`), no background task, no per-connection state, no timestamp column, an SQS client used only by a health probe, and a `serve` command that drains HTTP on SIGTERM and exits with status 1 when a connection is still open after `SHUTDOWN_GRACE_PERIOD` (`crates/service/src/command/serve.rs`).

What this change adds:

- two bounded contexts, each with three crates: `notification/event-stream` and `internal/cluster`;
- three tables through three new migration pairs, one per table;
- six background loops in the composition root, each scheduling a one-round use case: seed, listen and relay on every node, and elect, ingest and prune on a node with an event source;
- two more connection pools beside the one of the requests: three connections for the loops and one for the listener;
- two documented REST operations, one GraphQL query, two streaming routes;
- twelve configuration variables (FR-25): the eleven of the design input and the allow-list of browser origins (FR-28).

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
   - `sqlx::postgres::PgListener`: `connect_with(&PgPool)` takes one connection out of the pool for the life of the listener; `listen(&str)`; `recv() -> Result<PgNotification, Error>` reads the socket and returns one notification per call; it reconnects and re-subscribes by itself, loses the notifications sent in the gap, and fails only when that reconnect fails or the pool is closed; `next_buffered() -> Option<PgNotification>` returns only notifications that arrived while a statement ran on the listener connection (`src/listener.rs:260-271` and `:338-344`, `src/connection/stream.rs:123-139`), and `recv()` hands those out first, so nothing is drained and nothing is folded at this level (run: five commits gave five completions of `recv()` and `next_buffered()` returned nothing); it implements `Debug`.
   - No existing code reads or writes a `timestamptz`: the only table has no time column and the three `.sqlx` files hold none. The enabled features are `runtime-tokio`, `tls-rustls-aws-lc-rs`, `postgres`, `macros`, `migrate`, `uuid`; without `chrono` or `time` the query macros have no Rust type for `timestamptz` (the type table of `sqlx-postgres/src/type_checking.rs` lists the date and time types only in a `datetime-types` block keyed by those two features). The same table maps `void` to `()`, `bigint` to `i64` and arrays to slices, which is what the queries below rely on.
4. **aws-sdk-sqs 1.121.0 (locked).** `client.receive_message().queue_url(..).max_number_of_messages(i32).wait_time_seconds(i32).visibility_timeout(i32).message_attribute_names(impl Into<String>)` (one call per name; `visibility_timeout` at `src/operation/receive_message/builders.rs:454`, and without it the queue's own value applies); `Message` has `message_id`, `receipt_handle`, `body` and `message_attributes: Option<HashMap<String, MessageAttributeValue>>` with `string_value`; `client.delete_message_batch().queue_url(..).set_entries(Some(Vec<DeleteMessageBatchRequestEntry>))`, the entry builder needs `id` and `receipt_handle` and its `build()` returns a `Result`; the output lists `failed: Vec<BatchResultErrorEntry>` with HTTP 200, so the adapter must read it.

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
| `event.rs` | `NewEvent`, `StoredEvent` |
| `value_object.rs`, `value_object/event_type.rs` | `EventType` |
| `value_object/event_data.rs` | `EventData` |
| `value_object/sequence.rs` | `Sequence` |
| `value_object/cursor.rs` | `Cursor` |
| `value_object/page_limit.rs` | `PageLimit` |
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
    /// Data that enters through the bus: `DataEmpty`, `DataContainsNul`, `DataTooLarge` above `max_bytes`.
    pub fn new(value: String, max_bytes: usize) -> Result<Self, InvalidEvent>;
    /// Data read back from the log: the rules of `new` without the size limit.
    pub fn stored(value: String) -> Result<Self, InvalidEvent>;
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
    /// The cursor `distance` positions later; it saturates at `u64::MAX`.
    pub const fn advanced_by(self, distance: u64) -> Self;
}
impl From<Sequence> for Cursor { /* the same number */ }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEvent { pub id: Uuid, pub event_type: EventType, pub data: EventData, pub occurred_at: SystemTime }

/// An event as the log holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEvent { pub sequence: Sequence, pub id: Uuid, pub event_type: EventType, pub data: EventData, pub occurred_at: SystemTime }
impl StoredEvent {
    /// Rebuilds an event from the parts of a stored row: `Sequence::new`, `EventType::new` and
    /// `EventData::stored`, in this order; the first error is returned.
    pub fn restore(sequence: u64, id: Uuid, event_type: String, data: String, occurred_at: SystemTime)
        -> Result<Self, InvalidEvent>;
}

/// Bounds of one replay page: a number of events and a number of data bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageLimit { pub rows: usize, pub bytes: usize }
impl PageLimit {
    /// True when `page` holds `rows` events or more, or `bytes` bytes of data or more
    /// (the sum of `data.as_str().len()`): the read that returned it may have left events behind.
    pub fn reached_by(&self, page: &[StoredEvent]) -> bool;
}
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
    /// The read of the relay: at most `limit` events of every type with a sequence above `cursor`,
    /// in sequence order.
    async fn read_after(&self, cursor: Cursor, limit: u32) -> Result<Vec<StoredEvent>, EventLogError>;
    /// The read of a replay: the events of `types` with `after < sequence <= up_to`, in sequence order.
    /// The page ends after `limit.rows` events, or with the event whose data brings the page to
    /// `limit.bytes` bytes or more, whichever comes first; it is empty only when no event matches.
    /// The store reads at most the rows of that sequence range. An empty `types` gives an empty page
    /// without a read.
    async fn read_between(&self, after: Cursor, up_to: Cursor, types: &[EventType], limit: PageLimit)
        -> Result<Vec<StoredEvent>, EventLogError>;
    /// The highest stored sequence, `None` for an empty log.
    async fn latest_sequence(&self) -> Result<Option<Sequence>, EventLogError>;
    /// Deletes events recorded more than `retention` ago, by the clock of the store, oldest first.
    /// An adapter may bound what one call deletes; what is left waits for the next call. Returns the count.
    async fn prune(&self, retention: std::time::Duration) -> Result<u64, EventLogError>;
}

#[async_trait::async_trait]
pub trait EventCatalog: std::fmt::Debug + Send + Sync {
    /// Registers the types; a type that is already registered stays as it is.
    async fn register(&self, types: &[EventType]) -> Result<(), EventCatalogError>;
    /// One page of types ordered by name ascending, in byte order (the order of `EventType: Ord`).
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
>
> Assumption: the stored form is named `StoredEvent`, not `Event`. A struct `Event` with a field `event_type` fails `clippy::struct_field_names` (pedantic, denied by the gate: "field name starts with the struct's name"), and no suppression is planned. `NewEvent`, `StoredEvent` and `PageLimit` pass the same lint run (toolchain 1.99.1, the repository's `clippy.toml`).
>
> Assumption: the size limit of event data is a rule of ingestion only. `EventData::new` applies `EVENT_MAX_DATA_BYTES`; `EventData::stored` rebuilds what the log holds without a size limit, so lowering the variable never makes a stored row unreadable (a row that failed to map would stop the relay of every node at that row until retention removed it). `StoredEvent::restore` is the single fallible call of a row mapping: its three error cases are covered by unit tests of this crate, and an adapter maps its error with one `map_err`, without an error branch of its own that no stored row can reach.
>
> Assumption: the log has two reads because its two readers have different bounds. The relay reads every type after its cursor (`read_after`, bounded in rows). A replay reads some types inside a window of sequences (`read_between`), so that the database work of one page is bounded by the window whatever the types are, and its page is also bounded in bytes (NFR-9). `PageLimit::reached_by` is the one definition of "this page may have been cut"; the subscription and the in-memory fake use it, and the adapter implements the same cut in SQL.

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
| `http/dto.rs` | `ListEventTypesParams`, `EventTypeResponse`, `StreamParams`, `WsParams`, `Command`, `TypesError`, `parse_names`, `parse_types`, `parse_cursor` |
| `http/handler.rs` | `list_event_types`, `stream`, `ws`, `list_types` (shared with GraphQL), `MAX_MESSAGE_BYTES`, `READ_BUFFER_BYTES` |
| `http/frame.rs` | `Frame`, `event_json`, `error_json`, `write_json_string` |
| `http/hub.rs` | `Hub`, `HubSettings`, `HubError`, `Delivery`, `Subscriber` |
| `http/subscription.rs` | `Subscriptions`, `Subscription`, `SubscriptionRequest`, `Step`, `EndReason`, `TooManyTypes` |
| `http/session.rs` | `Session`, `Inbound` (the socket-free WebSocket state machine) |
| `http/socket.rs` | `Connection`, `impl Connection for WebSocket`, `serve`, `CLOSE_TIMEOUT` |
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
    /// The relay has its start cursor. Called once, after the cursor is stored and before the first
    /// page is read; from then on the publisher accepts subscribers.
    fn started(&self, cursor: Cursor);
    /// Hands one stored event to the local subscribers. Never waits. Returns the number of
    /// subscribers that fell behind with this event (their queue was full).
    fn publish(&self, event: &StoredEvent) -> usize;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayOutcome { Drained, More }
#[derive(Debug)]
pub struct RelayEvents { log: Arc<dyn EventLog>, publisher: Arc<dyn EventPublisher>, poll_interval: Duration, cursor: Option<Cursor> }
impl RelayEvents {
    pub const PAGE_SIZE: u32 = 500;
    pub const MAX_PAGES_PER_TICK: u32 = 20;
    pub fn new(log: Arc<dyn EventLog>, publisher: Arc<dyn EventPublisher>, poll_interval: Duration) -> Self;
    /// One round: see "Relay Algorithm". Logs `subscribers fell behind` at warn with `count`, once per
    /// page in which at least one subscriber fell behind.
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
/// `duplicates` is the number of valid events that the append skipped because their id was stored:
/// valid events minus `appended`, saturating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IngestReport { pub received: usize, pub appended: u64, pub invalid: usize, pub duplicates: u64 }
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
    /// After the acknowledgement, when `duplicates` is above zero, it logs `duplicate events skipped`
    /// at info with `count`, so that a producer that reuses an event id leaves a trace.
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
impl Frame { pub fn encode(event: &StoredEvent) -> Self; }
pub(crate) fn event_json(sequence: Sequence, event_type: &EventType, data: &str) -> String;
pub(crate) fn error_json(reason: &str) -> String;
/// Appends `text` as a JSON string, byte for byte what `serde_json::to_string(text)` writes:
/// `"` as \" and `\` as \\; U+0008 as \b, U+0009 as \t, U+000A as \n, U+000C as \f, U+000D as \r;
/// every other character below U+0020 as \u00xx with two lowercase hexadecimal digits;
/// everything else unchanged (U+007F, U+2028, U+2029 and characters outside the BMP included).
pub(crate) fn write_json_string(out: &mut String, text: &str);

// http/hub.rs
#[derive(Debug, Clone, Copy)]
pub struct HubSettings { pub max_subscribers: usize, pub buffer: usize }
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HubError {
    #[error("the node is starting")] NotReady,                       // the relay has no start cursor yet
    #[error("the node is at its subscriber capacity")] AtCapacity,
    #[error("the node is shutting down")] Closed,
}
// From<HubError> for Problem: every variant -> 503 `about:blank` with the error text as detail
// (FR-23), logged at warn as `subscription rejected` with `reason`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Delivery { pub delivered: usize, pub behind: usize }
// Debug by hand: `Hub { subscribers: <count>, types: <count>, .. }`
pub struct Hub { settings: HubSettings, registry: std::sync::Mutex<Registry>, count: tokio::sync::watch::Sender<usize> }
impl Hub {
    pub fn new(settings: HubSettings) -> Arc<Self>;
    /// Checked in this order under one lock: `Closed`, `NotReady`, `AtCapacity`.
    pub fn register(self: &Arc<Self>, types: &BTreeSet<EventType>) -> Result<Subscriber, HubError>;
    /// Sets the position of the hub when it has none; a second call changes nothing.
    pub fn start(&self, cursor: Cursor);
    /// Moves the position to the event and queues one shared frame for every subscriber of its type.
    /// `None` when the type has no subscriber: nothing is encoded.
    pub fn deliver(&self, event: &StoredEvent) -> Option<Delivery>;
    /// Ends every queue after the frames it holds; later `register` and `resume` calls answer `Closed`.
    pub fn close(&self);
    pub fn is_closed(&self) -> bool;
    /// Registered subscribers: every `Subscriber` value that has not been dropped.
    pub fn subscriber_count(&self) -> usize;
    /// Event types that have at least one queue.
    pub fn type_count(&self) -> usize;
    /// Completes when no subscriber is registered. What the composition root awaits at shutdown.
    pub async fn drained(&self);
}
impl EventPublisher for Hub { /* started -> start; publish -> deliver(event).map_or(0, |delivery| delivery.behind) */ }

// Debug by hand: the id
pub struct Subscriber { id: u64, position: Cursor, receiver: tokio::sync::mpsc::Receiver<Arc<Frame>>, hub: Arc<Hub> }
impl Subscriber {
    /// The position of the hub when this subscriber was registered.
    pub const fn position(&self) -> Cursor;
    /// The next queued frame. `None`: the queue ended, because the subscriber fell behind or the hub closed.
    pub async fn recv(&mut self) -> Option<Arc<Frame>>;
    /// A new, empty queue for the types of the subscriber, after `recv` returned `None`.
    /// `Err(HubError::Closed)` when the hub is closed.
    pub fn resume(&mut self) -> Result<(), HubError>;
    /// Adds the types and returns the position of the hub at that moment.
    #[must_use]
    pub fn subscribe(&self, types: &BTreeSet<EventType>) -> Cursor;
    pub fn unsubscribe(&self, types: &BTreeSet<EventType>);
    pub fn hub_closed(&self) -> bool;
}
impl Drop for Subscriber { /* removes the entry and its queues from the hub and publishes the new count */ }

// private
struct Entry { sender: Option<tokio::sync::mpsc::Sender<Arc<Frame>>>, types: BTreeSet<EventType> }
struct Registry {
    closed: bool,
    position: Option<Cursor>,                  // None until `start`: the hub is not ready
    next_id: u64,
    subscribers: BTreeMap<u64, Entry>,         // one entry per live `Subscriber`; removed only by its `Drop`
    by_type: HashMap<EventType, BTreeMap<u64, tokio::sync::mpsc::Sender<Arc<Frame>>>>,   // clones of `Entry::sender`
}
```

Rules of the registry; all of them hold after every method, and every method is one synchronous section under the mutex:

- **The entry owns the queue.** `Entry::sender` is the subscriber's own sender, and `by_type` holds one clone of it under each of the entry's types and under no other key: `subscribe` adds the names to `Entry::types` and the clones to `by_type` in the same section, and `unsubscribe` removes both. Falling behind, `Drop` and `resume` walk `Entry::types`, so a clone under a type that is missing there would never leave `by_type`, and the queue of that subscriber would never end. A subscriber with no type is therefore an ordinary entry whose queue stays open (FR-7 allows 0 types, FR-9 allows unsubscribing the last one), and a later `subscribe` clones the sender of the entry.
- **Falling behind.** When `try_send` fails for a subscriber, its `Entry::sender` becomes `None` and its clones leave `by_type`; the entry and its types stay. With the last sender gone, `recv` returns the frames already queued and then `None`. `resume` gives the entry a new channel of `buffer` frames and puts the clones back. While the sender is `None`, `subscribe` and `unsubscribe` change only `Entry::types`.
- **No empty key.** An entry of `by_type` whose map became empty is removed at once (unsubscribe, falling behind, drop), and `close` clears `by_type`. A client can subscribe to any well-formed name (FR-3), so without this rule `by_type` would keep every name ever used (NFR-9). `type_count` makes the rule testable.
- **The count is the number of connections.** `subscribers.len()` is what `MAX_SUBSCRIBERS` is compared with (`>=` answers `AtCapacity`) and what `count` publishes in `register` and in `Drop`. Both publish with `send_replace` while they still hold the registry lock, `register` in the same section as its `closed` check: published after the unlock, a registration that beat `close` could be invisible to a `drained` that already waits, and its WebSocket would lose close code 1001 (measured by the validation with the count raised after the unlock: 3 of 58 949 such registrations; none with the count raised under the lock). Falling behind and `close` do not change the count: a slot is free again only when the `Subscriber` value is dropped, which is when the connection has ended (FR-23).
- **Position.** `position` is `None` until `start`, then the cursor of the relay: `deliver` sets it to the sequence of every event, with or without subscribers. `register` copies it into the subscriber, `subscribe` returns it.
- `by_type` is only read and written by key; everything that is iterated is a `BTreeMap` or a `BTreeSet` (`iter_over_hash_type`). A poisoned mutex is taken over with `unwrap_or_else(PoisonError::into_inner)`, as `test-support` does. The subscriber id is a counter of the hub, not an identifier of the domain, so it does not come from `UuidGenerator`.
- No error arm exists for something that cannot happen: `try_send` is judged with `is_err()` (the receiver lives as long as the entry, so `Full` is the only error that occurs); a lookup of the subscriber's own id uses `Option` combinators (`map`, `and_then`, `unwrap_or_default`) and has no `else`; `drained` is `count.subscribe().wait_for(|count| *count == 0)` with its result bound to a named, unused variable (the sender lives in the hub that `&self` borrows).

> Assumption: a subscriber whose queue is full is not disconnected; it is taken off live delivery and catches up from the log on the same connection ("Subscription Algorithm"). The relay publishes a page at memory speed, far faster than the tasks of its subscribers are scheduled, so with eviction every relay round that carries more than `SUBSCRIBER_BUFFER` events of one type (a failed round, a lost listener with the poll fallback, a burst) would disconnect every subscriber of that type at once, however fast the clients are, and they would all reconnect with a cursor. Falling behind costs the same log reads as that reconnect and none of the connection churn, and it reuses the replay. This replaces the eviction of FR-22: the only client that is disconnected is one whose socket accepts no data (the write deadline of `socket::serve`, and the transport deadline named in "Failure Modes").
>
> Assumption: the frame is encoded under the registry lock, after the lookup found a subscriber, so `deliver` is one lock section with one lookup and no separate "has subscribers" check. The lock is then held for the encoding of one event (a copy of its data) in addition to the `try_send` calls; registrations wait that long.

```rust
// http/subscription.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionRequest { pub types: BTreeSet<EventType>, pub cursor: Option<Cursor> }
#[derive(Debug)]
pub struct Subscriptions { hub: Arc<Hub>, log: Arc<dyn EventLog>, permits: Arc<tokio::sync::Semaphore> }
impl Subscriptions {
    /// `replay_permits` log reads of all subscriptions of the node run at the same time; the others
    /// wait in the order they asked (the tokio semaphore is first in, first out).
    pub fn new(hub: Arc<Hub>, log: Arc<dyn EventLog>, replay_permits: usize) -> Self;
    /// Registers in the hub. No database read, nothing awaited.
    pub fn open(&self, request: SubscriptionRequest) -> Result<Subscription, HubError>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason { HubClosed, LogFailed }
/// What a subscription hands to its transport.
#[derive(Debug)]
pub enum Step {
    Frame(Arc<Frame>),      // write it
    Idle,                   // nothing to write; call `step` again
    End(EndReason),         // the stream ends; drop the subscription
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a connection subscribes to at most 100 event types")]
pub struct TooManyTypes;
#[derive(Debug)]
pub struct Subscription {
    subscriber: Subscriber,
    log: Arc<dyn EventLog>,
    permits: Arc<tokio::sync::Semaphore>,
    types: BTreeMap<EventType, Cursor>,   // type -> "since": only events above it are written
    floor: Cursor,                        // position of the connection: everything at or below it is settled
    target: Option<Cursor>,               // None: a replay must start; Some(t): replay while floor < t, else live
    pending: VecDeque<StoredEvent>,       // the rest of the last replay page
}
impl Subscription {
    pub const MAX_TYPES: usize = 100;
    pub const REPLAY_WINDOW: u64 = 10_000;                                              // sequences per page read
    pub const REPLAY_PAGE: PageLimit = PageLimit { rows: 500, bytes: 1_048_576 };
    /// One step; see "Subscription Algorithm". While `replaying()` is false the step reads nothing
    /// from the database and is safe to cancel.
    pub async fn step(&mut self) -> Step;
    /// `step` until a frame or the end: the next frame to write, `None` at the end of the stream.
    pub async fn next(&mut self) -> Option<Arc<Frame>>;
    /// True when the next `step` reads the log: `pending` is empty and a replay must start or run.
    /// False while events of a page are pending and while the subscription is live.
    pub fn replaying(&self) -> bool;
    pub fn subscribe(&mut self, types: BTreeSet<EventType>) -> Result<(), TooManyTypes>;
    pub fn unsubscribe(&mut self, types: &BTreeSet<EventType>);
    pub fn type_count(&self) -> usize;
    /// The floor.
    pub const fn position(&self) -> Cursor;
}

// http/session.rs: no socket, no timer
pub(crate) enum Inbound { Continue, Reply(Message), Close }
pub(crate) struct Session { subscription: Subscription }
impl Session {
    /// The largest text frame that is read as a command. 100 names of 128 characters in one command
    /// take 13 135 bytes.
    pub(crate) const MAX_COMMAND_BYTES: usize = 16_384;
    pub(crate) const fn new(subscription: Subscription) -> Self;
    /// Every `Message` variant is named: Text -> command, Binary -> error frame, Ping and Pong -> Continue, Close -> Close.
    pub(crate) fn on_message(&mut self, message: Message) -> Inbound;
    pub(crate) async fn step(&mut self) -> Step;
    pub(crate) fn replaying(&self) -> bool;
    /// The close frame of an ended stream: HubClosed -> 1001, LogFailed -> 1011.
    pub(crate) fn close_message(reason: EndReason) -> Message;
    pub(crate) fn type_count(&self) -> usize;
    pub(crate) const fn position(&self) -> Cursor;
}

// http/socket.rs
pub(crate) trait Connection: Send {
    fn receive(&mut self) -> impl Future<Output = Option<Result<Message, axum::Error>>> + Send;
    fn transmit(&mut self, message: Message) -> impl Future<Output = Result<(), axum::Error>> + Send;
}
impl Connection for axum::extract::ws::WebSocket { /* recv and send, nothing else */ }
/// How long the pump reads after a close frame, until the peer is gone.
pub(crate) const CLOSE_TIMEOUT: Duration = Duration::from_secs(1);
/// The pump: one task per connection; see "The WebSocket pump" below. `keep_alive` is the ping
/// period and the deadline of every write.
pub(crate) async fn serve<C: Connection>(connection: C, session: Session, keep_alive: Duration);

// http.rs
#[derive(Debug, Clone, Copy)]
pub struct StreamSettings { pub keep_alive: Duration }
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

The WebSocket pump (`socket::serve`), step by step. Its state is the connection, the session, `ping` (a `tokio::time::Interval` whose first tick comes `keep_alive` after the start, `MissedTickBehavior::Delay`) and the write deadline, which is `keep_alive`.

1. Every write goes through one function: `tokio::time::timeout(keep_alive, connection.transmit(message))`. Written: continue. Transport error: stop with `Gone`. Deadline: stop with `Stalled`.
2. The pump runs turns until one stops. Before each turn it asks `session.replaying()`.
   - **Replay turn** (`replaying()` is true: this step reads the log, which is one wait for a permit and one statement): the future of `session.step()` is pinned and polled to its end. While it is pending, a tick of `ping` writes a ping (rule 1) and the step is not dropped. No inbound frame is read in this turn.
   - **Live turn** (`replaying()` is false: the step hands out an event of a page that was already read, which awaits nothing, or awaits the queue): one `tokio::select!` over `connection.receive()`, `session.step()` and `ping.tick()`, in a function that only reports which arm completed. An inbound `None` or `Some(Err(_))` stops with `Gone` (a message or a frame above `MAX_MESSAGE_BYTES` arrives here as one `Some(Err(_))`); `Some(Ok(message))` goes to `session.on_message`: `Continue` continues, `Reply(frame)` is written, `Close` stops with `ClientClosed`. A tick writes a ping.
   - A replay therefore alternates between the two: one replay turn per log read, and live turns while the events of that page are written. A command of the client is read in one of those live turns. Reads that follow each other without an event to write (the read of the latest sequence and the first page, a page that returns no event) have no live turn between them.
   - The result of a step, in both turns: `Frame(frame)` writes `Message::Text(frame.json.clone())`; `Idle` continues; `End(reason)` stops with `Ended(reason)`.
3. After the last turn:

   | Stop | What the pump does |
   | --- | --- |
   | `Gone` | nothing |
   | `Stalled` | logs `subscriber evicted` at warn (`transport` `websocket`, `types` from `session.type_count()`, `last_sequence` from `session.position()`); nothing more is written |
   | `ClientClosed` | lingers |
   | `Ended(reason)` | writes `Session::close_message(reason)` (rule 1); when it was written, lingers |

   Lingering is `tokio::time::timeout(CLOSE_TIMEOUT, ..)` around a loop that calls `connection.receive()` until it returns `None`.
4. The function returns; the connection and the session are dropped, and `Drop for Subscriber` frees the slot.

Why each rule exists, with the library source it rests on:

- **Write deadline.** `WebSocket::send` is `SinkExt::send`, a feed and a flush with no deadline (`axum-0.8.9/src/extract/ws.rs:560-565`), and the flush stays pending while the socket accepts nothing (`tokio-tungstenite-0.29.0/src/lib.rs:365`). Without the deadline the task of a client that stops reading never ends and holds its slot, its queued frames and its replay page. With it such a task ends at most `STREAM_KEEP_ALIVE` after the write began, and dropping the socket closes the connection (FR-22).
- **A log read is not cancelled.** A dropped sqlx query is not cancelled on the server: the pooled connection reads the whole pending result before its next use (`sqlx-postgres-0.9.0/src/connection/mod.rs:94-103`), and a waiter that leaves the queue of the permit semaphore re-enters it at the end. If inbound frames could interrupt a page read, every 6-byte frame of a client would cost one more page query, and a client that sends faster than a read takes would stop its own replay (measured by the validation on the first draft: with a ping every 1 ms, 2 000 page queries were started and none completed). So no inbound frame is read while a log read is in flight, and the ping does not drop the step.
- **The rule is not wider than one read.** Inbound frames are read again as soon as the read has returned, while the events of its page are written. If commands waited for the whole replay, an unsubscribe sent during a replay would take effect only after every remaining frame of the removed type had been written (measured by the validation: 500 of 500 frames of the removed type followed the command; with the rule limited to the read, 0 to 4, with the same number of page reads).
- **Live turn.** In a live turn `session.step()` either hands out a pending event, which awaits nothing and so cannot be dropped half done, or awaits the queue (`mpsc::Receiver::recv` is safe to cancel and loses nothing), and `WebSocket::recv` keeps a partly read frame in the codec, so dropping the losing arms of the `select!` is harmless.
- **Lingering.** tungstenite queues the reply to a client's close frame and writes it only on the next read (`tungstenite-0.29.0/src/protocol/mod.rs:737` and `:455-466`); a pump that returned at once would drop the socket with the reply unwritten, and the client would see an abnormal closure (code 1006). After the server's own close frame the pump reads until the peer has answered and closed, so that the socket is not closed while inbound data is unread, which would reset the connection and could discard the close frame. `receive` returns `None` once the handshake is complete or the stream failed (`tokio-tungstenite-0.29.0/src/lib.rs:297-311`). A peer that never answers costs `CLOSE_TIMEOUT`.

Complexity: written as the functions `serve`, `run` (the loop), `replay_turn`, `pick` (the three-arm `select!`), `live_turn`, `on_inbound`, `on_step`, `send`, `finish` and `linger`, with `std::ops::ControlFlow<Stop>` as the result of a turn, the pump passes `clippy::cognitive_complexity` at 8 and the whole lint table with `-D warnings`; `pick` and `live_turn` in one function score 10.

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

/// Why a list of names is not a set of 1 to 100 event types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypesError {
    Empty,                 // no name at all
    Blank,                 // a name of length 0
    Name(InvalidEvent),    // `EventType::new` refused a name: `TypeLength` or `TypePattern`
    TooMany,               // more than 100 distinct names
}
/// The one rule for a list of names, for the query parameter and for the commands. Names are taken
/// in order; the first fault is returned: `Blank`, then `Name`, then `TooMany` as soon as the set
/// holds 101 names; `Empty` when the iterator gave nothing. A repeated name counts once.
pub(crate) fn parse_names<'a, I>(names: I) -> Result<BTreeSet<EventType>, TypesError>
where I: Iterator<Item = &'a str>;
/// The query parameter: `parse_names(raw.split(','))`; the error is the 422 problem with one violation on `types`.
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

- The two parameter structs derive `Validate` without a field rule: `ValidatedQuery` gives the problem-shaped 400 of a query that cannot be parsed (a missing `types` on the SSE route), and `parse_types` gives the 422.
- One typed core, `parse_names`, serves both callers, and each caller maps `TypesError` to its own answer. The limit of 100 counts distinct names everywhere (FR-4: a repeated name counts once).

  | `TypesError` | Query parameter: code of the 422 violation on `types` | Command: `error` text of the frame |
  | --- | --- | --- |
  | `Empty` | `required` | `types must hold 1 to 100 event type names` |
  | `Blank` | `required` | `invalid event type name` |
  | `Name(reason)` | `reason.code()`: `length` or `pattern` | `invalid event type name` |
  | `TooMany` | `max_items` | `types must hold 1 to 100 event type names` |

  `Session` passes the elements of the `types` array as they are (`names.iter().map(String::as_str)`), so `["a,b"]` is one invalid name and not two valid ones. `raw.split(',')` always gives at least one item, so the query reaches `Blank` for `types=` and for `a,,b`, and `Empty` only through a command with `"types": []`.
- `stream`: parse, `subscriptions.open(..)`, then `Sse::new(unfold(subscription, ..)).keep_alive(KeepAlive::new().interval(keep_alive))`, returned with one more header, `x-accel-buffering: no` (axum sets only `content-type` and `cache-control`, `axum-0.8.9/src/response/sse.rs:95-98`; without the header nginx buffers a proxied response and holds frames and keep-alives back). The `unfold` step is `subscription.next().await` mapped to `Ok::<Event, Infallible>(frame.sse.clone())`. The handler writes no log entry.
- `ws`, in this order: parse (422 or 400), `subscriptions.open(..)` (503), then the upgrade result: a rejection becomes `Problem::new(rejection.status()).with_detail(rejection.body_text())`, a success becomes `upgrade.max_message_size(MAX_MESSAGE_BYTES).max_frame_size(MAX_MESSAGE_BYTES).read_buffer_size(READ_BUFFER_BYTES).on_upgrade(move |socket| socket::serve(socket, Session::new(subscription), keep_alive))`. With this order every branch but the last line is reached by `oneshot` in a unit test; a request that is not an upgrade registers a subscriber and drops it at once, which reads nothing and awaits nothing.
- Limits of one WebSocket, as constants of `handler.rs`: `MAX_MESSAGE_BYTES = 32_768` (twice `Session::MAX_COMMAND_BYTES`) and `READ_BUFFER_BYTES = 4_096`. What they bound, read in `tungstenite-0.29.0`:
  - The defaults are a read buffer of 128 KiB, a message of 64 MiB and a frame of 16 MiB (`src/protocol/mod.rs:96-103`). The frame length is checked against `max_frame_size` when the header is parsed, and the buffer is then grown to the declared length (`src/protocol/frame/mod.rs:181-190`); `max_message_size` is checked only when a frame is complete. With the message limit alone a client makes the node hold 16 MiB per connection by sending one frame without its last byte (measured: +16 438 KiB per connection; with the frame limit +27 KiB and the connection ends at the header).
  - The read buffer is allocated per connection and filled with zeros on the first read (`frame/mod.rs:126` and `:233-237`); measured, an idle WebSocket costs 134 KiB with the default and 6 KiB with 4 096 bytes.
  - Inbound, a connection therefore holds at most the read buffer, one frame of at most 32 768 bytes and one message of at most 32 768 bytes.
  - Outbound, the write buffer is not set: `max_write_buffer_size` stays unlimited because a frame may be as large as a stored event, and the bound comes from the pump, which awaits `send` (feed and flush) before it writes the next message and ends the connection when a write misses its deadline. The buffer therefore holds at most one frame, one ping and the pong or close reply that tungstenite queues itself (it keeps one, `src/protocol/mod.rs:773-781`).
- GraphQL (`graphql.rs`): `#[graphql(name = "EventType")] struct EventTypeObject { name: String }` and `EventStreamQuery::event_types(ctx, #[graphql(default = 1)] page: i32, #[graphql(default = 30)] items_per_page: i32) -> async_graphql::Result<Vec<EventTypeObject>>`. It validates through `ListEventTypesParams` and calls `handler::list_types`, the same function the REST handler calls, then `problem_to_error`. The SDL field is `eventTypes(itemsPerPage: Int! = 30, page: Int! = 1): [EventType!]!`.

> Assumption: the size limit of a command is the constant `Session::MAX_COMMAND_BYTES` (16 384 bytes), not `HTTP_MAX_BODY_BYTES`. The largest valid command, 100 names of 128 characters, takes 13 135 bytes, so nothing is left for an operator to tune; and the variable was wrong in both directions: at its upper bound of 10 MiB the node would accept 20 MiB per message, at its lower bound of 1 024 it would refuse valid commands. A text message above the constant is answered with the error frame `the command is too large` (FR-10) as long as the transport lets it through, which is up to `MAX_MESSAGE_BYTES`; a larger message or frame is a transport error, `receive` returns it and the pump ends the connection without a close frame.
>
> Assumption: `Subscriptions::open` reads nothing from the database. The replay starts inside the stream, where it waits for a permit without holding a request open, so a reconnect storm cannot run into `HTTP_REQUEST_TIMEOUT`, and a log that cannot be read ends the stream instead of answering 500: a browser `EventSource` reconnects by itself after an ended stream and never after a 500 answer. The only answers of both routes that come from the server's state are therefore the three 503 of `HubError`.
>
> Assumption: `IngestReport::duplicates` and the entry `duplicate events skipped` are the only trace of an event that was skipped because its id was stored. The entry is at info, not warn, because a redelivery after a failed acknowledgement produces it in normal operation; it holds a count and no id.

Tests of the domain and application crates for the parts above (the hub, the subscription, the relay and the pump have their lists in "Subscription Algorithm" and "Relay Algorithm"):

- Domain: `EventData::new` at `max_bytes` and one byte above; `EventData::stored` (empty, with U+0000, 2 MiB accepted); `StoredEvent::restore` (a valid row; sequence 0; an invalid type; empty data; a row with two faults returns the first in the order sequence, type, data); `Cursor::advanced_by` (a plain sum and the saturation at `u64::MAX`); `PageLimit::reached_by` (an empty page; one event below and one at `rows`; one byte below and exactly at `bytes`; each with the other limit out of reach).
- `write_json_string`: its output equals `serde_json::to_string` of the same text, byte for byte, as a property over arbitrary strings and exhaustively for each of the 128 characters U+0000 to U+007F as a one-character string. `event_json` equals `serde_json::to_string` of a test struct with the members `id`, `type`, `data` in this order, for arbitrary data; `error_json` likewise. The oracle is equality of bytes, not a parse: a parsed comparison cannot see ` ` written for a space or `\u000a` for `\n`.
- `parse_names`: each `TypesError` and the order of the faults (a blank name before a later invalid one; `TooMany` at the 101st distinct name; `Empty` only for no name); 100 distinct names with repeats are accepted; `["a,b"]` is `Name(TypePattern)`. `parse_types` and `Session` map every `TypesError` as the table says.
- `Session::on_message`: every `Message` variant; every row of the error table; a text frame of exactly 16 384 bytes is read as a command and one of 16 385 bytes is answered with `the command is too large`; `Session::close_message` for both reasons.
- Handlers through `oneshot`: `stream` answers with the header `x-accel-buffering: no`; each `HubError` gives 503 with its detail and one `subscription rejected` entry; with a cursor and a failing log the answer is 200 and the body ends without a frame; `ws` reaches every problem and the 426.
- `tests/integration/ws.rs`, over a real socket, with a hub that the arrangement starts through `EventPublisher::started`:
  - the upgrade and one frame in each direction;
  - a handshake without `types` is answered with 101 and no close frame follows, and after a subscribe command a published event arrives (a subscriber without types is not closed);
  - a complete text frame of exactly 32 768 bytes is answered with the error frame `the command is too large`, and the connection then handles a subscribe command (the message and frame limits let the largest allowed frame through);
  - a frame header that declares 32 769 bytes, with no payload byte sent, ends the connection. Only the header-only form proves `max_frame_size`: a complete frame of that size ends the connection through `max_message_size` alone;
  - with a queue of 4 frames and a `RelayEvents` on the log fake whose first `tick` started the hub: 8 events of the subscribed type are appended and one more `tick` runs; the client reads the 8 frames in order and no close frame (a relay round larger than the queue disconnects nobody).

### Infrastructure crate

Manifest: `async-trait`, `aws-sdk-sqs`, `event-stream-domain`, `shared-domain`, `sqlx`, `thiserror`, `uuid`; dev `aws-sdk-sqs` with `test-util`, `aws-smithy-mocks`, `shared-infrastructure`, `test-support`, `tokio`.

| File under `crates/notification/event-stream/infrastructure/src/` | Content |
| --- | --- |
| `lib.rs` | modules and re-exports |
| `persistence.rs`, `persistence/event_log.rs` | `PgEventLog`, `PruneLimits`, `APPEND_LOCK_KEY`, `NOTIFY_CHANNEL` |
| `persistence/event_catalog.rs` | `PgEventCatalog` |
| `persistence/event_signal.rs` | `PgEventSignal` |
| `messaging.rs` | `SqsEventSource`, `SqsEventSourceSettings`, `VISIBILITY_TIMEOUT` |
| `persistence/event_log/tests.rs`, `persistence/event_catalog/tests.rs`, `messaging/tests.rs` | unit tests of the pure functions and, with `aws-smithy-mocks`, of the SQS calls |
| `tests/integration/{main.rs,support.rs,migration.rs,event_log.rs,event_catalog.rs,event_signal.rs,event_source.rs}` (crate root) | against PostgreSQL and LocalStack |

```rust
pub const APPEND_LOCK_KEY: i64 = 4_004_000_001;          // the only advisory lock of the service
pub const NOTIFY_CHANNEL: &str = "notification_events";
/// How long a received message stays hidden from the next receive, whatever the queue is configured with.
pub const VISIBILITY_TIMEOUT: Duration = Duration::from_secs(30);

/// Bounds of one `prune` call: rows per statement (Q7) and statements per call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PruneLimits { pub batch: u32, pub statements: u32 }
impl PruneLimits { pub const DEFAULT: Self = Self { batch: 200, statements: 5_000 }; }

#[derive(Debug)] pub struct PgEventLog { pool: sqlx::PgPool, prune: PruneLimits }
impl PgEventLog {
    /// The adapter of the service: `PruneLimits::DEFAULT`.
    pub const fn new(pool: sqlx::PgPool) -> Self;
    /// The same adapter with other prune bounds; the tests use small ones.
    pub const fn with_prune_limits(pool: sqlx::PgPool, prune: PruneLimits) -> Self;
}

#[derive(Debug)] pub struct PgEventCatalog { pool: sqlx::PgPool }
impl PgEventCatalog { pub const fn new(pool: sqlx::PgPool) -> Self; }

/// Owns a pool of one connection; the listener is created on the first `changed` call.
#[derive(Debug)] pub struct PgEventSignal { pool: sqlx::PgPool, listener: Option<sqlx::postgres::PgListener> }
impl PgEventSignal { pub const fn new(pool: sqlx::PgPool) -> Self; }

#[derive(Debug, Clone)]
pub struct SqsEventSourceSettings { pub queue_url: String, pub max_messages: i32, pub wait: Duration }
#[derive(Debug)] pub struct SqsEventSource { client: aws_sdk_sqs::Client, settings: SqsEventSourceSettings }
impl SqsEventSource { pub const fn new(client: aws_sdk_sqs::Client, settings: SqsEventSourceSettings) -> Self; }

// private to persistence/event_log.rs; every function below is pure and has unit tests
struct Row { sequence: i64, id: uuid::Uuid, event_type: String, data: String, occurred_at_micros: i64 }
struct Columns { ids: Vec<uuid::Uuid>, types: Vec<String>, data: Vec<String>, micros: Vec<i64> }   // the arrays of Q2
fn columns(events: &[NewEvent]) -> Result<Columns, EventLogError>;   // fails for a time that `to_micros` refuses
fn restore(row: Row) -> Result<StoredEvent, EventLogError>;          // StoredEvent::restore is its only fallible call
fn to_micros(time: SystemTime) -> Option<i64>;   // None before the epoch and above i64::MAX microseconds
fn from_micros(micros: i64) -> SystemTime;       // a negative value, which is never written, reads as the epoch
fn bound(value: u64) -> i64;                     // i64::try_from(value).unwrap_or(i64::MAX): cursors and limits
fn millis(duration: Duration) -> i64;            // whole milliseconds, i64::MAX when it does not fit
fn translate(error: sqlx::Error) -> EventLogError;
```

Shape of every adapter:

- Every `#[async_trait]` method only delegates to an inherent method (ADR 0008, as `PgExampleApiResourceRepository` does); rows are private structs; one `translate` function per adapter maps `sqlx::Error` to the port error.
- No error path is out of reach of the tests (NFR-16). A result that cannot fail gets no closure of its own: a foreign error is turned into the port error with two function paths, `.map_err(BoxError::from).map_err(EventLogError)` (or the error struct of the port in question), which add no function and no line to the coverage report. What can fail only for data that the check constraints keep out of the tables is decided in a pure function (`restore`, `columns`), and unit tests reach it with hand-built values, as `some-module-infrastructure` does.
- The adapters never sleep, retry or spawn (no tokio in this layer).

`PgEventLog`, one inherent method per port method; the statements are in "Data Model and Queries":

| Port method | Inherent method | What it does |
| --- | --- | --- |
| `append` | `insert` | an empty slice returns 0 without a statement; else `columns(events)?`, then Q1, Q2 and Q3 in one transaction; returns `rows_affected()` of Q2 |
| `read_after(cursor, limit)` | `select_after` | Q4 with `bound(cursor.value())` and `i64::from(limit)`; every row through `restore` |
| `read_between(after, up_to, types, limit)` | `select_between` | an empty `types` returns an empty page without a statement; else Q5 with `bound(after.value())`, `bound(up_to.value())`, the type names, and `limit.rows` and `limit.bytes` as `i64` (saturating); every row through `restore` |
| `latest_sequence` | `select_latest` | Q6, then the pure function `latest(Option<i64>)`: `None` for an empty table, else `Sequence::new(u64::try_from(value).unwrap_or(0))`; its error, which no stored value causes, is reached by a unit test with 0 and with -1 |
| `prune(retention)` | `delete_expired` | Q7 with `millis(retention)` and `i64::from(prune.batch)`, repeated until a statement deletes no row, at most `prune.statements` times (a `for` over that range); returns the saturating sum of `rows_affected()` |

- `restore(row)`: `StoredEvent::restore(u64::try_from(row.sequence).unwrap_or(0), Uuid::from_uuid(row.id), row.event_type, row.data, from_micros(row.occurred_at_micros))`. A negative sequence becomes 0, which `StoredEvent::restore` refuses like any other broken row, so the function has one fallible call and one error mapping. The data goes through `EventData::stored`: no size limit is applied to a stored row, and the adapter does not know `EVENT_MAX_DATA_BYTES`.
- Time. `to_micros` is `time.duration_since(UNIX_EPOCH)`, `as_micros()` (the digits below a microsecond are dropped) and `i64::try_from`; `columns` turns its `None` into an `EventLogError` before any statement runs. `from_micros` is `UNIX_EPOCH.checked_add(Duration::from_micros(..)).unwrap_or(UNIX_EPOCH)` on the value clamped at 0; it has no failure, because a strange time in one row must not stop the relay of every node at that row. The SQL of Q2, Q4 and Q5 converts without loss: every time `to_micros` accepts, up to `i64::MAX` microseconds, is read back as the same number.
- `delete_expired` is the only loop of the crate. Each statement is its own transaction, so a call that fails or is dropped at the fifth statement has deleted four batches. With `PruneLimits::DEFAULT` one call removes at most 200 x 5 000 = 1 000 000 rows; what is left waits for the next call. The loop ends on a statement that deletes nothing and not on a short batch, so its only comparison is `== 0`, and the mutant of that comparison stops after the first batch and is seen by the returned count.

`PgEventCatalog`: `register` runs Q8 (an empty slice returns without a statement), `find_page` runs Q9 with `page.limit()` and `page.offset()`; each name goes through `EventType::new` in a pure `restore` whose error is mapped as above.

`PgEventSignal::changed`:

1. When `listener` is `None`: `PgListener::connect_with(&pool)`, then `listen(NOTIFY_CHANNEL)`. An error of either returns at once and `listener` stays `None`.
2. Put the listener back into `self` (`Option::insert`), then await `recv()`. The listener is in `self` before the wait, so a call that is dropped while it waits keeps it.
3. `recv()` returned a notification: return `Ok(())`. Nothing is drained and nothing is folded here.
4. `recv()` returned an error: set `listener` to `None` and return the error; the next call starts at step 1 with a new listener.

What a call returns, read in `sqlx-postgres-0.9.0/src/listener.rs` and run against PostgreSQL 16 with the adapter written as above:

- One completion per notification. `recv()` loops over `try_recv()`, which returns one notification per call (lines 226 to 231 and 318 to 319). `next_buffered()` holds only what arrived while a statement ran on the listener connection (line 262: "This would only fill up if this listener is used as a connection"), in practice during `LISTEN`, and `try_recv()` returns those first (line 263), so they are not lost and need no drain.
- One notification per committed append. PostgreSQL sends a notification at commit and folds equal ones of one transaction. Run: three commits gave three completions and the fourth call stayed pending (30 of 30 runs); a rolled-back transaction gave none; one commit with two `pg_notify` calls gave one.
- Wake-ups are folded one level up, not here: `ListenForEvents::step` calls `wake.notify_one()` once per completion, `tokio::sync::Notify` keeps at most one permit, and one relay round reads up to `MAX_PAGES_PER_TICK` pages and is followed at once by the next round while rows remain ("Relay Algorithm"). N appends between two waits of the relay therefore cost N listen steps and one wake-up of the relay.
- A lost connection is not an error. `try_recv()` reconnects and listens again inside the same call (lines 286 to 308). Run: after `pg_terminate_backend` on the listener session the call stayed pending and the next commit woke it. Notifications sent in between are lost; the poll interval of the relay covers them.
- A call fails when the listener cannot be created or cannot reconnect (database unreachable: `PoolTimedOut` after `DATABASE_ACQUIRE_TIMEOUT`; run: 501 ms with a timeout of 500 ms) and when the pool is closed while it waits (`PoolClosed`, lines 268 to 276). An append committed before the listener exists is not seen; the relay's first round and its poll interval cover it.

`SqsEventSource`:

- `receive` (inherent `poll`): `receive_message()` with `queue_url(settings.queue_url)`, `max_number_of_messages(settings.max_messages)`, `wait_time_seconds(..)` and `visibility_timeout(..)` (the whole seconds of `settings.wait` and of `VISIBILITY_TIMEOUT` through one function `whole_seconds(Duration) -> i32`, which saturates), and `message_attribute_names` once for each of `event-type`, `event-id` and `event-time`. `visibility_timeout(i32)` is a method of the request builder (`aws-sdk-sqs-1.121.0`, `src/operation/receive_message/builders.rs:454`); without it the queue's own value applies, 30 s on a new queue and up to 12 hours.
- Each message goes through the pure function `inbound(Message) -> Result<InboundMessage, EventSourceError>`: `receipt_handle` missing is an error of the whole call (the message could never be acknowledged); `message_id` and `body` missing become empty strings, which the application then rejects as `Id` (unless an `event-id` attribute is present) and `Data`; the three attributes are looked up by key and read from `string_value`. The data type name is not checked: a `Number` attribute also carries a string value and is validated like any text, and a `Binary` attribute has none and counts as absent. The attribute map is never iterated.
- `acknowledge` (inherent `delete`): nothing for an empty list; otherwise one `delete_message_batch` per chunk of 10 (`receipts.chunks(10)`), entry ids `"0"` to `"9"` by position in the chunk. The entry builder's `build()` returns a `Result` that cannot fail here, because both of its required members are always set (`src/types/_delete_message_batch_request_entry.rs:88-103`); it is mapped with the two function paths of "Shape of every adapter" and has no closure. A non-empty `failed` list of an answer is an `EventSourceError` that carries the count; the batches before it stay deleted.

Tests of the crate:

- Unit, `persistence/event_log/tests.rs`: `columns` (order of the four arrays, a time before the epoch, a time above `i64::MAX` microseconds); `to_micros` and `from_micros` (0, a time with nanoseconds, `i64::MAX`, -1); `restore` (a valid row, sequence 0, sequence -1, an invalid type, empty data, data of 2 MiB); `latest`; `bound` (0, 2^63 - 1, 2^63, `u64::MAX`); `millis` (0, 1500 ms, `Duration::MAX`); `translate` keeps its source.
- Unit, `persistence/event_catalog/tests.rs`: `restore` with a valid and an invalid name.
- Unit, `messaging/tests.rs`, with `aws-smithy-mocks`: one rule that matches only a request with the queue URL, 10 messages, 5 s wait, 30 s visibility timeout and the three attribute names, so any other request fails the test and the mutants of `whole_seconds` die; `inbound` for a message with all three attributes, with none, with an attribute that has no string value and for one without a receipt handle; an empty answer; `acknowledge` of nothing makes no call; 11 receipts make two calls with the ids `"0"` to `"9"` and `"0"`; a non-empty `failed` list and a service error of each operation are an `EventSourceError` with a source.
- `tests/integration/migration.rs`: the two tables of the context, their named constraints and the two indexes of `notification_events`, read from the catalog with a filter on these two tables; `collation_name` of `notification_event_types.name` and of `notification_events.event_type` is `C` (`information_schema.columns`); the check constraints reject an invalid name and empty data.
- `tests/integration/event_log.rs` (`#[sqlx::test(migrator = "shared_infrastructure::persistence::MIGRATOR")]`):
  - `append`: order, idempotency by id, the gap a repeated id leaves, the time round trip for the epoch, for a time with nanoseconds (read back cut to the microsecond), for `9999-12-31T23:59:59.999999Z` and for `i64::MAX` microseconds;
  - `read_after`: cursor, limit, a cursor of `u64::MAX`; a row of 2 MiB inserted with SQL is read back whole;
  - `read_between`: the lower bound is exclusive and the upper inclusive, the type filter, the row limit, the byte limit (the row that reaches it is included; a limit of 1 byte gives one row), no types on a pool with a dead URL gives an empty page (no statement ran);
  - `latest_sequence` on an empty and on a filled table;
  - `prune`, with rows whose `recorded_at` the test sets by SQL: with a retention of 1 hour a row 30 minutes old stays and a row 2 hours old goes (this also kills the mutants of `millis`); with `PruneLimits { batch: 2, statements: 3 }` and 7 old rows, the first call returns 6 and leaves the row with the highest sequence, the second returns 1 and the third 0; with `PgEventLog::new` 450 old rows, more than two statements of the default batch, are gone after one call that returns 450;
  - two concurrent appenders with a paging reader never skip a row; an unreachable database is an `EventLogError` within the acquire timeout.
- `tests/integration/event_catalog.rs`: a type registered twice or by two concurrent calls is stored once; an empty slice; two concurrent `register` calls with the same 100 new names in opposite order both succeed; `find_page` returns `order-paid`, `order.paid`, `order.z`, `order_paid`, `ordera`, `orderpaid` in this order, with limit and offset.
- `tests/integration/event_signal.rs`. A test never drops a pending `changed` call to probe it: it pins the future and polls it with `tokio::time::timeout(Duration::from_millis(10), &mut call)`, which leaves the call alive. Every await of a call that must complete is inside a `tokio::time::timeout` of a few seconds too, so a mutant that leaves `changed` pending fails an assertion instead of being reported as a timed-out mutant (`.cargo/mutants.toml`). Every append of these tests is committed after the listener exists: an append before it is not seen.
  - First wake: poll the call (still pending) until `SELECT count(*) FROM pg_stat_activity WHERE datname = current_database() AND state = 'idle' AND query = 'LISTEN "notification_events"'` returns 1 (run: at most 4 polls), then append; the same call completes. The pending polls kill the mutant that replaces `changed` with `Ok(())`, and the test database has no other listener.
  - Then three appends give three completions, and a fourth call is still pending after 100 ms.
  - A pool with a dead URL: the call fails within the acquire timeout; after `Pool::set_connect_options` with the options of the test database (`sqlx-core-0.9.0/src/pool/mod.rs:555`) the next call passes the first-wake procedure.
  - The pool is closed while a call waits: the call fails, and so does the next one.
- `tests/integration/event_source.rs`, LocalStack, one queue per test: the round trip of the envelope; after `acknowledge` the queue reports `ApproximateNumberOfMessagesNotVisible` 0, which needs no wait; a missing queue is an error.

> Assumption: the default prune bounds are 200 rows per statement and 5 000 statements per call. Measured on PostgreSQL 16: a statement that deletes 200 rows of 200 bytes takes 1.4 ms, one that deletes 200 rows of 1 MiB each, the largest value `EVENT_MAX_DATA_BYTES` allows, takes 190 ms, so a statement stays far below the default `DATABASE_STATEMENT_TIMEOUT` of 5 s for every valid event size. A full call, 5 000 statements that delete a million rows, took 15 s; its statements slowed from 1.4 ms to 5.3 ms (the slowest took 26 ms), because each starts at the oldest index entry and steps over the entries of the rows deleted since the last vacuum. A row count that is safe for the largest events is small, so the call repeats the statement. 1 000 000 rows per `PRUNE_INTERVAL`, about 16 000 per second, is beyond what one leader can append: its appends are serial, one transaction of five round trips for at most 10 rows, after two calls of the queue. The bounds are a value of the adapter and not variables, because no deployment has to tune them; the constructor `with_prune_limits` exists so that the tests prove the bound with 7 rows instead of a million.
>
> Assumption: `prune` keeps its port signature and loops inside the adapter. The loop is a `for` over a fixed range whose every statement is bounded, and no other crate has to know the batch size. The port contract only narrows: one call deletes the oldest expired rows, at most a million of them, and a larger backlog takes several calls.
>
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

Contract of `Elector`, which "Election Algorithm" and the loops of the composition root rely on:

- `campaign` makes exactly one call of `LeaseStore::acquire_or_renew`, bounded by `call_timeout`, and publishes exactly one status. The elector keeps no state besides the watch channel, so a `campaign` that is dropped before it ends (the elect loop is cancelled at shutdown) changes nothing on the node: the status stays the last published one. Its statement may still have run on the database.
- `resign` reads the published status. Role `Leader`: one call of `release`, bounded by `call_timeout`; a failure or a timeout is logged as `lease store failed`, and the lease then ends by its expiry. Role `Follower`: no store call, so a follower stops without touching the database. In both cases `resign` publishes follower with no leader and the last term seen.
- A node whose `campaign` is never called keeps the initial status. That is the permanent state of a node without an event source ("Election Algorithm", who campaigns).

Tests of the crate (unit, `#[tokio::test(start_paused = true)]`, `InMemoryLeaseStore` unless stated):

- a won round, a lost round and a renewal: the returned and the published status, one `leadership changed` entry for the first two and none for the renewal; a receiver of `subscribe()` sees every status;
- a failing store and a hanging store: the round ends at once, or exactly `call_timeout` after its start, with `{ role: Follower, leader: None, term: the last term seen }` and one `lease store failed` entry;
- a slow store, a double defined in the crate's tests that answers after a scripted `tokio::time::sleep`: an answer `call_timeout - 1 ms` after the start still counts (leader), and the next round, which never answers, makes the node follower exactly `call_timeout` after its own start. These two figures and the pause of the loop give the step-down bound of "Election Algorithm";
- a `campaign` dropped by an outer `tokio::time::timeout` shorter than `call_timeout`: the status and the receiver are unchanged;
- `resign` as leader (exactly one `release`, then follower), as follower and on a new elector (the fake counted no call), with a failing and with a hanging `release` (one `lease store failed` entry, follower no later than `call_timeout`);
- the handler through `oneshot`: the body before the first round (`leader` `null`, `term` 0) and after a won and a lost round.

### Infrastructure crate

Manifest: `async-trait`, `cluster-domain`, `shared-domain`, `sqlx`, `uuid`; dev `shared-infrastructure`, `test-support`, `tokio`. Files: `lib.rs`, `persistence.rs` (`PgLeaseStore`, `LEASE_NAME`) with its sibling `persistence/tests.rs`, `tests/integration/{main.rs,migration.rs,lease_store.rs}`.

```rust
pub const LEASE_NAME: &str = "leader";
#[derive(Debug)] pub struct PgLeaseStore { pool: sqlx::PgPool }
impl PgLeaseStore { pub const fn new(pool: sqlx::PgPool) -> Self; }

// private
struct Row { holder: uuid::Uuid, term: i64 }                               // the columns of Q10 and Q11
fn restore(row: Row) -> Result<Leadership, LeaseStoreError>;              // u64::try_from(row.term), then Term::new
fn translate(error: sqlx::Error) -> LeaseStoreError;                      // every sqlx error is an unavailable store
fn millis(duration: Duration) -> i64;                                     // whole milliseconds, i64::MAX when it does not fit
```

- The two `#[async_trait]` methods only delegate to the inherent methods `campaign` (Q10, then Q11 when Q10 returned no row) and `expire` (Q12); the statements and their order are in "Election Algorithm".
- No branch of the adapter is out of reach of its tests (NFR-16):
  - Q11 is read with `fetch_one`. The row is never deleted, so a missing row cannot happen in a migrated database; if it does, it is `sqlx::Error::RowNotFound` and takes the same `translate` as every other failure, with no error path of its own.
  - `restore` is a pure function. The check constraint `cluster_leases_term_check` keeps a term below 1 out of the table, so its two failures (a term of 0, a negative term) are reached by unit tests in `persistence/tests.rs` with hand-built rows, as `some-module-infrastructure` does for its `restore`.
  - `millis` saturates with `unwrap_or(i64::MAX)`, which has no closure and no line of its own.
- Tests of the crate:
  - unit (`persistence/tests.rs`): `restore` with a valid row, with term 0 and with term -1; `translate` keeps the source; `millis` for 0, for 1500 ms and for `Duration::MAX`;
  - `tests/integration/migration.rs`: the table, `cluster_leases_pkey` and `cluster_leases_term_check` (term 0 is rejected), read from the catalog with a filter on `cluster_leases`, never from the whole schema;
  - `tests/integration/lease_store.rs`, two node ids A and B on one `#[sqlx::test]` database, no sleep and no wait in any case: the sequence measured in "Election Algorithm"; takeover without `release` after A renewed with `Duration::ZERO` (the lease then expires at the `now()` of that statement, and Q10 takes over on `<=`); `release` by a node that does not hold the lease changes nothing; `release` keeps the row and its term; after A acquired with a lease of one hour, `SELECT expires_at - now()` read by the test lies between 59 and 60 minutes, which proves that the lease time is bound in milliseconds and kills the mutants of `millis`; an unreachable database is a `LeaseStoreError` within the acquire timeout.

> Assumption: the takeover test uses a lease of zero, not of 1 ms. With 1 ms the second node took over in 17 of 500 attempts on the local database, with zero in 500 of 500: the outcome of a 1 ms lease depends on whether a millisecond passed between two statements.

## Composition Root

All changes are in `crates/service`. Its manifest gains the six crates, and `tokio-tungstenite` plus `futures-util` with `sink` as dev-dependencies. The workspace entry of `tower-http` gains the feature `cors`, which adds no crate (`cors = []` in `tower-http-0.7.1/Cargo.toml`).

### Adapters

New public fields of `bootstrap::Adapters`, each replaceable by a test:

```rust
pub event_log: Arc<dyn EventLog>,                   // control pool: relay, ingestion, pruning
pub replay_log: Arc<dyn EventLog>,                  // request pool: the replays of `Subscriptions`
pub event_catalog: Arc<dyn EventCatalog>,           // control pool: ingestion and the seed
pub catalog_reader: Arc<dyn EventCatalog>,          // request pool: `ListEventTypesHandler`
pub event_source: Option<Arc<dyn EventSource>>,     // None when EVENT_QUEUE_URL is unset
pub event_signal: Box<dyn EventSignal>,             // listener pool
pub lease_store: Arc<dyn LeaseStore>,               // control pool
```

A test assigns one fake to both fields of a pair (`Arc::clone`).

`Adapters::connect` still touches no network. It builds three lazy pools from the database settings; they differ only in their size:

| Pool | Connections | What uses it |
| --- | --- | --- |
| requests | `DATABASE_MAX_CONNECTIONS` | what a client request drives: the example repository, `PostgresProbe`, `catalog_reader`, `replay_log` |
| control | `CONTROL_POOL_CONNECTIONS` = 3 | what the loops drive: `lease_store`, `event_log`, `event_catalog` |
| listener | `LISTENER_POOL_CONNECTIONS` = 1 | `event_signal` |

A node therefore opens at most `DATABASE_MAX_CONNECTIONS + 4` connections (NFR-12).

Why three pools. The sqlx pool hands out connections in arrival order (`fair`, `sqlx-core-0.9.0/src/pool/options.rs:163`), so on one pool a lease renewal, a relay round or an append waits behind every queued client read, and client reads are not bounded in number: one catalog query per request, and the reads of every replay. Measured by the validation on the template's pool of 10: 30 replay requests per second delayed the campaigns up to the acquire timeout of 2 s, at which the leader steps down, and a reconnect storm of 30 000 clients failed the campaign and the relay round. With the control pool the campaigns answered within 0.05 s under the same storm (`probes/security-resilience-refute/`, runs `a` to `j`). A client read can now never take a control connection, and a loop never takes a request connection. Inside the request pool the replays are held to half of the connections by `replay_permits` ("Subscription Algorithm", bounds), so the catalog, the health probe and the example routes keep finding one.

The derived settings are pure functions of `bootstrap.rs`. Written as struct literals with a base inside `connect` they would have mutants ("delete field `max_connections`", "delete field `operation_timeout`") that no test can see behind `Box<dyn EventSignal>` and `Arc<dyn EventSource>`; as functions they are asserted field by field.

```rust
const CONTROL_POOL_CONNECTIONS: u32 = 3;
const LISTENER_POOL_CONNECTIONS: u32 = 1;
pub const QUEUE_WAIT: Duration = Duration::from_secs(5);     // the long poll of FR-15
pub const QUEUE_BATCH: i32 = 10;

/// The database settings with another pool size: `PostgresSettings { max_connections, ..database.clone() }`.
fn pool_settings(database: &PostgresSettings, max_connections: u32) -> PostgresSettings;
/// The settings of the ingestion client: `operation_timeout` is `sqs.operation_timeout.saturating_add(QUEUE_WAIT)`,
/// the other fields are copied.
fn ingestion_settings(sqs: &SqsSettings) -> SqsSettings;
/// Replay reads that may run at one time: `usize::try_from(max_connections / 2).unwrap_or(usize::MAX).max(1)`.
fn replay_permits(max_connections: u32) -> usize;
/// The time limit of one seed, relay, prune or ingest round:
/// `acquire_timeout x 2 + statement_timeout x 6 + ingestion.operation_timeout`, with `saturating_mul` and `saturating_add`.
fn round_timeout(database: &PostgresSettings, ingestion: &SqsSettings) -> Duration;
```

- `connect` calls `connect_lazy` with `&config.database`, with `&pool_settings(&config.database, CONTROL_POOL_CONNECTIONS)` and with `&pool_settings(&config.database, LISTENER_POOL_CONNECTIONS)`. `PgEventLog::new` and `PgEventCatalog::new` are each called twice, once per pool.
- When `EVENT_QUEUE_URL` is set, a second SQS client is built for ingestion with the same factory: `sqs_client(&ingestion_settings(&config.sqs))`, and `SqsEventSource::new(client, SqsEventSourceSettings { queue_url, max_messages: QUEUE_BATCH, wait: QUEUE_WAIT })`. On the shared client an empty long poll of 5 s would be cut by `SQS_OPERATION_TIMEOUT` (2 s). The plain sum `operation_timeout + QUEUE_WAIT` is not used: `Duration + Duration` fails `clippy::arithmetic_side_effects`. The client of the health probe, `SQS_OPERATION_TIMEOUT` and the three existing cross-field rules of `config.rs` are unchanged.
- To stay under 50 lines, `connect` delegates to two private functions: `pools(&PostgresSettings) -> Result<Pools, sqlx::Error>`, where `Pools { requests, control, listener }` is a private struct, and `async fn event_source(config: &Config) -> Option<Arc<dyn EventSource>>`.

> Assumption: the control pool has three connections and is not a variable. Its users are the elect, relay, ingest and prune loops and, for one statement at start, the seed loop. Each holds a connection for one statement or one short transaction, the pool queues in arrival order, and three leaves a spare when a statement that was cut by the round limit still occupies its connection. An operator sizes `DATABASE_MAX_CONNECTIONS` for clients; nothing in a deployment changes what the loops need.
>
> Assumption: `round_timeout` is derived from the configuration, and is neither a constant nor a variable. A round must be able to finish in every accepted configuration, and `DATABASE_STATEMENT_TIMEOUT` may be anything from 1 ms to 24 d. The longest round is `IngestEvents::process`: two pool acquires (register, append), six statements (Q8, then `BEGIN`, Q1, Q2, Q3 and `COMMIT` of the append) and one acknowledge call. With the defaults the limit is 2 x 2 s + 6 x 5 s + 7 s = 41 s. A statement that the database itself would still answer is therefore never cut, and a round that exceeds the limit is hung.

### Application, state, routes, schema

```rust
// bootstrap.rs
pub struct Application { pub router: Router, pub background: Background }
pub fn application(adapters: Adapters, config: &Config) -> Result<Application, BootstrapError>;
pub fn router(adapters: Adapters, config: &Config) -> Result<Router, BootstrapError>;   // kept: application(..)?.router
```

`application` does, besides today's wiring:

1. node id: `NodeId::new(config.cluster.node_id.unwrap_or_else(|| adapters.uuid_generator.generate()))`. With `NODE_ID` unset the node id is the first value the generator returns in this process;
2. `let hub = Hub::new(HubSettings { max_subscribers, buffer })`, an `Arc<Hub>`; `Subscriptions::new(Arc::clone(&hub), adapters.replay_log, replay_permits(config.database.max_connections))`;
3. `Arc::new(Elector::new(node, adapters.lease_store, ElectorSettings { ttl: CLUSTER_LEASE_TTL, call_timeout: CLUSTER_RENEW_INTERVAL }))`. "Election Algorithm" computes its bounds from this call timeout and from the pause of the elect loop;
4. `register_queries` also registers `ListEventTypesHandler::new(adapters.catalog_reader)`;
5. `AppState` gains `event_stream: EventStreamState { query_bus, subscriptions, settings: StreamSettings { keep_alive } }` and `cluster: ClusterState { status: elector.subscribe() }` (the derive `FromRef` covers them);
6. the schema gets `.data(event_stream.clone())`;
7. `Background` is filled:

   | Field | Value |
   | --- | --- |
   | `elector`, `hub` | the two `Arc`s of steps 2 and 3 |
   | `relay` | `RelayEvents::new(event_log, hub as Arc<dyn EventPublisher>, EVENT_RELAY_POLL_INTERVAL)` |
   | `wake`, `listen`, `signal` | one `Arc<Notify>`; `ListenForEvents::new(wake, RETRY_DELAY)`; `adapters.event_signal` |
   | `ingest` | `adapters.event_source.map(..)`: `Arc::new(IngestEvents::new(source, event_log, event_catalog, clock, EVENT_MAX_DATA_BYTES))`; `None` without a queue |
   | `prune` | `Arc::new(PruneEvents::new(event_log, EVENT_RETENTION))` |
   | `seed` | `SeedEventTypes::new(event_catalog, EVENT_TYPES)` |
   | `schedule` | `Schedule { renew_interval: CLUSTER_RENEW_INTERVAL, prune_interval: PRUNE_INTERVAL, retry_delay: RETRY_DELAY, round_timeout: round_timeout(&config.database, &ingestion_settings(&config.sqs)) }` |

   `event_log` and `event_catalog` are the control-pool adapters. `application` stays under 50 lines and 5 arguments by building the event-stream part and the cluster part in two private functions.

`bootstrap::router` returns the router and drops the `Background`. The hub of such a router is never started, so its two streaming routes answer the 503 problem `the node is starting`; the catalog, the cluster status and every existing route work as with `application`. A test that subscribes builds an `Application` and makes the hub ready ("Test Strategy").

Routes: `openapi::api_router()` adds `.nest("/api", event_stream_application::http::router())` and `.nest("/api", cluster_application::http::router())`, and `ApiDoc` gains the tags `EventStream` ("Event catalog endpoints.") and `Cluster` ("Cluster status endpoints."). The streaming routes arrive inside the context router and get the whole middleware stack. The request budget wraps only the handler future, which returns the response head; the body stream and the upgraded socket are outside it. `DefaultBodyLimit` does not apply to an upgraded socket. `http::router::assemble` gains the two layers of "Browser origins" below and nothing else.

GraphQL: `pub struct Query(ExampleApiResourceQuery, EventStreamQuery);` in `graphql/schema.rs`. The descriptions of `EventType`, `name`, `eventTypes` and its two arguments must not contain `node`, `Connection`, `update`, `delete` or `type Subscription`: an existing test searches the whole SDL for these strings ("Test Strategy", existing tests).

### Browser origins

The clients of this service are browser frontends, and they are served from other origins than the service. Today nothing in the service answers a cross-origin request: no response carries `access-control-allow-origin`, and a preflight `OPTIONS` gets the 405 problem, so a page on another origin can read neither the catalog nor an SSE stream and cannot post to GraphQL. A browser applies no such rule to a WebSocket, so that route would be open to the pages of every site. This change adds one allow-list for both, `CORS_ALLOWED_ORIGINS` (FR-28), with the default "no other origin".

| `CORS_ALLOWED_ORIGINS` | Catalog, GraphQL, SSE and every other HTTP route | WebSocket handshake |
| --- | --- | --- |
| unset (default) | exactly as today: no CORS header, `OPTIONS` is the 405 or 404 problem. A browser lets only pages of the service's own origin read the answers | refused with the 403 problem when it carries an `Origin` header |
| a list of origins | a request whose `Origin` is on the list gets `access-control-allow-origin: <that origin>` and `vary: origin` on every answer, problems and the SSE response head included; a preflight is answered by the layer; a request from another origin is served without the header, and its browser hides the answer | accepted when `Origin` is on the list, else the 403 problem |
| `*` | `access-control-allow-origin: *` on every answer | accepted from every origin |

A request without an `Origin` header is never restricted, on any route: command-line clients, other services and the test clients send none (read in `tungstenite-0.29.0/src/client.rs`: the handshake has `Host`, `Connection`, `Upgrade`, `Sec-WebSocket-Version` and `Sec-WebSocket-Key`; run with the pinned K6 image: neither `k6/ws` nor `k6/websockets` sends `Origin`). The list is a rule for browsers, not an authorization (NFR-13): it decides which sites may embed the service, and every event stays readable by any client that can reach the service.

```rust
// config.rs
/// The origins whose pages may use the service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllowedOrigins { NoOrigin, Any, List(Vec<HeaderValue>) }
impl AllowedOrigins {
    /// `NoOrigin`: false; `Any`: true; `List`: the list contains the value, byte for byte.
    pub fn allows(&self, origin: &HeaderValue) -> bool;
}
// Config gains `pub allowed_origins: AllowedOrigins`.

// http/origin.rs (new, declared in http.rs)
pub const PREFLIGHT_MAX_AGE: Duration = Duration::from_secs(600);
/// The CORS layer, or `None` for `NoOrigin`: the router then has no such layer.
pub fn cors(origins: &AllowedOrigins) -> Option<CorsLayer>;
/// Middleware: the 403 problem `the origin is not allowed` for a refused handshake, else `next.run(request)`.
pub(crate) async fn websocket_origin(State(origins): State<Arc<AllowedOrigins>>, request: Request, next: Next) -> Response;
/// A handshake (it has `sec-websocket-key`) whose `origin` header is present and not allowed.
fn refused(origins: &AllowedOrigins, headers: &HeaderMap) -> bool;
```

- `cors` builds `CorsLayer::new().allow_origin(..).allow_methods([Method::GET, Method::POST]).allow_headers([CONTENT_TYPE, HeaderName::from_static("last-event-id")]).max_age(PREFLIGHT_MAX_AGE)`, with `AllowOrigin::any()` for `Any` and `AllowOrigin::list(list.iter().cloned())` for `List`. No credentials are allowed, so `*` is valid, and nothing is exposed. `GET` and `POST` are the methods of every route of the router; `content-type` is what makes a JSON `POST` a preflighted request, and `last-event-id` is what a client sends to resume.
- Order of the stack in `assemble`, outermost first: request id (three layers), security headers, `TraceLayer`, then `.option_layer(origin::cors(&config.allowed_origins))`, then `from_fn_with_state(Arc::new(config.allowed_origins.clone()), origin::websocket_origin)`, then `CatchPanicLayer`, the request budget and the body limit. The stack has nine layers, and ten when an origin is allowed.
- `refused` is `headers.contains_key(SEC_WEBSOCKET_KEY) && headers.get(ORIGIN).is_some_and(|origin| !origins.allows(origin))`. The middleware is not a handler and is `pub(crate)`, so the existing test that lists the files with a `pub async fn` handler is unchanged.

What the design rests on, read in `tower-http-0.7.1/src/cors/` and run with `axum 0.8.9` and `tower 0.5.3` (probes `probes/security-resilience-refute/cors-probe` and `revision/C-probes/origin` in the scratchpad of this revision):

- **The layer must be absent, not empty, when no origin is allowed.** An installed `CorsLayer` answers every `OPTIONS` request itself with 200 and an empty body (`mod.rs`, `call`: `parts.method == Method::OPTIONS`), also for an unknown path and with an empty list, and adds `vary: origin` to every answer. That would change two existing status codes (NFR-20) and the answers the Schemathesis gate judges (`--checks all`). `tower::ServiceBuilder::option_layer(None)` adds nothing (run: `OPTIONS /api/graphql` stays 405).
- **The position.** Directly outside `TraceLayer` the layer does not compile (`Cors` needs a response body with `Default`, and the body of `TraceLayer` has none). Innermost it compiles, but the 503 of the request budget and the 500 of the panic catcher are built outside it and carry no header, so a browser hides those problem documents. Between `TraceLayer` and `CatchPanicLayer` every answer of an allowed origin carries the header, the 404, 405, 500 and 503 problems and the SSE response head included, and a preflight is logged with its request id (run).
- **`*` never enters a list.** `AllowOrigin::list` panics on a wildcard (`allow_origin.rs:55-66`), and list entries are compared byte for byte, so the configuration accepts either exactly `*` or a list of origins in the form a browser sends ("Configuration").
- **The handshake guard.** Run with the three settings: without `Origin` the handshake reaches the upgrade in every setting; with an origin that is not allowed it is answered 403 before the handler runs, so no subscriber is registered; with `*` every origin passes; a plain `GET` of the WebSocket route with a foreign `Origin` is not a handshake and gets the answer of the handler.

> Assumption: the allow-list is a variable, and the WebSocket route follows the same list. Which origins serve the frontends is different in every deployment, so a constant cannot hold it. A list that covered only the HTTP routes would be no rule at all: a page of any site could read the same events over the WebSocket route.
>
> Assumption: with the variable unset a WebSocket handshake that carries an `Origin` is refused, also when the page comes from the service's own origin. Behind a proxy the service does not know its public origin (the `Host` header may be rewritten), so "same origin" cannot be decided reliably; an operator who serves the frontend from the origin of the service lists that origin. The HTTP routes need no such entry, because a browser allows same-origin requests by itself.
>
> Assumption: a browser page can still make its visitors' browsers open SSE requests without reading them (a request without CORS carries no `Origin` and cannot be told apart). That is the per-client limit the PRD leaves to the edge proxy; the list does not claim to prevent it.

### Configuration

`cli.rs` gets one `Option<String>` argument per variable with `env = ".."` and `hide_env_values = true`; `config.rs` parses them into four new members of `Config`: `cluster`, `events`, `streams` and `allowed_origins` ("Browser origins").

```rust
#[derive(Debug, Clone)]
pub struct ClusterConfig { pub node_id: Option<Uuid>, pub lease_ttl: Duration, pub renew_interval: Duration }
#[derive(Debug, Clone)]
pub struct EventsConfig { pub queue_url: Option<String>, pub retention: Duration, pub relay_poll_interval: Duration,
                          pub max_data_bytes: usize, pub types: Vec<EventType> }
#[derive(Debug, Clone, Copy)]
pub struct StreamsConfig { pub subscriber_buffer: usize, pub max_subscribers: usize, pub keep_alive: Duration }
```

`Uuid` is `shared_domain::Uuid`. Each member is built by a function of the existing form `fn(&mut Collector, &ServeArgs) -> Option<..>`; one private struct groups the four, so that the `match` of `Config::from_args` gains one tuple member and the function stays inside the complexity limits.

| Variable | Type | Default | Bounds and reason text |
| --- | --- | --- | --- |
| `NODE_ID` | UUID | unset: generated by `UuidGenerator` | `must be a UUID` |
| `CLUSTER_LEASE_TTL` | duration | `10s` | `1s` to `5m` |
| `CLUSTER_RENEW_INTERVAL` | duration | `3s` | `100ms` or more, and at most a third of `CLUSTER_LEASE_TTL` (the lease rule below) |
| `EVENT_QUEUE_URL` | `http://` or `https://` URL | unset: the node never ingests and never campaigns | the existing `endpoint` check; an empty value is invalid |
| `EVENT_RETENTION` | duration | `1h` | `1m` to `30d` |
| `EVENT_RELAY_POLL_INTERVAL` | duration | `1s` | `100ms` to `1m` |
| `EVENT_MAX_DATA_BYTES` | integer | `65536` | 1 to 1 048 576 |
| `EVENT_TYPES` | comma-separated event type names | unset: nothing registered at start | every name a valid `EventType`: `must be a comma-separated list of event type names` |
| `SUBSCRIBER_BUFFER` | integer | `256` | 1 to 65 536 |
| `MAX_SUBSCRIBERS` | integer | `100000` | 1 to 1 000 000 |
| `STREAM_KEEP_ALIVE` | duration | `15s` | `1s` to `5m` |
| `CORS_ALLOWED_ORIGINS` | `*`, or comma-separated origins | unset: no other origin | `must be * or a comma-separated list of origins such as https://app.example.com` |

What each variable means now, where the validation changed it: `SUBSCRIBER_BUFFER` is the number of frames a subscriber may lag before it leaves live delivery and catches up from the log; `MAX_SUBSCRIBERS` counts open connections; `HTTP_MAX_BODY_BYTES` no longer applies to WebSocket messages; `DATABASE_MAX_CONNECTIONS` sizes the request pool, and half of it is the number of replay reads that run at one time.

`CORS_ALLOWED_ORIGINS` is parsed by `fn allowed_origins(raw: Option<&str>) -> Parsed<AllowedOrigins>`: unset gives `NoOrigin`; the value `*` gives `Any`; otherwise every item of `raw.split(',')` must be an origin, and one that is not makes the whole value invalid. `fn origin(item: &str) -> Option<HeaderValue>` accepts an item when all of this holds: `Uri::from_str(item)` succeeds; the scheme is `http` or `https`; the authority passes the existing `has_host_and_valid_port` and holds no `@`; `item == format!("{scheme}://{authority}")`, which refuses a path, a trailing slash and a query; `item` holds no ASCII upper-case letter; `HeaderValue::from_str(item)` succeeds. Run: `https://app.example`, `http://localhost:3000` and `http://[::1]:3000` are accepted; `https://app.example/`, `https://app.example/x`, `https://App.Example`, `https://user@app.example`, `ftp://app.example`, `app.example`, `null`, `*` inside a list and the empty string are refused. The form is strict because the list is compared byte for byte with what a browser sends, and a browser sends the origin in lower case, without a path and without the default port; an entry in another form would never match and nothing would say so.

The lease rule is a method of its own, `Collector::lease_rule(&mut self, renew: Option<Duration>, ttl: Option<Duration>)`, called from `Config::from_args` after `cross_field_rules`, so the rules between two variables still come last. `cross_field_rules` already has five parameters, the limit of `clippy::too_many_arguments`. The rule is the one "Election Algorithm" derives: `3 * CLUSTER_RENEW_INTERVAL <= CLUSTER_LEASE_TTL`, written `renew.saturating_mul(3) > ttl` for the violation and reported on `CLUSTER_RENEW_INTERVAL` with the reason `must be at most a third of CLUSTER_LEASE_TTL`. Equality is accepted: with a lease of `9s`, `3s` passes and `3001ms` fails; with the default lease of `10s`, `3333ms` passes and `3334ms` fails. A new helper `bounded_duration(raw, default, range)` serves the duration rows. No reason text holds a value (FR-25). `service check-config` uses the same code.

Constants that are not variables: `QUEUE_WAIT = 5 s`, `QUEUE_BATCH = 10`, `PRUNE_INTERVAL = 60 s`, `RETRY_DELAY = 1 s`, `CONTROL_POOL_CONNECTIONS = 3` and `LISTENER_POOL_CONNECTIONS = 1` in `bootstrap.rs`; `STREAM_DRAIN_TIMEOUT = 2 s` in `background.rs`; `PREFLIGHT_MAX_AGE = 10 min` in `http/origin.rs`.

> Assumption: the long-poll wait, the batch size, the prune period, the retry delay, the two pool sizes, the drain limit and the preflight age are constants: none needs tuning per deployment. The list of variables grows from eleven to twelve; the twelfth is the allow-list of origins, which no constant can hold.

Each variable gets its row in `docs/environment.md`, which rule 5 of `make.docs_check` compares with the help of `serve`, the compose files and `.env`. `.env` gets nine lines: `EVENT_QUEUE_URL=http://${LOCALSTACK_HOST}:${LOCALSTACK_PORT}/000000000000/notification-events` and the eight variables that have a default (`CLUSTER_LEASE_TTL`, `CLUSTER_RENEW_INTERVAL`, `EVENT_RETENTION`, `EVENT_RELAY_POLL_INTERVAL`, `EVENT_MAX_DATA_BYTES`, `SUBSCRIBER_BUFFER`, `MAX_SUBSCRIBERS`, `STREAM_KEEP_ALIVE`), each written with that default.

> Assumption: `NODE_ID`, `EVENT_TYPES` and `CORS_ALLOWED_ORIGINS` have no line in `.env` and are documented in `docs/environment.md` only. A node id in a shared file would give two local nodes the same identity, and an empty `EVENT_TYPES=` is not the same as an unset variable. The test targets run with the values of `.env` in their environment (`scripts/with-env.sh .env .env.test`), and the in-process suites read the configuration from it, so an allow-list in `.env` would install the CORS layer in every existing suite. FR-25 is amended accordingly: an entry in `.env` for a variable that has a default or a local value.
>
> Assumption: the default of `MAX_SUBSCRIBERS` stays 100 000, and the production compose reference sets its own value. The memory limit of that reference is 256 MiB, and a protected test asserts the figure (`tests/CLI/bats/make_security.bats`), so the limit cannot follow the cap. 100 000 idle SSE streams need about 2 GiB ("Performance Design"); the reference therefore sets `MAX_SUBSCRIBERS: ${MAX_SUBSCRIBERS:-5000}`, about 100 MiB of idle streams, and `docs/operations.md` gives the rule (the cap times 20 KiB stays below half of the memory limit) for whoever raises both.

### Background tasks, supervision and shutdown

`crates/service/src/background.rs` (new, declared in `lib.rs`) holds everything that is scheduled. The use cases are in the application crates; this module loops, supervises and stops. The module was written as a probe crate from this text, with scripted stand-ins for the use cases, before it was written down (`revision/C-probes/background` in the scratchpad of this revision; toolchain 1.99.1, tokio 1.54.1, the lint tables and `clippy.toml` of the repository): the lint and the complexity command lines of the gates pass, 29 tests under paused time give 100 % of the lines and functions of the module, and cargo-mutants reports 33 mutants, 29 caught, 4 unviable, none missed and none timed out.

```rust
/// How long the shutdown waits for the streams to end after the hub was closed.
pub const STREAM_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy)]
pub struct Schedule { pub renew_interval: Duration, pub prune_interval: Duration, pub retry_delay: Duration,
                      pub round_timeout: Duration }

#[derive(Debug)]
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
impl Background {
    /// Spawns the loops; called inside the runtime.
    #[must_use]
    pub fn spawn(self) -> Running;
}

/// Why the node stops serving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause { Signal(&'static str), TaskFailed(&'static str) }
impl Cause {
    /// `Ok(())` for a signal, `Err(BackgroundTaskFailed { task })` for a loop that died.
    pub const fn result(self) -> Result<(), BackgroundTaskFailed>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("background task {task} failed")]
pub struct BackgroundTaskFailed { pub task: &'static str }
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the round exceeded its time limit")]
pub struct RoundTimedOut;

#[derive(Debug)]
pub struct Running { /* stop: watch::Sender<bool>, tasks: JoinSet<Ended>, elector: Arc<Elector>, hub: Arc<Hub> */ }
impl Running {
    /// Waits for `signal` or for a loop that died, whichever is first (the signal when both are ready),
    /// writes the entry of the cause and returns it.
    pub async fn stopped<S: Future<Output = &'static str> + Send>(&mut self, signal: S) -> Cause;
    /// The shutdown sequence up to the HTTP drain: the loops, the lease, the hub, the streams.
    pub async fn shutdown(self);
}

// private
type Stop = watch::Receiver<bool>;
type Ended = (&'static str, Result<(), JoinError>);       // a loop task: its name and how it ended
/// `Some(output)` of `work`, or `None` when the stop flag is set, or its sender is gone, first.
async fn until_stopped<F: Future + Send>(stop: &mut Stop, work: F) -> Option<F::Output> {
    tokio::select! {
        biased;
        _ = stop.wait_for(|stopping| *stopping) => None,
        output = work => Some(output),
    }
}
/// One round under `limit`: `Ok(outcome) => outcome.map_err(Into::into)`, `Err(_elapsed) => Err(RoundTimedOut.into())`.
async fn bounded<T, E, F>(limit: Duration, round: F) -> Result<T, BoxError>
where F: Future<Output = Result<T, E>> + Send, E: Into<BoxError>;
/// Spawns one named loop: `tasks.spawn(async move { (name, tokio::spawn(work).await) })`.
fn start<F: Future<Output = ()> + Send + 'static>(tasks: &mut JoinSet<Ended>, name: &'static str, work: F);
```

| Loop and task name | Spawned | One round | What the stop flag cancels | Limit of the round | After the round |
| --- | --- | --- | --- | --- | --- |
| `seed` | on every node | `seed.tick()`; the loop ends at the first success | the round | `round_timeout` | after a failure: `catalog seed failed`, then `retry_delay` |
| `listen` | on every node | `listen.step(signal)` | the round | none: the step waits for a notification by design, and logs and sleeps by itself after a failure | the next round at once |
| `relay` | on every node | `relay.tick()` | the round | `round_timeout` | a failure is logged as `relay failed`; then `relay.wait(&wake)`, after `More` only `tokio::task::yield_now()` |
| `elect` | with an event source | `elector.campaign()` | the round of a follower; the renewal of a leader runs to its end | `call_timeout`, inside `campaign` | `renew_interval` |
| `ingest` | with an event source | wait until the status is leader; `ingest.receive()`; `ingest.process(batch)` | the wait and the receive; never `process` | `process`: `round_timeout`; the receive has the operation timeout of its client | after a failure: `ingestion failed`, then `retry_delay`; after a success the next round at once |
| `prune` | with an event source | when the status is leader: `prune.tick()` | the round | `round_timeout` | a failure is logged as `prune failed`; then `prune_interval` |

The six loops, as they are written (each is a private `async fn` that `spawn` hands to `start`):

```rust
async fn seed(seed: SeedEventTypes, schedule: Schedule, mut stop: Stop) {
    while let Some(Err(error)) = until_stopped(&mut stop, bounded(schedule.round_timeout, seed.tick())).await {
        log_seed_failed(&error);
        let pause = tokio::time::sleep(schedule.retry_delay);
        if until_stopped(&mut stop, pause).await.is_none() { return; }
    }
}

async fn listen(listen: ListenForEvents, mut signal: Box<dyn EventSignal>, mut stop: Stop) {
    while until_stopped(&mut stop, listen.step(signal.as_mut())).await.is_some() {}
}

async fn relay(mut relay: RelayEvents, wake: Arc<Notify>, limit: Duration, mut stop: Stop) {
    while let Some(outcome) = until_stopped(&mut stop, bounded(limit, relay.tick())).await {
        if let Err(error) = &outcome { log_relay_failed(error); }
        let more = outcome.ok() == Some(RelayOutcome::More);
        if until_stopped(&mut stop, rest(&relay, &wake, more)).await.is_none() { return; }
    }
}
/// After `More` only a yield, else the wake-up or the poll interval.
async fn rest(relay: &RelayEvents, wake: &Notify, more: bool) {
    if more { tokio::task::yield_now().await; } else { relay.wait(wake).await; }
}

async fn elect(elector: Arc<Elector>, interval: Duration, mut stop: Stop) {
    let status = elector.subscribe();
    while campaigned(&elector, &status, &mut stop).await
        && until_stopped(&mut stop, tokio::time::sleep(interval)).await.is_some() {}
}
/// One campaign; `false` when the stop flag ended it.
async fn campaigned(elector: &Elector, status: &watch::Receiver<ClusterStatus>, stop: &mut Stop) -> bool {
    let leading = status.borrow().role == Role::Leader;
    if leading {
        let _status = elector.campaign().await;
        true
    } else {
        until_stopped(stop, elector.campaign()).await.is_some()
    }
}

async fn ingest(ingest: Arc<IngestEvents>, mut status: watch::Receiver<ClusterStatus>, schedule: Schedule, mut stop: Stop) {
    loop {
        let leading = async { drop(status.wait_for(|status| status.role == Role::Leader).await) };
        if until_stopped(&mut stop, leading).await.is_none() { return; }
        let Some(received) = until_stopped(&mut stop, ingest.receive()).await else { return; };
        if let Err(error) = processed(&ingest, received, schedule.round_timeout).await {
            log_ingestion_failed(&error);
            let pause = tokio::time::sleep(schedule.retry_delay);
            if until_stopped(&mut stop, pause).await.is_none() { return; }
        }
    }
}
/// Phase 2 of a received batch under the round limit; a failed receive is the error of the round.
async fn processed(ingest: &IngestEvents, received: Result<Vec<InboundMessage>, IngestError>, limit: Duration)
    -> Result<(), BoxError> {
    let _report = bounded(limit, ingest.process(received?)).await?;
    Ok(())
}

async fn prune(prune: Arc<PruneEvents>, status: watch::Receiver<ClusterStatus>, schedule: Schedule, mut stop: Stop) {
    while let Some(outcome) = pruned(&prune, &status, schedule.round_timeout, &mut stop).await {
        if let Err(error) = outcome { log_prune_failed(&error); }
        let pause = tokio::time::sleep(schedule.prune_interval);
        if until_stopped(&mut stop, pause).await.is_none() { return; }
    }
}
/// One prune round when the node is leader, `Some(Ok(0))` when it is not; `None` when the stop flag ended it.
async fn pruned(prune: &PruneEvents, status: &watch::Receiver<ClusterStatus>, limit: Duration, stop: &mut Stop)
    -> Option<Result<u64, BoxError>>;
```

Each of the four `log_.._failed(error: &BoxError)` functions is one `tracing::error!` with the field `error` set to `shared_domain::error_chain(error.as_ref())`; a log macro inside a loop body would raise its cognitive complexity above the limit.

`Background::spawn`: one `watch::channel(false)` for the stop flag and one `JoinSet`; `start` for `seed`, `listen` and `relay`; then, only when `ingest` is `Some`, one `elector.subscribe()` and `start` for `elect`, `ingest` and `prune`. `Running` keeps the sender, the set, the elector and the hub.

Rules of the loops:

- **The stop flag is read in one place.** `until_stopped` is the only code that looks at the flag, as the first arm of a `tokio::select!`, which cargo-mutants does not mutate. There is no `while running(&stop)` condition and no pause helper that returns a `bool`: both have a mutant (`replace .. -> bool with true`) that leaves a loop without an await, the test then never ends, and the gate counts a timed-out mutant as surviving. Measured on the probe: with a helper `paused(..) -> bool` that mutant ran into the 120 s limit; with the pause written in the loop no mutant times out. A `Running` that is dropped ends the loops the same way, because the sender of the flag is gone.
- **Every round has an await that no mutant can remove.** The waits of a round are written in the loop body as `until_stopped(&mut stop, <future>).await`: the round itself for `seed`, `listen`, `relay` and the receive of `ingest`, and every pause. A helper that only decides something (`campaigned`, `pruned`, `processed`, `rest`) may be replaced by a constant by a mutant, and the loop still awaits in the same round.
- **What the stop flag cancels.** Every round and every pause, with two exceptions. `ingest.process` runs to its end, so a batch is either appended and acknowledged or not acknowledged at all (FR-24), and is cut only by `round_timeout`. The renewal of a leader runs to its end, at most `call_timeout`: a renewal that was cancelled on the node can still reach the database after the release of `resign`, and the lease would then stay taken for its whole time (measured by the validation: 8 of 348 such stops). A follower's campaign is cancelled; `resign` makes no store call for a follower, and if the cancelled statement took the lease on the database, the lease ends by its expiry, as after a crash ("Election Algorithm"). Each cancelled call is safe to cancel: `relay.tick` awaits only its log reads and stores its cursor after every page, each prune statement commits on its own, the seed is one idempotent statement, and `receive` leaves an unacknowledged batch to its visibility timeout.
- **A round cannot hang.** A statement in flight on a connection that went silent never returns: sqlx bounds only the pool acquire on the client, and `statement_timeout` is enforced by a server that can no longer answer (run by the validation through a proxy that went silent: the statement was still pending after 30 s). `bounded` cuts the seed, relay and prune rounds and `process` after `round_timeout` ("Adapters"), and the campaign is cut by its `call_timeout`. A cut round is a failed round: it writes the entry of its loop with the cause `the round exceeded its time limit`, and the next round runs on another connection. The cut is as safe as a cancellation by the stop flag; `process` cut before its commit acknowledges nothing, and cut after it the batch is delivered again and its ids are found stored (FR-17). The connection of the cut statement stays checked out until TCP gives up, which is what the spare connection of the control pool is for.
- **Leader gating is here and nowhere else.** The `ingest` and `prune` loops read `watch::Receiver<ClusterStatus>` from `elector.subscribe()` and compare `role` with `Role::Leader`. No closure, port or type of the cluster context enters an `event-stream` crate. A receive that began while the node was leader is processed also when the node is follower by then: that is the "one more batch" of "Failure Modes".
- **Who campaigns.** The `elect`, `ingest` and `prune` loops exist only on a node with an event source ("Election Algorithm"). A node without one makes no lease call and keeps the initial status.
- **A failed round is not a failed loop.** A loop never returns an error: a failed or cut round is logged with its error chain and the next round follows after the pause of the table.

Supervision. A loop task can still end by a panic of a dependency, and a node whose relay or elector is gone would keep its clients and deliver nothing: the health report has no probe for it, and `GET /api/cluster` returns the last published status, so nothing would show it. Such a node must not stay in service:

- `Running::failed(&mut self) -> &'static str` (private) is `while let Some(joined) = self.tasks.join_next().await { if let Ok((name, Err(_panic))) = joined { return name; } }` followed by `std::future::pending().await`. A loop that returns is passed over (the seed loop after its first success, and every loop after the stop flag); with no task left it never completes. It is safe to cancel, because `JoinSet::join_next` is. Each loop is spawned by `start` as an inner task that an outer task awaits, so the outer task cannot panic and reports the name of the loop.
- `Running::stopped(signal)` is `tokio::select! { biased; name = signal => Cause::Signal(name), task = self.failed() => Cause::TaskFailed(task) }`, then one log entry: `shutdown signal received` (info, `signal`), the entry `serve` writes today, or `background task failed` (error, `task`), written when the loop dies and not at shutdown.
- A failed loop is followed by the same shutdown sequence as a signal, and `serve` then returns `Err(BackgroundTaskFailed)`: the existing error path writes `serve failed` and the exit status is 1. The clients of the node get the end of their stream or close code 1001 and reconnect, the lease is released, and the orchestrator starts the node again (`restart: unless-stopped` in the compose files).

> Assumption: a dead loop ends the node; it is not reported by a readiness probe and not restarted in place. The PRD keeps the health report at its three probes (NFR-15), a failed probe would take the node out of the load balancer but would leave its open streams on a node that delivers nothing, and only the end of the process makes these clients reconnect. Restarting the loop inside the process would hide a defect: the lint policy leaves no panicking path in the code of the service, so a panic comes from a dependency in a state nobody has analysed.
>
> Assumption: a hung round is cut and retried, and does not end the node. During a database outage every node sees failing or hanging rounds, and NFR-11 wants the open streams to stay; a cut round costs one log entry and one connection until TCP gives up.
>
> Assumption: no deadline is set on the transport of an accepted connection. An SSE client that stops reading keeps its connection, its slot and up to 408 KiB of buffered frames until the edge proxy or TCP ends it ("Failure Modes"); the service already relies on the proxy for the header-read time and the number of connections (`docs/architecture.md`, operational assumptions). The socket option that would end such a connection (`TCP_USER_TIMEOUT` through `socket2`) exists only on some targets and applies to every connection of the node, so a value short enough to protect a shutdown would also cut every client whose network pauses for a few seconds.
>
> Assumption: `bootstrap::application` and the `Background` and `Running` types are the public seam for tests: an integration or end-to-end test builds an `Application`, replaces adapters before that, and either calls the one-round use cases directly or spawns the loops. `bootstrap::router` keeps its signature and the Gherkin world builds through `Adapters::connect`; the existing tests that construct `Adapters` by hand or pin a contract change as listed in "Test Strategy".

`serve` (`command/serve.rs`) stays one straight path. The process runs only as a spawned binary in the tests, where no loop can be made to panic, so `serve.rs` gets no branch that only a failed loop reaches; every such branch is in `background.rs`.

```rust
async fn serve(config: &Config) -> Result<Shutdown, BoxError> {
    let signals = Signals::install()?;
    let adapters = Adapters::connect(config).await?;
    let application = bootstrap::application(adapters, config)?;
    let listener = bind(config).await?;
    let addr = listener.local_addr()?;
    log_listening(addr, mode(config.app_env));
    let (stop, stopping) = watch::channel(false);
    let server = axum::serve(listener, application.router).with_graceful_shutdown(stopped(stopping));
    let server = tokio::spawn(async move { server.await });
    let mut running = application.background.spawn();
    let cause = running.stopped(signals.received()).await;
    stop.send_replace(true);
    match tokio::time::timeout(config.shutdown_grace_period, drain(running, server)).await {
        Ok(joined) => {
            joined??;
            cause.result()?;
            log_stopped();
            Ok(Shutdown::Drained)
        }
        Err(_elapsed) => {
            log_forced(config.shutdown_grace_period);
            Ok(Shutdown::Forced)
        }
    }
}

/// The shutdown sequence after the listener closed: the loops, the lease, the hub and the streams, then the HTTP drain.
async fn drain(running: Running, server: JoinHandle<std::io::Result<()>>) -> Result<std::io::Result<()>, JoinError> {
    running.shutdown().await;
    server.await
}
```

`log_signal` leaves `serve.rs`; the entry is written by `Running::stopped`, with the same message, level and field. The order, and why:

1. **The loops are spawned after `log_listening` and after `bind`.** `listening` stays the first entry of the process, which existing tests read as `lines[0]`, and a start that fails to bind writes exactly one entry, `serve failed`.
2. **`running.stopped(..)`** returns at the signal or when a loop died.
3. **`stop.send_replace(true)`, at once, as today.** The listener closes, so new connections are refused within the 10 ms that `docs/operations.md` promises. This only stops the accept loop and asks idle connections to close (`axum-0.8.9/src/serve/mod.rs`, the graceful shutdown): a response that is being written, an SSE stream included, goes on, and an upgraded connection is a detached task that the stop flag does not reach.
4. **`running.shutdown()`**, inside the one timeout of `SHUTDOWN_GRACE_PERIOD` (FR-24):
   1. set the stop flag of the loops and join them: `while self.tasks.join_next().await.is_some() {}`. Every loop ends at its next await; a batch in `process` and the renewal of a leader end first, each inside its own limit;
   2. `elector.resign()`: one `release`, bounded by `call_timeout`, when the node is leader; no store call when it is follower, so a node that never reached its database stops at once;
   3. `hub.close()`: every SSE response ends after the frames its queue holds, every WebSocket pump writes its queued frames and close code 1001; a subscription request from now on gets the 503 problem `the node is shutting down`;
   4. `tokio::time::timeout(STREAM_DRAIN_TIMEOUT, self.hub.drained())`, either outcome returns. `drained` completes when the last `Subscriber` was dropped, which is when every SSE response has ended and every pump has finished its close handshake.
5. **`server.await`**: the HTTP drain as today, then `stopped`, or `grace period elapsed, forcing shutdown` and exit status 1.

Why step 4.4 exists and why it has a limit of its own. The drain of `axum::serve` waits for connection tasks only; an upgraded WebSocket runs in a task of its own (`axum-0.8.9/src/extract/ws.rs:359-376`), and `start` drops the runtime as soon as `serve` returns. Without the wait, close code 1001 depends on a race with that drop (run by the validation: with 2 000 open sockets up to 105 clients got no close frame; with the wait all of them did, in 6 of 6 runs). The limit is 2 s: a client that answers the close frame is gone within milliseconds, one that stays silent costs `CLOSE_TIMEOUT` (1 s) of the pump, and a client that takes no data at all is ended only by the write deadline `STREAM_KEEP_ALIVE` (15 s by default), which the shutdown does not wait for. Such a client is cut off when the runtime is dropped and resumes with its last id. Because the wait is bounded by its own constant, one stalled WebSocket costs a shutdown 2 s and leaves the exit status 0; a stalled SSE response keeps its connection open and is the open connection of step 5 (exit status 1 at the end of the grace period, as for any request that does not finish).

Exit statuses: 0 after a signal when everything ended inside the grace period; 1 when the grace period ended first, when a loop died (`background task failed`, then `serve failed`) and for a bind or runtime failure as today; 2 for an invalid configuration. The entries `stopped` and `grace period elapsed, forcing shutdown` are unchanged.

> Assumption: FR-24 names the order "loops, lease, hub, drain". The listener is closed before the loops are stopped, as today, so that a deployment does not keep routing new connections to a node that is leaving; the order of FR-24 governs what happens to the work that is already there. `docs/operations.md`, "Shutdown sequence", gets the steps above.
>
> Assumption: a panic of a loop task during the shutdown itself is not turned into exit status 1. The panic hook of `telemetry.rs` has written its entry, the node is already leaving, and the cause of the stop stays what it was.

Log entries (NFR-15). The message texts are fixed, because the tests read them; no entry holds event data, an event type name taken from a request, or the value of a variable.

| Message | Level | Fields | Written by |
| --- | --- | --- | --- |
| `leadership changed` | info | `node_id`, `role`, `leader`, `term` | `Elector` |
| `lease store failed` | warn | `error` (chain) | `Elector` |
| `subscriber evicted` | warn | `transport` (`websocket`), `types` (count), `last_sequence` | `socket::serve`, when a write misses its deadline |
| `subscribers fell behind` | warn | `count` | `RelayEvents::tick`, once per page with at least one |
| `subscription rejected` | warn | `reason` (`the node is starting`, `the node is at its subscriber capacity`, `the node is shutting down`) | `From<HubError> for Problem` |
| `duplicate events skipped` | info | `count` | `IngestEvents::process`, when the count is above zero |
| `inbound message rejected` | warn | `message_id`, `reason` | `IngestEvents` |
| `event signal failed` | warn | `error` (chain) | `ListenForEvents` |
| `ingestion failed`, `relay failed`, `prune failed`, `catalog seed failed` | error | `error` (chain), once per failed or cut round | the loops of `background.rs` |
| `background task failed` | error | `task` (`seed`, `listen`, `relay`, `elect`, `ingest` or `prune`) | `Running::stopped`, when the loop dies |
| `shutdown signal received` | info | `signal` | `Running::stopped` (the existing entry, written here now) |

The `stream` handler writes no entry. A healthy node without `EVENT_QUEUE_URL` writes none of these entries, and a node with a queue writes `leadership changed` once when it takes the lease: the loops are silent while they work. The error chain of a failed round holds no URL and no credential: a pool that cannot connect reports `pool timed out while waiting for an open connection`, and an existing test of the served binary, `the_database_url_and_credentials_are_never_logged`, runs long enough to see the first `relay failed` and `event signal failed` entries of a database with a password in its URL.

Files of the template sync that this change touches and that are not yet in `.templatesyncignore` (read in the template and checked with the command of `docs/extension-seams.md`): `crates/service/src/lib.rs` (the declaration of `background`), `crates/service/src/command/serve.rs`, `crates/service/src/http/router.rs`, `crates/service/tests/integration/main.rs`, `crates/service/tests/integration/support.rs`, `crates/service/tests/integration/check_config.rs`, `docker-compose.yml`, `docker-compose.override.yml`, `docker-compose.load_test.override.yml`, `docker-compose.prod.yml`, `docs/architecture.md`, `docs/operations.md`, `docs/load-testing.md`, `docs/README.md`, `SECURITY.md`. Each goes into `.templatesyncignore` as a full path in the story that first changes it; without the entry the next sync resolves the difference in favour of the template and removes the change. The same rule holds for any other synchronised file a story turns out to touch. New files (`background.rs`, `http/origin.rs`, the feature files, the scenario script, the queue script) are not synchronised.

## Data Model and Queries

Three reversible pairs, one per table, each created with `make.create_migration` in the story that first needs its table, in this order: `name=create_notification_event_types`, `name=create_cluster_leases`, `name=create_notification_events`. The timestamp prefix is the one the command writes, so the order of the versions is the order of creation; no table refers to another, and the order carries no other meaning. The existing pair `20261110000000_create_example_api_resources` is not touched. The SQL below is the whole content of each file; the files are shown by bounded context, not in the order of their versions.

`<timestamp>_create_notification_event_types.up.sql`:

```sql
CREATE TABLE notification_event_types (
    name TEXT COLLATE "C" NOT NULL,
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT notification_event_types_pkey PRIMARY KEY (name),
    CONSTRAINT notification_event_types_name_check
        CHECK (char_length(name) BETWEEN 1 AND 128
            AND name ~ '^[a-z][a-z0-9]*([._-][a-z0-9]+)*$')
);
```

`<timestamp>_create_notification_event_types.down.sql`:

```sql
DROP TABLE notification_event_types;
```

`<timestamp>_create_notification_events.up.sql`:

```sql
CREATE TABLE notification_events (
    sequence BIGINT GENERATED ALWAYS AS IDENTITY,
    id UUID NOT NULL,
    event_type TEXT COLLATE "C" NOT NULL,
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

CREATE INDEX notification_events_recorded_at_sequence_idx
    ON notification_events (recorded_at, sequence);
```

`<timestamp>_create_notification_events.down.sql`:

```sql
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

Run on a scratch PostgreSQL 16 database: the three pairs applied, reverted and applied again give the same catalog (columns, constraints, indexes, the identity sequence), and the catalog after the revert is empty.

Decisions in the tables:

- `name` and `event_type` are declared `COLLATE "C"`. An event type name is ASCII, and its order must be the byte order everywhere: it is the order of the catalog (FR-1), of `EventType: Ord` and of the in-memory fake. With the collation of the database the order would depend on the server: the names `order-paid`, `order.paid`, `order.z`, `order_paid`, `ordera`, `orderpaid` come back in this order with `"C"` and as `order_paid`, `order-paid`, `order.paid`, `order.z`, `ordera`, `orderpaid` with `"en-US-x-icu"` (run). The local image sorts by byte whatever the collation says, so the migration test asserts the catalog entry (`collation_name` is `C`) and not only an order. The check constraints and the primary key are unchanged by the collation; Q9 reads the page by an index-only scan of the primary key.
- `notification_events` has four indexes, each with one user: the primary key (`sequence`) serves the relay read, the latest sequence and the window of a replay page; `notification_events_id_key` is the idempotency of the append; `notification_events_event_type_sequence_idx` serves a replay page whose types are rare in its window; `notification_events_recorded_at_sequence_idx` serves pruning, which reads the oldest rows in the order `(recorded_at, sequence)`. The second column makes that order total, so the rows of one batch, which share `recorded_at`, are always locked in the same order.

The migration set and the existing tests that pin it. `shared_infrastructure::persistence::MIGRATOR` embeds every file of `migrations/`, so the first new pair is seen by every test that runs the migrator. Four existing tests assert the number of migrations or the tables of the whole schema. They are changed once, in the story that adds the first pair, to forms that do not depend on how many pairs exist; the stories that add the second and the third pair do not touch them. No applied migration is edited.

| Existing test | What pins the set today | Change |
| --- | --- | --- |
| `crates/company-subdomain/some-module/infrastructure/tests/integration/migration.rs`, `the_migration_creates_the_table_with_named_constraints` | `COLUMNS` and `CONSTRAINTS` read every table of the schema except `_sqlx_migrations` and are compared with the three columns and the three constraints of the example table | the two catalog queries are filtered to the example table: `table_name = 'example_api_resources'` and `conrelid = 'example_api_resources'::regclass`; the assertions stay |
| same file, `the_embedded_migrator_holds_one_reversible_migration` | `_sqlx_migrations` holds 1 row and `MIGRATOR` holds 1 down migration | replaced by `the_embedded_migrator_holds_the_example_migration_as_a_reversible_pair`, a plain `#[test]`: the entries of `MIGRATOR.iter()` with version `20261110000000` are one up and one down migration, in this order (`Migration: Ord` compares the version, then the type, and `ReversibleUp` sorts before `ReversibleDown`, `sqlx-core-0.9.0/src/migrate/migration.rs:33-38`) |
| `crates/service/tests/integration/migrate.rs`, `migrate_applies_once_and_changes_nothing_the_second_time` | the output `applied 1 migration(s)` | the expected count is the number of up migrations of `MIGRATOR`, read by a helper `versions() -> Vec<i64>` of the test file |
| same file, `revert_removes_the_table_and_migrate_creates_it_again` | `VERSION = "20261110000000"`; one `migrate revert` removes the example table and the second prints `nothing to revert` | renamed `revert_removes_one_migration_at_a_time_and_migrate_applies_them_again`: the first revert prints `reverted <newest version>` and leaves the older versions in `_sqlx_migrations`; one further revert per older version prints that version, newest first; after them the example table is gone and one more revert prints `nothing to revert`; `migrate` then prints `applied <count> migration(s)`. The constant `VERSION` is removed; `TABLE_EXISTS` keeps reading `example_api_resources`, and the test names neither the newest migration nor its table, so a later pair needs no edit |

Tests that run the migrator and need no change:

- `crates/shared/infrastructure/tests/integration/persistence.rs`, `migrations_apply_revert_and_apply_again_to_the_same_catalog`, stays byte for byte. It is already independent of the set, and it is the test that proves every new pair reversible: it expects an empty catalog after `MIGRATOR.undo(&pool, 0)`, which fails for a down file that leaves anything behind and for a pair without a down file.
- The other tests of the three files above (the constraint test of the example table, `revert_on_a_database_that_was_never_migrated_has_nothing_to_revert`, the tests against an unreachable database) and `tests/e2e/world.rs` do not look at the set.
- `crates/service/tests/integration/serve.rs` asserts that `serve` creates neither `example_api_resources` nor `_sqlx_migrations`; that stays true.
- `tests/CLI/bats/make_database.bats`, which is protected, asserts `applied 0 migration(s)` for a second `make migrate` and `applied 1 migration(s)` for a `make migrate` after one `make migrate-rollback`. Both hold for any number of pairs, because `migrate revert` reverts exactly the newest applied pair.

The migration tests of the two new infrastructure crates read the catalog with a filter on their own tables, so they do not pin the set either. Both changed files are already listed in `.templatesyncignore` (`crates/company-subdomain/` and `crates/service/tests/integration/migrate.rs`).

> Assumption: each table has its own pair. A story adds the table its code reads and no other, an applied migration is never edited, and a revert removes one table. The two tables of `notification/event-stream` therefore come from two pairs, created by two stories.

How time crosses the sqlx boundary: it does not cross as a time type.

- `occurred_at` is bound and read as microseconds since the epoch (`i64`) and converted in SQL, without loss in either direction:
  - write (Q2): `'epoch'::timestamptz + (m / 3600000000) * interval '1 hour' + (m % 3600000000) * interval '1 microsecond'`. PostgreSQL multiplies an interval by a number in double precision (`bigint * interval` resolves to the operator of `double precision`, function `mul_d_interval`), so the direct form `m * interval '1 microsecond'` is exact only up to 2^53 microseconds, the year 2255: 9007199254740993 came back as 9007199254740992, and 253402300799999999, the last microsecond of the year 9999, as 253402300800000000. Split by hours, both products stay exact up to `i64::MAX`. A split by seconds is not enough: it is exact for the year 9999 and loses 384 microseconds at `i64::MAX`.
  - read (Q4, Q5): `(extract(epoch FROM occurred_at - 'epoch'::timestamptz) * 1000000)::bigint`. `extract` of an interval is `numeric` and exact; the form `extract(epoch FROM occurred_at)` returned 9223372036854775800 for `i64::MAX`.
  - Run with both expressions: 500 000 random values over the whole range of a non-negative `bigint` and the values 0, 1, 3599999999, 3600000000, 2^53 + 1, 253402300799999999, `i64::MAX` - 1 and `i64::MAX` all came back unchanged, with a session time zone that has daylight saving time.
- `recorded_at`, `first_seen_at` and `expires_at` are written by the database (`now()`) and never selected. A duration (lease time, retention) is bound as milliseconds (`i64`).
- The lease statements send no timestamp of a node.

Every query, as it is written in `sqlx::query!` (the text is the key of the `.sqlx` file, so it is used verbatim). The types are what PostgreSQL 16 reported for the prepared statement.

| # | Adapter method | Statement | Parameters | Columns |
| --- | --- | --- | --- | --- |
| Q1 | `PgEventLog::append` | `SELECT pg_advisory_xact_lock($1)` | `bigint` | `void` |
| Q2 | `PgEventLog::append` | see below | `uuid[]`, `text[]`, `text[]`, `bigint[]` | none |
| Q3 | `PgEventLog::append` | `SELECT pg_notify('notification_events', '')` | none | `void` |
| Q4 | `PgEventLog::read_after` | `SELECT sequence, id, event_type, data, (extract(epoch FROM occurred_at - 'epoch'::timestamptz) * 1000000)::bigint AS "occurred_at_micros!" FROM notification_events WHERE sequence > $1 ORDER BY sequence LIMIT $2` | `bigint`, `bigint` | `bigint`, `uuid`, `text`, `text`, `bigint` |
| Q5 | `PgEventLog::read_between` | see below | `bigint`, `bigint`, `text[]`, `bigint`, `bigint` | as Q4 |
| Q6 | `PgEventLog::latest_sequence` | `SELECT max(sequence) AS "latest?" FROM notification_events` | none | `bigint` |
| Q7 | `PgEventLog::prune`, one batch | see below | `bigint`, `bigint` | none |
| Q8 | `PgEventCatalog::register` | `INSERT INTO notification_event_types (name) SELECT batch.name FROM UNNEST($1::text[]) AS batch (name) ORDER BY batch.name ON CONFLICT (name) DO NOTHING` | `text[]` | none |
| Q9 | `PgEventCatalog::find_page` | `SELECT name FROM notification_event_types ORDER BY name LIMIT $1 OFFSET $2` | `bigint`, `bigint` | `text` |
| Q10 | `PgLeaseStore::acquire_or_renew` | see "Election Algorithm" | `text`, `uuid`, `bigint` | `uuid`, `bigint` |
| Q11 | `PgLeaseStore::acquire_or_renew`, when Q10 returned no row | `SELECT holder, term FROM cluster_leases WHERE name = $1` | `text` | `uuid`, `bigint` |
| Q12 | `PgLeaseStore::release` | `UPDATE cluster_leases SET expires_at = now() WHERE name = $1 AND holder = $2 AND expires_at > now()` | `text`, `uuid` | none |

Q2:

```sql
INSERT INTO notification_events (id, event_type, data, occurred_at)
SELECT batch.id, batch.event_type, batch.data,
       'epoch'::timestamptz
           + (batch.occurred_at_micros / 3600000000) * interval '1 hour'
           + (batch.occurred_at_micros % 3600000000) * interval '1 microsecond'
FROM UNNEST($1::uuid[], $2::text[], $3::text[], $4::bigint[])
     WITH ORDINALITY AS batch (id, event_type, data, occurred_at_micros, position)
ORDER BY batch.position
ON CONFLICT (id) DO NOTHING
```

Q5, parameters `(after, up_to, type names, limit.rows, limit.bytes)`:

```sql
SELECT page.sequence AS "sequence!", page.id AS "id!", page.event_type AS "event_type!",
       page.data AS "data!", page.occurred_at_micros AS "occurred_at_micros!"
FROM (
    SELECT sequence, id, event_type, data,
           (extract(epoch FROM occurred_at - 'epoch'::timestamptz) * 1000000)::bigint AS occurred_at_micros,
           sum(octet_length(data)) OVER (ORDER BY sequence) - octet_length(data) AS bytes_before
    FROM notification_events
    WHERE sequence > $1 AND sequence <= $2 AND event_type = ANY($3::text[])
    ORDER BY sequence
    LIMIT $4
) AS page
WHERE page.bytes_before < $5
ORDER BY page.sequence
```

Q7, parameters `(retention in milliseconds, batch)`:

```sql
DELETE FROM notification_events
WHERE sequence = ANY(ARRAY(
    SELECT sequence FROM notification_events
    WHERE recorded_at < now() - ($1::bigint * interval '1 millisecond')
    ORDER BY recorded_at, sequence
    LIMIT $2
    FOR UPDATE))
```

`PgEventLog::append` runs Q1, Q2 and Q3 in one transaction (`pool.begin()`, the three statements on `&mut *tx`, `commit()`); an empty slice returns 0 without a transaction. The result is `rows_affected()` of Q2. Measured: a batch with the same id twice inserts one row, and a repeated id consumes a sequence value, so sequences have gaps and no code may assume that `n + 1` follows `n`.

Why the lock: identity values are taken at insert time, not at commit. Without Q1 a transaction could commit sequence `n` after a reader already read past `n + 1`. With the lock taken first, appenders are serial, PostgreSQL makes a commit visible before it releases the transaction's locks, and a reader that pages by `sequence > cursor` never skips a row. The lock wait counts against `statement_timeout` (5 s), so a blocked appender fails instead of hanging.

Bounds of the reads. No read grows with the size of the log:

- Q4 (relay) walks the primary key from the cursor and stops after `$2` rows.
- Q6 is one step from the end of the primary key.
- Q5 (replay) reads only rows with `after < sequence <= up_to`. The subscription keeps that window at `Subscription::REPLAY_WINDOW` = 10 000 sequences, so one page reads at most 10 000 rows whatever the types are and however the planner decides. Without the upper bound the same statement read every row between the cursor and the first match: for a type whose rows are the newest of a table of 2 million, 1 835 800 rows were read and discarded to return 500 (460 ms, growing with the log). With the window, measured on that table with a row limit of 500 and a byte limit of 1 MiB:

| Window of 10 000 sequences | Plan PostgreSQL chose | Time |
| --- | --- | --- |
| one type, no row of it in the window | bitmap scan of `notification_events_event_type_sequence_idx` | 0.13 ms |
| 100 types, none in the window | index scan of `notification_events_event_type_sequence_idx` | 0.40 ms |
| one type with 5 % of the rows | bitmap scan of the same index, then a sort | 3.3 ms |
| one type with every row | bitmap scan of the same index, then a sort of 10 000 rows | 12.3 ms |
| the same four cases with the generic plan of a prepared statement | range scan of the primary key with the type filter | 1.3 ms, 10.2 ms, about 2 ms, 0.6 ms |

- Both plans are bounded by the window, so the bound does not rest on statistics. The index on `(event_type, sequence)` stays: it makes a page with few matches ten times cheaper or more, which is the common shape of a long replay.
- The byte limit of Q5 is applied in the statement: `bytes_before` is the data size of the rows before a row, and a row is returned while that sum is below `$5`. The row that reaches the limit is included, so a page is never empty when a row matches and never exceeds `limit.bytes` by more than one event. This is the cut of `PageLimit::reached_by`. Run: 200-byte rows with a limit of 500 bytes gave 3 rows, with a limit of 600 bytes 3 rows and with a limit of 1 byte 1 row; 60 000-byte rows with a limit of 1 MiB gave 18 rows (1 080 000 bytes) out of 40 candidates; a row limit of 2 below the byte limit gave 2 rows.
- Q9 reads `offset + limit` entries of the catalog, which holds one row per event type ever ingested or configured; no client request adds a row to it.

Pruning (FR-12). `PgEventLog::prune` runs Q7 as described in "Bounded Context notification/event-stream", infrastructure crate: a statement deletes the oldest expired rows, at most `batch` of them, and commits; the call repeats it until one deletes nothing.

- Why not one `DELETE ... WHERE recorded_at < ..`: its cost is the size of the backlog. 850 000 expired rows of 200 bytes took 1.15 s in one statement, which puts the 5 s of `DATABASE_STATEMENT_TIMEOUT` at about 3.7 million such rows and at fewer larger ones. A backlog above that fails, rolls back and is larger at the next round, for ever.
- Plan of Q7, the same with a custom and with a generic plan: an index scan of `notification_events_recorded_at_sequence_idx` under `LIMIT`, row locks in that order, then a primary-key scan for `sequence = ANY(..)`. The form `sequence IN (SELECT .. LIMIT $2)` is not used: with a generic plan PostgreSQL joined it by a hash join over a sequential scan of the whole table (957 ms on 3 000 000 rows for a batch of 1 000).
- When nothing is expired, the statement reads the start of that index and deletes nothing (0.07 ms).
- The measured times and the two default bounds are in the infrastructure section of the context.

Order and locks. Every statement that writes more than one row does it in a stated order, and no transaction holds row locks of two tables:

| Statement | Order of its rows and locks |
| --- | --- |
| Q2 | the order of the batch (`ORDER BY batch.position`), which becomes the sequence order; appenders are serial through the advisory lock of Q1, taken first |
| Q7 | `FOR UPDATE` locks the rows in the order `(recorded_at, sequence)` before any is deleted, and the delete finds them locked. Two nodes that prune at once (a leader that was paused past its lease, and the new one) lock in the same order: the second waits for the first, then skips the rows that are gone. Run: three sessions pruning the same backlog for 8 s beside three appenders completed 6 642 prune statements and 4 636 append transactions with no deadlock and no error |
| Q8 | name order (`ORDER BY batch.name`). Two sessions that insert the same new names wait for each other in one direction only. Run: the same 100 new names in opposite array order, 150 rounds per session: 37 and 29 statements failed with a deadlock without the `ORDER BY`, none with it. The statement has no `DISTINCT`: a name that is twice in the array is inserted once and the second is skipped by `ON CONFLICT DO NOTHING` (run with `b.x, a.x, b.x, a.x`: two rows, no error) |
| Q10, Q12 | one row |

An append never waits for a pruner except on one row: when it brings an id whose row is being deleted, it waits for that delete to commit and then inserts. A pruner never waits for an appender, because it locks only rows that were committed before its statement began. There is no cycle between the two.

After the three stories that add queries, `make.sqlx_prepare` has written twelve new files into `.sqlx/`: Q8 and Q9 with the catalog, Q10 to Q12 with the lease, Q1 to Q7 with the log.

## Wire Protocols

The normative text for clients is `docs/streaming-protocol.md` (new, FR-27); this section is its source. `docs/requests.http` gets one example per new route.

### Subscription request (both streaming routes)

- `types`: comma-separated event type names; required on `/api/events/stream`, optional on `/api/events/ws`; 1 to 100 distinct names, a repeated name counts once (0 allowed on the WebSocket route when the parameter is absent).
- Cursor: header `Last-Event-ID`, or query parameter `lastEventId`; the header wins; decimal `u64`. A cursor above the latest sequence of the log is treated as the latest sequence.
- Answers before a stream opens:

  | Status | When |
  | --- | --- |
  | 400 | `types` missing on the SSE route; a cursor that is not a `u64` |
  | 422 | a violation on `types` (the table of `TypesError`) |
  | 503, detail `the node is starting` | the relay of this node has not read its start position yet (the first moments after a start, or as long as the node cannot reach the database after it) |
  | 503, detail `the node is at its subscriber capacity` | `MAX_SUBSCRIBERS` connections are open |
  | 503, detail `the node is shutting down` | after the hub was closed |
  | 4xx of the upgrade | on the WebSocket route, a request that is not an upgrade (426 without upgrade state) |

- No answer depends on a database read. A subscription with a cursor is accepted while the log cannot be read, and its stream then ends: the SSE response ends without a frame, the WebSocket is closed with code 1011.
- A 503 is safe to retry on any node. A browser `EventSource` does not retry it by itself: the HTML standard fails the connection for every status but 200 and then never reconnects, so the page must create a new `EventSource` ("Falling behind and reconnecting" below).

### SSE frames

Response head: `200`, `content-type: text/event-stream`, `cache-control: no-cache`, `x-accel-buffering: no`. One frame per event:

```text
id: 1042
event: user.created
data: {"userId":"5f2c"}

```

- `id` is the sequence in decimal, `event` the event type, `data` the event data.
- Data with line breaks is written as one `data:` line per line. Before encoding, `\r\n` is replaced by `\n`, so a client reads the text with every line break as `\n` (a lone `\r` also arrives as `\n`; the SSE format has no way to carry `\r`). The WebSocket frame carries the text unchanged.
- Keep-alive: the comment frame `:` followed by an empty line, when no frame was written for `STREAM_KEEP_ALIVE`.
- Frames of one stream have strictly increasing ids, also when the connection fell behind and caught up from the log.
- The stream ends without a final frame when the node shuts down or when the log cannot be read during a replay. A stream is never ended because the client is slow; a client that reads nothing at all loses its connection ("Failure Modes").
- A proxy in front of the service must not buffer this response and must keep it open longer than `STREAM_KEEP_ALIVE`; the header `x-accel-buffering: no` tells nginx so.

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
| `types` empty (`[]`) or more than 100 distinct names | `types must hold 1 to 100 event type names` |
| a name that is not an event type, an empty name included | `invalid event type name` |
| the set would exceed 100 types | `a connection subscribes to at most 100 event types` |
| a binary frame | `binary frames are not supported` |
| a text frame above 16 384 bytes | `the command is too large` |

The checks of a text frame run in this order: size, JSON and shape (`Json::<Command>::from_bytes`), names (`parse_names`), the limit of the set. A message or a single frame above 32 768 bytes is not answered: the connection ends without a close frame.

Other rules:

- **Run-time subscribe.** It adds the types from the position the node has relayed to when it handles the command: the connection gets the events of an added type with an id above that position and above its own position (the id up to which the connection is settled; it is the higher of the two only while the relay of this node lags behind what the connection has already read from the log or received from another node), and none at or below either, also not later when the connection catches up from the log. It does not replay; replay belongs to the request (FR-11). Subscribing to a type that is already in the set changes nothing, its position included.
- **Unsubscribe.** No later frame of the removed types is written, also not one that was already queued or read from the log.
- **When commands are read.** A command is handled between two log reads of the connection and never while one is in flight. While the connection replays (after a request with a cursor, or after it fell behind) a command is read while the events of a page are written: it waits for the read in flight (one wait for a read permit plus one statement) and for the reads that follow it directly because they found no event to write. It then holds for everything that is not written yet: after an unsubscribe in the middle of a replay, the events of the removed type that were already read from the log are not written.
- **Ping.** The server sends a ping every `STREAM_KEEP_ALIVE`, also during a replay, and the library answers a client ping with a pong when the server next reads.
- **Write deadline.** A message that the socket does not accept within `STREAM_KEEP_ALIVE` ends the connection without a close frame.
- **Close.** Close codes sent by the server: 1001 at shutdown, after the frames that were already queued; 1011 when the log cannot be read during a replay. After its close frame the server waits up to 1 s for the client's close frame. A close frame from the client is answered with a close frame, and the subscription ends.
- **Order.** Frames of one connection have strictly increasing ids.

> Assumption: close code 1011 (internal error, RFC 6455) is used when the log cannot be read during a replay; the PRD fixes only 1001 for shutdown. Close code 1013 is not used: a subscriber that falls behind is not disconnected (the assumption under the hub's rules). A valid command gets no acknowledgement frame, because FR-9 and FR-10 define an answer only for an invalid one.

### Falling behind and reconnecting

What a client sees, and what `docs/streaming-protocol.md` must say:

- **Falling behind.** A connection that is more than `SUBSCRIBER_BUFFER` events behind is not closed. The node writes the frames it had queued, reads what the connection missed from the log by its position and continues with live delivery: no loss, no duplicate, the same order. The client sees a delay and nothing else.
- **Ended stream.** After an SSE stream ended or a WebSocket was closed with 1001 or 1011, or was lost without a close frame, the client reconnects to any node and passes the id of the last frame it received. A browser `EventSource` does this by itself for an ended stream or a network error and sends `Last-Event-ID`. A WebSocket client reconnects itself and passes `lastEventId`.
- **Refused request.** After an answer other than 200 or 101 a browser `EventSource` is failed for good (HTML standard, server-sent events: a status that is not 200 fails the connection, and a failed connection is not reconnected). The page must create a new `EventSource`, and because a new object has no `Last-Event-ID` it passes the last id as the query parameter `lastEventId`. 400 and 422 are faults of the request and are not retried; 503 is retried.
- **Back-off.** Every retry waits, with a delay that grows and has a random part, so that the clients of a lost node do not arrive together.

### SQS envelope

CloudEvents in binary style: the attributes are the metadata, the body is the data.

| Part | Rule |
| --- | --- |
| attribute `event-type` (String) | required, an event type name |
| attribute `event-id` (String) | optional UUID; default: the SQS `MessageId`, which a redelivery keeps. The id is what makes ingestion idempotent |
| attribute `event-time` (String) | optional RFC 3339 time in UTC with the suffix `Z`, from `1970-01-01T00:00:00Z` to `9999-12-31T23:59:59Z` with any number of fractional digits (`humantime::parse_rfc3339`); stored to the microsecond; default: `Clock::now()` at receipt |
| body | the event data: 1 to `EVENT_MAX_DATA_BYTES` bytes of UTF-8 without U+0000 |

How the adapter reads the queue, one `ReceiveMessage` per round:

| Request member | Value |
| --- | --- |
| `MaxNumberOfMessages` | 10 |
| `WaitTimeSeconds` | 5 (FR-15) |
| `VisibilityTimeout` | 30, the constant `VISIBILITY_TIMEOUT`; it replaces the visibility timeout of the queue for the messages of this receive (run on LocalStack 3.4.0: a queue created with 300 s returned an unacknowledged message, with the same `MessageId`, about 2 s after a receive that asked for 1 s) |
| `MessageAttributeNames` | `event-type`, `event-id`, `event-time`; an attribute that is not asked for is not returned |

A message is removed with `DeleteMessageBatch` after its event is stored or after it was rejected and logged. A message that is not removed returns after the visibility timeout; a queue with a redrive policy moves it to its dead-letter queue after the configured number of receives, which is a setting of the queue and not of the service.

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

A subscription reads the log by position, and the queue of the hub is its fast path. Its state:

- `floor`: the position of the connection. Every event at or below it is settled: written, or skipped for good.
- `target`: `None` when a replay must start; otherwise `Some(t)`: the subscription replays while `floor < t` and is live when `floor >= t`.
- `types`: every subscribed type with its "since" cursor. An event is wanted when its type is in the map and its sequence is above the since of that type.
- `pending`: the events of the last replay page that are not handed out yet.

`Subscriptions::open(request)`, synchronous:

1. `hub.register(&request.types)`. `Closed`, `NotReady` and `AtCapacity` end here (503). The subscriber carries `position`, the position of the hub at registration.
2. Every requested type enters `types` with the since `Cursor::START`.
3. Without a cursor: `floor = position` and `target = Some(position)`, so the subscription is live from the position of the node. With a cursor `c`: `floor = c` and `target = None`.

`Subscription::step()`:

1. **Pending**, when `pending` holds events: take events from the front until one is wanted; encode it with `Frame::encode` and return `Frame`. An event that is not wanted is dropped (its type was unsubscribed, or it lies at or below the since of its type). When `pending` runs empty without a wanted event, return `Idle`. This step awaits nothing and never goes on to a read: a step that took events from `pending` does not read the log.
2. **Start of a replay**, when `target` is `None`:
   1. acquire a permit, held until the step returns;
   2. when the hub is closed, return `End(HubClosed)`;
   3. `latest = log.latest_sequence()`; an error returns `End(LogFailed)`;
   4. `t = latest`, or `Cursor::START` for an empty log; `floor = t` when `types` is empty, else `floor = min(floor, t)`; `target = Some(t)`; return `Idle`.
3. **Page**, when `floor < target`:
   1. acquire a permit, held until the step returns;
   2. when the hub is closed, return `End(HubClosed)`;
   3. `up_to = min(floor.advanced_by(REPLAY_WINDOW), target)`; `page = log.read_between(floor, up_to, <the keys of types>, REPLAY_PAGE)`; an error returns `End(LogFailed)`;
   4. when `REPLAY_PAGE.reached_by(&page)`, `floor` becomes the sequence of the last event of the page; otherwise `floor = up_to`; the page is appended to `pending`; return `Idle`.
4. **Live**, when `floor >= target`: `subscriber.recv().await`.
   - A frame whose sequence is not above `floor`: return `Idle` (a replay covered it, or the client had it).
   - Any other frame: `floor` becomes its sequence; return `Frame` when it is wanted, else `Idle`.
   - `None`: `subscriber.resume()`. `Err`: return `End(HubClosed)`. `Ok`: the subscriber fell behind and has a new queue; `target = None`; return `Idle`, and the next step starts a replay from the floor.

`next()` calls `step` until it returns `Frame` (the frame) or `End` (`None`). `replaying()` is `pending.is_empty() && target.is_none_or(|t| floor < t)`: it is true exactly when the next step is step 2 or step 3, the two steps that read the log. While it is false the next step is step 1, which awaits nothing, or step 4, which awaits only the queue; both are safe to race against an inbound frame.

Notes on the steps:

- The `min` of step 2 is "a cursor above the latest sequence is treated as the latest sequence" (FR-11): without it a cursor of `u64::MAX` would suppress every later event. With no type there is nothing to replay, so the floor moves to the target at once; a later run-time subscribe starts at the hub position, which is not below it.
- A permit is `permits.acquire().await.ok()`: the semaphore is never closed, so there is no error arm. The check of step 2.2 and 3.2 comes after the permit, so that at shutdown the subscriptions that wait for a permit end one after the other without a read.
- A page that may have been cut continues after its last event; a page that was not cut settles its whole window, also when it is empty. That is the only place where the row and byte limits are judged.
- Steps 2, 3 and 4 await (the permit and the log, or the queue) before they return `Idle`. Step 1 returns `Idle` without awaiting only after it emptied `pending`, so the step after it is one of the other three. `next` therefore cannot spin on `Idle` without giving the runtime a turn. With fakes that answer at once, a wrong loop ends through the call budget of the log fake.
- Commands of a WebSocket arrive between steps, also in the middle of a replay ("The WebSocket pump": an inbound frame is read in every turn that does not read the log). The steps do not depend on when: an unsubscribe removes the type from `types`, so step 1 drops its pending events and step 3 no longer asks for it; a subscribe adds the type with its since, so step 1 and step 4 drop its events at or below the since and step 3 asks for it from the next page on. What a subscribe in the middle of a replay cannot bring back are the events of the new type at or below the floor of that moment: the pages up to the floor were read without the type, and a queued frame at or below the floor is dropped. The connection therefore gets an added type above its since and above its floor at the subscribe, which is what "Wire Protocols" states.
- The pump reads no inbound frame between two log reads that follow each other without an event to write: the read of the latest sequence and the first page after it, and every page that returns no event. A command waits for such a run of reads and is read as soon as a page with events is being written or the connection is live. No frame is written during such a run, so an unsubscribe still holds for every frame that follows it.
- After `End` the transport drops the subscription; `step` is not called again.

Why there is no gap and no duplicate. Let `s` be the sequence of an event the connection wants. Call "arming" the moment, under the registry lock, from which the hub queues for the subscriber: the registration, or a `resume`. Let `L` be the latest sequence that step 2 reads after an arming, and `R` the start cursor of the node's relay.

- The hub registers nobody before the relay has `R`, and from then on the relay publishes every event above `R`, in sequence order, each after it read it from the log ("Relay Algorithm"). `R <= L`, because `R` was read first.
- `s <= L`: `s` was committed before `L` was read, and every page is read after that. The pages cover `(floor, L]` without a hole, so the replay returns `s` when `s` is above the floor, once.
- `s > L`: `s` was committed after `L` was read, so after the arming, and `s > R`, so the relay publishes it after the arming. The frame is queued and written in the live phase. If the queue was full, the subscriber fell behind before `s`: the queue ends, the floor is the last frame taken from it, which is below `s`, and the replay that follows the next arming covers `s`, because `s` was published, hence committed, before that replay reads its own `L`.
- No duplicate: a replay returns only events above the floor and moves the floor to the end of what it settled; a queued frame is written only when it is above the floor, and it then becomes the floor. The ids of a connection therefore increase strictly.
- A relay that lags behind the client's cursor (the client got `s` from another node) publishes `s <= c` later; the floor drops it.
- A type that was added at run time has the hub position of that moment as its since. The hub queues its frames only from then on, all of them above the since, and a later replay skips the events at or below it. A run-time subscribe therefore never replays (FR-9), whatever happens to the connection afterwards.

This algorithm was run as code before it was written down: with a queue of 1, 2 and 64 frames, a window of 3 sequences and pages of 2 rows, every interleaving of 7 moves (append, one relay read, one client read) with the request placed at each of the 8 positions, 17 496 schedules per queue size, delivered every event above the cursor once and in order, or was refused with `NotReady` (probe `revision/A-probes/hubprobe` in the scratchpad of this revision, test `interleavings_deliver_every_event_above_the_cursor_once_and_in_order`).

Bounds (NFR-9), for one connection and for the node:

- **Memory of a replay.** `pending` holds at most one page: at most 500 events and at most 1 MiB of data plus the one event that reaches the budget. Events are encoded one at a time when they are handed out, so a replaying connection never holds a page in both encodings.
- **Database work of a connection.** One read is one statement over at most `REPLAY_WINDOW` = 10 000 sequences (1.2 to 1.9 ms on a local PostgreSQL 16 with 200-byte rows, with or without matching rows; 0.7 ms for 18 rows of 60 000 bytes cut from 500 candidates), or one index read for the latest sequence. The next read happens only when the transport asked for the next frame and `pending` is empty: a slow client reads the log slowly, and a client that reads nothing causes no read at all.
- **Database work of the node.** At most `replay_permits` reads of all subscriptions run at the same time; the others wait in arrival order. Replays therefore hold at most that many connections of the pool they read from and keep at most that many pages inside sqlx. A subscription waits for its permit inside its stream, where the SSE keep-alive and the WebSocket ping continue, never inside a request.
- **What keeps replays from starving the node.** Three things, the first two wired in the composition root. (1) The elector, the relay, ingestion, pruning and the seed use a connection pool of their own; the log adapter that `Subscriptions` holds and the catalog adapter of `ListEventTypesHandler` use the request pool, so a client-driven read never stands in front of a lease renewal or a relay round. (2) `replay_permits` is `max(1, DATABASE_MAX_CONNECTIONS / 2)`, so the catalog query always finds a connection in the request pool. (3) `open` reads nothing, so a storm of reconnects is accepted at once and served in order. Measured by the security review on the template's pool of 10 with a separate control pool of 3 and 4 permits: 30 000 clients that reconnect within one second are all served, the campaigns answer within 10 ms and the slowest relay round takes 12 ms; without the permits 26 391 of the 30 000 fail (`probes/security-resilience-refute/remedy/e-split-permits4-storm30000.out` and `d-split-storm30000.out`).
- **Falling behind again.** If more than `SUBSCRIBER_BUFFER` events of its types arrive while a subscription replays, its queue ends again; after the replay reached its target the subscription takes what the queue holds, drops what is not above the floor and starts the next replay from there. Each round starts at a higher floor and costs one read of the latest sequence plus its pages.
- **Shutdown.** A closed hub ends a replay before its next read (steps 2.2 and 3.2) and a live subscription after the frames its queue holds.

`subscribe(types)`: when `types.len()` of the subscription plus the number of names that are not yet in it exceeds 100, return `TooManyTypes` and change nothing. Otherwise `since = subscriber.subscribe(&types)`, and every name that is not yet in the map enters it with `since`; a name that is already there keeps its since. `unsubscribe(types)`: the names leave the hub entry and the map.

Tests that prove this section (unit tests of `http/subscription.rs` with a real `Hub`, real channels and `InMemoryEventLog` with a call budget, under paused time; each awaited call inside `tokio::time::timeout`; every test but the one for `NotReady` starts the hub in its arrangement with `EventPublisher::started(&*hub, cursor)`):

- `open` reads nothing (the fake counts zero calls); `NotReady`, `AtCapacity` and `Closed` are returned and leave no subscriber.
- Replay, then live: every event above the cursor once and in order; each case of the proof (an event at or below `L`, one above `L` that is queued, one above `L` that arrives after the queue ended, a frame at or below the cursor from a lagging relay); cursor 0; a cursor above the latest sequence is clamped and a later event arrives; an empty log.
- Pages: a page of exactly `rows` events and a page that reaches `bytes` continue after their last event; a short page settles its window; a type whose events all lie beyond several windows is found; the number of log calls is asserted in each case.
- No types: a cursor without types reads the latest sequence and no page; after a run-time subscribe events arrive.
- Falling behind: a relay round with more events than the queue holds reaches a reading subscription completely, in order, without `End`; the subscriber count stays 1; a second burst during the replay is also covered. When every frame left in the ended queue is unwanted (its type was unsubscribed), the replay still starts after the last of them: the log fake records `read_between` with `after` equal to that sequence.
- Permits: with one permit and a gated read of one subscription, the step of a second subscription makes no log call until the first read returned; when the hub is closed while it waits, it returns `End(HubClosed)` and the fake has recorded no call of it.
- Run-time subscribe: an event at the hub position of the subscribe and one below it are not written, one above it is, also when the connection replays afterwards from a floor below that position. A subscribe in the middle of a replay writes no stored event of the type at or below that position and writes the next one. With the hub position below the floor (a lagging relay), an event of the added type between the two is not written.
- Unsubscribe: a pending event and a queued frame of the removed type are not written. An unsubscribe followed by a subscribe of the same type writes no stored and no queued event of it at or below the position of the subscribe.
- Step 1: a step that drops the last pending events returns `Idle` and makes no log call (the fake counts); the step after it reads.
- `replaying()`: true after `open` with a cursor, between two pages of a replay and after falling behind; false while events of a page are pending and when live, also after a live frame moved the floor above the target.
- Ends: a failing `latest_sequence` and a failing page give `End(LogFailed)`; a closed hub gives `End(HubClosed)` before the next read (the gated fake sees no call) and, when live, after the queued frames. With a cursor and a log that returns full pages: one frame is read, the hub is closed, then exactly the rest of the first page is written and the stream ends with `HubClosed`, with one `read_between` call recorded.
- The limit of 100 types: 100 is accepted, the 101st is refused and changes nothing, a repeated name does not count.

## Relay Algorithm

Wake-up: the listen loop runs `ListenForEvents::step`: `signal.changed()` returns after a `NOTIFY` on `notification_events`, and the step calls `wake.notify_one()` (a stored permit, so a wake-up between two waits is not lost). On an error (the listener cannot be created or cannot reconnect, for example while the database is down) the step logs and sleeps `retry_delay`, and the next call builds a new listener. Every completion of `changed` is one `notify_one`; `Notify` keeps at most one permit, so any number of notifications between two relay rounds starts one round. The relay loop waits with `RelayEvents::wait`: `tokio::time::timeout(poll_interval, wake.notified())`, either outcome returns. A lost notification therefore costs at most `EVENT_RELAY_POLL_INTERVAL` (FR-18).

`RelayEvents::tick()`:

1. Start cursor. When `cursor` is `None`: `latest = log.latest_sequence()?`; `cursor = Some(latest or Cursor::START)`; then `publisher.started(cursor)`, once, before the first page is read. A node thus starts at the latest sequence of the moment its first round succeeds; older events reach a client only through replay. Until this read succeeds (database down at start) nothing is relayed, the hub answers `NotReady` to every subscription request, and the next round tries again.
2. Repeat at most `MAX_PAGES_PER_TICK` times (a `for` over a range, never an open loop):
   - `page = log.read_after(cursor, PAGE_SIZE)?` (Q4);
   - for every event in order: `behind += publisher.publish(&event)`;
   - when the page is not empty, the cursor becomes the sequence of its last event and is stored;
   - when `behind` of this page is above zero, log `subscribers fell behind` at warn with `count = behind`;
   - when the page is shorter than `PAGE_SIZE`, return `Drained`.
3. Return `More`; the loop then runs the next round at once.

Properties:

- One reader task per node, so every subscriber gets its frames in sequence order.
- `publish` cannot fail and a read fails only between two pages, so an error leaves the cursor at the last published event and the next round continues without loss and without repeat.
- The round makes one database read per page for all subscribers of the node, and none per subscriber (NFR-3).
- The relay never waits for a subscriber and never slows down for one (NFR-10). A round may carry more events of a type than a queue holds (a backlog after a failed round, the poll fallback, a burst): the subscribers whose queues are full fall behind and catch up from the log on their connection; nobody is disconnected.
- The start of the relay is the start of the node for clients. A subscription that was accepted before the relay knew its start cursor could miss the events committed between its own reads and that cursor, and a subscription without a cursor would have no position to start from. The hub therefore accepts nobody before `started`, and every subscription begins at or after the relay's start cursor (the proof in "Subscription Algorithm").

> Assumption: the start-up gap is closed by refusing subscriptions until the relay has started (503 `the node is starting`), not by accepting a subscription with a cursor and letting its stream wait for the relay. Both close the gap (each was run over every interleaving of 7 moves by the validation, 0 wrong schedules). The refusal is chosen because one rule then covers a subscription with and without a cursor, the second kind gets a defined start position, and no stream waits in a state that only `close` or the relay can end. Its cost: a browser `EventSource` that reconnects to a node in this state is failed by the 503 and must be created again by the page ("Wire Protocols"); at an ordinary start the state lasts for one database read, and after a start without the database for at most `EVENT_RELAY_POLL_INTERVAL` after the database is back.
>
> Assumption: falling behind is logged by the relay, once per page with a count (`subscribers fell behind`), and not once per subscriber. A round larger than the queue makes every subscriber of a type fall behind in the same page, so an entry per subscriber would write up to `MAX_SUBSCRIBERS` entries per burst. The entry `subscriber evicted` is kept for the one case in which the service still ends a connection because of its client: the write deadline of the WebSocket pump.

`Hub` as `EventPublisher`. `started(cursor)` is `Hub::start`: under the lock, the position becomes `cursor` if it was `None`. `publish(event)` is `Hub::deliver(event)` reduced to its `behind` count, 0 for `None`. `deliver`, one lock section:

1. lock the registry; `position = Some(event.sequence)`;
2. look up `by_type[event.event_type]`; when there is no entry, return `None`: nothing is encoded;
3. `frame = Arc::new(Frame::encode(event))`;
4. for every sender of the type, in subscriber-id order: `try_send(Arc::clone(&frame))`; an error puts the id on a local list;
5. every listed subscriber falls behind: its `Entry::sender` becomes `None` and its clones leave `by_type` under each of its types, with the empty maps; its queue ends after the frames it already holds;
6. unlock; return `Some(Delivery { delivered, behind })`.

`Hub::close`, one lock section: `closed = true`; `by_type` is cleared; every `Entry::sender` becomes `None`. The entries stay, so `subscriber_count` and `drained` follow the connections: a live subscription writes the frames its queue holds and then ends with `HubClosed`, a replaying one ends before its next read, and each entry leaves when its transport drops the subscription.

What the composition root awaits at shutdown: `hub.close()`, then `hub.drained()` inside a timeout of its own. When `drained` completes, every SSE body has ended and every WebSocket pump has written its queued frames and close code 1001 and has finished its close handshake, because a `Subscriber` is dropped only then. The timeout bounds the clients that do not take their frames: a WebSocket write that is blocked ends by its deadline of `STREAM_KEEP_ALIVE`, a close handshake without an answer by `CLOSE_TIMEOUT`, a blocked SSE response only by the transport ("Failure Modes"). The wait must end before the HTTP stop is awaited and before the runtime is dropped, because an upgraded connection is a detached task that the drain of `axum::serve` does not wait for (`axum-0.8.9/src/extract/ws.rs:359-376`).

Tests that prove this section:

- `RelayEvents` with `InMemoryEventLog` and a recording publisher: `started` is called exactly once, with the start cursor, after a successful `latest_sequence` and before the first `read_after`, and not at all while that read fails; paging; a backlog of exactly `PAGE_SIZE` events ends with `Drained` after one more, empty read; `More` after `MAX_PAGES_PER_TICK` full pages; an error keeps the cursor; the entry `subscribers fell behind` is written once per page with the sum of the page and not written for a page without it (asserted through `LogRecorder` at 0 and at 2). `RelayEvents::wait` under paused time: without a notification it returns exactly `poll_interval` after its start; with `wake` notified before the call it returns at once; with `wake` notified three times before the call the first `wait` returns at once and the second returns exactly `poll_interval` later, which is the fold of the wake-ups (`Notify` keeps one permit).
- `Hub` with real channels: `register` before `start` answers `NotReady`, and `Closed` wins over it; `start` twice keeps the first cursor; one test starts the hub through the trait (`EventPublisher::started(&*hub, cursor)`, after which `register` succeeds) and reads the behind count through `EventPublisher::publish`, so a mutant that empties either trait method of `Hub` fails a test; `deliver` returns `None` for a type without subscriber and `Some` with exact counts otherwise; a full queue makes exactly that subscriber fall behind, takes it off every type and keeps its entry and its slot (a second `register` at the cap answers `AtCapacity` until the first `Subscriber` is dropped); `resume` attaches the new queue to the types that are left after `subscribe` and `unsubscribe` calls made while behind; a subscriber registered without types stays open, receives after a `subscribe`, and stays open after it unsubscribed its last type; a subscriber that added a type with `subscribe` and then falls behind gets its queued frames and then `None` while `hub_closed()` is false, and it, like one that is dropped after such a `subscribe`, leaves no key of that type (`type_count` is 0); `type_count` returns to its old value after 100 fresh names were subscribed and unsubscribed; `subscribe` returns the position; `close` ends every queue after its frames, the queue of a subscriber without types included, makes `resume` and `register` answer `Closed` and leaves the count unchanged; `drained` completes at once on a hub without subscribers, is pending while one `Subscriber` lives, also after `close` and after that subscriber fell behind, and completes when the last one is dropped; the hand-written `Debug` output of `Hub` and `Subscriber`.
- `socket::serve` with a scripted `Connection` under paused time, every call inside `tokio::time::timeout`: frames out; a `transmit` that never completes ends the task exactly `keep_alive` later, frees the slot and writes `subscriber evicted` with the type count and the position; no `receive` call while a `latest_sequence` or a page read waits behind a gate, with pings written meanwhile and exactly one log call per read afterwards; an unsubscribe during a replay takes effect inside the page: with one page of 50 pending events of one type and an unsubscribe of that type ready in the script from the start, the double records fewer than 50 frames of the type and none after the `receive` call that returned the command (the `select!` picks at random between the two ready arms, and 50 picks in a row against the inbound arm do not happen: the chance is below 10^-20); commands and pings in the live phase; a client close frame is followed by one more `receive`; after `close` the queued frames, then code 1001, then the linger ends at `CLOSE_TIMEOUT` when the peer stays silent and at once when `receive` returns `None`; code 1011 after a failing log; a `receive` error and a failing `transmit` end the task without a close frame.

## Ingestion Algorithm

`IngestEvents::process(batch)`; an empty batch returns at once.

1. `received_at = clock.now()`, once per batch.
2. Keep the receipt of every message. Convert each message with `to_new_event`:
   - `event-type` missing: `MissingType`; not an `EventType`: `Type`;
   - id: the `event-id` attribute, else `message_id`; `shared_domain::Uuid::parse` fails: `Id`;
   - `event-time` present and `humantime::parse_rfc3339` fails: `Time`; absent: `received_at`;
   - `EventData::new(body, max_data_bytes)` fails: `Data`. This is the only place where `EVENT_MAX_DATA_BYTES` is applied; the log reads its rows back without a size limit.
   A rejected message is logged once at warn as `inbound message rejected` with the message id and the reason, never the body or the attribute values (FR-16).
3. When at least one event is valid: `catalog.register(distinct types)`, then `log.append(&events)`. The two calls are two transactions, the catalog first, and neither holds a lock of the other table. An error returns here; nothing is acknowledged and the bus redelivers the batch (FR-17).
4. `source.acknowledge(all receipts)`, valid and rejected alike. An error is returned; the redelivery finds every id stored and appends nothing.

The order register, append, acknowledge keeps two invariants: the catalog is a superset of the types in the log, and a message leaves the bus only after its event is stored.

Redelivery. The adapter receives every message with a visibility timeout of its own, `VISIBILITY_TIMEOUT` = 30 s, whatever the queue is configured with. A message that was received and not acknowledged (step 3 or 4 failed, or the leader stopped between the receive and the acknowledge) is delivered again 30 s after that receive, to whichever node is leader then. Three consequences:

- The redelivery of an event that was appended is recognised by its id as long as its row is in the log. A row stays at least `EVENT_RETENTION`, whose smallest value is 1 minute, and the redelivery comes after 30 s, so one failed acknowledge never produces a second row. Only an acknowledge that keeps failing for longer than `EVENT_RETENTION` while receiving still works can append an event a second time; FR-17 holds "while the row is retained".
- The up to 10 messages that a crashed leader had received are not part of the takeover bound of "Election Algorithm": they return 30 s after their receive, which can be later than the takeover.
- A batch whose `process` takes longer than 30 s becomes visible again before it is acknowledged. The next receive returns it once more; that append finds the ids and adds nothing, and the acknowledge with the newer receipts removes the messages.

Time. `occurred_at` is kept to the microsecond: the log stores whole microseconds since the epoch and drops the digits below. Every time `to_new_event` can produce, from `1970-01-01T00:00:00Z` to `9999-12-31T23:59:59.999999999Z` (the range of `humantime::parse_rfc3339`) and any `Clock::now()`, is stored and read back exactly to the microsecond ("Data Model and Queries").

> Assumption: the visibility timeout is a constant of the adapter and not the queue's setting or a twelfth variable. A queue whose own timeout exceeded `EVENT_RETENTION` (SQS allows 12 hours, the retention may be 1 minute) would defeat the idempotency; 30 s is the SQS default, is below the smallest retention and is far above the time one batch takes, and no deployment needs to tune it.

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
2. Only when no row came back: run Q11 with `fetch_one` as a second statement with a fresh snapshot and return its row. Q10 found a row to conflict with and the row is never deleted, so Q11 always finds it.
3. Both rows go through `restore`; every `sqlx::Error`, `RowNotFound` included, goes through `translate` ("Bounded Context internal/cluster", infrastructure crate).

Measured on PostgreSQL 16: node A gets term 1; A renews, term 1; B gets no row while A's lease is valid and reads `(A, 1)`; after A's `release`, B gets `(B, 2)`; A then gets no row and reads `(B, 2)`; a `release` by A changes no row; B takes its own released lease again with term 2. A lease renewed with a time of zero is taken over by the next statement of another node, with no wait in between (500 of 500 attempts). The row is never deleted, so the term never decreases, and `release` (Q12) only moves `expires_at` to `now()`.

Who campaigns. Only a node that can ingest takes part in the election: the composition root runs the elect loop when `Background.ingest` is `Some`, which is when `EVENT_QUEUE_URL` is set. A node without an event source never calls the lease store. Its status stays the initial one, so `GET /api/cluster` answers `role` `follower`, `leader` `null` and `term` 0 on it for the life of the process, and the loops that act only on `Role::Leader` never act there. The reason: the holder of the lease is the only node that reads the bus (FR-17), and Q10 lets a holder renew for ever. A node without a queue that won the first round would keep every node with a queue a follower, and nothing would be ingested (run: A campaigns first on an empty table and renews; B gets no row in each of twelve later rounds). Two consequences for a deployment: ingestion fails over only between nodes that have `EVENT_QUEUE_URL`, so the takeover of FR-20 and NFR-6 needs the variable on at least two nodes, and with it on one node nobody takes over while that node is down; a cluster in which no live node has it has no leader, ingests nothing and prunes nothing (FR-12).

> Assumption: a node without an event source does not observe the lease either. A read-only round would need a second port method, a thirteenth statement and a second mode of the elector, and its only effect would be that `GET /api/cluster` names the leader on such a node; nothing in the service acts on that value. The operator reads the leader from a node that campaigns. The validation of this design proposed the read-only round, because `leader` `null` with `term` 0 is also what a node reports before its first successful round; that cost is accepted and made explicit instead: FR-21 defines this answer for a node without an event source, `docs/environment.md` says in the row of `EVENT_QUEUE_URL` that such a node never ingests and never leads, and `docs/operations.md` states the two-candidate rule and how to read `leader` `null`.

`Elector::campaign()`, one round:

1. `tokio::time::timeout(settings.call_timeout, store.acquire_or_renew(node, settings.ttl))`.
2. Success with `leadership`: status `{ node_id, role: leadership.role_of(node), leader: Some(leadership.leader), term: Some(leadership.term) }`.
3. Error or timeout: log `lease store failed` at warn with the error chain; status `{ role: Follower, leader: None, term: the last term seen }`. The node steps down at once, which is the safe direction (FR-19).
4. Publish with `watch::Sender::send_replace`; when the role or the leader changed, log `leadership changed` at info with node id, role, leader and term.

`Elector::resign()`: when the published role is leader, `timeout(call_timeout, store.release(node))` (a failure is logged as `lease store failed`; the lease then ends by its expiry); when it is follower, no store call. Then publish follower with no leader. A `campaign` that the shutdown cancelled while the node was follower may have taken the lease on the database without the node knowing; `resign` does not release it, and the lease ends by its expiry like that of a crashed leader.

Timing. `T = CLUSTER_LEASE_TTL`, `R = CLUSTER_RENEW_INTERVAL`. Two facts of the composition root carry the bounds: the elect loop runs one `campaign` and then pauses `R`, and every lease call is cut after `call_timeout = R`. `d` is the time one lease call takes from its start to its answer, `0 <= d <= R`; it is a few milliseconds while the database is healthy.

- **Round period.** Two rounds of one node start `R + d` apart, so between `R` and `2R`. The loop is not a fixed-rate timer.
- **Step-down.** Let `b` be the database time (`now()`) of a leader's last successful renewal; its lease expires at `b + T`. The answer to that renewal reaches the node at most `R` after `b`, the pause adds `R`, and the next call fails or is cut after at most `R`. A leader that loses the lease store is therefore follower no later than `3R` after `b`, and not `2R`: the time the last renewal took to answer counts.
- **Cross-field rule.** `3 * CLUSTER_RENEW_INTERVAL <= CLUSTER_LEASE_TTL`, reported on `CLUSTER_RENEW_INTERVAL` with the reason `must be at most a third of CLUSTER_LEASE_TTL`. With it, `b + 3R <= b + T`: a cut-off leader stops believing in its lease no later than the instant at which another node may take it. Equality is accepted: with a lease of `9s`, `3s` passes and `3001ms` fails; with the default lease of `10s`, `3333ms` passes and `3334ms` fails; the defaults `3s` and `10s` pass. With the former rule `2R < T`, `R = 4s` and `T = 10s` were accepted and the node could believe in its lease for 2 s after it had expired.
- **Takeover after a crash.** The crashed leader's lease expires at most `T` after the crash. Take the last round of a follower whose statement ran before the expiry: its answer arrives within `d`, the pause is `R`, and the next round runs its statement and gets its answer within `d`. A follower therefore holds the lease, and knows it, no later than `T + R + 2d` after the crash: 13 s with the defaults and a healthy database, and at most `T + 3R`, 19 s with the defaults, when every lease call takes its whole budget (FR-20, NFR-6).
- **Takeover after a graceful stop.** `release` expires the lease at once, so the same reasoning without `T` gives `R + 2d`: 3 s with the defaults and a healthy database, at most `3R`. When `release` fails, the bound of a crash applies.
- **No bound without the lease store.** While lease calls fail or time out, every node that campaigns is follower and none takes over; the bounds count from the moment the calls succeed again.
- **Clocks.** Expiry compares `expires_at` with `now()` of the database only; no node clock takes part (NFR-7). The bounds above are durations measured by tokio time on one node, never a comparison of two clocks.

The bounds are for the lease. Messages that the crashed leader had received and not acknowledged come back later, after `VISIBILITY_TIMEOUT` ("Ingestion Algorithm").

> Assumption: the rule is tightened to a third instead of making the loop a fixed-rate timer or deriving the call timeout from `T - 2R`. The rule needs no new mechanism, the design input names "a third" as one of its two readings, and the defaults pass it. A call timeout of `T - 2R` would shrink to a few milliseconds for settings close to the old limit and make a healthy leader step down on every slow answer.

What the lease is for: it keeps the bus consumer single, which keeps the load on the queue and the log low. Correctness does not depend on it: a node that was paused past its expiry can at worst process one more batch, and that append is idempotent by event id and serial through the advisory lock.

## Failure Modes

| Failure | Behaviour |
| --- | --- |
| A node crashes | Its clients lose the connection and reconnect to any node with their last id; the replay returns what they missed, without duplicates (FR-11). No shared state is lost: the node held only connections and a relay cursor. Every reconnect is accepted without a database read; the replays then run `replay_permits` at a time, in arrival order, on the request pool, so the elector and the relay of the receiving node are not delayed. |
| A node starts, or restarts, while it cannot reach the database | Its relay has no start cursor, so the hub answers every subscription request with the 503 problem `the node is starting`; the relay retries each `EVENT_RELAY_POLL_INTERVAL`. After the first successful read the node accepts subscriptions, and none of them can miss an event (FR-18). At an ordinary start the same answer is possible for the few milliseconds of that first read. |
| The leader crashes | Another node that campaigns holds the lease no later than `T + R + 2d` after the crash ("Election Algorithm": 13 s with the defaults and a healthy database, at most `T + 3R` = 19 s). Messages stay on the bus meanwhile. The up to 10 messages that the crashed leader had received and not acknowledged return `VISIBILITY_TIMEOUT` (30 s) after that receive, which can be later than the takeover, and are appended once by the new leader. Delivery of stored events continues on every node. |
| The leader stops gracefully | `resign` expires the lease; another node holds it within `R + 2d` (3 s with the defaults and a healthy database). When `release` fails, the row above applies. |
| The leader cannot reach the lease store | Its next round fails or is cut after `call_timeout`, and it is follower no later than `3R` after the database time of its last renewal; the rule `3R <= T` puts that at or before the expiry of its lease. Its ingest loop finishes the batch in progress and receives no more. |
| A paused leader resumes after its lease expired | It can process at most one more batch before its next round makes it follower; the append is idempotent, so the log is unchanged. |
| A node runs without `EVENT_QUEUE_URL` | It never campaigns, so it cannot hold the lease and keep the nodes with a queue from ingesting. It serves the catalog, subscriptions and the relay; `GET /api/cluster` reports `follower`, `leader` `null`, `term` 0 on it. When only one node has a queue, nobody takes over while that node is down; when no live node has one, no node leads and nothing is ingested or pruned. |
| Database outage | Every election round fails or times out, so every node is follower, the leader no later than `3R` after its last renewal, and ingestion stops with the messages left on the bus. Open streams stay open and get keep-alives. The relay round fails, and `changed` fails as soon as the listener cannot reconnect; both are logged once per round and retried; the catalog answers 500. A node whose relay had started keeps accepting subscriptions: one without a cursor stays open; one with a cursor is accepted and its stream ends when its first log read fails (SSE: end of the response, WebSocket: close code 1011), after which the client retries. A replay that is running when the outage begins ends the same way. After the outage a leader is elected, the listener reconnects and each relay continues from its cursor (NFR-11). |
| Listener connection lost | `PgListener` reconnects and listens again inside the pending `changed` call, which does not fail (run: after `pg_terminate_backend` on the listener session the call stayed pending and the next commit woke it). `changed` fails only when that reconnect fails; `ListenForEvents` then logs `event signal failed`, waits `RETRY_DELAY`, and the next call builds a new listener. Notifications sent while no session listens are lost; the relay reads by cursor after `EVENT_RELAY_POLL_INTERVAL` at the latest, so no event is lost. |
| A proxy that does not pass notifications (transaction pooling) | No notification arrives and `changed` stays pending; every relay round is started by the poll interval. Delivery is slower by up to `EVENT_RELAY_POLL_INTERVAL` and loses nothing. |
| Queue outage or missing queue | `receive` fails; the ingest loop logs `ingestion failed`, waits `RETRY_DELAY` and tries again; everything else keeps working. The node keeps the lease, so ingestion waits until this node reaches the queue again; the lease follows the lease store, not the queue. |
| Slow client (it reads, but slower than its events arrive) | Its queue fills; the next `try_send` takes it off live delivery, and the relay logs `subscribers fell behind`. The connection stays: it writes the frames it had queued, then reads what it missed from the log, one bounded page at a time and only as fast as the client takes frames, and is live again when it has caught up. Other subscribers, the relay and ingestion never wait (FR-22, NFR-10). |
| A relay round with more events of a type than `SUBSCRIBER_BUFFER` (a backlog after a failed round or after the poll fallback, a burst) | Every subscriber of that type falls behind at once, however fast its client is, and catches up from the log on its connection: one read of the latest sequence and its pages each, `replay_permits` at a time. Nobody is disconnected and nothing is lost or repeated. With 10 000 subscribers of the type that is 20 000 or more short reads on the request pool, in arrival order; the control pool and the relay are not involved. |
| Client that stops reading (WebSocket) | The write that the socket does not accept ends by its deadline of `STREAM_KEEP_ALIVE`; the pump logs `subscriber evicted` and drops the socket, which closes the connection and frees the slot. Until then the connection holds its slot, at most `SUBSCRIBER_BUFFER` frame references, at most one replay page and one frame in the write buffer. |
| Client that stops reading (SSE) | hyper stops asking the response body for frames once about 408 KiB are buffered (`hyper-1.12.0/src/proto/h1/io.rs:23` and `dispatch.rs:384`), so nothing in the application runs for this connection any more and nothing in the application can end it: it keeps its slot, its queued frame references and its replay page until the connection ends. The service sets no deadline on the transport ("Composition Root", assumption): the connection ends when the edge proxy applies its send timeout, which is a deployment prerequisite stated in `docs/operations.md`, or when TCP gives up on a client that is gone. Run by the security review: without a bound a stalled SSE response was still open after 20 s. At a shutdown such a connection is the one that is still open when the grace period ends, so the exit status is 1, as for any request that does not finish. |
| A WebSocket message or frame above 32 768 bytes | The frame is refused when its header is read, before its payload is buffered; `receive` returns the error and the pump ends the connection without a close frame. A text frame between 16 385 and 32 768 bytes is answered with the error frame `the command is too large` and the connection stays open (FR-10). |
| A client keeps sending frames during its replay | No log read is interrupted or repeated: an inbound frame is not read while a log read of the connection is in flight, so each read costs one statement, however many frames arrive. The frames are handled between the reads, while the events of a page are written, and when the connection is live. |
| Subscriber cap reached | The request is answered with the 503 problem; no stream, no upgrade. The cap counts connections: a slot is free again when the connection of its subscriber has ended, not when the subscriber falls behind (FR-23). |
| Shutdown with open streams | `hub.close()` ends every queue after the frames it holds. Each SSE response ends, each WebSocket gets its queued frames, close code 1001 and up to `CLOSE_TIMEOUT` for the client's close frame. The composition root awaits `hub.drained()` for at most `STREAM_DRAIN_TIMEOUT` (2 s), so these writes happen before the runtime is dropped (FR-24). A client that does not take its frames within the bound is cut off and resumes with its last id; a stalled WebSocket therefore costs a shutdown 2 s and leaves the exit status 0. |
| The bus delivers a message twice | `ON CONFLICT (id) DO NOTHING`: no new row, no second delivery; the message is acknowledged (FR-17). The skipped insert uses up one sequence value, which is a gap and never a reuse. This holds while the first row is retained: a redelivery comes 30 s after the receive, and the shortest `EVENT_RETENTION` is 1 minute. |
| Invalid message (type, id, time) or oversized, empty or NUL-holding body | Logged once at warn without the body, acknowledged, not appended; the valid messages of the batch are processed (FR-16). |
| Acknowledge fails after the append | The error is logged as `ingestion failed`; the bus redelivers the messages 30 s after their receive; that append finds the ids stored, and the next acknowledge removes the messages. |
| A backlog of rows older than `EVENT_RETENTION` (the retention was lowered, or no node led for a long time) | One `prune` call deletes the oldest expired rows in statements of at most 200 rows that each commit on their own and stay far below `DATABASE_STATEMENT_TIMEOUT`, at most 5 000 statements and 1 000 000 rows per call. A statement that fails loses only its own 200 rows of progress and is logged as `prune failed`. The backlog shrinks by up to a million rows per `PRUNE_INTERVAL`, which is more than a leader appends in that time, until it is gone ("Data Model and Queries", pruning). |
| `EVENT_MAX_DATA_BYTES` is lowered while larger events are stored | Nothing fails: the limit is applied at ingestion only, and rows are read back without a size limit. The stored events are relayed and replayed until retention removes them. |
| Two nodes register the same new event types at once (seed at start, or two leaders during an overlap) | Q8 inserts in name order, so the two statements wait for each other in one direction only and both succeed; with the array order of the callers they deadlocked. |
| Cursor older than the retention window | The replay starts at the oldest retained event; the loss is not signalled (FR-11 assumption of the PRD). |
| A loop task ends by a panic | The node does not stay in service: `background task failed` (error, with the name of the loop) is written when the task dies, the shutdown sequence runs (the streams end or get close code 1001, the lease is released), `serve failed` is written and the exit status is 1; the orchestrator starts the node again and its clients resume on any node with their last id. Without this the node would keep its clients and deliver nothing, and neither the health report nor `GET /api/cluster`, which returns the last published status, would show it. |
| A round of a loop does not return (a statement in flight on a connection that went silent) | The seed, relay and prune rounds and `ingest.process` are cut after `round_timeout` (41 s with the defaults), a campaign after `call_timeout`. The cut round writes the entry of its loop (`relay failed` and the others) with the cause `the round exceeded its time limit`, and the next round runs on another connection. The connection of the cut statement stays checked out of the control pool until TCP gives up; the pool has one connection to spare for it. A leader whose `process` hangs keeps its lease, so ingestion pauses for at most the limit and no other node takes over. |
| The node is stopped while the database or the queue does not answer | The stop flag ends every round and every pause at once: a pool that is still waiting for a connection, a long poll, a campaign of a follower. `resign` makes no store call for a follower. An idle node therefore exits with status 0 within milliseconds, also with every dependency down, as before this change. |
| The node is stopped while a batch is in `process` or while the leader renews | Both end first: the batch is appended and acknowledged or not acknowledged at all, and the release is sent after the answer to the renewal. Each is bounded (`round_timeout`, `call_timeout`); when that takes longer than the grace period, the next row applies and the lease ends by its expiry. |
| The grace period ends during shutdown | As today: `serve` logs `grace period elapsed, forcing shutdown` and exits with status 1. What is still running (a connection that did not finish, a loop inside an uncancelled step) is dropped with the runtime within `FORCED_EXIT_LIMIT`. |
| Client reads flood the node (a reconnect storm, many catalog requests) | They use the request pool only, and replays at most half of it. The elector, the relay, ingestion and pruning have the control pool and the listener its own connection, so the leader keeps its lease and live delivery continues; the requests that find no connection within `DATABASE_ACQUIRE_TIMEOUT` are answered with the 500 problem, and replays wait for their permit inside their stream. |
| A browser page of an origin that is not in `CORS_ALLOWED_ORIGINS` (every other origin while the variable is unset) | An HTTP request is served without `access-control-allow-origin`, and the browser hides the answer from the page; a preflight is not approved. A WebSocket handshake is answered with the 403 problem before any subscriber is registered. Clients that send no `Origin` are not affected. |
| An invalid variable at start (an origin with a path, a renew interval above a third of the lease time, an empty `EVENT_QUEUE_URL`) | `serve` and `check-config` exit with status 2 and name the variable and the rule, never the value; nothing is bound and no loop starts. |
| `EVENT_QUEUE_URL` is unset in a deployment that should ingest, or names a queue that the region of `AWS_REGION` does not have | Unset: no node campaigns and nothing is ingested; the catalog, the subscriptions and the relay work, and nothing is logged, so `docs/operations.md` names `GET /api/cluster` (`leader` `null` on every node) as the check. Wrong queue or region: `ingestion failed` every `RETRY_DELAY` on the leader while the health report stays green, because the probe lists queues and does not read this one. The production compose reference passes the variable through from its env file and never inherits the local queue. |

## Performance Design

- **Publish cost follows the interested subscribers.** `Hub::deliver` looks up one key in `by_type` and visits only the senders under it. Subscribers of other types and idle connections are not touched (NFR-2). An event whose type has no subscriber on the node is not even encoded.
- **One encode per event and transport on the live path.** `Frame::encode` builds the SSE `Event` and the JSON text once; all subscribers share the `Arc<Frame>`. A WebSocket subscriber clones `Utf8Bytes` (a reference count). An SSE subscriber clones the built `Event` (one copy of the encoded bytes into its own write buffer; `axum` has no public constructor for a shared, finished `Event`). No live subscriber encodes, escapes or splits anything, which is how NFR-2 is read here. A replayed event is encoded for its one connection when it is handed out, in both forms; the form the transport does not use is the price of one `Frame` type for both paths.
- **Bounded memory per connection (NFR-9).** What one connection can hold, whatever its client does:

  | What | Bound |
  | --- | --- |
  | queue of the hub | `SUBSCRIBER_BUFFER` frame references; the frames are shared with the other subscribers of the type. The queue is not refilled while the subscriber is behind |
  | type set | 100 names of at most 128 characters, in the hub entry and in the subscription |
  | replay page | 500 events and 1 MiB of data plus one event; no page while the connection is live |
  | WebSocket, inbound | a read buffer of 4 096 bytes, one frame and one message of at most 32 768 bytes |
  | WebSocket, outbound | one frame at a time, because every write is awaited and has a deadline. The write buffer keeps the capacity of the largest frame it ever held (measured by the validation: 65 546 bytes kept after one frame of 65 536 bytes), so the bound is the encoded size of the largest event sent on the connection, at most that of an event of `EVENT_MAX_DATA_BYTES` |
  | SSE, outbound | the write buffer of hyper, about 408 KiB plus one frame, when the client does not read |
  | idle connection, measured by the security review | about 6 KiB for a WebSocket with the 4 096-byte read buffer (134 KiB with the default), about 20 KiB for an SSE stream |

- **Bounded memory per node.** At most `MAX_SUBSCRIBERS` connections hold the buffers above, because a slot is counted from the registration until the `Subscriber` is dropped. At most `replay_permits` replay pages are inside sqlx at one time. The relay holds one page of at most `PAGE_SIZE` events at a time. `by_type` holds only types that have a queue.
- **No lock across an await.** The registry is a `std::sync::Mutex`; `register`, `start`, `deliver`, `resume`, `subscribe`, `unsubscribe`, the removal in `Drop` and `close` are synchronous and hold it for a bounded number of map operations and `try_send` calls, and `deliver` for the encoding of one event. `drained` awaits a `watch` channel outside the lock.
- **No database read on the live path.** A node reads the log once per relay round, whatever the number of clients; a client reads only during its own replay (NFR-3). A replay follows a request with a cursor and a fall behind; it reads in windows of 10 000 sequences and pages of 500 events or 1 MiB, pulled by the socket, and at most `replay_permits` of them read at one time.
- **A burst does not disconnect anybody.** A relay round larger than a queue makes the subscribers of that type fall behind; they stay connected and catch up from the log. The cost is the log reads of "Failure Modes", on the request pool only.
- **Latency.** Append, commit, `NOTIFY`, one indexed range read, fan-out: no timer is on the path when the listener works. The poll interval is only the fallback.
- **Ingestion.** One transaction per batch of at most 10 messages, serial through the advisory lock; throughput is that of one leader (NFR-12).

> Assumption: `MAX_SUBSCRIBERS` is sized by memory. With the figures above, 100 000 idle connections take about 0.6 GiB as WebSockets and about 2 GiB as SSE streams, before any queued frame or replay page; `docs/operations.md` gives this rule, and the memory limit of the production compose reference must follow the chosen value.
>
> Assumption: the registry is one mutex. At 10 000 subscribers of one type a fan-out holds it for roughly the time of 10 000 `try_send` calls, during which registrations wait. Sharding the registry by type is a later optimisation that changes no interface; NFR-1 is checked with the load profiles before it is considered.

## Contract Impact

- **OpenAPI** (`.github/openapi-spec/spec.yaml`, regenerated with `make.generate_openapi_spec`): two new paths, `GET /api/event-types` (`api_event_types_get`, tag `EventStream`; 200 with an array of `EventType`, 400, 422, `default`) and `GET /api/cluster` (`api_cluster_get`, tag `Cluster`; 200 with `ClusterStatus`, `default`); new schemas `EventType`, `ClusterStatus`, `ClusterRole`; two new tags. `API_VERSION` in `crates/service/src/openapi.rs` becomes `1.1.0`, because operations are added (the rule in its doc comment). Spectral runs with `--fail-severity=hint`, so every operation and schema property carries a description and an example like the existing ones.
- **GraphQL** (`.github/graphql-spec/spec`, regenerated with `make.generate_graphql_spec`): the type `EventType { name: String! }` and the query field `eventTypes(itemsPerPage: Int! = 30, page: Int! = 1): [EventType!]!`.
- **Not in the OpenAPI document:** `GET /api/events/stream` and `GET /api/events/ws`. OpenAPI 3.1 cannot describe a WebSocket, and `make.schemathesis_validate` calls every documented operation and would wait on a response that never ends. Both routes are specified in `docs/streaming-protocol.md`.
- `make.openapi_diff` and `make.graphql_diff` report additions only; no existing operation, schema or status changes (NFR-20).
- **Cross-origin headers are not part of a contract document.** With `CORS_ALLOWED_ORIGINS` unset the answers of every route are byte for byte what they are today, `OPTIONS` included, and that is the configuration of every gate: the compose files set no value, so `make.schemathesis_validate` (`--checks all`) and the load-test stack run without the CORS layer. With a value set, the layer answers every `OPTIONS` request with 200 and an empty body and adds headers to the other answers; no status of a documented operation changes.
- **Tests that pin the contracts today** change with the contract they pin, in the story that adds the operation: the list of operations, the version, the tags and the schemas in `crates/service/src/openapi/tests.rs`, and the expected SDL in `crates/service/src/graphql/schema/tests.rs` ("Test Strategy", existing tests).

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

The revision after the validation adds no element to the model: the two extra connection pools, the loops, their supervision and the two origin layers are inside `compositionRoot`, and the feature `cors` belongs to a crate that is already a dependency. `xtask`, `test-support` and `e2e-tests` are outside the model (`scripts/validate-architecture-model.sh` leaves them out), so the new `[dependencies]` entries of `tests/support` (`event-stream-domain`, `cluster-domain`) and the new dev-dependencies of the layer crates need no relationship.

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
5. **Takeover bound and renew rule.** The input gives two readings of the renew rule and "within the lease time". With the "half" reading a cut-off leader could believe in its lease for up to one renew interval after it had expired, because the time its last renewal took to answer counts. Correction: `3 * CLUSTER_RENEW_INTERVAL <= CLUSTER_LEASE_TTL`, the "third" reading; every election call is bounded by the renew interval, so a cut-off leader is follower no later than three renew intervals after the database time of its last renewal, which the rule puts at or before the expiry of its lease; takeover after a crash within `TTL + RENEW` plus twice the answer time of a lease call (13 s with the defaults and a healthy database, at most `TTL + 3 * RENEW`), as "Election Algorithm" derives.
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
