# Streaming protocol

This document specifies the two streaming routes of the service, `GET /api/events/stream` (Server-Sent Events) and `GET /api/events/ws` (WebSocket), and the GraphQL subscription `events`, which `/api/graphql` serves over Server-Sent Events (see [GraphQL subscriptions over Server-Sent Events](#graphql-subscriptions-over-server-sent-events)), for the authors of clients. Neither route is in the OpenAPI document: OpenAPI cannot describe a WebSocket, and a response that never ends would hang the contract tests. The catalog `GET /api/event-types` and the node status `GET /api/events/status` are ordinary REST operations of the OpenAPI document; [`requests.http`](requests.http) has example requests for each of the four routes and for both forms of the GraphQL subscription. Variable names and defaults are listed in [`environment.md`](environment.md); how a node is run is described in [`operations.md`](operations.md).

## Contents

- [Events, ids and cursors](#events-ids-and-cursors)
- [Opening a stream](#opening-a-stream)
- [Answers before a stream opens](#answers-before-a-stream-opens)
- [Server-Sent Events](#server-sent-events)
- [WebSocket](#websocket)
- [Clients that accept no data](#clients-that-accept-no-data)
- [Delivery guarantee](#delivery-guarantee)
- [Resume, the window and the replay floor](#resume-the-window-and-the-replay-floor)
- [No subscriber authorization](#no-subscriber-authorization)
- [Browser access](#browser-access)
- [Reconnecting](#reconnecting)
- [GraphQL subscriptions over Server-Sent Events](#graphql-subscriptions-over-server-sent-events)
  - [The field and the event object](#the-field-and-the-event-object)
  - [Choosing the event stream](#choosing-the-event-stream)
  - [The POST form](#the-post-form)
  - [The GET form](#the-get-form)
  - [The head and the blocks](#the-head-and-the-blocks)
  - [Queries over Server-Sent Events](#queries-over-server-sent-events)
  - [Answers before a GraphQL stream](#answers-before-a-graphql-stream)
  - [Error results inside a GraphQL stream](#error-results-inside-a-graphql-stream)
  - [The cursor of a GraphQL subscription](#the-cursor-of-a-graphql-subscription)
  - [Keep-alive and the end of a GraphQL stream](#keep-alive-and-the-end-of-a-graphql-stream)
  - [Delivery and authorization](#delivery-and-authorization)
  - [Reconnecting a GraphQL subscription](#reconnecting-a-graphql-subscription)
  - [Connections per browser](#connections-per-browser)
- [Limits](#limits)

## Events, ids and cursors

Producers write events to one Kafka topic with exactly one partition. Every node of the service reads the whole topic and sends each event to its own subscribers of the event's type.

**Event type names.** A name has 1 to 128 characters and matches `^[a-z][a-z0-9]*([._-][a-z0-9]+)*$`, for example `user.created` or `order-paid`. The catalog of `GET /api/event-types` lists the names the deployment announces (`EVENT_TYPES`). It is informative: a client may subscribe to any well-formed name, and an event of a type that is not in the catalog still reaches the subscribers of that type.

**Event id.** The event at Kafka offset `o` has the id `o + 1`, written in decimal. The id of an event is the same on every node, so a client can resume on any node. Ids are not dense: transaction markers, records of aborted transactions, invalid records and events of other types take offsets, so a client sees gaps. A gap is never a loss.

**Cursor.** A cursor is a decimal unsigned 64-bit integer and means "every id up to this one was seen". A client resumes by sending the id of the last event it received; the stream then delivers ids greater than the cursor only. The cursor `0` asks for every retained event.

**The event object.** Each event reaches the client as one compact JSON object, the same object on every transport: the `data` line of the SSE route, the `data` member of a WebSocket frame and the field `data` of the GraphQL `Event`:

```json
{"id":"8f14e45f-ceea-467f-a8f4-3d1b5c4e1a2b","type":"user.created","occurredAt":"2026-11-14T10:00:00Z","data":{"userId":"5f2c"}}
```

| Member | Content |
| --- | --- |
| `id` | the envelope id the producer wrote: a UUID in the 36-character hyphenated form, in lower case. It identifies the event, not its position: an event written twice has one envelope id and two event ids. Deduplicate by this member. |
| `type` | the event type name |
| `occurredAt` | the time the producer wrote, an RFC 3339 time in UTC with the suffix `Z` or `+00:00`, unchanged |
| `data` | the producer's data, any JSON value; equal as a JSON value to what the producer wrote (member order inside it and the spelling of numbers may change, white space outside strings is removed) |

The members appear in this order. Members other than these four, the record key and the record headers never reach a client. A record whose value is not a valid envelope, or is larger than `EVENT_MAX_DATA_BYTES`, is skipped by every node and never reaches a client; it only leaves a gap in the ids.

## Opening a stream

| Route | Transport | `types` |
| --- | --- | --- |
| `GET /api/events/stream?types=<a,b,...>` | Server-Sent Events, `text/event-stream` | required: 1 to 100 names |
| `GET /api/events/ws[?types=<a,b,...>]` | WebSocket, upgrade to 101 | optional: 0 to 100 names; the client adds and removes types at run time |

Parameters, on both routes:

- `types`: comma-separated event type names. A repeated name counts once; the limit of 100 counts distinct names. The stream delivers every event of a listed type and no event of another type.
- The cursor, in the request header `Last-Event-ID` or the query parameter `lastEventId`. Each carrier that is present must hold a cursor, also when the other one does: a malformed `lastEventId` beside a valid header is refused (see [Answers before a stream opens](#answers-before-a-stream-opens)). When both hold a cursor the header wins: the stream resumes after the cursor of the header. A browser `EventSource` sends the header by itself when it reconnects after a stream ended, beside the `lastEventId` of its URL; a browser `WebSocket` cannot set headers and uses the parameter.

A request without a cursor receives live events only: those with an id above the position the node had read when it registered the subscription. A request with a cursor receives every retained event of its types above the cursor, in id order, then live events, with no gap and no duplicate between the phases. The replay runs inside the stream after it opened: no answer before the stream opens depends on a read from Kafka (see [Resume, the window and the replay floor](#resume-the-window-and-the-replay-floor)).

A cursor replays only the types of the opening request. A WebSocket request without `types` therefore starts live: its cursor replays nothing and only keeps ids at or below it from being written, and the types the client adds later by command never replay (see [WebSocket](#websocket)). A client that resumes names every type it wants replayed in `types` (see [Reconnecting](#reconnecting)).

A cursor ahead of the node's position is accepted. The stream then waits until the node has read the end of the topic twice after the stream began (the node reads it at least once per second while it can reach Kafka; keep-alives continue during the wait). A cursor at or below that end receives ids above the cursor. A cursor above the end of the topic is clamped to the end, never refused: the stream continues as if the cursor were the end and receives every later event.

## Answers before a stream opens

Every refusal is a problem document (RFC 9457, `application/problem+json`) and opens no stream and no socket. The checks run in the order of the table; the first that applies answers. The column "Route" names the SSE route (`GET /api/events/stream`), the WebSocket route (`GET /api/events/ws`) and the two forms of the GraphQL subscription on `/api/graphql` (see [Answers before a GraphQL stream](#answers-before-a-graphql-stream)); a row applies only to the routes it names.

| Order | Route | Status | Problem | When |
| --- | --- | --- | --- | --- |
| 1 | WebSocket | 403 | type `about:blank`, `detail` `the origin is not allowed` | the handshake carries an `Origin` header that `CORS_ALLOWED_ORIGINS` does not allow (see [Browser access](#browser-access)); no subscriber is registered |
| 2 | GraphQL POST | 415, 413, 400 or 422 | type `about:blank`, `detail` the text of the rejection, or `The request body is not a GraphQL request.` | the problems of the JSON transport, checked in this order: the body is not `application/json` (415), is larger than `HTTP_MAX_BODY_BYTES` (413), is not JSON (400), is not one JSON object, so a batch is refused (422), or has no string member `query` (422) |
| 3 | GraphQL GET | 414 | type `about:blank`, `detail` `The query string is too long.` | the query string, as received, is longer than `HTTP_MAX_BODY_BYTES` bytes |
| 4 | SSE, WebSocket, GraphQL GET | 400 | type `about:blank`, `detail` the parser text, which names the parameter | the query string cannot be read: `types` is missing on the SSE route, or a parameter is given twice on the SSE or the WebSocket route; on the GraphQL path, the percent-encoding or the UTF-8 is invalid, `query` is missing, or one of `query`, `operationName`, `variables` and `extensions` is given twice (an unknown parameter is ignored, also when repeated) |
| 5 | SSE, WebSocket, GraphQL GET | 422 | type `/problems/validation-failed`, one entry in `violations` | on the SSE and the WebSocket route, with `propertyPath` `types`: `types` is empty or holds an empty name (`code` `required`), a name that is not an event type name (`code` `length` or `pattern`), or more than 100 distinct names (`code` `max_items`); a name of the request is never echoed. On the GraphQL path, with `propertyPath` `variables` or `extensions` and `code` `format`: the parameter is present and is not a JSON-encoded object |
| 6 | SSE, WebSocket | 400 | type `about:blank`, `detail` `Last-Event-ID must be a decimal unsigned 64-bit integer` or `lastEventId must be a decimal unsigned 64-bit integer` | a carrier of the cursor that is present does not hold a decimal unsigned 64-bit integer (empty, text, a sign, blank, overflow); the detail names that carrier. The header is checked before the parameter, and a malformed parameter is refused also beside a valid header |
| 7 | SSE, WebSocket, GraphQL subscription | 503 | type `about:blank`, `detail` `the node is shutting down` | the node is stopping |
| 8 | SSE, WebSocket, GraphQL subscription | 503 | type `about:blank`, `detail` `the node is starting` | the node has not yet read to the end of the topic once since it started, or cannot reach its broker or its topic at start |
| 9 | SSE, WebSocket, GraphQL subscription | 503 | type `about:blank`, `detail` `the node is at its subscriber capacity` | the node already holds `MAX_SUBSCRIBERS` streams, of every transport together |
| 10 | WebSocket | 4xx | type `about:blank`, `detail` the text of the rejection | the request is not a valid WebSocket upgrade, for example 400 for a missing `Sec-WebSocket-Key` header or a `Connection` header without `upgrade`, 426 for a connection that cannot be upgraded |

On the GraphQL path, the checks that the table leaves out run inside the event stream, between the transport problems and the 503: a document error, a breach of the one-root-field rule, invalid `types` and a malformed cursor are answered as an error result (see [Error results inside a GraphQL stream](#error-results-inside-a-graphql-stream)). A GraphQL query is never answered with a 503 of rows 7 to 9. A GraphQL request whose head is not decided within `HTTP_REQUEST_TIMEOUT` gets the 503 problem `The request exceeded its time budget.` (see [The head and the blocks](#the-head-and-the-blocks)).

A 503 is safe to retry on any node. A 400, 403, 413, 414, 415 or 422 is a fault of the request and is not retried (see [Reconnecting](#reconnecting)).

A request with a cursor below the node's memory window is accepted while the node cannot read Kafka; its stream then ends at the first replay read that fails (SSE and GraphQL: the response ends, a GraphQL stream without `complete`; WebSocket: close code 1011).

## Server-Sent Events

The response head is `200` with `content-type: text/event-stream`, `cache-control: no-cache` and `x-accel-buffering: no` (the last keeps a proxy from holding frames and keep-alives back). The response stays open.

Each event is one frame of three fields and an empty line:

```text
id: 1043
event: user.created
data: {"id":"8f14e45f-ceea-467f-a8f4-3d1b5c4e1a2b","type":"user.created","occurredAt":"2026-11-14T10:00:00Z","data":{"userId":"5f2c"}}

```

- `id` is the event id. The browser stores it as the last event id and sends it in `Last-Event-ID` when it reconnects.
- `event` is the event type. A browser `EventSource` therefore delivers the frame to the listener registered for that type with `addEventListener('<type>', ...)`; its `onmessage` handler receives nothing, because it handles only frames without an `event` field.
- `data` is the [event object](#events-ids-and-cursors) as one line of compact JSON. Compact JSON escapes every line break inside strings, so `data` is always exactly one line.
- The ids of one stream strictly increase, also after the connection fell behind and caught up. They have gaps.

**Keep-alive.** When no frame was written for `STREAM_KEEP_ALIVE` (default 15 seconds), the server writes the comment line `:` followed by an empty line. A comment carries no id and does not change the client's last event id.

**End of the stream.** The response ends without a final frame in two cases: the node is shutting down (after the frames already queued for the stream), and a replay below the window could not read Kafka. The client then reconnects with its last id. The server never ends a stream because its client reads slowly; only a client that accepts no data at all loses its connection (see [Clients that accept no data](#clients-that-accept-no-data)).

## WebSocket

**Server frames.** Each event is one text frame. `id` is the event id as a JSON string (an id can exceed the integers a JavaScript number holds exactly), `type` the event type and `data` the [event object](#events-ids-and-cursors):

```json
{"id":"1043","type":"user.created","data":{"id":"8f14e45f-ceea-467f-a8f4-3d1b5c4e1a2b","type":"user.created","occurredAt":"2026-11-14T10:00:00Z","data":{"userId":"5f2c"}}}
```

The frame is valid JSON for every event, also for strings with quotes, backslashes, control characters and characters outside the Basic Multilingual Plane. The ids of one connection strictly increase.

**Client commands.** The client changes the set of types of its connection with text frames:

```json
{"action":"subscribe","types":["user.created","order.paid"]}
{"action":"unsubscribe","types":["order.paid"]}
```

- Each command takes 1 to 100 distinct names. A valid command gets no answer.
- After a subscribe, the connection receives the events of the added types with an id above the position the node had read when it handled the command and above the position the connection itself had reached. A run-time subscribe never replays older events, also not when the connection later falls behind and catches up; replay belongs to the request that opens the connection, where the cursor is given. That cursor replays only the types named in `types` of the same request: a request without `types` starts live, and its cursor only keeps ids at or below it from being written.
- After an unsubscribe, no later frame of the removed types is written, also not one that was already queued for the connection or read for it from the window or from Kafka.
- Subscribing to a type already in the set and unsubscribing from a type not in the set change nothing. The set never holds more than 100 types and may become empty; a connection without types stays open.
- Commands are read between two reads of the connection, never during a read from Kafka. A command sent during a replay takes effect for every frame not yet written; it neither interrupts nor repeats a read.

**Error frame.** An invalid command is answered with one text frame `{"error":"<reason>"}`. The set of types stays as it was and the connection stays open. The checks run in this order: the size, the JSON and its shape, the names, the limit of the set.

| Cause | `error` |
| --- | --- |
| a text frame larger than 16 384 bytes | `the command is too large` |
| a binary frame | `binary frames are not supported` |
| not JSON, an unknown `action`, an unknown member, `types` missing | `invalid command` |
| `types` empty, or more than 100 distinct names | `types must hold 1 to 100 event type names` |
| a name that is not an event type name, an empty name included | `invalid event type name` |
| the set would hold more than 100 types | `a connection subscribes to at most 100 event types` |

**Size limits.** A command is at most 16 384 bytes (the largest command without white space, 100 names of 128 characters, takes 13 134 bytes). A message or a single frame larger than 32 768 bytes is not answered: it ends the connection without a close frame, and a frame whose header declares more is refused before its payload is read.

**Ping.** The server sends a ping every `STREAM_KEEP_ALIVE`, also during a replay. It answers a ping of the client with a pong at its next read.

**Write deadline.** A message that the connection does not accept within `STREAM_KEEP_ALIVE` ends the connection without a close frame (see [Clients that accept no data](#clients-that-accept-no-data)).

**Close codes.** The server closes a connection with one of two codes:

| Code | When |
| --- | --- |
| 1001 (going away) | the node is shutting down; sent after the frames already queued for the connection |
| 1011 (internal error) | a replay below the window could not read Kafka |

After its own close frame the server waits up to 1 second for the close frame of the client. A close frame of the client is answered with a close frame and ends the subscription. After 1001 or 1011 the client reconnects, to any node, with its last id.

## Clients that accept no data

The server ends a connection because of its client in one case only: nothing got through to the client for `STREAM_KEEP_ALIVE`. Two bounds of that length enforce it. The server sets the socket option `TCP_USER_TIMEOUT` to `STREAM_KEEP_ALIVE` on every accepted connection, so the operating system ends a connection whose peer leaves sent data unacknowledged, or announces a zero window, for that long (Linux, HTTP/1.1 connections); and every WebSocket write has a deadline of `STREAM_KEEP_ALIVE`. Such a client loses its connection without the frames still queued for it: an SSE client without an end of its stream, a WebSocket client without a close frame. It reconnects with the last id it received and loses nothing that is still retained.

A client that reads slowly is never disconnected. When it falls more than `SUBSCRIBER_BUFFER` events behind, the node stops queueing live events for it; the connection then receives the frames already queued, then the events it missed, read from the node's memory window or from Kafka, in order and without duplicate, and then live events again. Other clients are not delayed.

An SSE stream on an HTTP/2 connection that is stalled by its own flow control while the connection stays alive is not ended by these bounds; it keeps its slot until its client leaves.

## Delivery guarantee

- **At least once, in id order, per subscription.** Every retained event of the subscription's types with an id above the cursor is delivered, in id order, across reconnects to any node, when every reconnect names those types in `types` (see [Reconnecting](#reconnecting)).
- **No duplicate on one connection.** The service never writes the same event id twice on one connection and never writes an id at or below the cursor of the request (a cursor above the end of the topic counts as the end).
- **Duplicates across reconnects and producers are the client's to remove.** The service removes no duplicates. An event a producer wrote twice arrives twice, with the same envelope `id` and two event ids. A client that reconnected with an older id than the last one it processed receives events again. The client deduplicates by the envelope `id` of the [event object](#events-ids-and-cursors). No fixed amount of memory covers both cases; size the memory of handled envelope ids by this rule: it must reach back over the producers' retry horizon (the longest time after which a producer can write an event again, for example an outbox relay after an outage) and over the distance between the cursor the client resumes with and the last event it processed (up to `EVENT_REPLAY_MAX_EVENTS` offsets). Keeping the ids of the last few minutes, or of the last few thousand events, is enough only when the producers retry quickly and the client always resumes with the last id it received.
- **Only committed records.** The service reads with isolation level read-committed: no record of an aborted transaction is delivered, and a record of an open transaction waits until its transaction ends.
- **Types.** An event of a type the subscription does not hold is never sent.
- **Below the replay floor, nothing and no signal.** An event below the replay floor (next section) is not delivered to a resuming client, and the client is not told. A client that suspects a loss reloads its state through the service that owns the data.

## Resume, the window and the replay floor

Each node keeps the recent valid events it has read in a memory window of at most `EVENT_WINDOW_MAX_EVENTS` events and `EVENT_WINDOW_MAX_BYTES` bytes. The lowest cursor the window serves completely is reported as `windowFirst` by `GET /api/events/status`; the newest event in it as `windowLast`.

Where a resume is served from depends on its cursor `c`, compared with the node's position `P` (reported as `position`) at the moment the stream starts:

| Cursor | What the stream does |
| --- | --- |
| none | live events above `P` |
| within the window (`windowFirst` to `P`) | the events above `c` from memory, then live; no read from Kafka |
| below the window, still in the topic | the events above `c` read from Kafka in pages of up to 500 records, then from the window, then live |
| more than `EVENT_REPLAY_MAX_EVENTS` below `P` | the replay starts at the replay floor `P - EVENT_REPLAY_MAX_EVENTS`; the events between `c` and the floor are skipped, and the client is not told |
| below the earliest offset the topic retains | the replay starts at the earliest retained event (or the window floor when that is lower); the events Kafka no longer holds are gone |
| between `P` and the end of the topic | waits until the node has read the end of the topic twice, then live events above `c` |
| above the end of the topic | clamped to the end: every later event |

The replay floor is the higher of the cursor and `P - EVENT_REPLAY_MAX_EVENTS` (default 1 000 000 offsets). It also applies to a cursor inside the window. The topic's retention limits a replay further: a replay reaches back to the earliest offset the topic still retains, and not beyond the floor.

Replays from Kafka are limited per node: at most `EVENT_REPLAY_PERMITS` replays read at a time and the others wait their turn, in order, inside their streams. A replay reads its next page only after its client took the frames of the last one, so a client that reads nothing causes no read. When a replay read fails, or three page reads in a row return no record and do not move the position, the stream ends (SSE and GraphQL: the response ends, a GraphQL stream without `complete`; WebSocket: close code 1011) and the client reconnects with its last id, to any node.

**A recreated topic.** Cursors belong to one topic. When the topic is deleted and created again under the same name, its offsets start again from zero, and the cursors clients hold no longer name the same events. A cursor above the end of the new topic is clamped to that end, so such a client receives every later event of the new topic and none of the events it missed. Operators restart every node after they recreate the topic (see [`operations.md`](operations.md)); clients reload their state through the owning service.

## No subscriber authorization

The service checks no token, no session and no user identity, on any route and for any client. Any client that reaches the service, a page in a browser on an allowed origin as much as a program outside a browser, can read the catalog and subscribe to any well-formed event type, also to a type that is not in the catalog. `CORS_ALLOWED_ORIGINS` does not change this (see [Browser access](#browser-access)).

Every event written to the topic can therefore reach every subscriber of its type: its envelope `id`, its `type`, its `occurredAt` and the whole `data` member. Producers write to the topic only events that are safe for every client: no personal data, no secret and nothing meant for one user or one tenant. The record key and the record headers are never forwarded. [`SECURITY.md`](../SECURITY.md) lists what the service protects and what it does not.

## Browser access

`CORS_ALLOWED_ORIGINS` names the origins whose pages may use the service. It is one allow-list for the HTTP routes, where the browser enforces it through the CORS headers, and for the WebSocket route, where the service enforces it itself, because a browser applies no CORS rule to a WebSocket.

| `CORS_ALLOWED_ORIGINS` | Catalog, status, GraphQL, SSE and every other HTTP route | WebSocket handshake |
| --- | --- | --- |
| unset (the default) | no response carries a CORS header; only pages of the service's own origin can read the answers | refused with the 403 problem when it carries an `Origin` header, also from the service's own origin |
| a list of origins | a request whose `Origin` is on the list gets `Access-Control-Allow-Origin` with that origin on every response, the problem responses and the SSE response head included; every response carries `Vary: Origin` | accepted when its `Origin` is on the list, else the 403 problem |
| `*` | `Access-Control-Allow-Origin: *` on every response | accepted from every origin |

With a list or `*` the service answers every preflight `OPTIONS` request itself with 200, the methods `GET` and `POST`, the request headers `Content-Type` and `Last-Event-ID` and a maximum age of 600 seconds. No credentials are allowed and no response header is exposed, so a page creates its `EventSource` without `withCredentials`. An entry is compared byte for byte with the `Origin` header, which a browser sends in lower case, with the scheme, the host and a port other than the default one.

A request without an `Origin` header is never restricted, on any route. The list is a rule for browsers, not an authorization: any client that does not run in a browser can subscribe to every type (see [No subscriber authorization](#no-subscriber-authorization)).

## Reconnecting

A stream can end for reasons that are not the client's fault: a node shuts down for a deployment, a replay could not read Kafka, the network drops the connection. The client then reconnects:

1. **To any node.** Ids are the same on every node; the load balancer may send the new request anywhere.
2. **With the last id it received**, in `Last-Event-ID` or `lastEventId`, **and with every type it holds in `types`**, because a cursor replays only the types of the request that carries it. A client that has not received an event yet reconnects without a cursor, or with the cursor it started with. A client that sends both carriers puts a cursor in each: the header decides where the stream resumes, and a malformed `lastEventId` is refused with 400 even beside a valid header.
3. **After a delay that grows with each failed attempt and has a random part**, so that the clients of a node that went away do not all return at the same moment. For example: wait a random time between half and all of `min(30 s, 1 s × 2^attempt)`, and reset `attempt` after a stream opened.
4. **Not after 400, 403 or 422.** These answers report a fault of the request (a malformed type list or cursor, an origin that is not allowed); repeating the request gives the same answer. **Retry after 503** (`the node is starting`, `the node is at its subscriber capacity`, `the node is shutting down`): another node, or the same node a little later, accepts the stream.

A browser `EventSource` reconnects by itself, with `Last-Event-ID`, only after a stream that was open ended. It does not retry an answer other than 200: it closes (`readyState` becomes `CLOSED`) and fires `error`. The page then creates a new `EventSource` and passes the last id in `lastEventId`, because it cannot set the header. A browser hides the status of a refused `EventSource` or `WebSocket` request from the page, so a page validates its type names and its cursor by the rules above before it connects; a refusal it then sees is a 503 or a network fault, and it retries with the growing delay. A client that can read the status, such as a server-side client or a page that reads the stream with `fetch`, follows rule 4 as written.

A minimal browser client of the SSE route:

```js
const TYPES = ['user.created', 'order.paid'];
let lastId = null;
let attempt = 0;
const seen = new Set(); // envelope ids already handled; a real client bounds it by the rule of "Delivery guarantee"

function handle(event) {
  console.log(event.type, event.data); // the page's own handling goes here
}

function connect() {
  const url = new URL('/api/events/stream', window.location.origin);
  url.searchParams.set('types', TYPES.join(','));
  if (lastId !== null) url.searchParams.set('lastEventId', lastId);
  const source = new EventSource(url);
  source.onopen = () => { attempt = 0; };
  for (const type of TYPES) {
    source.addEventListener(type, frame => {
      lastId = frame.lastEventId;
      const event = JSON.parse(frame.data);
      if (seen.has(event.id)) return;
      seen.add(event.id);
      handle(event);
    });
  }
  source.onerror = () => {
    if (source.readyState !== EventSource.CLOSED) return; // the browser reconnects by itself
    const delay = Math.min(30000, 1000 * 2 ** attempt) * (0.5 + Math.random() / 2);
    attempt += 1;
    setTimeout(connect, delay);
  };
}

connect();
```

A WebSocket client follows the same rules: it reconnects after close code 1001 or 1011 and after a lost connection, to any node, with the growing delay. Because a cursor replays only the types of the opening request and a run-time subscribe never replays, the reconnecting client puts its whole current set of types in the `types` parameter together with the `id` of the last frame it received in `lastEventId`, for example `/api/events/ws?types=user.created,order.paid&lastEventId=1043`, and uses commands only for the changes it makes after the socket opened. A client that reconnects without `types` and sends its subscriptions again as commands loses, without a signal, every event of those types between its last id and its new subscriptions.

## GraphQL subscriptions over Server-Sent Events

The GraphQL endpoint `/api/graphql` serves the subscription `events`, and every query, as an event stream when the request asks for one. The transport is the distinct connections mode of the graphql-sse protocol, with the request formats of GraphQL over HTTP: a POST with a JSON body, as the graphql-sse client sends it, and a GET with the operation in the query string, as a browser `EventSource` sends it. Both forms are served in every `APP_ENV`. A GraphQL subscription is a subscriber of the same hub as the SSE and WebSocket routes: it has the same event ids and cursors, the same window, replay and replay floor, the same handling of slow clients and a slot under the same `MAX_SUBSCRIBERS`. This section specifies what differs; [`requests.http`](requests.http) has a request for each form.

### The field and the event object

The schema has a subscription root with one field and no mutation root; the query root is unchanged:

```graphql
type Subscription {
  events(lastEventId: String, types: [String!]!): Event!
}

type Event {
  data: JSON!
  id: String!
  type: String!
}

scalar JSON
```

- `types` lists 1 to 100 event type names, by the rules of [Events, ids and cursors](#events-ids-and-cursors); a repeated name counts once. The subscription delivers every event of a listed type and no event of another type. Its types are fixed for its lifetime: no command adds or removes one.
- `lastEventId` is the optional cursor of a resume (see [The cursor of a GraphQL subscription](#the-cursor-of-a-graphql-subscription)).
- `Event.id` is the event id in decimal, the value of the `id` field of the SSE route and of the WebSocket frame for the same event, and the cursor of a resume. It is a `String`, because an id can exceed both the GraphQL `Int` and the integers a JavaScript number holds exactly.
- `Event.type` is the event type name.
- `Event.data` is the [event object](#events-ids-and-cursors) (envelope `id`, `type`, `occurredAt` and `data`) as a value of the scalar `JSON`. It is equal, as a JSON value with its members in the same order, to the `data` line of the SSE route and to the `data` member of the WebSocket frame for the same event.
- `Event.data` costs 501 of the complexity limit of 1 000, while `Event.id` and `Event.type` cost 1 each. A document that selects `data` more than once, under several aliases, under one name written twice or through fragments, exceeds the limit and is answered with a document error before any resolver runs. The selection `{ id type data }` costs 504 and runs.

Each event is one subscription result `{"data":{"<response name>":{...}}}`, where `<response name>` is the alias of the field or `events`, and the object holds the selected fields of `Event` under their aliases, in the order of the selection set. Fragments and the directives `@skip` and `@include` work inside the selection set as in a query.

### Choosing the event stream

A request to `/api/graphql` gets an event stream when its `Accept` header ranks `text/event-stream` above the other representations of its method:

- The values of every `Accept` field of the request, joined in order, are read as a comma-separated list of media ranges. Type and subtype compare without regard to letter case, parameters other than `q` are ignored, and `q` defaults to 1. A range whose `q` is not a valid qvalue (0 to 1 with at most three decimals) is ignored, and a header value that is not visible ASCII text counts as absent.
- `s` is the highest `q` of a range `text/event-stream`, `j` the highest `q` of a range `application/json` or `application/graphql-response+json`, `w` the highest `q` of a range `*/*` or `text/*`, and `h` the highest `q` of a range `text/html`. A range that is not listed has `q` 0.
- **POST** gets the event stream when `s > j`. A tie and a wildcard stay on JSON, the default of a POST.
- **GET** gets the event stream when an `Accept` field is present and `max(s, w) > max(j, h)`. A GET has no JSON representation, so a wildcard admits the event stream. A GET without `Accept`, or one that ranks a JSON type or `text/html` at least as high, keeps its answer without an event stream: the explorer page in `dev`, else the 405 problem with `Allow: POST`.

| `Accept` | Sent by | POST | GET |
| --- | --- | --- | --- |
| absent | a client that sends no `Accept` | JSON | explorer or 405 |
| `text/event-stream` | graphql-sse client, browser `EventSource` | event stream | event stream |
| `text/event-stream, multipart/mixed` | urql subscription with `fetchSubscriptions` | event stream | event stream |
| `application/graphql-response+json, application/graphql+json, application/json, text/event-stream, multipart/mixed` | urql query | JSON | explorer or 405 |
| `*/*` | curl, generic clients, Schemathesis | JSON | event stream |
| `text/*` | generic clients | JSON | event stream |
| `text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8` | browser navigation | JSON | explorer or 405 |
| `application/json` | JSON clients | JSON | explorer or 405 |
| `application/json;q=0.5, text/event-stream` | any | event stream | event stream |
| `text/event-stream;q=0` | any | JSON | explorer or 405 |
| `TEXT/Event-Stream; charset=utf-8` | any | event stream | event stream |
| `text/event-stream;q=2` (an invalid qvalue) | any | JSON | explorer or 405 |

Every answer to a GET of `/api/graphql` carries `Vary: Accept`, the explorer page and the 405 problem included, so that a cache never hands one representation to a request for the other. A POST without the event stream keeps the JSON transport, `200 application/json`; a subscription sent there registers no subscriber, and when its document is valid it is answered with `data` `null` and the one error `Subscriptions are not supported on this transport.`. Any other method gets the 405 problem with `Allow: GET,HEAD,POST`, except a CORS preflight that `CORS_ALLOWED_ORIGINS` answers (see [Browser access](#browser-access)).

### The POST form

`POST /api/graphql` takes a body of media type `application/json` holding one JSON object with the members `query` (string, required), `operationName` (string or null), `variables` (object or null) and `extensions` (object or null, accepted and ignored). One request carries one operation; a JSON array, a batch, is refused. The body limit `HTTP_MAX_BODY_BYTES` and the problems are those of the JSON transport. The graphql-sse client sends this form, with `content-type: application/json` and `accept: text/event-stream`:

```http
POST /api/graphql HTTP/1.1
Host: localhost:8081
Content-Type: application/json
Accept: text/event-stream

{"query":"subscription ($types: [String!]!) { events(types: $types) { id type data } }","variables":{"types":["user.created","order.paid"]}}
```

The answer, without the headers that every answer of the router carries and without the framing of the HTTP version, when the event at offset 1042, of type `user.created`, is the first to arrive:

```text
HTTP/1.1 200 OK
content-type: text/event-stream
cache-control: no-cache
x-accel-buffering: no
vary: accept

:

event: next
id: 1043
data: {"data":{"events":{"id":"1043","type":"user.created","data":{"id":"8f14e45f-ceea-467f-a8f4-3d1b5c4e1a2b","type":"user.created","occurredAt":"2026-11-14T10:00:00Z","data":{"userId":"5f2c"}}}}}

```

### The GET form

A `GET /api/graphql` that gets the event stream (see [Choosing the event stream](#choosing-the-event-stream)) runs the operation of its query string, in every `APP_ENV`. It exists for the browser `EventSource`, which can send only GET and cannot set headers. The query string is `application/x-www-form-urlencoded`, where `+` is a space, with the parameters:

- `query`, the document, required;
- `operationName`, optional;
- `variables` and `extensions`, optional, each a JSON-encoded object.

Unknown parameters are ignored. The `Last-Event-ID` header is read as on POST; a browser sends it by itself when it reconnects. A GET that runs a query is answered as on POST (see [Queries over Server-Sent Events](#queries-over-server-sent-events)); a mutation cannot run, because the schema has no mutation root. The subscription of the POST example, as a GET:

```http
GET /api/graphql?query=subscription%20(%24types%3A%20%5BString!%5D!)%20%7B%20events(types%3A%20%24types)%20%7B%20id%20type%20data%20%7D%20%7D&variables=%7B%22types%22%3A%5B%22user.created%22%2C%22order.paid%22%5D%7D HTTP/1.1
Host: localhost:8081
Accept: text/event-stream
```

The answer has the head and the blocks of the POST form. A page builds the URL with `URLSearchParams` and opens it with `new EventSource(url)` (see [Reconnecting a GraphQL subscription](#reconnecting-a-graphql-subscription)). A query string that cannot be run is refused before any stream with the 414, 400 or 422 problem of [Answers before a stream opens](#answers-before-a-stream-opens).

### The head and the blocks

Every event stream, of a subscription, a query or an error result, in either form, starts with status `200` and these headers:

- `content-type: text/event-stream`, with no parameter;
- `cache-control: no-cache`;
- `x-accel-buffering: no`, which keeps a proxy from holding blocks back;
- `vary: accept`;
- the headers of every answer of the router: the request id, the security headers and, for an allowed origin, the CORS headers.

The server writes the head only after the operation reached one of three points: the `events` field registered its subscriber, the operation produced its first result (the result of a query, or an error result), or the hub refused the subscriber, which is answered with the 503 problem instead of a head. This phase runs inside `HTTP_REQUEST_TIMEOUT`. When the budget expires first, the answer is the 503 problem `The request exceeded its time budget.`, and no subscriber remains registered. The phase never reads Kafka: the replay of a cursor runs after the head, inside the stream.

The body is a sequence of blocks, UTF-8 with LF line ends, each ending with an empty line. Exactly these four blocks are written, where `\n` is one line feed:

| Block | Bytes | When |
| --- | --- | --- |
| keep-alive comment | `:\n\n` | the first block of every event stream; then whenever `STREAM_KEEP_ALIVE` passes without a block |
| subscription result | `event: next\nid: <event id>\ndata: <result>\n\n` | once per delivered event |
| single result | `event: next\ndata: <result>\n\n` | the result of a query, or an error result |
| complete | `event: complete\ndata:\n\n` | directly after a single result, followed by the end of the body; never in a subscription stream |

- `<result>` is the execution result as compact JSON on one line, with `data` (`null` in an error result) and `errors` when there is at least one. Compact JSON escapes every line break inside strings, so a client that reads only the first `data:` line of a block reads the whole result.
- `<event id>` is the event id in decimal, without leading zeros. The `id:` line holds it whatever the selection set holds, also when `id` is not selected or is aliased. A browser `EventSource` stores it as its last event id and sends it back in `Last-Event-ID` when it reconnects. A single result carries no `id:` line, so a query or an error result never changes the last event id.
- The `data:` line of `complete` is bare: an `EventSource` dispatches the event with empty data, and no client reads it as a result.
- No other event name is ever written, so an event name never equals an event type name, and no `retry:` field is written.
- The lines of a subscription result come in another order than in a frame of the SSE route, which starts with `id:`; both are valid Server-Sent Events.

### Queries over Server-Sent Events

A query sent with the event stream, in either form, runs completely before the head, inside `HTTP_REQUEST_TIMEOUT`. It is answered with the head, the comment, exactly one `next` with its execution result, then `complete`, then the end of the body. The result is the one the JSON transport answers for the same request in the same state, field errors included (for example the `VALIDATION_FAILED` of the page limits of `eventTypes`); introspection queries are answered the same way. A query ignores `Last-Event-ID`, takes no subscriber slot and is never refused by the hub, also not on a node that is starting. A mutation is answered as a document error, because the schema has no mutation root. With `EVENT_TYPES` holding `user.created` and `order.paid`, the query `{ eventTypes { name } }` is answered with these blocks after the head:

```text
:

event: next
data: {"data":{"eventTypes":[{"name":"order.paid"},{"name":"user.created"}]}}

event: complete
data:

```

### Answers before a GraphQL stream

A request that gets the event stream is answered by the first rule that applies. This is the order of the REST streaming routes: validation answers come before availability answers.

1. A CORS preflight (`OPTIONS`) is answered by the CORS layer (see [Browser access](#browser-access)).
2. A transport error is answered with its problem and no stream: on POST, the 415, 413, 400 and 422 problems of the JSON transport, in that order; on GET, the 414, 400 and 422 problems of the query string (rows 2 to 5 of [Answers before a stream opens](#answers-before-a-stream-opens)).
3. A document error or a breach of the one-root-field rule is answered inside an event stream.
4. Invalid arguments of `events` are answered inside an event stream.
5. A node that cannot take the subscriber answers the 503 problem, type `about:blank`, with the `detail`, by precedence, `the node is shutting down`, `the node is starting` or `the node is at its subscriber capacity`. No head and no `next` are written, and no slot is taken.
6. Otherwise the event stream opens.

At any point before the head, the request budget may expire; the answer is then the 503 problem `The request exceeded its time budget.`. A 503 is safe to retry on any node, and the graphql-sse client retries it by itself. A transport problem is a fault of the request and is not retried.

### Error results inside a GraphQL stream

Faults of the operation travel through an accepted event stream, as the graphql-sse protocol requires: a page can read them, which a refused `EventSource` request would hide from it, and the graphql-sse client does not retry them. Each is answered with one single result `{"data":null,"errors":[...]}`, then `complete`, then the end of the body. No subscriber is registered, `subscribers` does not change, and no log entry is written.

**Invalid arguments.** The one error has the `message` `The request failed validation.`, `extensions.code` `VALIDATION_FAILED` and `extensions.violations` with exactly one entry:

| Input | `propertyPath` | `code` | `message` |
| --- | --- | --- | --- |
| `types: []` | `types` | `required` | `types must hold 1 to 100 event type names` |
| `types` holding an empty name | `types` | `required` | `types must not hold an empty event type name` |
| a name longer than 128 characters | `types` | `length` | `An event type must contain between 1 and 128 characters.` |
| a name that breaks the pattern | `types` | `pattern` | `An event type must match ^[a-z][a-z0-9]*([._-][a-z0-9]+)*$.` |
| more than 100 distinct names | `types` | `max_items` | `types must hold 1 to 100 event type names` |
| a `Last-Event-ID` header that is not a cursor (text, a sign, blank, overflow) | `Last-Event-ID` | `format` | `Last-Event-ID must be a decimal unsigned 64-bit integer` |
| a `lastEventId` argument that is not a cursor, also beside a valid header | `lastEventId` | `format` | `lastEventId must be a decimal unsigned 64-bit integer` |

The names are checked in list order, and the first fault answers; a repeated name counts once. The types are checked before the cursor and the header before the argument, and only the first failing check answers. No message echoes a value of the request. For example, `subscription { events(types: []) { id type data } }` is answered with these blocks after the head and the comment:

```text
event: next
data: {"data":null,"errors":[{"message":"The request failed validation.","locations":[{"line":1,"column":16}],"path":["events"],"extensions":{"code":"VALIDATION_FAILED","violations":[{"propertyPath":"types","message":"types must hold 1 to 100 event type names","code":"required"}]}}]}

event: complete
data:

```

**One root field.** A subscription operation must hold exactly one selection in its root selection set, a field selection written directly at the root: no fragment spread and no inline fragment at the root, even one that expands to one field; no field whose name starts with `__`; and no `@skip` or `@include` on that field. The same response name written twice counts as two selections. The rule is checked for the operation that runs, which `operationName` selects, before any resolver runs; fragments and directives inside the selection set of the field work as described above. A breach is answered with an error that has `locations` and no `path`, the `message` `The request failed validation.`, `extensions.code` `VALIDATION_FAILED` and one violation with `propertyPath` `query`, `code` `single_root_field` and `message` `a subscription operation must select exactly one root field`.

**Document errors.** A document that cannot be parsed or that fails validation is answered with the errors async-graphql reports, with the same `message` and `locations` as on the JSON transport and without `extensions.code`. Validation failures include:

- an unknown field or argument, or a wrong argument type;
- a variable that cannot be coerced;
- more than one operation without `operationName`, or an unknown `operationName`;
- a depth above 16 or a complexity above 1 000, among them a second selection of `Event.data` (see [The field and the event object](#the-field-and-the-event-object));
- a mutation, because the schema has no mutation root.

These messages may quote parts of the client's own request (a source line of a parse error; a field, argument, variable, fragment or operation name), exactly as on the JSON transport, and they are returned only to the requesting client. The errors that the service writes itself, of invalid arguments and of the root rule, hold no value of the request.

### The cursor of a GraphQL subscription

The cursor has two carriers: the `Last-Event-ID` request header, which both forms read, and the `lastEventId` argument, written in the document or passed through a variable. A `lastEventId` that is `null` or a variable that is left out is no carrier. Every carrier that is present must hold a cursor: a malformed `lastEventId` is refused also beside a valid header. When both hold a cursor, the header wins, as on the REST streaming routes: a browser `EventSource` reconnects with its original URL, whose document may still hold an old argument, and with a newer header.

A subscription with the cursor `c` receives every retained event of its types with an id above `c`, in id order, then live events, with no gap and no duplicate between the phases, exactly as on the REST streaming routes: from the window, from Kafka, or after the wait of a cursor ahead of the node, with the replay floor and the clamp to the end of the topic (see [Opening a stream](#opening-a-stream) and [Resume, the window and the replay floor](#resume-the-window-and-the-replay-floor)). A subscription without a cursor receives live events only, and the cursor `0` replays every retained event. All the types of a GraphQL subscription are named in its request, so its cursor replays every type it holds. On one connection the ids of the results strictly increase, also after the subscriber fell behind and caught up: no id is written twice, and no id at or below the cursor is written. A query ignores the header.

### Keep-alive and the end of a GraphQL stream

**Keep-alive.** Every event stream writes the comment block `:\n\n` right after the head, because some browsers hand a fetched response to the page only after its first body bytes. While a subscription stream stays open, the server writes `:\n\n` again whenever `STREAM_KEEP_ALIVE` (default 15 seconds) passes without a block, also while a replay waits for its turn or a cursor ahead of the node waits for the end of the topic. A comment carries no id and does not change the client's last event id. A client that accepts no data for `STREAM_KEEP_ALIVE` loses its connection, and a client that reads slowly is never disconnected (see [Clients that accept no data](#clients-that-accept-no-data)).

**End of a stream.** `complete` follows a single result only, the result of a query or an error result, and the body ends after it. A subscription stream never writes `complete`; it ends in one of four cases:

- the client closes the connection: the subscriber is dropped at once, and its slot is freed;
- the node shuts down: the stream ends after the results of the events already queued for it, without `complete`, and the node answers a new subscription with the 503 `the node is shutting down`;
- a replay below the window cannot read Kafka: the stream ends without `complete`;
- the reader of the node stopped: the node shuts down as in the second case.

In the last three cases the client reconnects, to any node, with its last id (see [Reconnecting a GraphQL subscription](#reconnecting-a-graphql-subscription)). A client therefore reads `complete` as "the operation has finished, do not send it again", and the end of a subscription stream without `complete` as "resume".

### Delivery and authorization

The GraphQL subscription has the [delivery guarantee](#delivery-guarantee) of the streaming routes: at least once, in id order, for the types of the subscription, across reconnects to any node, for retained events. The service removes no duplicates. The client deduplicates by `Event.data.id`, the envelope id, and not by `Event.id`, which is the position of the event: an event that a producer wrote twice has one envelope id and two event ids.

The GraphQL subscription has no subscriber authorization either: any client that reaches the service can subscribe through GraphQL to any well-formed event type, and every event on the topic can reach every subscriber of its type (see [No subscriber authorization](#no-subscriber-authorization)). `CORS_ALLOWED_ORIGINS` governs both forms unchanged (see [Browser access](#browser-access)): the preflight of the POST form, which sends `Content-Type: application/json` and may add `Last-Event-ID`, is answered for an allowed origin, and the GET form, as an `EventSource` sends it, needs no preflight. No credentials are allowed, so the graphql-sse client keeps its default `credentials: 'same-origin'` and a page creates its `EventSource` without `withCredentials`.

### Reconnecting a GraphQL subscription

Rules 1 to 3 of [Reconnecting](#reconnecting) hold: reconnect to any node, with the last id received, after a delay that grows with each failed attempt and has a random part. The last id is the `id:` line of the last subscription result, which is also `Event.id` when the selection holds `id`. Rule 4 reads, for the GraphQL forms: an error result followed by `complete` is a fault of the request and is not sent again unchanged, nor is a request that got a 400, 413, 414, 415 or 422 problem; a 503 and a subscription stream that ended without `complete` are retried.

**The graphql-sse client.** Keep its default distinct connections mode, `singleConnection: false`. Give it a `headers` function that adds `Last-Event-ID` with the last id: the client calls the function on every attempt and sends its first body again, so a retry resumes after the last event, because the header wins over a `lastEventId` that the first body may hold. The client retries an answer other than 200 and a stream that ended without `complete`, with its randomised exponential backoff (the option `retry`), up to `retryAttempts` failed attempts in a row (5 by default); then it calls `error`, and the application subscribes again later. After an error result it calls `complete` and does not retry.

```js
import { createClient } from 'graphql-sse';

const TYPES = ['user.created', 'order.paid'];
let lastId = null; // the event id of the last result
const seen = new Set(); // envelope ids already handled; a real client bounds it by the rule of "Delivery guarantee"

const client = createClient({
  url: '/api/graphql',
  headers: () => (lastId === null ? {} : { 'Last-Event-ID': lastId }),
  retryAttempts: 10,
});

client.subscribe(
  {
    query: 'subscription ($types: [String!]!) { events(types: $types) { id type data } }',
    variables: { types: TYPES },
  },
  {
    next: result => {
      if (result.errors) {
        console.error(result.errors); // a fault of the request; complete follows
        return;
      }
      const { id, data: envelope } = result.data.events;
      lastId = id;
      if (seen.has(envelope.id)) return;
      seen.add(envelope.id);
      console.log(envelope.type, envelope.data); // the application's own handling goes here
    },
    error: error => console.error(error), // the attempts are spent: subscribe again later, with a growing delay
    complete: () => {},
  }
);
```

**A browser `EventSource`.** It uses the GET form. Register listeners for the event names `next` and `complete`: `onmessage` receives nothing, because every block has an event name. Call `close()` on `complete`: the operation has finished, and an `EventSource` would otherwise send the request again after the end of the body. The browser reconnects by itself, with `Last-Event-ID`, after a subscription stream that was open ended without `complete`. It does not retry an answer other than 200: it closes and fires `error`. The page then creates a new `EventSource` and puts the last id into the request as the `lastEventId` argument, in the document or in the variables, because it cannot set the header.

```js
const TYPES = ['user.created', 'order.paid'];
const QUERY =
  'subscription ($types: [String!]!, $after: String) { events(types: $types, lastEventId: $after) { id type data } }';
let lastId = null;
let attempt = 0;
const seen = new Set(); // envelope ids already handled; a real client bounds it by the rule of "Delivery guarantee"

function connect() {
  const url = new URL('/api/graphql', window.location.origin);
  const variables = lastId === null ? { types: TYPES } : { types: TYPES, after: lastId };
  url.searchParams.set('query', QUERY);
  url.searchParams.set('variables', JSON.stringify(variables));
  const source = new EventSource(url);
  source.onopen = () => { attempt = 0; };
  source.addEventListener('next', message => {
    const result = JSON.parse(message.data);
    if (result.errors) {
      console.error(result.errors); // a fault of the request; complete follows
      return;
    }
    lastId = message.lastEventId;
    const envelope = result.data.events.data;
    if (seen.has(envelope.id)) return;
    seen.add(envelope.id);
    console.log(envelope.type, envelope.data); // the page's own handling goes here
  });
  source.addEventListener('complete', () => source.close());
  source.onerror = () => {
    if (source.readyState !== EventSource.CLOSED) return; // the browser reconnects by itself, with Last-Event-ID
    const delay = Math.min(30000, 1000 * 2 ** attempt) * (0.5 + Math.random() / 2);
    attempt += 1;
    setTimeout(connect, delay);
  };
}

connect();
```

**urql.** Set `fetchSubscriptions: true` on the client. urql then sends a subscription with `Accept: text/event-stream, multipart/mixed`, which gets the event stream, while its queries rank the JSON types as high as the event stream and stay on the JSON transport. urql does not resume a subscription by itself: the application keeps the last `Event.id` and, when the subscription ends without an error result or its request fails, starts it again with that id as `lastEventId`, after the growing delay.

```js
import { Client, cacheExchange, fetchExchange } from '@urql/core';

const client = new Client({
  url: '/api/graphql',
  exchanges: [cacheExchange, fetchExchange],
  fetchSubscriptions: true,
});
```

**Apollo Client** reaches the service through a link that uses the graphql-sse client, the recipe of the graphql-sse documentation. The link takes the options of the graphql-sse client above, `headers` and `retryAttempts` included; route the subscriptions to it with `split`, or every operation. Relay uses the graphql-sse client the same way in its network layer.

```js
import { ApolloLink, Observable } from '@apollo/client/core';
import { print } from 'graphql';
import { createClient } from 'graphql-sse';

class SseLink extends ApolloLink {
  constructor(options) {
    super();
    this.client = createClient(options);
  }

  request(operation) {
    return new Observable(sink =>
      this.client.subscribe(
        { ...operation, query: print(operation.query) },
        {
          next: sink.next.bind(sink),
          complete: sink.complete.bind(sink),
          error: sink.error.bind(sink),
        }
      )
    );
  }
}
```

### Connections per browser

Every subscription in the distinct connections mode, every `EventSource` and every stream of the SSE route holds one HTTP connection for its lifetime. Over HTTP/1.1 a browser opens at most 6 connections per origin, shared by every tab of that origin, so a page that holds six streams leaves no connection for any other request to the service. Serve the service to browsers through an edge proxy that speaks HTTP/2 (see [`operations.md`](operations.md)): HTTP/2 carries many streams over one connection, up to the number of concurrent streams the proxy allows. A page subscribes once for all the types it needs, up to 100, instead of once per type.

The single connection mode of graphql-sse, which would carry every operation of a client over one stream, is not supported: its first request, a PUT that reserves the stream, gets the 405 problem with `Allow: GET,HEAD,POST`. Clients keep their default distinct connections mode.

## Limits

Every input a client controls is bounded, and the bounds are the same on every node:

| Input | Limit |
| --- | --- |
| event types per subscription (the parameter or the GraphQL argument `types`), per command and per WebSocket connection | 100 distinct names |
| event type name | 1 to 128 characters of the pattern above |
| cursor, in the header, the parameter or the GraphQL argument | a decimal unsigned 64-bit integer |
| WebSocket command | 16 384 bytes |
| WebSocket message or frame | 32 768 bytes |
| GraphQL request body (POST form) and query string (GET form) | `HTTP_MAX_BODY_BYTES` (default 65 536 bytes); then the 413 or the 414 problem |
| GraphQL document | depth 16 and complexity 1 000; `Event.data` costs 501, so a document selects it at most once and each result holds one copy of the event object |
| GraphQL operations per request | one; a batch is refused |
| root fields of a GraphQL subscription | one field selection, written directly at the root |
| time before the head of a GraphQL event stream | `HTTP_REQUEST_TIMEOUT` (default 10 seconds); then the 503 `The request exceeded its time budget.` |
| events queued for one subscriber | `SUBSCRIBER_BUFFER` (default 256); a client further behind catches up from the window or Kafka |
| replay reads from Kafka at a time on one node | `EVENT_REPLAY_PERMITS` (default 4), pages of up to 500 records |
| how far back a resume reaches | `EVENT_REPLAY_MAX_EVENTS` offsets below the node's position (default 1 000 000), and the topic's retention |
| streams on one node | `MAX_SUBSCRIBERS` (default 100 000) for SSE, WebSocket and GraphQL subscriptions together (a GraphQL query takes no slot); then the 503 `the node is at its subscriber capacity` |
| time a connection may accept no data | `STREAM_KEEP_ALIVE` (default 15 seconds) |

The service sets no limit per client, such as connections per address or commands per second; the proxy in front of the service limits connections per client address (see [`operations.md`](operations.md)).
