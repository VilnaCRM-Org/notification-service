# PRD: Real-time event delivery hub (issue #4)

Source: <https://github.com/VilnaCRM-Org/notification-service/issues/4>. Bundle: `specs/4-realtime-event-delivery-hub/`. Bounded contexts: `notification/event-stream` and `internal/cluster`.

## Executive Summary

The notification service becomes an open-source alternative to Mercure, written in Rust: a cluster of identical nodes that pushes events from the VilnaCRM event bus to browsers.

The frontend flow has three steps:

1. Read the catalog of subscribable event types over REST (`GET /api/event-types`) or GraphQL (`eventTypes`).
2. Subscribe to up to 100 event types over Server-Sent Events (`GET /api/events/stream`) or WebSocket (`GET /api/events/ws`).
3. Receive every event of a subscribed type as soon as it enters the service from the event bus, and resume after a reconnect with `Last-Event-ID` without loss or duplicate.

How it works, in terms a reader of the requirements needs:

- One node, the leader, reads the event bus through the `EventSource` port and appends each event to an event log in PostgreSQL. The log assigns a sequence number, which is the total order, the SSE `id` and the resume cursor.
- Every node relays new log rows to its own connected subscribers through an in-process hub. A PostgreSQL notification wakes the relay; a poll is the fallback.
- The leader is chosen by a PostgreSQL lease with a term that grows on every change of holder. Raft is not used: the crate allow-list in the protected `architecture.toml` admits no Raft crate, and PostgreSQL is already the consistent store.
- The first `EventSource` adapter consumes an SQS queue (LocalStack in the local stack). The Kafka adapter and the schema repository are follow-ups; the port is their seam.

The code, the API and the table of the example context `company-subdomain/some-module` stay unchanged; its migration test is narrowed to its own table, and the two tests of `service migrate` that count migrations read the count from the embedded migrator, because the new migrations join the same set. Subscriber authorization is out of scope, so until it exists the service must carry only events that are safe for every client.

## Functional Requirements

Conventions used below:

- A problem is the RFC 9457 document of `shared_application::http::Problem`, media type `application/problem+json`.
- A parameter that cannot be parsed is answered with 400, type `about:blank`, and the parser text as `detail`. A parsed value that breaks a constraint is answered with 422, type `/problems/validation-failed`, and one `violations` entry (`propertyPath`, `message`, `code`) per broken constraint. This is the behaviour of `ValidatedQuery` in the example handlers.
- An event type name has 1 to 128 characters and matches `^[a-z][a-z0-9]*([._-][a-z0-9]+)*$`.
- A cursor is a sequence number written as a decimal unsigned 64-bit integer.

### Catalog

**FR-1 REST catalog.** `GET /api/event-types?page=&itemsPerPage=` returns 200 with a JSON array of `{"name": "<event type>"}`, sorted by name ascending in byte order (the names are ASCII, so `order-paid` comes before `order.paid` and `order_paid`, whatever the locale of the database). `page` is 1 or greater, default 1; `itemsPerPage` is 1 to 100, default 30. A page after the last one returns `[]`. `page=abc` returns 400; `page=0`, `itemsPerPage=0` and `itemsPerPage=101` return 422 with a violation on `page` or `itemsPerPage`. The operation is in the OpenAPI document and its committed snapshot, with the 200, 400, 422 and default problem responses.

**FR-2 GraphQL catalog.** The query `eventTypes(page: Int = 1, itemsPerPage: Int = 30): [EventType!]!` with `type EventType { name: String! }` returns the same page as FR-1 for the same arguments, through the same query handler. The limits of FR-1 apply; a violation is a GraphQL error built from the same 422 problem. The query is in the committed GraphQL snapshot.

**FR-3 Catalog registration.** An event type is in the catalog when (a) it is listed in `EVENT_TYPES` at node start, or (b) an event of that type was ingested. Registration is idempotent: a type registered twice, or by two nodes at once, appears once. A start with an invalid name in `EVENT_TYPES` fails with exit status 2 and names the variable, never its value.

> Assumption: a catalog entry is never removed in this issue. Removal needs the schema repository, which is a follow-up.
>
> Assumption: a client may subscribe to a well-formed type that is not in the catalog. The catalog is informative, as in Mercure, and a catalog check would add a database read to every subscription.

### Subscription over SSE

