# PRD: Real-time event delivery hub (issue #4)

Source: <https://github.com/VilnaCRM-Org/notification-service/issues/4>, as updated on 2026-11-14. Bundle: `specs/4-realtime-event-delivery-hub/`. Bounded context: `notification/event-stream`. This PRD replaces the first plan (PostgreSQL event log, leader lease, SQS source) completely; the requirement numbers are new.

## Executive Summary

The notification service becomes an open-source alternative to Mercure, written in Rust: a set of identical, stateless nodes that push events from the VilnaCRM event bus (Kafka) to browsers.

The frontend flow has three steps:

1. Read the catalog of subscribable event types over REST (`GET /api/event-types`) or GraphQL (`eventTypes`).
2. Subscribe to up to 100 event types over Server-Sent Events (`GET /api/events/stream`) or WebSocket (`GET /api/events/ws`).
3. Receive every event of a subscribed type as soon as it is written to the notifications topic, and resume after a reconnect with `Last-Event-ID` on any node.

How it works, in the terms a reader of the requirements needs:

- Every node reads the one notifications topic of Kafka itself (`KAFKA_TOPIC`, exactly one partition), without a consumer group. It commits no offset and keeps its position in memory. It filters each event against its own connected subscribers. There is no leader, no election, no lease, no lock, no link between nodes and no shared state besides Kafka (owner's decision of 2026-11-14).
- The id of an event on the wire is its Kafka offset plus 1. It is the SSE `id`, the WebSocket `id` and the resume cursor, and it is the same on every node.
- Each node keeps the most recent events in a memory window bounded by bytes. A resume inside the window is served from memory; a resume below it reads Kafka from the cursor, in bounded pages, under a few permits per node, and no further back than a configured number of offsets.
- Delivery is at least once. The service removes no duplicates: a client deduplicates by the envelope `id` of the event.
- The catalog comes from configuration (`EVENT_TYPES`). It is informative: a client may subscribe to any well-formed type.
- The Kafka client (`krafka`, pinned `=0.32.0`) sits behind a port of the domain, in the infrastructure crate, and is used only to read.
- The clients are browser frontends, usually served from another origin. One allow-list of origins, `CORS_ALLOWED_ORIGINS`, decides which pages may use the HTTP routes and the WebSocket route; by default it allows no other origin.
- The reader runs as a supervised loop. A node whose reader died leaves service with exit status 1, so its clients reconnect to a healthy node.
- PostgreSQL leaves the service and the repository, and so does the template's PostgreSQL-backed example context `company-subdomain/some-module` (owner: "Remove postgresql since we will not use it too").

Subscriber authorization is out of scope, so until it exists the topic must carry only events that are safe for every client.

## Functional Requirements

Conventions used below:

- A problem is the RFC 9457 document of `shared_application::http::Problem`, media type `application/problem+json`.
- A parameter that cannot be parsed is answered with 400, type `about:blank`, and the parser text as `detail`. A parsed value that breaks a constraint is answered with 422, type `/problems/validation-failed`, and one `violations` entry (`propertyPath`, `message`, `code`) per broken constraint. This is the behaviour of `ValidatedQuery`.
- An event type name has 1 to 128 characters and matches `^[a-z][a-z0-9]*([._-][a-z0-9]+)*$`.
- A cursor is a decimal unsigned 64-bit integer. The event at Kafka offset `o` has the id `o + 1`. A cursor `c` means "every id up to `c` was seen"; the next offset to read for it is `c`. A resume with cursor `c` delivers ids greater than `c` only.
- The tail position `P` of a node is the next offset its reader will read. It equals the id of the last offset the reader has passed. `E` is the end offset of the topic (under read-committed, the last stable offset) and `L` the earliest retained offset, both as the node last read them.
- Ids are not dense. Transaction markers, aborted records, invalid records and events of other types take offsets, so a client sees gaps. A gap is never a loss.

### Catalog

**FR-1 REST catalog.** `GET /api/event-types?page=&itemsPerPage=` returns 200 with a JSON array of `{"name": "<event type>"}`, sorted by name ascending in byte order (`order-paid` before `order.paid` before `order_paid`). `page` is 1 or greater, default 1; `itemsPerPage` is 1 to 100, default 30. A page after the last one returns `[]`. `page=abc` returns 400; `page=0`, `itemsPerPage=0` and `itemsPerPage=101` return 422 with a violation on `page` or `itemsPerPage`. The operation is in the OpenAPI document and its committed snapshot, with the 200, 400, 422 and default problem responses. It reads no broker and answers on a node that is starting.

**FR-2 GraphQL catalog.** The query `eventTypes(itemsPerPage: Int! = 30, page: Int! = 1): [EventType!]!` with `type EventType { name: String! }` returns the same page as FR-1 for the same arguments, through the same query handler. The limits of FR-1 apply; a violation is a GraphQL error built from the same 422 problem. The query is in the committed GraphQL snapshot.

**FR-3 Catalog from configuration.** The catalog is the set of names in `EVENT_TYPES` (FR-29): a comma-separated list of 1 to 1 000 event type names. A repeated name appears once. Unset, the catalog is empty and both routes return `[]`. Every node with the same configuration serves the same catalog. The catalog is informative: an event of a type that is not in it is still delivered to the subscribers of that type, and a client may subscribe to a well-formed type that is not in it. No type is registered from the topic and nothing is stored. A start with an invalid name, an empty entry or more than 1 000 distinct names fails with exit status 2 and names the variable, never its value.

> Assumption: the catalog is capped at 1 000 distinct names. Every external input is bounded (NFR-14), and a catalog page is built from the whole list on every request.

### Node status

**FR-4 Node status.** `GET /api/events/status` returns 200 with this node's view of its reader and its subscribers:

| Field | JSON type | Meaning |
| --- | --- | --- |
| `state` | string | `starting`, `ready` or `shutting-down` |
| `reason` | string or null | while `starting`: `broker-unreachable`, `topic-not-found`, `partition-count` (the topic has not exactly one partition) or `catching-up`; else `null` |
| `brokerReachable` | boolean | the last watermark read succeeded |
| `position` | string or null | `P` in decimal; `null` before the first seek |
| `endOffset` | string or null | `E` in decimal from the last successful watermark read; `null` before the first one |
| `lag` | string or null | `E - P` (0 when `P >= E`); `null` when either is unknown |
| `windowFirst` | string or null | the lowest cursor the window serves (FR-13); `null` before the first seek |
| `windowLast` | string or null | the id of the newest event in the window; `null` when the window is empty |
| `subscribers` | integer | open SSE and WebSocket subscriptions on this node (FR-25) |
| `rejectedRecords` | string | invalid records the reader skipped since the process started (FR-19) |

Positions and counts that can exceed 2^53 are JSON strings, as the WebSocket `id` is. The route reads no broker: it reports state the reader already holds. It answers in every state, also while the node is starting, and it is not subject to the 503 answers of FR-25. The operation is in the OpenAPI document and its snapshot. The GraphQL query `eventStreamStatus: EventStreamStatus!` returns the same fields with the same names (`subscribers: Int!`, `brokerReachable: Boolean!`, `state: String!`, the others `String` or `String!` as above) and is in the GraphQL snapshot. No GraphQL description holds the words `node`, `Connection`, `update` or `delete`, which the schema test refuses.

> Assumption: the route is `GET /api/events/status`, next to the two streaming routes. Issue criterion 7 asks for a status route; the first plan's `GET /api/cluster` is gone with the leader.
>
> Assumption: the status is also a GraphQL query. When the example context is removed (FR-33), `EventType` would be the first object type of the sorted GraphQL snapshot with a single field, and the protected contract test removes the first field of the first type, which leaves an empty type the diff tool cannot load. `EventStreamStatus` sorts before `EventType` and has several single-line fields, and the query is useful to a frontend on its own.

### Subscription over SSE

**FR-5 Open an SSE stream.** `GET /api/events/stream?types=<a,b,...>` answers 200 with `content-type: text/event-stream` and keeps the response open. `types` is required: a comma-separated list of 1 to 100 event type names; a repeated name counts once. The stream delivers every event of a listed type and no event of another type. The response head also carries `cache-control: no-cache` and `x-accel-buffering: no`, so that a proxy does not hold frames and keep-alives back. A node that is starting, at its cap or shutting down answers with the 503 problem of FR-25 and opens no stream.

> Assumption: the parameter is the comma-separated `types`, the same on both transports, as the issue writes it.

**FR-6 SSE frame.** Each event is one frame with three fields: `id` is the event id (FR conventions) in decimal, `event` is the event type, `data` is the event as one line of compact JSON (FR-18). Compact JSON escapes every line break inside strings, so `data` is always exactly one `data:` line and the client reads the JSON unchanged. Frames of one stream have strictly increasing ids, also after the connection fell behind and caught up (FR-24).

**FR-7 SSE keep-alive.** When no frame was written for `STREAM_KEEP_ALIVE` (default 15 s), the server writes an SSE comment line. A comment carries no `id` and does not change the client's last event id.

### Subscription over WebSocket

**FR-8 Open a WebSocket.** `GET /api/events/ws[?types=<a,b,...>][&lastEventId=<n>]` upgrades to WebSocket (101). `types` is optional here: 0 to 100 names, with the rules of FR-5. A connection without types stays open and receives events after a run-time subscribe (FR-10). Every refusal is answered before the upgrade, with a problem and without a socket: invalid parameters with the problems of FR-16, a node that is starting, at its cap or shutting down with the 503 of FR-25, a handshake from an origin that is not allowed with the 403 of FR-28, and a request that is not a valid WebSocket upgrade with the 4xx status of the upgrade rejection, as a problem.

> Assumption: `types` is optional on the WebSocket route because criterion 3 lets the client subscribe at run time; it stays required on the SSE route, which has no other way to name types.

**FR-9 Server frame.** Each event is one text frame `{"id":"<event id>","type":"<event type>","data":<event>}`. `id` is a JSON string holding the decimal event id; `type` is the event type; `data` is the event as a JSON object, the same object the SSE `data` line holds (FR-18). The frame is valid JSON for any event, including strings with quotes, backslashes, control characters and characters outside the Basic Multilingual Plane.

**FR-10 Client commands.** A text frame `{"action":"subscribe","types":[..]}` adds the types to the connection's set; `{"action":"unsubscribe","types":[..]}` removes them. Both take 1 to 100 distinct names; a repeated name counts once. A valid command is not answered. After a subscribe, the events of the added types that the node delivers from then on reach the connection: those with an id above the tail position of the node when it handled the command and above the position the connection itself had reached. After an unsubscribe, no later event of the removed types is delivered, also not one that was already queued for the connection or read for it from the window or from Kafka. A command is handled between two reads of the connection, never while one is in flight, so a command sent during a replay (FR-13, FR-14, FR-24) takes effect during that replay and does not interrupt or repeat a read. Subscribing to a type already in the set and unsubscribing from a type not in the set are no-ops. The set never exceeds 100 types, and it may become empty: a connection that unsubscribed from its last type stays open.

> Assumption: a run-time subscribe starts at the position of the node and does not replay, also not when the connection later falls behind and catches up (FR-24). Replay belongs to the connection request, which is where the cursor is given.

**FR-11 Error frame, limits, ping and close codes.** A frame that is not a valid command (not JSON, unknown `action`, an unknown member, missing or empty `types`, an invalid name, more than 100 distinct names, a set that would exceed 100 types, a binary frame, a text frame larger than 16 384 bytes) is answered with one text frame `{"error":"<reason>"}`. The subscription set is unchanged and the connection stays open. A message or a single frame larger than 32 768 bytes is not answered and ends the connection without a close frame; a frame that declares more than that is refused when its header is read, before its payload is buffered. The server sends a WebSocket ping every `STREAM_KEEP_ALIVE`, also during a replay, and answers a client ping with a pong. A close frame from the client is answered with a close frame and ends the subscription. The server closes a WebSocket with code 1001 at shutdown (FR-27) and with code 1011 when a replay cannot read Kafka (FR-14); after its own close frame it waits up to 1 s for the close frame of the client.

> Assumption: the size limit of a command is a constant, 16 384 bytes: the largest valid command, 100 names of 128 characters, takes 13 135 bytes. The transport limit for a message and for a frame is twice the command limit, and the read buffer of a connection is 4 096 bytes. `HTTP_MAX_BODY_BYTES` does not apply to WebSocket messages.

### Resume and replay

**FR-12 Cursor and resume.** A subscription that carries a cursor `c` receives every retained event of its types with an id above `c`, in id order, then live events, with no duplicate and no gap between the phases. The cursor is the `Last-Event-ID` request header or the `lastEventId` query parameter, on both routes; when both are present the header wins. A subscription without a cursor receives live events only: those with an id above the tail position of the node when the subscription was registered. A cursor of `0` asks for every retained event. The source of the replay depends on the cursor: the window (FR-13), Kafka (FR-14), or the tail when the cursor is ahead of it (FR-15). The replay runs inside the stream: a subscription request is answered without a broker read. "Retained" means: in the topic, at or above `L`, and not below the replay floor of FR-14.

> Assumption: both routes accept both carriers. A browser `EventSource` sends the header by itself on reconnect; a browser WebSocket cannot set headers, so it uses the query parameter.

**FR-13 Resume from the memory window.** Each node keeps the valid events its reader passed, in id order, in a window bounded by `EVENT_WINDOW_BYTES` (default 32 MiB). The size of an event counts its encoded frame plus a fixed per-event overhead that the architecture states. When an event would exceed the bound, the oldest events leave the window first. The window has a floor `F`, the lowest cursor it serves completely: every valid event with an id in `(F, P]` is in the window. At start `F` is the start position of FR-20; when an event with id `i` leaves the window, `F` becomes `i`. A cursor `c` with `F <= c <= P` is served from memory: the events of the subscription's types with ids in `(c, P]`, then live events. Such a resume reads no broker.

**FR-14 Replay from Kafka below the window.** A cursor `c < F` is replayed from Kafka, then from the window, then live, without gap or duplicate at either seam. The replay starts at the replay floor `max(c, L, min(F, P - EVENT_REPLAY_MAX_OFFSETS))`, with `L` and `P` as the node knows them when the replay starts; events with ids between `c` and that floor are not delivered and the client is not told. A cursor below `L` therefore starts at the earliest retained event. With `EVENT_REPLAY_MAX_OFFSETS` set to 0 no replay reads Kafka and a cursor below `F` starts at `F`. Replay reads:

- read pages of at most 500 records, and the next page only when the client has taken the frames of the last one, so a client that reads nothing causes no read;
- run under at most 4 permits per node; a replay waits for a permit in arrival order, inside its stream and never inside a request;
- each use a Kafka connection of their own, never the connection of the tail (`krafka` serves one request at a time per handle);
- filter by type and skip invalid records (FR-19) without counting them again;
- compare offsets with the cursor in the service, so a reset or rewind inside the client can never deliver an id at or below `c`.

When a replay read fails, or returns nothing for longer than `KAFKA_REQUEST_TIMEOUT` while the replay is still below `F`, the stream ends (SSE: the response ends; WebSocket: close code 1011) and the client retries with its last id. A subscriber that fell behind catches up by the same path (FR-24).

> Assumption: the replay is capped by a number of offsets below the tail, `EVENT_REPLAY_MAX_OFFSETS` (default 1 000 000), and not by age. An age cap needs record timestamps, which producers set and which can be skewed, or a time index lookup the plan has not verified on the pinned client; a count of offsets is exact and costs nothing to compute. At 100 events per second the default reaches back about 2.8 hours; the topic's retention bounds it further.
>
> Assumption: the 4 permits and the page of 500 records are constants. No deployment has to tune them, and they bound what one reconnect storm can make a node read at a time (NFR-14).

**FR-15 Cursor ahead of the tail or of the topic end.** A cursor `c > P` is accepted. The subscription waits for the tail and receives, as the tail passes them, the events with an id above `c` only. This one rule also covers a cursor above the topic end: it is neither refused nor lowered, and it receives nothing until the topic passes it. The node never delivers an id at or below `c`.

> Assumption: a cursor ahead of the topic end is not clamped and not refused, although design decision D7 offered those two forms. Lowering it to `E` would deliver ids at or below the cursor, which decision D8 forbids. Refusing it needs the true end of the topic at request time: the node's own `E` can be older than the cursor a faster node issued a moment ago, so a valid cursor would be refused, and a fresh broker read in the request path is excluded by FR-12. Waiting is exact for every cursor a node issued. A cursor from another topic (a recreated topic) receives nothing until the topic passes it; the protocol document tells operators and clients that cursors become invalid when the topic is recreated.

### Validation of input

**FR-16 Problems for invalid subscription input.** Both streaming routes answer invalid input with a problem and open no stream:

| Input | Status | Violation `propertyPath` |
| --- | --- | --- |
| `types` missing on the SSE route | 400 | none |
| `types` empty, or holding an empty name | 422 | `types` |
| an event type name that breaks the pattern or the length | 422 | `types` |
| more than 100 distinct names | 422 | `types` |
| `Last-Event-ID` or `lastEventId` that is not a decimal unsigned 64-bit integer (text, negative, overflow) | 400 | none |

The `detail` of a 400 names the parameter or header. The `code` of a violation on `types` is `required` (the list is empty or holds an empty name), `length` or `pattern` (a name that is not an event type) or `max_items` (more than 100 distinct names). An invalid name is never echoed in a log entry. The limit of 100 counts distinct names on both routes and in the commands of FR-10. The checks run in this order: the 403 of FR-28, then the 503 answers of FR-25, then the 400 and 422 of this table.

> Assumption: a malformed cursor is a 400 because it cannot be parsed, as `page=abc` is; the type list parses as text, so its faults are constraint violations and return 422.
>
> Assumption: the 403 and the 503 answers come before validation. A node that cannot serve says so to every request, and a refused origin learns nothing about the parameters.

### The reader of the topic

**FR-17 Reading without a consumer group.** Each node reads `KAFKA_TOPIC` itself, with no consumer group: it joins no group, commits no offset and keeps its position in memory only. Before it reads, and in every round until the check passes, it reads the metadata of the topic. A topic that does not exist (`reason` `topic-not-found`) or that has not exactly one partition (`reason` `partition-count`) keeps the node `starting`: it accepts no subscription, its readiness probe fails (FR-20), and it writes `event topic invalid` with the reason once per change of reason (NFR-15). The node never creates a topic and never asks the broker to create one. The reader reads with isolation level read-committed, set explicitly, so records of aborted transactions are never delivered and records of open transactions wait until their transaction ends. It never uses the client's automatic offset reset: a position below `L` is detected by the service from the watermarks and handled by FR-14 and FR-20.

> Assumption: a topic whose partition count grows while the node runs is not noticed until the node restarts. The pinned client does not report it to an assigned reader; a periodic re-check is a follow-up.

**FR-18 Event envelope.** The value of a Kafka record is a JSON object (UTF-8, RFC 8259) with these members:

| Member | Rule |
| --- | --- |
| `id` | string, a UUID in the 36-character hyphenated form, any version, letters in either case |
| `type` | string, an event type name |
| `occurredAt` | string, an RFC 3339 time in UTC written with the suffix `Z` or `+00:00`, from the year 1970 to the year 9999 |
| `data` | any JSON value |

The whole value is at most `EVENT_MAX_DATA_BYTES` bytes (default 65 536). A member name that occurs twice makes the record invalid. Other members are ignored and not forwarded. The record key and the headers are not interpreted. The event that a client receives (the SSE `data` line, the WebSocket `data` member) is the compact JSON object `{"id":"<uuid in lower case>","type":"<type>","occurredAt":"<as written>","data":<data>}`, in this member order; `data` is equal as a JSON value to the producer's `data` (same members and values; member order inside it and the spelling of numbers may change; white space outside strings is removed).

> Assumption: the event data is any JSON value and the client receives the whole envelope as one JSON object, instead of the first plan's opaque text in `data`. Design decision D4 makes the record value a JSON envelope, and decision D8 makes the client deduplicate by the envelope `id`, so the client must receive that `id`; the SSE frame has no field for it besides `data`. One object with the same shape on both transports is the smallest form that carries it.
>
> Assumption: `EVENT_MAX_DATA_BYTES` limits the whole record value, checked before the value is parsed. The name is kept from the first plan; the parse cost and the window then have a hard bound per record. Members other than the four are dropped so that a producer cannot enlarge what every client receives.
>
> Assumption: `occurredAt` accepts UTC only because the application layer may use `humantime` and no other time parser (protected allow-list), and `humantime::parse_rfc3339` accepts `Z` and `+00:00` only.

**FR-19 Invalid records.** A record whose value is missing, larger than `EVENT_MAX_DATA_BYTES`, not UTF-8, not a JSON object, or that breaks a rule of FR-18, is skipped. It is logged once at `warn` as `event record rejected` with its offset and the reason, without the value, the key, the headers or any member value, and counted in `rejectedRecords` (FR-4). It never reaches a client, it never stops the tail, and the records after it are processed normally. A replay that meets it skips it without logging or counting it again. A record batch that the client cannot decode at the tail position is skipped by seeking past it, logged in the same way with the reason `undecodable batch` and counted once.

> Assumption: a skipped record is dropped after the log entry; dead-letter handling is a concern of the producers and of a follow-up.

**FR-20 Start position, window fill and readiness.** At start a node reads the watermarks `L` and `E` and starts reading at `S = max(L, E - EVENT_WINDOW_PRELOAD)` (default 10 000 offsets below the end). It fills its window from there. It becomes `ready` once its position has reached the `E` it read at start; an empty topic (`E = L`) is ready at once. Until then it answers every subscription request, with or without a cursor, with the 503 problem whose `detail` is `the node is starting`, so no subscription can miss an event written around the start of the node. `ready` is reached once per process: a broker outage after that does not make the node `starting` again (NFR-6). `GET /api/health` gains one probe, the event stream: it fails while the node is `starting` and passes once it is `ready`; the cache and message-broker probes of the template stay. The node writes `event tail ready` once when it becomes ready.

> Assumption: a broker outage after the start does not fail the readiness probe and does not refuse new subscriptions. Every node is equally affected, so a failing probe on all of them would take the catalog and the window resumes away without moving a client anywhere better. The status route reports the outage (`brokerReachable`, `lag`).

**FR-21 Broker outages and recovery of the tail.** The tail never waits without bound: every broker call is wrapped in a time-out of the service derived from `KAFKA_REQUEST_TIMEOUT`, and the client's own request and connect time-outs are set from the same variable. The reader reads the watermarks at least once per second on a Kafka connection of its own, because the client's long poll reports no outage (it returns no records) while the watermark read returns the error. `brokerReachable`, `endOffset` and `lag` come from that read. A failed or cut round is retried after a pause (FR-26). After a failed watermark read is followed by a successful one, and whenever the watermarks show `E > P` while a poll returned no records, the reader seeks to its own position `P`, so that the client's internal fetch backoff (up to 30 s, not configurable) does not delay delivery after the broker is back. Connecting is retried by the service, because the client makes one attempt per call. When the broker is back the tail continues from `P`: no event retained in the topic is lost and none is delivered twice on a connection.

**FR-22 Every node delivers every event.** With two or more nodes reading the same topic, an event written to the topic is delivered to the matching subscribers of every node. Nodes need no configuration about each other. No leader, election, lease, lock, link between nodes, shared store besides Kafka or Kafka transaction written by the service exists, and the loss or start of a node changes nothing on the others. The cursor of an event is the same on every node, so a client resumes on any node (FR-12).

### Delivery guarantee

**FR-23 Delivery guarantee.** Delivery is at least once, in offset order, for the types of the subscription, across reconnects to any node, for events that are retained (FR-12). The service removes no duplicates: when a producer writes the same event twice, a client receives it twice, with the same envelope `id` and two different event ids (cursors). The client deduplicates by the envelope `id`. The service never sends the same offset twice on one connection and never sends an id at or below the cursor of a resume. An event of a type the subscription does not hold is never sent. An event below the replay floor of FR-14 is not delivered to a resuming client, and the client is not told.

> Assumption: no gap signal is sent when a cursor is below the replay floor. A gap signal is a follow-up; the client reloads its state through the owning service when it suspects a loss.

### Protection and lifecycle

**FR-24 Slow consumers.** Each subscriber has a queue of `SUBSCRIBER_BUFFER` events (default 256). Publishing never waits: when a subscriber's queue is full, the node stops queueing for it. The subscriber is not disconnected: it gets the frames already queued, then the events it missed, read from the window by its position, or from Kafka by FR-14 when the window has moved past it, in order and without duplicate, on the same connection, and then live events again. Other subscribers and the reader are not delayed. The service ends a connection because of its client in one case only: a client whose connection accepts no data for `STREAM_KEEP_ALIVE` is disconnected, without the frames still queued for it, an SSE client without an end of its stream and a WebSocket client without a close frame, and it resumes by reconnecting with its last id. "Accepts no data" means that nothing got through to the client for that time. A client that reads slowly is never disconnected. Two bounds of the same length enforce it: the service sets a socket send bound (`TCP_USER_TIMEOUT`, the value of `STREAM_KEEP_ALIVE`) on every accepted connection, by which the operating system ends a stalled connection of either transport, and a write to a WebSocket has a deadline of `STREAM_KEEP_ALIVE`. The send bound acts on HTTP/1.1 connections, on Linux. An SSE stream on an HTTP/2 connection that is stalled by its own flow control while the connection stays alive is not ended by it and keeps its slot and its buffers until its client leaves (Product Scope, out of scope).

> Assumption: a full queue does not disconnect the subscriber. Issue criterion 11 asks that a client that lags past the buffer catches up on its own connection, which this requirement does; only a client that takes no data at all is disconnected.

**FR-25 Subscriber cap and the 503 answers.** When the node already has `MAX_SUBSCRIBERS` subscribers (default 100 000), a new SSE or WebSocket request is answered with a 503 problem (type `about:blank`, `detail` `the node is at its subscriber capacity`) and no stream or upgrade. The cap counts connections: a slot is taken from the request until the connection has ended, and a subscriber that fell behind keeps its slot. The same 503 shape with its own `detail` answers a request on a node that is starting (`the node is starting`, FR-20) and on a node that is shutting down (`the node is shutting down`, FR-27). When more than one applies, the answer is that of shutting down, then of starting, then of the cap. A 503 is safe to retry on any node; a browser `EventSource` does not retry it by itself (FR-38). The cap is per node and not per client (NFR-14).

**FR-26 Supervision and exit status.** The reader (tail and watermark reads) runs as a supervised background loop, started after the node listens.

- A failed round does not end the loop: it is logged (NFR-15) and the next round follows after a pause of 1 s that doubles after each further failure up to 30 s and returns to 1 s after a success.
- A round that does not return is cut after a limit derived from `KAFKA_REQUEST_TIMEOUT` and counts as a failed round. The cut loses nothing: the tail continues from `P`.
- A loop that dies (a panic of its task) does not leave a node in service that keeps its clients and delivers nothing. The node writes `background task failed` (`error`, with the name of the loop), runs the shutdown sequence of FR-27 and exits with status 1. Its clients get the end of their stream or close code 1001 and resume on any node; the orchestrator starts the node again.

> Assumption: the pause after a failed round grows from 1 s to 30 s. Every node retries on its own, so during a broker outage the number of connection attempts per node stays small; a recovered broker is seen within 30 s, and FR-21 brings the backlog in by a seek.
>
> Assumption: a dead loop ends the node; it is not restarted in place. The lint policy leaves no panicking path in the service's code, so a panic comes from a dependency in a state nobody has analysed.

**FR-27 Graceful shutdown.** On SIGTERM or SIGINT the node refuses new connections at once, as today. Then, in this order, it:

1. stops the reader and every replay read; a broker call in flight is cut at once, also while the broker does not answer;
2. closes the hub, which ends every SSE response and closes every WebSocket with code 1001, each after the frames already queued for it; from then on a subscription request gets the 503 `the node is shutting down`;
3. waits at most 2 s until these connections have ended;
4. drains HTTP as today.

The whole sequence runs inside `SHUTDOWN_GRACE_PERIOD`, and the exit status is 0 when it ends in that time and 1 when the grace period ends first. An idle node stops within milliseconds, also with the broker down. A WebSocket client that takes no data is cut off without its close frame when the 2 s end; an SSE client that takes no data keeps the HTTP drain open until the send bound ends its connection, about `STREAM_KEEP_ALIVE` after it stalled, so a deployment in which exit status 0 matters in that case too sets `STREAM_KEEP_ALIVE` below `SHUTDOWN_GRACE_PERIOD`. The same sequence runs when the reader loop died, and the exit status is then 1 (FR-26). The node commits nothing to Kafka at shutdown.

> Assumption: the close codes are 1001 (going away) for shutdown and 1011 (internal error) for a replay that cannot read Kafka, from RFC 6455. The 2 s wait is a constant: an upgraded WebSocket is outside the HTTP drain, and no deployment has to tune the wait.

### Browser access

**FR-28 Browser origins.** `CORS_ALLOWED_ORIGINS` (FR-29) names the origins whose pages may use the service. It is one allow-list for the HTTP routes, where the browser enforces it through the CORS headers, and for the WebSocket route, where the service enforces it itself, because a browser applies no CORS rule to a WebSocket.

| `CORS_ALLOWED_ORIGINS` | Catalog, status, GraphQL, SSE and every other HTTP route | WebSocket handshake (a request that carries `Sec-WebSocket-Key`) |
| --- | --- | --- |
| unset (default) | no response carries a CORS header, and an `OPTIONS` request gets the 405 problem, or the 404 problem for an unknown path; a browser lets only pages of the service's own origin read the answers | answered with the 403 problem (`detail` `the origin is not allowed`) when it carries an `Origin` header; no subscriber is registered |
| a list of origins | a request whose `Origin` is on the list gets `Access-Control-Allow-Origin` with that origin on every response of every route, the problem responses (404, 405, 422, 500, 503) and the SSE response head included; every response carries `Vary: Origin`; a request of another origin is served without `Access-Control-Allow-Origin` | accepted when its `Origin` is on the list, else the 403 problem |
| `*` | `Access-Control-Allow-Origin: *` on every response | accepted from every origin |

With a list or `*` the service answers every `OPTIONS` request itself, also for an unknown path, with 200, an empty body and the headers of a preflight answer: the methods `GET` and `POST`, the request headers `Content-Type` and `Last-Event-ID` and a max age of 600 s. The answer carries `Access-Control-Allow-Origin` only for an allowed origin. No credentials are allowed and no response header is exposed. A request without an `Origin` header is never restricted, on any route. An entry is compared byte for byte with the `Origin` header. The list is a rule for browsers and not an authorization (NFR-13).

> Assumption: when the variable is unset the CORS layer is absent, not installed with an empty list, so no existing status code changes (NFR-21). With the variable unset a WebSocket handshake that carries `Origin` is refused, also from the service's own origin, because behind a proxy the service does not know its public origin.

### Configuration

**FR-29 Configuration.** The variables below are read once in the composition root and validated at start; `service check-config` applies the same rules. An invalid value fails with exit status 2 and a message `<VARIABLE>: <reason>` that never prints the value. Each variable has a row in `docs/environment.md`. `serve` starts with no variable of this table set (every one has a default or may be unset), as the protected image test requires.

| Name | Type | Default | Range and rule |
| --- | --- | --- | --- |
| `KAFKA_BOOTSTRAP_SERVERS` | comma-separated `host:port` | `localhost:9092` | 1 to 16 entries; host a DNS name, an IPv4 address or a bracketed IPv6 address; port 1 to 65 535; no empty entry. Reason: `must be a comma-separated list of host:port addresses` |
| `KAFKA_TOPIC` | Kafka topic name | `notifications` | 1 to 249 characters of `A-Z a-z 0-9 . _ -`, not `.` or `..`. Reason: `must be a Kafka topic name` |
| `KAFKA_SECURITY_PROTOCOL` | enum | `plaintext` | one of `plaintext`, `ssl`, `sasl_plaintext`, `sasl_ssl` |
| `KAFKA_SASL_MECHANISM` | enum | unset | one of `PLAIN`, `SCRAM-SHA-256`, `SCRAM-SHA-512`; required when `KAFKA_SECURITY_PROTOCOL` is `sasl_plaintext` or `sasl_ssl`, and must be unset otherwise |
| `KAFKA_SASL_USERNAME` | text | unset | not empty; the same condition as `KAFKA_SASL_MECHANISM` |
| `KAFKA_SASL_PASSWORD` | secret text | unset | not empty; the same condition; never logged, never printed |
| `KAFKA_REQUEST_TIMEOUT` | duration | `5s` | `1s` to `1m`; the client's request and connect time-outs and the base of the round limits (FR-21, FR-26) |
| `EVENT_TYPES` | comma-separated event type names | unset (empty catalog) | 1 to 1 000 distinct valid names; an empty value or entry is invalid (FR-3) |
| `EVENT_MAX_DATA_BYTES` | integer | `65536` | 1 to 1 048 576; the size limit of a record value (FR-18) |
| `EVENT_WINDOW_BYTES` | integer | `33554432` (32 MiB) | 1 048 576 to 1 073 741 824 (FR-13) |
| `EVENT_WINDOW_PRELOAD` | integer | `10000` | 0 to 1 000 000 offsets below the end at start (FR-20) |
| `EVENT_REPLAY_MAX_OFFSETS` | integer | `1000000` | 0 to 100 000 000 offsets below the tail (FR-14) |
| `SUBSCRIBER_BUFFER` | integer | `256` | 1 to 65 536 (FR-24) |
| `MAX_SUBSCRIBERS` | integer | `100000` | 1 to 1 000 000 (FR-25) |
| `STREAM_KEEP_ALIVE` | duration | `15s` | `1s` to `5m`; keep-alive, WebSocket ping, send bound and write deadline (FR-7, FR-11, FR-24) |
| `CORS_ALLOWED_ORIGINS` | `*`, or comma-separated origins | unset (no other origin, FR-28) | exactly `*`, or a list in which every entry is an origin in the form a browser sends it: scheme `http` or `https`, a host, an optional port, lower case, no path, no trailing slash, no query and no user information. `*` inside a list and an empty entry are invalid. Reason: `must be * or a comma-separated list of origins such as https://app.example.com` |

The reasons not given above use the existing texts (`must be one of ...`, `must be an integer from <a> to <b>`, `must be a duration from <a> to <b>`, `must not be empty`, `is required when KAFKA_SECURITY_PROTOCOL is sasl_plaintext or sasl_ssl`, `must not be set unless KAFKA_SECURITY_PROTOCOL is sasl_plaintext or sasl_ssl`). `.env` holds a line for each variable that has a default, written with that default, except `KAFKA_BOOTSTRAP_SERVERS`, whose line holds the address of the local broker. `EVENT_TYPES`, the three `KAFKA_SASL_*` variables and `CORS_ALLOWED_ORIGINS` have no line in `.env`. TLS (`ssl`, `sasl_ssl`) verifies the broker against the public root certificates the client ships; no custom root and no client certificate is configured.

Gone with the first plan, and never read: `NODE_ID`, `CLUSTER_LEASE_TTL`, `CLUSTER_RENEW_INTERVAL`, `EVENT_QUEUE_URL`, `EVENT_RETENTION`, `EVENT_RELAY_POLL_INTERVAL`, `LOAD_TEST_LOCALSTACK_PORT`. Gone with PostgreSQL (FR-31): `DATABASE_URL`, `DATABASE_MAX_CONNECTIONS`, `DATABASE_ACQUIRE_TIMEOUT`, `DATABASE_STATEMENT_TIMEOUT`, `DATABASE_WAIT_TIMEOUT`, `DB_VERSION`, `DB_HOST`, `DB_PORT`, `DB_NAME`, `DB_USER`, `DB_PASSWORD`, `SQLX_OFFLINE`, and the rule `HEALTH_PROBE_TIMEOUT >= DATABASE_ACQUIRE_TIMEOUT`. The template's other variables (`APP_ENV`, `HTTP_*`, `HEALTH_PROBE_TIMEOUT`, `SHUTDOWN_GRACE_PERIOD`, `AWS_*`, `SQS_*` and the rest) keep their meaning; `AWS_REGION` stays required because the template's SQS parts stay (design decision D11).

Constants and not variables, because no deployment has to tune them: the client id `notification-service`; the long-poll wait of the tail; the watermark period of 1 s (FR-21); the replay page of 500 records and the 4 permits (FR-14); the WebSocket limits (FR-11); the 2 s of the shutdown wait (FR-27); the retry pause of 1 s to 30 s (FR-26); the preflight age of 600 s (FR-28).

> Assumption: `KAFKA_BOOTSTRAP_SERVERS` has a default instead of being required. A protected test runs the production image with no Kafka variable and waits for the container to be healthy (liveness `GET /ping`); a required variable would break it. A node pointed at an address without a broker stays `starting`, which its status route, its readiness probe and its log show.
>
> Assumption: Kafka security covers plaintext, TLS with the public roots, SASL PLAIN and SCRAM. The pinned client compiles TLS (rustls) and these SASL mechanisms with its own dependencies (verified in its source, not probed against a broker); the architecture confirms each against the dependency policy and drops a value it cannot support, recording that in its decision record. OAUTHBEARER, AWS MSK IAM, a custom root and client certificates are follow-ups (decision D17).
>
> Assumption: the connect time-out equals `KAFKA_REQUEST_TIMEOUT` (the client requires the request time-out to be at least the connect time-out), so one variable covers both.

### The Kafka client behind a port

**FR-30 Kafka client behind a port.** The reader of the topic depends only on a port of the `event-stream` domain crate that offers: read the topic metadata (exists, partition count), read the watermarks (earliest, end), and read records from an offset with a bounded wait. No domain or application crate names `krafka` or any Kafka client type; the adapter lives in the `event-stream` infrastructure crate and is wired in the composition root. Replacing the client (for example by `rdkafka`) touches the infrastructure crate, the composition root and the dependency manifests only. The adapter uses the client only to read: it produces nothing, joins no group and commits nothing. `krafka` is pinned to the exact version `=0.32.0` in `[workspace.dependencies]`, and its test feature (`test-broker`) appears only on dev-dependency lines.

### Removal of PostgreSQL and of the example context

**FR-31 No database in the service.** After the removal:

- no crate depends on `sqlx`, and `Cargo.toml`, `Cargo.lock` and every manifest name no `sqlx` and no `some-module-*` crate;
- `service` has no `migrate` command; `service --help`, `service serve --help` and `service check-config --help` name no `DATABASE_*` variable;
- the composition root builds no pool, the health report has no database probe and `Dependency::Database` does not exist; `GET /api/health` reports the cache, the message broker (SQS, template) and the event stream (FR-20);
- `migrations/` and `.sqlx/` do not exist, and no file refers to them;
- `service serve` and `service check-config` succeed with no database variable set.

**FR-32 No database in the stack, the gates and CI.** After the removal:

- no compose file has a `database` or `migrate` service or a database volume; `.env`, `.env.test`, the `Dockerfile` and the dev container name no database variable or port;
- the make targets `migrate`, `migrate-rollback`, `create-migration`, `setup-test-db`, `setup-load-test-db`, `sqlx-prepare`, `validate-sqlx-metadata` and `ensure-database` do not exist, no other target depends on them, and `make help` lists none of them;
- the scripts `scripts/setup-test-db.sh`, `scripts/check-sqlx-metadata.sh` and `scripts/setup-load-test-db.sh` do not exist; `make/gates.mk` has no `SQLX_CHECK_CMD`; the gate of the sqlx metadata and its CI job are gone, and `make ci-list`, the pull-request workflows and `AGENTS.md` list the same gates;
- no workflow installs `sqlx-cli`; `scripts/install-tools.sh` and `scripts/doctor.sh` do not ask for it;
- the profile `.claude/rust-sdlc.yml` has `persistence.engine: null`, `persistence.migrations_dir: null`, `offline_metadata: false`, the keys `make.setup_test_db`, `make.create_migration`, `make.migrate`, `make.migrate_rollback`, `make.sqlx_prepare` and `make.validate_sqlx_metadata` set to `null`, and no required check `Persistence query metadata`;
- every path the removal changes or deletes that the template sync would restore is listed in `.templatesyncignore`;
- `make ci` passes, and every remaining gate still fails on the fault it exists for (the protected Bats suite runs green).

**FR-33 No example context.** After the removal the crates `crates/company-subdomain/some-module/*`, their routes, their GraphQL query and mutation fields, their OpenAPI paths and schemas, their features, steps, fixtures and tests, and their documentation do not exist. The OpenAPI and GraphQL snapshots are regenerated and committed; the GraphQL schema holds `eventTypes` and `eventStreamStatus` and has no mutation root. The workspace keeps at least 13 crates (a protected test asserts it), which holds because `notification/event-stream` is scaffolded before the example crates go. `workspace.dsl` has no database container and no example group.

**FR-34 Order and approval of the removal.** The removal runs after the notification feature, as its last epic, and only after the maintainer's approval. Its protected changes are exactly those listed by the architecture (at least: `protected-config.toml` entries for the two deleted scripts and the two required directories; `make/gates.mk`; the deleted protected scripts; `scripts/with-load-test-stack.sh`; `scripts/check-docs.sh`; the Bats files that test the database targets, variables and services; the hash pin of `features/health_check.feature`). They need the label `approved-config-change` and a session with `ALLOW_PROTECTED_CONFIG_EDIT=1`; the removed example routes and fields need the label `approved-breaking-change`. Because the configuration guard reads the required directories from the base branch, a preparatory pull request that only removes `.sqlx` and `migrations` from `required_directories` is merged before the pull request that deletes them. The notification feature never waits for this approval.

### Local stack, CI, load test and documentation

**FR-35 One Kafka broker in the stack and in CI.** The compose environments run one Kafka broker (image `apache/kafka`, KRaft mode, pinned by tag and digest), with automatic topic creation disabled and a health check. The stack creates the topic `notifications` with one partition before the service needs it. The service in the stack reads it. CI starts the broker through make, as it starts the other dependencies; the test targets (`make.integration_tests`, `make.tests_with_coverage`, `make.e2e_tests`, mutation tests) start it before they run. The broker of the stack and its topic survive a restart of the stack's service.

> Assumption: how the host reaches the broker (a host port and the variable for it, and any protected Bats edit that needs) is decided by the architecture, which lists it among the protected changes if it needs one.

**FR-36 Tests against the broker.** Integration tests of the `event-stream` infrastructure crate and of the service, and the end-to-end features, run against the broker of FR-35; each test uses a topic of its own. Fault cases a real broker cannot produce on demand (a hung request, a moved log start, an undecodable batch, a missing topic in a running node) use the client's in-memory fake broker. A two-node test runs two readers with their hubs against one topic and shows FR-22. Gherkin features under `features/` cover the catalog, the status route, SSE delivery, WebSocket delivery, resume from the window and from Kafka, an invalid record, and the 503 of a starting node.

**FR-37 K6 scenario.** The scenario `tests/Load/scripts/subscriptions.js` holds, in its smoke profile, 50 concurrent SSE subscribers and 50 concurrent WebSocket subscribers on one event type (`load.test`) while a publisher of the load-test stack writes events of that type to the topic. Each event carries its send time in `data` (`sentAtMs`). Before the first measured iteration the scenario's `setup` waits until `GET /api/events/status` reports `state` `ready`. Each iteration checks status 200 on a catalog request, that its WebSocket opens (101), and that its held SSE subscriber received at least one event; the time from `sentAtMs` to the receipt on the WebSocket is recorded in the trend `event_delivery_latency`. The SSE request carries no `test_type` tag of the profile, so its duration never enters the protected p99 budget. The catalog p99 budget and the delivery ceiling (`deliveryThreshold`) are entries of `endpoints.subscriptions` in `tests/Load/config.json.dist`, with all four profiles; the smoke profile runs inside `make.smoke_load_tests` and stays within both. No publishing route exists in the production service.

> Assumption: the publisher is part of the load-test stack and not of K6, because the pinned stock K6 image has no Kafka client and a custom image is not reproducible. The architecture chooses its form (design decision D14).
>
> Assumption: the smoke budgets start at the documented smoke ceiling of 500 ms for the catalog and 2 000 ms for delivery; they are budgets, not measurements, and are tightened by the rule of `docs/load-testing.md` after the first hosted-runner run. The scenario name sorts after `health`, which a protected test needs.

**FR-38 Documentation.** `docs/streaming-protocol.md` specifies both streaming routes, which are not in the OpenAPI document: parameters, limits and the cursor carriers; the answers before a stream opens (400, 422, 403 and the three 503 details, in the order of FR-16); the SSE head, frame and keep-alive; the event object of FR-18; the WebSocket frame, commands, error frame and reasons, size limits, ping, write deadline and close codes 1001 and 1011; the disconnection of a client that accepts no data; the delivery guarantee of FR-23, with deduplication by the envelope `id`; the window, the replay floor and what a client sees below it; the cursor ahead of the topic end (FR-15); the section "Browser access"; the reconnect procedure (reconnect to any node with the last id, after a delay that grows and has a random part; a browser `EventSource` does not reconnect after an answer other than 200, so the page creates a new one and passes `lastEventId`; 400, 403 and 422 are not retried, 503 is). `docs/requests.http` gets one example per new route. Updated in the same change: `docs/environment.md` (every row of FR-29, the removed variables); `docs/operations.md` (the topic and its one partition, the broker settings a deployment needs, readiness and the status route, the shutdown sequence and exit statuses, the send bound, the sizing of `MAX_SUBSCRIBERS` and `EVENT_WINDOW_BYTES` by memory, what the edge proxy must do, how far back a replay reaches); `docs/architecture.md`; `SECURITY.md` (NFR-13, NFR-14); new decision records under `docs/adr/` for the decisions of this plan, one of which supersedes the PostgreSQL record.

> Assumption: the two streaming routes stay out of the OpenAPI document because OpenAPI cannot describe WebSocket and a never-ending response would hang the Schemathesis gate.

### Traceability

| Issue criterion | FR ids |
| --- | --- |
| 1. REST and GraphQL catalog from configuration, in the snapshots | FR-1, FR-2, FR-3 |
| 2. SSE subscription delivers `id`, `event`, `data` of subscribed types only | FR-5, FR-6, FR-7, FR-18 |
| 3. WebSocket with run-time subscribe and unsubscribe, JSON frames | FR-8, FR-9, FR-10, FR-11 |
| 4. Resume on any node with `Last-Event-ID` or `lastEventId`, in order, before live, no repeat | FR-12, FR-13, FR-14, FR-15 |
| 5. At least once, no deduplication in the service, client deduplicates by `id` | FR-18, FR-23 |
| 6. Every node reads the topic without a consumer group; no leader, election, lease, lock or link | FR-17, FR-22 |
| 7. Subscriptions only when caught up; 503 when starting, shutting down or at capacity; node status route | FR-4, FR-20, FR-25 |
| 8. PostgreSQL and the example context removed | FR-31, FR-32, FR-33, FR-34 |
| 9. Kafka client behind a port of the domain | FR-30 |
| 10. Invalid records skipped, logged, counted | FR-19, FR-4 |
| 11. A slow or stalled client never blocks others or the reader | FR-24, FR-25, FR-11 |
| 12. RFC 9457 problems for invalid input | FR-16, FR-1 |
| 13. Browser origins through `CORS_ALLOWED_ORIGINS` | FR-28, FR-29 |
| 14. A node whose reader died exits with status 1 after the normal shutdown | FR-26, FR-27 |
| 15. One Kafka broker in the local stack and CI; tests against it | FR-35, FR-36 |
| 16. K6 smoke scenario with SSE and WebSocket subscribers | FR-37 |
| 17. Every local gate of `make ci` passes, 100 % coverage, no surviving mutant | FR-29, FR-32, FR-38, NFR-17, NFR-18, NFR-19, NFR-20, NFR-21 |

Departures from the wording of design decision D7, recorded at the requirement: a cursor ahead of the topic end waits instead of being clamped or refused (FR-15).

## Non-Functional Requirements

### Performance

**NFR-1 Capacity design target.** One node on the reference machine (4 vCPU, 8 GiB) is designed to hold 10 000 concurrent SSE or WebSocket subscribers with a p99 fan-out latency, from the moment the reader has the record to the socket write, below 100 ms at 100 events/s. This is a design target, not a measurement and not a merge gate; no latency, throughput or capacity figure exists for this design. This change delivers and tests the structural conditions of the target: NFR-2, NFR-3, NFR-9, NFR-10 and a hub test that hands one event to 10 000 subscriber queues of its type. The K6 trend of FR-37 measures another interval (from the publisher to a WebSocket client, including the broker and the generator), so it can neither prove nor disprove this target.

**NFR-2 Publish cost.** The cost of publishing one event on a node is proportional to the number of its subscribers of that type, never to the number of connections. On the live path a frame is encoded once per event and shared by all its subscribers, and an event whose type has no subscriber on the node is not encoded for a client (it is still validated and kept in the window). A replayed event is encoded for its one connection.

**NFR-3 No per-client broker read on the live path.** A connected client causes no broker read per event. The tail reads the topic once per node for all its clients. A client causes broker reads only during its own replay below the window (FR-14), in pages of at most 500 records, under at most 4 permits per node, and only as fast as it takes the frames. A resume inside the window and a subscription request read nothing from the broker.

**NFR-4 Smoke budget.** The K6 smoke profile of FR-37 passes within its p99 budget, within its delivery ceiling and within the existing success-rate threshold of the checks (above 99 %).

### Availability

**NFR-5 What must be up.** For a connected client to keep its connection: its node process. For it to receive new events: also the partition leader of the topic, so that its node's tail can read. For a new client: a node that is `ready` (it reached the end at its start once); a resume inside that node's window needs nothing else, a resume below it needs the broker. For a new event to be delivered: the producer must write it to the topic (the broker and the replicas its acknowledgement setting requires) and the nodes must read it. Nothing else is shared: no database, no leader, no other node.

**NFR-6 Cost of a broker failure.** While the partition cannot be read: open connections stay open and get keep-alives; no new event is delivered; nodes that are `ready` stay `ready` and accept subscriptions, a resume inside the window is served, a replay below the window ends its stream (FR-14) and the client retries; a node that is starting stays starting; each node retries with its own back-off (FR-26) and writes `event tail failed` once. When the partition is readable again each tail continues from its own position (FR-21), so every event still retained in the topic is delivered, in order, without repeat on a connection. How long a partition leader failover of Kafka takes depends on the broker deployment and is not stated here. Kafka itself (replication factor, `min.insync.replicas`, producer acknowledgements, retention) is a deployment prerequisite, documented in `docs/operations.md`.

**NFR-7 Any node serves any client.** Nodes are interchangeable for the catalog, the status and subscriptions. The loss of a node costs its clients one reconnect and no event that is still retained. A reconnect that reaches a node that is starting, at its cap or shutting down gets a 503 and is repeated (FR-38). A node that can no longer serve does not stay in service (FR-26).

### Delivery guarantees

**NFR-8 Guarantees.** As FR-23: at least once, in offset order, per subscription types, across reconnects to any node, for retained events; no deduplication in the service; no offset twice on one connection; no id at or below a resume cursor. Read-committed: no record of an aborted transaction is delivered.

### Resilience

**NFR-9 Bounded memory.** A subscriber holds at most `SUBSCRIBER_BUFFER` shared frame references, its type set of at most 100 names and, while it replays from Kafka, one page of at most 500 records (fetch limits of the client are soft: one record batch may exceed the page budget, and a record is at most `EVENT_MAX_DATA_BYTES` after FR-19). A WebSocket holds, whatever its client sends, at most a read buffer of 4 096 bytes, one inbound frame and one inbound message of at most 32 768 bytes each, and a write buffer of the size of the largest event sent on it. An SSE response whose client does not read holds the write buffer of the HTTP server plus one frame for about `STREAM_KEEP_ALIVE`. The hub keeps an entry only for an event type that has a subscriber. The window holds at most `EVENT_WINDOW_BYTES`. The node holds at most `MAX_SUBSCRIBERS` connections. `docs/operations.md` gives the sizing rule: `MAX_SUBSCRIBERS` times the cost of an idle connection, plus `EVENT_WINDOW_BYTES`, plus 4 replay pages, stays below half the memory limit. The per-connection cost of the first plan (about 6 KiB for an idle WebSocket, 20 KiB for an idle SSE stream) is an estimate, not a measurement. The production compose reference, with its memory limit of 256 MiB, sets `MAX_SUBSCRIBERS` to 5 000 unless its env file says otherwise.

**NFR-10 Back-pressure isolation.** No operation on the publish path waits for a client. A subscriber that cannot keep up affects only itself: it leaves live delivery and catches up at its own pace (FR-24). A connection that accepts no data is ended about `STREAM_KEEP_ALIVE` after it stalled and holds only its own slot and buffers until then. The tail and the other subscribers keep their pace.

**NFR-11 Bounded rounds.** No broker call of the service waits without bound (FR-21). A failed or hung round is cut, logged once per outage and retried with back-off (FR-26); a broker outage never ends a node. Reconnect storms after the loss of a node are bounded by the cap (FR-25) and by the 4 replay permits per node (FR-14).

### Scalability

**NFR-12 Horizontal scale.** Nodes are stateless apart from their connections, their tail position and their window; all shared state is in Kafka. Adding a node adds room for subscribers without configuration on the other nodes. Every node receives every event of the topic: the broker serves the whole topic once per node, and every node validates every event, so adding nodes does not raise the event rate the system can carry. The event rate is bounded by one partition and by what one node can read and validate. Several partitions are a follow-up.

### Security

**NFR-13 No subscriber authorization yet.** Any client that reaches the service can read the catalog and subscribe to any type. The topic must therefore carry only events that are safe for every client. `SECURITY.md` and the protocol document say so. The allow-list of origins (FR-28) binds browsers only. Follow-up: authorization with a token from the user service, and private topics.

**NFR-14 Input limits and secrets.** Every external input is bounded: 100 types per subscription, 128 characters per name, a 64-bit cursor, 16 384 bytes per WebSocket command and 32 768 bytes per WebSocket message or frame, `EVENT_MAX_DATA_BYTES` per record, 1 000 catalog names, `MAX_SUBSCRIBERS` connections per node. What a client can make a node do is bounded: a replay reads pages of 500 records under 4 permits and only as fast as its client reads, an inbound frame never repeats a read (FR-10), and the hub keeps no entry for a type without a subscriber. No log entry, problem or configuration error holds a secret, the value of a variable (`KAFKA_SASL_PASSWORD` above all), event data, a record key or header, or an event type name taken from a request. `make.check_security` and `make.secrets_scan` pass. The service sets no limit per client (connections per address, command rate): the edge proxy must limit connections per client address, which `docs/operations.md` states.

### Observability

**NFR-15 Structured logs.** One structured `tracing` entry with a fixed message is written for each case below. No entry holds event data, a record key or header, an event type name taken from a request or the value of a variable.

| Message | Level | Fields | Written when |
| --- | --- | --- | --- |
| `event tail ready` | `info` | position | the node reached the end it read at start (FR-20) |
| `event topic invalid` | `error` | reason (`topic-not-found`, `partition-count`) | the metadata check fails, once per change of reason (FR-17) |
| `event tail failed` | `warn` | the error chain | the first failed or cut round after a success (FR-21, FR-26) |
| `event tail recovered` | `info` | position | the first successful round after a failure |
| `event record rejected` | `warn` | offset, reason | an invalid record or undecodable batch was skipped by the tail (FR-19) |
| `replay failed` | `warn` | transport, the error chain | a replay could not read Kafka and its stream ended (FR-14) |
| `subscribers fell behind` | `warn` | their number | once per tail batch in which at least one subscriber fell behind (FR-24) |
| `subscriber evicted` | `warn` | transport, number of types, last id | a WebSocket write missed its deadline (FR-24); best effort, because the send bound of the operating system usually ends a stalled connection first and writes no entry |
| `subscription rejected` | `warn` | the `detail` of the 503 | a subscription request was refused (FR-25) |
| `background task failed` | `error` | the name of the loop | the reader loop died; the node shuts down (FR-26) |

A healthy node writes `event tail ready` once and is otherwise silent while it works. The existing entries of `serve` (`listening`, `shutdown signal received`, `stopped` and the entry of a forced shutdown) keep their message, level and fields.

**NFR-16 Counters.** The status route (FR-4) and its GraphQL query expose the counters and gauges of a node by name: `subscribers`, `rejectedRecords`, `position`, `endOffset`, `lag`, `windowFirst`, `windowLast`, `brokerReachable`, `state`, `reason`. A metrics endpoint is a follow-up.

### Maintainability and quality bar

**NFR-17 Coverage and mutation.** `make.tests_with_coverage` reports 100 % line and 100 % function coverage with no exclusion added. `make.mutation_tests_diff` reports zero surviving and zero timed-out mutants in the changed code. No `#[mutants::skip]`, no `#[coverage(off)]`.

**NFR-18 Static analysis and complexity.** `make.static_analysis` and `make.complexity` report zero findings: clippy pedantic, cognitive complexity at most 8, at most 50 lines and 5 arguments per function. No `#[allow]`; an `#[expect]` carries a reason. Time is read through the `Clock` port and tokio time; ids come from `UuidGenerator`. Errors reach clients as problem documents. Tools run only through make, and the git hooks are never bypassed.

**NFR-19 Architecture and protected files.** `make.architecture` passes: the domain crate uses only the allowed crates and no I/O; `event-stream` and `health-check` do not depend on each other except through the composition root. The only protected changes of the feature are allow-list entries in `architecture.toml`, made in one story after the maintainer's approval: `krafka` and `tokio` in the infrastructure layer, `serde_json` in the application layer, and `tokio-tungstenite` in the application dev list (the architecture gives the reasons); the protected changes of the removal are those of FR-34. WebSocket comes from the `ws` feature of axum and the CORS layer from the `cors` feature of tower-http. `workspace.dsl` is updated with the three new crates and the Kafka relationship. Decision records state why `krafka` was chosen (passes `deny.toml` without lifting the OpenSSL ban, no C toolchain for its Kafka code, time-outs, a fake broker for tests) and its weakness (one author, pre-1.0, fast releases), limited by the exact pin, the port and read-only use; `rdkafka` is the named alternative.

**NFR-20 Tests at every level.** Unit tests with fakes from `tests/support`; integration tests against the broker and, for faults, the fake broker (FR-36); a two-node test; Gherkin features; the JSON frame encoder tested for byte equality with `serde_json::to_string`. Tests use paused tokio time wherever a timer is involved. Every awaited call of a test is bounded by a time-out, and a fake that a loop waits on stays pending after its script instead of answering for ever, so a mutant fails an assertion and never hangs a test.

### Compatibility

**NFR-21 Contracts and exit statuses.** The OpenAPI and GraphQL snapshots under `.github/` are regenerated in the same change. During the feature epics `make.openapi_diff` and `make.graphql_diff` report additions only (`GET /api/event-types`, `GET /api/events/status`, `eventTypes`, `eventStreamStatus`); the removal of the example operations and fields (FR-33) is a breaking change approved by the label `approved-breaking-change`. No existing route, status code or variable of the service changes while `CORS_ALLOWED_ORIGINS` is unset, except what FR-31 to FR-33 remove. `make.schemathesis_validate` passes with the documented operations. Long-lived streams work under the existing request-timeout middleware, which bounds only the time to the response head. The exit statuses of `serve` keep their meaning: 0 after a shutdown inside the grace period, 1 after a forced shutdown, a failed start or a dead reader loop, 2 for an invalid configuration.

## Product Scope

### In scope

- The context `notification/event-stream` (domain, application, infrastructure crates), scaffolded with `make.new_context` before any crate is removed.
- The catalog from configuration over REST and GraphQL; the node status over REST and GraphQL.
- SSE and WebSocket subscriptions by event type, with resume from the memory window and replay from Kafka below it.
- The reader: one topic with one partition, no consumer group, read-committed, start below the end, readiness, recovery after outages; `krafka =0.32.0` behind a domain port.
- The hub with bounded queues, catch-up of a subscriber that falls behind, the disconnection of a client that accepts no data, the cap on connections, the 503 answers.
- The allow-list of browser origins; the supervision of the reader; the bounded shutdown.
- Configuration, documentation, decision records, contract snapshots, the architecture model, one Kafka broker in the compose environments and CI, the K6 scenario with its publisher, and the tests of NFR-20.
- The removal of PostgreSQL and of the example context (FR-31 to FR-34), as the last epic, after approval.

### Out of scope

| Item | Follow-up |
| --- | --- |
| Subscriber authorization (a token from the user service) and private or scoped topics | A later issue. Until then only events safe for every client (NFR-13). |
| Subscriptions narrower than an event type (for example one project) | Not planned in this issue. |
| Several partitions or several topics | A later issue; the growth path is a short list of offsets as the cursor. |
| The schema repository (payload validation, catalog descriptions) | A later issue; it replaces `EVENT_TYPES`. |
| Removal of duplicates in the service | Not planned; clients deduplicate by the envelope `id`. |
| Removing SQS and LocalStack from the repository | Only if the owner asks; they stay as the template ships them and the notification code does not use them. |
| A routing tier or any coordination between nodes | Not planned. |
| TLS client certificates, a custom root, SASL OAUTHBEARER and AWS MSK IAM | A later issue. |
| Limits per client (connections per address, command rate) | The edge proxy limits connections per address; limits in the service come with authorization. |
| A bound for an SSE stream on an HTTP/2 connection stalled by flow control | A later issue: a write deadline for SSE responses inside the service. |
| A gap signal for a cursor below the replay floor; dead-letter handling | Later issues, if operation shows the need. |
| Noticing a partition count that grows while a node runs | A later issue (FR-17). |
| Capacity measurement of NFR-1 | A later issue: it needs a load generator outside the node and server-side timestamps. |
| A metrics endpoint | A later observability issue. |
| A publish endpoint for clients | Not planned: events enter through the topic only. |

### Delivery guarantees

1. **Order:** the offset order of the one partition is the single total order for all types and all nodes. Ids are unique and increasing, may have gaps, and are the same on every node.
2. **Live delivery:** each node delivers each valid event to each of its matching subscribers at most once per connection, in offset order. A node accepts subscriptions only once it has reached the end it read at start (FR-20).
3. **Resume:** a client that reconnects to any node with the last id it received gets every later retained event of its types, in order, before live events, and never an id at or below its cursor.
4. **At least once:** the service removes no duplicates. An event written twice by a producer arrives twice, with the same envelope `id`; the client deduplicates by it.
5. **Retention:** what a resume can reach is bounded by the topic's retention and by `EVENT_REPLAY_MAX_OFFSETS` below the node's tail. A client away longer than that can miss events and is not told.
6. **Lagging client:** a subscriber more than `SUBSCRIBER_BUFFER` events behind is neither throttled nor disconnected; it catches up from the window or from Kafka on its connection. A client whose connection accepts no data for `STREAM_KEEP_ALIVE` is disconnected and recovers through guarantee 3.
7. **Not guaranteed:** delivery to a client that never reconnects; delivery of an invalid record; delivery of a record of an aborted transaction; order as the producers intended beyond the offset order; a slot for a client while another client occupies the node; confidentiality between clients.
