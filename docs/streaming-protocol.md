# Streaming protocol

This document specifies the two streaming routes of the service, `GET /api/events/stream` (Server-Sent Events) and `GET /api/events/ws` (WebSocket), for the authors of clients. Neither route is in the OpenAPI document: OpenAPI cannot describe a WebSocket, and a response that never ends would hang the contract tests. The catalog `GET /api/event-types` and the node status `GET /api/events/status` are ordinary REST operations of the OpenAPI document; [`requests.http`](requests.http) has one example request for each of the four routes. Variable names and defaults are listed in [`environment.md`](environment.md); how a node is run is described in [`operations.md`](operations.md).

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
- [Limits](#limits)

## Events, ids and cursors

Producers write events to one Kafka topic with exactly one partition. Every node of the service reads the whole topic and sends each event to its own subscribers of the event's type.

**Event type names.** A name has 1 to 128 characters and matches `^[a-z][a-z0-9]*([._-][a-z0-9]+)*$`, for example `user.created` or `order-paid`. The catalog of `GET /api/event-types` lists the names the deployment announces (`EVENT_TYPES`). It is informative: a client may subscribe to any well-formed name, and an event of a type that is not in the catalog still reaches the subscribers of that type.

**Event id.** The event at Kafka offset `o` has the id `o + 1`, written in decimal. The id of an event is the same on every node, so a client can resume on any node. Ids are not dense: transaction markers, records of aborted transactions, invalid records and events of other types take offsets, so a client sees gaps. A gap is never a loss.

**Cursor.** A cursor is a decimal unsigned 64-bit integer and means "every id up to this one was seen". A client resumes by sending the id of the last event it received; the stream then delivers ids greater than the cursor only. The cursor `0` asks for every retained event.

**The event object.** Each event reaches the client as one compact JSON object, the same object on both transports:

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

Every refusal is a problem document (RFC 9457, `application/problem+json`) and opens no stream and no socket. The checks run in the order of the table; the first that applies answers.

| Order | Status | Problem | When |
| --- | --- | --- | --- |
| 1 | 403 | type `about:blank`, `detail` `the origin is not allowed` | WebSocket route only: the handshake carries an `Origin` header that `CORS_ALLOWED_ORIGINS` does not allow (see [Browser access](#browser-access)); no subscriber is registered |
| 2 | 400 | type `about:blank`, `detail` the parser text, which names the parameter | the query string cannot be read: `types` is missing on the SSE route, or a parameter is given twice on either route |
| 3 | 422 | type `/problems/validation-failed`, one entry in `violations` with `propertyPath` `types` | `types` is empty or holds an empty name (`code` `required`), a name that is not an event type name (`code` `length` or `pattern`), or more than 100 distinct names (`code` `max_items`); a name of the request is never echoed |
| 4 | 400 | type `about:blank`, `detail` `Last-Event-ID must be a decimal unsigned 64-bit integer` or `lastEventId must be a decimal unsigned 64-bit integer` | a carrier of the cursor that is present does not hold a decimal unsigned 64-bit integer (empty, text, a sign, blank, overflow); the detail names that carrier. The header is checked before the parameter, and a malformed parameter is refused also beside a valid header |
| 5 | 503 | type `about:blank`, `detail` `the node is shutting down` | the node is stopping |
| 6 | 503 | type `about:blank`, `detail` `the node is starting` | the node has not yet read to the end of the topic once since it started, or cannot reach its broker or its topic at start |
| 7 | 503 | type `about:blank`, `detail` `the node is at its subscriber capacity` | the node already holds `MAX_SUBSCRIBERS` streams |
| 8 | 4xx | type `about:blank`, `detail` the text of the rejection | WebSocket route only: the request is not a valid WebSocket upgrade, for example 400 for a missing `Sec-WebSocket-Key` header or a `Connection` header without `upgrade`, 426 for a connection that cannot be upgraded |

A 503 is safe to retry on any node. A 400, 403 or 422 is a fault of the request and is not retried (see [Reconnecting](#reconnecting)).

A request with a cursor below the node's memory window is accepted while the node cannot read Kafka; its stream then ends at the first replay read that fails (SSE: the response ends; WebSocket: close code 1011).

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

Replays from Kafka are limited per node: at most `EVENT_REPLAY_PERMITS` replays read at a time and the others wait their turn, in order, inside their streams. A replay reads its next page only after its client took the frames of the last one, so a client that reads nothing causes no read. When a replay read fails, or three page reads in a row return no record and do not move the position, the stream ends (SSE: the response ends; WebSocket: close code 1011) and the client reconnects with its last id, to any node.

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

## Limits

Every input a client controls is bounded, and the bounds are the same on every node:

| Input | Limit |
| --- | --- |
| event types per subscription, per command and per WebSocket connection | 100 distinct names |
| event type name | 1 to 128 characters of the pattern above |
| cursor | a decimal unsigned 64-bit integer |
| WebSocket command | 16 384 bytes |
| WebSocket message or frame | 32 768 bytes |
| events queued for one subscriber | `SUBSCRIBER_BUFFER` (default 256); a client further behind catches up from the window or Kafka |
| replay reads from Kafka at a time on one node | `EVENT_REPLAY_PERMITS` (default 4), pages of up to 500 records |
| how far back a resume reaches | `EVENT_REPLAY_MAX_EVENTS` offsets below the node's position (default 1 000 000), and the topic's retention |
| streams on one node | `MAX_SUBSCRIBERS` (default 100 000); then the 503 `the node is at its subscriber capacity` |
| time a connection may accept no data | `STREAM_KEEP_ALIVE` (default 15 seconds) |

The service sets no limit per client, such as connections per address or commands per second; the proxy in front of the service limits connections per client address (see [`operations.md`](operations.md)).