**FR-4 Open an SSE stream.** `GET /api/events/stream?types=<a,b,...>` answers 200 with `content-type: text/event-stream` and keeps the response open. `types` is required: a comma-separated list of 1 to 100 event type names; a repeated name counts once. The stream delivers every event of a listed type and no event of another type.

> Assumption: the parameter is the comma-separated `types`, not `?type=<name>` as the issue text says. One parameter covers one and many types, and it is the same on both transports.

**FR-5 SSE frame.** Each event is one frame with three fields: `id` is the sequence in decimal, `event` is the event type, `data` is the event data. Data that holds line breaks is written as one `data:` line per line, as the SSE format requires, so the client reads the original text. Frames of one stream have strictly increasing ids.

**FR-6 SSE keep-alive.** When no frame was written for `STREAM_KEEP_ALIVE` (default 15 s), the server writes an SSE comment line. A comment carries no `id` and does not change the client's last event id.

### Subscription over WebSocket

**FR-7 Open a WebSocket.** `GET /api/events/ws[?types=<a,b,...>][&lastEventId=<n>]` upgrades to WebSocket (101). `types` is optional here: 0 to 100 names, with the rules of FR-4. Invalid parameters are answered before the upgrade with the problems of FR-13.

> Assumption: `types` is optional on the WebSocket route because criterion 3 lets the client subscribe at run time; it stays required on the SSE route, which has no other way to name types.

**FR-8 Server frame.** Each event is one text frame `{"id":"<sequence>","type":"<event type>","data":"<text>"}`. `id` is a JSON string holding the decimal sequence; `data` is the event data as a JSON string. The frame is valid JSON for any UTF-8 data, including quotes, backslashes, control characters and characters outside the Basic Multilingual Plane.

**FR-9 Client commands.** A text frame `{"action":"subscribe","types":[..]}` adds the types to the connection's set; `{"action":"unsubscribe","types":[..]}` removes them. Both take 1 to 100 names. After a subscribe, the events of the added types that the node relays from then on are delivered: those with a sequence above the position the node had relayed to when it handled the command and above the position the connection itself had reached. After an unsubscribe, no later event of the removed types is delivered, also not one that was already queued for the connection or read from the log for it. A command is handled between two log reads of the connection, never while one is in flight, so a command sent during a replay (FR-11, FR-22) takes effect during that replay and does not interrupt or repeat a read. Subscribing to a type already in the set and unsubscribing from a type not in the set are no-ops. The set never exceeds 100 types.

> Assumption: a run-time subscribe starts at the position of the node and does not replay, also not when the connection later falls behind and catches up from the log (FR-22). Replay belongs to the connection request (FR-11), which is where the cursor is given.

**FR-10 Error frame and ping.** A frame that is not a valid command (not JSON, unknown `action`, missing or empty `types`, an invalid name, more than 100 names, a set that would exceed 100 types, a binary frame, a text frame larger than 16 384 bytes) is answered with one text frame `{"error":"<reason>"}`. The subscription set is unchanged and the connection stays open. A message or a single frame larger than 32 768 bytes is not answered: the node refuses it when it reads the frame header, before the payload is buffered, and the connection ends without a close frame. The server sends a WebSocket ping every `STREAM_KEEP_ALIVE` and answers a client ping with a pong. A close frame from the client is answered with a close frame and ends the subscription.

> Assumption: the size limit of a command is a constant, 16 384 bytes, and not `HTTP_MAX_BODY_BYTES`: the largest valid command, 100 names of 128 characters, takes 13 135 bytes, so there is nothing to tune, and the variable would refuse valid commands at its lower bound and admit 20 MiB per message at its upper bound. The transport limit for a message and for a frame is twice the command limit, and the read buffer of a connection is 4 096 bytes. No new variable is needed.

### Resume

**FR-11 Replay, then live.** A subscription that carries a cursor receives every retained event of its types with a sequence above the cursor, in sequence order, then live events, with no duplicate and no gap between the two phases. The cursor is the `Last-Event-ID` request header or the `lastEventId` query parameter on both routes; when both are present the header wins. A subscription without a cursor receives live events only. A cursor of `0` replays every retained event of the types. A cursor above the latest sequence is treated as the latest sequence. The replay is read inside the stream, in bounded pages: a subscription request is answered without a database read, and when the log cannot be read during a replay the stream ends (SSE: the response ends; WebSocket: close code 1011) and the client retries with its last id.

> Assumption: both routes accept both carriers. A browser `EventSource` sends the header by itself on reconnect; a browser WebSocket cannot set headers, so it uses the query parameter.
>
> Assumption: when the cursor is older than the retention window the replay starts at the oldest retained event and the loss is not signalled. A gap signal is a follow-up.

**FR-12 Retention pruning.** The leader deletes log rows recorded more than `EVENT_RETENTION` ago (default 1 h). Pruning never deletes a younger row and never reuses a sequence. Followers do not prune, so a cluster without a leader does not prune.

### Validation

**FR-13 Problems for invalid subscription input.** Both streaming routes answer invalid input with a problem and open no stream:

| Input | Status | Violation `propertyPath` |
| --- | --- | --- |
| `types` missing on the SSE route | 400 | none |
| `types` empty, or holding an empty name | 422 | `types` |
| an event type name that breaks the pattern or the length | 422 | `types` |
| more than 100 distinct names | 422 | `types` |
| `Last-Event-ID` or `lastEventId` that is not a decimal unsigned 64-bit integer (text, negative, overflow) | 400 | none |

The `detail` of a 400 names the parameter or header. An invalid name is never echoed in a log entry.

> Assumption: a malformed cursor is a 400 because it cannot be parsed, which is the existing convention for `page=abc`; the type list parses as text, so its faults are constraint violations and return 422.

### Ingestion

**FR-14 `EventSource` port.** Ingestion depends only on the domain port `EventSource` (receive a batch of inbound messages, acknowledge them). No domain or application crate names SQS. Replacing the adapter touches the infrastructure crate and the composition root only.

**FR-15 SQS adapter and envelope.** When `EVENT_QUEUE_URL` is set, the adapter long-polls that queue (batch of at most 10, wait 5 s). The envelope is: message attribute `event-type` (required, an event type name), `event-id` (optional UUID; default the SQS `MessageId`), `event-time` (optional RFC 3339 time; default the `Clock` time at receipt); the message body is the event data, opaque UTF-8 text of at most `EVENT_MAX_DATA_BYTES` bytes (default 65536). Acknowledgement is `DeleteMessageBatch`. Every receive asks for a visibility timeout of 30 s, whatever the queue is configured with, so a message that was received and not acknowledged is delivered again 30 s after that receive. When `EVENT_QUEUE_URL` is unset the node never ingests and never campaigns for the lease (FR-19), and still serves the catalog, subscriptions and relay.

**FR-16 Invalid messages.** A message with a missing or invalid `event-type`, an `event-id` (or defaulted `MessageId`) that is not a UUID, an unparsable `event-time` or a body above the size limit is not appended. It is logged once at `warn` with the SQS message id and the reason, without the body, and it is acknowledged so that it does not block the queue. Valid messages of the same batch are processed normally.

> Assumption: an invalid message is dropped after the log entry. A dead-letter queue is a deployment setting of the queue (redrive policy) and a follow-up for the service.

**FR-17 Leader-only, idempotent ingestion.** Only the node whose status is leader receives from the source. A batch is appended to the log and its types are registered in the catalog (FR-3) before it is acknowledged. An event whose id is already in the log is not appended again, gets no new sequence, is delivered to no subscriber a second time, and is acknowledged. This holds while the row of the first delivery is retained: a redelivery comes 30 s after the receive (FR-15), and the shortest `EVENT_RETENTION` is 1 minute. When the append fails, nothing of the batch is acknowledged and the bus redelivers it.

### Cluster

**FR-18 Cross-node fan-out.** With two or more nodes on one database, an event ingested by any node is delivered to the matching subscribers of every node. Each node keeps a cursor, reads the log after it when the signal fires (PostgreSQL `LISTEN` on channel `notification_events`) or when `EVENT_RELAY_POLL_INTERVAL` elapses (default 1 s), and publishes the rows to its hub in sequence order. A lost notification or a broken listener connection delays delivery by at most one poll interval and loses nothing. A node starts its cursor at the latest sequence; older events reach a client only through replay (FR-11). A node accepts subscriptions once its relay has read that start position; until then it answers every subscription request, with or without a cursor, with a 503 problem whose `detail` is `the node is starting`, so no subscription can miss an event that is committed around the start of the node. At an ordinary start this lasts for one database read; on a node that starts without the database it lasts until the first relay round after the database is back.

**FR-19 Leader lease.** One lease row names the leader. Each node that has an event source (`EVENT_QUEUE_URL` is set) campaigns every `CLUSTER_RENEW_INTERVAL` (default 3 s); a node without one never calls the lease store and is always follower, so it can never hold the lease and keep the nodes that can ingest from leading. In a campaign the holder renews, another node acquires only when the lease has expired by the database clock (`CLUSTER_LEASE_TTL`, default 10 s). At any instant at most one node holds an unexpired lease. The term increases by exactly 1 when the holder changes and is unchanged by a renewal. A node whose election round fails or is cut after one `CLUSTER_RENEW_INTERVAL` becomes follower at once; a leader that cannot reach the lease store is therefore follower no later than three renew intervals after the database time of its last renewal, which the rule of FR-25 (`3 * CLUSTER_RENEW_INTERVAL <= CLUSTER_LEASE_TTL`) puts at or before the expiry of its lease.

**FR-20 Failover and release.** When the leader stops without releasing (crash, partition), another node that has an event source becomes leader no later than `CLUSTER_LEASE_TTL` plus one `CLUSTER_RENEW_INTERVAL` plus twice the answer time of a lease call after the stop (13 s with the defaults and a healthy database; never more than `CLUSTER_LEASE_TTL` plus three renew intervals, 19 s with the defaults, because a lease call is cut after one renew interval), and ingestion continues from the bus with no lost event. The messages that the stopped leader had received and not acknowledged are delivered again 30 s after their receive (FR-15), which can be later than the takeover. On graceful shutdown the leader releases the lease, and another node that has an event source becomes leader within one `CLUSTER_RENEW_INTERVAL` plus twice the answer time of a lease call (3 s with the defaults and a healthy database; never more than three renew intervals, 9 s with the defaults; when the release fails, the bound of a crash applies). Failover of ingestion needs `EVENT_QUEUE_URL` on at least two nodes: with one such node nobody takes over while it is down, and with none there is no leader and no ingestion. While the lease store is unreachable no node leads, and the bounds count from the moment it answers again.

> Assumption: criterion 6 says "within the configured lease time". The exact bound is the TTL (expiry) plus one renew interval (the pause before the next campaign of a follower) plus the answer time of the two lease calls around that pause: 13 s with the defaults when the database answers in milliseconds, and at most the TTL plus three renew intervals; the PRD states that bound.

**FR-21 Cluster status.** `GET /api/cluster` returns 200 with `{"nodeId": "<uuid>", "role": "leader" | "follower", "leader": "<uuid>" | null, "term": <integer>}`. It reports this node's view after its last election round: `leader` is `null` and `role` is `follower` when the last round failed or none has finished; `term` is the last term seen, `0` before the first successful round. A node without an event source runs no election round (FR-19), so it answers `role` `follower`, `leader` `null` and `term` `0` for the life of the process; the leader is read from a node that campaigns. The operation is in the OpenAPI document and its snapshot. `NODE_ID` sets the node id; when unset, a UUIDv7 from `UuidGenerator` is used for the life of the process.

### Protection and lifecycle

**FR-22 Slow consumers.** Each subscriber has a queue of `SUBSCRIBER_BUFFER` events (default 256). Publishing never waits: when a subscriber's queue is full, the node stops queueing for it. The subscriber is not disconnected: it gets the frames already queued, then the events it missed, read from the log by its position in bounded pages, in order and without duplicate, on the same connection, and then live events again. Other subscribers, the relay and ingestion are not delayed. A client whose socket accepts no data for `STREAM_KEEP_ALIVE` is disconnected without the frames still queued for it and without a close frame; it resumes by reconnecting with its last id (FR-11).

> Assumption: a full queue no longer disconnects the subscriber, as the first draft of this requirement did. The relay hands a page of events to the queues faster than any client task can take them, so one relay round with more than `SUBSCRIBER_BUFFER` events of a type (a backlog after a failed round, the poll fallback, a burst) would disconnect every subscriber of that type at once, however fast the clients are, and all of them would reconnect with a cursor. Catching up on the connection costs the same log reads and no reconnect. Criterion 8 of the issue ("a slow client never blocks others or ingestion") is unchanged; the only client that is disconnected is one that takes no data at all.

**FR-23 Subscriber cap.** When the node already has `MAX_SUBSCRIBERS` subscribers (default 100000), a new SSE or WebSocket request is answered with a 503 problem (type `about:blank`, a `detail` that says the node is at capacity) and no stream or upgrade. The cap counts connections: a slot is free again when the connection of the subscriber has ended, and a subscriber that fell behind (FR-22) keeps its slot. A client that stopped reading keeps its slot until it is disconnected, at most `STREAM_KEEP_ALIVE` after its socket stopped taking data. The same 503 shape with its own `detail` answers a request on a node whose relay has not started (`the node is starting`, FR-18) and on a node that is shutting down (`the node is shutting down`, FR-24). A 503 is safe to retry on any node.

**FR-24 Graceful shutdown.** On SIGTERM the node, in this order: stops the elector, ingest, relay and prune loops; releases the lease if it holds it; closes the hub, which ends every SSE response and closes every WebSocket with code 1001; then drains HTTP as today. The whole sequence fits in `SHUTDOWN_GRACE_PERIOD`. An ingest batch in progress is either appended and acknowledged or not acknowledged at all.

> Assumption: the WebSocket close codes are 1013 (try again later) for eviction and 1001 (going away) for shutdown, both from RFC 6455, so a client can tell them from an error.

### Configuration, load test and documentation

**FR-25 Configuration.** The variables below are read once in the composition root and validated at start; `service check-config` applies the same rules. An invalid value fails with exit status 2 and a message that names the variable and never prints its value. Each variable has a row in `docs/environment.md` and an entry in `.env`.

| Name | Type | Default | Bounds |
| --- | --- | --- | --- |
| `NODE_ID` | UUID | unset (a UUIDv7 is generated) | a valid UUID |
| `CLUSTER_LEASE_TTL` | duration | `10s` | `1s` to `5m` |
| `CLUSTER_RENEW_INTERVAL` | duration | `3s` | `100ms` or more, and at most a third of `CLUSTER_LEASE_TTL` (`3 * CLUSTER_RENEW_INTERVAL <= CLUSTER_LEASE_TTL`) |
| `EVENT_QUEUE_URL` | `http://` or `https://` URL | unset (the node never ingests and never leads) | a valid URL |
| `EVENT_RETENTION` | duration | `1h` | `1m` to `30d` |
| `EVENT_RELAY_POLL_INTERVAL` | duration | `1s` | `100ms` to `1m` |
| `EVENT_MAX_DATA_BYTES` | integer | `65536` | 1 to 1 048 576 |
| `EVENT_TYPES` | comma-separated event type names | unset (nothing registered at start) | every name valid (FR-3) |
| `SUBSCRIBER_BUFFER` | integer | `256` | 1 to 65 536 |
| `MAX_SUBSCRIBERS` | integer | `100000` | 1 to 1 000 000 |
| `STREAM_KEEP_ALIVE` | duration | `15s` | `1s` to `5m` |

> Assumption: the design input fixes the defaults and one cross-field rule; the lower and upper bounds in the table are chosen here so that every value is bounded and the defaults sit well inside. The design input gives two readings of the renew rule ("a third", "at least half"); the PRD takes "at most a third", which the defaults satisfy. With "less than half" a leader that lost the lease store could still believe in its lease after it had expired, because the answer time of its last renewal, the pause and the cut of the next call add up to three renew intervals (FR-19).

The local compose stack creates the SQS queue `notification-events` in LocalStack, in the region the service signs its requests for (`AWS_REGION`), and passes its URL as `EVENT_QUEUE_URL`.

**FR-26 K6 scenario.** A scenario under `tests/Load/scripts/` holds 50 concurrent SSE subscribers on one event type in its smoke profile while events of that type are published to the queue, and checks that each subscriber gets status 200 and receives the published events. Its p99 budget is an entry in `tests/Load/config.json.dist`; the smoke profile runs inside `make.smoke_load_tests` and stays within the budget. `docs/load-testing.md` gets the scenario's row.

> Assumption: the smoke budget starts at the documented smoke ceiling of 500 ms and is tightened by the rule of `docs/load-testing.md` after the first hosted-runner measurement. The protected pointers in `protected-config.toml` cover only the `health` budgets, so a new scenario entry is not a protected change.

**FR-27 Streaming protocol documentation.** A document under `docs/` specifies both streaming routes, which are not in the OpenAPI document: parameters and limits, the cursor carriers, the SSE frame and keep-alive, the WebSocket server frame, the commands, the error frame, ping, the close codes, the status codes and problems, the delivery guarantees, and the reconnect procedure. `docs/requests.http` gets one example per new route. `docs/environment.md` (in the row of `EVENT_QUEUE_URL`: a node without it never ingests and never leads), `docs/operations.md` (leader, failover, retention; that failover of ingestion needs `EVENT_QUEUE_URL` on at least two nodes; how to read `leader` `null` of `GET /api/cluster`; the takeover bounds of FR-20 and the 30 s redelivery delay after a leader crash) and `SECURITY.md` (the consequence stated in NFR-13) are updated in the same change.

> Assumption: the two streaming routes stay out of the OpenAPI document because OpenAPI cannot describe WebSocket and a never-ending response would hang the Schemathesis gate.

### Traceability

| Issue criterion | FR ids |
| --- | --- |
| 1. REST and GraphQL catalog, both in the snapshots | FR-1, FR-2, FR-3 |
| 2. SSE subscription delivers `id`, `event`, `data` of subscribed types only | FR-4, FR-5, FR-6, FR-15, FR-17 |
| 3. WebSocket with run-time subscribe and unsubscribe | FR-7, FR-8, FR-9, FR-10 |
| 4. Resume with `Last-Event-ID` or `lastEventId` | FR-11, FR-12 |
| 5. Delivery across nodes on one database | FR-18 |
| 6. One leader, failover, `GET /api/cluster` | FR-19, FR-20, FR-21 |
| 7. Ingestion through a port, SQS first, Kafka replaceable | FR-14, FR-15, FR-16, FR-17 |
| 8. A slow client never blocks others or ingestion | FR-22, FR-23 |
| 9. RFC 9457 problems for invalid input | FR-1, FR-13 |
| 10. K6 smoke scenario with SSE subscribers | FR-26 |
| 11. Every local gate of `make ci` passes, 100 % coverage, no surviving mutant | FR-25, FR-26, FR-27, NFR-16, NFR-17, NFR-18, NFR-19, NFR-20 |

## Non-Functional Requirements

### Performance

**NFR-1 Capacity.** One node on the reference machine (4 vCPU, 8 GiB) holds 10 000 concurrent SSE or WebSocket subscribers with a p99 fan-out latency, measured from the log append to the socket write, below 100 ms at 100 events/s.

**NFR-2 Publish cost.** The cost of publishing one event is proportional to the number of subscribers of its type, never to the number of connections. A frame is encoded once per event and transport and shared by all its subscribers.

**NFR-3 No database read on the live path.** A connected client causes zero per-event database reads. The node reads the log once per relay round for all its clients; a client reads the log only during its own replay (after a request with a cursor, or after it fell behind, FR-22), in bounded pages, each read covering at most 10 000 log positions.

**NFR-4 Smoke budget.** The K6 smoke profile of FR-26 passes within its p99 budget and the existing success-rate threshold of the checks (above 99 %).

### Availability and failover

**NFR-5 Any node serves any client.** Nodes are interchangeable for the catalog and subscriptions. The cluster status is each node's own view (FR-21): a node without an event source does not name the leader. The loss of a node costs its clients one reconnect and no event within the retention window. A reconnect that reaches a node that is starting, at its cap or shutting down is answered with a 503 (FR-23) and is repeated; a browser `EventSource` does not repeat a request that was answered with a status other than 200, so the page creates a new `EventSource` and passes the last id as `lastEventId` (FR-27).

**NFR-6 Failover time.** When another node has an event source, a leader crash pauses ingestion for at most `CLUSTER_LEASE_TTL` plus one `CLUSTER_RENEW_INTERVAL` plus twice the answer time of a lease call (13 s with the defaults and a healthy database, never more than `CLUSTER_LEASE_TTL` plus three renew intervals, 19 s); a graceful leader stop for at most one `CLUSTER_RENEW_INTERVAL` plus twice that answer time (3 s, never more than 9 s with the defaults). The up to 10 messages that a crashed leader had received and not acknowledged are delivered again 30 s after their receive. Without a second node that has an event source nobody takes over (FR-20). Delivery of events already in the log is not paused by a leader change.

**NFR-7 No split leadership by clock skew.** Lease expiry is judged by the database clock only, so the clocks of the nodes cannot produce two lease holders. A node that held the lease and was paused past its expiry can at worst ingest one more batch; the result is still correct because the append is idempotent by event id (FR-17).

### Delivery guarantees

**NFR-8 Guarantees.** Ingestion from the bus is at-least-once and idempotent by event id while the row of the event is retained (FR-17). The log sequence is a total order, equal to commit order. Each subscriber receives the events of its types in sequence order, each at most once per connection. Across reconnects with the last received id, a client receives every event of its types exactly once within the retention window. No guarantee is given for a cursor older than the window.

### Resilience

**NFR-9 Bounded memory.** A subscriber holds at most `SUBSCRIBER_BUFFER` shared frame references, its type set of at most 100 names and, while it replays, one page of at most 500 events and 1 MiB of data plus one event; a replay never loads the retention window at once. A WebSocket holds, whatever its client sends, at most a read buffer of 4 096 bytes, one inbound frame and one inbound message of at most 32 768 bytes each, and a write buffer of the size of the largest event sent on it. A node holds at most `MAX_SUBSCRIBERS` connections, counted from the request until the connection has ended (FR-23), so the memory of a node is bounded by `MAX_SUBSCRIBERS` times these figures and `MAX_SUBSCRIBERS` is sized by the memory of the node: an idle connection costs about 6 KiB (WebSocket) or 20 KiB (SSE). At most max(1, `DATABASE_MAX_CONNECTIONS` / 2) replay reads run at one time.

**NFR-10 Back-pressure isolation.** No operation on the publish path waits for a client. A subscriber that cannot keep up affects only itself: it is taken off live delivery and reads the log at its own pace (FR-22), and a socket that accepts no data is closed after `STREAM_KEEP_ALIVE`; the relay, other subscribers and ingestion keep their pace. A relay round that carries more events than a queue holds disconnects nobody.

**NFR-11 Database outage.** While PostgreSQL is unreachable: open connections stay open and get keep-alives; no new event is delivered; every node is follower and ingestion stops, with messages left on the bus; the catalog answers with the 500 problem; on a node whose relay had started, a subscription without a cursor is accepted, and a subscription with a cursor is accepted and its stream ends when its first log read fails (SSE: the response ends without a frame; WebSocket: close code 1011), after which the client retries; a node that started during the outage answers every subscription request with the 503 problem `the node is starting` until its relay has read its start position (FR-18). When the database returns, a leader is elected, ingestion resumes and each relay continues from its cursor (a node that started during the outage reads its start position then and had no subscriber before), so no event is lost. PostgreSQL high availability is a deployment prerequisite. An SQS outage pauses ingestion only; it is retried at the next round.

### Scalability

**NFR-12 Horizontal scale.** Nodes are stateless apart from their connections and their relay cursor; all shared state is in PostgreSQL. Adding a node adds subscriber capacity without configuration on the other nodes. Each node uses one dedicated listener connection besides its pool. Ingestion throughput is that of one leader and does not scale with the node count.

### Security

**NFR-13 No subscriber authorization yet.** Any client that reaches the service can read the catalog and subscribe to any type. Consequence: the service must carry only events that are safe for every client, and it must not be exposed with private data on the bus. `SECURITY.md` and the protocol document say so. Follow-up: authorization with a JWT from the user-service OAuth server, and private topics.

**NFR-14 Input limits and secrets.** Every external input is bounded: 100 types per subscription, 128 characters per name, 64-bit cursor, 16 384 bytes per WebSocket command and 32 768 bytes per WebSocket message or frame (FR-10), `EVENT_MAX_DATA_BYTES` per event, 10 messages per bus batch, `MAX_SUBSCRIBERS` connections. All SQL is parameterised. No log entry, problem or configuration error holds a secret, the value of a variable or the queue URL's credentials. `make.check_security` and `make.secrets_scan` pass.

### Observability

**NFR-15 Structured logs.** One structured `tracing` entry is written for: a leadership change (`info`: node id, role, leader, term); a lease store failure and the step-down (`warn`); a WebSocket that is disconnected because a write to it missed its deadline (`warn`, message `subscriber evicted`: transport, number of types, last sequence); subscribers that fell behind (`warn`: their number, once per relay page in which at least one fell behind); a rejected subscription (`warn`: the reason, one of starting, at capacity, shutting down); an invalid inbound message (`warn`: message id, reason); an ingestion, relay or prune failure (`error`, with the error chain, once per failed round). No entry contains event data. The existing readiness report keeps its three probes; no probe is added.

### Maintainability and quality bar

**NFR-16 Coverage and mutation.** `make.tests_with_coverage` reports 100 % line and 100 % function coverage with no exclusion added. `make.mutation_tests_diff` reports zero surviving and zero timed-out mutants in the changed code. No `#[mutants::skip]`, no `#[coverage(off)]`.

**NFR-17 Static analysis and complexity.** `make.static_analysis` and `make.complexity` report zero findings: clippy pedantic, cognitive complexity at most 8, at most 50 lines and 5 arguments per function. No `#[allow]`. Time is read through the `Clock` port and tokio time; ids come from `UuidGenerator`.

**NFR-18 Architecture and protected files.** `make.architecture` passes: domain crates use only the allowed crates and no I/O; the two new contexts do not depend on each other except through the composition root. No item of `protected-config.toml` changes, so `architecture.toml` is not edited; WebSocket comes from the `ws` feature of the existing axum dependency. `workspace.dsl` is updated with the six new crates and the SQS and PostgreSQL relationships. New tables come from new migrations; no applied migration is edited.

**NFR-19 Tests at every level.** Unit tests with fakes from `tests/support`; integration tests against PostgreSQL and LocalStack; a two-node relay test and a leader failover test on one database; Gherkin features under `features/` for the catalog, SSE delivery, WebSocket delivery and resume; the JSON frame encoder property-tested against `serde_json`. Tests use paused tokio time, not real waits, wherever a timer is involved.

### Compatibility

**NFR-20 Contracts.** The OpenAPI and GraphQL snapshots under `.github/` are regenerated in the same change; `make.openapi_diff` and `make.graphql_diff` report additions only. No existing route, schema, status code or variable changes. `make.schemathesis_validate` passes with the two new documented operations. Long-lived streams work under the existing request-timeout middleware, which bounds only the time to the response head.

## Product Scope

### In scope

- The contexts `notification/event-stream` (domain, application, infrastructure crates) and `internal/cluster` (the same three), scaffolded with `make.new_context`.
- The catalog over REST and GraphQL; SSE and WebSocket subscriptions with resume; the in-process hub with bounded queues, catch-up from the log for a subscriber that falls behind, and the subscriber cap.
- The event log, the catalog table and the lease table in PostgreSQL, with new migrations; the relay with signal and poll; retention pruning.
- The `EventSource` port and the SQS adapter; the queue in the local compose stack.
- Leader election by lease, `GET /api/cluster`, release on shutdown.
- Configuration, documentation, contract snapshots, the architecture model, the K6 scenario and the tests of NFR-19.

### Out of scope

| Item | Follow-up |
| --- | --- |
| Subscriber authorization and private topics | A later issue: JWT from the user-service OAuth server. Until then only events safe for every client (NFR-13). |
| Kafka adapter | A later issue with the label `approved-config-change`, because the Kafka client must be added to the protected allow-list. It implements `EventSource` and maps headers to the envelope of FR-15. |
| Schema repository integration (payload validation, catalog descriptions, type removal) | A later issue; the catalog port is the seam. Event data stays opaque text. |
| Raft or another consensus protocol | Rejected in an ADR of this change; no follow-up planned. |
| Removing the example context `company-subdomain/some-module` | The guided story of `docs/new-service.md`. |
| Gap signal for a cursor older than retention; dead-letter handling in the service | Later issues, opened if operation shows the need. |
| A publish endpoint for clients | Not planned: events enter through the bus only. |
| A second application replica in the compose stack | Not needed: cluster behaviour is proven by integration tests with two sets of adapters on one database. |
| Metrics endpoint and new health probes | A later observability issue; this issue ships log entries only (NFR-15). |

### Delivery guarantees

1. **Ingestion:** at-least-once from the bus. A message is acknowledged only after its event is in the log (or after it was judged invalid and logged). A redelivered event is recognised by its id and stored once, as long as its first row is retained: the bus redelivers after 30 s and the shortest retention is 1 minute (FR-17).
2. **Order:** the log sequence is the single total order for all types and all nodes. Sequences are unique and increasing; they may have gaps and are never reused.
3. **Live delivery:** every node delivers each log row to each of its matching subscribers at most once per connection, in sequence order, within one relay poll interval of the commit even when the notification is lost.
4. **Resume:** a client that reconnects to any node with the last id it received gets every later retained event of its types, in order, without duplicate, before live events.
5. **Retention:** events are kept for `EVENT_RETENTION` after they were recorded. A client away longer than that can miss events and is not told.
6. **Lagging client:** a subscriber more than `SUBSCRIBER_BUFFER` events behind is neither throttled nor disconnected; the node stops queueing for it and it catches up from the log on its connection, in order and without duplicate (guarantee 4 without the reconnect). A client whose socket accepts no data for `STREAM_KEEP_ALIVE` is disconnected and recovers through guarantee 4.
7. **Not guaranteed:** delivery to a client that never reconnects; delivery of a message the adapter judged invalid; order between events as produced on the bus (the order is that of ingestion); confidentiality between clients (NFR-13).
